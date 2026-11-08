//! Python's dynamic-typing operations that the model scripts apply to JSON they do not control, beyond what [`crate::pyexpr`]
//! already provides: floor division across `bool`/`int`/`float`, the value of an `or` chain, `dict` keys of any hashable type
//! with Python's equality, a warnings list that may not be a list, and `round()`/true division of integers.

use crate::pyexpr::{exception, Res};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::ensure_hashable;
use agent_runtime_support::pynumber::{float_from_int, int_from_float};
use agent_runtime_support::pysort::same_or_eq;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{DictKey, PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

/// `PyStr` from text.
pub fn s(text: &str) -> PyStr {
    PyStr::new(text)
}

/// The text of a message assembled from parts (`%s` of a value is its `str()`).
pub struct Message(PyStr);

impl Message {
    pub fn new() -> Self {
        Message(PyStr::default())
    }

    pub fn lit(mut self, text: &str) -> Self {
        self.0.push_str(text);
        self
    }

    /// `%s` / `{value}`.
    pub fn val(mut self, value: &PyValue) -> Self {
        self.0.push_py(&value.display_text());
        self
    }

    pub fn text(mut self, text: &PyStr) -> Self {
        self.0.push_py(text);
        self
    }

    /// `%d` of an integer.
    pub fn int(mut self, number: &BigInt) -> Self {
        self.0.push_str(&number.to_string());
        self
    }

    pub fn count(self, number: usize) -> Self {
        self.int(&BigInt::from(number))
    }

    /// `%s` of a float.
    pub fn float(mut self, number: f64) -> Self {
        self.0.push_str(&PyValue::Float(number).repr());
        self
    }

    pub fn done(self) -> PyStr {
        self.0
    }
}

impl Default for Message {
    fn default() -> Self {
        Self::new()
    }
}

/// `warnings.append(text)` on something that was given as the warnings list. A value that is not a list has no `append`,
/// and that is only found out when the first warning is written.
pub enum Warnings<'a> {
    List(&'a mut Vec<PyValue>),
    NotAList(&'static str),
}

impl<'a> Warnings<'a> {
    /// The sink for a supplied argument (a list is written to in place, anything else fails when written to).
    pub fn of(value: &'a mut PyValue) -> Warnings<'a> {
        match value {
            PyValue::List(items) => Warnings::List(items),
            other => Warnings::NotAList(other.type_name()),
        }
    }

    pub fn push(&mut self, text: PyStr) -> Res<()> {
        match self {
            Warnings::List(items) => {
                items.push(PyValue::Str(text));
                Ok(())
            }
            Warnings::NotAList(type_name) => Err(Failure::new(
                "AttributeError",
                &format!("'{type_name}' object has no attribute 'append'"),
            )),
        }
    }
}

/// Whether the exception is an `OSError` (any subclass) or a `ValueError` (any subclass: `UnicodeDecodeError`,
/// `JSONDecodeError`) or a `struct.error`: what the model scripts' handlers name.
pub fn is_os_value_or_struct_error(class: &str) -> bool {
    is_os_error(class) || is_value_error(class) || class == "error"
}

pub fn is_os_error(class: &str) -> bool {
    matches!(
        class,
        "OSError"
            | "FileNotFoundError"
            | "PermissionError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "FileExistsError"
            | "InterruptedError"
            | "ChildProcessError"
            | "ProcessLookupError"
            | "BrokenPipeError"
            | "BlockingIOError"
            | "ConnectionAbortedError"
            | "ConnectionResetError"
            | "ConnectionRefusedError"
            | "TimeoutError"
    )
}

pub fn is_value_error(class: &str) -> bool {
    matches!(
        class,
        "ValueError" | "UnicodeDecodeError" | "UnicodeEncodeError" | "JSONDecodeError"
    )
}

/// `except (TypeError, ValueError)`.
pub fn is_type_or_value_error(class: &str) -> bool {
    class == "TypeError" || is_value_error(class)
}

fn unsupported_floor_div(left: &PyValue, right: &PyValue) -> Failure {
    Failure::new(
        "TypeError",
        &format!(
            "unsupported operand type(s) for //: '{}' and '{}'",
            left.type_name(),
            right.type_name()
        ),
    )
}

fn as_integer(value: &PyValue) -> Option<BigInt> {
    match value.peel() {
        PyValue::Int(number) => Some(number.clone()),
        PyValue::Bool(flag) => Some(BigInt::from(u8::from(*flag))),
        _ => None,
    }
}

/// CPython's `float_floor_div` of two doubles.
fn float_floor_div(numerator: f64, denominator: f64) -> Res<f64> {
    if denominator == 0.0 {
        return Err(Failure::new(
            "ZeroDivisionError",
            "float floor division by zero",
        ));
    }
    let modulus = numerator % denominator;
    let mut quotient = (numerator - modulus) / denominator;
    if modulus != 0.0 && (denominator < 0.0) != (modulus < 0.0) {
        quotient -= 1.0;
    }
    let floored = if quotient != 0.0 {
        let floor = quotient.floor();
        if quotient - floor > 0.5 {
            floor + 1.0
        } else {
            floor
        }
    } else {
        (0.0f64).copysign(numerator / denominator)
    };
    Ok(floored)
}

/// `left // right` for numbers (`bool` counts as `int`); everything else is the interpreter's `TypeError`.
pub fn floor_div(left: &PyValue, right: &PyValue) -> Res<PyValue> {
    if let (Some(a), Some(b)) = (as_integer(left), as_integer(right)) {
        return int_floor_div(&a, &b).map(PyValue::Int);
    }
    let float_of = |value: &PyValue| -> Option<Res<f64>> {
        match value.peel() {
            PyValue::Float(number) => Some(Ok(*number)),
            other => as_integer(other).map(|number| float_from_int(&number).map_err(exception)),
        }
    };
    match (float_of(left), float_of(right)) {
        (Some(a), Some(b)) => Ok(PyValue::Float(float_floor_div(a?, b?)?)),
        _ => Err(unsupported_floor_div(left, right)),
    }
}

/// `a // b` for two integers (floor division: the quotient rounds toward minus infinity).
pub fn int_floor_div(a: &BigInt, b: &BigInt) -> Res<BigInt> {
    if b.is_zero() {
        return Err(Failure::new(
            "ZeroDivisionError",
            "integer division or modulo by zero",
        ));
    }
    let quotient = a / b;
    let remainder = a % b;
    Ok(
        if !remainder.is_zero() && (remainder.is_negative() != b.is_negative()) {
            quotient - 1
        } else {
            quotient
        },
    )
}

/// `a or b or c ...`: the first truthy value, else the last one (falsy as it is).
pub fn or_chain<'a>(values: &[&'a PyValue]) -> &'a PyValue {
    for value in values {
        if value.is_truthy() {
            return value;
        }
    }
    values[values.len() - 1]
}

