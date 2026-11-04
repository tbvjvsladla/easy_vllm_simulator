//! JSON values with the Python types the original program observes: `int` is arbitrary precision and distinct
//! from `bool` and `float`, dicts keep insertion order, strings are Python strings.

use crate::pystr::PyStr;
use num_bigint::BigInt;
use num_traits::{FromPrimitive, Zero};
use std::fmt;

/// `sys.get_int_max_str_digits()` default: Python refuses to render longer integers.
const MAX_INT_DIGITS: usize = 4300;
/// Deepest container nesting `json.dumps(indent=2)` can write. With an indent Python uses its pure-Python
/// encoder, which recurses once per container, so the interpreter's frame limit applies. Measured on the
/// original CLI: a verdict whose deepest container is the 995th level prints, the 996th raises `RecursionError`.
const PRETTY_MAX_DEPTH: usize = 995;

/// Why a value could not be rendered as JSON; each case is an exception Python raises while printing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpError {
    /// An integer with more than 4300 digits (`ValueError` from `int.__repr__`).
    IntegerTooLong,
    /// Containers nested beyond what the pure-Python encoder can recurse through (`RecursionError`).
    TooDeep,
}

impl DumpError {
    pub fn python_type(&self) -> &'static str {
        match self {
            DumpError::IntegerTooLong => "ValueError",
            DumpError::TooDeep => "RecursionError",
        }
    }
}

impl fmt::Display for DumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DumpError::IntegerTooLong => write!(
                f,
                "ValueError: Exceeds the limit ({MAX_INT_DIGITS} digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit"
            ),
            DumpError::TooDeep => f.write_str("RecursionError: maximum recursion depth exceeded"),
        }
    }
}

impl std::error::Error for DumpError {}

#[derive(Clone, Debug, PartialEq)]
pub enum PyValue {
    None,
    Bool(bool),
    Int(BigInt),
    Float(f64),
    Str(PyStr),
    List(Vec<PyValue>),
    Dict(PyDict),
}

/// Insertion-ordered dict. Assigning an existing key replaces the value and keeps the original position,
/// as Python does for duplicate keys in a JSON object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PyDict {
    entries: Vec<(PyStr, PyValue)>,
}

