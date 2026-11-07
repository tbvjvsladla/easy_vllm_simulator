//! Python `re` matching semantics on top of the maintained `fancy-regex` 0.19.2 core (exact pin, features `std`/`unicode`/`perf`).
//!
//! The pattern is parsed by [`crate::py_regex_parse`] (the CPython 3.12.3 parser, errors included) and then *translated*, node by node, into core syntax
//! that carries **no** crate-defined Unicode or case meaning: every character class is computed here as explicit code-point ranges from the
//! interpreter's own tables (`\w` = `str.isalnum()` or `_`, `\d` = decimal digits, `\s` = `Py_UNICODE_ISSPACE`, from `agent-runtime-support`; simple lower
//! case from its `str.lower()` tables, Unicode 15.0.0) and the `_casefix` equivalences of `re/_casefix.py`, and every literal is a `\x{..}` escape. The
//! crate therefore contributes only the matching machinery (backtracking, look-around, back-references, atomic groups), not its own `\w` or
//! case-folding, which follow a different Unicode revision.
//!
//! What this module adds on top of the core, as `re` defines it: `^`/`$`/`\Z`/`\b` as Python spells them, `IGNORECASE` with the `_casefix` pairs,
//! `ASCII`-mode classes, positions in code points, and `finditer`'s empty-match rule (after an empty match a non-empty match may start at the same
//! position; the retry is a second compiled form that rejects a match ending where the search started, with `\G`).
//!
//! Named boundaries (reported, never faked): a back-reference under `IGNORECASE` (the core compares exactly), text with a lone surrogate (the core
//! needs scalar values), the core's own compile or backtrack limits, and the `LOCALE` flag of `bytes` patterns.

use crate::py_regex_parse::{
    parse_pattern, At, Category, Node, ParseFailure, ParsedPattern, RepeatKind, SetItem,
    FLAG_ASCII, FLAG_DOTALL, FLAG_IGNORECASE, FLAG_LOCALE, FLAG_MULTILINE, FLAG_UNICODE, MAXREPEAT,
};
use agent_runtime_support::pylower::python_lower;
use agent_runtime_support::pystr::{is_decimal_digit, is_python_space, is_python_word, PyStr};
use std::sync::OnceLock;

const MAX_POINT: u32 = 0x10FFFF;

/// A set of code points as sorted, disjoint, non-adjacent inclusive intervals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeSet(Vec<(u32, u32)>);

impl CodeSet {
    pub fn new() -> CodeSet {
        CodeSet(Vec::new())
    }

    pub fn from_predicate(predicate: impl Fn(u32) -> bool) -> CodeSet {
        let mut set = CodeSet::new();
        let mut open: Option<u32> = None;
        for point in 0..=MAX_POINT {
            let hit = !(0xD800..=0xDFFF).contains(&point) && predicate(point);
            match (hit, open) {
                (true, None) => open = Some(point),
                (false, Some(start)) => {
                    set.0.push((start, point - 1));
                    open = None;
                }
                _ => {}
            }
        }
        if let Some(start) = open {
            set.0.push((start, MAX_POINT));
        }
        set
    }

    pub fn add(&mut self, low: u32, high: u32) {
        self.0.push((low, high));
        self.normalize();
    }

    fn normalize(&mut self) {
        self.0.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(self.0.len());
        for &(low, high) in &self.0 {
            match merged.last_mut() {
                Some(last) if low <= last.1.saturating_add(1) => last.1 = last.1.max(high),
                _ => merged.push((low, high)),
            }
        }
        self.0 = merged;
    }

    pub fn union_with(&mut self, other: &CodeSet) {
        self.0.extend_from_slice(&other.0);
        self.normalize();
    }

    pub fn complement(&self) -> CodeSet {
        let mut out = Vec::new();
        let mut next = 0u32;
        for &(low, high) in &self.0 {
            if low > next {
                out.push((next, low - 1));
            }
            next = high.saturating_add(1);
        }
        if next <= MAX_POINT {
            out.push((next, MAX_POINT));
        }
        CodeSet(out)
    }

