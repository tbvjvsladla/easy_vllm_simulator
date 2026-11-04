//! `classify_cell_api`: the Rust-only surface of the classifier, a separate program so that the `classify_cell` command
//! line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - every public and needed private function of the original module as a differential and bridge
//!   transport. Each stdin line is `{id, operation, arguments}`; each yields one line `{id, kind: value, value}` or
//!   `{id, kind: exception, type, message}` (`ClassifyError` keeps its own `type`). A row also carries `"stderr": text`
//!   when the call wrote warnings to stderr (the original's `sys.stderr.write`), so a batch keeps them apart.
//!   Operations: `classify`, `events_node_plan`, `scan_node_events`, `correlate_nodes`, `kill_events_in_window`,
//!   `events_scan_summary`, `bench_mode_record_path`, `read_bench_mode_record`, `write_bench_mode_record`,
//!   `read_cell_lite`, `classify_bench_mode`, `declared_lite_record`, `bench_mode_kind`, `resolve_self_node` and the
//!   helpers `_utc`, `_iso`, `_path_segment`, `_newest_event_ts`, `_miss_source`, `_read_events`, `_scan_desc`,
//!   `_bench_mode_record`, `_node_identity_resolver`, `_node_role_contract`, and `constants`. Arguments follow the Python
//!   signatures (names, defaults and the `TypeError` a wrong call raises); path arguments are relative to the
//!   process's working directory and the files they name are read or written when the operation runs.
//!   Opt-in: `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the result
//!   as compact ASCII JSON text in `value_json`), so the envelope never adds depth to a deep document. Rows and
//!   `arguments_json` texts are decoded with the `NaN`-identity decoder (`pyjson::loads_objects`).
//! * **datetime transport**: JSON has no `datetime`. An argument or result that is one is the object
//!   `{"datetime_utc": "<isoformat>"}` (an aware ISO-8601 text the original's `_utc` accepts; results are written as
//!   `datetime.isoformat()` of the UTC value, `+00:00`). `started`, `ended`, `lo`, `hi` and the result of `_utc` and
//!   `_newest_event_ts` use it; `None` is `null`. A value of any other type where the original expects a `datetime`
//!   raises the `TypeError` Python's arithmetic raises.
//! * **owners** (`OPEN_S3`): operations that ask `node_identity.sh` or `node_role_contract` need
//!   `CLASSIFY_CELL_SCRIPTS_DIR` (or the installed layout, or `EASY_VLLM_SOURCE_ROOT`) and, for the role contract,
//!   `CLASSIFY_CELL_ROLE_ADAPTER`; a missing configuration is the named `ClassifyCellConfigurationError`.
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `self-test` - the original's own checks, run for real against the installed owners.
//! * `native-help` - this list.

use crate::classify_cell::{
    bench_mode_kind, bench_mode_record_path, classify, classify_bench_mode, constants,
    correlate_nodes, declared_lite_record, events_node_plan, events_scan_summary,
    kill_events_in_window, newest_event_ts, parse_utc, path_segment, read_bench_mode_record,
    read_cell_lite, read_events, scan_node_events, write_bench_mode_record, BenchModeFields,
    BenchModeRequest, Diagnostics, Moment,
};
use crate::classify_cell_owner::Owners;
use crate::classify_cell_selftest::run_self_test;
use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::guidellm::Failure;
use crate::pydyn::{dict_of, iterate};
use crate::pyjson::{decode_file_text, loads, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, print_text, row_arguments, Bound, Signature, Wire,
};
use crate::utc::UtcInstant;

