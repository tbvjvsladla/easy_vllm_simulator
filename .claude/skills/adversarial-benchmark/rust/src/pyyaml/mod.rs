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
mod frames;
mod parser;
mod reader;
mod scanner;

use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use std::rc::Rc;

/// An exception `safe_load` raised: its Python class name and `str(exception)`.
#[derive(Debug, Clone, PartialEq)]
pub struct YamlError {
    pub class: &'static str,
    pub message: PyStr,
}

/// A position in the stream (`yaml.error.Mark` of a stream source: file name, line and column, no snippet).
#[derive(Clone, Debug)]
pub(crate) struct Mark {
    pub name: Rc<PyStr>,
    pub line: usize,
    pub column: usize,
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
        context_mark: Option<Mark>,
        problem: Option<String>,
        problem_mark: Option<Mark>,
    },
    /// `ReaderError` for a character YAML does not allow.
    Reader {
        name: Rc<PyStr>,
        position: usize,
        character: u32,
        reason: &'static str,
    },
    /// Any other exception (`UnicodeDecodeError`, `ValueError`, `KeyError`, ...).
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
    Failure::Marked {
        class,
        context: context.map(str::to_string),
        context_mark: context_mark.cloned(),
        problem: Some(problem.into()),
        problem_mark: problem_mark.cloned(),
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
    fn into_error(self) -> YamlError {
        match self {
            Failure::Plain { class, message } => YamlError { class, message },
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
                YamlError { class, message }
            }
        }
    }
}

/// `yaml.safe_load(open(name, encoding="utf-8"))` given the file's bytes and the name Python would show in marks,
/// under the recursion limit the original CLI runs with.
pub fn safe_load(bytes: &[u8], name: &PyStr) -> Result<PyValue, YamlError> {
    frames::reset(frames::LIMIT, false);
    load(Source::File { bytes, name }).map_err(Failure::into_error)
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
    load(Source::Text(text)).map_err(Failure::into_error)
}

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
    let result = load(Source::File { bytes, name }).map_err(Failure::into_error);
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

fn load(source: Source<'_>) -> Parsed<PyValue> {
    let _safe_load = frames::frame("safe_load")?;
    let _load = frames::frame("load")?;
    let name = Rc::new(match &source {
        Source::File { name, .. } => (*name).clone(),
        Source::Text(_) => PyStr::new(TEXT_SOURCE_NAME),
    });
    // `Loader.__init__` builds the reader (`Reader.__init__` -> `determine_encoding`) and then the scanner
    let loader = frames::frame("__init__")?;
    let reader = {
        let _init = frames::frame("__init__")?;
        match &source {
            Source::File { bytes, .. } => {
                let _determine = frames::frame("determine_encoding")?;
                reader::Reader::new(name, bytes)?
            }
            Source::Text(text) => reader::Reader::from_text(name, text)?,
        }
    };
    let scanner = {
        let _init = frames::frame("__init__")?;
        scanner::Scanner::new(reader)?
    };
    drop(loader);
    let mut parser = parser::Parser::new(scanner);
    frames::forget_trace();
    let _get_single_data = frames::frame("get_single_data")?;
    let composed = compose::get_single_node(&mut parser)?;
    compose::construct_document(composed)
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
}
