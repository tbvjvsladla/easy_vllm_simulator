//! evidence part 4 — port of `scripts/hintlib/evidence.py` lines 5313–7133: publication qualification (X8), identity ·
//! topology label, checkpoint observation, naming facts, tail sources, campaign approval · publish phase, publisher driving,
//! and the self-test fixture builders.
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::pyre::Re;
use crate::value::{Map, J};
use crate::{jd, jl};
use super::*;

// ── private helpers (not in Python) ─────────────────────────────────────────────────────────────────────────
macro_rules! re_priv {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Re {
            static CELL: std::sync::OnceLock<Re> = std::sync::OnceLock::new();
            CELL.get_or_init(|| Re::new($pat, 0))
        }
    };
}
re_priv!(sha40_re, r"^[0-9a-f]{40}$");
re_priv!(hf_url_re, r"^(?:https?://)?(?:[^/@\s]+@)?(?:huggingface\.co|hf\.co)/([A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*?)(?:\.git)?/?$");
re_priv!(gitdir_re, r"^gitdir:\s*(.+)$");
re_priv!(ref_line_re, r"^ref:\s*(\S+)$");
re_priv!(base_model_key_re, r"^base_model\s*:");
re_priv!(list_item_re, r"^\s*-\s*(.+)$");
re_priv!(hf_id_re, r"[A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*");
re_priv!(compose_var_re, r"\$\{(\w+)(?::-[^}]*)?\}");
re_priv!(compose_var_default_re, r"\$\{(\w+)(?::-([^}]*))?\}");
re_priv!(spec_sh_re, r"--speculative[-_]config\b|--num[-_]speculative[-_]tokens\b");
re_priv!(eager_sh_re, r"--enforce[-_]eager\b");
re_priv!(anchor_re, r"[0-9a-f]{40}");
re_priv!(approval_utc_re, r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$");

/// Python `==` on JSON values: dicts compare order-insensitively, numbers by value (`1 == 1.0 == True`).
fn py_eq(a: &J, b: &J) -> bool {
    fn num(v: &J) -> Option<f64> {
        match v {
            J::Bool(x) => Some(if *x { 1.0 } else { 0.0 }),
            J::Int(i) => Some(*i as f64),
            J::Float(f) => Some(*f),
            _ => None,
        }
    }
    match (a, b) {
        (J::Int(x), J::Int(y)) => x == y,
        (J::Dict(x), J::Dict(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map(|w| py_eq(v, w)).unwrap_or(false)),
        (J::List(x), J::List(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|(v, w)| py_eq(v, w)),
        _ => match (num(a), num(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}
fn py_in(v: &J, items: &[J]) -> bool {
    items.iter().any(|x| py_eq(x, v))
}

fn g<'a>(v: &'a J, k: &str) -> &'a J {
    &v[k]
}
/// `x or {}` for a dict-typed field.
fn or_dict(v: &J) -> J {
    if v.truthy() { v.clone() } else { J::dict() }
}
/// Python `str(x)`.
fn ps(v: &J) -> String {
    v.display()
}
/// `str(x or "")`.
fn ps_or(v: &J) -> String {
    if v.truthy() { v.display() } else { String::new() }
}
fn opt_str(v: &J) -> Option<String> {
    v.as_str().map(str::to_string)
}
fn repr_opt(v: &Option<String>) -> String {
    match v {
        Some(s) => crate::value::py_repr(s),
        None => "None".to_string(),
    }
}
fn repr_strs(v: &[String]) -> String {
    J::from(v.to_vec()).repr()
}
fn s_slice_from(s: &str, n: usize) -> String {
    s.chars().skip(n).collect()
}
/// `Path(s).name` (pathlib: empty and `.` components dropped).
fn py_name(s: &str) -> String {
    s.split('/').filter(|p| !p.is_empty() && *p != ".").last().unwrap_or("").to_string()
}
fn fname(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
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
            '\n' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
fn os_err_name(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        std::io::ErrorKind::IsADirectory => "IsADirectoryError",
        std::io::ErrorKind::NotADirectory => "NotADirectoryError",
        _ => "OSError",
    }
}
/// `path.read_text(encoding="utf-8", errors="replace")` (OSError → Err).
fn read_lossy(p: &Path) -> Result<String, std::io::Error> {
    std::fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned())
}
/// Same, an OSError escaping as the built-in exception.
fn read_lossy_hr(p: &Path) -> HR<String> {
    read_lossy(p).map_err(|e| core::io_err(&e, p))
}
fn py_strip(s: &str) -> &str {
    s.trim()
}
fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}
fn py_lower(s: &str) -> String {
    s.to_lowercase()
}
/// Python `float(str)` (whitespace stripped; `_` between digits allowed).
fn py_float(s: &str) -> Option<f64> {
    let t = s.trim();
    let t2: String;
    let t = if t.contains('_') {
        let b = t.as_bytes();
        for (i, c) in b.iter().enumerate() {
            if *c == b'_' && (i == 0 || i + 1 >= b.len() || !b[i - 1].is_ascii_digit() || !b[i + 1].is_ascii_digit()) {
                return None;
            }
        }
        t2 = t.replace('_', "");
        t2.as_str()
    } else {
        t
    };
    let low = t.to_ascii_lowercase();
    let body = low.trim_start_matches(['+', '-']);
    if !(body == "inf" || body == "infinity" || body == "nan") && !t.chars().all(|c| c.is_ascii_digit() || "+-.eE".contains(c)) {
        return None;
    }
    t.parse::<f64>().ok()
}
/// Python `re.escape` for use inside a pattern (semantic equivalent for the Rust engine).
fn re_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            o.push('\\');
            o.push(c);
        } else if c.is_whitespace() {
            o.push_str(&format!("\\x{{{:x}}}", c as u32));
        } else {
            o.push(c);
        }
    }
    o
}
fn rel_s(repo: &Path, p: &Path) -> HR<String> {
    _rel(repo, p)
}
fn int_or0(v: &J) -> HR<i64> {
    Ok(_as_int(v)?.unwrap_or(0))
}
fn sweep_meta(ev: &J) -> J {
    or_dict(&or_dict(&or_dict(&ev["sweep"])["index"])["meta"])
}
fn in_strs(v: &J, list: &[&str]) -> bool {
    v.as_str().map(|s| list.contains(&s)).unwrap_or(false)
}
fn hint_err_is_hint(e: &HintError) -> bool {
    !e.is_py()
}

// ── OWNER-MIRROR: .claude/skills/terraforming_node/scripts/campaign_template_validator.py ───────────────────
const V_FILL: &str = "<<FILL>>";
const V_HINT_APPROVAL_FIELDS: [&str; 3] = ["approved_by", "approved_utc", "source"];
const V_HINT_APPROVAL_SOURCES: [&str; 2] = ["declaration-popup", "publish-popup"];
const V_SELF_PUBLISH_TOPOLOGIES: [&str; 1] = ["single"];
const V_DERIVED_MEASUREMENT_NODES: [&str; 1] = ["cluster"];

// OWNER-MIRROR: campaign_template_validator.py::assigned_cells
fn v_assigned_cells(doc: &J, node: Option<&J>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(a) = doc["assignments"].as_dict() {
        for (nid, items) in a.iter() {
            if let Some(n) = node {
                if !py_eq(&J::Str(nid.to_string()), n) {
                    continue;
                }
            }
            for item in items.as_list().map(|l| l.as_slice()).unwrap_or(&[]) {
                if let Some(cell) = item.as_dict().and_then(|_| item["cell"].as_str()) {
                    if !cell.is_empty() && !cell.contains(V_FILL) && !out.iter().any(|c| c == cell) {
                        out.push(cell.to_string());
                    }
                }
            }
        }
    }
    out
}

// OWNER-MIRROR: campaign_template_validator.py::hint_approval_reason
fn v_hint_approval_reason(approval: &J) -> Option<String> {
    if !approval.is_dict() {
        return Some("approval 이 없다 — 사람 승인의 전사가 없는 항목은 사전승인이 아니다".to_string());
    }
    for field in V_HINT_APPROVAL_FIELDS {
        let val = &approval[field];
        match val.as_str() {
            Some(s) if !s.trim().is_empty() => {
                if s.contains(V_FILL) {
                    return Some(format!("approval.{field} 에 {V_FILL} 이 남아 있다 — 모르는 값을 그럴듯하게 채우지 않는다"));
                }
            }
            _ => return Some(format!("approval.{field} 가 비었다 — 반쪽 승인은 승인이 아니다")),
        }
    }
    let utc = approval["approved_utc"].as_str().unwrap_or("");
    if !approval_utc_re().is_match(utc) {
        return Some(format!(
            "approval.approved_utc {} 가 주입 시각 모양(YYYY-MM-DDTHH:MM:SSZ)이 아니다 — 발행 페이로드는 이 모양만 옮긴다",
            crate::value::py_repr(utc)
        ));
    }
    let src = approval["source"].as_str().unwrap_or("");
    if !V_HINT_APPROVAL_SOURCES.contains(&src) {
        return Some(format!(
            "approval.source {} 가 목록 밖이다(허용 {})",
            crate::value::py_repr(src),
            J::from(V_HINT_APPROVAL_SOURCES.to_vec()).repr()
        ));
    }
    None
}

// OWNER-MIRROR: campaign_template_validator.py::hint_target_reasons (+ retired_hint_key_reasons)
fn v_hint_target_reasons(doc: &J) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    let targets = &doc["hint_targets"];
    if let Some(l) = targets.as_list() {
        for (i, t) in l.iter().enumerate() {
            if t.is_dict() && t.get("arch").is_some() {
                // plan_26111708 B L8: the remedy names the live `campaign_init.sh` launcher (same text as the owner's).
                problems.push(format!(
                    "hint_targets[{i}].arch 는 폐기된 키다 — 태그 이름은 발행 도구가 전량 파생한다(2026-09-21 D8) — 손으로 적은 arch 는 '파생 가능한데 손으로 적은 값'이다. 키를 지우고 승인은 `campaign_init.sh --hint-approve` 로 적는다"
                ));
            }
        }
    }
    if doc.get("hint_targets").is_none() || targets.is_null() {
        return problems;
    }
    let Some(tl) = targets.as_list() else {
        problems.push("hint_targets 가 목록이 아니다".to_string());
        return problems;
    };
    let nodes: Vec<J> = doc["nodes"].items().iter().filter(|n| n.is_dict()).cloned().collect();
    if !tl.is_empty() && doc["self_role"].as_str() == Some("sub") {
        let own_ids: Vec<J> = nodes.iter().map(|n| n["node_id"].clone()).collect();
        if nodes.len() != 1 || nodes[0]["role"].as_str() != Some("sub") {
            problems.push("self_role=sub 인스턴스에 hint_targets 가 있는데 nodes[] 가 자기(role sub) 노드 하나가 아니다 — 파생 선언(emit_slice)은 그 서브 노드 하나만 싣는다".to_string());
        } else if !in_strs(&nodes[0]["topology"], &V_SELF_PUBLISH_TOPOLOGIES) {
            problems.push(format!(
                "self_role=sub 인스턴스에 hint_targets 가 있다 — topology {} 의 서브는 자체 발행하지 않는다(자체 발행 = {} · 승인은 메인 선언에만)",
                nodes[0]["topology"].repr(),
                J::from(V_SELF_PUBLISH_TOPOLOGIES.to_vec()).repr()
            ));
        }
        for (i, t) in tl.iter().enumerate() {
            if t.is_dict() && !py_in(&t["node_id"], &own_ids) {
                problems.push(format!(
                    "self_role=sub 인스턴스의 hint_targets[{i}] 는 다른 노드({}) 몫이다 — 파생 선언은 그 노드 몫만 운반한다",
                    t["node_id"].repr()
                ));
            }
        }
    }
    let node_ids: Vec<J> = nodes.iter().map(|n| n["node_id"].clone()).collect();
    let mut approved_at: Vec<(String, usize)> = Vec::new();
    for (i, t) in tl.iter().enumerate() {
        if !t.is_dict() {
            problems.push(format!("hint_targets[{i}] 가 객체가 아니다"));
            continue;
        }
        let nid = &t["node_id"];
        if !py_in(nid, &node_ids) {
            problems.push(format!("hint_targets[{i}].node_id {} 가 nodes[] 에 없다", nid.repr()));
            continue;
        }
        let empty: Vec<J> = Vec::new();
        let cells: &[J] = match t["cells"].as_list() {
            Some(l) if !l.is_empty() => l.as_slice(),
            _ => {
                problems.push(format!(
                    "hint_targets[{i}].cells 가 비었다 — 승인할 셀을 명시 열거한다(빈 목록 = 전 셀 해석은 폐기 · 백지 승인 ✗)"
                ));
                &empty
            }
        };
        let assigned_here = v_assigned_cells(doc, Some(nid));
        for c in cells {
            let cs = match c.as_str() {
                Some(s) if !s.trim().is_empty() && !s.contains(V_FILL) => s,
                _ => {
                    problems.push(format!("hint_targets[{i}].cells 에 빈 셀 이름 {} 이 있다", c.repr()));
                    continue;
                }
            };
            if !assigned_here.iter().any(|a| a == cs) {
                problems.push(format!(
                    "hint_targets[{i}].cells 의 {} 이 assignments[{}] 에 없다 — 배정 SSOT 는 assignments 이고 hint_targets 는 파생이다",
                    c.repr(),
                    nid.repr()
                ));
            }
            if let Some((_, j)) = approved_at.iter().find(|(k, _)| k == cs) {
                problems.push(format!(
                    "hint_targets[{i}].cells 의 {} 은 이미 hint_targets[{j}] 가 승인했다 — 한 셀의 승인은 하나다(어느 전사가 그 셀의 승인인지 갈리지 않는다)",
                    c.repr()
                ));
            } else {
                approved_at.push((cs.to_string(), i));
            }
        }
        if let Some(why) = v_hint_approval_reason(&t["approval"]) {
            problems.push(format!("hint_targets[{i}] {why}"));
        }
    }
    problems
}

// OWNER-MIRROR: campaign_template_validator.py::hint_approval_for
fn v_hint_approval_for(doc: &J, cell: &J, node: Option<&J>) -> Option<J> {
    if !v_hint_target_reasons(doc).is_empty() {
        return None;
    }
    for (i, t) in doc["hint_targets"].items().iter().enumerate() {
        if let Some(n) = node {
            if !py_eq(&t["node_id"], n) {
                continue;
            }
        }
        if py_in(cell, t["cells"].items()) {
            let mut ap = Map::new();
            for k in V_HINT_APPROVAL_FIELDS {
                ap.insert(k, t["approval"][k].clone());
            }
            return Some(jd! {"index" => i, "node_id" => t["node_id"].clone(), "cells" => J::List(t["cells"].items().to_vec()),
                              "approval" => J::Dict(ap)});
        }
    }
    None
}

// OWNER-MIRROR: campaign_template_validator.py::resolve_measurement_node
fn v_resolve_measurement_node(tag: &str, declared: &[J]) -> (Vec<J>, &'static str) {
    let t = J::from(tag);
    if py_in(&t, declared) {
        return (vec![t], "declared");
    }
    if V_DERIVED_MEASUREMENT_NODES.contains(&tag) {
        return (declared.to_vec(), "derived");
    }
    (Vec::new(), "unknown")
}

fn declared_nodes(decl: &J) -> Vec<J> {
    decl["nodes"].items().iter().filter(|n| n.is_dict() && n["node_id"].truthy()).map(|n| n["node_id"].clone()).collect()
}

// ── OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/check_smoke_model.py ─────────────────────────
fn text_lines_keepends(text: &str) -> Vec<String> {
    let t = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in t.chars() {
        cur.push(c);
        if c == '\n' {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
// OWNER-MIRROR: check_smoke_model.py::read_app_models_host_token
fn sm_read_app_models_host_token(compose: &Path, container_root: &str) -> HR<Option<String>> {
    let text = core::read_text(compose).map_err(|e| HintError::py("OSError", e))?;
    let re = Re::new(&format!(r"-\s*(\S+):{}(?::[a-z]+)?\s*(?:#.*)?$", re_escape(container_root)), 0);
    for line in text_lines_keepends(&text) {
        if let Some(c) = re.search(&line) {
            return Ok(crate::pyre::group(&c, 1).map(str::to_string));
        }
    }
    Ok(None)
}
// OWNER-MIRROR: check_smoke_model.py::read_manifest_field
fn sm_read_manifest_field(base: &Path, field: &str) -> HR<Option<String>> {
    let p = base.join("manifest.yaml");
    if !p.is_file() {
        return Ok(None);
    }
    let text = core::read_text(&p).map_err(|e| HintError::py("OSError", e))?;
    let re = Re::new(&format!(r"\s*{}\s*:\s*(\S+)", re_escape(field)), 0);
    for line in text_lines_keepends(&text) {
        if let Some(c) = re.match_at_start(&line) {
            let v = crate::pyre::group(&c, 1).unwrap_or("");
            return Ok(Some(strip_chars(v.trim(), "'\"").to_string()));
        }
    }
    Ok(None)
}
// OWNER-MIRROR: check_smoke_model.py::resolve_app_models_host_root
fn sm_resolve_app_models_host_root(token: &str, base: &Path) -> HR<Option<String>> {
    let Some(c) = compose_var_default_re().fullmatch(token) else { return Ok(Some(token.to_string())) };
    let var = crate::pyre::group(&c, 1).unwrap_or("").to_string();
    let default = crate::pyre::group(&c, 2).map(str::to_string);
    if let Ok(v) = std::env::var(&var) {
        if !v.is_empty() {
            return Ok(Some(v));
        }
    }
    if let Some(m) = sm_read_manifest_field(base, &var.to_lowercase())? {
        if !m.is_empty() {
            return Ok(Some(m));
        }
    }
    Ok(default.filter(|d| !d.is_empty()))
}

// ── owner: adversarial-benchmark parse_guidellm (native launcher · evaluate-jsonl transport) ──────────────
const REL_PARSE_GUIDELLM_LAUNCHER: &str = ".claude/skills/adversarial-benchmark/scripts/parse_guidellm.sh";

/// `parse_guidellm_native.build(doc)` through the owner's native launcher. Errors keep the transport's exception types.
fn guidellm_build(repo: &Path, doc: &J) -> HR<J> {
    let enc = |v: &J| crate::value::dumps_default(v);
    let mut args = Map::new();
    args.insert("doc", enc(doc));
    args.insert("benchmark_index", enc(&J::Int(0)));
    args.insert("engine_max", enc(&J::Null));
    args.insert("spec", enc(&J::Null));
    args.insert("max_error_rate", enc(&J::Float(0.0)));
    args.insert("server_alive_at_bench_end", enc(&J::Null));
    let req = jd! {"id" => "python-consumer", "operation" => "build", "arguments_json" => J::Dict(args), "result_json" => true};
    let line = crate::value::dumps_default(&req) + "\n";
    let proc = core::run_owner(&core::resolve(repo), REL_PARSE_GUIDELLM_LAUNCHER, &["evaluate-jsonl".to_string()], Some(&line), &[], false)?;
    if proc.returncode != 0 {
        return Err(HintError::py(
            "RuntimeError",
            format!("GuideLLM native owner failed (exit {}): {}", proc.returncode, proc.stderr.trim_end()),
        ));
    }
    if !proc.stderr.is_empty() {
        return Err(HintError::py("RuntimeError", format!("GuideLLM native transport returned diagnostics: {}", proc.stderr.trim_end())));
    }
    let lines = py_splitlines(&proc.stdout);
    if lines.len() != 1 {
        return Err(HintError::py("RuntimeError", "GuideLLM native transport did not return one record"));
    }
    let rec = crate::value::loads(&lines[0]).map_err(|e| HintError::py("JSONDecodeError", e))?;
    if rec["id"].as_str() != Some("python-consumer") {
        return Err(HintError::py("RuntimeError", "GuideLLM native transport returned a different record identity"));
    }
    if rec["encoding"].as_str() != Some("json-values-v1") {
        return Err(HintError::py("RuntimeError", "GuideLLM native owner lacks the required JSON transport encoding; rebuild it"));
    }
    match rec["kind"].as_str() {
        Some("value") => crate::value::loads(rec["value_json"].as_str().unwrap_or("")).map_err(|e| HintError::py("JSONDecodeError", e)),
        Some("exception") => Err(HintError::py(rec["type"].as_str().unwrap_or("RuntimeError"), rec["message"].display())),
        _ => Err(HintError::py("RuntimeError", "GuideLLM native transport returned an unknown record kind")),
    }
}


// ── 발행 자격(관측 · X8) ─────────────────────────────────────────────────────────────────────────────────────
/// `_SERVE_PROOF_PASS_EVIDENCE` — serve_proof 통과 어휘(닫힌 양성 목록).
pub fn _SERVE_PROOF_PASS_EVIDENCE() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"chat.content\": \"content_len\", \"v1.completions\": \"completion_text_len\"}").expect("constant"))
}

