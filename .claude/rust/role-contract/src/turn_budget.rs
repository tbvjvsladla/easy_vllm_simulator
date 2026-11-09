//! `turn_budget.py` as a pure library: the budget **declaration validator** (it owns no value; the request schema owns the caps).
//!
//! The original has two functions of interest - `schema_cap(field, path)` (reads the cap of one field from the agent-control request
//! schema, on every call: there is no cache and no caps constant) and `declare(max_turns, timeout_seconds, *, source, caps, unit,
//! max_dispatches)` (validates a declared budget and returns it) - plus its self-test. This module is their business, over
//! Python-compatible values ([`PyValue`]) so that the callers' arbitrary arguments keep Python's behaviour: the truthiness of
//! `caps or {...}`, `bool` being an `int` (a schema `maximum` of `true` is a cap), `isinstance` checks, `%r`/`str` of any value,
//! the `TypeError`/`KeyError` of `v > caps[k]`, and the validation order (unit, then timeout, then max_turns, then source).
//!
//! **What is not here.** No argument parser, no console, no process: the command line lives in the `sweep-stop` package
//! (`turn_budget_cli`), the typed transport for in-process importers in its private bridge. Errors are returned as [`Raised`]
//! (class, constructor arguments, context, and the logical Python frames below the caller) and rendered by those callers.
//!
//! **Frames.** [`Raised::frames`] lists the frames from the outermost frame this module knows (the `declare` or `schema_cap`
//! frame) down to the stdlib frame that raised; frames of the owner file carry an empty `file` and the caller fills in the owner
//! path (`__file__` of the process that is being stood for). Rows marked *observed* below were read from the saved original
//! raw (R49C07..C12, C15); the remaining stdlib lines (`json/__init__.py` 335, `json/decoder.py` 340 and 353) are the 3.12.3
//! source lines of the same functions and are *derived*, not observed (see `PORTING.md`).

use crate::pyprofile::fs_encode;
use crate::topology::{abspath, Frame};
use agent_runtime_support::pyjson::{decode_file_text, loads_text, utf8_error_parts, LoadError};
use agent_runtime_support::pyops::{compare, Compare};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;

/// The segments `REQUEST_SCHEMA = os.path.join(REPO, ".claude", "schemas", "agent-control-request.schema.json")` joins.
pub const REQUEST_SCHEMA_SEGMENTS: [&str; 3] =
    [".claude", "schemas", "agent-control-request.schema.json"];
/// Directory levels between the owner file and `REPO` (`dirname`, then four `..`).
pub const REPO_LEVELS_ABOVE_OWNER: usize = 5;
/// `sys.get_int_max_str_digits()` default.
const MAX_INT_DIGITS: usize = 4300;
/// The text every refusal starts with.
pub const FAIL_PREFIX: &str = "[turn-budget] FAIL: ";
pub const UNITS: [&str; 2] = ["native_turns", "wall_time"];
pub const EXIT_SELF_TEST_FAILED: i32 = 2;
/// The names the self-test's tripwire looks for in the module namespace, in its order (the revived grade table and its
/// escalation vocabulary). This program owns the list, its order and the labels.
pub const REVIVABLE_NAMES: [&str; 7] = [
    "GRADES",
    "GRADE_ORDER",
    "MIN_TURNS",
    "ESCALATION_FACTOR", // antipattern-ok: G-A2-grade-table — the tripwire list itself (the vocabulary must be written for the check to exist)
    "MAX_ATTEMPTS_BEFORE_HITL", // antipattern-ok: G-A2-grade-table — same
    "budget",
    "escalate",
];
/// The public names of this program standing alone (it has no Python module namespace): the facts the tripwire reads when a
/// Python importer has not marshalled its own module's names.
pub const STANDALONE_GLOBALS: [&str; 7] = [
    "REPO",
    "REQUEST_SCHEMA",
    "schema_cap",
    "schema_max_turns",
    "declare",
    "_self_test",
    "main",
];

/// `[n for n in REVIVABLE_NAMES if n in globals()]` over the given module-namespace names.
pub fn revived_names(module_names: &[PyStr]) -> Vec<PyStr> {
    REVIVABLE_NAMES
        .iter()
        .filter(|name| module_names.iter().any(|known| known.equals(name)))
        .map(|name| PyStr::new(name))
        .collect()
}

// ── the owner's own source lines that appear in a traceback ──────────────────────────────

fn owner(line: u32, function: &'static str, code: &'static str, expr: &'static str) -> Frame {
    Frame {
        file: Vec::new(),
        line,
        function,
        code,
        expr,
    }
}

fn stdlib(
    file: &str,
    line: u32,
    function: &'static str,
    code: &'static str,
    expr: &'static str,
) -> Frame {
    Frame {
        file: file.as_bytes().to_vec(),
        line,
        function,
        code,
        expr,
    }
}

const JSON_INIT: &str = "/usr/lib/python3.12/json/__init__.py";
const JSON_DECODER: &str = "/usr/lib/python3.12/json/decoder.py";

/// `schema_cap` line 39: `with open(path, encoding="utf-8") as f:` (observed).
fn frame_open() -> Frame {
    owner(
        39,
        "schema_cap",
        "with open(path, encoding=\"utf-8\") as f:",
        "open(path, encoding=\"utf-8\")",
    )
}

/// `schema_cap` line 40: `doc = json.load(f)` (observed).
fn frame_load() -> Frame {
    owner(40, "schema_cap", "doc = json.load(f)", "json.load(f)")
}

fn frame_doc_get() -> Frame {
    owner(
        41,
        "schema_cap",
        "node = (doc.get(\"properties\") or {}).get(field) or {}",
        "doc.get",
    )
}

