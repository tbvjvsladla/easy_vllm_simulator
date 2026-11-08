//! The read side of `campaign_init.py` that the topology predicate and the benchmark callers use: `_read_json`,
//! `read_declaration`, `campaigns_dir`, `active_campaign_id`, `derive_path` and the constants and the exception they need.
//! No writer, relay or campaign action is ported.
//!
//! **Scope (contract C3).** This is the *fixed current source* of that module. The original predicate loaded the checkout's
//! `campaign_init.py` with Python's import system and called it; a checkout whose copy is some other program (top-level
//! raise, syntax error, empty module, `sys.exit`) is a different contract that cannot be reproduced without running Python.
//! Callers that need to tell the two apart ask [`crate::topology::Predicate`] to gate on a registered source identity
//! (see `ROOT_ABI_PROPOSAL.json`); nothing here pretends that an arbitrary file is the current source.
//!
//! Values keep Python's types where the original's callers can observe them: the declaration is a JSON document
//! ([`PyValue`], big integers exact); paths are byte strings joined with `pathlib`'s normalization rules.

use crate::topology::{Crash, Detail, Frame, Where};
use agent_runtime_support::pyjson::{decode_utf8_strict, loads_text, utf8_error_text, LoadError};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// The reserved id of "no active campaign" (`BOOTSTRAP`).
pub const BOOTSTRAP: &str = "_bootstrap";
/// `RESERVED_IDS`.
pub const RESERVED_IDS: [&str; 2] = ["_template", BOOTSTRAP];
/// The owner module, relative to a checkout.
pub const MODULE_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py";

/// `PurgeGateRefusal`: the exception `derive_path` raises (its text is the message).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurgeGateRefusal(pub PyStr);

/// `str(PurePosixPath(text))` over any code unit (bytes of an OS path, or the code points of a Python `str`): repeated slashes
/// collapse (exactly two leading slashes stay), `.` components go, `..` stays. One rule for both so that path *text* can keep
/// lone surrogates and NUL until it reaches the file system.
fn normalize_units<U: Copy + PartialEq>(path: &[U], slash: U, dot: U) -> Vec<U> {
    if path.is_empty() {
        return vec![dot];
    }
    let leading = path.iter().take_while(|unit| **unit == slash).count();
    let mut out = Vec::new();
    match leading {
        0 => {}
        2 => out.extend([slash, slash]),
        _ => out.push(slash),
    }
    let parts = path
        .split(|unit| *unit == slash)
        .filter(|part| !part.is_empty() && !(part.len() == 1 && part[0] == dot));
    for (index, part) in parts.enumerate() {
        if index > 0 {
            out.push(slash);
        }
        out.extend_from_slice(part);
    }
    if out.is_empty() {
        out.push(dot);
    }
    out
}

/// `Path(base) / part` over any code unit (an absolute `part` replaces `base`).
fn join_units<U: Copy + PartialEq>(base: &[U], part: &[U], slash: U, dot: U) -> Vec<U> {
    if part.first() == Some(&slash) {
        return normalize_units(part, slash, dot);
    }
    let mut text = normalize_units(base, slash, dot);
    if text.len() == 1 && text[0] == dot {
        text.clear();
    } else {
        text.push(slash);
    }
    text.extend_from_slice(part);
    normalize_units(&text, slash, dot)
}

/// `str(PurePosixPath(text))` for an OS path.
pub fn normalize(path: &[u8]) -> Vec<u8> {
    normalize_units(path, b'/', b'.')
}

/// `Path(base) / part` for OS paths.
pub fn join(base: &[u8], part: &[u8]) -> Vec<u8> {
    join_units(base, part, b'/', b'.')
}

/// `str(Path(base) / part)` for Python text, kept as code points (lone surrogates and NUL are legal path text; only the file
/// system rejects them).
pub fn join_text(base: &PyStr, part: &PyStr) -> PyStr {
    PyStr::from_code_points(join_units(
        base.code_points(),
        part.code_points(),
        u32::from(b'/'),
        u32::from(b'.'),
    ))
}

fn os_path(bytes: &[u8]) -> &Path {
    Path::new(std::ffi::OsStr::from_bytes(bytes))
}

/// `Path.is_file()`: `ENOENT`, `ENOTDIR`, `EBADF`, `ELOOP` and an embedded NUL are "no"; any other failure of `stat` is raised.
pub fn is_file(path: &[u8]) -> Result<bool, std::io::Error> {
    stat_kind(path).map(|kind| kind == Some(false))
}

/// `Path.is_dir()` (same exception rule).
pub fn is_dir(path: &[u8]) -> Result<bool, std::io::Error> {
    stat_kind(path).map(|kind| kind == Some(true))
}

