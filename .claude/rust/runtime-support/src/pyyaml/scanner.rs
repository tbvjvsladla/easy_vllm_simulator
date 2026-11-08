//! `yaml/scanner.py` and `yaml/tokens.py`: characters to tokens, function by function.

use super::frames::{frame, nested};
use super::reader::Reader;
use super::{marked, Mark, Parsed};
use crate::pystr::PyStr;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum K {
    StreamStart,
    StreamEnd,
    Directive,
    DocumentStart,
    DocumentEnd,
    BlockSequenceStart,
    BlockMappingStart,
    BlockEnd,
    FlowSequenceStart,
    FlowMappingStart,
    FlowSequenceEnd,
    FlowMappingEnd,
    Key,
    Value,
    BlockEntry,
    FlowEntry,
    Alias,
    Anchor,
    Tag,
    Scalar,
}

impl K {
    /// `Token.id`
    pub fn id(self) -> &'static str {
        match self {
            K::StreamStart => "<stream start>",
            K::StreamEnd => "<stream end>",
            K::Directive => "<directive>",
            K::DocumentStart => "<document start>",
            K::DocumentEnd => "<document end>",
            K::BlockSequenceStart => "<block sequence start>",
            K::BlockMappingStart => "<block mapping start>",
            K::BlockEnd => "<block end>",
            K::FlowSequenceStart => "[",
            K::FlowMappingStart => "{",
            K::FlowSequenceEnd => "]",
            K::FlowMappingEnd => "}",
            K::Key => "?",
            K::Value => ":",
            K::BlockEntry => "-",
            K::FlowEntry => ",",
            K::Alias => "<alias>",
            K::Anchor => "<anchor>",
            K::Tag => "<tag>",
            K::Scalar => "<scalar>",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum DirectiveValue {
    None,
    Yaml(i64, i64),
    Tag(String, String),
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub kind: K,
    pub start: Mark,
    pub end: Mark,
    /// Scalar value; anchor / alias / directive name as text.
    pub text: PyStr,
    pub plain: bool,
    pub style: Option<char>,
    /// `(handle, suffix)` of a tag token.
    pub tag: Option<(Option<String>, String)>,
    pub directive: DirectiveValue,
}

fn token(kind: K, start: Mark, end: Mark) -> Parsed<Token> {
    nested(&["__init__"])?;
    Ok(Token {
        kind,
        start,
        end,
        text: PyStr::default(),
        plain: false,
        style: None,
        tag: None,
        directive: DirectiveValue::None,
    })
}

struct SimpleKey {
    token_number: usize,
    required: bool,
    index: usize,
    line: usize,
    column: usize,
    mark: Mark,
}

fn repr_char(c: char) -> String {
    PyStr::new(&c.to_string()).repr()
}

fn is_in(c: char, set: &str) -> bool {
    set.contains(c)
}

const NUL_SPACE_BREAKS: &str = "\0 \t\r\n\u{85}\u{2028}\u{2029}";
const BREAKS: &str = "\r\n\u{85}\u{2028}\u{2029}";
const NUL_BREAKS: &str = "\0\r\n\u{85}\u{2028}\u{2029}";

fn alnum(c: char) -> bool {
    c.is_ascii_digit() || c.is_ascii_uppercase() || c.is_ascii_lowercase() || c == '-' || c == '_'
}

pub(crate) struct Scanner<'a> {
    pub reader: Reader<'a>,
    done: bool,
    flow_level: i64,
    tokens: VecDeque<Token>,
    tokens_taken: usize,
    indent: i64,
    indents: Vec<i64>,
    allow_simple_key: bool,
    possible_simple_keys: Vec<(i64, SimpleKey)>,
}

impl<'a> Scanner<'a> {
    pub fn new(reader: Reader<'a>) -> Parsed<Self> {
        let mut scanner = Scanner {
            reader,
            done: false,
            flow_level: 0,
            tokens: VecDeque::new(),
            tokens_taken: 0,
            indent: -1,
            indents: Vec::new(),
            allow_simple_key: true,
            possible_simple_keys: Vec::new(),
        };
        let mark = scanner.reader.mark()?;
        scanner
            .tokens
            .push_back(token(K::StreamStart, mark.clone(), mark)?);
        Ok(scanner)
    }

    fn mark(&self) -> Parsed<Mark> {
        self.reader.mark()
    }

    fn peek(&mut self, offset: usize) -> Parsed<char> {
        self.reader.peek(offset)
    }

    fn prefix(&mut self, length: usize) -> Parsed<String> {
        Ok(self.reader.prefix(length)?.into_iter().collect())
    }

    fn forward(&mut self, length: usize) -> Parsed<()> {
        self.reader.forward(length)
    }

    fn error(
        &self,
        context: Option<&str>,
        context_mark: Option<&Mark>,
        problem: impl Into<String>,
    ) -> super::Failure {
        match self.mark() {
            Ok(mark) => marked("ScannerError", context, context_mark, problem, Some(&mark)),
            Err(error) => error,
        }
    }

    // ── token interface ─────────────────────────────────────────────────────────

    pub fn check_token(&mut self, kinds: &[K]) -> Parsed<bool> {
        let _frame = frame("check_token")?;
        while self.need_more_tokens()? {
            self.fetch_more_tokens()?;
        }
        Ok(match self.tokens.front() {
            Some(front) => kinds.is_empty() || kinds.contains(&front.kind),
            None => false,
        })
    }

    pub fn peek_token(&mut self) -> Parsed<Token> {
        let _frame = frame("peek_token")?;
        while self.need_more_tokens()? {
            self.fetch_more_tokens()?;
        }
        Ok(self.tokens.front().cloned().expect("a token is available"))
    }

    pub fn get_token(&mut self) -> Parsed<Token> {
        let _frame = frame("get_token")?;
        while self.need_more_tokens()? {
            self.fetch_more_tokens()?;
        }
        self.tokens_taken += 1;
        Ok(self.tokens.pop_front().expect("a token is available"))
    }