fn frame_props_get() -> Frame {
    owner(
        41,
        "schema_cap",
        "node = (doc.get(\"properties\") or {}).get(field) or {}",
        "(doc.get(\"properties\") or {}).get",
    )
}

fn frame_field_hash() -> Frame {
    owner(
        41,
        "schema_cap",
        "node = (doc.get(\"properties\") or {}).get(field) or {}",
        "(doc.get(\"properties\") or {}).get(field)",
    )
}

/// `schema_cap` line 42: `cap = node.get("maximum")` (observed).
fn frame_node_get() -> Frame {
    owner(42, "schema_cap", "cap = node.get(\"maximum\")", "node.get")
}

fn frames_json_load() -> Vec<Frame> {
    vec![
        frame_load(),
        stdlib(
            JSON_INIT,
            293,
            "load",
            "return loads(fp.read(),",
            "loads(fp.read(),",
        ),
    ]
}

/// What the stdlib frames below `json.load` are, by where the decoder raised.
enum JsonSite {
    Bom,
    ExpectingValue,
    ExtraData,
    Scanner,
}

fn frames_json(site: JsonSite) -> Vec<Frame> {
    let mut frames = frames_json_load();
    if let JsonSite::Bom = site {
        frames.push(stdlib(
            JSON_INIT,
            335,
            "loads",
            "raise JSONDecodeError(\"Unexpected UTF-8 BOM (decode using utf-8-sig)\",",
            "",
        ));
        return frames;
    }
    frames.push(stdlib(
        JSON_INIT,
        346,
        "loads",
        "return _default_decoder.decode(s)",
        "_default_decoder.decode(s)",
    ));
    if let JsonSite::ExtraData = site {
        frames.push(stdlib(
            JSON_DECODER,
            340,
            "decode",
            "raise JSONDecodeError(\"Extra data\", s, end)",
            "",
        ));
        return frames;
    }
    frames.push(stdlib(
        JSON_DECODER,
        337,
        "decode",
        "obj, end = self.raw_decode(s, idx=_w(s, 0).end())",
        "self.raw_decode(s, idx=_w(s, 0).end())",
    ));
    frames.push(match site {
        JsonSite::ExpectingValue => stdlib(
            JSON_DECODER,
            355,
            "raw_decode",
            "raise JSONDecodeError(\"Expecting value\", s, err.value) from None",
            "",
        ),
        _ => stdlib(
            JSON_DECODER,
            353,
            "raw_decode",
            "obj, end = self.scan_once(s, idx)",
            "self.scan_once(s, idx)",
        ),
    });
    frames
}

// ── what a caller sees ───────────────────────────────────────────────────────

/// An exception of the original: how to build the Python object, and what a traceback shows of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Raised {
    /// The builtin (or `JSONDecodeError`) class name.
    pub class: &'static str,
    /// Positional constructor arguments that build the exact object (`FileNotFoundError(2, strerror, filename)`).
    pub ctor: Vec<PyValue>,
    /// Keyword constructor arguments (`AttributeError(message, name=..., obj=...)`).
    pub ctor_kwargs: Box<Vec<(&'static str, PyValue)>>,
    /// `str(exception)` (a `SystemExit`'s message is the text the interpreter prints).
    pub message: PyStr,
    /// The exception whose handler raised this one, and whether `raise ... from None` hid it.
    pub context: Option<Box<Raised>>,
    pub suppress_context: bool,
    /// Frames from the outermost known one to the raising one (owner frames have an empty `file`).
    pub frames: Vec<Frame>,
}

impl Raised {
    fn new(class: &'static str, message: PyStr, frames: Vec<Frame>) -> Raised {
        Raised {
            class,
            ctor: vec![PyValue::Str(message.clone())],
            ctor_kwargs: Box::default(),
            message,
            context: None,
            suppress_context: false,
            frames,
        }
    }

    pub fn is_system_exit(&self) -> bool {
        self.class == "SystemExit"
    }

    /// Frames of a caller that sit above this exception's own frames.
    pub fn under(mut self, caller: Vec<Frame>) -> Raised {
        let mut frames = caller;
        frames.extend(self.frames);
        self.frames = frames;
        self
    }
}

/// How a call ended without a result.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// The original raises this.
    Raised(Box<Raised>),
    /// An argument shape this port does not carry (the original's behaviour is a file descriptor, a callable...): reported before
    /// any business, never answered with a guess.
    Unsupported(String),
}

impl From<Raised> for Failure {
    fn from(raised: Raised) -> Failure {
        Failure::Raised(Box::new(raised))
    }
}

fn exit(message: PyStr, frames: Vec<Frame>) -> Raised {
    Raised::new("SystemExit", message, frames)
}

fn fail_text(body: &str) -> PyStr {
    PyStr::new(&format!("{FAIL_PREFIX}{body}"))
}

fn join(parts: &[&PyStr]) -> PyStr {
    let mut out = PyStr::new("");
    for part in parts {
        out.push_py(part);
    }
    out
}

fn text(value: &str) -> PyStr {
    PyStr::new(value)
}

// ── paths ──────────────────────────────────────────────────────────────

/// The argument `path` of `schema_cap`: any Python object `open()` takes or refuses.
#[derive(Clone, Debug, PartialEq)]
pub enum PathValue {
    /// A plain object (`str`, `bytes`, `None`, ...).
    Value(PyValue),
    /// An `os.PathLike`: its `os.fspath()` (a `str` or `bytes` value) and its `str()` (what `f"{path}"` prints).
    Like { fspath: PyValue, text: PyStr },
}