/// `Some(is_dir)` for something that exists, `None` for the failures Python's `pathlib` ignores.
fn stat_kind(path: &[u8]) -> Result<Option<bool>, std::io::Error> {
    if path.contains(&0) {
        return Ok(None);
    }
    match std::fs::metadata(os_path(path)) {
        Ok(meta) => Ok(Some(meta.is_dir())),
        Err(error) => match error.raw_os_error() {
            Some(2 | 20 | 9 | 40) => Ok(None),
            _ => Err(error),
        },
    }
}

/// `strerror(errno)` as Python's `OSError` text: `[Errno N] message: 'path'`.
pub fn os_error_text(error: &std::io::Error, path: &[u8]) -> PyStr {
    let number = error.raw_os_error().unwrap_or(0);
    let full = error.to_string();
    let message = full
        .strip_suffix(&format!(" (os error {number})"))
        .unwrap_or(&full);
    let mut text = PyStr::new(&format!("[Errno {number}] {message}: "));
    text.push_str(&crate::pyprofile::fs_decode(path).repr());
    text
}

/// `Path.read_text(encoding="utf-8")`: `Err(Ok(error))` is an `OSError` (what `open` raises), `Err(Err(message))` the
/// `UnicodeDecodeError` text of the read. Universal newlines apply.
pub enum ReadFailure {
    Os(std::io::Error),
    Decode(DecodeInfo),
}

/// A `UnicodeDecodeError` of `read_text`: the text, the object being decoded and the span/reason of the first bad sequence.
pub struct DecodeInfo {
    pub message: String,
    pub object: Vec<u8>,
    pub start: usize,
    pub end: usize,
    pub reason: String,
}

/// The `Detail` of an `OSError` raised by a system call on `path`.
pub fn os_detail(error: &std::io::Error, path: &[u8]) -> Detail {
    let number = error.raw_os_error().unwrap_or(0);
    let full = error.to_string();
    let strerror = full
        .strip_suffix(&format!(" (os error {number})"))
        .unwrap_or(&full)
        .to_string();
    Detail::Os {
        errno: number,
        strerror,
        filename: path.to_vec(),
    }
}

pub fn read_text(path: &[u8]) -> Result<PyStr, ReadFailure> {
    let mut file = std::fs::File::open(os_path(path)).map_err(ReadFailure::Os)?;
    if file.metadata().map(|m| m.is_dir()).unwrap_or(false) {
        return Err(ReadFailure::Os(std::io::Error::from_raw_os_error(21)));
    }
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut bytes).map_err(ReadFailure::Os)?;
    match std::str::from_utf8(&bytes) {
        Ok(text) => Ok(PyStr::new(&text.replace("\r\n", "\n").replace('\r', "\n"))),
        Err(error) => {
            let (start, end, reason) =
                agent_runtime_support::pyjson::utf8_error_parts(&bytes, &error);
            Err(ReadFailure::Decode(DecodeInfo {
                message: utf8_error_text(&bytes, &error),
                object: bytes.clone(),
                start,
                end,
                reason: reason.to_string(),
            }))
        }
    }
}

/// `_read_json(path)`: the decoded document, or `None` for an `OSError` or a `ValueError` (invalid JSON or invalid UTF-8).
/// A document nested past the recursion limit is the `RecursionError` the original lets through.
pub fn read_json(path: &[u8]) -> Result<Option<PyValue>, Crash> {
    let Ok(text) = read_text(path) else {
        return Ok(None);
    };
    match loads_text(&text) {
        Ok(value) => Ok(Some(value)),
        Err(error @ LoadError::TooDeep { .. }) => Err(Crash::new(
            "RecursionError",
            &PyStr::new(&format!("{error}")),
            Where::Opaque,
        )),
        Err(_) => Ok(None),
    }
}

/// `read_declaration(base)`: the declaration if it is a JSON object, else `{}`.
pub fn read_declaration(base: &[u8]) -> Result<PyDict, Crash> {
    Ok(match read_json(&join(base, b"campaign.yaml"))? {
        Some(PyValue::Dict(dict)) => dict,
        _ => PyDict::new(),
    })
}

/// `campaigns_dir(repo_root)`: `repo_root` or the module's own root (`module_root`) joined with `campaigns`.
pub fn campaigns_dir(repo_root: Option<&[u8]>, module_root: &[u8]) -> Vec<u8> {
    match repo_root {
        None => join(module_root, b"campaigns"),
        Some(root) => join(root, b"campaigns"),
    }
}

