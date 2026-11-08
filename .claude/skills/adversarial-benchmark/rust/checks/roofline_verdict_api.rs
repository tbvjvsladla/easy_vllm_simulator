//! The `verdict_rule` API (`rubric_candidate`, `build_verdict`, the `_validate_cli` facade) through the
//! `evaluate-jsonl` transport, against values the ORIGINAL produced.
//!
//! `api_root.jsonl` is the root's frozen set of 348 observations (unchanged copy); `api_extra.jsonl` holds 2 299 more
//! observations captured from the original for odd types, huge integers, non-finite numbers, authority and spec
//! provenance, physical-limit and balance gates, malformed option objects and message text with lone surrogates.
//! Each row is `{id, operation, ..., expected}`; the answer must equal `{id, ...expected}` including value types,
//! key order, NaN and exception type and message.

#[path = "common.rs"]
mod common;

use common::{load_jsonl, same};
use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

const VERDICT_RULE: &str = env!("CARGO_BIN_EXE_verdict_rule");

fn without_expected(row: &PyDict) -> PyValue {
    let mut copy = PyDict::new();
    for (key, value) in row.iter() {
        if !key.equals("expected") {
            copy.insert(key.clone(), value.clone());
        }
    }
    PyValue::Dict(copy)
}

fn expected_answer(row: &PyDict) -> PyValue {
    let mut answer = PyDict::new();
    answer.set("id", row.get("id").cloned().expect("id"));
    let Some(PyValue::Dict(expected)) = row.get("expected") else {
        panic!("expected object")
    };
    for (key, value) in expected.iter() {
        answer.insert(key.clone(), value.clone());
    }
    PyValue::Dict(answer)
}

fn check(file: &str, at_least: usize) {
    let rows = load_jsonl(file);
    assert!(
        rows.len() >= at_least,
        "{file}: case set shrank to {}",
        rows.len()
    );
    let payload: String = rows
        .iter()
        .map(|row| without_expected(row).dumps_ascii().expect("dump") + "\n")
        .collect();
    let mut child = Command::new(VERDICT_RULE)
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answers: Vec<PyValue> = String::from_utf8(output.stdout)
        .expect("ASCII transport")
        .lines()
        .map(|line| loads(line).expect("answer line is JSON"))
        .collect();
    assert_eq!(answers.len(), rows.len(), "one answer line per row");
    let failures: Vec<String> = rows
        .iter()
        .zip(&answers)
        .filter(|(row, answer)| !same(answer, &expected_answer(row)))
        .map(|(row, answer)| {
            format!(
                "{}: want {} got {}",
                row.get("id").map(PyValue::repr).unwrap_or_default(),
                expected_answer(row)
                    .repr()
                    .chars()
                    .take(160)
                    .collect::<String>(),
                answer.repr().chars().take(160).collect::<String>()
            )
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} rows differ:\n{}",
        failures.len(),
        rows.len(),
        failures[..failures.len().min(12)].join("\n")
    );
}

#[test]
fn the_roots_frozen_api_observations_hold() {
    check("roofline_verdict/api_root.jsonl", 348);
}

#[test]
fn extra_api_observations_hold() {
    check("roofline_verdict/api_extra.jsonl", 2299);
}

#[test]
fn unknown_operations_and_malformed_lines_are_reported_per_line() {
    let mut child = Command::new(VERDICT_RULE)
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"{\"id\": \"a\", \"operation\": \"nope\"}\nnot json\n\n{\"id\": \"b\", \"operation\": \"rubric_candidate\", \"raw\": 2}\n")
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    let lines: Vec<PyValue> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| loads(l).unwrap())
        .collect();
    assert_eq!(
        lines.len(),
        3,
        "blank lines are skipped, every other line is answered"
    );
    let kind = |value: &PyValue| match value {
        PyValue::Dict(map) => map.get("kind").map(PyValue::repr),
        _ => None,
    };
    assert_eq!(kind(&lines[0]).as_deref(), Some("'exception'"));
    assert_eq!(kind(&lines[1]).as_deref(), Some("'exception'"));
    assert_eq!(kind(&lines[2]).as_deref(), Some("'value'"));
}
