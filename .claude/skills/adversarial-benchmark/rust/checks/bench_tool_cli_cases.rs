//! The frozen original command-line criteria of `resolve_bench_tool`: 45 invocations (help, usage errors,
//! `--self-test`, the shipped default pin, matching / drifting / absent images, malformed records and odd engine
//! answers). Each runs the real binary in a scratch tree; exit status, stdout, stderr (an uncaught traceback projected
//! as the references define it) and the whole tree afterwards (the recorded engine calls included) are compared.
//!
//! The container engine is a script placed first on `PATH` of the child (a command and side-effect fixture only: it
//! answers from three environment variables, starts nothing, queries no image and uses no network). The binary
//! knows nothing of it. The two cases that use the shipped pin read it through `BENCH_TOOL_PIN_PATH`, which the
//! environment of the check run must name (`NOT_RUN` otherwise): the binary has no other default in this tree.

#[path = "bench_tool_common.rs"]
mod common;

use common::*;
use std::process::{Command, Output};
use sweep_stop::pyvalue::{PyDict, PyValue};

const PIN_ENV: &str = "BENCH_TOOL_PIN_PATH";

fn run(scratch: &Scratch, argv: &[PyValue], environment: &PyDict) -> Output {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut search = std::ffi::OsString::from(scratch.root.join(".fixture-bin"));
    search.push(":");
    search.push(path);
    let mut command = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool"));
    command
        .args(
            argv.iter()
                .map(|a| text(a).replace("@ROOT@", &scratch.text())),
        )
        .current_dir(&scratch.root)
        .env("PATH", search)
        .env_remove("COLUMNS")
        .env_remove("LINES");
    for (name, value) in environment.iter() {
        command.env(name.to_utf8().expect("name"), text(value));
    }
    command.output().expect("run")
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

fn require_shipped_pin() {
    assert!(
        std::env::var_os(PIN_ENV).is_some_and(|path| std::path::Path::new(&path).is_file()),
        "NOT_RUN: {PIN_ENV} must name the shipped pin file"
    );
}

#[test]
fn every_invocation_matches_the_original_streams_status_and_tree() {
    require_shipped_pin();
    let cases = list(read_json("cli_cases.json"));
    assert_eq!(cases.len(), 45);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("cli");
        scratch.populate(field(case, "before"));
        let PyValue::List(argv) = field(case, "argv") else {
            panic!("argv is a list")
        };
        let output = run(&scratch, argv, dict(field(case, "environment")));
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
fn the_self_test_prints_what_the_original_printed() {
    require_shipped_pin();
    let scratch = Scratch::new("selftest");
    let output = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool"))
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
fn without_a_docker_command_the_original_failure_is_raised_and_nothing_is_pulled() {
    let recorded = list(read_json("cli_cases.json"));
    let base = recorded
        .iter()
        .map(dict)
        .find(|candidate| text(field(candidate, "id")) == "005-matching-index")
        .expect("matching case");
    let scratch = Scratch::new("nodocker");
    scratch.populate(field(base, "before"));
    std::fs::remove_file(scratch.root.join(".fixture-bin/docker")).expect("remove the fixture");
    let PyValue::List(argv) = field(base, "argv") else {
        panic!("argv is a list")
    };
    let output = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool"))
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
    assert!(output.stdout.is_empty());
}

/// The only engine command the binary may run is `docker image inspect IMAGE --format {{json .RepoDigests}}`; the
/// fixture asserts it and records every call, so an absent image (status 3) must show exactly one inspect and no pull.
#[test]
fn an_absent_image_is_inspected_once_and_never_pulled() {
    let recorded = list(read_json("cli_cases.json"));
    for id in ["011-null", "015-inspect-absent", "021-plain-absent"] {
        let case = recorded
            .iter()
            .map(dict)
            .find(|candidate| text(field(candidate, "id")) == id)
            .expect("absent case");
        let scratch = Scratch::new("absent");
        scratch.populate(field(case, "before"));
        let PyValue::List(argv) = field(case, "argv") else {
            panic!("argv is a list")
        };
        let output = run(&scratch, argv, dict(field(case, "environment")));
        assert_eq!(output.status.code(), Some(3), "{id}");
        let calls =
            std::fs::read_to_string(scratch.root.join("docker_calls.jsonl")).expect("calls");
        assert_eq!(calls.lines().count(), 1, "{id}: {calls}");
        assert!(
            calls.contains("\"image\", \"inspect\"") && !calls.contains("pull"),
            "{id}: {calls}"
        );
    }
}

/// Both engine streams are decoded (strict UTF-8, as `text=True` does) before the exit status is looked at: an
/// undecodable stream is an uncaught `UnicodeDecodeError` whatever the status and whichever stream it is.
#[test]
fn an_undecodable_engine_stream_is_an_error_even_for_a_failed_command() {
    let recorded = list(read_json("cli_cases.json"));
    let base = recorded
        .iter()
        .map(dict)
        .find(|candidate| text(field(candidate, "id")) == "005-matching-index")
        .expect("matching case");
    let PyValue::List(argv) = field(base, "argv") else {
        panic!("argv is a list")
    };
    let scripts = [
        ("stdout-failed", "printf '\\377'; exit 1"),
        ("stdout-ok", "printf '\\377'; exit 0"),
        (
            "stderr-ok",
            "printf '[\"x@sha256:a\"]'; printf '\\377' >&2; exit 0",
        ),
    ];
    for (label, body) in scripts {
        let scratch = Scratch::new("undecodable");
        scratch.populate(field(base, "before"));
        let script = scratch.root.join(".fixture-bin/docker");
        std::fs::write(&script, format!("#!/bin/bash\n{body}\n")).expect("fixture");
        let output = run(&scratch, argv, dict(field(base, "environment")));
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(
            stderr.trim_end().lines().last(),
            Some("UnicodeDecodeError: 'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"),
            "{label}"
        );
        assert!(output.stdout.is_empty(), "{label}");
    }
}
