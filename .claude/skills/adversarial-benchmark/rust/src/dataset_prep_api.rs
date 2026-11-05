//! `dataset_prep_api`: the Rust-only surface of the dataset axis, a separate program so that the `dataset_prep` command
//! line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - every function of the original module as a transport. Each stdin line is
//!   `{id, operation, arguments}` where `operation` is a Python function name: `_load_spec`, `_dir_digest`,
//!   `column_mapper_arg`, `data_arg`, `target_rows`, `cmd_prepare`, `cmd_slice`, `cmd_synthetic_sidecar`, `main`,
//!   `_self_test`, `_die`. A line yields `{id, kind: value, value}` or `{id, kind: exception, type, message}`, plus
//!   `stderr` (and `stdout`) when the call wrote to those streams before returning or raising - the `SystemExit` of
//!   a refused input has its message there. Arguments follow the Python signatures (names, defaults and the
//!   `TypeError` a wrong call raises); `cmd_*` take the `argparse.Namespace` as an object of attributes; `_dir_digest`
//!   takes the folder as text (the wire form of a `pathlib.Path`). Tuples come back as JSON arrays. Paths are
//!   relative to the working directory and the files they name are read when the operation runs. A command that
//!   starts a child process lets it share this program's standard output.
//!   Opt-in: `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the
//!   result as compact ASCII JSON text in `value_json`).
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `native-help` - this list.

use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::dataset_commands::{cmd_prepare, cmd_slice, cmd_synthetic_sidecar, Namespace};
use crate::dataset_prep::{
    column_mapper_arg, data_arg, die, dir_digest, load_spec, target_rows, Console, Recorder,
    OUTPUT_COUNT_COL, PREP_META, PREP_NAME, PROMPT_COUNT_COL,
};
use crate::dataset_prep_cli::main_with;
use crate::dataset_prep_selftest::self_test;
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, row_arguments, Bound, Signature, Wire,
};

const NATIVE_HELP: &str = "dataset_prep_api Rust-only commands:\n  dataset_prep_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  dataset_prep_api constants        the module constants as one JSON object\n  dataset_prep_api native-help      this text\n";

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

/// The constants of the original module, from this crate.
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    map.set("PREP_NAME", PyValue::str(PREP_NAME));
    map.set("PREP_META", PyValue::str(PREP_META));
    map.set("PROMPT_COUNT_COL", PyValue::str(PROMPT_COUNT_COL));
    map.set("OUTPUT_COUNT_COL", PyValue::str(OUTPUT_COUNT_COL));
    PyValue::Dict(map)
}

const LOAD_SPEC: Signature = Signature {
    function: "_load_spec",
    positional: &["path"],
    keyword_only: &[],
    optional: &[],
};
const DIR_DIGEST: Signature = Signature {
    function: "_dir_digest",
    positional: &["d"],
    keyword_only: &[],
    optional: &[],
};
const COLUMN_MAPPER_ARG: Signature = Signature {
    function: "column_mapper_arg",
    positional: &["spec", "with_counts"],
    keyword_only: &[],
    optional: &[],
};
const DATA_ARG: Signature = Signature {
    function: "data_arg",
    positional: &["kind", "path"],
    keyword_only: &[],
    optional: &[],
};
const TARGET_ROWS: Signature = Signature {
    function: "target_rows",
    positional: &["rows", "input_len"],
    keyword_only: &[],
    optional: &[],
};
const COMMAND_PREPARE: Signature = Signature {
    function: "cmd_prepare",
    positional: &["a"],
    keyword_only: &[],
    optional: &[],
};
const COMMAND_SLICE: Signature = Signature {
    function: "cmd_slice",
    positional: &["a"],
    keyword_only: &[],
    optional: &[],
};
const COMMAND_SYNTHETIC_SIDECAR: Signature = Signature {
    function: "cmd_synthetic_sidecar",
    positional: &["a"],
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
const DIE: Signature = Signature {
    function: "_die",
    positional: &["rc", "msg"],
    keyword_only: &[],
    optional: &[],
};
const CONSTANTS: Signature = Signature {
    function: "constants",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};

/// `cmd(a)`: the attributes of the namespace; anything but an object has none of them.
fn with_namespace(
    call: &Bound<'_>,
    first_attribute: &str,
    command: impl FnOnce(&Namespace<'_>) -> Result<i32, Failure>,
) -> Result<PyValue, Failure> {
    match call.get_ref("a").peel() {
        PyValue::Dict(fields) => Ok(PyValue::int(command(&Namespace(fields))?)),
        other => Err(Failure::new(
            "AttributeError",
            &format!(
                "'{}' object has no attribute '{first_attribute}'",
                other.type_name()
            ),
        )),
    }
}

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
        "_load_spec" => {
            let call = LOAD_SPEC.bind(arguments)?;
            load_spec(call.get_ref("path"), streams)
        }
        "_dir_digest" => {
            let call = DIR_DIGEST.bind(arguments)?;
            Ok(PyValue::str(&dir_digest(call.get_ref("d"))?))
        }
        "column_mapper_arg" => {
            let call = COLUMN_MAPPER_ARG.bind(arguments)?;
            Ok(PyValue::Str(column_mapper_arg(
                call.get_ref("spec"),
                call.get_ref("with_counts"),
            )?))
        }
        "data_arg" => {
            let call = DATA_ARG.bind(arguments)?;
            Ok(PyValue::Str(data_arg(
                call.get_ref("kind"),
                call.get_ref("path"),
            )?))
        }
        "target_rows" => {
            let call = TARGET_ROWS.bind(arguments)?;
            Ok(target_rows(call.get_ref("rows"), call.get_ref("input_len"))?.to_value())
        }
        "cmd_prepare" => {
            let call = COMMAND_PREPARE.bind(arguments)?;
            with_namespace(&call, "spec", |namespace| cmd_prepare(namespace, streams))
        }
        "cmd_slice" => {
            let call = COMMAND_SLICE.bind(arguments)?;
            with_namespace(&call, "spec", |namespace| cmd_slice(namespace, streams))
        }
        "cmd_synthetic_sidecar" => {
            let call = COMMAND_SYNTHETIC_SIDECAR.bind(arguments)?;
            with_namespace(&call, "input_len", |namespace| {
                cmd_synthetic_sidecar(namespace, streams)
            })
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
        "_die" => {
            let call = DIE.bind(arguments)?;
            let message = call.get_ref("msg").display_text();
            let code = call.get_ref("rc");
            let failure = match code.peel() {
                PyValue::Int(number) => match i32::try_from(number.clone()) {
                    Ok(code) => die(streams, code, &message),
                    Err(_) => {
                        streams.error(&PyStr::new(&format!("[dataset_prep] {}\n", message.repr())));
                        Failure::new("SystemExit", &number.to_string())
                    }
                },
                other => {
                    let mut text = PyStr::new("[dataset_prep] ");
                    text.push_py(&message);
                    text.push_str("\n");
                    streams.error(&text);
                    Failure::with_message("SystemExit", other.display_text())
                }
            };
            Err(failure)
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
            &PyStr::new(&format!("[dataset_prep_api] ERROR stdin: {error}\n")),
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
                &PyStr::new(&format!("[dataset_prep_api] ERROR stdin: {error}\n")),
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
