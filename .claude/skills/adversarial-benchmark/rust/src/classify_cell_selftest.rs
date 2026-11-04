//! `classify_cell --self-test`: the original's own checks, ported one for one and run for real.
//!
//! Every check **executes** the operation it names with the original's inputs - the pure classifier, the `bench_mode`
//! decision on synthetic sweep documents, the durable record read/write in a temporary directory, and the node-plan
//! and command-line scenarios against temporary repositories - and judges the answer; nothing here replays a stored
//! transcript. The check names printed are the original's (`classify_cell_selftest_names`, extracted from the original
//! program), so the transcript reads the same.
//!
//! The node scenarios ask the installed owners (`node_identity.sh`, and `node_role_contract.py` through the query
//! adapter) exactly where the original did; an unconfigured owner location is an error that ends the run, never a
//! silently shorter run. `K-mirror` reads the installed `fetch_sub_docs.sh`; the original skips that one check with a
//! `[SKIP]` line when the script is absent, because it is main-only and a sub node's delivered tree does not carry it.
//! That behaviour is kept (and printed), so a skipped `K-mirror` in a *main* checkout means the scripts directory is
//! wrong - and `F00` then fails as well.
//!
//! The figures are unit fixtures (invented timestamps and codes), never a measurement.

use crate::classify_cell::{
    bench_mode_kind, bench_mode_record_path, classify, classify_bench_mode, declared_lite_record,
    events_node_plan, kill_events_in_window, parse_utc, read_bench_mode_record, read_cell_lite,
    BenchModeFields, BenchModeRequest, Moment, BENCH_MODE_FULL, BENCH_MODE_LITE,
    BENCH_MODE_RECORD_NAME, CORRELATION_MATCHED, CORRELATION_MISS, CORRELATION_NOT_APPLICABLE,
    CORRELATION_NOT_SCANNED, CORRELATION_UNAVAILABLE, DOWNGRADE_BLACKBOX_KILL,
    DOWNGRADE_RUN_FAILED, EVENTS_LOCAL_PARTS, EVENTS_RECOVERED_PARTS, NODE_SCAN_MATCHED,
    NODE_SCAN_MISS, NODE_SCAN_NOT_COVERED, NODE_SCAN_NOT_SCANNED, NODE_SCAN_SKEW_UNDECLARED,
    OUTCOME_MEASURED, OUTCOME_MEASUREMENT_VOID, OUTCOME_SERVE_FAILED, TRIP_EVENT_KINDS,
    UNIDENTIFIED_NODE,
};
use crate::classify_cell_cli;
use crate::classify_cell_owner::Owners;
use crate::classify_cell_selftest_names::CHECK_NAMES;
use crate::cli::Streams;
use crate::guidellm::Failure;
use crate::ospath;
use crate::pydyn::{dict_of, is_text};
use crate::pyjson::{decode_file_text, loads_text};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::selftest::SelfTestReport;
use crate::stdout::StdoutProfile;
use crate::utc::UtcInstant;
use std::path::{Path, PathBuf};

// ── recorder ─────────────────────────────────────────────────────────────────────

struct Recorder {
    stdout: String,
    failures: Vec<String>,
}

impl Recorder {
    /// `check(name, condition, detail)`: one PASS or FAIL line. `id` selects the original's name.
    fn check(&mut self, id: &str, condition: bool, detail: impl FnOnce() -> String) {
        let name = CHECK_NAMES
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, name)| *name)
            .unwrap_or_else(|| panic!("no self-test check named {id}"));
        if condition {
            self.stdout.push_str(&format!("  [PASS] {name}\n"));
        } else {
            self.stdout
                .push_str(&format!("  [FAIL] {name} {}\n", detail()));
            self.failures.push(name.to_string());
        }
    }

    fn note(&mut self, line: &str) {
        self.stdout.push_str(&format!("  {line}\n"));
    }
}

// ── small value builders ─────────────────────────────────────────────────────────

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn n(number: i64) -> PyValue {
    PyValue::int(number)
}

fn obj(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::with_capacity(entries.len());
    for (key, value) in entries {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

fn arr(items: Vec<PyValue>) -> PyValue {
    PyValue::List(items)
}

fn texts(items: &[&str]) -> PyValue {
    PyValue::List(items.iter().map(|item| s(item)).collect())
}

/// `value[key]` as a value (`None` when absent: the checks only read keys that exist).
fn g(value: &PyValue, key: &str) -> PyValue {
    dict_of(value)
        .and_then(|map| map.get(key))
        .cloned()
        .unwrap_or(PyValue::None)
}

/// `value[a][b]...` through nested dicts.
fn at(value: &PyValue, path: &[&str]) -> PyValue {
    path.iter()
        .fold(value.clone(), |current, key| g(&current, key))
}

fn is_none(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::None)
}

fn eq_text(value: &PyValue, text: &str) -> bool {
    is_text(value, text)
}

fn starts_with(value: &PyValue, prefix: &str) -> bool {
    match value.peel() {
        PyValue::Str(text) => {
            let prefix: Vec<u32> = prefix.chars().map(u32::from).collect();
            text.code_points().starts_with(&prefix)
        }
        _ => false,
    }
}

fn contains(value: &PyValue, needle: &str) -> bool {
    match value.peel() {
        PyValue::Str(text) => {
            let needle: Vec<u32> = needle.chars().map(u32::from).collect();
            needle.is_empty()
                || text
                    .code_points()
                    .windows(needle.len())
                    .any(|window| window == needle.as_slice())
        }
        _ => false,
    }
}

fn len_of(value: &PyValue) -> usize {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items.len(),
        PyValue::Dict(map) => map.len(),
        _ => 0,
    }
}

fn shown(value: &PyValue) -> String {
    value.display_text().to_utf8().unwrap_or_default()
}

fn json_compact(value: &PyValue) -> String {
    value.dumps_ascii().unwrap_or_default()
}

fn instant(text: &str, field: &str) -> Result<UtcInstant, Failure> {
    parse_utc(&s(text), field)
}

fn failure_text(failure: &Failure) -> String {
    failure.line().to_utf8().unwrap_or_default()
}

/// `ev(kind, ts, mode)`: one event line as the blackbox writes it.
fn ev(kind: &str, ts: &str, mode: &str) -> String {
    json_compact(&obj(vec![
        ("ts", s(ts)),
        ("kind", s(kind)),
        ("source", s("mem_watchdog_eta")),
        ("mode", s(mode)),
        ("targets", s("abc")),
    ]))
}

fn armed(kind: &str, ts: &str) -> String {
    ev(kind, ts, "armed")
}

fn lines_of(items: &[String]) -> PyValue {
    PyValue::List(items.iter().map(|line| s(line)).collect())
}

fn hits_of(
    lines: &[String],
    started: UtcInstant,
    ended: UtcInstant,
) -> Result<Vec<PyValue>, Failure> {
    match kill_events_in_window(
        &lines_of(lines),
        Moment::At(started),
        Moment::At(ended),
        &n(0),
    )? {
        PyValue::List(items) => Ok(items),
        _ => Ok(Vec::new()),
    }
}

// ── the sweep documents of the D checks ──────────────────────────────────────────

/// `run(k, ok, tps, base)`: run `k` is at minute `base + k - 1`, from `:00` to `:50`.
fn run_doc(k: i64, ok: bool, tps: f64, base: i64) -> PyValue {
    obj(vec![
        ("run", n(k)),
        (
            "started_utc",
            s(&format!("2026-09-04T10:{:02}:00Z", k - 1 + base)),
        ),
        (
            "ended_utc",
            s(&format!("2026-09-04T10:{:02}:50Z", k - 1 + base)),
        ),
        ("run_bench_rc", n(0)),
        ("parse_rc", n(0)),
        ("measurement_ok", PyValue::Bool(ok)),
        ("decode_tps", PyValue::Float(tps)),
    ])
}

fn run_ok(k: i64) -> PyValue {
    run_doc(k, true, 30.0, 1)
}

