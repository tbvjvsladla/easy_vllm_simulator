//! `yaml.safe_load` of PyYAML 6.0.3, ported to Rust.
//!
//! The manifest reader of `roofline.py` is `yaml.safe_load(open(path, encoding="utf-8"))`. Its observable behaviour is
//! the pure-Python pipeline `SafeLoader` = Reader → Scanner → Parser → Composer → Resolver → SafeConstructor: which
//! documents are accepted, the YAML 1.1 types they become, and - for the rest - the exception class and the exact
//! multi-line message with its file name, line and column. Matching that text needs PyYAML's own scanner and parser,
//! so this module is a function-by-function port of those files rather than a wrapper around another YAML library
//! (whose grammar and messages differ). Derived from PyYAML 6.0.3 (`yaml/{reader,scanner,parser,composer,resolver,
//! constructor,error,tokens,events,nodes}.py`), MIT License, Copyright (c) 2017-2021 Ingy döt Net and (c) 2006-2016
//! Kirill Simonov; see `notices/NOTICE.md` and `notices/PyYAML-6.0.3-LICENSE.txt`.
//!
//! Python behaviour reproduced: the source is read like a text file (UTF-8, universal newlines, 8192-byte chunks
//! whose boundaries decide *when* a decoding error surfaces), the Reader pulls 4096 characters at a time, the
//! constructor builds containers level by level (so the first error is the one a breadth-first walk meets), and
//! exceptions keep their class names (`ScannerError`, `ParserError`, `ComposerError`, `ConstructorError`,
//! `ReaderError`, `UnicodeDecodeError`, `ValueError`, ...).
//!
//! Shared objects: Python builds one object for an anchored node however many aliases point at it. So does the port
//! ([`PyValue::Shared`]): the constructed value of a node that more than one place refers to is built once and
//! cloned by reference, which keeps a 26-level doubling graph of aliases (tens of millions of nodes when expanded)
//! as small as it is in Python. A node that contains itself is the one place where a [`PyValue`] (a tree) cannot be
//! the Python object: the inner reference is a marker that prints as `[...]` / `{...}` and refuses JSON like
//! `json.dumps` does ("Circular reference detected"); `nodes: &n [*n, *n]` has length 2, `gpus_per_node: &a [*a]`
//! fails `int()` with the same message and `repr`, `interconnect: &i {bandwidth_gbps: *i}` fails at the JSON dump.
//!
//! Nesting depth is the recursion limit of the original, reproduced by counting Python frames (see [`frames`]): the
//! port opens a frame wherever the original calls a Python function, so `RecursionError` comes at the same document
//! and wins over (or loses to) the same syntax error as it does there.
//!
//! Differences that remain, by design: a document that makes a value contain itself *and* shares a cyclic
//! sub-structure exponentially is not built in linear time (the value of a node inside a cycle depends on which nodes
//! of the cycle are open around it, so only acyclic and singly-cyclic sharing is memoised); the `!!binary` and
//! timestamp constructors follow CPython 3.12.3 (`binascii.a2b_base64`, `str.encode`, `datetime`) as observed.

mod compose;
mod dump;
mod frames;
mod parser;
mod reader;
mod scanner;
mod sites;

use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
pub use compose::dict_key;
pub use dump::{safe_dump_pieces, safe_dump_text, Piece};
pub use frames::PySite;
use std::rc::Rc;

/// An exception `safe_load` raised: its Python class name and `str(exception)`, and for the two classes whose constructor
/// needs more than the message the actual constructor fields (never to be recovered from the message).
#[derive(Debug, Clone, PartialEq)]
pub struct YamlError {
    pub class: &'static str,
    pub message: PyStr,
    pub detail: YamlDetail,
}

impl YamlError {
    /// The Python traceback of the original this failure mirrors (outermost first, from `yaml.safe_load` down), as source-mapping data; `None`
    /// when the failure arose where the port has no mapped chain (the scanner, the constructor, a late reader chunk). Never a native stack.
    pub fn frames(&self) -> Option<&[PySite]> {
        match &self.detail {
            YamlDetail::Marked(detail) => detail.frames.as_deref().map(Vec::as_slice),
            YamlDetail::Decode { frames, .. } => frames.as_deref().map(Vec::as_slice),
            _ => None,
        }
    }
}

/// The constructor fields of `yaml.reader.ReaderError` and of the `UnicodeDecodeError` of the text file underneath.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum YamlDetail {
    #[default]
    None,
    /// `ReaderError(name, position, character, encoding, reason)` (`encoding` is `"unicode"` for a text stream).
    Reader {
        name: PyStr,
        position: usize,
        character: u32,
        encoding: &'static str,
        reason: &'static str,
    },
    /// `UnicodeDecodeError(encoding, object, start, end, reason)`: `object` is the byte chunk that was being decoded.
    Decode {
        encoding: &'static str,
        object: Vec<u8>,
        start: usize,
        end: usize,
        reason: &'static str,
        /// The mapped traceback of the failure (see [`YamlError::frames`]).
        frames: Option<Box<Vec<PySite>>>,
    },
    /// The constructor state of a `yaml.error.MarkedYAMLError` subclass (`ScannerError`, `ParserError`, `ComposerError`, `ConstructorError`):
    /// `MarkedYAMLError(context, context_mark, problem, problem_mark, note)`. The marks are the six-field `yaml.error.Mark` objects.
    Marked(Box<MarkedDetail>),
}

/// The state of `MarkedYAMLError(context, context_mark, problem, problem_mark, note)`; the marks are the six-field `yaml.error.Mark` objects.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkedDetail {
    pub context: Option<PyStr>,
    pub context_mark: Option<Mark6>,
    pub problem: Option<PyStr>,
    pub problem_mark: Option<Mark6>,
    pub note: Option<PyStr>,
    /// `context_mark is problem_mark`: PyYAML passed ONE `Mark` object for both (decided from the marks' provenance, not their values).
    pub shared_marks: bool,
    /// The mapped traceback of the failure (see [`YamlError::frames`]).
    pub frames: Option<Box<Vec<PySite>>>,
}

