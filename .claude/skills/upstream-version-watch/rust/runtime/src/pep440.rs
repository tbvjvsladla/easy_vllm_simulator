//! `packaging.version.Version` of packaging 24.0 (the part `resolve_torch_pin.py` uses): the parse of a version text, its integer fields
//! and its sort key. Adapted from `packaging/version.py` of packaging 24.0 (dual licensed Apache-2.0 / BSD-2-Clause, see `notices/`).
//!
//! The text is matched the way the original's compiled pattern matches it (`^\s*` + `VERSION_PATTERN` + `\s*$`, `re.VERBOSE | re.IGNORECASE`,
//! `search`, a `str` pattern): ASCII digits only (`[0-9]`), Unicode `\s`, and `IGNORECASE` for the keywords and for the local label class
//! (`[a-z0-9]`) as `re` compiles it: a character matches a letter when its simple lowercase is that letter, plus the two equivalences
//! `re` adds (`i`~`ı` U+0131, `s`~`ſ` U+017F); so `İ` U+0130 (simple lowercase `i`) and `K` U+212A (`k`) match too. The alternatives are tried in
//! the pattern's order with its greedy choices, so the captures are the ones the regular expression yields.
//!
//! The fields are made in the original's order (`epoch`, `release` left to right, `pre`, `post`, `dev`, `local` left to right) and every
//! integer comes from `int(text)` with its decimal-digit limit of 4300 (leading zeros count), so the first failing conversion is the
//! error. The lowercase of a matched letter text is Python's `str.lower()` (only `İ` -> `i` U+0307 and `K` -> `k` change besides ASCII).
//! The label of a pre/post/dev part is rewritten only when its lowered text is exactly an alias (`alpha`, `beta`, `c`, `pre`, `preview`, `rev`,
//! `r`); otherwise it stays as lowered and is compared as text by the sort key.

use crate::thrown::Thrown;
use agent_runtime_support::pystr::is_python_space;
use num_bigint::BigInt;
use num_traits::Zero;
use std::cmp::Ordering;

/// `sys.flags.int_max_str_digits` of the frozen interpreter profile (`-X int_max_str_digits=4300`).
const MAX_INT_DIGITS: usize = 4300;

/// A letter label with its number (`("rc", 1)`).
#[derive(Debug)]
struct Tagged {
    label: Vec<u32>,
    number: BigInt,
}

/// A part of the local version: text sorts before a number, texts as `str`, numbers as `int`.
#[derive(Debug)]
enum Local {
    Text(Vec<u32>),
    Number(BigInt),
}

/// A parsed version (`packaging.version.Version._version` plus what the sort key needs).
#[derive(Debug)]
pub(crate) struct Version {
    epoch: BigInt,
    release: Vec<BigInt>,
    /// The release without its trailing zeros (the sort key compares this prefix).
    trimmed: usize,
    pre: Option<Tagged>,
    post: Option<Tagged>,
    dev: Option<Tagged>,
    local: Option<Vec<Local>>,
}

/// Why a text is not a version.
pub(crate) enum Refusal {
    /// The pattern does not match: `InvalidVersion`.
    Invalid,
    /// An integer conversion failed: `ValueError` (not `InvalidVersion`).
    Thrown(Box<Thrown>),
}

type Span = (usize, usize);

#[derive(Default, Clone, Copy)]
struct Captures {
    epoch: Option<Span>,
    release: Span,
    pre_label: Option<Span>,
    pre_number: Option<Span>,
    post_first: Option<Span>,
    post_label: Option<Span>,
    post_second: Option<Span>,
    dev_label: Option<Span>,
    dev_number: Option<Span>,
    local: Option<Span>,
}

const PRE_LABELS: [&str; 8] = ["alpha", "a", "beta", "b", "preview", "pre", "c", "rc"];
const POST_LABELS: [&str; 3] = ["post", "rev", "r"];
const DEV_LABELS: [&str; 1] = ["dev"];

fn is_digit(cp: u32) -> bool {
    (0x30..=0x39).contains(&cp)
}

fn is_separator(cp: u32) -> bool {
    cp == u32::from(b'-') || cp == u32::from(b'_') || cp == u32::from(b'.')
}

