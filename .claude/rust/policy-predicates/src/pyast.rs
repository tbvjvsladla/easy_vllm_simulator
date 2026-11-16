//! The few facts the predicates read from Python *source text* through `ast`/`inspect` in the original: function
//! definitions with their line spans (`ast.FunctionDef.lineno..end_lineno`, `inspect.getsource`,
//! `ast.get_source_segment`), the str constants inside a span (`ast.Constant` str values, implicit concatenation
//! merged, f-string literal parts), and the tuple of str constants assigned to a top-level name.
//!
//! This is a tokenizer over Python's lexical grammar (strings with prefixes and triple quotes, comments, brackets,
//! backslash continuation, logical lines and their indentation), not a parser: it answers only those structural
//! questions, and answers them from the same tokens CPython's tokenizer produces for well-formed source.

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Name(String),
    /// One string literal token: its decoded value pieces (an f-string yields its literal parts), and whether it is an f-string.
    Str { pieces: Vec<String>, fstring: bool },
    Op(String),
    Number(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    pub end_line: usize,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct Logical {
    pub indent: usize,
    pub first_line: usize,
    pub last_line: usize,
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone)]
pub struct FuncDef {
    pub name: String,
    pub is_async: bool,
    pub indent: usize,
    /// line of `def` (ast lineno)
    pub def_line: usize,
    /// first decorator line (inspect.getsource start)
    pub start_line: usize,
    pub end_line: usize,
    /// index of the def's logical line
    pub logical: usize,
}

