//! The pattern parser of CPython 3.12.3 `re/_parser.py`, ported from the source text (nothing is executed): the tokenizer, `_class_escape`, `_escape`,
//! `_parse_sub`, `_parse`, `_parse_flags`, `fix_flags` and `parse`, plus the look-behind width check of `_compiler._compile`. It produces a typed tree
//! ([`Node`]) and, for every pattern Python's `re` rejects, the `re.error` Python would raise: the same `msg`, the same `pos` (so the same `lineno`/`colno`
//! and message text), or the `OverflowError`/`ValueError` the parser raises instead.
//!
//! Named boundaries (never turned into an invented Python exception): `\N{...}` character names (no Unicode name database is shipped), non-ASCII group
//! names (no `str.isidentifier` tables), and the `FutureWarning`/`DeprecationWarning` texts of the parser (nested-set and set-operation hints), whose
//! `warnings` output this port does not reproduce.
//!
//! Position arithmetic follows the original: the tokenizer's `index` is the offset after the current token and `tell()` is `index - len(next)`.

use agent_runtime_support::pystr::PyStr;
use std::collections::BTreeMap;

pub const MAXREPEAT: u64 = 4_294_967_295;
const MAXWIDTH: u128 = 1u128 << 64;

pub const FLAG_TEMPLATE: u32 = 1;
pub const FLAG_IGNORECASE: u32 = 2;
pub const FLAG_LOCALE: u32 = 4;
pub const FLAG_MULTILINE: u32 = 8;
pub const FLAG_DOTALL: u32 = 16;
pub const FLAG_UNICODE: u32 = 32;
pub const FLAG_VERBOSE: u32 = 64;
pub const FLAG_DEBUG: u32 = 128;
pub const FLAG_ASCII: u32 = 256;
const TYPE_FLAGS: u32 = FLAG_ASCII | FLAG_LOCALE | FLAG_UNICODE;
const GLOBAL_FLAGS: u32 = FLAG_DEBUG | FLAG_TEMPLATE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Digit,
    NotDigit,
    Space,
    NotSpace,
    Word,
    NotWord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetItem {
    Literal(u32),
    Range(u32, u32),
    Category(Category),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    Beginning,
    BeginningString,
    End,
    EndString,
    Boundary,
    NonBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatKind {
    Greedy,
    Lazy,
    Possessive,
}

pub type Seq = Vec<Node>;

#[derive(Clone, Debug)]
pub enum Node {
    Literal(u32),
    NotLiteral(u32),
    Any,
    Set {
        negate: bool,
        items: Vec<SetItem>,
    },
    /// `\d \D \s \S \w \W` outside a set (the original's `IN` of one category).
    Category(Category),
    At(At),
    Branch(Vec<Seq>),
    Sub {
        group: Option<usize>,
        add: u32,
        del: u32,
        body: Seq,
    },
    Repeat {
        kind: RepeatKind,
        min: u64,
        max: u64,
        body: Seq,
    },
    GroupRef(usize),
    Exists {
        group: usize,
        yes: Seq,
        no: Option<Seq>,
    },
    Assert {
        ahead: bool,
        negate: bool,
        body: Seq,
    },
    Atomic(Seq),
}

/// `re.error(msg, pattern, pos)`: `pos` is `None` only for the compile-time look-behind check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReError {
    pub msg: PyStr,
    pub pattern: Option<Pattern>,
    pub pos: Option<i64>,
    /// How many `_parse_sub` frames the error passed out of (the traceback depth of the raise), counted while it propagates.
    pub sub_depth: u32,
    /// The line of `re/_parser.py` that raises it when the site's traceback is known, else 0.
    pub site: u32,
}

/// The pattern an error names: text or bytes, kept as code points (bytes as Latin-1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub points: Vec<u32>,
    pub is_text: bool,
}

#[derive(Clone, Debug)]
pub enum ParseFailure {
    Re(ReError),
    /// `OverflowError(message)`.
    Overflow(String),
    /// `ValueError(message)`.
    Value(String),
    /// A situation outside the named profile.
    Unsupported {
        boundary: &'static str,
        detail: String,
    },
}

impl From<ReError> for ParseFailure {
    fn from(error: ReError) -> ParseFailure {
        ParseFailure::Re(error)
    }
}

fn unsupported(boundary: &'static str, detail: &str) -> ParseFailure {
    ParseFailure::Unsupported {
        boundary,
        detail: detail.to_string(),
    }
}

type Parsed<T> = Result<T, ParseFailure>;

/// The result of a successful parse.
#[derive(Debug)]
pub struct ParsedPattern {
    pub body: Seq,
    pub flags: u32,
    pub groups: usize,
    pub is_text: bool,
}

fn is_digit(c: u32) -> bool {
    (u32::from(b'0')..=u32::from(b'9')).contains(&c)
}
fn is_oct(c: u32) -> bool {
    (u32::from(b'0')..=u32::from(b'7')).contains(&c)
}
fn is_hex(c: u32) -> bool {
    is_digit(c)
        || (u32::from(b'a')..=u32::from(b'f')).contains(&c)
        || (u32::from(b'A')..=u32::from(b'F')).contains(&c)
}
fn is_ascii_letter(c: u32) -> bool {
    (u32::from(b'a')..=u32::from(b'z')).contains(&c)
        || (u32::from(b'A')..=u32::from(b'Z')).contains(&c)
}
fn is_verbose_space(c: u32) -> bool {
    matches!(c, 0x20 | 0x09 | 0x0A | 0x0D | 0x0B | 0x0C)
}
const SPECIAL: &str = ".\\[{()*+?^$|";

fn special(c: u32) -> bool {
    char::from_u32(c).is_some_and(|c| SPECIAL.contains(c))
}

/// One tokenizer token: a character or a backslash and the character after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token {
    first: u32,
    second: Option<u32>,
}

impl Token {
    fn len(self) -> usize {
        if self.second.is_some() {
            2
        } else {
            1
        }
    }
    fn is(self, c: char) -> bool {
        self.second.is_none() && self.first == u32::from(c)
    }
    fn points(self) -> Vec<u32> {
        let mut out = vec![self.first];
        out.extend(self.second);
        out
    }
    fn is_backslash(self) -> bool {
        self.first == u32::from(b'\\')
    }
}

