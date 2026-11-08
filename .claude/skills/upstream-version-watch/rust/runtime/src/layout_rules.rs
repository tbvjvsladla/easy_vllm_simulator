//! The fixed rules of `check_spec_layout.py` as pure functions over code points: the tripwire lists, the three regular expressions of the
//! source (matched by hand, with the backtracking the engine does), and the two numeric tests the command makes of `num_nextn_predict_layers`.
//!
//! Nothing here reads a file or prints. Python's `re` semantics that matter: `\s` is `str.isspace`, `\d` is `str.isdecimal` (Unicode `Nd`), `.`
//! is every character but `\n`, and `$` matches at the end or just before a final `\n`.

use crate::pytext::is_decimal;
use crate::thrown::{Done, Thrown};
use agent_runtime_support::pystr::{is_python_space, PyStr};
use agent_runtime_support::pyvalue::PyValue;
use num_bigint::BigInt;

/// `DSPARK_ONLY_MARKERS`: the parts only a DSpark speculator holds (a closed tripwire list; the source gives the reason).
pub const DSPARK_ONLY_MARKERS: [&str; 9] = [
    "main_norm",
    "main_proj",
    "markov_head",
    "markov_w1",
    "markov_w2",
    "confidence_head",
    "hc_head_base",
    "hc_head_fn",
    "hc_head_scale",
];

/// `PLAIN_MTP_COMPONENTS`: the parts the plain-MTP module holds.
pub const PLAIN_MTP_COMPONENTS: [&str; 15] = [
    "attn",
    "attn_norm",
    "input_layernorm",
    "ffn",
    "ffn_norm",
    "mlp",
    "norm",
    "emb",
    "embed_tokens",
    "head",
    "shared_head",
    "eh_proj",
    "enorm",
    "hnorm",
    "tok_emb",
];

/// `SPEC_KEY_RE.pattern` and `SPEC_KEY_RE.flags` (`re.UNICODE`, which a `str` pattern always carries).
pub const SPEC_KEY_PATTERN: &str = r"^(?:model\.layers\.\d+\.mtp_block|mtp\.(\d+))\.(.+)$";
pub const SPEC_KEY_FLAGS: i64 = 32;

const LF: u32 = 0x0A;

