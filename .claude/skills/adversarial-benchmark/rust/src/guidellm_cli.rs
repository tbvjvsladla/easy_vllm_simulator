//! Command line of `parse_guidellm`.
//!
//! The legacy surface is the original program's argparse interface ([`crate::argv::PARSE_GUIDELLM`]): usage, help
//! and argument-error text is byte-for-byte the original's, including the logical program name
//! `parse_guidellm.py`; `--json` is accepted and unused (the output is always JSON); `--self-test` runs the
//! original's G1..G29 scenarios against the Rust library.
//!
//! Rust-only additions are positional words, which the original rejected, so they cannot collide with a legacy
//! invocation (`native-help` lists them):
//!
//! * `evaluate-jsonl` - the API as a differential transport: `{id, operation, arguments}` lines in,
//!   `{id, kind: value|exception, ...}` lines out (operations `build`, `inherit_accept_len`, `classify_errors`);
//! * `spec-source --lite-raw PATH` - the inline Python body of `sweep_bench.sh`'s `_spec_axis_args`: prints the
//!   warm-JSON path a lite raw document points at when that document can be inherited from, otherwise
//!   `Class: message` on stderr and status 1;
//! * `bench-tool` - the one `BENCH_TOOL` token this parser writes into its output.
//!
//! Failure timing follows the original `main`: argument checks first, then the inputs in a fixed order
//! (`--accept-len-src`, the engine log, `--post-health-json`, `--benchmarks-json`, `--prefix-cache-json`). A file
//! that cannot be read or parsed as JSON, a refused spec source or a `ValueError` of `build` end with
//! `[parse_guidellm] ERROR ...` and status 2. Any other exception (including `RecursionError` of a too deeply
//! nested document and the `ValueError` of a malformed engine-log number) ends the program with status 1 and a
//! traceback, of which only the text before it and the final `Class: message` line are reproduced; warnings
//! already written stay. Engine-log read problems are warnings.

use crate::argv::{self, guidellm_args as arg, Outcome, PARSE_GUIDELLM};
use crate::cli::{
    on_large_stack, os_error_text, write_stderr, PrintFailure, Streams, TRACEBACK_HEADER,
};
use crate::engine_log::{decode_replacing, generation_throughputs};
use crate::guidellm::{
    build, carry_prefix_cache, classify_errors, inherit_accept_len, BuildRequest, Failure,
    SpecAxis, BENCH_TOOL,
};
use crate::guidellm_selftest::run_self_test;
use crate::pyjson::{decode_file_text, loads, loads_text, LoadError};
use crate::pynumber::{float_from_str, round_to_two_places};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::terminal;
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use std::os::unix::ffi::OsStringExt;

const NATIVE_HELP: &str = "parse_guidellm Rust-only commands (the legacy options are unchanged; see -h):\n  parse_guidellm evaluate-jsonl                JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  parse_guidellm spec-source --lite-raw PATH   the _spec_axis_args helper of sweep_bench.sh (prints the warm JSON path)\n  parse_guidellm bench-tool                    the BENCH_TOOL token written into every measurement\n  parse_guidellm native-help                   this text\n";
const SPEC_SOURCE_USAGE: &str = "usage: parse_guidellm spec-source --lite-raw PATH\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, io);
    io.finish(status)
}

fn dispatch(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let first = args.first().and_then(PyStr::to_utf8);
    match (first.as_deref(), args.len()) {
        (Some("evaluate-jsonl"), 1) => run_jsonl(io),
        (Some("spec-source"), _) => run_spec_source(&args[1..], io),
        (Some("bench-tool"), 1) => print_text(io, BENCH_TOOL),
        (Some("native-help"), 1) => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, &PyStr::default()),
        },
        _ => run_legacy(args, io),
    }
}

fn print_text(io: &mut Streams<'_>, text: &str) -> i32 {
    match io.print(&PyStr::new(text)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure, &PyStr::default()),
    }
}

/// The program dies on a failed `print`: the warnings already written, the traceback, status 1.
fn print_failed(io: &mut Streams<'_>, failure: &PrintFailure, stderr_so_far: &PyStr) -> i32 {
    let mut text = stderr_so_far.clone();
    text.push_str(TRACEBACK_HEADER);
    text.push_str(&failure.line());
    text.push_str("\n");
    write_stderr(io, &text);
    1
}

