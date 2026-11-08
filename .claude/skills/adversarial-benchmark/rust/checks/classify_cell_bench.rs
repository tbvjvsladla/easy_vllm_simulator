//! Controls of the timing harness `classify_cell_bench` (the observer, not production): it must reject what is wrong
//! before and instead of timing, and time what is right.
//!
//! * a correct full `expected_json` (the original's value as `json.dumps(value, ensure_ascii=True,
//!   separators=(',', ':'))`, from the frozen original rows in `data/classify_cell/bench_expected.jsonl`) is accepted
//!   and one timing document is printed;
//! * a deliberately wrong `expected_json`, a malformed one (not a string), an unknown operation and a warm call that
//!   writes a diagnostic are each refused with a nonzero status and **no** timing JSON;
//! * a row without `expected_json` still has to return `Ok` without a diagnostic.
//!
//! The timed-call enforcement (an `Err` or a diagnostic in any timed call fails the run and prints no timings) is
//! checked by the harness's own unit tests, which can inject a call that fails only on its third invocation.

use std::path::PathBuf;
use std::process::{Command, Output};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("classify_cell_bench")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn bench(rows: &str, label: &str) -> Output {
    let dir = scratch(label);
    let path = dir.join("rows.jsonl");
    std::fs::write(&path, rows).expect("rows");
    Command::new(env!("CARGO_BIN_EXE_classify_cell_bench"))
        .arg(&path)
        .args(["2", "3"])
        .env_remove("CLASSIFY_CELL_SCRIPTS_DIR")
        .output()
        .expect("run")
}

fn expected_rows() -> Vec<String> {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("checks/data/classify_cell/bench_expected.jsonl"),
    )
    .expect("expected rows");
    let rows: Vec<String> = text.lines().map(str::to_string).collect();
    assert!(
        rows.len() >= 8,
        "the control corpus covers every pure operation"
    );
    rows
}

fn refused(output: &Output, what: &str) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "{what}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "{what}: a refused run prints no timing JSON"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("REFUSED"),
        "{what}"
    );
}

#[test]
fn full_expected_values_of_the_original_rows_are_accepted_and_timed() {
    let rows = expected_rows();
    let output = bench(&(rows.join("\n") + "\n"), "accepted");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let PyValue::Dict(timing) =
        loads(std::str::from_utf8(&output.stdout).expect("UTF-8")).expect("one JSON document")
    else {
        panic!("timing is not an object")
    };
    assert_eq!(timing.get("cases"), Some(&PyValue::int(rows.len())));
    assert!(matches!(timing.get("rounds_ns"), Some(PyValue::List(rounds)) if rounds.len() == 2));
}

#[test]
fn a_wrong_expected_value_is_refused_without_timings() {
    for (index, row) in expected_rows().iter().enumerate() {
        // Change one character of the expected value: any mismatch of the full value is a refusal.
        let PyValue::Dict(parsed) = loads(row).expect("row") else {
            panic!()
        };
        let Some(PyValue::Str(expected)) = parsed.get("expected_json") else {
            panic!()
        };
        let expected = expected.to_utf8().expect("ASCII");
        let wrong = expected.replacen('"', "'", 1).replacen("null", "nul", 1);
        let wrong = if wrong == expected {
            format!("{expected} ")
        } else {
            wrong
        };
        let mut tampered = parsed.clone();
        tampered.set("expected_json", PyValue::str(&wrong));
        let line = PyValue::Dict(tampered).dumps_ascii().expect("json");
        refused(
            &bench(&(line + "\n"), &format!("wrong{index}")),
            "wrong expected_json",
        );
    }
}

#[test]
fn a_malformed_expected_json_type_is_refused() {
    for malformed in ["5", "null", "true", "[]", "{}"] {
        let row = format!(
            r#"{{"id":"m","operation":"bench_mode_kind","arguments":{{"record":null}},"expected_json":{malformed}}}"#
        );
        refused(&bench(&(row + "\n"), "malformed"), malformed);
    }
}

#[test]
fn an_unknown_operation_or_a_failing_call_is_refused() {
    let unknown = r#"{"id":"u","operation":"no_such_operation","arguments":{}}"#;
    refused(
        &bench(&(unknown.to_string() + "\n"), "unknown"),
        "unknown operation",
    );
    let wrong_call = r#"{"id":"w","operation":"classify","arguments":{"serve_rc":0}}"#;
    refused(
        &bench(&(wrong_call.to_string() + "\n"), "wrongcall"),
        "missing arguments",
    );
}

#[test]
fn a_diagnostic_is_fatal_even_when_the_call_returns_ok() {
    // `_read_events` of a file that does not exist returns `[[], []]` and writes a warning on stderr.
    let row = r#"{"id":"d","operation":"_read_events","arguments":{"paths":["/nonexistent/classify-cell-bench-control"]}}"#;
    refused(
        &bench(&(row.to_string() + "\n"), "diagnostic"),
        "diagnostic",
    );
}

#[test]
fn a_row_without_expected_json_is_timed_when_it_is_clean() {
    let row = r#"{"id":"c","operation":"bench_mode_kind","arguments":{"record":null}}"#;
    let output = bench(&(row.to_string() + "\n"), "clean");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("rounds_ns"));
}