const NATIVE_HELP: &str = "classify_cell_api Rust-only commands:\n  classify_cell_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  classify_cell_api constants        the module constants as one JSON object\n  classify_cell_api self-test        the original's own checks, run against the installed owners (CLASSIFY_CELL_SCRIPTS_DIR)\n  classify_cell_api native-help      this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], owners: &Owners, io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(owners, io),
        Some("constants") if args.len() == 1 => {
            print_text(io, &constants().dumps_ascii().unwrap_or_default())
        }
        Some("self-test") if args.len() == 1 => {
            let report = on_large_stack(|| run_self_test(owners));
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

// ── Python call binding ─────────────────────────────────────────────────────────

const fn signature(
    function: &'static str,
    positional: &'static [&'static str],
    optional: &'static [&'static str],
) -> Signature {
    Signature {
        function,
        positional,
        keyword_only: &[],
        optional,
    }
}

const CLASSIFY: Signature = signature(
    "classify",
    &[
        "serve_rc",
        "measure_rc",
        "kill_hits",
        "events_scanned",
        "correlation",
        "lite",
    ],
    &["correlation", "lite"],
);
const KILL_EVENTS_IN_WINDOW: Signature = signature(
    "kill_events_in_window",
    &["event_lines", "started", "ended", "tolerance_s"],
    &["tolerance_s"],
);
const CORRELATE_NODES: Signature = signature(
    "correlate_nodes",
    &[
        "node_scans",
        "lo",
        "hi",
        "tolerance_s",
        "cross_node_tolerance_s",
        "cross_node_tolerance_source",
    ],
    &[
        "tolerance_s",
        "cross_node_tolerance_s",
        "cross_node_tolerance_source",
    ],
);
const EVENTS_SCAN_SUMMARY: Signature = signature("events_scan_summary", &["scan_doc"], &[]);
const BENCH_MODE_RECORD_PATH: Signature =
    signature("bench_mode_record_path", &["sweep_index_path"], &[]);
const READ_BENCH_MODE_RECORD: Signature = signature(
    "read_bench_mode_record",
    &["sweep_index_path", "index_doc", "record_path"],
    &["record_path"],
);
const WRITE_BENCH_MODE_RECORD: Signature = signature(
    "write_bench_mode_record",
    &["sweep_index_path", "record"],
    &[],
);
const READ_CELL_LITE: Signature = signature("read_cell_lite", &["raw_path", "started"], &[]);
const CLASSIFY_BENCH_MODE: Signature = signature(
    "classify_bench_mode",
    &[
        "index",
        "event_lines",
        "events_scanned",
        "tolerance_s",
        "node_scans",
        "plan",
        "cross_node_tolerance_s",
        "cross_node_tolerance_source",
    ],
    &[
        "tolerance_s",
        "node_scans",
        "plan",
        "cross_node_tolerance_s",
        "cross_node_tolerance_source",
    ],
);
const DECLARED_LITE_RECORD: Signature = signature("declared_lite_record", &["detail"], &[]);
const BENCH_MODE_KIND: Signature = signature("bench_mode_kind", &["record"], &[]);
const EVENTS_NODE_PLAN: Signature = signature(
    "events_node_plan",
    &[
        "topology",
        "measured_node",
        "measured_node_source",
        "manifest_path",
        "repo",
    ],
    &[
        "measured_node",
        "measured_node_source",
        "manifest_path",
        "repo",
    ],
);
const SCAN_NODE_EVENTS: Signature = signature("scan_node_events", &["repo", "plan"], &[]);
const RESOLVE_SELF_NODE: Signature = signature("resolve_self_node", &["repo"], &[]);
const PATH_SEGMENT: Signature = signature("_path_segment", &["value"], &[]);
const UTC: Signature = signature("_utc", &["value", "field"], &[]);
const ISO: Signature = signature("_iso", &["ts"], &[]);
const NEWEST_EVENT_TS: Signature = signature("_newest_event_ts", &["lines"], &[]);
const MISS_SOURCE: Signature = signature("_miss_source", &["correlation", "events_scanned"], &[]);
const READ_EVENTS: Signature = signature("_read_events", &["paths"], &[]);
const SCAN_DESC: Signature = signature("_scan_desc", &["scans", "plan"], &[]);
const BENCH_MODE_RECORD: Signature = signature(
    "_bench_mode_record",
    &[
        "mode",
        "source",
        "reason",
        "reason_source",
        "repetition",
        "kill_hits",
        "correlation",
        "events_scan",
    ],
    &[
        "reason",
        "reason_source",
        "repetition",
        "kill_hits",
        "correlation",
        "events_scan",
    ],
);
const NODE_IDENTITY_RESOLVER: Signature = signature("_node_identity_resolver", &[], &[]);
const NODE_ROLE_CONTRACT: Signature = signature("_node_role_contract", &[], &[]);
const CONSTANTS: Signature = signature("constants", &[], &[]);

