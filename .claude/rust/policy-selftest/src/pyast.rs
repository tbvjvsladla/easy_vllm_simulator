//! A Python 3.12 source parser producing the slice of `ast` the originals inspect.
//!
//! The originals call `ast.parse(text)` and then `ast.walk(tree)` (breadth first, children in `_fields` order) looking
//! for `Assign`, `Call`, `Compare`, `FunctionDef`, `Assert`, `Constant` and `Name` nodes and their `lineno` /
//! `end_lineno`. This module builds a tree with every node that can have children (statements, expressions and the
//! helper nodes `arguments`, `arg`, `keyword`, `comprehension`, `ExceptHandler`, `withitem`) in CPython's field order
//! and with CPython's line spans, so a breadth-first walk visits the inspected nodes in the same order at the same
//! depth. Leaf-only nodes the originals never look at (`Load`, operators, `alias`) are omitted: a leaf changes neither
//! the depth nor the relative order of any other node. `match` statements are not supported (the repository has none);
//! they raise a syntax error like any other unsupported text.
use num_bigint::BigInt;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum Const {
    None,
    Bool(bool),
    Int(BigInt),
    Float(f64),
    Complex,
    Str(String),
    Bytes(Vec<u8>),
    Ellipsis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    NotEq,
    Lt,
    LtE,
    Gt,
    GtE,
    Is,
    IsNot,
    In,
    NotIn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Module,
    FunctionDef,
    AsyncFunctionDef,
    ClassDef,
    Return,
    Delete,
    Assign,
    AugAssign,
    AnnAssign,
    For,
    While,
    If,
    With,
    Raise,
    Try,
    Assert,
    Import,
    Global,
    Expr,
    Pass,
    TypeAlias,
    Arguments,
    Arg,
    Keyword,
    Comprehension,
    ExceptHandler,
    WithItem,
    TypeParam,
    BoolOp,
    NamedExpr,
    BinOp,
    UnaryOp,
    Lambda,
    IfExp,
    Dict,
    Set,
    ListComp,
    SetComp,
    DictComp,
    GeneratorExp,
    Await,
    Yield,
    YieldFrom,
    Compare,
    Call,
    FormattedValue,
    JoinedStr,
    Constant,
    Attribute,
    Subscript,
    Starred,
    Name,
    List,
    Tuple,
    Slice,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: Kind,
    pub line: usize,
    pub end_line: usize,
    pub children: Vec<Node>,
    /// `Name.id`, `Attribute.attr`, `FunctionDef.name`, `keyword.arg`.
    pub name: String,
    pub constant: Option<Const>,
    pub ops: Vec<CmpOp>,
    /// `Assign`: number of targets (children = targets + value). `Call`: number of positional args
    /// (children = func + args + keywords).
    pub count: usize,
}

impl Node {
    fn new(kind: Kind, line: usize, end_line: usize, children: Vec<Node>) -> Node {
        Node { kind, line, end_line, children, name: String::new(), constant: None, ops: Vec::new(), count: 0 }
    }
}

/// `ast.walk(tree)`: breadth-first, children in field order.
pub fn walk(root: &Node) -> Vec<&Node> {
    let mut out = Vec::new();
    let mut q = VecDeque::new();
    q.push_back(root);
    while let Some(n) = q.pop_front() {
        out.push(n);
        for c in &n.children {
            q.push_back(c);
        }
    }
    out
}

// ───────────────────────────────────────────────────────────── tokens
#[derive(Clone, Debug, PartialEq)]
enum Tk {
    Name(String),
    Number(String),
    /// prefix (lowercase), body char range, triple
    Str { prefix: String, start: usize, end: usize },
    Op(&'static str),
    Newline,
    Indent,
    Dedent,
    End,
}

#[derive(Clone, Debug)]
struct Tok {
    tk: Tk,
    line: usize,
    end_line: usize,
}

const OPS: [&str; 49] = [
    "**=", "//=", ">>=", "<<=", "...", "!=", "%=", "&=", "**", "*=", "+=", "-=", "->", "//", "/=", ":=", "<<", "<=",
    "==", ">=", ">>", "@=", "^=", "|=", "%", "&", "(", ")", "*", "+", ",", "-", ".", "/", ":", ";", "<", "=", ">", "@",
    "[", "]", "^", "{", "|", "}", "~", "!", "\\",
];

struct Lexer<'a> {
    s: &'a [char],
    i: usize,
    end: usize,
    line: usize,
    toks: Vec<Tok>,
    depth: usize,
    indents: Vec<usize>,
    at_line_start: bool,
    expr_mode: bool,
}

fn is_id_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}
fn is_id_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric() || matches!(c, '\u{300}'..='\u{36f}')
}

impl<'a> Lexer<'a> {
    fn peek(&self, k: usize) -> Option<char> {
        if self.i + k < self.end {
            Some(self.s[self.i + k])
        } else {
            None
        }
    }

    fn push(&mut self, tk: Tk, line: usize) {
        let end_line = self.line;
        self.toks.push(Tok { tk, line, end_line });
    }

    fn run(mut self) -> Result<Vec<Tok>, String> {
        loop {
            if self.at_line_start && !self.expr_mode && self.depth == 0 {
                // measure indentation
                let mut col = 0usize;
                let mut j = self.i;
                while j < self.end {
                    match self.s[j] {
                        ' ' => col += 1,
                        '\t' => col = (col / 8 + 1) * 8,
                        '\x0c' => col = 0,
                        _ => break,
                    }
                    j += 1;
                }
                // blank / comment-only line?
                let c = if j < self.end { Some(self.s[j]) } else { None };
                match c {
                    None => {
                        self.i = j;
                    }
                    Some('#') | Some('\n') | Some('\r') => {
                        self.i = j;
                        // skip comment
                        if c == Some('#') {
                            while self.i < self.end && self.s[self.i] != '\n' && self.s[self.i] != '\r' {
                                self.i += 1;
                            }
                        }
                        if self.i < self.end {
                            self.eat_newline_char();
                            continue;
                        }
                    }
                    Some('\\') if j + 1 < self.end && matches!(self.s[j + 1], '\n' | '\r') => {
                        // line continuation at line start: treat as part of logical line
                        self.i = j;
                        self.at_line_start = false;
                        self.indent_to(col)?;
                    }
                    Some(_) => {
                        self.i = j;
                        self.at_line_start = false;
                        self.indent_to(col)?;
                    }
                }
                if self.i >= self.end {
                    break;
                }
            }
            if self.i >= self.end {
                break;
            }
            let c = self.s[self.i];
            match c {
                ' ' | '\t' | '\x0c' => self.i += 1,
                '#' => {
                    while self.i < self.end && self.s[self.i] != '\n' && self.s[self.i] != '\r' {
                        self.i += 1;
                    }
                }
                '\n' | '\r' => {
                    let l = self.line;
                    self.eat_newline_char();
                    if self.depth == 0 && !self.expr_mode {
                        self.toks.push(Tok { tk: Tk::Newline, line: l, end_line: l });
                        self.at_line_start = true;
                    }
                }
                '\\' if matches!(self.peek(1), Some('\n') | Some('\r')) => {
                    self.i += 1;
                    self.eat_newline_char();
                }
                c if c.is_ascii_digit() || (c == '.' && self.peek(1).map_or(false, |d| d.is_ascii_digit())) => {
                    let l = self.line;
                    let t = self.number();
                    self.push(Tk::Number(t), l);
                }
                c if is_id_start(c) => {
                    let l = self.line;
                    let st = self.i;
                    while self.i < self.end && is_id_char(self.s[self.i]) {
                        self.i += 1;
                    }
                    let word: String = self.s[st..self.i].iter().collect();
                    if let Some(q) = self.peek(0) {
                        if (q == '"' || q == '\'') && is_string_prefix(&word) {
                            let prefix = word.to_lowercase();
                            let (bs, be) = self.string(&prefix)?;
                            self.push(Tk::Str { prefix, start: bs, end: be }, l);
                            continue;
                        }
                    }
                    self.push(Tk::Name(word), l);
                }
                '"' | '\'' => {
                    let l = self.line;
                    let (bs, be) = self.string("")?;
                    self.push(Tk::Str { prefix: String::new(), start: bs, end: be }, l);
                }
                _ => {
                    let l = self.line;
                    let mut found = None;
                    for op in OPS.iter() {
                        let n = op.chars().count();
                        if self.i + n <= self.end && self.s[self.i..self.i + n].iter().copied().eq(op.chars()) {
                            found = Some(*op);
                            break;
                        }
                    }
                    let op = found.ok_or_else(|| format!("invalid character {c:?} at line {}", self.line))?;
                    self.i += op.chars().count();
                    match op {
                        "(" | "[" | "{" => self.depth += 1,
                        ")" | "]" | "}" => self.depth = self.depth.saturating_sub(1),
                        _ => {}
                    }
                    self.push(Tk::Op(op), l);
                }
            }
        }
        if !self.expr_mode {
            if !matches!(self.toks.last().map(|t| &t.tk), Some(Tk::Newline) | None) {
                let l = self.line;
                self.toks.push(Tok { tk: Tk::Newline, line: l, end_line: l });
            }
            while self.indents.len() > 1 {
                self.indents.pop();
                let l = self.line;
                self.toks.push(Tok { tk: Tk::Dedent, line: l, end_line: l });
            }
        }
        let l = self.line;
        self.toks.push(Tok { tk: Tk::End, line: l, end_line: l });
        Ok(self.toks)
    }

