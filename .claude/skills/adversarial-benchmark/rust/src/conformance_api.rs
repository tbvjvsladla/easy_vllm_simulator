//! `conformance_full_superset_lite_api`: the Rust-only surface of the conformance checker, a separate program so that
//! the `conformance_full_superset_lite` command line keeps answering exactly what the original did (it ignores its
//! arguments, `--help` included).
//!
//! * `evaluate-jsonl` - the functions of the original module as a transport. Each stdin line is
//!   `{id, operation, arguments}` where `operation` is a Python function name: `read`, `_emit`, `native_value`,
//!   `native_lite_certificate_gates`, `main`. A line yields `{id, kind: value, value}` or
//!   `{id, kind: exception, type, message}`. The operations that print (`_emit`, `main`) add a `stdout` field with
//!   everything they printed, **also when they end in an exception** (the lines printed before it are part of the
//!   behaviour). Arguments follow the Python signatures (names, and the `TypeError` a wrong call raises). Opt-in:
//!   `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the result as
//!   compact ASCII JSON text in `value_json`). There is no dispatch to any other name.
//! * `constants` - `LITE_METRIC_KEYS` and `CERT_LITE_FIELDS` as one JSON object.
//! * `native-help` - this list.
//!
//! The scripts directory (`SDIR`) is the one of the environment variable `CONFORMANCE_SCRIPTS_DIR`; an operation that
//! needs it fails with a `RuntimeError` when it is not set. The checks launch the real launchers found there.

use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::conformance::{
    emit, read, Context, CERT_LITE_FIELDS, LITE_METRIC_KEYS, SCRIPTS_DIR_ENV,
};
use crate::conformance_checks;
use crate::conformance_native::{native_lite_certificate_gates, native_value, path_argument};
use crate::dataset_prep::Recorder;
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, print_text, row_arguments, strings, value_record,
    Signature, Wire,
};

const NATIVE_HELP: &str = "conformance_full_superset_lite_api Rust-only commands:\n  conformance_full_superset_lite_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  conformance_full_superset_lite_api constants        LITE_METRIC_KEYS and CERT_LITE_FIELDS as one JSON object\n  conformance_full_superset_lite_api native-help      this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_with(args, Context::from_environment().as_ref(), io)
}

/// [`run`] with an explicit context (`None` = no scripts directory is known).
pub fn run_with(args: &[PyStr], context: Option<&Context>, io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(io, context),
        Some("constants") if args.len() == 1 => {
            print_text(io, &constants().dumps_ascii().unwrap_or_default())
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

/// The module constants of the original.
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    map.set("LITE_METRIC_KEYS", strings(&LITE_METRIC_KEYS));
    map.set("CERT_LITE_FIELDS", strings(&CERT_LITE_FIELDS));
    PyValue::Dict(map)
}

const READ: Signature = Signature {
    function: "read",
    positional: &["name"],
    keyword_only: &[],
    optional: &[],
};
const EMIT: Signature = Signature {
    function: "_emit",
    positional: &["results"],
    keyword_only: &[],
    optional: &[],
};
const NATIVE_VALUE: Signature = Signature {
    function: "native_value",
    positional: &["operation", "arguments"],
    keyword_only: &[],
    optional: &[],
};
const NATIVE_LITE_CERTIFICATE_GATES: Signature = Signature {
    function: "native_lite_certificate_gates",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};
const MAIN: Signature = Signature {
    function: "main",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};

/// What one operation produced: its value or the exception it raised, and (for the operations that print) the
/// text it wrote to standard output, whichever way it ended.
pub struct Evaluation {
    pub result: Result<PyValue, Failure>,
    pub stdout: Option<PyStr>,
}

fn needs(context: Option<&Context>) -> Result<&Context, Failure> {
    context.ok_or_else(|| {
        Failure::new(
            "RuntimeError",
            &format!(
                "{SCRIPTS_DIR_ENV} is not set: the scripts directory of the checks is unknown"
            ),
        )
    })
}

fn silent(result: Result<PyValue, Failure>) -> Evaluation {
    Evaluation {
        result,
        stdout: None,
    }
}

/// One operation of the original module.
pub fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    context: Option<&Context>,
) -> Evaluation {
    match operation {
        "read" => silent((|| {
            let call = READ.bind(arguments)?;
            let name =
                path_argument(call.get_ref("name")).map_err(|raised| raised.into_failure())?;
            let text = read(needs(context)?, &name).map_err(|raised| raised.into_failure())?;
            Ok(text.map_or(PyValue::None, |text| PyValue::str(&text)))
        })()),
        "_emit" => {
            let mut console = Recorder::default();
            let result = (|| {
                let call = EMIT.bind(arguments)?;
                emit(call.get_ref("results"), &mut console)
                    .map(PyValue::Bool)
                    .map_err(|raised| raised.into_failure())
            })();
            Evaluation {
                result,
                stdout: Some(console.stdout),
            }
        }
        "native_value" => silent((|| {
            let call = NATIVE_VALUE.bind(arguments)?;
            native_value(
                needs(context)?,
                call.get_ref("operation"),
                call.get_ref("arguments"),
            )
            .map(PyValue::Str)
            .map_err(|raised| raised.into_failure())
        })()),
        "native_lite_certificate_gates" => silent((|| {
            NATIVE_LITE_CERTIFICATE_GATES.bind(arguments)?;
            native_lite_certificate_gates(needs(context)?)
                .map(PyValue::Bool)
                .map_err(|raised| raised.into_failure())
        })()),
        "main" => {
            let mut console = Recorder::default();
            let result = (|| {
                MAIN.bind(arguments)?;
                conformance_checks::main(needs(context)?, &mut console)
                    .map(PyValue::int)
                    .map_err(|raised| raised.into_failure())
            })();
            Evaluation {
                result,
                stdout: Some(console.stdout),
            }
        }
        other => silent(Err(Failure::new(
            "ValueError",
            &format!("unknown operation {}", PyStr::new(other).repr()),
        ))),
    }
}