fn decode_escapes(raw: &str, bytes_literal: bool) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' || i + 1 >= chars.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let n = chars[i + 1];
        i += 2;
        match n {
            '\n' => {}
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            'f' => out.push('\x0c'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\x0b'),
            '0'..='7' => {
                let mut v = n.to_digit(8).unwrap();
                let mut k = 0;
                while k < 2 && i < chars.len() && chars[i].is_digit(8) {
                    v = v * 8 + chars[i].to_digit(8).unwrap();
                    i += 1;
                    k += 1;
                }
                out.push(char::from_u32(v).unwrap_or('\u{fffd}'));
            }
            'x' | 'u' | 'U' if !(bytes_literal && n != 'x') => {
                let len = match n { 'x' => 2, 'u' => 4, _ => 8 };
                let hex: String = chars[i..(i + len).min(chars.len())].iter().collect();
                if let Ok(v) = u32::from_str_radix(&hex, 16) {
                    out.push(char::from_u32(v).unwrap_or('\u{fffd}'));
                    i += len;
                } else {
                    out.push('\\');
                    out.push(n);
                }
            }
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// f-string literal parts: text outside `{...}` replacement fields, `{{`/`}}` unescaped.
fn fstring_pieces(body: &str, raw: bool) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut pieces = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' {
            if i + 1 < chars.len() && chars[i + 1] == '{' {
                cur.push('{');
                i += 2;
                continue;
            }
            if !cur.is_empty() {
                pieces.push(if raw { cur.clone() } else { decode_escapes(&cur, false) });
                cur.clear();
            }
            // skip the replacement field (nested braces and strings inside it)
            let mut depth = 0;
            let mut quote: Option<char> = None;
            while i < chars.len() {
                let d = chars[i];
                if let Some(q) = quote {
                    if d == q {
                        quote = None;
                    }
                } else if d == '\'' || d == '"' {
                    quote = Some(d);
                } else if d == '{' {
                    depth += 1;
                } else if d == '}' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            continue;
        }
        if c == '}' && i + 1 < chars.len() && chars[i + 1] == '}' {
            cur.push('}');
            i += 2;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    if !cur.is_empty() {
        pieces.push(if raw { cur } else { decode_escapes(&cur, false) });
    }
    pieces
}

pub fn tokenize(src: &str) -> Vec<Logical> {
    let chars: Vec<char> = src.chars().collect();
    let mut logicals: Vec<Logical> = Vec::new();
    let mut cur: Option<Logical> = None;
    let mut line = 1usize;
    let mut depth = 0usize;
    let mut i = 0usize;
    let mut at_line_start = true;
    let mut col = 0usize;
    let mut continuation = false;
    while i < chars.len() {
        let c = chars[i];
        if at_line_start && depth == 0 && !continuation {
            // measure indentation of a physical line
            let mut j = i;
            let mut ind = 0;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t' || chars[j] == '\x0c') {
                ind += if chars[j] == '\t' { 8 - ind % 8 } else { 1 };
                j += 1;
            }
            col = ind;
            i = j;
            at_line_start = false;
            continue;
        }
        at_line_start = false;
        match c {
            '\n' => {
                if depth == 0 && !continuation {
                    if let Some(l) = cur.take() {
                        logicals.push(l);
                    }
                }
                continuation = false;
                line += 1;
                i += 1;
                at_line_start = true;
            }
            ' ' | '\t' | '\x0c' | '\r' => i += 1,
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '\\' if i + 1 < chars.len() && (chars[i + 1] == '\n' || (chars[i + 1] == '\r' && chars.get(i + 2) == Some(&'\n'))) => {
                continuation = true;
                i += 1;
                if chars[i] == '\r' {
                    i += 1;
                }
            }
            _ => {
                let start_line = line;
                let tok;
                // string literal (with prefix)?
                let mut j = i;
                while j < chars.len() && j - i < 2 && "rRbBuUfF".contains(chars[j]) {
                    j += 1;
                }
                let is_str = j < chars.len() && (chars[j] == '\'' || chars[j] == '"')
                    && (j == i || chars[i..j].iter().all(|c| "rRbBuUfF".contains(*c)));
                if is_str && (j == i || !chars[i].is_alphanumeric() || true) && (j == i || is_prefix(&chars[i..j])) {
                    let prefix: String = chars[i..j].iter().collect::<String>().to_lowercase();
                    let q = chars[j];
                    let triple = j + 2 < chars.len() && chars[j + 1] == q && chars[j + 2] == q;
                    let mut k = if triple { j + 3 } else { j + 1 };
                    let body_start = k;
                    let body_end;
                    loop {
                        if k >= chars.len() {
                            body_end = k;
                            break;
                        }
                        let d = chars[k];
                        if d == '\\' {
                            if k + 1 < chars.len() && chars[k + 1] == '\n' {
                                line += 1;
                            }
                            k += 2;
                            continue;
                        }
                        if d == '\n' {
                            line += 1;
                            if !triple {
                                body_end = k;
                                break;
                            }
                        }
                        if d == q {
                            if !triple {
                                body_end = k;
                                k += 1;
                                break;
                            }
                            if k + 2 < chars.len() && chars[k + 1] == q && chars[k + 2] == q {
                                body_end = k;
                                k += 3;
                                break;
                            }
                        }
                        k += 1;
                    }
                    let body: String = chars[body_start..body_end.min(chars.len())].iter().collect();
                    let raw = prefix.contains('r');
                    let fstring = prefix.contains('f');
                    let pieces = if fstring {
                        fstring_pieces(&body, raw)
                    } else if raw {
                        vec![body]
                    } else {
                        vec![decode_escapes(&body, prefix.contains('b'))]
                    };
                    tok = Tok::Str { pieces, fstring };
                    i = k;
                } else if c.is_alphabetic() || c == '_' {
                    let mut k = i;
                    while k < chars.len() && (chars[k].is_alphanumeric() || chars[k] == '_') {
                        k += 1;
                    }
                    tok = Tok::Name(chars[i..k].iter().collect());
                    i = k;
                } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).map_or(false, |d| d.is_ascii_digit())) {
                    let mut k = i;
                    while k < chars.len() && (chars[k].is_alphanumeric() || chars[k] == '.' || chars[k] == '_') {
                        k += 1;
                    }
                    tok = Tok::Number(chars[i..k].iter().collect());
                    i = k;
                } else {
                    let three: String = chars[i..(i + 3).min(chars.len())].iter().collect();
                    let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
                    let op = if ["**=", "//=", ">>=", "<<=", "...", "!=="].contains(&three.as_str()) {
                        three
                    } else if ["==", "!=", "<=", ">=", "->", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "**", "//", "<<", ">>", ":=", "@="].contains(&two.as_str()) {
                        two
                    } else {
                        c.to_string()
                    };
                    i += op.chars().count();
                    match op.as_str() {
                        "(" | "[" | "{" => depth += 1,
                        ")" | "]" | "}" => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                    let d_after = depth;
                    let t = Token { tok: Tok::Op(op), line: start_line, end_line: line, depth: d_after };
                    push_tok(&mut cur, t, col, start_line, line);
                    continue;
                }
                let t = Token { tok, line: start_line, end_line: line, depth };
                push_tok(&mut cur, t, col, start_line, line);
            }
        }
    }
    if let Some(l) = cur.take() {
        logicals.push(l);
    }
    logicals
}

