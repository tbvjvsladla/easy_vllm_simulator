//! artifacts part 3 — port of `scripts/hintlib/artifacts.py` lines 2944–5361: sub recipe (multi), value-status
//! candidates, reproduce steps + observed durations, `collect`, file-level verification records, relevance marks,
//! native source image, `_EvOverlay`.
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::{self, HintError, HR};
use crate::pyre::{self, Re};
use crate::value::{Map, J};
use crate::{jd, jl};
use super::*;

mod util;
use util::*;

// ── private regex constants (the Python module-level patterns of this range) ──────────────────────
fn re_post_req() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r#""POST /v1/(?:chat/)?completions HTTP/[0-9.]+""#, 0)
}
fn re_exempt_literal() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"^(shell-default \S+?:\d+: [a-z0-9-]+):.*$", 0)
}
fn re_arch_class() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"\b([A-Z][A-Za-z0-9]*For[A-Z][A-Za-z0-9]*)\b", 0)
}
fn re_vllm_py_path() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"(?<![\w.-])(vllm/[A-Za-z0-9_./-]+\.py)\b", 0)
}
fn re_model_module() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"vllm/(?:model_executor/)?models/([a-z0-9_]+)(?=[/.])", 0)
}
const MODEL_MODULE_GENERIC: [&str; 9] =
    ["registry", "utils", "interfaces", "interfaces_base", "adapters", "module_mapping", "__init__", "config", "vision"];
fn re_log_call() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r#"\blogger\.(?:info|warning|error|debug|info_once|warning_once)\(\s*f?(['"])((?:(?!\1).)*)\1"#, pyre::S)
}
fn re_dockerfile_directive() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"#\s*(?:syntax|escape|check)\s*=[^\n]*\n", pyre::I)
}
fn re_local_date() -> &'static Re {
    // OWNER-MIRROR (same module, part a1): artifacts.py:139 _LOCAL_DATE
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$", 0)
}
fn re_engine_ts() -> &'static Re {
    // OWNER-MIRROR (hintlib/evidence.py::_ENGINE_TS)
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"\b(?:INFO|WARNING|ERROR|DEBUG|CRITICAL)\s+(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\b", 0)
}
fn re_ansi() -> &'static Re {
    // OWNER-MIRROR (hintlib/evidence.py::_ANSI)
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, "\x1b\\[[0-9;]*[A-Za-z]", 0)
}
fn re_rerender_by() -> &'static Re {
    // OWNER-MIRROR (same module, part a1): artifacts.py:1045 RERENDER_GENERATED_BY_RE
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"^renderer@[0-9a-f]{12}\Z", 0)
}

fn key_err(k: &str) -> HintError {
    HintError::py("KeyError", crate::value::py_repr(k))
}
/// `d[k]` (KeyError when absent).
fn idx(d: &J, k: &str) -> HR<J> {
    d.get(k).cloned().ok_or_else(|| key_err(k))
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// STUBS still pending (ctx-dependent) are below the pure section.
// ════════════════════════════════════════════════════════════════════════════════════════════════

// ── 값의 지위 후보 ──────────────────────────────────────────────────────────────────────────
/// `_strip_comment` — `값  # 주석` → (값, 주석).
pub fn _strip_comment(value: &str) -> HR<(String, Option<String>)> {
    let chars: Vec<char> = value.chars().collect();
    let mut quote: Option<char> = None;
    for (i, &ch) in chars.iter().enumerate() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch == '#' && (i == 0 || chars[i - 1] == ' ' || chars[i - 1] == '\t') {
            let head: String = chars[..i].iter().collect();
            let tail: String = chars[i + 1..].iter().collect();
            let t = py_strip(&tail).to_string();
            return Ok((py_strip(&head).to_string(), if t.is_empty() { None } else { Some(t) }));
        }
    }
    Ok((py_strip(value).to_string(), None))
}

fn re_knob() -> &'static Re {
    static C: OnceLock<Re> = OnceLock::new();
    re_cached(&C, r"^([A-Za-z][A-Za-z0-9_-]*)\s*:(.*)$", 0)
}

