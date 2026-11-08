//! The corrections of the second round, each against saved original results (`checks/data/v2_cases.json`, taken from the
//! original programs' one-time runs; nothing is computed by the code under test):
//!
//! * the signed limits of the C `int` precision and of sequence repetition (6 original calls), the bare `--` and the
//!   malformed, undecodable and directory `--parsed` files (4 command lines);
//! * the order of evaluation (a NaN headroom sort and the two conversion orders of the feasible limits, 3 calls) and the
//!   keyword-before-positional priority of Python's call binding (CPython's own messages);
//! * the failing output descriptor `/dev/full` (an operating-system sink, no disk): the interpreter's exit statuses and
//!   reports for the three command lines, and a deliberate non-zero report for the API;
//! * carriage returns and CRLF kept literal in the report;
//! * the benchmark program's preflight against whole expected values, and its refusal to report after any failure.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::bench::{run_cases, Case};
use recipe_runtime::observe::same_value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join(name)
}

fn document() -> PyDict {
    let text = std::fs::read_to_string(data("v2_cases.json")).expect("NOT_RUN: saved v2 cases");
    match loads(&text).expect("JSON") {
        PyValue::Dict(dict) => dict,
        other => panic!("{}", other.repr()),
    }
}

fn rows(key: &str) -> Vec<PyDict> {
    match document().get(key) {
        Some(PyValue::List(items)) => items
            .iter()
            .map(|item| match item {
                PyValue::Dict(dict) => dict.clone(),
                other => panic!("{}", other.repr()),
            })
            .collect(),
        other => panic!("{other:?}"),
    }
}

fn text(value: Option<&PyValue>) -> String {
    match value {
        Some(PyValue::Str(own)) => own.to_utf8().expect("text"),
        other => panic!("{other:?}"),
    }
}

fn api(requests: &[PyDict]) -> Vec<PyDict> {
    let mut input = String::new();
    for request in requests {
        let mut line = PyDict::new();
        for key in ["id", "operation", "args", "kwargs"] {
            line.set(key, request.get(key).cloned().expect("request member"));
        }
        input.push_str(&PyValue::Dict(line).dumps_ascii().expect("request"));
        input.push('\n');
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_recipe_numeric_api"))
        .arg("evaluate-jsonl")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success() && output.stderr.is_empty());
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .lines()
        .map(|line| match loads(line).expect("record") {
            PyValue::Dict(dict) => dict,
            other => panic!("{}", other.repr()),
        })
        .collect()
}