impl PyDict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: PyStr, value: PyValue) {
        match self
            .entries
            .iter_mut()
            .find(|(existing, _)| *existing == key)
        {
            Some(slot) => slot.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    /// Convenience for keys known at compile time.
    pub fn set(&mut self, key: &str, value: PyValue) {
        self.insert(PyStr::new(key), value);
    }

    pub fn get(&self, key: &str) -> Option<&PyValue> {
        self.entries
            .iter()
            .find(|(existing, _)| existing.equals(key))
            .map(|(_, value)| value)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(PyStr, PyValue)> {
        self.entries.iter()
    }
}

impl PyValue {
    pub fn int(value: impl Into<BigInt>) -> PyValue {
        PyValue::Int(value.into())
    }

    pub fn str(text: &str) -> PyValue {
        PyValue::Str(PyStr::new(text))
    }

    /// Python `type(value).__name__`.
    pub fn type_name(&self) -> &'static str {
        match self {
            PyValue::None => "NoneType",
            PyValue::Bool(_) => "bool",
            PyValue::Int(_) => "int",
            PyValue::Float(_) => "float",
            PyValue::Str(_) => "str",
            PyValue::List(_) => "list",
            PyValue::Dict(_) => "dict",
        }
    }

    /// Python `bool(value)`.
    pub fn is_truthy(&self) -> bool {
        match self {
            PyValue::None => false,
            PyValue::Bool(flag) => *flag,
            PyValue::Int(number) => !number.is_zero(),
            PyValue::Float(number) => *number != 0.0,
            PyValue::Str(text) => !text.is_empty(),
            PyValue::List(items) => !items.is_empty(),
            PyValue::Dict(map) => !map.is_empty(),
        }
    }

    /// The value when it is a real `int` (`bool` is excluded, as `isinstance(v, int) and not isinstance(v, bool)`).
    pub fn as_plain_int(&self) -> Option<&BigInt> {
        match self {
            PyValue::Int(number) => Some(number),
            _ => None,
        }
    }

    /// Python `value == number` for a non-negative `int` the caller holds.
    pub fn equals_int(&self, number: &BigInt) -> bool {
        match self {
            PyValue::Int(own) => own == number,
            PyValue::Bool(flag) => BigInt::from(u8::from(*flag)) == *number,
            PyValue::Float(own) => {
                own.is_finite()
                    && own.fract() == 0.0
                    && BigInt::from_f64(*own).as_ref() == Some(number)
            }
            _ => false,
        }
    }

    /// Python `repr(value)`.
    pub fn repr(&self) -> String {
        match self {
            PyValue::None => "None".to_string(),
            PyValue::Bool(true) => "True".to_string(),
            PyValue::Bool(false) => "False".to_string(),
            PyValue::Int(number) => number.to_string(),
            PyValue::Float(number) => float_repr(*number),
            PyValue::Str(text) => text.repr(),
            PyValue::List(items) => {
                let inner: Vec<String> = items.iter().map(PyValue::repr).collect();
                format!("[{}]", inner.join(", "))
            }
            PyValue::Dict(map) => {
                let inner: Vec<String> = map
                    .iter()
                    .map(|(key, value)| format!("{}: {}", key.repr(), value.repr()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
        }
    }

    /// Python `str(value)`: a string is itself, everything else is its repr.
    pub fn display_text(&self) -> PyStr {
        match self {
            PyValue::Str(text) => text.clone(),
            other => PyStr::new(&other.repr()),
        }
    }

    /// `json.dumps(value, ensure_ascii=False, indent=2)`; the text may still hold lone surrogates.
    pub fn dumps_pretty(&self) -> Result<PyStr, DumpError> {
        let mut out = PyStr::default();
        self.write_json(&mut out, Layout::Pretty { level: 0 }, false)?;
        Ok(out)
    }

    /// `json.dumps(value)`: default separators and `ensure_ascii=True`, so the text is pure ASCII.
    pub fn dumps_ascii(&self) -> Result<String, DumpError> {
        let mut out = PyStr::default();
        self.write_json(&mut out, Layout::Compact, true)?;
        Ok(out.to_utf8().expect("ensure_ascii output is ASCII"))
    }

    fn write_json(&self, out: &mut PyStr, layout: Layout, ascii: bool) -> Result<(), DumpError> {
        match self {
            PyValue::None => out.push_str("null"),
            PyValue::Bool(true) => out.push_str("true"),
            PyValue::Bool(false) => out.push_str("false"),
            PyValue::Int(number) => {
                let digits = number.to_string();
                if digits.trim_start_matches('-').len() > MAX_INT_DIGITS {
                    return Err(DumpError::IntegerTooLong);
                }
                out.push_str(&digits);
            }
            PyValue::Float(number) => out.push_str(&float_json(*number)),
            PyValue::Str(text) => text.write_json(out, ascii),
            PyValue::List(items) => {
                write_sequence(out, layout, ('[', ']'), items.len(), |out, index, inner| {
                    items[index].write_json(out, inner, ascii)
                })?
            }
            PyValue::Dict(map) => {
                let entries: Vec<&(PyStr, PyValue)> = map.iter().collect();
                write_sequence(
                    out,
                    layout,
                    ('{', '}'),
                    entries.len(),
                    |out, index, inner| {
                        let (key, value) = entries[index];
                        key.write_json(out, ascii);
                        out.push_str(": ");
                        value.write_json(out, inner, ascii)
                    },
                )?
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Layout {
    Compact,
    Pretty { level: usize },
}

impl Layout {
    fn nested(self) -> Layout {
        match self {
            Layout::Compact => Layout::Compact,
            Layout::Pretty { level } => Layout::Pretty { level: level + 1 },
        }
    }
}

fn write_sequence(
    out: &mut PyStr,
    layout: Layout,
    (open, close): (char, char),
    count: usize,
    mut write_item: impl FnMut(&mut PyStr, usize, Layout) -> Result<(), DumpError>,
) -> Result<(), DumpError> {
    if matches!(layout, Layout::Pretty { level } if level + 1 > PRETTY_MAX_DEPTH) {
        return Err(DumpError::TooDeep);
    }
    out.push_code_point(u32::from(open));
    if count == 0 {
        out.push_code_point(u32::from(close));
        return Ok(());
    }
    let inner = layout.nested();
    for index in 0..count {
        match (layout, index) {
            (Layout::Compact, 0) => {}
            (Layout::Compact, _) => out.push_str(", "),
            (Layout::Pretty { .. }, 0) => push_indent(out, inner),
            (Layout::Pretty { .. }, _) => {
                out.push_code_point(u32::from(','));
                push_indent(out, inner);
            }
        }
        write_item(out, index, inner)?;
    }
    if let Layout::Pretty { level } = layout {
        push_indent(out, Layout::Pretty { level });
    }
    out.push_code_point(u32::from(close));
    Ok(())
}

fn push_indent(out: &mut PyStr, layout: Layout) {
    if let Layout::Pretty { level } = layout {
        out.push_code_point(u32::from('\n'));
        for _ in 0..level * 2 {
            out.push_code_point(u32::from(' '));
        }
    }
}

/// Digits and decimal exponent of a finite positive double in scientific form, as `format!("{:.*e}")` renders it.
fn scientific_digits(magnitude: f64, precision: usize) -> (String, i32) {
    let text = format!("{magnitude:.precision$e}");
    let (mantissa, exponent) = text.split_once('e').expect("{:e} always has an exponent");
    (
        mantissa.chars().filter(|c| c.is_ascii_digit()).collect(),
        exponent.parse().expect("exponent is an integer"),
    )
}

/// Python's `repr` takes, among the shortest digit strings that read back as the same double, the one closest to
/// the exact value and, when two are equally close (the exact value ends in a 5 right after them), the one with
/// the even last digit: `repr(2175058994972888.25)` is `2175058994972888.2`. Rust's shortest formatter breaks that
/// tie upward (`...8.3`). `digits` is Rust's shortest form; the Python choice is returned.
fn settle_exact_tie(magnitude: f64, digits: String, exponent: i32) -> (String, i32) {
    let length = digits.len();
    let (wide, wide_exponent) = scientific_digits(magnitude, length);
    if wide.len() != length + 1 || !wide.ends_with('5') {
        return (digits, exponent);
    }
    // Only an expansion that stops right after that 5 is a tie; a double has at most 767 significant digits.
    let (exact, _) = scientific_digits(magnitude, 800);
    if exact[length + 1..].bytes().any(|digit| digit != b'0') {
        return (digits, exponent);
    }
    let lower = &wide[..length];
    let mut chosen = lower.to_string();
    let mut chosen_exponent = wide_exponent;
    if (lower.as_bytes()[length - 1] - b'0') % 2 == 1 {
        let mut bytes = lower.as_bytes().to_vec();
        let mut index = length;
        loop {
            if index == 0 {
                bytes.insert(0, b'1');
                bytes.truncate(length);
                chosen_exponent += 1;
                break;
            }
            index -= 1;
            if bytes[index] == b'9' {
                bytes[index] = b'0';
            } else {
                bytes[index] += 1;
                break;
            }
        }
        chosen = String::from_utf8(bytes).expect("ASCII digits");
    }
    let trimmed = chosen.trim_end_matches('0');
    let trimmed = if trimmed.is_empty() { "0" } else { trimmed };
    let reads_back = format!("{trimmed}e{}", chosen_exponent - (trimmed.len() as i32 - 1))
        .parse::<f64>()
        .is_ok_and(|value| value == magnitude);
    if reads_back {
        (trimmed.to_string(), chosen_exponent)
    } else {
        (digits, exponent)
    }
}

/// Python `repr(float)`: shortest round-trip digits, exponent form outside `1e-4 <= |x| < 1e16`.
pub fn float_repr(number: f64) -> String {
    if number.is_nan() {
        return "nan".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if number == 0.0 {
        return if number.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_string();
    }
    let scientific = format!("{:e}", number.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("{:e} always has an exponent");
    let exponent: i32 = exponent.parse().expect("exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    let (digits, exponent) = settle_exact_tie(number.abs(), digits, exponent);
    let sign = if number < 0.0 { "-" } else { "" };
    if !(-4..16).contains(&exponent) {
        let fraction = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        return format!(
            "{sign}{}{fraction}e{exponent_sign}{:02}",
            &digits[..1],
            exponent.abs()
        );
    }
    if exponent >= 0 {
        let integer_len = exponent as usize + 1;
        if digits.len() <= integer_len {
            format!("{sign}{digits}{}.0", "0".repeat(integer_len - digits.len()))
        } else {
            format!(
                "{sign}{}.{}",
                &digits[..integer_len],
                &digits[integer_len..]
            )
        }
    } else {
        format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize))
    }
}

fn float_json(number: f64) -> String {
    if number.is_nan() {
        "NaN".to_string()
    } else if number.is_infinite() {
        if number > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        }
        .to_string()
    } else {
        float_repr(number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_repr_matches_python() {
        let cases = [
            (1.0, "1.0"),
            (0.1, "0.1"),
            (1.5, "1.5"),
            (100.0, "100.0"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (123456789012345680.0, "1.2345678901234568e+17"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (-2.5, "-2.5"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            // Two shortest strings read back as this double and are equally close; Python takes the even one.
            (2_175_058_994_972_888.0 + 0.25, "2175058994972888.2"),
            (2_175_058_994_972_888.0 + 0.75, "2175058994972888.8"),
            (9007199254740993.0, "9007199254740992.0"),
        ];
        for (number, expected) in cases {
            assert_eq!(float_repr(number), expected, "{number:e}");
        }
        assert_eq!(float_repr(f64::INFINITY), "inf");
        assert_eq!(float_json(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(float_json(f64::NAN), "NaN");
    }

    #[test]
    fn equality_follows_python_numeric_rules() {
        let three = BigInt::from(3);
        assert!(PyValue::Float(3.0).equals_int(&three));
        assert!(!PyValue::Float(3.5).equals_int(&three));
        assert!(!PyValue::Bool(true).equals_int(&three));
        assert!(PyValue::Bool(true).equals_int(&BigInt::from(1)));
        assert!(!PyValue::str("3").equals_int(&three));
        assert!(!PyValue::None.equals_int(&three));
        assert!(PyValue::Float(2f64.powi(80)).equals_int(&(BigInt::from(1) << 80)));
    }

    #[test]
    fn dumps_match_python_layouts() {
        let mut inner = PyDict::new();
        inner.set("k", PyValue::List(vec![]));
        inner.set("n", PyValue::int(2));
        let value = PyValue::List(vec![PyValue::Dict(inner), PyValue::Dict(PyDict::new())]);
        assert_eq!(
            value.dumps_pretty().unwrap().to_utf8().unwrap(),
            "[\n  {\n    \"k\": [],\n    \"n\": 2\n  },\n  {}\n]"
        );
        assert_eq!(value.dumps_ascii().unwrap(), "[{\"k\": [], \"n\": 2}, {}]");
    }
}