/// `_serving_yaml_knobs` — top-level knobs of the serving yaml.
pub fn _serving_yaml_knobs(path: &Path) -> HR<Vec<(String, String, Option<String>)>> {
    let mut out: Vec<(String, String, Option<String>)> = Vec::new();
    if !path.is_file() {
        return Ok(out);
    }
    let text = _read(path)?;
    let mut cur: Option<usize> = None;
    for raw in py_splitlines(&text) {
        if py_strip(raw).is_empty() || py_lstrip(raw).starts_with('#') {
            continue;
        }
        if raw.starts_with(' ') || raw.starts_with('\t') {
            if let Some(i) = cur {
                let joined = format!("{} {}", out[i].1, py_strip(raw));
                out[i].1 = py_strip(&joined).to_string();
            }
            continue;
        }
        let Some(m) = re_knob().match_at_start(raw) else { continue };
        let (val, comment) = _strip_comment(pyre::group(&m, 2).unwrap_or(""))?;
        let vc: Vec<char> = val.chars().collect();
        let v = if vc.len() >= 2 && vc[0] == vc[vc.len() - 1] && (vc[0] == '\'' || vc[0] == '"') {
            val.trim_matches(|c| c == '\'' || c == '"').to_string()
        } else {
            val
        };
        out.push((pyre::group(&m, 1).unwrap_or("").to_string(), v, comment));
        cur = Some(out.len() - 1);
    }
    Ok(out)
}

// ── 재현 절차 · 관측 소요 (pure parts) ─────────────────────────────────────────────────────
/// `_first_measure_utc` — earliest measurement JSON `date` of the sweep (or the lite_raw measured_utc).
pub fn _first_measure_utc(repo: &Path, ev: &J) -> HR<(Option<String>, Option<String>)> {
    let sweep = or_dict(g(ev, "sweep"));
    let sdir = if sweep.is_dict() { sweep.get_str("dir").map(str::to_string) } else { None };
    let Some(sdir) = sdir else { return Ok((None, None)) };
    let sd = repo.join(&sdir);
    if !sd.is_dir() {
        return Ok((None, None));
    }
    let mut best: Option<(String, String)> = None;
    let cell = g(ev, "cell");
    for f in rglob_suffix(&sd, ".json") {
        if let Some(cs) = cell.as_str() {
            if !core::level_raw_is_measured_tool(&f, cs) {
                continue;
            }
        }
        let doc = _json_file(&f)?;
        let date = doc.as_ref().map(|d| g(d, "date")).unwrap_or(J::Null);
        let ds = if date.truthy() { s(&date) } else { String::new() };
        if let Some(m) = re_local_date().match_at_start(&ds) {
            let gp = |i| pyre::group(&m, i).unwrap_or("").to_string();
            let utc = format!("{}-{}-{}T{}:{}:{}Z", gp(1), gp(2), gp(3), gp(4), gp(5), gp(6));
            if best.as_ref().map(|b| utc < b.0).unwrap_or(true) {
                best = Some((utc, rel_posix(repo, &f)?));
            }
        }
    }
    if let Some(cs) = cell.as_str() {
        let rp = sd.join(format!("lite_raw_{cs}.json"));
        let raw = if rp.is_file() { _json_file(&rp)? } else { None };
        let mu = match &raw {
            Some(r) if r.get("config_name").map(|x| x.as_str() == Some(cs)).unwrap_or(false) => {
                _utc_z(r.get("measured_utc").and_then(J::as_str))?
            }
            _ => None,
        };
        if let Some(mu) = mu {
            if best.as_ref().map(|b| mu <= b.0).unwrap_or(true) {
                return Ok((
                    Some(mu),
                    Some(format!(
                        "{} measured_utc({HOST_START_MARK} · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)",
                        rel_posix(repo, &rp)?
                    )),
                ));
            }
        }
    }
    Ok(match best {
        Some((a, b)) => (Some(a), Some(b)),
        None => (None, None),
    })
}

