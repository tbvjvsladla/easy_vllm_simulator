//! `tomllib/_parser.py`: `loads(s)` with the default `parse_float`, over the text as Unicode code points. Function names, the order of checks, the
//! error messages and the positions they report follow the original; positions are code-point offsets into the text after `\r\n` became `\n`.
//!
//! The original recurses once per nested array or inline table. This port recurses the same way and counts the interpreter frames the original
//! would have on its stack ([`RECURSION_FRAME_BUDGET`]); a document that needs more is a named unsupported domain, not a result.

use super::failure::{ExceptionClass, LoadsFailure, Raised, UnsupportedDomain};
use super::flags::{Flag, Flags, Key};
use super::nested::{NestError, NestedDict};
use super::repr::{key_repr, single_char_repr, string_repr, text_repr};
use super::scalars::{
    datetime_value, match_datetime, match_local_time, match_number, number_value, NumberFailure,
};
use super::value::{Table, Text, TomlValue};

/// How many of the interpreter's 1000 recursion-limit frames this port lets the parser use. The other half is the share reserved for the
/// caller's own stack and the leaf calls the count does not model.
pub const RECURSION_FRAME_BUDGET: usize = 500;

/// The frames the original holds when the first value is parsed: `loads`, `key_value_rule`.
const BASE_FRAMES: usize = 2;

type Parsed<T> = Result<(usize, T), LoadsFailure>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Illegal {
    /// `ILLEGAL_BASIC_STR_CHARS` (also the literal-string and comment sets): the ASCII controls and DEL except tab.
    InLine,
    /// `ILLEGAL_MULTILINE_BASIC_STR_CHARS`: the same, except that newline is allowed.
    InMultiline,
}

impl Illegal {
    fn contains(self, code: u32) -> bool {
        let control = code < 32 || code == 127;
        control && code != 9 && !(self == Illegal::InMultiline && code == 10)
    }
}

fn is_whitespace(code: u32) -> bool {
    code == 0x20 || code == 0x09
}

fn is_whitespace_or_newline(code: u32) -> bool {
    is_whitespace(code) || code == 0x0A
}

fn is_bare_key_char(code: u32) -> bool {
    char::from_u32(code).is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_key_initial(code: u32) -> bool {
    is_bare_key_char(code) || code == u32::from(b'"') || code == u32::from(b'\'')
}

fn is_unicode_scalar_value(code: u32) -> bool {
    code <= 55295 || (57344..=1114111).contains(&code)
}

fn code(character: char) -> u32 {
    u32::from(character)
}

/// The statement-level state of one document: its table and the flags of its keys.
struct Output {
    data: NestedDict,
    flags: Flags,
}

struct Parser<'a> {
    src: &'a [u32],
    /// The interpreter frames the original would currently hold.
    frames: usize,
}

/// `tomllib.loads(source)`.
pub fn loads(source: &[u32]) -> Result<Table, LoadsFailure> {
    let src = normalize_newlines(source);
    let mut parser = Parser {
        src: &src,
        frames: BASE_FRAMES,
    };
    let mut out = Output {
        data: NestedDict::new(),
        flags: Flags::new(),
    };
    let mut header: Key = Vec::new();
    let mut pos = 0;

    loop {
        pos = parser.skip_chars(pos, is_whitespace);
        let Some(character) = parser.char_at(pos) else {
            break;
        };
        if character == code('\n') {
            pos += 1;
            continue;
        }
        if is_key_initial(character) {
            pos = parser.key_value_rule(pos, &mut out, &header)?;
            pos = parser.skip_chars(pos, is_whitespace);
        } else if character == code('[') {
            let second = parser.char_at(pos + 1);
            out.flags.finalize_pending();
            if second == Some(code('[')) {
                (pos, header) = parser.create_list_rule(pos, &mut out)?;
            } else {
                (pos, header) = parser.create_dict_rule(pos, &mut out)?;
            }
            pos = parser.skip_chars(pos, is_whitespace);
        } else if character != code('#') {
            return Err(parser.error(pos, "Invalid statement"));
        }

        pos = parser.skip_comment(pos)?;

        let Some(character) = parser.char_at(pos) else {
            break;
        };
        if character != code('\n') {
            return Err(parser.error(pos, "Expected newline or end of document after a statement"));
        }
        pos += 1;
    }
    Ok(out.data.into_table())
}

