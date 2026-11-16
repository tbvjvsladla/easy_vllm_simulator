//! Port of `scripts/render_bench_section.py` — the hint 03-benchmark section renderer and the bench_report reader
//! (sweep table · measurement-config table · lite metrics) shared by hint_tag/hint_collect and adversarial-benchmark.
//!
//! The writer of the tables read here is adversarial-benchmark `render_report` (its strings are the contract).
//! `BenchSectionFailure` is `HintError::py("BenchSectionFailure", msg)`; Python built-ins the original lets through keep
//! their type names (`ValueError`, `OSError` subclasses, `KeyError`, `AttributeError`, `TypeError`, `OverflowError`).
use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::{HintError, HR};
use crate::io;
use crate::value::{py_repr, Map, J};

pub const SECTION_TITLE: &str = "## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)";
pub const LEGACY_CAPTION: &str = "> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).";
pub const CURRENT_CAPTION: &str = "> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.\n> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.";
pub const REPORT_HEADER: &str = "| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |";
pub const REPORT_TRUNC_MARK: &str = "**⚠ 절삭된 부하 레벨";
pub const REPORT_NO_TRUNC: &str = "_절삭된 레벨 없음";
pub const VERDICT_MARK: &str = "★판정점";
pub const MEASUREMENT_CONFIG_TITLE: &str = "## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)";
pub const MEASUREMENT_CONFIG_KEYS: [&str; 21] = [
    "bench_mode",
    "bench_mode_kind",
    "bench_mode_source",
    "downgrade_reason",
    "downgrade_reason_source",
    "bench_tool",
    "bench_tool_version",
    "bench_tool_version_source",
    "repeats",
    "repeats_source",
    "repeats_completed",
    "dataset_kind",
    "dataset_source",
    "dataset_id",
    "dataset_revision",
    "dataset_source_sha256",
    "dataset_column_map",
    "dataset_preprocess_strategy",
    "dataset_seed",
    "dataset_duplicate_sampling",
    "dataset_tokenizer",
];
const _MEASUREMENT_INT_KEYS: [&str; 2] = ["repeats", "repeats_completed"];
pub const REPORT_LITE_MODE_LINE: &str = "mode: lite";
pub const REPORT_LITE_SECTION_PREFIX: &str = "## lite 지표";
pub const LITE_SECTION_TITLE: &str = "## lite 지표 — 서빙 성공 직후 스냅샷 (결정론 파싱 · 손저작 ✗)";
crate::re_static!(_LITE_GEN_TPS_ROW, r"gen tokens/sec[^|]*\|\s*[0-9]");
crate::re_static!(_INT_VALUE, r"-?[0-9]+");

const PROG: &str = "render_bench_section.py";
/// The module docstring (`argparse` description, RawDescriptionHelpFormatter).
const DOC: &str = r#"render_bench_section.py — hint 태그 03-benchmark 절의 **동시성별 표**를 결정론으로 렌더한다.

왜 있는가(plan_26090616 H · 사용자 지시 2026-09-06):
    full 벤치는 동시성 레벨마다 측정한다. 그런데 인증서는 판정점 하나(`decode_tps_conc1`)만 싣고,
    hint 의 벤치 절은 인증서만 읽었다 — 그래서 **레벨별 곡선이 hint 에 한 번도 실린 적이 없다**.
    수신자는 "동시성 1에서 50 t/s" 만 보고 그 레시피가 부하에서 어떻게 되는지 알 수 없었다.

    수치는 **LLM 이 옮기지 않는다.** 이 스크립트가 벤치 산출물에서 파싱해 렌더하고, `--verify` 가
    발행된 절을 다시 렌더해 **diff 0** 을 요구한다. 손이 닿으면 그 자리에서 걸린다.

출처 우선순위(둘 다 결정론 · 어느 쪽을 썼는지 산출에 적는다 — 헌법 §결정론 규율):
    ① `--sweep-index <sweep_index.json>` — 가장 풍부(꼬리 p99·산포 std 포함). `output/` 평면이라
       캠페인이 끝나면 사라질 수 있다.
    ② `--report <bench_report_*.md>` — `docs/benchmark/` 에서 태어난 **바인딩된 증거**라 살아남는다.
       `render_report.py` 가 결정론으로 낸 표를 되읽는다(두 스크립트 사이의 형식이 계약이다).

    ③ 경량 리포트(`mode: lite` · 2026-09-14 · plan_26091407 §4.5) — 스윕 표가 **없다**(동시성 곡선을 재지 않았다).
       이때는 `## lite 지표` 표를 그대로 옮긴 절을 렌더한다(lite_metrics 가 이미 결정론으로 쓴 표 · 재산정 ✗).

  측정 구성 표(두 리포트 공통 · `render_report.py MEASUREMENT_CONFIG_TITLE`)는 `parse_measurement_config` 가 되읽는다 —
  bench_mode · 도구 · 반복 N · downgrade_reason 을 hint 동봉 문서·PAYLOAD·카탈로그로 나르는 **유일한 파서**다.
  lite 지표 존재 판정(`lite_metrics_present`)도 여기 하나다 — hint_tag(`HINT_MISSING_LITE` 요구)와 hint_collect(그 코드를
  적는 손)가 같은 술어를 쓴다(두 판정기가 갈리면 "적으라고 요구하는데 적는 손이 다른 기준을 쓴다" 가 된다).

  렌더: render_bench_section.py --report docs/benchmark/bench_report_....md
  대조: render_bench_section.py --verify --section <03-benchmark.md> --report <...>
  전수: render_bench_section.py --verify --all --hints-dir <dir>
  단위: render_bench_section.py --selftest
"#;

fn failure<T>(msg: impl Into<String>) -> HR<T> {
    Err(HintError::py("BenchSectionFailure", msg))
}

fn pyerr<T>(ty: &str, msg: impl Into<String>) -> HR<T> {
    Err(HintError::py(ty, msg))
}

// ── Python str helpers ─────────────────────────────────────────────────────────────────────────
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}

fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(py_isspace)
}

