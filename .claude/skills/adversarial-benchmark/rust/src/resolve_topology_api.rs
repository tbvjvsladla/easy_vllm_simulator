//! `resolve_topology_api`: the Rust-only surface of the topology resolver, a separate program so that the
//! `resolve_topology` command line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - the functions of the original module as a transport. Each stdin line is
//!   `{id, operation, arguments}` where `operation` is a Python function name: `_manifests`,
//!   `_manifest_topology_line`, `_run`, `resolve`. A line yields `{id, kind: value, value}` or
//!   `{id, kind: exception, type, message}`. Arguments follow the Python signatures (names, defaults and the
//!   `TypeError` a wrong call raises). Tuples come back as JSON arrays (`_run` is `[code, stdout, stderr]`, `resolve`
//!   is `[rc, topology, why]`). `resolve` runs the checkout's real parity owner as a process; the interpreter is the
//!   `python` argument when it is truthy, else `RESOLVE_TOPOLOGY_PYTHON` (default `python3`). Opt-in:
//!   `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the result as
//!   compact ASCII JSON text in `value_json`). There is no network access and no dispatch to any other name.
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `native-help` - this list.

use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, row_arguments, Signature, Wire,
};
use crate::resolve_topology::{
    manifest_topology_line, manifests, resolve, run as run_command, ResolveContext, DEFAULT_PYTHON,
    EXIT_OK, EXIT_UNRESOLVED, EXIT_UNTERRAFORMED, EXIT_USAGE, PARITY_REL, PYTHON_ENV, TAG,
};

const NATIVE_HELP: &str = "resolve_topology_api Rust-only commands:\n  resolve_topology_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  resolve_topology_api constants        the module constants as one JSON object\n  resolve_topology_api native-help      this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_with(args, &ResolveContext::from_environment(), io)
}

/// [`run`] with an explicit context (the interpreter of the owner when a call names none).
pub fn run_with(args: &[PyStr], context: &ResolveContext, io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(io, context),
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

/// The constants of the original module, from this crate (the two environment names are this port's).
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    map.set("EXIT_OK", PyValue::int(EXIT_OK));
    map.set("EXIT_USAGE", PyValue::int(EXIT_USAGE));
    map.set("EXIT_UNTERRAFORMED", PyValue::int(EXIT_UNTERRAFORMED));
    map.set("EXIT_UNRESOLVED", PyValue::int(EXIT_UNRESOLVED));
    map.set("PARITY_REL", PyValue::str(PARITY_REL));
    map.set("TAG", PyValue::str(TAG));
    map.set("PYTHON_ENV", PyValue::str(PYTHON_ENV));
    map.set("DEFAULT_PYTHON", PyValue::str(DEFAULT_PYTHON));
    PyValue::Dict(map)
}

const MANIFESTS: Signature = Signature {
    function: "_manifests",
    positional: &["repo"],
    keyword_only: &[],
    optional: &[],
};
const MANIFEST_TOPOLOGY_LINE: Signature = Signature {
    function: "_manifest_topology_line",
    positional: &["path"],
    keyword_only: &[],
    optional: &[],
};
const RUN: Signature = Signature {
    function: "_run",
    positional: &["argv", "timeout"],
    keyword_only: &[],
    optional: &["timeout"],
};
const RESOLVE: Signature = Signature {
    function: "resolve",
    positional: &["repo", "python"],
    keyword_only: &[],
    optional: &["python"],
};
const CONSTANTS: Signature = Signature {
    function: "constants",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};

/// The default of `_run`'s `timeout` parameter.
const DEFAULT_TIMEOUT_SECONDS: i64 = 60;

/// One operation of the original module.
pub fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    context: &ResolveContext,
) -> Result<PyValue, Failure> {
    match operation {
        "_manifests" => {
            let call = MANIFESTS.bind(arguments)?;
            let found = manifests(call.get_ref("repo"))?;
            Ok(PyValue::List(found.into_iter().map(PyValue::Str).collect()))
        }
        "_manifest_topology_line" => {
            let call = MANIFEST_TOPOLOGY_LINE.bind(arguments)?;
            Ok(manifest_topology_line(call.get_ref("path"))?.map_or(PyValue::None, PyValue::Str))
        }
        "_run" => {
            let call = RUN.bind(arguments)?;
            let default = PyValue::int(DEFAULT_TIMEOUT_SECONDS);
            let timeout = call.get_or_ref("timeout", &default);
            Ok(run_command(call.get_ref("argv"), timeout)?.to_value())
        }
        "resolve" => {
            let call = RESOLVE.bind(arguments)?;
            Ok(resolve(call.get_ref("repo"), call.get_ref("python"), context)?.to_value())
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

fn evaluate_row(row: &PyValue, context: &ResolveContext) -> PyValue {
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
    let outcome = (|| {
        let operation = row_operation(map)?;
        evaluate_operation(&operation, &row_arguments(map, true)?, context)
    })();
    match outcome {
        Ok(value) => crate::repeat_api_cli::value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    }
}

fn run_jsonl(io: &mut Streams<'_>, context: &ResolveContext) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[resolve_topology_api] ERROR stdin: {error}\n")),
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
                &PyStr::new(&format!("[resolve_topology_api] ERROR stdin: {error}\n")),
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
