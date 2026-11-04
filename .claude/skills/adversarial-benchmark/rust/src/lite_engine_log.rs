//! The engine-log patterns of `lite_metrics.py`: the KV-cache, weight and non-torch figures of a serve log.
//!
//! The original runs `re.search` with these `str` patterns (Unicode rules: `\d` is a decimal digit, `\s` is
//! `str.isspace`; no flags, so literals are exact):
//!
//! ```text
//! KV priority 1  kv[_ ]cache[_ ]memory[_ ]?bytes['"]?\s*[=:]\s*([\d,]+)
//!             2  [Aa]vailable KV cache memory[:= ]+([\d.]+)\s*GiB
//!             3  reserved for KV [Cc]ache[:= ]+([\d.]+)\s*GiB
//!             4  reserved\s+([\d.]+)\s*GiB\s+memory\s+for\s+KV\s+[Cc]ache
//!             5  GPU KV cache (?:size|memory)[:= ]+([\d.]+)\s*GiB
//!             6  KV [Cc]ache[^\n]*?([\d.]+)\s*GiB
//! tokens        GPU KV cache size[:= ]+([\d,]+)\s*tokens
//! weights       [Mm]odel (?:weights take|loading took)[:= ]*([\d.]+)\s*GiB
//! non-torch     non[_-]?torch[^\n]*?([\d.]+)\s*GiB
//! ```
//!
//! Like [`crate::engine_log`] this is a hand-written matcher of the same language, not a regular-expression
//! engine. Every pattern is deterministic after its literal prefix except the lazy `[^\n]*?` of priority 6 and of
//! the non-torch pattern, which [`lazy_figure_then_gib`] resolves with the backtracking order of `re`: the
//! earliest position from which `([\d.]+)\s*GiB` matches wins, and `[\d.]+` is greedy. (A shorter run of
//! digits can never be followed by `\s*GiB`, because the next character would be a digit or a dot, so the maximal
//! run is the only candidate at a position.)
//!
//! The decoding of the file (`open(path, encoding="utf-8", errors="replace")` with universal newlines) is
//! [`decode_log_text`].

use crate::pystr::{is_decimal_digit, is_python_space};

type Text<'a> = &'a [char];
/// Range of the capture group in the text, `start..end` in characters.
type Group = (usize, usize);

/// Which capture the KV-memory priority list matched, and where.
pub fn first_kv_memory_group(text: Text<'_>) -> Option<String> {
    KV_MEMORY_PATTERNS
        .iter()
        .find_map(|pattern| search(text, *pattern))
        .map(|(start, end)| text[start..end].iter().collect())
}

/// `re.search(r"GPU KV cache size[:= ]+([\d,]+)\s*tokens", text)` group 1.
pub fn kv_tokens_group(text: Text<'_>) -> Option<String> {
    search(text, match_kv_tokens).map(|(start, end)| text[start..end].iter().collect())
}

/// `re.search(r"[Mm]odel (?:weights take|loading took)[:= ]*([\d.]+)\s*GiB", text)` group 1.
pub fn weights_group(text: Text<'_>) -> Option<String> {
    search(text, match_weights).map(|(start, end)| text[start..end].iter().collect())
}

/// `re.search(r"non[_-]?torch[^\n]*?([\d.]+)\s*GiB", text)` group 1.
pub fn nontorch_group(text: Text<'_>) -> Option<String> {
    search(text, match_nontorch).map(|(start, end)| text[start..end].iter().collect())
}

/// `bytes.decode("utf-8", errors="replace")` followed by the translation of `\r\n` and `\r` to `\n` that a text
/// file opened with the default `newline=None` applies.
pub fn decode_log_text(bytes: &[u8]) -> Vec<char> {
    let decoded = String::from_utf8_lossy(bytes);
    let mut text = Vec::with_capacity(decoded.len());
    let mut characters = decoded.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' {
            if characters.peek() == Some(&'\n') {
                characters.next();
            }
            text.push('\n');
        } else {
            text.push(character);
        }
    }
    text
}

const KV_MEMORY_PATTERNS: [fn(Text<'_>, usize) -> Option<Group>; 6] = [
    match_kv_memory_bytes,
    match_available_kv_cache,
    match_reserved_for_kv_cache,
    match_reserved_gib_memory_for_kv,
    match_gpu_kv_cache_size_or_memory,
    match_kv_cache_then_gib,
];

// ── pattern scaffolding ───────────────────────────────────────────────────────────

/// `re.search`: the match of the earliest start position at which `pattern` matches.
fn search(text: Text<'_>, pattern: impl Fn(Text<'_>, usize) -> Option<Group>) -> Option<Group> {
    (0..text.len()).find_map(|start| pattern(text, start))
}

fn is_space(character: char) -> bool {
    is_python_space(u32::from(character))
}

fn is_digit(character: char) -> bool {
    is_decimal_digit(u32::from(character))
}