/// `_phase_span` — (start, end, source) of a campaign phase status.
pub fn _phase_span(repo: &Path, ev: &J, node: &str, phase: &str, cell: &str) -> HR<(Option<String>, Option<String>, Option<String>)> {
    let cid = g(ev, "campaign_id");
    if !cid.truthy() || node.is_empty() {
        return Ok((None, None, None));
    }
    let phases = g(ev, "phases");
    let own = if phases.is_dict() { or_dict(phases).get(phase).cloned().unwrap_or(J::Null) } else { J::Null };
    let own_span = |own: &J| -> Option<(Option<String>, Option<String>, Option<String>)> {
        if own.is_dict() && g(own, "started_utc").truthy() && g(own, "ended_utc").truthy() {
            let p = g(own, "_path");
            let src = if p.truthy() { s(&p) } else { format!("evidence.phases.{phase}") };
            Some((Some(s(&g(own, "started_utc"))), Some(s(&g(own, "ended_utc"))), Some(src)))
        } else {
            None
        }
    };
    if node != "cluster" {
        return Ok(own_span(&own).unwrap_or((None, None, None)));
    }
    let decl = or_dict(g(ev, "declaration"));
    let mut ids: Vec<String> = Vec::new();
    if decl.is_dict() {
        for n in or_list(g(&decl, "nodes")).items() {
            if n.is_dict() && g(n, "node_id").truthy() {
                let id = s(&g(n, "node_id"));
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids.sort();
    let base = repo.join("campaigns").join(s(&cid)).join("phases");
    let mut docs: Vec<J> = Vec::new();
    for nid in &ids {
        if nid.contains('/') || nid.starts_with('.') {
            continue;
        }
        if let Some(d) = _json_file(&base.join(nid).join(format!("{phase}.status.json")))? {
            if d.get("cell_id").and_then(J::as_str) == Some(cell) && g(&d, "started_utc").truthy() && g(&d, "ended_utc").truthy() {
                docs.push(d);
            }
        }
    }
    if docs.is_empty() {
        return Ok(own_span(&own).unwrap_or((None, None, None)));
    }
    let st = docs.iter().map(|d| s(&g(d, "started_utc"))).min().unwrap();
    let en = docs.iter().map(|d| s(&g(d, "ended_utc"))).max().unwrap();
    Ok((
        Some(st),
        Some(en),
        Some(format!("campaigns/{}/phases/<선언 노드 {}>/{phase}.status.json(같은 셀 · 가장 이른 시작 ~ 가장 늦은 끝)", s(&cid), docs.len())),
    ))
}

/// `current_serve_window` — the official API-server session containing this measurement.
pub fn current_serve_window(repo: &Path, ev: &J, measure_start: Option<&str>, measure_end: Option<&str>) -> HR<Option<J>> {
    let sweep = or_dict(g(ev, "sweep"));
    let ms_raw: Option<String> = match measure_start.filter(|x| !x.is_empty()) {
        Some(x) => Some(x.to_string()),
        None => match _first_measure_utc(repo, ev)?.0 {
            Some(x) if !x.is_empty() => Some(x),
            _ => _lite_measured_utc(repo, ev)?.0,
        },
    };
    let ms = _utc_z(ms_raw.as_deref())?;
    let me_src: J = match measure_end.filter(|x| !x.is_empty()) {
        Some(x) => J::str(x),
        None => {
            let gu = g(&sweep, "generated_utc");
            if gu.truthy() {
                gu
            } else {
                J::from(ms.clone())
            }
        }
    };
    let me = _utc_z(me_src.as_str())?;
    let (Some(ms), Some(me)) = (ms, me) else { return Ok(None) };
    if me < ms {
        return Ok(None);
    }
    let meta = or_dict(g(&or_dict(g(&sweep, "index")), "meta"));
    let cname = g(&meta, "config_name");
    let cell = if cname.truthy() {
        s(&cname)
    } else {
        let c = g(ev, "cell");
        if c.truthy() { s(&c) } else { String::new() }
    };
    let topo = g(ev, "topology");
    let axis = g(ev, "node");
    let role = if axis.as_str() == Some("cluster") { J::str("main") } else { axis };
    let decl = or_dict(g(ev, "declaration"));
    let mut nodes: Vec<String> = Vec::new();
    for n in decl.get("nodes").map(|x| x.items().to_vec()).unwrap_or_default() {
        if n.is_dict() && g(&n, "role") == role && g(&n, "node_id").truthy() {
            nodes.push(s(&g(&n, "node_id")));
        }
    }
    if nodes.is_empty() {
        nodes.push(s(&role));
    }
    let mut uniq: Vec<String> = Vec::new();
    for n in nodes {
        if !uniq.contains(&n) {
            uniq.push(n);
        }
    }
    let mut records: Vec<J> = Vec::new();
    for node in &uniq {
        let docs = crate::evidence::_node_session_docs(repo, node)?;
        let Some(m) = docs.as_dict() else { continue };
        for d in m.values() {
            if !d.truthy() || d.get("config_file").and_then(J::as_str) != Some(cell.as_str()) || g(d, "topology") != topo {
                continue;
            }
            records.push(jd! {
                "start_utc" => idx(d, "started_utc")?, "end_utc" => g(d, "stopped_utc"),
                "session_id" => idx(d, "session_id")?, "node_id" => node.clone(), "source" => idx(d, "_source")?,
            });
        }
    }
    let st = |r: &J| s(&g(r, "start_utc"));
    let earlier: Vec<&J> = records.iter().filter(|r| st(r) <= ms).collect();
    if earlier.is_empty() {
        return Ok(None);
    }
    let latest = earlier.iter().map(|r| st(r)).max().unwrap();
    let candidates: Vec<&&J> = earlier.iter().filter(|r| st(r) == latest).collect();
    if candidates.len() != 1 {
        return Ok(None);
    }
    let chosen = (*candidates[0]).clone();
    let end = g(&chosen, "end_utc");
    if !end.truthy() || s(&end) < me || records.iter().any(|r| ms < st(r) && st(r) <= me) {
        return Ok(None);
    }
    Ok(Some(chosen))
}

/// `pre_sweep_note` — the marker sentence of the bench · serve duration cells.
pub fn pre_sweep_note(pre: &J) -> HR<String> {
    let srcs: Vec<String> = pre["sources"].items().iter().map(s).collect();
    Ok(format!(
        "최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 {}건 관측(상한 시각 {}~{} · 스모크 최소 추론 {SMOKE_INFERENCE_REQUESTS}회 초과(요청 목적·앞선 벤치 여부는 미관측) · 세션 {} · 출처 {} 의 POST 접근 줄 × 전후 vLLM 시각 줄(컨테이너 시계 UTC 가정 · 구간을 확인한 요청만, 세션 전체 요청 수 아님))",
        s(&idx(pre, "count")?),
        s(&idx(pre, "first_ub")?),
        s(&idx(pre, "last_ub")?),
        s(&idx(pre, "session_source")?),
        srcs.join(" · ")
    ))
}

// ── 수집 (file writers) ─────────────────────────────────────────────────────────────────────
fn dest_of(payload: &Path, slot: &str, name: &str) -> PathBuf {
    payload.join("artifacts").join(slot).join(name)
}
fn mkparent(dest: &Path) -> HR<()> {
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| core::io_err(&e, p))?;
    }
    Ok(())
}
fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
fn chmod(p: &Path, mode: u32) -> HR<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode & 0o777)).map_err(|e| core::io_err(&e, p))
}
fn mode_of(p: &Path) -> HR<u32> {
    use std::os::unix::fs::PermissionsExt;
    Ok(std::fs::metadata(p).map_err(|e| core::io_err(&e, p))?.permissions().mode() & 0o777)
}

