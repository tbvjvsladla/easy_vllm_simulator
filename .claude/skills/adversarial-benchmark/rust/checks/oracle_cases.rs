//! Replays the 465 externally captured inputs of the original program and compares verdicts and errors.
//!
//! Source of truth: `sweep-reference/{cases,expected}.json` (never modified here) and byte-exact stdout goldens
//! derived from the same expected values.

#[path = "common.rs"]
mod common;

use common::{dict, list, load_json_file, load_jsonl, oracle_dir, text_of};
use sweep_stop::jsonl::evaluate_lines;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};
use sweep_stop::{evaluate, parse_utc};

fn reference() -> (Vec<PyValue>, Vec<PyValue>) {
    let dir = oracle_dir();
    (
        list(load_json_file(&dir.join("cases.json"))),
        list(load_json_file(&dir.join("expected.json"))),
    )
}

/// The record the original's reference harness stores for one case.
fn library_outcome(case: &PyValue) -> PyValue {
    let case = dict(case);
    let state = case.get("state").expect("case has state");
    let now = case.get("now_utc").expect("case has now_utc");
    let mut record = PyDict::new();
    record.set("id", case.get("id").cloned().expect("case has id"));
    match parse_utc(now, "--now-utc").and_then(|now| evaluate(state, &now)) {
        Ok(evaluation) => {
            record.set("kind", PyValue::str("value"));
            record.set("value", evaluation.to_value());
            record.set("exit_code", PyValue::int(evaluation.exit_code()));
        }
        Err(error) => {
            record.set("kind", PyValue::str("exception"));
            record.set("type", PyValue::str(error.python_type()));
            record.set("message", PyValue::str(&error.message()));
        }
    }
    PyValue::Dict(record)
}

fn kind_counts(outcomes: &[PyValue]) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for outcome in outcomes {
        let outcome = dict(outcome);
        match (
            text_of(outcome.get("kind")).as_str(),
            outcome.get("type").map(|t| text_of(Some(t))),
        ) {
            ("value", _) => counts.0 += 1,
            (_, Some(kind)) if kind == "StopEvalError" => counts.1 += 1,
            (_, Some(kind)) if kind == "TypeError" => counts.2 += 1,
            other => panic!("unexpected outcome kind {other:?}"),
        }
    }
    counts
}

fn assert_all_equal(label: &str, got: &[PyValue], want: &[PyValue]) {
    assert_eq!(got.len(), want.len(), "{label}: case count");
    let mismatches: Vec<String> = got
        .iter()
        .zip(want)
        .filter(|(got, want)| got != want)
        .map(|(got, want)| format!("  got  {}\n  want {}", got.repr(), want.repr()))
        .collect();
    assert!(
        mismatches.is_empty(),
        "{label}: {} of {} cases differ:\n{}",
        mismatches.len(),
        got.len(),
        mismatches.join("\n")
    );
}

#[test]
fn library_matches_every_expected_outcome() {
    let (cases, expected) = reference();
    assert_eq!(cases.len(), 465, "the reference has 465 inputs");
    let got: Vec<PyValue> = cases.iter().map(library_outcome).collect();
    assert_eq!(
        kind_counts(&expected),
        (370, 93, 2),
        "reference composition"
    );
    assert_eq!(kind_counts(&got), (370, 93, 2), "candidate composition");
    assert_all_equal("library", &got, &expected);
}

#[test]
fn jsonl_transport_matches_every_expected_outcome() {
    let (cases, expected) = reference();
    let input: String = cases
        .iter()
        .map(|case| {
            let mut line = case.dumps_ascii().unwrap();
            line.push('\n');
            line
        })
        .collect();
    let output = evaluate_lines(&input);
    let got: Vec<PyValue> = output
        .lines()
        .map(|line| loads(line).expect("output line is JSON"))
        .collect();
    assert_all_equal("jsonl", &got, &expected);
}

#[test]
fn verdict_json_is_byte_identical_to_python_dumps() {
    let (cases, _) = reference();
    let goldens = load_jsonl("pretty_goldens.jsonl");
    assert_eq!(goldens.len(), 370, "one golden per value case");
    let by_id: std::collections::HashMap<String, &PyValue> = cases
        .iter()
        .map(|case| (text_of(dict(case).get("id")), case))
        .collect();
    for golden in &goldens {
        let id = text_of(golden.get("id"));
        let case = dict(by_id[&id]);
        let now = parse_utc(case.get("now_utc").unwrap(), "--now-utc").unwrap();
        let evaluation = evaluate(case.get("state").unwrap(), &now).unwrap();
        let mut rendered = evaluation.to_value().dumps_pretty().unwrap();
        rendered.push_str("\n");
        let rendered = rendered.to_utf8().unwrap();
        assert_eq!(
            rendered,
            text_of(golden.get("stdout")),
            "stdout bytes for {id}"
        );
        let exit_code = golden
            .get("exit_code")
            .and_then(PyValue::as_plain_int)
            .unwrap();
        assert_eq!(
            exit_code.to_string(),
            evaluation.exit_code().to_string(),
            "exit code for {id}"
        );
    }
}

/// The compiled-in repeat floor must be the schema's own `minimum`, not a copy that can drift.
#[test]
fn repeat_floor_tracks_the_campaign_schema() {
    let root = std::env::var_os("EASY_VLLM_SOURCE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .unwrap()
                .to_path_buf()
        });
    let schema = load_json_file(&root.join("campaigns/_template/campaign.schema.json"));
    let minimum = ["definitions", "budgets", "properties", "repeats", "minimum"]
        .iter()
        .try_fold(&schema, |node, key| match node {
            PyValue::Dict(map) => map.get(key),
            _ => None,
        })
        .and_then(PyValue::as_plain_int)
        .expect("schema declares budgets.repeats.minimum");
    assert_eq!(
        minimum.to_string(),
        sweep_stop::FULL_REPEATS_MIN.to_string()
    );
}

#[test]
fn an_unprintable_integer_is_reported_like_python_not_hidden() {
    let digits = "9".repeat(4300);
    let state = format!(
        "{{\"declared_budget\": {{\"max_cells\": 6, \"wall_clock_budget_s\": 3600, \"consecutive_failure_limit\": 3, \"declared_by\": \"a\", \"basis\": \"b\"}}, \"cells_planned\": 0, \"started_utc\": \"2026-09-04T09:00:00Z\", \"cells\": [{{\"cell_key\": \"a\", \"cell_outcome\": \"measured\", \"repetition\": {{\"runs_attempted\": {digits}}}}}, {{\"cell_key\": \"b\", \"cell_outcome\": \"measured\", \"repetition\": {{\"runs_attempted\": {digits}}}}}], \"cells_remaining\": []}}"
    );
    let state = loads(&state).unwrap();
    let now = parse_utc(&PyValue::str("2026-09-04T09:30:00Z"), "--now-utc").unwrap();
    let verdict = evaluate(&state, &now).unwrap();
    assert_eq!(
        verdict.runs_attempted.to_string().len(),
        4301,
        "the sum is computed exactly"
    );
    let error = verdict.to_value().dumps_pretty().unwrap_err();
    assert_eq!(error.python_type(), "ValueError");
}
