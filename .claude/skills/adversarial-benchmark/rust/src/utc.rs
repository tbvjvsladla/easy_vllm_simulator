//! UTC instants and the `--now-utc` / `started_utc` parser.
//!
//! The original calls `datetime.fromisoformat` (CPython 3.12 C implementation) and then
//! `astimezone(timezone.utc)`. Its accepted grammar is wider than ISO 8601 profiles in common libraries
//! (any single separator character, `YYYYMMDD`, ISO week dates, `,` fractions, sub-minute offsets), so the C
//! parsing routines are ported one for one from CPython 3.12.3 `Modules/_datetimemodule.c`
//! (<https://github.com/python/cpython/blob/v3.12.3/Modules/_datetimemodule.c>, PSF License; see
//! `notices/NOTICE.md`). A C string is modelled as bytes followed by an implicit NUL.

use crate::error::SweepError;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;

const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;
const DAYS_IN_MONTH: [i64; 13] = [-1, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MAX_YEAR: i64 = 9999;

/// A UTC instant with microsecond precision between 0001-01-01 and 9999-12-31, counted from 0001-01-01T00:00:00.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtcInstant {
    micros_since_origin: i64,
}

impl UtcInstant {
    fn from_micros(micros_since_origin: i64) -> Option<Self> {
        (0..days_before_year(MAX_YEAR + 1) * MICROS_PER_DAY)
            .contains(&micros_since_origin)
            .then_some(UtcInstant {
                micros_since_origin,
            })
    }

    /// `int((self - earlier).total_seconds())`: float division, then truncation toward zero.
    pub fn whole_seconds_since(&self, earlier: &UtcInstant) -> i64 {
        let micros = self.micros_since_origin - earlier.micros_since_origin;
        total_seconds(micros).trunc() as i64
    }

    /// `strftime("%Y-%m-%dT%H:%M:%SZ")` on Linux, where `%Y` is not zero padded.
    pub fn evaluated_text(&self) -> String {
        let CivilTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            ..
        } = self.civil_time();
        format!("{year}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }

    /// `datetime.isoformat()` of the UTC value (`+00:00` suffix), used by the parser checks.
    pub fn iso_text(&self) -> String {
        let CivilTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            microsecond,
        } = self.civil_time();
        let fraction = if microsecond == 0 {
            String::new()
        } else {
            format!(".{microsecond:06}")
        };
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{fraction}+00:00")
    }

    fn civil_time(&self) -> CivilTime {
        let days = self.micros_since_origin.div_euclid(MICROS_PER_DAY);
        let in_day = self.micros_since_origin.rem_euclid(MICROS_PER_DAY);
        let (year, month, day) = ordinal_to_ymd(days + 1);
        let seconds = in_day / MICROS_PER_SECOND;
        CivilTime {
            year,
            month,
            day,
            hour: seconds / 3600,
            minute: seconds / 60 % 60,
            second: seconds % 60,
            microsecond: in_day % MICROS_PER_SECOND,
        }
    }
}

struct CivilTime {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    microsecond: i64,
}

/// `timedelta.total_seconds()`: the microsecond count divided by 10**6, correctly rounded.
fn total_seconds(micros: i64) -> f64 {
    const EXACT_LIMIT: i64 = 1 << 53;
    if micros.abs() < EXACT_LIMIT {
        return micros as f64 / MICROS_PER_SECOND as f64;
    }
    let sign = if micros < 0 { "-" } else { "" };
    let magnitude = micros.unsigned_abs();
    format!(
        "{sign}{}.{:06}",
        magnitude / 1_000_000,
        magnitude % 1_000_000
    )
    .parse()
    .expect("a decimal literal is a valid f64")
}