/// serve_proof 한 건이 이 셀·이 측정의 통과 관측인가 → (ok, 이유).
pub fn _serve_proof_ok(sp: &J, cell: &str, image_tag: Option<&str>) -> HR<(bool, String)> {
    if sp["plane"].as_str() == Some("native") {
        let health = if sp["health"].is_dict() { sp["health"].clone() } else { J::dict() };
        let endpoints = if sp["endpoints"].is_dict() { sp["endpoints"].clone() } else { J::dict() };
        let h_ep = if endpoints["health"].is_dict() { endpoints["health"].clone() } else { J::dict() };
        let inference = if sp["inference"].is_dict() { sp["inference"].clone() } else { J::dict() };
        let i_ep = if endpoints["inference"].is_dict() { endpoints["inference"].clone() } else { J::dict() };
        let ok = sp["kind"].as_str() == Some("native_multinode_serve_proof")
            && sp["status"].as_str() == Some("PASS")
            && health["ok"] == J::Bool(true)
            && ps(&h_ep["http_status"]) == "200"
            && inference["ok"] == J::Bool(true)
            && ps(&i_ep["http_status"]) == "200"
            && int_or0(&inference["completion_text_len"])? >= 1;
        return Ok((
            ok,
            format!(
                "native kind={} status={} health={}/{} inference={}/{} completion_text_len={}",
                sp["kind"].repr(),
                sp["status"].repr(),
                health["ok"].repr(),
                h_ep["http_status"].repr(),
                inference["ok"].repr(),
                i_ep["http_status"].repr(),
                inference["completion_text_len"].repr()
            ),
        ));
    }
    let http = if !sp["health_http"].is_null() { sp["health_http"].clone() } else { sp["health_http_code"].clone() };
    let verdict = ps_or(&sp["inference_verdict"]).trim().to_string();
    let evidence_kind = ps_or(&sp["evidence"]).trim().to_string();
    let len_key = _SERVE_PROOF_PASS_EVIDENCE()[evidence_kind.as_str()].as_str().map(str::to_string);
    let n = match &len_key {
        Some(k) => _as_int(&sp[k.as_str()])?,
        None => None,
    };
    let n_repr = n.map(|x| x.to_string()).unwrap_or_else(|| "None".into());
    let why = format!(
        "config={} ready={} health={} verdict={} evidence={} {}={}",
        sp["config"].repr(),
        sp["ready"].repr(),
        http.repr(),
        crate::value::py_repr(&verdict),
        crate::value::py_repr(&evidence_kind),
        len_key.clone().unwrap_or_else(|| "len".into()),
        n_repr
    );
    if sp["config"].as_str() != Some(cell) || ps(&http) != "200" || sp["ready"] == J::Bool(false) {
        return Ok((false, why));
    }
    if verdict != "pass" || len_key.is_none() || n.unwrap_or(0) < 1 {
        return Ok((false, why));
    }
    if !ps_or(&sp["provenance"]).starts_with("measured") {
        return Ok((false, why + " provenance 비관측"));
    }
    if let Some(it) = image_tag.filter(|s| !s.is_empty()) {
        if sp["image_tag"].truthy() && !py_eq(&sp["image_tag"], &J::from(it)) {
            return Ok((false, format!("{why} image_tag {} ≠ 측정 {}", sp["image_tag"].repr(), crate::value::py_repr(it))));
        }
    }
    Ok((true, why))
}

/// 발행 자격 serve_proof 의 시점 범위 {written_utc, written_utc_source, timing, timing_note} | None.
pub fn serve_proof_scope(repo: &Path, ev: &J) -> HR<Option<J>> {
    let repo = _repo(repo)?;
    let rel = &ev["sources"]["serve_proof"];
    let Some(rel) = rel.as_str() else { return Ok(None) };
    if !ev["serve_proof"].is_dict() || !repo.join(rel).is_file() {
        return Ok(None);
    }
    let mut att = ev["serve_proof"].clone();
    att.set("_path", rel);
    let (written, wsrc) = _attestation_written(&repo, &att)?;
    let (timing, tnote, _win) = _attestation_timing(&repo, ev, written.as_deref())?;
    let Some(tnote) = tnote.filter(|t| !t.is_empty()) else { return Ok(None) };
    Ok(Some(jd! {"written_utc" => written, "written_utc_source" => wsrc, "timing" => timing,
                 "timing_note" => tnote.replace("측정 뒤 작성된 관측", "측정 뒤 작성된 스모크 산출물")}))
}

/// 발행 자격 = 관측(X8). 어느 관측도 없으면 HINT_QUALIFICATION_UNOBSERVED.
pub fn qualification(ev: &J) -> HR<J> {
    let repo = PathBuf::from(if ev["repo"].truthy() { ps(&ev["repo"]) } else { ".".to_string() });
    let cell = ps(&ev["cell"]);
    let topology = ps(&ev["topology"]);
    let mut tried: Vec<String> = Vec::new();
    let sp = &ev["serve_proof"];
    if sp.is_dict() {
        let bi = or_dict(&ev["build_identity"]);
        let (ok, why) = _serve_proof_ok(sp, &cell, bi["image_tag"].as_str())?;
        if ok {
            if sp["plane"].as_str() == Some("native") {
                _native_producer_evidence(&repo, &topology, &cell, sp, ev["sources"]["serve_proof"].as_str())?;
                return Ok(jd! {"health_200" => true, "inference_observed" => true,
                               "sources" => jl![ev["sources"]["serve_proof"].clone(), ev["sources"]["cleanup_attestation"].clone()],
                               "method" => "native_serve_proof+cleanup_attestation"});
            }
            let mut out = jd! {"health_200" => true, "inference_observed" => true,
                               "sources" => jl![ev["sources"]["serve_proof"].clone()], "method" => "serve_proof"};
            if let Some(sc) = serve_proof_scope(&repo, ev)? {
                if sc.truthy() {
                    out.set("scope", sc);
                }
            }
            return Ok(out);
        }
        tried.push(format!("serve_proof 불성립({why})"));
    }
    let sw = or_dict(&ev["sweep"]);
    if sw["binding"].as_str() == Some("output") {
        let base = repo.join(ps(&sw["dir"]));
        for ph in glob_post_health(&base, &format!("post_health_{cell}.json")) {
            let doc = _read_json_opt(&ph, "post_health")?;
            if !doc.is_dict() {
                continue;
            }
            if ps(&doc["health_http_code"]) != "200"
                || _boolish(&doc["container_running"])? != Some(true)
                || _boolish(&doc["container_oom_killed"])? == Some(true)
            {
                tried.push(format!("{} 불성립", rel_s(&repo, &ph)?));
                continue;
            }
            let parent = ph.parent().unwrap_or(Path::new("")).to_path_buf();
            let bench = parent.join(format!("bench_{cell}.json"));
            let bdoc = if core::level_raw_is_measured_tool(&bench, &cell) { _read_json_opt(&bench, "bench json")? } else { J::Null };
            if bdoc.is_dict() && int_or0(&bdoc["completed"])? >= 1 {
                return Ok(jd! {"health_200" => true, "inference_observed" => true,
                               "sources" => jl![rel_s(&repo, &ph)?, rel_s(&repo, &bench)?], "method" => "post_health+bench"});
            }
            tried.push(format!("{} completed<1 또는 부재", rel_s(&repo, &bench)?));
            let gl = parent.join(format!("guidellm_{cell}.json"));
            let gdoc = if gl.is_file() && core::level_raw_is_measured_tool(&gl, &cell) { _read_json_opt(&gl, "guidellm json")? } else { J::Null };
            if gdoc.is_dict() {
                let (done, why) = match guidellm_build(&repo, &gdoc) {
                    Ok(b) => {
                        let d = int_or0(&b["completed"])?;
                        (d, format!("successful={d}"))
                    }
                    Err(e) if e.code == "py:ValueError" => (0, e.message.clone()),
                    Err(e) => return Err(e),
                };
                if done >= 1 {
                    return Ok(jd! {"health_200" => true, "inference_observed" => true,
                                   "sources" => jl![rel_s(&repo, &ph)?, rel_s(&repo, &gl)?],
                                   "method" => "post_health+guidellm(parse_guidellm completed = request_totals.successful)"});
                }
                tried.push(format!("{} 성공 요청<1({why})", rel_s(&repo, &gl)?));
            }
        }
    } else if sw.truthy() {
        tried.push(format!("스윕이 {} 로만 묶였다 — post_health 원시가 이 측정의 것이 아니다", ps(&sw["binding"])));
    }
    if ev["report_kind"].as_str() == Some("lite") {
        let born = _report_born_utc(&repo, ev["bench_report_path"].as_str())?;
        let jdir = if born.is_some() { lite_join(&repo, ev)?.0 } else { None };
        let mut dirs = vec![jdir.unwrap_or_else(|| repo.join("output").join(&topology).join("benchlog"))];
        if sw["binding"].as_str() == Some("output") {
            dirs.push(repo.join(ps(&sw["dir"])));
        }
        let raw_name = format!("lite_raw_{cell}.json");
        for d in &dirs {
            let raw = _read_json_opt(&d.join(&raw_name), "lite_raw")?;
            if !raw.is_dict() {
                continue;
            }
            if born.is_none() || raw["config_name"].as_str() != Some(cell.as_str()) || !py_eq(&raw["measured_utc"], &J::from(born.clone())) {
                tried.push(format!(
                    "{} 조인 불성립(measured_utc={} ≠ 리포트 생성일 {} 또는 config_name={})",
                    rel_s(&repo, &d.join(&raw_name))?,
                    raw["measured_utc"].repr(),
                    repr_opt(&born),
                    raw["config_name"].repr()
                ));
                continue;
            }
            let warm = d.join(format!("lite_warm_{cell}.json"));
            let named = py_name(&ps_or(&raw["bench_warm_json"]));
            if !named.is_empty() && named != fname(&warm) {
                tried.push(format!("lite_raw 가 가리키는 warm 파일 {} ≠ {}", crate::value::py_repr(&named), fname(&warm)));
                continue;
            }
            let doc = _read_json_opt(&warm, "lite_warm")?;
            if doc.is_dict() && int_or0(&doc["completed"])? >= 1 {
                return Ok(jd! {"health_200" => true, "inference_observed" => true,
                               "sources" => jl![rel_s(&repo, &d.join(&raw_name))?, rel_s(&repo, &warm)?],
                               "method" => "lite_raw+lite_warm(measured_utc = 리포트 생성일 · lite_bench health 200 선검사 전제)"});
            }
            tried.push(format!("{} completed<1 또는 부재", rel_s(&repo, &warm)?));
        }
        if !tried.iter().any(|t| t.contains("lite_raw")) {
            tried.push(format!("lite_raw_{cell}.json 부재(조인 키 없음)"));
        }
    }
    let shown = if tried.is_empty() { vec!["관측 파일 0".to_string()] } else { tried };
    core::fail(
        "HINT_QUALIFICATION_UNOBSERVED",
        format!("셀 {} 에 health 200 + 추론 1회 **관측**이 없다: {}", ev["cell"].repr(), repr_strs(&shown)),
        Some(&format!(
            "스모크가 `output/{topology}/benchlog/serve_proof_{cell}.json`(pass · 생성 길이 ≥1)을 남기거나, 같은 measured_utc 의 스윕(post_health + bench completed≥1) 또는 lite 셀이면 리포트 생성일과 같은 measured_utc 의 `lite_raw_{cell}.json` + lite_warm(completed≥1)이 이 노드의 출력 평면에 있어야 한다. 선언(manifest.runtime)은 증거가 아니다. 서브에서 잰 셀은 메인이 그 원시를 볼 수 없다(문서기반 회수 only). 이번 해소의 측정 노드 = {} — 인증서가 없는 셀(REFUTE·lite)은 노드 축이 배정(main)으로 해소돼 cluster 로 잰 증거 포인터가 걸러질 수 있다: 실제 측정 노드가 다르면 `--node cluster|main|sub` 로 명시해 다시 실행한다(2026-09-22 통합 · runtime_selftest 재현).",
            ev["node"].repr()
        )),
    )
}

/// `base.glob("level_*/**/<name>")` sorted (pathlib component order).
fn glob_post_health(base: &Path, name: &str) -> Vec<PathBuf> {
    fn walk(d: &Path, name: &str, out: &mut std::collections::BTreeSet<PathBuf>) {
        let p = d.join(name);
        if p.exists() {
            out.insert(p);
        }
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, name, out);
                }
            }
        }
    }
    let mut out = std::collections::BTreeSet::new();
    if let Ok(rd) = std::fs::read_dir(base) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let p = e.path();
            if n.starts_with("level_") && p.is_dir() {
                walk(&p, name, &mut out);
            }
        }
    }
    out.into_iter().collect()
}

// ── identity · 토폴로지 라벨 ─────────────────────────────────────────────────────────────────────────────────
/// 강식별 6키의 측정 출처: 인증서 > 스윕 meta > 리포트 측정 환경 표 → (6키, 출처, 키별 출처 덮어쓰기).
pub fn _measured_identity(repo: &Path, ev: &J) -> HR<(J, String, J)> {
    if ev["certificate"].truthy() {
        return Ok((
            _identity_from_certificate(repo, &ev["certificate"])?,
            format!("인증서({})", py_name(&ps_or(&ev["certificate_path"]))),
            J::dict(),
        ));
    }
    let meta = sweep_meta(ev);
    if meta.truthy() {
        return Ok((_identity_from_meta(&meta)?, format!("sweep_index.meta({})", ps(&or_dict(&ev["sweep"])["dir"])), J::dict()));
    }
    let snap = _lite_snapshot(repo, ev["bench_report_path"].as_str())?;
    if snap.truthy() {
        let mut six = _identity_from_snapshot(repo, ev, &snap)?;
        let tp_src = six.as_dict_mut().and_then(|m| m.remove("_tp_source")).unwrap_or(J::Null);
        let per = if tp_src.truthy() { jd! {"tp" => tp_src} } else { J::dict() };
        return Ok((six, format!("bench_report 측정 환경 표({})", py_name(&ps(&ev["bench_report_path"]))), per));
    }
    let mut m = Map::new();
    for k in ["model", "gpu", "vllm", "quant", "topology", "tp"] {
        m.insert(k, J::Null);
    }
    Ok((J::Dict(m), "관측 없음".to_string(), J::dict()))
}

pub fn _identity_from_meta(meta: &J) -> HR<J> {
    Ok(jd! {"model" => _norm_value(&meta["model"])?, "gpu" => _norm_value(&meta["gpu_model"])?,
            "vllm" => _norm_value(&meta["vllm_version"])?, "quant" => _norm_value(&meta["quantization"])?,
            "topology" => _norm_value(&meta["topology"])?, "tp" => _as_int(&meta["tensor_parallel_size"])?})
}

/// 리포트 `## 측정 환경 스냅샷` → 6키(+ `_tp_source`).
pub fn _identity_from_snapshot(repo: &Path, ev: &J, snap: &J) -> HR<J> {
    let mut tp = _as_int(&_norm_value(&snap["tensor_parallel_size"])?)?;
    let mut tp_src: Option<String> = tp.map(|_| "bench_report 측정 환경 표 tensor_parallel_size".to_string());
    if tp.is_none() && ev["triplet"]["yaml"].truthy() {
        let yrel = ps(&ev["triplet"]["yaml"]);
        let y = _owner_yaml(repo, &repo.join(&yrel), "서빙 yaml")?;
        tp = if y.is_dict() { _as_int(&y["tensor-parallel-size"])? } else { None };
        tp_src = tp.map(|_| format!("서빙 yaml tensor-parallel-size({yrel} · 선언 — lite 표에 TP 칸이 없다)"));
    }
    Ok(jd! {"model" => _norm_value(&snap["model"])?, "gpu" => _norm_value(&snap["gpu_model"])?,
            "vllm" => _norm_value(&snap["vllm_version"])?, "quant" => _norm_value(&snap["quantization"])?,
            "topology" => _norm_value(&snap["topology"])?, "tp" => tp, "_tp_source" => tp_src})
}

/// PAYLOAD.identity — 강식별 6키(측정) + 체크포인트 관측 + source{필드: 출처}.
pub fn identity(ev: &J) -> HR<J> {
    let repo = _repo(Path::new(&if ev["repo"].truthy() { ps(&ev["repo"]) } else { ".".to_string() }))?;
    let (six, src6, per_key) = _measured_identity(&repo, ev)?;
    let need: Vec<String> = ["model", "gpu", "vllm", "topology", "tp"]
        .iter()
        .filter(|k| matches!(&six[**k], J::Null) || six[**k].as_str() == Some(""))
        .map(|k| k.to_string())
        .collect();
    if !need.is_empty() {
        return core::fail(
            "HINT_IDENTITY_UNDERIVABLE",
            format!("강식별 키 {} 를 관측에서 파생하지 못했다({src6}).", repr_strs(&need)),
            Some("인증서 또는 조인된 스윕 meta 가 있어야 한다 — 발행 기록의 손 identity 를 복사하지 않는다."),
        );
    }
    if !py_eq(&six["topology"], &ev["topology"]) {
        return core::fail(
            "HINT_IDENTITY_TOPOLOGY_MISMATCH",
            format!("측정 topology={} ≠ 셀 토폴로지 {}", six["topology"].repr(), ev["topology"].repr()),
            None,
        );
    }
    let ck = _checkpoint(&repo, ev)?;
    let vocab = crate::naming::load_vocab(&repo)?;
    let mut out = six.clone();
    out.set("hf_repo", ck["hf_repo"].clone());
    out.set("hf_revision", ck["hf_revision"].clone());
    out.set("base_model", ck["base_model"].clone());
    let model_s = six["model"].as_str().map(str::to_string).unwrap_or_else(|| ps(&six["model"]));
    out.set("base_slug", crate::naming::base_slug(&model_s, &vocab)?);
    let mut source = Map::new();
    if let Some(m) = six.as_dict() {
        for (k, _) in m.iter() {
            source.insert(k, per_key.get(k).cloned().unwrap_or_else(|| J::from(src6.clone())));
        }
    }
    source.insert("hf_repo", ck["hf_repo_source"].clone());
    let hrs = if ck["hf_revision_source"].truthy() {
        ck["hf_revision_source"].clone()
    } else if ck["reason"].truthy() {
        ck["reason"].clone()
    } else {
        J::from("체크포인트 미관측")
    };
    source.insert("hf_revision", hrs);
    source.insert("base_model", ck["base_model_source"].clone());
    source.insert("base_slug", format!("model({}) − vocab quant_suffixes(naming.base_slug)", ps(&six["model"])));
    out.set("source", J::Dict(source));
    let (qc, qcs) = _quant_composition(&ck)?;
    out.set("quant_composition", qc.map(J::List));
    if let Some(s) = out.get_mut("source") {
        s.set("quant_composition", qcs);
    }
    if out["quant"].is_null() || out["quant"].as_str() == Some("") {
        let q = _quant_raw(&ck)?;
        let mut tok: Option<String> = None;
        if let Some(raw) = q["raw"].as_str() {
            if !raw.trim().is_empty() {
                tok = match crate::naming::normalize("quant", &q["raw"], &vocab) {
                    Ok(t) => Some(t),
                    Err(e) if hint_err_is_hint(&e) => None,
                    Err(e) => return Err(e),
                };
            }
        }
        if let Some(tok) = tok.filter(|t| !t.is_empty()) {
            out.set("quant", tok.clone());
            let obs = _measured_quant_none(&repo, ev)?;
            let meas = match obs {
                Some(o) => format!("{QUANT_NONE_OBSERVED_MARK}({o}) — 비양자화 관측이라 이름 q 축 토큰으로 옮겼다"),
                None => format!("측정 identity quantization 미관측({src6})"),
            };
            let txt = format!("naming-axis(q) — {meas} · q 축 = {} · vocab quant[{tok}]←{}", ps(&q["source"]), q["raw"].repr());
            if let Some(s) = out.get_mut("source") {
                s.set("quant", txt);
            }
        }
    }
    Ok(out)
}

/// template._f_context 가 같은 문자열로 N/A 덧말을 건너뛴다.
pub const QUANT_NONE_OBSERVED_MARK: &str = "측정 identity quantization = none(관측)";

