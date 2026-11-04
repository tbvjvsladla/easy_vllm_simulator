//! `yaml/composer.py`, `yaml/resolver.py` (implicit resolvers) and `yaml/constructor.py` (`SafeConstructor`).
//!
//! The composer turns events into a node graph (aliases point at the node already built); the resolver gives plain
//! scalars their YAML 1.1 tag; the constructor turns nodes into values. `SafeConstructor` builds sequences and
//! mappings with generators, which postpones filling a container until the document's outermost call returns:
//! containers are therefore filled level by level, in order, and a conversion error is raised by the first scalar a
//! breadth-first walk meets. [`Builder`] keeps that order (it is observable through *which* error comes first)
//! and assembles the finished values afterwards.

use super::frames::{frame, nested};
use super::parser::{Ev, Event, Parser};
use super::{marked, plain, Failure, Mark, Parsed};
use crate::pynumber::{float_from_str, int_from_str};
use crate::pystr::PyStr;
use crate::pyvalue::{DictKey, NonStringKey, Opaque, PyDict, PyValue};
use num_bigint::BigInt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

const TAG_PREFIX: &str = "tag:yaml.org,2002:";

#[derive(Debug)]
enum NodeKind {
    Scalar(PyStr),
    Sequence(Vec<usize>),
    Mapping(Vec<(usize, usize)>),
}

#[derive(Debug)]
struct Node {
    tag: String,
    kind: NodeKind,
    start: Mark,
}

impl Node {
    fn id(&self) -> &'static str {
        match self.kind {
            NodeKind::Scalar(_) => "scalar",
            NodeKind::Sequence(_) => "sequence",
            NodeKind::Mapping(_) => "mapping",
        }
    }
}

pub(crate) struct Composed {
    nodes: Vec<Node>,
    root: Option<usize>,
}

fn tag(name: &str) -> String {
    format!("{TAG_PREFIX}{name}")
}

fn composer_error(
    context: Option<&str>,
    context_mark: Option<&Mark>,
    problem: String,
    mark: &Mark,
) -> Failure {
    marked("ComposerError", context, context_mark, problem, Some(mark))
}

pub(crate) fn get_single_node(parser: &mut Parser<'_>) -> Parsed<Composed> {
    let _frame = frame("get_single_node")?;
    let mut nodes: Vec<Node> = Vec::new();
    parser.get_event()?;
    let mut root = None;
    if !parser.check_event(|e| matches!(e, Ev::StreamEnd))? {
        root = Some(compose_document(parser, &mut nodes)?);
    }
    if !parser.check_event(|e| matches!(e, Ev::StreamEnd))? {
        let event = parser.get_event()?;
        let document = root.map(|id| nodes[id].start.clone());
        return Err(composer_error(
            Some("expected a single document in the stream"),
            document.as_ref(),
            "but found another document".to_string(),
            &event.start,
        ));
    }
    parser.get_event()?;
    Ok(Composed { nodes, root })
}

fn compose_document(parser: &mut Parser<'_>, nodes: &mut Vec<Node>) -> Parsed<usize> {
    let _frame = frame("compose_document")?;
    parser.get_event()?;
    let mut anchors: HashMap<String, usize> = HashMap::new();
    let node = compose_node(parser, nodes, &mut anchors)?;
    parser.get_event()?;
    Ok(node)
}

fn compose_node(
    parser: &mut Parser<'_>,
    nodes: &mut Vec<Node>,
    anchors: &mut HashMap<String, usize>,
) -> Parsed<usize> {
    let _frame = frame("compose_node")?;
    if parser.check_event(|e| matches!(e, Ev::Alias { .. }))? {
        let event = parser.get_event()?;
        let Ev::Alias { anchor } = event.ev else {
            unreachable!()
        };
        return anchors.get(&anchor).copied().ok_or_else(|| {
            composer_error(
                None,
                None,
                format!("found undefined alias {}", PyStr::new(&anchor).repr()),
                &event.start,
            )
        });
    }
    let event = parser.peek_event()?;
    let anchor = match &event.ev {
        Ev::Scalar { anchor, .. }
        | Ev::SequenceStart { anchor, .. }
        | Ev::MappingStart { anchor, .. } => anchor.clone(),
        _ => None,
    };
    if let Some(name) = &anchor {
        if let Some(&first) = anchors.get(name) {
            return Err(composer_error(
                Some(&format!(
                    "found duplicate anchor {}; first occurrence",
                    PyStr::new(name).repr()
                )),
                Some(&nodes[first].start),
                "second occurrence".to_string(),
                &event.start,
            ));
        }
    }
    nested(&["descend_resolver"])?;
    // `if self.check_event(ScalarEvent) ... elif self.check_event(SequenceStartEvent) ... elif ...`
    let node = match event.ev {
        Ev::Scalar { .. } => {
            nested(&["check_event"])?;
            compose_scalar_node(parser, nodes, anchors, anchor)?
        }
        Ev::SequenceStart { .. } => {
            nested(&["check_event"])?;
            nested(&["check_event"])?;
            compose_collection(parser, nodes, anchors, anchor, true)?
        }
        Ev::MappingStart { .. } => {
            nested(&["check_event"])?;
            nested(&["check_event"])?;
            nested(&["check_event"])?;
            compose_collection(parser, nodes, anchors, anchor, false)?
        }
        _ => {
            for _ in 0..3 {
                nested(&["check_event"])?;
            }
            return Err(plain(
                "UnboundLocalError",
                "cannot access local variable 'node' where it is not associated with a value",
            ));
        }
    };
    nested(&["ascend_resolver"])?;
    Ok(node)
}

fn compose_scalar_node(
    parser: &mut Parser<'_>,
    nodes: &mut Vec<Node>,
    anchors: &mut HashMap<String, usize>,
    anchor: Option<String>,
) -> Parsed<usize> {
    let _frame = frame("compose_scalar_node")?;
    let event = parser.get_event()?;
    let Ev::Scalar {
        tag: explicit,
        implicit,
        value,
        ..
    } = event.ev
    else {
        unreachable!()
    };
    let tag = match explicit.as_deref() {
        None | Some("!") => {
            nested(&["resolve"])?;
            resolve_scalar(&value, implicit)
        }
        Some(other) => other.to_string(),
    };
    nested(&["__init__"])?;
    nodes.push(Node {
        tag,
        kind: NodeKind::Scalar(value),
        start: event.start,
    });
    let id = nodes.len() - 1;
    if let Some(name) = anchor {
        anchors.insert(name, id);
    }
    Ok(id)
}