/// `_copy` — copy a file into `artifacts/<slot>/`.
pub fn _copy(payload: &Path, source: &Path, slot: &str, name: Option<&str>) -> HR<String> {
    if !source.is_file() {
        return core::fail("HINT_ARTIFACT_ABSENT", format!("복사할 아티팩트가 없다: {}", file_name(source)), None);
    }
    let nm = match name.filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => file_name(source),
    };
    let dest = dest_of(payload, slot, &nm);
    if dest.exists() {
        return core::fail("HINT_ARTIFACT_NAME_COLLISION", format!("슬롯 {slot} 에 같은 이름이 두 번: {}", file_name(&dest)), None);
    }
    mkparent(&dest)?;
    std::fs::copy(source, &dest).map_err(|e| core::io_err(&e, source))?;
    chmod(&dest, mode_of(source)?)?;
    rel_posix(payload, &dest)
}

/// `_write` — write a text file into `artifacts/<slot>/`.
pub fn _write(payload: &Path, slot: &str, name: &str, text: &str) -> HR<String> {
    let dest = dest_of(payload, slot, name);
    if dest.exists() {
        return core::fail("HINT_ARTIFACT_NAME_COLLISION", format!("슬롯 {slot} 에 같은 이름이 두 번: {name}"), None);
    }
    mkparent(&dest)?;
    std::fs::write(&dest, text).map_err(|e| core::io_err(&e, &dest))?;
    rel_posix(payload, &dest)
}

/// `_write_bytes` — ship observed bytes as they are (signature: `bytes` → `&[u8]`; no caller outside this part).
pub fn _write_bytes(payload: &Path, slot: &str, name: &str, data: &[u8], mode: Option<i64>) -> HR<String> {
    let dest = dest_of(payload, slot, name);
    if dest.exists() {
        return core::fail("HINT_ARTIFACT_NAME_COLLISION", format!("슬롯 {slot} 에 같은 이름이 두 번: {name}"), None);
    }
    mkparent(&dest)?;
    std::fs::write(&dest, data).map_err(|e| core::io_err(&e, &dest))?;
    if let Some(m) = mode {
        chmod(&dest, (m & 0o777) as u32)?;
    }
    rel_posix(payload, &dest)
}

