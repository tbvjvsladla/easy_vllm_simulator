//! A JSON reader that behaves like Python's `json.loads`, and the text decoders that feed it.
//!
//! The state files are written and read by Python, so the reader accepts what `json` accepts (`NaN`,
//! `Infinity`, lone surrogate escapes, arbitrary-size integers) and reports failures with the same messages and
//! character positions as `json.JSONDecodeError`. It follows the structure of CPython 3.12.3
//! `Modules/_json.c` (scanner) and `Lib/json/decoder.py`; see `notices/NOTICE.md`.

use crate::pystr::PyStr;
use crate::pyvalue::{json_nan_constant, PyDict, PyValue};
use num_bigint::BigInt;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

/// Deepest container nesting the C scanner accepts, counting the outermost container as level 1.
///
/// CPython 3.12.3 enforces its C recursion limit (10 000) with `Py_EnterRecursiveCall`; a few levels are used by
/// the callers. Measured on the original CLI (stdin and file alike): level 9 997 parses, level 9 998 raises
/// `RecursionError`.
const MAX_DEPTH: usize = 9997;
/// `sys.get_int_max_str_digits()` default.
const MAX_INT_DIGITS: usize = 4300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// `json.JSONDecodeError`.
    Decode {
        message: &'static str,
        line: usize,
        column: usize,
        position: usize,
    },
    /// `int()` refusing a decimal literal with more than 4300 digits (`ValueError`).
    IntegerTooLong { digits: usize },
    /// Input that is not UTF-8 (`UnicodeDecodeError`, a `ValueError`).
    Encoding(String),
    /// Nesting beyond the C recursion limit (`RecursionError`, not a `ValueError`); names the container kind.
    TooDeep { container: &'static str },
}

impl LoadError {
    /// The original CLI catches `(OSError, ValueError)` around loading; a `RecursionError` escapes it.
    pub fn is_value_error(&self) -> bool {
        !matches!(self, LoadError::TooDeep { .. })
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Decode { message, line, column, position } => {
                write!(f, "{message}: line {line} column {column} (char {position})")
            }
            LoadError::IntegerTooLong { digits } => write!(
                f,
                "Exceeds the limit ({MAX_INT_DIGITS} digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
            ),
            LoadError::Encoding(message) => f.write_str(message),
            LoadError::TooDeep { container } => write!(
                f,
                "RecursionError: maximum recursion depth exceeded while decoding a JSON {container} from a unicode string"
            ),
        }
    }
}

impl std::error::Error for LoadError {}

/// Universal-newline translation of a text stream opened with `newline=None`.
pub(crate) fn translate_newlines(points: Vec<u32>) -> PyStr {
    const CR: u32 = 0x0D;
    const LF: u32 = 0x0A;
    if !points.contains(&CR) {
        return PyStr::from_code_points(points);
    }
    let mut out = Vec::with_capacity(points.len());
    let mut index = 0;
    while index < points.len() {
        if points[index] == CR {
            out.push(LF);
            if points.get(index + 1) == Some(&LF) {
                index += 1;
            }
        } else {
            out.push(points[index]);
        }
        index += 1;
    }
    PyStr::from_code_points(out)
}

/// `open(path, encoding="utf-8").read()`: strict UTF-8 and universal newlines.
///
/// The message names the offending bytes the way `UnicodeDecodeError` does: one byte as `byte 0xXX in position
/// P`, a longer invalid or truncated sequence as `bytes in position P-Q`.
pub fn decode_file_text(bytes: &[u8]) -> Result<PyStr, LoadError> {
    let text = decode_utf8_strict(bytes)?;
    // without a carriage return there is nothing to translate (and nothing to copy)
    if !bytes.contains(&b'\r') {
        return Ok(text);
    }
    Ok(translate_newlines(text.code_points().to_vec()))
}

/// `bytes.decode("utf-8")`: strict, with no newline translation (what `open()` adds on top of it).
pub fn decode_utf8_strict(bytes: &[u8]) -> Result<PyStr, LoadError> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(PyStr::new(text)),
        Err(error) => Err(LoadError::Encoding(utf8_error_text(bytes, &error))),
    }
}

/// The `start`, the length of the undecodable run (`end - start`) and the `reason` of the `UnicodeDecodeError` that
/// `bytes.decode("utf-8")` raises for the failure `from_utf8` reported: the constructor fields, not a message.
pub fn utf8_error_parts(bytes: &[u8], error: &std::str::Utf8Error) -> (usize, usize, &'static str) {
    let start = error.valid_up_to();
    match error.error_len() {
        None => (start, bytes.len() - start, "unexpected end of data"),
        Some(length) if (0xC2..=0xF4).contains(&bytes[start]) => {
            (start, length, "invalid continuation byte")
        }
        Some(length) => (start, length, "invalid start byte"),
    }
}

/// `str(UnicodeDecodeError)` of `bytes.decode("utf-8")` for the failure `from_utf8` reported: the offending byte(s)
/// named by position and the reason Python gives.
pub fn utf8_error_text(bytes: &[u8], error: &std::str::Utf8Error) -> String {
    let (start, length, reason) = utf8_error_parts(bytes, error);
    let subject = if length == 1 {
        format!("byte 0x{:02x} in position {start}", bytes[start])
    } else {
        format!("bytes in position {start}-{}", start + length - 1)
    };
    format!("'utf-8' codec can't decode {subject}: {reason}")
}

/// `sys.stdin.read()` on this interpreter: UTF-8 with `surrogateescape` and **no** newline translation. CPython opens
/// standard input with `newline="\n"` on POSIX, so `\r\n` and a bare `\r` arrive as they were written (unlike a text
/// file opened by name, see [`decode_file_text`]); the saved original reports a JSON error in such a document at the
/// position that counts the `\r`. Undecodable bytes become `U+DC80..U+DCFF`, so they survive to the output unchanged.
pub fn decode_stdin_text(bytes: &[u8]) -> PyStr {
    PyStr::from_surrogateescape(bytes)
}

/// `json.loads(text)`.
pub fn loads(text: &str) -> Result<PyValue, LoadError> {
    loads_text(&PyStr::new(text))
}

