//! The frozen post-quality measurement of the model scripts' functions: preflight, measured loops and the buffered report.
//!
//! The fixture rows carry the operation, its arguments and the **expected whole value** taken from a saved original result.
//! Before any loop runs, every case is called once outside the timer and its complete value is compared (types, tuples, key
//! order, contents) with the expected one; a failure or a difference ends the run. In a measured loop each call's complete value
//! is kept alive with `black_box`, and a failing call ends the run. All series and the header are buffered: nothing is
//! reported unless every series succeeded. The calls are the real functions: the arguments are bound at each call and a
//! function that reads files opens, stats and reads them each time (nothing is cached).

use crate::model_api::{encode, resolve, ModelFunction};
use crate::model_card::HF_API_BASE;
use crate::observe::same_value;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::hint::black_box;
use std::time::Instant;

/// One measured function call with the value it must produce.
pub struct Case {
    pub metric: String,
    function: &'static ModelFunction,
    args: Vec<PyValue>,
    kwargs: PyDict,
    pub expected: PyValue,
}

impl Case {
    fn call(&mut self) -> Res<PyValue> {
        self.function
            .call(&mut self.args, &mut self.kwargs, HF_API_BASE)
    }
}

fn benchmark_error(message: String) -> Failure {
    Failure::new("BenchmarkError", &message)
}

fn text_of(members: &PyDict, key: &str) -> Res<String> {
    match members.get(key) {
        Some(PyValue::Str(own)) => own
            .to_utf8()
            .ok_or_else(|| benchmark_error(format!("{key} is not text"))),
        _ => Err(benchmark_error(format!(
            "a fixture row has no text member {key}"
        ))),
    }
}

/// Reads the fixture: a JSON array of `{metric, operation, args, kwargs, expected, source_id}`, parsed once.
pub fn load(source: &str) -> Res<Vec<Case>> {
    let PyValue::List(rows) =
        loads(source).map_err(|error| benchmark_error(format!("fixture is not JSON: {error}")))?
    else {
        return Err(benchmark_error("the fixture is not an array".to_string()));
    };
    let mut cases = Vec::new();
    for row in rows {
        let PyValue::Dict(members) = row else {
            return Err(benchmark_error(
                "a fixture row is not an object".to_string(),
            ));
        };
        let metric = text_of(&members, "metric")?;
        let function = resolve(&text_of(&members, "operation")?)?;
        let args = match members.get("args") {
            Some(PyValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let kwargs = match members.get("kwargs") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        cases.push(Case {
            metric,
            function,
            args,
            kwargs,
            expected,
        });
    }
    Ok(cases)
}

/// The preflight, the measured loops and the report text; an `Err` means no report exists.
pub fn run_cases(cases: &mut [Case], rounds: usize, iterations: usize) -> Res<String> {
    for case in cases.iter_mut() {
        let value = case.call().map_err(|failure| {
            benchmark_error(format!(
                "{} fails before measurement: {}",
                case.metric,
                failure.line().to_utf8().unwrap_or_default()
            ))
        })?;
        let value = encode(&value);
        if !same_value(&case.expected, &value) {
            return Err(benchmark_error(format!(
                "{}: the value differs from the saved original result: {}",
                case.metric,
                value.repr().chars().take(160).collect::<String>()
            )));
        }
    }
    let mut report = format!("{{\"rounds\": {rounds}, \"iterations\": {iterations}}}\n");
    for case in cases.iter_mut() {
        let mut samples = Vec::with_capacity(rounds);
        for _ in 0..rounds {
            let started = Instant::now();
            for _ in 0..iterations {
                match case.call() {
                    Ok(value) => {
                        black_box(value);
                    }
                    Err(failure) => {
                        return Err(benchmark_error(format!(
                            "{} failed inside a measured loop: {}",
                            case.metric,
                            failure.line().to_utf8().unwrap_or_default()
                        )));
                    }
                }
            }
            samples.push(started.elapsed().as_nanos() as f64 / iterations.max(1) as f64);
        }
        let mut sorted = samples.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let median = sorted[sorted.len() / 2];
        let shown: Vec<String> = samples
            .iter()
            .map(|sample| format!("{sample:.1}"))
            .collect();
        report.push_str(&format!(
            "{{\"metric\": \"{}\", \"unit\": \"ns_per_call\", \"median\": {median:.1}, \"rounds\": [{}], \"round_count\": {rounds}, \"iterations\": {iterations}}}\n",
            case.metric,
            shown.join(", ")
        ));
    }
    Ok(report)
}
