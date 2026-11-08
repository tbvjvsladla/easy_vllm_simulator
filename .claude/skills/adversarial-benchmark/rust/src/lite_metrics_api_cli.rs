//! `lite_metrics_api`: the Rust-only surface of the lite metrics contract, a separate program so that the
//! `lite_metrics` command line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - every public and needed private operation of the original module as a differential and bridge
//!   transport. Each stdin line is `{id, operation, arguments}`; each yields one line `{id, kind: value, value}` or
//!   `{id, kind: exception, type, message}` (`SystemExit` is its own `type`). Operations: `gen_tps_from_bench`,
//!   `accept_len_from_bench`, `cold_ttft_ms`, `parse_engine_log`, `node_capacity`, `render_single`, `render_multi`,
//!   `build`, `judge_lite`, `read_lite_verdict`, `_load_json`, `_num`, `_requests_ok`, `_verdict` and `constants`.
//!   Arguments follow the Python signatures (names and the `TypeError` a wrong call raises); path arguments are
//!   relative to the process's working directory and the files they name are read when the operation runs.
//!   Opt-in: `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the result
//!   as compact ASCII JSON text in `value_json`), so the envelope never adds depth to a deep document. Rows and
//!   `arguments_json` texts are decoded with the `NaN`-identity decoder (`pyjson::loads_objects`); the operations
//!   treat that carrier as the number it is.
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `self-test` - native checks of the core operations and of the owner's D1 fixture (`LITE_METRICS_FIXTURE`).
//! * `native-help` - this list.

use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::guidellm::Failure;
use crate::lite_metrics::{
    accept_len_from_bench, build, cold_ttft_ms, gen_tps_from_bench, judge_lite, load_json,
    node_capacity, number_or_none, parse_engine_log, read_lite_verdict, render_multi,
    render_single, requests_ok, verdict_record, CONNECT_ERROR_PATTERN, GIB, INITIAL_TEST_FAILED,
    KV_PATTERNS, KV_TOKENS_PATTERN, LITE_EXIT_CODES, LITE_VERDICT_NAMES, NONTORCH_PATTERN,
    VERDICT_SOURCE, WEIGHTS_PATTERN,
};
use crate::lite_metrics_selftest::run_self_test;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, print_text, row_arguments, strings, value_record,
    Bound, Signature, Wire,
};

const NATIVE_HELP: &str = "lite_metrics_api Rust-only commands:\n  lite_metrics_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  lite_metrics_api constants        the module constants as one JSON object\n  lite_metrics_api self-test        native checks of the core operations and the owner's D1 fixture (LITE_METRICS_FIXTURE)\n  lite_metrics_api native-help      this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(io),
        Some("constants") if args.len() == 1 => {
            print_text(io, &constants().dumps_ascii().unwrap_or_default())
        }
        Some("self-test") if args.len() == 1 => {
            let report = run_self_test(std::env::var_os("LITE_METRICS_FIXTURE"));
            match io.print_lines(&PyStr::new(&report.stdout)) {
                Ok(()) => {
                    write_stderr(io, &PyStr::new(&report.stderr));
                    report.exit_code
                }
                Err(failure) => print_failed(io, &failure),
            }
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
    let mut exits = PyDict::new();
    for (name, code) in LITE_EXIT_CODES {
        exits.set(name, PyValue::int(code));
    }
    let mut map = PyDict::new();
    map.set("GIB", PyValue::Float(GIB));
    map.set("LITE_VERDICTS", strings(&LITE_VERDICT_NAMES));
    map.set("LITE_EXIT", PyValue::Dict(exits));
    map.set("_KV_PATTERNS", strings(&KV_PATTERNS));
    map.set("_KV_TOKENS_PAT", PyValue::str(KV_TOKENS_PATTERN));
    map.set("_WEIGHTS_PAT", PyValue::str(WEIGHTS_PATTERN));
    map.set("_NONTORCH_PAT", PyValue::str(NONTORCH_PATTERN));
    map.set("_INITIAL_TEST_FAILED", PyValue::str(INITIAL_TEST_FAILED));
    map.set("_CONNECT_ERROR_RE", PyValue::str(CONNECT_ERROR_PATTERN));
    map.set("VERDICT_SOURCE", PyValue::str(VERDICT_SOURCE));
    PyValue::Dict(map)
}

// ── Python call binding ─────────────────────────────────────────────────────────