/// `yaml.error.Mark(name, index, line, column, buffer, pointer)` as the loader of a text FILE builds it: `index` counts the characters the
/// reader consumed, `line` and `column` are zero-based, and `buffer`/`pointer` are `None` (a file stream keeps no snippet; only a `str`
/// source passes its buffer, and this port's marks of a `str` source carry no snippet either).
#[derive(Debug, Clone, PartialEq)]
pub struct Mark6 {
    pub name: PyStr,
    pub index: usize,
    pub line: usize,
    pub column: usize,
    pub buffer: Option<PyStr>,
    pub pointer: Option<usize>,
}

/// One pull of the reader on the open text file, in order: `read(4096)`, then what it returned or the exception it raised.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadEvent {
    Read { size: usize },
    Return { text: PyStr },
    Error { class: &'static str, message: PyStr },
}

/// A position in the stream (`yaml.error.Mark` of a stream source: file name, line and column, no snippet).
#[derive(Clone, Debug)]
pub(crate) struct Mark {
    pub name: Rc<PyStr>,
    pub index: usize,
    pub line: usize,
    pub column: usize,
    /// Which Python `Mark` OBJECT this stands for. The reader creates a new object on every `get_mark()` (a new origin here) and the
    /// scanner, parser and composer then pass that object around by reference (a clone keeps the origin). Two marks of an error are the
    /// same Python object exactly when their origins are equal; equal positions of two different objects are not.
    pub origin: u64,
}

impl Mark {
    fn render(&self) -> PyStr {
        let mut out = PyStr::new("  in \"");
        out.push_py(&self.name);
        out.push_str(&format!(
            "\", line {}, column {}",
            self.line + 1,
            self.column + 1
        ));
        out
    }

    fn to_public(&self) -> Mark6 {
        Mark6 {
            name: (*self.name).clone(),
            index: self.index,
            line: self.line,
            column: self.column,
            buffer: None,
            pointer: None,
        }
    }

    fn same_place(&self, other: &Mark) -> bool {
        *self.name == *other.name && self.line == other.line && self.column == other.column
    }
}

