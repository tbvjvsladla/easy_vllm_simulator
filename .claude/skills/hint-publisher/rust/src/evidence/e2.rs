//! evidence part 2 — port of `scripts/hintlib/evidence.py` lines 1913–3566: ledger timeline · launch markers · full-bench
//! definition · bench command reconstruction · auxiliary node observation · campaign boundary · same-cell launches ·
//! tool snapshots. A `CellEvidence` is a `J::Dict` (dataclass fields as keys).
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::core::{self, HintError, HR};
use crate::pyre::{self, Re};
use crate::value::{Map, J};
use crate::{jd, jl};
use super::*;

macro_rules! re_priv {
    ($name:ident, $pat:expr) => {
        re_priv!($name, $pat, 0);
    };
    ($name:ident, $pat:expr, $flags:expr) => {
        fn $name() -> &'static Re {
            static C: OnceLock<Re> = OnceLock::new();
            C.get_or_init(|| Re::new($pat, $flags))
        }
    };
}

// e1-owned module patterns (`_UTC` · `_CELL_RE`) used here — private copies of the same literals.
re_priv!(re_utc, r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z");
re_priv!(re_cell, r"^[A-Za-z0-9][A-Za-z0-9._-]*$");
re_priv!(re_sid_chars, r"[A-Za-z0-9._-]+");
re_priv!(re_sha40, r"[0-9a-f]{40}");
re_priv!(re_digits, r"\d+");
// `datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ")` (CPython `_strptime` field regexes · IGNORECASE).
re_priv!(
    re_strptime,
    r"(\d\d\d\d)-(1[0-2]|0[1-9]|[1-9])-(3[01]|[12]\d|0[1-9]|[1-9]| [1-9])T(2[0-3]|[0-1]\d|\d):([0-5]\d|\d):(6[0-1]|[0-5]\d|\d)Z",
    pyre::I
);
re_priv!(re_shlex_safe, r"[A-Za-z0-9_@%+=:,./-]+");

pub const REL_EVENTS_ROOT: &str = "docs/logs";
pub const REL_EVENTS_RECOVERED_ROOT: &str = "sync_staging/sub_docs/logs";
pub const REL_DOCS_RULES: &str = ".claude/rules/docs.md";
pub const _EVENT_LABEL_PREFIXES: [&str; 2] = ["smoke", "serve"];
pub const _EVENT_OPEN_KINDS: [&str; 2] = ["budget_declare", "budget_honored"];
pub const _EVENT_CLOSE_KINDS: [&str; 3] = ["budget_clear", "budget_none", "budget_expired"];
pub const _EVENT_WINDOWED_KINDS: [&str; 10] = ["budget_clear", "budget_none", "budget_expired", "watchdog_trip", "watchdog_kill_ack", "watchdog_highrate_hold", "thermal_trip", "thermal_kill_ack", "earlyoom_kill", "budget_rejected"];
pub const _EVENT_DETAIL_FIELDS: [&str; 28] = ["floor_mib", "weights_mib", "kv_mib", "overhead_mib", "mem_total_mib", "ttl_s", "remaining_before_s", "existed", "arm_ceiling_mib", "remaining_s", "mem_avail_mib", "rate_mib_s", "max_rate_mib_s", "legacy_rule_would_trip", "rule", "threshold_mib", "action", "target", "reason", "stage", "short_by_mib", "model", "tensor_parallel_size", "topology", "duration_s", "verdict", "soc_temp_c", "gpu_pwr_w"];
crate::re_static!(_SAFE_DETAIL_VALUE, r"[A-Za-z0-9 ._:+=-]{1,80}");
crate::re_static!(_IPV4_LIKE, r"(?<![0-9])\d{1,3}(?:\.\d{1,3}){3}(?![0-9])");
crate::re_static!(_ENGINE_TS, r"\b(?:INFO|WARNING|ERROR|DEBUG|CRITICAL)\s+(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\b");
crate::re_static!(_ANSI, r"\x1b\[[0-9;]*[A-Za-z]");

/// `_ENGINE_MARKERS` — (kind, pattern, pick "first"|"last").
pub fn _ENGINE_MARKERS() -> &'static [(&'static str, Re, &'static str)] {
    static C: OnceLock<Vec<(&'static str, Re, &'static str)>> = OnceLock::new();
    C.get_or_init(|| {
        vec![
            ("engine_prefetch_disabled", Re::new(r"Auto-prefetch is disabled because the filesystem \(([A-Za-z0-9_]+)\)", 0), "first"),
            ("engine_weights_loaded", Re::new(r"Loading weights took ([0-9.]+) seconds", 0), "last"),
            ("engine_model_loaded", Re::new(r"Model loading took ([0-9.]+) GiB memory and ([0-9.]+) seconds", 0), "last"),
            ("engine_shm_broadcast_wait", Re::new(r"No available shared memory broadcast block found in (\d+) seconds", 0), "first"),
            (
                "engine_kv_cache_sized",
                Re::new(r"GPU KV cache size: ([0-9,]+) tokens(?:, Maximum concurrency for ([0-9,]+) tokens per request: ([0-9.]+)x)?", 0),
                "first",
            ),
            ("engine_init_done", Re::new(r"init engine \(profile, create kv cache, warmup model\) took ([0-9.]+) s", 0), "first"),
            ("server_starting", Re::new(r"Starting vLLM server on ", 0), "first"),
        ]
    })
}
crate::re_static!(_FULL_DEF_RE, r"full 계측\((?P<head>[^()`]*)`(?P<inner>[^`]*)`(?P<tail>[^()`]*)\)");
crate::re_static!(_FULL_GATE_LEG_RE, r"([A-Za-z][A-Za-z0-9_-]*)\s*게이트");
crate::re_static!(_FULL_REPEATS_RE, r"반복\s*≥\s*(\d+)");

// ── small private helpers ────────────────────────────────────────────────────────────────────
type Row = (String, i64, J);

struct Stream {
    key: String,
    rows: Vec<Row>,
    srcs: Vec<String>,
    bads: Vec<(String, i64)>,
}

#[derive(Clone)]
struct Win {
    label: J,
    open_kind: String,
    start_ts: String,
    rows: Vec<usize>,
    end_ts: Option<String>,
    end_kind: String,
    stream_last: Option<String>,
    foreign: bool,
}

impl Win {
    fn to_j(&self) -> J {
        let mut m = Map::new();
        m.insert("label", self.label.clone());
        m.insert("open_kind", self.open_kind.clone());
        m.insert("start_ts", self.start_ts.clone());
        m.insert("rows", J::List(self.rows.iter().map(|i| J::Int(*i as i64)).collect()));
        m.insert("end_ts", J::from(self.end_ts.clone()));
        m.insert("end_kind", self.end_kind.clone());
        if let Some(s) = &self.stream_last {
            m.insert("stream_last", s.clone());
        }
        if self.foreign {
            m.insert("foreign", true);
        }
        J::Dict(m)
    }
    fn holds(&self, t: &str) -> bool {
        let end = self.end_ts.as_deref().or(self.stream_last.as_deref());
        self.start_ts.as_str() <= t && end.map_or(true, |e| t <= e)
    }
}

fn utc_full(s: &str) -> bool {
    re_utc().is_full(s)
}

fn kind_in(d: &J, set: &[&str]) -> bool {
    d.get("kind").and_then(J::as_str).map_or(false, |k| set.contains(&k))
}

fn str_in(v: &J, set: &[&str]) -> bool {
    v.as_str().map_or(false, |k| set.contains(&k))
}

/// `d.get(k) in (None, "")`.
fn none_or_empty(v: Option<&J>) -> bool {
    matches!(v, None | Some(J::Null)) || matches!(v, Some(J::Str(s)) if s.is_empty())
}

/// `x or None` for a dict value.
fn or_none(v: Option<&J>) -> J {
    match v {
        Some(x) if x.truthy() => x.clone(),
        _ => J::Null,
    }
}

fn disp(v: &J) -> String {
    v.display()
}

fn opt_disp(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "None".to_string())
}

fn ev_cell(ev: &J) -> String {
    ev["cell"].as_str().unwrap_or_default().to_string()
}

fn ev_node(ev: &J) -> String {
    ev["node"].as_str().unwrap_or_default().to_string()
}

fn ev_mode_is_campaign(ev: &J) -> bool {
    ev.get("mode").and_then(J::as_str) == Some("campaign")
}

fn os_err_text(e: &std::io::Error, path: &Path) -> String {
    let ty = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        std::io::ErrorKind::IsADirectory => "IsADirectoryError",
        _ => "OSError",
    };
    let ty = if e.raw_os_error() == Some(21) { "IsADirectoryError" } else { ty };
    format!("{ty}: {}", core::io_err_text(e, path))
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `path.read_bytes().decode("utf-8", errors="replace")`.
fn read_lossy(p: &Path) -> Result<String, std::io::Error> {
    std::fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// `path.read_text(encoding="utf-8", errors="replace")` (universal newlines).
fn read_text_universal(p: &Path) -> HR<String> {
    let t = read_lossy(p).map_err(|e| core::io_err(&e, p))?;
    Ok(t.replace("\r\n", "\n").replace('\r', "\n"))
}

fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}

