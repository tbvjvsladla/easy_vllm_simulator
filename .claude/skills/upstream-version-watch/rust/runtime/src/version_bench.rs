//! The frozen measurement of the two ported `resolve_torch_pin` functions (`ABI.md`, benchmark fixture): the rows are `{metric, operation, calls:
//! [{args_dto, kwargs_dto, expected: {value, input_ref}}], iterations?}`.
//!
//! The boundary is the one the original's actor measures (`r35_benchmark_original.py`):
//! * **Before any timer** the fixture is decoded once and EVERY call of every row is run exactly once through the same typed business function the normal
//!   service runs; its whole value DTO and its origin path are compared with the saved original (`expected`), and any difference, or a call that raises, is
//!   refused here: nothing is timed and nothing is reported. (That is one real helper call per fixture call, 45 for the frozen fixture; the measuring loop
//!   below does not run a second preflight.)
//! * **Timed**: each iteration runs the row's calls in order through the business function and builds the list of the RAW typed returns (the Python value
//!   and, where the helper itself makes it, its origin) and drops that list inside the timer. No DTO is encoded, no origin path is turned into a value, no
//!   response dictionary or JSON is made, no expected value is read and no alias comparison is made in the timed region (see the source markers
//!   `TIMED-PATH-BEGIN` / `TIMED-PATH-END`, which a check scans).
//! * The report is written only after every series passed: `rounds` rounds per row of its own iteration count (else the process's), one `ns_per_call` being
//!   one whole row iteration (the 44 calls of the batch row, the single call of the other).

use crate::bench::{benchmark_error, iterations_of, text_of};
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::torch_pin::{Raised, Returned};
use crate::version_api::{resolve_operation, typed_result, Function};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::observe::same_value;
use recipe_runtime::pyexpr::Res;
use std::hint::black_box;
use std::time::Instant;

struct Input {
    args: Vec<PyValue>,
    kwargs: PyDict,
}

/// One fixture row, decoded once.
pub struct Row {
    pub metric: String,
    function: Function,
    inputs: Vec<Input>,
    /// For each call: the saved value DTO and the saved origin path (`null` or a path).
    expected: Vec<(PyValue, PyValue)>,
    iterations: Option<usize>,
}

impl Row {
    /// The number of function calls of one iteration of the row.
    pub fn calls(&self) -> usize {
        self.inputs.len()
    }

    /// The iterations of each round: the row's own count, else `default`.
    pub fn iterations_or(&self, default: usize) -> usize {
        self.iterations.unwrap_or(default)
    }
}

/// Reads the fixture: a JSON array of rows, decoded once.
pub fn load(source: &str) -> Res<Vec<Row>> {
    let PyValue::List(rows) =
        loads(source).map_err(|error| benchmark_error(format!("fixture is not JSON: {error}")))?
    else {
        return Err(benchmark_error("the fixture is not an array".to_string()));
    };
    let mut out = Vec::new();
    for row in rows {
        let PyValue::Dict(members) = row else {
            return Err(benchmark_error(
                "a fixture row is not an object".to_string(),
            ));
        };
        let metric = text_of(&members, "metric")?;
        let function = resolve_operation(&text_of(&members, "operation")?).map_err(|raised| {
            benchmark_error(format!(
                "{metric}: {}",
                raised.rows[0].message.to_utf8().unwrap_or_default()
            ))
        })?;
        let Some(PyValue::List(calls)) = members.get("calls") else {
            return Err(benchmark_error(format!("{metric}: the row has no calls")));
        };
        if calls.is_empty() {
            return Err(benchmark_error(format!("{metric}: the row has no calls")));
        }
        let mut inputs = Vec::new();
        let mut expected = Vec::new();
        for call in calls {
            let PyValue::Dict(call) = call else {
                return Err(benchmark_error(format!(
                    "{metric}: a call is not an object"
                )));
            };
            let args = match call.get("args_dto") {
                None => Vec::new(),
                Some(dto) => decode_args(dto)
                    .map_err(|problem| benchmark_error(format!("{metric}: args_dto: {problem}")))?,
            };
            let kwargs = match call.get("kwargs_dto") {
                None => PyDict::new(),
                Some(dto) => decode_kwargs(dto).map_err(|problem| {
                    benchmark_error(format!("{metric}: kwargs_dto: {problem}"))
                })?,
            };
            let Some(PyValue::Dict(want)) = call.get("expected") else {
                return Err(benchmark_error(format!(
                    "{metric}: a call has no expected {{value, input_ref}}"
                )));
            };
            let (Some(value), Some(input_ref)) = (want.get("value"), want.get("input_ref")) else {
                return Err(benchmark_error(format!(
                    "{metric}: expected needs value and input_ref"
                )));
            };
            expected.push((value.clone(), input_ref.clone()));
            inputs.push(Input { args, kwargs });
        }
        let iterations = iterations_of(&members, &metric)?;
        out.push(Row {
            metric,
            function,
            inputs,
            expected,
            iterations,
        });
    }
    Ok(out)
}