/// Everything that can stop the load.
#[derive(Debug, Clone)]
pub(crate) enum Failure {
    /// A `MarkedYAMLError` subclass: context, context mark, problem, problem mark.
    Marked {
        class: &'static str,
        context: Option<String>,
        context_mark: Option<Box<Mark>>,
        problem: Option<String>,
        problem_mark: Option<Box<Mark>>,
    },
    /// `ReaderError` for a character YAML does not allow.
    Reader {
        name: Rc<PyStr>,
        position: usize,
        character: u32,
        reason: &'static str,
    },
    /// A `UnicodeDecodeError` of the text file: the byte chunk, the undecodable run and the reason.
    Decode {
        object: Vec<u8>,
        start: usize,
        end: usize,
        reason: &'static str,
    },
    /// Any other exception (`ValueError`, `KeyError`, ...).
    Plain { class: &'static str, message: PyStr },
}

pub(crate) type Parsed<T> = Result<T, Failure>;

pub(crate) fn marked(
    class: &'static str,
    context: Option<&str>,
    context_mark: Option<&Mark>,
    problem: impl Into<String>,
    problem_mark: Option<&Mark>,
) -> Failure {
    // raising `X(...)` first calls `MarkedYAMLError.__init__`, which may itself be one frame too many
    if let Err(error) = frames::nested(&["__init__"]) {
        return error;
    }
    frames::capture(None);
    Failure::Marked {
        class,
        context: context.map(str::to_string),
        context_mark: context_mark.cloned().map(Box::new),
        problem: Some(problem.into()),
        problem_mark: problem_mark.cloned().map(Box::new),
    }
}

pub(crate) fn plain(class: &'static str, message: impl Into<String>) -> Failure {
    Failure::Plain {
        class,
        message: PyStr::new(&message.into()),
    }
}

impl Failure {
    /// `type(exception).__name__` and `str(exception)`.
    pub(crate) fn into_error(self) -> YamlError {
        match self {
            Failure::Plain { class, message } => {
                let _ = frames::take_captured();
                YamlError {
                    class,
                    message,
                    detail: YamlDetail::None,
                }
            }
            Failure::Decode {
                object,
                start,
                end,
                reason,
            } => {
                let subject = if end - start == 1 {
                    format!("byte 0x{:02x} in position {start}", object[start])
                } else {
                    format!("bytes in position {start}-{}", end - 1)
                };
                YamlError {
                    class: "UnicodeDecodeError",
                    message: PyStr::new(&format!("'utf-8' codec can't decode {subject}: {reason}")),
                    detail: YamlDetail::Decode {
                        encoding: "utf-8",
                        object,
                        start,
                        end,
                        reason,
                        frames: frames::take_captured().map(Box::new),
                    },
                }
            }
            Failure::Reader {
                name,
                position,
                character,
                reason,
            } => {
                let mut message = PyStr::new(&format!(
                    "unacceptable character #x{character:04x}: {reason}\n  in \""
                ));
                message.push_py(&name);
                message.push_str(&format!("\", position {position}"));
                YamlError {
                    class: "ReaderError",
                    message,
                    detail: YamlDetail::Reader {
                        name: (*name).clone(),
                        position,
                        character,
                        encoding: "unicode",
                        reason,
                    },
                }
            }
            Failure::Marked {
                class,
                context,
                context_mark,
                problem,
                problem_mark,
            } => {
                let mut lines: Vec<PyStr> = Vec::new();
                if let Some(context) = &context {
                    lines.push(PyStr::new(context));
                }
                if let Some(mark) = &context_mark {
                    let show = match (&problem, &problem_mark) {
                        (Some(_), Some(problem_mark)) => !mark.same_place(problem_mark),
                        _ => true,
                    };
                    if show {
                        lines.push(mark.render());
                    }
                }
                if let Some(problem) = &problem {
                    lines.push(PyStr::new(problem));
                }
                if let Some(mark) = &problem_mark {
                    lines.push(mark.render());
                }
                let mut message = PyStr::default();
                for (index, line) in lines.iter().enumerate() {
                    if index > 0 {
                        message.push_str("\n");
                    }
                    message.push_py(line);
                }
                let shared_marks = matches!((&context_mark, &problem_mark), (Some(a), Some(b)) if a.origin == b.origin);
                YamlError {
                    class,
                    message,
                    detail: YamlDetail::Marked(Box::new(MarkedDetail {
                        shared_marks,
                        context: context.as_deref().map(PyStr::new),
                        context_mark: context_mark.as_deref().map(Mark::to_public),
                        problem: problem.as_deref().map(PyStr::new),
                        problem_mark: problem_mark.as_deref().map(Mark::to_public),
                        note: None,
                        frames: frames::take_captured().map(Box::new),
                    })),
                }
            }
        }
    }
}

/// `yaml.safe_load(open(name, encoding="utf-8"))` given the file's bytes and the name Python would show in marks,
/// under the recursion limit the original CLI runs with.
pub fn safe_load(bytes: &[u8], name: &PyStr) -> Result<PyValue, YamlError> {
    frames::reset(frames::LIMIT, false);
    load(Source::File { bytes, name }, None).map_err(Failure::into_error)
}

/// [`safe_load`] for a caller that keys dictionaries or tests membership with the loaded values (Python's `dict` and `in` ask
/// "the same object?" before "equal?", which decides `nan`): NaNs are objects, as for a `str` source.
pub fn safe_load_objects(bytes: &[u8], name: &PyStr) -> Result<PyValue, YamlError> {
    frames::reset(frames::LIMIT, false);
    load_with(Source::File { bytes, name }, None, true).map_err(Failure::into_error)
}

/// [`safe_load`] that also reports every pull of the reader on the file (what a program that opened the file itself and
/// watches `read` sees), in order, whether the load went on or failed.
pub fn safe_load_observed(
    bytes: &[u8],
    name: &PyStr,
) -> (Result<PyValue, YamlError>, Vec<ReadEvent>) {
    frames::reset(frames::LIMIT, false);
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let result =
        load(Source::File { bytes, name }, Some(events.clone())).map_err(Failure::into_error);
    let events = events.take();
    (result, events)
}

/// The name PyYAML shows in the marks of a `str` source.
pub const TEXT_SOURCE_NAME: &str = "<unicode string>";

/// The recursion limit of the interpreter the original runs on.
const PYTHON_RECURSION_LIMIT: usize = 1000;

/// `yaml.safe_load(text)` for a `str` already read: the marks name `<unicode string>` and the document is read whole
/// (no 8192-byte decoding chunks). `open_frames` is how many Python frames the caller of `safe_load` keeps open
/// (module, `main`, ...); `safe_load` itself is the next one.
pub fn safe_load_text(text: &PyStr, open_frames: usize) -> Result<PyValue, YamlError> {
    frames::reset(PYTHON_RECURSION_LIMIT.saturating_sub(open_frames), false);
    load(Source::Text(text), None).map_err(Failure::into_error)
}

/// Where the pulls of the reader are told (a program watching the file).
pub(crate) type Observer = Rc<std::cell::RefCell<Vec<ReadEvent>>>;

/// Where the characters of a load come from.
enum Source<'a> {
    /// An open text file: the bytes, and the name the marks show.
    File { bytes: &'a [u8], name: &'a PyStr },
    /// A `str`.
    Text(&'a PyStr),
}

/// What a load did besides its result: the deepest Python stack the original would have reached (`safe_load` = 1)
/// and the calls it made while composing, as `(depth, function name)`.
#[derive(Debug)]
pub struct Probe {
    pub result: Result<PyValue, YamlError>,
    pub peak: usize,
    pub trace: Vec<(usize, &'static str)>,
}

/// A load with no recursion limit that reports the stack depth - the transport for comparing the port's call
/// structure with the original's (`roofline yaml-depth`).
pub fn probe(bytes: &[u8], name: &PyStr) -> Probe {
    frames::reset(usize::MAX / 2, true);
    let result = load(Source::File { bytes, name }, None).map_err(Failure::into_error);
    let mut trace = frames::take_trace();
    if !trace.iter().any(|(_, name)| *name == "get_single_data") {
        trace.clear();
    }
    if let Some(at) = trace
        .iter()
        .position(|(_, name)| *name == "construct_document")
    {
        trace.truncate(at);
    }
    Probe {
        result,
        peak: frames::peak(),
        trace,
    }
}

fn load(source: Source<'_>, observer: Option<Observer>) -> Parsed<PyValue> {
    // a `str` source is read by programs that test membership: its NaNs are objects (`pyvalue::nan_object`)
    let nan_objects = matches!(source, Source::Text(_));
    load_with(source, observer, nan_objects)
}

/// [`load`] with the NaN-object choice made by the caller: with `nan_objects`, `.nan` is the one shared object of
/// `SafeConstructor.nan_value`, any other NaN is an object of its node and an alias reuses it (see `pyvalue::nan_object`).
fn load_with(source: Source<'_>, observer: Option<Observer>, nan_objects: bool) -> Parsed<PyValue> {
    let _safe_load = frames::frame("safe_load")?;
    let _load = frames::frame("load")?;
    let name = Rc::new(match &source {
        Source::File { name, .. } => (*name).clone(),
        Source::Text(_) => PyStr::new(TEXT_SOURCE_NAME),
    });
    // `Loader.__init__` builds the reader (`Reader.__init__` -> `determine_encoding`) and then the scanner
    frames::hint("SafeLoader", None);
    let loader = frames::frame("__init__")?;
    let reader = {
        // `Reader.__init__` is the first of the six base initialisers `SafeLoader.__init__` calls; it reaches `determine_encoding` at its
        // second call site (the file-stream branch)
        frames::hint("Reader", Some(0));
        let _init = frames::frame("__init__")?;
        match &source {
            Source::File { bytes, .. } => {
                frames::occurrence(1);
                let _determine = frames::frame("determine_encoding")?;
                reader::Reader::new(name, bytes, observer)?
            }
            Source::Text(text) => reader::Reader::from_text(name, text)?,
        }
    };
    let scanner = {
        frames::hint("Scanner", Some(1));
        let _init = frames::frame("__init__")?;
        scanner::Scanner::new(reader)?
    };
    drop(loader);
    let mut parser = parser::Parser::new(scanner);
    frames::forget_trace();
    let _get_single_data = frames::frame("get_single_data")?;
    let composed = compose::get_single_node(&mut parser)?;
    compose::construct_document(composed, nan_objects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_text(text: &str) -> Result<PyValue, YamlError> {
        safe_load(text.as_bytes(), &PyStr::new("m.yaml"))
    }

    fn message(text: &str) -> String {
        let error = load_text(text).unwrap_err();
        format!("{}: {}", error.class, error.message.to_utf8().unwrap())
    }

    #[test]
    fn plain_mappings_and_sequences() {
        let value = load_text("a: 1\nb: [x, 2.5, null]\nc:\n  - d: yes\n").unwrap();
        assert_eq!(
            value.repr(),
            "{'a': 1, 'b': ['x', 2.5, None], 'c': [{'d': True}]}"
        );
    }

    #[test]
    fn a_doubling_graph_of_aliases_is_shared_not_copied() {
        // 26 collections, each holding the previous one twice: 2^25 leaves when expanded, 26 objects in Python
        let mut text = String::from("a: 1\nx0: &x0 [0]\n");
        for i in 1..=25 {
            text.push_str(&format!("x{i}: &x{i} [*x{}, *x{}]\n", i - 1, i - 1));
        }
        let value = load_text(&text).unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("not a dict")
        };
        assert_eq!(map.len(), 27);
        assert_eq!(map.get("a").unwrap().repr(), "1");
        // the same object, not a copy: one more wrapper level per step, never a deep copy
        // x25 is referred to once, x24 twice
        let PyValue::List(items) = map.get("x25").unwrap() else {
            panic!("x25 is a plain list")
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], PyValue::Shared(_)));
        let (PyValue::Shared(first), PyValue::Shared(second)) = (&items[0], &items[1]) else {
            panic!("both halves are shared")
        };
        assert!(std::sync::Arc::ptr_eq(first, second));
    }

