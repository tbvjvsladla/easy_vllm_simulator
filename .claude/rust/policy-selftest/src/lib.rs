//! Rust port of the policies runtime self-checks (`.claude/policies/runtime/`):
//! `runtime_selftest.py` (pre-commit tripwires and the regression suite), `antipattern_scan.py` and
//! `constitution_lint.py`. No Python is started; git, node (wiki-desk TS scanner) and bash children are the
//! originals' own.
pub mod antipattern;
pub mod argp;
pub mod certkey;
pub mod constitution_lint;
pub mod gitx;
pub mod hint_suite;
pub mod pjson;
pub mod pyast;
pub mod pytext;
pub mod selftest;
pub mod suite;
pub mod tempdir;
pub mod tripwires;

use std::path::PathBuf;

/// The repository root the original derived from `__file__` (`.claude/policies/runtime/<x>.py` -> three parents up).
///
/// The launcher execs the binary with argv[0] = `<repo>/.claude/policies/runtime/<x>.py`; a direct start of the
/// binary (`<repo>/.claude/rust/policy-selftest/target/<profile>/<bin>`) is resolved from the executable path.
pub fn repo_root() -> PathBuf {
    if let Some(a0) = std::env::args_os().next() {
        let p = PathBuf::from(&a0);
        if p.extension().map_or(false, |e| e == "py") {
            let abs = if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) };
            if let Some(r) = abs.parent().and_then(|d| d.parent()).and_then(|d| d.parent()).and_then(|d| d.parent()) {
                return std::fs::canonicalize(r).unwrap_or_else(|_| r.to_path_buf());
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
        let mut cur = exe.as_path();
        while let Some(parent) = cur.parent() {
            if parent.ends_with(".claude/rust") {
                if let Some(root) = parent.parent().and_then(|c| c.parent()) {
                    return root.to_path_buf();
                }
            }
            cur = parent;
        }
    }
    PathBuf::from(".")
}

pub fn args() -> Vec<String> {
    std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect()
}