/// `json.dumps(value, ensure_ascii=True, separators=(',', ':'))`: the form the expected values of the timing
/// harness are written in.
pub fn compact_json(value: &PyValue, out: &mut String) -> Result<(), String> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                compact_json(item, out)?;
            }
            out.push(']');
        }
        PyValue::Dict(map) => {
            out.push('{');
            for (index, (key, item)) in map.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(
                    &PyValue::Str(key.clone())
                        .dumps_ascii()
                        .map_err(|error| error.to_string())?,
                );
                out.push(':');
                compact_json(item, out)?;
            }
            out.push('}');
        }
        scalar => out.push_str(&scalar.dumps_ascii().map_err(|error| error.to_string())?),
    }
    Ok(())
}

fn row_operation(row: &PyDict) -> Result<String, Failure> {
    match row.get("operation") {
        Some(PyValue::Str(name)) => Ok(name.to_utf8().unwrap_or_default()),
        Some(other) => Ok(other.repr()),
        None => Err(Failure::new("KeyError", "'operation'")),
    }
}

fn is_printing(operation: &str) -> bool {
    matches!(operation, "_emit" | "main")
}

fn with_stdout(record: PyValue, stdout: Option<PyStr>) -> PyValue {
    match (record, stdout) {
        (PyValue::Dict(mut map), Some(text)) => {
            map.set("stdout", PyValue::Str(text));
            PyValue::Dict(map)
        }
        (record, _) => record,
    }
}

fn evaluate_row(row: &PyValue, context: Option<&Context>) -> PyValue {
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
    let prepared = row_operation(map)
        .and_then(|operation| row_arguments(map, true).map(|arguments| (operation, arguments)));
    let (operation, arguments) = match prepared {
        Ok(parts) => parts,
        Err(failure) => return exception_record(id, &failure, wire),
    };
    let evaluation = evaluate_operation(&operation, &arguments, context);
    let stdout = if is_printing(&operation) {
        evaluation.stdout
    } else {
        None
    };
    let record = match evaluation.result {
        Ok(value) => value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    };
    with_stdout(record, stdout)
}

fn run_jsonl(io: &mut Streams<'_>, context: Option<&Context>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!(
                "[conformance_full_superset_lite_api] ERROR stdin: {error}\n"
            )),
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
                &PyStr::new(&format!(
                    "[conformance_full_superset_lite_api] ERROR stdin: {error}\n"
                )),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads_objects(line) {
                Ok(row) => evaluate_row(&row, context),
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
