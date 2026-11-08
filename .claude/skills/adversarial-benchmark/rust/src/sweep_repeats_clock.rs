//! The one time calculation of `selftest_sweep_repeats` (case M1): the end of the window the repeat stop recorded is
//! read with `datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ")`, moved by a number of seconds and written back with
//! `strftime` of the same format.
//!
//! The reader follows the way Python's `_strptime` builds its pattern for that format (compiled ignoring case): each
//! numeric field takes the first alternative of its regular expression that lets the rest of the pattern match, so a
//! one-digit month, day, hour, minute or second is accepted, and a text that does not match, or has data after the
//! trailing `Z`, is a `ValueError` with Python's message. Only ASCII digits are read (the deployed repeat axis writes
//! the compact UTC form; the Unicode decimal digits a `str` pattern would also accept do not occur in it). The
//! writer leaves the year unpadded, as the C library's `%Y` does.

use crate::conformance::Raised;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;

const FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
const SECONDS_PER_DAY: i64 = 86_400;
/// The years `datetime` can hold.
const FIRST_YEAR: i64 = 1;
const LAST_YEAR: i64 = 9999;

/// A UTC time with whole seconds, as the verifier needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WholeSecondTime {
    seconds_since_epoch: i64,
}

#[derive(Clone, Copy)]
enum Field {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

/// Lengths in which a field's pattern can match at `position`, in the order of its alternatives.
fn alternatives(field: Field, text: &[char], position: usize) -> Vec<usize> {
    let at = |offset: usize| text.get(position + offset).copied();
    let digit = |offset: usize| at(offset).is_some_and(|c| c.is_ascii_digit());
    let between = |offset: usize, low: char, high: char| {
        at(offset).is_some_and(|c| (low..=high).contains(&c))
    };
    let mut found = Vec::new();
    let mut offer = |matched: bool, length: usize| {
        if matched {
            found.push(length);
        }
    };
    match field {
        Field::Year => offer((0..4).all(digit), 4),
        Field::Month => {
            offer(at(0) == Some('1') && between(1, '0', '2'), 2);
            offer(at(0) == Some('0') && between(1, '1', '9'), 2);
            offer(between(0, '1', '9'), 1);
        }
        Field::Day => {
            offer(at(0) == Some('3') && between(1, '0', '1'), 2);
            offer(between(0, '1', '2') && digit(1), 2);
            offer(at(0) == Some('0') && between(1, '1', '9'), 2);
            offer(between(0, '1', '9'), 1);
            offer(at(0) == Some(' ') && between(1, '1', '9'), 2);
        }
        Field::Hour => {
            offer(at(0) == Some('2') && between(1, '0', '3'), 2);
            offer(between(0, '0', '1') && digit(1), 2);
            offer(digit(0), 1);
        }
        Field::Minute => {
            offer(between(0, '0', '5') && digit(1), 2);
            offer(digit(0), 1);
        }
        Field::Second => {
            offer(at(0) == Some('6') && between(1, '0', '1'), 2);
            offer(between(0, '0', '5') && digit(1), 2);
            offer(digit(0), 1);
        }
    }
    found
}

/// What follows each field in the pattern: the literal (matched ignoring case).
const PATTERN: [(Field, char); 6] = [
    (Field::Year, '-'),
    (Field::Month, '-'),
    (Field::Day, 'T'),
    (Field::Hour, ':'),
    (Field::Minute, ':'),
    (Field::Second, 'Z'),
];

fn literal_matches(text: &[char], position: usize, literal: char) -> bool {
    text.get(position)
        .is_some_and(|c| c.to_ascii_uppercase() == literal)
}

/// The first complete match of the pattern: each field's number and the end position.
fn match_pattern(
    text: &[char],
    position: usize,
    field_index: usize,
    numbers: &mut Vec<i64>,
) -> Option<usize> {
    let Some((field, literal)) = PATTERN.get(field_index).copied() else {
        return Some(position);
    };
    for length in alternatives(field, text, position) {
        if !literal_matches(text, position + length, literal) {
            continue;
        }
        let digits: String = text[position..position + length].iter().collect();
        numbers.push(digits.trim().parse().expect("ASCII digits"));
        match match_pattern(text, position + length + 1, field_index + 1, numbers) {
            Some(end) => return Some(end),
            None => {
                numbers.pop();
            }
        }
    }
    None
}

fn value_error(message: &str) -> Raised {
    Raised::new("ValueError", message)
}

/// `datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")` for a loaded JSON value.
pub fn strptime(value: &PyValue) -> Result<WholeSecondTime, Raised> {
    let PyValue::Str(shown) = value.peel() else {
        return Err(Raised::new(
            "TypeError",
            &format!(
                "strptime() argument 1 must be str, not {}",
                value.type_name()
            ),
        ));
    };
    let characters: Vec<char> = shown.to_utf8().unwrap_or_default().chars().collect();
    let mut numbers = Vec::new();
    let Some(end) = match_pattern(&characters, 0, 0, &mut numbers) else {
        return Err(value_error(&format!(
            "time data {} does not match format {}",
            shown.repr(),
            PyStr::new(FORMAT).repr()
        )));
    };
    if end != characters.len() {
        let rest: String = characters[end..].iter().collect();
        return Err(value_error(&format!("unconverted data remains: {rest}")));
    }
    let [year, month, day, hour, minute, second] = numbers[..] else {
        unreachable!("a complete match has one number per field");
    };
    if year < FIRST_YEAR {
        return Err(value_error(&format!("year {year} is out of range")));
    }
    if !(1..=days_in_month(year, month)).contains(&day) {
        return Err(value_error("day is out of range for month"));
    }
    if second > 59 {
        return Err(value_error("second must be in 0..59"));
    }
    let days = days_from_civil(year, month, day);
    Ok(WholeSecondTime {
        seconds_since_epoch: days * SECONDS_PER_DAY + hour * 3600 + minute * 60 + second,
    })
}

impl WholeSecondTime {
    /// `self + timedelta(seconds=seconds)`; leaving years 1..9999 is an `OverflowError`.
    pub fn plus_seconds(self, seconds: i64) -> Result<WholeSecondTime, Raised> {
        let moved = WholeSecondTime {
            seconds_since_epoch: self.seconds_since_epoch + seconds,
        };
        let (year, ..) = civil_from_days(moved.seconds_since_epoch.div_euclid(SECONDS_PER_DAY));
        if (FIRST_YEAR..=LAST_YEAR).contains(&year) {
            Ok(moved)
        } else {
            Err(Raised::new("OverflowError", "date value out of range"))
        }
    }

