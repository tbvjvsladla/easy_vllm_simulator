//! Contract checks of the py-boundary-guard bins (plan 26111805 「보존 계약」, cell c21): what their consumers read,
//! through the launchers they call — consumers.tsv rows of `py_boundary_guard` (pre-commit hook), `inline_python_scan`
//! (policy-distribution `check --repo .`, W gate) and `crate_allow` (campaign check `deps-allow`). The bins run for
//! real against throwaway git repositories; the checks fix exit codes, marker and finding line prefixes, the
//! stdout/stderr split and the launcher override variables, not message texts. `crate_allow` also: the harness-package
//! rule for source-less packages (`path-unknown`), and exit 2 for no tracked lock and for a non-UTF-8 argument
//! (contract consumers.md 「계약 시험에 반영할 것」, c20 3회차).
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
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("pybg-contract-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    Repo(dir)
}

fn git(dir: &Path, args: &[&str]) {
    let st = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?}");
}

fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

/// `bash <launcher> <args>` with `<VAR>=<bin>` (the consumers' override), cwd `cwd`.
fn launch(launcher: &str, var: &str, bin: &str, cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(root().join(launcher))
        .args(args)
        .env(var, bin)
        .current_dir(cwd)
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn code(o: &Output) -> Option<i32> {
    o.status.code()
}

const LAUNCHERS: [(&str, &str); 3] = [
    (
        ".claude/policies/runtime/py_boundary_guard.sh",
        "PY_BOUNDARY_GUARD_BIN",
    ),
    (
        ".claude/rust/py-boundary-guard/inline_python_scan.sh",
        "INLINE_PYTHON_SCAN_BIN",
    ),
    (
        ".claude/rust/py-boundary-guard/crate_allow.sh",
        "CRATE_ALLOW_BIN",
    ),
];

/// Every launcher refuses an absent executable with exit 2 and starts nothing (the hook fails closed on it). The probe is
/// `--help`, which every real bin answers with exit 0, so the 2 comes from the absent override, not from the bin.
#[test]
fn launchers_exist_and_an_absent_executable_is_exit_2() {
    let r = repo("absent");
    for (launcher, var) in LAUNCHERS {
        assert!(root().join(launcher).is_file(), "{launcher}");
        let out = launch(
            launcher,
            var,
            "/nonexistent/evs-contract-bin",
            &r.0,
            &["--help"],
        );
        assert_eq!(code(&out), Some(2), "{launcher}");
        assert_eq!(text(&out.stdout), "", "{launcher}");
    }
}

/// The `env` ELF of this host, found on `PATH` (the launchers start only an ELF): a stand-in whose output shows which
/// executable a launcher started.
fn env_elf() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .map(|d| d.join("env"))
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .find(|p| {
            std::fs::read(p)
                .map(|b| b.starts_with(b"\x7fELF"))
                .unwrap_or(false)
        })
        .expect("an `env` ELF on PATH")
}

/// The override variable names the executable the launcher starts, with the arguments passed on unchanged: a symlink to
/// the `env` ELF as `<VAR>` runs `printf` and its words reach stdout (no real bin of the crate prints them).
#[test]
fn launcher_override_runs_the_named_executable() {
    let r = repo("override");
    let stand_in = r.0.join("stand-in-elf");
    std::os::unix::fs::symlink(env_elf(), &stand_in).unwrap();
    for (launcher, var) in LAUNCHERS {
        let out = launch(
            launcher,
            var,
            stand_in.to_str().unwrap(),
            &r.0,
            &["printf", "%s|%s", var, "--repo"],
        );
        assert_eq!(code(&out), Some(0), "{launcher}: {}", text(&out.stderr));
        assert_eq!(text(&out.stdout), format!("{var}|--repo"), "{launcher}");
    }
}

const PY_ALLOW: &str = ".claude/policies/py_boundary_allowlist.tsv";