/// `str.splitlines()` (no keepends).
fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let brk = matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if brk {
            out.push(&s[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(_, '\n')) = it.peek() {
                    it.next();
                    end += 1;
                }
            }
            start = end;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// `str(Path(s))`: repeated slashes and `.` components collapse, a trailing slash goes.
fn py_path_str(s: &str) -> String {
    if s.is_empty() {
        return ".".to_string();
    }
    let abs = s.starts_with('/');
    let lead2 = s.starts_with("//") && !s.starts_with("///");
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let body = parts.join("/");
    let prefix = if lead2 { "//" } else if abs { "/" } else { "" };
    if body.is_empty() {
        return if prefix.is_empty() { ".".to_string() } else { prefix.to_string() };
    }
    format!("{prefix}{body}")
}

/// `Path(s).name`.
fn py_path_name(s: &str) -> String {
    s.split('/').filter(|p| !p.is_empty() && *p != ".").last().unwrap_or("").to_string()
}

/// `float(text)` of a str (already stripped); `None` = ValueError.
fn py_float(text: &str) -> Option<f64> {
    let t = py_strip(text);
    let (sign, body) = match t.strip_prefix('-') {
        Some(b) => (-1.0, b),
        None => (1.0, t.strip_prefix('+').unwrap_or(t)),
    };
    let low = body.to_ascii_lowercase();
    if low == "inf" || low == "infinity" {
        return Some(sign * f64::INFINITY);
    }
    if low == "nan" {
        return Some(f64::NAN);
    }
    // digits with single underscores between digits, optional fraction and exponent.
    let b: Vec<char> = body.chars().collect();
    let mut i = 0;
    let digits = |i: &mut usize| -> Option<usize> {
        let st = *i;
        while *i < b.len() {
            if b[*i].is_ascii_digit() {
                *i += 1;
            } else if b[*i] == '_' && *i > st && b[*i - 1].is_ascii_digit() && *i + 1 < b.len() && b[*i + 1].is_ascii_digit() {
                *i += 1;
            } else {
                break;
            }
        }
        Some(*i - st)
    };
    let int_n = digits(&mut i)?;
    let mut frac_n = 0;
    if i < b.len() && b[i] == '.' {
        i += 1;
        frac_n = digits(&mut i)?;
    }
    if int_n == 0 && frac_n == 0 {
        return None;
    }
    if i < b.len() && (b[i] == 'e' || b[i] == 'E') {
        i += 1;
        if i < b.len() && (b[i] == '+' || b[i] == '-') {
            i += 1;
        }
        if digits(&mut i)? == 0 {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    let clean: String = body.chars().filter(|c| *c != '_').collect();
    clean.parse::<f64>().ok().map(|v| sign * v)
}

/// Python `==` on JSON-shaped values (numbers compare numerically, bool is an int).
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
        _ => match (num(a), num(b)) {
            (Some(x), Some(y)) => x == y,
            (None, None) => match (a, b) {
                (J::Null, J::Null) => true,
                (J::Str(x), J::Str(y)) => x == y,
                (J::List(x), J::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q)),
                (J::Dict(x), J::Dict(y)) => {
                    x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map(|w| py_eq(v, w)).unwrap_or(false))
                }
                _ => false,
            },
            _ => false,
        },
    }
}

/// `x.get(key)` on a value that must be a dict.
fn dget<'a>(x: &'a J, key: &str) -> HR<&'a J> {
    match x {
        J::Dict(m) => Ok(m.get(key).unwrap_or(&crate::value::NULL)),
        other => pyerr("AttributeError", format!("'{}' object has no attribute 'get'", other.type_name())),
    }
}

/// `x[key]` on a value that must be a dict.
fn dkey<'a>(x: &'a J, key: &str) -> HR<&'a J> {
    match x {
        J::Dict(m) => match m.get(key) {
            Some(v) => Ok(v),
            None => pyerr("KeyError", py_repr(key)),
        },
        J::List(_) => pyerr("TypeError", "list indices must be integers or slices, not str"),
        J::Str(_) => pyerr("TypeError", "string indices must be integers, not 'str'"),
        other => pyerr("TypeError", format!("'{}' object is not subscriptable", other.type_name())),
    }
}

/// `list(x)` / `for v in x` of a JSON value.
fn py_iter(x: &J) -> HR<Vec<J>> {
    match x {
        J::List(v) => Ok(v.clone()),
        J::Dict(m) => Ok(m.keys().map(J::from).collect()),
        J::Str(s) => Ok(s.chars().map(|c| J::Str(c.to_string())).collect()),
        other => pyerr("TypeError", format!("'{}' object is not iterable", other.type_name())),
    }
}

/// `x or default`.
fn or_else<'a>(x: &'a J, default: &'a J) -> &'a J {
    if x.truthy() {
        x
    } else {
        default
    }
}

// ── 칸 파싱 · 표시 ─────────────────────────────────────────────────────────────────────────────
/// `_num(text)`: a table cell → number or `None` (`N/A`·blank are missing, not 0).
pub fn _num(text: &str) -> HR<J> {
    let t = py_strip(text);
    let up = t.to_uppercase();
    if t.is_empty() || up == "N/A" || up == "-" || up == "—" {
        return Ok(J::Null);
    }
    Ok(py_float(t).map(J::Float).unwrap_or(J::Null))
}

fn fmt_fixed(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    format!("{:.*}", digits, x)
}

fn int_str(x: f64) -> String {
    if x.abs() < 9.0e18 {
        (x as i64).to_string()
    } else {
        format!("{:.0}", x)
    }
}

/// `_fmt(value, digits)`: display rounding (`N/A` for None · integral values without decimals).
pub fn _fmt(value: &J, digits: i64) -> HR<String> {
    let d = digits.max(0) as usize;
    match value {
        J::Null => Ok("N/A".to_string()),
        J::Bool(b) => Ok(if *b { "1" } else { "0" }.to_string()),
        J::Int(i) => Ok(i.to_string()),
        J::Float(f) => {
            if f.is_finite() && f.fract() == 0.0 {
                Ok(int_str(*f))
            } else {
                Ok(fmt_fixed(*f, d))
            }
        }
        J::Str(s) => match py_float(s) {
            Some(x) => Ok(fmt_fixed(x, d)),
            None => pyerr("ValueError", format!("could not convert string to float: {}", py_repr(s))),
        },
        other => pyerr("TypeError", format!("float() argument must be a string or a real number, not '{}'", other.type_name())),
    }
}

fn float_to_int(x: f64) -> HR<J> {
    if x.is_nan() {
        return pyerr("ValueError", "cannot convert float NaN to integer");
    }
    if x.is_infinite() {
        return pyerr("OverflowError", "cannot convert float infinity to integer");
    }
    let t = x.trunc();
    if t.abs() < 9.2e18 {
        Ok(J::Int(t as i64))
    } else {
        Ok(J::Float(t))
    }
}

// ── 리포트 파서 ────────────────────────────────────────────────────────────────────────────────
/// `parse_report(text)`: the sweep table of a `bench_report_*.md` → `{"levels", "truncated", "source_kind"}`.
pub fn parse_report(text: &str) -> HR<J> {
    let lines = splitlines(text);
    let head = match lines.iter().position(|ln| py_strip(ln) == REPORT_HEADER) {
        Some(i) => i,
        None => {
            return failure(
                "리포트에서 스윕 표 머리행을 찾지 못했다 — render_report.py 의 형식이 바뀌었거나 \
                 이 문서가 full 리포트가 아니다. 빈 표를 내지 않는다(형식 계약 위반은 fail-loud).",
            )
        }
    };
    let mut levels: Vec<J> = Vec::new();
    for ln in lines.iter().skip(head + 2) {
        let s = py_strip(ln);
        if !s.starts_with('|') {
            break;
        }
        let cells: Vec<String> = s.trim_matches('|').split('|').map(|c| py_strip(c).to_string()).collect();
        if cells.len() != 7 {
            break;
        }
        let conc_raw = &cells[0];
        let verdict_point = conc_raw.contains(VERDICT_MARK);
        let conc = _num(&conc_raw.replace(VERDICT_MARK, ""))?;
        let (done, failed) = match cells[6].split_once('/') {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (cells[6].clone(), String::new()),
        };
        let level = match conc.as_f64() {
            Some(x) => float_to_int(x)?,
            None => J::Null,
        };
        let mut m = Map::new();
        m.insert("level", level);
        m.insert("verdict_point", verdict_point);
        m.insert("decode_tps", _num(&cells[1])?);
        m.insert("output_throughput", _num(&cells[2])?);
        m.insert("total_token_throughput", _num(&cells[3])?);
        m.insert("ttft_ms_median", _num(&cells[4])?);
        m.insert("itl_ms_median", _num(&cells[5])?);
        m.insert("completed", _num(&done)?);
        m.insert("failed", _num(&failed)?);
        levels.push(J::Dict(m));
    }
    if levels.is_empty() {
        return failure("스윕 표에 행이 하나도 없다 — 측정이 없었는지 파싱이 깨졌는지 구분되지 않으므로 통과시키지 않는다.");
    }
    let mut truncated: Vec<J> = Vec::new();
    let mut in_trunc = false;
    for ln in lines.iter().skip(head) {
        if ln.starts_with(REPORT_TRUNC_MARK) {
            in_trunc = true;
            continue;
        }
        if in_trunc {
            if let Some(rest) = ln.strip_prefix("- ") {
                truncated.push(J::from(py_strip(rest)));
            } else if !py_strip(ln).is_empty() {
                break;
            }
        }
    }
    Ok(crate::jd! {"levels" => levels, "truncated" => truncated, "source_kind" => "bench_report"})
}