/// `str.splitlines()`.
fn py_splitlines(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            '\n' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
                out.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Python `float(s)` for the `[0-9.]+` captures.
fn py_float(s: &str) -> HR<f64> {
    let t = py_strip(s);
    let ok = !t.is_empty() && t.chars().filter(|c| *c == '.').count() <= 1 && t.chars().any(|c| c.is_ascii_digit());
    if ok {
        if let Ok(v) = t.parse::<f64>() {
            return Ok(v);
        }
    }
    Err(HintError::py("ValueError", format!("could not convert string to float: {}", crate::value::py_repr(s))))
}

fn re_escape(s: &str) -> String {
    fancy_regex::escape(s).into_owned()
}

/// `shlex.quote`.
fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.is_ascii() && re_shlex_safe().is_full(s) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

/// Entries of `dir` whose name ends with `suffix` (pathlib `glob("*<suffix>")` · dot-files included), sorted.
fn glob_suffix(dir: &Path, prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let mut out = glob_suffix_unsorted(dir, prefix, suffix);
    out.sort();
    out
}

/// Same, in directory (readdir) order — pathlib `glob` without `sorted()`.
fn glob_suffix_unsorted(dir: &Path, prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let out: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let n = name_of(p);
                n.starts_with(prefix) && n.ends_with(suffix) && n.len() >= prefix.len() + suffix.len()
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    out
}

fn is_symlink(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

// Private copies of tiny e1/e3 helpers (same rules as the Python one-liners) so this part runs on its own:
// `_repo` = Path.resolve · `_rel` = core.rel · `_as_int` · `_norm_value` · `_read_json_opt` · `_read_json_soft` ·
// `_log_lines` · `_nonempty`.
fn repo_(repo: &Path) -> HR<PathBuf> {
    Ok(core::resolve(repo))
}

fn rel_(repo: &Path, path: &Path) -> HR<String> {
    core::rel(repo, path)
}

fn as_int(v: &J) -> HR<Option<i64>> {
    Ok(match v {
        J::Int(i) => Some(*i),
        J::Str(s) if re_digits().is_full(py_strip(s)) => py_strip(s).parse::<i64>().ok(),
        _ => None,
    })
}

fn norm_value(v: &J) -> HR<J> {
    Ok(match v {
        J::Null => J::Null,
        J::Str(s) => {
            let t = py_strip(s);
            if ["", "n/a", "na", "null", "none", "unknown", "?"].contains(&t.to_lowercase().as_str()) {
                J::Null
            } else {
                J::from(t)
            }
        }
        o => o.clone(),
    })
}

fn read_json_opt(p: &Path, what: &str) -> HR<J> {
    if !p.exists() {
        return Ok(J::Null);
    }
    let why = match std::fs::read(p) {
        Err(e) => os_err_text(&e, p),
        Ok(b) => match String::from_utf8(b) {
            Err(_) => format!("UnicodeDecodeError: {}", core::read_text(p).err().unwrap_or_default()),
            Ok(t) => match crate::value::loads(&t) {
                Ok(v) => return Ok(v),
                Err(e) => format!("JSONDecodeError: {e}"),
            },
        },
    };
    core::fail(
        "HINT_EVIDENCE_UNREADABLE",
        format!("{what} 를 읽을 수 없다({}): {why}", name_of(p)),
        Some("파일을 고치거나 지운다 — 깨진 증거를 부재로 읽지 않는다."),
    )
}

/// `_read_json_soft(path)` → (doc|None, reason|None).
fn read_soft(p: &Path) -> HR<(J, J)> {
    match read_json_opt(p, &name_of(p)) {
        Ok(d) => Ok((d, J::Null)),
        Err(e) if e.is_py() => Err(e),
        Err(e) => Ok((J::Null, J::from(format!("{} 판독 불가({})", name_of(p), e.code)))),
    }
}

fn log_lines(p: &Path) -> HR<Vec<String>> {
    match read_lossy(p) {
        Ok(t) => Ok(t.split('\n').map(str::to_string).collect()),
        Err(e) => core::fail("HINT_EVIDENCE_UNREADABLE", format!("엔진 로그를 읽을 수 없다({}): {}", name_of(p), os_err_text(&e, p)), None),
    }
}

fn nonempty(p: &Path) -> HR<bool> {
    Ok(std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false))
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// `datetime(y, mo, d, h, mi, s)` validity → epoch seconds.
fn civil_epoch(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> Option<i64> {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || !(0..=23).contains(&h)
        || !(0..=59).contains(&mi) || !(0..=59).contains(&s)
    {
        return None;
    }
    Some(core::days_from_civil(y, mo as u32, d as u32) * 86400 + h * 3600 + mi * 60 + s)
}

fn fmt_epoch(ep: i64) -> String {
    let days = ep.div_euclid(86400);
    let secs = ep.rem_euclid(86400);
    let (y, m, d) = core::civil_from_days(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn utc_epoch_of(s: &J) -> Option<i64> {
    let text = s.display();
    let c = re_strptime().fullmatch(&text)?;
    let g = |i: usize| -> i64 { py_strip(pyre::group(&c, i).unwrap_or("0")).parse::<i64>().unwrap_or(-1) };
    civil_epoch(g(1), g(2), g(3), g(4), g(5), g(6))
}

fn utc_epoch_str(s: Option<&str>) -> Option<i64> {
    utc_epoch_of(&J::from(s))
}

/// `_utc_dt` — the parsed datetime as epoch seconds (`J::Int`), or None.
pub fn _utc_dt(s: &J) -> HR<Option<J>> {
    Ok(utc_epoch_of(s).map(J::Int))
}

fn dur(a: &str, b: &str) -> Option<String> {
    let da = utc_epoch_str(Some(a))?;
    let db = utc_epoch_str(Some(b))?;
    let s = db - da;
    let (sign, s) = if s < 0 { ("-", -s) } else { ("", s) };
    let (h, r) = (s / 3600, s % 3600);
    let (m, sec) = (r / 60, r % 60);
    Some(if h != 0 { format!("{sign}{h}h{m:02}m{sec:02}s") } else { format!("{sign}{m}m{sec:02}s") })
}

fn dur_s(a: &str, b: &str) -> String {
    opt_disp(&dur(a, b))
}

/// `_dur(a, b)` — `12m34s` · `1h02m03s` (b − a) · None when either is unreadable.
pub fn _dur(a: &J, b: &J) -> HR<Option<String>> {
    let (Some(da), Some(db)) = (utc_epoch_of(a), utc_epoch_of(b)) else { return Ok(None) };
    Ok(dur(&fmt_epoch(da), &fmt_epoch(db)))
}

pub fn _label_is_cell(label: &J, cell: &str) -> HR<bool> {
    Ok(label_is_cell(label, cell))
}

fn label_is_cell(label: &J, cell: &str) -> bool {
    match label.as_str() {
        Some(l) => l == cell || _EVENT_LABEL_PREFIXES.iter().any(|p| l == format!("{p}-{cell}")),
        None => false,
    }
}

pub fn _session_is_cell(sid: &J, cell: &str) -> HR<bool> {
    Ok(session_is_cell(sid, cell))
}

fn session_is_cell(sid: &J, cell: &str) -> bool {
    let Some(s) = sid.as_str() else { return false };
    let pat = format!("(?:{})-{}(?:-\\d{{8}}T\\d{{6}}Z)?", _EVENT_LABEL_PREFIXES.join("|"), re_escape(cell));
    Re::new(&pat, 0).is_full(s)
}

/// `_node_session_docs` — {sid: doc+_source | None} (directory order, as pathlib `glob`).
pub fn _node_session_docs(repo: &Path, node: &str) -> HR<J> {
    if !re_cell().is_full(node) || node.starts_with('.') {
        return Ok(J::dict());
    }
    let mut chosen: Vec<(String, PathBuf)> = Vec::new();
    for p in glob_suffix_unsorted(&repo.join(REL_EVENTS_ROOT).join(node).join("sessions"), "", ".json") {
        chosen.push((name_of(&p), p));
    }
    if _self_node(repo)?.0.as_deref() != Some(node) {
        for p in glob_suffix_unsorted(&repo.join(REL_EVENTS_RECOVERED_ROOT).join(node).join("sessions"), "", ".json") {
            let n = name_of(&p);
            if let Some(slot) = chosen.iter_mut().find(|(k, _)| *k == n) {
                slot.1 = p;
            } else {
                chosen.push((n, p));
            }
        }
    }
    let mut out = Map::new();
    let repo_res = core::resolve(repo);
    for (_n, p) in chosen {
        let name = name_of(&p);
        let sid = name.strip_suffix(".json").unwrap_or(&name).to_string();
        out.insert(sid.clone(), J::Null);
        let anc_link = p.ancestors().filter(|q| !q.as_os_str().is_empty()).any(is_symlink);
        if !re_sid_chars().is_full(&sid) || anc_link || !p.is_file() || !core::resolve(&p).starts_with(&repo_res) {
            continue;
        }
        let (d, _bad) = read_soft(&p)?;
        if !d.is_dict() || d.get("session_id").and_then(J::as_str) != Some(sid.as_str()) {
            continue;
        }
        let start = d.get("started_utc").cloned().unwrap_or(J::Null);
        let stop = d.get("stopped_utc").cloned().unwrap_or(J::Null);
        let start_ok = matches!(&start, J::Str(s) if utc_full(s) && utc_epoch_str(Some(s)).is_some());
        let stop_bad = match &stop {
            J::Null => false,
            J::Str(s) => !utc_full(s) || utc_epoch_str(Some(s)).is_none() || s.as_str() < start.as_str().unwrap_or(""),
            _ => true,
        };
        if !start_ok || stop_bad {
            continue;
        }
        let mut m = d.as_dict().cloned().unwrap_or_default();
        m.insert("_source", rel_(repo, &p)?);
        out.insert(sid, J::Dict(m));
    }
    Ok(J::Dict(out))
}

pub fn _session_event_is_cell(d: &J, ts: &str, ev: &J, sessions: &J) -> HR<bool> {
    Ok(session_event_is_cell(d, ts, ev, sessions))
}

fn session_event_is_cell(d: &J, ts: &str, ev: &J, sessions: &J) -> bool {
    let Some(sid) = d.get("session_id").and_then(J::as_str) else { return false };
    let kind = d.get("kind").and_then(J::as_str).unwrap_or("");
    if kind != "serve_start" && kind != "serve_stop" {
        return false;
    }
    let Some(doc) = sessions.get(sid) else { return session_is_cell(&J::from(sid), &ev_cell(ev)) };
    let cn = &ev["sweep"]["index"]["meta"]["config_name"];
    let config = if cn.truthy() { cn.clone() } else { ev["cell"].clone() };
    let time_key = if kind == "serve_start" { "started_utc" } else { "stopped_utc" };
    let topo = &ev["topology"];
    let dt = d.get("topology").cloned().unwrap_or(J::Null);
    let ds = d.get("source").cloned().unwrap_or(J::Null);
    doc.truthy()
        && doc.get("config_file").cloned().unwrap_or(J::Null) == config
        && doc.get("topology").cloned().unwrap_or(J::Null) == *topo
        && (dt.is_null() || dt == *topo)
        && (ds.is_null() || ds.as_str() == Some("blackbox_session"))
        && doc.get(time_key).and_then(J::as_str) == Some(ts)
}

pub const _MEASURED_MEM_TOTAL_KINDS: [&str; 2] = ["budget_declare", "budget_declare_rejected"];
pub const _DECL_MEM_TAIL: &str = " · 단 mem_total_mib 는 러너 측정";
pub const _MEM_TOTAL_NOTE: &str = "러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)";

pub fn _event_fields(d: &J) -> HR<String> {
    let mut out: Vec<String> = Vec::new();
    let src = d.get("source").cloned().unwrap_or(J::Null);
    let measured_mem = kind_in(d, &_MEASURED_MEM_TOTAL_KINDS) && (src.is_null() || src.as_str() == Some("blackbox_session"));
    for k in _EVENT_DETAIL_FIELDS {
        let v = d.get(k).cloned().unwrap_or(J::Null);
        if k == "mem_total_mib" && measured_mem && v.is_number() {
            out.push(format!("{k}={}({_MEM_TOTAL_NOTE})", v.repr()));
        } else if let J::Bool(b) = v {
            out.push(format!("{k}={}", if b { "true" } else { "false" }));
        } else if v.is_number() {
            out.push(format!("{k}={}", v.repr()));
        } else if let J::Str(s) = &v {
            if _SAFE_DETAIL_VALUE().is_full(s) && !_IPV4_LIKE().is_search(s) {
                out.push(format!("{k}={s}"));
            }
        }
    }
    Ok(out.join(" · "))
}

pub fn _measured_key(repo: &Path, ev: &J) -> HR<Option<String>> {
    let a = norm_value(&ev["certificate"]["measured_utc"])?;
    if a.truthy() {
        return Ok(Some(a.display()));
    }
    let b = &ev["sweep"]["generated_utc"];
    if b.truthy() {
        return Ok(Some(b.display()));
    }
    _report_born_utc(repo, ev["bench_report_path"].as_str())
}

pub fn _event_node_dirs(repo: &Path, ev: &J) -> HR<Vec<String>> {
    let mut have: BTreeSet<String> = BTreeSet::new();
    for root in [REL_EVENTS_ROOT, REL_EVENTS_RECOVERED_ROOT] {
        let r = repo.join(root);
        if r.is_dir() {
            for d in sorted_dirs(&r) {
                if d.join("events").is_dir() {
                    have.insert(name_of(&d));
                }
            }
        }
    }
    if have.is_empty() {
        return Ok(Vec::new());
    }
    let nodes: Vec<&J> = ev["declaration"]["nodes"].items().iter().filter(|n| n.is_dict() && n["node_id"].truthy()).collect();
    let want: Vec<String> = if ev["node"].as_str() == Some("cluster") {
        let w: Vec<String> = nodes.iter().map(|n| n["node_id"].display()).collect();
        if w.is_empty() { have.iter().cloned().collect() } else { w }
    } else {
        let w: Vec<String> = nodes.iter().filter(|n| n.get("role").cloned().unwrap_or(J::Null) == ev["node"]).map(|n| n["node_id"].display()).collect();
        if w.is_empty() { vec![ev["node"].display()] } else { w }
    };
    let mut out: Vec<String> = Vec::new();
    for w in want {
        if !out.contains(&w) && have.contains(&w) {
            out.push(w);
        }
    }
    Ok(out)
}

pub fn _node_event_files(repo: &Path, node: &str) -> HR<(Vec<PathBuf>, J)> {
    let (self_node, _src) = _self_node(repo)?;
    let local = glob_suffix(&repo.join(REL_EVENTS_ROOT).join(node).join("events"), "", ".jsonl");
    if self_node.as_deref() == Some(node) {
        return Ok((local, jd! {"location" => format!("{REL_EVENTS_ROOT}/{node}/events(이 체크아웃 노드 · 로컬)"), "shadowed" => J::list()}));
    }
    let mirror = glob_suffix(&repo.join(REL_EVENTS_RECOVERED_ROOT).join(node).join("events"), "", ".jsonl");
    let mnames: Vec<String> = mirror.iter().map(|p| name_of(p)).collect();
    let mut chosen: Vec<(String, PathBuf)> = local.iter().map(|p| (name_of(p), p.clone())).collect();
    for p in &mirror {
        let n = name_of(p);
        if let Some(slot) = chosen.iter_mut().find(|(k, _)| *k == n) {
            slot.1 = p.clone();
        } else {
            chosen.push((n, p.clone()));
        }
    }
    let mut shadowed: Vec<String> = Vec::new();
    for p in &local {
        if mnames.contains(&name_of(p)) {
            shadowed.push(rel_(repo, p)?);
        }
    }
    shadowed.sort();
    let mut loc = if !mirror.is_empty() {
        format!("{REL_EVENTS_RECOVERED_ROOT}/{node}/events(fetch_sub_docs 회수 미러)")
    } else {
        format!("{REL_EVENTS_ROOT}/{node}/events(회수 미러 없음 — 로컬 사본)")
    };
    if !mirror.is_empty() && local.iter().any(|p| !mnames.contains(&name_of(p))) {
        loc += &format!(" + {REL_EVENTS_ROOT}/{node}/events(미러에 없는 이름만)");
    }
    chosen.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((chosen.into_iter().map(|(_, p)| p).collect(), jd! {"location" => loc, "shadowed" => J::from(shadowed)}))
}

pub fn _event_rows(path: &Path) -> HR<(Vec<(String, i64, J)>, i64)> {
    let text = match read_lossy(path) {
        Ok(t) => t,
        Err(e) => {
            return core::fail("HINT_EVIDENCE_UNREADABLE", format!("블랙박스 원장을 읽을 수 없다({}): {}", name_of(path), os_err_text(&e, path)), None)
        }
    };
    let mut rows: Vec<Row> = Vec::new();
    let mut bad = 0;
    for (i, line) in text.split('\n').enumerate() {
        let s = py_strip(line);
        if s.is_empty() {
            continue;
        }
        let Ok(d) = crate::value::loads(s) else {
            bad += 1;
            continue;
        };
        if let Some(ts) = d.get("ts").and_then(J::as_str) {
            if d.is_dict() && utc_full(ts) {
                rows.push((ts.to_string(), i as i64 + 1, d.clone()));
            }
        }
    }
    rows.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    Ok((rows, bad))
}

crate::re_static!(_MONTH_LEDGER, r"\d{4}-\d{2}\.jsonl");

fn node_streams(repo: &Path, node: &str) -> HR<Vec<Stream>> {
    let mut month: Vec<(String, usize, Row)> = Vec::new();
    let mut month_bad: Vec<(String, i64)> = Vec::new();
    let mut out: Vec<Stream> = Vec::new();
    let files = _node_event_files(repo, node)?.0;
    for (fi, f) in files.iter().enumerate() {
        let rel = rel_(repo, f)?;
        let (rows, bad) = _event_rows(f)?;
        if _MONTH_LEDGER().is_full(&name_of(f)) {
            month.extend(rows.into_iter().map(|r| (rel.clone(), fi, r)));
            month_bad.push((rel, bad));
        } else {
            let n = rows.len();
            out.push(Stream { key: rel.clone(), rows, srcs: vec![rel.clone(); n], bads: vec![(rel, bad)] });
        }
    }
    if !month_bad.is_empty() {
        month.sort_by(|a, b| (&a.2 .0, a.1, a.2 .1).cmp(&(&b.2 .0, b.1, b.2 .1)));
        let mut keys: Vec<String> = month_bad.iter().map(|(k, _)| k.clone()).collect();
        keys.sort();
        let srcs = month.iter().map(|(r, _, _)| r.clone()).collect();
        let rows = month.into_iter().map(|(_, _, r)| r).collect();
        out.insert(0, Stream { key: keys.join("+"), rows, srcs, bads: month_bad });
    }
    Ok(out)
}

fn stream_to_j(s: &Stream) -> J {
    let rows: Vec<J> = s.rows.iter().map(|(t, l, d)| jl![t.clone(), *l, d.clone()]).collect();
    let mut b = Map::new();
    for (k, v) in &s.bads {
        b.insert(k.clone(), *v);
    }
    jl![s.key.clone(), J::List(rows), J::from(s.srcs.clone()), J::Dict(b)]
}

fn rows_of_j(rows: &J) -> Vec<Row> {
    rows.items().iter().map(|r| (r.at(0).as_str().unwrap_or_default().to_string(), r.at(1).as_i64().unwrap_or(0), r.at(2).clone())).collect()
}

fn stream_of_j(s: &J) -> Stream {
    let bads = s.at(3).as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.as_i64().unwrap_or(0))).collect()).unwrap_or_default();
    Stream {
        key: s.at(0).as_str().unwrap_or_default().to_string(),
        rows: rows_of_j(s.at(1)),
        srcs: s.at(2).items().iter().map(|x| x.display()).collect(),
        bads,
    }
}

/// `_node_streams` — [(stream key, rows (ts order), per-row source file, {file: unparsable lines})].
pub fn _node_streams(repo: &Path, node: &str) -> HR<Vec<(String, Vec<(String, i64, J)>, Vec<String>, J)>> {
    Ok(node_streams(repo, node)?
        .into_iter()
        .map(|s| {
            let mut b = Map::new();
            for (k, v) in &s.bads {
                b.insert(k.clone(), *v);
            }
            (s.key, s.rows, s.srcs, J::Dict(b))
        })
        .collect())
}

fn label_key(l: &J) -> (u8, String) {
    match l {
        J::Null => (0, String::new()),
        J::Str(s) => (1, s.clone()),
        o => (2, o.repr()),
    }
}

fn node_opens(streams: &[Stream]) -> Vec<(String, J)> {
    let mut v: Vec<(String, J)> = streams
        .iter()
        .flat_map(|s| s.rows.iter())
        .filter(|(_, _, d)| kind_in(d, &_EVENT_OPEN_KINDS))
        .map(|(ts, _, d)| (ts.clone(), d.get("label").cloned().unwrap_or(J::Null)))
        .collect();
    v.sort_by(|a, b| (&a.0, label_key(&a.1)).cmp(&(&b.0, label_key(&b.1))));
    v
}

/// `_node_opens(streams)` — streams as J `[[key, rows, srcs, bads], ...]`.
pub fn _node_opens(streams: &J) -> HR<Vec<(String, Option<String>)>> {
    let ss: Vec<Stream> = streams.items().iter().map(stream_of_j).collect();
    Ok(node_opens(&ss).into_iter().map(|(t, l)| (t, if l.is_null() { None } else { Some(l.display()) })).collect())
}

fn event_windows(rows: &[Row], opens: Option<&[(String, J)]>) -> Vec<Win> {
    let mut wins: Vec<Win> = Vec::new();
    let mut cur: Option<Win> = None;
    for (idx, (ts, _ln, d)) in rows.iter().enumerate() {
        let k = d.get("kind").and_then(J::as_str).unwrap_or("");
        let is_k = d.get("kind").map_or(false, J::is_str);
        if is_k && _EVENT_OPEN_KINDS.contains(&k) {
            if let Some(mut c) = cur.take() {
                c.end_ts = Some(ts.clone());
                c.end_kind = format!("replaced({k})");
                wins.push(c);
            }
            cur = Some(Win {
                label: d.get("label").cloned().unwrap_or(J::Null),
                open_kind: k.to_string(),
                start_ts: ts.clone(),
                rows: vec![idx],
                end_ts: None,
                end_kind: "open".to_string(),
                stream_last: None,
                foreign: false,
            });
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };
        let lab = d.get("label");
        let lab_v = lab.cloned().unwrap_or(J::Null);
        if is_k && _EVENT_CLOSE_KINDS.contains(&k) && (none_or_empty(lab) || lab_v == c.label) {
            c.rows.push(idx);
            c.end_ts = Some(ts.clone());
            c.end_kind = format!("{k}{}", if d.get("existed") == Some(&J::Bool(false)) { "(existed=false)" } else { "" });
            wins.push(cur.take().unwrap());
        } else if none_or_empty(lab) && is_k && _EVENT_WINDOWED_KINDS.contains(&k) {
            c.rows.push(idx);
        } else if lab_v == c.label {
            c.rows.push(idx);
        }
    }
    if let Some(c) = cur {
        wins.push(c);
    }
    if let Some(opens) = opens.filter(|o| !o.is_empty()) {
        for w in wins.iter_mut() {
            let nxt = opens.iter().find(|(ts, lab)| ts.as_str() > w.start_ts.as_str() && *lab != w.label).map(|(t, _)| t.clone());
            let Some(nxt) = nxt else { continue };
            let first = w.rows[0];
            w.rows.retain(|&i| i == first || rows[i].0 < nxt);
            if let Some(e) = &w.end_ts {
                if nxt.as_str() < e.as_str() {
                    w.end_ts = Some(nxt);
                    w.end_kind = "replaced(다른 스트림의 다른 라벨 선언 · 노드당 슬롯 1)".to_string();
                }
            }
        }
    }
    wins
}

/// `_event_windows(rows, node_opens=None)` — rows `[[ts, line, row], ...]` · node_opens `[[ts, label], ...]` or None.
pub fn _event_windows(rows: &J, node_opens: &J) -> HR<Vec<J>> {
    let rs = rows_of_j(rows);
    let opens: Vec<(String, J)> = node_opens.items().iter().map(|o| (o.at(0).display(), o.at(1).clone())).collect();
    Ok(event_windows(&rs, if node_opens.is_null() { None } else { Some(&opens) }).iter().map(Win::to_j).collect())
}

type Taken = HashMap<(String, usize), Option<usize>>;

/// `_ledger_timeline(repo, ev, ctx=None)` — the stub form (a `ctx` passed here is not filled; use `_ledger_timeline_mut`).
pub fn _ledger_timeline(repo: &Path, ev: &J, ctx: &J) -> HR<(Vec<J>, Vec<String>, Option<J>)> {
    if ctx.is_dict() {
        let mut m = ctx.as_dict().cloned().unwrap_or_default();
        return _ledger_timeline_mut(repo, ev, Some(&mut m));
    }
    _ledger_timeline_mut(repo, ev, None)
}

fn ctx_list<'a>(ctx: &'a mut Map, k: &str) -> &'a mut Vec<J> {
    let v = ctx.setdefault(k, J::list());
    if !v.is_list() {
        *v = J::list();
    }
    v.as_list_mut().unwrap()
}

