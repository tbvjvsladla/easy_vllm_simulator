//! Rust port of `adversarial-benchmark/scripts/classify_cell.py`: the three-way close of a broad-search cell
//! (`serve_failed` / `measurement_void` / `measured`, plus `not_measured`), the cause of a void read from node
//! blackbox events by **time correlation**, and the `bench_mode` (full or lite) decision with its durable record.
//!
//! The original is a module of plain Python functions that other programs `import`, so every public function takes and
//! returns Python values ([`PyValue`]) and fails with the exception (class and message) Python raises on a wrong-typed
//! argument; the classification rules themselves are ported once and not re-derived. Timestamps are typed
//! ([`UtcInstant`]) and a tolerance is a `timedelta` built with CPython's construction rules ([`pydyn`]).
//!
//! Reused, not copied: the repeat condition and summary come from [`crate::repeat_axis`], the lite verdict vocabulary
//! from [`crate::lite_metrics`], the full-bench tool token from [`crate::guidellm`]. The two owners that are not ported
//! yet (`node_identity.sh`, `node_role_contract.py`) are asked, never re-implemented ([`crate::classify_cell_owner`]).
//!
//! Names follow the original wire (`generated_by="classify_cell.py"`, the Korean notes, key order).

use crate::classify_cell_owner::{Descriptor, Owners};
use crate::guidellm::{Failure, BENCH_TOOL};
use crate::lite_metrics::{open_target, read_descriptor, read_lite_verdict, OpenTarget};
use crate::ospath;
use crate::pydyn::{
    as_get_target, dict_of, ensure_hashable, equals, equals_zero, is_text, iterate,
    iterate_borrowed, join, join_values, key_error, or_none, subscript, timedelta_seconds_micros,
    type_error, Text,
};
use crate::pyfs::{os_error_class, os_string};
use crate::pyjson::{decode_file_text, loads_text, LoadError};
use crate::pystr::lit;
use crate::pystr::PyStr;
use crate::pyvalue::{DumpError, PyDict, PyValue};
use crate::repeat_axis::{repetition_established, summarize, SummarizeRequest};
use crate::utc::{parse_instant, InstantFailure, UtcInstant};

pub const OUTCOME_SERVE_FAILED: &str = "serve_failed";
pub const OUTCOME_MEASUREMENT_VOID: &str = "measurement_void";
pub const OUTCOME_MEASURED: &str = "measured";
pub const OUTCOME_NOT_MEASURED: &str = "not_measured";

/// Events that mean a kill was **carried out** (`*_trip` is the judgement and `*_trip_dryrun` is observation only).
pub const KILL_EVENT_KINDS: [&str; 3] = ["watchdog_kill_ack", "thermal_kill_ack", "earlyoom_kill"];
/// The judgement just before the kill; supporting evidence only.
pub const TRIP_EVENT_KINDS: [&str; 2] = ["watchdog_trip", "thermal_trip"];

pub const BENCH_MODE_FULL: &str = "full";
pub const BENCH_MODE_LITE: &str = "lite";
pub const BENCH_MODES: [&str; 2] = [BENCH_MODE_FULL, BENCH_MODE_LITE];
pub const DOWNGRADE_RUN_FAILED: &str = "run_failed";
pub const DOWNGRADE_BLACKBOX_KILL: &str = "blackbox_kill";
pub const DOWNGRADE_REASONS: [&str; 2] = [DOWNGRADE_RUN_FAILED, DOWNGRADE_BLACKBOX_KILL];
pub const BENCH_MODE_SOURCE_DECLARED_PREFIX: &str = "declared(";
pub const BENCH_MODE_RECORD_NAME: &str = "bench_mode.json";

pub const CORRELATION_MATCHED: &str = "matched";
pub const CORRELATION_MISS: &str = "miss";
pub const CORRELATION_NOT_SCANNED: &str = "not_scanned";
pub const CORRELATION_UNAVAILABLE: &str = "unavailable";
pub const CORRELATION_NOT_APPLICABLE: &str = "not_applicable";
pub const DOWNGRADE_CORRELATIONS: [&str; 5] = [
    CORRELATION_MATCHED,
    CORRELATION_MISS,
    CORRELATION_NOT_SCANNED,
    CORRELATION_UNAVAILABLE,
    CORRELATION_NOT_APPLICABLE,
];

pub const EVENTS_LOCAL_PARTS: [&str; 2] = ["docs", "logs"];
pub const EVENTS_RECOVERED_PARTS: [&str; 3] = ["sync_staging", "sub_docs", "logs"];
pub const MEASURED_NODE_CLUSTER: &str = "cluster";

pub const NODE_SCAN_MATCHED: &str = "matched";
pub const NODE_SCAN_MISS: &str = "miss";
pub const NODE_SCAN_NOT_SCANNED: &str = "not_scanned";
pub const NODE_SCAN_NOT_COVERED: &str = "not_covered";
pub const NODE_SCAN_SKEW_UNDECLARED: &str = "skew_undeclared";
pub const NODE_SCAN_STATUSES: [&str; 5] = [
    NODE_SCAN_MATCHED,
    NODE_SCAN_MISS,
    NODE_SCAN_NOT_SCANNED,
    NODE_SCAN_NOT_COVERED,
    NODE_SCAN_SKEW_UNDECLARED,
];
/// The place-holder node of a target set the plan could not determine (not a path component; never read).
pub const UNIDENTIFIED_NODE: &str = "(미정)";

/// `LITE_VOID_REASONS`: the lite verdicts that explain a failed measurement, and the void reason each becomes.
pub const LITE_VOID_REASONS: [(&str, &str); 2] = [
    ("measurement_path_failed", "lite_measurement_path_failed"),
    ("server_failed", "lite_server_failed"),
];

/// What the program writes to stderr while it works (`sys.stderr.write` of the original), in order.
#[derive(Debug, Default)]
pub struct Diagnostics {
    pub stderr: PyStr,
}

impl Diagnostics {
    pub fn write(&mut self, text: &PyStr) {
        self.stderr.push_py(text);
    }
}

fn failure(class: &'static str, message: PyStr) -> Failure {
    Failure::with_message(class, message)
}

fn text_value(text: PyStr) -> PyValue {
    PyValue::Str(text)
}