// ── how a run ends early ────────────────────────────────────────────────────────

/// Why the program stops before it prints its result.
pub(crate) enum Termination {
    /// A diagnosed problem: the line to write to stderr, status 2.
    Handled(PyStr),
    /// An exception no handler of the original catches: traceback, status 1.
    Uncaught(Failure),
}

impl From<Failure> for Termination {
    fn from(failure: Failure) -> Self {
        Termination::Uncaught(failure)
    }
}

fn error_line(label: &str, detail: &PyStr) -> PyStr {
    let mut text = PyStr::new(&format!("[parse_guidellm] ERROR {label}: "));
    text.push_py(detail);
    text.push_str("\n");
    text
}

/// `ValueError`s of the library become diagnosed errors (status 2); everything else escapes.
fn diagnosed(failure: Failure) -> Termination {
    if failure.is_value_error() {
        let mut text = PyStr::new("[parse_guidellm] ERROR ");
        text.push_py(&failure.message);
        text.push_str("\n");
        Termination::Handled(text)
    } else {
        Termination::Uncaught(failure)
    }
}

/// The exception `json.load` raises for a document, as the class the original would show.
fn load_failure(error: &LoadError) -> Failure {
    match error {
        LoadError::TooDeep { .. } => {
            let text = error.to_string();
            Failure::new(
                "RecursionError",
                text.strip_prefix("RecursionError: ").unwrap_or(&text),
            )
        }
        LoadError::Decode { .. } => Failure::new("JSONDecodeError", &error.to_string()),
        LoadError::Encoding(_) => Failure::new("UnicodeDecodeError", &error.to_string()),
        LoadError::IntegerTooLong { .. } => Failure::new("ValueError", &error.to_string()),
    }
}

/// The `OSError` subclass Python raises for an operating-system error number (Linux numbering).
fn os_error_class(error: &std::io::Error) -> &'static str {
    match error.raw_os_error() {
        Some(1 | 13) => "PermissionError",
        Some(2) => "FileNotFoundError",
        Some(3) => "ProcessLookupError",
        Some(4) => "InterruptedError",
        Some(10) => "ChildProcessError",
        Some(11) => "BlockingIOError",
        Some(17) => "FileExistsError",
        Some(20) => "NotADirectoryError",
        Some(21) => "IsADirectoryError",
        Some(32 | 108) => "BrokenPipeError",
        Some(103) => "ConnectionAbortedError",
        Some(104) => "ConnectionResetError",
        Some(110) => "TimeoutError",
        Some(111) => "ConnectionRefusedError",
        _ => "OSError",
    }
}

/// `open(path, ...).read()`: the bytes, or the `OSError` class and `str(exception)`.
fn read_file(path: &PyStr) -> Result<Vec<u8>, (&'static str, String)> {
    let bytes = path
        .to_surrogateescape_bytes()
        .map_err(|failure| ("UnicodeEncodeError", failure.message()))?;
    std::fs::read(std::ffi::OsString::from_vec(bytes))
        .map_err(|error| (os_error_class(&error), os_error_text(&error, path)))
}

/// `_load`: the JSON document of a file, or the reason the original stops (status 2 with the label).
fn load_json(path: &PyStr, label: &str) -> Result<PyValue, Termination> {
    let bytes = read_file(path)
        .map_err(|(_, message)| Termination::Handled(error_line(label, &PyStr::new(&message))))?;
    let parsed = decode_file_text(&bytes).and_then(|text| loads_text(&text));
    parsed.map_err(|error| {
        if error.is_value_error() {
            Termination::Handled(error_line(label, &PyStr::new(&error.to_string())))
        } else {
            Termination::Uncaught(load_failure(&error))
        }
    })
}

// ── engine log ──────────────────────────────────────────────────────────────────

