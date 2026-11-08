//! `resolve_bench_tool_api`: the Rust-only surface of the pin resolver, a separate program so that the
//! `resolve_bench_tool` command line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - the functions of the original module as a transport. Each stdin line is
//!   `{id, operation, arguments}` where `operation` is a Python function name: `load_pin`, `classify`, `_inspect`,
//!   `main`, `_self_test`. A line yields `{id, kind: value, value}` or `{id, kind: exception, type, message}`, plus
//!   `stderr` (and `stdout`) when the call wrote to those streams before returning or raising; the `SystemExit` of
//!   a refused command line has its text there. Arguments follow the Python signatures (names, defaults and the
//!   `TypeError` a wrong call raises). Tuples come back as JSON arrays (`classify` is `[rc, observed, matches]`).
//!   `load_pin` takes the pin file as text (the wire form of a path). Opt-in: `arguments_json` (each argument as raw
//!   JSON text, decoded separately) and `result_json: true` (the result as compact ASCII JSON text in
//!   `value_json`). `accepted_digests` is not an operation: its result is a set, which has no JSON form, and it is
//!   reachable through `classify` (the decision) and `load_pin` (the record) alone.
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `native-help` - this list.

use crate::bench_tool_cli::main_with;
use crate::bench_tool_pin::{
    classify, default_pin_path, inspect, load_pin, DEFAULT_TOOL, DIGEST_SOURCE, PIN_PATH_ENV,
    RC_ABSENT, RC_ARGS, RC_OK,
};
use crate::bench_tool_selftest::self_test;
use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::dataset_prep::Recorder;
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, row_arguments, Signature, Wire,
};

const NATIVE_HELP: &str = "resolve_bench_tool_api Rust-only commands:\n  resolve_bench_tool_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  resolve_bench_tool_api constants        the module constants as one JSON object\n  resolve_bench_tool_api native-help      this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(io),
        Some("constants") if args.len() == 1 => {
            crate::repeat_api_cli::print_text(io, &constants().dumps_ascii().unwrap_or_default())
        }
        Some("native-help") if args.len() == 1 => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure),
        },
        _ => {
            write_stderr(io, &PyStr::new(NATIVE_HELP));
            2
        }
    };
    io.finish(status)
}

/// The constants of the original module, from this crate. `PIN_RELATIVE` is the default pin file as this process
/// resolves it (the environment variable [`PIN_PATH_ENV`], else the pin file beside the crate's folder).
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    map.set("DEFAULT_TOOL", PyValue::str(DEFAULT_TOOL));
    map.set("PIN_RELATIVE", PyValue::Str(default_pin_path()));
    map.set("RC_OK", PyValue::int(RC_OK));
    map.set("RC_ARGS", PyValue::int(RC_ARGS));
    map.set("RC_ABSENT", PyValue::int(RC_ABSENT));
    map.set("DIGEST_SOURCE", PyValue::str(DIGEST_SOURCE));
    map.set("PIN_PATH_ENV", PyValue::str(PIN_PATH_ENV));
    PyValue::Dict(map)
}

const LOAD_PIN: Signature = Signature {
    function: "load_pin",
    positional: &["path", "tool", "version"],
    keyword_only: &[],
    optional: &["version"],
};
const CLASSIFY: Signature = Signature {
    function: "classify",
    positional: &["entry", "repo_digests"],
    keyword_only: &[],
    optional: &[],
};
const INSPECT: Signature = Signature {
    function: "_inspect",
    positional: &["image_ref"],
    keyword_only: &[],
    optional: &[],
};
const MAIN: Signature = Signature {
    function: "main",
    positional: &["argv"],
    keyword_only: &[],
    optional: &["argv"],
};
const SELF_TEST: Signature = Signature {
    function: "_self_test",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};
const CONSTANTS: Signature = Signature {
    function: "constants",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};

