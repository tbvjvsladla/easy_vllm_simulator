//! The checks of the conformance checker that run the installed Rust owners through their Bash launchers.
//!
//! * [`native_value`] sends one operation to `bench_reporting_api.sh evaluate-jsonl` and returns the text it answers.
//! * [`native_lite_certificate_gates`] runs the installed `publish_benchmark_record.sh` against a scratch directory:
//!   the refusal of a failed lite verdict in both publication paths and the positive control.
//! * [`LiteMetrics`] speaks to `lite_metrics_api.sh` with the protocol of the former Python adapter
//!   (`lite_metrics_native.py`), without running or reading that adapter: the same launcher, the same
//!   `evaluate-jsonl` request, the same reading of the answer, the same exceptions.
//!
//! Every call is a real child process (`bash` plus the launcher and the binary behind it); none is replaced by a
//! call into this crate's own library, so a broken launcher or a missing binary shows up here as it does in the
//! original.

use crate::captured_process::{run_captured_in, run_captured_unbounded, ChildSettings};
use crate::conformance::{
    dict_get, join_argument_error, rstrip, splitlines, subscript, tail, Context, Raised,
};
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_text};
use crate::pystr::PyStr;
use crate::pyvalue::{DumpError, PyDict, PyValue};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The wall-clock limit of the original's `subprocess.run(..., timeout=120)` calls.
const CHILD_TIMEOUT_SECONDS: u64 = 120;

/// How many characters of the child's standard error the failure message of `native_value` carries.
const STDERR_TAIL_CHARACTERS: usize = 600;

/// The stream decoding of `subprocess.run(..., text=True)`: strict UTF-8, universal newlines.
struct TextRun {
    status: i64,
    stdout: PyStr,
    stderr: PyStr,
}

fn dump_failure(error: &DumpError) -> Raised {
    let text = error.to_string();
    let message = text
        .strip_prefix(&format!("{}: ", error.python_type()))
        .unwrap_or(&text);
    Raised::new(error.python_type(), message)
}

/// The exception of a failed `subprocess.run`: a timeout carries the command and the limit as its arguments; an
/// operating-system error carries `(errno, text)`.
fn process_failure(failure: Failure, shown: &[PyValue], limit_text: &str) -> Raised {
    if failure.class == "TimeoutExpired" {
        let command = PyValue::List(shown.to_vec()).repr();
        return Raised::with_repr(failure, format!("TimeoutExpired({command}, {limit_text})"));
    }
    let message = failure.message.to_utf8().unwrap_or_default();
    let parsed = message
        .strip_prefix("[Errno ")
        .and_then(|rest| rest.split_once("] "))
        .and_then(|(errno, rest)| {
            let text = rest
                .split_once(": '")
                .or_else(|| rest.split_once(": \""))?
                .0;
            Some((errno.to_string(), text.to_string()))
        });
    match parsed {
        Some((errno, text)) => {
            let repr = format!("{}({errno}, {})", failure.class, PyStr::new(&text).repr());
            Raised::with_repr(failure, repr)
        }
        None => Raised::from_failure(failure),
    }
}

/// `subprocess.run(command, input=..., capture_output=True, text=True, timeout=...)`; `limit` is the timeout and the
/// text of its `str()`, or `None` for the calls the original makes without a timeout.
fn run_text(
    command: &[PyStr],
    input: Option<&str>,
    limit: Option<(Duration, &str)>,
) -> Result<TextRun, Raised> {
    let words: Vec<OsString> = command
        .iter()
        .map(|word| {
            crate::pyfs::os_string(word)
                .map_err(|failure| Raised::from_failure(failure.into_failure()))
        })
        .collect::<Result<_, _>>()?;
    let shown: Vec<PyValue> = command.iter().cloned().map(PyValue::Str).collect();
    let settings = ChildSettings {
        input: input.map(|text| text.as_bytes().to_vec()),
        ..ChildSettings::default()
    };
    let captured = match limit {
        Some((duration, text)) => run_captured_in(&words, &shown, duration, text, &settings)
            .map_err(|failure| process_failure(failure, &shown, text)),
        None => run_captured_unbounded(&words, &shown, &settings)
            .map_err(|failure| process_failure(failure, &shown, "")),
    }?;
    // `communicate` decodes the output stream first, then the error stream.
    let stdout = decode_file_text(&captured.stdout)
        .map_err(|error| Raised::decoding(&captured.stdout, &error))?;
    let stderr = decode_file_text(&captured.stderr)
        .map_err(|error| Raised::decoding(&captured.stderr, &error))?;
    Ok(TextRun {
        status: captured.status,
        stdout,
        stderr,
    })
}

