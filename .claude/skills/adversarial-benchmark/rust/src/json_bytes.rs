//! `json.loads(bytes)` / `json.load(binary_file)`: JSON from raw bytes, decoded the way CPython 3.12 does it.
//!
//! A file opened as text is strict UTF-8 ([`crate::pyjson::decode_file_text`]). An HTTP response is read as bytes, and
//! `json.loads` then guesses the encoding from the first bytes (`json.detect_encoding`) and decodes with the
//! `surrogatepass` error handler - so a byte-order mark, UTF-16 and UTF-32 bodies are accepted, a lone surrogate
//! survives into the text, and everything else that cannot be decoded is a `UnicodeDecodeError` with CPython's text.
//! The decoded text is then parsed by the same decoder as every other JSON text of this crate.

use crate::pyjson::{loads_text_objects, LoadError};
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;

const BOM_UTF8: [u8; 3] = [0xEF, 0xBB, 0xBF];
const BOM_UTF16_LE: [u8; 2] = [0xFF, 0xFE];
const BOM_UTF16_BE: [u8; 2] = [0xFE, 0xFF];
const BOM_UTF32_LE: [u8; 4] = [0xFF, 0xFE, 0x00, 0x00];
const BOM_UTF32_BE: [u8; 4] = [0x00, 0x00, 0xFE, 0xFF];

/// The encodings `json.detect_encoding` can answer. `*WithMark` are the `utf-16`/`utf-32` codecs, which read the byte
/// order from a byte-order mark they then drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    Utf8,
    Utf8Signature,
    Utf16WithMark,
    Utf16LittleEndian,
    Utf16BigEndian,
    Utf32WithMark,
    Utf32LittleEndian,
    Utf32BigEndian,
}

/// `json.detect_encoding(b)`.
fn detect_encoding(bytes: &[u8]) -> Encoding {
    if bytes.starts_with(&BOM_UTF32_BE) || bytes.starts_with(&BOM_UTF32_LE) {
        return Encoding::Utf32WithMark;
    }
    if bytes.starts_with(&BOM_UTF16_BE) || bytes.starts_with(&BOM_UTF16_LE) {
        return Encoding::Utf16WithMark;
    }
    if bytes.starts_with(&BOM_UTF8) {
        return Encoding::Utf8Signature;
    }
    match bytes {
        [first, second, third, fourth, ..] => {
            if *first == 0 {
                return if *second != 0 {
                    Encoding::Utf16BigEndian
                } else {
                    Encoding::Utf32BigEndian
                };
            }
            if *second == 0 {
                return if *third != 0 || *fourth != 0 {
                    Encoding::Utf16LittleEndian
                } else {
                    Encoding::Utf32LittleEndian
                };
            }
            Encoding::Utf8
        }
        [0, _] => Encoding::Utf16BigEndian,
        [_, 0] => Encoding::Utf16LittleEndian,
        _ => Encoding::Utf8,
    }
}

/// `json.loads(data)` for `bytes`: detect the encoding, decode with `surrogatepass`, parse. Failures are
/// [`LoadError::Encoding`] (the `UnicodeDecodeError` text) or the parse errors of the JSON decoder.
pub fn loads_bytes(bytes: &[u8]) -> Result<PyValue, LoadError> {
    let text = decode_bytes(bytes).map_err(LoadError::Encoding)?;
    loads_text_objects(&text)
}

/// `data.decode(detect_encoding(data), "surrogatepass")`.
pub fn decode_bytes(bytes: &[u8]) -> Result<PyStr, String> {
    let points = match detect_encoding(bytes) {
        Encoding::Utf8 => decode_utf8(bytes)?,
        Encoding::Utf8Signature => decode_utf8(&bytes[BOM_UTF8.len()..])?,
        Encoding::Utf16WithMark => {
            let big_endian = bytes.starts_with(&BOM_UTF16_BE);
            decode_utf16(bytes, BOM_UTF16_LE.len(), big_endian)?
        }
        Encoding::Utf16LittleEndian => decode_utf16(bytes, 0, false)?,
        Encoding::Utf16BigEndian => decode_utf16(bytes, 0, true)?,
        Encoding::Utf32WithMark => {
            let big_endian = bytes.starts_with(&BOM_UTF32_BE);
            decode_utf32(bytes, BOM_UTF32_LE.len(), big_endian)?
        }
        Encoding::Utf32LittleEndian => decode_utf32(bytes, 0, false)?,
        Encoding::Utf32BigEndian => decode_utf32(bytes, 0, true)?,
    };
    Ok(PyStr::from_code_points(points))
}