/// `main(argv)` with `argv` a list of strings (absent: the process's own arguments, which a transport has none of).
fn argv_of(value: &PyValue) -> Result<Vec<PyStr>, Failure> {
    match value.peel() {
        PyValue::None => Ok(Vec::new()),
        PyValue::List(items) | PyValue::Tuple(items) => items
            .iter()
            .map(|item| match item.peel() {
                PyValue::Str(text) => Ok(text.clone()),
                other => Err(Failure::new(
                    "TypeError",
                    &format!("argument of type '{}' is not iterable", other.type_name()),
                )),
            })
            .collect(),
        other => Err(Failure::new(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

/// One operation, with what it wrote to the two standard streams kept in `streams`. The transport never forwards the
/// operation's own exit: a `SystemExit` is reported as the exception it is.
pub fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    streams: &mut Recorder,
) -> Result<PyValue, Failure> {
    match operation {
        "load_pin" => {
            let call = LOAD_PIN.bind(arguments)?;
            let row = load_pin(
                call.get_ref("path"),
                call.get_ref("tool"),
                call.get_ref("version"),
            )?;
            Ok(PyValue::Dict(row))
        }
        "classify" => {
            let call = CLASSIFY.bind(arguments)?;
            Ok(classify(call.get_ref("entry"), call.get_ref("repo_digests"))?.to_value())
        }
        "_inspect" => {
            let call = INSPECT.bind(arguments)?;
            Ok(inspect(call.get_ref("image_ref"))?.map_or(PyValue::None, PyValue::List))
        }
        "main" => {
            let call = MAIN.bind(arguments)?;
            let args = argv_of(call.get_ref("argv"))?;
            Ok(PyValue::int(main_with(&args, streams)?))
        }
        "_self_test" => {
            SELF_TEST.bind(arguments)?;
            Ok(PyValue::int(self_test(streams)?))
        }
        "constants" => {
            CONSTANTS.bind(arguments)?;
            Ok(constants())
        }
        other => Err(Failure::new(
            "ValueError",
            &format!("unknown operation {}", PyStr::new(other).repr()),
        )),
    }
}

fn row_operation(row: &PyDict) -> Result<String, Failure> {
    match row.get("operation") {
        Some(PyValue::Str(name)) => Ok(name.to_utf8().unwrap_or_default()),
        Some(other) => Ok(other.repr()),
        None => Err(Failure::new("KeyError", "'operation'")),
    }
}

/// The record with the streams the call wrote to attached when they are not empty.
fn with_streams(record: PyValue, streams: &Recorder) -> PyValue {
    let PyValue::Dict(mut map) = record else {
        return record;
    };
    if !streams.stderr.is_empty() {
        map.set("stderr", PyValue::Str(streams.stderr.clone()));
    }
    if !streams.stdout.is_empty() {
        map.set("stdout", PyValue::Str(streams.stdout.clone()));
    }
    PyValue::Dict(map)
}

fn evaluate_row(row: &PyValue) -> PyValue {
    let PyValue::Dict(map) = row else {
        return exception_record(
            PyValue::None,
            &Failure::new("TypeError", "operation row is not an object"),
            Wire::Values,
        );
    };
    let id = map.get("id").cloned().unwrap_or(PyValue::None);
    let wire = match map.get("result_json") {
        Some(PyValue::Bool(true)) => Wire::JsonText,
        _ => Wire::Values,
    };
    let mut streams = Recorder::default();
    let outcome = (|| {
        let operation = row_operation(map)?;
        evaluate_operation(&operation, &row_arguments(map, true)?, &mut streams)
    })();
    let record = match outcome {
        Ok(value) => crate::repeat_api_cli::value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    };
    with_streams(record, &streams)
}

fn run_jsonl(io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[resolve_bench_tool_api] ERROR stdin: {error}\n")),
        );
        return 2;
    }
    // The transport is deliberately strict UTF-8 in and ASCII out; it is not the legacy stream behaviour.
    let text = match decode_file_text(&bytes) {
        Ok(text) => text
            .to_utf8()
            .expect("strict decoding yields no surrogates"),
        Err(error) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[resolve_bench_tool_api] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads_objects(line) {
                Ok(row) => evaluate_row(&row),
                Err(error) => exception_record(PyValue::None, &load_failure(&error), Wire::Values),
            };
            match record.dumps_ascii() {
                Ok(json) => out.push_str(&json),
                Err(error) => {
                    let id = match &record {
                        PyValue::Dict(map) => map.get("id").cloned().unwrap_or(PyValue::None),
                        _ => PyValue::None,
                    };
                    let text = error.to_string();
                    let message = text
                        .strip_prefix(&format!("{}: ", error.python_type()))
                        .unwrap_or(&text);
                    out.push_str(
                        &exception_record(
                            id,
                            &Failure::new(error.python_type(), message),
                            Wire::Values,
                        )
                        .dumps_ascii()
                        .unwrap_or_default(),
                    );
                }
            }
            out.push('\n');
        }
        out
    });
    match io.print_lines(&PyStr::new(&output)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure),
    }
}
