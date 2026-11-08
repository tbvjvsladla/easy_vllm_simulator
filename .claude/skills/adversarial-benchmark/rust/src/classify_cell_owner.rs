//! The owners `classify_cell` asks instead of deciding: where the installed `adversarial-benchmark/scripts` tree is
//! (the original found it from `__file__`), the node-identity resolver `node_identity.sh`, and the multi-node
//! participation contract `node_role_contract.py`.
//!
//! **No rule of either owner is copied here.** [`Owners::resolve_self_node`] runs the real `node_identity.sh
//! --resolve --repo REPO` with the original's timeout and reports its own failure text. The participation contract is
//! the single native definition of the policy, the `role-contract` library (see [`crate::role_bridge`]): the owner
//! FILE is only looked up (`isfile`), never read or executed, and [`Owners::open_role_contract`] returns a
//! [`NativeRole`] whose `load_yaml` and `evaluate_manifest` are that library's. The documents stay in the library's own
//! (Q) values; the classifier reads them lazily through [`Descriptor`]s - a scalar comes across as a typed value, a
//! list/tuple/dict as a handle into the shared document with its type and length - and asks only for what the original's
//! code touches (`doc.get("nodes")`, each node's `role`, `result["violations"]`, each violation's `code`). So
//! metadata the classifier never reads (a huge integer key, a self-referencing anchor) cannot change a plan, exactly as
//! in the original. No Python runs and nothing falls back to a Python classifier.
//!
//! A check can install a stand-in role source ([`Owners::with_role_source`]); production code never does.

use crate::cli::os_error_text;
use crate::guidellm::Failure;
use crate::ospath;
use crate::pyjson::decode_utf8_strict;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::ffi::OsString;
use std::io::Read;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::role_bridge::FixedRoleContract;
pub use crate::role_bridge::{NativeRole, Node, RoleFailure, RoleSource, BRIDGE_ERROR};

/// Environment variable naming the installed `adversarial-benchmark/scripts` directory (the original's
/// `os.path.dirname(os.path.abspath(__file__))`). A launcher sets it to its own directory.
pub const SCRIPTS_DIR_ENV: &str = "CLASSIFY_CELL_SCRIPTS_DIR";
/// Environment variable naming the repository root; the scripts directory is derived from it when the explicit one is
/// not given (`<root>/.claude/skills/adversarial-benchmark/scripts`).
pub const SOURCE_ROOT_ENV: &str = "EASY_VLLM_SOURCE_ROOT";
/// Exception class reported when an operation needs the scripts directory and none is configured. The original
/// always knew its own location, so this is not one of its exceptions.
pub const CONFIGURATION_ERROR: &str = "ClassifyCellConfigurationError";
/// The `subprocess.run(..., timeout=60)` of the original's resolver call.
pub const RESOLVER_TIMEOUT_SECONDS: u64 = 60;

const SCRIPTS_REL: [&str; 3] = [".claude/skills/adversarial-benchmark", "scripts", ""];

/// Where the owners are looked up. Cheap to build; nothing is read until an operation needs an owner.
#[derive(Clone, Debug)]
pub struct Owners {
    scripts_dir: Option<PyStr>,
    timeout_seconds: u64,
    role_source: Option<Arc<dyn RoleSource>>,
}

impl Owners {
    /// An explicit configuration (checks build several).
    pub fn new(scripts_dir: Option<PyStr>) -> Self {
        Owners {
            scripts_dir,
            timeout_seconds: RESOLVER_TIMEOUT_SECONDS,
            role_source: None,
        }
    }

    /// The configuration of this process: [`SCRIPTS_DIR_ENV`] (else the installed layout around the executable, else
    /// [`SOURCE_ROOT_ENV`]).
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
        Owners::new(scripts_dir)
    }

    /// CHECK-ONLY seam: answers `load_yaml` / `evaluate_manifest` from `source` instead of the `role-contract`
    /// library, so a check can hand the classifier the crafted typed values the original's stand-in owners returned
    /// or the exceptions they raised. Production code (`from_environment`) never calls it.
    pub fn with_role_source(mut self, source: Arc<dyn RoleSource>) -> Self {
        self.role_source = Some(source);
        self
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

    /// `_node_role_contract()`: `None` when the owner file is absent (the original also read an owner whose top level
    /// raised as `None`; the file is not executed here, so that one case is a named limit, see `PORTING.md` D1), else the
    /// native role. A configuration failure is an error, never `None`.
    pub fn open_role_contract(&self) -> Result<Option<NativeRole>, Failure> {
        let path = self.role_contract_path()?;
        if !ospath::isfile(&path) {
            return Ok(None);
        }
        let source: Arc<dyn RoleSource> = match &self.role_source {
            Some(source) => source.clone(),
            None => Arc::new(FixedRoleContract),
        };
        Ok(Some(NativeRole::new(source)))
    }
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

/// What an owner call returned, or the exception it raised.
pub type Raised<T> = Result<T, Failure>;

/// A value the role library holds, as far as the classifier has looked at it: a scalar is its value; a container is a
/// handle into the shared document with its type and length (so `isinstance`, truthiness and `len` need no content).
#[derive(Clone, Debug)]
pub enum Descriptor {
    Value(PyValue),
    Container {
        node: Node,
        kind: &'static str,
        len: usize,
    },
}

impl Descriptor {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_text_has_universal_newlines() {
        let text = universal_newlines(&PyStr::new("a\r\nb\rc\n"));
        assert_eq!(text.to_utf8().unwrap(), "a\nb\nc\n");
    }
}