/// `_ledger_timeline` with the in-place filled `ctx` (nodes · spans · windows · budget_rows).
pub fn _ledger_timeline_mut(repo: &Path, ev: &J, mut ctx: Option<&mut Map>) -> HR<(Vec<J>, Vec<String>, Option<J>)> {
    let cell = ev_cell(ev);
    let measured = _measured_key(repo, ev)?;
    let mut out: Vec<J> = Vec::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut measured_win: Option<J> = None;
    for node in _event_node_dirs(repo, ev)? {
        let streams = node_streams(repo, &node)?;
        let sessions = _node_session_docs(repo, &node)?;
        let opens = node_opens(&streams);
        if let Some(ctx) = ctx.as_deref_mut() {
            ctx_list(ctx, "nodes").push(J::from(node.clone()));
            for s in &streams {
                if !s.rows.is_empty() {
                    let (f0, l0) = (s.rows[0].0.clone(), s.rows[s.rows.len() - 1].0.clone());
                    let spans = ctx.setdefault("spans", J::dict());
                    let sp = spans.as_dict_mut().unwrap().setdefault(&node, jl![f0.clone(), l0.clone()]);
                    let a = sp.at(0).as_str().unwrap_or_default().to_string();
                    let b = sp.at(1).as_str().unwrap_or_default().to_string();
                    *sp = jl![if f0 < a { f0 } else { a }, if l0 > b { l0 } else { b }];
                }
                for w in event_windows(&s.rows, Some(&opens)) {
                    let i0 = w.rows[0];
                    let row = jd! {"node" => node.clone(), "rel" => s.srcs[i0].clone(), "label" => w.label.clone(),
                        "start_ts" => w.start_ts.clone(), "end_ts" => J::from(w.end_ts.clone()), "line" => s.rows[i0].1,
                        "stream_last" => s.rows[s.rows.len() - 1].0.clone()};
                    ctx_list(ctx, "windows").push(row);
                }
                for ((ts, ln, d), rel) in s.rows.iter().zip(s.srcs.iter()) {
                    if kind_in(d, &_EVENT_OPEN_KINDS) || kind_in(d, &_EVENT_CLOSE_KINDS) || kind_in(d, &["budget_renew"]) {
                        let row = jd! {"node" => node.clone(), "utc" => ts.clone(), "kind" => d.get("kind").cloned().unwrap_or(J::Null),
                            "label" => or_none(d.get("label")), "rel" => rel.clone(), "line" => *ln};
                        ctx_list(ctx, "budget_rows").push(row);
                    }
                }
            }
        }
        let mut arena: Vec<Win> = Vec::new();
        let mut taken: Taken = HashMap::new();
        let mut declare_wins: Vec<(String, usize)> = Vec::new();
        for s in &streams {
            let mut in_win: HashSet<usize> = HashSet::new();
            for w in event_windows(&s.rows, Some(&opens)) {
                arena.push(w);
                let wi = arena.len() - 1;
                if !label_is_cell(&arena[wi].label, &cell) {
                    continue;
                }
                for idx in arena[wi].rows.clone() {
                    let (ts, _ln, d) = &s.rows[idx];
                    if kind_in(d, &["serve_start", "serve_stop"]) && !session_event_is_cell(d, ts, ev, &sessions) {
                        continue;
                    }
                    in_win.insert(idx);
                    taken.insert((s.key.clone(), idx), Some(wi));
                }
                if arena[wi].open_kind == "budget_declare" {
                    arena[wi].stream_last = Some(s.rows[s.rows.len() - 1].0.clone());
                    declare_wins.push((s.key.clone(), wi));
                }
            }
            for (idx, (ts, _ln, d)) in s.rows.iter().enumerate() {
                if in_win.contains(&idx) {
                    continue;
                }
                let m = if kind_in(d, &["serve_start", "serve_stop"]) {
                    session_event_is_cell(d, ts, ev, &sessions)
                } else {
                    label_is_cell(&d.get("label").cloned().unwrap_or(J::Null), &cell)
                };
                if m {
                    taken.insert((s.key.clone(), idx), None);
                }
            }
        }
        let foreign_wins = foreign_label_windows(&streams, &mut taken, &cell, &mut arena);
        for s in &streams {
            if s.rows.iter().any(|(_, _, d)| kind_in(d, &_EVENT_OPEN_KINDS)) {
                continue;
            }
            for (idx, (ts, _ln, d)) in s.rows.iter().enumerate() {
                if d.get("label").map_or(false, J::truthy) || !kind_in(d, &_EVENT_WINDOWED_KINDS) {
                    continue;
                }
                let cands: Vec<usize> = declare_wins.iter().map(|(_, w)| *w).chain(foreign_wins.iter().copied()).collect();
                let w = cands.into_iter().find(|&wi| {
                    let w = &arena[wi];
                    w.start_ts.as_str() <= ts.as_str() && w.end_ts.as_ref().map_or(true, |e| ts.as_str() <= e.as_str())
                });
                if let Some(wi) = w {
                    taken.insert((s.key.clone(), idx), Some(wi));
                }
            }
        }
        declare_wins.sort_by(|a, b| (&arena[a.1].start_ts, &a.0).cmp(&(&arena[b.1].start_ts, &b.0)));
        let order: HashMap<usize, usize> = declare_wins.iter().enumerate().map(|(i, (_, w))| (*w, i + 1)).collect();
        let hold: Vec<usize> = declare_wins
            .iter()
            .map(|(_, w)| *w)
            .filter(|&w| measured.as_deref().map_or(false, |m| !m.is_empty() && arena[w].holds(m)))
            .collect();
        if measured_win.is_none() {
            if let Some(&w) = hold.last() {
                let w = &arena[w];
                measured_win = Some(jd! {"start_ts" => w.start_ts.clone(), "end_ts" => J::from(w.end_ts.clone()), "node" => node.clone()});
            }
        }
        let node_start = out.len();
        for s in &streams {
            let mut mine: Vec<usize> = taken.keys().filter(|(r, _)| *r == s.key).map(|(_, i)| *i).collect();
            mine.sort();
            for idx in mine {
                let (ts, ln, d) = &s.rows[idx];
                let rel = &s.srcs[idx];
                used.insert(rel.clone());
                let w = taken[&(s.key.clone(), idx)];
                let k = d.get("kind").cloned().unwrap_or(J::Null).display();
                let fields = _event_fields(d)?;
                let label = or_none(d.get("label"));
                let mem_tail = if fields.contains(_MEM_TOTAL_NOTE) { _DECL_MEM_TAIL } else { "" };
                if let Some(wi) = w.filter(|&wi| arena[wi].foreign) {
                    let w = &arena[wi];
                    let note = foreign_label_note(&w.label, &cell)?;
                    let detail = if k == "budget_declare" {
                        let renews = w.rows.iter().filter(|&&j| kind_in(&s.rows[j].2, &["budget_renew"])).count();
                        let closed = closed_text(w);
                        let rel_m = match &measured {
                            None => "측정 시각 미관측".to_string(),
                            Some(m) if w.holds(m) => {
                                format!("measured_utc {m} 가 이 창 시간 안 — 라벨 불일치만으로 이 측정·기동의 귀속 여부를 판단하지 않는다")
                            }
                            Some(m) => format!(
                                "measured_utc {m} 는 이 창 밖({}",
                                if w.start_ts.as_str() > m.as_str() { "측정 뒤)" } else { "측정 전)" }
                            ),
                        };
                        format!("예산 선언(선언값 — 측정 아님{mem_tail}) · {fields} · {note} · budget_renew {renews}회 · {closed} · {rel_m}")
                    } else if none_or_empty(d.get("label")) {
                        format!(
                            "{k} · {fields} · 라벨 없음 → {} 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 {} 후 {}) · {note}",
                            disp(&w.label),
                            w.start_ts,
                            dur_s(&w.start_ts, ts)
                        )
                    } else {
                        format!("{k} · {fields} · 창 시작 {} 후 {} · {note}", w.start_ts, dur_s(&w.start_ts, ts))
                    };
                    out.push(jd! {"utc" => ts.clone(), "node" => node.clone(), "kind" => k, "label" => label, "detail" => detail,
                        "source" => format!("{rel}:{ln}"), "label_scope" => LABEL_SCOPE_OTHER, "window_label" => w.label.clone()});
                    continue;
                }
                let detail = match w {
                    Some(wi) if k == "budget_declare" && order.contains_key(&wi) => {
                        let w = &arena[wi];
                        let renews = w.rows.iter().filter(|&&j| kind_in(&s.rows[j].2, &["budget_renew"])).count();
                        let rel_m = match &measured {
                            None => "측정 시각 미관측".to_string(),
                            Some(m)
                                if measured_win.as_ref().map_or(false, |mw| {
                                    mw["node"].as_str() == Some(node.as_str()) && mw["start_ts"].as_str() == Some(w.start_ts.as_str())
                                }) =>
                            {
                                format!("measured_utc {m} 가 이 창 안(이 측정의 기동)")
                            }
                            Some(m) if w.end_ts.is_none() && w.start_ts.as_str() <= m.as_str() => format!(
                                "measured_utc {m} 는 이 창의 관측 범위 밖(창이 열린 채 스트림이 {} 에서 끝난다 — 이 측정의 기동으로 귀속하지 않는다)",
                                opt_disp(&w.stream_last)
                            ),
                            Some(m) => format!(
                                "measured_utc {m} 는 이 창 밖(다른 기동 · {}",
                                if w.start_ts.as_str() > m.as_str() { "측정 뒤 — 이 측정과 무관할 수 있다)" } else { "측정 전)" }
                            ),
                        };
                        let closed = closed_text(w);
                        format!(
                            "예산 선언(선언값 — 측정 아님{mem_tail}) · {fields} · 기동 창 {}/{}: budget_renew {renews}회 · {closed} · {rel_m}",
                            order[&wi],
                            declare_wins.len()
                        )
                    }
                    Some(wi) if none_or_empty(d.get("label")) => {
                        let w = &arena[wi];
                        format!(
                            "{k} · {fields} · 라벨 없음 → {} 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 {} 후 {})",
                            disp(&w.label),
                            w.start_ts,
                            dur_s(&w.start_ts, ts)
                        )
                    }
                    Some(wi) => {
                        let w = &arena[wi];
                        format!("{k} · {fields} · 창 시작 {} 후 {}", w.start_ts, dur_s(&w.start_ts, ts))
                    }
                    None => {
                        let sid = d.get("session_id").cloned().unwrap_or(J::Null);
                        format!("{k} · {fields}") + &if sid.truthy() { format!(" · session={}", sid.display()) } else { String::new() }
                    }
                };
                let mut entry = jd! {"utc" => ts.clone(), "node" => node.clone(), "kind" => k, "label" => label, "detail" => detail,
                    "source" => format!("{rel}:{ln}")};
                let sid = d.get("session_id").cloned().unwrap_or(J::Null);
                if session_event_is_cell(d, ts, ev, &sessions) && !session_is_cell(&sid, &cell) {
                    if let Some(doc) = sid.as_str().and_then(|s| sessions.get(s)).filter(|x| x.truthy()) {
                        entry.set("session_binding_source", doc["_source"].clone());
                    }
                }
                out.push(entry);
            }
            let mut bads = s.bads.clone();
            bads.sort();
            for (brel, bad) in bads {
                if bad != 0 && used.contains(&brel) {
                    out.push(jd! {"utc" => J::Null, "node" => node.clone(), "kind" => "ledger_unparsable_lines", "label" => J::Null,
                        "detail" => format!("JSON 파싱 불가 {bad}줄 — 건너뜀(부재가 아니라 판독 불가 · 그 줄의 사건은 이 표에 없다)"),
                        "source" => brel});
                }
            }
        }
        let tail = out.split_off(node_start);
        out.extend(merge_stream_copies(tail)?);
    }
    Ok((out, used.into_iter().collect(), measured_win))
}

fn closed_text(w: &Win) -> String {
    match &w.end_ts {
        Some(e) if !e.is_empty() => format!("닫힘 {} {e}(선언 후 {})", w.end_kind, dur_s(&w.start_ts, e)),
        _ => "닫힘 미관측(창이 열린 채 원장이 끝난다)".to_string(),
    }
}

pub const LABEL_SCOPE_OTHER: &str = "other-label-same-node";

pub fn foreign_label_note(label: &J, cell: &str) -> HR<String> {
    Ok(format!(
        "라벨 다름: `{}` — 같은 노드 · 이 셀 라벨(`{cell}`) 아님 · 창 밖에 남은 이 셀 행과 시간이 겹치는 선언 창이라 싣는다(셀 라벨 일치 집계에서 제외 · 이 라벨 분류는 세션·호출 기록의 귀속 확인과 별도)",
        label.display()
    ))
}

fn foreign_label_windows(streams: &[Stream], taken: &mut Taken, cell: &str, arena: &mut Vec<Win>) -> Vec<usize> {
    let mut sess: Vec<(String, Vec<String>)> = Vec::new();
    let mut ivs: Vec<(String, String)> = Vec::new();
    for ((skey, idx), w) in taken.iter() {
        if w.is_some() {
            continue;
        }
        let Some(s) = streams.iter().find(|s| s.key == *skey) else { continue };
        let (ts, _ln, d) = &s.rows[*idx];
        let sid = if kind_in(d, &["serve_start", "serve_stop"]) { d.get("session_id").and_then(J::as_str) } else { None };
        match sid.filter(|x| !x.is_empty()) {
            Some(sid) => match sess.iter_mut().find(|(k, _)| k == sid) {
                Some(e) => e.1.push(ts.clone()),
                None => sess.push((sid.to_string(), vec![ts.clone()])),
            },
            None => ivs.push((ts.clone(), ts.clone())),
        }
    }
    for (_, v) in &sess {
        ivs.push((v.iter().min().unwrap().clone(), v.iter().max().unwrap().clone()));
    }
    if ivs.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let opens = node_opens(streams);
    for s in streams {
        for mut w in event_windows(&s.rows, Some(&opens)) {
            if !w.label.truthy() || label_is_cell(&w.label, cell) {
                continue;
            }
            w.stream_last = Some(s.rows[s.rows.len() - 1].0.clone());
            let end = w.end_ts.clone().filter(|e| !e.is_empty()).or(w.stream_last.clone()).unwrap();
            if !ivs.iter().any(|(a, b)| w.start_ts.as_str() <= b.as_str() && end.as_str() >= a.as_str()) {
                continue;
            }
            w.foreign = true;
            let is_decl = w.open_kind == "budget_declare";
            let rows = w.rows.clone();
            arena.push(w);
            let wi = arena.len() - 1;
            for idx in rows {
                taken.entry((s.key.clone(), idx)).or_insert(Some(wi));
            }
            if is_decl {
                out.push(wi);
            }
        }
    }
    out
}

/// `_foreign_label_windows(streams, taken, cell)` — J form: streams `[[key, rows, srcs, bads]]` · taken
/// `[[[skey, idx], window|None], ...]` (returned windows only; the in-place `taken` update is not visible through J).
pub fn _foreign_label_windows(streams: &J, taken: &J, cell: &str) -> HR<Vec<J>> {
    let ss: Vec<Stream> = streams.items().iter().map(stream_of_j).collect();
    let mut arena: Vec<Win> = Vec::new();
    let mut t: Taken = HashMap::new();
    for e in taken.items() {
        let k = (e.at(0).at(0).display(), e.at(0).at(1).as_i64().unwrap_or(0) as usize);
        let v = if e.at(1).is_null() {
            None
        } else {
            arena.push(Win {
                label: e.at(1)["label"].clone(),
                open_kind: e.at(1)["open_kind"].display(),
                start_ts: e.at(1)["start_ts"].display(),
                rows: vec![],
                end_ts: e.at(1)["end_ts"].as_str().map(str::to_string),
                end_kind: e.at(1)["end_kind"].display(),
                stream_last: None,
                foreign: false,
            });
            Some(arena.len() - 1)
        };
        t.insert(k, v);
    }
    let out = foreign_label_windows(&ss, &mut t, cell, &mut arena);
    Ok(out.into_iter().map(|i| arena[i].to_j()).collect())
}

pub fn _win_holds(w: &J, t: &str) -> HR<bool> {
    let end = if w["end_ts"].is_null() { w.get("stream_last").cloned().unwrap_or(J::Null) } else { w["end_ts"].clone() };
    let st = w["start_ts"].display();
    Ok(st.as_str() <= t && (end.is_null() || t <= end.display().as_str()))
}

fn tl_key(r: &J) -> (bool, String, String, String, i64) {
    let src = r.get("source").filter(|x| x.truthy()).map(J::display).unwrap_or_default();
    let (f, ln) = match src.rfind(':') {
        Some(i) => (src[..i].to_string(), src[i + 1..].to_string()),
        None => (String::new(), src.clone()),
    };
    let utc = r.get("utc").cloned().unwrap_or(J::Null);
    let node = r.get("node").filter(|x| x.truthy()).map(J::display).unwrap_or_default();
    let lnv = if re_digits().is_full(py_strip(&ln)) { py_strip(&ln).parse::<i64>().unwrap_or(0) } else { 0 };
    (utc.is_null(), if utc.truthy() { utc.display() } else { String::new() }, node, if f.is_empty() { src.clone() } else { f }, lnv)
}

