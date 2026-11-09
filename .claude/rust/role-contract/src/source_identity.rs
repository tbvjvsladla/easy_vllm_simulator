//! Source-version identity of the two original Python programs the topology predicate stands for: the owner
//! `topology_parity.py` and the campaign module `campaign_init.py`.
//!
//! The native predicate and campaign read side are the semantics of the *reviewed current* sources. A checkout whose copy of
//! either program is different (a modified owner, a campaign module that raises, does not parse, is empty or exits) is a
//! different program; its behavior is not a native answer. This module only decides which side of that line a source is on:
//! the bytes of the checkout's file are compared with the compile-time bytes of the **existing canonical originals** at their
//! canonical relative paths (no second copy, no digest, nothing parsed to guess a Python behavior).
//!
//! A source that is absent is not a different program (the original treats a missing campaign module as an absent leg and
//! a missing owner is its caller's concern); an unreadable one is neither identical nor known to differ and is reported as
//! such.

use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Bytes of the canonical `topology_parity.py` (`.claude/policies/runtime/topology_parity.py`).
pub const REVIEWED_OWNER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../policies/runtime/topology_parity.py"
));
/// Bytes of the canonical `campaign_init.py` (`.claude/skills/terraforming_node/scripts/campaign_init.py`).
pub const REVIEWED_CAMPAIGN: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../skills/terraforming_node/scripts/campaign_init.py"
));
/// Bytes of the reviewed ORIGINAL `manifest_contract.py` (e84fa13c...), kept unchanged as the sibling compat owner
/// (`.claude/skills/terraforming_node/scripts/manifest_contract_compat_owner.py`): `manifest_contract.py` itself is now the thin import adapter.
pub const REVIEWED_MANIFEST_CONTRACT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../skills/terraforming_node/scripts/manifest_contract_compat_owner.py"
));
/// The owner path relative to a checkout.
pub const OWNER_REL: &str = ".claude/policies/runtime/topology_parity.py";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identity {
    /// Byte-identical to the reviewed original.
    Reviewed,
    /// No such file (a missing campaign module is the original's "absent leg").
    Missing,
    /// A different program.
    Different,
    /// Exists but cannot be read (`OSError` text): identity unknown.
    Unreadable(String),
}

/// Compares the file at `path` with `reviewed`.
pub fn identify(path: &[u8], reviewed: &[u8]) -> Identity {
    let target = Path::new(std::ffi::OsStr::from_bytes(path));
    match std::fs::metadata(target) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Identity::Missing,
        Err(error) if matches!(error.raw_os_error(), Some(2 | 20 | 9 | 40)) => {
            return Identity::Missing
        }
        Err(error) => return Identity::Unreadable(error.to_string()),
    }
    match std::fs::read(target) {
        Ok(bytes) if bytes == reviewed => Identity::Reviewed,
        Ok(_) => Identity::Different,
        Err(error) => Identity::Unreadable(error.to_string()),
    }
}

/// What the sources of a run decide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// The reviewed current sources: the native predicate is the original's behavior.
    NativeCurrentSource,
    /// At least one source is a different (or unreadable) program: only the original program itself can answer.
    LegacySourceVariant { reason: String },
    /// The run names an original program that cannot be inspected at all (a named owner that does not exist): neither the
    /// native answer nor the original program's can be given.
    Unmet { reason: String },
}

/// The route for a run of the owner `owner` (the real original program the run stands for, when one is named) over the
/// checkout `root` (`None` when the command does not read a checkout).
pub fn route(owner: Option<&[u8]>, root: Option<&[u8]>) -> Route {
    if let Some(owner) = owner {
        match identify(owner, REVIEWED_OWNER) {
            Identity::Reviewed => {}
            Identity::Missing => {
                return Route::Unmet {
                    reason: "the owner program named by the run does not exist".to_string(),
                }
            }
            Identity::Different => {
                return Route::LegacySourceVariant {
                    reason: "the owner program is not the reviewed topology_parity.py".to_string(),
                }
            }
            Identity::Unreadable(why) => {
                return Route::LegacySourceVariant {
                    reason: format!("the owner program cannot be read ({why})"),
                }
            }
        }
    }
    if let Some(root) = root {
        let module = crate::campaign::join(root, crate::campaign::MODULE_REL.as_bytes());
        match identify(&module, REVIEWED_CAMPAIGN) {
            Identity::Reviewed | Identity::Missing => {}
            Identity::Different => {
                return Route::LegacySourceVariant {
                    reason: "the checkout's campaign_init.py is not the reviewed campaign_init.py"
                        .to_string(),
                }
            }
            Identity::Unreadable(why) => {
                return Route::LegacySourceVariant {
                    reason: format!("the checkout's campaign_init.py cannot be read ({why})"),
                }
            }
        }
    }
    Route::NativeCurrentSource
}