/// Judges one timestamp field the way the original `_parse_utc` does.
///
/// `field` names the input in messages (`--now-utc`, `started_utc`).
pub fn parse_utc(value: &PyValue, field: &str) -> Result<UtcInstant, SweepError> {
    let not_a_timestamp = || {
        SweepError::stop_eval(format!(
            "{field} 는 ISO-8601 UTC 문자열이어야 한다(받은 값: {})",
            value.repr()
        ))
    };
    let PyValue::Str(text) = value else {
        return Err(not_a_timestamp());
    };
    let mut stripped = text.strip();
    if stripped.is_empty() {
        return Err(not_a_timestamp());
    }
    if stripped.ends_with_code_point('Z' as u32) {
        let mut points = stripped.code_points().to_vec();
        points.pop();
        points.extend("+00:00".chars().map(u32::from));
        stripped = PyStr::from_code_points(points);
    }
    let parsed = parse_iso_datetime(&stripped).map_err(|reason| {
        SweepError::stop_eval(format!(
            "{field} 를 시각으로 읽을 수 없다({}): {reason}",
            value.repr()
        ))
    })?;
    let Some(offset_micros) = parsed.offset_micros else {
        return Err(SweepError::stop_eval(format!(
            "{field} 에 타임존이 없다({}) — 벽시계 추정 금지",
            value.repr()
        )));
    };
    let local = parsed.local_micros();
    UtcInstant::from_micros(local - offset_micros)
        .ok_or_else(|| SweepError::Overflow("date value out of range".to_string()))
}

/// Result of `datetime.fromisoformat`: wall-clock fields plus the UTC offset when the text carried one.
struct ParsedDateTime {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    microsecond: i64,
    offset_micros: Option<i64>,
}

impl ParsedDateTime {
    fn local_micros(&self) -> i64 {
        let days = ymd_to_ordinal(self.year, self.month, self.day) - 1;
        days * MICROS_PER_DAY
            + (self.hour * 3600 + self.minute * 60 + self.second) * MICROS_PER_SECOND
            + self.microsecond
    }
}

/// `datetime.fromisoformat(text)`; the error is `str(ValueError)`.
fn parse_iso_datetime(text: &PyStr) -> Result<ParsedDateTime, String> {
    let invalid = || format!("Invalid isoformat string: {}", text.repr());
    // CPython rejects anything shorter than 7 characters, then lets one surrogate stand as the date/time
    // separator by replacing it with `T`: the separator is at character 7, 8 or 10 depending on the format, and
    // only the first surrogate found there is replaced. Any other lone surrogate leaves the text unencodable.
    if text.len() < 7 {
        return Err(invalid());
    }
    let mut cleaned = text.clone();
    if let Some(position) = [7, 8, 10]
        .into_iter()
        .take_while(|&position| position <= text.len())
        .find(|&position| {
            text.code_points()
                .get(position)
                .is_some_and(|&c| (0xD800..=0xDFFF).contains(&c))
        })
    {
        cleaned = PyStr::from_code_points(
            text.code_points()
                .iter()
                .enumerate()
                .map(|(index, &c)| if index == position { u32::from('T') } else { c })
                .collect(),
        );
    }
    let Some(utf8) = cleaned.to_utf8() else {
        return Err(invalid());
    };
    let bytes = utf8.as_bytes();
    let separator = find_datetime_separator(bytes).ok_or_else(invalid)?;
    let (year, month, day) = parse_date(bytes, separator).map_err(|_| invalid())?;

    let mut time = TimeParts::default();
    let mut has_offset = false;
    if bytes.len() > separator {
        let rest = separator + utf8_char_width(bytes[separator]);
        let (parts, offset_found) =
            parse_time(&bytes[rest.min(bytes.len())..]).map_err(|_| invalid())?;
        time = parts;
        has_offset = offset_found;
    }

    let offset_micros = if has_offset {
        Some(timezone_offset_micros(&time)?)
    } else {
        None
    };
    check_date_args(year, month, day)?;
    check_time_args(&time)?;
    Ok(ParsedDateTime {
        year,
        month,
        day,
        hour: time.hour,
        minute: time.minute,
        second: time.second,
        microsecond: time.microsecond,
        offset_micros,
    })
}

#[derive(Default)]
struct TimeParts {
    hour: i64,
    minute: i64,
    second: i64,
    microsecond: i64,
    offset_seconds: i64,
    offset_micros: i64,
}

/// Bytes of the UTF-8 sequence starting with `lead`, as the C code derives it from the lead byte.
fn utf8_char_width(lead: u8) -> usize {
    if lead & 0x80 == 0 {
        1
    } else {
        match lead & 0xF0 {
            0xE0 => 3,
            0xF0 => 4,
            _ => 2,
        }
    }
}