fn compose_collection(
    parser: &mut Parser<'_>,
    nodes: &mut Vec<Node>,
    anchors: &mut HashMap<String, usize>,
    anchor: Option<String>,
    is_sequence: bool,
) -> Parsed<usize> {
    let _frame = frame(if is_sequence {
        "compose_sequence_node"
    } else {
        "compose_mapping_node"
    })?;
    let start_event: Event = parser.get_event()?;
    let explicit = match &start_event.ev {
        Ev::SequenceStart { tag, .. } | Ev::MappingStart { tag, .. } => tag.clone(),
        _ => None,
    };
    let tag = match explicit.as_deref() {
        None | Some("!") => {
            nested(&["resolve"])?;
            tag(if is_sequence { "seq" } else { "map" })
        }
        Some(other) => other.to_string(),
    };
    nested(&["__init__"])?;
    nodes.push(Node {
        tag,
        kind: if is_sequence {
            NodeKind::Sequence(Vec::new())
        } else {
            NodeKind::Mapping(Vec::new())
        },
        start: start_event.start,
    });
    let id = nodes.len() - 1;
    if let Some(name) = anchor {
        anchors.insert(name, id);
    }
    if is_sequence {
        while !parser.check_event(|e| matches!(e, Ev::SequenceEnd))? {
            let child = compose_node(parser, nodes, anchors)?;
            if let NodeKind::Sequence(items) = &mut nodes[id].kind {
                items.push(child);
            }
        }
    } else {
        while !parser.check_event(|e| matches!(e, Ev::MappingEnd))? {
            let key = compose_node(parser, nodes, anchors)?;
            let value = compose_node(parser, nodes, anchors)?;
            if let NodeKind::Mapping(pairs) = &mut nodes[id].kind {
                pairs.push((key, value));
            }
        }
    }
    parser.get_event()?;
    Ok(id)
}

// ── resolver ────────────────────────────────────────────────────────────────────

fn resolve_scalar(value: &PyStr, implicit: (bool, bool)) -> String {
    if implicit.0 {
        let text: String = value
            .code_points()
            .iter()
            .filter_map(|&p| char::from_u32(p))
            .collect();
        let lossless = text.chars().count() == value.len();
        if lossless {
            if let Some(name) = implicit_tag(&text) {
                return tag(name);
            }
        }
    }
    tag("str")
}

/// The implicit resolvers of `Resolver`, tried in registration order for the scalar's first character.
fn implicit_tag(text: &str) -> Option<&'static str> {
    let first = text.chars().next();
    let candidates: &[&'static str] = match first {
        None => &["null"],
        Some('y' | 'Y' | 'n' | 'N' | 't' | 'T' | 'f' | 'F' | 'o' | 'O') => &["bool", "null"],
        Some('-' | '+') => &["float", "int"],
        Some('0'..='9') => &["float", "int", "timestamp"],
        Some('.') => &["float"],
        Some('<') => &["merge"],
        Some('~') => &["null"],
        Some('=') => &["value"],
        Some('!' | '&' | '*') => &["yaml"],
        _ => &[],
    };
    // Candidates are listed per first character; the order inside one list is the order of registration:
    // bool, float, int, merge, null, timestamp, value, yaml. `n`/`N` are in both bool and null.
    let order = [
        "bool",
        "float",
        "int",
        "merge",
        "null",
        "timestamp",
        "value",
        "yaml",
    ];
    for name in order {
        if !candidates.contains(&name) {
            continue;
        }
        let matched = match name {
            "bool" => is_bool(text),
            "float" => is_float(text),
            "int" => is_int(text),
            "merge" => text == "<<",
            "null" => matches!(text, "~" | "null" | "Null" | "NULL" | ""),
            "timestamp" => is_timestamp(text),
            "value" => text == "=",
            "yaml" => matches!(text, "!" | "&" | "*"),
            _ => false,
        };
        if matched {
            return Some(name);
        }
    }
    None
}

fn is_bool(text: &str) -> bool {
    matches!(
        text,
        "yes"
            | "Yes"
            | "YES"
            | "no"
            | "No"
            | "NO"
            | "true"
            | "True"
            | "TRUE"
            | "false"
            | "False"
            | "FALSE"
            | "on"
            | "On"
            | "ON"
            | "off"
            | "Off"
            | "OFF"
    )
}

fn digits(text: &str, allowed: impl Fn(char) -> bool) -> bool {
    !text.is_empty() && text.chars().all(allowed)
}

fn all_of(text: &str, set: impl Fn(char) -> bool) -> bool {
    text.chars().all(set)
}

/// `(?::[0-5]?[0-9])+` over the whole of `rest`: groups of one or two digits, the first digit at most 5 when two.
fn sexagesimal_tail(rest: &str) -> bool {
    if rest.is_empty() {
        return false;
    }
    rest.split(':').skip(1).all(|part| {
        let bytes = part.as_bytes();
        match bytes.len() {
            1 => bytes[0].is_ascii_digit(),
            2 => (b'0'..=b'5').contains(&bytes[0]) && bytes[1].is_ascii_digit(),
            _ => false,
        }
    }) && rest.starts_with(':')
}

fn is_int(text: &str) -> bool {
    let body = text.strip_prefix(['-', '+']).unwrap_or(text);
    if let Some(rest) = body.strip_prefix("0b") {
        if digits(rest, |c| matches!(c, '0' | '1' | '_')) {
            return true;
        }
    }
    if body.len() > 1
        && body.starts_with('0')
        && digits(&body[1..], |c| matches!(c, '0'..='7' | '_'))
    {
        return true;
    }
    if body == "0"
        || (body.starts_with(|c: char| ('1'..='9').contains(&c))
            && all_of(body, |c| c.is_ascii_digit() || c == '_'))
    {
        return true;
    }
    if let Some(rest) = body.strip_prefix("0x") {
        if digits(rest, |c| c.is_ascii_hexdigit() || c == '_') {
            return true;
        }
    }
    // [1-9][0-9_]*(:[0-5]?[0-9])+
    if let Some(at) = body.find(':') {
        let (head, tail) = body.split_at(at);
        return head.starts_with(|c: char| ('1'..='9').contains(&c))
            && all_of(head, |c| c.is_ascii_digit() || c == '_')
            && sexagesimal_tail(tail);
    }
    false
}

fn exponent_ok(rest: &str) -> bool {
    // (?:[eE][-+][0-9]+)?
    if rest.is_empty() {
        return true;
    }
    let Some(tail) = rest.strip_prefix(['e', 'E']) else {
        return false;
    };
    let Some(number) = tail.strip_prefix(['-', '+']) else {
        return false;
    };
    digits(number, |c| c.is_ascii_digit())
}

