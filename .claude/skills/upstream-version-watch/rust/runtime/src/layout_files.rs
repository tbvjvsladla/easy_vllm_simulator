//! The three file-reading functions of `check_spec_layout.py`: `read_method_from_config`, `load_index_keys` and `read_num_nextn`, with the file
//! steps told as the original collector sees them (`isfile`, `file_open`, `file_read`, `file_iter_line`, `file_close`, ...).
//!
//! Two reading styles are kept apart because their reading and decoding differ: the config is read by LINE ITERATION (`for line in f`: the open file
//! is read 8192 bytes at a time, only as the next line is needed, and decoded as it comes, so where a decoding error surfaces, and which bytes it
//! names, is the chunk boundary's, the first match returns without waiting for the end of the file and a later bad byte is never read), the JSON
//! files by `json.load(f)` (`f.read()`: the whole rest of the file is read and decoded at once, the error names the whole file; the read is a step
//! AFTER the open).
//!
//! Where an exception can escape the command uncaught, the failing function stamps the frames the interpreter's traceback would show (see
//! [`crate::layout_traceback`]); stamping changes nothing else and an exception that is caught never leaves a stamp.

use crate::host::Sink;
use crate::layout_rules::{config_line_value, search_quoted_method, text_of, unquote_value};
use crate::layout_sites::SiteId;
use crate::textfile::{StreamFailure, TextStream};
use crate::thrown::{encode_thrown, Ctor, Done, Thrown};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::as_get_target;
use agent_runtime_support::pyjson::{
    decode_file_text, loads_text, utf8_error_parts, utf8_error_text, LoadError,
};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;
use recipe_runtime::model_files::{fspath, is_dir, is_file, join, os_failure};
use std::io::Read;

/// What a call runs on: the console and trace sink, `sys.argv` (the program name first) and the terminal width argparse formats for.
pub struct LayoutWorld<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
    /// The frames an uncaught exception would show, innermost first: stamped where it is raised and by each caller that lets it through.
    pub frames: Vec<SiteId>,
}

impl<'a> LayoutWorld<'a> {
    pub fn new(
        host: &'a mut dyn Sink,
        argv: &'a [PyStr],
        columns: Option<&'a str>,
    ) -> LayoutWorld<'a> {
        LayoutWorld {
            host,
            argv,
            columns,
            frames: Vec::new(),
        }
    }

    /// The file name the script's own frames show: the caller's `argv[0]` when it names the script, else the canonical logical name.
    pub fn script_file(&self) -> PyStr {
        match self.argv.first() {
            Some(program) if crate::main_cmd::basename(program) == crate::layout_main::PROGRAM => {
                program.clone()
            }
            _ => PyStr::new(crate::layout_main::PROGRAM),
        }
    }

    /// Lets a callee's outcome through; when it is an exception that stamped frames, the call site is the next frame outwards.
    pub fn through(&mut self, call_site: SiteId, outcome: Done<PyValue>) -> Done<PyValue> {
        if outcome.is_err() && !self.frames.is_empty() {
            self.frames.push(call_site);
        }
        outcome
    }

    fn stamp(&mut self, frames: &[SiteId]) {
        self.frames = frames.to_vec();
    }
}

impl LayoutWorld<'_> {
    fn event(&mut self, name: &str, path: &PyStr, extra: Vec<(&str, PyValue)>) {
        if self.host.tracing() {
            let mut fields = vec![("path", PyValue::Str(path.clone()))];
            fields.extend(extra);
            self.host.trace(name, fields);
        }
    }

    /// [`Self::event`] for fields that are costly to build (they copy the document text): `extra` runs only when the host traces. The host is asked
    /// once, as `event` asks it, and is told the same name and the same fields in the same order.
    fn event_with(
        &mut self,
        name: &str,
        path: &PyStr,
        extra: impl FnOnce() -> Vec<(&'static str, PyValue)>,
    ) {
        if self.host.tracing() {
            let mut fields = vec![("path", PyValue::Str(path.clone()))];
            fields.extend(extra());
            self.host.trace(name, fields);
        }
    }
}

const INDEX_FILES: [&str; 2] = [
    "model.safetensors.index.json",
    "pytorch_model.bin.index.json",
];
const NO_INDEX: &str = "index.json 부재(단일 샤드 체크포인트일 수 있음 — 검사 생략)";

