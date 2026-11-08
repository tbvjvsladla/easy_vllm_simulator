//! `topology_parity_api`: the Rust-only typed API surface of the topology predicate and the campaign read side, a separate
//! program so that the `topology_parity` command line keeps answering exactly what the owner CLI did. It is the bridge for
//! checks that observe the original's *functions*, and it calls the actual native functions of
//! `role_contract::{topology, campaign}`; it stores no expected answer and no case id.
//!
//! ```text
//! topology_parity_api evaluate-jsonl      one JSON request per stdin line, one JSON answer per stdout line
//! topology_parity_api native-help         this text
//! ```
//!
//! Request: `{"id": ANY, "operation": NAME, "arguments": {...}, "source_owner": PATH?, "module_alias": NAME?,
//! "compat_owner": PATH?}`.
//! `source_owner` is the logical owner path shown in frames (metadata); `module_alias` is the loader alias the observer gave the
//! original module (the module name of classes the original defines, e.g. `PurgeGateRefusal`); `compat_owner` is the REAL
//! original owner program the call stands for (its bytes decide the route, see below).
//!
//! | operation | arguments | answer value |
//! |---|---|---|
//! | `read_layer_header` | `path` | `tuple[topology or None, str]` |
//! | `layer_files` | `repo` | `list[Path]` |
//! | `evaluate` | `repo` (default `"."`), `expect` (default `null`) | the verdict `dict` in construction order |
//! | `active_campaign_id` | `module`, `repo_root` (or `null`) | `str` |
//! | `read_declaration` | `module`, `base` | `dict` |
//! | `campaigns_dir` | `module`, `repo_root` (or `null`) | `Path` |
//! | `derive_path` | `module`, `kind`, `cell`, `sweep`, `context`, `camp_id`, `repo_root` | `Path`, or the `PurgeGateRefusal` |
//!
//! `module` is the checkout path of `campaign_init.py` (`REPO_ROOT` is `Path(module).resolve().parents[4]`); it is read as bytes
//! and a path only, never imported or run.
//!
//! **Routes.** The native answer is the original's only for the *reviewed* sources: the campaign module (and, when named, the
//! owner) must be byte-identical to the canonical originals (`role_contract::source_identity`). For any other source the
//! answer is `kind: "unmet_condition"` with `route: "LegacySourceVariant"` - the source-version compatibility route needs
//! the approved generic original-function carrier, which this bridge does not contain and never imitates.
//!
//! **Unmet answers execute nothing** (`executed: false` on every `unmet_condition` answer). Their `route` is
//! `"LegacySourceVariant"` when the source calls for the compatibility route but it could not be served (the route the source
//! requires - not a claim that it ran), and `"NotExecuted"` for every other unmet condition (a rejected `compat.deadline_ms`,
//! a missing argument, an owner gate): such an answer never claims `NativeCurrentSource`, a program that did not run.
//! `compat.deadline_ms`, when present, must be an integer `>= 0` whose deadline the clock can represent (0 is an already
//! expired deadline, kept as declared); anything else is the unmet condition `InvalidCompatDeadline`, never a default.
//! A `legacy_observation` / `legacy_control_incomplete` observation echoes the executed bridge script as `bridge_script`.
//!
//! **Answer**: `{"id", "kind": "value"|"exception"|"unmet_condition"|"incomplete", "route", "value"|"exception"|"condition",
//! "bridge": {...}}`.
//! * `value`: the typed DTO tagging of the project's recorders: `["str", s]`, `["int", "digits"]`, `["float", repr]`,
//!   `["bool", b]`, `["none"]`, `["list", [...]]`, `["tuple", [...]]`, `["dict", [[k, v], ...]]` (insertion order),
//!   `["bytes", "hex"]`, `["path", {"module": "pathlib", "type": "PosixPath", "value": s}]`. A value with no declared
//!   tag (date, datetime, set, YAML marks) makes the answer `kind: "incomplete"` (the control is incomplete; nothing is
//!   faked) and names the paths in `bridge.unsupported_observations`.
//! * `exception`: `{"type", "module", "message", "args": [DTO], "suppress_context": false, "relation": null,
//!   "attributes": {name: DTO}, "frames": [{"file", "line", "function"}]}` - the business exception of the original
//!   function (per-class typed `args` and public attributes). `module` is `module_alias` for classes the original module
//!   defines and `builtins` otherwise. `frames` are the **logical** frames of the original source from the called function
//!   down; they are source metadata, not Rust or Python OS frames.
//! * `bridge` (answer level, never inside the business record): `frame_code` (the source line of each frame),
//!   `frames_complete: false` (the caller's frames are not part of the business frames and native code has no Python
//!   caller), `observer_frames: "none"`, `attributes_incomplete`, `module_alias_missing`, `bridge_error`.

use crate::captured_process::{run_captured_in, ChildSettings};
use crate::cli::{write_stderr, Streams};
use crate::pystr::PyStr as ProcessStr;
use agent_runtime_support::pyjson::{decode_file_text, loads_objects};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use role_contract::campaign::{self, Derived, ModuleSite};
use role_contract::pyprofile::{with_profile, TextProfile};
use role_contract::source_identity::{identify, route, Identity, Route, REVIEWED_CAMPAIGN};
use role_contract::topology::{self, Crash, Detail, Frame, Where};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const NATIVE_HELP: &str = "topology_parity_api Rust-only commands:\n  topology_parity_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments[, source_owner, module_alias, compat_owner]}\n  topology_parity_api native-help      this text\n";
const DEFAULT_OWNER: &str = ".claude/policies/runtime/topology_parity.py";

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn tagged(tag: &str, value: PyValue) -> PyValue {
    PyValue::List(vec![s(tag), value])
}

