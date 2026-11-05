//! The frozen original command-line criteria of `resolve_topology`: 33 invocations (23 resolutions against a declared
//! local owner-output fixture, and 10 parser/help/usage cases including the complete `--self-test` transcript). Each
//! runs the real binary in a scratch tree; exit status, standard output, standard error (an uncaught exception
//! projected as the references define it: the text before the traceback and its last line) and the whole tree
//! afterwards are compared.
//!
//! The `--self-test` case reads the checkout of the run (`RESOLVE_TOPOLOGY_REPO_ROOT`, `_SCRIPTS_DIR`, `_RUNTIME_DIR`
//! name the actual sources; `NOT_RUN` when they are not set). The binary under test is the one Cargo built for this
//! package (`CARGO_BIN_EXE_...`); the self-test copies that very binary and the API binary Cargo built.

#[path = "resolve_topology_common.rs"]
mod common;

use common::*;
use std::process::{Command, Output};
use sweep_stop::pyvalue::{PyDict, PyValue};

const CLI: &str = env!("CARGO_BIN_EXE_resolve_topology");
const API: &str = env!("CARGO_BIN_EXE_resolve_topology_api");
const SOURCES: [&str; 3] = [
    "RESOLVE_TOPOLOGY_REPO_ROOT",
    "RESOLVE_TOPOLOGY_SCRIPTS_DIR",
    "RESOLVE_TOPOLOGY_RUNTIME_DIR",
];

fn run(scratch: &Scratch, argv: &[PyValue]) -> Output {
    Command::new(CLI)
        .args(argv.iter().map(|a| text(&fill(a, scratch))))
        .current_dir(&scratch.root)
        .env("RESOLVE_TOPOLOGY_PYTHON", python())
        .env("RESOLVE_TOPOLOGY_API_BIN", API)
        .env_remove("RESOLVE_TOPOLOGY_BIN")
        .env_remove("COLUMNS")
        .env_remove("LINES")
        .output()
        .expect("run")
}

fn rebased(scratch: &Scratch, bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace(&scratch.text(), "@ROOT@")
}

/// Whether the child's standard error is what the case recorded: exact text, or - after an uncaught exception - the
/// text before the traceback and its final line.
fn stderr_agrees(case: &PyDict, actual: &str) -> bool {
    if let Some(exact) = case.get("stderr") {
        return actual == text(exact);
    }
    let marker = "Traceback (most recent call last):\n";
    let Some(at) = actual.find(marker) else {
        return false;
    };
    actual[..at] == text(field(case, "stderr_pretrace"))
        && actual.trim_end().lines().last() == Some(text(field(case, "stderr_last_line")).as_str())
}

fn expected_stdout(case: &PyDict) -> String {
    match case.get("stdout_file") {
        Some(name) => std::fs::read_to_string(data(&text(name))).expect("transcript"),
        None => text(field(case, "stdout")),
    }
}

fn exit_code(case: &PyDict) -> i32 {
    match field(case, "exit_code") {
        PyValue::Int(code) => i32::try_from(code.clone()).expect("exit code"),
        other => panic!("exit code {}", other.repr()),
    }
}

fn require_sources() {
    for name in SOURCES {
        assert!(
            std::env::var_os(name).is_some_and(|value| std::path::Path::new(&value).exists()),
            "NOT_RUN: {name} must name the actual source of the self-test"
        );
    }
}

