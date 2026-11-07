//! `json.load(response)` of `fetch_json` for a response body that is raw bytes: CPython 3.12.3 `json.loads(bytes)`.
//!
//! `loads` of a `bytes` argument picks the codec with `json.detect_encoding` (UTF-32 and UTF-16 byte-order marks, the UTF-8 mark, then the pattern
//! of zero bytes in the first four), decodes with the `surrogatepass` handler and hands the text to the scanner. A byte-order mark that is still
//! in the decoded text is NOT rejected: that check belongs to the `str` branch of `loads` only. The scanner and its `JSONDecodeError` positions
//! are the shared [`agent_runtime_support::pyjson`] reader; this module adds the byte decoding, the exception objects and the stdlib site each
//! failure was raised from (the frames a traceback names).
//!
//! The decoders reproduce the three failure reasons of `utf-8` and the truncation and range reasons of the UTF-16/UTF-32 decoders. Only the
//! UTF-8 failures (invalid start byte) were captured against the original; the UTF-16 and UTF-32 failure texts are derived from the CPython
//! decoders' rules and have no saved original (see `ABI.md`, scope limits).

use crate::http_direct::failure::ErrorValue;
use crate::http_direct::PyException;
use agent_runtime_support::pyjson::{loads_text, utf8_error_parts, LoadError};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

/// Where in the interpreter's `json` package a failure was raised, which decides the frames an uncaught report shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonSite {
    /// `s = s.decode(detect_encoding(s), 'surrogatepass')` of `loads` (the codec raised).
    BytesDecode,
    /// `raise JSONDecodeError("Expecting value", s, err.value) from None` of `raw_decode`.
    ExpectingValue,
    /// An exception the C scanner itself raised inside `self.scan_once(s, idx)` of `raw_decode`.
    Scanner,
    /// `raise JSONDecodeError("Extra data", s, end)` of `decode`.
    ExtraData,
    /// Raised by the scanner for a nesting the interpreter's recursion limit refuses.
    Recursion,
}

/// A failure of `json.load` with the site it came from and whether the interpreter marked its context suppressed (`raise ... from None`).
#[derive(Clone, Debug)]
pub struct JsonFailure {
    pub error: Box<PyException>,
    pub site: JsonSite,
    pub from_none: bool,
}

/// `json.detect_encoding(b)`.
pub fn detect_encoding(bytes: &[u8]) -> &'static str {
    const UTF32_BE_BOM: [u8; 4] = [0, 0, 0xFE, 0xFF];
    const UTF32_LE_BOM: [u8; 4] = [0xFF, 0xFE, 0, 0];
    if bytes.starts_with(&UTF32_BE_BOM) || bytes.starts_with(&UTF32_LE_BOM) {
        return "utf-32";
    }
    if bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE]) {
        return "utf-16";
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return "utf-8-sig";
    }
    if bytes.len() >= 4 {
        if bytes[0] == 0 {
            return if bytes[1] != 0 {
                "utf-16-be"
            } else {
                "utf-32-be"
            };
        }
        if bytes[1] == 0 {
            return if bytes[2] != 0 || bytes[3] != 0 {
                "utf-16-le"
            } else {
                "utf-32-le"
            };
        }
    } else if bytes.len() == 2 {
        if bytes[0] == 0 {
            return "utf-16-be";
        }
        if bytes[1] == 0 {
            return "utf-16-le";
        }
    }
    "utf-8"
}

/// What a codec reports when it cannot decode: the fields of `UnicodeDecodeError(encoding, object, start, end, reason)`.
struct UndecodableRun {
    start: usize,
    end: usize,
    reason: &'static str,
}

/// The codec names `json.detect_encoding` can return, as the `encoding` of a `UnicodeDecodeError` (`utf-8-sig` reports its inner `utf-8`).
const CODEC_NAMES: [&str; 7] = [
    "utf-8",
    "utf-16",
    "utf-16-le",
    "utf-16-be",
    "utf-32",
    "utf-32-le",
    "utf-32-be",
];

