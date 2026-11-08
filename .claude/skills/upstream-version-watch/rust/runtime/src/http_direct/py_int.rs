//! `int(value, base)` of CPython 3.12.3 for the two kinds of operand `http.client` hands it: the text of a header or status field and the
//! bytes of a chunk-size line. Only the outcome that the callers branch on is modelled: the integer, or the `ValueError` message.
//!
//! Pinned sources: `Objects/longobject.c` (`PyLong_FromString`, `long_from_string_base`, `_PyLong_FromBytes`, `PyLong_FromUnicodeObject`) and
//! `Objects/unicodeobject.c` (`_PyUnicode_TransformDecimalAndSpaceToASCII`).
//!
//! * The literal parser is the same for both operands and works on C bytes: leading whitespace, one optional sign, for base 16 one optional
//!   `0x`/`0X` prefix (one underscore may follow it), digits with single underscores between them, at least one digit, then only trailing
//!   whitespace. Its whitespace is C `Py_ISSPACE`: space and `\t \n \v \f \r` only. ASCII `\x1c`..`\x1f` are NOT whitespace there.
//! * A **bytes** operand reaches the parser as it is.
//! * A **text** operand is first transformed: an all-ASCII text is used unchanged; otherwise each code point below 127 is copied (ASCII controls
//!   included), a later Unicode whitespace becomes a space, a Unicode decimal digit becomes its ASCII digit, and the first other code point becomes
//!   `?` and ends the transformed text.
//! * A base-10 literal of more than 4300 digits is refused by the default `int_max_str_digits` limit (the profile of the saved originals).
//! * The `ValueError` of a bad literal shows `%.200R`: for bytes the first 200 bytes are taken, their `repr` is cut at 200 characters; for text the
//!   `repr` of the whole original text is cut at 200 code points.

use agent_runtime_support::pystr::{decimal_digit_value, is_python_space, PyStr};
use agent_runtime_support::pyvalue::bytes_repr;
use num_bigint::BigInt;

/// The `-X int_max_str_digits` value of the interpreter profile the saved originals ran under.
const MAX_STR_DIGITS: usize = 4300;
/// The precision of `%.200R` and the byte limit of `_PyLong_FromBytes` in the literal error.
const ERROR_LITERAL_LIMIT: usize = 200;

/// What `int()` is applied to.
#[derive(Clone, Copy, Debug)]
pub enum IntOperand<'a> {
    Text(&'a str),
    Bytes(&'a [u8]),
}

/// `%.200R` of `text_repr`: at most 200 code points.
fn limited(text_repr: &str) -> String {
    text_repr.chars().take(ERROR_LITERAL_LIMIT).collect()
}

impl IntOperand<'_> {
    /// The `%.200R` of the operand, as `int()` shows it in its `ValueError`.
    fn literal_repr(&self) -> String {
        match self {
            IntOperand::Text(text) => limited(&PyStr::new(text).repr()),
            IntOperand::Bytes(bytes) => {
                limited(&bytes_repr(&bytes[..bytes.len().min(ERROR_LITERAL_LIMIT)]))
            }
        }
    }

    /// The bytes the C literal parser sees.
    fn parser_input(&self) -> Vec<u8> {
        match self {
            IntOperand::Bytes(bytes) => bytes.to_vec(),
            IntOperand::Text(text) if text.is_ascii() => text.as_bytes().to_vec(),
            IntOperand::Text(text) => {
                let mut transformed = Vec::with_capacity(text.len());
                for character in text.chars() {
                    let point = u32::from(character);
                    if point < 127 {
                        transformed.push(point as u8);
                    } else if is_python_space(point) {
                        transformed.push(b' ');
                    } else if let Some(digit) = decimal_digit_value(point) {
                        transformed.push(b'0' + digit as u8);
                    } else {
                        transformed.push(b'?');
                        break;
                    }
                }
                transformed
            }
        }
    }
}

