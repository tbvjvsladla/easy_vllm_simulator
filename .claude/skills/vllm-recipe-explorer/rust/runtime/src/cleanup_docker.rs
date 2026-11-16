//! `cleanup_docker.py`: the deterministic docker cleanup of the recipe explorer.
//!
//! * `_run(cmd, timeout=60)`: `subprocess.run` with both pipes and the timeout ([`crate::cleanup_process`]); any ordinary exception is
//!   `(1, str(error))`, otherwise `(returncode, stdout decoded as UTF-8 with replacement)`.
//! * `preserve_set(canonical_only=False)`: the refs that must never be deleted: the `IMAGE_TAG=` lines of the delivered env files (multi,
//!   then single, in the directory's own order), the image-like strings of the `resolved.json` files (found by the original's regular
//!   expression over `json.dumps` of the loaded JSON, not by walking it), the mandatory canonical production resolution and the two canonical
//!   Dockerfile templates (any defect is a `RuntimeError` raised from its cause), the optional delivered Dockerfiles, and the images
//!   of `docker ps -a`; each ref keeps the first reason it was given.
//! * `list_images()`: `docker image ls -a --format {{json .}}`, one JSON value per non-blank line.
//! * `main(canonical_only=False)`: the argument parsing of the original, the keep table, the candidate table, the usage, and with `--apply` the
//!   `docker rmi` of every candidate and the `docker builder prune`.
//!
//! Every file and process step is told to an [`Observe`] when the caller asks (the checks' control record); production tells nothing. The
//! module's `REPO` is a value given to each call, never read from the environment. Nothing here knows a case, a provider or an answer.

use crate::cleanup_process::run_text;
use crate::cliparse::{Kind, Opt, Spec, Value};
use crate::escalation::Raised;
use crate::functional_smoke::Observe;
use crate::memory::open_text_file;
use crate::model_files::{fspath, join, os_failure, splitlines};
use crate::pyexpr::{get, get_or};
use crate::transform_cli::{parse_command, Console};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::{decode_utf8_strict, loads_text};
use agent_runtime_support::pystr::{EncodeFailure, PyStr};
use agent_runtime_support::pyvalue::{DumpError, PyDict, PyValue};
use std::collections::HashSet;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;

pub const MODULE: &str = "cleanup_docker";

// ── exceptions with their constructor fields ─────────────────────────────────────────────────────

/// The fields the builtin classes need to be rebuilt (the ACTUAL ones: nothing is guessed from a message).
#[derive(Clone, Debug)]
pub enum Ctor {
    Json {
        msg: PyStr,
        doc: PyStr,
        pos: usize,
    },
    Unicode {
        encoding: String,
        object: Vec<u8>,
        start: usize,
        end: usize,
        reason: String,
    },
    /// `UnicodeEncodeError(encoding, object, start, end, reason)`: the encoder's own failure (the `str` that was being encoded).
    Encode {
        encoding: String,
        object: PyStr,
        start: usize,
        end: usize,
        reason: String,
    },
}

/// An exception of a call with the constructor fields of each row of its chain (the top row first).
#[derive(Clone, Debug)]
pub struct Thrown {
    pub raised: Raised,
    pub ctors: Vec<Option<Ctor>>,
    /// The canonical builtin classes in the MRO of an exception the CALLER reported (`builtin_bases` of a write acknowledgement), in MRO order;
    /// `None` for an exception raised here. It classifies argparse's `except (AttributeError, OSError)` and is not part of the result chain.
    pub bases: Option<Vec<String>>,
}

pub type Done<T> = Result<T, Box<Thrown>>;

impl Thrown {
    pub fn plain(raised: Raised) -> Box<Thrown> {
        Box::new(Thrown {
            raised,
            ctors: Vec::new(),
            bases: None,
        })
    }

    /// `raise Class(message) from cause`.
    fn from_cause(class: &str, message: PyStr, cause: &Thrown) -> Box<Thrown> {
        let top = Raised::new(class, "builtins", message).from_cause(cause.raised.clone());
        let mut ctors = vec![None];
        ctors.extend(cause.ctors.iter().cloned());
        if ctors.iter().all(Option::is_none) {
            ctors.clear();
        }
        Box::new(Thrown {
            raised: top,
            ctors,
            bases: None,
        })
    }

    /// The chain rows of `raised`, each with its `constructor` when it has one.
    pub fn chain(&self) -> PyValue {
        let mut chain = self.raised.chain_value();
        if let PyValue::List(rows) = &mut chain {
            for (index, row) in rows.iter_mut().enumerate() {
                let Some(Some(ctor)) = self.ctors.get(index) else {
                    continue;
                };
                let PyValue::Dict(row) = row else { continue };
                let mut fields = PyDict::new();
                match ctor {
                    Ctor::Json { msg, doc, pos } => {
                        fields.set("kind", PyValue::str("json_decode"));
                        fields.set("msg", PyValue::Str(msg.clone()));
                        fields.set("doc", PyValue::Str(doc.clone()));
                        fields.set("pos", PyValue::int(*pos));
                    }
                    Ctor::Unicode {
                        encoding,
                        object,
                        start,
                        end,
                        reason,
                    } => {
                        fields.set("kind", PyValue::str("unicode_decode"));
                        fields.set("encoding", PyValue::str(encoding));
                        fields.set(
                            "object_hex",
                            PyValue::str(
                                &object
                                    .iter()
                                    .map(|b| format!("{b:02x}"))
                                    .collect::<String>(),
                            ),
                        );
                        fields.set("start", PyValue::int(*start));
                        fields.set("end", PyValue::int(*end));
                        fields.set("reason", PyValue::str(reason));
                    }
                    Ctor::Encode {
                        encoding,
                        object,
                        start,
                        end,
                        reason,
                    } => {
                        fields.set("kind", PyValue::str("unicode_encode"));
                        fields.set("encoding", PyValue::str(encoding));
                        fields.set("object", PyValue::Str(object.clone()));
                        fields.set("start", PyValue::int(*start));
                        fields.set("end", PyValue::int(*end));
                        fields.set("reason", PyValue::str(reason));
                    }
                }
                row.set("constructor", PyValue::Dict(fields));
            }
        }
        chain
    }
}

