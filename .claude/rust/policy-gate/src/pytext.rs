//! Small Python `str` helpers over Rust `&str` (the texts here are strictly decoded UTF-8): whitespace strip,
//! `splitlines`, `repr`, and `pathlib.PurePosixPath` string normalization.

use agent_runtime_support::pystr::{is_python_space, PyStr};
use agent_runtime_support::pyvalue::PyValue;

pub fn is_space(c: char) -> bool {
    is_python_space(c as u32)
}

/// `s.strip()` (no argument).
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

pub fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_space)
}

/// `s.splitlines()`.
pub fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes: Vec<(usize, char)> = s.char_indices().collect();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let (pos, c) = bytes[i];
        let is_break = matches!(
            c,
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if is_break {
            out.push(&s[start..pos]);
            let mut next = pos + c.len_utf8();
            if c == '\r' && i + 1 < bytes.len() && bytes[i + 1].1 == '\n' {
                next += 1;
                i += 1;
            }
            start = next;
        }
        i += 1;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// `repr(s)`.
pub fn repr(s: &str) -> String {
    PyStr::new(s).repr()
}

/// `repr(list_of_str)`.
pub fn repr_list<S: AsRef<str>>(items: &[S]) -> String {
    let parts: Vec<String> = items.iter().map(|s| repr(s.as_ref())).collect();
    format!("[{}]", parts.join(", "))
}

/// `repr(value)` of an optional JSON value (`None` when absent).
pub fn repr_opt(v: Option<&PyValue>) -> String {
    match v {
        None => "None".to_string(),
        Some(v) => v.repr(),
    }
}

/// `str(value)`.
pub fn display(v: &PyValue) -> String {
    let t = v.display_text();
    t.to_utf8().unwrap_or_else(|| String::from_utf8_lossy(&t.to_backslashreplace_bytes()).into_owned())
}

/// `str(PurePosixPath(s))`.
pub fn pure_path_str(s: &str) -> String {
    if s.is_empty() {
        return ".".to_string();
    }
    let leading = if s.starts_with("//") && !s.starts_with("///") {
        "//"
    } else if s.starts_with('/') {
        "/"
    } else {
        ""
    };
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let body = parts.join("/");
    if leading.is_empty() && body.is_empty() {
        return ".".to_string();
    }
    format!("{leading}{body}")
}

/// `PyStr` -> `String`, keeping lone surrogates visible rather than panicking.
pub fn pystr_text(s: &PyStr) -> String {
    s.to_utf8().unwrap_or_else(|| String::from_utf8_lossy(&s.to_backslashreplace_bytes()).into_owned())
}
