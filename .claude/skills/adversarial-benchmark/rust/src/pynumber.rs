//! Python's numeric conversions as the interpreter performs them on JSON values: `float(x)`, `int(x)` and
//! `round(x, 2)`.
//!
//! The conversions are specified by CPython 3.12.3 behaviour (`float_from_string_inner`,
//! `_PyUnicode_TransformDecimalAndSpaceToASCII`, `_Py_string_to_number_with_underscores`, `PyLong_FromString`,
//! `float.__round__`); this file re-implements the observable rules, not the C code. Exceptions are returned as
//! [`PyException`] values so a caller can handle some classes (`TypeError`, `ValueError`) and let the others
//! escape, exactly as the Python program does.

use crate::pystr::{decimal_digit_value, is_python_space, PyStr};
use crate::pyvalue::PyValue;
use num_bigint::BigInt;
use num_traits::{FromPrimitive, ToPrimitive};
use std::fmt;

/// `sys.get_int_max_str_digits()` default.
const MAX_INT_DIGITS: usize = 4300;

/// A Python exception: its class name and `str(exception)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyException {
    pub class: &'static str,
    pub message: String,
}

impl PyException {
    pub fn new(class: &'static str, message: impl Into<String>) -> Self {
        PyException {
            class,
            message: message.into(),
        }
    }

    /// The exceptions `except (TypeError, ValueError)` catches.
    pub fn is_type_or_value_error(&self) -> bool {
        matches!(self.class, "TypeError" | "ValueError")
    }
}

impl fmt::Display for PyException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class, self.message)
    }
}

impl std::error::Error for PyException {}

/// `_PyUnicode_TransformDecimalAndSpaceToASCII`: how `float(str)` and `int(str)` see their argument. ASCII text is
/// used as is; otherwise characters above U+007E become a space (Unicode whitespace), an ASCII digit (Unicode
/// decimal digit) or `?`, which ends the text because nothing after it can make the literal valid.
fn transform_decimal_and_space(text: &PyStr) -> Vec<u8> {
    let points = text.code_points();
    if points.iter().all(|&point| point < 128) {
        return points.iter().map(|&point| point as u8).collect();
    }
    let mut out = Vec::with_capacity(points.len());
    for &point in points {
        if point < 127 {
            out.push(point as u8);
        } else if is_python_space(point) {
            out.push(b' ');
        } else if let Some(digit) = decimal_digit_value(point) {
            out.push(b'0' + digit as u8);
        } else {
            out.push(b'?');
            break;
        }
    }
    out
}

/// `Py_ISSPACE`: the C-locale whitespace of byte strings (not the Unicode `isspace`).
fn is_ascii_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

fn float_value_error(original: &PyStr) -> PyException {
    PyException::new(
        "ValueError",
        format!("could not convert string to float: {}", original.repr()),
    )
}

/// `_Py_string_to_number_with_underscores`: an underscore is allowed only between two ASCII digits, and the text
/// must not hold NUL. Returns the text without underscores.
fn remove_underscores(text: &[u8]) -> Option<Vec<u8>> {
    if text.contains(&0) {
        return None;
    }
    if !text.contains(&b'_') {
        return Some(text.to_vec());
    }
    let mut out = Vec::with_capacity(text.len());
    let mut previous = 0u8;
    for &byte in text {
        if byte == b'_' {
            if !previous.is_ascii_digit() {
                return None;
            }
        } else {
            if previous == b'_' && !byte.is_ascii_digit() {
                return None;
            }
            out.push(byte);
        }
        previous = byte;
    }
    if previous == b'_' {
        return None;
    }
    Some(out)
}

/// `float(str)`.
pub fn float_from_str(original: &PyStr) -> Result<f64, PyException> {
    let invalid = || float_value_error(original);
    let ascii = transform_decimal_and_space(original);
    let cleaned = remove_underscores(&ascii).ok_or_else(invalid)?;
    let start = cleaned
        .iter()
        .position(|&byte| !is_ascii_space(byte))
        .ok_or_else(invalid)?;
    let end = cleaned
        .iter()
        .rposition(|&byte| !is_ascii_space(byte))
        .map_or(start, |last| last + 1);
    parse_float_literal(&cleaned[start..end]).ok_or_else(invalid)
}