fn is_float(text: &str) -> bool {
    let body = text.strip_prefix(['-', '+']).unwrap_or(text);
    let signed = body.len() != text.len();
    // [-+]?\.(inf|Inf|INF)  |  \.(nan|NaN|NAN)
    if matches!(body, ".inf" | ".Inf" | ".INF") {
        return true;
    }
    if !signed && matches!(body, ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    // \.[0-9][0-9_]*(?:[eE][-+][0-9]+)?   (no sign)
    if !signed {
        if let Some(rest) = body.strip_prefix('.') {
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '_'))
                .unwrap_or(rest.len());
            if end > 0
                && rest.starts_with(|c: char| c.is_ascii_digit())
                && exponent_ok(&rest[end..])
            {
                return true;
            }
        }
    }
    // [-+]?[0-9][0-9_]*\.[0-9_]*(?:[eE][-+][0-9]+)?
    if let Some(dot) = body.find('.') {
        let (whole, rest) = body.split_at(dot);
        let rest = &rest[1..];
        if whole.starts_with(|c: char| c.is_ascii_digit())
            && all_of(whole, |c| c.is_ascii_digit() || c == '_')
        {
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '_'))
                .unwrap_or(rest.len());
            if exponent_ok(&rest[end..]) {
                return true;
            }
        }
        // [-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*
        if let Some(colon) = whole.find(':') {
            let (head, tail) = whole.split_at(colon);
            if head.starts_with(|c: char| c.is_ascii_digit())
                && all_of(head, |c| c.is_ascii_digit() || c == '_')
                && sexagesimal_tail(tail)
                && all_of(rest, |c| c.is_ascii_digit() || c == '_')
            {
                return true;
            }
        }
    }
    false
}

/// `^(?:[0-9]{4}-[0-9]{2}-[0-9]{2} | [0-9]{4}-[0-9][0-9]?-[0-9][0-9]? (?:[Tt]|[ \t]+)[0-9][0-9]?:[0-9][0-9]:[0-9][0-9]
/// (?:\.[0-9]*)? (?:[ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?)$`
fn is_timestamp(text: &str) -> bool {
    let bytes: Vec<char> = text.chars().collect();
    parse_timestamp(&bytes, true).is_some()
}

#[derive(Debug, Default, Clone)]
struct Stamp {
    year: String,
    month: String,
    day: String,
    hour: Option<String>,
    minute: String,
    second: String,
    fraction: Option<String>,
    tz: Option<String>,
    tz_sign: Option<char>,
    tz_hour: String,
    tz_minute: Option<String>,
}

fn take_digits(chars: &[char], at: &mut usize, min: usize, max: usize) -> Option<String> {
    let start = *at;
    while *at < chars.len() && chars[*at].is_ascii_digit() && *at - start < max {
        *at += 1;
    }
    (*at - start >= min).then(|| chars[start..*at].iter().collect())
}

/// Matches the resolver's regexp (`resolver == true`: month/day with two digits when no time follows) or the
/// constructor's `timestamp_regexp` (`false`).
fn parse_timestamp(chars: &[char], resolver: bool) -> Option<Stamp> {
    let mut at = 0;
    let mut stamp = Stamp {
        year: take_digits(chars, &mut at, 4, 4)?,
        ..Stamp::default()
    };
    if chars.get(at) != Some(&'-') {
        return None;
    }
    at += 1;
    stamp.month = take_digits(chars, &mut at, 1, 2)?;
    if chars.get(at) != Some(&'-') {
        return None;
    }
    at += 1;
    stamp.day = take_digits(chars, &mut at, 1, 2)?;
    if at == chars.len() {
        // date only: the resolver's first alternative needs exactly two-digit month and day
        return (!resolver || (stamp.month.len() == 2 && stamp.day.len() == 2)).then_some(stamp);
    }
    // time part
    if matches!(chars.get(at), Some('T' | 't')) {
        at += 1;
    } else if matches!(chars.get(at), Some(' ' | '\t')) {
        while matches!(chars.get(at), Some(' ' | '\t')) {
            at += 1;
        }
    } else {
        return None;
    }
    stamp.hour = Some(take_digits(chars, &mut at, 1, 2)?);
    if chars.get(at) != Some(&':') {
        return None;
    }
    at += 1;
    stamp.minute = take_digits(chars, &mut at, 2, 2)?;
    if chars.get(at) != Some(&':') {
        return None;
    }
    at += 1;
    stamp.second = take_digits(chars, &mut at, 2, 2)?;
    if chars.get(at) == Some(&'.') {
        at += 1;
        let start = at;
        while at < chars.len() && chars[at].is_ascii_digit() {
            at += 1;
        }
        stamp.fraction = Some(chars[start..at].iter().collect());
    }
    if at < chars.len() {
        let saved = at;
        while matches!(chars.get(at), Some(' ' | '\t')) {
            at += 1;
        }
        match chars.get(at) {
            Some('Z') => {
                stamp.tz = Some("Z".to_string());
                at += 1;
            }
            Some(&sign) if sign == '-' || sign == '+' => {
                at += 1;
                stamp.tz_sign = Some(sign);
                stamp.tz_hour = take_digits(chars, &mut at, 1, 2)?;
                if chars.get(at) == Some(&':') {
                    at += 1;
                    stamp.tz_minute = Some(take_digits(chars, &mut at, 2, 2)?);
                }
                stamp.tz = Some(
                    chars[saved..at]
                        .iter()
                        .collect::<String>()
                        .trim_start_matches([' ', '\t'])
                        .to_string(),
                );
            }
            _ => return None,
        }
    }
    (at == chars.len()).then_some(stamp)
}

// ── constructor ─────────────────────────────────────────────────────────────────

/// What `construct_object` returned for a node, before containers are filled.
#[derive(Debug, Clone)]
enum Built {
    Value(PyValue),
    List(Vec<usize>),
    /// mapping, with the node pairs resolved to built-node ids (keys, values)
    Dict(Vec<(usize, usize)>),
    /// `!!set`: the keys of a mapping
    Set(Vec<usize>),
    /// `!!omap` / `!!pairs`: a list of 2-item lists
    Pairs(Vec<(usize, usize)>),
}

struct Builder {
    nodes: Vec<Node>,
    built: HashMap<usize, Built>,
    queue: VecDeque<usize>,
    under_construction: HashSet<usize>,
    /// State of [`Builder::assemble`], set up once the containers are filled.
    shape: Shape,
}