fn text_of(points: &[u32]) -> PyStr {
    PyStr::from_code_points(points.to_vec())
}

struct Source {
    chars: Vec<u32>,
    is_text: bool,
    index: usize,
    next: Option<Token>,
}

impl Source {
    fn new(chars: Vec<u32>, is_text: bool) -> Parsed<Source> {
        let mut source = Source {
            chars,
            is_text,
            index: 0,
            next: None,
        };
        source.advance()?;
        Ok(source)
    }

    fn pattern(&self) -> Pattern {
        Pattern {
            points: self.chars.clone(),
            is_text: self.is_text,
        }
    }

    /// `Tokenizer.__next`.
    fn advance(&mut self) -> Parsed<()> {
        let mut index = self.index;
        let Some(&first) = self.chars.get(index) else {
            self.next = None;
            return Ok(());
        };
        let mut second = None;
        if first == u32::from(b'\\') {
            index += 1;
            let Some(&follow) = self.chars.get(index) else {
                return Err(ReError {
                    msg: PyStr::new("bad escape (end of pattern)"),
                    pattern: Some(self.pattern()),
                    pos: Some(self.chars.len() as i64 - 1),
                    sub_depth: 0,
                    site: 0,
                }
                .into());
            };
            second = Some(follow);
        }
        self.index = index + 1;
        self.next = Some(Token { first, second });
        Ok(())
    }

    fn matches(&mut self, c: char) -> Parsed<bool> {
        if self.next.is_some_and(|token| token.is(c)) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn get(&mut self) -> Parsed<Option<Token>> {
        let this = self.next;
        self.advance()?;
        Ok(this)
    }

    fn getwhile(&mut self, n: usize, accepts: fn(u32) -> bool) -> Parsed<Vec<u32>> {
        let mut result = Vec::new();
        for _ in 0..n {
            let Some(token) = self.next else { break };
            if token.second.is_some() || !accepts(token.first) {
                break;
            }
            result.push(token.first);
            self.advance()?;
        }
        Ok(result)
    }

    fn tell(&self) -> i64 {
        self.index as i64 - self.next.map_or(0, |token| token.len() as i64)
    }

    fn seek(&mut self, index: i64) -> Parsed<()> {
        self.index = usize::try_from(index).unwrap_or(0);
        self.advance()
    }

    fn error(&self, msg: &str, offset: i64) -> ParseFailure {
        self.error_text(&PyStr::new(msg), offset)
    }

    fn error_text(&self, msg: &PyStr, offset: i64) -> ParseFailure {
        let msg = if self.is_text {
            msg.clone()
        } else {
            // msg.encode('ascii', 'backslashreplace').decode('ascii')
            PyStr::from_code_points(
                msg.to_backslashreplace_bytes()
                    .into_iter()
                    .map(u32::from)
                    .collect(),
            )
        };
        ParseFailure::Re(ReError {
            msg,
            pattern: Some(self.pattern()),
            pos: Some(self.tell() - offset),
            sub_depth: 0,
            site: 0,
        })
    }

    /// `getuntil(terminator, name)`.
    fn getuntil(&mut self, terminator: char, name: &str) -> Parsed<Vec<u32>> {
        let mut result = Vec::new();
        loop {
            let c = self.next;
            self.advance()?;
            let Some(token) = c else {
                if result.is_empty() {
                    return Err(self.error(&format!("missing {name}"), 0));
                }
                return Err(self.error(
                    &format!("missing {terminator}, unterminated name"),
                    result.len() as i64,
                ));
            };
            if token.is(terminator) {
                if result.is_empty() {
                    return Err(self.error(&format!("missing {name}"), 1));
                }
                break;
            }
            result.extend(token.points());
        }
        Ok(result)
    }

    /// `checkgroupname`: ASCII identifiers are decided here; anything else is outside the profile.
    fn check_group_name(&self, name: &[u32], offset: i64) -> Parsed<()> {
        let ascii = name.iter().all(|&c| c < 128);
        if !ascii {
            if !self.is_text {
                let shown = name_ascii_repr(name);
                return Err(self.error_text(
                    &PyStr::new(&format!("bad character in group name {shown}")),
                    name.len() as i64 + offset,
                ));
            }
            return Err(unsupported(
                "non_ascii_group_name",
                "a group name with a non-ASCII character needs the Unicode identifier tables",
            ));
        }
        let identifier = name.first().is_some_and(|&c| c == 95 || is_ascii_letter(c))
            && name
                .iter()
                .all(|&c| c == 95 || is_ascii_letter(c) || is_digit(c));
        if !identifier {
            let shown = text_of(name).repr();
            return Err(self.error_text(
                &PyStr::new(&format!("bad character in group name {shown}")),
                name.len() as i64 + offset,
            ));
        }
        Ok(())
    }
}

fn name_ascii_repr(name: &[u32]) -> String {
    // `%a` of a name: the repr with non-ASCII characters escaped
    let repr = text_of(name).repr();
    let mut out = String::new();
    for c in repr.chars() {
        if (c as u32) < 128 {
            out.push(c);
        } else if (c as u32) < 256 {
            out.push_str(&format!("\\x{:02x}", c as u32));
        } else if (c as u32) < 65536 {
            out.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            out.push_str(&format!("\\U{:08x}", c as u32));
        }
    }
    out
}

struct State {
    flags: u32,
    groupdict: Vec<(Vec<u32>, usize)>,
    groupwidths: Vec<Option<(u128, u128)>>,
    lookbehindgroups: Option<usize>,
    grouprefpos: Vec<(usize, i64)>,
}

impl State {
    fn groups(&self) -> usize {
        self.groupwidths.len()
    }
    fn group_named(&self, name: &[u32]) -> Option<usize> {
        self.groupdict
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, gid)| *gid)
    }
    fn check_group(&self, gid: usize) -> bool {
        gid < self.groups() && self.groupwidths[gid].is_some()
    }
    fn check_lookbehind_group(&self, gid: usize, source: &Source) -> Parsed<()> {
        if let Some(limit) = self.lookbehindgroups {
            if !self.check_group(gid) {
                return Err(source.error("cannot refer to an open group", 0));
            }
            if gid >= limit {
                return Err(source.error(
                    "cannot refer to group defined in the same lookbehind subpattern",
                    0,
                ));
            }
        }
        Ok(())
    }
}

