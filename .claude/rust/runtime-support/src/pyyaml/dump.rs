//! `yaml.safe_dump(data, stream, allow_unicode=True, sort_keys=False)` of PyYAML 6.0.3, ported to Rust: the text it writes for a
//! JSON-valued document.
//!
//! The pure-Python pipeline `SafeDumper` = Representer → Serializer → Emitter → Resolver. The values this port takes are the ones a JSON
//! document holds (`None`, `bool`, `int`, `float`, `str`, `list`, `dict`; a tuple is a list as in `SafeRepresenter`), with the defaults of
//! `dump_all`: block style (`default_flow_style=False`), `indent` 2, `width` 80, `line_break` LF, no explicit start or end, no
//! version or tags, no `canonical`, `encoding=None` (text). Derived from PyYAML 6.0.3 (`yaml/{representer,serializer,emitter,
//! resolver,nodes,events}.py`), MIT License, Copyright (c) 2017-2021 Ingy döt Net and (c) 2006-2016 Kirill Simonov; see
//! `notices/NOTICE.md` and `notices/PyYAML-6.0.3-LICENSE.txt`. The implicit resolvers are the ones the loader already ports
//! ([`super::compose`]).
//!
//! Chunking: the emitter writes many small strings to the stream and flushes it at the end of the document and of the stream;
//! [`safe_dump_pieces`] returns those calls in order (what a buffered file does with them depends on the sizes). A lone surrogate, the
//! only way a JSON value could break an encoding write, is escaped by the emitter itself (`\uXXXX`); the representer's failures (an
//! integer of more than 4300 digits) happen before the first write, as in Python. [`safe_dump_text`] is the concatenation.
//!
//! Not modelled (outside the profile, never produced from JSON data): aliases and anchors (a value two places share), tags that must be
//! written, the flow style of a non-empty collection, block scalars (`|`, `>`), custom representers or dumpers, and the recursion limit
//! of the representer (a nesting beyond [`MAX_DEPTH`] is refused with a `RecursionError`).

use super::compose::resolve_scalar;
use super::YamlError;
use crate::pystr::PyStr;
use crate::pyvalue::{float_repr, PyValue};

const TAG_PREFIX: &str = "tag:yaml.org,2002:";
const MAX_INT_DIGITS: usize = 4300;
/// Nesting beyond this is refused (the representer recurses about three Python frames per level under a limit of 1000).
pub const MAX_DEPTH: usize = 330;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Str,
    Int,
    Float,
    Bool,
    Null,
    Seq,
    Map,
}

impl Kind {
    fn tag(self) -> String {
        let name = match self {
            Kind::Str => "str",
            Kind::Int => "int",
            Kind::Float => "float",
            Kind::Bool => "bool",
            Kind::Null => "null",
            Kind::Seq => "seq",
            Kind::Map => "map",
        };
        format!("{TAG_PREFIX}{name}")
    }

    /// `len(prepare_tag(tag))`: the tag as the emitter would write it (`!!str`, ...), which `check_simple_key` counts.
    fn prepared_len(self) -> usize {
        2 + match self {
            Kind::Str | Kind::Int | Kind::Seq | Kind::Map => 3,
            Kind::Float => 5,
            Kind::Bool | Kind::Null => 4,
        }
    }
}

#[derive(Debug)]
enum Node {
    Scalar(Kind, PyStr),
    Sequence(Vec<Node>),
    Mapping(Vec<(Node, Node)>),
}

fn error(class: &'static str, message: impl AsRef<str>) -> YamlError {
    YamlError {
        class,
        message: PyStr::new(message.as_ref()),
        detail: super::YamlDetail::None,
    }
}

// ── the representer ──────────────────────────────────────────────────────────────────────────────