/// How the built nodes refer to each other: what [`Builder::assemble`] needs to share an object like Python does.
#[derive(Default)]
struct Shape {
    /// how many places refer to a node (the root counts once)
    refs: Vec<u32>,
    /// strongly connected component of a node
    component: Vec<usize>,
    /// whether a component contains a cycle (more than one node, or a node that refers to itself)
    cyclic: Vec<bool>,
    /// the nodes being assembled, outermost first, and the same as a set
    path: std::cell::RefCell<Vec<usize>>,
    open: std::cell::RefCell<HashSet<usize>>,
    /// what the nodes being assembled have met so far, one entry per open node
    frames: std::cell::RefCell<Vec<Met>>,
    /// assembled shared nodes
    memo: std::cell::RefCell<HashMap<usize, Vec<Memo>>>,
}

/// Which nodes of a cycle a value was built around. A node's value depends only on which of the nodes it can reach are
/// open (being assembled) around it: an open node is a marker, any other is expanded.
#[derive(Default, Clone)]
struct Met {
    /// open nodes the value holds a marker for
    markers: HashSet<usize>,
    /// nodes of a cycle the value expands
    expanded: HashSet<usize>,
}

/// A shared node's value and the circumstances it was built in; it is the value of the node wherever the same open
/// nodes are markers and the same expanded nodes are not open.
struct Memo {
    value: PyValue,
    met: Met,
}

fn constructor_error(
    context: Option<&str>,
    context_mark: Option<&Mark>,
    problem: String,
    mark: Option<&Mark>,
) -> Failure {
    marked("ConstructorError", context, context_mark, problem, mark)
}

pub(crate) fn construct_document(composed: Composed) -> Parsed<PyValue> {
    let _frame = frame("construct_document")?;
    let Composed { nodes, root } = composed;
    let Some(root) = root else {
        return Ok(PyValue::None);
    };
    let mut builder = Builder {
        nodes,
        built: HashMap::new(),
        queue: VecDeque::new(),
        under_construction: HashSet::new(),
        shape: Shape::default(),
    };
    builder.construct_object(root)?;
    while let Some(id) = builder.queue.pop_front() {
        builder.fill(id)?;
    }
    builder.analyse(root);
    Ok(builder.assemble(root))
}

impl Builder {
    fn node_tag(&self, id: usize) -> String {
        self.nodes[id].tag.clone()
    }

    /// `construct_scalar`: a mapping with a `=` (value) key yields that key's value node's scalar.
    fn scalar_text(&self, id: usize) -> Parsed<PyStr> {
        match &self.nodes[id].kind {
            NodeKind::Mapping(pairs) => {
                for (key, value) in pairs.clone() {
                    if self.nodes[key].tag == tag("value") {
                        return self.scalar_text(value);
                    }
                }
                Err(constructor_error(
                    None,
                    None,
                    format!("expected a scalar node, but found {}", self.nodes[id].id()),
                    Some(&self.nodes[id].start),
                ))
            }
            NodeKind::Scalar(text) => Ok(text.clone()),
            _ => Err(constructor_error(
                None,
                None,
                format!("expected a scalar node, but found {}", self.nodes[id].id()),
                Some(&self.nodes[id].start),
            )),
        }
    }

    fn construct_object(&mut self, id: usize) -> Parsed<()> {
        if self.built.contains_key(&id) {
            return Ok(());
        }
        let node_tag = self.node_tag(id);
        let name = node_tag.strip_prefix(TAG_PREFIX);
        let built = match name {
            Some("null") => {
                self.scalar_text(id)?;
                Built::Value(PyValue::None)
            }
            Some("bool") => {
                let value = self.scalar_text(id)?;
                Built::Value(self.bool_value(&value)?)
            }
            Some("int") => Built::Value(self.int_value(id)?),
            Some("float") => Built::Value(self.float_value(id)?),
            Some("binary") => Built::Value(self.binary_value(id)?),
            Some("timestamp") => Built::Value(self.timestamp_value(id)?),
            Some("str") => Built::Value(PyValue::Str(self.scalar_text(id)?)),
            Some("seq") => {
                self.expect_sequence(id)?;
                self.under_construction.insert(id);
                self.queue.push_back(id);
                Built::List(Vec::new())
            }
            Some("map") | Some("set") | Some("omap") | Some("pairs") => {
                // the generator constructors only start filling later; the kind of container is known now
                self.queue.push_back(id);
                match name {
                    Some("map") => Built::Dict(Vec::new()),
                    Some("set") => Built::Set(Vec::new()),
                    _ => Built::Pairs(Vec::new()),
                }
            }
            _ => {
                return Err(constructor_error(
                    None,
                    None,
                    format!(
                        "could not determine a constructor for the tag {}",
                        PyStr::new(&node_tag).repr()
                    ),
                    Some(&self.nodes[id].start),
                ))
            }
        };
        self.built.insert(id, built);
        Ok(())
    }

    fn expect_sequence(&self, id: usize) -> Parsed<()> {
        if matches!(self.nodes[id].kind, NodeKind::Sequence(_)) {
            Ok(())
        } else {
            Err(constructor_error(
                None,
                None,
                format!(
                    "expected a sequence node, but found {}",
                    self.nodes[id].id()
                ),
                Some(&self.nodes[id].start),
            ))
        }
    }