#[test]
fn every_invocation_matches_the_original_streams_status_and_tree() {
    require_sources();
    let cases = list(read_json("cli_cases.json"));
    assert_eq!(cases.len(), 33);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("cli");
        scratch.populate(field(case, "before"));
        let PyValue::List(argv) = field(case, "argv") else {
            panic!("argv is a list")
        };
        let output = run(&scratch, argv);
        let status_ok = output.status.code() == Some(exit_code(case));
        let stdout_ok = rebased(&scratch, &output.stdout) == expected_stdout(case);
        let stderr_ok = stderr_agrees(case, &rebased(&scratch, &output.stderr));
        let tree_ok = scratch.snapshot() == expected_tree(field(case, "after"));
        if !(status_ok && stdout_ok && stderr_ok && tree_ok) {
            failures.push(format!(
                "{id}: status={status_ok} stdout={stdout_ok} stderr={stderr_ok} tree={tree_ok}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "differing invocations:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_selftest_transcript_is_the_originals_and_nothing_goes_to_standard_error() {
    require_sources();
    let scratch = Scratch::new("selftest");
    let output = run(&scratch, &[PyValue::str("--self-test")]);
    assert_eq!(output.status.code(), Some(0));
    let expected = std::fs::read_to_string(data("selftest_original.stdout")).expect("transcript");
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The binary under test copied out of any installed layout: `current_exe()` of the copy has no `rust` ancestor, so the
/// self-test sources can only come from the environment. (`CARGO_BIN_EXE_...` may itself live in an installed
/// `.../adversarial-benchmark/rust/target/release`, where the layout supplies them.)
fn isolated_cli(scratch: &Scratch, folder: &str) -> std::path::PathBuf {
    let directory = scratch.root.join(folder);
    std::fs::create_dir_all(&directory).expect("folder");
    assert!(
        !directory
            .ancestors()
            .any(|ancestor| ancestor.file_name().is_some_and(|name| name == "rust")),
        "NOT_RUN: the scratch location must not lie inside a `rust` folder"
    );
    let copy = directory.join("resolve_topology");
    std::fs::copy(CLI, &copy).expect("copy keeps the executable mode");
    copy
}

#[test]
fn the_abbreviated_and_repeated_options_follow_argparse() {
    let scratch = Scratch::new("argparse");
    // `--self` is an abbreviation of `--self-test`. The copied binary is outside any installed layout and its
    // environment names no source, so the run ends with the configuration error of this port (what the original read
    // from its own file location), as an uncaught exception.
    let isolated = isolated_cli(&scratch, "isolated");
    let output = Command::new(&isolated)
        .arg("--self")
        .current_dir(&scratch.root)
        .env_remove("RESOLVE_TOPOLOGY_REPO_ROOT")
        .env_remove("RESOLVE_TOPOLOGY_SCRIPTS_DIR")
        .env_remove("RESOLVE_TOPOLOGY_RUNTIME_DIR")
        .env_remove("RESOLVE_TOPOLOGY_BIN")
        .env_remove("RESOLVE_TOPOLOGY_API_BIN")
        .output()
        .expect("run");
    let shown = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{shown}");
    assert!(
        shown.trim_end().lines().last().is_some_and(|line| line.starts_with("ResolveTopologyConfigurationError: the repository root is not configured; set RESOLVE_TOPOLOGY_REPO_ROOT")),
        "{shown}"
    );
    // the last of repeated `--repo` wins (store action)
    let twice = Command::new(CLI)
        .args(["--repo", "/nonexistent-one", "--repo", ""])
        .current_dir(&scratch.root)
        .output()
        .expect("run");
    assert_eq!(twice.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&twice.stderr),
        "[resolve_topology] ERROR --repo 가 필요하다\n"
    );
}

/// The other side of the same rule: inside the installed layout (`.../adversarial-benchmark/rust/target/release`) the
/// sources are found from the executable's own place with no environment at all. A deliberate fixture: a layout whose
/// checkout has no parity owner, so the run gets past the configuration and stops at the missing owner file.
#[test]
fn an_executable_in_the_installed_layout_finds_its_sources_without_environment() {
    let scratch = Scratch::new("layout");
    let skill = scratch
        .root
        .join("layout/.claude/skills/adversarial-benchmark");
    std::fs::create_dir_all(skill.join("scripts")).expect("scripts");
    let release = skill.join("rust/target/release");
    std::fs::create_dir_all(&release).expect("release");
    std::fs::copy(CLI, release.join("resolve_topology")).expect("copy");
    let output = Command::new(release.join("resolve_topology"))
        .arg("--self-test")
        .current_dir(&scratch.root)
        .env_remove("RESOLVE_TOPOLOGY_REPO_ROOT")
        .env_remove("RESOLVE_TOPOLOGY_SCRIPTS_DIR")
        .env_remove("RESOLVE_TOPOLOGY_RUNTIME_DIR")
        .env_remove("RESOLVE_TOPOLOGY_BIN")
        .env("RESOLVE_TOPOLOGY_PYTHON", python())
        .output()
        .expect("run");
    let shown = String::from_utf8_lossy(&output.stderr).into_owned();
    let last = shown
        .trim_end()
        .lines()
        .last()
        .unwrap_or_default()
        .to_string();
    assert_eq!(output.status.code(), Some(1), "{shown}");
    assert!(
        !last.contains("ResolveTopologyConfigurationError"),
        "{shown}"
    );
    let owner = format!(
        "{}/layout/.claude/policies/runtime/topology_parity.py",
        scratch.text()
    );
    assert!(
        last.starts_with("FileNotFoundError: [Errno 2] No such file or directory:")
            && last.contains(&owner),
        "{shown}"
    );
}
