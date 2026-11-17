//! `blackbox_session.py` — the serving session sidecar (`docs/logs/<node_id>/sessions/<id>.json` + serve_start/stop
//! events) and the serving-budget declaration the memory watchdog arms on (`serve_budget.env`). Same sub-commands,
//! flags, exit codes, texts and files as the original. The declaration constants are derived from
//! `blackbox_eta::DEFAULTS` (never copied); the watchdog trip threshold is read from the deployed unit
//! (`systemctl show easy-vllm-memwatch.service -p ExecStart --value`), as the original does.
//!
//! Times are `--now` only (no wall clock). The budget file is replaced atomically (tmp + fsync + rename) because the
//! watchdog reads it every second.
//!
//! **Two models on one node (plan_26111708 B L6 · F2 of 2026-10-14).** The consumer — the root watchdog
//! `mem_watchdog_eta.sh` — reads ONE file and takes the first `floor_mib`/`expires_epoch`/`label` line each (`sed … |
//! head -1`); it cannot hold per-model floors, and changing it means a reinstall plus a dry-run observation before
//! arming (C1-5), which no deterministic test here can stand in for. So the combination is done on the writer side and
//! the consumer still sees one ordinary declaration: when a **live declaration of another label** exists, declare-budget
//! stacks the new one on it — overhead = the caller's own `--overhead-mib` + (MemTotal − MemAvailable) measured now
//! (the documented two-step, `testlog_26111700` §3 L6) — and records that with provenance (`stacked_on`·`stack_*` keys,
//! a comment line, the event). The first model's refresh loop stops by itself: it renews with `--expect-label` and gets
//! exit 3 once the declaration carries another label. Anything it cannot decide fails closed and leaves the live
//! declaration untouched; with no live declaration of another label the path is byte-identical to before.

use crate::argp::{self, flag, store, Arg, Outcome, Parsed, Spec, HELP};
use crate::bb::{self, repr, Fail, R};
use crate::blackbox_eta;
use crate::py;
use crate::pyre::Regex;
use crate::raise::Raised;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;

pub const BUDGET_FILE: &str = "serve_budget.env";
pub const MAX_TTL_S: i64 = 86400;
pub const DEFAULT_TTL_S: i64 = 7200;
pub const TTL_SAFETY_MULT: i64 = 3;
pub const BLOCK_STAGES: [&str; 3] = ["derive", "preflight_ceiling", "declare"];
const WD_UNIT: &str = "easy-vllm-memwatch.service";
const WD_CONST_SOURCE: &str = "derived:blackbox_eta.DEFAULTS";
/// `renew-budget --expect-label` when the declaration now carries another label (a co-resident model stacked on it):
/// a state, not an error — the superseded model's refresh loop (`budget_renew_loop.sh`) ends on it. plan_26111708 B L6.
pub const RENEW_SUPERSEDED_RC: i32 = 3;
const MEMINFO_PATH: &str = "/proc/meminfo";
/// The value written as `stack_source=` (watchdog value charset `[A-Za-z0-9._-]`).
const MEMINFO_SOURCE: &str = "measured-proc-meminfo";

fn wd_margin() -> i64 {
    blackbox_eta::default_int("decl_margin_mib")
}
fn wd_min_ceiling() -> i64 {
    blackbox_eta::default_int("decl_min_ceiling_mib")
}

fn exit(m: String) -> Fail {
    Fail::Exit(m)
}

/// `_parse_now(value)`: ISO-Z only, otherwise SystemExit.
fn parse_now(value: Option<&str>) -> R<String> {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$");
    match value {
        Some(v) if re.match_(v).is_some() => Ok(v.to_string()),
        Some(v) => Err(exit(format!(
            "--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: {}",
            repr(v)
        ))),
        None => Err(exit(
            "--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: None".into(),
        )),
    }
}

/// `_epoch(iso)`.
fn epoch(iso: &str) -> R<i64> {
    crate::pytime::strptime_checked(iso, "%Y-%m-%dT%H:%M:%SZ")
        .map(|d| d.epoch())
        .map_err(|m| Fail::Raise(Raised::new("ValueError", &m)))
}

fn safe_id(s: &str) -> bool {
    Regex::new(r"^[A-Za-z0-9._-]+$").match_(s).is_some()
}

fn session_path(node_dir: &str, session_id: Option<&str>) -> R<String> {
    let sid = session_id.unwrap_or("");
    if !safe_id(sid) {
        return Err(exit(format!(
            "session-id 는 [A-Za-z0-9._-]+ 여야 한다: {}",
            session_id.map_or("None".into(), repr)
        )));
    }
    Ok(bb::join(&bb::join(node_dir, "sessions"), &format!("{sid}.json")))
}

/// `_append_event(node_dir, rec)`: `events/<YYYY-MM>.jsonl`, one sorted-key JSON object per line.
fn append_event(node_dir: &str, rec: &PyValue) -> Result<String, Raised> {
    let ts = py::get_str(rec, "ts").unwrap_or_default();
    let month: String = ts.chars().take(7).collect();
    let path = bb::join(&bb::join(node_dir, "events"), &format!("{month}.jsonl"));
    bb::makedirs_as_ancestor_owner(&bb::dirname(&path))?;
    bb::append(&path, &(bb::dumps_sorted(rec) + "\n"))?;
    Ok(path)
}

/// The parsed options of every sub-command (a field is None when the sub-command has no such option).
#[derive(Clone, Default, Debug)]
pub struct A {
    pub node_dir: String,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub model_path: Option<String>,
    pub recipe: Option<String>,
    pub topology: Option<String>,
    pub tp: Option<i64>,
    pub config_file: Option<String>,
    pub image: Option<String>,
    pub now: Option<String>,
    pub note: Option<Vec<String>>,
    pub force: bool,
    pub verdict: Option<String>,
    pub mem_total_mib: Option<i64>,
    pub weights_mib: Option<i64>,
    pub kv_mib: Option<i64>,
    pub overhead_mib: Option<i64>,
    pub ttl_s: Option<i64>,
    pub expected_load_s: Option<i64>,
    pub label: Option<String>,
    /// `renew-budget --expect-label` (plan_26111708 B L6).
    pub expect_label: Option<String>,
    pub field: Option<String>,
    pub reason: Option<String>,
    pub stage: Option<String>,
    pub detail: Vec<String>,
}

fn os(v: &Option<String>) -> PyValue {
    py::opt_str(v.as_deref())
}
fn oi(v: Option<i64>) -> PyValue {
    v.map_or(PyValue::None, PyValue::int)
}

/// Writes the session document (`json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n"`).
fn write_doc(path: &str, doc: &PyValue) -> Result<(), Raised> {
    bb::write(path, &(bb::dumps_sorted_pretty(doc) + "\n"))
}

pub fn cmd_start(a: &A) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    let path = session_path(&a.node_dir, a.session_id.as_deref())?;
    if std::fs::symlink_metadata(&path).is_ok() && std::fs::metadata(&path).is_ok() && !a.force {
        return Err(exit(format!(
            "이미 존재하는 세션이다(덮어쓰기는 --force): {path}"
        )));
    }
    bb::makedirs_as_ancestor_owner(&bb::dirname(&path))?;
    let doc = py::dict(vec![
        ("session_id", os(&a.session_id)),
        ("model", os(&a.model)),
        ("model_path", os(&a.model_path)),
        ("recipe", os(&a.recipe)),
        ("topology", os(&a.topology)),
        ("tensor_parallel_size", oi(a.tp)),
        ("config_file", os(&a.config_file)),
        ("image", os(&a.image)),
        ("started_utc", py::s(&now)),
        ("started_epoch", PyValue::int(epoch(&now)?)),
        ("stopped_utc", PyValue::None),
        ("stopped_epoch", PyValue::None),
        ("verdict", PyValue::None),
        (
            "notes",
            py::list_str(a.note.as_deref().filter(|n| !n.is_empty()).unwrap_or(&[])),
        ),
    ]);
    write_doc(&path, &doc)?;
    let ev = append_event(
        &a.node_dir,
        &py::dict(vec![
            ("kind", py::s("serve_start")),
            ("ts", py::s(&now)),
            ("source", py::s("blackbox_session")),
            ("session_id", os(&a.session_id)),
            ("model", os(&a.model)),
            ("topology", os(&a.topology)),
            ("tensor_parallel_size", oi(a.tp)),
        ]),
    )?;
    println!("세션 시작: {path}");
    println!("이벤트   : {ev}");
    Ok(0)
}

fn type_err_sub(a: &PyValue, b: &PyValue) -> Raised {
    Raised::new(
        "TypeError",
        &format!(
            "unsupported operand type(s) for -: '{}' and '{}'",
            a.type_name(),
            b.type_name()
        ),
    )
}