fn merge_stream_copies(rows: Vec<J>) -> HR<Vec<J>> {
    let mut sorted = rows;
    sorted.sort_by(|a, b| tl_key(a).cmp(&tl_key(b)));
    let mut first: HashMap<(String, String), usize> = HashMap::new();
    let mut kept: Vec<J> = Vec::new();
    let file_of = |s: &str| -> String { s.rfind(':').map(|i| s[..i].to_string()).unwrap_or_default() };
    for r in sorted {
        if r.get("utc").map_or(true, J::is_null) {
            kept.push(r);
            continue;
        }
        let key = (r["utc"].display(), r.get("kind").cloned().unwrap_or(J::Null).display());
        if let Some(&fi) = first.get(&key) {
            let f0 = &mut kept[fi];
            if file_of(&f0["source"].display()) != file_of(&r["source"].display()) {
                let d = format!("{} · 같은 사건의 다른 스트림 사본 {}(한 번으로 싣는다)", f0["detail"].display(), r["source"].display());
                f0.set("detail", d);
                continue;
            }
        } else {
            first.insert(key, kept.len());
        }
        kept.push(r);
    }
    Ok(kept)
}

pub fn _merge_stream_copies(rows: &J) -> HR<Vec<J>> {
    merge_stream_copies(rows.items().to_vec())
}

pub fn _startup_markers(repo: &Path, ev: &J, win: &J) -> HR<Vec<J>> {
    let sw = &ev["sweep"];
    if sw["binding"].as_str() != Some("output") || win.is_null() {
        return Ok(Vec::new());
    }
    let cell = ev_cell(ev);
    let base = repo.join(sw["dir"].display());
    let vp = as_int(&sw["index"]["verdict_point_level"])?.filter(|v| *v != 0).unwrap_or(1);
    let cands = [base.join(format!("lite_engine_{cell}.log")), base.join(format!("level_{vp:02}")).join(format!("engine_{cell}.log"))];
    let Some(log) = cands.into_iter().find(|p| p.is_file()) else { return Ok(Vec::new()) };
    let rel = rel_(repo, &log)?;
    let text = match read_lossy(&log) {
        Ok(t) => t,
        Err(e) => return core::fail("HINT_EVIDENCE_UNREADABLE", format!("엔진 로그를 읽을 수 없다({}): {}", name_of(&log), os_err_text(&e, &log)), None),
    };
    let ws = win["start_ts"].display();
    let Some(start_ep) = utc_epoch_str(Some(&ws)) else {
        return Err(HintError::py("AttributeError", "'NoneType' object has no attribute 'year'"));
    };
    let (sy, sm, sd) = core::civil_from_days(start_ep.div_euclid(86400));
    let end: Option<String> = match win.get("end_ts").filter(|x| x.truthy()) {
        Some(e) => Some(e.display()),
        None => _measured_key(repo, ev)?,
    };
    let to_utc = |c: &pyre::Captures| -> Option<String> {
        let g = |i: usize| pyre::group(c, i).unwrap_or("0").parse::<i64>().unwrap_or(0);
        let (mo, d) = (g(1), g(2));
        let year = sy + if (mo, d) < (sm as i64, sd as i64) { 1 } else { 0 };
        civil_epoch(year, mo, d, g(3), g(4), g(5)).map(fmt_epoch)
    };
    type Hit = (usize, String, Vec<Option<String>>);
    let mut first: Option<(usize, String)> = None;
    let markers = _ENGINE_MARKERS();
    let mut hits: Vec<Vec<Hit>> = vec![Vec::new(); markers.len()];
    for (i, line) in text.split('\n').enumerate() {
        let i = i + 1;
        for seg in line.split('\r') {
            let clean = _ANSI().sub_literal(seg, "");
            let Some(tm) = _ENGINE_TS().search(&clean) else { continue };
            let Some(utc) = to_utc(&tm) else { continue };
            if first.is_none() {
                first = Some((i, utc.clone()));
            }
            for (mi, (_k, rx, _p)) in markers.iter().enumerate() {
                if let Some(m) = rx.search(&clean) {
                    let groups = (0..m.len()).map(|g| pyre::group(&m, g).map(str::to_string)).collect();
                    hits[mi].push((i, utc.clone(), groups));
                }
            }
        }
    }
    let Some(first) = first else { return Ok(Vec::new()) };
    let mut stamps: Vec<&String> = vec![&first.1];
    for v in &hits {
        stamps.extend(v.iter().map(|h| &h.1));
    }
    let mn = (*stamps.iter().min().unwrap()).clone();
    let mx = (*stamps.iter().max().unwrap()).clone();
    let node = ev["node"].clone();
    let end_s = opt_disp(&end);
    if mn < ws || end.as_ref().map_or(false, |e| !e.is_empty() && mx.as_str() > e.as_str()) {
        return Ok(vec![jd! {"utc" => J::Null, "node" => node, "kind" => "engine_log_clock_unverified", "label" => J::Null,
            "detail" => format!("엔진 로그 시각(컨테이너 시계 · UTC 가정)이 측정 창 [{ws}, {end_s}] 밖({mn}~{mx}) — 시계 정합을 관측하지 못해 기동 표지를 싣지 않는다"),
            "source" => rel}]);
    }
    let mut rows = vec![jd! {"utc" => first.1.clone(), "node" => node.clone(), "kind" => "engine_log_window_start", "label" => J::Null,
        "detail" => format!("캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 {ws} 후 {} · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [{ws}, {end_s}] 안에 드는 것을 확인", dur_s(&ws, &first.1)),
        "source" => format!("{rel}:{}", first.0)}];
    for (mi, (kind, _rx, pick)) in markers.iter().enumerate() {
        let v = &hits[mi];
        if v.is_empty() {
            continue;
        }
        let (i, utc, m) = if *pick == "first" { &v[0] } else { &v[v.len() - 1] };
        let n = v.len();
        let g = |k: usize| m.get(k).cloned().flatten();
        let gs = |k: usize| opt_disp(&g(k));
        let detail = match *kind {
            "engine_prefetch_disabled" => format!("가중치 auto-prefetch 꺼짐 줄 ×{n} — filesystem {}(vLLM 로그 문구)", gs(1)),
            "engine_weights_loaded" => {
                let mut best: Option<f64> = None;
                for h in v {
                    let x = py_float(&h.2.get(1).cloned().flatten().unwrap_or_default())?;
                    best = Some(match best { Some(b) if !(x > b) => b, _ => x });
                }
                let best = best.unwrap_or(0.0);
                format!("가중치 적재 완료 줄 ×{n}(rank·단계별) · 최장 {best:.2}s · 이 행 = 마지막 줄")
            }
            "engine_model_loaded" => format!("모델 적재 완료 줄 ×{n}(rank 별) · 마지막 줄 {} GiB · {:.1}s", gs(1), py_float(&gs(2))?),
            "engine_shm_broadcast_wait" => format!(
                "shm_broadcast 대기 경고({}s 단위) ×{n} · {}–{}({}) — vLLM 문구상 프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현",
                gs(1),
                v[0].1,
                v[n - 1].1,
                dur_s(&v[0].1, &v[n - 1].1)
            ),
            "engine_kv_cache_sized" => {
                format!("GPU KV cache {} tokens", gs(1))
                    + &match g(2).filter(|x| !x.is_empty()) {
                        Some(c) => format!(" · 요청당 {c} tokens 기준 최대 동시성 {}x", gs(3)),
                        None => String::new(),
                    }
            }
            "engine_init_done" => format!("엔진 초기화(profile·KV 할당·warmup) {}s", gs(1)),
            _ => format!("API 서버 기동 — 이 측정 창 선언 {ws} 후 {} · 캡처 로그 첫 시각 {} 후 {}", dur_s(&ws, utc), first.1, dur_s(&first.1, utc)),
        };
        rows.push(jd! {"utc" => utc.clone(), "node" => node.clone(), "kind" => *kind, "label" => J::Null, "detail" => detail,
            "source" => format!("{rel}:{i}")});
    }
    Ok(rows)
}

/// `_timeline_key(r)` → `[utc is None, utc or "", node, file, line]`.
pub fn _timeline_key(r: &J) -> HR<J> {
    let (a, b, c, d, e) = tl_key(r);
    Ok(jl![a, b, c, d, e])
}

pub fn event_timeline(repo: &Path, ev: &J) -> HR<Vec<J>> {
    let repo = repo_(repo)?;
    Ok(_timeline_parts(&repo, ev)?.0)
}

/// `_NODE_IDENTITY_CANDIDATES` — owner resolver, then the sub runtime delivery.
pub fn _NODE_IDENTITY_CANDIDATES() -> Vec<PathBuf> {
    let skill = core::skill_dir();
    let skills = skill.parent().map(Path::to_path_buf).unwrap_or_default();
    let dotclaude = skills.parent().map(Path::to_path_buf).unwrap_or_default();
    vec![
        skills.join("terraforming_node").join("scripts").join("node_blackbox").join("node_identity.sh"),
        dotclaude.join("runtime").join("node_blackbox").join("node_identity.sh"),
    ]
}

type SelfNode = (Option<String>, String);

fn self_node_cache() -> &'static Mutex<Vec<(String, SelfNode)>> {
    static C: OnceLock<Mutex<Vec<(String, SelfNode)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Vec::new()))
}

/// `_SELF_NODE_CACHE[key] = val` (`val` None = `pop`) — returns the previous entry (selftest hook).
pub fn _self_node_cache_put(key: &str, val: Option<(Option<String>, String)>) -> Option<(Option<String>, String)> {
    let mut c = self_node_cache().lock().unwrap();
    let prev = c.iter().position(|(k, _)| k == key).map(|i| c.remove(i).1);
    if let Some(v) = val {
        c.push((key.to_string(), v));
    }
    prev
}

pub fn _self_node(repo: &Path) -> HR<(Option<String>, String)> {
    let key = core::posix(&core::resolve(repo));
    if let Some((_, v)) = self_node_cache().lock().unwrap().iter().find(|(k, _)| *k == key) {
        return Ok(v.clone());
    }
    let resolver = _NODE_IDENTITY_CANDIDATES().into_iter().find(|c| c.is_file());
    let res: SelfNode = match resolver {
        None => (None, "node_identity.sh 부재(terraforming_node/scripts/node_blackbox · .claude/runtime/node_blackbox)".to_string()),
        Some(r) => {
            let argv = vec!["bash".to_string(), core::posix(&r), "--resolve".into(), "--repo".into(), key.clone()];
            match core::run_capture(&argv, None, &[], &[], None) {
                Err(e) => (None, format!("node_identity.sh 실행 실패: {e}")),
                Ok(p) => {
                    let out = String::from_utf8_lossy(&p.stdout).into_owned();
                    let node = py_strip(&out).to_string();
                    if p.returncode == 0 && !node.is_empty() && !node.contains('/') && !node.starts_with('.') {
                        (Some(node), "node_identity.sh --resolve(self_role → 유일한 role: main)".to_string())
                    } else {
                        let err = String::from_utf8_lossy(&p.stderr).into_owned();
                        let why: Vec<String> = py_splitlines(&err).iter().take(2).map(|l| py_strip(l).to_string()).filter(|l| !l.is_empty()).collect();
                        let why = why.join(" / ");
                        (None, format!("node_identity.sh --resolve rc={}", p.returncode) + &if why.is_empty() { String::new() } else { format!(" — {why}") })
                    }
                }
            }
        }
    };
    self_node_cache().lock().unwrap().push((key, res.clone()));
    Ok(res)
}

crate::re_static!(_CAMPAIGN_NONE_NOTE, r"^활성 캠페인 없음\((?P<id>_bootstrap)\)$");

pub fn measurement_campaign(ev: &J) -> HR<J> {
    let empty = J::dict();
    let sw = if ev["sweep"].is_dict() { &ev["sweep"] } else { &empty };
    let idx = if sw["index"].is_dict() { &sw["index"] } else { &empty };
    let bk = &idx["bench_knobs"];
    let where_ = format!("{}/sweep_index.json bench_knobs", sw.get("dir").cloned().unwrap_or(J::Null).display());
    if !bk.is_dict() {
        let src = if sw.truthy() { "미관측(스윕 색인에 bench_knobs 없음 — 측정 시점 캠페인 해소 기록 부재)" } else { "미관측(스윕 색인 없음)" };
        return Ok(jd! {"status" => "unobserved", "id" => J::Null, "source" => src});
    }
    let camp = bk.get("campaign").cloned().unwrap_or(J::Null);
    if let J::Str(c) = &camp {
        if !py_strip(c).is_empty() && !RESERVED_CAMPAIGN_IDS.contains(&c.as_str()) {
            return Ok(jd! {"status" => "in-campaign", "id" => py_strip(c), "source" => format!("{where_}.campaign(측정 시점 활성 캠페인 · bench_preset)")});
        }
    }
    let note_v = bk.get("campaign_pins_note").cloned().unwrap_or(J::Null);
    let note = if note_v.truthy() { note_v.display() } else { String::new() };
    let note = py_strip(&note).to_string();
    if camp.is_null() {
        if let Some(m) = _CAMPAIGN_NONE_NOTE().match_at_start(&note) {
            let id = pyre::named(&m, "id").unwrap_or_default().to_string();
            let m0 = pyre::group(&m, 0).unwrap_or_default().to_string();
            return Ok(jd! {"status" => "outside", "id" => id,
                "source" => format!("{where_}.campaign = null · campaign_pins_note '{m0}'(측정 시점 ACTIVE 해소 · repeat_axis)")});
        }
    }
    Ok(jd! {"status" => "unobserved", "id" => J::Null,
        "source" => format!("미관측({where_}.campaign={} · campaign_pins_note={} — 알려진 모양 밖)", camp.repr(), note_v.repr())})
}

pub fn campaign_instances_of(repo: &Path, cell: &str) -> HR<Vec<J>> {
    let root = repo.join("campaigns");
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    for cdir in sorted_dirs(&root) {
        if RESERVED_CAMPAIGN_IDS.contains(&name_of(&cdir).as_str()) {
            continue;
        }
        let cells = cdir.join("cells");
        if !cells.is_dir() {
            continue;
        }
        for d in sorted_dirs(&cells) {
            if name_of(&d).starts_with('_') {
                continue;
            }
            let mut m: Option<&str> = None;
            if name_of(&d) == cell {
                m = Some("cell_dir");
            } else if d.join("config.yaml").is_file() {
                match _owner_yaml(repo, &d.join("config.yaml"), "캠페인 셀 config") {
                    Ok(doc) => {
                        if doc.is_dict() && doc["serving"]["config_name"].as_str() == Some(cell) {
                            m = Some("serving.config_name");
                        }
                    }
                    Err(e) if e.is_py() => return Err(e),
                    Err(_) => {}
                }
            }
            let Some(m) = m else { continue };
            let mut last = J::Null;
            let jp = cdir.join("journey.jsonl");
            if jp.is_file() {
                let text = read_lossy(&jp).map_err(|e| core::io_err(&e, &jp))?;
                let rows: Vec<String> = py_splitlines(&text).into_iter().filter(|l| !py_strip(l).is_empty()).collect();
                if let Some(r) = rows.last() {
                    match crate::value::loads(r) {
                        Ok(J::Dict(d)) => last = d.get("kind").cloned().unwrap_or(J::Null),
                        Ok(o) => return Err(HintError::py("AttributeError", format!("'{}' object has no attribute 'get'", o.type_name()))),
                        Err(_) => last = J::from("unparsable"),
                    }
                }
            }
            out.push(jd! {"campaign" => name_of(&cdir), "cell_dir" => rel_(repo, &d)?, "match" => m, "last_journey" => last});
        }
    }
    Ok(out)
}