fn bash_command(context: &Context, launcher: &str, arguments: &[&PyStr]) -> Vec<PyStr> {
    let mut command = vec![PyStr::new("bash"), context.join(&PyStr::new(launcher))];
    command.extend(arguments.iter().map(|argument| (*argument).clone()));
    command
}

fn child_limit() -> (Duration, String) {
    (
        Duration::from_secs(CHILD_TIMEOUT_SECONDS),
        CHILD_TIMEOUT_SECONDS.to_string(),
    )
}

fn runtime_error(message: &PyStr, prefix: &str) -> Raised {
    let mut text = PyStr::new(prefix);
    text.push_py(message);
    Raised::from_failure(Failure::with_message("RuntimeError", text))
}

// ── native_value ────────────────────────────────────────────────────────────────

/// `native_value(operation, arguments)`: the text the installed reporting owner returns for one operation.
///
/// The request is one JSON line `{"id": "conformance", "operation": ..., "arguments": ...}` on the standard input of
/// `bash bench_reporting_api.sh evaluate-jsonl`. A nonzero exit or any standard error output is a `RuntimeError`
/// carrying the last 600 characters of standard error; an answer that is not a `value` record of this request with a
/// string value is a `RuntimeError` showing the answer.
pub fn native_value(
    context: &Context,
    operation: &PyValue,
    arguments: &PyValue,
) -> Result<PyStr, Raised> {
    let mut row = PyDict::new();
    row.set("id", PyValue::str("conformance"));
    row.set("operation", operation.clone());
    row.set("arguments", arguments.clone());
    let request = PyValue::Dict(row)
        .dumps_ascii()
        .map_err(|error| dump_failure(&error))?;
    let command = bash_command(
        context,
        "bench_reporting_api.sh",
        &[&PyStr::new("evaluate-jsonl")],
    );
    let (limit, limit_text) = child_limit();
    let run = run_text(
        &command,
        Some(&format!("{request}\n")),
        Some((limit, &limit_text)),
    )?;
    if run.status != 0 || !run.stderr.is_empty() {
        return Err(runtime_error(
            &tail(&run.stderr, STDERR_TAIL_CHARACTERS),
            "native reporting call failed: ",
        ));
    }
    let result = loads_text(&run.stdout)
        .map_err(|error| Raised::from_failure(crate::repeat_api_cli::load_failure(&error)))?;
    let id = dict_get(&result, "id")?;
    let kind = dict_get(&result, "kind")?;
    let value = dict_get(&result, "value")?;
    let is_answer = crate::conformance::equals_text(&id, "conformance")
        && crate::conformance::equals_text(&kind, "value")
        && matches!(value, PyValue::Str(_));
    match value {
        PyValue::Str(text) if is_answer => Ok(text),
        _ => Err(runtime_error(
            &PyStr::new(&result.repr()),
            "native reporting returned no text: ",
        )),
    }
}

// ── native_lite_certificate_gates ───────────────────────────────────────────────

