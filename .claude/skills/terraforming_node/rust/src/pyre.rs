//! A backtracking matcher with CPython `re` semantics for the pattern subset the ported programs use: literals and
//! escapes, `.`, classes (`[...]`, `[^...]`, ranges, `\d \s \S \w` inside), `\d \s \S \w \b`, `^ $` (no MULTILINE: `$` also
//! before a final newline), capturing and `(?:...)` groups, `|`, and the quantifiers `* + ? {n} {n,m}` (greedy, or lazy
//! with a trailing `?`). Matching is code-point based; `\d` is Unicode decimal, `\s` is `str.isspace()`, `\w` is
//! alphanumeric or `_`. The first successful path in priority order is the match, as in CPython's engine.

use crate::py;
use agent_runtime_support::pystr::is_decimal_digit;

#[derive(Debug, Clone)]
enum Item {
    Ch(char),
    Range(char, char),
    Digit,
    Space,
    NotSpace,
    Word,
}

#[derive(Debug, Clone)]
enum Node {
    Ch(char),
    Any,
    Class(Vec<Item>, bool),
    Digit,
    Space,
    NotSpace,
    Word,
    Boundary,
    Start,
    End,
    Group(Box<Node>, Option<usize>),
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Rep(Box<Node>, usize, usize, bool),
}

#[derive(Debug, Clone)]
pub struct Regex {
    node: Node,
    groups: usize,
}

