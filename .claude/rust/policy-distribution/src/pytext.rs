//! The few Python `str`/`pathlib` text semantics the original leans on: `Path.read_text(encoding="utf-8")` (strict
//! decoding + universal newlines), `str.splitlines()`, `str.strip()`, `repr(str)`/`repr(list)` and the exception text
//! of a failed read, so the report strings are byte-for-byte the original's.
use agent_runtime_support::pyjson::utf8_error_text;
use agent_runtime_support::pystr::{is_python_space, PyStr};
use std::path::{Path, PathBuf};

/// A failed `read_text` as the original's `except (OSError, UnicodeError) as exc` sees it: type name + `str(exc)`.
#[derive(Debug, Clone)]
pub struct PyError {
    pub type_name: String,
    pub message: String,
}

impl PyError {
    pub fn new(type_name: &str, message: impl Into<String>) -> Self {
        PyError { type_name: type_name.to_string(), message: message.into() }
    }
    /// `f"{type(exc).__name__}: {exc}"`.
    pub fn colon_space(&self) -> String {
        format!("{}: {}", self.type_name, self.message)
    }
    pub fn is_unicode(&self) -> bool {
        self.type_name == "UnicodeDecodeError"
    }
}

/// Python's OSError subclass name for an errno (the ones `open()`/`exec` can raise here).
pub fn os_error_name(errno: Option<i32>) -> &'static str {
    match errno {
        Some(2) => "FileNotFoundError",
        Some(13) | Some(1) => "PermissionError",
        Some(20) => "NotADirectoryError",
        Some(21) => "IsADirectoryError",
        Some(17) => "FileExistsError",
        Some(4) => "InterruptedError",
        Some(11) => "BlockingIOError",
        Some(110) => "TimeoutError",
        Some(111) => "ConnectionRefusedError",
        _ => "OSError",
    }
}

/// `strerror` text without Rust's ` (os error N)` suffix.
pub fn strerror(err: &std::io::Error) -> String {
    let text = err.to_string();
    match text.rfind(" (os error ") {
        Some(at) => text[..at].to_string(),
        None => text,
    }
}

/// `str(OSError(errno, strerror, filename))` = `[Errno N] strerror: 'filename'`.
pub fn os_error(err: &std::io::Error, filename: &str) -> PyError {
    let errno = err.raw_os_error();
    let message = match errno {
        Some(n) => format!("[Errno {n}] {}: {}", strerror(err), py_repr(filename)),
        None => err.to_string(),
    };
    PyError::new(os_error_name(errno), message)
}

/// `repr(str)`.
pub fn py_repr(text: &str) -> String {
    PyStr::new(text).repr()
}

/// `repr(list_of_str)` = `str(list)`.
pub fn py_list_repr(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| py_repr(s)).collect();
    format!("[{}]", parts.join(", "))
}

/// Universal-newline translation of a text-mode read (`\r\n` and lone `\r` become `\n`).
pub fn universal_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

/// Strict UTF-8 decode of bytes as `bytes.decode("utf-8")` (error text = `str(UnicodeDecodeError)`).
pub fn decode_strict(bytes: &[u8]) -> Result<String, PyError> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s.to_string()),
        Err(e) => Err(PyError::new("UnicodeDecodeError", utf8_error_text(bytes, &e))),
    }
}

/// `Path.read_text(encoding="utf-8")`.
pub fn read_text(path: &Path) -> Result<String, PyError> {
    let bytes = std::fs::read(path).map_err(|e| os_error(&e, &path.to_string_lossy()))?;
    Ok(universal_newlines(&decode_strict(&bytes)?))
}

/// `Path.read_text(encoding="utf-8", errors="replace")`.
pub fn read_text_replace(path: &Path) -> Result<String, PyError> {
    let bytes = std::fs::read(path).map_err(|e| os_error(&e, &path.to_string_lossy()))?;
    Ok(universal_newlines(&String::from_utf8_lossy(&bytes)))
}

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// `str.splitlines()` (no keepends): every Python line boundary, `\r\n` as one.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut iter = text.char_indices().peekable();
    while let Some((at, c)) = iter.next() {
        if is_line_break(c) {
            out.push(&text[start..at]);
            let mut next = at + c.len_utf8();
            if c == '\r' && bytes.get(next) == Some(&b'\n') {
                iter.next();
                next += 1;
            }
            start = next;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// `str.strip()` with Python's whitespace set.
pub fn py_strip(text: &str) -> &str {
    text.trim_matches(|c: char| is_python_space(c as u32))
}

/// `str.strip(chars)` for an ASCII character set.
pub fn strip_chars<'a>(text: &'a str, chars: &str) -> &'a str {
    text.trim_matches(|c: char| chars.contains(c))
}

/// `text[-n:]` in code points.
pub fn tail_chars(text: &str, n: usize) -> String {
    let count = text.chars().count();
    if count <= n {
        return text.to_string();
    }
    text.chars().skip(count - n).collect()
}

/// `str.find` as a signed index (-1 = absent), byte offsets (only compared with each other).
pub fn find(text: &str, needle: &str) -> i64 {
    text.find(needle).map(|i| i as i64).unwrap_or(-1)
}

/// Python `PurePath` ordering: component-wise comparison of the parts (not of the joined string).
pub fn path_key(path: &Path) -> Vec<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    path.components().map(|c| c.as_os_str().as_bytes().to_vec()).collect()
}

pub fn sort_paths(paths: &mut [PathBuf]) {
    paths.sort_by_key(|p| path_key(p));
}

/// `str(path.relative_to(root))`.
pub fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string_lossy().into_owned())
}