impl PathValue {
    pub fn text_of(path: &PyStr) -> PathValue {
        PathValue::Value(PyValue::Str(path.clone()))
    }

    /// `f"{path}"`.
    fn display(&self) -> PyStr {
        match self {
            PathValue::Value(value) => value.display_text(),
            PathValue::Like { text, .. } => text.clone(),
        }
    }

    fn operand(&self) -> &PyValue {
        match self {
            PathValue::Value(value) => value,
            PathValue::Like { fspath, .. } => fspath,
        }
    }
}

/// `REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))`: lexical, no
/// symbolic link is resolved.
pub fn repo_of(owner_file: &[u8]) -> Vec<u8> {
    let mut path = owner_file.to_vec();
    for _ in 0..REPO_LEVELS_ABOVE_OWNER {
        path.extend_from_slice(b"/..");
    }
    abspath(&path)
}

/// `REQUEST_SCHEMA = os.path.join(REPO, ".claude", "schemas", ...)`.
pub fn request_schema_of(repo: &[u8]) -> Vec<u8> {
    let mut path = repo.to_vec();
    for segment in REQUEST_SCHEMA_SEGMENTS {
        if path.last() != Some(&b'/') {
            path.push(b'/');
        }
        path.extend_from_slice(segment.as_bytes());
    }
    path
}

/// `os.path.relpath(REQUEST_SCHEMA, REPO)`, which the `--caps` source line prints.
pub fn request_schema_relative() -> String {
    REQUEST_SCHEMA_SEGMENTS.join("/")
}

fn os_class(errno: i32) -> &'static str {
    match errno {
        1 | 13 => "PermissionError",
        2 => "FileNotFoundError",
        20 => "NotADirectoryError",
        21 => "IsADirectoryError",
        _ => "OSError",
    }
}

fn os_raised(error: &std::io::Error, filename: Option<&PyValue>, frames: Vec<Frame>) -> Raised {
    let errno = error.raw_os_error().unwrap_or(0);
    let shown = error.to_string();
    let strerror = shown
        .split(" (os error")
        .next()
        .unwrap_or(&shown)
        .to_string();
    let mut message = format!("[Errno {errno}] {strerror}");
    let mut ctor = vec![PyValue::int(errno), PyValue::str(&strerror)];
    if let Some(name) = filename {
        message.push_str(": ");
        message.push_str(&name.repr());
        ctor.push(name.clone());
    }
    Raised {
        class: os_class(errno),
        ctor,
        ctor_kwargs: Box::default(),
        message: PyStr::new(&message),
        context: None,
        suppress_context: false,
        frames,
    }
}

/// The operating-system path `open(path)` uses, or the exception `open` raises for an object that is not a path.
fn os_path(path: &PathValue) -> Result<Vec<u8>, Failure> {
    let operand = path.operand();
    let bytes = match operand.peel() {
        PyValue::Str(name) => match fs_encode(name) {
            Ok(bytes) => bytes,
            Err(failure) => {
                let mut raised = Raised::new(
                    "UnicodeEncodeError",
                    PyStr::new(&failure.message),
                    vec![frame_open()],
                );
                raised.ctor = vec![
                    PyValue::str(failure.encoding),
                    PyValue::Str(name.clone()),
                    PyValue::int(failure.start),
                    PyValue::int(failure.end),
                    PyValue::str(&failure.reason),
                ];
                return Err(raised.into());
            }
        },
        PyValue::Bytes(bytes) => bytes.clone(),
        // `open(int)` opens a file descriptor: a different operation, not carried.
        PyValue::Int(_) | PyValue::Bool(_) => {
            return Err(Failure::Unsupported(format!(
                "path of type {} is a file descriptor for open()",
                operand.type_name()
            )))
        }
        other => {
            return Err(Raised::new(
                "TypeError",
                text(&format!(
                    "expected str, bytes or os.PathLike object, not {}",
                    other.type_name()
                )),
                vec![frame_open()],
            )
            .into())
        }
    };
    if bytes.contains(&0) {
        return Err(
            Raised::new("ValueError", text("embedded null byte"), vec![frame_open()]).into(),
        );
    }
    Ok(bytes)
}

/// `open(path, encoding="utf-8")` then `json.load`: the decoded document.
fn load_document(path: &PathValue) -> Result<PyValue, Failure> {
    let os = os_path(path)?;
    let filename = path.operand().clone();
    let mut file = std::fs::File::open(std::ffi::OsStr::from_bytes(&os))
        .map_err(|error| os_raised(&error, Some(&filename), vec![frame_open()]))?;
    let mut bytes = Vec::new();
    if let Err(error) = file.read_to_end(&mut bytes) {
        // `open` of a directory fails in `open` itself with the file name; any other read failure has none.
        let named = if error.raw_os_error() == Some(21) {
            Some(&filename)
        } else {
            None
        };
        let frames = if named.is_some() {
            vec![frame_open()]
        } else {
            frames_json_load()
        };
        return Err(os_raised(&error, named, frames).into());
    }
    let document = match decode_file_text(&bytes) {
        Ok(document) => document,
        Err(LoadError::Encoding(message)) => {
            let error = std::str::from_utf8(&bytes).expect_err("the decoder refused these bytes");
            let (start, length, reason) = utf8_error_parts(&bytes, &error);
            let mut frames = frames_json_load();
            frames.push(stdlib("<frozen codecs>", 322, "decode", "", ""));
            let mut raised = Raised::new("UnicodeDecodeError", PyStr::new(&message), frames);
            raised.ctor = vec![
                PyValue::str("utf-8"),
                PyValue::Bytes(bytes.clone()),
                PyValue::int(start),
                PyValue::int(start + length),
                PyValue::str(reason),
            ];
            return Err(raised.into());
        }
        Err(other) => return Err(json_raised(&other, &PyStr::new("")).into()),
    };
    loads_text(&document).map_err(|error| Failure::from(json_raised(&error, &document)))
}