fn level_doc(level: i64, runs: Vec<PyValue>) -> PyValue {
    obj(vec![
        ("level", n(level)),
        ("status", s("ok")),
        (
            "measured",
            obj(vec![
                ("measurement_ok", PyValue::Bool(true)),
                ("runs", arr(runs)),
            ]),
        ),
    ])
}

/// `index(*levels, requested, clamp_run, verdict_point)`.
fn index_doc(
    levels: Vec<PyValue>,
    requested: i64,
    clamp_run: Option<PyValue>,
    verdict_point: Option<i64>,
) -> PyValue {
    let mut repetition = vec![
        ("requested", n(requested)),
        ("requested_source", s("declared(fixture)")),
        ("kind", s("warm-rerun")),
    ];
    if let Some(clamp) = clamp_run {
        repetition.push(("clamp_run", clamp));
    }
    let mut doc = vec![("levels", arr(levels)), ("repetition", obj(repetition))];
    if let Some(point) = verdict_point {
        doc.push(("verdict_point_level", n(point)));
    }
    obj(doc)
}

fn plain_index(levels: Vec<PyValue>) -> PyValue {
    index_doc(levels, 3, None, Some(1))
}

fn with_key(doc: &PyValue, key: &str, value: PyValue) -> PyValue {
    let mut map = dict_of(doc).cloned().unwrap_or_default();
    map.set(key, value);
    PyValue::Dict(map)
}

/// `classify_bench_mode(index, lines, scanned, tolerance_s)` with no node scans (the explicit-events path).
fn bench_mode(
    index: &PyValue,
    lines: &[String],
    scanned: &PyValue,
    tolerance: i64,
) -> Result<PyValue, Failure> {
    let (event_lines, tolerance) = (lines_of(lines), n(tolerance));
    classify_bench_mode(&BenchModeRequest {
        index,
        event_lines: &event_lines,
        events_scanned: scanned,
        tolerance_s: &tolerance,
        node_scans: &PyValue::None,
        plan: &PyValue::None,
        cross_node_tolerance_s: &PyValue::None,
        cross_node_tolerance_source: &PyValue::None,
    })
}

// ── temporary directories and the in-process command line ────────────────────────

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        for attempt in 0..10_000_u32 {
            let path = base.join(format!(
                "{prefix}{}-{attempt}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.subsec_nanos())
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(TempDir { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::other("no temporary directory name"))
    }

    fn join(&self, relative: &str) -> String {
        self.path.join(relative).to_string_lossy().into_owned()
    }

    /// `put(rel, text)`: write a file (creating its directories) and return its path.
    fn put(&self, relative: &str, text: &str) -> std::io::Result<String> {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, text)?;
        Ok(path.to_string_lossy().into_owned())
    }

    fn remove(&self, relative: &str) -> std::io::Result<()> {
        std::fs::remove_file(self.path.join(relative))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The original's `main(argv)` called in process with its streams captured: `(exit status, stdout JSON or {})`.
fn cli(owners: &Owners, argv: &[&str]) -> (i32, PyValue) {
    let args: Vec<PyStr> = argv.iter().map(|item| PyStr::new(item)).collect();
    let (mut input, mut output, mut errors) =
        (std::io::empty(), Vec::<u8>::new(), Vec::<u8>::new());
    let status = classify_cell_cli::run(
        &args,
        owners,
        &mut Streams::new(
            &mut input,
            &mut output,
            &mut errors,
            StdoutProfile::default(),
        ),
    );
    let parsed = decode_file_text(&output)
        .ok()
        .and_then(|text| {
            if text.is_blank() {
                None
            } else {
                loads_text(&text).ok()
            }
        })
        .unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    (status, parsed)
}

fn node_status(record: &PyValue, node: &str) -> PyValue {
    match at(record, &["events_scan", "nodes"]).peel() {
        PyValue::List(entries) => entries
            .iter()
            .find(|entry| eq_text(&g(entry, "node"), node))
            .map_or(PyValue::None, |entry| g(entry, "status")),
        _ => PyValue::None,
    }
}

fn node_names(record: &PyValue) -> Vec<String> {
    match at(record, &["events_scan", "nodes"]).peel() {
        PyValue::List(entries) => entries
            .iter()
            .map(|entry| shown(&g(entry, "node")))
            .collect(),
        _ => Vec::new(),
    }
}

fn status_of(pair: &PyValue) -> PyValue {
    match pair.peel() {
        PyValue::Tuple(items) | PyValue::List(items) if items.len() == 2 => items[1].clone(),
        _ => PyValue::None,
    }
}

fn record_of(pair: &PyValue) -> PyValue {
    match pair.peel() {
        PyValue::Tuple(items) | PyValue::List(items) if items.len() == 2 => items[0].clone(),
        _ => PyValue::None,
    }
}

fn io_failure(error: &std::io::Error) -> Failure {
    Failure::new("OSError", &error.to_string())
}

// ── `K-mirror`: the fetch script's default destination ───────────────────────────

fn is_py_space(point: u32) -> bool {
    PyStr::from_code_points(vec![point]).is_blank()
}

/// `re.findall(r'^DEST="\$\{DEST:-\$\{SRC%/\}/([^}"]+)\}"\s*$', source, re.MULTILINE)`.
fn dest_defaults(source: &[char]) -> Vec<String> {
    let prefix: Vec<char> = "DEST=\"${DEST:-${SRC%/}/".chars().collect();
    let mut found = Vec::new();
    let mut from = 0;
    while from < source.len() {
        let at_line_start = from == 0 || source[from - 1] == '\n';
        if at_line_start && source[from..].starts_with(&prefix) {
            let start = from + prefix.len();
            let mut end = start;
            while end < source.len() && source[end] != '}' && source[end] != '"' {
                end += 1;
            }
            if end > start && source[end..].starts_with(&['}', '"']) {
                let mut cursor = end + 2;
                let mut matched_to = None;
                loop {
                    if cursor == source.len() || source[cursor] == '\n' {
                        matched_to = Some(cursor);
                        break;
                    }
                    if is_py_space(u32::from(source[cursor])) {
                        cursor += 1;
                    } else {
                        break;
                    }
                }
                if let Some(stop) = matched_to {
                    // Greedy `\s*` backs off to the last position where `$` holds.
                    let mut last = stop;
                    let mut probe = stop;
                    while probe < source.len() && is_py_space(u32::from(source[probe])) {
                        probe += 1;
                        if probe == source.len() || source[probe] == '\n' {
                            last = probe;
                        }
                    }
                    found.push(source[start..end].iter().collect());
                    from = last.max(end + 2);
                    continue;
                }
            }
        }
        from += 1;
    }
    found
}

/// `re.search(r'rsync -az [^\n]*"\$SUB_HOST:\$SUB_WORK_DIR/docs/" "\$DEST/"', source)`.
fn copies_docs(source: &str) -> bool {
    let tail = "\"$SUB_HOST:$SUB_WORK_DIR/docs/\" \"$DEST/\"";
    source.lines().any(|line| {
        line.find("rsync -az ")
            .is_some_and(|start| line[start + "rsync -az ".len()..].contains(tail))
    })
}

// ── the checks ───────────────────────────────────────────────────────────────────

/// Runs the checks. The transcript, the failure summary and the status are the original's; an uncaught exception ends
/// the run with its traceback line and status 1.
pub fn run_self_test(owners: &Owners) -> SelfTestReport {
    let mut recorder = Recorder {
        stdout: String::new(),
        failures: Vec::new(),
    };
    if let Err(failure) = run_checks(&mut recorder, owners) {
        return SelfTestReport {
            stdout: recorder.stdout,
            stderr: format!(
                "{}{}\n",
                crate::cli::TRACEBACK_HEADER,
                failure_text(&failure)
            ),
            exit_code: 1,
        };
    }
    if !recorder.failures.is_empty() {
        let listed: Vec<String> = recorder
            .failures
            .iter()
            .map(|name| PyStr::new(name).repr())
            .collect();
        return SelfTestReport {
            stdout: recorder.stdout,
            stderr: format!(
                "[classify_cell --self-test] FAIL {} 건: [{}]\n",
                recorder.failures.len(),
                listed.join(", ")
            ),
            exit_code: 1,
        };
    }
    recorder.stdout.push_str(
        "[classify_cell --self-test] OK — K1~K3 · C1~C9 · L1~L7(lite 판정 운반) · D1~D19(bench_mode 확정 · 실패주입 · 음성대조) · E1~E6 · K-mirror · N1~N6 · F00 · F1~F11 · G1~G10(대조 대상 노드 · 이 노드 해소 · 계획 불성립 시 이 노드 기록) 전부 통과\n",
    );
    SelfTestReport {
        stdout: recorder.stdout,
        stderr: String::new(),
        exit_code: 0,
    }
}

fn run_checks(rec: &mut Recorder, owners: &Owners) -> Result<(), Failure> {
    let started = instant("2026-09-04T10:00:00Z", "s")?;
    let ended = instant("2026-09-04T10:30:00Z", "e")?;

    let lines = vec![
        armed("watchdog_trip", "2026-09-04T10:10:00Z"),
        armed("watchdog_kill_ack", "2026-09-04T10:10:01Z"),
        armed("watchdog_kill_ack", "2026-09-03T10:10:01Z"), // outside the window
        ev("watchdog_trip_dryrun", "2026-09-04T10:12:00Z", "dry-run"),
        ev("watchdog_kill_ack", "2026-09-04T10:13:00Z", "dry-run"), // observation only
        "not json at all".to_string(),
    ];
    let hits = hits_of(&lines, started, ended)?;
    let hits_value = arr(hits.clone());
    rec.check(
        "K1",
        hits.iter()
            .all(|hit| starts_with(&g(hit, "ts"), "2026-09-04")),
        String::new,
    );
    rec.check(
        "K2",
        hits.iter().all(|hit| !eq_text(&g(hit, "mode"), "dry-run")),
        String::new,
    );
    rec.check("K3", hits.len() == 2, String::new);

    let none = PyValue::None;
    let no_hits = arr(Vec::new());
    let events_main = s("docs/logs/main/events/2026-09.jsonl");

    let out = classify(&n(3), &n(0), &no_hits, &s("none"), &none, &none)?;
    rec.check(
        "C1",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_SERVE_FAILED) && is_none(&g(&out, "void_reason")),
        String::new,
    );

    // R4 (2026-09-11): the cause of a serve failure is answered by the time correlation as well.
    let out = classify(&n(3), &none, &hits_value, &events_main, &none, &none)?;
    rec.check(
        "C7",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_SERVE_FAILED)
            && eq_text(&g(&out, "void_reason"), "watchdog_kill_ack")
            && starts_with(&g(&out, "void_reason_source"), "events(")
            && contains(&g(&out, "note"), "호스트 평면의 사살"),
        String::new,
    );
    let out = classify(&n(3), &none, &no_hits, &events_main, &none, &none)?;
    rec.check(
        "C8",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_SERVE_FAILED)
            && is_none(&g(&out, "void_reason"))
            && starts_with(&g(&out, "void_reason_source"), "correlation-miss("),
        String::new,
    );
    let trip_hits: Vec<PyValue> = hits
        .iter()
        .filter(|hit| {
            TRIP_EVENT_KINDS
                .iter()
                .any(|kind| eq_text(&g(hit, "kind"), kind))
        })
        .cloned()
        .collect();
    let out = classify(&n(3), &none, &arr(trip_hits), &events_main, &none, &none)?;
    rec.check("C9", is_none(&g(&out, "void_reason")), String::new);

    let out = classify(&n(0), &n(4), &hits_value, &events_main, &none, &none)?;
    rec.check(
        "C2",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_MEASUREMENT_VOID)
            && eq_text(&g(&out, "void_reason"), "watchdog_kill_ack")
            && starts_with(&g(&out, "void_reason_source"), "events("),
        String::new,
    );
    let out = classify(&n(0), &n(4), &no_hits, &events_main, &none, &none)?;
    rec.check(
        "C3",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_MEASUREMENT_VOID)
            && eq_text(&g(&out, "void_reason"), "unknown")
            && starts_with(&g(&out, "void_reason_source"), "correlation-miss("),
        String::new,
    );
    let out = classify(&n(0), &n(0), &no_hits, &s("none"), &none, &none)?;
    rec.check(
        "C4",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_MEASURED),
        String::new,
    );

    lite_checks(rec, started, &hits_value)?;

    let out = classify(&n(0), &n(0), &hits_value, &s("x"), &none, &none)?;
    rec.check(
        "C5",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_MEASURED)
            && contains(&g(&out, "note"), "사살 이벤트가"),
        String::new,
    );
    let trips_only = hits_of(
        &[armed("watchdog_trip", "2026-09-04T10:10:00Z")],
        started,
        ended,
    )?;
    let out = classify(&n(0), &n(4), &arr(trips_only), &s("x"), &none, &none)?;
    rec.check(
        "C6",
        eq_text(&g(&out, "void_reason"), "unknown") && len_of(&g(&out, "kill_events")) == 1,
        String::new,
    );

    bench_mode_checks(rec)?;
    record_checks(rec, owners)?;
    node_checks(rec, owners)?;
    Ok(())
}

