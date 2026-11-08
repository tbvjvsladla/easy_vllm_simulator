//! The dynamic-typing operations the three recipe modules perform on values they do not control (JSON from files and
//! API callers): `int()`/`float()`, `.get`/`[key]`, arithmetic across `bool`/`int`/`float`, and the exceptions Python
//! raises for the rest. Every function returns the exception (class and message) CPython raises, built from the common
//! support package; nothing here decides a recipe rule.

use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::{as_get_target, iterate, key_error, subscript};
use agent_runtime_support::pynumber::{self, PyException};
use agent_runtime_support::pyops::{self, Compare};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

pub type Res<T> = Result<T, Failure>;

pub fn exception(error: PyException) -> Failure {
    Failure::from(error)
}

pub fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

pub fn big(value: i64) -> BigInt {
    BigInt::from(value)
}

/// `int(value)`.
pub fn int_of(value: &PyValue) -> Res<BigInt> {
    pynumber::int_call(value).map_err(exception)
}

/// `float(value)`.
pub fn float_of(value: &PyValue) -> Res<f64> {
    pynumber::float_call(value).map_err(exception)
}

/// `float(integer)`: `OverflowError` beyond the double range.
pub fn float_of_int(value: &BigInt) -> Res<f64> {
    pynumber::float_from_int(value).map_err(exception)
}

/// `value.get(key)` (`None` when absent); a value that is not an object has no `get`.
pub fn get(value: &PyValue, key: &str) -> Res<PyValue> {
    Ok(as_get_target(value)?
        .get(key)
        .cloned()
        .unwrap_or(PyValue::None))
}

/// `value.get(key, default)`.
pub fn get_or(value: &PyValue, key: &str, default: PyValue) -> Res<PyValue> {
    Ok(as_get_target(value)?.get(key).cloned().unwrap_or(default))
}

/// `value[key]`.
pub fn item(value: &PyValue, key: &str) -> Res<PyValue> {
    subscript(value, key).cloned()
}

/// `KeyError(key)`.
pub fn missing_key(key: &str) -> Failure {
    key_error(key)
}

pub fn is_none(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::None)
}

/// `for item in value`.
pub fn items_of(value: &PyValue) -> Res<Vec<PyValue>> {
    iterate(value)
}

/// `text.strip().lower()` for a value that must be a string; anything else has no `strip`.
pub fn stripped_lower(value: &PyValue) -> Res<PyStr> {
    match value.peel() {
        PyValue::Str(own) => Ok(lower(&own.strip())),
        other => Err(Failure::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'strip'", other.type_name()),
        )),
    }
}

/// `str.lower()` with CPython's Unicode 15 mapping. A lone surrogate is neither cased nor case-ignorable, so it ends
/// the context of the Final_Sigma rule: each run between surrogates is lowered on its own.
pub fn lower(text: &PyStr) -> PyStr {
    let mut out: Vec<u32> = Vec::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut Vec<u32>| {
        if !run.is_empty() {
            out.extend(
                agent_runtime_support::pylower::python_lower(run)
                    .chars()
                    .map(u32::from),
            );
            run.clear();
        }
    };
    for &point in text.code_points() {
        match char::from_u32(point) {
            Some(c) => run.push(c),
            None => {
                flush(&mut run, &mut out);
                out.push(point);
            }
        }
    }
    flush(&mut run, &mut out);
    PyStr::from_code_points(out)
}

/// `PyNumber_AsSsize_t`: an integer as a signed index-sized value, `OverflowError` outside the range.
fn as_ssize(count: &BigInt) -> Res<i64> {
    count.to_i64().ok_or_else(|| {
        Failure::new(
            "OverflowError",
            "cannot fit 'int' into an index-sized integer",
        )
    })
}