    pub fn contains(&self, point: u32) -> bool {
        self.0
            .binary_search_by(|&(low, high)| {
                if point < low {
                    std::cmp::Ordering::Greater
                } else if point > high {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Core class syntax: explicit ranges, with the surrogate block removed (the core holds scalar values only).
    fn class(&self) -> String {
        let mut clipped = self.clone();
        clipped.0 = {
            let mut out = Vec::new();
            for &(low, high) in &self.0 {
                if high < 0xD800 || low > 0xDFFF {
                    out.push((low, high));
                } else {
                    if low < 0xD800 {
                        out.push((low, 0xD7FF));
                    }
                    if high > 0xDFFF {
                        out.push((0xE000, high));
                    }
                }
            }
            out
        };
        if clipped.0.is_empty() {
            return "[^\\x{0}-\\x{10FFFF}]".to_string();
        }
        let mut text = String::from("[");
        for &(low, high) in &clipped.0 {
            if low == high {
                text.push_str(&format!("\\x{{{low:X}}}"));
            } else {
                text.push_str(&format!("\\x{{{low:X}}}-\\x{{{high:X}}}"));
            }
        }
        text.push(']');
        text
    }
}

fn unicode_word() -> &'static CodeSet {
    static SET: OnceLock<CodeSet> = OnceLock::new();
    SET.get_or_init(|| CodeSet::from_predicate(is_python_word))
}
fn unicode_digit() -> &'static CodeSet {
    static SET: OnceLock<CodeSet> = OnceLock::new();
    SET.get_or_init(|| CodeSet::from_predicate(is_decimal_digit))
}
fn unicode_space() -> &'static CodeSet {
    static SET: OnceLock<CodeSet> = OnceLock::new();
    SET.get_or_init(|| CodeSet::from_predicate(is_python_space))
}

fn ascii_word() -> CodeSet {
    CodeSet::from_predicate(|c| {
        c < 128 && (c == 95 || char::from_u32(c).is_some_and(|c| c.is_ascii_alphanumeric()))
    })
}
fn ascii_digit() -> CodeSet {
    CodeSet::from_predicate(|c| (48..=57).contains(&c))
}
fn ascii_space() -> CodeSet {
    CodeSet::from_predicate(|c| matches!(c, 9..=13 | 32))
}

fn category_set(category: Category, unicode: bool) -> CodeSet {
    let (base, negated) = match category {
        Category::Digit => (
            if unicode {
                unicode_digit().clone()
            } else {
                ascii_digit()
            },
            false,
        ),
        Category::NotDigit => (
            if unicode {
                unicode_digit().clone()
            } else {
                ascii_digit()
            },
            true,
        ),
        Category::Space => (
            if unicode {
                unicode_space().clone()
            } else {
                ascii_space()
            },
            false,
        ),
        Category::NotSpace => (
            if unicode {
                unicode_space().clone()
            } else {
                ascii_space()
            },
            true,
        ),
        Category::Word => (
            if unicode {
                unicode_word().clone()
            } else {
                ascii_word()
            },
            false,
        ),
        Category::NotWord => (
            if unicode {
                unicode_word().clone()
            } else {
                ascii_word()
            },
            true,
        ),
    };
    if negated {
        base.complement()
    } else {
        base
    }
}

/// `(character, simple lower case)` for every character whose simple lower case is another character, from the interpreter's `str.lower()` tables
/// (a one-scalar lower case; for U+0130 and the capital sigma, the first scalar of the full mapping, which is `_PyUnicode_ToLowercase`).
fn lower_pairs() -> &'static Vec<(u32, u32)> {
    static PAIRS: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        let mut pairs = Vec::new();
        for point in 0..=MAX_POINT {
            let Some(c) = char::from_u32(point) else {
                continue;
            };
            if point < 0x41 || (0x7B..0xC0).contains(&point) {
                continue; // no case mapping below 0x41 or in 0x7B..0xBF except none (0xB5 maps upward only)
            }
            let lowered = python_lower(&c.to_string());
            let first = lowered.chars().next().map(u32::from).unwrap_or(point);
            if first != point {
                pairs.push((point, first));
            }
        }
        pairs
    })
}

fn simple_lower(point: u32, ascii_only: bool) -> u32 {
    if ascii_only {
        return if (65..=90).contains(&point) {
            point + 32
        } else {
            point
        };
    }
    if point < 0x41 {
        return point;
    }
    match lower_pairs().binary_search_by_key(&point, |&(c, _)| c) {
        Ok(index) => lower_pairs()[index].1,
        Err(_) => point,
    }
}