/// `_require_registered` — missing codes only from `evidence.MISSING_CODES`.
pub fn _require_registered(repo: &Path, missing: &[String]) -> HR<()> {
    let _ = repo;
    let codes = crate::evidence::MISSING_CODES();
    let Some(cm) = codes.as_dict() else {
        return core::fail("HINT_OWNER_MODULE_MISSING", "hintlib.evidence.MISSING_CODES 가 dict 가 아니다(결손 코드 사전의 소유자).", None);
    };
    let mut unknown: Vec<String> = missing.iter().filter(|m| !cm.contains_key(m)).cloned().collect();
    unknown.sort();
    unknown.dedup();
    if !unknown.is_empty() {
        return core::fail(
            "HINT_MISSING_CODE_UNREGISTERED",
            format!("artifacts 가 등재되지 않은 결손 코드를 냈다: {}", J::from(unknown).repr()),
            Some("evidence.MISSING_CODES 에 뜻과 함께 등재한다(tripwire — 조용히 '미등록' 으로 배포되지 않게)."),
        );
    }
    Ok(())
}

/// `_exempt_record` — drop the matched original text from a PII exemption record.
pub fn _exempt_record(rec: &str) -> HR<String> {
    Ok(match re_exempt_literal().match_at_start(rec) {
        Some(m) => pyre::group(&m, 1).unwrap_or("").to_string(),
        None => rec.to_string(),
    })
}

/// `slot_confidence` — 3 signals = file × applied evidence × declaration.
pub fn slot_confidence(row: &J) -> HR<String> {
    let ev = or_dict(g(row, "evidence"));
    let kind = g(&ev, "kind");
    let applied = matches!(kind.as_str(), Some("build-ledger") | Some("reconstruction")) || g(&ev, "observed").truthy();
    let n = g(row, "files").truthy() as i64 + applied as i64 + g(row, "rationale").truthy() as i64;
    Ok(format!("{}-signal", n.max(1)))
}

/// `_slot`.
pub fn _slot(files: &[String], evidence: &J, excluded: &J) -> HR<J> {
    let mut f = files.to_vec();
    f.sort();
    let mut row = jd! {"files" => f.clone(), "applicable" => !f.is_empty(), "rationale" => J::Null, "evidence" => evidence.clone()};
    if excluded.truthy() {
        row.set("excluded", excluded.clone());
    }
    let conf = slot_confidence(&row)?;
    row.set("confidence", conf);
    Ok(row)
}

pub const VERIFICATION_VALUES: [&str; 2] = ["verified", "generated-unverified"];
pub const GENERATED_BY_VALUES: [&str; 3] = ["cell-run", "renderer", "agent"];
/// `generated_by_ok` — closed list + `renderer@<rev12>`.
pub fn generated_by_ok(value: &J) -> HR<bool> {
    Ok(match value.as_str() {
        Some(v) => GENERATED_BY_VALUES.contains(&v) || re_rerender_by().is_match(v),
        None => false,
    })
}

pub const RELEVANCE_VALUES: [&str; 3] = ["required", "inactive-inferred", "unknown"];
pub const UNVERIFIED_HEADER_KEY: &str = "_verification";
pub const NATIVE_UNVERIFIED_REASON: &str = "native 셀(Docker 미사용)이라 이 파일은 실행 검증되지 않았다";
pub const NATIVE_LAUNCH_NAME: &str = "native-launch.md";
pub const _SIG_MIN: i64 = 12;

/// `unverified_header` — the file's first-line warning (without the comment sign).
pub fn unverified_header(reason: &str, generated_by: &str) -> HR<String> {
    Ok(format!("⚠ generated-unverified — {reason} · 생성: {generated_by}"))
}

fn directive_index(lines: &[&str]) -> usize {
    lines.iter().position(|x| !re_dockerfile_directive().is_match(&format!("{x}\n"))).unwrap_or(lines.len())
}

