//! A backtracking matcher for the part of Python's `re` (a `str` pattern, CPython 3.12.3) that packaging 24.0 uses for dependency specifiers.
//!
//! The grammar patterns of packaging (`_tokenizer.DEFAULT_RULES`, `Specifier._regex`) are compiled from their original text, so this module
//! has to follow the regex semantics those texts rely on instead of a hand-written reading of them: ordered alternation with backtracking, greedy
//! repeats, capturing groups restored on backtrack, fixed-width look-behind, `\b` and `\s` of Unicode `str` patterns, `$` before a trailing
//! newline, `re.VERBOSE` (blanks and comments of the pattern text) and `re.IGNORECASE` as `re` compiles it for ASCII letters and digits.
//!
//! Supported syntax (anything else is a [`PatternError`], never a silent approximation): literals, escapes `\s \b \t \n \r` and escaped
//! punctuation, classes with ranges and negation, `(...)`, `(?:...)`, `(?P<name>...)`, `(?<=...)`, `(?<!...)`, `|`, `* + ?`, `^`, `$`.

use agent_runtime_support::pystr::{is_python_space, is_python_word};

/// A pattern text this module does not support or that is malformed.
#[derive(Debug)]
pub(super) struct PatternError(pub(super) String);

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported or malformed pattern: {}", self.0)
    }
}

fn unsupported(what: &str) -> PatternError {
    PatternError(what.to_string())
}

#[derive(Debug)]
enum SetItem {
    Char(u32),
    Range(u32, u32),
    /// `\s`
    Space,
}

#[derive(Debug)]
struct CharSet {
    negated: bool,
    items: Vec<SetItem>,
}

#[derive(Debug)]
enum Node {
    Literal(u32),
    Set(CharSet),
    Sequence(Vec<Node>),
    Alternation(Vec<Node>),
    Repeat {
        body: Box<Node>,
        min: usize,
        max: Option<usize>,
    },
    Group {
        body: Box<Node>,
        index: Option<usize>,
    },
    WordBoundary,
    StartOfText,
    /// `$` without `re.MULTILINE`: the end, or just before a newline that ends the text.
    EndOfText,
    LookBehind {
        body: Box<Node>,
        width: usize,
        negative: bool,
    },
}

/// The simple lowercase of one character as far as it can reach ASCII or the two characters `re` equates with `i` and `s` (U+0130 and the Kelvin
/// sign lower-case to ASCII letters).
fn simple_lower(code_point: u32) -> u32 {
    match code_point {
        0x41..=0x5A => code_point + 32,
        0x130 => 0x69,
        0x212A => 0x6B,
        other => other,
    }
}

/// The characters that stand for the lowercase ASCII `letter` under `re.IGNORECASE`: its simple lowercase is the letter, or the equivalences
/// `re` adds (`i` ~ U+0131, `s` ~ U+017F).
fn equals_ignoring_case(code_point: u32, literal: u32) -> bool {
    let lower = simple_lower(code_point);
    lower == simple_lower(literal)
        || (simple_lower(literal) == 0x69 && lower == 0x131)
        || (simple_lower(literal) == 0x73 && lower == 0x17F)
}

impl CharSet {
    fn holds(&self, code_point: u32) -> bool {
        self.items.iter().any(|item| match item {
            SetItem::Char(member) => *member == code_point,
            SetItem::Range(first, last) => (*first..=*last).contains(&code_point),
            SetItem::Space => is_python_space(code_point),
        })
    }

    /// Membership of the set `re` builds for `IGNORECASE`: the lowercase of the character is looked up, and a range holding `i` or `s` also holds its
    /// equivalent U+0131 / U+017F.
    fn holds_ignoring_case(&self, code_point: u32) -> bool {
        let lower = simple_lower(code_point);
        self.holds(lower)
            || (lower == 0x131 && self.holds(0x69))
            || (lower == 0x17F && self.holds(0x73))
    }