const fn signature(function: &'static str, positional: &'static [&'static str]) -> Signature {
    Signature {
        function,
        positional,
        keyword_only: &[],
        optional: &[],
    }
}

const GEN_TPS: Signature = signature("gen_tps_from_bench", &["bench"]);
const ACCEPT_LEN: Signature = signature("accept_len_from_bench", &["bench", "leg"]);
const COLD_TTFT: Signature = signature("cold_ttft_ms", &["bench_cold"]);
const PARSE_ENGINE_LOG: Signature = signature("parse_engine_log", &["path"]);
const NODE_CAPACITY: Signature = signature("node_capacity", &["node", "engine", "is_master"]);
const RENDER_SINGLE: Signature = signature(
    "render_single",
    &["gen", "gen_src", "ttft", "cap", "kv_gib", "gpu_total_gib"],
);
const RENDER_MULTI: Signature = signature("render_multi", &["gen", "ttft", "caps", "kv_gib"]);
const BUILD: Signature = signature("build", &["raw"]);
const JUDGE_LITE: Signature = signature("judge_lite", &["raw", "built"]);
const READ_LITE_VERDICT: Signature = signature("read_lite_verdict", &["raw"]);
const LOAD_JSON: Signature = signature("_load_json", &["path"]);
const NUM: Signature = signature("_num", &["v"]);
const REQUESTS_OK: Signature = signature("_requests_ok", &["bench", "expected"]);
const VERDICT: Signature = signature("_verdict", &["v", "reasons"]);
const CONSTANTS: Signature = signature("constants", &[]);

fn pair(first: PyValue, second: PyValue) -> PyValue {
    PyValue::Tuple(vec![first, second])
}

/// One operation of the module by name, with the arguments as a Python call gives them.
pub fn evaluate_operation(operation: &str, arguments: &PyDict) -> Result<PyValue, Failure> {
    match operation {
        "gen_tps_from_bench" => {
            let call: Bound<'_> = GEN_TPS.bind(arguments)?;
            Ok(match gen_tps_from_bench(&call.get("bench"))? {
                Some(gen) => pair(
                    PyValue::Float(gen.tokens_per_second),
                    PyValue::str(gen.source.label()),
                ),
                None => pair(PyValue::None, PyValue::None),
            })
        }
        "accept_len_from_bench" => {
            let call = ACCEPT_LEN.bind(arguments)?;
            Ok(
                match accept_len_from_bench(&call.get("bench"), &call.get("leg"))? {
                    Some(accept) => pair(accept.value, PyValue::Str(accept.source)),
                    None => pair(PyValue::None, PyValue::None),
                },
            )
        }
        "cold_ttft_ms" => {
            let call = COLD_TTFT.bind(arguments)?;
            Ok(cold_ttft_ms(&call.get("bench_cold"))?.map_or(PyValue::None, PyValue::Float))
        }
        "parse_engine_log" => {
            let call = PARSE_ENGINE_LOG.bind(arguments)?;
            Ok(parse_engine_log(&call.get("path"))?.to_value())
        }
        "node_capacity" => {
            let call = NODE_CAPACITY.bind(arguments)?;
            Ok(node_capacity(
                &call.get("node"),
                &call.get("engine"),
                &call.get("is_master"),
            )?
            .to_value())
        }
        "render_single" => {
            let call = RENDER_SINGLE.bind(arguments)?;
            Ok(PyValue::Str(render_single(
                &call.get("gen"),
                &call.get("cap"),
                &call.get("ttft"),
                &call.get("kv_gib"),
                &call.get("gpu_total_gib"),
            )?))
        }
        "render_multi" => {
            let call = RENDER_MULTI.bind(arguments)?;
            Ok(PyValue::Str(render_multi(
                &call.get("gen"),
                &call.get("ttft"),
                &call.get("caps"),
                &call.get("kv_gib"),
            )?))
        }
        "build" => {
            let call = BUILD.bind(arguments)?;
            Ok(build(&call.get("raw"))?.to_value())
        }
        "judge_lite" => {
            let call = JUDGE_LITE.bind(arguments)?;
            Ok(judge_lite(&call.get("raw"), &call.get("built"))?.to_value())
        }
        "read_lite_verdict" => {
            let call = READ_LITE_VERDICT.bind(arguments)?;
            Ok(read_lite_verdict(&call.get("raw"))
                .map_or(PyValue::None, |verdict| PyValue::str(verdict.label())))
        }
        "_load_json" => {
            let call = LOAD_JSON.bind(arguments)?;
            Ok(load_json(&call.get("path"))?.unwrap_or(PyValue::None))
        }
        "_num" => {
            let call = NUM.bind(arguments)?;
            Ok(number_or_none(&call.get("v"))?.map_or(PyValue::None, PyValue::Float))
        }
        "_requests_ok" => {
            let call = REQUESTS_OK.bind(arguments)?;
            Ok(requests_ok(&call.get("bench"), &call.get("expected"))?.to_value())
        }
        "_verdict" => {
            let call = VERDICT.bind(arguments)?;
            verdict_record(&call.get("v"), &call.get("reasons"))
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

// ── evaluate-jsonl ──────────────────────────────────────────────────────────────

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
    let outcome = (|| {
        let operation = match map.get("operation") {
            Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
            Some(other) => other.repr(),
            None => return Err(Failure::new("KeyError", "'operation'")),
        };
        evaluate_operation(&operation, &row_arguments(map, true)?)
    })();
    match outcome {
        Ok(value) => value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    }
}

fn run_jsonl(io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[lite_metrics_api] ERROR stdin: {error}\n")),
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
                &PyStr::new(&format!("[lite_metrics_api] ERROR stdin: {error}\n")),
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
