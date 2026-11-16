//! `datetime.strptime` for the directive subset the ported programs use (`%Y %m %d %H %M %S` and literals), with
//! CPython's `_strptime` regular expressions (Unicode `\d`, IGNORECASE literals, whole-string match, backtracking over
//! the directive alternatives), and the proleptic Gregorian arithmetic of `date`.

use agent_runtime_support::pystr::decimal_digit_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

fn dec(c: char) -> Option<u32> {
    decimal_digit_value(c as u32)
}
fn ascii(c: char, lo: char, hi: char) -> bool {
    c >= lo && c <= hi
}

/// Alternatives of one directive at `i`: (end, value).
fn alts(d: char, s: &[char], i: usize) -> Vec<(usize, u32)> {
    let g = |k: usize| s.get(i + k).copied();
    let mut v = vec![];
    match d {
        'Y' => {
            if (0..4).all(|k| g(k).and_then(dec).is_some()) {
                v.push((
                    i + 4,
                    (0..4).fold(0, |a, k| a * 10 + dec(g(k).unwrap()).unwrap()),
                ));
            }
        }
        'm' => {
            // 1[0-2]|0[1-9]|[1-9]
            if let (Some('1'), Some(b)) = (g(0), g(1)) {
                if ascii(b, '0', '2') {
                    v.push((i + 2, 10 + dec(b).unwrap()));
                }
            }
            if let (Some('0'), Some(b)) = (g(0), g(1)) {
                if ascii(b, '1', '9') {
                    v.push((i + 2, dec(b).unwrap()));
                }
            }
            if let Some(a) = g(0) {
                if ascii(a, '1', '9') {
                    v.push((i + 1, dec(a).unwrap()));
                }
            }
        }
        'd' => {
            // 3[01]|[12]\d|0[1-9]|[1-9]| [1-9]
            if let (Some('3'), Some(b)) = (g(0), g(1)) {
                if b == '0' || b == '1' {
                    v.push((i + 2, 30 + dec(b).unwrap()));
                }
            }
            if let (Some(a), Some(b)) = (g(0), g(1)) {
                if (a == '1' || a == '2') && dec(b).is_some() {
                    v.push((i + 2, dec(a).unwrap() * 10 + dec(b).unwrap()));
                }
            }
            if let (Some('0'), Some(b)) = (g(0), g(1)) {
                if ascii(b, '1', '9') {
                    v.push((i + 2, dec(b).unwrap()));
                }
            }
            if let Some(a) = g(0) {
                if ascii(a, '1', '9') {
                    v.push((i + 1, dec(a).unwrap()));
                }
            }
            if let (Some(' '), Some(b)) = (g(0), g(1)) {
                if ascii(b, '1', '9') {
                    v.push((i + 2, dec(b).unwrap()));
                }
            }
        }
        'H' => {
            // 2[0-3]|[0-1]\d|\d
            if let (Some('2'), Some(b)) = (g(0), g(1)) {
                if ascii(b, '0', '3') {
                    v.push((i + 2, 20 + dec(b).unwrap()));
                }
            }
            if let (Some(a), Some(b)) = (g(0), g(1)) {
                if (a == '0' || a == '1') && dec(b).is_some() {
                    v.push((i + 2, dec(a).unwrap() * 10 + dec(b).unwrap()));
                }
            }
            if let Some(a) = g(0).and_then(dec) {
                v.push((i + 1, a));
            }
        }
        'M' => {
            // [0-5]\d|\d
            if let (Some(a), Some(b)) = (g(0), g(1)) {
                if ascii(a, '0', '5') && dec(b).is_some() {
                    v.push((i + 2, dec(a).unwrap() * 10 + dec(b).unwrap()));
                }
            }
            if let Some(a) = g(0).and_then(dec) {
                v.push((i + 1, a));
            }
        }
        'S' => {
            // 6[0-1]|[0-5]\d|\d
            if let (Some('6'), Some(b)) = (g(0), g(1)) {
                if b == '0' || b == '1' {
                    v.push((i + 2, 60 + dec(b).unwrap()));
                }
            }
            if let (Some(a), Some(b)) = (g(0), g(1)) {
                if ascii(a, '0', '5') && dec(b).is_some() {
                    v.push((i + 2, dec(a).unwrap() * 10 + dec(b).unwrap()));
                }
            }
            if let Some(a) = g(0).and_then(dec) {
                v.push((i + 1, a));
            }
        }
        _ => {}
    }
    v
}