fn dict_from(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::with_capacity(entries.len());
    for (key, value) in entries {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

fn str_list(items: &[&str]) -> PyValue {
    PyValue::List(items.iter().map(|item| PyValue::str(item)).collect())
}

/// `_path_segment(value)`: a name usable as one path component (`..` and separators are refused; a leading `.` too).
pub fn path_segment(value: &PyValue) -> bool {
    match value.peel() {
        PyValue::Str(text) => {
            !text.is_empty()
                && !text.code_points().contains(&('/' as u32))
                && text.code_points().first() != Some(&('.' as u32))
        }
        _ => false,
    }
}

// ── time ────────────────────────────────────────────────────────────────────────

/// `_utc(value, field)`: an ISO-8601 text with a UTC offset as an instant. `ClassifyError` for text that cannot be
/// judged; the `OverflowError` of `astimezone` is a different failure and is not caught by callers that catch the
/// first.
pub fn parse_utc(value: &PyValue, field: &str) -> Result<UtcInstant, Failure> {
    parse_instant(value).map_err(|why| match why {
        InstantFailure::NotText => failure(
            "ClassifyError",
            Text::new()
                .lit(field)
                .lit(" 는 ISO-8601 UTC 문자열이어야 한다(받은 값: ")
                .lit(&value.repr())
                .lit(")")
                .done(),
        ),
        InstantFailure::Unreadable(reason) => failure(
            "ClassifyError",
            Text::new()
                .lit(field)
                .lit(" 를 시각으로 읽을 수 없다(")
                .lit(&value.repr())
                .lit("): ")
                .lit(&reason)
                .done(),
        ),
        InstantFailure::NoTimezone => failure(
            "ClassifyError",
            Text::new()
                .lit(field)
                .lit(" 에 타임존이 없다(")
                .lit(&value.repr())
                .lit(")")
                .done(),
        ),
        InstantFailure::OutOfRange => Failure::new("OverflowError", "date value out of range"),
    })
}

fn is_classify_error(failure: &Failure) -> bool {
    failure.class == "ClassifyError"
}

/// A value that stands where the original expects a `datetime`: an instant, or a value of another type whose first use
/// as a `datetime` raises the `TypeError` Python's arithmetic raises (the operation is named when it is used, as
/// Python names it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    At(UtcInstant),
    /// A value of this type (`type(value).__name__`), not a `datetime`.
    Other(&'static str),
}

impl Moment {
    /// `moment + timedelta(microseconds=delta_micros)` (`operator` is `+` or `-` as the original wrote it).
    fn shifted(self, delta_micros: i128, operator: &str) -> Result<UtcInstant, Failure> {
        match self {
            Moment::At(instant) => instant
                .shifted_by_micros(delta_micros)
                .ok_or_else(|| Failure::new("OverflowError", "date value out of range")),
            Moment::Other(type_name) => Err(type_error(&format!(
                "unsupported operand type(s) for {operator}: '{type_name}' and 'datetime.timedelta'"
            ))),
        }
    }
}

/// `_iso(ts)`.
fn iso(instant: Option<UtcInstant>) -> PyValue {
    instant.map_or(PyValue::None, |ts| PyValue::str(&ts.evaluated_text()))
}

// ── event windows ───────────────────────────────────────────────────────────────

fn compare_text(left: &PyValue, right: &PyValue) -> std::cmp::Ordering {
    match (left.peel(), right.peel()) {
        (PyValue::Str(a), PyValue::Str(b)) => a.code_points().cmp(b.code_points()),
        _ => std::cmp::Ordering::Equal,
    }
}

/// `str.strip()` of a value that must be a string.
fn strip_text(value: &PyValue) -> Result<PyStr, Failure> {
    match value.peel() {
        PyValue::Str(text) => Ok(text.strip()),
        other => Err(Failure::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'strip'", other.type_name()),
        )),
    }
}

/// `json.loads(text)` as the original's callers see it: `ValueError`s (undecodable text, an oversized integer) are
/// `None`; a document nested beyond the recursion limit escapes as `RecursionError`.
fn loads_or_none(text: &PyStr) -> Result<Option<PyValue>, Failure> {
    match loads_text(text) {
        Ok(value) => Ok(Some(value)),
        Err(error @ LoadError::TooDeep { .. }) => {
            let rendered = error.to_string();
            Err(Failure::new(
                "RecursionError",
                rendered
                    .strip_prefix("RecursionError: ")
                    .unwrap_or(&rendered),
            ))
        }
        Err(_) => Ok(None),
    }
}

/// `json.loads(value)` for a value that must be text.
fn loads_value(value: &PyValue) -> Result<Option<PyValue>, Failure> {
    match value.peel() {
        PyValue::Str(text) => loads_or_none(text),
        other => Err(type_error(&format!(
            "the JSON object must be str, bytes or bytearray, not {}",
            other.type_name()
        ))),
    }
}

/// `kill_events_in_window(event_lines, started, ended, tolerance_s)`: the **carried-out** kill events (and trips)
/// inside `[started - tolerance, ended + tolerance]`. A dry-run is observation only and never counts. Damaged lines are
/// skipped, not read as "no events".
pub fn kill_events_in_window(
    event_lines: &PyValue,
    started: Moment,
    ended: Moment,
    tolerance_s: &PyValue,
) -> Result<PyValue, Failure> {
    let lo = started.shifted(-timedelta_seconds_micros(tolerance_s)?, "-")?;
    let hi = ended.shifted(timedelta_seconds_micros(tolerance_s)?, "+")?;
    let mut hits: Vec<PyValue> = Vec::new();
    for line in iterate(event_lines)? {
        let line = strip_text(&line)?;
        if line.is_empty() {
            continue;
        }
        let Some(event) = loads_or_none(&line)? else {
            continue;
        };
        let Some(event) = dict_of(&event) else {
            continue;
        };
        let kind = or_none(event.get("kind"));
        let is_kill = KILL_EVENT_KINDS.iter().any(|name| is_text(&kind, name));
        let is_trip = TRIP_EVENT_KINDS.iter().any(|name| is_text(&kind, name));
        if !is_kill && !is_trip {
            continue;
        }
        if event
            .get("mode")
            .is_some_and(|mode| is_text(mode, "dry-run"))
        {
            continue;
        }
        let ts = match parse_utc(&or_none(event.get("ts")), "event.ts") {
            Ok(ts) => ts,
            Err(error) if is_classify_error(&error) => continue,
            Err(error) => return Err(error),
        };
        if lo <= ts && ts <= hi {
            hits.push(dict_from(vec![
                ("ts", or_none(event.get("ts"))),
                ("kind", kind),
                ("source", or_none(event.get("source"))),
                ("mode", or_none(event.get("mode"))),
                ("targets", or_none(event.get("targets"))),
            ]));
        }
    }
    hits.sort_by(|a, b| {
        compare_text(
            &or_none(dict_of(a).and_then(|map| map.get("ts"))),
            &or_none(dict_of(b).and_then(|map| map.get("ts"))),
        )
    });
    Ok(PyValue::List(hits))
}

/// `_newest_event_ts(lines)`: the latest event time among the readable lines, or `None`.
pub fn newest_event_ts(lines: &PyValue) -> Result<Option<UtcInstant>, Failure> {
    let mut newest: Option<UtcInstant> = None;
    for line in iterate(lines)? {
        let Some(event) = loads_value(&line)? else {
            continue;
        };
        let ts = match dict_of(&event) {
            Some(map) => match parse_utc(&or_none(map.get("ts")), "event.ts") {
                Ok(ts) => Some(ts),
                Err(error) if is_classify_error(&error) => continue,
                Err(error) => return Err(error),
            },
            None => None,
        };
        if let Some(ts) = ts {
            if newest.is_none_or(|known| ts > known) {
                newest = Some(ts);
            }
        }
    }
    Ok(newest)
}

/// `bool(scan.get("remote"))`-style read of an optional entry.
fn truthy(value: Option<&PyValue>) -> bool {
    value.is_some_and(PyValue::is_truthy)
}

/// `correlate_nodes(node_scans, lo, hi, tolerance_s, cross_node_tolerance_s, cross_node_tolerance_source)`: each node's
/// window correlation -> `(hits, downgrade_correlation, per_node)`. No file is read here (a scan carries its lines).
pub fn correlate_nodes(
    node_scans: &PyValue,
    lo: Moment,
    hi: Moment,
    tolerance_s: &PyValue,
    cross_node_tolerance_s: &PyValue,
    cross_node_tolerance_source: &PyValue,
) -> Result<(Vec<PyValue>, &'static str, Vec<PyValue>), Failure> {
    let declared = !matches!(cross_node_tolerance_s.peel(), PyValue::None);
    let mut hits: Vec<PyValue> = Vec::new();
    let mut per_node: Vec<PyValue> = Vec::new();
    for scan_value in iterate(node_scans)? {
        let scan = as_get_target(&scan_value)?;
        let remote = truthy(scan.get("remote"));
        let tol = if remote {
            if declared {
                cross_node_tolerance_s.clone()
            } else {
                PyValue::int(0)
            }
        } else {
            tolerance_s.clone()
        };
        let repo = or_none(scan.get("repo"));
        let unidentified = truthy(scan.get("unidentified"));
        let tol_source: PyStr = if unidentified {
            PyStr::new("n/a(대조 대상 미정 — 판독하지 않았다)")
        } else if remote {
            if declared {
                Text::new()
                    .lit("declared(")
                    .val(cross_node_tolerance_source)
                    .lit(")")
                    .done()
            } else {
                PyStr::new(
                    "undeclared(노드 간 시계 허용오차 미선언 — 창을 넓히지 않고 미스를 결론내지 않는다)",
                )
            }
        } else {
            PyStr::new("same-node-clock(--tolerance-s)")
        };
        let files_value = or_none(scan.get("files"));
        let mut relative: Vec<PyValue> = Vec::new();
        if files_value.is_truthy() {
            for path in iterate(&files_value)? {
                relative.push(if repo.is_truthy() {
                    let path_text = ospath::fspath(&path)?;
                    let start = ospath::fspath(&repo)?;
                    PyValue::Str(ospath::relpath(&path_text, &start)?)
                } else {
                    path
                });
            }
        }
        let mut entry = PyDict::with_capacity(10);
        entry.set("node", or_none(scan.get("node")));
        entry.set("remote", PyValue::Bool(remote));
        entry.set("files", PyValue::List(relative));
        entry.set("tolerance_s", tol.clone());
        entry.set("tolerance_source", text_value(tol_source));
        if truthy(scan.get("fallback")) {
            entry.set("fallback", PyValue::Bool(true));
        }
        if unidentified {
            entry.set("status", PyValue::str(NODE_SCAN_NOT_SCANNED));
            entry.set(
                "detail",
                PyValue::str(
                    "노드 계획 불성립 — 이 노드 밖 대조 대상을 정하지 못했다(보지 못했음)",
                ),
            );
        } else if !files_value.is_truthy() {
            entry.set("status", PyValue::str(NODE_SCAN_NOT_SCANNED));
            entry.set(
                "detail",
                PyValue::str(if remote {
                    "회수본 없음 — fetch_sub_docs.sh 로 회수한 뒤 판정 기록을 다시 쓴다"
                } else {
                    "블랙박스 events 파일 0"
                }),
            );
        } else {
            let lines = {
                let given = or_none(scan.get("lines"));
                if given.is_truthy() {
                    given
                } else {
                    PyValue::List(Vec::new())
                }
            };
            let node_hits = match kill_events_in_window(&lines, lo, hi, &tol)? {
                PyValue::List(items) => items,
                _ => Vec::new(),
            };
            let mut stamped: Vec<PyValue> = Vec::with_capacity(node_hits.len());
            for hit in node_hits {
                let PyValue::Dict(mut map) = hit else {
                    continue;
                };
                map.set("node", or_none(scan.get("node")));
                stamped.push(PyValue::Dict(map));
            }
            let any_executed = stamped.iter().any(|hit| {
                dict_of(hit)
                    .and_then(|map| map.get("kind"))
                    .is_some_and(|kind| KILL_EVENT_KINDS.iter().any(|name| is_text(kind, name)))
            });
            hits.extend(stamped);
            if any_executed {
                entry.set("status", PyValue::str(NODE_SCAN_MATCHED));
                if remote {
                    entry.set(
                        "detail",
                        text_value(if declared {
                            Text::new()
                                .lit("허용오차 ")
                                .val(&tol)
                                .lit("s declared")
                                .done()
                        } else {
                            PyStr::new("허용오차 미선언 — 0s · 창 안 매치만 인정")
                        }),
                    );
                }
            } else if remote {
                let newest = newest_event_ts(&lines)?;
                let edge = hi.shifted(timedelta_seconds_micros(&tol)?, "+")?;
                entry.set("newest_event_utc", iso(newest));
                if newest.is_none_or(|known| known < edge) {
                    entry.set("status", PyValue::str(NODE_SCAN_NOT_COVERED));
                    entry.set(
                        "detail",
                        text_value(
                            Text::new()
                                .lit("회수본의 최신 이벤트 ")
                                .val(&iso(newest))
                                .lit(" < 창 끝+허용오차 ")
                                .val(&iso(Some(edge)))
                                .lit(" — 이 창을 덮는 기록이 아니다(회수가 이르다)")
                                .done(),
                        ),
                    );
                } else if !declared {
                    entry.set("status", PyValue::str(NODE_SCAN_SKEW_UNDECLARED));
                    entry.set(
                        "detail",
                        PyValue::str(
                            "회수본은 창을 덮지만 노드 간 시계 허용오차가 선언되지 않았다",
                        ),
                    );
                } else {
                    entry.set("status", PyValue::str(NODE_SCAN_MISS));
                }
            } else {
                entry.set("status", PyValue::str(NODE_SCAN_MISS));
            }
        }
        per_node.push(PyValue::Dict(entry));
    }
    hits.sort_by(|a, b| {
        compare_text(
            &or_none(dict_of(a).and_then(|map| map.get("ts"))),
            &or_none(dict_of(b).and_then(|map| map.get("ts"))),
        )
    });
    let statuses: Vec<&PyValue> = per_node
        .iter()
        .filter_map(|entry| dict_of(entry).and_then(|map| map.get("status")))
        .collect();
    let has = |name: &str| statuses.iter().any(|status| is_text(status, name));
    let correlation = if has(NODE_SCAN_MATCHED) {
        CORRELATION_MATCHED
    } else if per_node.is_empty() || has(NODE_SCAN_NOT_SCANNED) || has(NODE_SCAN_NOT_COVERED) {
        CORRELATION_NOT_SCANNED
    } else if has(NODE_SCAN_SKEW_UNDECLARED) {
        CORRELATION_UNAVAILABLE
    } else {
        CORRELATION_MISS
    };
    Ok((hits, correlation, per_node))
}

/// `events_scan_summary(scan_doc)`: one line describing a record's `events_scan` (`None` when there is nothing to say).
pub fn events_scan_summary(scan_doc: &PyValue) -> Result<PyValue, Failure> {
    let Some(doc) = dict_of(scan_doc) else {
        return Ok(PyValue::None);
    };
    let plan_value = or_none(doc.get("plan"));
    let empty = PyDict::new();
    let plan = dict_of(&plan_value).unwrap_or(&empty);
    let head = match plan.get("undeterminable") {
        Some(reason) if reason.is_truthy() => Some(
            Text::new()
                .lit("대조 노드 미정(")
                .val(reason)
                .lit(")")
                .done(),
        ),
        _ => None,
    };
    let nodes_value = or_none(doc.get("nodes"));
    let PyValue::List(entries) = nodes_value.peel() else {
        if let Some(head) = head {
            return Ok(text_value(head));
        }
        return match plan.get("nodes") {
            Some(nodes) if nodes.is_truthy() => Ok(text_value(
                Text::new()
                    .lit("대조 노드 계획[")
                    .str(&join(",", nodes)?)
                    .lit("] · 대조할 창 없음")
                    .done(),
            )),
            _ => Ok(PyValue::None),
        };
    };
    let mut parts: Vec<PyValue> = Vec::new();
    for entry in entries {
        let Some(map) = dict_of(entry) else {
            continue;
        };
        let mut text = Text::new()
            .val(&or_none(map.get("node")))
            .lit(if truthy(map.get("remote")) {
                "(원격)"
            } else {
                ""
            })
            .lit("=")
            .val(&or_none(map.get("status")));
        let status_is_miss = map
            .get("status")
            .is_some_and(|s| is_text(s, NODE_SCAN_MISS));
        if !status_is_miss && truthy(map.get("detail")) {
            text = text.lit("(").val(&or_none(map.get("detail"))).lit(")");
        }
        parts.push(text_value(text.done()));
    }
    let body = if parts.is_empty() {
        PyStr::new("대조 노드 0")
    } else {
        Text::new()
            .lit("대조 노드[")
            .str(&join_values(" · ", &parts)?)
            .lit("]")
            .done()
    };
    Ok(text_value(match head {
        Some(head) => Text::new().str(&head).lit(" · ").str(&body).done(),
        None => body,
    }))
}

// ── bench_mode records ──────────────────────────────────────────────────────────

/// `bench_mode_record_path(sweep_index_path)`: the sidecar next to the sweep index.
pub fn bench_mode_record_path(sweep_index_path: &PyValue) -> Result<PyStr, Failure> {
    let absolute = ospath::abspath(&ospath::fspath(sweep_index_path)?)?;
    Ok(ospath::join_all(
        &ospath::dirname(&absolute),
        &[BENCH_MODE_RECORD_NAME],
    ))
}

/// Why a stored record could not be read or is not for this sweep (`status` of [`read_bench_mode_record`]).
fn unreadable(name: &PyStr, why: PyStr) -> PyStr {
    Text::new()
        .lit("unreadable(")
        .str(name)
        .lit(" 판독 실패: ")
        .str(&why)
        .lit(")")
        .done()
}

/// `read_bench_mode_record(sweep_index_path, index_doc, record_path)` -> `(record | None, status)` with status `ok`,
/// `absent(...)`, `unreadable(...)` or `stale(...)`. Absence, damage and staleness are never folded into full or lite.
pub fn read_bench_mode_record(
    sweep_index_path: &PyValue,
    index_doc: &PyValue,
    record_path: &PyValue,
) -> Result<PyValue, Failure> {
    let path = if record_path.is_truthy() {
        ospath::fspath(record_path)?
    } else {
        bench_mode_record_path(sweep_index_path)?
    };
    let name = ospath::basename(&path);
    let pair = |record: Option<PyValue>, status: PyStr| {
        PyValue::Tuple(vec![record.unwrap_or(PyValue::None), text_value(status)])
    };
    if !ospath::isfile(&path) {
        return Ok(pair(
            None,
            Text::new()
                .lit("absent(")
                .str(&name)
                .lit(" 부재 — 이 스윕을 분류한 기록이 없다)")
                .done(),
        ));
    }
    let os = match os_string(&path) {
        Ok(os) => os,
        Err(error) => return Err(error.into_failure()),
    };
    let bytes = match std::fs::read(std::path::Path::new(&os)) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Ok(pair(
                None,
                unreadable(&name, PyStr::new(&crate::cli::os_error_text(&error, &path))),
            ))
        }
    };
    let record = match decode_file_text(&bytes) {
        Ok(text) => match loads_text(&text) {
            Ok(record) => record,
            Err(error @ LoadError::TooDeep { .. }) => {
                let rendered = error.to_string();
                return Err(Failure::new(
                    "RecursionError",
                    rendered
                        .strip_prefix("RecursionError: ")
                        .unwrap_or(&rendered),
                ));
            }
            Err(error) => {
                return Ok(pair(
                    None,
                    unreadable(&name, PyStr::new(&error.to_string())),
                ))
            }
        },
        Err(error) => {
            return Ok(pair(
                None,
                unreadable(&name, PyStr::new(&error.to_string())),
            ))
        }
    };
    let Some(map) = dict_of(&record) else {
        return Ok(pair(
            None,
            Text::new()
                .lit("unreadable(")
                .str(&name)
                .lit(" 가 객체가 아니다)")
                .done(),
        ));
    };
    let index_utc = match dict_of(index_doc) {
        Some(index) => or_none(index.get("generated_utc")),
        None => PyValue::None,
    };
    let recorded = or_none(map.get("sweep_index_generated_utc"));
    if !equals(&recorded, &index_utc) {
        return Ok(pair(
            None,
            Text::new()
                .lit("stale(")
                .str(&name)
                .lit(" 는 다른 측정(generated_utc=")
                .val(&recorded)
                .lit(")의 분류다 · 이 스윕 ")
                .val(&index_utc)
                .lit(")")
                .done(),
        ));
    }
    Ok(pair(Some(record), PyStr::new("ok")))
}

