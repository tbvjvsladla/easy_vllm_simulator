//! The command line of `manifest_contract.py` (`--topology single|multi`, `--repo`, `--require-flag`, `--json`) as a function of its
//! arguments: the exact grammar, the flow of `main` (branch conflict, topology, manifest file, Flag gate, output) and the
//! text, status and traceback the owner's process shows for it. Writing the bytes (the stdout codec, the stderr codec, the
//! flush) is the caller's job; this module answers with text and a status.
//!
//! **Grammar.** Only the exact shapes native code answers: separate tokens, any order, each at most once, `--topology` followed by
//! `single` or `multi`, `--repo` followed by a value that does not start with `-` (an empty value is accepted, as `argparse`
//! does), plus the two switches. Everything else (`-h`, `--self-test`, abbreviations, `--opt=value`, duplicates, other
//! options, positionals, dash-leading values, a topology outside the two choices) belongs to the owner's `argparse`: a caller
//! routes it to the real original before any native business ([`parse_exact`] returning `None`).
//!
//! **Flow** (owner lines 401-462): `repo = os.path.abspath(--repo)` (lexical); a conflict between an explicit topology and the
//! checkout branch's lane is exit 6; no topology is exit 2; a manifest path that is not a file is exit 3; a load failure of any
//! kind is `manifest 파싱 실패: <str(e)>` and exit 2; then the Flag gate; the result is printed as JSON (`ensure_ascii=False`,
//! `indent=2`) or as one line, and the status is the gate's exit code in both arms of the `--require-flag` conditional.
//!
//! One `git -C <repo> rev-parse --abbrev-ref HEAD` is run per invocation at most (the owner's `_git_branch`: no timeout, the
//! environment's `PATH`, stdout stripped, every failure `None`).

use crate::campaign::{join, os_error_text};
use crate::load::{load_yaml_objects, LoadFailure};
use crate::manifest_gate::{
    branch_topology_conflict, evaluate_contract, py_str, resolve_topology, BranchProbe,
    GateFailure, OwnerFrame, Raised, EXIT_NO_MANIFEST, EXIT_TOPOLOGY_MISMATCH, EXIT_USAGE,
    REDIRECT_TEMPLATE,
};
use crate::pyprofile::fs_decode;
use crate::topology::{abspath, Frame};
use crate::topology_cli::push_frame;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{
    json_key, key_is_float, DumpError, NonStringKey, PyDict, PyValue,
};
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};

// ── owner frames of `main` ───────────────────────────────────────────────────────────────

/// `sys.exit(main())` at the end of the owner.
pub const MODULE_EXIT: OwnerFrame = OwnerFrame {
    line: 462,
    function: "<module>",
    code: "sys.exit(main())",
    expr: "main()",
};
/// `main` reaching `evaluate_contract(man, topology)`.
pub const MAIN_EVALUATE: OwnerFrame = OwnerFrame {
    line: 444,
    function: "main",
    code: "res = evaluate_contract(man, topology)",
    expr: "evaluate_contract(man, topology)",
};
/// `main` reaching `os.path.abspath(args.repo)` (the first thing it does with the repository argument).
pub const MAIN_ABSPATH: OwnerFrame = OwnerFrame {
    line: 414,
    function: "main",
    code: "repo = os.path.abspath(args.repo)",
    expr: "os.path.abspath(args.repo)",
};
/// The `print(json.dumps(res, ...))` line when `json.dumps` raises (the marked part is the `json.dumps(...)` call).
pub const MAIN_DUMPS: OwnerFrame = OwnerFrame {
    line: 450,
    function: "main",
    code: "print(json.dumps(res, ensure_ascii=False, indent=2))",
    expr: "json.dumps(res, ensure_ascii=False, indent=2)",
};

/// The `print` statements of `main` that write to stdout. When the write fails the marked span is the whole first line, so
/// Python prints no carets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintSite {
    Conflict,
    Missing,
    Json,
    Plain,
}

impl PrintSite {
    pub fn frame(self) -> OwnerFrame {
        let (line, code) = match self {
            PrintSite::Conflict => (418, "print(json.dumps({\"flag\": False, \"reason\": conflict, \"topology\": args.topology,"),
            PrintSite::Missing => (432, "print(json.dumps({\"flag\": False, \"reason\": msg, \"topology\": topology,"),
            PrintSite::Json => (450, "print(json.dumps(res, ensure_ascii=False, indent=2))"),
            PrintSite::Plain => (453, "print(\"Flag valid · topology=%s · gpus_per_node=%s · manifest_tp=%s · model_source=%s\""),
        };
        OwnerFrame {
            line,
            function: "main",
            code,
            expr: code,
        }
    }
}

/// Every row above, for the source-binding check.
pub const MAIN_FRAMES: [&OwnerFrame; 4] =
    [&MODULE_EXIT, &MAIN_EVALUATE, &MAIN_DUMPS, &MAIN_ABSPATH];

// ── std-lib frames (CPython 3.12.3 at /usr/lib/python3.12) ──────────────────────────────