enum Tok {
    Dir(char),
    Lit(char),
}

fn tokens(fmt: &str) -> Vec<Tok> {
    let mut t = vec![];
    let mut it = fmt.chars();
    while let Some(c) = it.next() {
        if c == '%' {
            t.push(Tok::Dir(it.next().unwrap_or('%')));
        } else {
            t.push(Tok::Lit(c));
        }
    }
    t
}

fn lower(c: char) -> String {
    c.to_lowercase().collect()
}

/// CPython's `format_regex.match(data)`: the first successful path in priority order and the end it reached.
fn walk(toks: &[Tok], s: &[char], i: usize, vals: &mut Vec<(char, u32)>) -> Option<usize> {
    let Some(first) = toks.first() else {
        return Some(i);
    };
    match first {
        Tok::Lit(c) => {
            if s.get(i).is_some_and(|x| lower(*x) == lower(*c)) {
                walk(&toks[1..], s, i + 1, vals)
            } else {
                None
            }
        }
        Tok::Dir(d) => {
            for (end, v) in alts(*d, s, i) {
                vals.push((*d, v));
                if let Some(e) = walk(&toks[1..], s, end, vals) {
                    return Some(e);
                }
                vals.pop();
            }
            None
        }
    }
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// `datetime.strptime(text, fmt)`; `None` = ValueError.
pub fn strptime(text: &str, fmt: &str) -> Option<DateTime> {
    strptime_checked(text, fmt).ok()
}

/// `datetime.strptime(text, fmt)`; `Err` = the `ValueError` message CPython gives.
pub fn strptime_checked(text: &str, fmt: &str) -> Result<DateTime, String> {
    let s: Vec<char> = text.chars().collect();
    let mut vals = vec![];
    let repr = |t: &str| agent_runtime_support::pystr::PyStr::new(t).repr();
    let Some(end) = walk(&tokens(fmt), &s, 0, &mut vals) else {
        return Err(format!(
            "time data {} does not match format {}",
            repr(text),
            repr(fmt)
        ));
    };
    if end != s.len() {
        return Err(format!(
            "unconverted data remains: {}",
            s[end..].iter().collect::<String>()
        ));
    }
    let get = |d: char, def: u32| {
        vals.iter()
            .rev()
            .find(|(k, _)| *k == d)
            .map_or(def, |(_, v)| *v)
    };
    let dt = DateTime {
        year: get('Y', 1900) as i64,
        month: get('m', 1),
        day: get('d', 1),
        hour: get('H', 0),
        minute: get('M', 0),
        second: get('S', 0),
    };
    if dt.year < 1 {
        return Err(format!("year {} is out of range", dt.year));
    }
    if dt.day > days_in_month(dt.year, dt.month) {
        return Err("day is out of range for month".into());
    }
    if dt.second > 59 {
        return Err("second must be in 0..59".into());
    }
    Ok(dt)
}

/// The proleptic Gregorian date of a day count since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

impl DateTime {
    /// Seconds since the epoch (UTC).
    pub fn epoch(&self) -> i64 {
        self.ordinal_days() * 86400 + (self.hour * 3600 + self.minute * 60 + self.second) as i64
    }
    pub fn from_epoch(t: i64) -> DateTime {
        let days = t.div_euclid(86400);
        let rem = t.rem_euclid(86400);
        let (year, month, day) = civil_from_days(days);
        DateTime {
            year,
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: (rem % 3600 / 60) as u32,
            second: (rem % 60) as u32,
        }
    }
    pub fn date_iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    pub fn ordinal_days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
    pub fn iso_z(&self) -> String {
        format!(
            "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}