/// `re/_casefix.py` `_EXTRA_CASES` (CPython 3.12.3): the characters that match each other under `IGNORECASE` although their lower cases differ.
const EXTRA_CASES: &[(u32, &[u32])] = &[
    (0x0069, &[0x0131]),
    (0x0073, &[0x017f]),
    (0x00b5, &[0x03bc]),
    (0x0131, &[0x0069]),
    (0x017f, &[0x0073]),
    (0x0345, &[0x03b9, 0x1fbe]),
    (0x0390, &[0x1fd3]),
    (0x03b0, &[0x1fe3]),
    (0x03b2, &[0x03d0]),
    (0x03b5, &[0x03f5]),
    (0x03b8, &[0x03d1]),
    (0x03b9, &[0x0345, 0x1fbe]),
    (0x03ba, &[0x03f0]),
    (0x03bc, &[0x00b5]),
    (0x03c0, &[0x03d6]),
    (0x03c1, &[0x03f1]),
    (0x03c2, &[0x03c3]),
    (0x03c3, &[0x03c2]),
    (0x03c6, &[0x03d5]),
    (0x03d0, &[0x03b2]),
    (0x03d1, &[0x03b8]),
    (0x03d5, &[0x03c6]),
    (0x03d6, &[0x03c0]),
    (0x03f0, &[0x03ba]),
    (0x03f1, &[0x03c1]),
    (0x03f5, &[0x03b5]),
    (0x0432, &[0x1c80]),
    (0x0434, &[0x1c81]),
    (0x043e, &[0x1c82]),
    (0x0441, &[0x1c83]),
    (0x0442, &[0x1c84, 0x1c85]),
    (0x044a, &[0x1c86]),
    (0x0463, &[0x1c87]),
    (0x1c80, &[0x0432]),
    (0x1c81, &[0x0434]),
    (0x1c82, &[0x043e]),
    (0x1c83, &[0x0441]),
    (0x1c84, &[0x0442, 0x1c85]),
    (0x1c85, &[0x0442, 0x1c84]),
    (0x1c86, &[0x044a]),
    (0x1c87, &[0x0463]),
    (0x1c88, &[0xa64b]),
    (0x1e61, &[0x1e9b]),
    (0x1e9b, &[0x1e61]),
    (0x1fbe, &[0x0345, 0x03b9]),
    (0x1fd3, &[0x0390]),
    (0x1fe3, &[0x03b0]),
    (0xa64b, &[0x1c88]),
    (0xfb05, &[0xfb06]),
    (0xfb06, &[0xfb05]),
];

fn fixes_of(point: u32) -> &'static [u32] {
    EXTRA_CASES
        .iter()
        .find(|(key, _)| *key == point)
        .map_or(&[][..], |(_, values)| values)
}

/// What a case-insensitive item matches: every character whose lower case is in `lowered` (`IN_UNI_IGNORE` lowers the subject first).
fn case_closure(lowered: &CodeSet, ascii_only: bool) -> CodeSet {
    let mut result = CodeSet::new();
    if ascii_only {
        for &(low, high) in &lowered.0 {
            result.0.push((low, high));
        }
        result.normalize();
        for upper in 65u32..=90 {
            if lowered.contains(upper + 32) {
                result.add(upper, upper);
            }
        }
        return result;
    }
    // characters that are their own lower case
    let changing = CodeSet::from_predicate(|c| {
        lower_pairs()
            .binary_search_by_key(&c, |&(known, _)| known)
            .is_ok()
    });
    let own = changing.complement();
    let mut kept = CodeSet::new();
    for &(low, high) in &lowered.0 {
        kept.0.push((low, high));
    }
    kept.normalize();
    // keep only members that are their own lower case: intersection with `own` = complement of (changing ∪ complement(kept))
    let mut dropped = changing.clone();
    dropped.union_with(&kept.complement());
    let mut closure = dropped.complement();
    let _ = own;
    for &(source, target) in lower_pairs() {
        if lowered.contains(target) {
            closure.add(source, source);
        }
    }
    closure
}

/// The lowered, fix-expanded member set of literals and ranges (the charmap of `_optimize_charset`).
fn lowered_members(items: &[SetItem], ascii_only: bool) -> CodeSet {
    let mut set = CodeSet::new();
    for item in items {
        match *item {
            SetItem::Literal(point) => {
                let lowered = simple_lower(point, ascii_only);
                set.add(lowered, lowered);
                if !ascii_only {
                    for &extra in fixes_of(lowered) {
                        set.add(extra, extra);
                    }
                }
            }
            SetItem::Range(low, high) => {
                for point in low..=high {
                    let lowered = simple_lower(point, ascii_only);
                    set.0.push((lowered, lowered));
                    if !ascii_only {
                        for &extra in fixes_of(lowered) {
                            set.0.push((extra, extra));
                        }
                    }
                }
                set.normalize();
            }
            SetItem::Category(_) => {}
        }
    }
    set
}

