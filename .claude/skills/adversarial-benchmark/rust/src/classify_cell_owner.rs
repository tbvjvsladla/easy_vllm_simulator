//! The owners `classify_cell` asks instead of deciding: where the installed `adversarial-benchmark/scripts` tree is
//! (the original found it from `__file__`), the node-identity resolver `node_identity.sh`, and - until its port - the
//! multi-node participation contract `node_role_contract.py`.
//!
//! **No rule of either owner is copied here.** [`Owners::resolve_self_node`] runs the real `node_identity.sh
//! --resolve --repo REPO` with the original's timeout and reports its own failure text. [`RoleSession`] starts a
//! *query adapter* (a small Python program named by `CLASSIFY_CELL_ROLE_ADAPTER`) that loads the actual
//! `node_role_contract.py` the way the original did and calls that module's own `load_yaml` and `evaluate_manifest`.
//! The documents stay **in the owner's process**; the classifier reads them lazily through [`Descriptor`]s - a scalar
//! comes across as a typed value, a list/tuple/dict as a handle with its type and length - and asks only for what the
//! original's code touches (`doc.get("nodes")`, each node's `role`, `result["violations"]`, each violation's `code`).
//! So metadata the classifier never reads (a huge integer key, a self-referencing anchor) cannot change a plan, exactly
//! as in the original. Nothing falls back to a Python classifier: when the owner exists but cannot be queried the
//! failure says so by name.
//!
//! `OPEN_S3`: the adapter and the per-call process start are provisional and go away when `node_role_contract` is ported;
//! the latency they add is not a property of this port.

use crate::cli::os_error_text;
use crate::guidellm::Failure;
use crate::ospath;
use crate::pyjson::{decode_utf8_strict, loads};
use crate::pystr::PyStr;
use crate::pyvalue::{DictKey, NonStringKey, Opaque, PyDict, PyValue};
use num_bigint::BigInt;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// Environment variable naming the installed `adversarial-benchmark/scripts` directory (the original's
/// `os.path.dirname(os.path.abspath(__file__))`). A launcher sets it to its own directory.
pub const SCRIPTS_DIR_ENV: &str = "CLASSIFY_CELL_SCRIPTS_DIR";
/// Environment variable naming the repository root; the scripts directory is derived from it when the explicit one is
/// not given (`<root>/.claude/skills/adversarial-benchmark/scripts`).
pub const SOURCE_ROOT_ENV: &str = "EASY_VLLM_SOURCE_ROOT";
/// Environment variable naming the role-contract query adapter program.
pub const ROLE_ADAPTER_ENV: &str = "CLASSIFY_CELL_ROLE_ADAPTER";
/// File name of the role-contract query adapter looked up in the scripts directory when [`ROLE_ADAPTER_ENV`] is not set.
pub const DEFAULT_ROLE_ADAPTER: &str = "classify_cell_role_query.py";
/// Environment variable naming the Python interpreter that runs the adapter (default `python3`).
pub const PYTHON_ENV: &str = "CLASSIFY_CELL_PYTHON";
/// Exception class reported when an operation needs the scripts directory and none is configured. The original
/// always knew its own location, so this is not one of its exceptions.
pub const CONFIGURATION_ERROR: &str = "ClassifyCellConfigurationError";
/// Exception class reported when the owner exists but the query path to it is not usable.
pub const ADAPTER_ERROR: &str = "RoleContractAdapterError";

/// The `subprocess.run(..., timeout=60)` of the original's resolver call.
pub const RESOLVER_TIMEOUT_SECONDS: u64 = 60;

const SCRIPTS_REL: [&str; 3] = [".claude/skills/adversarial-benchmark", "scripts", ""];

/// Where the owners are looked up and how the role-contract owner is queried. Cheap to build; nothing is read until
/// an operation needs an owner.
#[derive(Clone, Debug)]
pub struct Owners {
    scripts_dir: Option<PyStr>,
    python: OsString,
    adapter: Option<OsString>,
    timeout_seconds: u64,
}