// ── L: lite verdict carry ────────────────────────────────────────────────────────

fn lite_checks(
    rec: &mut Recorder,
    started: UtcInstant,
    hits_value: &PyValue,
) -> Result<(), Failure> {
    let dir = TempDir::new("classify-cell-lite.").map_err(|error| io_failure(&error))?;
    let raw = |name: &str, verdict: &str, measured: &str| -> Result<PyValue, Failure> {
        let text = json_compact(&obj(vec![
            ("lite_verdict", s(verdict)),
            ("measured_utc", s(measured)),
        ]));
        let path = dir.put(name, &text).map_err(|error| io_failure(&error))?;
        Ok(s(&path))
    };
    let none = PyValue::None;
    let no_hits = arr(Vec::new());
    let started_some = Some(Moment::At(started));

    let server_failed = read_cell_lite(
        &raw("sf.json", "server_failed", "2026-09-04T10:05:00Z")?,
        started_some,
    )?;
    let out = classify(&n(0), &n(7), &no_hits, &s("none"), &none, &server_failed)?;
    rec.check(
        "L1",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_MEASUREMENT_VOID)
            && eq_text(&g(&out, "void_reason"), "lite_server_failed")
            && starts_with(&g(&out, "void_reason_source"), "lite_raw(")
            && eq_text(&g(&server_failed, "lite_raw_status"), "fresh"),
        || shown(&out),
    );
    let path_failed = read_cell_lite(
        &raw("mp.json", "measurement_path_failed", "2026-09-04T10:05:00Z")?,
        started_some,
    )?;
    let out = classify(&n(0), &n(6), &no_hits, &s("none"), &none, &path_failed)?;
    rec.check(
        "L2",
        eq_text(&g(&out, "void_reason"), "lite_measurement_path_failed"),
        || shown(&out),
    );
    let out = classify(&n(0), &n(7), hits_value, &s("x"), &none, &server_failed)?;
    rec.check(
        "L3",
        eq_text(&g(&out, "void_reason"), "watchdog_kill_ack"),
        || shown(&out),
    );
    let old = read_cell_lite(
        &raw("old.json", "server_failed", "2026-09-03T10:05:00Z")?,
        started_some,
    )?;
    let out = classify(&n(0), &n(7), &no_hits, &s("none"), &none, &old)?;
    rec.check(
        "L4",
        eq_text(&g(&old, "lite_raw_status"), "stale")
            && is_none(&g(&old, "lite_verdict"))
            && eq_text(&g(&out, "void_reason"), "unknown"),
        || shown(&out),
    );
    let outside = read_cell_lite(
        &raw("voc.json", "ok", "2026-09-04T10:05:00Z")?,
        started_some,
    )?;
    let out = classify(&n(0), &n(7), &no_hits, &s("none"), &none, &outside)?;
    rec.check("L5", eq_text(&g(&out, "void_reason"), "unknown"), || {
        shown(&out)
    });
    let absent = read_cell_lite(&s(&dir.join("nope.json")), started_some)?;
    let expected = obj(vec![
        ("lite_verdict", PyValue::None),
        ("lite_verdict_source", PyValue::None),
        ("lite_raw_status", s("absent")),
    ]);
    rec.check("L6", crate::pydyn::equals(&absent, &expected), || {
        shown(&absent)
    });
    let out = classify(&n(0), &n(0), &no_hits, &s("none"), &none, &server_failed)?;
    rec.check(
        "L7",
        eq_text(&g(&out, "cell_outcome"), OUTCOME_MEASURED),
        || shown(&out),
    );
    Ok(())
}

