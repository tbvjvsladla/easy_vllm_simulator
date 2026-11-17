//! `validate_runtime_patch.py`: bind an ephemeral runtime patch (`configs/<model>_patch.py`) to the current model and topology inputs.
//! `stamp` writes `configs/<model>_patch.provenance.json`, `verify` compares it with what the inputs bind now.
//!
//! The record binds the patch (the *resolved* file), the canonical production resolution, the model stem, the topology and the SHA-256 of every
//! model input that exists (`<stem>.sh`, `<stem>.yaml`, `envs/.env.<stem>`). The business of the original is kept in its order:
//! 1. `_inside`: the patch and the configuration directory are resolved (`Path.resolve`, non-strict) and the patch must lie under the directory;
//! 2. the patch must be an existing `*_patch.py` file; its stem (taken from the *resolved* name) must use the ASCII allow-list;
//! 3. the model inputs are looked up through the **lexical** configuration argument (no resolution), at least one of `.sh`/`.yaml` must exist;
//! 4. the resolution must exist, decode as UTF-8 with universal newlines and be a non-empty JSON object (it is read again for its hash);
//! 5. the patch and resolution hashes are taken, then the record is compared (`verify`) or written (`stamp`).
//!
//! `stamp` writes the sidecar through an exclusively created temporary file in the sidecar's directory (mode `0600`, a random `prefix.xxxxxxxx`
//! name) and `rename`; the temporary file is removed whenever it still exists. The record is rendered with sorted keys, indent 2,
//! `ensure_ascii=False` and a final newline.
//!
//! Exit status: 0; 2 (every `OSError`, `ValueError`, `KeyError`, `TypeError`, `JSONDecodeError`, with `runtime-patch-provenance FAIL: <message>`
//! on standard error; argparse); 1 (the uncaught `RuntimeError` of a symlink loop). Outcomes outside the modelled domain are named
//! ([`Refusal`]) and end with status 70, never disguised as one of the above: a document nested beyond the C recursion limit, a link chain deeper than
//! [`crate::runtime_patch_path::MAX_LINK_DEPTH`], a `PYTHONIOENCODING` other than unset or `ascii:strict`, a non-ASCII text written to a strict ASCII stream
//! where the original raises outside its handler.

use crate::host::Sink;
use crate::runtime_patch_args::{parse_arguments, Arguments, Exit};
use crate::runtime_patch_path::{
    abspath, child_text, exists, is_file, is_relative_to, make_directories, name_of, parent_text,
    path_text, read_bytes, resolve, OsFault, PathBytes, PathFailure,
};
use crate::runtime_patch_traceback::{symlink_loop_report, Action, ResolveCall};
use crate::thrown::{Ctor, Done, Thrown};
use crate::torch_main::Refusal;
use agent_runtime_support::pydyn::equals;
use agent_runtime_support::pyjson::{decode_file_text, loads_text, LoadError};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;
use ring::digest::{digest, SHA256};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;

pub const PROGRAM: &str = "validate_runtime_patch.sh";
pub const SCHEMA_VERSION: i64 = 1;
const PATCH_SUFFIX: &[u8] = b"_patch.py";
const STEM_ALLOWED: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-";
/// `tempfile._RandomNameSequence.characters` and its name length.
const NAME_CHARACTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
const NAME_LENGTH: usize = 8;
/// `tempfile.TMP_MAX`.
const TEMPORARY_ATTEMPTS: usize = 10_000;
const EEXIST: i32 = 17;

/// How the process's `sys.stdout` encodes: the interpreter's default for the saved profiles, or the explicit `PYTHONIOENCODING=ascii:strict`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdioProfile {
    Default,
    AsciiStrict,
}

impl StdioProfile {
    /// The profile `PYTHONIOENCODING` selects; any other value is not modelled.
    pub fn from_setting(value: Option<&[u8]>) -> Result<StdioProfile, Refusal> {
        match value {
            None | Some(b"") => Ok(StdioProfile::Default),
            Some(b"ascii:strict") => Ok(StdioProfile::AsciiStrict),
            Some(other) => Err(Refusal {
                boundary: "PYTHONIOENCODING",
                detail: format!(
                    "{:?} is neither unset nor \"ascii:strict\"; the encoding of the interpreter's streams under it is not reproduced",
                    String::from_utf8_lossy(other)
                ),
            }),
        }
    }
}

