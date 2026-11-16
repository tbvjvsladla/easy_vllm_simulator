//! `harness_verify.observe_as_of` (the caller-side reference date of a policy audit, observed once and recorded), as
//! the original reached it through `_clock_owner()`. Pure apart from the one clock read when no instant is injected.
use crate::pytext::py_repr;
use std::time::{SystemTime, UNIX_EPOCH};

/// One UTC instant: civil date-time + microseconds (what `datetime.datetime.now(timezone.utc)` holds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtcInstant {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub microsecond: u32,
}

impl UtcInstant {
    pub fn now() -> Self {
        let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        Self::from_unix(d.as_secs() as i64, d.subsec_micros())
    }
    pub fn from_unix(secs: i64, micros: u32) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        UtcInstant {
            year,
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: (rem % 3600 / 60) as u32,
            second: (rem % 60) as u32,
            microsecond: micros,
        }
    }
    pub fn date_iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    /// `datetime.isoformat()` of an aware UTC datetime (microseconds only when non-zero).
    pub fn isoformat(&self) -> String {
        let frac = if self.microsecond != 0 { format!(".{:06}", self.microsecond) } else { String::new() };
        format!("{}T{:02}:{:02}:{:02}{frac}+00:00", self.date_iso(), self.hour, self.minute, self.second)
    }
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The reference-date record (`as_of`, `as_of_source`, `observed_utc`, `repro_as_of`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsOfRecord {
    pub as_of: String,
    pub as_of_source: &'static str,
    pub observed_utc: Option<String>,
    pub repro_as_of: String,
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// `datetime.date.fromisoformat` of a string already matching `\d{4}-\d{2}-\d{2}` (ValueError text on failure).
fn validate_iso_date(text: &str) -> Result<(), String> {
    if !text.is_ascii() {
        return Err(format!("Invalid isoformat string: {}", py_repr(text)));
    }
    let year: i64 = text[0..4].parse().unwrap();
    let month: u32 = text[5..7].parse().unwrap();
    let day: u32 = text[8..10].parse().unwrap();
    if year < 1 {
        return Err(format!("year {year} is out of range"));
    }
    if !(1..=12).contains(&month) {
        return Err("month must be in 1..12".into());
    }
    let dim = match month {
        2 => if is_leap(year) { 29 } else { 28 },
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day < 1 || day > dim {
        return Err("day is out of range for month".into());
    }
    Ok(())
}

/// `re.fullmatch(r"\d{4}-\d{2}-\d{2}", s)` with Python's Unicode `\d`.
fn shape_ok(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let digit = |c: char| agent_runtime_support::pystr::is_decimal_digit(c as u32);
    chars.len() == 10
        && chars.iter().enumerate().all(|(i, &c)| if i == 4 || i == 7 { c == '-' } else { digit(c) })
}

/// `observe_as_of(explicit, now_utc)`: Err = the ValueError message.
pub fn observe_as_of(explicit: Option<&str>, now_utc: Option<UtcInstant>) -> Result<AsOfRecord, String> {
    if let Some(explicit) = explicit {
        if !shape_ok(explicit) {
            return Err(format!("--as-of {} is not a YYYY-MM-DD date", py_repr(explicit)));
        }
        validate_iso_date(explicit)?;
        return Ok(AsOfRecord {
            as_of: explicit.to_string(),
            as_of_source: "explicit-caller-input",
            observed_utc: None,
            repro_as_of: explicit.to_string(),
        });
    }
    let observed = now_utc.unwrap_or_else(UtcInstant::now);
    let as_of = observed.date_iso();
    Ok(AsOfRecord {
        as_of: as_of.clone(),
        as_of_source: "observed-utc-clock",
        observed_utc: Some(observed.isoformat()),
        repro_as_of: as_of,
    })
}