// ── D: bench_mode decision ───────────────────────────────────────────────────────

fn bench_mode_checks(rec: &mut Recorder) -> Result<(), Failure> {
    let events_files = arr(vec![s("docs/logs/main/events/2026-09.jsonl")]);
    let none_text = s("none");
    let ok3 = || {
        vec![
            run_ok(1),
            run_doc(2, true, 31.0, 1),
            run_doc(3, true, 29.0, 1),
        ]
    };
    let broken = || vec![run_ok(1), run_doc(2, false, 30.0, 1)];

    let out = bench_mode(&plain_index(vec![level_doc(1, ok3())]), &[], &none_text, 0)?;
    rec.check(
        "D1",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && is_none(&g(&out, "downgrade_reason"))
            && eq_text(
                &g(&out, "downgrade_correlation"),
                CORRELATION_NOT_APPLICABLE,
            )
            && bench_mode_kind(&out) == "full",
        || shown(&out),
    );

    let kill_in = vec![armed("watchdog_kill_ack", "2026-09-04T10:02:10Z")];
    // D-T (2026-10-26): repeats established, but the measurement tool must be the bound one for `full`.
    let indexed = with_key(
        &plain_index(vec![level_doc(1, ok3())]),
        "meta",
        obj(vec![("bench_tool", s("vllm-bench-serve"))]),
    );
    let out = bench_mode(&indexed, &[], &none_text, 0)?;
    rec.check(
        "D-T1",
        is_none(&g(&out, "bench_mode"))
            && contains(&g(&out, "bench_mode_source"), "vllm-bench-serve")
            && bench_mode_kind(&out) == "undetermined",
        || shown(&g(&out, "bench_mode_source")),
    );
    let indexed = with_key(&indexed, "meta", obj(vec![("bench_tool", s("guidellm"))]));
    let out = bench_mode(&indexed, &[], &none_text, 0)?;
    rec.check(
        "D-T2",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL),
        || shown(&g(&out, "bench_mode_source")),
    );

    let out = bench_mode(
        &plain_index(vec![level_doc(1, broken())]),
        &kill_in,
        &events_files,
        0,
    )?;
    rec.check(
        "D2",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_LITE)
            && eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && starts_with(&g(&out, "downgrade_reason_source"), "events(")
            && bench_mode_kind(&out) == "downgraded-lite"
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_MATCHED),
        || shown(&out),
    );

    let out = bench_mode(
        &plain_index(vec![level_doc(1, broken())]),
        &[armed("thermal_trip", "2026-09-04T10:02:10Z")],
        &events_files,
        0,
    )?;
    rec.check(
        "D3",
        eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_RUN_FAILED)
            && contains(&g(&out, "downgrade_reason_source"), "사인 불충분")
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_MISS),
        || shown(&out),
    );

    let outside = vec![armed("watchdog_kill_ack", "2026-09-04T10:05:00Z")];
    let out = bench_mode(
        &plain_index(vec![level_doc(1, broken())]),
        &outside,
        &events_files,
        0,
    )?;
    rec.check(
        "D4",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_LITE)
            && eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_RUN_FAILED)
            && len_of(&g(&out, "repetition_kill_events")) == 0
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_MISS),
        || shown(&out),
    );
    let out = bench_mode(
        &plain_index(vec![level_doc(1, broken())]),
        &outside,
        &events_files,
        200,
    )?;
    rec.check(
        "D4b",
        eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL),
        || shown(&out),
    );

    let out = bench_mode(
        &plain_index(vec![level_doc(1, broken())]),
        &[ev("watchdog_kill_ack", "2026-09-04T10:02:10Z", "dry-run")],
        &events_files,
        0,
    )?;
    rec.check(
        "D5",
        eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_RUN_FAILED),
        || shown(&out),
    );

    let wide = vec![
        run_doc(1, true, 10.0, 1),
        run_doc(2, true, 30.0, 1),
        run_doc(3, true, 50.0, 1),
    ];
    let out = bench_mode(&plain_index(vec![level_doc(1, wide)]), &[], &none_text, 0)?;
    rec.check(
        "D6",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL) && is_none(&g(&out, "downgrade_reason")),
        || shown(&out),
    );

    let out = bench_mode(
        &plain_index(vec![level_doc(1, ok3())]),
        &kill_in,
        &events_files,
        0,
    )?;
    rec.check(
        "D7",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(
                &g(&out, "downgrade_correlation"),
                CORRELATION_NOT_APPLICABLE,
            ),
        || shown(&out),
    );

    let no_runs = obj(vec![
        (
            "levels",
            arr(vec![obj(vec![
                ("level", n(1)),
                (
                    "measured",
                    obj(vec![("measurement_ok", PyValue::Bool(true))]),
                ),
            ])]),
        ),
        ("verdict_point_level", n(1)),
    ]);
    let out = bench_mode(&no_runs, &[], &none_text, 0)?;
    rec.check(
        "D8",
        is_none(&g(&out, "bench_mode"))
            && starts_with(&g(&out, "bench_mode_source"), "not_evaluated("),
        || shown(&out),
    );

    let five = vec![
        run_ok(1),
        run_ok(2),
        run_ok(3),
        run_ok(4),
        run_doc(5, false, 30.0, 1),
    ];
    let out = bench_mode(
        &index_doc(vec![level_doc(1, five)], 5, None, Some(1)),
        &[],
        &events_files,
        0,
    )?;
    rec.check(
        "D9",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && contains(&g(&out, "bench_mode_source"), "판정점 level 1 run 5"),
        || shown(&out),
    );

    let level2_broken = vec![run_doc(1, true, 30.0, 5), run_doc(2, false, 30.0, 5)];
    let index = plain_index(vec![level_doc(1, ok3()), level_doc(2, level2_broken)]);
    let out = bench_mode(&index, &[], &events_files, 0)?;
    rec.check(
        "D10",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && contains(&g(&out, "bench_mode_source"), "적응 상한 클램프")
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_MISS),
        || shown(&out),
    );
    let out = bench_mode(
        &index,
        &[armed("thermal_kill_ack", "2026-09-04T10:06:20Z")],
        &events_files,
        0,
    )?;
    rec.check(
        "D10b",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_LITE)
            && eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL),
        || shown(&out),
    );

    let out = bench_mode(
        &index_doc(
            vec![level_doc(1, vec![run_ok(1), run_ok(2)])],
            2,
            None,
            Some(1),
        ),
        &[],
        &none_text,
        0,
    )?;
    rec.check(
        "D11",
        is_none(&g(&out, "bench_mode"))
            && starts_with(&g(&out, "bench_mode_source"), "unclassifiable("),
        || shown(&out),
    );

    let declared_ok = obj(vec![
        ("bench_mode", s("lite")),
        ("downgrade_reason", PyValue::None),
        (
            "bench_mode_source",
            s("declared(lite_bench · lite-only 셀)"),
        ),
    ]);
    let undetermined_plain = obj(vec![
        ("bench_mode", s("lite")),
        ("downgrade_reason", PyValue::None),
        ("bench_mode_source", s("runs[]")),
    ]);
    let unknown_reason = obj(vec![
        ("bench_mode", s("lite")),
        ("downgrade_reason", s("variance")),
    ]);
    rec.check(
        "D12",
        bench_mode_kind(&declared_ok) == "declared-lite"
            && bench_mode_kind(&undetermined_plain) == "undetermined"
            && bench_mode_kind(&unknown_reason) == "undetermined"
            && bench_mode_kind(&PyValue::None) == "undetermined",
        String::new,
    );
    let declared = declared_lite_record(&s("lite_bench · lite-only 셀"))?;
    let shape = BenchModeFields::new(PyValue::None, s("x")).into_record();
    let keys = |value: &PyValue| -> Vec<String> {
        let mut found: Vec<String> = dict_of(value)
            .map(|map| {
                map.iter()
                    .map(|(key, _)| key.to_utf8().unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default();
        found.sort();
        found
    };
    rec.check(
        "D12b",
        bench_mode_kind(&declared) == "declared-lite"
            && is_none(&g(&declared, "downgrade_reason"))
            && eq_text(
                &g(&declared, "downgrade_correlation"),
                CORRELATION_NOT_APPLICABLE,
            )
            && keys(&declared) == keys(&shape),
        || shown(&declared),
    );
    let refused = declared_lite_record(&s("  "));
    rec.check(
        "D12c",
        matches!(&refused, Err(failure) if failure.is_value_error()),
        String::new,
    );

    let clamp = obj(vec![
        ("level", n(2)),
        ("run", n(1)),
        ("started_utc", s("2026-09-04T10:05:00Z")),
        ("ended_utc", s("2026-09-04T10:05:30Z")),
        ("run_bench_rc", n(3)),
        ("parse_rc", n(0)),
    ]);
    let clamped = || index_doc(vec![level_doc(1, ok3())], 3, Some(clamp.clone()), Some(1));
    let out = bench_mode(&clamped(), &[], &events_files, 0)?;
    rec.check(
        "D13",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_MISS)
            && eq_text(&at(&out, &["repetition", "stop", "kind"]), "clamp"),
        || shown(&out),
    );
    let out = bench_mode(
        &clamped(),
        &[armed("watchdog_kill_ack", "2026-09-04T10:05:10Z")],
        &events_files,
        0,
    )?;
    rec.check(
        "D14",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_LITE)
            && eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && bench_mode_kind(&out) == "downgraded-lite",
        || shown(&out),
    );
    let out = bench_mode(
        &clamped(),
        &[armed("thermal_trip", "2026-09-04T10:05:10Z")],
        &events_files,
        0,
    )?;
    rec.check(
        "D15",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && contains(&g(&out, "bench_mode_source"), "사인 불충분"),
        || shown(&out),
    );
    let out = bench_mode(&clamped(), &[], &none_text, 0)?;
    rec.check(
        "D16",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_NOT_SCANNED),
        || shown(&out),
    );
    let out = bench_mode(
        &plain_index(vec![level_doc(1, broken())]),
        &[],
        &arr(Vec::new()),
        0,
    )?;
    rec.check(
        "D16b",
        eq_text(&g(&out, "downgrade_reason"), DOWNGRADE_RUN_FAILED)
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_NOT_SCANNED),
        || shown(&out),
    );
    let unreadable_clamp = obj(vec![
        ("level", n(2)),
        ("run", n(1)),
        ("unreadable", s("부재")),
    ]);
    let out = bench_mode(
        &index_doc(
            vec![level_doc(1, ok3())],
            3,
            Some(unreadable_clamp),
            Some(1),
        ),
        &kill_events(),
        &events_files,
        0,
    )?;
    rec.check(
        "D17",
        eq_text(&g(&out, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_UNAVAILABLE),
        || shown(&out),
    );
    let short_level = vec![run_doc(1, true, 30.0, 5), run_doc(2, true, 30.0, 5)];
    let far_level = vec![
        run_doc(1, true, 30.0, 9),
        run_doc(2, true, 30.0, 9),
        run_doc(3, true, 30.0, 9),
    ];
    let out = bench_mode(
        &plain_index(vec![
            level_doc(1, ok3()),
            level_doc(2, short_level),
            level_doc(4, far_level),
        ]),
        &[],
        &events_files,
        0,
    )?;
    rec.check(
        "D18",
        is_none(&g(&out, "bench_mode"))
            && starts_with(&g(&out, "bench_mode_source"), "unclassifiable("),
        || shown(&out),
    );
    let out = bench_mode(
        &index_doc(vec![level_doc(1, ok3())], 3, None, None),
        &[],
        &events_files,
        0,
    )?;
    rec.check(
        "D19",
        is_none(&g(&out, "bench_mode"))
            && contains(&g(&out, "bench_mode_source"), "verdict_point_level"),
        || shown(&out),
    );
    Ok(())
}

