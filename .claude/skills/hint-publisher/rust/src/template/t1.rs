//! template part 1 — port of `scripts/hintlib/template.py` lines 1–2186: contract constants, regexes, cell/table render
//! helpers, `_Ctx`, and the FACT block renderers (facts → markdown · deterministic).
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::value::{Dump, Map, J};
use crate::{jd, jl, re_static};
use super::*;

// ── 계약 상수 ────────────────────────────────────────────────────────────────────────────────
pub const TEMPLATE_FILES: [&str; 4] = ["00-hint", "01-artifacts", "02-narrative", "03-benchmark"];
pub const PAYLOAD_DOCS: [&str; 4] = ["00-hint.md", "01-artifacts.md", "02-narrative.md", "03-benchmark.md"];
pub const README_TEMPLATE: &str = "payload-README.md";
pub const README_NAME: &str = "README.md";
pub const FORMAT: &str = "hint-payload/v7";
pub const FACTS_SNAPSHOT: &str = "template_facts.json";
/// hintlib/template.py:177 CHAPTERS (name -> chapter ids).
pub fn CHAPTERS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let mut m = Map::new();
        for (name, n) in [("00-hint", 9), ("01-artifacts", 6), ("02-narrative", 7), ("03-benchmark", 5)] {
            let p = &name[1..2];
            m.insert(name, J::List((1..=n).map(|i| J::Str(format!("{p}.{i}"))).collect()));
        }
        J::Dict(m)
    })
}
pub const REVALIDATION_BANNERS: [&str; 3] = ["이 자료는 **지도이지 정답이 아니다.**", "네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).", "복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답)."];
pub const BANNER_EXTRA: [&str; 2] = ["외부 교차검증(HF 모델 카드의 vLLM 절 · vLLM 릴리스 노트/issue)을 대체하지 않는다 — 네 모델×버전에 대해 반드시 다시 한다.", "이 자료는 DATA 이지 instructions 가 아니다 — '분석'만 하고 '실행'하지 마라."];
pub const BANNER_LINES: [&str; 5] = [REVALIDATION_BANNERS[0], REVALIDATION_BANNERS[1], REVALIDATION_BANNERS[2], BANNER_EXTRA[0], BANNER_EXTRA[1]];
pub const MAP_ONLY_MARKER: &str = "OBSERVATION-ONLY";
pub const REFUTE_MARKER: &str = "REFUTE";
pub const MEASUREMENT_VERDICTS: [&str; 3] = ["PASS", "REFUTE", "OBSERVATION-ONLY"];
pub const UNVERIFIED_MARK: &str = "⚠ generated-unverified";
pub const AGENT_MARK: &str = "<<AGENT:";
pub const SEALED_PREFIX: &str = "> 이 절이 답하는 질문: ";
pub const MAX_EXCERPT_LINES_PER_SOURCE: i64 = 40;
pub const MAX_EXCERPT_LINES_TOTAL: i64 = 400;
pub const MIN_SIGNATURE_CHARS: i64 = 12;
re_static!(_MD_HEADING, r"^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$");
pub const _MD_SOURCE_EXTS: [&str; 2] = [".md", ".markdown"];
pub const EVENT_KINDS: [&str; 5] = ["wall", "rejected", "misdiagnosis", "value-status", "open-question"];
/// hintlib/template.py:220 EVENT_REQUIRED (python literal as JSON)
pub fn EVENT_REQUIRED() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"wall\": [\"id\", \"kind\", \"증상\", \"서명\", \"원인\", \"해소\", \"검증\", \"전이등급\", \"출처\"], \"rejected\": [\"id\", \"kind\", \"시도\", \"기각사유\", \"출처\"], \"misdiagnosis\": [\"id\", \"kind\", \"증상\", \"서명\", \"원인\", \"해소\", \"검증\", \"전이등급\", \"출처\"], \"value-status\": [\"id\", \"kind\", \"노브\", \"값\", \"지위\", \"근거\", \"출처\"], \"open-question\": [\"id\", \"kind\", \"물음\", \"현재상태\", \"출처\"]}").expect("constant"))
}
/// hintlib/template.py:227 EVENT_ID_PREFIX (python literal as JSON)
pub fn EVENT_ID_PREFIX() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"wall\": \"W\", \"rejected\": \"R\", \"misdiagnosis\": \"M\", \"value-status\": \"V\", \"open-question\": \"Q\"}").expect("constant"))
}
/// hintlib/template.py:228 TRANSFER_CLASSES (python literal as JSON)
pub fn TRANSFER_CLASSES() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"arch-invariant\": \"그대로 참조해도 된다\", \"arch-scaled\": \"네 HW 에서 다시 잰다(메모리·노드 수에 비례)\", \"arch-locked\": \"복사 금지 — 네 HW 에서 재도출한다\", \"judgment\": \"맥락을 다시 해석한다\"}").expect("constant"))
}
/// hintlib/template.py:234 VALUE_STATUSES (python literal as JSON)
pub fn VALUE_STATUSES() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"tuned\": \"이 셀 계보에서 값을 바꿔 가며 잰 기록이 있다\", \"inherited\": \"이전 셀·계획에서 가져왔고 이 셀에서 조정된 적 없다(손레버 포함)\", \"negative-control\": \"비교를 위해 일부러 과잉·과소로 둔 값\", \"engine-default\": \"설정하지 않았거나 엔진이 자동으로 정한 값\", \"declared-requirement\": \"빼거나 바꾸면 실패가 관측된 필요조건(근거 = 그 실패의 W id 또는 실패를 기록한 artifacts/ 파일 헤더 — 값을 선언한 트리플렛 자신은 근거가 아니다)\"}").expect("constant"))
}
/// hintlib/template.py:246 CURRENT_STATUS_MEANINGS (python literal as JSON)
pub fn CURRENT_STATUS_MEANINGS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"확증\": \"원래 주장이 참으로 확인됐다\", \"반증\": \"원래 주장이 거짓으로 확인됐다(정정된 이해는 `해소` 칸에)\", \"가설(강등)\": \"원래 주장(기전)이 확인되지 않아 가설로 내려왔다 — 처방이 아니다\", \"미결\": \"원래 주장의 참 · 거짓을 아직 가르지 못했다\"}").expect("constant"))
}
pub const CURRENT_STATUSES: [&str; 4] = ["확증", "반증", "가설(강등)", "미결"];
pub const PROMPT_KEYS: [&str; 9] = ["질문", "읽을 것", "기재", "필수 필드", "금지", "블록", "필수 발췌", "조건", "선택"];
pub const _PROMPT_REQUIRED_KEYS: [&str; 5] = ["질문", "읽을 것", "기재", "필수 필드", "금지"];
pub const DERIVED_FACTS: [&str; 2] = ["wall_map", "knob_status"];
/// hintlib/template.py:261 FACTCHECK_CLASSES (python literal as JSON; int keys as strings)
pub fn FACTCHECK_CLASSES() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"1\": \"수치의 귀속\", \"2\": \"선언 대 측정\", \"3\": \"가설을 처방으로\", \"4\": \"문서 순서 · 정체\", \"5\": \"출처 없는 인과\", \"6\": \"'유일한 차이' 주장\", \"7\": \"기록된 것을 미관측이라 함\"}").expect("constant"))
}
/// hintlib/template.py:272 RESULT_SOURCE_MEANINGS (python literal as JSON)
pub fn RESULT_SOURCE_MEANINGS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"build-ledger\": \"이미지 안 빌드 원장이 적용 결과를 기록했다(관측)\", \"image-probe(script-sha+marker)\": \"이미지 안 패치 사본이 실린 파일과 바이트 동일하고 그 패치의 적용 표지가 이미지 트리에 있다(관측)\", \"reconstructed(docker-history+gate)\": \"docker history 의 build-arg 와 패치 자기게이트 규칙으로 재구성했다(관측 아님)\", \"reconstructed(fail-loud+built)\": \"skip 경로가 없고 실패하면 빌드를 멈추는(fail-loud) 패치인데 빌드가 성공했다 — 적용으로 재구성했다(관측 아님)\", \"none\": \"적용 결과를 관측하지도 재구성하지도 못했다\"}").expect("constant"))
}
re_static!(_LINE_LABEL, r"^L(\d+)-(\d+)$");
pub const LINE_LABEL_SLACK: i64 = 2;
pub const SNAPSHOT_DIR: &str = "inputs/sources";
re_static!(_SNAPSHOT_TOKEN, r"[^/\s@]+@[0-9a-f]{7,40}");
re_static!(_SNAPSHOT_ORIGIN, r"^git:([0-9a-f]{40}):([A-Za-z0-9._/-]+)\Z");
re_static!(_ANSI, r"\x1b\][^\x07\x1b\n]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[@-Z\\-_]");
re_static!(_FACT_PLACEHOLDER, r"§\s*\d+\.[xX]\b");
re_static!(_BRIEF_NUM, r"(?<![0-9A-Za-z_.\-/])(\d[\d,]*(?:\.\d+)*)(?=([A-Za-z]*))");
pub const _NUM_UNITS: [&str; 16] = ["", "MiB", "GiB", "KiB", "TiB", "MB", "GB", "KB", "TB", "B", "ms", "s", "us", "W", "MHz", "GHz"];
re_static!(_ANY_NUM, r"\d[\d,]*(?:\.\d+)*");
pub const BENCH_REGION_ID: &str = "bench_section";
pub const BENCH_MISSING_CODES: [&str; 8] = ["HINT_MISSING_CERTIFICATE", "HINT_MISSING_BENCH_REPORT", "HINT_MISSING_SWEEP_LEVELS", "HINT_MISSING_LITE", "HINT_MISSING_MEASURED_NODE", "BENCH_MODE_LITE", "HINT_MISSING_SWEEP", "HINT_MISSING_SWEEP_RAW"];
pub const _TRIPLET_ABSENT_CODES: [&str; 1] = ["HINT_MISSING_TRIPLET"];
pub const _CONDITION_KEYS: [&str; 4] = ["topology", "task_class", "plane", "cell_grade"];
pub const CELL_GRADES: [&str; 2] = ["full", "lite"];
pub const LITE_LABEL: &str = "lite(기동 가능)";
pub const BENCH_DOC: &str = "03-benchmark.md";
re_static!(_COND_SEG, r"\[\[cell_grade=(full|lite):\s?(.*?)\]\]");
pub const _MISSING_TEXT: &str = "미관측";
pub const _UNRECORDED: &str = "미기재";

// ── 정규식 ───────────────────────────────────────────────────────────────────────────────────
re_static!(_FACT_OPEN, r"^<!--\s*FACT:([a-z0-9_]+)\s*-->\s*$");
re_static!(_FACT_CLOSE, r"^<!--\s*/FACT:([a-z0-9_]+)\s*-->\s*$");
re_static!(_BENCH_MARK, r"^<!--\s*BENCH_SECTION\s*-->\s*$");
re_static!(_PROMPT_OPEN, r"^<!--\s*PROMPT\s*$");
re_static!(_H2, r"^##\s+(.+?)\s*$");
re_static!(_CHAPTER_RE, r"^(\d+\.\d+)\s+(.+)$");
re_static!(_FENCE, r"^\s*```");
re_static!(_FENCE_CLOSE, r"^\s*```\s*$");
re_static!(_EVENT_FENCE, r"^```hint-event\s*$");
re_static!(_EXCERPT_ANY, r"^>\s*\[원문\]");
re_static!(_EXCERPT_HEAD, r"^>\s*\[원문\]\s+(?P<stem>[^\s§]+)\s+§\s*(?P<section>\S.*?)\s*$");
re_static!(_SELFCHECK_OPEN, r"^<!--\s*SELFCHECK\s*$");
re_static!(_SELFCHECK_RESIDUE, r"<!--\s*SELFCHECK\b");
re_static!(_PROMPT_FIELD, r"^([^\s:][^:]*?):\s*(.*)$");
re_static!(_EVENT_KEY, r"^([^\s:#][^\s:]*)\s*:\s*(.*)$");
re_static!(_README_TOKEN, r"\{\{([A-Z_]+)\}\}");
pub const _README_DROP: &str = "\u{0}readme-drop-line\u{0}";
re_static!(_WS, r"\s+");
re_static!(_ELLIPSIS, r"…|\.\.\.");

// ── private Python-semantics helpers ─────────────────────────────────────────────────────────
static EMPTY: J = J::Dict(Map(Vec::new()));
re_static!(_LOG_SRC_LINE, r":L?\d+$");

fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}
/// `str.splitlines()` (no keepends).
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
/// `Path(s).name` (posix).
fn pname(s: &str) -> String {
    s.split('/').filter(|p| !p.is_empty() && *p != ".").last().unwrap_or("").to_string()
}
/// `Path(s).as_posix()`.
fn pposix(s: &str) -> String {
    let lead = if s.starts_with("//") && !s.starts_with("///") { "//" } else if s.starts_with('/') { "/" } else { "" };
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let r = format!("{lead}{}", parts.join("/"));
    if r.is_empty() { ".".into() } else { r }
}
/// `v in (None, "")`.
fn ne(v: &J) -> bool {
    matches!(v, J::Null) || matches!(v, J::Str(s) if s.is_empty())
}
fn nz(v: &J) -> bool {
    v.truthy()
}
/// `v is None or v == "" or v == [] or v == {}`.
fn empty_val(v: &J) -> bool {
    match v {
        J::Null => true,
        J::Str(s) => s.is_empty(),
        J::List(l) => l.is_empty(),
        J::Dict(m) => m.is_empty(),
        _ => false,
    }
}
/// `str(v)`.
fn ds(v: &J) -> String {
    v.display()
}
/// `x or y`.
fn or<'a>(a: &'a J, b: &'a J) -> &'a J {
    if a.truthy() { a } else { b }
}
/// `for x in (v or [])` (list items; dict keys; str chars).
fn iter_or(v: &J) -> Vec<J> {
    match v {
        J::List(l) => l.clone(),
        J::Dict(m) => m.keys().map(J::from).collect(),
        J::Str(s) => s.chars().map(|c| J::Str(c.to_string())).collect(),
        _ => Vec::new(),
    }
}
fn dicts(v: &J) -> Vec<J> {
    iter_or(v).into_iter().filter(J::is_dict).collect()
}
/// `_dict(obj, key)` by reference.
fn dd<'a>(obj: &'a J, key: &str) -> &'a J {
    match obj.get(key) {
        Some(v @ J::Dict(_)) => v,
        _ => &EMPTY,
    }
}
fn dict_keys_sorted(v: &J) -> Vec<String> {
    let mut k: Vec<String> = v.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default();
    k.sort();
    k
}
fn dump_cell(v: &J) -> String {
    crate::value::dumps_with(v, Dump { indent: None, item_sep: ", ", key_sep: ": ", ensure_ascii: false, sort_keys: true })
}
fn scalar(v: &J) -> String {
    match v {
        J::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        other => other.display(),
    }
}
fn esc(s: &str) -> String {
    s.replace('|', "\\|").replace('\r', " ").replace('\n', " ")
}
fn cla(v: &J, absent: &str) -> String {
    if empty_val(v) {
        return esc(absent);
    }
    match v {
        J::List(l) => l.iter().map(|x| cla(x, absent)).collect::<Vec<_>>().join(" · "),
        J::Dict(_) => esc(&dump_cell(v)),
        other => esc(&scalar(other)),
    }
}
fn cl(v: &J) -> String {
    cla(v, _MISSING_TEXT)
}
fn cls(s: &str) -> String {
    cla(&J::from(s), _MISSING_TEXT)
}
fn clsa(s: &str, absent: &str) -> String {
    cla(&J::from(s), absent)
}
fn clo(s: &Option<String>, absent: &str) -> String {
    match s {
        Some(x) => clsa(x, absent),
        None => esc(absent),
    }
}
fn coda(v: &J, absent: &str) -> String {
    let s = cla(v, absent);
    if empty_val(v) {
        return s;
    }
    if s.contains('`') { format!("`` {s} ``") } else { format!("`{s}`") }
}
fn cod(v: &J) -> String {
    coda(v, _MISSING_TEXT)
}
fn strs_list(v: Vec<String>) -> J {
    J::List(v.into_iter().map(J::Str).collect())
}
fn tb(headers: &[&str], rows: Vec<Vec<String>>) -> Vec<String> {
    let mut out = vec![format!("| {} |", headers.join(" | ")), format!("|{}", "---|".repeat(headers.len()))];
    for r in rows {
        out.push(format!("| {} |", r.join(" | ")));
    }
    out
}
fn row<const N: usize>(a: [String; N]) -> Vec<String> {
    a.to_vec()
}
fn topology(facts: &J) -> Option<String> {
    match dd(facts, "identity").get_str("topology") {
        Some(t @ ("single" | "multi")) => Some(t.to_string()),
        _ => None,
    }
}
fn task_class(facts: &J) -> J {
    or(&facts["task_class"], &dd(facts, "publication")["task_class"]).clone()
}
fn tc_is(facts: &J, v: &str) -> bool {
    task_class(facts).as_str() == Some(v)
}
fn lite(facts: &J) -> bool {
    if !facts.is_dict() {
        return false;
    }
    if let Some(g) = facts.get_str("cell_grade") {
        if CELL_GRADES.contains(&g) {
            return g == "lite";
        }
    }
    let m = dd(facts, "measurement");
    let mc = dd(facts, "measurement_config");
    if mc.get_str("bench_mode_kind") == Some("downgraded-lite") || mc["downgrade_reason"].truthy() {
        return false;
    }
    if m.get_str("benchmark_mode") == Some("lite") {
        return true;
    }
    mc.get_str("bench_mode") == Some("lite") && (mc.get_str("bench_mode_kind") == Some("declared-lite") || tc_is(facts, "hint_map_only"))
}
fn grade(facts: &J) -> &'static str {
    if lite(facts) { "lite" } else { "full" }
}
fn value_text(v: &J) -> String {
    if ne(v) {
        return "(빈 값)".into();
    }
    if v.is_dict() || v.is_list() {
        return dump_cell(v);
    }
    py_strip(&scalar(v)).to_string()
}
fn yes_no(v: &J) -> String {
    match v {
        J::Bool(true) => "예".into(),
        J::Bool(false) => "아니오".into(),
        _ => _MISSING_TEXT.into(),
    }
}
fn missing_codes(missing: &J) -> Vec<String> {
    let v: Vec<String> = match missing {
        J::Dict(m) => m.keys().map(str::to_string).collect(),
        J::List(l) => l.iter().map(ds).collect(),
        _ => Vec::new(),
    };
    let mut v = crate::value::sorted_strs(v);
    v.dedup();
    v
}
fn missing_meanings() -> J {
    crate::evidence::MISSING_CODES().clone()
}

