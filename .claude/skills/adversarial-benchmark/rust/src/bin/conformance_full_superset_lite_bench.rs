//! Generic timing harness of the conformance checker operations. It prepares its inputs outside the timer (rows parsed,
//! argument objects built, the scripts directory read from the environment once) and times the whole production
//! call inside it: the dispatch and argument binding (`evaluate_operation`), the function body, every file it reads,
//! every real child process it starts (the installed launchers and the binaries behind them), the output it
//! generates into a capture sink (the equivalent of Python's `StringIO`: the text is built and kept, not
//! discarded) and every value it returns. Each result is passed through `black_box` and dropped inside the timed
//! region. It knows no case: the rows are a file given on the command line, and nothing is cached between iterations.
//!
//! ```text
//! conformance_full_superset_lite_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]
//! ```
//!
//! `ROWS.jsonl` holds `{id, operation, arguments, expected_json, expected_stdout}` rows (the
//! `conformance_full_superset_lite_api evaluate-jsonl` input, `operation` a bare function name); with ids the given
//! rows are timed in that order, without them every row. The scripts directory is the environment variable
//! `CONFORMANCE_SCRIPTS_DIR`, fixed for the process.
//!
//! **Observer checks.** Every row carries `expected_json` - a JSON *string* holding the expected full value as
//! `json.dumps(value, ensure_ascii=True, separators=(',', ':'))` - and `expected_stdout` - a JSON string holding the
//! whole text the operation printed (`""` for an operation that prints nothing). A row without either, or with one
//! that is not a string, is refused while preparing. Each prepared operation is called once untimed first; that warm
//! call must return a value (an exception refuses the run), the value serialised the same way must equal
//! `expected_json` and the captured printed text must equal `expected_stdout`, both exactly. These checks run outside
//! the timer and add no call. The timed calls count every exception and, if any occurred, the process exits nonzero
//! and prints **no** timing JSON. A refusal is exit status 1 (usage errors are 2). Production code never sees the
//! expectations.
//!
//! On success the output is one JSON object: the nanoseconds of the timed region per round.

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;
use sweep_stop::conformance::Context;
use sweep_stop::conformance_api::{compact_json, evaluate_operation};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn fail(message: &str) -> ExitCode {
    eprintln!("[conformance_full_superset_lite_bench] ERROR {message}");
    ExitCode::from(2)
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("[conformance_full_superset_lite_bench] REFUSED {message}");
    ExitCode::from(1)
}

#[derive(Debug)]
struct Prepared {
    id: String,
    operation: String,
    arguments: PyDict,
    expected_json: String,
    expected_stdout: String,
}

/// An expectation every row must carry, as a JSON string.
fn expected_text(row: &PyDict, key: &str) -> Result<String, String> {
    match row.get(key) {
        None => Err(format!("a row has no {key}")),
        Some(PyValue::Str(text)) => text.to_utf8().ok_or_else(|| format!("{key} is not UTF-8")),
        Some(other) => Err(format!("{key} must be a string, got {}", other.type_name())),
    }
}

fn prepare(rows: &str, wanted: &[String]) -> Result<Vec<Prepared>, String> {
    let mut found: Vec<Prepared> = Vec::new();
    for line in rows.lines().filter(|line| !line.trim().is_empty()) {
        let PyValue::Dict(row) = loads(line).map_err(|error| error.to_string())? else {
            return Err("a row is not an object".to_string());
        };
        let text = |key: &str| match row.get(key) {
            Some(PyValue::Str(text)) => text.to_utf8().ok_or_else(|| format!("{key} is not UTF-8")),
            _ => Err(format!("a row has no string {key}")),
        };
        let arguments = match row.get("arguments") {
            Some(PyValue::Dict(arguments)) => arguments.clone(),
            _ => return Err("a row has no arguments object".to_string()),
        };
        found.push(Prepared {
            id: text("id")?,
            operation: text("operation")?,
            arguments,
            expected_json: expected_text(&row, "expected_json")?,
            expected_stdout: expected_text(&row, "expected_stdout")?,
        });
    }
    if wanted.is_empty() {
        return Ok(found);
    }
    wanted
        .iter()
        .map(|id| {
            found
                .iter()
                .position(|known| &known.id == id)
                .map(|at| found.swap_remove(at))
                .ok_or_else(|| format!("no row {id}"))
        })
        .collect()
}

/// The untimed warm call of one row: it must return a value and, when the row says what it must return or print,
/// exactly that.
fn warm(row: &Prepared, context: Option<&Context>) -> Result<(), String> {
    let evaluation = evaluate_operation(&row.operation, &row.arguments, context);
    let value = evaluation.result.map_err(|failure| {
        format!(
            "{} ({}) failed: {}",
            row.id,
            row.operation,
            failure.line().to_utf8().unwrap_or_default()
        )
    })?;
    let mut actual = String::new();
    compact_json(&value, &mut actual)?;
    if actual != row.expected_json {
        return Err(format!(
            "{} ({}) returned a value that differs from expected_json",
            row.id, row.operation
        ));
    }
    // an operation that prints nothing (`read`, `native_value`, the gates check) is expected to print ""
    let printed = evaluation
        .stdout
        .map_or(Some(String::new()), |text| text.to_utf8());
    if printed.as_deref() != Some(row.expected_stdout.as_str()) {
        return Err(format!(
            "{} ({}) printed text that differs from expected_stdout",
            row.id, row.operation
        ));
    }
    Ok(())
}