/// The pieces `json.dump(value, handle, ensure_ascii=False, indent=2)` hands the file, one by one (`_make_iterencode`):
/// a dict's string values and every key are chunks of their own, a list item is joined with the separator before it.
/// `emit` returns an error to stop, as the file object's `write` does.
fn dump_chunks(
    value: &PyValue,
    depth: usize,
    emit: &mut dyn FnMut(PyStr) -> Result<(), Failure>,
) -> Result<(), Failure> {
    const MAX_DEPTH: usize = 995;
    let scalar = |value: &PyValue| -> Result<PyStr, Failure> {
        value
            .dumps_indented(2)
            .map_err(|error| dump_failure(&error))
    };
    let indent_of = |level: usize| PyStr::new(&format!("\n{}", "  ".repeat(level)));
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            if items.is_empty() {
                return emit(PyStr::new("[]"));
            }
            if depth + 1 > MAX_DEPTH {
                return Err(dump_failure(&DumpError::TooDeep));
            }
            let level = depth + 1;
            let mut buffer = PyStr::new("[");
            buffer.push_py(&indent_of(level));
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    buffer = PyStr::new(",");
                    buffer.push_py(&indent_of(level));
                }
                match item.peel() {
                    PyValue::List(_) | PyValue::Tuple(_) | PyValue::Dict(_) => {
                        emit(buffer.clone())?;
                        dump_chunks(item, level, emit)?;
                    }
                    other => {
                        let mut chunk = buffer.clone();
                        chunk.push_py(&scalar(other)?);
                        emit(chunk)?;
                    }
                }
            }
            emit(indent_of(depth))?;
            emit(PyStr::new("]"))
        }
        PyValue::Dict(map) => {
            if map.is_empty() {
                return emit(PyStr::new("{}"));
            }
            if depth + 1 > MAX_DEPTH {
                return Err(dump_failure(&DumpError::TooDeep));
            }
            let level = depth + 1;
            emit(PyStr::new("{"))?;
            emit(indent_of(level))?;
            for (index, (key, item)) in map.iter().enumerate() {
                if index > 0 {
                    let mut separator = PyStr::new(",");
                    separator.push_py(&indent_of(level));
                    emit(separator)?;
                }
                emit(scalar(&PyValue::Str(key.clone()))?)?;
                emit(PyStr::new(": "))?;
                match item.peel() {
                    PyValue::List(_) | PyValue::Tuple(_) | PyValue::Dict(_) => {
                        dump_chunks(item, level, emit)?
                    }
                    other => emit(scalar(other)?)?,
                }
            }
            emit(indent_of(depth))?;
            emit(PyStr::new("}"))
        }
        other => emit(scalar(other)?),
    }
}

fn dump_failure(error: &DumpError) -> Failure {
    let rendered = error.to_string();
    let message = rendered
        .strip_prefix(&format!("{}: ", error.python_type()))
        .unwrap_or(&rendered);
    Failure::new(error.python_type(), message)
}

/// The text `write_bench_mode_record` puts in the file: `json.dump(record, ensure_ascii=False, indent=2)` and a
/// newline, encoded as the file's strict UTF-8 stream encodes each chunk as it arrives. The first chunk that cannot be
/// encoded (a lone surrogate) or cannot be produced ends the write; its error is the result.
pub fn encode_record_file(record: &PyValue) -> Result<Vec<u8>, Failure> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut emit = |chunk: PyStr| -> Result<(), Failure> {
        let encoded = crate::pyfs::strict_utf8(&chunk)
            .map_err(|error| Failure::new("UnicodeEncodeError", &error.message()))?;
        bytes.extend_from_slice(&encoded);
        Ok(())
    };
    dump_chunks(record, 0, &mut emit)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// `write_bench_mode_record(sweep_index_path, record)`: write the record **atomically** (a temporary file in the same
/// directory, then `os.replace`) so a reader never sees half a record. The temporary file is removed on any failure.
pub fn write_bench_mode_record(
    sweep_index_path: &PyValue,
    record: &PyValue,
) -> Result<PyStr, Failure> {
    let out = bench_mode_record_path(sweep_index_path)?;
    let directory = ospath::dirname(&out);
    let prefix = format!(".{BENCH_MODE_RECORD_NAME}.");
    let (mut file, temporary) = create_temporary(&directory, &prefix)?;
    let outcome = (|| -> Result<(), Failure> {
        use std::io::Write;
        let bytes = encode_record_file(record);
        // The chunks before a failing one were written, then the file is closed; the file is removed below either way.
        let written = match &bytes {
            Ok(bytes) => file.write_all(bytes),
            Err(_) => Ok(()),
        };
        drop(file);
        bytes?;
        written.map_err(|error| {
            Failure::new(
                os_error_class(&error),
                &crate::cli::os_error_text(&error, &temporary),
            )
        })?;
        replace(&temporary, &out)
    })();
    if let Err(error) = outcome {
        if ospath::lexists(&temporary) {
            if let Ok(os) = os_string(&temporary) {
                let _ = std::fs::remove_file(std::path::Path::new(&os));
            }
        }
        return Err(error);
    }
    Ok(out)
}

/// `os.replace(source, destination)`.
fn replace(source: &PyStr, destination: &PyStr) -> Result<(), Failure> {
    let source_os = os_string(source).map_err(|error| error.into_failure())?;
    let destination_os = os_string(destination).map_err(|error| error.into_failure())?;
    std::fs::rename(
        std::path::Path::new(&source_os),
        std::path::Path::new(&destination_os),
    )
    .map_err(|error| {
        let mut message = PyStr::new(&crate::cli::os_error_text(&error, source));
        message.push_str(" -> ");
        message.push_str(&destination.repr());
        Failure::with_message(os_error_class(&error), message)
    })
}