pub fn campaign_absence(ev: &J, repo: Option<&Path>) -> HR<J> {
    if ev_mode_is_campaign(ev) {
        return Ok(jd! {"kind" => "none", "phrase" => "캠페인 모드", "reason" => J::Null, "source" => "캠페인 인스턴스 실재", "instances" => J::list()});
    }
    let mc = measurement_campaign(ev)?;
    let inst = match repo {
        Some(r) => campaign_instances_of(r, &ev["cell"].as_str().unwrap_or_default().to_string())?,
        None => Vec::new(),
    };
    let inst_txt = inst.iter().map(|x| format!("{}(journey 마지막 {})", x["cell_dir"].display(), x["last_journey"].display())).collect::<Vec<_>>().join(" · ");
    let src = mc["source"].clone();
    let srcs = src.display();
    let id = mc["id"].display();
    let instj = J::List(inst.clone());
    match mc["status"].as_str() {
        Some("in-campaign") => {
            let mine: Vec<&J> = inst.iter().filter(|x| x["campaign"] == mc["id"]).collect();
            if !mine.is_empty() {
                let dirs = mine.iter().map(|x| x["cell_dir"].display()).collect::<Vec<_>>().join(" · ");
                return Ok(jd! {"kind" => "preserved", "phrase" => format!("캠페인 {id} 인스턴스 보존 · 재생 모드는 읽지 않았다"), "instances" => instj,
                    "reason" => format!("측정 캠페인 {id} 의 인스턴스가 보존돼 있다({dirs}) — 재생 모드는 인스턴스를 읽지 않는다(캠페인 모드 발행이면 읽힌다 · {srcs})"),
                    "source" => src});
            }
            if repo.is_some() {
                return Ok(jd! {"kind" => "purged", "phrase" => format!("캠페인 purge({id} 인스턴스 부재 관측)"), "instances" => instj,
                    "reason" => format!("측정 캠페인 {id} 의 인스턴스가 campaigns/ 에 없다 — purge 됐다({srcs})"), "source" => src});
            }
            return Ok(jd! {"kind" => "purged", "phrase" => "캠페인 purge", "instances" => instj,
                "reason" => format!("측정 캠페인 {id} 의 인스턴스를 읽지 않았다({srcs})"), "source" => src});
        }
        Some("outside") => {
            let extra = if inst.is_empty() {
                String::new()
            } else {
                format!(" · 같은 셀의 앞선 캠페인 인스턴스 {inst_txt} 는 보존돼 있으나 이 측정의 것이 아니라 읽지 않았다")
            };
            return Ok(jd! {"kind" => "outside", "phrase" => format!("캠페인 밖 측정(측정 시점 ACTIVE={id} · 이 측정의 캠페인 인스턴스 없음)"),
                "instances" => instj,
                "reason" => format!("캠페인 밖 측정 — purge 된 인스턴스가 아니라 이 측정의 인스턴스는 애초에 없다({srcs}){extra}"), "source" => src});
        }
        _ => {}
    }
    if !inst.is_empty() {
        return Ok(jd! {"kind" => "preserved", "phrase" => "캠페인 인스턴스 보존 · 재생 모드는 읽지 않았다(측정 캠페인 소속 미관측)", "instances" => instj,
            "reason" => format!("같은 셀의 캠페인 인스턴스 {inst_txt} 가 보존돼 있다 — 재생 모드는 읽지 않는다 · 측정이 그 캠페인의 것인지는 미관측({srcs})"),
            "source" => src});
    }
    if repo.is_some() {
        return Ok(jd! {"kind" => "absent", "phrase" => "캠페인 인스턴스 부재(purge 인지 캠페인 밖인지 측정 기록으로 가르지 못함)", "instances" => J::list(),
            "reason" => format!("campaigns/ 에 이 셀의 인스턴스가 없다 · 측정 캠페인 소속 미관측({srcs})"), "source" => src});
    }
    Ok(jd! {"kind" => "purged", "phrase" => "캠페인 purge", "reason" => J::Null, "source" => src, "instances" => J::list()})
}

pub fn event_ledger_spans(repo: &Path, ev: &J) -> HR<Vec<J>> {
    let repo = repo_(repo)?;
    let repo = repo.as_path();
    let (self_node, self_src) = _self_node(repo)?;
    let end: Option<String> = match _measure_window(repo, ev) {
        Ok(w) => w.1,
        Err(e) if e.is_py() => return Err(e),
        Err(_) => None,
    };
    let decl: Vec<String> = ev["declaration"]["nodes"].items().iter().filter(|n| n.is_dict() && n["node_id"].truthy()).map(|n| n["node_id"].display()).collect();
    let have = _event_node_dirs(repo, ev)?;
    let mut want: Vec<String> = Vec::new();
    let pre = if ev["node"].as_str() == Some("cluster") { decl } else { Vec::new() };
    for n in pre.into_iter().chain(have) {
        if !want.contains(&n) {
            want.push(n);
        }
    }
    let mut out = Vec::new();
    for node in want {
        let mut files: Vec<String> = Vec::new();
        let (mut first, mut last): (Option<String>, Option<String>) = (None, None);
        let (node_files, node_src) = _node_event_files(repo, &node)?;
        for f in &node_files {
            let (rows, _bad) = _event_rows(f)?;
            if rows.is_empty() {
                continue;
            }
            files.push(rel_(repo, f)?);
            let (a, b) = (rows[0].0.clone(), rows[rows.len() - 1].0.clone());
            first = Some(match first { None => a, Some(x) => if a < x { a } else { x } });
            last = Some(match last { None => b, Some(x) => if b > x { b } else { x } });
        }
        let covers = match (&last, &end) {
            (Some(l), Some(e)) if !l.is_empty() && !e.is_empty() => J::Bool(l >= e),
            _ => J::Null,
        };
        let live = self_node.as_deref() == Some(node.as_str());
        let live_src = match &self_node {
            Some(s) => format!("이 체크아웃 노드 = {s}({self_src})"),
            None => format!("이 체크아웃 노드 미해소 — {self_src}"),
        };
        let mut span = jd! {"node" => node.clone(), "first_utc" => J::from(first), "last_utc" => J::from(last), "files" => J::from(files),
            "covers_measurement" => covers.clone(), "measurement_end_utc" => J::from(end.clone()), "location" => node_src["location"].clone(),
            "shadowed_local_copies" => node_src["shadowed"].clone(), "live_local" => live, "live_local_source" => live_src};
        if covers != J::Bool(true) {
            if let Some(echo) = _smoke_ledger_echo(repo, ev, &node)? {
                span.set("smoke_echo", echo);
            }
        }
        out.push(span);
    }
    Ok(out)
}

pub const SMOKE_ECHO_KINDS: [&str; 2] = ["budget_declare", "budget_honored"];
pub const SMOKE_ECHO_NOTE: &str = "원장 미러 밖 — 스모크 로그 echo";

pub fn _smoke_ledger_echo(repo: &Path, ev: &J, node: &str) -> HR<Option<J>> {
    let empty = J::dict();
    let att = if ev["attestation"].is_dict() { &ev["attestation"] } else { &empty };
    if !att["_path"].truthy() || att.get("config").cloned().unwrap_or(J::Null) != ev["cell"] {
        return Ok(None);
    }
    let written = _attestation_written(repo, att)?.0;
    let (timing, _note, win) = _attestation_timing(repo, ev, written.as_deref())?;
    if timing.as_deref() != Some("before-measurement") {
        return Ok(None);
    }
    let sb = attestation_same_boot(repo, ev, written.as_deref(), &win.unwrap_or(J::Null))?;
    let Some(sb) = sb.filter(|x| x.truthy()) else { return Ok(None) };
    if sb.get("strength").and_then(J::as_str) != Some("strong") {
        return Ok(None);
    }
    let rel = sb["files"].at(0).display();
    let lg = repo.join(&rel);
    if !lg.is_file() {
        return Ok(None);
    }
    let label = format!("smoke-{}", ev_cell(ev));
    let pat = Re::new(
        &format!(r"^\s*(?:\[[^\]]*\]\s*)?{}:\s+({})\b[^{{]*(\{{.*\}})\s*$", re_escape(node), SMOKE_ECHO_KINDS.join("|")),
        0,
    );
    let mut rows = Vec::new();
    for (i, line) in log_lines(&lg)?.iter().enumerate() {
        let clean = _ANSI().sub_literal(line, "");
        let Some(m) = pat.match_at_start(&clean) else { continue };
        let Ok(d) = crate::value::loads(pyre::group(&m, 2).unwrap_or_default()) else { continue };
        let ts = if d.is_dict() { d.get("ts").cloned().unwrap_or(J::Null) } else { J::Null };
        let ts_ok = matches!(&ts, J::Str(s) if utc_full(s));
        if !ts_ok || d.get("kind").and_then(J::as_str) != pyre::group(&m, 1) {
            continue;
        }
        let lab = d.get("label").cloned().unwrap_or(J::Null);
        if !(lab.is_null() || lab.as_str() == Some(label.as_str())) {
            continue;
        }
        rows.push(jd! {"ts" => ts, "kind" => d["kind"].clone(), "label" => lab, "source" => format!("{rel}:{}", i + 1)});
    }
    if rows.is_empty() {
        return Ok(None);
    }
    Ok(Some(jd! {"log" => rel.clone(), "rows" => J::List(rows),
        "basis" => format!("같은 기동 판별(attestation_same_boot 강한 판별)이 지목한 스모크 로그 {rel} 의 `{node}:` 원장 JSON echo 줄")}))
}

pub fn event_files(repo: &Path, ev: &J) -> HR<Vec<String>> {
    let repo = repo_(repo)?;
    Ok(_timeline_parts(&repo, ev)?.1.into_iter().map(|(p, _)| p).collect())
}

pub const CAMPAIGN_SCOPE_THIS: &str = "this-campaign";
pub const CAMPAIGN_SCOPE_PRIOR: &str = "prior-campaign";
pub const PRIOR_CAMPAIGN_NOTE: &str = "같은 셀 이름 · 앞선 캠페인";

pub fn _campaign_boundary(ev: &J, win: &J, repo: Option<&Path>) -> HR<J> {
    let camp = ev_mode_is_campaign(ev);
    let decl = if camp { ev["declaration"]["declared_utc"].as_str().filter(|s| utc_full(s)).map(str::to_string) } else { None };
    let ws = if camp { win.get("start_ts").filter(|x| x.truthy()).map(J::display) } else { None };
    if decl.is_none() && ws.is_none() {
        let why = if !camp {
            format!("재생 모드({}) — 캠페인 선언을 관측하지 못한다", campaign_absence(ev, repo)?["phrase"].display())
        } else {
            "캠페인 선언 declared_utc · 측정을 서빙한 선언 창 모두 미관측".to_string()
        };
        return Ok(jd! {"utc" => J::Null, "observed" => false, "campaign_id" => ev["campaign_id"].clone(),
            "source" => format!("{why} · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)")});
    }
    let mut parts = Vec::new();
    if let Some(d) = &decl {
        parts.push(format!("campaigns/{}/campaign.yaml declared_utc {d}", ev["campaign_id"].display()));
    }
    if let Some(w) = &ws {
        parts.push(format!("이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 {w}"));
    }
    let b = [decl, ws].into_iter().flatten().filter(|x| !x.is_empty()).min().unwrap_or_default();
    Ok(jd! {"utc" => b.clone(), "observed" => true, "campaign_id" => ev["campaign_id"].clone(),
        "source" => parts.join(" · ") + &format!(" → 경계 = 앞선 값 {b}(이전 행 = {PRIOR_CAMPAIGN_NOTE})")})
}

pub fn event_campaign_boundary(repo: &Path, ev: &J) -> HR<J> {
    let repo = repo_(repo)?;
    let win = _ledger_timeline_mut(&repo, ev, None)?.2.unwrap_or(J::Null);
    let bnd = _campaign_boundary(ev, &win, Some(&repo))?;
    let same = _same_cell_pre_sessions(&repo, ev, bnd["utc"].as_str())?;
    _note_same_cell(&bnd, &J::List(same))
}

pub fn _note_same_cell(bnd: &J, same: &J) -> HR<J> {
    if !same.truthy() {
        return Ok(bnd.clone());
    }
    let items = same.items();
    let mut m = bnd.as_dict().cloned().unwrap_or_default();
    let src = format!(
        "{} · 단 이전 기동 중 {}건은 {SAME_CELL_PRE_NOTE}(세션 id 가 이 셀 기록에 관측: {})",
        bnd["source"].display(),
        items.len(),
        items.iter().map(|s| format!("{} = {}", s["session"].display(), s["source"].display())).collect::<Vec<_>>().join(" · ")
    );
    m.insert("same_cell_sessions", same.clone());
    m.insert("source", src);
    Ok(J::Dict(m))
}

pub const CAMPAIGN_SCOPE_SAME_CELL_PRE: &str = "same-cell-pre-boundary";
pub const SAME_CELL_PRE_NOTE: &str = "캠페인 선언 전 · 같은 셀 기동";
crate::re_static!(_SIMLOG_DIR_REF, r#"docs/simlog/([^\s`'"()<>|/]+)/"#);

pub fn _cell_simlog_logs(repo: &Path, ev: &J) -> HR<Vec<PathBuf>> {
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    let cell = ev["cell"].clone();
    for p in ev["pointers"].items() {
        if !p.is_dict() {
            return Err(HintError::py("AttributeError", format!("'{}' object has no attribute 'get'", p.type_name())));
        }
        let Some(rel) = p.get("path").and_then(J::as_str) else { continue };
        if !str_in(&p["kind"], &["testlog", "devlog"]) || p.get("cell_id").cloned().unwrap_or(J::Null) != cell {
            continue;
        }
        if Path::new(rel).is_absolute() || Path::new(rel).components().any(|c| c.as_os_str() == "..") || !repo.join(rel).is_file() {
            continue;
        }
        let Ok(text) = read_lossy(&repo.join(rel)) else { continue };
        dirs.extend(_SIMLOG_DIR_REF().findall(&text));
    }
    let mut out = Vec::new();
    for d in dirs {
        let base = repo.join("docs").join("simlog").join(&d);
        if base.is_dir() && !is_symlink(&base) {
            out.extend(glob_suffix(&base, "", ".log").into_iter().filter(|f| f.is_file() && !is_symlink(f)));
        }
    }
    Ok(out)
}

pub fn _same_cell_pre_sessions(repo: &Path, ev: &J, b: Option<&str>) -> HR<Vec<J>> {
    let Some(b) = b.filter(|x| !x.is_empty()) else { return Ok(Vec::new()) };
    let cell = ev_cell(ev);
    let mut starts: Vec<(String, String)> = Vec::new();
    let mut declares: Vec<String> = Vec::new();
    for node in _event_node_dirs(repo, ev)? {
        let sessions = _node_session_docs(repo, &node)?;
        for f in _node_event_files(repo, &node)?.0 {
            for (ts, _ln, d) in _event_rows(&f)?.0 {
                if ts.as_str() >= b {
                    continue;
                }
                if kind_in(&d, &["serve_start"]) && session_event_is_cell(&d, &ts, ev, &sessions) {
                    starts.push((ts.clone(), d["session_id"].display()));
                } else if kind_in(&d, &["budget_declare"]) && label_is_cell(&d.get("label").cloned().unwrap_or(J::Null), &cell) {
                    declares.push(ts);
                }
            }
        }
    }
    if starts.is_empty() {
        return Ok(Vec::new());
    }
    let mut logs: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for lg in _cell_simlog_logs(repo, ev)? {
        let lines = log_lines(&lg)?;
        logs.push((lg, lines));
    }
    starts.sort();
    starts.dedup();
    let mut out = Vec::new();
    for (i, (ts, sid)) in starts.iter().enumerate() {
        let mut hit = None;
        'o: for (lg, lines) in &logs {
            for (n, x) in lines.iter().enumerate() {
                if x.contains(sid.as_str()) {
                    hit = Some(format!("{}:{}", rel_(repo, lg)?, n + 1));
                    break 'o;
                }
            }
        }
        let Some(hit) = hit else { continue };
        let nxt = declares
            .iter()
            .filter(|x| x.as_str() > ts.as_str())
            .chain(starts[i + 1..].iter().map(|(s, _)| s).filter(|s| s.as_str() > ts.as_str()))
            .map(String::as_str)
            .chain(std::iter::once(b))
            .min()
            .unwrap()
            .to_string();
        out.push(jd! {"session" => sid.clone(), "start" => ts.clone(), "end" => nxt, "source" => hit});
    }
    Ok(out)
}

pub fn _scope_rows(rows: &J, bnd: &J, same_cell: &J) -> HR<Vec<J>> {
    let Some(b) = bnd.get("utc").filter(|x| x.truthy()).map(J::display) else { return Ok(rows.items().to_vec()) };
    let mut out = Vec::new();
    for r in rows.items() {
        let Some(u) = r.get("utc").and_then(J::as_str) else {
            out.push(r.clone());
            continue;
        };
        let sc = if u < b.as_str() {
            same_cell.items().iter().find(|s| s["start"].display().as_str() <= u && u < s["end"].display().as_str())
        } else {
            None
        };
        let det = r.get("detail").filter(|x| x.truthy()).map(J::display).unwrap_or_default();
        let mut m = r.as_dict().cloned().unwrap_or_default();
        if let Some(sc) = sc {
            m.insert("campaign_scope", CAMPAIGN_SCOPE_SAME_CELL_PRE);
            m.insert(
                "detail",
                format!(
                    "{det} · {SAME_CELL_PRE_NOTE}(캠페인 경계 {b} 이전이나 세션 {} 이 이 셀 기록 {} 에 관측 — 이 셀의 시도로 센다)",
                    sc["session"].display(),
                    sc["source"].display()
                ),
            );
        } else if u < b.as_str() {
            m.insert("campaign_scope", CAMPAIGN_SCOPE_PRIOR);
            m.insert("detail", format!("{det} · {PRIOR_CAMPAIGN_NOTE}(캠페인 경계 {b} 이전 — 이 셀의 시도로 세지 않는다)"));
        } else {
            m.insert("campaign_scope", CAMPAIGN_SCOPE_THIS);
        }
        out.push(J::Dict(m));
    }
    Ok(out)
}

pub fn _timeline_parts(repo: &Path, ev: &J) -> HR<(Vec<J>, Vec<(String, String)>)> {
    let mut ctx = Map::new();
    let (rows, files, win) = _ledger_timeline_mut(repo, ev, Some(&mut ctx))?;
    let (mrows, mfiles) = _measurement_rows(repo, ev, &J::Dict(ctx))?;
    let win = win.unwrap_or(J::Null);
    let bnd = _campaign_boundary(ev, &win, Some(repo))?;
    let mut all: Vec<J> = rows.clone();
    all.extend(mrows);
    all.extend(_startup_markers(repo, ev, &win)?);
    all.sort_by(|a, b| tl_key(a).cmp(&tl_key(b)));
    let same = _same_cell_pre_sessions(repo, ev, bnd["utc"].as_str())?;
    let out = _scope_rows(&J::List(all), &bnd, &J::List(same))?;
    let mut cited: Vec<(String, String)> = files.iter().map(|f| (f.clone(), "events_ledger".to_string())).collect();
    let put = |cited: &mut Vec<(String, String)>, k: String, v: &str, overwrite: bool| {
        if let Some(e) = cited.iter_mut().find(|(a, _)| *a == k) {
            if overwrite {
                e.1 = v.to_string();
            }
        } else {
            cited.push((k, v.to_string()));
        }
    };
    for r in &rows {
        if let Some(s) = r.get("session_binding_source").filter(|x| x.truthy()) {
            put(&mut cited, s.display(), "raw_json", true);
        }
    }
    for (f, k) in mfiles {
        put(&mut cited, f, &k, false);
    }
    cited.sort();
    Ok((out, cited))
}