struct Std {
    file: &'static str,
    line: u32,
    function: &'static str,
    code: &'static str,
    expr: &'static str,
}

const JSON_DUMPS: Std = Std {
    file: "/usr/lib/python3.12/json/__init__.py",
    line: 238,
    function: "dumps",
    code: "**kw).encode(obj)",
    expr: "encode(obj)",
};
const ENCODER_ENCODE: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 202,
    function: "encode",
    code: "chunks = list(chunks)",
    expr: "list(chunks)",
};
const ENCODER_TOP: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 432,
    function: "_iterencode",
    code: "yield from _iterencode_dict(o, _current_indent_level)",
    expr: "",
};
const YIELD_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 406,
    function: "_iterencode_dict",
    code: "yield from chunks",
    expr: "",
};
const YIELD_LIST: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 326,
    function: "_iterencode_list",
    code: "yield from chunks",
    expr: "",
};
const RAISE_CIRCULAR: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 285,
    function: "_iterencode_list",
    code: "raise ValueError(\"Circular reference detected\")",
    expr: "",
};
const RAISE_CIRCULAR_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 341,
    function: "_iterencode_dict",
    code: "raise ValueError(\"Circular reference detected\")",
    expr: "",
};
const RAISE_BAD_KEY: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 377,
    function: "_iterencode_dict",
    code: "raise TypeError(f'keys must be str, int, float, bool or None, '",
    expr: "",
};
const YIELD_INT_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 395,
    function: "_iterencode_dict",
    code: "yield _intstr(value)",
    expr: "_intstr(value)",
};
const YIELD_INT_LIST: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 314,
    function: "_iterencode_list",
    code: "yield buf + _intstr(value)",
    expr: "_intstr(value)",
};
const RAISE_INT_KEY: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 373,
    function: "_iterencode_dict",
    code: "key = _intstr(key)",
    expr: "_intstr(key)",
};
const CALL_LIST_IN_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 401,
    function: "_iterencode_dict",
    code: "chunks = _iterencode_list(value, _current_indent_level)",
    expr: "_iterencode_list(value, _current_indent_level)",
};
const CALL_DICT_IN_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 403,
    function: "_iterencode_dict",
    code: "chunks = _iterencode_dict(value, _current_indent_level)",
    expr: "_iterencode_dict(value, _current_indent_level)",
};
const CALL_OTHER_IN_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 405,
    function: "_iterencode_dict",
    code: "chunks = _iterencode(value, _current_indent_level)",
    expr: "_iterencode(value, _current_indent_level)",
};
const CALL_LIST_IN_LIST: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 321,
    function: "_iterencode_list",
    code: "chunks = _iterencode_list(value, _current_indent_level)",
    expr: "_iterencode_list(value, _current_indent_level)",
};
const CALL_DICT_IN_LIST: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 323,
    function: "_iterencode_list",
    code: "chunks = _iterencode_dict(value, _current_indent_level)",
    expr: "_iterencode_dict(value, _current_indent_level)",
};
const CALL_OTHER_IN_LIST: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 325,
    function: "_iterencode_list",
    code: "chunks = _iterencode(value, _current_indent_level)",
    expr: "_iterencode(value, _current_indent_level)",
};
/// `yield _floatstr(value)` of a dict value (captured raw R48K06) and `yield buf + _floatstr(value)` of a list item (derived from the same
/// encoder source, no raw).
const FLOAT_IN_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 398,
    function: "_iterencode_dict",
    code: "yield _floatstr(value)",
    expr: "_floatstr(value)",
};
/// `key = _floatstr(key)` of a dict KEY that is a float (captured raw R48K08, one case at 995 containers).
const FLOAT_KEY_IN_DICT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 364,
    function: "_iterencode_dict",
    code: "key = _floatstr(key)",
    expr: "_floatstr(key)",
};
const FLOAT_IN_LIST: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 317,
    function: "_iterencode_list",
    code: "yield buf + _floatstr(value)",
    expr: "_floatstr(value)",
};
const CALL_DEFAULT: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 439,
    function: "_iterencode",
    code: "o = _default(o)",
    expr: "_default(o)",
};
const RAISE_NOT_SERIALIZABLE: Std = Std {
    file: "/usr/lib/python3.12/json/encoder.py",
    line: 180,
    function: "default",
    code: "raise TypeError(f'Object of type {o.__class__.__name__} '",
    expr: "",
};

fn std_frame(row: &Std) -> Frame {
    Frame {
        file: row.file.as_bytes().to_vec(),
        line: row.line,
        function: row.function,
        code: row.code,
        expr: row.expr,
    }
}

fn owner_frame(file: &[u8], row: &OwnerFrame) -> Frame {
    Frame {
        file: file.to_vec(),
        line: row.line,
        function: row.function,
        code: row.code,
        expr: row.expr,
    }
}