/// `is_lite_report(text)`: the header line `mode: lite` stands on a line of its own.
pub fn is_lite_report(text: &str) -> HR<bool> {
    Ok(splitlines(text).iter().any(|ln| py_strip(ln) == REPORT_LITE_MODE_LINE))
}

/// `lite_metrics_present(text)`: the single `HINT_MISSING_LITE` predicate (section present · gen tokens/sec measured).
pub fn lite_metrics_present(text: &str) -> HR<bool> {
    Ok(text.contains("lite 지표") && _LITE_GEN_TPS_ROW().is_search(text))
}

pub fn _unescape_cell(cell: &str) -> HR<String> {
    Ok(py_strip(&cell.replace("\\|", "|")).to_string())
}

/// `_split_row(line)`: `| a | b\|c |` → `["a", "b|c"]` (an escaped `|` is not a separator).
pub fn _split_row(line: &str) -> HR<Vec<String>> {
    let mut body = py_strip(line);
    if let Some(b) = body.strip_prefix('|') {
        body = b;
    }
    if body.ends_with('|') && !body.ends_with("\\|") {
        body = &body[..body.len() - 1];
    }
    // re.split(r"(?<!\\)\|", body)
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut prev: Option<char> = None;
    for c in body.chars() {
        if c == '|' && prev != Some('\\') {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
        prev = Some(c);
    }
    parts.push(cur);
    parts.iter().map(|c| _unescape_cell(c)).collect()
}

/// `parse_measurement_config(text)`: the measurement-config table → `{key: value}`; `None` when the section is absent.
pub fn parse_measurement_config(text: &str) -> HR<Option<J>> {
    let lines = splitlines(text);
    let start = match lines.iter().position(|ln| py_strip(ln) == MEASUREMENT_CONFIG_TITLE) {
        Some(i) => i,
        None => return Ok(None),
    };
    let mut out = Map::new();
    let mut seen_table = false;
    for ln in lines.iter().skip(start + 1) {
        let stripped = py_strip(ln);
        if stripped.starts_with("## ") {
            break;
        }
        if !stripped.starts_with('|') {
            if seen_table && stripped.is_empty() {
                break;
            }
            continue;
        }
        let cells = _split_row(stripped)?;
        if cells.len() != 2 {
            return failure(format!("측정 구성 표 행이 2칸이 아니다: {}", py_repr(stripped)));
        }
        let (key, value) = (&cells[0], &cells[1]);
        if key == "키" || key == "---" || key.chars().all(|c| c == '-') {
            seen_table = true;
            continue;
        }
        if !MEASUREMENT_CONFIG_KEYS.contains(&key.as_str()) {
            return failure(format!("측정 구성 표에 계약 밖 키가 있다: {} — render_report 와 계약이 갈라졌다", py_repr(key)));
        }
        seen_table = true;
        if value.to_uppercase() == "N/A" || value.is_empty() {
            out.insert(key.as_str(), J::Null);
        } else if _MEASUREMENT_INT_KEYS.contains(&key.as_str()) && _INT_VALUE().is_full(value) {
            out.insert(key.as_str(), value.parse::<i64>().map(J::Int).unwrap_or_else(|_| J::Float(value.parse::<f64>().unwrap_or(0.0))));
        } else {
            out.insert(key.as_str(), value.as_str());
        }
    }
    if out.is_empty() {
        return failure("측정 구성 절은 있는데 표 행이 하나도 없다 — 빈 기재로 접지 않는다");
    }
    Ok(Some(J::Dict(out)))
}

/// `parse_lite_report(text)`: the `## lite 지표` table of a lite report, rows kept verbatim.
pub fn parse_lite_report(text: &str) -> HR<J> {
    let lines = splitlines(text);
    let start = match lines.iter().position(|ln| py_strip(ln).starts_with(REPORT_LITE_SECTION_PREFIX)) {
        Some(i) => i,
        None => return failure("리포트에 `## lite 지표` 절이 없다 — 빈 lite 절을 내지 않는다."),
    };
    let mut table: Vec<J> = Vec::new();
    for ln in lines.iter().skip(start + 1) {
        let stripped = py_strip(ln);
        if stripped.starts_with("## ") {
            break;
        }
        if stripped.starts_with('|') {
            table.push(J::from(stripped));
        } else if !table.is_empty() {
            break;
        }
    }
    if table.len() < 3 {
        return failure("`## lite 지표` 절에 표(머리행·구분행·값행)가 없다 — lite 산정 실패를 빈 표로 접지 않는다.");
    }
    Ok(crate::jd! {"table" => table, "source_kind" => "bench_report(lite)"})
}

fn join_str(items: &[J]) -> HR<String> {
    let mut parts = Vec::with_capacity(items.len());
    for (i, v) in items.iter().enumerate() {
        match v {
            J::Str(s) => parts.push(s.clone()),
            other => return pyerr("TypeError", format!("sequence item {i}: expected str instance, {} found", other.type_name())),
        }
    }
    Ok(parts.join("\n"))
}

/// `render_lite(parsed, source_name)`: the lite table → hint section.
pub fn render_lite(parsed: &J, source_name: &str) -> HR<String> {
    let kind = dkey(parsed, "source_kind")?.display();
    let mut out: Vec<J> = vec![
        J::from(LITE_SECTION_TITLE),
        J::from(""),
        J::from("> 서빙 성공 직후의 **lite 스냅샷**이다 — cold 1회 + warm burst 1회. 동시성 곡선·반복·판정·인증서가 없다."),
        J::from("> 성능 baseline 이 아니며(OBSERVATION-ONLY) 수신자는 자기 환경에서 재측정한다."),
        J::from(format!("> 출처: `{source_name}` ({kind}) · 표는 리포트의 행을 그대로 옮긴다(재산정 ✗ · `--verify` diff 0).")),
        J::from(""),
    ];
    out.extend(py_iter(dkey(parsed, "table")?)?);
    out.push(J::from(""));
    join_str(&out)
}

/// `parse_sweep_index(doc)`: `sweep_index.json` → structured levels (tail p99 · spread included).
pub fn parse_sweep_index(doc: &J) -> HR<J> {
    let mut levels: Vec<J> = Vec::new();
    let vp = dget(doc, "verdict_point_level")?.clone();
    let empty_list = J::List(Vec::new());
    let empty_dict = J::dict();
    for lv in py_iter(or_else(dget(doc, "levels")?, &empty_list))? {
        let m = or_else(dget(&lv, "measured")?, &empty_dict);
        let mut o = Map::new();
        o.insert("level", dget(&lv, "level")?.clone());
        o.insert("verdict_point", py_eq(dget(&lv, "level")?, &vp));
        o.insert("status", dget(&lv, "status")?.clone());
        for k in [
            "decode_tps",
            "output_throughput",
            "total_token_throughput",
            "ttft_ms_median",
            "ttft_ms_p99",
            "itl_ms_median",
            "itl_ms_p99",
            "tpot_ms_median",
            "completed",
            "failed",
            "error_rate",
            "measurement_ok",
        ] {
            o.insert(k, dget(m, k)?.clone());
        }
        levels.push(J::Dict(o));
    }
    if levels.is_empty() {
        return failure("sweep_index 에 레벨이 없다 — 빈 표를 내지 않는다.");
    }
    let truncated = py_iter(or_else(dget(doc, "truncated")?, &empty_list))?;
    Ok(crate::jd! {"levels" => levels, "truncated" => truncated, "source_kind" => "sweep_index"})
}

/// `render(parsed, source_name)`: structured levels → the markdown section (the only table maker).
pub fn render(parsed: &J, source_name: &str) -> HR<String> {
    let levels = py_iter(dkey(parsed, "levels")?)?;
    let mut has_tail = false;
    for lv in &levels {
        if !dget(lv, "ttft_ms_p99")?.is_null() || !dget(lv, "itl_ms_p99")?.is_null() {
            has_tail = true;
            break;
        }
    }
    let kind = dkey(parsed, "source_kind")?.display();
    let mut out: Vec<String> = vec![
        SECTION_TITLE.to_string(),
        String::new(),
        CURRENT_CAPTION.to_string(),
        "> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.".to_string(),
        format!("> 출처: `{source_name}` ({kind}) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다)."),
        "> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.".to_string(),
        String::new(),
    ];
    let f = |lv: &J, k: &str| -> HR<String> { _fmt(dget(lv, k)?, 2) };
    if has_tail {
        out.push("| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50 | TTFT p99 | ITL p50 | ITL p99 | 완료/실패 |".to_string());
        out.push("|---|---|---|---|---|---|---|---|---|".to_string());
        for lv in &levels {
            let mark = if dget(lv, "verdict_point")?.truthy() { " ★판정점" } else { "" };
            out.push(format!(
                "| {}{} | {} | {} | {} | {} | {} | {} | {} | {}/{} |",
                dget(lv, "level")?.display(),
                mark,
                f(lv, "decode_tps")?,
                f(lv, "output_throughput")?,
                f(lv, "total_token_throughput")?,
                f(lv, "ttft_ms_median")?,
                f(lv, "ttft_ms_p99")?,
                f(lv, "itl_ms_median")?,
                f(lv, "itl_ms_p99")?,
                f(lv, "completed")?,
                f(lv, "failed")?
            ));
        }
    } else {
        out.push("| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |".to_string());
        out.push("|---|---|---|---|---|---|---|".to_string());
        for lv in &levels {
            let mark = if dget(lv, "verdict_point")?.truthy() { " ★판정점" } else { "" };
            out.push(format!(
                "| {}{} | {} | {} | {} | {} | {} | {}/{} |",
                dget(lv, "level")?.display(),
                mark,
                f(lv, "decode_tps")?,
                f(lv, "output_throughput")?,
                f(lv, "total_token_throughput")?,
                f(lv, "ttft_ms_median")?,
                f(lv, "itl_ms_median")?,
                f(lv, "completed")?,
                f(lv, "failed")?
            ));
        }
    }
    out.push(String::new());
    if levels.len() == 1 {
        out.push(
            "> ⚠ 레벨이 **하나뿐**이다 — 이 레시피의 부하 거동은 이 hint 로 알 수 없다. 곡선이 필요하면 재측정해야 한다(부재를 성능 판정으로 읽지 말 것)."
                .to_string(),
        );
        out.push(String::new());
    }
    let truncated = dkey(parsed, "truncated")?;
    if truncated.truthy() {
        out.push("**⚠ 절삭된 부하 레벨(조용히 자르지 않는다):**".to_string());
        for t in py_iter(truncated)? {
            out.push(format!("- {}", t.display()));
        }
        out.push(String::new());
    } else {
        out.push("_절삭된 레벨 없음(요청 전 레벨 완주)._".to_string());
        out.push(String::new());
    }
    Ok(out.join("\n"))
}

/// `extract_section(text, title)`: the published section up to the next `## ` heading (`None` when absent).
pub fn extract_section(text: &str, title: &str) -> HR<Option<String>> {
    let lines = splitlines(text);
    let start = match lines.iter().position(|ln| py_strip(ln) == title) {
        Some(i) => i,
        None => return Ok(None),
    };
    let mut end = lines.len();
    for (i, ln) in lines.iter().enumerate().skip(start + 1) {
        if ln.starts_with("## ") {
            end = i;
            break;
        }
    }
    Ok(Some(format!("{}\n", py_rstrip(&lines[start..end].join("\n")))))
}

// ── CLI ────────────────────────────────────────────────────────────────────────────────────────
#[derive(Default)]
struct Args {
    report: Option<String>,
    sweep_index: Option<String>,
    section: Option<String>,
    out: Option<String>,
    verify: bool,
    selftest: bool,
}

fn read_path(s: &str) -> HR<String> {
    let p = PathBuf::from(py_path_str(s));
    crate::core::read_text(&p).map_err(|e| {
        let ty = if e.starts_with("[Errno 2]") {
            "FileNotFoundError"
        } else if e.starts_with("[Errno 21]") {
            "IsADirectoryError"
        } else if e.starts_with("[Errno 13]") {
            "PermissionError"
        } else if e.starts_with("[Errno") {
            "OSError"
        } else {
            "UnicodeDecodeError"
        };
        HintError::py(ty, e)
    })
}

fn build_from_args(a: &Args) -> HR<(String, String)> {
    if let Some(si) = a.sweep_index.as_deref().filter(|s| !s.is_empty()) {
        let name = py_path_name(&py_path_str(si));
        let text = read_path(si)?;
        let doc = crate::value::loads(&text).map_err(|e| HintError::py("JSONDecodeError", e))?;
        return Ok((render(&parse_sweep_index(&doc)?, &name)?, name));
    }
    if let Some(r) = a.report.as_deref().filter(|s| !s.is_empty()) {
        let name = py_path_name(&py_path_str(r));
        let text = read_path(r)?;
        if is_lite_report(&text)? {
            return Ok((render_lite(&parse_lite_report(&text)?, &name)?, name));
        }
        return Ok((render(&parse_report(&text)?, &name)?, name));
    }
    failure("--sweep-index 또는 --report 가 필요하다(출처 없이 표를 만들지 않는다)")
}

fn caught(e: &HintError) -> bool {
    matches!(
        e.code.trim_start_matches("py:"),
        "BenchSectionFailure"
            | "OSError"
            | "FileNotFoundError"
            | "IsADirectoryError"
            | "PermissionError"
            | "NotADirectoryError"
            | "ValueError"
            | "JSONDecodeError"
            | "UnicodeDecodeError"
    )
}

fn run(a: &Args) -> HR<i32> {
    if a.selftest {
        return Ok(_selftest());
    }
    let (body, src) = build_from_args(a)?;
    if a.verify {
        let section = match a.section.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => s,
            None => return failure("--verify 는 --section 이 필요하다"),
        };
        let text = read_path(section)?;
        let title = splitlines(&body).first().map(|l| py_strip(l).to_string()).unwrap_or_default();
        let published = match extract_section(&text, &title)? {
            Some(p) => p,
            None => return failure(format!("발행 문서에 벤치 절이 없다: {section}")),
        };
        if py_strip(&published) != py_strip(&body) {
            let a_lines: Vec<String> = splitlines(&body).iter().map(|s| s.to_string()).collect();
            let b_lines: Vec<String> = splitlines(py_strip(&published)).iter().map(|s| s.to_string()).collect();
            let diff = unified_diff(&a_lines, &b_lines, "rendered", "published").join("\n");
            return failure(format!("발행된 벤치 절이 재렌더와 다르다 — 숫자가 손을 탔다:\n{}", crate::core::head_chars(&diff, 4000)));
        }
        io::outln(&format!("[render_bench_section] VERIFY PASS diff=0 ({src})"));
        return Ok(0);
    }
    if let Some(out) = a.out.as_deref().filter(|s| !s.is_empty()) {
        let p = PathBuf::from(py_path_str(out));
        std::fs::write(&p, body.as_bytes()).map_err(|e| crate::core::io_err(&e, &p))?;
        io::outln(&format!("[render_bench_section] wrote {out} ({src})"));
    } else {
        io::out(&body);
    }
    Ok(0)
}