pub fn _json_key_line(path: &Path, key: &str, value: &J) -> HR<Option<i64>> {
    let Ok(text) = read_lossy(path) else { return Ok(None) };
    let rx = Re::new(&format!(r#""{}"\s*:\s*{}"#, re_escape(key), re_escape(&crate::value::dumps_unicode(value))), 0);
    Ok(text.split('\n').enumerate().find(|(_, l)| rx.is_search(l)).map(|(i, _)| i as i64 + 1))
}

fn files_j(files: &[(String, String)]) -> J {
    J::List(files.iter().map(|(a, b)| jl![a.clone(), b.clone()]).collect())
}

pub fn _other_measurements(repo: &Path, ev: &J) -> HR<Vec<J>> {
    let measured = _measured_key(repo, ev)?;
    let ident = match ev["lineage_seeds"].get("identity").filter(|x| x.truthy()) {
        Some(x) => x.clone(),
        None => _measured_identity(repo, ev)?.0,
    };
    let this_topic = ev["publication"]["topic"].clone();
    let mut found: Vec<(String, J)> = Vec::new();
    let mut add = |utc: &J, node: &J, what: String, source: String, files: Vec<(String, String)>| {
        let Some(u) = utc.as_str().filter(|u| utc_full(u) && Some(*u) != measured.as_deref()) else { return };
        let idx = match found.iter().position(|(k, _)| k == u) {
            Some(i) => i,
            None => {
                let nd = if str_in(node, &NODE_AXIS) { node.clone() } else { ev["node"].clone() };
                found.push((u.to_string(), jd! {"utc" => u, "node" => nd, "what" => J::list(), "source" => source, "files" => J::list()}));
                found.len() - 1
            }
        };
        let e = &mut found[idx].1;
        e.get_mut("what").unwrap().push(what);
        for f in files {
            e.get_mut("files").unwrap().push(jl![f.0, f.1]);
        }
    };
    let cell = ev_cell(ev);
    for rec in publication_records(repo)? {
        let Some(rid) = rec.get("_id").and_then(J::as_str).map(str::to_string) else { continue };
        if rec.get("_unreadable").is_some() || this_topic.as_str() == Some(rid.as_str()) {
            continue;
        }
        if !crate::lineage::identity_equal(&rec["identity"], &ident)? {
            continue;
        }
        let (via, _mm) = crate::lineage::record_cell_match(repo, &rid, &rec, &cell)?;
        let Some(via) = via else { continue };
        let rec_rel = format!("{}/{rid}.json", core::REL_EVIDENCE_DIR);
        let empty = J::dict();
        let bm = if rec["benchmark"].is_dict() { &rec["benchmark"] } else { &empty };
        let mut mu = norm_value(&bm["measured_utc"])?;
        let s1 = &rec["raw_log_paths"]["simlog"];
        let simlog = if s1.truthy() { s1.clone() } else { rec["scaffolded"]["simlog"].clone() };
        let (idx, idx_bad) = match simlog.as_str() {
            Some(s) if !s.is_empty() && !Path::new(s).is_absolute() && !Path::new(s).components().any(|c| c.as_os_str() == "..") => {
                read_soft(&repo.join(s).join("sweep_index.json"))?
            }
            _ => (J::Null, J::Null),
        };
        let idx = if idx.is_dict() { idx } else { J::dict() };
        let gu = idx["generated_utc"].as_str().map(str::to_string);
        let node = if idx["meta"].is_dict() { idx["meta"].get("measured_node").cloned().unwrap_or(J::Null) } else { J::Null };
        let mut files: Vec<(String, String)> = Vec::new();
        let idx_rel = gu.as_ref().map(|_| format!("{}/sweep_index.json", simlog.display()));
        let source;
        if mu.truthy() {
            let ln = _json_key_line(&repo.join(&rec_rel), "measured_utc", &mu)?;
            source = match ln { Some(l) => format!("{rec_rel}:{l}"), None => rec_rel.clone() };
            files.push((rec_rel.clone(), "publication_record".into()));
        } else if let Some(g) = &gu {
            mu = J::from(g.clone());
            let ir = idx_rel.clone().unwrap();
            let ln = _json_key_line(&repo.join(&ir), "generated_utc", &mu)?;
            source = match ln { Some(l) => format!("{ir}:{l}"), None => ir };
        } else {
            continue;
        }
        if let Some(ir) = &idx_rel {
            files.push((ir.clone(), "raw_json".into()));
        }
        let verdict = norm_value(&bm["verdict"])?;
        let extra = match &gu {
            Some(g) => format!(
                " · simlog 사본 generated_utc {g}{}",
                if J::from(g.clone()) == mu { "(같음)" } else { " ≠ measured_utc — 기록끼리 어긋남" }
            ),
            None if idx_bad.truthy() => format!(" · simlog 사본 {}", idx_bad.display()),
            None => String::new(),
        };
        let vtxt = if verdict.truthy() { verdict.display() } else { "미기재".to_string() };
        add(&mu, &node, format!("발행 기록 {rid}(셀 대조 {via} · verdict {vtxt}){extra}"), source, files);
    }
    let topo = ev["topology"].as_str().unwrap_or_default().to_string();
    for sw in _output_sweeps(repo, &topo, Some(&cell))? {
        let out_idx = repo.join(sw["dir"].display()).join("sweep_index.json");
        let doc = &sw["index"];
        let empty = J::dict();
        let meta = if doc["meta"].is_dict() { &doc["meta"] } else { &empty };
        let gu = doc["generated_utc"].clone();
        let sid = jd! {"model" => meta["model"].clone(), "gpu" => meta["gpu_model"].clone(), "topology" => meta["topology"].clone(),
            "tp" => meta["tensor_parallel_size"].clone()};
        if gu.is_str() && meta["config_name"] == ev["cell"] && crate::lineage::identity_equal(&sid, &ident)? {
            let rel = rel_(repo, &out_idx)?;
            let ln = _json_key_line(&out_idx, "generated_utc", &gu)?;
            let src = match ln { Some(l) => format!("{rel}:{l}"), None => rel.clone() };
            add(&gu, &meta["measured_node"], "출력 평면에 보존된 같은 셀의 다른 측정".to_string(), src, vec![(rel, "sweep_json".into())]);
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found.into_iter().map(|(_, v)| v).collect())
}

pub fn _measurement_rows(repo: &Path, ev: &J, ctx: &J) -> HR<(Vec<J>, Vec<(String, String)>)> {
    let cell = ev_cell(ev);
    let measured = _measured_key(repo, ev)?;
    let nodes: Vec<String> = ctx["nodes"].items().iter().map(J::display).collect();
    let spans = &ctx["spans"];
    let wins: Vec<J> = ctx["windows"].items().to_vec();
    let mut brows: Vec<J> = ctx["budget_rows"].items().to_vec();
    brows.sort_by(|a, b| (a["utc"].display(), a["rel"].display(), a["line"].as_i64()).cmp(&(b["utc"].display(), b["rel"].display(), b["line"].as_i64())));
    let mut out = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();
    let sp = |n: &str| -> Option<(String, String)> { spans.get(n).map(|s| (s.at(0).display(), s.at(1).display())) };
    let span_txt = |ns: &[String]| -> String {
        ns.iter()
            .map(|n| format!("{n}({})", match sp(n) { Some((a, b)) => format!("기록 {a}~{b}"), None => "행 없음".into() }))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let wtxt = |w: &J| -> String {
        let end = if w["end_ts"].truthy() { w["end_ts"].display() } else { format!("닫힘 미관측(스트림 마지막 행 {})", w["stream_last"].display()) };
        let lab = if w["label"].truthy() { w["label"].display() } else { "라벨 없음".into() };
        format!("{lab} {}~{end} ({}:{})", w["start_ts"].display(), w["rel"].display(), w["line"].display())
    };
    for m in _other_measurements(repo, ev)? {
        let t = m["utc"].display();
        let seen: Vec<String> = nodes.iter().filter(|n| sp(n).map_or(false, |(a, b)| a <= t && t <= b)).cloned().collect();
        let unseen: Vec<String> = nodes.iter().filter(|n| !seen.contains(n)).cloned().collect();
        let live: Vec<&J> = wins.iter().filter(|w| seen.contains(&w["node"].display()) && w["start_ts"].display() <= t).collect();
        let cover: Vec<&J> = live
            .iter()
            .copied()
            .filter(|w| if w["end_ts"].is_null() { t <= w["stream_last"].display() } else { t <= w["end_ts"].display() })
            .collect();
        let dangling: Vec<&J> = live.iter().copied().filter(|w| w["end_ts"].is_null() && t > w["stream_last"].display()).collect();
        let mine: Vec<&J> = cover.iter().copied().filter(|w| label_is_cell(&w["label"], &cell)).collect();
        let gap = if !unseen.is_empty() && !seen.is_empty() { format!(" · 이 시각을 관측하지 않은 원장: {}", span_txt(&unseen)) } else { String::new() };
        let jw = |ws: &[&J]| ws.iter().map(|w| wtxt(w)).collect::<Vec<_>>().join(" · ");
        let (label, led) = if nodes.is_empty() {
            ("measurement (ledger unobserved)", "원장 미관측(이 셀 노드 축의 docs/logs/<node>/events 없음)".to_string())
        } else if seen.is_empty() {
            ("measurement (ledger unobserved)", format!("원장이 이 시각을 관측하지 않는다(기록 범위 밖): {}", span_txt(&unseen)))
        } else if !mine.is_empty() {
            ("measurement (budget window recorded)", format!("이 셀 선언 창 안: {}{gap}", jw(&mine)))
        } else if !cover.is_empty() {
            ("measurement (other label's budget window)", format!("이 셀 라벨의 선언은 없고 다른 라벨의 창 안: {}{gap}", jw(&cover)))
        } else if !dangling.is_empty() {
            (
                "measurement (budget window closure unobserved)",
                format!("덮는 창은 관측되지 않았으나 스트림이 창을 연 채 끝났다(그 시각의 창 상태 미관측): {}{gap}", jw(&dangling)),
            )
        } else {
            (
                "measurement (no budget event recorded)",
                format!("이 시각을 관측한 원장({})의 어느 라벨의 예산 선언 창도 이 시각을 덮지 않는다 — 예산 선언 없이 잰 측정{gap}", seen.join(", ")),
            )
        };
        let before: Vec<&J> = brows.iter().filter(|r| r["utc"].display() <= t).collect();
        let after: Vec<&J> = brows.iter().filter(|r| r["utc"].display() > t).collect();
        let mut near = Vec::new();
        for (tag, r) in [("직전", before.last().copied()), ("직후", after.first().copied())] {
            let Some(r) = r else {
                near.push(format!("{tag} 예산 행 없음"));
                continue;
            };
            let lab = if r["label"].truthy() { format!("({})", r["label"].display()) } else { String::new() };
            near.push(format!("{tag} 예산 행 {}{lab} {}({}:{})", r["kind"].display(), r["utc"].display(), r["rel"].display(), r["line"].display()));
            files.push((r["rel"].display(), "events_ledger".into()));
        }
        for w in cover.iter().chain(dangling.iter()) {
            files.push((w["rel"].display(), "events_ledger".into()));
        }
        let rel_m = match &measured {
            None => "측정 시각 미관측".to_string(),
            Some(ms) => format!("이 발행의 측정 {ms} {}", if t.as_str() < ms.as_str() { "전" } else { "뒤 — 이 측정과 무관할 수 있다" }),
        };
        let what = m["what"].items().iter().map(J::display).collect::<Vec<_>>().join(" / ");
        let detail = format!(
            "같은 셀의 다른 측정(측정 노드 축 {}) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 미기록) · {rel_m} · {what} · 원장: {led} · {}",
            m["node"].display(),
            near.join(" · ")
        );
        out.push(jd! {"utc" => t, "node" => J::Null, "kind" => "measurement", "label" => label, "detail" => detail, "source" => m["source"].clone()});
        for f in m["files"].items() {
            files.push((f.at(0).display(), f.at(1).display()));
        }
    }
    Ok((out, files))
}

pub fn bench_definition(repo: &Path, ev: &J) -> HR<J> {
    let repo = repo_(repo)?;
    let repo = repo.as_path();
    let mut out = jd! {"current_full_definition" => J::Null, "source" => REL_DOCS_RULES, "source_line" => J::Null, "required_repeats" => J::Null,
        "required_legs" => J::list(), "this_repeats" => J::Null, "repeats_source" => "미관측", "bench_tool" => J::Null,
        "bench_tool_source" => J::Null, "verdict_level" => J::Null, "meets_current_full" => J::Null, "reasons" => J::list()};
    let mut reasons: Vec<String> = Vec::new();
    let p = repo.join(REL_DOCS_RULES);
    if p.is_file() {
        for (i, ln) in read_text_universal(&p)?.split('\n').enumerate() {
            if let Some(m) = _FULL_DEF_RE().search(ln) {
                out.set("current_full_definition", pyre::group(&m, 0).unwrap_or_default());
                out.set("source_line", i as i64 + 1);
                break;
            }
        }
    }
    if !out["current_full_definition"].truthy() {
        let skill_rel = ".claude/skills/adversarial-benchmark/SKILL.md";
        let skill = repo.join(skill_rel);
        if skill.is_file() {
            for (i, ln) in py_splitlines(&read_text_universal(&skill)?).iter().enumerate() {
                if ln.contains("full bench 의 정의") && ln.contains("`full = lite ∪ GuideLLM × 반복 ≥3`") {
                    out.set("current_full_definition", "`full = lite ∪ GuideLLM × 반복 ≥3`");
                    out.set("source", skill_rel);
                    out.set("source_line", i as i64 + 1);
                    out.set("required_repeats", 3);
                    out.set("required_legs", jl!["lite", "GuideLLM"]);
                    break;
                }
            }
        }
    }
    let sent = out["current_full_definition"].as_str().map(str::to_string).filter(|s| !s.is_empty());
    if let Some(sent) = &sent {
        if let Some(dm) = _FULL_DEF_RE().search(sent) {
            let inner = pyre::named(&dm, "inner").unwrap_or_default();
            let rm = _FULL_REPEATS_RE().search(inner);
            out.set("required_repeats", J::from(rm.and_then(|r| pyre::group(&r, 1).and_then(|x| x.parse::<i64>().ok()))));
            let legs = inner.split('×').next().unwrap_or_default();
            let mut all: Vec<String> = _FULL_GATE_LEG_RE().findall(pyre::named(&dm, "head").unwrap_or_default());
            all.extend(legs.split('∪').map(|x| py_strip(x).to_string()).filter(|x| !x.is_empty()));
            let mut dd: Vec<String> = Vec::new();
            for x in all {
                if !dd.contains(&x) {
                    dd.push(x);
                }
            }
            out.set("required_legs", J::from(dd));
        }
    } else {
        reasons.push(format!("{REL_DOCS_RULES} 에서 full 정의 문장(`full 계측(…)`)을 읽지 못했다 — 대조하지 않는다"));
    }
    let mc = measurement_config(repo, ev)?;
    out.set("bench_tool", mc.get("bench_tool").cloned().unwrap_or(J::Null));
    out.set("bench_tool_source", mc.get("source").cloned().unwrap_or(J::Null));
    let sw = &ev["sweep"];
    let vp = as_int(&sw["index"]["verdict_point_level"])?;
    out.set("verdict_level", J::from(vp));
    let req = as_int(&mc["repeats"])?;
    let mut this: Option<i64> = None;
    let mut rsrc = "미관측".to_string();
    let lv = match vp {
        Some(v) => {
            let mut f = None;
            for x in sw["levels"].items() {
                if x.is_dict() && as_int(&x["level"])? == Some(v) {
                    f = Some(x.clone());
                    break;
                }
            }
            f
        }
        None => None,
    };
    let lm = lv.as_ref().map(|l| l["measured"].clone()).filter(J::is_dict).unwrap_or_else(J::dict);
    let src_disp = mc.get("source").cloned().unwrap_or(J::Null).display();
    if let Some(rc) = as_int(&mc["repeats_completed"])? {
        this = Some(rc);
        rsrc = format!("측정 구성 표 repeats_completed(완주 · {src_disp})");
    } else if vp.is_none() {
        rsrc = "미관측 — 스윕 판정점 레벨(verdict_point_level) 미기재 · 측정 구성 표에 완주 수 없음".into();
    } else if let Some(rc) = as_int(&lm["repeats_completed"])? {
        this = Some(rc);
        rsrc = format!("sweep_index levels[{}].measured.repeats_completed(완주 · {})", vp.unwrap(), sw["dir"].display());
    } else if lm["runs"].is_list() {
        this = Some(lm["runs"].items().iter().filter(|r| r.is_dict() && r["measurement_ok"] == J::Bool(true)).count() as i64);
        rsrc = format!("sweep_index levels[{}].measured.runs[] measurement_ok 수(완주 · {})", vp.unwrap(), sw["dir"].display());
    } else if sw["binding"].as_str() == Some("output") {
        let cell = ev_cell(ev);
        let ldir = repo.join(sw["dir"].display()).join(format!("level_{:02}", vp.unwrap()));
        let run_dirs: Vec<String> = if ldir.is_dir() {
            let mut v: Vec<String> = sorted_dirs(&ldir).iter().map(|d| name_of(d)).filter(|n| n.starts_with("run_")).collect();
            v.sort();
            v
        } else {
            Vec::new()
        };
        let bj = ldir.join(format!("bench_{cell}.json"));
        if !run_dirs.is_empty() {
            rsrc = format!(
                "판정 불가 — {} 에 반복 축 {} 가 있는데 집계(runs[]·repeats_completed)가 없다(파일 수는 실패 run 까지 센다)",
                rel_(repo, &ldir)?,
                J::from(run_dirs).repr()
            );
        } else if bj.is_file() && core::level_raw_is_measured_tool(&bj, &cell) {
            this = Some(1);
            rsrc = format!("observed({}/bench_{cell}.json 1개 · run_KK/ 없음 · runs[] 없음 — 반복 축 이전의 단일 run 배치)", rel_(repo, &ldir)?);
        }
    }
    if this.is_none() {
        if let Some(r) = req {
            rsrc += &format!(" · 요청 반복 {r}(선언 — 완주 수가 아니라 판정에 쓰지 않는다)");
        }
    }
    out.set("this_repeats", J::from(this));
    out.set("repeats_source", rsrc);
    let bm = mc.get("bench_mode").cloned().unwrap_or(J::Null);
    let need = out["required_repeats"].as_i64();
    let legs: Vec<String> = out["required_legs"].items().iter().map(|x| x.display().to_lowercase()).collect();
    let bt = out["bench_tool"].clone();
    let tool = if bt.truthy() { bt.display().to_lowercase() } else { String::new() };
    let has_g = legs.iter().any(|l| l == "guidellm");
    let mut verdict: Option<bool> = None;
    if sent.is_some() {
        if bm.as_str() == Some("lite") {
            verdict = Some(false);
            reasons.push("측정 구성 bench_mode=lite — full 측정이 아니다".into());
        }
        if let (Some(n), Some(t)) = (need, this) {
            if t < n {
                verdict = Some(false);
                reasons.push(format!("판정점 반복 {t} < 현행 정의 {n}"));
            }
        }
        if has_g && !tool.is_empty() && !tool.contains("guidellm") {
            verdict = Some(false);
            reasons.push(format!("측정 도구 {} — 현행 정의의 full 레그(GuideLLM)가 아니다", bt.display()));
        }
        if verdict.is_none() {
            if let (Some(n), Some(t)) = (need, this) {
                if t >= n && (!has_g || tool.contains("guidellm")) {
                    verdict = Some(true);
                    reasons.push(format!("판정점 반복 {t} ≥ {n} · 도구 {}", bt.display()));
                }
            }
        }
        if verdict.is_none() {
            reasons.push("반복 수 또는 도구를 관측하지 못했다 — 충족 여부를 판정하지 않는다(모름 ≠ 미충족)".into());
        }
        if verdict == Some(false) && bm.as_str() == Some("full") {
            reasons.push(format!("측정 기록의 bench_mode=full 은 기록된 라벨({src_disp})이다 — 위 대조는 현행 정의 기준이라 두 판정이 갈린다"));
        }
    }
    let mut meets = J::from(verdict);
    if out["source"].display().ends_with("adversarial-benchmark/SKILL.md") && verdict == Some(true) {
        let lite = &sw["index"]["lite"];
        let levels = sw["levels"].items();
        let mut counts: Vec<Option<i64>> = Vec::new();
        for lv in levels.iter().filter(|l| l.is_dict()) {
            counts.push(as_int(&lv["measured"]["repeats_completed"])?);
        }
        let n = need.unwrap_or(0);
        if lite["lite_verdict"].as_str() == Some("pass") && !levels.is_empty() && counts.len() == levels.len() && counts.iter().all(|c| c.map_or(false, |c| c >= n)) {
            reasons.push(format!("sweep_index lite_verdict=pass · 측정 레벨 {}개 모두 완주 반복 ≥{}", levels.len(), opt_disp(&need.map(|x| x.to_string()))));
        } else {
            meets = J::Null;
            reasons.push("판정점 반복과 도구는 확인했다 — lite 통과 또는 전체 측정 레벨의 완주 반복을 확인하지 못했다".into());
        }
    }
    out.set("meets_current_full", meets);
    out.set("reasons", J::from(reasons));
    out.set("repeats_requested", J::from(req));
    Ok(out)
}

/// hintlib/evidence.py:3215 _BENCH_JSON_FLAGS (python literal as JSON)
pub fn _BENCH_JSON_FLAGS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("[[\"backend\", \"--backend\"], [\"endpoint\", \"--endpoint\"], [\"model_id\", \"--model\"], [\"tokenizer_id\", \"--tokenizer\"], [\"dataset_name\", \"--dataset-name\"], [\"random_input_len\", \"--random-input-len\"], [\"random_output_len\", \"--random-output-len\"], [\"random_range_ratio\", \"--random-range-ratio\"], [\"num_prompts\", \"--num-prompts\"], [\"max_concurrency\", \"--max-concurrency\"], [\"request_rate\", \"--request-rate\"], [\"burstiness\", \"--burstiness\"], [\"num_warmups\", \"--num-warmups\"], [\"temperature\", \"--temperature\"], [\"seed\", \"--seed\"]]").expect("constant"))
}
/// hintlib/evidence.py:3221 _BENCH_JSON_BOOL_FLAGS (python literal as JSON)
pub fn _BENCH_JSON_BOOL_FLAGS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("[[\"ignore_eos\", \"--ignore-eos\"], [\"trust_remote_code\", \"--trust-remote-code\"]]").expect("constant"))
}
pub const BENCH_COMMAND_SOURCE: &str = "reconstructed(bench json)";
pub const _BENCH_UNRECORDED: [&str; 9] = ["--base-url/--port", "--endpoint", "--dataset-name", "--ignore-eos", "--num-warmups", "--temperature", "--random-range-ratio", "--seed", "--trust-remote-code"];