fn represent(value: &PyValue, depth: usize) -> Result<Node, YamlError> {
    if depth > MAX_DEPTH {
        return Err(error("RecursionError", "maximum recursion depth exceeded"));
    }
    Ok(match value.peel() {
        PyValue::None => Node::Scalar(Kind::Null, PyStr::new("null")),
        PyValue::Bool(flag) => {
            Node::Scalar(Kind::Bool, PyStr::new(if *flag { "true" } else { "false" }))
        }
        PyValue::Int(number) => {
            let digits = number.to_string();
            if digits.trim_start_matches('-').len() > MAX_INT_DIGITS {
                return Err(error(
                    "ValueError",
                    format!("Exceeds the limit ({MAX_INT_DIGITS} digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit"),
                ));
            }
            Node::Scalar(Kind::Int, PyStr::new(&digits))
        }
        PyValue::Float(number) => {
            let text = if number.is_nan() {
                ".nan".to_string()
            } else if *number == f64::INFINITY {
                ".inf".to_string()
            } else if *number == f64::NEG_INFINITY {
                "-.inf".to_string()
            } else {
                let mut text = float_repr(*number).to_lowercase();
                if !text.contains('.') && text.contains('e') {
                    text = text.replacen('e', ".0e", 1);
                }
                text
            };
            Node::Scalar(Kind::Float, PyStr::new(&text))
        }
        PyValue::Str(text) => Node::Scalar(Kind::Str, text.clone()),
        PyValue::List(items) | PyValue::Tuple(items) => {
            let mut nodes = Vec::with_capacity(items.len());
            for item in items {
                nodes.push(represent(item, depth + 1)?);
            }
            Node::Sequence(nodes)
        }
        PyValue::Dict(map) => {
            let mut pairs = Vec::with_capacity(map.len());
            for (other, key, item) in map.keyed_entries() {
                if other.is_some() {
                    return Err(unrepresentable(value));
                }
                pairs.push((
                    Node::Scalar(Kind::Str, key.clone()),
                    represent(item, depth + 1)?,
                ));
            }
            Node::Mapping(pairs)
        }
        _ => return Err(unrepresentable(value)),
    })
}

/// `RepresenterError("cannot represent an object", data)`.
fn unrepresentable(value: &PyValue) -> YamlError {
    error(
        "RepresenterError",
        format!("('cannot represent an object', {})", value.repr()),
    )
}

// ── the serializer: events ───────────────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Event {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    /// `(kind, value, implicit)` with `implicit = (tag == detected tag, tag == default tag)`.
    Scalar(Kind, PyStr, (bool, bool)),
    SequenceStart,
    SequenceEnd,
    MappingStart,
    MappingEnd,
}

fn serialize(node: &Node, events: &mut Vec<Event>) {
    match node {
        Node::Scalar(kind, value) => {
            let detected = resolve_scalar(value, (true, false));
            let default = resolve_scalar(value, (false, true));
            let own = kind.tag();
            events.push(Event::Scalar(
                *kind,
                value.clone(),
                (own == detected, own == default),
            ));
        }
        Node::Sequence(items) => {
            events.push(Event::SequenceStart);
            for item in items {
                serialize(item, events);
            }
            events.push(Event::SequenceEnd);
        }
        Node::Mapping(pairs) => {
            events.push(Event::MappingStart);
            for (key, value) in pairs {
                serialize(key, events);
                serialize(value, events);
            }
            events.push(Event::MappingEnd);
        }
    }
}

// ── the emitter ──────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    StreamStart,
    FirstDocumentStart,
    DocumentStart,
    DocumentEnd,
    DocumentRoot,
    FirstFlowSequenceItem,
    FirstFlowMappingKey,
    FirstBlockSequenceItem,
    BlockSequenceItem,
    FirstBlockMappingKey,
    BlockMappingKey,
    BlockMappingSimpleValue,
    BlockMappingValue,
    Nothing,
}

struct Analysis {
    scalar: Vec<u32>,
    empty: bool,
    multiline: bool,
    allow_flow_plain: bool,
    allow_block_plain: bool,
    allow_single_quoted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Plain,
    Single,
    Double,
}

/// What the emitter does to its stream, in order: one `stream.write(data)` per piece of text, and the `stream.flush()` calls (the end of
/// the document, the end of the stream).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Write(PyStr),
    Flush,
}

const BEST_INDENT: usize = 2;
const BEST_WIDTH: usize = 80;

