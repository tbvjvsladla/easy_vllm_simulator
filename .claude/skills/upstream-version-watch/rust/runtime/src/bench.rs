//! The frozen measurement of the eleven functions: preflight, measured loops and the buffered report. The rows and the case type are
//! the generic ones of `recipe_runtime::bench`; the loop is local because a row may carry its own iteration count (the generic harness
//! has one global count).
//!
//! A fixture row is `{metric, operation, args, kwargs, globals?, profile?, iterations?, expected}` with `operation`
//! `resolve_build_track.<function>`, `expected` the **whole DTO** of a saved original result and `iterations` an optional positive
//! integer (absent: the process's `--iterations`). Before any timer every row is called exactly once (a real call: the real file system
//! and the real YAML) and its complete DTO (types, key order) is compared with the expected one; a failing call or a difference ends the
//! run. Each measured iteration is the whole public call plus the creation of the same DTO, created and dropped inside the timer;
//! nothing is cached or traced and the loop reads no expected value. Nothing is reported unless every series succeeded.

use crate::api::{call, resolve_operation, Capture, Globals};
use crate::dto::encode;
use crate::host::YamlProfile;
use crate::manifest::World;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::bench::Case;
use recipe_runtime::observe::same_value;
use recipe_runtime::pyexpr::Res;
use std::hint::black_box;
use std::time::Instant;

/// One measured series: the case and the number of iterations of each of its rounds (`None`: the process's own count).
pub struct Series {
    pub case: Case,
    pub iterations: Option<usize>,
}

pub(crate) fn benchmark_error(message: String) -> Failure {
    Failure::new("BenchmarkError", &message)
}

pub(crate) fn text_of(members: &PyDict, key: &str) -> Res<String> {
    match members.get(key) {
        Some(PyValue::Str(own)) => own
            .to_utf8()
            .ok_or_else(|| benchmark_error(format!("{key} is not text"))),
        _ => Err(benchmark_error(format!(
            "a fixture row has no text member {key}"
        ))),
    }
}

/// The optional per-row `iterations`: absent is `None`; present it is an integer of at least one that fits the machine.
pub(crate) fn iterations_of(members: &PyDict, metric: &str) -> Res<Option<usize>> {
    match members.get("iterations") {
        None => Ok(None),
        Some(PyValue::Int(count)) => match usize::try_from(count.clone()) {
            Ok(count) if count > 0 => Ok(Some(count)),
            _ => Err(benchmark_error(format!(
                "{metric}: iterations is not a positive integer"
            ))),
        },
        Some(_) => Err(benchmark_error(format!(
            "{metric}: iterations is not a positive integer"
        ))),
    }
}

/// Reads the fixture: a JSON array of rows, parsed once. `columns` and `default_profile` are the process's own.
pub fn load(
    source: &str,
    columns: Option<String>,
    default_profile: YamlProfile,
) -> Res<Vec<Series>> {
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
        let function = resolve_operation(&text_of(&members, "operation")?).map_err(|raised| {
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
        let mut globals = Globals::defaults();
        if let Some(given) = members.get("globals") {
            globals
                .replace_from(given)
                .map_err(|problem| benchmark_error(format!("{metric}: {problem}")))?;
        }
        let profile = match members.get("profile") {
            None => default_profile,
            Some(PyValue::Str(name)) => name
                .to_utf8()
                .and_then(|name| YamlProfile::from_declared(&name))
                .ok_or_else(|| benchmark_error(format!("{metric}: unknown profile")))?,
            Some(_) => return Err(benchmark_error(format!("{metric}: profile is text"))),
        };
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        let iterations = iterations_of(&members, &metric)?;
        let columns = columns.clone();
        let case = Case {
            metric,
            call: Box::new(move || {
                let mut capture = Capture::new(profile, false);
                let value = call(
                    function,
                    &mut World {
                        host: &mut capture,
                        argv: &globals.argv,
                        columns: columns.as_deref(),
                    },
                    &args,
                    &kwargs,
                )
                .map_err(|thrown| {
                    Failure::with_message("BenchmarkError", thrown.raised.report())
                })?;
                encode(&value).map_err(benchmark_error)
            }),
            expected,
        };
        cases.push(Series { case, iterations });
    }
    Ok(cases)
}

/// The preflight, the measured loops and the report text; an `Err` means no report exists.
///
/// Every row is called exactly once and compared whole before any timer starts (a count of its own does not repeat it); then each row is
/// measured over `rounds` rounds of its own iteration count (or `default_iterations`). The header keeps the process-wide count, each
/// series line states the count it actually ran.
pub fn run_series(series: &[Series], rounds: usize, default_iterations: usize) -> Res<String> {
    for row in series {
        let case = &row.case;
        let value = (case.call)().map_err(|failure| {
            benchmark_error(format!(
                "{} fails before measurement: {}",
                case.metric,
                failure.line().to_utf8().unwrap_or_default()
            ))
        })?;
        if !same_value(&case.expected, &value) {
            return Err(benchmark_error(format!(
                "{}: the value differs from the saved original result: {}",
                case.metric,
                value.repr().chars().take(160).collect::<String>()
            )));
        }
    }
    let mut report = format!("{{\"rounds\": {rounds}, \"iterations\": {default_iterations}}}\n");
    for row in series {
        let case = &row.case;
        let iterations = row.iterations.unwrap_or(default_iterations);
        let mut samples = Vec::with_capacity(rounds);
        for _ in 0..rounds {
            let started = Instant::now();
            for _ in 0..iterations {
                match (case.call)() {
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
            samples.push(started.elapsed().as_nanos() as f64 / iterations as f64);
        }
        let mut sorted = samples.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let median = sorted[sorted.len() / 2];
        let shown: Vec<String> = samples
            .iter()
            .map(|sample| format!("{sample:.1}"))
            .collect();
        let metric = PyValue::str(&case.metric)
            .dumps_ascii()
            .map_err(|error| benchmark_error(format!("cannot encode a metric name: {error}")))?;
        report.push_str(&format!(
            "{{\"metric\": {metric}, \"unit\": \"ns_per_call\", \"median\": {median:.1}, \"rounds\": [{}], \"round_count\": {rounds}, \"iterations\": {iterations}}}\n",
            shown.join(", ")
        ));
    }
    Ok(report)
}
