//! The `resolve_bench_tool_api` program itself (not only the in-process operation function): the frozen pure and
//! file cases sent as `evaluate-jsonl` rows through the real binary, and the shape of every kind of answer line.

#[path = "bench_tool_common.rs"]
mod common;

use common::*;
use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn evaluate(rows: &[String]) -> Vec<PyValue> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool_api"))
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

fn request(row: &PyDict, arguments: &PyValue) -> String {
    let mut request = PyDict::new();
    request.set("id", field(row, "id").clone());
    request.set("operation", field(row, "operation").clone());
    request.set("arguments", arguments.clone());
    PyValue::Dict(request).dumps_ascii().expect("row")
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

#[test]
fn the_recorded_pure_calls_answer_the_same_through_the_binary() {
    let recorded = read_jsonl("pure_api.jsonl");
    let rows: Vec<String> = recorded
        .iter()
        .map(|row| request(row, field(row, "arguments")))
        .collect();
    let answers = evaluate(&rows);
    assert_eq!(answers.len(), recorded.len());
    for (row, answer) in recorded.iter().zip(&answers) {
        let id = text(field(row, "id"));
        same_answer(answer, dict(field(row, "expected")), &id);
        let answer = dict(answer);
        assert!(
            answer.get("stderr").is_none() && answer.get("stdout").is_none(),
            "{id}"
        );
    }
}

#[test]
fn the_recorded_file_cases_answer_the_same_through_the_binary() {
    for case in list(read_json("io_cases.json")) {
        let case = dict(&case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("api-io");
        scratch.populate(field(case, "before"));
        let arguments = rebase(field(case, "arguments"), "@ROOT@", &scratch.text());
        let answers = evaluate(&[request(case, &arguments)]);
        let want = rebase(field(case, "expected"), "@ROOT@", &scratch.text());
        same_answer(&answers[0], dict(&want), &id);
        assert_eq!(scratch.snapshot(), after_tree(case), "{id}");
    }
}

#[test]
fn every_kind_of_answer_line_has_its_shape() {
    let rows = [
        r#"{"id":1,"operation":"classify","arguments":{"entry":{},"repo_digests":null}}"#,
        r#"{"id":2,"operation":"classify","arguments":{"entry":{}}}"#,
        r#"{"id":3,"operation":"nope","arguments":{}}"#,
        r#"{"id":4,"operation":"main","arguments":{"argv":["--unknown-x"]}}"#,
        r#"{"id":5,"operation":"classify","arguments":{"entry":{},"repo_digests":null},"result_json":true}"#,
        r#"{"id":6,"operation":"constants","arguments":{}}"#,
    ]
    .map(String::from);
    let answers = evaluate(&rows);
    assert_eq!(answers.len(), 6);
    assert_eq!(key(field(dict(&answers[0]), "value")), "[3, null, null]");
    let second = dict(&answers[1]);
    assert_eq!(text(field(second, "type")), "TypeError");
    assert_eq!(
        text(field(second, "message")),
        "classify() missing 1 required positional argument: 'repo_digests'"
    );
    assert_eq!(text(field(dict(&answers[2]), "type")), "ValueError");
    let usage = dict(&answers[3]);
    assert_eq!(text(field(usage, "type")), "SystemExit");
    assert_eq!(text(field(usage, "message")), "2");
    assert!(text(field(usage, "stderr")).ends_with("unrecognized arguments: --unknown-x\n"));
    assert_eq!(
        text(field(dict(&answers[4]), "value_json")),
        "[3, null, null]"
    );
    let constants = dict(field(dict(&answers[5]), "value"));
    assert_eq!(text(field(constants, "DEFAULT_TOOL")), "guidellm");
    assert_eq!(key(field(constants, "RC_ABSENT")), "3");
}

#[test]
fn the_native_commands_and_a_wrong_call() {
    let help = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool_api"))
        .arg("native-help")
        .output()
        .expect("run");
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).contains("evaluate-jsonl"));
    let constants = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool_api"))
        .arg("constants")
        .output()
        .expect("run");
    assert_eq!(constants.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&constants.stdout).contains("\"RC_OK\": 0"));
    let wrong = Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool_api"))
        .arg("nonsense")
        .output()
        .expect("run");
    assert_eq!(wrong.status.code(), Some(2));
}