/// The exception `json.loads` raises for a document the reader refused (`doc` is the text it read).
fn json_raised(error: &LoadError, doc: &PyStr) -> Raised {
    match error {
        LoadError::Decode {
            message,
            line,
            column,
            position,
        } => {
            let shown = format!("{message}: line {line} column {column} (char {position})");
            let site = if message.starts_with("Unexpected UTF-8 BOM") {
                JsonSite::Bom
            } else if *message == "Expecting value" {
                JsonSite::ExpectingValue
            } else if *message == "Extra data" {
                JsonSite::ExtraData
            } else {
                JsonSite::Scanner
            };
            let hides_stop = matches!(site, JsonSite::ExpectingValue);
            let mut raised = Raised::new("JSONDecodeError", PyStr::new(&shown), frames_json(site));
            raised.ctor = vec![
                PyValue::str(message),
                PyValue::Str(doc.clone()),
                PyValue::int(*position),
            ];
            if hides_stop {
                let mut stop =
                    Raised::new("StopIteration", PyStr::new(&position.to_string()), vec![]);
                stop.ctor = vec![PyValue::int(*position)];
                raised.context = Some(Box::new(stop));
                raised.suppress_context = true;
            }
            raised
        }
        LoadError::IntegerTooLong { .. } => Raised::new(
            "ValueError",
            PyStr::new(&error.to_string()),
            frames_json(JsonSite::Scanner),
        ),
        LoadError::TooDeep { .. } => {
            let shown = error.to_string();
            let body = shown.strip_prefix("RecursionError: ").unwrap_or(&shown);
            Raised::new(
                "RecursionError",
                PyStr::new(body),
                frames_json(JsonSite::Scanner),
            )
        }
        LoadError::Encoding(message) => Raised::new(
            "UnicodeDecodeError",
            PyStr::new(message),
            frames_json_load(),
        ),
    }
}

// ── Python object helpers ────────────────────────────────────────────────────────

fn attribute_error(owner_value: &PyValue, frame: Frame) -> Raised {
    let message = format!(
        "'{}' object has no attribute 'get'",
        owner_value.type_name()
    );
    let mut raised = Raised::new("AttributeError", PyStr::new(&message), vec![frame]);
    raised.ctor_kwargs = Box::new(vec![
        ("name", PyValue::str("get")),
        ("obj", owner_value.clone()),
    ]);
    raised
}

fn lookup<'a>(dict: &'a PyDict, key: &PyStr) -> Option<&'a PyValue> {
    dict.iter()
        .find(|(name, _)| name.code_points() == key.code_points())
        .map(|(_, value)| value)
}

/// `obj.get(key)` where `obj` is whatever the document held: a dict answers, anything else has no `get`.
fn get_of(obj: &PyValue, key: &PyValue, frame: Frame) -> Result<Option<PyValue>, Raised> {
    let PyValue::Dict(dict) = obj.peel() else {
        return Err(attribute_error(obj, frame));
    };
    match key.peel() {
        PyValue::Str(name) => Ok(lookup(dict, name).cloned()),
        PyValue::List(_) | PyValue::Dict(_) => Err(Raised::new(
            "TypeError",
            text(&format!("unhashable type: '{}'", key.type_name())),
            vec![frame_field_hash()],
        )),
        // A JSON object has only string keys: any other hashable key is absent.
        _ => Ok(None),
    }
}

/// `repr(value)`, with the `ValueError` an integer beyond the digit limit raises.
fn repr_checked(value: &PyValue, frames: &[Frame]) -> Result<String, Raised> {
    check_digits(value, frames)?;
    Ok(value.repr())
}

/// `str(value)`, same limit.
fn display_checked(value: &PyValue, frames: &[Frame]) -> Result<PyStr, Raised> {
    check_digits(value, frames)?;
    Ok(value.display_text())
}

fn check_digits(value: &PyValue, frames: &[Frame]) -> Result<(), Raised> {
    let too_long = match value.peel() {
        PyValue::Int(_) => value.repr().trim_start_matches('-').len() > MAX_INT_DIGITS,
        PyValue::List(items) | PyValue::Tuple(items) => {
            for item in items {
                check_digits(item, frames)?;
            }
            false
        }
        PyValue::Dict(dict) => {
            for (_, item) in dict.iter() {
                check_digits(item, frames)?;
            }
            false
        }
        _ => false,
    };
    if too_long {
        return Err(Raised::new(
            "ValueError",
            text("Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit"),
            frames.to_vec(),
        ));
    }
    Ok(())
}

// ── schema_cap ─────────────────────────────────────────────────────────────

