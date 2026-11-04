//! A string with Python `str` semantics: a sequence of code points that may include lone surrogates.
//!
//! The state files are parsed with Python's JSON rules, which accept `"\ud800"`, and Python decodes undecodable
//! stdin bytes into `U+DC80..U+DCFF` (`surrogateescape`). Rust `String` cannot hold such values, so text stays a
//! code-point vector until it is encoded for a stream.

use crate::unicode_tables::{DECIMAL_DIGITS, NON_PRINTABLE, PYTHON_WHITESPACE};
use std::fmt;

/// Text built by the program is `Owned` (and grows in place); text fixed in the source is `Static`: converted once
/// per use site (see [`lit!`]) into a buffer that lives for the whole run - one per literal in the source, never one
/// per use - and copied by reference. Both read the same; writing to static text copies it first.
#[derive(Clone, Debug)]
enum Repr {
    Owned(Vec<u32>),
    Static(&'static [u32]),
}

#[derive(Clone, Debug)]
pub struct PyStr(Repr);

impl Default for PyStr {
    fn default() -> Self {
        PyStr(Repr::Owned(Vec::new()))
    }
}

impl PartialEq for PyStr {
    fn eq(&self, other: &Self) -> bool {
        let (left, right) = (self.points(), other.points());
        std::ptr::eq(left, right) || left == right
    }
}

impl Eq for PyStr {}

/// A `PyStr` for text fixed in the source, converted once for this use site and a plain pointer copy afterwards.
macro_rules! lit {
    ($text:expr) => {{
        static CELL: std::sync::OnceLock<&'static [u32]> = std::sync::OnceLock::new();
        $crate::pystr::PyStr::from_static(
            *CELL.get_or_init(|| $crate::pystr::PyStr::static_points($text)),
        )
    }};
}
pub(crate) use lit;

fn in_ranges(table: &[(u32, u32)], code_point: u32) -> bool {
    table
        .binary_search_by(|&(first, last)| {
            if code_point < first {
                std::cmp::Ordering::Greater
            } else if code_point > last {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn is_surrogate(code_point: u32) -> bool {
    (0xD800..=0xDFFF).contains(&code_point)
}

/// Unicode decimal digit, which is what `\d` matches in a Python `str` pattern.
pub(crate) fn is_decimal_digit(code_point: u32) -> bool {
    in_ranges(DECIMAL_DIGITS, code_point)
}

/// `unicodedata.decimal(chr(code_point))`. Decimal digits come in aligned runs of ten starting at zero (checked
/// by the table generator), so the value is the offset inside the table range modulo ten.
pub(crate) fn decimal_digit_value(code_point: u32) -> Option<u32> {
    DECIMAL_DIGITS
        .iter()
        .find(|&&(first, last)| (first..=last).contains(&code_point))
        .map(|&(first, _)| (code_point - first) % 10)
}

/// `Py_UNICODE_ISSPACE`.
pub(crate) fn is_python_space(code_point: u32) -> bool {
    in_ranges(PYTHON_WHITESPACE, code_point)
}

impl PyStr {
    pub fn new(text: &str) -> Self {
        // one code point per character, so the byte length is an upper bound: no regrowth while filling
        let mut points = Vec::with_capacity(text.len());
        points.extend(text.chars().map(u32::from));
        PyStr(Repr::Owned(points))
    }

    /// The code points of text fixed in the source, in a buffer kept for the rest of the run. Called once per
    /// use site by [`lit!`]; not for text that varies.
    pub fn static_points(text: &str) -> &'static [u32] {
        Box::leak(
            text.chars()
                .map(u32::from)
                .collect::<Vec<u32>>()
                .into_boxed_slice(),
        )
    }

    pub fn from_static(points: &'static [u32]) -> Self {
        PyStr(Repr::Static(points))
    }

    pub fn from_code_points(points: Vec<u32>) -> Self {
        PyStr(Repr::Owned(points))
    }

    fn points(&self) -> &[u32] {
        match &self.0 {
            Repr::Owned(points) => points,
            Repr::Static(points) => points,
        }
    }

    /// The buffer for writing; shared text is copied first.
    fn owned(&mut self) -> &mut Vec<u32> {
        if let Repr::Static(points) = &self.0 {
            self.0 = Repr::Owned(points.to_vec());
        }
        match &mut self.0 {
            Repr::Owned(points) => points,
            Repr::Static(_) => unreachable!("made owned above"),
        }
    }

    /// `bytes.decode("utf-8", "surrogateescape")`: valid sequences decode normally and every byte of an
    /// invalid sequence becomes `U+DC00 + byte`.
    pub fn from_surrogateescape(mut bytes: &[u8]) -> Self {
        let mut points = Vec::with_capacity(bytes.len());
        loop {
            match std::str::from_utf8(bytes) {
                Ok(valid) => {
                    points.extend(valid.chars().map(u32::from));
                    return PyStr::from_code_points(points);
                }
                Err(error) => {
                    let (valid, rest) = bytes.split_at(error.valid_up_to());
                    points.extend(
                        std::str::from_utf8(valid)
                            .expect("prefix is valid")
                            .chars()
                            .map(u32::from),
                    );
                    let invalid = error.error_len().unwrap_or(rest.len());
                    points.extend(rest[..invalid].iter().map(|&byte| 0xDC00 + u32::from(byte)));
                    bytes = &rest[invalid..];
                }
            }
        }
    }

    pub fn code_points(&self) -> &[u32] {
        self.points()
    }

    pub fn is_empty(&self) -> bool {
        self.points().is_empty()
    }

    pub fn len(&self) -> usize {
        self.points().len()
    }

    pub fn push_str(&mut self, text: &str) {
        let points = self.owned();
        points.reserve(text.len());
        points.extend(text.chars().map(u32::from));
    }

    pub fn push_code_point(&mut self, code_point: u32) {
        self.owned().push(code_point);
    }

    pub fn push_py(&mut self, other: &PyStr) {
        self.owned().extend_from_slice(other.points());
    }

    /// Compares with a Rust string without allocating.
    pub fn equals(&self, text: &str) -> bool {
        let points = self.points();
        if text.is_ascii() {
            // one code point per byte: compare lengths first, then without decoding
            return points.len() == text.len()
                && points
                    .iter()
                    .zip(text.bytes())
                    .all(|(&point, byte)| point == u32::from(byte));
        }
        points.iter().copied().eq(text.chars().map(u32::from))
    }

    /// `None` when the string holds a lone surrogate, which has no UTF-8 form.
    pub fn to_utf8(&self) -> Option<String> {
        self.points()
            .iter()
            .map(|&point| char::from_u32(point))
            .collect()
    }

    pub fn ends_with_code_point(&self, code_point: u32) -> bool {
        self.points().last() == Some(&code_point)
    }

    /// True when `strip()` would leave nothing: empty or whitespace only.
    pub fn is_blank(&self) -> bool {
        self.points()
            .iter()
            .all(|&point| in_ranges(PYTHON_WHITESPACE, point))
    }

    /// Python `str.strip()` with no argument.
    pub fn strip(&self) -> PyStr {
        let is_space = |point: &&u32| in_ranges(PYTHON_WHITESPACE, **point);
        let start = self
            .points()
            .iter()
            .position(|p| !is_space(&p))
            .unwrap_or(self.points().len());
        let end = self
            .points()
            .iter()
            .rposition(|p| !is_space(&p))
            .map_or(start, |last| last + 1);
        PyStr::from_code_points(self.points()[start..end].to_vec())
    }

    /// Python `repr(str)`.
    pub fn repr(&self) -> String {
        let has_single = self.points().contains(&('\'' as u32));
        let has_double = self.points().contains(&('"' as u32));
        let quote = if has_single && !has_double { '"' } else { '\'' };
        let mut out = String::with_capacity(self.points().len() + 2);
        out.push(quote);
        for &point in self.points() {
            match point {
                0x5C => out.push_str("\\\\"),
                0x09 => out.push_str("\\t"),
                0x0A => out.push_str("\\n"),
                0x0D => out.push_str("\\r"),
                p if p == quote as u32 => {
                    out.push('\\');
                    out.push(quote);
                }
                p if p < 0x20 || p == 0x7F => out.push_str(&format!("\\x{p:02x}")),
                p if p < 0x7F => out.push(char::from(p as u8)),
                p if in_ranges(NON_PRINTABLE, p) => out.push_str(&escape_non_printable(p)),
                p => out.push(char::from_u32(p).expect("printable code points are scalar values")),
            }
        }
        out.push(quote);
        out
    }

    /// Appends this string as a JSON string literal, as `json.dumps` writes it.
    ///
    /// With `ensure_ascii` every non-ASCII code point is `\uXXXX`-escaped (lone surrogates included), which is
    /// the default of `json.dumps`. Without it the text is copied as is, lone surrogates included; whether it
    /// can be written to a stream is decided by the stream encoder.
    pub fn write_json(&self, out: &mut PyStr, ensure_ascii: bool) {
        out.push_code_point(0x22);
        for &point in self.points() {
            match point {
                0x22 => out.push_str("\\\""),
                0x5C => out.push_str("\\\\"),
                0x08 => out.push_str("\\b"),
                0x0C => out.push_str("\\f"),
                0x0A => out.push_str("\\n"),
                0x0D => out.push_str("\\r"),
                0x09 => out.push_str("\\t"),
                p if p < 0x20 => out.push_str(&format!("\\u{p:04x}")),
                p if ensure_ascii && !(0x20..0x7F).contains(&p) => push_ascii_escape(out, p),
                p => out.push_code_point(p),
            }
        }
        out.push_code_point(0x22);
    }

    /// `text.encode("utf-8", "surrogateescape")`: `U+DC80..U+DCFF` become the original bytes; any other lone
    /// surrogate fails like Python's `UnicodeEncodeError`.
    pub fn to_surrogateescape_bytes(&self) -> Result<Vec<u8>, EncodeFailure> {
        let mut bytes = Vec::with_capacity(self.points().len());
        let mut index = 0;
        while index < self.points().len() {
            let point = self.points()[index];
            if let Some(character) = char::from_u32(point) {
                let mut buffer = [0; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            } else if (0xDC80..=0xDCFF).contains(&point) {
                bytes.push((point - 0xDC00) as u8);
            } else {
                // CPython emits the leading escapable bytes of a surrogate run, then reports the rest of the run
                // (from the first character it cannot escape) as one error.
                let run_start = self.points()[..index]
                    .iter()
                    .rposition(|&p| !is_surrogate(p))
                    .map_or(0, |last| last + 1);
                let start = (run_start..index)
                    .find(|&k| !(0xDC80..=0xDCFF).contains(&self.points()[k]))
                    .unwrap_or(index);
                let end = index
                    + self.points()[index..]
                        .iter()
                        .take_while(|&&p| is_surrogate(p))
                        .count();
                return Err(EncodeFailure {
                    start,
                    end,
                    first: self.points()[start],
                });
            }
            index += 1;
        }
        Ok(bytes)
    }

    /// `text.encode("utf-8", "backslashreplace")`, which is how Python writes to `sys.stderr`.
    pub fn to_backslashreplace_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.points().len());
        for &point in self.points() {
            match char::from_u32(point) {
                Some(character) => {
                    let mut buffer = [0; 4];
                    bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                }
                None => bytes.extend_from_slice(format!("\\u{point:04x}").as_bytes()),
            }
        }
        bytes
    }
}

fn push_ascii_escape(out: &mut PyStr, point: u32) {
    if point >= 0x10000 {
        let offset = point - 0x10000;
        out.push_str(&format!(
            "\\u{:04x}\\u{:04x}",
            0xD800 + (offset >> 10),
            0xDC00 + (offset & 0x3FF)
        ));
    } else {
        out.push_str(&format!("\\u{point:04x}"));
    }
}

fn escape_non_printable(point: u32) -> String {
    if point < 0x100 {
        format!("\\x{point:02x}")
    } else if point < 0x10000 {
        format!("\\u{point:04x}")
    } else {
        format!("\\U{point:08x}")
    }
}

/// A run of surrogates that cannot be written to a UTF-8 stream (`UnicodeEncodeError`).
///
/// `start..end` are indexes into the text being encoded, as in Python's message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodeFailure {
    pub start: usize,
    pub end: usize,
    pub first: u32,
}

impl EncodeFailure {
    /// `str(exception)`.
    pub fn message(&self) -> String {
        if self.end - self.start == 1 {
            format!(
                "'utf-8' codec can't encode character '\\u{:04x}' in position {}: surrogates not allowed",
                self.first, self.start
            )
        } else {
            format!(
                "'utf-8' codec can't encode characters in position {}-{}: surrogates not allowed",
                self.start,
                self.end - 1
            )
        }
    }
}

impl fmt::Display for EncodeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnicodeEncodeError: {}", self.message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_text_reads_like_owned_text_and_is_copied_before_it_is_written() {
        let fixed = lit!("값 abc");
        let owned = PyStr::new("값 abc");
        assert_eq!(fixed, owned);
        assert_eq!(owned, fixed);
        assert!(fixed.equals("값 abc") && !fixed.equals("값 abd") && !fixed.equals("값 ab"));
        assert!(PyStr::new("abc").equals("abc") && !PyStr::new("abc").equals("abcd"));
        // writing to a copy never changes the literal other users of the same site get
        let mut copy = fixed.clone();
        copy.push_str("!");
        copy.push_code_point(0xD800);
        assert_eq!(fixed.len(), 5);
        assert_eq!(copy.len(), 7);
        assert_eq!(lit!("값 abc").len(), 5);
        assert!(copy.to_utf8().is_none());
        assert_eq!(fixed.repr(), "'값 abc'");
    }