fn is_break(point: u32) -> bool {
    matches!(point, 0x0A | 0x85 | 0x2028 | 0x2029)
}

/// `ch in '\0 \t\r\n\x85  '`.
fn is_whitespace(point: u32) -> bool {
    matches!(
        point,
        0x00 | 0x20 | 0x09 | 0x0D | 0x0A | 0x85 | 0x2028 | 0x2029
    )
}

struct Emitter {
    pieces: Vec<Piece>,
    events: Vec<Event>,
    current: usize,
    states: Vec<State>,
    state: State,
    indents: Vec<Option<usize>>,
    indent: Option<usize>,
    flow_level: usize,
    root_context: bool,
    mapping_context: bool,
    simple_key_context: bool,
    column: usize,
    whitespace: bool,
    indention: bool,
    open_ended: bool,
    analysis: Option<Analysis>,
    style: Option<Style>,
}

impl Emitter {
    fn new(events: Vec<Event>) -> Emitter {
        Emitter {
            pieces: Vec::new(),
            events,
            current: 0,
            states: Vec::new(),
            state: State::StreamStart,
            indents: Vec::new(),
            indent: None,
            flow_level: 0,
            root_context: false,
            mapping_context: false,
            simple_key_context: false,
            column: 0,
            whitespace: true,
            indention: true,
            open_ended: false,
            analysis: None,
            style: None,
        }
    }

    fn run(mut self) -> Result<Vec<Piece>, YamlError> {
        for index in 0..self.events.len() {
            self.current = index;
            self.step()?;
        }
        Ok(self.pieces)
    }

    fn event(&self) -> &Event {
        &self.events[self.current]
    }

    /// `self.events[0]` of the original: the event after the current one.
    fn next_event(&self) -> Option<&Event> {
        self.events.get(self.current + 1)
    }

    fn unreachable(what: &str) -> YamlError {
        error(
            "EmitterError",
            format!("{what} is outside the supported profile"),
        )
    }

    fn step(&mut self) -> Result<(), YamlError> {
        match self.state {
            State::StreamStart => {
                if !matches!(self.event(), Event::StreamStart) {
                    return Err(Self::unreachable("a stream without its start"));
                }
                self.state = State::FirstDocumentStart;
                Ok(())
            }
            State::FirstDocumentStart => self.expect_document_start(true),
            State::DocumentStart => self.expect_document_start(false),
            State::DocumentEnd => self.expect_document_end(),
            State::DocumentRoot => {
                self.states.push(State::DocumentEnd);
                self.expect_node(true, false, false, false)
            }
            State::FirstFlowSequenceItem => self.expect_first_flow_sequence_item(),
            State::FirstFlowMappingKey => self.expect_first_flow_mapping_key(),
            State::FirstBlockSequenceItem => self.expect_block_sequence_item(true),
            State::BlockSequenceItem => self.expect_block_sequence_item(false),
            State::FirstBlockMappingKey => self.expect_block_mapping_key(true),
            State::BlockMappingKey => self.expect_block_mapping_key(false),
            State::BlockMappingSimpleValue => {
                self.write_indicator(":", false, false, false);
                self.states.push(State::BlockMappingKey);
                self.expect_node(false, false, true, false)
            }
            State::BlockMappingValue => {
                self.write_indent();
                self.write_indicator(":", true, false, true);
                self.states.push(State::BlockMappingKey);
                self.expect_node(false, false, true, false)
            }
            State::Nothing => Err(Self::unreachable("an event after the stream end")),
        }
    }

    fn increase_indent(&mut self, flow: bool, indentless: bool) {
        self.indents.push(self.indent);
        match self.indent {
            None => self.indent = Some(if flow { BEST_INDENT } else { 0 }),
            Some(current) if !indentless => self.indent = Some(current + BEST_INDENT),
            Some(_) => {}
        }
    }

    // ── states ──

