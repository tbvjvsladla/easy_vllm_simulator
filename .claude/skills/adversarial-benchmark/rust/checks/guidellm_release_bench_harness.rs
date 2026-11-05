//! The timing harness `resolve_guidellm_bench`: it times only rows whose untimed warm call returns the whole expected
//! value without a diagnostic, and a failure anywhere ends it with status 1 and no timing line.

#[path = "guidellm_release_common.rs"]
mod common;

use common::*;
use std::process::Command;

fn bench(rows: &str, extra: &[&str]) -> std::process::Output {
    let scratch = Scratch::new("bench");
    let path = scratch.root.join("rows.jsonl");
    std::fs::write(&path, rows).expect("rows");
    Command::new(env!("CARGO_BIN_EXE_resolve_guidellm_bench"))
        .arg(&path)
        .args(["3", "5"])
        .args(extra)
        .output()
        .expect("run")
}

const GOOD: &str = r#"{"id":"image","operation":"image_ref","arguments":{"version":"0.7.3","repo":"vllm-project/guidellm"},"expected_json":"\"ghcr.io/vllm-project/guidellm:v0.7.3\""}"#;

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
    let wrong = GOOD.replace("v0.7.3", "v0.7.4");
    let failing = r#"{"id":"raises","operation":"version_from_tag","arguments":{"tag":"latest"}}"#;
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
fn the_bench_never_reaches_the_network_and_a_merge_row_is_timed_on_a_real_file() {
    // A row that would need the latest release is refused (RuntimeError), not answered from the network.
    let latest = r#"{"id":"latest","operation":"fetch_latest","arguments":{}}"#;
    assert_eq!(bench(&format!("{latest}\n"), &[]).status.code(), Some(1));
    // A merge row works on a real file: it is whole read + rewrite of the file it names.
    let scratch = Scratch::new("bench-merge");
    let record = scratch.root.join("record.json");
    std::fs::write(&record, "{}").expect("record");
    let row = format!(
        "{{\"id\":\"merge\",\"operation\":\"merge_record\",\"arguments\":{{\"path\":{},\"res\":{{\"version\":\"1.0\",\"image_ref\":\"i\",\"version_source\":\"s\"}}}}}}",
        sweep_stop::pyvalue::PyValue::str(&scratch.root.join("record.json").to_string_lossy())
            .dumps_ascii()
            .expect("path")
    );
    let output = bench(&format!("{row}\n"), &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read_to_string(&record)
        .expect("record")
        .contains("\"default_version\": \"1.0\""));
}

#[test]
fn a_malformed_expected_json_and_a_missing_row_are_refused_and_usage_errors_are_status_2() {
    let malformed = GOOD.replace(
        r#""expected_json":"\"ghcr.io/vllm-project/guidellm:v0.7.3\"""#,
        r#""expected_json":5"#,
    );
    assert_eq!(bench(&format!("{malformed}\n"), &[]).status.code(), Some(1));
    assert_eq!(
        bench(&format!("{GOOD}\n"), &["absent"]).status.code(),
        Some(1)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_resolve_guidellm_bench"))
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
}