    fn matches(&self, code_point: u32, ignore_case: bool) -> bool {
        let inside = if ignore_case {
            self.holds_ignoring_case(code_point)
        } else {
            self.holds(code_point)
        };
        inside != self.negated
    }
}

/// The result of a successful match: the span of the whole match and of every capturing group.
pub(super) struct Match {
    spans: Vec<Option<(usize, usize)>>,
    names: Vec<(String, usize)>,
}

impl Match {
    /// `match.span(0)`.
    pub(super) fn whole(&self) -> (usize, usize) {
        self.spans[0].expect("a match always has its whole span")
    }

    /// `match.span(name)` of a named group (`None` when the group did not take part).
    pub(super) fn named(&self, name: &str) -> Option<(usize, usize)> {
        let (_, index) = self.names.iter().find(|(known, _)| known == name)?;
        self.spans[*index]
    }
}

/// A compiled pattern.
pub(super) struct Pattern {
    root: Node,
    group_count: usize,
    names: Vec<(String, usize)>,
    ignore_case: bool,
}

struct Parser<'a> {
    text: &'a [char],
    at: usize,
    verbose: bool,
    group_count: usize,
    names: Vec<(String, usize)>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.text.get(self.at).copied()
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn eat_str(&mut self, expected: &str) -> bool {
        let wanted: Vec<char> = expected.chars().collect();
        if self.text[self.at..].starts_with(&wanted) {
            self.at += wanted.len();
            true
        } else {
            false
        }
    }

    /// The blanks and `#` comments `re.VERBOSE` ignores between pattern items.
    fn skip_verbose_blanks(&mut self) {
        if !self.verbose {
            return;
        }
        while let Some(c) = self.peek() {
            if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c') {
                self.at += 1;
            } else if c == '#' {
                while self.peek().is_some_and(|c| c != '\n') {
                    self.at += 1;
                }
            } else {
                break;
            }
        }
    }

    fn alternation(&mut self) -> Result<Node, PatternError> {
        let mut branches = vec![self.sequence()?];
        while self.eat('|') {
            branches.push(self.sequence()?);
        }
        Ok(if branches.len() == 1 {
            branches.remove(0)
        } else {
            Node::Alternation(branches)
        })
    }

    fn sequence(&mut self) -> Result<Node, PatternError> {
        let mut items: Vec<Node> = Vec::new();
        loop {
            self.skip_verbose_blanks();
            let Some(c) = self.peek() else { break };
            match c {
                '|' | ')' => break,
                '*' | '+' | '?' => {
                    self.at += 1;
                    let (min, max) = match c {
                        '*' => (0, None),
                        '+' => (1, None),
                        _ => (0, Some(1)),
                    };
                    if self.peek() == Some('?') || self.peek() == Some('+') {
                        return Err(unsupported("lazy or possessive repeat"));
                    }
                    let body = items
                        .pop()
                        .ok_or_else(|| unsupported("nothing to repeat"))?;
                    if matches!(
                        body,
                        Node::StartOfText | Node::EndOfText | Node::WordBoundary
                    ) {
                        return Err(unsupported("repeat of an assertion"));
                    }
                    items.push(Node::Repeat {
                        body: Box::new(body),
                        min,
                        max,
                    });
                }
                '{' => return Err(unsupported("counted repeat")),
                _ => {
                    let node = self.atom()?;
                    items.push(node);
                }
            }
        }
        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Node::Sequence(items)
        })
    }

    fn atom(&mut self) -> Result<Node, PatternError> {
        let c = self.peek().expect("atom is only asked at a character");
        self.at += 1;
        match c {
            '(' => self.group(),
            '[' => self.set(),
            '\\' => self.escape(),
            '^' => Ok(Node::StartOfText),
            '$' => Ok(Node::EndOfText),
            '.' => Err(unsupported("any-character")),
            other => Ok(Node::Literal(u32::from(other))),
        }
    }

    fn close_group(&mut self) -> Result<(), PatternError> {
        if self.eat(')') {
            Ok(())
        } else {
            Err(unsupported("missing )"))
        }
    }

    fn group(&mut self) -> Result<Node, PatternError> {
        if !self.eat('?') {
            self.group_count += 1;
            let index = self.group_count;
            let body = self.alternation()?;
            self.close_group()?;
            return Ok(Node::Group {
                body: Box::new(body),
                index: Some(index),
            });
        }
        if self.eat(':') {
            let body = self.alternation()?;
            self.close_group()?;
            return Ok(Node::Group {
                body: Box::new(body),
                index: None,
            });
        }
        if self.eat_str("P<") {
            let mut name = String::new();
            while let Some(c) = self.peek() {
                self.at += 1;
                if c == '>' {
                    break;
                }
                name.push(c);
            }
            self.group_count += 1;
            let index = self.group_count;
            self.names.push((name, index));
            let body = self.alternation()?;
            self.close_group()?;
            return Ok(Node::Group {
                body: Box::new(body),
                index: Some(index),
            });
        }
        let negative = if self.eat_str("<=") {
            false
        } else if self.eat_str("<!") {
            true
        } else {
            return Err(unsupported("group extension"));
        };
        let body = self.alternation()?;
        self.close_group()?;
        let width =
            fixed_width(&body).ok_or_else(|| unsupported("look-behind needs a fixed width"))?;
        Ok(Node::LookBehind {
            body: Box::new(body),
            width,
            negative,
        })
    }

    fn escaped_char(c: char) -> Result<u32, PatternError> {
        Ok(match c {
            't' => 9,
            'n' => 10,
            'r' => 13,
            c if c.is_ascii_alphanumeric() => return Err(unsupported("escape letter")),
            c => u32::from(c),
        })
    }

    fn escape(&mut self) -> Result<Node, PatternError> {
        let c = self
            .peek()
            .ok_or_else(|| unsupported("pattern ends with a backslash"))?;
        self.at += 1;
        Ok(match c {
            'b' => Node::WordBoundary,
            's' => Node::Set(CharSet {
                negated: false,
                items: vec![SetItem::Space],
            }),
            other => Node::Literal(Self::escaped_char(other)?),
        })
    }

    fn set(&mut self) -> Result<Node, PatternError> {
        let negated = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return Err(unsupported("unterminated class"));
            };
            self.at += 1;
            if c == ']' && !first {
                break;
            }
            first = false;
            let low = if c == '\\' {
                let escaped = self
                    .peek()
                    .ok_or_else(|| unsupported("unterminated class"))?;
                self.at += 1;
                if escaped == 's' {
                    items.push(SetItem::Space);
                    continue;
                }
                Self::escaped_char(escaped)?
            } else {
                u32::from(c)
            };
            let is_range = self.peek() == Some('-')
                && self.text.get(self.at + 1).is_some_and(|&next| next != ']');
            if is_range {
                self.at += 1;
                let high_char = self.peek().expect("range end checked above");
                self.at += 1;
                let high = if high_char == '\\' {
                    let escaped = self
                        .peek()
                        .ok_or_else(|| unsupported("unterminated class"))?;
                    self.at += 1;
                    Self::escaped_char(escaped)?
                } else {
                    u32::from(high_char)
                };
                if high < low {
                    return Err(unsupported("bad character range"));
                }
                items.push(SetItem::Range(low, high));
            } else {
                items.push(SetItem::Char(low));
            }
        }
        Ok(Node::Set(CharSet { negated, items }))
    }
}