/// `tempfile.mkstemp(prefix=prefix, dir=directory)`: an exclusively created, owner-only file with a random name.
fn create_temporary(directory: &PyStr, prefix: &str) -> Result<(std::fs::File, PyStr), Failure> {
    use std::os::unix::fs::OpenOptionsExt;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    for _ in 0..10_000 {
        let mut random = [0_u8; 8];
        random_bytes(&mut random);
        let suffix: String = random
            .iter()
            .map(|byte| char::from(ALPHABET[usize::from(*byte) % ALPHABET.len()]))
            .collect();
        let name = ospath::join_all(directory, &[&format!("{prefix}{suffix}")]);
        let os = os_string(&name).map_err(|error| error.into_failure())?;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(std::path::Path::new(&os))
        {
            Ok(file) => return Ok((file, name)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(Failure::new(
                    os_error_class(&error),
                    &crate::cli::os_error_text(&error, &name),
                ))
            }
        }
    }
    Err(Failure::new(
        "FileExistsError",
        "No usable temporary file name found",
    ))
}

fn random_bytes(buffer: &mut [u8]) {
    use std::io::Read;
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(buffer))
        .is_err()
    {
        // Only the uniqueness of a temporary name depends on this; fall back to the clock and the address of a local.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        for (index, byte) in buffer.iter_mut().enumerate() {
            *byte = (now >> (index * 8 % 64)) as u8 ^ (index as u8).wrapping_mul(31);
        }
    }
}

/// `bench_mode_kind(record)` -> `full` | `declared-lite` | `downgraded-lite` | `undetermined`.
pub fn bench_mode_kind(record: &PyValue) -> &'static str {
    let Some(map) = dict_of(record) else {
        return "undetermined";
    };
    let mode = or_none(map.get("bench_mode"));
    let reason = or_none(map.get("downgrade_reason"));
    let source = match map.get("bench_mode_source") {
        Some(value) if value.is_truthy() => value.clone(),
        _ => PyValue::str(""),
    };
    let reason_is_none = matches!(reason.peel(), PyValue::None);
    if is_text(&mode, BENCH_MODE_FULL) && reason_is_none {
        return "full";
    }
    if is_text(&mode, BENCH_MODE_LITE)
        && DOWNGRADE_REASONS.iter().any(|name| is_text(&reason, name))
    {
        return "downgraded-lite";
    }
    if is_text(&mode, BENCH_MODE_LITE) && reason_is_none {
        if let PyValue::Str(text) = source.peel() {
            let prefix: Vec<u32> = BENCH_MODE_SOURCE_DECLARED_PREFIX
                .chars()
                .map(u32::from)
                .collect();
            if text.code_points().starts_with(&prefix) {
                return "declared-lite";
            }
        }
    }
    "undetermined"
}

/// The fields of one `bench_mode` record (`_bench_mode_record`'s arguments); [`BenchModeFields::new`] gives the original's
/// defaults.
#[derive(Clone, Debug)]
pub struct BenchModeFields {
    pub mode: PyValue,
    pub source: PyValue,
    pub reason: PyValue,
    pub reason_source: PyValue,
    pub repetition: PyValue,
    /// `kill_hits`: a falsy value is recorded as `[]`.
    pub kill_hits: PyValue,
    pub correlation: PyValue,
    pub events_scan: PyValue,
}

impl BenchModeFields {
    pub fn new(mode: PyValue, source: PyValue) -> Self {
        BenchModeFields {
            mode,
            source,
            reason: PyValue::None,
            reason_source: PyValue::None,
            repetition: PyValue::None,
            kill_hits: PyValue::None,
            correlation: PyValue::str(CORRELATION_NOT_APPLICABLE),
            events_scan: PyValue::None,
        }
    }

    /// `_bench_mode_record(...)`: one record's shape (key order is part of the wire).
    pub fn into_record(self) -> PyValue {
        let kill_events = if self.kill_hits.is_truthy() {
            self.kill_hits
        } else {
            PyValue::List(Vec::new())
        };
        PyValue::Dict(PyDict::from_distinct_entries([
            (lit!("bench_mode"), self.mode),
            (lit!("bench_mode_source"), self.source),
            (lit!("downgrade_reason"), self.reason),
            (lit!("downgrade_reason_source"), self.reason_source),
            (lit!("downgrade_correlation"), self.correlation),
            (lit!("events_scan"), self.events_scan),
            (lit!("repetition"), self.repetition),
            (lit!("repetition_kill_events"), kill_events),
        ]))
    }
}

/// A record with only a mode and its source (the shape of an undetermined or declared decision).
fn plain_record(mode: PyValue, source: PyValue) -> PyValue {
    BenchModeFields::new(mode, source).into_record()
}

/// `declared_lite_record(detail)`: the record of a cell that was **declared** lite-only (no full was attempted).
pub fn declared_lite_record(detail: &PyValue) -> Result<PyValue, Failure> {
    let text = if detail.is_truthy() {
        detail.display_text().strip()
    } else {
        PyStr::default()
    };
    if text.is_empty() {
        return Err(Failure::new(
            "ValueError",
            "declared_lite_record: 선언 출처 설명이 비었다 — 누가 lite 만 재기로 했는지 적는다",
        ));
    }
    Ok(plain_record(
        PyValue::str(BENCH_MODE_LITE),
        text_value(
            Text::new()
                .lit(BENCH_MODE_SOURCE_DECLARED_PREFIX)
                .str(&text)
                .lit(")")
                .done(),
        ),
    ))
}

// ── cell outcome ────────────────────────────────────────────────────────────────

/// `_miss_source(correlation, events_scanned)`: how a missing cause is described; looked up by hash, so an unhashable
/// `correlation` is a `TypeError`.
pub fn miss_source(correlation: &PyValue, events_scanned: &PyValue) -> Result<PyStr, Failure> {
    ensure_hashable(correlation)?;
    let prefix = if is_text(correlation, CORRELATION_NOT_SCANNED) {
        lit!("not-scanned(")
    } else if is_text(correlation, CORRELATION_UNAVAILABLE) {
        lit!("correlation-unavailable(")
    } else {
        lit!("correlation-miss(")
    };
    Ok(Text::new()
        .fixed(&prefix)
        .val(events_scanned)
        .fixed(&lit!(")"))
        .done())
}

/// `read_cell_lite(raw_path, started)`: the lite raw of this cell as `{lite_verdict, lite_verdict_source,
/// lite_raw_status}`. It reads, it does not judge; a raw measured before the cell began is a leftover and is not read.
pub fn read_cell_lite(raw_path: &PyValue, started: Option<Moment>) -> Result<PyValue, Failure> {
    let absent = || {
        dict_from(vec![
            ("lite_verdict", PyValue::None),
            ("lite_verdict_source", PyValue::None),
            ("lite_raw_status", PyValue::str("absent")),
        ])
    };
    if !raw_path.is_truthy() {
        return Ok(absent());
    }
    // `open(raw_path, encoding="utf-8")` then `json.load`: a missing file is absence; any other `OSError` or a
    // `ValueError` (not UTF-8, not JSON, an embedded NUL) is unreadable.
    let unreadable = || {
        dict_from(vec![
            ("lite_verdict", PyValue::None),
            (
                "lite_verdict_source",
                text_value(
                    Text::new()
                        .lit("lite_raw(")
                        .val(raw_path)
                        .lit(") 판독 실패")
                        .done(),
                ),
            ),
            ("lite_raw_status", PyValue::str("unreadable")),
        ])
    };
    let bytes = match open_target(raw_path) {
        Ok(OpenTarget::Path(path)) => {
            let os = match os_string(&path) {
                Ok(os) => os,
                Err(error) => {
                    let raised = error.into_failure();
                    return if raised.class == "ValueError" || raised.class == "UnicodeEncodeError" {
                        Ok(unreadable())
                    } else {
                        Err(raised)
                    };
                }
            };
            match std::fs::read(std::path::Path::new(&os)) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(absent()),
                Err(_) => return Ok(unreadable()),
            }
        }
        Ok(OpenTarget::Descriptor(descriptor)) => match read_descriptor(descriptor) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(absent()),
            Err(_) => return Ok(unreadable()),
        },
        Err(error) if error.class == "ValueError" => return Ok(unreadable()),
        Err(error) => return Err(error),
    };
    let Ok(text) = decode_file_text(&bytes) else {
        return Ok(unreadable());
    };
    let raw = match loads_text(&text) {
        Ok(raw) => raw,
        Err(error @ LoadError::TooDeep { .. }) => {
            let rendered = error.to_string();
            return Err(Failure::new(
                "RecursionError",
                rendered
                    .strip_prefix("RecursionError: ")
                    .unwrap_or(&rendered),
            ));
        }
        Err(_) => return Ok(unreadable()),
    };
    let measured_value = dict_of(&raw).map(|map| or_none(map.get("measured_utc")));
    let measured = match &measured_value {
        Some(value) => match parse_utc(value, "lite raw measured_utc") {
            Ok(instant) => Some(instant),
            Err(error) if is_classify_error(&error) => None,
            Err(error) => return Err(error),
        },
        None => None,
    };
    let before_start = match (measured, started) {
        (Some(measured), Some(Moment::At(start))) => measured < start,
        (Some(_), Some(Moment::Other(type_name))) => {
            return Err(type_error(&format!(
                "'<' not supported between instances of 'datetime.datetime' and '{type_name}'"
            )))
        }
        _ => false,
    };
    if measured.is_none() || before_start {
        return Ok(dict_from(vec![
            ("lite_verdict", PyValue::None),
            ("lite_raw_status", PyValue::str("stale")),
            (
                "lite_verdict_source",
                text_value(
                    Text::new()
                        .lit("lite_raw(")
                        .val(raw_path)
                        .lit(") measured_utc=")
                        .lit(
                            &measured_value
                                .as_ref()
                                .map_or_else(|| "None".to_string(), PyValue::repr),
                        )
                        .lit(" 가 셀 시작 전이거나 없다 — 이번 셀의 판정이 아니다")
                        .done(),
                ),
            ),
        ]));
    }
    Ok(dict_from(vec![
        (
            "lite_verdict",
            read_lite_verdict(&raw).map_or(PyValue::None, |verdict| PyValue::str(verdict.label())),
        ),
        ("lite_raw_status", PyValue::str("fresh")),
        (
            "lite_verdict_source",
            text_value(
                Text::new()
                    .lit("lite_raw(")
                    .val(raw_path)
                    .lit(") · lite_metrics.read_lite_verdict")
                    .done(),
            ),
        ),
    ]))
}