// ── public helper ports ──────────────────────────────────────────────────────────────────────
/// `_norm`: collapse whitespace runs to one space and strip.
pub fn _norm(text: &str) -> HR<String> {
    Ok(py_strip(&_WS().sub_literal(text, " ")).to_string())
}

/// `normalize_source` — split on `\n` only, drop trailing `\r`, keep the last `\r` fragment, remove ANSI sequences.
pub fn normalize_source(text: &str) -> HR<String> {
    let clean = _ANSI().sub_literal(text, "");
    let out: Vec<&str> = clean
        .split('\n')
        .map(|line| {
            let line = line.trim_end_matches('\r');
            match line.rfind('\r') {
                Some(i) => &line[i + 1..],
                None => line,
            }
        })
        .collect();
    Ok(out.join("\n"))
}

/// `read_source_raw(path, errors=...)` — read without newline translation (`errors="replace"` = lossy UTF-8; anything
/// else = strict, a decode failure is `UnicodeDecodeError`).
pub fn read_source_raw(path: &Path, errors: &str) -> HR<String> {
    let bytes = std::fs::read(path).map_err(|e| core::io_err(&e, path))?;
    if errors == "replace" {
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        String::from_utf8(bytes).map_err(|e| HintError::py("UnicodeDecodeError", format!("'utf-8' codec can't decode bytes: {e}")))
    }
}

pub fn _scalar(v: &J) -> HR<String> {
    Ok(scalar(v))
}
pub fn _cell(v: &J, absent: &str) -> HR<String> {
    Ok(cla(v, absent))
}
pub fn _code(v: &J, absent: &str) -> HR<String> {
    Ok(coda(v, absent))
}
/// `_table(headers, rows)` — rows = list of lists of str.
pub fn _table(headers: &[String], rows: &J) -> HR<Vec<String>> {
    let h: Vec<&str> = headers.iter().map(String::as_str).collect();
    Ok(tb(&h, rows.items().iter().map(|r| r.items().iter().map(ds).collect()).collect()))
}
pub fn _dict(obj: &J, key: &str) -> HR<J> {
    Ok(dd(obj, key).clone())
}
/// `_axis(v)` -> `[value, source]`.
pub fn _axis(v: &J) -> HR<J> {
    Ok(if v.is_dict() { jl![v["value"].clone(), v["source"].clone()] } else { jl![v.clone(), J::Null] })
}
pub fn _topology(facts: &J) -> HR<Option<String>> {
    Ok(topology(facts))
}
pub fn _task_class(facts: &J) -> HR<Option<String>> {
    // Python returns the raw value (normally str); a non-str truthy value is shown by its str().
    let t = task_class(facts);
    Ok(if t.is_null() { None } else { Some(ds(&t)) })
}
/// `is_lite_cell` — the single lite-cell predicate.
pub fn is_lite_cell(facts: &J) -> HR<bool> {
    Ok(lite(facts))
}
pub fn cell_grade(facts: &J) -> HR<String> {
    Ok(grade(facts).to_string())
}
pub fn payload_docs(facts: &J) -> HR<Vec<String>> {
    let l = lite(facts);
    Ok(PAYLOAD_DOCS.iter().filter(|n| !(l && **n == BENCH_DOC)).map(|s| s.to_string()).collect())
}
pub fn _cond_text(line: &str, facts: &J) -> HR<String> {
    if !line.contains("[[cell_grade=") {
        return Ok(line.to_string());
    }
    let g = grade(facts);
    Ok(_COND_SEG().sub_fn(line, |m| {
        if crate::pyre::group(m, 1) == Some(g) { crate::pyre::group(m, 2).unwrap_or("").to_string() } else { String::new() }
    }))
}
pub fn _value_text(v: &J) -> HR<String> {
    Ok(value_text(v))
}

/// `template._Ctx` — renderer context. `templates` = the loaded template docs (Python `dict` name -> `_Doc`, load order).
pub struct _Ctx<'a> {
    pub repo: &'a Path,
    pub facts: &'a J,
    pub templates: &'a [(String, _Doc)],
}

// ── 사실 블록 렌더러 (facts → markdown · 결정론) ─────────────────────────────────────────────────
pub fn _f_header(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    let camp = dd(f, "campaign");
    let naming = dd(f, "naming");
    let cnm: Vec<String> = ["id", "cell", "node", "mode"].iter().map(|k| if *k == "id" { camp_id(camp) } else { cl(&camp[*k]) }).collect();
    let rows = vec![
        row(["태그".into(), format!("`{}`", ds(&f["tag"]))]),
        row(["형식".into(), format!("`{FORMAT}` · 이름 문법 `{}`", cl(&naming["grammar"]))]),
        row(["생성(UTC · 주입)".into(), cl(&f["generated_utc"])]),
        row(["캠페인 · 셀 · 노드 · 모드".into(), cnm.join(" · ")]),
    ];
    Ok(tb(&["항목", "값"], rows).join("\n"))
}

fn camp_id(camp: &J) -> String {
    let v = cl(&camp["id"]);
    let mem = dd(camp, "membership");
    if !camp["id"].truthy() && mem.get_str("status") == Some("outside") {
        return format!("캠페인 밖(`{}` · {})", cl(&mem["id"]), cl(&mem["source"]));
    }
    if camp["id"].truthy() && camp["id_source"].truthy() { format!("{v}({})", cl(&camp["id_source"])) } else { v }
}
pub fn _camp_id(camp: &J) -> HR<String> {
    Ok(camp_id(camp))
}

pub fn _f_grade(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    let q = dd(f, "qualification");
    let mc = dd(f, "measurement_config");
    let tc = task_class(f);
    let sources: Vec<String> = if q["sources"].is_list() { q["sources"].items().iter().map(|s| format!("`{}`", ds(s))).collect() } else { vec![] };
    let grade_txt = if lite(f) {
        format!("`{LITE_LABEL}` — lite 벤치만 통과한 셀(이 HW 에서 기동 · 추론 최소조건 증명 · **성능 판정 아님**) · `{BENCH_DOC}` 미발행")
    } else {
        format!("`full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `{BENCH_DOC}`")
    };
    let rows = vec![
        row(["발행 자격(관측)".into(), format!("health 200 = {} · 추론 1회 = {}", cl(&q["health_200"]), cl(&q["inference_observed"]))]),
        row(["자격 근거".into(), cl(&strs_list(sources)) + &qual_scope_text(q)]),
        row(["증거 등급(task_class)".into(), cl(&tc)]),
        row(["측정 등급(bench_mode)".into(), cla(&mc["bench_mode"], _UNRECORDED)]),
        row(["셀 등급(cell_grade)".into(), grade_txt]),
        row(["성능 판정(measurement.verdict)".into(), verdict_text(f)]),
    ];
    let mut out = tb(&["항목", "값"], rows);
    out.extend(marker_lines(f));
    Ok(out.join("\n"))
}

fn verdict_text(f: &J) -> String {
    let m = dd(f, "measurement");
    let v = &m["verdict"];
    let src = &dd(m, "sources")["verdict"];
    if lite(f) {
        return format!(
            "`{LITE_LABEL}` — **성능 판정 아님**(full 판정 PASS · REFUTE 와 분리 · 기계 판정 칸 `{}`) · 출처 {}",
            cla(v, _MISSING_TEXT),
            cla(src, "출처 미기재")
        );
    }
    if ne(v) {
        return format!("{_MISSING_TEXT} — {}", cla(src, "판정 원천 없음"));
    }
    format!("`{}` — 출처 {}", cl(v), cla(src, "출처 미기재"))
}
pub fn _verdict_text(f: &J) -> HR<String> {
    Ok(verdict_text(f))
}

fn marker_lines(f: &J) -> Vec<String> {
    let mut out = Vec::new();
    let m = dd(f, "measurement");
    if m.get_str("verdict") == Some(REFUTE_MARKER) {
        let reasons: Vec<J> = iter_or(&m["verdict_reasons"]).into_iter().filter(|x| matches!(x, J::Str(_) | J::Int(_) | J::Float(_) | J::Bool(_))).collect();
        out.push(String::new());
        out.push(format!(
            "> **{REFUTE_MARKER}** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 {}.",
            cla(&dd(m, "sources")["verdict"], "출처 미기재")
        ));
        if !reasons.is_empty() {
            out.push(format!("> 기각 사유: {}", cl(&J::List(reasons))));
        }
    }
    if lite(f) {
        out.push(String::new());
        out.push(format!("> **{LITE_LABEL}** · **{MAP_ONLY_MARKER}** — 이 셀은 lite 벤치(서빙 직후 1회 스냅샷)만 통과했다: 이 HW 에서 기동 · 추론이 된다는 **최소조건 증명이지 성능 판정이 아니다**(full 판정 PASS · REFUTE 와 분리). 수치는 관측 게재이며 baseline · 권고가 아니다 — `{BENCH_DOC}`(부하 곡선 · like-with-like)는 싣지 않는다. baseline 을 주장하려면 full 측정이 필요하다."));
    } else if tc_is(f, "hint_map_only") {
        out.push(String::new());
        out.push(format!("> **{MAP_ONLY_MARKER}** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다."));
    } else if m.get_str("verdict") == Some(MAP_ONLY_MARKER) {
        out.push(String::new());
        out.push(format!(
            "> **{MAP_ONLY_MARKER}** — 이 셀은 **기준(사용자 목표 c · 외부 레퍼런스 E · expected)이 없어** 성능 판정을 주장하지 않는다(인증서 없음 · 루프 없음). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 {}.",
            cla(&dd(m, "sources")["verdict"], "출처 미기재")
        ));
    }
    out
}
pub fn _marker_lines(f: &J) -> HR<Vec<String>> {
    Ok(marker_lines(f))
}

pub const _AXIS_ORDER: [&str; 9] = ["hw", "gpus_per_node", "nodes", "role", "target", "plane", "q", "len", "kv"];
pub const _SEGMENT_ORDER: [&str; 4] = ["vllm", "model", "arch", "recipe"];

fn axis(v: &J) -> (J, J) {
    if v.is_dict() { (v["value"].clone(), v["source"].clone()) } else { (v.clone(), J::Null) }
}
fn ordered_keys(m: &J, order: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = order.iter().map(|s| s.to_string()).collect();
    let mut rest: Vec<String> = m.as_dict().map(|d| d.keys().filter(|k| !order.contains(k)).map(str::to_string).collect()).unwrap_or_default();
    rest.sort();
    out.extend(rest);
    out
}

pub fn _f_context(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    let ident = dd(f, "identity");
    let naming = dd(f, "naming");
    let isrc = dd(ident, "source");
    let rowf = |label: &str, key: &str| -> Vec<String> {
        if ne(&ident[key]) {
            return vec![label.into(), _MISSING_TEXT.into(), if isrc[key].truthy() { cl(&isrc[key]) } else { _MISSING_TEXT.into() }];
        }
        let mut src = if isrc[key].truthy() { cl(&isrc[key]) } else { "출처 미기재".into() };
        let sv = if isrc[key].truthy() { ds(&isrc[key]) } else { String::new() };
        if key == "quant" && sv.starts_with("naming-axis") && !ds(&isrc[key]).contains("측정 identity quantization = none(관측)") {
            src += " — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다";
        }
        vec![label.into(), cl(&ident[key]), src]
    };
    let mut rows = vec![
        rowf("모델", "model"),
        rowf("HF repo", "hf_repo"),
        rowf("HF revision(체크포인트 git HEAD)", "hf_revision"),
        rowf("base_model", "base_model"),
        rowf("base 슬러그", "base_slug"),
        rowf("양자화", "quant"),
        rowf("GPU", "gpu"),
        vec!["토폴로지 · TP".into(), format!("{} · TP={}", cl(&ident["topology"]), cl(&ident["tp"])), cl(&isrc["topology"])],
        vec!["실행 평면".into(), cl(&f["plane"]), "artifacts.plane_of".into()],
        rowf("vLLM(측정 강식별 키 `vllm_version`)", "vllm"),
    ];
    let nobs = dd(naming, "vllm_observed");
    let prod = &nobs["certificate_vllm_version_producer"];
    if prod.truthy() && !ne(&ident["vllm"]) && ds(&ident["vllm"]) == ds(&nobs["certificate_vllm_version"]) {
        let last = rows.last_mut().unwrap();
        last[2] = format!("{} · 생산자 `{}`(0.4 표 `인증서 vllm_version` 행 — 엔진 자기보고는 그 표의 다른 행)", last[2], cl(prod));
    }
    let mut out = tb(&["항목", "값", "출처"], rows);
    let seg = dd(naming, "segments");
    let axes = dd(naming, "axes");
    let mut arows = Vec::new();
    for name in ordered_keys(seg, &_SEGMENT_ORDER) {
        if let Some(x) = seg.get(&name) {
            let (v, s) = axis(x);
            arows.push(vec![format!("세그먼트 `{name}`"), cl(&v), cl(&s)]);
        }
    }
    for name in ordered_keys(axes, &_AXIS_ORDER) {
        if let Some(x) = axes.get(&name) {
            let (v, s) = axis(x);
            arows.push(vec![format!("축 `{name}`"), cl(&v), cl(&s)]);
        }
    }
    let tail = &naming["tail"];
    if tail.is_list() {
        for r in tail.items() {
            if r.is_dict() {
                arows.push(vec![format!("꼬리 `{}`", cl(&r["token"])), cl(&r["meaning"]), tail_evidence_text(&r["evidence"])]);
            }
        }
        if naming["timestamp"].truthy() {
            arows.push(vec![
                "timestamp".into(),
                format!("`{}`", cl(&naming["timestamp"])),
                "같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론)".into(),
            ]);
        }
    }
    out.extend([
        String::new(),
        "**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.".to_string(),
        String::new(),
    ]);
    if arows.is_empty() {
        out.push("_명명 축 미관측._".into());
    } else {
        out.extend(tb(&["축", "값", "출처"], arows));
    }
    if !tail.is_list() {
        out.extend([String::new(), "_꼬리 미확정 — `continue` 가 draft `inputs/tail.json` 을 근거 대조해 확정한다(0.9 후보 표)._".to_string()]);
    }
    Ok(out.join("\n"))
}

fn tail_evidence_text(ev: &J) -> String {
    if !ev.is_dict() || !ev.truthy() {
        return "근거 미특정".into();
    }
    format!("`{}` · 키 `{}` = `{}`", cl(&ev["file"]), cl(&ev["key"]), cla(&ev["value"], "(빈 값)"))
}
pub fn _tail_evidence_text(ev: &J) -> HR<String> {
    Ok(tail_evidence_text(ev))
}

pub fn _f_name_tail(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    let nm = dd(f, "naming");
    let tail = &nm["tail"];
    if tail.is_list() {
        let ts = if nm["timestamp"].truthy() { format!(" · timestamp `{}`", cl(&nm["timestamp"])) } else { String::new() };
        let mut out = vec![format!("**확정 이름** `{}` — 꼬리 {}토큰{ts}.", cl(&f["tag"]), tail.len()), String::new()];
        if tail.truthy() {
            out.extend(tb(
                &["토큰", "뜻", "근거(서빙 설정)"],
                tail.items().iter().filter(|r| r.is_dict()).map(|r| vec![format!("`{}`", cl(&r["token"])), cl(&r["meaning"]), tail_evidence_text(&r["evidence"])]).collect(),
            ));
        } else {
            out.push("_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._".into());
        }
        if nm["timestamp"].truthy() {
            out.extend([
                String::new(),
                format!("- `{}` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).", cl(&nm["timestamp"])),
            ]);
        }
        return Ok(out.join("\n"));
    }
    let base = or(&f["base_tag"], &f["tag"]);
    let cands = dicts(&f["tail_candidates"]);
    let mut out = vec![
        format!("**기본 이름**(꼬리 전) `{}` — 꼬리 미확정. 아래는 옛 v6 뒤 3축(ple · spec · graph)의 결정론 파생을 돌린 **후보**다(강제 ✗ · 이 셀을 가르는 노브만 고른다 · 셋을 v6 순서 그대로 쓰면 거부된다).", cl(base)),
        String::new(),
    ];
    if !cands.is_empty() {
        let rows = cands
            .iter()
            .map(|x| {
                let src = if x["source"].truthy() {
                    x["source"].clone()
                } else if x["error"].truthy() {
                    J::Str(format!("{}: {}", ds(&x["error"]), ds(&x["message"])))
                } else {
                    J::Null
                };
                vec![
                    cl(&x["axis"]),
                    if x["token"].truthy() { format!("`{}`", cl(&x["token"])) } else { "—".into() },
                    cla(&x["meaning"], "—"),
                    if x["evidence"].truthy() { tail_evidence_text(&x["evidence"]) } else { "근거 미특정 — 저작자가 file · key · value 를 적는다".into() },
                    cla(&src, "—"),
                ]
            })
            .collect();
        out.extend(tb(&["축", "후보 토큰", "뜻", "근거", "출처 · 오류"], rows));
    } else {
        out.push("_후보 없음 — naming.tail_candidates 가 후보를 내지 못했다(꼬리는 저작자가 서빙 설정에서 직접 고른다)._".into());
    }
    Ok(out.join("\n"))
}

pub const _BUILD_KNOWN: [&str; 13] = ["track", "dockerfile", "vllm_repo", "image_tag", "image_digest", "torch", "cuda", "ngc", "cpu_arch", "driver", "driver_by_node", "driver_conflict", "os"];
/// hintlib/template.py:688 _BUILD_LABELS (python literal as JSON)
pub fn _BUILD_LABELS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"track\": \"빌드 트랙\", \"dockerfile\": \"Dockerfile(선택자)\", \"vllm_repo\": \"vLLM 저장소\", \"image_tag\": \"이미지 태그\", \"image_digest\": \"이미지 digest\", \"torch\": \"torch\", \"cuda\": \"CUDA\", \"ngc\": \"NGC 베이스\", \"cpu_arch\": \"CPU arch\", \"driver\": \"드라이버\", \"driver_by_node\": \"드라이버(노드별 관측)\", \"driver_conflict\": \"⚠ 드라이버 관측 ≠ 선언\", \"os\": \"호스트 OS\"}").expect("constant"))
}

/// The `src(value, *cands)` closure of `_f_resolved`.
fn src_of(value: &J, cands: &[J]) -> String {
    if ne(value) {
        return _MISSING_TEXT.into();
    }
    for s in cands {
        if s.truthy() {
            return cl(s);
        }
    }
    "출처 미기재".into()
}
fn obs_where(ss: &J) -> String {
    [("observed", "관측"), ("declared", "선언")]
        .iter()
        .filter(|(k, _)| ss[*k].truthy())
        .map(|(k, lab)| format!("{lab}: {}", cl(&ss[*k])))
        .collect::<Vec<_>>()
        .join(" · ")
}
fn sj(s: &str) -> J {
    if s.is_empty() { J::Null } else { J::from(s) }
}