#[derive(Clone, Copy)]
struct Mode {
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    unicode: bool,
}

impl Mode {
    fn of(flags: u32, is_text: bool) -> Mode {
        let ascii = flags & FLAG_ASCII != 0 || !is_text;
        Mode {
            ignore_case: flags & FLAG_IGNORECASE != 0,
            multiline: flags & FLAG_MULTILINE != 0,
            dotall: flags & FLAG_DOTALL != 0,
            unicode: !ascii && (flags & FLAG_UNICODE != 0 || is_text),
        }
    }
    fn combine(self, add: u32, del: u32, is_text: bool) -> Mode {
        let mut flags = 0u32;
        if self.ignore_case {
            flags |= FLAG_IGNORECASE;
        }
        if self.multiline {
            flags |= FLAG_MULTILINE;
        }
        if self.dotall {
            flags |= FLAG_DOTALL;
        }
        if self.unicode {
            flags |= FLAG_UNICODE;
        } else if is_text {
            flags |= FLAG_ASCII;
        }
        let combined = (flags | add) & !del;
        Mode::of(combined, is_text)
    }
}

/// Why a pattern or a search is not handled.
#[derive(Clone, Debug)]
pub enum EngineFailure {
    Parse(ParseFailure),
    /// A named boundary of the profile.
    Unsupported {
        boundary: &'static str,
        detail: String,
    },
}

impl From<ParseFailure> for EngineFailure {
    fn from(failure: ParseFailure) -> EngineFailure {
        EngineFailure::Parse(failure)
    }
}

fn boundary(boundary: &'static str, detail: impl Into<String>) -> EngineFailure {
    EngineFailure::Unsupported {
        boundary,
        detail: detail.into(),
    }
}

struct Translator {
    is_text: bool,
}

fn escaped(point: u32) -> String {
    format!("\\x{{{point:X}}}")
}

impl Translator {
    fn word_class(&self, mode: Mode) -> String {
        category_set(Category::Word, mode.unicode).class()
    }

    fn set_class(&self, negate: bool, items: &[SetItem], mode: Mode) -> String {
        let mut members = CodeSet::new();
        let mut has_members = false;
        for item in items {
            match *item {
                SetItem::Literal(point) => {
                    has_members = true;
                    members.add(point, point);
                }
                SetItem::Range(low, high) => {
                    has_members = true;
                    members.add(low, high);
                }
                SetItem::Category(_) => {}
            }
        }
        let mut categories = CodeSet::new();
        for item in items {
            if let SetItem::Category(category) = *item {
                categories.union_with(&category_set(category, mode.unicode));
            }
        }
        let mut matched = if mode.ignore_case && has_members {
            let lowered = lowered_members(items, !mode.unicode);
            let mut all = lowered;
            all.union_with(&categories);
            case_closure(&all, !mode.unicode)
        } else {
            members.union_with(&categories);
            members
        };
        if negate {
            matched = matched.complement();
        }
        matched.class()
    }

    fn literal(&self, point: u32, negate: bool, mode: Mode) -> String {
        if mode.ignore_case {
            let lowered = simple_lower(point, !mode.unicode);
            let mut set = CodeSet::new();
            set.add(lowered, lowered);
            if mode.unicode {
                for &extra in fixes_of(lowered) {
                    set.add(extra, extra);
                }
            }
            let mut matched = case_closure(&set, !mode.unicode);
            if negate {
                matched = matched.complement();
            }
            return matched.class();
        }
        if negate {
            let mut set = CodeSet::new();
            set.add(point, point);
            return set.complement().class();
        }
        escaped(point)
    }

    fn seq(&self, seq: &[Node], mode: Mode, out: &mut String) -> Result<(), EngineFailure> {
        for node in seq {
            self.node(node, mode, out)?;
        }
        Ok(())
    }

    fn group(&self, seq: &[Node], mode: Mode, out: &mut String) -> Result<(), EngineFailure> {
        out.push_str("(?:");
        self.seq(seq, mode, out)?;
        out.push(')');
        Ok(())
    }