/// 측정 원천이 quantization = `none` 을 measured 출처와 함께 적었으면 그 출처 문자열.
pub fn _measured_quant_none(repo: &Path, ev: &J) -> HR<Option<String>> {
    fn hit(doc: &J, wh: &str) -> Option<String> {
        let v = &doc["quantization"];
        let vs = &doc["quantization_source"];
        if let (Some(v), Some(vs)) = (v.as_str(), vs.as_str()) {
            if v.trim().to_lowercase() == "none" && vs.trim().starts_with("measured") {
                return Some(format!("{wh}.quantization = none · quantization_source = {}", vs.trim()));
            }
        }
        None
    }
    if ev["certificate"].truthy() {
        return Ok(hit(&ev["certificate"], &format!("인증서({})", py_name(&ps_or(&ev["certificate_path"])))));
    }
    let meta = sweep_meta(ev);
    if meta.truthy() {
        return Ok(hit(&meta, &format!("sweep_index.meta({})", ps(&or_dict(&ev["sweep"])["dir"]))));
    }
    let snap = _lite_snapshot(repo, ev["bench_report_path"].as_str())?;
    Ok(if snap.truthy() { hit(&snap, &format!("bench_report 측정 환경 표({})", py_name(&ps_or(&ev["bench_report_path"])))) } else { None })
}

/// X7 — 파생 라벨 `"<topology> TP=<tp>"` + multi 면 `"(Ray)"`.
pub fn topology_label(identity: &J) -> HR<String> {
    let idn = or_dict(identity);
    let top = &idn["topology"];
    let tp = &idn["tp"];
    let ok_top = in_strs(top, &TOPOLOGIES);
    match (ok_top, tp) {
        (true, J::Int(n)) if *n >= 1 => {
            let t = top.as_str().unwrap_or("");
            Ok(format!("{t} TP={n}") + if t == "multi" { "(Ray)" } else { "" })
        }
        _ => core::fail("HINT_TOPOLOGY_UNDERIVABLE", format!("topology/tp 로 라벨을 만들 수 없다: {}/{}", top.repr(), tp.repr()), None),
    }
}

// ── 체크포인트 관측(양자화·PLE·HF repo·base_model) ─────────────────────────────────────────────────────────────
pub fn _serving_yaml(repo: &Path, ev: &J) -> HR<(Option<J>, String)> {
    let rel = &ev["triplet"]["yaml"];
    if !rel.truthy() {
        return Ok((None, format!("서빙 yaml 부재(output/{}/configs/{}.yaml)", ps(&ev["topology"]), ps(&ev["cell"]))));
    }
    let rel = ps(rel);
    let doc = _owner_yaml(repo, &repo.join(&rel), "서빙 yaml")?;
    if !doc.is_dict() {
        return core::fail("HINT_EVIDENCE_UNREADABLE", format!("서빙 yaml 최상위가 사전이 아니다: {rel}"), None);
    }
    Ok((Some(doc), rel))
}

fn ck_fail(mut out: Map, reason: String) -> J {
    out.insert("reason", reason.clone());
    out.insert("hf_repo_source", reason.clone());
    out.insert("base_model_source", reason);
    J::Dict(out)
}

fn ck_read(repo: &Path, ev: &J, out: &mut Map, host: &Path) -> HR<()> {
    out.insert("observed", true);
    let cfg = _read_json_opt(&host.join("config.json"), "체크포인트 config.json")?;
    out.insert("config", if cfg.is_dict() { cfg } else { J::Null });
    let hq = _read_json_opt(&host.join("hf_quant_config.json"), "hf_quant_config.json")?;
    out.insert("hf_quant", if hq.is_dict() { hq } else { J::Null });
    let label = ps(out.get("host_label").unwrap_or(&J::Null));
    let (a, b) = _hf_repo(host, &label)?;
    out.insert("hf_repo", a);
    out.insert("hf_repo_source", b);
    let mk = _measured_key(repo, ev)?;
    let (a, b) = _hf_revision(host, &label, mk.as_deref())?;
    out.insert("hf_revision", a);
    out.insert("hf_revision_source", b);
    let (a, b) = _base_model(host, &label)?;
    out.insert("base_model", a);
    out.insert("base_model_source", b);
    Ok(())
}

/// 서빙 yaml `model:` → 호스트 체크포인트(check_smoke_model 규칙). 출처에 호스트 경로를 적지 않는다.
pub fn _checkpoint(repo: &Path, ev: &J) -> HR<J> {
    let mut out = Map::new();
    out.insert("observed", false);
    let (y, yrel) = _serving_yaml(repo, ev)?;
    let model = y.as_ref().map(|y| y["model"].clone()).unwrap_or(J::Null);
    let model = match model.as_str() {
        Some(m) if m.starts_with('/') => m.to_string(),
        _ => return Ok(ck_fail(out, format!("{yrel} 에 컨테이너 절대경로 model: 이 없다"))),
    };
    out.insert("model_path", model.clone());
    let base = repo.join("output").join(ps(&ev["topology"]));
    if ev["plane"].as_str() == Some("native") {
        let mp = base.join("manifest.yaml");
        let mf = if mp.is_file() { _owner_yaml(repo, &mp, "manifest")? } else { J::Null };
        let mut hits: Vec<(String, String)> = Vec::new();
        if let Some(m) = mf.as_dict() {
            for (k, v) in m.iter() {
                if let Some(v) = v.as_str() {
                    let r = v.trim_end_matches('/');
                    if k.ends_with("_path") && v.starts_with('/') && (model == r || model.starts_with(&format!("{r}/"))) {
                        hits.push((k.to_string(), r.to_string()));
                    }
                }
            }
        }
        hits.sort_by_key(|kv| std::cmp::Reverse(kv.1.chars().count()));
        let Some((field, root)) = hits.into_iter().next() else {
            return Ok(ck_fail(out, format!("native model: 이 manifest 경로 필드의 하위가 아니다({yrel})")));
        };
        let tail = s_slice_from(&model, root.chars().count());
        let label = format!("native model: → <manifest.{field}>{tail}");
        out.insert("host_label", label.clone());
        let host = PathBuf::from(&model);
        if !host.is_dir() {
            return Ok(ck_fail(out, format!("체크포인트 호스트 경로 부재({label})")));
        }
        out.insert("model_path", format!("<manifest.{field}>{tail}"));
        ck_read(repo, ev, &mut out, &host)?;
        return Ok(J::Dict(out));
    }
    _smoke_model(repo)?;
    let root: String = model.split('/').take(3).collect::<Vec<_>>().join("/");
    let compose = base.join("docker-compose.yaml");
    let token = if compose.is_file() { sm_read_app_models_host_token(&compose, &root)? } else { None };
    let host_root = match token.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => sm_resolve_app_models_host_root(t, &base)?,
        None => None,
    };
    let field = compose_var_re().fullmatch(token.as_deref().unwrap_or("")).and_then(|c| crate::pyre::group(&c, 1).map(|s| s.to_lowercase()));
    let label = match &field {
        Some(f) if !f.is_empty() => format!("<manifest.{f}>"),
        _ => format!("<compose:{root} 마운트>"),
    };
    let tail = s_slice_from(&model, root.chars().count());
    let hl = format!("{model} → {label}{tail}");
    out.insert("host_label", hl.clone());
    let host = host_root.filter(|h| !h.is_empty()).map(|h| PathBuf::from(model.replacen(&root, &h, 1)));
    match host {
        Some(h) if h.is_dir() => {
            ck_read(repo, ev, &mut out, &h)?;
            Ok(J::Dict(out))
        }
        _ => Ok(ck_fail(out, format!("체크포인트 호스트 경로 미해소 또는 부재({hl})"))),
    }
}

crate::re_static!(_GIT_REF_RE, r"^refs/[A-Za-z0-9._/-]+$");

/// 체크포인트를 받아 온 커밋 — 체크포인트 `.git` 의 HEAD(→ ref → loose ref 또는 packed-refs). git 은 부르지 않는다.
pub fn _hf_revision(host: &Path, label: &str, measured: Option<&str>) -> HR<(Option<String>, String)> {
    let mut gd = host.join(".git");
    let head_res: Result<Option<String>, std::io::Error> = (|| {
        if gd.is_file() {
            let t = read_lossy(&gd)?;
            if let Some(c) = gitdir_re().match_at_start(t.trim()) {
                gd = core::resolve(&host.join(crate::pyre::group(&c, 1).unwrap_or("").trim()));
            }
        }
        if !gd.join("HEAD").is_file() {
            return Ok(None);
        }
        Ok(Some(read_lossy(&gd.join("HEAD"))?.trim().to_string()))
    })();
    let head = match head_res {
        Ok(Some(h)) => h,
        Ok(None) => return Ok((None, format!("체크포인트 .git/HEAD 부재({label}) — revision 미관측"))),
        Err(e) => return Ok((None, format!("체크포인트 .git/HEAD 를 읽지 못했다({label} · {}) — revision 미관측", os_err_name(&e)))),
    };
    let g = gd;
    if sha40_re().is_match(&head) {
        return _revision_at(&g, &head, &format!("체크포인트 .git/HEAD(detached · {label}) — checkout 커밋(작업트리 일치는 미검증)"), measured);
    }
    let rf = ref_line_re().match_at_start(&head).and_then(|c| crate::pyre::group(&c, 1).map(str::to_string)).unwrap_or_default();
    if !_GIT_REF_RE().is_match(&rf) || rf.split('/').any(|p| p == "..") {
        return Ok((None, format!("체크포인트 .git/HEAD 가 SHA·ref 모양이 아니다({label}) — revision 미관측")));
    }
    let mut sha: Option<String> = None;
    let mut how = String::new();
    let mut packed = Map::new();
    let r: Result<(), std::io::Error> = (|| {
        let loose = g.join(&rf);
        if loose.is_file() {
            let v = read_lossy(&loose)?.trim().to_string();
            if sha40_re().is_match(&v) {
                sha = Some(v);
                how = "loose ref".into();
            }
        }
        if g.join("packed-refs").is_file() {
            for ln in py_splitlines(&read_lossy(&g.join("packed-refs"))?) {
                let st = ln.trim();
                let parts: Vec<&str> = st.splitn(2, ' ').collect();
                if parts.len() == 2 && sha40_re().is_match(parts[0]) && !ln.starts_with('#') && !ln.starts_with('^') {
                    packed.insert(parts[1].trim(), parts[0]);
                }
            }
        }
        Ok(())
    })();
    if let Err(e) = r {
        return Ok((None, format!("체크포인트 {rf} 를 읽지 못했다({label} · {}) — revision 미관측", os_err_name(&e))));
    }
    if sha.is_none() {
        if let Some(v) = packed.get(&rf) {
            sha = v.as_str().map(str::to_string);
            how = "packed-refs".into();
        }
    }
    let Some(sha) = sha else {
        return Ok((None, format!("체크포인트 HEAD → {rf} 를 loose ref·packed-refs 어디서도 해소하지 못했다({label}) — revision 미관측")));
    };
    let branch = rf.rsplit('/').next().unwrap_or("").to_string();
    let remote = format!("refs/remotes/origin/{branch}");
    let rp = g.join(&remote);
    let rv: Option<String> = if rp.is_file() {
        read_lossy(&rp).ok().map(|t| t.trim().to_string())
    } else {
        packed.get(&remote).and_then(|v| v.as_str().map(str::to_string))
    };
    let tail = if rv.as_deref() == Some(sha.as_str()) {
        format!(" · {remote} 와 일치(받아 온 원격 커밋)")
    } else if let Some(rv) = rv.as_deref().filter(|s| !s.is_empty()) {
        format!(" · {remote} 는 다른 커밋({}) — 로컬 커밋일 수 있다", core::head_chars(rv, 12))
    } else {
        format!(" · {remote} 미관측")
    };
    _revision_at(&g, &sha, &format!("체크포인트 .git HEAD → {rf} → {how}({label}){tail} — checkout 커밋(작업트리 일치는 미검증)"), measured)
}

crate::re_static!(_REFLOG_RE, r"^([0-9a-f]{40}) ([0-9a-f]{40}) .*? (\d{9,11}) [+-]\d{4}(?:\t|$)");

/// 지금의 HEAD 가 측정 시점의 checkout 과 같은가 — 체크포인트 reflog(`logs/HEAD`) 대조.
pub fn _revision_at(g: &Path, sha: &str, desc: &str, measured: Option<&str>) -> HR<(Option<String>, String)> {
    let Some(measured) = measured.filter(|m| !m.is_empty()) else {
        return Ok((Some(sha.to_string()), format!("{desc} · 측정 시각 미관측 — 측정 시점 checkout 일치 미검증")));
    };
    let p = g.join("logs").join("HEAD");
    let text = if p.is_file() { read_lossy(&p).unwrap_or_default() } else { String::new() };
    let mut moves: Vec<(String, String)> = Vec::new();
    for ln in text.split('\n') {
        if let Some(c) = _REFLOG_RE().match_at_start(ln) {
            let ep: i64 = crate::pyre::group(&c, 3).unwrap_or("0").parse().unwrap_or(0);
            moves.push((crate::pyre::group(&c, 2).unwrap_or("").to_string(), core::Civil::from_epoch(ep).iso() + "Z"));
        }
    }
    if moves.is_empty() {
        return Ok((Some(sha.to_string()), format!("{desc} · reflog(logs/HEAD) 부재·판독 불가 — 측정 시점 checkout 일치 미검증")));
    }
    let before: Vec<&(String, String)> = moves.iter().filter(|(_, u)| u.as_str() <= measured).collect();
    let Some(last) = before.last() else {
        return Ok((None, format!(
            "{desc} · 그러나 reflog 첫 항목 {} 이 measured_utc {measured} 뒤다(측정 뒤 받아 온 checkout) — 측정한 revision 미관측(None · 지금 HEAD 를 고르지 않는다)",
            moves[0].1
        )));
    };
    if last.0 != sha {
        return Ok((None, format!(
            "{desc} · 그러나 측정 시점 checkout(reflog · measured_utc {measured} 이전 마지막 이동) = {} ≠ 지금 HEAD — 측정 뒤 이동: 측정한 revision 을 고르지 않는다(None)",
            core::head_chars(&last.0, 12)
        )));
    }
    let later = moves.iter().filter(|(_, u)| u.as_str() > measured).count();
    Ok((Some(sha.to_string()), format!(
        "{desc} · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc {measured} 이전 마지막 이동 {}{})",
        last.1,
        if later > 0 { format!(" · 측정 뒤 이동 {later}회 있었으나 같은 커밋") } else { String::new() }
    )))
}

/// 체크포인트 `.git/config` 의 huggingface.co remote url → HF repo id.
pub fn _hf_repo(host: &Path, label: &str) -> HR<(Option<String>, String)> {
    let p = host.join(".git").join("config");
    if !p.is_file() {
        return Ok((None, format!("체크포인트 .git/config 부재({label}) — HF repo 미관측")));
    }
    for line in py_splitlines(&read_lossy_hr(&p)?) {
        let s = line.trim();
        if s.starts_with("url") && s.contains('=') {
            let v = s.splitn(2, '=').nth(1).unwrap_or("").trim();
            if let Some(c) = hf_url_re().match_at_start(v) {
                return Ok((crate::pyre::group(&c, 1).map(str::to_string), format!("체크포인트 .git/config remote url(huggingface.co · {label})")));
            }
        }
    }
    Ok((None, format!("체크포인트 .git/config 에 huggingface.co remote 가 없다({label})")))
}

/// HF 카드(README 머리 YAML)의 `base_model` — 단일 값일 때만.
pub fn _base_model(host: &Path, label: &str) -> HR<(Option<String>, String)> {
    let p = host.join("README.md");
    if !p.is_file() {
        return Ok((None, format!("체크포인트 README.md 부재({label})")));
    }
    let lines = py_splitlines(&read_lossy_hr(&p)?);
    if lines.is_empty() || lines[0].trim() != "---" {
        return Ok((None, format!("README 머리 YAML 없음({label})")));
    }
    let mut vals: Vec<String> = Vec::new();
    let mut on = false;
    for ln in &lines[1..] {
        if ln.trim() == "---" {
            break;
        }
        if base_model_key_re().is_match(ln) {
            let rest = ln.splitn(2, ':').nth(1).unwrap_or("").trim();
            on = rest.is_empty();
            if !rest.is_empty() {
                vals.push(strip_chars(rest, "'\"[] ").to_string());
            }
            continue;
        }
        if on {
            if let Some(c) = list_item_re().match_at_start(ln) {
                vals.push(strip_chars(crate::pyre::group(&c, 1).unwrap_or("").trim(), "'\"").to_string());
                continue;
            }
            on = false;
        }
    }
    let vals: Vec<String> = vals.into_iter().filter(|v| hf_id_re().is_full(v)).collect();
    if vals.len() == 1 {
        return Ok((Some(vals[0].clone()), format!("체크포인트 README base_model({label})")));
    }
    if vals.is_empty() {
        return Ok((None, format!("README 머리 YAML 에 base_model 없음({label})")));
    }
    Ok((None, format!("README base_model 이 {}개 — 단일 기반 모델로 접지 않는다({label})", vals.len())))
}

fn ck_label(ck: &J) -> String {
    if ck["host_label"].truthy() { ps(&ck["host_label"]) } else { "체크포인트".to_string() }
}
fn hq_quant(ck: &J) -> J {
    if ck["hf_quant"].is_dict() { or_dict(&ck["hf_quant"])["quantization"].clone() } else { J::Null }
}

/// 체크포인트 → q 축 raw(체크포인트가 선언한 주 방식 · MIXED_PRECISION 만 우세 알고리즘).
pub fn _quant_raw(ck: &J) -> HR<J> {
    let label = ck_label(ck);
    if !ck["observed"].truthy() || !ck["config"].is_dict() {
        return Ok(jd! {"source" => ""});
    }
    let qc = &ck["config"]["quantization_config"];
    let hq = hq_quant(ck);
    let (method, algo, mut layers, mut wh);
    if qc.is_dict() {
        method = py_lower(&ps_or(&qc["quant_method"]));
        algo = qc["quant_algo"].clone();
        layers = qc["quantized_layers"].clone();
        wh = "config.json quantization_config".to_string();
    } else if hq.is_dict() {
        method = "modelopt".to_string();
        algo = hq["quant_algo"].clone();
        layers = hq["quantized_layers"].clone();
        wh = "hf_quant_config.json quantization".to_string();
    } else {
        return Ok(jd! {"raw" => "none", "source" => format!("{label}/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화")});
    }
    if method == "modelopt" {
        if ps_or(&algo).to_uppercase() == "MIXED_PRECISION" {
            if !layers.is_dict() && hq.is_dict() {
                layers = hq["quantized_layers"].clone();
                wh += " + hf_quant_config.json quantized_layers";
            }
            let mut algos: Vec<(J, i64)> = Vec::new();
            if let Some(m) = or_dict(&layers).as_dict() {
                for (_, v) in m.iter() {
                    let key = if v.is_dict() {
                        v["quant_algo"].clone()
                    } else if v.is_str() {
                        v.clone()
                    } else {
                        continue;
                    };
                    if let Some(e) = algos.iter_mut().find(|(k, _)| *k == key) {
                        e.1 += 1;
                    } else {
                        algos.push((key, 1));
                    }
                }
            }
            algos.retain(|(k, _)| !k.is_null());
            let mut top = algos.clone();
            top.sort_by(|a, b| b.1.cmp(&a.1));
            let shown = format!("{{{}}}", algos.iter().map(|(k, n)| format!("{}: {n}", k.repr())).collect::<Vec<_>>().join(", "));
            if top.is_empty() || (top.len() > 1 && top[0].1 == top[1].1) {
                return Ok(jd! {"source" => format!("{label}/{wh} MIXED_PRECISION quantized_layers 우세 알고리즘 판정 불가({shown})")});
            }
            let total: i64 = algos.iter().map(|x| x.1).sum();
            return Ok(jd! {"raw" => format!("modelopt-dominant:{}", ps(&top[0].0)),
                           "source" => format!("{label}/{wh}(modelopt MIXED_PRECISION) quantized_layers 우세 {} {}/{total}", ps(&top[0].0), top[0].1)});
        }
        if algo.truthy() {
            return Ok(jd! {"raw" => format!("modelopt:{}", ps(&algo)), "source" => format!("{label}/{wh} quant_method=modelopt quant_algo={}", ps(&algo))});
        }
        return Ok(jd! {"source" => format!("{label}/{wh} modelopt 인데 quant_algo 가 없다")});
    }
    if method == "compressed-tensors" && qc.is_dict() && qc["format"].truthy() {
        return Ok(jd! {"raw" => format!("compressed-tensors:{}", ps(&qc["format"])),
                       "source" => format!("{label}/{wh} quant_method=compressed-tensors format={}", ps(&qc["format"]))});
    }
    if !method.is_empty() {
        return Ok(jd! {"raw" => method.clone(), "source" => format!("{label}/{wh} quant_method={method}")});
    }
    Ok(jd! {"source" => format!("{label}/{wh} 에 quant_method 가 없다")})
}

