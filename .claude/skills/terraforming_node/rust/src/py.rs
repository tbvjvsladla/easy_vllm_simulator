//! Small Python `str` behaviours the ported programs rely on (code-point semantics over Rust `String`s).

use agent_runtime_support::pystr::{is_python_space, is_python_word, PyStr};
use agent_runtime_support::pyvalue::PyValue;

pub fn is_space(c: char) -> bool {
    is_python_space(c as u32)
}
pub fn is_word(c: char) -> bool {
    is_python_word(c as u32)
}

/// `str.splitlines()` (keepends=False).
pub fn splitlines(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\n' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => {
                out.push(std::mem::take(&mut cur));
            }
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
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

/// `str.strip()`.
pub fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `str.split()` with no argument.
pub fn split_ws(text: &str) -> Vec<&str> {
    text.split(is_space).filter(|s| !s.is_empty()).collect()
}

/// The first `n` code points (`text[:n]` for n >= 0).
pub fn head(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// `bytes.decode("utf-8", errors="replace")`.
pub fn decode_replace(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A PyStr as UTF-8 text; lone surrogates become U+FFFD (documented residual: the original would fail to print them).
pub fn text(s: &PyStr) -> String {
    match s.to_utf8() {
        Some(t) => t,
        None => s
            .code_points()
            .iter()
            .map(|&p| char::from_u32(p).unwrap_or('\u{fffd}'))
            .collect(),
    }
}

/// Python `str(value)` as UTF-8 text.
pub fn display(v: &PyValue) -> String {
    text(&v.display_text())
}

/// Python slice bounds `[:k]` over a length (negative k counts from the end).
pub fn slice_end(len: usize, k: i64) -> usize {
    if k >= 0 {
        (k as usize).min(len)
    } else {
        len.saturating_sub(k.unsigned_abs() as usize)
    }
}

/// `json.dumps(value, ensure_ascii=False, indent=2)` as UTF-8 text.
pub fn dumps_pretty(v: &PyValue) -> String {
    text(&v.dumps_pretty().expect("serialisable value"))
}

/// `json.dumps(value, ensure_ascii=False)`.
pub fn dumps_unicode(v: &PyValue) -> String {
    text(&v.dumps_unicode().expect("serialisable value"))
}

pub fn s(t: &str) -> PyValue {
    PyValue::str(t)
}

/// `value.get(key)` on a dict (None for a missing key or a non-dict).
pub fn get<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(key).map(|x| x.peel()),
        _ => None,
    }
}

/// `value.get(key)` as text when it is a non-empty str (`x.get(k)` truthy and a str).
pub fn get_str(v: &PyValue, key: &str) -> Option<String> {
    match get(v, key) {
        Some(PyValue::Str(s)) if !s.is_empty() => Some(text(s)),
        _ => None,
    }
}

pub fn truthy(v: Option<&PyValue>) -> bool {
    v.is_some_and(|x| x.is_truthy())
}

/// Python `str(x)` of an optional value (`None` when absent).
pub fn display_opt(v: Option<&PyValue>) -> String {
    v.map_or_else(|| "None".to_string(), display)
}

/// `dict(pairs)` builder keeping insertion order.
pub fn dict(pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = agent_runtime_support::pyvalue::PyDict::new();
    for (k, v) in pairs {
        d.set(k, v);
    }
    PyValue::Dict(d)
}

pub fn list_str(items: &[String]) -> PyValue {
    PyValue::List(items.iter().map(|x| s(x)).collect())
}

pub fn opt_str(v: Option<&str>) -> PyValue {
    v.map_or(PyValue::None, s)
}