/// `_engine_max`: the highest `generation throughput` figure of the log, `round(max, 2)`.
///
/// Only `OSError` is handled (a warning); a figure that is not a number (the pattern also matches a lone comma)
/// ends the program, unlike `parse_bench`.
fn engine_max(path: &PyStr, stderr: &mut PyStr) -> Result<Option<f64>, Termination> {
    let bytes = match read_file(path) {
        Ok(bytes) => bytes,
        Err((_, message)) => {
            stderr.push_str(&format!("[parse_guidellm] WARN engine-log: {message}\n"));
            return Ok(None);
        }
    };
    let text = decode_replacing(&bytes);
    let mut best: Option<f64> = None;
    for found in generation_throughputs(&text) {
        let number = float_from_str(&PyStr::new(&found.replace(',', "")))
            .map_err(|error| Termination::Uncaught(error.into()))?;
        if best.is_none_or(|current| number > current) {
            best = Some(number);
        }
    }
    Ok(best.map(round_to_two_places))
}

/// `prefix_cache_run`: the observation of the run's prefix-cache counters, carried into the output.
pub(crate) fn prefix_cache_run(
    path: Option<&PyStr>,
    stderr: &mut PyStr,
) -> Result<PyValue, Termination> {
    let Some(path) = path.filter(|path| !path.is_empty()) else {
        return Ok(PyValue::None);
    };
    let observation = load_json(path, "--prefix-cache-json")?;
    let carry = carry_prefix_cache(&observation)?;
    if let Some(warning) = carry.warning {
        stderr.push_py(&warning);
    }
    Ok(carry.record)
}

// ── legacy interface ────────────────────────────────────────────────────────────

/// The parsed, truthiness-normalised options (an empty path counts as not given).
struct Options {
    benchmarks_json: PyStr,
    engine_log: Option<PyStr>,
    benchmark_index: BigInt,
    accept_len_src: Option<PyStr>,
    post_health_json: Option<PyStr>,
    prefix_cache_json: Option<PyStr>,
    max_error_rate: f64,
}

/// What the heavy part of a run hands back to the thread that owns the streams.
struct Finished {
    exit_code: i32,
    stdout: Option<PyStr>,
    stderr: PyStr,
}

const SPEC_AXIS_REQUIRED: &str = "[parse_guidellm] ERROR spec 축을 정하라 — --accept-len-src(lite 레그 승계) 또는 --spec-axis-absent(부재 명시) 중 **정확히 하나**.\n  GuideLLM 은 수용길이를 보고하지 않는데 verdict_rule 은 spec_on 으로 물리 상한을\n  R_token/R_fp 중에서 고른다. 조용히 False 를 적으면 spec 이 켜진 모델의 상한이 낮게\n  잡혀 거짓 판정이 된다 — 부재와 결측을 가른다.\n";

fn run_legacy(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match argv::parse_with(&PARSE_GUIDELLM, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            io.print_help(PARSE_GUIDELLM.help(terminal::columns()));
            return 0;
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{}: error: ",
                PARSE_GUIDELLM.usage(terminal::columns()),
                PARSE_GUIDELLM.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
    };
    if matches.flag(arg::SELF_TEST) {
        let report = run_self_test();
        if let Err(failure) = io.print_lines(&PyStr::new(&report.stdout)) {
            return print_failed(io, &failure, &PyStr::default());
        }
        write_stderr(io, &PyStr::new(&report.stderr));
        return report.exit_code;
    }
    let given = |action: usize| {
        matches
            .value(action)
            .filter(|path| !path.is_empty())
            .cloned()
    };
    let Some(benchmarks_json) = given(arg::BENCHMARKS_JSON) else {
        write_stderr(
            io,
            &PyStr::new("[parse_guidellm] ERROR --benchmarks-json 은 필수다\n"),
        );
        return 2;
    };
    let accept_len_src = given(arg::ACCEPT_LEN_SRC);
    if accept_len_src.is_some() == matches.flag(arg::SPEC_AXIS_ABSENT) {
        write_stderr(io, &PyStr::new(SPEC_AXIS_REQUIRED));
        return 2;
    }
    let options = Options {
        benchmarks_json,
        engine_log: given(arg::ENGINE_LOG),
        benchmark_index: matches
            .int(arg::BENCHMARK_INDEX)
            .cloned()
            .unwrap_or_else(BigInt::zero),
        accept_len_src,
        post_health_json: given(arg::POST_HEALTH_JSON),
        prefix_cache_json: given(arg::PREFIX_CACHE_JSON),
        max_error_rate: matches.float(arg::MAX_ERROR_RATE).unwrap_or(0.0),
    };
    let finished = on_large_stack(move || execute(&options));
    if let Some(output) = &finished.stdout {
        if let Err(failure) = io.print_body(output) {
            return print_failed(io, &failure, &finished.stderr);
        }
    }
    write_stderr(io, &finished.stderr);
    finished.exit_code
}