/// One timed call: the whole production call, its result and captured output passed through `black_box` and dropped
/// here. Returns whether it was clean (a value, no exception).
fn call(row: &Prepared, context: Option<&Context>) -> bool {
    let evaluation = black_box(evaluate_operation(
        black_box(&row.operation),
        black_box(&row.arguments),
        black_box(context),
    ));
    let clean = evaluation.result.is_ok();
    drop(evaluation);
    clean
}

/// The timed rounds. Any call that was not clean makes the whole run fail (no timings are returned).
fn timed_rounds<F: Fn(&Prepared) -> bool>(
    rows: &[Prepared],
    rounds: usize,
    iterations: usize,
    call: F,
) -> Result<Vec<u128>, String> {
    let mut timings = Vec::with_capacity(rounds);
    let mut unclean = 0_usize;
    for _ in 0..rounds {
        let start = Instant::now();
        for _ in 0..iterations {
            for row in rows {
                unclean += usize::from(!call(row));
            }
        }
        timings.push(start.elapsed().as_nanos());
    }
    if unclean > 0 {
        return Err(format!("{unclean} timed call(s) returned an exception"));
    }
    Ok(timings)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [rows_path, rounds, iterations, ids @ ..] = args.as_slice() else {
        return fail(
            "usage: conformance_full_superset_lite_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]",
        );
    };
    let (Ok(rounds), Ok(iterations)) = (rounds.parse::<usize>(), iterations.parse::<usize>())
    else {
        return fail("ROUNDS and ITERATIONS must be non-negative integers");
    };
    let rows = match std::fs::read_to_string(rows_path) {
        Ok(rows) => rows,
        Err(error) => return fail(&format!("{rows_path}: {error}")),
    };
    // Rows with duplicate ids in `ids` are not supported: each id selects one row.
    let prepared = match prepare(&rows, ids) {
        Ok(prepared) => prepared,
        Err(message) => return refuse(&message),
    };
    // The scripts directory of the process: read once, outside the timer.
    let context = Context::from_environment();
    for row in &prepared {
        if let Err(message) = warm(row, context.as_ref()) {
            return refuse(&message);
        }
    }
    let timings = match timed_rounds(&prepared, rounds, iterations, |row| {
        call(row, context.as_ref())
    }) {
        Ok(timings) => timings,
        Err(message) => return refuse(&message),
    };
    let list: Vec<String> = timings.iter().map(u128::to_string).collect();
    println!(
        "{{\"cases\": {}, \"iterations\": {iterations}, \"rounds_ns\": [{}]}}",
        prepared.len(),
        list.join(", ")
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(operation: &str) -> Prepared {
        Prepared {
            id: "r".to_string(),
            operation: operation.to_string(),
            arguments: PyDict::new(),
            expected_json: "true".to_string(),
            expected_stdout: String::new(),
        }
    }

    #[test]
    fn a_timed_call_that_is_not_clean_fails_the_run_and_returns_no_timings() {
        let rows = [row("_emit")];
        let seen = std::cell::Cell::new(0);
        let flaky = |_: &Prepared| {
            seen.set(seen.get() + 1);
            seen.get() != 3
        };
        assert!(timed_rounds(&rows, 2, 3, flaky).is_err());
        assert_eq!(seen.get(), 6, "every timed call still runs");
        assert_eq!(timed_rounds(&rows, 2, 3, |_| true).map(|t| t.len()), Ok(2));
    }

    #[test]
    fn the_timed_call_reports_an_exception_as_not_clean() {
        assert!(!call(&row("no_such_operation"), None));
        let mut emit = row("_emit");
        emit.arguments
            .set("results", loads(r#"[["a", true, ""]]"#).unwrap());
        assert!(call(&emit, None));
    }

    #[test]
    fn the_warm_call_checks_value_and_printed_text() {
        let mut emit = row("_emit");
        emit.arguments
            .set("results", loads(r#"[["a", true, ""]]"#).unwrap());
        emit.expected_stdout = "PASS a\n\nfull ⊇ lite conformance: 1/1 PASS\n".to_string();
        assert_eq!(warm(&emit, None), Ok(()));
        emit.expected_stdout = "other".to_string();
        assert!(warm(&emit, None).unwrap_err().contains("expected_stdout"));
        emit.expected_stdout = "PASS a\n\nfull ⊇ lite conformance: 1/1 PASS\n".to_string();
        emit.expected_json = "false".to_string();
        assert!(warm(&emit, None).unwrap_err().contains("expected_json"));
        assert!(warm(&row("no_such_operation"), None).is_err());
    }

    #[test]
    fn a_missing_or_malformed_expectation_is_refused_while_preparing() {
        let wrong = r#"{"id": "a", "operation": "_emit", "arguments": {}, "expected_json": "true", "expected_stdout": 1}"#;
        assert!(prepare(wrong, &[])
            .unwrap_err()
            .contains("expected_stdout must be a string"));
        let no_stdout =
            r#"{"id": "a", "operation": "_emit", "arguments": {}, "expected_json": "true"}"#;
        assert!(prepare(no_stdout, &[])
            .unwrap_err()
            .contains("a row has no expected_stdout"));
        let no_json =
            r#"{"id": "a", "operation": "_emit", "arguments": {}, "expected_stdout": ""}"#;
        assert!(prepare(no_json, &[])
            .unwrap_err()
            .contains("a row has no expected_json"));
    }
}
