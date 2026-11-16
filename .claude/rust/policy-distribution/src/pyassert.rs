//! `assert` statement detector equivalent to the original's
//! `[node.lineno for node in ast.walk(ast.parse(text)) if isinstance(node, ast.Assert)]`.
//!
//! `assert` is a hard keyword of Python 3: the only place the NAME token `assert` can stand in valid code is the start of
//! an `assert` statement. So a lexer that skips comments and every string form (prefixes, triple quotes, escapes and the
//! nested replacement fields of PEP 701 f-strings) and reports the line of each `assert` NAME token finds exactly the
//! nodes `ast.walk` finds. Differences, documented: lines come in source order (ast.walk is breadth-first; the report is a
//! list of violations whose emptiness is the gate decision), and only the syntax errors this lexer can see (null byte,
//! unterminated string) are reported as `SyntaxError`; a file Python cannot parse for other reasons is not flagged.
use agent_runtime_support::pystr::is_python_word;

struct Lex<'a> {
    c: &'a [char],
    i: usize,
    line: usize,
    hits: Vec<usize>,
}

fn is_word(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric() || (!c.is_ascii() && is_python_word(c as u32))
}

fn string_prefix(word: &str) -> Option<(bool, bool)> {
    // (raw, fstring)
    match word.to_ascii_lowercase().as_str() {
        "r" => Some((true, false)),
        "u" | "b" => Some((false, false)),
        "br" | "rb" => Some((true, false)),
        "f" => Some((false, true)),
        "fr" | "rf" => Some((true, true)),
        _ => None,
    }
}

impl<'a> Lex<'a> {
    fn peek(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.c.get(self.i).copied()?;
        self.i += 1;
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }

    /// Code until end of input (top level) or, inside an f-string replacement field, until the field's terminator at
    /// bracket depth 0 (`}` / `:` / `!` handled by the caller through `stop`).
    fn code(&mut self, in_field: bool, record: bool) -> Result<Option<char>, String> {
        let mut depth = 0usize;
        while let Some(ch) = self.peek(0) {
            if in_field && depth == 0 && (ch == '}' || ch == ':' || (ch == '!' && self.peek(1) != Some('='))) {
                return Ok(Some(ch));
            }
            match ch {
                '#' => {
                    while let Some(x) = self.peek(0) {
                        if x == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    self.bump();
                }
                ')' | ']' | '}' => {
                    depth = depth.saturating_sub(1);
                    self.bump();
                }
                '\'' | '"' => self.string(false, false)?,
                c if is_word(c) => {
                    let start = self.i;
                    while self.peek(0).map(is_word).unwrap_or(false) {
                        self.i += 1;
                    }
                    let word: String = self.c[start..self.i].iter().collect();
                    if matches!(self.peek(0), Some('\'') | Some('"')) && !c.is_ascii_digit() {
                        if let Some((raw, f)) = string_prefix(&word) {
                            self.string(raw, f)?;
                            continue;
                        }
                    }
                    if record && word == "assert" {
                        self.hits.push(self.line);
                    }
                }
                _ => {
                    self.bump();
                }
            }
        }
        if in_field {
            return Err("f-string: expecting '}'".into());
        }
        Ok(None)
    }

    fn string(&mut self, raw: bool, fstring: bool) -> Result<(), String> {
        let start_line = self.line;
        let q = self.bump().unwrap();
        let triple = self.peek(0) == Some(q) && self.peek(1) == Some(q);
        if triple {
            self.bump();
            self.bump();
        }
        loop {
            let ch = match self.peek(0) {
                Some(ch) => ch,
                None => {
                    return Err(if triple {
                        format!("unterminated triple-quoted string literal (detected at line {})|{start_line}", self.line)
                    } else {
                        format!("unterminated string literal (detected at line {start_line})|{start_line}")
                    })
                }
            };
            if ch == '\\' {
                self.bump();
                if fstring && !raw && matches!(self.peek(0), Some('{')) {
                    // `\{` is not an escape: the brace still opens a field; leave it for the next turn.
                    continue;
                }
                if self.peek(0).is_some() {
                    self.bump();
                }
                continue;
            }
            if ch == '\n' && !triple {
                return Err(format!("unterminated string literal (detected at line {start_line})|{start_line}"));
            }
            if ch == q {
                if !triple {
                    self.bump();
                    return Ok(());
                }
                if self.peek(1) == Some(q) && self.peek(2) == Some(q) {
                    self.bump();
                    self.bump();
                    self.bump();
                    return Ok(());
                }
            }
            if fstring && ch == '{' {
                if self.peek(1) == Some('{') {
                    self.bump();
                    self.bump();
                    continue;
                }
                self.bump();
                self.field()?;
                continue;
            }
            if fstring && ch == '}' && self.peek(1) == Some('}') {
                self.bump();
                self.bump();
                continue;
            }
            self.bump();
        }
    }

    /// One replacement field after its `{`: expression [`!conv`] [`:spec`] `}`; the spec may hold nested fields.
    fn field(&mut self) -> Result<(), String> {
        let mut stop = self.code(true, false)?;
        if stop == Some('!') {
            self.bump();
            while self.peek(0).map(|c| c != ':' && c != '}').unwrap_or(false) {
                self.bump();
            }
            stop = self.peek(0);
        }
        if stop == Some(':') {
            self.bump();
            loop {
                match self.peek(0) {
                    None => return Err("f-string: expecting '}'".into()),
                    Some('{') => {
                        self.bump();
                        self.field()?;
                    }
                    Some('}') => break,
                    Some(_) => {
                        self.bump();
                    }
                }
            }
        }
        self.bump(); // the closing `}`
        Ok(())
    }
}

/// Line numbers of the `assert` statements in `text`, or the `str(SyntaxError)` the lexer could see.
pub fn assert_lines(text: &str, filename: &str) -> Result<Vec<usize>, String> {
    if text.contains('\0') {
        return Err("source code string cannot contain null bytes".into());
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lex = Lex { c: &chars, i: 0, line: 1, hits: Vec::new() };
    match lex.code(false, true) {
        Ok(_) => Ok(lex.hits),
        Err(msg) => {
            let (msg, line) = match msg.split_once('|') {
                Some((m, l)) => (m.to_string(), l.to_string()),
                None => (msg, lex.line.to_string()),
            };
            Err(format!("{msg} ({filename}, line {line})"))
        }
    }
}