/// The kill event the D2/D17 scenarios use (`kill_in`).
fn kill_events() -> Vec<String> {
    vec![armed("watchdog_kill_ack", "2026-09-04T10:02:10Z")]
}

// ── E: the durable record and the command line ───────────────────────────────────

fn record_checks(rec: &mut Recorder, owners: &Owners) -> Result<(), Failure> {
    let dir = TempDir::new("classify-cell.").map_err(|error| io_failure(&error))?;
    let broken = vec![run_ok(1), run_doc(2, false, 30.0, 1)];
    let index_path = dir.join("sweep_index.json");
    let doc = with_key(
        &plain_index(vec![level_doc(1, broken)]),
        "generated_utc",
        s("2026-09-04T11:00:00Z"),
    );
    dir.put("sweep_index.json", &json_compact(&doc))
        .map_err(|error| io_failure(&error))?;
    let path_value = s(&index_path);
    let read = |doc: &PyValue| read_bench_mode_record(&path_value, doc, &PyValue::None);

    let pair = read(&doc)?;
    rec.check(
        "E1",
        is_none(&record_of(&pair)) && starts_with(&status_of(&pair), "absent("),
        || shown(&status_of(&pair)),
    );
    let events_path = dir
        .put(
            "events.jsonl",
            &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:05:00Z")),
        )
        .map_err(|error| io_failure(&error))?;
    let (status, _) = cli(
        owners,
        &[
            "--sweep-index",
            &index_path,
            "--events",
            &events_path,
            "--write-bench-mode",
        ],
    );
    let pair = read(&doc)?;
    let record = record_of(&pair);
    let leftovers = std::fs::read_dir(&dir.path)
        .map_err(|error| io_failure(&error))?
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(&format!(".{BENCH_MODE_RECORD_NAME}"))
        })
        .count();
    rec.check(
        "E2",
        status == 0
            && eq_text(&status_of(&pair), "ok")
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_RUN_FAILED)
            && leftovers == 0,
        || {
            format!(
                "({status}, {}, {})",
                shown(&status_of(&pair)),
                shown(&record)
            )
        },
    );
    let (status, _) = cli(
        owners,
        &[
            "--sweep-index",
            &index_path,
            "--events",
            &events_path,
            "--tolerance-s",
            "200",
            "--write-bench-mode",
        ],
    );
    let pair = read(&doc)?;
    let record = record_of(&pair);
    rec.check(
        "E3",
        status == 0
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && crate::pydyn::equals(&g(&record, "tolerance_s"), &n(200)),
        || shown(&record),
    );
    let pair = read(&with_key(&doc, "generated_utc", s("2026-09-04T12:00:00Z")))?;
    rec.check(
        "E4",
        is_none(&record_of(&pair)) && starts_with(&status_of(&pair), "stale("),
        || shown(&status_of(&pair)),
    );
    let record_file = bench_mode_record_path(&path_value)?;
    std::fs::write(
        Path::new(&record_file.to_utf8().unwrap_or_default()),
        "{\"bench_mode\": \"fu",
    )
    .map_err(|error| io_failure(&error))?;
    let pair = read(&doc)?;
    rec.check(
        "E5",
        is_none(&record_of(&pair)) && starts_with(&status_of(&pair), "unreadable("),
        || shown(&status_of(&pair)),
    );
    let (mixed, _) = cli(
        owners,
        &[
            "--sweep-index",
            &index_path,
            "--serve-rc",
            "0",
            "--measure-rc",
            "0",
            "--started-utc",
            "2026-09-04T10:00:00Z",
            "--ended-utc",
            "2026-09-04T11:00:00Z",
        ],
    );
    let (no_write, _) = cli(
        owners,
        &[
            "--write-bench-mode",
            "--serve-rc",
            "0",
            "--measure-rc",
            "0",
            "--started-utc",
            "2026-09-04T10:00:00Z",
            "--ended-utc",
            "2026-09-04T11:00:00Z",
        ],
    );
    rec.check("E6", mixed == 2 && no_write == 2, || {
        format!("({mixed}, {no_write})")
    });
    Ok(())
}