/// `UnicodeDecodeError` as the `bytes.decode(codec)` call raises it; `str()` follows `UnicodeDecodeError.__str__`.
pub fn unicode_decode_error(
    encoding: &'static str,
    object: &[u8],
    start: usize,
    end: usize,
    reason: &'static str,
) -> PyException {
    debug_assert!(CODEC_NAMES.contains(&encoding));
    let subject = if end - start == 1 {
        format!("byte 0x{:02x} in position {start}", object[start])
    } else {
        format!("bytes in position {start}-{}", end - 1)
    };
    let message = format!("'{encoding}' codec can't decode {subject}: {reason}");
    PyException {
        class: "UnicodeDecodeError",
        module: "builtins",
        message: PyStr::new(&message),
        args: vec![
            PyValue::str(encoding).into(),
            PyValue::Bytes(object.to_vec()).into(),
            PyValue::int(start).into(),
            PyValue::int(end).into(),
            PyValue::str(reason).into(),
        ],
        attributes: vec![
            ("reason", PyValue::str(reason).into()),
            ("encoding", PyValue::str(encoding).into()),
            ("object", PyValue::Bytes(object.to_vec()).into()),
            ("start", PyValue::int(start).into()),
            ("end", PyValue::int(end).into()),
        ],
        http_headers: None,
        link: None,
    }
}

/// `bytes.decode("utf-8", "surrogatepass")`: strict UTF-8 plus the three-byte forms of the surrogates U+D800..U+DFFF.
fn decode_utf8_surrogatepass(bytes: &[u8]) -> Result<Vec<u32>, UndecodableRun> {
    let mut points: Vec<u32> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match std::str::from_utf8(&bytes[at..]) {
            Ok(rest) => {
                points.extend(rest.chars().map(u32::from));
                return Ok(points);
            }
            Err(error) => {
                let valid_end = at + error.valid_up_to();
                points.extend(
                    std::str::from_utf8(&bytes[at..valid_end])
                        .expect("valid prefix")
                        .chars()
                        .map(u32::from),
                );
                if let [0xED, second @ 0xA0..=0xBF, third @ 0x80..=0xBF, ..] = bytes[valid_end..] {
                    points.push(0xD000 | (u32::from(second & 0x3F) << 6) | u32::from(third & 0x3F));
                    at = valid_end + 3;
                    continue;
                }
                let (start, length, reason) = utf8_error_parts(&bytes[at..], &error);
                return Err(UndecodableRun {
                    start: at + start,
                    end: at + start + length,
                    reason,
                });
            }
        }
    }
    Ok(points)
}

/// Code units of a UTF-16 stream with `surrogatepass`: a lone surrogate unit is kept, a high unit followed by a low one is combined, a single
/// trailing byte is "truncated data".
fn decode_utf16_surrogatepass(
    bytes: &[u8],
    little_endian: bool,
) -> Result<Vec<u32>, UndecodableRun> {
    let unit = |at: usize| -> u32 {
        let pair = [bytes[at], bytes[at + 1]];
        u32::from(if little_endian {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        })
    };
    let mut points = Vec::with_capacity(bytes.len() / 2);
    let mut at = 0;
    while at < bytes.len() {
        if bytes.len() - at < 2 {
            return Err(UndecodableRun {
                start: at,
                end: bytes.len(),
                reason: "truncated data",
            });
        }
        let first = unit(at);
        at += 2;
        if (0xD800..0xDC00).contains(&first) && bytes.len() - at >= 2 {
            let second = unit(at);
            if (0xDC00..0xE000).contains(&second) {
                points.push(0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00));
                at += 2;
                continue;
            }
        }
        points.push(first);
    }
    Ok(points)
}