    fn indent_to(&mut self, col: usize) -> Result<(), String> {
        let cur = *self.indents.last().unwrap();
        let l = self.line;
        if col > cur {
            self.indents.push(col);
            self.toks.push(Tok { tk: Tk::Indent, line: l, end_line: l });
        } else {
            while col < *self.indents.last().unwrap() {
                self.indents.pop();
                self.toks.push(Tok { tk: Tk::Dedent, line: l, end_line: l });
            }
            if col != *self.indents.last().unwrap() {
                return Err(format!("unindent does not match any outer indentation level (line {l})"));
            }
        }
        Ok(())
    }

    fn eat_newline_char(&mut self) {
        if self.s[self.i] == '\r' && self.peek(1) == Some('\n') {
            self.i += 2;
        } else {
            self.i += 1;
        }
        self.line += 1;
    }

    fn number(&mut self) -> String {
        let st = self.i;
        let c0 = self.s[self.i];
        if c0 == '0' && matches!(self.peek(1), Some('x' | 'X' | 'o' | 'O' | 'b' | 'B')) {
            self.i += 2;
            while self.i < self.end && (self.s[self.i].is_ascii_hexdigit() || self.s[self.i] == '_') {
                self.i += 1;
            }
        } else {
            while self.i < self.end && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '_') {
                self.i += 1;
            }
            if self.peek(0) == Some('.') {
                self.i += 1;
                while self.i < self.end && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '_') {
                    self.i += 1;
                }
            }
            if matches!(self.peek(0), Some('e' | 'E'))
                && (self.peek(1).map_or(false, |d| d.is_ascii_digit())
                    || (matches!(self.peek(1), Some('+' | '-')) && self.peek(2).map_or(false, |d| d.is_ascii_digit())))
            {
                self.i += 2;
                while self.i < self.end && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '_') {
                    self.i += 1;
                }
            }
        }
        if matches!(self.peek(0), Some('j' | 'J')) {
            self.i += 1;
        }
        self.s[st..self.i].iter().collect()
    }

    /// Consume a string literal starting at the quote; returns the body char range.
    fn string(&mut self, prefix: &str) -> Result<(usize, usize), String> {
        let (bs, be, ni, nl) = scan_string(self.s, self.i, self.end, prefix, self.line)?;
        self.i = ni;
        self.line = nl;
        Ok((bs, be))
    }
}

fn is_string_prefix(w: &str) -> bool {
    matches!(
        w.to_lowercase().as_str(),
        "r" | "u" | "b" | "f" | "br" | "rb" | "fr" | "rf"
    )
}

/// Scan a string literal whose opening quote is at `i`. Returns (body_start, body_end, next_index, next_line).
fn scan_string(s: &[char], i: usize, end: usize, prefix: &str, line: usize) -> Result<(usize, usize, usize, usize), String> {
    let q = s[i];
    let triple = i + 2 < end && s[i + 1] == q && s[i + 2] == q;
    let qlen = if triple { 3 } else { 1 };
    let raw = prefix.contains('r');
    let fstr = prefix.contains('f');
    let mut j = i + qlen;
    let mut line = line;
    let bs = j;
    loop {
        if j >= end {
            return Err(format!("unterminated string literal (line {line})"));
        }
        let c = s[j];
        if c == '\\' && fstr && j + 1 < end && matches!(s[j + 1], '{' | '}') {
            // in an f-string a backslash does not escape a brace
            j += 1;
            continue;
        }
        if c == '\\' {
            if j + 1 < end && s[j + 1] == '\r' && j + 2 < end && s[j + 2] == '\n' {
                j += 3;
                line += 1;
            } else {
                if j + 1 < end && matches!(s[j + 1], '\n' | '\r') {
                    line += 1;
                }
                j += 2;
            }
            continue;
        }
        if fstr && c == '{' {
            if j + 1 < end && s[j + 1] == '{' {
                j += 2;
                continue;
            }
            let (nj, nl) = scan_fexpr(s, j + 1, end, line)?;
            j = nj;
            line = nl;
            continue;
        }
        if c == '\n' || c == '\r' {
            if !triple {
                return Err(format!("unterminated string literal (line {line})"));
            }
            if c == '\r' && j + 1 < end && s[j + 1] == '\n' {
                j += 1;
            }
            line += 1;
            j += 1;
            continue;
        }
        if c == q && (!triple || (j + 2 < end && s[j + 1] == q && s[j + 2] == q)) {
            let _ = raw;
            return Ok((bs, j, j + qlen, line));
        }
        j += 1;
    }
}

/// Scan an f-string replacement field starting after `{`; returns the index after the closing `}`.
fn scan_fexpr(s: &[char], mut j: usize, end: usize, mut line: usize) -> Result<(usize, usize), String> {
    let mut depth = 0usize;
    loop {
        if j >= end {
            return Err("unterminated f-string field".into());
        }
        let c = s[j];
        match c {
            '\'' | '"' => {
                // nested string: find its prefix letters already passed; scan as plain (prefix letters precede)
                let mut k = j;
                let mut pre = String::new();
                while k > 0 && s[k - 1].is_ascii_alphabetic() {
                    k -= 1;
                }
                if k < j {
                    let w: String = s[k..j].iter().collect();
                    if is_string_prefix(&w) {
                        pre = w.to_lowercase();
                    }
                }
                let (_, _, nj, nl) = scan_string(s, j, end, &pre, line)?;
                j = nj;
                line = nl;
            }
            '(' | '[' | '{' => {
                depth += 1;
                j += 1;
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                j += 1;
            }
            '}' => {
                if depth == 0 {
                    return Ok((j + 1, line));
                }
                depth -= 1;
                j += 1;
            }
            '!' if depth == 0 && !(j + 1 < end && s[j + 1] == '=') => {
                j += 1;
            }
            ':' if depth == 0 => {
                // format spec: literal text with nested fields until the closing brace
                j += 1;
                loop {
                    if j >= end {
                        return Err("unterminated f-string spec".into());
                    }
                    match s[j] {
                        '{' => {
                            let (nj, nl) = scan_fexpr(s, j + 1, end, line)?;
                            j = nj;
                            line = nl;
                        }
                        '}' => return Ok((j + 1, line)),
                        '\n' => {
                            line += 1;
                            j += 1;
                        }
                        _ => j += 1,
                    }
                }
            }
            '\n' => {
                line += 1;
                j += 1;
            }
            '#' => return Err("f-string expression part cannot include '#'".into()),
            _ => j += 1,
        }
    }
}

// ───────────────────────────────────────────────────────────── string values
fn decode_escapes(body: &[char], bytes: bool) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let c = body[i];
        if c != '\\' || i + 1 >= body.len() {
            out.push(c as u32);
            i += 1;
            continue;
        }
        let n = body[i + 1];
        i += 2;
        match n {
            '\n' => {}
            '\r' => {
                if i < body.len() && body[i] == '\n' {
                    i += 1;
                }
            }
            '\\' => out.push('\\' as u32),
            '\'' => out.push('\'' as u32),
            '"' => out.push('"' as u32),
            'a' => out.push(7),
            'b' => out.push(8),
            'f' => out.push(12),
            'n' => out.push(10),
            'r' => out.push(13),
            't' => out.push(9),
            'v' => out.push(11),
            '0'..='7' => {
                let mut v = n.to_digit(8).unwrap();
                let mut k = 0;
                while k < 2 && i < body.len() && body[i].is_digit(8) {
                    v = v * 8 + body[i].to_digit(8).unwrap();
                    i += 1;
                    k += 1;
                }
                out.push(v);
            }
            'x' => {
                let h: String = body.get(i..i + 2).map(|x| x.iter().collect()).unwrap_or_default();
                let v = u32::from_str_radix(&h, 16).map_err(|_| "truncated \\xXX escape".to_string())?;
                i += 2;
                out.push(v);
            }
            'u' | 'U' if !bytes => {
                let w = if n == 'u' { 4 } else { 8 };
                let h: String = body.get(i..i + w).map(|x| x.iter().collect()).unwrap_or_default();
                let v = u32::from_str_radix(&h, 16).map_err(|_| "truncated \\uXXXX escape".to_string())?;
                i += w;
                out.push(v);
            }
            'N' if !bytes => {
                // \N{NAME}: the name table is not available; keep a placeholder code point per escape.
                while i < body.len() && body[i] != '}' {
                    i += 1;
                }
                i += 1;
                out.push(0xfffd);
            }
            other => {
                out.push('\\' as u32);
                out.push(other as u32);
            }
        }
    }
    Ok(out)
}