/// `os.path.isfile(path)`, told to the trace.
pub fn isfile(world: &mut LayoutWorld<'_>, path: &PyStr) -> bool {
    let found = is_file(path);
    world.event("isfile", path, vec![("result", PyValue::Bool(found))]);
    found
}

/// `os.path.isdir(path)`, told to the trace.
pub fn isdir(world: &mut LayoutWorld<'_>, path: &PyStr) -> bool {
    let found = is_dir(path);
    world.event("isdir", path, vec![("result", PyValue::Bool(found))]);
    found
}

fn failure_thrown(failure: Failure) -> Box<Thrown> {
    Thrown::plain(Raised::from(failure))
}

/// A file that was opened: its path as `open()` took it and the open file, which is read only when a step asks for bytes.
struct Opened {
    name: PyStr,
    file: std::fs::File,
}

/// Why an `open()` failed, with `repr(error)` for an operating-system error (what `{e!r}` prints).
struct OpenFailure {
    thrown: Box<Thrown>,
    os_repr: Option<String>,
}

/// `repr(OSError)` of an operating-system failure: the class, the errno and the message of the system (no file name: it is not an argument).
fn os_repr(class: &str, error: &std::io::Error) -> Option<String> {
    let detail = error.to_string();
    let strerror = detail.split(" (os error").next().unwrap_or(&detail);
    error
        .raw_os_error()
        .map(|errno| format!("{class}({errno}, {})", PyStr::new(strerror).repr()))
}

fn os_open_failure(error: &std::io::Error, path: &PyStr) -> OpenFailure {
    let failure = os_failure(error, Some(path));
    OpenFailure {
        os_repr: os_repr(failure.class, error),
        thrown: failure_thrown(failure),
    }
}

/// `open(path, encoding="utf-8")`: the file is opened and nothing is read. The steps are told as the collector sees them (`file_open`, and
/// `file_open_error` when it fails); a path that is not text is a `TypeError` before anything is told.
fn open_for_text(world: &mut LayoutWorld<'_>, path: &PyValue) -> Result<Opened, OpenFailure> {
    let name = fspath(path, false)
        .map_err(|failure| OpenFailure {
            thrown: failure_thrown(failure),
            os_repr: None,
        })?
        .clone();
    let mut keywords = PyDict::new();
    keywords.set("encoding", PyValue::str("utf-8"));
    world.event(
        "file_open",
        &name,
        vec![
            ("args", PyValue::List(Vec::new())),
            ("kwargs", PyValue::Dict(keywords)),
        ],
    );
    let opened = open_file(&name);
    if let Err(failure) = &opened {
        world.event(
            "file_open_error",
            &name,
            vec![
                ("type", PyValue::str(&failure.thrown.raised.class)),
                (
                    "message",
                    PyValue::Str(failure.thrown.raised.message.clone()),
                ),
            ],
        );
    }
    opened.map(|file| Opened { name, file })
}

fn open_file(name: &PyStr) -> Result<std::fs::File, OpenFailure> {
    use std::os::unix::ffi::OsStrExt;
    let encoded = name
        .to_surrogateescape_bytes()
        .map_err(|failure| OpenFailure {
            thrown: encode_thrown(name, &failure),
            os_repr: None,
        })?;
    if encoded.contains(&0) {
        return Err(OpenFailure {
            thrown: Thrown::builtin("ValueError", "embedded null byte"),
            os_repr: None,
        });
    }
    let file = std::fs::File::open(std::ffi::OsStr::from_bytes(&encoded))
        .map_err(|error| os_open_failure(&error, name))?;
    // the system call opens a directory; the file object's own check of the descriptor is what refuses it, still at `open()`
    if file.metadata().is_ok_and(|metadata| metadata.is_dir()) {
        return Err(os_open_failure(
            &std::io::Error::from_raw_os_error(21),
            name,
        ));
    }
    Ok(file)
}

/// The decoding error of a chunk or of a whole file: `UnicodeDecodeError` with the five constructor fields of the failure itself.
fn decode_thrown(
    message: &str,
    object: Vec<u8>,
    start: usize,
    end: usize,
    reason: &str,
) -> Box<Thrown> {
    Box::new(Thrown {
        raised: Raised::plain("UnicodeDecodeError", message),
        ctors: vec![Some(Ctor::Decode {
            encoding: "utf-8".to_string(),
            object,
            start,
            end,
            reason: reason.to_string(),
        })],
        bases: None,
    })
}