impl Owners {
    /// An explicit configuration (checks build several).
    pub fn new(
        scripts_dir: Option<PyStr>,
        python: impl Into<OsString>,
        adapter: Option<OsString>,
    ) -> Self {
        Owners {
            scripts_dir,
            python: python.into(),
            adapter,
            timeout_seconds: RESOLVER_TIMEOUT_SECONDS,
        }
    }

    /// The configuration of this process: [`SCRIPTS_DIR_ENV`] (else the installed layout around the executable, else
    /// [`SOURCE_ROOT_ENV`]), [`ROLE_ADAPTER_ENV`] and [`PYTHON_ENV`].
    pub fn from_environment() -> Self {
        let text = |name: &str| -> Option<PyStr> {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(|value| PyStr::from_surrogateescape(value.as_bytes()))
        };
        let scripts_dir = text(SCRIPTS_DIR_ENV)
            .or_else(installed_scripts_dir)
            .or_else(|| {
                text(SOURCE_ROOT_ENV)
                    .map(|root| ospath::join_all(&root, &[SCRIPTS_REL[0], SCRIPTS_REL[1]]))
            });
        // The adapter is a program of the same scripts directory unless one is named (a launcher names its own).
        let adapter = std::env::var_os(ROLE_ADAPTER_ENV)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                let beside = ospath::join_all(&scripts_dir.clone()?, &[DEFAULT_ROLE_ADAPTER]);
                if !ospath::isfile(&beside) {
                    return None;
                }
                beside
                    .to_surrogateescape_bytes()
                    .ok()
                    .map(OsString::from_vec)
            });
        Owners::new(
            scripts_dir,
            std::env::var_os(PYTHON_ENV).unwrap_or_else(|| OsString::from("python3")),
            adapter,
        )
    }

    /// The same configuration with a different resolver timeout (checks only; the message still names the value).
    pub fn with_timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// `os.path.dirname(os.path.abspath(__file__))` of the original.
    pub fn scripts_dir(&self) -> Result<PyStr, Failure> {
        let Some(directory) = &self.scripts_dir else {
            return Err(Failure::new(
                CONFIGURATION_ERROR,
                &format!(
                    "the scripts directory the owners are looked up from is not configured; set {SCRIPTS_DIR_ENV} (the original derived it from its own file location)"
                ),
            ));
        };
        ospath::abspath(directory)
    }

    /// `_node_identity_resolver()`: the first existing of the owner copy and the delivered runtime copy.
    pub fn node_identity_resolver(&self) -> Result<Option<PyStr>, Failure> {
        let here = self.scripts_dir()?;
        let candidates = [
            ospath::join_all(
                &here,
                &[
                    "..",
                    "..",
                    "terraforming_node",
                    "scripts",
                    "node_blackbox",
                    "node_identity.sh",
                ],
            ),
            ospath::join_all(
                &here,
                &[
                    "..",
                    "..",
                    "..",
                    "runtime",
                    "node_blackbox",
                    "node_identity.sh",
                ],
            ),
        ];
        Ok(candidates
            .into_iter()
            .find(ospath::isfile)
            .map(|found| ospath::normpath(&found)))
    }

    /// The `node_role_contract.py` path of the original's `_node_role_contract()` (not yet checked for existence).
    pub fn role_contract_path(&self) -> Result<PyStr, Failure> {
        let here = self.scripts_dir()?;
        Ok(ospath::join_all(
            &here,
            &[
                "..",
                "..",
                "terraforming_node",
                "scripts",
                "node_role_contract.py",
            ],
        ))
    }

    /// `resolve_self_node(repo)`: the node id of the node this repository sits on and where that answer came from, or
    /// `None` and the reason. The resolver is run for real; its failure text is carried, not swallowed.
    pub fn resolve_self_node(&self, repo: &PyValue) -> Result<(Option<PyStr>, PyStr), Failure> {
        if !repo.is_truthy() {
            return Ok((None, PyStr::new("repo 미지정")));
        }
        let Some(resolver) = self.node_identity_resolver()? else {
            return Ok((
                None,
                PyStr::new(
                    "node_identity.sh 부재(terraforming_node/scripts/node_blackbox · .claude/runtime/node_blackbox)",
                ),
            ));
        };
        let repo = ospath::fspath(repo).map_err(|_| {
            Failure::new(
                "TypeError",
                &format!(
                    "expected str, bytes or os.PathLike object, not {}",
                    repo.type_name()
                ),
            )
        })?;
        let command = [
            PyStr::new("bash"),
            resolver.clone(),
            PyStr::new("--resolve"),
            PyStr::new("--repo"),
            repo.clone(),
        ];
        let ran = match run_captured(&command, Duration::from_secs(self.timeout_seconds))? {
            Ran::Finished(ran) => ran,
            Ran::SpawnFailed(error) => {
                let mut why = PyStr::new("node_identity.sh 실행 실패: ");
                why.push_py(&error);
                return Ok((None, why));
            }
            Ran::TimedOut => {
                let mut why = PyStr::new("node_identity.sh 실행 실패: Command '");
                why.push_str(&list_repr(&command));
                why.push_str(&format!(
                    "' timed out after {} seconds",
                    self.timeout_seconds
                ));
                return Ok((None, why));
            }
        };
        let node = ran.stdout.strip();
        let acceptable = ran.code == 0 && {
            let value = PyValue::Str(node.clone());
            crate::classify_cell::path_segment(&value)
        };
        if !acceptable {
            // `" / ".join(line.strip() for line in stderr.splitlines()[:2] if line.strip())`
            let why: Vec<PyStr> = splitlines(&ran.stderr)
                .into_iter()
                .take(2)
                .map(|line| line.strip())
                .filter(|line| !line.is_empty())
                .collect();
            let mut text = PyStr::new(&format!("node_identity.sh --resolve rc={}", ran.code));
            if !why.is_empty() {
                text.push_str(" — ");
                for (index, line) in why.iter().enumerate() {
                    if index > 0 {
                        text.push_str(" / ");
                    }
                    text.push_py(line);
                }
            }
            return Ok((None, text));
        }
        Ok((
            Some(node),
            PyStr::new("node_identity.sh --resolve(self_role → 유일한 role: main)"),
        ))
    }

    /// `_node_role_contract()`: `None` when the owner file is absent or its top level raised (the original's `None`),
    /// else a session that queries it. A configuration or adapter failure is an error, never `None`.
    pub fn open_role_contract(&self) -> Result<Option<RoleSession>, Failure> {
        let path = self.role_contract_path()?;
        if !ospath::isfile(&path) {
            return Ok(None);
        }
        let Some(adapter) = &self.adapter else {
            return Err(adapter_error(format!(
                "the node_role_contract owner exists at {} but no query adapter is configured; set {ROLE_ADAPTER_ENV} (this build does not fall back to a Python classifier)",
                path.repr()
            )));
        };
        let mut child = Command::new(&self.python)
            .arg("-B")
            .arg(adapter)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| {
                adapter_error(format!(
                    "cannot start the node_role_contract adapter {adapter:?} with {:?}: {error}",
                    self.python
                ))
            })?;
        let writer = child.stdin.take();
        let reader = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut session = RoleSession {
            child,
            writer,
            reader,
        };
        let reply = session.ask("load", vec![("path", PyValue::Str(path))])?;
        match reply.get("loaded") {
            Some(PyValue::Bool(true)) => Ok(Some(session)),
            _ => Ok(None),
        }
    }
}

