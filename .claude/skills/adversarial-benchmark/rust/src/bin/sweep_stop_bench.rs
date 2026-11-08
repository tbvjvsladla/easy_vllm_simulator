//! Warm-evaluation timing harness for `sweep_stop` (measurement only; it makes no claim about other programs).
//!
//! The reference protocol is 11 rounds of 200 evaluations over one 100-cell state. Boundaries are timed as
//! separate series so process start-up and JSON handling never hide inside the compute figure:
//!
//! * `evaluate_typed` - the pure decision returning the typed `Evaluation`, result consumed with `black_box`;
//! * `evaluate_to_value` - the same decision plus building the complete ordered value (the counterpart of the
//!   dict Python's `evaluate` returns), result consumed with `black_box`;
//! * `parse_evaluate_render` - parse the state text, evaluate, render the verdict JSON (in process);
//! * `process` - spawn the sibling `sweep_stop` binary (`--spawn N`, off by default).
//!
//! Usage: `sweep_stop_bench [--rounds 11] [--iterations 200] [--cells 100] [--spawn 0]`; one JSON document on stdout.

use std::hint::black_box;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;
use sweep_stop::{evaluate, parse_utc};

const NOW: &str = "2026-09-04T09:30:00Z";

struct Settings {
    rounds: usize,
    iterations: usize,
    cells: usize,
    spawn: usize,
}

fn parse_settings() -> Result<Settings, String> {
    let mut settings = Settings {
        rounds: 11,
        iterations: 200,
        cells: 100,
        spawn: 0,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let slot = match flag.as_str() {
            "--rounds" => &mut settings.rounds,
            "--iterations" => &mut settings.iterations,
            "--cells" => &mut settings.cells,
            "--spawn" => &mut settings.spawn,
            other => return Err(format!("unknown argument {other}")),
        };
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        *slot = value
            .parse()
            .map_err(|_| format!("{flag} expects a non-negative integer, got {value}"))?;
    }
    Ok(settings)
}

fn state_text(cells: usize) -> String {
    let cell_list: Vec<String> = (0..cells)
        .map(|index| {
            format!(
                "{{\"cell_key\": \"{index}\", \"cell_outcome\": \"measured\", \"repetition\": {{\"requested\": 3, \"runs_attempted\": 3}}}}"
            )
        })
        .collect();
    format!(
        "{{\"declared_budget\": {{\"max_cells\": 6, \"wall_clock_budget_s\": 3600, \"consecutive_failure_limit\": 3, \
         \"repeats\": 3, \"repeats_source\": \"declared(reference fixture)\", \"declared_by\": \"external-reference\", \
         \"basis\": \"compatibility capture; no actual serving\"}}, \"cells_planned\": 6, \
         \"started_utc\": \"2026-09-04T09:00:00Z\", \"cells\": [{}], \"cells_remaining\": [\"next\"]}}",
        cell_list.join(", ")
    )
}

/// Nanoseconds per call for each round of `iterations` calls.
fn time_rounds(rounds: usize, iterations: usize, mut work: impl FnMut()) -> Vec<f64> {
    (0..rounds)
        .map(|_| {
            let started = Instant::now();
            for _ in 0..iterations {
                work();
            }
            started.elapsed().as_nanos() as f64 / iterations as f64
        })
        .collect()
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    match sorted.len() {
        0 => f64::NAN,
        len if len % 2 == 1 => sorted[len / 2],
        len => (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0,
    }
}

fn series_json(name: &str, unit: &str, samples: &[f64]) -> String {
    let list: Vec<String> = samples
        .iter()
        .map(|sample| format!("{sample:.3}"))
        .collect();
    format!(
        "  \"{name}\": {{\"unit\": \"{unit}\", \"median\": {:.3}, \"rounds\": [{}]}}",
        median(samples),
        list.join(", ")
    )
}

fn spawn_once(binary: &std::path::Path, text: &str) {
    let mut child = Command::new(binary)
        .args(["--state", "-", "--now-utc", NOW])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sweep_stop");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(text.as_bytes())
        .expect("write state");
    child.wait().expect("wait for sweep_stop");
}

fn main() {
    let settings = match parse_settings() {
        Ok(settings) => settings,
        Err(message) => {
            eprintln!("sweep_stop_bench: {message}");
            std::process::exit(2);
        }
    };
    let text = state_text(settings.cells);
    let state = loads(&text).expect("generated state is valid JSON");
    let now = parse_utc(&PyValue::str(NOW), "--now-utc").expect("fixed timestamp is valid");
    black_box(evaluate(&state, &now).expect("generated state is valid"));

    let evaluate_typed = time_rounds(settings.rounds, settings.iterations, || {
        black_box(evaluate(black_box(&state), black_box(&now)).expect("valid verdict"));
    });
    let evaluate_to_value = time_rounds(settings.rounds, settings.iterations, || {
        black_box(
            evaluate(black_box(&state), black_box(&now))
                .expect("valid verdict")
                .to_value(),
        );
    });
    let in_process = time_rounds(settings.rounds, settings.iterations, || {
        let state = loads(black_box(&text)).expect("valid state");
        let verdict = evaluate(&state, &now).expect("valid verdict");
        black_box(
            verdict
                .to_value()
                .dumps_pretty()
                .expect("encodable verdict"),
        );
    });

    let mut sections = vec![
        format!("  \"cells\": {}", settings.cells),
        format!("  \"rounds\": {}", settings.rounds),
        format!("  \"iterations_per_round\": {}", settings.iterations),
        series_json("evaluate_typed", "ns_per_call", &evaluate_typed),
        series_json("evaluate_to_value", "ns_per_call", &evaluate_to_value),
        series_json("parse_evaluate_render", "ns_per_call", &in_process),
    ];
    if settings.spawn > 0 {
        let binary = std::env::current_exe()
            .expect("current exe")
            .with_file_name("sweep_stop");
        let process = time_rounds(settings.rounds, settings.spawn, || {
            spawn_once(&binary, &text)
        });
        sections.push(series_json("process", "ns_per_spawn", &process));
    }
    println!("{{\n{}\n}}", sections.join(",\n"));
}
