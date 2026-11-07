//! `parse_vllm_log.py`: the memory breakdown of a vLLM serving log.
//!
//! `parse_log(text)` searches a fixed set of case-insensitive patterns (preferred forms before fallbacks, the first match of each),
//! converts the numbers (a comma-grouped number is `float(s.replace(",", ""))`, the token count `int(...)`) and returns twelve
//! fields and `raw_matched` in the original's order. A match whose number cannot be converted raises (`ValueError`, including the
//! interpreter's limit of 4300 digits for an integer text); no failed conversion becomes `None`, and a missing line is `None`.
//!
//! The patterns are the original's fixed source patterns, not a general regular-expression interface. They are matched by a small
//! backtracking matcher over code points with the semantics of CPython's `re` for exactly what they use: `IGNORECASE` on a Unicode
//! string (an ASCII letter also matches the characters that lower-case to it or are folded with it: `İ` and `ı` for `i`, `ſ` for `s`,
//! the Kelvin sign for `k`), `\s` as `str.isspace`, `\d` as the Unicode decimal digits, `\b` between a word character
//! (alphanumeric or `_`) and anything else, and `.` as any character but a line feed. Rust's own regular expressions are not assumed
//! to agree with these (their Unicode version and word rules differ), and the common package provides the character predicates.

use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pynumber::{float_from_str, int_from_str};
use agent_runtime_support::pystr::{is_decimal_digit, is_python_space, is_python_word, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The module name the original is known by.
pub const MODULE: &str = "parse_vllm_log";
/// `_NUM` and `_GIB`: the source's two building blocks, as text (this owner's canonical constants).
pub const NUM: &str = r"([\d,]+(?:\.\d+)?)";
pub const GIB: &str = r"\s*Gi?B";

// ── the matcher ──────────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum ClassItem {
    Digit,
    Char(char),
}

#[derive(Clone)]
enum Atom {
    Literal(char),
    Space,
    Digit,
    Any,
    Class(Vec<ClassItem>),
    /// `(?:a|b)`: alternatives, each a sequence.
    Group(Vec<Vec<Item>>),
    Boundary,
    CaptureStart,
    CaptureEnd,
}

#[derive(Clone)]
struct Item {
    atom: Atom,
    min: usize,
    max: usize,
    lazy: bool,
}

const MANY: usize = usize::MAX;

fn once(atom: Atom) -> Item {
    Item {
        atom,
        min: 1,
        max: 1,
        lazy: false,
    }
}

fn repeated(atom: Atom, min: usize, max: usize, lazy: bool) -> Item {
    Item {
        atom,
        min,
        max,
        lazy,
    }
}

/// A literal text, one item per character.
fn text(literal: &str) -> Vec<Item> {
    literal.chars().map(|c| once(Atom::Literal(c))).collect()
}

fn spaces() -> Item {
    repeated(Atom::Space, 1, MANY, false)
}

fn maybe_spaces() -> Item {
    repeated(Atom::Space, 0, MANY, false)
}

fn join(parts: Vec<Vec<Item>>) -> Vec<Item> {
    parts.into_iter().flatten().collect()
}

/// `\s+` between the words of a phrase: `"a b c"` is `a\s+b\s+c`.
fn phrase(words: &str) -> Vec<Item> {
    let mut items = Vec::new();
    for (position, word) in words.split(' ').enumerate() {
        if position > 0 {
            items.push(spaces());
        }
        items.extend(text(word));
    }
    items
}

/// `_NUM`: `([\d,]+(?:\.\d+)?)`, the number in group 1.
fn number() -> Vec<Item> {
    vec![
        once(Atom::CaptureStart),
        repeated(
            Atom::Class(vec![ClassItem::Digit, ClassItem::Char(',')]),
            1,
            MANY,
            false,
        ),
        repeated(
            Atom::Group(vec![vec![
                once(Atom::Literal('.')),
                repeated(Atom::Digit, 1, MANY, false),
            ]]),
            0,
            1,
            false,
        ),
        once(Atom::CaptureEnd),
    ]
}

/// `_GIB`: `\s*Gi?B`.
fn gib() -> Vec<Item> {
    vec![
        maybe_spaces(),
        once(Atom::Literal('g')),
        repeated(Atom::Literal('i'), 0, 1, false),
        once(Atom::Literal('b')),
    ]
}