/// A failure of the common JSON reader as the Python exception it stands for: `JSONDecodeError` with its constructor fields, or the
/// `ValueError` / `RecursionError` the decoder lets through.
fn json_thrown(error: LoadError, document: &PyStr) -> Box<Thrown> {
    match error {
        LoadError::Decode { message, position, .. } => Box::new(Thrown {
            raised: Raised::new("JSONDecodeError", "json.decoder", PyStr::new(&error.to_string())),
            ctors: vec![Some(Ctor::Json {
                msg: message.to_string(),
                doc: document.clone(),
                pos: position,
            })],
            bases: None,
        }),
        LoadError::IntegerTooLong { .. } => Thrown::builtin("ValueError", &error.to_string()),
        LoadError::TooDeep { container } => Thrown::builtin(
            "RecursionError",
            &format!(
                "maximum recursion depth exceeded while decoding a JSON {container} from a unicode string"
            ),
        ),
        LoadError::Encoding(message) => Thrown::builtin("UnicodeDecodeError", &message),
    }
}

fn is_json_decode_error(thrown: &Thrown) -> bool {
    thrown.raised.class == "JSONDecodeError" && thrown.raised.module == "json.decoder"
}

/// `repr(JSONDecodeError)`: the class and the repr of the message (the exception's arguments are its formatted message).
fn json_error_repr(thrown: &Thrown) -> String {
    format!("JSONDecodeError({})", thrown.raised.message.repr())
}

/// Where in `json.load(f).get(key)` a step failed: the read (an operating-system error or a decoding error), the parse, or the `.get`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    ReadSystem,
    ReadDecode,
    Parse,
    Get,
}

/// A failure of `json.load(f).get(key)` with `repr(error)` for an operating-system error.
struct JsonFailure {
    thrown: Box<Thrown>,
    os_repr: Option<String>,
    step: Step,
}

/// `json.load(f)`: the `file_read` step, then `f.read()` reads the rest of the file and decodes it at once (the error names the whole file),
/// then `json.loads` of the text.
fn json_load(world: &mut LayoutWorld<'_>, file: &mut Opened) -> Result<PyValue, JsonFailure> {
    world.event(
        "file_read",
        &file.name,
        vec![
            ("args", PyValue::List(Vec::new())),
            ("kwargs", PyValue::Dict(PyDict::new())),
        ],
    );
    let mut bytes = Vec::new();
    if let Err(error) = file.file.read_to_end(&mut bytes) {
        let failure = os_failure(&error, None);
        world.event(
            "file_read_error",
            &file.name,
            vec![
                ("type", PyValue::str(failure.class)),
                ("message", PyValue::Str(failure.message.clone())),
            ],
        );
        return Err(JsonFailure {
            os_repr: os_repr(failure.class, &error),
            thrown: failure_thrown(failure),
            step: Step::ReadSystem,
        });
    }
    // The whole file is validated and decoded ONCE (`open(..., encoding="utf-8").read()`: strict UTF-8, universal newlines). Only a rejected file is
    // inspected again, to derive the exact `UnicodeDecodeError` constructor fields and the `file_read_error` event of the original.
    let text = match decode_file_text(&bytes) {
        Ok(text) => text,
        Err(LoadError::Encoding(reported)) => {
            let Err(error) = std::str::from_utf8(&bytes) else {
                // `decode_file_text` reports an encoding error only for bytes `from_utf8` rejects; if that ever changed, keep the common message
                return Err(JsonFailure {
                    thrown: Thrown::builtin("UnicodeDecodeError", &reported),
                    os_repr: None,
                    step: Step::ReadDecode,
                });
            };
            let message = utf8_error_text(&bytes, &error);
            let (start, length, reason) = utf8_error_parts(&bytes, &error);
            world.event(
                "file_read_error",
                &file.name,
                vec![
                    ("type", PyValue::str("UnicodeDecodeError")),
                    ("message", PyValue::str(&message)),
                ],
            );
            return Err(JsonFailure {
                thrown: decode_thrown(&message, bytes, start, start + length, reason),
                os_repr: None,
                step: Step::ReadDecode,
            });
        }
        Err(other) => {
            return Err(JsonFailure {
                thrown: Thrown::builtin("UnicodeDecodeError", &other.to_string()),
                os_repr: None,
                step: Step::ReadDecode,
            })
        }
    };
    world.event_with("file_read_return", &file.name, || {
        vec![("text", PyValue::Str(text.clone()))]
    });
    loads_text(&text).map_err(|error| JsonFailure {
        thrown: json_thrown(error, &text),
        os_repr: None,
        step: Step::Parse,
    })
}