/// `Traceback (most recent call last):`, the frames (outermost first) and `Class: message`.
pub fn render_traceback(frames: &[Frame], class: &str, message: &PyStr) -> PyStr {
    let mut out = PyStr::new("Traceback (most recent call last):\n");
    let mut run = 0usize;
    let mut previous: Option<&Frame> = None;
    for frame in frames {
        // `traceback.StackSummary.format`: the same file, line and function again and again print three times, then one
        // `[Previous line repeated N more time(s)]` line before the next different frame (or the exception line)
        let same = previous.is_some_and(|p| {
            p.file == frame.file && p.line == frame.line && p.function == frame.function
        });
        if same {
            run += 1;
        } else {
            push_repeats(&mut out, run);
            run = 1;
        }
        if run <= REPEAT_CUTOFF {
            push_frame(&mut out, frame);
        }
        previous = Some(frame);
    }
    push_repeats(&mut out, run);
    out.push_str(class);
    if !message.is_empty() {
        out.push_str(": ");
        out.push_py(message);
    }
    out.push_str("\n");
    out
}

/// `traceback._RECURSIVE_CUTOFF`.
const REPEAT_CUTOFF: usize = 3;

fn push_repeats(out: &mut PyStr, run: usize) {
    if run > REPEAT_CUTOFF {
        let more = run - REPEAT_CUTOFF;
        out.push_str(&format!(
            "  [Previous line repeated {more} more time{}]\n",
            if more > 1 { "s" } else { "" }
        ));
    }
}

// ── the exact grammar ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// `--topology`, `None` when absent.
    pub topology: Option<&'static str>,
    /// `--repo` (default `.`), as the raw argument bytes.
    pub repo: Vec<u8>,
    pub require_flag: bool,
    pub json: bool,
}

/// The shapes native code answers; `None` for anything the owner's `argparse` must judge.
pub fn parse_exact(args: &[Vec<u8>]) -> Option<Request> {
    let mut request = Request {
        topology: None,
        repo: b".".to_vec(),
        require_flag: false,
        json: false,
    };
    let (mut repo_seen, mut index) = (false, 0);
    while index < args.len() {
        match args[index].as_slice() {
            b"--topology" => {
                if request.topology.is_some() {
                    return None;
                }
                request.topology = Some(match args.get(index + 1)?.as_slice() {
                    b"single" => "single",
                    b"multi" => "multi",
                    _ => return None,
                });
                index += 2;
            }
            b"--repo" => {
                let value = args.get(index + 1)?;
                if repo_seen || value.first() == Some(&b'-') {
                    return None;
                }
                repo_seen = true;
                request.repo = value.clone();
                index += 2;
            }
            b"--require-flag" => {
                if request.require_flag {
                    return None;
                }
                request.require_flag = true;
                index += 1;
            }
            b"--json" => {
                if request.json {
                    return None;
                }
                request.json = true;
                index += 1;
            }
            _ => return None,
        }
    }
    Some(request)
}

// ── answers ─────────────────────────────────────────────────────────────────────────────

/// One `print(text)` to stdout; `text` is without the newline `print` adds.
#[derive(Clone, Debug, PartialEq)]
pub struct Printed {
    pub text: PyStr,
    pub site: PrintSite,
}

/// What the owner's process shows: at most one stdout print, at most one stderr text (a message or a traceback) and the status.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub stdout: Option<Printed>,
    pub stderr: Option<PyStr>,
    pub status: i32,
    /// An exception the owner does not handle ended the process (the stderr text is its traceback).
    pub crashed: bool,
    /// Class and message of that exception, when modelled (the in-process caller gets the exception, not the traceback text).
    pub exception: Option<(String, PyStr)>,
}

/// A shape this port does not answer (no captured original): visible to the caller, never answered by guessing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    pub boundary: &'static str,
    pub detail: String,
}

impl Answer {
    fn message(text: PyStr, status: i32) -> Answer {
        let mut line = text;
        line.push_str("\n");
        Answer {
            stdout: None,
            stderr: Some(line),
            status,
            crashed: false,
            exception: None,
        }
    }
    fn printed(text: PyStr, site: PrintSite, status: i32) -> Answer {
        Answer {
            stdout: Some(Printed { text, site }),
            stderr: None,
            status,
            crashed: false,
            exception: None,
        }
    }
    fn crash(frames: &[Frame], class: &str, message: &PyStr) -> Answer {
        Answer {
            stdout: None,
            stderr: Some(render_traceback(frames, class, message)),
            status: 1,
            crashed: true,
            exception: Some((class.to_string(), message.clone())),
        }
    }
}

/// The traceback text and status of a stdout `print` that failed to encode: the caller found a `UnicodeEncodeError` message
/// for the text it tried to write.
pub fn print_failure(owner: &[u8], site: PrintSite, message: &str) -> Answer {
    Answer::crash(
        &[
            owner_frame(owner, &MODULE_EXIT),
            owner_frame(owner, &site.frame()),
        ],
        "UnicodeEncodeError",
        &PyStr::new(message),
    )
}