/// `sequence * count` for a string or list and an integer count (`bool` counts as an integer). The count is converted to a
/// signed index first, so a count outside it is an `OverflowError` even for an empty sequence or a negative count; then a
/// non-positive count or an empty sequence gives the empty sequence; then the length is checked against the index range
/// (`OverflowError: repeated string is too long` for a string, `MemoryError` for a list) and the storage is requested
/// fallibly (`MemoryError` when it cannot be had).
fn repeat_sequence(sequence: &PyValue, count: &BigInt) -> Option<Res<PyValue>> {
    let list_or_text = matches!(sequence.peel(), PyValue::Str(_) | PyValue::List(_));
    if !list_or_text {
        return None;
    }
    Some((|| -> Res<PyValue> {
        let times = as_ssize(count)?;
        let length = match sequence.peel() {
            PyValue::Str(own) => own.len(),
            PyValue::List(items) => items.len(),
            _ => unreachable!("checked above"),
        };
        if times <= 0 || length == 0 {
            return Ok(match sequence.peel() {
                PyValue::Str(_) => PyValue::Str(PyStr::default()),
                _ => PyValue::List(Vec::new()),
            });
        }
        let times = usize::try_from(times).expect("positive index-sized count");
        let total = match length
            .checked_mul(times)
            .filter(|&n| i64::try_from(n).is_ok())
        {
            Some(total) => total,
            None => {
                return Err(match sequence.peel() {
                    PyValue::Str(_) => Failure::new("OverflowError", "repeated string is too long"),
                    _ => Failure::new("MemoryError", ""),
                });
            }
        };
        match sequence.peel() {
            PyValue::Str(own) => {
                let mut points: Vec<u32> = Vec::new();
                points
                    .try_reserve_exact(total)
                    .map_err(|_| Failure::new("MemoryError", ""))?;
                for _ in 0..times {
                    points.extend_from_slice(own.code_points());
                }
                Ok(PyValue::Str(PyStr::from_code_points(points)))
            }
            PyValue::List(items) => {
                let mut repeated: Vec<PyValue> = Vec::new();
                repeated
                    .try_reserve_exact(total)
                    .map_err(|_| Failure::new("MemoryError", ""))?;
                for _ in 0..times {
                    repeated.extend(items.iter().cloned());
                }
                Ok(PyValue::List(repeated))
            }
            _ => unreachable!("checked above"),
        }
    })())
}

/// `left * right` for the operand types that can occur: numbers; a string or list times an integer repeats it; anything
/// else is the interpreter's `TypeError`.
pub fn mul(left: &PyValue, right: &PyValue) -> Res<PyValue> {
    let integer = |value: &PyValue| match value.peel() {
        PyValue::Int(number) => Some(number.clone()),
        PyValue::Bool(flag) => Some(BigInt::from(u8::from(*flag))),
        _ => None,
    };
    if let Some(count) = integer(right) {
        if let Some(done) = repeat_sequence(left, &count) {
            return done;
        }
    }
    if let Some(count) = integer(left) {
        if let Some(done) = repeat_sequence(right, &count) {
            return done;
        }
    }
    pyops::multiply(left, right).map_err(exception)
}

/// `left - right`.
pub fn sub(left: &PyValue, right: &PyValue) -> Res<PyValue> {
    pyops::subtract(left, right).map_err(exception)
}

/// `left / right` where `left` is a float.
pub fn float_div(left: f64, right: &PyValue) -> Res<f64> {
    pyops::divide(&PyValue::Float(left), right).map_err(exception)
}

/// `left <op> right`.
pub fn compare(operator: Compare, left: &PyValue, right: &PyValue) -> Res<bool> {
    pyops::compare(operator, left, right).map_err(exception)
}

/// `a / 2**shift` for an integer `a`, as Python's integer true division rounds it: one correct rounding of the exact
/// quotient, `OverflowError` when it does not fit a double.
pub fn int_div_power_of_two(number: &BigInt, shift: u32) -> Res<f64> {
    let scale = 2f64.powi(shift as i32);
    if number.bits() <= 1000 {
        return Ok(float_of_int(number)? / scale);
    }
    let negative = number.is_negative();
    let magnitude = number.abs();
    let quotient = &magnitude >> shift as usize;
    let sticky = (&magnitude & ((BigInt::from(1) << shift as usize) - 1)) != BigInt::zero();
    let folded = if sticky {
        quotient | BigInt::from(1)
    } else {
        quotient
    };
    let value = folded.to_f64().unwrap_or(f64::INFINITY);
    if value.is_infinite() {
        return Err(Failure::new(
            "OverflowError",
            "integer division result too large for a float",
        ));
    }
    Ok(if negative { -value } else { value })
}

/// `value / 4096` for a value of any type (the context term of the activation formula).
pub fn div_by_4096(value: &PyValue) -> Res<f64> {
    match value.peel() {
        PyValue::Int(number) => int_div_power_of_two(number, 12),
        PyValue::Bool(flag) => Ok(f64::from(u8::from(*flag)) / 4096.0),
        other => pyops::divide(other, &PyValue::int(4096)).map_err(exception),
    }
}

/// `x * y` where both are floats already (Python float multiplication).
pub fn f(value: f64) -> PyValue {
    PyValue::Float(value)
}

/// A new object with the given members in order.
pub fn object(members: Vec<(&str, PyValue)>) -> PyDict {
    let mut dict = PyDict::new();
    for (key, value) in members {
        dict.set(key, value);
    }
    dict
}