/// `re`'s simple lowercase of one character as far as it can reach the ASCII letters of the pattern: ASCII upper case, `İ` (U+0130) and the Kelvin sign.
fn simple_lower(cp: u32) -> u32 {
    match cp {
        0x41..=0x5A => cp + 32,
        0x130 => 0x69,
        0x212A => 0x6B,
        _ => cp,
    }
}

/// Whether `cp` matches the lowercase ASCII letter `letter` under `re.IGNORECASE` (Unicode).
fn matches_letter(cp: u32, letter: u8) -> bool {
    let lower = simple_lower(cp);
    lower == u32::from(letter)
        || (letter == b'i' && lower == 0x131)
        || (letter == b's' && lower == 0x17F)
}

/// The local label class `[a-z0-9]` under `re.IGNORECASE`.
fn is_label_char(cp: u32) -> bool {
    let lower = simple_lower(cp);
    is_digit(lower) || (0x61..=0x7A).contains(&lower) || lower == 0x131 || lower == 0x17F
}

struct Matcher<'a> {
    text: &'a [u32],
}

impl Matcher<'_> {
    fn at(&self, position: usize) -> Option<u32> {
        self.text.get(position).copied()
    }

    fn digits_from(&self, position: usize) -> usize {
        let mut end = position;
        while self.at(end).is_some_and(is_digit) {
            end += 1;
        }
        end
    }

    fn literal(&self, position: usize, word: &str) -> Option<usize> {
        let mut at = position;
        for letter in word.bytes() {
            if !self.at(at).is_some_and(|cp| matches_letter(cp, letter)) {
                return None;
            }
            at += 1;
        }
        Some(at)
    }

    /// The positions after an optional `[-_.]` in the order the pattern tries them (taken first).
    fn after_separator(&self, position: usize) -> [Option<usize>; 2] {
        if self.at(position).is_some_and(is_separator) {
            [Some(position + 1), Some(position)]
        } else {
            [Some(position), None]
        }
    }

    /// `(?P<n>[0-9]+)?` (greedy: no shorter digit run can lead to a match, the next token never starts with a digit).
    fn optional_digits(&self, position: usize) -> (usize, Option<Span>) {
        let end = self.digits_from(position);
        if end > position {
            (end, Some((position, end)))
        } else {
            (position, None)
        }
    }

    fn run(&self) -> Option<Captures> {
        let mut caps = Captures::default();
        let mut start = 0;
        while self.at(start).is_some_and(is_python_space) {
            start += 1;
        }
        // `v?`
        if self.at(start).is_some_and(|cp| matches_letter(cp, b'v'))
            && self.epoch(start + 1, &mut caps)
        {
            return Some(caps);
        }
        if self.epoch(start, &mut caps) {
            return Some(caps);
        }
        None
    }

    /// `(?:(?P<epoch>[0-9]+)!)?` then the release.
    fn epoch(&self, position: usize, caps: &mut Captures) -> bool {
        let end = self.digits_from(position);
        if end > position && self.at(end) == Some(u32::from(b'!')) {
            caps.epoch = Some((position, end));
            if self.release(end + 1, caps) {
                return true;
            }
            caps.epoch = None;
        }
        self.release(position, caps)
    }

    /// `(?P<release>[0-9]+(?:\.[0-9]+)*)` (greedy; a shorter release is always followed by a digit or a dot and a digit, which nothing after it accepts).
    fn release(&self, position: usize, caps: &mut Captures) -> bool {
        let mut end = self.digits_from(position);
        if end == position {
            return false;
        }
        while self.at(end) == Some(u32::from(b'.')) {
            let next = self.digits_from(end + 1);
            if next == end + 1 {
                break;
            }
            end = next;
        }
        caps.release = (position, end);
        self.pre(end, caps)
    }

    /// `(?P<pre>[-_.]?(?P<pre_l>alpha|a|beta|b|preview|pre|c|rc)[-_.]?(?P<pre_n>[0-9]+)?)?`
    fn pre(&self, position: usize, caps: &mut Captures) -> bool {
        for first in self.after_separator(position).into_iter().flatten() {
            for word in PRE_LABELS {
                let Some(after) = self.literal(first, word) else {
                    continue;
                };
                for second in self.after_separator(after).into_iter().flatten() {
                    let (end, number) = self.optional_digits(second);
                    caps.pre_label = Some((first, after));
                    caps.pre_number = number;
                    if self.post(end, caps) {
                        return true;
                    }
                }
            }
        }
        caps.pre_label = None;
        caps.pre_number = None;
        self.post(position, caps)
    }

    /// `(?P<post>(?:-(?P<post_n1>[0-9]+))|(?:[-_.]?(?P<post_l>post|rev|r)[-_.]?(?P<post_n2>[0-9]+)?))?`
    fn post(&self, position: usize, caps: &mut Captures) -> bool {
        if self.at(position) == Some(u32::from(b'-')) {
            let end = self.digits_from(position + 1);
            if end > position + 1 {
                caps.post_first = Some((position + 1, end));
                if self.dev(end, caps) {
                    return true;
                }
                caps.post_first = None;
            }
        }
        for first in self.after_separator(position).into_iter().flatten() {
            for word in POST_LABELS {
                let Some(after) = self.literal(first, word) else {
                    continue;
                };
                for second in self.after_separator(after).into_iter().flatten() {
                    let (end, number) = self.optional_digits(second);
                    caps.post_label = Some((first, after));
                    caps.post_second = number;
                    if self.dev(end, caps) {
                        return true;
                    }
                }
            }
        }
        caps.post_label = None;
        caps.post_second = None;
        self.dev(position, caps)
    }

    /// `(?P<dev>[-_.]?(?P<dev_l>dev)[-_.]?(?P<dev_n>[0-9]+)?)?`
    fn dev(&self, position: usize, caps: &mut Captures) -> bool {
        for first in self.after_separator(position).into_iter().flatten() {
            for word in DEV_LABELS {
                let Some(after) = self.literal(first, word) else {
                    continue;
                };
                for second in self.after_separator(after).into_iter().flatten() {
                    let (end, number) = self.optional_digits(second);
                    caps.dev_label = Some((first, after));
                    caps.dev_number = number;
                    if self.local(end, caps) {
                        return true;
                    }
                }
            }
        }
        caps.dev_label = None;
        caps.dev_number = None;
        self.local(position, caps)
    }

    /// `(?:\+(?P<local>[a-z0-9]+(?:[-_\.][a-z0-9]+)*))?` (greedy, as above).
    fn local(&self, position: usize, caps: &mut Captures) -> bool {
        if self.at(position) == Some(u32::from(b'+')) {
            let begin = position + 1;
            let mut end = begin;
            while self.at(end).is_some_and(is_label_char) {
                end += 1;
            }
            if end > begin {
                loop {
                    if self.at(end).is_some_and(is_separator)
                        && self.at(end + 1).is_some_and(is_label_char)
                    {
                        end += 1;
                        while self.at(end).is_some_and(is_label_char) {
                            end += 1;
                        }
                    } else {
                        break;
                    }
                }
                caps.local = Some((begin, end));
                if self.finish(end) {
                    return true;
                }
                caps.local = None;
            }
        }
        self.finish(position)
    }

    /// `\s*$`
    fn finish(&self, position: usize) -> bool {
        let mut end = position;
        while self.at(end).is_some_and(is_python_space) {
            end += 1;
        }
        end == self.text.len()
    }
}

