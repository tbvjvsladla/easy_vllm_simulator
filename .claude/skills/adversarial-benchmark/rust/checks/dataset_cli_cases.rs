//! The frozen original command-line criteria of `dataset_prep`: 83 invocations (help, usage errors, `--self-test`,
//! `synthetic-sidecar`, `slice` with its odd inputs, `prepare` against a fixture Docker command) and the two process
//! witnesses of the output order. Each runs the real binary in a scratch tree; exit status, stdout, stderr (an
//! uncaught traceback projected as the references define it) and the whole tree afterwards are compared.
//!
//! The Docker command is a script placed first on `PATH` of the child (a subprocess command and side-effect
//! fixture only: it starts no container and uses no network). The binary knows nothing of it.

#[path = "dataset_common.rs"]
mod common;

use common::*;
use std::process::{Command, Output};
use sweep_stop::pyvalue::{PyDict, PyValue};

fn run(scratch: &Scratch, argv: &[PyValue], mode: &str) -> Output {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut search = std::ffi::OsString::from(scratch.root.join(".fixture-bin"));
    search.push(":");
    search.push(path);
    Command::new(env!("CARGO_BIN_EXE_dataset_prep"))
        .args(
            argv.iter()
                .map(|a| text(a).replace("@ROOT@", &scratch.text())),
        )
        .current_dir(&scratch.root)
        .env("PATH", search)
        .env("DATASET_FIXTURE_DOCKER_MODE", mode)
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

fn exit_code(case: &PyDict) -> i32 {
    match field(case, "exit_code") {
        PyValue::Int(code) => i32::try_from(code.clone()).expect("exit code"),
        other => panic!("exit code {}", other.repr()),
    }
}

#[test]
fn every_invocation_matches_the_original_streams_status_and_tree() {
    let cases = list(read_json("cli_cases.json"));
    assert_eq!(cases.len(), 83);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("cli");
        scratch.populate(field(case, "before"));
        let PyValue::List(argv) = field(case, "argv") else {
            panic!("argv is a list")
        };
        let output = run(&scratch, argv, &text(field(case, "mode")));
        let status_ok = output.status.code() == Some(exit_code(case));
        let stdout_ok = rebased(&scratch, &output.stdout) == text(field(case, "stdout"));
        let stderr_ok = stderr_agrees(case, &rebased(&scratch, &output.stderr));
        let tree_ok = scratch.snapshot() == after_tree(case);
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
fn a_child_process_writes_to_the_shared_output_before_the_buffered_banner() {
    let recorded = list(read_json("cli_cases.json"));
    let process = list(read_json("process_cases.json"));
    assert_eq!(process.len(), 2);
    for case in &process {
        let case = dict(case);
        let id = text(field(case, "id"));
        let base = recorded
            .iter()
            .map(dict)
            .find(|candidate| text(field(candidate, "id")) == text(field(case, "from")))
            .expect("the case the witnesses start from");
        let scratch = Scratch::new("process");
        scratch.populate(field(base, "before"));
        let script = base64_decode(&text(field(case, "fixture_docker_base64")));
        std::fs::write(scratch.root.join(".fixture-bin/docker"), script).expect("fixture script");
        let PyValue::List(argv) = field(case, "argv") else {
            panic!("argv is a list")
        };
        let output = run(&scratch, argv, &text(field(case, "mode")));
        assert_eq!(output.status.code(), Some(exit_code(case)), "{id}");
        assert_eq!(
            rebased(&scratch, &output.stdout),
            text(field(case, "stdout")),
            "{id} stdout"
        );
        assert_eq!(
            rebased(&scratch, &output.stderr),
            text(field(case, "stderr")),
            "{id} stderr"
        );
    }
}

#[test]
fn the_self_test_prints_what_the_original_printed() {
    let scratch = Scratch::new("selftest");
    let output = Command::new(env!("CARGO_BIN_EXE_dataset_prep"))
        .arg("--self-test")
        .current_dir(&scratch.root)
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0));
    let expected = std::fs::read_to_string(data("selftest_original.stdout")).expect("golden");
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    assert!(output.stderr.is_empty());
}

#[test]
fn without_a_docker_command_the_original_failure_is_raised_not_replaced() {
    let recorded = list(read_json("cli_cases.json"));
    let base = recorded
        .iter()
        .map(dict)
        .find(|candidate| text(field(candidate, "id")) == "066-prepare-fresh")
        .expect("fresh prepare");
    let scratch = Scratch::new("nodocker");
    scratch.populate(field(base, "before"));
    std::fs::remove_file(scratch.root.join(".fixture-bin/docker")).expect("remove the fixture");
    let PyValue::List(argv) = field(base, "argv") else {
        panic!("argv is a list")
    };
    let output = Command::new(env!("CARGO_BIN_EXE_dataset_prep"))
        .args(
            argv.iter()
                .map(|a| text(a).replace("@ROOT@", &scratch.text())),
        )
        .current_dir(&scratch.root)
        .env("PATH", scratch.root.join("empty"))
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.starts_with("Traceback (most recent call last):\n"),
        "{stderr}"
    );
    assert_eq!(
        stderr.trim_end().lines().last(),
        Some("FileNotFoundError: [Errno 2] No such file or directory: 'docker'")
    );
}