/// `main(argv)`: `Err(code)` is argparse's `SystemExit(code)`; uncaught exceptions are `Err` of `HintError`.
pub fn main_inner(argv: &[String]) -> Result<HR<i32>, i32> {
    let a = parse_args(argv)?;
    Ok(match run(&a) {
        Ok(c) => Ok(c),
        Err(e) if caught(&e) => {
            io::errln(&format!("[render_bench_section] FAIL {}", e.message));
            Ok(1)
        }
        Err(e) => Err(e),
    })
}

/// CLI entry; returns the exit code.
pub fn main(args: &[String]) -> i32 {
    match main_inner(args) {
        Err(code) => code,
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            // An exception the original does not catch: Python prints a traceback and exits 1.
            io::errln("Traceback (most recent call last):");
            io::errln(&format!("{}: {}", e.code.trim_start_matches("py:"), e.message));
            1
        }
    }
}

// ── argparse (Python 3.12 layout for this parser) ──────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Help,
    Val(usize),
    Flag(usize),
}

const OPTS: [(&str, Kind); 8] = [
    ("-h", Kind::Help),
    ("--help", Kind::Help),
    ("--report", Kind::Val(0)),
    ("--sweep-index", Kind::Val(1)),
    ("--section", Kind::Val(2)),
    ("--out", Kind::Val(3)),
    ("--verify", Kind::Flag(0)),
    ("--selftest", Kind::Flag(1)),
];

