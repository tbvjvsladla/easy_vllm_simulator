//! `datetime.date` as the original uses it: CPython 3.12's C `date.fromisoformat` (lengths 7, 8 and 10 only; the
//! `YYYY-MM-DD`, `YYYYMMDD`, `YYYY-Www[-D]`, `YYYYWww[D]` forms; ASCII digits; range checks of `date()`), `isoformat()`,
//! ordering and day arithmetic. No clock is read here.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Days since 0001-01-01 (proleptic Gregorian, `date.toordinal() - 1`).
fn ordinal(d: Date) -> i64 {
    let y = (d.year - 1) as i64;
    let mut days = y * 365 + y / 4 - y / 100 + y / 400;
    for m in 1..d.month {
        days += days_in_month(d.year, m) as i64;
    }
    days + (d.day as i64 - 1)
}

fn from_ordinal(mut n: i64) -> Option<Date> {
    if n < 0 {
        return None;
    }
    // 400-year cycles have 146097 days
    let mut year: i64 = 1 + 400 * (n / 146097);
    n %= 146097;
    loop {
        let len = if is_leap(year as i32) { 366 } else { 365 };
        if n < len {
            break;
        }
        n -= len;
        year += 1;
    }
    if year > 9999 {
        return None;
    }
    let mut month = 1;
    loop {
        let dim = days_in_month(year as i32, month) as i64;
        if n < dim {
            break;
        }
        n -= dim;
        month += 1;
    }
    Some(Date { year: year as i32, month, day: n as u32 + 1 })
}

impl Date {
    pub fn new(year: i32, month: u32, day: u32) -> Option<Date> {
        if !(1..=9999).contains(&year) || !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some(Date { year, month, day })
    }

    pub fn isoformat(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `self + timedelta(days=n)`; `None` = OverflowError.
    pub fn add_days(&self, n: i64) -> Option<Date> {
        from_ordinal(ordinal(*self) + n)
    }

    /// `(self - other).days`
    pub fn days_since(&self, other: &Date) -> i64 {
        ordinal(*self) - ordinal(*other)
    }

    /// `date.fromisoformat(text)` (C implementation): `None` = ValueError.
    pub fn fromisoformat(text: &str) -> Option<Date> {
        let b = text.as_bytes();
        let len = b.len();
        if !(len == 7 || len == 8 || len == 10) {
            return None;
        }
        let digits = |pos: usize, n: usize| -> Option<i32> {
            let mut v = 0i32;
            for i in 0..n {
                let c = *b.get(pos + i)?;
                if !c.is_ascii_digit() {
                    return None;
                }
                v = v * 10 + (c - b'0') as i32;
            }
            Some(v)
        };
        let year = digits(0, 4)?;
        let mut p = 4;
        let sep = b.get(p) == Some(&b'-');
        if sep {
            p += 1;
        }
        if b.get(p) == Some(&b'W') {
            p += 1;
            let week = digits(p, 2)?;
            p += 2;
            let day = if p < len {
                if sep {
                    if b.get(p) != Some(&b'-') {
                        return None;
                    }
                    p += 1;
                }
                digits(p, 1)?
            } else {
                1
            };
            return iso_to_ymd(year, week, day);
        }
        let month = digits(p, 2)?;
        p += 2;
        if sep {
            if b.get(p) != Some(&b'-') {
                return None;
            }
            p += 1;
        }
        let day = digits(p, 2)?;
        if month < 1 || day < 1 {
            return None;
        }
        Date::new(year, month as u32, day as u32)
    }
}

fn iso_to_ymd(year: i32, week: i32, day: i32) -> Option<Date> {
    if !(1..=9999).contains(&year) {
        return None;
    }
    if week <= 0 || week >= 53 {
        let mut out_of_range = true;
        if week == 53 {
            // ISO years have 53 weeks if Jan 1 is a Thursday, or a Wednesday in a leap year
            let jan1 = Date { year, month: 1, day: 1 };
            let first_weekday = (ordinal(jan1) % 7) as i32; // 0 = Monday
            if first_weekday == 3 || (first_weekday == 2 && is_leap(year)) {
                out_of_range = false;
            }
        }
        if out_of_range {
            return None;
        }
    }
    if day <= 0 || day >= 8 {
        return None;
    }
    let jan1 = Date { year, month: 1, day: 1 };
    let first_weekday = ordinal(jan1) % 7; // Monday = 0
    // iso_week1_monday
    let mut week1_monday = ordinal(jan1) - first_weekday;
    if first_weekday > 3 {
        week1_monday += 7;
    }
    let ord = week1_monday + (week as i64 - 1) * 7 + (day as i64 - 1);
    let d = from_ordinal(ord)?;
    Some(d)
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn basics() {
        assert_eq!(Date::fromisoformat("2026-01-15").unwrap().isoformat(), "2026-01-15");
        assert_eq!(Date::fromisoformat("20260115").unwrap().isoformat(), "2026-01-15");
        assert_eq!(Date::fromisoformat("2026-W01-1").unwrap().isoformat(), "2025-12-29");
        assert_eq!(Date::fromisoformat("2026W01").unwrap().isoformat(), "2025-12-29");
        assert!(Date::fromisoformat("2026-02-30").is_none());
        assert!(Date::fromisoformat("never").is_none());
        let d = Date::fromisoformat("2026-01-15").unwrap();
        assert_eq!(d.add_days(90).unwrap().isoformat(), "2026-04-15");
        assert_eq!(Date::fromisoformat("2026-04-15").unwrap().days_since(&d), 90);
    }
}