/// Byte at `index`, or NUL past the end (the C string terminator).
fn at(bytes: &[u8], index: usize) -> u8 {
    bytes.get(index).copied().unwrap_or(0)
}

fn is_digit(byte: u8) -> bool {
    byte.is_ascii_digit()
}

/// `parse_digits`: reads exactly `count` digits at `start`, returning the value and the next index.
fn parse_digits(bytes: &[u8], start: usize, count: usize) -> Option<(i64, usize)> {
    let mut value = 0;
    for offset in 0..count {
        let byte = at(bytes, start + offset);
        if !is_digit(byte) {
            return None;
        }
        value = value * 10 + i64::from(byte - b'0');
    }
    Some((value, start + count))
}

/// `_find_isoformat_datetime_separator`: the byte index where the date part ends, `None` for malformed input.
fn find_datetime_separator(bytes: &[u8]) -> Option<usize> {
    let len = bytes.len();
    if len == 7 {
        return Some(7);
    }
    if at(bytes, 4) == b'-' {
        if at(bytes, 5) == b'W' {
            if len < 8 {
                return None;
            }
            if len > 8 && at(bytes, 8) == b'-' {
                if len == 9 {
                    return None;
                }
                if len > 10 && is_digit(at(bytes, 10)) {
                    return Some(8);
                }
                return Some(10);
            }
            return Some(8);
        }
        return Some(10);
    }
    if at(bytes, 4) == b'W' {
        let mut index = 7;
        while index < len && is_digit(bytes[index]) {
            index += 1;
        }
        if index < 9 {
            return Some(index);
        }
        return Some(if index % 2 == 0 { 7 } else { 8 });
    }
    Some(8)
}

/// `parse_isoformat_date`: `YYYY-MM-DD`, `YYYYMMDD`, `YYYY-Www[-D]` and `YYYYWww[D]`.
fn parse_date(bytes: &[u8], date_len: usize) -> Result<(i64, i64, i64), ()> {
    let (year, mut cursor) = parse_digits(bytes, 0, 4).ok_or(())?;
    let uses_separator = at(bytes, cursor) == b'-';
    if uses_separator {
        cursor += 1;
    }
    if at(bytes, cursor) == b'W' {
        cursor += 1;
        let (week, after_week) = parse_digits(bytes, cursor, 2).ok_or(())?;
        cursor = after_week;
        let weekday = if cursor < date_len {
            if uses_separator {
                let separator = at(bytes, cursor);
                cursor += 1;
                if separator != b'-' {
                    return Err(());
                }
            }
            parse_digits(bytes, cursor, 1).ok_or(())?.0
        } else {
            1
        };
        return iso_week_to_ymd(year, week, weekday);
    }
    let (month, after_month) = parse_digits(bytes, cursor, 2).ok_or(())?;
    cursor = after_month;
    if uses_separator {
        let separator = at(bytes, cursor);
        cursor += 1;
        if separator != b'-' {
            return Err(());
        }
    }
    let (day, _) = parse_digits(bytes, cursor, 2).ok_or(())?;
    Ok((year, month, day))
}

fn iso_week_to_ymd(iso_year: i64, week: i64, weekday: i64) -> Result<(i64, i64, i64), ()> {
    if !(1..=MAX_YEAR).contains(&iso_year) {
        return Err(());
    }
    if !(1..=52).contains(&week) {
        let first_weekday = (ymd_to_ordinal(iso_year, 1, 1) + 6) % 7;
        let has_week_53 = first_weekday == 3 || (first_weekday == 2 && is_leap_year(iso_year));
        if week != 53 || !has_week_53 {
            return Err(());
        }
    }
    if !(1..=7).contains(&weekday) {
        return Err(());
    }
    let first_day = ymd_to_ordinal(iso_year, 1, 1);
    let first_weekday = (first_day + 6) % 7;
    let mut week_one_monday = first_day - first_weekday;
    if first_weekday > 3 {
        week_one_monday += 7;
    }
    Ok(ordinal_to_ymd(
        week_one_monday + (week - 1) * 7 + weekday - 1,
    ))
}