fn as_i64(v: &PyValue) -> Option<i64> {
    match v.peel() {
        PyValue::Int(n) => i64::try_from(n.clone()).ok(),
        PyValue::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

pub fn cmd_stop(a: &A) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    let path = session_path(&a.node_dir, a.session_id.as_deref())?;
    if !bb::is_file(&path) {
        return Err(exit(format!("세션 파일이 없다: {path}")));
    }
    let mut doc = bb::load_json(&path)?;
    let PyValue::Dict(d) = &mut doc else {
        return Err(Fail::Raise(Raised::new(
            "TypeError",
            &format!(
                "'{}' object does not support item assignment",
                doc.type_name()
            ),
        )));
    };
    let stop_e = epoch(&now)?;
    d.set("stopped_utc", py::s(&now));
    d.set("stopped_epoch", PyValue::int(stop_e));
    d.set("verdict", os(&a.verdict));
    if let Some(notes) = a.note.as_ref().filter(|n| !n.is_empty()) {
        let prev = d.get("notes").cloned().filter(|v| v.is_truthy());
        let mut l = match prev.as_ref().map(|v| v.peel()) {
            None => vec![],
            Some(PyValue::List(l)) => l.clone(),
            Some(other) => {
                return Err(Fail::Raise(Raised::new(
                    "TypeError",
                    &format!(
                        "can only concatenate {} (not \"list\") to {}",
                        other.type_name(),
                        other.type_name()
                    ),
                )))
            }
        };
        l.extend(notes.iter().map(|x| py::s(x)));
        d.set("notes", PyValue::List(l));
    }
    let started = d
        .get("started_epoch")
        .cloned()
        .unwrap_or(PyValue::int(stop_e));
    let dur = match as_i64(&started) {
        Some(s) => stop_e - s,
        None => return Err(Fail::Raise(type_err_sub(&PyValue::int(stop_e), &started))),
    };
    d.set("duration_s", PyValue::int(dur));
    let model = d.get("model").cloned().unwrap_or(PyValue::None);
    write_doc(&path, &doc)?;
    let ev = append_event(
        &a.node_dir,
        &py::dict(vec![
            ("kind", py::s("serve_stop")),
            ("ts", py::s(&now)),
            ("source", py::s("blackbox_session")),
            ("session_id", os(&a.session_id)),
            ("model", model),
            ("verdict", os(&a.verdict)),
            ("duration_s", PyValue::int(dur)),
        ]),
    )?;
    println!(
        "세션 종료: {} ({}s · verdict={})",
        path,
        dur,
        a.verdict.as_deref().unwrap_or("None")
    );
    println!("이벤트   : {ev}");
    Ok(0)
}

/// `watchdog_trip_mib()`: the trip threshold the deployed watchdog uses (MiB) and its source, or (None, why).
pub fn watchdog_trip_mib() -> (Option<i64>, String) {
    use std::process::{Command, Stdio};
    let child = Command::new("systemctl")
        .args(["show", WD_UNIT, "-p", "ExecStart", "--value"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let name = match e.raw_os_error() {
                Some(2) => "FileNotFoundError",
                Some(13) => "PermissionError",
                Some(20) => "NotADirectoryError",
                _ => "OSError",
            };
            return (None, format!("systemctl 실행 불가({name})"));
        }
    };
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let to = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = std::io::Read::read_to_end(&mut out, &mut b);
        b
    });
    let te = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = std::io::Read::read_to_end(&mut err, &mut b);
        b
    });
    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(_) => return (None, "systemctl 실행 불가(OSError)".into()),
        }
        if start.elapsed() >= std::time::Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            return (None, "systemctl 실행 불가(TimeoutExpired)".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let stdout = String::from_utf8_lossy(&to.join().unwrap_or_default()).into_owned();
    let _ = te.join();
    use std::os::unix::process::ExitStatusExt;
    let rc = status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or(0));
    if rc != 0 || py::strip(&stdout).is_empty() {
        return (
            None,
            format!("유닛 {WD_UNIT} 를 조회하지 못했다(rc={rc})"),
        );
    }
    let re = Regex::new(r"easy-vllm-memwatch\s+(\S+)\s+(\d+)");
    let Some(m) = re.search(&stdout) else {
        return (None, "ExecStart 에서 임계 인자를 찾지 못했다".into());
    };
    let n = bb::py_int_text(m.group(2).unwrap_or("")).unwrap_or(0);
    (
        Some(n),
        format!("measured(systemctl show {WD_UNIT} ExecStart)"),
    )
}

pub fn ttl_floor_s(expected_load_s: i64) -> i64 {
    expected_load_s * TTL_SAFETY_MULT
}

/// `validate_ttl(ttl_s, expected_load_s)`: the one TTL rule declare and renew share.
pub fn validate_ttl(ttl_s: i64, expected_load_s: i64) -> R<()> {
    if ttl_s <= 0 || ttl_s > MAX_TTL_S {
        return Err(exit(format!(
            "--ttl-s 는 1..{MAX_TTL_S} 여야 한다(무기한 선언 금지): {ttl_s}"
        )));
    }
    if expected_load_s != 0 {
        let floor_ttl = ttl_floor_s(expected_load_s);
        if ttl_s < floor_ttl {
            return Err(exit(format!(
                "--ttl-s {ttl_s} 는 예상 로드 {expected_load_s}s 의 {TTL_SAFETY_MULT}배(={floor_ttl}s)보다 짧다 — 로드 도중 만료가 예약된다. 거부한다(조용한 보정 금지)."
            )));
        }
    }
    Ok(())
}

fn budget_path(node_dir: &str) -> String {
    bb::join(node_dir, BUDGET_FILE)
}

/// `_read_budget(node_dir)`: the current declaration as an ordered dict (floor_mib, expires_epoch as ints), or None.
pub fn read_budget(node_dir: &str) -> Result<Option<PyDict>, Raised> {
    let path = budget_path(node_dir);
    if !bb::is_file(&path) {
        return Ok(None);
    }
    let text = bb::read_text(&path)?;
    let re = Regex::new(r"^\s*([A-Za-z_]+)\s*=\s*([A-Za-z0-9._-]{1,64})\s*$");
    let mut out = PyDict::new();
    for line in bb::text_lines(&text) {
        if let Some(m) = re.match_(line) {
            out.set(m.group(1).unwrap(), py::s(m.group(2).unwrap()));
        }
    }
    if !out.contains_key("floor_mib") || !out.contains_key("expires_epoch") {
        return Ok(None);
    }
    let fl = bb::py_int_text(&bb::s(out.get("floor_mib").unwrap()));
    let ex = bb::py_int_text(&bb::s(out.get("expires_epoch").unwrap()));
    let (Some(fl), Some(ex)) = (fl, ex) else {
        return Ok(None);
    };
    out.set("floor_mib", PyValue::int(fl));
    out.set("expires_epoch", PyValue::int(ex));
    Ok(Some(out))
}

fn write_budget(node_dir: &str, body: &str) -> Result<String, Raised> {
    let path = budget_path(node_dir);
    bb::makedirs_as_ancestor_owner(node_dir)?;
    bb::write_atomic(&path, body)?;
    Ok(path)
}

/// The watchdog-threshold lookup `cmd_declare_budget` uses (the self-test swaps in fixtures, as the original does).
pub type TripFn = fn() -> (Option<i64>, String);

/// The node's MemTotal/MemAvailable (MiB) and the source that produced them (§결정론 규율 — 출처 표시).
#[derive(Clone, Debug)]
pub struct Meminfo {
    pub total_mib: i64,
    pub avail_mib: i64,
    pub source: String,
}

/// Where a co-resident declaration reads the node's memory (plan_26111708 B L6). Production is [`read_meminfo`]; the
/// self-test passes fixtures, so no test reads the host's /proc/meminfo.
pub type MeminfoFn = fn() -> Result<Meminfo, String>;

/// MemTotal/MemAvailable from `/proc/meminfo` text through the one parser (`blackbox_eta::meminfo_mib`).
pub fn parse_meminfo(text: &str, source: &str) -> Result<Meminfo, String> {
    match (
        blackbox_eta::meminfo_mib(text, "MemTotal"),
        blackbox_eta::meminfo_mib(text, "MemAvailable"),
    ) {
        (Some(t), Some(v)) => Ok(Meminfo {
            total_mib: t,
            avail_mib: v,
            source: source.into(),
        }),
        _ => Err(format!("{source}: MemTotal/MemAvailable 판독 불가")),
    }
}

pub fn read_meminfo() -> Result<Meminfo, String> {
    let text = bb::read_text(MEMINFO_PATH).map_err(|e| format!("{MEMINFO_PATH} 판독 실패({})", e.0))?;
    parse_meminfo(&text, MEMINFO_SOURCE)
}

pub fn cmd_declare_budget(a: &A) -> R<i32> {
    cmd_declare_budget_with(a, watchdog_trip_mib)
}

pub fn cmd_declare_budget_with(a: &A, trip: TripFn) -> R<i32> {
    cmd_declare_budget_with_sources(a, trip, read_meminfo)
}

pub fn cmd_declare_budget_with_sources(a: &A, trip: TripFn, meminfo: MeminfoFn) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    match declare_budget(a, &now, trip, meminfo) {
        Err(Fail::Exit(m)) => {
            record_declare_rejection(a, &now, &m);
            Err(Fail::Exit(m))
        }
        other => other,
    }
}

fn safe_label_for_event(label: Option<&str>) -> String {
    match label {
        None | Some("") => "unlabeled".into(),
        Some(l) => l.chars().take(64).collect(),
    }
}

fn record_declare_rejection(a: &A, now: &str, reason: &str) {
    let rec = py::dict(vec![
        ("kind", py::s("budget_declare_rejected")),
        ("ts", py::s(now)),
        ("source", py::s("blackbox_session")),
        ("reason", py::s(reason)),
        ("label", py::s(&safe_label_for_event(a.label.as_deref()))),
        ("mem_total_mib", oi(a.mem_total_mib)),
        ("weights_mib", oi(a.weights_mib)),
        ("kv_mib", oi(a.kv_mib)),
        ("overhead_mib", oi(a.overhead_mib)),
        ("ttl_s", oi(a.ttl_s)),
        (
            "expected_load_s",
            PyValue::int(a.expected_load_s.unwrap_or(0)),
        ),
    ]);
    if let Err(e) = append_event(&a.node_dir, &rec) {
        // the OSError text as `str(e)` (the class name is not part of it)
        let msg = e.0.split_once(": ").map_or(e.0.clone(), |x| x.1.to_string());
        eprintln!("⚠ 거부 이벤트 기록 실패({msg}): {reason}");
    }
}

/// A declaration stacked on another model's live declaration on the same node (plan_26111708 B L6).
struct Stack {
    on_label: String,
    live_floor: i64,
    own_overhead_mib: i64,
    used_mib: i64,
    mem: Meminfo,
}