fn adapter_error(message: String) -> Failure {
    Failure::new(ADAPTER_ERROR, &message)
}

/// The installed layout of a native build: `.../adversarial-benchmark/rust/target/<profile>/<program>` finds
/// `.../adversarial-benchmark/scripts`.
fn installed_scripts_dir() -> Option<PyStr> {
    let exe = std::env::current_exe().ok()?;
    for ancestor in exe.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "rust") {
            let scripts = ancestor.parent()?.join("scripts");
            if scripts.is_dir() {
                return Some(PyStr::from_surrogateescape(scripts.as_os_str().as_bytes()));
            }
        }
    }
    None
}

/// `repr(list_of_str)`.
fn list_repr(items: &[PyStr]) -> String {
    let inner: Vec<String> = items.iter().map(PyStr::repr).collect();
    format!("[{}]", inner.join(", "))
}

// ── running the resolver ────────────────────────────────────────────────────────

struct Captured {
    code: i64,
    stdout: PyStr,
    stderr: PyStr,
}

enum Ran {
    Finished(Captured),
    /// `OSError` from starting the program; carries `str(exception)`.
    SpawnFailed(PyStr),
    TimedOut,
}

/// `str.splitlines()`: lines end at `\n`, `\r`, `\r\n`, `\v`, `\f`, `\x1c`-`\x1e`, `\x85`, `\u2028` and `\u2029`.
fn splitlines(text: &PyStr) -> Vec<PyStr> {
    let points = text.code_points();
    let is_break = |point: u32| {
        matches!(
            point,
            0x0A | 0x0B | 0x0C | 0x0D | 0x1C | 0x1D | 0x1E | 0x85 | 0x2028 | 0x2029
        )
    };
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < points.len() {
        if is_break(points[index]) {
            lines.push(PyStr::from_code_points(points[start..index].to_vec()));
            if points[index] == 0x0D && points.get(index + 1) == Some(&0x0A) {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    if start < points.len() {
        lines.push(PyStr::from_code_points(points[start..].to_vec()));
    }
    lines
}

/// `subprocess.run(command, capture_output=True, text=True, timeout=..., check=False)`.
fn run_captured(command: &[PyStr], timeout: Duration) -> Result<Ran, Failure> {
    let mut arguments: Vec<OsString> = Vec::new();
    for argument in command {
        let bytes = argument
            .to_surrogateescape_bytes()
            .map_err(|failure| Failure::new("UnicodeEncodeError", &failure.message()))?;
        if bytes.contains(&0) {
            return Err(Failure::new("ValueError", "embedded null byte"));
        }
        arguments.push(OsString::from_vec(bytes));
    }
    let (program, rest) = arguments.split_first().expect("a command has a program");
    let mut child = match Command::new(program)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return Ok(Ran::SpawnFailed(PyStr::new(&os_error_text(
                &error,
                &command[0],
            ))))
        }
    };
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    };
    let out_reader = drain(Box::new(child.stdout.take().expect("piped")));
    let err_reader = drain(Box::new(child.stderr.take().expect("piped")));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => break None,
        }
    };
    let Some(status) = status else {
        // As `subprocess.run`: kill, reap, raise. The pipes are not drained again (a grandchild that still holds them
        // must not hold this program up); the reader threads end on their own when the pipes close.
        let _ = child.kill();
        let _ = child.wait();
        drop((out_reader, err_reader));
        return Ok(Ran::TimedOut);
    };
    let out_bytes = out_reader.join().unwrap_or_default();
    let err_bytes = err_reader.join().unwrap_or_default();
    let decode = |bytes: &[u8]| -> Result<PyStr, Failure> {
        let text = decode_utf8_strict(bytes)
            .map_err(|error| Failure::new("UnicodeDecodeError", &error.to_string()))?;
        Ok(universal_newlines(&text))
    };
    let code = status
        .code()
        .map(i64::from)
        .or_else(|| status.signal().map(|signal| -i64::from(signal)))
        .unwrap_or(-1);
    Ok(Ran::Finished(Captured {
        code,
        stdout: decode(&out_bytes)?,
        stderr: decode(&err_bytes)?,
    }))
}