    #[test]
    fn repr_picks_quotes_like_python() {
        assert_eq!(PyStr::new("abc").repr(), "'abc'");
        assert_eq!(PyStr::new("it's").repr(), "\"it's\"");
        assert_eq!(PyStr::new("a'b\"c").repr(), "'a\\'b\"c'");
        assert_eq!(PyStr::new("키😀").repr(), "'키😀'");
        assert_eq!(PyStr::new("\u{200b}\u{7f}\n").repr(), "'\\u200b\\x7f\\n'");
        assert_eq!(
            PyStr::from_code_points(vec![0xD800, 0xFF600]).repr(),
            "'\\ud800\\U000ff600'"
        );
    }

    #[test]
    fn strip_uses_python_whitespace() {
        assert_eq!(
            PyStr::new("\u{1c} \u{a0}x\u{3000}\t").strip(),
            PyStr::new("x")
        );
        assert!(
            PyStr::new("\u{200b}").strip().len() == 1,
            "U+200B is not whitespace in Python"
        );
        assert!(PyStr::new(" \u{85}\u{2028} ").strip().is_empty());
    }

    #[test]
    fn json_escapes_control_characters_in_lowercase() {
        let mut out = PyStr::default();
        PyStr::new("a\"\\\u{1}\u{7f}\u{2028}").write_json(&mut out, false);
        assert_eq!(out.to_utf8().unwrap(), "\"a\\\"\\\\\\u0001\u{7f}\u{2028}\"");
        let mut ascii = PyStr::default();
        PyStr::from_code_points(vec![0x61, 0xE9, 0x1F600, 0xD800, 0x7F])
            .write_json(&mut ascii, true);
        assert_eq!(
            ascii.to_utf8().unwrap(),
            "\"a\\u00e9\\ud83d\\ude00\\ud800\\u007f\""
        );
    }

