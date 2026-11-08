//! `conformance_full_superset_lite_bench` as a program: it times the whole production call (here the whole checker with
//! its real launchers and the emitter), verifies the warm call against the expected value and printed text, and
//! refuses to produce timings when anything is wrong.

#[path = "conformance_common.rs"]
mod common;

use common::*;
use std::process::{Command, Output};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;

const BENCH: &str = env!("CARGO_BIN_EXE_conformance_full_superset_lite_bench");

fn bench(rows: &str, arguments: &[&str], scripts: Option<&std::path::Path>) -> Output {
    let tree = Tree::new("bench-rows");
    tree.write("rows.jsonl", rows.as_bytes());
    let mut command = Command::new(BENCH);
    with_overrides(
        &mut command,
        env!("CARGO_BIN_EXE_bench_reporting_api"),
        env!("CARGO_BIN_EXE_publish_benchmark_record"),
        env!("CARGO_BIN_EXE_lite_metrics_api"),
    );
    command
        .arg(tree.path("rows.jsonl"))
        .args(arguments)
        .env_remove("CONFORMANCE_SCRIPTS_DIR");
    if let Some(dir) = scripts {
        command.env("CONFORMANCE_SCRIPTS_DIR", dir);
    }
    command.output().expect("run")
}

fn emit_row(id: &str, results: &str, expected_json: &str, expected_stdout: &str) -> String {
    let stdout = PyValue::str(expected_stdout).dumps_ascii().expect("JSON");
    let json = PyValue::str(expected_json).dumps_ascii().expect("JSON");
    format!(
        "{{\"id\": \"{id}\", \"operation\": \"_emit\", \"arguments\": {{\"results\": {results}}}, \"expected_json\": {json}, \"expected_stdout\": {stdout}}}\n"
    )
}

#[test]
fn timings_are_printed_after_a_verified_warm_call() {
    let rows = emit_row(
        "pass",
        r#"[["a", true, ""]]"#,
        "true",
        "PASS a\n\nfull ⊇ lite conformance: 1/1 PASS\n",
    ) + &emit_row(
        "fail",
        r#"[["a", false, "why"]]"#,
        "false",
        "FAIL a  — why\n\nfull ⊇ lite conformance: 0/1 FAIL\n",
    );
    let output = bench(&rows, &["3", "4"], None);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let PyValue::Dict(record) =
        loads(String::from_utf8(output.stdout).expect("UTF-8").trim()).expect("JSON")
    else {
        panic!("not an object");
    };
    assert_eq!(field(&record, "cases"), &PyValue::int(2));
    assert_eq!(field(&record, "iterations"), &PyValue::int(4));
    let PyValue::List(rounds) = field(&record, "rounds_ns") else {
        panic!("no rounds")
    };
    assert_eq!(rounds.len(), 3);
}

#[test]
fn the_whole_checker_is_timed_with_its_real_child_processes() {
    let tree = Tree::new("bench-installed");
    let scripts = install_copy(&tree);
    let expected = PyValue::str(&read_text("normal_stdout.txt"))
        .dumps_ascii()
        .expect("JSON");
    let rows = format!(
        "{{\"id\": \"normal\", \"operation\": \"main\", \"arguments\": {{}}, \"expected_json\": \"0\", \"expected_stdout\": {expected}}}\n"
    );
    let output = bench(&rows, &["2", "1"], Some(&scripts));
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let PyValue::Dict(record) =
        loads(String::from_utf8(output.stdout).expect("UTF-8").trim()).expect("JSON")
    else {
        panic!("not an object");
    };
    let PyValue::List(rounds) = field(&record, "rounds_ns") else {
        panic!("no rounds")
    };
    assert_eq!(rounds.len(), 2);
    // a whole-checker round starts real children (bash, three launchers, the binaries behind them): not instant
    let PyValue::Int(first) = &rounds[0] else {
        panic!("not an integer")
    };
    assert!(
        first > &num_bigint::BigInt::from(1_000_000),
        "a whole checker run took {first} ns"
    );
}

#[test]
fn a_wrong_expectation_refuses_the_run_without_timings() {
    for (rows, why) in [
        (
            emit_row(
                "a",
                r#"[["a", true, ""]]"#,
                "false",
                "PASS a\n\nfull ⊇ lite conformance: 1/1 PASS\n",
            ),
            "expected_json",
        ),
        (
            emit_row("a", r#"[["a", true, ""]]"#, "true", "other"),
            "expected_stdout",
        ),
    ] {
        let output = bench(&rows, &["2", "2"], None);
        assert_eq!(output.status.code(), Some(1), "{why}");
        assert!(output.stdout.is_empty(), "{why}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(why),
            "{why}"
        );
    }
}

#[test]
fn an_operation_that_raises_refuses_the_run() {
    let rows = "{\"id\": \"short\", \"operation\": \"_emit\", \"arguments\": {\"results\": [[\"a\", true]]}, \"expected_json\": \"true\", \"expected_stdout\": \"\"}\n";
    let output = bench(rows, &["1", "1"], None);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not enough values to unpack"));
}

#[test]
fn a_row_without_both_expectations_is_refused_before_any_call() {
    for rows in [
        "{\"id\": \"a\", \"operation\": \"_emit\", \"arguments\": {\"results\": []}, \"expected_json\": \"true\"}\n",
        "{\"id\": \"a\", \"operation\": \"_emit\", \"arguments\": {\"results\": []}, \"expected_stdout\": \"\"}\n",
    ] {
        let output = bench(rows, &["1", "1"], None);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("a row has no expected_"));
    }
}

#[test]
fn usage_errors_and_unknown_ids() {
    let output = Command::new(BENCH).output().expect("run");
    assert_eq!(output.status.code(), Some(2));
    let rows = emit_row("a", "[]", "true", "\nfull ⊇ lite conformance: 0/0 PASS\n");
    assert_eq!(bench(&rows, &["x", "1"], None).status.code(), Some(2));
    let unknown = bench(&rows, &["1", "1", "nope"], None);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("no row nope"));
    assert_eq!(bench(&rows, &["1", "1", "a"], None).status.code(), Some(0));
}