/// `json.load(f).get(key)` of the opened file, then the `with` statement closes it.
fn json_member(
    world: &mut LayoutWorld<'_>,
    file: &mut Opened,
    key: &str,
) -> Result<PyValue, JsonFailure> {
    let outcome = json_load(world, file).and_then(|mut document| {
        if let Err(failure) = as_get_target(&document) {
            return Err(JsonFailure {
                thrown: failure_thrown(failure),
                os_repr: None,
                step: Step::Get,
            });
        }
        // the document is ours and is dropped right after: the member is moved out of it, not cloned (a missing key is `None`)
        Ok(match &mut document {
            PyValue::Dict(map) => map
                .get_mut(key)
                .map_or(PyValue::None, |slot| std::mem::replace(slot, PyValue::None)),
            _ => PyValue::None,
        })
    });
    world.event("file_close", &file.name, Vec::new());
    outcome
}

/// `(json.loads(raw) or {}).get("method")` or, when `raw` is not JSON, the first `"method": "..."` of the source pattern. The `bool` says whether
/// the failure is the `.get` of the source's attribute access (a stamped site) rather than the parse.
fn method_of(raw: &[u32]) -> Result<PyValue, (Box<Thrown>, bool)> {
    let text = text_of(raw);
    match loads_text(&text) {
        Ok(value) => {
            let target = if value.is_truthy() {
                value
            } else {
                PyValue::Dict(PyDict::new())
            };
            as_get_target(&target)
                .map(|map| map.get("method").cloned().unwrap_or(PyValue::None))
                .map_err(|failure| (failure_thrown(failure), true))
        }
        Err(LoadError::Decode { .. }) => {
            Ok(search_quoted_method(raw)
                .map_or(PyValue::None, |group| PyValue::Str(text_of(&group))))
        }
        Err(other) => Err((json_thrown(other, &text), false)),
    }
}

/// Where `first_method` failed: a decoding error of the iteration, the `.get` of the method, or anything else (not stamped).
enum Fault {
    Iteration,
    MethodGet,
    Other,
}

/// `for line in f`: the open file is read as the decoder needs bytes and the lines are told one by one; the first line that is the speculative config
/// decides and ends the loop (the file is closed without the rest of it ever being read or decoded).
fn first_method(
    world: &mut LayoutWorld<'_>,
    file: &mut Opened,
) -> Result<PyValue, (Box<Thrown>, Fault)> {
    let mut stream = TextStream::from_reader(&mut file.file);
    loop {
        match stream.next_line_checked() {
            Ok(Some(line)) => {
                world.event(
                    "file_iter_line",
                    &file.name,
                    vec![("text", PyValue::str(&line))],
                );
                let points: Vec<u32> = line.chars().map(u32::from).collect();
                if let Some(group) = config_line_value(&points) {
                    return method_of(&unquote_value(&group)).map_err(|(thrown, get)| {
                        (thrown, if get { Fault::MethodGet } else { Fault::Other })
                    });
                }
            }
            Ok(None) => {
                world.event("file_iter_end", &file.name, Vec::new());
                return Ok(PyValue::None);
            }
            Err(StreamFailure::Decode(failure)) => {
                let message = failure.message();
                world.event(
                    "file_iter_error",
                    &file.name,
                    vec![
                        ("type", PyValue::str("UnicodeDecodeError")),
                        ("message", PyValue::str(&message)),
                    ],
                );
                return Err((
                    decode_thrown(
                        &message,
                        failure.object,
                        failure.start,
                        failure.end,
                        failure.reason,
                    ),
                    Fault::Iteration,
                ));
            }
            Err(StreamFailure::Read(error)) => {
                let failure = os_failure(&error, None);
                world.event(
                    "file_iter_error",
                    &file.name,
                    vec![
                        ("type", PyValue::str(failure.class)),
                        ("message", PyValue::Str(failure.message.clone())),
                    ],
                );
                return Err((failure_thrown(failure), Fault::Other));
            }
        }
    }
}

