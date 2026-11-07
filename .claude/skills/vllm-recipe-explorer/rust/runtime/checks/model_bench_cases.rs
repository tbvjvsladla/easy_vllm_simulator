//! The frozen post-quality measurement program of the model functions: what it checks before any timer starts, what it refuses,
//! and the shape of the report. The fixture is built from the saved original results (the three pure and the two file-reading
//! functions of the measurement contract); the checks run only a handful of timed iterations (diagnostics, not a measurement).

use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_bench::{load, run_cases};

use super::support::{items, load as load_saved, member, rebased, text_of, Scratch, TOKEN};

/// The saved case and result rows of one id, as a fixture row `{metric, operation, args, kwargs, expected, source_id}`.
fn fixture_row(id: &str, reference: &str) -> PyValue {
    let cases = load_saved("cases.json");
    let results = load_saved("results.json");
    let case = items(&cases)
        .iter()
        .find(|case| text_of(member(case, "id")) == id)
        .expect("a saved case");
    let row = items(member(&results, "rows"))
        .iter()
        .find(|row| text_of(member(row, "id")) == id)
        .expect("a saved result");
    let replacements = [(TOKEN, reference.to_string())];
    let operation = format!(
        "{}.{}",
        text_of(member(case, "owner")),
        text_of(member(case, "function"))
    );
    let mut dict = PyDict::new();
    dict.set("metric", PyValue::str(&operation));
    dict.set("operation", PyValue::str(&operation));
    dict.set("args", rebased(member(case, "args"), &replacements));
    dict.set("kwargs", rebased(member(case, "kwargs"), &replacements));
    dict.set("expected", rebased(member(row, "value"), &replacements));
    dict.set("source_id", PyValue::str(id));
    PyValue::Dict(dict)
}

fn fixture(ids: &[&str], reference: &str) -> String {
    PyValue::List(ids.iter().map(|id| fixture_row(id, reference)).collect())
        .dumps_unicode()
        .expect("a fixture")
        .to_utf8()
        .expect("text")
}

const PURE: [&str; 3] = ["api-004", "api-023", "api-142"];
const FILES: [&str; 2] = ["api-102", "api-172"];

#[test]
fn the_pure_and_the_file_reading_fixtures_pass_and_report_every_metric() {
    let scratch = Scratch::new("bench");
    let reference = scratch.root.to_string_lossy().into_owned();
    for ids in [&PURE[..], &FILES[..]] {
        let mut cases = load(&fixture(ids, &reference)).expect("a fixture");
        let report = run_cases(&mut cases, 2, 3).expect("every case passes");
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines.len(), ids.len() + 1, "{report}");
        assert_eq!(lines[0], r#"{"rounds": 2, "iterations": 3}"#);
        for line in &lines[1..] {
            assert!(
                line.contains(r#""unit": "ns_per_call""#) && line.contains(r#""round_count": 2"#),
                "{line}"
            );
        }
    }
}

#[test]
fn a_wrong_expected_value_ends_the_run_before_any_timing_and_leaves_no_report() {
    let scratch = Scratch::new("bench-refused");
    let reference = scratch.root.to_string_lossy().into_owned();
    // A tuple the original returned must be tagged as one: the same elements as a plain list are a different value.
    let tuple_as_list =
        fixture(&PURE, &reference).replace(r#"{"__tuple__": [2, true,"#, r#"[2, true,"#);
    let tuple_as_list = tuple_as_list.replacen("[]]}", "[]]", 1);
    let mut cases = load(&tuple_as_list).expect("a fixture");
    let failure = run_cases(&mut cases, 1, 1).expect_err("a tuple is not a list");
    assert_eq!(failure.class, "BenchmarkError");
    assert!(failure
        .message
        .to_utf8()
        .is_some_and(|text| text.contains("the value differs from the saved original result")));
    // A changed number, a changed key order and an integer where the original had a float are all refused.
    for (from, to) in [
        (r#""num_params": 32"#, r#""num_params": 33"#),
        (r#""serve_bpw": 2.0"#, r#""serve_bpw": 2"#),
    ] {
        let text = fixture(&FILES, &reference);
        assert!(text.contains(from), "{from}");
        let mut cases = load(&text.replacen(from, to, 1)).expect("a fixture");
        assert!(run_cases(&mut cases, 1, 1).is_err(), "{from}");
    }
    // A function that raises is refused before measurement, and a row without an expected value is refused at load.
    let raising = r#"[{"metric": "m", "operation": "parse_model_config._model_id_from_path", "args": [3], "kwargs": {}, "expected": "x"}]"#;
    let failure =
        run_cases(&mut load(raising).expect("loads"), 1, 1).expect_err("a call that fails");
    assert!(failure.message.to_utf8().is_some_and(|text| text
        .contains("fails before measurement")
        && text.contains("AttributeError")));
    let no_expected = r#"[{"metric": "m", "operation": "parse_model_config._model_id_from_path", "args": ["a"], "kwargs": {}}]"#;
    assert!(load(no_expected).is_err());
    assert!(load("{}").is_err());
}

#[test]
fn the_program_prints_the_report_only_when_every_case_passed() {
    let scratch = Scratch::new("bench-program");
    let reference = scratch.root.to_string_lossy().into_owned();
    let write = |name: &str, text: &str| {
        let path = scratch.root.join(name);
        std::fs::write(&path, text).expect("write a fixture");
        path
    };
    let good = write("good.json", &fixture(&PURE, &reference));
    let run = |path: &std::path::Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_recipe_model_bench"))
            .arg("--fixture")
            .arg(path)
            .args(["--rounds", "1", "--iterations", "1"])
            .output()
            .expect("run the program")
    };
    let output = run(&good);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 4);
    let bad = write(
        "bad.json",
        &fixture(&PURE, &reference)
            .replace(r#""expected": "Org/Name""#, r#""expected": "Org/Other""#),
    );
    let output = run(&bad);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "nothing is reported after a refusal"
    );
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("recipe_model_bench: "));
}