fn is_prefix(p: &[char]) -> bool {
    let s: String = p.iter().collect::<String>().to_lowercase();
    matches!(s.as_str(), "r" | "u" | "b" | "f" | "br" | "rb" | "fr" | "rf")
}

fn push_tok(cur: &mut Option<Logical>, t: Token, col: usize, start_line: usize, end_line: usize) {
    match cur {
        Some(l) => {
            l.last_line = end_line;
            l.tokens.push(t);
        }
        None => {
            *cur = Some(Logical { indent: col, first_line: start_line, last_line: end_line, tokens: vec![t] });
        }
    }
}

/// Every function definition (any nesting level), in source order.
pub fn functions(logicals: &[Logical]) -> Vec<FuncDef> {
    let mut out = Vec::new();
    for (idx, l) in logicals.iter().enumerate() {
        let toks = &l.tokens;
        let (is_async, name_at) = match toks.first().map(|t| &t.tok) {
            Some(Tok::Name(n)) if n == "def" => (false, 1),
            Some(Tok::Name(n)) if n == "async" && matches!(toks.get(1).map(|t| &t.tok), Some(Tok::Name(d)) if d == "def") => (true, 2),
            _ => continue,
        };
        let name = match toks.get(name_at).map(|t| &t.tok) {
            Some(Tok::Name(n)) => n.clone(),
            _ => continue,
        };
        // body: the def line itself when it has an inline body, else the following deeper logical lines
        let mut end_line = l.last_line;
        let mut k = idx + 1;
        while k < logicals.len() && logicals[k].indent > l.indent {
            end_line = logicals[k].last_line;
            k += 1;
        }
        let mut start_line = l.first_line;
        let mut d = idx;
        while d > 0 {
            let p = &logicals[d - 1];
            if p.indent == l.indent && matches!(p.tokens.first().map(|t| &t.tok), Some(Tok::Op(o)) if o == "@") {
                start_line = p.first_line;
                d -= 1;
            } else {
                break;
            }
        }
        out.push(FuncDef { name, is_async, indent: l.indent, def_line: l.first_line, start_line, end_line, logical: idx });
    }
    out
}

/// Whole physical lines `from..=to` (1-based), joined with their original line ends.
pub fn lines(src: &str, from: usize, to: usize) -> String {
    let mut out = String::new();
    for (n, l) in src.split_inclusive('\n').enumerate() {
        let n = n + 1;
        if n >= from && n <= to {
            out.push_str(l);
        }
    }
    out
}

/// `ast.get_source_segment(src, node)` of a top-level def: from the `def` line to the end of its last body line.
pub fn segment(src: &str, f: &FuncDef) -> String {
    let s = lines(src, f.def_line, f.end_line);
    s.strip_suffix('\n').unwrap_or(&s).to_string()
}

