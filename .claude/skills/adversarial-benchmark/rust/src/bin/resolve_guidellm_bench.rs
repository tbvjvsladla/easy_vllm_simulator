//! Generic timing harness of the GuideLLM release resolver operations. It prepares its inputs outside the timer (rows parsed,
//! argument objects built, owner configuration read) and times the whole production call inside it: the dispatch and argument
//! binding (`evaluate_operation`), the function body, every file it reads or writes (and the cleanup it does), every
//! owner it asks and every value it returns; each result is passed through `black_box` and dropped inside the timed
//! region. It knows no case: the rows are a file given on the command line, and nothing is cached between iterations.
//! Files the operations read or scan are part of that cost.
//!
//! ```text
//! resolve_guidellm_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]
//! ```
//!
//! `ROWS.jsonl` holds `{id, operation, arguments[, expected_json]}` rows (the `resolve_guidellm_api evaluate-jsonl` input, `operation` = `<owner>.<function>` or a bare name plus an `owner` field);
//! with ids the given rows are timed in that order, without them every row.
//!
//! **Observer checks.** Each prepared operation is called once untimed first (the original's harness does the same). That
//! warm call must return `Ok` and write nothing to stderr (a diagnostic is fatal here); and when the row carries
//! `expected_json` - a JSON *string* holding the expected full value as `json.dumps(value, ensure_ascii=True,
//! separators=(',', ':'))` - the returned value serialised the same way must equal it exactly. A malformed
//! `expected_json` (present but not a string) is refused. These checks run outside the timer and add no call. The timed
//! calls count every `Err` or diagnostic and, if any occurred, the process exits nonzero and prints **no** timing JSON.
//! A refusal is exit status 1 (usage errors are 2). Production code never sees `expected_json`.
//!
//! On success the output is one JSON object: the nanoseconds of the timed region per round.

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;
use sweep_stop::dataset_prep::Recorder;
use sweep_stop::guidellm_release_api::{evaluate_operation, RefusedTransport};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn fail(message: &str) -> ExitCode {
    eprintln!("[resolve_guidellm_bench] ERROR {message}");
    ExitCode::from(2)
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("[resolve_guidellm_bench] REFUSED {message}");
    ExitCode::from(1)
}

struct Prepared {
    id: String,
    operation: String,
    arguments: PyDict,
    expected_json: Option<String>,
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
        let expected_json = match row.get("expected_json") {
            None => None,
            Some(PyValue::Str(expected)) => Some(
                expected
                    .to_utf8()
                    .ok_or_else(|| "expected_json is not UTF-8".to_string())?,
            ),
            Some(other) => {
                return Err(format!(
                    "expected_json must be a string, got {}",
                    other.type_name()
                ))
            }
        };
        found.push(Prepared {
            id: text("id")?,
            operation: text("operation")?,
            arguments,
            expected_json,
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

/// `json.dumps(value, ensure_ascii=True, separators=(',', ':'))`. Scalars are written by the crate's own scalar writer
/// (separators do not touch them); only the container punctuation is compact here.
fn compact_json(value: &PyValue, out: &mut String) -> Result<(), String> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                compact_json(item, out)?;
            }
            out.push(']');
        }
        PyValue::Dict(map) => {
            out.push('{');
            for (index, (key, item)) in map.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(
                    &PyValue::Str(key.clone())
                        .dumps_ascii()
                        .map_err(|error| error.to_string())?,
                );
                out.push(':');
                compact_json(item, out)?;
            }
            out.push('}');
        }
        scalar => out.push_str(&scalar.dumps_ascii().map_err(|error| error.to_string())?),
    }
    Ok(())
}

/// The untimed warm call of one row: it must succeed without a diagnostic and, when the row says what it must return,
/// return exactly that.
fn warm(row: &Prepared) -> Result<(), String> {
    let mut diagnostics = Recorder::default();
    let outcome = evaluate_operation(
        &row.operation,
        &row.arguments,
        &mut diagnostics,
        &RefusedTransport,
    );
    let value = outcome.map_err(|failure| {
        format!(
            "{} ({}) failed: {}",
            row.id,
            row.operation,
            failure.line().to_utf8().unwrap_or_default()
        )
    })?;
    if !diagnostics.stderr.is_empty() {
        return Err(format!(
            "{} ({}) wrote a diagnostic: {}",
            row.id,
            row.operation,
            diagnostics.stderr.to_utf8().unwrap_or_default().trim_end()
        ));
    }
    if let Some(expected) = &row.expected_json {
        let mut actual = String::new();
        compact_json(&value, &mut actual)?;
        if &actual != expected {
            return Err(format!(
                "{} ({}) returned a value that differs from expected_json",
                row.id, row.operation
            ));
        }
    }
    Ok(())
}

/// One timed call: the whole production call, its result passed through `black_box` and dropped here. Returns whether
/// it was clean (`Ok` and no diagnostic); the check reads two flags and adds no work to the call.
fn call(row: &Prepared) -> bool {
    let mut diagnostics = Recorder::default();
    let outcome = black_box(evaluate_operation(
        black_box(&row.operation),
        black_box(&row.arguments),
        &mut diagnostics,
        &RefusedTransport,
    ));
    let clean = outcome.is_ok() && diagnostics.stderr.is_empty();
    drop(outcome);
    drop(black_box(diagnostics));
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
        return Err(format!(
            "{unclean} timed call(s) returned an error or a diagnostic"
        ));
    }
    Ok(timings)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [rows_path, rounds, iterations, ids @ ..] = args.as_slice() else {
        return fail("usage: resolve_guidellm_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]");
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
    for row in &prepared {
        if let Err(message) = warm(row) {
            return refuse(&message);
        }
    }
    let timings = match timed_rounds(&prepared, rounds, iterations, call) {
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

    fn row() -> Prepared {
        Prepared {
            id: "r".to_string(),
            operation: "version_from_tag".to_string(),
            arguments: PyDict::new(),
            expected_json: None,
        }
    }

    #[test]
    fn a_timed_call_that_is_not_clean_fails_the_run_and_returns_no_timings() {
        let rows = [row()];
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
    fn the_timed_call_reports_an_error_as_not_clean() {
        let mut broken = row();
        broken.operation = "no_such_operation".to_string();
        assert!(!call(&broken));
        let mut ok = row();
        ok.operation = "image_ref".to_string();
        ok.arguments.set("version", PyValue::str("1.2.3"));
        assert!(call(&ok));
    }

    #[test]
    fn compact_json_matches_python_separators() {
        let value = loads(r#"{"a": [1, 2.5, null, "é"], "b": {}, "c": []}"#).unwrap();
        let mut text = String::new();
        compact_json(&value, &mut text).unwrap();
        assert_eq!(text, r#"{"a":[1,2.5,null,"\u00e9"],"b":{},"c":[]}"#);
    }
}