    fn node(&self, node: &Node, mode: Mode, out: &mut String) -> Result<(), EngineFailure> {
        match node {
            Node::Literal(point) => out.push_str(&self.literal(*point, false, mode)),
            Node::NotLiteral(point) => out.push_str(&self.literal(*point, true, mode)),
            Node::Any => {
                if mode.dotall {
                    out.push_str("[\\x{0}-\\x{10FFFF}]");
                } else {
                    out.push_str("[^\\x{A}]");
                }
            }
            Node::Set { negate, items } => out.push_str(&self.set_class(*negate, items, mode)),
            Node::Category(category) => {
                out.push_str(&category_set(*category, mode.unicode).class())
            }
            Node::At(kind) => match kind {
                At::Beginning => out.push_str(if mode.multiline { "(?m:^)" } else { "\\A" }),
                At::BeginningString => out.push_str("\\A"),
                At::End => out.push_str(if mode.multiline {
                    "(?m:$)"
                } else {
                    "(?=\\n?\\z)"
                }),
                At::EndString => out.push_str("\\z"),
                At::Boundary | At::NonBoundary => {
                    let word = self.word_class(mode);
                    let boundary = matches!(kind, At::Boundary);
                    let body = if boundary {
                        format!("(?:(?<={word})(?!{word})|(?<!{word})(?={word}))")
                    } else {
                        format!("(?:(?<={word})(?={word})|(?<!{word})(?!{word}))")
                    };
                    // `AT_BOUNDARY`/`AT_NON_BOUNDARY` fail on the empty string
                    out.push_str(&format!("(?!\\A\\z){body}"));
                }
            },
            Node::Branch(items) => {
                out.push_str("(?:");
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push('|');
                    }
                    self.seq(item, mode, out)?;
                }
                out.push(')');
            }
            Node::Sub {
                group,
                add,
                del,
                body,
            } => {
                let inner = mode.combine(*add, *del, self.is_text);
                if let Some(number) = group {
                    out.push_str(&format!("(?P<g{number}>"));
                    self.seq(body, inner, out)?;
                    out.push(')');
                } else {
                    self.group(body, inner, out)?;
                }
            }
            Node::Repeat {
                kind,
                min,
                max,
                body,
            } => {
                if *kind == RepeatKind::Possessive {
                    out.push_str("(?>");
                }
                self.group(body, mode, out)?;
                let upper = if *max == MAXREPEAT {
                    String::new()
                } else {
                    max.to_string()
                };
                out.push_str(&format!("{{{min},{upper}}}"));
                match kind {
                    RepeatKind::Lazy => out.push('?'),
                    RepeatKind::Possessive => out.push(')'),
                    RepeatKind::Greedy => {}
                }
            }
            Node::GroupRef(number) => {
                if mode.ignore_case {
                    return Err(boundary(
                        "backreference_ignorecase",
                        "a back-reference under IGNORECASE compares case-insensitively; the core compares exactly",
                    ));
                }
                out.push_str(&format!("(?P=g{number})"));
            }
            Node::Exists { group, yes, no } => {
                out.push_str(&format!("(?({group})"));
                self.seq(yes, mode, out)?;
                if let Some(no) = no {
                    out.push('|');
                    self.seq(no, mode, out)?;
                }
                out.push(')');
            }
            Node::Assert {
                ahead,
                negate,
                body,
            } => {
                out.push_str(match (ahead, negate) {
                    (true, false) => "(?=",
                    (true, true) => "(?!",
                    (false, false) => "(?<=",
                    (false, true) => "(?<!",
                });
                self.seq(body, mode, out)?;
                out.push(')');
            }
            Node::Atomic(body) => {
                out.push_str("(?>");
                self.seq(body, mode, out)?;
                out.push(')');
            }
        }
        Ok(())
    }
}

/// A compiled Python pattern.
pub struct CompiledPattern {
    first: fancy_regex::Regex,
    after_empty: fancy_regex::Regex,
    anchored: fancy_regex::Regex,
    pub is_text: bool,
    pub groups: usize,
}