// ── K-mirror, N, F, G: the node plan and the node scenarios ──────────────────────

const MULTI_MANIFEST: &str = "self_role: main\ntopology: multi\ngpus_per_node: 1\nnodes:\n  - role: main\n    host: 192.0.2.10\n  - role: sub\n    host: 192.0.2.11\n";

fn node_checks(rec: &mut Recorder, owners: &Owners) -> Result<(), Failure> {
    k_mirror(rec, owners)?;

    let dir = TempDir::new("classify-cell-nodes.").map_err(|error| io_failure(&error))?;
    let put = |relative: &str, text: &str| -> Result<String, Failure> {
        dir.put(relative, text).map_err(|error| io_failure(&error))
    };
    let multi_manifest = put("repo/output/multi/manifest.yaml", MULTI_MANIFEST)?;
    let single_manifest = put(
        "repo/output/single/manifest.yaml",
        "self_role: main\ntopology: single\ngpus_per_node: 1\n",
    )?;
    let duplicated = put(
        "dup/output/multi/manifest.yaml",
        &format!("{MULTI_MANIFEST}  - role: sub\n    host: 192.0.2.12\n"),
    )?;
    let repo = dir.join("repo");

    let plan = |topology: &str,
                measured: Option<&str>,
                source: Option<&str>,
                manifest: Option<&str>,
                repo: Option<&str>| {
        events_node_plan(
            &s(topology),
            &measured.map_or(PyValue::None, s),
            &source.map_or(PyValue::None, s),
            &manifest.map_or(PyValue::None, s),
            &repo.map_or(PyValue::None, s),
            owners,
        )
    };

    let pl = plan(
        "single",
        Some("main"),
        Some("derived(manifest.self_role=main)"),
        None,
        None,
    )?;
    rec.check(
        "N1",
        is_none(&g(&pl, "undeterminable"))
            && crate::pydyn::equals(&g(&pl, "nodes"), &texts(&["main"]))
            && eq_text(&g(&pl, "self_node"), "main")
            && g(&pl, "excluded_rule").is_truthy(),
        || shown(&pl),
    );
    let pl = plan("single", None, None, Some(&single_manifest), Some(&repo))?;
    rec.check(
        "N2",
        crate::pydyn::equals(&g(&pl, "nodes"), &texts(&["main"]))
            && contains(&g(&pl, "source"), "node_identity.sh --resolve"),
        || shown(&pl),
    );
    let cluster = plan("single", Some("cluster"), None, None, None)?;
    let escaping = plan("single", Some("../x"), None, None, None)?;
    rec.check(
        "N3",
        g(&cluster, "undeterminable").is_truthy() && g(&escaping, "undeterminable").is_truthy(),
        String::new,
    );
    let pl = plan(
        "multi",
        Some("cluster"),
        None,
        Some(&multi_manifest),
        Some(&repo),
    )?;
    let contract_ok = match owners.open_role_contract() {
        Ok(session) => session.is_some(),
        Err(failure) => {
            rec.note(&format!(
                "(node_role_contract unavailable: {})",
                failure_text(&failure)
            ));
            false
        }
    };
    let identity_ok = owners.node_identity_resolver()?.is_some();
    rec.check("F00", identity_ok, String::new);
    rec.check(
        "N4",
        !contract_ok
            || (is_none(&g(&pl, "undeterminable"))
                && crate::pydyn::equals(&g(&pl, "nodes"), &texts(&["main", "sub"]))
                && eq_text(&g(&pl, "self_node"), "main")),
        || shown(&pl),
    );
    let dup_repo = dir.join("dup");
    let pl = plan("multi", None, None, Some(&duplicated), Some(&dup_repo))?;
    rec.check(
        "N5",
        g(&pl, "undeterminable").is_truthy()
            && len_of(&g(&pl, "nodes")) == 0
            && eq_text(&g(&pl, "self_node"), "main"),
        || shown(&pl),
    );
    let wrong = plan(
        "multi",
        Some("main"),
        None,
        Some(&multi_manifest),
        Some(&repo),
    )?;
    rec.check("N6", g(&wrong, "undeterminable").is_truthy(), String::new);

    let clamp = obj(vec![
        ("level", n(2)),
        ("run", n(1)),
        ("started_utc", s("2026-09-04T10:05:00Z")),
        ("ended_utc", s("2026-09-04T10:05:30Z")),
        ("run_bench_rc", n(3)),
        ("parse_rc", n(0)),
    ]);
    let ok3 = || {
        vec![
            run_ok(1),
            run_doc(2, true, 31.0, 1),
            run_doc(3, true, 29.0, 1),
        ]
    };
    let sweep =
        |topology: &str, measured_node: &str, name: &str, root: &str| -> Result<String, Failure> {
            let doc = with_key(
                &with_key(
                    &index_doc(vec![level_doc(1, ok3())], 3, Some(clamp.clone()), Some(1)),
                    "generated_utc",
                    s("2026-09-04T11:00:00Z"),
                ),
                "meta",
                obj(vec![
                    ("topology", s(topology)),
                    ("measured_node", s(measured_node)),
                    ("measured_node_source", s("derived(fixture)")),
                ]),
            );
            put(
                &format!("{root}/output/{topology}/benchlog/sweep_{name}/sweep_index.json"),
                &json_compact(&doc),
            )
        };
    let main_events = "repo/docs/logs/main/events/2026-09.jsonl";
    let mirror_events = "repo/sync_staging/sub_docs/logs/sub/events/2026-09.jsonl";
    put(
        main_events,
        &format!("{}\n", armed("budget_renew", "2026-09-04T10:40:00Z")),
    )?;

    if contract_ok {
        multi_scenarios(rec, owners, &dir, &repo, &sweep, main_events, mirror_events)?;
    } else {
        rec.check("F0", false, || "node_role_contract 적재 실패".to_string());
    }
    single_scenarios(rec, owners, &dir, &repo, &sweep, main_events, contract_ok)?;
    Ok(())
}

type Sweep<'a> = dyn Fn(&str, &str, &str, &str) -> Result<String, Failure> + 'a;

fn put_in(dir: &TempDir, relative: &str, text: &str) -> Result<String, Failure> {
    dir.put(relative, text).map_err(|error| io_failure(&error))
}

