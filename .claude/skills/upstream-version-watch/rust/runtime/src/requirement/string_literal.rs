//! `ast.literal_eval(quoted_string)` as `packaging._parser.process_python_str` calls it for the quoted strings of a marker: whether the literal
//! compiles, and the `SyntaxWarning` CPython 3.12.3 prints to standard error for an invalid escape sequence.
//!
//! The quoted string is one token of the grammar's `QUOTED_STRING` rule (a quote, characters other than that quote, the quote). The value of the
//! literal is not needed: packaging keeps it only inside a marker that the requirement projection never evaluates. What matters is the
//! observable behaviour of compiling the literal: the tokenizer's view of the text (`Parser/tokenizer.c`), the escape decoding
//! (`Parser/string_parser.c` and `_PyUnicode_DecodeUnicodeEscapeInternal` of `Objects/unicodeobject.c`) and the warning text.
//!
//! Derived from CPython 3.12.3 (PSF License Version 2); see `notices/NOTICE-unicode-names.md`.

use super::unicode_names::code_of_name;

/// Why `ast.literal_eval` raises for the literal (every case is an `Exception` the original's caller swallows).
#[derive(Debug, PartialEq, Eq)]
pub(super) enum LiteralRejection {
    /// `source code string cannot contain null bytes`.
    NullCharacter,
    /// A lone surrogate cannot be encoded to the UTF-8 source (`UnicodeEncodeError`).
    UnencodableCharacter,
    /// The tokenizer finds no closed single-line literal (unterminated string).
    UnterminatedString,
    /// The escape decoder raises (`unicodeescape` codec: truncated, malformed or unknown escape).
    BadEscape,
}

/// What compiling an accepted literal printed.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Accepted {
    /// The `SyntaxWarning` line written to standard error (`None` when the literal is silent).
    pub(super) warning: Option<String>,
}

const MAX_UNICODE: u32 = 0x10FFFF;
const BACKSLASH: u32 = 0x5C;
const NEWLINE: u32 = 0x0A;

/// The tokenizer's input translation for a source string: `\r\n` and a lone `\r` become `\n`.
fn translate_newlines(text: &[u32]) -> Vec<u32> {
    let mut translated = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        if text[at] == 0x0D {
            translated.push(NEWLINE);
            if text.get(at + 1) == Some(&NEWLINE) {
                at += 1;
            }
        } else {
            translated.push(text[at]);
        }
        at += 1;
    }
    translated
}

/// The string scan of the tokenizer for a single-quoted literal: a backslash takes the next character with it, a bare newline or the end
/// of the text before the closing quote is an unterminated string. Returns the index of the closing quote.
fn closing_quote(source: &[u32], quote: u32) -> Result<usize, LiteralRejection> {
    let mut at = 1;
    while at < source.len() {
        match source[at] {
            BACKSLASH => at += 1,
            NEWLINE => return Err(LiteralRejection::UnterminatedString),
            c if c == quote => return Ok(at),
            _ => {}
        }
        at += 1;
    }
    Err(LiteralRejection::UnterminatedString)
}

fn hex_value(code_point: u32) -> Option<u32> {
    char::from_u32(code_point).and_then(|c| c.to_digit(16))
}

/// The first invalid escape of a decoded body, as the C decoder remembers it (`first_invalid_escape`).
enum FirstInvalid {
    /// `\` followed by this character.
    Escape(u32),
    /// An octal escape above `\377`, with its three digits.
    Octal([u32; 3]),
}

fn warning_line(invalid: &FirstInvalid) -> String {
    let message = match invalid {
        FirstInvalid::Escape(c) => format!(
            "invalid escape sequence '\\{}'",
            char::from_u32(*c).unwrap_or('\u{fffd}')
        ),
        FirstInvalid::Octal(digits) => format!(
            "invalid octal escape sequence '\\{}'",
            digits
                .iter()
                .map(|&digit| char::from_u32(digit).unwrap_or('\u{fffd}'))
                .collect::<String>()
        ),
    };
    format!("<unknown>:1: SyntaxWarning: {message}\n")
}

