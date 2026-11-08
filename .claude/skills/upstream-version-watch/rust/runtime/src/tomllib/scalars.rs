//! `tomllib/_re.py`: the number, local-time and date-time patterns and their conversions. The three regular expressions are matched by hand: each
//! alternative and optional group is tried in the pattern's order, and because everything after a mandatory part is optional the greedy choice
//! never needs backtracking.

use super::failure::{ExceptionClass, Raised, UnsupportedDomain};
use super::value::{CivilDate, CivilDateTime, CivilTime, TomlValue, UtcOffset};
use num_bigint::{BigInt, BigUint, Sign};

/// `sys.get_int_max_str_digits()` of the frozen interpreter profile (`-X int_max_str_digits=4300`, also the default).
pub const INT_MAX_STR_DIGITS: usize = 4300;

fn at(src: &[u32], pos: usize) -> u32 {
    src.get(pos).copied().unwrap_or(u32::MAX)
}

fn is_digit(code: u32) -> bool {
    (u32::from(b'0')..=u32::from(b'9')).contains(&code)
}

fn is_hex_digit(code: u32) -> bool {
    char::from_u32(code).is_some_and(|c| c.is_ascii_hexdigit())
}

fn is_octal_digit(code: u32) -> bool {
    (u32::from(b'0')..=u32::from(b'7')).contains(&code)
}

fn is_binary_digit(code: u32) -> bool {
    code == u32::from(b'0') || code == u32::from(b'1')
}

fn is_char(code: u32, expected: char) -> bool {
    code == u32::from(expected)
}

fn digit_value(code: u32) -> u32 {
    code - u32::from(b'0')
}

/// `[digit](?:_?[digit])*` from `pos` (the first digit already verified by the caller): the end of the longest run.
fn end_of_digit_run(src: &[u32], mut pos: usize, is_member: fn(u32) -> bool) -> usize {
    pos += 1;
    loop {
        if is_member(at(src, pos)) {
            pos += 1;
        } else if is_char(at(src, pos), '_') && is_member(at(src, pos + 1)) {
            pos += 2;
        } else {
            return pos;
        }
    }
}

/// What `RE_NUMBER` matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberMatch {
    pub end: usize,
    /// Whether the `floatpart` group matched something (`match.group("floatpart")` is truthy).
    pub is_float: bool,
}

/// `RE_NUMBER.match(src, pos)`.
pub fn match_number(src: &[u32], pos: usize) -> Option<NumberMatch> {
    if is_char(at(src, pos), '0') {
        let radix_digit: Option<fn(u32) -> bool> = match char::from_u32(at(src, pos + 1)) {
            Some('x') => Some(is_hex_digit),
            Some('b') => Some(is_binary_digit),
            Some('o') => Some(is_octal_digit),
            _ => None,
        };
        if let Some(is_member) = radix_digit {
            if is_member(at(src, pos + 2)) {
                return Some(NumberMatch {
                    end: end_of_digit_run(src, pos + 2, is_member),
                    is_float: false,
                });
            }
        }
    }
    let mut cursor = pos;
    if is_char(at(src, cursor), '+') || is_char(at(src, cursor), '-') {
        cursor += 1;
    }
    let first = at(src, cursor);
    if is_char(first, '0') {
        cursor += 1;
    } else if is_digit(first) {
        cursor = end_of_digit_run(src, cursor, is_digit);
    } else {
        return None;
    }
    let integer_end = cursor;
    if is_char(at(src, cursor), '.') && is_digit(at(src, cursor + 1)) {
        cursor = end_of_digit_run(src, cursor + 1, is_digit);
    }
    if matches!(char::from_u32(at(src, cursor)), Some('e' | 'E')) {
        let mut exponent = cursor + 1;
        if is_char(at(src, exponent), '+') || is_char(at(src, exponent), '-') {
            exponent += 1;
        }
        if is_digit(at(src, exponent)) {
            cursor = end_of_digit_run(src, exponent, is_digit);
        }
    }
    Some(NumberMatch {
        end: cursor,
        is_float: cursor != integer_end,
    })
}

fn ascii(src: &[u32]) -> String {
    src.iter()
        .filter_map(|&code| char::from_u32(code))
        .collect()
}