/// C `Py_ISSPACE`.
fn is_c_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `int(operand, base)` for `base` 10 or 16: the integer, or the message of the `ValueError`.
pub fn parse_int(operand: IntOperand<'_>, base: u32) -> Result<BigInt, String> {
    let invalid = || {
        format!(
            "invalid literal for int() with base {base}: {}",
            operand.literal_repr()
        )
    };
    let bytes = operand.parser_input();
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && is_c_space(bytes[start]) {
        start += 1;
    }
    while end > start && is_c_space(bytes[end - 1]) {
        end -= 1;
    }
    let mut rest = &bytes[start..end];
    let negative = rest.first() == Some(&b'-');
    if matches!(rest.first(), Some(b'-' | b'+')) {
        rest = &rest[1..];
    }
    let mut after_prefix = false;
    if base == 16 && rest.len() >= 2 && rest[0] == b'0' && matches!(rest[1], b'x' | b'X') {
        rest = &rest[2..];
        after_prefix = true;
    }
    if after_prefix && rest.first() == Some(&b'_') {
        rest = &rest[1..];
    }
    let mut digits = Vec::with_capacity(rest.len());
    let mut previous_was_digit = false;
    for (at, &byte) in rest.iter().enumerate() {
        if byte == b'_' {
            let followed_by_digit = rest
                .get(at + 1)
                .is_some_and(|next| char::from(*next).is_digit(base));
            if !previous_was_digit || !followed_by_digit {
                return Err(invalid());
            }
            previous_was_digit = false;
        } else if char::from(byte).is_digit(base) {
            digits.push(byte);
            previous_was_digit = true;
        } else {
            return Err(invalid());
        }
    }
    if digits.is_empty() {
        return Err(invalid());
    }
    if base == 10 && digits.len() > MAX_STR_DIGITS {
        return Err(format!(
            "Exceeds the limit ({MAX_STR_DIGITS} digits) for integer string conversion: value has {} digits; use sys.set_int_max_str_digits() to increase the limit",
            digits.len()
        ));
    }
    let magnitude = BigInt::parse_bytes(&digits, base).ok_or_else(invalid)?;
    Ok(if negative { -magnitude } else { magnitude })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str, base: u32) -> Result<BigInt, String> {
        parse_int(IntOperand::Text(value), base)
    }

    #[test]
    fn decimal_literals_follow_cpython() {
        assert_eq!(text("200", 10), Ok(BigInt::from(200)));
        assert_eq!(text(" +200\r\n", 10), Ok(BigInt::from(200)));
        assert_eq!(text("-1", 10), Ok(BigInt::from(-1)));
        assert_eq!(text("1_0", 10), Ok(BigInt::from(10)));
        assert_eq!(text("007", 10), Ok(BigInt::from(7)));
        assert_eq!(
            text("ABC", 10),
            Err("invalid literal for int() with base 10: 'ABC'".to_string())
        );
        assert!(text("1__0", 10).is_err());
        assert!(text("_1", 10).is_err());
        assert!(text("1_", 10).is_err());
        assert!(text("", 10).is_err());
        assert!(text("+", 10).is_err());
        assert!(text("1 2", 10).is_err());
        assert!(text("0x10", 10).is_err());
    }

    #[test]
    fn hexadecimal_chunk_sizes_follow_cpython() {
        let bytes = |value: &[u8]| parse_int(IntOperand::Bytes(value), 16);
        assert_eq!(bytes(b"5\r\n"), Ok(BigInt::from(5)));
        assert_eq!(bytes(b"0x1F"), Ok(BigInt::from(31)));
        assert_eq!(bytes(b"-5"), Ok(BigInt::from(-5)));
        assert_eq!(
            bytes(b"NOPE\r\n"),
            Err("invalid literal for int() with base 16: b'NOPE\\r\\n'".to_string())
        );
        assert_eq!(
            bytes(b""),
            Err("invalid literal for int() with base 16: b''".to_string())
        );
        assert!(bytes(b"0x").is_err());
    }

    #[test]
    fn text_maps_unicode_whitespace_and_refuses_other_non_ascii() {
        assert_eq!(text("\u{a0}5\u{85}", 10), Ok(BigInt::from(5)));
        assert!(text("\u{b2}", 10).is_err());
    }

    #[test]
    fn long_decimal_literals_hit_the_interpreter_limit() {
        let long = "1".repeat(4301);
        let refusal = text(&long, 10).unwrap_err();
        assert!(refusal.starts_with("Exceeds the limit (4300 digits)"));
        assert!(text(&"1".repeat(4300), 10).is_ok());
    }

    #[test]
    fn ascii_separator_controls_are_not_whitespace_for_either_operand() {
        // N1: `\x1c`..`\x1f` are copied by the transform and are not `Py_ISSPACE`, so `int()` rejects them.
        for control in ['\x1c', '\x1d', '\x1e', '\x1f'] {
            assert!(
                text(&format!("{control}1"), 10).is_err(),
                "{control:?} leading"
            );
            assert!(
                text(&format!("1{control}"), 10).is_err(),
                "{control:?} trailing"
            );
            let bytes = [control as u8, b'1'];
            assert!(parse_int(IntOperand::Bytes(&bytes), 10).is_err());
        }
        // a Latin-1 text (not all ASCII) takes the per-code-point transform: the control is still copied, so still rejected
        assert!(text("\u{1c}1\u{e9}", 10).is_err());
    }

    #[test]
    fn c_whitespace_and_unicode_space_are_accepted_where_cpython_accepts_them() {
        // all-ASCII text and bytes: vertical tab, form feed, newline, carriage return, tab, space are `Py_ISSPACE`
        for blank in ['\x0b', '\x0c', '\n', '\r', '\t', ' '] {
            assert_eq!(text(&format!("{blank}1{blank}"), 10), Ok(BigInt::from(1)));
            let bytes = [blank as u8, b'1', blank as u8];
            assert_eq!(
                parse_int(IntOperand::Bytes(&bytes), 10),
                Ok(BigInt::from(1))
            );
        }
        // non-breaking space and NEL are Unicode whitespace: text accepts them (transformed to a space), bytes do not
        assert_eq!(text("\u{a0}1", 10), Ok(BigInt::from(1)));
        assert_eq!(text("1\u{85}", 10), Ok(BigInt::from(1)));
        assert!(parse_int(IntOperand::Bytes(b"\xa01"), 10).is_err());
        // a sign is read after leading whitespace only, and whitespace after it is not skipped
        assert_eq!(text("\u{a0}-1", 10), Ok(BigInt::from(-1)));
        assert!(text("-\u{a0}1", 10).is_err());
        // DEL is not whitespace and not a digit in either branch
        assert!(text("\u{7f}1", 10).is_err());
    }

    #[test]
    fn the_transform_ends_at_the_first_non_decimal_non_space_character() {
        assert_eq!(
            text("12\u{e9}", 10),
            Err("invalid literal for int() with base 10: '12\u{e9}'".to_string())
        );
        // an Arabic-Indic digit is a Unicode decimal digit and becomes its ASCII digit
        assert_eq!(text("\u{661}\u{662}", 10), Ok(BigInt::from(12)));
    }

    #[test]
    fn long_literals_in_the_error_are_cut_like_percent_200_r() {
        // N2, bytes: the first 200 bytes, then their repr cut at 200 characters (`b'` + 198 bytes)
        let long_bytes = vec![b'G'; 201];
        let message = parse_int(IntOperand::Bytes(&long_bytes), 16).unwrap_err();
        let prefix = "invalid literal for int() with base 16: ";
        let shown = message.strip_prefix(prefix).unwrap();
        assert_eq!(shown.chars().count(), 200);
        assert!(shown.starts_with("b'GGG") && !shown.ends_with('\''));
        assert_eq!(message.chars().count(), 240);
        // a short bytes literal keeps its closing quote
        let short = parse_int(IntOperand::Bytes(b"GG"), 16).unwrap_err();
        assert_eq!(short, format!("{prefix}b'GG'"));
        // N2, text: the repr of the whole text cut at 200 code points (`'` + 199 characters)
        let long_text = "G".repeat(219);
        let message = text(&long_text, 10).unwrap_err();
        let shown = message
            .strip_prefix("invalid literal for int() with base 10: ")
            .unwrap();
        assert_eq!(shown.chars().count(), 200);
        assert_eq!(shown, format!("'{}", "G".repeat(199)));
    }

    #[test]
    fn the_error_cut_counts_code_points_and_escapes_after_the_byte_limit() {
        // code points above U+FFFF count once (not as two UTF-16 units)
        let astral = format!("\u{1F600}{}", "G".repeat(300));
        let message = text(&astral, 10).unwrap_err();
        let shown = message
            .strip_prefix("invalid literal for int() with base 10: ")
            .unwrap();
        assert_eq!(shown.chars().count(), 200);
        assert!(shown.starts_with("'\u{1F600}GG"));
        // bytes: 200 bytes are taken first, and escapes (`\xff` is four characters) are cut by the 200-character limit afterwards
        let high = vec![0xffu8; 250];
        let message = parse_int(IntOperand::Bytes(&high), 16).unwrap_err();
        let shown = message
            .strip_prefix("invalid literal for int() with base 16: ")
            .unwrap();
        assert_eq!(shown.chars().count(), 200);
        assert!(shown.starts_with("b'\\xff\\xff"));
    }
}