/// `SubPattern.getwidth`.
pub fn width_of(seq: &[Node], widths: &[Option<(u128, u128)>]) -> (u128, u128) {
    let (mut lo, mut hi) = (0u128, 0u128);
    for node in seq {
        match node {
            Node::Branch(items) => {
                let (mut i, mut j) = (MAXWIDTH, 0u128);
                for item in items {
                    let (l, h) = width_of(item, widths);
                    i = i.min(l);
                    j = j.max(h);
                }
                lo += i;
                hi += j;
            }
            Node::Atomic(body) | Node::Sub { body, .. } => {
                let (i, j) = width_of(body, widths);
                lo += i;
                hi += j;
            }
            Node::Repeat { min, max, body, .. } => {
                let (i, j) = width_of(body, widths);
                lo += i * u128::from(*min);
                if *max == MAXREPEAT && j != 0 {
                    hi = MAXWIDTH;
                } else {
                    hi += j * u128::from(*max);
                }
            }
            Node::Literal(_)
            | Node::NotLiteral(_)
            | Node::Any
            | Node::Set { .. }
            | Node::Category(_) => {
                lo += 1;
                hi += 1;
            }
            Node::GroupRef(gid) => {
                let (i, j) = widths[*gid].unwrap_or((0, 0));
                lo += i;
                hi += j;
            }
            Node::Exists { yes, no, .. } => {
                let (mut i, mut j) = width_of(yes, widths);
                if let Some(no) = no {
                    let (l, h) = width_of(no, widths);
                    i = i.min(l);
                    j = j.max(h);
                } else {
                    i = 0;
                }
                lo += i;
                hi += j;
            }
            Node::At(_) | Node::Assert { .. } => {}
        }
    }
    (lo.min(MAXWIDTH), hi.min(MAXWIDTH))
}

fn class_category(second: u32) -> Option<Category> {
    Some(match char::from_u32(second)? {
        'd' => Category::Digit,
        'D' => Category::NotDigit,
        's' => Category::Space,
        'S' => Category::NotSpace,
        'w' => Category::Word,
        'W' => Category::NotWord,
        _ => return None,
    })
}

fn simple_escape(second: u32) -> Option<u32> {
    Some(match char::from_u32(second)? {
        'a' => 7,
        'b' => 8,
        'f' => 12,
        'n' => 10,
        'r' => 13,
        't' => 9,
        'v' => 11,
        '\\' => 92,
        _ => return None,
    })
}

fn escape_text(points: &[u32]) -> String {
    points
        .iter()
        .map(|&c| char::from_u32(c).unwrap_or('\u{fffd}'))
        .collect()
}

enum ClassItem {
    Literal(u32),
    Category(Category),
}

/// `\N{name}` is outside the profile; a malformed form is decided like the original.
fn name_escape(source: &mut Source) -> Parsed<u32> {
    if !source.matches('{')? {
        return Err(source.error("missing {", 0));
    }
    let name = source.getuntil('}', "character name")?;
    let _ = name;
    Err(unsupported(
        "unicode_character_name",
        "a \\N{...} escape needs the Unicode name database, which this port does not ship",
    ))
}

fn hex_escape(
    source: &mut Source,
    mut escape: Vec<u32>,
    count: usize,
    total: usize,
) -> Parsed<Vec<u32>> {
    escape.extend(source.getwhile(count, is_hex)?);
    if escape.len() != total {
        return Err(source.error(
            &format!("incomplete escape {}", escape_text(&escape)),
            escape.len() as i64,
        ));
    }
    Ok(escape)
}

fn parse_radix(points: &[u32], radix: u32) -> u64 {
    points.iter().fold(0u64, |acc, &c| {
        acc * u64::from(radix)
            + u64::from(
                char::from_u32(c)
                    .and_then(|d| d.to_digit(radix))
                    .unwrap_or(0),
            )
    })
}

/// `_class_escape`.
fn class_escape(source: &mut Source, token: Token) -> Parsed<ClassItem> {
    let second = token.second.unwrap_or(0);
    if let Some(code) = simple_escape(second) {
        return Ok(ClassItem::Literal(code));
    }
    if let Some(category) = class_category(second) {
        return Ok(ClassItem::Category(category));
    }
    let escape = token.points();
    let c = char::from_u32(second).unwrap_or('\0');
    let bad = |source: &Source, escape: &[u32]| {
        source.error(
            &format!("bad escape {}", escape_text(escape)),
            escape.len() as i64,
        )
    };
    match c {
        'x' => {
            let escape = hex_escape(source, escape, 2, 4)?;
            Ok(ClassItem::Literal(parse_radix(&escape[2..], 16) as u32))
        }
        'u' if source.is_text => {
            let escape = hex_escape(source, escape, 4, 6)?;
            Ok(ClassItem::Literal(parse_radix(&escape[2..], 16) as u32))
        }
        'U' if source.is_text => {
            let escape = hex_escape(source, escape, 8, 10)?;
            let value = parse_radix(&escape[2..], 16);
            if value > 0x10FFFF {
                return Err(bad(source, &escape));
            }
            Ok(ClassItem::Literal(value as u32))
        }
        'N' if source.is_text => Ok(ClassItem::Literal(name_escape(source)?)),
        c if c.is_digit(8) => {
            let mut escape = escape;
            escape.extend(source.getwhile(2, is_oct)?);
            let value = parse_radix(&escape[1..], 8);
            if value > 0o377 {
                return Err(source.error(
                    &format!(
                        "octal escape value {} outside of range 0-0o377",
                        escape_text(&escape)
                    ),
                    escape.len() as i64,
                ));
            }
            Ok(ClassItem::Literal(value as u32))
        }
        c if c.is_ascii_digit() => Err(bad(source, &escape)),
        _ => {
            if is_ascii_letter(second) {
                return Err(bad(source, &escape));
            }
            Ok(ClassItem::Literal(second))
        }
    }
}

