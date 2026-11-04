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
        if self.message.is_empty() {
            // Python prints just the class name when `str(exception)` is empty (e.g. `MemoryError`).
            f.write_str(self.class)
        } else {
            write!(f, "{}: {}", self.class, self.message)
        }
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
        PyValue::Bytes(bytes) => float_from_str(&latin1(bytes)).map_err(|error| {
            PyException::new(
                error.class,
                format!(
                    "could not convert string to float: {}",
                    crate::pyvalue::bytes_repr(bytes)
                ),
            )
        }),
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
        PyValue::Bytes(bytes) => int_from_str(&latin1(bytes)).map_err(|error| {
            if error.message.starts_with("Exceeds the limit") {
                error
            } else {
                PyException::new(
                    error.class,
                    format!(
                        "invalid literal for int() with base 10: {}",
                        crate::pyvalue::bytes_repr(bytes)
                            .chars()
                            .take(200)
                            .collect::<String>()
                    ),
                )
            }
        }),
        other => Err(PyException::new(
            "TypeError",
            format!(
                "int() argument must be a string, a bytes-like object or a real number, not '{}'",
                other.type_name()
            ),
        )),
    }
}

/// Python `round(number, 2)`; see [`round_to_places`].
pub fn round_to_two_places(number: f64) -> f64 {
    round_to_places(number, 2)
}

/// Python `round(number, places)` for a small positive `places`: the double nearest to the exact value rounded
/// half-to-even at that many decimals.
///
/// Python does not scale and round; it rounds the exact decimal expansion of the double (so `round(2.675, 2)` is
/// `2.67` because the double is below the tie, and `round(0.125, 2)` is `0.12` because it is exactly a tie), then
/// converts the decimal string back with correct rounding. This does the same with exact integer arithmetic.
pub fn round_to_places(number: f64, places: u32) -> f64 {
    if !number.is_finite() {
        return number;
    }
    let (negative, mantissa, exponent) = decompose(number);
    if let Some((units, unit)) = scaled_units(mantissa, exponent, places) {
        // `units / 10^places` of two exactly representable numbers is the correctly rounded decimal value (the
        // classic exact-operand division); the sign is symmetric, so -0.0 and tiny negatives stay negative zero
        if units < (1u128 << 53) && unit <= 10_000_000_000_000_000_000_000 {
            let value = units as f64 / unit as f64;
            return if negative { -value } else { value };
        }
    }
    fixed_text(number, places)
        .parse()
        .expect("a decimal literal parses")
}

/// Sign, mantissa and binary exponent of a finite double: `number = ±mantissa × 2^exponent` exactly (subnormals have
/// no implicit leading one).
fn decompose(number: f64) -> (bool, u64, i64) {
    let bits = number.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7FF) as i64;
    let fraction = bits & ((1u64 << 52) - 1);
    if exponent_bits == 0 {
        (bits >> 63 == 1, fraction, -1074)
    } else {
        (
            bits >> 63 == 1,
            fraction | (1u64 << 52),
            exponent_bits - 1075,
        )
    }
}

/// `round-half-even(mantissa × 2^exponent × 10^places)` and `10^places` in 128-bit arithmetic, or `None` when the
/// scaled value needs more than 128 bits (the caller then uses big integers).
fn scaled_units(mantissa: u64, exponent: i64, places: u32) -> Option<(u128, u128)> {
    if places > 22 {
        return None;
    }
    let unit = 10u128.pow(places);
    let scaled = u128::from(mantissa) * unit;
    let units = if exponent >= 0 {
        let shift = u32::try_from(exponent).ok()?;
        if shift > scaled.leading_zeros() {
            return None;
        }
        scaled << shift
    } else {
        let shift = u32::try_from(-exponent).ok()?;
        if shift >= 128 {
            // below one unit in the last place by a wide margin: no tie is possible, it rounds to zero
            0
        } else {
            let quotient = scaled >> shift;
            let remainder = scaled - (quotient << shift);
            let twice = remainder << 1;
            let divisor = 1u128 << shift;
            let round_up = twice > divisor || (twice == divisor && quotient & 1 == 1);
            quotient + u128::from(round_up)
        }
    };
    Some((units, unit))
}

/// Python `'%.{places}f' % number`: exact decimal expansion rounded half-to-even; `nan`, `inf`, `-inf` spelled
/// like Python.
pub fn format_fixed(number: f64, places: u32) -> String {
    if number.is_nan() {
        "nan".to_string()
    } else if number.is_infinite() {
        if number > 0.0 { "inf" } else { "-inf" }.to_string()
    } else {
        fixed_text(number, places)
    }
}

/// The decimal text of a finite double rounded to `places` decimals (see [`round_to_places`]).
fn fixed_text(number: f64, places: u32) -> String {
    let (negative, mantissa, exponent) = decompose(number);
    if let Some((units, unit)) = scaled_units(mantissa, exponent, places) {
        if places == 0 {
            // `'%.0f'` has no decimal point.
            return format!("{}{units}", if negative { "-" } else { "" });
        }
        return format!(
            "{}{}.{:0>width$}",
            if negative { "-" } else { "" },
            units / unit,
            units % unit,
            width = places as usize
        );
    }
    fixed_text_big(number, places)
}

/// [`fixed_text`] with big integers, for any magnitude and number of places.
fn fixed_text_big(number: f64, places: u32) -> String {
    let (negative, mantissa, exponent) = decompose(number);
    let unit = BigInt::from(10u8).pow(places);
    let scaled = BigInt::from(mantissa) * &unit;
    let units = if exponent >= 0 {
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
    if places == 0 {
        return format!("{}{units}", if negative { "-" } else { "" });
    }
    let (whole, fractional): (BigInt, BigInt) = (&units / &unit, &units % &unit);
    format!(
        "{}{whole}.{fractional:0>width$}",
        if negative { "-" } else { "" },
        width = places as usize
    )
}

/// The bytes as a string of code points 0..=255; `float()` and `int()` of `bytes` accept only ASCII, and every
/// other byte makes the literal invalid either way.
fn latin1(bytes: &[u8]) -> PyStr {
    PyStr::from_code_points(bytes.iter().map(|&b| u32::from(b)).collect())
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
    fn the_128_bit_path_equals_the_big_integer_path() {
        // a deterministic spread: random bit patterns, decimal-looking values, ties and subnormals
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut numbers = vec![
            0.0,
            -0.0,
            0.125,
            2.675,
            0.375,
            5e-324,
            -5e-324,
            1e22,
            1e23,
            f64::MAX,
            0.5,
            1.5,
            2.5,
        ];
        for _ in 0..20000 {
            let bits = next();
            let value = f64::from_bits(bits);
            if value.is_finite() {
                numbers.push(value);
            }
            let decimal = (next() % 2_000_000) as f64 / 1000.0 - 1000.0;
            numbers.push(decimal);
            numbers.push(decimal / 8.0);
            numbers.push(f64::from(next() as u32) / 1e6);
        }
        for number in numbers {
            for places in [0u32, 1, 2, 3, 5, 9, 22, 23, 30] {
                let exact = fixed_text_big(number, places);
                assert_eq!(fixed_text(number, places), exact, "{number:e} at {places}");
                let reference: f64 = exact.parse().unwrap();
                let rounded = round_to_places(number, places);
                assert_eq!(
                    rounded.to_bits(),
                    reference.to_bits(),
                    "{number:e} at {places}"
                );
            }
        }
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