fn is_digit_or_comma(character: char) -> bool {
    is_digit(character) || character == ','
}

fn is_digit_or_dot(character: char) -> bool {
    is_digit(character) || character == '.'
}

fn is_separator(character: char) -> bool {
    matches!(character, ':' | '=' | ' ')
}

/// The end of the exact literal at `at`.
fn literal(text: Text<'_>, at: usize, literal: &str) -> Option<usize> {
    let mut index = at;
    for expected in literal.chars() {
        if *text.get(index)? != expected {
            return None;
        }
        index += 1;
    }
    Some(index)
}

/// One character out of `choices`.
fn one_of(text: Text<'_>, at: usize, choices: &[char]) -> Option<usize> {
    text.get(at)
        .filter(|character| choices.contains(character))
        .map(|_| at + 1)
}

fn run(text: Text<'_>, mut at: usize, keep: impl Fn(char) -> bool) -> usize {
    while text.get(at).is_some_and(|&character| keep(character)) {
        at += 1;
    }
    at
}

/// A run of at least one character.
fn run_one_or_more(text: Text<'_>, at: usize, keep: impl Fn(char) -> bool) -> Option<usize> {
    let end = run(text, at, keep);
    (end > at).then_some(end)
}

/// `\s*GiB` at `at`; the end of the match.
fn spaces_then_gib(text: Text<'_>, at: usize) -> Option<usize> {
    literal(text, run(text, at, is_space), "GiB")
}

/// `([\d.]+)\s*GiB` at `at`: the group.
fn figure_then_gib(text: Text<'_>, at: usize) -> Option<Group> {
    let end = run_one_or_more(text, at, is_digit_or_dot)?;
    spaces_then_gib(text, end)?;
    Some((at, end))
}

/// `[^\n]*?([\d.]+)\s*GiB` from `from`: the lazy prefix grows one character at a time and never crosses a
/// newline, but the group and the `\s*` after it may.
fn lazy_figure_then_gib(text: Text<'_>, from: usize) -> Option<Group> {
    let mut at = from;
    loop {
        match figure_then_gib(text, at) {
            Some(group) => return Some(group),
            None => {
                let run_end = run(text, at, is_digit_or_dot);
                if run_end > at {
                    // Every later position inside this run ends the run in the same place with the same tail.
                    at = run_end;
                    continue;
                }
            }
        }
        if at >= text.len() || text[at] == '\n' {
            return None;
        }
        at += 1;
    }
}

// ── the patterns ──────────────────────────────────────────────────────────────────

/// `kv[_ ]cache[_ ]memory[_ ]?bytes['"]?\s*[=:]\s*([\d,]+)`
fn match_kv_memory_bytes(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = literal(text, start, "kv")?;
    at = one_of(text, at, &['_', ' '])?;
    at = literal(text, at, "cache")?;
    at = one_of(text, at, &['_', ' '])?;
    at = literal(text, at, "memory")?;
    at = one_of(text, at, &['_', ' ']).unwrap_or(at);
    at = literal(text, at, "bytes")?;
    at = one_of(text, at, &['\'', '"']).unwrap_or(at);
    at = run(text, at, is_space);
    at = one_of(text, at, &['=', ':'])?;
    at = run(text, at, is_space);
    let end = run_one_or_more(text, at, is_digit_or_comma)?;
    Some((at, end))
}

/// `[Aa]vailable KV cache memory[:= ]+([\d.]+)\s*GiB`
fn match_available_kv_cache(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = one_of(text, start, &['A', 'a'])?;
    at = literal(text, at, "vailable KV cache memory")?;
    at = run_one_or_more(text, at, is_separator)?;
    figure_then_gib(text, at)
}

/// `reserved for KV [Cc]ache[:= ]+([\d.]+)\s*GiB`
fn match_reserved_for_kv_cache(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = literal(text, start, "reserved for KV ")?;
    at = one_of(text, at, &['C', 'c'])?;
    at = literal(text, at, "ache")?;
    at = run_one_or_more(text, at, is_separator)?;
    figure_then_gib(text, at)
}

/// `reserved\s+([\d.]+)\s*GiB\s+memory\s+for\s+KV\s+[Cc]ache`
fn match_reserved_gib_memory_for_kv(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = literal(text, start, "reserved")?;
    at = run_one_or_more(text, at, is_space)?;
    let group = figure_then_gib(text, at)?;
    at = spaces_then_gib(text, group.1)?;
    at = run_one_or_more(text, at, is_space)?;
    at = literal(text, at, "memory")?;
    at = run_one_or_more(text, at, is_space)?;
    at = literal(text, at, "for")?;
    at = run_one_or_more(text, at, is_space)?;
    at = literal(text, at, "KV")?;
    at = run_one_or_more(text, at, is_space)?;
    at = one_of(text, at, &['C', 'c'])?;
    literal(text, at, "ache")?;
    Some(group)
}