struct Parser<'a> {
    p: &'a [char],
    i: usize,
    groups: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.p.get(self.i).copied()
    }
    fn alt(&mut self) -> Node {
        let mut alts = vec![self.seq()];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.seq());
        }
        if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            Node::Alt(alts)
        }
    }
    fn seq(&mut self) -> Node {
        let mut v = vec![];
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom();
            v.push(self.quant(atom));
        }
        Node::Seq(v)
    }
    fn escape(&mut self) -> Node {
        let c = self.p[self.i];
        self.i += 1;
        match c {
            'd' => Node::Digit,
            's' => Node::Space,
            'S' => Node::NotSpace,
            'w' => Node::Word,
            'b' => Node::Boundary,
            'n' => Node::Ch('\n'),
            't' => Node::Ch('\t'),
            other => Node::Ch(other),
        }
    }
    fn class(&mut self) -> Node {
        let mut neg = false;
        if self.peek() == Some('^') {
            neg = true;
            self.i += 1;
        }
        let mut items = vec![];
        let mut first = true;
        while let Some(c) = self.peek() {
            if c == ']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            self.i += 1;
            let lo = if c == '\\' {
                let e = self.p[self.i];
                self.i += 1;
                match e {
                    'd' => {
                        items.push(Item::Digit);
                        continue;
                    }
                    's' => {
                        items.push(Item::Space);
                        continue;
                    }
                    'S' => {
                        items.push(Item::NotSpace);
                        continue;
                    }
                    'w' => {
                        items.push(Item::Word);
                        continue;
                    }
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.p.get(self.i + 1).is_some_and(|&x| x != ']') {
                self.i += 1;
                let mut hi = self.p[self.i];
                self.i += 1;
                if hi == '\\' {
                    hi = self.p[self.i];
                    self.i += 1;
                }
                items.push(Item::Range(lo, hi));
            } else {
                items.push(Item::Ch(lo));
            }
        }
        Node::Class(items, neg)
    }
    fn atom(&mut self) -> Node {
        let c = self.p[self.i];
        self.i += 1;
        match c {
            '(' => {
                let idx = if self.p[self.i..].starts_with(&['?', ':']) {
                    self.i += 2;
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let inner = self.alt();
                self.i += 1; // ')'
                Node::Group(Box::new(inner), idx)
            }
            '[' => self.class(),
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '\\' => self.escape(),
            other => Node::Ch(other),
        }
    }
    fn number(&mut self) -> usize {
        let mut n = 0;
        while let Some(d) = self.peek().and_then(|c| c.to_digit(10)) {
            n = n * 10 + d as usize;
            self.i += 1;
        }
        n
    }
    fn quant(&mut self, atom: Node) -> Node {
        let (min, max) = match self.peek() {
            Some('*') => {
                self.i += 1;
                (0, usize::MAX)
            }
            Some('+') => {
                self.i += 1;
                (1, usize::MAX)
            }
            Some('?') => {
                self.i += 1;
                (0, 1)
            }
            Some('{') => {
                self.i += 1;
                let lo = self.number();
                let hi = if self.peek() == Some(',') {
                    self.i += 1;
                    if self.peek() == Some('}') {
                        usize::MAX
                    } else {
                        self.number()
                    }
                } else {
                    lo
                };
                self.i += 1; // '}'
                (lo, hi)
            }
            _ => return atom,
        };
        let greedy = if self.peek() == Some('?') {
            self.i += 1;
            false
        } else {
            true
        };
        Node::Rep(Box::new(atom), min, max, greedy)
    }
}

type Caps = Vec<Option<(usize, usize)>>;
type K<'k> = &'k mut dyn FnMut(&mut Caps, usize) -> bool;

fn item_hit(it: &Item, c: char) -> bool {
    match it {
        Item::Ch(x) => c == *x,
        Item::Range(a, b) => c >= *a && c <= *b,
        Item::Digit => is_decimal_digit(c as u32),
        Item::Space => py::is_space(c),
        Item::NotSpace => !py::is_space(c),
        Item::Word => py::is_word(c),
    }
}

fn atom_hit(n: &Node, c: char) -> Option<bool> {
    Some(match n {
        Node::Ch(x) => c == *x,
        Node::Any => c != '\n',
        Node::Class(items, neg) => items.iter().any(|it| item_hit(it, c)) != *neg,
        Node::Digit => is_decimal_digit(c as u32),
        Node::Space => py::is_space(c),
        Node::NotSpace => !py::is_space(c),
        Node::Word => py::is_word(c),
        _ => return None,
    })
}

fn m(s: &[char], n: &Node, pos: usize, caps: &mut Caps, k: K) -> bool {
    if let Some(hit) = s.get(pos).and_then(|&c| atom_hit(n, c)) {
        return hit && k(caps, pos + 1);
    }
    match n {
        Node::Ch(_)
        | Node::Any
        | Node::Class(..)
        | Node::Digit
        | Node::Space
        | Node::NotSpace
        | Node::Word => false,
        Node::Start => pos == 0 && k(caps, pos),
        Node::End => (pos == s.len() || (pos + 1 == s.len() && s[pos] == '\n')) && k(caps, pos),
        Node::Boundary => {
            let before = pos > 0 && py::is_word(s[pos - 1]);
            let after = pos < s.len() && py::is_word(s[pos]);
            before != after && k(caps, pos)
        }
        Node::Seq(v) => seq(s, v, 0, pos, caps, k),
        Node::Alt(v) => {
            for a in v {
                if m(s, a, pos, caps, k) {
                    return true;
                }
            }
            false
        }
        Node::Group(inner, idx) => {
            let start = pos;
            let g = *idx;
            m(s, inner, pos, caps, &mut |c: &mut Caps, p: usize| {
                let Some(g) = g else { return k(c, p) };
                let saved = c[g];
                c[g] = Some((start, p));
                if k(c, p) {
                    return true;
                }
                c[g] = saved;
                false
            })
        }
        Node::Rep(inner, min, max, greedy) => rep(s, inner, *min, *max, *greedy, 0, pos, caps, k),
    }
}

fn seq(s: &[char], v: &[Node], idx: usize, pos: usize, caps: &mut Caps, k: K) -> bool {
    if idx == v.len() {
        return k(caps, pos);
    }
    m(s, &v[idx], pos, caps, &mut |c: &mut Caps, p: usize| {
        seq(s, v, idx + 1, p, c, k)
    })
}

#[allow(clippy::too_many_arguments)]
fn rep(
    s: &[char],
    n: &Node,
    min: usize,
    max: usize,
    greedy: bool,
    count: usize,
    pos: usize,
    caps: &mut Caps,
    k: K,
) -> bool {
    let more = |caps: &mut Caps, k: K| {
        count < max
            && m(s, n, pos, caps, &mut |c: &mut Caps, p: usize| {
                if p == pos && count >= min {
                    return false;
                }
                rep(s, n, min, max, greedy, count + 1, p, c, k)
            })
    };
    if greedy {
        if more(caps, k) {
            return true;
        }
        count >= min && k(caps, pos)
    } else {
        if count >= min && k(caps, pos) {
            return true;
        }
        more(caps, k)
    }
}

/// One match: the text of every group (index 0 = the whole match).
#[derive(Debug, Clone)]
pub struct Match {
    pub groups: Vec<Option<String>>,
    pub start: usize,
    pub end: usize,
}

impl Match {
    pub fn group(&self, i: usize) -> Option<&str> {
        self.groups.get(i).and_then(|g| g.as_deref())
    }
}

impl Regex {
    pub fn new(pattern: &str) -> Regex {
        let p: Vec<char> = pattern.chars().collect();
        let mut parser = Parser {
            p: &p,
            i: 0,
            groups: 0,
        };
        let node = parser.alt();
        Regex {
            node,
            groups: parser.groups,
        }
    }

    fn at(&self, s: &[char], start: usize) -> Option<Match> {
        let mut caps: Caps = vec![None; self.groups + 1];
        let mut end = None;
        let ok = m(
            s,
            &self.node,
            start,
            &mut caps,
            &mut |c: &mut Caps, p: usize| {
                c[0] = Some((start, p));
                end = Some(p);
                true
            },
        );
        if !ok {
            return None;
        }
        let groups = caps
            .iter()
            .map(|g| g.map(|(a, b)| s[a..b].iter().collect()))
            .collect();
        Some(Match {
            groups,
            start,
            end: end.unwrap_or(start),
        })
    }

    /// `re.match` (anchored at the start).
    pub fn match_(&self, text: &str) -> Option<Match> {
        let s: Vec<char> = text.chars().collect();
        self.at(&s, 0)
    }

    /// `re.search`.
    pub fn search(&self, text: &str) -> Option<Match> {
        let s: Vec<char> = text.chars().collect();
        (0..=s.len()).find_map(|i| self.at(&s, i))
    }
}