fn check_api(key: &str, count: usize) {
    let cases = rows(key);
    assert_eq!(cases.len(), count);
    let answers = api(&cases);
    assert_eq!(answers.len(), count);
    let mut problems = Vec::new();
    for (case, got) in cases.iter().zip(&answers) {
        let id = text(case.get("id"));
        let PyValue::Dict(want) = case.get("expected").expect("expected") else {
            panic!()
        };
        for (member, value) in want.iter() {
            let name = member.to_utf8().unwrap();
            if !got
                .get(&name)
                .is_some_and(|actual| same_value(value, actual))
            {
                problems.push(format!(
                    "{id} {name}: {:?} != {}",
                    got.get(&name).map(PyValue::repr),
                    value.repr().chars().take(200).collect::<String>()
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn signed_c_int_precision_and_index_sized_repetition_raise_the_originals_overflow_errors() {
    check_api("boundary_api", 6);
    // In range: the smallest C int formats with no decimals; one below it, and a repeated empty or negative count above the
    // signed index range, are the OverflowError; a count in range repeats (bool counts as an integer).
    let probe = |operation: &str, args: &str| {
        let request = format!(r#"{{"id": "p", "operation": "{operation}", "args": {args}}}"#);
        let mut dict = PyDict::new();
        if let PyValue::Dict(parsed) = loads(&request).unwrap() {
            for (key, value) in parsed.iter() {
                dict.set(&key.to_utf8().unwrap(), value.clone());
            }
        }
        dict.set("kwargs", PyValue::Dict(PyDict::new()));
        api(&[dict]).remove(0)
    };
    let value = |row: &PyDict| row.get("value").cloned().unwrap_or(PyValue::None);
    assert_eq!(
        value(&probe("rank_recipes._fmt_num", "[1.5, -2147483648]")),
        PyValue::str("2")
    );
    let error = probe("rank_recipes._fmt_num", "[1.5, -2147483649]");
    assert_eq!(error.get("type"), Some(&PyValue::str("OverflowError")));
    assert_eq!(
        error.get("message"),
        Some(&PyValue::str("Python int too large to convert to C int"))
    );
    let report = value(&probe(
        "rank_recipes.render_report",
        r#"[{}, {}, "ab", 3, 1]"#,
    ));
    assert!(
        text(Some(&report)).contains("hard-gate: ababab GiB"),
        "{}",
        report.repr()
    );
    let limit = probe(
        "rank_recipes.render_report",
        r#"[{}, {}, "ab", 9223372036854775808, 1]"#,
    );
    assert_eq!(
        limit.get("message"),
        Some(&PyValue::str(
            "cannot fit 'int' into an index-sized integer"
        ))
    );
    // Within the index range the product is still checked: a huge string cannot be built.
    let big = probe(
        "rank_recipes.render_report",
        r#"[{}, {}, "ab", 9223372036854775807, 1]"#,
    );
    let kind = text(big.get("type"));
    assert!(
        kind == "OverflowError" || kind == "MemoryError",
        "{}",
        PyValue::Dict(big).repr()
    );
}

fn run_cli(program: &str, args: &[String], stdout: Stdio) -> (Option<i32>, String, String) {
    let output = Command::new(program)
        .args(args)
        .env_clear()
        .stdout(stdout)
        .stderr(Stdio::piped())
        .output()
        .expect("run");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn program(name: &str) -> &'static str {
    match name {
        "quant_table" => env!("CARGO_BIN_EXE_quant_table"),
        "estimate_vram" => env!("CARGO_BIN_EXE_estimate_vram"),
        "rank_recipes" => env!("CARGO_BIN_EXE_rank_recipes"),
        other => panic!("{other}"),
    }
}

fn args_of(case: &PyDict) -> Vec<String> {
    let inputs = [
        ("{BINPUT}", data("boundary_input")),
        ("{CRINPUT}", data("cr_input")),
        ("{INPUT}", data("cli_input")),
    ];
    match case.get("args") {
        Some(PyValue::List(items)) => items
            .iter()
            .map(|item| {
                let mut value = text(Some(item));
                for (name, path) in &inputs {
                    value = value.replace(name, path.to_str().unwrap());
                }
                value
            })
            .collect(),
        other => panic!("{other:?}"),
    }
}

fn expected_text(case: &PyDict, key: &str) -> String {
    let mut value = text(case.get(key));
    for (name, path) in [
        ("{BINPUT}", data("boundary_input")),
        ("{CRINPUT}", data("cr_input")),
        ("{INPUT}", data("cli_input")),
    ] {
        value = value.replace(name, path.to_str().unwrap());
    }
    value
}

#[test]
fn the_boundary_command_lines_end_as_the_originals_did() {
    for case in rows("boundary_cli") {
        let (code, out, err) = run_cli(
            program(&text(case.get("program"))),
            &args_of(&case),
            Stdio::piped(),
        );
        let id = text(case.get("id"));
        assert_eq!(
            code.map(i64::from),
            match case.get("exit") {
                Some(PyValue::Int(n)) => n.to_string().parse().ok(),
                _ => None,
            },
            "{id}"
        );
        assert_eq!(out, expected_text(&case, "stdout"), "{id}");
        assert_eq!(err, expected_text(&case, "stderr"), "{id}");
    }
}

#[test]
fn evaluation_order_and_nan_sorting_follow_the_original() {
    check_api("order", 3);
}

#[test]
fn keywords_are_checked_before_surplus_positionals_as_cpython_binds() {
    // Expected texts are what CPython raised for these two calls (the independent binding primitive).
    for (kwargs, message) in [
        (
            r#"{"unexpected": 1}"#,
            "is_offline_only_quant() got an unexpected keyword argument 'unexpected'",
        ),
        (
            r#"{"quant": 1}"#,
            "is_offline_only_quant() got multiple values for argument 'quant'",
        ),
    ] {
        let request = format!(
            r#"{{"id": "b", "operation": "quant_table.is_offline_only_quant", "args": [null, null], "kwargs": {kwargs}}}"#
        );
        let mut dict = PyDict::new();
        if let PyValue::Dict(parsed) = loads(&request).unwrap() {
            for (key, value) in parsed.iter() {
                dict.set(&key.to_utf8().unwrap(), value.clone());
            }
        }
        let row = api(&[dict]).remove(0);
        assert_eq!(row.get("type"), Some(&PyValue::str("TypeError")));
        assert_eq!(row.get("message"), Some(&PyValue::str(message)));
    }
    // With no bad keyword the surplus positional is still reported, then missing parameters.
    let request = r#"{"id": "b", "operation": "quant_table.is_offline_only_quant", "args": [null, null], "kwargs": {}}"#;
    let mut dict = PyDict::new();
    if let PyValue::Dict(parsed) = loads(request).unwrap() {
        for (key, value) in parsed.iter() {
            dict.set(&key.to_utf8().unwrap(), value.clone());
        }
    }
    let row = api(&[dict]).remove(0);
    assert_eq!(
        row.get("message"),
        Some(&PyValue::str(
            "is_offline_only_quant() takes 1 positional argument but 2 were given"
        ))
    );
}

#[test]
fn a_full_output_descriptor_ends_every_program_as_the_interpreter_or_a_deliberate_report_does() {
    let full = || {
        Stdio::from(
            std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .expect("/dev/full"),
        )
    };
    for case in rows("io") {
        let id = text(case.get("id"));
        let (code, _, err) = run_cli(program(&text(case.get("program"))), &args_of(&case), full());
        assert_eq!(
            code.map(i64::from),
            match case.get("exit") {
                Some(PyValue::Int(n)) => n.to_string().parse().ok(),
                _ => None,
            },
            "{id}"
        );
        assert_eq!(err, expected_text(&case, "stderr"), "{id}");
    }
    for (arguments, input) in [
        (vec!["constants"], ""),
        (
            vec!["evaluate-jsonl"],
            "{\"id\": \"1\", \"operation\": \"quant_table.dtype_bpw\", \"args\": [\"bf16\"]}\n",
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_recipe_numeric_api"))
            .args(&arguments)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(full())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write");
        let output = child.wait_with_output().expect("wait");
        let err = String::from_utf8_lossy(&output.stderr).into_owned();
        assert_eq!(output.status.code(), Some(1), "{arguments:?}: {err}");
        assert_eq!(err, "recipe_numeric_api: cannot write standard output: OSError: [Errno 28] No space left on device\n");
    }
}

#[test]
fn carriage_returns_and_crlf_stay_literal_in_the_report() {
    let PyValue::Dict(case) = document().get("cr").cloned().expect("cr case") else {
        panic!()
    };
    let (code, out, err) = run_cli(
        program(&text(case.get("program"))),
        &args_of(&case),
        Stdio::piped(),
    );
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(out, expected_text(&case, "stdout"));
    assert_eq!(err, expected_text(&case, "stderr"));
    assert!(
        out.contains("x\ry\r\nz") && out.contains("warn\rvalue\r\nend"),
        "carriage returns survive as they were written"
    );
}

/// A benchmark fixture file in a directory this test created for itself (exclusively, never reusing a name that exists) and removes
/// when the value is dropped, and nothing else.
struct BenchFixture {
    _directory: recipe_runtime::model_files::PrivateDirectory,
    path: PathBuf,
}

impl std::ops::Deref for BenchFixture {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.path
    }
}

fn bench_fixture(edit: impl Fn(&mut Vec<PyValue>)) -> BenchFixture {
    let directory = recipe_runtime::model_files::PrivateDirectory::create("recipe-bench-v2-")
        .expect("a private fixture directory");
    let source = std::fs::read_to_string(data("bench_fixture_v2.json")).expect("fixture");
    let PyValue::List(mut rows) = loads(&source).unwrap() else {
        panic!()
    };
    edit(&mut rows);
    let path = directory.path().join("fixture.json");
    std::fs::write(&path, PyValue::List(rows).dumps_ascii().unwrap()).unwrap();
    BenchFixture {
        _directory: directory,
        path,
    }
}

fn run_bench(fixture: &std::path::Path) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_recipe_numeric_bench"))
        .args([
            "--fixture",
            fixture.to_str().unwrap(),
            "--rounds",
            "2",
            "--iterations",
            "2",
        ])
        .env_clear()
        .output()
        .expect("run");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn set_member(rows: &mut [PyValue], index: usize, key: &str, value: PyValue) {
    if let PyValue::Dict(dict) = &mut rows[index] {
        dict.set(key, value);
    }
}

#[test]
fn the_benchmark_checks_every_whole_value_before_any_loop_and_reports_only_complete_series() {
    // The frozen fixture with its saved expected values: three series, the report shape (diagnostic timer, 2 x 2).
    let good = bench_fixture(|_| {});
    let (code, out, err) = run_bench(&good);
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(out.lines().count(), 4, "{out}");
    // A wrong value, a wrong type (float 2.0 as the integer 2) or a different key order in the expected whole value is a
    // preflight failure: no report at all, although the first series would have succeeded.
    let wrong_value = bench_fixture(|rows| set_member(rows, 2, "expected", PyValue::Float(0.25)));
    let wrong_type = bench_fixture(|rows| {
        let PyValue::Dict(row) = &rows[1] else {
            panic!()
        };
        let PyValue::Dict(expected) = row.get("expected").unwrap() else {
            panic!()
        };
        let mut changed = PyDict::new();
        for (key, value) in expected.iter() {
            let key_text = key.to_utf8().unwrap();
            changed.set(
                &key_text,
                if key_text == "kv_gib" {
                    PyValue::int(2)
                } else {
                    value.clone()
                },
            );
        }
        set_member(rows, 1, "expected", PyValue::Dict(changed));
    });
    let reordered = bench_fixture(|rows| {
        let PyValue::Dict(row) = &rows[1] else {
            panic!()
        };
        let PyValue::Dict(expected) = row.get("expected").unwrap() else {
            panic!()
        };
        let mut entries: Vec<_> = expected.iter().cloned().collect();
        entries.swap(0, 1);
        let mut changed = PyDict::new();
        for (key, value) in entries {
            changed.insert(key, value);
        }
        set_member(rows, 1, "expected", PyValue::Dict(changed));
    });
    for fixture in [&wrong_value, &wrong_type, &reordered] {
        let (code, out, err) = run_bench(fixture);
        assert_eq!(code, Some(1), "{err}");
        assert!(
            out.is_empty(),
            "no timing is reported after a failed preflight: {out}"
        );
        assert!(
            err.contains("differs from the saved original result"),
            "{err}"
        );
    }
    let missing = bench_fixture(|rows| {
        if let PyValue::Dict(row) = &mut rows[0] {
            let mut copy = PyDict::new();
            for (key, value) in row.iter() {
                if !key.equals("expected") {
                    copy.insert(key.clone(), value.clone());
                }
            }
            rows[0] = PyValue::Dict(copy);
        }
    });
    let (code, out, err) = run_bench(&missing);
    assert_eq!((code, out.is_empty()), (Some(1), true));
    assert!(err.contains("no expected value"), "{err}");
    // The fixture directories are removed by their owners when they go out of scope.
}

fn injected(metric: &str, expected: PyValue, fail_from_call: Option<usize>) -> Case {
    let calls = AtomicUsize::new(0);
    Case {
        metric: metric.to_string(),
        expected: expected.clone(),
        call: Box::new(move || {
            let number = calls.fetch_add(1, Ordering::SeqCst) + 1;
            match fail_from_call {
                Some(first) if number >= first => Err(
                    agent_runtime_support::failure::Failure::new("ValueError", "injected"),
                ),
                _ => Ok(expected.clone()),
            }
        }),
    }
}

#[test]
fn a_failure_inside_a_measured_loop_or_in_the_warm_call_leaves_no_report_even_after_an_earlier_series(
) {
    let value = || PyValue::Float(0.5);
    let ok = run_cases(
        &[
            injected("first", value(), None),
            injected("second", value(), None),
        ],
        2,
        3,
    )
    .unwrap();
    assert_eq!(ok.lines().count(), 3);
    // The second series passes its warm call (call 1) and fails inside the loop (call 2 onwards).
    let timed = run_cases(
        &[
            injected("first", value(), None),
            injected("second", value(), Some(2)),
        ],
        2,
        3,
    )
    .unwrap_err();
    assert_eq!(timed.class, "BenchmarkError");
    assert!(
        timed
            .message
            .to_utf8()
            .unwrap()
            .contains("second failed inside a measured loop"),
        "{}",
        timed.line().to_utf8().unwrap()
    );
    let warm = run_cases(
        &[
            injected("first", value(), None),
            injected("second", value(), Some(1)),
        ],
        2,
        3,
    )
    .unwrap_err();
    assert!(warm
        .message
        .to_utf8()
        .unwrap()
        .contains("second fails before measurement"));
    let wrong = run_cases(
        &[Case {
            metric: "m".into(),
            expected: PyValue::int(1),
            call: Box::new(|| Ok(PyValue::Bool(true))),
        }],
        1,
        1,
    )
    .unwrap_err();
    assert!(
        wrong
            .message
            .to_utf8()
            .unwrap()
            .contains("differs from the saved original result"),
        "a bool is not the integer 1"
    );
}
