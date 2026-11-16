//! evidence part 3 — port of `scripts/hintlib/evidence.py` lines 3567–5312: full copies of the same run, engine-log
//! observation (env echo · banners · tail/head windows), vLLM observation labels, attestation scope / same-boot judgement,
//! lite measurement window, and the two evidence resolvers (`from_campaign` · `from_publication`) with their common tail
//! `_assemble`. A `CellEvidence` is a `J::Dict` (dataclass fields in declaration order — see `new_cell_evidence`).
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::{self, Civil, HintError, HR};
use crate::pyre::Re;
use crate::value::{py_repr, Map, J};
use crate::{jd, jl, re_static};
use super::*;

// ── private helpers (not in Python) ─────────────────────────────────────────────────────────────────────────────
macro_rules! prx {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Re {
            static C: OnceLock<Re> = OnceLock::new();
            C.get_or_init(|| Re::new($pat, 0))
        }
    };
}
// Same-module patterns owned by other parts (e1/e2 ranges) — private copies of the identical Python literals.
prx!(rx_ansi, r"\x1b\[[0-9;]*[A-Za-z]");
prx!(rx_engine_ts, r"\b(?:INFO|WARNING|ERROR|DEBUG|CRITICAL)\s+(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\b");
prx!(rx_utc, r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z");
prx!(rx_sha40, r"^[0-9a-f]{40}$");
prx!(rx_cell, r"^[A-Za-z0-9][A-Za-z0-9._-]*$");
prx!(rx_simlog_ref, r#"docs/simlog/([^\s`'"()<>|/]+)/"#);
prx!(rx_ray_dedup, r"\s*\[repeated (\d+)x across cluster\].*$");
prx!(rx_nccl_host, r"(?:^|[\s)])([A-Za-z0-9][A-Za-z0-9.-]*):\d+:\d+ \[\d+\] NCCL INFO");
prx!(rx_ray_ip, r"\bip=(\d{1,3}(?:\.\d{1,3}){3})\)");
prx!(rx_node_ph, r"<node:([A-Za-z0-9_-]+)>");
prx!(rx_env_echo, r"\bNCCL INFO (NCCL_[A-Z0-9_]+) set by environment to (\S+)");
prx!(rx_param_echo, r"\bNCCL INFO (NCCL_[A-Z0-9_]+) set to (\S+)");
prx!(rx_rt_network, r"\bNCCL INFO Using network (\S+)");
prx!(rx_rt_ib, r"\bNCCL INFO NET/IB : Using (.+?)\s*$");
prx!(rx_rt_version, r"\bNCCL INFO NCCL version (\S+)");
prx!(rx_banner1, r"█\s+version\s+(\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)");
prx!(rx_banner2, r"vLLM API server version (\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)");
prx!(rx_banner3, r"Initializing a V1 LLM engine \(v(\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)\)");
prx!(rx_mark_image_rule, r#"re\.search\(r"(easy-vllm:[^"]+)",\s*_img\)"#);
prx!(rx_mark_image_src, r#"_img\s*=\s*grep_env\(envtext,\s*"IMAGE_TAG"\)"#);
prx!(rx_mark_env_override, r#"os\.environ\.get\("VLLM_VER"\)"#);
prx!(rx_mark_banner, r#"vllm_build\s*=\s*_m\.group\(1\)"#);
prx!(rx_mmdd_utc, r"\d{4}-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z");
prx!(rx_mmdd_hms, r"(\d{2})-(\d{2}) (\d{2}):(\d{2}):(\d{2})");
prx!(rx_neg_int_dot, r"-?\d+\.");
prx!(rx_xyz, r"([0-9]+\.[0-9]+\.[0-9]+)");
prx!(rx_vllm_pin, r"^vllm==(\S+)\s*$");
prx!(rx_measured_utc_line, r"\s*MEASURED_UTC=");
prx!(rx_compose_project, r"vllm_(.+)_project");
prx!(rx_lite_sub, r"_\d{8}T\d{6}Z");
prx!(rx_utc_parse, r"^(\d{4})-(\d{1,2})-(\d{1,2})T(\d{1,2}):(\d{1,2}):(\d{1,2})Z$");
prx!(rx_ymd_hms, r"^(\d{4})-(\d{1,2})-(\d{1,2}) (\d{1,2}):(\d{1,2}):(\d{1,2})$");

const UNOBSERVED: [&str; 7] = ["", "n/a", "na", "null", "none", "unknown", "?"];

/// `_unobserved` (same rule as e1's — private copy, pure).
fn unobs(v: &J) -> bool {
    match v {
        J::Null => true,
        J::Str(s) => UNOBSERVED.contains(&s.trim().to_lowercase().as_str()),
        _ => false,
    }
}

/// `_norm_value`.
fn norm(v: &J) -> J {
    if unobs(v) {
        return J::Null;
    }
    match v {
        J::Str(s) => J::Str(s.trim().to_string()),
        o => o.clone(),
    }
}

/// `_as_int`.
fn as_int(v: &J) -> Option<i64> {
    match v {
        J::Bool(_) => None,
        J::Int(i) => Some(*i),
        J::Str(s) => {
            let t = s.trim();
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
                t.parse().ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

fn opt_str(v: &J) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// `a or b` on J values.
fn or<'a>(a: &'a J, b: &'a J) -> &'a J {
    if a.truthy() {
        a
    } else {
        b
    }
}

/// `str(d.get(k, ""))`.
fn pystr_get(d: &J, k: &str) -> String {
    match d.get(k) {
        Some(v) => v.display(),
        None => String::new(),
    }
}

fn repr_opt(o: &Option<String>) -> String {
    match o {
        Some(s) => py_repr(s),
        None => "None".to_string(),
    }
}

fn disp_opt(o: &Option<String>) -> String {
    o.clone().unwrap_or_else(|| "None".to_string())
}

fn rel(repo: &Path, p: &Path) -> HR<String> {
    core::rel(repo, p)
}

fn pname(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn has_dotdot(s: &str) -> bool {
    s.split('/').any(|c| c == "..")
}

fn is_symlink(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

fn ev_s(ev: &J, k: &str) -> String {
    ev.get_str(k).unwrap_or_default().to_string()
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// `datetime(y, mo, d, h, mi, s)` (ValueError → None).
fn mk_civil(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> Option<Civil> {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || !(0..24).contains(&h)
        || !(0..60).contains(&mi) || !(0..62).contains(&s) || s > 59
    {
        return None;
    }
    Some(Civil { year: y, month: mo as u32, day: d as u32, hour: h as u32, minute: mi as u32, second: s as u32 })
}

fn caps6(re: &Re, s: &str) -> Option<Civil> {
    let c = re.fullmatch(s)?;
    let n = |i: usize| -> i64 { c.get(i).map(|m| m.as_str().parse().unwrap_or(-1)).unwrap_or(-1) };
    mk_civil(n(1), n(2), n(3), n(4), n(5), n(6))
}

/// `_utc_dt(s)` — `strptime(str(s), "%Y-%m-%dT%H:%M:%SZ")`.
fn utc_dt(s: Option<&str>) -> Option<Civil> {
    caps6(rx_utc_parse(), s?)
}

fn iso_z(c: &Civil) -> String {
    format!("{}Z", c.iso())
}

/// `_utc_of_epoch`.
fn utc_of_epoch(ep: i64) -> String {
    iso_z(&Civil::from_epoch(ep))
}

fn mtime_epoch(p: &Path) -> Option<i64> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    match m.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Some(d.as_secs() as i64),
        Err(e) => Some(-(e.duration().as_secs_f64().ceil() as i64)),
    }
}

fn os_type(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(2) => "FileNotFoundError",
        Some(13) | Some(1) => "PermissionError",
        Some(21) => "IsADirectoryError",
        Some(20) => "NotADirectoryError",
        _ => "OSError",
    }
}

fn py_io(e: &std::io::Error, p: &Path) -> HintError {
    HintError::py(os_type(e), core::io_err_text(e, p))
}

/// `str.splitlines()`.
fn splitlines(s: &str) -> Vec<String> {
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
            '\n' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
                out.push(std::mem::take(&mut cur))
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Entries of `dir` whose name satisfies `pred` (non-recursive glob), sorted.
fn glob_dir(dir: &Path, pred: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| pred(&e.file_name().to_string_lossy()))
            .map(|e| dir.join(e.file_name()))
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// `fnmatch`-style `*<infix>*<suffix>` name test (`*vllm-serve*.log`, `*serve*.log`).
fn infix_suffix(name: &str, infix: &str, suffix: &str) -> bool {
    name.len() >= suffix.len() && name.ends_with(suffix) && name[..name.len() - suffix.len()].contains(infix)
}

/// `Path.rglob(name)` (does not descend into symlinked directories).
fn rglob(dir: &Path, name: &str, out: &mut BTreeSet<PathBuf>) {
    if dir.join(name).exists() || is_symlink(&dir.join(name)) {
        out.insert(dir.join(name));
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut subs: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| dir.join(e.file_name()))
            .collect();
        subs.sort();
        for s in subs {
            rglob(&s, name, out);
        }
    }
}

fn ts_of(c: &crate::pyre::Captures<'_>) -> String {
    let g = |i: usize| c.get(i).map(|m| m.as_str()).unwrap_or("");
    format!("{}-{} {}:{}:{}", g(1), g(2), g(3), g(4), g(5))
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `_read_json_soft` with a typed result.
fn read_json_soft(path: &Path) -> HR<(Option<J>, Option<String>)> {
    match _read_json_opt(path, &pname(path)) {
        Ok(J::Null) => Ok((None, None)),
        Ok(v) => Ok((Some(v), None)),
        Err(e) if !e.is_py() => Ok((None, Some(format!("{} 판독 불가({})", pname(path), e.code)))),
        Err(e) => Err(e),
    }
}

fn git_text(repo: &Path, rev: &str, rel_path: &str) -> HR<Option<Vec<u8>>> {
    core::git_bytes(repo, &["show", &format!("{rev}:{rel_path}")], false)
}

fn files_j(files: &[(String, String)]) -> J {
    J::List(files.iter().map(|(a, b)| jl![a.as_str(), b.as_str()]).collect())
}

fn files_of(j: &J) -> Vec<(String, String)> {
    j.items().iter().map(|x| (x.at(0).display(), x.at(1).display())).collect()
}

/// A new `CellEvidence` (e1 `new_cell_evidence` defaults; `set` overrides by field name).
fn make_ev(mode: &str, campaign_id: Option<&str>, cell: &str, node: &str, topology: &str, set: Vec<(&str, J)>) -> J {
    let mut ev = new_cell_evidence(mode, campaign_id, cell, node, topology);
    for (k, v) in set {
        ev.set(k, v);
    }
    ev
}

fn sources_set(ev: &mut J, k: &str, v: impl Into<J>) {
    if !ev["sources"].is_dict() {
        ev.set("sources", J::dict());
    }
    if let Some(s) = ev.get_mut("sources") {
        s.set(k, v);
    }
}

fn bi_set(ev: &mut J, k: &str, v: impl Into<J>) {
    if let Some(b) = ev.get_mut("build_identity") {
        b.set(k, v);
    }
}

fn bi_src_set(ev: &mut J, k: &str, v: impl Into<J>) {
    if let Some(b) = ev.get_mut("build_identity") {
        if let Some(s) = b.get_mut("source") {
            s.set(k, v);
        }
    }
}

// OWNER-MIRROR: .claude/skills/terraforming_node/scripts/campaign_template_validator.py::assigned_cells
fn assigned_cells(doc: &J, node: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(a) = doc.get("assignments").and_then(J::as_dict) {
        for (nid, items) in a.iter() {
            if nid != node {
                continue;
            }
            for item in items.items() {
                if let Some(cell) = item.get_str("cell") {
                    if !cell.is_empty() && !cell.contains("<<FILL>>") && !out.iter().any(|c| c == cell) {
                        out.push(cell.to_string());
                    }
                }
            }
        }
    }
    out
}

// OWNER-MIRROR: .claude/skills/terraforming_node/scripts/campaign_template_validator.py::resolve_measurement_node
//   (+ DERIVED_MEASUREMENT_NODES = ("cluster",))
fn resolve_measurement_node(tag: &str, declared: &[String]) -> (Vec<String>, &'static str) {
    if declared.iter().any(|d| d == tag) {
        return (vec![tag.to_string()], "declared");
    }
    if tag == "cluster" {
        return (declared.to_vec(), "derived");
    }
    (Vec::new(), "unknown")
}

// OWNER-MIRROR: .claude/policies/runtime/completion_gate.py::certificate_run_key (CERTIFICATE_RUN_KEY_FIELDS)
fn certificate_run_key(cert: &J) -> Option<Vec<String>> {
    let mut vals = Vec::new();
    for k in ["model", "gpu_model", "vllm_version", "quantization", "topology", "tensor_parallel_size", "measured_utc"] {
        let v = cert.get(k).unwrap_or(&J::Null);
        if v.is_null() || v.display().trim().is_empty() {
            return None;
        }
        vals.push(v.display().trim().to_string());
    }
    Some(vals)
}

// ── 같은 실행의 전체 사본(G1) ───────────────────────────────────────────────────────────────────────────────────
re_static!(_ENGINE_CORE_PID, r"\(EngineCore pid=(\d+)\)");
re_static!(_LOAD_WEIGHTS, r"(\d{2}-\d{2} \d{2}:\d{2}:\d{2})\s+\[[^\]]*\]\s+Loading weights took ([0-9.]+) seconds");
pub const SAME_RUN_RULE: &str = "EngineCore pid 집합 일치 ∧ 가중치 로드 줄(시각·초) 공유 ∧ 사본 첫 시각 ≤ 꼬리 첫 시각(∧ ≤ 측정 시작)";

struct RunMarks {
    pids: BTreeSet<String>,
    loads: BTreeSet<(String, String)>,
    first_ts: Option<String>,
}

fn run_marks(path: &Path) -> HR<RunMarks> {
    let mut m = RunMarks { pids: BTreeSet::new(), loads: BTreeSet::new(), first_ts: None };
    for line in _log_lines(path)? {
        let clean = rx_ansi().sub_literal(&line, "");
        m.pids.extend(_ENGINE_CORE_PID().findall(&clean));
        if let Some(c) = _LOAD_WEIGHTS().search(&clean) {
            m.loads.insert((c[1].to_string(), c[2].to_string()));
        }
        if m.first_ts.is_none() {
            if let Some(tm) = rx_engine_ts().search(&clean) {
                m.first_ts = Some(ts_of(&tm));
            }
        }
    }
    Ok(m)
}

/// 엔진 로그의 실행 표지 — {pids, loads, first_ts} (sets as sorted lists).
pub fn _run_marks(path: &Path) -> HR<J> {
    let m = run_marks(path)?;
    Ok(jd! {
        "pids" => J::from(m.pids.into_iter().collect::<Vec<String>>()),
        "loads" => J::List(m.loads.into_iter().map(|(a, b)| jl![a, b]).collect()),
        "first_ts" => m.first_ts,
    })
}

fn same_run_candidates(repo: &Path, ev: &J, have: &[PathBuf]) -> HR<Vec<PathBuf>> {
    let base = repo.join("output").join(ev_s(ev, "topology")).join("benchlog");
    let suffix = format!("_{}.log", ev_s(ev, "cell"));
    let mut out: Vec<PathBuf> =
        if base.is_dir() { glob_dir(&base, |n| n.ends_with(&suffix)).into_iter().filter(|p| p.is_file()).collect() } else { Vec::new() };
    for c in ev["lineage_seeds"]["evidence_candidates"].items() {
        if c.get_str("kind") != Some("engine_log") {
            continue;
        }
        if let Some(r) = c.get_str("path") {
            if !r.is_empty() && !r.starts_with('/') && !has_dotdot(r) {
                out.push(repo.join(r));
            }
        }
    }
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut res = Vec::new();
    for p in out {
        if seen.contains(&p) || have.contains(&p) || !p.is_file() || is_symlink(&p) || !nonempty(&p) {
            continue;
        }
        seen.push(p.clone());
        let name = pname(&p);
        let parent = p.parent().map(pname).unwrap_or_default();
        if name.starts_with("lite_engine_") || name.contains("watchdog") || parent.starts_with("level_") || parent.starts_with("run_") {
            continue;
        }
        res.push(p);
    }
    Ok(res)
}

/// 전체 사본 후보 (`have` = list of `{"__path__"}`/str).
pub fn _same_run_candidates(repo: &Path, ev: &J, have: &J) -> HR<Vec<PathBuf>> {
    let have: Vec<PathBuf> = have.items().iter().map(crate::api::decode_path).collect();
    same_run_candidates(repo, ev, &have)
}

pub fn _mmdd(utc: Option<&str>) -> HR<Option<String>> {
    Ok(rx_mmdd_utc().fullmatch(utc.unwrap_or("")).map(|m| format!("{}-{} {}:{}:{}", &m[1], &m[2], &m[3], &m[4], &m[5])))
}

fn same_run_copies(repo: &Path, ev: &J, tails: &[PathBuf]) -> HR<Vec<(PathBuf, String)>> {
    let mut tmarks = Vec::new();
    for t in tails {
        if nonempty(t) {
            let m = run_marks(t)?;
            if !m.pids.is_empty() {
                tmarks.push((t.clone(), m));
            }
        }
    }
    if tmarks.is_empty() {
        return Ok(Vec::new());
    }
    let start = match _measure_window(repo, ev) {
        Ok((s, _, _)) => _mmdd(s.as_deref())?,
        Err(e) if !e.is_py() => None,
        Err(e) => return Err(e),
    };
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    for cand in same_run_candidates(repo, ev, tails)? {
        let cm = run_marks(&cand)?;
        let cfirst = match &cm.first_ts {
            Some(f) if !cm.pids.is_empty() => f.clone(),
            _ => continue,
        };
        for (t, tm) in &tmarks {
            let shared: Vec<&(String, String)> = cm.loads.intersection(&tm.loads).collect();
            let tfirst = match &tm.first_ts {
                Some(f) => f.clone(),
                None => continue,
            };
            if cm.pids != tm.pids || shared.is_empty() || cfirst > tfirst {
                continue;
            }
            if let Some(s) = &start {
                if &cfirst > s {
                    continue;
                }
            }
            let pids = J::from(cm.pids.iter().cloned().collect::<Vec<String>>()).repr();
            let msg = format!(
                "같은 실행의 전체 사본 — 판별: EngineCore pid {pids} 일치 · 가중치 로드 줄 `{} … {} seconds` 공유 · 사본 첫 시각 {cfirst} ≤ 꼬리 첫 시각 {tfirst}{}(꼬리 캡처 {} · 규칙 {SAME_RUN_RULE})",
                shared[0].0,
                shared[0].1,
                start.as_ref().map(|s| format!(" ≤ 측정 시작 {s}")).unwrap_or_default(),
                rel(repo, t)?
            );
            if let Some(e) = out.iter_mut().find(|(p, _)| p == &cand) {
                e.1 = msg;
            } else {
                out.push((cand.clone(), msg));
            }
            break;
        }
    }
    Ok(out)
}

/// 같은 실행 사본 판별 (`tails` = list of paths) → {path: 근거}.
pub fn _same_run_copies(repo: &Path, ev: &J, tails: &J) -> HR<J> {
    let tails: Vec<PathBuf> = tails.items().iter().map(crate::api::decode_path).collect();
    let mut m = Map::new();
    for (p, b) in same_run_copies(repo, ev, &tails)? {
        m.insert(core::posix(&p), b);
    }
    Ok(J::Dict(m))
}

/// `_measured_engine_logs_ex` with typed copies (ordered `(path, 근거)`).
pub fn measured_engine_logs_ex_t(repo: &Path, ev: &J) -> HR<(Vec<PathBuf>, String, Vec<(PathBuf, String)>)> {
    let (mut logs, mut why) = measured_engine_logs_base(repo, ev)?;
    if logs.is_empty() {
        return Ok((logs, why, Vec::new()));
    }
    let nat = native_preserved_engine_logs(repo, ev)?;
    let tails: Vec<PathBuf> = logs.iter().filter(|q| !nat.contains(q)).cloned().collect();
    let copies = same_run_copies(repo, ev, &tails)?;
    if !copies.is_empty() {
        let mut parts = Vec::new();
        for (p, b) in &copies {
            parts.push(format!("{}({b})", rel(repo, p)?));
        }
        why += &format!(" + {}", parts.join(" · "));
        let mut l: Vec<PathBuf> = copies.iter().map(|(p, _)| p.clone()).collect();
        l.extend(logs);
        logs = l;
    }
    Ok((logs, why, copies))
}

/// 이 측정의 엔진 로그(파일 목록, 근거, {같은 실행 전체 사본 posix 경로: 판별 근거}).
pub fn _measured_engine_logs_ex(repo: &Path, ev: &J) -> HR<(Vec<PathBuf>, String, J)> {
    let (logs, why, copies) = measured_engine_logs_ex_t(repo, ev)?;
    let mut m = Map::new();
    for (p, b) in copies {
        m.insert(core::posix(&p), b);
    }
    Ok((logs, why, J::Dict(m)))
}

pub const LITE_PUBLISH_DIR: &str = "lite_publish";
pub const _LITE_PUBLISH_SUB_RE: &str = "_\\d{8}T\\d{6}Z";

/// `re.fullmatch(re.escape(cell) + _LITE_PUBLISH_SUB_RE, name)`.
fn lite_sub_match(cell: &str, name: &str) -> bool {
    name.strip_prefix(cell).map(|rest| rx_lite_sub().is_full(rest)).unwrap_or(false)
}

/// lite 조인 키 = 바인딩된 경량 리포트 `생성일` · 리포트가 없으면 lite 등급 인증서 measured_utc.
pub fn _lite_join_key(repo: &Path, ev: &J) -> HR<(Option<String>, String)> {
    if let Some(born) = _report_born_utc(repo, ev.get_str("bench_report_path"))? {
        return Ok((Some(born), "measured_utc = 리포트 생성일".to_string()));
    }
    let cert = if ev["certificate"].truthy() { ev["certificate"].clone() } else { J::dict() };
    let mu = if cert_is_lite(&cert)? { norm(&cert["measured_utc"]) } else { J::Null };
    if mu.truthy() {
        Ok((Some(mu.display()), "measured_utc = lite 인증서 measured_utc".to_string()))
    } else {
        Ok((None, "조인 키 없음(리포트 생성일 · lite 인증서 measured_utc 모두 없음)".to_string()))
    }
}

/// lite-only 셀의 바인딩 측정 raw 가 있는 디렉터리 (디렉터리|None, raw|None, 근거).
pub fn lite_join(repo: &Path, ev: &J) -> HR<(Option<PathBuf>, Option<J>, String)> {
    let repo = core::resolve(repo);
    let (key, kwhy) = _lite_join_key(&repo, ev)?;
    let key = match key {
        Some(k) => k,
        None => return Ok((None, None, kwhy)),
    };
    let cell = ev_s(ev, "cell");
    let base = repo.join("output").join(ev_s(ev, "topology")).join("benchlog");
    let mut cands = vec![base.clone()];
    let publ = base.join(LITE_PUBLISH_DIR);
    if publ.is_dir() {
        cands.extend(glob_dir(&publ, |n| lite_sub_match(&cell, n)).into_iter().filter(|q| q.is_dir() && !is_symlink(q)));
    }
    let raw_name = format!("lite_raw_{cell}.json");
    let mut hits = Vec::new();
    for d in cands {
        let (raw, _bad) = read_json_soft(&d.join(&raw_name))?;
        if let Some(raw) = raw {
            if raw.is_dict() && raw.get_str("config_name") == Some(cell.as_str()) && raw.get_str("measured_utc") == Some(key.as_str()) {
                hits.push((d, raw));
            }
        }
    }
    if hits.len() == 1 {
        let (d, raw) = hits.remove(0);
        let why = format!("{} · {kwhy} {key}", rel(&repo, &d.join(&raw_name))?);
        return Ok((Some(d), Some(raw), why));
    }
    if !hits.is_empty() {
        let mut rels = Vec::new();
        for (d, _) in &hits {
            rels.push(rel(&repo, d)?);
        }
        return Ok((None, None, format!("모호 — {key} 와 조인되는 lite raw 가 {}곳({}) · 고르지 않는다", hits.len(), rels.join(", "))));
    }
    Ok((None, None, format!("{kwhy} {key} 와 조인되는 {raw_name} 없음(벤치로그 루트 · {LITE_PUBLISH_DIR}/{cell}_<UTC>/)")))
}

fn measured_engine_logs_base(repo: &Path, ev: &J) -> HR<(Vec<PathBuf>, String)> {
    let sw = &ev["sweep"];
    let cell = ev_s(ev, "cell");
    if sw.get_str("binding") == Some("output") {
        let base = repo.join(sw["dir"].display());
        let idx = if sw["index"].truthy() { sw["index"].clone() } else { J::dict() };
        let levels = if sw["levels"].truthy() { sw["levels"].clone() } else { J::list() };
        let mut logs = _sweep_engine_logs(&base, &idx, &levels, &cell)?;
        let mut why = format!("{}(스윕 generated_utc = measured_utc · 색인이 기록한 레그만)", sw["dir"].display());
        let nat = native_preserved_engine_logs(repo, ev)?;
        if !nat.is_empty() {
            let mut l: Vec<PathBuf> = logs.into_iter().filter(|q| nonempty(q)).collect();
            l.extend(nat.iter().cloned());
            logs = l;
            let mut rels = Vec::new();
            for q in &nat {
                rels.push(rel(repo, q)?);
            }
            why += &format!(" + native serve proof evidence_dir 보존 엔진 로그({} · 이 run 의 vllm serve 전체 출력)", rels.join(", "));
        }
        return Ok((logs, why));
    }
    if !sw.truthy() && ev.get_str("report_kind") == Some("lite") {
        let (d, _raw, jwhy) = lite_join(repo, ev)?;
        if let Some(d) = d {
            let eng = d.join(format!("lite_engine_{cell}.log"));
            if eng.is_file() {
                let head = d.join(format!("lite_engine_head_{cell}.log"));
                let mut logs = Vec::new();
                if head.is_file() && nonempty(&head) {
                    logs.push(head);
                }
                logs.push(eng);
                let tail = if logs.len() > 1 { " · 엔진 로그 머리 캡처 포함" } else { " · 꼬리 캡처만(머리 캡처 없음)" };
                return Ok((logs, format!("lite_raw 조인({jwhy}){tail}")));
            }
        }
        return Ok((Vec::new(), format!("lite 셀 — lite_raw 조인 불성립 또는 엔진 로그 부재({jwhy})")));
    }
    if sw.truthy() {
        let b = sw.get("binding").cloned().unwrap_or(J::Null);
        Ok((Vec::new(), format!("스윕이 출력 평면에 묶이지 않았다(binding={}) — 그 로그는 이 측정의 것이 아니다", b.repr())))
    } else {
        Ok((Vec::new(), "측정 스윕 없음".to_string()))
    }
}

pub fn _measured_engine_logs_base(repo: &Path, ev: &J) -> HR<(Vec<PathBuf>, String)> {
    measured_engine_logs_base(repo, ev)
}

fn nonempty(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false)
}

pub fn _nonempty(p: &Path) -> HR<bool> {
    Ok(nonempty(p))
}

fn native_preserved_engine_logs(repo: &Path, ev: &J) -> HR<Vec<PathBuf>> {
    if ev.get_str("plane") != Some("native") || !ev["serve_proof"].is_dict() {
        return Ok(Vec::new());
    }
    let raw = match ev["serve_proof"].get_str("evidence_dir") {
        Some(r) if !r.trim().is_empty() && !r.starts_with('/') && !has_dotdot(r) => r,
        _ => return Ok(Vec::new()),
    };
    let d = repo.join(raw.trim()).join("logs");
    if core::resolve(&d).strip_prefix(core::resolve(repo)).is_err() {
        return Ok(Vec::new());
    }
    if !d.is_dir() {
        return Ok(Vec::new());
    }
    Ok(glob_dir(&d, |n| infix_suffix(n, "vllm-serve", ".log")).into_iter().filter(|q| q.is_file() && nonempty(q)).collect())
}

pub fn _native_preserved_engine_logs(repo: &Path, ev: &J) -> HR<Vec<PathBuf>> {
    native_preserved_engine_logs(repo, ev)
}

/// 엔진 로그 줄(바이트로 읽고 `\n` 으로만 자른다).
pub fn _log_lines(path: &Path) -> HR<Vec<String>> {
    match std::fs::read(path) {
        Ok(b) => Ok(lossy(&b).split('\n').map(str::to_string).collect()),
        Err(e) => core::fail(
            "HINT_EVIDENCE_UNREADABLE",
            format!("엔진 로그를 읽을 수 없다({}): {}: {}", pname(path), os_type(&e), core::io_err_text(&e, path)),
            None,
        ),
    }
}

fn node_of_line(line: &str, table: &[(String, String)]) -> HR<(Option<String>, Option<String>)> {
    for rx in [rx_nccl_host(), rx_ray_ip()] {
        let m = match rx.search(line) {
            Some(m) => m,
            None => continue,
        };
        let sub = crate::pii::substitute(&m[1], table, false)?;
        if let Some(pm) = rx_node_ph().fullmatch(&sub) {
            let lab = pm[1].to_string();
            let node = if lab == "main" || lab == "sub" { Some(lab.clone()) } else { None };
            return Ok((node, Some(lab)));
        }
    }
    Ok((None, None))
}

/// 로그 줄의 호스트 → 치환표의 노드 표지 (`table` = list of [literal, placeholder]).
pub fn _node_of_line(line: &str, table: &J) -> HR<(Option<String>, Option<String>)> {
    let t: Vec<(String, String)> = table.items().iter().map(|r| (r.at(0).display(), r.at(1).display())).collect();
    node_of_line(line, &t)
}

re_static!(_TAIL_CAP_RE, r#"\btail\s+-(?:n\s*)?(\d+)\s*>\s*"\$ELOG""#);

fn log_tail_caps(repo: &Path, ev: &J) -> HR<Vec<(String, (i64, String))>> {
    let sel = _tool_selection(repo, ev)?;
    let mut out: Vec<(String, (i64, String))> = Vec::new();
    for t in sel["tools"].items() {
        let role = t["role"].display();
        if (role != "lite" && role != "bench") || !sel["at"].truthy() {
            continue;
        }
        let at = sel["at"].display();
        let rp = t["repo_path"].display();
        let raw = git_text(repo, &at, &rp)?;
        let rv = _tool_revision(repo, &at, &rp)?.unwrap_or_else(J::dict);
        let text = lossy(raw.as_deref().unwrap_or(b""));
        for (i, ln) in text.split('\n').enumerate() {
            if let Some(m) = _TAIL_CAP_RE().search(ln) {
                if !ln.trim_start().starts_with('#') {
                    let rev = or(&rv["git_rev"], &sel["at"]).display();
                    let v = (m[1].parse::<i64>().unwrap_or(0), format!("{}@{} L{}", t["name"].display(), core::head_chars(&rev, 12), i + 1));
                    if let Some(e) = out.iter_mut().find(|(k, _)| *k == role) {
                        e.1 = v;
                    } else {
                        out.push((role.clone(), v));
                    }
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// {역할: [N, 출처]} — 측정 시점 판본 도구의 `tail -N` 상한.
pub fn _log_tail_caps(repo: &Path, ev: &J) -> HR<J> {
    let mut m = Map::new();
    for (k, (n, s)) in log_tail_caps(repo, ev)? {
        m.insert(k, jl![n, s]);
    }
    Ok(J::Dict(m))
}

fn tail_windows(repo: &Path, ev: &J, logs: &[PathBuf], copies: &[PathBuf]) -> HR<Vec<(String, String)>> {
    let caps = log_tail_caps(repo, ev)?;
    let mut full = native_preserved_engine_logs(repo, ev)?;
    full.extend(copies.iter().cloned());
    let mut windows: Vec<(String, String)> = Vec::new();
    for lp in logs {
        let name = pname(lp);
        if full.contains(lp) || name.starts_with("lite_engine_head_") {
            continue;
        }
        let role = if name.starts_with("lite_engine_") { "lite" } else { "bench" };
        let cap = caps.iter().find(|(k, _)| k == role).map(|(_, v)| v.clone());
        let n_lines: i64 = std::fs::read(lp).map(|b| b.iter().filter(|&&c| c == b'\n').count() as i64).unwrap_or(-1);
        if let Some((n, src)) = cap {
            if n_lines >= n {
                let r = rel(repo, lp)?;
                let v = format!("tail-{n}({src})");
                if let Some(e) = windows.iter_mut().find(|(k, _)| *k == r) {
                    e.1 = v;
                } else {
                    windows.push((r, v));
                }
            }
        }
    }
    Ok(windows)
}

fn paths_of(j: &J) -> Vec<PathBuf> {
    match j {
        J::Dict(m) => m.keys().map(PathBuf::from).collect(),
        other => other.items().iter().map(crate::api::decode_path).collect(),
    }
}

fn pairs_j(v: &[(String, String)]) -> J {
    let mut m = Map::new();
    for (k, x) in v {
        m.insert(k.clone(), x.clone());
    }
    J::Dict(m)
}

/// {로그 상대경로: `tail-<N>(…)`} — 꼬리 상한에 닿은 꼬리 캡처만 (`logs` list of paths · `copies` dict/list of paths).
pub fn _tail_windows(repo: &Path, ev: &J, logs: &J, copies: &J) -> HR<J> {
    Ok(pairs_j(&tail_windows(repo, ev, &paths_of(logs), &paths_of(copies))?))
}

fn head_windows(repo: &Path, ev: &J, logs: &[PathBuf]) -> HR<Vec<(String, String)>> {
    let cell = ev_s(ev, "cell");
    let mut out: Vec<(String, String)> = Vec::new();
    for lp in logs {
        if !pname(lp).starts_with("lite_engine_head_") {
            continue;
        }
        let rp = lp.parent().unwrap_or(Path::new("")).join(format!("lite_raw_{cell}.json"));
        let raw = if rp.is_file() { read_json_soft(&rp)?.0 } else { None };
        let raw = match raw {
            Some(r) if r.is_dict() => r,
            _ => continue,
        };
        let n = match raw.get("engine_log_head_lines") {
            Some(J::Int(n)) if *n > 0 => *n,
            _ => continue,
        };
        let named = or(&raw["engine_log_head"], &J::Null);
        let named = if named.truthy() { named.display() } else { String::new() };
        if pname(Path::new(&named)) != pname(lp) {
            continue;
        }
        let n_lines = match std::fs::read(lp) {
            Ok(b) => b.iter().filter(|&&c| c == b'\n').count() as i64,
            Err(_) => continue,
        };
        if n_lines >= n {
            out.push((rel(repo, lp)?, format!("head-{n}({} engine_log_head_lines)", rel(repo, &rp)?)));
        }
    }
    Ok(out)
}

/// {머리 캡처 로그 상대경로: `head-<N>(…)`}.
pub fn _head_windows(repo: &Path, ev: &J, logs: &J) -> HR<J> {
    Ok(pairs_j(&head_windows(repo, ev, &paths_of(logs))?))
}

re_static!(_ENV_KEY_PRESENT_RE, r"Unknown vLLM environment variable detected:\s*([A-Z][A-Z0-9_]*)\b");

/// 이 측정의 엔진 로그가 되읊은 env · NCCL 런타임 관측(공유 계약 `facts['measurement_env_observed']`).
pub fn measurement_env_observed(repo: &Path, ev: &J) -> HR<Vec<J>> {
    let repo = core::resolve(repo);
    let (logs, _why, copies) = measured_engine_logs_ex_t(&repo, ev)?;
    if logs.is_empty() {
        return Ok(Vec::new());
    }
    let man = if ev["manifest"].is_dict() { ev["manifest"].clone() } else { J::Null };
    let table = crate::pii::substitution_table(&repo, &man)?;
    let copy_paths: Vec<PathBuf> = copies.iter().map(|(p, _)| p.clone()).collect();
    let windows = tail_windows(&repo, ev, &logs, &copy_paths)?;
    let heads = head_windows(&repo, ev, &logs)?;
    let win_of = |r: &str| windows.iter().find(|(k, _)| k == r).map(|(_, v)| v.clone());
    let head_of = |r: &str| heads.iter().find(|(k, _)| k == r).map(|(_, v)| v.clone());
    type K = (String, String, Option<String>, Option<String>);
    let mut rows: Vec<(K, J, BTreeSet<String>)> = Vec::new();
    let mut index: HashMap<K, usize> = HashMap::new();
    for lp in &logs {
        let r = rel(&repo, lp)?;
        for (i0, line) in _log_lines(lp)?.iter().enumerate() {
            let i = i0 + 1;
            let km = if !line.contains("NCCL INFO") { _ENV_KEY_PRESENT_RE().search(&rx_ansi().sub_literal(line, "")).map(|c| c[1].to_string()) } else { None };
            if let Some(key) = km {
                let (node, label) = node_of_line(&rx_ansi().sub_literal(line, ""), &table)?;
                let k: K = ("key-present".into(), key.clone(), None, node.clone());
                if let Some(&ix) = index.get(&k) {
                    rows[ix].2.insert(r.clone());
                    let n = rows[ix].2.len();
                    rows[ix].1.set("seen_in_logs", n);
                    continue;
                }
                let mut row = jd! {"key" => key.as_str(), "value" => J::Null, "node" => node.clone(), "source" => format!("{r}:L{i}"),
                    "kind" => "key-present", "seen_in_logs" => 1i64,
                    "value_note" => "키 존재만 관측 · 값 미관측(vLLM 'Unknown vLLM environment variable detected' 경고 — 값은 로그에 없다)"};
                if let Some(w) = win_of(&r) {
                    row.set("log_window", w);
                }
                if let Some(h) = head_of(&r) {
                    row.set("log_head_window", h);
                }
                if node.is_none() {
                    if let Some(l) = &label {
                        row.set("node_label", l.as_str());
                    }
                }
                index.insert(k.clone(), rows.len());
                rows.push((k, row, [r.clone()].into_iter().collect()));
                continue;
            }
            if !line.contains("NCCL INFO") {
                continue;
            }
            let clean = rx_ansi().sub_literal(line, "").trim_end().to_string();
            let dm = rx_ray_dedup().search(&clean).map(|c| (c.get(0).unwrap().start(), c[1].parse::<i64>().unwrap_or(0)));
            let dedup = dm.map(|d| d.1).unwrap_or(0);
            let body = match dm {
                Some((s, _)) => clean[..s].to_string(),
                None => clean.clone(),
            };
            let mut hit: Option<(String, String, String)> = None;
            for (kind, rx) in [("env-echo", rx_env_echo()), ("param-echo", rx_param_echo())] {
                if let Some(m) = rx.search(&body) {
                    let mut val = m[2].to_string();
                    if rx_neg_int_dot().is_full(&val) {
                        val.pop();
                    }
                    hit = Some((kind.to_string(), m[1].to_string(), val));
                    break;
                }
            }
            if hit.is_none() {
                for (key, rx) in [("nccl:network", rx_rt_network()), ("nccl:net_ib_devices", rx_rt_ib()), ("nccl:version", rx_rt_version())] {
                    if let Some(m) = rx.search(&body) {
                        hit = Some(("runtime".to_string(), key.to_string(), m[1].trim().to_string()));
                        break;
                    }
                }
            }
            let (kind, key, raw_val) = match hit {
                Some(h) => h,
                None => continue,
            };
            let (node, label) = node_of_line(&body, &table)?;
            let val = crate::pii::substitute(&raw_val, &table, true)?;
            let k: K = (kind.clone(), key.clone(), Some(val.clone()), node.clone());
            if let Some(&ix) = index.get(&k) {
                rows[ix].2.insert(r.clone());
                let n = rows[ix].2.len();
                rows[ix].1.set("seen_in_logs", n);
                continue;
            }
            let mut row = jd! {"key" => key.as_str(), "value" => val.as_str(), "node" => node.clone(), "source" => format!("{r}:L{i}"),
                "kind" => kind.as_str(), "seen_in_logs" => 1i64};
            if let Some(w) = win_of(&r) {
                row.set("log_window", w);
            }
            if let Some(h) = head_of(&r) {
                row.set("log_head_window", h);
            }
            if dedup != 0 {
                row.set("ray_repeated", dedup);
            }
            if val != raw_val {
                row.set("value_note", "기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)");
            }
            if node.is_none() {
                if let Some(l) = &label {
                    row.set("node_label", l.as_str());
                }
            }
            index.insert(k.clone(), rows.len());
            rows.push((k, row, [r.clone()].into_iter().collect()));
        }
    }
    Ok(rows.into_iter().map(|(_, r, _)| r).collect())
}

/// All startup epochs in an accumulated engine log, with original line numbers.
pub fn _banners_of(path: &Path) -> HR<Vec<(i64, String, Option<String>)>> {
    let mut out = Vec::new();
    for (i, line) in _log_lines(path)?.iter().enumerate() {
        for seg in line.split('\r') {
            let clean = rx_ansi().sub_literal(seg, "");
            for rx in [rx_banner1(), rx_banner2(), rx_banner3()] {
                if let Some(m) = rx.search(&clean) {
                    let tm = rx_engine_ts().search(&clean).map(|t| ts_of(&t));
                    out.push((i as i64 + 1, m[1].to_string(), tm));
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// First startup banner (signature fix: the stamp is optional — no callers outside this part).
pub fn _banner_of(path: &Path) -> HR<Option<(i64, String, Option<String>)>> {
    Ok(_banners_of(path)?.into_iter().next())
}

/// Yearless container clock is UTC-assumed; exactly one year must fit the bound window.
pub fn _banner_in_window(stamp: Option<&str>, start: &str, end: &str) -> HR<Option<String>> {
    let (a, b) = match (utc_dt(Some(start)), utc_dt(Some(end))) {
        (Some(a), Some(b)) => (a, b),
        _ => return Ok(None),
    };
    let stamp = match stamp {
        Some(s) if !s.is_empty() && a <= b => s,
        _ => return Ok(None),
    };
    let mut hits = Vec::new();
    for year in a.year..=b.year {
        if let Some(c) = caps6(rx_ymd_hms(), &format!("{year}-{stamp}")) {
            let v = iso_z(&c);
            if start <= v.as_str() && v.as_str() <= end {
                hits.push(v);
            }
        }
    }
    Ok(if hits.len() == 1 { Some(hits.remove(0)) } else { None })
}

/// 이웃 산출물 읽기 — 반환 `[doc|null, 사유|null]` (typed form: `_read_json_soft_t`).
pub fn _read_json_soft(path: &Path) -> HR<J> {
    let (d, b) = read_json_soft(path)?;
    Ok(jl![J::from(d), J::from(b)])
}

/// `_read_json_soft` → (doc|None, 사유|None).
pub fn _read_json_soft_t(path: &Path) -> HR<(Option<J>, Option<String>)> {
    read_json_soft(path)
}

fn digest_sweeps(repo: &Path, ev: &J, digest: Option<&str>, tag: Option<&str>) -> HR<Vec<J>> {
    let mut out = Vec::new();
    let digest = match digest {
        Some(d) if !d.is_empty() => d,
        _ => return Ok(out),
    };
    let base = repo.join("output").join(ev_s(ev, "topology")).join("benchlog");
    for d in glob_dir(&base, |n| n.starts_with("sweep_")) {
        let p = d.join("sweep_index.json");
        if !p.exists() {
            continue;
        }
        let (idx, _bad) = read_json_soft(&p)?;
        let idx = match idx {
            Some(i) if i.is_dict() => i,
            _ => continue,
        };
        let meta = if idx["meta"].is_dict() { idx["meta"].clone() } else { J::dict() };
        if meta.get_str("image_digest") != Some(digest) || !pystr_get(&meta, "image_digest_source").starts_with("measured") {
            continue;
        }
        let tag_ok = match tag {
            None => true,
            Some(t) => meta.get_str("image_tag") == Some(t) && pystr_get(&meta, "image_tag_source").starts_with("measured"),
        };
        if let Some(g) = idx.get_str("generated_utc") {
            if rx_utc().is_full(g) {
                let levels = if idx["levels"].is_list() { idx["levels"].clone() } else { J::list() };
                out.push(jd! {"dir" => core::posix(&d), "utc" => g, "idx" => idx.clone(), "tag_ok" => tag_ok, "levels" => levels});
            }
        }
    }
    out.sort_by(|a, b| (a["utc"].display(), a["dir"].display()).cmp(&(b["utc"].display(), b["dir"].display())));
    Ok(out)
}

/// 출력 평면 스윕 중 측정한 digest(와 태그)가 이 측정과 같은 것 [{dir, utc, idx, tag_ok, levels}](utc 순).
pub fn _digest_sweeps(repo: &Path, ev: &J, digest: Option<&str>, tag: Option<&str>) -> HR<Vec<J>> {
    digest_sweeps(repo, ev, digest, tag)
}

#[derive(Clone)]
struct Cand {
    utc: Option<String>,
    val: String,
    src: String,
    files: Vec<(String, String)>,
}

fn cand_of(j: &J) -> Cand {
    Cand { utc: opt_str(j.at(0)), val: j.at(1).display(), src: j.at(2).display(), files: files_of(j.at(3)) }
}

fn pick_banner(cands: &[Cand], measured: Option<&str>, basis: &str, what: &str) -> J {
    let vals: Vec<String> = cands.iter().map(|c| c.val.clone()).collect::<BTreeSet<_>>().into_iter().collect();
    if vals.len() > 1 {
        let mut by = Vec::new();
        for v in &vals {
            let s = cands.iter().filter(|c| &c.val == v).min_by(|a, b| a.src.cmp(&b.src)).map(|c| c.src.clone()).unwrap_or_default();
            by.push(format!("{v} ← {s}"));
        }
        let files: Vec<(String, String)> = cands.iter().flat_map(|c| c.files.clone()).collect();
        return jd! {"value" => J::Null, "basis" => "conflict", "files" => files_j(&files),
            "source" => format!("{what} 이 서로 다른 버전을 말한다 — 고르지 않는다: {}", by.join(" · "))};
    }
    let md = utc_dt(measured);
    let dist = |c: &Cand| -> (i64, String) {
        let d = match (utc_dt(c.utc.as_deref()), md) {
            (Some(cd), Some(md)) => (cd.epoch() - md.epoch()).abs(),
            _ => 0,
        };
        (d, c.src.clone())
    };
    let best = cands.iter().min_by(|a, b| dist(a).cmp(&dist(b))).unwrap();
    let more = cands.len() - 1;
    let tail = if more > 0 { format!(" · 같은 값의 다른 로그 {more}건") } else { String::new() };
    jd! {"value" => vals[0].as_str(), "basis" => basis, "files" => files_j(&best.files), "source" => format!("{} — {what}{tail}", best.src)}
}

fn engine_banner(repo: &Path, ev: &J) -> HR<J> {
    let sw = &ev["sweep"];
    let meta = sw["index"]["meta"].clone();
    let meta = if meta.truthy() { meta } else { J::dict() };
    let mm = _measured_meta(&ev["sweep"], &ev["certificate"])?;
    let digest = or(&mm["image_digest"], &ev["build_identity"]["image_digest"]).clone();
    let tag: Option<String> =
        if pystr_get(&mm, "image_tag_source").starts_with("sweep_index.meta.image_tag(measured") { mm.get("image_tag").map(J::display) } else { None };
    let tag = tag.filter(|_| !mm["image_tag"].is_null());
    let measured = _measured_key(repo, ev)?;
    let (logs_, _w, copies) = measured_engine_logs_ex_t(repo, ev)?;
    let (first, last, _src) = _measure_window(repo, ev)?;
    let window = crate::artifacts::current_serve_window(repo, ev, first.as_deref(), last.as_deref())?;
    let window = window.filter(|w| w.truthy());
    let mut current: Vec<Cand> = Vec::new();
    let mut unbound: Vec<Cand> = Vec::new();
    for lp in &logs_ {
        for b in _banners_of(lp)? {
            let r = rel(repo, lp)?;
            let copy = copies.iter().find(|(p, _)| p == lp).map(|(_, s)| format!(" · {s}")).unwrap_or_default();
            let source = format!("{r}:{}{copy}", b.0);
            unbound.push(Cand { utc: measured.clone(), val: b.1.clone(), src: source.clone(), files: vec![(r.clone(), "engine_log".into())] });
            if let Some(w) = &window {
                let end_utc = w["end_utc"].as_str().map(str::to_string);
                let xs: Vec<&String> = [first.as_ref(), measured.as_ref(), end_utc.as_ref()].into_iter().flatten().filter(|x| !x.is_empty()).collect();
                let upper = match xs.iter().min() {
                    Some(u) => (*u).clone(),
                    None => return Err(HintError::py("ValueError", "min() iterable argument is empty")),
                };
                if let Some(stamp) = _banner_in_window(b.2.as_deref(), &w["start_utc"].display(), &upper)? {
                    let wsrc = w["source"].display();
                    current.push(Cand {
                        utc: Some(stamp.clone()),
                        val: b.1.clone(),
                        src: format!("{source} · 기동 {stamp}(컨테이너 시계 UTC 가정) · {wsrc}"),
                        files: vec![(r.clone(), "engine_log".into()), (wsrc.clone(), "raw_json".into())],
                    });
                }
            }
        }
    }
    if !current.is_empty() {
        return Ok(pick_banner(&current, measured.as_deref(), "this-measurement", "측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너"));
    }
    if window.is_none() {
        if sw.get_str("binding") == Some("output") && !unobs(&meta["vllm_build"]) {
            let ip = repo.join(sw["dir"].display()).join("sweep_index.json");
            let r = rel(repo, &ip)?;
            let ln = _json_key_line(&ip, "vllm_build", &meta["vllm_build"])?.filter(|n| *n != 0).map(|n| n.to_string()).unwrap_or("?".into());
            unbound.push(Cand { utc: measured.clone(), val: meta["vllm_build"].display(), src: format!("{r}:{ln} meta.vllm_build"), files: vec![(r, "sweep_json".into())] });
        }
        if !unbound.is_empty() {
            return Ok(pick_banner(&unbound, measured.as_deref(), "measurement-capture-unbound",
                "측정 캡처에 기록된 값 · 현재 serve 세션 경계 미관측 — 누적 로그의 기동 epoch 귀속 미검증"));
        }
    }
    let digest_s = if digest.truthy() { Some(digest.display()) } else { None };
    let sweeps = digest_sweeps(repo, ev, digest_s.as_deref(), tag.as_deref())?;
    let this_dir = if sw["dir"].truthy() { Some(core::resolve(&repo.join(sw["dir"].display()))) } else { None };
    let mut rec: Vec<Cand> = Vec::new();
    for s in &sweeps {
        let sdir = PathBuf::from(s["dir"].display());
        if let Some(td) = &this_dir {
            if &core::resolve(&sdir) == td {
                continue;
            }
        }
        let smeta = if s["idx"]["meta"].truthy() { s["idx"]["meta"].clone() } else { J::dict() };
        if !unobs(&smeta["vllm_build"]) {
            let ip = sdir.join("sweep_index.json");
            let r = rel(repo, &ip)?;
            let ln = _json_key_line(&ip, "vllm_build", &smeta["vllm_build"])?.filter(|n| *n != 0).map(|n| n.to_string()).unwrap_or("?".into());
            rec.push(Cand { utc: opt_str(&s["utc"]), val: smeta["vllm_build"].display(), src: format!("{r}:{ln} meta.vllm_build"), files: vec![(r, "sweep_json".into())] });
            continue;
        }
        let cfg = or(&smeta["config_name"], or(&s["idx"]["config"], &J::Null)).clone();
        let cfg = if cfg.truthy() { cfg.display() } else { String::new() };
        for lp in _sweep_engine_logs(&sdir, &s["idx"], &s["levels"], &cfg)? {
            if let Some(b) = _banner_of(&lp)? {
                let r = rel(repo, &lp)?;
                rec.push(Cand { utc: opt_str(&s["utc"]), val: b.1, src: format!("{r}:{}", b.0), files: vec![(r, "engine_log".into())] });
                break;
            }
        }
    }
    if !rec.is_empty() {
        let what = format!("측정 digest {}… 를 기록한(sweep_index.meta.image_digest · measured) 다른 스윕의 엔진 로그", core::head_chars(&digest.display(), 19));
        return Ok(pick_banner(&rec, measured.as_deref(), "same-digest-recorded", &what));
    }
    let brackets: Vec<J> = if tag.is_some() { sweeps.iter().filter(|s| s["tag_ok"].truthy()).cloned().collect() } else { Vec::new() };
    let mut inf: Vec<Cand> = Vec::new();
    if !brackets.is_empty() {
        let topo = ev_s(ev, "topology");
        let tag_s = tag.clone().unwrap_or_default();
        for d in glob_dir(&repo.join("output").join(&topo).join("benchlog"), |n| n.starts_with("serve_fail_")) {
            let cfg = pname(&d)["serve_fail_".len()..].to_string();
            let env_rel = format!("output/{topo}/envs/.env.{cfg}");
            if !d.is_dir() || !rx_cell().is_match(&cfg) || !repo.join(&env_rel).is_file() {
                continue;
            }
            if _env_first(repo, Some(&env_rel))?.get_str("IMAGE_TAG") != Some(tag_s.as_str()) {
                continue;
            }
            for lp in glob_dir(&d, |n| n.ends_with(".log")) {
                let b = match _banner_of(&lp)? {
                    Some(b) => b,
                    None => continue,
                };
                let stamp = match &b.2 {
                    Some(s) if !s.is_empty() => s.clone(),
                    _ => continue,
                };
                let (t_utc, s1, s2) = match bracketed(&stamp, &brackets, measured.as_deref()) {
                    Some(t) => t,
                    None => continue,
                };
                let r = rel(repo, &lp)?;
                let src = format!(
                    "{r}:{}(컨테이너 시계 {stamp} → {t_utc} UTC 가정) — digest 미기록 로그: {env_rel} IMAGE_TAG(현재 파일 값)={tag_s} · 앞뒤 스윕 {}({}) · {}({}) 가 같은 태그→digest {}… 를 측정했다 (그 사이 재태깅은 관측 대상 밖) → 같은 digest 로 **추론**(기록 아님)",
                    b.0,
                    rel(repo, Path::new(&s1["dir"].display()))?,
                    s1["utc"].display(),
                    rel(repo, Path::new(&s2["dir"].display()))?,
                    s2["utc"].display(),
                    core::head_chars(&digest.display(), 19)
                );
                inf.push(Cand { utc: Some(t_utc), val: b.1, src, files: vec![(r, "engine_log".into())] });
            }
        }
    }
    if !inf.is_empty() {
        return Ok(pick_banner(&inf, measured.as_deref(), "same-digest-inferred", "digest 를 기록하지 않은 기동 실패 로그의 배너"));
    }
    let mut why = if window.is_some() {
        "현재 serve 세션에 결속되는 배너 없음(누적 로그의 과거·미래·시각 없는 기동은 이번 측정으로 세지 않는다)".to_string()
    } else if !digest.truthy() {
        "측정 digest 미관측".to_string()
    } else {
        "측정 캡처와 같은 digest 보조 로그에서 기동 배너 관측 없음".to_string()
    };
    let win = if !logs_.is_empty() {
        let cp: Vec<PathBuf> = copies.iter().map(|(p, _)| p.clone()).collect();
        tail_windows(repo, ev, &logs_, &cp)?
    } else {
        Vec::new()
    };
    if !win.is_empty() && win.len() == logs_.len() {
        let parts: Vec<String> = win.iter().map(|(r, w)| format!("{r} = 보존 꼬리 창 {w}")).collect();
        why += &format!(" · 관측 범위: {} — 기동 배너 자리(로그 머리)는 창 밖이다 · 머리 캡처(lite_engine_head_*) 없는 측정이라 배너 부재를 관측하지 못했다", parts.join(" · "));
    }
    Ok(jd! {"value" => J::Null, "basis" => "unobserved", "files" => J::list(), "source" => format!("미관측 — {why}")})
}

/// 엔진 자기보고(기동 배너) — {value, basis, files, source}.
pub fn _engine_banner(repo: &Path, ev: &J) -> HR<J> {
    engine_banner(repo, ev)
}

fn bracketed(mmdd_hms: &str, brackets: &[J], measured: Option<&str>) -> Option<(String, J, J)> {
    let base = utc_dt(measured).or_else(|| utc_dt(brackets.last().and_then(|b| b["utc"].as_str())))?;
    let m = rx_mmdd_hms().fullmatch(mmdd_hms)?;
    let n = |i: usize| m[i].parse::<i64>().unwrap_or(-1);
    for year in [base.year, base.year - 1] {
        let t = match mk_civil(year, n(1), n(2), n(3), n(4), n(5)) {
            Some(c) => iso_z(&c),
            None => continue,
        };
        let before: Vec<&J> = brackets.iter().filter(|s| s["utc"].display() <= t).collect();
        let after: Vec<&J> = brackets.iter().filter(|s| s["utc"].display() >= t).collect();
        if !before.is_empty() && !after.is_empty() {
            return Some((t, before[before.len() - 1].clone(), after[0].clone()));
        }
    }
    None
}

/// 컨테이너 시각 → 앞뒤 스윕 괄호 (utc, 앞, 뒤) | None.
pub fn _bracketed(mmdd_hms: &str, brackets: &J, measured: Option<&str>) -> HR<Option<(String, J, J)>> {
    Ok(bracketed(mmdd_hms, brackets.items(), measured))
}

/// 후보 [[utc, 값, 출처, files]] → 값이 하나면 측정 시각에 가장 가까운 출처 · 둘 이상이면 conflict.
pub fn _pick_banner(cands: &J, measured: Option<&str>, basis: &str, what: &str) -> HR<J> {
    let c: Vec<Cand> = cands.items().iter().map(cand_of).collect();
    if c.is_empty() {
        return Err(HintError::py("ValueError", "min() iterable argument is empty"));
    }
    Ok(pick_banner(&c, measured, basis, what))
}

/// hintlib/evidence.py:4159 _LITE_VLLM_SOURCE_NOTES (python literal as JSON)
pub fn _LITE_VLLM_SOURCE_NOTES() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"derived(envfile IMAGE_TAG)\": \"render_report.lite_identity 가 envfile IMAGE_TAG(`easy-vllm:X.Y.Z`)에서 파생 — **엔진 자기보고가 아니다**(빌드 입력 이미지 태그를 x.y.z 로 자른 값 · rc/dev 접미가 사라진다) · env 덮어쓰기(EASY_VLLM_VERSION)였다면 declared(env EASY_VLLM_VERSION) 로 남았을 것이다\", \"declared(env EASY_VLLM_VERSION)\": \"render_report.lite_identity 가 env EASY_VLLM_VERSION 선언을 옮김 — **엔진 자기보고가 아니다**(선언값)\", \"measured(engine log 자기보고)\": \"render_report.lite_identity 가 엔진 로그 배너의 x.y.z 접두를 씀(envfile IMAGE_TAG 규칙이 값을 내지 못했다)\", \"declared(config yaml comment)\": \"render_report.lite_identity 가 config yaml 주석을 옮김 — **엔진 자기보고가 아니다**(선언값)\"}").expect("constant"))
}

/// lite 등급 인증서의 vllm_version producer — (라벨, 출처) | None(기존 규칙).
pub fn _lite_cert_vllm_producer(repo: &Path, ev: &J, cert_v: &str) -> HR<Option<(String, String)>> {
    let brp = match ev.get_str("bench_report_path") {
        Some(b) if !b.is_empty() => b.to_string(),
        _ => return Ok(None),
    };
    if !(cert_is_lite(&ev["certificate"])? && ev.get_str("report_kind") == Some("lite")) {
        return Ok(None);
    }
    let snap = _lite_snapshot(repo, Some(&brp))?;
    let rep_v = norm(&snap["vllm_version"]);
    let rep_src = norm(&snap["vllm_version_source"]);
    if !rep_v.truthy() || !rep_src.truthy() {
        return Ok(None);
    }
    let name = pname(Path::new(&brp));
    if rep_v != J::from(cert_v) {
        return Ok(Some((
            "unverified".into(),
            format!(
                "미검증 — 경량 리포트 {name} 측정 환경 표 vllm_version={}({}) ≠ 인증서 {} (두 기록이 다른 값을 말한다 — 고르지 않는다)",
                rep_v.repr(),
                rep_src.display(),
                py_repr(cert_v)
            ),
        )));
    }
    let note = _LITE_VLLM_SOURCE_NOTES()
        .get(&rep_src.display())
        .map(J::display)
        .unwrap_or_else(|| "출처 어휘가 알려진 목록 밖이다 — 서술을 지어내지 않고 리포트 값만 옮긴다".to_string());
    Ok(Some((
        "lite-report-identity".into(),
        format!("경량 리포트 {name} 측정 환경 표 vllm_version={} · vllm_version_source={} (= 인증서 값 · 같은 측정) — {note}", rep_v.display(), rep_src.display()),
    )))
}

/// 인증서 vllm_version 의 producer → (라벨, 출처 서술). `sel` = `_tool_selection` 결과 또는 Null.
pub fn _cert_vllm_producer(repo: &Path, ev: &J, sel: &J) -> HR<(String, String)> {
    let cert_v = norm(&ev["certificate"]["vllm_version"]);
    let meta = &ev["sweep"]["index"]["meta"];
    let meta_v = norm(&meta["vllm_version"]);
    if !cert_v.truthy() {
        return Ok(("absent".into(), "인증서 vllm_version 없음".into()));
    }
    let cv = cert_v.display();
    if let Some(l) = _lite_cert_vllm_producer(repo, ev, &cv)? {
        return Ok(l);
    }
    let sel = if sel.truthy() { sel.clone() } else { _tool_selection(repo, ev)? };
    let relp = format!("{REL_BENCH_TOOLS_DIR}/sweep_bench.sh");
    let raw = if sel["at"].truthy() { git_text(repo, &sel["at"].display(), &relp)? } else { None };
    let raw = match raw {
        Some(r) => r,
        None => return Ok(("unverified".into(), format!("미검증 — 측정 시점 sweep_bench.sh 판본 미관측({})", sel["at_method"].display()))),
    };
    let text = lossy(&raw);
    let lines: Vec<&str> = text.split('\n').collect();
    let rv = _tool_revision(repo, &sel["at"].display(), &relp)?.unwrap_or_else(J::dict);
    let rev = or(&rv["git_rev"], &sel["at"]).display();
    let rev12 = core::head_chars(&rev, 12);
    let find = |rx: &Re| lines.iter().position(|ln| rx.is_search(ln)).map(|i| i + 1);
    let (ir, isrc, ienv, iban) = (find(rx_mark_image_rule()), find(rx_mark_image_src()), find(rx_mark_env_override()), find(rx_mark_banner()));
    let (ir, isrc) = match (ir, isrc) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            return Ok(("unverified".into(), format!("미검증 — sweep_bench.sh@{rev12} 에서 IMAGE_TAG 파생 규칙 표지를 찾지 못했다(판본이 다른 규칙을 쓴다 · tripwire)")))
        }
    };
    let pat = rx_mark_image_rule().search(lines[ir - 1]).map(|c| c[1].to_string()).unwrap_or_default();
    let declared = norm(&meta["image_tag_declared"]);
    let dtext = if declared.truthy() { declared.display() } else { String::new() };
    let cand: Option<String> = Re::try_new(&pat, 0).ok().and_then(|re| {
        re.search(&dtext).and_then(|c| if c.len() > 1 { c.get(1).map(|m| m.as_str().to_string()) } else { None })
    });
    let env_txt = match ienv {
        Some(n) => format!("더 높은 우선순위의 덮어쓰기(`--vllm-version` 인자 → VLLM_VER · EASY_VLLM_VERSION env · L{n})는 측정 기록에 남지 않아 배제할 수 없다"),
        None => "env 덮어쓰기 표지 없음".to_string(),
    };
    if cand.as_deref() == Some(cv.as_str()) && cand.as_deref().map(|c| !c.is_empty()).unwrap_or(false) {
        let same = if meta_v == cert_v { "= sweep_index.meta.vllm_version(인증서는 그 옮김)".to_string() } else { format!("(meta.vllm_version={} 와 다름)", meta_v.repr()) };
        return Ok((
            "image-tag-line".into(),
            format!(
                "sweep_bench.sh@{rev12} L{isrc}·L{ir}: 선언 IMAGE_TAG (sweep_index.meta.image_tag_declared {})에 그 판본의 정규식 `{pat}` → {cv} {same} — **엔진 자기보고가 아니다**(빌드 입력 이미지 태그를 x.y.z 로 자른 값 · rc/dev 접미가 사라진다). {env_txt}",
                declared.display()
            ),
        ));
    }
    let build = norm(&meta["vllm_build"]);
    let btext = if build.truthy() { build.display() } else { String::new() };
    let bm = rx_xyz().match_at_start(&btext).map(|c| c[1].to_string());
    if cand.is_none() && bm.as_deref() == Some(cv.as_str()) {
        if let Some(b) = iban {
            return Ok(("engine-banner-prefix".into(), format!("sweep_bench.sh@{rev12} L{b}: 이미지 태그 규칙이 값을 내지 못해 엔진 로그 배너 {} 의 x.y.z 접두를 썼다 · {env_txt}", build.display())));
        }
    }
    Ok(("unverified".into(), format!("미검증 — sweep_bench.sh@{rev12} 이미지 태그 규칙 결과 {} ≠ 인증서 {} · 배너 접두도 아니다 ({env_txt} — 덮어쓰기였을 수 있다)", repr_opt(&cand), cert_v.repr())))
}

/// `local(rel)` of `_runtime_observations`.
fn obs_local(repo: &Path, rel_: &J) -> Option<PathBuf> {
    let r = rel_.as_str()?;
    if !r.starts_with("docs/simlog/") || has_dotdot(r) {
        return None;
    }
    let path = repo.join(r);
    let mut p = path.clone();
    loop {
        if p.starts_with(repo) && is_symlink(&p) {
            return None;
        }
        if !p.pop() || !p.starts_with(repo) {
            break;
        }
    }
    core::resolve(&path).strip_prefix(core::resolve(repo)).ok()?;
    Some(path)
}

/// Recorded runtime observations explicitly joined by a cell's evidence index ([{data, source, files}]).
pub fn _runtime_observations(repo: &Path, ev: &J) -> HR<Vec<J>> {
    let sw = &ev["sweep"];
    let idx = &sw["index"];
    let config = or(&idx["meta"]["config_name"], or(&idx["config"], &ev["cell"])).clone();
    let measured = _measured_key(repo, ev)?;
    let measured = match measured {
        Some(m) if !m.is_empty() && sw["dir"].truthy() && sw.get_str("binding") == Some("output") => m,
        _ => return Ok(Vec::new()),
    };
    let cell = ev_s(ev, "cell");
    let mut candidates: BTreeSet<PathBuf> = BTreeSet::new();
    for ptr in ev["pointers"].items() {
        if !ptr.is_dict() || ptr.get_str("cell_id") != Some(cell.as_str()) {
            continue;
        }
        let r = &ptr["path"];
        let kind = ptr.get_str("kind").unwrap_or("");
        if kind == "simlog" {
            if let Some(path) = obs_local(repo, r) {
                if path.is_dir() {
                    rglob(&path, "evidence_index.json", &mut candidates);
                } else if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
                    candidates.insert(path);
                }
            }
        } else if ["plan", "devlog", "testlog"].contains(&kind) {
            let rs = match r.as_str() {
                Some(s) if s.starts_with("docs/") && !has_dotdot(s) => s,
                _ => continue,
            };
            let f = repo.join(rs);
            if !f.is_file() {
                continue;
            }
            let mut p = f.clone();
            let mut sym = false;
            loop {
                if p != repo && p.starts_with(repo) && is_symlink(&p) {
                    sym = true;
                }
                if !p.pop() || !p.starts_with(repo) {
                    break;
                }
            }
            if sym {
                continue;
            }
            let text = lossy(&std::fs::read(&f).map_err(|e| py_io(&e, &f))?);
            for dirname in rx_simlog_ref().findall(&text) {
                if let Some(path) = obs_local(repo, &J::from(format!("docs/simlog/{dirname}"))) {
                    if path.is_dir() {
                        rglob(&path, "evidence_index.json", &mut candidates);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for path in candidates {
        if obs_local(repo, &J::from(rel(repo, &path)?)).is_none() {
            continue;
        }
        let doc = match read_json_soft(&path)?.0 {
            Some(d) if d.is_dict() => d,
            _ => continue,
        };
        if doc["config"] != config || doc["measured_node"] != ev["node"] || doc.get_str("sweep_generated_utc") != Some(measured.as_str())
            || doc["sweep_dir"] != sw["dir"]
        {
            continue;
        }
        let target = match obs_local(repo, &doc["installed_runtime_identity"]) {
            Some(t) if t.parent() == path.parent() && t.is_file() => t,
            _ => continue,
        };
        let data = match read_json_soft(&target)?.0 {
            Some(d) if d.is_dict() => d,
            _ => continue,
        };
        let runtime_rel = rel(repo, &target)?;
        let command = match data.get("collection_argv") {
            None => "이 runtime 기록에 collection_argv 필드 없음(다른 실행 원장 미조회)".to_string(),
            Some(argv) => {
                let shape = argv.as_list().map(|l| !l.is_empty() && l.iter().all(|a| a.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false))).unwrap_or(false);
                format!("수집 명령 {}: {runtime_rel}#/collection_argv(실행 성공·진위·값 출처는 이 필드만으로 미검증)", if shape { "기록" } else { "필드(argv 형식 미확인)" })
            }
        };
        let prel = rel(repo, &path)?;
        let source = format!(
            "{runtime_rel} (관측 기록 · {prel}가 config={}, node={}, sweep_dir={}, measured_utc={measured}로 결속; {command}; 컴파일 바이트 일치는 별도 미검증)",
            config.display(),
            ev["node"].display(),
            sw["dir"].display()
        );
        out.push(jd! {"data" => data, "source" => source, "files" => files_j(&[(prel, "raw_json".into()), (runtime_rel, "raw_json".into())])});
    }
    Ok(out)
}

fn runtime_observed_field(observations: &[J], key: &str) -> (Option<String>, String) {
    let mut got: Vec<(String, String)> = Vec::new();
    for o in observations {
        if let Some(v) = o["data"].get_str(key) {
            if !unobs(&J::from(v)) && (key != "source_head" || rx_sha40().is_full(v)) {
                got.push((v.to_string(), format!("{} {key}", o["source"].display())));
            }
        }
    }
    if got.is_empty() {
        return (None, format!("미관측 — 이 측정에 결속된 runtime 관측의 {key} 없음"));
    }
    if got.iter().map(|g| &g.0).collect::<BTreeSet<_>>().len() != 1 {
        return (None, format!("상충하는 runtime 관측 — 고르지 않는다: {}", got.iter().map(|(v, s)| format!("{v} ← {s}")).collect::<Vec<_>>().join(" · ")));
    }
    let mut srcs: Vec<String> = Vec::new();
    for (_, s) in &got {
        if !srcs.contains(s) {
            srcs.push(s.clone());
        }
    }
    (Some(got[0].0.clone()), srcs.join(" · "))
}

pub fn _runtime_observed_field(observations: &J, key: &str) -> HR<(Option<String>, String)> {
    Ok(runtime_observed_field(observations.items(), key))
}

fn distinct(j: &J) -> usize {
    j.as_dict().map(|m| m.values().map(J::repr).collect::<BTreeSet<_>>().len()).unwrap_or(0)
}

/// PAYLOAD.naming.vllm_observed — vLLM 버전 관측 **라벨**(칸마다 producer).
pub fn vllm_observed(ev: &J, repo: Option<&Path>) -> HR<J> {
    if repo.is_none() && !ev["repo"].truthy() {
        return core::fail(
            "HINT_EVIDENCE_REPO_REQUIRED",
            "vllm_observed 에 저장소가 없다(repo 인자 · ev.repo 둘 다 없음).",
            Some("evidence.from_campaign/from_publication 이 만든 CellEvidence 를 넘기거나 repo 를 준다."),
        );
    }
    let rp = core::resolve(&repo.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(ev["repo"].display())));
    let banner = engine_banner(&rp, ev)?;
    let (label, psrc) = _cert_vllm_producer(&rp, ev, &J::Null)?;
    let cert_v = norm(&ev["certificate"]["vllm_version"]);
    let meta = &ev["sweep"]["index"]["meta"];
    let (wheel, mut wheel_src) = _wheel_meta(&rp, ev)?;
    let note = _attestation_timing_note(ev)?;
    let (_wv, _ws, wheel_nodes) = _attested_host(&ev["attestation"], "vllm_dist_version", note.as_deref())?;
    let wheel_basis = if wheel.is_none() && ev.get_str("plane") != Some("native") && distinct(&wheel_nodes) > 1 { Some("observed-node-difference") } else { None };
    if wheel.is_none() {
        wheel_src += &format!(
            "(sweep_index.meta.vllm_build={} 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천)",
            meta.get("vllm_build").map(J::repr).unwrap_or("None".into())
        );
    }
    let observations = _runtime_observations(&rp, ev)?;
    let (rv, rvs) = runtime_observed_field(&observations, "vllm_version");
    let (co, cos) = runtime_observed_field(&observations, "source_head");
    let (de, des) = runtime_observed_field(&observations, "source_describe");
    let mut m = Map::new();
    m.insert("wheel_meta_label", "vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님)");
    m.insert("engine_self_report", banner["value"].clone());
    m.insert("engine_self_report_source", banner["source"].clone());
    m.insert("engine_self_report_basis", banner["basis"].clone());
    m.insert("certificate_vllm_version", cert_v);
    m.insert("certificate_vllm_version_producer", label.as_str());
    m.insert("certificate_vllm_version_source", psrc.as_str());
    m.insert("wheel_meta", J::from(wheel.clone()));
    m.insert("wheel_meta_source", wheel_src.as_str());
    m.insert("wheel_meta_basis", J::from(wheel_basis));
    if !observations.is_empty() {
        m.insert("runtime_version", J::from(rv));
        m.insert("runtime_version_source", rvs);
        m.insert("runtime_source_checkout", J::from(co));
        m.insert("runtime_source_checkout_source", cos);
        m.insert("runtime_source_describe", J::from(de));
        m.insert("runtime_source_describe_source", des);
    }
    let tail = if label == "image-tag-line" || (label == "lite-report-identity" && psrc.contains("derived(envfile IMAGE_TAG)")) { "(빌드 입력 태그 파생 · 엔진 자기보고 아님)" } else { "" };
    let wm = if wheel.as_deref().map(|w| !w.is_empty()).unwrap_or(false) {
        "관측"
    } else if wheel_basis.is_some() {
        "노드별 상이·공통값 미선택"
    } else {
        "미관측"
    };
    m.insert("source", format!("칸별 출처는 *_source — engine_self_report={} · certificate_vllm_version={label}{tail} · wheel_meta={wm}", banner["basis"].display()));
    Ok(J::Dict(m))
}

/// Stub-signature form kept for callers compiled against it: the Python mutates `ev` in place, which `&J` cannot do —
/// fails loudly instead of silently dropping the binding. Call `bind_image_ledger_mut`.
pub fn bind_image_ledger(_ev: &J, _view: &J) -> HR<()> {
    Err(HintError::py("RuntimeError", "evidence.bind_image_ledger mutates the CellEvidence — call evidence::bind_image_ledger_mut(&mut ev, view)"))
}

/// artifacts 가 읽은 대표 빌드 원장의 vllm 블록을 증거에 묶는다 (in place).
pub fn bind_image_ledger_mut(ev: &mut J, view: &J) -> HR<()> {
    if !ev["build_identity"].is_dict() {
        return Ok(());
    }
    if ev["build_identity"].get("source").is_none() {
        bi_set(ev, "source", J::dict());
    }
    let mut led = _ledger_vllm(&ev["attestation"])?;
    let mut lsrc = format!("빌드 원장({} · attestation)", ev["attestation"].get("_path").map(J::display).unwrap_or("None".into()));
    if !led.truthy() && view.is_dict() && view["vllm"].is_dict() && view["vllm"].truthy() {
        led = view["vllm"].clone();
        lsrc = format!(
            "빌드 원장({} · {})",
            view.get("source").map(J::display).unwrap_or("None".into()),
            if view["provenance"].truthy() { view["provenance"].display() } else { "provenance 미기재".into() }
        );
        ev.set("image_ledger", jd! {"vllm" => led.clone(), "source" => lsrc.as_str()});
    }
    if led.truthy() {
        let sha = if led["git_sha"].truthy() { led["git_sha"].display() } else { String::new() }.trim().to_lowercase();
        if !ev["build_identity"]["vllm_sha"].truthy() && rx_sha40().is_match(&sha) {
            bi_set(ev, "vllm_sha", sha.as_str());
            bi_src_set(ev, "vllm_sha", format!("{lsrc} vllm.git_sha(resolved.json 미바인딩 또는 to_sha 없음)"));
        }
        if led["describe"].truthy() {
            bi_set(ev, "vllm_describe", led["describe"].display());
            bi_src_set(ev, "vllm_describe", format!("{lsrc} vllm.describe(git describe --tags · 빌드 시점)"));
        }
    }
    if ev["resolve"].is_dict() && ev["resolve"]["from_ref"].truthy() {
        let fr = ev["resolve"]["from_ref"].display();
        let rs = ev["sources"].get("resolve").map(J::display).unwrap_or("None".into());
        bi_set(ev, "vllm_prev_pin", fr);
        bi_src_set(ev, "vllm_prev_pin", format!("{rs} upstream_delta.from_ref(해소 기록)"));
    }
    Ok(())
}

/// 설치된 vLLM 배포 메타 버전 — (값|None, 출처).
pub fn _wheel_meta(repo: &Path, ev: &J) -> HR<(Option<String>, String)> {
    let note = _attestation_timing_note(ev)?;
    let (v, where_, by_node) = _attested_host(&ev["attestation"], "vllm_dist_version", note.as_deref())?;
    let native = ev.get_str("plane") == Some("native");
    if v.is_some() && !native {
        if distinct(&by_node) > 1 {
            return Ok((None, format!("노드별 상이 — 값을 고르지 않는다: {}", disp_opt(&where_))));
        }
        return Ok((v, disp_opt(&where_)));
    }
    let freezes = if ev["pip_freeze_paths"].truthy() {
        ev["pip_freeze_paths"].clone()
    } else if ev["pip_freeze_path"].truthy() {
        jd! {"main" => ev["pip_freeze_path"].clone()}
    } else {
        J::dict()
    };
    let mut items: Vec<(String, J)> = freezes.as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()).unwrap_or_default();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let mut got: Vec<(String, String)> = Vec::new();
    for (n, relv) in &items {
        let p = repo.join(relv.display());
        if !p.is_file() {
            continue;
        }
        let text = lossy(&std::fs::read(&p).map_err(|e| py_io(&e, &p))?).replace("\r\n", "\n");
        for ln in splitlines(&text) {
            if let Some(m) = rx_vllm_pin().match_at_start(ln.trim()) {
                got.push((n.clone(), m[1].to_string()));
                break;
            }
        }
    }
    if !got.is_empty() {
        let where2 = got.iter().map(|(n, _)| format!("{n}={}", freezes[n.as_str()].display())).collect::<Vec<_>>().join(" · ");
        if got.iter().map(|g| &g.1).collect::<BTreeSet<_>>().len() > 1 {
            return Ok((None, format!("native pip freeze `vllm==` 노드별 상이({where2}) — 값을 고르지 않는다")));
        }
        return Ok((Some(got[0].1.clone()), format!("native pip freeze `vllm==` 줄({where2} · 노드별 관측 · 일치)")));
    }
    if v.is_some() && distinct(&by_node) == 1 {
        return Ok((v, format!("{} · ⚠ native 셀인데 pip freeze 에 vllm 줄이 없어 원천 이미지 실행의 관측을 쓴다", disp_opt(&where_))));
    }
    let mut led = _ledger_vllm(&ev["attestation"])?;
    let mut lsrc = format!("빌드 원장({} · attestation) vllm.dist_version", ev["attestation"].get("_path").map(J::display).unwrap_or("None".into()));
    if !led.truthy() && ev["image_ledger"].is_dict() {
        led = if ev["image_ledger"]["vllm"].truthy() { ev["image_ledger"]["vllm"].clone() } else { J::dict() };
        lsrc = format!("{} vllm.dist_version", ev["image_ledger"].get("source").map(J::display).unwrap_or("None".into()));
    }
    if led["dist_version"].truthy() && !native {
        return Ok((Some(led["dist_version"].display()), format!("{lsrc}(빌드 시점 measured · 이미지 안 배포 메타)")));
    }
    let observations = _runtime_observations(repo, ev)?;
    let candidates: Vec<J> = observations
        .iter()
        .filter(|o| o["data"].get_str("distribution_metadata_version").map(|s| !unobs(&J::from(s))).unwrap_or(false))
        .cloned()
        .collect();
    let explicit: Vec<J> = candidates
        .iter()
        .filter(|o| o["data"].get_str("distribution_metadata_version_source").map(|s| !s.trim().is_empty()).unwrap_or(false))
        .cloned()
        .collect();
    if explicit.len() != candidates.len() {
        return Ok((None, format!("배포 메타 값은 기록되었으나 명시적 출처가 없어 미검증 — 값을 고르지 않는다: {}", candidates.iter().map(|o| o["source"].display()).collect::<Vec<_>>().join(" · "))));
    }
    let (recorded, recorded_source) = runtime_observed_field(&explicit, "distribution_metadata_version");
    if !candidates.is_empty() && recorded.is_none() {
        return Ok((None, recorded_source));
    }
    if let Some(r) = recorded {
        let sources = explicit
            .iter()
            .map(|o| format!("{} distribution_metadata_version_source={}", o["source"].display(), o["data"]["distribution_metadata_version_source"].repr()))
            .collect::<Vec<_>>()
            .join(" · ");
        return Ok((Some(r), format!("{recorded_source} · 기록된 출처(선언 · 검증되지 않음): {sources} · 수집 성공은 별도 검증")));
    }
    Ok((None, "미관측 — 결속된 명시적 배포 메타 기록 · attestation vllm_dist_version · native pip freeze · 빌드 원장 vllm.dist_version에 값 없음(runtime_version 기록은 배포 메타로 간주하지 않는다)".to_string()))
}

pub const _ATTEST_TIME_KEYS: [&str; 4] = ["written_utc", "captured_utc", "generated_utc", "utc"];
pub const ATTESTATION_SCOPE_OTHER_RUN: &str = "image-build/smoke — not this cell's run";

/// 묶인 노드 정합 attestation 의 **범위** 라벨(공유 계약 `facts['attestation_scope']`).
pub fn attestation_scope(repo: &Path, ev: &J) -> HR<Option<J>> {
    let repo = core::resolve(repo);
    let att = &ev["attestation"];
    if !att.is_dict() || !att["_path"].truthy() {
        return Ok(None);
    }
    let cfg = att["config"].clone();
    let (written, wsrc) = _attestation_written(&repo, att)?;
    let phase = if att["phase"].truthy() {
        att["phase"].clone()
    } else if att["checks"].truthy() {
        J::from("build(v1 · --build 대조 행)")
    } else {
        J::from("미기재")
    };
    let (timing, tnote, win) = _attestation_timing(&repo, ev, written.as_deref())?;
    let (c, p) = (cfg.display(), phase.display());
    let timing_s = timing.as_deref();
    let mut scope: Option<String> = None;
    if cfg != ev["cell"] {
        scope = Some(ATTESTATION_SCOPE_OTHER_RUN.to_string());
    } else if timing_s == Some("after-measurement") {
        scope = Some(format!("same config({c}) · {p} — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증 · 파일은 다음 실행이 덮는다)"));
    } else if timing_s == Some("before-measurement") {
        if let Some(sb) = attestation_same_boot(&repo, ev, written.as_deref(), &J::from(win.clone()))?.filter(|s| s.truthy()) {
            let mark = if sb.get_str("strength") == Some("strong") { SAME_BOOT_MARK } else { WEAK_SAME_BOOT_MARK };
            scope = Some(format!("same config({c}) · {p} — {mark}(판별: {})", sb["basis"].display()));
        } else {
            scope = Some(format!("same config({c}) · {p} — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 기동과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)"));
        }
    } else if timing_s == Some("during-measurement") {
        scope = Some(format!("same config({c}) · {p} — 측정 중의 관측(측정을 서빙한 실행 · 파일은 다음 실행이 덮는다)"));
    }
    let scope = scope.unwrap_or_else(|| format!("same config({c}) · {p} — 이 측정 실행과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)"));
    let mut out = jd! {"config" => cfg, "written_utc" => written, "written_utc_source" => wsrc, "bound_by" => att.get("_bound_by").cloned().unwrap_or(J::Null),
        "phase" => phase, "path" => att["_path"].clone(), "scope" => scope};
    if tnote.as_deref().map(|t| !t.is_empty()).unwrap_or(false) {
        out.set("timing", J::from(timing));
        out.set("timing_note", J::from(tnote));
        out.set("measure_window", J::from(win));
    }
    Ok(Some(out))
}

/// attestation 작성 시각 = 문서의 시각 칸 · 없으면 파일 mtime.
pub fn _attestation_written(repo: &Path, att: &J) -> HR<(Option<String>, String)> {
    for k in _ATTEST_TIME_KEYS {
        if let Some(v) = att.get_str(k) {
            if rx_utc().is_full(v) {
                return Ok((Some(v.to_string()), format!("attestation {k}")));
            }
        }
    }
    if let Some(p) = att.get_str("_path") {
        if let Some(mt) = mtime_epoch(&repo.join(p)) {
            return Ok((Some(utc_of_epoch(mt)), format!("{p} 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)")));
        }
    }
    Ok((None, "미관측(시각 칸 없음 · 파일 stat 불가)".to_string()))
}

/// 호스트 사실 출처에 붙일 작성 시각 대 측정 창 문장(F7).
pub fn _attestation_timing_note(ev: &J) -> HR<Option<String>> {
    let att = &ev["attestation"];
    if !att.is_dict() || !att["_path"].truthy() || !ev["repo"].truthy() {
        return Ok(None);
    }
    let repo = core::resolve(Path::new(&ev["repo"].display()));
    let written = _attestation_written(&repo, att)?.0;
    let (timing, mut tnote, win) = _attestation_timing(&repo, ev, written.as_deref())?;
    if timing.as_deref() == Some("before-measurement") && att["config"] == ev["cell"] {
        let sb = attestation_same_boot(&repo, ev, written.as_deref(), &J::from(win))?;
        if let Some(sb) = sb.filter(|s| s.truthy()) {
            let f0 = sb["files"].at(0).display();
            let t = disp_opt(&tnote);
            tnote = Some(if sb.get_str("strength") == Some("strong") {
                format!("{t} · {SAME_BOOT_MARK}(판별 근거 = attestation 범위 줄 · 스모크 로그 {f0})")
            } else {
                format!("{t} · {WEAK_SAME_BOOT_MARK}(판별 근거 = attestation 범위 줄 · 원장 {f0})")
            });
        }
    }
    Ok(tnote)
}

pub const SAME_BOOT_MARK: &str = "측정을 서빙한 같은 기동의 스모크 직후 관측";
pub const WEAK_SAME_BOOT_MARK: &str = "원장 순서로 판별한 같은 기동의 관측(결속된 스모크 로그 미확인 · 제한검색 · 약한 판별)";
re_static!(_SERVER_STARTING, r"Starting vLLM server on ");
re_static!(_APISERVER_PID, r"\(APIServer pid=(\d+)\)");
re_static!(_HONORED_TS, r#"budget_honored\b.*?"ts"\s*:\s*"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)""#);

pub fn _smoke_serve_logs(repo: &Path) -> HR<Vec<PathBuf>> {
    let base = repo.join("docs").join("simlog");
    if !base.is_dir() {
        return Ok(Vec::new());
    }
    let pred = |n: &str| infix_suffix(n, "serve", ".log");
    let mut set: BTreeSet<PathBuf> = BTreeSet::new();
    for d1 in glob_dir(&base, |_| true).into_iter().filter(|p| p.is_dir()) {
        set.extend(glob_dir(&d1, pred));
        for d2 in glob_dir(&d1, |_| true).into_iter().filter(|p| p.is_dir()) {
            set.extend(glob_dir(&d2, pred));
        }
    }
    Ok(set.into_iter().filter(|p| p.is_file() && !is_symlink(p) && !pname(p).contains("watchdog")).collect())
}

/// 측정 엔진 로그의 첫 `Starting vLLM server on` 줄 시각(UTC 가정 · 연도 = year_hint) · 출처 `파일:줄`.
pub fn _api_server_start(repo: &Path, ev: &J, year_hint: &str) -> HR<(Option<String>, Option<String>)> {
    let dt0 = utc_dt(Some(year_hint));
    for lp in measured_engine_logs_ex_t(repo, ev)?.0 {
        for (i, line) in _log_lines(&lp)?.iter().enumerate() {
            let clean = rx_ansi().sub_literal(line, "");
            if !_SERVER_STARTING().is_search(&clean) {
                continue;
            }
            let (tm, d0) = match (rx_engine_ts().search(&clean), dt0) {
                (Some(t), Some(d)) => (t, d),
                _ => continue,
            };
            let n = |k: usize| tm[k].parse::<i64>().unwrap_or(-1);
            if let Some(c) = mk_civil(d0.year, n(1), n(2), n(3), n(4), n(5)) {
                return Ok((Some(iso_z(&c)), Some(format!("{}:{}", rel(repo, &lp)?, i + 1))));
            }
        }
    }
    Ok((None, None))
}

/// G10 판별 — {basis, files[], strength: strong|weak} | None(판별 불가).
pub fn attestation_same_boot(repo: &Path, ev: &J, written: Option<&str>, win: &J) -> HR<Option<J>> {
    let repo = core::resolve(repo);
    let repo = repo.as_path();
    let att = if ev["attestation"].is_dict() { ev["attestation"].clone() } else { J::dict() };
    let end = win["end_utc"].clone();
    let start = win["start_utc"].clone();
    let written = match written {
        Some(w) if !w.is_empty() => w.to_string(),
        _ => return Ok(None),
    };
    if !end.truthy() || !att["_path"].truthy() || att["config"] != ev["cell"] {
        return Ok(None);
    }
    let end = end.display();
    let cell = ev_s(ev, "cell");
    let label = format!("smoke-{cell}");
    type Row = (String, i64, J, String);
    let mut decl: Option<(String, Row, BTreeSet<String>)> = None;
    for node in _event_node_dirs(repo, ev)? {
        let mut rows: Vec<Row> = Vec::new();
        for f in _node_event_files(repo, &node)?.0 {
            let fr = rel(repo, &f)?;
            for (ts, ln, d) in _event_rows(&f)?.0 {
                rows.push((ts, ln, d, fr.clone()));
            }
        }
        rows.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
        if rows.is_empty() || rows[rows.len() - 1].0 < end {
            continue;
        }
        let ds: Vec<&Row> = rows
            .iter()
            .filter(|r| r.2.get_str("kind") == Some("budget_declare") && r.2.get_str("label") == Some(label.as_str()) && r.0 <= written)
            .collect();
        let d = match ds.last() {
            Some(d) => (*d).clone(),
            None => continue,
        };
        let brk = rows.iter().any(|r| {
            d.0 < r.0 && r.0 <= end
                && (r.2.get_str("kind") == Some("budget_declare") || (r.2.get_str("kind") == Some("budget_clear") && r.2["existed"] == J::Bool(true)))
        });
        if brk {
            return Ok(None);
        }
        let honored: BTreeSet<String> = rows
            .iter()
            .filter(|r| r.2.get_str("kind") == Some("budget_honored") && r.2.get_str("label") == Some(label.as_str()) && d.0 <= r.0 && r.0 <= written)
            .map(|r| r.0.clone())
            .collect();
        if decl.is_none() || node == "main" {
            decl = Some((node.clone(), d, honored));
        }
    }
    let (node, d, ledger_honored) = match decl {
        Some(x) => x,
        None => return Ok(None),
    };
    let mut pids: BTreeSet<String> = BTreeSet::new();
    for lp in measured_engine_logs_ex_t(repo, ev)?.0 {
        let b = std::fs::read(&lp).map_err(|e| py_io(&e, &lp))?;
        pids.extend(_APISERVER_PID().findall(&rx_ansi().sub_literal(&lossy(&b), "")));
    }
    if pids.len() > 1 {
        return Ok(None);
    }
    let pid_txt = format!("측정 엔진 로그 APIServer pid {}", pids.iter().next().map(|p| format!("{p} 하나")).unwrap_or("미관측".into()));
    let mut same_decl = ledger_honored.clone();
    same_decl.insert(d.0.clone());
    let mut hit: Option<(PathBuf, usize, usize, usize, String)> = None;
    let mut seen: Vec<String> = Vec::new();
    let ap = att["_path"].display();
    for lg in _smoke_serve_logs(repo)? {
        let lines = _log_lines(&lg)?;
        if lines.iter().any(|x| x.contains(&ap)) {
            seen.push(rel(repo, &lg)?);
        }
        let i_pass = lines.iter().position(|x| x.contains("SMOKE PASS"));
        let i_att = i_pass.and_then(|p| lines.iter().enumerate().position(|(i, x)| i > p && x.contains(&ap)));
        let i_keep = i_att.and_then(|a| lines.iter().enumerate().position(|(i, x)| i > a && x.contains("--keep-up")));
        let honored: BTreeSet<String> = lines.iter().filter_map(|x| _HONORED_TS().search(x).map(|m| m[1].to_string())).collect();
        let inter: Vec<&String> = honored.intersection(&same_decl).collect();
        if let (Some(k), false) = (i_keep, inter.is_empty()) {
            hit = Some((lg.clone(), i_pass.unwrap() + 1, i_att.unwrap() + 1, k + 1, inter[0].clone()));
            break;
        }
    }
    if let Some((lg, a, b, c, hts)) = hit {
        let basis = format!(
            "스모크 로그 {} L{a} SMOKE PASS → L{b} attestation 작성 → L{c} --keep-up · 원장 {}:{} budget_declare({label} · {}) 가 측정 끝 {end} 까지 재선언 · 창 닫힘 없이 이어짐({node} 원장 기록 범위 안) · 스모크 로그 budget_honored ts {hts} = 그 선언{} · {pid_txt}{}",
            rel(repo, &lg)?,
            d.3,
            d.1,
            d.0,
            if hts == d.0 { "" } else { "의 원장 budget_honored" },
            if start.truthy() { format!(" · 측정 시작 {}", start.display()) } else { String::new() }
        );
        return Ok(Some(jd! {"basis" => basis, "files" => jl![rel(repo, &lg)?, d.3.as_str()], "strength" => "strong"}));
    }
    if !seen.is_empty() || !start.truthy() {
        return Ok(None);
    }
    let start = start.display();
    if !(d.0 <= written && written < start) {
        return Ok(None);
    }
    let mut times = vec![format!("attestation 작성 {written}")];
    let sp = &ev["serve_proof"];
    let sp_rel = ev["sources"]["serve_proof"].as_str();
    if sp.is_dict() && sp["config"] == ev["cell"] && sp_rel.map(|r| repo.join(r).is_file()).unwrap_or(false) {
        let mut spj = sp.clone();
        spj.set("_path", sp_rel.unwrap());
        let spw = _attestation_written(repo, &spj)?.0;
        match spw {
            Some(w) if !w.is_empty() && d.0 <= w && w < start => times.push(format!("serve_proof 작성 {w}")),
            _ => return Ok(None),
        }
    } else {
        times.push("serve_proof 미관측".into());
    }
    let (srv, srv_src) = _api_server_start(repo, ev, &d.0)?;
    if let Some(s) = &srv {
        if !s.is_empty() && !(d.0 <= *s && *s <= written) {
            return Ok(None);
        }
    }
    let srv_txt = match (&srv, &srv_src) {
        (Some(s), src) if !s.is_empty() => format!("API 서버 기동 {s}({} · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성]", disp_opt(src)),
        _ => "API 서버 기동 시각 미관측(모순 검사 불가)".into(),
    };
    let basis = format!(
        "원장 순서로 판별(결속된 스모크 로그 미확인 · 제한검색 — docs/simlog/**/*serve*.log 에서 이 attestation 을 적은 로그 미확인; 다른 경로의 보존 여부는 판정하지 않음): 원장 {}:{} budget_declare({label} · {}) ≤ {} < 측정 시작 {start} · 그 선언이 측정 끝 {end} 까지 재선언 · 창 닫힘 없이 이어짐({node} 원장 기록 범위 안) · {srv_txt} · {pid_txt}",
        d.3,
        d.1,
        d.0,
        times.join(" · ")
    );
    let mut files = vec![J::from(d.3.as_str())];
    if let Some(s) = srv_src.filter(|s| !s.is_empty()) {
        files.push(J::from(s.rsplit_once(':').map(|x| x.0).unwrap_or(&s)));
    }
    Ok(Some(jd! {"basis" => basis, "files" => J::List(files), "strength" => "weak"}))
}

re_static!(_BENCH_JSON_DATE, r"^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$");
pub const LITE_START_NOTE: &str = "lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각";

/// lite-only 셀의 측정 창과 레그 — {start_utc, start_source, end_utc, end_source, raw, raw_doc, legs} | None.
pub fn lite_window(repo: &Path, ev: &J) -> HR<Option<J>> {
    let repo = core::resolve(repo);
    if ev["sweep"].truthy() || ev.get_str("report_kind") != Some("lite") {
        return Ok(None);
    }
    let (d, raw, _jwhy) = lite_join(&repo, ev)?;
    let (d, raw) = match (d, raw) {
        (Some(d), Some(r)) => (d, r),
        _ => return Ok(None),
    };
    let cell = ev_s(ev, "cell");
    let raw_p = d.join(format!("lite_raw_{cell}.json"));
    let born = raw["measured_utc"].display();
    let mut legs = Map::new();
    for leg in ["cold", "warm"] {
        let p = d.join(format!("lite_{leg}_{cell}.json"));
        let bj = &raw[format!("bench_{leg}_json").as_str()];
        let named = pname(Path::new(&if bj.truthy() { bj.display() } else { String::new() }));
        if !named.is_empty() && named != pname(&p) {
            continue;
        }
        if let Some(doc) = read_json_soft(&p)?.0.filter(|x| x.is_dict()) {
            legs.insert(leg, jd! {"path" => rel(&repo, &p)?, "doc" => doc});
        }
    }
    let mut end: Option<String> = None;
    let mut esrc: Option<String> = None;
    let mut dates: Vec<(String, String)> = Vec::new();
    for (_leg, x) in legs.iter() {
        let dt = &x["doc"]["date"];
        let ds = if dt.truthy() { dt.display() } else { String::new() };
        if let Some(m) = _BENCH_JSON_DATE().match_at_start(&ds) {
            let u = format!("{}-{}-{}T{}:{}:{}Z", &m[1], &m[2], &m[3], &m[4], &m[5], &m[6]);
            if u >= born {
                dates.push((u, x["path"].display()));
            }
        }
    }
    if let Some((u, p)) = dates.into_iter().max() {
        end = Some(u);
        esrc = Some(format!("{p} date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정)"));
    } else if let Some(mt) = mtime_epoch(&raw_p).map(utc_of_epoch) {
        if mt >= born {
            esrc = Some(format!("{} 파일 mtime(파일시스템 관측 · 레그 뒤에 쓰인다)", rel(&repo, &raw_p)?));
            end = Some(mt);
        }
    }
    let rr = rel(&repo, &raw_p)?;
    Ok(Some(jd! {"start_utc" => raw["measured_utc"].clone(), "start_source" => format!("{rr} measured_utc(= bench_report 머리 생성일 · {LITE_START_NOTE})"),
        "end_utc" => end, "end_source" => esrc, "raw" => rr.as_str(), "raw_doc" => raw.clone(), "legs" => J::Dict(legs)}))
}

re_static!(_TOOL_FLAG_RE, r#"(--[a-z][a-z0-9-]*)(?:[ =]([^\s\\'"$-][^\s\\'"$]*))?"#);

/// 측정 시점 판본 lite_bench.sh 의 첫 `vllm bench serve` 호출 인자 — {flags, source, measured_line, tool} | None.
pub fn lite_tool_args(repo: &Path, ev: &J) -> HR<Option<J>> {
    let repo = core::resolve(repo);
    let sel = _tool_selection(&repo, ev)?;
    let t = match sel["tools"].items().iter().find(|x| x.get_str("role") == Some("lite")) {
        Some(t) => t.clone(),
        None => return Ok(None),
    };
    if !sel["at"].truthy() {
        return Ok(None);
    }
    let at = sel["at"].display();
    let rp = t["repo_path"].display();
    let raw = match git_text(&repo, &at, &rp)? {
        Some(r) if !r.is_empty() => r,
        _ => return Ok(None),
    };
    let rv = _tool_revision(&repo, &at, &rp)?.unwrap_or_else(J::dict);
    let text = lossy(&raw);
    let lines: Vec<&str> = text.split('\n').collect();
    let tag = format!("{}@{}", t["name"].display(), core::head_chars(&or(&rv["git_rev"], &sel["at"]).display(), 12));
    let mline = lines.iter().position(|ln| rx_measured_utc_line().is_match(ln)).map(|i| format!("L{}", i + 1));
    for (i, ln) in lines.iter().enumerate() {
        let code = ln.split('#').next().unwrap_or("");
        let pos = match code.find("bench serve") {
            Some(p) => p,
            None => continue,
        };
        let mut j = i;
        let mut body: Vec<String> = vec![code[pos + "bench serve".len()..].to_string()];
        while lines[j].trim_end().ends_with('\\') && j + 1 < lines.len() {
            j += 1;
            body.push(lines[j].to_string());
        }
        let mut flags: Vec<(String, Option<String>)> = Vec::new();
        for part in &body {
            let head = part.split('#').next().unwrap_or("");
            for m in _TOOL_FLAG_RE().finditer(head) {
                let flag = m[1].to_string();
                let val = m.get(2).map(|x| x.as_str().to_string());
                let e = m.get(0).unwrap().end();
                let after: String = part[e..].chars().take(2).collect::<String>().trim_start().to_string();
                let first: String = after.chars().take(1).collect();
                if val.is_none() && !(first.is_empty() || first == "-" || first == "\\") {
                    continue;
                }
                if !flags.iter().any(|(f, _)| *f == flag) {
                    flags.push((flag, val));
                }
            }
        }
        let fj = J::List(flags.into_iter().map(|(f, v)| jl![f, J::from(v)]).collect());
        return Ok(Some(jd! {"flags" => fj, "source" => format!("{tag} L{}-L{}", i + 1, j + 1), "measured_line" => mline, "tool" => tag.as_str()}));
    }
    Ok(None)
}

/// 이 측정의 창 (시작 | None, 끝 | None, 출처).
pub fn _measure_window(repo: &Path, ev: &J) -> HR<(Option<String>, Option<String>, String)> {
    if let Some(lw) = lite_window(repo, ev)?.filter(|l| l.truthy()) {
        let es = if lw["end_source"].truthy() { lw["end_source"].display() } else { "미관측".into() };
        return Ok((opt_str(&lw["start_utc"]), opt_str(&lw["end_utc"]), format!("시작 = {} · 끝 = {es}", lw["start_source"].display())));
    }
    let end = _measured_key(repo, ev)?;
    let (mut start, mut ssrc) = crate::artifacts::_first_measure_utc(repo, ev)?;
    if let (Some(s), Some(e)) = (&start, &end) {
        if !s.is_empty() && !e.is_empty() && s > e {
            ssrc = Some(format!("{}(date 를 UTC 로 읽으면 측정 끝 뒤 — 시계 가정 불성립 · 버림)", disp_opt(&ssrc)));
            start = None;
        }
    }
    let ss = ssrc.filter(|s| !s.is_empty()).unwrap_or_else(|| "미관측".into());
    Ok((start, end, format!("시작 {ss} · 끝 = 측정 키(인증서 measured_utc · 스윕 generated_utc · lite 리포트 생성일)")))
}

/// attestation 작성 시각 대 측정 창 — (분류, 문장, 창).
pub fn _attestation_timing(repo: &Path, ev: &J, written: Option<&str>) -> HR<(Option<String>, Option<String>, Option<J>)> {
    let written = match written {
        Some(w) if !w.is_empty() => w,
        _ => return Ok((None, None, None)),
    };
    let (start, end, src) = _measure_window(repo, ev)?;
    let end = match end {
        Some(e) if !e.is_empty() => e,
        _ => return Ok((None, None, None)),
    };
    let start = start.filter(|s| !s.is_empty());
    let win = jd! {"start_utc" => start.clone(), "end_utc" => end.as_str(), "source" => src};
    if written > end.as_str() {
        return Ok((Some("after-measurement".into()), Some(format!("작성 {written} > 측정 끝 {end} — 측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증)")), Some(win)));
    }
    if let Some(s) = &start {
        if written < s.as_str() {
            return Ok((Some("before-measurement".into()), Some(format!("작성 {written} < 측정 시작 {s} — 측정 전(빌드·스모크)의 관측")), Some(win)));
        }
        return Ok((Some("during-measurement".into()), Some(format!("측정 창 {s} → {end} 안의 작성 {written} — 측정 중의 관측")), Some(win)));
    }
    Ok((Some("before-end".into()), Some(format!("작성 {written} ≤ 측정 끝 {end}(측정 시작 미관측 — 측정 전·중 미판별)")), Some(win)))
}

// ── 증거 조립(두 해소기의 공통 꼬리) ────────────────────────────────────────────────────────────────────────────
fn strs_of(j: &J) -> Vec<String> {
    j.items().iter().map(J::display).collect()
}

fn sorted_set(v: Vec<String>) -> Vec<String> {
    v.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
}

/// 두 해소기의 공통 꼬리. `runner` = docker 주입 훅(J::Null = 기본).
pub fn _assemble(repo: &Path, ev: &J, measured_key: Option<&str>, simlog_dirs: &J, _runner: &J /* callable */) -> HR<J> {
    _assemble_r(repo, ev, measured_key, simlog_dirs, None)
}

/// `_assemble` with a typed docker runner hook (None = real `docker`).
pub fn _assemble_r(repo: &Path, ev: &J, measured_key: Option<&str>, simlog_dirs: &J, runner: Option<DockerRunner>) -> HR<J> {
    let mut ev = ev.clone();
    let mut missing: Vec<String> = strs_of(&ev["missing"]);
    let topo = ev_s(&ev, "topology");
    let cell = ev_s(&ev, "cell");
    ev.set("repo", core::posix(repo));
    let triplet = _triplet(repo, &topo, &cell)?;
    let tlen = triplet.len();
    ev.set("triplet", triplet);
    sources_set(&mut ev, "triplet", format!("output/{topo}/(configs|envs) · 이름 = 셀 id(I14 · 2026-09-11 R10)"));
    if tlen != 3 {
        missing.push("HINT_MISSING_TRIPLET".into());
    }
    let (rep_missing, report_kind) = _report_missing(repo, &ev["certificate"], ev.get_str("bench_report_path"))?;
    ev.set("report_kind", J::from(report_kind.clone()));
    missing.extend(rep_missing);
    sources_set(
        &mut ev,
        "report_kind",
        if report_kind.as_deref().map(|r| !r.is_empty()).unwrap_or(false) { "bench_report 머리 `mode: lite` 줄(render_bench_section.is_lite_report)" } else { "바인딩할 bench_report 없음" },
    );
    sources_set(&mut ev, "missing", "evidence.MISSING_CODES 어휘(부재는 기재 · 차단은 양성 검출 — 2026-09-06 Q7/Q8)");
    if report_kind.as_deref() == Some("lite") && (!ev["certificate"].truthy() || cert_is_lite(&ev["certificate"])?) {
        ev.set("sweep", J::Null);
        sources_set(&mut ev, "sweep", "lite 셀(경량 리포트) — 스윕 조인 대상 아님");
    } else {
        let measurement = or(&ev["cell_status"]["measurement"], &J::Null).clone();
        let (sw, sm, ssrc) = _bind_sweep(repo, &topo, &cell, measured_key, simlog_dirs, &measurement["sweep_dir"], &measurement["sweep_config"], &measurement["sweep_generated_utc"])?;
        ev.set("sweep", J::from(sw));
        sources_set(&mut ev, "sweep", ssrc);
        missing.extend(sm);
    }
    if !ev["certificate"].truthy() && missing.iter().any(|c| c == "HINT_MISSING_CERTIFICATE") {
        let not_due = _certificate_not_due(repo, &ev)?;
        if not_due.as_deref().map(|s| !s.is_empty()).unwrap_or(false) {
            missing.retain(|c| c != "HINT_MISSING_CERTIFICATE");
        }
        let v = not_due.filter(|s| !s.is_empty()).unwrap_or_else(|| "인증서가 발행되는 판정(explicit ∧ PASS) 또는 판정·권한 미관측 — 부재 = 결손".into());
        sources_set(&mut ev, "certificate_due", v);
    }
    let (sp, sp_rel) = _serve_proof(repo, &topo, &cell)?;
    ev.set("serve_proof", J::from(sp));
    sources_set(&mut ev, "serve_proof", sp_rel.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("output/{topo}/benchlog/serve_proof_{cell}.json 부재")));
    let native = _native_producer_evidence(repo, &topo, &cell, &ev["serve_proof"], sp_rel.as_deref())?;
    if native.truthy() {
        for k in ["plane", "native_install_path", "pip_freeze_path", "pip_freeze_paths", "cleanup_attestation_path", "cleanup_attestation", "native_launch_path", "native_launch_error"] {
            ev.set(k, native[k].clone());
        }
        let s = native["source"].display();
        let nl = if native["native_launch_path"].truthy() {
            format!("{s}#native_launch_path(preserved repo-relative)")
        } else {
            format!(
                "{s}#native_launch_path 부재{}",
                if native["native_launch_error"].truthy() { format!(" · producer 사유 native_launch_error: {}", native["native_launch_error"].display()) } else { " · 옛 proof(필드 없음)".into() }
            )
        };
        sources_set(&mut ev, "native_launch_path", nl);
        sources_set(&mut ev, "plane", format!("{s}#plane(observed native)"));
        sources_set(&mut ev, "native_install_path", format!("{s}#native_install_path(preserved repo-relative)"));
        sources_set(&mut ev, "pip_freeze_path", format!("{s}#pip_freeze_paths.main(preserved repo-relative)"));
        sources_set(&mut ev, "cleanup_attestation", format!("{s}#cleanup_attestation_path → {}", native["cleanup_attestation_path"].display()));
        let mut seeds = ev["lineage_seeds"].clone();
        if !seeds.is_dict() {
            seeds = J::dict();
        }
        seeds.set("native_cleanup_attestation", native["cleanup_attestation_path"].clone());
        ev.set("lineage_seeds", seeds);
    }
    let (man, msrc) = _manifest(repo, &topo)?;
    ev.set("manifest", J::from(man));
    sources_set(&mut ev, "manifest", msrc);
    let env_rel = ev["triplet"].get_str("env").map(str::to_string);
    let env = _env_first(repo, env_rel.as_deref())?;
    let measured_meta = _measured_meta(&ev["sweep"], &ev["certificate"])?;
    let (bi, bm) = _build_identity_r(repo, &topo, &env, env_rel.as_deref(), &measured_meta, &ev["manifest"], runner)?;
    ev.set("build_identity", bi);
    missing.extend(bm);
    if ev["build_identity"].get_str("track") == Some("source-build") {
        bi_set(&mut ev, "source_coordinate_scope", "소스 기준점 — 변경 작업트리·컴파일 바이트 일치는 별도 증거 필요");
    }
    sources_set(&mut ev, "build_identity", "build_identity.source(필드별)");
    let (resolve, rsrc) = _bind_resolve(repo, &topo, &ev["build_identity"])?;
    ev.set("resolve", J::from(resolve));
    sources_set(&mut ev, "resolve", rsrc);
    let ref_ = { let r = &ev["build_identity"]["vllm_ref"]; if r.truthy() { r.display() } else { String::new() } }.trim().to_lowercase();
    let ref_sha = rx_sha40().is_match(&ref_);
    if ref_sha {
        bi_set(&mut ev, "vllm_sha", ref_.as_str());
        let src = format!("VLLM_REF(40자 커밋 · {})", ev["build_identity"]["source"].get("vllm_ref").map(J::display).unwrap_or("None".into()));
        bi_src_set(&mut ev, "vllm_sha", src);
    }
    let fork = crate::naming::vllm_repo_kind(&ev["build_identity"]["vllm_repo"])?.as_deref() == Some("fork");
    if ev["resolve"].is_null() {
        missing.push("HINT_MISSING_RESOLVE".into());
    } else {
        if !ref_sha && ev["resolve"]["to_sha"].truthy() && !fork {
            let to = ev["resolve"]["to_sha"].clone();
            bi_set(&mut ev, "vllm_sha", to);
            let s = format!("{} upstream_delta.to_sha", ev["sources"]["resolve"].display());
            bi_src_set(&mut ev, "vllm_sha", s);
        }
        let ngc = ev["build_identity"]["ngc"].clone();
        let ngc_tag = { let t = &ev["resolve"]["ngc_tag"]; if t.truthy() { t.display() } else { String::new() } };
        if ev["resolve"]["ngc_image"].truthy() && ngc.truthy() && ngc_tag.starts_with(&ngc.display()) {
            let img = ev["resolve"]["ngc_image"].clone();
            bi_set(&mut ev, "ngc", img);
            let s = format!("{} = resolved.json ngc_base.image 접두 일치", ev["build_identity"]["source"]["ngc"].display());
            bi_src_set(&mut ev, "ngc", s);
        }
    }
    let node = ev_s(&ev, "node");
    let (att, am, asrc) = _bind_attestation(repo, &topo, &node, &cell, &ev["build_identity"])?;
    ev.set("attestation", J::from(att));
    missing.extend(am);
    sources_set(&mut ev, "attestation", asrc);
    // `&mut ev` coerces to `&J` while the e1 stubs take `&J` and stays valid when they become `&mut J`.
    _host_facts(&mut ev, Some(repo))?;
    _local_image_digest_r(repo, &mut ev, runner)?;
    let led = _ledger_vllm(&ev["attestation"])?;
    let led_sha = if led["git_sha"].truthy() { led["git_sha"].display() } else { String::new() }.trim().to_lowercase();
    let att_path = ev["attestation"].get("_path").map(J::display).unwrap_or("None".into());
    if fork && !ref_sha {
        let ok = rx_sha40().is_match(&led_sha);
        bi_set(&mut ev, "vllm_sha", if ok { J::from(led_sha.as_str()) } else { J::Null });
        let s = if ok {
            format!("빌드 원장 vllm.git_sha({att_path} · 포크 VLLM_REPO — resolved.json 은 업스트림 해소라 쓰지 않는다)")
        } else {
            "미관측(포크 VLLM_REPO · 빌드 원장 vllm.git_sha 없음 — D-h: 이름은 차단된다)".to_string()
        };
        bi_src_set(&mut ev, "vllm_sha", s);
    } else if !ev["build_identity"]["vllm_sha"].truthy() && rx_sha40().is_match(&led_sha) {
        let par = &ev["attestation"]["parity"];
        let pv = if par.is_dict() { or(&par["vllm_sha"], &J::Null).clone() } else { J::Null };
        let desc = &led["describe"];
        let s = format!(
            "빌드 원장 vllm.git_sha({att_path} · measured build-time in-image{}{} — resolved.json 미바인딩)",
            if desc.truthy() { format!(" · describe={}", desc.display()) } else { String::new() },
            if pv.is_dict() && pv["verdict"].truthy() { format!(" · parity.vllm_sha={}", pv["verdict"].display()) } else { String::new() }
        );
        bi_set(&mut ev, "vllm_sha", led_sha.as_str());
        bi_src_set(&mut ev, "vllm_sha", s);
    }
    if !repo.join(core::REL_PII_TERMS).exists() {
        missing.push("HINT_MISSING_PII_TERMS".into());
    }
    ev.set("missing", J::from(sorted_set(missing)));
    let mc = measurement_config(repo, &ev)?;
    if mc.get_str("bench_mode") == Some("lite") {
        let mut m = strs_of(&ev["missing"]);
        m.push("BENCH_MODE_LITE".into());
        ev.set("missing", J::from(sorted_set(m)));
    }
    let unknown: Vec<String> = strs_of(&ev["missing"]).into_iter().filter(|c| MISSING_CODES().get(c).is_none()).collect();
    if !unknown.is_empty() {
        return core::fail(
            "HINT_MISSING_CODE_UNREGISTERED",
            format!("등재되지 않은 결손 코드: {}", J::from(unknown).repr()),
            Some("evidence.MISSING_CODES 에 뜻과 함께 등재한다(tripwire — 조용히 차단/통과로 떨어지지 않게)."),
        );
    }
    let mut seeds = if ev["lineage_seeds"].is_dict() { ev["lineage_seeds"].clone() } else { J::dict() };
    seeds.set("cell_key", cell.as_str());
    let identity = _measured_identity(repo, &ev)?.0;
    seeds.set("identity", identity.clone());
    let mut toks = vec![cell.clone()];
    let rel_of = crate::naming::release_of(&ev["build_identity"]["vllm_ref"])?;
    for v in [ev["build_identity"]["image_tag"].clone(), J::from(rel_of)] {
        if v.truthy() && !toks.contains(&v.display()) {
            toks.push(v.display());
        }
    }
    seeds.set("axis_tokens", J::from(toks));
    let mut cands: Vec<(String, String)> = Vec::new();
    let bench = format!("output/{topo}/benchlog");
    if ev["attestation"].truthy() {
        let p = ev["attestation"]["_path"].clone();
        cands.push((p.display(), "attestation".into()));
        seeds.set("attestation", p);
    }
    if let Some(s) = sp_rel.as_ref().filter(|s| !s.is_empty()) {
        cands.push((s.clone(), "serve_proof".into()));
    }
    if ev["cleanup_attestation_path"].truthy() {
        cands.push((ev["cleanup_attestation_path"].display(), "native_cleanup_attestation".into()));
    }
    let mut names = vec![cell.clone()];
    let cp = { let c = &ev["build_identity"]["compose_project"]; if c.truthy() { c.display() } else { String::new() } };
    if let Some(pm) = rx_compose_project().fullmatch(&cp) {
        if !names.iter().any(|n| n == &pm[1]) {
            names.push(pm[1].to_string());
        }
    }
    for n in &names {
        let d = repo.join(&bench).join(format!("serve_fail_{n}"));
        if d.is_dir() {
            cands.push((rel(repo, &d)?, "engine_failure_logs".into()));
        }
    }
    let mut ls = if ev["lineage_seeds"].is_dict() { ev["lineage_seeds"].clone() } else { J::dict() };
    ls.set("identity", identity);
    ev.set("lineage_seeds", ls);
    for (f, kind) in _timeline_parts(repo, &ev)?.1 {
        cands.push((f, kind));
    }
    if !ev["resolve"].is_null() {
        cands.push((format!("output/{topo}/resolved.json"), "resolve".into()));
    }
    for lp in measured_engine_logs_ex_t(repo, &ev)?.0 {
        cands.push((rel(repo, &lp)?, "engine_log".into()));
    }
    for (f, kind) in files_of(&engine_banner(repo, &ev)?["files"]) {
        cands.push((f, kind));
    }
    for o in _runtime_observations(repo, &ev)? {
        cands.extend(files_of(&o["files"]));
    }
    let mut seen: Vec<String> = Vec::new();
    let mut ec = Vec::new();
    for (p, k) in cands {
        if seen.contains(&p) {
            continue;
        }
        seen.push(p.clone());
        ec.push(jd! {"path" => p, "kind" => k});
    }
    seeds.set("evidence_candidates", J::List(ec));
    ev.set("lineage_seeds", seeds);
    let lsrc = if ev.get_str("mode") == Some("campaign") {
        "캠페인 journey·escalation·grounding(셀 필터) + 측정 identity + 증거 후보".to_string()
    } else {
        format!("발행 기록 토픽 + 측정 identity + 증거 후보({})", campaign_absence(&ev, Some(repo))?["phrase"].display())
    };
    sources_set(&mut ev, "lineage_seeds", lsrc);
    Ok(ev)
}

pub fn _release_of(ref_: &J) -> HR<Option<String>> {
    crate::naming::release_of(ref_)
}

/// 노드 축 자동 파생의 입력 — 이 셀의 측정 TP 와 출처.
pub fn _tp_signal(repo: &Path, topology: &str, cell: &str, cert: &J, measured_key: Option<&str>, report_rel: Option<&str>, simlog_dirs: &J, measurement: &J) -> HR<(Option<i64>, String)> {
    if let Some(tp) = as_int(&norm(&cert["tensor_parallel_size"])) {
        return Ok((Some(tp), "인증서 tensor_parallel_size".into()));
    }
    if measured_key.map(|m| !m.is_empty()).unwrap_or(false) {
        let m = or(measurement, &J::Null);
        let (sw, _m, _s) = _bind_sweep(repo, topology, cell, measured_key, simlog_dirs, &m["sweep_dir"], &m["sweep_config"], &m["sweep_generated_utc"])?;
        let sw = sw.unwrap_or(J::Null);
        if let Some(tp) = as_int(&norm(&sw["index"]["meta"]["tensor_parallel_size"])) {
            return Ok((Some(tp), format!("sweep_index.meta.tensor_parallel_size({} · generated_utc = 측정 키)", sw.get("dir").map(J::display).unwrap_or("None".into()))));
        }
    }
    if let Some(tp) = as_int(&norm(&_lite_snapshot(repo, report_rel)?["tensor_parallel_size"])) {
        return Ok((Some(tp), format!("bench_report({}) 측정 환경 표 tensor_parallel_size", pname(Path::new(report_rel.unwrap_or(""))))));
    }
    let yrel = format!("output/{topology}/configs/{cell}.yaml");
    if repo.join(&yrel).is_file() {
        let y = _owner_yaml(repo, &repo.join(&yrel), "서빙 yaml")?;
        if y.is_dict() {
            if let Some(tp) = as_int(&y["tensor-parallel-size"]) {
                return Ok((Some(tp), format!("서빙 yaml tensor-parallel-size({yrel} · 선언 — 측정 기록이 TP 를 들지 않는다)")));
            }
        }
    }
    Ok((None, "측정 TP 미관측(인증서·조인 스윕 meta·리포트 표·서빙 yaml 모두 없음)".into()))
}

/// `--node` 미지정일 때 **cluster** 축 자동 파생.
pub fn _auto_node(repo: &Path, topology: &str, cell: &str, cert: &J, measured_key: Option<&str>, report_rel: Option<&str>, simlog_dirs: &J, measurement: &J) -> HR<(Option<String>, String)> {
    if topology != "multi" {
        return Ok((None, format!("topology={topology} — cluster 자동 파생 대상 아님")));
    }
    let (tp, tp_src) = _tp_signal(repo, topology, cell, cert, measured_key, report_rel, simlog_dirs, measurement)?;
    let (man, man_src) = _manifest(repo, topology)?;
    let gpn = as_int(&man.unwrap_or(J::Null)["gpus_per_node"]);
    let gpn_s = gpn.map(|g| g.to_string()).unwrap_or("None".into());
    let (sp, sp_rel) = _serve_proof(repo, topology, cell)?;
    let sp = sp.unwrap_or(J::Null);
    let multi_proof = sp.is_dict()
        && matches!(sp.get_str("kind"), Some("multinode_serve_proof") | Some("native_multinode_serve_proof"))
        && sp.get("cell").or_else(|| sp.get("config")).map(|v| v == &J::from(cell)).unwrap_or(false);
    let att_rel = format!("output/{topology}/benchlog/attestation_{cell}.json");
    let att = repo.join(&att_rel).is_file();
    let mut multi: Vec<String> = Vec::new();
    if multi_proof {
        multi.push(format!("멀티 serve proof {}(kind={})", disp_opt(&sp_rel), sp["kind"].display()));
    }
    if att {
        multi.push(format!("노드 정합 attestation {att_rel}"));
    }
    if let (Some(tp), Some(g)) = (tp, gpn.filter(|g| *g != 0)) {
        if tp > g {
            return Ok((
                Some("cluster".into()),
                format!(
                    "자동 파생 — 측정 TP={tp}({tp_src}) > gpus_per_node={g}({man_src}) · 단일 노드 형상일 수 없다{}",
                    if multi.is_empty() { String::new() } else { format!(" · 일치 신호: {}", multi.join(" · ")) }
                ),
            ));
        }
        if !multi.is_empty() {
            return core::fail(
                "HINT_NODE_AXIS_UNDERIVABLE",
                format!("노드 축이 모호하다 — 측정 TP={tp}({tp_src}) ≤ gpus_per_node={g}(단일 노드 형상)인데 멀티 신호가 있다: {}.", J::from(multi).repr()),
                Some("측정한 노드 축을 `--node main|sub|cluster` 로 명시한다(한 태그 = 한 노드 형상의 실행 기록)."),
            );
        }
        return Ok((None, format!("측정 TP={tp} ≤ gpus_per_node={g} — 단일 노드 형상(자동 cluster ✗)")));
    }
    if !multi.is_empty() {
        return Ok((Some("cluster".into()), format!("자동 파생 — TP·gpus_per_node 대조 불가({tp_src} · gpus_per_node={gpn_s}) · 멀티 신호: {}", multi.join(" · "))));
    }
    Ok((None, format!("cluster 신호 없음({tp_src} · gpus_per_node={gpn_s} · 멀티 serve proof·attestation 부재)")))
}

fn resolve_node(ev_node: &J, measured: &J, topology: &str, assigned: Option<&str>, declared: &[String], cert_node_source: &str, auto: Option<&(Option<String>, String)>) -> HR<(String, String)> {
    let measured: Option<String> = if unobs(measured) { None } else { Some(measured.display().trim().to_string()) };
    let (node, src) = if ev_node.is_null() {
        if let Some(m) = measured.as_ref().filter(|m| NODE_AXIS.contains(&m.as_str())) {
            (m.clone(), format!("{cert_node_source} measured_node"))
        } else if let Some(a) = auto.filter(|a| a.0.as_deref() == Some("cluster")) {
            ("cluster".to_string(), a.1.clone())
        } else if let Some(a) = assigned.filter(|a| !a.is_empty()) {
            (a.to_string(), "캠페인 assignments(배정 SSOT)".to_string())
        } else {
            return core::fail(
                "HINT_NODE_AXIS_UNDERIVABLE",
                format!("발행 노드 축을 파생하지 못했다(measured_node·자동 cluster·배정 모두 없음{}).", auto.map(|a| format!(" · 자동: {}", a.1)).unwrap_or_default()),
                Some("`--node main|sub|cluster` 로 준다."),
            );
        }
    } else {
        (_check_node(ev_node)?, "발행 인자 --node".to_string())
    };
    if node == "cluster" && topology != "multi" {
        return core::fail("HINT_ARCH_NODE_AXIS_CERT_MISMATCH", format!("cluster 축은 multi 토폴로지에서만 성립한다(topology={topology})."), None);
    }
    if let Some(m) = &measured {
        if m != &node {
            return core::fail(
                "HINT_ARCH_NODE_AXIS_CERT_MISMATCH",
                format!("{cert_node_source} measured_node={} ≠ 발행 노드 축 {} — 한 태그 = 한 노드 형상의 실행 기록이다.", py_repr(m), py_repr(&node)),
                Some("측정한 그 노드 축으로 발행한다(측정 노드 어휘는 campaign_template_validator.resolve_measurement_node)."),
            );
        }
    }
    if (node == "main" || node == "sub") && assigned.map(|a| !a.is_empty() && a != node).unwrap_or(false) && declared.contains(&node) {
        return core::fail("HINT_NODE_ASSIGNMENT_MISMATCH", format!("셀은 {} 에 배정됐는데 발행 노드 축이 {} 다.", py_repr(assigned.unwrap()), py_repr(&node)), None);
    }
    Ok((node, src))
}

/// 발행 노드 축 (`auto` = Null 또는 [node|null, 사유]).
pub fn _resolve_node(ev_node: &J, measured: &J, topology: &str, assigned: Option<&str>, declared: &[String], cert_node_source: &str, auto: &J) -> HR<(String, String)> {
    let a = if auto.is_null() { None } else { Some((opt_str(auto.at(0)), auto.at(1).display())) };
    resolve_node(ev_node, measured, topology, assigned, declared, cert_node_source, a.as_ref())
}

/// 포인터가 발행 노드 축에 속하는가 (`val` = validator 모듈 자리 · Null = 모듈 없음 · 규칙은 OWNER-MIRROR).
pub fn _pointer_ok(p: &J, node: &str, declared: &[String], val: &J) -> HR<bool> {
    // `getattr(val, "DERIVED_MEASUREMENT_NODES", ())`: a null `val` (no validator module) has no derived axes.
    Ok(pointer_ok(p, node, declared, !val.is_null()))
}

fn pointer_ok(p: &J, node: &str, declared: &[String], with_validator: bool) -> bool {
    let nid = p.get("node_id").cloned().unwrap_or(J::Null);
    if nid.is_null() || nid == J::from("") {
        return declared.len() <= 1;
    }
    if nid == J::from(node) {
        return true;
    }
    if with_validator && node == "cluster" {
        let (cands, _k) = resolve_measurement_node(node, declared);
        return cands.iter().any(|c| nid == J::from(c.as_str()));
    }
    false
}

/// 셀 포인터의 인증서 (rel|None, cert|None, 파일 부재 포인터).
pub fn _certificate_from_pointers(repo: &Path, ptrs: &J) -> HR<(Option<String>, Option<J>, Vec<String>)> {
    let mut found: Vec<(Vec<String>, (String, J))> = Vec::new();
    let mut absent = Vec::new();
    let mut g = false;
    for p in ptrs.items() {
        let r = match p.get_str("path") {
            Some(r) if !r.is_empty() => r.to_string(),
            _ => continue,
        };
        if !repo.join(&r).is_file() {
            absent.push(r);
            continue;
        }
        let cert = parse_certificate(repo, &J::from(r.as_str()))?;
        if !g {
            let _ = cgate(repo)?;
            g = true;
        }
        let key = certificate_run_key(&cert).unwrap_or_else(|| vec!["\u{0}unidentifiable".into(), r.clone()]);
        if !found.iter().any(|(k, _)| *k == key) {
            found.push((key, (r, cert)));
        }
    }
    if found.len() > 1 {
        let mut rels: Vec<String> = found.iter().map(|(_, v)| v.0.clone()).collect();
        rels.sort();
        return core::fail(
            "HINT_CERTIFICATE_AMBIGUOUS",
            format!("이 셀에 서로 다른 측정의 인증서가 {}건 걸렸다: {}", found.len(), J::from(rels).repr()),
            Some("stale 포인터를 캠페인 writer 로 정리한다 — 발행기가 둘 중 하나를 고르지 않는다."),
        );
    }
    if let Some((_, (r, c))) = found.into_iter().next() {
        return Ok((Some(r), Some(c), absent));
    }
    Ok((None, None, absent))
}

/// 캠페인 셀 → CellEvidence. **명시 id 만 읽는다 — `campaigns/ACTIVE` 를 열지 않는다**. `docker` = 테스트 주입 훅(Null = 기본).
pub fn from_campaign(repo: &Path, campaign_id: &J, cell: &J, node: &J, _docker: &J /* callable */) -> HR<J> {
    from_campaign_r(repo, campaign_id, cell, node, None)
}

/// `from_campaign` with a typed docker runner hook (None = real `docker`).
pub fn from_campaign_r(repo: &Path, campaign_id: &J, cell: &J, node: &J, docker: Option<DockerRunner>) -> HR<J> {
    let repo = core::resolve(repo);
    let repo = repo.as_path();
    let cid = _check_campaign_id(campaign_id)?;
    let cell = _check_cell(cell)?;
    let base = repo.join("campaigns").join(&cid);
    if !base.is_dir() {
        return core::fail("HINT_CAMPAIGN_ABSENT", format!("캠페인 인스턴스 부재: campaigns/{cid}"), Some("purge 된 캠페인이면 `--publication <topic> --replay` 로 발행 기록에서 재생한다."));
    }
    let decl = _read_json_opt(&base.join("campaign.yaml"), "campaign.yaml")?;
    if !decl.is_dict() || decl.get_str("id") != Some(cid.as_str()) {
        return core::fail("HINT_CAMPAIGN_DECLARATION_INVALID", format!("campaigns/{cid}/campaign.yaml 의 id 가 {} 가 아니다.", py_repr(&cid)), None);
    }
    let _ = _validator(repo)?;
    let nodes: Vec<J> = decl["nodes"].items().iter().filter(|n| n.is_dict()).cloned().collect();
    let declared: Vec<String> = nodes.iter().filter(|n| n["node_id"].truthy()).map(|n| n["node_id"].display()).collect();
    let assigned = match declared.iter().find(|n| assigned_cells(&decl, n).contains(&cell)) {
        Some(a) => a.clone(),
        None => {
            return core::fail(
                "HINT_CELL_ASSIGNMENT_ABSENT",
                format!("campaigns/{cid} assignments 에 셀 {} 이 없다.", py_repr(&cell)),
                Some("배정 SSOT 는 assignments 다 — 셀 이름이 곧 증거 연결이다(2026-09-11 R10)."),
            )
        }
    };
    let node_decl = nodes.iter().find(|n| n["node_id"] == J::from(assigned.as_str())).cloned().unwrap_or_else(J::dict);
    let topology = or(&node_decl["topology"], &decl["control_variables"]["topology"]).clone();
    let topo = match topology.as_str() {
        Some(t) if TOPOLOGIES.contains(&t) => t.to_string(),
        _ => return core::fail("HINT_TOPOLOGY_UNDERIVABLE", format!("캠페인 선언에서 토폴로지를 해소하지 못했다: {}", topology.repr()), None),
    };
    let cdir = base.join("cells").join(&cell);
    let mut sources = jd! {"campaign_id" => "발행 인자 --campaign(명시 id · ACTIVE 미독)", "cell" => "발행 인자 --cell",
        "declaration" => format!("campaigns/{cid}/campaign.yaml(명시 id)"),
        "topology" => format!("campaigns/{cid}/campaign.yaml nodes[{assigned}].topology")};
    let mut missing: Vec<String> = Vec::new();
    let status = _read_json_opt(&cdir.join("cell.status.json"), "cell.status.json")?;
    let lock = _read_json_opt(&cdir.join("lockset.json"), "lockset.json")?;
    let config = if cdir.join("config.yaml").is_file() { _owner_yaml(repo, &cdir.join("config.yaml"), "셀 config")? } else { J::Null };
    for (code, obj, name, key) in [
        ("HINT_MISSING_CELL_STATUS", &status, "cell.status.json", "cell_status"),
        ("HINT_MISSING_LOCKSET", &lock, "lockset.json", "lockset"),
        ("HINT_MISSING_CELL_CONFIG", &config, "config.yaml", "cell_config"),
    ] {
        if !obj.is_dict() {
            missing.push(code.into());
        } else {
            sources.set(key, format!("campaigns/{cid}/cells/{cell}/{name}"));
        }
    }
    let ptr_doc = _read_json_opt(&base.join("evidence_pointers.json"), "evidence_pointers.json")?;
    let raw: Vec<J> = if ptr_doc["pointers"].is_list() { ptr_doc["pointers"].items().to_vec() } else { Vec::new() };
    let cell_ptrs: Vec<J> = raw.into_iter().filter(|p| p.is_dict() && p.get_str("cell_id") == Some(cell.as_str())).collect();
    let cert_ptrs = J::List(cell_ptrs.iter().filter(|p| p.get_str("kind") == Some("certificate")).cloned().collect());
    let (cert_rel, cert, absent) = _certificate_from_pointers(repo, &cert_ptrs)?;
    let certj = J::from(cert.clone());
    let mut auto: Option<(Option<String>, String)> = None;
    if node.is_null() && unobs(&certj["measured_node"]) {
        let pre = sorted_set(
            cell_ptrs.iter().filter(|p| p.get_str("kind") == Some("bench_report")).filter_map(|p| p.get_str("path")).filter(|p| repo.join(p).is_file()).map(str::to_string).collect(),
        );
        let pre_rel = if pre.len() == 1 { Some(pre[0].clone()) } else { _report_twin(cert_rel.as_deref())? };
        let mk = norm(&certj["measured_utc"]);
        let mk = if mk.truthy() { Some(mk.display()) } else { _report_born_utc(repo, pre_rel.as_deref())? };
        let measurement = if status.is_dict() { status["measurement"].clone() } else { J::Null };
        auto = Some(_auto_node(repo, &topo, &cell, &certj, mk.as_deref(), pre_rel.as_deref(), &J::list(), &measurement)?);
    }
    let (node, nsrc) = resolve_node(node, &certj["measured_node"], &topo, Some(&assigned), &declared, "인증서", auto.as_ref())?;
    sources.set("node", nsrc);
    let mut pointers: Vec<J> = Vec::new();
    for p in &cell_ptrs {
        if pointer_ok(p, &node, &declared, true) {
            pointers.push(p.clone());
        }
    }
    pointers.sort_by(|a, b| (a["kind"].display(), a["path"].display()).cmp(&(b["kind"].display(), b["path"].display())));
    if let Some(cr) = &cert_rel {
        if !pointers.iter().any(|p| p.get_str("path") == Some(cr.as_str())) {
            return core::fail("HINT_ARCH_NODE_AXIS_CERT_MISMATCH", format!("인증서 포인터 {cr} 의 node_id 가 발행 노드 축 {} 과 어긋난다.", py_repr(&node)), None);
        }
    }
    sources.set(
        "pointers",
        format!("campaigns/{cid}/evidence_pointers.json · cell_id={cell} · node∈{node}{}", if absent.is_empty() { String::new() } else { format!(" · 파일 부재 포인터 {}", J::from(absent.clone()).repr()) }),
    );
    let reports = sorted_set(pointers.iter().filter(|p| p.get_str("kind") == Some("bench_report")).filter_map(|p| p.get_str("path")).filter(|p| repo.join(p).is_file()).map(str::to_string).collect());
    let twin = _report_twin(cert_rel.as_deref())?;
    let report_rel: Option<String>;
    if let Some(t) = twin.as_ref().filter(|t| reports.contains(t)) {
        report_rel = Some(t.clone());
        sources.set("bench_report_path", "캠페인 포인터(인증서 이름 쌍)");
    } else if reports.len() == 1 {
        report_rel = Some(reports[0].clone());
        sources.set("bench_report_path", "캠페인 포인터 bench_report");
    } else if reports.len() > 1 {
        return core::fail(
            "HINT_BENCH_REPORT_AMBIGUOUS",
            format!("셀 {cell} 에 bench_report 포인터가 {}건이다: {}", reports.len(), J::from(reports.clone()).repr()),
            Some("측정 하나만 가리키도록 포인터를 정리한다 — 발행기가 고르지 않는다."),
        );
    } else if let Some(t) = twin.as_ref().filter(|t| !t.is_empty() && repo.join(t).is_file()) {
        report_rel = Some(t.clone());
        sources.set("bench_report_path", "인증서 이름 쌍(docs.md 명명 SSOT · 리포트 포인터 producer 부재 — G9)");
    } else {
        report_rel = None;
    }
    sources.set("certificate", match &cert_rel { Some(c) => format!("캠페인 포인터 certificate({c})"), None => "인증서 포인터 없음".into() });
    _require_report_joins_certificate(repo, report_rel.as_deref(), &certj)?;
    let mut phases = Map::new();
    let mut pcands = vec![assigned.clone()];
    if node == "cluster" {
        pcands.extend(declared.iter().filter(|d| **d != assigned).cloned());
    }
    for cand in &pcands {
        for f in glob_dir(&base.join("phases").join(cand), |n| n.ends_with(".status.json")) {
            let doc = _read_json_opt(&f, "phase status")?;
            if doc.is_dict() && doc.get_str("cell_id") == Some(cell.as_str()) && doc["phase"].truthy() && !phases.contains_key(&doc["phase"].display()) {
                let mut e = Map::new();
                for k in ["phase", "state", "node_id", "started_utc", "ended_utc", "first_started_utc", "proof"] {
                    e.insert(k, doc.get(k).cloned().unwrap_or(J::Null));
                }
                e.insert("_path", rel(repo, &f)?);
                phases.insert(doc["phase"].display(), J::Dict(e));
            }
        }
    }
    sources.set("phases", format!("campaigns/{cid}/phases/<node>/*.status.json 중 cell_id={cell}(노드별 덮어쓰기 — 마지막 셀만 남는다)"));
    let mut seeds = J::dict();
    for (fname, key) in [("journey.jsonl", "journey"), ("escalation_candidates.jsonl", "escalations")] {
        let p = base.join(fname);
        if p.is_file() {
            let text = core::read_text(&p).map_err(|e| HintError::py("OSError", e))?;
            let rows: Vec<J> = splitlines(&text).iter().map(|x| _json_line(x).unwrap_or(J::Null)).filter(|l| l.is_dict() && l.get_str("cell_id") == Some(cell.as_str())).collect();
            seeds.set(key, J::List(rows));
        }
    }
    let gdir = base.join("grounding");
    if gdir.is_dir() {
        let latest = glob_dir(&gdir, |n| n.ends_with(".json"));
        if let Some(last) = latest.last() {
            let g = _read_json_opt(last, "grounding")?;
            let refs = if g.is_dict() { or(&g["export"], &J::Null)["references"].clone() } else { J::Null };
            let gr: Vec<J> = refs.items().iter().filter(|r| r.is_dict()).map(|r| or(&r["path"], &r["ref_id"]).clone()).collect();
            seeds.set("grounding_refs", J::List(gr));
        }
    }
    if let Some(pr) = decl.get_str("plan_ref") {
        seeds.set("plan_ref", pr);
    }
    let ev = make_ev(
        "campaign",
        Some(&cid),
        &cell,
        &node,
        &topo,
        vec![
            ("declaration", decl.clone()),
            ("cell_status", if status.is_dict() { status.clone() } else { J::Null }),
            ("lockset", if lock.is_dict() { lock.clone() } else { J::Null }),
            ("cell_config", if config.is_dict() { config.clone() } else { J::Null }),
            ("certificate", certj.clone()),
            ("certificate_path", J::from(cert_rel.clone())),
            ("bench_report_path", J::from(report_rel.clone())),
            ("pointers", J::List(pointers)),
            ("phases", J::Dict(phases)),
            ("lineage_seeds", seeds),
            ("missing", J::from(missing)),
            ("sources", sources),
        ],
    );
    let key = if certj["measured_utc"].truthy() { certj["measured_utc"].clone() } else { J::from(_report_born_utc(repo, report_rel.as_deref())?) };
    let mk = norm(&key);
    let mk = if mk.is_null() { None } else { Some(mk.display()) };
    _assemble_r(repo, &ev, mk.as_deref(), &J::list(), docker)
}

pub fn _json_line(text: &str) -> HR<J> {
    Ok(crate::value::loads(text).unwrap_or(J::Null))
}

/// 발행 기록 재생(purge 된 캠페인의 유일한 통로). **읽기만 한다**. `docker` = 테스트 주입 훅(Null = 기본).
pub fn from_publication(repo: &Path, topic: &J, node: &J, _docker: &J /* callable */) -> HR<J> {
    from_publication_r(repo, topic, node, None)
}

/// `from_publication` with a typed docker runner hook (None = real `docker`).
pub fn from_publication_r(repo: &Path, topic: &J, node: &J, docker: Option<DockerRunner>) -> HR<J> {
    let repo = core::resolve(repo);
    let repo = repo.as_path();
    let topic = match topic.as_str() {
        Some(t) if !t.is_empty() && !t.contains('/') => t.to_string(),
        _ => return core::fail("HINT_PUBLICATION_TOPIC_INVALID", format!("발행 토픽이 올바르지 않다: {}", topic.repr()), None),
    };
    let rec_path = repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.json"));
    let record = _read_json_opt(&rec_path, "발행 기록")?;
    if !record.is_dict() {
        return core::fail("HINT_PUBLICATION_ABSENT", format!("발행 기록 부재: {}/{topic}.json", core::REL_EVIDENCE_DIR), None);
    }
    let pid = record.get("publication_id").cloned().unwrap_or(J::Null);
    if !(pid.is_null() || pid == J::from(topic.as_str())) {
        return core::fail("HINT_PUBLICATION_UNREADABLE", format!("발행 기록 publication_id={} ≠ {}", pid.repr(), py_repr(&topic)), None);
    }
    let man_path = repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.work-manifest.json"));
    let work_manifest = _read_json_opt(&man_path, "work-manifest")?;
    let ident = if record["identity"].is_dict() { record["identity"].clone() } else { J::dict() };
    let topology = ident.get("topology").cloned().unwrap_or(J::Null);
    let topo = match topology.as_str() {
        Some(t) if TOPOLOGIES.contains(&t) => t.to_string(),
        _ => return core::fail("HINT_TOPOLOGY_UNDERIVABLE", format!("발행 기록 identity.topology 가 single|multi 가 아니다: {}", topology.repr()), None),
    };
    let scaff = if record["scaffolded"].is_dict() { record["scaffolded"].clone() } else { J::dict() };
    let cert_rel = scaff.get_str("certificate").map(str::to_string);
    let cert = match &cert_rel {
        Some(c) if !c.is_empty() && repo.join(c).is_file() => Some(parse_certificate(repo, &J::from(c.as_str()))?),
        _ => None,
    };
    let certj = J::from(cert.clone());
    let mut report_rel = scaff.get_str("bench_report").map(str::to_string);
    if report_rel.as_ref().map(|r| !r.is_empty() && !repo.join(r).is_file()).unwrap_or(false) {
        report_rel = None;
    }
    _require_report_joins_certificate(repo, report_rel.as_deref(), &certj)?;
    let a = norm(&certj["measured_utc"]);
    let b = norm(&record["benchmark"]["measured_utc"]);
    let measured_key = if a.truthy() {
        Some(a.display())
    } else if b.truthy() {
        Some(b.display())
    } else {
        _report_born_utc(repo, report_rel.as_deref())?
    };
    let simlog = or(&record["raw_log_paths"]["simlog"], &scaff["simlog"]).clone();
    let simlog_dirs = match simlog.as_str() {
        Some(s) if !s.is_empty() => jl![s],
        _ => J::list(),
    };
    let (cell, cell_src) = _cell_of_publication(repo, &topo, measured_key.as_deref(), &simlog_dirs)?;
    let mut auto: Option<(Option<String>, String)> = None;
    if node.is_null() && unobs(&certj["measured_node"]) {
        auto = Some(_auto_node(repo, &topo, &cell, &certj, measured_key.as_deref(), report_rel.as_deref(), &simlog_dirs, &J::Null)?);
    }
    let (node, node_src) = resolve_node(node, &certj["measured_node"], &topo, None, &[], "인증서", auto.as_ref())?;
    let mut items: Vec<(String, J)> = scaff.as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()).unwrap_or_default();
    items.sort_by(|x, y| x.0.cmp(&y.0));
    let pointers: Vec<J> = items
        .into_iter()
        .filter(|(_, v)| v.as_str().map(|s| !s.is_empty()).unwrap_or(false))
        .map(|(k, v)| jd! {"kind" => k, "path" => v, "cell_id" => cell.as_str(), "node_id" => node.as_str()})
        .collect();
    let sources = jd! {"publication" => format!("{}/{topic}.json", core::REL_EVIDENCE_DIR), "topology" => "발행 기록 identity.topology",
        "cell" => cell_src, "node" => node_src, "pointers" => "발행 기록 scaffolded(캠페인 인스턴스 purge — 캠페인 포인터 없음)",
        "certificate" => if cert.is_some() { format!("발행 기록 scaffolded.certificate({})", disp_opt(&cert_rel)) } else { "발행 기록에 인증서 없음".into() },
        "bench_report_path" => if report_rel.as_ref().map(|r| !r.is_empty()).unwrap_or(false) { "발행 기록 scaffolded.bench_report" } else { "발행 기록에 리포트 없음" }};
    let publication = jd! {"topic" => topic.as_str(), "record" => record.clone(), "record_path" => rel(repo, &rec_path)?,
        "manifest_path" => if man_path.is_file() { J::from(rel(repo, &man_path)?) } else { J::Null },
        "manifest" => if work_manifest.is_dict() { work_manifest.clone() } else { J::Null },
        "task_class" => record.get("task_class").cloned().unwrap_or(J::Null)};
    let ev = make_ev(
        "publication-replay",
        None,
        &cell,
        &node,
        &topo,
        vec![
            ("certificate", certj.clone()),
            ("certificate_path", if cert.is_some() { J::from(cert_rel.clone()) } else { J::Null }),
            ("bench_report_path", J::from(report_rel.clone())),
            ("pointers", J::List(pointers)),
            ("publication", publication),
            ("lineage_seeds", jd! {"this_topic" => topic.as_str()}),
            ("missing", jl!["HINT_MISSING_CAMPAIGN_INSTANCE"]),
            ("sources", sources),
        ],
    );
    _assemble_r(repo, &ev, measured_key.as_deref(), &simlog_dirs, docker)
}

fn tuple_list_repr(v: &[(J, String)]) -> String {
    format!("[{}]", v.iter().map(|(a, b)| format!("({}, {})", a.repr(), py_repr(b))).collect::<Vec<_>>().join(", "))
}

/// 발행 기록에서 셀 이름 파생 — (셀, 출처).
pub fn _cell_of_publication(repo: &Path, topology: &str, measured_key: Option<&str>, simlog_dirs: &J) -> HR<(String, String)> {
    let mk = measured_key.filter(|m| !m.is_empty());
    for sd in simlog_dirs.items() {
        let sds = sd.display();
        let idx = _read_json_opt(&repo.join(&sds).join("sweep_index.json"), "simlog sweep_index.json")?;
        if idx.is_dict() {
            let name = or(&idx["meta"]["config_name"], &idx["config"]).clone();
            if let Some(m) = mk {
                if idx.get_str("generated_utc") != Some(m) {
                    continue;
                }
            }
            if let Some(n) = name.as_str() {
                if rx_cell().is_match(n) {
                    return Ok((n.to_string(), format!("{sds}/sweep_index.json meta.config_name(발행 기록의 simlog 사본)")));
                }
            }
        }
    }
    if let Some(m) = mk {
        let mut hits: Vec<(J, String)> = Vec::new();
        for s in _output_sweeps(repo, topology, None)? {
            let idx = &s["index"];
            if idx.get_str("generated_utc") == Some(m) {
                hits.push((or(&idx["meta"]["config_name"], &idx["config"]).clone(), s["dir"].display()));
            }
        }
        if hits.len() == 1 {
            return Ok((hits[0].0.display(), format!("{}(generated_utc = measured_utc {m})", hits[0].1)));
        }
        if hits.len() > 1 {
            return core::fail("HINT_CELL_AMBIGUOUS", format!("measured_utc={m} 인 스윕이 {}개다: {}", hits.len(), tuple_list_repr(&hits)), None);
        }
        let blr = repo.join("output").join(topology).join("benchlog");
        let lr = |n: &str| n.starts_with("lite_raw_") && n.ends_with(".json") && n.len() >= "lite_raw_.json".len();
        let mut paths = glob_dir(&blr, lr);
        let mut sub: Vec<PathBuf> = Vec::new();
        for d in glob_dir(&blr.join(LITE_PUBLISH_DIR), |_| true).into_iter().filter(|d| d.is_dir()) {
            sub.extend(glob_dir(&d, lr));
        }
        sub.sort();
        paths.extend(sub);
        let mut lite_hits: Vec<String> = Vec::new();
        for raw_path in paths {
            let raw = _read_json_opt(&raw_path, "lite_raw")?;
            let name = if raw.is_dict() { raw.get_str("config_name").map(str::to_string) } else { None };
            let name = match name {
                Some(n) => n,
                None => continue,
            };
            let parent = raw_path.parent().map(Path::to_path_buf).unwrap_or_default();
            if rx_cell().is_match(&name)
                && pname(&raw_path) == format!("lite_raw_{name}.json")
                && (parent == blr || lite_sub_match(&name, &pname(&parent)))
                && raw.get_str("measured_utc") == Some(m)
                && !lite_hits.contains(&name)
            {
                lite_hits.push(name);
            }
        }
        if lite_hits.len() == 1 {
            return Ok((
                lite_hits[0].clone(),
                format!("output/{topology}/benchlog/[{LITE_PUBLISH_DIR}/<셀>_<UTC>/]lite_raw_{}.json(measured_utc = 리포트 생성일 {m} · config_name = 파일 이름)", lite_hits[0]),
            ));
        }
        if lite_hits.len() > 1 {
            return core::fail("HINT_CELL_AMBIGUOUS", format!("measured_utc={m} 인 lite raw 가 {}개다: {}", lite_hits.len(), J::from(lite_hits).repr()), None);
        }
    }
    core::fail(
        "HINT_CELL_UNDERIVABLE",
        "발행 기록에서 셀 이름을 파생하지 못했다(simlog 사본·조인되는 스윕 모두 없음).",
        Some("셀 이름은 토픽 문자열에서 추측하지 않는다 — 발행 기록의 simlog 에 sweep_index.json 이 있어야 한다."),
    )
}