/// `classify(serve_rc, measure_rc, kill_hits, events_scanned, correlation, lite)`: two return codes and the event
/// correlation -> the cell's close. `measure_rc` `None` means the measurement step was **not entered** (absence), which
/// is not success. `correlation` (optional) says why a missing cause is missing; `lite` (optional) carries the lite
/// verdict of this cell's raw and explains a failed measurement after a carried-out kill.
pub fn classify(
    serve_rc: &PyValue,
    measure_rc: &PyValue,
    kill_hits: &PyValue,
    events_scanned: &PyValue,
    correlation: &PyValue,
    lite: &PyValue,
) -> Result<PyValue, Failure> {
    let hits = iterate_borrowed(kill_hits)?;
    let mut executed: Vec<&PyValue> = Vec::new();
    for hit in hits.as_slice() {
        let kind = subscript(hit, "kind")?;
        if KILL_EVENT_KINDS.iter().any(|name| is_text(kind, name)) {
            executed.push(hit);
        }
    }
    let unseen = is_text(correlation, CORRELATION_NOT_SCANNED)
        || is_text(correlation, CORRELATION_UNAVAILABLE);
    let events_source = || {
        text_value(
            Text::new()
                .fixed(&lit!("events("))
                .val(events_scanned)
                .fixed(&lit!(")"))
                .done(),
        )
    };
    if !equals_zero(serve_rc) {
        if !executed.is_empty() {
            let (kind, ts) = first_kill(&executed)?;
            return Ok(cell_result(
                PyValue::Str(lit!(OUTCOME_SERVE_FAILED)),
                kind.clone(),
                events_source(),
                kill_hits.clone(),
                Text::new()
                    .fixed(&lit!("서빙이 성립하지 않았다(rc="))
                    .val(serve_rc)
                    .fixed(&lit!("). 구간 안에서 "))
                    .val(kind)
                    .fixed(&lit!(" 가 "))
                    .val(ts)
                    .fixed(&lit!(" 에 **집행**됐다 — 이 죽음은 모델·빌드 평면의 구동불가가 아니라 호스트 평면의 사살이다."))
                    .done(),
            ));
        }
        let source = miss_source(correlation, events_scanned)?;
        return Ok(cell_result(
            PyValue::Str(lit!(OUTCOME_SERVE_FAILED)),
            PyValue::None,
            text_value(source),
            kill_hits.clone(),
            Text::new()
                .fixed(&lit!(
                    "서빙이 성립하지 않아 측정 단계에 도달하지 않았다(rc="
                ))
                .val(serve_rc)
                .fixed(&lit!("). "))
                .fixed(&if unseen {
                    lit!("대조 대상 노드 중 구간을 보지 못한 노드가 있다")
                } else {
                    lit!("구간 안에 집행된 사살 이벤트는 없다")
                })
                .fixed(&lit!(" — 사인을 추측하지 않는다."))
                .done(),
        ));
    }
    if matches!(measure_rc.peel(), PyValue::None) {
        return Ok(cell_result(
            PyValue::Str(lit!(OUTCOME_NOT_MEASURED)),
            PyValue::None,
            PyValue::None,
            kill_hits.clone(),
            lit!("서빙은 성립했으나 측정 단계에 들어가지 않았다(rc 부재) — 성공으로 집계하지 않는다."),
        ));
    }
    if !equals_zero(measure_rc) {
        if !executed.is_empty() {
            let (kind, ts) = first_kill(&executed)?;
            return Ok(cell_result(
                PyValue::Str(lit!(OUTCOME_MEASUREMENT_VOID)),
                kind.clone(),
                events_source(),
                kill_hits.clone(),
                Text::new()
                    .fixed(&lit!("서빙은 성립했으나 "))
                    .val(kind)
                    .fixed(&lit!(" 가 "))
                    .val(ts)
                    .fixed(&lit!(" 에 집행되어 측정이 파괴됐다."))
                    .done(),
            ));
        }
        let lite_value = if lite.is_truthy() { lite } else { &*EMPTY_DICT };
        let lite_map = as_get_target(lite_value)?;
        let lite_verdict = or_none_ref(lite_map.get("lite_verdict"));
        ensure_hashable(lite_verdict)?;
        if let Some((_, void_reason)) = LITE_VOID_REASONS
            .iter()
            .find(|(name, _)| is_text(lite_verdict, name))
        {
            let source = lite_map
                .get("lite_verdict_source")
                .cloned()
                .ok_or_else(|| key_error("lite_verdict_source"))?;
            let explanation = if is_text(lite_verdict, "server_failed") {
                lit!("서버가 응답했지만 실패했다(실사용 불가 · 재발동 제안 대상)")
            } else {
                lit!("요청이 서버에 닿지 못했다(하네스 결함 · cap 차감 ✗)")
            };
            return Ok(cell_result(
                PyValue::Str(lit!(OUTCOME_MEASUREMENT_VOID)),
                PyValue::str(void_reason),
                source,
                kill_hits.clone(),
                Text::new()
                    .fixed(&lit!("서빙은 성립했으나 lite 게이트가 불통과였다(rc="))
                    .val(measure_rc)
                    .fixed(&lit!(" · lite_verdict="))
                    .val(lite_verdict)
                    .fixed(&lit!(") — "))
                    .fixed(&explanation)
                    .fixed(&lit!(". 구간 안에 집행된 사살은 없다."))
                    .done(),
            ));
        }
        let source = miss_source(correlation, events_scanned)?;
        return Ok(cell_result(
            PyValue::Str(lit!(OUTCOME_MEASUREMENT_VOID)),
            PyValue::Str(lit!("unknown")),
            text_value(source),
            kill_hits.clone(),
            Text::new()
                .fixed(&lit!("서빙은 성립했으나 측정이 실패했고(rc="))
                .val(measure_rc)
                .fixed(&lit!(") "))
                .fixed(&if unseen {
                    lit!("대조 대상 노드 중 구간을 보지 못한 노드가 있다")
                } else {
                    lit!("구간 안에 집행된 사살 이벤트가 없다")
                })
                .fixed(&lit!(" — 사인을 추측하지 않는다."))
                .done(),
        ));
    }
    let note = if executed.is_empty() {
        lit!("측정 성립.")
    } else {
        PyStr::new(&format!(
            "측정 성립. 다만 같은 구간에 집행된 사살 이벤트가 {}건 있다 — 대상이 이 셀이 아니었을 수 있으나 기록으로 남긴다.",
            executed.len()
        ))
    };
    Ok(cell_result(
        PyValue::Str(lit!(OUTCOME_MEASURED)),
        PyValue::None,
        PyValue::None,
        kill_hits.clone(),
        note,
    ))
}

/// `first["kind"]`, `first["ts"]` of the first carried-out kill (the `KeyError` of a hit without a time).
fn first_kill<'a>(executed: &[&'a PyValue]) -> Result<(&'a PyValue, &'a PyValue), Failure> {
    let first = executed[0];
    Ok((subscript(first, "kind")?, subscript(first, "ts")?))
}

/// The five-key cell close in the original's key order.
fn cell_result(
    outcome: PyValue,
    void_reason: PyValue,
    void_reason_source: PyValue,
    kill_events: PyValue,
    note: PyStr,
) -> PyValue {
    PyValue::Dict(PyDict::from_distinct_entries([
        (lit!("cell_outcome"), outcome),
        (lit!("void_reason"), void_reason),
        (lit!("void_reason_source"), void_reason_source),
        (lit!("kill_events"), kill_events),
        (lit!("note"), text_value(note)),
    ]))
}

/// `{}` for `(lite or {})`.
static EMPTY_DICT: std::sync::LazyLock<PyValue> =
    std::sync::LazyLock::new(|| PyValue::Dict(PyDict::new()));

fn or_none_ref(value: Option<&PyValue>) -> &PyValue {
    static NONE: PyValue = PyValue::None;
    value.unwrap_or(&NONE)
}

// ── node plan and scan ──────────────────────────────────────────────────────────

/// `_read_events(paths)`: every readable events file's lines (universal newlines, undecodable bytes replaced) and the
/// files that were read. An `OSError` is warned about on stderr and the file is left out; it is never read as "no
/// events".
pub fn read_events(
    paths: &[PyValue],
    diagnostics: &mut Diagnostics,
) -> Result<(Vec<PyValue>, Vec<PyValue>), Failure> {
    let mut lines: Vec<PyValue> = Vec::new();
    let mut scanned: Vec<PyValue> = Vec::new();
    for path in paths {
        // `open(path, encoding="utf-8", errors="replace")`: a string is a path, an integer a descriptor. A failure is
        // kept as the `str(OSError)` the warning prints.
        let read: Result<Vec<u8>, String> = match open_target(path)? {
            OpenTarget::Path(text) => {
                let os = os_string(&text).map_err(|error| error.into_failure())?;
                std::fs::read(std::path::Path::new(&os))
                    .map_err(|error| crate::cli::os_error_text(&error, &text))
            }
            OpenTarget::Descriptor(descriptor) => {
                read_descriptor(descriptor).map_err(|error| descriptor_error_text(&error))
            }
        };
        match read {
            Ok(bytes) => {
                let text = decode_replacing_with_newlines(&bytes);
                lines.extend(split_lines_keeping_newlines(&text));
                scanned.push(path.clone());
            }
            Err(why) => {
                diagnostics.write(
                    &Text::new()
                        .lit("[classify_cell] WARN events 판독 실패(")
                        .val(path)
                        .lit("): ")
                        .lit(&why)
                        .lit("\n")
                        .done(),
                );
            }
        }
    }
    Ok((lines, scanned))
}

/// `str(OSError)` of an error that names no file (`[Errno 9] Bad file descriptor`).
fn descriptor_error_text(error: &std::io::Error) -> String {
    let detail = error.to_string();
    let message = detail.split(" (os error").next().unwrap_or(&detail);
    match error.raw_os_error() {
        Some(errno) => format!("[Errno {errno}] {message}"),
        None => message.to_string(),
    }
}

/// `bytes.decode("utf-8", errors="replace")` with universal newlines (`\r\n` and `\r` become `\n`).
fn decode_replacing_with_newlines(bytes: &[u8]) -> PyStr {
    let decoded = String::from_utf8_lossy(bytes);
    let mut points: Vec<u32> = Vec::with_capacity(decoded.len());
    let mut chars = decoded.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            points.push(u32::from('\n'));
        } else {
            points.push(u32::from(c));
        }
    }
    PyStr::from_code_points(points)
}

/// `file.readlines()`: lines end at `\n` and keep it.
fn split_lines_keeping_newlines(text: &PyStr) -> Vec<PyValue> {
    let points = text.code_points();
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, &point) in points.iter().enumerate() {
        if point == '\n' as u32 {
            lines.push(PyValue::Str(PyStr::from_code_points(
                points[start..=index].to_vec(),
            )));
            start = index + 1;
        }
    }
    if start < points.len() {
        lines.push(PyValue::Str(PyStr::from_code_points(
            points[start..].to_vec(),
        )));
    }
    lines
}