/// `read_method_from_config(config_yaml)`: the `method` of the first `speculative-config:` line, `None` when the file cannot be opened or read
/// (an `OSError`), has no such line, or its value holds no method; a malformed value is searched by the source pattern.
pub fn read_method_from_config(
    world: &mut LayoutWorld<'_>,
    config_yaml: &PyValue,
) -> Done<PyValue> {
    let mut file = match open_for_text(world, config_yaml) {
        Ok(file) => file,
        Err(failure) if failure.thrown.is_a("OSError") => return Ok(PyValue::None),
        Err(failure) => return Err(failure.thrown),
    };
    let outcome = first_method(world, &mut file);
    world.event("file_close", &file.name, Vec::new());
    match outcome {
        Ok(method) => Ok(method),
        Err((thrown, _)) if thrown.is_a("OSError") => Ok(PyValue::None),
        Err((thrown, fault)) => {
            match fault {
                Fault::Iteration => {
                    world.stamp(&[SiteId::CodecsDecode, SiteId::ConfigIteration]);
                }
                Fault::MethodGet => world.stamp(&[SiteId::MethodGet]),
                Fault::Other => {}
            }
            Err(thrown)
        }
    }
}

/// What the `try` of `load_index_keys` can end with: the `weight_map`, or the `repr` of an `OSError` / `JSONDecodeError` it catches, or an
/// exception it does not.
enum Index {
    Map(PyValue),
    Caught(String),
    Raised(Box<Thrown>, Option<Step>),
}

fn read_index(world: &mut LayoutWorld<'_>, path: &PyStr) -> Index {
    let mut file = match open_for_text(world, &PyValue::Str(path.clone())) {
        Ok(file) => file,
        Err(failure) => {
            return match failure.os_repr {
                Some(repr) if failure.thrown.is_a("OSError") => Index::Caught(repr),
                _ => Index::Raised(failure.thrown, None),
            }
        }
    };
    match json_member(world, &mut file, "weight_map") {
        Ok(map) => Index::Map(map),
        Err(failure) => match failure.os_repr {
            Some(repr) if failure.thrown.is_a("OSError") => Index::Caught(repr),
            _ if is_json_decode_error(&failure.thrown) => {
                Index::Caught(json_error_repr(&failure.thrown))
            }
            _ => Index::Raised(failure.thrown, Some(failure.step)),
        },
    }
}

/// `load_index_keys(model_dir)`: `(keys, file name)` of the first index file that exists, or `(None, reason)` when none does or the first that does
/// cannot be read as JSON (the second is then NOT tried).
pub fn load_index_keys(world: &mut LayoutWorld<'_>, model_dir: &PyValue) -> Done<PyValue> {
    let directory = fspath(model_dir, false).map_err(failure_thrown)?.clone();
    for name in INDEX_FILES {
        let path = join(&directory, &PyStr::new(name));
        if !isfile(world, &path) {
            continue;
        }
        let weight_map = match read_index(world, &path) {
            Index::Map(map) => map,
            Index::Caught(repr) => {
                return Ok(PyValue::Tuple(vec![
                    PyValue::None,
                    PyValue::str(&format!("{name} 파싱 실패: {repr}")),
                ]))
            }
            Index::Raised(thrown, step) => {
                match step {
                    Some(Step::ReadDecode) => world.stamp(&[
                        SiteId::CodecsDecode,
                        SiteId::JsonLoadRead,
                        SiteId::IndexJsonLoad,
                    ]),
                    Some(Step::Get) => world.stamp(&[SiteId::IndexGet]),
                    _ => {}
                }
                return Err(thrown);
            }
        };
        // `... or {}`, then `list(wm.keys())`
        let keys = if weight_map.is_truthy() {
            match weight_map {
                // An owned dict whose keys are all strings gives its keys up by move (the values drop); any other dict is
                // refused by the accessor, comes back unchanged and takes the borrowed clone path below.
                PyValue::Dict(map) => match map.try_into_string_entries() {
                    Ok(entries) => entries.map(|(key, _)| PyValue::Str(key)).collect(),
                    Err(map) => map
                        .iter()
                        .map(|(key, _)| PyValue::Str(key.clone()))
                        .collect(),
                },
                shared => match shared.peel() {
                    PyValue::Dict(map) => map
                        .iter()
                        .map(|(key, _)| PyValue::Str(key.clone()))
                        .collect(),
                    other => {
                        world.stamp(&[SiteId::IndexKeys]);
                        return Err(Thrown::builtin(
                            "AttributeError",
                            &format!("'{}' object has no attribute 'keys'", other.type_name()),
                        ));
                    }
                },
            }
        } else {
            Vec::new()
        };
        return Ok(PyValue::Tuple(vec![
            PyValue::List(keys),
            PyValue::str(name),
        ]));
    }
    Ok(PyValue::Tuple(vec![PyValue::None, PyValue::str(NO_INDEX)]))
}