fn from_raised(raised: Raised) -> Box<Thrown> {
    Thrown::plain(raised)
}

fn from_failure(failure: Failure) -> Box<Thrown> {
    Thrown::plain(Raised::from(failure))
}

fn dump_error(error: DumpError) -> Box<Thrown> {
    let class = error.python_type();
    let shown = error.to_string();
    let message = shown
        .strip_prefix(&format!("{class}: "))
        .unwrap_or(&shown)
        .to_string();
    Thrown::plain(Raised::plain(class, &message))
}

fn exit_with(code: i64) -> Box<Thrown> {
    let mut raised = Raised::new("SystemExit", "builtins", PyStr::new(&code.to_string()));
    raised.code = Some(Box::new(PyValue::int(code)));
    Thrown::plain(raised)
}

// ── the world a call runs in ─────────────────────────────────────────────────────────────────────

/// The console of a call: the parser's sinks ([`Console`]) and the two real writes the business code makes, each of which can fail.
pub trait CleanupConsole: Console {
    /// One `sys.stdout.write(text)`. The failure is the exception of that write (a `print` propagates it into the business code).
    fn out(&mut self, text: &PyStr) -> Done<()>;
    /// One `sys.stderr.write(text)` of a `print(..., file=sys.stderr)`.
    fn err(&mut self, text: &PyStr) -> Done<()>;
    /// The exception an argparse output write raised that argparse does NOT swallow (see [`argparse_swallows`]): the parser stopped at that
    /// write, so the exception, not the `SystemExit`, is the outcome of the parse. Taken once; `None` when every write succeeded or failed
    /// with an `AttributeError` or an `OSError`.
    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        None
    }
}

/// `except (AttributeError, OSError): pass` of `ArgumentParser._print_message`: whether the exception is one of those or a subclass.
/// An exception the caller reported carries the canonical builtin classes of its real MRO: it is swallowed exactly when `OSError` or
/// `AttributeError` is among them, whatever its own name and module say. An exception raised here has no such descriptor and is judged by
/// its builtin class name (the `OSError` family, `AttributeError`).
pub fn argparse_swallows(thrown: &Thrown) -> bool {
    match &thrown.bases {
        Some(bases) => bases
            .iter()
            .any(|base| base == "OSError" || base == "AttributeError"),
        None => {
            let raised = &thrown.raised;
            raised.module == "builtins"
                && (raised.class == "AttributeError"
                    || raised.class == "ConnectionError"
                    || is_os_error(&raised.class))
        }
    }
}

/// The outcome of one argparse output write: `Some` only for an exception argparse does not swallow.
pub fn parser_write_failure(outcome: Done<()>) -> Option<Box<Thrown>> {
    outcome.err().filter(|thrown| !argparse_swallows(thrown))
}

/// The console the call prints to, and whether the file and process steps are told to it.
pub struct World<'a> {
    pub console: &'a mut dyn CleanupConsole,
    pub tracing: bool,
}

impl Observe for World<'_> {
    fn enabled(&self) -> bool {
        self.tracing
    }

    fn record(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.console.observe(name, fields);
    }
}

fn note(world: &mut World<'_>, name: &str, fields: Vec<(&str, PyValue)>) {
    if world.tracing {
        world.console.observe(name, fields);
    }
}

/// `print(text)` to standard output: the text and then its newline are two writes, each of which can fail before the next step.
fn print(world: &mut World<'_>, text: &PyStr) -> Done<()> {
    world.console.out(text)?;
    world.console.out(&PyStr::new("\n"))
}

// ── paths and files ──────────────────────────────────────────────────────────────────────────────

/// `os.path.join(repo, *parts)`: a `repo` that is not a path fails first (`os.fspath`).
fn join_repo(repo: &PyValue, parts: &[&str]) -> Done<PyStr> {
    let mut path = fspath(repo, false).map_err(from_failure)?.clone();
    for part in parts {
        path = join(&path, &PyStr::new(part));
    }
    Ok(path)
}

fn native(path: &PyStr) -> Option<std::path::PathBuf> {
    let bytes = path.to_surrogateescape_bytes().ok()?;
    if bytes.contains(&0) {
        return None;
    }
    Some(std::path::PathBuf::from(std::ffi::OsStr::from_bytes(
        &bytes,
    )))
}

/// `os.path.isdir` / `os.path.isfile`, observed.
fn exists_as(world: &mut World<'_>, name: &str, path: &PyStr) -> bool {
    note(world, name, vec![("path", PyValue::Str(path.clone()))]);
    let found = native(path)
        .and_then(|native| std::fs::metadata(native).ok())
        .is_some_and(|metadata| {
            if name == "isdir" {
                metadata.is_dir()
            } else {
                metadata.is_file()
            }
        });
    note(
        world,
        &format!("{name}_return"),
        vec![("value", PyValue::Bool(found))],
    );
    found
}