/// `schema_cap(field, path)`: the `maximum` of `properties[field]` in the schema at `path`, read now.
///
/// The result is an `int` (a `bool` counts: `true` is a cap of 1) that is at least 1; anything else is the original's
/// fail-loud `SystemExit`.
pub fn schema_cap(field: &PyValue, path: &PathValue) -> Result<PyValue, Failure> {
    let document = load_document(path)?;
    let properties = get_of(&document, &PyValue::str("properties"), frame_doc_get())?;
    let properties = match properties {
        Some(value) if value.is_truthy() => value,
        _ => PyValue::Dict(PyDict::new()),
    };
    let node = get_of(&properties, field, frame_props_get())?;
    let node = match node {
        Some(value) if value.is_truthy() => value,
        _ => PyValue::Dict(PyDict::new()),
    };
    let cap = get_of(&node, &PyValue::str("maximum"), frame_node_get())?;
    if let Some(cap) = cap {
        let is_int = matches!(cap.peel(), PyValue::Int(_) | PyValue::Bool(_));
        if is_int {
            let below_one = compare(Compare::Less, &cap, &PyValue::int(1))
                .expect("an int compares with an int");
            if !below_one {
                return Ok(cap);
            }
        }
    }
    let message = join(&[
        &text(&format!("{FAIL_PREFIX}요청 스키마에서 ")),
        &field.display_text(),
        &text(" 상한을 읽지 못했다 — "),
        &path.display(),
    ]);
    let frame = owner(
        44,
        "schema_cap",
        "raise SystemExit(f\"[turn-budget] FAIL: 요청 스키마에서 {field} 상한을 읽지 못했다 — {path}\")",
        "",
    );
    Err(exit(message, vec![frame]).into())
}

// ── declare ───────────────────────────────────────────────────────────

/// The arguments of `declare` that influence its answer (`max_dispatches` is ignored by the original and is not carried).
#[derive(Clone, Debug)]
pub struct Declaration {
    pub max_turns: PyValue,
    pub timeout_seconds: PyValue,
    pub source: PyValue,
    pub caps: PyValue,
    pub unit: PyValue,
}

impl Declaration {
    /// The defaults of the keyword arguments: `caps=None`, `unit="native_turns"`.
    pub fn new(max_turns: PyValue, timeout_seconds: PyValue, source: PyValue) -> Declaration {
        Declaration {
            max_turns,
            timeout_seconds,
            source,
            caps: PyValue::None,
            unit: PyValue::str("native_turns"),
        }
    }
}

fn declare_frame(line: u32, code: &'static str, expr: &'static str) -> Frame {
    owner(line, "declare", code, expr)
}

/// `caps[k]` for a `caps` of any type.
fn subscript(caps: &PyValue, key: &str) -> Result<PyValue, Raised> {
    let frames = vec![declare_frame(78, "if v > caps[k]:", "caps[k]")];
    match caps.peel() {
        PyValue::Dict(dict) => lookup(dict, &PyStr::new(key)).cloned().ok_or_else(|| {
            let mut raised = Raised::new("KeyError", PyStr::new(&PyStr::new(key).repr()), frames);
            raised.ctor = vec![PyValue::str(key)];
            raised
        }),
        PyValue::List(_) | PyValue::Tuple(_) | PyValue::Bytes(_) => Err(Raised::new(
            "TypeError",
            text(&format!(
                "{} indices must be integers or slices, not str",
                match caps.type_name() {
                    "bytes" => "byte",
                    other => other,
                }
            )),
            frames,
        )),
        PyValue::Str(_) => Err(Raised::new(
            "TypeError",
            text("string indices must be integers, not 'str'"),
            frames,
        )),
        other => Err(Raised::new(
            "TypeError",
            text(&format!(
                "'{}' object is not subscriptable",
                other.type_name()
            )),
            frames,
        )),
    }
}

