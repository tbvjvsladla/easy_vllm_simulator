//! The saved original executions of the three modules, compared with the native programs:
//!
//! * 255 calls of the importable functions (every function, helper and boundary the originals were driven through:
//!   numbers, strings, `None`, `bool`, lists and objects as arguments, hybrid and dense shapes, missing and malformed
//!   fields, the exceptions with their class and message) through one `recipe_numeric_api evaluate-jsonl` process;
//! * 24 command-line runs (normal output, JSON, report, self-test, help, argparse errors, unhandled exceptions).
//!
//! Expected values are the saved results of the original Python programs (`checks/data/*.json`); nothing here is
//! computed by the code under test. The exception frames of the original tracebacks were removed from the saved CLI
//! stderr (the final `Class: message` line is kept); native stderr is compared as it is.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join(name)
}

fn read_rows(name: &str) -> Vec<PyDict> {
    let text =
        std::fs::read_to_string(data(name)).unwrap_or_else(|e| panic!("NOT_RUN: {name}: {e}"));
    match loads(&text).expect("the saved data is JSON") {
        PyValue::List(rows) => rows
            .into_iter()
            .map(|row| match row {
                PyValue::Dict(dict) => dict,
                other => panic!("not an object: {}", other.repr()),
            })
            .collect(),
        other => panic!("not an array: {}", other.repr()),
    }
}

/// Type-exact equality as the original's observer used: NaN equals NaN, objects compare their key order.
fn strict_equal(left: &PyValue, right: &PyValue) -> bool {
    match (left.peel(), right.peel()) {
        (PyValue::Float(a), PyValue::Float(b)) => {
            (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits() || a == b
        }
        (PyValue::List(a), PyValue::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| strict_equal(x, y))
        }
        (PyValue::Dict(a), PyValue::Dict(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && strict_equal(va, vb))
        }
        (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b) && a == b,
    }
}

fn field<'a>(row: &'a PyDict, key: &str) -> &'a PyValue {
    row.get(key).unwrap_or_else(|| panic!("missing {key}"))
}

fn text(value: &PyValue) -> String {
    match value {
        PyValue::Str(own) => own.to_utf8().expect("text"),
        other => panic!("not a string: {}", other.repr()),
    }
}

#[test]
fn every_saved_function_call_is_reproduced_by_one_api_process() {
    let cases = read_rows("api_cases.json");
    assert_eq!(cases.len(), 255);
    let mut input = String::new();
    for case in &cases {
        let mut request = PyDict::new();
        request.set("id", field(case, "id").clone());
        request.set("operation", field(case, "operation").clone());
        request.set("args", field(case, "args").clone());
        request.set("kwargs", field(case, "kwargs").clone());
        input.push_str(&PyValue::Dict(request).dumps_ascii().expect("request"));
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
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answers: Vec<String> = String::from_utf8(output.stdout)
        .expect("utf-8")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(answers.len(), cases.len(), "exactly one record per request");
    let mut problems = Vec::new();
    for (case, line) in cases.iter().zip(&answers) {
        let got = match loads(line).expect("a record is JSON") {
            PyValue::Dict(dict) => dict,
            other => panic!("not an object: {}", other.repr()),
        };
        let want = match field(case, "expected") {
            PyValue::Dict(dict) => dict,
            other => panic!("not an object: {}", other.repr()),
        };
        let id = text(field(case, "id"));
        if !strict_equal(field(&got, "id"), field(case, "id")) {
            problems.push(format!("{id}: id"));
        }
        let kind = text(field(want, "kind"));
        let mut keys = vec!["kind", "stdout", "stderr"];
        keys.extend(if kind == "value" {
            vec!["value"]
        } else {
            vec!["type", "message"]
        });
        for key in keys {
            if !strict_equal(field(&got, key), field(want, key)) {
                problems.push(format!(
                    "{id} {key}: {} != {}",
                    field(&got, key)
                        .repr()
                        .chars()
                        .take(160)
                        .collect::<String>(),
                    field(want, key)
                        .repr()
                        .chars()
                        .take(160)
                        .collect::<String>()
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{} problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn every_saved_command_line_run_is_reproduced() {
    let cases = read_rows("cli_cases.json");
    assert_eq!(cases.len(), 24);
    let input = data("cli_input");
    let input_text = input.to_str().expect("path");
    let mut problems = Vec::new();
    for case in &cases {
        let id = text(field(case, "id"));
        let program = match text(field(case, "program")).as_str() {
            "quant_table" => env!("CARGO_BIN_EXE_quant_table"),
            "estimate_vram" => env!("CARGO_BIN_EXE_estimate_vram"),
            "rank_recipes" => env!("CARGO_BIN_EXE_rank_recipes"),
            other => panic!("unknown program {other}"),
        };
        let args: Vec<String> = match field(case, "args") {
            PyValue::List(items) => items
                .iter()
                .map(|item| text(item).replace("{INPUT}", input_text))
                .collect(),
            other => panic!("not an array: {}", other.repr()),
        };
        let output = Command::new(program)
            .args(&args)
            .env_clear()
            .current_dir(&input)
            .output()
            .expect("run");
        let want_exit = match field(case, "exit") {
            PyValue::Int(number) => number.to_string(),
            other => panic!("exit: {}", other.repr()),
        };
        let want_out = text(field(case, "stdout")).replace("{INPUT}", input_text);
        let want_err = text(field(case, "stderr")).replace("{INPUT}", input_text);
        if output.status.code().map(|code| code.to_string()) != Some(want_exit) {
            problems.push(format!("{id}: exit {:?}", output.status.code()));
        }
        if String::from_utf8_lossy(&output.stdout) != want_out {
            problems.push(format!("{id}: stdout differs"));
        }
        if String::from_utf8_lossy(&output.stderr) != want_err {
            problems.push(format!(
                "{id}: stderr {:?} != {:?}",
                String::from_utf8_lossy(&output.stderr),
                want_err
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "{} problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}