/// `round(number)` of a double: half to even, as an integer (`OverflowError`/`ValueError` for infinity/NaN).
pub fn round_to_int(number: f64) -> Res<BigInt> {
    int_from_float(number.round_ties_even()).map_err(exception)
}

/// `native / divisor` for an integer and a double (`float(native)` first, so a huge integer is an `OverflowError`).
pub fn int_over_float(native: &BigInt, divisor: f64) -> Res<f64> {
    let numerator = float_from_int(native).map_err(exception)?;
    if divisor == 0.0 {
        return Err(Failure::new("ZeroDivisionError", "float division by zero"));
    }
    Ok(numerator / divisor)
}

/// `a / b` for two integers: Python's integer true division (one correct rounding of the exact quotient).
pub fn int_over_int(a: &BigInt, b: &BigInt) -> Res<f64> {
    agent_runtime_support::pynumber::int_true_divide(a, b).map_err(exception)
}

/// `int(value)` of a value read from JSON (a shared NaN object is looked through).
pub fn int_of(value: &PyValue) -> Res<BigInt> {
    crate::pyexpr::int_of(value.peel())
}

/// `float(value)` of a value read from JSON.
pub fn float_of(value: &PyValue) -> Res<f64> {
    crate::pyexpr::float_of(value.peel())
}

/// `left - right` of values read from JSON.
pub fn sub(left: &PyValue, right: &PyValue) -> Res<PyValue> {
    crate::pyexpr::sub(left.peel(), right.peel())
}