/// `_build_row(key, v, bsrc, src, build)` with the `_f_resolved` `src` closure (the stub's callable is that closure).
pub fn _build_row(key: &str, v: &J, bsrc: &J, src: &J, build: &J) -> HR<(String, String, String)> {
    let _ = src;
    Ok(build_row(key, v, bsrc, build))
}
fn build_row(key: &str, v: &J, bsrc: &J, build: &J) -> (String, String, String) {
    let label = ds(&_BUILD_LABELS()[key]);
    if key == "driver_by_node" && !v.truthy() && bsrc["driver_by_node"].truthy() {
        return (label, "이 측정에 결속된 노드별 입력 미관측".into(), cl(&bsrc["driver_by_node"]));
    }
    if key == "driver_by_node" && v.is_dict() && v.truthy() {
        let mut items: Vec<(&str, &J)> = v.as_dict().unwrap().iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        let val = items.iter().map(|(n, x)| format!("{}={}", cls(n), cl(x))).collect::<Vec<_>>().join(" · ");
        return (label, val, src_of(v, &[bsrc[key].clone(), bsrc["driver"].clone()]));
    }
    if key == "driver_conflict" && v.is_dict() && v.truthy() {
        let ss = dd(v, "sources");
        let val = format!("관측 {} ≠ 선언 {}", cl(&v["observed"]), cl(&v["declared"]));
        let w = obs_where(ss);
        return (label, val, src_of(v, &[sj(&w), bsrc[key].clone()]));
    }
    let chk = &build["driver_check"];
    if key == "driver_conflict" && empty_v3(v) && chk.is_dict() && matches!(chk.get_str("verdict"), Some("agree" | "unpaired")) {
        let w = obs_where(dd(chk, "sources"));
        let val = if chk.get_str("verdict") == Some("agree") {
            format!("충돌 없음 — 관측 {} = 선언 {}", cl(&chk["observed"]), cl(&chk["declared"]))
        } else {
            "대조 불가 — ".to_string()
                + &if chk["observed"].truthy() {
                    format!("관측 {} · 선언 없음", cl(&chk["observed"]))
                } else {
                    format!("선언 {} · ", cl(&chk["declared"]))
                        + if chk["observation_scope"].truthy() { "이 측정에 결속된 관측 입력 없음" } else { "관측 없음" }
                }
        };
        return (label, val, if w.is_empty() { "출처 미기재".into() } else { w });
    }
    if key == "image_digest" && ne(v) && bsrc["image_digest_local_note"].truthy() {
        return (label, _MISSING_TEXT.into(), cl(&bsrc["image_digest_local_note"]));
    }
    (label, cl(v), src_of(v, &[bsrc[key].clone()]))
}
/// `v in (None, "", {})`.
fn empty_v3(v: &J) -> bool {
    ne(v) || matches!(v, J::Dict(m) if m.is_empty())
}

pub fn _prev_release_row(nbi: &J, build: &J, bsrc: &J, src: &J) -> HR<(String, String, String)> {
    let _ = src;
    Ok(prev_release_row(nbi, build, bsrc))
}
fn prev_release_row(nbi: &J, build: &J, bsrc: &J) -> (String, String, String) {
    let pr = &nbi["prev_release"];
    let desc = &build["vllm_describe"];
    let prev_pin = &build["vllm_prev_pin"];
    let mut obs = Vec::new();
    if desc.truthy() {
        obs.push(format!("빌드 원장 describe `{}`", cl(desc)));
    }
    if prev_pin.truthy() {
        obs.push(format!("업스트림 델타 from_ref `{}`(이 빌드가 올라온 이전 릴리스)", cl(prev_pin)));
    }
    let w = [&bsrc["vllm_describe"], &bsrc["vllm_prev_pin"]].iter().filter(|x| x.truthy()).map(|x| cl(x)).collect::<Vec<_>>().join(" · ");
    let lab = "직전 릴리스(prev_release)".to_string();
    if pr.truthy() {
        return (lab, cl(pr), src_of(pr, &[J::from("naming.vllm_build_input")]));
    }
    if matches!(nbi.get_str("kind"), Some("release" | "wheel")) {
        let val = "해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만)".to_string()
            + &if obs.is_empty() { String::new() } else { format!(" · 관측: {}", obs.join(" · ")) };
        return (lab, val, if w.is_empty() { "naming.vllm_build_input(kind=release)".into() } else { w });
    }
    let val = if obs.is_empty() { _MISSING_TEXT.to_string() } else { format!("{_MISSING_TEXT}(이름 입력) · 관측: {}", obs.join(" · ")) };
    (lab, val, if w.is_empty() { _MISSING_TEXT.into() } else { w })
}

pub fn _f_resolved(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    let build = dd(f, "build");
    let naming = dd(f, "naming");
    let bsrc = dd(build, "source");
    let nbi = dd(naming, "vllm_build_input");
    let nobs = dd(naming, "vllm_observed");
    let nbs = J::from("naming.vllm_build_input");
    let rf = or(&nbi["ref"], &build["vllm_ref"]);
    let sha = or(&nbi["sha"], &build["vllm_sha"]);
    let ident = dd(f, "identity");
    let t3 = |a: &str, b: String, c: String| vec![a.to_string(), b, c];
    let mut rows = vec![
        t3("vLLM 빌드 입력(종류)", cl(&nbi["kind"]), src_of(&nbi["kind"], &[nbs.clone()])),
        t3("vLLM 빌드 입력(ref)", cl(rf), src_of(rf, &[bsrc["vllm_ref"].clone(), nbs.clone()])),
        t3(
            "vLLM 빌드 계보 SHA(40자)",
            cl(sha),
            if sha.truthy() { src_of(sha, &[bsrc["vllm_sha"].clone(), nbs.clone()]) } else { "빌드 입력/원장에 결속된 SHA 미관측 — runtime checkout 관측과 컴파일 바이트 증명은 별개".into() },
        ),
    ];
    let (a, b, cc) = prev_release_row(nbi, build, bsrc);
    rows.push(vec![a, b, cc]);
    for (a, b, cc) in vllm_observed_rows(nobs) {
        rows.push(vec![a, b, cc]);
    }
    rows.push(t3("체크포인트 revision(HF · git HEAD)", cl(&ident["hf_revision"]), src_of(&ident["hf_revision"], &[dd(ident, "source")["hf_revision"].clone()])));
    for key in _BUILD_KNOWN {
        let (a, b, cc) = build_row(key, &build[key], bsrc, build);
        rows.push(vec![a, b, cc]);
    }
    let skip_extra = ["source", "vllm_ref", "vllm_sha", "driver_check", "vllm_describe", "vllm_prev_pin", "source_coordinate_scope"];
    for key in dict_keys_sorted(build) {
        if _BUILD_KNOWN.contains(&key.as_str()) || skip_extra.contains(&key.as_str()) {
            continue;
        }
        rows.push(vec![format!("`{key}`"), cl(&build[key.as_str()]), src_of(&build[key.as_str()], &[bsrc[key.as_str()].clone()])]);
    }
    let mut out = tb(&["항목", "값", "출처"], rows);
    if build["source_coordinate_scope"].truthy() {
        out.extend([String::new(), "> 빌드 입력의 릴리스 태그 또는 SHA는 소스 기준점이다. 변경된 작업트리와 컴파일 바이트의 동일성은 별도 증거가 필요하다. 엔진·인증서·설치 distribution 버전은 각각의 출처로 구분한다.".to_string()]);
    } else {
        out.extend([String::new(), "> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).".to_string()]);
    }
    let qc = &ident["quant_composition"];
    let qsrc = &dd(ident, "source")["quant_composition"];
    out.extend([String::new(), "**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)".to_string(), String::new()]);
    if qc.is_list() && qc.truthy() {
        out.extend(tb(
            &["범위", "dtype", "출처"],
            qc.items().iter().filter(|x| x.is_dict()).map(|x| vec![cl(&x["scope"]), format!("`{}`", cl(&x["dtype"])), cl(&x["source"])]).collect(),
        ));
    } else {
        out.push(format!("_{} — {}._", if qc.is_list() { "빈 구성" } else { _MISSING_TEXT }, cla(qsrc, "출처 미기재")));
    }
    out.extend([String::new(), required_patches_line(f)?]);
    Ok(out.join("\n"))
}

fn patch_records(f: &J) -> Vec<J> {
    let slots = dd(f, "slots");
    let mut out = Vec::new();
    for name in ["build_patch_pre", "build_patch_post"] {
        out.extend(dicts(&dd(slots, name)["file_records"]));
    }
    out
}
pub fn _patch_records(f: &J) -> HR<Vec<J>> {
    Ok(patch_records(f))
}

fn required_patches_line(f: &J) -> HR<String> {
    let recs = patch_records(f);
    let asym = dd(dd(f, "applied_set"), "node_asymmetry");
    if asym.truthy() {
        let mut per = Vec::new();
        for ph in ["pre", "post"] {
            if let Some(t) = crate::artifacts::node_asymmetry_phase_text(asym, ph)? {
                if !t.is_empty() {
                    per.push((ph, t));
                }
            }
        }
        let rep_ = cl(&asym["representative"]);
        let head = if recs.is_empty() {
            format!("**이 모델에 필요한 패치**: 대표({rep_}) 측정 이미지 원장 기준 실린 빌드 패치 없음")
        } else {
            format!("**이 모델에 필요한 패치**(대표 {rep_} 원장 기준 실린 파일 {}개 — 01 §1.1 관련성)", recs.len())
        };
        let mut parts = Vec::new();
        for (ph, t) in &per {
            let tail = t.splitn(2, " — ").nth(1).ok_or_else(|| HintError::py("IndexError", "list index out of range"))?;
            parts.push(format!("`{ph}` {tail}"));
        }
        return Ok(format!("{head} · **노드 비대칭** — {}(원장 관측 — 필요 판정이 아니다 · 01 §1.1 노드별 원장).", parts.join(" · ")));
    }
    if recs.is_empty() {
        return Ok("**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).".into());
    }
    let mut by: Vec<(String, Vec<String>)> = Vec::new();
    for r in &recs {
        let k = ds(or(&r["relevance"], &J::from("unknown")));
        let name = pname(&ds(&r["path"]));
        match by.iter_mut().find(|(a, _)| *a == k) {
            Some((_, v)) => v.push(name),
            None => by.push((k, vec![name])),
        }
    }
    let get = |k: &str| by.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
    let req = get("required");
    Ok("**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): ".to_string()
        + &if req.is_empty() { "required 0개".to_string() } else { format!("required {}", req.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join(" · ")) }
        + &format!(" · inactive-inferred {}개 · unknown {}개", get("inactive-inferred").len(), get("unknown").len())
        + "(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).")
}
pub fn _required_patches_line(f: &J) -> HR<String> {
    required_patches_line(f)
}

/// hintlib/template.py:838 _VLLM_OBS_LABELS (python literal as JSON)
pub fn _VLLM_OBS_LABELS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"engine_self_report\": \"vLLM 엔진 자기보고(엔진 로그 기동 배너)\", \"certificate_vllm_version\": \"인증서 `vllm_version`(강한 일치 키)\", \"wheel_meta\": \"wheel 메타(`vllm_build`)\", \"runtime_version\": \"vLLM runtime 버전(관측 기록 · 배포 메타 여부 미판정)\", \"runtime_source_checkout\": \"vLLM runtime source checkout SHA(컴파일 바이트 증명 아님)\", \"runtime_source_describe\": \"vLLM runtime source describe(관측 기록)\"}").expect("constant"))
}
pub const _VLLM_OBS_ALWAYS: [&str; 2] = ["engine_self_report", "wheel_meta"];
pub const _VLLM_OBS_META_SUFFIX: [&str; 4] = ["_source", "_producer", "_label", "_basis"];

fn vllm_observed_rows(nobs: &J) -> Vec<(String, String, String)> {
    let all: Vec<String> = nobs.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default();
    let keys: Vec<String> = all.into_iter().filter(|k| k != "source" && k != "sources" && !_VLLM_OBS_META_SUFFIX.iter().any(|s| k.ends_with(s))).collect();
    let labels = _VLLM_OBS_LABELS().as_dict().unwrap();
    let mut order: Vec<String> = labels.keys().filter(|k| keys.iter().any(|x| x == k) || _VLLM_OBS_ALWAYS.contains(k)).map(str::to_string).collect();
    let mut rest: Vec<String> = keys.iter().filter(|k| !labels.contains_key(k)).cloned().collect();
    rest.sort();
    order.extend(rest);
    let per = dd(nobs, "sources");
    let mut rows = Vec::new();
    for k in order {
        let raw = &nobs[k.as_str()];
        let (val, mut src, label) = if raw.is_dict() { (raw["value"].clone(), raw["source"].clone(), raw["label"].clone()) } else { (raw.clone(), J::Null, J::Null) };
        if !src.truthy() {
            let mut parts = Vec::new();
            let p = &nobs[format!("{k}_producer").as_str()];
            if p.truthy() {
                parts.push(format!("생산자 `{}`", cl(p)));
            }
            let s1 = &nobs[format!("{k}_source").as_str()];
            if s1.truthy() || per[k.as_str()].truthy() {
                parts.push(cl(or(s1, &per[k.as_str()])));
            }
            let b = &nobs[format!("{k}_basis").as_str()];
            if b.truthy() {
                parts.push(format!("지위 `{}`", cl(b)));
            }
            src = sj(&parts.join(" · "));
        }
        let lab = if label.truthy() {
            label
        } else if nobs[format!("{k}_label").as_str()].truthy() {
            nobs[format!("{k}_label").as_str()].clone()
        } else if let Some(l) = labels.get(&k) {
            l.clone()
        } else {
            J::Str(format!("`{k}`"))
        };
        if ne(&val) {
            let mv = if k == "wheel_meta" && nobs.get_str("wheel_meta_basis") == Some("observed-node-difference") {
                "노드별 상이·공통값 미선택(각 값은 출처)".to_string()
            } else {
                _MISSING_TEXT.to_string()
            };
            rows.push((cl(&lab), mv, if src.truthy() { cl(&src) } else { _MISSING_TEXT.into() }));
        } else {
            let s = if src.truthy() { src } else { nobs["source"].clone() };
            rows.push((cl(&lab), cl(&val), if s.truthy() { cl(&s) } else { "출처 미기재".into() }));
        }
    }
    rows
}
pub fn _vllm_observed_rows(nobs: &J) -> HR<Vec<(String, String, String)>> {
    Ok(vllm_observed_rows(nobs))
}

pub fn _f_meta(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    let camp = dd(f, "campaign");
    let pub_ = dd(f, "publication");
    let appr = dd(f, "approval");
    let cn: Vec<String> = ["id", "cell", "node"].iter().map(|k| if *k == "id" { camp_id(camp) } else { cl(&camp[*k]) }).collect();
    let mut rows = vec![
        row(["태그".into(), format!("`{}`", ds(&f["tag"]))]),
        row(["형식".into(), format!("`{FORMAT}`")]),
        row(["앵커".into(), "`PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다".into()]),
        row(["캠페인 · 셀 · 노드".into(), cn.join(" · ")]),
        row(["발행 모드".into(), cl(&camp["mode"])]),
        row(["발행 토픽".into(), cl(&pub_["topic"])]),
        row(["work-manifest".into(), cl(&pub_["manifest_ref"])]),
        row(["증거 등급(task_class)".into(), cl(&task_class(f))]),
    ];
    for (a, b) in approval_rows(appr, dd(f, "prior_approval")) {
        rows.push(vec![a, b]);
    }
    rows.push(row(["현행 full 정의 충족(측정 등급)".into(), bench_definition_verdict(dd(f, "bench_definition"))]));
    rows.push(row(["독립 사실 검증(발행 전)".into(), factcheck_line(&f["factcheck"])]));
    Ok(tb(&["항목", "값"], rows).join("\n"))
}

fn approval_rows(appr: &J, prior: &J) -> Vec<(String, String)> {
    if appr.truthy() || !prior.truthy() {
        return vec![
            ("승인 출처".into(), cl(&appr["source"])),
            ("승인 시각(UTC)".into(), cl(&appr["approved_utc"])),
            ("승인 발화(전사)".into(), cl(&appr["approved_by"])),
        ];
    }
    let tag = format!("(재생 전용 대체 — 원 발행의 승인 · 이 판의 승인은 continue 가 적는다 · {})", cl(&prior["replay_source"]));
    vec![
        ("승인 출처".into(), format!("{}{tag}", cl(&prior["source"]))),
        ("승인 시각(UTC)".into(), format!("{}(원 발행)", cl(&prior["approved_utc"]))),
        ("승인 발화(전사)".into(), format!("{}(원 발행)", cl(&prior["approved_by"]))),
    ]
}
pub fn _approval_rows(appr: &J, prior: &J) -> HR<Vec<(String, String)>> {
    Ok(approval_rows(appr, prior))
}
pub fn _yes_no(v: &J) -> HR<String> {
    Ok(yes_no(v))
}

fn bench_definition_source(bd: &J) -> String {
    let src = &bd["source"];
    let ln = &bd["source_line"];
    if src.truthy() && matches!(ln, J::Int(_) | J::Bool(_)) { format!("{}:{}", cl(src), ds(ln)) } else { cl(src) }
}
pub fn _bench_definition_source(bd: &J) -> HR<String> {
    Ok(bench_definition_source(bd))
}
fn bench_definition_verdict(bd: &J) -> String {
    if !bd.truthy() {
        return _MISSING_TEXT.into();
    }
    format!("{} · 이 측정의 반복 {} · 정의 출처 `{}`(03 §3.5)", yes_no(&bd["meets_current_full"]), cl(&bd["this_repeats"]), bench_definition_source(bd))
}
pub fn _bench_definition_verdict(bd: &J) -> HR<String> {
    Ok(bench_definition_verdict(bd))
}

fn factcheck_line(fc: &J) -> String {
    if !fc.is_dict() || !fc.truthy() {
        return "미실시 — continue 가 `inputs/factcheck.json`(저작자가 아닌 Agent 의 검증 보고)을 요구한다".into();
    }
    match fc.get_str("status") {
        Some("passed") => format!(
            "통과 — 검증자 `{}`(저작자 `{}` 아님) · 주장 {}건 · 지적 {}건 중 고침 {}건 · 열림 0 · {}",
            cl(&fc["checker"]),
            cl(&fc["author"]),
            cl(&fc["claims_checked"]),
            cl(&fc["items"]),
            cl(&fc["fixed"]),
            cl(&fc["checked_utc"])
        ),
        Some("waived") => {
            let what = match ds(&fc["reason_code"]).as_str() {
                "HINT_FACTCHECK_ABSENT" => "독립 검증 보고 없이 발행",
                "HINT_FACTCHECK_INVALID" => "검증 보고 모양 결함인 채 발행",
                "HINT_FACTCHECK_OPEN" => "검증 지적이 열린 채 발행",
                _ => "사실 검증 게이트 미통과인 채 발행",
            };
            format!("**사람 면제**({what}) — 사유코드 `{}` · 열린 지적 {} · 면제 발화 {}", cl(&fc["reason_code"]), cla(&fc["open"], "0"), cl(&fc["waiver"]))
        }
        _ => format!("미통과(`{}`) — 열린 지적 {}", cl(&fc["reason_code"]), cla(&fc["open"], "0")),
    }
}
pub fn _factcheck_line(fc: &J) -> HR<String> {
    Ok(factcheck_line(fc))
}