    fn expect_document_start(&mut self, first: bool) -> Result<(), YamlError> {
        match self.event() {
            Event::DocumentStart => {
                // `implicit = first and not explicit and not canonical and not version and not tags and not check_empty_document()`;
                // the document's root event has a tag, so it is never the empty document.
                let implicit = first;
                if !implicit {
                    self.write_indent();
                    self.write_indicator("---", true, false, false);
                }
                self.state = State::DocumentRoot;
                Ok(())
            }
            Event::StreamEnd => {
                if self.open_ended {
                    self.write_indicator("...", true, false, false);
                    self.write_indent();
                }
                self.pieces.push(Piece::Flush);
                self.state = State::Nothing;
                Ok(())
            }
            _ => Err(Self::unreachable("an event that is not a document start")),
        }
    }

    fn expect_document_end(&mut self) -> Result<(), YamlError> {
        if !matches!(self.event(), Event::DocumentEnd) {
            return Err(Self::unreachable("an event that is not a document end"));
        }
        self.write_indent();
        self.pieces.push(Piece::Flush);
        self.state = State::DocumentStart;
        Ok(())
    }

    fn expect_node(
        &mut self,
        root: bool,
        _sequence: bool,
        mapping: bool,
        simple_key: bool,
    ) -> Result<(), YamlError> {
        self.root_context = root;
        self.mapping_context = mapping;
        self.simple_key_context = simple_key;
        match self.event() {
            Event::Scalar(..) => {
                self.process_tag()?;
                self.increase_indent(true, false);
                self.process_scalar();
                self.indent = self.indents.pop().expect("an indent was pushed");
                self.state = self.states.pop().expect("a state was pushed");
                Ok(())
            }
            Event::SequenceStart => {
                self.process_collection_tag();
                if self.flow_level > 0 || self.check_empty_sequence() {
                    self.write_indicator("[", true, true, false);
                    self.flow_level += 1;
                    self.increase_indent(true, false);
                    self.state = State::FirstFlowSequenceItem;
                } else {
                    let indentless = self.mapping_context && !self.indention;
                    self.increase_indent(false, indentless);
                    self.state = State::FirstBlockSequenceItem;
                }
                Ok(())
            }
            Event::MappingStart => {
                self.process_collection_tag();
                if self.flow_level > 0 || self.check_empty_mapping() {
                    self.write_indicator("{", true, true, false);
                    self.flow_level += 1;
                    self.increase_indent(true, false);
                    self.state = State::FirstFlowMappingKey;
                } else {
                    self.increase_indent(false, false);
                    self.state = State::FirstBlockMappingKey;
                }
                Ok(())
            }
            _ => Err(Self::unreachable("an event that is not a node")),
        }
    }

    fn expect_first_flow_sequence_item(&mut self) -> Result<(), YamlError> {
        if !matches!(self.event(), Event::SequenceEnd) {
            return Err(Self::unreachable("a flow sequence with items"));
        }
        self.indent = self.indents.pop().expect("an indent was pushed");
        self.flow_level -= 1;
        self.write_indicator("]", false, false, false);
        self.state = self.states.pop().expect("a state was pushed");
        Ok(())
    }

    fn expect_first_flow_mapping_key(&mut self) -> Result<(), YamlError> {
        if !matches!(self.event(), Event::MappingEnd) {
            return Err(Self::unreachable("a flow mapping with items"));
        }
        self.indent = self.indents.pop().expect("an indent was pushed");
        self.flow_level -= 1;
        self.write_indicator("}", false, false, false);
        self.state = self.states.pop().expect("a state was pushed");
        Ok(())
    }

    fn expect_block_sequence_item(&mut self, first: bool) -> Result<(), YamlError> {
        if !first && matches!(self.event(), Event::SequenceEnd) {
            self.indent = self.indents.pop().expect("an indent was pushed");
            self.state = self.states.pop().expect("a state was pushed");
        } else {
            self.write_indent();
            self.write_indicator("-", true, false, true);
            self.states.push(State::BlockSequenceItem);
            self.expect_node(false, true, false, false)?;
        }
        Ok(())
    }