/// What `IGNORECASE` makes a pattern character match: the character, the characters whose lower case it is, and the characters
/// CPython folds with it (`i`/`ı`, `s`/`ſ`; the Kelvin sign and `İ` lower-case to `k` and `i`).
fn literal_matches(pattern: char, found: u32) -> bool {
    let Some(found) = char::from_u32(found) else {
        return false;
    };
    if pattern.is_ascii_alphabetic() {
        let lower = pattern.to_ascii_lowercase();
        if found.to_ascii_lowercase() == lower && found.is_ascii() {
            return true;
        }
        return match lower {
            'i' => matches!(found, '\u{130}' | '\u{131}'),
            's' => found == '\u{17f}',
            'k' => found == '\u{212a}',
            _ => false,
        };
    }
    found == pattern
}

fn class_matches(items: &[ClassItem], found: u32) -> bool {
    items.iter().any(|item| match item {
        ClassItem::Digit => is_decimal_digit(found),
        ClassItem::Char(c) => found == u32::from(*c),
    })
}

type Captures = (Option<usize>, Option<usize>);

struct Matcher<'t> {
    text: &'t [u32],
}

impl Matcher<'_> {
    fn single(&self, atom: &Atom, at: usize) -> bool {
        let Some(&found) = self.text.get(at) else {
            return false;
        };
        match atom {
            Atom::Literal(c) => literal_matches(*c, found),
            Atom::Space => is_python_space(found),
            Atom::Digit => is_decimal_digit(found),
            Atom::Any => found != u32::from('\n'),
            Atom::Class(items) => class_matches(items, found),
            _ => false,
        }
    }

    fn is_word_at(&self, at: usize) -> bool {
        self.text.get(at).is_some_and(|&c| is_python_word(c))
    }

    fn run(
        &self,
        items: &[Item],
        pos: usize,
        caps: &mut Captures,
        done: &mut dyn FnMut(usize, &mut Captures) -> bool,
    ) -> bool {
        let Some((item, rest)) = items.split_first() else {
            return done(pos, caps);
        };
        match &item.atom {
            Atom::CaptureStart => {
                let saved = caps.0;
                caps.0 = Some(pos);
                if self.run(rest, pos, caps, done) {
                    return true;
                }
                caps.0 = saved;
                false
            }
            Atom::CaptureEnd => {
                let saved = caps.1;
                caps.1 = Some(pos);
                if self.run(rest, pos, caps, done) {
                    return true;
                }
                caps.1 = saved;
                false
            }
            Atom::Boundary => {
                let before = pos > 0 && self.is_word_at(pos - 1);
                before != self.is_word_at(pos) && self.run(rest, pos, caps, done)
            }
            Atom::Group(alternatives) => {
                // Only `(?:...)` and `(?:...)?` occur: one try of any alternative, then (when optional) none.
                let with =
                    |caps: &mut Captures, done: &mut dyn FnMut(usize, &mut Captures) -> bool| {
                        alternatives.iter().any(|alternative| {
                            self.run(alternative, pos, caps, &mut |end, caps| {
                                self.run(rest, end, caps, done)
                            })
                        })
                    };
                if with(caps, done) {
                    return true;
                }
                item.min == 0 && self.run(rest, pos, caps, done)
            }
            atom => {
                let mut count = 0;
                while count < item.max && self.single(atom, pos + count) {
                    count += 1;
                }
                if count < item.min {
                    return false;
                }
                if item.lazy {
                    (item.min..=count).any(|used| self.run(rest, pos + used, caps, done))
                } else {
                    (item.min..=count)
                        .rev()
                        .any(|used| self.run(rest, pos + used, caps, done))
                }
            }
        }
    }
}

/// `re.search(pattern, text, re.IGNORECASE)`: the text of group 1 of the leftmost match, `None` when nothing matches.
fn search(items: &[Item], points: &[u32]) -> Option<PyStr> {
    let matcher = Matcher { text: points };
    for start in 0..=points.len() {
        let mut caps: Captures = (None, None);
        let mut found: Option<Captures> = None;
        if matcher.run(items, start, &mut caps, &mut |_, caps| {
            found = Some(*caps);
            true
        }) {
            return found.and_then(|(from, to)| match (from, to) {
                (Some(from), Some(to)) => Some(PyStr::from_code_points(points[from..to].to_vec())),
                _ => None,
            });
        }
    }
    None
}

// ── the patterns (the original's, in the original's order) ───────────────────────────────────────