/// `read_num_nextn(model_dir)`: the `num_nextn_predict_layers` of `config.json` as the document holds it (no coercion), `None` when the file
/// cannot be opened or is not JSON.
pub fn read_num_nextn(world: &mut LayoutWorld<'_>, model_dir: &PyValue) -> Done<PyValue> {
    let directory = fspath(model_dir, false).map_err(failure_thrown)?.clone();
    let path = join(&directory, &PyStr::new("config.json"));
    let mut file = match open_for_text(world, &PyValue::Str(path)) {
        Ok(file) => file,
        Err(failure) if failure.thrown.is_a("OSError") => return Ok(PyValue::None),
        Err(failure) => return Err(failure.thrown),
    };
    match json_member(world, &mut file, "num_nextn_predict_layers") {
        Ok(value) => Ok(value),
        Err(failure) if failure.thrown.is_a("OSError") || is_json_decode_error(&failure.thrown) => {
            Ok(PyValue::None)
        }
        Err(failure) => {
            match failure.step {
                Step::ReadDecode => world.stamp(&[
                    SiteId::CodecsDecode,
                    SiteId::JsonLoadRead,
                    SiteId::NextnJsonLoad,
                ]),
                Step::Get => world.stamp(&[SiteId::NextnGet]),
                _ => {}
            }
            Err(failure.thrown)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A host that counts how often it is asked whether it traces and keeps what it is told.
    struct Counting {
        traces: bool,
        asked: Cell<usize>,
        told: Vec<(String, Vec<String>)>,
    }

    impl Sink for Counting {
        fn out(&mut self, _: &PyStr) -> Done<()> {
            Ok(())
        }
        fn err(&mut self, _: &PyStr) -> Done<()> {
            Ok(())
        }
        fn parser_help(&mut self, _: &PyStr) {}
        fn parser_error(&mut self, _: &PyStr, _: &PyStr) {}
        fn tracing(&self) -> bool {
            self.asked.set(self.asked.get() + 1);
            self.traces
        }
        fn trace(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
            self.told.push((
                name.to_string(),
                fields.iter().map(|(key, _)| key.to_string()).collect(),
            ));
        }
    }

    fn run(traces: bool) -> (usize, usize, Vec<(String, Vec<String>)>) {
        let mut host = Counting {
            traces,
            asked: Cell::new(0),
            told: Vec::new(),
        };
        let built = Cell::new(0usize);
        {
            let argv = [PyStr::new("check_spec_layout.py")];
            let mut world = LayoutWorld::new(&mut host, &argv, None);
            let path = PyStr::new("p");
            world.event_with("lazy", &path, || {
                built.set(built.get() + 1);
                vec![("text", PyValue::str("t"))]
            });
            world.event("eager", &path, vec![("n", PyValue::int(1))]);
        }
        (built.get(), host.asked.get(), host.told)
    }

    #[test]
    fn the_lazy_fields_are_built_only_for_a_host_that_traces_and_the_host_is_asked_once_per_event()
    {
        let (built, asked, told) = run(false);
        assert_eq!(
            (built, asked),
            (0, 2),
            "a host that does not trace never has the fields built"
        );
        assert!(told.is_empty());
        let (built, asked, told) = run(true);
        assert_eq!((built, asked), (1, 2), "built once, one question per event");
        assert_eq!(
            told,
            vec![
                (
                    "lazy".to_string(),
                    vec!["path".to_string(), "text".to_string()]
                ),
                (
                    "eager".to_string(),
                    vec!["path".to_string(), "n".to_string()]
                ),
            ]
        );
    }
}