fn guard(r: &Repo, args: &[&str]) -> Output {
    launch(
        ".claude/policies/runtime/py_boundary_guard.sh",
        "PY_BOUNDARY_GUARD_BIN",
        env!("CARGO_BIN_EXE_py_boundary_guard"),
        &r.0,
        args,
    )
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
    assert_eq!(
        code(&out),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let out = guard(&r, &["check", "--repo", root_arg]);
    assert_eq!(
        code(&out),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );

    write(&r.0, ".claude/skills/x/tool.py", "x = 1\n");
    git(&r.0, &["add", "-A"]);
    for args in [
        &["check", "--repo", root_arg, "--staged-additions"][..],
        &["check", "--repo", root_arg],
    ] {
        let out = guard(&r, args);
        assert_eq!(code(&out), Some(1), "{args:?}");
        assert!(
            text(&out.stdout)
                .lines()
                .any(|l| l == "VIOLATION .claude/skills/x/tool.py"),
            "{}",
            text(&out.stdout)
        );
    }
    // usage and input errors are 2 (the hook treats every non-zero as a refusal)
    assert_eq!(code(&guard(&r, &["check", "--bogus"])), Some(2));
    assert_eq!(code(&guard(&r, &["frob"])), Some(2));
    std::fs::remove_file(r.0.join(PY_ALLOW)).unwrap();
    assert_eq!(
        code(&guard(&r, &["check", "--repo", root_arg])),
        Some(2),
        "an unreadable allowlist fails closed"
    );
}

const INLINE_ALLOW: &str = ".claude/rust/py-boundary-guard/inline_python_allowlist.tsv";

fn scan(r: &Repo, args: &[&str]) -> Output {
    launch(
        ".claude/rust/py-boundary-guard/inline_python_scan.sh",
        "INLINE_PYTHON_SCAN_BIN",
        env!("CARGO_BIN_EXE_inline_python_scan"),
        &r.0,
        args,
    )
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
    assert_eq!(
        code(&out),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert!(!text(&out.stdout).contains("VIOLATION"));

    write(&r.0, "a/bad.sh", "v=$(python3 -c 'print(1)')\n");
    git(&r.0, &["add", "-A"]);
    let out = scan(&r, &["check", "--repo", "."]);
    assert_eq!(code(&out), Some(1));
    assert!(
        text(&out.stdout)
            .lines()
            .any(|l| l.starts_with("VIOLATION a/bad.sh:1:")),
        "{}",
        text(&out.stdout)
    );
    assert_eq!(code(&scan(&r, &["check", "--bogus"])), Some(2));
}

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// A repository whose one tracked lock (beside its tracked manifest, package `a 0.1.0`) pins `syn 3.0.9`, listed,
/// noticed and present in the repository's cargo home.
fn crate_repo(tag: &str, listed_version: &str) -> Repo {
    let r = repo(tag);
    write(
        &r.0,
        "a/Cargo.toml",
        "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
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
    git(&r.0, &["add", "a/Cargo.lock", "a/Cargo.toml"]);
    r
}

fn crate_allow(r: &Repo, args: &[&str]) -> Output {
    launch(
        ".claude/rust/py-boundary-guard/crate_allow.sh",
        "CRATE_ALLOW_BIN",
        env!("CARGO_BIN_EXE_crate_allow"),
        &r.0,
        args,
    )
}

/// deps-allow: `crate_allow.sh check` (cwd = the repository) — pass = exit 0 and `DEPS-ALLOW-PASS` as the last stdout
/// line; a finding = exit 1, `DEPS-VIOLATION <kind> <crate>@<version> …` on stdout and no marker; usage = exit 2.
#[test]
fn crate_allow_check_marker_and_findings() {
    let ok = crate_repo("allow-pass", "3.0.9");
    let out = crate_allow(&ok, &["check"]);
    assert_eq!(
        code(&out),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert_eq!(text(&out.stdout).lines().last(), Some("DEPS-ALLOW-PASS"));

    let bad = crate_repo("allow-fail", "3.0.8");
    let out = crate_allow(&bad, &["check"]);
    let so = text(&out.stdout);
    assert_eq!(code(&out), Some(1), "{so}");
    assert!(
        so.lines()
            .any(|l| l.starts_with("DEPS-VIOLATION unlisted syn@3.0.9")),
        "{so}"
    );
    assert!(!so.contains("DEPS-ALLOW-PASS"), "{so}");

    assert_eq!(code(&crate_allow(&ok, &["check", "--bogus"])), Some(2));
    assert_eq!(code(&crate_allow(&ok, &[])), Some(2));
}

/// A source-less (path) package passes only as a harness package — the `[package]` name and version of a tracked
/// `Cargo.toml` beside a tracked `Cargo.lock`; any other is `DEPS-VIOLATION path-unknown <crate>@<version>`, exit 1:
/// a package no manifest declares, one whose manifest is not tracked, one whose tracked manifest has no tracked lock
/// beside it.
#[test]
fn crate_allow_path_package_needs_a_tracked_manifest_beside_its_lock() {
    let lock_with_b = format!(
        "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"b\"\nversion = \"0.2.0\"\n\n\
         [[package]]\nname = \"syn\"\nversion = \"3.0.9\"\nsource = \"{CRATES_IO}\"\nchecksum = \"00\"\n"
    );
    let unknown = |r: &Repo, case: &str| {
        let out = crate_allow(r, &["check"]);
        let so = text(&out.stdout);
        assert_eq!(code(&out), Some(1), "{case}: {so}{}", text(&out.stderr));
        assert!(
            so.lines()
                .any(|l| l.starts_with("DEPS-VIOLATION path-unknown b@0.2.0")),
            "{case}: {so}"
        );
        assert!(
            !so.lines().any(|l| l.contains("a@0.1.0")),
            "{case}: the harness package a passes: {so}"
        );
        assert!(!so.contains("DEPS-ALLOW-PASS"), "{case}: {so}");
    };

    let none = crate_repo("path-none", "3.0.9");
    write(&none.0, "a/Cargo.lock", &lock_with_b);
    unknown(&none, "no manifest declares b");

    let untracked = crate_repo("path-untracked", "3.0.9");
    write(&untracked.0, "a/Cargo.lock", &lock_with_b);
    write(
        &untracked.0,
        "b/Cargo.toml",
        "[package]\nname = \"b\"\nversion = \"0.2.0\"\n",
    );
    write(
        &untracked.0,
        "b/Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"b\"\nversion = \"0.2.0\"\n",
    );
    unknown(&untracked, "b's manifest and lock are not tracked");

    let lockless = crate_repo("path-lockless", "3.0.9");
    write(&lockless.0, "a/Cargo.lock", &lock_with_b);
    write(
        &lockless.0,
        "b/Cargo.toml",
        "[package]\nname = \"b\"\nversion = \"0.2.0\"\n",
    );
    git(&lockless.0, &["add", "b/Cargo.toml"]);
    unknown(
        &lockless,
        "b's tracked manifest has no tracked lock beside it",
    );

    // the same package with its own tracked manifest and lock is a harness package: pass
    write(
        &lockless.0,
        "b/Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"b\"\nversion = \"0.2.0\"\n",
    );
    git(&lockless.0, &["add", "b/Cargo.lock"]);
    let out = crate_allow(&lockless, &["check"]);
    assert_eq!(
        code(&out),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert_eq!(text(&out.stdout).lines().last(), Some("DEPS-ALLOW-PASS"));
}

/// Input errors are exit 2 with no marker and no finding line: a repository without any tracked `Cargo.lock` (nothing
/// to check is not a pass) and an argument that is not UTF-8.
#[test]
fn crate_allow_no_tracked_lock_and_non_utf8_argument_are_exit_2() {
    let r = crate_repo("no-lock", "3.0.9");
    git(&r.0, &["rm", "-q", "--cached", "a/Cargo.lock"]);
    let out = crate_allow(&r, &["check"]);
    assert_eq!(
        code(&out),
        Some(2),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert!(
        !text(&out.stdout).contains("DEPS-"),
        "{}",
        text(&out.stdout)
    );
    assert!(!out.stderr.is_empty());

    // the argument names a readable copy of the allowlist: only its non-UTF-8 name makes it an input error
    use std::os::unix::ffi::OsStrExt;
    let ok = crate_repo("non-utf8", "3.0.9");
    let mut name = ok.0.clone().into_os_string();
    name.push(std::ffi::OsStr::from_bytes(b"/allow-\xff\xfe.tsv"));
    std::fs::copy(
        ok.0.join(".claude/rust/py-boundary-guard/crate_allowlist.tsv"),
        &name,
    )
    .unwrap();
    let run = |allowlist: &std::ffi::OsStr| {
        Command::new("bash")
            .arg(root().join(".claude/rust/py-boundary-guard/crate_allow.sh"))
            .arg("check")
            .arg("--allowlist")
            .arg(allowlist)
            .env("CRATE_ALLOW_BIN", env!("CARGO_BIN_EXE_crate_allow"))
            .current_dir(&ok.0)
            .output()
            .unwrap()
    };
    let utf8 = ok.0.join("allow-copy.tsv");
    std::fs::copy(&name, &utf8).unwrap();
    assert_eq!(
        code(&run(utf8.as_os_str())),
        Some(0),
        "the same table under a UTF-8 name passes"
    );
    let out = run(&name);
    assert_eq!(
        code(&out),
        Some(2),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert_eq!(text(&out.stdout), "");
    assert!(!out.stderr.is_empty());
}