/// `active_campaign_id(repo_root)`: the id the `ACTIVE` pointer names when that is an existing directory, else `_bootstrap`.
/// A pointer that cannot be read or decoded is the exception Python raises; `module_site` is the checkout path of the
/// owner module (traceback context, not an OS stack).
pub fn active_campaign_id(
    repo_root: Option<&[u8]>,
    module_root: &[u8],
    module_file: &[u8],
) -> Result<PyStr, Crash> {
    let base = campaigns_dir(repo_root, module_root);
    let pointer = join(&base, b"ACTIVE");
    let site = |frames: Vec<Frame>, class: &'static str, message: PyStr, detail: Detail| {
        Crash::new(
            class,
            &message,
            Where::Campaign {
                module: module_file.to_vec(),
                frames,
            },
        )
        .with_detail(detail)
    };
    let pointer_is_file = is_file(&pointer).map_err(|error| {
        site(
            Vec::new(),
            os_class(&error),
            os_error_text(&error, &pointer),
            os_detail(&error, &pointer),
        )
    })?;
    if pointer_is_file {
        let text = match read_text(&pointer) {
            Ok(text) => text,
            Err(ReadFailure::Os(error)) => {
                let class = os_class(&error);
                return Err(site(
                    Frame::pathlib_open(),
                    class,
                    os_error_text(&error, &pointer),
                    os_detail(&error, &pointer),
                ));
            }
            Err(ReadFailure::Decode(info)) => {
                return Err(site(
                    Frame::pathlib_read(),
                    "UnicodeDecodeError",
                    PyStr::new(&info.message),
                    Detail::Decode {
                        encoding: "utf-8",
                        object: info.object,
                        start: info.start,
                        end: info.end,
                        reason: info.reason,
                    },
                ));
            }
        };
        let name = text.strip();
        if !name.code_points().is_empty() {
            let bytes = match crate::pyprofile::fs_encode(&name) {
                Ok(bytes) => bytes,
                Err(_) => return Ok(PyStr::new(BOOTSTRAP)),
            };
            let directory = is_dir(&join(&base, &bytes)).map_err(|error| {
                site(
                    Vec::new(),
                    os_class(&error),
                    os_error_text(&error, &join(&base, &bytes)),
                    os_detail(&error, &join(&base, &bytes)),
                )
            })?;
            if directory {
                return Ok(name);
            }
        }
    }
    Ok(PyStr::new(BOOTSTRAP))
}

/// The `OSError` subclass Python raises for the errno.
pub fn os_class(error: &std::io::Error) -> &'static str {
    match error.raw_os_error() {
        Some(1 | 13) => "PermissionError",
        Some(2) => "FileNotFoundError",
        Some(17) => "FileExistsError",
        Some(20) => "NotADirectoryError",
        Some(21) => "IsADirectoryError",
        _ => "OSError",
    }
}

/// `derive_path(kind, cell=, sweep=, context=, camp_id=, repo_root=)` for the kinds the read side needs (`root`, `config`,
/// `lockset`, `cell-status`, `sweep`): the producer path as a normalized byte string, or the refusal the original raises.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Derived {
    Path(Vec<u8>),
    Refusal(PurgeGateRefusal),
}

/// The module's own root ( of the module) and file, which  calls fall back to.
#[derive(Clone, Copy, Debug)]
pub struct ModuleSite<'a> {
    pub root: &'a [u8],
    pub file: &'a [u8],
}