pub fn bench_command(repo: &Path, ev: &J) -> HR<J> {
    let repo = repo_(repo)?;
    let repo = repo.as_path();
    let sw = &ev["sweep"];
    let unrec: Vec<String> = _BENCH_UNRECORDED.iter().map(|s| s.to_string()).collect();
    let none = |source: String| -> J {
        jd! {"command" => J::Null, "command_source" => J::Null, "fields" => J::list(), "unrecorded" => J::from(unrec.clone()), "source" => source}
    };
    if sw["binding"].as_str() != Some("output") {
        return Ok(none(format!(
            "미재구성 — 스윕이 출력 평면에 묶이지 않았다(binding={}) · 레벨 원시가 이 측정의 것이 아니다",
            sw.get("binding").cloned().unwrap_or(J::Null).repr()
        )));
    }
    let cell = ev_cell(ev);
    let idx = &sw["index"];
    let vp_rec = as_int(&idx["verdict_point_level"])?.filter(|v| *v != 0);
    let vp = vp_rec.unwrap_or(1);
    let at = if vp_rec.is_some() { format!("판정점 level {vp}") } else { "판정점 미기재 → 첫 레벨 level 1(판정점이라는 뜻 아님)".to_string() };
    let path = repo.join(sw["dir"].display()).join(format!("level_{vp:02}")).join(format!("bench_{cell}.json"));
    let doc = if path.is_file() { read_json_opt(&path, "레벨 벤치 JSON")? } else { J::Null };
    let rel = rel_(repo, &path)?;
    if doc.is_dict() && !core::level_raw_is_measured_tool(&path, &cell) {
        return Ok(none(format!(
            "미재구성 — {rel} 는 이 레벨의 측정 도구(bench_tool_{cell}.json)가 아닌 옛 도구 원시다({at} · 도구를 바꿔 재스윕한 자리의 잔재 — 2026-09-23 D1)"
        )));
    }
    let Some(dm) = doc.as_dict() else { return Ok(none(format!("미재구성 — {rel} 부재({at})"))) };
    if !["num_prompts", "total_input_tokens", "completed"].iter().all(|k| dm.contains_key(k)) {
        return Ok(none(format!("미재구성 — {rel} 가 vllm bench serve 결과 모양이 아니다(num_prompts·total_input_tokens 부재)")));
    }
    let mut fields: Vec<J> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let mut add = |flag: &str, value: J, key: &str, source: String| {
        fields.push(jd! {"flag" => flag, "value" => value.clone(), "field" => key, "source" => source});
        args.push(flag.to_string());
        if value != J::Bool(true) {
            let s = match &value {
                J::Bool(false) => "False".to_string(),
                v => v.display(),
            };
            args.push(shlex_quote(&s));
        }
    };
    for fl in _BENCH_JSON_FLAGS().items() {
        let (key, flag) = (fl.at(0).display(), fl.at(1).display());
        let v = dm.get(&key).cloned().unwrap_or(J::Null);
        if v.is_null() || v.is_dict() || v.is_list() || matches!(&v, J::Str(s) if py_strip(s).is_empty()) {
            continue;
        }
        add(&flag, v, &key, format!("{rel} {key}"));
    }
    for fl in _BENCH_JSON_BOOL_FLAGS().items() {
        let (key, flag) = (fl.at(0).display(), fl.at(1).display());
        if dm.get(&key) == Some(&J::Bool(true)) {
            add(&flag, J::Bool(true), &key, format!("{rel} {key}=true"));
        }
    }
    let done = as_int(&doc["completed"])?;
    let mut not_derived: Vec<String> = Vec::new();
    for (key, tot, flag) in [("random_input_len", "total_input_tokens", "--random-input-len"), ("random_output_len", "total_output_tokens", "--random-output-len")] {
        let t = as_int(&doc[tot])?;
        if dm.contains_key(key) {
            continue;
        }
        let (Some(dn), Some(tv)) = (done.filter(|d| *d != 0), t) else {
            not_derived.push(format!("{flag}({tot} ÷ completed 가 나누어떨어지지 않거나 없음 — 파생 ✗)"));
            continue;
        };
        if tv.rem_euclid(dn) != 0 {
            not_derived.push(format!("{flag}({tot} ÷ completed 가 나누어떨어지지 않거나 없음 — 파생 ✗)"));
            continue;
        }
        let per = tv.div_euclid(dn);
        let mut why = format!("derived({tot} {tv} ÷ completed {dn} — 요청당 평균 · 균일 길이는 관측 아님)");
        let il = if key == "random_input_len" { as_int(&idx["input_len"])? } else { None };
        if let Some(il) = il {
            if il != per {
                not_derived.push(format!("{flag}(파생 {per} ≠ sweep_index.input_len {il} — 고르지 않는다)"));
                continue;
            }
            why += &format!(" · sweep_index.input_len={il} 일치");
        }
        add(flag, J::Int(per), tot, why);
    }
    let present: Vec<String> = fields.iter().map(|f| f["flag"].display()).collect();
    let mut unr: Vec<String> = _BENCH_UNRECORDED.iter().filter(|f| !present.contains(&f.to_string())).map(|s| s.to_string()).collect();
    unr.extend(not_derived);
    Ok(jd! {"command" => format!("vllm bench serve {}", args.join(" ")), "command_source" => BENCH_COMMAND_SOURCE,
        "source" => format!("{BENCH_COMMAND_SOURCE} — {rel}({at} · 결과 JSON 필드만 · 원문 명령 아님)"),
        "fields" => J::List(fields), "unrecorded" => J::from(unr)})
}

