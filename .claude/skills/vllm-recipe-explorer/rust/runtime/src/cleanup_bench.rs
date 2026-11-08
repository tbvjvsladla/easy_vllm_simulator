//! The frozen post-quality measurement of the `cleanup_docker` operations: preflight, measured loops and the buffered report (the generic
//! harness of [`crate::bench`]).
//!
//! A fixture row is `{metric, operation, args, kwargs, globals?, expected}` with `operation` one of the four `cleanup_docker.*` operations and
//! `expected` the **whole typed DTO** of a saved original result. Before any timer every row is called once (a real call: the real file
//! system and the real child processes found through `PATH`) and its complete DTO (types, key order) is compared with the expected one; a
//! failing call or a difference ends the run. Each measured iteration is the whole public call plus the creation of the same typed DTO,
//! created and dropped inside the timer; nothing is cached or traced. Nothing is reported unless every series succeeded. What the
//! processes saw is the caller's to check, outside the timers.

use crate::bench::Case;
use crate::cleanup_api::{call, resolve, Capture, Defaults, Globals};
use crate::cleanup_docker::World;
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
pub fn load(source: &str, defaults: Defaults) -> Res<Vec<Case>> {
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
        let mut globals = Globals::defaults(&defaults.repo);
        if let Some(given) = members.get("globals") {
            globals
                .replace_from(given)
                .map_err(|problem| benchmark_error(format!("{metric}: {problem}")))?;
        }
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        let columns = defaults.columns.clone();
        cases.push(Case {
            metric,
            call: Box::new(move || {
                let mut capture = Capture::new(false);
                call(
                    function,
                    &mut World {
                        console: &mut capture,
                        tracing: false,
                    },
                    &globals,
                    columns.as_deref(),
                    &args,
                    &kwargs,
                )
                .map_err(|thrown| Failure::with_message("BenchmarkError", thrown.raised.report()))
            }),
            expected,
        });
    }
    Ok(cases)
}