enum EscapeResult {
    Node(Node),
}

/// `_escape`.
fn escape_outside(source: &mut Source, token: Token, state: &State) -> Parsed<EscapeResult> {
    let second = token.second.unwrap_or(0);
    let c = char::from_u32(second).unwrap_or('\0');
    match c {
        'A' => return Ok(EscapeResult::Node(Node::At(At::BeginningString))),
        'b' => return Ok(EscapeResult::Node(Node::At(At::Boundary))),
        'B' => return Ok(EscapeResult::Node(Node::At(At::NonBoundary))),
        'Z' => return Ok(EscapeResult::Node(Node::At(At::EndString))),
        _ => {}
    }
    if let Some(category) = class_category(second) {
        return Ok(EscapeResult::Node(Node::Category(category)));
    }
    if let Some(code) = simple_escape(second) {
        return Ok(EscapeResult::Node(Node::Literal(code)));
    }
    let mut escape = token.points();
    let bad = |source: &Source, escape: &[u32]| {
        source.error(
            &format!("bad escape {}", escape_text(escape)),
            escape.len() as i64,
        )
    };
    match c {
        'x' => {
            let escape = hex_escape(source, escape, 2, 4)?;
            Ok(EscapeResult::Node(Node::Literal(
                parse_radix(&escape[2..], 16) as u32,
            )))
        }
        'u' if source.is_text => {
            let escape = hex_escape(source, escape, 4, 6)?;
            Ok(EscapeResult::Node(Node::Literal(
                parse_radix(&escape[2..], 16) as u32,
            )))
        }
        'U' if source.is_text => {
            let escape = hex_escape(source, escape, 8, 10)?;
            let value = parse_radix(&escape[2..], 16);
            if value > 0x10FFFF {
                return Err(bad(source, &escape));
            }
            Ok(EscapeResult::Node(Node::Literal(value as u32)))
        }
        'N' if source.is_text => Ok(EscapeResult::Node(Node::Literal(name_escape(source)?))),
        '0' => {
            escape.extend(source.getwhile(2, is_oct)?);
            Ok(EscapeResult::Node(Node::Literal(
                parse_radix(&escape[1..], 8) as u32,
            )))
        }
        c if c.is_ascii_digit() => {
            if source
                .next
                .is_some_and(|t| t.second.is_none() && is_digit(t.first))
            {
                let more = source.get()?.map(|t| t.first).unwrap_or(0);
                escape.push(more);
                if is_oct(escape[1])
                    && is_oct(escape[2])
                    && source
                        .next
                        .is_some_and(|t| t.second.is_none() && is_oct(t.first))
                {
                    let more = source.get()?.map(|t| t.first).unwrap_or(0);
                    escape.push(more);
                    let value = parse_radix(&escape[1..], 8);
                    if value > 0o377 {
                        return Err(source.error(
                            &format!(
                                "octal escape value {} outside of range 0-0o377",
                                escape_text(&escape)
                            ),
                            escape.len() as i64,
                        ));
                    }
                    return Ok(EscapeResult::Node(Node::Literal(value as u32)));
                }
            }
            let group = parse_radix(&escape[1..], 10) as usize;
            if group < state.groups() {
                if !state.check_group(group) {
                    return Err(source.error("cannot refer to an open group", escape.len() as i64));
                }
                state.check_lookbehind_group(group, source)?;
                return Ok(EscapeResult::Node(Node::GroupRef(group)));
            }
            Err(source.error(
                &format!("invalid group reference {group}"),
                escape.len() as i64 - 1,
            ))
        }
        _ => {
            if is_ascii_letter(second) {
                return Err(bad(source, &escape));
            }
            Ok(EscapeResult::Node(Node::Literal(second)))
        }
    }
}

fn flag_of(c: u32) -> Option<u32> {
    Some(match char::from_u32(c)? {
        'i' => FLAG_IGNORECASE,
        'L' => FLAG_LOCALE,
        'm' => FLAG_MULTILINE,
        's' => FLAG_DOTALL,
        'x' => FLAG_VERBOSE,
        'a' => FLAG_ASCII,
        't' => FLAG_TEMPLATE,
        'u' => FLAG_UNICODE,
        _ => return None,
    })
}

fn is_alpha(c: u32) -> bool {
    char::from_u32(c).is_some_and(char::is_alphabetic)
}