/// `parse_isoformat_time` for the text after the separator. Returns the parts and whether an offset was present.
fn parse_time(bytes: &[u8]) -> Result<(TimeParts, bool), ()> {
    let len = bytes.len();
    let mut zone_start = 0;
    loop {
        if matches!(at(bytes, zone_start), b'Z' | b'+' | b'-') {
            break;
        }
        zone_start += 1;
        if zone_start >= len {
            break;
        }
    }

    let mut parts = TimeParts::default();
    let (hour, minute, second, microsecond, trailing) = parse_hh_mm_ss_ff(bytes, 0, zone_start)?;
    parts.hour = hour;
    parts.minute = minute;
    parts.second = second;
    parts.microsecond = microsecond;

    if zone_start == len {
        return if trailing {
            Err(())
        } else {
            Ok((parts, false))
        };
    }
    if at(bytes, zone_start) == b'Z' {
        return if at(bytes, zone_start + 1) != 0 {
            Err(())
        } else {
            Ok((parts, true))
        };
    }
    let sign: i64 = if at(bytes, zone_start) == b'-' { -1 } else { 1 };
    let (zone_hour, zone_minute, zone_second, zone_micro, zone_trailing) =
        parse_hh_mm_ss_ff(bytes, zone_start + 1, len)?;
    parts.offset_seconds = sign * (zone_hour * 3600 + zone_minute * 60 + zone_second);
    parts.offset_micros = sign * zone_micro;
    if zone_trailing {
        Err(())
    } else {
        Ok((parts, true))
    }
}

