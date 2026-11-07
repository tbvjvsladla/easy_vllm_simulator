//! The few Python operators the verdict rule applies to values it does not control (JSON from files and API
//! callers): arithmetic and ordering across `bool`/`int`/`float` with CPython's exactness, and the `TypeError`s
//! other operand types raise. Only the operators the ported programs use are provided.

use crate::pynumber::PyException;
use crate::pyvalue::PyValue;
use num_bigint::BigInt;
use num_traits::FromPrimitive;
use std::cmp::Ordering;

/// A Python real number; `bool` counts as `int`.
#[derive(Clone, Debug)]
pub enum Num {
    Int(BigInt),
    Float(f64),
}

/// `isinstance(value, (int, float))` (so `True` is a number).
pub fn num_of(value: &PyValue) -> Option<Num> {
    match value {
        PyValue::Bool(flag) => Some(Num::Int(BigInt::from(u8::from(*flag)))),
        PyValue::Int(number) => Some(Num::Int(number.clone())),
        PyValue::Float(number) => Some(Num::Float(*number)),
        _ => None,
    }
}

impl Num {
    /// `float(number)`; `OverflowError` for an `int` beyond the double range.
    pub fn to_f64(&self) -> Result<f64, PyException> {
        match self {
            Num::Float(number) => Ok(*number),
            Num::Int(number) => crate::pynumber::float_from_int(number),
        }
    }

    pub fn into_value(self) -> PyValue {
        match self {
            Num::Int(number) => PyValue::Int(number),
            Num::Float(number) => PyValue::Float(number),
        }
    }
}

fn unsupported(operator: &str, left: &PyValue, right: &PyValue) -> PyException {
    PyException::new(
        "TypeError",
        format!(
            "unsupported operand type(s) for {operator}: '{}' and '{}'",
            left.type_name(),
            right.type_name()
        ),
    )
}

fn is_sequence(value: &PyValue) -> bool {
    matches!(value, PyValue::Str(_) | PyValue::List(_))
}

/// `left * right` for the operand types that can occur: numbers, or a sequence times a float.
pub fn multiply(left: &PyValue, right: &PyValue) -> Result<PyValue, PyException> {
    match (num_of(left), num_of(right)) {
        (Some(Num::Int(a)), Some(Num::Int(b))) => Ok(PyValue::Int(a * b)),
        (Some(a), Some(b)) => Ok(PyValue::Float(a.to_f64()? * b.to_f64()?)),
        _ => {
            let sequence_by_float = (is_sequence(left) && matches!(right, PyValue::Float(_)))
                || (is_sequence(right) && matches!(left, PyValue::Float(_)));
            if sequence_by_float {
                Err(PyException::new(
                    "TypeError",
                    "can't multiply sequence by non-int of type 'float'",
                ))
            } else {
                Err(unsupported("*", left, right))
            }
        }
    }
}

/// `left - right` for numbers.
pub fn subtract(left: &PyValue, right: &PyValue) -> Result<PyValue, PyException> {
    match (num_of(left), num_of(right)) {
        (Some(Num::Int(a)), Some(Num::Int(b))) => Ok(PyValue::Int(a - b)),
        (Some(a), Some(b)) => Ok(PyValue::Float(a.to_f64()? - b.to_f64()?)),
        _ => Err(unsupported("-", left, right)),
    }
}

/// True division `left / right`; the result is always a float.
pub fn divide(left: &PyValue, right: &PyValue) -> Result<f64, PyException> {
    match (num_of(left), num_of(right)) {
        (Some(a), Some(b)) => {
            let (a, b) = (a.to_f64()?, b.to_f64()?);
            if b == 0.0 {
                Err(PyException::new(
                    "ZeroDivisionError",
                    "float division by zero",
                ))
            } else {
                Ok(a / b)
            }
        }
        _ => Err(unsupported("/", left, right)),
    }
}

/// Exact ordering of two real numbers (`None` when a NaN is involved), as Python compares `int` with `float`.
fn order(left: &Num, right: &Num) -> Option<Ordering> {
    match (left, right) {
        (Num::Int(a), Num::Int(b)) => Some(a.cmp(b)),
        (Num::Float(a), Num::Float(b)) => a.partial_cmp(b),
        (Num::Int(a), Num::Float(b)) => int_against_float(a, *b),
        (Num::Float(a), Num::Int(b)) => int_against_float(b, *a).map(Ordering::reverse),
    }
}

fn int_against_float(number: &BigInt, float: f64) -> Option<Ordering> {
    if float.is_nan() {
        return None;
    }
    if float.is_infinite() {
        return Some(if float > 0.0 {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let truncated = BigInt::from_f64(float.trunc()).expect("finite");
    Some(match number.cmp(&truncated) {
        Ordering::Equal => {
            let fraction = float - float.trunc();
            if fraction > 0.0 {
                Ordering::Less
            } else if fraction < 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }
        other => other,
    })
}

/// The comparison operators the programs use.
#[derive(Clone, Copy, Debug)]
pub enum Compare {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl Compare {
    fn symbol(self) -> &'static str {
        match self {
            Compare::Less => "<",
            Compare::LessEqual => "<=",
            Compare::Greater => ">",
            Compare::GreaterEqual => ">=",
        }
    }
}

/// `left <op> right`; a NaN makes every ordering false, other operand types raise `TypeError`.
pub fn compare(operator: Compare, left: &PyValue, right: &PyValue) -> Result<bool, PyException> {
    if let (PyValue::Str(a), PyValue::Str(b)) = (left, right) {
        let ordering = a.code_points().cmp(b.code_points());
        return Ok(match operator {
            Compare::Less => ordering == Ordering::Less,
            Compare::LessEqual => ordering != Ordering::Greater,
            Compare::Greater => ordering == Ordering::Greater,
            Compare::GreaterEqual => ordering != Ordering::Less,
        });
    }
    let (Some(a), Some(b)) = (num_of(left), num_of(right)) else {
        return Err(PyException::new(
            "TypeError",
            format!(
                "'{}' not supported between instances of '{}' and '{}'",
                operator.symbol(),
                left.type_name(),
                right.type_name()
            ),
        ));
    };
    Ok(match (order(&a, &b), operator) {
        (None, _) => false,
        (Some(o), Compare::Less) => o == Ordering::Less,
        (Some(o), Compare::LessEqual) => o != Ordering::Greater,
        (Some(o), Compare::Greater) => o == Ordering::Greater,
        (Some(o), Compare::GreaterEqual) => o != Ordering::Less,
    })
}

/// `isinstance(value, (int, float)) and value > 0`, the guard the physical-limit check uses.
pub fn is_positive_number(value: &PyValue) -> Result<bool, PyException> {
    match num_of(value) {
        Some(_) => compare(Compare::Greater, value, &PyValue::int(0)),
        None => Ok(false),
    }
}

/// `'%.{places}f' % value` for a real number (`int` and `bool` are accepted, as `%f` converts them).
pub fn percent_fixed(value: &PyValue, places: u32) -> Result<String, PyException> {
    match num_of(value) {
        Some(number) => Ok(crate::pynumber::format_fixed(number.to_f64()?, places)),
        None => Err(PyException::new(
            "TypeError",
            format!("must be real number, not {}", value.type_name()),
        )),
    }
}

/// `int(value)` for a real number (`%d` and `int()` truncate).
pub fn trunc_to_int(value: &PyValue) -> Result<BigInt, PyException> {
    crate::pynumber::int_call(value)
}