    fn expect_block_mapping_key(&mut self, first: bool) -> Result<(), YamlError> {
        if !first && matches!(self.event(), Event::MappingEnd) {
            self.indent = self.indents.pop().expect("an indent was pushed");
            self.state = self.states.pop().expect("a state was pushed");
        } else {
            self.write_indent();
            if self.check_simple_key() {
                self.states.push(State::BlockMappingSimpleValue);
                self.expect_node(false, false, true, true)?;
            } else {
                self.write_indicator("?", true, false, true);
                self.states.push(State::BlockMappingValue);
                self.expect_node(false, false, true, false)?;
            }
        }
        Ok(())
    }

    // ── checks ──

    fn check_empty_sequence(&self) -> bool {
        matches!(self.event(), Event::SequenceStart)
            && matches!(self.next_event(), Some(Event::SequenceEnd))
    }

    fn check_empty_mapping(&self) -> bool {
        matches!(self.event(), Event::MappingStart)
            && matches!(self.next_event(), Some(Event::MappingEnd))
    }

    fn check_simple_key(&mut self) -> bool {
        let mut length = 0;
        match self.event() {
            Event::Scalar(kind, _, _) => length += kind.prepared_len(),
            Event::SequenceStart => length += Kind::Seq.prepared_len(),
            Event::MappingStart => length += Kind::Map.prepared_len(),
            _ => {}
        }
        let is_scalar = matches!(self.event(), Event::Scalar(..));
        if is_scalar {
            if self.analysis.is_none() {
                self.analysis = Some(self.analyze_current());
            }
            length += self.analysis.as_ref().expect("analysed").scalar.len();
        }
        length < 128
            && ((is_scalar && {
                let analysis = self.analysis.as_ref().expect("analysed");
                !analysis.empty && !analysis.multiline
            }) || self.check_empty_sequence()
                || self.check_empty_mapping())
    }

    // ── tags and styles ──

    fn process_collection_tag(&mut self) {
        // `(not canonical or tag is None) and event.implicit`: the collection tag is the default one, so nothing is written.
    }

    fn process_tag(&mut self) -> Result<(), YamlError> {
        if self.style.is_none() {
            self.style = Some(self.choose_scalar_style());
        }
        let Event::Scalar(_, _, implicit) = self.event() else {
            unreachable!("process_tag is called for scalars");
        };
        let plain = self.style == Some(Style::Plain);
        if (plain && implicit.0) || (!plain && implicit.1) {
            return Ok(());
        }
        Err(Self::unreachable("a scalar that needs its tag written"))
    }

    fn analyze_current(&self) -> Analysis {
        let Event::Scalar(_, value, _) = self.event() else {
            unreachable!("the analysed event is a scalar");
        };
        analyze_scalar(value.code_points())
    }

    fn choose_scalar_style(&mut self) -> Style {
        if self.analysis.is_none() {
            self.analysis = Some(self.analyze_current());
        }
        let implicit_plain = match self.event() {
            Event::Scalar(_, _, implicit) => implicit.0,
            _ => false,
        };
        let analysis = self.analysis.as_ref().expect("analysed");
        if implicit_plain
            && !(self.simple_key_context && (analysis.empty || analysis.multiline))
            && ((self.flow_level > 0 && analysis.allow_flow_plain)
                || (self.flow_level == 0 && analysis.allow_block_plain))
        {
            return Style::Plain;
        }
        if analysis.allow_single_quoted && !(self.simple_key_context && analysis.multiline) {
            return Style::Single;
        }
        Style::Double
    }

    fn process_scalar(&mut self) {
        if self.analysis.is_none() {
            self.analysis = Some(self.analyze_current());
        }
        if self.style.is_none() {
            self.style = Some(self.choose_scalar_style());
        }
        let split = !self.simple_key_context;
        let text = self.analysis.as_ref().expect("analysed").scalar.clone();
        match self.style.expect("chosen") {
            Style::Double => self.write_double_quoted(&text, split),
            Style::Single => self.write_single_quoted(&text, split),
            Style::Plain => self.write_plain(&text, split),
        }
        self.analysis = None;
        self.style = None;
    }

    // ── writers ──

    /// One `stream.write(data)` of the original (an empty one changes nothing and is not recorded).
    fn write_points(&mut self, points: &[u32]) {
        if !points.is_empty() {
            self.pieces
                .push(Piece::Write(PyStr::from_code_points(points.to_vec())));
        }
    }