fn multi_scenarios(
    rec: &mut Recorder,
    owners: &Owners,
    dir: &TempDir,
    repo: &str,
    sweep: &Sweep<'_>,
    main_events: &str,
    mirror_events: &str,
) -> Result<(), Failure> {
    let idx = sweep("multi", "cluster", "m", "repo")?;
    let sub_kill = format!(
        "{}\n{}\n",
        armed("watchdog_kill_ack", "2026-09-04T10:05:10Z"),
        armed("serve_stop", "2026-09-04T10:40:00Z")
    );
    put_in(dir, mirror_events, &sub_kill)?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "F1",
        status == 0
            && eq_text(&g(&record, "bench_mode"), BENCH_MODE_LITE)
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && eq_text(&g(&record, "downgrade_correlation"), CORRELATION_MATCHED)
            && eq_text(&node_status(&record, "sub"), NODE_SCAN_MATCHED)
            && contains(&g(&record, "downgrade_reason_source"), "node=sub"),
        || shown(&record),
    );
    dir.remove(mirror_events)
        .map_err(|error| io_failure(&error))?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "F2",
        status == 0
            && eq_text(&g(&record, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(
                &g(&record, "downgrade_correlation"),
                CORRELATION_NOT_SCANNED,
            )
            && eq_text(&node_status(&record, "main"), NODE_SCAN_MISS)
            && eq_text(&node_status(&record, "sub"), NODE_SCAN_NOT_SCANNED)
            && contains(&g(&record, "bench_mode_source"), "not-scanned(")
            && contains(&g(&record, "bench_mode_source"), "sub(원격)=not_scanned"),
        || shown(&record),
    );
    put_in(
        dir,
        mirror_events,
        &format!("{}\n", armed("serve_stop", "2026-09-04T09:00:00Z")),
    )?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "F3",
        status == 0
            && eq_text(
                &g(&record, "downgrade_correlation"),
                CORRELATION_NOT_SCANNED,
            )
            && eq_text(&node_status(&record, "sub"), NODE_SCAN_NOT_COVERED),
        || shown(&record),
    );
    put_in(
        dir,
        mirror_events,
        &format!("{}\n", armed("serve_stop", "2026-09-04T10:40:00Z")),
    )?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "F4",
        status == 0
            && eq_text(&g(&record, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(
                &g(&record, "downgrade_correlation"),
                CORRELATION_UNAVAILABLE,
            )
            && eq_text(&node_status(&record, "sub"), NODE_SCAN_SKEW_UNDECLARED),
        || shown(&record),
    );
    let (status, record) = cli(
        owners,
        &[
            "--sweep-index",
            &idx,
            "--events-from-repo",
            repo,
            "--cross-node-tolerance-s",
            "2",
            "--cross-node-tolerance-source",
            "fixture 선언",
        ],
    );
    let sub_entry = match at(&record, &["events_scan", "nodes"]).peel() {
        PyValue::List(entries) => entries
            .iter()
            .find(|entry| eq_text(&g(entry, "node"), "sub"))
            .cloned()
            .ok_or_else(|| Failure::new("StopIteration", ""))?,
        _ => return Err(Failure::new("StopIteration", "")),
    };
    rec.check(
        "F5",
        status == 0
            && eq_text(&g(&record, "downgrade_correlation"), CORRELATION_MISS)
            && crate::pydyn::equals(&g(&sub_entry, "tolerance_s"), &n(2))
            && eq_text(&g(&sub_entry, "tolerance_source"), "declared(fixture 선언)"),
        || shown(&record),
    );
    let (no_source, _) = cli(
        owners,
        &[
            "--sweep-index",
            &idx,
            "--events-from-repo",
            repo,
            "--cross-node-tolerance-s",
            "2",
        ],
    );
    rec.check("F6", no_source == 2, String::new);
    put_in(
        dir,
        mirror_events,
        &format!(
            "{}\n{}\n",
            armed("watchdog_kill_ack", "2026-09-04T10:05:40Z"),
            armed("serve_stop", "2026-09-04T10:40:00Z")
        ),
    )?;
    let (status, record) = cli(
        owners,
        &[
            "--sweep-index",
            &idx,
            "--events-from-repo",
            repo,
            "--cross-node-tolerance-s",
            "15",
            "--cross-node-tolerance-source",
            "fixture 선언",
        ],
    );
    rec.check(
        "F7",
        status == 0 && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL),
        || shown(&record),
    );
    dir.remove(mirror_events)
        .map_err(|error| io_failure(&error))?;
    put_in(
        dir,
        "repo/docs/logs/sub/events/2026-09.jsonl",
        &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:05:10Z")),
    )?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "F8",
        status == 0
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && eq_text(&node_status(&record, "sub"), NODE_SCAN_MATCHED),
        || shown(&record),
    );
    dir.remove("repo/docs/logs/sub/events/2026-09.jsonl")
        .map_err(|error| io_failure(&error))?;

    // F9/F10: the plan does not lean on manifest self_role; an undeterminable plan still reads this node's records.
    put_in(
        dir,
        "legacy/output/multi/manifest.yaml",
        &MULTI_MANIFEST.replace("self_role: main\n", ""),
    )?;
    put_in(
        dir,
        "legacy/output/single/manifest.yaml",
        "topology: single\ngpus_per_node: 1\n",
    )?;
    let legacy = dir.join("legacy");
    let idx_legacy = sweep("multi", "cluster", "legacy", "legacy")?;
    put_in(
        dir,
        "legacy/docs/logs/main/events/2026-09.jsonl",
        &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:05:10Z")),
    )?;
    let (status, record) = cli(
        owners,
        &["--sweep-index", &idx_legacy, "--events-from-repo", &legacy],
    );
    rec.check(
        "F9",
        status == 0
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && eq_text(&g(&record, "downgrade_correlation"), CORRELATION_MATCHED)
            && is_none(&at(&record, &["events_scan", "plan", "undeterminable"]))
            && eq_text(&node_status(&record, "main"), NODE_SCAN_MATCHED)
            && eq_text(&node_status(&record, "sub"), NODE_SCAN_NOT_SCANNED),
        || shown(&record),
    );
    let idx_dup = sweep("multi", "cluster", "dup", "dup")?;
    let dup_repo = dir.join("dup");
    let (status, record) = cli(
        owners,
        &["--sweep-index", &idx_dup, "--events-from-repo", &dup_repo],
    );
    rec.check(
        "F10a",
        status == 0
            && eq_text(&g(&record, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(
                &g(&record, "downgrade_correlation"),
                CORRELATION_NOT_SCANNED,
            )
            && at(&record, &["events_scan", "plan", "undeterminable"]).is_truthy()
            && eq_text(
                &node_status(&record, UNIDENTIFIED_NODE),
                NODE_SCAN_NOT_SCANNED,
            )
            && contains(&g(&record, "bench_mode_source"), "대조 노드 미정"),
        || shown(&record),
    );
    put_in(
        dir,
        "dup/docs/logs/main/events/2026-09.jsonl",
        &format!("{}\n", armed("serve_stop", "2026-09-04T10:40:00Z")),
    )?;
    let (status, record) = cli(
        owners,
        &["--sweep-index", &idx_dup, "--events-from-repo", &dup_repo],
    );
    rec.check(
        "F10b",
        status == 0
            && eq_text(
                &g(&record, "downgrade_correlation"),
                CORRELATION_NOT_SCANNED,
            )
            && eq_text(&node_status(&record, "main"), NODE_SCAN_MISS)
            && eq_text(
                &node_status(&record, UNIDENTIFIED_NODE),
                NODE_SCAN_NOT_SCANNED,
            ),
        || shown(&record),
    );
    put_in(
        dir,
        "dup/docs/logs/main/events/2026-09.jsonl",
        &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:05:10Z")),
    )?;
    let (status, record) = cli(
        owners,
        &["--sweep-index", &idx_dup, "--events-from-repo", &dup_repo],
    );
    rec.check(
        "F10c",
        status == 0
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && eq_text(&g(&record, "downgrade_correlation"), CORRELATION_MATCHED),
        || shown(&record),
    );

    // F11: aggregation order - a matched node wins over another node that is not_scanned.
    dir.remove(main_events)
        .map_err(|error| io_failure(&error))?;
    put_in(dir, mirror_events, &sub_kill)?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "F11",
        status == 0
            && eq_text(&g(&record, "downgrade_correlation"), CORRELATION_MATCHED)
            && eq_text(&node_status(&record, "main"), NODE_SCAN_NOT_SCANNED)
            && eq_text(&node_status(&record, "sub"), NODE_SCAN_MATCHED)
            && contains(&g(&record, "downgrade_reason_source"), "허용오차 미선언"),
        || shown(&record),
    );
    dir.remove(mirror_events)
        .map_err(|error| io_failure(&error))?;
    put_in(
        dir,
        main_events,
        &format!("{}\n", armed("budget_renew", "2026-09-04T10:40:00Z")),
    )?;
    Ok(())
}