/// `left * right` of values read from JSON.
pub fn mul(left: &PyValue, right: &PyValue) -> Res<PyValue> {
    crate::pyexpr::mul(left.peel(), right.peel())
}

/// `left <op> right` of values read from JSON.
pub fn compare(
    operator: agent_runtime_support::pyops::Compare,
    left: &PyValue,
    right: &PyValue,
) -> Res<bool> {
    crate::pyexpr::compare(operator, left.peel(), right.peel())
}

/// The `dict` key a value stands for (`TypeError: unhashable type` for a list or dict): Python's identity and equality, see
/// `agent_runtime_support::pydyn::dict_key`.
pub fn dict_key_of(value: &PyValue) -> Res<DictKey> {
    static FRESH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    agent_runtime_support::pydyn::dict_key(
        value,
        FRESH.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    )
}

/// An insertion-ordered mapping keyed by any hashable Python value, with Python's equality (`1`, `1.0` and `True` are one
/// key; the first spelling stays). Lookup is linear: the maps here hold a handful of dtypes.
pub struct KeyMap<V> {
    entries: Vec<(PyValue, V)>,
}

impl<V> KeyMap<V> {
    pub fn new() -> Self {
        KeyMap {
            entries: Vec::new(),
        }
    }

    fn position(&self, key: &PyValue) -> Option<usize> {
        self.entries
            .iter()
            .position(|(held, _)| same_or_eq(held, key))
    }

    /// `map[key]` for reading or creating: the `TypeError` of an unhashable key comes first.
    pub fn entry(&mut self, key: &PyValue, make: impl FnOnce() -> V) -> Res<&mut V> {
        ensure_hashable(key)?;
        let at = match self.position(key) {
            Some(at) => at,
            None => {
                self.entries.push((key.clone(), make()));
                self.entries.len() - 1
            }
        };
        Ok(&mut self.entries[at].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = &(PyValue, V)> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<V> Default for KeyMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// A dict built from `(key, value)` pairs with keys of any hashable type, in order.
pub fn dict_from_pairs(pairs: Vec<(PyValue, PyValue)>) -> Res<PyDict> {
    let mut dict = PyDict::new();
    for (key, value) in pairs {
        dict.insert_key(dict_key_of(&key)?, value);
    }
    Ok(dict)
}

/// `str` sort order of two texts (code point order, as Python compares strings).
pub fn compare_text(a: &PyStr, b: &PyStr) -> std::cmp::Ordering {
    a.code_points().cmp(b.code_points())
}

/// `value[:-n]` / `rstrip` helpers on `PyStr`.
pub fn rstrip_slashes(text: &PyStr) -> PyStr {
    let slash = u32::from('/');
    let mut points = text.code_points().to_vec();
    while points.last() == Some(&slash) {
        points.pop();
    }
    PyStr::from_code_points(points)
}

/// `text.lstrip("/")`.
pub fn lstrip_slashes(text: &PyStr) -> PyStr {
    let slash = u32::from('/');
    let points = text.code_points();
    let start = points
        .iter()
        .position(|&c| c != slash)
        .unwrap_or(points.len());
    PyStr::from_code_points(points[start..].to_vec())
}

/// `text.startswith(prefix)`.
pub fn starts_with(text: &PyStr, prefix: &PyStr) -> bool {
    text.code_points().starts_with(prefix.code_points())
}

/// `text.endswith(suffix)`.
pub fn ends_with(text: &PyStr, suffix: &str) -> bool {
    let suffix: Vec<u32> = suffix.chars().map(u32::from).collect();
    text.code_points().ends_with(&suffix)
}

/// The text from code point `start` on.
pub fn tail_from(text: &PyStr, start: usize) -> PyStr {
    PyStr::from_code_points(text.code_points()[start.min(text.len())..].to_vec())
}

/// Whether `needle` occurs in `text` (`needle in text`).
pub fn contains(text: &PyStr, needle: &str) -> bool {
    let needle: Vec<u32> = needle.chars().map(u32::from).collect();
    if needle.is_empty() {
        return true;
    }
    text.code_points()
        .windows(needle.len())
        .any(|window| window == needle.as_slice())
}