/// Everything of the original's `main` after the argument checks.
fn execute(options: &Options) -> Finished {
    let mut stderr = PyStr::default();
    match produce(options, &mut stderr) {
        Ok(output) => Finished {
            exit_code: 0,
            stdout: Some(output),
            stderr,
        },
        Err(Termination::Handled(line)) => {
            stderr.push_py(&line);
            Finished {
                exit_code: 2,
                stdout: None,
                stderr,
            }
        }
        Err(Termination::Uncaught(failure)) => {
            stderr.push_str(TRACEBACK_HEADER);
            stderr.push_py(&failure.line());
            stderr.push_str("\n");
            Finished {
                exit_code: 1,
                stdout: None,
                stderr,
            }
        }
    }
}

fn produce(options: &Options, stderr: &mut PyStr) -> Result<PyStr, Termination> {
    let mut spec = SpecAxis::declared_absent();
    if let Some(path) = &options.accept_len_src {
        let source = load_json(path, "--accept-len-src")?;
        spec = inherit_accept_len(&source, &PyValue::Str(path.clone())).map_err(diagnosed)?;
    }
    let engine_max = match &options.engine_log {
        Some(path) => engine_max(path, stderr)?,
        None => None,
    };
    // The server's liveness at the end of the bench is read only from an observation file: unknown is `None`,
    // never folded into `False` or `True`.
    let mut server_alive_at_bench_end = None;
    if let Some(path) = &options.post_health_json {
        let health = load_json(path, "--post-health-json")?;
        let value = match &health {
            PyValue::Dict(map) => map.get("server_alive_at_bench_end"),
            _ => None,
        };
        match value {
            Some(PyValue::Bool(alive)) => server_alive_at_bench_end = Some(*alive),
            other => {
                let shown = other.map_or_else(|| "None".to_string(), PyValue::repr);
                return Err(Termination::Handled(PyStr::new(&format!(
                    "[parse_guidellm] ERROR --post-health-json 에 server_alive_at_bench_end(boolean) 가 없다: {shown}\n"
                ))));
            }
        }
    }
    let document = load_json(&options.benchmarks_json, "--benchmarks-json")?;
    let request = BuildRequest {
        benchmark_index: PyValue::Int(options.benchmark_index.clone()),
        engine_max: engine_max.map_or(PyValue::None, PyValue::Float),
        spec: spec.to_value(),
        max_error_rate: PyValue::Float(options.max_error_rate),
        server_alive_at_bench_end: server_alive_at_bench_end.map_or(PyValue::None, PyValue::Bool),
    };
    let PyValue::Dict(mut measurement) = build(&document, &request).map_err(diagnosed)? else {
        unreachable!("build returns an object")
    };
    let prefix = prefix_cache_run(options.prefix_cache_json.as_ref(), stderr)?;
    measurement.set("prefix_cache_run", prefix);
    let mut json = PyValue::Dict(measurement)
        .dumps_pretty()
        .map_err(|error| Failure::new(error.python_type(), &error.to_string()))?;
    json.push_str("\n");
    Ok(json)
}

// ── evaluate-jsonl ──────────────────────────────────────────────────────────────

/// The wire encoding of the opt-in JSON transport (`result_json`); a bridge checks it to fail loudly against a
/// native binary that predates the transport.
const JSON_VALUES_ENCODING: &str = "json-values-v1";

/// How a row's result is put on the wire.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wire {
    /// `{id, kind: value, value}` / `{id, kind: exception, type, message}`: the result is a nested JSON value.
    Values,
    /// `result_json: true`: `{id, encoding, kind: value, value_json}`: the whole result as compact ASCII JSON text,
    /// so the record around it never adds depth to the result.
    JsonText,
}

fn exception_record(id: PyValue, failure: &Failure, wire: Wire) -> PyValue {
    let mut map = PyDict::new();
    map.set("id", id);
    if wire == Wire::JsonText {
        map.set("encoding", PyValue::str(JSON_VALUES_ENCODING));
    }
    map.set("kind", PyValue::str("exception"));
    map.set("type", PyValue::str(failure.class));
    map.set("message", PyValue::Str(failure.message.clone()));
    PyValue::Dict(map)
}

