//! The part of Python `tomllib` that `render_sub_env.py` uses: `tomllib.loads` as a syntax gate on the rendered Codex
//! profile, top-level keys of that profile, and the `[dependencies]`-style tables of `Cargo.toml` files (`path` of inline
//! tables and of `[dependencies.<name>]` sub-tables, also under `[target.<cfg>.*]`).
//!
//! Supported TOML: comments, `[table]` / `[[array-of-tables]]` headers with bare / quoted dotted keys, `key = value` with
//! dotted keys, basic and literal strings (single- and multi-line), integers, floats, booleans, date-times (kept as their
//! text), arrays (multi-line, trailing comma) and inline tables. A document outside this subset is reported as a decode
//! error (documented residual: tomllib's exact message and position are not reproduced).

#[derive(Clone, Debug, PartialEq)]
pub enum Toml {
    Str(String),
    Other(String),
    Array(Vec<Toml>),
    Table(Vec<(String, Toml)>),
}

impl Toml {
    pub fn get(&self, k: &str) -> Option<&Toml> {
        match self {
            Toml::Table(t) => t.iter().find(|(a, _)| a == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn keys(&self) -> Vec<String> {
        match self {
            Toml::Table(t) => t.iter().map(|(k, _)| k.clone()).collect(),
            _ => vec![],
        }
    }
    fn table_mut(&mut self) -> Option<&mut Vec<(String, Toml)>> {
        match self {
            Toml::Table(t) => Some(t),
            _ => None,
        }
    }
}

struct P<'a> {
    c: Vec<char>,
    i: usize,
    _s: &'a str,
}

type R<T> = Result<T, String>;

impl<'a> P<'a> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.i += 1;
        }
    }
    fn ws_nl_comments(&mut self) {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\n') | Some('\r') => self.i += 1,
                Some('#') => {
                    while !matches!(self.peek(), None | Some('\n')) {
                        self.i += 1;
                    }
                }
                _ => break,
            }
        }
    }
    fn eol(&mut self) -> R<()> {
        self.ws();
        if self.peek() == Some('#') {
            while !matches!(self.peek(), None | Some('\n')) {
                self.i += 1;
            }
        }
        match self.peek() {
            None => Ok(()),
            Some('\n') => {
                self.i += 1;
                Ok(())
            }
            Some('\r') if self.c.get(self.i + 1) == Some(&'\n') => {
                self.i += 2;
                Ok(())
            }
            Some(ch) => Err(format!("Expected newline or end of document after a statement (found {ch:?})")),
        }
    }
    fn key_part(&mut self) -> R<String> {
        self.ws();
        match self.peek() {
            Some('"') => self.basic_string(),
            Some('\'') => self.literal_string(),
            _ => {
                let s = self.i;
                while matches!(self.peek(), Some(ch) if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
                    self.i += 1;
                }
                if s == self.i {
                    return Err("Invalid key".into());
                }
                Ok(self.c[s..self.i].iter().collect())
            }
        }
    }
    fn key(&mut self) -> R<Vec<String>> {
        let mut parts = vec![self.key_part()?];
        loop {
            self.ws();
            if self.peek() == Some('.') {
                self.i += 1;
                parts.push(self.key_part()?);
            } else {
                return Ok(parts);
            }
        }
    }
    fn basic_string(&mut self) -> R<String> {
        if self.c[self.i..].starts_with(&['"', '"', '"']) {
            self.i += 3;
            if self.peek() == Some('\n') {
                self.i += 1;
            }
            let mut out = String::new();
            loop {
                if self.c[self.i..].starts_with(&['"', '"', '"']) {
                    self.i += 3;
                    return Ok(out);
                }
                match self.peek() {
                    None => return Err("Unterminated string".into()),
                    Some('\\') => {
                        if matches!(self.c.get(self.i + 1), Some('\n') | Some(' ') | Some('\t')) {
                            self.i += 1;
                            while matches!(self.peek(), Some('\n') | Some(' ') | Some('\t') | Some('\r')) {
                                self.i += 1;
                            }
                        } else {
                            out.push(self.escape()?);
                        }
                    }
                    Some(ch) => {
                        out.push(ch);
                        self.i += 1;
                    }
                }
            }
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return Err("Unterminated string".into()),
                Some('"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some('\\') => out.push(self.escape()?),
                Some(ch) => {
                    out.push(ch);
                    self.i += 1;
                }
            }
        }
    }
    fn escape(&mut self) -> R<char> {
        self.i += 1;
        let ch = self.peek().ok_or("Unterminated string")?;
        self.i += 1;
        Ok(match ch {
            'b' => '\u{8}',
            't' => '\t',
            'n' => '\n',
            'f' => '\u{c}',
            'r' => '\r',
            '"' => '"',
            '\\' => '\\',
            'u' | 'U' => {
                let n = if ch == 'u' { 4 } else { 8 };
                let hex: String = self.c.get(self.i..self.i + n).ok_or("Unescaped")?.iter().collect();
                self.i += n;
                char::from_u32(u32::from_str_radix(&hex, 16).map_err(|_| "Invalid hex value")?)
                    .ok_or("Escaped character is not a Unicode scalar value")?
            }
            _ => return Err("Unescaped '\\' in a string".into()),
        })
    }
    fn literal_string(&mut self) -> R<String> {
        if self.c[self.i..].starts_with(&['\'', '\'', '\'']) {
            self.i += 3;
            if self.peek() == Some('\n') {
                self.i += 1;
            }
            let s = self.i;
            loop {
                if self.c[self.i..].starts_with(&['\'', '\'', '\'']) {
                    let v = self.c[s..self.i].iter().collect();
                    self.i += 3;
                    return Ok(v);
                }
                if self.peek().is_none() {
                    return Err("Unterminated string".into());
                }
                self.i += 1;
            }
        }
        self.i += 1;
        let s = self.i;
        loop {
            match self.peek() {
                None | Some('\n') => return Err("Unterminated string".into()),
                Some('\'') => {
                    let v = self.c[s..self.i].iter().collect();
                    self.i += 1;
                    return Ok(v);
                }
                _ => self.i += 1,
            }
        }
    }
    fn value(&mut self) -> R<Toml> {
        self.ws();
        match self.peek() {
            Some('"') => Ok(Toml::Str(self.basic_string()?)),
            Some('\'') => Ok(Toml::Str(self.literal_string()?)),
            Some('[') => {
                self.i += 1;
                let mut items = vec![];
                loop {
                    self.ws_nl_comments();
                    if self.peek() == Some(']') {
                        self.i += 1;
                        return Ok(Toml::Array(items));
                    }
                    items.push(self.value()?);
                    self.ws_nl_comments();
                    match self.peek() {
                        Some(',') => self.i += 1,
                        Some(']') => {
                            self.i += 1;
                            return Ok(Toml::Array(items));
                        }
                        _ => return Err("Unclosed array".into()),
                    }
                }
            }
            Some('{') => {
                self.i += 1;
                let mut t = Toml::Table(vec![]);
                self.ws();
                if self.peek() == Some('}') {
                    self.i += 1;
                    return Ok(t);
                }
                loop {
                    let k = self.key()?;
                    self.ws();
                    if self.peek() != Some('=') {
                        return Err("Expected '=' after a key in a key/value pair".into());
                    }
                    self.i += 1;
                    let v = self.value()?;
                    insert(&mut t, &k, v)?;
                    self.ws();
                    match self.peek() {
                        Some(',') => self.i += 1,
                        Some('}') => {
                            self.i += 1;
                            return Ok(t);
                        }
                        _ => return Err("Unclosed inline table".into()),
                    }
                }
            }
            Some(_) => {
                let s = self.i;
                while matches!(self.peek(), Some(ch) if ch.is_ascii_alphanumeric() || "+-_.:".contains(ch)
                    || (ch == ' ' && self.c.get(self.i + 1).map_or(false, |d| d.is_ascii_digit())
                        && self.i > s && self.c[self.i - 1].is_ascii_digit()))
                {
                    self.i += 1;
                }
                let tok: String = self.c[s..self.i].iter().collect();
                if tok.is_empty() || !valid_scalar(&tok) {
                    return Err("Invalid value".into());
                }
                Ok(Toml::Other(tok))
            }
            None => Err("Invalid value".into()),
        }
    }
}