/// The traceback of a stdout `print` whose write failed at once (for example `BrokenPipeError: [Errno 32] Broken pipe`): the
/// owner dies at that `print` with the already formed last line.
pub fn print_write_failure(owner: &[u8], site: PrintSite, last_line: &PyStr) -> PyStr {
    let frames = [
        owner_frame(owner, &MODULE_EXIT),
        owner_frame(owner, &site.frame()),
    ];
    let mut out = PyStr::new("Traceback (most recent call last):\n");
    for frame in &frames {
        push_frame(&mut out, frame);
    }
    out.push_py(last_line);
    out.push_str("\n");
    out
}

fn json_object(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in entries {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

fn refusal_json(reason: &PyStr, topology: PyValue) -> Result<PyStr, Unsupported> {
    json_object(vec![
        ("flag", PyValue::Bool(false)),
        ("reason", PyValue::Str(reason.clone())),
        ("topology", topology),
        ("redirect", PyValue::str(REDIRECT_TEMPLATE)),
    ])
    .dumps_unicode()
    .map_err(|error| Unsupported {
        boundary: "json_refusal",
        detail: error.to_string(),
    })
}

// ── the real branch probe ───────────────────────────────────────────────────────────────

/// The launcher's marker: present exactly when the launcher (or the in-process adapter) recorded the CALLER's value of every plumbing
/// variable below before it changed any of them.
pub const CALLER_ENV_MARKER: &str = "MANIFEST_CONTRACT_CALLER_ENV";
/// The plumbing variables the launcher / adapter add, rewrite or drop around this program. `MANIFEST_CONTRACT_CALLER_<suffix>` carries the
/// caller's value of `MANIFEST_CONTRACT_<suffix>`; a missing carrier means the caller had the variable unset. Nothing else is recorded:
/// no other variable of the caller (credentials included) is read, copied or serialised.
pub const PLUMBING_SUFFIXES: [&str; 6] = [
    "BRIDGE",
    "BRIDGE_REQUEST",
    "COMPAT_OWNER",
    "PUBLIC_ARGV0",
    "PYTHON",
    "VALIDATE_ONLY",
];

/// How a child of this program (the `git` probe, the legacy interpreter) gets back the caller's own values of the plumbing variables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChildEnv {
    remove: Vec<OsString>,
    set: Vec<(OsString, OsString)>,
}

impl ChildEnv {
    /// From a variable lookup (`None` = unset). Without the marker the process environment IS the caller's: nothing is changed.
    pub fn from_lookup(lookup: &dyn Fn(&str) -> Option<OsString>) -> Self {
        if lookup(CALLER_ENV_MARKER).is_none() {
            return ChildEnv::default();
        }
        let mut plan = ChildEnv {
            remove: vec![OsString::from(CALLER_ENV_MARKER)],
            set: Vec::new(),
        };
        for suffix in PLUMBING_SUFFIXES {
            let name = format!("MANIFEST_CONTRACT_{suffix}");
            let carrier = format!("MANIFEST_CONTRACT_CALLER_{suffix}");
            plan.remove.push(OsString::from(&name));
            plan.remove.push(OsString::from(&carrier));
            if let Some(value) = lookup(&carrier) {
                plan.set.push((OsString::from(name), value));
            }
        }
        plan
    }

    pub fn from_process() -> Self {
        Self::from_lookup(&|name| std::env::var_os(name))
    }

    /// Applies the plan on top of the inherited environment (removals first, then the caller's values).
    pub fn apply(&self, command: &mut Command) {
        for name in &self.remove {
            command.env_remove(name);
        }
        for (name, value) in &self.set {
            command.env(name, value);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.set.is_empty()
    }
}

/// `git -C <repo> rev-parse --abbrev-ref HEAD` as the owner runs it: the child gets the caller's environment (the plumbing variables are
/// restored from the recorded caller facts) and the caller's standard descriptors.
#[derive(Clone, Debug, Default)]
pub struct GitBranch {
    pub env: ChildEnv,
}

impl GitBranch {
    pub fn from_process() -> Self {
        GitBranch {
            env: ChildEnv::from_process(),
        }
    }
}

impl BranchProbe for GitBranch {
    fn branch(&mut self, repo: &[u8]) -> Option<PyStr> {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(OsStr::from_bytes(repo))
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.env.apply(&mut command);
        let output = command.output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        Some(PyStr::new(&text).strip())
    }
}

// ── JSON failures ───────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Container {
    Dict,
    List,
}

/// What a container is about to create a generator for when the recursion limit stops it.
#[derive(Clone, Copy)]
enum Child {
    Dict,
    List,
    Other,
}

enum Found {
    NotSerializable,
    CircularList,
    CircularDict,
    BadKey,
    HugeInt,
    HugeIntKey,
    TooDeep(Child),
    /// A leaf the encoder cannot serialize whose own generator frame still fits but whose `default(o)` call does not.
    DefaultTooDeep,
    /// A float under the deepest container that still fits: the `floatstr` call of the container's own frame is the one beyond the limit.
    FloatTooDeep,
    /// A float KEY of the deepest container that still fits: the `floatstr` call of the dict's own frame spelling the key is beyond the limit.
    FloatKeyTooDeep,
}

/// The Python frames below the first `_iterencode_dict` generator when `print(json.dumps(res, ...))` runs: the module, `main`,
/// `json.dumps`, `JSONEncoder.encode` and `_iterencode`. With CPython's default recursion limit the encoder can therefore hold
/// at most `limit - base` nested containers (one generator frame each); the generator that would create the next one fails.
const PY_RECURSION_LIMIT: usize = 1000;
const JSON_BASE_FRAMES: usize = 5;
const MAX_JSON_CONTAINERS: usize = PY_RECURSION_LIMIT - JSON_BASE_FRAMES;

fn too_deep(path: &[Container], child: Child) -> Option<Found> {
    (path.len() >= MAX_JSON_CONTAINERS).then_some(Found::TooDeep(child))
}

/// A leaf the encoder cannot serialize (bytes or an opaque object): its `_iterencode` generator frame is one below the container
/// that creates it and `default(o)` is called from that frame, so with `MAX - 1` enclosing containers the generator still fits and
/// the `_default(o)` CALL is the frame that hits the limit (captured: R48K05, 994 containers); with `MAX` the container's own
/// child-creation line fails like for any other child.
fn leaf_too_deep(path: &[Container]) -> Option<Found> {
    too_deep(path, Child::Other)
        .or_else(|| (path.len() + 1 >= MAX_JSON_CONTAINERS).then_some(Found::DefaultTooDeep))
}

fn key_is_huge_int(key: &NonStringKey) -> bool {
    matches!(key.value.as_deref().map(PyValue::peel), Some(PyValue::Int(number)) if too_many_digits(&number.to_string()))
}

/// The first failing value of `json.dumps` in the encoder's order (a dictionary's key is checked before its value), with the
/// containers entered on the way (the last one is the container that holds, or for a key failure is, the failing item; for
/// the recursion limit it is the container whose generator cannot create its child).
fn find_failure(value: &PyValue, path: &mut Vec<Container>) -> Option<Found> {
    match value.peel() {
        PyValue::Int(number) if too_many_digits(&number.to_string()) => Some(Found::HugeInt),
        PyValue::Float(_) if path.len() >= MAX_JSON_CONTAINERS => Some(Found::FloatTooDeep),
        PyValue::Bytes(_) => leaf_too_deep(path).or(Some(Found::NotSerializable)),
        PyValue::Opaque(opaque) if opaque.repr == "[...]" => {
            too_deep(path, Child::List).or(Some(Found::CircularList))
        }
        PyValue::Opaque(opaque) if opaque.repr == "{...}" => {
            too_deep(path, Child::Dict).or(Some(Found::CircularDict))
        }
        PyValue::Opaque(_) => leaf_too_deep(path).or(Some(Found::NotSerializable)),
        PyValue::List(items) | PyValue::Tuple(items) => {
            if let Some(found) = too_deep(path, Child::List) {
                return Some(found);
            }
            path.push(Container::List);
            for item in items {
                if let Some(found) = find_failure(item, path) {
                    return Some(found);
                }
            }
            path.pop();
            None
        }
        PyValue::Dict(map) => {
            if let Some(found) = too_deep(path, Child::Dict) {
                return Some(found);
            }
            path.push(Container::Dict);
            for (other, _, entry) in map.keyed_entries() {
                if let Some(key) = other {
                    if key_is_huge_int(key) {
                        return Some(Found::HugeIntKey);
                    }
                    if key_is_float(key) && path.len() >= MAX_JSON_CONTAINERS {
                        return Some(Found::FloatKeyTooDeep);
                    }
                    if json_key(key).is_err() {
                        return Some(Found::BadKey);
                    }
                }
                if let Some(found) = find_failure(entry, path) {
                    return Some(found);
                }
            }
            path.pop();
            None
        }
        _ => None,
    }
}

/// Whether the first failure in the encoder's order is an integer KEY beyond the digit limit (the dumper does not refuse it).
fn has_huge_int_key(res: &PyDict) -> bool {
    let mut path = Vec::new();
    matches!(
        find_failure(&PyValue::Dict(res.clone()), &mut path),
        Some(Found::HugeIntKey)
    )
}

/// The generator line that creates a child inside `parent` (`chunks = _iterencode_*(value, _current_indent_level)`).
fn call_row(parent: Container, child: Child) -> &'static Std {
    match (parent, child) {
        (Container::Dict, Child::List) => &CALL_LIST_IN_DICT,
        (Container::Dict, Child::Dict) => &CALL_DICT_IN_DICT,
        (Container::Dict, Child::Other) => &CALL_OTHER_IN_DICT,
        (Container::List, Child::List) => &CALL_LIST_IN_LIST,
        (Container::List, Child::Dict) => &CALL_DICT_IN_LIST,
        (Container::List, Child::Other) => &CALL_OTHER_IN_LIST,
    }
}