fn single_scenarios(
    rec: &mut Recorder,
    owners: &Owners,
    dir: &TempDir,
    repo: &str,
    sweep: &Sweep<'_>,
    main_events: &str,
    contract_ok: bool,
) -> Result<(), Failure> {
    let idx = sweep("single", "main", "s", "repo")?;
    put_in(
        dir,
        "repo/docs/logs/sub/events/2026-09.jsonl",
        &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:05:10Z")),
    )?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "G1",
        status == 0
            && eq_text(&g(&record, "bench_mode"), BENCH_MODE_FULL)
            && eq_text(&g(&record, "downgrade_correlation"), CORRELATION_MISS)
            && node_names(&record) == ["main"]
            && g(&at(&record, &["events_scan", "plan"]), "excluded_rule").is_truthy(),
        || shown(&record),
    );
    put_in(
        dir,
        main_events,
        &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:05:10Z")),
    )?;
    let (status, record) = cli(owners, &["--sweep-index", &idx, "--events-from-repo", repo]);
    rec.check(
        "G2",
        status == 0 && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL),
        || shown(&record),
    );
    put_in(
        dir,
        main_events,
        &format!("{}\n", armed("budget_renew", "2026-09-04T10:40:00Z")),
    )?;
    let idx_sub = sweep("single", "sub", "on-sub", "repo")?;
    let (status, record) = cli(
        owners,
        &["--sweep-index", &idx_sub, "--events-from-repo", repo],
    );
    let first_remote = match at(&record, &["events_scan", "nodes"]).peel() {
        PyValue::List(entries) => entries
            .first()
            .map_or(PyValue::None, |entry| g(entry, "remote")),
        _ => PyValue::None,
    };
    rec.check(
        "G3",
        status == 0
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && node_names(&record) == ["sub"]
            && matches!(first_remote.peel(), PyValue::Bool(false)),
        || shown(&record),
    );

    let cell = [
        "--serve-rc",
        "3",
        "--measure-rc",
        "absent",
        "--started-utc",
        "2026-09-04T10:00:00Z",
        "--ended-utc",
        "2026-09-04T10:30:00Z",
        "--events-from-repo",
        repo,
    ];
    let with_topology = |topology: &'static str| -> Vec<&str> {
        let mut argv: Vec<&str> = cell.to_vec();
        argv.extend(["--topology", topology]);
        argv
    };
    let (status, out) = cli(owners, &with_topology("single"));
    rec.check(
        "G4",
        status == 0
            && is_none(&g(&out, "void_reason"))
            && starts_with(&g(&out, "void_reason_source"), "correlation-miss(")
            && contains(&g(&out, "void_reason_source"), "대조 노드[main=miss]"),
        || shown(&out),
    );
    let (no_topology, _) = cli(owners, &cell);
    rec.check("G5", no_topology == 2, String::new);
    if contract_ok {
        let (status, out) = cli(owners, &with_topology("multi"));
        rec.check(
            "G6",
            status == 0
                && eq_text(&g(&out, "void_reason"), "watchdog_kill_ack")
                && starts_with(&g(&out, "void_reason_source"), "events("),
            || shown(&out),
        );
        dir.remove("repo/docs/logs/sub/events/2026-09.jsonl")
            .map_err(|error| io_failure(&error))?;
        let (status, out) = cli(owners, &with_topology("multi"));
        rec.check(
            "G7",
            status == 0
                && is_none(&g(&out, "void_reason"))
                && starts_with(&g(&out, "void_reason_source"), "not-scanned(")
                && eq_text(&g(&out, "downgrade_correlation"), CORRELATION_NOT_SCANNED),
            || shown(&out),
        );
    }
    let (mixed, _) = cli(
        owners,
        &[
            "--sweep-index",
            &idx,
            "--events-from-repo",
            repo,
            "--topology",
            "single",
        ],
    );
    rec.check("G8", mixed == 2, String::new);
    let old_doc = with_key(
        &index_doc(
            vec![level_doc(
                1,
                vec![
                    run_ok(1),
                    run_doc(2, true, 31.0, 1),
                    run_doc(3, true, 29.0, 1),
                ],
            )],
            3,
            Some(obj(vec![
                ("level", n(2)),
                ("run", n(1)),
                ("started_utc", s("2026-09-04T10:05:00Z")),
                ("ended_utc", s("2026-09-04T10:05:30Z")),
                ("run_bench_rc", n(3)),
                ("parse_rc", n(0)),
            ])),
            Some(1),
        ),
        "generated_utc",
        s("2026-09-04T11:00:00Z"),
    );
    let idx_old = put_in(
        dir,
        "repo/output/single/benchlog/sweep_old/sweep_index.json",
        &json_compact(&old_doc),
    )?;
    let no_manifest = dir.join("nomanifest");
    let (status, record) = cli(
        owners,
        &[
            "--sweep-index",
            &idx_old,
            "--events-from-repo",
            &no_manifest,
        ],
    );
    rec.check(
        "G9",
        status == 0
            && eq_text(
                &g(&record, "downgrade_correlation"),
                CORRELATION_NOT_SCANNED,
            )
            && at(&record, &["events_scan", "plan", "undeterminable"]).is_truthy()
            && contains(&g(&record, "bench_mode_source"), "대조 노드 미정"),
        || shown(&record),
    );
    put_in(
        dir,
        main_events,
        &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:05:10Z")),
    )?;
    let (status, record) = cli(
        owners,
        &["--sweep-index", &idx_old, "--events-from-repo", repo],
    );
    rec.check(
        "G9b",
        status == 0
            && eq_text(&g(&record, "downgrade_reason"), DOWNGRADE_BLACKBOX_KILL)
            && node_names(&record) == ["main", UNIDENTIFIED_NODE],
        || shown(&record),
    );
    put_in(
        dir,
        main_events,
        &format!("{}\n", armed("budget_renew", "2026-09-04T10:40:00Z")),
    )?;

    // G10: cell-closing mode on a manifest without self_role: this node stands by the owner's rule.
    put_in(
        dir,
        "legacy/docs/logs/main/events/2026-09.jsonl",
        &format!("{}\n", armed("watchdog_kill_ack", "2026-09-04T10:10:00Z")),
    )?;
    let legacy = dir.join("legacy");
    let (status, out) = cli(
        owners,
        &[
            "--serve-rc",
            "3",
            "--measure-rc",
            "absent",
            "--started-utc",
            "2026-09-04T10:00:00Z",
            "--ended-utc",
            "2026-09-04T10:30:00Z",
            "--events-from-repo",
            &legacy,
            "--topology",
            "single",
        ],
    );
    rec.check(
        "G10",
        status == 0
            && eq_text(&g(&out, "void_reason"), "watchdog_kill_ack")
            && starts_with(&g(&out, "void_reason_source"), "events(")
            && is_none(&at(&out, &["events_scan", "plan", "undeterminable"])),
        || shown(&out),
    );
    Ok(())
}

fn k_mirror(rec: &mut Recorder, owners: &Owners) -> Result<(), Failure> {
    let scripts = owners.scripts_dir()?;
    let fetch = ospath::normpath(&ospath::join_all(
        &scripts,
        &[
            "..",
            "..",
            "upstream-version-watch",
            "scripts",
            "fetch_sub_docs.sh",
        ],
    ));
    if !ospath::isfile(&fetch) {
        rec.note("[SKIP] K-mirror fetch_sub_docs.sh 부재(서브 배달 트리 — 메인 전용 스크립트다)");
        return Ok(());
    }
    let bytes = crate::pyfs::read_bytes(&fetch)?;
    let source = decode_file_text(&bytes)
        .map_err(|error| Failure::new("UnicodeDecodeError", &error.to_string()))?
        .to_utf8()
        .unwrap_or_default();
    let chars: Vec<char> = source.chars().collect();
    let dest = dest_defaults(&chars);
    let copies = copies_docs(&source);
    let expected_dest = EVENTS_RECOVERED_PARTS[..2].join("/");
    rec.check(
        "K-mirror",
        dest == [expected_dest.clone()]
            && copies
            && EVENTS_RECOVERED_PARTS[2..] == EVENTS_LOCAL_PARTS[1..],
        || format!("({dest:?}, {copies})"),
    );
    Ok(())
}