/// `declare(...)`: the declared budget, validated and returned as the original's dict.
pub fn declare(declaration: &Declaration, default_schema: &PathValue) -> Result<PyValue, Failure> {
    let caps = if declaration.caps.is_truthy() {
        declaration.caps.clone()
    } else {
        let max_turns =
            schema_cap(&PyValue::str("max_turns"), default_schema).map_err(|failure| {
                under_failure(
                    failure,
                    declare_frame(
                        60,
                        "caps = caps or {\"max_turns\": schema_cap(\"max_turns\"),",
                        "schema_cap(\"max_turns\")",
                    ),
                )
            })?;
        let timeout =
            schema_cap(&PyValue::str("timeout_seconds"), default_schema).map_err(|failure| {
                under_failure(
                    failure,
                    declare_frame(
                        61,
                        "\"timeout_seconds\": schema_cap(\"timeout_seconds\")}",
                        "schema_cap(\"timeout_seconds\")",
                    ),
                )
            })?;
        let mut dict = PyDict::new();
        dict.set("max_turns", max_turns);
        dict.set("timeout_seconds", timeout);
        PyValue::Dict(dict)
    };
    let unit_frames = vec![declare_frame(
        63,
        "raise SystemExit(f\"[turn-budget] FAIL: unsupported budget unit {unit!r}\")",
        "",
    )];
    let unit = match declaration.unit.peel() {
        PyValue::Str(unit) if UNITS.iter().any(|known| unit.equals(known)) => unit.clone(),
        other => {
            let shown = repr_checked(other, &unit_frames)?;
            return Err(exit(
                fail_text(&format!("unsupported budget unit {shown}")),
                unit_frames,
            )
            .into());
        }
    };
    let wall = unit.equals("wall_time");
    if wall && !matches!(declaration.max_turns.peel(), PyValue::None) {
        return Err(exit(
            fail_text("wall_time provider cannot enforce native max_turns"),
            vec![declare_frame(
                66,
                "raise SystemExit(\"[turn-budget] FAIL: wall_time provider cannot enforce native max_turns\")",
                "",
            )],
        )
        .into());
    }
    let mut values: Vec<(&str, &PyValue)> = vec![("timeout_seconds", &declaration.timeout_seconds)];
    if !wall {
        values.push(("max_turns", &declaration.max_turns));
    }
    for (key, value) in values {
        if matches!(value.peel(), PyValue::None) {
            return Err(exit(
                fail_text(&format!(
                    "{key} 가 선언되지 않았다 — 기본값을 만들지 않는다(등급표 폐기).\n  → 예산은 과업의 크기를 아는 쪽이 정한다. `--max-turns`/`--timeout-seconds` 로 선언하라.\n  → 규율: scope ⊥ budget(매달림은 timeout 이 잡는다) · checkpoint부터 재개."
                )),
                vec![declare_frame(72, "raise SystemExit(", "")],
            )
            .into());
        }
        let valid = match value.peel() {
            PyValue::Int(_) => !compare(Compare::Less, value, &PyValue::int(1))
                .expect("an int compares with an int"),
            _ => false,
        };
        if !valid {
            let frames = vec![declare_frame(
                77,
                "raise SystemExit(f\"[turn-budget] FAIL: {k}={v!r} 은 1 이상의 정수가 아니다.\")",
                "",
            )];
            let shown = repr_checked(value, &frames)?;
            return Err(exit(
                fail_text(&format!("{key}={shown} 은 1 이상의 정수가 아니다.")),
                frames,
            )
            .into());
        }
        let cap = subscript(&caps, key)?;
        let over = compare(Compare::Greater, value, &cap).map_err(|error| {
            Raised::new(
                "TypeError",
                PyStr::new(&error.message),
                vec![declare_frame(78, "if v > caps[k]:", "v > caps[k]")],
            )
        })?;
        if over {
            let frames = vec![declare_frame(79, "raise SystemExit(", "")];
            let shown = display_checked(value, &frames)?;
            let limit = display_checked(&cap, &frames)?;
            let message = join(&[
                &text(&format!("{FAIL_PREFIX}{key}=")),
                &shown,
                &text(" 가 전송 상한 "),
                &limit,
                &text(" 을 넘는다(요청 스키마).\n  → 상한에서 모자란다면 예산 문제가 아니라 **과업을 더 작게 쪼개라는 신호**다(scope ⊥ budget)."),
            ]);
            return Err(exit(message, frames).into());
        }
    }
    let source = match declaration.source.peel() {
        PyValue::Str(source) if !source.strip().is_empty() => source.strip(),
        _ => {
            return Err(exit(
                fail_text("예산 출처(source)가 비어 있다 — 값 옆에 출처를 적는다."),
                vec![declare_frame(
                    84,
                    "raise SystemExit(\"[turn-budget] FAIL: 예산 출처(source)가 비어 있다 — 값 옆에 출처를 적는다.\")",
                    "",
                )],
            )
            .into())
        }
    };
    let mut result = PyDict::new();
    result.set("max_turns", declaration.max_turns.clone());
    result.set("timeout_seconds", declaration.timeout_seconds.clone());
    result.set("source", PyValue::Str(source));
    if wall {
        result.set("budget_unit", PyValue::Str(unit));
    }
    Ok(PyValue::Dict(result))
}

fn under_failure(failure: Failure, caller: Frame) -> Failure {
    match failure {
        Failure::Raised(raised) => Failure::Raised(Box::new(raised.under(vec![caller]))),
        other => other,
    }
}

// ── --caps ───────────────────────────────────────────────────────────

/// The document `--caps` dumps: both caps read from the schema now and the source line naming the schema.
pub fn caps_document(default_schema: &PathValue) -> Result<PyValue, Failure> {
    let call = |line: u32, code: &'static str, field: &'static str, expr: &'static str| {
        schema_cap(&PyValue::str(field), default_schema)
            .map_err(|failure| under_failure(failure, owner(line, "main", code, expr)))
    };
    let max_turns = call(
        166,
        "json.dump({\"max_turns\": schema_cap(\"max_turns\"),",
        "max_turns",
        "schema_cap(\"max_turns\")",
    )?;
    let timeout = call(
        167,
        "\"timeout_seconds\": schema_cap(\"timeout_seconds\"),",
        "timeout_seconds",
        "schema_cap(\"timeout_seconds\")",
    )?;
    let mut dict = PyDict::new();
    dict.set("max_turns", max_turns);
    dict.set("timeout_seconds", timeout);
    dict.set(
        "source",
        PyValue::str(&format!("request-schema:{}", request_schema_relative())),
    );
    Ok(PyValue::Dict(dict))
}

// ── the self-test ──────────────────────────────────────────────────────────

/// Where a `print` of the self-test is in the owner (for the traceback of a print that fails).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintSite {
    pub line: u32,
    pub code: &'static str,
}

/// Why the self-test stopped before its summary line.
pub enum SelfTestStop<E> {
    /// An exception of the owner that no `except SystemExit` caught (the failing frame below `_self_test` included).
    Raised(Raised),
    Unsupported(String),
    /// The sink (a `print`) failed, at this site.
    Print(E, PrintSite),
}

const fn site(line: u32, code: &'static str) -> PrintSite {
    PrintSite { line, code }
}

