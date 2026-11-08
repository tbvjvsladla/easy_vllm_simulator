//! Negative controls of the native `--self-test`: it executes the callers, the launchers, the copied native programs and
//! the real parity owner, so a defect in any of them must fail it (a transcript printer would pass regardless).
//!
//! Every scenario edits only a private copy of the sources in a scratch tree (the actual sources are read, never
//! changed) and runs this package's binary with the sources named by the environment variables of the self-test.
//! The actual sources come from the environment of the run (`NOT_RUN` when it does not name them).

#[path = "resolve_topology_common.rs"]
mod common;

use common::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CLI: &str = env!("CARGO_BIN_EXE_resolve_topology");
const API: &str = env!("CARGO_BIN_EXE_resolve_topology_api");

fn source(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .unwrap_or_else(|| panic!("NOT_RUN: {name} must name the actual source of the self-test"))
}

/// Private copies: a scripts directory (every file of the actual one) the scenarios may edit.
fn copy_scripts(scratch: &Scratch) -> PathBuf {
    let from = source("RESOLVE_TOPOLOGY_SCRIPTS_DIR");
    let to = scratch.root.join("scripts");
    std::fs::create_dir_all(&to).expect("scripts");
    for entry in std::fs::read_dir(from).expect("scripts dir") {
        let entry = entry.expect("entry");
        if entry.path().is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).expect("copy");
        }
    }
    to
}

fn selftest(scratch: &Scratch, scripts: &Path, repo: &Path, extra: &[(&str, &str)]) -> Output {
    let mut command = Command::new(CLI);
    command
        .arg("--self-test")
        .current_dir(&scratch.root)
        .env("RESOLVE_TOPOLOGY_REPO_ROOT", repo)
        .env("RESOLVE_TOPOLOGY_SCRIPTS_DIR", scripts)
        .env(
            "RESOLVE_TOPOLOGY_RUNTIME_DIR",
            source("RESOLVE_TOPOLOGY_RUNTIME_DIR"),
        )
        .env("RESOLVE_TOPOLOGY_API_BIN", API)
        .env("RESOLVE_TOPOLOGY_PYTHON", python())
        .env_remove("RESOLVE_TOPOLOGY_BIN");
    for (name, value) in extra {
        command.env(name, value);
    }
    command.output().expect("run")
}

fn failed_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains("[FAIL]"))
        .map(String::from)
        .collect()
}