/// The `TypeError` of `os.path.join(repo, *parts, node, ...)` for a non-string argument: the first argument is checked by
/// `os.fspath` itself, a later one by `genericpath._check_arg_types` (which names its class with `repr`).
fn join_parts_error(first: &PyValue, node: &PyValue) -> Failure {
    match (first.peel(), node.peel()) {
        (PyValue::Str(_), _) => type_error(&format!(
            "join() argument must be str, bytes, or os.PathLike object, not '{}'",
            node.type_name()
        )),
        _ => type_error(&format!(
            "expected str, bytes or os.PathLike object, not {}",
            first.type_name()
        )),
    }
}

/// `read(node, remote)` of `scan_node_events`: the node's events files and their lines.
fn read_node(
    repo: &PyValue,
    node: &PyValue,
    remote: bool,
    diagnostics: &mut Diagnostics,
) -> Result<PyDict, Failure> {
    let (PyValue::Str(repo_text), PyValue::Str(node_text)) = (repo.peel(), node.peel()) else {
        return Err(join_parts_error(repo, node));
    };
    let locations: Vec<&[&str]> = if remote {
        vec![&EVENTS_RECOVERED_PARTS[..], &EVENTS_LOCAL_PARTS[..]]
    } else {
        vec![&EVENTS_LOCAL_PARTS[..]]
    };
    let mut paths: Vec<PyStr> = Vec::new();
    for parts in locations {
        let mut found: Vec<PyStr> = Vec::new();
        let mut tail: Vec<&PyStr> = Vec::new();
        let owned: Vec<PyStr> = parts.iter().map(|part| PyStr::new(part)).collect();
        tail.extend(owned.iter());
        let events = PyStr::new("events");
        let pattern_name = PyStr::new("*.jsonl");
        tail.push(node_text);
        tail.push(&events);
        tail.push(&pattern_name);
        let pattern = ospath::join(repo_text, &tail);
        for candidate in ospath::glob(&pattern) {
            if ospath::isfile(&candidate) {
                found.push(candidate);
            }
        }
        found.sort_by(|a, b| a.code_points().cmp(b.code_points()));
        paths.extend(found);
    }
    let path_values: Vec<PyValue> = paths.into_iter().map(PyValue::Str).collect();
    let (lines, got) = read_events(&path_values, diagnostics)?;
    let mut scan = PyDict::with_capacity(6);
    scan.set("node", node.clone());
    scan.set("remote", PyValue::Bool(remote));
    scan.set("files", PyValue::List(got));
    scan.set("lines", PyValue::List(lines));
    scan.set("repo", repo.clone());
    Ok(scan)
}

/// `scan_node_events(repo, plan)`: reads each planned node's events. When the plan could not be determined only this
/// node's local records are read, and the rest is one `(미정)` entry that is `not_scanned` - never `miss`.
pub fn scan_node_events(
    repo: &PyValue,
    plan: &PyValue,
    diagnostics: &mut Diagnostics,
) -> Result<PyValue, Failure> {
    let mut scans: Vec<PyValue> = Vec::new();
    let Some(plan_map) = dict_of(plan) else {
        return Ok(PyValue::List(scans));
    };
    if truthy(plan_map.get("undeterminable")) {
        let self_node = or_none(plan_map.get("self_node"));
        if path_segment(&self_node) {
            let mut scan = read_node(repo, &self_node, false, diagnostics)?;
            scan.set("fallback", PyValue::Bool(true));
            scans.push(PyValue::Dict(scan));
        }
        scans.push(dict_from(vec![
            ("node", PyValue::str(UNIDENTIFIED_NODE)),
            ("remote", PyValue::Bool(true)),
            ("unidentified", PyValue::Bool(true)),
            ("files", PyValue::List(Vec::new())),
            ("lines", PyValue::List(Vec::new())),
            ("repo", repo.clone()),
        ]));
        return Ok(PyValue::List(scans));
    }
    let nodes = or_none(plan_map.get("nodes"));
    let nodes = if nodes.is_truthy() {
        nodes
    } else {
        PyValue::List(Vec::new())
    };
    for node in iterate(&nodes)? {
        let self_node = or_none(plan_map.get("self_node"));
        let remote = !equals(&node, &self_node);
        scans.push(PyValue::Dict(read_node(repo, &node, remote, diagnostics)?));
    }
    Ok(PyValue::List(scans))
}

/// The plan under construction in `events_node_plan` (the same dict the original mutates).
struct Plan(PyDict);

impl Plan {
    fn new(topology: &PyValue, manifest_path: &PyValue) -> Self {
        let mut map = PyDict::with_capacity(8);
        map.set("topology", topology.clone());
        map.set("nodes", PyValue::List(Vec::new()));
        map.set("self_node", PyValue::None);
        map.set("self_node_source", PyValue::None);
        map.set("source", PyValue::None);
        map.set("undeterminable", PyValue::None);
        map.set("excluded_rule", PyValue::None);
        map.set("manifest", manifest_path.clone());
        Plan(map)
    }
}

/// The answer of the node-identity resolver, asked at most once per plan.
struct SelfIdentity<'a> {
    owners: &'a Owners,
    repo: &'a PyValue,
    answer: Option<(Option<PyStr>, PyStr)>,
}

impl SelfIdentity<'_> {
    fn get(&mut self) -> Result<(Option<PyStr>, PyStr), Failure> {
        if self.answer.is_none() {
            self.answer = Some(self.owners.resolve_self_node(self.repo)?);
        }
        Ok(self.answer.clone().expect("just set"))
    }
}

/// `events_node_plan(topology, measured_node, measured_node_source, manifest_path, repo)`: which nodes' events a
/// correlation must read. The plan's `undeterminable` is set when none can be named; even then `self_node` (when the
/// resolver names it) is read, because this node's records share the host clock.
pub fn events_node_plan(
    topology: &PyValue,
    measured_node: &PyValue,
    measured_node_source: &PyValue,
    manifest_path: &PyValue,
    repo: &PyValue,
    owners: &Owners,
) -> Result<PyValue, Failure> {
    let mut plan = Plan::new(topology, manifest_path);
    let mut identity = SelfIdentity {
        owners,
        repo,
        answer: None,
    };
    let node_value = |node: &Option<PyStr>| node.clone().map_or(PyValue::None, PyValue::Str);

    // `bad(reason)`
    macro_rules! bad {
        ($reason:expr) => {{
            plan.0.set("undeterminable", text_value($reason));
            let (node, source) = identity.get()?;
            let named = node_value(&node);
            plan.0.set(
                "self_node",
                if path_segment(&named) {
                    named
                } else {
                    PyValue::None
                },
            );
            plan.0.set("self_node_source", text_value(source));
            return Ok(PyValue::Dict(plan.0));
        }};
    }

    if is_text(topology, "single") {
        let given = !matches!(measured_node.peel(), PyValue::None);
        let (node, source): (PyValue, PyStr) = if given {
            let note = if measured_node_source.is_truthy() {
                measured_node_source.clone()
            } else {
                PyValue::str("출처 미기재")
            };
            (
                measured_node.clone(),
                Text::new()
                    .lit("sweep_index.meta.measured_node=")
                    .val(measured_node)
                    .lit("(")
                    .val(&note)
                    .lit(")")
                    .done(),
            )
        } else {
            let (node, why) = identity.get()?;
            (
                node_value(&node),
                Text::new().lit("이 노드 — ").str(&why).done(),
            )
        };
        if is_text(&node, MEASURED_NODE_CLUSTER) || !path_segment(&node) {
            bad!(Text::new()
                .lit("single 인데 측정 노드를 정하지 못했다(")
                .lit(&node.repr())
                .lit(" ← ")
                .str(&source)
                .lit(")")
                .done());
        }
        plan.0.set("nodes", PyValue::List(vec![node.clone()]));
        plan.0.set("self_node", node);
        plan.0.set("self_node_source", text_value(source.clone()));
        plan.0.set(
            "source",
            text_value(
                Text::new()
                    .lit("single: 측정 노드만 — ")
                    .str(&source)
                    .done(),
            ),
        );
        plan.0.set(
            "excluded_rule",
            PyValue::str("single 의 노드는 각자 독립 서빙이다 — 다른 노드(docs/logs/<다른 노드>)의 사살은 이 측정의 사인이 아니므로 대조하지 않는다"),
        );
        return Ok(PyValue::Dict(plan.0));
    }
    if is_text(topology, "multi") {
        if !matches!(measured_node.peel(), PyValue::None)
            && !is_text(measured_node, MEASURED_NODE_CLUSTER)
        {
            bad!(Text::new()
                .lit("multi 인데 measured_node=")
                .lit(&measured_node.repr())
                .lit(" — 분산 서빙의 측정 정체성은 쌍(")
                .lit(MEASURED_NODE_CLUSTER)
                .lit(")이다")
                .done());
        }
        let Some(mut contract) = owners.open_role_contract()? else {
            bad!(PyStr::new(
                "node_role_contract 부재 — multi 참여 노드를 판정할 소유자가 없다"
            ));
        };
        let manifest_is_file = manifest_path.is_truthy() && isfile_value(manifest_path)?;
        if !manifest_is_file {
            bad!(Text::new()
                .lit("manifest 부재(")
                .val(manifest_path)
                .lit(") — 참여 노드를 읽을 사실이 없다")
                .done());
        }
        let manifest_text = ospath::fspath(manifest_path)?;
        let doc = match contract.load_yaml(&manifest_text)? {
            Ok(doc) => doc,
            Err(raised) => bad!(Text::new()
                .lit("manifest 판독 실패(")
                .val(manifest_path)
                .lit("): ")
                .str(&raised.message)
                .done()),
        };
        if !doc.is_dict() {
            bad!(Text::new()
                .lit("manifest 가 객체가 아니다(")
                .val(manifest_path)
                .lit(")")
                .done());
        }
        // An exception of `evaluate_manifest` is not caught by the original: it propagates. The documents stay with the
        // owner; only what the next lines read is fetched.
        let result = contract.evaluate_manifest("multi")??;
        if !result.is_dict() {
            return Err(Failure::new(
                "AttributeError",
                &format!("'{}' object has no attribute 'get'", result.type_name()),
            ));
        }
        let violations = contract.get(&result, "violations")?;
        if violations.as_ref().is_some_and(Descriptor::is_truthy) {
            let violations = violations.expect("checked");
            let mut codes: Vec<PyValue> = Vec::new();
            for violation in contract.items(&violations)? {
                if !violation.is_dict() {
                    return Err(Failure::new(
                        "AttributeError",
                        &format!("'{}' object has no attribute 'get'", violation.type_name()),
                    ));
                }
                codes.push(match contract.get(&violation, "code")? {
                    Some(code) => code.type_only(),
                    None => PyValue::str("?"),
                });
            }
            bad!(Text::new()
                .lit("node_role_contract 위반: ")
                .str(&join_values("; ", &codes)?)
                .done());
        }
        let nodes = contract.get(&doc, "nodes")?;
        let node_items = match nodes {
            Some(nodes) if nodes.is_truthy() => contract.items(&nodes)?,
            _ => Vec::new(),
        };
        let mut roles: Vec<PyValue> = Vec::new();
        for node in node_items {
            if node.is_dict() {
                roles.push(match contract.get(&node, "role")? {
                    // A tuple's content decides hashing and equality; a list or dict is unhashable whatever it holds.
                    Some(role @ Descriptor::Container { kind: "tuple", .. }) => {
                        contract.materialize(&role)?
                    }
                    Some(role) => role.type_only(),
                    None => PyValue::None,
                });
            }
        }
        for role in &roles {
            ensure_hashable(role)?;
        }
        let distinct = roles
            .iter()
            .enumerate()
            .filter(|(at, role)| roles[..*at].iter().all(|earlier| !equals(earlier, role)))
            .count();
        if roles.is_empty() || distinct != roles.len() || !roles.iter().all(path_segment) {
            bad!(Text::new()
                .lit("manifest nodes[].role 로 참여 노드를 정할 수 없다(")
                .lit(&PyValue::List(roles.clone()).repr())
                .lit(" — node_id = role 슬러그 · 비었거나 중복)")
                .done());
        }
        let (self_node, self_source) = identity.get()?;
        let self_value = node_value(&self_node);
        let joined = join_values(",", &roles)?;
        if !roles.iter().any(|role| equals(&self_value, role)) {
            bad!(Text::new()
                .lit("이 노드(")
                .lit(&self_value.repr())
                .lit(" ← ")
                .str(&self_source)
                .lit(")가 manifest nodes[].role(")
                .str(&joined)
                .lit(")에 없다 — 어느 노드의 events 가 이 저장소의 것인지 모른다")
                .done());
        }
        plan.0.set("nodes", PyValue::List(roles));
        plan.0.set("self_node", self_value);
        plan.0.set("self_node_source", text_value(self_source));
        plan.0.set(
            "source",
            text_value(
                Text::new()
                    .lit("multi: 서빙 참여 노드 전부 — manifest nodes[].role(")
                    .str(&joined)
                    .lit(") · node_role_contract 위반 0")
                    .done(),
            ),
        );
        return Ok(PyValue::Dict(plan.0));
    }
    bad!(Text::new()
        .lit("topology=")
        .lit(&topology.repr())
        .lit(" — 대조 대상 노드 규칙이 없다(single|multi)")
        .done())
}