/// A scratch directory that is removed when it goes out of scope (`tempfile.TemporaryDirectory`).
struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    const NAME_CHARACTERS: &'static [u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    const NAME_LENGTH: usize = 8;
    const ATTEMPTS: u32 = 100;

    fn create(prefix: &str) -> Result<ScratchDirectory, Raised> {
        use std::hash::{BuildHasher, Hasher};
        use std::os::unix::fs::DirBuilderExt;
        let base = std::env::temp_dir();
        let mut failure = None;
        for attempt in 0..Self::ATTEMPTS {
            let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
            hasher.write_u32(attempt);
            hasher.write_u32(std::process::id());
            let mut bits = hasher.finish();
            let name: String = (0..Self::NAME_LENGTH)
                .map(|_| {
                    let character = Self::NAME_CHARACTERS[(bits % 37) as usize] as char;
                    bits /= 37;
                    character
                })
                .collect();
            let path = base.join(format!("{prefix}{name}"));
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(ScratchDirectory { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    failure = Some(Raised::os_error(&error, &path_text(&path)));
                    break;
                }
            }
        }
        Err(failure.unwrap_or_else(|| {
            Raised::new(
                "FileExistsError",
                "No usable temporary directory name found",
            )
        }))
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn path_text(path: &Path) -> PyStr {
    use std::os::unix::ffi::OsStrExt;
    PyStr::from_surrogateescape(path.as_os_str().as_bytes())
}

fn write_json(path: &Path, value: &PyValue) -> Result<(), Raised> {
    let text = value.dumps_ascii().map_err(|error| dump_failure(&error))?;
    std::fs::write(path, text).map_err(|error| Raised::os_error(&error, &path_text(path)))
}

fn object<const N: usize>(items: [(&str, PyValue); N]) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in items {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

/// The class of the real publication CLI refusing a failed lite verdict, and its positive control.
///
/// Three runs of the installed `publish_benchmark_record.sh` in a scratch directory: the beta path (a raw lite
/// verdict, `--lite-raw-json`) must refuse with status 3 and print nothing; the gamma path (a downgraded sweep index
/// with a `server_failed` lite block) must exit 0 and print nothing; the same sweep index with a `pass` lite block
/// must print a lite-grade certificate (`benchmark_mode: lite`, `verdict: not_applicable`).
pub fn native_lite_certificate_gates(context: &Context) -> Result<bool, Raised> {
    const STAMP: &str = "2026-09-04T11:22:33Z";
    let scratch = ScratchDirectory::create("native-lite-cert-gates.")?;
    let raw = scratch.file("raw.json");
    write_json(
        &raw,
        &object([("lite_verdict", PyValue::str("server_failed"))]),
    )?;
    let run = |arguments: &[PyStr]| -> Result<TextRun, Raised> {
        let refs: Vec<&PyStr> = arguments.iter().collect();
        let command = bash_command(context, "publish_benchmark_record.sh", &refs);
        let (limit, limit_text) = child_limit();
        run_text(&command, None, Some((limit, &limit_text)))
    };
    let text = |value: &str| PyStr::new(value);
    let beta = run(&[text("--lite-raw-json"), path_text(&raw), text("--stdout")])?;
    let mut index = object([
        ("meta", PyValue::Dict(PyDict::new())),
        ("generated_utc", PyValue::str(STAMP)),
        ("levels", PyValue::List(Vec::new())),
        ("repetition", object([("requested", PyValue::int(3))])),
        (
            "lite",
            object([("lite_verdict", PyValue::str("server_failed"))]),
        ),
    ]);
    let sweep_index = scratch.file("sweep_index.json");
    let verdict = scratch.file("verdict.json");
    write_json(&verdict, &object([("verdict", PyValue::str("PASS"))]))?;
    write_json(
        &scratch.file("bench_mode.json"),
        &object([
            ("sweep_index_generated_utc", PyValue::str(STAMP)),
            ("bench_mode", PyValue::str("lite")),
            ("downgrade_reason", PyValue::str("run_failed")),
            ("bench_mode_source", PyValue::str("self-test fixture")),
        ]),
    )?;
    let gamma_arguments = [
        text("--sweep-index"),
        path_text(&sweep_index),
        text("--verdict-json"),
        path_text(&verdict),
        text("--stdout"),
    ];
    write_json(&sweep_index, &index)?;
    let gamma_refused = run(&gamma_arguments)?;
    if let Some(PyValue::Dict(lite)) = match &mut index {
        PyValue::Dict(map) => map.get_mut("lite"),
        _ => None,
    } {
        lite.set("lite_verdict", PyValue::str("pass"));
    }
    write_json(&sweep_index, &index)?;
    let gamma_published = run(&gamma_arguments)?;
    let printed = gamma_published.stdout.to_utf8().unwrap_or_default();
    Ok(beta.status == 3
        && beta.stdout.is_empty()
        && gamma_refused.status == 0
        && gamma_refused.stdout.is_empty()
        && gamma_published.status == 0
        && printed.contains("benchmark_mode: lite")
        && printed.contains("verdict: not_applicable"))
}

// ── the lite metrics owner ──────────────────────────────────────────────────────

/// The exception classes `getattr(builtins, name)` gives that are `Exception` subclasses (or `SystemExit`): the
/// closed list of classes an exception record of the lite metrics owner may name. Any other name is reported as a
/// `RuntimeError` showing the name. Keep sorted; it is a tripwire, not a derivation.
const BUILTIN_EXCEPTIONS: &[&str] = &[
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BlockingIOError",
    "BrokenPipeError",
    "BufferError",
    "BytesWarning",
    "ChildProcessError",
    "ConnectionAbortedError",
    "ConnectionError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "DeprecationWarning",
    "EOFError",
    "EncodingWarning",
    "EnvironmentError",
    "Exception",
    "ExceptionGroup",
    "FileExistsError",
    "FileNotFoundError",
    "FloatingPointError",
    "FutureWarning",
    "IOError",
    "ImportError",
    "ImportWarning",
    "IndentationError",
    "IndexError",
    "InterruptedError",
    "IsADirectoryError",
    "KeyError",
    "LookupError",
    "MemoryError",
    "ModuleNotFoundError",
    "NameError",
    "NotADirectoryError",
    "NotImplementedError",
    "OSError",
    "OverflowError",
    "PendingDeprecationWarning",
    "PermissionError",
    "ProcessLookupError",
    "RecursionError",
    "ReferenceError",
    "ResourceWarning",
    "RuntimeError",
    "RuntimeWarning",
    "StopAsyncIteration",
    "StopIteration",
    "SyntaxError",
    "SyntaxWarning",
    "SystemError",
    "SystemExit",
    "TabError",
    "TimeoutError",
    "TypeError",
    "UnboundLocalError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeError",
    "UnicodeTranslateError",
    "UnicodeWarning",
    "UserWarning",
    "ValueError",
    "Warning",
    "ZeroDivisionError",
];

/// The classes the original's `except (RuntimeError, ValueError, KeyError, TypeError)` around the import catches,
/// subclasses included.
const IMPORT_CAUGHT: &[&str] = &[
    "RuntimeError",
    "RecursionError",
    "NotImplementedError",
    "ValueError",
    "JSONDecodeError",
    "UnicodeError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeTranslateError",
    "KeyError",
    "TypeError",
];

const REQUEST_ID: &str = "python-consumer";
const RECORD_ENCODING: &str = "json-values-v1";

/// The modules the checker has imported in this context (the original's `sys.modules`). Only a **successful** import is
/// kept, and only the imported constants: a failed import leaves nothing behind (the interpreter removes a module
/// that failed to import, so the next `import` runs again), and every operation, report, source read and child process
/// still happens each time.
#[derive(Debug, Default)]
pub struct ModuleCache {
    lite_metrics: std::sync::Mutex<Option<LiteBindings>>,
}

/// What the lite metrics owner declares as constants at "import" time.
#[derive(Clone, Debug)]
pub struct LiteBindings {
    /// `tuple(LITE_VERDICTS)`: the verdict vocabulary.
    pub verdicts: Vec<PyValue>,
    /// `LITE_EXIT`: the exit status of each verdict.
    pub exit: PyValue,
}

/// The transport to the lite metrics owner: the launcher the adapter would use.
pub struct LiteMetrics {
    launcher: PyStr,
}

impl LiteMetrics {
    /// The launcher `lite_metrics_api.sh` of the scripts directory, in its directory after symbolic links are followed
    /// (the former Python adapter's `Path(adapter).resolve().with_name(...)`; nothing depends on that adapter file).
    pub fn beside_adapter(context: &Context) -> LiteMetrics {
        let adapter = context.join(&PyStr::new("lite_metrics_api.sh"));
        let resolved = crate::pyfs::os_string(&adapter)
            .ok()
            .and_then(|os| std::fs::canonicalize(Path::new(&os)).ok())
            .map(|path| path_text(&path))
            .unwrap_or(adapter);
        let directory = crate::ospath::dirname(&resolved);
        LiteMetrics {
            launcher: crate::ospath::join(&directory, &[&PyStr::new("lite_metrics_api.sh")]),
        }
    }

    fn run(&self, arguments: &[&str], payload: Option<&str>) -> Result<PyStr, Raised> {
        let mut command = vec![PyStr::new("bash"), self.launcher.clone()];
        command.extend(arguments.iter().map(|argument| PyStr::new(argument)));
        // `subprocess.run` of the original adapter has no `timeout`: no limit is invented here either.
        let run = run_text(&command, payload, None)?;
        if run.status != 0 || !run.stderr.is_empty() {
            let mut message = PyStr::new(&format!(
                "lite metrics native transport failed ({}): ",
                run.status
            ));
            message.push_py(&rstrip(&run.stderr));
            return Err(Raised::from_failure(Failure::with_message(
                "RuntimeError",
                message,
            )));
        }
        Ok(run.stdout)
    }

    /// `import lite_metrics_native`: the cached module when this context imported it before, else [`Self::import`]
    /// once. The lock is held across the import, as the interpreter's import lock is, and only `Ok` is stored.
    pub fn load(&self, modules: &ModuleCache) -> Result<LiteBindings, Raised> {
        let mut slot = modules
            .lite_metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(bindings) = slot.as_ref() {
            return Ok(bindings.clone());
        }
        let bindings = self.import()?;
        *slot = Some(bindings.clone());
        Ok(bindings)
    }

    /// The module-level code of the adapter: read the constants; a failure of the expected kinds is an `ImportError`.
    /// Every call asks the owner; [`Self::load`] is the cached form `main` uses.
    pub fn import(&self) -> Result<LiteBindings, Raised> {
        let read = || -> Result<LiteBindings, Raised> {
            let text = self.run(&["constants"], None)?;
            let bindings = loads_text(&text).map_err(|error| {
                Raised::from_failure(crate::repeat_api_cli::load_failure(&error))
            })?;
            let verdicts =
                crate::conformance::iterate_values(&subscript(&bindings, "LITE_VERDICTS")?)?;
            let exit = subscript(&bindings, "LITE_EXIT")?;
            Ok(LiteBindings { verdicts, exit })
        };
        read().map_err(|error| {
            if IMPORT_CAUGHT.contains(&error.failure().class) {
                Raised::from_failure(Failure::with_message(
                    "ImportError",
                    error.failure().message.clone(),
                ))
            } else {
                error
            }
        })
    }

    /// `_call(operation, arguments)`: one `evaluate-jsonl` request with each argument as its own JSON text.
    pub fn call(&self, operation: &str, arguments: &[(&str, &PyValue)]) -> Result<PyValue, Raised> {
        let mut texts = PyDict::new();
        for (name, value) in arguments {
            let text = value.dumps_ascii().map_err(|error| dump_failure(&error))?;
            texts.set(name, PyValue::str(&text));
        }
        let request = PyValue::Dict({
            let mut map = PyDict::new();
            map.set("id", PyValue::str(REQUEST_ID));
            map.set("operation", PyValue::str(operation));
            map.set("arguments_json", PyValue::Dict(texts));
            map.set("result_json", PyValue::Bool(true));
            map
        });
        let line = request
            .dumps_ascii()
            .map_err(|error| dump_failure(&error))?;
        let answer = self.run(&["evaluate-jsonl"], Some(&format!("{line}\n")))?;
        let lines = splitlines(&answer);
        let [only] = lines.as_slice() else {
            return Err(Raised::new(
                "RuntimeError",
                "lite metrics native transport did not return one record",
            ));
        };
        let record = loads_text(only)
            .map_err(|error| Raised::from_failure(crate::repeat_api_cli::load_failure(&error)))?;
        let id = dict_get(&record, "id")?;
        let encoding = dict_get(&record, "encoding")?;
        if !crate::conformance::equals_text(&id, REQUEST_ID)
            || !crate::conformance::equals_text(&encoding, RECORD_ENCODING)
        {
            return Err(Raised::new(
                "RuntimeError",
                "lite metrics native binary lacks the required JSON transport; rebuild it",
            ));
        }
        let kind = dict_get(&record, "kind")?;
        if crate::conformance::equals_text(&kind, "value") {
            return match subscript(&record, "value_json")? {
                PyValue::Str(json) => loads_text(&json).map_err(|error| {
                    Raised::from_failure(crate::repeat_api_cli::load_failure(&error))
                }),
                other => Err(Raised::new(
                    "TypeError",
                    &format!(
                        "the JSON object must be str, bytes or bytearray, not {}",
                        other.type_name()
                    ),
                )),
            };
        }
        if !crate::conformance::equals_text(&kind, "exception") {
            return Err(Raised::new(
                "RuntimeError",
                "lite metrics native transport returned an unknown record kind",
            ));
        }
        Err(reported_exception(&record))
    }

    pub fn build(&self, raw: &PyValue) -> Result<PyValue, Raised> {
        self.call("build", &[("raw", raw)])
    }

    pub fn judge_lite(&self, raw: &PyValue, built: &PyValue) -> Result<PyValue, Raised> {
        self.call("judge_lite", &[("raw", raw), ("built", built)])
    }

    pub fn read_lite_verdict(&self, raw: &PyValue) -> Result<PyValue, Raised> {
        self.call("read_lite_verdict", &[("raw", raw)])
    }

    pub fn load_json(&self, path: &PyValue) -> Result<PyValue, Raised> {
        self.call("_load_json", &[("path", path)])
    }
}

/// `_raise(record)`: the exception an exception record stands for. A name that is not a builtin exception class is a
/// `RuntimeError` naming it; a `KeyError` message is read back as the Python literal it prints (`'key'`) so the key
/// is the argument.
fn reported_exception(record: &PyValue) -> Raised {
    let (name, message) = match (subscript(record, "type"), subscript(record, "message")) {
        (Ok(name), Ok(message)) => (name, message),
        (Err(error), _) | (_, Err(error)) => return error,
    };
    let PyValue::Str(class_name) = &name else {
        return Raised::new(
            "TypeError",
            &format!("attribute name must be string, not '{}'", name.type_name()),
        );
    };
    let known = class_name
        .to_utf8()
        .and_then(|text| BUILTIN_EXCEPTIONS.iter().find(|known| **known == text));
    let Some(class) = known else {
        let mut text = PyStr::new("lite metrics native owner reported ");
        text.push_py(class_name);
        text.push_str(": ");
        text.push_py(&message.display_text());
        return Raised::from_failure(Failure::with_message("RuntimeError", text));
    };
    if *class == "KeyError" {
        let argument = match &message {
            PyValue::Str(text) => literal_value(text).unwrap_or_else(|| message.clone()),
            other => other.clone(),
        };
        return Raised::with_repr(
            Failure::new("KeyError", &argument.repr()),
            format!("KeyError({})", argument.repr()),
        );
    }
    Raised::with_repr(
        Failure::with_message(class, message.display_text()),
        format!("{class}({})", message.repr()),
    )
}

/// `ast.literal_eval(text)` for the literals a `KeyError` message holds: a quoted string, an integer, `None`,
/// `True`, `False`. Anything else is left to the caller (the original keeps the message itself when the text is not a
/// literal; richer literals are outside this port's scope).
fn literal_value(text: &PyStr) -> Option<PyValue> {
    let source = text.to_utf8()?;
    let source = source.trim_start_matches([' ', '\t']);
    match source {
        "None" => return Some(PyValue::None),
        "True" => return Some(PyValue::Bool(true)),
        "False" => return Some(PyValue::Bool(false)),
        _ => {}
    }
    if let Ok(number) = source.parse::<i64>() {
        return Some(PyValue::int(number));
    }
    let mut characters = source.chars();
    let quote = characters.next().filter(|c| *c == '\'' || *c == '"')?;
    let mut value = String::new();
    loop {
        match characters.next()? {
            '\\' => value.push(match characters.next()? {
                'n' => '\n',
                't' => '\t',
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                _ => return None,
            }),
            c if c == quote => break,
            c => value.push(c),
        }
    }
    characters.next().is_none().then(|| PyValue::str(&value))
}

/// The `TypeError` of passing a value that is not text where the original joins a path.
pub fn path_argument(value: &PyValue) -> Result<PyStr, Raised> {
    match value.peel() {
        PyValue::Str(text) => Ok(text.clone()),
        other => Err(join_argument_error(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(text: &str) -> PyValue {
        crate::pyjson::loads(text).expect("literal JSON")
    }

    #[test]
    fn a_reported_key_error_carries_the_key_as_its_argument() {
        let raised = reported_exception(&record(r#"{"type": "KeyError", "message": "'raw'"}"#));
        assert_eq!(raised.failure().class, "KeyError");
        assert_eq!(raised.repr(), "KeyError('raw')");
        let odd = reported_exception(&record(
            r#"{"type": "KeyError", "message": "not a literal"}"#,
        ));
        assert_eq!(odd.repr(), "KeyError('not a literal')");
    }

    #[test]
    fn a_reported_class_outside_the_builtins_is_a_runtime_error_naming_it() {
        let raised =
            reported_exception(&record(r#"{"type": "JSONDecodeError", "message": "bad"}"#));
        assert_eq!(raised.failure().class, "RuntimeError");
        assert_eq!(
            raised.failure().message.to_utf8().unwrap(),
            "lite metrics native owner reported JSONDecodeError: bad"
        );
        let known = reported_exception(&record(r#"{"type": "ValueError", "message": "bad"}"#));
        assert_eq!(known.repr(), "ValueError('bad')");
        let not_exception =
            reported_exception(&record(r#"{"type": "KeyboardInterrupt", "message": "x"}"#));
        assert_eq!(not_exception.failure().class, "RuntimeError");
    }

    #[test]
    fn literals_of_key_error_messages_are_read_like_python() {
        let value = |text: &str| literal_value(&PyStr::new(text)).map(|v| v.repr());
        assert_eq!(value("'a\\'b'").as_deref(), Some("\"a'b\""));
        assert_eq!(value("\"x\"").as_deref(), Some("'x'"));
        assert_eq!(value("5").as_deref(), Some("5"));
        assert_eq!(value("None").as_deref(), Some("None"));
        assert_eq!(value("'x' 'y'"), None);
        assert_eq!(value("plain"), None);
    }

    #[test]
    fn process_failures_keep_their_python_arguments() {
        let shown = [PyValue::str("bash"), PyValue::str("x.sh")];
        let timeout = process_failure(
            Failure::new(
                "TimeoutExpired",
                "Command '...' timed out after 120 seconds",
            ),
            &shown,
            "120",
        );
        assert_eq!(timeout.repr(), "TimeoutExpired(['bash', 'x.sh'], 120)");
        let missing = process_failure(
            Failure::new(
                "FileNotFoundError",
                "[Errno 2] No such file or directory: 'bash'",
            ),
            &shown,
            "120",
        );
        assert_eq!(
            missing.repr(),
            "FileNotFoundError(2, 'No such file or directory')"
        );
    }

    #[test]
    fn the_scratch_directory_is_removed_with_its_owner() {
        let path = {
            let scratch = ScratchDirectory::create("conformance-unit.").expect("scratch");
            assert!(scratch.path.is_dir());
            scratch.path.clone()
        };
        assert!(!path.exists());
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("conformance-unit."));
        assert_eq!(name.len(), "conformance-unit.".len() + 8);
    }
}