/// The traceback of `json.dumps(res, ensure_ascii=False, indent=2)` failing with `error`, built from the captured CPython 3.12.3
/// encoder: the value failures, the key refusal, the circular list and dict, a huge integer in a value, and the recursion limit
/// (one `yield from chunks` frame per container, the failing container at its child-creation line; the repeated frames are
/// collapsed by [`render_traceback`]). The huge integer in KEY position (`key = _intstr(key)`, frames 432/406/373) is captured too.
/// Captured at the recursion limit: the dict-in-dict container (`_iterencode_dict` line 403) and the bytes leaf at 994 enclosing
/// dicts (R48K05: the `_default(o)` CALL of the leaf's `_iterencode` frame, line 439). The other container kinds and the opaque
/// leaf at the limit come from the encoder source alone and are UNVERIFIED until a raw exists. Also captured (R48K06/R48K07): a float
/// value under 995 containers (`yield _floatstr(value)`, line 398) and a cyclic dict at 995 (the child-generator call, line 403, before its
/// circular check). A float in a LIST at the limit (line 317), a float dict KEY (line 364) and a float inside a set are not captured: the
/// list form comes from the encoder source, the key form is not modelled (KNOWN LIMIT). An error/failure pair that does not agree is [`Unsupported`] (nothing is
/// answered by guessing).
fn dumps_failure(owner: &[u8], res: &PyDict, error: DumpError) -> Result<Answer, Unsupported> {
    let unsupported = |why: &str| Unsupported {
        boundary: "json_dumps_traceback",
        detail: format!("{error}: {why}"),
    };
    let mut path = Vec::new();
    let found = find_failure(&PyValue::Dict(res.clone()), &mut path);
    // (containers whose `yield from chunks` frame is on the stack, the frames below them, class, message)
    let (entered, terminal, class, message) = match (error, found) {
        // found in the encoder's order and not seen by the dumper, so whatever error it reports later is not the first one
        (_, Some(Found::HugeIntKey)) => (
            path.len().saturating_sub(1),
            vec![&RAISE_INT_KEY],
            "ValueError",
            INT_STR_LIMIT_MESSAGE.to_string(),
        ),
        (DumpError::NotSerializable(name), Some(Found::NotSerializable)) => (
            path.len(),
            vec![&CALL_DEFAULT, &RAISE_NOT_SERIALIZABLE],
            "TypeError",
            format!("Object of type {name} is not JSON serializable"),
        ),
        (DumpError::Circular, Some(Found::CircularList)) => (
            path.len(),
            vec![&RAISE_CIRCULAR],
            "ValueError",
            "Circular reference detected".to_string(),
        ),
        (DumpError::Circular, Some(Found::CircularDict)) => (
            path.len(),
            vec![&RAISE_CIRCULAR_DICT],
            "ValueError",
            "Circular reference detected".to_string(),
        ),
        (DumpError::IntegerTooLong, Some(Found::HugeInt)) => (
            path.len().saturating_sub(1),
            vec![match path.last() {
                Some(Container::List) => &YIELD_INT_LIST,
                _ => &YIELD_INT_DICT,
            }],
            "ValueError",
            INT_STR_LIMIT_MESSAGE.to_string(),
        ),
        (DumpError::KeyNotAllowed(name), Some(Found::BadKey)) => (
            path.len().saturating_sub(1),
            vec![&RAISE_BAD_KEY],
            "TypeError",
            format!("keys must be str, int, float, bool or None, not {name}"),
        ),
        (DumpError::TooDeep, Some(Found::TooDeep(child))) => (
            path.len().saturating_sub(1),
            vec![call_row(
                path.last().copied().unwrap_or(Container::Dict),
                child,
            )],
            "RecursionError",
            "maximum recursion depth exceeded".to_string(),
        ),
        (DumpError::TooDeep, Some(Found::FloatTooDeep)) => (
            path.len().saturating_sub(1),
            vec![match path.last() {
                Some(Container::List) => &FLOAT_IN_LIST,
                _ => &FLOAT_IN_DICT,
            }],
            "RecursionError",
            "maximum recursion depth exceeded".to_string(),
        ),
        (DumpError::TooDeep, Some(Found::FloatKeyTooDeep)) => (
            path.len().saturating_sub(1),
            vec![&FLOAT_KEY_IN_DICT],
            "RecursionError",
            "maximum recursion depth exceeded".to_string(),
        ),
        (DumpError::TooDeep, Some(Found::DefaultTooDeep)) => (
            path.len(),
            vec![&CALL_DEFAULT],
            "RecursionError",
            "maximum recursion depth exceeded".to_string(),
        ),
        _ => return Err(unsupported("no captured traceback exists for this failure")),
    };
    let mut frames = vec![
        owner_frame(owner, &MODULE_EXIT),
        owner_frame(owner, &MAIN_DUMPS),
        std_frame(&JSON_DUMPS),
        std_frame(&ENCODER_ENCODE),
        std_frame(&ENCODER_TOP),
    ];
    for container in path.iter().take(entered) {
        frames.push(std_frame(match container {
            Container::Dict => &YIELD_DICT,
            Container::List => &YIELD_LIST,
        }));
    }
    frames.extend(terminal.into_iter().map(std_frame));
    Ok(Answer::crash(&frames, class, &PyStr::new(&message)))
}