    /// Runs the postponed part of a container's constructor.
    fn fill(&mut self, id: usize) -> Parsed<()> {
        let tag_name = self.node_tag(id);
        match tag_name.strip_prefix(TAG_PREFIX) {
            Some("seq") => {
                let NodeKind::Sequence(children) = &self.nodes[id].kind else {
                    unreachable!()
                };
                let children = children.clone();
                for &child in &children {
                    self.construct_object(child)?;
                }
                self.built.insert(id, Built::List(children));
                self.under_construction.remove(&id);
            }
            Some("map") => {
                let _generator = frame("construct_yaml_map")?;
                let pairs = self.mapping_pairs(id)?;
                self.built.insert(id, Built::Dict(pairs));
            }
            Some("set") => {
                let _generator = frame("construct_yaml_set")?;
                let pairs = self.mapping_pairs(id)?;
                self.built.insert(
                    id,
                    Built::Set(pairs.into_iter().map(|(key, _)| key).collect()),
                );
            }
            Some("omap") | Some("pairs") => {
                let (what, expect_list) = if tag_name.ends_with("omap") {
                    ("an ordered map", "an ordered map")
                } else {
                    ("pairs", "pairs")
                };
                let _ = expect_list;
                let start = self.nodes[id].start.clone();
                let context = format!("while constructing {what}");
                let NodeKind::Sequence(items) = &self.nodes[id].kind else {
                    return Err(constructor_error(
                        Some(&context),
                        Some(&start),
                        format!("expected a sequence, but found {}", self.nodes[id].id()),
                        Some(&start),
                    ));
                };
                let items = items.clone();
                let mut pairs = Vec::new();
                for sub in items {
                    let NodeKind::Mapping(entries) = &self.nodes[sub].kind else {
                        return Err(constructor_error(
                            Some(&context),
                            Some(&start),
                            format!(
                                "expected a mapping of length 1, but found {}",
                                self.nodes[sub].id()
                            ),
                            Some(&self.nodes[sub].start),
                        ));
                    };
                    if entries.len() != 1 {
                        return Err(constructor_error(
                            Some(&context),
                            Some(&start),
                            format!(
                                "expected a single mapping item, but found {} items",
                                entries.len()
                            ),
                            Some(&self.nodes[sub].start),
                        ));
                    }
                    let (key, value) = entries[0];
                    self.construct_object(key)?;
                    self.construct_object(value)?;
                    pairs.push((key, value));
                }
                self.built.insert(id, Built::Pairs(pairs));
            }
            _ => {}
        }
        Ok(())
    }

    /// `construct_mapping`: flatten merge keys, then construct every key and value in order.
    fn mapping_pairs(&mut self, id: usize) -> Parsed<Vec<(usize, usize)>> {
        let _frame = frame("construct_mapping")?;
        if !matches!(self.nodes[id].kind, NodeKind::Mapping(_)) {
            return Err(constructor_error(
                None,
                None,
                format!("expected a mapping node, but found {}", self.nodes[id].id()),
                Some(&self.nodes[id].start),
            ));
        }
        self.flatten_mapping(id)?;
        let NodeKind::Mapping(pairs) = &self.nodes[id].kind else {
            unreachable!()
        };
        let pairs = pairs.clone();
        for &(key, value) in &pairs {
            self.construct_object(key)?;
            if self.is_unhashable(key) {
                return Err(constructor_error(
                    Some("while constructing a mapping"),
                    Some(&self.nodes[id].start),
                    "found unhashable key".to_string(),
                    Some(&self.nodes[key].start),
                ));
            }
            self.construct_object(value)?;
        }
        Ok(pairs)
    }

    fn is_unhashable(&self, key: usize) -> bool {
        matches!(
            self.built.get(&key),
            Some(Built::List(_) | Built::Dict(_) | Built::Set(_) | Built::Pairs(_))
        )
    }

    fn flatten_mapping(&mut self, id: usize) -> Parsed<()> {
        let _frame = frame("flatten_mapping")?;
        let mut merge: Vec<(usize, usize)> = Vec::new();
        let mut index = 0;
        loop {
            let (key_node, value_node) = {
                let NodeKind::Mapping(pairs) = &self.nodes[id].kind else {
                    unreachable!()
                };
                if index >= pairs.len() {
                    break;
                }
                pairs[index]
            };
            if self.nodes[key_node].tag == tag("merge") {
                if let NodeKind::Mapping(pairs) = &mut self.nodes[id].kind {
                    pairs.remove(index);
                }
                match &self.nodes[value_node].kind {
                    NodeKind::Mapping(_) => {
                        self.flatten_mapping(value_node)?;
                        let NodeKind::Mapping(items) = &self.nodes[value_node].kind else {
                            unreachable!()
                        };
                        merge.extend(items.iter().copied());
                    }
                    NodeKind::Sequence(subnodes) => {
                        let subnodes = subnodes.clone();
                        let mut submerge: Vec<Vec<(usize, usize)>> = Vec::new();
                        for sub in subnodes {
                            if !matches!(self.nodes[sub].kind, NodeKind::Mapping(_)) {
                                return Err(constructor_error(
                                    Some("while constructing a mapping"),
                                    Some(&self.nodes[id].start),
                                    format!(
                                        "expected a mapping for merging, but found {}",
                                        self.nodes[sub].id()
                                    ),
                                    Some(&self.nodes[sub].start),
                                ));
                            }
                            self.flatten_mapping(sub)?;
                            let NodeKind::Mapping(items) = &self.nodes[sub].kind else {
                                unreachable!()
                            };
                            submerge.push(items.clone());
                        }
                        submerge.reverse();
                        for items in submerge {
                            merge.extend(items);
                        }
                    }
                    NodeKind::Scalar(_) => {
                        return Err(constructor_error(
                            Some("while constructing a mapping"),
                            Some(&self.nodes[id].start),
                            format!(
                                "expected a mapping or list of mappings for merging, but found {}",
                                self.nodes[value_node].id()
                            ),
                            Some(&self.nodes[value_node].start),
                        ));
                    }
                }
            } else {
                if self.nodes[key_node].tag == tag("value") {
                    self.nodes[key_node].tag = tag("str");
                }
                index += 1;
            }
        }
        if !merge.is_empty() {
            if let NodeKind::Mapping(pairs) = &mut self.nodes[id].kind {
                let mut combined = merge;
                combined.append(pairs);
                *pairs = combined;
            }
        }
        Ok(())
    }

    fn bool_value(&self, value: &PyStr) -> Parsed<PyValue> {
        let lowered = value.to_utf8().unwrap_or_default().to_lowercase();
        match lowered.as_str() {
            "yes" | "true" | "on" => Ok(PyValue::Bool(true)),
            "no" | "false" | "off" => Ok(PyValue::Bool(false)),
            _ => Err(plain("KeyError", PyStr::new(&lowered).repr())),
        }
    }

