//! Private Python-compat helpers of artifacts part 3 (not re-exported: `pub(super)` only reaches `a3`).
#![allow(dead_code)]
use std::path::{Component, Path};

use crate::core::{HintError, HR};
use crate::pyre::Re;
use crate::value::J;

/// `str.isspace()` of one character.
pub(crate) fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
pub(crate) fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}
pub(crate) fn py_lstrip(s: &str) -> &str {
    s.trim_start_matches(py_isspace)
}
pub(crate) fn clen(s: &str) -> usize {
    s.chars().count()
}
/// `str.splitlines()` (no keepends).
pub(crate) fn py_splitlines(s: &str) -> Vec<&str> {
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

/// `_get(obj, name)` (default None) on a J value.
pub(crate) fn g(obj: &J, name: &str) -> J {
    obj.get(name).cloned().unwrap_or(J::Null)
}
/// `x or {}`.
pub(crate) fn or_dict(x: J) -> J {
    if x.truthy() {
        x
    } else {
        J::dict()
    }
}
/// `x or []`.
pub(crate) fn or_list(x: J) -> J {
    if x.truthy() {
        x
    } else {
        J::list()
    }
}
/// Python `str(x)` of a J (None → "None").
pub(crate) fn s(x: &J) -> String {
    x.display()
}
/// Python `==` between a J and an int (1 == 1.0 == True).
pub(crate) fn eq_int(x: &J, n: i64) -> bool {
    match x {
        J::Int(i) => *i == n,
        J::Float(f) => *f == n as f64,
        J::Bool(b) => (*b as i64) == n,
        _ => false,
    }
}

/// `shlex.quote`.
pub(crate) fn shq(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let safe = s.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if safe {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}

/// `format(x, "g")`.
pub(crate) fn fmt_g(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let e = format!("{:.5e}", x);
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if (-4..6).contains(&exp) {
        let decimals = (5 - exp).max(0) as usize;
        let mut t = format!("{:.*}", decimals, x);
        if t.contains('.') {
            while t.ends_with('0') {
                t.pop();
            }
            if t.ends_with('.') {
                t.pop();
            }
        }
        t
    } else {
        let mut m = mant.to_string();
        if m.contains('.') {
            while m.ends_with('0') {
                m.pop();
            }
            if m.ends_with('.') {
                m.pop();
            }
        }
        format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
    }
}

/// Components of a PurePosixPath text (`a//b/./c/` → [a, b, c]; a leading `/` is kept as the first part).
pub(crate) fn pure_parts(p: &str) -> Vec<String> {
    let mut out = Vec::new();
    if p.starts_with('/') {
        out.push("/".to_string());
    }
    for part in p.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        out.push(part.to_string());
    }
    out
}
fn join_parts(parts: &[String]) -> String {
    if parts.is_empty() {
        return ".".into();
    }
    if parts[0] == "/" {
        return format!("/{}", parts[1..].join("/"));
    }
    parts.join("/")
}
/// `str(PurePath(p))`.
pub(crate) fn pure_str(p: &str) -> String {
    join_parts(&pure_parts(p))
}
/// `str(PurePath(p).parent)`.
pub(crate) fn pure_parent(p: &str) -> String {
    let parts = pure_parts(p);
    if parts.len() <= 1 {
        if parts.first().map(String::as_str) == Some("/") {
            return "/".into();
        }
        return ".".into();
    }
    join_parts(&parts[..parts.len() - 1])
}
/// `PurePath(p).name`.
pub(crate) fn pure_name(p: &str) -> String {
    let parts = pure_parts(p);
    match parts.last() {
        Some(x) if x != "/" => x.clone(),
        _ => String::new(),
    }
}
/// `PurePath(p).suffix`.
pub(crate) fn pure_suffix(p: &str) -> String {
    let name = pure_name(p);
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => name[i..].to_string(),
        _ => String::new(),
    }
}

fn comps(p: &Path) -> Vec<String> {
    p.components()
        .filter_map(|c| match c {
            Component::RootDir => Some("/".to_string()),
            Component::CurDir => None,
            other => Some(other.as_os_str().to_string_lossy().into_owned()),
        })
        .collect()
}
/// `p.relative_to(base).as_posix()` (lexical).
pub(crate) fn rel_posix(base: &Path, p: &Path) -> HR<String> {
    let b = comps(base);
    let q = comps(p);
    if q.len() < b.len() || q[..b.len()] != b[..] {
        return Err(HintError::py(
            "ValueError",
            format!("{} is not in the subpath of {}", crate::value::py_repr(&crate::core::posix(p)), crate::value::py_repr(&crate::core::posix(base))),
        ));
    }
    Ok(join_parts(&q[b.len()..].to_vec()))
}
/// Python `Path.__lt__` key (list of parts).
pub(crate) fn path_key(p: &Path) -> Vec<String> {
    comps(p)
}

/// `Path.rglob(pattern)` for a `*.<ext>` pattern: every entry under `root` (no symlinked-directory recursion).
pub(crate) fn rglob_suffix(root: &Path, suffix: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    fn walk(d: &Path, suffix: &str, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(suffix) {
                out.push(p.clone());
            }
            if let Ok(ft) = e.file_type() {
                if ft.is_dir() {
                    walk(&p, suffix, out);
                }
            }
        }
    }
    walk(root, suffix, &mut out);
    out.sort_by_key(|p| path_key(p));
    out
}

pub(crate) fn re_cached(cell: &'static std::sync::OnceLock<Re>, pat: &str, flags: u32) -> &'static Re {
    cell.get_or_init(|| Re::new(pat, flags))
}

/// `datetime(y, m, d, h, mi, s)` validity (Python raises ValueError otherwise).
pub(crate) fn valid_dt(y: i64, mo: i64, d: i64, h: i64, mi: i64, se: i64) -> bool {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&mo) || h > 23 || mi > 59 || se > 59 || h < 0 || mi < 0 || se < 0 {
        return false;
    }
    let dim = match mo {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    };
    (1..=dim).contains(&d)
}

/// Read a file as text with `errors="replace"`.
pub(crate) fn read_lossy(p: &Path) -> HR<String> {
    std::fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| crate::core::io_err(&e, p))
}
/// `path.read_text(encoding="utf-8")` (strict).
pub(crate) fn read_strict(p: &Path) -> HR<String> {
    let b = std::fs::read(p).map_err(|e| crate::core::io_err(&e, p))?;
    utf8_strict(b)
}
pub(crate) fn utf8_strict(b: Vec<u8>) -> HR<String> {
    String::from_utf8(b).map_err(|e| {
        let pos = e.utf8_error().valid_up_to();
        let byte = e.as_bytes().get(pos).copied().unwrap_or(0);
        HintError::py("UnicodeDecodeError", format!("'utf-8' codec can't decode byte 0x{byte:02x} in position {pos}: invalid start byte"))
    })
}