/// `int(text)` of ASCII digits: the decimal-digit limit counts every digit, leading zeros included.
fn int_of(points: &[u32]) -> Result<BigInt, Refusal> {
    if points.len() > MAX_INT_DIGITS {
        return Err(Refusal::Thrown(Thrown::builtin(
            "ValueError",
            &format!(
                "Exceeds the limit ({MAX_INT_DIGITS} digits) for integer string conversion: value has {} digits; use sys.set_int_max_str_digits() to increase the limit",
                points.len()
            ),
        )));
    }
    if points.len() <= 18 {
        let mut number: u64 = 0;
        for &point in points {
            number = number * 10 + u64::from(point - 0x30);
        }
        return Ok(BigInt::from(number));
    }
    let bytes: Vec<u8> = points.iter().map(|&point| point as u8).collect();
    Ok(BigInt::parse_bytes(&bytes, 10).expect("ASCII digits"))
}

/// Python's `str.lower()` of a matched label text.
fn lowered(points: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(points.len());
    for &point in points {
        match point {
            0x41..=0x5A => out.push(point + 32),
            0x130 => {
                out.push(0x69);
                out.push(0x307);
            }
            0x212A => out.push(0x6B),
            _ => out.push(point),
        }
    }
    out
}

fn equals(points: &[u32], word: &str) -> bool {
    points.len() == word.len()
        && points
            .iter()
            .zip(word.bytes())
            .all(|(&a, b)| a == u32::from(b))
}

