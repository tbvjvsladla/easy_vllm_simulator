//! Python's dynamic-typing operations on [`PyValue`]s, as the classifier's code performs them on values it did not
//! build itself: subscripting, iteration, hashing, equality, `str.join`, `.get` on a non-dict, and
//! `datetime.timedelta(seconds=value)`. Each failure is the exception (class and message) CPython raises.

use crate::guidellm::Failure;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};

pub fn type_error(message: &str) -> Failure {
    Failure::new("TypeError", message)
}

/// `KeyError(key)`: the message is the key's `repr`.
pub fn key_error(key: &str) -> Failure {
    Failure::with_message("KeyError", PyStr::new(&PyStr::new(key).repr()))
}

/// `value["key"]` for a string key.
pub fn subscript<'a>(value: &'a PyValue, key: &str) -> Result<&'a PyValue, Failure> {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).ok_or_else(|| key_error(key)),
        PyValue::List(_) => Err(type_error(
            "list indices must be integers or slices, not str",
        )),
        PyValue::Tuple(_) => Err(type_error(
            "tuple indices must be integers or slices, not str",
        )),
        PyValue::Str(_) => Err(type_error("string indices must be integers, not 'str'")),
        PyValue::Bytes(_) => Err(type_error(
            "byte indices must be integers or slices, not str",
        )),
        other => Err(type_error(&format!(
            "'{}' object is not subscriptable",
            other.type_name()
        ))),
    }
}

/// `for item in value`: what a loop sees. A dict yields its keys and a string its characters.
pub fn iterate(value: &PyValue) -> Result<Vec<PyValue>, Failure> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Dict(map) => Ok(map
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect()),
        PyValue::Bytes(bytes) => Ok(bytes.iter().map(|&byte| PyValue::int(byte)).collect()),
        other => Err(type_error(&format!(
            "'{}' object is not iterable",
            other.type_name()
        ))),
    }
}

/// `hash(value)` succeeds: lists, dicts and sets are unhashable.
pub fn ensure_hashable(value: &PyValue) -> Result<(), Failure> {
    match value.peel() {
        PyValue::List(_) | PyValue::Dict(_) => Err(type_error(&format!(
            "unhashable type: '{}'",
            value.type_name()
        ))),
        PyValue::Opaque(opaque) if opaque.type_name == "set" => {
            Err(type_error("unhashable type: 'set'"))
        }
        // A tuple is hashed through its items.
        PyValue::Tuple(items) => items.iter().try_for_each(ensure_hashable),
        _ => Ok(()),
    }
}