fn with_number(prefix: Vec<Item>) -> Vec<Item> {
    join(vec![prefix, number(), gib()])
}

fn weights_pattern() -> Vec<Item> {
    let lead = vec![
        once(Atom::Group(vec![
            phrase("model weights take"),
            phrase("weights take"),
        ])),
        spaces(),
    ];
    with_number(lead)
}

fn loading_pattern() -> Vec<Item> {
    join(vec![
        phrase("model loading took"),
        vec![spaces()],
        number(),
        gib(),
        vec![
            repeated(
                Atom::Group(vec![join(vec![vec![spaces()], text("memory")])]),
                0,
                1,
                false,
            ),
            once(Atom::Boundary),
        ],
    ])
}

fn non_torch_pattern() -> Vec<Item> {
    with_number(join(vec![phrase("non_torch_memory takes"), vec![spaces()]]))
}

fn activation_pattern() -> Vec<Item> {
    let lead = vec![repeated(
        Atom::Group(vec![join(vec![text("pytorch"), vec![spaces()]])]),
        0,
        1,
        false,
    )];
    with_number(join(vec![
        lead,
        phrase("activation peak memory takes"),
        vec![spaces()],
    ]))
}

fn kv_reserved_is_pattern() -> Vec<Item> {
    with_number(join(vec![
        phrase("reserved for kv cache is"),
        vec![spaces()],
    ]))
}

fn kv_available_pattern() -> Vec<Item> {
    with_number(join(vec![
        phrase("available kv cache memory:"),
        vec![maybe_spaces()],
    ]))
}

fn kv_clamp_pattern() -> Vec<Item> {
    join(vec![
        text("reserved"),
        vec![spaces()],
        number(),
        gib(),
        vec![spaces()],
        phrase("memory for kv cache"),
    ])
}

fn total_pool_pattern() -> Vec<Item> {
    let lead = join(vec![
        text("gpu_memory_utilization"),
        vec![maybe_spaces(), once(Atom::Literal('('))],
        vec![repeated(
            Atom::Class(vec![ClassItem::Digit, ClassItem::Char('.')]),
            1,
            MANY,
            false,
        )],
        vec![
            once(Atom::Literal(')')),
            maybe_spaces(),
            once(Atom::Literal('=')),
            maybe_spaces(),
        ],
    ]);
    with_number(lead)
}

fn gmu_pattern() -> Vec<Item> {
    let joiner = || {
        once(Atom::Class(vec![
            ClassItem::Char('-'),
            ClassItem::Char('_'),
        ]))
    };
    join(vec![
        text("gpu"),
        vec![joiner()],
        text("memory"),
        vec![joiner()],
        text("utilization"),
        vec![
            maybe_spaces(),
            once(Atom::Class(vec![
                ClassItem::Char('='),
                ClassItem::Char('('),
            ])),
            maybe_spaces(),
            once(Atom::CaptureStart),
            repeated(Atom::Digit, 1, MANY, false),
            repeated(
                Atom::Group(vec![vec![
                    once(Atom::Literal('.')),
                    repeated(Atom::Digit, 1, MANY, false),
                ]]),
                0,
                1,
                false,
            ),
            once(Atom::CaptureEnd),
        ],
    ])
}

fn graph_pattern() -> Vec<Item> {
    with_number(join(vec![
        phrase("cuda graph pool memory:"),
        vec![maybe_spaces()],
    ]))
}

fn tokens_pattern() -> Vec<Item> {
    join(vec![
        phrase("gpu kv cache size:"),
        vec![maybe_spaces()],
        number(),
        vec![maybe_spaces()],
        text("tokens"),
    ])
}

fn concurrency_pattern() -> Vec<Item> {
    join(vec![
        phrase("maximum concurrency for"),
        vec![
            spaces(),
            repeated(
                Atom::Class(vec![ClassItem::Digit, ClassItem::Char(',')]),
                1,
                MANY,
                false,
            ),
            spaces(),
        ],
        phrase("tokens per request:"),
        vec![maybe_spaces()],
        number(),
        text("x"),
    ])
}

fn backend_pattern() -> Vec<Item> {
    join(vec![
        text("using"),
        vec![
            spaces(),
            once(Atom::CaptureStart),
            repeated(Atom::Any, 1, MANY, true),
            once(Atom::CaptureEnd),
            spaces(),
        ],
        text("backend"),
    ])
}

// ── parse_log ────────────────────────────────────────────────────────────────────────────────────