    fn put_str(&mut self, text: &str) {
        if !text.is_empty() {
            self.pieces.push(Piece::Write(PyStr::new(text)));
        }
    }

    fn write_indicator(
        &mut self,
        indicator: &str,
        need_whitespace: bool,
        whitespace: bool,
        indention: bool,
    ) {
        let data = if self.whitespace || !need_whitespace {
            indicator.to_string()
        } else {
            format!(" {indicator}")
        };
        self.whitespace = whitespace;
        self.indention = self.indention && indention;
        self.column += data.chars().count();
        self.open_ended = false;
        self.put_str(&data);
    }

    fn write_indent(&mut self) {
        let indent = self.indent.unwrap_or(0);
        if !self.indention || self.column > indent || (self.column == indent && !self.whitespace) {
            self.write_line_break(None);
        }
        if self.column < indent {
            self.whitespace = true;
            let spaces = " ".repeat(indent - self.column);
            self.put_str(&spaces);
            self.column = indent;
        }
    }

    fn write_line_break(&mut self, data: Option<u32>) {
        self.whitespace = true;
        self.indention = true;
        self.column = 0;
        self.write_points(&[data.unwrap_or(0x0A)]);
    }

    fn write_single_quoted(&mut self, text: &[u32], split: bool) {
        self.write_indicator("'", true, false, false);
        let mut spaces = false;
        let mut breaks = false;
        let (mut start, mut end) = (0usize, 0usize);
        while end <= text.len() {
            let ch = text.get(end).copied();
            if spaces {
                if ch != Some(0x20) {
                    if start + 1 == end
                        && self.column > BEST_WIDTH
                        && split
                        && start != 0
                        && end != text.len()
                    {
                        self.write_indent();
                    } else {
                        let data = &text[start..end];
                        self.column += data.len();
                        self.write_points(data);
                    }
                    start = end;
                }
            } else if breaks {
                if ch.is_none_or(|c| !is_break(c)) {
                    if text[start] == 0x0A {
                        self.write_line_break(None);
                    }
                    for &br in &text[start..end] {
                        if br == 0x0A {
                            self.write_line_break(None);
                        } else {
                            self.write_line_break(Some(br));
                        }
                    }
                    self.write_indent();
                    start = end;
                }
            } else if ch.is_none_or(|c| c == 0x20 || is_break(c) || c == 0x27) && start < end {
                let data = &text[start..end];
                self.column += data.len();
                self.write_points(data);
                start = end;
            }
            if ch == Some(0x27) {
                self.column += 2;
                self.put_str("''");
                start = end + 1;
            }
            if let Some(c) = ch {
                spaces = c == 0x20;
                breaks = is_break(c);
            }
            end += 1;
        }
        self.write_indicator("'", false, false, false);
    }

    fn write_double_quoted(&mut self, text: &[u32], split: bool) {
        self.write_indicator("\"", true, false, false);
        let (mut start, mut end) = (0usize, 0usize);
        while end <= text.len() {
            let ch = text.get(end).copied();
            let needs_escape = match ch {
                None => true,
                Some(c) => {
                    matches!(c, 0x22 | 0x5C | 0x85 | 0x2028 | 0x2029 | 0xFEFF)
                        || !((0x20..=0x7E).contains(&c)
                            || (0xA0..=0xD7FF).contains(&c)
                            || (0xE000..=0xFFFD).contains(&c))
                }
            };
            if needs_escape {
                if start < end {
                    let data = &text[start..end];
                    self.column += data.len();
                    self.write_points(data);
                    start = end;
                }
                if let Some(c) = ch {
                    let data = match c {
                        0x00 => "\\0".to_string(),
                        0x07 => "\\a".to_string(),
                        0x08 => "\\b".to_string(),
                        0x09 => "\\t".to_string(),
                        0x0A => "\\n".to_string(),
                        0x0B => "\\v".to_string(),
                        0x0C => "\\f".to_string(),
                        0x0D => "\\r".to_string(),
                        0x1B => "\\e".to_string(),
                        0x22 => "\\\"".to_string(),
                        0x5C => "\\\\".to_string(),
                        0x85 => "\\N".to_string(),
                        0xA0 => "\\_".to_string(),
                        0x2028 => "\\L".to_string(),
                        0x2029 => "\\P".to_string(),
                        c if c <= 0xFF => format!("\\x{c:02X}"),
                        c if c <= 0xFFFF => format!("\\u{c:04X}"),
                        c => format!("\\U{c:08X}"),
                    };
                    self.column += data.chars().count();
                    self.put_str(&data);
                    start = end + 1;
                }
            }
            if 0 < end
                && end + 1 < text.len()
                && (ch == Some(0x20) || start >= end)
                && self.column + (end - start) > BEST_WIDTH
                && split
            {
                let mut data: Vec<u32> = text[start..end].to_vec();
                data.push(u32::from('\\'));
                if start < end {
                    start = end;
                }
                self.column += data.len();
                self.write_points(&data);
                self.write_indent();
                self.whitespace = false;
                self.indention = false;
                if text[start] == 0x20 {
                    self.column += 1;
                    self.put_str("\\");
                }
            }
            end += 1;
        }
        self.write_indicator("\"", false, false, false);
    }