/// `_parse_letter_version(letter, number)` for a captured label (the label is present).
fn letter_version(text: &[u32], label: Span, number: Option<Span>) -> Result<Tagged, Refusal> {
    let number = match number {
        Some((from, to)) => int_of(&text[from..to])?,
        None => BigInt::zero(),
    };
    let mut label = lowered(&text[label.0..label.1]);
    if equals(&label, "alpha") {
        label = vec![u32::from(b'a')];
    } else if equals(&label, "beta") {
        label = vec![u32::from(b'b')];
    } else if equals(&label, "c") || equals(&label, "pre") || equals(&label, "preview") {
        label = vec![u32::from(b'r'), u32::from(b'c')];
    } else if equals(&label, "rev") || equals(&label, "r") {
        label = "post".bytes().map(u32::from).collect();
    }
    Ok(Tagged { label, number })
}

impl Version {
    /// `Version(text)`: `Err(Refusal::Invalid)` where the original raises `InvalidVersion`, `Err(Refusal::Thrown)` where an integer conversion raises.
    pub(crate) fn parse(text: &[u32]) -> Result<Version, Refusal> {
        let caps = Matcher { text }.run().ok_or(Refusal::Invalid)?;
        let epoch = match caps.epoch {
            Some((from, to)) => int_of(&text[from..to])?,
            None => BigInt::zero(),
        };
        let mut release = Vec::new();
        let (from, to) = caps.release;
        let mut begin = from;
        for at in from..=to {
            if at == to || text[at] == u32::from(b'.') {
                release.push(int_of(&text[begin..at])?);
                begin = at + 1;
            }
        }
        let pre = match caps.pre_label {
            Some(label) => Some(letter_version(text, label, caps.pre_number)?),
            None => None,
        };
        let post = match (caps.post_label, caps.post_first.or(caps.post_second)) {
            (Some(label), number) => Some(letter_version(text, label, number)?),
            // implicit post release `1.0-1`: a number without a letter
            (None, Some((from, to))) => Some(Tagged {
                label: "post".bytes().map(u32::from).collect(),
                number: int_of(&text[from..to])?,
            }),
            (None, None) => None,
        };
        let dev = match caps.dev_label {
            Some(label) => Some(letter_version(text, label, caps.dev_number)?),
            None => None,
        };
        let local = match caps.local {
            Some((from, to)) => {
                let mut parts = Vec::new();
                let mut begin = from;
                for at in from..=to {
                    if at == to || matches!(text[at], 0x2E | 0x5F | 0x2D) {
                        let part = &text[begin..at];
                        parts.push(if part.iter().all(|&cp| is_digit(cp)) {
                            Local::Number(int_of(part)?)
                        } else {
                            Local::Text(lowered(part))
                        });
                        begin = at + 1;
                    }
                }
                Some(parts)
            }
            None => None,
        };
        let mut trimmed = release.len();
        while trimmed > 0 && release[trimmed - 1].is_zero() {
            trimmed -= 1;
        }
        Ok(Version {
            epoch,
            release,
            trimmed,
            pre,
            post,
            dev,
            local,
        })
    }

    /// `".".join(map(str, self.release))`: the whole release, no padding or truncation.
    pub(crate) fn release_text(&self) -> String {
        let mut out = String::new();
        for (at, part) in self.release.iter().enumerate() {
            if at > 0 {
                out.push('.');
            }
            out.push_str(&part.to_string());
        }
        out
    }