/// `_missing_meanings` — the code table owner is evidence (`MISSING_CODES`; in-crate, so it cannot be missing).
pub fn _missing_meanings() -> HR<J> {
    Ok(missing_meanings())
}

fn missing_block(missing: &J, meanings: &J, reasons: &J) -> String {
    let items: Vec<(String, J)> = match missing {
        J::Dict(m) => {
            let mut v: Vec<(String, J)> = m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        }
        J::List(_) => missing_codes(missing).into_iter().map(|c| (c, J::Null)).collect(),
        _ => Vec::new(),
    };
    if items.is_empty() {
        return "**결손**: _없음 — 결손 코드표를 전수 대조한 결과 0건이다(침묵은 '없음' 과 구분되지 않는다 — 없음도 적는다)._".into();
    }
    let mut table_meanings: Option<J> = if meanings.is_null() { None } else { Some(meanings.clone()) };
    let mut rows: Vec<(String, String)> = Vec::new();
    for (code, meaning) in &items {
        let mut m = meaning.clone();
        if !m.truthy() {
            let t = table_meanings.get_or_insert_with(missing_meanings);
            m = t[code.as_str()].clone();
        }
        rows.push((format!("`{code}`"), cl(or(&m, &J::from("미등록 사유코드 — `evidence.MISSING_CODES` 에 뜻을 등재하라")))));
    }
    let mut out = vec!["**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).".to_string(), String::new()];
    let rs: Vec<(String, J)> = reasons.as_dict().map(|m| m.iter().filter(|(_, v)| v.truthy()).map(|(k, v)| (k.to_string(), v.clone())).collect()).unwrap_or_default();
    if !rs.is_empty() {
        let tr: Vec<Vec<String>> = rows
            .iter()
            .zip(items.iter())
            .map(|((a, b), (code, _))| {
                let key = code.trim_matches('`');
                let r = rs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or(J::Null);
                vec![a.clone(), b.clone(), cla(&r, "—")]
            })
            .collect();
        out.extend(tb(&["코드", "뜻", "이 셀의 사유"], tr));
    } else {
        out.extend(tb(&["코드", "뜻"], rows.into_iter().map(|(a, b)| vec![a, b]).collect()));
    }
    out.join("\n")
}
pub fn _missing_block(missing: &J, meanings: &J, reasons: &J) -> HR<String> {
    Ok(missing_block(missing, meanings, reasons))
}
/// `_missing_codes` (a set; returned sorted).
pub fn _missing_codes(missing: &J) -> HR<Vec<String>> {
    Ok(missing_codes(missing))
}

fn attestation_scope_line(f: &J) -> Option<String> {
    let sc = &f["attestation_scope"];
    if !sc.is_dict() || !sc.truthy() {
        return None;
    }
    let mut parts = vec![format!("config `{}`", cl(&sc["config"]))];
    if sc["phase"].truthy() {
        parts.push(format!("phase {}", cl(&sc["phase"])));
    }
    parts.push(format!(
        "작성 {}{}",
        cl(&sc["written_utc"]),
        if sc["written_utc_source"].truthy() { format!("({})", cl(&sc["written_utc_source"])) } else { String::new() }
    ));
    if sc["bound_by"].truthy() {
        parts.push(format!("묶은 근거 {}", cl(&sc["bound_by"])));
    }
    if sc["path"].truthy() {
        parts.push(format!("파일 `{}`", cl(&sc["path"])));
    }
    if sc["timing_note"].truthy() {
        parts.push(format!("측정 창 대비 {}", cl(&sc["timing_note"])));
    }
    Some(format!("> **attestation 범위** — {}: **{}**", parts.join(" · "), cl(&sc["scope"])))
}
pub fn _attestation_scope_line(f: &J) -> HR<Option<String>> {
    Ok(attestation_scope_line(f))
}

pub fn _f_missing(c: &_Ctx) -> HR<String> {
    let mut out = missing_block(&c.facts["missing"], &J::Null, dd(c.facts, "missing_reasons"));
    if let Some(note) = attestation_scope_line(c.facts) {
        if !note.is_empty() && missing_codes(&c.facts["missing"]).iter().any(|code| code.contains("ATTESTATION")) {
            out += "\n\n";
            out += &note;
        }
    }
    Ok(out)
}

pub fn _f_bench_missing(c: &_Ctx) -> HR<String> {
    let missing = &c.facts["missing"];
    let codes = missing_codes(missing);
    let sel: Vec<&str> = BENCH_MISSING_CODES.iter().copied().filter(|code| codes.iter().any(|x| x == code)).collect();
    let mut out: Vec<String> = Vec::new();
    let m = dd(c.facts, "measurement");
    let v = m.get_str("verdict");
    if codes.iter().any(|x| x == "HINT_MISSING_CERTIFICATE") {
        out.extend([
            "> **인증서가 없다** — 인증서가 발행되는 판정(루브릭 권한 explicit ∧ PASS, 또는 판정 · 권한 미관측)인데 인증서가 묶이지 않았다(발행은 `adversarial-benchmark` 의 책임).".to_string(),
            format!("> 부재는 '성능이 나빴다' 가 아니라 인증서 형태로 '**그 형태로 판정되지 않았다**' 는 뜻이다 — 판정 {} · 위 수치는 관측이다.", verdict_text(c.facts)),
            String::new(),
        ]);
    } else if v == Some(REFUTE_MARKER)
        || v == Some(MAP_ONLY_MARKER)
        || (v == Some("PASS") && !(if dd(m, "sources")["verdict"].truthy() { ds(&dd(m, "sources")["verdict"]) } else { String::new() }).starts_with("certificate"))
    {
        out.extend([
            format!("> 인증서 비발행 판정(결손 아님) — {} · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `{}`).", verdict_text(c.facts), cl(&m["rubric_authority"])),
            String::new(),
        ]);
    }
    if sel.is_empty() {
        out.push(format!(
            "**측정 결손**: _없음 — 측정 결손 코드({})를 대조한 결과 0건이다(없음도 적는다)._",
            BENCH_MISSING_CODES.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join(" · ")
        ));
        return Ok(out.join("\n"));
    }
    let sub = match missing {
        J::Dict(mm) => {
            let mut d = Map::new();
            for (k, v) in mm.iter() {
                if sel.contains(&k) {
                    d.insert(k, v.clone());
                }
            }
            J::Dict(d)
        }
        _ => strs_list(sel.iter().map(|s| s.to_string()).collect()),
    };
    out.push(missing_block(&sub, &J::Null, &J::Null).replacen("**결손** —", "**측정 결손** —", 1));
    Ok(out.join("\n"))
}

/// `(p.fields, p.chapter)` of each prompt of a template doc (t2's `_Doc` / `_Prompt`).
fn doc_prompts(doc: &_Doc) -> HR<Vec<(J, String)>> {
    Ok(doc.prompts.iter().map(|p| (p.fields.clone(), p.chapter.clone())).collect())
}

/// `_event_homes(templates)` — kind -> "<file> §<chapter>" list, derived from the templates' `블록:` directives.
pub fn _event_homes(templates: &[(String, _Doc)]) -> HR<J> {
    let mut homes: Vec<(String, Vec<String>)> = Vec::new();
    for name in TEMPLATE_FILES {
        let Some((_, doc)) = templates.iter().find(|(n, _)| n == name) else { continue };
        for (fields, chapter) in doc_prompts(doc)? {
            let dir = _directives(&fields)?;
            for kind in iter_or(&dir["blocks"]) {
                let k = ds(&kind);
                let home = format!("{} §{}", &name[..2], chapter);
                match homes.iter_mut().find(|(a, _)| *a == k) {
                    Some((_, v)) => v.push(home),
                    None => homes.push((k, vec![home])),
                }
            }
        }
    }
    let mut m = Map::new();
    for (k, v) in homes {
        m.insert(k, v.join(" · "));
    }
    Ok(J::Dict(m))
}

pub fn _event_kinds_md(templates: &[(String, _Doc)]) -> HR<String> {
    let homes = _event_homes(templates)?;
    let rows = EVENT_KINDS
        .iter()
        .map(|k| {
            vec![
                format!("`{k}`"),
                format!("`{}<n>`", ds(&EVENT_ID_PREFIX()[*k])),
                EVENT_REQUIRED()[*k].items().iter().map(ds).collect::<Vec<_>>().join(" · "),
                cl(&homes[*k]),
            ]
        })
        .collect();
    Ok(tb(&["kind", "id", "필수 필드", "자리"], rows).join("\n"))
}

pub fn _legend_md() -> HR<String> {
    let join = |d: &J| d.as_dict().unwrap().iter().map(|(k, v)| format!("**{k}**({})", ds(v))).collect::<Vec<_>>().join(" · ");
    let out = [
        "**범례** — 린터가 hint-event 값 검증에 쓰는 어휘와 같은 상수에서 생성했다.".to_string(),
        String::new(),
        format!("- 전이등급: {}", join(TRANSFER_CLASSES())),
        format!("- 지위: {}", join(VALUE_STATUSES())),
        format!("- 현재지위(오진 · 권장 — **원래 주장**의 지위): {}", join(CURRENT_STATUS_MEANINGS())),
    ];
    Ok(out.join("\n"))
}

pub fn _f_legend(c: &_Ctx) -> HR<String> {
    Ok(_event_kinds_md(c.templates)? + "\n\n" + &_legend_md()?)
}

fn shipped_names(f: &J) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(m) = dd(f, "slots").as_dict() {
        for row in m.values() {
            if row.is_dict() {
                for p in iter_or(&row["files"]) {
                    let n = pname(&ds(&p));
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
        }
    }
    names.sort();
    names
}
pub fn _shipped_names(f: &J) -> HR<Vec<String>> {
    Ok(shipped_names(f))
}

fn slot_reason(s: &J) -> String {
    if s["rationale"].truthy() {
        return cl(&s["rationale"]);
    }
    if s["machine_reason"].truthy() {
        return format!("{}(기계 파생)", cl(&s["machine_reason"]));
    }
    _MISSING_TEXT.into()
}
pub fn _slot_reason(s: &J) -> HR<String> {
    Ok(slot_reason(s))
}

fn btick_join(xs: &[String]) -> String {
    xs.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join(" · ")
}

pub fn _f_slots(c: &_Ctx) -> HR<String> {
    let slots = dd(c.facts, "slots");
    if !slots.truthy() {
        return Ok("_슬롯 판정 미관측 — artifacts 수집 결과가 없다(결손)._".into());
    }
    let names = dict_keys_sorted(slots);
    let mut rows = Vec::new();
    for name in &names {
        let s = if slots[name.as_str()].is_dict() { &slots[name.as_str()] } else { &EMPTY };
        let ev = dd(s, "evidence");
        let files: Vec<String> = iter_or(&s["files"]).iter().map(|p| format!("`{}`", pposix(&ds(p)))).collect();
        rows.push(vec![
            format!("`{name}`"),
            if s["files"].truthy() { "실림".into() } else { "안 실림".into() },
            cla(&strs_list(files), "—"),
            cl(&s["confidence"]),
            cl(&if ev.truthy() { J::Str(format!("{}:{}", ds(&ev["kind"]), ds(&ev["ref"]))) } else { J::Null }),
            slot_reason(s),
        ]);
    }
    let mut out = tb(&["슬롯", "실림", "파일", "신호", "적용 증거", "사유"], rows);
    let mut notes: Vec<String> = Vec::new();
    for name in &names {
        let sl = &slots[name.as_str()];
        let isd = sl.is_dict();
        let ev = dd(if isd { sl } else { &EMPTY }, "evidence");
        for r in iter_or(&ev["recipe_vs_image"]) {
            if r.is_dict() && r["warning"].truthy() {
                notes.push(format!(
                    "- ⚠ `{name}` `{}` — {}(이미지 Created {} · {} {}{})",
                    cl(&r["path"]),
                    cl(&r["warning"]),
                    cl(&r["image_created_utc"]),
                    cl(or(&r["last_commit_kind"], &J::from("마지막 커밋"))),
                    cl(&r["last_commit_utc"]),
                    if r["checkout_path"].truthy() { format!(" · 이 체크아웃 통로 `{}` 의 파일이 아니다", cl(&r["checkout_path"])) } else { String::new() }
                ));
            }
        }
        for r in iter_or(&ev["context_not_shipped"]) {
            if r.is_dict() {
                notes.push(format!("- ⚠ `{name}` COPY 원천 `{}` 미실림 — {}", cl(&r["source"]), cl(&r["why"])));
            }
        }
        let revs = dicts(&ev["selected_revisions"]);
        let rev_paths: Vec<String> = revs.iter().filter(|r| r["path"].truthy()).map(|r| ds(&r["path"])).collect();
        let sel = &ev["selected_revision"];
        if sel.is_dict() && sel.truthy() && (!sel["path"].truthy() || !rev_paths.contains(&ds(&sel["path"]))) {
            notes.push(revision_line(name, sel));
        }
        notes.extend(revs.iter().map(|r| revision_line(name, r)));
        for w in iter_or(&ev["revision_warnings"]) {
            if let J::Str(s) = &w {
                if !s.is_empty() {
                    notes.push(format!("- ⚠ `{name}` 리비전 경고: {}", cl(&w)));
                }
            }
        }
        let listed: Vec<String> = if isd {
            dicts(&sl["excluded"])
                .iter()
                .filter(|x| {
                    ds(or(or(&x["reason"], &x["why"]), &J::from(""))).starts_with(_POST_MEASUREMENT)
                })
                .map(|x| ds(&x["file"]))
                .collect()
        } else {
            vec![]
        };
        let regen: Vec<String> = iter_or(&ev["post_measurement_regenerated"])
            .into_iter()
            .filter_map(|x| match x {
                J::Str(s) if !s.is_empty() => Some(s),
                _ => None,
            })
            .filter(|x| !listed.iter().any(|f| !f.is_empty() && (x == f || x.ends_with(&format!("/{f}")))))
            .collect();
        if !regen.is_empty() {
            let shipped: Vec<String> = if isd { iter_or(&sl["files"]).iter().map(|p| pname(&ds(p))).collect() } else { vec![] };
            let on: Vec<String> = regen.iter().filter(|x| shipped.contains(&pname(x)) || shipped.contains(&format!("{}.template", pname(x)))).cloned().collect();
            let off: Vec<String> = regen.iter().filter(|x| !on.contains(x)).cloned().collect();
            if !on.is_empty() {
                notes.push(format!("- ⚠ `{name}` 측정 뒤 재생성된 파일(mtime > 측정 시각)을 실었다: {} — 실린 형상은 측정 당시 값과 다를 수 있다(측정 당시 값 = 01 §1.4 측정 env 관측)", btick_join(&on)));
            }
            if !off.is_empty() {
                notes.push(format!("- `{name}` 측정 뒤 재생성된 파일(mtime > 측정 시각)은 싣지 않았다: {} — 측정 당시 값이 아니다(측정 당시 값 = 01 §1.4 측정 env 관측)", btick_join(&off)));
            }
        }
    }
    if !notes.is_empty() {
        out.extend([String::new(), "**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)".to_string(), String::new()]);
        out.extend(notes);
    }
    let ex = excluded_lines(slots);
    if !ex.is_empty() {
        out.extend([String::new(), "**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)".to_string(), String::new()]);
        out.extend(ex);
    }
    out.extend(file_marks_table(slots));
    out.extend(agent_requests_table(c.facts));
    Ok(out.join("\n"))
}

fn file_records(slots: &J) -> Vec<(String, J)> {
    let mut out = Vec::new();
    for name in dict_keys_sorted(slots) {
        let row = &slots[name.as_str()];
        if !row.is_dict() {
            continue;
        }
        for r in dicts(&row["file_records"]) {
            if r["path"].truthy() {
                out.push((name.clone(), r));
            }
        }
    }
    out
}
pub fn _file_records(slots: &J) -> HR<Vec<(String, J)>> {
    Ok(file_records(slots))
}
fn verification_cell(r: &J) -> String {
    format!("`{}` · {}", cl(&r["verification"]), cl(&r["generated_by"]))
}
pub fn _verification_cell(r: &J) -> HR<String> {
    Ok(verification_cell(r))
}
fn file_marks_table(slots: &J) -> Vec<String> {
    let recs = file_records(slots);
    if recs.is_empty() {
        return vec![];
    }
    let mut out = vec![
        String::new(),
        "**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)".to_string(),
        String::new(),
    ];
    out.extend(tb(
        &["슬롯", "파일", "검증", "검증 근거", "관련성", "관련성 근거"],
        recs.iter()
            .map(|(name, r)| {
                vec![
                    format!("`{name}`"),
                    format!("`{}`", cl(&r["path"])),
                    verification_cell(r),
                    cl(&r["verification_basis"]),
                    format!("`{}`", cl(&r["relevance"])),
                    cla(&r["relevance_basis"], "—"),
                ]
            })
            .collect(),
    ));
    out
}
pub fn _file_marks_table(slots: &J) -> HR<Vec<String>> {
    Ok(file_marks_table(slots))
}
fn agent_requests_table(f: &J) -> Vec<String> {
    let reqs: Vec<J> = dicts(&f["agent_requests"]).into_iter().filter(|r| r["path"].truthy()).collect();
    if reqs.is_empty() {
        return vec![];
    }
    let mut out = vec![
        String::new(),
        format!("**Agent 저작 요청 {}건**(렌더러 입력이 없어 기계가 만들지 못한 자리 — 저작 Agent 가 증거에서 쓴다 · 첫 줄 `{UNVERIFIED_MARK} … 생성: agent` 유지 · verification 은 저작 뒤에도 generated-unverified)", reqs.len()),
        String::new(),
    ];
    out.extend(tb(&["파일", "사유", "입력"], reqs.iter().map(|r| vec![format!("`{}`", cl(&r["path"])), cl(&r["why"]), cla(&r["inputs"], "—")]).collect()));
    out
}
pub fn _agent_requests_table(f: &J) -> HR<Vec<String>> {
    Ok(agent_requests_table(f))
}

fn revision_line(slot: &str, r: &J) -> String {
    let hm = if r["history_match"].is_dict() { Some(&r["history_match"]) } else { None };
    let commit = &r["commit"];
    let head = if commit.truthy() { format!("커밋 `{}`", core::head_chars(&ds(commit), 12)) } else { "리비전 미특정".into() };
    let path = if r["origin_path"].truthy() {
        format!(" `{}`(이 체크아웃 통로 `{}` 대신)", cl(&r["origin_path"]), cl(&r["path"]))
    } else if r["path"].truthy() {
        format!(" `{}`", cl(&r["path"]))
    } else {
        String::new()
    };
    let m = match hm {
        Some(h) => format!("docker history 대조 {}/{}", cl(&h["matched"]), cl(&h["total"])),
        None => "docker history 대조 없음".into(),
    };
    let mut extra = String::new();
    if !ne(&r["shipped"]) {
        extra += &format!(" · 실린 바이트 `{}`", cl(&r["shipped"]));
    }
    if r["basis"].truthy() {
        extra += &format!(" · 근거 `{}`", cl(&r["basis"]));
    }
    if r["executed"] == J::Bool(false) {
        extra += " · 쓰인 바이트임을 관측으로 확인 해당 없음(실행 안 됨)";
    } else if r["verified"].is_bool() {
        extra += &format!(" · 쓰인 바이트임을 관측으로 확인 {}", yes_no(&r["verified"]));
    }
    let after = &r["commits_after_measurement"];
    if after.is_list() && after.truthy() {
        extra += &format!(" · 측정 뒤 커밋 {}건", after.len());
    }
    let line = format!("- `{slot}`{path} 실린 리비전: {head} · 선택 방법 `{}` · {m}{extra}", cl(&r["method"]));
    if r["warning"].truthy() { format!("{line} — ⚠ {}", cl(&r["warning"])) } else { line }
}
pub fn _revision_line(slot: &str, r: &J) -> HR<String> {
    Ok(revision_line(slot, r))
}

pub const _EXCLUDED_BRIEF: [&str; 1] = ["known-non-patch"];
pub const _EXCLUDED_IN_APPLIED: [&str; 1] = ["skipped"];
pub const _POST_MEASUREMENT: &str = "post-measurement-regenerated";
/// hintlib/template.py:1253 _EXCLUDED_MEANINGS
pub fn _EXCLUDED_MEANINGS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| jd! {_POST_MEASUREMENT => "측정 뒤 재생성된 형상 — 렌더러가 측정 뒤에 바뀌면 측정 당시 없던 키 · 다른 값이 실린다 (측정 당시 값 = 01 §1.4 측정 env 관측)"})
}
pub const _EXCLUDED_EXTRA_KEYS: [&str; 2] = ["mtime_utc", "measured_utc"];

