//! The measurement-config table reader the publisher consults (`render_bench_section.parse_measurement_config` of
//! hint-publisher, the table's single parser owner; the writer is adversarial-benchmark `render_report`).
//!
//! The owner is still the hint-publisher Python module; this is the faithful Rust port of the one function the
//! publisher calls (plus its constants), kept byte-for-byte on its contract strings so that the publisher's Rust path
//! never starts an interpreter. When hint-publisher moves to Rust this module is replaced by a call into that owner.

use crate::json::{int, obj, s, V};
use agent_runtime_support::pystr::{is_decimal_digit, is_python_space};
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub const MEASUREMENT_CONFIG_TITLE: &str = "## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)";
pub const MEASUREMENT_CONFIG_KEYS: [&str; 21] = [
    "bench_mode", "bench_mode_kind", "bench_mode_source", "downgrade_reason",
    "downgrade_reason_source", "bench_tool", "bench_tool_version", "bench_tool_version_source",
    "repeats", "repeats_source", "repeats_completed",
    "dataset_kind", "dataset_source", "dataset_id", "dataset_revision", "dataset_source_sha256",
    "dataset_column_map", "dataset_preprocess_strategy", "dataset_seed", "dataset_duplicate_sampling",
    "dataset_tokenizer",
];
const MEASUREMENT_INT_KEYS: [&str; 2] = ["repeats", "repeats_completed"];

fn py_strip(t: &str) -> &str {
    t.trim_matches(|c: char| is_python_space(c as u32))
}

/// `str.splitlines()` (the line-boundary set of Python `str`).
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < bytes.len() {
        let (at, c) = bytes[i];
        let boundary = matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if boundary {
            out.push(&text[start..at]);
            let mut next = at + c.len_utf8();
            if c == '\r' && i + 1 < bytes.len() && bytes[i + 1].1 == '\n' {
                next += 1;
                i += 1;
            }
            start = next;
        }
        i += 1;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

fn unescape_cell(cell: &str) -> String {
    py_strip(&cell.replace("\\|", "|")).to_string()
}

/// `_split_row`: `| a | b\|c |` -> `['a', 'b|c']` (an escaped `|` is not a separator).
fn split_row(line: &str) -> Vec<String> {
    let mut body = py_strip(line);
    if let Some(rest) = body.strip_prefix('|') {
        body = rest;
    }
    if body.ends_with('|') && !body.ends_with("\\|") {
        body = &body[..body.len() - 1];
    }
    // re.split(r"(?<!\\)\|", body)
    let mut cells = Vec::new();
    let mut cur_start = 0;
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    for (k, (at, c)) in chars.iter().enumerate() {
        if *c == '|' && !(k > 0 && chars[k - 1].1 == '\\') {
            cells.push(unescape_cell(&body[cur_start..*at]));
            cur_start = at + 1;
        }
    }
    cells.push(unescape_cell(&body[cur_start..]));
    cells
}

fn is_int_text(v: &str) -> bool {
    // re.fullmatch(r"-?[0-9]+", value)
    let d = v.strip_prefix('-').unwrap_or(v);
    !d.is_empty() && d.chars().all(|c| c.is_ascii_digit())
}

/// `str.upper() == "N/A"` for the cell text.
fn is_na(v: &str) -> bool {
    v.to_uppercase() == "N/A"
}

/// `parse_measurement_config(text)`: `Ok(None)` = no section (a pre-table report), `Err(message)` = the owner's
/// `BenchSectionFailure`.
pub fn parse_measurement_config(text: &str) -> Result<Option<V>, String> {
    let lines = splitlines(text);
    let start = match lines.iter().position(|ln| py_strip(ln) == MEASUREMENT_CONFIG_TITLE) {
        Some(i) => i,
        None => return Ok(None),
    };
    let mut out = PyDict::new();
    let mut seen_table = false;
    for ln in &lines[start + 1..] {
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
        let cells = split_row(stripped);
        if cells.len() != 2 {
            return Err(format!("측정 구성 표 행이 2칸이 아니다: {}", crate::json::repr(stripped)));
        }
        let (key, value) = (&cells[0], &cells[1]);
        if key == "키" || key == "---" || key.chars().all(|c| c == '-') {
            seen_table = true;
            continue;
        }
        if !MEASUREMENT_CONFIG_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "측정 구성 표에 계약 밖 키가 있다: {} — render_report 와 계약이 갈라졌다",
                crate::json::repr(key)
            ));
        }
        seen_table = true;
        let v = if is_na(value) || value.is_empty() {
            PyValue::None
        } else if MEASUREMENT_INT_KEYS.contains(&key.as_str()) && is_int_text(value) {
            match value.parse::<num_bigint::BigInt>() {
                Ok(n) => PyValue::Int(n),
                Err(_) => int(0),
            }
        } else {
            s(value)
        };
        out.set(key, v);
    }
    if out.is_empty() {
        return Err("측정 구성 절은 있는데 표 행이 하나도 없다 — 빈 기재로 접지 않는다".to_string());
    }
    let _ = obj; // (builder kept for symmetry with the other modules)
    let _ = is_decimal_digit;
    Ok(Some(PyValue::Dict(out)))
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn table_and_failures() {
        let t = format!("# r\n\n{MEASUREMENT_CONFIG_TITLE}\n\n| 키 | 값 |\n|---|---|\n| bench_mode | lite |\n| repeats | 3 |\n| downgrade_reason | N/A |\n");
        let v = parse_measurement_config(&t).unwrap().unwrap();
        assert_eq!(v.repr(), "{'bench_mode': 'lite', 'repeats': 3, 'downgrade_reason': None}");
        assert!(parse_measurement_config("# none\n").unwrap().is_none());
        assert!(parse_measurement_config(&format!("{MEASUREMENT_CONFIG_TITLE}\n| a | b | c |\n")).is_err());
        assert!(parse_measurement_config(&format!("{MEASUREMENT_CONFIG_TITLE}\nno table\n")).is_err());
        assert_eq!(split_row("| a | b\\|c |"), vec!["a", "b|c"]);
    }
}