    /// `packaging.utils.canonicalize_version(version, strip_trailing_zero=strip_trailing_zero)` of this parsed version: the epoch when it is not zero,
    /// the release (its trailing `.0` parts removed when asked), the pre-release label and number, `.post<n>`, `.dev<n>` and `+<local>` with the local
    /// parts joined by dots. Text parts are the lowered labels the parse kept; numbers print in decimal.
    pub(crate) fn canonical_text(&self, strip_trailing_zero: bool) -> Vec<u32> {
        let mut text = String::new();
        if !self.epoch.is_zero() {
            text.push_str(&format!("{}!", self.epoch));
        }
        let mut release = self.release_text();
        if strip_trailing_zero {
            while release.ends_with(".0") {
                release.truncate(release.len() - 2);
            }
        }
        text.push_str(&release);
        let mut out: Vec<u32> = text.chars().map(u32::from).collect();
        if let Some(pre) = &self.pre {
            out.extend_from_slice(&pre.label);
            out.extend(pre.number.to_string().chars().map(u32::from));
        }
        if let Some(post) = &self.post {
            out.extend(format!(".post{}", post.number).chars().map(u32::from));
        }
        if let Some(dev) = &self.dev {
            out.extend(format!(".dev{}", dev.number).chars().map(u32::from));
        }
        if let Some(local) = &self.local {
            out.push(u32::from(b'+'));
            for (at, part) in local.iter().enumerate() {
                if at > 0 {
                    out.push(u32::from(b'.'));
                }
                match part {
                    Local::Text(points) => out.extend_from_slice(points),
                    Local::Number(number) => {
                        out.extend(number.to_string().chars().map(u32::from));
                    }
                }
            }
        }
        out
    }

    /// The order of `Version._key`: epoch, the release without trailing zeros, pre (a dev-only version before every pre-release, none after them),
    /// post (none first), dev (none last), local (none first; texts before numbers).
    pub(crate) fn order(&self, other: &Version) -> Ordering {
        self.epoch
            .cmp(&other.epoch)
            .then_with(|| self.release[..self.trimmed].cmp(&other.release[..other.trimmed]))
            .then_with(|| pre_order(self, other))
            .then_with(|| post_order(&self.post, &other.post))
            .then_with(|| dev_order(&self.dev, &other.dev))
            .then_with(|| local_order(&self.local, &other.local))
    }
}

fn tagged_order(a: &Tagged, b: &Tagged) -> Ordering {
    a.label.cmp(&b.label).then_with(|| a.number.cmp(&b.number))
}

/// `_pre`: `-inf` for a dev-only version, `+inf` without pre (and not dev-only), else the tuple.
fn pre_order(a: &Version, b: &Version) -> Ordering {
    fn rank(v: &Version) -> (u8, Option<&Tagged>) {
        match &v.pre {
            None if v.post.is_none() && v.dev.is_some() => (0, None),
            None => (2, None),
            Some(tag) => (1, Some(tag)),
        }
    }
    let (ra, ta) = rank(a);
    let (rb, tb) = rank(b);
    ra.cmp(&rb).then_with(|| match (ta, tb) {
        (Some(x), Some(y)) => tagged_order(x, y),
        _ => Ordering::Equal,
    })
}

/// `_post`: `-inf` without post.
fn post_order(a: &Option<Tagged>, b: &Option<Tagged>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(x), Some(y)) => tagged_order(x, y),
    }
}

/// `_dev`: `+inf` without dev.
fn dev_order(a: &Option<Tagged>, b: &Option<Tagged>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => tagged_order(x, y),
    }
}

fn local_part_order(a: &Local, b: &Local) -> Ordering {
    match (a, b) {
        (Local::Text(x), Local::Text(y)) => x.cmp(y),
        (Local::Number(x), Local::Number(y)) => x.cmp(y),
        (Local::Text(_), Local::Number(_)) => Ordering::Less,
        (Local::Number(_), Local::Text(_)) => Ordering::Greater,
    }
}

/// `_local`: `-inf` without local, else the tuple of parts (a prefix is smaller).
fn local_order(a: &Option<Vec<Local>>, b: &Option<Vec<Local>>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(x), Some(y)) => {
            for (p, q) in x.iter().zip(y.iter()) {
                let order = local_part_order(p, q);
                if order != Ordering::Equal {
                    return order;
                }
            }
            x.len().cmp(&y.len())
        }
    }
}