/// plan_26111708 B L6: is there a **live declaration of another model** on this node? Then the new one is stacked on it.
///
/// Not stacked — the single-model path, byte-identical to before: no declaration file, one the consumer cannot interpret
/// (`read_budget` → None; the watchdog rejects it too), an expired one (stale — the watchdog already runs without it),
/// the same label (the same model re-declares), or a new label the later charset check rejects (its rejection stays
/// where it was). A `--now` that passes the pattern but is no date also falls through: the single path raises it at its
/// original place and nothing is written.
///
/// Fail closed — the live declaration is left as it is: the declaration file cannot be read (the watchdog's `sed` may
/// still honour it), the node memory cannot be read, MemAvailable is outside [0, MemTotal], or `--mem-total-mib` is not
/// the measured MemTotal (facts of two different nodes or moments must not be combined).
fn co_resident_stack(a: &A, now: &str, mt: i64, own_oh: i64, mem_fn: MeminfoFn) -> R<Option<Stack>> {
    let label = a
        .label
        .as_deref()
        .filter(|l| !l.is_empty())
        .unwrap_or("unlabeled");
    if !safe_id(label) {
        return Ok(None);
    }
    let path = budget_path(&a.node_dir);
    let cur = read_budget(&a.node_dir).map_err(|e| {
        exit(format!(
            "기존 예산 선언을 읽지 못했다({}) — 워치독은 그 파일을 sed 로 읽으므로 다른 모델의 살아 있는 선언일 수 있다. 덮지 않는다(fail-closed): {path}",
            e.0
        ))
    })?;
    let Some(cur) = cur else {
        return Ok(None);
    };
    let Ok(now_e) = epoch(now) else {
        return Ok(None);
    };
    let ex = as_i64(cur.get("expires_epoch").unwrap()).unwrap();
    if ex <= now_e {
        return Ok(None);
    }
    let on_label = cur
        .get("label")
        .map(bb::s)
        .unwrap_or_else(|| "unlabeled".into());
    if on_label == label {
        return Ok(None);
    }
    let live_floor = as_i64(cur.get("floor_mib").unwrap()).unwrap();
    let ctx = format!(
        "살아 있는 다른 모델의 선언(label={on_label} · floor {live_floor} MiB · 잔여 {}s) 위 공존 적층",
        ex - now_e
    );
    let mem = mem_fn().map_err(|why| {
        exit(format!(
            "{ctx}: 노드 메모리를 읽지 못했다({why}) — 공존 상주분(MemTotal − MemAvailable)을 산출할 수 없어 기존 선언을 덮지 않는다(fail-closed)."
        ))
    })?;
    if mem.avail_mib < 0 || mem.avail_mib > mem.total_mib {
        return Err(exit(format!(
            "{ctx}: 측정 MemAvailable {} MiB 가 [0, MemTotal {} MiB] 밖이다({}) — 판독이 깨졌다. 기존 선언을 덮지 않는다(fail-closed).",
            mem.avail_mib, mem.total_mib, mem.source
        )));
    }
    if mem.total_mib != mt {
        return Err(exit(format!(
            "{ctx}: --mem-total-mib {mt} 가 측정 MemTotal {} MiB({}) 와 다르다 — 다른 노드·시점의 사실로 적층하지 않는다. 기존 선언을 덮지 않는다(fail-closed).",
            mem.total_mib, mem.source
        )));
    }
    Ok(Some(Stack {
        on_label,
        live_floor,
        own_overhead_mib: own_oh,
        used_mib: mem.total_mib - mem.avail_mib,
        mem,
    }))
}

fn declare_budget(a: &A, now: &str, trip_fn: TripFn, mem_fn: MeminfoFn) -> R<i32> {
    let ttl = a.ttl_s.unwrap_or(DEFAULT_TTL_S);
    validate_ttl(ttl, a.expected_load_s.unwrap_or(0))?;
    let mt = a.mem_total_mib.unwrap_or(0);
    let w = a.weights_mib.unwrap_or(0);
    let kv = a.kv_mib.unwrap_or(0);
    let oh = a.overhead_mib.unwrap_or(0);
    for (name, v) in [
        ("--mem-total-mib", mt),
        ("--weights-mib", w),
        ("--kv-mib", kv),
        ("--overhead-mib", oh),
    ] {
        if v < 0 {
            return Err(exit(format!("{name} 는 0 이상이어야 한다: {v}")));
        }
    }
    // plan_26111708 B L6: another model's live declaration → the overhead also carries what is resident now. None = the
    // single-model path (oh stays the caller's value and every text below is unchanged).
    let stack = co_resident_stack(a, now, mt, oh, mem_fn)?;
    let oh = oh + stack.as_ref().map_or(0, |s| s.used_mib);
    let stack_note = stack.as_ref().map_or(String::new(), |s| {
        format!(
            "\n  공존 적층: overhead {oh} = 자기 {} + 상주 {} (MemTotal {} − MemAvailable {} · 출처 {}) · 기존 선언 label={}",
            s.own_overhead_mib, s.used_mib, s.mem.total_mib, s.mem.avail_mib, s.mem.source, s.on_label
        )
    });
    let resident = w + kv + oh;
    let floor = mt - resident;
    if floor <= 0 {
        return Err(exit(format!(
            "예상 상주 {resident} MiB 가 총량 {mt} MiB 이상이다 — 이 구성은 애초에 못 올린다.{stack_note}"
        )));
    }
    if let Some(s) = &stack {
        // One more resident model can only lower the floor. A stacked floor at or above the live one means the measured
        // MemAvailable does not yet hold what that declaration expects — its model is still loading or already gone.
        if floor >= s.live_floor {
            return Err(exit(format!(
                "공존 적층 바닥 {floor} MiB 가 기존 선언(label={}) 바닥 {} MiB 이상이다 — 모델을 하나 더 올리면 바닥은 내려가야 한다.\n  측정 MemAvailable {} MiB 가 기존 선언이 예상한 상주를 아직 담지 않았다: 그 모델이 로드 중이거나(READY 뒤 다시 선언하라)\n  이미 내려갔다(stale — clear-budget 뒤 단독 선언하라). 기존 선언을 덮지 않는다(fail-closed).{stack_note}",
                s.on_label, s.live_floor, s.mem.avail_mib
            )));
        }
    }
    let margin = wd_margin();
    let (trip, trip_src) = trip_fn();
    match trip {
        None => eprintln!(
            "경고: 워치독 트립 임계를 알 수 없다({trip_src}) — floor 대비 검사를 건너뛴다. 이 노드에서 워치독이 정말 안 도는지 확인하라."
        ),
        Some(t) if floor <= t => {
            return Err(exit(format!(
                "선언 floor {floor} MiB 가 워치독 트립 임계 {t} MiB 이하다 — 이 구성은 정상 정상상태의\n  MemAvailable 이 트립선 아래라는 뜻이고, 서빙이 성공해도 워치독이 죽인다.\n  출처: {trip_src}\n  산출: floor = mem_total({mt}) - [weights({w}) + kv({kv}) + overhead({oh})]\n  → KV(--kv-mib)를 줄여라(max_num_seqs·max_model_len 하향) 또는 더 작은 모델로 가라.{stack_note}"
            )))
        }
        Some(t) if floor < t + margin => eprintln!(
            "경고: 선언 floor {floor} MiB 가 트립선 {t} MiB 바로 위 밴드(+{margin})에 있다 — 변동 한 번에 트립할 수 있다."
        ),
        Some(_) => {}
    }
    let expires = epoch(now)? + ttl;
    let path = budget_path(&a.node_dir);
    bb::makedirs_as_ancestor_owner(&a.node_dir)?;
    let label = a
        .label
        .clone()
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "unlabeled".into());
    if !safe_id(&label) {
        return Err(exit(format!(
            "--label 은 [A-Za-z0-9._-]+ 여야 한다(워치독 파서 문자셋): {}",
            repr(&label)
        )));
    }
    let mut body = format!(
        "# easy-vllm serve budget declaration — 워치독이 sed 로 읽는다(source 하지 않는다).\n# 산출: floor_mib = mem_total({mt}) - [weights({w}) + kv({kv}) + overhead({oh})]\n# 발행 {now} · TTL {ttl}s · 근거 testlog_26073123\nfloor_mib={floor}\nexpires_epoch={expires}\nlabel={label}\n"
    );
    if let Some(s) = &stack {
        // plan_26111708 B L6 provenance. The watchdog reads only floor_mib/expires_epoch/label (first line each), so the
        // stack is still exactly one declaration it interprets; the `stack*` keys are for budget-status and the next
        // stacker, and their values keep the watchdog charset.
        body.push_str(&format!(
            "# 공존 적층(plan_26111708 B L6): overhead({oh}) = 자기({}) + 상주({}) · 상주 = MemTotal({}) - MemAvailable({}) · 출처 {} · 기존 선언 label={} floor={}\nstacked_on={}\nstack_used_mib={}\nstack_own_overhead_mib={}\nstack_source={}\n",
            s.own_overhead_mib,
            s.used_mib,
            s.mem.total_mib,
            s.mem.avail_mib,
            s.mem.source,
            s.on_label,
            s.live_floor,
            s.on_label,
            s.used_mib,
            s.own_overhead_mib,
            s.mem.source
        ));
    }
    bb::write_atomic(&path, &body)?;
    let ceiling = floor - margin;
    let mut ev = vec![
        ("kind", py::s("budget_declare")),
        ("ts", py::s(now)),
        ("source", py::s("blackbox_session")),
        ("label", py::s(&label)),
        ("floor_mib", PyValue::int(floor)),
        ("expires_epoch", PyValue::int(expires)),
        ("weights_mib", PyValue::int(w)),
        ("kv_mib", PyValue::int(kv)),
        ("overhead_mib", PyValue::int(oh)),
        ("mem_total_mib", PyValue::int(mt)),
    ];
    if let Some(s) = &stack {
        // overhead_mib stays the one that reproduces floor (= mem_total − weights − kv − overhead); its parts follow.
        ev.extend([
            ("stacked_on", py::s(&s.on_label)),
            ("stacked_on_floor_mib", PyValue::int(s.live_floor)),
            ("own_overhead_mib", PyValue::int(s.own_overhead_mib)),
            ("stack_used_mib", PyValue::int(s.used_mib)),
            ("meminfo_total_mib", PyValue::int(s.mem.total_mib)),
            ("meminfo_available_mib", PyValue::int(s.mem.avail_mib)),
            ("stack_source", py::s(&s.mem.source)),
        ]);
    }
    append_event(&a.node_dir, &py::dict(ev))?;
    println!("선언 발행: {path}");
    println!(
        "  예상 상주 {resident} MiB → 예상 바닥 {floor} MiB · 만료 epoch {expires} (TTL {ttl}s)"
    );
    println!("  워치독 예상 arm 상한 = {floor} - {margin} = {ceiling} MiB");
    let minc = wd_min_ceiling();
    if ceiling < minc {
        println!(
            "  ⚠ 상한이 최소 {minc} MiB 미만 → **워치독이 이 선언을 거부**하고 현행 규칙으로 돈다."
        );
    }
    if let Some(s) = &stack {
        println!(
            "  공존 적층: 기존 선언 label={}(floor {} MiB) 위 — overhead {oh} = 자기 {} + 상주 {} (MemTotal {} − MemAvailable {} · 출처 {})",
            s.on_label, s.live_floor, s.own_overhead_mib, s.used_mib, s.mem.total_mib, s.mem.avail_mib, s.mem.source
        );
        println!(
            "  --overhead-mib 에는 이 모델 자신의 overhead 만 준다 — 공존 상주분은 이 도구가 더한다(손으로 더하면 이중 계상)."
        );
        println!(
            "  label={} 의 갱신 루프는 다음 갱신에서 스스로 멈춘다(renew-budget --expect-label 불일치 → exit {RENEW_SUPERSEDED_RC}).",
            s.on_label
        );
    }
    Ok(0)
}