    fn need_more_tokens(&mut self) -> Parsed<bool> {
        let _frame = frame("need_more_tokens")?;
        if self.done {
            return Ok(false);
        }
        if self.tokens.is_empty() {
            return Ok(true);
        }
        self.stale_possible_simple_keys()?;
        Ok(self.next_possible_simple_key()? == Some(self.tokens_taken))
    }

    fn fetch_more_tokens(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_more_tokens")?;
        self.scan_to_next_token()?;
        self.stale_possible_simple_keys()?;
        self.unwind_indent(self.reader.column as i64)?;
        let ch = self.peek(0)?;
        if ch == '\0' {
            return self.fetch_stream_end();
        }
        if ch == '%' && self.check_directive()? {
            return self.fetch_directive();
        }
        if ch == '-' && self.check_document_start()? {
            let _wrapper = frame("fetch_document_start")?;
            return self.fetch_document_indicator(K::DocumentStart);
        }
        if ch == '.' && self.check_document_end()? {
            let _wrapper = frame("fetch_document_end")?;
            return self.fetch_document_indicator(K::DocumentEnd);
        }
        match ch {
            '[' => {
                let _wrapper = frame("fetch_flow_sequence_start")?;
                return self.fetch_flow_collection_start(K::FlowSequenceStart);
            }
            '{' => {
                let _wrapper = frame("fetch_flow_mapping_start")?;
                return self.fetch_flow_collection_start(K::FlowMappingStart);
            }
            ']' => {
                let _wrapper = frame("fetch_flow_sequence_end")?;
                return self.fetch_flow_collection_end(K::FlowSequenceEnd);
            }
            '}' => {
                let _wrapper = frame("fetch_flow_mapping_end")?;
                return self.fetch_flow_collection_end(K::FlowMappingEnd);
            }
            ',' => return self.fetch_flow_entry(),
            _ => {}
        }
        if ch == '-' && self.check_block_entry()? {
            return self.fetch_block_entry();
        }
        if ch == '?' && self.check_key()? {
            return self.fetch_key();
        }
        if ch == ':' && self.check_value()? {
            return self.fetch_value();
        }
        match ch {
            '*' => return self.fetch_alias_or_anchor(K::Alias),
            '&' => return self.fetch_alias_or_anchor(K::Anchor),
            '!' => return self.fetch_tag(),
            '|' if self.flow_level == 0 => {
                let _wrapper = frame("fetch_literal")?;
                return self.fetch_block_scalar('|');
            }
            '>' if self.flow_level == 0 => {
                let _wrapper = frame("fetch_folded")?;
                return self.fetch_block_scalar('>');
            }
            '\'' => {
                let _wrapper = frame("fetch_single")?;
                return self.fetch_flow_scalar('\'');
            }
            '"' => {
                let _wrapper = frame("fetch_double")?;
                return self.fetch_flow_scalar('"');
            }
            _ => {}
        }
        if self.check_plain()? {
            return self.fetch_plain();
        }
        Err(self.error(
            Some("while scanning for the next token"),
            None,
            format!(
                "found character {} that cannot start any token",
                repr_char(ch)
            ),
        ))
    }

    // ── simple keys ─────────────────────────────────────────────────────────────

    fn next_possible_simple_key(&self) -> Parsed<Option<usize>> {
        let _frame = frame("next_possible_simple_key")?;
        Ok(self
            .possible_simple_keys
            .iter()
            .map(|(_, key)| key.token_number)
            .min())
    }