/// The DTO of a value; paths of values with no declared tag go to `unsupported`.
fn dto(value: &PyValue, at: &str, unsupported: &mut Vec<String>) -> PyValue {
    match value.peel() {
        PyValue::None => PyValue::List(vec![s("none")]),
        PyValue::Bool(b) => tagged("bool", PyValue::Bool(*b)),
        PyValue::Int(n) => tagged("int", s(&n.to_string())),
        PyValue::Float(_) => tagged("float", s(&value.repr())),
        PyValue::Str(text) => tagged("str", PyValue::Str(text.clone())),
        PyValue::Bytes(bytes) => tagged(
            "bytes",
            s(&bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()),
        ),
        PyValue::List(items) => tagged(
            "list",
            PyValue::List(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| dto(v, &format!("{at}[{i}]"), unsupported))
                    .collect(),
            ),
        ),
        PyValue::Tuple(items) => tagged(
            "tuple",
            PyValue::List(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| dto(v, &format!("{at}[{i}]"), unsupported))
                    .collect(),
            ),
        ),
        PyValue::Dict(dict) => tagged(
            "dict",
            PyValue::List(
                dict.keyed_entries()
                    .map(|(_, key, v)| {
                        PyValue::List(vec![
                            tagged("str", PyValue::Str(key.clone())),
                            dto(v, &format!("{at}[{}]", key.repr()), unsupported),
                        ])
                    })
                    .collect(),
            ),
        ),
        other => {
            unsupported.push(format!("{at}: {}", other.type_name()));
            let mut map = PyDict::new();
            map.set("type", s(other.type_name()));
            map.set("repr", s(&other.repr()));
            tagged("opaque", PyValue::Dict(map))
        }
    }
}

fn path_dto(bytes: &[u8]) -> PyValue {
    let mut map = PyDict::new();
    map.set("module", s("pathlib"));
    map.set("type", s("PosixPath"));
    map.set(
        "value",
        PyValue::Str(role_contract::pyprofile::fs_decode(bytes)),
    );
    tagged("path", PyValue::Dict(map))
}

/// What one operation produced.
enum Outcome {
    Value(PyValue),
    Exception(Box<ExceptionRecord>),
    Unmet(String),
    /// The complete observation of the source-version compatibility route (the actual original function ran in a child).
    Legacy(PyValue),
    /// The compatibility child hit the private transport limit or could not be run: incomplete control, no business result.
    LegacyControlIncomplete(PyValue),
}

/// What the caller configures for the source-version compatibility route (project policy defaults apply when absent).
struct Context {
    owner: Vec<u8>,
    compat_owner: Option<Vec<u8>>,
    alias: Option<String>,
    id: PyValue,
    deadline_ms: u64,
    /// Why the declared `compat.deadline_ms` is not a valid deadline (N11: rejected, never defaulted).
    deadline_error: Option<String>,
    compat_python: Vec<u8>,
    bridge_script: Option<Vec<u8>>,
}

/// An exception of the original: the business record and the bridge metadata kept apart from it.
struct ExceptionRecord {
    class: String,
    defined_in_original_module: bool,
    message: PyStr,
    args: Vec<PyValue>,
    attributes: Vec<(String, PyValue)>,
    frames: Vec<Frame>,
    attributes_incomplete: Vec<String>,
    /// The implicit context or explicit cause, as the original raised it (`{kind, exception}`), else none.
    relation: Option<PyValue>,
}

struct Bridge(String);

fn frames_of(
    crash: &Crash,
    owner: &[u8],
    via_evaluate: bool,
    wrapper: Option<Frame>,
) -> Vec<Frame> {
    let site = |line, function, code, expr, file: &[u8]| Frame {
        file: file.to_vec(),
        line,
        function,
        code,
        expr,
    };
    let mut frames: Vec<Frame> = Vec::new();
    match &crash.site {
        Where::Evaluate { line, code, expr } => {
            frames.push(site(*line, "evaluate", code, expr, owner))
        }
        Where::GitStderr {
            call_line,
            call_code,
            call_expr,
        } => {
            frames.push(site(*call_line, "evaluate", call_code, call_expr, owner));
            frames.push(site(
                87,
                "_git",
                "proc = subprocess.run([\"git\", \"-C\", str(repo), *args],",
                "subprocess.run([\"git\", \"-C\", str(repo), *args],",
                owner,
            ));
            frames.extend(topology::subprocess_frames());
        }
        Where::Campaign {
            module,
            frames: below,
        } => {
            if via_evaluate {
                frames.push(site(
                    262,
                    "evaluate",
                    "camp_id = mod.active_campaign_id(str(root))",
                    "mod.active_campaign_id(str(root))",
                    owner,
                ));
            }
            if let Some(wrapper) = wrapper {
                frames.push(wrapper);
            }
            if !below.is_empty() {
                frames.push(site(
                    113,
                    "active_campaign_id",
                    "name = pointer.read_text(encoding=\"utf-8\").strip()",
                    "pointer.read_text(encoding=\"utf-8\")",
                    module,
                ));
                frames.extend(below.iter().cloned());
            }
        }
        Where::Print | Where::Opaque => {}
    }
    frames
}

/// The business exception record of a crash: per-class typed constructor arguments and public attributes.
/// The implicit context of `Path.resolve`'s `RuntimeError`: the `OSError` (`errno`) of `stat(filename)` it handled.
fn os_context(errno: i32, filename: &[u8]) -> PyValue {
    let error = std::io::Error::from_raw_os_error(errno);
    let full = error.to_string();
    let strerror = full
        .strip_suffix(&format!(" (os error {errno})"))
        .unwrap_or(&full)
        .to_string();
    let mut attributes = PyDict::new();
    attributes.set("errno", tagged("int", s(&errno.to_string())));
    attributes.set("strerror", tagged("str", s(&strerror)));
    attributes.set(
        "filename",
        tagged(
            "str",
            PyValue::Str(role_contract::pyprofile::fs_decode(filename)),
        ),
    );
    attributes.set("filename2", PyValue::List(vec![s("none")]));
    let mut exception = PyDict::new();
    exception.set("type", s("OSError"));
    exception.set("module", s("builtins"));
    exception.set(
        "message",
        PyValue::Str(campaign::os_error_text(&error, filename)),
    );
    exception.set(
        "args",
        PyValue::List(vec![
            tagged("int", s(&errno.to_string())),
            tagged("str", s(&strerror)),
        ]),
    );
    exception.set("suppress_context", PyValue::Bool(false));
    exception.set("relation", PyValue::None);
    exception.set("attributes", PyValue::Dict(attributes));
    exception.set("frames", PyValue::List(Vec::new()));
    let mut relation = PyDict::new();
    relation.set("kind", s("context"));
    relation.set("exception", PyValue::Dict(exception));
    PyValue::Dict(relation)
}