/// `os.path.isfile(value)` for a value of any type: a string is a path, an integer is a descriptor, others raise.
fn isfile_value(value: &PyValue) -> Result<bool, Failure> {
    match value.peel() {
        PyValue::Str(path) => Ok(ospath::isfile(path)),
        PyValue::Int(_) | PyValue::Bool(_) => {
            let descriptor = match value.peel() {
                PyValue::Bool(flag) => Some(i64::from(*flag)),
                PyValue::Int(number) => num_traits::ToPrimitive::to_i64(number),
                _ => None,
            };
            Ok(descriptor
                .and_then(|fd| std::fs::metadata(format!("/proc/self/fd/{fd}")).ok())
                .is_some_and(|data| data.is_file()))
        }
        other => Err(type_error(&format!(
            "stat: path should be string, bytes, os.PathLike or integer, not {}",
            other.type_name()
        ))),
    }
}

// ── classify_bench_mode ─────────────────────────────────────────────────────────

/// What `classify_bench_mode` is given beyond the sweep index and the explicit events (each as the caller gave it).
pub struct BenchModeRequest<'a> {
    pub index: &'a PyValue,
    pub event_lines: &'a PyValue,
    pub events_scanned: &'a PyValue,
    pub tolerance_s: &'a PyValue,
    pub node_scans: &'a PyValue,
    pub plan: &'a PyValue,
    pub cross_node_tolerance_s: &'a PyValue,
    pub cross_node_tolerance_source: &'a PyValue,
}

/// `classify_bench_mode(...)`: the `bench_mode` (full or lite) of a sweep from its raw `runs[]`, the place the sweep
/// stopped, and the blackbox events of that place's time window. Post-hoc, like `void_reason`:
///
/// 1. the repeat summary is **recomputed** from the raw runs (the index's own summary is not trusted);
/// 2. a repeat condition that cannot be judged is not judged (`null`);
/// 3. a place the sweep stopped is correlated with the events: a carried-out kill means lite (`blackbox_kill`);
/// 4. otherwise the repeat condition decides: met -> full, broken -> lite (`run_failed`); and `full` also needs the
///    measurement tool to be the full-bench tool when the index names one.
pub fn classify_bench_mode(request: &BenchModeRequest<'_>) -> Result<PyValue, Failure> {
    let index = request.index;
    let levels = dict_of(index).and_then(|map| map.get("levels"));
    let Some(map) = dict_of(index).filter(
        |_| matches!(levels.map(PyValue::peel), Some(PyValue::List(items)) if !items.is_empty()),
    ) else {
        return Ok(plain_record(
            PyValue::None,
            PyValue::str("not_evaluated(sweep_index levels 부재 — 측정 레벨이 없다)"),
        ));
    };
    let levels = map.get("levels").expect("checked above");
    static NO_DECLARATION: std::sync::LazyLock<PyDict> = std::sync::LazyLock::new(PyDict::new);
    let declared: &PyDict = match map.get("repetition").and_then(dict_of) {
        Some(declared) => declared,
        None => &NO_DECLARATION,
    };
    let summary = summarize(&SummarizeRequest {
        levels,
        requested: or_none_ref(declared.get("requested")),
        requested_source: or_none_ref(declared.get("requested_source")),
        kind: or_none_ref(declared.get("kind")),
        clamp_run: or_none_ref(declared.get("clamp_run")),
    })?
    .to_value();
    let verdict = repetition_established(&summary, or_none_ref(map.get("verdict_point_level")))?;
    let why = verdict.reason.clone();
    let Some(established) = verdict.established else {
        return Ok(BenchModeFields {
            repetition: summary,
            ..BenchModeFields::new(PyValue::None, text_value(why))
        }
        .into_record());
    };

    // `scanned_txt`: the explicit files list as text ("none" when empty), or the caller's own truthy value.
    let scanned_txt: PyValue = match request.events_scanned.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            if items.is_empty() {
                PyValue::str("none")
            } else {
                text_value(join_values(",", items)?)
            }
        }
        other if other.is_truthy() => other.clone(),
        _ => PyValue::str("none"),
    };
    let scans_given = !matches!(request.node_scans.peel(), PyValue::None);
    let node_scans: PyValue = if scans_given {
        request.node_scans.clone()
    } else {
        let lines = if request.event_lines.is_truthy() {
            PyValue::List(iterate(request.event_lines)?)
        } else {
            PyValue::List(Vec::new())
        };
        let files = if is_text(&scanned_txt, "none") {
            PyValue::List(Vec::new())
        } else {
            match scanned_txt.peel() {
                PyValue::Str(text) => PyValue::List(split_on_comma(text)),
                other => {
                    return Err(Failure::new(
                        "AttributeError",
                        &format!("'{}' object has no attribute 'split'", other.type_name()),
                    ))
                }
            }
        };
        PyValue::List(vec![PyValue::Dict(PyDict::from_distinct_entries([
            (lit!("node"), PyValue::Str(lit!("explicit"))),
            (lit!("remote"), PyValue::Bool(false)),
            (lit!("lines"), lines),
            (lit!("files"), files),
        ]))])
    };
    let plan_is_dict = dict_of(request.plan).is_some();
    let plan_value = if plan_is_dict {
        request.plan.clone()
    } else {
        PyValue::Dict(PyDict::from_distinct_entries([(
            lit!("source"),
            PyValue::Str(lit!(
                "explicit(호출자가 대상 events 를 선언했다 — 노드 계획 없음)"
            )),
        )]))
    };
    let mut scan_doc = PyDict::from_distinct_entries([
        (lit!("plan"), plan_value),
        (lit!("nodes"), PyValue::None),
        (
            lit!("cross_node_tolerance_s"),
            request.cross_node_tolerance_s.clone(),
        ),
        (
            lit!("cross_node_tolerance_source"),
            request.cross_node_tolerance_source.clone(),
        ),
    ]);

    let stop = or_none_ref(dict_of(&summary).and_then(|map| map.get("stop")));
    let mut hits: Vec<PyValue> = Vec::new();
    let mut correlation = CORRELATION_NOT_APPLICABLE;
    let mut window: Option<PyStr> = None;
    let mut signal: Option<PyStr> = None;
    if let Some(stop_map) = dict_of(stop) {
        let part = |name: &str| or_none_ref(stop_map.get(name));
        signal = Some(
            Text::new()
                .fixed(&lit!("stop["))
                .val(part("kind"))
                .fixed(&lit!(" · level="))
                .val(part("level"))
                .fixed(&lit!(" · run="))
                .val(part("run"))
                .fixed(&lit!("]("))
                .val(part("detail"))
                .fixed(&lit!(")"))
                .done(),
        );
        window = Some(
            Text::new()
                .fixed(&lit!("["))
                .val(part("window_start_utc"))
                .fixed(&lit!(", "))
                .val(part("window_end_utc"))
                .fixed(&lit!("] tol="))
                .val(request.tolerance_s)
                .fixed(&lit!("s"))
                .done(),
        );
        let bounds = parse_utc(part("window_start_utc"), "stop.window_start_utc").and_then(|lo| {
            parse_utc(part("window_end_utc"), "stop.window_end_utc").map(|hi| (lo, hi))
        });
        match bounds {
            Err(error) if is_classify_error(&error) => {
                correlation = CORRELATION_UNAVAILABLE;
                window = Some(
                    Text::new()
                        .lit("창 시각 판독 불가: ")
                        .str(&error.message)
                        .done(),
                );
            }
            Err(error) => return Err(error),
            Ok((lo, hi)) => {
                let (found, aggregate, per_node) = correlate_nodes(
                    &node_scans,
                    Moment::At(lo),
                    Moment::At(hi),
                    request.tolerance_s,
                    request.cross_node_tolerance_s,
                    request.cross_node_tolerance_source,
                )?;
                hits = found;
                correlation = aggregate;
                scan_doc.insert(lit!("nodes"), PyValue::List(per_node));
            }
        }
    }
    let scan_doc = PyValue::Dict(scan_doc);
    let scope: Option<PyStr> = if plan_is_dict {
        match events_scan_summary(&scan_doc)? {
            PyValue::Str(text) => Some(text),
            _ => None,
        }
    } else {
        None
    };
    let scope_tail = |scope: &Option<PyStr>| -> PyStr {
        match scope {
            Some(text) if !text.is_empty() => Text::new().lit(" · ").str(text).done(),
            _ => PyStr::default(),
        }
    };
    let window_text = window.unwrap_or_else(|| lit!("None"));
    let signal_text = signal.unwrap_or_else(|| lit!("None"));
    let first_executed = hits.iter().find(|hit| {
        dict_of(hit)
            .and_then(|map| map.get("kind"))
            .is_some_and(|kind| KILL_EVENT_KINDS.iter().any(|name| is_text(kind, name)))
    });
    // The text that names the first carried-out kill, built while the hit is still borrowed from `hits`.
    let kill_source: Option<PyStr> = first_executed.map(|first| {
        let first_map = dict_of(first).expect("hits are dicts");
        let node_part = if plan_is_dict {
            Text::new()
                .fixed(&lit!(" · node="))
                .val(or_none_ref(first_map.get("node")))
                .done()
        } else {
            PyStr::default()
        };
        Text::new()
            .fixed(&lit!("events("))
            .val(&scanned_txt)
            .fixed(&lit!(") · "))
            .val(or_none_ref(first_map.get("kind")))
            .fixed(&lit!(" @ "))
            .val(or_none_ref(first_map.get("ts")))
            .str(&node_part)
            .fixed(&lit!(" ∈ "))
            .str(&window_text)
            .fixed(&lit!(" · "))
            .str(&signal_text)
            .str(&scope_tail(&scope))
            .done()
    });
    let trips = hits.len();
    let hits_value = PyValue::List(hits);
    if let Some(reason_source) = kill_source {
        return Ok(BenchModeFields {
            reason: PyValue::Str(lit!(DOWNGRADE_BLACKBOX_KILL)),
            reason_source: text_value(reason_source),
            repetition: summary,
            kill_hits: hits_value,
            correlation: correlation_value(correlation),
            events_scan: scan_doc,
            ..BenchModeFields::new(
                PyValue::Str(lit!(BENCH_MODE_LITE)),
                text_value(
                    Text::new()
                        .str(&why)
                        .fixed(&lit!(" · 멈춘 자리 창 안에 집행된 사살 — kill 이벤트는 레벨·run 순번과 무관한 강등 트리거"))
                        .done(),
                ),
            )
        }
        .into_record());
    }
    let tail = scope_tail(&scope);
    let note: PyStr = if correlation == CORRELATION_MISS {
        let mut text = Text::new()
            .fixed(&lit!("correlation-miss("))
            .val(&scanned_txt)
            .fixed(&lit!(" · 창 "))
            .str(&window_text);
        if trips > 0 {
            text = text.lit(&format!(" · 트립 {trips}건은 집행이 아니라 사인 불충분"));
        }
        text.str(&tail).fixed(&lit!(")")).done()
    } else if correlation == CORRELATION_NOT_SCANNED {
        if plan_is_dict {
            Text::new()
                .lit("not-scanned(대조 대상 노드 중 이 창을 덮는 기록을 판독하지 못한 노드가 있다 — 보지 못했음이지 사살 없음이 아니다 · 창 ")
                .str(&window_text)
                .str(&tail)
                .lit(")")
                .done()
        } else {
            Text::new()
                .lit("not-scanned(판독한 블랙박스 events 파일 0 — 사살 여부를 보지 못했다 · 창 ")
                .str(&window_text)
                .lit(")")
                .done()
        }
    } else if correlation == CORRELATION_UNAVAILABLE {
        let nodes_known = dict_of(&scan_doc)
            .and_then(|map| map.get("nodes"))
            .is_some_and(|nodes| !matches!(nodes.peel(), PyValue::None));
        if dict_of(stop).is_some() && nodes_known {
            Text::new()
                .lit("correlation-unavailable(원격 노드 시계 허용오차 미선언 — 미스를 결론내지 않는다 · 창 ")
                .str(&window_text)
                .str(&tail)
                .lit(")")
                .done()
        } else {
            Text::new()
                .lit("correlation-unavailable(")
                .str(&window_text)
                .lit(")")
                .done()
        }
    } else {
        PyStr::default()
    };

    // `full` is `lite ∪ GuideLLM × repeats >= 3`: an index that names a measurement tool other than the bound one is
    // not full, and is not a downgrade either, so it is not decided.
    let tool: &PyValue = or_none_ref(
        map.get("meta")
            .and_then(dict_of)
            .and_then(|meta| meta.get("bench_tool")),
    );
    let tool_unnamed =
        matches!(tool.peel(), PyValue::None) || is_text(tool, "") || is_text(tool, "NA");
    if established && !tool_unnamed && !is_text(tool, BENCH_TOOL) {
        return Ok(BenchModeFields {
            repetition: summary,
            kill_hits: hits_value,
            correlation: correlation_value(correlation),
            events_scan: scan_doc,
            ..BenchModeFields::new(
                PyValue::None,
                text_value(
                    Text::new()
                        .lit("undeterminable(측정 도구 ")
                        .val(tool)
                        .lit(" ≠ full-bench 바인딩 도구 ")
                        .lit(BENCH_TOOL)
                        .lit(" — 반복은 성립했으나 full 정의 불충족 · ")
                        .str(&why)
                        .lit(")")
                        .done(),
                ),
            )
        }
        .into_record());
    }
    if established {
        let source = if note.is_empty() {
            why
        } else {
            Text::new().str(&why).lit(" · ").str(&note).done()
        };
        return Ok(BenchModeFields {
            repetition: summary,
            kill_hits: hits_value,
            correlation: correlation_value(correlation),
            events_scan: scan_doc,
            ..BenchModeFields::new(PyValue::Str(lit!(BENCH_MODE_FULL)), text_value(source))
        }
        .into_record());
    }
    Ok(BenchModeFields {
        reason: PyValue::Str(lit!(DOWNGRADE_RUN_FAILED)),
        reason_source: text_value(
            Text::new()
                .str(&signal_text)
                .fixed(&lit!(" · "))
                .str(&note)
                .done(),
        ),
        repetition: summary,
        kill_hits: hits_value,
        correlation: correlation_value(correlation),
        events_scan: scan_doc,
        ..BenchModeFields::new(PyValue::Str(lit!(BENCH_MODE_LITE)), text_value(why))
    }
    .into_record())
}