/// `_parse_flags`: `None` for global flags (already merged into the state).
fn parse_flags(source: &mut Source, state: &mut State, first: Token) -> Parsed<Option<(u32, u32)>> {
    let mut add_flags = 0u32;
    let mut del_flags = 0u32;
    let mut current = Some(first);
    let is_dash = |t: Option<Token>| t.is_some_and(|t| t.is('-'));
    if !is_dash(current) {
        loop {
            let token = current.unwrap_or(first);
            let flag = flag_of(token.first).unwrap_or(0);
            if source.is_text {
                if token.is('L') {
                    return Err(source.error(
                        "bad inline flags: cannot use 'L' flag with a str pattern",
                        0,
                    ));
                }
            } else if token.is('u') {
                return Err(source.error(
                    "bad inline flags: cannot use 'u' flag with a bytes pattern",
                    0,
                ));
            }
            add_flags |= flag;
            if flag & TYPE_FLAGS != 0 && add_flags & TYPE_FLAGS != flag {
                return Err(source.error(
                    "bad inline flags: flags 'a', 'u' and 'L' are incompatible",
                    0,
                ));
            }
            current = source.get()?;
            let Some(token) = current else {
                return Err(source.error("missing -, : or )", 0));
            };
            if token.second.is_none()
                && matches!(char::from_u32(token.first), Some(')' | '-' | ':'))
            {
                break;
            }
            if token.second.is_some() || flag_of(token.first).is_none() {
                let msg = if token.second.is_none() && is_alpha(token.first) {
                    "unknown flag"
                } else {
                    "missing -, : or )"
                };
                return Err(source.error(msg, token.len() as i64));
            }
        }
    }
    let token = current.unwrap_or(first);
    if token.is(')') {
        state.flags |= add_flags;
        return Ok(None);
    }
    if add_flags & GLOBAL_FLAGS != 0 {
        return Err(source.error("bad inline flags: cannot turn on global flag", 1));
    }
    if token.is('-') {
        let next = source.get()?;
        let Some(token) = next else {
            return Err(source.error("missing flag", 0));
        };
        if token.second.is_some() || flag_of(token.first).is_none() {
            let msg = if token.second.is_none() && is_alpha(token.first) {
                "unknown flag"
            } else {
                "missing flag"
            };
            return Err(source.error(msg, token.len() as i64));
        }
        let mut token = token;
        loop {
            let flag = flag_of(token.first).unwrap_or(0);
            if flag & TYPE_FLAGS != 0 {
                return Err(source.error(
                    "bad inline flags: cannot turn off flags 'a', 'u' and 'L'",
                    0,
                ));
            }
            del_flags |= flag;
            let Some(follow) = source.get()? else {
                return Err(source.error("missing :", 0));
            };
            if follow.is(':') {
                break;
            }
            if follow.second.is_some() || flag_of(follow.first).is_none() {
                let msg = if follow.second.is_none() && is_alpha(follow.first) {
                    "unknown flag"
                } else {
                    "missing :"
                };
                return Err(source.error(msg, follow.len() as i64));
            }
            token = follow;
        }
    }
    if del_flags & GLOBAL_FLAGS != 0 {
        return Err(source.error("bad inline flags: cannot turn off global flag", 1));
    }
    if add_flags & del_flags != 0 {
        return Err(source.error("bad inline flags: flag turned on and off", 1));
    }
    Ok(Some((add_flags, del_flags)))
}

fn deeper(failure: ParseFailure) -> ParseFailure {
    match failure {
        ParseFailure::Re(mut error) => {
            error.sub_depth += 1;
            ParseFailure::Re(error)
        }
        other => other,
    }
}

fn with_site(failure: ParseFailure, line: u32) -> ParseFailure {
    match failure {
        ParseFailure::Re(mut error) => {
            error.site = line;
            ParseFailure::Re(error)
        }
        other => other,
    }
}

fn parse_sub(source: &mut Source, state: &mut State, verbose: bool, nested: u32) -> Parsed<Seq> {
    parse_sub_frames(source, state, verbose, nested).map_err(deeper)
}

fn parse_sub_frames(
    source: &mut Source,
    state: &mut State,
    verbose: bool,
    nested: u32,
) -> Parsed<Seq> {
    let mut items: Vec<Seq> = Vec::new();
    let mut verbose = verbose;
    loop {
        let first = nested == 0 && items.is_empty();
        items.push(parse(source, state, verbose, nested + 1, first)?);
        if !source.matches('|')? {
            break;
        }
        if nested == 0 {
            verbose = state.flags & FLAG_VERBOSE != 0;
        }
    }
    if items.len() == 1 {
        return Ok(items.pop().unwrap_or_default());
    }
    Ok(vec![Node::Branch(items)])
}