    fn int_value(&self, id: usize) -> Parsed<PyValue> {
        let value = self.scalar_text(id)?;
        let cleaned: String = value
            .code_points()
            .iter()
            .filter(|&&p| p != u32::from('_'))
            .filter_map(|&p| char::from_u32(p))
            .collect();
        if cleaned.is_empty() {
            return Err(plain("IndexError", "string index out of range"));
        }
        let (sign, body) = match cleaned.strip_prefix('-') {
            Some(rest) => (-1i32, rest.to_string()),
            None => (1, cleaned.strip_prefix('+').unwrap_or(&cleaned).to_string()),
        };
        let parse = |digits: &str, radix: u32| -> Parsed<BigInt> {
            BigInt::parse_bytes(digits.as_bytes(), radix).ok_or_else(|| {
                plain(
                    "ValueError",
                    format!(
                        "invalid literal for int() with base {radix}: {}",
                        PyStr::new(digits).repr()
                    ),
                )
            })
        };
        let magnitude = if body == "0" {
            BigInt::from(0)
        } else if let Some(rest) = body.strip_prefix("0b") {
            parse(rest, 2)?
        } else if let Some(rest) = body.strip_prefix("0x") {
            parse(rest, 16)?
        } else if body.starts_with('0') {
            parse(&body, 8)?
        } else if body.contains(':') {
            let mut total = BigInt::from(0);
            let mut base = BigInt::from(1);
            for part in body.split(':').rev() {
                let digit =
                    int_from_str(&PyStr::new(part)).map_err(|e| plain("ValueError", e.message))?;
                total += digit * &base;
                base *= 60;
            }
            total
        } else {
            int_from_str(&PyStr::new(&body)).map_err(|e| plain("ValueError", e.message))?
        };
        Ok(PyValue::Int(if sign < 0 { -magnitude } else { magnitude }))
    }

    fn float_value(&self, id: usize) -> Parsed<PyValue> {
        let value = self.scalar_text(id)?;
        let cleaned: String = value
            .code_points()
            .iter()
            .filter(|&&p| p != u32::from('_'))
            .filter_map(|&p| char::from_u32(p))
            .collect::<String>()
            .to_lowercase();
        if cleaned.is_empty() {
            return Err(plain("IndexError", "string index out of range"));
        }
        let (sign, body) = match cleaned.strip_prefix('-') {
            Some(rest) => (-1.0f64, rest.to_string()),
            None => (
                1.0,
                cleaned.strip_prefix('+').unwrap_or(&cleaned).to_string(),
            ),
        };
        let convert = |text: &str| {
            float_from_str(&PyStr::new(text)).map_err(|e| plain("ValueError", e.message))
        };
        if body == ".inf" {
            return Ok(PyValue::Float(sign * f64::INFINITY));
        }
        if body == ".nan" {
            return Ok(PyValue::Float(f64::NAN));
        }
        if body.contains(':') {
            let mut total = 0.0f64;
            let mut base = 1.0f64;
            for part in body.split(':').rev() {
                total += convert(part)? * base;
                base *= 60.0;
            }
            return Ok(PyValue::Float(sign * total));
        }
        Ok(PyValue::Float(sign * convert(&body)?))
    }

    /// Which Python object a NaN key is. `.nan` (with any sign or underscores) is `SafeConstructor.nan_value`, one object
    /// for every such scalar; any other NaN (`!!float nan`, `-nan`, a sexagesimal with a NaN part) is built fresh for
    /// its node, and an alias reuses the node's object. Python's dict keeps one entry per object, because `nan != nan`
    /// and only identity makes a lookup succeed. `0` is the shared object, `node + 1` the one of a node.
    fn nan_origin(&self, id: usize) -> usize {
        let NodeKind::Scalar(text) = &self.nodes[id].kind else {
            return id + 1;
        };
        let cleaned: String = text
            .code_points()
            .iter()
            .filter(|&&p| p != u32::from('_'))
            .filter_map(|&p| char::from_u32(p))
            .collect::<String>()
            .to_lowercase();
        let body = cleaned.strip_prefix(['+', '-']).unwrap_or(&cleaned);
        if body == ".nan" {
            0
        } else {
            id + 1
        }
    }

    fn binary_value(&self, id: usize) -> Parsed<PyValue> {
        let value = self.scalar_text(id)?;
        let points = value.code_points();
        if let Some(start) = points.iter().position(|&p| p > 127) {
            // `str.encode('ascii')` fails on the first run of non-ASCII characters; the message names a single
            // character with its escape, or the position range of the run
            let end = points[start..]
                .iter()
                .position(|&p| p <= 127)
                .map_or(points.len(), |n| start + n);
            let detail = if end - start == 1 {
                format!(
                    "character '{}' in position {start}",
                    unicode_escape(points[start])
                )
            } else {
                format!("characters in position {start}-{}", end - 1)
            };
            return Err(constructor_error(
                None,
                None,
                format!(
                    "failed to convert base64 data into ascii: 'ascii' codec can't encode {detail}: ordinal not in range(128)"
                ),
                Some(&self.nodes[id].start),
            ));
        }
        let text: Vec<u8> = points.iter().map(|&p| p as u8).collect();
        match decode_base64(&text) {
            Ok(bytes) => Ok(PyValue::Bytes(bytes)),
            Err(message) => Err(constructor_error(
                None,
                None,
                format!("failed to decode base64 data: {message}"),
                Some(&self.nodes[id].start),
            )),
        }
    }

