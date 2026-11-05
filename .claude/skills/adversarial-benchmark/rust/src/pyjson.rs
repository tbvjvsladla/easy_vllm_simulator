//! A JSON reader that behaves like Python's `json.loads`, and the text decoders that feed it.
//!
//! The state files are written and read by Python, so the reader accepts what `json` accepts (`NaN`,
//! `Infinity`, lone surrogate escapes, arbitrary-size integers) and reports failures with the same messages and
//! character positions as `json.JSONDecodeError`. It follows the structure of CPython 3.12.3
//! `Modules/_json.c` (scanner) and `Lib/json/decoder.py`; see `notices/NOTICE.md`.

use crate::pystr::PyStr;
use crate::pyvalue::{json_nan_constant, PyDict, PyValue};
use num_bigint::BigInt;
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

/// `str(UnicodeDecodeError)` of `bytes.decode("utf-8")` for the failure `from_utf8` reported: the offending byte(s)
/// named by position and the reason Python gives.
pub fn utf8_error_text(bytes: &[u8], error: &std::str::Utf8Error) -> String {
    let start = error.valid_up_to();
    let (length, reason) = match error.error_len() {
        None => (bytes.len() - start, "unexpected end of data"),
        Some(length) if (0xC2..=0xF4).contains(&bytes[start]) => {
            (length, "invalid continuation byte")
        }
        Some(length) => (length, "invalid start byte"),
    };
    let subject = if length == 1 {
        format!("byte 0x{:02x} in position {start}", bytes[start])
    } else {
        format!("bytes in position {start}-{}", start + length - 1)
    };
    format!("'utf-8' codec can't decode {subject}: {reason}")
}

/// `sys.stdin.read()` on this interpreter: UTF-8 with `surrogateescape` and universal newlines. Undecodable bytes
/// become `U+DC80..U+DCFF`, so they survive to the output unchanged.
pub fn decode_stdin_text(bytes: &[u8]) -> PyStr {
    translate_newlines(PyStr::from_surrogateescape(bytes).code_points().to_vec())
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
    let reader = Reader {
        points: text.code_points(),
        nan_objects,
    };
    if reader.at(0) == Some(0xFEFF) {
        return Err(reader.decode_error("Unexpected UTF-8 BOM (decode using utf-8-sig)", 0));
    }
    let start = reader.skip_whitespace(0);
    let (value, end) = reader
        .value(start, 0)
        .map_err(|failure| reader.finish(failure))?;
    let end = reader.skip_whitespace(end);
    if end != reader.points.len() {
        return Err(reader.decode_error("Extra data", end));
    }
    Ok(value)
}

enum Failure {
    /// Python's `StopIteration(idx)`: no value starts at `idx`.
    NoValue(usize),
    Decode(&'static str, usize),
    Other(LoadError),
}

struct Reader<'a> {
    points: &'a [u32],
    /// Whether `NaN` decodes to the shared object (see [`loads_objects`]).
    nan_objects: bool,
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

    fn finish(&self, failure: Failure) -> LoadError {
        match failure {
            Failure::NoValue(position) => self.decode_error("Expecting value", position),
            Failure::Decode(message, position) => self.decode_error(message, position),
            Failure::Other(error) => error,
        }
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
        let mut points: Vec<u32> = Vec::new();
        let mut index = start;
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
        let mut map = PyDict::new();
        let mut index = self.skip_whitespace(start);
        if self.is_at(index, '}') {
            return Ok((PyValue::Dict(map), index + 1));
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
            map.insert(key, value);
            index = self.skip_whitespace(after_value);
            if self.is_at(index, '}') {
                return Ok((PyValue::Dict(map), index + 1));
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
    fn stdin_decoding_escapes_bad_bytes_and_translates_newlines() {
        let text = decode_stdin_text(b"a\r\nb\rc\xff");
        assert_eq!(text.code_points(), &[0x61, 0x0A, 0x62, 0x0A, 0x63, 0xDCFF]);
        assert_eq!(
            decode_file_text(b"x\r\ny").unwrap().code_points(),
            &[0x78, 0x0A, 0x79]
        );
    }
}
