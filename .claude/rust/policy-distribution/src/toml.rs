//! Minimal TOML 1.0 reader for what `.codex/config.toml` (and the discovery self-test's mutations of it) can hold:
//! comments, bare/quoted/dotted keys, `[table]` / `[[array]]` headers, basic/literal (multi-line) strings, integers,
//! floats, booleans, arrays and inline tables. Errors carry tomllib's wording and `(at line L, column C)` position for
//! the cases this reader detects (invalid value/statement, missing `=`, duplicate key/table). Date-times are rejected
//! as an invalid value (tomllib accepts them; no repository config uses them — documented divergence).
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>),
    Table(BTreeMap<String, Value>),
}

#[derive(Debug, Clone)]
pub struct DecodeError(pub String);

struct P<'a> {
    c: &'a [char],
    i: usize,
}

type R<T> = Result<T, DecodeError>;

impl<'a> P<'a> {
    fn pos(&self) -> String {
        if self.i >= self.c.len() {
            return "(at end of document)".into();
        }
        let line = self.c[..self.i].iter().filter(|&&c| c == '\n').count() + 1;
        let col = match self.c[..self.i].iter().rposition(|&c| c == '\n') {
            Some(nl) => self.i - nl,
            None => self.i + 1,
        };
        format!("(at line {line}, column {col})")
    }
    fn err<T>(&self, msg: &str) -> R<T> {
        Err(DecodeError(format!("{msg} {}", self.pos())))
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn at(&self, s: &str) -> bool {
        let v: Vec<char> = s.chars().collect();
        self.c.get(self.i..self.i + v.len()) == Some(&v[..])
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.i += 1;
        }
    }
    fn comment(&mut self) {
        if self.peek() == Some('#') {
            while let Some(c) = self.peek() {
                if c == '\n' {
                    break;
                }
                self.i += 1;
            }
        }
    }
    fn ws_nl_comments(&mut self) {
        loop {
            self.ws();
            self.comment();
            match self.peek() {
                Some('\n') => self.i += 1,
                Some('\r') if self.c.get(self.i + 1) == Some(&'\n') => self.i += 2,
                _ => break,
            }
        }
    }