/// The grammar `PyOS_string_to_double` accepts for the whole text: an optional sign, then `inf`, `infinity`,
/// `nan` (any case) or decimal digits with an optional fraction and exponent.
fn parse_float_literal(text: &[u8]) -> Option<f64> {
    let (negative, body) = match text.first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let word = body.to_ascii_lowercase();
    if word == b"inf" || word == b"infinity" {
        return Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if word == b"nan" {
        return Some(f64::NAN);
    }
    let digits = |from: usize| {
        body[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let integer = digits(0);
    let mut index = integer;
    let mut fraction = 0;
    if body.get(index) == Some(&b'.') {
        fraction = digits(index + 1);
        index += 1 + fraction;
    }
    if integer + fraction == 0 {
        return None;
    }
    if matches!(body.get(index), Some(b'e' | b'E')) {
        let mut after = index + 1;
        if matches!(body.get(after), Some(b'+' | b'-')) {
            after += 1;
        }
        let exponent = digits(after);
        if exponent == 0 {
            return None;
        }
        index = after + exponent;
    }
    if index != body.len() {
        return None;
    }
    let magnitude: f64 = std::str::from_utf8(body).ok()?.parse().ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

/// `float(int)`: correctly rounded, `OverflowError` when the value does not fit a double.
pub fn float_from_int(number: &BigInt) -> Result<f64, PyException> {
    match number.to_f64() {
        Some(value) if value.is_finite() => Ok(value),
        _ => Err(PyException::new(
            "OverflowError",
            "int too large to convert to float",
        )),
    }
}

/// `float(value)` for a JSON value.
pub fn float_call(value: &PyValue) -> Result<f64, PyException> {
    match value {
        PyValue::Bool(flag) => Ok(f64::from(u8::from(*flag))),
        PyValue::Int(number) => float_from_int(number),
        PyValue::Float(number) => Ok(*number),
        PyValue::Str(text) => float_from_str(text),
        other => Err(PyException::new(
            "TypeError",
            format!(
                "float() argument must be a string or a real number, not '{}'",
                other.type_name()
            ),
        )),
    }
}

fn int_value_error(original: &PyStr) -> PyException {
    PyException::new(
        "ValueError",
        // CPython formats the argument with `%.200R`: the repr is cut to 200 characters.
        format!(
            "invalid literal for int() with base 10: {}",
            original.repr().chars().take(200).collect::<String>()
        ),
    )
}

/// `int(str)` in base 10: surrounding ASCII whitespace, one sign, digits with single underscores between them.
/// A literal with more than 4300 digits is refused, but only once it is otherwise valid.
pub fn int_from_str(original: &PyStr) -> Result<BigInt, PyException> {
    let invalid = || int_value_error(original);
    let ascii = transform_decimal_and_space(original);
    let mut index = ascii
        .iter()
        .position(|&byte| !is_ascii_space(byte))
        .ok_or_else(invalid)?;
    let negative = ascii[index] == b'-';
    if matches!(ascii[index], b'+' | b'-') {
        index += 1;
    }
    let mut digits: Vec<u8> = Vec::new();
    let mut previous = 0u8;
    while let Some(&byte) = ascii.get(index) {
        if byte == b'_' {
            if !previous.is_ascii_digit() {
                return Err(invalid());
            }
        } else if byte.is_ascii_digit() {
            digits.push(byte);
        } else {
            break;
        }
        previous = byte;
        index += 1;
    }
    if digits.is_empty() || previous == b'_' {
        return Err(invalid());
    }
    if ascii[index..].iter().any(|&byte| !is_ascii_space(byte)) {
        return Err(invalid());
    }
    if digits.len() > MAX_INT_DIGITS {
        return Err(PyException::new(
            "ValueError",
            format!(
                "Exceeds the limit ({MAX_INT_DIGITS} digits) for integer string conversion: value has {} digits; use sys.set_int_max_str_digits() to increase the limit",
                digits.len()
            ),
        ));
    }
    let magnitude = BigInt::parse_bytes(&digits, 10).ok_or_else(invalid)?;
    Ok(if negative { -magnitude } else { magnitude })
}

/// `int(float)`: truncation toward zero, exact for any finite double.
pub fn int_from_float(number: f64) -> Result<BigInt, PyException> {
    if number.is_nan() {
        return Err(PyException::new(
            "ValueError",
            "cannot convert float NaN to integer",
        ));
    }
    BigInt::from_f64(number).ok_or_else(|| {
        PyException::new("OverflowError", "cannot convert float infinity to integer")
    })
}

/// `int(value)` for a JSON value.
pub fn int_call(value: &PyValue) -> Result<BigInt, PyException> {
    match value {
        PyValue::Bool(flag) => Ok(BigInt::from(u8::from(*flag))),
        PyValue::Int(number) => Ok(number.clone()),
        PyValue::Float(number) => int_from_float(*number),
        PyValue::Str(text) => int_from_str(text),
        other => Err(PyException::new(
            "TypeError",
            format!(
                "int() argument must be a string, a bytes-like object or a real number, not '{}'",
                other.type_name()
            ),
        )),
    }
}

/// Python `round(number, 2)`: the double nearest to the exact value rounded half-to-even at two decimals.
///
/// Python does not scale and round; it rounds the exact decimal expansion of the double (so `round(2.675, 2)` is
/// `2.67` because the double is below the tie, and `round(0.125, 2)` is `0.12` because it is exactly a tie), then
/// converts the decimal string back with correct rounding. This does the same with exact integer arithmetic.
pub fn round_to_two_places(number: f64) -> f64 {
    if !number.is_finite() {
        return number;
    }
    let bits = number.to_bits();
    let negative = bits >> 63 == 1;
    let exponent_bits = ((bits >> 52) & 0x7FF) as i64;
    let fraction = bits & ((1u64 << 52) - 1);
    // number = mantissa * 2^exponent exactly (subnormals have no implicit leading one).
    let (mantissa, exponent) = if exponent_bits == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), exponent_bits - 1075)
    };
    let scaled = BigInt::from(mantissa) * 100;
    let hundredths = if exponent >= 0 {
        scaled << exponent as usize
    } else {
        let shift = (-exponent) as usize;
        let quotient: BigInt = &scaled >> shift;
        let remainder = &scaled - (&quotient << shift);
        let twice: BigInt = remainder << 1usize;
        let divisor = BigInt::from(1) << shift;
        let round_up = match twice.cmp(&divisor) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => quotient.bit(0),
            std::cmp::Ordering::Less => false,
        };
        if round_up {
            quotient + 1
        } else {
            quotient
        }
    };
    let (whole, cents): (BigInt, BigInt) = (&hundredths / 100, &hundredths % 100);
    let cents = cents.to_u32().expect("remainder of 100 fits");
    let text = format!("{}{whole}.{cents:02}", if negative { "-" } else { "" });
    text.parse().expect("a decimal literal parses")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_of(text: &str) -> Option<f64> {
        float_from_str(&PyStr::new(text)).ok()
    }

    fn int_of(text: &str) -> Option<String> {
        int_from_str(&PyStr::new(text)).ok().map(|n| n.to_string())
    }

    #[test]
    fn float_grammar() {
        assert_eq!(float_of(" 2_0.5 "), Some(20.5));
        assert_eq!(float_of("1e5"), Some(100000.0));
        assert_eq!(float_of(".5"), Some(0.5));
        assert_eq!(float_of("5."), Some(5.0));
        assert_eq!(float_of("1_e5"), None);
        assert_eq!(float_of("1e_5"), None);
        assert_eq!(float_of("1__0"), None);
        assert_eq!(float_of("e5"), None);
        assert_eq!(float_of("1e"), None);
        assert_eq!(float_of("0x10"), None);
        assert_eq!(float_of("- 1"), None);
        assert_eq!(float_of("1e999"), Some(f64::INFINITY));
        assert!(float_of("-NaN").unwrap().is_nan());
        assert_eq!(float_of("iNfInItY"), Some(f64::INFINITY));
        assert_eq!(float_of("infin"), None);
        assert_eq!(float_of("\u{FF12}\u{FF10}"), Some(20.0));
        assert_eq!(float_of("\u{0661}\u{0662}.\u{0665}"), Some(12.5));
        assert_eq!(
            float_of("\u{1c}20\u{1f}"),
            None,
            "ASCII control separators are not whitespace for float()"
        );
        assert_eq!(float_of("\u{a0}1\u{2003}"), Some(1.0));
        assert_eq!(float_of(""), None);
        assert_eq!(float_of("   "), None);
    }

    #[test]
    fn int_grammar() {
        assert_eq!(int_of(" 12 "), Some("12".into()));
        assert_eq!(int_of("+7"), Some("7".into()));
        assert_eq!(int_of("-0"), Some("0".into()));
        assert_eq!(int_of("1_000"), Some("1000".into()));
        assert_eq!(int_of("1__0"), None);
        assert_eq!(int_of("_1"), None);
        assert_eq!(int_of("1_"), None);
        assert_eq!(int_of("- 5"), None);
        assert_eq!(int_of("2.0"), None);
        assert_eq!(int_of("\u{0663}\u{0663}"), Some("33".into()));
        assert_eq!(int_of("007"), Some("7".into()));
        let long = "9".repeat(4301);
        let error = int_from_str(&PyStr::new(&long)).unwrap_err();
        assert!(error.message.starts_with("Exceeds the limit (4300 digits)"));
        assert_eq!(
            int_from_str(&PyStr::new(&format!("{long}x")))
                .unwrap_err()
                .message,
            // CPython cuts the repr in the message to 200 characters (`%.200R`).
            format!(
                "invalid literal for int() with base 10: '{}",
                "9".repeat(199)
            ),
            "an invalid literal is reported before the digit limit"
        );
    }

    #[test]
    fn float_of_ints_rounds_to_nearest_even_and_overflows() {
        let big = BigInt::from(2u8).pow(53) + 1;
        assert_eq!(float_from_int(&big).unwrap(), 9007199254740992.0);
        let max_tie = BigInt::from(2u8).pow(1024) - BigInt::from(2u8).pow(970);
        assert_eq!(float_from_int(&max_tie).unwrap_err().class, "OverflowError");
        assert_eq!(
            float_from_int(&(max_tie - 1)).unwrap(),
            f64::MAX,
            "just below the tie rounds down to the largest double"
        );
    }

    #[test]
    fn int_of_floats_truncates_and_refuses_non_finite() {
        assert_eq!(int_from_float(2.8).unwrap(), BigInt::from(2));
        assert_eq!(int_from_float(-0.8).unwrap(), BigInt::from(0));
        assert_eq!(
            int_from_float(f64::NAN).unwrap_err().message,
            "cannot convert float NaN to integer"
        );
        assert_eq!(
            int_from_float(f64::NEG_INFINITY).unwrap_err().class,
            "OverflowError"
        );
        assert_eq!(int_from_float(1e20).unwrap(), BigInt::from(10u8).pow(20));
    }

    #[test]
    fn round_is_half_even_on_the_exact_value() {
        assert_eq!(round_to_two_places(0.125), 0.12);
        assert_eq!(round_to_two_places(0.375), 0.38);
        assert_eq!(round_to_two_places(0.625), 0.62);
        assert_eq!(round_to_two_places(2.675), 2.67);
        assert_eq!(round_to_two_places(1.005), 1.0);
        assert_eq!(round_to_two_places(1e300), 1e300);
        assert_eq!(round_to_two_places(1e-300), 0.0);
        assert!(round_to_two_places(-1e-9).is_sign_negative());
        assert_eq!(round_to_two_places(f64::INFINITY), f64::INFINITY);
        assert!(round_to_two_places(f64::NAN).is_nan());
        assert_eq!(round_to_two_places(50.0), 50.0);
        assert_eq!(round_to_two_places(5e-324), 0.0);
        assert_eq!(round_to_two_places(f64::MAX), f64::MAX);
    }
}
