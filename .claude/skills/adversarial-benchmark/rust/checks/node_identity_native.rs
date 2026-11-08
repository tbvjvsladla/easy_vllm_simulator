//! The native node-identity lookup behind `Owners::resolve_self_node`: the private helper `node_identity_resolve` (which
//! calls the `role-contract` resolver) as the caller runs it, and a replay of the saved observations of the original.
//!
//! The replay (`the_helper_reproduces_every_saved_observation_of_the_original`) reads the saved reference directory named
//! by `R42_IDENTITY_REFERENCE` (one directory per observed row with `execution.json`, `stdout.bin`, `stderr.bin`); it
//! is skipped, loudly, when the variable is not set. The saved bytes are the oracle: nothing here executes the original.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::classify_cell_owner::{Owners, CONFIGURATION_ERROR};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;

const HELPER: &str = env!("CARGO_BIN_EXE_node_identity_resolve");

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("node_identity_native")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
    std::fs::write(path, bytes).expect("write");
}

/// A scripts directory with an owner file present (its content is never read by the native path).
fn owners_layout(label: &str) -> (PathBuf, Owners) {
    let root = scratch(label);
    put(
        &root,
        ".claude/skills/adversarial-benchmark/scripts/.keep",
        b"",
    );
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh",
        b"# presence only: the native resolver does not execute this file\n",
    );
    let scripts = root.join(".claude/skills/adversarial-benchmark/scripts");
    let owners = Owners::new(Some(PyStr::new(scripts.to_str().expect("UTF-8"))))
        .with_identity_helper(PathBuf::from(HELPER));
    (root, owners)
}

fn resolve(owners: &Owners, repo: &Path) -> (Option<String>, String) {
    let repo = PyValue::str(repo.to_str().expect("UTF-8"));
    let (node, source) = owners.resolve_self_node(&repo).expect("resolve");
    (
        node.map(|n| n.to_utf8().expect("text")),
        source.to_utf8().expect("text"),
    )
}

#[test]
fn the_native_lookup_answers_like_the_owner_for_a_sub_a_main_and_a_failure() {
    let (root, owners) = owners_layout("answers");
    let repo = root.join("repo");
    put(
        &repo,
        "output/multi/manifest.yaml",
        b"self_role: sub   # issued\nnodes:\n  - role: main\n",
    );
    let (node, source) = resolve(&owners, &repo);
    assert_eq!(
        (node.as_deref(), source.as_str()),
        (
            Some("sub"),
            "node_identity.sh --resolve(self_role → 유일한 role: main)"
        )
    );

    let main_repo = root.join("main_repo");
    put(
        &main_repo,
        "output/multi/manifest.yaml",
        b"nodes:\n  - role: main\n  - role: sub\n",
    );
    assert_eq!(resolve(&owners, &main_repo).0.as_deref(), Some("main"));

    // A failure carries the first two stderr lines, stripped, exactly as the original's relay does.
    let bare = root.join("bare");
    std::fs::create_dir_all(&bare).expect("bare");
    let (node, source) = resolve(&owners, &bare);
    assert_eq!(node, None);
    assert_eq!(
        source,
        "node_identity.sh --resolve rc=1 — [node-identity] FAIL: node_id 를 해소하지 못했다 — hostname 으로 대체하지 않는다. / [node-identity]   찾은 곳: (a) {}/output/*/manifest.yaml 의 self_role: (서브 · terraforming 발급)  (b) 동 파일의 유일한 role: main (메인)"
            .replace("{}", bare.to_str().expect("UTF-8"))
    );

    let not_dir = root.join("a_file");
    put(&root, "a_file", b"x");
    let (node, source) = resolve(&owners, &not_dir);
    assert_eq!(node, None);
    assert_eq!(
        source,
        "node_identity.sh --resolve rc=1 — [node-identity] FAIL: repo 루트를 못 찾았다(node_id 해소 불가)."
    );
}