fn excluded_lines(slots: &J) -> Vec<String> {
    let mut out = Vec::new();
    for name in dict_keys_sorted(slots) {
        let row = if slots[name.as_str()].is_dict() { &slots[name.as_str()] } else { &EMPTY };
        let mut brief: Vec<(String, Vec<String>)> = Vec::new();
        for x in iter_or(&row["excluded"]) {
            if !x.is_dict() {
                continue;
            }
            let why = ds(or(or(&x["why"], &x["reason"]), &J::from("")));
            let code = py_strip(why.split('(').next().unwrap_or("")).to_string();
            if _EXCLUDED_IN_APPLIED.contains(&code.as_str()) {
                continue;
            }
            if _EXCLUDED_BRIEF.contains(&code.as_str()) {
                let file = ds(&x["file"]);
                match brief.iter_mut().find(|(a, _)| *a == code) {
                    Some((_, v)) => v.push(file),
                    None => brief.push((code, vec![file])),
                }
                continue;
            }
            let extras: Vec<String> = _EXCLUDED_EXTRA_KEYS.iter().filter(|k| !ne(&x[**k])).map(|k| format!("{k} {}", cl(&x[*k]))).collect();
            let meaning = if why.contains('(') { None } else { _EXCLUDED_MEANINGS().get_str(&code).map(str::to_string) };
            out.push(format!(
                "- `{name}` `{}` — `{}`{}{}",
                cl(&x["file"]),
                cls(&why),
                meaning.map(|m| format!(" · {m}")).unwrap_or_default(),
                if extras.is_empty() { String::new() } else { format!(" · {}", extras.join(" · ")) }
            ));
        }
        brief.sort_by(|a, b| a.0.cmp(&b.0));
        for (code, files) in brief {
            out.push(format!("- `{name}` {code} {}건: {}", files.len(), files.iter().map(|f| format!("`{}`", cls(f))).collect::<Vec<_>>().join(" · ")));
        }
    }
    out
}
pub fn _excluded_lines(slots: &J) -> HR<Vec<String>> {
    Ok(excluded_lines(slots))
}

fn missing_sentence(f: &J, code: &str, absent_text: &str) -> String {
    let missing = &f["missing"];
    if !missing_codes(missing).iter().any(|x| x == code) {
        return absent_text.to_string();
    }
    let mut meaning = if missing.is_dict() { missing[code].clone() } else { J::Null };
    if !meaning.truthy() {
        meaning = missing_meanings()[code].clone();
    }
    let reason = &dd(f, "missing_reasons")[code];
    format!(
        "_결손 `{code}` — {}{}(00 §0.7 결손 표)._",
        cla(&meaning, "뜻 미등록"),
        if reason.truthy() { format!(" · 이 셀의 사유: {}", cl(reason)) } else { String::new() }
    )
}
pub fn _missing_sentence(f: &J, code: &str, absent_text: &str) -> HR<String> {
    Ok(missing_sentence(f, code, absent_text))
}

pub fn _f_applied_set(c: &_Ctx) -> HR<String> {
    let a = dd(c.facts, "applied_set");
    if !a.truthy() {
        return Ok(missing_sentence(c.facts, "HINT_MISSING_APPLIED_SET", "_적용 집합 없음 — 빌드 패치 슬롯 입력이 없고 결손 코드도 없다(판정할 대상이 없다)._"));
    }
    let shipped = shipped_names(c.facts);
    let mut out = vec![
        format!(
            "**적용 집합** — 상태 `{}` · 출처 `{}`{}",
            cl(&a["status"]),
            cl(&a["source"]),
            if a.get_str("status") == Some("unobservable") { " · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)" } else { "" }
        ),
        String::new(),
    ];
    let patches = dicts(&a["patches"]);
    if !patches.is_empty() {
        let rows = patches
            .iter()
            .map(|p| {
                let res = &p["result"];
                let shown = if res.as_str() == Some("unobserved") { "unobserved(적용 미관측)".to_string() } else { cl(res) };
                vec![
                    cl(&p["phase"]),
                    format!("`{}`", cl(&p["file"])),
                    shown,
                    cl(&p["result_source"]),
                    cla(&p["reason"], "—"),
                    if shipped.contains(&ds(&p["file"])) { "실림".into() } else { "안 실림".into() },
                    cla(&p["declared_model_trigger"], "—"),
                ]
            })
            .collect();
        out.extend(tb(&["단계", "파일", "결과", "결과 출처", "사유", "실림", "model-trigger"], rows));
        let mut used: Vec<String> = patches.iter().filter(|p| p["result_source"].truthy()).map(|p| ds(&p["result_source"])).collect();
        used.sort();
        used.dedup();
        if !used.is_empty() {
            out.extend([String::new(), "**결과 출처 범례**".to_string(), String::new()]);
            for u in used {
                let m = RESULT_SOURCE_MEANINGS().get(&u).map(ds).unwrap_or_else(|| "뜻 미등록(template.RESULT_SOURCE_MEANINGS)".into());
                out.push(format!("- `{u}` — {m}"));
            }
        }
        out.extend(patch_evidence_table(&patches));
    } else {
        out.push("_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._".into());
    }
    let asym = dd(a, "node_asymmetry");
    if asym.truthy() && asym["nodes"].is_dict() {
        out.extend([
            String::new(),
            format!("**노드별 원장**(cluster · 대표 `{}` — {})", cl(&asym["representative"]), cl(&asym["note"])),
            String::new(),
        ]);
        let mut nrows = Vec::new();
        let nodes = asym["nodes"].as_dict().unwrap();
        let mut ks: Vec<&str> = nodes.keys().collect();
        ks.sort();
        for n in ks {
            let nd = nodes.get(n).unwrap();
            let pts = if nd.is_dict() { dicts(&nd["patches"]) } else { vec![] };
            for x in &pts {
                nrows.push(vec![format!("`{}`", cls(n)), cl(&x["phase"]), format!("`{}`", cl(&x["file"])), cl(&x["result"]), format!("`{}`", cl(&nd["source"]))]);
            }
            if pts.is_empty() {
                let srcv = if nd.is_dict() { nd["source"].clone() } else { J::Null };
                nrows.push(vec![format!("`{}`", cls(n)), "—".into(), "—".into(), "원장 행 없음".into(), format!("`{}`", cl(&srcv))]);
            }
        }
        out.extend(tb(&["노드", "단계", "파일", "결과", "출처"], nrows));
    }
    let probes = dicts(&a["probes"]);
    if !probes.is_empty() {
        out.extend([String::new(), "**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)".to_string(), String::new()]);
        out.extend(tb(&["순위", "결과", "대상"], probes.iter().map(|x| vec![cl(&x["tier"]), cl(&x["result"]), cla(or(&x["ref"], &x["image"]), "—")]).collect()));
        if let Some(note) = attestation_scope_line(c.facts) {
            if !note.is_empty() && probes.iter().any(|x| x.get_str("tier") == Some("attestation")) {
                out.extend([String::new(), note]);
            }
        }
    }
    let rec = dd(a, "reconstruction");
    let args = dd(rec, "build_args");
    if rec.truthy() {
        out.extend([String::new(), format!("**재구성 입력** — `{}`(docker history build-arg × 패치 자기게이트 규칙)", cl(&rec["source"])), String::new()]);
        if args.truthy() {
            out.extend(tb(&["build-arg", "값"], dict_keys_sorted(args).iter().map(|k| vec![format!("`{k}`"), cla(&args[k.as_str()], "(빈 값)")]).collect()));
        } else {
            out.push("_build-arg 미관측._".into());
        }
    }
    Ok(out.join("\n"))
}

fn markers_text(mk: &J) -> Option<String> {
    if !mk.is_dict() || !mk.truthy() {
        return None;
    }
    let s = format!("{} {}/{}", cl(&mk["status"]), cla(&mk["found"], "0"), cla(&mk["declared"], "0"));
    let miss = &mk["missing"];
    Some(if miss.truthy() { format!("{s} · 부재 {}", cl(miss)) } else { s })
}
pub fn _markers_text(mk: &J) -> HR<Option<String>> {
    Ok(markers_text(mk))
}

fn patch_evidence_table(patches: &[J]) -> Vec<String> {
    let keys = ["evidence", "script_identity", "image_script_sha256", "ledger_result_source"];
    let rows_p: Vec<&J> = patches.iter().filter(|p| keys.iter().any(|k| p[*k].truthy())).collect();
    if rows_p.is_empty() {
        return vec![];
    }
    let mut rows = Vec::new();
    let mut loops: Vec<(String, Vec<J>)> = Vec::new();
    for p in rows_p {
        let ev = dd(p, "evidence");
        let sha = &p["image_script_sha256"];
        let led = &p["ledger_script_sha256"];
        let sha_txt = if sha.truthy() {
            cod(sha)
        } else if led.truthy() {
            format!("{}(빌드 원장 script_sha256 — 이미지 사본 아님)", cod(led))
        } else {
            "—".into()
        };
        rows.push(vec![
            format!("`{}`", cl(&p["file"])),
            cla(&p["script_identity"], "—"),
            sha_txt,
            clo(&markers_text(&ev["markers"]), "—"),
            cla(&ev["static"], "—"),
            cla(&p["ledger_result_source"], "—"),
        ]);
        if ev["loop"].truthy() {
            let ph = ds(&p["phase"]);
            let idx = match loops.iter().position(|(a, _)| *a == ph) {
                Some(i) => i,
                None => {
                    loops.push((ph, vec![]));
                    loops.len() - 1
                }
            };
            if !loops[idx].1.contains(&ev["loop"]) {
                loops[idx].1.push(ev["loop"].clone());
            }
        }
    }
    let mut out = vec![String::new(), "**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)".to_string(), String::new()];
    out.extend(tb(&["파일", "스크립트 바이트 정체", "이미지 사본 sha256", "적용 표지(상태 찾음/선언)", "정적 규칙", "원장 분류"], rows));
    if !loops.is_empty() {
        out.push(String::new());
    }
    loops.sort_by(|a, b| a.0.cmp(&b.0));
    for (ph, xs) in loops {
        out.push(format!("- `{ph}` 빌드 루프: {}", xs.iter().map(cl).collect::<Vec<_>>().join(" · ")));
    }
    out
}
pub fn _patch_evidence_table(patches: &J) -> HR<Vec<String>> {
    Ok(patch_evidence_table(patches.items()))
}

fn candidates(f: &J) -> Vec<J> {
    let raw = &f["value_status_candidates"];
    if let J::Dict(m) = raw {
        let mut items: Vec<(&str, &J)> = m.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        return items
            .into_iter()
            .map(|(k, v)| {
                let mut d = Map::new();
                d.insert("knob", k);
                match v {
                    J::Dict(vm) => d.update(vm),
                    other => d.insert("value", other.clone()),
                }
                J::Dict(d)
            })
            .collect();
    }
    dicts(raw).into_iter().filter(|x| x["knob"].truthy() || x["name"].truthy()).collect()
}
pub fn _candidates(f: &J) -> HR<Vec<J>> {
    Ok(candidates(f))
}
fn knob(c: &J) -> String {
    py_strip(&ds(or(&c["knob"], &c["name"]))).to_string()
}
pub fn _knob(c: &J) -> HR<String> {
    Ok(knob(c))
}

pub fn _f_value_status(c: &_Ctx) -> HR<String> {
    let cands = candidates(c.facts);
    if cands.is_empty() {
        return Ok("_서빙 노브 후보 0개 — 트리플렛 yaml 을 읽지 못했다(결손)._".into());
    }
    let rows = cands
        .iter()
        .map(|x| {
            vec![
                format!("`{}`", knob(x)),
                value_text(&x["value"]).replace('|', "\\|"),
                cla(or(&x["candidate_status"], &x["status"]), "후보 없음"),
                cla(&x["reason"], "—"),
                cla(&x["comment"], "—"),
                cl(&x["source"]),
            ]
        })
        .collect();
    let mut out = vec![
        format!("서빙 노브 {}개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\\|` 는 `|` 로 적는다).", cands.len()),
        String::new(),
    ];
    out.extend(tb(&["노브", "값", "후보 지위", "후보 근거", "yaml 주석", "출처"], rows));
    Ok(out.join("\n"))
}

fn qual_scope_text(q: &J) -> String {
    let sc = &q["scope"];
    if !sc.is_dict() || !sc.truthy() || !sc["timing_note"].truthy() {
        return String::new();
    }
    format!(" — 시점 범위: 작성 {}({}) · 측정 창 대비 {}", cl(&sc["written_utc"]), cl(&sc["written_utc_source"]), cl(&sc["timing_note"]))
}
pub fn _qual_scope_text(q: &J) -> HR<String> {
    Ok(qual_scope_text(q))
}
fn qualification_line(f: &J) -> String {
    let q = dd(f, "qualification");
    let srcs: Vec<String> = if q["sources"].is_list() { q["sources"].items().iter().map(|s| format!("`{}`", ds(s))).collect() } else { vec![] };
    format!(
        "**기동 성공 판정(관측 · 발행 자격)**: health 200 = {} · 추론 1회 = {} · 근거 {}{}",
        cl(&q["health_200"]),
        cl(&q["inference_observed"]),
        cl(&strs_list(srcs)),
        qual_scope_text(q)
    )
}
pub fn _qualification_line(f: &J) -> HR<String> {
    Ok(qualification_line(f))
}

pub fn _f_reproduce(c: &_Ctx) -> HR<String> {
    let steps = dicts(&c.facts["reproduce_steps"]);
    let mut rows = Vec::new();
    let mut cmds: Vec<String> = Vec::new();
    for (i0, s) in steps.iter().enumerate() {
        let i = i0 as i64 + 1;
        let dur = &s["duration"];
        let dur = if ne(dur) || dur.as_str() == Some("unobserved") { J::from(_MISSING_TEXT) } else { dur.clone() };
        let name = cl(or(or(&s["phase"], &s["step"]), &s["name"]));
        let order = cl(or(&s["order"], &J::Int(i)));
        let obs = dd(s, "observed");
        let span = if obs.truthy() && (obs["start_utc"].truthy() || obs["end_utc"].truthy()) {
            format!("{} → {} ({})", cla(&obs["start_utc"], "?"), cla(&obs["end_utc"], "?"), cl(&obs["bound"]))
        } else {
            _MISSING_TEXT.into()
        };
        rows.push(vec![order.clone(), name.clone(), cl(or(&s["success"], &s["success_criterion"])), cl(&dur), span, cl(&s["source"]), cla(&s["command_source"], "—")]);
        let cmd = or(&s["command"], &s["input"]);
        cmds.push(format!("**{order}. {name}**"));
        match cmd {
            J::Str(cs) if !py_strip(cs).is_empty() => {
                let fence = if cs.contains("```") { "````" } else { "```" };
                cmds.push(String::new());
                cmds.push(format!("{fence}sh"));
                cmds.extend(cs.trim_matches('\n').split('\n').map(str::to_string));
                cmds.push(fence.to_string());
                cmds.push(String::new());
            }
            _ => cmds.extend([String::new(), format!("_명령 {_MISSING_TEXT}._"), String::new()]),
        }
    }
    let mut out;
    if !rows.is_empty() {
        out = tb(&["순서", "단계", "성공 판정", "소요", "관측 구간(UTC)", "소요 출처", "명령 출처"], rows);
        let mut spans = Vec::new();
        for s in &steps {
            let ls = dd(dd(s, "observed"), "layer_span");
            if ls.truthy() {
                spans.push(format!(
                    "- `{}` 이미지 층 CreatedAt(docker history): {} → {} · 층 {}개 · 출처 `{}`{}",
                    cl(or(or(&s["phase"], &s["step"]), &s["name"])),
                    cl(&ls["oldest_utc"]),
                    cl(&ls["newest_utc"]),
                    cl(&ls["layers"]),
                    cl(&ls["source"]),
                    if ls["note"].truthy() { format!(" — {}", cl(&ls["note"])) } else { String::new() }
                ));
            }
        }
        if !spans.is_empty() {
            out.extend([String::new(), "**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)".to_string(), String::new()]);
            out.extend(spans);
        }
        let regen = regenerated_env(c.facts);
        if steps.iter().any(|s| ds(or(&s["phase"], &s["step"])) == "render") && !regen.is_empty() {
            out.extend([
                String::new(),
                format!(
                    "> ⚠ `render` 단계는 **지금의** 렌더러로 env 를 다시 만든다 — 측정 뒤 재생성된 env 형상(01 §1.1 싣지 않은 파일 `{_POST_MEASUREMENT}`: {})이 있으니 렌더러가 측정 뒤 바뀌었을 수 있다 — 다시 렌더한 값을 아래 '측정 실행의 env 관측' 표(측정 당시 값)와 대조한다.",
                    btick_join(&regen)
                ),
            ]);
        }
        out.extend([
            String::new(),
            "**단계별 명령**(기계 파생 — `명령 출처`가 `derived(…)`이면 사용법에서 파생하고 `reconstructed(…)`이면 괄호 안에 명시된 관측, 파일 선언·설정 또는 추론을 조합해 재구성한다. 예를 들어 docker history와 bench JSON은 기록된 관측이며 compose 파일은 선언·설정이다. 설정만으로 과거 compose build 실행 여부는 알 수 없다. 어느 명령도 실행 원문(로그에 남은 줄)은 아니다)".to_string(),
            String::new(),
        ]);
        out.extend(cmds);
        while out.last().map(|s| s.is_empty()).unwrap_or(false) {
            out.pop();
        }
    } else {
        out = vec!["_재현 단계 미관측 — 셀 실행 기록이 없다(결손)._".to_string()];
    }
    out.extend(context_map_lines(c.facts));
    out.extend(derived_env_lines(c.facts));
    out.extend([String::new(), qualification_line(c.facts)]);
    Ok(out.join("\n"))
}

