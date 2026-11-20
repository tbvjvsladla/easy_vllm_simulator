//! Contract checks of the py-boundary-guard bins (plan 26111805 「보존 계약」, cell c21): what their consumers read,
//! through the launchers they call — consumers.tsv rows of `py_boundary_guard` (pre-commit hook), `inline_python_scan`
//! (policy-distribution `check --repo .`, W gate) and `crate_allow` (campaign check `deps-allow`). The bins run for
//! real against throwaway git repositories; the checks fix exit codes, marker and finding line prefixes, the
//! stdout/stderr split and the launcher override variables, not message texts.
//!
//! `py_exec_trap` has no consumer outside its own checks (consumers.tsv: golden(test) rows only) and is not fixed here.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root of this crate (`.claude/rust/py-boundary-guard` → three levels up).
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo(tag: &str) -> Repo {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("pybg-contract-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    Repo(dir)
}

fn git(dir: &Path, args: &[&str]) {
    let st = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
    assert!(st.success(), "git {args:?}");
}

fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

/// `bash <launcher> <args>` with `<VAR>=<bin>` (the consumers' override), cwd `cwd`.
fn launch(launcher: &str, var: &str, bin: &str, cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash").arg(root().join(launcher)).args(args).env(var, bin).current_dir(cwd).output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn code(o: &Output) -> Option<i32> {
    o.status.code()
}

/// Every launcher refuses an absent executable with exit 2 and starts nothing (the hook fails closed on it).
#[test]
fn launchers_exist_and_an_absent_executable_is_exit_2() {
    let r = repo("absent");
    for (launcher, var) in [
        (".claude/policies/runtime/py_boundary_guard.sh", "PY_BOUNDARY_GUARD_BIN"),
        (".claude/rust/py-boundary-guard/inline_python_scan.sh", "INLINE_PYTHON_SCAN_BIN"),
        (".claude/rust/py-boundary-guard/crate_allow.sh", "CRATE_ALLOW_BIN"),
    ] {
        assert!(root().join(launcher).is_file(), "{launcher}");
        let out = launch(launcher, var, "/nonexistent/evs-contract-bin", &r.0, &["check"]);
        assert_eq!(code(&out), Some(2), "{launcher}");
        assert_eq!(text(&out.stdout), "", "{launcher}");
    }
}

const PY_ALLOW: &str = ".claude/policies/py_boundary_allowlist.tsv";

fn guard(r: &Repo, args: &[&str]) -> Output {
    launch(".claude/policies/runtime/py_boundary_guard.sh", "PY_BOUNDARY_GUARD_BIN", env!("CARGO_BIN_EXE_py_boundary_guard"), &r.0, args)
}

/// pre-commit: `check --repo <root> --staged-additions` — exit 0 when no staged `*.py` is outside the allowlist,
/// non-zero (1) with `VIOLATION <path>` on stdout when one is; the full `check` judges every tracked `*.py`.
#[test]
fn py_boundary_guard_check_exit_codes() {
    let r = repo("guard");
    write(&r.0, PY_ALLOW, "# c\nconfigs/check_reqs.py\tvLLM deps\n");
    write(&r.0, "configs/check_reqs.py", "x = 1\n");
    write(&r.0, "a.rs", "fn main() {}\n");
    git(&r.0, &["add", "-A"]);
    let root_arg = r.0.to_str().unwrap();
    let out = guard(&r, &["check", "--repo", root_arg, "--staged-additions"]);
    assert_eq!(code(&out), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    let out = guard(&r, &["check", "--repo", root_arg]);
    assert_eq!(code(&out), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));

    write(&r.0, ".claude/skills/x/tool.py", "x = 1\n");
    git(&r.0, &["add", "-A"]);
    for args in [&["check", "--repo", root_arg, "--staged-additions"][..], &["check", "--repo", root_arg]] {
        let out = guard(&r, args);
        assert_eq!(code(&out), Some(1), "{args:?}");
        assert!(text(&out.stdout).lines().any(|l| l == "VIOLATION .claude/skills/x/tool.py"), "{}", text(&out.stdout));
    }
    // usage and input errors are 2 (the hook treats every non-zero as a refusal)
    assert_eq!(code(&guard(&r, &["check", "--bogus"])), Some(2));
    assert_eq!(code(&guard(&r, &["frob"])), Some(2));
    std::fs::remove_file(r.0.join(PY_ALLOW)).unwrap();
    assert_eq!(code(&guard(&r, &["check", "--repo", root_arg])), Some(2), "an unreadable allowlist fails closed");
}

const INLINE_ALLOW: &str = ".claude/rust/py-boundary-guard/inline_python_allowlist.tsv";

fn scan(r: &Repo, args: &[&str]) -> Output {
    launch(".claude/rust/py-boundary-guard/inline_python_scan.sh", "INLINE_PYTHON_SCAN_BIN", env!("CARGO_BIN_EXE_inline_python_scan"), &r.0, args)
}

/// policy-distribution and the W gate: `check --repo .` (cwd = the repository) — exit 0 clean, 1 with
/// `VIOLATION <path>:<line>:` lines on stdout, 2 on a usage error.
#[test]
fn inline_python_scan_check_exit_codes() {
    let r = repo("scan");
    write(&r.0, INLINE_ALLOW, "# none\n");
    write(&r.0, "a/ok.sh", "echo hi\n");
    git(&r.0, &["add", "-A"]);
    let out = scan(&r, &["check", "--repo", "."]);
    assert_eq!(code(&out), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    assert!(!text(&out.stdout).contains("VIOLATION"));

    write(&r.0, "a/bad.sh", "v=$(python3 -c 'print(1)')\n");
    git(&r.0, &["add", "-A"]);
    let out = scan(&r, &["check", "--repo", "."]);
    assert_eq!(code(&out), Some(1));
    assert!(text(&out.stdout).lines().any(|l| l.starts_with("VIOLATION a/bad.sh:1:")), "{}", text(&out.stdout));
    assert_eq!(code(&scan(&r, &["check", "--bogus"])), Some(2));
}

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// A repository whose one tracked lock pins `syn 3.0.9`, listed, noticed and present in the repository's cargo home.
fn crate_repo(tag: &str, listed_version: &str) -> Repo {
    let r = repo(tag);
    write(
        &r.0,
        "a/Cargo.lock",
        &format!(
            "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"syn\"\nversion = \"3.0.9\"\nsource = \"{CRATES_IO}\"\nchecksum = \"00\"\n"
        ),
    );
    write(
        &r.0,
        ".claude/rust/py-boundary-guard/crate_allowlist.tsv",
        &format!("crate\tversion\tlicense\ttransitive\tscope\tuse\treason\nsyn\t{listed_version}\tMIT OR Apache-2.0\t3\truntime\tscan\tc\n"),
    );
    write(
        &r.0,
        ".claude/rust/py-boundary-guard/notices/NOTICE-crates.md",
        "# n\n\n| crate | version | license | used by |\n|---|---|---|---|\n| syn | 3.0.9 | MIT OR Apache-2.0 | t |\n",
    );
    write(
        &r.0,
        ".claude/runtime/rust/cargo/registry/src/index.crates.io-0000000000000000/syn-3.0.9/Cargo.toml",
        "[package]\nname = \"syn\"\nversion = \"3.0.9\"\nlicense = \"MIT OR Apache-2.0\"\n",
    );
    git(&r.0, &["add", "a/Cargo.lock"]);
    r
}

fn crate_allow(r: &Repo, args: &[&str]) -> Output {
    launch(".claude/rust/py-boundary-guard/crate_allow.sh", "CRATE_ALLOW_BIN", env!("CARGO_BIN_EXE_crate_allow"), &r.0, args)
}

/// deps-allow: `crate_allow.sh check` (cwd = the repository) — pass = exit 0 and `DEPS-ALLOW-PASS` as the last stdout
/// line; a finding = exit 1, `DEPS-VIOLATION <kind> <crate>@<version> …` on stdout and no marker; usage = exit 2.
#[test]
fn crate_allow_check_marker_and_findings() {
    let ok = crate_repo("allow-pass", "3.0.9");
    let out = crate_allow(&ok, &["check"]);
    assert_eq!(code(&out), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    assert_eq!(text(&out.stdout).lines().last(), Some("DEPS-ALLOW-PASS"));

    let bad = crate_repo("allow-fail", "3.0.8");
    let out = crate_allow(&bad, &["check"]);
    let so = text(&out.stdout);
    assert_eq!(code(&out), Some(1), "{so}");
    assert!(so.lines().any(|l| l.starts_with("DEPS-VIOLATION unlisted syn@3.0.9")), "{so}");
    assert!(!so.contains("DEPS-ALLOW-PASS"), "{so}");

    assert_eq!(code(&crate_allow(&ok, &["check", "--bogus"])), Some(2));
    assert_eq!(code(&crate_allow(&ok, &[])), Some(2));
}