/// `str(UnicodeDecodeError)`: one byte is named, a longer range is given by its first and last position.
fn decode_failure(codec: &str, bytes: &[u8], start: usize, end: usize, reason: &str) -> String {
    if end - start == 1 {
        format!(
            "'{codec}' codec can't decode byte 0x{:02x} in position {start}: {reason}",
            bytes[start]
        )
    } else {
        format!(
            "'{codec}' codec can't decode bytes in position {start}-{}: {reason}",
            end - 1
        )
    }
}

fn is_surrogate(point: u32) -> bool {
    (0xD800..=0xDFFF).contains(&point)
}

/// The surrogate a `surrogatepass` UTF-8 decoder accepts at the start of `rest` (CPython `PyCodec_SurrogatePassErrors`
/// takes any three-byte form whose code point is a surrogate), or `None`.
fn encoded_surrogate(rest: &[u8]) -> Option<u32> {
    match rest {
        [first, second, third, ..]
            if first & 0xF0 == 0xE0 && second & 0xC0 == 0x80 && third & 0xC0 == 0x80 =>
        {
            let point = (u32::from(first & 0x0F) << 12)
                | (u32::from(second & 0x3F) << 6)
                | u32::from(third & 0x3F);
            is_surrogate(point).then_some(point)
        }
        _ => None,
    }
}

/// Strict UTF-8 plus `surrogatepass`. Positions in a failure are relative to `bytes` (for `utf-8-sig`, to what follows
/// the signature, as in CPython).
fn decode_utf8(bytes: &[u8]) -> Result<Vec<u32>, String> {
    let mut points = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match std::str::from_utf8(&bytes[at..]) {
            Ok(valid) => {
                points.extend(valid.chars().map(u32::from));
                break;
            }
            Err(error) => {
                let start = at + error.valid_up_to();
                points.extend(
                    std::str::from_utf8(&bytes[at..start])
                        .expect("the prefix before the first error is valid")
                        .chars()
                        .map(u32::from),
                );
                if let Some(point) = encoded_surrogate(&bytes[start..]) {
                    points.push(point);
                    at = start + 3;
                    continue;
                }
                let (length, reason) = match error.error_len() {
                    None => (bytes.len() - start, "unexpected end of data"),
                    Some(length) if (0xC2..=0xF4).contains(&bytes[start]) => {
                        (length, "invalid continuation byte")
                    }
                    Some(length) => (length, "invalid start byte"),
                };
                return Err(decode_failure(
                    "utf-8",
                    bytes,
                    start,
                    start + length,
                    reason,
                ));
            }
        }
    }
    Ok(points)
}

fn unit16(bytes: &[u8], at: usize, big_endian: bool) -> u32 {
    let pair = [bytes[at], bytes[at + 1]];
    u32::from(if big_endian {
        u16::from_be_bytes(pair)
    } else {
        u16::from_le_bytes(pair)
    })
}

/// The `utf-16` family with `surrogatepass`: an unpaired surrogate passes through, a pair is combined, and only a
/// single trailing byte is an error (`truncated data`). `start` skips a byte-order mark; positions count from the
/// beginning of `bytes`.
fn decode_utf16(bytes: &[u8], start: usize, big_endian: bool) -> Result<Vec<u32>, String> {
    let mut points = Vec::with_capacity(bytes.len() / 2);
    let mut at = start;
    while at < bytes.len() {
        if bytes.len() - at < 2 {
            return Err(decode_failure(
                "utf-16",
                bytes,
                at,
                bytes.len(),
                "truncated data",
            ));
        }
        let unit = unit16(bytes, at, big_endian);
        at += 2;
        let is_high = (0xD800..0xDC00).contains(&unit);
        if is_high && bytes.len() - at >= 2 {
            let next = unit16(bytes, at, big_endian);
            if (0xDC00..=0xDFFF).contains(&next) {
                at += 2;
                points.push(0x10000 + ((unit - 0xD800) << 10) + (next - 0xDC00));
                continue;
            }
        }
        points.push(unit);
    }
    Ok(points)
}

