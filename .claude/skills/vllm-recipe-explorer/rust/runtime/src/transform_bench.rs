//! The frozen post-quality measurement of the two transform functions: preflight, measured loops and the buffered report (the generic
//! harness of [`crate::bench`]).
//!
//! A fixture row is `{metric, operation, args, kwargs, expected}` with `operation` one of `rope_scaling_translate.translate` and
//! `parse_vllm_log.parse_log`, and `expected` the **whole value** of a saved original result. Before any timer every row is called once
//! and its complete value (types, key order) is compared with the expected one; a failing call or a difference ends the run. Inside a
//! measured loop each call creates its complete result and drops it (nothing accumulates); a failing call ends the run. The measured
//! unit is the public function, argument binding included; the fixture's arguments are parsed once, before any timer, and every call
//! is handed the same decoded arguments (translation never changes them). Nothing is reported unless every series succeeded.

use crate::bench::Case;
use crate::pyexpr::Res;
use crate::rope_translate::Profile;
use crate::transform_api::{call, resolve};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub use crate::bench::run_cases;

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

/// Reads the fixture: a JSON array of `{metric, operation, args, kwargs, expected}`, parsed once.
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
        let function = resolve(&text_of(&members, "operation")?).map_err(|raised| {
            benchmark_error(format!(
                "{metric}: {}",
                raised.report().to_utf8().unwrap_or_default().trim_end()
            ))
        })?;
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
        let profile = Profile::canonical();
        cases.push(Case {
            metric,
            call: Box::new(move || {
                call(function, &profile, &args, &kwargs)
                    .map(PyValue::Dict)
                    .map_err(|raised| Failure::with_message("BenchmarkError", raised.report()))
            }),
            expected,
        });
    }
    Ok(cases)
}