fn fixed_width(node: &Node) -> Option<usize> {
    match node {
        Node::Literal(_) | Node::Set(_) => Some(1),
        Node::Sequence(items) => items.iter().map(fixed_width).sum(),
        Node::Alternation(branches) => {
            let first = fixed_width(branches.first()?)?;
            branches
                .iter()
                .all(|branch| fixed_width(branch) == Some(first))
                .then_some(first)
        }
        Node::Group { body, .. } => fixed_width(body),
        Node::Repeat { body, min, max } if Some(*min) == *max => {
            fixed_width(body).map(|width| width * min)
        }
        Node::WordBoundary | Node::StartOfText | Node::EndOfText => Some(0),
        Node::Repeat { .. } | Node::LookBehind { .. } => None,
    }
}

impl Pattern {
    /// Compiles `source` with `re.VERBOSE` (`verbose`) and `re.IGNORECASE` (`ignore_case`) as given.
    pub(super) fn compile(
        source: &str,
        verbose: bool,
        ignore_case: bool,
    ) -> Result<Pattern, PatternError> {
        let text: Vec<char> = source.chars().collect();
        let mut parser = Parser {
            text: &text,
            at: 0,
            verbose,
            group_count: 0,
            names: Vec::new(),
        };
        let root = parser.alternation()?;
        if parser.at != text.len() {
            return Err(unsupported("unbalanced )"));
        }
        Ok(Pattern {
            root,
            group_count: parser.group_count,
            names: parser.names,
            ignore_case,
        })
    }