/// `json.loads(text)` for text that may hold lone surrogates.
///
/// Recursion follows the document's nesting, up to [`MAX_DEPTH`] levels; callers parsing untrusted deep
/// documents on a small stack should run on a thread with a large one (`cli::run` does).
pub fn loads_text(text: &PyStr) -> Result<PyValue, LoadError> {
    loads_text_with(text, false)
}

/// `json.loads(text)` where the literal `NaN` is the one object Python's decoder returns for it (the same object in
/// every document of the process), so that a program that tests membership finds it again. Everything else is
/// [`loads_text`]; the plain decoder keeps `Float` values, which most readers match on directly.
pub fn loads_objects(text: &str) -> Result<PyValue, LoadError> {
    loads_text_with(&PyStr::new(text), true)
}

/// [`loads_objects`] for text that may hold lone surrogates.
pub fn loads_text_objects(text: &PyStr) -> Result<PyValue, LoadError> {
    loads_text_with(text, true)
}

fn loads_text_with(text: &PyStr, nan_objects: bool) -> Result<PyValue, LoadError> {
    read_document(text, nan_objects, false).map_err(|error| match error {
        UniqueError::Load(error) => error,
        UniqueError::DuplicateKey(_) => {
            unreachable!("only loads_text_unique asks the reader to look for repeated keys")
        }
    })
}

/// What [`loads_text_unique`] can end with: the reader's own error, or the first repeated key of an object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UniqueError {
    Load(LoadError),
    /// `object_pairs_hook` found `key` twice in one object (raised when that object was completely read).
    DuplicateKey(PyStr),
}

/// `json.loads(text, object_pairs_hook=hook)` for a hook that rejects an object holding a key twice: the same reader as
/// [`loads_text`], but the first key an object repeats is reported, at the moment that object has been read to its closing
/// brace (innermost objects first, empty objects included). An error found later in the document does not hide it, and an
/// error inside the object before its closing brace comes first, as in the C scanner. No dictionary is built differently
/// for a document without repeated keys: the value equals what [`loads_text`] returns.
pub fn loads_text_unique(text: &PyStr) -> Result<PyValue, UniqueError> {
    read_document(text, false, true)
}

fn read_document(text: &PyStr, nan_objects: bool, unique: bool) -> Result<PyValue, UniqueError> {
    let reader = Reader {
        points: text.code_points(),
        nan_objects,
        unique,
    };
    let failed = |error: LoadError| UniqueError::Load(error);
    if reader.at(0) == Some(0xFEFF) {
        return Err(failed(
            reader.decode_error("Unexpected UTF-8 BOM (decode using utf-8-sig)", 0),
        ));
    }
    let start = reader.skip_whitespace(0);
    let (value, end) = reader
        .value(start, 0)
        .map_err(|failure| reader.finish(failure))?;
    let end = reader.skip_whitespace(end);
    if end != reader.points.len() {
        return Err(failed(reader.decode_error("Extra data", end)));
    }
    Ok(value)
}

/// An object of at most this many entries is settled by comparing its keys pairwise (no allocation, and cheaper than hashing a handful of keys); a
/// larger one by a hash index of its keys' code points, so a large object costs time linear in its size, not quadratic.
const SMALL_OBJECT: usize = 16;

/// The repeated keys of the finished `entries`, as `(position of the later entry, position of the first entry with that key)` in document order of
/// the later entries. Keys are equal when their code points are equal (the equality of [`PyStr`]: lone surrogates included). The hash index borrows
/// the keys' code-point slices only for this scan; nothing about its iteration order is ever observed.
fn repeated_keys(entries: &[(PyStr, PyValue)]) -> Vec<(usize, usize)> {
    let mut repeats = Vec::new();
    if entries.len() <= SMALL_OBJECT {
        for (later, (key, _)) in entries.iter().enumerate() {
            if let Some(first) = entries[..later]
                .iter()
                .position(|(earlier, _)| earlier == key)
            {
                repeats.push((later, first));
            }
        }
    } else {
        let mut first_at: HashMap<&[u32], usize> = HashMap::with_capacity(entries.len());
        for (position, (key, _)) in entries.iter().enumerate() {
            match first_at.entry(key.code_points()) {
                Entry::Occupied(first) => repeats.push((position, *first.get())),
                Entry::Vacant(slot) => {
                    slot.insert(position);
                }
            }
        }
    }
    repeats
}

/// The object a reader has read to its closing brace: `entries` in document order. A key that repeats keeps the position of its first occurrence and
/// the value of its last (as assigning to a dict does). With `unique`, the first key to repeat (the earliest later occurrence, in document order) is
/// returned instead, whatever came after it.
fn settle(mut entries: Vec<(PyStr, PyValue)>, unique: bool) -> Result<PyDict, PyStr> {
    let repeats = repeated_keys(&entries);
    let Some(&(first_repeat, _)) = repeats.first() else {
        return Ok(PyDict::from_settled_entries(entries));
    };
    if unique {
        return Err(entries[first_repeat].0.clone());
    }
    let mut dropped = vec![false; entries.len()];
    for &(later, first) in &repeats {
        // in document order, so the last value stays
        let value = std::mem::replace(&mut entries[later].1, PyValue::None);
        entries[first].1 = value;
        dropped[later] = true;
    }
    let settled = entries
        .into_iter()
        .zip(dropped)
        .filter_map(|(entry, dropped)| (!dropped).then_some(entry))
        .collect();
    Ok(PyDict::from_settled_entries(settled))
}

enum Failure {
    /// Python's `StopIteration(idx)`: no value starts at `idx`.
    NoValue(usize),
    Decode(&'static str, usize),
    Other(LoadError),
    /// A repeated key found by the reader that was asked to look for them (see [`loads_text_unique`]).
    Duplicate(PyStr),
}

struct Reader<'a> {
    points: &'a [u32],
    /// Whether `NaN` decodes to the shared object (see [`loads_objects`]).
    nan_objects: bool,
    /// Whether an object that repeats a key is reported (see [`loads_text_unique`]).
    unique: bool,
}

