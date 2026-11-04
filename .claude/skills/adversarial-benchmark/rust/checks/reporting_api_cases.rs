//! The frozen original-only API criteria of the five reporting and naming owners: 203 helper/builder rows and
//! 129 naming and lite-document rows, every value or exception (class and message) compared with what the
//! original produced.

#[path = "reporting_common.rs"]
mod common;

use common::*;
use sweep_stop::bench_reporting_api::evaluate_operation;
use sweep_stop::pyvalue::{PyDict, PyValue};
use sweep_stop::report_support::Diagnostics;

fn run_rows(name: &str, expected_rows: usize) {
    let rows = read_jsonl(name);
    assert_eq!(rows.len(), expected_rows, "{name}: row count");
    let mut failures = Vec::new();
    for row in &rows {
        let id = text(field(row, "id"));
        let operation = format!(
            "{}.{}",
            text(field(row, "owner")),
            text(field(row, "operation"))
        );
        let arguments = dict(field(row, "arguments")).clone();
        let want = dict(field(row, "expected"));
        let mut diagnostics = Diagnostics::default();
        let got = evaluate_operation(&operation, &arguments, &mut diagnostics);
        let agrees = match (got, text(field(want, "kind")).as_str()) {
            (Ok(value), "value") => key(&value) == key(field(want, "value")),
            (Err(failure), "exception") => {
                failure.class == text(field(want, "type"))
                    && failure.message.to_utf8() == Some(text(field(want, "message")))
            }
            _ => false,
        };
        if !agrees {
            failures.push(id);
        }
    }
    assert!(failures.is_empty(), "{name}: differing rows {failures:?}");
}

#[test]
fn pure_helpers_and_builders_match_the_original() {
    run_rows("api_pure.jsonl", 203);
}

#[test]
fn naming_and_lite_documents_match_the_original() {
    run_rows("api_extra.jsonl", 129);
}

#[test]
fn an_unknown_operation_and_wrong_calls_fail_as_python_calls_do() {
    let mut diagnostics = Diagnostics::default();
    let empty = PyDict::new();
    let unknown = evaluate_operation("doc_naming.nope", &empty, &mut diagnostics).unwrap_err();
    assert_eq!(unknown.class, "ValueError");
    let missing = evaluate_operation("doc_naming.gpu_key", &empty, &mut diagnostics).unwrap_err();
    assert_eq!(missing.class, "TypeError");
    assert_eq!(
        missing.message.to_utf8().unwrap(),
        "gpu_key() missing 1 required positional argument: 'gpu_model'"
    );
    let mut stray = PyDict::new();
    stray.set("gpu_model", PyValue::None);
    stray.set("extra", PyValue::None);
    let extra = evaluate_operation("doc_naming.gpu_key", &stray, &mut diagnostics).unwrap_err();
    assert_eq!(
        extra.message.to_utf8().unwrap(),
        "gpu_key() got an unexpected keyword argument 'extra'"
    );
}
