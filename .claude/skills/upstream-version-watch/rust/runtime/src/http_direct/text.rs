//! The few `str` operations of CPython that the HTTP parsing depends on, over text decoded from ISO-8859-1 (so every code point is below U+0100).

use agent_runtime_support::pystr::is_python_space;

/// `bytes.decode("iso-8859-1")`.
pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| char::from(byte)).collect()
}

/// `str.isspace()` of one character.
pub fn is_space(character: char) -> bool {
    is_python_space(u32::from(character))
}

/// `str.strip()`.
pub fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `str.split(None, maxsplit)`: runs of whitespace separate fields, leading whitespace is skipped, and once `maxsplit` fields were cut the
/// remainder (with its trailing whitespace) is the last field unless it is only whitespace.
pub fn split_whitespace_max(text: &str, maxsplit: usize) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut position = 0;
    let mut remaining_splits = maxsplit;
    let at = |index: usize| text[index..].chars().next();
    let step = |index: usize| index + at(index).map_or(0, char::len_utf8);
    while remaining_splits > 0 {
        remaining_splits -= 1;
        while position < text.len() && at(position).is_some_and(is_space) {
            position = step(position);
        }
        if position == text.len() {
            return fields;
        }
        let begin = position;
        position = step(position);
        while position < text.len() && !at(position).is_some_and(is_space) {
            position = step(position);
        }
        fields.push(&text[begin..position]);
    }
    if position < text.len() {
        while position < text.len() && at(position).is_some_and(is_space) {
            position = step(position);
        }
        if position != text.len() {
            fields.push(&text[position..]);
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_with_a_limit_keeps_the_remainder_like_python() {
        assert_eq!(
            split_whitespace_max("HTTP/1.1 200 OK\r\n", 2),
            ["HTTP/1.1", "200", "OK\r\n"]
        );
        assert_eq!(
            split_whitespace_max("HTTP/1.1 200\r\n", 2),
            ["HTTP/1.1", "200"]
        );
        assert_eq!(
            split_whitespace_max("HTTP/1.1 200 \r\n", 2),
            ["HTTP/1.1", "200"]
        );
        assert_eq!(
            split_whitespace_max("HTTP/1.1 200 \r\n", 1),
            ["HTTP/1.1", "200 \r\n"]
        );
        assert_eq!(split_whitespace_max("hello\n", 1), ["hello"]);
        assert_eq!(split_whitespace_max("  a  b c ", 2), ["a", "b", "c "]);
        assert!(split_whitespace_max("", 2).is_empty());
    }

    #[test]
    fn latin1_keeps_every_byte() {
        assert_eq!(latin1(&[0x43, 0xe9]), "Cé");
        assert_eq!(strip("\u{a0} x\u{85}"), "x");
    }
}