fn action_name(k: Kind) -> &'static str {
    match k {
        Kind::Help => "-h/--help",
        Kind::Val(0) => "--report",
        Kind::Val(1) => "--sweep-index",
        Kind::Val(2) => "--section",
        Kind::Val(_) => "--out",
        Kind::Flag(0) => "--verify",
        Kind::Flag(_) => "--selftest",
    }
}

fn columns() -> i64 {
    std::env::var("COLUMNS").ok().and_then(|v| v.trim().parse::<i64>().ok()).filter(|c| *c > 0).unwrap_or(80)
}

fn usage_text(width: i64) -> String {
    let prefix = "usage: ";
    let parts = ["[-h]", "[--report REPORT]", "[--sweep-index SWEEP_INDEX]", "[--section SECTION]", "[--out OUT]", "[--verify]", "[--selftest]"];
    let full = format!("{PROG} {}", parts.join(" "));
    let text_width = width;
    if (prefix.len() + full.len()) as i64 <= text_width {
        return format!("{prefix}{full}\n\n");
    }
    let get_lines = |parts: &[&str], indent: &str, prefix: Option<&str>| -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut line: Vec<&str> = Vec::new();
        let indent_length = indent.len() as i64;
        let mut line_len = match prefix {
            Some(p) => p.len() as i64 - 1,
            None => indent_length - 1,
        };
        for part in parts {
            if line_len + 1 + part.len() as i64 > text_width && !line.is_empty() {
                lines.push(format!("{indent}{}", line.join(" ")));
                line.clear();
                line_len = indent_length - 1;
            }
            line.push(part);
            line_len += part.len() as i64 + 1;
        }
        if !line.is_empty() {
            lines.push(format!("{indent}{}", line.join(" ")));
        }
        if prefix.is_some() && !lines.is_empty() {
            lines[0] = lines[0][indent.len()..].to_string();
        }
        lines
    };
    let lines: Vec<String> = if (prefix.len() + PROG.len()) as f64 <= 0.75 * text_width as f64 {
        let indent = " ".repeat(prefix.len() + PROG.len() + 1);
        let mut all = vec![PROG];
        all.extend(parts.iter());
        get_lines(&all, &indent, Some(prefix))
    } else {
        let indent = " ".repeat(prefix.len());
        let mut l = get_lines(&parts, &indent, None);
        if l.len() > 1 {
            l = get_lines(&parts, &indent, None);
        }
        let mut out = vec![PROG.to_string()];
        out.extend(l);
        out
    };
    format!("{prefix}{}\n\n", lines.join("\n"))
}

/// `textwrap.wrap(text, width)` for whitespace-separated words (break_long_words, no hyphen splitting).
fn wrap(text: &str, width: i64) -> Vec<String> {
    let collapsed: String = text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    let mut chunks: Vec<String> = Vec::new();
    for (i, w) in collapsed.split(' ').enumerate() {
        if i > 0 {
            chunks.push(" ".to_string());
        }
        chunks.push(w.to_string());
    }
    chunks.reverse();
    let mut lines = Vec::new();
    let width = width.max(1);
    while !chunks.is_empty() {
        let mut cur: Vec<String> = Vec::new();
        let mut cur_len: i64 = 0;
        if !lines.is_empty() && chunks.last().map(|c| c.trim().is_empty()).unwrap_or(false) {
            chunks.pop();
        }
        while let Some(c) = chunks.last() {
            let l = c.chars().count() as i64;
            if cur_len + l <= width {
                cur_len += l;
                cur.push(chunks.pop().unwrap());
            } else {
                break;
            }
        }
        if let Some(c) = chunks.last() {
            if c.chars().count() as i64 > width {
                let space_left = (width - cur_len).max(1) as usize;
                let c = chunks.pop().unwrap();
                let head: String = c.chars().take(space_left).collect();
                let tail: String = c.chars().skip(space_left).collect();
                cur.push(head);
                chunks.push(tail);
            }
        }
        if cur.last().map(|c| c.trim().is_empty()).unwrap_or(false) {
            cur.pop();
        }
        if !cur.is_empty() {
            lines.push(cur.concat());
        }
    }
    lines
}

