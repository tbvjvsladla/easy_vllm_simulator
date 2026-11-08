//! The timing harness `resolve_bench_tool_bench`: it times only rows whose untimed warm call returns the whole expected value
//! without a diagnostic, and a failure anywhere ends it with status 1 and no timing line.

#[path = "bench_tool_common.rs"]
mod common;

use common::*;
use std::process::Command;

fn bench(rows: &str, extra: &[&str]) -> std::process::Output {
    let scratch = Scratch::new("bench");
    let path = scratch.root.join("rows.jsonl");
    std::fs::write(&path, rows).expect("rows");
    Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool_bench"))
        .arg(&path)
        .args(["3", "5"])
        .args(extra)
        .output()
        .expect("run")
}

const GOOD: &str = r#"{"id":"mode","operation":"classify","arguments":{"entry":{"index_digest":"sha256:a"},"repo_digests":["x@sha256:b","x@sha256:a"]},"expected_json":"[0,\"sha256:a\",true]"}"#;

#[test]
fn a_row_with_the_whole_expected_value_is_timed() {
    let output = bench(&format!("{GOOD}\n"), &[]);
    assert_eq!(output.status.code(), Some(0));
    let line = String::from_utf8(output.stdout).expect("text");
    assert!(
        line.starts_with("{\"cases\": 1, \"iterations\": 5, \"rounds_ns\": ["),
        "{line}"
    );
}

#[test]
fn a_wrong_expected_value_a_failing_row_or_a_diagnostic_refuses_every_timing() {
    let wrong = GOOD.replace(r#"[0,\"sha256:a\",true]"#, r#"[0,\"sha256:a\",false]"#);
    let failing =
        r#"{"id":"raises","operation":"classify","arguments":{"entry":null,"repo_digests":[]}}"#;
    let diagnostic = r#"{"id":"dies","operation":"main","arguments":{"argv":["--bogus"]}}"#;
    for (label, rows) in [
        ("wrong", wrong.as_str()),
        ("failing", failing),
        ("diagnostic", diagnostic),
    ] {
        let output = bench(&format!("{GOOD}\n{rows}\n"), &[]);
        assert_eq!(output.status.code(), Some(1), "{label}");
        assert!(
            output.stdout.is_empty(),
            "{label}: no timing may be printed"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("REFUSED"),
            "{label}"
        );
    }
}

#[test]
fn a_malformed_expected_json_and_a_missing_row_are_refused_and_usage_errors_are_status_2() {
    let malformed = GOOD.replace(
        r#""expected_json":"[0,\"sha256:a\",true]""#,
        r#""expected_json":5"#,
    );
    assert_eq!(bench(&format!("{malformed}\n"), &[]).status.code(), Some(1));
    assert_eq!(
        bench(&format!("{GOOD}\n"), &["absent"]).status.code(),
        Some(1)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool_bench"))
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
}