/// `os.listdir(path)` in the order the directory gives its entries, observed.
fn list_dir(world: &mut World<'_>, path: &PyStr) -> Done<Vec<PyStr>> {
    note(world, "listdir", vec![("path", PyValue::Str(path.clone()))]);
    let outcome: Result<Vec<PyStr>, Failure> = match native(path) {
        None => Err(Failure::new("ValueError", "embedded null byte")),
        Some(native) => std::fs::read_dir(&native)
            .map_err(|error| os_failure(&error, Some(path)))
            .and_then(|entries| {
                entries
                    .map(|entry| {
                        entry
                            .map(|entry| PyStr::from_surrogateescape(entry.file_name().as_bytes()))
                            .map_err(|error| os_failure(&error, Some(path)))
                    })
                    .collect()
            }),
    };
    match outcome {
        Ok(names) => {
            note(
                world,
                "listdir_return",
                vec![(
                    "value",
                    PyValue::List(names.iter().cloned().map(PyValue::Str).collect()),
                )],
            );
            Ok(names)
        }
        Err(failure) => {
            let raised = Raised::from(failure);
            note(
                world,
                "listdir_error",
                vec![
                    ("type", PyValue::str(&raised.class)),
                    ("module", PyValue::str(&raised.module)),
                    ("message", PyValue::Str(raised.message.clone())),
                ],
            );
            Err(from_raised(raised))
        }
    }
}

/// How the text of a file is decoded (`errors=`): `None` is the default, strict.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Errors {
    Default,
    Strict,
    Replace,
}

/// The `UnicodeDecodeError` of `bytes.decode("utf-8")` with its constructor fields.
fn unicode_error(bytes: &[u8]) -> Option<Box<Thrown>> {
    let error = std::str::from_utf8(bytes).err()?;
    let start = error.valid_up_to();
    let (length, reason) = match error.error_len() {
        None => (bytes.len() - start, "unexpected end of data"),
        Some(length) if (0xC2..=0xF4).contains(&bytes[start]) => {
            (length, "invalid continuation byte")
        }
        Some(length) => (length, "invalid start byte"),
    };
    let message = agent_runtime_support::pyjson::utf8_error_text(bytes, &error);
    Some(Box::new(Thrown {
        bases: None,
        raised: Raised::plain("UnicodeDecodeError", &message),
        ctors: vec![Some(Ctor::Unicode {
            encoding: "utf-8".to_string(),
            object: bytes.to_vec(),
            start,
            end: start + length,
            reason: reason.to_string(),
        })],
    }))
}

/// Universal newlines of a text file opened without `newline=`.
fn translate_newlines(text: PyStr) -> PyStr {
    let points = text.code_points();
    if !points.contains(&0x0D) {
        return text;
    }
    let mut out = Vec::with_capacity(points.len());
    let mut at = 0;
    while at < points.len() {
        if points[at] == 0x0D {
            out.push(0x0A);
            if points.get(at + 1) == Some(&0x0A) {
                at += 1;
            }
        } else {
            out.push(points[at]);
        }
        at += 1;
    }
    PyStr::from_code_points(out)
}

/// The `open(path)` of a text file. The path is converted for the operating system first (`os.fsencode`: UTF-8 with `surrogateescape`); a
/// lone surrogate that cannot be escaped is the `UnicodeEncodeError` of that encoder, with the five constructor fields of its own failure.
fn open_checked(path: &PyStr) -> Done<std::fs::File> {
    if let Err(failure) = path.to_surrogateescape_bytes() {
        return Err(encode_thrown(path, &failure));
    }
    open_text_file(path).map_err(from_failure)
}

/// The `UnicodeEncodeError` of encoding `object`: the class and message of the encoder's failure and its five constructor fields.
pub fn encode_thrown(object: &PyStr, failure: &EncodeFailure) -> Box<Thrown> {
    Box::new(Thrown {
        bases: None,
        raised: Raised::plain("UnicodeEncodeError", &failure.message()),
        ctors: vec![Some(Ctor::Encode {
            encoding: "utf-8".to_string(),
            object: object.clone(),
            start: failure.start,
            end: failure.end,
            reason: "surrogates not allowed".to_string(),
        })],
    })
}

/// The exception of a failed write to a real stream (`BrokenPipeError`, `OSError`, ...).
pub fn write_thrown(error: &std::io::Error) -> Box<Thrown> {
    from_failure(crate::stdio::os_failure(error))
}

/// `open(path, encoding="utf-8"[, errors=...])` and `.read()`, observed as the collector observes them.
fn read_file(world: &mut World<'_>, path: &PyStr, errors: Errors) -> Done<PyStr> {
    let mut keywords = PyDict::new();
    keywords.set("encoding", PyValue::str("utf-8"));
    match errors {
        Errors::Default => {}
        Errors::Strict => keywords.set("errors", PyValue::str("strict")),
        Errors::Replace => keywords.set("errors", PyValue::str("replace")),
    }
    note(
        world,
        "file_open",
        vec![
            ("path", PyValue::Str(path.clone())),
            ("args", PyValue::List(Vec::new())),
            ("kwargs", PyValue::Dict(keywords)),
        ],
    );
    let mut file = match open_checked(path) {
        Ok(file) => file,
        Err(thrown) => {
            note(
                world,
                "file_open_error",
                vec![
                    ("path", PyValue::Str(path.clone())),
                    ("type", PyValue::str(&thrown.raised.class)),
                    ("message", PyValue::Str(thrown.raised.message.clone())),
                ],
            );
            return Err(thrown);
        }
    };
    note(
        world,
        "file_read",
        vec![
            ("args", PyValue::List(Vec::new())),
            ("kwargs", PyValue::Dict(PyDict::new())),
            ("path", PyValue::Str(path.clone())),
        ],
    );
    let mut bytes = Vec::new();
    let outcome: Done<PyStr> = match file.read_to_end(&mut bytes) {
        Err(error) => Err(from_failure(os_failure(&error, None))),
        Ok(_) if errors == Errors::Replace => Ok(translate_newlines(PyStr::new(
            &String::from_utf8_lossy(&bytes),
        ))),
        Ok(_) => match unicode_error(&bytes) {
            Some(error) => Err(error),
            None => Ok(translate_newlines(
                decode_utf8_strict(&bytes).expect("valid UTF-8 was checked"),
            )),
        },
    };
    match &outcome {
        Ok(text) => note(
            world,
            "file_read_return",
            vec![("text", PyValue::Str(text.clone()))],
        ),
        Err(failure) => note(
            world,
            "file_read_error",
            vec![
                ("type", PyValue::str(&failure.raised.class)),
                ("message", PyValue::Str(failure.raised.message.clone())),
            ],
        ),
    }
    outcome
}