// ── the run ─────────────────────────────────────────────────────────────────────────────

const INT_STR_DIGIT_LIMIT: usize = 4300;
const INT_STR_LIMIT_MESSAGE: &str = "Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit";

/// Whether CPython 3.12 refuses to spell an integer written with these decimal digits (sign excluded).
fn too_many_digits(decimal: &str) -> bool {
    decimal.trim_start_matches('-').len() > INT_STR_DIGIT_LIMIT
}

/// `%s` of one result value in the plain line.
fn display(res: &PyDict, key: &str) -> Result<PyStr, Unsupported> {
    let Some(value) = res.get(key) else {
        return Ok(PyStr::new("None"));
    };
    if matches!(value.peel(), PyValue::Int(number) if too_many_digits(&number.to_string())) {
        return Err(Unsupported {
            boundary: "int_str_digit_limit",
            detail: format!("'{key}' is an integer beyond the 4300-digit limit; only the gpus_per_node position of the plain line was captured"),
        });
    }
    Ok(py_str(value))
}

/// The traceback of the plain line when `gpus_per_node` is an integer beyond the digit limit: the `%` formatting of the
/// captured original fails on that argument (frame line 454, marked `~~~^^^…` under `res["gpus_per_node"]`).
fn huge_gpus_traceback(owner: &[u8]) -> PyStr {
    const CODE: &str = "% (res[\"topology\"], res[\"gpus_per_node\"], res[\"manifest_tp\"], res[\"model_source\"]))";
    const MARKED: &str = "res[\"gpus_per_node\"]";
    let mut out = PyStr::new("Traceback (most recent call last):\n");
    push_frame(&mut out, &owner_frame(owner, &MODULE_EXIT));
    push_frame(
        &mut out,
        &Frame {
            file: owner.to_vec(),
            line: 454,
            function: "main",
            code: CODE,
            expr: "",
        },
    );
    let start = CODE.find(MARKED).expect("the argument is in the line");
    let subscript = MARKED.find('[').expect("a subscript");
    out.push_str(&format!(
        "    {}{}{}\n",
        " ".repeat(start),
        "~".repeat(subscript),
        "^".repeat(MARKED.len() - subscript)
    ));
    out.push_str("ValueError: ");
    out.push_str(INT_STR_LIMIT_MESSAGE);
    out.push_str("\n");
    out
}