/// `\r\n` and `\r` become `\n` (a text-mode pipe).
fn universal_newlines(text: &PyStr) -> PyStr {
    let points = text.code_points();
    let (cr, lf) = (0x0D_u32, 0x0A_u32);
    let mut out = Vec::with_capacity(points.len());
    let mut index = 0;
    while index < points.len() {
        if points[index] == cr {
            out.push(lf);
            if points.get(index + 1) == Some(&lf) {
                index += 1;
            }
        } else {
            out.push(points[index]);
        }
        index += 1;
    }
    PyStr::from_code_points(out)
}

// ── the role-contract owner ─────────────────────────────────────────────────────

/// A loaded `node_role_contract` owner. Each method is one query to the owner. The outer `Err` is a failure of the
/// query path; the inner `Err` is an exception the owner itself raised (class and `str`).
pub struct RoleSession {
    child: Child,
    writer: Option<ChildStdin>,
    reader: BufReader<ChildStdout>,
}

/// What an owner call returned, or the exception it raised.
pub type Raised<T> = Result<T, Failure>;

impl RoleSession {
    fn request(&mut self, request: PyDict) -> Result<PyDict, Failure> {
        let line = PyValue::Dict(request)
            .dumps_ascii()
            .map_err(|error| adapter_error(error.to_string()))?;
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| adapter_error("adapter input is closed".to_string()))?;
        writeln!(writer, "{line}")
            .and_then(|()| writer.flush())
            .map_err(|error| adapter_error(format!("cannot write to the adapter: {error}")))?;
        let mut answer = String::new();
        let read = self
            .reader
            .read_line(&mut answer)
            .map_err(|error| adapter_error(format!("cannot read from the adapter: {error}")))?;
        if read == 0 {
            return Err(adapter_error(
                "the adapter ended without answering (see its stderr)".to_string(),
            ));
        }
        match loads(&answer) {
            Ok(PyValue::Dict(reply)) => Ok(reply),
            Ok(other) => Err(adapter_error(format!(
                "the adapter answered a {}, not an object",
                other.type_name()
            ))),
            Err(error) => Err(adapter_error(format!("unreadable adapter answer: {error}"))),
        }
    }

    fn ask(&mut self, operation: &str, extra: Vec<(&str, PyValue)>) -> Result<PyDict, Failure> {
        let mut request = PyDict::new();
        request.set("op", PyValue::str(operation));
        for (name, value) in extra {
            request.set(name, value);
        }
        self.request(request)
    }

    /// Splits an answer into the owner's value (a descriptor) or the owner's exception.
    fn outcome(reply: &PyDict) -> Result<Raised<Descriptor>, Failure> {
        match reply.get("ok") {
            Some(PyValue::Bool(true)) => {
                let node = reply
                    .get("value")
                    .ok_or_else(|| adapter_error("the adapter answer has no value".to_string()))?;
                Ok(Ok(Descriptor::decode(node).map_err(adapter_error)?))
            }
            Some(PyValue::Bool(false)) => {
                let class = match reply.get("type") {
                    Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
                    _ => ADAPTER_ERROR.to_string(),
                };
                let message = match reply.get("message") {
                    Some(PyValue::Str(text)) => text.clone(),
                    _ => PyStr::default(),
                };
                Ok(Err(Failure::with_message(exception_class(&class), message)))
            }
            _ => Err(adapter_error("the adapter answer has no ok".to_string())),
        }
    }

    /// A mechanical read of a value the owner holds: any failure is a failure of the query path, never an owner
    /// exception (the owner's own functions are `load_yaml` and `evaluate_manifest` only).
    fn read(
        &mut self,
        operation: &str,
        handle: u64,
        extra: Vec<(&str, PyValue)>,
    ) -> Result<PyDict, Failure> {
        let mut arguments = vec![("handle", PyValue::int(handle))];
        arguments.extend(extra);
        let reply = self.ask(operation, arguments)?;
        match reply.get("ok") {
            Some(PyValue::Bool(true)) => Ok(reply),
            _ => Err(adapter_error(format!(
                "the adapter could not {operation} a value of the owner: {}",
                reply
                    .get("message")
                    .map(|message| message.display_text().to_utf8().unwrap_or_default())
                    .unwrap_or_default()
            ))),
        }
    }

    /// `contract.load_yaml(manifest_path)`: the document, held by the owner.
    pub fn load_yaml(&mut self, manifest_path: &PyStr) -> Result<Raised<Descriptor>, Failure> {
        let reply = self.ask(
            "load_yaml",
            vec![("path", PyValue::Str(manifest_path.clone()))],
        )?;
        Self::outcome(&reply)
    }

    /// `contract.evaluate_manifest(doc, topology=topology)` on the document the last `load_yaml` returned.
    pub fn evaluate_manifest(&mut self, topology: &str) -> Result<Raised<Descriptor>, Failure> {
        let reply = self.ask("evaluate", vec![("topology", PyValue::str(topology))])?;
        Self::outcome(&reply)
    }

    /// `value.get(key)` of a dict the owner holds (`None` when the key is absent).
    pub fn get(&mut self, value: &Descriptor, key: &str) -> Result<Option<Descriptor>, Failure> {
        let Descriptor::Container { handle, kind, .. } = value else {
            return Err(adapter_error(
                "get asked of a value that is not a container".to_string(),
            ));
        };
        if *kind != "dict" {
            return Err(adapter_error(format!("get asked of a {kind}, not a dict")));
        }
        let reply = self.read("get", *handle, vec![("key", PyValue::str(key))])?;
        match reply.get("found") {
            Some(PyValue::Bool(true)) => {
                let node = reply
                    .get("value")
                    .ok_or_else(|| adapter_error("the adapter answer has no value".to_string()))?;
                Ok(Some(Descriptor::decode(node).map_err(adapter_error)?))
            }
            _ => Ok(None),
        }
    }

    /// `for item in value`: a list's or tuple's elements, a dict's keys, as descriptors.
    pub fn items(&mut self, value: &Descriptor) -> Result<Vec<Descriptor>, Failure> {
        match value {
            Descriptor::Value(scalar) => crate::pydyn::iterate(scalar)
                .map(|items| items.into_iter().map(Descriptor::Value).collect()),
            Descriptor::Container { handle, .. } => {
                let reply = self.read("items", *handle, Vec::new())?;
                let Some(PyValue::List(nodes)) = reply.get("value") else {
                    return Err(adapter_error("the adapter answer has no items".to_string()));
                };
                nodes
                    .iter()
                    .map(|node| Descriptor::decode(node).map_err(adapter_error))
                    .collect()
            }
        }
    }

    /// The whole value as a [`PyValue`], for the few places that need its content (a tuple used as a node role). A
    /// self-referencing value cannot be materialised and is reported as a failure of the query path.
    pub fn materialize(&mut self, value: &Descriptor) -> Result<PyValue, Failure> {
        match value {
            Descriptor::Value(scalar) => Ok(scalar.clone()),
            Descriptor::Container { handle, .. } => {
                let reply = self.read("materialize", *handle, Vec::new())?;
                let typed = reply
                    .get("value")
                    .ok_or_else(|| adapter_error("the adapter answer has no value".to_string()))?;
                decode_typed(typed).map_err(adapter_error)
            }
        }
    }
}

