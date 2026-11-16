//! Python `str` text helpers the originals rely on: `splitlines()`, `strip()`, code-point slicing, `re` wrappers.
use fancy_regex::Regex;

/// `str.isspace()` for one code point (Python's whitespace set, wider than Rust's for the 0x1c..0x1f separators).
pub fn py_isspace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\x0b' | '\x0c' | '\r' | '\x1c' | '\x1d' | '\x1e' | '\x1f' | ' ' | '\u{85}' | '\u{a0}'
        | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// `str.splitlines()` (no keepends).
pub fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if is_line_break(c) {
            out.push(&s[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(j, '\n')) = it.peek() {
                    it.next();
                    end = j + 1;
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

/// `s[:n]` by code points.
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `str.lower()` approximation good for the ASCII/Hangul text these tools see (Rust's Unicode lowercase).
pub fn py_lower(s: &str) -> String {
    s.to_lowercase()
}

pub fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| panic!("bad regex {pattern}: {e}"))
}

/// `re.search(p, s) is not None`.
pub fn search(rx: &Regex, s: &str) -> bool {
    rx.is_match(s).unwrap_or(false)
}

/// Compile `pattern` for `re.match` (anchored at the start only).
pub fn re_match(pattern: &str) -> Regex {
    re(&format!(r"\A(?:{pattern})"))
}

/// `re.findall` for a pattern without groups (whole matches) or with exactly one group (that group).
pub fn findall(rx: &Regex, s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for c in rx.captures_iter(s).flatten() {
        if c.len() > 1 {
            out.push(c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default());
        } else {
            out.push(c.get(0).map(|m| m.as_str().to_string()).unwrap_or_default());
        }
    }
    out
}

/// Python `repr(str)` (the quote choice and escapes of CPython).
pub fn py_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if !is_printable(c) => {
                let v = c as u32;
                if v <= 0xff {
                    out.push_str(&format!("\\x{v:02x}"));
                } else if v <= 0xffff {
                    out.push_str(&format!("\\u{v:04x}"));
                } else {
                    out.push_str(&format!("\\U{v:08x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn is_printable(c: char) -> bool {
    let v = c as u32;
    if (0x80..0xa0).contains(&v) || v == 0xad {
        return false;
    }
    if (0x2028..=0x2029).contains(&v) || (0xd800..0xe000).contains(&v) || v == 0xfeff {
        return false;
    }
    if c.is_whitespace() && c != ' ' {
        return false;
    }
    true
}

/// Python `repr(list_of_str)`.
pub fn py_repr_list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| py_repr_str(s)).collect::<Vec<_>>().join(", "))
}

/// `os.path.normpath` for relative POSIX paths.
pub fn normpath(p: &str) -> String {
    let abs = p.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.last().map_or(false, |l| *l != "..") {
                    parts.pop();
                } else if !abs {
                    parts.push("..");
                }
            }
            s => parts.push(s),
        }
    }
    let body = parts.join("/");
    if abs {
        format!("/{body}")
    } else if body.is_empty() {
        ".".into()
    } else {
        body
    }
}
