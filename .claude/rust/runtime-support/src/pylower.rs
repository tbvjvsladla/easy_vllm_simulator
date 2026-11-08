//! `str.lower()` of CPython 3.12.3 (Unicode 15.0.0), for the texts the sweep producer lower-cases.
//!
//! Rust's `str::to_lowercase` follows a newer Unicode than the interpreter the original ran under (a character that
//! CPython 3.12.3 does not know, such as U+1C89, is mapped by it), so it is not used here. This is `unicode_lower` of
//! CPython: every scalar goes through the full lower-case mapping, one scalar becoming several for U+0130, and the capital
//! sigma is decided by the Final_Sigma context: it becomes the final form when a cased character precedes it and none
//! follows it, case-ignorable characters being skipped on both sides. The data is generated from the interpreter itself
//! (`unicode_lower_table`); there is no locale rule and no Python at run time.

use crate::unicode_lower_table::{CASED_NOT_SKIPPED, CASE_IGNORABLE, MULTI, SINGLE};

const CAPITAL_SIGMA: char = '\u{3A3}';
const SMALL_SIGMA: char = '\u{3C3}';
const FINAL_SIGMA: char = '\u{3C2}';

fn in_ranges(table: &[(u32, u32)], scalar: char) -> bool {
    let point = u32::from(scalar);
    table
        .binary_search_by(|&(first, last)| {
            if point < first {
                std::cmp::Ordering::Greater
            } else if point > last {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn is_skipped(scalar: char) -> bool {
    in_ranges(CASE_IGNORABLE, scalar)
}

fn is_cased(scalar: char) -> bool {
    // a case-ignorable character is skipped before it is asked whether it is cased, so only the others matter
    in_ranges(CASED_NOT_SKIPPED, scalar)
}

/// `handle_capital_sigma` of CPython: the sigma at `at` in `text`.
fn sigma_at(text: &[char], at: usize) -> char {
    let before = text[..at].iter().rev().find(|&&c| !is_skipped(c));
    if !before.is_some_and(|&c| is_cased(c)) {
        return SMALL_SIGMA;
    }
    let after = text[at + 1..].iter().find(|&&c| !is_skipped(c));
    if after.is_some_and(|&c| is_cased(c)) {
        SMALL_SIGMA
    } else {
        FINAL_SIGMA
    }
}

fn mapped(scalar: char, out: &mut String) {
    let point = u32::from(scalar);
    if let Ok(at) = SINGLE.binary_search_by_key(&point, |&(from, _)| from) {
        out.push(char::from_u32(SINGLE[at].1).expect("the table holds scalars"));
    } else if let Ok(at) = MULTI.binary_search_by_key(&point, |&(from, _)| from) {
        out.extend(
            MULTI[at]
                .1
                .iter()
                .map(|&to| char::from_u32(to).expect("the table holds scalars")),
        );
    } else {
        out.push(scalar);
    }
}

/// `text.lower()`.
pub fn python_lower(text: &str) -> String {
    let scalars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (at, &scalar) in scalars.iter().enumerate() {
        if scalar == CAPITAL_SIGMA {
            out.push(sigma_at(&scalars, at));
        } else {
            mapped(scalar, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::python_lower;

    #[test]
    fn the_unicode_15_boundary_and_the_one_to_many_mapping() {
        // U+1C89 is unassigned in Unicode 15 (CPython 3.12.3) and stays; U+1C89's newer pair is not applied
        assert_eq!(python_lower("X\u{1c89}"), "x\u{1c89}");
        assert_eq!(python_lower("\u{130}"), "i\u{307}");
        assert_eq!(python_lower("ABC\u{c9}"), "abc\u{e9}");
    }

    #[test]
    fn the_final_sigma_follows_the_cased_context_through_case_ignorable_characters() {
        for (given, want) in [
            ("\u{3a3}", "\u{3c3}"),
            ("A\u{3a3}", "a\u{3c2}"),
            ("A\u{3a3}B", "a\u{3c3}b"),
            ("A.\u{3a3}", "a.\u{3c2}"),
            ("A\u{345}\u{3a3}", "a\u{345}\u{3c2}"),
            ("A\u{3a3}\u{345}", "a\u{3c2}\u{345}"),
            ("A\u{3a3}\u{345}B", "a\u{3c3}\u{345}b"),
            ("1\u{3a3}", "1\u{3c3}"),
            ("A\u{3a3}1", "a\u{3c2}1"),
            ("\u{3a3}A", "\u{3c3}a"),
        ] {
            assert_eq!(python_lower(given), want, "{given:?}");
        }
    }
}