    #[test]
    fn surrogateescape_round_trips_invalid_bytes_and_keeps_valid_text() {
        let decoded =
            PyStr::from_surrogateescape(b"a\xffb\xe1\x80Z\xf0\x90\xed\xa0\x80\xea\xb0\x80");
        assert_eq!(
            decoded.code_points(),
            &[
                0x61, 0xDCFF, 0x62, 0xDCE1, 0xDC80, 0x5A, 0xDCF0, 0xDC90, 0xDCED, 0xDCA0, 0xDC80,
                0xAC00
            ]
        );
        assert_eq!(
            decoded.to_surrogateescape_bytes().unwrap(),
            b"a\xffb\xe1\x80Z\xf0\x90\xed\xa0\x80\xea\xb0\x80"
        );
    }

    #[test]
    fn other_lone_surrogates_fail_as_a_run_like_python() {
        let failure = PyStr::from_code_points(vec![0x61, 0xDCFF, 0xD800, 0xD801, 0x62])
            .to_surrogateescape_bytes()
            .unwrap_err();
        assert_eq!((failure.start, failure.end, failure.first), (2, 4, 0xD800));
        assert_eq!(
            failure.to_string(),
            "UnicodeEncodeError: 'utf-8' codec can't encode characters in position 2-3: surrogates not allowed"
        );
        let single = PyStr::from_code_points(vec![0x61, 0xD800])
            .to_surrogateescape_bytes()
            .unwrap_err();
        assert_eq!(
            single.to_string(),
            "UnicodeEncodeError: 'utf-8' codec can't encode character '\\ud800' in position 1: surrogates not allowed"
        );
        assert_eq!(
            PyStr::from_code_points(vec![0x61, 0xDCFF, 0xD800]).to_backslashreplace_bytes(),
            b"a\\udcff\\ud800"
        );
    }
}