/// The default `tolerance_s=0`.
static ZERO: std::sync::LazyLock<PyValue> = std::sync::LazyLock::new(|| PyValue::int(0));

fn pair(first: PyValue, second: PyValue) -> PyValue {
    PyValue::Tuple(vec![first, second])
}

/// `{"datetime_utc": text}` for an instant.
fn datetime_value(instant: UtcInstant) -> PyValue {
    let mut map = PyDict::new();
    map.set("datetime_utc", PyValue::str(&instant.iso_text()));
    PyValue::Dict(map)
}

fn instant_or_none(instant: Option<UtcInstant>) -> PyValue {
    instant.map_or(PyValue::None, datetime_value)
}

/// A `datetime` argument: the transport object, else a value of another type (which fails where Python's arithmetic on
/// it would - see [`Moment`]).
fn datetime_argument(value: &PyValue) -> Result<Moment, Failure> {
    if let Some(map) = dict_of(value) {
        if map.len() == 1 {
            if let Some(text) = map.get("datetime_utc") {
                return parse_utc(text, "datetime_utc").map(Moment::At);
            }
        }
    }
    Ok(Moment::Other(value.peel().type_name()))
}

/// One operation of the module by name, with the arguments as a Python call gives them. Warnings the call writes to
/// stderr are appended to `diagnostics`.
pub fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    owners: &Owners,
    diagnostics: &mut Diagnostics,
) -> Result<PyValue, Failure> {
    match operation {
        "classify" => {
            let call: Bound<'_> = CLASSIFY.bind(arguments)?;
            classify(
                call.get_ref("serve_rc"),
                call.get_ref("measure_rc"),
                call.get_ref("kill_hits"),
                call.get_ref("events_scanned"),
                call.get_ref("correlation"),
                call.get_ref("lite"),
            )
        }
        "kill_events_in_window" => {
            let call = KILL_EVENTS_IN_WINDOW.bind(arguments)?;
            let tolerance = call.get_or("tolerance_s", PyValue::int(0));
            let started = datetime_argument(&call.get("started"))?;
            let ended = datetime_argument(&call.get("ended"))?;
            kill_events_in_window(&call.get("event_lines"), started, ended, &tolerance)
        }
        "correlate_nodes" => {
            let call = CORRELATE_NODES.bind(arguments)?;
            let lo = datetime_argument(&call.get("lo"))?;
            let hi = datetime_argument(&call.get("hi"))?;
            let (hits, correlation, per_node) = correlate_nodes(
                &call.get("node_scans"),
                lo,
                hi,
                &call.get_or("tolerance_s", PyValue::int(0)),
                &call.get("cross_node_tolerance_s"),
                &call.get("cross_node_tolerance_source"),
            )?;
            Ok(PyValue::Tuple(vec![
                PyValue::List(hits),
                PyValue::str(correlation),
                PyValue::List(per_node),
            ]))
        }
        "events_scan_summary" => {
            let call = EVENTS_SCAN_SUMMARY.bind(arguments)?;
            events_scan_summary(call.get_ref("scan_doc"))
        }
        "bench_mode_record_path" => {
            let call = BENCH_MODE_RECORD_PATH.bind(arguments)?;
            Ok(PyValue::Str(bench_mode_record_path(
                &call.get("sweep_index_path"),
            )?))
        }
        "read_bench_mode_record" => {
            let call = READ_BENCH_MODE_RECORD.bind(arguments)?;
            read_bench_mode_record(
                &call.get("sweep_index_path"),
                &call.get("index_doc"),
                &call.get("record_path"),
            )
        }
        "write_bench_mode_record" => {
            let call = WRITE_BENCH_MODE_RECORD.bind(arguments)?;
            Ok(PyValue::Str(write_bench_mode_record(
                &call.get("sweep_index_path"),
                &call.get("record"),
            )?))
        }
        "read_cell_lite" => {
            let call = READ_CELL_LITE.bind(arguments)?;
            let started = call.get("started");
            let started = if matches!(started.peel(), PyValue::None) {
                None
            } else {
                Some(datetime_argument(&started)?)
            };
            read_cell_lite(&call.get("raw_path"), started)
        }
        "classify_bench_mode" => {
            let call = CLASSIFY_BENCH_MODE.bind(arguments)?;
            let tolerance = call.get_or_ref("tolerance_s", &ZERO);
            classify_bench_mode(&BenchModeRequest {
                index: call.get_ref("index"),
                event_lines: call.get_ref("event_lines"),
                events_scanned: call.get_ref("events_scanned"),
                tolerance_s: tolerance,
                node_scans: call.get_ref("node_scans"),
                plan: call.get_ref("plan"),
                cross_node_tolerance_s: call.get_ref("cross_node_tolerance_s"),
                cross_node_tolerance_source: call.get_ref("cross_node_tolerance_source"),
            })
        }
        "declared_lite_record" => {
            let call = DECLARED_LITE_RECORD.bind(arguments)?;
            declared_lite_record(call.get_ref("detail"))
        }
        "bench_mode_kind" => {
            let call = BENCH_MODE_KIND.bind(arguments)?;
            Ok(PyValue::str(bench_mode_kind(call.get_ref("record"))))
        }
        "events_node_plan" => {
            let call = EVENTS_NODE_PLAN.bind(arguments)?;
            events_node_plan(
                &call.get("topology"),
                &call.get("measured_node"),
                &call.get("measured_node_source"),
                &call.get("manifest_path"),
                &call.get("repo"),
                owners,
            )
        }
        "scan_node_events" => {
            let call = SCAN_NODE_EVENTS.bind(arguments)?;
            scan_node_events(&call.get("repo"), &call.get("plan"), diagnostics)
        }
        "resolve_self_node" => {
            let call = RESOLVE_SELF_NODE.bind(arguments)?;
            let (node, source) = owners.resolve_self_node(&call.get("repo"))?;
            Ok(pair(
                node.map_or(PyValue::None, PyValue::Str),
                PyValue::Str(source),
            ))
        }
        "_path_segment" => {
            let call = PATH_SEGMENT.bind(arguments)?;
            Ok(PyValue::Bool(path_segment(&call.get("value"))))
        }
        "_utc" => {
            let call = UTC.bind(arguments)?;
            let field = match call.get("field").peel() {
                PyValue::Str(text) => text.to_utf8().unwrap_or_default(),
                other => other.display_text().to_utf8().unwrap_or_default(),
            };
            Ok(datetime_value(parse_utc(&call.get("value"), &field)?))
        }
        "_iso" => {
            let call = ISO.bind(arguments)?;
            let ts = call.get("ts");
            if matches!(ts.peel(), PyValue::None) {
                return Ok(PyValue::None);
            }
            match datetime_argument(&ts)? {
                Moment::At(instant) => Ok(PyValue::str(&instant.evaluated_text())),
                Moment::Other(type_name) => Err(Failure::new(
                    "AttributeError",
                    &format!("'{type_name}' object has no attribute 'strftime'"),
                )),
            }
        }
        "_newest_event_ts" => {
            let call = NEWEST_EVENT_TS.bind(arguments)?;
            Ok(instant_or_none(newest_event_ts(&call.get("lines"))?))
        }
        "_miss_source" => {
            let call = MISS_SOURCE.bind(arguments)?;
            let miss = classify_miss_source(&call.get("correlation"), &call.get("events_scanned"))?;
            Ok(PyValue::Str(miss))
        }
        "_read_events" => {
            let call = READ_EVENTS.bind(arguments)?;
            let paths = iterate(&call.get("paths"))?;
            let (lines, scanned) = read_events(&paths, diagnostics)?;
            Ok(pair(PyValue::List(lines), PyValue::List(scanned)))
        }
        "_scan_desc" => {
            let call = SCAN_DESC.bind(arguments)?;
            let mut files: Vec<PyValue> = Vec::new();
            for scan in iterate(&call.get("scans"))? {
                let map = crate::pydyn::as_get_target(&scan)?;
                if let Some(found) = map.get("files").filter(|found| found.is_truthy()) {
                    files.extend(iterate(found)?);
                }
            }
            let text = if files.is_empty() {
                PyStr::new("none")
            } else {
                crate::pydyn::join_values(",", &files)?
            };
            Ok(pair(PyValue::List(files), PyValue::Str(text)))
        }
        "_bench_mode_record" => {
            let call = BENCH_MODE_RECORD.bind(arguments)?;
            let mut fields = BenchModeFields::new(call.get("mode"), call.get("source"));
            fields.reason = call.get("reason");
            fields.reason_source = call.get("reason_source");
            fields.repetition = call.get("repetition");
            fields.kill_hits = call.get("kill_hits");
            fields.correlation = call.get_or("correlation", PyValue::str("not_applicable"));
            fields.events_scan = call.get("events_scan");
            Ok(fields.into_record())
        }
        "_node_identity_resolver" => {
            NODE_IDENTITY_RESOLVER.bind(arguments)?;
            Ok(owners
                .node_identity_resolver()?
                .map_or(PyValue::None, PyValue::Str))
        }
        "_node_role_contract" => {
            // The module itself cannot cross a JSON boundary: the answer is whether the original would have one.
            NODE_ROLE_CONTRACT.bind(arguments)?;
            Ok(PyValue::Bool(owners.open_role_contract()?.is_some()))
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

/// `_miss_source` is private to the original module; the operation exposes it through the cell classification's own
/// use (not-scanned / correlation-unavailable / correlation-miss).
fn classify_miss_source(correlation: &PyValue, events_scanned: &PyValue) -> Result<PyStr, Failure> {
    crate::classify_cell::miss_source(correlation, events_scanned)
}

// ── evaluate-jsonl ──────────────────────────────────────────────────────────────

fn evaluate_row(row: &PyValue, owners: &Owners) -> PyValue {
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
    let mut diagnostics = Diagnostics::default();
    let outcome = (|| {
        let operation = match map.get("operation") {
            Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
            Some(other) => other.repr(),
            None => return Err(Failure::new("KeyError", "'operation'")),
        };
        evaluate_operation(
            &operation,
            &row_arguments(map, true)?,
            owners,
            &mut diagnostics,
        )
    })();
    let mut record = match outcome {
        Ok(value) => crate::repeat_api_cli::value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    };
    if !diagnostics.stderr.is_empty() {
        if let PyValue::Dict(map) = &mut record {
            map.set("stderr", PyValue::Str(diagnostics.stderr));
        }
    }
    record
}

fn run_jsonl(owners: &Owners, io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[classify_cell_api] ERROR stdin: {error}\n")),
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
                &PyStr::new(&format!("[classify_cell_api] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads_objects(line) {
                Ok(row) => evaluate_row(&row, owners),
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

/// A row decoded for the timing harness: JSON text -> value (kept for symmetry with the other `*_api` programs).
pub fn decode_row(line: &str) -> Result<PyValue, Failure> {
    loads(line).map_err(|error| load_failure(&error))
}