/// A value the owner holds, as far as the classifier has looked at it: a scalar is its value; a container is a handle
/// to the owner's object with its type and length (so `isinstance`, truthiness and `len` need no content).
#[derive(Clone, Debug)]
pub enum Descriptor {
    Value(PyValue),
    Container {
        handle: u64,
        kind: &'static str,
        len: usize,
    },
}

impl Descriptor {
    /// `["ref", handle, "list"|"tuple"|"dict", len]`, or a typed scalar (see [`decode_typed`]).
    fn decode(node: &PyValue) -> Result<Descriptor, String> {
        if let PyValue::List(parts) = node.peel() {
            if matches!(parts.first(), Some(PyValue::Str(tag)) if tag.equals("ref")) {
                let handle = parts
                    .get(1)
                    .and_then(|value| value.as_plain_int())
                    .and_then(num_traits::ToPrimitive::to_u64)
                    .ok_or("a reference without a handle")?;
                let kind = match parts.get(2).map(PyValue::peel) {
                    Some(PyValue::Str(kind)) if kind.equals("list") => "list",
                    Some(PyValue::Str(kind)) if kind.equals("tuple") => "tuple",
                    Some(PyValue::Str(kind)) if kind.equals("dict") => "dict",
                    _ => return Err("a reference of an unknown container type".to_string()),
                };
                let len = parts
                    .get(3)
                    .and_then(|value| value.as_plain_int())
                    .and_then(num_traits::ToPrimitive::to_usize)
                    .ok_or("a reference without a length")?;
                return Ok(Descriptor::Container { handle, kind, len });
            }
        }
        decode_typed(node).map(Descriptor::Value)
    }

