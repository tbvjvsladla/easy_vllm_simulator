//! The workspace secret file `envs/.env` seen through its kind and mode bits only (plan_26112012 #2 · #2b, h56 (ii)).
//!
//! * The file is looked at with `lstat`: a symbolic link is never followed, the content is never opened or read, and no key
//!   name or value is ever returned.
//! * The one correction narrows: the new mode is `mode & 0o600` (owner read/write at most), so a mode is never widened
//!   (664 → 600, 640 → 600, 700 → 600, 600 and 400 stay). A symbolic link, a non-regular file or a failed `lstat` is
//!   `unknown`: it is never taken for 600 and never corrected.
//! * The path is fixed here ([`ENV_FILE_REL`] under the workspace root the caller derives itself); no command line carries it.
//!
//! One implementation for the sub's equip tool (`check_sub_hint_equip --narrow-env-mode`, terraforming_node) and the main's scan
//! (`scan_node --narrow-env-mode`, adversarial-benchmark); both executables already depend on this crate.

use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// The workspace secret file, relative to the workspace root.
pub const ENV_FILE_REL: &str = "envs/.env";
/// The widest mode the file keeps: owner read and write.
pub const NARROWEST_ALLOWED_MODE: u32 = 0o600;
const PERMISSION_BITS: u32 = 0o7777;

/// What `lstat` of the file says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvFileState {
    Absent,
    /// A regular file with these permission bits (setuid/setgid/sticky included).
    Regular(u32),
    /// A symbolic link, a directory or another non-regular file, or an `lstat` that failed for another reason than absence.
    Unknown,
}

impl EnvFileState {
    /// A regular file with a bit outside owner read/write.
    pub fn is_wide(self) -> bool {
        matches!(self, EnvFileState::Regular(mode) if mode & !NARROWEST_ALLOWED_MODE != 0)
    }

    /// `absent` | `ok` (600 or narrower) | `wide` | `unknown`.
    pub fn status(self) -> &'static str {
        match self {
            EnvFileState::Absent => "absent",
            EnvFileState::Unknown => "unknown",
            EnvFileState::Regular(_) if self.is_wide() => "wide",
            EnvFileState::Regular(_) => "ok",
        }
    }

    /// The permission bits as octal digits (`664`) for a regular file, nothing otherwise.
    pub fn mode_text(self) -> Option<String> {
        match self {
            EnvFileState::Regular(mode) => Some(format!("{mode:03o}")),
            _ => None,
        }
    }

    fn mode_value(self) -> PyValue {
        self.mode_text().map_or(PyValue::None, |text| PyValue::str(&text))
    }
}

/// `lstat` of `<workspace>/envs/.env`.
pub fn observe(workspace: &Path) -> EnvFileState {
    match std::fs::symlink_metadata(workspace.join(ENV_FILE_REL)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => EnvFileState::Absent,
        Err(_) => EnvFileState::Unknown,
        Ok(meta) if meta.file_type().is_file() => EnvFileState::Regular(meta.permissions().mode() & PERMISSION_BITS),
        Ok(_) => EnvFileState::Unknown,
    }
}

/// What the correction did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NarrowAction {
    Narrowed,
    Unchanged,
    Absent,
    /// Not a regular file, `lstat` failed, or the mode after the change is not the narrowed one: not corrected, a human looks.
    Unknown,
}

impl NarrowAction {
    pub fn as_str(self) -> &'static str {
        match self {
            NarrowAction::Narrowed => "narrowed",
            NarrowAction::Unchanged => "unchanged",
            NarrowAction::Absent => "absent",
            NarrowAction::Unknown => "unknown",
        }
    }
}

/// One correction: the measured state before and after (both from `lstat`) and the action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Narrowing {
    pub before: EnvFileState,
    pub after: EnvFileState,
    pub action: NarrowAction,
}

/// Narrows `<workspace>/envs/.env` to `mode & 0o600` when it is a regular file wider than that; never widens, never follows a link,
/// never opens the file. The state after is measured again (a mode that did not become the narrowed one is `unknown`).
pub fn narrow(workspace: &Path) -> Narrowing {
    let before = observe(workspace);
    let unchanged = |action| Narrowing { before, after: before, action };
    let mode = match before {
        EnvFileState::Absent => return unchanged(NarrowAction::Absent),
        EnvFileState::Unknown => return unchanged(NarrowAction::Unknown),
        EnvFileState::Regular(_) if !before.is_wide() => return unchanged(NarrowAction::Unchanged),
        EnvFileState::Regular(mode) => mode,
    };
    let narrowed = mode & NARROWEST_ALLOWED_MODE;
    let set = std::fs::set_permissions(workspace.join(ENV_FILE_REL), std::fs::Permissions::from_mode(narrowed));
    let after = observe(workspace);
    let action = if set.is_ok() && after == EnvFileState::Regular(narrowed) { NarrowAction::Narrowed } else { NarrowAction::Unknown };
    Narrowing { before, after, action }
}

fn object(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut out = PyDict::new();
    for (key, value) in entries {
        out.set(key, value);
    }
    PyValue::Dict(out)
}

/// The record of one correction: `{path_rel, before, after, action, provenance: measured}` (before/after = octal digits or null).
pub fn narrowing_value(narrowing: &Narrowing) -> PyValue {
    object(vec![
        ("path_rel", PyValue::str(ENV_FILE_REL)),
        ("before", narrowing.before.mode_value()),
        ("after", narrowing.after.mode_value()),
        ("action", PyValue::str(narrowing.action.as_str())),
        ("provenance", PyValue::str("measured")),
    ])
}