/// A pattern to compile: `str` text or `bytes`.
pub enum PatternSource<'a> {
    Text(&'a PyStr),
    Bytes(&'a [u8]),
}

fn core_error(error: &fancy_regex::Error) -> EngineFailure {
    boundary(
        "regex_core_compile",
        format!("the matching core refused the translated pattern: {error}"),
    )
}

/// `re.compile(pattern)` with no flags argument.
pub fn compile(source: PatternSource<'_>) -> Result<CompiledPattern, EngineFailure> {
    let (points, is_text) = match source {
        PatternSource::Text(text) => (text.code_points().to_vec(), true),
        PatternSource::Bytes(bytes) => (bytes.iter().map(|&b| u32::from(b)).collect(), false),
    };
    let ParsedPattern {
        body,
        flags,
        groups,
        is_text,
    } = parse_pattern(points, is_text, 0)?;
    if flags & FLAG_LOCALE != 0 {
        return Err(boundary(
            "locale_flag",
            "the LOCALE flag depends on the process locale",
        ));
    }
    let translator = Translator { is_text };
    let mut text = String::new();
    translator.seq(&body, Mode::of(flags, is_text), &mut text)?;
    let first = fancy_regex::Regex::new(&format!("(?:{text})")).map_err(|e| core_error(&e))?;
    let after_empty =
        fancy_regex::Regex::new(&format!("(?:{text})(?!\\G)")).map_err(|e| core_error(&e))?;
    let anchored =
        fancy_regex::Regex::new(&format!("\\A(?:{text})")).map_err(|e| core_error(&e))?;
    Ok(CompiledPattern {
        first,
        after_empty,
        anchored,
        is_text,
        groups,
    })
}

/// The matches of `finditer`, spans in code points (bytes for a `bytes` subject).
pub struct Matches<'a> {
    pattern: &'a CompiledPattern,
    haystack: String,
    /// code-point index for each byte offset that starts a character (and the end)
    point_at: Vec<usize>,
    position: usize,
    must_advance: bool,
    finished: bool,
}

impl CompiledPattern {
    pub fn finditer_text<'a>(&'a self, subject: &PyStr) -> Result<Matches<'a>, EngineFailure> {
        let mut haystack = String::new();
        let mut point_at = Vec::new();
        for (index, &point) in subject.code_points().iter().enumerate() {
            let Some(c) = char::from_u32(point) else {
                return Err(boundary(
                    "lone_surrogate_subject",
                    "the matching core holds scalar values only; a lone surrogate in the searched text is not modelled",
                ));
            };
            for _ in 0..c.len_utf8() {
                point_at.push(index);
            }
            haystack.push(c);
        }
        point_at.push(subject.len());
        Ok(self.matches_over(haystack, point_at))
    }

    pub fn finditer_bytes<'a>(&'a self, subject: &[u8]) -> Matches<'a> {
        let mut haystack = String::new();
        let mut point_at = Vec::new();
        for (index, &byte) in subject.iter().enumerate() {
            let c = char::from(byte);
            for _ in 0..c.len_utf8() {
                point_at.push(index);
            }
            haystack.push(c);
        }
        point_at.push(subject.len());
        self.matches_over(haystack, point_at)
    }

    fn matches_over<'a>(&'a self, haystack: String, point_at: Vec<usize>) -> Matches<'a> {
        Matches {
            pattern: self,
            haystack,
            point_at,
            position: 0,
            must_advance: false,
            finished: false,
        }
    }
}

impl Matches<'_> {
    /// The next match as `(start, end)` in code points.
    pub fn next_span(&mut self) -> Result<Option<(usize, usize)>, EngineFailure> {
        if self.finished || self.position > self.haystack.len() {
            return Ok(None);
        }
        let regex = if self.must_advance {
            &self.pattern.after_empty
        } else {
            &self.pattern.first
        };
        let found = regex
            .find_from_pos(&self.haystack, self.position)
            .map_err(|error| {
                boundary(
                    "regex_core_limit",
                    format!("the matching core stopped the search: {error}"),
                )
            })?;
        let Some(found) = found else {
            self.finished = true;
            return Ok(None);
        };
        let (start, end) = (found.start(), found.end());
        self.must_advance = start == end;
        self.position = end;
        if start == end && end == self.haystack.len() {
            // nothing can follow an empty match at the end
            self.position = self.haystack.len();
        }
        Ok(Some((self.point_at[start], self.point_at[end])))
    }
}

/// The spans of the groups of one match, in code points: `[0]` is the whole match, `None` a group that did not take part.
pub type GroupSpans = Vec<Option<(usize, usize)>>;