/// `re.search(...)` on a value that must be a string.
fn searchable(value: &PyValue) -> Res<Vec<u32>> {
    match value.peel() {
        PyValue::Str(text) => Ok(text.code_points().to_vec()),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "expected string or bytes-like object, got '{}'",
                other.type_name()
            ),
        )),
    }
}

/// `s.replace(",", "")`.
fn without_commas(grouped: &PyStr) -> PyStr {
    PyStr::from_code_points(
        grouped
            .code_points()
            .iter()
            .copied()
            .filter(|&point| point != u32::from(','))
            .collect(),
    )
}

fn float_of_number(grouped: &PyStr) -> Res<f64> {
    float_from_str(&without_commas(grouped)).map_err(crate::pyexpr::exception)
}

fn search_float(pattern: &[Item], points: &[u32]) -> Res<Option<f64>> {
    search(pattern, points)
        .map(|found| float_of_number(&found))
        .transpose()
}

fn optional_float(value: Option<f64>) -> PyValue {
    value.map_or(PyValue::None, PyValue::Float)
}

/// `parse_log(text)`: the twelve fields and the evidence of what matched.
pub fn parse_log(log: &PyValue) -> Res<PyDict> {
    let points = searchable(log)?;
    let mut weights = search_float(&weights_pattern(), &points)?;
    if weights.is_none() {
        weights = search_float(&loading_pattern(), &points)?;
    }
    let non_torch = search_float(&non_torch_pattern(), &points)?;
    let activation = search_float(&activation_pattern(), &points)?;
    let mut kv_gib = search_float(&kv_reserved_is_pattern(), &points)?;
    if kv_gib.is_none() {
        kv_gib = search_float(&kv_available_pattern(), &points)?;
    }
    if kv_gib.is_none() {
        kv_gib = search_float(&kv_clamp_pattern(), &points)?;
    }
    let total_pool = search_float(&total_pool_pattern(), &points)?;
    let gmu = search_float(&gmu_pattern(), &points)?;
    let cuda_graph = search_float(&graph_pattern(), &points)?;
    let tokens = match search(&tokens_pattern(), &points) {
        Some(found) => {
            Some(int_from_str(&without_commas(&found)).map_err(crate::pyexpr::exception)?)
        }
        None => None,
    };
    let concurrency = search_float(&concurrency_pattern(), &points)?;
    let backend = search(&backend_pattern(), &points).map(|found| found.strip());
    let overhead = match (total_pool, weights, kv_gib) {
        (Some(total), Some(weights), Some(kv)) => {
            let rest = total - weights - kv;
            (rest >= 0.0).then_some(rest)
        }
        _ => None,
    };
    let tokens_value = tokens.map_or(PyValue::None, PyValue::Int);
    let backend_value = backend.map_or(PyValue::None, PyValue::Str);

    let mut raw = PyDict::new();
    let evidence: [(&str, PyValue); 10] = [
        ("weights_gib", optional_float(weights)),
        ("non_torch_gib", optional_float(non_torch)),
        ("torch_activation_peak_gib", optional_float(activation)),
        ("kv_cache_gib", optional_float(kv_gib)),
        ("total_pool_gib", optional_float(total_pool)),
        ("kv_cache_tokens", tokens_value.clone()),
        ("max_concurrency", optional_float(concurrency)),
        ("attention_backend_used", backend_value.clone()),
        ("gmu_trial", optional_float(gmu)),
        ("cuda_graph_gib", optional_float(cuda_graph)),
    ];
    for (name, value) in evidence {
        if !matches!(value, PyValue::None) {
            raw.set(name, value);
        }
    }
    let mut result = PyDict::new();
    result.set("weights_gib", optional_float(weights));
    result.set("kv_cache_gib", optional_float(kv_gib));
    result.set("kv_cache_tokens", tokens_value);
    result.set("max_concurrency", optional_float(concurrency));
    result.set("non_torch_gib", optional_float(non_torch));
    result.set("torch_activation_peak_gib", optional_float(activation));
    result.set("total_pool_gib", optional_float(total_pool));
    result.set("non_kv_overhead_gib", optional_float(overhead));
    result.set("gmu_trial", optional_float(gmu));
    result.set("cuda_graph_gib", optional_float(cuda_graph));
    result.set("attention_backend_used", backend_value);
    result.set("raw_matched", PyValue::Dict(raw));
    Ok(result)
}