fn parse(
    source: &mut Source,
    state: &mut State,
    verbose: bool,
    nested: u32,
    first: bool,
) -> Parsed<Seq> {
    let mut verbose = verbose;
    let mut subpattern: Seq = Vec::new();
    while let Some(this) = source.next {
        if this.is('|') || this.is(')') {
            break;
        }
        source.get()?;
        if verbose {
            if this.second.is_none() && is_verbose_space(this.first) {
                continue;
            }
            if this.is('#') {
                loop {
                    let t = source.get()?;
                    if t.is_none() || t.is_some_and(|t| t.is('\n')) {
                        break;
                    }
                }
                continue;
            }
        }
        if this.is_backslash() {
            let EscapeResult::Node(node) = escape_outside(source, this, state)?;
            subpattern.push(node);
        } else if !special(this.first) {
            subpattern.push(Node::Literal(this.first));
        } else if this.is('[') {
            let here = source.tell() - 1;
            if source.next.is_some_and(|t| t.is('[')) {
                return Err(unsupported(
                    "re_future_warning",
                    "the parser's 'Possible nested set' FutureWarning text is not reproduced",
                ));
            }
            let negate = source.matches('^')?;
            let mut set: Vec<SetItem> = Vec::new();
            loop {
                let Some(this) = source.get()? else {
                    return Err(with_site(
                        source.error("unterminated character set", source.tell() - here),
                        568,
                    ));
                };
                if this.is(']') && !set.is_empty() {
                    break;
                }
                let code1 = if this.is_backslash() {
                    class_escape(source, this)?
                } else {
                    if !set.is_empty()
                        && this.second.is_none()
                        && matches!(char::from_u32(this.first), Some('-' | '&' | '~' | '|'))
                        && source
                            .next
                            .is_some_and(|t| t.second.is_none() && t.first == this.first)
                    {
                        return Err(unsupported(
                            "re_future_warning",
                            "the parser's 'Possible set operation' FutureWarning text is not reproduced",
                        ));
                    }
                    ClassItem::Literal(this.first)
                };
                if source.matches('-')? {
                    let Some(that) = source.get()? else {
                        return Err(with_site(
                            source.error("unterminated character set", source.tell() - here),
                            591,
                        ));
                    };
                    if that.is(']') {
                        set.push(match code1 {
                            ClassItem::Literal(c) => SetItem::Literal(c),
                            ClassItem::Category(c) => SetItem::Category(c),
                        });
                        set.push(SetItem::Literal(u32::from(b'-')));
                        break;
                    }
                    let code2 = if that.is_backslash() {
                        class_escape(source, that)?
                    } else {
                        if that.is('-') {
                            return Err(unsupported(
                                "re_future_warning",
                                "the parser's 'Possible set difference' FutureWarning text is not reproduced",
                            ));
                        }
                        ClassItem::Literal(that.first)
                    };
                    let (ClassItem::Literal(lo), ClassItem::Literal(hi)) = (&code1, &code2) else {
                        return Err(range_error(source, this, that));
                    };
                    if hi < lo {
                        return Err(range_error(source, this, that));
                    }
                    set.push(SetItem::Range(*lo, *hi));
                } else {
                    set.push(match code1 {
                        ClassItem::Literal(c) => SetItem::Literal(c),
                        ClassItem::Category(c) => SetItem::Category(c),
                    });
                }
            }
            let mut unique: Vec<SetItem> = Vec::new();
            for item in set {
                if !unique.contains(&item) {
                    unique.push(item);
                }
            }
            if let ([SetItem::Literal(c)], _) = (unique.as_slice(), ()) {
                subpattern.push(if negate {
                    Node::NotLiteral(*c)
                } else {
                    Node::Literal(*c)
                });
            } else {
                subpattern.push(Node::Set {
                    negate,
                    items: unique,
                });
            }
        } else if this.second.is_none()
            && matches!(char::from_u32(this.first), Some('*' | '+' | '?' | '{'))
        {
            let here = source.tell();
            let (mut min, mut max) = (0u64, MAXREPEAT);
            if this.is('?') {
                min = 0;
                max = 1;
            } else if this.is('*') {
                min = 0;
                max = MAXREPEAT;
            } else if this.is('+') {
                min = 1;
                max = MAXREPEAT;
            } else {
                if source.next.is_some_and(|t| t.is('}')) {
                    subpattern.push(Node::Literal(this.first));
                    continue;
                }
                let mut lo = String::new();
                let mut hi = String::new();
                while let Some(t) = source.next {
                    if t.second.is_some() || !is_digit(t.first) {
                        break;
                    }
                    lo.push(char::from_u32(t.first).unwrap_or('0'));
                    source.get()?;
                }
                if source.matches(',')? {
                    while let Some(t) = source.next {
                        if t.second.is_some() || !is_digit(t.first) {
                            break;
                        }
                        hi.push(char::from_u32(t.first).unwrap_or('0'));
                        source.get()?;
                    }
                } else {
                    hi = lo.clone();
                }
                if !source.matches('}')? {
                    subpattern.push(Node::Literal(this.first));
                    source.seek(here)?;
                    continue;
                }
                let too_large =
                    || ParseFailure::Overflow("the repetition number is too large".to_string());
                if !lo.is_empty() {
                    min = lo
                        .parse::<u128>()
                        .ok()
                        .filter(|v| *v < u128::from(MAXREPEAT))
                        .ok_or_else(too_large)? as u64;
                }
                if !hi.is_empty() {
                    max = hi
                        .parse::<u128>()
                        .ok()
                        .filter(|v| *v < u128::from(MAXREPEAT))
                        .ok_or_else(too_large)? as u64;
                    if max < min {
                        return Err(source
                            .error("min repeat greater than max repeat", source.tell() - here));
                    }
                }
            }
            let item = subpattern.last();
            let repeat_offset = source.tell() - here + this.len() as i64;
            let Some(item) = item else {
                return Err(source.error("nothing to repeat", repeat_offset));
            };
            if matches!(item, Node::At(_)) {
                return Err(source.error("nothing to repeat", repeat_offset));
            }
            if matches!(item, Node::Repeat { .. }) {
                return Err(source.error("multiple repeat", repeat_offset));
            }
            let body: Seq = match item {
                Node::Sub {
                    group: None,
                    add: 0,
                    del: 0,
                    body,
                } => body.clone(),
                other => vec![other.clone()],
            };
            let kind = if source.matches('?')? {
                RepeatKind::Lazy
            } else if source.matches('+')? {
                RepeatKind::Possessive
            } else {
                RepeatKind::Greedy
            };
            let last = subpattern.len() - 1;
            subpattern[last] = Node::Repeat {
                kind,
                min,
                max,
                body,
            };
        } else if this.is('.') {
            subpattern.push(Node::Any);
        } else if this.is('(') {
            let start = source.tell() - 1;
            let mut capture = true;
            let mut atomic = false;
            let mut name: Option<Vec<u32>> = None;
            let mut add_flags = 0u32;
            let mut del_flags = 0u32;
            if source.matches('?')? {
                let Some(ch) = source.get()? else {
                    return Err(source.error("unexpected end of pattern", 0));
                };
                let c = if ch.second.is_none() {
                    char::from_u32(ch.first).unwrap_or('\0')
                } else {
                    '\\'
                };
                if c == 'P' {
                    if source.matches('<')? {
                        let n = source.getuntil('>', "group name")?;
                        source.check_group_name(&n, 1)?;
                        name = Some(n);
                    } else if source.matches('=')? {
                        let n = source.getuntil(')', "group name")?;
                        source.check_group_name(&n, 1)?;
                        let Some(gid) = state.group_named(&n) else {
                            return Err(source.error_text(
                                &PyStr::new(&format!("unknown group name {}", text_of(&n).repr())),
                                n.len() as i64 + 1,
                            ));
                        };
                        if !state.check_group(gid) {
                            return Err(
                                source.error("cannot refer to an open group", n.len() as i64 + 1)
                            );
                        }
                        state.check_lookbehind_group(gid, source)?;
                        subpattern.push(Node::GroupRef(gid));
                        continue;
                    } else {
                        let Some(next) = source.get()? else {
                            return Err(source.error("unexpected end of pattern", 0));
                        };
                        return Err(source.error(
                            &format!("unknown extension ?P{}", escape_text(&next.points())),
                            next.len() as i64 + 2,
                        ));
                    }
                } else if c == ':' && ch.second.is_none() {
                    capture = false;
                } else if c == '#' && ch.second.is_none() {
                    loop {
                        if source.next.is_none() {
                            return Err(source
                                .error("missing ), unterminated comment", source.tell() - start));
                        }
                        if source.get()?.is_some_and(|t| t.is(')')) {
                            break;
                        }
                    }
                    continue;
                } else if ch.second.is_none() && matches!(c, '=' | '!' | '<') {
                    let mut ahead = true;
                    let mut kind = c;
                    let mut previous_lookbehind = None;
                    if c == '<' {
                        let Some(next) = source.get()? else {
                            return Err(source.error("unexpected end of pattern", 0));
                        };
                        if !(next.second.is_none()
                            && matches!(char::from_u32(next.first), Some('=' | '!')))
                        {
                            return Err(source.error(
                                &format!("unknown extension ?<{}", escape_text(&next.points())),
                                next.len() as i64 + 2,
                            ));
                        }
                        kind = char::from_u32(next.first).unwrap_or('=');
                        ahead = false;
                        previous_lookbehind = state.lookbehindgroups;
                        if previous_lookbehind.is_none() {
                            state.lookbehindgroups = Some(state.groups());
                        }
                    }
                    let body = parse_sub(source, state, verbose, nested + 1)?;
                    if !ahead && previous_lookbehind.is_none() {
                        state.lookbehindgroups = None;
                    }
                    if !source.matches(')')? {
                        return Err(source
                            .error("missing ), unterminated subpattern", source.tell() - start));
                    }
                    subpattern.push(Node::Assert {
                        ahead,
                        negate: kind == '!',
                        body,
                    });
                    continue;
                } else if c == '(' && ch.second.is_none() {
                    let condname = source.getuntil(')', "group name")?;
                    let numeric = condname.iter().all(|&d| is_digit(d));
                    let condgroup;
                    if !numeric {
                        source.check_group_name(&condname, 1)?;
                        let Some(found) = state.group_named(&condname) else {
                            return Err(source.error_text(
                                &PyStr::new(&format!(
                                    "unknown group name {}",
                                    text_of(&condname).repr()
                                )),
                                condname.len() as i64 + 1,
                            ));
                        };
                        condgroup = found;
                    } else {
                        let value: u128 = escape_text(&condname).parse().unwrap_or(u128::MAX);
                        if value == 0 {
                            return Err(source.error("bad group number", condname.len() as i64 + 1));
                        }
                        if value >= u128::from(i32::MAX as u32) {
                            return Err(source.error(
                                &format!("invalid group reference {value}"),
                                condname.len() as i64 + 1,
                            ));
                        }
                        condgroup = value as usize;
                        if !state.grouprefpos.iter().any(|(g, _)| *g == condgroup) {
                            state
                                .grouprefpos
                                .push((condgroup, source.tell() - condname.len() as i64 - 1));
                        }
                    }
                    state.check_lookbehind_group(condgroup, source)?;
                    let yes = parse(source, state, verbose, nested + 1, false).map_err(deeper)?;
                    let no = if source.matches('|')? {
                        let no =
                            parse(source, state, verbose, nested + 1, false).map_err(deeper)?;
                        if source.next.is_some_and(|t| t.is('|')) {
                            return Err(
                                source.error("conditional backref with more than two branches", 0)
                            );
                        }
                        Some(no)
                    } else {
                        None
                    };
                    if !source.matches(')')? {
                        return Err(source
                            .error("missing ), unterminated subpattern", source.tell() - start));
                    }
                    subpattern.push(Node::Exists {
                        group: condgroup,
                        yes,
                        no,
                    });
                    continue;
                } else if c == '>' && ch.second.is_none() {
                    capture = false;
                    atomic = true;
                } else if ch.second.is_none() && (flag_of(ch.first).is_some() || c == '-') {
                    match parse_flags(source, state, ch)? {
                        None => {
                            if !first || !subpattern.is_empty() {
                                return Err(source.error(
                                    "global flags not at the start of the expression",
                                    source.tell() - start,
                                ));
                            }
                            verbose = state.flags & FLAG_VERBOSE != 0;
                            continue;
                        }
                        Some((add, del)) => {
                            add_flags = add;
                            del_flags = del;
                            capture = false;
                        }
                    }
                } else {
                    return Err(source.error(
                        &format!("unknown extension ?{}", escape_text(&ch.points())),
                        ch.len() as i64 + 1,
                    ));
                }
            }
            let group = if capture {
                let gid = state.groups();
                state.groupwidths.push(None);
                if let Some(name) = &name {
                    if let Some(old) = state.group_named(name) {
                        return Err(source.error_text(
                            &PyStr::new(&format!(
                                "redefinition of group name {} as group {gid}; was group {old}",
                                text_of(name).repr()
                            )),
                            name.len() as i64 + 1,
                        ));
                    }
                    state.groupdict.push((name.clone(), gid));
                }
                Some(gid)
            } else {
                None
            };
            let sub_verbose =
                (verbose || add_flags & FLAG_VERBOSE != 0) && del_flags & FLAG_VERBOSE == 0;
            let body = parse_sub(source, state, sub_verbose, nested + 1)?;
            if !source.matches(')')? {
                return Err(
                    source.error("missing ), unterminated subpattern", source.tell() - start)
                );
            }
            if let Some(gid) = group {
                state.groupwidths[gid] = Some(width_of(&body, &state.groupwidths));
            }
            if atomic {
                subpattern.push(Node::Atomic(body));
            } else {
                subpattern.push(Node::Sub {
                    group,
                    add: add_flags,
                    del: del_flags,
                    body,
                });
            }
        } else if this.is('^') {
            subpattern.push(Node::At(At::Beginning));
        } else if this.is('$') {
            subpattern.push(Node::At(At::End));
        } else {
            return Err(unsupported(
                "re_parser_internal",
                "an unsupported special character reached the parser",
            ));
        }
    }
    Ok(subpattern)
}