/// 체크포인트 config 에서 관측되는 dtype 구성 → `[{scope, dtype, source[, count]}]`(해석 ✗).
pub fn _quant_composition(ck: &J) -> HR<(Option<Vec<J>>, String)> {
    let label = ck_label(ck);
    let cfg = if ck["observed"].truthy() { ck["config"].clone() } else { J::Null };
    if !cfg.is_dict() {
        let why = if ck["reason"].truthy() { ps(&ck["reason"]) } else { label.clone() };
        return Ok((None, format!("체크포인트 config 미관측({why}) — 구성 미기재")));
    }
    let mut out: Vec<J> = Vec::new();
    let qc = cfg["quantization_config"].clone();
    let hq = hq_quant(ck);
    let (wh, q) = if qc.is_dict() {
        ("config.json quantization_config", qc.clone())
    } else if hq.is_dict() {
        ("hf_quant_config.json quantization", hq.clone())
    } else {
        ("", J::Null)
    };
    if !q.is_null() {
        let m0 = if q["quant_method"].truthy() { ps(&q["quant_method"]) } else if wh.starts_with("hf_quant") { "modelopt".into() } else { String::new() };
        let method = py_lower(&m0);
        let algo = q["quant_algo"].clone();
        let mut layers = q["quantized_layers"].clone();
        if !layers.is_dict() && hq.is_dict() {
            layers = hq["quantized_layers"].clone();
        }
        if ps_or(&algo).to_uppercase() == "MIXED_PRECISION" && layers.is_dict() {
            let mut by: Vec<(String, Vec<String>)> = Vec::new();
            for (name, v) in layers.as_dict().unwrap().iter() {
                let a = if v.is_dict() { v["quant_algo"].clone() } else if v.is_str() { v.clone() } else { J::Null };
                if a.truthy() {
                    let key = ps(&a);
                    if let Some(e) = by.iter_mut().find(|(k, _)| *k == key) {
                        e.1.push(name.to_string());
                    } else {
                        by.push((key, vec![name.to_string()]));
                    }
                }
            }
            by.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
            for (a, names) in by {
                out.push(jd! {"scope" => format!("quantized_layers {}개(예 {})", names.len(), names[0]), "dtype" => a, "count" => names.len(),
                              "source" => format!("{label}/{wh} quantized_layers(modelopt MIXED_PRECISION)")});
            }
        } else {
            let detail = ["fmt", "weight_block_size", "group_size"]
                .iter()
                .filter(|k| !q[**k].is_null())
                .map(|k| format!("{k}={}", ps(&q[*k])))
                .collect::<Vec<_>>()
                .join(" · ");
            let dtype = if method == "modelopt" && algo.truthy() {
                format!("{method}:{}", ps(&algo))
            } else if !method.is_empty() {
                method.clone()
            } else {
                ps_or(&algo)
            };
            if !dtype.is_empty() {
                out.push(jd! {"scope" => "quantization_config 선언 대상(양자화 층 · 제외 목록 밖)",
                              "dtype" => dtype + &if detail.is_empty() { String::new() } else { format!(" ({detail})") },
                              "source" => format!("{label}/{wh} quant_method{}", if algo.truthy() { "·quant_algo" } else { "" })});
            }
        }
        let mut ign = if q["ignore"].is_list() { q["ignore"].clone() } else if q["exclude_modules"].is_list() { q["exclude_modules"].clone() } else { J::Null };
        if ign.is_null() && hq.is_dict() && hq["exclude_modules"].is_list() {
            ign = hq["exclude_modules"].clone();
        }
        if ign.truthy() {
            out.push(jd! {"scope" => format!("양자화 제외 모듈 {}개", ign.len()), "dtype" => "unquantized(아래 기본 dtype)", "count" => ign.len(),
                          "source" => format!("{label}/{wh} ignore·exclude_modules")});
        }
    }
    let tc = if cfg["text_config"].is_dict() { cfg["text_config"].clone() } else { J::dict() };
    let pairs = [(&cfg, "config.json"), (&tc, "config.json text_config")];
    for (d, dw) in pairs {
        if let Some(s) = d["expert_dtype"].as_str() {
            if !s.trim().is_empty() {
                out.push(jd! {"scope" => "routed experts", "dtype" => s.trim(), "source" => format!("{label}/{dw} expert_dtype")});
                break;
            }
        }
    }
    'outer: for k in ["torch_dtype", "dtype"] {
        for (d, dw) in pairs {
            if let Some(s) = d[k].as_str() {
                if !s.trim().is_empty() {
                    out.push(jd! {"scope" => "기본 dtype(비양자화 가중치·계산)", "dtype" => s.trim(), "source" => format!("{label}/{dw} {k}")});
                    break 'outer;
                }
            }
        }
    }
    if out.is_empty() {
        return Ok((Some(Vec::new()), format!("{label}/config.json 에 양자화·dtype 칸이 없다(관측 · 빈 구성)")));
    }
    Ok((Some(out), format!("{label}/config.json(+hf_quant_config.json) 관측 칸(evidence._quant_composition 닫힌 목록)")))
}

