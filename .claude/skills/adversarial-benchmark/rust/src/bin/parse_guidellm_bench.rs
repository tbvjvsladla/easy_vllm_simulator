//! Warm timing harness for the `parse_guidellm` library (measurement only; no claim about other programs).
//!
//! Boundary: pre-parsed arguments in, the complete ordered result value of `build` out, created and dropped inside
//! the timer. JSON reading, request parsing and process start-up are outside it. Nothing is cached between calls
//! and no output field is left out; the result is consumed with `black_box`.
//!
//! Usage: `parse_guidellm_bench --cases API.jsonl --ids id1,id2,... [--rounds 11] [--iterations 200]`;
//! one JSON document on stdout. Cases are the `build` rows of the API observation file (`{id, operation,
//! arguments}`); a row that is not a `build` row, or whose result is an exception, is an error (the timed
//! population is value cases only).

use std::hint::black_box;
use std::time::Instant;
use sweep_stop::guidellm::{build, BuildRequest};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;

struct Prepared {
    doc: PyValue,
    request: BuildRequest,
}

fn prepare(arguments: &PyValue) -> Prepared {
    let PyValue::Dict(map) = arguments else {
        panic!("arguments must be an object")
    };
    let defaults = BuildRequest::default();
    let raw = |name: &str, default: PyValue| map.get(name).cloned().unwrap_or(default);
    Prepared {
        doc: map.get("doc").cloned().expect("doc"),
        request: BuildRequest {
            benchmark_index: raw("benchmark_index", defaults.benchmark_index),
            engine_max: raw("engine_max", defaults.engine_max),
            spec: raw("spec", defaults.spec),
            max_error_rate: raw("max_error_rate", defaults.max_error_rate),
            server_alive_at_bench_end: raw(
                "server_alive_at_bench_end",
                defaults.server_alive_at_bench_end,
            ),
        },
    }
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let len = sorted.len();
    if len % 2 == 1 {
        sorted[len / 2]
    } else {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    }
}

fn main() {
    let (mut cases, mut ids, mut rounds, mut iterations) =
        (String::new(), String::new(), 11usize, 200usize);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .unwrap_or_else(|| panic!("{flag} needs a value"));
        match flag.as_str() {
            "--cases" => cases = value,
            "--ids" => ids = value,
            "--rounds" => rounds = value.parse().expect("rounds"),
            "--iterations" => iterations = value.parse().expect("iterations"),
            other => panic!("unknown argument {other}"),
        }
    }
    let wanted: Vec<&str> = ids.split(',').filter(|id| !id.is_empty()).collect();
    let text = std::fs::read_to_string(&cases).expect("cases file");
    let mut prepared = Vec::new();
    for line in text.lines() {
        let PyValue::Dict(row) = loads(line).expect("JSON line") else {
            continue;
        };
        let id = match row.get("id") {
            Some(PyValue::Str(id)) => id.to_utf8().unwrap_or_default(),
            _ => continue,
        };
        if wanted.contains(&id.as_str()) {
            let operation =
                matches!(row.get("operation"), Some(PyValue::Str(op)) if op.equals("build"));
            assert!(operation, "{id} is not a build row");
            let case = prepare(row.get("arguments").expect("arguments"));
            assert!(
                build(&case.doc, &case.request).is_ok(),
                "{id} does not produce a value"
            );
            prepared.push(case);
        }
    }
    assert_eq!(
        prepared.len(),
        wanted.len(),
        "every requested case is present"
    );
    // One warm traversal of every case before anything is timed.
    for case in &prepared {
        black_box(build(&case.doc, &case.request).expect("value"));
    }
    let per_round: Vec<f64> = (0..rounds)
        .map(|_| {
            let started = Instant::now();
            for _ in 0..iterations {
                for case in &prepared {
                    black_box(
                        build(black_box(&case.doc), black_box(&case.request)).expect("value"),
                    );
                }
            }
            started.elapsed().as_nanos() as f64 / (iterations * prepared.len()) as f64
        })
        .collect();
    let list: Vec<String> = per_round.iter().map(|v| format!("{v:.1}")).collect();
    println!(
        "{{\"cases\": {}, \"rounds\": {rounds}, \"iterations\": {iterations}, \"unit\": \"ns/value\", \"median\": {:.1}, \"round_values\": [{}]}}",
        prepared.len(),
        median(&per_round),
        list.join(", ")
    );
}