    fn key_part(&mut self) -> R<String> {
        match self.peek() {
            Some('"') => self.basic_string(),
            Some('\'') => self.literal_string(),
            _ => {
                let start = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                    self.i += 1;
                }
                if self.i == start {
                    return self.err("Invalid initial character for a key part");
                }
                Ok(self.c[start..self.i].iter().collect())
            }
        }
    }
    fn key(&mut self) -> R<Vec<String>> {
        let mut parts = vec![self.key_part()?];
        loop {
            self.ws();
            if self.peek() != Some('.') {
                return Ok(parts);
            }
            self.i += 1;
            self.ws();
            parts.push(self.key_part()?);
        }
    }

    fn escape(&mut self, out: &mut String) -> R<()> {
        self.i += 1; // backslash
        let c = match self.peek() {
            Some(c) => c,
            None => return self.err("Unterminated string"),
        };
        self.i += 1;
        let simple = match c {
            'b' => Some('\u{8}'),
            't' => Some('\t'),
            'n' => Some('\n'),
            'f' => Some('\u{c}'),
            'r' => Some('\r'),
            '"' => Some('"'),
            '\\' => Some('\\'),
            _ => None,
        };
        if let Some(s) = simple {
            out.push(s);
            return Ok(());
        }
        let n = match c {
            'u' => 4,
            'U' => 8,
            _ => {
                self.i -= 2;
                return self.err("Unescaped '\\' in a string");
            }
        };
        let hex: String = self.c.get(self.i..self.i + n).map(|s| s.iter().collect()).unwrap_or_default();
        if hex.len() != n || !hex.chars().all(|h| h.is_ascii_hexdigit()) {
            return self.err("Invalid hex value");
        }
        self.i += n;
        match char::from_u32(u32::from_str_radix(&hex, 16).unwrap()) {
            Some(ch) => out.push(ch),
            None => return self.err("Escaped character is not a Unicode scalar value"),
        }
        Ok(())
    }

    fn basic_string(&mut self) -> R<String> {
        if self.at("\"\"\"") {
            self.i += 3;
            if self.peek() == Some('\n') {
                self.i += 1;
            }
            let mut out = String::new();
            loop {
                if self.at("\"\"\"") {
                    self.i += 3;
                    while self.peek() == Some('"') && out.len() < usize::MAX {
                        out.push('"');
                        self.i += 1;
                    }
                    return Ok(out);
                }
                match self.peek() {
                    None => return self.err("Unterminated string"),
                    Some('\\') => {
                        if matches!(self.c.get(self.i + 1), Some(' ') | Some('\t') | Some('\n')) {
                            self.i += 1;
                            while matches!(self.peek(), Some(' ') | Some('\t') | Some('\n') | Some('\r')) {
                                self.i += 1;
                            }
                        } else {
                            self.escape(&mut out)?;
                        }
                    }
                    Some(c) => {
                        out.push(c);
                        self.i += 1;
                    }
                }
            }
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return self.err("Unterminated string"),
                Some('"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some('\\') => self.escape(&mut out)?,
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn literal_string(&mut self) -> R<String> {
        if self.at("'''") {
            self.i += 3;
            if self.peek() == Some('\n') {
                self.i += 1;
            }
            let start = self.i;
            loop {
                if self.at("'''") {
                    let mut s: String = self.c[start..self.i].iter().collect();
                    self.i += 3;
                    while self.peek() == Some('\'') {
                        s.push('\'');
                        self.i += 1;
                    }
                    return Ok(s);
                }
                if self.peek().is_none() {
                    return self.err("Unterminated string");
                }
                self.i += 1;
            }
        }
        self.i += 1;
        let start = self.i;
        loop {
            match self.peek() {
                None | Some('\n') => return self.err("Unterminated string"),
                Some('\'') => {
                    let s = self.c[start..self.i].iter().collect();
                    self.i += 1;
                    return Ok(s);
                }
                _ => self.i += 1,
            }
        }
    }

    fn value(&mut self) -> R<Value> {
        match self.peek() {
            Some('"') => Ok(Value::Str(self.basic_string()?)),
            Some('\'') => Ok(Value::Str(self.literal_string()?)),
            Some('[') => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.ws_nl_comments();
                    if self.peek() == Some(']') {
                        self.i += 1;
                        return Ok(Value::Array(items));
                    }
                    items.push(self.value()?);
                    self.ws_nl_comments();
                    match self.peek() {
                        Some(',') => self.i += 1,
                        Some(']') => {
                            self.i += 1;
                            return Ok(Value::Array(items));
                        }
                        _ => return self.err("Unclosed array"),
                    }
                }
            }
            Some('{') => {
                self.i += 1;
                let mut table = BTreeMap::new();
                self.ws();
                if self.peek() == Some('}') {
                    self.i += 1;
                    return Ok(Value::Table(table));
                }
                loop {
                    self.ws();
                    let key = self.key()?;
                    self.ws();
                    if self.peek() != Some('=') {
                        return self.err("Expected '=' after a key in a key/value pair");
                    }
                    self.i += 1;
                    self.ws();
                    let v = self.value()?;
                    if insert(&mut table, &key, v).is_err() {
                        return self.err("Cannot overwrite a value");
                    }
                    self.ws();
                    match self.peek() {
                        Some(',') => self.i += 1,
                        Some('}') => {
                            self.i += 1;
                            return Ok(Value::Table(table));
                        }
                        _ => return self.err("Unclosed inline table"),
                    }
                }
            }
            _ => {
                for (lit, v) in [("true", true), ("false", false)] {
                    if self.at(lit) {
                        self.i += lit.len();
                        return Ok(Value::Bool(v));
                    }
                }
                let start = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || "+-_.".contains(c)) {
                    self.i += 1;
                }
                let tok: String = self.c[start..self.i].iter().collect();
                let clean = tok.replace('_', "");
                let body = clean.trim_start_matches(['+', '-']);
                let ok_digits = !body.is_empty() && !tok.starts_with('_') && !tok.ends_with('_') && !tok.contains("__");
                if ok_digits && body.chars().all(|c| c.is_ascii_digit()) && (body == "0" || !body.starts_with('0')) {
                    if let Ok(n) = clean.parse::<i64>() {
                        return Ok(Value::Int(n));
                    }
                }
                if ok_digits && (body.contains('.') || body.contains('e') || body.contains('E'))
                    && body.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                {
                    if let Ok(f) = clean.parse::<f64>() {
                        return Ok(Value::Float(f));
                    }
                }
                if matches!(body, "inf" | "nan") {
                    return Ok(Value::Float(if body == "inf" { if clean.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY } } else { f64::NAN }));
                }
                self.i = start;
                self.err("Invalid value")
            }
        }
    }
}