fn context_map_lines(f: &J) -> Vec<String> {
    let all = dicts(&f["build_context_map"]);
    let rows: Vec<&J> = all.iter().filter(|r| r["slot_path"].truthy()).collect();
    let mk: Vec<&J> = all.iter().filter(|r| r.get_str("create") == Some("empty-dir")).collect();
    if rows.is_empty() && mk.is_empty() {
        return vec![];
    }
    let both: Vec<&J> = rows.iter().chain(mk.iter()).copied().collect();
    let mut roots: Vec<String> = both.iter().filter(|r| r["context_root"].truthy()).map(|r| ds(&r["context_root"])).collect();
    roots.sort();
    roots.dedup();
    let example = both.iter().any(|r| r.get_str("context_scope") == Some("reconstruction_example"));
    let rj = || roots.iter().map(|x| format!("`{}/`", cls(x))).collect::<Vec<_>>().join(" · ");
    let suffix = if !roots.is_empty() && example {
        format!(" — 재구성 예시 통로 {}(실제 원본·후속층의 빌드 경로 증명 아님 · 별도 실행 기록 확인)", rj())
    } else if !roots.is_empty() {
        format!(" — ⚠ 빌드 레시피 · 빌드 패치 행은 다른 컨텍스트 {}(이 이미지의 레시피 통로 · 2단계 docker build 와 같은 자리 · 자리 칸에 그 뿌리를 붙였다)", rj())
    } else {
        String::new()
    };
    let header = if example {
        "**슬롯 → 재구성 예시 컨텍스트 매핑**(예시 위치 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 설정; 실제 실행 경로는 별도 기록 확인)"
    } else {
        "**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)"
    };
    let mut out = vec![String::new(), format!("{header}{suffix}"), String::new()];
    let place = |r: &J| -> String {
        let cp = if r["context_root"].truthy() { J::Str(format!("{}/{}", ds(&r["context_root"]), ds(&r["context_path"]))) } else { r["context_path"].clone() };
        format!("`{}`", cl(&cp))
    };
    let mut trows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| vec![format!("`{}`", cl(&r["slot_path"])), if r["context_path"].truthy() { place(r) } else { "—".into() }, cl(&r["basis"])])
        .collect();
    trows.extend(mk.iter().map(|r| vec!["(실린 파일 없음 — 빈 디렉터리 생성)".to_string(), place(r), cl(&r["basis"])]));
    out.extend(tb(&["zip 경로", "빌드 컨텍스트 자리", "근거"], trows));
    out
}
pub fn _context_map_lines(f: &J) -> HR<Vec<String>> {
    Ok(context_map_lines(f))
}

fn kv_sorted(d: &Map) -> String {
    let mut items: Vec<(&str, &J)> = d.iter().collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    items.iter().map(|(k, v)| format!("{k}={}", ds(v))).collect::<Vec<_>>().join(" · ")
}
fn derived_rule_text(d: &J) -> Option<String> {
    if let J::Dict(ev) = &d["effective_values"] {
        if !ev.is_empty() {
            let mut items: Vec<(&str, &J)> = ev.iter().collect();
            items.sort_by(|a, b| a.0.cmp(b.0));
            return Some(items.iter().filter_map(|(opt, kv)| kv.as_dict().map(|m| format!("{opt} → {}", kv_sorted(m)))).collect::<Vec<_>>().join(" | "));
        }
    }
    if let J::Dict(vr) = &d["value_rule"] {
        if !vr.is_empty() {
            return Some(kv_sorted(vr));
        }
    }
    None
}
pub fn _derived_rule_text(d: &J) -> HR<Option<String>> {
    Ok(derived_rule_text(d))
}

fn derived_env_lines(f: &J) -> Vec<String> {
    let mut rows = Vec::new();
    for e in iter_or(&f["env_shapes"]) {
        if !e.is_dict() {
            continue;
        }
        for d in dicts(&e["derived"]) {
            let ph = dd(&d, "placeholders");
            let rule = if d["rule_text"].truthy() { d["rule_text"].clone() } else { derived_rule_text(&d).map(J::Str).unwrap_or(J::Null) };
            rows.push(vec![
                format!("`{}`", cl(or(&e["template"], &e["file"]))),
                format!("`{}`", cl(&d["field"])),
                cl(&strs_list(iter_or(&d["keys"]).iter().map(|k| format!("`{}`", ds(k))).collect())),
                cla(&strs_list(dict_keys_sorted(ph).iter().map(|k| format!("`{}`", ds(&ph[k.as_str()]))).collect()), "—"),
                cla(&rule, "규칙 미관측(렌더러에서 읽지 못함 — 지어내지 않는다)"),
                cla(&d["source"], "—"),
            ]);
        }
    }
    if rows.is_empty() {
        return vec![];
    }
    let mut out = vec![String::new(), "**env 형상의 파생 키 · 실효값**(한 manifest 필드 → 여러 키 · 키마다 자기 자리표시 — 값은 필드 값이 아니라 아래 규칙의 결과다)".to_string(), String::new()];
    out.extend(tb(&["형상", "manifest 필드", "키", "자리표시", "실효값 규칙", "출처"], rows));
    out
}
pub fn _derived_env_lines(f: &J) -> HR<Vec<String>> {
    Ok(derived_env_lines(f))
}

fn regenerated_env(f: &J) -> Vec<String> {
    let row = dd(dd(f, "slots"), "compose");
    let mut out: Vec<String> = iter_or(&dd(row, "evidence")["post_measurement_regenerated"])
        .into_iter()
        .filter_map(|x| match x {
            J::Str(s) if !s.is_empty() => Some(s),
            _ => None,
        })
        .collect();
    for x in iter_or(&row["excluded"]) {
        if x.is_dict() && ds(or(or(&x["reason"], &x["why"]), &J::from(""))).starts_with(_POST_MEASUREMENT) {
            let f_ = ds(&x["file"]);
            if !out.iter().any(|y| *y == f_ || y.ends_with(&format!("/{f_}"))) {
                out.push(f_);
            }
        }
    }
    out
}
pub fn _regenerated_env(f: &J) -> HR<Vec<String>> {
    Ok(regenerated_env(f))
}

pub fn _f_sub_recipe(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    if topology(f).as_deref() != Some("multi") {
        return Ok("해당 없음 — single 토폴로지(서브 노드 · Ray worker 없음).".into());
    }
    let sr = &f["sub_recipe"];
    if !sr.is_dict() || !sr.truthy() {
        return Ok(missing_sentence(f, "HINT_MISSING_SUB_RECIPE", "_서브 레시피 없음 — sub_recipe 를 싣지 않았고 결손 코드도 없다(생산자 artifacts 가 말하지 않은 부재)._"));
    }
    let mut out = vec!["원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).".to_string(), String::new()];
    if f.get_str("plane") == Some("native") || sr["_verification"].truthy() {
        let ns = dd(f, "native_source");
        let mut h = vec![
            format!("> **Docker 형 참고**(원천 셀 `{}` · **이 셀 run 아님** · `generated-unverified`) — 아래 master/slave 표 · 서브 전달 env · 기동 순서는 compose 경로의 형상이다. 이 셀(native)은 compose 로 기동되지 않았다 — 실제 기동은 01 §1.4 serve 단계(native 정문 `up`)와 native 기동 기록(`artifacts/triplet/native-launch.md`)이 말한다.", cl(&ns["cell"])),
            String::new(),
        ];
        h.extend(out);
        out = h;
    }
    if missing_codes(&f["missing"]).iter().any(|x| x == "HINT_MISSING_SUB_RECIPE") {
        let mut h = vec![missing_sentence(f, "HINT_MISSING_SUB_RECIPE", ""), String::new()];
        h.extend(out);
        out = h;
    }
    let roles = dd(sr, "role_diff");
    if roles.truthy() {
        out.extend(tb(&["역할", "하는 일"], dict_keys_sorted(roles).iter().map(|k| vec![format!("`{k}`"), cl(&roles[k.as_str()])]).collect()));
    } else {
        out.push("_역할 차이 미관측._".into());
    }
    let order = if sr["launch_order"].is_list() { sr["launch_order"].items().to_vec() } else { vec![] };
    out.extend([String::new(), "**기동 순서**".to_string(), String::new()]);
    if order.is_empty() {
        out.push("_미관측._".into());
    } else {
        out.extend(order.iter().enumerate().map(|(i, s)| format!("{}. {}", i + 1, cl(s))));
    }
    let fw = if sr["env_forward"].is_list() { sr["env_forward"].items().to_vec() } else { vec![] };
    let rows: Vec<Vec<String>> = fw
        .iter()
        .map(|r| {
            if r.is_dict() {
                vec![format!("`{}`", cl(&r["key"])), cl(&r["group"]), cl(or(&r["from"], &r["source"])), cla(&r["why"], "—")]
            } else {
                vec![format!("`{}`", cl(r)), _MISSING_TEXT.into(), _MISSING_TEXT.into(), "—".into()]
            }
        })
        .collect();
    out.extend([String::new(), "**서브 전달 env**(값은 원본 JSON · 여기서는 무리와 이유만)".to_string(), String::new()]);
    if rows.is_empty() {
        out.push("_전달 env 미관측._".into());
    } else {
        out.extend(tb(&["키", "무리", "출처", "이유"], rows));
    }
    let ids = if sr["image_identity_args"].is_list() { sr["image_identity_args"].items().to_vec() } else { vec![] };
    let idt = ids.iter().map(|x| format!("`{}`", cl(x))).collect::<Vec<_>>().join(" · ");
    out.extend([String::new(), format!("**이미지 정체성 build-arg**: {}", if idt.is_empty() { _MISSING_TEXT.to_string() } else { idt })]);
    let pnv = dd(sr, "per_node_values");
    out.extend([String::new(), "**노드별 값(형상)**".to_string(), String::new()]);
    if pnv.truthy() {
        out.extend(tb(&["키", "자리표시"], dict_keys_sorted(pnv).iter().map(|k| vec![format!("`{k}`"), cod(&pnv[k.as_str()])]).collect()));
    } else {
        out.push("_미관측._".into());
    }
    let pre = if sr["preconditions"].is_list() { sr["preconditions"].items().to_vec() } else { vec![] };
    out.extend([String::new(), "**선행조건**".to_string(), String::new()]);
    if pre.is_empty() {
        out.push("_미관측._".into());
    } else {
        out.extend(pre.iter().map(|x| format!("- {}", cl(x))));
    }
    let par = dd(sr, "parity_attestation");
    out.extend([String::new(), "**ABI 동일성 관측**(같아야 하는 것은 digest 가 아니라 ABI — 노드마다 로컬 빌드)".to_string(), String::new()]);
    if par.truthy() {
        out.extend(tb(&["축", "관측"], dict_keys_sorted(par).iter().map(|k| vec![format!("`{k}`"), cl(&par[k.as_str()])]).collect()));
    } else {
        out.push("_미관측._".into());
    }
    if let Some(note) = attestation_scope_line(f) {
        if !note.is_empty() {
            out.extend([String::new(), note]);
        }
    }
    Ok(out.join("\n"))
}

fn env_rows(f: &J) -> Vec<J> {
    dicts(&f["measurement_env_observed"]).into_iter().filter(|r| r["key"].truthy()).collect()
}
pub fn _env_rows(f: &J) -> HR<Vec<J>> {
    Ok(env_rows(f))
}
fn env_value(v: &J) -> String {
    if v.is_null() {
        return _MISSING_TEXT.into();
    }
    if ds(v).is_empty() { "(빈 값)".into() } else { cod(v) }
}
pub fn _env_value(v: &J) -> HR<String> {
    Ok(env_value(v))
}

pub fn _f_measurement_env(c: &_Ctx) -> HR<String> {
    let rows = env_rows(c.facts);
    if rows.is_empty() {
        return Ok("_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._".into());
    }
    let n_kp = rows.iter().filter(|r| r.get_str("kind") == Some("key-present")).count();
    let mut out = vec![
        format!(
            "**측정 실행의 env 관측 {}행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다){}",
            rows.len(),
            if n_kp > 0 { format!(" — 그중 {n_kp}행은 종류 `key-present`: 키가 프로세스 env 에 **있었다**는 관측뿐이고 값은 로그에 없다(값 미관측)") } else { String::new() }
        ),
        String::new(),
    ];
    let seen = |r: &J| -> String {
        let mut parts = Vec::new();
        if r["occurrences"].truthy() {
            parts.push(format!("{}줄", ds(&r["occurrences"])));
        }
        if r["seen_in_logs"].truthy() {
            parts.push(format!("로그 {}개", ds(&r["seen_in_logs"])));
        }
        let rep = ray_folded(r);
        if rep != 0 {
            parts.push(format!("Ray 접힘 +{rep}(접힌 사본의 노드 미관측)"));
        }
        if parts.is_empty() { "—".into() } else { parts.join(" · ") }
    };
    out.extend(tb(
        &["#", "키", "값", "노드", "종류", "출처(첫 줄)", "관측"],
        rows.iter()
            .enumerate()
            .map(|(i, r)| {
                let node = if r["node"].truthy() {
                    r["node"].clone()
                } else if r["node_label"].truthy() {
                    J::Str(format!("미특정({})", ds(&r["node_label"])))
                } else {
                    J::Null
                };
                vec![
                    (i + 1).to_string(),
                    format!("`{}`", cl(&r["key"])),
                    if r.get_str("kind") == Some("key-present") { "(키 존재 · 값 미관측)".into() } else { env_value(&r["value"]) },
                    cla(&node, "미특정"),
                    cla(&r["kind"], "—"),
                    cl(&r["source"]),
                    seen(r),
                ]
            })
            .collect(),
    ));
    let mut notes: Vec<String> = rows.iter().filter(|r| r["value_note"].truthy()).map(|r| ds(&r["value_note"])).collect();
    notes.sort();
    notes.dedup();
    if !notes.is_empty() {
        out.push(String::new());
        out.extend(notes.iter().map(|n| format!("- 값 주: {}", cls(n))));
    }
    if rows.iter().any(|r| ray_folded(r) != 0) {
        out.extend([String::new(), format!("- Ray 접힘: 출처 줄 끝의 `{_RAY_FOLD_TEXT}` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 **첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖).")]);
    }
    let wins = log_windows(&rows, "log_window");
    let heads = log_windows(&rows, "log_head_window");
    let sorted_w = |w: &Vec<(String, Vec<String>)>| {
        let mut v = w.clone();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    if !wins.is_empty() {
        let items = sorted_w(&wins).iter().map(|(w, fs)| format!("`{}`(꼬리 창 출처 로그 {}개 · 이 표 {})", cls(w), fs.len(), row_span(&rows, "log_window", w))).collect::<Vec<_>>().join(" · ");
        out.extend([
            String::new(),
            format!(
                "- 로그 창(꼬리): {items} — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). {}",
                if !heads.is_empty() { "창 앞에서 echo 한 줄은 그 로그에 없다 — 이 주의는 꼬리 창 출처 행에만 걸린다(머리 캡처 행은 아래)." } else { "창 밖에서 echo 한 줄은 이 표에 없다 — 표에 없는 키 · 한 노드에만 있는 키는 창 밖이었을 수 있다." }
            ),
        ]);
    }
    if !heads.is_empty() {
        let items = sorted_w(&heads).iter().map(|(w, fs)| format!("`{}`(머리 캡처 출처 로그 {}개 · 이 표 {})", cls(w), fs.len(), row_span(&rows, "log_head_window", w))).collect::<Vec<_>>().join(" · ");
        out.extend([
            String::new(),
            format!(
                "- 로그 창(머리): {items} — 이 캡처는 엔진 출력의 **처음 N줄**이다(lite_bench.sh 머리 캡처 `head -n N` 상한에 닿았다 · 생산자 evidence `log_head_window`). 꼬리 창의 주의는 이 행에 걸리지 않는다 · 머리 N줄 뒤에서 echo 한 줄은 그 로그에 없다.{}",
                if !wins.is_empty() { " 머리 캡처와 꼬리 창 사이(엔진 출력의 가운데)는 어느 캡처에도 없다 — 표에 없는 키 · 한 노드에만 있는 키는 그 사이였을 수 있다." } else { "" }
            ),
        ]);
    }
    Ok(out.join("\n"))
}

fn row_span(rows: &[J], field: &str, win: &str) -> String {
    let idx: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| py_strip(&ds(or(&r[field], &J::from("")))) == win)
        .map(|(i, _)| i + 1)
        .collect();
    let mut spans: Vec<[usize; 2]> = Vec::new();
    for i in idx {
        match spans.last_mut() {
            Some(s) if i == s[1] + 1 => s[1] = i,
            _ => spans.push([i, i]),
        }
    }
    if spans.is_empty() {
        return "행 없음".into();
    }
    format!("행 {}", spans.iter().map(|[a, b]| if a == b { format!("#{a}") } else { format!("#{a}–{b}") }).collect::<Vec<_>>().join(", "))
}
pub fn _row_span(rows: &J, field: &str, win: &str) -> HR<String> {
    Ok(row_span(rows.items(), field, win))
}

