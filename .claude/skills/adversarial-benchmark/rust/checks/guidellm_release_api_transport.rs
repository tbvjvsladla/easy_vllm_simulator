//! The `resolve_guidellm_api` program itself (not only the in-process operation function): the frozen pure rows and the
//! fixture-driven fetch rows sent as `evaluate-jsonl` through the real binary, the shape of every kind of answer line,
//! and the guarantee that without a fixture the API never reaches the network.

#[path = "guidellm_release_common.rs"]
mod common;

use common::*;
use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn evaluate(rows: &[String]) -> Vec<PyValue> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resolve_guidellm_api"))
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

fn same_answer(answer: &PyValue, want: &PyDict, id: &str) {
    let answer = dict(answer);
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

fn row_text(row: &PyDict, keys: &[&str]) -> String {
    let mut out = PyDict::new();
    for name in keys {
        if let Some(value) = row.get(name) {
            out.set(name, value.clone());
        }
    }
    PyValue::Dict(out).dumps_ascii().expect("row")
}

#[test]
fn frozen_pure_rows_through_the_binary_agree_with_the_original() {
    let rows = read_jsonl("pure_api.jsonl");
    let lines: Vec<String> = rows
        .iter()
        .map(|r| row_text(r, &["id", "operation", "arguments"]))
        .collect();
    let answers = evaluate(&lines);
    assert_eq!(answers.len(), 48);
    for (row, answer) in rows.iter().zip(&answers) {
        same_answer(
            answer,
            dict(field(row, "expected")),
            &text(field(row, "id")),
        );
    }
}

#[test]
fn frozen_fetch_rows_through_the_binary_agree_and_carry_the_request() {
    let rows = list(read_json("fetch_cases.json"));
    let rows: Vec<&PyDict> = rows.iter().map(dict).collect::<Vec<_>>();
    let lines: Vec<String> = rows
        .iter()
        .map(|r| row_text(r, &["id", "operation", "arguments", "fetch_fixture"]))
        .collect();
    let answers = evaluate(&lines);
    assert_eq!(answers.len(), 30);
    for (row, answer) in rows.iter().zip(&answers) {
        same_answer(
            answer,
            dict(field(row, "expected")),
            &text(field(row, "id")),
        );
        assert_eq!(
            key(field(dict(answer), "request")),
            key(field(row, "expected_request")),
            "{}",
            text(field(row, "id"))
        );
    }
}

#[test]
fn without_a_fixture_the_api_refuses_the_network_and_other_shapes_are_reported() {
    let answers = evaluate(&[
        r#"{"id":"a","operation":"fetch_latest","arguments":{}}"#.to_string(),
        r#"{"id":"b","operation":"main","arguments":{"argv":[]}}"#.to_string(),
        r#"{"id":"c","operation":"fetch_latest","arguments":{"opener":"callable"}}"#.to_string(),
        r#"{"id":"d","operation":"main","arguments":{"argv":["--version=1.2.3","--json"]},"result_json":true}"#.to_string(),
        r#"{"id":"e","operation":"no_such","arguments":{}}"#.to_string(),
        r#"{"id":"f","operation":"image_ref","arguments":{"version":"1","oops":2}}"#.to_string(),
        r#"{"id":"g","operation":"main","arguments":{"argv":[]},"fetch_fixture":{"response_hex":"7b7d"}}"#.to_string(),
        r#"{"id":"h","operation":"main","arguments":{"argv":[]},"fetch_fixture":{"failure":{"type":"URLError","message":"<urlopen error x>"}}}"#.to_string(),
        r#"{"id":"i","operation":"main","arguments":{"argv":[]},"fetch_fixture":{"response_hex":"7b2274616700"}}"#.to_string(),
    ]);
    let kind = |index: usize| text(field(dict(&answers[index]), "type"));
    assert_eq!(kind(0), "RuntimeError");
    assert_eq!(kind(1), "RuntimeError");
    assert_eq!(kind(2), "TypeError");
    assert_eq!(kind(4), "ValueError");
    assert_eq!(kind(5), "TypeError");
    // the declared version needs no network, and its streams are reported
    let declared = dict(&answers[3]);
    assert_eq!(text(field(declared, "kind")), "value");
    assert!(text(field(declared, "value_json")).starts_with("0"));
    assert!(text(field(declared, "stdout")).contains("\"version\": \"1.2.3\""));
    // main over a fixture: tag missing -> ValueError text, exit 2; opener failure -> UNRESOLVED, exit 3; bad body -> 2
    let exit_of = |index: usize| key(field(dict(&answers[index]), "value"));
    assert_eq!(exit_of(6), "2");
    assert!(text(field(dict(&answers[6]), "stderr"))
        .starts_with("[resolve_guidellm] ERROR 릴리즈 응답에 tag_name 이 없다"));
    assert_eq!(exit_of(7), "3");
    assert!(text(field(dict(&answers[7]), "stderr")).contains("(URLError: <urlopen error x>)"));
    assert_eq!(exit_of(8), "2");
}

#[test]
fn constants_and_help_are_answered() {
    let output = Command::new(env!("CARGO_BIN_EXE_resolve_guidellm_api"))
        .arg("constants")
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0));
    let constants = loads(String::from_utf8(output.stdout).expect("ascii").trim()).expect("json");
    assert_eq!(
        text(field(dict(&constants), "REPO_DEFAULT")),
        "vllm-project/guidellm"
    );
    assert_eq!(
        text(field(dict(&constants), "RECORD_PATH_ENV")),
        "GUIDELLM_RECORD_PATH"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_resolve_guidellm_api"))
        .arg("bogus")
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
}
