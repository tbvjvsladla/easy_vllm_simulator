//! The frozen post-quality measurement of the functional smoke operations: preflight, measured loops and the buffered report (the generic
//! harness of [`crate::bench`]).
//!
//! A fixture row is `{metric, operation, args, kwargs, globals?, expected}` with `operation` one of the six `functional_smoke.*`
//! operations and `expected` the **whole value** of a saved original result. Before any timer every row is called once (a real call: an HTTP
//! row makes its real requests to the base URL in its arguments, a mock row evaluates the real mock path) and its complete value (types,
//! key order) is compared with the expected one; a failing call or a difference ends the run. Each measured iteration is the whole public
//! call, its complete value created and dropped inside the timer; nothing is cached or traced. Nothing is reported unless every series
//! succeeded. The requests the peer saw are the caller's to check, outside the timers.

use crate::bench::Case;
use crate::functional_api::resolve;
use crate::functional_smoke::{call, Constants, Plain};
use crate::pyexpr::Res;
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

/// Reads the fixture: a JSON array of rows, parsed once.
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
        let mut constants = Constants::canonical();
        if let Some(given) = members.get("globals") {
            constants
                .replace_from(given)
                .map_err(|problem| benchmark_error(format!("{metric}: {problem}")))?;
        }
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        cases.push(Case {
            metric,
            call: Box::new(move || {
                call(function, &mut Plain, &constants, &args, &kwargs)
                    .map_err(|raised| Failure::with_message("BenchmarkError", raised.report()))
            }),
            expected,
        });
    }
    Ok(cases)
}