/// window -> unique source logs (insertion order).
fn log_windows(rows: &[J], field: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for r in rows {
        if let J::Str(w) = &r[field] {
            let w = py_strip(w);
            if w.is_empty() {
                continue;
            }
            let src = _LOG_SRC_LINE().sub_literal(&ds(or(&r["source"], &J::from(""))), "");
            let idx = match out.iter().position(|(a, _)| a == w) {
                Some(i) => i,
                None => {
                    out.push((w.to_string(), vec![]));
                    out.len() - 1
                }
            };
            if !out[idx].1.contains(&src) {
                out[idx].1.push(src);
            }
        }
    }
    out
}
/// `_log_windows` — `{window: set(source logs)}` (sets returned sorted).
pub fn _log_windows(rows: &J, field: &str) -> HR<J> {
    let mut m = Map::new();
    for (w, fs) in log_windows(rows.items(), field) {
        m.insert(w, J::from(crate::value::sorted_strs(fs)));
    }
    Ok(J::Dict(m))
}

pub const _RAY_FOLD_TEXT: &str = "[repeated Nx across cluster]";
fn ray_folded(r: &J) -> i64 {
    match r["ray_repeated"] {
        J::Int(v) if v > 0 => v,
        _ => 0,
    }
}
pub fn _ray_folded(r: &J) -> HR<i64> {
    Ok(ray_folded(r))
}

pub fn _f_measurement_env_nodes(c: &_Ctx) -> HR<String> {
    let f = c.facts;
    if topology(f).as_deref() != Some("multi") {
        return Ok("해당 없음 — single 토폴로지의 단일 측정 노드에는 노드 간 비교 대상이 없다.".into());
    }
    let rows = env_rows(f);
    if rows.is_empty() {
        return Ok("_노드별 env 관측 없음 — 01 §1.4 측정 env 관측 표가 비었다._".into());
    }
    let mut by: Vec<(String, Vec<(String, Vec<String>)>)> = Vec::new();
    let mut folded: Vec<(String, i64)> = Vec::new();
    let mut windowed: Vec<(String, Vec<String>)> = Vec::new();
    fn add_set(v: &mut Vec<(String, Vec<String>)>, k: &str, x: &str) {
        match v.iter_mut().find(|(a, _)| a == k) {
            Some((_, s)) => {
                if !s.iter().any(|y| y == x) {
                    s.push(x.to_string())
                }
            }
            None => v.push((k.to_string(), vec![x.to_string()])),
        }
    }
    for r in &rows {
        let key = ds(&r["key"]);
        let one = std::slice::from_ref(r);
        if !log_windows(one, "log_window").is_empty() {
            add_set(&mut windowed, &key, "꼬리");
        }
        if !log_windows(one, "log_head_window").is_empty() {
            add_set(&mut windowed, &key, "머리");
        }
        let node = ds(or(&r["node"], &J::from("미특정")));
        let v = if r.get_str("kind") == Some("key-present") { "(키 존재 · 값 미관측)".to_string() } else { env_value(&r["value"]) };
        let ki = match by.iter().position(|(a, _)| *a == key) {
            Some(i) => i,
            None => {
                by.push((key.clone(), vec![]));
                by.len() - 1
            }
        };
        let nodes = &mut by[ki].1;
        let ni = match nodes.iter().position(|(a, _)| *a == node) {
            Some(i) => i,
            None => {
                nodes.push((node, vec![]));
                nodes.len() - 1
            }
        };
        if !nodes[ni].1.contains(&v) {
            nodes[ni].1.push(v);
        }
        let fr = ray_folded(r);
        match folded.iter_mut().find(|(a, _)| *a == key) {
            Some((_, n)) => *n = (*n).max(fr),
            None => folded.push((key, fr.max(0))),
        }
    }
    by.sort_by(|a, b| a.0.cmp(&b.0));
    let mut same = Vec::new();
    let mut diff = Vec::new();
    for (key, nodes) in &by {
        let get = |n: &str| nodes.iter().find(|(a, _)| a == n).map(|(_, v)| v.clone());
        let main = get("main");
        let sub = get("sub");
        let eq = match (&main, &sub) {
            (Some(a), Some(b)) => {
                let (mut a, mut b) = (a.clone(), b.clone());
                a.sort();
                b.sort();
                a == b
            }
            _ => false,
        };
        if eq && nodes.len() == 2 {
            same.push(key.clone());
            continue;
        }
        let verdict = if main.is_some() && sub.is_some() {
            if eq { "같음".to_string() } else { "다름".to_string() }
        } else if main.is_some() || sub.is_some() {
            let one = if main.is_some() { "main" } else { "sub" };
            let other = if one == "main" { "sub" } else { "main" };
            let fk = folded.iter().find(|(a, _)| a == key).map(|(_, n)| *n).unwrap_or(0);
            if fk != 0 {
                format!("{one} 만 줄에 남음 · Ray 접힘 +{fk} — {other} 사본 여부 미관측")
            } else if get("미특정").is_some() {
                format!("{one} 관측 · 노드 미특정 줄 있음 — {other} 여부 미관측")
            } else if let Some((_, ws)) = windowed.iter().find(|(a, _)| a == key) {
                let mut ws = ws.clone();
                ws.sort();
                ws.reverse();
                format!("{one} 만 캡처 창에 남음 — {other} 여부 미관측({} 캡처)", ws.join("·"))
            } else {
                format!("{one} 만 관측")
            }
        } else {
            "노드 미특정".to_string()
        };
        let j = |n: &str| {
            let s = get(n).unwrap_or_default().join(" · ");
            if s.is_empty() { "—".to_string() } else { s }
        };
        diff.push(vec![format!("`{key}`"), j("main"), j("sub"), j("미특정"), verdict]);
    }
    let mut out = vec![
        format!("**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 키(`{_RAY_FOLD_TEXT}`)와 머리·꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)"),
        String::new(),
    ];
    out.push(format!("- 양 노드 같은 값 {}개: {}", same.len(), if same.is_empty() { "없음".to_string() } else { btick_join(&same) }));
    if !diff.is_empty() {
        out.push(String::new());
        out.extend(tb(&["키", "main", "sub", "노드 미특정", "판정"], diff));
    }
    Ok(out.join("\n"))
}

pub fn _f_tool_snapshots(c: &_Ctx) -> HR<String> {
    let rows: Vec<J> = dicts(&c.facts["tool_snapshots"]).into_iter().filter(|x| x["name"].truthy()).collect();
    if rows.is_empty() {
        return Ok("_측정 도구 원문 스냅샷 없음 — 측정 시각 이전 커밋의 도구 원문을 고정하지 못했다(evidence.tool_snapshots) · 도구가 정한 인자는 원문 없이 단정하지 않는다._".into());
    }
    let mut out = vec![
        format!("**측정 도구 원문 {}건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)", rows.len()),
        String::new(),
    ];
    let rev = |sha: &J, utc: &J| -> String {
        if !sha.truthy() {
            return "관측 없음".into();
        }
        cod(&J::Str(core::head_chars(&ds(sha), 12))) + &if utc.truthy() { format!(" {}", cl(utc)) } else { String::new() }
    };
    let basis = |x: &J| -> Option<String> {
        match &x["next_rev_basis"] {
            J::Str(b) if !py_strip(b).is_empty() => Some(py_strip(b).to_string()),
            _ => None,
        }
    };
    let nxt = |x: &J| -> String {
        if x["next_rev"].truthy() {
            return rev(&x["next_rev"], &x["next_rev_utc"]);
        }
        match basis(x) {
            Some(b) => cls(&b),
            None => "관측 없음".into(),
        }
    };
    out.extend(tb(
        &["도구", "역할", "저장소 경로", "리비전(커밋 UTC)", "다음 변경", "발췌 출처 토큰", "다시 얻기"],
        rows.iter()
            .map(|x| {
                vec![
                    format!("`{}`", cl(&x["name"])),
                    cla(&x["role"], "—"),
                    format!("`{}`", cl(&x["repo_path"])),
                    rev(&x["git_rev"], &x["git_rev_utc"]),
                    nxt(x),
                    cod(&if x["snapshot_rel"].truthy() { J::Str(pname(&ds(&x["snapshot_rel"]))) } else { J::Null }),
                    cod(&if x["git_rev"].truthy() && x["repo_path"].truthy() { J::Str(format!("git show {}:{}", ds(&x["git_rev"]), ds(&x["repo_path"]))) } else { J::Null }),
                ]
            })
            .collect(),
    ));
    let mut whys: Vec<String> = rows.iter().filter(|x| x["why"].truthy()).map(|x| format!("- `{}` — {}", cl(&x["name"]), cl(&x["why"]))).collect();
    whys.push("- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).".into());
    if rows.iter().any(|x| !x["next_rev"].truthy() && basis(x).is_none()) {
        whys.push("- 다음 변경 `관측 없음` = 측정 시점 커밋 뒤 그 파일을 바꾼 커밋을 찾지 못했다(지금 HEAD 가 그 커밋의 후손이 아니면 찾지 않는다) — '도구가 바뀌지 않았다' 는 뜻이 아니다.".into());
    }
    let mut notes: Vec<String> = Vec::new();
    for x in &rows {
        for k in ["driver_note", "worktree_note"] {
            if let J::Str(v) = &x[k] {
                if !v.is_empty() && !notes.contains(v) {
                    notes.push(v.clone());
                }
            }
        }
    }
    out.push(String::new());
    out.extend(whys);
    out.extend(notes.iter().map(|n| format!("- {}", cls(n))));
    Ok(out.join("\n"))
}

pub const _ATTEMPT_OPEN: [&str; 1] = ["budget_declare"];
pub const _ATTEMPT_RENEW: [&str; 1] = ["budget_renew"];
pub const _ATTEMPT_CLOSE: [&str; 2] = ["budget_clear", "budget_expired"];
pub const _ATTEMPT_ALERT: [&str; 8] = ["watchdog_trip", "watchdog_kill_ack", "thermal_trip", "thermal_kill_ack", "earlyoom_kill", "budget_declare_rejected", "budget_rejected", "budget_blocked"];
pub const _ATTEMPT_NOT_EVENT: [&str; 1] = ["measurement"];
pub const _LABEL_SCOPE_OTHER: &str = "other-label-same-node";

fn timeline_rows(f: &J) -> Vec<J> {
    dicts(&f["event_timeline"])
}
pub fn _timeline_rows(f: &J) -> HR<Vec<J>> {
    Ok(timeline_rows(f))
}

/// One start attempt (the Python dict of `_attempts`).
#[derive(Clone)]
struct Attempt {
    node: J,
    label: J,
    declare: J,
    renew: Vec<J>,
    alerts: Vec<(String, i64)>,
    end: Option<String>,
    end_utc: J,
    other: i64,
    scope: J,
    other_label: bool,
}
impl Attempt {
    fn to_j(&self) -> J {
        let mut al = Map::new();
        for (k, n) in &self.alerts {
            al.insert(k.as_str(), *n);
        }
        jd! {
            "node" => self.node.clone(), "label" => self.label.clone(), "declare" => self.declare.clone(),
            "renew" => J::List(self.renew.clone()), "alerts" => J::Dict(al),
            "end" => self.end.clone().map(J::Str).unwrap_or(J::Null), "end_utc" => self.end_utc.clone(),
            "other" => self.other, "scope" => self.scope.clone(), "other_label" => self.other_label,
        }
    }
}

fn attempts(rows: &[J]) -> Vec<Attempt> {
    let mut out: Vec<Attempt> = Vec::new();
    let mut cur: Vec<(String, usize)> = Vec::new();
    let mut last: Option<usize> = None;
    for r in rows {
        let key = ds(&r["node"]);
        let kind = ds(or(&r["kind"], &J::from("")));
        if _ATTEMPT_NOT_EVENT.contains(&kind.as_str()) {
            continue;
        }
        let opens = _ATTEMPT_OPEN.contains(&kind.as_str());
        let mut a = if opens { None } else { cur.iter().find(|(k, _)| *k == key).map(|(_, i)| *i).or(last) };
        if a.is_none() {
            out.push(Attempt {
                node: r["node"].clone(),
                label: r["label"].clone(),
                declare: if opens { r["utc"].clone() } else { J::Null },
                renew: vec![],
                alerts: vec![],
                end: None,
                end_utc: J::Null,
                other: 0,
                scope: r["campaign_scope"].clone(),
                other_label: r.get_str("label_scope") == Some(_LABEL_SCOPE_OTHER),
            });
            let i = out.len() - 1;
            match cur.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 = i,
                None => cur.push((key.clone(), i)),
            }
            last = Some(i);
            a = Some(i);
            if opens {
                continue;
            }
        }
        let at = &mut out[a.unwrap()];
        if _ATTEMPT_RENEW.contains(&kind.as_str()) {
            at.renew.push(r["utc"].clone());
        } else if _ATTEMPT_CLOSE.contains(&kind.as_str()) {
            at.end = Some(kind.clone());
            at.end_utc = r["utc"].clone();
        } else if _ATTEMPT_ALERT.contains(&kind.as_str()) {
            match at.alerts.iter_mut().find(|(k, _)| *k == kind) {
                Some(e) => e.1 += 1,
                None => at.alerts.push((kind.clone(), 1)),
            }
        } else {
            at.other += 1;
        }
    }
    out
}
pub fn _attempts(rows: &J) -> HR<Vec<J>> {
    Ok(attempts(rows.items()).iter().map(Attempt::to_j).collect())
}

fn attempt_label(a: &Attempt) -> String {
    if !a.label.truthy() {
        return "—".into();
    }
    format!("`{}`{}", cl(&a.label), if a.other_label { "(라벨 다름 — 같은 노드 · 이 셀 라벨 아님)" } else { "" })
}
fn attempt_from_j(a: &J) -> Attempt {
    Attempt {
        node: a["node"].clone(),
        label: a["label"].clone(),
        declare: a["declare"].clone(),
        renew: a["renew"].items().to_vec(),
        alerts: a["alerts"].as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.as_i64().unwrap_or(0))).collect()).unwrap_or_default(),
        end: if a["end"].truthy() { Some(ds(&a["end"])) } else { None },
        end_utc: a["end_utc"].clone(),
        other: a["other"].as_i64().unwrap_or(0),
        scope: a["scope"].clone(),
        other_label: a["other_label"].truthy(),
    }
}
pub fn _attempt_label(a: &J) -> HR<String> {
    Ok(attempt_label(&attempt_from_j(a)))
}

fn attempts_rows(att: &[Attempt], prefix: &str) -> Vec<String> {
    tb(
        &["시도", "노드", "label", "선언(UTC)", "갱신(renew)", "사살 · 트립 · 거부", "닫힘", "그 밖 행"],
        att.iter()
            .enumerate()
            .map(|(i, a)| {
                let mut al = a.alerts.clone();
                al.sort_by(|x, y| x.0.cmp(&y.0));
                vec![
                    format!("{prefix}{}", i + 1),
                    cl(&a.node),
                    attempt_label(a),
                    cla(&a.declare, "—(선언 행 없음)"),
                    if a.renew.is_empty() { "없음".into() } else { format!("{}회 · 첫 {}", a.renew.len(), cl(&a.renew[0])) },
                    if al.is_empty() { "없음".into() } else { al.iter().map(|(k, n)| format!("`{k}` {n}")).collect::<Vec<_>>().join(" · ") },
                    match &a.end {
                        Some(e) => format!("`{e}` {}", cl(&a.end_utc)),
                        None => "—(닫힘 행 없음)".into(),
                    },
                    a.other.to_string(),
                ]
            })
            .collect(),
    )
}
pub fn _attempts_rows(att: &J, prefix: &str) -> HR<Vec<String>> {
    Ok(attempts_rows(&att.items().iter().map(attempt_from_j).collect::<Vec<_>>(), prefix))
}

fn boundary_line(bnd: &J) -> Vec<String> {
    if !bnd.is_dict() {
        return vec![];
    }
    if !bnd["utc"].truthy() {
        return vec![String::new(), format!("> **캠페인 경계 미관측** — {}", cl(&bnd["source"]))];
    }
    vec![String::new(), format!("> **캠페인 경계** {}(`{}` · evidence.event_campaign_boundary): {}", cl(&bnd["utc"]), cl(&bnd["campaign_id"]), cl(&bnd["source"]))]
}
pub fn _boundary_line(bnd: &J) -> HR<Vec<String>> {
    Ok(boundary_line(bnd))
}

fn attempts_table(rows: &[J], bnd: &J) -> Vec<String> {
    let att = attempts(rows);
    if att.is_empty() {
        return vec!["_기동 시도 미관측 — 이 셀 label 의 원장 행이 없다._".into()];
    }
    if !(bnd.is_dict() && bnd["utc"].truthy()) {
        return attempts_rows(&att, "");
    }
    let prior: Vec<Attempt> = att.iter().filter(|a| a.scope.as_str() == Some("prior-campaign")).cloned().collect();
    let this: Vec<Attempt> = att.iter().filter(|a| a.scope.as_str() != Some("prior-campaign")).cloned().collect();
    let n_other = prior.iter().filter(|a| a.other_label).count();
    let n_same = prior.len() - n_other;
    let mut out = if !this.is_empty() {
        attempts_rows(&this, "")
    } else {
        vec![format!(
            "_이번 캠페인(경계 {} 이후)의 기동 시도 미관측 — 아래는 {}",
            cl(&bnd["utc"]),
            if n_other == 0 { "같은 셀 이름의 앞선 캠페인 행뿐이다._" } else { "경계 이전 행뿐이다._" }
        )]
    };
    if !prior.is_empty() && n_other == 0 {
        out.extend([
            String::new(),
            format!("**같은 셀 이름 · 앞선 캠페인의 기동 {}회**(캠페인 경계 {} 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)", prior.len(), cl(&bnd["utc"])),
            String::new(),
        ]);
    } else if !prior.is_empty() {
        let mut parts = Vec::new();
        if n_same > 0 {
            parts.push(format!("이 셀 라벨로 묶인 {n_same}회"));
        }
        parts.push(format!("라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) {n_other}회"));
        out.extend([
            String::new(),
            format!("**캠페인 경계 이전 기동 {}회**(캠페인 경계 {} 이전 — {} · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)", prior.len(), cl(&bnd["utc"]), parts.join(" · ")),
            String::new(),
        ]);
    }
    if !prior.is_empty() {
        out.extend(attempts_rows(&prior, "앞선-"));
    }
    out
}
pub fn _attempts_table(rows: &J, bnd: &J) -> HR<Vec<String>> {
    Ok(attempts_table(rows.items(), bnd))
}