/// `s.replace("\r\n", "\n")`: one left-to-right pass.
fn normalize_newlines(source: &[u32]) -> Vec<u32> {
    let mut normalized = Vec::with_capacity(source.len());
    let mut at = 0;
    while at < source.len() {
        if source[at] == code('\r') && source.get(at + 1) == Some(&code('\n')) {
            normalized.push(code('\n'));
            at += 2;
        } else {
            normalized.push(source[at]);
            at += 1;
        }
    }
    normalized
}

impl Parser<'_> {
    fn char_at(&self, pos: usize) -> Option<u32> {
        self.src.get(pos).copied()
    }

    /// `src.startswith(literal, pos)` for an ASCII literal.
    fn starts_with(&self, literal: &str, pos: usize) -> bool {
        let wanted = literal.chars().map(u32::from);
        pos <= self.src.len()
            && self.src[pos..].len() >= literal.len()
            && self.src[pos..]
                .iter()
                .copied()
                .zip(wanted)
                .all(|(a, b)| a == b)
    }

    /// `src[pos:pos + len]`, clamped to the text.
    fn slice(&self, pos: usize, len: usize) -> &[u32] {
        let start = pos.min(self.src.len());
        let end = pos.saturating_add(len).min(self.src.len());
        &self.src[start..end]
    }

    /// `suffixed_err`: the message with the coordinates of `pos`, as a `TOMLDecodeError` raised outside any `except` block.
    fn error(&self, pos: usize, message: &str) -> LoadsFailure {
        LoadsFailure::Raised(Raised::plain(
            ExceptionClass::TomlDecodeError,
            self.suffixed(pos, message),
        ))
    }

    /// The same, raised with `from None` inside an `except` block.
    fn error_from_none(&self, pos: usize, message: &str) -> LoadsFailure {
        LoadsFailure::Raised(Raised::from_none(
            ExceptionClass::TomlDecodeError,
            self.suffixed(pos, message),
        ))
    }

    fn suffixed(&self, pos: usize, message: &str) -> String {
        format!("{message} (at {})", self.coordinates(pos))
    }

    /// `coord_repr`: `end of document`, or `line L, column C` counted in code points.
    fn coordinates(&self, pos: usize) -> String {
        if pos >= self.src.len() {
            return "end of document".to_string();
        }
        let newline = code('\n');
        let before = &self.src[..pos];
        let line = before.iter().filter(|&&c| c == newline).count() + 1;
        let column = if line == 1 {
            pos + 1
        } else {
            let last_newline = before
                .iter()
                .rposition(|&c| c == newline)
                .expect("line > 1");
            pos - last_newline
        };
        format!("line {line}, column {column}")
    }

    /// `skip_chars`
    fn skip_chars(&self, mut pos: usize, is_member: fn(u32) -> bool) -> usize {
        while self.char_at(pos).is_some_and(is_member) {
            pos += 1;
        }
        pos
    }

    /// `skip_until`: the position of `expect` (or the end of the text), after rejecting an illegal character before it.
    fn skip_until(
        &self,
        pos: usize,
        expect: &str,
        error_on: Illegal,
        error_on_eof: bool,
    ) -> Result<usize, LoadsFailure> {
        let found = (pos..self.src.len()).find(|&candidate| self.starts_with(expect, candidate));
        let new_pos = match found {
            Some(found) => found,
            None => {
                let end = self.src.len();
                if error_on_eof {
                    return Err(
                        self.error_from_none(end, &format!("Expected {}", string_repr(expect)))
                    );
                }
                end
            }
        };
        let span = self.slice(pos, new_pos.saturating_sub(pos));
        if let Some(offset) = span.iter().position(|&c| error_on.contains(c)) {
            let at = pos + offset;
            return Err(self.error(
                at,
                &format!("Found invalid character {}", single_char_repr(self.src[at])),
            ));
        }
        Ok(new_pos)
    }

    fn skip_comment(&self, pos: usize) -> Result<usize, LoadsFailure> {
        if self.char_at(pos) == Some(code('#')) {
            return self.skip_until(pos + 1, "\n", Illegal::InLine, false);
        }
        Ok(pos)
    }

    fn skip_comments_and_array_ws(&self, mut pos: usize) -> Result<usize, LoadsFailure> {
        loop {
            let before = pos;
            pos = self.skip_chars(pos, is_whitespace_or_newline);
            pos = self.skip_comment(pos)?;
            if pos == before {
                return Ok(pos);
            }
        }
    }

    fn create_dict_rule(&self, pos: usize, out: &mut Output) -> Parsed<Key> {
        let pos = self.skip_chars(pos + 1, is_whitespace);
        let (pos, key) = self.parse_key(pos)?;

        if out.flags.is(&key, Flag::ExplicitNest) || out.flags.is(&key, Flag::Frozen) {
            return Err(self.error(pos, &format!("Cannot declare {} twice", key_repr(&key))));
        }
        out.flags.set(&key, Flag::ExplicitNest, false);
        match out.data.get_or_create_nest(&key, true) {
            Ok(_) => {}
            Err(problem) => return Err(self.nest_problem(pos, problem)),
        }

        if !self.starts_with("]", pos) {
            return Err(self.error(pos, "Expected ']' at the end of a table declaration"));
        }
        Ok((pos + 1, key))
    }

    fn create_list_rule(&self, pos: usize, out: &mut Output) -> Parsed<Key> {
        let pos = self.skip_chars(pos + 2, is_whitespace);
        let (pos, key) = self.parse_key(pos)?;

        if out.flags.is(&key, Flag::Frozen) {
            return Err(self.error(
                pos,
                &format!("Cannot mutate immutable namespace {}", key_repr(&key)),
            ));
        }
        // Free the namespace now that it points to another empty list item...
        out.flags.unset_all(&key);
        // ...but this key precisely is still prohibited from table declaration
        out.flags.set(&key, Flag::ExplicitNest, false);
        if let Err(problem) = out.data.append_nest_to_list(&key) {
            return Err(self.nest_problem(pos, problem));
        }

        if !self.starts_with("]]", pos) {
            return Err(self.error(pos, "Expected ']]' at the end of an array declaration"));
        }
        Ok((pos + 2, key))
    }

    /// The `except KeyError: raise suffixed_err(src, pos, "Cannot overwrite a value") from None` of the callers; an `IndexError` is not caught
    /// there and leaves `loads` as it is.
    fn nest_problem(&self, pos: usize, problem: NestError) -> LoadsFailure {
        match problem {
            NestError::NotANest => self.error_from_none(pos, "Cannot overwrite a value"),
            NestError::EmptyArray => LoadsFailure::Raised(Raised::plain(
                ExceptionClass::IndexError,
                "list index out of range".to_string(),
            )),
        }
    }

    fn key_value_rule(
        &mut self,
        pos: usize,
        out: &mut Output,
        header: &Key,
    ) -> Result<usize, LoadsFailure> {
        let (pos, (key, value)) = self.parse_key_value_pair(pos)?;
        let (key_stem, key_parent) = key.split_last().expect("a key has a part");
        let abs_key_parent: Key = header.iter().chain(key_parent).cloned().collect();

        for length in 1..key.len() {
            let cont_key: Key = header.iter().chain(&key[..length]).cloned().collect();
            // Check that dotted key syntax does not redefine an existing table
            if out.flags.is(&cont_key, Flag::ExplicitNest) {
                return Err(self.error(
                    pos,
                    &format!("Cannot redefine namespace {}", key_repr(&cont_key)),
                ));
            }
            // Containers in the relative path can't be opened with the table syntax or
            // dotted key/value syntax in following table sections.
            out.flags.add_pending(cont_key, Flag::ExplicitNest);
        }

        if out.flags.is(&abs_key_parent, Flag::Frozen) {
            return Err(self.error(
                pos,
                &format!(
                    "Cannot mutate immutable namespace {}",
                    key_repr(&abs_key_parent)
                ),
            ));
        }

        let nest = match out.data.get_or_create_nest(&abs_key_parent, true) {
            Ok(nest) => nest,
            Err(problem) => return Err(self.nest_problem(pos, problem)),
        };
        if nest.contains_key(key_stem) {
            return Err(self.error(pos, "Cannot overwrite a value"));
        }
        // Mark inline table and array namespaces recursively immutable
        if matches!(value, TomlValue::Table(_) | TomlValue::Array(_)) {
            let full_key: Key = header.iter().chain(&key).cloned().collect();
            out.flags.set(&full_key, Flag::Frozen, true);
        }
        nest.insert_new(key_stem.clone(), value);
        Ok(pos)
    }

    fn parse_key_value_pair(&mut self, pos: usize) -> Parsed<(Key, TomlValue)> {
        self.frames += 1;
        let result = self.key_value_pair_body(pos);
        self.frames -= 1;
        result
    }

    fn key_value_pair_body(&mut self, pos: usize) -> Parsed<(Key, TomlValue)> {
        let (pos, key) = self.parse_key(pos)?;
        if self.char_at(pos) != Some(code('=')) {
            return Err(self.error(pos, "Expected '=' after a key in a key/value pair"));
        }
        let pos = self.skip_chars(pos + 1, is_whitespace);
        let (pos, value) = self.parse_value(pos)?;
        Ok((pos, (key, value)))
    }

    fn parse_key(&self, pos: usize) -> Parsed<Key> {
        let (mut pos, key_part) = self.parse_key_part(pos)?;
        let mut key = vec![key_part];
        pos = self.skip_chars(pos, is_whitespace);
        loop {
            if self.char_at(pos) != Some(code('.')) {
                return Ok((pos, key));
            }
            pos = self.skip_chars(pos + 1, is_whitespace);
            let (next, key_part) = self.parse_key_part(pos)?;
            key.push(key_part);
            pos = self.skip_chars(next, is_whitespace);
        }
    }

    fn parse_key_part(&self, pos: usize) -> Parsed<Text> {
        match self.char_at(pos) {
            Some(c) if is_bare_key_char(c) => {
                let end = self.skip_chars(pos, is_bare_key_char);
                Ok((end, self.src[pos..end].to_vec()))
            }
            Some(c) if c == code('\'') => self.parse_literal_str(pos),
            Some(c) if c == code('"') => self.parse_one_line_basic_str(pos),
            _ => Err(self.error(pos, "Invalid initial character for a key part")),
        }
    }

    fn parse_one_line_basic_str(&self, pos: usize) -> Parsed<Text> {
        self.parse_basic_str(pos + 1, false)
    }

    fn parse_array(&mut self, pos: usize) -> Parsed<TomlValue> {
        self.frames += 1;
        let result = self.array_body(pos);
        self.frames -= 1;
        result
    }

    fn array_body(&mut self, pos: usize) -> Parsed<TomlValue> {
        let mut array = Vec::new();
        let mut pos = self.skip_comments_and_array_ws(pos + 1)?;
        if self.starts_with("]", pos) {
            return Ok((pos + 1, TomlValue::Array(array)));
        }
        loop {
            let (next, value) = self.parse_value(pos)?;
            array.push(value);
            pos = self.skip_comments_and_array_ws(next)?;

            let separator = self.slice(pos, 1);
            if separator == [code(']')] {
                return Ok((pos + 1, TomlValue::Array(array)));
            }
            if separator != [code(',')] {
                return Err(self.error(pos, "Unclosed array"));
            }
            pos += 1;

            pos = self.skip_comments_and_array_ws(pos)?;
            if self.starts_with("]", pos) {
                return Ok((pos + 1, TomlValue::Array(array)));
            }
        }
    }

    fn parse_inline_table(&mut self, pos: usize) -> Parsed<TomlValue> {
        self.frames += 1;
        let result = self.inline_table_body(pos);
        self.frames -= 1;
        result
    }

    fn inline_table_body(&mut self, pos: usize) -> Parsed<TomlValue> {
        let mut nested_dict = NestedDict::new();
        let mut flags = Flags::new();

        let mut pos = self.skip_chars(pos + 1, is_whitespace);
        if self.starts_with("}", pos) {
            return Ok((pos + 1, TomlValue::Table(nested_dict.into_table())));
        }
        loop {
            let (next, (key, value)) = self.parse_key_value_pair(pos)?;
            pos = next;
            let (key_stem, key_parent) = key.split_last().expect("a key has a part");
            if flags.is(&key, Flag::Frozen) {
                return Err(self.error(
                    pos,
                    &format!("Cannot mutate immutable namespace {}", key_repr(&key)),
                ));
            }
            let nest = match nested_dict.get_or_create_nest(key_parent, false) {
                Ok(nest) => nest,
                Err(problem) => return Err(self.nest_problem(pos, problem)),
            };
            if nest.contains_key(key_stem) {
                return Err(self.error(
                    pos,
                    &format!("Duplicate inline table key {}", text_repr(key_stem)),
                ));
            }
            let freezes = matches!(value, TomlValue::Table(_) | TomlValue::Array(_));
            nest.insert_new(key_stem.clone(), value);
            pos = self.skip_chars(pos, is_whitespace);
            let separator = self.slice(pos, 1);
            if separator == [code('}')] {
                return Ok((pos + 1, TomlValue::Table(nested_dict.into_table())));
            }
            if separator != [code(',')] {
                return Err(self.error(pos, "Unclosed inline table"));
            }
            if freezes {
                flags.set(&key, Flag::Frozen, true);
            }
            pos = self.skip_chars(pos + 1, is_whitespace);
        }
    }

    fn parse_basic_str_escape(&self, pos: usize, multiline: bool) -> Parsed<Text> {
        let escape_id = self.slice(pos, 2);
        let pos = pos + 2;
        let is_line_continuation = escape_id == [code('\\'), code(' ')]
            || escape_id == [code('\\'), code('\t')]
            || escape_id == [code('\\'), code('\n')];
        if multiline && is_line_continuation {
            // Skip whitespace until next non-whitespace character or end of the doc. Error if non-whitespace is found before newline.
            let mut pos = pos;
            if escape_id != [code('\\'), code('\n')] {
                pos = self.skip_chars(pos, is_whitespace);
                match self.char_at(pos) {
                    None => return Ok((pos, Vec::new())),
                    Some(c) if c != code('\n') => {
                        return Err(self.error(pos, "Unescaped '\\' in a string"))
                    }
                    Some(_) => pos += 1,
                }
            }
            pos = self.skip_chars(pos, is_whitespace_or_newline);
            return Ok((pos, Vec::new()));
        }
        if escape_id == [code('\\'), code('u')] {
            return self.parse_hex_char(pos, 4);
        }
        if escape_id == [code('\\'), code('U')] {
            return self.parse_hex_char(pos, 8);
        }
        let replacement = match escape_id {
            [92, 98] => 0x08,  // \b backspace
            [92, 116] => 0x09, // \t tab
            [92, 110] => 0x0A, // \n linefeed
            [92, 102] => 0x0C, // \f form feed
            [92, 114] => 0x0D, // \r carriage return
            [92, 34] => 0x22,  // \" quote
            [92, 92] => 0x5C,  // \\ backslash
            _ => return Err(self.error_from_none(pos, "Unescaped '\\' in a string")),
        };
        Ok((pos, vec![replacement]))
    }

    fn parse_hex_char(&self, pos: usize, hex_len: usize) -> Parsed<Text> {
        let hex = self.slice(pos, hex_len);
        let all_hex = hex
            .iter()
            .all(|&c| char::from_u32(c).is_some_and(|c| c.is_ascii_hexdigit()));
        if hex.len() != hex_len || !all_hex {
            return Err(self.error(pos, "Invalid hex value"));
        }
        let pos = pos + hex_len;
        let value = hex.iter().fold(0u32, |acc, &c| {
            acc * 16 + char::from_u32(c).and_then(|c| c.to_digit(16)).unwrap_or(0)
        });
        if !is_unicode_scalar_value(value) {
            return Err(self.error(pos, "Escaped character is not a Unicode scalar value"));
        }
        Ok((pos, vec![value]))
    }

    fn parse_literal_str(&self, pos: usize) -> Parsed<Text> {
        let start = pos + 1; // Skip starting apostrophe
        let end = self.skip_until(start, "'", Illegal::InLine, true)?;
        Ok((end + 1, self.src[start..end].to_vec())) // Skip ending apostrophe
    }

    fn parse_multiline_str(&self, pos: usize, literal: bool) -> Parsed<Text> {
        let mut pos = pos + 3;
        if self.starts_with("\n", pos) {
            pos += 1;
        }
        let (delimiter, mut pos, mut result) = if literal {
            let end = self.skip_until(pos, "'''", Illegal::InMultiline, true)?;
            (code('\''), end + 3, self.src[pos..end].to_vec())
        } else {
            let (after, result) = self.parse_basic_str(pos, true)?;
            (code('"'), after, result)
        };

        // Add at maximum two extra apostrophes/quotes if the end sequence is 4 or 5 chars long instead of just 3.
        if self.char_at(pos) != Some(delimiter) {
            return Ok((pos, result));
        }
        pos += 1;
        result.push(delimiter);
        if self.char_at(pos) != Some(delimiter) {
            return Ok((pos, result));
        }
        pos += 1;
        result.push(delimiter);
        Ok((pos, result))
    }

    fn parse_basic_str(&self, mut pos: usize, multiline: bool) -> Parsed<Text> {
        let error_on = if multiline {
            Illegal::InMultiline
        } else {
            Illegal::InLine
        };
        let mut result: Text = Vec::new();
        let mut start_pos = pos;
        loop {
            let Some(character) = self.char_at(pos) else {
                return Err(self.error_from_none(pos, "Unterminated string"));
            };
            if character == code('"') {
                if !multiline {
                    result.extend_from_slice(&self.src[start_pos..pos]);
                    return Ok((pos + 1, result));
                }
                if self.starts_with("\"\"\"", pos) {
                    result.extend_from_slice(&self.src[start_pos..pos]);
                    return Ok((pos + 3, result));
                }
                pos += 1;
                continue;
            }
            if character == code('\\') {
                result.extend_from_slice(&self.src[start_pos..pos]);
                let (after, parsed_escape) = self.parse_basic_str_escape(pos, multiline)?;
                pos = after;
                result.extend(parsed_escape);
                start_pos = pos;
                continue;
            }
            if error_on.contains(character) {
                return Err(self.error(
                    pos,
                    &format!("Illegal character {}", single_char_repr(character)),
                ));
            }
            pos += 1;
        }
    }

    fn parse_value(&mut self, pos: usize) -> Parsed<TomlValue> {
        self.frames += 1;
        let result = if self.frames > RECURSION_FRAME_BUDGET {
            Err(LoadsFailure::Unsupported(
                UnsupportedDomain::NestingBeyondRecursionLimit,
            ))
        } else {
            self.value_body(pos)
        };
        self.frames -= 1;
        result
    }

    fn value_body(&mut self, pos: usize) -> Parsed<TomlValue> {
        let character = self.char_at(pos);

        // Basic strings
        if character == Some(code('"')) {
            let (end, text) = if self.starts_with("\"\"\"", pos) {
                self.parse_multiline_str(pos, false)?
            } else {
                self.parse_one_line_basic_str(pos)?
            };
            return Ok((end, TomlValue::Str(text)));
        }

        // Literal strings
        if character == Some(code('\'')) {
            let (end, text) = if self.starts_with("'''", pos) {
                self.parse_multiline_str(pos, true)?
            } else {
                self.parse_literal_str(pos)?
            };
            return Ok((end, TomlValue::Str(text)));
        }

        // Booleans
        if character == Some(code('t')) && self.starts_with("true", pos) {
            return Ok((pos + 4, TomlValue::Bool(true)));
        }
        if character == Some(code('f')) && self.starts_with("false", pos) {
            return Ok((pos + 5, TomlValue::Bool(false)));
        }

        // Arrays
        if character == Some(code('[')) {
            return self.parse_array(pos);
        }

        // Inline tables
        if character == Some(code('{')) {
            return self.parse_inline_table(pos);
        }

        // Dates and times
        if let Some(matched) = match_datetime(self.src, pos) {
            return match datetime_value(&matched) {
                Ok(value) => Ok((matched.end, value)),
                Err(cause) => Err(LoadsFailure::Raised(Raised::from_cause(
                    ExceptionClass::TomlDecodeError,
                    self.suffixed(pos, "Invalid date or datetime"),
                    cause,
                ))),
            };
        }
        if let Some((end, time)) = match_local_time(self.src, pos) {
            return Ok((end, TomlValue::Time(time)));
        }

        // Integers and "normal" floats. The pattern matches greedily any type starting with a decimal char, so it comes after the dates and times.
        if let Some(matched) = match_number(self.src, pos) {
            return match number_value(self.src, pos, matched) {
                Ok(value) => Ok((matched.end, value)),
                Err(NumberFailure::Raised(raised)) => Err(LoadsFailure::Raised(raised)),
                Err(NumberFailure::Unsupported(domain)) => Err(LoadsFailure::Unsupported(domain)),
            };
        }

        // Special floats
        if let Some(value) = special_float(self.slice(pos, 3), 3) {
            return Ok((pos + 3, TomlValue::Float(value)));
        }
        if let Some(value) = special_float(self.slice(pos, 4), 4) {
            return Ok((pos + 4, TomlValue::Float(value)));
        }

        Err(self.error(pos, "Invalid value"))
    }
}

/// `float(text)` for the spellings the parser passes: `inf`/`nan` with an optional sign. The sign of a NaN is its sign bit.
fn special_float(text: &[u32], expected_len: usize) -> Option<f64> {
    let spelled: String = text.iter().filter_map(|&c| char::from_u32(c)).collect();
    if text.len() != expected_len || spelled.len() != text.len() {
        return None;
    }
    let (negative, name) = match spelled.as_str() {
        "inf" | "nan" => (false, spelled.as_str()),
        "+inf" | "+nan" => (false, &spelled[1..]),
        "-inf" | "-nan" => (true, &spelled[1..]),
        _ => return None,
    };
    let sign_bit = u64::from(negative) << 63;
    let bits = if name == "inf" {
        sign_bit | 0x7ff0_0000_0000_0000
    } else {
        sign_bit | 0x7ff8_0000_0000_0000
    };
    Some(f64::from_bits(bits))
}