/// What a call runs on: the console, `sys.argv`, the terminal width and the stream profile.
pub struct World<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
    pub profile: StdioProfile,
}

/// How `main` stopped before returning 0.
#[derive(Debug)]
pub enum Stop {
    /// `SystemExit(code)` (argparse, or the `parser.exit(2, ...)` of the handler).
    Exit(i32),
    /// An exception nothing handles, as its full report.
    Uncaught(PyStr),
    Refused(Refusal),
}

/// An exception inside the `try` of `main`.
enum Failure {
    /// One the `except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError)` clause catches: its `str`.
    Caught(PyStr),
    /// `Path.resolve` raised `RuntimeError("Symlink loop ...")`.
    Loop {
        filename: PathBytes,
        call: ResolveCall,
    },
    Refused(Refusal),
    /// An exception of a stream write the handler does not catch.
    Thrown(Box<Thrown>),
}

impl From<OsFault> for Failure {
    fn from(fault: OsFault) -> Failure {
        Failure::Caught(fault.message())
    }
}

fn refusal(boundary: &'static str, detail: impl Into<String>) -> Failure {
    Failure::Refused(Refusal {
        boundary,
        detail: detail.into(),
    })
}

fn caught(message: &str) -> Failure {
    Failure::Caught(PyStr::new(message))
}

fn failure_of(failure: PathFailure, call: ResolveCall) -> Failure {
    match failure {
        PathFailure::Os(fault) => fault.into(),
        PathFailure::SymlinkLoop { filename } => Failure::Loop { filename, call },
        PathFailure::TooDeep => refusal(
            "realpath_depth",
            "a chain of symbolic links nested deeper than the port follows; the interpreter's own bound is its stack depth",
        ),
    }
}

fn bytes_of(text: &PyStr) -> Result<PathBytes, Failure> {
    text.to_surrogateescape_bytes().map_err(|_| {
        refusal(
            "argv",
            "an argument holding a surrogate that is not an escaped byte",
        )
    })
}

/// The classes `except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError)` catches, among the exceptions a stream write raises.
fn is_handled(thrown: &Thrown) -> bool {
    thrown.is_a("OSError")
        || thrown.is_a("ValueError")
        || matches!(
            thrown.raised.class.as_str(),
            "UnicodeEncodeError"
                | "UnicodeDecodeError"
                | "JSONDecodeError"
                | "KeyError"
                | "TypeError"
        )
}

/// A character in the `\xNN`/`\uNNNN`/`\UNNNNNNNN` form `UnicodeEncodeError` prints.
fn escaped(point: u32) -> String {
    match point {
        0..=0xFF => format!("\\x{point:02x}"),
        0x100..=0xFFFF => format!("\\u{point:04x}"),
        _ => format!("\\U{point:08x}"),
    }
}

/// The `UnicodeEncodeError` of writing `text` to a strict ASCII stream: the first run of characters above 127, as the encoder reports it.
fn ascii_encode_error(text: &PyStr) -> Option<Box<Thrown>> {
    let points = text.code_points();
    let start = points.iter().position(|point| *point > 127)?;
    let end = start
        + points[start..]
            .iter()
            .take_while(|point| **point > 127)
            .count();
    let subject = if end == start + 1 {
        format!("character '{}' in position {start}", escaped(points[start]))
    } else {
        format!("characters in position {start}-{}", end - 1)
    };
    let reason = "ordinal not in range(128)";
    Some(Box::new(Thrown {
        raised: Raised::plain(
            "UnicodeEncodeError",
            &format!("'ascii' codec can't encode {subject}: {reason}"),
        ),
        ctors: vec![Some(Ctor::Encode {
            encoding: "ascii".to_string(),
            object: text.clone(),
            start,
            end,
            reason: reason.to_string(),
        })],
        bases: Some(
            [
                "UnicodeEncodeError",
                "UnicodeError",
                "ValueError",
                "Exception",
                "BaseException",
            ]
            .iter()
            .map(|name| name.to_string())
            .collect(),
        ),
    }))
}

/// `sys.stdout.write(text)` under the profile.
fn write_stdout(world: &mut World<'_>, text: &PyStr) -> Done<()> {
    if world.profile == StdioProfile::AsciiStrict {
        if let Some(error) = ascii_encode_error(text) {
            return Err(error);
        }
    }
    world.host.out(text)
}