/// `_insert_header` — put the warning into the shipped copy.
pub fn _insert_header(payload: &Path, rel: &str, reason: &str, generated_by: &str) -> HR<J> {
    let path = payload.join(rel);
    let mode = mode_of(&path)?;
    let text = read_strict(&path)?;
    let line = unverified_header(reason, generated_by)?;
    let out;
    if rel.ends_with(".json") {
        let mut doc = crate::value::loads(&text).map_err(|e| HintError::py("JSONDecodeError", e))?;
        if !doc.is_dict() {
            return core::fail("HINT_ARTIFACT_HEADER_UNSUPPORTED", format!("JSON 최상위가 사전이 아니라 경고 키를 넣을 수 없다: {rel}"), None);
        }
        doc.set(UNVERIFIED_HEADER_KEY, line.clone());
        std::fs::write(&path, core::dumps(&doc)).map_err(|e| core::io_err(&e, &path))?;
        out = jd! {"style" => "json-key", "key" => UNVERIFIED_HEADER_KEY, "text" => line};
    } else if text.starts_with("#!") {
        let (first, rest) = match text.split_once('\n') {
            Some((a, b)) => (a, b),
            None => (text.as_str(), ""),
        };
        std::fs::write(&path, format!("{first}\n# {line}\n{rest}")).map_err(|e| core::io_err(&e, &path))?;
        out = jd! {"style" => "comment", "line" => 2, "text" => format!("# {line}"), "why_line_2" => "shebang 보존"};
    } else if re_dockerfile_directive().is_match(&text) {
        let lines: Vec<&str> = text.split('\n').collect();
        let k = directive_index(&lines);
        let hl = format!("# {line}");
        let mut all: Vec<&str> = lines[..k].to_vec();
        all.push(&hl);
        all.extend_from_slice(&lines[k..]);
        std::fs::write(&path, all.join("\n")).map_err(|e| core::io_err(&e, &path))?;
        out = jd! {"style" => "comment", "line" => (k + 1) as i64, "text" => hl.clone(), "why_line_2" => "Dockerfile 파서 지시자 보존"};
    } else {
        std::fs::write(&path, format!("# {line}\n{text}")).map_err(|e| core::io_err(&e, &path))?;
        out = jd! {"style" => "comment", "line" => 1, "text" => format!("# {line}")};
    }
    chmod(&path, mode)?;
    Ok(out)
}

/// `_agent_request` — a place the renderer cannot fill (request, not generation).
pub fn _agent_request(slot: &str, name: &str, why: &str, inputs: &[String]) -> HR<J> {
    Ok(jd! {
        "slot" => slot, "name" => name, "path" => format!("artifacts/{slot}/{name}"), "why" => why,
        "inputs" => inputs.to_vec(), "verification" => "generated-unverified", "generated_by" => "agent",
        "header_required" => true, "header" => unverified_header("Agent 가 증거에서 저작했다(실행 검증 ✗)", "agent")?,
    })
}

/// `_EvOverlay` — a view that overrides some evidence fields (base unchanged). Evidence is a `J` dict here, so the
/// view is the merged dict (`dict(base, **over)`).
#[derive(Clone, Debug, Default)]
pub struct _EvOverlay {
    pub base: J,
    pub over: Map,
}
impl _EvOverlay {
    pub fn new(base: &J, over: &Map) -> _EvOverlay {
        _EvOverlay { base: base.clone(), over: over.clone() }
    }
    /// The merged evidence value.
    pub fn view(&self) -> J {
        let mut m = self.base.as_dict().cloned().unwrap_or_default();
        m.update(&self.over);
        J::Dict(m)
    }
}

/// `_log_signatures` — logger literals the patch plants (≥ `_SIG_MIN` code points).
pub fn _log_signatures(text: &str) -> HR<Vec<String>> {
    let mut sigs: Vec<String> = Vec::new();
    let splitter = {
        static C: OnceLock<Re> = OnceLock::new();
        re_cached(&C, r"[%{]", 0)
    };
    for m in re_log_call().finditer(text) {
        let lit0 = pyre::group(&m, 2).unwrap_or("");
        let first = splitter.split(lit0).into_iter().next().unwrap_or_default();
        let lit = py_strip(&first).to_string();
        if clen(&lit) as i64 >= _SIG_MIN && !sigs.contains(&lit) {
            sigs.push(lit);
        }
    }
    Ok(sigs)
}

/// `_patch_declared` — declared target archs / model modules / shared files.
pub fn _patch_declared(text: &str, trigger: Option<&str>) -> HR<J> {
    let trig = trigger.unwrap_or("");
    let mut mods: Vec<String> = re_model_module().findall(text).into_iter().filter(|m| !MODEL_MODULE_GENERIC.contains(&m.as_str())).collect();
    mods.sort();
    mods.dedup();
    let mut shared: Vec<String> = re_vllm_py_path()
        .findall(text)
        .into_iter()
        .filter(|p| {
            !re_model_module()
                .match_at_start(p)
                .map(|mm| !MODEL_MODULE_GENERIC.contains(&pyre::group(&mm, 1).unwrap_or("")))
                .unwrap_or(false)
        })
        .collect();
    shared.sort();
    shared.dedup();
    let mut archs = re_arch_class().findall(trig);
    archs.sort();
    archs.dedup();
    Ok(jd! {
        "archs" => archs, "modules" => mods, "shared_files" => shared, "general" => trig.contains("범용"),
        "trigger" => if trig.is_empty() { J::Null } else { J::str(trig) },
    })
}