fn help_text(width: i64) -> String {
    let max_help_position = 24.min((width - 20).max(4));
    let help_position = (25 + 2 + 2).min(max_help_position);
    let help_width = (width - help_position).max(11);
    let action_width = (help_position - 2 - 2) as usize;
    let actions: [(&str, Option<&str>); 7] = [
        ("-h, --help", Some("show this help message and exit")),
        ("--report REPORT", None),
        ("--sweep-index SWEEP_INDEX", None),
        ("--section SECTION", Some("--verify: 대조할 발행 문서")),
        ("--out OUT", Some("렌더 결과를 쓸 경로(생략 시 stdout)")),
        ("--verify", None),
        ("--selftest", None),
    ];
    let mut items = String::new();
    for (header, help) in actions {
        match help {
            None => items.push_str(&format!("  {header}\n")),
            Some(h) => {
                let lines = wrap(h, help_width);
                let indent_first;
                if header.len() <= action_width {
                    items.push_str(&format!("  {header:<action_width$}  "));
                    indent_first = 0;
                } else {
                    items.push_str(&format!("  {header}\n"));
                    indent_first = help_position as usize;
                }
                if let Some(first) = lines.first() {
                    items.push_str(&format!("{}{first}\n", " ".repeat(indent_first)));
                    for l in &lines[1..] {
                        items.push_str(&format!("{}{l}\n", " ".repeat(help_position as usize)));
                    }
                }
            }
        }
    }
    let mut s = format!("{}{}\n\n\noptions:\n{}\n", usage_text(width), DOC, items);
    while s.contains("\n\n\n") {
        s = s.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", s.trim_matches('\n'))
}

fn usage_only(width: i64) -> String {
    format!("{}\n", usage_text(width).trim_matches('\n'))
}

fn arg_error(width: i64, msg: &str) -> i32 {
    io::err(&usage_only(width));
    io::err(&format!("{PROG}: error: {msg}\n"));
    2
}

enum Pat {
    A,
    Sep,
    O(Option<Kind>, String, Option<String>, bool),
}

fn negative_number(s: &str) -> bool {
    let b = &s[1..];
    (!b.is_empty() && b.chars().all(|c| c.is_ascii_digit()))
        || match b.split_once('.') {
            Some((i, f)) => i.chars().all(|c| c.is_ascii_digit()) && !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()),
            None => false,
        }
}

fn lookup(s: &str) -> Option<Kind> {
    OPTS.iter().find(|(n, _)| *n == s).map(|(_, k)| *k)
}

fn classify(arg: &str) -> Result<Pat, String> {
    if arg.is_empty() || !arg.starts_with('-') {
        return Ok(Pat::A);
    }
    if let Some(k) = lookup(arg) {
        return Ok(Pat::O(Some(k), arg.to_string(), None, false));
    }
    if arg.chars().count() == 1 {
        return Ok(Pat::A);
    }
    if let Some((o, e)) = arg.split_once('=') {
        if let Some(k) = lookup(o) {
            return Ok(Pat::O(Some(k), o.to_string(), Some(e.to_string()), true));
        }
    }
    // _get_option_tuples
    let mut tuples: Vec<(Kind, &str, Option<String>)> = Vec::new();
    let second_dash = arg[1..].starts_with('-');
    let mut sep = false;
    if second_dash {
        let (prefix, explicit) = match arg.split_once('=') {
            Some((p, e)) => (p, Some(e.to_string())),
            None => (arg, None),
        };
        sep = explicit.is_some();
        for (n, k) in OPTS.iter() {
            if n.starts_with(prefix) {
                tuples.push((*k, n, explicit.clone()));
            }
        }
    } else {
        let short: String = arg.chars().take(2).collect();
        let short_explicit: String = arg.chars().skip(2).collect();
        for (n, k) in OPTS.iter() {
            if *n == short {
                tuples.push((*k, n, Some(short_explicit.clone())));
            } else if n.starts_with(arg) {
                tuples.push((*k, n, None));
            }
        }
    }
    if tuples.len() > 1 {
        let names: Vec<&str> = tuples.iter().map(|t| t.1).collect();
        return Err(format!("ambiguous option: {arg} could match {}", names.join(", ")));
    }
    if let Some((k, n, e)) = tuples.into_iter().next() {
        return Ok(Pat::O(Some(k), n.to_string(), e, sep));
    }
    if negative_number(arg) {
        return Ok(Pat::A);
    }
    if arg.contains(' ') {
        return Ok(Pat::A);
    }
    Ok(Pat::O(None, arg.to_string(), None, false))
}

fn parse_args(argv: &[String]) -> Result<Args, i32> {
    let width = columns() - 2;
    let mut pats: Vec<Pat> = Vec::new();
    let mut after_sep = false;
    for a in argv {
        if after_sep {
            pats.push(Pat::A);
        } else if a == "--" {
            pats.push(Pat::Sep);
            after_sep = true;
        } else {
            match classify(a) {
                Ok(p) => pats.push(p),
                Err(msg) => return Err(arg_error(width, &msg)),
            }
        }
    }
    let mut out = Args::default();
    let mut extras: Vec<String> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        match &pats[i] {
            Pat::A | Pat::Sep => {
                extras.push(argv[i].clone());
                i += 1;
            }
            Pat::O(None, _, _, _) => {
                extras.push(argv[i].clone());
                i += 1;
            }
            Pat::O(Some(k), opt, explicit, sep) => {
                let k = *k;
                match (k, explicit) {
                    (Kind::Help | Kind::Flag(_), Some(e)) => {
                        // Python 3.12: a single-dash flag with an attached (non `=`, non-dash) rest queues the flag
                        // and moves the rest on as further short flags/extras — here -h runs and exits first.
                        if !opt[1..].starts_with('-') && !e.is_empty() && !*sep && !e.starts_with('-') {
                            io::out(&help_text(width));
                            return Err(0);
                        }
                        return Err(arg_error(width, &format!("argument {}: ignored explicit argument {}", action_name(k), py_repr(e))));
                    }
                    (Kind::Help, None) => {
                        io::out(&help_text(width));
                        return Err(0);
                    }
                    (Kind::Flag(n), None) => {
                        if n == 0 {
                            out.verify = true;
                        } else {
                            out.selftest = true;
                        }
                        i += 1;
                    }
                    (Kind::Val(n), e) => {
                        let value = match e {
                            Some(v) => {
                                i += 1;
                                v.clone()
                            }
                            None => {
                                // nargs=None: exactly one following 'A' (a `--` separator does not count).
                                let j = i + 1;
                                if !(j < pats.len() && matches!(pats[j], Pat::A)) {
                                    return Err(arg_error(width, &format!("argument {}: expected one argument", action_name(k))));
                                }
                                i = j + 1;
                                argv[j].clone()
                            }
                        };
                        match n {
                            0 => out.report = Some(value),
                            1 => out.sweep_index = Some(value),
                            2 => out.section = Some(value),
                            _ => out.out = Some(value),
                        }
                    }
                }
            }
        }
    }
    if !extras.is_empty() {
        return Err(arg_error(width, &format!("unrecognized arguments: {}", extras.join(" "))));
    }
    Ok(out)
}

// ── difflib.unified_diff ───────────────────────────────────────────────────────────────────────
struct Matcher<'a> {
    a: &'a [String],
    b: &'a [String],
    b2j: HashMap<&'a str, Vec<usize>>,
}