/// `parse_hh_mm_ss_ff` over `bytes[start..end]` (the byte at `end` may be read, as in C).
///
/// Returns `(hour, minute, second, microsecond, trailing)` where `trailing` is the C return value 1
/// ("not at the end of the string").
fn parse_hh_mm_ss_ff(
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<(i64, i64, i64, i64, bool), ()> {
    const FRACTION_CORRECTION: [i64; 5] = [100_000, 10_000, 1_000, 100, 10];
    let mut values = [0_i64; 3];
    let mut cursor = start;
    let mut has_separator = true;

    for index in 0..3 {
        let (value, after) = parse_digits(bytes, cursor, 2).ok_or(())?;
        values[index] = value;
        let separator = at(bytes, after);
        cursor = after + 1;
        if index == 0 {
            has_separator = separator == b':';
        }
        if cursor >= end {
            return Ok((values[0], values[1], values[2], 0, separator != 0));
        } else if has_separator && separator == b':' {
            continue;
        } else if separator == b'.' || separator == b',' {
            break;
        } else if !has_separator {
            cursor -= 1;
        } else {
            return Err(());
        }
    }

    let remaining = end.saturating_sub(cursor);
    let to_parse = remaining.min(6);
    let (mut microsecond, mut after) = parse_digits(bytes, cursor, to_parse).ok_or(())?;
    if to_parse < 6 {
        microsecond *= FRACTION_CORRECTION[to_parse - 1];
    }
    while is_digit(at(bytes, after)) {
        after += 1;
    }
    Ok((
        values[0],
        values[1],
        values[2],
        microsecond,
        at(bytes, after) != 0,
    ))
}

/// `tzinfo_from_isoformat_results`: a zero offset is UTC, anything else must lie strictly inside +-24h.
fn timezone_offset_micros(time: &TimeParts) -> Result<i64, String> {
    if time.offset_seconds == 0 {
        return Ok(0);
    }
    let micros = time.offset_seconds * MICROS_PER_SECOND + time.offset_micros;
    if micros.abs() >= MICROS_PER_DAY {
        return Err(format!(
            "offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not {}.",
            timedelta_repr(micros)
        ));
    }
    Ok(micros)
}

/// `repr(timedelta)` for a normalised microsecond count.
fn timedelta_repr(micros: i64) -> String {
    let days = micros.div_euclid(MICROS_PER_DAY);
    let remainder = micros.rem_euclid(MICROS_PER_DAY);
    let seconds = remainder / MICROS_PER_SECOND;
    let microseconds = remainder % MICROS_PER_SECOND;
    let mut fields = Vec::new();
    if days != 0 {
        fields.push(format!("days={days}"));
    }
    if seconds != 0 {
        fields.push(format!("seconds={seconds}"));
    }
    if microseconds != 0 {
        fields.push(format!("microseconds={microseconds}"));
    }
    if fields.is_empty() {
        "datetime.timedelta(0)".to_string()
    } else {
        format!("datetime.timedelta({})", fields.join(", "))
    }
}

fn check_date_args(year: i64, month: i64, day: i64) -> Result<(), String> {
    if !(1..=MAX_YEAR).contains(&year) {
        return Err(format!("year {year} is out of range"));
    }
    if !(1..=12).contains(&month) {
        return Err("month must be in 1..12".to_string());
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err("day is out of range for month".to_string());
    }
    Ok(())
}

fn check_time_args(time: &TimeParts) -> Result<(), String> {
    if !(0..=23).contains(&time.hour) {
        return Err("hour must be in 0..23".to_string());
    }
    if !(0..=59).contains(&time.minute) {
        return Err("minute must be in 0..59".to_string());
    }
    if !(0..=59).contains(&time.second) {
        return Err("second must be in 0..59".to_string());
    }
    if !(0..=999_999).contains(&time.microsecond) {
        return Err("microsecond must be in 0..999999".to_string());
    }
    Ok(())
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    if month == 2 && is_leap_year(year) {
        29
    } else {
        DAYS_IN_MONTH[month as usize]
    }
}

fn days_before_year(year: i64) -> i64 {
    let previous = year - 1;
    previous * 365 + previous / 4 - previous / 100 + previous / 400
}

fn days_before_month(year: i64, month: i64) -> i64 {
    (1..month).map(|m| days_in_month(year, m)).sum()
}

/// Proleptic Gregorian ordinal where 0001-01-01 is day 1.
fn ymd_to_ordinal(year: i64, month: i64, day: i64) -> i64 {
    days_before_year(year) + days_before_month(year, month) + day
}

fn ordinal_to_ymd(ordinal: i64) -> (i64, i64, i64) {
    let days_400y = days_before_year(401);
    let days_100y = days_before_year(101);
    let days_4y = days_before_year(5);
    let mut n = ordinal - 1;
    let (n400, rest) = (n.div_euclid(days_400y), n.rem_euclid(days_400y));
    n = rest;
    let mut year = n400 * 400 + 1;
    let (n100, rest) = (n / days_100y, n % days_100y);
    n = rest;
    let (n4, rest) = (n / days_4y, n % days_4y);
    n = rest;
    let (n1, rest) = (n / 365, n % 365);
    n = rest;
    year += n100 * 100 + n4 * 4 + n1;
    if n1 == 4 || n100 == 4 {
        return (year - 1, 12, 31);
    }
    let leap = n1 == 3 && (n4 != 24 || n100 == 3);
    let mut month = (n + 50) >> 5;
    let mut preceding = (1..month)
        .map(|m| DAYS_IN_MONTH[m as usize] + i64::from(m == 2 && leap))
        .sum::<i64>();
    if preceding > n {
        month -= 1;
        preceding -= DAYS_IN_MONTH[month as usize] + i64::from(month == 2 && leap);
    }
    (year, month, n - preceding + 1)
}

/// Why a timestamp text did not become an instant, for callers that word their own errors (`classify_cell._utc`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstantFailure {
    /// Not a `str`, or nothing but whitespace.
    NotText,
    /// `datetime.fromisoformat` raised `ValueError` with this `str(exception)`.
    Unreadable(String),
    /// A well-formed text without a UTC offset (a naive `datetime`).
    NoTimezone,
    /// `astimezone(timezone.utc)` left years 1..9999 (`OverflowError: date value out of range`).
    OutOfRange,
}