pub const REL_BENCH_TOOLS_DIR: &str = ".claude/skills/adversarial-benchmark/scripts";
/// hintlib/evidence.py:3310 _BENCH_TOOLS (python literal as JSON)
pub fn _BENCH_TOOLS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("[[\"sweep_bench.sh\", \"sweep\"], [\"run_bench.sh\", \"bench\"], [\"lite_bench.sh\", \"lite\"]]").expect("constant"))
}
pub const _SDIR_CALL: &str = "\"$SDIR/{}\"";
/// hintlib/evidence.py:3313 _BENCH_TOOL_ARG (python literal as JSON)
pub fn _BENCH_TOOL_ARG() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"vllm-bench-serve\": \"vllm\", \"guidellm\": \"guidellm\"}").expect("constant"))
}
crate::re_static!(_TOOL_ARG_RE, r#"--tool\s+"?([A-Za-z][A-Za-z0-9_-]*)"?"#);
crate::re_static!(_REFLOG_LINE_RE, r"^([0-9a-f]{40}) ([0-9a-f]{40}) .* (\d+) [+-]\d{4}(?:\t|$)");

/// `_ENV_ECHO_RES` — (kind, pattern).
pub fn _ENV_ECHO_RES() -> &'static [(&'static str, Re)] {
    static C: OnceLock<Vec<(&'static str, Re)>> = OnceLock::new();
    C.get_or_init(|| {
        vec![
            ("env-echo", Re::new(r"\bNCCL INFO (NCCL_[A-Z0-9_]+) set by environment to (\S+)", 0)),
            ("param-echo", Re::new(r"\bNCCL INFO (NCCL_[A-Z0-9_]+) set to (\S+)", 0)),
        ]
    })
}
/// `_ENV_RUNTIME_RES` — (key, pattern).
pub fn _ENV_RUNTIME_RES() -> &'static [(&'static str, Re)] {
    static C: OnceLock<Vec<(&'static str, Re)>> = OnceLock::new();
    C.get_or_init(|| {
        vec![
            ("nccl:network", Re::new(r"\bNCCL INFO Using network (\S+)", 0)),
            ("nccl:net_ib_devices", Re::new(r"\bNCCL INFO NET/IB : Using (.+?)\s*$", 0)),
            ("nccl:version", Re::new(r"\bNCCL INFO NCCL version (\S+)", 0)),
        ]
    })
}
crate::re_static!(_RAY_DEDUP_RE, r"\s*\[repeated (\d+)x across cluster\].*$");
crate::re_static!(_NCCL_HOST_RE, r"(?:^|[\s)])([A-Za-z0-9][A-Za-z0-9.-]*):\d+:\d+ \[\d+\] NCCL INFO");
crate::re_static!(_RAY_IP_RE, r"\bip=(\d{1,3}(?:\.\d{1,3}){3})\)");
crate::re_static!(_NODE_PH_RE, r"<node:([A-Za-z0-9_-]+)>");
/// `_BANNER_RES` — vLLM start banners.
pub fn _BANNER_RES() -> &'static [Re] {
    static C: OnceLock<Vec<Re>> = OnceLock::new();
    C.get_or_init(|| {
        vec![
            Re::new(r"█\s+version\s+(\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)", 0),
            Re::new(r"vLLM API server version (\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)", 0),
            Re::new(r"Initializing a V1 LLM engine \(v(\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)\)", 0),
        ]
    })
}
/// `_CERT_VLLM_MARKS` — (name, pattern) in dict order.
pub fn _CERT_VLLM_MARKS() -> &'static [(&'static str, Re)] {
    static C: OnceLock<Vec<(&'static str, Re)>> = OnceLock::new();
    C.get_or_init(|| {
        vec![
            ("image_rule", Re::new(r#"re\.search\(r"(easy-vllm:[^"]+)",\s*_img\)"#, 0)),
            ("image_src", Re::new(r#"_img\s*=\s*grep_env\(envtext,\s*"IMAGE_TAG"\)"#, 0)),
            ("env_override", Re::new(r#"os\.environ\.get\("VLLM_VER"\)"#, 0)),
            ("banner", Re::new(r#"vllm_build\s*=\s*_m\.group\(1\)"#, 0)),
        ]
    })
}
pub const _WORKTREE_NOTE: &str = "측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다";

pub fn _utc_of_epoch(ep: i64) -> HR<String> {
    Ok(fmt_epoch(ep))
}

pub fn _git_repo(repo: &Path) -> HR<bool> {
    Ok(repo.join(".git").exists())
}

fn git_nc(repo: &Path, args: &[&str]) -> HR<core::Proc> {
    core::git_nocheck(repo, args)
}

pub fn _head_at(repo: &Path, utc: Option<&str>) -> HR<(Option<String>, String)> {
    let Some(ep) = utc_epoch_str(utc) else { return Ok((None, "측정 시각 미관측 — 판본을 고르지 않는다".into())) };
    let utc = utc.unwrap_or_default();
    if !_git_repo(repo)? {
        return Ok((None, "git 저장소가 아니다 — 측정 시점 판본 미관측".into()));
    }
    let lp = py_strip(&git_nc(repo, &["rev-parse", "--git-path", "logs/HEAD"])?.stdout).to_string();
    let has_head_log = !lp.is_empty() && repo.join(&lp).is_file();
    let mut entries: Vec<(i64, usize, String)> = Vec::new();
    if has_head_log {
        let raw = read_lossy(&repo.join(&lp)).unwrap_or_default();
        for (i, ln) in raw.split('\n').enumerate() {
            if let Some(m) = _REFLOG_LINE_RE().match_at_start(ln) {
                if let Ok(t) = pyre::group(&m, 3).unwrap_or_default().parse::<i64>() {
                    entries.push((t, i, pyre::group(&m, 2).unwrap_or_default().to_string()));
                }
            }
        }
    }
    let before: Vec<&(i64, usize, String)> = entries.iter().filter(|e| e.0 <= ep).collect();
    if let Some((ts, _i, sha)) = before.iter().max().map(|e| (*e).clone()) {
        if git_nc(repo, &["cat-file", "-e", &format!("{sha}^{{commit}}")])?.returncode == 0 {
            return Ok((Some(sha), format!("reflog HEAD@{{{ts}}}({}) = 측정 시각 {utc} 직전 체크아웃", fmt_epoch(ts))));
        }
    }
    let why = if !before.is_empty() {
        format!("reflog HEAD@{{{}}} 의 커밋 오브젝트 부재(gc 등)", before.iter().map(|e| e.0).max().unwrap())
    } else if !entries.is_empty() {
        format!("reflog 가 측정 시각을 덮지 않는다(최초 {})", fmt_epoch(entries.iter().map(|e| e.0).min().unwrap()))
    } else {
        "reflog 없음".to_string()
    };
    let out = git_nc(repo, &["-c", "log.showSignature=false", "log", "-1", &format!("--before={utc}"), "--format=%H", "HEAD"])?;
    let sha = if out.returncode == 0 { py_strip(&out.stdout).to_string() } else { String::new() };
    if re_sha40().is_full(&sha) {
        return Ok((Some(sha), format!("HEAD 이력의 committer date ≤ {utc}({why} — 측정 때 체크아웃 브랜치는 미검증)")));
    }
    Ok((None, format!("측정 시각 이전 커밋 없음({why})")))
}

/// `_git_text` bytes (`git show <rev>:<path>` · failure = None).
pub fn _git_text_bytes(repo: &Path, rev: &str, rel_path: &str) -> HR<Option<Vec<u8>>> {
    core::git_bytes(repo, &["show", &format!("{rev}:{rel_path}")], false)
}

/// `_git_text` — the stub form returns the bytes decoded (`errors="replace"`) as `J::Str`; `_git_text_bytes` keeps bytes.
pub fn _git_text(repo: &Path, rev: &str, rel_path: &str) -> HR<Option<J>> {
    Ok(_git_text_bytes(repo, rev, rel_path)?.map(|b| J::Str(String::from_utf8_lossy(&b).into_owned())))
}

pub fn _tool_revision(repo: &Path, at: &str, rel_path: &str) -> HR<Option<J>> {
    if git_nc(repo, &["cat-file", "-e", &format!("{at}:{rel_path}")])?.returncode != 0 {
        return Ok(None);
    }
    let out = git_nc(repo, &["-c", "log.showSignature=false", "log", "-1", "--format=%H %ct", at, "--", rel_path])?;
    let parts: Vec<String> = if out.returncode == 0 { out.stdout.split_whitespace().map(str::to_string).collect() } else { Vec::new() };
    if parts.len() != 2 || !re_sha40().is_full(&parts[0]) {
        return Ok(None);
    }
    let mut rev = parts[0].clone();
    let mut blob = Vec::new();
    for r in [rev.as_str(), at] {
        blob.push(py_strip(&git_nc(repo, &["rev-parse", "--verify", "--quiet", &format!("{r}:{rel_path}")])?.stdout).to_string());
    }
    let mut rev_note = J::Null;
    if blob[0].is_empty() || blob[0] != blob[1] {
        rev_note = J::from(format!(
            "경로 제한 log 의 마지막 변경 {} 의 blob 이 측정 시점 {} 와 달라 측정 시점 커밋을 쓴다",
            core::head_chars(&rev, 12),
            core::head_chars(at, 12)
        ));
        rev = at.to_string();
    }
    let ep = |s: &str| -> HR<i64> { s.parse::<i64>().map_err(|_| HintError::py("ValueError", format!("invalid literal for int() with base 10: {}", crate::value::py_repr(s)))) };
    let mut row = jd! {"git_rev" => rev.clone(), "git_rev_utc" => if rev == parts[0] { J::from(fmt_epoch(ep(&parts[1])?)) } else { J::Null },
        "git_rev_note" => rev_note, "next_rev" => J::Null, "next_rev_utc" => J::Null};
    if rev != parts[0] {
        let ct = py_strip(&git_nc(repo, &["-c", "log.showSignature=false", "log", "-1", "--format=%ct", at])?.stdout).to_string();
        row.set("git_rev_utc", if !ct.is_empty() && ct.chars().all(|c| c.is_ascii_digit()) { J::from(fmt_epoch(ep(&ct)?)) } else { J::Null });
    }
    let at12 = core::head_chars(at, 12);
    if git_nc(repo, &["merge-base", "--is-ancestor", at, "HEAD"])?.returncode == 0 {
        let nx = git_nc(repo, &["-c", "log.showSignature=false", "log", "--reverse", "--format=%H %ct", &format!("{at}..HEAD"), "--", rel_path])?;
        let first: Vec<String> = if nx.returncode == 0 {
            nx.stdout.lines().next().unwrap_or("").split_whitespace().map(str::to_string).collect()
        } else {
            Vec::new()
        };
        if first.len() == 2 {
            row.set("next_rev", first[0].clone());
            row.set("next_rev_utc", fmt_epoch(ep(&first[1])?));
            row.set("next_rev_basis", format!("{at12}..HEAD 경로 이력의 첫 변경"));
        } else if nx.returncode == 0 {
            row.set("next_rev_basis", format!("{at12}..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트)"));
        } else {
            row.set("next_rev_basis", "미관측(git log 실패)");
        }
    } else {
        row.set(
            "next_rev_basis",
            format!("미관측 — 지금 HEAD 가 측정 시점 커밋 {at12} 의 후손이 아니다(다른 브랜치에서 발행 · 다음 변경을 말하지 않는다 — '변경 없음' 이 아니다)"),
        );
    }
    Ok(Some(row))
}

pub fn _measured_bench_tool(ev: &J) -> HR<Option<String>> {
    let a = norm_value(&ev["sweep"]["index"]["meta"]["bench_tool"])?;
    if a.truthy() {
        return Ok(Some(a.display()));
    }
    let b = norm_value(&ev["certificate"]["bench_tool"])?;
    Ok(if b.truthy() { Some(b.display()) } else { None })
}

pub fn _tool_selection(repo: &Path, ev: &J) -> HR<J> {
    let measured = _measured_key(repo, ev)?;
    let (at, method) = _head_at(repo, measured.as_deref())?;
    let mut tools: Vec<J> = Vec::new();
    let mut out = jd! {"at" => J::from(at.clone()), "at_method" => method, "measured_utc" => J::from(measured.clone()), "tools" => J::list(), "driver_note" => J::Null};
    let Some(at) = at else { return Ok(out) };
    let sw = &ev["sweep"];
    let idx = &sw["index"];
    let sweep_rel = format!("{REL_BENCH_TOOLS_DIR}/sweep_bench.sh");
    let sweep_text = if sw.truthy() {
        String::from_utf8_lossy(&_git_text_bytes(repo, &at, &sweep_rel)?.unwrap_or_default()).into_owned()
    } else {
        String::new()
    };
    let call = |n: &str| _SDIR_CALL.replace("{}", n);
    if sw.truthy() && !sweep_text.is_empty() {
        tools.push(jd! {"name" => "sweep_bench.sh", "repo_path" => sweep_rel.clone(), "role" => "sweep",
            "why" => format!("스윕 색인 조립자({}/sweep_index.json · generated_utc = measured_utc 조인)", sw.get("dir").cloned().unwrap_or(J::Null).display())});
        if sw["levels"].truthy() && sweep_text.contains(&call("run_bench.sh")) {
            tools.push(jd! {"name" => "run_bench.sh", "repo_path" => format!("{REL_BENCH_TOOLS_DIR}/run_bench.sh"), "role" => "bench",
                "why" => "그 판본 sweep_bench.sh 가 레벨마다 부른다(`\"$SDIR/run_bench.sh\"`) · 색인에 레벨 기록"});
        }
        if idx["lite"].is_dict() && sweep_text.contains(&call("lite_bench.sh")) {
            tools.push(jd! {"name" => "lite_bench.sh", "repo_path" => format!("{REL_BENCH_TOOLS_DIR}/lite_bench.sh"), "role" => "lite",
                "why" => "그 판본 sweep_bench.sh 가 lite 레그로 부른다(`\"$SDIR/lite_bench.sh\"`) · 색인에 lite 기록"});
        }
    } else if !sw.truthy() && ev["report_kind"].as_str() == Some("lite") {
        tools.push(jd! {"name" => "lite_bench.sh", "repo_path" => format!("{REL_BENCH_TOOLS_DIR}/lite_bench.sh"), "role" => "lite",
            "why" => "경량 리포트 셀(lite_bench.sh 가 리포트를 발행한다)"});
    }
    if sweep_text.is_empty() {
        out.set("tools", J::List(tools));
        return Ok(out);
    }
    let ls = git_nc(repo, &["ls-tree", "--name-only", &at, &format!("{REL_BENCH_TOOLS_DIR}/")])?;
    let mut names: Vec<String> = if ls.returncode == 0 {
        py_splitlines(&ls.stdout).iter().map(|p| p.rsplit('/').next().unwrap_or("").to_string()).collect()
    } else {
        Vec::new()
    };
    names.sort();
    let mbt = _measured_bench_tool(ev)?;
    let want = _BENCH_TOOL_ARG().get(mbt.as_deref().unwrap_or("")).and_then(J::as_str).map(str::to_string);
    let fixed: Vec<String> = _BENCH_TOOLS().items().iter().map(|x| x.at(0).display()).collect();
    let mut notes: Vec<String> = Vec::new();
    let mut n_cand = 0;
    for nm in names {
        if !nm.ends_with(".sh") || fixed.contains(&nm) {
            continue;
        }
        let text = String::from_utf8_lossy(&_git_text_bytes(repo, &at, &format!("{REL_BENCH_TOOLS_DIR}/{nm}"))?.unwrap_or_default()).into_owned();
        if !text.contains(&call("sweep_bench.sh")) {
            continue;
        }
        let mut consts: BTreeSet<String> = BTreeSet::new();
        for ln in py_splitlines(&text) {
            if ln.trim_start_matches(py_isspace).starts_with('#') {
                continue;
            }
            for m in _TOOL_ARG_RE().finditer(&ln) {
                consts.insert(pyre::group(&m, 1).unwrap_or_default().to_string());
            }
        }
        let consts: Vec<String> = consts.into_iter().collect();
        if let Some(w) = want.as_deref().filter(|w| !w.is_empty()) {
            if consts.len() == 1 && consts[0] != w {
                notes.push(format!(
                    "{nm} 배제(그 판본은 sweep_bench 에 --tool {} 를 넘긴다 ≠ 이 측정 bench_tool {} → --tool {w})",
                    consts[0],
                    opt_disp(&mbt)
                ));
                continue;
            }
        }
        let ctxt = if consts.is_empty() { "없음".to_string() } else { J::from(consts.clone()).repr() };
        tools.push(jd! {"name" => nm.clone(), "repo_path" => format!("{REL_BENCH_TOOLS_DIR}/{nm}"), "role" => "driver-candidate",
            "why" => format!("그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 {ctxt} 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다")});
        notes.push(format!("{nm} 후보(미검증)"));
        n_cand += 1;
    }
    let scope = format!("그 판본의 {REL_BENCH_TOOLS_DIR}/");
    let outside = "저장소 밖 호출자(세션 스크래치 스크립트 등)·미커밋 스크립트는 관측 대상 밖";
    let mut note = if !notes.is_empty() {
        format!("{scope} 안에서 sweep_bench.sh 를 부르는 드라이버: {}", notes.join(" · "))
    } else {
        format!("{scope} 안에 sweep_bench.sh 를 부르는 스크립트가 없다 — 드라이버 미관측({outside})")
    };
    if !notes.is_empty() && n_cand == 0 {
        note += &format!(" → 이 디렉터리의 드라이버는 이 측정을 부르지 않았다(배제로 추론 · 호출 기록 아님) — sweep_bench.sh 를 직접 또는 저장소 밖 호출자가 불렀다({outside})");
    }
    out.set("tools", J::List(tools));
    out.set("driver_note", note);
    Ok(out)
}

/// `tool_snapshots` — the stub form (the `ev.lineage_seeds` update is applied to a copy; use `tool_snapshots_mut`).
pub fn tool_snapshots(repo: &Path, ev: &J, draft_dir: &Path) -> HR<Vec<J>> {
    let mut e = ev.clone();
    tool_snapshots_mut(repo, &mut e, Some(draft_dir))
}

/// `tool_snapshots(repo, ev, draft_dir)` with the in-place `ev.lineage_seeds['evidence_candidates']` update.
pub fn tool_snapshots_mut(repo: &Path, ev: &mut J, draft_dir: Option<&Path>) -> HR<Vec<J>> {
    let repo = repo_(repo)?;
    let repo = repo.as_path();
    let Some(draft) = draft_dir else {
        return core::fail("HINT_DRAFT_REQUIRED", "tool_snapshots 는 draft 디렉터리가 필요하다(스냅숏은 draft/inputs/sources 에만 쓴다).", None);
    };
    let sel = _tool_selection(repo, ev)?;
    let at = sel["at"].display();
    let mut rows: Vec<J> = Vec::new();
    for t in sel["tools"].items() {
        let rp = t["repo_path"].display();
        let Some(rv) = _tool_revision(repo, &at, &rp)? else { continue };
        let Some(data) = _git_text_bytes(repo, &at, &rp)? else { continue };
        let rel_path = crate::lineage::tool_snapshot_rel(&t["name"].display(), &rv["git_rev"].display())?;
        let dest = draft.join(&rel_path);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).map_err(|e| core::io_err(&e, p))?;
        }
        std::fs::write(&dest, &data).map_err(|e| core::io_err(&e, &dest))?;
        rows.push(jd! {"name" => t["name"].clone(), "repo_path" => rp, "git_rev" => rv["git_rev"].clone(), "snapshot_rel" => rel_path,
            "role" => t["role"].clone(), "why" => t["why"].clone(), "head_at_measurement" => sel["at"].clone(), "rev_method" => sel["at_method"].clone(),
            "git_rev_utc" => rv["git_rev_utc"].clone(), "next_rev" => rv["next_rev"].clone(), "next_rev_utc" => rv["next_rev_utc"].clone(),
            "next_rev_basis" => rv["next_rev_basis"].clone(), "git_rev_note" => rv["git_rev_note"].clone(), "worktree_note" => _WORKTREE_NOTE,
            "driver_note" => if t["role"].as_str() == Some("sweep") { sel["driver_note"].clone() } else { J::Null }});
    }
    let mut cands: Vec<J> = ev["lineage_seeds"]["evidence_candidates"].items().iter().filter(|c| c.is_dict()).cloned().collect();
    let have: Vec<J> = cands.iter().map(|c| c.get("path").cloned().unwrap_or(J::Null)).collect();
    for r in &rows {
        if !have.contains(&r["snapshot_rel"]) {
            cands.push(jd! {"path" => r["snapshot_rel"].clone(), "kind" => crate::lineage::TOOL_SOURCE_KIND,
                "origin" => format!("git:{}:{}", r["git_rev"].display(), r["repo_path"].display())});
        }
    }
    let mut seeds = ev["lineage_seeds"].as_dict().cloned().unwrap_or_default();
    seeds.insert("evidence_candidates", J::List(cands));
    if let Some(m) = ev.as_dict_mut() {
        m.insert("lineage_seeds", J::Dict(seeds));
    }
    Ok(rows)
}

fn walk_named(dir: &Path, name: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        if name_of(&p) == name {
            out.push(p.clone());
        }
        if !is_symlink(&p) && p.is_dir() {
            walk_named(&p, name, out);
        }
    }
}

pub fn _sweep_engine_logs(base: &Path, idx: &J, levels: &J, cell: &str) -> HR<Vec<PathBuf>> {
    let mut logs = Vec::new();
    if idx["lite"].is_dict() && base.join(format!("lite_engine_{cell}.log")).is_file() {
        let head = base.join(format!("lite_engine_head_{cell}.log"));
        if head.is_file() && nonempty(&head)? {
            logs.push(head);
        }
        logs.push(base.join(format!("lite_engine_{cell}.log")));
    }
    let mut lv: BTreeSet<i64> = BTreeSet::new();
    for x in levels.items().iter().filter(|x| x.is_dict()) {
        if let Some(n) = as_int(&x["level"])?.filter(|n| *n != 0) {
            lv.insert(n);
        }
    }
    for n in lv {
        let mut found = Vec::new();
        walk_named(&base.join(format!("level_{n:02}")), &format!("engine_{cell}.log"), &mut found);
        found.retain(|p| p.is_file());
        found.sort();
        logs.extend(found);
    }
    Ok(logs)
}

pub fn _measured_engine_logs(repo: &Path, ev: &J) -> HR<(Vec<PathBuf>, String)> {
    let (logs, why, _copies) = _measured_engine_logs_ex(repo, ev)?;
    Ok((logs, why))
}