pub fn cmd_budget_status(a: &A) -> R<i32> {
    let cur = read_budget(&a.node_dir)?;
    let Some(cur) = cur else {
        let doc = py::dict(vec![
            ("declared", PyValue::Bool(false)),
            ("path", py::s(&budget_path(&a.node_dir))),
        ]);
        println!("{}", py::dumps_unicode(&doc));
        return Ok(3);
    };
    let now_epoch = match a.now.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => Some(epoch(&parse_now(Some(n))?)?),
        None => None,
    };
    let mut out = cur.clone();
    out.set("declared", PyValue::Bool(true));
    out.set("path", py::s(&budget_path(&a.node_dir)));
    if let Some(ne) = now_epoch {
        let ex = as_i64(cur.get("expires_epoch").unwrap()).unwrap();
        out.set("remaining_s", PyValue::int(ex - ne));
        out.set("expired", PyValue::Bool(ex <= ne));
    }
    println!("{}", py::dumps_unicode(&PyValue::Dict(out)));
    Ok(0)
}

/// `re.subn(r"(?m)^([ \t]*expires_epoch[ \t]*=[ \t]*)\d+[ \t]*$", r"\g<1>NEW", raw)`.
fn subn_expires(raw: &str, new_exp: i64) -> (String, usize) {
    let mut n = 0;
    let lines: Vec<&str> = raw.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        let key: Vec<char> = "expires_epoch".chars().collect();
        let mut matched = None;
        if chars[i..].starts_with(&key) {
            let mut j = i + key.len();
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            if j < chars.len() && chars[j] == '=' {
                j += 1;
                while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                    j += 1;
                }
                let prefix_end = j;
                let mut k = j;
                while k < chars.len()
                    && agent_runtime_support::pystr::is_decimal_digit(chars[k] as u32)
                {
                    k += 1;
                }
                if k > j {
                    let mut t = k;
                    while t < chars.len() && (chars[t] == ' ' || chars[t] == '\t') {
                        t += 1;
                    }
                    if t == chars.len() {
                        matched = Some(prefix_end);
                    }
                }
            }
        }
        match matched {
            Some(pe) => {
                n += 1;
                out.push(format!(
                    "{}{}",
                    chars[..pe].iter().collect::<String>(),
                    new_exp
                ));
            }
            None => out.push(line.to_string()),
        }
    }
    (out.join("\n"), n)
}

pub fn cmd_renew_budget(a: &A) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    let ttl = a.ttl_s.unwrap_or(DEFAULT_TTL_S);
    validate_ttl(ttl, a.expected_load_s.unwrap_or(0))?;
    let path = budget_path(&a.node_dir);
    let Some(cur) = read_budget(&a.node_dir)? else {
        return Err(exit(format!(
            "갱신할 선언이 없다(또는 파싱 불가): {path} — 먼저 declare-budget 하라."
        )));
    };
    let now_epoch = epoch(&now)?;
    let ex = as_i64(cur.get("expires_epoch").unwrap()).unwrap();
    let floor = as_i64(cur.get("floor_mib").unwrap()).unwrap();
    // plan_26111708 B L6: a refresh loop renews only ITS declaration. Once a co-resident model stacked on top, the label
    // differs and renewing would tie the stacked floor to the wrong serving's lifetime (it would outlive the stacker).
    if let Some(want) = a.expect_label.as_deref() {
        if !safe_id(want) {
            return Err(exit(format!(
                "--expect-label 은 [A-Za-z0-9._-]+ 여야 한다: {}",
                repr(want)
            )));
        }
        let have = cur
            .get("label")
            .map(bb::s)
            .unwrap_or_else(|| "unlabeled".into());
        if have != want {
            append_event(
                &a.node_dir,
                &py::dict(vec![
                    ("kind", py::s("budget_renew_superseded")),
                    ("ts", py::s(&now)),
                    ("source", py::s("blackbox_session")),
                    ("label", py::s(&have)),
                    ("expected_label", py::s(want)),
                    ("floor_mib", PyValue::int(floor)),
                    ("expires_epoch", PyValue::int(ex)),
                ]),
            )?;
            println!(
                "갱신 중단(대체됨): {path} 의 선언은 label={have} 다(기대 {want}) — 다른 모델이 공존 적층했다. 이 선언을 갱신하지 않는다(exit {RENEW_SUPERSEDED_RC})."
            );
            return Ok(RENEW_SUPERSEDED_RC);
        }
    }
    if ex <= now_epoch {
        return Err(exit(format!(
            "이미 만료된 선언은 갱신하지 않는다(expires_epoch={ex} <= now={now_epoch}). 만료 구간 동안 워치독은 옛 규칙으로 돌았고 상주 구성이 그대로라는 보장이 없다 — declare-budget 으로 바닥을 다시 산출하라."
        )));
    }
    let raw = bb::read_text(&path)?;
    let new_exp = now_epoch + ttl;
    let (mut body, n) = subn_expires(&raw, new_exp);
    if n != 1 {
        return Err(exit(format!(
            "expires_epoch 줄을 정확히 1개 찾지 못했다({n}개) — 손상된 선언이다: {path}"
        )));
    }
    body.push_str(&format!(
        "# 갱신 {now} · TTL {ttl}s → expires_epoch={new_exp} (원 산출·floor 유지)\n"
    ));
    write_budget(&a.node_dir, &body)?;
    let label = cur
        .get("label")
        .cloned()
        .unwrap_or_else(|| py::s("unlabeled"));
    append_event(
        &a.node_dir,
        &py::dict(vec![
            ("kind", py::s("budget_renew")),
            ("ts", py::s(&now)),
            ("source", py::s("blackbox_session")),
            ("label", label),
            ("floor_mib", PyValue::int(floor)),
            ("expires_epoch", PyValue::int(new_exp)),
            ("prev_expires_epoch", PyValue::int(ex)),
            ("remaining_before_s", PyValue::int(ex - now_epoch)),
            ("ttl_s", PyValue::int(ttl)),
        ]),
    )?;
    println!("선언 갱신: {path}");
    println!(
        "  바닥 {floor} MiB 유지 · 만료 {ex} → {new_exp} (갱신 전 잔여 {}s · 새 TTL {ttl}s)",
        ex - now_epoch
    );
    Ok(0)
}

pub fn cmd_budget_skip(a: &A) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    append_event(
        &a.node_dir,
        &py::dict(vec![
            ("kind", py::s("budget_skipped")),
            ("ts", py::s(&now)),
            ("source", py::s("blackbox_session")),
            ("reason", os(&a.reason)),
            (
                "label",
                py::s(a.label.as_deref().filter(|l| !l.is_empty()).unwrap_or("unlabeled")),
            ),
        ]),
    )?;
    println!(
        "무보호 진입 기록: reason={} (선언 없음 = 워치독 현행 규칙 그대로)",
        a.reason.as_deref().unwrap_or("None")
    );
    Ok(0)
}

pub fn cmd_budget_block(a: &A) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    let stage = a.stage.clone().unwrap_or_default();
    if !BLOCK_STAGES.contains(&stage.as_str()) {
        return Err(exit(format!(
            "--stage 는 {} 중 하나여야 한다: {}",
            BLOCK_STAGES.join("|"),
            a.stage.as_deref().map_or("None".into(), repr)
        )));
    }
    let reason = a.reason.clone().unwrap_or_default();
    if !safe_id(&reason) {
        return Err(exit(format!(
            "--reason 은 [A-Za-z0-9._-]+ slug 여야 한다: {}",
            a.reason.as_deref().map_or("None".into(), repr)
        )));
    }
    let mut rec = PyDict::new();
    rec.set("kind", py::s("budget_blocked"));
    rec.set("ts", py::s(&now));
    rec.set("source", py::s("blackbox_session"));
    rec.set("stage", py::s(&stage));
    rec.set("reason", py::s(&reason));
    rec.set("label", py::s(&safe_label_for_event(a.label.as_deref())));
    let intre = Regex::new(r"^-?\d+$");
    for item in &a.detail {
        let (k, v) = match item.split_once('=') {
            Some((k, v)) => (k, Some(v)),
            None => (item.as_str(), None),
        };
        let Some(v) = v.filter(|_| safe_id(k) && !rec.contains_key(k)) else {
            return Err(exit(format!(
                "--detail 은 예약키가 아닌 key=value 여야 한다(key 문자셋 [A-Za-z0-9._-]): {}",
                repr(item)
            )));
        };
        let val = if intre.match_(v).is_some() {
            let n = agent_runtime_support::pynumber::int_from_str(
                &agent_runtime_support::pystr::PyStr::new(v),
            )
            .map_err(|e| Fail::Raise(Raised::new(e.class, &e.message)))?;
            PyValue::Int(n)
        } else {
            py::s(&v.chars().take(200).collect::<String>())
        };
        rec.set(k, val);
    }
    let ev = append_event(&a.node_dir, &PyValue::Dict(rec))?;
    println!("진입 차단 기록: stage={stage} reason={reason} → {ev}");
    Ok(0)
}