fn range_error(source: &Source, this: Token, that: Token) -> ParseFailure {
    source.error_text(
        &PyStr::new(&format!(
            "bad character range {}-{}",
            escape_text(&this.points()),
            escape_text(&that.points())
        )),
        (this.len() + 1 + that.len()) as i64,
    )
}

/// The `_compiler._compile` look-behind rule: a look-behind body must have a single width.
fn check_lookbehind(seq: &[Node], widths: &[Option<(u128, u128)>]) -> Result<(), ReError> {
    for node in seq {
        match node {
            Node::Assert { ahead, body, .. } => {
                if !ahead {
                    let (lo, hi) = width_of(body, widths);
                    if lo != hi {
                        return Err(ReError {
                            msg: PyStr::new("look-behind requires fixed-width pattern"),
                            pattern: None,
                            pos: None,
                            sub_depth: 0,
                            site: 0,
                        });
                    }
                }
                check_lookbehind(body, widths)?;
            }
            Node::Branch(items) => {
                for item in items {
                    check_lookbehind(item, widths)?;
                }
            }
            Node::Sub { body, .. } | Node::Repeat { body, .. } | Node::Atomic(body) => {
                check_lookbehind(body, widths)?;
            }
            Node::Exists { yes, no, .. } => {
                check_lookbehind(yes, widths)?;
                if let Some(no) = no {
                    check_lookbehind(no, widths)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// `sre_parse.parse(pattern, flags)` followed by the compile-time checks. `is_text` tells a `str` pattern from a `bytes` one (given as Latin-1 points).
pub fn parse_pattern(chars: Vec<u32>, is_text: bool, flags: u32) -> Parsed<ParsedPattern> {
    let mut source = Source::new(chars, is_text)?;
    let mut state = State {
        flags,
        groupdict: Vec::new(),
        groupwidths: vec![None],
        lookbehindgroups: None,
        grouprefpos: Vec::new(),
    };
    let body = parse_sub(&mut source, &mut state, flags & FLAG_VERBOSE != 0, 0)?;
    // fix_flags
    let mut fixed = state.flags;
    if is_text {
        if fixed & FLAG_LOCALE != 0 {
            return Err(ParseFailure::Value(
                "cannot use LOCALE flag with a str pattern".to_string(),
            ));
        }
        if fixed & FLAG_ASCII == 0 {
            fixed |= FLAG_UNICODE;
        } else if fixed & FLAG_UNICODE != 0 {
            return Err(ParseFailure::Value(
                "ASCII and UNICODE flags are incompatible".to_string(),
            ));
        }
    } else {
        if fixed & FLAG_UNICODE != 0 {
            return Err(ParseFailure::Value(
                "cannot use UNICODE flag with a bytes pattern".to_string(),
            ));
        }
        if fixed & FLAG_LOCALE != 0 && fixed & FLAG_ASCII != 0 {
            return Err(ParseFailure::Value(
                "ASCII and LOCALE flags are incompatible".to_string(),
            ));
        }
    }
    if source.next.is_some() {
        return Err(source.error("unbalanced parenthesis", 0));
    }
    let groups = state.groups();
    let positions: BTreeMap<usize, i64> = BTreeMap::new();
    let _ = positions;
    for (group, pos) in &state.grouprefpos {
        if *group >= groups {
            return Err(ReError {
                msg: PyStr::new(&format!("invalid group reference {group}")),
                pattern: Some(source.pattern()),
                pos: Some(*pos),
                sub_depth: 0,
                site: 0,
            }
            .into());
        }
    }
    check_lookbehind(&body, &state.groupwidths)?;
    Ok(ParsedPattern {
        body,
        flags: fixed,
        groups,
        is_text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(text: &str) -> Vec<u32> {
        text.chars().map(u32::from).collect()
    }

    fn failure(text: &str) -> ReError {
        match parse_pattern(points(text), true, 0) {
            Err(ParseFailure::Re(error)) => error,
            other => panic!("a re.error for {text:?}, not {other:?}"),
        }
    }

    #[test]
    fn an_unterminated_set_is_reported_at_the_set_start_as_python_does() {
        let error = failure("[");
        assert_eq!(error.msg.to_utf8().unwrap(), "unterminated character set");
        assert_eq!(error.pos, Some(0));
        let error = failure("ab[cd");
        assert_eq!(error.pos, Some(2));
    }

    #[test]
    fn quantifier_and_group_errors_carry_the_original_message_and_position() {
        let error = failure("*a");
        assert_eq!(
            (error.msg.to_utf8().unwrap().as_str(), error.pos),
            ("nothing to repeat", Some(0))
        );
        let error = failure("a**");
        assert_eq!(
            (error.msg.to_utf8().unwrap().as_str(), error.pos),
            ("multiple repeat", Some(2))
        );
        let error = failure("(ab");
        assert_eq!(
            error.msg.to_utf8().unwrap(),
            "missing ), unterminated subpattern"
        );
        assert_eq!(error.pos, Some(0));
        let error = failure("ab)");
        assert_eq!(
            (error.msg.to_utf8().unwrap().as_str(), error.pos),
            ("unbalanced parenthesis", Some(2))
        );
        let error = failure("a{3,2}");
        assert_eq!(
            error.msg.to_utf8().unwrap(),
            "min repeat greater than max repeat"
        );
        let error = failure("\\q");
        assert_eq!(
            (error.msg.to_utf8().unwrap().as_str(), error.pos),
            ("bad escape \\q", Some(0))
        );
        let error = failure("ab\\");
        assert_eq!(
            (error.msg.to_utf8().unwrap().as_str(), error.pos),
            ("bad escape (end of pattern)", Some(2))
        );
        let error = failure("(?P<n>a)(?P<n>b)");
        assert!(error
            .msg
            .to_utf8()
            .unwrap()
            .starts_with("redefinition of group name 'n' as group 2; was group 1"));
        let error = failure("(?<=a+)b");
        assert_eq!(
            error.msg.to_utf8().unwrap(),
            "look-behind requires fixed-width pattern"
        );
        assert_eq!(error.pos, None);
    }

    #[test]
    fn well_formed_patterns_parse_and_a_huge_repeat_is_an_overflow_error() {
        for ok in [
            "",
            "(?=aba)",
            "(?<=prefix)AB",
            "(?P<pair>ab)(?P=pair)",
            "\\w+\\s+\\d+",
            "(?i)[a-z]+",
            "a|b",
            "(a)(?(1)b|c)",
            "x{2,3}?",
            "[a-]",
        ] {
            parse_pattern(points(ok), true, 0).unwrap_or_else(|e| panic!("{ok:?}: {e:?}"));
        }
        match parse_pattern(points("a{99999999999}"), true, 0) {
            Err(ParseFailure::Overflow(message)) => {
                assert_eq!(message, "the repetition number is too large")
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            parse_pattern(points("a(?i)"), true, 0),
            Err(ParseFailure::Re(e)) if e.msg.to_utf8().unwrap() == "global flags not at the start of the expression"
        ));
    }
}