/// The value of one of the correlation vocabulary words.
fn correlation_value(word: &str) -> PyValue {
    PyValue::Str(match word {
        CORRELATION_MATCHED => lit!(CORRELATION_MATCHED),
        CORRELATION_MISS => lit!(CORRELATION_MISS),
        CORRELATION_NOT_SCANNED => lit!(CORRELATION_NOT_SCANNED),
        CORRELATION_UNAVAILABLE => lit!(CORRELATION_UNAVAILABLE),
        _ => lit!(CORRELATION_NOT_APPLICABLE),
    })
}

/// `text.split(",")`.
fn split_on_comma(text: &PyStr) -> Vec<PyValue> {
    text.code_points()
        .split(|&point| point == ',' as u32)
        .map(|piece| PyValue::Str(PyStr::from_code_points(piece.to_vec())))
        .collect()
}

/// `generated_by` of every record this program writes.
pub const GENERATED_BY: &str = "classify_cell.py";

/// The constants of the module as one JSON object (what a bridge asks instead of copying them).
pub fn constants() -> PyValue {
    let mut lite_void = PyDict::new();
    for (name, reason) in LITE_VOID_REASONS {
        lite_void.set(name, PyValue::str(reason));
    }
    dict_from(vec![
        ("OUTCOME_SERVE_FAILED", PyValue::str(OUTCOME_SERVE_FAILED)),
        (
            "OUTCOME_MEASUREMENT_VOID",
            PyValue::str(OUTCOME_MEASUREMENT_VOID),
        ),
        ("OUTCOME_MEASURED", PyValue::str(OUTCOME_MEASURED)),
        ("OUTCOME_NOT_MEASURED", PyValue::str(OUTCOME_NOT_MEASURED)),
        ("KILL_EVENT_KINDS", str_list(&KILL_EVENT_KINDS)),
        ("TRIP_EVENT_KINDS", str_list(&TRIP_EVENT_KINDS)),
        ("BENCH_MODE_FULL", PyValue::str(BENCH_MODE_FULL)),
        ("BENCH_MODE_LITE", PyValue::str(BENCH_MODE_LITE)),
        ("BENCH_MODES", str_list(&BENCH_MODES)),
        ("DOWNGRADE_RUN_FAILED", PyValue::str(DOWNGRADE_RUN_FAILED)),
        (
            "DOWNGRADE_BLACKBOX_KILL",
            PyValue::str(DOWNGRADE_BLACKBOX_KILL),
        ),
        ("DOWNGRADE_REASONS", str_list(&DOWNGRADE_REASONS)),
        (
            "BENCH_MODE_SOURCE_DECLARED_PREFIX",
            PyValue::str(BENCH_MODE_SOURCE_DECLARED_PREFIX),
        ),
        (
            "BENCH_MODE_RECORD_NAME",
            PyValue::str(BENCH_MODE_RECORD_NAME),
        ),
        ("CORRELATION_MATCHED", PyValue::str(CORRELATION_MATCHED)),
        ("CORRELATION_MISS", PyValue::str(CORRELATION_MISS)),
        (
            "CORRELATION_NOT_SCANNED",
            PyValue::str(CORRELATION_NOT_SCANNED),
        ),
        (
            "CORRELATION_UNAVAILABLE",
            PyValue::str(CORRELATION_UNAVAILABLE),
        ),
        (
            "CORRELATION_NOT_APPLICABLE",
            PyValue::str(CORRELATION_NOT_APPLICABLE),
        ),
        ("DOWNGRADE_CORRELATIONS", str_list(&DOWNGRADE_CORRELATIONS)),
        ("EVENTS_LOCAL_PARTS", str_list(&EVENTS_LOCAL_PARTS)),
        ("EVENTS_RECOVERED_PARTS", str_list(&EVENTS_RECOVERED_PARTS)),
        ("MEASURED_NODE_CLUSTER", PyValue::str(MEASURED_NODE_CLUSTER)),
        ("NODE_SCAN_MATCHED", PyValue::str(NODE_SCAN_MATCHED)),
        ("NODE_SCAN_MISS", PyValue::str(NODE_SCAN_MISS)),
        ("NODE_SCAN_NOT_SCANNED", PyValue::str(NODE_SCAN_NOT_SCANNED)),
        ("NODE_SCAN_NOT_COVERED", PyValue::str(NODE_SCAN_NOT_COVERED)),
        (
            "NODE_SCAN_SKEW_UNDECLARED",
            PyValue::str(NODE_SCAN_SKEW_UNDECLARED),
        ),
        ("NODE_SCAN_STATUSES", str_list(&NODE_SCAN_STATUSES)),
        ("UNIDENTIFIED_NODE", PyValue::str(UNIDENTIFIED_NODE)),
        ("LITE_VOID_REASONS", PyValue::Dict(lite_void)),
    ])
}