fn code(c: char) -> u32 {
    u32::from(c)
}

impl Reader<'_> {
    fn at(&self, index: usize) -> Option<u32> {
        self.points.get(index).copied()
    }

    fn is_at(&self, index: usize, c: char) -> bool {
        self.at(index) == Some(code(c))
    }

    fn digit_at(&self, index: usize) -> bool {
        self.at(index).is_some_and(|c| (0x30..=0x39).contains(&c))
    }

    fn skip_whitespace(&self, mut index: usize) -> usize {
        while matches!(self.at(index), Some(0x20 | 0x09 | 0x0A | 0x0D)) {
            index += 1;
        }
        index
    }

    fn decode_error(&self, message: &'static str, position: usize) -> LoadError {
        let before = &self.points[..position.min(self.points.len())];
        let line = before.iter().filter(|&&c| c == 0x0A).count() + 1;
        let column = match before.iter().rposition(|&c| c == 0x0A) {
            Some(newline) => position - newline,
            None => position + 1,
        };
        LoadError::Decode {
            message,
            line,
            column,
            position,
        }
    }

    fn finish(&self, failure: Failure) -> UniqueError {
        UniqueError::Load(match failure {
            Failure::NoValue(position) => self.decode_error("Expecting value", position),
            Failure::Decode(message, position) => self.decode_error(message, position),
            Failure::Other(error) => error,
            Failure::Duplicate(key) => return UniqueError::DuplicateKey(key),
        })
    }

    fn matches_literal(&self, index: usize, literal: &str) -> bool {
        literal
            .chars()
            .enumerate()
            .all(|(offset, expected)| self.is_at(index + offset, expected))
    }

    fn value(&self, index: usize, depth: usize) -> Result<(PyValue, usize), Failure> {
        let Some(first) = self.at(index).and_then(char::from_u32) else {
            return Err(Failure::NoValue(index));
        };
        match first {
            '"' => self
                .string(index + 1)
                .map(|(text, next)| (PyValue::Str(text), next)),
            '{' => self.object(index + 1, depth + 1),
            '[' => self.array(index + 1, depth + 1),
            'n' if self.matches_literal(index, "null") => Ok((PyValue::None, index + 4)),
            't' if self.matches_literal(index, "true") => Ok((PyValue::Bool(true), index + 4)),
            'f' if self.matches_literal(index, "false") => Ok((PyValue::Bool(false), index + 5)),
            'N' if self.matches_literal(index, "NaN") => Ok((
                if self.nan_objects {
                    json_nan_constant()
                } else {
                    PyValue::Float(f64::NAN)
                },
                index + 3,
            )),
            'I' if self.matches_literal(index, "Infinity") => {
                Ok((PyValue::Float(f64::INFINITY), index + 8))
            }
            '-' if self.matches_literal(index, "-Infinity") => {
                Ok((PyValue::Float(f64::NEG_INFINITY), index + 9))
            }
            _ => self.number(index),
        }
    }

    fn number(&self, start: usize) -> Result<(PyValue, usize), Failure> {
        let last = self.points.len().saturating_sub(1);
        let mut index = start;
        if self.is_at(index, '-') {
            index += 1;
            if index > last {
                return Err(Failure::NoValue(start));
            }
        }
        match self.at(index) {
            Some(0x31..=0x39) => {
                index += 1;
                while self.digit_at(index) {
                    index += 1;
                }
            }
            Some(0x30) => index += 1,
            _ => return Err(Failure::NoValue(start)),
        }
        let mut is_float = false;
        if index < last && self.is_at(index, '.') && self.digit_at(index + 1) {
            is_float = true;
            index += 2;
            while self.digit_at(index) {
                index += 1;
            }
        }
        if index < last && (self.is_at(index, 'e') || self.is_at(index, 'E')) {
            let exponent_start = index;
            index += 1;
            if index < last && (self.is_at(index, '-') || self.is_at(index, '+')) {
                index += 1;
            }
            while self.digit_at(index) {
                index += 1;
            }
            if self.digit_at(index - 1) {
                is_float = true;
            } else {
                index = exponent_start;
            }
        }
        let literal: String = self.points[start..index]
            .iter()
            .filter_map(|&c| char::from_u32(c))
            .collect();
        if is_float {
            let number = literal
                .parse::<f64>()
                .expect("a JSON number literal is a valid f64");
            return Ok((PyValue::Float(number), index));
        }
        let digits = literal.trim_start_matches('-').len();
        if digits > MAX_INT_DIGITS {
            return Err(Failure::Other(LoadError::IntegerTooLong { digits }));
        }
        let number: BigInt = literal
            .parse()
            .expect("a JSON integer literal is a valid integer");
        Ok((PyValue::Int(number), index))
    }

    /// `scanstring_unicode`: `start` is the index after the opening quote.
    fn string(&self, start: usize) -> Result<(PyStr, usize), Failure> {
        let unterminated = || Failure::Decode("Unterminated string starting at", start - 1);
        // The ordinary code points up to the first quote, backslash or control character are copied once, with their exact length. Only a string that
        // holds an escape or a control character goes on to the loop below, which starts at that character and handles it as it always did.
        let rest = self.points.get(start..).unwrap_or(&[]);
        let Some(offset) = rest
            .iter()
            .position(|&c| c == 0x22 || c == 0x5C || c <= 0x1F)
        else {
            return Err(unterminated());
        };
        if rest[offset] == 0x22 {
            return Ok((
                PyStr::from_code_points(rest[..offset].to_vec()),
                start + offset + 1,
            ));
        }
        let mut points: Vec<u32> = rest[..offset].to_vec();
        let mut index = start + offset;
        loop {
            let Some(c) = self.at(index) else {
                return Err(unterminated());
            };
            match c {
                0x22 => return Ok((PyStr::from_code_points(points), index + 1)),
                0x5C => {
                    index += 1;
                    let Some(escape_point) = self.at(index) else {
                        return Err(unterminated());
                    };
                    let escape = char::from_u32(escape_point).unwrap_or('\0');
                    if escape != 'u' {
                        let decoded = match escape {
                            '"' => '"',
                            '\\' => '\\',
                            '/' => '/',
                            'b' => '\u{8}',
                            'f' => '\u{c}',
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            _ => return Err(Failure::Decode("Invalid \\escape", index - 1)),
                        };
                        points.push(code(decoded));
                        index += 1;
                        continue;
                    }
                    index += 1;
                    let mut unit = self.hex4(index, index + 4)?;
                    index += 4;
                    if (0xD800..0xDC00).contains(&unit)
                        && index + 6 < self.points.len()
                        && self.is_at(index, '\\')
                        && self.is_at(index + 1, 'u')
                    {
                        let low = self.hex4(index + 2, index + 6)?;
                        if (0xDC00..0xE000).contains(&low) {
                            unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                            index += 6;
                        }
                    }
                    points.push(unit);
                }
                c if c <= 0x1F => {
                    return Err(Failure::Decode("Invalid control character at", index))
                }
                c => {
                    points.push(c);
                    index += 1;
                }
            }
        }
    }

    /// Four hex digits at `from..to`. The closing quote must follow (`end >= len` is an error in CPython).
    fn hex4(&self, from: usize, to: usize) -> Result<u32, Failure> {
        if to >= self.points.len() {
            return Err(Failure::Decode("Invalid \\uXXXX escape", from - 1));
        }
        let mut unit = 0;
        for index in from..to {
            let digit = self
                .at(index)
                .and_then(char::from_u32)
                .and_then(|c| c.to_digit(16));
            unit = (unit << 4) | digit.ok_or(Failure::Decode("Invalid \\uXXXX escape", to - 5))?;
        }
        Ok(unit)
    }

    fn object(&self, start: usize, depth: usize) -> Result<(PyValue, usize), Failure> {
        if depth > MAX_DEPTH {
            return Err(Failure::Other(LoadError::TooDeep {
                container: "object",
            }));
        }
        let mut entries: Vec<(PyStr, PyValue)> = Vec::new();
        let mut index = self.skip_whitespace(start);
        if self.is_at(index, '}') {
            return Ok((PyValue::Dict(PyDict::new()), index + 1));
        }
        loop {
            if !self.is_at(index, '"') {
                return Err(Failure::Decode(
                    "Expecting property name enclosed in double quotes",
                    index,
                ));
            }
            let (key, after_key) = self.string(index + 1)?;
            index = self.skip_whitespace(after_key);
            if !self.is_at(index, ':') {
                return Err(Failure::Decode("Expecting ':' delimiter", index));
            }
            index = self.skip_whitespace(index + 1);
            let (value, after_value) = self.value(index, depth)?;
            entries.push((key, value));
            index = self.skip_whitespace(after_value);
            if self.is_at(index, '}') {
                // repeated keys are settled only now, at the closing brace: every error found before it comes first
                return match settle(entries, self.unique) {
                    Ok(map) => Ok((PyValue::Dict(map), index + 1)),
                    Err(key) => Err(Failure::Duplicate(key)),
                };
            }
            if !self.is_at(index, ',') {
                return Err(Failure::Decode("Expecting ',' delimiter", index));
            }
            index = self.skip_whitespace(index + 1);
        }
    }

    fn array(&self, start: usize, depth: usize) -> Result<(PyValue, usize), Failure> {
        if depth > MAX_DEPTH {
            return Err(Failure::Other(LoadError::TooDeep { container: "array" }));
        }
        let mut items = Vec::new();
        let mut index = self.skip_whitespace(start);
        if self.is_at(index, ']') {
            return Ok((PyValue::List(items), index + 1));
        }
        loop {
            let (value, after_value) = self.value(index, depth)?;
            items.push(value);
            index = self.skip_whitespace(after_value);
            if self.is_at(index, ']') {
                return Ok((PyValue::List(items), index + 1));
            }
            if !self.is_at(index, ',') {
                return Err(Failure::Decode("Expecting ',' delimiter", index));
            }
            index = self.skip_whitespace(index + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_python_number_types() {
        let value =
            loads("[1, 1.0, -0, 1e2, 12345678901234567890123, true, NaN, -Infinity]").unwrap();
        assert_eq!(
            value.repr(),
            "[1, 1.0, 0, 100.0, 12345678901234567890123, True, nan, -inf]"
        );
    }

    #[test]
    fn duplicate_keys_keep_first_position_and_last_value() {
        let value = loads(r#"{"a": 1, "b": 2, "a": 3}"#).unwrap();
        assert_eq!(value.repr(), "{'a': 3, 'b': 2}");
    }

    #[test]
    fn the_unique_reader_reports_the_first_repeated_key_when_its_object_closes() {
        let text = |text: &str| PyStr::new(text);
        let value =
            loads_text_unique(&text(r#"{"a": {"b": 1}, "c": [{"d": 2}], "e": {}}"#)).unwrap();
        assert_eq!(
            value,
            loads(r#"{"a": {"b": 1}, "c": [{"d": 2}], "e": {}}"#).unwrap()
        );
        let repeated = |text: &str| loads_text_unique(&PyStr::new(text)).unwrap_err();
        assert_eq!(
            repeated(r#"{"a": 1, "b": 2, "a": 3}"#),
            UniqueError::DuplicateKey(PyStr::new("a"))
        );
        // At every nesting level, the inner object (closed first) is the one reported.
        assert_eq!(
            repeated(r#"{"x": 1, "y": {"k": 1, "k": 2}, "x": 3}"#),
            UniqueError::DuplicateKey(PyStr::new("k"))
        );
        // The same key in two different objects is not a repetition.
        assert!(loads_text_unique(&text(r#"{"a": {"k": 1}, "b": {"k": 2}}"#)).is_ok());
        // An error inside the object before its closing brace comes first; a later one cannot hide a closed object's repetition.
        assert!(matches!(
            repeated(r#"{"a": 1, "a": 2 "z""#),
            UniqueError::Load(LoadError::Decode { .. })
        ));
        assert_eq!(
            repeated(r#"[{"k": 1, "k": 2}, oops"#),
            UniqueError::DuplicateKey(PyStr::new("k"))
        );
        // The plain reader is unchanged: the repeated key keeps its first position and last value.
        assert_eq!(loads(r#"{"a": 1, "a": 2}"#).unwrap().repr(), "{'a': 2}");
    }

    #[test]
    fn lone_surrogates_survive_and_pairs_combine() {
        assert_eq!(loads(r#""\ud800""#).unwrap().repr(), "'\\ud800'");
        assert_eq!(loads(r#""\ud83d\ude00""#).unwrap().repr(), "'😀'");
    }

    #[test]
    fn errors_carry_python_messages_and_positions() {
        let cases = [
            ("", "Expecting value: line 1 column 1 (char 0)"),
            (
                "{",
                "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)",
            ),
            ("[1 2]", "Expecting ',' delimiter: line 1 column 4 (char 3)"),
            (
                "{\"a\" 1}",
                "Expecting ':' delimiter: line 1 column 6 (char 5)",
            ),
            ("[1,]", "Expecting value: line 1 column 4 (char 3)"),
            ("1 2", "Extra data: line 1 column 3 (char 2)"),
            (
                "\"abc",
                "Unterminated string starting at: line 1 column 1 (char 0)",
            ),
            (
                "\"a\tb\"",
                "Invalid control character at: line 1 column 3 (char 2)",
            ),
            ("\"\\x\"", "Invalid \\escape: line 1 column 2 (char 1)"),
            (
                "\"\\u12g4\"",
                "Invalid \\uXXXX escape: line 1 column 3 (char 2)",
            ),
            (
                "\u{feff}{}",
                "Unexpected UTF-8 BOM (decode using utf-8-sig): line 1 column 1 (char 0)",
            ),
            ("[\n1,\n}", "Expecting value: line 3 column 1 (char 5)"),
        ];
        for (text, expected) in cases {
            assert_eq!(loads(text).unwrap_err().to_string(), expected, "{text:?}");
        }
    }

    #[test]
    fn strict_decoding_reports_the_span_python_reports() {
        let message = |bytes: &[u8]| decode_file_text(bytes).unwrap_err().to_string();
        assert_eq!(
            message(b"{\"a\":\xff}"),
            "'utf-8' codec can't decode byte 0xff in position 5: invalid start byte"
        );
        assert_eq!(
            message(b"{\"a\":\xe1\x80A"),
            "'utf-8' codec can't decode bytes in position 5-6: invalid continuation byte"
        );
        assert_eq!(
            message(b"{\"a\":\xf0\x90"),
            "'utf-8' codec can't decode bytes in position 5-6: unexpected end of data"
        );
        assert_eq!(
            message(b"{\"a\":\xe1"),
            "'utf-8' codec can't decode byte 0xe1 in position 5: unexpected end of data"
        );
        assert_eq!(
            message(b"{\"a\":\xed\xa0\x80"),
            "'utf-8' codec can't decode byte 0xed in position 5: invalid continuation byte"
        );
    }

    #[test]
    fn stdin_decoding_escapes_bad_bytes_and_keeps_every_newline_form_while_a_file_translates_them()
    {
        let text = decode_stdin_text(b"a\r\nb\rc\xff");
        assert_eq!(
            text.code_points(),
            &[0x61, 0x0D, 0x0A, 0x62, 0x0D, 0x63, 0xDCFF]
        );
        assert_eq!(
            decode_file_text(b"x\r\ny").unwrap().code_points(),
            &[0x78, 0x0A, 0x79]
        );
    }

    // ── objects: settling repeated keys ─────────────────────────────────────────────────────────

    /// A JSON string of the code points, ASCII-only: quotes, backslashes, controls, surrogates and everything above `~` as `\uXXXX` escapes (a
    /// character beyond the BMP as its surrogate pair), so the same key can be written in several spellings.
    fn json_string(points: &[u32]) -> String {
        let mut out = String::from("\"");
        for &point in points {
            match point {
                0x22 => out.push_str("\\\""),
                0x5C => out.push_str("\\\\"),
                0x20..=0x7E => out.push(char::from_u32(point).expect("ASCII")),
                0x10000.. => {
                    let offset = point - 0x10000;
                    out.push_str(&format!(
                        "\\u{:04x}\\u{:04x}",
                        0xD800 + (offset >> 10),
                        0xDC00 + (offset & 0x3FF)
                    ));
                }
                _ => out.push_str(&format!("\\u{point:04x}")),
            }
        }
        out.push('"');
        out
    }

    fn points(text: &str) -> Vec<u32> {
        text.chars().map(u32::from).collect()
    }

    /// The object text of `(key, value)` pairs in the given order, values being integers.
    fn object_text(pairs: &[(Vec<u32>, i64)]) -> String {
        let members: Vec<String> = pairs
            .iter()
            .map(|(key, value)| format!("{}: {value}", json_string(key)))
            .collect();
        format!("{{{}}}", members.join(", "))
    }

    /// The dict the long-standing way: one `PyDict::insert` per pair, in document order (first position, last value). This code path does not
    /// belong to the object reader, so it is an independent reference for it.
    fn inserted(pairs: &[(Vec<u32>, i64)]) -> PyValue {
        let mut map = PyDict::new();
        for (key, value) in pairs {
            map.insert(PyStr::from_code_points(key.clone()), PyValue::int(*value));
        }
        PyValue::Dict(map)
    }

    fn parsed(text: &str) -> PyValue {
        loads_text(&PyStr::new(text)).expect("a document that parses")
    }

    /// `pad` distinct keys placed first, so the same repeated structure is read by the pairwise branch (`pad` 0) and by the hash branch (`pad` 20).
    fn padded(pad: usize, pairs: &[(&str, i64)]) -> Vec<(Vec<u32>, i64)> {
        (0..pad)
            .map(|at| (points(&format!("pad{at}")), -1 - at as i64))
            .chain(pairs.iter().map(|(key, value)| (points(key), *value)))
            .collect()
    }

    #[test]
    fn objects_of_every_size_equal_the_dict_built_by_insertion() {
        for size in [0usize, 1, 2, 15, 16, 17, 64, 2048] {
            let pairs: Vec<(Vec<u32>, i64)> = (0..size)
                .map(|at| (points(&format!("model.layers.{at}.mlp.weight")), at as i64))
                .collect();
            assert_eq!(
                parsed(&object_text(&pairs)),
                inserted(&pairs),
                "{size} keys"
            );
            // the same object nested in an array and as a member of another object
            let nested = format!("[{0}, {{\"inner\": {0}}}]", object_text(&pairs));
            let PyValue::List(items) = parsed(&nested) else {
                panic!("a list")
            };
            assert_eq!(items[0], inserted(&pairs));
        }
    }

    #[test]
    fn repeated_keys_keep_the_first_position_and_the_last_value_on_both_branches() {
        let cases: [&[(&str, i64)]; 7] = [
            &[("a", 1), ("b", 2), ("a", 3)],
            &[("a", 1), ("a", 2), ("a", 3), ("a", 4)],
            &[("b", 1), ("a", 2), ("c", 3), ("a", 4), ("b", 5), ("a", 6)],
            &[("a", 1), ("b", 2), ("c", 3), ("c", 4)],
            &[("x", 1), ("x", 2), ("y", 3), ("z", 4)],
            &[("", 1), ("k", 2), ("", 3)],
            &[("ab", 1), ("abc", 2), ("ab", 3), ("abc", 4), ("a", 5)],
        ];
        for (at, case) in cases.iter().enumerate() {
            for pad in [0usize, 20] {
                let pairs = padded(pad, case);
                assert_eq!(
                    parsed(&object_text(&pairs)),
                    inserted(&pairs),
                    "case {at}, pad {pad}"
                );
            }
        }
        // a fixed deterministic corpus: sizes 1..=40, key ids drawn from a pool smaller than the object, so repeats are common
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move |bound: u64| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) % bound
        };
        for size in 1..=40usize {
            let pool = (size as u64 / 2).max(1);
            let pairs: Vec<(Vec<u32>, i64)> = (0..size)
                .map(|at| (points(&format!("k.{}.w", next(pool))), at as i64))
                .collect();
            assert_eq!(
                parsed(&object_text(&pairs)),
                inserted(&pairs),
                "corpus object of {size}"
            );
        }
    }

    #[test]
    fn keys_are_equal_when_their_code_points_are_equal_however_they_are_written() {
        let pair = |key: &str, value: i64| (points(key), value);
        for pad in [0usize, 20] {
            let before: Vec<(Vec<u32>, i64)> = padded(pad, &[]);
            // `"ab"` written three ways is one key: the first position, the last value
            let text = format!(
                "{{{}\"ab\": 1, \"a\\u0062\": 2, \"\\u0061b\": 3}}",
                before
                    .iter()
                    .map(|(key, value)| format!("{}: {value}, ", json_string(key)))
                    .collect::<String>()
            );
            let mut expected = before.clone();
            expected.extend([pair("ab", 1), pair("ab", 2), pair("ab", 3)]);
            assert_eq!(
                parsed(&text),
                inserted(&expected),
                "escaped spellings, pad {pad}"
            );
            // a character beyond the BMP: the literal and its surrogate-pair escape are the same key
            let text = format!(
                "{{{}\"\u{1F600}\": 1, \"\\ud83d\\ude00\": 2}}",
                before
                    .iter()
                    .map(|(key, value)| format!("{}: {value}, ", json_string(key)))
                    .collect::<String>()
            );
            let mut expected = before.clone();
            expected.extend([(vec![0x1F600], 1), (vec![0x1F600], 2)]);
            assert_eq!(parsed(&text), inserted(&expected), "astral key, pad {pad}");
            // lone surrogates: the same lone surrogate repeats, two different ones, or one against a pair half, do not
            let lone = |units: &[u32]| units.to_vec();
            let mut pairs = before.clone();
            pairs.extend([
                (lone(&[0xD800]), 1),
                (lone(&[0xDC00]), 2),
                (lone(&[0xD800]), 3),
                (lone(&[0xD800, 0x41]), 4),
                (lone(&[0xDC00, 0xD800]), 5),
            ]);
            let text = object_text(&pairs);
            assert_eq!(
                parsed(&text),
                inserted(&pairs),
                "lone surrogates, pad {pad}"
            );
            let PyValue::Dict(map) = parsed(&text) else {
                panic!("a dict")
            };
            assert_eq!(map.len(), pad + 4);
            // keys that differ only in length or after a long common prefix are different keys
            let long = "p".repeat(40);
            let pairs = padded(
                pad,
                &[
                    ("a", 1),
                    ("aa", 2),
                    (&format!("{long}x"), 3),
                    (&format!("{long}y"), 4),
                    (&format!("{long}x"), 5),
                ],
            );
            assert_eq!(
                parsed(&object_text(&pairs)),
                inserted(&pairs),
                "prefixes, pad {pad}"
            );
        }
    }

    #[test]
    fn the_unique_reader_reports_the_first_repeated_key_in_document_order_on_both_branches() {
        let repeated = |pad: usize, pairs: &[(&str, i64)]| {
            loads_text_unique(&PyStr::new(&object_text(&padded(pad, pairs)))).unwrap_err()
        };
        for pad in [0usize, 20] {
            // several keys repeat: the one whose SECOND occurrence comes first is reported, not the first key of the object
            assert_eq!(
                repeated(pad, &[("a", 1), ("b", 2), ("b", 3), ("a", 4)]),
                UniqueError::DuplicateKey(PyStr::new("b"))
            );
            assert_eq!(
                repeated(pad, &[("a", 1), ("a", 2), ("b", 3), ("b", 4)]),
                UniqueError::DuplicateKey(PyStr::new("a"))
            );
            // a key repeated three times is still reported once, by its key
            assert_eq!(
                repeated(pad, &[("k", 1), ("k", 2), ("k", 3)]),
                UniqueError::DuplicateKey(PyStr::new("k"))
            );
            // a document without repeats is the plain reader's value
            let pairs = padded(pad, &[("a", 1), ("b", 2)]);
            assert_eq!(
                loads_text_unique(&PyStr::new(&object_text(&pairs))).unwrap(),
                inserted(&pairs)
            );
        }
    }

    #[test]
    fn in_the_unique_reader_every_error_before_the_closing_brace_comes_before_a_repeat_and_a_closed_repeat_comes_before_a_later_error(
    ) {
        let head =
            |pad: usize| -> String { (0..pad).map(|at| format!("\"pad{at}\": {at}, ")).collect() };
        for pad in [0usize, 20] {
            let unique = |text: String| loads_text_unique(&PyStr::new(&text)).unwrap_err();
            // a syntax error after the repeat but before the closing brace: the syntax error
            assert!(matches!(
                unique(format!("{{{}\"a\": 1, \"a\": 2 \"z\"", head(pad))),
                UniqueError::Load(LoadError::Decode {
                    message: "Expecting ',' delimiter",
                    ..
                })
            ));
            assert!(matches!(
                unique(format!("{{{}\"a\": 1, \"a\": 2, 5: 1}}", head(pad))),
                UniqueError::Load(LoadError::Decode {
                    message: "Expecting property name enclosed in double quotes",
                    ..
                })
            ));
            assert!(matches!(
                unique(format!("{{{}\"a\": 1, \"a\": 2, \"b\": oops}}", head(pad))),
                UniqueError::Load(LoadError::Decode {
                    message: "Expecting value",
                    ..
                })
            ));
            // the object closed with its repeat: the repeat comes before any later error of the document
            assert_eq!(
                unique(format!("[{{{}\"k\": 1, \"k\": 2}}, oops", head(pad))),
                UniqueError::DuplicateKey(PyStr::new("k"))
            );
            assert_eq!(
                unique(format!("{{{}\"k\": 1, \"k\": 2}} trailing", head(pad))),
                UniqueError::DuplicateKey(PyStr::new("k"))
            );
            // an inner object closes first, whatever repeats later in the outer one
            assert_eq!(
                unique(format!(
                    "{{{}\"a\": 1, \"y\": {{{}\"k\": 1, \"k\": 2}}, \"a\": 3}}",
                    head(pad),
                    head(pad)
                )),
                UniqueError::DuplicateKey(PyStr::new("k"))
            );
            // a repeat in a later nested object is found when that object closes
            assert_eq!(
                unique(format!(
                    "{{{}\"x\": {{\"k\": 1}}, \"y\": {{{}\"m\": 1, \"m\": 2}}}}",
                    head(pad),
                    head(pad)
                )),
                UniqueError::DuplicateKey(PyStr::new("m"))
            );
            // an integer with too many digits, after the repeat, inside the same object: that error
            let long = "9".repeat(5000);
            assert!(matches!(
                unique(format!(
                    "{{{}\"a\": 1, \"a\": 2, \"n\": {long}}}",
                    head(pad)
                )),
                UniqueError::Load(LoadError::IntegerTooLong { digits: 5000 })
            ));
        }
    }

    #[test]
    fn a_nesting_too_deep_after_a_repeat_is_the_error_and_not_the_repeat() {
        // the reader recurses once per level: give the test the stack the command-line programs give their parser
        let outcome = std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn(|| {
                let deep = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
                let mut results = Vec::new();
                for pad in [0usize, 20] {
                    let head: String = (0..pad).map(|at| format!("\"pad{at}\": {at}, ")).collect();
                    let text = format!("{{{head}\"a\": 1, \"a\": 2, \"deep\": {deep}}}");
                    results.push(loads_text_unique(&PyStr::new(&text)).unwrap_err());
                    results.push(UniqueError::Load(
                        loads_text(&PyStr::new(&text)).unwrap_err(),
                    ));
                }
                results
            })
            .expect("a thread")
            .join()
            .expect("the reader does not overflow");
        for error in outcome {
            assert_eq!(
                error,
                UniqueError::Load(LoadError::TooDeep { container: "array" })
            );
        }
    }

    // ── strings: the unescaped span is copied once, everything else is scanned as before ───────

    /// The decoded code points of a document that is one JSON string, or the error.
    fn decode_string(text: &PyStr) -> Result<Vec<u32>, LoadError> {
        match loads_text(text) {
            Ok(PyValue::Str(decoded)) => Ok(decoded.code_points().to_vec()),
            Ok(other) => panic!("not a string: {}", other.repr()),
            Err(error) => Err(error),
        }
    }

    /// `"` + the code points written as they are (a lone surrogate or a character beyond the BMP stays one raw code point) + `"`.
    fn raw_quoted(points: &[u32]) -> PyStr {
        let mut all = vec![0x22];
        all.extend_from_slice(points);
        all.push(0x22);
        PyStr::from_code_points(all)
    }

    /// Code points at every boundary of the scanner's cases and of the encodings.
    const EDGE_POINTS: [u32; 24] = [
        0x20, 0x21, 0x23, 0x2F, 0x5B, 0x5D, 0x7E, 0x7F, 0x80, 0xFF, 0x7FF, 0x800, 0xFFFF, 0xD7FF,
        0xD800, 0xDBFF, 0xDC00, 0xDFFF, 0xE000, 0xFFFD, 0x10000, 0x1F600, 0x10FFFF, 0x41,
    ];

    #[test]
    fn strings_round_trip_for_every_kind_of_code_point_written_raw_or_escaped_at_every_position() {
        // raw: any code point above the control range except quote and backslash, as one raw code point (lone surrogates included)
        for &point in &EDGE_POINTS {
            for (before, after) in [(0usize, 0usize), (3, 0), (0, 3), (31, 31)] {
                let mut points = vec![0x61; before];
                points.push(point);
                points.extend(vec![0x7A; after]);
                assert_eq!(
                    decode_string(&raw_quoted(&points)).unwrap(),
                    points,
                    "raw {point:#x} {before}/{after}"
                );
            }
        }
        // escaped: every code point from the whole range of cases, written with `json_string` (controls, quote, backslash, surrogates, pairs)
        let mut escaped_cases: Vec<u32> = (0..0x20).collect();
        escaped_cases.extend([0x22, 0x5C]);
        escaped_cases.extend(EDGE_POINTS);
        for &point in &escaped_cases {
            for (before, after) in [(0usize, 0usize), (2, 0), (0, 2), (5, 7)] {
                let mut points = vec![0x61; before];
                points.push(point);
                points.extend(vec![0x7A; after]);
                let text = PyStr::new(&json_string(&points));
                // a surrogate half written as an escape is a lone surrogate unless it pairs with the next escape: check the pair rule by itself
                if (0xD800..0xE000).contains(&point) {
                    continue;
                }
                assert_eq!(
                    decode_string(&text).unwrap(),
                    points,
                    "escaped {point:#x} {before}/{after}"
                );
            }
        }
        // the empty string, plain strings of 1, 2, 31 and 5000 code points, and all the edge points in one string
        for length in [0usize, 1, 2, 31, 5000] {
            let points: Vec<u32> = (0..length).map(|at| 0x61 + (at as u32 % 26)).collect();
            assert_eq!(
                decode_string(&raw_quoted(&points)).unwrap(),
                points,
                "plain {length}"
            );
        }
        assert_eq!(
            decode_string(&raw_quoted(&EDGE_POINTS)).unwrap(),
            EDGE_POINTS.to_vec()
        );
        // the escape first, in the middle and last of an otherwise plain run
        for text in [
            r#""\n0123456789""#,
            r#""01234\t56789""#,
            r#""0123456789\\""#,
            r#""\"abc\"""#,
            r#""\/\b\f\r""#,
        ] {
            let expected: Vec<u32> = match text {
                r#""\n0123456789""# => points("\n0123456789"),
                r#""01234\t56789""# => points("01234\t56789"),
                r#""0123456789\\""# => points("0123456789\\"),
                r#""\"abc\"""# => points("\"abc\""),
                _ => points("/\u{8}\u{c}\r"),
            };
            assert_eq!(
                decode_string(&PyStr::new(text)).unwrap(),
                expected,
                "{text}"
            );
        }
    }

    #[test]
    fn surrogate_escapes_pair_only_with_a_following_low_escape_and_stay_lone_otherwise() {
        let decode = |text: &str| decode_string(&PyStr::new(text)).unwrap();
        assert_eq!(
            decode(r#""ab\ud83d\ude00cd""#),
            vec![0x61, 0x62, 0x1F600, 0x63, 0x64]
        );
        assert_eq!(
            decode(r#""ab\ud83dcd""#),
            vec![0x61, 0x62, 0xD83D, 0x63, 0x64]
        );
        assert_eq!(decode(r#""ab\ude00""#), vec![0x61, 0x62, 0xDE00]);
        assert_eq!(decode(r#""\ud83d\u0041""#), vec![0xD83D, 0x41]);
        assert_eq!(decode(r#""\ud83d\ud83d\ude00""#), vec![0xD83D, 0x1F600]);
        assert_eq!(
            decode(r#""x\u00e9y\u20acz""#),
            vec![0x78, 0xE9, 0x79, 0x20AC, 0x7A]
        );
    }

    #[test]
    fn string_errors_keep_their_messages_and_absolute_positions() {
        let error = |text: &str| match loads_text(&PyStr::new(text)).unwrap_err() {
            LoadError::Decode {
                message, position, ..
            } => (message, position),
            other => panic!("not a decode error: {other:?}"),
        };
        // unterminated: at the opening quote, whatever plain run or escape was read before the end
        assert_eq!(error("\"abc"), ("Unterminated string starting at", 0));
        assert_eq!(error("\""), ("Unterminated string starting at", 0));
        assert_eq!(error("[1, \"ab"), ("Unterminated string starting at", 4));
        assert_eq!(error("[\"ab\\n"), ("Unterminated string starting at", 1));
        assert_eq!(error("\"ab\\"), ("Unterminated string starting at", 0));
        // a control character, at its own position (the plain run before it is copied, the character is then the loop's)
        assert_eq!(error("\"ab\u{1}c\""), ("Invalid control character at", 3));
        assert_eq!(error("\"\u{1}\""), ("Invalid control character at", 1));
        assert_eq!(error("[\"abc\ndef\"]"), ("Invalid control character at", 5));
        assert_eq!(
            error("\"ab\\n\u{1f}\""),
            ("Invalid control character at", 5)
        );
        // an escape the format does not have, and the `\u` forms
        assert_eq!(error("\"ab\\x\""), ("Invalid \\escape", 3));
        assert_eq!(error("\"\\q\""), ("Invalid \\escape", 1));
        assert_eq!(error("\"\\u12\""), ("Invalid \\uXXXX escape", 2));
        assert_eq!(error("\"abcd\\u12G4\""), ("Invalid \\uXXXX escape", 6));
        // the error is at the `u` of the escape that is short of digits (index 8: the second escape begins at 7)
        assert_eq!(error("\"\\ud83d\\u12\""), ("Invalid \\uXXXX escape", 8));
        assert_eq!(error("\"\\u0041"), ("Invalid \\uXXXX escape", 2));
    }

    #[test]
    fn a_late_string_error_comes_before_an_earlier_repeat_in_the_unique_reader_on_both_branches() {
        for pad in [0usize, 20] {
            let head: String = (0..pad).map(|at| format!("\"pad{at}\": {at}, ")).collect();
            let unique = |text: String| loads_text_unique(&PyStr::new(&text)).unwrap_err();
            assert!(matches!(
                unique(format!("{{{head}\"a\": 1, \"a\": 2, \"b\": \"x\u{1}y\"}}")),
                UniqueError::Load(LoadError::Decode {
                    message: "Invalid control character at",
                    ..
                })
            ));
            assert!(matches!(
                unique(format!(
                    "{{{head}\"a\": 1, \"a\": 2, \"b\": \"unterminated}}"
                )),
                UniqueError::Load(LoadError::Decode {
                    message: "Unterminated string starting at",
                    ..
                })
            ));
            assert!(matches!(
                unique(format!("{{{head}\"a\": 1, \"a\": 2, \"b\\q\": 3}}")),
                UniqueError::Load(LoadError::Decode {
                    message: "Invalid \\escape",
                    ..
                })
            ));
        }
    }
}