fn points_to_string(p: &[u32]) -> String {
    p.iter().map(|&v| char::from_u32(v).unwrap_or('\u{fffd}')).collect()
}

fn parse_number(text: &str) -> Result<Const, String> {
    let t: String = text.chars().filter(|&c| c != '_').collect();
    let low = t.to_lowercase();
    if low.ends_with('j') {
        return Ok(Const::Complex);
    }
    let radix = if low.starts_with("0x") {
        Some(16)
    } else if low.starts_with("0o") {
        Some(8)
    } else if low.starts_with("0b") {
        Some(2)
    } else {
        None
    };
    if let Some(r) = radix {
        return BigInt::parse_bytes(low[2..].as_bytes(), r).map(Const::Int).ok_or_else(|| format!("bad number {text}"));
    }
    if low.contains('.') || low.contains('e') {
        return low.parse::<f64>().map(Const::Float).map_err(|_| format!("bad number {text}"));
    }
    BigInt::parse_bytes(low.as_bytes(), 10).map(Const::Int).ok_or_else(|| format!("bad number {text}"))
}

// ───────────────────────────────────────────────────────────── parser
/// An expression with its source span (including the parentheses this production consumed).
struct E {
    n: Node,
    sl: usize,
    el: usize,
}

struct Parser<'a> {
    src: &'a [char],
    t: Vec<Tok>,
    p: usize,
}

type R<T> = Result<T, String>;

fn leaf(kind: Kind, line: usize, end_line: usize) -> Node {
    Node::new(kind, line, end_line, Vec::new())
}