/// Code units of a UTF-32 stream with `surrogatepass`: surrogate values are kept, values above U+10FFFF and a short tail are errors.
fn decode_utf32_surrogatepass(
    bytes: &[u8],
    little_endian: bool,
) -> Result<Vec<u32>, UndecodableRun> {
    let mut points = Vec::with_capacity(bytes.len() / 4);
    let mut at = 0;
    while at < bytes.len() {
        if bytes.len() - at < 4 {
            return Err(UndecodableRun {
                start: at,
                end: bytes.len(),
                reason: "truncated data",
            });
        }
        let quad = [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];
        let value = if little_endian {
            u32::from_le_bytes(quad)
        } else {
            u32::from_be_bytes(quad)
        };
        if value > 0x10FFFF {
            return Err(UndecodableRun {
                start: at,
                end: at + 4,
                reason: "code point not in range(0x110000)",
            });
        }
        points.push(value);
        at += 4;
    }
    Ok(points)
}

/// The byte order a `utf-16` / `utf-32` stream starts with, and how many bytes the mark takes; no mark means the host's order (little endian on
/// the supported Linux targets).
fn byte_order_mark(bytes: &[u8], unit_width: usize) -> (bool, usize) {
    match (unit_width, bytes) {
        (2, [0xFF, 0xFE, ..]) => (true, 2),
        (2, [0xFE, 0xFF, ..]) => (false, 2),
        (4, [0xFF, 0xFE, 0, 0, ..]) => (true, 4),
        (4, [0, 0, 0xFE, 0xFF, ..]) => (false, 4),
        _ => (true, 0),
    }
}

/// `bytes.decode(encoding, "surrogatepass")` for the codecs `detect_encoding` returns.
pub fn decode_with_codec(bytes: &[u8], encoding: &'static str) -> Result<PyStr, Box<PyException>> {
    let fail = |run: UndecodableRun, object: &[u8], name: &'static str| {
        Box::new(unicode_decode_error(
            name, object, run.start, run.end, run.reason,
        ))
    };
    let decoded = match encoding {
        "utf-8" => decode_utf8_surrogatepass(bytes).map_err(|run| fail(run, bytes, "utf-8")),
        "utf-8-sig" => {
            // the codec drops the mark first, so the error object and positions belong to what follows it
            let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
            decode_utf8_surrogatepass(body).map_err(|run| fail(run, body, "utf-8"))
        }
        "utf-16" | "utf-32" => {
            let width = if encoding == "utf-16" { 2 } else { 4 };
            let (little, mark) = byte_order_mark(bytes, width);
            let body = &bytes[mark..];
            let result = if width == 2 {
                decode_utf16_surrogatepass(body, little)
            } else {
                decode_utf32_surrogatepass(body, little)
            };
            // positions are those of the whole input, the mark included
            result.map_err(|run| {
                fail(
                    UndecodableRun {
                        start: run.start + mark,
                        end: run.end + mark,
                        reason: run.reason,
                    },
                    bytes,
                    encoding,
                )
            })
        }
        "utf-16-le" | "utf-16-be" => decode_utf16_surrogatepass(bytes, encoding == "utf-16-le")
            .map_err(|run| fail(run, bytes, encoding)),
        _ => decode_utf32_surrogatepass(bytes, encoding == "utf-32-le")
            .map_err(|run| fail(run, bytes, encoding)),
    };
    decoded.map(PyStr::from_code_points)
}

/// `JSONDecodeError(msg, doc, pos)` with the `lineno` and `colno` the constructor derives from them.
pub fn json_decode_error(
    msg: &str,
    doc: &PyStr,
    position: usize,
    line: usize,
    column: usize,
) -> PyException {
    let message = format!("{msg}: line {line} column {column} (char {position})");
    PyException {
        class: "JSONDecodeError",
        module: "json.decoder",
        message: PyStr::new(&message),
        args: vec![PyValue::str(&message).into()],
        attributes: vec![
            ("msg", PyValue::str(msg).into()),
            ("doc", PyValue::Str(doc.clone()).into()),
            ("pos", PyValue::int(position).into()),
            ("lineno", PyValue::int(line).into()),
            ("colno", PyValue::int(column).into()),
        ],
        http_headers: None,
        link: None,
    }
}

fn plain_error(class: &'static str, message: &str) -> PyException {
    PyException {
        class,
        module: "builtins",
        message: PyStr::new(message),
        args: vec![ErrorValue::Plain(PyValue::str(message))],
        attributes: Vec::new(),
        http_headers: None,
        link: None,
    }
}