/// `decode_unicode_with_escapes`: the escape decoder over the literal body. Non-ASCII characters are ordinary characters (the C code rewrites them
/// to `\UXXXXXXXX`, and a backslash before one to `\u005c`, so they can neither start nor complete an escape).
fn decode_body(body: &[u32]) -> Result<Option<FirstInvalid>, LiteralRejection> {
    let mut first_invalid = None;
    let mut at = 0;
    while at < body.len() {
        let c = body[at];
        at += 1;
        if c != BACKSLASH {
            continue;
        }
        let Some(&escaped) = body.get(at) else {
            // a trailing backslash is rewritten to `\u005c`: a backslash
            break;
        };
        if escaped >= 0x80 {
            // `\` before a non-ASCII character is `\u005c` followed by that character
            continue;
        }
        at += 1;
        match char::from_u32(escaped).unwrap_or('\0') {
            '\n' | '\\' | '\'' | '"' | 'b' | 'f' | 't' | 'n' | 'r' | 'v' | 'a' => {}
            '0'..='7' => {
                let mut value = escaped - u32::from(b'0');
                let mut digits = vec![escaped];
                for _ in 0..2 {
                    match body.get(at) {
                        Some(&next) if (0x30..=0x37).contains(&next) => {
                            value = (value << 3) + next - 0x30;
                            digits.push(next);
                            at += 1;
                        }
                        _ => break,
                    }
                }
                if value > 0o377 && first_invalid.is_none() {
                    first_invalid = Some(FirstInvalid::Octal([digits[0], digits[1], digits[2]]));
                }
            }
            'x' | 'u' | 'U' => {
                let count = match escaped {
                    0x78 => 2,
                    0x75 => 4,
                    _ => 8,
                };
                let mut value: u32 = 0;
                for _ in 0..count {
                    let digit = body
                        .get(at)
                        .copied()
                        .and_then(hex_value)
                        .ok_or(LiteralRejection::BadEscape)?;
                    value = (value << 4) | digit;
                    at += 1;
                }
                if value > MAX_UNICODE {
                    return Err(LiteralRejection::BadEscape);
                }
            }
            'N' => {
                if body.get(at) != Some(&u32::from(b'{')) {
                    return Err(LiteralRejection::BadEscape);
                }
                at += 1;
                let start = at;
                while at < body.len() && body[at] != u32::from(b'}') {
                    at += 1;
                }
                if at >= body.len() || at == start {
                    return Err(LiteralRejection::BadEscape);
                }
                let name = &body[start..at];
                at += 1;
                // Names are looked up as the C decoder sees them: a non-ASCII character was rewritten to `\U...`, which is never part of a name.
                let ascii: Option<Vec<u8>> = name
                    .iter()
                    .map(|&c| u8::try_from(c).ok().filter(u8::is_ascii))
                    .collect();
                if ascii.as_deref().and_then(code_of_name).is_none() {
                    return Err(LiteralRejection::BadEscape);
                }
            }
            _ => {
                if first_invalid.is_none() {
                    first_invalid = Some(FirstInvalid::Escape(escaped));
                }
            }
        }
    }
    Ok(first_invalid)
}

/// Compiles the marker's quoted string the way `ast.literal_eval` does; `token` is the whole quoted string including its quotes.
pub(super) fn compile_quoted_string(token: &[u32]) -> Result<Accepted, LiteralRejection> {
    if token.contains(&0) {
        return Err(LiteralRejection::NullCharacter);
    }
    if token.iter().any(|c| (0xD800..=0xDFFF).contains(c)) {
        return Err(LiteralRejection::UnencodableCharacter);
    }
    let source = translate_newlines(token);
    let quote = source[0];
    let closing = closing_quote(&source, quote)?;
    if closing != source.len() - 1 {
        // The quoted-string rule never yields text after a closed literal; refusing here keeps that assumption visible.
        return Err(LiteralRejection::UnterminatedString);
    }
    let body = &source[1..closing];
    if !body.contains(&BACKSLASH) {
        return Ok(Accepted { warning: None });
    }
    let first_invalid = decode_body(body)?;
    Ok(Accepted {
        warning: first_invalid.as_ref().map(warning_line),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(text: &str) -> Vec<u32> {
        text.chars().map(u32::from).collect()
    }

    fn compile(text: &str) -> Result<Accepted, LiteralRejection> {
        compile_quoted_string(&points(text))
    }

    #[test]
    fn plain_and_valid_escapes_are_silent() {
        assert_eq!(compile("\"abc\"").unwrap().warning, None);
        assert_eq!(
            compile(r#""a\x41\u0041\N{LATIN CAPITAL LETTER A}\n""#)
                .unwrap()
                .warning,
            None
        );
    }

    #[test]
    fn an_unknown_escape_warns_once_for_the_first_one() {
        let accepted = compile(r#""a\q\z""#).unwrap();
        assert_eq!(
            accepted.warning.as_deref(),
            Some("<unknown>:1: SyntaxWarning: invalid escape sequence '\\q'\n")
        );
    }

    #[test]
    fn an_octal_escape_above_377_warns_with_its_digits() {
        let accepted = compile(r#""\400""#).unwrap();
        assert_eq!(
            accepted.warning.as_deref(),
            Some("<unknown>:1: SyntaxWarning: invalid octal escape sequence '\\400'\n")
        );
    }

    #[test]
    fn bad_escapes_and_bad_text_are_refused() {
        assert_eq!(compile(r#""\x4""#), Err(LiteralRejection::BadEscape));
        assert_eq!(
            compile(r#""\N{NO SUCH NAME}""#),
            Err(LiteralRejection::BadEscape)
        );
        assert_eq!(compile(r#""\U00110000""#), Err(LiteralRejection::BadEscape));
        assert_eq!(
            compile("\"a\n\""),
            Err(LiteralRejection::UnterminatedString)
        );
        assert_eq!(compile("\"a\0\""), Err(LiteralRejection::NullCharacter));
        assert_eq!(
            compile(r#""a\""#),
            Err(LiteralRejection::UnterminatedString)
        );
    }

    #[test]
    fn a_backslash_continues_the_line() {
        assert_eq!(compile("\"a\\\nb\"").unwrap().warning, None);
        assert_eq!(compile("\"a\\\r\nb\"").unwrap().warning, None);
    }
}
