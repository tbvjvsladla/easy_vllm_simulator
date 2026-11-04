//! Generic timing harness of the classifier operations. It prepares its inputs outside the timer (rows parsed,
//! arguments bound, owner configuration read) and times the whole operation inside it: the function body, every file
//! it reads or writes (and the cleanup it does), every owner it asks and every value it returns are inside; each
//! result is passed through `black_box` and dropped inside the timed region. It knows no case: the rows are a file
//! given on the command line, and nothing is cached between iterations.
//!
//! ```text
//! classify_cell_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]
//! ```
//!
//! `ROWS.jsonl` holds `{id, operation, arguments}` rows (the `classify_cell_api evaluate-jsonl` input); with ids the
//! given rows are timed in that order, without them every row. Each prepared operation is called once untimed first
//! (the original's harness does the same). The output is one JSON object: the nanoseconds of the timed region per
//! round. Operations that ask the not-yet-ported owners (`node_identity.sh`, `node_role_contract`) include that
//! latency; it is `OPEN_S3` and is not a property of this port.

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;
use sweep_stop::classify_cell::Diagnostics;
use sweep_stop::classify_cell_api_cli::evaluate_operation;
use sweep_stop::classify_cell_owner::Owners;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn fail(message: &str) -> ExitCode {
    eprintln!("[classify_cell_bench] ERROR {message}");
    ExitCode::from(2)
}

struct Prepared {
    operation: String,
    arguments: PyDict,
}

fn prepare(rows: &str, wanted: &[String]) -> Result<Vec<Prepared>, String> {
    let mut found: Vec<(String, Prepared)> = Vec::new();
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
        found.push((
            text("id")?,
            Prepared {
                operation: text("operation")?,
                arguments,
            },
        ));
    }
    if wanted.is_empty() {
        return Ok(found.into_iter().map(|(_, row)| row).collect());
    }
    wanted
        .iter()
        .map(|id| {
            found
                .iter()
                .position(|(known, _)| known == id)
                .map(|at| {
                    let (_, row) = found.swap_remove(at);
                    row
                })
                .ok_or_else(|| format!("no row {id}"))
        })
        .collect()
}

fn call(owners: &Owners, row: &Prepared) {
    let mut diagnostics = Diagnostics::default();
    drop(black_box(evaluate_operation(
        black_box(&row.operation),
        black_box(&row.arguments),
        black_box(owners),
        &mut diagnostics,
    )));
    drop(black_box(diagnostics));
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [rows_path, rounds, iterations, ids @ ..] = args.as_slice() else {
        return fail("usage: classify_cell_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]");
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
        Err(message) => return fail(&message),
    };
    let owners = Owners::from_environment();
    for row in &prepared {
        call(&owners, row);
    }
    let mut timings = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let start = Instant::now();
        for _ in 0..iterations {
            for row in &prepared {
                call(&owners, row);
            }
        }
        timings.push(start.elapsed().as_nanos());
    }
    let list: Vec<String> = timings.iter().map(u128::to_string).collect();
    println!(
        "{{\"cases\": {}, \"iterations\": {iterations}, \"rounds_ns\": [{}]}}",
        prepared.len(),
        list.join(", ")
    );
    ExitCode::SUCCESS
}