impl CompiledPattern {
    /// `pattern.match(subject)`: a match that starts at position 0, with the spans of all groups.
    pub fn match_at_start(&self, subject: &PyStr) -> Result<Option<GroupSpans>, EngineFailure> {
        let mut haystack = String::new();
        let mut point_at = Vec::new();
        for (index, &point) in subject.code_points().iter().enumerate() {
            let Some(c) = char::from_u32(point) else {
                return Err(boundary(
                    "lone_surrogate_subject",
                    "the matching core holds scalar values only; a lone surrogate in the searched text is not modelled",
                ));
            };
            for _ in 0..c.len_utf8() {
                point_at.push(index);
            }
            haystack.push(c);
        }
        point_at.push(subject.len());
        let captures = self.anchored.captures(&haystack).map_err(|error| {
            boundary(
                "regex_core_limit",
                format!("the matching core stopped the search: {error}"),
            )
        })?;
        Ok(captures.map(|captures| {
            (0..=self.groups.saturating_sub(1))
                .map(|index| {
                    captures
                        .get(index)
                        .map(|m| (point_at[m.start()], point_at[m.end()]))
                })
                .collect()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(pattern: &str, text: &str) -> Vec<(usize, usize)> {
        let pattern = PyStr::new(pattern);
        let compiled = compile(PatternSource::Text(&pattern)).unwrap_or_else(|e| panic!("{e:?}"));
        let mut matches = compiled.finditer_text(&PyStr::new(text)).unwrap();
        let mut out = Vec::new();
        while let Some(span) = matches.next_span().unwrap() {
            out.push(span);
        }
        out
    }

    #[test]
    fn empty_matches_follow_the_python_rule_including_a_non_empty_match_at_the_same_position() {
        assert_eq!(spans("", "ab"), vec![(0, 0), (1, 1), (2, 2)]);
        assert_eq!(spans("(?=aba)", "ababa"), vec![(0, 0), (2, 2)]);
        assert_eq!(spans("x*", "axb"), vec![(0, 0), (1, 2), (2, 2), (3, 3)]);
        assert_eq!(spans("(?:|a)", "a"), vec![(0, 0), (0, 1), (1, 1)]);
    }

    #[test]
    fn python_anchors_and_dot_differ_from_the_core_defaults() {
        assert_eq!(spans("$", "alpha\n"), vec![(5, 5), (6, 6)]);
        assert_eq!(spans("(?m)^", "a\nb"), vec![(0, 0), (2, 2)]);
        assert_eq!(spans("a.b", "a\nb"), Vec::<(usize, usize)>::new());
        assert_eq!(spans("(?s)a.b", "a\nb"), vec![(0, 3)]);
        assert_eq!(spans("\\Aa", "aa"), vec![(0, 1)]);
        assert_eq!(spans("a\\Z", "a\n"), Vec::<(usize, usize)>::new());
    }

    #[test]
    fn classes_follow_the_interpreters_unicode_tables_and_offsets_are_code_points() {
        assert_eq!(spans("\\w+\\s+\\d+", "한글 ١٢३ tail"), vec![(0, 6)]);
        assert_eq!(spans("\\d", "a٣"), vec![(1, 2)]);
        assert_eq!(spans("(?a)\\w", "é_"), vec![(1, 2)]);
        assert_eq!(spans("\\bab\\b", "ab cab ab"), vec![(0, 2), (7, 9)]);
    }

    #[test]
    fn ignorecase_uses_the_python_lowering_and_the_casefix_pairs() {
        // İ lowers to 'i'; ı and ſ and the Kelvin sign are equivalent to i, s, k under re.IGNORECASE
        assert_eq!(spans("(?i)[a-z]+", "İıſK"), vec![(0, 4)]);
        assert_eq!(spans("(?i)i", "İıI"), vec![(0, 1), (1, 2), (2, 3)]);
        assert_eq!(spans("(?i)straSSe", "STRAsse"), vec![(0, 7)]);
    }

    #[test]
    fn backreferences_lookbehind_and_conditionals_run_in_the_core() {
        assert_eq!(spans("(?P<pair>ab)(?P=pair)", "abab then ab"), vec![(0, 4)]);
        assert_eq!(spans("(?<=prefix)AB", "prefixAB tail"), vec![(6, 8)]);
        assert_eq!(spans("(a)?(?(1)b|c)", "abc"), vec![(0, 2), (2, 3)]);
        assert_eq!(spans("ab|a.*", "ababa xxx"), vec![(0, 2), (2, 4), (4, 9)]);
    }
}