#[test]
fn the_unmodified_sources_pass_even_when_the_run_substitutes_binaries() {
    let scratch = Scratch::new("controls-ok");
    let scripts = source("RESOLVE_TOPOLOGY_SCRIPTS_DIR");
    // overrides a test run could carry: the copied probes must still run their own deployed files
    let output = selftest(
        &scratch,
        &scripts,
        &source("RESOLVE_TOPOLOGY_REPO_ROOT"),
        &[
            ("RESOLVE_TOPOLOGY_API_BIN_UNUSED", "/bin/false"),
            ("REPEAT_AXIS_BIN", "/bin/false"),
            ("REPEAT_AXIS_API_BIN", "/bin/false"),
            ("BENCH_PRESET_BIN", "/bin/false"),
            ("BENCH_PRESET_API_BIN", "/bin/false"),
            ("REPEAT_AXIS_REPO_ROOT", "/nonexistent-repeat-root"),
            ("BENCH_PRESET_REPO_ROOT", "/nonexistent-preset-root"),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(failed_lines(&output).is_empty());
    // and the bin override of the resolver itself, set in the environment of the self-test run, is not what the copy executes
    let output = selftest(
        &scratch,
        &scripts,
        &source("RESOLVE_TOPOLOGY_REPO_ROOT"),
        &[("RESOLVE_TOPOLOGY_BIN", "/bin/false")],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_caller_that_infers_the_topology_from_a_branch_name_fails_the_self_test() {
    let scratch = Scratch::new("controls-branch");
    let scripts = copy_scripts(&scratch);
    let path = scripts.join("judge_bench.sh");
    let mut body = std::fs::read_to_string(&path).expect("caller");
    body.push_str("\n# BR=\"$(git symbolic-ref --short HEAD)\"\n");
    std::fs::write(&path, body).expect("edit");
    let output = selftest(
        &scratch,
        &scripts,
        &source("RESOLVE_TOPOLOGY_REPO_ROOT"),
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    let failures = failed_lines(&output);
    assert!(
        failures
            .iter()
            .any(|line| line.contains("S0 judge_bench.sh")),
        "{failures:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("[resolve_topology] --self-test FAIL"),
        "{stderr}"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("--self-test OK"));
}

#[test]
fn a_caller_that_no_longer_calls_the_resolver_fails_the_self_test() {
    let scratch = Scratch::new("controls-call");
    let scripts = copy_scripts(&scratch);
    let path = scripts.join("run_bench.sh");
    let body = std::fs::read_to_string(&path).expect("caller");
    assert!(
        body.contains("resolve_topology.sh"),
        "the actual caller names the launcher"
    );
    // the stale Python spelling: the S0 text check sees no launcher, and the copied tree no longer resolves through it
    std::fs::write(
        &path,
        body.replace("resolve_topology.sh", "resolve_topology.py"),
    )
    .expect("edit");
    let output = selftest(
        &scratch,
        &scripts,
        &source("RESOLVE_TOPOLOGY_REPO_ROOT"),
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    let failures = failed_lines(&output);
    assert!(
        failures.iter().any(|line| line.contains("S0 run_bench.sh")),
        "{failures:?}"
    );
}

/// A checkout whose parity owner says RED for everything, over the actual configuration and tools (symbolic links).
#[test]
fn a_parity_owner_that_judges_differently_fails_the_self_test() {
    let scratch = Scratch::new("controls-owner");
    let real = source("RESOLVE_TOPOLOGY_REPO_ROOT");
    let repo = scratch.root.join("repo");
    let skill = ".claude/skills/adversarial-benchmark";
    std::fs::create_dir_all(repo.join(".claude/skills")).expect("skills");
    std::os::unix::fs::symlink(real.join(skill), repo.join(skill)).expect("link");
    let parity = repo.join(".claude/policies/runtime/topology_parity.py");
    std::fs::create_dir_all(parity.parent().unwrap()).expect("policies");
    std::fs::write(
        &parity,
        "import json, sys\nprint(json.dumps({'verdict': 'FAIL', 'topology': 'multi', 'reasons': ['control']}))\nsys.exit(5)\n",
    )
    .expect("owner");
    let output = selftest(
        &scratch,
        &source("RESOLVE_TOPOLOGY_SCRIPTS_DIR"),
        &repo,
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    let failures = failed_lines(&output);
    assert!(
        failures
            .iter()
            .any(|line| line.contains("P1 메인 multi-node")),
        "{failures:?}"
    );
    assert!(
        failures.iter().any(|line| line.contains("S1 judge_bench")),
        "{failures:?}"
    );
    let shown = String::from_utf8_lossy(&output.stdout);
    assert!(
        shown.contains("4자일치 RED — control"),
        "the failed check names the owner's reason"
    );
}

#[test]
fn an_unconfigured_source_ends_the_run_with_a_configuration_error_not_a_shorter_run() {
    let scratch = Scratch::new("controls-config");
    let output = Command::new(CLI)
        .arg("--self-test")
        .current_dir(&scratch.root)
        .env(
            "RESOLVE_TOPOLOGY_REPO_ROOT",
            source("RESOLVE_TOPOLOGY_REPO_ROOT"),
        )
        .env(
            "RESOLVE_TOPOLOGY_SCRIPTS_DIR",
            source("RESOLVE_TOPOLOGY_SCRIPTS_DIR"),
        )
        .env(
            "RESOLVE_TOPOLOGY_RUNTIME_DIR",
            scratch.root.join("absent-runtime"),
        )
        .env("RESOLVE_TOPOLOGY_API_BIN", API)
        .env("RESOLVE_TOPOLOGY_PYTHON", python())
        .output()
        .expect("run");
    // R, P and S0 run; the copy of a program from the missing runtime directory is a FileNotFoundError
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        stderr.trim_end().lines().last().is_some_and(
            |line| line.starts_with("FileNotFoundError: [Errno 2] No such file or directory:")
        ),
        "{stderr}"
    );
}