fn ledger_span_lines(f: &J) -> Vec<String> {
    let spans = dicts(&f["event_ledger_spans"]);
    if spans.is_empty() {
        return vec![];
    }
    let mut parts = Vec::new();
    let mut outs = Vec::new();
    let mut locs = Vec::new();
    for x in &spans {
        let files = iter_or(&x["files"]).iter().map(|p| format!("`{}`", cl(p))).collect::<Vec<_>>().join(" · ");
        if !x["last_utc"].truthy() {
            parts.push(format!("{} = 원장 없음(이 노드의 사건은 관측 대상 밖)", cl(&x["node"])));
            outs.push(ds(&x["node"]));
            continue;
        }
        let sh: Vec<J> = iter_or(&x["shadowed_local_copies"]).into_iter().filter(J::truthy).collect();
        parts.push(format!(
            "{} = {} → {}({}{})",
            cl(&x["node"]),
            cl(&x["first_utc"]),
            cl(&x["last_utc"]),
            files,
            if sh.is_empty() { String::new() } else { format!(" · 회수 미러가 가린 옛 로컬 사본 {}", sh.iter().map(|p| format!("`{}`", cl(p))).collect::<Vec<_>>().join(" · ")) }
        ));
        if x["covers_measurement"] == J::Bool(false) {
            let s = format!("{}(마지막 행 {} < 측정 끝 {})", ds(&x["node"]), ds(&x["last_utc"]), ds(&x["measurement_end_utc"]));
            if x["live_local"] == J::Bool(true) {
                locs.push(s);
            } else {
                outs.push(s);
            }
        }
    }
    let mut line = format!("> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): {}", parts.join(" · "));
    let echoed: Vec<&J> = spans
        .iter()
        .filter(|x| x["covers_measurement"] != J::Bool(true) && x["smoke_echo"].is_dict() && x["smoke_echo"]["rows"].truthy())
        .collect();
    if !outs.is_empty() {
        line += &format!(
            " — ⚠ {}: 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수){}",
            outs.iter().map(|o| cls(o)).collect::<Vec<_>>().join(" · "),
            if echoed.is_empty() { "" } else { " — 단 아래 보조 관측의 행은 예외" }
        );
    }
    if !locs.is_empty() {
        line += &format!(
            " — {}: 마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님 — 행은 사건 · 6시간 갱신 때만 쓰인다)",
            locs.iter().map(|o| cls(o)).collect::<Vec<_>>().join(" · ")
        );
    }
    let mut out = vec![String::new(), line];
    for x in echoed {
        let e = &x["smoke_echo"];
        let rows = dicts(&e["rows"]);
        let mut kinds: Vec<String> = rows.iter().map(|r| ds(&r["kind"])).collect();
        kinds.sort();
        kinds.dedup();
        let items = rows
            .iter()
            .map(|r| {
                format!(
                    "`{}` {}{} — `{}`",
                    cl(&r["kind"]),
                    cl(&r["ts"]),
                    if r["label"].truthy() { format!("(`{}`)", cl(&r["label"])) } else { String::new() },
                    cl(&r["source"])
                )
            })
            .collect::<Vec<_>>()
            .join(" · ");
        let n = cl(&x["node"]);
        out.push(">".into());
        out.push(format!(
            "> **보조 관측 · {n}**({n} 원장 미러 밖 — 스모크 로그 echo · {}): {items} — 이 기동에서 {n} 에 대해 관측된 것은 {} 뿐이다 · 그 밖의 {n} 사건(사살 · 트립 · 갱신 · clear)은 여전히 **관측 범위 밖**이다(없음이 아니다)",
            cl(&e["basis"]),
            btick_join(&kinds)
        ));
    }
    out
}
pub fn _ledger_span_lines(f: &J) -> HR<Vec<String>> {
    Ok(ledger_span_lines(f))
}

pub fn _f_event_timeline(c: &_Ctx) -> HR<String> {
    let rows = timeline_rows(c.facts);
    if rows.is_empty() {
        let mut v = vec!["_블랙박스 이벤트 미관측 — 이 셀 label·config 에 맞는 원장 행이 없다(원장 파일이 없는 것과 파일은 있으나 이 셀 행이 0 인 것을 이 표는 구분하지 않는다 — evidence.event_timeline 이 둘 다 빈 목록으로 준다)._".to_string()];
        v.extend(ledger_span_lines(c.facts));
        return Ok(v.join("\n"));
    }
    let bnd = &c.facts["event_campaign_boundary"];
    let n_prior = rows.iter().filter(|r| r.get_str("campaign_scope") == Some("prior-campaign")).count();
    let n_same = rows.iter().filter(|r| r.get_str("campaign_scope") == Some("same-cell-pre-boundary")).count();
    let split = if bnd.is_dict() && bnd["utc"].truthy() && (n_prior > 0 || n_same > 0) {
        format!(
            " · 이번 캠페인 {}행{}{}(내용 칸에 표기)",
            rows.len() - n_prior - n_same,
            if n_same > 0 { format!(" · 캠페인 선언 전 · 같은 셀 기동 {n_same}행(이 셀의 시도로 센다)") } else { String::new() },
            if n_prior > 0 { format!(" · 같은 셀 이름 · 앞선 캠페인 {n_prior}행") } else { String::new() }
        )
    } else {
        String::new()
    };
    let n_other = rows.iter().filter(|r| r.get_str("label_scope") == Some(_LABEL_SCOPE_OTHER)).count();
    let other = if n_other > 0 {
        format!(" · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) {n_other}행 — 셀 라벨 일치 집계에서 제외(세션·호출 기록의 귀속 확인과 별도)")
    } else {
        String::new()
    };
    let mut out = vec![format!(
        "**블랙박스 이벤트 {}행**(원장에서 이 셀 label·config 에 맞는 행{} · 시각순 · 기계 발췌{split}{other})",
        rows.len(),
        if n_other > 0 { "과 그와 시간이 겹치는 같은 노드의 다른 라벨 창" } else { "만" }
    )];
    out.extend(boundary_line(bnd));
    out.extend(ledger_span_lines(c.facts));
    out.push(String::new());
    out.extend(tb(
        &["#", "UTC", "노드", "kind", "label", "내용", "출처"],
        rows.iter()
            .enumerate()
            .map(|(i, r)| {
                vec![
                    (i + 1).to_string(),
                    cl(&r["utc"]),
                    cl(&r["node"]),
                    format!("`{}`", cl(&r["kind"])),
                    if r["label"].truthy() { format!("`{}`", cl(&r["label"])) } else { "—".into() },
                    cla(&r["detail"], "—"),
                    cl(&r["source"]),
                ]
            })
            .collect(),
    ));
    out.extend([
        String::new(),
        "**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)".to_string(),
        String::new(),
    ]);
    out.extend(attempts_table(&rows, bnd));
    Ok(out.join("\n"))
}

pub fn _f_event_attempts(c: &_Ctx) -> HR<String> {
    let rows = timeline_rows(c.facts);
    if rows.is_empty() {
        return Ok("_기동 시도 미관측 — 블랙박스 원장에 이 셀 label 의 행이 없다(01 §1.4)._".into());
    }
    let bnd = &c.facts["event_campaign_boundary"];
    let mut out = vec!["**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)".to_string()];
    out.extend(boundary_line(bnd));
    out.extend(ledger_span_lines(c.facts));
    out.push(String::new());
    out.extend(attempts_table(&rows, bnd));
    Ok(out.join("\n"))
}

pub fn _f_bench_definition(c: &_Ctx) -> HR<String> {
    let bd = dd(c.facts, "bench_definition");
    if !bd.truthy() {
        return Ok("_현행 full 정의 미관측 — evidence.bench_definition 결과가 없다._".into());
    }
    let sent = py_strip(&ds(or(&bd["current_full_definition"], &J::from("")))).to_string();
    let mut out = vec![format!("**현행 full 정의**(`{}` 원문 그대로):", bench_definition_source(bd)), String::new()];
    let mut lines = py_splitlines(&sent);
    if lines.is_empty() {
        lines.push(_MISSING_TEXT.into());
    }
    out.extend(lines.iter().map(|ln| format!("> {ln}")));
    let mut rows = vec![row(["이 측정의 반복 수".into(), cl(&bd["this_repeats"]), cl(&bd["repeats_source"])])];
    let has = |k: &str| bd.as_dict().map(|m| m.contains_key(k)).unwrap_or(false);
    if has("required_repeats") {
        rows.push(row(["현행 정의의 반복 요건".into(), cl(&bd["required_repeats"]), "위 정의 문장".into()]));
    }
    if has("bench_tool") {
        rows.push(row(["이 측정의 도구".into(), cl(&bd["bench_tool"]), cl(&bd["bench_tool_source"])]));
    }
    let reasons: Vec<String> = iter_or(&bd["reasons"]).into_iter().filter_map(|x| match x {
        J::Str(s) if !s.is_empty() => Some(s),
        _ => None,
    }).collect();
    rows.push(row([
        "현행 full 정의 충족".into(),
        yes_no(&bd["meets_current_full"]),
        if reasons.is_empty() { "evidence.bench_definition(반복 수 × 현행 정의)".into() } else { reasons.join(" · ") },
    ]));
    out.push(String::new());
    out.extend(tb(&["항목", "값", "출처 · 사유"], rows));
    Ok(out.join("\n"))
}

pub fn _f_lineage(c: &_Ctx) -> HR<String> {
    let items = dicts(&c.facts["lineage_reading_list"]);
    if items.is_empty() {
        return Ok("_계보 문서 0건 — 서사 원재료가 없다(린터가 `HINT_LINEAGE_EMPTY` 로 막는다)._".into());
    }
    let rows = items
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let doc = format!("`{}`{}", cl(&x["path"]), if x.get_str("tier") == Some("ambiguous") { " (모호 해소)" } else { "" });
            vec![(i + 1).to_string(), doc, cl(&x["kind"]), cl(&x["date_key"]), cl(&x["depth"]), cl(&x["bytes"]), cla(&x["axis_tokens"], "—"), cla(&x["superseded_by"], "—")]
        })
        .collect();
    let mut out = vec![
        format!("계보 문서 {}건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).", items.len()),
        String::new(),
    ];
    out.extend(tb(&["#", "문서", "종류", "날짜", "깊이", "크기", "셀 축 토큰", "대체됨"], rows));
    Ok(out.join("\n"))
}

pub const _LEVEL_KEY_ORDER: [&str; 2] = ["level", "status"];

pub fn _f_measurement(c: &_Ctx) -> HR<String> {
    let mut out = _measurement_table(c)?;
    out.extend(marker_lines(c.facts));
    Ok(out.join("\n"))
}

pub fn _measurement_table(c: &_Ctx) -> HR<Vec<String>> {
    let m = dd(c.facts, "measurement");
    if !m.truthy() {
        return Ok(vec!["_측정값 없음(미기재 — 0 이 아니다)._".into()]);
    }
    let md = m.as_dict().unwrap();
    let per = dd(m, "sources");
    let mut keys: Vec<String> = if md.contains_key("measured_utc") { vec!["measured_utc".into()] } else { vec![] };
    let mut rest: Vec<String> = md.keys().filter(|k| !["measured_utc", "source", "sources"].contains(k)).map(str::to_string).collect();
    rest.sort();
    keys.extend(rest);
    if md.contains_key("source") {
        keys.push("source".into());
    }
    let mut rows = Vec::new();
    let mut subtables = Vec::new();
    for k in keys {
        let v = &m[k.as_str()];
        match v {
            J::Dict(vm) if !vm.is_empty() && vm.values().all(|x| !x.is_dict() && !x.is_list()) => {
                for sk in dict_keys_sorted(v) {
                    rows.push(vec![format!("`{k}.{sk}`"), cla(&v[sk.as_str()], _UNRECORDED), cla(or(&per[format!("{k}.{sk}").as_str()], &per[k.as_str()]), "—")]);
                }
            }
            J::List(l) if !l.is_empty() && l.iter().all(J::is_dict) => {
                let mut cols: Vec<String> = _LEVEL_KEY_ORDER.iter().filter(|x| l.iter().any(|r| r.as_dict().unwrap().contains_key(x))).map(|s| s.to_string()).collect();
                let mut extra: Vec<String> = l.iter().flat_map(|r| r.as_dict().unwrap().keys().map(str::to_string).collect::<Vec<_>>()).filter(|x| !cols.contains(x)).collect();
                extra.sort();
                extra.dedup();
                cols.extend(extra);
                subtables.extend([String::new(), format!("**`{k}`**({}행 · 출처 {})", l.len(), cla(&per[k.as_str()], "—")), String::new()]);
                let hs: Vec<String> = cols.iter().map(|x| format!("`{x}`")).collect();
                let hr: Vec<&str> = hs.iter().map(String::as_str).collect();
                subtables.extend(tb(&hr, l.iter().map(|r| cols.iter().map(|x| cla(&r[x.as_str()], _UNRECORDED)).collect()).collect()));
            }
            _ => rows.push(vec![format!("`{k}`"), cla(v, _UNRECORDED), cla(&per[k.as_str()], "—")]),
        }
    }
    let mut out = tb(&["키", "값", "출처"], rows);
    out.extend(subtables);
    Ok(out)
}

/// `_bench_module(repo)` — the render_bench_section owner is compiled into this crate (`crate::render_bench_section`);
/// there is nothing to load, so this returns `None`. Callers use `crate::render_bench_section::*` directly.
pub fn _bench_module(repo: &Path) -> HR<J> {
    let _ = repo;
    Ok(J::Null)
}

pub fn _f_measurement_config(c: &_Ctx) -> HR<String> {
    let mc = dd(c.facts, "measurement_config");
    if !mc.truthy() {
        return Ok("_측정 구성 미기재 — 바인딩된 리포트에 측정 구성 표가 없다(부재는 미기재다 · lite 로 접지 않는다)._".into());
    }
    let order = crate::render_bench_section::MEASUREMENT_CONFIG_KEYS;
    let md = mc.as_dict().unwrap();
    let mut keys: Vec<String> = order.iter().filter(|k| md.contains_key(k)).map(|s| s.to_string()).collect();
    let mut rest: Vec<String> = md.keys().filter(|k| !order.contains(k)).map(str::to_string).collect();
    rest.sort();
    keys.extend(rest);
    Ok(tb(&["키", "값"], keys.iter().map(|k| vec![format!("`{k}`"), cla(&mc[k.as_str()], _UNRECORDED)]).collect()).join("\n"))
}

pub fn _f_dataset_provenance(c: &_Ctx) -> HR<String> {
    let dp = dd(c.facts, "dataset_provenance");
    let mut out = vec!["### 데이터셋 출처".to_string(), String::new()];
    if !dp.truthy() {
        out.push("_데이터셋 출처 미관측 — facts 에 dataset_provenance 가 없다(생산자 evidence.dataset_provenance · 이 판 이전 draft 면 `hint.sh refacts` 로 재파생)._".into());
        return Ok(out.join("\n"));
    }
    if dp.get_str("status") == Some(crate::evidence::DATASET_STATUS_INFERRED) {
        out.extend(["> 아래 `synthetic` 은 **부재에서 추론한** 값이다 — 리포트 측정 구성 표 · 인증서 어디에도 데이터셋 기재가 없는 측정(데이터셋 축 도입 전)이며, 소급 기재하지 않았다.".to_string(), String::new()]);
    } else {
        out.extend(["> 리포트 측정 구성 표 · 인증서 소프트 지문이 **기재한** 데이터셋 출처다(파싱만 · 합성 ✗ · `/app/` 밖 호스트 경로는 이름만).".to_string(), String::new()]);
    }
    let mut rows: Vec<Vec<String>> = crate::evidence::DATASET_KEYS.iter().chain(["dataset_provenance"].iter()).map(|k| vec![format!("`{k}`"), cla(&dp[*k], _UNRECORDED)]).collect();
    rows.push(vec!["`status`".into(), cl(&dp["status"])]);
    rows.push(vec!["`source`".into(), cl(&dp["source"])]);
    out.extend(tb(&["키", "값"], rows));
    Ok(out.join("\n"))
}

pub fn _f_lite_summary(c: &_Ctx) -> HR<String> {
    if !lite(c.facts) {
        return Ok(String::new());
    }
    let mut out = vec![
        format!("### lite 측정 — `{LITE_LABEL}` · 성능 판정 아님"),
        String::new(),
        format!("> lite 셀은 `{BENCH_DOC}` 를 싣지 않는다 — lite 벤치 1회의 수치 · 측정 구성 · 측정 도구 원문을 여기 둔다(파싱만 · 재계산 ✗). 이 HW 에서 기동 · 추론이 된다는 최소 증명이며 baseline · 권고가 아니다."),
        String::new(),
    ];
    out.extend(_measurement_table(c)?);
    out.extend([String::new(), "**측정 구성**".to_string(), String::new(), _f_measurement_config(c)?, String::new(), "**측정 도구 원문**".to_string(), String::new(), _f_tool_snapshots(c)?]);
    Ok(out.join("\n"))
}

pub fn _f_bench_section(c: &_Ctx) -> HR<String> {
    if let J::Str(md) = &c.facts["bench_section_md"] {
        if !py_strip(md).is_empty() {
            return Ok(md.trim_matches('\n').to_string());
        }
    }
    Ok("_부하 곡선 없음 — 바인딩된 리포트 · 스윕 색인이 없다. 이 레시피의 부하 거동은 이 hint 로 알 수 없다(부재를 성능 판정으로 읽지 말 것)._".into())
}

/// A FACT renderer (`Callable[[_Ctx], str]`).
pub type FactRenderer = for<'a, 'b> fn(&'a _Ctx<'b>) -> HR<String>;

/// `FACT_RENDERERS` — fact id -> renderer, in the Python dict order.
pub fn FACT_RENDERERS() -> &'static [(&'static str, FactRenderer)] {
    &[
        ("header", _f_header),
        ("grade", _f_grade),
        ("context", _f_context),
        ("resolved", _f_resolved),
        ("meta", _f_meta),
        ("missing", _f_missing),
        ("legend", _f_legend),
        ("slots", _f_slots),
        ("applied_set", _f_applied_set),
        ("value_status", _f_value_status),
        ("reproduce", _f_reproduce),
        ("sub_recipe", _f_sub_recipe),
        ("lineage", _f_lineage),
        ("measurement", _f_measurement),
        ("measurement_config", _f_measurement_config),
        (BENCH_REGION_ID, _f_bench_section),
        ("bench_missing", _f_bench_missing),
        ("event_timeline", _f_event_timeline),
        ("event_attempts", _f_event_attempts),
        ("bench_definition", _f_bench_definition),
        ("measurement_env", _f_measurement_env),
        ("measurement_env_nodes", _f_measurement_env_nodes),
        ("tool_snapshots", _f_tool_snapshots),
        ("name_tail", _f_name_tail),
        ("dataset_provenance", _f_dataset_provenance),
        ("lite_summary", _f_lite_summary),
    ]
}
