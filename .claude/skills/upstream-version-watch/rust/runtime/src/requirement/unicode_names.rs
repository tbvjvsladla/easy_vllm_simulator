//! The character-name lookup `\N{name}` of a Python string literal uses: `_getcode` of `Modules/unicodedata.c` (CPython 3.12.3, Unicode 15.0.0) with
//! named sequences not allowed (the string-literal decoder passes `with_named_seq = 0`).
//!
//! Derived from CPython 3.12.3 (PSF License Version 2); see `notices/NOTICE-unicode-names.md`. The tables are in [`super::unicode_names_data`],
//! copied from `unicodename_db.h` by `tools/gen_unicode_names.rs`.

use super::unicode_names_data::{
    ALIASES_END, ALIASES_START, CODE_HASH, CODE_MAGIC, CODE_POLY, CODE_SIZE, LEXICON,
    LEXICON_OFFSET, NAMED_SEQUENCES_END, NAMED_SEQUENCES_START, NAME_ALIASES, NAME_MAX_LENGTH,
    PHRASEBOOK, PHRASEBOOK_OFFSET1, PHRASEBOOK_OFFSET2, PHRASEBOOK_SHIFT, PHRASEBOOK_SHORT,
};

const HANGUL_BASE: u32 = 0xAC00;
const HANGUL_LEAD_COUNT: u32 = 19;
const HANGUL_VOWEL_COUNT: u32 = 21;
const HANGUL_TRAIL_COUNT: u32 = 28;
const HANGUL_NEUTRAL_COUNT: u32 = HANGUL_VOWEL_COUNT * HANGUL_TRAIL_COUNT;
const HANGUL_SYLLABLE_COUNT: u32 = HANGUL_LEAD_COUNT * HANGUL_NEUTRAL_COUNT;

/// `hangul_syllables[][3]`: lead, vowel and trail parts by index (`None` is the table's null entry).
const HANGUL_PARTS: [[Option<&str>; 3]; 28] = [
    [Some("G"), Some("A"), Some("")],
    [Some("GG"), Some("AE"), Some("G")],
    [Some("N"), Some("YA"), Some("GG")],
    [Some("D"), Some("YAE"), Some("GS")],
    [Some("DD"), Some("EO"), Some("N")],
    [Some("R"), Some("E"), Some("NJ")],
    [Some("M"), Some("YEO"), Some("NH")],
    [Some("B"), Some("YE"), Some("D")],
    [Some("BB"), Some("O"), Some("L")],
    [Some("S"), Some("WA"), Some("LG")],
    [Some("SS"), Some("WAE"), Some("LM")],
    [Some(""), Some("OE"), Some("LB")],
    [Some("J"), Some("YO"), Some("LS")],
    [Some("JJ"), Some("U"), Some("LT")],
    [Some("C"), Some("WEO"), Some("LP")],
    [Some("K"), Some("WE"), Some("LH")],
    [Some("T"), Some("WI"), Some("M")],
    [Some("P"), Some("YU"), Some("B")],
    [Some("H"), Some("EU"), Some("BS")],
    [None, Some("YI"), Some("S")],
    [None, Some("I"), Some("SS")],
    [None, None, Some("NG")],
    [None, None, Some("J")],
    [None, None, Some("C")],
    [None, None, Some("K")],
    [None, None, Some("T")],
    [None, None, Some("P")],
    [None, None, Some("H")],
];

/// `is_unified_ideograph` (the ranges of `makeunicodedata.py:cjk_ranges`).
fn is_unified_ideograph(code: u32) -> bool {
    matches!(code,
        0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0x20000..=0x2A6DF
        | 0x2A700..=0x2B739
        | 0x2B740..=0x2B81D
        | 0x2B820..=0x2CEA1
        | 0x2CEB0..=0x2EBE0
        | 0x30000..=0x3134A
        | 0x31350..=0x323AF)
}

fn is_alias(code: u32) -> bool {
    (ALIASES_START..ALIASES_END).contains(&code)
}

fn is_named_sequence(code: u32) -> bool {
    (NAMED_SEQUENCES_START..NAMED_SEQUENCES_END).contains(&code)
}