/// `json.loads(text)`: the failure with the fields of `JSONDecodeError` when it is one.
fn load_json(text: &PyStr) -> Done<PyValue> {
    use agent_runtime_support::pyjson::LoadError;
    loads_text(text).map_err(|error| match &error {
        LoadError::Decode {
            message, position, ..
        } => {
            let mut raised = Raised::from(crate::model_files::load_failure(error.clone()));
            if message.starts_with("Expecting value") {
                raised.suppress_context = true;
            }
            Box::new(Thrown {
                bases: None,
                raised,
                ctors: vec![Some(Ctor::Json {
                    msg: PyStr::new(message),
                    doc: text.clone(),
                    pos: *position,
                })],
            })
        }
        _ => from_failure(crate::model_files::load_failure(error)),
    })
}

// ── the regular expressions of the original, by hand ─────────────────────────────────────────────

fn space(point: u32) -> bool {
    agent_runtime_support::pystr::is_python_space(point)
}

fn line_starts(points: &[u32], at: usize) -> bool {
    at == 0 || points[at - 1] == 0x0A
}

fn starts_with(points: &[u32], at: usize, literal: &str) -> bool {
    let wanted: Vec<u32> = literal.chars().map(u32::from).collect();
    points[at..].starts_with(&wanted)
}

/// `re.finditer(r"^IMAGE_TAG=(\S+)", text, re.M)`: the group of every match.
fn image_tags(text: &PyStr) -> Vec<PyStr> {
    let points = text.code_points();
    let mut found = Vec::new();
    for at in 0..points.len() {
        if !line_starts(points, at) || !starts_with(points, at, "IMAGE_TAG=") {
            continue;
        }
        let begin = at + "IMAGE_TAG=".len();
        let mut end = begin;
        while end < points.len() && !space(points[end]) {
            end += 1;
        }
        if end > begin {
            found.push(PyStr::from_code_points(points[begin..end].to_vec()));
        }
    }
    found
}

/// `re.finditer(r"^FROM\s+(\S+)", text, re.M)`: the group of every match (`\s+` may cross line breaks; matching resumes after the match).
fn from_refs(text: &PyStr) -> Vec<PyStr> {
    let points = text.code_points();
    let mut found = Vec::new();
    let mut at = 0;
    while at < points.len() {
        if line_starts(points, at) && starts_with(points, at, "FROM") {
            let mut cursor = at + 4;
            let spaces = cursor;
            while cursor < points.len() && space(points[cursor]) {
                cursor += 1;
            }
            if cursor > spaces {
                let begin = cursor;
                while cursor < points.len() && !space(points[cursor]) {
                    cursor += 1;
                }
                if cursor > begin {
                    found.push(PyStr::from_code_points(points[begin..cursor].to_vec()));
                    at = cursor;
                    continue;
                }
            }
        }
        at += 1;
    }
    found
}

/// `re.findall(r'"((?:easy-vllm|nvcr\.io/nvidia/pytorch)[^"]*)"', text)`.
fn image_strings(text: &str) -> Vec<PyStr> {
    let points: Vec<u32> = text.chars().map(u32::from).collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < points.len() {
        if points[at] == u32::from('"') {
            for prefix in ["easy-vllm", "nvcr.io/nvidia/pytorch"] {
                if starts_with(&points, at + 1, prefix) {
                    let mut end = at + 1 + prefix.chars().count();
                    while end < points.len() && points[end] != u32::from('"') {
                        end += 1;
                    }
                    if end < points.len() {
                        found.push(PyStr::from_code_points(points[at + 1..end].to_vec()));
                        at = end;
                        break;
                    }
                }
            }
        }
        at += 1;
    }
    found
}

// ── the keep set ─────────────────────────────────────────────────────────────────────────────────

/// `keep` (a set of `str`) and `why` (a dict from each ref to its first reason, in insertion order).
#[derive(Clone, Debug, Default)]
pub struct Keep {
    set: HashSet<Vec<u32>>,
    pub why: Vec<(PyStr, PyStr)>,
}

impl Keep {
    /// `add(ref, reason)` of the original: the ref is stripped, a blank one is nothing.
    fn add(&mut self, reference: &PyStr, reason: &PyStr) {
        let reference = reference.strip();
        if reference.is_empty() {
            return;
        }
        self.insert(reference, reason);
    }

    /// `keep.add(ref); why.setdefault(ref, reason)`.
    fn insert(&mut self, reference: PyStr, reason: &PyStr) {
        self.set.insert(reference.code_points().to_vec());
        if !self.why.iter().any(|(held, _)| *held == reference) {
            self.why.push((reference, reason.clone()));
        }
    }

