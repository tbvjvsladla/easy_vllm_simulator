//! The few Python text operations the module's functions use, as Python defines them.

use crate::thrown::{Done, Thrown};
use crate::unicode_class::{ALPHA, DIGIT};
use crate::unicode_decimal::DECIMAL;
use agent_runtime_support::pynumber::int_from_str;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyValue, Typed};
use num_bigint::BigInt;

fn in_ranges(table: &[(u32, u32)], code_point: u32) -> bool {
    table
        .binary_search_by(|&(first, last)| {
            if code_point < first {
                std::cmp::Ordering::Greater
            } else if code_point > last {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// `str.isdigit()` of one character (decimal digits and the other digit characters such as `²`).
pub fn is_digit(code_point: u32) -> bool {
    in_ranges(DIGIT, code_point)
}

/// What `\d` of a `str` regular expression matches: `str.isdecimal()` of one character (Unicode `Nd`; `²` is a digit but not a decimal).
pub fn is_decimal(code_point: u32) -> bool {
    in_ranges(DECIMAL, code_point)
}

/// `str.isalpha()` of one character.
pub fn is_alpha(code_point: u32) -> bool {
    in_ranges(ALPHA, code_point)
}

/// `int(text)` of a text of digits: Unicode decimal digits count, a digit that is not decimal (`²`) is the `ValueError` Python raises.
pub fn int_of_digits(points: &[u32]) -> Done<BigInt> {
    int_from_str(&PyStr::from_code_points(points.to_vec()))
        .map_err(|error| Thrown::builtin(error.class, &error.message))
}

/// `str(value)`: a date prints its ISO date, a datetime its ISO form with a space, everything else as the common `str()` does.
pub fn py_str(value: &PyValue) -> PyStr {
    if let PyValue::Opaque(opaque) = value.peel() {
        match &opaque.typed {
            Typed::Date(iso) => return PyStr::new(iso),
            Typed::DateTime(iso) => return PyStr::new(&iso.replacen('T', " ", 1)),
            _ => {}
        }
    }
    value.display_text()
}

/// `text` with its code points taken from a slice.
pub fn text_of(points: &[u32]) -> PyStr {
    PyStr::from_code_points(points.to_vec())
}