pub fn _ple_present(ck: &J) -> HR<Option<bool>> {
    let cfg = if ck["observed"].truthy() { ck["config"].clone() } else { J::Null };
    if !cfg.is_dict() {
        return Ok(None);
    }
    let tc = if cfg["text_config"].is_dict() { cfg["text_config"].clone() } else { J::dict() };
    for d in [&cfg, &tc] {
        if _PLE_KEYS.iter().any(|k| d.get(k).is_some()) {
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}

// ── naming facts (SPEC §3.3) ─────────────────────────────────────────────────────────────────────────────────
/// `_yaml_get(y, *keys)` → (첫 실재 키, 값) | (None, None).
pub fn _yaml_get(y: &J, keys: &[&str]) -> HR<(Option<String>, J)> {
    for k in keys {
        if let Some(v) = y.get(k) {
            return Ok((Some(k.to_string()), v.clone()));
        }
    }
    Ok((None, J::Null))
}

/// 측정 기록과 yaml 값의 동치(수치 · 참거짓 · 글자).
pub fn _same(a: &J, b: &J) -> HR<bool> {
    let ba = if let J::Bool(x) = a { Some(*x) } else { _boolish(&J::Str(ps(a)))? };
    let bb = if let J::Bool(x) = b { Some(*x) } else { _boolish(&J::Str(ps(b)))? };
    if let (Some(x), Some(y)) = (ba, bb) {
        return Ok(x == y);
    }
    match (py_float(&ps(a)), py_float(&ps(b))) {
        (Some(x), Some(y)) => Ok((x - y).abs() < 1e-9),
        _ => Ok(ps(a).trim().to_lowercase() == ps(b).trim().to_lowercase()),
    }
}

/// 측정 때의 서빙 노브 기록 {키: [값, 출처]}.
pub fn _measured_knobs(repo: &Path, ev: &J) -> HR<J> {
    let mut out = Map::new();
    let meta = sweep_meta(ev);
    let mut layers: Vec<(J, &str)> = vec![(meta.clone(), "sweep meta"), (or_dict(&ev["certificate"]), "인증서")];
    if !meta.truthy() && !ev["certificate"].truthy() {
        layers.push((_lite_snapshot(repo, ev["bench_report_path"].as_str())?, "리포트 측정 환경 표"));
    }
    for (doc, src) in layers {
        if let Some(m) = or_dict(&doc).as_dict() {
            for (k, v) in m.iter() {
                if !out.contains_key(k) && !_unobserved(v)? {
                    out.insert(k, jl![v.clone(), src]);
                }
            }
        }
    }
    Ok(J::Dict(out))
}

/// 디스크의 트리플렛이 측정한 그것인가 — 어긋나면 HINT_TRIPLET_DRIFT.
pub fn _drift_check(repo: &Path, y: &J, yrel: &str, env: &J, ev: &J) -> HR<()> {
    let rec = _measured_knobs(repo, ev)?;
    let mut bad: Vec<String> = Vec::new();
    let pairs: [(&str, &str, Option<J>); 8] = [
        ("max-model-len", "max_model_len", None),
        ("kv-cache-memory-bytes", "kv_cache_memory_bytes", None),
        ("gpu-memory-utilization", "gpu_memory_utilization", None),
        ("tensor-parallel-size", "tensor_parallel_size", None),
        ("max-num-seqs", "max_num_seqs", None),
        ("model", "model_path", None),
        ("kv-cache-dtype", "kv_cache_dtype", Some(J::from("auto"))),
        ("enforce-eager", "enforce_eager", Some(J::Bool(false))),
    ];
    for (yk, mk, default) in pairs {
        let Some(r) = rec.get(mk) else { continue };
        let got = if let Some(v) = y.get(yk) {
            v.clone()
        } else if let Some(d) = default {
            d
        } else {
            continue;
        };
        if !_same(&got, &r[0])? {
            bad.push(format!("{yk}: yaml={} {}={}", got.repr(), ps(&r[1]), r[0].repr()));
        }
    }
    let spec_on = match rec.get("spec_on") {
        Some(r) => _boolish(&J::Str(ps(&r[0])))?,
        None => None,
    };
    if let Some(on) = spec_on {
        let (k, _) = _yaml_get(y, &["speculative-config", "speculative_config"])?;
        if k.is_some() != on {
            let r = &rec["spec_on"];
            bad.push(format!(
                "speculative-config: yaml {} {} spec_on={}",
                if k.as_deref().map(|s| !s.is_empty()).unwrap_or(false) { "있음" } else { "없음" },
                ps(&r[1]),
                r[0].repr()
            ));
        }
    }
    let decl_img = rec.get("image_tag_declared").map(|r| r[0].clone()).unwrap_or(J::Null);
    if env["IMAGE_TAG"].truthy() && !decl_img.is_null() && !py_eq(&env["IMAGE_TAG"], &decl_img) {
        bad.push(format!("IMAGE_TAG: env={} meta.image_tag_declared={}", env["IMAGE_TAG"].repr(), decl_img.repr()));
    }
    if !bad.is_empty() {
        return core::fail(
            "HINT_TRIPLET_DRIFT",
            format!("디스크 트리플렛({yrel})이 측정 때와 다르다: {}", repr_strs(&bad)),
            Some("측정한 그 트리플렛으로 되돌리거나, 바뀐 트리플렛으로 다시 서빙·측정한다(이름은 측정한 것에서 짓는다)."),
        );
    }
    Ok(())
}

/// SPEC §3.3 naming facts — 각 축은 `{raw, source}`.
pub fn naming_facts(repo: &Path, ev: &J) -> HR<J> {
    let repo = _repo(repo)?;
    let topology = ps(&ev["topology"]);
    let cell = ps(&ev["cell"]);
    let (y, yrel) = _serving_yaml(&repo, ev)?;
    let y = y.filter(|v| v.truthy()).unwrap_or_else(J::dict);
    let env_t = &ev["triplet"]["env"];
    let env = _env_first(&repo, env_t.as_str().filter(|s| !s.is_empty()))?;
    let env_rel = if env_t.truthy() { ps(env_t) } else { format!("output/{topology}/envs/.env.{cell}(부재)") };
    if !yrel.is_empty() && ev["triplet"]["yaml"].truthy() {
        _drift_check(&repo, &y, &yrel, &env, ev)?;
    }
    let b = or_dict(&ev["build_identity"]);
    let bsrc = or_dict(&b["source"]);
    let conflicts = or_dict(&b["env_history_conflicts"]);
    if conflicts.truthy() {
        return core::fail(
            "HINT_VLLM_INPUT_CONFLICT",
            format!("셀 env 와 이미지 history build-arg 가 다르다: {}", ps(&conflicts)),
            Some("측정 이미지가 다른 입력으로 빌드됐다 — 셀 env 를 실제 빌드 입력으로 바로잡거나 다시 빌드한다."),
        );
    }
    let ck = _checkpoint(&repo, ev)?;
    let (plane, plane_src) = crate::artifacts::plane_and_source(&repo, ev, &J::Null)?;
    let track = if plane == "native" && b["native_wheelhouse_source"].truthy() && b["track"].truthy() {
        b["track"].clone()
    } else if plane == "native" {
        J::from("native")
    } else {
        b["track"].clone()
    };
    let dflt = |k: &str| bsrc.get(k).map(ps).unwrap_or_else(|| "미관측".to_string());
    let src = [
        format!("track={}", ps(&bsrc["track"])),
        format!("ref={}", ps(&bsrc["vllm_ref"])),
        format!("version={}", ps(&bsrc["vllm_version"])),
        format!("repo={}", dflt("vllm_repo")),
        format!("sha={}", dflt("vllm_sha")),
        format!("exact_tag={}", dflt("image_git_exact_tag")),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let vllm = jd! {"track" => track, "vllm_ref" => b["vllm_ref"].clone(), "vllm_version" => b["vllm_version"].clone(),
                    "vllm_repo" => b["vllm_repo"].clone(), "vllm_sha" => b["vllm_sha"].clone(), "prev_release" => _prev_release(ev)?,
                    "wheel_url" => b["wheel_url"].clone(), "source" => src};
    let mut model = jd! {"hf_repo" => ck["hf_repo"].clone(), "model_path" => ck["model_path"].clone(),
                         "source" => format!("서빙 yaml model({yrel})") + &if ck["hf_repo"].truthy() { format!(" · hf_repo={}", ps(&ck["hf_repo_source"])) } else { String::new() }};
    if !ck["model_path"].truthy() {
        model.set("source", "");
    }
    let arch = _arch_facts(&repo, ev)?;
    let q = _quant_raw(&ck)?;
    let mut facts = jd! {"vllm" => vllm, "model" => model, "arch" => arch, "q" => q};
    let (k, v) = _yaml_get(&y, &["max-model-len", "max_model_len"])?;
    facts.set(
        "len",
        match k.filter(|k| !k.is_empty()) {
            Some(k) => jd! {"raw" => match _as_int(&v)? { Some(n) => J::Int(n), None => v.clone() }, "source" => format!("서빙 yaml {k}({yrel})")},
            None => jd! {"source" => format!("서빙 yaml 에 max-model-len 없음({yrel}) — 엔진 기본값(모델 최대)은 이름으로 쓰지 않는다")},
        },
    );
    let (k, v) = _yaml_get(&y, &["kv-cache-dtype", "kv_cache_dtype"])?;
    facts.set(
        "kv",
        match k.filter(|k| !k.is_empty()) {
            Some(k) => jd! {"raw" => ps(&v), "source" => format!("서빙 yaml {k}({yrel})")},
            None if ev["triplet"]["yaml"].truthy() => jd! {"raw" => "auto", "source" => format!("서빙 yaml kv-cache-dtype 부재 관측({yrel}) → 엔진 기본값 auto")},
            None => jd! {"source" => ""},
        },
    );
    facts.set("spec", _spec_fact(&repo, ev, &y, &yrel)?);
    facts.set("graph", _graph_fact(&repo, ev, &y, &yrel)?);
    facts.set("ple", _ple_fact(ev, &env, &env_rel, &ck)?);
    let mut psrc = format!("artifacts.plane_of → {plane_src}");
    if plane == "native" {
        psrc += &format!(" · env: IMAGE_TAG/BUILD_DOCKERFILE 없음({env_rel})");
        if ev["sources"]["plane"].truthy() {
            psrc += &format!(" · serve_proof {}", ps(&ev["sources"]["plane"]));
        }
    }
    facts.set("plane", jd! {"raw" => plane, "source" => psrc});
    Ok(facts)
}

// ── 이름 꼬리 근거 대조 입력 ───────────────────────────────────────────────────────────────────────────────────
crate::re_static!(_SH_FLAG_RE, r"^--([A-Za-z0-9][A-Za-z0-9_-]*)(?:=(.*))?$");
crate::re_static!(_SH_ASSIGN_RE, r"^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$");

/// 평탄화 값 → 문자열(bool 소문자 · None 빈 문자열 · 컨테이너 = 정렬 압축 JSON).
pub fn _flat_str(v: &J) -> HR<String> {
    Ok(match v {
        J::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        J::Null => String::new(),
        J::Dict(_) | J::List(_) => crate::value::dumps_tight_sorted(v),
        other => ps(other),
    })
}

/// 중첩 yaml → `a.b` 평탄 키(목록 `a.0` · 문자열 JSON 객체 한 단계 펼침).
pub fn _flatten(prefix: &str, v: &J, out: &mut Map) -> HR<()> {
    match v {
        J::Dict(m) => {
            for (k, x) in m.iter() {
                let key = if prefix.is_empty() { k.to_string() } else { format!("{prefix}.{k}") };
                _flatten(&key, x, out)?;
            }
            Ok(())
        }
        J::List(l) => {
            out.insert(prefix, _flat_str(v)?);
            for (i, x) in l.iter().enumerate() {
                _flatten(&format!("{prefix}.{i}"), x, out)?;
            }
            Ok(())
        }
        _ => {
            out.insert(prefix, _flat_str(v)?);
            if let Some(s) = v.as_str() {
                if s.trim().starts_with('{') {
                    if let Ok(J::Dict(m)) = crate::value::loads(s) {
                        for (k, x) in m.iter() {
                            _flatten(&format!("{prefix}.{k}"), x, out)?;
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

pub fn _flat_env(text: &str) -> HR<J> {
    let mut out = Map::new();
    for ln in py_splitlines(text) {
        let s = ln.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if let Some(c) = _SH_ASSIGN_RE().match_at_start(s) {
            let mut val: String = crate::pyre::group(&c, 2).unwrap_or("").trim().to_string();
            let ch: Vec<char> = val.chars().collect();
            if ch.len() >= 2 && ch[0] == ch[ch.len() - 1] && (ch[0] == '\'' || ch[0] == '"') {
                val = ch[1..ch.len() - 1].iter().collect();
            }
            out.insert(crate::pyre::group(&c, 1).unwrap_or(""), val);
        }
    }
    Ok(J::Dict(out))
}

/// Python `shlex.split(s, comments=True)` (posix). `Err` = ValueError.
fn shlex_split(s: &str) -> Result<Vec<String>, String> {
    let ws = |c: char| " \t\r\n".contains(c);
    let mut toks = Vec::new();
    let mut tok = String::new();
    let mut quoted = false;
    let mut state = ' ';
    let mut esc_state = 'a';
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    loop {
        let c = chars.get(i).copied();
        i += 1;
        match state {
            ' ' | 'a' => {
                let Some(c) = c else { break };
                if ws(c) {
                    if state == 'a' {
                        state = ' ';
                        toks.push(std::mem::take(&mut tok));
                        quoted = false;
                    }
                } else if c == '#' {
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    i += 1;
                    if state == 'a' {
                        state = ' ';
                        toks.push(std::mem::take(&mut tok));
                        quoted = false;
                    } else if quoted {
                        toks.push(std::mem::take(&mut tok));
                        quoted = false;
                    }
                } else if c == '\\' {
                    esc_state = 'a';
                    state = '\\';
                } else if c == '\'' || c == '"' {
                    quoted = true;
                    state = c;
                } else {
                    tok.push(c);
                    state = 'a';
                }
            }
            '\'' | '"' => {
                let Some(c) = c else { return Err("No closing quotation".into()) };
                if c == state {
                    state = 'a';
                } else if c == '\\' && state == '"' {
                    esc_state = state;
                    state = '\\';
                } else {
                    tok.push(c);
                }
            }
            _ => {
                let Some(c) = c else { return Err("No escaped character".into()) };
                if (esc_state == '\'' || esc_state == '"') && c != esc_state && c != '\\' {
                    tok.push('\\');
                }
                tok.push(c);
                state = esc_state;
            }
        }
    }
    if state == 'a' || quoted {
        if !tok.is_empty() || quoted {
            toks.push(tok);
        }
    }
    Ok(toks)
}

/// 러너 .sh → {`--flag`: 값} + 변수 대입 `K=V`(셸을 실행하지 않는다).
pub fn _flat_sh(text: &str) -> HR<J> {
    let mut out = Map::new();
    for ln in py_splitlines(text) {
        let s = ln.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if let Some(c) = _SH_ASSIGN_RE().match_at_start(s) {
            let v = crate::pyre::group(&c, 2).unwrap_or("").trim();
            if !v.contains(' ') {
                out.insert(crate::pyre::group(&c, 1).unwrap_or(""), strip_chars(v, "'\""));
            }
        }
        let Ok(toks) = shlex_split(s) else { continue };
        for (i, t) in toks.iter().enumerate() {
            let Some(fm) = _SH_FLAG_RE().match_at_start(t) else { continue };
            let name = crate::pyre::group(&fm, 1).unwrap_or("");
            if let Some(v) = crate::pyre::group(&fm, 2) {
                out.insert(format!("--{name}"), v);
            } else if i + 1 < toks.len() && !toks[i + 1].starts_with("--") && !["\\", "|", "&&", ";"].contains(&toks[i + 1].as_str()) {
                out.insert(format!("--{name}"), toks[i + 1].clone());
            } else {
                out.insert(format!("--{name}"), "true");
            }
        }
    }
    Ok(J::Dict(out))
}

/// 이 셀의 서빙 설정 파일 → {저장소 상대경로: {평탄 key: 값(str)}}(naming.validate_tail 의 근거 대조 입력).
pub fn tail_sources(repo: &Path, ev: &J) -> HR<J> {
    let repo = _repo(repo)?;
    let mut out = Map::new();
    let mut items: Vec<(String, J)> = or_dict(&ev["triplet"]).as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()).unwrap_or_default();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    for (kind, rel) in items {
        let rel = ps(&rel);
        let p = repo.join(&rel);
        if !p.is_file() {
            continue;
        }
        match kind.as_str() {
            "yaml" => {
                let doc = _owner_yaml(&repo, &p, "서빙 yaml")?;
                if doc.is_dict() {
                    let mut flat = Map::new();
                    _flatten("", &doc, &mut flat)?;
                    out.insert(rel, J::Dict(flat));
                }
            }
            "env" => out.insert(rel, _flat_env(&read_lossy_hr(&p)?)?),
            "sh" => out.insert(rel, _flat_sh(&read_lossy_hr(&p)?)?),
            _ => {}
        }
    }
    if ev["mode"].as_str() == Some("campaign") && ev["campaign_id"].truthy() {
        let rel = format!("campaigns/{}/cells/{}/config.yaml", ps(&ev["campaign_id"]), ps(&ev["cell"]));
        if repo.join(&rel).is_file() {
            let doc = _owner_yaml(&repo, &repo.join(&rel), "셀 config")?;
            if doc.is_dict() {
                let mut flat = Map::new();
                _flatten("", &doc, &mut flat)?;
                out.insert(rel, J::Dict(flat));
            }
        }
    }
    Ok(J::Dict(out))
}

/// attestation 이 든 메인 빌드 원장의 `vllm` 사전(v1 최상위 · v2 nodes.main) · 없으면 {}.
pub fn _ledger_vllm(att: &J) -> HR<J> {
    let att = or_dict(att);
    let second = if att["nodes"].is_dict() { or_dict(&or_dict(&att["nodes"])["main"])["build_ledger"].clone() } else { J::Null };
    for led in [att["build_ledger"].clone(), second] {
        if led.is_dict() && led["vllm"].is_dict() {
            return Ok(led["vllm"].clone());
        }
    }
    Ok(J::dict())
}

/// 커밋 핀 릴리스: 원장 describe 또는 측정 이미지의 격리된 Git 정확 태그.
pub fn _prev_release(ev: &J) -> HR<Option<String>> {
    let d = _ledger_vllm(&ev["attestation"])?["describe"].clone();
    let exact = or_dict(&ev["build_identity"])["image_git_exact_tag"].clone();
    if d.truthy() && exact.truthy() && crate::naming::_describe_release(&d)? != crate::naming::_describe_release(&exact)? {
        return core::fail("HINT_VLLM_INPUT_CONFLICT", "build ledger describe conflicts with measured image exact tag", None);
    }
    Ok(if d.truthy() {
        Some(ps(&d))
    } else if exact.is_null() {
        None
    } else {
        Some(ps(&exact))
    })
}

pub fn _arch_facts(repo: &Path, ev: &J) -> HR<J> {
    let man = or_dict(&ev["manifest"]);
    let msrc = ps_or(&ev["sources"]["manifest"]);
    if !man.truthy() {
        return Ok(jd! {"gpu_model" => J::Null, "source" => msrc});
    }
    if !man["topology"].is_null() && !py_eq(&man["topology"], &ev["topology"]) {
        return core::fail(
            "HINT_TOPOLOGY_MANIFEST_MISMATCH",
            format!("manifest topology={} ≠ 셀 토폴로지 {}", man["topology"].repr(), ev["topology"].repr()),
            Some("토폴로지 사실의 권위는 manifest 다 — 이 체크아웃 통로를 확인한다(4자일치)."),
        );
    }
    let nodes: Vec<J> = man["nodes"].items().iter().filter(|n| n.is_dict()).cloned().collect();
    let gpn = _as_int(&man["gpus_per_node"])?;
    let role = ps(&ev["node"]);
    let mut gpu = man["gpu_model"].clone();
    let count: i64;
    if role == "cluster" {
        count = nodes.len() as i64;
        if count < 2 {
            return core::fail("HINT_AXIS_UNDERIVABLE", format!("cluster 축인데 manifest nodes[] 가 {count}개다."), None);
        }
    } else {
        count = 1;
        if role == "sub" && man["self_role"].as_str() != Some("sub") {
            let entry = nodes.iter().find(|n| n["role"].as_str() == Some("sub")).cloned().unwrap_or_else(J::dict);
            gpu = if entry["gpu_model"].truthy() { entry["gpu_model"].clone() } else { J::Null };
        }
    }
    let (measured, msrc2) = if ev["certificate"].truthy() {
        (_norm_value(&ev["certificate"]["gpu_model"])?, "인증서 gpu_model")
    } else {
        (_norm_value(&sweep_meta(ev)["gpu_model"])?, "sweep meta gpu_model")
    };
    if measured.truthy() && gpu.truthy() && ps(&measured).trim().to_lowercase() != ps(&gpu).trim().to_lowercase() {
        return core::fail(
            "HINT_ARCH_GPU_CONFLICT",
            format!("manifest gpu_model={} ≠ {msrc2}={} — 다른 하드웨어의 측정이다.", gpu.repr(), measured.repr()),
            None,
        );
    }
    let (six, six_src, per_key) = _measured_identity(repo, ev)?;
    let mut tp = six["tp"].clone();
    let mut tp_src = per_key.get("tp").map(ps).unwrap_or(six_src);
    if tp.is_null() && ev["triplet"]["yaml"].truthy() {
        let (y, yrel) = _serving_yaml(repo, ev)?;
        tp = J::from(_as_int(&or_dict(&y.unwrap_or(J::Null))["tensor-parallel-size"])?);
        tp_src = format!("서빙 yaml tensor-parallel-size({yrel})");
    }
    if let (false, Some(g)) = (tp.is_null(), gpn.filter(|g| *g != 0)) {
        let tpi = tp.as_i64();
        if role == "cluster" && tpi != Some(count * g) {
            return core::fail("HINT_ARCH_TP_CONFLICT", format!("cluster {count}노드×{g}GPU ≠ 측정 TP={}({tp_src})", ps(&tp)), None);
        }
        if (role == "main" || role == "sub") && tpi.map(|t| t > g).unwrap_or(false) {
            return core::fail(
                "HINT_ARCH_TP_CONFLICT",
                format!("발행 노드 축 {}(1노드×{g}GPU)인데 측정 TP={}({tp_src}) — 단일 노드 형상이 아니다.", crate::value::py_repr(&role), ps(&tp)),
                Some("두 노드를 묶은 측정이면 `--node cluster` 로 발행한다(한 태그 = 한 노드 형상의 실행 기록)."),
            );
        }
    }
    let mut arch = jd! {"gpu_model" => gpu, "gpus_per_node" => gpn, "nodes" => count, "role" => role.clone(),
                        "source" => format!("{msrc} gpu_model·gpus_per_node·nodes[] · 발행 노드 축({})", ps(&ev["sources"]["node"]))
                            + &if measured.truthy() { format!(" · {msrc2} 일치") } else { String::new() }};
    let (tgt, tsrc) = _target_gpu(ev, Some(repo))?;
    if let Some(ts) = tsrc {
        arch.set("target_gpu", tgt);
        let s = ps(&arch["source"]) + &format!(" · target: {ts}");
        arch.set("source", s);
    }
    Ok(arch)
}

fn decl_target(v: &J) -> Option<String> {
    v.as_str().filter(|s| !s.trim().is_empty() && !s.contains("<<FILL>>")).map(|s| s.trim().to_string())
}

fn promotion_anchor(ev: &J) -> J {
    let pubn = if ev["publication"].is_dict() { ev["publication"].clone() } else { J::dict() };
    for d in [pubn["record"].clone(), pubn["manifest"].clone(), pubn.clone()] {
        if d.is_dict() && d["promotion_target"].is_dict() {
            return or_dict(&d["promotion_target"])["anchor"].clone();
        }
    }
    J::Null
}

fn json_get_attr(doc: &J, what: &str) -> HR<J> {
    if doc.is_dict() {
        Ok(doc[what].clone())
    } else {
        Err(HintError::py("AttributeError", format!("'{}' object has no attribute 'get'", doc.type_name())))
    }
}

/// 시뮬레이션 타겟 선언(셀 config > 캠페인 control_variables > 재생 전용 대체).
pub fn _target_gpu(ev: &J, repo: Option<&Path>) -> HR<(Option<String>, Option<String>)> {
    let cfg = or_dict(&ev["cell_config"]);
    if cfg["target_gpu"].is_dict() {
        let v = &cfg["target_gpu"]["gpu_model"];
        if let Some(t) = decl_target(v) {
            return Ok((Some(t), Some("셀 config target_gpu.gpu_model(선언)".into())));
        }
        if v.is_null() {
            return Ok((None, Some("셀 config target_gpu.gpu_model 이 null — no-simulation-target-declared".into())));
        }
    }
    let cv = or_dict(&or_dict(&ev["declaration"])["control_variables"]);
    if let Some(t) = decl_target(&cv["target_gpu"]) {
        return Ok((Some(t), Some("캠페인 control_variables.target_gpu(선언)".into())));
    }
    if ev["mode"].as_str() == Some("campaign") {
        return Ok((None, None));
    }
    let roof = or_dict(&or_dict(&ev["sweep"])["roofline"]);
    if let Some(gm) = roof["gpu_model"].as_str().filter(|s| !s.trim().is_empty()) {
        let phrase = ps(&campaign_absence(ev, repo)?["phrase"]);
        return Ok((Some(gm.trim().to_string()), Some(format!(
            "{}/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · {phrase} 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님)",
            ps(&or_dict(&ev["sweep"])["dir"])
        ))));
    }
    let anchor = promotion_anchor(ev);
    if let (Some(repo), Some(a)) = (repo, anchor.as_str()) {
        if anchor_re().is_full(a) {
            let r = core::git_nocheck(repo, &["show", &format!("{a}:PAYLOAD.json")])?;
            let ax = if r.returncode == 0 {
                match crate::value::loads(&r.stdout) {
                    Ok(doc) => {
                        let n = json_get_attr(&doc, "naming")?;
                        or_dict(&or_dict(&n)["axes"])["target"].clone()
                    }
                    Err(_) => J::Null,
                }
            } else {
                J::Null
            };
            if ax.is_dict() && ax["value"].as_str() == Some("native") {
                let phrase = ps(&campaign_absence(ev, Some(repo))?["phrase"]);
                return Ok((None, Some(format!(
                    "봉인된 발행 페이로드 {}:PAYLOAD.json naming.axes.target=native(발행 당시 셀 config 선언 관측) · {phrase} 로 선언 원본 미관측(재생 전용 대체)",
                    core::head_chars(a, 12)
                ))));
            }
        }
    }
    Ok((None, None))
}

/// 재생 전용: 발행 기록 `promotion_target.anchor` 의 PAYLOAD.json → (문서, 앵커).
pub fn _sealed_payload(repo: &Path, ev: &J) -> HR<(Option<J>, Option<String>)> {
    let anchor = promotion_anchor(ev);
    let Some(a) = anchor.as_str().filter(|a| anchor_re().is_full(a)) else { return Ok((None, None)) };
    let r = core::git_nocheck(repo, &["show", &format!("{a}:PAYLOAD.json")])?;
    if r.returncode != 0 {
        return Ok((None, Some(a.to_string())));
    }
    match crate::value::loads(&r.stdout) {
        Ok(doc) => Ok((if doc.is_dict() { Some(doc) } else { None }, Some(a.to_string()))),
        Err(_) => Ok((None, Some(a.to_string()))),
    }
}

/// 재생 전용 대체: 봉인 페이로드가 기록한 campaign · approval(출처 첨부).
pub fn sealed_publication(repo: &Path, ev: &J) -> HR<Option<J>> {
    if ev["mode"].as_str() == Some("campaign") {
        return Ok(None);
    }
    let (doc, anchor) = _sealed_payload(repo, ev)?;
    let Some(doc) = doc else { return Ok(None) };
    let anchor = anchor.unwrap_or_default();
    let camp = if doc["campaign"].is_dict() { doc["campaign"].clone() } else { J::dict() };
    if (!camp["cell"].is_null() && !py_eq(&camp["cell"], &ev["cell"])) || (!camp["node"].is_null() && !py_eq(&camp["node"], &ev["node"])) {
        return Ok(None);
    }
    let src = format!(
        "봉인된 발행 페이로드 {}:PAYLOAD.json(발행 기록 promotion_target.anchor · 원 발행 당시 기록) · {} 로 원본 미관측(재생 전용 대체)",
        core::head_chars(&anchor, 12),
        ps(&campaign_absence(ev, Some(repo))?["phrase"])
    );
    let ap = if doc["approval"].is_dict() && doc["approval"].truthy() { Some(doc["approval"].clone()) } else { None };
    let cid = camp["id"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let approval = match ap {
        Some(ap) => {
            let mut m = Map::new();
            for k in ["approved_by", "approved_utc", "source"] {
                if let Some(v) = ap.get(k) {
                    m.insert(k, v.clone());
                }
            }
            J::Dict(m)
        }
        None => J::Null,
    };
    Ok(Some(jd! {"campaign" => match cid { Some(c) => jd! {"id" => c, "cell" => camp["cell"].clone(), "node" => camp["node"].clone()}, None => J::Null },
                 "approval" => approval, "anchor" => anchor, "source" => src}))
}

fn sh_text_of(repo: &Path, ev: &J) -> HR<(J, String)> {
    let sh_rel = ev["triplet"]["sh"].clone();
    let text = if sh_rel.truthy() { read_lossy_hr(&repo.join(ps(&sh_rel)))? } else { String::new() };
    Ok((sh_rel, text))
}

pub fn _spec_fact(repo: &Path, ev: &J, y: &J, yrel: &str) -> HR<J> {
    let (k, v) = _yaml_get(y, &["speculative-config", "speculative_config"])?;
    let (sh_rel, sh_text) = sh_text_of(repo, ev)?;
    let in_sh = spec_sh_re().is_search(&sh_text);
    let Some(k) = k else {
        if in_sh {
            return Ok(jd! {"source" => format!("러너({})가 speculative 인자를 CLI 로 넘긴다 — 파싱하지 않는다(이름을 추측하지 않는다)", ps(&sh_rel))});
        }
        if !ev["triplet"]["yaml"].truthy() {
            return Ok(jd! {"source" => ""});
        }
        return Ok(jd! {"raw" => J::Null, "source" => format!("서빙 yaml·러너에 speculative-config 부재 관측({yrel})")});
    };
    if in_sh {
        return core::fail("HINT_SPEC_CONFLICT", format!("speculative 설정이 yaml({yrel})과 러너({}) 두 곳에 있다.", ps(&sh_rel)), None);
    }
    let cfg = match v.as_str() {
        Some(s) => match crate::value::loads(s) {
            Ok(d) => d,
            Err(_) => return Ok(jd! {"source" => format!("서빙 yaml {k} 가 JSON 이 아니다({yrel})")}),
        },
        None => v.clone(),
    };
    let n = if cfg.is_dict() { cfg["num_speculative_tokens"].clone() } else { J::Null };
    match _as_int(&n)? {
        Some(ni) if ni >= 1 => Ok(jd! {"raw" => ni, "source" => format!("서빙 yaml {k}.num_speculative_tokens({yrel} · method={})", ps(&cfg["method"])),
                                       "evidence" => jd! {"file" => yrel, "key" => format!("{k}.num_speculative_tokens"), "value" => _flat_str(&n)?}}),
        _ => Ok(jd! {"source" => format!("서빙 yaml {k} 에 num_speculative_tokens(≥1)가 없다({yrel})")}),
    }
}

pub fn _graph_fact(repo: &Path, ev: &J, y: &J, yrel: &str) -> HR<J> {
    if !ev["triplet"]["yaml"].truthy() {
        return Ok(jd! {"source" => ""});
    }
    let (k, v) = _yaml_get(y, &["enforce-eager", "enforce_eager"])?;
    let (sh_rel, sh_text) = sh_text_of(repo, ev)?;
    if let Some(k) = k {
        let b = if let J::Bool(x) = v { Some(x) } else { _boolish(&J::Str(ps(&v)))? };
        let Some(b) = b else {
            return Ok(jd! {"source" => format!("서빙 yaml {k}={} 를 참/거짓으로 읽지 못했다({yrel})", v.repr())});
        };
        return Ok(jd! {"raw" => if b { "eager" } else { "graph" }, "source" => format!("서빙 yaml {k}: {}({yrel})", if b { "true" } else { "false" }),
                       "evidence" => jd! {"file" => yrel, "key" => k.clone(), "value" => _flat_str(&v)?}});
    }
    if let Some(m) = eager_sh_re().search(&sh_text) {
        let g0 = crate::pyre::group(&m, 0).unwrap_or("").to_string();
        let flat = _flat_sh(&sh_text)?;
        let val = flat.get(&g0).cloned().unwrap_or(J::from("true"));
        return Ok(jd! {"raw" => "eager", "source" => format!("러너 --enforce-eager({})", ps(&sh_rel)),
                       "evidence" => jd! {"file" => sh_rel.clone(), "key" => g0, "value" => val}});
    }
    let (k2, cc) = _yaml_get(y, &["compilation-config", "compilation_config"])?;
    if let Some(k2) = k2 {
        let doc = match cc.as_str() {
            Some(s) => match crate::value::loads(s) {
                Ok(d) => d,
                Err(_) => return Ok(jd! {"source" => format!("서빙 yaml {k2} 가 JSON 이 아니다({yrel})")}),
            },
            None => cc.clone(),
        };
        let mode = if doc.is_dict() { ps_or(&doc["cudagraph_mode"]).to_uppercase() } else { String::new() };
        if mode == "NONE" {
            return Ok(jd! {"raw" => "eager", "source" => format!("서빙 yaml {k2}.cudagraph_mode=NONE({yrel})"),
                           "evidence" => jd! {"file" => yrel, "key" => format!("{k2}.cudagraph_mode"), "value" => ps(&doc["cudagraph_mode"])}});
        }
    }
    Ok(jd! {"raw" => "graph", "source" => format!("서빙 yaml enforce-eager 부재 관측({yrel}) → 엔진 기본값 cudagraph")})
}

/// PLE 축 — serve 시점의 셀 env 가 권위 + 모델 config 의 PLE 실재.
pub fn _ple_fact(ev: &J, env: &J, env_rel: &str, ck: &J) -> HR<J> {
    let present = _ple_present(ck)?;
    let label = ck_label(ck);
    let meta = sweep_meta(ev);
    let decl = if ev["cell_config"].is_dict() { or_dict(&or_dict(&ev["cell_config"])["declared_axes"])["ple_mode"].clone() } else { J::Null };
    let keys = J::from(_PLE_KEYS.to_vec()).repr();
    if present == Some(false) && env["VLLM_PLE_MMAP"].as_str() == Some("1") {
        return core::fail(
            "HINT_PLE_CONFLICT",
            format!("셀 env VLLM_PLE_MMAP=1({env_rel}) 인데 모델 config 에 PLE 키{keys} 가 없다({label})."),
            Some("PLE 없는 모델이면 셀 env 의 VLLM_PLE_MMAP 줄을 지우고 다시 측정한다 · PLE 모델이면 evidence._PLE_KEYS (tripwire 목록)에 그 형식의 키를 등재한다."),
        );
    }
    let decl_ok = decl.as_str().map(|s| !s.trim().is_empty() && !s.contains("<<FILL>>")).unwrap_or(false);
    let mut out;
    if present == Some(false) {
        out = jd! {"raw" => "none", "source" => format!("모델 config PLE 키{keys} 부재 관측({label})")};
    } else if ev["triplet"]["env"].truthy() {
        let mm = env["VLLM_PLE_MMAP"].clone();
        if mm.as_str() == Some("1") {
            out = jd! {"raw" => "mmap", "source" => format!("셀 env VLLM_PLE_MMAP=1({env_rel})"),
                       "evidence" => jd! {"file" => env_rel, "key" => "VLLM_PLE_MMAP", "value" => "1"}};
        } else if present == Some(true) {
            out = jd! {"raw" => "resident", "source" => format!("셀 env VLLM_PLE_MMAP={}(부재/0 = stock resident · {env_rel}) · 모델 PLE 실재({label})", mm.repr())};
            if !mm.is_null() {
                out.set("evidence", jd! {"file" => env_rel, "key" => "VLLM_PLE_MMAP", "value" => ps(&mm)});
            }
        } else if decl_ok {
            out = jd! {"raw" => decl.as_str().unwrap().trim(), "source" => "셀 config declared_axes.ple_mode(선언 · 모델 config 미관측)"};
        } else {
            out = jd! {"source" => format!("모델 config 미관측이라 PLE 실재를 판정하지 못했다({label}) — env 에 mmap 도 없다")};
        }
    } else if decl_ok {
        out = jd! {"raw" => decl.as_str().unwrap().trim(), "source" => "셀 config declared_axes.ple_mode(선언 · 셀 env 부재)"};
    } else if !_unobserved(&meta["ple_mode"])? {
        out = jd! {"raw" => meta["ple_mode"].clone(), "source" => format!("sweep meta ple_mode({})", ps(&meta["ple_mode_source"]))};
    } else {
        out = jd! {"source" => ""};
    }
    if ps(&out["source"]).starts_with("셀 config declared_axes.ple_mode") && ev["mode"].as_str() == Some("campaign") && ev["campaign_id"].truthy() {
        out.set("evidence", jd! {"file" => format!("campaigns/{}/cells/{}/config.yaml", ps(&ev["campaign_id"]), ps(&ev["cell"])),
                                 "key" => "declared_axes.ple_mode", "value" => ps(&decl).trim()});
    }
    let mps = meta.get("ple_mode_source").map(ps).unwrap_or_default();
    if out.get("raw").is_some()
        && !_unobserved(&meta["ple_mode"])?
        && mps.starts_with("declared")
        && (meta["ple_mode"].as_str() == Some("mmap")) != (out["raw"].as_str() == Some("mmap"))
    {
        return core::fail(
            "HINT_PLE_CONFLICT",
            format!("PLE 축이 어긋난다: 파생={}({}) · 스윕 meta={}", out["raw"].repr(), ps(&out["source"]), meta["ple_mode"].repr()),
            Some("측정 때의 셀 env 로 되돌리거나 다시 측정한다."),
        );
    }
    Ok(out)
}

// ── 캠페인 승인 · publish 위상 ───────────────────────────────────────────────────────────────────────────────
/// O6/X3 사전승인(campaign_template_validator 판정 한 벌 · 결손 기록은 죽는다) · 승인 없음 = None.
pub fn approval_for(repo: &Path, campaign_id: &J, cell: &J, node: &J) -> HR<Option<J>> {
    let repo = _repo(repo)?;
    let cid = _check_campaign_id(campaign_id)?;
    let decl = _read_json_opt(&repo.join("campaigns").join(&cid).join("campaign.yaml"), "campaign.yaml")?;
    if !decl.is_dict() {
        return core::fail("HINT_CAMPAIGN_ABSENT", format!("campaigns/{cid}/campaign.yaml 부재"), None);
    }
    _validator(&repo)?;
    let reasons = v_hint_target_reasons(&decl);
    if !reasons.is_empty() {
        return core::fail(
            "HINT_APPROVAL_INVALID",
            format!("hint_targets 승인 기록이 무너졌다: {}", repr_strs(&reasons)),
            // plan_26111708 B L8: the remedy names the live `campaign_init.sh` launcher.
            Some("`campaign_init.sh --hint-approve --campaign-id <id> --node N --cells … --approved-by <발화 전사> --utc U` 로 다시 적는다(손편집 ✗)."),
        );
    }
    let declared = declared_nodes(&decl);
    let (mut cands, kind) = v_resolve_measurement_node(&_check_node(node)?, &declared);
    if kind == "unknown" {
        cands = vec![node.clone()];
    }
    for cand in cands {
        if let Some(hit) = v_hint_approval_for(&decl, cell, Some(&cand)) {
            let a = &hit["approval"];
            return Ok(Some(jd! {"approved_by" => a["approved_by"].clone(), "approved_utc" => a["approved_utc"].clone(),
                                "source" => "campaign:hint_targets", "hitl_source" => a["source"].clone(),
                                "node_id" => hit["node_id"].clone(), "index" => hit["index"].clone()}));
        }
    }
    Ok(None)
}

/// publish 위상 진행표 한 줄 — 소유자 `campaign_init --phase-set publish` CLI(유일한 writer). `runner` = 테스트 주입(프로덕션 J::Null).
pub fn phase_set_publish(repo: &Path, campaign_id: &J, cell: &J, node: &J, tag: &J, remote_sha: &J, utc: &J, runner: &J /* callable */) -> HR<()> {
    let _ = runner;
    phase_set_publish_with(repo, campaign_id, cell, node, tag, remote_sha, utc, None)
}

/// [`phase_set_publish`] with a typed runner hook (`None` = the owner launcher `campaign_init.sh`).
#[allow(clippy::too_many_arguments)]
pub fn phase_set_publish_with(
    repo: &Path,
    campaign_id: &J,
    cell: &J,
    node: &J,
    tag: &J,
    remote_sha: &J,
    utc: &J,
    runner: Option<&dyn Fn(&[String]) -> HR<core::Proc>>,
) -> HR<()> {
    let repo = _repo(repo)?;
    let utc = core::require_utc(utc.as_str(), "publish 위상 --utc")?;
    let cid = _check_campaign_id(campaign_id)?;
    let cell_s = _check_cell(cell)?;
    let Some(tag_s) = tag.as_str().filter(|t| t.starts_with(core::HINT_TAG_PREFIX)) else {
        return core::fail("HINT_TAG_NAMESPACE", format!("hint/ 이름공간 밖 태그: {}", tag.repr()), None);
    };
    let Some(sha) = remote_sha.as_str().filter(|s| sha40_re().is_match(s)) else {
        return core::fail("HINT_REMOTE_SHA_INVALID", format!("원격 태그 오브젝트 SHA 가 40자 소문자 16진이 아니다: {}", remote_sha.repr()), None);
    };
    let mut phase_node = _check_node(node)?;
    if phase_node == "cluster" {
        let decl = or_dict(&_read_json_opt(&repo.join("campaigns").join(&cid).join("campaign.yaml"), "campaign.yaml")?);
        _validator(&repo)?;
        let declared = declared_nodes(&decl);
        match declared.iter().find(|n| v_assigned_cells(&decl, Some(n)).contains(&cell_s)) {
            Some(n) => phase_node = ps(n),
            None => {
                return core::fail("HINT_CELL_ASSIGNMENT_ABSENT", format!("cluster 셀 {} 의 배정 노드를 찾지 못했다(campaigns/{cid}).", cell.repr()), None)
            }
        }
    }
    let argv: Vec<String> = [
        "--campaign-id", &cid, "--phase-set", "publish", "--node", &phase_node, "--cell", &cell_s, "--state", "done",
        "--proof-predicate", "원격 태그 오브젝트 SHA = 로컬 봉인 SHA(git ls-remote 대조)", "--proof-ok",
        "--proof-source", &format!("refs/tags/{tag_s}@{sha}"), "--ended-utc", &utc, "--authored-by", "main",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let proc = match runner {
        Some(r) => r(&argv)?,
        None => core::run_owner(&repo, core::REL_CAMPAIGN_INIT, &argv, None, &[], false)?,
    };
    if proc.returncode != 0 {
        return core::fail(
            "HINT_CAMPAIGN_WRITE_FAILED",
            format!("publish 위상 기록 실패(rc={}): {}", proc.returncode, core::tail_chars(proc.stderr.trim(), 800)),
            None,
        );
    }
    Ok(())
}

// ── 발행기 구동(campaign 모드 · 손 JSON 0) ────────────────────────────────────────────────────────────────────
/// 결정론 토픽 `<campaign>__<node>__<cell>`([A-Za-z0-9_.] 정규화) · 재생 모드면 원 토픽.
pub fn topic_for(ev: &J) -> HR<String> {
    if ev["mode"].as_str() == Some("publication-replay") {
        return Ok(ps(&or_dict(&ev["publication"])["topic"]));
    }
    let raw = format!("{}__{}__{}", ps(&ev["campaign_id"]), ps(&ev["node"]), ps(&ev["cell"]));
    Ok(raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '.' { c } else { '_' }).collect())
}

/// `evidence_publisher <sub> --repo-root <repo> …` through the owner's native launcher (absent = fail closed).
pub fn _publisher(repo: &Path, sub: &str, args: &[String], accept: &[i64]) -> HR<(i64, J)> {
    let mut argv = vec![sub.to_string(), "--repo-root".to_string(), core::posix(repo)];
    argv.extend(args.iter().cloned());
    let proc = core::run_owner(repo, core::REL_EVIDENCE_PUBLISHER, &argv, None, &[], false)?;
    let doc = if proc.stdout.trim().is_empty() {
        J::dict()
    } else {
        match crate::value::loads(&proc.stdout) {
            Ok(d) => d,
            Err(_) => {
                return core::fail(
                    "HINT_PUBLISHER_OUTPUT_UNREADABLE",
                    format!("evidence_publisher {sub} 출력이 JSON 이 아니다(rc={}): {}{}", proc.returncode, core::tail_chars(&proc.stdout, 600), core::tail_chars(&proc.stderr, 600)),
                    None,
                )
            }
        }
    };
    let rc = proc.returncode as i64;
    if !accept.contains(&rc) {
        let d = or_dict(&doc);
        return core::fail_exit(
            "HINT_PUBLISHER_FAILED",
            format!("evidence_publisher {sub} rc={rc}: {} {}", ps(&d["reason_codes"]), core::head_chars(&crate::value::dumps_unicode(&d["messages"]), 800)),
            Some("사유코드를 해소한다 — 발행기의 판정을 우회하지 않는다."),
            if rc != 0 { rc as i32 } else { 1 },
        );
    }
    Ok((rc, doc))
}

/// plan = 실재하는 plan_ref 또는 셀 plan 포인터; 서사는 종류마다 하나.
pub fn _narratives(ev: &J) -> HR<J> {
    let mut out = Map::new();
    let plan = or_dict(&ev["declaration"])["plan_ref"].clone();
    let erepo = PathBuf::from(ps(&ev["repo"]));
    for kind in ["plan", "devlog", "testlog"] {
        let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for p in ev["pointers"].items() {
            if p["kind"].as_str() == Some(kind) {
                if let Some(s) = p["path"].as_str() {
                    set.insert(s.to_string());
                }
            }
        }
        let mut cands: Vec<String> = set.into_iter().collect();
        if kind == "plan" {
            if let Some(pl) = plan.as_str().filter(|s| !s.is_empty()) {
                if !cands.iter().any(|c| c == pl) {
                    cands.insert(0, pl.to_string());
                }
            }
        }
        let cands: Vec<String> = cands.into_iter().filter(|c| erepo.join(c).is_file()).collect();
        if cands.is_empty() {
            return core::fail(
                "HINT_NARRATIVE_EVIDENCE_ABSENT",
                format!("셀 {} 의 {kind} 증거가 없다.", ev["cell"].repr()),
                // plan_26111708 B L8: the remedy names the live `campaign_init.sh` launcher.
                Some(&format!(
                    "셀 {kind} 문서를 먼저 발행하고 `campaign_init.sh --evidence-add --kind {kind} --path <docs/…> --cell {} --node <n> --campaign-id {}` 로 등록한다. plan_ref가 원격 계획이면 선언을 유지하고 기존 위임 범위의 로컬 실행계획을 등록한다.",
                    ps(&ev["cell"]),
                    ps(&ev["campaign_id"])
                )),
            );
        }
        if cands.len() > 1 {
            return core::fail(
                "HINT_NARRATIVE_EVIDENCE_AMBIGUOUS",
                format!("셀 {} 의 {kind} 후보가 {}건이다: {}", ev["cell"].repr(), cands.len(), repr_strs(&cands)),
                Some("이 셀의 판정·서사 문서 하나만 가리키도록 포인터를 정리한다 — 발행기가 고르지 않는다."),
            );
        }
        out.insert(kind, cands[0].clone());
    }
    Ok(J::Dict(out))
}

/// runtime.identity 의 관측(스윕 meta → lite 리포트 측정 환경 표) — identity 를 복사하지 않는다.
pub fn _runtime_identity(repo: &Path, ev: &J) -> HR<(J, String)> {
    let meta = sweep_meta(ev);
    let (obs, src) = if meta.truthy() {
        let sw = or_dict(&ev["sweep"]);
        (_identity_from_meta(&meta)?, format!("sweep_index.meta({} · {})", ps(&sw["dir"]), ps(&sw["binding"])))
    } else {
        let snap = _lite_snapshot(repo, ev["bench_report_path"].as_str())?;
        if !snap.truthy() {
            return core::fail(
                "HINT_RUNTIME_IDENTITY_UNOBSERVED",
                "runtime identity 를 관측할 곳이 없다(스윕 meta·lite 측정 환경 표 모두 부재).",
                Some("identity 를 복사하지 않는다 — 측정 산출물이 있어야 한다."),
            );
        }
        let mut obs = _identity_from_snapshot(repo, ev, &snap)?;
        let tp_src = obs.as_dict_mut().and_then(|m| m.remove("_tp_source")).unwrap_or(J::Null);
        let src = format!("bench_report 측정 환경 표({})", py_name(&ps(&ev["bench_report_path"])))
            + &if tp_src.truthy() { format!(" · tp={}", ps(&tp_src)) } else { String::new() };
        (obs, src)
    };
    let need: Vec<String> = ["model", "gpu", "vllm", "topology", "tp"]
        .iter()
        .filter(|k| obs[**k].is_null() || obs[**k].as_str() == Some(""))
        .map(|k| k.to_string())
        .collect();
    if !need.is_empty() {
        return core::fail(
            "HINT_RUNTIME_IDENTITY_UNOBSERVED",
            format!("runtime identity 키 {} 가 관측되지 않았다({src}).", repr_strs(&need)),
            Some("identity 를 복사하지 않는다 — 측정 meta 에 그 키가 있어야 한다."),
        );
    }
    Ok((obs, src))
}

/// 비배포 산문 PII 스캔(nondeploy_prose) — 파일 · 디렉터리.
pub fn _pii_scan(repo: &Path, rels: &J, terms: &[String]) -> HR<Vec<String>> {
    let mut bad = Vec::new();
    let mut items: Vec<(String, String)> = or_dict(rels).as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), ps(v))).collect()).unwrap_or_default();
    items.sort();
    for (_kind, rel) in items {
        let p = repo.join(&rel);
        if p.is_file() {
            let hits = crate::pii::scan_text(&read_lossy_hr(&p)?, Some(terms), "nondeploy_prose", &[], false, true, None)?;
            bad.extend(hits.iter().map(|h| format!("{rel}:{} {}", h.line, h.pattern)));
        } else if p.is_dir() {
            let hits = crate::pii::scan_tree(&p, Some(terms), "nondeploy_prose", &[], true, true, None, None)?;
            bad.extend(hits.iter().map(|(r, h)| format!("{rel}/{r}:{} {}", h.line, h.pattern)));
        }
    }
    Ok(bad)
}

/// append-raw simlog 의 멱등 재개(같은 바이트 = 건너뜀 · 다른 바이트 = 죽음).
pub fn _append_simlog(repo: &Path, topic: &str, record: &J, src: &str, dest: &str, utc: &str) -> HR<()> {
    let sim = &or_dict(&record["scaffolded"])["simlog"];
    if let Some(sim) = sim.as_str() {
        let dp = repo.join(sim).join(dest);
        if dp.is_file() {
            let a = std::fs::read(&dp).map_err(|e| core::io_err(&e, &dp))?;
            let sp = repo.join(src);
            let b = std::fs::read(&sp).map_err(|e| core::io_err(&e, &sp))?;
            if a == b {
                return Ok(());
            }
            return core::fail("HINT_SIMLOG_DEST_CONFLICT", format!("{sim}/{dest} 가 이미 다른 내용으로 있다(append-only)."), None);
        }
    }
    let args: Vec<String> = ["--topic", topic, "--kind", "simlog", "--src", src, "--recorded-utc", utc, "--dest-name", dest].iter().map(|s| s.to_string()).collect();
    _publisher(repo, "append-raw", &args, &[0])?;
    Ok(())
}

pub fn _record(repo: &Path, topic: &str) -> HR<J> {
    let rec = _read_json_opt(&repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.json")), "발행 기록")?;
    if !rec.is_dict() {
        return core::fail("HINT_PUBLICATION_ABSENT", format!("발행 기록 부재: {}/{topic}.json", core::REL_EVIDENCE_DIR), None);
    }
    Ok(rec)
}

fn sv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// evidence_publisher 를 구동해 work-manifest 를 만든다(campaign 모드 · 판정은 첫 쓰기 전).
pub fn drive_publisher(repo: &Path, ev: &J, topic: &J, generated_utc: &J, draft_dir: &Path) -> HR<PathBuf> {
    let repo = _repo(repo)?;
    if ev["mode"].as_str() != Some("campaign") {
        return core::fail(
            "HINT_REPLAY_READ_ONLY",
            "발행 기록 재생 모드는 읽기만 한다 — 기존 발행 기록·manifest 를 쓴다.",
            Some("`ev.publication['manifest_path']` 를 그대로 쓴다."),
        );
    }
    let gu = core::require_utc(generated_utc.as_str(), "--generated-utc")?;
    let topic = ps(topic);
    let inputs = draft_dir.join("inputs");
    let cell = ps(&ev["cell"]);
    // ── 판정(부수효과 0) ──
    let qual = qualification(ev)?;
    let narr = _narratives(ev)?;
    let ident = identity(ev)?;
    let measured6 = _measured_identity(&repo, ev)?.0;
    let mut six = Map::new();
    for k in ["model", "gpu", "vllm", "quant", "topology", "tp"] {
        six.insert(k, if k == "quant" { measured6[k].clone() } else { ident[k].clone() });
    }
    let six = J::Dict(six);
    let (rt_ident, rt_src) = _runtime_identity(&repo, ev)?;
    let mc = measurement_config(&repo, ev)?;
    let Some(report) = ev["bench_report_path"].as_str().filter(|s| !s.is_empty()).map(str::to_string) else {
        return core::fail(
            "HINT_BENCH_REPORT_ABSENT",
            format!("셀 {} 에 바인딩할 bench_report 가 없다.", ev["cell"].repr()),
            Some("벤치 스킬이 docs/benchmark/ 에 발행한 리포트를 캠페인 포인터로 등록한다(lite 는 `lite_bench.sh --publish-report`)."),
        );
    };
    let sw = or_dict(&ev["sweep"]);
    let downgraded = mc["bench_mode_kind"].as_str() == Some("downgraded-lite");
    let (task, mode, verdict): (&str, &str, Option<String>);
    if ev["report_kind"].as_str() == Some("lite") && !downgraded {
        task = "hint_map_only";
        mode = "lite";
        verdict = None;
    } else {
        let v = if sw["binding"].as_str() == Some("output") { or_dict(&sw["verdict"])["verdict"].clone() } else { J::Null };
        let mut vv = if v.truthy() {
            v
        } else if cert_is_lite(&ev["certificate"])? {
            J::Null
        } else {
            or_dict(&ev["certificate"])["verdict"].clone()
        };
        if vv.as_str() == Some(_JUDGE_NO_BASELINE_VERDICT) {
            vv = J::from(OBSERVATION_ONLY_VERDICT);
        }
        if !in_strs(&vv, &["PASS", "FAIL", "REFUTE", OBSERVATION_ONLY_VERDICT]) {
            return core::fail("HINT_VERDICT_UNOBSERVED", format!("셀 {} 의 벤치 판정을 관측하지 못했다(verdict.json·인증서): {}", ev["cell"].repr(), vv.repr()), None);
        }
        task = "full_benchmark";
        mode = "full";
        verdict = vv.as_str().map(str::to_string);
    }
    let sweep_dir = sw["dir"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let mut sim_srcs: Vec<(String, String)> = Vec::new();
    if task == "full_benchmark" && !downgraded {
        let Some(sd) = sweep_dir.clone() else {
            return core::fail("HINT_SIMLOG_SOURCE_ABSENT", "full 발행은 simlog(스윕 원시)가 필요한데 조인된 스윕이 없다.", None);
        };
        let vp = _as_int(&or_dict(&sw["index"])["verdict_point_level"])?.filter(|n| *n != 0).unwrap_or(1);
        let idx_src = format!("{sd}/sweep_index.json");
        let mut meas = format!("{sd}/level_{vp:02}/measured.json");
        if !repo.join(&meas).is_file() {
            meas = format!("{sd}/level_{vp:02}_measured.json");
        }
        for (s, d) in [(idx_src, "sweep_index.json".to_string()), (meas, format!("level_{vp:02}_measured.json"))] {
            if !repo.join(&s).is_file() {
                return core::fail("HINT_SIMLOG_SOURCE_ABSENT", format!("simlog 원천 부재: {s}"), None);
            }
            sim_srcs.push((s, d));
        }
    }
    let vj_ok = sweep_dir.as_ref().map(|sd| sw["binding"].as_str() == Some("output") && repo.join(sd).join("verdict.json").is_file()).unwrap_or(false);
    if task == "full_benchmark" && !downgraded && !vj_ok {
        let why = match &sweep_dir {
            None => "조인된 스윕이 없다".to_string(),
            Some(_) if sw["binding"].as_str() != Some("output") => format!("스윕이 {} 로만 묶였다(출력 평면 원시가 이 측정의 것이 아니다)", ps(&sw["binding"])),
            Some(sd) => format!("{sd}/verdict.json 부재"),
        };
        return core::fail(
            "HINT_VERDICT_JSON_ABSENT",
            format!("full_benchmark 발행에 판정 산출물(verdict.json)이 필요한데 넘길 수 없다 — {why}."),
            Some("verdict_rule 산출물이 있는 측정(스윕 generated_utc = 측정 키)으로 발행한다. 없으면 루브릭 권한이 게이트에 닿지 않아 인증서 요구가 되살아난다(인증서를 손으로 만들지 않는다)."),
        );
    }
    let terms = crate::pii::require_terms(&repo)?;
    let mut pre = narr.clone();
    pre.set("bench_report", report.clone());
    let cert_path = ev["certificate_path"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let lite_cert = cert_is_lite(&ev["certificate"])? && cert_path.is_some();
    if let Some(cp) = &cert_path {
        if verdict.as_deref() == Some("PASS") || lite_cert {
            pre.set("certificate", cp.clone());
        }
    }
    let hits = _pii_scan(&repo, &pre, &terms)?;
    if !hits.is_empty() {
        return core::fail(
            "HINT_PII_SCAN_FAILED",
            format!("비배포 증거에 PII 매치(nondeploy_prose): {}", repr_strs(&hits[..hits.len().min(10)])),
            Some("원문 문서를 고친다(서사 평면은 사람이 저작한다) — 발행기가 가리지 않는다."),
        );
    }
    // ── 쓰기 ──
    std::fs::create_dir_all(&inputs).map_err(|e| core::io_err(&e, &inputs))?;
    core::write_json(&inputs.join("identity.json"), &six)?;
    let ident_path = core::posix(&inputs.join("identity.json"));
    let mut init = sv(&["--topic", &topic, "--task-class", task, "--generated-utc", &gu, "--identity-json", &ident_path, "--benchmark-mode", mode]);
    if let Some(v) = &verdict {
        init.extend(sv(&["--benchmark-verdict", v]));
    }
    let rec = _read_json_opt(&repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.json")), "발행 기록")?;
    if !(downgraded && rec.is_dict() && rec["task_class"].as_str() == Some("hint_map_only")) {
        _publisher(&repo, "init", &init, &[0])?;
    }
    for kind in ["plan", "devlog", "testlog"] {
        _publisher(
            &repo,
            "set-narrative",
            &sv(&["--topic", &topic, "--kind", kind, "--narrative-file", &ps(&narr[kind]), "--author", "hint.py(evidence.drive_publisher)", "--generated-utc", &gu]),
            &[0],
        )?;
    }
    if task == "full_benchmark" {
        for (s, d) in &sim_srcs {
            let r = _record(&repo, &topic)?;
            _append_simlog(&repo, &topic, &r, s, d, &gu)?;
        }
        let vs = verdict.clone().unwrap_or_default();
        let mut argv = sv(&["--topic", &topic, "--verdict", &vs, "--generated-utc", &gu, "--bench-report-src", &report]);
        if let Some(cp) = &cert_path {
            if verdict.as_deref() == Some("PASS") && !lite_cert {
                argv.extend(sv(&["--certificate-src", cp]));
            }
        }
        if vj_ok {
            argv.extend(sv(&["--verdict-json-src", &format!("{}/verdict.json", sweep_dir.clone().unwrap_or_default())]));
        }
        if !(downgraded && _record(&repo, &topic)?["task_class"].as_str() == Some("hint_map_only")) {
            _publisher(&repo, "publish-benchmark", &argv, &[0])?;
        }
        if downgraded {
            let reason = &mc["downgrade_reason"];
            if !reason.truthy() {
                return core::fail("HINT_DOWNGRADE_REASON_ABSENT", "강등 셀인데 측정 구성 표에 downgrade_reason 이 없다.", None);
            }
            _publisher(
                &repo,
                "init",
                &sv(&["--topic", &topic, "--task-class", "hint_map_only", "--generated-utc", &gu, "--identity-json", &ident_path, "--benchmark-mode", "lite",
                      "--benchmark-verdict", &vs, "--downgrade-from", "full_benchmark", "--downgrade-reason", &ps(reason)]),
                &[0],
            )?;
            if lite_cert {
                _publisher(
                    &repo,
                    "publish-lite-report",
                    &sv(&["--topic", &topic, "--generated-utc", &gu, "--bench-report-src", &report, "--certificate-src", cert_path.as_deref().unwrap_or("")]),
                    &[0],
                )?;
            }
        }
    } else {
        let mut a = sv(&["--topic", &topic, "--generated-utc", &gu, "--bench-report-src", &report]);
        if lite_cert {
            a.extend(sv(&["--certificate-src", cert_path.as_deref().unwrap_or("")]));
        }
        _publisher(&repo, "publish-lite-report", &a, &[0])?;
    }
    let _ = cell;
    let record = _record(&repo, &topic)?;
    _write_pii_and_runtime(&repo, &record, &inputs, &terms, &qual, &rt_ident, &rt_src)?;
    _finalize(&repo, &topic, &inputs)
}

/// `os.path.relpath(path, start)` for absolute normalized paths.
fn relpath(path: &Path, start: &Path) -> String {
    let p = core::resolve(path);
    let s = core::resolve(start);
    let pc: Vec<_> = p.components().collect();
    let sc: Vec<_> = s.components().collect();
    let mut i = 0;
    while i < pc.len() && i < sc.len() && pc[i] == sc[i] {
        i += 1;
    }
    let mut parts: Vec<String> = (i..sc.len()).map(|_| "..".to_string()).collect();
    parts.extend(pc[i..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    if parts.is_empty() { ".".to_string() } else { parts.join("/") }
}

/// pii.json · runtime.json(+출처 사이드카) — 바인딩된 실제 경로를 finalize 가 쓸 문자열 그대로.
pub fn _write_pii_and_runtime(repo: &Path, record: &J, inputs: &Path, terms: &[String], qual: &J, rt_ident: &J, rt_src: &str) -> HR<()> {
    let mdir = repo.join(core::REL_EVIDENCE_DIR);
    let scaff = or_dict(&record["scaffolded"]);
    let mut keys: Vec<String> = record["required_evidence"].items().iter().chain(record["or_group_scaffolded"].items().iter()).map(ps).collect();
    keys.sort();
    keys.dedup();
    let mut scan = Map::new();
    let mut exempt: Vec<J> = Vec::new();
    cgate(repo)?;
    let reason = CGate::PII_MACHINE_RAW_EXEMPTION_REASON;
    for k in keys {
        let Some(rel) = scaff[k.as_str()].as_str().filter(|s| !s.is_empty()) else { continue };
        let mrel = relpath(&repo.join(rel), &mdir);
        if k == "simlog" {
            exempt.push(jd! {"path" => mrel, "reason" => reason});
        } else {
            scan.insert(mrel, rel);
        }
    }
    let hits = _pii_scan(repo, &J::Dict(scan.clone()), terms)?;
    if !hits.is_empty() {
        return core::fail("HINT_PII_SCAN_FAILED", format!("바인딩된 비배포 증거에 PII 매치(nondeploy_prose): {}", repr_strs(&hits[..hits.len().min(10)])), None);
    }
    let scanned: Vec<String> = crate::value::sorted_strs(scan.keys().map(str::to_string));
    core::write_json(&inputs.join("pii.json"), &jd! {"passed" => true, "scanned_paths" => scanned, "exempt_paths" => J::List(exempt)})?;
    core::write_json(
        &inputs.join("runtime.json"),
        &jd! {"health_ok" => qual["health_200"].truthy(), "functional_smoke_passed" => qual["inference_observed"].truthy(), "identity" => rt_ident.clone()},
    )?;
    core::write_json(
        &inputs.join("runtime.provenance.json"),
        &jd! {"qualification" => qual.clone(), "identity_source" => rt_src,
              "containers" => "미기재 — post_health 는 컨테이너 이름·restart_count 를 남기지 않는다(스키마 필수 칸 · 합성 ✗)"},
    )
}

pub fn _finalize(repo: &Path, topic: &str, inputs: &Path) -> HR<PathBuf> {
    for f in ["pii.json", "runtime.json"] {
        if !inputs.join(f).is_file() {
            return core::fail(
                "HINT_PUBLISHER_INPUTS_ABSENT",
                format!("도구가 쓴 발행기 입력 부재: {}", core::posix(&inputs.join(f))),
                Some("`hint.py publish` 가 draft 의 inputs/ 를 만든다 — 손으로 쓰지 않는다."),
            );
        }
    }
    _publisher(
        repo,
        "finalize",
        &sv(&["--topic", topic, "--pii-scan-json", &core::posix(&inputs.join("pii.json")), "--runtime-json", &core::posix(&inputs.join("runtime.json"))]),
        &[0, 1],
    )?;
    let man = repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.work-manifest.json"));
    if !man.is_file() {
        return core::fail("HINT_PUBLISHER_FAILED", format!("finalize 뒤 work-manifest 부재: {}", _rel(repo, &man)?), None);
    }
    Ok(man)
}

/// X6 — 승격 목표를 발행 기록에 적고 finalize 를 다시 돌려 manifest 로 방출한다.
pub fn set_promotion_target(repo: &Path, topic: &J, tag: &J, topology: &J, anchor: &J, generated_utc: &J, draft_dir: Option<&Path>) -> HR<PathBuf> {
    let repo = _repo(repo)?;
    let gu = core::require_utc(generated_utc.as_str(), "--generated-utc")?;
    let Some(draft_dir) = draft_dir else {
        return core::fail(
            "HINT_PUBLISHER_INPUTS_ABSENT",
            "finalize 재실행에 draft 의 inputs/ 가 필요하다(draft_dir 미지정).",
            Some("hint.py continue 가 state.json 의 draft 경로를 넘긴다."),
        );
    };
    let (topic, tag, topology, anchor) = (ps(topic), ps(tag), ps(topology), ps(anchor));
    _publisher(
        &repo,
        "set-promotion-target",
        &sv(&["--topic", &topic, "--tag", &tag, "--topology", &topology, "--anchor", &anchor, "--generated-utc", &gu]),
        &[0],
    )?;
    let man = _finalize(&repo, &topic, &draft_dir.join("inputs"))?;
    let doc = or_dict(&_read_json_opt(&man, "work-manifest")?);
    let want = jd! {"kind" => "hint", "tag" => tag, "topology" => topology, "anchor" => anchor};
    if !py_eq(&doc["promotion_target"], &want) {
        return core::fail(
            "HINT_PROMOTION_TARGET_NOT_EMITTED",
            format!("finalize 가 promotion_target 을 방출하지 않았다: {}", doc["promotion_target"].repr()),
            None,
        );
    }
    Ok(man)
}

/// 캠페인 서사 재바인딩(campaign 모드 · 커밋 전 draft) → {"rebound": [...], "manifest": Path}.
pub fn rebind_campaign_narratives(repo: &Path, ev: &J, topic: &J, generated_utc: &J, draft_dir: &Path) -> HR<J> {
    let repo = _repo(repo)?;
    if ev["mode"].as_str() != Some("campaign") {
        return core::fail(
            "HINT_REPLAY_READ_ONLY",
            "발행 기록 재생 모드는 읽기만 한다 — 서사 재바인딩은 campaign 모드 draft 전용이다.",
            Some("재생 draft 의 발행기 입력은 publisher_inputs 가 쓴다."),
        );
    }
    let gu = core::require_utc(generated_utc.as_str(), "--generated-utc")?;
    let topic = ps(topic);
    let inputs = draft_dir.join("inputs");
    let record = _record(&repo, &topic)?;
    let narr = _narratives(ev)?;
    let qual = qualification(ev)?;
    let (rt_ident, rt_src) = _runtime_identity(&repo, ev)?;
    let bound = or_dict(&record["scaffolded"]);
    let mut rebound: Vec<J> = Vec::new();
    for kind in ["plan", "devlog", "testlog"] {
        let old = &bound[kind];
        let new = &narr[kind];
        if py_eq(old, new) {
            continue;
        }
        let Some(olds) = old.as_str().filter(|s| !s.is_empty()) else {
            return core::fail(
                "HINT_NARRATIVE_BINDING_ABSENT",
                format!("발행 기록 {topic} 에 {kind} 바인딩이 없다 — 재바인딩할 자리가 없다."),
                Some("publish(drive_publisher)가 묶은 발행 기록에서만 재바인딩한다."),
            );
        };
        if repo.join(olds).exists() {
            return core::fail(
                "HINT_NARRATIVE_REBIND_CONFLICT",
                format!("{kind}: 발행 기록은 {} 에 묶였고 캠페인 포인터는 {} 인데 옛 문서가 여전히 실재한다 — 개명이 아니다.", old.repr(), new.repr()),
                Some("어느 문서가 이 셀의 서사인지 사람이 정한다: 개명이면 옛 파일을 없애고(`campaign_init --evidence-relocate`), 아니면 캠페인 포인터를 옛 문서로 되돌린다. 도구는 조용히 교체하지 않는다."),
            );
        }
        rebound.push(jd! {"kind" => kind, "from" => old.clone(), "to" => new.clone()});
    }
    let terms = crate::pii::require_terms(&repo)?;
    let hits = _pii_scan(&repo, &narr, &terms)?;
    if !hits.is_empty() {
        return core::fail(
            "HINT_PII_SCAN_FAILED",
            format!("비배포 증거에 PII 매치(nondeploy_prose): {}", repr_strs(&hits[..hits.len().min(10)])),
            Some("원문 문서를 고친다(서사 평면은 사람이 저작한다) — 발행기가 가리지 않는다."),
        );
    }
    for r in &rebound {
        _publisher(
            &repo,
            "set-narrative",
            &sv(&["--topic", &topic, "--kind", &ps(&r["kind"]), "--narrative-file", &ps(&r["to"]), "--author", "hint.py(evidence.rebind_campaign_narratives)", "--generated-utc", &gu]),
            &[0],
        )?;
    }
    let record = _record(&repo, &topic)?;
    std::fs::create_dir_all(&inputs).map_err(|e| core::io_err(&e, &inputs))?;
    _write_pii_and_runtime(&repo, &record, &inputs, &terms, &qual, &rt_ident, &rt_src)?;
    let man = _finalize(&repo, &topic, &inputs)?;
    Ok(jd! {"rebound" => J::List(rebound), "manifest" => crate::api::enc_path(&man)})
}

/// `output/<topology>/manifest.yaml`(소유 로더) — 부재 = None.
pub fn output_manifest(repo: &Path, topology: &str) -> HR<Option<J>> {
    if !TOPOLOGIES.contains(&topology) {
        return core::fail("HINT_TOPOLOGY_UNDERIVABLE", format!("토폴로지가 single|multi 가 아니다: {}", crate::value::py_repr(topology)), None);
    }
    Ok(_manifest(&_repo(repo)?, topology)?.0)
}

/// 재생 드래프트의 발행기 입력 `<draft>/inputs/{pii,runtime,runtime.provenance}.json`.
pub fn publisher_inputs(repo: &Path, ev: &J, draft_dir: &Path) -> HR<PathBuf> {
    let repo = _repo(repo)?;
    if ev["mode"].as_str() != Some("publication-replay") {
        return core::fail("HINT_PUBLISHER_INPUTS_MODE", "publisher_inputs 는 재생 드래프트 전용이다 — campaign 모드는 drive_publisher 가 쓴다.", None);
    }
    let record = or_dict(&ev["publication"])["record"].clone();
    if !record.is_dict() {
        return core::fail("HINT_PUBLICATION_ABSENT", "재생 발행 기록을 읽지 못했다 — publisher 입력을 관측할 기록이 없다.", None);
    }
    let terms = crate::pii::require_terms(&repo)?;
    let qual = qualification(ev)?;
    let (rt_ident, rt_src) = _runtime_identity(&repo, ev)?;
    let inputs = draft_dir.join("inputs");
    std::fs::create_dir_all(&inputs).map_err(|e| core::io_err(&e, &inputs))?;
    _write_pii_and_runtime(&repo, &record, &inputs, &terms, &qual, &rt_ident, &rt_src)?;
    Ok(inputs)
}

// ── 자체검사 도구 ──────────────────────────────────────────────────────────────────────────────────────────
/// `_OWNER_LINKS` — 자체검사 격리 저장소에 심링크하는 소유 파일(원본 경로).
pub fn _OWNER_LINKS() -> Vec<&'static str> {
    vec![
        core::REL_COMPLETION_GATE,
        core::REL_EVIDENCE_PUBLISHER,
        core::REL_CAMPAIGN_VALIDATOR,
        core::REL_RENDER_BENCH_SECTION,
        REL_MANIFEST_CONTRACT,
        REL_CHECK_SMOKE_MODEL,
        core::REL_SLAVE_FORWARD,
        core::REL_RENDER_DOCKERFILE,
        // the owner hint runs is the launcher (parse_guidellm.sh evaluate-jsonl), not the .py module it replaced.
        REL_PARSE_GUIDELLM_LAUNCHER,
    ]
}
pub const _M_UTC: &str = "2026-01-02T03:04:05Z";
pub const _SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const _SHA_H: &str = "4444444444444444444444444444444444444444";
pub const _SHA0: &str = "0000000000000000000000000000000000000000";
pub const _DIGEST: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

/// `_code(fn, ...)` — 실패 코드(성공 = None). Rust 에서는 호출 결과를 받는다.
pub fn _code<T>(r: HR<T>) -> HR<Option<String>> {
    match r {
        Ok(_) => Ok(None),
        Err(e) if !e.is_py() => Ok(Some(e.code)),
        Err(e) => Err(e),
    }
}

/// 읽기 전용 docker 가짜의 서술자(J) — 실행은 [`_fake_docker_run`].
pub fn _fake_docker(state: &J) -> HR<J> {
    Ok(jd! {"__fake_docker__" => state.clone()})
}

/// 가짜 docker 한 번: inspect/history 만 답하고 argv 를 state['calls'] 에 남긴다.
pub fn _fake_docker_run(state: &mut J, argv: &[String]) -> core::Proc {
    if let Some(m) = state.as_dict_mut() {
        let calls = m.setdefault("calls", J::list());
        calls.push(J::from(argv.to_vec()));
    }
    if argv.len() >= 4 && argv[1] == "image" && argv[2] == "inspect" {
        let img = state["images"][argv[3].as_str()].clone();
        if img.is_null() {
            return core::Proc { returncode: 1, stdout: String::new(), stderr: "no such image".into() };
        }
        return core::Proc { returncode: 0, stdout: crate::value::dumps_default(&jl![img]), stderr: String::new() };
    }
    if argv.get(1).map(String::as_str) == Some("history") {
        let h = state["history"][argv.last().map(String::as_str).unwrap_or("")].clone();
        return core::Proc { returncode: if h.is_null() { 1 } else { 0 }, stdout: ps_or(&h), stderr: String::new() };
    }
    core::Proc { returncode: 1, stdout: String::new(), stderr: "unexpected".into() }
}

fn wr(p: &Path, text: &str) -> HR<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| core::io_err(&e, d))?;
    }
    std::fs::write(p, text).map_err(|e| core::io_err(&e, p))
}
fn mkd(p: &Path) -> HR<()> {
    std::fs::create_dir_all(p).map_err(|e| core::io_err(&e, p))
}
fn jl_lines(rows: &[J]) -> String {
    rows.iter().map(|x| match x { J::Str(s) => s.clone(), o => crate::value::dumps_default(o) } + "\n").collect()
}

/// 격리 저장소 — 소유 파일은 이 체크아웃의 파일에 심링크(+ 그 native launcher 가 있으면 함께) · 데이터는 전부 픽스처.
/// campaign_init 은 launcher 사본(cli 픽스처)으로 둔다. 반환 (repo, {cert, report, plan, devlog, testlog, docker, sweep, decl}).
pub fn _fixture(td: &Path) -> HR<(PathBuf, J)> {
    let code_root = core::skill_dir().parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
    let repo = td.join("repo");
    std::fs::create_dir(&repo).map_err(|e| core::io_err(&e, &repo))?;
    for rel in _OWNER_LINKS() {
        // 의도된 차이: Rust 는 소유 CLI 를 `<stem>.sh` launcher 로 부른다 — launcher 가 소유 진입점이다(plan_26111615 D4 ·
        //   `.py` 는 링크하지도 읽지도 않는다). launcher 가 없으면 소유 모듈 부재다(fail-closed).
        let lrel = core::owner_launcher(Path::new(""), rel);
        let ls = code_root.join(&lrel);
        if !ls.is_file() {
            return core::fail("HINT_OWNER_MODULE_MISSING", format!("자체검사 소유 모듈 부재: {rel}"), None);
        }
        let ld = repo.join(&lrel);
        if !ld.exists() {
            mkd(ld.parent().unwrap())?;
            std::os::unix::fs::symlink(&ls, &ld).map_err(|e| core::io_err(&e, &ld))?;
        }
    }
    // campaign_init 의 자리(디렉터리)만 둔다 — launcher 사본은 cli 픽스처가 둔다(`.py` 사본 ✗ · plan_26111615 D4).
    mkd(repo.join(core::REL_CAMPAIGN_INIT).parent().unwrap())?;
    let vocab = crate::value::loads(r#"{"schema_version": 1, "hw": {"gb10": ["NVIDIA GB10", "GB10"], "h100": ["NVIDIA H100"]}, "quant": {"nvfp4": ["nvfp4", "modelopt-dominant:NVFP4"], "fp8": ["fp8"], "bf16": ["bf16", "none"]}, "kv": {"auto": ["auto"], "fp8": ["fp8"]}, "ple": {"mmap": ["mmap"], "resident": ["resident"], "offload": ["offload"], "none": ["none"]}, "graph": {"graph": ["graph"], "eager": ["eager"]}, "plane": {"docker": "", "native": "bare"}, "quant_suffixes": ["nvfp4", "fp8", "bf16"]}"#).expect("vocab");
    core::write_json(&repo.join(core::REL_VOCAB), &vocab)?;
    wr(&repo.join(core::REL_PII_TERMS), "# fixture\nfixturesecretterm\n")?;
    let out = repo.join("output/multi");
    for d in ["configs", "envs", "benchlog/sweep_c1-a/level_01"] {
        mkd(&out.join(d))?;
    }
    let models = td.join("models");
    let ckpt = models.join("Org/Fixture-Model-NVFP4");
    mkd(&ckpt.join(".git"))?;
    core::write_json(&ckpt.join("config.json"), &crate::value::loads(r#"{"quantization_config": {"quant_method": "modelopt", "quant_algo": "MIXED_PRECISION", "quantized_layers": {"l0": {"quant_algo": "NVFP4"}, "l1": {"quant_algo": "NVFP4"}, "l2": {"quant_algo": "FP8"}}}, "text_config": {"ple_layer_ids": [1], "ple_embed_dim": 8}}"#).expect("cfg"))?;
    wr(&ckpt.join("README.md"), "---\nbase_model:\n- Org/Fixture-Model\nlicense: other\n---\n# card\n")?;
    wr(&ckpt.join(".git/config"), "[remote \"origin\"]\n\turl = https://huggingface.co/Org/Fixture-Model-NVFP4\n")?;
    wr(&ckpt.join(".git/HEAD"), "ref: refs/heads/main\n")?;
    wr(&ckpt.join(".git/refs/heads/main"), &format!("{_SHA_H}\n"))?;
    wr(&ckpt.join(".git/packed-refs"), &format!("# pack-refs with: peeled\n{_SHA_H} refs/remotes/origin/main\n"))?;
    let ev_dir = repo.join("docs/logs/main/events");
    mkd(&ev_dir)?;
    let op = format!("/{}/op-fixture/serve_budget.env", "home");
    let ip = ["10", "1", "2", "3"].join(".") + ":8000";
    let ledger: Vec<J> = vec![
        jd! {"kind" => "budget_clear", "existed" => false, "ts" => "2026-01-02T01:00:00Z"},
        jd! {"kind" => "budget_declare", "label" => "smoke-c1-a", "floor_mib" => 100, "weights_mib" => 50, "ts" => "2026-01-02T01:00:00Z", "decl_path" => op},
        jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-02T01:30:00Z"},
        jd! {"kind" => "budget_declare", "label" => "smoke-c1-a-kv8g", "floor_mib" => 90, "ts" => "2026-01-02T01:40:00Z"},
        jd! {"kind" => "watchdog_trip", "rule" => "absolute", "mem_avail_mib" => 9000, "ts" => "2026-01-02T01:45:00Z"},
        jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-02T01:50:00Z"},
        jd! {"kind" => "serve_start", "session_id" => "serve-c1-a-20260102T020000Z", "model" => "fixture", "ts" => "2026-01-02T02:00:00Z"},
        jd! {"kind" => "serve_start", "session_id" => "serve-c1-ab-20260102T020000Z", "model" => "other", "ts" => "2026-01-02T02:00:00Z"},
        jd! {"kind" => "budget_declare", "label" => "smoke-c1-a", "floor_mib" => 100, "mem_total_mib" => 124608, "ts" => "2026-01-02T02:30:00Z"},
        jd! {"kind" => "budget_renew", "label" => "smoke-c1-a", "ttl_s" => 900, "ts" => "2026-01-02T02:59:00Z"},
        jd! {"kind" => "watchdog_trip_nomatch", "ts" => "2026-01-02T03:00:00Z"},
        J::from("{not json"),
        jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-02T03:10:00Z"},
        jd! {"kind" => "thermal_trip", "soc_temp_c" => 98, "source" => "unit:fixture-thermal", "ts" => "2026-01-02T02:45:00Z"},
        jd! {"kind" => "budget_clear", "label" => "smoke-zz-other", "existed" => true, "ts" => "2026-01-02T02:50:00Z"},
        jd! {"kind" => "watchdog_highrate_hold", "target" => ip, "mem_avail_mib" => 7, "ts" => "2026-01-02T02:52:00Z"},
    ];
    wr(&ev_dir.join("2026-01.jsonl"), &jl_lines(&ledger))?;
    wr(&ev_dir.join("watchdog.jsonl"), &jl_lines(&[
        jd! {"kind" => "budget_honored", "label" => "smoke-c1-a", "arm_ceiling_mib" => 70, "ts" => "2026-01-02T02:30:01Z"},
        jd! {"kind" => "watchdog_highrate_hold", "mem_avail_mib" => 60000, "rate_mib_s" => 25000, "ts" => "2026-01-02T02:31:00Z"},
        jd! {"kind" => "budget_none", "arm_ceiling_mib" => 999999999, "ts" => "2026-01-02T03:10:01Z"},
        jd! {"kind" => "watchdog_highrate_hold", "mem_avail_mib" => 59000, "rate_mib_s" => 26000, "ts" => "2026-01-02T02:31:00Z"},
        jd! {"kind" => "budget_rejected", "source" => "mem_watchdog_eta", "mode" => "armed", "arm_ceiling_mib" => 999999999,
             "reason" => "floor_mib/expires_epoch 파싱 실패", "ts" => "2026-01-02T03:10:00Z"},
    ]))?;
    wr(&ev_dir.join("thermal.jsonl"), &jl_lines(&[
        jd! {"kind" => "thermal_trip", "soc_temp_c" => 99, "ts" => "2026-01-02T01:45:00Z"},
        jd! {"kind" => "thermal_trip", "soc_temp_c" => 98, "ts" => "2026-01-02T02:45:00Z"},
    ]))?;
    wr(&repo.join("docs/logs/sub/events/2026-01.jsonl"), &jl_lines(&[jd! {"kind" => "budget_declare", "label" => "smoke-c1-b", "ts" => "2026-01-02T02:30:00Z"}]))?;
    wr(&out.join("manifest.yaml"), "topology: multi\nself_role: main\ngpus_per_node: 1\ngpu_model: NVIDIA GB10\ncpu_arch: aarch64\nnodes:\n  - role: main\n  - role: sub\n    gpu_model: NVIDIA GB10\n")?;
    wr(&out.join("docker-compose.yaml"), &format!("services:\n  vllm-master-serve:\n    volumes:\n      - {}:/app/quant_models:ro\n", core::posix(&models)))?;
    wr(&out.join("configs/c1-a.yaml"), "# 셀 c1-a\nmodel: /app/quant_models/Org/Fixture-Model-NVFP4\ntensor-parallel-size: 2\nmax-model-len: 4096\nkv-cache-dtype: auto\nkv-cache-memory-bytes: 1073741824\nenforce-eager: true   # 승계\nspeculative-config: '{\"method\":\"mtp\",\"num_speculative_tokens\":2}'\n")?;
    wr(&out.join("Dockerfile.source-build"), "FROM base\nARG VLLM_REPO=x\nARG VLLM_REF=y\nARG SM12X_PORT=0\n")?;
    wr(&out.join("configs/c1-a.sh"), "#!/bin/bash\nvllm serve --config \"/app/configs/${CONFIG_FILE}.yaml\"\n")?;
    wr(&out.join("envs/.env.c1-a"), "CONFIG_FILE=c1-a\nIMAGE_TAG=fixture:0.9.0-source\nBUILD_DOCKERFILE=Dockerfile.source-build\nVLLM_REF=v0.9.0\nVLLM_PLE_MMAP=1\n")?;
    core::write_json(&out.join("resolved.json"), &jd! {"vllm_version" => "0.9.0", "torch" => jd! {"pin" => "2.9.0"},
        "ngc_base" => jd! {"tag" => "26.01-py3", "image" => "nvcr.io/nvidia/pytorch:26.01-py3"},
        "build_track" => jd! {"decision" => "source-build"}, "upstream_delta" => jd! {"to_ref" => "v0.9.0", "to_sha" => _SHA_A}})?;
    let sweep = out.join("benchlog/sweep_c1-a");
    let meta = jd! {"model" => "fixture-model-nvfp4", "measured_node" => "cluster", "config_name" => "c1-a", "gpu_model" => "NVIDIA GB10",
        "vllm_version" => "0.9.0", "quantization" => "NA", "topology" => "multi", "tensor_parallel_size" => "2",
        "image_tag" => "fixture:0.9.0-source", "image_tag_source" => "measured(docker inspect)",
        "image_tag_declared" => "fixture:0.9.0-source", "image_digest" => _DIGEST,
        "image_digest_source" => "measured(docker inspect .Image)", "max_model_len" => "4096", "kv_cache_dtype" => "auto",
        "enforce_eager" => "true", "ple_mode" => "mmap", "ple_mode_source" => "declared(envfile VLLM_PLE_MMAP=1)",
        "gpu_memory_utilization" => "NA", "kv_cache_memory_bytes" => "1073741824",
        "model_path" => "/app/quant_models/Org/Fixture-Model-NVFP4", "max_num_seqs" => "NA"};
    core::write_json(&sweep.join("sweep_index.json"), &jd! {"config" => "c1-a", "topology" => "multi", "generated_utc" => _M_UTC,
        "verdict_point_level" => 1, "meta" => meta, "lite" => jd! {"gen_tps" => 10.0, "raw_json" => core::posix(&sweep.join("x.json"))},
        "levels" => jl![jd! {"level" => 1, "status" => "ok", "measured" => jd! {"decode_tps" => 12.3, "completed" => 4, "failed" => 0,
            "measurement_ok" => true}, "bench_json" => core::posix(&sweep.join("b.json"))}]})?;
    core::write_json(&sweep.join("level_01/measured.json"), &jd! {"decode_tps" => 12.3, "measurement_ok" => true, "completed" => 4})?;
    core::write_json(&sweep.join("level_01/bench_c1-a.json"), &jd! {"completed" => 4, "failed" => 0})?;
    core::write_json(&sweep.join("level_01/post_health_c1-a.json"), &jd! {"provenance" => "measured", "config" => "c1-a",
        "health_http_code" => "200", "container_running" => "true", "container_oom_killed" => "false"})?;
    core::write_json(&sweep.join("verdict.json"), &jd! {"verdict" => "PASS", "rubric" => jd! {"authority" => "weak", "floor" => 8.5,
        "ratio_M_over_primary" => 1.23, "source" => "expected_achievable(roofline×MBU)"}})?;
    core::write_json(&sweep.join("roofline.json"), &jd! {"gpu_model" => "NVIDIA GB10"})?;
    std::fs::write(sweep.join("lite_engine_c1-a.log"), [
        "(EngineCore pid=1) \x1b[36mINFO 01-02 02:31:00 [a.py:1] engine up",
        "(Worker_TP0 pid=2) INFO 01-02 02:32:00 [weight_utils.py:1] Auto-prefetch is disabled because the filesystem (CIFS) is not a recognized network FS",
        "progress 10%\rprogress 100%",
        "(EngineCore pid=1) INFO 01-02 02:40:00 [kv_cache_utils.py:1] GPU KV cache size: 1,000 tokens, Maximum concurrency for 4,096 tokens per request: 2.00x",
        "(APIServer pid=3) INFO 01-02 02:58:00 [entry.py:1] Starting vLLM server on http://0.0.0.0:8080",
        "",
    ].join("\n")).map_err(|e| core::io_err(&e, &sweep))?;
    core::write_json(&out.join("benchlog/attestation_c1-a.json"), &jd! {"config" => "c1-a", "image_tag" => "fixture:0.9.0-source",
        "checks" => jl![jd! {"axis" => "vllm_sha", "node" => "sub", "observed" => _SHA_A}]})?;
    for d in ["plan", "devlog", "testlog", "benchmark", "simlog"] {
        mkd(&repo.join("docs").join(d))?;
    }
    let stem = "26010212_fixture-model-nvfp4_GB10_0.9.0";
    let cert_rel = format!("docs/benchmark/benchmark_{stem}.yaml");
    let rep_rel = format!("docs/benchmark/bench_report_{stem}.md");
    wr(&repo.join(&cert_rel), &format!(
        "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: fixture-model-nvfp4\ngpu_model: \"NVIDIA GB10\"\nvllm_version: 0.9.0\nquantization: N/A\ntopology: multi\ntensor_parallel_size: 2\nimage_tag: \"fixture:0.9.0-source\"\nimage_digest: \"{_DIGEST}\"\nmax_model_len: 4096\nkv_cache_dtype: auto\nenforce_eager: true\nbenchmark_mode: full\ndecode_tps_conc1: 12.3\nrubric_authority: weak\nprimary_source: expected_achievable(roofline×MBU)\nprimary_tps: 10.0\nfloor_tps: 8.5\ntolerance: 0.15\nratio_M_over_primary: 1.23\nspec_on: true\nlite_included: true\nlite_gen_tps_warm: 10.0\nmeasured_utc: \"{_M_UTC}\"\nmeasured_node: cluster\n"))?;
    let rbs_title = crate::render_bench_section::MEASUREMENT_CONFIG_TITLE;
    let rbs_header = crate::render_bench_section::REPORT_HEADER;
    wr(&repo.join(&rep_rel), &[
        "# 성능 보고서", "", &format!("> 생성일 {_M_UTC}."), "", rbs_title, "", "| 키 | 값 |", "|---|---|",
        "| bench_mode | full |", "| bench_mode_kind | full |", "| repeats | 3 |", "",
        "## lite 지표 (full ⊇ lite)", "", "| 메트릭 | Main | Sub |", "|---|---|---|",
        "| gen tokens/sec (warm) [master] | 10.00 t/s | — |", "",
        "## 부하 스윕 곡선", "", rbs_header, "|---|---|---|---|---|---|---|",
        "| 1 ★판정점 | 12.3 | 12 | 60 | 100 | 20 | 4/0 |", "| 2 | 20.1 | 20 | 90 | 120 | 25 | 4/0 |", "",
        "## 측정 환경 스냅샷", "", "| 키 | 값 |", "|---|---|", "| model | fixture-model-nvfp4 |", "",
    ].join("\n"))?;
    let (plan, devlog, testlog) = ("docs/plan/plan_26010200_fixture.md", "docs/devlog/devlog_26010213_fixture.md", "docs/testlog/testlog_26010213_fixture.md");
    for (p, body) in [(plan, "# plan\n픽스처 계획.\n"), (devlog, "# devlog\n픽스처 서사.\n"), (testlog, "# testlog\n픽스처 판정.\n")] {
        wr(&repo.join(p), body)?;
    }
    let camp = repo.join("campaigns/c1");
    mkd(&camp.join("cells/c1-a"))?;
    let decl = jd! {"id" => "c1", "plan_ref" => plan,
        "nodes" => jl![jd! {"node_id" => "main", "role" => "main", "topology" => "multi"}, jd! {"node_id" => "sub", "role" => "sub", "topology" => "multi"}],
        "assignments" => jd! {"main" => jl![jd! {"cell" => "c1-a", "mode" => "AUTO"}], "sub" => jl![jd! {"cell" => "c1-b", "mode" => "AUTO"}]},
        "control_variables" => jd! {"topology" => "multi", "target_gpu" => "NVIDIA GB10"},
        "hint_targets" => jl![jd! {"node_id" => "main", "cells" => jl!["c1-a"],
            "approval" => jd! {"approved_by" => "사용자 발화 전사", "approved_utc" => "2026-01-02T00:00:00Z", "source" => "declaration-popup"}}]};
    core::write_json(&camp.join("campaign.yaml"), &decl)?;
    core::write_json(&camp.join("cells/c1-a/cell.status.json"), &jd! {"cell_id" => "c1-a", "cell_outcome" => "measured"})?;
    core::write_json(&camp.join("cells/c1-a/lockset.json"), &jd! {"id" => "c1-a", "provenance" => "hand-authored"})?;
    wr(&camp.join("cells/c1-a/config.yaml"), "declared_axes:\n  ple_mode: mmap\ntarget_gpu:\n  gpu_model: NVIDIA GB10\n")?;
    core::write_json(&camp.join("evidence_pointers.json"), &jd! {"campaign_id" => "c1", "pointers" => jl![
        jd! {"kind" => "certificate", "path" => cert_rel.clone(), "cell_id" => "c1-a", "node_id" => "cluster"},
        jd! {"kind" => "bench_report", "path" => rep_rel.clone(), "cell_id" => "c1-a", "node_id" => "cluster"},
        jd! {"kind" => "devlog", "path" => devlog, "cell_id" => "c1-a", "node_id" => "main"},
        jd! {"kind" => "testlog", "path" => testlog, "cell_id" => "c1-a", "node_id" => "main"},
        jd! {"kind" => "testlog", "path" => "docs/testlog/testlog_26010214_other.md", "cell_id" => "c1-b", "node_id" => "sub"}]})?;
    core::write_json(&camp.join("phases/main/serve.status.json"), &jd! {"phase" => "serve", "state" => "done", "node_id" => "main",
        "cell_id" => "c1-a", "started_utc" => "2026-01-02T01:00:00Z", "ended_utc" => "2026-01-02T01:30:00Z"})?;
    wr(&camp.join("journey.jsonl"), &(crate::value::dumps_unicode(&jd! {"cell_id" => "c1-a", "next_intent" => "testlog_26010213_fixture 참조"}) + "\n"))?;
    mkd(&repo.join("campaigns/decoy"))?;
    core::write_json(&repo.join("campaigns/decoy/campaign.yaml"), &jd! {"id" => "decoy"})?;
    let active = repo.join("campaigns/ACTIVE");
    wr(&active, "decoy\n")?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&active, std::fs::Permissions::from_mode(0)).map_err(|e| core::io_err(&e, &active))?;
    }
    let mut images = Map::new();
    images.insert(_DIGEST, jd! {"Id" => _DIGEST, "Created" => "2026-01-01T00:00:00Z", "Architecture" => "arm64",
        "Config" => jd! {"Env" => jl!["PYTORCH_VERSION=2.9.0a0", "CUDA_VERSION=13.0", "NVIDIA_PYTORCH_VERSION=26.01", "PATH=/usr/bin"],
                         "Labels" => jd! {"com.docker.compose.project" => "vllm_c1-build_project"}}});
    let mut history = Map::new();
    history.insert(_DIGEST, "RUN |3 VLLM_REPO=https://github.com/vllm-project/vllm.git VLLM_REF=v0.9.0 SM12X_PORT=0 /bin/sh -c build\nCOPY x y\nRUN |2 BASE_INTERNAL_URL=http://base-internal/x CUDA_VERSION=130 /bin/sh -c base\n");
    let dstate = jd! {"images" => J::Dict(images), "history" => J::Dict(history)};
    Ok((repo, jd! {"cert" => cert_rel, "report" => rep_rel, "plan" => plan, "devlog" => devlog, "testlog" => testlog,
                   "docker" => dstate, "sweep" => crate::api::enc_path(&sweep), "decl" => decl}))
}

/// 트리 → {상대 posix 경로: 바이트(`{"__bytes__": hex}`)} — 심링크 · ACTIVE 제외.
pub fn _tree(root: &Path) -> HR<J> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                out.push(p.clone());
                if p.is_dir() && !p.is_symlink() {
                    walk(&p, out);
                }
            }
        }
    }
    let mut all = Vec::new();
    walk(root, &mut all);
    all.sort();
    let mut m = Map::new();
    for p in all {
        if p.is_file() && !p.is_symlink() && fname(&p) != "ACTIVE" {
            let b = std::fs::read(&p).map_err(|e| core::io_err(&e, &p))?;
            let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
            m.insert(core::posix(p.strip_prefix(root).unwrap_or(&p)), jd! {"__bytes__" => hex});
        }
    }
    Ok(J::Dict(m))
}