/// The failure of the scanner on `document`, as the interpreter raises it.
fn scan_failure(error: LoadError, document: &PyStr) -> JsonFailure {
    match error {
        LoadError::Decode { message, line, column, position } => {
            let site = match message {
                "Expecting value" => JsonSite::ExpectingValue,
                "Extra data" => JsonSite::ExtraData,
                _ => JsonSite::Scanner,
            };
            JsonFailure { error: Box::new(json_decode_error(message, document, position, line, column)), site, from_none: message == "Expecting value" }
        }
        LoadError::IntegerTooLong { .. } => JsonFailure { error: Box::new(plain_error("ValueError", &error.to_string())), site: JsonSite::Scanner, from_none: false },
        LoadError::Encoding(message) => JsonFailure { error: Box::new(plain_error("ValueError", &message)), site: JsonSite::Scanner, from_none: false },
        LoadError::TooDeep { container } => JsonFailure {
            error: Box::new(plain_error(
                "RecursionError",
                &format!("maximum recursion depth exceeded while decoding a JSON {container} from a unicode string"),
            )),
            site: JsonSite::Recursion,
            from_none: false,
        },
    }
}

/// `json.loads(body)` for a `bytes` body.
pub fn loads_bytes(body: &[u8]) -> Result<PyValue, JsonFailure> {
    let text = decode_with_codec(body, detect_encoding(body)).map_err(|error| JsonFailure {
        error,
        site: JsonSite::BytesDecode,
        from_none: false,
    })?;
    // The shared reader applies the `str` branch's mark check; a decoded `bytes` body skips it, so a leading U+FEFF reaches the scanner, which
    // cannot start a value with it and is not whitespace: "Expecting value" at position 0.
    if text.code_points().first() == Some(&0xFEFF) {
        return Err(scan_failure(
            LoadError::Decode {
                message: "Expecting value",
                line: 1,
                column: 1,
                position: 0,
            },
            &text,
        ));
    }
    loads_text(&text).map_err(|error| scan_failure(error, &text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(bytes: &[u8]) -> String {
        match loads_bytes(bytes) {
            Ok(value) => value.repr(),
            Err(failure) => format!(
                "{}: {}",
                failure.error.class,
                failure.error.message.to_utf8().unwrap()
            ),
        }
    }

    #[test]
    fn the_codec_follows_the_marks_and_the_zero_pattern_of_the_first_four_bytes() {
        let cases: [(&[u8], &str); 12] = [
            (b"\xff\xfe\x00\x00{\x00\x00\x00", "utf-32"),
            (b"\x00\x00\xfe\xff\x00\x00\x00{", "utf-32"),
            (b"\xff\xfe{\x00", "utf-16"),
            (b"\xfe\xff\x00{", "utf-16"),
            (b"\xef\xbb\xbf{}", "utf-8-sig"),
            (b"\x00{\x00\"", "utf-16-be"),
            (b"\x00\x00\x00{", "utf-32-be"),
            (b"{\x00\"\x00", "utf-16-le"),
            (b"{\x00\x00\x00", "utf-32-le"),
            (b"\x00{", "utf-16-be"),
            (b"{\x00", "utf-16-le"),
            (b"{}", "utf-8"),
        ];
        for (bytes, expected) in cases {
            assert_eq!(detect_encoding(bytes), expected, "{bytes:?}");
        }
    }

    #[test]
    fn every_codec_reads_the_same_object() {
        let utf8 = b"{\"x\":1}";
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(utf8);
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(utf8.iter().flat_map(|byte| [*byte, 0]))
            .collect();
        let utf16_be_bom: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain(utf8.iter().flat_map(|byte| [0, *byte]))
            .collect();
        let utf16_le: Vec<u8> = utf8.iter().flat_map(|byte| [*byte, 0]).collect();
        let utf32_le_bom: Vec<u8> = [0xFF, 0xFE, 0, 0]
            .into_iter()
            .chain(utf8.iter().flat_map(|byte| [*byte, 0, 0, 0]))
            .collect();
        let utf32_be: Vec<u8> = utf8.iter().flat_map(|byte| [0, 0, 0, *byte]).collect();
        let expected = text_of(utf8);
        for bytes in [
            with_bom,
            utf16,
            utf16_be_bom,
            utf16_le,
            utf32_le_bom,
            utf32_be,
        ] {
            assert_eq!(text_of(&bytes), expected);
        }
    }

    #[test]
    fn an_encoded_surrogate_is_kept_and_a_bad_byte_reports_the_utf8_run() {
        let kept =
            loads_bytes(b"{\"x\":\"\xed\xa0\x80\"}").expect("surrogatepass keeps the surrogate");
        let PyValue::Dict(members) = kept else {
            panic!("an object")
        };
        assert_eq!(
            members.get("x"),
            Some(&PyValue::Str(PyStr::from_code_points(vec![0xD800])))
        );
        let failure = loads_bytes(b"\xff").unwrap_err();
        assert_eq!(failure.site, JsonSite::BytesDecode);
        assert_eq!(
            failure.error.message.to_utf8().unwrap(),
            "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"
        );
        assert_eq!(text_of(b"\xe1\x80A"), "UnicodeDecodeError: 'utf-8' codec can't decode bytes in position 0-1: invalid continuation byte");
        assert_eq!(text_of(b"{\"x\":\"\xe1\x80"), "UnicodeDecodeError: 'utf-8' codec can't decode bytes in position 6-7: unexpected end of data");
    }

    #[test]
    fn the_mark_of_a_decoded_text_is_not_rejected_and_the_scanner_reports_the_value_it_expected() {
        // `utf-8-sig` drops one mark; a second one is ordinary text the scanner cannot start a value with
        let failure = loads_bytes(b"\xef\xbb\xbf\xef\xbb\xbf{}").unwrap_err();
        assert_eq!(failure.site, JsonSite::ExpectingValue);
        assert!(failure.from_none);
        assert_eq!(
            failure.error.message.to_utf8().unwrap(),
            "Expecting value: line 1 column 1 (char 0)"
        );
    }

    #[test]
    fn scanner_failures_keep_their_constructor_fields_and_the_from_none_mark_of_expecting_value() {
        let empty = loads_bytes(b"").unwrap_err();
        assert_eq!(
            empty.error.message.to_utf8().unwrap(),
            "Expecting value: line 1 column 1 (char 0)"
        );
        assert!(empty.from_none);
        let truncated = loads_bytes(b"{\"assets\":[").unwrap_err();
        assert_eq!(
            truncated.error.message.to_utf8().unwrap(),
            "Expecting value: line 1 column 12 (char 11)"
        );
        let extra = loads_bytes(b"{} x").unwrap_err();
        assert_eq!((extra.site, extra.from_none), (JsonSite::ExtraData, false));
        let delimiter = loads_bytes(b"{\"a\" 1}").unwrap_err();
        assert_eq!(
            (delimiter.site, delimiter.from_none),
            (JsonSite::Scanner, false)
        );
        assert_eq!(
            delimiter.error.message.to_utf8().unwrap(),
            "Expecting ':' delimiter: line 1 column 6 (char 5)"
        );
    }

    #[test]
    fn utf16_and_utf32_failures_use_the_positions_of_the_whole_input() {
        assert_eq!(text_of(b"\xff\xfe{\x00\"\x00\x00"), "UnicodeDecodeError: 'utf-16' codec can't decode byte 0x00 in position 6: truncated data");
        assert_eq!(text_of(b"\x00\x00\x00{\x00\x11\x00\x00"), "UnicodeDecodeError: 'utf-32-be' codec can't decode bytes in position 4-7: code point not in range(0x110000)");
        assert_eq!(text_of(b"\x00\x00\x00{\x00\x00"), "UnicodeDecodeError: 'utf-32-be' codec can't decode bytes in position 4-5: truncated data");
    }
}