/// `print(text)`: the text and then the line end, two writes.
fn print(world: &mut World<'_>, text: &PyStr) -> Result<(), Failure> {
    write_stdout(world, text)
        .and_then(|()| write_stdout(world, &PyStr::new("\n")))
        .map_err(Failure::Thrown)
}

fn hex_digest(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `_sha(path)`.
fn sha(path: &[u8]) -> Result<String, Failure> {
    Ok(hex_digest(&read_bytes(path)?))
}

/// What `_context` returns that the actions use: the sidecar's lexical path and the record.
struct Context {
    sidecar: PathBytes,
    record: PyValue,
}

fn model_input_name(stem: &[u8], suffix: &str) -> PathBytes {
    [stem, suffix.as_bytes()].concat()
}

/// `_inside(path, parent)`: the resolved patch, which must lie under the resolved directory.
fn inside(patch: &[u8], directory: &[u8]) -> Result<PathBytes, Failure> {
    let resolved = resolve(patch).map_err(|failure| failure_of(failure, ResolveCall::Patch))?;
    let root = resolve(directory).map_err(|failure| failure_of(failure, ResolveCall::Parent))?;
    if !is_relative_to(&resolved, &root) {
        return Err(Failure::Caught(PyStr::from_surrogateescape(
            &[b"path escapes authoritative directory: ".as_slice(), patch].concat(),
        )));
    }
    Ok(resolved)
}

fn valid_stem(stem: &[u8]) -> bool {
    !stem.is_empty() && stem.iter().all(|byte| STEM_ALLOWED.contains(byte))
}

/// `_context(args)`.
fn context(arguments: &Arguments) -> Result<Context, Failure> {
    let patch_argument = path_text(&bytes_of(&arguments.patch)?);
    let config_dir = path_text(&bytes_of(&arguments.config_dir)?);
    let patch = inside(&patch_argument, &config_dir)?;
    let name = name_of(&patch);
    if !is_file(&patch)? || !name.ends_with(PATCH_SUFFIX) {
        return Err(caught(
            "patch must be an existing configs/<model>_patch.py file",
        ));
    }
    let stem = &name[..name.len() - PATCH_SUFFIX.len()];
    if !valid_stem(stem) {
        return Err(caught("invalid model stem"));
    }
    let env_dir = path_text(&bytes_of(&arguments.env_dir)?);
    let mut inputs: Vec<(String, String)> = Vec::new();
    for suffix in [".sh", ".yaml"] {
        let file = model_input_name(stem, suffix);
        let candidate = child_text(&config_dir, &file);
        if is_file(&candidate)? {
            inputs.push((
                String::from_utf8_lossy(&file).into_owned(),
                sha(&candidate)?,
            ));
        }
    }
    if inputs.is_empty() {
        return Err(caught(
            "runtime patch requires a current model .sh or .yaml input",
        ));
    }
    let env_name = [b".env.".as_slice(), stem].concat();
    let env = child_text(&env_dir, &env_name);
    if is_file(&env)? {
        inputs.push((
            format!("envs/{}", String::from_utf8_lossy(&env_name)),
            sha(&env)?,
        ));
    }
    let resolution = path_text(&bytes_of(&arguments.resolution)?);
    if !is_file(&resolution)? {
        return Err(caught("canonical production resolution missing"));
    }
    let parsed = parse_resolution(&resolution)?;
    if !matches!(&parsed, PyValue::Dict(object) if !object.is_empty()) {
        return Err(caught(
            "canonical production resolution must be a non-empty object",
        ));
    }
    let sidecar = child_text(
        &config_dir,
        &model_input_name(stem, "_patch.provenance.json"),
    );
    let patch_sha = sha(&patch)?;
    let resolution_sha = sha(&resolution)?;
    inputs.sort();
    let mut bound = PyDict::new();
    for (key, hash) in &inputs {
        bound.set(key, PyValue::str(hash));
    }
    // keys in sorted order: `json.dump(..., sort_keys=True)`
    let mut record = PyDict::new();
    record.set("canonical_resolution_sha256", PyValue::str(&resolution_sha));
    record.set("model_inputs_sha256", PyValue::Dict(bound));
    record.set(
        "model_stem",
        PyValue::Str(PyStr::from_surrogateescape(stem)),
    );
    record.set("patch_sha256", PyValue::str(&patch_sha));
    record.set("schema_version", PyValue::int(SCHEMA_VERSION));
    record.set("topology", PyValue::Str(arguments.topology.clone()));
    Ok(Context {
        sidecar,
        record: PyValue::Dict(record),
    })
}

/// `json.loads(resolution.read_text(encoding="utf-8"))`.
fn parse_resolution(resolution: &[u8]) -> Result<PyValue, Failure> {
    let text = decode_file_text(&read_bytes(resolution)?).map_err(load_failure)?;
    loads_text(&text).map_err(load_failure)
}

fn load_failure(error: LoadError) -> Failure {
    if error.is_value_error() {
        Failure::Caught(PyStr::new(&error.to_string()))
    } else {
        refusal(
            "json_recursion",
            "a JSON document nested beyond the C recursion limit; the interpreter's RecursionError traceback is not reproduced",
        )
    }
}

fn random_name() -> Result<String, OsFault> {
    let mut name = String::with_capacity(NAME_LENGTH);
    // rejection sampling keeps every character equally likely (37 does not divide 256)
    let limit = 256 - 256 % NAME_CHARACTERS.len();
    while name.len() < NAME_LENGTH {
        let mut byte = [0u8; 1];
        crate::pyhash::os_entropy(&mut byte).map_err(|error| OsFault::on(error, &[]))?;
        if usize::from(byte[0]) < limit {
            name.push(char::from(
                NAME_CHARACTERS[usize::from(byte[0]) % NAME_CHARACTERS.len()],
            ));
        }
    }
    Ok(name)
}

/// `tempfile.mkstemp(prefix=prefix, dir=directory)`: the open file and its absolute path.
fn make_temporary(prefix: &[u8], directory: &[u8]) -> Result<(std::fs::File, PathBytes), OsFault> {
    let directory = abspath(directory)?;
    for _ in 0..TEMPORARY_ATTEMPTS {
        let name = [prefix, random_name()?.as_bytes()].concat();
        let file = [directory.as_slice(), b"/", &name].concat();
        let file = if directory.ends_with(b"/") {
            [directory.as_slice(), &name].concat()
        } else {
            file
        };
        let opened = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(std::ffi::OsStr::from_bytes(&file));
        match opened {
            Ok(handle) => return Ok((handle, file)),
            Err(error) if error.raw_os_error() == Some(EEXIST) => continue,
            Err(error) => return Err(OsFault::on(error, &[&file])),
        }
    }
    Err(OsFault {
        error: std::io::Error::from_raw_os_error(EEXIST),
        files: Vec::new(),
    })
}

/// The sidecar text: `json.dump(record, fh, ensure_ascii=False, sort_keys=True, indent=2)` and a newline.
fn sidecar_bytes(record: &PyValue) -> Result<Vec<u8>, Failure> {
    let mut text = record
        .dumps_pretty()
        .map_err(|_| refusal("json_dump", "the record could not be rendered"))?;
    text.push_str("\n");
    text.to_surrogateescape_bytes().map_err(|_| {
        refusal(
            "json_dump",
            "the record holds a surrogate that is not an escaped byte",
        )
    })
}

fn write_and_replace(
    temporary: std::fs::File,
    tmp: &[u8],
    sidecar: &[u8],
    bytes: &[u8],
) -> Result<(), Failure> {
    let mut file = temporary;
    file.write_all(bytes)
        .map_err(|error| OsFault::on(error, &[]))?;
    drop(file);
    std::fs::rename(
        std::ffi::OsStr::from_bytes(tmp),
        std::ffi::OsStr::from_bytes(sidecar),
    )
    .map_err(|error| OsFault::on(error, &[tmp, sidecar]).into())
}

/// `stamp(args)`.
fn stamp(world: &mut World<'_>, arguments: &Arguments) -> Result<(), Failure> {
    let Context { sidecar, record } = context(arguments)?;
    make_directories(&parent_text(&sidecar))?;
    let (temporary, tmp) = make_temporary(
        &[name_of(&sidecar).as_slice(), b"."].concat(),
        &parent_text(&sidecar),
    )?;
    let written = sidecar_bytes(&record)
        .and_then(|bytes| write_and_replace(temporary, &tmp, &sidecar, &bytes));
    // `finally: if os.path.exists(tmp): os.unlink(tmp)`; its own failure replaces the pending one
    if exists(&tmp) {
        std::fs::remove_file(std::ffi::OsStr::from_bytes(&tmp))
            .map_err(|error| OsFault::on(error, &[&tmp]))?;
    }
    written?;
    print(world, &PyStr::from_surrogateescape(&sidecar))
}

/// `verify(args)`.
fn verify(world: &mut World<'_>, arguments: &Arguments) -> Result<(), Failure> {
    let Context { sidecar, record } = context(arguments)?;
    let shown = PyStr::from_surrogateescape(&sidecar);
    if !is_file(&sidecar)? {
        let mut message = PyStr::new("runtime patch provenance missing: ");
        message.push_py(&shown);
        return Err(Failure::Caught(message));
    }
    let actual = parse_resolution(&sidecar)?;
    if !equals(&actual, &record) {
        return Err(caught(
            "runtime patch provenance is stale or does not bind current topology/model inputs",
        ));
    }
    let mut line = PyStr::new("PASS ");
    line.push_py(&shown);
    print(world, &line)
}

fn run_action(world: &mut World<'_>, arguments: &Arguments) -> Result<(), Failure> {
    if arguments.action.equals("stamp") {
        stamp(world, arguments)
    } else {
        verify(world, arguments)
    }
}

fn action_of(arguments: &Arguments) -> Action {
    if arguments.action.equals("stamp") {
        Action::Stamp
    } else {
        Action::Verify
    }
}

fn parse_failure(world: &mut World<'_>, exit: Exit) -> Stop {
    let ascii = world.profile == StdioProfile::AsciiStrict;
    let all_ascii = [&exit.help, &exit.usage, &exit.error]
        .iter()
        .all(|text| text.code_points().iter().all(|point| *point <= 127));
    if ascii && !all_ascii {
        return Stop::Refused(Refusal {
            boundary: "ascii_stream_usage",
            detail: "argparse's output holds a non-ASCII character that the strict ASCII stream refuses outside the handler".to_string(),
        });
    }
    if !exit.help.is_empty() {
        world.host.parser_help(&exit.help);
    }
    if !exit.error.is_empty() {
        world.host.parser_error(&exit.usage, &exit.error);
    }
    Stop::Exit(exit.status)
}

/// `main(argv)` over `sys.argv`: `Ok` is a return of 0.
pub fn main(world: &mut World<'_>) -> Result<(), Stop> {
    let arguments = match parse_arguments(world.argv, world.columns) {
        Ok(arguments) => arguments,
        Err(exit) => return Err(parse_failure(world, exit)),
    };
    match run_action(world, &arguments) {
        Ok(()) => Ok(()),
        Err(Failure::Caught(message)) => report_failure(world, &message),
        Err(Failure::Thrown(thrown)) if is_handled(&thrown) => {
            report_failure(world, &thrown.raised.message)
        }
        Err(Failure::Thrown(thrown)) => Err(Stop::Uncaught(thrown.raised.report())),
        Err(Failure::Refused(refusal)) => Err(Stop::Refused(refusal)),
        Err(Failure::Loop { filename, call }) => {
            let script = world
                .argv
                .first()
                .cloned()
                .unwrap_or_else(|| PyStr::new(PROGRAM));
            Err(Stop::Uncaught(symlink_loop_report(
                &script,
                action_of(&arguments),
                call,
                &PyStr::from_surrogateescape(&filename),
            )))
        }
    }
}

/// `parser.exit(2, f"runtime-patch-provenance FAIL: {exc}\n")`: the message on standard error (a failed write is swallowed), then `SystemExit(2)`.
fn report_failure(world: &mut World<'_>, message: &PyStr) -> Result<(), Stop> {
    let mut line = PyStr::new("runtime-patch-provenance FAIL: ");
    line.push_py(message);
    line.push_str("\n");
    if world.profile == StdioProfile::AsciiStrict && ascii_encode_error(&line).is_some() {
        return Err(Stop::Refused(Refusal {
            boundary: "ascii_stream_message",
            detail: "the failure message holds a non-ASCII character that the strict ASCII standard error refuses outside the handler".to_string(),
        }));
    }
    world.host.parser_error(&PyStr::default(), &line);
    Err(Stop::Exit(2))
}
