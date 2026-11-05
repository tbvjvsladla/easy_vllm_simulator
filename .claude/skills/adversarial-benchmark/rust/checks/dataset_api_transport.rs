//! The `dataset_prep_api` program itself (not only the in-process operation function): the frozen pure and file
//! cases sent as `evaluate-jsonl` rows through the real binary, and the shape of every kind of answer line.

#[path = "dataset_common.rs"]
mod common;

use common::*;
use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;

fn evaluate(rows: &[String]) -> Vec<PyValue> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dataset_prep_api"))
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut input = child.stdin.take().expect("stdin");
    for row in rows {
        writeln!(input, "{row}").expect("row");
    }
    drop(input);
    let output = child.wait_with_output().expect("output");
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8(output.stdout)
        .expect("ASCII")
        .lines()
        .map(|line| loads(line).expect("an answer line is JSON"))
        .collect()
}

#[test]
fn the_recorded_pure_calls_answer_the_same_through_the_binary() {
    let recorded = read_jsonl("pure_api.jsonl");
    let rows: Vec<String> = recorded
        .iter()
        .map(|row| {
            let mut request = sweep_stop::pyvalue::PyDict::new();
            request.set("id", field(row, "id").clone());
            request.set("operation", field(row, "operation").clone());
            request.set("arguments", field(row, "arguments").clone());
            PyValue::Dict(request).dumps_ascii().expect("row")
        })
        .collect();
    let answers = evaluate(&rows);
    assert_eq!(answers.len(), recorded.len());
    for (row, answer) in recorded.iter().zip(&answers) {
        let (answer, want) = (dict(answer), dict(field(row, "expected")));
        let id = text(field(row, "id"));
        assert_eq!(text(field(answer, "id")), id);
        assert_eq!(
            text(field(answer, "kind")),
            text(field(want, "kind")),
            "{id}"
        );
        if text(field(want, "kind")) == "value" {
            assert_eq!(
                key(field(answer, "value")),
                key(field(want, "value")),
                "{id}"
            );
        } else {
            assert_eq!(
                text(field(answer, "type")),
                text(field(want, "type")),
                "{id}"
            );
            assert_eq!(
                text(field(answer, "message")),
                text(field(want, "message")),
                "{id}"
            );
        }
        assert!(
            answer.get("stderr").is_none() && answer.get("stdout").is_none(),
            "{id}"
        );
    }
}

#[test]
fn a_refused_input_carries_its_standard_error_and_the_other_wire_forms_hold() {
    let scratch = Scratch::new("transport");
    std::fs::write(scratch.root.join("spec.json"), "{}").expect("spec");
    let root = scratch.text();
    let rows = vec![
        format!(r#"{{"id":"refused","operation":"_load_spec","arguments":{{"path":"{root}/spec.json"}}}}"#),
        r#"{"id":"json-text","operation":"data_arg","arguments":{"kind":"json_file","path":"/x"},"result_json":true}"#.to_string(),
        r#"{"id":"raw-arguments","operation":"target_rows","arguments_json":{"rows":"[[1024,\"a\"]]","input_len":"1024"}}"#.to_string(),
        r#"{"id":"unknown","operation":"nope","arguments":{}}"#.to_string(),
        r#"{"id":"stray","operation":"data_arg","arguments":{"kind":1,"path":2,"extra":3}}"#.to_string(),
        r#"{"id":"short","operation":"data_arg","arguments":{"kind":1}}"#.to_string(),
        r#"{"id":"constants","operation":"constants","arguments":{}}"#.to_string(),
        "[1]".to_string(),
    ];
    let answers = evaluate(&rows);
    let at = |index: usize| dict(&answers[index]);
    assert_eq!(text(field(at(0), "type")), "SystemExit");
    assert_eq!(text(field(at(0), "message")), "2");
    assert_eq!(
        text(field(at(0), "stderr")),
        format!("[dataset_prep] 데이터셋 해소값에 dataset_kind 가 없다: {root}/spec.json\n")
    );
    assert_eq!(text(field(at(1), "encoding")), "json-values-v1");
    assert_eq!(
        text(field(at(1), "value_json")),
        r#""{\"kind\":\"json_file\",\"path\":\"/x\",\"load_kwargs\":{\"split\":\"train\"}}""#
    );
    assert_eq!(key(field(at(2), "value")), r#"[1024, ["a"]]"#);
    assert_eq!(text(field(at(3), "message")), "unknown operation 'nope'");
    assert_eq!(
        text(field(at(4), "message")),
        "data_arg() got an unexpected keyword argument 'extra'"
    );
    assert_eq!(
        text(field(at(5), "message")),
        "data_arg() missing 1 required positional argument: 'path'"
    );
    assert_eq!(
        key(field(at(6), "value")),
        r#"{"PREP_NAME": "prepared.jsonl", "PREP_META": "prepared_meta.json", "PROMPT_COUNT_COL": "prompt_tokens_count", "OUTPUT_COUNT_COL": "output_tokens_count"}"#
    );
    assert_eq!(
        text(field(at(7), "message")),
        "operation row is not an object"
    );
}

/// Rows recorded from the original with a non-finite `input_len` and counts beyond the scan limit
/// (`numeric_wire_cases.jsonl`: id, operation, arguments, and the original's answer under `expected`).
///
/// The wire JSON decoder of the real `dataset_prep_api` binary decodes `NaN` as the decoder's one shared object, a
/// different value kind from the plain `Float(NaN)` the check helpers read; calling the operation function on
/// helper-decoded rows would never reach it. This check therefore sends the rows - only `id`, `operation` and
/// `arguments`, the `NaN`/`Infinity` tokens kept - through the subprocess and compares the whole answers. It guards the
/// corrected defect where a shared NaN (and a count table past the scan limit, which switches to a hash lookup) was
/// handed to the arithmetic unpeeled.
#[test]
fn non_finite_input_lengths_and_many_distinct_counts_answer_through_the_wire_like_the_original() {
    let recorded = read_jsonl("numeric_wire_cases.jsonl");
    assert_eq!(recorded.len(), 9);
    let rows: Vec<String> = recorded
        .iter()
        .map(|row| {
            let mut request = sweep_stop::pyvalue::PyDict::new();
            request.set("id", field(row, "id").clone());
            request.set("operation", field(row, "operation").clone());
            request.set("arguments", field(row, "arguments").clone());
            PyValue::Dict(request).dumps_ascii().expect("row")
        })
        .collect();
    for token in ["NaN", "Infinity", "-Infinity"] {
        assert!(
            rows.iter().any(|row| row.contains(&format!(": {token}"))),
            "the wire input keeps the {token} token"
        );
    }
    assert!(rows.iter().all(|row| !row.contains("expected")));
    let answers = evaluate(&rows);
    assert_eq!(answers.len(), recorded.len());
    for (row, answer) in recorded.iter().zip(&answers) {
        let (answer, want) = (dict(answer), dict(field(row, "expected")));
        let id = text(field(row, "id"));
        assert_eq!(text(field(answer, "id")), id);
        assert_eq!(
            text(field(answer, "kind")),
            text(field(want, "kind")),
            "{id}"
        );
        if text(field(want, "kind")) == "value" {
            assert_eq!(
                key(field(answer, "value")),
                key(field(want, "value")),
                "{id}"
            );
        } else {
            assert_eq!(
                text(field(answer, "type")),
                text(field(want, "type")),
                "{id}"
            );
            assert_eq!(
                text(field(answer, "message")),
                text(field(want, "message")),
                "{id}"
            );
        }
    }
}