/// `value.get(...)` where `value` should be a dict: any other type has no `get`.
pub fn as_get_target(value: &PyValue) -> Result<&PyDict, Failure> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map),
        other => Err(Failure::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

/// The dict when the value is one (`isinstance(value, dict)`).
pub fn dict_of(value: &PyValue) -> Option<&PyDict> {
    match value.peel() {
        PyValue::Dict(map) => Some(map),
        _ => None,
    }
}

/// `dict.get(key)` of an optional entry as a value (`None` when absent).
pub fn or_none(value: Option<&PyValue>) -> PyValue {
    value.cloned().unwrap_or(PyValue::None)
}

pub fn is_text(value: &PyValue, text: &str) -> bool {
    matches!(value.peel(), PyValue::Str(own) if own.equals(text))
}

/// `value == 0` for a value of any type.
pub fn equals_zero(value: &PyValue) -> bool {
    match value.peel() {
        PyValue::Int(number) => number.is_zero(),
        PyValue::Bool(flag) => !*flag,
        PyValue::Float(number) => *number == 0.0,
        _ => false,
    }
}

/// `left == right` for the value kinds a JSON document holds (the comparison the sort and campaign owners use).
pub fn equals(left: &PyValue, right: &PyValue) -> bool {
    crate::pysort::py_eq(left, right)
}

/// `separator.join(value)`: every item must be a string.
pub fn join(separator: &str, value: &PyValue) -> Result<PyStr, Failure> {
    let items = match value.peel() {
        PyValue::List(_)
        | PyValue::Tuple(_)
        | PyValue::Dict(_)
        | PyValue::Str(_)
        | PyValue::Bytes(_) => iterate(value)?,
        _ => return Err(type_error("can only join an iterable")),
    };
    join_values(separator, &items)
}

/// `separator.join(items)` over already-iterated items.
pub fn join_values(separator: &str, items: &[PyValue]) -> Result<PyStr, Failure> {
    let mut out = PyStr::default();
    for (index, item) in items.iter().enumerate() {
        let PyValue::Str(text) = item.peel() else {
            return Err(type_error(&format!(
                "sequence item {index}: expected str instance, {} found",
                item.type_name()
            )));
        };
        if index > 0 {
            out.push_str(separator);
        }
        out.push_py(text);
    }
    Ok(out)
}

/// `"%s" % value` for a value that is not a tuple.
pub fn show(value: &PyValue) -> PyStr {
    value.display_text()
}

/// Appends the `%s` rendering of each part: strings as they are, anything else through `str()`.
pub struct Text(pub PyStr);

impl Text {
    pub fn new() -> Self {
        Text(PyStr::default())
    }

    pub fn lit(mut self, text: &str) -> Self {
        self.0.push_str(text);
        self
    }

    pub fn val(mut self, value: &PyValue) -> Self {
        self.0.push_py(&value.display_text());
        self
    }

    pub fn str(mut self, text: &PyStr) -> Self {
        self.0.push_py(text);
        self
    }

    pub fn done(self) -> PyStr {
        self.0
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

// ── datetime.timedelta(seconds=...) ─────────────────────────────────────────────

const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;
const MAX_DELTA_DAYS: i64 = 999_999_999;

/// `datetime.timedelta(seconds=value)` as microseconds, with CPython 3.12.3's construction rules: an `int` or `bool`
/// is exact, a `float` splits into whole and fractional parts and rounds the leftover microseconds half to even,
/// anything else is a `TypeError`, and a result beyond +-999999999 days is an `OverflowError`.
pub fn timedelta_seconds_micros(value: &PyValue) -> Result<i128, Failure> {
    let micros_per_second = BigInt::from(MICROS_PER_SECOND);
    let mut total: BigInt;
    let mut leftover = 0.0_f64;
    match value.peel() {
        PyValue::Int(number) => total = number * &micros_per_second,
        PyValue::Bool(flag) => total = BigInt::from(u8::from(*flag)) * &micros_per_second,
        PyValue::Float(number) => {
            let number = *number;
            let whole = number.trunc();
            let fraction = number.fract();
            let whole_int = big_from_float(whole)?;
            total = whole_int * &micros_per_second;
            if fraction != 0.0 {
                let scaled = MICROS_PER_SECOND as f64 * fraction;
                total += big_from_float(scaled.trunc())?;
                leftover += scaled.fract();
            }
        }
        other => {
            return Err(type_error(&format!(
                "unsupported type for timedelta seconds component: {}",
                other.type_name()
            )))
        }
    }
    if leftover != 0.0 {
        let mut whole_us = leftover.round();
        if (whole_us - leftover).abs() == 0.5 {
            let odd = f64::from(u8::from(total.magnitude().bit(0)));
            whole_us = 2.0 * ((leftover + odd) * 0.5).round() - odd;
        }
        total += BigInt::from(whole_us as i64);
    }
    let days = floor_div(&total, &BigInt::from(MICROS_PER_DAY));
    let Some(days_i32) = days.to_i32() else {
        return Err(Failure::new(
            "OverflowError",
            "Python int too large to convert to C int",
        ));
    };
    if i64::from(days_i32).abs() > MAX_DELTA_DAYS {
        return Err(Failure::new(
            "OverflowError",
            &format!("days={days_i32}; must have magnitude <= {MAX_DELTA_DAYS}"),
        ));
    }
    Ok(total.to_i128().expect("a timedelta fits in i128"))
}

fn floor_div(numerator: &BigInt, denominator: &BigInt) -> BigInt {
    let quotient = numerator / denominator;
    if (numerator % denominator).is_negative() {
        quotient - 1
    } else {
        quotient
    }
}

/// `int(number)` of an integral float (`PyLong_FromDouble`).
fn big_from_float(number: f64) -> Result<BigInt, Failure> {
    if number.is_nan() {
        return Err(Failure::new(
            "ValueError",
            "cannot convert float NaN to integer",
        ));
    }
    if number.is_infinite() {
        return Err(Failure::new(
            "OverflowError",
            "cannot convert float infinity to integer",
        ));
    }
    Ok(BigInt::from_f64(number).expect("finite float"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timedelta_matches_cpython_edges() {
        let micros = |value: PyValue| {
            timedelta_seconds_micros(&value).map_err(|f| f.line().to_utf8().unwrap())
        };
        assert_eq!(micros(PyValue::int(1)), Ok(1_000_000));
        assert_eq!(micros(PyValue::int(-1)), Ok(-1_000_000));
        assert_eq!(micros(PyValue::Bool(true)), Ok(1_000_000));
        assert_eq!(micros(PyValue::Float(1.5)), Ok(1_500_000));
        assert_eq!(micros(PyValue::Float(0.000_000_5)), Ok(0));
        assert_eq!(
            micros(PyValue::int(86_400_i64 * 999_999_999)),
            Ok(86_400_i128 * 999_999_999 * 1_000_000)
        );
        assert_eq!(
            micros(PyValue::int(86_400_i64 * 1_000_000_000)),
            Err("OverflowError: days=1000000000; must have magnitude <= 999999999".to_string())
        );
        assert_eq!(
            micros(PyValue::int(-86_400_i64 * 999_999_999 - 1)),
            Err("OverflowError: days=-1000000000; must have magnitude <= 999999999".to_string())
        );
        assert_eq!(
            micros(PyValue::int(10_i128.pow(20))),
            Err("OverflowError: Python int too large to convert to C int".to_string())
        );
        assert_eq!(
            micros(PyValue::Float(f64::NAN)),
            Err("ValueError: cannot convert float NaN to integer".to_string())
        );
        assert_eq!(
            micros(PyValue::str("5")),
            Err("TypeError: unsupported type for timedelta seconds component: str".to_string())
        );
    }

    #[test]
    fn equality_crosses_number_types() {
        assert!(equals(&PyValue::int(1), &PyValue::Float(1.0)));
        assert!(equals(&PyValue::Bool(true), &PyValue::int(1)));
        assert!(!equals(&PyValue::str("1"), &PyValue::int(1)));
        assert!(!equals(
            &PyValue::Float(f64::NAN),
            &PyValue::Float(f64::NAN)
        ));
    }
}