#[test]
fn a_falsy_repo_returns_before_the_resolver_and_needs_no_helper() {
    let root = scratch("falsy");
    put(
        &root,
        ".claude/skills/adversarial-benchmark/scripts/.keep",
        b"",
    );
    let scripts = root.join(".claude/skills/adversarial-benchmark/scripts");
    let owners = Owners::new(Some(PyStr::new(scripts.to_str().expect("UTF-8"))))
        .with_identity_helper(root.join("no_such_helper"));
    for repo in [PyValue::None, PyValue::str("")] {
        let (node, source) = owners.resolve_self_node(&repo).expect("resolve");
        assert_eq!(
            (node, source.to_utf8().expect("text").as_str()),
            (None, "repo 미지정")
        );
    }
}

#[test]
fn a_missing_helper_is_a_named_configuration_error_never_a_bash_or_python_fallback() {
    let root = scratch("no_helper");
    put(
        &root,
        ".claude/skills/adversarial-benchmark/scripts/.keep",
        b"",
    );
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh",
        b"#!/bin/bash\necho SHOULD_NOT_RUN\n",
    );
    let scripts = root.join(".claude/skills/adversarial-benchmark/scripts");
    let owners = Owners::new(Some(PyStr::new(scripts.to_str().expect("UTF-8"))))
        .with_identity_helper(root.join("no_such_helper"));
    let failure = owners
        .resolve_self_node(&PyValue::str(root.to_str().expect("UTF-8")))
        .expect_err("a named configuration error");
    assert_eq!(failure.class, CONFIGURATION_ERROR);
    let message = failure.message.to_utf8().expect("text");
    assert!(message.contains("node_identity_resolve"), "{message}");
}

fn run_helper(args: &[&std::ffi::OsStr], env_all: &str) -> (i32, Vec<u8>, Vec<u8>) {
    let out = Command::new(HELPER)
        .args(args)
        .env_clear()
        .env("LC_ALL", env_all)
        .stdin(Stdio::null())
        .output()
        .expect("helper");
    (
        out.status.code().expect("exit code"),
        out.stdout,
        out.stderr,
    )
}

#[test]
fn the_helper_interface_rejects_what_it_does_not_define() {
    use std::ffi::OsStr;
    for args in [
        vec![],
        vec![OsStr::new("--repo")],
        vec![OsStr::new("--repo"), OsStr::new("")],
        vec![
            OsStr::new("--resolve"),
            OsStr::new("--repo"),
            OsStr::new("/"),
        ],
        vec![
            OsStr::new("--repo"),
            OsStr::new("/"),
            OsStr::new("--node-id"),
            OsStr::new("x"),
        ],
    ] {
        let (code, out, err) = run_helper(&args, "C");
        assert_eq!(code, 2, "{args:?}");
        assert!(out.is_empty());
        assert!(String::from_utf8_lossy(&err).contains("usage: node_identity_resolve"));
    }
}