impl<'a> Parser<'a> {
    fn tk(&self) -> &Tk {
        &self.t[self.p].tk
    }
    fn tk_at(&self, k: usize) -> &Tk {
        &self.t[(self.p + k).min(self.t.len() - 1)].tk
    }
    fn line(&self) -> usize {
        self.t[self.p].line
    }
    fn prev_end(&self) -> usize {
        self.t[self.p.saturating_sub(1)].end_line
    }
    fn is_op(&self, op: &str) -> bool {
        matches!(self.tk(), Tk::Op(o) if *o == op)
    }
    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.tk(), Tk::Name(n) if n == kw)
    }
    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.p += 1;
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.is_kw(kw) {
            self.p += 1;
            true
        } else {
            false
        }
    }
    fn expect_op(&mut self, op: &str) -> R<()> {
        if self.eat_op(op) {
            Ok(())
        } else {
            Err(format!("expected '{op}' at line {} got {:?}", self.line(), self.tk()))
        }
    }
    fn name(&mut self) -> R<String> {
        match self.tk().clone() {
            Tk::Name(n) if !is_keyword(&n) => {
                self.p += 1;
                Ok(n)
            }
            other => Err(format!("expected name at line {} got {other:?}", self.line())),
        }
    }

    // ── module / statements
    fn module(&mut self) -> R<Node> {
        let mut body = Vec::new();
        while !matches!(self.tk(), Tk::End) {
            if matches!(self.tk(), Tk::Newline) {
                self.p += 1;
                continue;
            }
            body.extend(self.statement()?);
        }
        let el = body.last().map_or(1, |n: &Node| n.end_line);
        Ok(Node::new(Kind::Module, 1, el, body))
    }

    fn statement(&mut self) -> R<Vec<Node>> {
        if self.is_op("@") {
            return Ok(vec![self.decorated()?]);
        }
        let kw = if let Tk::Name(n) = self.tk() { n.clone() } else { String::new() };
        match kw.as_str() {
            "if" => Ok(vec![self.if_stmt()?]),
            "while" => Ok(vec![self.while_stmt()?]),
            "for" => Ok(vec![self.for_stmt(self.line())?]),
            "try" => Ok(vec![self.try_stmt()?]),
            "with" => Ok(vec![self.with_stmt(self.line())?]),
            "def" => Ok(vec![self.funcdef(Vec::new(), self.line(), false)?]),
            "class" => Ok(vec![self.classdef(Vec::new())?]),
            "async" => {
                let l = self.line();
                self.p += 1;
                if self.is_kw("def") {
                    Ok(vec![self.funcdef(Vec::new(), l, true)?])
                } else if self.is_kw("for") {
                    Ok(vec![self.for_stmt(l)?])
                } else if self.is_kw("with") {
                    Ok(vec![self.with_stmt(l)?])
                } else {
                    Err("bad async".into())
                }
            }
            "match" if self.looks_like_match() => Err("match statements are not supported".into()),
            _ => self.simple_stmts(),
        }
    }

    fn looks_like_match(&self) -> bool {
        // `match <subject>:` NEWLINE INDENT `case`
        let mut k = 1;
        let mut depth = 0i32;
        loop {
            match self.tk_at(k) {
                Tk::Op(o) if matches!(*o, "(" | "[" | "{") => depth += 1,
                Tk::Op(o) if matches!(*o, ")" | "]" | "}") => depth -= 1,
                Tk::Op(":") if depth == 0 => return matches!(self.tk_at(k + 1), Tk::Newline),
                Tk::Op("=") if depth == 0 => return false,
                Tk::Newline | Tk::End => return false,
                _ => {}
            }
            k += 1;
        }
    }

    fn block(&mut self) -> R<Vec<Node>> {
        self.expect_op(":")?;
        if matches!(self.tk(), Tk::Newline) {
            self.p += 1;
            while matches!(self.tk(), Tk::Newline) {
                self.p += 1;
            }
            if !matches!(self.tk(), Tk::Indent) {
                return Err(format!("expected an indented block at line {}", self.line()));
            }
            self.p += 1;
            let mut body = Vec::new();
            while !matches!(self.tk(), Tk::Dedent | Tk::End) {
                if matches!(self.tk(), Tk::Newline) {
                    self.p += 1;
                    continue;
                }
                body.extend(self.statement()?);
            }
            if matches!(self.tk(), Tk::Dedent) {
                self.p += 1;
            }
            Ok(body)
        } else {
            self.simple_stmts()
        }
    }

    fn decorated(&mut self) -> R<Node> {
        let mut decos = Vec::new();
        while self.eat_op("@") {
            decos.push(self.named_expression()?.n);
            if !matches!(self.tk(), Tk::Newline) {
                return Err("expected newline after decorator".into());
            }
            self.p += 1;
        }
        if self.is_kw("def") {
            let l = self.line();
            self.funcdef(decos, l, false)
        } else if self.is_kw("async") {
            let l = self.line();
            self.p += 1;
            self.funcdef(decos, l, true)
        } else if self.is_kw("class") {
            self.classdef(decos)
        } else {
            Err("bad decorator target".into())
        }
    }

    fn type_params(&mut self) -> R<Vec<Node>> {
        let mut out = Vec::new();
        if !self.is_op("[") {
            return Ok(out);
        }
        self.p += 1;
        while !self.is_op("]") {
            let l = self.line();
            self.eat_op("*");
            self.eat_op("**");
            self.name()?;
            let mut ch = Vec::new();
            if self.eat_op(":") {
                ch.push(self.expression()?.n);
            }
            out.push(Node::new(Kind::TypeParam, l, self.prev_end(), ch));
            if !self.eat_op(",") {
                break;
            }
        }
        self.expect_op("]")?;
        Ok(out)
    }

    fn funcdef(&mut self, decos: Vec<Node>, line: usize, is_async: bool) -> R<Node> {
        if !self.eat_kw("def") {
            return Err("expected def".into());
        }
        let name = self.name()?;
        let tparams = self.type_params()?;
        self.expect_op("(")?;
        let args = self.params(")", true)?;
        self.expect_op(")")?;
        let mut returns = None;
        if self.eat_op("->") {
            returns = Some(self.expression()?.n);
        }
        let body = self.block()?;
        let el = body.last().map_or(line, |n| n.end_line);
        let mut ch = vec![args];
        ch.extend(body);
        ch.extend(decos);
        if let Some(r) = returns {
            ch.push(r);
        }
        ch.extend(tparams);
        let mut n = Node::new(if is_async { Kind::AsyncFunctionDef } else { Kind::FunctionDef }, line, el, ch);
        n.name = name;
        Ok(n)
    }

    fn classdef(&mut self, decos: Vec<Node>) -> R<Node> {
        let line = self.line();
        self.eat_kw("class");
        let name = self.name()?;
        let tparams = self.type_params()?;
        let mut bases = Vec::new();
        let mut kws = Vec::new();
        if self.eat_op("(") {
            let (a, k) = self.call_args()?;
            bases = a;
            kws = k;
            self.expect_op(")")?;
        }
        let body = self.block()?;
        let el = body.last().map_or(line, |n| n.end_line);
        let mut ch = bases;
        ch.extend(kws);
        ch.extend(body);
        ch.extend(decos);
        ch.extend(tparams);
        let mut n = Node::new(Kind::ClassDef, line, el, ch);
        n.name = name;
        Ok(n)
    }

    /// Function / lambda parameters up to `close`; returns an `arguments` node.
    fn params(&mut self, close: &str, annotations: bool) -> R<Node> {
        let line = self.line();
        let mut posonly: Vec<Node> = Vec::new();
        let mut args: Vec<Node> = Vec::new();
        let mut vararg: Option<Node> = None;
        let mut kwonly: Vec<Node> = Vec::new();
        let mut kw_defaults: Vec<Node> = Vec::new();
        let mut kwarg: Option<Node> = None;
        let mut defaults: Vec<Node> = Vec::new();
        let mut star_seen = false;
        while !self.is_op(close) {
            if self.eat_op("/") {
                posonly.append(&mut args);
            } else if self.eat_op("**") {
                kwarg = Some(self.param(annotations, true)?);
            } else if self.eat_op("*") {
                star_seen = true;
                if !self.is_op(",") && !self.is_op(close) {
                    vararg = Some(self.param(annotations, true)?);
                }
            } else {
                let a = self.param(annotations, false)?;
                let d = if self.eat_op("=") { Some(self.expression()?.n) } else { None };
                if star_seen {
                    kwonly.push(a);
                    if let Some(d) = d {
                        kw_defaults.push(d);
                    }
                } else {
                    args.push(a);
                    if let Some(d) = d {
                        defaults.push(d);
                    }
                }
            }
            if !self.eat_op(",") {
                break;
            }
        }
        let mut ch = posonly;
        ch.extend(args);
        ch.extend(vararg);
        ch.extend(kwonly);
        ch.extend(kw_defaults);
        ch.extend(kwarg);
        ch.extend(defaults);
        Ok(Node::new(Kind::Arguments, line, self.prev_end(), ch))
    }

    fn param(&mut self, annotations: bool, star: bool) -> R<Node> {
        let l = self.line();
        let name = self.name()?;
        let mut ch = Vec::new();
        if annotations && self.eat_op(":") {
            if star && self.is_op("*") {
                ch.push(self.star_expression()?.n);
            } else {
                ch.push(self.expression()?.n);
            }
        }
        let mut n = Node::new(Kind::Arg, l, self.prev_end(), ch);
        n.name = name;
        Ok(n)
    }

    fn if_stmt(&mut self) -> R<Node> {
        let line = self.line();
        self.p += 1; // if / elif
        let test = self.named_expression()?.n;
        let body = self.block()?;
        let mut orelse = Vec::new();
        if self.is_kw("elif") {
            orelse.push(self.if_stmt()?);
        } else if self.eat_kw("else") {
            orelse = self.block()?;
        }
        let el = orelse.last().or(body.last()).map_or(line, |n| n.end_line);
        let mut ch = vec![test];
        ch.extend(body);
        ch.extend(orelse);
        Ok(Node::new(Kind::If, line, el, ch))
    }

    fn while_stmt(&mut self) -> R<Node> {
        let line = self.line();
        self.p += 1;
        let test = self.named_expression()?.n;
        let body = self.block()?;
        let mut orelse = Vec::new();
        if self.eat_kw("else") {
            orelse = self.block()?;
        }
        let el = orelse.last().or(body.last()).map_or(line, |n| n.end_line);
        let mut ch = vec![test];
        ch.extend(body);
        ch.extend(orelse);
        Ok(Node::new(Kind::While, line, el, ch))
    }

    fn for_stmt(&mut self, line: usize) -> R<Node> {
        self.eat_kw("for");
        let target = self.target_list()?.n;
        if !self.eat_kw("in") {
            return Err("expected in".into());
        }
        let iter = self.star_expressions()?.n;
        let body = self.block()?;
        let mut orelse = Vec::new();
        if self.eat_kw("else") {
            orelse = self.block()?;
        }
        let el = orelse.last().or(body.last()).map_or(line, |n| n.end_line);
        let mut ch = vec![target, iter];
        ch.extend(body);
        ch.extend(orelse);
        Ok(Node::new(Kind::For, line, el, ch))
    }

    fn try_stmt(&mut self) -> R<Node> {
        let line = self.line();
        self.p += 1;
        let body = self.block()?;
        let mut handlers = Vec::new();
        while self.is_kw("except") {
            let hl = self.line();
            self.p += 1;
            self.eat_op("*");
            let mut ch = Vec::new();
            if !self.is_op(":") {
                ch.push(self.expression_list_until_as()?.n);
                if self.eat_kw("as") {
                    self.name()?;
                }
            }
            let hb = self.block()?;
            let hel = hb.last().map_or(hl, |n| n.end_line);
            ch.extend(hb);
            handlers.push(Node::new(Kind::ExceptHandler, hl, hel, ch));
        }
        let mut orelse = Vec::new();
        if self.eat_kw("else") {
            orelse = self.block()?;
        }
        let mut finalbody = Vec::new();
        if self.eat_kw("finally") {
            finalbody = self.block()?;
        }
        let el = finalbody
            .last()
            .or(orelse.last())
            .or(handlers.last())
            .or(body.last())
            .map_or(line, |n| n.end_line);
        let mut ch = body;
        ch.extend(handlers);
        ch.extend(orelse);
        ch.extend(finalbody);
        Ok(Node::new(Kind::Try, line, el, ch))
    }

    /// `except A, B` is not valid 3.x; `except (A, B)` is an expression.
    fn expression_list_until_as(&mut self) -> R<E> {
        self.expression()
    }

    fn with_stmt(&mut self, line: usize) -> R<Node> {
        self.eat_kw("with");
        let mut items = Vec::new();
        let paren_items = self.is_op("(") && self.paren_with_items();
        if paren_items {
            self.p += 1;
            while !self.is_op(")") {
                items.push(self.with_item()?);
                if !self.eat_op(",") {
                    break;
                }
            }
            self.expect_op(")")?;
        } else {
            loop {
                items.push(self.with_item()?);
                if !self.eat_op(",") {
                    break;
                }
            }
        }
        let body = self.block()?;
        let el = body.last().map_or(line, |n| n.end_line);
        let mut ch = items;
        ch.extend(body);
        Ok(Node::new(Kind::With, line, el, ch))
    }

    fn paren_with_items(&self) -> bool {
        // '(' ... ')' ':' with a top-level `as` or `,` inside
        let mut k = 1;
        let mut depth = 1i32;
        let mut has = false;
        loop {
            match self.tk_at(k) {
                Tk::Op(o) if matches!(*o, "(" | "[" | "{") => depth += 1,
                Tk::Op(o) if matches!(*o, ")" | "]" | "}") => {
                    depth -= 1;
                    if depth == 0 {
                        return has && matches!(self.tk_at(k + 1), Tk::Op(":"));
                    }
                }
                Tk::Name(n) if n == "as" && depth == 1 => has = true,
                Tk::Op(",") if depth == 1 => has = true,
                Tk::End => return false,
                _ => {}
            }
            k += 1;
        }
    }

    fn with_item(&mut self) -> R<Node> {
        let ctx = self.expression()?;
        let mut ch = vec![ctx.n];
        let mut el = ctx.el;
        if self.eat_kw("as") {
            let t = self.single_target()?;
            el = t.el;
            ch.push(t.n);
        }
        Ok(Node::new(Kind::WithItem, ctx.sl, el, ch))
    }

    fn simple_stmts(&mut self) -> R<Vec<Node>> {
        let mut out = Vec::new();
        loop {
            out.push(self.simple_stmt()?);
            if !self.eat_op(";") {
                break;
            }
            if matches!(self.tk(), Tk::Newline) {
                break;
            }
        }
        match self.tk() {
            Tk::Newline => {
                self.p += 1;
                Ok(out)
            }
            Tk::End => Ok(out),
            other => Err(format!("invalid syntax at line {} near {other:?}", self.line())),
        }
    }

    fn simple_stmt(&mut self) -> R<Node> {
        let line = self.line();
        let kw = if let Tk::Name(n) = self.tk() { n.clone() } else { String::new() };
        match kw.as_str() {
            "pass" | "break" | "continue" => {
                self.p += 1;
                return Ok(leaf(Kind::Pass, line, line));
            }
            "return" => {
                self.p += 1;
                let mut ch = Vec::new();
                if !self.at_stmt_end() {
                    ch.push(self.star_expressions()?.n);
                }
                return Ok(Node::new(Kind::Return, line, self.prev_end(), ch));
            }
            "raise" => {
                self.p += 1;
                let mut ch = Vec::new();
                if !self.at_stmt_end() {
                    ch.push(self.expression()?.n);
                    if self.eat_kw("from") {
                        ch.push(self.expression()?.n);
                    }
                }
                return Ok(Node::new(Kind::Raise, line, self.prev_end(), ch));
            }
            "global" | "nonlocal" => {
                self.p += 1;
                loop {
                    self.name()?;
                    if !self.eat_op(",") {
                        break;
                    }
                }
                return Ok(leaf(Kind::Global, line, self.prev_end()));
            }
            "del" => {
                self.p += 1;
                let mut ch = Vec::new();
                loop {
                    ch.push(self.bitwise_or()?.n);
                    if !self.eat_op(",") || self.at_stmt_end() {
                        break;
                    }
                }
                return Ok(Node::new(Kind::Delete, line, self.prev_end(), ch));
            }
            "assert" => {
                self.p += 1;
                let mut ch = vec![self.expression()?.n];
                if self.eat_op(",") {
                    ch.push(self.expression()?.n);
                }
                return Ok(Node::new(Kind::Assert, line, self.prev_end(), ch));
            }
            "import" => {
                self.p += 1;
                loop {
                    self.dotted_name()?;
                    if self.eat_kw("as") {
                        self.name()?;
                    }
                    if !self.eat_op(",") {
                        break;
                    }
                }
                return Ok(leaf(Kind::Import, line, self.prev_end()));
            }
            "from" => {
                self.p += 1;
                while self.eat_op(".") || self.eat_op("...") {}
                if !self.is_kw("import") {
                    self.dotted_name()?;
                }
                if !self.eat_kw("import") {
                    return Err("expected import".into());
                }
                if self.eat_op("*") {
                    return Ok(leaf(Kind::Import, line, self.prev_end()));
                }
                let paren = self.eat_op("(");
                loop {
                    if paren && self.is_op(")") {
                        break;
                    }
                    self.name()?;
                    if self.eat_kw("as") {
                        self.name()?;
                    }
                    if !self.eat_op(",") {
                        break;
                    }
                }
                if paren {
                    self.expect_op(")")?;
                }
                return Ok(leaf(Kind::Import, line, self.prev_end()));
            }
            "type" if matches!(self.tk_at(1), Tk::Name(_)) && matches!(self.tk_at(2), Tk::Op("=") | Tk::Op("[")) => {
                self.p += 1;
                let nl = self.line();
                let nm = self.name()?;
                let mut name = leaf(Kind::Name, nl, nl);
                name.name = nm;
                let tp = self.type_params()?;
                self.expect_op("=")?;
                let v = self.expression()?;
                let mut ch = vec![name];
                ch.extend(tp);
                ch.push(v.n);
                return Ok(Node::new(Kind::TypeAlias, line, v.el, ch));
            }
            _ => {}
        }
        // expression statement / assignment
        let first = self.star_expressions_or_yield()?;
        if self.is_op("=") {
            let mut parts = vec![first];
            while self.eat_op("=") {
                parts.push(self.star_expressions_or_yield()?);
            }
            let value = parts.pop().unwrap();
            let el = value.el;
            let count = parts.len();
            let mut ch: Vec<Node> = parts.into_iter().map(|e| e.n).collect();
            ch.push(value.n);
            let mut n = Node::new(Kind::Assign, line, el, ch);
            n.count = count;
            return Ok(n);
        }
        if let Tk::Op(o) = self.tk() {
            if matches!(*o, "+=" | "-=" | "*=" | "/=" | "//=" | "%=" | "**=" | ">>=" | "<<=" | "&=" | "^=" | "|=" | "@=") {
                self.p += 1;
                let v = self.star_expressions_or_yield()?;
                return Ok(Node::new(Kind::AugAssign, line, v.el, vec![first.n, v.n]));
            }
            if *o == ":" {
                self.p += 1;
                let ann = self.expression()?;
                let mut el = ann.el;
                let mut ch = vec![first.n, ann.n];
                if self.eat_op("=") {
                    let v = self.star_expressions_or_yield()?;
                    el = v.el;
                    ch.push(v.n);
                }
                return Ok(Node::new(Kind::AnnAssign, line, el, ch));
            }
        }
        Ok(Node::new(Kind::Expr, first.sl, first.el, vec![first.n]))
    }

    fn at_stmt_end(&self) -> bool {
        matches!(self.tk(), Tk::Newline | Tk::End | Tk::Op(";"))
    }

    fn dotted_name(&mut self) -> R<()> {
        self.name()?;
        while self.eat_op(".") {
            self.name()?;
        }
        Ok(())
    }

    // ── expressions
    fn star_expressions_or_yield(&mut self) -> R<E> {
        if self.is_kw("yield") {
            self.yield_expr()
        } else {
            self.star_expressions()
        }
    }

    fn yield_expr(&mut self) -> R<E> {
        let l = self.line();
        self.p += 1;
        if self.eat_kw("from") {
            let v = self.expression()?;
            let el = v.el;
            return Ok(E { n: Node::new(Kind::YieldFrom, l, el, vec![v.n]), sl: l, el });
        }
        if self.at_stmt_end() || self.is_op(")") || self.is_op("]") || self.is_op("}") || self.is_op("=") {
            return Ok(E { n: leaf(Kind::Yield, l, l), sl: l, el: l });
        }
        let v = self.star_expressions()?;
        let el = v.el;
        Ok(E { n: Node::new(Kind::Yield, l, el, vec![v.n]), sl: l, el })
    }

    /// Comma list of star_expression; a Tuple when there is a comma.
    fn star_expressions(&mut self) -> R<E> {
        let first = self.star_expression()?;
        if !self.is_op(",") {
            return Ok(first);
        }
        let sl = first.sl;
        let mut el = first.el;
        let mut elts = vec![first.n];
        while self.eat_op(",") {
            el = self.prev_end();
            if self.expr_can_start() {
                let e = self.star_expression()?;
                el = e.el;
                elts.push(e.n);
            } else {
                break;
            }
        }
        Ok(E { n: Node::new(Kind::Tuple, sl, el, elts), sl, el })
    }

    fn expr_can_start(&self) -> bool {
        match self.tk() {
            Tk::Name(n) => !is_keyword(n) || matches!(n.as_str(), "None" | "True" | "False" | "not" | "lambda" | "await" | "yield"),
            Tk::Number(_) | Tk::Str { .. } => true,
            Tk::Op(o) => matches!(*o, "(" | "[" | "{" | "-" | "+" | "~" | "*" | "..." | "**"),
            _ => false,
        }
    }

    fn star_expression(&mut self) -> R<E> {
        if self.is_op("*") {
            let l = self.line();
            self.p += 1;
            let v = self.bitwise_or()?;
            let el = v.el;
            return Ok(E { n: Node::new(Kind::Starred, l, el, vec![v.n]), sl: l, el });
        }
        self.expression()
    }

    fn star_named_expression(&mut self) -> R<E> {
        if self.is_op("*") {
            return self.star_expression();
        }
        self.named_expression()
    }

    fn named_expression(&mut self) -> R<E> {
        if let Tk::Name(n) = self.tk() {
            if !is_keyword(n) && matches!(self.tk_at(1), Tk::Op(":=")) {
                let l = self.line();
                let mut target = leaf(Kind::Name, l, l);
                target.name = n.clone();
                self.p += 2;
                let v = self.expression()?;
                let el = v.el;
                return Ok(E { n: Node::new(Kind::NamedExpr, l, el, vec![target, v.n]), sl: l, el });
            }
        }
        self.expression()
    }

    fn expression(&mut self) -> R<E> {
        if self.is_kw("lambda") {
            return self.lambdef();
        }
        let body = self.disjunction()?;
        if self.is_kw("if") {
            // conditional expression (not a comprehension `if`: callers stop before `for`)
            self.p += 1;
            let test = self.disjunction()?;
            if !self.eat_kw("else") {
                return Err(format!("expected 'else' at line {}", self.line()));
            }
            let orelse = self.expression()?;
            let (sl, el) = (body.sl, orelse.el);
            return Ok(E { n: Node::new(Kind::IfExp, sl, el, vec![test.n, body.n, orelse.n]), sl, el });
        }
        Ok(body)
    }

    fn lambdef(&mut self) -> R<E> {
        let l = self.line();
        self.p += 1;
        let args = self.params(":", false)?;
        self.expect_op(":")?;
        let body = self.expression()?;
        let el = body.el;
        Ok(E { n: Node::new(Kind::Lambda, l, el, vec![args, body.n]), sl: l, el })
    }

    fn disjunction(&mut self) -> R<E> {
        self.boolop("or", Self::conjunction)
    }
    fn conjunction(&mut self) -> R<E> {
        self.boolop("and", Self::inversion)
    }
    fn boolop(&mut self, kw: &str, next: fn(&mut Self) -> R<E>) -> R<E> {
        let first = next(self)?;
        if !self.is_kw(kw) {
            return Ok(first);
        }
        let sl = first.sl;
        let mut el = first.el;
        let mut vals = vec![first.n];
        while self.eat_kw(kw) {
            let e = next(self)?;
            el = e.el;
            vals.push(e.n);
        }
        Ok(E { n: Node::new(Kind::BoolOp, sl, el, vals), sl, el })
    }
    fn inversion(&mut self) -> R<E> {
        if self.is_kw("not") {
            let l = self.line();
            self.p += 1;
            let v = self.inversion()?;
            let el = v.el;
            return Ok(E { n: Node::new(Kind::UnaryOp, l, el, vec![v.n]), sl: l, el });
        }
        self.comparison()
    }
    fn comparison(&mut self) -> R<E> {
        let left = self.bitwise_or()?;
        let mut ops = Vec::new();
        let mut comps = Vec::new();
        let mut el = left.el;
        loop {
            let op = match self.tk() {
                Tk::Op("==") => CmpOp::Eq,
                Tk::Op("!=") => CmpOp::NotEq,
                Tk::Op("<") => CmpOp::Lt,
                Tk::Op("<=") => CmpOp::LtE,
                Tk::Op(">") => CmpOp::Gt,
                Tk::Op(">=") => CmpOp::GtE,
                Tk::Name(n) if n == "in" => CmpOp::In,
                Tk::Name(n) if n == "is" => {
                    if matches!(self.tk_at(1), Tk::Name(m) if m == "not") {
                        self.p += 1;
                        CmpOp::IsNot
                    } else {
                        CmpOp::Is
                    }
                }
                Tk::Name(n) if n == "not" && matches!(self.tk_at(1), Tk::Name(m) if m == "in") => {
                    self.p += 1;
                    CmpOp::NotIn
                }
                _ => break,
            };
            self.p += 1;
            let r = self.bitwise_or()?;
            el = r.el;
            ops.push(op);
            comps.push(r.n);
        }
        if ops.is_empty() {
            return Ok(left);
        }
        let sl = left.sl;
        let mut ch = vec![left.n];
        ch.extend(comps);
        let mut n = Node::new(Kind::Compare, sl, el, ch);
        n.ops = ops;
        Ok(E { n, sl, el })
    }

    fn binary(&mut self, ops: &[&str], next: fn(&mut Self) -> R<E>) -> R<E> {
        let mut left = next(self)?;
        loop {
            let hit = matches!(self.tk(), Tk::Op(o) if ops.contains(o));
            if !hit {
                return Ok(left);
            }
            self.p += 1;
            let r = next(self)?;
            let (sl, el) = (left.sl, r.el);
            left = E { n: Node::new(Kind::BinOp, sl, el, vec![left.n, r.n]), sl, el };
        }
    }
    fn bitwise_or(&mut self) -> R<E> {
        self.binary(&["|"], Self::bitwise_xor)
    }
    fn bitwise_xor(&mut self) -> R<E> {
        self.binary(&["^"], Self::bitwise_and)
    }
    fn bitwise_and(&mut self) -> R<E> {
        self.binary(&["&"], Self::shift)
    }
    fn shift(&mut self) -> R<E> {
        self.binary(&["<<", ">>"], Self::sum)
    }
    fn sum(&mut self) -> R<E> {
        self.binary(&["+", "-"], Self::term)
    }
    fn term(&mut self) -> R<E> {
        self.binary(&["*", "/", "//", "%", "@"], Self::factor)
    }
    fn factor(&mut self) -> R<E> {
        if matches!(self.tk(), Tk::Op("+") | Tk::Op("-") | Tk::Op("~")) {
            let l = self.line();
            self.p += 1;
            let v = self.factor()?;
            let el = v.el;
            return Ok(E { n: Node::new(Kind::UnaryOp, l, el, vec![v.n]), sl: l, el });
        }
        self.power()
    }
    fn power(&mut self) -> R<E> {
        let base = self.await_primary()?;
        if self.eat_op("**") {
            let r = self.factor()?;
            let (sl, el) = (base.sl, r.el);
            return Ok(E { n: Node::new(Kind::BinOp, sl, el, vec![base.n, r.n]), sl, el });
        }
        Ok(base)
    }
    fn await_primary(&mut self) -> R<E> {
        if self.is_kw("await") {
            let l = self.line();
            self.p += 1;
            let v = self.primary()?;
            let el = v.el;
            return Ok(E { n: Node::new(Kind::Await, l, el, vec![v.n]), sl: l, el });
        }
        self.primary()
    }

    fn primary(&mut self) -> R<E> {
        let mut e = self.atom()?;
        loop {
            if self.eat_op(".") {
                let attr = self.name_any()?;
                let el = self.prev_end();
                let mut n = Node::new(Kind::Attribute, e.sl, el, vec![e.n]);
                n.name = attr;
                e = E { n, sl: e.sl, el };
            } else if self.is_op("(") {
                self.p += 1;
                let (args, kws) = self.call_args()?;
                self.expect_op(")")?;
                let el = self.prev_end();
                let mut ch = vec![e.n];
                let count = args.len();
                ch.extend(args);
                ch.extend(kws);
                let mut n = Node::new(Kind::Call, e.sl, el, ch);
                n.count = count;
                e = E { n, sl: e.sl, el };
            } else if self.is_op("[") {
                self.p += 1;
                let sl_node = self.slices()?;
                self.expect_op("]")?;
                let el = self.prev_end();
                e = E { n: Node::new(Kind::Subscript, e.sl, el, vec![e.n, sl_node]), sl: e.sl, el };
            } else {
                return Ok(e);
            }
        }
    }

    fn name_any(&mut self) -> R<String> {
        match self.tk().clone() {
            Tk::Name(n) => {
                self.p += 1;
                Ok(n)
            }
            other => Err(format!("expected attribute name got {other:?}")),
        }
    }

    fn slices(&mut self) -> R<Node> {
        let l = self.line();
        let first = self.slice_item()?;
        if !self.is_op(",") {
            return Ok(first.n);
        }
        let mut el = first.el;
        let mut elts = vec![first.n];
        while self.eat_op(",") {
            el = self.prev_end();
            if self.is_op("]") {
                break;
            }
            let e = self.slice_item()?;
            el = e.el;
            elts.push(e.n);
        }
        Ok(Node::new(Kind::Tuple, l, el, elts))
    }

    fn slice_item(&mut self) -> R<E> {
        let l = self.line();
        let mut lower = None;
        if !self.is_op(":") {
            let e = self.star_named_expression()?;
            if !self.is_op(":") {
                return Ok(e);
            }
            lower = Some(e);
        }
        self.expect_op(":")?;
        let mut ch = Vec::new();
        let sl = lower.as_ref().map_or(l, |e| e.sl);
        if let Some(e) = lower {
            ch.push(e.n);
        }
        let mut el = self.prev_end();
        if !self.is_op(":") && !self.is_op(",") && !self.is_op("]") {
            let u = self.expression()?;
            el = u.el;
            ch.push(u.n);
        }
        if self.eat_op(":") {
            el = self.prev_end();
            if !self.is_op(",") && !self.is_op("]") {
                let s = self.expression()?;
                el = s.el;
                ch.push(s.n);
            }
        }
        Ok(E { n: Node::new(Kind::Slice, sl, el, ch), sl, el })
    }

    /// Call arguments up to `)` (not consumed): (positional args, keyword nodes).
    fn call_args(&mut self) -> R<(Vec<Node>, Vec<Node>)> {
        let mut args = Vec::new();
        let mut kws = Vec::new();
        while !self.is_op(")") {
            let l = self.line();
            if self.eat_op("**") {
                let v = self.expression()?;
                kws.push(Node::new(Kind::Keyword, l, v.el, vec![v.n]));
            } else if self.is_op("*") {
                args.push(self.star_expression()?.n);
            } else if matches!(self.tk(), Tk::Name(n) if !is_keyword(n)) && matches!(self.tk_at(1), Tk::Op("=")) {
                let name = self.name()?;
                self.p += 1;
                let v = self.expression()?;
                let mut k = Node::new(Kind::Keyword, l, v.el, vec![v.n]);
                k.name = name;
                kws.push(k);
            } else {
                let e = self.named_expression()?;
                if self.is_kw("for") || self.is_kw("async") {
                    let gens = self.comprehensions()?;
                    let el = self.prev_end();
                    let mut ch = vec![e.n];
                    ch.extend(gens);
                    // a bare generator argument spans the call parentheses in CPython; position is the element start
                    args.push(Node::new(Kind::GeneratorExp, e.sl, el, ch));
                } else {
                    args.push(e.n);
                }
            }
            if !self.eat_op(",") {
                break;
            }
        }
        Ok((args, kws))
    }

    fn comprehensions(&mut self) -> R<Vec<Node>> {
        let mut out = Vec::new();
        loop {
            let l = self.line();
            if self.eat_kw("async") {
            }
            if !self.eat_kw("for") {
                break;
            }
            let target = self.target_list()?.n;
            if !self.eat_kw("in") {
                return Err("expected in".into());
            }
            let iter = self.disjunction()?.n;
            let mut ch = vec![target, iter];
            while self.is_kw("if") {
                self.p += 1;
                ch.push(self.disjunction()?.n);
            }
            out.push(Node::new(Kind::Comprehension, l, self.prev_end(), ch));
            if !(self.is_kw("for") || self.is_kw("async")) {
                break;
            }
        }
        Ok(out)
    }

    /// `for` / comprehension targets: comma list at bitwise_or level (no `in` comparison).
    fn target_list(&mut self) -> R<E> {
        let first = self.single_target()?;
        if !self.is_op(",") {
            return Ok(first);
        }
        let sl = first.sl;
        let mut el = first.el;
        let mut elts = vec![first.n];
        while self.eat_op(",") {
            el = self.prev_end();
            if self.is_kw("in") || self.is_op("=") || self.is_op(":") {
                break;
            }
            let e = self.single_target()?;
            el = e.el;
            elts.push(e.n);
        }
        Ok(E { n: Node::new(Kind::Tuple, sl, el, elts), sl, el })
    }

    fn single_target(&mut self) -> R<E> {
        if self.is_op("*") {
            let l = self.line();
            self.p += 1;
            let v = self.bitwise_or()?;
            let el = v.el;
            return Ok(E { n: Node::new(Kind::Starred, l, el, vec![v.n]), sl: l, el });
        }
        self.bitwise_or()
    }

    fn atom(&mut self) -> R<E> {
        let l = self.line();
        let tk = self.tk().clone();
        match tk {
            Tk::Name(n) => {
                let c = match n.as_str() {
                    "None" => Some(Const::None),
                    "True" => Some(Const::Bool(true)),
                    "False" => Some(Const::Bool(false)),
                    _ => None,
                };
                if let Some(c) = c {
                    self.p += 1;
                    let mut nd = leaf(Kind::Constant, l, l);
                    nd.constant = Some(c);
                    return Ok(E { n: nd, sl: l, el: l });
                }
                if is_keyword(&n) {
                    return Err(format!("invalid syntax at line {l}: keyword {n}"));
                }
                self.p += 1;
                let mut nd = leaf(Kind::Name, l, l);
                nd.name = n;
                Ok(E { n: nd, sl: l, el: l })
            }
            Tk::Number(t) => {
                self.p += 1;
                let mut nd = leaf(Kind::Constant, l, l);
                nd.constant = Some(parse_number(&t)?);
                Ok(E { n: nd, sl: l, el: l })
            }
            Tk::Str { .. } => self.strings(),
            Tk::Op("...") => {
                self.p += 1;
                let mut nd = leaf(Kind::Constant, l, l);
                nd.constant = Some(Const::Ellipsis);
                Ok(E { n: nd, sl: l, el: l })
            }
            Tk::Op("(") => {
                self.p += 1;
                if self.eat_op(")") {
                    let el = self.prev_end();
                    return Ok(E { n: Node::new(Kind::Tuple, l, el, Vec::new()), sl: l, el });
                }
                if self.is_kw("yield") {
                    let y = self.yield_expr()?;
                    self.expect_op(")")?;
                    return Ok(E { n: y.n, sl: l, el: self.prev_end() });
                }
                let first = self.star_named_expression()?;
                if self.is_kw("for") || self.is_kw("async") {
                    let gens = self.comprehensions()?;
                    self.expect_op(")")?;
                    let el = self.prev_end();
                    let mut ch = vec![first.n];
                    ch.extend(gens);
                    return Ok(E { n: Node::new(Kind::GeneratorExp, l, el, ch), sl: l, el });
                }
                if self.is_op(",") {
                    let mut elts = vec![first.n];
                    while self.eat_op(",") {
                        if self.is_op(")") {
                            break;
                        }
                        elts.push(self.star_named_expression()?.n);
                    }
                    self.expect_op(")")?;
                    let el = self.prev_end();
                    return Ok(E { n: Node::new(Kind::Tuple, l, el, elts), sl: l, el });
                }
                self.expect_op(")")?;
                Ok(E { n: first.n, sl: l, el: self.prev_end() })
            }
            Tk::Op("[") => {
                self.p += 1;
                if self.eat_op("]") {
                    let el = self.prev_end();
                    return Ok(E { n: Node::new(Kind::List, l, el, Vec::new()), sl: l, el });
                }
                let first = self.star_named_expression()?;
                if self.is_kw("for") || self.is_kw("async") {
                    let gens = self.comprehensions()?;
                    self.expect_op("]")?;
                    let el = self.prev_end();
                    let mut ch = vec![first.n];
                    ch.extend(gens);
                    return Ok(E { n: Node::new(Kind::ListComp, l, el, ch), sl: l, el });
                }
                let mut elts = vec![first.n];
                while self.eat_op(",") {
                    if self.is_op("]") {
                        break;
                    }
                    elts.push(self.star_named_expression()?.n);
                }
                self.expect_op("]")?;
                let el = self.prev_end();
                Ok(E { n: Node::new(Kind::List, l, el, elts), sl: l, el })
            }
            Tk::Op("{") => {
                self.p += 1;
                if self.eat_op("}") {
                    let el = self.prev_end();
                    return Ok(E { n: Node::new(Kind::Dict, l, el, Vec::new()), sl: l, el });
                }
                let mut keys: Vec<Node> = Vec::new();
                let mut vals: Vec<Node> = Vec::new();
                // first item decides dict vs set
                if self.eat_op("**") {
                    vals.push(self.bitwise_or()?.n);
                    return self.dict_rest(l, keys, vals);
                }
                let first = self.star_named_expression()?;
                if self.eat_op(":") {
                    let v = self.expression()?;
                    if self.is_kw("for") || self.is_kw("async") {
                        let gens = self.comprehensions()?;
                        self.expect_op("}")?;
                        let el = self.prev_end();
                        let mut ch = vec![first.n, v.n];
                        ch.extend(gens);
                        return Ok(E { n: Node::new(Kind::DictComp, l, el, ch), sl: l, el });
                    }
                    keys.push(first.n);
                    vals.push(v.n);
                    return self.dict_rest(l, keys, vals);
                }
                if self.is_kw("for") || self.is_kw("async") {
                    let gens = self.comprehensions()?;
                    self.expect_op("}")?;
                    let el = self.prev_end();
                    let mut ch = vec![first.n];
                    ch.extend(gens);
                    return Ok(E { n: Node::new(Kind::SetComp, l, el, ch), sl: l, el });
                }
                let mut elts = vec![first.n];
                while self.eat_op(",") {
                    if self.is_op("}") {
                        break;
                    }
                    elts.push(self.star_named_expression()?.n);
                }
                self.expect_op("}")?;
                let el = self.prev_end();
                Ok(E { n: Node::new(Kind::Set, l, el, elts), sl: l, el })
            }
            other => Err(format!("invalid syntax at line {l}: {other:?}")),
        }
    }

    fn dict_rest(&mut self, l: usize, mut keys: Vec<Node>, mut vals: Vec<Node>) -> R<E> {
        while self.eat_op(",") {
            if self.is_op("}") {
                break;
            }
            if self.eat_op("**") {
                vals.push(self.bitwise_or()?.n);
                continue;
            }
            let k = self.expression()?;
            self.expect_op(":")?;
            let v = self.expression()?;
            keys.push(k.n);
            vals.push(v.n);
        }
        self.expect_op("}")?;
        let el = self.prev_end();
        let mut ch = keys;
        ch.extend(vals);
        Ok(E { n: Node::new(Kind::Dict, l, el, ch), sl: l, el })
    }

    /// Adjacent string literals: a Constant (str or bytes) or a JoinedStr when any part is an f-string.
    fn strings(&mut self) -> R<E> {
        let l = self.line();
        let mut fparts: Vec<Node> = Vec::new();
        let mut any_f = false;
        let mut is_bytes = None;
        let mut text: Vec<u32> = Vec::new();
        let mut el = l;
        while let Tk::Str { prefix, start, end } = self.tk().clone() {
            let tline = self.t[self.p].line;
            el = self.t[self.p].end_line;
            self.p += 1;
            let b = prefix.contains('b');
            if let Some(pb) = is_bytes {
                if pb != b {
                    return Err("cannot mix bytes and nonbytes literals".into());
                }
            }
            is_bytes = Some(b);
            let raw = prefix.contains('r');
            if prefix.contains('f') {
                any_f = true;
                self.fstring_fields(start, end, tline, raw, &mut fparts)?;
            } else {
                let body = &self.src[start..end];
                if raw {
                    text.extend(body.iter().map(|&c| c as u32));
                } else {
                    text.extend(decode_escapes(body, b)?);
                }
            }
        }
        if any_f {
            return Ok(E { n: Node::new(Kind::JoinedStr, l, el, fparts), sl: l, el });
        }
        let mut nd = leaf(Kind::Constant, l, el);
        nd.constant = Some(if is_bytes == Some(true) {
            Const::Bytes(text.iter().map(|&v| v as u8).collect())
        } else {
            Const::Str(points_to_string(&text))
        });
        Ok(E { n: nd, sl: l, el })
    }

    /// FormattedValue children for every replacement field of the f-string body `[start, end)`.
    fn fstring_fields(&self, start: usize, end: usize, line: usize, _raw: bool, out: &mut Vec<Node>) -> R<()> {
        let s = self.src;
        let mut j = start;
        let mut line = line;
        while j < end {
            let c = s[j];
            if c == '\n' {
                line += 1;
                j += 1;
                continue;
            }
            if c == '\\' && !(j + 1 < end && matches!(s[j + 1], '{' | '}')) {
                if j + 1 < end && s[j + 1] == '\n' {
                    line += 1;
                }
                j += 2;
                continue;
            }
            if c == '{' {
                if j + 1 < end && s[j + 1] == '{' {
                    j += 2;
                    continue;
                }
                let (node, nj, nl) = self.fstring_field(j + 1, end, line)?;
                out.push(node);
                j = nj;
                line = nl;
                continue;
            }
            j += 1;
        }
        Ok(())
    }

    /// One replacement field starting after `{`: (FormattedValue, index after `}`, line after).
    fn fstring_field(&self, st: usize, end: usize, line: usize) -> R<(Node, usize, usize)> {
        let s = self.src;
        // find the expression end: first top-level `!` (not `!=`), `:` or `}` or a trailing `=`
        let mut j = st;
        let mut depth = 0i32;
        let mut l = line;
        while j < end {
            match s[j] {
                '\'' | '"' => {
                    let mut k = j;
                    let mut pre = String::new();
                    while k > st && s[k - 1].is_ascii_alphabetic() {
                        k -= 1;
                    }
                    if k < j {
                        let w: String = s[k..j].iter().collect();
                        if is_string_prefix(&w) {
                            pre = w.to_lowercase();
                        }
                    }
                    let (_, _, nj, nl) = scan_string(s, j, end, &pre, l)?;
                    j = nj;
                    l = nl;
                    continue;
                }
                '(' | '[' | '{' => depth += 1,
                ')' | ']' => depth -= 1,
                '}' if depth > 0 => depth -= 1,
                '}' => break,
                '!' if depth == 0 && !(j + 1 < end && s[j + 1] == '=') => break,
                ':' if depth == 0 => break,
                '\n' => l += 1,
                _ => {}
            }
            j += 1;
        }
        let mut expr_end = j;
        // `{x=}` debugging specifier
        {
            let mut k = expr_end;
            while k > st && s[k - 1].is_whitespace() {
                k -= 1;
            }
            if k > st && s[k - 1] == '=' && !(k >= st + 2 && matches!(s[k - 2], '=' | '!' | '<' | '>')) {
                expr_end = k - 1;
            }
        }
        let value = parse_expr_range(s, st, expr_end, line)?;
        let mut ch = vec![value];
        // conversion
        if j < end && s[j] == '!' {
            j += 2;
        }
        if j < end && s[j] == ':' {
            j += 1;
            let spec_line = l;
            let mut spec_parts = Vec::new();
            let spec_start = j;
            // format spec until the matching `}` with nested fields
            loop {
                if j >= end {
                    return Err("unterminated f-string spec".into());
                }
                match s[j] {
                    '{' => {
                        let (n, nj, nl) = self.fstring_field(j + 1, end, l)?;
                        spec_parts.push(n);
                        j = nj;
                        l = nl;
                    }
                    '}' => break,
                    '\n' => {
                        l += 1;
                        j += 1;
                    }
                    _ => j += 1,
                }
            }
            let _ = spec_start;
            ch.push(Node::new(Kind::JoinedStr, spec_line, l, spec_parts));
        }
        if j < end && s[j] == '}' {
            j += 1;
        }
        Ok((Node::new(Kind::FormattedValue, line, l, ch), j, l))
    }
}