/// `_gethash`: the name hash over the ASCII-uppercased bytes.
fn name_hash(name: &[u8], scale: u64) -> u64 {
    let mut hash: u64 = 0;
    for &byte in name {
        hash = hash
            .wrapping_mul(scale)
            .wrapping_add(u64::from(byte.to_ascii_uppercase()));
        let high = hash & 0xff00_0000;
        if high != 0 {
            hash = (hash ^ ((high >> 24) & 0xff)) & 0x00ff_ffff;
        }
    }
    hash
}

/// `_getucname(code, buffer, NAME_MAXLEN, with_alias_and_seq = 1)`: the name of `code`, `None` where the C function returns 0.
fn name_of_code(code: u32) -> Option<Vec<u8>> {
    if code >= 0x110000 {
        return None;
    }
    if (HANGUL_BASE..HANGUL_BASE + HANGUL_SYLLABLE_COUNT).contains(&code) {
        let index = code - HANGUL_BASE;
        let lead = (index / HANGUL_NEUTRAL_COUNT) as usize;
        let vowel = ((index % HANGUL_NEUTRAL_COUNT) / HANGUL_TRAIL_COUNT) as usize;
        let trail = (index % HANGUL_TRAIL_COUNT) as usize;
        let mut name = b"HANGUL SYLLABLE ".to_vec();
        for (row, column) in [(lead, 0), (vowel, 1), (trail, 2)] {
            name.extend_from_slice(HANGUL_PARTS[row][column].unwrap_or("").as_bytes());
        }
        return Some(name);
    }
    if is_unified_ideograph(code) {
        return Some(format!("CJK UNIFIED IDEOGRAPH-{code:X}").into_bytes());
    }
    let block = PHRASEBOOK_OFFSET1[(code >> PHRASEBOOK_SHIFT) as usize] as usize;
    let mut offset = PHRASEBOOK_OFFSET2
        [(block << PHRASEBOOK_SHIFT) + (code & ((1 << PHRASEBOOK_SHIFT) - 1)) as usize]
        as usize;
    if offset == 0 {
        return None;
    }
    let mut name: Vec<u8> = Vec::new();
    loop {
        let first = i32::from(PHRASEBOOK[offset]) - i32::from(PHRASEBOOK_SHORT);
        let word = if first >= 0 {
            let word = ((first as usize) << 8) + usize::from(PHRASEBOOK[offset + 1]);
            offset += 2;
            word
        } else {
            let word = usize::from(PHRASEBOOK[offset]);
            offset += 1;
            word
        };
        if !name.is_empty() {
            if name.len() > NAME_MAX_LENGTH {
                return None;
            }
            name.push(b' ');
        }
        let mut at = LEXICON_OFFSET[word] as usize;
        while LEXICON[at] < 128 {
            if name.len() >= NAME_MAX_LENGTH {
                return None;
            }
            name.push(LEXICON[at]);
            at += 1;
        }
        if name.len() >= NAME_MAX_LENGTH {
            return None;
        }
        name.push(LEXICON[at] & 127);
        if LEXICON[at] == 128 {
            // the last word ends with the byte 0x80: its character is the terminating NUL of the C buffer, not part of the name
            name.pop();
            break;
        }
    }
    Some(name)
}

/// `_cmpname`: whether `code` has exactly this name (compared with the given name uppercased).
fn has_name(code: u32, name: &[u8]) -> bool {
    name_of_code(code).is_some_and(|known| {
        known.len() == name.len()
            && known
                .iter()
                .zip(name)
                .all(|(known, given)| *known == given.to_ascii_uppercase())
    })
}

/// `_check_alias_and_seq` with `with_named_seq = 0`.
fn resolve_alias(table_value: u32) -> Option<u32> {
    if is_named_sequence(table_value) {
        None
    } else if is_alias(table_value) {
        Some(NAME_ALIASES[(table_value - ALIASES_START) as usize])
    } else {
        Some(table_value)
    }
}