impl<'a> Matcher<'a> {
    fn new(a: &'a [String], b: &'a [String]) -> Matcher<'a> {
        let mut b2j: HashMap<&'a str, Vec<usize>> = HashMap::new();
        for (i, e) in b.iter().enumerate() {
            b2j.entry(e.as_str()).or_default().push(i);
        }
        let n = b.len();
        if n >= 200 {
            let ntest = n / 100 + 1;
            b2j.retain(|_, v| v.len() <= ntest);
        }
        Matcher { a, b, b2j }
    }

    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
        let (a, b) = (self.a, self.b);
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut newj2len: HashMap<usize, usize> = HashMap::new();
            if let Some(js) = self.b2j.get(a[i].as_str()) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = j.checked_sub(1).and_then(|p| j2len.get(&p)).copied().unwrap_or(0) + 1;
                    newj2len.insert(j, k);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            j2len = newj2len;
        }
        while besti > alo && bestj > blo && a[besti - 1] == b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && a[besti + bestsize] == b[bestj + bestsize] {
            bestsize += 1;
        }
        (besti, bestj, bestsize)
    }

    fn matching_blocks(&self) -> Vec<(usize, usize, usize)> {
        let (la, lb) = (self.a.len(), self.b.len());
        let mut queue = vec![(0, la, 0, lb)];
        let mut blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let (i, j, k) = self.find_longest_match(alo, ahi, blo, bhi);
            if k > 0 {
                blocks.push((i, j, k));
                if alo < i && blo < j {
                    queue.push((alo, i, blo, j));
                }
                if i + k < ahi && j + k < bhi {
                    queue.push((i + k, ahi, j + k, bhi));
                }
            }
        }
        blocks.sort();
        let (mut i1, mut j1, mut k1) = (0, 0, 0);
        let mut out = Vec::new();
        for (i2, j2, k2) in blocks {
            if i1 + k1 == i2 && j1 + k1 == j2 {
                k1 += k2;
            } else {
                if k1 > 0 {
                    out.push((i1, j1, k1));
                }
                (i1, j1, k1) = (i2, j2, k2);
            }
        }
        if k1 > 0 {
            out.push((i1, j1, k1));
        }
        out.push((la, lb, 0));
        out
    }

    fn opcodes(&self) -> Vec<(&'static str, usize, usize, usize, usize)> {
        let (mut i, mut j) = (0, 0);
        let mut ans = Vec::new();
        for (ai, bj, size) in self.matching_blocks() {
            let tag = if i < ai && j < bj {
                "replace"
            } else if i < ai {
                "delete"
            } else if j < bj {
                "insert"
            } else {
                ""
            };
            if !tag.is_empty() {
                ans.push((tag, i, ai, j, bj));
            }
            i = ai + size;
            j = bj + size;
            if size > 0 {
                ans.push(("equal", ai, i, bj, j));
            }
        }
        ans
    }

    fn grouped_opcodes(&self, n: usize) -> Vec<Vec<(&'static str, usize, usize, usize, usize)>> {
        let mut codes = self.opcodes();
        if codes.is_empty() {
            codes = vec![("equal", 0, 1, 0, 1)];
        }
        if codes[0].0 == "equal" {
            let (t, i1, i2, j1, j2) = codes[0];
            codes[0] = (t, i1.max(i2.saturating_sub(n)), i2, j1.max(j2.saturating_sub(n)), j2);
        }
        let last = codes.len() - 1;
        if codes[last].0 == "equal" {
            let (t, i1, i2, j1, j2) = codes[last];
            codes[last] = (t, i1, i2.min(i1 + n), j1, j2.min(j1 + n));
        }
        let nn = n + n;
        let mut groups = Vec::new();
        let mut group = Vec::new();
        for (tag, mut i1, i2, mut j1, j2) in codes {
            if tag == "equal" && i2 - i1 > nn {
                group.push((tag, i1, i2.min(i1 + n), j1, j2.min(j1 + n)));
                groups.push(std::mem::take(&mut group));
                i1 = i1.max(i2.saturating_sub(n));
                j1 = j1.max(j2.saturating_sub(n));
            }
            group.push((tag, i1, i2, j1, j2));
        }
        if !group.is_empty() && !(group.len() == 1 && group[0].0 == "equal") {
            groups.push(group);
        }
        groups
    }
}

fn format_range_unified(start: usize, stop: usize) -> String {
    let mut beginning = start + 1;
    let length = stop - start;
    if length == 1 {
        return beginning.to_string();
    }
    if length == 0 {
        beginning -= 1;
    }
    format!("{beginning},{length}")
}

/// `difflib.unified_diff(a, b, fromfile, tofile, lineterm="")` (n=3).
fn unified_diff(a: &[String], b: &[String], fromfile: &str, tofile: &str) -> Vec<String> {
    let m = Matcher::new(a, b);
    let mut out = Vec::new();
    let mut started = false;
    for group in m.grouped_opcodes(3) {
        if !started {
            started = true;
            out.push(format!("--- {fromfile}"));
            out.push(format!("+++ {tofile}"));
        }
        let (first, last) = (group[0], group[group.len() - 1]);
        out.push(format!("@@ -{} +{} @@", format_range_unified(first.1, last.2), format_range_unified(first.3, last.4)));
        for (tag, i1, i2, j1, j2) in group {
            if tag == "equal" {
                for l in &a[i1..i2] {
                    out.push(format!(" {l}"));
                }
                continue;
            }
            if tag == "replace" || tag == "delete" {
                for l in &a[i1..i2] {
                    out.push(format!("-{l}"));
                }
            }
            if tag == "replace" || tag == "insert" {
                for l in &b[j1..j2] {
                    out.push(format!("+{l}"));
                }
            }
        }
    }
    out
}

// ── selftest ───────────────────────────────────────────────────────────────────────────────────
fn raised<T>(r: HR<T>) -> bool {
    matches!(r, Err(ref e) if e.code.trim_start_matches("py:") == "BenchSectionFailure")
}