    pub fn contains(&self, reference: &PyStr) -> bool {
        self.set.contains(reference.code_points())
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// `sorted(keep)`: by code point.
    pub fn sorted(&self) -> Vec<PyStr> {
        let mut all: Vec<Vec<u32>> = self.set.iter().cloned().collect();
        all.sort();
        all.into_iter().map(PyStr::from_code_points).collect()
    }

    fn reason_of(&self, reference: &PyStr) -> PyStr {
        self.why
            .iter()
            .find(|(held, _)| held == reference)
            .map_or_else(PyStr::default, |(_, reason)| reason.clone())
    }
}

fn is_os_error(class: &str) -> bool {
    crate::model_dyn::is_os_error(class)
}

/// `except (OSError, UnicodeError, JSONDecodeError, KeyError, TypeError, ValueError)`.
fn resolution_error(class: &str) -> bool {
    is_os_error(class)
        || crate::model_dyn::is_value_error(class)
        || class == "KeyError"
        || class == "TypeError"
}

/// `except (OSError, UnicodeError)`.
fn template_error(class: &str) -> bool {
    is_os_error(class) || class == "UnicodeDecodeError" || class == "UnicodeEncodeError"
}

fn text(value: &str) -> PyStr {
    PyStr::new(value)
}

fn with(prefix: &str, middle: &PyStr, suffix: &str) -> PyStr {
    let mut out = PyStr::new(prefix);
    out.push_py(middle);
    out.push_str(suffix);
    out
}

/// The mandatory canonical production resolution: every track's base image, or the `ValueError` the original raises.
fn read_resolution(world: &mut World<'_>, repo: &PyValue, keep: &mut Keep) -> Done<()> {
    let path = join_repo(
        repo,
        &[
            ".claude",
            "skills",
            "upstream-version-watch",
            "assets",
            "current-production-resolution.json",
        ],
    )?;
    let inner = |world: &mut World<'_>, keep: &mut Keep| -> Done<()> {
        let body = read_file(world, &path, Errors::Default)?;
        let resolution = load_json(&body)?;
        let mut sources: Vec<(PyStr, PyValue)> = vec![(
            text("default"),
            get(&resolution, "ngc_base").map_err(from_failure)?,
        )];
        let tracks = get(&resolution, "tracks").map_err(from_failure)?;
        let tracks = if tracks.is_truthy() {
            tracks
        } else {
            PyValue::Dict(PyDict::new())
        };
        // `.items()`: only a mapping has it
        let PyValue::Dict(entries) = tracks.peel() else {
            return Err(from_raised(Raised::plain(
                "AttributeError",
                &format!("'{}' object has no attribute 'items'", tracks.type_name()),
            )));
        };
        for (name, track) in entries.iter() {
            if matches!(track.peel(), PyValue::Dict(_)) {
                sources.push((
                    with("tracks.", name, ""),
                    get(track, "ngc_base").map_err(from_failure)?,
                ));
            }
        }
        let mut seen = false;
        for (label, base) in &sources {
            if !matches!(base.peel(), PyValue::Dict(_)) {
                continue;
            }
            let tag = get(base, "tag").map_err(from_failure)?;
            let tag = match tag.peel() {
                PyValue::Str(tag) if !tag.strip().is_empty() => tag.strip(),
                _ => {
                    return Err(from_raised(Raised::plain(
                        "ValueError",
                        &format!(
                            "empty ngc_base.tag ({})",
                            label.to_utf8().unwrap_or_default()
                        ),
                    )))
                }
            };
            keep.add(
                &with("nvcr.io/nvidia/pytorch:", &tag, ""),
                &with("shared production resolution (", label, ")"),
            );
            seen = true;
        }
        if !seen {
            return Err(from_raised(Raised::plain(
                "ValueError",
                "no ngc_base.tag in any track",
            )));
        }
        Ok(())
    };
    match inner(world, keep) {
        Ok(()) => Ok(()),
        Err(error) if resolution_error(&error.raised.class) => Err(Thrown::from_cause(
            "RuntimeError",
            with(
                "canonical shared production resolution invalid: ",
                &error.raised.message,
                "",
            ),
            &error,
        )),
        Err(error) => Err(error),
    }
}

/// `if not text.strip() or not re.search(r"^FROM\s+\S+", ...)` and the `FROM` refs of a canonical template.
fn read_template(world: &mut World<'_>, path: &PyStr, label: &str, keep: &mut Keep) -> Done<()> {
    let body = match read_file(world, path, Errors::Strict) {
        Ok(body) => body,
        Err(error) if template_error(&error.raised.class) => {
            return Err(Thrown::from_cause(
                "RuntimeError",
                with(
                    &format!("canonical shared template invalid ({label}): "),
                    &error.raised.message,
                    "",
                ),
                &error,
            ))
        }
        Err(error) => return Err(error),
    };
    let refs = from_refs(&body);
    if body.strip().is_empty() || refs.is_empty() {
        return Err(from_raised(Raised::plain(
            "RuntimeError",
            &format!("canonical shared template invalid ({label}): missing FROM instruction"),
        )));
    }
    for reference in refs {
        add_from(keep, &reference, &format!("FROM {label}"));
    }
    Ok(())
}

fn add_from(keep: &mut Keep, reference: &PyStr, reason: &str) {
    let points = reference.code_points();
    let placeholder = points.windows(2).any(|pair| pair == [0x7B, 0x7B]);
    if points.first() != Some(&u32::from('$')) && !placeholder {
        keep.add(reference, &text(reason));
    }
}