    fn stale_possible_simple_keys(&mut self) -> Parsed<()> {
        let _frame = frame("stale_possible_simple_keys")?;
        let mut index = 0;
        while index < self.possible_simple_keys.len() {
            let stale = {
                let key = &self.possible_simple_keys[index].1;
                key.line != self.reader.line || self.reader.index - key.index > 1024
            };
            if stale {
                let key = &self.possible_simple_keys[index].1;
                if key.required {
                    return Err(marked(
                        "ScannerError",
                        Some("while scanning a simple key"),
                        Some(&key.mark),
                        "could not find expected ':'",
                        Some(&self.mark()?),
                    ));
                }
                self.possible_simple_keys.remove(index);
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    fn save_possible_simple_key(&mut self) -> Parsed<()> {
        let _frame = frame("save_possible_simple_key")?;
        let required = self.flow_level == 0 && self.indent == self.reader.column as i64;
        if self.allow_simple_key {
            self.remove_possible_simple_key()?;
            let token_number = self.tokens_taken + self.tokens.len();
            let mark = self.mark()?;
            nested(&["__init__"])?;
            let key = SimpleKey {
                token_number,
                required,
                index: self.reader.index,
                line: self.reader.line,
                column: self.reader.column,
                mark,
            };
            self.possible_simple_keys.push((self.flow_level, key));
        }
        Ok(())
    }

    fn remove_possible_simple_key(&mut self) -> Parsed<()> {
        let _frame = frame("remove_possible_simple_key")?;
        if let Some(position) = self
            .possible_simple_keys
            .iter()
            .position(|(level, _)| *level == self.flow_level)
        {
            let key = &self.possible_simple_keys[position].1;
            if key.required {
                return Err(marked(
                    "ScannerError",
                    Some("while scanning a simple key"),
                    Some(&key.mark),
                    "could not find expected ':'",
                    Some(&self.mark()?),
                ));
            }
            self.possible_simple_keys.remove(position);
        }
        Ok(())
    }

    fn unwind_indent(&mut self, column: i64) -> Parsed<()> {
        let _frame = frame("unwind_indent")?;
        if self.flow_level != 0 {
            return Ok(());
        }
        while self.indent > column {
            let mark = self.mark()?;
            self.indent = self.indents.pop().expect("indent stack");
            let end = token(K::BlockEnd, mark.clone(), mark)?;
            self.tokens.push_back(end);
        }
        Ok(())
    }

    fn add_indent(&mut self, column: i64) -> Parsed<bool> {
        let _frame = frame("add_indent")?;
        if self.indent < column {
            self.indents.push(self.indent);
            self.indent = column;
            return Ok(true);
        }
        Ok(false)
    }

    // ── fetchers ────────────────────────────────────────────────────────────────

    fn fetch_stream_end(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_stream_end")?;
        self.unwind_indent(-1)?;
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        self.possible_simple_keys.clear();
        let mark = self.mark()?;
        self.tokens
            .push_back(token(K::StreamEnd, mark.clone(), mark)?);
        self.done = true;
        Ok(())
    }

    fn fetch_directive(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_directive")?;
        self.unwind_indent(-1)?;
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        let scanned = self.scan_directive()?;
        self.tokens.push_back(scanned);
        Ok(())
    }

    fn fetch_document_indicator(&mut self, kind: K) -> Parsed<()> {
        let _frame = frame("fetch_document_indicator")?;
        self.unwind_indent(-1)?;
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        let start = self.mark()?;
        self.forward(3)?;
        let end = self.mark()?;
        self.tokens.push_back(token(kind, start, end)?);
        Ok(())
    }

    fn fetch_flow_collection_start(&mut self, kind: K) -> Parsed<()> {
        let _frame = frame("fetch_flow_collection_start")?;
        self.save_possible_simple_key()?;
        self.flow_level += 1;
        self.allow_simple_key = true;
        let start = self.mark()?;
        self.forward(1)?;
        let end = self.mark()?;
        self.tokens.push_back(token(kind, start, end)?);
        Ok(())
    }

    fn fetch_flow_collection_end(&mut self, kind: K) -> Parsed<()> {
        let _frame = frame("fetch_flow_collection_end")?;
        self.remove_possible_simple_key()?;
        self.flow_level -= 1;
        self.allow_simple_key = false;
        let start = self.mark()?;
        self.forward(1)?;
        let end = self.mark()?;
        self.tokens.push_back(token(kind, start, end)?);
        Ok(())
    }

    fn fetch_flow_entry(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_flow_entry")?;
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        let start = self.mark()?;
        self.forward(1)?;
        let end = self.mark()?;
        self.tokens.push_back(token(K::FlowEntry, start, end)?);
        Ok(())
    }

    fn fetch_block_entry(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_block_entry")?;
        if self.flow_level == 0 {
            if !self.allow_simple_key {
                return Err(self.error(None, None, "sequence entries are not allowed here"));
            }
            if self.add_indent(self.reader.column as i64)? {
                let mark = self.mark()?;
                self.tokens
                    .push_back(token(K::BlockSequenceStart, mark.clone(), mark)?);
            }
        }
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        let start = self.mark()?;
        self.forward(1)?;
        let end = self.mark()?;
        self.tokens.push_back(token(K::BlockEntry, start, end)?);
        Ok(())
    }

    fn fetch_key(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_key")?;
        if self.flow_level == 0 {
            if !self.allow_simple_key {
                return Err(self.error(None, None, "mapping keys are not allowed here"));
            }
            if self.add_indent(self.reader.column as i64)? {
                let mark = self.mark()?;
                self.tokens
                    .push_back(token(K::BlockMappingStart, mark.clone(), mark)?);
            }
        }
        self.allow_simple_key = self.flow_level == 0;
        self.remove_possible_simple_key()?;
        let start = self.mark()?;
        self.forward(1)?;
        let end = self.mark()?;
        self.tokens.push_back(token(K::Key, start, end)?);
        Ok(())
    }

    fn fetch_value(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_value")?;
        if let Some(position) = self
            .possible_simple_keys
            .iter()
            .position(|(level, _)| *level == self.flow_level)
        {
            let (_, key) = self.possible_simple_keys.remove(position);
            let at = key.token_number - self.tokens_taken;
            self.tokens
                .insert(at, token(K::Key, key.mark.clone(), key.mark.clone())?);
            if self.flow_level == 0 && self.add_indent(key.column as i64)? {
                self.tokens.insert(
                    at,
                    token(K::BlockMappingStart, key.mark.clone(), key.mark.clone())?,
                );
            }
            self.allow_simple_key = false;
        } else {
            if self.flow_level == 0 && !self.allow_simple_key {
                return Err(self.error(None, None, "mapping values are not allowed here"));
            }
            if self.flow_level == 0 && self.add_indent(self.reader.column as i64)? {
                let mark = self.mark()?;
                self.tokens
                    .push_back(token(K::BlockMappingStart, mark.clone(), mark)?);
            }
            self.allow_simple_key = self.flow_level == 0;
            self.remove_possible_simple_key()?;
        }
        let start = self.mark()?;
        self.forward(1)?;
        let end = self.mark()?;
        self.tokens.push_back(token(K::Value, start, end)?);
        Ok(())
    }

    fn fetch_alias_or_anchor(&mut self, kind: K) -> Parsed<()> {
        let _frame = frame(if kind == K::Alias {
            "fetch_alias"
        } else {
            "fetch_anchor"
        })?;
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let scanned = self.scan_anchor(kind)?;
        self.tokens.push_back(scanned);
        Ok(())
    }

    fn fetch_tag(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_tag")?;
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let scanned = self.scan_tag()?;
        self.tokens.push_back(scanned);
        Ok(())
    }

    fn fetch_block_scalar(&mut self, style: char) -> Parsed<()> {
        let _frame = frame("fetch_block_scalar")?;
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        let scanned = self.scan_block_scalar(style)?;
        self.tokens.push_back(scanned);
        Ok(())
    }

    fn fetch_flow_scalar(&mut self, style: char) -> Parsed<()> {
        let _frame = frame("fetch_flow_scalar")?;
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let scanned = self.scan_flow_scalar(style)?;
        self.tokens.push_back(scanned);
        Ok(())
    }

    fn fetch_plain(&mut self) -> Parsed<()> {
        let _frame = frame("fetch_plain")?;
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let scanned = self.scan_plain()?;
        self.tokens.push_back(scanned);
        Ok(())
    }

    // ── checkers ────────────────────────────────────────────────────────────────

    fn check_directive(&self) -> Parsed<bool> {
        let _frame = frame("check_directive")?;
        Ok(self.reader.column == 0)
    }

    fn check_document_start(&mut self) -> Parsed<bool> {
        let _frame = frame("check_document_start")?;
        Ok(self.reader.column == 0
            && self.prefix(3)? == "---"
            && is_in(self.peek(3)?, NUL_SPACE_BREAKS))
    }

    fn check_document_end(&mut self) -> Parsed<bool> {
        let _frame = frame("check_document_end")?;
        Ok(self.reader.column == 0
            && self.prefix(3)? == "..."
            && is_in(self.peek(3)?, NUL_SPACE_BREAKS))
    }

    fn check_block_entry(&mut self) -> Parsed<bool> {
        let _frame = frame("check_block_entry")?;
        Ok(is_in(self.peek(1)?, NUL_SPACE_BREAKS))
    }

    fn check_key(&mut self) -> Parsed<bool> {
        let _frame = frame("check_key")?;
        Ok(self.flow_level != 0 || is_in(self.peek(1)?, NUL_SPACE_BREAKS))
    }

    fn check_value(&mut self) -> Parsed<bool> {
        let _frame = frame("check_value")?;
        Ok(self.flow_level != 0 || is_in(self.peek(1)?, NUL_SPACE_BREAKS))
    }

    fn check_plain(&mut self) -> Parsed<bool> {
        let _frame = frame("check_plain")?;
        let ch = self.peek(0)?;
        if !is_in(ch, "\0 \t\r\n\u{85}\u{2028}\u{2029}-?:,[]{}#&*!|>'\"%@`") {
            return Ok(true);
        }
        Ok(!is_in(self.peek(1)?, NUL_SPACE_BREAKS)
            && (ch == '-' || (self.flow_level == 0 && is_in(ch, "?:"))))
    }

    // ── scanners ────────────────────────────────────────────────────────────────

    fn scan_to_next_token(&mut self) -> Parsed<()> {
        let _frame = frame("scan_to_next_token")?;
        if self.reader.index == 0 && self.peek(0)? == '\u{FEFF}' {
            self.forward(1)?;
        }
        let mut found = false;
        while !found {
            while self.peek(0)? == ' ' {
                self.forward(1)?;
            }
            if self.peek(0)? == '#' {
                while !is_in(self.peek(0)?, NUL_BREAKS) {
                    self.forward(1)?;
                }
            }
            if !self.scan_line_break()?.is_empty() {
                if self.flow_level == 0 {
                    self.allow_simple_key = true;
                }
            } else {
                found = true;
            }
        }
        Ok(())
    }

    fn scan_directive(&mut self) -> Parsed<Token> {
        let _frame = frame("scan_directive")?;
        let start = self.mark()?;
        self.forward(1)?;
        let name = self.scan_directive_name(&start)?;
        let (value, end) = if name == "YAML" {
            let value = self.scan_yaml_directive_value(&start)?;
            (value, self.mark()?)
        } else if name == "TAG" {
            let value = self.scan_tag_directive_value(&start)?;
            (value, self.mark()?)
        } else {
            let end = self.mark()?;
            while !is_in(self.peek(0)?, NUL_BREAKS) {
                self.forward(1)?;
            }
            (DirectiveValue::None, end)
        };
        self.scan_directive_ignored_line(&start)?;
        let mut t = token(K::Directive, start, end)?;
        t.text = PyStr::new(&name);
        t.directive = value;
        Ok(t)
    }

    fn scan_directive_name(&mut self, start: &Mark) -> Parsed<String> {
        let _frame = frame("scan_directive_name")?;
        let mut length = 0;
        let mut ch = self.peek(length)?;
        while alnum(ch) {
            length += 1;
            ch = self.peek(length)?;
        }
        if length == 0 {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!(
                    "expected alphabetic or numeric character, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        let value = self.prefix(length)?;
        self.forward(length)?;
        let ch = self.peek(0)?;
        if !is_in(ch, "\0 \r\n\u{85}\u{2028}\u{2029}") {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!(
                    "expected alphabetic or numeric character, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        Ok(value)
    }

    fn scan_yaml_directive_value(&mut self, start: &Mark) -> Parsed<DirectiveValue> {
        let _frame = frame("scan_yaml_directive_value")?;
        while self.peek(0)? == ' ' {
            self.forward(1)?;
        }
        let major = self.scan_yaml_directive_number(start)?;
        let ch = self.peek(0)?;
        if ch != '.' {
            let again = self.peek(0)?;
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!("expected a digit or '.', but found {}", repr_char(again)),
            ));
        }
        self.forward(1)?;
        let minor = self.scan_yaml_directive_number(start)?;
        let ch = self.peek(0)?;
        if !is_in(ch, "\0 \r\n\u{85}\u{2028}\u{2029}") {
            let again = self.peek(0)?;
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!("expected a digit or ' ', but found {}", repr_char(again)),
            ));
        }
        Ok(DirectiveValue::Yaml(major, minor))
    }

    fn scan_yaml_directive_number(&mut self, start: &Mark) -> Parsed<i64> {
        let _frame = frame("scan_yaml_directive_number")?;
        let ch = self.peek(0)?;
        if !ch.is_ascii_digit() {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!("expected a digit, but found {}", repr_char(ch)),
            ));
        }
        let mut length = 0;
        while self.peek(length)?.is_ascii_digit() {
            length += 1;
        }
        let digits = self.prefix(length)?;
        self.forward(length)?;
        Ok(digits.parse::<i64>().unwrap_or(i64::MAX))
    }

    fn scan_tag_directive_value(&mut self, start: &Mark) -> Parsed<DirectiveValue> {
        let _frame = frame("scan_tag_directive_value")?;
        while self.peek(0)? == ' ' {
            self.forward(1)?;
        }
        let handle = self.scan_tag_directive_handle(start)?;
        while self.peek(0)? == ' ' {
            self.forward(1)?;
        }
        let prefix = self.scan_tag_directive_prefix(start)?;
        Ok(DirectiveValue::Tag(handle, prefix))
    }

    fn scan_tag_directive_handle(&mut self, start: &Mark) -> Parsed<String> {
        let _frame = frame("scan_tag_directive_handle")?;
        let value = self.scan_tag_handle("directive", start)?;
        let ch = self.peek(0)?;
        if ch != ' ' {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!("expected ' ', but found {}", repr_char(ch)),
            ));
        }
        Ok(value)
    }