/// `_self_test()`: `emit(text, site)` is one `print`; the return value is the exit status (0 or 2).
pub fn self_test<E>(
    default_schema: &PathValue,
    module_names: Option<&[PyStr]>,
    emit: &mut dyn FnMut(&PyStr, PrintSite) -> Result<(), E>,
) -> Result<i32, SelfTestStop<E>> {
    let mut ok = true;
    let mut chk = |cond: bool, label: &PyStr, at: PrintSite| -> Result<(), SelfTestStop<E>> {
        let mut line = PyStr::new(if cond { "  [PASS] " } else { "  [FAIL] " });
        line.push_py(label);
        emit(&line, at).map_err(|error| SelfTestStop::Print(error, at))?;
        ok = ok && cond;
        Ok(())
    };
    let wrap = |failure: Failure, line: u32, code: &'static str, expr: &'static str| match failure {
        Failure::Raised(raised) => {
            SelfTestStop::Raised(raised.under(vec![owner(line, "_self_test", code, expr)]))
        }
        Failure::Unsupported(why) => SelfTestStop::Unsupported(why),
    };

    // tripwire: the module namespace never holds the revived grade-table names (the importer's own names when it marshals them)
    let standalone: Vec<PyStr> = STANDALONE_GLOBALS
        .iter()
        .map(|name| PyStr::new(name))
        .collect();
    let revived = PyValue::List(
        revived_names(module_names.unwrap_or(&standalone))
            .into_iter()
            .map(PyValue::Str)
            .collect(),
    );
    chk(
        !revived.is_truthy(),
        &join(&[
            &text("등급표·증액계수·정지한계가 되살아나지 않았다(발견 "),
            &PyStr::new(&revived.repr()),
            &text(")"),
        ]),
        site(
            105,
            "chk(revived == [], f\"등급표·증액계수·정지한계가 되살아나지 않았다(발견 {revived})\")",
        ),
    )?;

    const CAPS_LINE: &str = "caps = {\"max_turns\": schema_cap(\"max_turns\"), \"timeout_seconds\": schema_cap(\"timeout_seconds\")}";
    let max_turns_cap = schema_cap(&PyValue::str("max_turns"), default_schema)
        .map_err(|f| wrap(f, 107, CAPS_LINE, "schema_cap(\"max_turns\")"))?;
    let timeout_cap = schema_cap(&PyValue::str("timeout_seconds"), default_schema)
        .map_err(|f| wrap(f, 107, CAPS_LINE, "schema_cap(\"timeout_seconds\")"))?;
    let mut caps = PyDict::new();
    caps.set("max_turns", max_turns_cap.clone());
    caps.set("timeout_seconds", timeout_cap);
    let caps = PyValue::Dict(caps);
    let mut expected = PyDict::new();
    expected.set("max_turns", PyValue::int(100));
    expected.set("timeout_seconds", PyValue::int(3600));
    chk(
        caps == PyValue::Dict(expected),
        &join(&[
            &text("상한을 요청 스키마에서 읽는다(상수 복제 ✗) → "),
            &PyStr::new(&caps.repr()),
        ]),
        site(
            108,
            "chk(caps == {\"max_turns\": 100, \"timeout_seconds\": 3600},",
        ),
    )?;

    let call = |max_turns: PyValue, timeout: PyValue, source: &str, unit: &str| {
        let mut declaration = Declaration::new(max_turns, timeout, PyValue::str(source));
        declaration.unit = PyValue::str(unit);
        declare(&declaration, default_schema)
    };
    let native = "native_turns";
    let dict_get = |value: &PyValue, key: &str| -> PyValue {
        match value {
            PyValue::Dict(dict) => dict.get(key).cloned().unwrap_or(PyValue::None),
            _ => PyValue::None,
        }
    };
    let int_is = |value: &PyValue, number: u32| {
        matches!(value, PyValue::Int(_)) && *value == PyValue::int(number)
    };

    let declared = call(
        PyValue::int(24),
        PyValue::int(1800),
        "선언: 빌드 1회 + 스모크 — 원장 attempt 3 의 실측 turns=19",
        native,
    )
    .map_err(|f| {
        wrap(
            f,
            111,
            "d = declare(24, 1800, source=\"선언: 빌드 1회 + 스모크 — 원장 attempt 3 의 실측 turns=19\")",
            "declare(24, 1800, source=\"선언: 빌드 1회 + 스모크 — 원장 attempt 3 의 실측 turns=19\")",
        )
    })?;
    chk(
        int_is(&dict_get(&declared, "max_turns"), 24)
            && int_is(&dict_get(&declared, "timeout_seconds"), 1800),
        &text("선언한 값이 그대로 나온다(재계산·보정 ✗)"),
        site(
            112,
            "chk(d[\"max_turns\"] == 24 and d[\"timeout_seconds\"] == 1800,",
        ),
    )?;
    let has_origin = match dict_get(&declared, "source") {
        PyValue::Str(source) => source.to_utf8().is_some_and(|s| s.contains("원장")),
        _ => false,
    };
    chk(
        has_origin,
        &text("값 옆에 출처가 남는다"),
        site(
            114,
            "chk(\"원장\" in d[\"source\"], \"값 옆에 출처가 남는다\")",
        ),
    )?;

    let bad_values: [(PyValue, &str); 5] = [
        (PyValue::None, "미선언"),
        (PyValue::int(0), "0"),
        (PyValue::int(-1), "음수"),
        (PyValue::Bool(true), "bool"),
        (PyValue::str("10"), "문자열"),
    ];
    for (bad, label) in bad_values {
        let label_pass = join(&[&text(label), &text(" max_turns → fail-loud")]);
        match call(bad, PyValue::int(600), "s", native) {
            Ok(_) => chk(
                false,
                &label_pass,
                site(119, "chk(False, f\"{label} max_turns → fail-loud\")"),
            )?,
            Err(Failure::Raised(raised)) if raised.is_system_exit() => chk(
                true,
                &label_pass,
                site(121, "chk(True, f\"{label} max_turns → fail-loud\")"),
            )?,
            Err(failure) => {
                return Err(wrap(
                    failure,
                    118,
                    "declare(bad, 600, source=\"s\")",
                    "declare(bad, 600, source=\"s\")",
                ))
            }
        }
    }

    match call(PyValue::int(10), PyValue::None, "s", native) {
        Ok(_) => chk(false, &text("timeout 미선언 → fail-loud"), site(124, "chk(False, \"timeout 미선언 → fail-loud\")"))?,
        Err(Failure::Raised(raised)) if raised.is_system_exit() => chk(
            raised.message.to_utf8().is_some_and(|m| m.contains("scope ⊥ budget")),
            &text("timeout 미선언 → fail-loud(두 규율을 함께 말한다)"),
            site(126, "chk(\"scope ⊥ budget\" in str(e), \"timeout 미선언 → fail-loud(두 규율을 함께 말한다)\")"),
        )?,
        Err(failure) => {
            return Err(wrap(failure, 123, "declare(10, None, source=\"s\")", "declare(10, None, source=\"s\")"))
        }
    }

    let over = match &max_turns_cap {
        PyValue::Int(_) | PyValue::Bool(_) => {
            let base = match &max_turns_cap {
                PyValue::Bool(flag) => PyValue::int(u32::from(*flag)),
                other => other.clone(),
            };
            match base {
                PyValue::Int(number) => PyValue::Int(number + 1u32),
                other => other,
            }
        }
        other => other.clone(),
    };
    match call(over, PyValue::int(600), "s", native) {
        Ok(_) => chk(
            false,
            &text("상한 초과 → fail-loud"),
            site(129, "chk(False, \"상한 초과 → fail-loud\")"),
        )?,
        Err(Failure::Raised(raised)) if raised.is_system_exit() => chk(
            raised
                .message
                .to_utf8()
                .is_some_and(|m| m.contains("쪼개라")),
            &text("상한 초과는 증액이 아니라 **과업 분할**을 지시한다"),
            site(
                131,
                "chk(\"쪼개라\" in str(e), \"상한 초과는 증액이 아니라 **과업 분할**을 지시한다\")",
            ),
        )?,
        Err(failure) => {
            return Err(wrap(
                failure,
                128,
                "declare(caps[\"max_turns\"] + 1, 600, source=\"s\")",
                "declare(caps[\"max_turns\"] + 1, 600, source=\"s\")",
            ))
        }
    }

    match call(PyValue::int(10), PyValue::int(600), "   ", native) {
        Ok(_) => chk(
            false,
            &text("출처 없는 선언 → fail-loud"),
            site(134, "chk(False, \"출처 없는 선언 → fail-loud\")"),
        )?,
        Err(Failure::Raised(raised)) if raised.is_system_exit() => chk(
            true,
            &text("출처 없는 선언 → fail-loud(출처 표시 규율)"),
            site(
                136,
                "chk(True, \"출처 없는 선언 → fail-loud(출처 표시 규율)\")",
            ),
        )?,
        Err(failure) => {
            return Err(wrap(
                failure,
                133,
                "declare(10, 600, source=\"   \")",
                "declare(10, 600, source=\"   \")",
            ))
        }
    }

    let wall = call(
        PyValue::None,
        PyValue::int(90),
        "per-invocation",
        "wall_time",
    )
    .map_err(|f| {
        wrap(
            f,
            138,
            "wall = declare(None, 90, source=\"per-invocation\", unit=\"wall_time\")",
            "declare(None, 90, source=\"per-invocation\", unit=\"wall_time\")",
        )
    })?;
    let wall_ok = matches!(dict_get(&wall, "budget_unit"), PyValue::Str(ref unit) if unit.equals("wall_time"))
        && !matches!(&wall, PyValue::Dict(dict) if dict.contains_key("max_dispatches"));
    chk(
        wall_ok,
        &text("Codex wall-time declaration requires no cumulative dispatch count"),
        site(
            139,
            "chk(wall[\"budget_unit\"] == \"wall_time\" and \"max_dispatches\" not in wall,",
        ),
    )?;
    // `max_dispatches=3` is accepted and ignored by the original
    let legacy = call(PyValue::None, PyValue::int(90), "historical caller", "wall_time").map_err(|f| {
        wrap(
            f,
            141,
            "legacy = declare(None, 90, source=\"historical caller\", unit=\"wall_time\", max_dispatches=3)",
            "declare(None, 90, source=\"historical caller\", unit=\"wall_time\", max_dispatches=3)",
        )
    })?;
    chk(
        !matches!(&legacy, PyValue::Dict(dict) if dict.contains_key("max_dispatches")),
        &text("new budgets do not emit a legacy dispatch count"),
        site(142, "chk(\"max_dispatches\" not in legacy, \"new budgets do not emit a legacy dispatch count\")"),
    )?;
    for (max_turns, timeout) in [
        (PyValue::None, PyValue::None),
        (PyValue::int(2), PyValue::int(90)),
    ] {
        match call(max_turns, timeout, "negative", "wall_time") {
            Ok(_) => chk(
                false,
                &text("Codex missing timeout or native turn cap rejected"),
                site(
                    147,
                    "chk(False, \"Codex missing timeout or native turn cap rejected\")",
                ),
            )?,
            Err(Failure::Raised(raised)) if raised.is_system_exit() => chk(
                true,
                &text("Codex missing timeout or native turn cap rejected"),
                site(
                    149,
                    "chk(True, \"Codex missing timeout or native turn cap rejected\")",
                ),
            )?,
            Err(failure) => {
                return Err(wrap(
                    failure,
                    146,
                    "declare(source=\"negative\", unit=\"wall_time\", **kwargs)",
                    "declare(source=\"negative\", unit=\"wall_time\", **kwargs)",
                ))
            }
        }
    }

    let summary = text(if ok {
        "self-test: PASS"
    } else {
        "self-test: FAIL"
    });
    let at = site(
        151,
        "print(\"self-test: %s\" % (\"PASS\" if ok else \"FAIL\"))",
    );
    emit(&summary, at).map_err(|error| SelfTestStop::Print(error, at))?;
    Ok(if ok { 0 } else { EXIT_SELF_TEST_FAILED })
}