/// `preserve_set(canonical_only)`.
pub fn preserve_set(world: &mut World<'_>, repo: &PyValue, canonical_only: &PyValue) -> Done<Keep> {
    let canonical_only = canonical_only.is_truthy();
    let mut keep = Keep::default();
    let topologies: &[&str] = if canonical_only {
        &[]
    } else {
        &["multi", "single"]
    };
    // ① the IMAGE_TAG lines of the delivered env files
    for topo in topologies {
        let envdir = join_repo(repo, &["output", topo, "envs"])?;
        if exists_as(world, "isdir", &envdir) {
            for name in list_dir(world, &envdir)? {
                if !name.code_points().starts_with(&[0x2E, 0x65, 0x6E, 0x76]) {
                    continue;
                }
                let path = join(&envdir, &name);
                let body = match read_file(world, &path, Errors::Replace) {
                    Ok(body) => body,
                    Err(error) if is_os_error(&error.raised.class) => continue,
                    Err(error) => return Err(error),
                };
                for value in image_tags(&body) {
                    let points = value.code_points();
                    let full = points.contains(&u32::from(':')) || points.contains(&u32::from('/'));
                    let reference = if full {
                        value.clone()
                    } else {
                        with("easy-vllm:", &value, "")
                    };
                    let reason = with(&format!("env {topo}/"), &name, "");
                    keep.add(&reference, &reason);
                }
            }
        }
    }
    // ② the image-like strings of the resolved.json files
    let resolved: Vec<PyStr> = if canonical_only {
        Vec::new()
    } else {
        vec![
            join_repo(repo, &["resolved.json"])?,
            join_repo(repo, &["output", "multi", "resolved.json"])?,
            join_repo(repo, &["output", "single", "resolved.json"])?,
        ]
    };
    for path in &resolved {
        if !exists_as(world, "isfile", path) {
            continue;
        }
        let blob = match read_file(world, path, Errors::Default).and_then(|body| load_json(&body)) {
            Ok(blob) => blob,
            Err(_) => continue,
        };
        let dumped = blob.dumps_ascii().map_err(dump_error)?;
        for reference in image_strings(&dumped) {
            keep.add(&reference, &text("resolved.json"));
        }
    }
    // ③ the mandatory canonical resolution and templates
    let templates = join_repo(
        repo,
        &[".claude", "skills", "upstream-version-watch", "templates"],
    )?;
    read_resolution(world, repo, &mut keep)?;
    for (name, label) in [
        ("Dockerfile.template", "shared/Dockerfile.template"),
        (
            "Dockerfile.source-build.template",
            "shared/Dockerfile.source-build.template",
        ),
    ] {
        let path = join(&templates, &text(name));
        read_template(world, &path, label, &mut keep)?;
    }
    // the optional delivered Dockerfiles
    let mut dockerfiles: Vec<(PyStr, String)> = Vec::new();
    for topo in topologies {
        for name in ["Dockerfile", "Dockerfile.source-build"] {
            dockerfiles.push((
                join_repo(repo, &["output", topo, name])?,
                format!("{topo}/{name}"),
            ));
        }
    }
    for (path, label) in &dockerfiles {
        if !exists_as(world, "isfile", path) {
            continue;
        }
        let body = read_file(world, path, Errors::Replace)?;
        for reference in from_refs(&body) {
            add_from(&mut keep, &reference, &format!("FROM {label}"));
        }
    }
    // ④ the images of the containers that exist
    let (code, out) = run_text(
        world,
        &PyValue::List(
            ["docker", "ps", "-a", "--format", "{{.Image}}"]
                .iter()
                .map(|part| PyValue::str(part))
                .collect(),
        ),
        &PyValue::int(60),
    );
    if code == 0 {
        for line in splitlines(&out) {
            keep.add(&line, &text("container in use"));
        }
    }
    Ok(keep)
}

fn strings(parts: &[&str]) -> PyValue {
    PyValue::List(parts.iter().map(|part| PyValue::str(part)).collect())
}

/// `list_images()`.
pub fn list_images(world: &mut World<'_>) -> Done<Vec<PyValue>> {
    let (code, out) = run_text(
        world,
        &strings(&["docker", "image", "ls", "-a", "--format", "{{json .}}"]),
        &PyValue::int(60),
    );
    if code != 0 {
        let line = with("[cleanup] FAIL: docker image ls 실패 — ", &out.strip(), "");
        // `print(..., file=sys.stderr)`: the text, then its newline, as two writes
        world.console.err(&line)?;
        world.console.err(&PyStr::new("\n"))?;
        return Err(exit_with(2));
    }
    let mut images = Vec::new();
    for line in splitlines(&out) {
        if line.strip().is_empty() {
            continue;
        }
        images.push(load_json(&line)?);
    }
    Ok(images)
}

// ── the command line ─────────────────────────────────────────────────────────────────────────────

fn option(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    }
}

/// The program names the parsers were built for: `argparse` takes `os.path.basename(sys.argv[0])`, and a [`Spec`] holds a `'static` name,
/// so every distinct name is kept once (a process meets a handful).
fn intern(name: &str) -> &'static str {
    use std::sync::{Mutex, OnceLock};
    static NAMES: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
    let mut names = NAMES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(held) = names.iter().find(|held| **held == name) {
        return held;
    }
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(leaked);
    leaked
}

/// `os.path.basename(path)`: what follows the last `/`.
fn basename(path: &PyStr) -> String {
    let shown = path.to_utf8().unwrap_or_else(|| {
        String::from_utf8_lossy(&path.to_surrogateescape_bytes().unwrap_or_default()).into_owned()
    });
    shown.rsplit('/').next().unwrap_or("").to_string()
}