    #[test]
    fn sharing_inside_a_cycle_stays_linear_and_keeps_its_markers() {
        let mut text = String::from("cyc: &r {x0: &x0 [*r]");
        for i in 1..=25 {
            text.push_str(&format!(", x{i}: &x{i} [*x{}, *x{}]", i - 1, i - 1));
        }
        text.push_str("}\nn: &n [*n, *n]\n");
        let value = load_text(&text).unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("not a dict")
        };
        assert_eq!(map.get("n").unwrap().repr(), "[[...], [...]]");
        assert!(map.get("cyc").unwrap().dumps_pretty().is_err());
    }

    /// How far up the container a cycle marker stands for sits (`Opaque.cycle_up`), counted in container levels (an `!!omap` entry is a
    /// tuple, so its key and value sit two levels below the list).
    fn marker_up(value: &PyValue) -> Option<usize> {
        match value.peel() {
            PyValue::Opaque(opaque) => opaque.cycle_up(),
            _ => None,
        }
    }

    fn only_item(value: &PyValue) -> &PyValue {
        match value.peel() {
            PyValue::List(items) | PyValue::Tuple(items) => &items[0],
            PyValue::Dict(map) => map.iter().next().map(|(_, v)| v).unwrap(),
            other => panic!("not a container: {other:?}"),
        }
    }

    #[test]
    fn a_cycle_marker_knows_how_many_container_levels_up_its_target_is() {
        // `&a [*a]`: the marker's parent is the list itself
        let value = load_text("a: &a [*a]\n").unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("dict")
        };
        assert_eq!(marker_up(only_item(map.get("a").unwrap())), Some(1));
        // a dictionary that contains itself
        let value = load_text("d: &d {k: *d}\n").unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("dict")
        };
        assert_eq!(marker_up(only_item(map.get("d").unwrap())), Some(1));
        // two levels: `&x [[*x]]`
        let value = load_text("x: &x [[*x]]\n").unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("dict")
        };
        assert_eq!(
            marker_up(only_item(only_item(map.get("x").unwrap()))),
            Some(2)
        );
        // an `!!omap` entry is a tuple: key and value sit two levels below the list
        let value = load_text("o: &o !!omap [{k: *o}]\n").unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("dict")
        };
        let entry = only_item(map.get("o").unwrap());
        let PyValue::Tuple(parts) = entry.peel() else {
            panic!("an omap entry is a tuple")
        };
        assert_eq!(marker_up(&parts[1]), Some(2));
    }

    #[test]
    fn a_shared_node_with_a_marker_to_an_outer_container_is_rebuilt_where_the_distance_differs() {
        // t = [s, [s]] and s = [t]: `s` sits at depth 1 and at depth 2 of `t`, so its marker is 2 levels up in the first place and 3 in the second
        let value = load_text("t: &t [&s [*t], [*s]]\n").unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("dict")
        };
        let PyValue::List(items) = map.get("t").unwrap().peel() else {
            panic!("list")
        };
        assert_eq!(marker_up(only_item(&items[0])), Some(2));
        assert_eq!(marker_up(only_item(only_item(&items[1]))), Some(3));
    }

    #[test]
    fn the_hanging_input_is_an_ordinary_parser_error() {
        assert_eq!(
            message("gpu_model: [}\n"),
            "ParserError: while parsing a flow node\nexpected the node content, but found '}'\n  in \"m.yaml\", line 1, column 13"
        );
    }

    #[test]
    fn tabs_are_scanner_errors() {
        assert_eq!(
            message("a:\n\t- b\n"),
            "ScannerError: while scanning for the next token\nfound character '\\t' that cannot start any token\n  in \"m.yaml\", line 2, column 1"
        );
    }

    #[test]
    fn anchors_aliases_and_merge_keys() {
        let value = load_text("base: &b {x: 1, y: 2}\nuse:\n  <<: *b\n  y: 3\n").unwrap();
        assert_eq!(
            value.repr(),
            "{'base': {'x': 1, 'y': 2}, 'use': {'x': 1, 'y': 3}}"
        );
    }

    #[test]
    fn block_and_multiline_scalars() {
        let value = load_text("a: |\n  l1\n  l2\nb: >-\n  x\n  y\nc: first\n  second\n").unwrap();
        assert_eq!(
            value.repr(),
            "{'a': 'l1\\nl2\\n', 'b': 'x y', 'c': 'first second'}"
        );
    }

    #[test]
    fn resolution_follows_yaml_one_one() {
        let value = load_text(
            "a: 1e5\nb: 1.5e+3\nc: 010\nd: 1_000\ne: 1:30\nf: Off\ng: .inf\nh: 0x1F\ni: ~\n",
        )
        .unwrap();
        assert_eq!(
            value.repr(),
            "{'a': '1e5', 'b': 1500.0, 'c': 8, 'd': 1000, 'e': 90, 'f': False, 'g': inf, 'h': 31, 'i': None}"
        );
    }

    #[test]
    fn several_documents_are_refused_like_pyyaml() {
        assert!(message("a: 1\n---\nb: 2\n")
            .starts_with("ComposerError: expected a single document in the stream"));
    }

    #[test]
    fn undefined_tags_and_aliases() {
        assert!(message("a: !foo 1\n")
            .starts_with("ConstructorError: could not determine a constructor for the tag '!foo'"));
        assert!(message("a: *x\n").starts_with("ComposerError: found undefined alias 'x'"));
    }

    #[test]
    fn the_reader_error_and_the_decoding_error_carry_their_constructor_fields() {
        let name = PyStr::new("m.yaml");
        let error = safe_load(b"a: \x00\n", &name).unwrap_err();
        assert_eq!(error.class, "ReaderError");
        assert_eq!(
            error.detail,
            YamlDetail::Reader {
                name: PyStr::new("m.yaml"),
                position: 3,
                character: 0,
                encoding: "unicode",
                reason: "special characters are not allowed",
            }
        );
        let error = safe_load(b"a\xff\n", &name).unwrap_err();
        assert_eq!(error.class, "UnicodeDecodeError");
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "'utf-8' codec can't decode byte 0xff in position 1: invalid start byte"
        );
        assert_eq!(
            bare(&error),
            YamlDetail::Decode {
                encoding: "utf-8",
                object: b"a\xff\n".to_vec(),
                start: 1,
                end: 2,
                reason: "invalid start byte",
                frames: None,
            }
        );
        // an incomplete sequence at the end of the file: the incremental decoder keeps the bytes until the end is known and then fails on
        // what it holds (positions are relative to that buffer, not to the file)
        let error = safe_load(b"ab\xe2\x82", &name).unwrap_err();
        assert_eq!(
            bare(&error),
            YamlDetail::Decode {
                encoding: "utf-8",
                object: b"\xe2\x82".to_vec(),
                start: 0,
                end: 2,
                reason: "unexpected end of data",
                frames: None,
            }
        );
        // the marked errors carry the state of `MarkedYAMLError(...)`, asserted in `marked_errors_expose_their_constructor_state_with_six_field_marks`
        assert!(matches!(
            safe_load(b"a: [\n", &name).unwrap_err().detail,
            YamlDetail::Marked(_)
        ));
    }

    /// The constructor state of an error without its mapped traceback (the traceback is asserted on its own).
    fn bare(error: &YamlError) -> YamlDetail {
        match error.detail.clone() {
            YamlDetail::Decode {
                encoding,
                object,
                start,
                end,
                reason,
                ..
            } => YamlDetail::Decode {
                encoding,
                object,
                start,
                end,
                reason,
                frames: None,
            },
            YamlDetail::Marked(mut detail) => {
                detail.frames = None;
                YamlDetail::Marked(detail)
            }
            other => other,
        }
    }

    fn mark(name: &str, index: usize, line: usize, column: usize) -> Mark6 {
        Mark6 {
            name: PyStr::new(name),
            index,
            line,
            column,
            buffer: None,
            pointer: None,
        }
    }

    fn chain(error: &YamlError) -> Vec<(&'static str, u32, &'static str)> {
        error
            .frames()
            .expect("a mapped traceback")
            .iter()
            .map(|site| (site.file, site.line, site.function))
            .collect()
    }

    #[test]
    fn a_parser_error_carries_the_source_mapped_traceback_of_the_frames_the_port_opened() {
        // `nodes: [` + newline: the flow sequence has no content; every entry is (file, line executing, function) of PyYAML 6.0.1, outermost first
        let error = safe_load(b"nodes: [\n", &PyStr::new("m.yaml")).unwrap_err();
        assert_eq!(
            chain(&error),
            vec![
                ("yaml/__init__.py", 125, "safe_load"),
                ("yaml/__init__.py", 81, "load"),
                ("yaml/constructor.py", 49, "get_single_data"),
                ("yaml/composer.py", 36, "get_single_node"),
                ("yaml/composer.py", 55, "compose_document"),
                ("yaml/composer.py", 84, "compose_node"),
                ("yaml/composer.py", 133, "compose_mapping_node"),
                ("yaml/composer.py", 82, "compose_node"),
                ("yaml/composer.py", 110, "compose_sequence_node"),
                ("yaml/parser.py", 98, "check_event"),
                ("yaml/parser.py", 474, "parse_flow_sequence_first_entry"),
                ("yaml/parser.py", 495, "parse_flow_sequence_entry"),
                ("yaml/parser.py", 268, "parse_flow_node"),
                ("yaml/parser.py", 369, "parse_node"),
            ]
        );
    }

    #[test]
    fn a_decoding_error_of_the_first_chunk_is_traced_through_the_loader_and_the_reader() {
        let error = safe_load(b"\xff", &PyStr::new("m.yaml")).unwrap_err();
        assert_eq!(error.class, "UnicodeDecodeError");
        assert_eq!(
            chain(&error),
            vec![
                ("yaml/__init__.py", 125, "safe_load"),
                ("yaml/__init__.py", 79, "load"),
                ("yaml/loader.py", 34, "__init__"),
                ("yaml/reader.py", 85, "__init__"),
                ("yaml/reader.py", 124, "determine_encoding"),
                ("yaml/reader.py", 178, "update_raw"),
                ("<frozen codecs>", 322, "decode"),
            ]
        );
    }

    #[test]
    fn every_parser_raise_site_maps_to_its_own_function_and_unmapped_domains_say_so() {
        let name = PyStr::new("m.yaml");
        // (document, the function whose raise it is, the raise line): the last frame is that function at that line
        let cases: [(&[u8], &str, u32); 6] = [
            (b"a: 1\n- b\n", "parse_block_mapping_key", 438),
            (b"- a\nb: 1\n", "parse_block_sequence_entry", 392),
            (b"{a: 1 b: 2}\n", "parse_flow_mapping_key", 549),
            (b"[{a: 1} b]\n", "parse_flow_sequence_entry", 483),
            (b"%YAML 1.1\n%YAML 1.1\n--- a\n", "process_directives", 224),
            (b"!x!y a\n", "parse_node", 305),
        ];
        for (document, function, line) in cases {
            let error = safe_load(document, &name).unwrap_err();
            assert_eq!(error.class, "ParserError", "{document:?}");
            let last = chain(&error).pop().unwrap();
            assert_eq!((last.2, last.1), (function, line), "{document:?}");
        }
        // a scanner or constructor error has no mapped chain: never a guess
        assert!(safe_load(b"a: @\n", &name).unwrap_err().frames().is_none());
        assert!(safe_load(b"a: !foo 1\n", &name)
            .unwrap_err()
            .frames()
            .is_none());
    }

    #[test]
    fn marked_errors_expose_their_constructor_state_with_six_field_marks() {
        let name = PyStr::new("m.yaml");
        // ParserError: context, both marks (the reader's character index, zero-based line and column), no note, buffer and pointer None for a file
        let error = safe_load(b"nodes: [\n", &name).unwrap_err();
        assert_eq!(error.class, "ParserError");
        assert_eq!(
            bare(&error),
            YamlDetail::Marked(Box::new(MarkedDetail {
                context: Some(PyStr::new("while parsing a flow node")),
                context_mark: Some(mark("m.yaml", 9, 1, 0)),
                problem: Some(PyStr::new(
                    "expected the node content, but found '<stream end>'"
                )),
                problem_mark: Some(mark("m.yaml", 9, 1, 0)),
                note: None,
                shared_marks: true,
                frames: None,
            }))
        );
        // the text of the error is unchanged by the extra data
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "while parsing a flow node\nexpected the node content, but found '<stream end>'\n  in \"m.yaml\", line 2, column 1"
        );
        // ScannerError: a context but no context mark
        let error = safe_load(b"a: @\n", &name).unwrap_err();
        assert_eq!(error.class, "ScannerError");
        assert_eq!(
            bare(&error),
            YamlDetail::Marked(Box::new(MarkedDetail {
                context: Some(PyStr::new("while scanning for the next token")),
                context_mark: None,
                problem: Some(PyStr::new(
                    "found character '@' that cannot start any token"
                )),
                problem_mark: Some(mark("m.yaml", 3, 0, 3)),
                note: None,
                shared_marks: false,
                frames: None,
            }))
        );
        // ComposerError and ConstructorError: no context
        let error = safe_load(b"a: *x\n", &name).unwrap_err();
        assert_eq!(error.class, "ComposerError");
        let YamlDetail::Marked(detail) = error.detail else {
            panic!("marked detail")
        };
        let MarkedDetail {
            context,
            problem,
            problem_mark,
            ..
        } = *detail;
        assert_eq!(context, None);
        assert_eq!(problem, Some(PyStr::new("found undefined alias 'x'")));
        assert_eq!(problem_mark, Some(mark("m.yaml", 3, 0, 3)));
        let error = safe_load(b"a: !foo 1\n", &name).unwrap_err();
        assert_eq!(error.class, "ConstructorError");
        assert!(matches!(
            error.detail,
            YamlDetail::Marked(ref detail) if matches!(detail.problem_mark, Some(Mark6 { line: 0, .. }))
        ));
    }

    #[test]
    fn every_pull_of_the_reader_is_reported_in_order_whether_the_load_went_on_or_failed() {
        let name = PyStr::new("m.yaml");
        let (result, reads) = safe_load_observed(b"a: 1\n", &name);
        assert!(result.is_ok());
        assert_eq!(
            reads,
            vec![
                ReadEvent::Read { size: 4096 },
                ReadEvent::Return {
                    text: PyStr::new("a: 1\n")
                },
                ReadEvent::Read { size: 4096 },
                ReadEvent::Return {
                    text: PyStr::new("")
                },
            ]
        );
        let (result, reads) = safe_load_observed(b"a\xff\n", &name);
        assert_eq!(result.unwrap_err().class, "UnicodeDecodeError");
        assert_eq!(reads.len(), 2);
        assert_eq!(reads[0], ReadEvent::Read { size: 4096 });
        assert!(matches!(
            &reads[1],
            ReadEvent::Error {
                class: "UnicodeDecodeError",
                ..
            }
        ));
    }

    #[test]
    fn yaml_dates_datetimes_sets_and_non_string_keys_keep_their_structured_data() {
        use crate::pyvalue::Typed;
        let value = load_text(
            "d: 2026-01-01\nt: 2001-12-14 21:59:43.10 -5\nu: 2001-12-14T21:59:43Z\ns: !!set {b, a}\n2026-02-03: k\n",
        )
        .unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("not a dict")
        };
        let typed = |key: &str| match map.get(key).unwrap() {
            PyValue::Opaque(opaque) => opaque.typed.clone(),
            other => panic!("{other:?}"),
        };
        assert!(matches!(typed("d"), Typed::Date(iso) if iso == "2026-01-01"));
        // a negative offset is a valid `datetime.timezone`: -05:00 is `timedelta(days=-1, seconds=68400)`
        assert!(
            matches!(typed("t"), Typed::DateTime(iso) if iso == "2001-12-14T21:59:43.100000-05:00")
        );
        assert!(matches!(typed("u"), Typed::DateTime(iso) if iso == "2001-12-14T21:59:43+00:00"));
        let Typed::Set(members) = typed("s") else {
            panic!("a set")
        };
        let mut shown: Vec<String> = members.iter().map(PyValue::repr).collect();
        shown.sort();
        assert_eq!(shown, ["'a'", "'b'"]);
        let key = map
            .keyed_entries()
            .find_map(|(other, _, _)| other.cloned())
            .expect("a non-string key");
        let Some(kept) = key.value else {
            panic!("the key keeps its value")
        };
        assert!(
            matches!(kept.peel(), PyValue::Opaque(opaque) if matches!(&opaque.typed, Typed::Date(iso) if iso == "2026-02-03"))
        );
        // an offset of a day or more is still the original's ValueError
        assert!(message("a: 2001-12-14 21:59:43 +24:00\n")
            .starts_with("ValueError: offset must be a timedelta strictly between"));
    }

    #[test]
    fn a_set_holds_one_of_the_values_python_treats_as_equal() {
        use crate::pyvalue::Typed;
        let value = load_text("s: !!set {1, true, 1.0, a, x}\n").unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("not a dict")
        };
        let PyValue::Opaque(opaque) = map.get("s").unwrap() else {
            panic!("a set")
        };
        let Typed::Set(members) = &opaque.typed else {
            panic!("a set")
        };
        let shown: Vec<String> = members.iter().map(PyValue::repr).collect();
        // `1`, `True` and `1.0` are one element (the first inserted stays)
        assert_eq!(shown, ["1", "'a'", "'x'"]);
    }
    /// The expected values of the next three tests are PyYAML 6.0.3's own results for the same documents (CPython 3.12 `datetime`), taken
    /// with the independent oracle script of the worker checks; `constructor.py` 132-144 builds the mapping (the first key object stays,
    /// the last value wins) and `set(mapping)` for a `!!set`.
    fn entries_of(text: &str) -> Vec<(String, String)> {
        let value = load_text(text).unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("not a dict")
        };
        map.keyed_entries()
            .map(|(_, key, value)| (key.to_utf8().unwrap(), value.repr()))
            .collect()
    }

    #[test]
    fn timestamp_keys_that_are_the_same_instant_are_one_key_and_the_first_object_stays() {
        // the same UTC instant written in two offsets: one key (the first object, UTC), the last value
        assert_eq!(
            entries_of("? 2001-12-14T21:59:43Z\n: a\n? 2001-12-14T22:59:43+01:00\n: b\n"),
            [(
                "datetime.datetime(2001, 12, 14, 21, 59, 43, tzinfo=datetime.timezone.utc)"
                    .to_string(),
                "'b'".to_string()
            )]
        );
        // a negative offset too; naive values compare by their fields and never equal an aware one; a date is not a midnight datetime
        assert_eq!(
            entries_of(
                "? 2001-12-14T21:59:43Z\n: a\n? 2001-12-14T16:59:43-05:00\n: b\n? 2001-12-14T16:59:43\n: c\n? 2001-12-14T16:59:43\n: d\n? 2001-12-14\n: e\n? 2001-12-14T00:00:00\n: f\n"
            ),
            [
                (
                    "datetime.datetime(2001, 12, 14, 21, 59, 43, tzinfo=datetime.timezone.utc)".to_string(),
                    "'b'".to_string()
                ),
                ("datetime.datetime(2001, 12, 14, 16, 59, 43)".to_string(), "'d'".to_string()),
                ("datetime.date(2001, 12, 14)".to_string(), "'e'".to_string()),
                ("datetime.datetime(2001, 12, 14, 0, 0)".to_string(), "'f'".to_string()),
            ]
        );
        // two instants one microsecond apart stay two keys
        assert_eq!(
            entries_of("? 2001-12-14T21:59:43.000001Z\n: a\n? 2001-12-14T21:59:43Z\n: b\n").len(),
            2
        );
    }

    #[test]
    fn a_set_of_timestamps_keeps_one_member_per_python_equality_in_first_order() {
        use crate::pyvalue::Typed;
        let value = load_text(
            "s: !!set\n  ? 2001-12-14T21:59:43Z\n  ? 2001-12-14T22:59:43+01:00\n  ? 2001-12-14T16:59:43\n  ? 2001-12-14T16:59:43.000000\n  ? 2001-12-14\n",
        )
        .unwrap();
        let PyValue::Dict(map) = &value else {
            panic!("not a dict")
        };
        let PyValue::Opaque(opaque) = map.get("s").unwrap() else {
            panic!("a set")
        };
        let Typed::Set(members) = &opaque.typed else {
            panic!("a set")
        };
        let shown: Vec<String> = members.iter().map(PyValue::repr).collect();
        assert_eq!(
            shown,
            [
                "datetime.datetime(2001, 12, 14, 21, 59, 43, tzinfo=datetime.timezone.utc)",
                "datetime.datetime(2001, 12, 14, 16, 59, 43)",
                "datetime.date(2001, 12, 14)"
            ]
        );
        assert_eq!(
            opaque.repr,
            "{datetime.datetime(2001, 12, 14, 21, 59, 43, tzinfo=datetime.timezone.utc), datetime.datetime(2001, 12, 14, 16, 59, 43), datetime.date(2001, 12, 14)}"
        );
    }

    #[test]
    fn a_datetime_repr_leaves_out_a_zero_microsecond_and_then_a_zero_second_like_cpython() {
        let reprs = entries_of(
            "a: {x: 2001-12-14T21:59:00}\nb: {x: 2001-12-14T21:59:01}\nc: {x: 2001-12-14T21:59:00.5}\nd: {x: 2001-12-14T21:59:00Z}\ne: {x: 2001-12-14T00:00:00+05:30}\nf: {x: 2001-12-14T21:00:00.000001-03:30}\n",
        );
        // `repr` of a container holds the `repr` of its elements: the value `repr` is that of the dict
        let want = [
            "{'x': datetime.datetime(2001, 12, 14, 21, 59)}",
            "{'x': datetime.datetime(2001, 12, 14, 21, 59, 1)}",
            "{'x': datetime.datetime(2001, 12, 14, 21, 59, 0, 500000)}",
            "{'x': datetime.datetime(2001, 12, 14, 21, 59, tzinfo=datetime.timezone.utc)}",
            "{'x': datetime.datetime(2001, 12, 14, 0, 0, tzinfo=datetime.timezone(datetime.timedelta(seconds=19800)))}",
            "{'x': datetime.datetime(2001, 12, 14, 21, 0, 0, 1, tzinfo=datetime.timezone(datetime.timedelta(days=-1, seconds=73800)))}",
        ];
        let shown: Vec<&str> = reprs.iter().map(|(_, repr)| repr.as_str()).collect();
        assert_eq!(shown, want);
    }
    /// A signed zero offset is the singleton UTC time zone, as `Z` is: `timezone.__new__` returns `cls.utc` for no name and no offset.
    /// Non-zero and negative offsets, a naive value and the identity of equal instants are unchanged.
    #[test]
    fn a_signed_zero_offset_prints_as_the_utc_singleton_and_is_the_same_instant_as_z() {
        let reprs = entries_of(
            "a: {x: 2001-12-14T21:59:00+00:00}\nb: {x: 2001-12-14T21:59:00-00:00}\nc: {x: 2001-12-14 21:59:00 +0}\nd: {x: 2001-12-14 21:59:00 -0}\ne: {x: 2001-12-14T21:59:00Z}\nf: {x: 2001-12-14T21:59:00+00:01}\ng: {x: 2001-12-14T21:59:00-00:01}\nh: {x: 2001-12-14T21:59:00}\n",
        );
        let shown: Vec<&str> = reprs.iter().map(|(_, repr)| repr.as_str()).collect();
        let utc = "{'x': datetime.datetime(2001, 12, 14, 21, 59, tzinfo=datetime.timezone.utc)}";
        assert_eq!(shown[..5], [utc; 5]);
        assert_eq!(
            shown[5],
            "{'x': datetime.datetime(2001, 12, 14, 21, 59, tzinfo=datetime.timezone(datetime.timedelta(seconds=60)))}"
        );
        assert_eq!(
            shown[6],
            "{'x': datetime.datetime(2001, 12, 14, 21, 59, tzinfo=datetime.timezone(datetime.timedelta(days=-1, seconds=86340)))}"
        );
        assert_eq!(shown[7], "{'x': datetime.datetime(2001, 12, 14, 21, 59)}");
        // the typed value and the identity: `+00:00`, `-00:00` and `Z` are one key
        assert_eq!(
            entries_of("? 2001-12-14T21:59:00Z\n: a\n? 2001-12-14T21:59:00+00:00\n: b\n? 2001-12-14T21:59:00-00:00\n: c\n"),
            [(
                "datetime.datetime(2001, 12, 14, 21, 59, tzinfo=datetime.timezone.utc)".to_string(),
                "'c'".to_string()
            )]
        );
    }

    fn list_v(value: &PyValue) -> Vec<PyValue> {
        match value.peel() {
            PyValue::Dict(map) => match map.get("v").map(PyValue::peel) {
                Some(PyValue::List(items)) => items.clone(),
                other => panic!("a list under v, got {other:?}"),
            },
            other => panic!("a mapping, got {other:?}"),
        }
    }

    #[test]
    fn the_objects_loader_gives_nan_identity_and_safe_load_keeps_plain_floats() {
        use crate::pyvalue::same_nan_object;
        let (bytes, name) = (
            "v: [.nan, .nan, &n !!float nan, *n, !!float nan]\n".as_bytes(),
            PyStr::new("m.yaml"),
        );
        let plain = list_v(&safe_load(bytes, &name).unwrap());
        assert!(
            plain.iter().all(|item| !same_nan_object(item, item)),
            "safe_load: no object identity"
        );
        let objects = list_v(&safe_load_objects(bytes, &name).unwrap());
        // `.nan` is one shared object, an anchor and its alias are one object, a separate `!!float nan` is another
        assert!(same_nan_object(&objects[0], &objects[1]));
        assert!(same_nan_object(&objects[2], &objects[3]));
        assert!(!same_nan_object(&objects[0], &objects[2]));
        assert!(!same_nan_object(&objects[2], &objects[4]));
        assert!(!same_nan_object(&objects[0], &objects[4]));
    }

    #[test]
    fn the_objects_loader_answers_like_safe_load_everywhere_but_nan_identity() {
        let name = PyStr::new("m.yaml");
        for doc in [
            "a: 1\nb: [x, 2.5, null, 2001-12-14]\nc: !!binary aGVsbG8=\n",
            "a: [1, 2\n",
            "a: !!python/object/apply:os.system [x]\n",
            "a:\n\t- b\n",
            "v: [.nan, !!float nan, .inf]\n",
        ] {
            match (
                safe_load(doc.as_bytes(), &name),
                safe_load_objects(doc.as_bytes(), &name),
            ) {
                (Ok(plain), Ok(objects)) => assert_eq!(plain.repr(), objects.repr(), "{doc:?}"),
                (Err(plain), Err(objects)) => assert_eq!(plain, objects, "{doc:?}"),
                (plain, objects) => panic!("{doc:?}: {plain:?} vs {objects:?}"),
            }
        }
    }
}
