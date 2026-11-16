//! Rust self-test suites that replaced the five `scripts/selftest_*.py` unittest programs (plan_26111615 pass 3).
//!
//! Each suite is a module with a `run() -> i32` entry point and a `src/bin/selftest_<name>.rs` executable started by the
//! `scripts/selftest_<name>.sh` launcher. The originals patched module internals with `unittest.mock`; here the same
//! observations come from the library's own injection seams (`invoke_with`, thread-local check hooks), from stub
//! executables placed first on `PATH` inside a private temporary directory, and from fixture trees. No Python is started.
//!
//! Output: one `  PASS <label>` / `  FAIL <label>` line per assertion group and a final verdict line; exit 0 only when
//! every check passed.

pub mod bootstrap_permissions;
pub mod campaign_lifecycle;
pub mod claude_native_events;
pub mod harness_switch;
pub mod sub_hint_write_scopes;

use agent_runtime_support::pyvalue::PyValue;
use std::path::{Path, PathBuf};

/// The verdict collector: `ck(label, cond)` prints one line and remembers failures.
pub struct Checks {
    pub suite: &'static str,
    pub failures: Vec<String>,
    pub count: usize,
}

impl Checks {
    pub fn new(suite: &'static str) -> Checks {
        Checks { suite, failures: vec![], count: 0 }
    }
    /// One assertion (or one assertion group): `label` and whether it held; `detail` is shown only on failure.
    pub fn ck(&mut self, label: &str, cond: bool, detail: impl FnOnce() -> String) {
        self.count += 1;
        if cond {
            println!("  PASS {label}");
        } else {
            let d = detail();
            println!("  FAIL {label}{}", if d.is_empty() { String::new() } else { format!(" — {d}") });
            self.failures.push(label.to_string());
        }
    }
    pub fn group(&self, name: &str) {
        println!("## {name}");
    }
    /// The verdict line and the exit status.
    pub fn finish(self) -> i32 {
        if self.failures.is_empty() {
            println!("[{}] PASS ({} checks)", self.suite, self.count);
            0
        } else {
            println!("[{}] FAIL {} of {}: {:?}", self.suite, self.failures.len(), self.count, self.failures);
            1
        }
    }
}

/// A private temporary directory under `TMPDIR`, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> TempDir {
        let base = std::env::temp_dir();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        for k in 0..10000u32 {
            let p = base.join(format!("{prefix}{:08x}", seed.wrapping_add(k.wrapping_mul(2654435761)) ^ std::process::id()));
            if std::fs::create_dir(&p).is_ok() {
                return TempDir(std::fs::canonicalize(&p).unwrap_or(p));
            }
        }
        panic!("no temporary directory");
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes an executable shell stub (`#!/bin/bash` + body).
pub fn stub(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).expect("stub dir");
    }
    std::fs::write(path, format!("#!/bin/bash\n{body}")).expect("stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).expect("stub mode");
}

pub fn write(path: &Path, text: &str) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).expect("parent dir");
    }
    std::fs::write(path, text).expect("write");
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// `json.loads` of a fixture literal (a bad literal is a bug of the suite).
pub fn j(text: &str) -> PyValue {
    agent_runtime_support::pyjson::loads(text).unwrap_or_else(|e| panic!("fixture JSON: {e}: {text}"))
}

/// `json.dumps(v)` (ASCII, Python separators).
pub fn dumps(v: &PyValue) -> String {
    v.dumps_ascii().expect("JSON-safe")
}

/// `PATH` with `dir` first.
pub fn path_with(dir: &Path) -> String {
    format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default())
}

/// The repository under test: four levels above the scripts directory the launcher names (`TERRAFORMING_OWNER_PATH`),
/// else derived from this executable's place (`<repo>/.claude/skills/terraforming_node/rust/target/<profile>/<bin>`).
pub fn repo_root() -> PathBuf {
    crate::owner::owner_path("selftest.py").ancestors().nth(5).map(Path::to_path_buf).unwrap_or_default()
}

/// The terraforming scripts directory of this repository.
pub fn scripts_dir() -> PathBuf {
    repo_root().join(".claude/skills/terraforming_node/scripts")
}

/// Every file below `root` (relative path → bytes), for "nothing was written" comparisons.
pub fn snapshot(roots: &[&Path], base: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    fn walk(p: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let path = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                walk(&path, base, out);
            } else if ft.is_file() {
                let rel = path.strip_prefix(base).unwrap_or(&path).display().to_string();
                out.push((rel, std::fs::read(&path).unwrap_or_default()));
            }
        }
    }
    for r in roots {
        walk(r, base, &mut out);
    }
    out.sort();
    out
}

/// `git <args>` in `cwd` with a fixed identity and no global/system configuration; panics on failure (fixture setup).
pub fn git(cwd: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}
