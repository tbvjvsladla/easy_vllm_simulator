//! Where the deployed owner is: the root of the skill a binary belongs to, derived from the binary's own path, never from the
//! working directory, the build tree or an environment variable. The deployed layout puts the executables at
//! `<skill root>/rust/runtime/target/release/<program>`, so the root is the path five directories up, taken lexically (links
//! in the path are not resolved, as `os.path.abspath(__file__)` does not resolve them). Two places hang off the root: the
//! default feedback log, `feedback/recipe_feedback.jsonl`, and the companion trial sources of the Rust trial owner
//! (`rust/runtime/src/run_trial.rs`, `rust/runtime/src/run_trial_session.rs`) that the parity check reads. A binary that runs from another layout gets a root that is simply what five directories up is there;
//! callers that know better pass the root or the path explicitly.

use crate::feedback::{abspath, dirname};
use crate::model_files::{join, os_failure};
use crate::pyexpr::Res;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::os::unix::ffi::OsStrExt;

/// The skill root of the program at `executable` (an absolute path): five `dirname`s up.
pub fn skill_root_of(executable: &PyStr) -> PyStr {
    let mut root = executable.clone();
    for _ in 0..5 {
        root = dirname(&root);
    }
    root
}

/// The skill root of the running program: from the path it was started by when that names a directory (lexical absolute path),
/// otherwise from the path of the executable the system reports.
pub fn process_skill_root() -> Res<PyStr> {
    let started_by = std::env::args_os().next();
    let executable = match started_by {
        Some(name) if name.as_bytes().contains(&b'/') => {
            abspath(&PyValue::Str(PyStr::from_surrogateescape(name.as_bytes())))?
        }
        _ => {
            let path = std::env::current_exe().map_err(|error| os_failure(&error, None))?;
            PyStr::from_surrogateescape(path.as_os_str().as_bytes())
        }
    };
    Ok(skill_root_of(&executable))
}

/// `<root>/feedback/recipe_feedback.jsonl` (`DEFAULT_LOG_PATH`).
pub fn default_log_path(skill_root: &PyStr) -> PyStr {
    join(
        &join(skill_root, &PyStr::new("feedback")),
        &PyStr::new("recipe_feedback.jsonl"),
    )
}

/// The companion sources of the parity check, in reading order: `<root>/rust/runtime/src/run_trial.rs` and
/// `<root>/rust/runtime/src/run_trial_session.rs` (the trial is the Rust program; the retired `scripts/run_trial.py` is not read).
pub fn trial_source_paths(skill_root: &PyStr) -> Vec<PyStr> {
    let sources = join(
        &join(&join(skill_root, &PyStr::new("rust")), &PyStr::new("runtime")),
        &PyStr::new("src"),
    );
    ["run_trial.rs", "run_trial_session.rs"]
        .iter()
        .map(|name| join(&sources, &PyStr::new(name)))
        .collect()
}