fn valid_scalar(t: &str) -> bool {
    if t == "true" || t == "false" || t == "inf" || t == "nan" || t == "+inf" || t == "-inf" || t == "+nan" || t == "-nan" {
        return true;
    }
    let first = t.chars().next().unwrap();
    first.is_ascii_digit() || first == '+' || first == '-'
}

fn insert(t: &mut Toml, k: &[String], v: Toml) -> R<()> {
    let tab = t.table_mut().ok_or("Cannot overwrite a value")?;
    if k.len() == 1 {
        if tab.iter().any(|(a, _)| a == &k[0]) {
            return Err("Cannot overwrite a value".into());
        }
        tab.push((k[0].clone(), v));
        return Ok(());
    }
    if !tab.iter().any(|(a, _)| a == &k[0]) {
        tab.push((k[0].clone(), Toml::Table(vec![])));
    }
    let (_, child) = tab.iter_mut().find(|(a, _)| a == &k[0]).unwrap();
    insert(child, &k[1..], v)
}

fn descend<'t>(t: &'t mut Toml, k: &[String], array_last: bool) -> R<&'t mut Toml> {
    let mut cur = t;
    for part in k {
        let tab = cur.table_mut().ok_or("Cannot declare a table twice")?;
        if !tab.iter().any(|(a, _)| a == part) {
            tab.push((part.clone(), Toml::Table(vec![])));
        }
        let (_, child) = tab.iter_mut().find(|(a, _)| a == part).unwrap();
        cur = child;
        if array_last {
            if let Toml::Array(items) = cur {
                cur = items.last_mut().ok_or("bad array of tables")?;
            }
        }
    }
    Ok(cur)
}