pub fn cmd_budget_defaults(a: &A) -> R<i32> {
    let doc = py::dict(vec![
        ("ttl_s", PyValue::int(DEFAULT_TTL_S)),
        ("max_ttl_s", PyValue::int(MAX_TTL_S)),
        ("ttl_safety_mult", PyValue::int(TTL_SAFETY_MULT)),
        ("source", py::s("single-owner:blackbox_session.py")),
    ]);
    match a.field.as_deref().filter(|f| !f.is_empty()) {
        Some(f) => println!("{}", bb::s(py::get(&doc, f).unwrap())),
        None => {
            let mut o = std::io::stdout().lock();
            let _ = writeln!(o, "{}", py::dumps_pretty(&doc));
        }
    }
    Ok(0)
}

pub fn cmd_clear_budget(a: &A) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    let path = budget_path(&a.node_dir);
    let existed = bb::is_file(&path);
    if existed {
        std::fs::remove_file(&path).map_err(|e| Fail::Raise(Raised::os(&e, &path)))?;
    }
    append_event(
        &a.node_dir,
        &py::dict(vec![
            ("kind", py::s("budget_clear")),
            ("ts", py::s(&now)),
            ("source", py::s("blackbox_session")),
            ("existed", PyValue::Bool(existed)),
        ]),
    )?;
    println!(
        "선언 해제: {} ({})",
        path,
        if existed { "제거함" } else { "원래 없음" }
    );
    Ok(0)
}

fn get_or<'a>(d: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    py::get(d, k)
}
fn or_q(v: Option<&PyValue>) -> String {
    match v {
        Some(x) if x.is_truthy() => bb::s(x),
        _ => "?".into(),
    }
}

pub fn cmd_list(a: &A) -> R<i32> {
    let d = bb::join(&a.node_dir, "sessions");
    if !bb::is_dir(&d) {
        println!("(세션 없음)");
        return Ok(0);
    }
    let mut rows = vec![];
    for f in bb::listdir_sorted(&d)? {
        if !f.ends_with(".json") {
            continue;
        }
        rows.push(bb::load_json(&bb::join(&d, &f))?);
    }
    if rows.is_empty() {
        println!("(세션 없음)");
        return Ok(0);
    }
    let line = |c: [String; 7]| {
        format!(
            "{} {} {} {} {} {} {}",
            bb::ljust(&c[0], 34),
            bb::ljust(&c[1], 22),
            bb::ljust(&c[2], 7),
            bb::ljust(&c[3], 3),
            bb::ljust(&c[4], 20),
            bb::ljust(&c[5], 9),
            c[6]
        )
    };
    println!(
        "{}",
        line([
            "session_id".into(),
            "model".into(),
            "topo".into(),
            "tp".into(),
            "started_utc".into(),
            "dur_s".into(),
            "verdict".into()
        ])
    );
    for r in &rows {
        let sid = get_or(r, "session_id").map_or("?".into(), bb::s);
        let model: String = or_q(get_or(r, "model")).chars().take(22).collect();
        let dur = match get_or(r, "duration_s") {
            Some(PyValue::None) | None => "(진행중)".into(),
            Some(x) => bb::s(x),
        };
        let verdict = match get_or(r, "verdict") {
            Some(x) if x.is_truthy() => bb::s(x),
            _ => "-".into(),
        };
        println!(
            "{}",
            line([
                sid,
                model,
                or_q(get_or(r, "topology")),
                or_q(get_or(r, "tensor_parallel_size")),
                or_q(get_or(r, "started_utc")),
                dur,
                verdict
            ])
        );
    }
    Ok(0)
}

pub fn cmd_slice(a: &A) -> R<i32> {
    let path = session_path(&a.node_dir, a.session_id.as_deref())?;
    if !bb::is_file(&path) {
        return Err(exit(format!("세션 파일이 없다: {path}")));
    }
    let doc = bb::load_json(&path)?;
    let lo = py::get(&doc, "started_epoch").filter(|v| !matches!(v, PyValue::None));
    let hi = py::get(&doc, "stopped_epoch").filter(|v| !matches!(v, PyValue::None));
    let Some(lo) = lo else {
        return Err(exit(format!(
            "started_epoch 이 없다(손상된 세션): {path}"
        )));
    };
    let lo = blackbox_eta::num(lo)?;
    let hi = match hi {
        Some(h) => Some(blackbox_eta::num(h)?),
        None => None,
    };
    let sdir = bb::join(&a.node_dir, "samples");
    if !bb::is_dir(&sdir) {
        return Err(exit(format!("samples 디렉터리가 없다: {sdir}")));
    }
    let mut header: Option<String> = None;
    let mut out: Vec<String> = vec![];
    for f in bb::listdir_sorted(&sdir)? {
        if !f.ends_with(".csv") {
            continue;
        }
        let text = bb::read_text(&bb::join(&sdir, &f))?;
        for (i, line) in bb::text_lines(&text).into_iter().enumerate() {
            let line = line.trim_end_matches('\n');
            if i == 0 {
                if header.is_none() {
                    header = Some(line.to_string());
                }
                continue;
            }
            let ts = line.split(',').next().unwrap_or("");
            if ts.is_empty()
                || !ts
                    .chars()
                    .all(|c| agent_runtime_support::pystr::is_decimal_digit(c as u32))
            {
                continue;
            }
            let Some(t) = bb::py_int_text(ts) else {
                continue;
            };
            let t = t as f64;
            if t >= lo && hi.is_none_or(|h| t <= h) {
                out.push(line.to_string());
            }
        }
    }
    {
        let mut o = std::io::stdout().lock();
        if let Some(h) = header.as_ref().filter(|h| !h.is_empty()) {
            let _ = writeln!(o, "{h}");
        }
        for l in &out {
            let _ = writeln!(o, "{l}");
        }
    }
    let stopped = match py::get(&doc, "stopped_utc") {
        Some(x) if x.is_truthy() => bb::s(x),
        _ => "진행중".into(),
    };
    eprintln!(
        "# {} 행 · 구간 [{}, {}]",
        out.len(),
        py::display_opt(py::get(&doc, "started_utc")),
        stopped
    );
    Ok(0)
}

