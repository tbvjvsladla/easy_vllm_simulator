//! The frozen original command-line criteria of `resolve_guidellm`: 36 invocations (help, usage errors, `--self-test`,
//! declared versions of every odd shape, a custom repository, repeated options, record merges and the record failures
//! that end in a traceback). Each runs the real binary in a scratch tree; exit status, stdout, stderr (an uncaught
//! traceback projected as the references define it) and the whole tree afterwards are compared. None of them reaches
//! the network: a declared version needs none.
//!
//! The self-test reads the shipped pin record through `GUIDELLM_RECORD_PATH`, which the environment of the check run
//! must name (`NOT_RUN` otherwise): the binary has no other default in this tree.
//!
//! The remaining tests characterise what the references do not record: which record path wins, the abbreviations of
//! argparse, and that an empty `--version` takes the latest-release branch (aimed at a closed loopback proxy port, so no
//! request leaves this machine).

#[path = "guidellm_release_common.rs"]
mod common;

use common::*;
use std::process::{Command, Output};
use sweep_stop::pyvalue::{PyDict, PyValue};

const RECORD_ENV: &str = "GUIDELLM_RECORD_PATH";
const BINARY: &str = env!("CARGO_BIN_EXE_resolve_guidellm");

fn plain_command(scratch: &Scratch) -> Command {
    let mut command = Command::new(BINARY);
    command
        .current_dir(&scratch.root)
        .env_remove("COLUMNS")
        .env_remove("LINES")
        .env_remove(RECORD_ENV);
    command
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

fn require_shipped_record() -> std::ffi::OsString {
    let path = std::env::var_os(RECORD_ENV).filter(|path| std::path::Path::new(path).is_file());
    path.unwrap_or_else(|| panic!("NOT_RUN: {RECORD_ENV} must name the shipped pin record"))
}

fn run_case(case: &PyDict, record: &std::ffi::OsString) -> (Scratch, Output) {
    let scratch = Scratch::new("cli");
    scratch.populate(field(case, "before"));
    let PyValue::List(argv) = field(case, "argv") else {
        panic!("argv is a list")
    };
    let mut command = plain_command(&scratch);
    command.args(
        argv.iter()
            .map(|a| text(a).replace("@ROOT@", &scratch.text())),
    );
    if text(field(case, "id")) == "002-selftest" {
        command.env(RECORD_ENV, record);
    }
    let output = command.output().expect("run");
    (scratch, output)
}

#[test]
fn every_invocation_matches_the_original_streams_status_and_tree() {
    let record = require_shipped_record();
    let cases = list(read_json("cli_cases.json"));
    assert_eq!(cases.len(), 36);
    let mut statuses = [0_usize; 3];
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let (scratch, output) = run_case(case, &record);
        let want = exit_code(case);
        let status_ok = output.status.code() == Some(want);
        let stdout_ok = rebased(&scratch, &output.stdout) == text(field(case, "stdout"));
        let stderr_ok = stderr_agrees(case, &rebased(&scratch, &output.stderr));
        let tree_ok = scratch.snapshot() == after_tree(case);
        statuses[usize::try_from(want).expect("small status")] += 1;
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
    assert_eq!(statuses, [20, 11, 5], "exits 0/1/2 as recorded");
}

#[test]
fn the_self_test_prints_what_the_original_printed() {
    let record = require_shipped_record();
    let scratch = Scratch::new("selftest");
    let output = plain_command(&scratch)
        .arg("--self-test")
        .env(RECORD_ENV, record)
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0));
    let expected = std::fs::read_to_string(data("selftest_original.stdout")).expect("golden");
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    assert!(output.stderr.is_empty());
}

fn pin_copy(scratch: &Scratch, name: &str) -> std::path::PathBuf {
    let path = scratch.root.join(name);
    std::fs::copy(require_shipped_record(), &path).expect("copy the record");
    path
}

#[test]
fn an_explicit_record_path_wins_over_the_environment_and_the_environment_over_the_crate_default() {
    let scratch = Scratch::new("record-path");
    let from_env = pin_copy(&scratch, "env.json");
    let explicit = pin_copy(&scratch, "explicit.json");
    let before = std::fs::read(&from_env).expect("env record");
    let output = plain_command(&scratch)
        .args(["--version", "9.9.9", "--record", "--record-path"])
        .arg(&explicit)
        .env(RECORD_ENV, &from_env)
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        std::fs::read(&from_env).expect("env record"),
        before,
        "the environment record is untouched"
    );
    assert!(
        String::from_utf8_lossy(&std::fs::read(&explicit).expect("explicit")).contains("9.9.9")
    );
    let output = plain_command(&scratch)
        .args(["--version", "9.9.8", "--record"])
        .env(RECORD_ENV, &from_env)
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&std::fs::read(&from_env).expect("env record")).contains("9.9.8")
    );
}

#[test]
fn an_empty_record_path_is_an_error_not_a_default() {
    let scratch = Scratch::new("empty-record-path");
    let environment_record = pin_copy(&scratch, "env.json");
    let output = plain_command(&scratch)
        .args(["--version", "9.9.9", "--record", "--record-path", ""])
        .env(RECORD_ENV, &environment_record)
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(
        stderr.trim_end().lines().last(),
        Some("FileNotFoundError: [Errno 2] No such file or directory: ''")
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn options_may_be_abbreviated_and_an_ambiguous_prefix_is_a_usage_error() {
    let scratch = Scratch::new("abbreviations");
    let output = plain_command(&scratch)
        .args(["--ver", "1.2.3", "--js"])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"version\": \"1.2.3\""));
    let output = plain_command(&scratch)
        .args(["--rec", "--version", "1"])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("resolve_guidellm.py: error: ambiguous option: --rec could match --record, --record-path"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn an_empty_version_asks_for_the_latest_release_and_never_falls_back() {
    // A proxy address nobody listens on: the lookup must fail locally and report it, with no remote request.
    let closed_port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("address").port()
    };
    let scratch = Scratch::new("latest-unreachable");
    let output = plain_command(&scratch)
        .arg("--version=")
        .env("https_proxy", format!("http://127.0.0.1:{closed_port}"))
        .env_remove("HTTPS_PROXY")
        .env_remove("no_proxy")
        .env_remove("NO_PROXY")
        .env_remove("SSL_CERT_FILE")
        .env_remove("SSL_CERT_DIR")
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.starts_with(
            "[resolve_guidellm] UNRESOLVED 최신 릴리즈를 조회하지 못했다(URLError: <urlopen error "
        ),
        "{stderr}"
    );
    assert!(
        stderr.ends_with(
            ").\n  → 조용히 옛 버전으로 떨어지지 않는다. 쓸 버전을 선언하라: --version <x.y.z>\n"
        ),
        "{stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "no version is ever printed without a resolution"
    );
}
