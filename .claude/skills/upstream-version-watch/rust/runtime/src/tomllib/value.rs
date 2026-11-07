//! The values `tomllib.loads` returns, as Rust types: strings as Unicode code points, integers of arbitrary size, floats by their exact IEEE-754
//! value (NaN sign and `-0.0` included), the three temporal kinds as distinct types, arrays and insertion-ordered tables.

use num_bigint::BigInt;
use std::collections::HashMap;

/// A TOML key or string value: Unicode code points, as a Python `str` (surrogate code points included).
pub type Text = Vec<u32>;

/// A parsed TOML value (`dict`, `list`, `str`, `int`, `float`, `bool`, `datetime.date`/`time`/`datetime`).
#[derive(Debug, Clone, PartialEq)]
pub enum TomlValue {
    Bool(bool),
    Int(BigInt),
    Float(f64),
    Str(Text),
    Date(CivilDate),
    Time(CivilTime),
    DateTime(CivilDateTime),
    Array(Vec<TomlValue>),
    Table(Table),
}

/// `datetime.date`, already validated by the matcher of the datetime pattern and by [`CivilDate::validated`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

/// `datetime.time` without time zone: the fraction is already truncated to microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub microsecond: u32,
}

/// A fixed UTC offset in minutes (`datetime.timezone(timedelta(...))`), strictly inside one day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtcOffset {
    pub total_minutes: i16,
}

/// `datetime.datetime`, naive (`offset == None`) or aware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilDateTime {
    pub date: CivilDate,
    pub time: CivilTime,
    pub offset: Option<UtcOffset>,
}

impl CivilDate {
    /// `datetime.date(year, month, day)` for a `month` already in `1..=12`: the `ValueError` text of the constructor, or the date.
    pub fn validated(year: u16, month: u8, day: u8) -> Result<CivilDate, String> {
        if year == 0 {
            return Err(format!("year {year} is out of range"));
        }
        if day > days_in_month(year, month) {
            return Err("day is out of range for month".to_string());
        }
        Ok(CivilDate { year, month, day })
    }

    /// `date.isoformat()`.
    pub fn isoformat(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl CivilTime {
    /// `time.isoformat()`: the fraction is written only when it is not zero.
    pub fn isoformat(&self) -> String {
        let base = format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second);
        if self.microsecond == 0 {
            base
        } else {
            format!("{base}.{:06}", self.microsecond)
        }
    }
}

impl UtcOffset {
    /// The `+HH:MM` / `-HH:MM` suffix of `datetime.isoformat()`; a zero offset is `+00:00`.
    pub fn isoformat(&self) -> String {
        let sign = if self.total_minutes < 0 { '-' } else { '+' };
        let magnitude = self.total_minutes.unsigned_abs();
        format!("{sign}{:02}:{:02}", magnitude / 60, magnitude % 60)
    }
}

impl CivilDateTime {
    /// `datetime.isoformat()` with the `T` separator.
    pub fn isoformat(&self) -> String {
        let offset = self.offset.map(|o| o.isoformat()).unwrap_or_default();
        format!(
            "{}T{}{offset}",
            self.date.isoformat(),
            self.time.isoformat()
        )
    }
}

/// An insertion-ordered mapping with text keys: the `dict` of a TOML table.
#[derive(Debug, Clone, Default)]
pub struct Table {
    entries: Vec<(Text, TomlValue)>,
    positions: HashMap<Text, usize>,
}

impl PartialEq for Table {
    /// Ordered equality: the entries in insertion order.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Table {
    pub fn new() -> Self {
        Table::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&Text, &TomlValue)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub fn get(&self, key: &[u32]) -> Option<&TomlValue> {
        self.positions.get(key).map(|&at| &self.entries[at].1)
    }

    pub fn contains_key(&self, key: &[u32]) -> bool {
        self.positions.contains_key(key)
    }

    /// `self[key] = value` for a key that is not yet present (the parser checks absence first, as the original does).
    pub(crate) fn insert_new(&mut self, key: Text, value: TomlValue) {
        debug_assert!(!self.positions.contains_key(&key));
        self.positions.insert(key.clone(), self.entries.len());
        self.entries.push((key, value));
    }

    /// `if key not in self: self[key] = {}` then `self[key]`.
    pub(crate) fn slot_or_new_table(&mut self, key: &[u32]) -> &mut TomlValue {
        let at = match self.positions.get(key) {
            Some(&at) => at,
            None => {
                let at = self.entries.len();
                self.positions.insert(key.to_vec(), at);
                self.entries
                    .push((key.to_vec(), TomlValue::Table(Table::new())));
                at
            }
        };
        &mut self.entries[at].1
    }

    /// The value stored under an existing key.
    pub(crate) fn get_mut(&mut self, key: &[u32]) -> Option<&mut TomlValue> {
        let at = *self.positions.get(key)?;
        Some(&mut self.entries[at].1)
    }
}