    fn timestamp_value(&self, id: usize) -> Parsed<PyValue> {
        self.scalar_text(id)?;
        let NodeKind::Scalar(text) = &self.nodes[id].kind else {
            return Err(plain(
                "AttributeError",
                "'NoneType' object has no attribute 'groupdict'",
            ));
        };
        let chars: Vec<char> = text
            .code_points()
            .iter()
            .filter_map(|&p| char::from_u32(p))
            .collect();
        let Some(stamp) = parse_timestamp(&chars, false) else {
            return Err(plain(
                "AttributeError",
                "'NoneType' object has no attribute 'groupdict'",
            ));
        };
        let int = |s: &str| s.parse::<i64>().unwrap_or(0);
        let (year, month, day) = (int(&stamp.year), int(&stamp.month), int(&stamp.day));
        let valid_date = |year: i64, month: i64, day: i64| -> Result<(), Failure> {
            if !(1..=9999).contains(&year) {
                return Err(plain("ValueError", format!("year {year} is out of range")));
            }
            if !(1..=12).contains(&month) {
                return Err(plain("ValueError", "month must be in 1..12"));
            }
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            let days = [
                31,
                if leap { 29 } else { 28 },
                31,
                30,
                31,
                30,
                31,
                31,
                30,
                31,
                30,
                31,
            ][(month - 1) as usize];
            if !(1..=days).contains(&day) {
                return Err(plain("ValueError", "day is out of range for month"));
            }
            Ok(())
        };
        let Some(hour) = stamp.hour.as_deref().filter(|h| !h.is_empty()) else {
            valid_date(year, month, day)?;
            return Ok(PyValue::Opaque(Opaque {
                type_name: "datetime.date",
                repr: format!("datetime.date({year}, {month}, {day})"),
                truthy: true,
            }));
        };
        let (hour, minute, second) = (int(hour), int(&stamp.minute), int(&stamp.second));
        let mut fraction = String::new();
        if let Some(f) = stamp.fraction.as_deref().filter(|f| !f.is_empty()) {
            fraction = f.chars().take(6).collect();
            while fraction.len() < 6 {
                fraction.push('0');
            }
        }
        let micro = if fraction.is_empty() {
            0
        } else {
            int(&fraction)
        };
        // tzinfo: the constructor builds the offset before datetime() validates the fields
        let mut tz_repr = String::new();
        if let Some(sign) = stamp.tz_sign {
            let tz_hour = int(&stamp.tz_hour);
            let tz_minute = stamp.tz_minute.as_deref().map_or(0, int);
            let mut seconds = tz_hour * 3600 + tz_minute * 60;
            if sign == '-' {
                seconds = -seconds;
            }
            let (days, rest) = (seconds.div_euclid(86400), seconds.rem_euclid(86400));
            if days != 0 && !(days == -1 && rest == 0) || seconds.abs() >= 86400 {
                let mut parts = Vec::new();
                if days != 0 {
                    parts.push(format!("days={days}"));
                }
                if rest != 0 {
                    parts.push(format!("seconds={rest}"));
                }
                return Err(plain(
                    "ValueError",
                    format!(
                        "offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not datetime.timedelta({}).",
                        parts.join(", ")
                    ),
                ));
            }
            tz_repr = format!(
                ", tzinfo=datetime.timezone(datetime.timedelta({}))",
                timedelta_args(seconds)
            );
        } else if stamp.tz.is_some() {
            tz_repr = ", tzinfo=datetime.timezone.utc".to_string();
        }
        valid_date(year, month, day)?;
        if !(0..=23).contains(&hour) {
            return Err(plain("ValueError", "hour must be in 0..23"));
        }
        if !(0..=59).contains(&minute) {
            return Err(plain("ValueError", "minute must be in 0..59"));
        }
        if !(0..=59).contains(&second) {
            return Err(plain("ValueError", "second must be in 0..59"));
        }
        let micro_part = if micro != 0 {
            format!(", {micro}")
        } else {
            String::new()
        };
        let second_part = if micro != 0 || second != 0 {
            format!(", {second}")
        } else {
            String::new()
        };
        let minute_part = if micro != 0 || second != 0 || minute != 0 {
            format!(", {minute}")
        } else {
            String::new()
        };
        let _ = (&micro_part, &second_part, &minute_part);
        Ok(PyValue::Opaque(Opaque {
            type_name: "datetime.datetime",
            repr: format!(
                "datetime.datetime({year}, {month}, {day}, {hour}, {minute}, {second}{}{tz_repr})",
                if micro != 0 {
                    format!(", {micro}")
                } else {
                    String::new()
                }
            ),
            truthy: true,
        }))
    }

    /// Edges of the built graph: the nodes a built container holds.
    fn children(built: &Built) -> Vec<usize> {
        match built {
            Built::Value(_) => Vec::new(),
            Built::List(children) | Built::Set(children) => children.clone(),
            Built::Dict(pairs) | Built::Pairs(pairs) => {
                pairs.iter().flat_map(|&(k, v)| [k, v]).collect()
            }
        }
    }

    /// Counts the references to each node and finds the cycles (Tarjan, without recursion).
    fn analyse(&mut self, root: usize) {
        let count = self.nodes.len();
        let mut edges: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut refs = vec![0u32; count];
        refs[root] += 1;
        for (&id, built) in &self.built {
            edges[id] = Self::children(built);
            for &child in &edges[id] {
                refs[child] += 1;
            }
        }
        let mut index = vec![usize::MAX; count];
        let mut low = vec![0usize; count];
        let mut on_stack = vec![false; count];
        let mut component = vec![usize::MAX; count];
        let mut cyclic: Vec<bool> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut next = 0usize;
        for start in 0..count {
            if index[start] != usize::MAX || !self.built.contains_key(&start) {
                continue;
            }
            let mut work: Vec<(usize, usize)> = vec![(start, 0)];
            index[start] = next;
            low[start] = next;
            next += 1;
            stack.push(start);
            on_stack[start] = true;
            while let Some(&mut (node, ref mut at)) = work.last_mut() {
                if *at < edges[node].len() {
                    let child = edges[node][*at];
                    *at += 1;
                    if index[child] == usize::MAX {
                        index[child] = next;
                        low[child] = next;
                        next += 1;
                        stack.push(child);
                        on_stack[child] = true;
                        work.push((child, 0));
                    } else if on_stack[child] {
                        low[node] = low[node].min(index[child]);
                    }
                } else {
                    work.pop();
                    if let Some(&(parent, _)) = work.last() {
                        low[parent] = low[parent].min(low[node]);
                    }
                    if low[node] == index[node] {
                        let id = cyclic.len();
                        let mut members = 0;
                        loop {
                            let member = stack.pop().expect("component member");
                            on_stack[member] = false;
                            component[member] = id;
                            members += 1;
                            if member == node {
                                break;
                            }
                        }
                        cyclic.push(members > 1 || edges[node].contains(&node));
                    }
                }
            }
        }
        self.shape.refs = refs;
        self.shape.component = component;
        self.shape.cyclic = cyclic;
    }

