//! Executable assertion-id extraction per evidence file kind (`policy_registry.extract_assertion_ids`).
//!
//! `.py` used CPython's `ast`: top-level `def` names (not `async def`), top-level `Assign`/`AnnAssign` targets that are
//! plain names, and `Class.test*` for every class anywhere whose bases name `TestCase` (as a name or the attribute of a
//! dotted base). Here a small Python lexer (strings incl. f-string replacement fields, comments, brackets, line
//! continuation) builds logical lines and reads the same three facts from them. A source the lexer cannot close (an
//! unterminated string or bracket) is `None`, like the original's `SyntaxError`; other syntax errors CPython would reject are
//! not detected (evidence files are importable modules). The answers equal the original's on every `.py` of the
//! repository (golden `checks/data/assertion_ids.json`).

use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use fancy_regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

pub type Ids = BTreeSet<String>;

/// `str.splitlines()` (the separators Python's `str` recognises).
pub fn py_splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let is_sep = matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if is_sep {
            out.push(&text[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(_, '\n')) = chars.peek() {
                    chars.next();
                    end += 1;
                }
            }
            start = end;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// `str.strip()` with no argument (Python whitespace).
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

pub fn is_py_space(c: char) -> bool {
    agent_runtime_support::pystr::is_python_space(c as u32)
}

/// `open(path, encoding="utf-8").read()`; `None` = OSError/UnicodeError.
fn read_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    read_text_bytes(&bytes)
}

pub fn read_text_bytes(bytes: &[u8]) -> Option<String> {
    let text = pyjson::decode_file_text(bytes).ok()?;
    text.to_utf8()
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static regex")
}

fn findall_group1(re: &Regex, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos <= text.len() {
        match re.captures_from_pos(text, pos) {
            Ok(Some(c)) => {
                let m = c.get(0).unwrap();
                out.push(c.get(1).map(|g| g.as_str().to_string()).unwrap_or_default());
                pos = if m.end() == m.start() { next_char(text, m.end()) } else { m.end() };
            }
            _ => break,
        }
    }
    out
}

fn next_char(text: &str, pos: usize) -> usize {
    match text[pos..].chars().next() {
        Some(c) => pos + c.len_utf8(),
        None => text.len() + 1,
    }
}

