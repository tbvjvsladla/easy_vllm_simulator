//! The engine-log scan of `parse_bench.py`: every `generation throughput: N tokens/s` figure in the text.
//!
//! The original is `re.compile(r"generation throughput:\s*([\d,]+(?:\.\d+)?)\s*tokens/s", re.IGNORECASE)` with
//! `findall`. This module is a hand-written matcher with the same language, not a regular-expression engine:
//! after the literal the pattern is deterministic (a run of whitespace, a run of digits and commas, an optional
//! fraction, a run of whitespace, a literal), so no backtracking is needed. For a `str` pattern Python's `re`
//! uses Unicode rules: `\s` is `str.isspace`, `\d` is a Unicode decimal digit, and `IGNORECASE` compares simple
//! lower-case mappings plus the equivalences `i`/`ı` and `s`/`ſ`.

use crate::pystr::{is_decimal_digit, is_python_space};

const PREFIX: &str = "generation throughput:";
const SUFFIX: &str = "tokens/s";

/// One pattern character against one text character under `re.IGNORECASE` (Unicode).
pub(crate) fn matches_ignoring_case(text: char, pattern: char) -> bool {
    match pattern {
        // The only characters whose simple lower-case mapping is `i` / `k` beyond ASCII are U+0130 (İ) and
        // U+212A (KELVIN SIGN); `re` also treats `ı` as `i` and `ſ` as `s` (its `_equivalences` table).
        'i' => matches!(text, 'i' | 'I' | '\u{130}' | '\u{131}'),
        'k' => matches!(text, 'k' | 'K' | '\u{212A}'),
        's' => matches!(text, 's' | 'S' | '\u{17F}'),
        letter if letter.is_ascii_alphabetic() => text.eq_ignore_ascii_case(&letter),
        other => text == other,
    }
}

fn is_space(character: char) -> bool {
    is_python_space(u32::from(character))
}

fn is_digit(character: char) -> bool {
    is_decimal_digit(u32::from(character))
}

fn literal_end(text: &[char], at: usize, literal: &str) -> Option<usize> {
    let mut index = at;
    for pattern in literal.chars() {
        if !matches_ignoring_case(*text.get(index)?, pattern) {
            return None;
        }
        index += 1;
    }
    Some(index)
}

fn skip_while(text: &[char], mut index: usize, keep: impl Fn(char) -> bool) -> usize {
    while text.get(index).is_some_and(|&c| keep(c)) {
        index += 1;
    }
    index
}

/// Tries the pattern at `start`; on success the capture group's range and the end of the whole match.
fn match_at(text: &[char], start: usize) -> Option<(usize, usize, usize)> {
    let after_prefix = literal_end(text, start, PREFIX)?;
    let group_start = skip_while(text, after_prefix, is_space);
    let mut index = skip_while(text, group_start, |c| is_digit(c) || c == ',');
    if index == group_start {
        return None;
    }
    if text.get(index) == Some(&'.') {
        let after_fraction = skip_while(text, index + 1, is_digit);
        if after_fraction > index + 1 {
            index = after_fraction;
        }
    }
    let group_end = index;
    let after_space = skip_while(text, group_end, is_space);
    let end = literal_end(text, after_space, SUFFIX)?;
    Some((group_start, group_end, end))
}

/// `findall`: the capture group of each non-overlapping match, left to right.
pub fn generation_throughputs(text: &[char]) -> Vec<String> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < text.len() {
        match match_at(text, at) {
            Some((first, last, end)) => {
                found.push(text[first..last].iter().collect());
                at = end;
            }
            None => at += 1,
        }
    }
    found
}

/// `bytes.decode("utf-8", errors="replace")` for the log file, as characters. The number of replacement
/// characters never matters to the pattern (U+FFFD is neither a digit, a space nor a pattern letter).
pub fn decode_replacing(bytes: &[u8]) -> Vec<char> {
    String::from_utf8_lossy(bytes).chars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> Vec<String> {
        generation_throughputs(&text.chars().collect::<Vec<_>>())
    }

    #[test]
    fn plain_and_repeated_matches() {
        assert_eq!(scan("generation throughput: 50 tokens/s\n"), ["50"]);
        assert_eq!(
            scan("generation throughput: 10 tokens/s\ngeneration throughput: 1,200.125 tokens/s\n"),
            ["10", "1,200.125"]
        );
        assert_eq!(scan("GENERATION THROUGHPUT:\t50.005 TOKENS/S"), ["50.005"]);
        assert_eq!(scan("generation throughput:5tokens/s"), ["5"]);
    }

    #[test]
    fn matches_are_consumed_left_to_right() {
        assert_eq!(
            scan("generation throughput: 5 tokens/sgeneration throughput: 7 tokens/s"),
            ["5", "7"]
        );
        assert_eq!(
            scan("generation throughput: generation throughput: 8 tokens/s"),
            ["8"]
        );
    }

    #[test]
    fn fraction_needs_digits_and_nothing_trails() {
        assert!(scan("generation throughput: 12. tokens/s").is_empty());
        assert!(scan("generation throughput: 12.5.6 tokens/s").is_empty());
        assert_eq!(scan("generation throughput: ,.5 tokens/s"), [",.5"]);
        assert_eq!(scan("generation throughput: ,,, tokens/s"), [",,,"]);
        assert!(scan("generation throughput: tokens/s").is_empty());
    }

    #[test]
    fn unicode_whitespace_digits_and_case() {
        assert_eq!(scan("generation throughput:\u{a0}5\u{3000}tokens/s"), ["5"]);
        assert_eq!(scan("generation throughput:\u{1c}5\u{1f}tokens/s"), ["5"]);
        assert!(scan("generation throughput:\u{200b}5 tokens/s").is_empty());
        assert_eq!(
            scan("generation throughput: \u{ff11}\u{ff12},\u{ff13}.\u{ff14} tokens/s"),
            ["\u{ff11}\u{ff12},\u{ff13}.\u{ff14}"]
        );
        assert!(scan("generation throughput: \u{b2} tokens/s").is_empty());
        assert_eq!(scan("GENERAT\u{130}ON THROUGHPUT: 5 TOKENS/S"), ["5"]);
        assert_eq!(scan("generat\u{131}on throughput: 5 tokens/s"), ["5"]);
        assert_eq!(scan("generation throughput: 5 token\u{17f}/\u{17f}"), ["5"]);
        assert_eq!(scan("generation throughput: 5 to\u{212a}ens/s"), ["5"]);
        assert!(scan("generation  throughput: 5 tokens/s").is_empty());
        assert!(scan("generation\u{a0}throughput: 5 tokens/s").is_empty());
    }

    #[test]
    fn replacement_decoding_keeps_the_surrounding_text() {
        let text = decode_replacing(
            b"generation thr\xffoughput: 5 tokens/s\ngeneration throughput: 7 tokens/s\n",
        );
        assert_eq!(generation_throughputs(&text), ["7"]);
    }
}