/// `match_to_number`: `parse_float(text)` (the default `float`) for a float, `int(text, 0)` otherwise.
pub fn number_value(
    src: &[u32],
    start: usize,
    matched: NumberMatch,
) -> Result<TomlValue, NumberFailure> {
    let text = ascii(&src[start..matched.end]);
    if matched.is_float {
        let digits_only: String = text.chars().filter(|&c| c != '_').collect();
        return digits_only
            .parse::<f64>()
            .map(TomlValue::Float)
            .map_err(|_| NumberFailure::Unsupported(UnsupportedDomain::FloatTextNotConvertible));
    }
    int_from_literal(&text)
        .map(TomlValue::Int)
        .map_err(NumberFailure::Raised)
}

/// How a matched number fails to become a value.
#[derive(Debug)]
pub enum NumberFailure {
    Raised(Raised),
    Unsupported(UnsupportedDomain),
}

/// `int(text, 0)` for text the number pattern accepted: an optional sign with decimal digits, or a `0x`/`0o`/`0b` literal; underscores between
/// digits are dropped. A decimal literal of more than [`INT_MAX_STR_DIGITS`] digits is the `ValueError` of the interpreter's conversion limit;
/// the power-of-two bases have no such limit.
fn int_from_literal(text: &str) -> Result<BigInt, Raised> {
    let unsigned = text.trim_start_matches(['+', '-']);
    let negative = text.starts_with('-');
    let (radix, digits) = match unsigned.get(..2) {
        Some("0x") => (16, &unsigned[2..]),
        Some("0o") => (8, &unsigned[2..]),
        Some("0b") => (2, &unsigned[2..]),
        _ => (10, unsigned),
    };
    let digits: String = digits.chars().filter(|&c| c != '_').collect();
    if radix == 10 && digits.len() > INT_MAX_STR_DIGITS {
        return Err(Raised::plain(
            ExceptionClass::ValueError,
            format!(
                "Exceeds the limit ({INT_MAX_STR_DIGITS} digits) for integer string conversion: value has {} digits; use sys.set_int_max_str_digits() to increase the limit",
                digits.len()
            ),
        ));
    }
    let magnitude = BigUint::parse_bytes(digits.as_bytes(), radix)
        .expect("the number pattern admits only digits of the radix");
    Ok(BigInt::from_biguint(
        if negative { Sign::Minus } else { Sign::Plus },
        magnitude,
    ))
}

/// The captured pieces of a time (`RE_LOCALTIME`), with the end of the match.
#[derive(Debug, Clone, Copy)]
struct TimeMatch {
    end: usize,
    time: CivilTime,
}

/// `[01][0-9]|2[0-3]` at `pos`: the hour value.
fn hour_at(src: &[u32], pos: usize) -> Option<u8> {
    let (first, second) = (at(src, pos), at(src, pos + 1));
    let valid = ((is_char(first, '0') || is_char(first, '1')) && is_digit(second))
        || (is_char(first, '2') && (u32::from(b'0')..=u32::from(b'3')).contains(&second));
    valid.then(|| (digit_value(first) * 10 + digit_value(second)) as u8)
}

/// `[0-5][0-9]` at `pos`.
fn minute_or_second_at(src: &[u32], pos: usize) -> Option<u8> {
    let (first, second) = (at(src, pos), at(src, pos + 1));
    ((u32::from(b'0')..=u32::from(b'5')).contains(&first) && is_digit(second))
        .then(|| (digit_value(first) * 10 + digit_value(second)) as u8)
}

/// `_TIME_RE_STR` at `pos`: `HH:MM:SS` with an optional fraction whose first six digits are kept and the rest dropped.
fn match_time_text(src: &[u32], pos: usize) -> Option<TimeMatch> {
    let hour = hour_at(src, pos)?;
    if !is_char(at(src, pos + 2), ':') {
        return None;
    }
    let minute = minute_or_second_at(src, pos + 3)?;
    if !is_char(at(src, pos + 5), ':') {
        return None;
    }
    let second = minute_or_second_at(src, pos + 6)?;
    let mut end = pos + 8;
    let mut microsecond = 0;
    if is_char(at(src, end), '.') && is_digit(at(src, end + 1)) {
        let first_digit = end + 1;
        end = first_digit;
        while is_digit(at(src, end)) {
            end += 1;
        }
        let kept = (end - first_digit).min(6);
        let mut value = 0u32;
        for offset in 0..6 {
            let digit = if offset < kept {
                digit_value(src[first_digit + offset])
            } else {
                0
            };
            value = value * 10 + digit;
        }
        microsecond = value;
    }
    Some(TimeMatch {
        end,
        time: CivilTime {
            hour,
            minute,
            second,
            microsecond,
        },
    })
}