fn strip_block_comments(text: &str) -> String {
    // re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("/*") {
        match rest[i + 2..].find("*/") {
            Some(j) => {
                out.push_str(&rest[..i]);
                rest = &rest[i + 2 + j + 2..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

pub fn shell_ids_text(text: &str) -> Ids {
    static FUNC: OnceLock<Regex> = OnceLock::new();
    static VAR: OnceLock<Regex> = OnceLock::new();
    static LONGOPT: OnceLock<Regex> = OnceLock::new();
    let func = FUNC.get_or_init(|| re(r"(?m)^[ \t]*([A-Za-z_][A-Za-z0-9_]*)[ \t]*\([ \t]*\)[ \t]*\{"));
    let var = VAR.get_or_init(|| re(r"(?m)(?:^|[;() \t])([A-Za-z_][A-Za-z0-9_]*)="));
    let longopt = LONGOPT.get_or_init(|| re(r"--([a-z][a-z0-9-]*)\)"));
    let mut lines = Vec::new();
    for line in py_splitlines(text) {
        let mut out = String::new();
        let mut quote: Option<char> = None;
        let mut escaped = false;
        for ch in line.chars() {
            if escaped {
                out.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' && quote != Some('\'') {
                out.push(ch);
                escaped = true;
                continue;
            }
            if ch == '\'' || ch == '"' {
                if quote.is_none() {
                    quote = Some(ch);
                } else if quote == Some(ch) {
                    quote = None;
                }
                out.push(ch);
                continue;
            }
            if ch == '#' && quote.is_none() {
                break;
            }
            out.push(ch);
        }
        lines.push(out);
    }
    let executable = lines.join("\n");
    let mut ids = Ids::new();
    ids.extend(findall_group1(func, &executable));
    ids.extend(findall_group1(var, &executable));
    ids.extend(findall_group1(longopt, &executable));
    ids
}

pub fn yaml_ids_text(text: &str) -> Ids {
    static KEY: OnceLock<Regex> = OnceLock::new();
    let key = KEY.get_or_init(|| re(r"^[ \t]*([A-Za-z_][A-Za-z0-9_.\-]*)[ \t]*:[ \t]*(.*)$"));
    let mut ids = Ids::new();
    let mut block_indent: Option<usize> = None;
    for raw in py_splitlines(text) {
        let stripped = raw.trim_start_matches(' ');
        let indent = raw.chars().count() - stripped.chars().count();
        if let Some(bi) = block_indent {
            if stripped.is_empty() || indent > bi {
                continue;
            }
            block_indent = None;
        }
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        let line = match raw.find(" #") {
            Some(i) => &raw[..i],
            None => raw,
        };
        if let Ok(Some(c)) = key.captures(line) {
            ids.insert(c.get(1).unwrap().as_str().to_string());
            let value = py_strip(c.get(2).map(|m| m.as_str()).unwrap_or(""));
            if value.starts_with('|') || value.starts_with('>') {
                block_indent = Some(indent);
            }
        }
    }
    ids
}

pub fn markdown_ids_text(text: &str) -> Ids {
    let mut ids = Ids::new();
    for line in py_splitlines(text) {
        let stripped = py_strip(line);
        if stripped.starts_with('#') {
            let heading = py_strip(stripped.trim_start_matches('#'));
            if !heading.is_empty() {
                ids.insert(heading.to_string());
            }
        }
    }
    ids
}

pub fn typescript_ids_text(text: &str) -> Ids {
    static DECL: OnceLock<Regex> = OnceLock::new();
    let decl = DECL.get_or_init(|| {
        re(r"(?m)^(?:export[ \t]+)?(?:default[ \t]+)?(?:async[ \t]+)?(?:function\*?|class|const|let|type|interface)[ \t]+([A-Za-z_$][A-Za-z0-9_$]*)")
    });
    findall_group1(decl, &strip_block_comments(text)).into_iter().collect()
}

pub fn rust_ids_text(text: &str) -> Ids {
    static DECL: OnceLock<Regex> = OnceLock::new();
    let decl = DECL.get_or_init(|| {
        re(r"(?m)^(?:pub(?:\([a-z]+\))?[ \t]+)?(?:const|static|fn|struct|enum|trait)[ \t]+([A-Za-z_][A-Za-z0-9_]*)")
    });
    findall_group1(decl, &strip_rust_block_comments(text)).into_iter().collect()
}

/// Rust block comments removed with a minimal Rust lexer: `/*` inside a string/char literal or a `//` line comment does
/// not open a comment (the original's `re.sub(r"/\*.*?\*/", ...)` did, so a glob string like `"cells/*/x"` swallowed
/// every declaration up to the next `*/`; Rust evidence files became real ports only after the Python originals were
/// retired). Block comments nest, as in Rust. An unterminated block comment keeps the rest of the text (as before).
pub fn strip_rust_block_comments(text: &str) -> String {
    let b = text.as_bytes();
    let n = b.len();
    let mut out: Vec<u8> = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let c = b[i];
        if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            let end = b[i..].iter().position(|&x| x == b'\n').map_or(n, |p| i + p);
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            let mut depth = 1;
            let mut j = i + 2;
            while j < n && depth > 0 {
                if b[j] == b'/' && j + 1 < n && b[j + 1] == b'*' {
                    depth += 1;
                    j += 2;
                } else if b[j] == b'*' && j + 1 < n && b[j + 1] == b'/' {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            if depth > 0 {
                out.extend_from_slice(&b[i..]);
                i = n;
            } else {
                i = j;
            }
        } else if c == b'r' && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) && raw_string_open(b, i + 1).is_some() {
            let (hashes, body) = raw_string_open(b, i + 1).unwrap_or((0, i + 1));
            let close: Vec<u8> = std::iter::once(b'"').chain(std::iter::repeat(b'#').take(hashes)).collect();
            let end = b[body..].windows(close.len()).position(|w| w == close.as_slice()).map_or(n, |p| body + p + close.len());
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if c == b'"' {
            let mut j = i + 1;
            while j < n && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(n);
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if c == b'\'' {
            // char literal ('x', '\n', '\'', '\u{..}') vs lifetime ('a): copy a literal whole, a lifetime tick alone.
            let end = if i + 2 < n && b[i + 1] == b'\\' {
                b[i + 2..].iter().position(|&x| x == b'\'').map(|p| i + 2 + p + 1)
            } else {
                let ch_len = text[i + 1..].chars().next().map_or(0, |ch| ch.len_utf8());
                if ch_len > 0 && i + 1 + ch_len < n && b[i + 1 + ch_len] == b'\'' { Some(i + 2 + ch_len) } else { None }
            };
            let end = end.unwrap_or(i + 1);
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

/// After an `r` at `start-1`: `#*"` opens a raw string -> (hash count, index of the first body byte).
fn raw_string_open(b: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut j = start;
    while j < b.len() && b[j] == b'#' {
        j += 1;
    }
    if j < b.len() && b[j] == b'"' {
        Some((j - start, j + 1))
    } else {
        None
    }
}

/// `_json_assertion_ids`: the top-level keys of a JSON object file (`safe_load_json_object` failures -> None).
pub fn json_ids_bytes(bytes: &[u8]) -> Option<Ids> {
    let text = std::str::from_utf8(bytes).ok()?;
    match pyjson::loads(text) {
        Ok(PyValue::Dict(d)) => Some(d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect()),
        _ => None,
    }
}

/// `Path.suffix.lower()`
pub fn suffix_lower(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => name[i..].to_lowercase(),
        _ => String::new(),
    }
}

/// `extract_assertion_ids(path)` over the bytes the caller read (the kind comes from `path`'s suffix).
pub fn extract_from_bytes(path: &Path, bytes: Option<&[u8]>) -> Option<Ids> {
    let kind = suffix_lower(path);
    if !matches!(kind.as_str(), ".py" | ".rs" | ".ts" | ".sh" | ".yaml" | ".yml" | ".md" | ".json") {
        return None;
    }
    let bytes = bytes?;
    if kind == ".json" {
        return json_ids_bytes(bytes);
    }
    let text = read_text_bytes(bytes)?;
    match kind.as_str() {
        ".py" => python_ids_text(&text),
        ".rs" => Some(rust_ids_text(&text)),
        ".ts" => Some(typescript_ids_text(&text)),
        ".sh" => Some(shell_ids_text(&text)),
        ".yaml" | ".yml" => Some(yaml_ids_text(&text)),
        ".md" => Some(markdown_ids_text(&text)),
        _ => None,
    }
}

pub fn extract_assertion_ids(path: &Path) -> Option<Ids> {
    let kind = suffix_lower(path);
    if !matches!(kind.as_str(), ".py" | ".rs" | ".ts" | ".sh" | ".yaml" | ".yml" | ".md" | ".json") {
        return None;
    }
    if kind == ".json" {
        let bytes = std::fs::read(path).ok()?;
        return json_ids_bytes(&bytes);
    }
    let text = read_text(path)?;
    extract_from_bytes(path, Some(text.as_bytes()))
}

// ───────────────────────────── Python ─────────────────────────────

struct LogicalLine {
    indent: usize,
    /// String literals replaced by `S`, comments removed, continuation joined with a space.
    text: String,
}

struct Lexer<'a> {
    s: &'a [char],
    i: usize,
}

const STRING_PREFIX: &[&str] = &[
    "", "r", "u", "b", "br", "rb", "f", "fr", "rf", "R", "U", "B", "F", "Br", "bR", "BR", "Rb", "rB", "RB", "Fr", "fR", "FR",
    "Rf", "rF", "RF",
];

impl<'a> Lexer<'a> {
    fn peek(&self, k: usize) -> Option<char> {
        self.s.get(self.i + k).copied()
    }

    /// If a string literal (with prefix) starts at `i`, return (prefix length).
    fn string_start(&self) -> Option<usize> {
        for plen in [2usize, 1, 0] {
            let Some(q) = self.peek(plen) else { continue };
            if q != '\'' && q != '"' {
                continue;
            }
            let prefix: String = self.s[self.i..self.i + plen].iter().collect();
            if STRING_PREFIX.contains(&prefix.as_str()) {
                // the char before the prefix must not be an identifier char (else `xr"..."` is a name then a string)
                if plen > 0 && self.i > 0 && is_ident_char(self.s[self.i - 1]) {
                    continue;
                }
                return Some(plen);
            }
        }
        None
    }

    /// Skip a string literal starting at `self.i` (prefix included). Returns false on an unterminated literal.
    fn skip_string(&mut self, plen: usize) -> bool {
        let prefix: String = self.s[self.i..self.i + plen].iter().collect::<String>().to_lowercase();
        let raw = prefix.contains('r');
        let fstr = prefix.contains('f');
        self.i += plen;
        let q = self.s[self.i];
        let triple = self.peek(1) == Some(q) && self.peek(2) == Some(q);
        self.i += if triple { 3 } else { 1 };
        loop {
            let Some(c) = self.peek(0) else { return false };
            if c == '\\' && !raw {
                self.i += 2;
                continue;
            }
            if c == '\\' && raw {
                // a raw string still cannot end with an odd backslash before the quote
                self.i += 2;
                continue;
            }
            if !triple && c == '\n' {
                return false;
            }
            if fstr && c == '{' {
                if self.peek(1) == Some('{') {
                    self.i += 2;
                    continue;
                }
                self.i += 1;
                if !self.skip_replacement_field() {
                    return false;
                }
                continue;
            }
            if c == q {
                if triple {
                    if self.peek(1) == Some(q) && self.peek(2) == Some(q) {
                        self.i += 3;
                        return true;
                    }
                    self.i += 1;
                    continue;
                }
                self.i += 1;
                return true;
            }
            self.i += 1;
        }
    }

    /// Inside an f-string after `{`: expression, optional `!conv`, optional `:spec` (with nested fields), up to `}`.
    fn skip_replacement_field(&mut self) -> bool {
        let mut depth = 0i32;
        loop {
            let Some(c) = self.peek(0) else { return false };
            if let Some(plen) = self.string_start() {
                if !self.skip_string(plen) {
                    return false;
                }
                continue;
            }
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' => depth -= 1,
                '}' => {
                    if depth == 0 {
                        self.i += 1;
                        return true;
                    }
                    depth -= 1;
                }
                ':' if depth == 0 => {
                    self.i += 1;
                    return self.skip_format_spec();
                }
                '!' if depth == 0 && self.peek(1) != Some('=') => {
                    // conversion
                }
                _ => {}
            }
            self.i += 1;
        }
    }

    fn skip_format_spec(&mut self) -> bool {
        loop {
            let Some(c) = self.peek(0) else { return false };
            match c {
                '{' => {
                    self.i += 1;
                    if !self.skip_replacement_field() {
                        return false;
                    }
                }
                '}' => {
                    self.i += 1;
                    return true;
                }
                _ => self.i += 1,
            }
        }
    }
}

fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

fn logical_lines(source: &str) -> Option<Vec<LogicalLine>> {
    let chars: Vec<char> = source.chars().collect();
    let mut lx = Lexer { s: &chars, i: 0 };
    let mut out = Vec::new();
    let mut at_line_start = true;
    let mut indent = 0usize;
    let mut text = String::new();
    let mut depth = 0i32;
    let mut started = false;
    while lx.i < chars.len() {
        let c = chars[lx.i];
        if at_line_start && depth == 0 {
            // measure indentation
            let mut col = 0usize;
            let mut j = lx.i;
            while j < chars.len() {
                match chars[j] {
                    ' ' => col += 1,
                    '\t' => col = (col / 8 + 1) * 8,
                    '\x0c' => col = 0,
                    _ => break,
                }
                j += 1;
            }
            // blank or comment-only line?
            let rest = chars.get(j).copied();
            if rest.is_none() {
                break;
            }
            if matches!(rest, Some('\n') | Some('\r') | Some('#')) {
                // skip to end of line
                while lx.i < chars.len() && chars[lx.i] != '\n' {
                    lx.i += 1;
                }
                lx.i += 1;
                continue;
            }
            if rest == Some('\\') && matches!(chars.get(j + 1), Some('\n')) {
                // a lone continuation at line start: joins with the next line; keep the indentation of this line
            }
            indent = col;
            lx.i = j;
            at_line_start = false;
            started = true;
            continue;
        }
        let _ = c;
        if let Some(plen) = lx.string_start() {
            if !lx.skip_string(plen) {
                return None;
            }
            text.push('S');
            continue;
        }
        match c {
            '#' => {
                while lx.i < chars.len() && chars[lx.i] != '\n' {
                    lx.i += 1;
                }
                continue;
            }
            '\\' if matches!(chars.get(lx.i + 1), Some('\n')) => {
                text.push(' ');
                lx.i += 2;
                continue;
            }
            '\\' if matches!(chars.get(lx.i + 1), Some('\r')) => {
                text.push(' ');
                lx.i += 2;
                if chars.get(lx.i) == Some(&'\n') {
                    lx.i += 1;
                }
                continue;
            }
            '\n' | '\r' => {
                if depth > 0 {
                    text.push(' ');
                } else {
                    if started && !text.trim().is_empty() {
                        out.push(LogicalLine { indent, text: std::mem::take(&mut text) });
                    }
                    text.clear();
                    started = false;
                    at_line_start = true;
                }
                lx.i += 1;
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
        text.push(c);
        lx.i += 1;
    }
    if depth != 0 {
        return None;
    }
    if started && !text.trim().is_empty() {
        out.push(LogicalLine { indent, text });
    }
    Some(out)
}

const KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or",
    "pass", "raise", "return", "try", "while", "with", "yield",
];

fn is_name(s: &str) -> bool {
    let s = s.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\x0c');
    let mut it = s.chars();
    match it.next() {
        Some(c) if c == '_' || c.is_alphabetic() => {}
        _ => return false,
    }
    it.all(is_ident_char) && !KEYWORDS.contains(&s)
}

fn leading_word(s: &str) -> &str {
    let end = s.find(|c: char| !is_ident_char(c)).unwrap_or(s.len());
    &s[..end]
}

/// Positions (char-wise segments) of the depth-0 separators `sep` in `text`.
fn split_depth0(text: &str, is_sep: impl Fn(&[char], usize) -> Option<usize>) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            if let Some(width) = is_sep(&chars, i) {
                out.push(std::mem::take(&mut cur));
                i += width;
                continue;
            }
        }
        cur.push(c);
        i += 1;
    }
    out.push(cur);
    out
}

fn assign_eq(chars: &[char], i: usize) -> Option<usize> {
    if chars[i] != '=' {
        return None;
    }
    let next = chars.get(i + 1).copied();
    if next == Some('=') {
        return None;
    }
    let prev = if i > 0 { Some(chars[i - 1]) } else { None };
    if matches!(prev, Some('=' | '!' | '<' | '>' | ':' | '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' | '@')) {
        return None;
    }
    Some(1)
}

fn has_depth0_colon(text: &str) -> Option<usize> {
    let chars: Vec<char> = text.chars().collect();
    let mut depth = 0i32;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ':' if depth == 0 && chars.get(i + 1) != Some(&'=') => return Some(i),
            _ => {}
        }
    }
    None
}

const COMPOUND: &[&str] = &["if", "elif", "else", "for", "while", "try", "except", "finally", "with", "def", "class", "async"];

fn simple_statement_targets(stmt: &str, ids: &mut Ids) {
    let stmt = stmt.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\x0c');
    if stmt.is_empty() {
        return;
    }
    let segments = split_depth0(stmt, assign_eq);
    let first = &segments[0];
    if let Some(colon) = has_depth0_colon(first) {
        // AnnAssign: target is what precedes the annotation colon
        let target: String = first.chars().take(colon).collect();
        if is_name(&target) {
            ids.insert(target.trim().to_string());
        }
        return;
    }
    if segments.len() < 2 {
        return;
    }
    for seg in &segments[..segments.len() - 1] {
        if leading_word(seg.trim_start()) == "lambda" || seg.contains("lambda ") || seg.contains("lambda:") {
            break;
        }
        if is_name(seg) {
            ids.insert(seg.trim().to_string());
        }
    }
}

/// The parenthesised base list of `class NAME(...)`, split at depth-1 commas.
fn class_bases(after_name: &str) -> Vec<String> {
    let s = after_name.trim_start();
    if !s.starts_with('(') {
        return Vec::new();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut out = Vec::new();
    for &c in &chars {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(std::mem::take(&mut cur));
                    break;
                }
            }
            ',' if depth == 1 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out
}

fn base_is_testcase(base: &str) -> bool {
    let b = base.trim();
    if b.is_empty() || b.starts_with('*') {
        return false;
    }
    // keyword (metaclass=...) is not a base
    let chars: Vec<char> = b.chars().collect();
    if chars.iter().enumerate().any(|(i, _)| assign_eq(&chars, i).is_some()) {
        return false;
    }
    if is_name(b) {
        return b == "TestCase";
    }
    // Attribute: dotted names only (a.b.TestCase); a call or subscript is neither Name nor Attribute
    let parts: Vec<&str> = b.split('.').map(|p| p.trim()).collect();
    if parts.len() >= 2 && parts.iter().all(|p| is_name(p) || *p == "S") {
        return *parts.last().unwrap() == "TestCase";
    }
    false
}

pub fn python_ids_text(source: &str) -> Option<Ids> {
    let lines = logical_lines(source)?;
    let mut ids = Ids::new();
    for line in &lines {
        if line.indent != 0 {
            continue;
        }
        let t = line.text.trim_start();
        let word = leading_word(t);
        if word == "def" {
            let name = leading_word(t[3..].trim_start());
            if !name.is_empty() {
                ids.insert(name.to_string());
            }
            continue;
        }
        if t.starts_with('@') || COMPOUND.contains(&word) {
            continue;
        }
        if (word == "match" || word == "case") && t[word.len()..].starts_with(' ') && t.trim_end().ends_with(':') {
            continue; // soft-keyword compound statement
        }
        if matches!(word, "import" | "from" | "del" | "global" | "nonlocal" | "assert" | "return" | "raise" | "pass" | "break" | "continue") {
            continue;
        }
        for stmt in split_depth0(&line.text, |c, i| if c[i] == ';' { Some(1) } else { None }) {
            simple_statement_targets(&stmt, &mut ids);
        }
    }
    // TestCase classes anywhere
    for (k, line) in lines.iter().enumerate() {
        let t = line.text.trim_start();
        let mut rest = t;
        if leading_word(rest) != "class" {
            continue;
        }
        rest = rest[5..].trim_start();
        let name = leading_word(rest);
        if name.is_empty() {
            continue;
        }
        let after = &rest[name.len()..];
        if !class_bases(after).iter().any(|b| base_is_testcase(b)) {
            continue;
        }
        let Some(body) = lines.get(k + 1) else { continue };
        if body.indent <= line.indent {
            continue;
        }
        let body_indent = body.indent;
        for item in &lines[k + 1..] {
            if item.indent <= line.indent {
                break;
            }
            if item.indent != body_indent {
                continue;
            }
            let it = item.text.trim_start();
            if leading_word(it) == "def" {
                let m = leading_word(it[3..].trim_start());
                if m.starts_with("test") {
                    ids.insert(format!("{name}.{m}"));
                }
            }
        }
    }
    Some(ids)
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn rust_shapes() {
        // `/*` inside strings, raw strings and line comments opens no comment; real block comments (nested) hide decls.
        let src = "fn a() { let g = \"cells/*/x\"; }\n// see docs/*/y\nconst B: &str = r#\"q/*\"#;\nlet c = '\"'; fn x<'l>() {}\n/* outer /* inner */\nfn hidden() {}\n*/\npub fn d() {}\nstatic E: u8 = b'/';\npub(crate) fn f() {}\n    fn indented() {}\n";
        let ids = rust_ids_text(src);
        let want: Ids = ["a", "B", "d", "E", "f"].iter().map(|s| s.to_string()).collect();
        assert_eq!(ids, want);
        // an unterminated block comment keeps the rest
        assert!(rust_ids_text("/* open\nfn g() {}\n").contains("g"));
    }
    #[test]
    fn python_shapes() {
        let src = "import unittest\nX = 1\nY: int = 2\nA = B = 3\na, b = 4, 5\nf = lambda x=1: x\nasync def beta(): pass\n@dec\ndef alpha(\n  q=1):\n    Z = 2\nclass K(unittest.TestCase):\n    def test_one(self):\n        s = \"\"\"\ndef fake(): pass\n\"\"\"\n    def helper(self): pass\nclass P:\n    def test_two(self): pass\nx += 1\nif True: W = 1\nd = f\"{a!r:>{w}} {b['k']}\"\n";
        let ids = python_ids_text(src).unwrap();
        let want: Ids = ["X", "Y", "A", "B", "f", "alpha", "K.test_one", "d"].iter().map(|s| s.to_string()).collect();
        assert_eq!(ids, want);
        assert!(python_ids_text("x = '''open").is_none());
    }
}