    /// `type(value).__name__`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Descriptor::Value(value) => value.type_name(),
            Descriptor::Container { kind, .. } => kind,
        }
    }

    /// `isinstance(value, dict)`.
    pub fn is_dict(&self) -> bool {
        matches!(self, Descriptor::Container { kind: "dict", .. })
    }

    /// `bool(value)`.
    pub fn is_truthy(&self) -> bool {
        match self {
            Descriptor::Value(value) => value.is_truthy(),
            Descriptor::Container { len, .. } => *len > 0,
        }
    }

    /// A value of the same type with no content: for the places where Python only looks at the type (an error message,
    /// an unhashability check) and the content is never read.
    pub fn type_only(&self) -> PyValue {
        match self {
            Descriptor::Value(value) => value.clone(),
            Descriptor::Container { kind: "list", .. } => PyValue::List(Vec::new()),
            Descriptor::Container { kind: "tuple", .. } => PyValue::Tuple(Vec::new()),
            Descriptor::Container { .. } => PyValue::Dict(PyDict::new()),
        }
    }
}

impl Drop for RoleSession {
    fn drop(&mut self) {
        self.writer.take();
        let _ = self.child.wait();
    }
}

/// The exception classes the adapter relays are names; a `Failure` wants a static one.
fn exception_class(name: &str) -> &'static str {
    const KNOWN: [&str; 18] = [
        "AttributeError",
        "TypeError",
        "ValueError",
        "KeyError",
        "OSError",
        "FileNotFoundError",
        "PermissionError",
        "NotADirectoryError",
        "IsADirectoryError",
        "ImportError",
        "ModuleNotFoundError",
        "SyntaxError",
        "NameError",
        "RuntimeError",
        "RecursionError",
        "UnicodeDecodeError",
        "YAMLError",
        "ScannerError",
    ];
    KNOWN
        .iter()
        .find(|known| **known == name)
        .copied()
        // An owner-defined class: the name is kept for the report; the handful of names leaked live for the process.
        .unwrap_or_else(|| Box::leak(name.to_string().into_boxed_str()))
}