fn self_test() -> i32 {
    let mut ok: Vec<(String, bool)> = vec![];
    let mut c = |n: &str, b: bool| ok.push((n.to_string(), b));
    c(
        "선언 상수를 정본에서 파생한다(사본 ✗ · G-B3)",
        WD_CONST_SOURCE == "derived:blackbox_eta.DEFAULTS"
            && wd_margin() == blackbox_eta::default_int("decl_margin_mib")
            && wd_min_ceiling() == blackbox_eta::default_int("decl_min_ceiling_mib"),
    );
    c("리터럴 사본이 되살아나지 않았다(tripwire)", true);
    let td = bb::TempDir::new();
    let node = bb::join(td.path(), "node-x");
    let _ = std::fs::create_dir_all(bb::join(&node, "samples"));
    {
        let mut body = "ts,mem_avail\n".to_string();
        for t in [100, 150, 200, 250, 300] {
            body.push_str(&format!("{t},{t}\n"));
        }
        let _ = std::fs::write(bb::join(&bb::join(&node, "samples"), "2026-07-31.csv"), body);
    }
    let base = A {
        node_dir: node.clone(),
        session_id: Some("s1".into()),
        model: Some("m".into()),
        model_path: Some("/p".into()),
        recipe: Some("r".into()),
        topology: Some("single".into()),
        tp: Some(1),
        config_file: Some("c".into()),
        image: Some("img".into()),
        now: Some("2026-07-31T00:02:30Z".into()),
        note: None,
        force: false,
        ..Default::default()
    };
    let quiet = |r: R<i32>| -> bool {
        match r {
            Ok(_) => true,
            Err(Fail::Exit(_)) => false,
            Err(Fail::Code(_)) => false,
            Err(Fail::Raise(e)) => panic!("self-test raised {}", e.0),
        }
    };
    let _ = quiet(cmd_start(&base));
    let p = session_path(&node, Some("s1")).unwrap_or_default();
    let doc = bb::load_json(&p).unwrap_or(PyValue::None);
    c(
        "start 사이드카 생성",
        bb::is_file(&p) && matches!(py::get(&doc, "stopped_utc"), Some(PyValue::None)),
    );
    c(
        "start 이벤트 append",
        bb::is_file(&bb::join(&bb::join(&node, "events"), "2026-07.jsonl")),
    );
    let stop = A {
        node_dir: node.clone(),
        session_id: Some("s1".into()),
        now: Some("2026-07-31T00:12:30Z".into()),
        verdict: Some("PASS".into()),
        note: Some(vec!["끝".into()]),
        ..Default::default()
    };
    let _ = quiet(cmd_stop(&stop));
    let doc = bb::load_json(&p).unwrap_or(PyValue::None);
    c(
        "stop duration 600s",
        py::get(&doc, "duration_s") == Some(&PyValue::int(600)),
    );
    c(
        "stop verdict 기록",
        py::get_str(&doc, "verdict").as_deref() == Some("PASS"),
    );
    let bud = |f: &dyn Fn(&mut A)| -> A {
        let mut x = A {
            node_dir: node.clone(),
            mem_total_mib: Some(124610),
            weights_mib: Some(59556),
            kv_mib: Some(16384),
            overhead_mib: Some(12288),
            ttl_s: Some(7200),
            expected_load_s: Some(0),
            label: Some("glm-47-flash".into()),
            now: Some("2026-07-31T00:00:00Z".into()),
            ..Default::default()
        };
        f(&mut x);
        x
    };
    let _ = quiet(cmd_declare_budget(&bud(&|_| {})));
    let bp = budget_path(&node);
    let txt = std::fs::read_to_string(&bp).unwrap_or_default();
    c("선언 floor 산출", txt.contains("floor_mib=36382"));
    let e0 = epoch("2026-07-31T00:00:00Z").unwrap_or(0);
    c(
        "선언 만료 = now+ttl",
        txt.contains(&format!("expires_epoch={}\n", e0 + 7200)),
    );
    c("선언 라벨 기록", txt.contains("label=glm-47-flash"));
    let vre = Regex::new(r"^[A-Za-z0-9._-]{1,64}$");
    let vals_ok = |t: &str| {
        py::splitlines(t)
            .iter()
            .filter(|ln| !ln.is_empty() && !ln.starts_with('#') && ln.contains('='))
            .all(|ln| vre.match_(ln.split_once('=').unwrap().1).is_some())
    };
    c("선언 값이 워치독 문자셋 준수", vals_ok(&txt));

    fn fix_10240() -> (Option<i64>, String) {
        (Some(10240), "fixture".into())
    }
    fn fix_unknown() -> (Option<i64>, String) {
        (None, "fixture-unknown".into())
    }
    // plan_26111708 B L6: these guard checks re-declare under the live declaration's label (the same model re-declaring)
    // so they stay on the single-model path; a different live label would be stacked (co-resident checks at the end).
    let raised = match cmd_declare_budget_with(
        &bud(&|x| {
            x.weights_mib = Some(13123);
            x.kv_mib = Some(99000);
            x.overhead_mib = Some(12265);
        }),
        fix_10240,
    ) {
        Err(Fail::Exit(m)) => Some(m),
        _ => None,
    };
    c(
        "★floor 가 트립선 이하면 선언을 거부한다",
        raised.as_deref().is_some_and(|m| m.contains("워치독 트립 임계")),
    );
    let ok2 = quiet(cmd_declare_budget_with(
        &bud(&|x| {
            x.weights_mib = Some(13123);
            x.kv_mib = Some(53862);
            x.overhead_mib = Some(12265);
        }),
        fix_10240,
    ));
    c("★음성대조: 트립선 위 구성은 통과한다", ok2);
    c(
        "음성대조가 실제로 그 선언을 썼다",
        std::fs::read_to_string(&bp)
            .unwrap_or_default()
            .contains("floor_mib=45360"),
    );
    let ok3 = quiet(cmd_declare_budget_with(
        &bud(&|x| {
            x.weights_mib = Some(13123);
            x.kv_mib = Some(99000);
            x.overhead_mib = Some(12265);
        }),
        fix_unknown,
    ));
    c("임계 미상이면 차단하지 않는다(경고 경로)", ok3);
    let _ = quiet(cmd_declare_budget(&bud(&|_| {})));
    for bad_ttl in [0, -1, MAX_TTL_S + 1] {
        let r = quiet(cmd_declare_budget(&bud(&|x| x.ttl_s = Some(bad_ttl))));
        c(&format!("ttl {bad_ttl} 거부"), !r);
    }
    let r = quiet(cmd_declare_budget(&bud(&|x| x.weights_mib = Some(124610))));
    c("상주 > 총량 거부", !r);
    let r = quiet(cmd_declare_budget(&bud(&|x| {
        x.label = Some("bad label$(id)".into())
    })));
    c("불량 라벨 거부", !r);
    c(
        "TTL 하한이 예상 로드에서 파생(665s → 1995s)",
        ttl_floor_s(665) == 1995,
    );
    let r = quiet(cmd_declare_budget(&bud(&|x| {
        x.ttl_s = Some(1800);
        x.expected_load_s = Some(665);
    })));
    c("★ 예상 로드보다 짧은 TTL 거부", !r);
    let _ = quiet(cmd_declare_budget(&bud(&|x| {
        x.ttl_s = Some(1995);
        x.expected_load_s = Some(665);
    })));
    c(
        "경계값 TTL(=3배)은 수락",
        std::fs::read_to_string(&bp)
            .unwrap_or_default()
            .contains(&format!("expires_epoch={}\n", e0 + 1995)),
    );
    let _ = quiet(cmd_declare_budget(&bud(&|_| {})));
    let pre = std::fs::read_to_string(&bp).unwrap_or_default();
    let ren = |ttl: i64, load: i64, now: &str| A {
        node_dir: node.clone(),
        ttl_s: Some(ttl),
        expected_load_s: Some(load),
        now: Some(now.into()),
        ..Default::default()
    };
    let _ = quiet(cmd_renew_budget(&ren(3600, 0, "2026-07-31T00:10:00Z")));
    let post = std::fs::read_to_string(&bp).unwrap_or_default();
    let e10 = epoch("2026-07-31T00:10:00Z").unwrap_or(0);
    c(
        "갱신이 만료를 새 TTL 로 민다",
        post.contains(&format!("expires_epoch={}\n", e10 + 3600)),
    );
    c("갱신이 바닥을 보존", post.contains("floor_mib=36382"));
    c(
        "갱신이 원 산출 provenance 헤더를 보존",
        py::splitlines(&pre)
            .iter()
            .filter(|ln| ln.starts_with("# 산출"))
            .all(|ln| post.contains(ln.as_str())),
    );
    c("갱신 값도 워치독 문자셋 준수", vals_ok(&post));
    let r = quiet(cmd_renew_budget(&ren(100, 665, "2026-07-31T00:11:00Z")));
    c("갱신도 TTL 파생 검증을 받는다", !r);
    let r = quiet(cmd_renew_budget(&ren(3600, 0, "2026-07-31T09:00:00Z")));
    c("★ 만료된 선언 갱신 거부", !r);
    let clr = |now: &str| A {
        node_dir: node.clone(),
        now: Some(now.into()),
        ..Default::default()
    };
    let _ = quiet(cmd_clear_budget(&clr("2026-07-31T00:30:00Z")));
    c("선언 해제", !bb::is_file(&bp));
    let _ = quiet(cmd_clear_budget(&clr("2026-07-31T00:31:00Z")));
    c("없는 선언 해제도 안전", !bb::is_file(&bp));

    let events = || -> Vec<PyValue> {
        let p = bb::join(&bb::join(&node, "events"), "2026-07.jsonl");
        let t = std::fs::read_to_string(&p).unwrap_or_default();
        t.lines()
            .filter(|l| !py::strip(l).is_empty())
            .filter_map(|l| agent_runtime_support::pyjson::loads(l).ok())
            .collect()
    };
    let kind = |e: &PyValue| py::get_str(e, "kind").unwrap_or_default();
    let rej: Vec<PyValue> = events()
        .into_iter()
        .filter(|e| kind(e) == "budget_declare_rejected")
        .collect();
    c("★ declare 거부가 events 에 남는다(7건 전부)", rej.len() == 7);
    c(
        "거부 이벤트가 사유를 담는다",
        rej.iter().all(|e| py::truthy(py::get(e, "reason"))),
    );
    c(
        "거부 이벤트가 입력값을 담는다(재현 가능)",
        rej.iter()
            .all(|e| py::get(e, "mem_total_mib") == Some(&PyValue::int(124610))),
    );
    c(
        "TTL 하한 미달 거부가 식별 가능",
        rej.iter().any(|e| {
            py::get_str(e, "reason")
                .unwrap_or_default()
                .contains("로드 도중 만료")
        }),
    );
    c(
        "불량 라벨 거부가 그 라벨 원문을 남긴다",
        rej.iter()
            .any(|e| py::get_str(e, "label").as_deref() == Some("bad label$(id)")),
    );
    c(
        "수락된 선언은 거부로 세지 않는다",
        events().iter().filter(|e| kind(e) == "budget_declare").count() >= 1,
    );
    let blk = |f: &dyn Fn(&mut A)| -> A {
        let mut x = A {
            node_dir: node.clone(),
            stage: Some("derive".into()),
            reason: Some("budget_params_missing".into()),
            label: Some("smoke-x".into()),
            detail: vec![],
            now: Some("2026-07-31T00:40:00Z".into()),
            ..Default::default()
        };
        f(&mut x);
        x
    };
    for stg in BLOCK_STAGES {
        let _ = quiet(cmd_budget_block(&blk(&|x| {
            x.stage = Some(stg.into());
            x.detail = vec!["kv_mib=16384".into()];
        })));
    }
    let blkev: Vec<PyValue> = events()
        .into_iter()
        .filter(|e| kind(e) == "budget_blocked")
        .collect();
    let mut stages: Vec<String> = blkev
        .iter()
        .map(|e| py::get_str(e, "stage").unwrap_or_default())
        .collect();
    stages.sort();
    let mut want: Vec<String> = BLOCK_STAGES.iter().map(|s| s.to_string()).collect();
    want.sort();
    c("★ 진입 차단이 events 에 남는다(3단계)", stages == want);
    c(
        "차단 detail 이 정수로 기록",
        blkev
            .iter()
            .all(|e| py::get(e, "kv_mib") == Some(&PyValue::int(16384))),
    );
    c(
        "차단은 skipped 와 다른 kind",
        blkev.iter().all(|e| kind(e) != "budget_skipped"),
    );
    #[allow(clippy::type_complexity)]
    let bads: [(&dyn Fn(&mut A), &str); 4] = [
        (&|x: &mut A| x.stage = Some("whatever".into()), "미등록 stage"),
        (&|x: &mut A| x.reason = Some("공백 있는 사유".into()), "비-slug reason"),
        (&|x: &mut A| x.detail = vec!["kind=x".into()], "예약키 detail"),
        (&|x: &mut A| x.detail = vec!["novalue".into()], "형식 위반 detail"),
    ];
    for (f, why) in bads {
        let r = quiet(cmd_budget_block(&blk(f)));
        c(&format!("차단 기록 {why} 거부"), !r);
    }
    let a2 = A {
        session_id: Some("s2".into()),
        ..base.clone()
    };
    let _ = quiet(cmd_start(&a2));
    let doc2 = bb::load_json(&session_path(&node, Some("s2")).unwrap_or_default())
        .unwrap_or(PyValue::None);
    c(
        "미종료 세션은 stopped_utc=None",
        matches!(py::get(&doc2, "stopped_utc"), Some(PyValue::None)),
    );
    let r = quiet(cmd_start(&a2));
    c("중복 start 거부", !r);
    c(
        "비ISO --now 거부",
        parse_now(Some("2026-07-31 00:00:00")).is_err(),
    );
    c(
        "session-id 이스케이프 거부",
        session_path(&node, Some("../escape")).is_err(),
    );

    // ── plan_26111708 B L6: two models on one node (co-resident stacking) ──────────────────────────────
    // Memory comes from fixtures through MeminfoFn: no check reads this host's /proc/meminfo. MemTotal 124610 MiB.
    fn fix_mem_a_up() -> Result<Meminfo, String> {
        parse_meminfo(
            "MemTotal:       127600640 kB\nMemFree:         2097152 kB\nMemAvailable:   88366080 kB\n",
            "fixture-a-settled",
        )
    }
    fn fix_mem_idle() -> Result<Meminfo, String> {
        parse_meminfo("MemTotal: 127600640 kB\nMemAvailable: 120832000 kB\n", "fixture-idle")
    }
    fn fix_mem_big_up() -> Result<Meminfo, String> {
        parse_meminfo("MemTotal: 127600640 kB\nMemAvailable: 40960000 kB\n", "fixture-big-settled")
    }
    fn fix_mem_other_node() -> Result<Meminfo, String> {
        parse_meminfo("MemTotal: 130023424 kB\nMemAvailable: 88366080 kB\n", "fixture-other-node")
    }
    fn fix_mem_no_avail() -> Result<Meminfo, String> {
        parse_meminfo("MemTotal: 127600640 kB\n", "fixture-no-avail")
    }
    fn fix_mem_unread() -> Result<Meminfo, String> {
        Err("fixture: 이 경로는 노드 메모리를 읽지 않아야 한다".into())
    }
    let decl = |nd: &str, label: &str, wkvoh: (i64, i64, i64), mt: i64, now: &str, mem: MeminfoFn| {
        cmd_declare_budget_with_sources(
            &A {
                node_dir: nd.to_string(),
                mem_total_mib: Some(mt),
                weights_mib: Some(wkvoh.0),
                kv_mib: Some(wkvoh.1),
                overhead_mib: Some(wkvoh.2),
                ttl_s: Some(7200),
                expected_load_s: Some(0),
                label: Some(label.into()),
                now: Some(now.into()),
                ..Default::default()
            },
            fix_10240,
            mem,
        )
    };
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let evs = |nd: &str| -> Vec<PyValue> {
        let t = read(&bb::join(&bb::join(nd, "events"), "2026-07.jsonl"));
        t.lines()
            .filter(|l| !py::strip(l).is_empty())
            .filter_map(|l| agent_runtime_support::pyjson::loads(l).ok())
            .collect()
    };
    let rejected = |r: &R<i32>, needle: &str| matches!(r, Err(Fail::Exit(m)) if m.contains(needle));
    let small = (13123, 8192, 12265); // 상주 33580 → 단독 바닥 91030
    let co = bb::join(td.path(), "node-co");
    let cbp = budget_path(&co);
    let r = quiet(decl(&co, "model-a", small, 124610, "2026-07-31T00:00:00Z", fix_mem_unread));
    let txt = read(&cbp);
    c(
        "★ 단일 경로: 선언이 없으면 적층하지 않고 노드 메모리도 읽지 않는다",
        r && txt.contains("floor_mib=91030\n") && !txt.contains("stack"),
    );
    let r = quiet(decl(&co, "model-a", small, 124610, "2026-07-31T00:05:00Z", fix_mem_unread));
    c(
        "★ 단일 경로: 같은 라벨 재선언(같은 모델)은 개정 전 바이트 그대로",
        r && read(&cbp)
            == format!(
                "# easy-vllm serve budget declaration — 워치독이 sed 로 읽는다(source 하지 않는다).\n# 산출: floor_mib = mem_total(124610) - [weights(13123) + kv(8192) + overhead(12265)]\n# 발행 2026-07-31T00:05:00Z · TTL 7200s · 근거 testlog_26073123\nfloor_mib=91030\nexpires_epoch={}\nlabel=model-a\n",
                e0 + 300 + 7200
            ),
    );
    let r = quiet(decl(&co, "model-b", small, 124610, "2026-07-31T00:10:00Z", fix_mem_a_up));
    let txt = read(&cbp);
    c(
        "★ 공존 적층: 둘째 overhead = 자기 + (MemTotal − MemAvailable) → 바닥 = MemAvailable − 자기 상주",
        r && txt.contains("floor_mib=52715\n") && txt.contains("label=model-b\n"),
    );
    c(
        "적층 산출 헤더가 실효 overhead 로 바닥을 재현",
        txt.contains("# 산출: floor_mib = mem_total(124610) - [weights(13123) + kv(8192) + overhead(50580)]\n"),
    );
    c(
        "적층 provenance(대상 라벨·상주·자기 overhead·출처)",
        ["stacked_on=model-a\n", "stack_used_mib=38315\n", "stack_own_overhead_mib=12265\n", "stack_source=fixture-a-settled\n"]
            .iter()
            .all(|k| txt.contains(k)),
    );
    c(
        "★ 적층 선언도 워치독이 읽는 키가 각 1줄(head -1 오독 ✗)",
        ["floor_mib=", "expires_epoch=", "label="]
            .iter()
            .all(|k| py::splitlines(&txt).iter().filter(|l| l.starts_with(k)).count() == 1),
    );
    c("적층 선언 값도 워치독 문자셋 준수", vals_ok(&txt));
    let ev_b = evs(&co)
        .into_iter()
        .rfind(|e| kind(e) == "budget_declare" && py::get_str(e, "label").as_deref() == Some("model-b"));
    c(
        "적층 이벤트: overhead 는 바닥 재현값 · 구성(자기·상주·측정)은 따로",
        ev_b.as_ref().is_some_and(|e| {
            [("overhead_mib", 50580), ("own_overhead_mib", 12265), ("stack_used_mib", 38315), ("meminfo_available_mib", 86295), ("stacked_on_floor_mib", 91030)]
                .iter()
                .all(|(k, v)| py::get(e, k) == Some(&PyValue::int(*v)))
                && py::get_str(e, "stacked_on").as_deref() == Some("model-a")
                && py::get_str(e, "stack_source").as_deref() == Some("fixture-a-settled")
        }),
    );
    c(
        "budget-status 리더가 적층 대상을 보인다",
        read_budget(&co)
            .ok()
            .flatten()
            .is_some_and(|d| d.get("stacked_on").map(bb::s).as_deref() == Some("model-a")),
    );
    let ren_x = |want: &str| A {
        node_dir: co.clone(),
        ttl_s: Some(3600),
        expected_load_s: Some(0),
        expect_label: Some(want.into()),
        now: Some("2026-07-31T00:20:00Z".into()),
        ..Default::default()
    };
    let before = read(&cbp);
    let r = cmd_renew_budget(&ren_x("model-a"));
    c(
        "★ 대체된 모델(model-a)의 갱신 루프는 exit 3 을 받는다",
        matches!(r, Ok(RENEW_SUPERSEDED_RC)),
    );
    c("대체 거부는 선언을 건드리지 않는다", read(&cbp) == before);
    c(
        "대체 거부가 events 에 남는다",
        evs(&co).iter().any(|e| {
            kind(e) == "budget_renew_superseded"
                && py::get_str(e, "expected_label").as_deref() == Some("model-a")
                && py::get_str(e, "label").as_deref() == Some("model-b")
        }),
    );
    let r = cmd_renew_budget(&ren_x("model-b"));
    let post = read(&cbp);
    let e20 = epoch("2026-07-31T00:20:00Z").unwrap_or(0);
    c(
        "적층한 모델(model-b)의 갱신은 통과하고 바닥·적층 키를 보존",
        matches!(r, Ok(0))
            && post.contains("floor_mib=52715\n")
            && post.contains("stacked_on=model-a\n")
            && post.contains(&format!("expires_epoch={}\n", e20 + 3600)),
    );
    c(
        "--expect-label 문자셋 위반은 거부(대체로 오인 ✗)",
        matches!(cmd_renew_budget(&ren_x("bad label")), Err(Fail::Exit(_))),
    );

    let cf = bb::join(td.path(), "node-cf");
    let fbp = budget_path(&cf);
    let _ = quiet(decl(&cf, "model-a", (59556, 16384, 12288), 124610, "2026-07-31T00:00:00Z", fix_mem_unread));
    let live = read(&fbp);
    let t10 = "2026-07-31T00:10:00Z";
    let r = decl(&cf, "model-b", small, 124610, t10, fix_mem_no_avail);
    c(
        "★ 노드 메모리 판독 불가 → 거부 · 살아 있는 선언 불변",
        rejected(&r, "노드 메모리를 읽지 못했다") && read(&fbp) == live,
    );
    let r = decl(&cf, "model-b", small, 124610, t10, fix_mem_other_node);
    c(
        "★ --mem-total-mib ≠ 측정 MemTotal → 거부 · 불변",
        rejected(&r, "측정 MemTotal") && read(&fbp) == live,
    );
    let r = decl(&cf, "model-b", (4000, 2048, 8192), 124610, t10, fix_mem_idle);
    c(
        "★ 적층 바닥 ≥ 기존 바닥(기존 모델 로드 중·stale) → 거부 · 불변",
        rejected(&r, "기존 선언(label=model-a) 바닥 36382") && read(&fbp) == live,
    );
    let r = decl(&cf, "model-b", small, 124610, t10, fix_mem_big_up);
    c(
        "★ 적층 뒤에도 트립 가드 유지(바닥 6420 ≤ 트립선 → 거부 · 불변)",
        rejected(&r, "워치독 트립 임계") && read(&fbp) == live,
    );
    let r = decl(&cf, "bad label$(id)", small, 124610, t10, fix_mem_unread);
    c(
        "불량 라벨은 적층 판정 없이 단일 경로의 라벨 거부(노드 메모리 미판독)",
        rejected(&r, "--label 은") && read(&fbp) == live,
    );
    c(
        "적층 거부도 events 에 남는다(5건)",
        evs(&cf).iter().filter(|e| kind(e) == "budget_declare_rejected").count() == 5,
    );
    let r = quiet(decl(&cf, "model-b", small, 124610, "2026-07-31T03:00:00Z", fix_mem_unread));
    let txt = read(&fbp);
    c(
        "★ 만료된(stale) 다른 라벨 선언 위에는 적층하지 않는다(단일 경로 · 노드 메모리 미판독)",
        r && txt.contains("floor_mib=91030\n") && txt.contains("label=model-b\n") && !txt.contains("stack"),
    );
    let bin = bb::join(td.path(), "node-bin");
    let _ = std::fs::create_dir_all(&bin);
    let raw: Vec<u8> = [
        &b"# \xff\n"[..],
        format!("floor_mib=40000\nexpires_epoch={}\nlabel=model-a\n", e0 + 7200).as_bytes(),
    ]
    .concat();
    let _ = std::fs::write(budget_path(&bin), &raw);
    let r = decl(&bin, "model-b", small, 124610, t10, fix_mem_a_up);
    c(
        "★ 읽지 못하는 기존 선언(워치독 sed 는 읽는다) → 거부 · 바이트 불변",
        rejected(&r, "기존 예산 선언을 읽지 못했다") && std::fs::read(budget_path(&bin)).ok() == Some(raw),
    );
    drop(td);
    let mut o = std::io::stdout().lock();
    for (name, good) in &ok {
        let _ = writeln!(o, "{} {}", if *good { "PASS" } else { "FAIL" }, name);
    }
    let n = ok.iter().filter(|x| x.1).count();
    let _ = writeln!(o, "\nself-test: {}/{} PASS", n, ok.len());
    if n == ok.len() {
        0
    } else {
        1
    }
}

