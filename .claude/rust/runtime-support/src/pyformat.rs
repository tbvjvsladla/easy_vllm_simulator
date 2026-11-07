//! Python's `str % float`, the printf-style formatting the report renderer applies to a caller-supplied format
//! string (`fnum(v, fmt)` does `fmt % float(v)`). Only a single non-tuple float argument is modelled, which is all
//! that call site can produce; every failure is the exception class and message CPython 3.12 raises.

use crate::failure::Failure;
use crate::pyops::Num;
use crate::pyvalue::{float_repr, PyValue};

#[derive(Clone, Copy, Default)]
struct Flags {
    left_justify: bool,
    force_sign: bool,
    space_sign: bool,
    alternate: bool,
    zero_pad: bool,
}

/// Parsed `%[flags][width][.precision][length]conversion`.
struct Spec {
    flags: Flags,
    width: usize,
    precision: Option<usize>,
}

fn type_error(message: &str) -> Failure {
    Failure::new("TypeError", message)
}

/// `fmt % value` where `fmt` is a string and `value` one float.
pub fn percent_float(fmt: &[u32], value: f64) -> Result<Vec<u32>, Failure> {
    let mut out: Vec<u32> = Vec::with_capacity(fmt.len() + 8);
    let mut index = 0;
    let mut consumed = false;
    while index < fmt.len() {
        if fmt[index] != u32::from('%') {
            out.push(fmt[index]);
            index += 1;
            continue;
        }
        index += 1;
        let incomplete = || Failure::new("ValueError", "incomplete format");
        let mut current = *fmt.get(index).ok_or_else(incomplete)?;
        if current == u32::from('%') {
            out.push(current);
            index += 1;
            continue;
        }
        if current == u32::from('(') {
            return Err(type_error("format requires a mapping"));
        }
        let mut flags = Flags::default();
        loop {
            match char::from_u32(current) {
                Some('-') => flags.left_justify = true,
                Some('+') => flags.force_sign = true,
                Some(' ') => flags.space_sign = true,
                Some('#') => flags.alternate = true,
                Some('0') => flags.zero_pad = true,
                _ => break,
            }
            index += 1;
            current = *fmt.get(index).ok_or_else(incomplete)?;
        }
        let mut width = 0usize;
        if current == u32::from('*') {
            take_argument(&mut consumed)?;
            return Err(type_error("* wants int"));
        }
        while let Some(digit) = digit_value(current) {
            width = width.saturating_mul(10).saturating_add(digit);
            if width > i32::MAX as usize {
                return Err(Failure::new("ValueError", "width too big"));
            }
            index += 1;
            current = *fmt.get(index).ok_or_else(incomplete)?;
        }
        let mut precision = None;
        if current == u32::from('.') {
            index += 1;
            current = *fmt.get(index).ok_or_else(incomplete)?;
            if current == u32::from('*') {
                take_argument(&mut consumed)?;
                return Err(type_error("* wants int"));
            }
            let mut digits = 0usize;
            while let Some(digit) = digit_value(current) {
                digits = digits.saturating_mul(10).saturating_add(digit);
                if digits > i32::MAX as usize {
                    return Err(Failure::new("ValueError", "precision too big"));
                }
                index += 1;
                current = *fmt.get(index).ok_or_else(incomplete)?;
            }
            precision = Some(digits);
        }
        if matches!(char::from_u32(current), Some('h' | 'l' | 'L')) {
            index += 1;
            current = *fmt.get(index).ok_or_else(incomplete)?;
        }
        let position = index;
        index += 1;
        let conversion = char::from_u32(current);
        let spec = Spec {
            flags,
            width,
            precision,
        };
        match conversion {
            Some('s' | 'r' | 'a') => {
                take_argument(&mut consumed)?;
                let text: Vec<u32> = float_repr(value).chars().map(u32::from).collect();
                let cut = match spec.precision {
                    Some(limit) if limit < text.len() => &text[..limit],
                    _ => &text[..],
                };
                pad_text(&mut out, cut, &spec);
            }
            Some('d' | 'i' | 'u') => {
                take_argument(&mut consumed)?;
                let integer = truncated_integer(value)?;
                let (negative, digits) = match integer.strip_prefix('-') {
                    Some(rest) => (true, rest.to_string()),
                    None => (false, integer),
                };
                let digits = match spec.precision {
                    Some(minimum) if digits.len() < minimum => {
                        format!("{}{digits}", "0".repeat(minimum - digits.len()))
                    }
                    _ => digits,
                };
                let sign = sign_text(negative, &spec.flags);
                pad_number(&mut out, sign, &digits, &spec, true);
            }
            Some(letter @ ('o' | 'x' | 'X')) => {
                take_argument(&mut consumed)?;
                return Err(type_error(&format!(
                    "%{letter} format: an integer is required, not float"
                )));
            }
            Some('c') => {
                take_argument(&mut consumed)?;
                return Err(type_error("%c requires int or char"));
            }
            Some(letter @ ('e' | 'E' | 'f' | 'F' | 'g' | 'G')) => {
                take_argument(&mut consumed)?;
                let negative = value.is_sign_negative() && !value.is_nan();
                let body = float_body(value.abs(), letter, &spec);
                let sign = sign_text(negative, &spec.flags);
                pad_number(&mut out, sign, &body, &spec, true);
            }
            _ => {
                let shown = match conversion {
                    Some(letter) => format!("'{letter}' (0x{:x})", current),
                    None => format!("'\\ud800' (0x{current:x})"),
                };
                return Err(Failure::new(
                    "ValueError",
                    &format!("unsupported format character {shown} at index {position}"),
                ));
            }
        }
    }
    if !consumed {
        return Err(type_error(
            "not all arguments converted during string formatting",
        ));
    }
    Ok(out)
}