/// `_fired_text` — matched signatures and counts.
pub fn _fired_text(sigs: &[String], counts: &J, fired: Option<bool>) -> HR<String> {
    let mut cnt: Vec<(String, J)> = Vec::new();
    if let Some(m) = counts.as_dict() {
        for (k, v) in m.iter() {
            if sigs.iter().any(|x| x == k) && v.truthy() {
                cnt.push((k.to_string(), v.clone()));
            }
        }
    }
    if fired == Some(true) && !cnt.is_empty() {
        let pos = |k: &str| sigs.iter().position(|x| x == k).unwrap_or(0);
        let mut top = cnt.clone();
        top.sort_by(|a, b| {
            let (va, vb) = (a.1.as_f64().unwrap_or(0.0), b.1.as_f64().unwrap_or(0.0));
            vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal).then(pos(&a.0).cmp(&pos(&b.0)))
        });
        let shown: Vec<String> =
            top.iter().take(3).map(|(k, v)| format!("{} {}회", crate::value::py_repr(&core::head_chars(k, 48)), s(v))).collect();
        let mut out = format!("발화({}", shown.join(" · "));
        if top.len() > 3 {
            out.push_str(&format!(" 외 {}개", top.len() - 3));
        }
        out.push_str(&format!(" · 서명 {}개 중 {}개 · 횟수 = 읽은 로그 전체 합)", sigs.len(), top.len()));
        return Ok(out);
    }
    Ok(if fired == Some(false) { format!("무발화(서명 {}개 전부 0회)", sigs.len()) } else { "발화 미판정".to_string() })
}

fn strs_of(j: &J) -> Vec<String> {
    j.items().iter().map(s).collect()
}

/// `_relevance` — (relevance, basis). `counts`: `J::Null` = None.
pub fn _relevance(declared: &J, model: &J, sigs: &[String], fired: Option<bool>, n_logs: i64, counts: &J) -> HR<(String, String)> {
    let darchs = strs_of(&declared["archs"]);
    let dmods = strs_of(&declared["modules"]);
    let mut tgt: Vec<String> = darchs.clone();
    tgt.extend(dmods.iter().map(|m| format!("models/{m}")));
    let tgt_txt = if tgt.is_empty() { "없음".to_string() } else { J::from(tgt.clone()).repr() };
    if !g(model, "observed").truthy() {
        return Ok(("unknown".into(), format!("① 모델 config 미관측({}) · 선언 {tgt_txt}", s(&g(model, "source")))));
    }
    if tgt.is_empty() {
        return Ok(("unknown".into(), "① 패치 헤더·본문에 대상 선언(model-trigger 아키텍처 · vllm/models/<model_type>) 없음".into()));
    }
    let mut archs: Vec<String> = strs_of(&or_list(g(model, "architectures")));
    archs.sort();
    archs.dedup();
    let mtype = g(model, "model_type");
    let general = g(declared, "general").truthy();
    let matched = darchs.iter().any(|a| archs.contains(a)) || (mtype.truthy() && dmods.iter().any(|m| J::str(m.clone()) == mtype));
    let d1 = format!(
        "① 선언 {} {} 셀 모델(architectures {} · model_type {}){}",
        J::from(tgt.clone()).repr(),
        if matched { "∋" } else { "∌" },
        J::from(archs.clone()).repr(),
        s(&mtype),
        if general { " · 범용 선언" } else { "" }
    );
    if sigs.is_empty() {
        return Ok(("unknown".into(), format!("{d1} · ② 패치가 심는 logger 발화 서명 없음(발화 미관측)")));
    }
    if n_logs == 0 {
        return Ok(("unknown".into(), format!("{d1} · ② 엔진 로그 미관측(서명 {}개를 대조할 로그 없음)", sigs.len())));
    }
    let fired_t = fired == Some(true);
    let d2 = if counts.is_null() {
        format!("② 엔진 로그 {n_logs}개에서 서명 {}(서명 {}개 · 매치 서명 미기록)", if fired_t { "발화" } else { "무발화" }, sigs.len())
    } else {
        format!("② 엔진 로그 {n_logs}개에서 서명 {}", _fired_text(sigs, counts, fired)?)
    };
    if matched && fired_t {
        return Ok(("required".into(), format!("{d1} · {d2}")));
    }
    if !matched && !fired_t && !general {
        return Ok(("inactive-inferred".into(), format!("{d1} · {d2} — 적용됐으나 이 모델에서 돌지 않은 것으로 **추론**(확정 ✗)")));
    }
    Ok(("unknown".into(), format!("{d1} · {d2} — 두 신호가 한 방향을 가리키지 않는다")))
}

