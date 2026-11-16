//! The `resolve_topology_api` program itself: the 59 frozen original-first API cases (23 `resolve` with a declared local
//! owner-output fixture, 27 helper cases, 2 reader cases and 7 `_run` cases) sent as `evaluate-jsonl` rows through the real
//! binary, each in its own scratch tree whose whole content afterwards is compared; then the call-shape controls the
//! frozen cases do not reach (the retired interpreter route, the timeout forms, a descendant that holds the pipes).
//!
//! The judge of `resolve` in the frozen cases is a check-only shell stand-in of the native helper that answers the declared
//! bytes and exit status (`common::stub_helper`, named through `RESOLVE_TOPOLOGY_PARITY_BIN`); the real native helper judges
//! the cases that declare no answer and is exercised by the self-test checks. No Python runs (plan_26111615 pass 3).

#[path = "resolve_topology_common.rs"]
mod common;

use common::*;
use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

const API: &str = env!("CARGO_BIN_EXE_resolve_topology_api");

/// Sends the rows through the binary in the scratch tree (its working directory) and returns the answer lines.
fn evaluate(
    scratch: &Scratch,
    rows: &[String],
    environment: &[(&str, Option<&str>)],
) -> Vec<PyValue> {
    let mut command = Command::new(API);
    command
        .arg("evaluate-jsonl")
        .current_dir(&scratch.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .env_remove("RESOLVE_TOPOLOGY_PYTHON");
    match helper_for(scratch) {
        Some(stub) => command.env("RESOLVE_TOPOLOGY_PARITY_BIN", stub),
        None => command.env_remove("RESOLVE_TOPOLOGY_PARITY_BIN"),
    };
    for (name, value) in environment {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
    let mut child = command.spawn().expect("spawn");
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

fn request(id: &str, operation: &str, arguments: &PyValue) -> String {
    let mut request = PyDict::new();
    request.set("id", PyValue::str(id));
    request.set("operation", PyValue::str(operation));
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
fn the_frozen_api_cases_answer_the_same_through_the_binary() {
    let cases = list(read_json("api_cases.json"));
    assert_eq!(cases.len(), 59);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("api");
        scratch.populate(field(case, "before"));
        let arguments = fill(field(case, "arguments"), &scratch);
        let started = std::time::Instant::now();
        let answers = evaluate(
            &scratch,
            &[request(&id, &text(field(case, "operation")), &arguments)],
            &[],
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(30),
            "{id}"
        );
        let want = fill(field(case, "expected"), &scratch);
        let outcome = std::panic::catch_unwind(|| same_answer(&answers[0], dict(&want), &id));
        let tree_ok = scratch.snapshot() == expected_tree(field(case, "after"));
        if outcome.is_err() || !tree_ok {
            failures.push(format!(
                "{id}: answer_ok={} tree_ok={tree_ok}",
                outcome.is_ok()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "differing cases:\n{}",
        failures.join("\n")
    );
}

/// A checkout tree whose owner fixture answers a PASS verdict (the frozen case `002-pass-single`).
fn passing_scratch() -> Scratch {
    let cases = list(read_json("api_cases.json"));
    let case = cases
        .iter()
        .map(dict)
        .find(|case| text(field(case, "id")) == "002-pass-single")
        .expect("the frozen case");
    let scratch = Scratch::new("api-python");
    scratch.populate(field(case, "before"));
    scratch
}

fn resolve_row(scratch: &Scratch, extra: &str) -> String {
    format!(
        r#"{{"id":"r","operation":"resolve","arguments":{{"repo":{}{extra}}}}}"#,
        PyValue::str(&scratch.text()).dumps_ascii().unwrap()
    )
}

#[test]
fn an_interpreter_request_is_the_named_configuration_error_and_a_falsy_one_is_the_native_default() {
    let scratch = passing_scratch();
    let value_of = |answers: &[PyValue]| key(field(dict(&answers[0]), "value"));
    // a falsy `python` (absent, null, "", 0, false, []) is the native helper (here the stand-in that declares PASS single)
    for extra in [
        "",
        r#","python":null"#,
        r#","python":"""#,
        r#","python":0"#,
        r#","python":false"#,
        r#","python":[]"#,
    ] {
        let answers = evaluate(&scratch, &[resolve_row(&scratch, extra)], &[]);
        assert!(value_of(&answers).starts_with("[0, \"single\""), "{extra}: {}", value_of(&answers));
    }
    // a truthy `python` (any type) asks for the retired interpreter route: the named configuration error, nothing runs
    for extra in [r#","python":"python3""#, r#","python":"/nonexistent/interpreter""#, r#","python":5"#] {
        let answers = evaluate(&scratch, &[resolve_row(&scratch, extra)], &[]);
        let answer = dict(&answers[0]);
        assert_eq!(text(field(answer, "type")), "ResolveTopologyConfigurationError", "{extra}");
        assert!(text(field(answer, "message")).contains("retired"), "{extra}");
    }
    // so does a non-empty RESOLVE_TOPOLOGY_PYTHON in the environment, `python3` included
    let answers = evaluate(
        &scratch,
        &[resolve_row(&scratch, "")],
        &[("RESOLVE_TOPOLOGY_PYTHON", Some("python3"))],
    );
    assert_eq!(text(field(dict(&answers[0]), "type")), "ResolveTopologyConfigurationError");
}

fn run_row(argv: &str, timeout: Option<&str>) -> String {
    let timeout = timeout.map_or(String::new(), |timeout| format!(r#","timeout":{timeout}"#));
    format!(r#"{{"id":"x","operation":"_run","arguments":{{"argv":{argv}{timeout}}}}}"#)
}

fn answer_value(answers: &[PyValue]) -> String {
    key(field(dict(&answers[0]), "value"))
}

#[test]
fn run_reports_the_timeout_as_the_caller_wrote_it_and_refuses_what_it_does_not_model() {
    let scratch = Scratch::new("api-timeout");
    let sleeper = r#"["/bin/sh","-c","sleep 5"]"#;
    for (timeout, shown) in [
        ("0.2", "0.2"),
        ("0.25", "0.25"),
        ("0", "0"),
        ("-1", "-1"),
        ("0.0", "0.0"),
        ("1e-1", "0.1"),
    ] {
        let started = std::time::Instant::now();
        let answers = evaluate(&scratch, &[run_row(sleeper, Some(timeout))], &[]);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(4),
            "{timeout}"
        );
        assert_eq!(
            answer_value(&answers),
            format!(
                r#"[null, "", "Command '['/bin/sh', '-c', 'sleep 5']' timed out after {shown} seconds"]"#
            ),
            "{timeout}"
        );
    }
    for timeout in ["null", r#""5""#, "true", "[1]"] {
        let answers = evaluate(&scratch, &[run_row(sleeper, Some(timeout))], &[]);
        let answer = dict(&answers[0]);
        assert_eq!(text(field(answer, "type")), "UnsupportedInput", "{timeout}");
    }
    // the default timeout (60) applies to a call that names none
    let answers = evaluate(
        &scratch,
        &[run_row(r#"["/bin/sh","-c","printf ok"]"#, None)],
        &[],
    );
    assert_eq!(answer_value(&answers), r#"[0, "ok", ""]"#);
    // a command line that is not a list of strings
    let answers = evaluate(
        &scratch,
        &[
            run_row(r#"["/bin/sh",5]"#, None),
            run_row("[]", None),
            run_row(r#""ls""#, None),
        ],
        &[],
    );
    let kinds: Vec<(String, String)> = answers
        .iter()
        .map(|answer| {
            (
                text(field(dict(answer), "type")),
                text(field(dict(answer), "message")),
            )
        })
        .collect();
    assert_eq!(
        kinds[0],
        (
            "TypeError".to_string(),
            "expected str, bytes or os.PathLike object, not int".to_string()
        )
    );
    assert_eq!(
        kinds[1],
        (
            "IndexError".to_string(),
            "list index out of range".to_string()
        )
    );
    assert_eq!(kinds[2].0, "UnsupportedInput");
}

/// The reported defect of the original-first baseline: the direct child exits at once, a descendant keeps the inherited
/// pipes open. `_run` ends at its own deadline (not at the descendant's end) with the `TimeoutExpired` text.
#[test]
fn a_descendant_that_holds_the_pipes_is_bounded_by_the_deadline() {
    let scratch = Scratch::new("api-descendant");
    let argv = r#"["/bin/sh","-c","(sleep 2) & exit 0"]"#;
    let started = std::time::Instant::now();
    let answers = evaluate(&scratch, &[run_row(argv, Some("0.25"))], &[]);
    let elapsed = started.elapsed();
    assert_eq!(
        answer_value(&answers),
        r#"[null, "", "Command '['/bin/sh', '-c', '(sleep 2) & exit 0']' timed out after 0.25 seconds"]"#
    );
    assert!(
        elapsed < std::time::Duration::from_millis(1800),
        "{elapsed:?}"
    );
}

#[test]
fn the_wrong_calls_are_the_type_errors_of_the_original_signatures() {
    let scratch = Scratch::new("api-calls");
    let rows = [
        r#"{"id":1,"operation":"resolve","arguments":{}}"#,
        r#"{"id":2,"operation":"_manifests","arguments":{"repo":"/x","extra":1}}"#,
        r#"{"id":3,"operation":"_manifests","arguments":{"repo":5}}"#,
        r#"{"id":4,"operation":"_manifest_topology_line","arguments":{"path":null}}"#,
        r#"{"id":5,"operation":"main","arguments":{}}"#,
        r#"{"id":6,"operation":"resolve","arguments":{"repo":[1]}}"#,
        r#"{"id":7,"operation":"resolve","arguments":{"repo":"/nonexistent-topology-api-repo"},"result_json":true}"#,
    ]
    .map(String::from);
    let answers = evaluate(&scratch, &rows, &[]);
    let pair = |index: usize| {
        let answer = dict(&answers[index]);
        (text(field(answer, "type")), text(field(answer, "message")))
    };
    assert_eq!(
        pair(0),
        (
            "TypeError".to_string(),
            "resolve() missing 1 required positional argument: 'repo'".to_string()
        )
    );
    assert_eq!(
        pair(1),
        (
            "TypeError".to_string(),
            "_manifests() got an unexpected keyword argument 'extra'".to_string()
        )
    );
    assert_eq!(
        pair(2),
        (
            "TypeError".to_string(),
            "expected str, bytes or os.PathLike object, not int".to_string()
        )
    );
    assert_eq!(
        pair(3),
        (
            "TypeError".to_string(),
            "expected str, bytes or os.PathLike object, not NoneType".to_string()
        )
    );
    assert_eq!(
        pair(4),
        (
            "ValueError".to_string(),
            "unknown operation 'main'".to_string()
        )
    );
    assert_eq!(
        pair(5),
        (
            "TypeError".to_string(),
            "expected str, bytes or os.PathLike object, not list".to_string()
        )
    );
    let tuple = text(field(dict(&answers[6]), "value_json"));
    assert!(
        tuple.starts_with("[3, null, \"output/*/manifest.yaml"),
        "{tuple}"
    );
}

#[test]
fn the_native_commands_and_a_wrong_call() {
    let help = Command::new(API).arg("native-help").output().expect("run");
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).contains("evaluate-jsonl"));
    let constants = Command::new(API).arg("constants").output().expect("run");
    assert_eq!(constants.status.code(), Some(0));
    let shown = String::from_utf8_lossy(&constants.stdout).into_owned();
    assert!(
        shown.contains("\"EXIT_UNRESOLVED\": 5")
            && shown.contains("\"TAG\": \"[resolve_topology]\""),
        "{shown}"
    );
    let wrong = Command::new(API).arg("nonsense").output().expect("run");
    assert_eq!(wrong.status.code(), Some(2));
}
