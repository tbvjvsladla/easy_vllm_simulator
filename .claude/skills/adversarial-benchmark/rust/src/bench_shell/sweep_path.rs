//! `sweep-path`: the one place a sweep's output directory is chosen, checked and (for a fresh measurement) claimed.

use super::{Console, Context, Done, Stop};
use crate::guidellm::Failure;
use crate::ospath;
use crate::pyfs::{is_dir, mkdir_parents_exist_ok, mkdir_plain, normalize, os_string, FsFailure};
use crate::pystr::PyStr;
use std::path::Path;

/// `print("[sweep_bench] ERROR --sweep-dir: " + reason, file=sys.stderr)` and `SystemExit(2)`.
fn refuse(console: &mut Console, reason: &str) -> Stop {
    console.warn(&format!("[sweep_bench] ERROR --sweep-dir: {reason}\n"));
    Stop::Exit(2)
}

/// The components of `PurePosixPath(text).parts` without the root.
fn components(text: &PyStr) -> Vec<PyStr> {
    let slash = u32::from('/');
    text.code_points()
        .split(|&point| point == slash)
        .filter(|part| !part.is_empty() && *part != [u32::from('.')])
        .map(|part| PyStr::from_code_points(part.to_vec()))
        .collect()
}

/// `Path.is_symlink()`: errors that mean "no such file" are `False`, any other is raised.
fn is_symlink(path: &PyStr) -> Result<bool, Failure> {
    let os = match os_string(path) {
        Ok(os) => os,
        Err(_) => return Ok(false),
    };
    match std::fs::symlink_metadata(Path::new(&os)) {
        Ok(data) => Ok(data.file_type().is_symlink()),
        Err(error) if matches!(error.raw_os_error(), Some(2 | 20 | 9 | 40)) => Ok(false),
        Err(error) => Err(FsFailure::Os {
            error,
            path: path.clone(),
        }
        .into_failure()),
    }
}

/// `Path.resolve()` (non-strict): symbolic links of the existing part followed, the rest kept and normalised.
fn resolve(path: &PyStr) -> Result<PyStr, Failure> {
    let absolute = ospath::abspath(path)?;
    let mut existing = absolute.clone();
    let mut rest: Vec<PyStr> = Vec::new();
    loop {
        let os = os_string(&existing).map_err(FsFailure::into_failure)?;
        match std::fs::canonicalize(Path::new(&os)) {
            Ok(real) => {
                let mut resolved = PyStr::from_surrogateescape(
                    std::os::unix::ffi::OsStrExt::as_bytes(real.as_os_str()),
                );
                for part in rest.iter().rev() {
                    resolved = ospath::join(&resolved, &[part]);
                }
                return Ok(ospath::normpath(&resolved));
            }
            Err(_) => {
                let (head, tail) = ospath::split(&existing);
                if tail.is_empty() || head == existing {
                    return Ok(absolute);
                }
                rest.push(tail);
                existing = head;
            }
        }
    }
}

fn is_sweep_name(name: &PyStr) -> bool {
    let Some(text) = name.to_utf8() else {
        return false;
    };
    let Some(tail) = text.strip_prefix("sweep_") else {
        return false;
    };
    let mut chars = tail.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// `sweep-path REPO TOPOLOGY PATH REASSEMBLE ACTION`: prints the checked directory; `claim` creates it atomically.
pub(super) fn sweep_path(context: &Context<'_>, console: &mut Console) -> Done {
    if context.args.len() != 5 {
        let count = context.args.len();
        return Err(Failure::new(
            "ValueError",
            &if count > 5 {
                "too many values to unpack (expected 5)".to_string()
            } else {
                format!("not enough values to unpack (expected 5, got {count})")
            },
        )
        .into());
    }
    let (repo_text, topology, raw, reassemble, action) = (
        &context.args[0],
        &context.args[1],
        &context.args[2],
        &context.args[3],
        &context.args[4],
    );
    if !(topology.equals("single") || topology.equals("multi")) || raw.is_empty() {
        return Err(refuse(
            console,
            "explicit topology single|multi and a nonempty path are required",
        ));
    }
    let repo = resolve(repo_text)?;
    if components(raw).iter().any(|part| part.equals("..")) {
        return Err(refuse(console, "parent traversal is not allowed"));
    }
    let path = if raw.code_points().first() == Some(&u32::from('/')) {
        normalize(raw)
    } else {
        normalize(&ospath::join(&repo, &[raw]))
    };
    let output = ospath::join_all(&repo, &["output"]);
    let topology_dir = ospath::join(&output, &[topology]);
    let parent = ospath::join_all(&topology_dir, &["benchlog"]);
    let (path_parent, name) = ospath::split(&path);
    if normalize(&path_parent) != normalize(&parent) || !is_sweep_name(&name) {
        return Err(refuse(
            console,
            "choose one sweep_* directory directly under output/<topology>/benchlog",
        ));
    }
    for part in [&output, &topology_dir, &parent, &path] {
        if is_symlink(part)? {
            return Err(refuse(console, "symlink paths are not allowed"));
        }
    }
    let reassembling = reassemble.equals("1");
    if reassembling {
        if !is_dir(&path)? {
            return Err(refuse(console, "reassembly requires an existing directory"));
        }
    } else if ospath::lexists(&path) {
        return Err(refuse(
            console,
            "fresh measurement requires an absent directory; existing evidence is preserved",
        ));
    }
    if action.equals("claim") && !reassembling {
        let claimed = (|| -> Result<(), Result<Failure, ()>> {
            mkdir_parents_exist_ok(&parent).map_err(Ok)?;
            if resolve(&parent).map_err(Ok)? != parent {
                return Err(Err(()));
            }
            mkdir_plain(&path).map_err(Ok)
        })();
        match claimed {
            Ok(()) => {}
            Err(Err(())) => return Err(refuse(console, "output parent changed to a symlink")),
            Err(Ok(failure)) if failure.class.ends_with("Error") && is_os_error(&failure) => {
                return Err(refuse(
                    console,
                    &format!(
                        "cannot claim fresh output: {}",
                        failure.message.to_utf8().unwrap_or_default()
                    ),
                ));
            }
            Err(Ok(failure)) => return Err(failure.into()),
        }
    }
    console.print(path);
    Ok(())
}

/// Whether the failure is an `OSError` (a subclass of it), the only class the claim step turns into a refusal.
fn is_os_error(failure: &Failure) -> bool {
    matches!(
        failure.class,
        "OSError"
            | "FileNotFoundError"
            | "FileExistsError"
            | "PermissionError"
            | "NotADirectoryError"
            | "IsADirectoryError"
            | "BlockingIOError"
            | "InterruptedError"
            | "TimeoutError"
            | "BrokenPipeError"
            | "ChildProcessError"
            | "ConnectionAbortedError"
            | "ConnectionResetError"
            | "ConnectionRefusedError"
            | "ProcessLookupError"
    )
}