    fn write_plain(&mut self, text: &[u32], split: bool) {
        if self.root_context {
            self.open_ended = true;
        }
        if text.is_empty() {
            return;
        }
        if !self.whitespace {
            self.column += 1;
            self.put_str(" ");
        }
        self.whitespace = false;
        self.indention = false;
        let mut spaces = false;
        let mut breaks = false;
        let (mut start, mut end) = (0usize, 0usize);
        while end <= text.len() {
            let ch = text.get(end).copied();
            if spaces {
                if ch != Some(0x20) {
                    if start + 1 == end && self.column > BEST_WIDTH && split {
                        self.write_indent();
                        self.whitespace = false;
                        self.indention = false;
                    } else {
                        let data = &text[start..end];
                        self.column += data.len();
                        self.write_points(data);
                    }
                    start = end;
                }
            } else if breaks {
                if ch.is_none_or(|c| !is_break(c)) {
                    if text[start] == 0x0A {
                        self.write_line_break(None);
                    }
                    for &br in &text[start..end] {
                        if br == 0x0A {
                            self.write_line_break(None);
                        } else {
                            self.write_line_break(Some(br));
                        }
                    }
                    self.write_indent();
                    self.whitespace = false;
                    self.indention = false;
                    start = end;
                }
            } else if ch.is_none_or(|c| c == 0x20 || is_break(c)) {
                let data = &text[start..end];
                self.column += data.len();
                self.write_points(data);
                start = end;
            }
            if let Some(c) = ch {
                spaces = c == 0x20;
                breaks = is_break(c);
            }
            end += 1;
        }
    }
}