    fn scan_tag_directive_prefix(&mut self, start: &Mark) -> Parsed<String> {
        let _frame = frame("scan_tag_directive_prefix")?;
        let value = self.scan_tag_uri("directive", start)?;
        let ch = self.peek(0)?;
        if !is_in(ch, "\0 \r\n\u{85}\u{2028}\u{2029}") {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!("expected ' ', but found {}", repr_char(ch)),
            ));
        }
        Ok(value)
    }

    fn scan_directive_ignored_line(&mut self, start: &Mark) -> Parsed<()> {
        let _frame = frame("scan_directive_ignored_line")?;
        while self.peek(0)? == ' ' {
            self.forward(1)?;
        }
        if self.peek(0)? == '#' {
            while !is_in(self.peek(0)?, NUL_BREAKS) {
                self.forward(1)?;
            }
        }
        let ch = self.peek(0)?;
        if !is_in(ch, NUL_BREAKS) {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!(
                    "expected a comment or a line break, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        self.scan_line_break()?;
        Ok(())
    }

    fn scan_anchor(&mut self, kind: K) -> Parsed<Token> {
        let _frame = frame("scan_anchor")?;
        let start = self.mark()?;
        let name = if self.peek(0)? == '*' {
            "alias"
        } else {
            "anchor"
        };
        self.forward(1)?;
        let mut length = 0;
        let mut ch = self.peek(length)?;
        while alnum(ch) {
            length += 1;
            ch = self.peek(length)?;
        }
        if length == 0 {
            return Err(self.error(
                Some(&format!("while scanning an {name}")),
                Some(&start),
                format!(
                    "expected alphabetic or numeric character, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        let value = self.prefix(length)?;
        self.forward(length)?;
        let ch = self.peek(0)?;
        if !is_in(ch, "\0 \t\r\n\u{85}\u{2028}\u{2029}?:,]}%@`") {
            return Err(self.error(
                Some(&format!("while scanning an {name}")),
                Some(&start),
                format!(
                    "expected alphabetic or numeric character, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        let end = self.mark()?;
        let mut t = token(kind, start, end)?;
        t.text = PyStr::new(&value);
        Ok(t)
    }

    fn scan_tag(&mut self) -> Parsed<Token> {
        let _frame = frame("scan_tag")?;
        let start = self.mark()?;
        let mut ch = self.peek(1)?;
        let handle: Option<String>;
        let suffix: String;
        if ch == '<' {
            handle = None;
            self.forward(2)?;
            suffix = self.scan_tag_uri("tag", &start)?;
            let ch = self.peek(0)?;
            if ch != '>' {
                let again = self.peek(0)?;
                return Err(self.error(
                    Some("while parsing a tag"),
                    Some(&start),
                    format!("expected '>', but found {}", repr_char(again)),
                ));
            }
            self.forward(1)?;
        } else if is_in(ch, NUL_SPACE_BREAKS) {
            handle = None;
            suffix = "!".to_string();
            self.forward(1)?;
        } else {
            let mut length = 1;
            let mut use_handle = false;
            while !is_in(ch, "\0 \r\n\u{85}\u{2028}\u{2029}") {
                if ch == '!' {
                    use_handle = true;
                    break;
                }
                length += 1;
                ch = self.peek(length)?;
            }
            if use_handle {
                handle = Some(self.scan_tag_handle("tag", &start)?);
            } else {
                handle = Some("!".to_string());
                self.forward(1)?;
            }
            suffix = self.scan_tag_uri("tag", &start)?;
        }
        let ch = self.peek(0)?;
        if !is_in(ch, "\0 \r\n\u{85}\u{2028}\u{2029}") {
            return Err(self.error(
                Some("while scanning a tag"),
                Some(&start),
                format!("expected ' ', but found {}", repr_char(ch)),
            ));
        }
        let end = self.mark()?;
        let mut t = token(K::Tag, start, end)?;
        t.tag = Some((handle, suffix));
        Ok(t)
    }

    fn scan_block_scalar(&mut self, style: char) -> Parsed<Token> {
        let _frame = frame("scan_block_scalar")?;
        let folded = style == '>';
        let mut chunks: Vec<u32> = Vec::new();
        let start = self.mark()?;
        self.forward(1)?;
        let (chomping, increment) = self.scan_block_scalar_indicators(&start)?;
        self.scan_block_scalar_ignored_line(&start)?;
        let min_indent = (self.indent + 1).max(1);
        let (mut breaks, mut end);
        let indent: i64;
        match increment {
            None => {
                let (b, max_indent, e) = self.scan_block_scalar_indentation()?;
                breaks = b;
                end = e;
                indent = min_indent.max(max_indent);
            }
            Some(increment) => {
                indent = min_indent + increment - 1;
                let (b, e) = self.scan_block_scalar_breaks(indent)?;
                breaks = b;
                end = e;
            }
        }
        let mut line_break = String::new();
        while self.reader.column as i64 == indent && self.peek(0)? != '\0' {
            for b in &breaks {
                chunks.extend(b.chars().map(u32::from));
            }
            let leading_non_space = !is_in(self.peek(0)?, " \t");
            let mut length = 0;
            while !is_in(self.peek(length)?, NUL_BREAKS) {
                length += 1;
            }
            chunks.extend(self.prefix(length)?.chars().map(u32::from));
            self.forward(length)?;
            line_break = self.scan_line_break()?;
            let (b, e) = self.scan_block_scalar_breaks(indent)?;
            breaks = b;
            end = e;
            if self.reader.column as i64 == indent && self.peek(0)? != '\0' {
                if folded && line_break == "\n" && leading_non_space && !is_in(self.peek(0)?, " \t")
                {
                    if breaks.is_empty() {
                        chunks.push(u32::from(' '));
                    }
                } else {
                    chunks.extend(line_break.chars().map(u32::from));
                }
            } else {
                break;
            }
        }
        if chomping != Some(false) {
            chunks.extend(line_break.chars().map(u32::from));
        }
        if chomping == Some(true) {
            for b in &breaks {
                chunks.extend(b.chars().map(u32::from));
            }
        }
        let mut t = token(K::Scalar, start, end)?;
        t.text = PyStr::from_code_points(chunks);
        t.style = Some(style);
        Ok(t)
    }

    fn scan_block_scalar_indicators(
        &mut self,
        start: &Mark,
    ) -> Parsed<(Option<bool>, Option<i64>)> {
        let _frame = frame("scan_block_scalar_indicators")?;
        let mut chomping: Option<bool> = None;
        let mut increment: Option<i64> = None;
        let zero_error = |scanner: &Scanner<'_>| {
            scanner.error(
                Some("while scanning a block scalar"),
                Some(start),
                "expected indentation indicator in the range 1-9, but found 0",
            )
        };
        let mut ch = self.peek(0)?;
        if ch == '+' || ch == '-' {
            chomping = Some(ch == '+');
            self.forward(1)?;
            ch = self.peek(0)?;
            if ch.is_ascii_digit() {
                let value = i64::from(ch as u8 - b'0');
                if value == 0 {
                    return Err(zero_error(self));
                }
                increment = Some(value);
                self.forward(1)?;
            }
        } else if ch.is_ascii_digit() {
            let value = i64::from(ch as u8 - b'0');
            if value == 0 {
                return Err(zero_error(self));
            }
            increment = Some(value);
            self.forward(1)?;
            ch = self.peek(0)?;
            if ch == '+' || ch == '-' {
                chomping = Some(ch == '+');
                self.forward(1)?;
            }
        }
        let ch = self.peek(0)?;
        if !is_in(ch, "\0 \r\n\u{85}\u{2028}\u{2029}") {
            return Err(self.error(
                Some("while scanning a block scalar"),
                Some(start),
                format!(
                    "expected chomping or indentation indicators, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        Ok((chomping, increment))
    }

    fn scan_block_scalar_ignored_line(&mut self, start: &Mark) -> Parsed<()> {
        let _frame = frame("scan_block_scalar_ignored_line")?;
        while self.peek(0)? == ' ' {
            self.forward(1)?;
        }
        if self.peek(0)? == '#' {
            while !is_in(self.peek(0)?, NUL_BREAKS) {
                self.forward(1)?;
            }
        }
        let ch = self.peek(0)?;
        if !is_in(ch, NUL_BREAKS) {
            return Err(self.error(
                Some("while scanning a block scalar"),
                Some(start),
                format!(
                    "expected a comment or a line break, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        self.scan_line_break()?;
        Ok(())
    }

    fn scan_block_scalar_indentation(&mut self) -> Parsed<(Vec<String>, i64, Mark)> {
        let _frame = frame("scan_block_scalar_indentation")?;
        let mut chunks = Vec::new();
        let mut max_indent = 0i64;
        let mut end = self.mark()?;
        while is_in(self.peek(0)?, " \r\n\u{85}\u{2028}\u{2029}") {
            if self.peek(0)? != ' ' {
                chunks.push(self.scan_line_break()?);
                end = self.mark()?;
            } else {
                self.forward(1)?;
                if self.reader.column as i64 > max_indent {
                    max_indent = self.reader.column as i64;
                }
            }
        }
        Ok((chunks, max_indent, end))
    }

    fn scan_block_scalar_breaks(&mut self, indent: i64) -> Parsed<(Vec<String>, Mark)> {
        let _frame = frame("scan_block_scalar_breaks")?;
        let mut chunks = Vec::new();
        let mut end = self.mark()?;
        while (self.reader.column as i64) < indent && self.peek(0)? == ' ' {
            self.forward(1)?;
        }
        while is_in(self.peek(0)?, BREAKS) {
            chunks.push(self.scan_line_break()?);
            end = self.mark()?;
            while (self.reader.column as i64) < indent && self.peek(0)? == ' ' {
                self.forward(1)?;
            }
        }
        Ok((chunks, end))
    }

    fn scan_flow_scalar(&mut self, style: char) -> Parsed<Token> {
        let _frame = frame("scan_flow_scalar")?;
        let double = style == '"';
        let mut chunks: Vec<u32> = Vec::new();
        let start = self.mark()?;
        let quote = self.peek(0)?;
        self.forward(1)?;
        chunks.extend(self.scan_flow_scalar_non_spaces(double, &start)?);
        while self.peek(0)? != quote {
            chunks.extend(self.scan_flow_scalar_spaces(double, &start)?);
            chunks.extend(self.scan_flow_scalar_non_spaces(double, &start)?);
        }
        self.forward(1)?;
        let end = self.mark()?;
        let mut t = token(K::Scalar, start, end)?;
        t.text = PyStr::from_code_points(chunks);
        t.style = Some(style);
        Ok(t)
    }

    fn scan_flow_scalar_non_spaces(&mut self, double: bool, start: &Mark) -> Parsed<Vec<u32>> {
        let _frame = frame("scan_flow_scalar_non_spaces")?;
        let mut chunks: Vec<u32> = Vec::new();
        loop {
            let mut length = 0;
            while !is_in(self.peek(length)?, "'\"\\\0 \t\r\n\u{85}\u{2028}\u{2029}") {
                length += 1;
            }
            if length > 0 {
                chunks.extend(self.prefix(length)?.chars().map(u32::from));
                self.forward(length)?;
            }
            let mut ch = self.peek(0)?;
            if !double && ch == '\'' && self.peek(1)? == '\'' {
                chunks.push(u32::from('\''));
                self.forward(2)?;
            } else if (double && ch == '\'') || (!double && is_in(ch, "\"\\")) {
                chunks.push(u32::from(ch));
                self.forward(1)?;
            } else if double && ch == '\\' {
                self.forward(1)?;
                ch = self.peek(0)?;
                let replacement = match ch {
                    '0' => Some('\0'),
                    'a' => Some('\u{7}'),
                    'b' => Some('\u{8}'),
                    't' | '\t' => Some('\t'),
                    'n' => Some('\n'),
                    'v' => Some('\u{b}'),
                    'f' => Some('\u{c}'),
                    'r' => Some('\r'),
                    'e' => Some('\u{1b}'),
                    ' ' => Some(' '),
                    '"' => Some('"'),
                    '\\' => Some('\\'),
                    '/' => Some('/'),
                    'N' => Some('\u{85}'),
                    '_' => Some('\u{a0}'),
                    'L' => Some('\u{2028}'),
                    'P' => Some('\u{2029}'),
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    chunks.push(u32::from(replacement));
                    self.forward(1)?;
                } else if let Some(count) = match ch {
                    'x' => Some(2),
                    'u' => Some(4),
                    'U' => Some(8),
                    _ => None,
                } {
                    self.forward(1)?;
                    for k in 0..count {
                        let digit = self.peek(k)?;
                        if !digit.is_ascii_hexdigit() {
                            let again = self.peek(k)?;
                            return Err(self.error(
                                Some("while scanning a double-quoted scalar"),
                                Some(start),
                                format!(
                                    "expected escape sequence of {count} hexadecimal numbers, but found {}",
                                    repr_char(again)
                                ),
                            ));
                        }
                    }
                    let digits = self.prefix(count)?;
                    let code = u32::from_str_radix(&digits, 16).expect("hex digits");
                    if code > 0x10FFFF {
                        return Err(super::plain(
                            "ValueError",
                            "chr() arg not in range(0x110000)",
                        ));
                    }
                    chunks.push(code);
                    self.forward(count)?;
                } else if is_in(ch, BREAKS) {
                    self.scan_line_break()?;
                    chunks.extend(
                        self.scan_flow_scalar_breaks(double, start)?
                            .iter()
                            .flat_map(|b| b.chars().map(u32::from)),
                    );
                } else {
                    return Err(self.error(
                        Some("while scanning a double-quoted scalar"),
                        Some(start),
                        format!("found unknown escape character {}", repr_char(ch)),
                    ));
                }
            } else {
                return Ok(chunks);
            }
        }
    }

    fn scan_flow_scalar_spaces(&mut self, double: bool, start: &Mark) -> Parsed<Vec<u32>> {
        let _frame = frame("scan_flow_scalar_spaces")?;
        let mut chunks: Vec<u32> = Vec::new();
        let mut length = 0;
        while is_in(self.peek(length)?, " \t") {
            length += 1;
        }
        let whitespaces = self.prefix(length)?;
        self.forward(length)?;
        let ch = self.peek(0)?;
        if ch == '\0' {
            return Err(self.error(
                Some("while scanning a quoted scalar"),
                Some(start),
                "found unexpected end of stream",
            ));
        } else if is_in(ch, BREAKS) {
            let line_break = self.scan_line_break()?;
            let breaks = self.scan_flow_scalar_breaks(double, start)?;
            if line_break != "\n" {
                chunks.extend(line_break.chars().map(u32::from));
            } else if breaks.is_empty() {
                chunks.push(u32::from(' '));
            }
            for b in &breaks {
                chunks.extend(b.chars().map(u32::from));
            }
        } else {
            chunks.extend(whitespaces.chars().map(u32::from));
        }
        Ok(chunks)
    }

    fn scan_flow_scalar_breaks(&mut self, _double: bool, start: &Mark) -> Parsed<Vec<String>> {
        let _frame = frame("scan_flow_scalar_breaks")?;
        let mut chunks = Vec::new();
        loop {
            let prefix = self.prefix(3)?;
            if (prefix == "---" || prefix == "...") && is_in(self.peek(3)?, NUL_SPACE_BREAKS) {
                return Err(self.error(
                    Some("while scanning a quoted scalar"),
                    Some(start),
                    "found unexpected document separator",
                ));
            }
            while is_in(self.peek(0)?, " \t") {
                self.forward(1)?;
            }
            if is_in(self.peek(0)?, BREAKS) {
                chunks.push(self.scan_line_break()?);
            } else {
                return Ok(chunks);
            }
        }
    }

    fn scan_plain(&mut self) -> Parsed<Token> {
        let _frame = frame("scan_plain")?;
        let mut chunks: Vec<u32> = Vec::new();
        let start = self.mark()?;
        let mut end = start.clone();
        let indent = self.indent + 1;
        let mut spaces: Vec<String> = Vec::new();
        loop {
            let mut length = 0;
            if self.peek(0)? == '#' {
                break;
            }
            loop {
                let ch = self.peek(length)?;
                if is_in(ch, NUL_SPACE_BREAKS)
                    || (ch == ':' && {
                        let next = self.peek(length + 1)?;
                        is_in(next, NUL_SPACE_BREAKS)
                            || (self.flow_level != 0 && is_in(next, ",[]{}"))
                    })
                    || (self.flow_level != 0 && is_in(ch, ",?[]{}"))
                {
                    break;
                }
                length += 1;
            }
            if length == 0 {
                break;
            }
            self.allow_simple_key = false;
            for s in &spaces {
                chunks.extend(s.chars().map(u32::from));
            }
            chunks.extend(self.prefix(length)?.chars().map(u32::from));
            self.forward(length)?;
            end = self.mark()?;
            let more = self.scan_plain_spaces(indent, &start)?;
            let Some(more) = more else { break };
            spaces = more;
            if spaces.is_empty()
                || self.peek(0)? == '#'
                || (self.flow_level == 0 && (self.reader.column as i64) < indent)
            {
                break;
            }
        }
        let mut t = token(K::Scalar, start, end)?;
        t.text = PyStr::from_code_points(chunks);
        t.plain = true;
        Ok(t)
    }

    /// `scan_plain_spaces`: `None` is Python's bare `return` (a document separator follows).
    fn scan_plain_spaces(&mut self, _indent: i64, _start: &Mark) -> Parsed<Option<Vec<String>>> {
        let _frame = frame("scan_plain_spaces")?;
        let mut chunks: Vec<String> = Vec::new();
        let mut length = 0;
        while self.peek(length)? == ' ' {
            length += 1;
        }
        let whitespaces = self.prefix(length)?;
        self.forward(length)?;
        let ch = self.peek(0)?;
        if is_in(ch, BREAKS) {
            let line_break = self.scan_line_break()?;
            self.allow_simple_key = true;
            let prefix = self.prefix(3)?;
            if (prefix == "---" || prefix == "...") && is_in(self.peek(3)?, NUL_SPACE_BREAKS) {
                return Ok(None);
            }
            let mut breaks: Vec<String> = Vec::new();
            while is_in(self.peek(0)?, " \r\n\u{85}\u{2028}\u{2029}") {
                if self.peek(0)? == ' ' {
                    self.forward(1)?;
                } else {
                    breaks.push(self.scan_line_break()?);
                    let prefix = self.prefix(3)?;
                    if (prefix == "---" || prefix == "...")
                        && is_in(self.peek(3)?, NUL_SPACE_BREAKS)
                    {
                        return Ok(None);
                    }
                }
            }
            if line_break != "\n" {
                chunks.push(line_break);
            } else if breaks.is_empty() {
                chunks.push(" ".to_string());
            }
            chunks.extend(breaks);
        } else if !whitespaces.is_empty() {
            chunks.push(whitespaces);
        }
        Ok(Some(chunks))
    }

    fn scan_tag_handle(&mut self, name: &str, start: &Mark) -> Parsed<String> {
        let _frame = frame("scan_tag_handle")?;
        let ch = self.peek(0)?;
        if ch != '!' {
            return Err(self.error(
                Some(&format!("while scanning a {name}")),
                Some(start),
                format!("expected '!', but found {}", repr_char(ch)),
            ));
        }
        let mut length = 1;
        let mut ch = self.peek(length)?;
        if ch != ' ' {
            while alnum(ch) {
                length += 1;
                ch = self.peek(length)?;
            }
            if ch != '!' {
                self.forward(length)?;
                return Err(self.error(
                    Some(&format!("while scanning a {name}")),
                    Some(start),
                    format!("expected '!', but found {}", repr_char(ch)),
                ));
            }
            length += 1;
        }
        let value = self.prefix(length)?;
        self.forward(length)?;
        Ok(value)
    }

    fn scan_tag_uri(&mut self, name: &str, start: &Mark) -> Parsed<String> {
        let _frame = frame("scan_tag_uri")?;
        let mut chunks = String::new();
        let mut appended = false;
        let mut length = 0;
        let mut ch = self.peek(length)?;
        while ch.is_ascii_alphanumeric() || is_in(ch, "-;/?:@&=+$,_.!~*'()[]%") {
            if ch == '%' {
                chunks.push_str(&self.prefix(length)?);
                self.forward(length)?;
                length = 0;
                chunks.push_str(&self.scan_uri_escapes(name, start)?);
                appended = true;
            } else {
                length += 1;
            }
            ch = self.peek(length)?;
        }
        let mut any = appended;
        if length > 0 {
            chunks.push_str(&self.prefix(length)?);
            self.forward(length)?;
            any = true;
        }
        if !any {
            return Err(self.error(
                Some(&format!("while parsing a {name}")),
                Some(start),
                format!("expected URI, but found {}", repr_char(ch)),
            ));
        }
        Ok(chunks)
    }

    fn scan_uri_escapes(&mut self, name: &str, start: &Mark) -> Parsed<String> {
        let _frame = frame("scan_uri_escapes")?;
        let mut codes: Vec<u8> = Vec::new();
        let mark = self.mark()?;
        while self.peek(0)? == '%' {
            self.forward(1)?;
            for k in 0..2 {
                let digit = self.peek(k)?;
                if !digit.is_ascii_hexdigit() {
                    let again = self.peek(k)?;
                    return Err(self.error(
                        Some(&format!("while scanning a {name}")),
                        Some(start),
                        format!(
                            "expected URI escape sequence of 2 hexadecimal numbers, but found {}",
                            repr_char(again)
                        ),
                    ));
                }
            }
            codes.push(u8::from_str_radix(&self.prefix(2)?, 16).expect("hex"));
            self.forward(2)?;
        }
        match std::str::from_utf8(&codes) {
            Ok(text) => Ok(text.to_string()),
            Err(error) => Err(marked(
                "ScannerError",
                Some(&format!("while scanning a {name}")),
                Some(start),
                crate::pyjson::utf8_error_text(&codes, &error),
                Some(&mark),
            )),
        }
    }

    fn scan_line_break(&mut self) -> Parsed<String> {
        let _frame = frame("scan_line_break")?;
        let ch = self.peek(0)?;
        if is_in(ch, "\r\n\u{85}") {
            if self.prefix(2)? == "\r\n" {
                self.forward(2)?;
            } else {
                self.forward(1)?;
            }
            return Ok("\n".to_string());
        }
        if is_in(ch, "\u{2028}\u{2029}") {
            self.forward(1)?;
            return Ok(ch.to_string());
        }
        Ok(String::new())
    }
}