fn record_of(
    crash: &Crash,
    owner: &[u8],
    via_evaluate: bool,
    wrapper: Option<Frame>,
) -> ExceptionRecord {
    let message = crash.message.clone();
    let mut attributes: Vec<(String, PyValue)> = Vec::new();
    let mut incomplete: Vec<String> = Vec::new();
    let mut relation: Option<PyValue> = None;
    let args: Vec<PyValue> = match &*crash.detail {
        Detail::Plain => vec![tagged("str", PyValue::Str(message.clone()))],
        Detail::Os {
            errno,
            strerror,
            filename,
        } => {
            attributes.push(("errno".to_string(), tagged("int", s(&errno.to_string()))));
            attributes.push(("strerror".to_string(), tagged("str", s(strerror))));
            attributes.push((
                "filename".to_string(),
                tagged(
                    "str",
                    PyValue::Str(role_contract::pyprofile::fs_decode(filename)),
                ),
            ));
            attributes.push(("filename2".to_string(), PyValue::List(vec![s("none")])));
            vec![
                tagged("int", s(&errno.to_string())),
                tagged("str", s(strerror)),
            ]
        }
        Detail::Decode {
            encoding,
            object,
            start,
            end,
            reason,
        } => {
            let hex: String = object.iter().map(|b| format!("{b:02x}")).collect();
            let typed = [
                ("encoding", tagged("str", s(encoding))),
                ("object", tagged("bytes", s(&hex))),
                ("start", tagged("int", s(&start.to_string()))),
                ("end", tagged("int", s(&end.to_string()))),
                ("reason", tagged("str", s(reason))),
            ];
            for (name, value) in &typed {
                attributes.push((name.to_string(), value.clone()));
            }
            typed.iter().map(|(_, v)| v.clone()).collect()
        }
        Detail::Encode {
            encoding,
            object,
            start,
            end,
            reason,
        } => {
            let typed = [
                ("encoding", tagged("str", s(encoding))),
                ("object", tagged("str", PyValue::Str(object.clone()))),
                ("start", tagged("int", s(&start.to_string()))),
                ("end", tagged("int", s(&end.to_string()))),
                ("reason", tagged("str", s(reason))),
            ];
            for (name, value) in &typed {
                attributes.push((name.to_string(), value.clone()));
            }
            typed.iter().map(|(_, v)| v.clone()).collect()
        }
        Detail::SymlinkLoop { filename } => {
            relation = Some(os_context(40, filename));
            vec![tagged("str", PyValue::Str(message.clone()))]
        }
        Detail::SourceUnmatched => vec![tagged("str", PyValue::Str(message.clone()))],
        Detail::Attribute { name, obj } => {
            let mut unsupported = Vec::new();
            attributes.push(("name".to_string(), tagged("str", s(name))));
            attributes.push(("obj".to_string(), dto(obj, "obj", &mut unsupported)));
            incomplete.extend(unsupported);
            vec![tagged("str", PyValue::Str(message.clone()))]
        }
    };
    ExceptionRecord {
        class: crash.class.to_string(),
        defined_in_original_module: false,
        message,
        args,
        attributes,
        frames: frames_of(crash, owner, via_evaluate, wrapper),
        attributes_incomplete: incomplete,
        relation,
    }
}

fn frame_value(frame: &Frame) -> PyValue {
    let mut map = PyDict::new();
    map.set(
        "file",
        PyValue::Str(role_contract::pyprofile::fs_decode(&frame.file)),
    );
    map.set("line", PyValue::int(frame.line));
    map.set("function", s(frame.function));
    PyValue::Dict(map)
}

fn exception_json(record: &ExceptionRecord, alias: Option<&str>) -> (PyValue, PyValue) {
    let mut exception = PyDict::new();
    exception.set("type", s(&record.class));
    let module = if record.defined_in_original_module {
        alias.map_or(PyValue::None, s)
    } else {
        s("builtins")
    };
    exception.set("module", module);
    exception.set("message", PyValue::Str(record.message.clone()));
    exception.set("args", PyValue::List(record.args.clone()));
    exception.set("suppress_context", PyValue::Bool(false));
    exception.set("relation", record.relation.clone().unwrap_or(PyValue::None));
    let mut attributes = PyDict::new();
    for (name, value) in &record.attributes {
        attributes.set(name, value.clone());
    }
    exception.set("attributes", PyValue::Dict(attributes));
    exception.set(
        "frames",
        PyValue::List(record.frames.iter().map(frame_value).collect()),
    );
    let mut bridge = PyDict::new();
    bridge.set(
        "frame_code",
        PyValue::List(record.frames.iter().map(|f| s(f.code)).collect()),
    );
    bridge.set("frames_complete", PyValue::Bool(false));
    bridge.set(
        "observer_frames",
        s("none: the caller is a native process (no Python caller frames)"),
    );
    bridge.set(
        "attributes_incomplete",
        PyValue::List(record.attributes_incomplete.iter().map(|a| s(a)).collect()),
    );
    bridge.set(
        "module_alias_missing",
        PyValue::Bool(record.defined_in_original_module && alias.is_none()),
    );
    (PyValue::Dict(exception), PyValue::Dict(bridge))
}