/// `datetime.fromisoformat(text.strip()[:-1] + "+00:00" if it ends with "Z" else ...).astimezone(timezone.utc)`
/// with the failure kinds kept apart. This is the same parse as [`parse_utc`] (same grammar, same `ValueError` text);
/// only the wording around it is the caller's.
pub fn parse_instant(value: &PyValue) -> Result<UtcInstant, InstantFailure> {
    let PyValue::Str(text) = value.peel() else {
        return Err(InstantFailure::NotText);
    };
    let mut stripped = text.strip();
    if stripped.is_empty() {
        return Err(InstantFailure::NotText);
    }
    if stripped.ends_with_code_point('Z' as u32) {
        let mut points = stripped.code_points().to_vec();
        points.pop();
        points.extend("+00:00".chars().map(u32::from));
        stripped = PyStr::from_code_points(points);
    }
    let parsed = parse_iso_datetime(&stripped).map_err(InstantFailure::Unreadable)?;
    let Some(offset_micros) = parsed.offset_micros else {
        return Err(InstantFailure::NoTimezone);
    };
    UtcInstant::from_micros(parsed.local_micros() - offset_micros).ok_or(InstantFailure::OutOfRange)
}

impl UtcInstant {
    /// `datetime.datetime.now(datetime.timezone.utc)` read from the system clock now, with microsecond precision.
    pub fn now() -> UtcInstant {
        let since_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_micros()).unwrap_or(i64::MAX)
            });
        let unix_origin = (ymd_to_ordinal(1970, 1, 1) - 1) * MICROS_PER_DAY;
        UtcInstant::from_micros(unix_origin + since_unix)
            .expect("the system clock reads a year between 1 and 9999")
    }
}

impl UtcInstant {
    /// `self + timedelta(microseconds=delta_micros)`; `None` when the result leaves years 1..9999
    /// (`OverflowError: date value out of range`).
    pub fn shifted_by_micros(&self, delta_micros: i128) -> Option<UtcInstant> {
        let moved = i128::from(self.micros_since_origin).checked_add(delta_micros)?;
        UtcInstant::from_micros(i64::try_from(moved).ok()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(text: &str) -> Result<UtcInstant, SweepError> {
        parse_utc(&PyValue::str(text), "--now-utc")
    }

    #[test]
    fn calendar_round_trip_covers_leap_rules() {
        for (y, m, d) in [
            (1, 1, 1),
            (1600, 2, 29),
            (1900, 3, 1),
            (2000, 12, 31),
            (2024, 2, 29),
            (9999, 12, 31),
        ] {
            assert_eq!(ordinal_to_ymd(ymd_to_ordinal(y, m, d)), (y, m, d));
        }
        assert_eq!(ymd_to_ordinal(1, 1, 1), 1);
    }

    #[test]
    fn parses_the_forms_the_harness_writes_and_python_also_accepts() {
        let expected = "2026-09-04T09:30:00+00:00";
        for text in [
            "2026-09-04T09:30:00Z",
            "2026-09-04T18:30:00+09:00",
            "20260904T093000+0000",
            "2026-W36-5T09:30:00Z",
        ] {
            assert_eq!(utc(text).unwrap().iso_text(), expected, "{text}");
        }
        assert_eq!(
            utc("2026-09-04X09:30:00,5Z").unwrap().iso_text(),
            "2026-09-04T09:30:00.500000+00:00"
        );
    }

    #[test]
    fn rejects_like_python() {
        assert_eq!(
            utc("2026-09-04T24:00:00Z").unwrap_err().message(),
            "--now-utc 를 시각으로 읽을 수 없다('2026-09-04T24:00:00Z'): hour must be in 0..23"
        );
        assert!(utc("2026-09-04T09:30:00z")
            .unwrap_err()
            .message()
            .contains("Invalid isoformat string: '2026-09-04T09:30:00z'"));
        assert!(utc("2026-09-04")
            .unwrap_err()
            .message()
            .contains("타임존이 없다"));
        assert_eq!(
            utc("0001-01-01T00:00:00+01:00").unwrap_err(),
            SweepError::Overflow("date value out of range".into())
        );
    }

    #[test]
    fn elapsed_truncates_toward_zero() {
        let start = utc("2026-09-04T09:00:00Z").unwrap();
        let slightly_before = utc("2026-09-04T08:59:59.900000Z").unwrap();
        assert_eq!(slightly_before.whole_seconds_since(&start), 0);
        assert_eq!(
            utc("2026-09-04T08:59:59Z")
                .unwrap()
                .whole_seconds_since(&start),
            -1
        );
        assert_eq!(
            utc("2026-09-04T09:30:00.999999Z")
                .unwrap()
                .whole_seconds_since(&start),
            1800
        );
    }
}