/// `tomllib.loads(text)`; `Err` = a decode-error message.
pub fn loads(text: &str) -> R<Toml> {
    let mut p = P { c: text.chars().collect(), i: 0, _s: text };
    let mut root = Toml::Table(vec![]);
    let mut header: Vec<String> = vec![];
    loop {
        p.ws_nl_comments();
        match p.peek() {
            None => break,
            Some('[') => {
                let arr = p.c.get(p.i + 1) == Some(&'[');
                p.i += if arr { 2 } else { 1 };
                let k = p.key()?;
                p.ws();
                let close = if arr { "]]" } else { "]" };
                for ch in close.chars() {
                    if p.peek() != Some(ch) {
                        return Err("Expected ']' at the end of a table declaration".into());
                    }
                    p.i += 1;
                }
                p.eol()?;
                if arr {
                    let parent = descend(&mut root, &k[..k.len() - 1], true)?;
                    let tab = parent.table_mut().ok_or("bad array of tables")?;
                    let last = k.last().unwrap();
                    if !tab.iter().any(|(a, _)| a == last) {
                        tab.push((last.clone(), Toml::Array(vec![])));
                    }
                    let (_, a) = tab.iter_mut().find(|(a, _)| a == last).unwrap();
                    match a {
                        Toml::Array(items) => items.push(Toml::Table(vec![])),
                        _ => return Err("Cannot overwrite a value".into()),
                    }
                } else {
                    descend(&mut root, &k, true)?;
                }
                header = k;
            }
            Some(_) => {
                let k = p.key()?;
                p.ws();
                if p.peek() != Some('=') {
                    return Err("Expected '=' after a key in a key/value pair".into());
                }
                p.i += 1;
                let v = p.value()?;
                p.eol()?;
                let target = descend(&mut root, &header, true)?;
                insert(target, &k, v)?;
            }
        }
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_like() {
        let t = loads("[package]\nname = \"x\"\n[dependencies]\na = { path = \"../a\" }\nb = \"1\"\n[dependencies.c]\npath = '../c'\n[[bin]]\nname = \"q\"\n[[bin]]\nname = \"r\"\narr = [1, 2,\n 3, ]\n").unwrap();
        let deps = t.get("dependencies").unwrap();
        assert_eq!(deps.get("a").unwrap().get("path"), Some(&Toml::Str("../a".into())));
        assert_eq!(deps.get("c").unwrap().get("path"), Some(&Toml::Str("../c".into())));
        assert!(loads("a = \n").is_err());
        assert!(loads("a = 1\na = 2\n").is_err());
    }
}