// ── argument decoding ───────────────────────────────────────────────────────────

fn text_of(value: &PyValue) -> Option<PyStr> {
    match value.peel() {
        PyValue::Str(text) => Some(text.clone()),
        _ => None,
    }
}

/// A path-like argument: a string, or `{"type": "Path", "value": string}`.
fn path_arg(args: &PyDict, name: &str, default: Option<&str>) -> Result<Vec<u8>, Bridge> {
    let value = match args.get(name) {
        Some(value) => value.clone(),
        None => match default {
            Some(text) => s(text),
            None => return Err(Bridge(format!("missing required argument: '{name}'"))),
        },
    };
    let text = match value.peel() {
        PyValue::Str(text) => Some(text.clone()),
        PyValue::Dict(d) => match (d.get("type"), d.get("value")) {
            (Some(kind), Some(inner)) if matches!(kind.peel(), PyValue::Str(k) if k.equals("Path")) => {
                text_of(inner)
            }
            _ => None,
        },
        _ => None,
    };
    let text =
        text.ok_or_else(|| Bridge(format!("argument '{name}' must be a str or a Path object")))?;
    role_contract::pyprofile::fs_encode(&text).map_err(|why| {
        Bridge(format!(
            "argument '{name}' cannot be encoded for the file system: {why}"
        ))
    })
}

fn optional_path(args: &PyDict, name: &str) -> Result<Option<Vec<u8>>, Bridge> {
    match args.get(name).map(PyValue::peel) {
        None | Some(PyValue::None) => Ok(None),
        Some(_) => path_arg(args, name, None).map(Some),
    }
}

fn optional_text(args: &PyDict, name: &str) -> Result<Option<String>, Bridge> {
    match args.get(name).map(PyValue::peel) {
        None | Some(PyValue::None) => Ok(None),
        Some(PyValue::Str(text)) => Ok(Some(text.to_utf8().unwrap_or_default())),
        Some(_) => Err(Bridge(format!("argument '{name}' must be a str or null"))),
    }
}

/// `(module root, module file)`: `Path(module).resolve().parents[4]`.
fn module_site(args: &PyDict) -> Result<(Vec<u8>, Vec<u8>), Result<Outcome, Bridge>> {
    let module = path_arg(args, "module", None).map_err(Err)?;
    let resolved = match topology::resolve_checked(&module) {
        Ok(resolved) => resolved,
        Err(crash) => {
            return Err(Ok(Outcome::Exception(Box::new(record_of(
                &crash,
                DEFAULT_OWNER.as_bytes(),
                false,
                None,
            )))))
        }
    };
    let parts: Vec<&[u8]> = resolved
        .split(|b| *b == b'/')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 5 {
        // `Path.parents[4]` raises IndexError(4): a plain int argument
        let record = ExceptionRecord {
            class: "IndexError".to_string(),
            defined_in_original_module: false,
            message: PyStr::new("4"),
            args: vec![tagged("int", s("4"))],
            attributes: Vec::new(),
            frames: Vec::new(),
            attributes_incomplete: Vec::new(),
            relation: None,
        };
        return Err(Ok(Outcome::Exception(Box::new(record))));
    }
    let mut root = Vec::new();
    for part in &parts[..parts.len() - 5] {
        root.push(b'/');
        root.extend_from_slice(part);
    }
    if root.is_empty() {
        root.push(b'/');
    }
    Ok((root, module))
}

// ── operations ──────────────────────────────────────────────────────────────────

fn unmet(route: &str, why: &str) -> Outcome {
    Outcome::Unmet(format!("{route}: {why}"))
}

fn input_dto(value: &PyValue) -> PyValue {
    match value.peel() {
        PyValue::Str(text) => tagged("str", PyValue::Str(text.clone())),
        PyValue::Dict(d) => match (d.get("type"), d.get("value")) {
            (Some(kind), Some(inner)) if matches!(kind.peel(), PyValue::Str(k) if k.equals("Path")) => {
                tagged("path", inner.clone())
            }
            _ => dto(value, "argument", &mut Vec::new()),
        },
        other => dto(other, "argument", &mut Vec::new()),
    }
}

fn find_scripts_bridge() -> Option<Vec<u8>> {
    let exe = std::env::current_exe().ok()?;
    for ancestor in exe.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "rust") {
            let script = ancestor
                .parent()?
                .join("scripts")
                .join("topology_legacy_api.py");
            return script
                .is_file()
                .then(|| std::os::unix::ffi::OsStrExt::as_bytes(script.as_os_str()).to_vec());
        }
    }
    None
}

/// A private, unpredictable, exclusive `0700` directory for one compatibility call; removed with everything in it on every
/// return path.
struct PrivateDir(std::path::PathBuf);