fn loop_failure(metric: &str, raised: &Raised) -> Failure {
    benchmark_error(format!(
        "{metric} failed inside a measured loop: {}",
        raised.rows[0].message.to_utf8().unwrap_or_default()
    ))
}

/// The preflight: every call of every row exactly once, before any timer; the whole value DTO and the origin path must equal the saved ones.
pub fn preflight(rows: &[Row]) -> Res<()> {
    for row in rows {
        for (at, (input, (want_value, want_ref))) in
            row.inputs.iter().zip(&row.expected).enumerate()
        {
            let returned = row
                .function
                .call(&input.args, &input.kwargs)
                .map_err(|raised| {
                    benchmark_error(format!(
                        "{} call {at} fails before measurement: {}",
                        row.metric,
                        raised.rows[0].message.to_utf8().unwrap_or_default()
                    ))
                })?;
            let (value, origin) = typed_result(&returned).map_err(benchmark_error)?;
            if !same_value(want_value, &value) {
                return Err(benchmark_error(format!(
                    "{} call {at}: the value differs from the saved original result: {}",
                    row.metric,
                    value.repr().chars().take(160).collect::<String>()
                )));
            }
            if !same_value(want_ref, &origin) {
                return Err(benchmark_error(format!(
                    "{} call {at}: the origin differs from the saved original: {}",
                    row.metric,
                    origin.repr().chars().take(160).collect::<String>()
                )));
            }
        }
    }
    Ok(())
}

/// The measuring loops and the report text (no preflight here: [`run_rows`] does it once before this).
fn measure(rows: &[Row], rounds: usize, default_iterations: usize) -> Res<String> {
    let mut report = format!("{{\"rounds\": {rounds}, \"iterations\": {default_iterations}}}\n");
    for row in rows {
        let iterations = row.iterations_or(default_iterations);
        let mut samples = Vec::with_capacity(rounds);
        for _ in 0..rounds {
            // TIMED-PATH-BEGIN
            let started = Instant::now();
            for _ in 0..iterations {
                let mut results: Vec<Returned> = Vec::with_capacity(row.inputs.len());
                for input in &row.inputs {
                    match row.function.call(&input.args, &input.kwargs) {
                        Ok(returned) => results.push(returned),
                        Err(raised) => return Err(loop_failure(&row.metric, &raised)),
                    }
                }
                drop(black_box(results));
            }
            let elapsed = started.elapsed();
            // TIMED-PATH-END
            samples.push(elapsed.as_nanos() as f64 / iterations as f64);
        }
        let mut sorted = samples.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let median = sorted[sorted.len() / 2];
        let shown: Vec<String> = samples
            .iter()
            .map(|sample| format!("{sample:.1}"))
            .collect();
        let metric = PyValue::str(&row.metric)
            .dumps_ascii()
            .map_err(|error| benchmark_error(format!("cannot encode a metric name: {error}")))?;
        report.push_str(&format!(
            "{{\"metric\": {metric}, \"unit\": \"ns_per_call\", \"median\": {median:.1}, \"rounds\": [{}], \"round_count\": {rounds}, \"iterations\": {iterations}}}\n",
            shown.join(", ")
        ));
    }
    Ok(report)
}

/// The preflight (once per call, before any timer), then the measurement; an `Err` means no report exists.
pub fn run_rows(rows: &[Row], rounds: usize, default_iterations: usize) -> Res<String> {
    preflight(rows)?;
    measure(rows, rounds, default_iterations)
}