fn value_record(id: PyValue, value: PyValue, wire: Wire) -> PyValue {
    let mut map = PyDict::new();
    map.set("id", id.clone());
    match wire {
        Wire::Values => {
            map.set("kind", PyValue::str("value"));
            map.set("value", value);
        }
        Wire::JsonText => match value.dumps_ascii() {
            Ok(text) => {
                map.set("encoding", PyValue::str(JSON_VALUES_ENCODING));
                map.set("kind", PyValue::str("value"));
                map.set("value_json", PyValue::str(&text));
            }
            Err(error) => {
                return exception_record(
                    id,
                    &Failure::new(error.python_type(), &error.to_string()),
                    wire,
                )
            }
        },
    }
    PyValue::Dict(map)
}

fn transport_error(message: &str) -> Failure {
    Failure::new("TypeError", message)
}

fn key_error(name: &str) -> Failure {
    Failure::with_message("KeyError", PyStr::new(&PyStr::new(name).repr()))
}

/// The arguments of one operation row, as the Python function would receive them. Nothing is checked here: a
/// value of any JSON type reaches the library, which uses it, stores it or fails at the operation that needs it
/// (and an absent one is the original signature's default).
struct Arguments(PyDict);

impl Arguments {
    fn required(&self, name: &str) -> Result<&PyValue, Failure> {
        self.0.get(name).ok_or_else(|| key_error(name))
    }

    fn optional(&self, name: &str, default: PyValue) -> PyValue {
        self.0.get(name).cloned().unwrap_or(default)
    }
}

fn evaluate_operation(operation: &str, arguments: &Arguments) -> Result<PyValue, Failure> {
    let defaults = BuildRequest::default();
    match operation {
        "build" => {
            let request = BuildRequest {
                benchmark_index: arguments.optional("benchmark_index", defaults.benchmark_index),
                engine_max: arguments.optional("engine_max", defaults.engine_max),
                spec: arguments.optional("spec", defaults.spec),
                max_error_rate: arguments.optional("max_error_rate", defaults.max_error_rate),
                server_alive_at_bench_end: arguments.optional(
                    "server_alive_at_bench_end",
                    defaults.server_alive_at_bench_end,
                ),
            };
            build(arguments.required("doc")?, &request)
        }
        "inherit_accept_len" => {
            let spec = inherit_accept_len(arguments.required("src")?, arguments.required("path")?)?;
            let mut out = PyDict::with_capacity(2);
            out.set("accept_len", spec.accept_len);
            out.set("source", spec.source);
            Ok(PyValue::Dict(out))
        }
        "classify_errors" => {
            let split = classify_errors(
                arguments.required("run")?,
                &arguments.optional("benchmark_index", defaults.benchmark_index),
                &arguments.optional(
                    "server_alive_at_bench_end",
                    defaults.server_alive_at_bench_end,
                ),
            )?;
            Ok(split.map_or(PyValue::None, |split| split.to_value()))
        }
        other => Err(Failure::new(
            "ValueError",
            &format!("unknown operation {}", PyStr::new(other).repr()),
        )),
    }
}

/// The arguments of a row: `arguments` (nested JSON, the standard form) or `arguments_json` (each argument a raw
/// JSON text decoded on its own, so a document near the nesting limit is not nested one level deeper by the
/// request around it). The two are mutually exclusive.
fn row_arguments(row: &PyDict) -> Result<Arguments, Failure> {
    match (row.get("arguments"), row.get("arguments_json")) {
        (Some(_), Some(_)) => Err(transport_error(
            "arguments and arguments_json are mutually exclusive",
        )),
        (Some(PyValue::Dict(arguments)), None) => Ok(Arguments(arguments.clone())),
        (Some(other), None) => Err(transport_error(&format!(
            "arguments must be an object, got {}",
            other.type_name()
        ))),
        (None, Some(PyValue::Dict(texts))) => {
            let mut decoded = PyDict::new();
            for (name, text) in texts.iter() {
                let PyValue::Str(text) = text else {
                    return Err(transport_error(&format!(
                        "arguments_json values must be JSON texts, got {}",
                        text.type_name()
                    )));
                };
                let value = loads_text(text).map_err(|error| load_failure(&error))?;
                decoded.insert(name.clone(), value);
            }
            Ok(Arguments(decoded))
        }
        (None, Some(other)) => Err(transport_error(&format!(
            "arguments_json must be an object, got {}",
            other.type_name()
        ))),
        (None, None) => Err(key_error("arguments")),
    }
}