    /// The Python object a node stands for. An object that several places refer to is built once and shared
    /// ([`PyValue::Shared`]), as in Python; an object that contains itself gets a marker where it recurs (it prints
    /// as `[...]` / `{...}` and refuses JSON like `json.dumps` does). What a cyclic node's value is depends on which
    /// nodes of its cycle are open around it, so a remembered value is reused only where that is the same.
    fn assemble(&self, id: usize) -> PyValue {
        let built = self
            .built
            .get(&id)
            .expect("every reachable node was constructed");
        if let Built::Value(value) = built {
            return value.clone();
        }
        let shape = &self.shape;
        if shape.open.borrow().contains(&id) {
            if let Some(top) = shape.frames.borrow_mut().last_mut() {
                top.markers.insert(id);
            }
            let (type_name, repr) = match built {
                Built::Dict(_) => ("dict", "{...}"),
                Built::Set(_) => ("set", "set(...)"),
                _ => ("list", "[...]"),
            };
            return PyValue::Opaque(Opaque {
                type_name,
                repr: repr.to_string(),
                truthy: true,
            });
        }
        let shared = shape.refs[id] >= 2;
        let cyclic = shape.cyclic[shape.component[id]];
        if shared {
            if let Some((value, met)) = self.remembered(id) {
                if let Some(top) = shape.frames.borrow_mut().last_mut() {
                    top.markers.extend(met.markers);
                    top.expanded.extend(met.expanded);
                }
                return value;
            }
        }
        shape.path.borrow_mut().push(id);
        shape.open.borrow_mut().insert(id);
        shape.frames.borrow_mut().push(Met::default());
        let value = match built {
            Built::Value(_) => unreachable!(),
            Built::List(children) => {
                PyValue::List(children.iter().map(|&child| self.assemble(child)).collect())
            }
            Built::Pairs(pairs) => PyValue::List(
                pairs
                    .iter()
                    .map(|&(key, value)| {
                        PyValue::Tuple(vec![self.assemble(key), self.assemble(value)])
                    })
                    .collect(),
            ),
            Built::Set(keys) => {
                let mut reprs: Vec<String> = Vec::new();
                for &key in keys {
                    let repr = self.assemble(key).repr();
                    if !reprs.contains(&repr) {
                        reprs.push(repr);
                    }
                }
                PyValue::Opaque(Opaque {
                    type_name: "set",
                    repr: if reprs.is_empty() {
                        "set()".to_string()
                    } else {
                        format!("{{{}}}", reprs.join(", "))
                    },
                    truthy: !reprs.is_empty(),
                })
            }
            Built::Dict(pairs) => {
                let mut map = PyDict::new();
                for &(key, value) in pairs {
                    let key_value = self.assemble(key);
                    let value = self.assemble(value);
                    map.insert_key(dict_key(&key_value, self.nan_origin(key)), value);
                }
                PyValue::Dict(map)
            }
        };
        shape.path.borrow_mut().pop();
        shape.open.borrow_mut().remove(&id);
        let mut met = shape.frames.borrow_mut().pop().expect("frame of this node");
        // a marker for the node itself is inside the value, not a circumstance of it
        met.markers.remove(&id);
        if cyclic {
            met.expanded.insert(id);
        }
        if let Some(top) = shape.frames.borrow_mut().last_mut() {
            top.markers.extend(met.markers.iter().copied());
            top.expanded.extend(met.expanded.iter().copied());
        }
        if shared {
            let value = PyValue::Shared(Arc::new(value));
            shape.memo.borrow_mut().entry(id).or_default().push(Memo {
                value: value.clone(),
                met,
            });
            value
        } else {
            value
        }
    }

    /// The value remembered for a shared node, if it was built around the same open nodes as the present ones.
    fn remembered(&self, id: usize) -> Option<(PyValue, Met)> {
        let shape = &self.shape;
        let memo = shape.memo.borrow();
        let open = shape.open.borrow();
        memo.get(&id)?
            .iter()
            .find(|entry| {
                entry.met.markers.iter().all(|node| open.contains(node))
                    && !shape
                        .path
                        .borrow()
                        .iter()
                        .any(|node| entry.met.expanded.contains(node))
            })
            .map(|entry| (entry.value.clone(), entry.met.clone()))
    }
}

fn timedelta_args(seconds: i64) -> String {
    let (days, rest) = (seconds.div_euclid(86400), seconds.rem_euclid(86400));
    let mut parts = Vec::new();
    if days != 0 {
        parts.push(format!("days={days}"));
    }
    if rest != 0 {
        parts.push(format!("seconds={rest}"));
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(", ")
    }
}

/// A dict key made of a YAML value. A string is a string key; every other hashable value is a non-string key whose
/// identity is equal exactly when Python's keys are: `1`, `1.0` and `True` are one key, whatever the size of the integral
/// float (exact integer value); a NaN is equal only to the same object (`nan_origin`: see [`Builder::nan_origin`]).
fn dict_key(value: &PyValue, nan_origin: usize) -> DictKey {
    let other = |identity: String| {
        DictKey::Other(NonStringKey {
            identity,
            repr: value.repr(),
        })
    };
    match value {
        PyValue::Str(text) => DictKey::Str(text.clone()),
        PyValue::None => other("none".to_string()),
        PyValue::Bool(flag) => other(format!("num:{}", u8::from(*flag))),
        PyValue::Int(number) => other(format!("num:{number}")),
        PyValue::Float(number) if number.is_nan() => other(format!("nan:{nan_origin}")),
        PyValue::Float(number) if number.is_finite() && number.fract() == 0.0 => {
            let exact = crate::pynumber::int_from_float(*number)
                .expect("a finite integral float converts to an integer exactly");
            other(format!("num:{exact}"))
        }
        PyValue::Float(number) => other(format!("float:{}", crate::pyvalue::float_repr(*number))),
        value => other(format!("other:{}", value.repr())),
    }
}

/// How `repr` spells one character of a `UnicodeEncodeError` message (`\\xNN`, `\\uNNNN` or `\\UNNNNNNNN`, lower case).
fn unicode_escape(point: u32) -> String {
    if point < 0x100 {
        format!("\\x{point:02x}")
    } else if point < 0x10000 {
        format!("\\u{point:04x}")
    } else {
        format!("\\U{point:08x}")
    }
}

/// CPython 3.12.3 `binascii.a2b_base64(data)` in its non-strict mode, which is what `base64.decodebytes` calls
/// (`Modules/binascii.c`, `binascii_a2b_base64_impl`): characters outside the alphabet are skipped, `=` counts toward
/// the closing pad only when it follows at least two data characters of a group, and a group of exactly one
/// character is an error whose message counts the characters decoded so far.
fn decode_base64(input: &[u8]) -> Result<Vec<u8>, String> {
    fn digit(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out: Vec<u8> = Vec::new();
    let mut quad_pos = 0u8;
    let mut left = 0u8;
    let mut pads = 0u8;
    for &byte in input {
        if byte == b'=' {
            if quad_pos >= 2 {
                pads += 1;
                if quad_pos + pads >= 4 {
                    return Ok(out);
                }
            }
            continue;
        }
        let Some(value) = digit(byte) else { continue };
        pads = 0;
        match quad_pos {
            0 => {
                quad_pos = 1;
                left = value;
            }
            1 => {
                quad_pos = 2;
                out.push((left << 2) | (value >> 4));
                left = value & 0x0f;
            }
            2 => {
                quad_pos = 3;
                out.push((left << 4) | (value >> 2));
                left = value & 0x03;
            }
            _ => {
                quad_pos = 0;
                out.push((left << 6) | value);
                left = 0;
            }
        }
    }
    match quad_pos {
        0 => Ok(out),
        1 => Err(format!(
            "Invalid base64-encoded string: number of data characters ({}) cannot be 1 more than a multiple of 4",
            out.len() / 3 * 4 + 1
        )),
        _ => Err("Incorrect padding".to_string()),
    }
}