/// `GPU KV cache (?:size|memory)[:= ]+([\d.]+)\s*GiB`
fn match_gpu_kv_cache_size_or_memory(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = literal(text, start, "GPU KV cache ")?;
    at = literal(text, at, "size").or_else(|| literal(text, at, "memory"))?;
    at = run_one_or_more(text, at, is_separator)?;
    figure_then_gib(text, at)
}

/// `KV [Cc]ache[^\n]*?([\d.]+)\s*GiB`
fn match_kv_cache_then_gib(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = literal(text, start, "KV ")?;
    at = one_of(text, at, &['C', 'c'])?;
    at = literal(text, at, "ache")?;
    lazy_figure_then_gib(text, at)
}

/// `GPU KV cache size[:= ]+([\d,]+)\s*tokens`
fn match_kv_tokens(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = literal(text, start, "GPU KV cache size")?;
    at = run_one_or_more(text, at, is_separator)?;
    let end = run_one_or_more(text, at, is_digit_or_comma)?;
    literal(text, run(text, end, is_space), "tokens")?;
    Some((at, end))
}

/// `[Mm]odel (?:weights take|loading took)[:= ]*([\d.]+)\s*GiB`
fn match_weights(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = one_of(text, start, &['M', 'm'])?;
    at = literal(text, at, "odel ")?;
    at = literal(text, at, "weights take").or_else(|| literal(text, at, "loading took"))?;
    at = run(text, at, is_separator);
    figure_then_gib(text, at)
}

/// `non[_-]?torch[^\n]*?([\d.]+)\s*GiB`
fn match_nontorch(text: Text<'_>, start: usize) -> Option<Group> {
    let mut at = literal(text, start, "non")?;
    at = one_of(text, at, &['_', '-']).unwrap_or(at);
    at = literal(text, at, "torch")?;
    lazy_figure_then_gib(text, at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn priority_follows_the_pattern_list_not_the_position() {
        let text = chars("KV cache 7.0 GiB\nkv_cache_memory_bytes: 21474836480\n");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("21474836480"));
        let text = chars("KV cache 7.0 GiB\nAvailable KV cache memory: 20.5 GiB\n");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("20.5"));
    }

    #[test]
    fn quoted_python_dict_key_still_matches() {
        let text = chars("{'kv_cache_memory_bytes': 21474836480}");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("21474836480"));
        let text = chars("\"kv cache memory bytes\"  =  1,024");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("1,024"));
    }

    #[test]
    fn vllm_026_reserved_phrase() {
        let text = chars("Initial free memory 113.47 GiB, reserved 20.0 GiB memory for KV Cache");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("20.0"));
    }

    #[test]
    fn lazy_prefix_stops_at_newline_but_the_tail_may_cross_it() {
        let text = chars("KV cache is 12\nGiB");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("12"));
        let text = chars("KV cache is big\n 12 GiB");
        assert_eq!(first_kv_memory_group(&text), None);
    }

    #[test]
    fn lazy_prefix_takes_the_earliest_figure() {
        let text = chars("KV cache 3 x 4.5 GiB");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("4.5"));
        let text = chars("KV cache 3.5 GiB of 8 GiB");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("3.5"));
    }

    #[test]
    fn a_matching_pattern_with_an_unparsable_group_still_wins() {
        let text = chars("Available KV cache memory: ... GiB\nKV cache 5 GiB");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some("..."));
        let text = chars("Available KV cache memory: .. GiB\nKV cache 5 GiB");
        assert_eq!(first_kv_memory_group(&text).as_deref(), Some(".."));
    }

    #[test]
    fn tokens_weights_and_nontorch() {
        assert_eq!(
            kv_tokens_group(&chars("GPU KV cache size: 154,192 tokens")).as_deref(),
            Some("154,192")
        );
        assert_eq!(
            weights_group(&chars("Model loading took 1.005 GiB")).as_deref(),
            Some("1.005")
        );
        assert_eq!(
            weights_group(&chars("model weights take12.5GiB")).as_deref(),
            Some("12.5")
        );
        assert_eq!(weights_group(&chars("Model weights taken 12.5 GiB")), None);
        assert_eq!(
            weights_group(&chars("Model weights take12.5GiB")).as_deref(),
            Some("12.5")
        );
        assert_eq!(
            nontorch_group(&chars("non-torch memory: 1.25 GiB")).as_deref(),
            Some("1.25")
        );
        assert_eq!(
            nontorch_group(&chars("nontorch 2 GiB")).as_deref(),
            Some("2")
        );
    }

    #[test]
    fn decoding_replaces_invalid_bytes_and_translates_newlines() {
        assert_eq!(
            decode_log_text(b"a\r\nb\rc\n\xffd"),
            chars("a\nb\nc\n\u{fffd}d")
        );
    }
}