impl PrivateDir {
    fn create() -> Result<PrivateDir, String> {
        use std::io::Read;
        use std::os::unix::fs::DirBuilderExt;
        let mut random = [0u8; 12];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|error| format!("no randomness for the private directory: {error}"))?;
        let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir().join(format!("topology_legacy_{name}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|error| format!("the private directory cannot be created: {error}"))?;
        Ok(PrivateDir(path))
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The source-version compatibility route of an original function: the approved generic child
/// (`topology_legacy_api.py --mode SourceVersionCompat --request ABS --result ABS`) runs the REAL selected module under the
/// configured interpreter through the bounded capture (own process group, private transport limit); its raw stdout/stderr,
/// exit status, PID and exclusive typed sidecar are returned together, unchanged. A relative real module path is made
/// absolute against the current directory; nothing is retried and no field is filled in. A transport deadline or control
/// failure is a `legacy_control_incomplete` observation, never a business result.
fn legacy(
    ctx: &Context,
    operation: &str,
    args: &PyDict,
    module: Option<&[u8]>,
    reason: &str,
) -> Outcome {
    use std::io::Write;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;
    let Some(module) = module.filter(|m| !m.is_empty() && !m.contains(&0)) else {
        return unmet("LegacySourceVariant", &format!("{reason}; the real original module for this operation must be named by `compat_owner` (or `module`)"));
    };
    let module = topology::abspath(module);
    let Some(script) = ctx.bridge_script.clone().or_else(find_scripts_bridge) else {
        return unmet("LegacySourceVariant", &format!("{reason}; the approved generic bridge topology_legacy_api.py is not installed beside the native programs and the request names none"));
    };
    let get = |name: &str, default: Option<&str>| -> Option<PyValue> {
        args.get(name).cloned().or_else(|| default.map(s))
    };
    let (positional, keywords): (&[&str], &[&str]) = match operation {
        "read_layer_header" => (&["path"], &[]),
        "layer_files" => (&["repo"], &[]),
        "evaluate" => (&["repo"], &["expect"]),
        "active_campaign_id" | "campaigns_dir" => (&[], &["repo_root"]),
        "read_declaration" => (&["base"], &[]),
        _ => (
            &["kind"],
            &["cell", "sweep", "context", "camp_id", "repo_root"],
        ),
    };
    let mut args_dto = Vec::new();
    for name in positional {
        let default = if operation == "evaluate" {
            Some(".")
        } else {
            None
        };
        match get(name, default) {
            Some(value) => args_dto.push(input_dto(&value)),
            None => return unmet("Unmet", &format!("missing required argument: '{name}'")),
        }
    }
    let mut kwargs_dto = Vec::new();
    for name in keywords {
        if let Some(value) = args.get(name) {
            kwargs_dto.push(PyValue::List(vec![
                tagged("str", s(name)),
                input_dto(value),
            ]));
        }
    }
    let alias = ctx
        .alias
        .clone()
        .unwrap_or_else(|| "topology_legacy_module".to_string());
    let mut request = PyDict::new();
    request.set("schema", s("topology-legacy-api/1"));
    request.set("mode", s("SourceVersionCompat"));
    request.set("id", ctx.id.clone());
    request.set(
        "module_path",
        PyValue::Str(role_contract::pyprofile::fs_decode(&module)),
    );
    request.set("module_alias", s(&alias));
    request.set("operation", s(operation));
    request.set("args_dto", tagged("list", PyValue::List(args_dto)));
    request.set("kwargs_dto", tagged("dict", PyValue::List(kwargs_dto)));
    let mut logical = PyDict::new();
    logical.set(
        "source_owner",
        PyValue::Str(role_contract::pyprofile::fs_decode(&ctx.owner)),
    );
    request.set("logical_source_context", PyValue::Dict(logical));
    let rendered = match PyValue::Dict(request.clone()).dumps_ascii() {
        Ok(text) => text,
        Err(_) => return unmet("Unmet", "the request cannot be rendered as JSON"),
    };
    let directory = match PrivateDir::create() {
        Ok(directory) => directory,
        Err(why) => return unmet("Unmet", &why),
    };
    let request_path = directory.0.join("request.json");
    let result_path = directory.0.join("result.json");
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&request_path)
        .and_then(|mut file| file.write_all(rendered.as_bytes()));
    if let Err(error) = written {
        return unmet(
            "Unmet",
            &format!("the request file cannot be written: {error}"),
        );
    }
    let words: Vec<std::ffi::OsString> = vec![
        std::ffi::OsStr::from_bytes(&ctx.compat_python).to_os_string(),
        "-B".into(),
        std::ffi::OsStr::from_bytes(&script).to_os_string(),
        "--mode".into(),
        "SourceVersionCompat".into(),
        "--request".into(),
        request_path.clone().into_os_string(),
        "--result".into(),
        result_path.clone().into_os_string(),
    ];
    let shown: Vec<crate::pyvalue::PyValue> = words
        .iter()
        .map(|w| crate::pyvalue::PyValue::Str(ProcessStr::from_surrogateescape(w.as_bytes())))
        .collect();
    let pid = Arc::new(AtomicU32::new(0));
    // stdin DEVNULL as the approved child requires: the capture's `input` pipe would not be a null device, so the child
    // gets its null stdin through the shell-free `Stdio` default of the settings below
    let settings = ChildSettings {
        own_process_group: true,
        pid_sink: Some(pid.clone()),
        null_stdin: true,
        ..ChildSettings::default()
    };
    let limit = Duration::from_millis(ctx.deadline_ms);
    let limit_text = format!("{}", ctx.deadline_ms as f64 / 1000.0);
    let outcome = run_captured_in(&words, &shown, limit, &limit_text, &settings);
    let child_pid = pid.load(Ordering::SeqCst);
    let hex = |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() };
    let mut observation = PyDict::new();
    observation.set(
        "child_pid",
        if child_pid == 0 {
            PyValue::None
        } else {
            PyValue::int(child_pid)
        },
    );
    observation.set(
        "interpreter",
        PyValue::Str(role_contract::pyprofile::fs_decode(&ctx.compat_python)),
    );
    observation.set(
        "module_path",
        PyValue::Str(role_contract::pyprofile::fs_decode(&module)),
    );
    observation.set(
        "bridge_script",
        PyValue::Str(role_contract::pyprofile::fs_decode(&script)),
    );
    observation.set("limit_ms", PyValue::int(ctx.deadline_ms));
    observation.set("request", PyValue::Dict(request));
    let captured = match outcome {
        Ok(captured) => captured,
        Err(failure) => {
            observation.set("control_failure", s(failure.class));
            observation.set(
                "control_message",
                PyValue::Str(PyStr::from_surrogateescape(
                    failure.message.to_utf8().unwrap_or_default().as_bytes(),
                )),
            );
            observation.set("complete", PyValue::Bool(false));
            return Outcome::LegacyControlIncomplete(PyValue::Dict(observation));
        }
    };
    observation.set(
        "exit_code",
        if captured.status < 0 {
            PyValue::None
        } else {
            PyValue::int(captured.status)
        },
    );
    if captured.status < 0 {
        observation.set("signal", PyValue::int(-captured.status));
    }
    observation.set("stdout_hex", s(&hex(&captured.stdout)));
    observation.set("stderr_hex", s(&hex(&captured.stderr)));
    match std::fs::read(&result_path) {
        Ok(bytes) => match decode_file_text(&bytes)
            .ok()
            .and_then(|t| t.to_utf8())
            .map(|t| loads_objects(&t))
        {
            Some(Ok(sidecar)) => observation.set("sidecar", sidecar),
            _ => observation.set("sidecar_error", s("the sidecar is not readable JSON")),
        },
        Err(error) => observation.set("sidecar_error", s(&format!("no sidecar: {error}"))),
    }
    Outcome::Legacy(PyValue::Dict(observation))
}

fn run_operation(
    operation: &str,
    args: &PyDict,
    ctx: &Context,
    unsupported: &mut Vec<String>,
) -> Result<Outcome, Bridge> {
    let (owner, compat_owner) = (ctx.owner.as_slice(), ctx.compat_owner.as_deref());
    let owner_gate = |root: Option<&[u8]>| route(compat_owner, root);
    match operation {
        "read_layer_header" => {
            let path = path_arg(args, "path", None)?;
            if let Route::Unmet { reason } = owner_gate(None) {
                return Ok(unmet("Unmet", &reason));
            }
            if let Route::LegacySourceVariant { reason } = owner_gate(None) {
                return Ok(legacy(ctx, operation, args, compat_owner, &reason));
            }
            let (value, why) = topology::read_layer_header(&path);
            Ok(Outcome::Value(dto(
                &PyValue::Tuple(vec![value.map_or(PyValue::None, s), PyValue::Str(why)]),
                "value",
                unsupported,
            )))
        }
        "layer_files" => {
            let repo = path_arg(args, "repo", None)?;
            if let Route::Unmet { reason } = owner_gate(None) {
                return Ok(unmet("Unmet", &reason));
            }
            if let Route::LegacySourceVariant { reason } = owner_gate(None) {
                return Ok(legacy(ctx, operation, args, compat_owner, &reason));
            }
            Ok(match topology::layer_files(&repo) {
                Ok(files) => Outcome::Value(tagged(
                    "list",
                    PyValue::List(files.iter().map(|f| path_dto(f)).collect()),
                )),
                Err(crash) => Outcome::Exception(Box::new(record_of(&crash, owner, false, None))),
            })
        }
        "evaluate" => {
            let repo = path_arg(args, "repo", Some("."))?;
            let expect = match args.get("expect").map(PyValue::peel) {
                None | Some(PyValue::None) => None,
                Some(PyValue::Str(text)) => Some(text.to_utf8().unwrap_or_default()),
                Some(_) => {
                    return Err(Bridge(
                        "argument 'expect' must be a str or null".to_string(),
                    ))
                }
            };
            let root = topology::resolve(&repo);
            if let Route::Unmet { reason } = owner_gate(Some(&root)) {
                return Ok(unmet("Unmet", &reason));
            }
            if let Route::LegacySourceVariant { reason } = owner_gate(Some(&root)) {
                return Ok(legacy(ctx, operation, args, compat_owner, &reason));
            }
            Ok(match topology::evaluate(&repo, expect.as_deref()) {
                Ok(report) => Outcome::Value(dto(&report, "value", unsupported)),
                Err(crash) => Outcome::Exception(Box::new(record_of(&crash, owner, false, None))),
            })
        }
        "active_campaign_id" | "read_declaration" | "campaigns_dir" | "derive_path" => {
            let (root, module) = match module_site(args) {
                Ok(site) => site,
                Err(Ok(outcome)) => return Ok(outcome),
                Err(Err(bridge)) => return Err(bridge),
            };
            match identify(&module, REVIEWED_CAMPAIGN) {
                Identity::Reviewed => {}
                Identity::Missing => {
                    return Ok(unmet(
                        "Unmet",
                        "the campaign module file named by the request does not exist",
                    ))
                }
                Identity::Different => {
                    return Ok(legacy(
                        ctx,
                        operation,
                        args,
                        Some(&module),
                        "the campaign module is not the reviewed campaign_init.py",
                    ))
                }
                Identity::Unreadable(why) => {
                    return Ok(legacy(
                        ctx,
                        operation,
                        args,
                        Some(&module),
                        &format!("the campaign module cannot be read ({why})"),
                    ))
                }
            }
            if let Route::Unmet { reason } = owner_gate(None) {
                return Ok(unmet("Unmet", &reason));
            }
            if let Route::LegacySourceVariant { reason } = owner_gate(None) {
                return Ok(legacy(ctx, operation, args, compat_owner, &reason));
            }
            campaign_operation(operation, args, &root, &module, owner, unsupported)
        }
        other => Err(Bridge(format!("unknown operation '{other}'"))),
    }
}

fn campaign_operation(
    operation: &str,
    args: &PyDict,
    root: &[u8],
    module: &[u8],
    owner: &[u8],
    unsupported: &mut Vec<String>,
) -> Result<Outcome, Bridge> {
    let repo_root = optional_path(args, "repo_root")?;
    let crash = |crash: Crash, wrapper: Option<Frame>| {
        Ok(Outcome::Exception(Box::new(record_of(
            &crash, owner, false, wrapper,
        ))))
    };
    match operation {
        "active_campaign_id" => {
            match campaign::active_campaign_id(repo_root.as_deref(), root, module) {
                Ok(id) => Ok(Outcome::Value(dto(&PyValue::Str(id), "value", unsupported))),
                Err(failure) => crash(failure, None),
            }
        }
        "read_declaration" => {
            let base = path_arg(args, "base", None)?;
            match campaign::read_declaration(&base) {
                Ok(dict) => Ok(Outcome::Value(dto(
                    &PyValue::Dict(dict),
                    "value",
                    unsupported,
                ))),
                Err(failure) => crash(failure, None),
            }
        }
        "campaigns_dir" => Ok(Outcome::Value(path_dto(&campaign::campaigns_dir(
            repo_root.as_deref(),
            root,
        )))),
        _ => {
            let kind = optional_text(args, "kind")?
                .ok_or_else(|| Bridge("missing required argument: 'kind'".to_string()))?;
            let camp_id = match args.get("camp_id").map(PyValue::peel) {
                None | Some(PyValue::None) => None,
                Some(PyValue::Str(text)) => Some(text.clone()),
                Some(_) => {
                    return Err(Bridge(
                        "argument 'camp_id' must be a str or null".to_string(),
                    ))
                }
            };
            let derived = campaign::derive_path(
                &kind,
                optional_text(args, "cell")?.as_deref(),
                optional_text(args, "sweep")?.as_deref(),
                optional_text(args, "context")?.as_deref(),
                camp_id.as_ref(),
                repo_root.as_deref(),
                ModuleSite { root, file: module },
            );
            match derived {
                Ok(Derived::Path(path)) => Ok(Outcome::Value(path_dto(&path))),
                Ok(Derived::Refusal(refusal)) => {
                    // the raise statements of `derive_path` in the owner source (line numbers are source metadata)
                    let (line, code) = match kind.as_str() {
                        "config" | "lockset" | "cell-status" => (129, "raise PurgeGateRefusal(f\"--derive {kind} 는 --cell 이 필요하다(셀 없는 셀 입력은 없다)\")"),
                        "sweep" => (135, "raise PurgeGateRefusal(\"--derive sweep 은 --sweep 이 필요하다\")"),
                        _ => (147, "raise PurgeGateRefusal(f\"알 수 없는 파생 종류: {kind!r}\")"),
                    };
                    let frame = Frame {
                        file: module.to_vec(),
                        line,
                        function: "derive_path",
                        code,
                        expr: "",
                    };
                    Ok(Outcome::Exception(Box::new(ExceptionRecord {
                        class: "PurgeGateRefusal".to_string(),
                        defined_in_original_module: true,
                        message: refusal.0.clone(),
                        args: vec![tagged("str", PyValue::Str(refusal.0))],
                        attributes: Vec::new(),
                        frames: vec![frame],
                        attributes_incomplete: Vec::new(),
                        relation: None,
                    })))
                }
                Err(failure) => {
                    let wrapper = Frame {
                        file: module.to_vec(),
                        line: 123,
                        function: "derive_path",
                        code: "camp = camp_id or active_campaign_id(repo_root)",
                        expr: "active_campaign_id(repo_root)",
                    };
                    crash(failure, Some(wrapper))
                }
            }
        }
    }
}

/// The answer of one request row.
fn evaluate_row(row: &PyValue) -> PyValue {
    let mut out = PyDict::new();
    let request = match row.peel() {
        PyValue::Dict(d) => d,
        _ => {
            out.set("id", PyValue::None);
            out.set("kind", s("exception"));
            out.set(
                "exception",
                bridge_exception("a request row must be a JSON object"),
            );
            return PyValue::Dict(out);
        }
    };
    out.set("id", request.get("id").cloned().unwrap_or(PyValue::None));
    let operation = request
        .get("operation")
        .and_then(text_of)
        .and_then(|t| t.to_utf8())
        .unwrap_or_default();
    let empty = PyDict::new();
    let arguments = match request.get("arguments").map(PyValue::peel) {
        Some(PyValue::Dict(d)) => d,
        _ => &empty,
    };
    let bytes_of = |key: &str| {
        request
            .get(key)
            .and_then(text_of)
            .and_then(|t| role_contract::pyprofile::fs_encode(&t).ok())
    };
    let owner = bytes_of("source_owner").unwrap_or_else(|| DEFAULT_OWNER.as_bytes().to_vec());
    let compat_owner = bytes_of("compat_owner");
    let alias = request
        .get("module_alias")
        .and_then(text_of)
        .and_then(|t| t.to_utf8());
    let mut unsupported = Vec::new();
    let compat = request.get("compat").map(PyValue::peel);
    let compat_text = |key: &str| match compat {
        Some(PyValue::Dict(d)) => d
            .get(key)
            .and_then(text_of)
            .and_then(|t| role_contract::pyprofile::fs_encode(&t).ok()),
        _ => None,
    };
    let deadline: Result<u64, String> = match compat {
        Some(PyValue::Dict(d)) => match d.get("deadline_ms").map(PyValue::peel) {
            None => Ok(60_000),
            Some(PyValue::Int(n)) => {
                let text = n.to_string();
                match text.parse::<u64>() {
                    Ok(ms)
                        if std::time::Instant::now()
                            .checked_add(Duration::from_millis(ms))
                            .is_some() =>
                    {
                        Ok(ms)
                    }
                    Ok(_) | Err(_) if text.starts_with('-') => Err(format!(
                        "compat.deadline_ms must not be negative (got {text})"
                    )),
                    _ => Err(format!(
                        "compat.deadline_ms is beyond the representable clock range (got {text})"
                    )),
                }
            }
            Some(_) => {
                Err("compat.deadline_ms must be an integer number of milliseconds".to_string())
            }
        },
        _ => Ok(60_000),
    };
    let ctx = Context {
        owner,
        compat_owner,
        alias: alias.clone(),
        id: request.get("id").cloned().unwrap_or(PyValue::None),
        deadline_ms: deadline.as_ref().copied().unwrap_or(60_000),
        deadline_error: deadline.as_ref().err().cloned(),
        compat_python: compat_text("python").unwrap_or_else(|| b"python3".to_vec()),
        bridge_script: compat_text("bridge_script"),
    };
    // N11: a declared deadline that is not a valid one is rejected BEFORE anything runs (never defaulted, never executed)
    let outcome = if let Some(why) = ctx.deadline_error.as_deref() {
        Ok(unmet("InvalidCompatDeadline", why))
    } else {
        match run_operation(&operation, arguments, &ctx, &mut unsupported) {
            // a library-level "the campaign module is not the reviewed source" is never a business exception
            Ok(Outcome::Exception(record)) if record.class == "SourceUnmatched" => Ok(unmet(
                "LegacySourceVariant",
                &record.message.to_utf8().unwrap_or_default(),
            )),
            other => other,
        }
    };
    let mut bridge = PyDict::new();
    match outcome {
        Ok(Outcome::Value(value)) if unsupported.is_empty() => {
            out.set("kind", s("value"));
            out.set("route", s("NativeCurrentSource"));
            out.set("value", value);
        }
        Ok(Outcome::Value(value)) => {
            out.set("kind", s("incomplete"));
            out.set("route", s("NativeCurrentSource"));
            out.set("value", value);
            bridge.set(
                "unsupported_observations",
                PyValue::List(unsupported.iter().map(|u| s(u)).collect()),
            );
        }
        Ok(Outcome::Exception(mut record)) => {
            record.defined_in_original_module =
                record.defined_in_original_module || record.class == "PurgeGateRefusal";
            let (exception, meta) = exception_json(&record, alias.as_deref());
            out.set("kind", s("exception"));
            out.set("route", s("NativeCurrentSource"));
            out.set("exception", exception);
            bridge = match meta {
                PyValue::Dict(d) => d,
                _ => PyDict::new(),
            };
        }
        Ok(Outcome::LegacyControlIncomplete(observation)) => {
            out.set("kind", s("legacy_control_incomplete"));
            out.set("route", s("LegacySourceVariant"));
            out.set("observation", observation);
        }
        Ok(Outcome::Legacy(observation)) => {
            out.set("kind", s("legacy_observation"));
            out.set("route", s("LegacySourceVariant"));
            out.set("observation", observation);
        }
        Ok(Outcome::Unmet(condition)) => {
            out.set("kind", s("unmet_condition"));
            out.set(
                "route",
                s(if condition.starts_with("LegacySourceVariant") {
                    "LegacySourceVariant"
                } else {
                    "NotExecuted"
                }),
            );
            out.set("executed", PyValue::Bool(false));
            out.set("condition", s(&condition));
        }
        Err(Bridge(message)) => {
            out.set("kind", s("exception"));
            out.set("route", s("bridge"));
            out.set("exception", bridge_exception(&message));
            bridge.set("bridge_error", PyValue::Bool(true));
        }
    }
    if operation == "evaluate" {
        bridge.set("recursion_budget", s("CLI budget (the YAML loader's frame limit as `main` calls it): the original API caller's frame depth above evaluate is not in the saved records and the shared file loader takes no caller-frame budget; not modelled"));
    }
    bridge.set("chain_basis", s("no cause/context by source for the scoped operations except Path.resolve's Symlink loop (implicit context: the ELOOP OSError)"));
    out.set("bridge", PyValue::Dict(bridge));
    PyValue::Dict(out)
}

/// A request the bridge itself cannot serve (unknown operation, missing or ill-typed argument): a `TypeError`-shaped record
/// that the answer's `bridge.bridge_error` marks as the bridge's own (not the original's).
fn bridge_exception(message: &str) -> PyValue {
    let mut exception = PyDict::new();
    exception.set("type", s("TypeError"));
    exception.set("module", s("builtins"));
    exception.set("message", s(message));
    exception.set("args", PyValue::List(vec![tagged("str", s(message))]));
    exception.set("suppress_context", PyValue::Bool(false));
    exception.set("relation", PyValue::None);
    exception.set("attributes", PyValue::Dict(PyDict::new()));
    exception.set("frames", PyValue::List(Vec::new()));
    PyValue::Dict(exception)
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(
    args: &[ProcessStr],
    io: &mut Streams<'_>,
    env: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> i32 {
    let status = match args.first().and_then(ProcessStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => match TextProfile::from_environment(env) {
            Ok(profile) => with_profile(&profile, || run_jsonl(io)),
            Err(why) => {
                write_stderr(
                    io,
                    &ProcessStr::new(&format!(
                        "topology_parity_api: unsupported text profile: {why}\n"
                    )),
                );
                2
            }
        },
        Some("native-help") if args.len() == 1 => match io.stdout.write_all(NATIVE_HELP.as_bytes())
        {
            Ok(()) => 0,
            Err(_) => 1,
        },
        _ => {
            write_stderr(io, &ProcessStr::new(NATIVE_HELP));
            2
        }
    };
    io.finish(status)
}

fn run_jsonl(io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if io.stdin.read_to_end(&mut bytes).is_err() {
        return 1;
    }
    let text = decode_file_text(&bytes)
        .ok()
        .and_then(|t| t.to_utf8())
        .unwrap_or_default();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let answer = match loads_objects(line) {
            Ok(row) => evaluate_row(&row),
            Err(error) => {
                let mut out = PyDict::new();
                out.set("id", PyValue::None);
                out.set("kind", s("exception"));
                out.set("route", s("bridge"));
                out.set(
                    "exception",
                    bridge_exception(&format!("request is not JSON: {error}")),
                );
                PyValue::Dict(out)
            }
        };
        let rendered = answer.dumps_ascii().unwrap_or_default();
        if io.write_plain(&ProcessStr::new(&rendered)).is_err()
            || io.write_plain(&ProcessStr::new("\n")).is_err()
        {
            return 120;
        }
    }
    0
}