/// `RE_LOCALTIME.match(src, pos)` and `match_to_localtime`: the end of the match and the time (a matched time is always valid).
pub fn match_local_time(src: &[u32], pos: usize) -> Option<(usize, CivilTime)> {
    match_time_text(src, pos).map(|matched| (matched.end, matched.time))
}

/// What `RE_DATETIME` matched, before the date is validated.
#[derive(Debug, Clone, Copy)]
pub struct DateTimeMatch {
    pub end: usize,
    year: u16,
    month: u8,
    day: u8,
    time: Option<CivilTime>,
    offset: Option<UtcOffset>,
}

fn four_digit_year_at(src: &[u32], pos: usize) -> Option<u16> {
    let mut year = 0u16;
    for offset in 0..4 {
        let code = at(src, pos + offset);
        if !is_digit(code) {
            return None;
        }
        year = year * 10 + digit_value(code) as u16;
    }
    Some(year)
}

/// `0[1-9]|1[0-2]` at `pos`.
fn month_at(src: &[u32], pos: usize) -> Option<u8> {
    let (first, second) = (at(src, pos), at(src, pos + 1));
    let valid = (is_char(first, '0') && (u32::from(b'1')..=u32::from(b'9')).contains(&second))
        || (is_char(first, '1') && (u32::from(b'0')..=u32::from(b'2')).contains(&second));
    valid.then(|| (digit_value(first) * 10 + digit_value(second)) as u8)
}

/// `0[1-9]|[12][0-9]|3[01]` at `pos`.
fn day_at(src: &[u32], pos: usize) -> Option<u8> {
    let (first, second) = (at(src, pos), at(src, pos + 1));
    let valid = (is_char(first, '0') && (u32::from(b'1')..=u32::from(b'9')).contains(&second))
        || ((is_char(first, '1') || is_char(first, '2')) && is_digit(second))
        || (is_char(first, '3') && (is_char(second, '0') || is_char(second, '1')));
    valid.then(|| (digit_value(first) * 10 + digit_value(second)) as u8)
}

/// `([Zz])|([+-])(HH):(MM)` at `pos`: the end and the offset.
fn offset_at(src: &[u32], pos: usize) -> Option<(usize, UtcOffset)> {
    let marker = at(src, pos);
    if is_char(marker, 'Z') || is_char(marker, 'z') {
        return Some((pos + 1, UtcOffset { total_minutes: 0 }));
    }
    if !is_char(marker, '+') && !is_char(marker, '-') {
        return None;
    }
    let hour = hour_at(src, pos + 1)?;
    if !is_char(at(src, pos + 3), ':') {
        return None;
    }
    let minute = minute_or_second_at(src, pos + 4)?;
    let magnitude = i16::from(hour) * 60 + i16::from(minute);
    let total_minutes = if is_char(marker, '-') {
        -magnitude
    } else {
        magnitude
    };
    Some((pos + 6, UtcOffset { total_minutes }))
}

/// `RE_DATETIME.match(src, pos)`: a date, optionally followed by a separator (`T`, `t` or a space), a time and an offset.
pub fn match_datetime(src: &[u32], pos: usize) -> Option<DateTimeMatch> {
    let year = four_digit_year_at(src, pos)?;
    if !is_char(at(src, pos + 4), '-') {
        return None;
    }
    let month = month_at(src, pos + 5)?;
    if !is_char(at(src, pos + 7), '-') {
        return None;
    }
    let day = day_at(src, pos + 8)?;
    let mut matched = DateTimeMatch {
        end: pos + 10,
        year,
        month,
        day,
        time: None,
        offset: None,
    };
    let separator = at(src, pos + 10);
    if is_char(separator, 'T') || is_char(separator, 't') || is_char(separator, ' ') {
        if let Some(time) = match_time_text(src, pos + 11) {
            matched.end = time.end;
            matched.time = Some(time.time);
            if let Some((end, offset)) = offset_at(src, time.end) {
                matched.end = end;
                matched.offset = Some(offset);
            }
        }
    }
    Some(matched)
}

/// `match_to_datetime`: the date or date-time value, or the `ValueError` of the `date`/`datetime` constructor.
pub fn datetime_value(matched: &DateTimeMatch) -> Result<TomlValue, Raised> {
    let date = CivilDate::validated(matched.year, matched.month, matched.day)
        .map_err(|message| Raised::plain(ExceptionClass::ValueError, message))?;
    Ok(match matched.time {
        None => TomlValue::Date(date),
        Some(time) => TomlValue::DateTime(CivilDateTime {
            date,
            time,
            offset: matched.offset,
        }),
    })
}