#[test]
fn the_helper_reproduces_every_saved_observation_of_the_original() {
    let Some(reference) = std::env::var_os("R42_IDENTITY_REFERENCE") else {
        eprintln!("SKIPPED: R42_IDENTITY_REFERENCE is not set (saved observations of the original not available)");
        return;
    };
    let reference = PathBuf::from(reference);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&reference)
        .expect("reference")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("execution.json").is_file() && path.join("stdout.bin").is_file())
        .collect();
    dirs.sort();
    let mut compared = 0;
    let mut failures = Vec::new();
    for dir in dirs {
        let execution =
            std::fs::read_to_string(dir.join("execution.json")).expect("execution.json");
        let value = sweep_stop::pyjson::loads(&execution).expect("JSON");
        let get = |key: &str| -> PyValue { dict_get(&value, key) };
        let exit_code = match get("exit_code") {
            PyValue::Int(code) => code.to_string(),
            _ => continue,
        };
        let argv = list_of_strings(&get("argv"));
        // ["/bin/bash", OWNER, "--resolve", "--repo", REPO]
        assert_eq!(&argv[2..4], ["--resolve", "--repo"], "{}", dir.display());
        let (repo, owner) = (&argv[4], &argv[1]);
        let cwd = match get("cwd") {
            PyValue::Str(text) => text.to_utf8().expect("cwd"),
            other => panic!("cwd {other:?}"),
        };
        let locale = match dict_get(&get("environment"), "LC_ALL") {
            PyValue::Str(text) => text.to_utf8().expect("LC_ALL"),
            other => panic!("LC_ALL {other:?}"),
        };
        let out = Command::new(HELPER)
            .args(["--repo", repo.as_str(), "--source-owner", owner.as_str()])
            .env_clear()
            .env("LC_ALL", &locale)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .output()
            .expect("helper");
        let expected_out = std::fs::read(dir.join("stdout.bin")).expect("stdout.bin");
        let expected_err = std::fs::read(dir.join("stderr.bin")).expect("stderr.bin");
        let ok = out.status.code().map(|c| c.to_string()) == Some(exit_code.clone())
            && out.stdout == expected_out
            && out.stderr == expected_err;
        compared += 1;
        if !ok {
            failures.push(format!(
                "{}: status {:?} (saved {exit_code}); stdout {:?} (saved {:?}); stderr {:?} (saved {:?})",
                dir.file_name().expect("name").to_string_lossy(),
                out.status.code(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&expected_out),
                String::from_utf8_lossy(&out.stderr),
                String::from_utf8_lossy(&expected_err),
            ));
        }
    }
    eprintln!("compared {compared} saved rows, {} differ", failures.len());
    assert!(compared > 0, "no saved rows found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn dict_get(value: &PyValue, key: &str) -> PyValue {
    match value {
        PyValue::Dict(dict) => dict
            .get(key)
            .cloned()
            .unwrap_or_else(|| panic!("missing key {key}")),
        other => panic!("not a dict: {other:?}"),
    }
}

fn list_of_strings(value: &PyValue) -> Vec<String> {
    match value {
        PyValue::List(items) => items
            .iter()
            .map(|item| match item {
                PyValue::Str(text) => text.to_utf8().expect("text"),
                other => panic!("not a string {other:?}"),
            })
            .collect(),
        other => panic!("not a list {other:?}"),
    }
}

/// The two new original observations of the H2 repair (`R42_TARGETED_REFERENCE`: directories with `execution.json`,
/// `stdout.bin`, `stderr.bin`). Skipped loudly when the variable is not set; the saved bytes are the oracle.
#[test]
fn the_helper_reproduces_the_two_targeted_observations() {
    let Some(reference) = std::env::var_os("R42_TARGETED_REFERENCE") else {
        eprintln!("SKIPPED: R42_TARGETED_REFERENCE is not set");
        return;
    };
    let mut compared = 0;
    for case in [
        "H01-later-selected-invalid-utf8--PU",
        "H02-late-nul-after-large-text--PU",
    ] {
        let dir = PathBuf::from(&reference).join(case);
        let execution =
            std::fs::read_to_string(dir.join("execution.json")).expect("execution.json");
        let value = sweep_stop::pyjson::loads(&execution).expect("JSON");
        let argv = list_of_strings(&dict_get(&value, "argv"));
        let cwd = match dict_get(&value, "cwd") {
            PyValue::Str(text) => text.to_utf8().expect("cwd"),
            other => panic!("cwd {other:?}"),
        };
        let locale = match dict_get(&dict_get(&value, "environment"), "LC_ALL") {
            PyValue::Str(text) => text.to_utf8().expect("LC_ALL"),
            other => panic!("LC_ALL {other:?}"),
        };
        let out = Command::new(HELPER)
            .args([
                "--repo",
                argv[4].as_str(),
                "--source-owner",
                argv[1].as_str(),
            ])
            .env_clear()
            .env("LC_ALL", &locale)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .output()
            .expect("helper");
        assert_eq!(out.status.code(), Some(0), "{case}");
        assert_eq!(
            out.stdout,
            std::fs::read(dir.join("stdout.bin")).expect("stdout.bin"),
            "{case} stdout"
        );
        assert_eq!(
            out.stderr,
            std::fs::read(dir.join("stderr.bin")).expect("stderr.bin"),
            "{case} stderr"
        );
        compared += 1;
    }
    assert_eq!(compared, 2);
}