fn points_of(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

/// `text` with its code points taken from a slice.
pub fn text_of(points: &[u32]) -> PyStr {
    PyStr::from_code_points(points.to_vec())
}

fn literal_at(points: &[u32], at: usize, literal: &str) -> Option<usize> {
    let wanted = points_of(literal);
    points[at..]
        .starts_with(&wanted)
        .then_some(at + wanted.len())
}

fn skip_space(points: &[u32], mut at: usize) -> usize {
    while points.get(at).is_some_and(|&c| is_python_space(c)) {
        at += 1;
    }
    at
}

fn skip_decimal(points: &[u32], mut at: usize) -> usize {
    while points.get(at).is_some_and(|&c| is_decimal(c)) {
        at += 1;
    }
    at
}

/// `(.+)$` over `rest`: the greedy run of characters other than `\n`, which must end where the text ends or just before a final `\n`.
fn dot_plus_end(rest: &[u32]) -> Option<Vec<u32>> {
    let body = match rest.last() {
        Some(&LF) => &rest[..rest.len() - 1],
        _ => rest,
    };
    (!body.is_empty() && !body.contains(&LF)).then(|| body.to_vec())
}

/// What `SPEC_KEY_RE.match(key)` captures: the index of an `mtp.<digits>.` key (`None` for a `model.layers.<digits>.mtp_block.` key, which the
/// caller calls `block`) and the text after the dot.
pub struct SpecKey {
    pub index: Option<Vec<u32>>,
    pub rest: Vec<u32>,
}

/// `SPEC_KEY_RE.match(key)`.
pub fn match_spec_key(key: &[u32]) -> Option<SpecKey> {
    // `model\.layers\.\d+\.mtp_block`
    if let Some(after_layers) = literal_at(key, 0, "model.layers.") {
        let after_digits = skip_decimal(key, after_layers);
        if after_digits > after_layers {
            if let Some(after_block) = literal_at(key, after_digits, ".mtp_block") {
                if let Some(after_dot) = literal_at(key, after_block, ".") {
                    if let Some(rest) = dot_plus_end(&key[after_dot..]) {
                        return Some(SpecKey { index: None, rest });
                    }
                }
            }
        }
    }
    // `mtp\.(\d+)`
    let after_prefix = literal_at(key, 0, "mtp.")?;
    let after_digits = skip_decimal(key, after_prefix);
    if after_digits == after_prefix {
        return None;
    }
    let after_dot = literal_at(key, after_digits, ".")?;
    let rest = dot_plus_end(&key[after_dot..])?;
    Some(SpecKey {
        index: Some(key[after_prefix..after_digits].to_vec()),
        rest,
    })
}

/// `m.group(2).split(".")[0]`: the text up to the first dot (empty when the first character is the dot).
pub fn first_component(rest: &[u32]) -> Vec<u32> {
    let dot = u32::from('.');
    let end = rest.iter().position(|&c| c == dot).unwrap_or(rest.len());
    rest[..end].to_vec()
}

/// `re.match(r"\s*speculative[-_]config\s*:\s*(.+)$", line)`: the first group. The trailing `\s*` is greedy and gives characters back until
/// `(.+)$` can match (a line `speculative-config:   \n` therefore captures one space).
pub fn config_line_value(line: &[u32]) -> Option<Vec<u32>> {
    let at = skip_space(line, 0);
    let at = literal_at(line, at, "speculative")?;
    let separator = *line.get(at)?;
    if separator != u32::from('-') && separator != u32::from('_') {
        return None;
    }
    let at = literal_at(line, at + 1, "config")?;
    let at = skip_space(line, at);
    let at = literal_at(line, at, ":")?;
    let rest = &line[at..];
    let blank = skip_space(rest, 0);
    (0..=blank)
        .rev()
        .find_map(|taken| dot_plus_end(&rest[taken..]))
}

/// `group.strip().strip("'\"")`: surrounding whitespace, then any run of quote characters (single or double) at both ends.
pub fn unquote_value(group: &[u32]) -> Vec<u32> {
    let stripped = text_of(group).strip();
    let points = stripped.code_points();
    let is_quote = |c: &u32| *c == u32::from('\'') || *c == u32::from('"');
    let start = points
        .iter()
        .position(|c| !is_quote(c))
        .unwrap_or(points.len());
    let end = points
        .iter()
        .rposition(|c| !is_quote(c))
        .map_or(start, |last| last + 1);
    points[start..end.max(start)].to_vec()
}

/// `re.search(r'"method"\s*:\s*"([^"]+)"', raw)`: the first group of the first match, if any.
pub fn search_quoted_method(raw: &[u32]) -> Option<Vec<u32>> {
    let quote = u32::from('"');
    for start in 0..raw.len() {
        let Some(mut at) = literal_at(raw, start, "\"method\"") else {
            continue;
        };
        at = skip_space(raw, at);
        let Some(after_colon) = literal_at(raw, at, ":") else {
            continue;
        };
        at = skip_space(raw, after_colon);
        if raw.get(at) != Some(&quote) {
            continue;
        }
        let first = at + 1;
        let mut end = first;
        while end < raw.len() && raw[end] != quote {
            end += 1;
        }
        // `[^"]+` needs one character and the closing quote must follow it
        if end > first && end < raw.len() {
            return Some(raw[first..end].to_vec());
        }
    }
    None
}

/// `any(marker in component for marker in DSPARK_ONLY_MARKERS)`.
pub fn holds_dspark_marker(component: &[u32]) -> bool {
    DSPARK_ONLY_MARKERS.iter().any(|marker| {
        let wanted = points_of(marker);
        component
            .windows(wanted.len())
            .any(|window| window == wanted.as_slice())
    })
}

/// `component in PLAIN_MTP_COMPONENTS` (exact membership).
pub fn is_plain_component(component: &[u32]) -> bool {
    PLAIN_MTP_COMPONENTS
        .iter()
        .any(|plain| points_of(plain) == component)
}

/// `nextn in (1, None)`: tuple membership uses `==`, so `True` and `1.0` are `1`.
pub fn is_one_or_none(nextn: &PyValue) -> bool {
    match nextn.peel() {
        PyValue::None => true,
        PyValue::Int(number) => *number == BigInt::from(1),
        PyValue::Bool(flag) => *flag,
        PyValue::Float(number) => *number == 1.0,
        _ => false,
    }
}

/// `nextn - 1` for the value a JSON document holds: an `int`, a `float` or a `bool` subtract; any other type is the `TypeError` Python raises.
pub fn minus_one(nextn: &PyValue) -> Done<PyValue> {
    match nextn.peel() {
        PyValue::Int(number) => Ok(PyValue::Int(number - 1)),
        PyValue::Bool(flag) => Ok(PyValue::int(i64::from(*flag) - 1)),
        PyValue::Float(number) => Ok(PyValue::Float(number - 1.0)),
        other => Err(Thrown::builtin(
            "TypeError",
            &format!(
                "unsupported operand type(s) for -: '{}' and 'int'",
                other.type_name()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Vec<u32> {
        points_of(text)
    }

    fn matched(text: &str) -> Option<(Option<String>, String)> {
        match_spec_key(&key(text)).map(|found| {
            (
                found
                    .index
                    .map(|digits| text_of(&digits).to_utf8().unwrap()),
                text_of(&first_component(&found.rest)).to_utf8().unwrap(),
            )
        })
    }

    #[test]
    fn spec_keys_follow_the_regular_expression_on_unicode_end_dot_and_line_semantics() {
        assert_eq!(
            matched("mtp.0.attn.weight"),
            Some((Some("0".into()), "attn".into()))
        );
        assert_eq!(
            matched("model.layers.43.mtp_block.main_norm.weight"),
            Some((None, "main_norm".into()))
        );
        // Unicode decimal digits count for `\d`, with their leading zeros; a superscript is no decimal
        assert_eq!(
            matched("mtp.٠٣.ffn"),
            Some((Some("٠٣".into()), "ffn".into()))
        );
        assert_eq!(matched("mtp.²π.ffn"), None);
        assert_eq!(matched("mtp.007.x"), Some((Some("007".into()), "x".into())));
        // the part after the dot needs one character; a first character dot gives an empty component
        assert_eq!(matched("mtp.0."), None);
        assert_eq!(matched("mtp.0..x"), Some((Some("0".into()), String::new())));
        // `$` accepts one final line feed (not part of the group), nothing else; `.` never matches a line feed
        assert_eq!(
            matched("mtp.0.attn\n"),
            Some((Some("0".into()), "attn".into()))
        );
        assert_eq!(matched("mtp.0.attn\n\n"), None);
        assert_eq!(matched("mtp.0.at\ntn"), None);
        assert_eq!(matched("mtp.0.\n"), None);
        // a carriage return and the Unicode separators are ordinary characters
        assert_eq!(
            matched("mtp.0.at\rtn"),
            Some((Some("0".into()), "at\rtn".into()))
        );
        assert_eq!(
            matched("mtp.0.a\u{2028}b"),
            Some((Some("0".into()), "a\u{2028}b".into()))
        );
        // anchored at the start, exact prefix
        assert_eq!(matched("xmtp.0.attn"), None);
        assert_eq!(matched("model.layers.x.mtp_block.a"), None);
        assert_eq!(matched("model.layers.1.mtp_block"), None);
        assert_eq!(matched("mtp.0attn"), None);
    }

    #[test]
    fn a_config_line_gives_back_whitespace_until_the_group_can_match() {
        let group =
            |text: &str| config_line_value(&key(text)).map(|g| text_of(&g).to_utf8().unwrap());
        assert_eq!(
            group("speculative-config: {\"method\":\"mtp\"}\n"),
            Some("{\"method\":\"mtp\"}".into())
        );
        assert_eq!(group("  speculative_config :x"), Some("x".into()));
        assert_eq!(group("speculative-config:   \n"), Some(" ".into()));
        assert_eq!(group("speculative-config:\n"), None);
        assert_eq!(group("speculative-config:"), None);
        assert_eq!(group("speculative.config: x"), None);
        assert_eq!(group("# speculative-config: x"), None);
        assert_eq!(group("speculative-config: a  \n"), Some("a  ".into()));
        assert_eq!(
            group("\u{a0}speculative-config\u{2003}:\u{3000}z"),
            Some("z".into())
        );
    }

    #[test]
    fn values_are_stripped_of_whitespace_then_quote_runs_and_the_method_is_searched_by_the_source_pattern(
    ) {
        let clean = |text: &str| text_of(&unquote_value(&key(text))).to_utf8().unwrap();
        assert_eq!(clean(" '\"{x}\"' \n"), "{x}");
        assert_eq!(clean("''''"), "");
        assert_eq!(clean("a'b"), "a'b");
        let found =
            |text: &str| search_quoted_method(&key(text)).map(|g| text_of(&g).to_utf8().unwrap());
        assert_eq!(
            found("{\"method\" : \"mtp\", \"a\": 1}"),
            Some("mtp".into())
        );
        assert_eq!(found("{\"method\":\"\"}"), None);
        assert_eq!(found("{\"method\":\"open"), None);
        assert_eq!(found("{\"method\": 5, \"method\":\"b\"}"), Some("b".into()));
        assert_eq!(
            found("\"method\"\n:\n\"two\nlines\""),
            Some("two\nlines".into())
        );
    }

    #[test]
    fn the_nextn_tests_follow_python_equality_and_subtraction() {
        assert!(is_one_or_none(&PyValue::None));
        assert!(is_one_or_none(&PyValue::int(1)));
        assert!(is_one_or_none(&PyValue::Bool(true)));
        assert!(is_one_or_none(&PyValue::Float(1.0)));
        assert!(!is_one_or_none(&PyValue::Bool(false)));
        assert!(!is_one_or_none(&PyValue::str("1")));
        assert_eq!(minus_one(&PyValue::int(3)).unwrap().repr(), "2");
        assert_eq!(minus_one(&PyValue::Bool(false)).unwrap().repr(), "-1");
        assert_eq!(minus_one(&PyValue::Float(2.5)).unwrap().repr(), "1.5");
        let error = minus_one(&PyValue::str("2")).unwrap_err();
        assert_eq!(
            error.raised.message.to_utf8().unwrap(),
            "unsupported operand type(s) for -: 'str' and 'int'"
        );
    }
}