fn crash_of_gate(owner: &[u8], raised: &Raised) -> Answer {
    let mut frames = vec![
        owner_frame(owner, &MODULE_EXIT),
        owner_frame(owner, &MAIN_EVALUATE),
    ];
    frames.extend(raised.frames.iter().map(|row| owner_frame(owner, row)));
    Answer::crash(&frames, raised.class, &raised.message)
}

/// `os.path.abspath(path)`: an absolute path never asks for the working directory; a relative one does, and CPython's `posixpath`
/// (a frozen module, no source line) raises the `OSError` of `os.getcwd()` when the directory was deleted. The traceback and
/// status are the owner's: nothing is substituted for the missing directory.
fn python_abspath(path: &[u8], owner: &[u8]) -> Result<Vec<u8>, Answer> {
    if path.first() != Some(&b'/') {
        if let Err(error) = std::env::current_dir() {
            return Err(getcwd_failure(owner, &error));
        }
    }
    Ok(abspath(path))
}

fn getcwd_failure(owner: &[u8], error: &std::io::Error) -> Answer {
    let number = error.raw_os_error().unwrap_or(0);
    // the subclass CPython picks for the errno
    let class = match number {
        2 => "FileNotFoundError",
        1 | 13 => "PermissionError",
        _ => "OSError",
    };
    let full = error.to_string();
    let text = full
        .strip_suffix(&format!(" (os error {number})"))
        .unwrap_or(&full);
    let frames = [
        owner_frame(owner, &MODULE_EXIT),
        owner_frame(owner, &MAIN_ABSPATH),
        Frame {
            file: b"<frozen posixpath>".to_vec(),
            line: 420,
            function: "abspath",
            code: "",
            expr: "",
        },
    ];
    Answer::crash(
        &frames,
        class,
        &PyStr::new(&format!("[Errno {number}] {text}")),
    )
}

