//! The `repr()` spellings the parser's error messages embed: strings, single characters and key tuples. The quoting and escaping rules are those of
//! `str.__repr__`, taken from the common package.

use super::flags::Key;
use agent_runtime_support::pystr::PyStr;

/// `repr(text)` for a string given as code points.
pub fn text_repr(points: &[u32]) -> String {
    PyStr::from_code_points(points.to_vec()).repr()
}

/// `repr(text)` for a string literal of the parser (an expected delimiter).
pub fn string_repr(text: &str) -> String {
    PyStr::new(text).repr()
}

/// `repr(character)` for one code point.
pub fn single_char_repr(point: u32) -> String {
    text_repr(&[point])
}

/// `repr(key)` of a key tuple: `('a',)`, `('a', 'b')`, `()`.
pub fn key_repr(key: &Key) -> String {
    let parts: Vec<String> = key.iter().map(|part| text_repr(part)).collect();
    match parts.as_slice() {
        [only] => format!("({only},)"),
        _ => format!("({})", parts.join(", ")),
    }
}