fn digit_value(code_point: u32) -> Option<usize> {
    char::from_u32(code_point)
        .filter(char::is_ascii_digit)
        .map(|digit| digit as usize - '0' as usize)
}

/// `unicode_format_getnextarg` for one non-tuple argument: it can be taken once.
fn take_argument(consumed: &mut bool) -> Result<(), Failure> {
    if *consumed {
        return Err(type_error("not enough arguments for format string"));
    }
    *consumed = true;
    Ok(())
}

fn sign_text(negative: bool, flags: &Flags) -> &'static str {
    if negative {
        "-"
    } else if flags.force_sign {
        "+"
    } else if flags.space_sign {
        " "
    } else {
        ""
    }
}

/// `int(value)` of a float as decimal text (`%d`): infinity and NaN cannot be converted.
fn truncated_integer(value: f64) -> Result<String, Failure> {
    if value.is_nan() {
        return Err(Failure::new(
            "ValueError",
            "cannot convert float NaN to integer",
        ));
    }
    if value.is_infinite() {
        return Err(Failure::new(
            "OverflowError",
            "cannot convert float infinity to integer",
        ));
    }
    let number = crate::pynumber::int_from_float(value)
        .map_err(|error| Failure::new(error.class, &error.message))?;
    Ok(number.to_string())
}

/// Digits of a non-negative float for `e`, `f` or `g` (and the upper-case spellings); precision defaults to 6.
fn float_body(magnitude: f64, letter: char, spec: &Spec) -> String {
    let upper = letter.is_ascii_uppercase();
    let precision = spec.precision.unwrap_or(6);
    let text = if magnitude.is_nan() {
        "nan".to_string()
    } else if magnitude.is_infinite() {
        "inf".to_string()
    } else {
        match letter.to_ascii_lowercase() {
            'f' => fixed_text(magnitude, precision, spec.flags.alternate),
            'e' => exponent_text(magnitude, precision, spec.flags.alternate),
            _ => general_text(magnitude, precision, spec.flags.alternate),
        }
    };
    if upper {
        text.to_uppercase()
    } else {
        text
    }
}