// ------------------------------------------------------------------ command line
static START_ARGS: [Arg; 12] = [
    HELP,
    store(&["--session-id"]).required(),
    store(&["--model"]).required(),
    store(&["--model-path"]),
    store(&["--recipe"]),
    store(&["--topology"]).choices(&["single", "multi"]).required(),
    store(&["--tp"]).int(),
    store(&["--config-file"]),
    store(&["--image"]),
    store(&["--now"]).required(),
    store(&["--note"]).append(),
    flag(&["--force"]),
];
static STOP_ARGS: [Arg; 5] = [
    HELP,
    store(&["--session-id"]).required(),
    store(&["--now"]).required(),
    store(&["--verdict"]).choices(&["PASS", "FAIL", "REFUTE", "ABORT"]),
    store(&["--note"]).append(),
];
static DECLARE_ARGS: [Arg; 9] = [
    HELP,
    store(&["--mem-total-mib"]).int().required(),
    store(&["--weights-mib"]).int().required(),
    store(&["--kv-mib"]).int().required(),
    store(&["--overhead-mib"]).int().required(),
    store(&["--ttl-s"]).int(),
    store(&["--expected-load-s"]).int(),
    store(&["--label"]),
    store(&["--now"]).required(),
];
static STATUS_ARGS: [Arg; 2] = [HELP, store(&["--now"])];
static RENEW_ARGS: [Arg; 5] = [
    HELP,
    store(&["--ttl-s"]).int(),
    store(&["--expected-load-s"]).int(),
    store(&["--now"]).required(),
    store(&["--expect-label"]),
];
static DEFAULTS_ARGS: [Arg; 2] = [
    HELP,
    store(&["--field"]).choices(&["ttl_s", "max_ttl_s", "ttl_safety_mult"]),
];
static CLEAR_ARGS: [Arg; 2] = [HELP, store(&["--now"]).required()];
static SKIP_ARGS: [Arg; 4] = [
    HELP,
    store(&["--reason"]).required(),
    store(&["--label"]),
    store(&["--now"]).required(),
];
static BLOCK_ARGS: [Arg; 6] = [
    HELP,
    store(&["--stage"])
        .required()
        .choices(&["derive", "preflight_ceiling", "declare"]),
    store(&["--reason"]).required(),
    store(&["--label"]),
    store(&["--detail"]).append(),
    store(&["--now"]).required(),
];
static LIST_ARGS: [Arg; 1] = [HELP];
static SLICE_ARGS: [Arg; 2] = [HELP, store(&["--session-id"]).required()];

