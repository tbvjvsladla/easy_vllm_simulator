//! `bytes.decode("unicode_escape")` of CPython 3.12.3 (`_PyUnicode_DecodeUnicodeEscapeInternal`, read from the source): every byte that is not part of
//! a backslash escape becomes the code point of the same value (so UTF-8 text turns into its Latin-1 reading), the escapes `\\ \' \" \a \b \f \n \r \t \v`,
//! octal (up to three digits), `\xXX`, `\uXXXX`, `\UXXXXXXXX` and a backslash-newline are decoded, and anything else keeps its backslash.
//!
//! Errors are the codec's strict ones (`UnicodeDecodeError('unicodeescape', ...)` with the codec's `reason` and the start/end of the bad run). An
//! unknown escape and an octal value above `0o377` are only *warnings* in the interpreter (a `DeprecationWarning`, still decoded); this decoder reports
//! the first of them in [`Decoded::first_invalid_escape`] so the caller can decide whether the warning is visible. `\N{name}` needs the Unicode name
//! database, which is not shipped: a well-formed one is [`EscapeFailure::NamedCharacter`].

use agent_runtime_support::pystr::PyStr;

/// The first escape the interpreter would warn about: its text as the warning prints it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvalidEscape {
    /// `invalid escape sequence '\q'`
    Unknown(u8),
    /// `invalid octal escape sequence '\400'`
    Octal(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Decoded {
    pub text: PyStr,
    pub first_invalid_escape: Option<InvalidEscape>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EscapeFailure {
    /// `UnicodeDecodeError('unicodeescape', object, start, end, reason)`.
    Error {
        start: usize,
        end: usize,
        reason: &'static str,
    },
    /// A well-formed `\N{name}`: not decided by this port.
    NamedCharacter,
}

fn hex_value(byte: u8) -> Option<u32> {
    char::from(byte).to_digit(16)
}

pub fn decode(input: &[u8]) -> Result<Decoded, EscapeFailure> {
    let mut out: Vec<u32> = Vec::with_capacity(input.len());
    let mut first_invalid = None;
    let end = input.len();
    let mut s = 0usize;
    while s < end {
        let start = s;
        let byte = input[s];
        s += 1;
        if byte != b'\\' {
            out.push(u32::from(byte));
            continue;
        }
        let fail = |message: &'static str, at: usize| EscapeFailure::Error {
            start,
            end: at,
            reason: message,
        };
        if s >= end {
            return Err(fail("\\ at end of string", s));
        }
        let c = input[s];
        s += 1;
        match c {
            b'\n' => {}
            b'\\' => out.push(0x5C),
            b'\'' => out.push(0x27),
            b'"' => out.push(0x22),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b't' => out.push(9),
            b'n' => out.push(10),
            b'r' => out.push(13),
            b'v' => out.push(11),
            b'a' => out.push(7),
            b'0'..=b'7' => {
                let mut ch = u32::from(c - b'0');
                if s < end && (b'0'..=b'7').contains(&input[s]) {
                    ch = (ch << 3) + u32::from(input[s] - b'0');
                    s += 1;
                    if s < end && (b'0'..=b'7').contains(&input[s]) {
                        ch = (ch << 3) + u32::from(input[s] - b'0');
                        s += 1;
                    }
                }
                if ch > 0o377 && first_invalid.is_none() {
                    first_invalid = Some(InvalidEscape::Octal(
                        String::from_utf8_lossy(&input[s - 3..s]).into_owned(),
                    ));
                }
                out.push(ch);
            }
            b'x' | b'u' | b'U' => {
                let (count, message) = match c {
                    b'x' => (2, "truncated \\xXX escape"),
                    b'u' => (4, "truncated \\uXXXX escape"),
                    _ => (8, "truncated \\UXXXXXXXX escape"),
                };
                let mut ch: u32 = 0;
                for _ in 0..count {
                    if s >= end {
                        return Err(fail(message, s));
                    }
                    let Some(digit) = hex_value(input[s]) else {
                        return Err(fail(message, s));
                    };
                    ch = ch.wrapping_shl(4).wrapping_add(digit);
                    s += 1;
                }
                if ch > 0x10FFFF {
                    return Err(fail("illegal Unicode character", s));
                }
                out.push(ch);
            }
            b'N' => {
                let message = "malformed \\N character escape";
                if s >= end {
                    return Err(fail(message, s));
                }
                if input[s] == b'{' {
                    s += 1;
                    let name_start = s;
                    while s < end && input[s] != b'}' {
                        s += 1;
                    }
                    if s >= end {
                        return Err(fail(message, s));
                    }
                    if s - name_start > 0 {
                        return Err(EscapeFailure::NamedCharacter);
                    }
                }
                return Err(fail(message, s));
            }
            other => {
                if first_invalid.is_none() {
                    first_invalid = Some(InvalidEscape::Unknown(other));
                }
                out.push(0x5C);
                out.push(u32::from(other));
            }
        }
    }
    Ok(Decoded {
        text: PyStr::from_code_points(out),
        first_invalid_escape: first_invalid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(input: &[u8]) -> String {
        decode(input)
            .unwrap()
            .text
            .code_points()
            .iter()
            .map(|&c| char::from_u32(c).unwrap_or('\u{fffd}'))
            .collect()
    }

    #[test]
    fn plain_bytes_are_read_as_latin_1_and_known_escapes_are_decoded() {
        assert_eq!(text("é".as_bytes()), "Ã©");
        assert_eq!(text(b"a\\nb\\\\c\\x41\\u0041\\101"), "a\nb\\cAAA");
        assert_eq!(text(b"x\\\ny"), "xy");
        assert_eq!(decode(b"\\ud800").unwrap().text.code_points(), &[0xD800]);
        assert_eq!(
            decode(b"\\U0001F600").unwrap().text.code_points(),
            &[0x1F600]
        );
    }

    #[test]
    fn unknown_escapes_keep_the_backslash_and_report_the_first_warning() {
        let decoded = decode(b"\\q\\w").unwrap();
        assert_eq!(decoded.text.code_points(), &[92, 113, 92, 119]);
        assert_eq!(
            decoded.first_invalid_escape,
            Some(InvalidEscape::Unknown(b'q'))
        );
        assert_eq!(
            decode(b"\\400").unwrap().first_invalid_escape,
            Some(InvalidEscape::Octal("400".to_string()))
        );
    }

    #[test]
    fn truncated_and_malformed_escapes_give_the_codecs_error_runs() {
        assert_eq!(
            decode(b"\\x0"),
            Err(EscapeFailure::Error {
                start: 0,
                end: 3,
                reason: "truncated \\xXX escape"
            })
        );
        assert_eq!(
            decode(b"ab\\xZ1"),
            Err(EscapeFailure::Error {
                start: 2,
                end: 4,
                reason: "truncated \\xXX escape"
            })
        );
        assert_eq!(
            decode(b"\\"),
            Err(EscapeFailure::Error {
                start: 0,
                end: 1,
                reason: "\\ at end of string"
            })
        );
        assert_eq!(
            decode(b"\\U00110000"),
            Err(EscapeFailure::Error {
                start: 0,
                end: 10,
                reason: "illegal Unicode character"
            })
        );
        assert_eq!(
            decode(b"\\N"),
            Err(EscapeFailure::Error {
                start: 0,
                end: 2,
                reason: "malformed \\N character escape"
            })
        );
        assert_eq!(
            decode(b"\\N{}"),
            Err(EscapeFailure::Error {
                start: 0,
                end: 3,
                reason: "malformed \\N character escape"
            })
        );
        assert_eq!(
            decode(b"\\N{LATIN SMALL LETTER A}"),
            Err(EscapeFailure::NamedCharacter)
        );
    }
}