fn fixed_text(magnitude: f64, precision: usize, alternate: bool) -> String {
    let mut text = format!("{magnitude:.precision$}");
    if alternate && precision == 0 {
        text.push('.');
    }
    text
}

/// `d.ddde+XX` the way C prints it: at least two exponent digits.
fn exponent_text(magnitude: f64, precision: usize, alternate: bool) -> String {
    let rendered = format!("{magnitude:.precision$e}");
    let (mantissa, exponent) = rendered
        .split_once('e')
        .expect("Rust's exponent format has an e");
    let exponent: i32 = exponent.parse().expect("exponent is an integer");
    let mut mantissa = mantissa.to_string();
    if alternate && precision == 0 {
        mantissa.push('.');
    }
    format!(
        "{mantissa}e{}{:02}",
        if exponent < 0 { '-' } else { '+' },
        exponent.abs()
    )
}

fn general_text(magnitude: f64, precision: usize, alternate: bool) -> String {
    let significant = precision.max(1);
    if magnitude == 0.0 {
        return if alternate {
            format!("{:.*}", significant - 1, 0.0)
        } else {
            "0".to_string()
        };
    }
    let rounded = format!("{magnitude:.*e}", significant - 1);
    let exponent: i32 = rounded
        .split_once('e')
        .and_then(|(_, tail)| tail.parse().ok())
        .expect("exponent is an integer");
    let text = if exponent < -4 || exponent >= significant as i32 {
        let mut text = exponent_text(magnitude, significant - 1, alternate);
        if !alternate {
            let (mantissa, tail) = text.split_once('e').expect("exponent form");
            let trimmed = trim_fraction_zeros(mantissa);
            text = format!("{trimmed}e{tail}");
        }
        text
    } else {
        let places = (significant as i32 - 1 - exponent) as usize;
        let text = format!("{magnitude:.places$}");
        if alternate {
            if places == 0 {
                format!("{text}.")
            } else {
                text
            }
        } else {
            trim_fraction_zeros(&text)
        }
    };
    text
}

fn trim_fraction_zeros(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text.to_string()
    }
}

fn pad_text(out: &mut Vec<u32>, text: &[u32], spec: &Spec) {
    let padding = spec.width.saturating_sub(text.len());
    if !spec.flags.left_justify {
        out.extend(std::iter::repeat_n(u32::from(' '), padding));
    }
    out.extend_from_slice(text);
    if spec.flags.left_justify {
        out.extend(std::iter::repeat_n(u32::from(' '), padding));
    }
}

/// Sign, then digits, padded to the width: zeros between sign and digits (unless left-justified), else spaces.
fn pad_number(out: &mut Vec<u32>, sign: &str, digits: &str, spec: &Spec, numeric: bool) {
    let length = sign.len() + digits.chars().count();
    let padding = spec.width.saturating_sub(length);
    let zero = numeric && spec.flags.zero_pad && !spec.flags.left_justify;
    if !spec.flags.left_justify && !zero {
        out.extend(std::iter::repeat_n(u32::from(' '), padding));
    }
    out.extend(sign.chars().map(u32::from));
    if zero {
        out.extend(std::iter::repeat_n(u32::from('0'), padding));
    }
    out.extend(digits.chars().map(u32::from));
    if spec.flags.left_justify {
        out.extend(std::iter::repeat_n(u32::from(' '), padding));
    }
}

/// `number % float` for a numeric left operand (`int`, `bool`, `float`): Python's floored float remainder.
pub fn float_remainder(left: &Num, right: f64) -> Result<PyValue, Failure> {
    let dividend = left
        .to_f64()
        .map_err(|error| Failure::new(error.class, &error.message))?;
    if right == 0.0 {
        return Err(Failure::new("ZeroDivisionError", "float modulo"));
    }
    let mut modulus = dividend % right;
    if modulus != 0.0 {
        if (right < 0.0) != (modulus < 0.0) {
            modulus += right;
        }
    } else {
        modulus = 0.0_f64.copysign(right);
    }
    Ok(PyValue::Float(modulus))
}