/// `main()` of the owner for an exact request. `owner` is the owner program's path (the `__file__` of its frames).
pub fn execute(
    request: &Request,
    owner: &[u8],
    probe: &mut dyn BranchProbe,
) -> Result<Answer, Unsupported> {
    let repo = match python_abspath(&request.repo, owner) {
        Ok(repo) => repo,
        Err(answer) => return Ok(answer),
    };
    let explicit = request.topology;
    let topology_value = || explicit.map_or(PyValue::None, PyValue::str);
    if let Some(conflict) = branch_topology_conflict(probe, &repo, explicit) {
        return Ok(if request.json {
            Answer::printed(
                refusal_json(&conflict, topology_value())?,
                PrintSite::Conflict,
                EXIT_TOPOLOGY_MISMATCH,
            )
        } else {
            Answer::message(conflict, EXIT_TOPOLOGY_MISMATCH)
        });
    }
    let Some(topology) = resolve_topology(probe, &repo, explicit) else {
        return Ok(Answer::message(
            PyStr::new(
                "topology 불명 — --topology 명시 필요(git 브랜치가 single-node/multi-node 아님)",
            ),
            EXIT_USAGE,
        ));
    };

    let manifest = join(
        &join(&join(&repo, b"output"), topology.as_bytes()),
        b"manifest.yaml",
    );
    let manifest_text = fs_decode(&manifest);
    let is_file = std::fs::metadata(OsStr::from_bytes(&manifest))
        .map(|meta| meta.is_file())
        .unwrap_or(false);
    if !is_file {
        let mut message = PyStr::new("manifest 부재: ");
        message.push_py(&manifest_text);
        message.push_str(" (테라포밍 미완 — info-only)");
        return Ok(if request.json {
            Answer::printed(
                refusal_json(&message, PyValue::str(&topology))?,
                PrintSite::Missing,
                EXIT_NO_MANIFEST,
            )
        } else {
            Answer::message(message, EXIT_NO_MANIFEST)
        });
    }

    let loaded = match load_yaml_objects(&PyValue::Str(manifest_text.clone())) {
        Ok(value) if value.is_truthy() => value,
        Ok(_) => PyValue::Dict(PyDict::new()),
        Err(failure) => return parse_failure(failure),
    };

    let evaluation = match evaluate_contract(&loaded, &topology) {
        Ok(evaluation) => evaluation,
        Err(GateFailure::Raised(raised)) => return Ok(crash_of_gate(owner, &raised)),
        Err(GateFailure::Unsupported { boundary, detail }) => {
            return Err(Unsupported { boundary, detail })
        }
    };
    let (flag, code) = (evaluation.flag(), evaluation.exit_code());
    let mut res = evaluation.res;
    res.set("manifest_path", PyValue::Str(manifest_text));
    if !flag {
        res.set("redirect", PyValue::str(REDIRECT_TEMPLATE));
    }
    // `return res["exit_code"] if args.require_flag else (EXIT_OK if res["flag"] else res["exit_code"])`
    let status = if request.require_flag {
        code
    } else if flag {
        0
    } else {
        code
    };
    if request.json {
        return match PyValue::Dict(res.clone()).dumps_pretty() {
            // the dumper spells an integer key of any size; CPython refuses one beyond the digit limit at `_intstr(key)`
            Ok(_) if has_huge_int_key(&res) => {
                dumps_failure(owner, &res, DumpError::IntegerTooLong)
            }
            Ok(text) => Ok(Answer::printed(text, PrintSite::Json, status)),
            Err(error) => dumps_failure(owner, &res, error),
        };
    }
    if flag {
        if matches!(res.get("gpus_per_node").map(PyValue::peel), Some(PyValue::Int(number)) if too_many_digits(&number.to_string()))
        {
            return Ok(Answer {
                stdout: None,
                stderr: Some(huge_gpus_traceback(owner)),
                status: 1,
                crashed: true,
                exception: Some(("ValueError".to_string(), PyStr::new(INT_STR_LIMIT_MESSAGE))),
            });
        }
        let mut line = PyStr::new("Flag valid · topology=");
        for (key, label) in [
            ("topology", ""),
            ("gpus_per_node", " · gpus_per_node="),
            ("manifest_tp", " · manifest_tp="),
            ("model_source", " · model_source="),
        ] {
            line.push_str(label);
            line.push_py(&display(&res, key)?);
        }
        Ok(Answer::printed(line, PrintSite::Plain, status))
    } else {
        let mut message = PyStr::new("Flag 미발급: ");
        message.push_py(&display(&res, "reason")?);
        Ok(Answer::message(message, status))
    }
}

/// `print("manifest 파싱 실패: %s" % e, file=sys.stderr)` for the exception `yaml.safe_load(open(...))` raised.
fn parse_failure(failure: LoadFailure) -> Result<Answer, Unsupported> {
    let text = match failure {
        LoadFailure::Os { error, filename } => os_error_text(
            &error,
            &filename.to_surrogateescape_bytes().unwrap_or_default(),
        ),
        LoadFailure::Yaml(error) => error.message.clone(),
        LoadFailure::NotPathLike(_) | LoadFailure::EmbeddedNull => {
            return Err(Unsupported {
                boundary: "manifest_path_not_openable_text",
                detail: "an argument-derived path cannot be a non-path or contain a NUL"
                    .to_string(),
            })
        }
        LoadFailure::Unsupported { boundary, detail } => {
            return Err(Unsupported { boundary, detail })
        }
    };
    let mut message = PyStr::new("manifest 파싱 실패: ");
    message.push_py(&text);
    Ok(Answer::message(message, EXIT_USAGE))
}