fn evaluate_row(row: &PyValue) -> PyValue {
    let PyValue::Dict(map) = row else {
        return exception_record(
            PyValue::None,
            &transport_error("operation row is not an object"),
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
            None => return Err(key_error("operation")),
        };
        evaluate_operation(&operation, &row_arguments(map)?)
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
            &PyStr::new(&format!("[parse_guidellm] ERROR stdin: {error}\n")),
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
                &PyStr::new(&format!("[parse_guidellm] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads(line) {
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
                    out.push_str(
                        &exception_record(
                            id,
                            &Failure::new(error.python_type(), &error.to_string()),
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
        Err(failure) => print_failed(io, &failure, &PyStr::default()),
    }
}

// ── spec-source ─────────────────────────────────────────────────────────────────

/// What a lite raw document's `bench_warm_json` names to `os.stat` and `open`: a path, or - for a JSON integer
/// or boolean - an open file descriptor of this process (`os.stat(3)` and `open(3)` take the number as one).
enum WarmTarget {
    Path(PyStr),
    Descriptor(i32),
}

impl WarmTarget {
    /// `os.stat`'s conversion of the value: a string is a path, an integer is a descriptor number (`bool` is an
    /// integer), anything else is a `TypeError`, and a number outside a C `int` is an `OverflowError`; neither
    /// is caught by `os.path.isfile`.
    fn of(warm: &PyValue) -> Result<Self, Failure> {
        let number = match warm {
            PyValue::Str(path) => return Ok(WarmTarget::Path(path.clone())),
            PyValue::Int(number) => number.clone(),
            PyValue::Bool(flag) => BigInt::from(u8::from(*flag)),
            other => {
                return Err(Failure::new(
                    "TypeError",
                    &format!(
                        "stat: path should be string, bytes, os.PathLike or integer, not {}",
                        other.type_name()
                    ),
                ))
            }
        };
        match number.to_i32() {
            Some(descriptor) => Ok(WarmTarget::Descriptor(descriptor)),
            None if number > BigInt::zero() => {
                Err(Failure::new("OverflowError", "fd is greater than maximum"))
            }
            None => Err(Failure::new("OverflowError", "fd is less than minimum")),
        }
    }

    /// `os.stat(target)`, `None` where it raises `OSError`/`ValueError` (which `os.path.isfile` turns into
    /// `False`). A descriptor is stat-ed through the process's own descriptor table (`/proc/self/fd/N` is the
    /// kernel's `fstat` of that descriptor); a descriptor that is not open, or a negative number, is `None`.
    fn status(&self) -> Option<std::fs::Metadata> {
        match self {
            WarmTarget::Path(path) => {
                let bytes = path.to_surrogateescape_bytes().ok()?;
                std::fs::metadata(std::ffi::OsString::from_vec(bytes)).ok()
            }
            WarmTarget::Descriptor(number) if *number >= 0 => {
                std::fs::metadata(format!("/proc/self/fd/{number}")).ok()
            }
            WarmTarget::Descriptor(_) => None,
        }
    }

    /// `os.path.isfile(target) and os.path.getsize(target) != 0`.
    fn is_usable(&self) -> bool {
        self.status()
            .is_some_and(|data| data.is_file() && data.len() > 0)
    }

    /// `json.load(open(target, encoding="utf-8"))`'s read: the bytes, or the `OSError` class and message.
    fn read(&self) -> Result<Vec<u8>, (&'static str, String)> {
        match self {
            WarmTarget::Path(path) => read_file(path),
            WarmTarget::Descriptor(number) => read_descriptor(*number),
        }
    }
}

/// `open(number, ...).read()`: reads the **current** offset of the shared open file description to its end
/// (which is where the offset is left afterwards, visible to every process sharing the description) and closes
/// the descriptor, as Python's file object that took ownership does when it is released.
///
/// This is the one place the crate needs `unsafe`: a descriptor *number* has no owner the type system knows.
/// The contract that makes the conversion sound: the caller has just seen the number name an open descriptor of
/// this process (`WarmTarget::status`), the process runs this on a single worker while the rest of the program
/// holds no other handle to it (only the standard streams, which the standard library never closes), and the
/// descriptor is consumed exactly once here - so ownership moves to the `File`, which closes it on drop.
fn read_descriptor(number: i32) -> Result<Vec<u8>, (&'static str, String)> {
    use std::io::Read;
    use std::os::fd::{FromRawFd, RawFd};
    // SAFETY: see the function documentation; `number` was verified open and is not used again afterwards.
    let mut file = unsafe { std::fs::File::from_raw_fd(number as RawFd) };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        // The error of a read carries no file name: `[Errno 9] Bad file descriptor`.
        let detail = error.to_string();
        let message = detail
            .split(" (os error")
            .next()
            .unwrap_or(&detail)
            .to_string();
        let message = match error.raw_os_error() {
            Some(errno) => format!("[Errno {errno}] {message}"),
            None => message,
        };
        (os_error_class(&error), message)
    })?;
    Ok(bytes)
}

/// The inline body of `_spec_axis_args`: `Ok(warm)` is what it prints, `Err` is `sys.exit(f"{Class}: {msg}")`
/// (status 1) or an exception the body does not handle.
fn spec_source(raw: &PyStr) -> Result<PyValue, SpecSourceStop> {
    let document = read_json_file(raw)?;
    let warm = match &document {
        PyValue::Dict(map) => map.get("bench_warm_json").cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    if !warm.is_truthy() {
        return Err(SpecSourceStop::Exit(Failure::new(
            "ValueError",
            "lite raw 에 bench_warm_json 이 없다",
        )));
    }
    let target = WarmTarget::of(&warm).map_err(SpecSourceStop::Crash)?;
    if !target.is_usable() {
        let mut message = PyStr::new("bench_warm_json 부재/빈 파일: ");
        message.push_py(&warm.display_text());
        return Err(SpecSourceStop::Exit(Failure::with_message(
            "ValueError",
            message,
        )));
    }
    let source = read_json_bytes(target.read())?;
    inherit_accept_len(&source, &warm).map_err(|failure| {
        if failure.is_value_error() {
            SpecSourceStop::Exit(failure)
        } else {
            SpecSourceStop::Crash(failure)
        }
    })?;
    Ok(warm)
}

enum SpecSourceStop {
    /// Caught by `except (OSError, ValueError)`: `sys.exit("Class: message")`.
    Exit(Failure),
    /// Not caught: the interpreter prints a traceback.
    Crash(Failure),
}

/// `json.load(open(path, encoding="utf-8"))` with the handler of the helper around it.
fn read_json_file(path: &PyStr) -> Result<PyValue, SpecSourceStop> {
    read_json_bytes(read_file(path))
}

/// The decode and parse of what `open(...)` read, under the helper's `except (OSError, ValueError)`.
fn read_json_bytes(
    read: Result<Vec<u8>, (&'static str, String)>,
) -> Result<PyValue, SpecSourceStop> {
    let bytes =
        read.map_err(|(class, message)| SpecSourceStop::Exit(Failure::new(class, &message)))?;
    decode_file_text(&bytes)
        .and_then(|text| loads_text(&text))
        .map_err(|error| {
            if error.is_value_error() {
                SpecSourceStop::Exit(load_failure(&error))
            } else {
                SpecSourceStop::Crash(load_failure(&error))
            }
        })
}

fn run_spec_source(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let [flag, path] = args else {
        write_stderr(io, &PyStr::new(SPEC_SOURCE_USAGE));
        return 2;
    };
    if !flag.equals("--lite-raw") {
        write_stderr(io, &PyStr::new(SPEC_SOURCE_USAGE));
        return 2;
    }
    let path = path.clone();
    match on_large_stack(move || spec_source(&path)) {
        Ok(warm) => match io.print(&warm.display_text()) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, &PyStr::default()),
        },
        Err(SpecSourceStop::Exit(failure)) => {
            let mut text = failure.line();
            text.push_str("\n");
            write_stderr(io, &text);
            1
        }
        Err(SpecSourceStop::Crash(failure)) => {
            let mut text = PyStr::new(TRACEBACK_HEADER);
            text.push_py(&failure.line());
            text.push_str("\n");
            write_stderr(io, &text);
            1
        }
    }
}