/// `inspect.getsource(fn)`: decorators included, whole lines.
pub fn getsource(src: &str, f: &FuncDef) -> String {
    lines(src, f.start_line, f.end_line)
}

/// The str constants (`ast.Constant` with a str value) inside the logical lines `from..to` (logical indices, exclusive
/// end): adjacent string tokens are one constant (implicit concatenation); an f-string group yields its literal parts.
pub fn str_constants(logicals: &[Logical], from: usize, to: usize) -> Vec<String> {
    let mut out = Vec::new();
    for l in &logicals[from..to.min(logicals.len())] {
        let mut group: Vec<&Token> = Vec::new();
        let flush = |group: &mut Vec<&Token>, out: &mut Vec<String>| {
            if group.is_empty() {
                return;
            }
            let any_f = group.iter().any(|t| matches!(&t.tok, Tok::Str { fstring: true, .. }));
            if any_f {
                // JoinedStr: adjacent literal text merges into one Constant per run between fields
                for t in group.iter() {
                    if let Tok::Str { pieces, .. } = &t.tok {
                        out.extend(pieces.iter().cloned());
                    }
                }
            } else {
                let mut s = String::new();
                for t in group.iter() {
                    if let Tok::Str { pieces, .. } = &t.tok {
                        s.push_str(&pieces.concat());
                    }
                }
                out.push(s);
            }
            group.clear();
        };
        for t in &l.tokens {
            if matches!(t.tok, Tok::Str { .. }) {
                group.push(t);
            } else {
                flush(&mut group, &mut out);
            }
        }
        flush(&mut group, &mut out);
    }
    out
}

/// str constants of a function's own span (its def logical line through its body).
pub fn function_str_constants(logicals: &[Logical], f: &FuncDef) -> Vec<String> {
    let mut end = f.logical + 1;
    while end < logicals.len() && logicals[end].indent > f.indent {
        end += 1;
    }
    str_constants(logicals, f.logical, end)
}

/// For every top-level `NAME = <tuple>` assignment, the str elements of the tuple (the last assignment wins, as the
/// original's loop overwrites `found`). `Some(None)` = assigned but not a tuple; `None` = never assigned.
pub fn top_level_tuple_of_str(logicals: &[Logical], name: &str) -> Option<Option<Vec<String>>> {
    let mut found = None;
    for l in logicals.iter().filter(|l| l.indent == 0) {
        let t = &l.tokens;
        if t.len() < 3 {
            continue;
        }
        let is_target = matches!(&t[0].tok, Tok::Name(n) if n == name) && matches!(&t[1].tok, Tok::Op(o) if o == "=");
        if !is_target {
            continue;
        }
        let value = &t[2..];
        // a tuple: parenthesised with a top-level comma (or `()`), or a bare comma-separated list
        let paren = matches!(&value[0].tok, Tok::Op(o) if o == "(")
            && matches!(&value[value.len() - 1].tok, Tok::Op(o) if o == ")");
        let base = if paren { 1 } else { 0 };
        let commas = value.iter().filter(|x| matches!(&x.tok, Tok::Op(o) if o == ",") && x.depth == base).count();
        let empty = paren && value.len() == 2;
        if !(commas > 0 || empty) {
            found = Some(None);
            continue;
        }
        let mut elts = Vec::new();
        let mut cur = String::new();
        let mut in_str = false;
        for x in value {
            match &x.tok {
                Tok::Str { pieces, .. } if x.depth == base => {
                    cur.push_str(&pieces.concat());
                    in_str = true;
                }
                Tok::Op(o) if o == "," && x.depth == base => {
                    if in_str {
                        elts.push(std::mem::take(&mut cur));
                    }
                    in_str = false;
                }
                _ => {}
            }
        }
        if in_str {
            elts.push(cur);
        }
        found = Some(Some(elts));
    }
    found
}