pub fn derive_path(
    kind: &str,
    cell: Option<&str>,
    sweep: Option<&str>,
    context: Option<&str>,
    camp_id: Option<&PyStr>,
    repo_root: Option<&[u8]>,
    site: ModuleSite<'_>,
) -> Result<Derived, Crash> {
    let (module_root, module_file) = (site.root, site.file);
    let camp = match camp_id.filter(|id| !id.code_points().is_empty()) {
        Some(id) => id.clone(),
        None => active_campaign_id(repo_root, module_root, module_file)?,
    };
    let camp_bytes = crate::pyprofile::fs_encode(&camp)
        .map_err(|failure| crate::topology::encode_crash(&camp, &failure))?;
    let base = join(&campaigns_dir(repo_root, module_root), &camp_bytes);
    let refuse = |text: String| Derived::Refusal(PurgeGateRefusal(PyStr::new(&text)));
    Ok(match kind {
        "root" => Derived::Path(base),
        "config" | "lockset" | "cell-status" => match cell.filter(|c| !c.is_empty()) {
            None => refuse(format!(
                "--derive {kind} 는 --cell 이 필요하다(셀 없는 셀 입력은 없다)"
            )),
            Some(cell) => {
                let name = match kind {
                    "config" => "config.yaml",
                    "lockset" => "lockset.json",
                    _ => "cell.status.json",
                };
                Derived::Path(join(
                    &join(&join(&base, b"cells"), cell.as_bytes()),
                    name.as_bytes(),
                ))
            }
        },
        "sweep" => match sweep.filter(|s| !s.is_empty()) {
            None => refuse("--derive sweep 은 --sweep 이 필요하다".to_string()),
            Some(sweep) => Derived::Path(join(
                &join(&base, b"sweeps"),
                format!("{sweep}.json").as_bytes(),
            )),
        },
        "relay" => {
            let name = context
                .filter(|c| !c.is_empty())
                .map(|c| format!("{c}.json"))
                .unwrap_or_default();
            Derived::Path(join(&join(&base, b"relay"), name.as_bytes()))
        }
        "relay-root" => Derived::Path(join(&base, b"relay")),
        "pending-hitl" => Derived::Path(join(&join(&base, b"relay"), b"pending_hitl.json")),
        "evidence" => Derived::Path(join(&base, b"evidence_pointers.json")),
        "phases" => Derived::Path(join(&base, b"phases")),
        other => refuse(format!(
            "알 수 없는 파생 종류: {}",
            PyStr::new(other).repr()
        )),
    })
}

/// Strict UTF-8 decode helper shared with the predicate (`bytes.decode("utf-8")`).
pub fn decode(bytes: &[u8]) -> Result<PyStr, String> {
    decode_utf8_strict(bytes).map_err(|_| match std::str::from_utf8(bytes) {
        Err(error) => utf8_error_text(bytes, &error),
        Ok(_) => String::new(),
    })
}

// ── the loaded module's own location, and the typed `derive_path("root", camp_id=...)` the repeat callers ask ──────────────

/// Where a loaded `campaign_init.py` believes it lives: `REPO_ROOT = Path(__file__).resolve().parents[4]` and the path it was
/// loaded from. The module-global root (not a caller's checkout) is what `active_campaign_id()` and `derive_path` use when
/// they are called without a `repo_root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedModuleSite {
    pub root: Vec<u8>,
    pub file: Vec<u8>,
}

impl OwnedModuleSite {
    pub fn as_site(&self) -> ModuleSite<'_> {
        ModuleSite {
            root: &self.root,
            file: &self.file,
        }
    }
}

/// The import-time computation of the module: `Path(module).resolve().parents[4]`. A symlink is followed (so the root may
/// belong to another checkout), a symlink loop or an unreadable link is the exception `resolve` raises, and a resolved path
/// with fewer than five components is `IndexError(4)` (its text is `4`).
pub fn module_site(module: &[u8]) -> Result<OwnedModuleSite, Crash> {
    let resolved = crate::topology::resolve_checked(module)?;
    let parts: Vec<&[u8]> = resolved
        .split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 5 {
        return Err(Crash::new(
            "IndexError",
            &PyStr::new("4"),
            Where::Campaign {
                module: module.to_vec(),
                frames: Vec::new(),
            },
        ));
    }
    let mut root = Vec::new();
    for part in &parts[..parts.len() - 5] {
        root.push(b'/');
        root.extend_from_slice(part);
    }
    if root.is_empty() {
        root.push(b'/');
    }
    Ok(OwnedModuleSite {
        root,
        file: module.to_vec(),
    })
}

/// `str(derive_path("root", camp_id=camp_id))` as the repeat callers use it, with any JSON value as `camp_id`: a falsy value
/// means the active campaign (`camp_id or active_campaign_id()`), a text is a path component (an absolute one replaces the
/// campaigns directory, `..` stays lexical), any other truthy type is the `TypeError` of `Path / value`. The result stays
/// path *text* - nothing is encoded for the file system here.
pub fn derive_root_path(camp_id: &PyValue, site: ModuleSite<'_>) -> Result<PyStr, Crash> {
    let camp = if camp_id.is_truthy() {
        camp_id.clone()
    } else {
        PyValue::Str(active_campaign_id(None, site.root, site.file)?)
    };
    let PyValue::Str(name) = camp.peel() else {
        return Err(Crash::new(
            "TypeError",
            &PyStr::new(&format!(
                "unsupported operand type(s) for /: 'PosixPath' and '{}'",
                camp.type_name()
            )),
            Where::Campaign {
                module: site.file.to_vec(),
                frames: Vec::new(),
            },
        ));
    };
    let campaigns = crate::pyprofile::fs_decode(&campaigns_dir(None, site.root));
    Ok(join_text(&campaigns, name))
}