fn insert(table: &mut BTreeMap<String, Value>, key: &[String], v: Value) -> Result<(), ()> {
    let (last, prefix) = key.split_last().unwrap();
    let mut cur = table;
    for part in prefix {
        let entry = cur.entry(part.clone()).or_insert_with(|| Value::Table(BTreeMap::new()));
        match entry {
            Value::Table(t) => cur = t,
            _ => return Err(()),
        }
    }
    if cur.contains_key(last) {
        return Err(());
    }
    cur.insert(last.clone(), v);
    Ok(())
}

fn table_at<'t>(root: &'t mut BTreeMap<String, Value>, path: &[String]) -> Option<&'t mut BTreeMap<String, Value>> {
    let mut cur = root;
    for part in path {
        let entry = cur.entry(part.clone()).or_insert_with(|| Value::Table(BTreeMap::new()));
        cur = match entry {
            Value::Table(t) => t,
            Value::Array(items) => match items.last_mut() {
                Some(Value::Table(t)) => t,
                _ => return None,
            },
            _ => return None,
        };
    }
    Some(cur)
}

/// `tomllib.loads(text)` (the caller decoded the bytes as UTF-8).
pub fn loads(text: &str) -> Result<BTreeMap<String, Value>, DecodeError> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = P { c: &chars, i: 0 };
    let mut root: BTreeMap<String, Value> = BTreeMap::new();
    let mut current: Vec<String> = Vec::new();
    let mut declared: Vec<Vec<String>> = Vec::new();
    loop {
        p.ws_nl_comments();
        if p.peek().is_none() {
            return Ok(root);
        }
        if p.peek() == Some('[') {
            let array = p.at("[[");
            p.i += if array { 2 } else { 1 };
            p.ws();
            let key = p.key()?;
            p.ws();
            // tomllib checks the namespace (duplicate/overwrite, reported at the position after the key) before the
            // closing bracket.
            let close = if array { "]]" } else { "]" };
            if array {
                let (last, prefix) = key.split_last().unwrap();
                let parent = match table_at(&mut root, prefix) {
                    Some(t) => t,
                    None => return p.err("Cannot overwrite a value"),
                };
                let entry = parent.entry(last.clone()).or_insert_with(|| Value::Array(Vec::new()));
                match entry {
                    Value::Array(items) => items.push(Value::Table(BTreeMap::new())),
                    _ => return p.err("Cannot overwrite a value"),
                }
            } else {
                if declared.contains(&key) {
                    let tuple = key.iter().map(|k| crate::pytext::py_repr(k)).collect::<Vec<_>>();
                    let shown = if tuple.len() == 1 { format!("({},)", tuple[0]) } else { format!("({})", tuple.join(", ")) };
                    return p.err(&format!("Cannot declare {shown} twice"));
                }
                if table_at(&mut root, &key).is_none() {
                    return p.err("Cannot overwrite a value");
                }
                declared.push(key.clone());
            }
            if !p.at(close) {
                return p.err(if array { "Expected ']]' at the end of an array declaration" } else { "Expected ']' at the end of a table declaration" });
            }
            p.i += close.len();
            current = key;
        } else {
            let key = p.key()?;
            p.ws();
            if p.peek() != Some('=') {
                return p.err("Expected '=' after a key in a key/value pair");
            }
            p.i += 1;
            p.ws();
            let v = p.value()?;
            let table = match table_at(&mut root, &current) {
                Some(t) => t,
                None => return p.err("Cannot overwrite a value"),
            };
            if insert(table, &key, v).is_err() {
                return p.err("Cannot overwrite a value");
            }
        }
        p.ws();
        p.comment();
        match p.peek() {
            None | Some('\n') => {}
            Some('\r') if chars.get(p.i + 1) == Some(&'\n') => {}
            _ => return p.err("Expected newline or end of document after a statement"),
        }
    }
}