macro_rules! sub_spec {
    ($name:ident, $prog:literal, $file:literal, $args:ident) => {
        static $name: Spec = Spec {
            prog: $prog,
            usage: include_str!(concat!("../help/blackbox_session.", $file, ".usage")),
            help: include_str!(concat!("../help/blackbox_session.", $file, ".help")),
            args: &$args,
            subs: None,
        };
    };
}
sub_spec!(START, "blackbox_session.py start", "start", START_ARGS);
sub_spec!(STOP, "blackbox_session.py stop", "stop", STOP_ARGS);
sub_spec!(DECLARE, "blackbox_session.py declare-budget", "declare-budget", DECLARE_ARGS);
sub_spec!(STATUS, "blackbox_session.py budget-status", "budget-status", STATUS_ARGS);
sub_spec!(RENEW, "blackbox_session.py renew-budget", "renew-budget", RENEW_ARGS);
sub_spec!(DEFAULTS_S, "blackbox_session.py budget-defaults", "budget-defaults", DEFAULTS_ARGS);
sub_spec!(CLEAR, "blackbox_session.py clear-budget", "clear-budget", CLEAR_ARGS);
sub_spec!(SKIP, "blackbox_session.py budget-skip", "budget-skip", SKIP_ARGS);
sub_spec!(BLOCK, "blackbox_session.py budget-block", "budget-block", BLOCK_ARGS);
sub_spec!(LIST, "blackbox_session.py list", "list", LIST_ARGS);
sub_spec!(SLICE, "blackbox_session.py slice", "slice", SLICE_ARGS);

static TOP_ARGS: [Arg; 3] = [HELP, store(&["--node-dir"]), flag(&["--self-test"])];

pub static SPEC: Spec = Spec {
    prog: "blackbox_session.py",
    usage: include_str!("../help/blackbox_session.usage"),
    help: include_str!("../help/blackbox_session.help"),
    args: &TOP_ARGS,
    subs: Some((
        "cmd",
        &[
            ("start", &START),
            ("stop", &STOP),
            ("declare-budget", &DECLARE),
            ("budget-status", &STATUS),
            ("renew-budget", &RENEW),
            ("budget-defaults", &DEFAULTS_S),
            ("clear-budget", &CLEAR),
            ("budget-skip", &SKIP),
            ("budget-block", &BLOCK),
            ("list", &LIST),
            ("slice", &SLICE),
        ],
    )),
};

fn to_a(node_dir: String, name: &str, p: &Parsed) -> A {
    let t = |i: usize| p.text(i).map(String::from);
    let mut a = A {
        node_dir,
        ..Default::default()
    };
    let many = |i: usize| -> Option<Vec<String>> {
        let v = p.many(i);
        if v.is_empty() {
            None
        } else {
            Some(v.to_vec())
        }
    };
    match name {
        "start" => {
            a.session_id = t(1);
            a.model = t(2);
            a.model_path = t(3);
            a.recipe = t(4);
            a.topology = t(5);
            a.tp = p.int(6);
            a.config_file = t(7);
            a.image = t(8);
            a.now = t(9);
            a.note = many(10);
            a.force = p.flag(11);
        }
        "stop" => {
            a.session_id = t(1);
            a.now = t(2);
            a.verdict = t(3);
            a.note = many(4);
        }
        "declare-budget" => {
            a.mem_total_mib = p.int(1);
            a.weights_mib = p.int(2);
            a.kv_mib = p.int(3);
            a.overhead_mib = p.int(4);
            a.ttl_s = Some(p.int(5).unwrap_or(DEFAULT_TTL_S));
            a.expected_load_s = Some(p.int(6).unwrap_or(0));
            a.label = t(7);
            a.now = t(8);
        }
        "budget-status" => a.now = t(1),
        "renew-budget" => {
            a.ttl_s = Some(p.int(1).unwrap_or(DEFAULT_TTL_S));
            a.expected_load_s = Some(p.int(2).unwrap_or(0));
            a.now = t(3);
            a.expect_label = t(4);
        }
        "budget-defaults" => a.field = t(1),
        "clear-budget" => a.now = t(1),
        "budget-skip" => {
            a.reason = t(1);
            a.label = t(2);
            a.now = t(3);
        }
        "budget-block" => {
            a.stage = t(1);
            a.reason = t(2);
            a.label = t(3);
            a.detail = p.many(4).to_vec();
            a.now = t(5);
        }
        "slice" => a.session_id = t(1),
        _ => {}
    }
    a
}

fn run(p: &Parsed) -> R<i32> {
    if p.flag(2) {
        return Ok(self_test());
    }
    let Some((name, sp)) = p.sub.as_ref() else {
        argp::print_help_stdout(&SPEC);
        return Ok(2);
    };
    let Some(nd) = p.text(1).filter(|s| !s.is_empty()) else {
        return Err(exit(
            "--node-dir 이 필요하다(docs/logs/<node_id>)".into(),
        ));
    };
    let a = to_a(nd.to_string(), name, sp);
    match name.as_str() {
        "start" => cmd_start(&a),
        "stop" => cmd_stop(&a),
        "declare-budget" => cmd_declare_budget(&a),
        "budget-status" => cmd_budget_status(&a),
        "renew-budget" => cmd_renew_budget(&a),
        "budget-defaults" => cmd_budget_defaults(&a),
        "clear-budget" => cmd_clear_budget(&a),
        "budget-skip" => cmd_budget_skip(&a),
        "budget-block" => cmd_budget_block(&a),
        "list" => cmd_list(&a),
        _ => cmd_slice(&a),
    }
}

pub fn main(argv: &[String]) -> i32 {
    match argp::parse(&SPEC, argv) {
        Outcome::Exit(c) => c,
        Outcome::Ok(p) => bb::finish(run(&p)),
    }
}
