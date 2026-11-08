//! The timing harness `resolve_topology_bench`: it times only rows whose untimed warm call returns the whole expected
//! value, and a failure anywhere ends it with status 1 and no timing line. The rows here are files of a scratch tree
//! (no process owner), so the harness itself - not the resolver - is under test.

#[path = "resolve_topology_common.rs"]
mod common;

use common::*;
use std::process::Command;
use sweep_stop::pyvalue::PyValue;

fn bench(scratch: &Scratch, rows: &str, extra: &[&str]) -> std::process::Output {
    let path = scratch.root.join("rows.jsonl");
    std::fs::write(&path, rows).expect("rows");
    Command::new(env!("CARGO_BIN_EXE_resolve_topology_bench"))
        .arg(&path)
        .args(["3", "5"])
        .args(extra)
        .current_dir(&scratch.root)
        .output()
        .expect("run")
}

fn good(scratch: &Scratch) -> String {
    std::fs::create_dir_all(scratch.root.join("repo/output/single")).expect("tree");
    std::fs::write(
        scratch.root.join("repo/output/single/manifest.yaml"),
        "topology: single\n",
    )
    .expect("manifest");
    let repo = format!("{}/repo", scratch.text());
    let expected = format!("[\"{repo}/output/single/manifest.yaml\"]");
    let mut row = sweep_stop::pyvalue::PyDict::new();
    row.set("id", PyValue::str("manifests"));
    row.set("operation", PyValue::str("_manifests"));
    let mut arguments = sweep_stop::pyvalue::PyDict::new();
    arguments.set("repo", PyValue::str(&repo));
    row.set("arguments", PyValue::Dict(arguments));
    row.set("expected_json", PyValue::str(&expected));
    PyValue::Dict(row).dumps_ascii().expect("row")
}

#[test]
fn a_row_with_the_whole_expected_value_is_timed() {
    let scratch = Scratch::new("bench-good");
    let output = bench(&scratch, &format!("{}\n", good(&scratch)), &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = String::from_utf8(output.stdout).expect("text");
    assert!(
        line.starts_with("{\"cases\": 1, \"iterations\": 5, \"rounds_ns\": ["),
        "{line}"
    );
}

#[test]
fn a_wrong_expected_value_or_a_failing_row_refuses_every_timing() {
    let scratch = Scratch::new("bench-refuse");
    let good = good(&scratch);
    let wrong = good.replace("manifest.yaml", "other.yaml");
    let failing = r#"{"id":"raises","operation":"_manifests","arguments":{"repo":5}}"#;
    let unknown = r#"{"id":"unknown","operation":"main","arguments":{}}"#;
    for (label, rows) in [
        ("wrong", wrong.as_str()),
        ("failing", failing),
        ("unknown", unknown),
    ] {
        let output = bench(&scratch, &format!("{good}\n{rows}\n"), &[]);
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
    let scratch = Scratch::new("bench-usage");
    let good = good(&scratch);
    let malformed = good.replace(
        "\"expected_json\": \"",
        "\"expected_json\": 5, \"unused\": \"",
    );
    assert_eq!(
        bench(&scratch, &format!("{malformed}\n"), &[])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        bench(&scratch, &format!("{good}\n"), &["absent"])
            .status
            .code(),
        Some(1)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_resolve_topology_bench"))
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
}