/// The `utf-32` family with `surrogatepass`: a surrogate passes through; a value above U+10FFFF and a trailing
/// partial unit are errors.
fn decode_utf32(bytes: &[u8], start: usize, big_endian: bool) -> Result<Vec<u32>, String> {
    let mut points = Vec::with_capacity(bytes.len() / 4);
    let mut at = start;
    while at < bytes.len() {
        if bytes.len() - at < 4 {
            return Err(decode_failure(
                "utf-32",
                bytes,
                at,
                bytes.len(),
                "truncated data",
            ));
        }
        let quad = [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];
        let point = if big_endian {
            u32::from_be_bytes(quad)
        } else {
            u32::from_le_bytes(quad)
        };
        if point > 0x10FFFF {
            return Err(decode_failure(
                "utf-32",
                bytes,
                at,
                at + 4,
                "code point not in range(0x110000)",
            ));
        }
        points.push(point);
        at += 4;
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(bytes: &[u8]) -> String {
        decode_bytes(bytes)
            .expect("decodes")
            .to_utf8()
            .expect("no surrogates")
    }

    #[test]
    fn the_encoding_is_detected_like_json_detect_encoding() {
        assert_eq!(detect_encoding(b""), Encoding::Utf8);
        assert_eq!(detect_encoding(b"{}"), Encoding::Utf8);
        assert_eq!(detect_encoding(b"\xEF\xBB\xBF{}"), Encoding::Utf8Signature);
        assert_eq!(detect_encoding(b"\xFF\xFE{\0"), Encoding::Utf16WithMark);
        assert_eq!(
            detect_encoding(b"\xFF\xFE\0\0{\0\0\0"),
            Encoding::Utf32WithMark
        );
        assert_eq!(detect_encoding(b"\0{\0}"), Encoding::Utf16BigEndian);
        assert_eq!(detect_encoding(b"{\0}\0"), Encoding::Utf16LittleEndian);
        assert_eq!(detect_encoding(b"\0\0\0{"), Encoding::Utf32BigEndian);
        assert_eq!(detect_encoding(b"{\0\0\0"), Encoding::Utf32LittleEndian);
        assert_eq!(detect_encoding(b"\0{"), Encoding::Utf16BigEndian);
        assert_eq!(detect_encoding(b"{\0"), Encoding::Utf16LittleEndian);
        assert_eq!(detect_encoding(b"{\0x"), Encoding::Utf8);
    }

    #[test]
    fn marks_and_wide_encodings_decode_to_the_same_text() {
        assert_eq!(text(b"\xEF\xBB\xBF{}"), "{}");
        assert_eq!(text(b"\xFF\xFE{\0}\0"), "{}");
        assert_eq!(text(b"\xFE\xFF\0{\0}"), "{}");
        assert_eq!(text(b"{\0}\0"), "{}");
        assert_eq!(text(b"\0{\0}"), "{}");
        assert_eq!(text(b"\xFF\xFE\0\0{\0\0\0}\0\0\0"), "{}");
        assert_eq!(text(b"{\0\0\0}\0\0\0"), "{}");
        assert_eq!(text(b"\0\0\0{\0\0\0}"), "{}");
        // a surrogate pair and a character outside the BMP
        assert_eq!(text(b"\xFF\xFE\x3D\xD8\x00\xDE"), "\u{1F600}");
    }

    #[test]
    fn surrogatepass_keeps_lone_surrogates() {
        let decoded = decode_bytes(b"\xED\xA0\x80").expect("surrogatepass");
        assert_eq!(decoded.code_points(), &[0xD800]);
        let decoded = decode_bytes(b"\xFF\xFE\x00\xD8").expect("a lone high surrogate");
        assert_eq!(decoded.code_points(), &[0xD800]);
    }

    #[test]
    fn failures_carry_the_cpython_text() {
        assert_eq!(
            decode_bytes(b"{\"x\":\"\xFF\"}").unwrap_err(),
            "'utf-8' codec can't decode byte 0xff in position 6: invalid start byte"
        );
        assert_eq!(
            decode_bytes(b"\xE2\x82").unwrap_err(),
            "'utf-8' codec can't decode bytes in position 0-1: unexpected end of data"
        );
        assert_eq!(
            decode_bytes(b"\xEF\xBB\xBF\xFF").unwrap_err(),
            "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"
        );
        assert_eq!(
            decode_bytes(b"\xFF\xFE{\0}").unwrap_err(),
            "'utf-16' codec can't decode byte 0x7d in position 4: truncated data"
        );
        assert_eq!(
            decode_bytes(b"\0\0\0{\0\0\0").unwrap_err(),
            "'utf-32' codec can't decode bytes in position 4-6: truncated data"
        );
        assert_eq!(
            decode_bytes(b"\0\0\0{\x00\x11\x00\x00").unwrap_err(),
            "'utf-32' codec can't decode bytes in position 4-7: code point not in range(0x110000)"
        );
    }

    #[test]
    fn json_is_read_from_bytes() {
        let value = loads_bytes(b"\xEF\xBB\xBF{\"tag_name\":\"v1.2.3\"}").expect("BOM JSON");
        assert_eq!(value.dumps_ascii().unwrap(), "{\"tag_name\": \"v1.2.3\"}");
        assert!(loads_bytes(b"").is_err());
    }
}