/// Rebuilds a Python value from the adapter's typed encoding (every node is `[tag, ...]`, so no value can be mistaken
/// for a tag):
/// `["none"]`, `["bool", b]`, `["int", "hex"]`, `["float", "repr"]`, `["str", s]`, `["list", [..]]`,
/// `["tuple", [..]]`, `["dict", [[key, value], ..]]` (a key is a typed value, or `["other", identity, repr]` for a
/// non-string key), `["bytes", "hex"]`, `["opaque", type_name, repr, truthy]`.
pub fn decode_typed(node: &PyValue) -> Result<PyValue, String> {
    let PyValue::List(parts) = node.peel() else {
        return Err("a typed node is not a list".to_string());
    };
    let Some(PyValue::Str(tag)) = parts.first() else {
        return Err("a typed node has no tag".to_string());
    };
    let tag = tag.to_utf8().unwrap_or_default();
    let arg = |index: usize| {
        parts
            .get(index)
            .ok_or_else(|| format!("typed node {tag} lacks argument {index}"))
    };
    let text = |index: usize| -> Result<PyStr, String> {
        match arg(index)?.peel() {
            PyValue::Str(text) => Ok(text.clone()),
            _ => Err(format!("typed node {tag} argument {index} is not text")),
        }
    };
    let items = |index: usize| -> Result<&Vec<PyValue>, String> {
        match arg(index)?.peel() {
            PyValue::List(items) => Ok(items),
            _ => Err(format!("typed node {tag} argument {index} is not a list")),
        }
    };
    Ok(match tag.as_str() {
        "none" => PyValue::None,
        "bool" => match arg(1)?.peel() {
            PyValue::Bool(flag) => PyValue::Bool(*flag),
            _ => return Err("typed bool without a bool".to_string()),
        },
        // Hexadecimal on the wire (no decimal conversion, so no digit limit is involved in carrying a value).
        "int" => PyValue::Int(
            BigInt::parse_bytes(text(1)?.to_utf8().unwrap_or_default().as_bytes(), 16)
                .ok_or("typed int is not hexadecimal")?,
        ),
        "float" => PyValue::Float(match text(1)?.to_utf8().unwrap_or_default().as_str() {
            "nan" => f64::NAN,
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            other => other
                .parse::<f64>()
                .map_err(|error| format!("typed float: {error}"))?,
        }),
        "str" => PyValue::Str(text(1)?),
        "list" => PyValue::List(
            items(1)?
                .iter()
                .map(decode_typed)
                .collect::<Result<_, _>>()?,
        ),
        "tuple" => PyValue::Tuple(
            items(1)?
                .iter()
                .map(decode_typed)
                .collect::<Result<_, _>>()?,
        ),
        "dict" => {
            let mut map = PyDict::new();
            for pair in items(1)? {
                let PyValue::List(pair) = pair.peel() else {
                    return Err("typed dict entry is not a pair".to_string());
                };
                let [key, value] = pair.as_slice() else {
                    return Err("typed dict entry is not a pair".to_string());
                };
                let value = decode_typed(value)?;
                match key.peel() {
                    PyValue::List(parts) if matches!(parts.first(), Some(PyValue::Str(tag)) if tag.equals("other")) =>
                    {
                        let identity = match parts.get(1).map(PyValue::peel) {
                            Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
                            _ => return Err("typed key lacks an identity".to_string()),
                        };
                        let repr = match parts.get(2).map(PyValue::peel) {
                            Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
                            _ => return Err("typed key lacks a repr".to_string()),
                        };
                        map.insert_key(DictKey::Other(NonStringKey { identity, repr }), value);
                    }
                    other => match decode_typed(other)? {
                        PyValue::Str(text) => map.insert(text, value),
                        _ => {
                            return Err("typed dict key is neither a string nor `other`".to_string())
                        }
                    },
                }
            }
            PyValue::Dict(map)
        }
        "bytes" => {
            let hex = text(1)?.to_utf8().unwrap_or_default();
            let bytes = (0..hex.len() / 2)
                .map(|at| u8::from_str_radix(&hex[2 * at..2 * at + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
                .map_err(|error| format!("typed bytes: {error}"))?;
            PyValue::Bytes(bytes)
        }
        "opaque" => PyValue::Opaque(Opaque {
            type_name: opaque_type_name(&text(1)?.to_utf8().unwrap_or_default()),
            repr: text(2)?.to_utf8().unwrap_or_default(),
            truthy: match arg(3)?.peel() {
                PyValue::Bool(flag) => *flag,
                _ => return Err("typed opaque without a truth value".to_string()),
            },
        }),
        other => return Err(format!("unknown typed tag {other:?}")),
    })
}

fn opaque_type_name(name: &str) -> &'static str {
    const KNOWN: [&str; 4] = ["date", "datetime", "set", "frozenset"];
    KNOWN
        .iter()
        .find(|known| **known == name)
        .copied()
        .unwrap_or_else(|| Box::leak(name.to_string().into_boxed_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> PyValue {
        decode_typed(&loads(text).unwrap()).unwrap()
    }

    #[test]
    fn typed_values_round_trip() {
        let value = typed(
            r#"["dict", [[["str","a"],["list",[["int","1"],["float","nan"],["none"]]]], [["other","num:1","1"],["bool",true]]]]"#,
        );
        assert_eq!(value.repr(), "{'a': [1, nan, None], 1: True}");
        assert_eq!(
            typed(r#"["opaque","date","datetime.date(2026, 1, 1)",true]"#).repr(),
            "datetime.date(2026, 1, 1)"
        );
    }

    #[test]
    fn unconfigured_scripts_directory_is_named() {
        let owners = Owners::new(None, "python3", None);
        let failure = owners.scripts_dir().unwrap_err();
        assert_eq!(failure.class, CONFIGURATION_ERROR);
    }

    #[test]
    fn captured_text_has_universal_newlines() {
        let text = universal_newlines(&PyStr::new("a\r\nb\rc\n"));
        assert_eq!(text.to_utf8().unwrap(), "a\nb\nc\n");
    }
}