fn is_keyword(n: &str) -> bool {
    matches!(
        n,
        "False" | "None" | "True" | "and" | "as" | "assert" | "async" | "await" | "break" | "class" | "continue" | "def"
            | "del" | "elif" | "else" | "except" | "finally" | "for" | "from" | "global" | "if" | "import" | "in" | "is"
            | "lambda" | "nonlocal" | "not" | "or" | "pass" | "raise" | "return" | "try" | "while" | "with" | "yield"
    )
}

fn line_of(s: &[char], idx: usize, base_idx: usize, base_line: usize) -> usize {
    base_line + s[base_idx..idx].iter().filter(|&&c| c == '\n').count()
}

/// Parse the expression text `s[st..end]` (an f-string field) whose first char is on `line`.
fn parse_expr_range(s: &[char], st: usize, end: usize, line: usize) -> R<Node> {
    let lex = Lexer { s, i: st, end, line, toks: Vec::new(), depth: 0, indents: vec![0], at_line_start: false, expr_mode: true };
    let toks = lex.run()?;
    let _ = line_of;
    let mut p = Parser { src: s, t: toks, p: 0 };
    let e = if p.is_kw("yield") { p.yield_expr()? } else { p.star_expressions()? };
    if !matches!(p.tk(), Tk::End) {
        return Err(format!("f-string: invalid expression at line {line}"));
    }
    Ok(e.n)
}

/// `ast.parse(text)`; Err = SyntaxError.
pub fn parse(text: &str) -> Result<Node, String> {
    let chars: Vec<char> = text.chars().collect();
    let lex = Lexer {
        s: &chars,
        i: 0,
        end: chars.len(),
        line: 1,
        toks: Vec::new(),
        depth: 0,
        indents: vec![0],
        at_line_start: true,
        expr_mode: false,
    };
    let toks = lex.run()?;
    let mut p = Parser { src: &chars, t: toks, p: 0 };
    p.module()
}