fn run_checks(ck: &mut dyn FnMut(&str, bool)) {
    let report = [
        "# 성능 보고서",
        "",
        "## 부하 스윕 곡선 (client-load · reload 없음)",
        "",
        REPORT_HEADER,
        "|---|---|---|---|---|---|---|",
        "| 1 ★판정점 | 50.58 | 50.69957426582275 | 253.6959165410896 | 210.4933261871338 | 19.02007589153215 | 16/0 |",
        "| 2 | 61.10 | 120.5 | 400.25 | 300.1 | 32.4 | 16/0 |",
        "",
        "**⚠ 절삭된 부하 레벨(적응 상한 클램프 — 조용히 자르지 않음):**",
        "- level 4 truncated: parse/measurement_ok=false",
        "",
        "## 다음 절",
        "",
    ]
    .join("\n");
    let parsed = parse_report(&report).unwrap_or(J::Null);
    let levels = parsed["levels"].items();
    ck("리포트 표 2행 파싱", levels.len() == 2);
    ck("판정점 표시가 값을 오염시키지 않는다", levels.first().map(|l| l["level"] == J::Int(1) && l["verdict_point"] == J::Bool(true)).unwrap_or(false));
    ck("절삭 로그 회수", parsed["truncated"] == crate::jl!["level 4 truncated: parse/measurement_ok=false"]);
    let body = render(&parsed, "bench_report_x.md").unwrap_or_default();
    ck("렌더에 두 레벨이 모두 있다", body.contains("| 1 ★판정점 |") && body.contains("| 2 |"));
    ck("표시 반올림(원본 오염 ✗)", body.contains("50.70") && !body.contains("50.69957426582275"));
    ck("절삭이 절에 남는다", body.contains("level 4 truncated"));
    ck(
        "대표 run 범위와 반복별 warmup·요청 수 차이를 명시",
        body.contains("대표 run(첫 완주 run)")
            && body.contains("반복 run의 warmup·요청 수·완료 결과는 서로 다를 수")
            && !body.contains("동시 요청 수만")
            && !body.contains("request-rate=inf"),
    );
    ck("★음성대조: 머리행 부재는 빈 표가 아니라 FAIL", raised(parse_report("# 리포트\n본문뿐\n")));
    ck("★음성대조: 행 0 은 FAIL", raised(parse_report(&["#", REPORT_HEADER, "|---|---|---|---|---|---|---|", "", ""].join("\n"))));

    let mut idx = crate::jd! {"verdict_point_level" => 1, "levels" => vec![crate::jd!{
        "level" => 1, "status" => "ok", "measured" => crate::jd!{"decode_tps" => 50.58, "ttft_ms_median" => 210.5,
            "ttft_ms_p99" => 217.6, "itl_ms_median" => 19.0, "itl_ms_p99" => 19.15, "completed" => 16, "failed" => 0}}],
        "truncated" => J::List(vec![])};
    let tail_body = parse_sweep_index(&idx).and_then(|p| render(&p, "sweep_index.json")).unwrap_or_default();
    ck("sweep_index 는 꼬리 열(p99)을 낸다", tail_body.contains("TTFT p99") && tail_body.contains("217.60"));
    if let Some(m) = idx.get_mut("levels").and_then(|l| l.as_list_mut()).and_then(|l| l.get_mut(0)).and_then(|l| l.get_mut("measured")) {
        m.set("runs", crate::jl![crate::jd! {"completed" => 16, "warmup" => 2}, crate::jd! {"completed" => 18, "warmup" => 0}]);
    }
    let heterogeneous = parse_sweep_index(&idx).and_then(|p| render(&p, "sweep_index.json")).unwrap_or_default();
    ck(
        "반복별 요청·warmup 차이가 있어도 대표 표 값은 유지하고 동일 반복이라고 단정하지 않는다",
        heterogeneous.contains("16/0")
            && heterogeneous.contains("대표 run(첫 완주 run)")
            && heterogeneous.contains("서로 다를 수")
            && !heterogeneous.contains("동시 요청 수만"),
    );
    ck("레벨 1개면 곡선 부재를 명시한다", tail_body.contains("레벨이 **하나뿐**"));

    let doc = format!("머리말\n\n{body}\n## 그다음\n내용\n");
    let ex = |t: &str, title: &str| extract_section(t, title).ok().flatten();
    ck("절 추출이 왕복한다", ex(&doc, SECTION_TITLE).map(|s| py_strip(&s) == py_strip(&body)).unwrap_or(false));
    ck("절 부재는 None", ex("# 아무것도 없음\n", SECTION_TITLE).is_none());
    let tampered = doc.replace("50.58", "99.99");
    ck("★음성대조: 한 숫자만 손대도 diff 가 잡는다", ex(&tampered, SECTION_TITLE).map(|s| py_strip(&s) != py_strip(&body)).unwrap_or(false));

    let lite_doc = [
        "# 경량 성능 보고서(lite)",
        "",
        REPORT_LITE_MODE_LINE,
        "",
        "> 생성일 2026-01-01T00:00:00Z.",
        "",
        MEASUREMENT_CONFIG_TITLE,
        "",
        "> 기재다.",
        "",
        "| 키 | 값 |",
        "|---|---|",
        "| bench_mode | lite |",
        "| bench_mode_kind | declared-lite |",
        "| bench_mode_source | declared(a \\| b) |",
        "| downgrade_reason | N/A |",
        "| repeats | 1 |",
        "",
        "## lite 지표 (서빙 성공 직후 스냅샷 · inform-only)",
        "",
        "> 설명",
        "",
        "| 메트릭 | 값 |",
        "|---|---|",
        "| gen tokens/sec (warm) | 26.00 t/s |",
        "| cold-start TTFT | 120 ms |",
        "",
        "## 측정 환경 스냅샷",
        "",
    ]
    .join("\n");
    let mc = parse_measurement_config(&lite_doc).ok().flatten();
    let want = crate::jd! {"bench_mode" => "lite", "bench_mode_kind" => "declared-lite", "bench_mode_source" => "declared(a | b)",
        "downgrade_reason" => J::Null, "repeats" => 1};
    ck("측정 구성 표를 키→값으로 읽는다(N/A=None · 반복 정수 · 이스케이프 `|` 복원)", mc.map(|m| py_eq(&m, &want)).unwrap_or(false));
    ck("★측정 구성 절 부재는 None(미기재 — 파싱 실패와 다르다)", matches!(parse_measurement_config(&report), Ok(None)));
    ck(
        "★음성대조: 계약 밖 키는 FAIL(계약이 갈라진 것을 조용히 버리지 않는다)",
        raised(parse_measurement_config(&lite_doc.replace("| repeats | 1 |", "| surprise | 1 |"))),
    );
    let lite = |t: &str| is_lite_report(t).unwrap_or(false);
    ck("경량 리포트 판별은 헤더 줄로만", lite(&lite_doc) && !lite(&report) && !lite("본문에 mode: lite 라는 말이 섞였을 뿐\n"));
    let lmp = |t: &str| lite_metrics_present(t).unwrap_or(false);
    ck("lite 지표 술어: 수치 행이 있으면 참", lmp(&lite_doc));
    ck("★음성대조 lite 지표 술어: gen tokens/sec 가 N/A 면 거짓(결손)", !lmp(&lite_doc.replace("26.00 t/s", "N/A")));
    ck("★음성대조 lite 지표 술어: 절이 없으면 거짓", !lmp(&report));
    let lite_body = parse_lite_report(&lite_doc).and_then(|p| render_lite(&p, "bench_report_x.md")).unwrap_or_default();
    ck("lite 절이 표를 행 그대로 옮긴다", lite_body.contains("| gen tokens/sec (warm) | 26.00 t/s |") && lite_body.starts_with(LITE_SECTION_TITLE));
    let doc2 = format!("머리\n\n{lite_body}\n## 다음\n");
    ck("lite 절 추출이 왕복한다", ex(&doc2, LITE_SECTION_TITLE).map(|s| py_strip(&s) == py_strip(&lite_body)).unwrap_or(false));
    ck(
        "★음성대조: lite 절 숫자 손댐을 diff 가 잡는다",
        ex(&doc2.replace("26.00", "99.00"), LITE_SECTION_TITLE).map(|s| py_strip(&s) != py_strip(&lite_body)).unwrap_or(false),
    );
    ck("★음성대조: 표 없는 lite 절은 빈 절이 아니라 FAIL", raised(parse_lite_report("# r\n\nmode: lite\n\n## lite 지표\n\n> 산정 실패\n")));
}

/// `_selftest()`: prints `  PASS/FAIL <label>` lines and the verdict; returns the exit code.
pub fn _selftest() -> i32 {
    let mut ok = true;
    run_checks(&mut |label, cond| {
        io::outln(&format!("{}{label}", if cond { "  PASS " } else { "  FAIL " }));
        ok = ok && cond;
    });
    io::outln(&format!("[render_bench_section] {}", if ok { "PASS" } else { "FAIL" }));
    if ok {
        0
    } else {
        1
    }
}

/// The failing selftest labels (empty = PASS).
pub fn selftest() -> Vec<String> {
    let mut bad = Vec::new();
    run_checks(&mut |label, cond| {
        if !cond {
            bad.push(label.to_string());
        }
    });
    bad
}