/// The parser of `main` for the program name `prog`.
pub fn spec(prog: &str) -> Spec {
    Spec {
        prog: intern(prog),
        description: "docker 찌꺼기 결정론 정리 (dry-run 기본)",
        options: vec![
            option(&["-h", "--help"], "help", Kind::Help, None),
            option(
                &["--apply"],
                "apply",
                Kind::Flag,
                Some("dry-run 표의 항목을 실제 삭제(사람 승인 후)"),
            ),
            option(
                &["--cache-age-hours"],
                "cache_age_hours",
                Kind::Int,
                Some("빌드캐시 prune 기준(기본 336h=14일 미접근 — 활성 트랙 ccache 보존)"),
            ),
            option(
                &["--keep"],
                "keep",
                Kind::TextAppend,
                Some("추가 보존 이미지 ref(반복 지정 가능)"),
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// `"%-Ns" % value`: padded on the right to `width` code points.
fn left(value: &PyStr, width: usize) -> PyStr {
    let mut out = value.clone();
    for _ in value.len()..width {
        out.push_str(" ");
    }
    out
}

fn lines_of(prefix: &str, out: &PyStr) -> PyStr {
    let mut joined = PyStr::default();
    for (index, line) in splitlines(out).iter().enumerate() {
        if index > 0 {
            joined.push_str("\n");
        }
        joined.push_str(prefix);
        joined.push_py(line);
    }
    joined
}

fn last_line(out: &PyStr) -> PyStr {
    let stripped = out.strip();
    if stripped.is_empty() {
        return PyStr::default();
    }
    splitlines(&stripped).pop().unwrap_or_default()
}

struct Candidate {
    reference: PyStr,
    size: PyValue,
    created: PyValue,
    target: PyValue,
    dangling: bool,
}

/// `main(canonical_only)` over `argv` (`sys.argv`, the program name first).
pub fn main(
    world: &mut World<'_>,
    repo: &PyValue,
    argv: &[PyStr],
    canonical_only: &PyValue,
    columns: Option<&str>,
) -> Done<()> {
    if argv.is_empty() {
        return Err(from_raised(Raised::plain(
            "IndexError",
            "list index out of range",
        )));
    }
    let mut tap = Tap {
        inner: &mut *world.console,
        message: None,
    };
    let parsed = parse_command(&mut tap, &spec(&basename(&argv[0])), &argv[1..], columns);
    let message = tap.message.take();
    // A parser output write that failed with an exception argparse does not swallow stopped the parse there: that exception is the
    // outcome, in place of the `SystemExit`. Raised inside the `except ArgumentError` handler it has that error as its implicit context.
    if let Some(mut failure) = tap.inner.take_parser_failure() {
        let context = match &parsed {
            Err(exit) => exit
                .context
                .clone()
                .or_else(|| argument_error(message.as_ref())),
            Ok(_) => None,
        };
        if let Some(context) = context {
            if failure.raised.cause.is_none() {
                failure.raised.cause = Some(Box::new(context));
                failure.raised.via_context = true;
            }
        }
        return Err(failure);
    }
    let values = match parsed {
        Ok(values) => values,
        Err(exit) => {
            let mut raised = Raised::new(
                "SystemExit",
                "builtins",
                PyStr::new(&exit.status.to_string()),
            );
            raised.code = Some(Box::new(PyValue::int(exit.status)));
            // `parse_known_args` turns an `ArgumentError` into `self.error(str(err))`: the exit is raised while it is handled
            if let Some(cause) = exit.context.or_else(|| argument_error(message.as_ref())) {
                raised.cause = Some(Box::new(cause));
                raised.via_context = true;
            }
            return Err(from_raised(raised));
        }
    };
    let apply = values.contains_key("apply");
    let hours = match values.get("cache_age_hours") {
        Some(Value::Int(number)) => number.to_string(),
        _ => "336".to_string(),
    };
    let extra: Vec<PyStr> = match values.get("keep") {
        Some(Value::Texts(list)) => list.clone(),
        _ => Vec::new(),
    };

    let mut keep = preserve_set(world, repo, canonical_only)?;
    for reference in &extra {
        keep.insert(reference.clone(), &text("--keep 수동"));
    }
    print(
        world,
        &text(&format!("[cleanup] 보존리스트({}):", keep.len())),
    )?;
    for reference in keep.sorted() {
        let mut line = PyStr::new("  KEEP  ");
        line.push_py(&left(&reference, 60));
        line.push_str(" (");
        line.push_py(&keep.reason_of(&reference));
        line.push_str(")");
        print(world, &line)?;
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    for image in list_images(world)? {
        let repository = get_or(&image, "Repository", PyValue::str("")).map_err(from_failure)?;
        let tag = get_or(&image, "Tag", PyValue::str("")).map_err(from_failure)?;
        let mut reference = repository.display_text();
        reference.push_str(":");
        reference.push_py(&tag.display_text());
        let is_none =
            |value: &PyValue| matches!(value.peel(), PyValue::Str(s) if s.equals("<none>"));
        let dangling = is_none(&get(&image, "Repository").map_err(from_failure)?)
            || is_none(&get(&image, "Tag").map_err(from_failure)?);
        let target = if dangling {
            get(&image, "ID").map_err(from_failure)?
        } else {
            PyValue::Str(reference.clone())
        };
        if !dangling && keep.contains(&reference) {
            continue;
        }
        candidates.push(Candidate {
            reference,
            size: get(&image, "Size").map_err(from_failure)?,
            created: get(&image, "CreatedSince").map_err(from_failure)?,
            target,
            dangling,
        });
    }

    print(
        world,
        &text(&format!(
            "\n[cleanup] 이미지 삭제 후보({}) — 보존리스트 밖:",
            candidates.len()
        )),
    )?;
    if candidates.is_empty() {
        print(world, &text("  (없음)"))?;
    }
    for candidate in &candidates {
        let mut line = PyStr::new("  RM    ");
        line.push_py(&left(&candidate.reference, 60));
        line.push_str(" ");
        line.push_py(&left(&candidate.size.display_text(), 10));
        line.push_str(" ");
        line.push_py(&candidate.created.display_text());
        if candidate.dangling {
            line.push_str("  [dangling]");
        }
        print(world, &line)?;
    }

    let df = strings(&["docker", "system", "df"]);
    let (_, out) = run_text(world, &df, &PyValue::int(60));
    print(
        world,
        &with("\n[cleanup] 현 사용량:\n", &lines_of("  ", &out), ""),
    )?;
    print(
        world,
        &text(&format!(
            "\n[cleanup] 빌드캐시: {hours}h 미접근분 prune 대상 (docker builder prune --filter unused-for={hours}h)"
        )),
    )?;

    if !apply {
        print(
            world,
            &text("\n[cleanup] DRY-RUN 종료 — 표 검토 후: bash .claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.sh --apply [--keep <ref>]"),
        )?;
        return Ok(());
    }

    let mut fails = 0;
    for candidate in &candidates {
        let command = PyValue::List(vec![
            PyValue::str("docker"),
            PyValue::str("rmi"),
            candidate.target.clone(),
        ]);
        let (code, out) = run_text(world, &command, &PyValue::int(120));
        if code != 0 {
            fails += 1;
        }
        let mut line = PyStr::new("[cleanup] rmi ");
        line.push_py(&left(&candidate.target.display_text(), 60));
        line.push_str(if code == 0 { " OK " } else { " FAIL " });
        line.push_py(&last_line(&out));
        print(world, &line)?;
    }
    let prune = PyValue::List(vec![
        PyValue::str("docker"),
        PyValue::str("builder"),
        PyValue::str("prune"),
        PyValue::str("-f"),
        PyValue::str("--filter"),
        PyValue::str(&format!("unused-for={hours}h")),
    ]);
    let (code, out) = run_text(world, &prune, &PyValue::int(600));
    print(
        world,
        &text(&format!(
            "[cleanup] builder prune: {}",
            if code == 0 { "OK" } else { "FAIL" }
        )),
    )?;
    let tail: Vec<PyStr> = splitlines(&out.strip());
    let from = tail.len().saturating_sub(3);
    for line in &tail[from..] {
        print(world, &with("  ", line, ""))?;
    }
    let (_, out) = run_text(world, &df, &PyValue::int(60));
    print(
        world,
        &with("[cleanup] 정리 후 사용량:\n", &lines_of("  ", &out), ""),
    )?;
    if fails > 0 {
        print(
            world,
            &text(&format!(
                "[cleanup] ⚠ rmi 실패 {fails}건(자식 레이어 참조 등) — 표 재검토"
            )),
        )?;
    }
    Ok(())
}

/// The console the parser writes to, remembering the message of a usage error.
struct Tap<'a> {
    inner: &'a mut dyn CleanupConsole,
    message: Option<PyStr>,
}

impl Console for Tap<'_> {
    fn print_stderr(&mut self, text: &PyStr) -> crate::pyexpr::Res<()> {
        self.inner.print_stderr(text)
    }

    fn write_stdout(&mut self, text: &PyStr) {
        self.inner.write_stdout(text);
    }

    fn print_stdout(&mut self, text: &PyStr) {
        self.inner.print_stdout(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        self.message = Some(message.clone());
        self.inner.parser_error(usage, message);
    }

    fn parser_help(&mut self, help: &PyStr) {
        self.inner.parser_help(help);
    }

    fn write_uncaught(&mut self, text: &PyStr) {
        self.inner.write_uncaught(text);
    }

    fn observe(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.inner.observe(name, fields);
    }

    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        self.inner.read_stdin()
    }
}

/// The `ArgumentError` behind a usage error whose message names an argument (`cleanup_docker.py: error: argument --keep: ...`); the
/// errors `parse_args` itself reports (`unrecognized arguments`) are not raised from one.
fn argument_error(message: Option<&PyStr>) -> Option<Raised> {
    let shown = message?.to_utf8()?;
    let detail = shown.trim_end_matches('\n').rsplit_once(": error: ")?.1;
    detail
        .starts_with("argument ")
        .then(|| Raised::new("ArgumentError", "argparse", PyStr::new(detail)))
}

/// The typed values of the wire (`W/ABI.md`).
pub mod dto {
    use super::*;

    fn kind(name: &str) -> PyDict {
        let mut map = PyDict::new();
        map.set("kind", PyValue::str(name));
        map
    }

    pub fn run_tuple(code: i64, out: &PyStr) -> PyValue {
        let mut map = kind("run_tuple");
        map.set(
            "items",
            PyValue::List(vec![PyValue::int(code), PyValue::Str(out.clone())]),
        );
        PyValue::Dict(map)
    }

    pub fn preserve_tuple(keep: &Keep) -> PyValue {
        let mut map = kind("preserve_tuple");
        map.set(
            "keep",
            PyValue::List(keep.sorted().into_iter().map(PyValue::Str).collect()),
        );
        let mut why = PyDict::new();
        for (reference, reason) in &keep.why {
            why.insert(reference.clone(), PyValue::Str(reason.clone()));
        }
        map.set("why", PyValue::Dict(why));
        PyValue::Dict(map)
    }

    pub fn json(data: Vec<PyValue>) -> PyValue {
        let mut map = kind("json");
        map.set("data", PyValue::List(data));
        PyValue::Dict(map)
    }

    pub fn none() -> PyValue {
        PyValue::Dict(kind("none"))
    }
}