/// `find_syllable`: the longest part of `column` that `text` starts with (index and length), `(None, 0)` when none matches.
fn longest_syllable_part(text: &[u8], column: usize, count: usize) -> (Option<usize>, usize) {
    let mut best: Option<usize> = None;
    let mut best_len: Option<usize> = None;
    for (index, row) in HANGUL_PARTS.iter().take(count).enumerate() {
        let part = row[column].unwrap_or("").as_bytes();
        if best_len.is_some_and(|len| part.len() <= len) {
            continue;
        }
        if text.starts_with(part) {
            best = Some(index);
            best_len = Some(part.len());
        }
    }
    (best, best_len.unwrap_or(0))
}

fn hangul_code(name: &[u8]) -> Option<u32> {
    let mut rest = &name[b"HANGUL SYLLABLE ".len()..];
    let mut indexes = [0usize; 3];
    let counts = [
        HANGUL_LEAD_COUNT as usize,
        HANGUL_VOWEL_COUNT as usize,
        HANGUL_TRAIL_COUNT as usize,
    ];
    for column in 0..3 {
        let (index, length) = longest_syllable_part(rest, column, counts[column]);
        indexes[column] = index?;
        rest = &rest[length..];
    }
    rest.is_empty().then(|| {
        HANGUL_BASE
            + (indexes[0] as u32 * HANGUL_VOWEL_COUNT + indexes[1] as u32) * HANGUL_TRAIL_COUNT
            + indexes[2] as u32
    })
}

fn ideograph_code(name: &[u8]) -> Option<u32> {
    let digits = &name[b"CJK UNIFIED IDEOGRAPH-".len()..];
    if digits.len() != 4 && digits.len() != 5 {
        return None;
    }
    let mut code: u32 = 0;
    for &digit in digits {
        let value = match digit {
            b'0'..=b'9' => digit - b'0',
            b'A'..=b'F' => digit - b'A' + 10,
            _ => return None,
        };
        code = code * 16 + u32::from(value);
    }
    is_unified_ideograph(code).then_some(code)
}

/// `\N{name}`: the code point named `name` (ASCII bytes; names, aliases and the algorithmic Hangul/CJK names, not named sequences).
pub(super) fn code_of_name(name: &[u8]) -> Option<u32> {
    if name.starts_with(b"HANGUL SYLLABLE ") {
        return hangul_code(name);
    }
    if name.starts_with(b"CJK UNIFIED IDEOGRAPH-") {
        return ideograph_code(name);
    }
    let mask = CODE_SIZE - 1;
    let hash = name_hash(name, u64::from(CODE_MAGIC)) as u32;
    let mut index = !hash & mask;
    let mut value = CODE_HASH[index as usize];
    if value == 0 {
        return None;
    }
    if has_name(value, name) {
        return resolve_alias(value);
    }
    let mut increment = (hash ^ (hash >> 3)) & mask;
    if increment == 0 {
        increment = mask;
    }
    loop {
        index = (index.wrapping_add(increment)) & mask;
        value = CODE_HASH[index as usize];
        if value == 0 {
            return None;
        }
        if has_name(value, name) {
            return resolve_alias(value);
        }
        increment <<= 1;
        if increment > mask {
            increment ^= CODE_POLY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_name_is_found_in_any_letter_case() {
        assert_eq!(code_of_name(b"LATIN CAPITAL LETTER A"), Some(0x41));
        assert_eq!(code_of_name(b"latin small letter a"), Some(0x61));
    }

    #[test]
    fn an_alias_resolves_and_a_named_sequence_does_not() {
        assert_eq!(code_of_name(b"LINE FEED"), Some(0x0A));
        assert_eq!(
            code_of_name(b"LATIN CAPITAL LETTER A WITH MACRON AND GRAVE"),
            None
        );
    }

    #[test]
    fn algorithmic_names_are_case_sensitive_and_checked() {
        assert_eq!(code_of_name(b"HANGUL SYLLABLE GA"), Some(0xAC00));
        assert_eq!(code_of_name(b"hangul syllable ga"), None);
        assert_eq!(code_of_name(b"CJK UNIFIED IDEOGRAPH-4E00"), Some(0x4E00));
        assert_eq!(code_of_name(b"CJK UNIFIED IDEOGRAPH-0041"), None);
    }

    #[test]
    fn an_unknown_name_is_not_found() {
        assert_eq!(code_of_name(b"NO SUCH NAME"), None);
    }
}