    /// `pattern.match(text, start)`.
    pub(super) fn match_at(&self, text: &[u32], start: usize) -> Option<Match> {
        let mut run = Run {
            text,
            ignore_case: self.ignore_case,
            spans: vec![None; self.group_count + 1],
        };
        let mut end = None;
        let found = run.step(&self.root, start, &mut |_, position| {
            end = Some(position);
            true
        });
        if !found {
            return None;
        }
        run.spans[0] = Some((start, end.expect("a found match has an end")));
        Some(Match {
            spans: run.spans,
            names: self.names.clone(),
        })
    }

    /// `pattern.search(text)`: the leftmost start with a match.
    pub(super) fn search(&self, text: &[u32]) -> Option<Match> {
        (0..=text.len()).find_map(|start| self.match_at(text, start))
    }
}

/// What the rest of a match does after a node matched up to a position (`true` when the whole match goes through).
type Next<'n> = &'n mut dyn FnMut(&mut Run<'_>, usize) -> bool;

struct Run<'t> {
    text: &'t [u32],
    ignore_case: bool,
    spans: Vec<Option<(usize, usize)>>,
}

impl Run<'_> {
    fn at(&self, position: usize) -> Option<u32> {
        self.text.get(position).copied()
    }

    fn is_word_at(&self, position: usize) -> bool {
        self.at(position).is_some_and(is_python_word)
    }

    fn step(&mut self, node: &Node, position: usize, next: Next<'_>) -> bool {
        match node {
            Node::Literal(wanted) => match self.at(position) {
                Some(found)
                    if found == *wanted
                        || (self.ignore_case && equals_ignoring_case(found, *wanted)) =>
                {
                    next(self, position + 1)
                }
                _ => false,
            },
            Node::Set(set) => match self.at(position) {
                Some(found) if set.matches(found, self.ignore_case) => next(self, position + 1),
                _ => false,
            },
            Node::Sequence(items) => self.sequence(items, position, next),
            Node::Alternation(branches) => {
                for branch in branches {
                    if self.step(branch, position, &mut *next) {
                        return true;
                    }
                }
                false
            }
            Node::Repeat { body, min, max } => self.repeat(body, *min, *max, 0, position, next),
            Node::Group { body, index } => match index {
                None => self.step(body, position, next),
                Some(index) => {
                    let index = *index;
                    let outer = self.spans[index];
                    let found = self.step(body, position, &mut |run, end| {
                        let before = run.spans[index];
                        run.spans[index] = Some((position, end));
                        if next(run, end) {
                            true
                        } else {
                            run.spans[index] = before;
                            false
                        }
                    });
                    if !found {
                        self.spans[index] = outer;
                    }
                    found
                }
            },
            Node::WordBoundary => {
                let before = position > 0 && self.is_word_at(position - 1);
                let after = self.is_word_at(position);
                before != after && next(self, position)
            }
            Node::StartOfText => position == 0 && next(self, position),
            Node::EndOfText => {
                let length = self.text.len();
                let at_end = position == length
                    || (position + 1 == length && self.text[position] == u32::from('\n'));
                at_end && next(self, position)
            }
            Node::LookBehind {
                body,
                width,
                negative,
            } => {
                let matched = position >= *width
                    && self.step(body, position - width, &mut |_, end| end == position);
                (matched != *negative) && next(self, position)
            }
        }
    }

    fn sequence(&mut self, items: &[Node], position: usize, next: Next<'_>) -> bool {
        match items.split_first() {
            None => next(self, position),
            Some((first, rest)) => self.step(first, position, &mut |run, after| {
                run.sequence(rest, after, &mut *next)
            }),
        }
    }

    /// Whether a one-character node (a literal or a class) matches the character at `position`.
    fn single_matches(&self, node: &Node, position: usize) -> bool {
        match (node, self.at(position)) {
            (Node::Literal(wanted), Some(found)) => {
                found == *wanted || (self.ignore_case && equals_ignoring_case(found, *wanted))
            }
            (Node::Set(set), Some(found)) => set.matches(found, self.ignore_case),
            _ => false,
        }
    }

    /// A greedy repeat: one more iteration is tried before stopping; an iteration that consumes nothing ends the loop as in `re`.
    ///
    /// A repeat of a single character takes the longest run first and gives characters back one at a time, which is the same order of tries
    /// without a recursion level per character.
    fn repeat(
        &mut self,
        body: &Node,
        min: usize,
        max: Option<usize>,
        done: usize,
        position: usize,
        next: Next<'_>,
    ) -> bool {
        if matches!(body, Node::Literal(_) | Node::Set(_)) {
            let mut run = 0;
            while max.is_none_or(|limit| run < limit) && self.single_matches(body, position + run) {
                run += 1;
            }
            return (min..=run).rev().any(|taken| next(self, position + taken));
        }
        if max.is_none_or(|limit| done < limit) {
            let matched = self.step(body, position, &mut |run, after| {
                if after == position && done >= min {
                    return false;
                }
                run.repeat(body, min, max, done + 1, after, &mut *next)
            });
            if matched {
                return true;
            }
        }
        done >= min && next(self, position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(text: &str) -> Vec<u32> {
        text.chars().map(u32::from).collect()
    }

    #[test]
    fn alternation_backtracks_into_the_rest_of_the_pattern() {
        let pattern = Pattern::compile("(?P<op>==|===)x", false, false).unwrap();
        let found = pattern.match_at(&points("===x"), 0).unwrap();
        assert_eq!(found.whole(), (0, 4));
        assert_eq!(found.named("op"), Some((0, 3)));
    }

    #[test]
    fn dollar_matches_before_a_final_newline_only() {
        let pattern = Pattern::compile("a$", false, false).unwrap();
        assert!(pattern.search(&points("a\n")).is_some());
        assert!(pattern.search(&points("a\n\n")).is_none());
    }

    #[test]
    fn verbose_ignores_blanks_and_comments() {
        let pattern = Pattern::compile("a  # one ( a\n b", true, false).unwrap();
        assert!(pattern.match_at(&points("ab"), 0).is_some());
    }

    #[test]
    fn look_behind_reads_the_text_before_the_position() {
        let pattern = Pattern::compile("(?<==)x", false, false).unwrap();
        assert!(pattern.match_at(&points("=x"), 1).is_some());
        assert!(pattern.match_at(&points("ax"), 1).is_none());
    }

    #[test]
    fn ignore_case_follows_the_re_equivalences() {
        let pattern = Pattern::compile("[a-z]s", false, true).unwrap();
        assert!(pattern.match_at(&points("K\u{17F}"), 0).is_some());
        assert!(pattern.match_at(&points("\u{130}S"), 0).is_some());
    }

    #[test]
    fn word_boundary_is_unicode() {
        let pattern = Pattern::compile(r"\bor\b", false, false).unwrap();
        assert!(pattern.match_at(&points("or "), 0).is_some());
        assert!(pattern.match_at(&points("or\u{e9}"), 0).is_none());
    }
}