    /// `strftime("%Y-%m-%dT%H:%M:%SZ")` (the year unpadded, like glibc).
    pub fn format(self) -> String {
        let days = self.seconds_since_epoch.div_euclid(SECONDS_PER_DAY);
        let in_day = self.seconds_since_epoch.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        format!(
            "{year}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            in_day / 3600,
            in_day % 3600 / 60,
            in_day % 60
        )
    }
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<WholeSecondTime, Raised> {
        strptime(&PyValue::str(text))
    }

    #[test]
    fn the_compact_utc_form_round_trips_and_moves_by_seconds() {
        let end = read("2026-09-01T00:00:30Z").expect("a time");
        assert_eq!(end.format(), "2026-09-01T00:00:30Z");
        assert_eq!(
            end.plus_seconds(10).expect("moved").format(),
            "2026-09-01T00:00:40Z"
        );
        assert_eq!(
            end.plus_seconds(120).expect("moved").format(),
            "2026-09-01T00:02:30Z"
        );
        assert_eq!(
            end.plus_seconds(-31).expect("moved").format(),
            "2026-08-31T23:59:59Z"
        );
        assert_eq!(
            read("2000-01-01T00:00:00Z")
                .expect("a time")
                .plus_seconds(1_000_000_000)
                .expect("moved")
                .format(),
            "2031-09-09T01:46:40Z"
        );
    }

    #[test]
    fn python_accepts_one_digit_fields_and_ignores_case() {
        assert_eq!(
            read("2026-9-1t1:2:3z").expect("a time").format(),
            "2026-09-01T01:02:03Z"
        );
        // the day field tries `3[0-1]` first, then `[1-2]\d`: "1T" cannot be two digits, so one digit is taken.
        assert_eq!(
            read("2026-09-1T00:00:00Z").expect("a time").format(),
            "2026-09-01T00:00:00Z"
        );
    }

    #[test]
    fn a_text_that_does_not_fit_is_the_value_error_python_raises() {
        let message = |text: &str| {
            read(text)
                .expect_err("refused")
                .failure()
                .message
                .to_utf8()
                .expect("text")
        };
        assert_eq!(
            message("not-a-time"),
            "time data 'not-a-time' does not match format '%Y-%m-%dT%H:%M:%SZ'"
        );
        assert_eq!(
            message("2026-09-01T00:00:00Z+00"),
            "unconverted data remains: +00"
        );
        assert_eq!(
            message("2026-02-30T00:00:00Z"),
            "day is out of range for month"
        );
        assert_eq!(message("2026-09-01T00:00:60Z"), "second must be in 0..59");
        assert_eq!(message("0000-09-01T00:00:00Z"), "year 0 is out of range");
        // hour 24 is no alternative of the hour field, so nothing matches
        assert_eq!(
            message("2026-09-01T24:00:00Z"),
            "time data '2026-09-01T24:00:00Z' does not match format '%Y-%m-%dT%H:%M:%SZ'"
        );
    }

    #[test]
    fn a_value_that_is_not_text_is_a_type_error_and_the_range_is_checked() {
        let refused = strptime(&PyValue::int(5)).expect_err("not text");
        assert_eq!(refused.failure().class, "TypeError");
        let edge = read("9999-12-31T23:59:59Z").expect("a time");
        assert_eq!(
            edge.plus_seconds(1).expect_err("overflow").failure().class,
            "OverflowError"
        );
        assert_eq!(
            read("0999-01-01T00:00:00Z").expect("a time").format(),
            "999-01-01T00:00:00Z"
        );
    }
}