/// `Emitter.analyze_scalar` with `allow_unicode=True`.
fn analyze_scalar(scalar: &[u32]) -> Analysis {
    if scalar.is_empty() {
        return Analysis {
            scalar: Vec::new(),
            empty: true,
            multiline: false,
            allow_flow_plain: false,
            allow_block_plain: true,
            allow_single_quoted: true,
        };
    }
    let mut block_indicators = false;
    let mut flow_indicators = false;
    let mut line_breaks = false;
    let mut special_characters = false;
    let mut leading_space = false;
    let mut leading_break = false;
    let mut trailing_space = false;
    let mut trailing_break = false;
    let mut break_space = false;
    let mut space_break = false;

    let starts = |prefix: &str| {
        let p: Vec<u32> = prefix.chars().map(u32::from).collect();
        scalar.starts_with(&p)
    };
    if starts("---") || starts("...") {
        block_indicators = true;
        flow_indicators = true;
    }
    let mut preceded_by_whitespace = true;
    let mut followed_by_whitespace = scalar.len() == 1 || is_whitespace(scalar[1]);
    let mut previous_space = false;
    let mut previous_break = false;
    let mut index = 0;
    while index < scalar.len() {
        let ch = scalar[index];
        let as_char = char::from_u32(ch);
        if index == 0 {
            if matches!(
                as_char,
                Some(
                    '#' | ','
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '&'
                        | '*'
                        | '!'
                        | '|'
                        | '>'
                        | '\''
                        | '"'
                        | '%'
                        | '@'
                        | '`'
                )
            ) {
                flow_indicators = true;
                block_indicators = true;
            }
            if matches!(as_char, Some('?' | ':')) {
                flow_indicators = true;
                if followed_by_whitespace {
                    block_indicators = true;
                }
            }
            if as_char == Some('-') && followed_by_whitespace {
                flow_indicators = true;
                block_indicators = true;
            }
        } else {
            if matches!(as_char, Some(',' | '?' | '[' | ']' | '{' | '}')) {
                flow_indicators = true;
            }
            if as_char == Some(':') {
                flow_indicators = true;
                if followed_by_whitespace {
                    block_indicators = true;
                }
            }
            if as_char == Some('#') && preceded_by_whitespace {
                flow_indicators = true;
                block_indicators = true;
            }
        }
        if is_break(ch) {
            line_breaks = true;
        }
        if !(ch == 0x0A || (0x20..=0x7E).contains(&ch)) {
            let printable = (ch == 0x85
                || (0xA0..=0xD7FF).contains(&ch)
                || (0xE000..=0xFFFD).contains(&ch)
                || (0x10000..0x10FFFF).contains(&ch))
                && ch != 0xFEFF;
            if !printable {
                special_characters = true;
            }
        }
        if ch == 0x20 {
            if index == 0 {
                leading_space = true;
            }
            if index == scalar.len() - 1 {
                trailing_space = true;
            }
            if previous_break {
                break_space = true;
            }
            previous_space = true;
            previous_break = false;
        } else if is_break(ch) {
            if index == 0 {
                leading_break = true;
            }
            if index == scalar.len() - 1 {
                trailing_break = true;
            }
            if previous_space {
                space_break = true;
            }
            previous_space = false;
            previous_break = true;
        } else {
            previous_space = false;
            previous_break = false;
        }
        index += 1;
        preceded_by_whitespace = is_whitespace(ch);
        followed_by_whitespace = index + 1 >= scalar.len() || is_whitespace(scalar[index + 1]);
    }
    let mut allow_flow_plain = true;
    let mut allow_block_plain = true;
    let mut allow_single_quoted = true;
    if leading_space || leading_break || trailing_space || trailing_break {
        allow_flow_plain = false;
        allow_block_plain = false;
    }
    if break_space {
        allow_flow_plain = false;
        allow_block_plain = false;
        allow_single_quoted = false;
    }
    if space_break || special_characters {
        allow_flow_plain = false;
        allow_block_plain = false;
        allow_single_quoted = false;
    }
    if line_breaks {
        allow_flow_plain = false;
        allow_block_plain = false;
    }
    if flow_indicators {
        allow_flow_plain = false;
    }
    if block_indicators {
        allow_block_plain = false;
    }
    Analysis {
        scalar: scalar.to_vec(),
        empty: false,
        multiline: line_breaks,
        allow_flow_plain,
        allow_block_plain,
        allow_single_quoted,
    }
}

/// `yaml.safe_dump(value, stream, allow_unicode=True, sort_keys=False)` as the calls it makes on the stream: every `write` and `flush`,
/// in order. The representer's failures come before the first of them.
pub fn safe_dump_pieces(value: &PyValue) -> Result<Vec<Piece>, YamlError> {
    let node = represent(value, 0)?;
    let mut events = vec![Event::StreamStart, Event::DocumentStart];
    serialize(&node, &mut events);
    events.push(Event::DocumentEnd);
    events.push(Event::StreamEnd);
    Emitter::new(events).run()
}

/// `yaml.safe_dump(value, allow_unicode=True, sort_keys=False)`: the text of one document.
pub fn safe_dump_text(value: &PyValue) -> Result<PyStr, YamlError> {
    let mut text = PyStr::default();
    for piece in safe_dump_pieces(value)? {
        if let Piece::Write(part) = piece {
            text.push_py(&part);
        }
    }
    Ok(text)
}
