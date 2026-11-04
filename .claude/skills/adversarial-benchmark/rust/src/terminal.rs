//! The terminal width the original program lays its usage and help out for.
//!
//! argparse takes the width from `shutil.get_terminal_size()`, which (CPython 3.12.3 `Lib/shutil.py`) does this:
//!
//! 1. `int(os.environ["COLUMNS"])`; a missing or non-integer value counts as 0;
//! 2. if that is not positive, `os.get_terminal_size(sys.__stdout__.fileno())` - the physical size of file
//!    descriptor 1 only, never stdin or stderr; failure (not a terminal, closed descriptor) falls back to 80
//!    columns, and so does a reported width of 0.
//!
//! The environment is read from the running process only; nothing here changes the terminal, locale or any
//! profile.

use crate::pystr::{decimal_digit_value, is_python_space, PyStr};

const FALLBACK_COLUMNS: u32 = 80;
/// `sys.get_int_max_str_digits()` default: longer decimal strings make `int()` raise `ValueError`.
const MAX_INT_DIGITS: usize = 4300;

/// Terminal width in columns for this process.
pub fn columns() -> u32 {
    use std::os::unix::ffi::OsStrExt;
    let from_env =
        std::env::var_os("COLUMNS").map(|value| PyStr::from_surrogateescape(value.as_bytes()));
    resolve(from_env.as_ref(), stdout_columns)
}

/// `shutil.get_terminal_size().columns` given the `COLUMNS` value (if set) and a way to ask file descriptor 1.
pub fn resolve(columns_env: Option<&PyStr>, query_stdout: impl FnOnce() -> Option<u16>) -> u32 {
    match columns_env.and_then(parse_python_int) {
        Some(columns) if columns > 0 => u32::try_from(columns).unwrap_or(u32::MAX),
        _ => query_stdout()
            .filter(|&width| width > 0)
            .map_or(FALLBACK_COLUMNS, u32::from),
    }
}

/// `os.get_terminal_size(1).columns`, `None` when descriptor 1 is not a terminal.
fn stdout_columns() -> Option<u16> {
    rustix::termios::tcgetwinsize(std::io::stdout())
        .ok()
        .map(|size| size.ws_col)
}

/// Python `int(text)` for a decimal `str`, `None` where it raises `ValueError`. Values beyond `i128` saturate.
///
/// Accepts surrounding Unicode whitespace, a sign, Unicode decimal digits and single underscores between digits.
pub fn parse_python_int(text: &PyStr) -> Option<i128> {
    // `_PyUnicode_TransformDecimalAndSpaceToASCII`: ASCII (below DEL) is kept, other whitespace becomes a space,
    // other decimal digits become ASCII digits, anything else cannot be part of a number.
    let mut ascii = String::with_capacity(text.len());
    for &point in text.code_points() {
        if point < 127 {
            ascii.push(char::from(point as u8));
        } else if is_python_space(point) {
            ascii.push(' ');
        } else {
            ascii.push(char::from_digit(decimal_digit_value(point)?, 10)?);
        }
    }
    let trimmed = ascii.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r'));
    let (negative, body) = match trimmed.as_bytes().first()? {
        b'-' => (true, &trimmed[1..]),
        b'+' => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    let groups: Vec<&str> = body.split('_').collect();
    if groups
        .iter()
        .any(|group| group.is_empty() || !group.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    if groups.iter().map(|group| group.len()).sum::<usize>() > MAX_INT_DIGITS {
        return None;
    }
    let magnitude = groups
        .iter()
        .flat_map(|group| group.bytes())
        .fold(0_i128, |value, digit| {
            value
                .saturating_mul(10)
                .saturating_add(i128::from(digit - b'0'))
        });
    Some(if negative { -magnitude } else { magnitude })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(text: &str) -> Option<i128> {
        parse_python_int(&PyStr::new(text))
    }

    #[test]
    fn parses_what_python_int_accepts() {
        for (text, want) in [
            ("80", 80),
            (" 60 ", 60),
            ("+40", 40),
            ("4_0", 40),
            ("0_0_7", 7),
            ("٦٠", 60),
            ("６０", 60),
            ("\u{3000}８０\u{2003}", 80),
            ("\t\n 12\r", 12),
            ("-5", -5),
            ("007", 7),
        ] {
            assert_eq!(int(text), Some(want), "{text:?}");
        }
        assert_eq!(
            int(&"9".repeat(60)),
            Some(i128::MAX),
            "huge values saturate"
        );
    }

    #[test]
    fn rejects_what_python_int_rejects() {
        for text in [
            "", " ", "abc", "1e2", "0x50", "4__0", "_40", "40_", "4 0", "- 5", "+", "1.5", "٤٠x",
            "\u{7f}1", "\u{1c}5", "²",
        ] {
            assert_eq!(int(text), None, "{text:?}");
        }
        assert_eq!(
            int(&"1".repeat(4301)),
            None,
            "more than 4300 digits raises ValueError"
        );
        assert_eq!(int(&"1".repeat(4300)), Some(i128::MAX));
    }

    #[test]
    fn environment_first_then_stdout_then_eighty() {
        let never = || -> Option<u16> { panic!("a positive COLUMNS must not query the terminal") };
        assert_eq!(resolve(Some(&PyStr::new("٦٠")), never), 60);
        assert_eq!(resolve(Some(&PyStr::new(" 4_0 ")), never), 40);
        assert_eq!(
            resolve(
                Some(&PyStr::new("999999999999999999999999999999999999999")),
                never
            ),
            u32::MAX
        );
        for unusable in ["0", "-5", "abc", "", "1e2", "4__0"] {
            assert_eq!(
                resolve(Some(&PyStr::new(unusable)), || Some(55)),
                55,
                "{unusable:?} falls back to the terminal"
            );
            assert_eq!(
                resolve(Some(&PyStr::new(unusable)), || None),
                80,
                "{unusable:?} without a terminal"
            );
        }
        assert_eq!(resolve(None, || Some(55)), 55);
        assert_eq!(
            resolve(None, || Some(0)),
            80,
            "a terminal reporting zero columns falls back"
        );
        assert_eq!(resolve(None, || None), 80);
    }
}