/// `_default_basis`.
pub fn _default_basis(slot: &str, row: &J, name: &str) -> HR<String> {
    let _ = name;
    let ev = or_dict(g(row, "evidence"));
    let note = g(&ev, "note");
    Ok(match slot {
        "triplet" => format!("셀 트리플렛 — {}", if note.truthy() { s(&note) } else { "서빙 입력".into() }),
        "runtime_patch" => {
            let arming = g(&ev, "arming");
            let t = if arming.is_dict() { arm_log_text(&arming)? } else { "무장 관측 원천 없음 — 적용 관측 ✗".into() };
            format!("셀 configs 의 런타임 패치({t})")
        }
        "build_patch_pre" | "build_patch_post" => {
            let t = format!("적용 판정 {}({}) · {}", s(&g(&ev, "kind")), s(&g(&ev, "ref")), if note.truthy() { s(&note) } else { String::new() });
            t.trim_end_matches([' ', '·']).to_string()
        }
        "build_recipe" => format!("쓰인 레시피({}) — 개정 선택 slots.build_recipe.evidence.selected_revisions", s(&g(&ev, "ref"))),
        "compose" => "셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본)".into(),
        _ => format!("{slot} — {}", if note.truthy() { s(&note) } else { s(&g(&ev, "ref")) }),
    })
}

/// `_compose_volumes` — relative host dir → container path.
pub fn _compose_volumes(text: &str) -> HR<J> {
    static C: OnceLock<Re> = OnceLock::new();
    let re = re_cached(&C, r#"(?m)^\s*-\s*['"]?\./([A-Za-z0-9_.-]+)/?:(/[A-Za-z0-9_./-]+)"#, 0);
    let mut out = Map::new();
    for m in re.finditer(text) {
        let k = pyre::group(&m, 1).unwrap_or("");
        if !out.contains_key(k) {
            out.insert(k, pyre::group(&m, 2).unwrap_or("").trim_end_matches('/'));
        }
    }
    Ok(J::Dict(out))
}

pub const _PARENT_PREFIX: &str = "artifacts/build_recipe/parent-source-core/";

/// `_marked_leaf_matches_frozen` (signature: `bytes` → `&[u8]`; no caller outside this part).
pub fn _marked_leaf_matches_frozen(shipped: &[u8], frozen: &[u8], target: &str, mark: &J) -> HR<bool> {
    if shipped == frozen {
        return Ok(!g(mark, "header").truthy());
    }
    let header = or_dict(g(mark, "header"));
    if target != "Dockerfile"
        || g(mark, "verification").as_str() != Some("generated-unverified")
        || g(mark, "generated_by").as_str() != Some("renderer")
        || g(&header, "style").as_str() != Some("comment")
    {
        return Ok(false);
    }
    let Some(line) = header.get_str("text").map(str::to_string) else { return Ok(false) };
    if !line.starts_with("# ⚠ generated-unverified — ") || !line.ends_with(" · 생성: renderer") || line.contains('\n') || line.contains('\r') {
        return Ok(false);
    }
    let text = utf8_strict(frozen.to_vec())?;
    let lines: Vec<&str> = text.split('\n').collect();
    let index = if text.starts_with("#!") {
        1
    } else if re_dockerfile_directive().is_match(&text) {
        directive_index(&lines)
    } else {
        0
    };
    if !eq_int(&g(&header, "line"), index as i64 + 1) {
        return Ok(false);
    }
    let mut all: Vec<&str> = lines[..index.min(lines.len())].to_vec();
    all.push(&line);
    all.extend_from_slice(&lines[index.min(lines.len())..]);
    Ok(shipped == all.join("\n").as_bytes())
}

pub const _LITE_TOOL_LOAD_FLAGS: [&str; 8] = ["--dataset-name", "--random-input-len", "--random-output-len", "--random-range-ratio", "--ignore-eos", "--trust-remote-code", "--max-concurrency", "--request-rate"];
pub const SMOKE_INFERENCE_REQUESTS: i64 = 1;
pub const REL_NATIVE_SERVE: &str = ".claude/skills/upstream-version-watch/scripts/native_multinode_serve.py";

include!("a3/ctx.rs");
