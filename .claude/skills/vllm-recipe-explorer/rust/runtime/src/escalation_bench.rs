//! The frozen post-quality measurement of the escalation judgment: preflight, measured loops and the buffered report.
//!
//! A fixture row is `{metric, operation: "escalation_predicate.judge", args, kwargs, provider, expected}`; `expected` is the **whole value**
//! of a saved original result and `provider` says where the classifier comes from, and is one of exactly two things:
//!
//! * `{"kind": "fixed-classifier", "kill_event_kinds": [...]}`: a stateless control that answers every load with that `KILL_EVENT_KINDS`
//!   (explicitly a control: no launcher run, no classifier call is measured);
//! * `{"kind": "canonical-classifier", "classifier": PATH}`: the actual asset `classify_cell_native.py` at PATH and its neighbouring
//!   launcher, so that every full judgment makes the real `constants` call of the existing Rust classifier owner.
//!
//! Before any timer every row is called once and its complete value (types, key order) is compared with the expected one; a failing call
//! or a difference ends the run. In a measured loop each call's complete value is kept alive and dropped inside the timer. The
//! dependency counters (launcher runs attempted and succeeded) are plain counters read before the loop, after the preflight and after
//! the loops: nothing grows inside a loop. Nothing is reported unless every series succeeded. The report is a header line and one line
//! per row; a row's line adds `provider` and `dependency_counts` (`preflight` and `loop`, each `{launcher_runs, succeeded}`).

use crate::escalation::{Dependency, Loaded, Raised, Vocabulary};
use crate::escalation_api::call_judge;
use crate::escalation_bridge::Bridge;
use crate::model_api::encode;
use crate::observe::same_value;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use std::hint::black_box;
use std::time::Instant;

/// The stateless control loader.
struct FixedClassifier {
    kinds: Vec<PyValue>,
}

impl Dependency for FixedClassifier {
    fn load(&mut self) -> Result<Loaded, Raised> {
        Ok(Loaded {
            kill_event_kinds: Some(self.kinds.clone()),
        })
    }

    fn utc(&mut self, _value: &PyStr, _field: &str) -> Result<PyValue, Raised> {
        Err(Raised::plain(
            "RuntimeError",
            "the fixed control loads only",
        ))
    }

    fn kill_events(
        &mut self,
        _l: &[PyStr],
        _s: &PyValue,
        _e: &PyValue,
        _t: &BigInt,
    ) -> Result<PyValue, Raised> {
        Err(Raised::plain(
            "RuntimeError",
            "the fixed control loads only",
        ))
    }
}

enum Source {
    Fixed(FixedClassifier),
    Canonical(Bridge),
}

impl Source {
    fn dependency(&mut self) -> &mut dyn Dependency {
        match self {
            Source::Fixed(fixed) => fixed,
            Source::Canonical(bridge) => bridge,
        }
    }

    fn counters(&self) -> (u64, u64) {
        match self {
            Source::Fixed(_) => (0, 0),
            Source::Canonical(bridge) => bridge.counters(),
        }
    }
}

/// One measured judgment with the value it must produce.
pub struct Case {
    pub metric: String,
    provider: &'static str,
    source: Source,
    args: Vec<PyValue>,
    kwargs: PyDict,
    expected: PyValue,
}

impl Case {
    fn call(&mut self) -> Result<PyValue, Raised> {
        let vocabulary = Vocabulary::canonical();
        let value = call_judge(
            self.source.dependency(),
            &vocabulary,
            &self.args,
            &self.kwargs,
        )?;
        Ok(PyValue::Dict(value))
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
        if text_of(&members, "operation")? != "escalation_predicate.judge" {
            return Err(benchmark_error(format!(
                "{metric}: the operation is escalation_predicate.judge"
            )));
        }
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
        let Some(PyValue::Dict(provider)) = members.get("provider") else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no provider"
            )));
        };
        let (name, source) = match (text_of(provider, "kind")?.as_str(), provider.len()) {
            ("fixed-classifier", 2) => match provider.get("kill_event_kinds") {
                Some(PyValue::List(kinds)) => (
                    "fixed-classifier",
                    Source::Fixed(FixedClassifier {
                        kinds: kinds.clone(),
                    }),
                ),
                _ => {
                    return Err(benchmark_error(format!(
                        "{metric}: kill_event_kinds is an array"
                    )))
                }
            },
            ("canonical-classifier", 2) => match provider.get("classifier") {
                Some(PyValue::Str(path)) => (
                    "canonical-classifier",
                    Source::Canonical(Bridge::for_asset(path.clone())),
                ),
                _ => return Err(benchmark_error(format!("{metric}: classifier is a path"))),
            },
            _ => {
                return Err(benchmark_error(format!(
                    "{metric}: the provider is exactly fixed-classifier or canonical-classifier"
                )))
            }
        };
        cases.push(Case {
            metric,
            provider: name,
            source,
            args,
            kwargs,
            expected,
        });
    }
    Ok(cases)
}

fn counts_text(before: (u64, u64), after: (u64, u64)) -> String {
    format!(
        "{{\"launcher_runs\": {}, \"succeeded\": {}}}",
        after.0 - before.0,
        after.1 - before.1
    )
}

/// The preflight, the measured loops and the report text; an `Err` means no report exists.
pub fn run_cases(cases: &mut [Case], rounds: usize, iterations: usize) -> Res<String> {
    let mut started: Vec<(u64, u64)> = Vec::new();
    let mut preflight: Vec<(u64, u64)> = Vec::new();
    for case in cases.iter_mut() {
        let before = case.source.counters();
        let value = case.call().map_err(|raised| {
            benchmark_error(format!(
                "{} fails before measurement: {}",
                case.metric,
                raised.report().to_utf8().unwrap_or_default().trim_end()
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
        started.push(before);
        preflight.push(case.source.counters());
    }
    let mut report = format!("{{\"rounds\": {rounds}, \"iterations\": {iterations}}}\n");
    for (index, case) in cases.iter_mut().enumerate() {
        let loop_start = case.source.counters();
        let mut samples = Vec::with_capacity(rounds);
        for _ in 0..rounds {
            let began = Instant::now();
            for _ in 0..iterations {
                match case.call() {
                    Ok(value) => {
                        black_box(value);
                    }
                    Err(raised) => {
                        return Err(benchmark_error(format!(
                            "{} failed inside a measured loop: {}",
                            case.metric,
                            raised.report().to_utf8().unwrap_or_default().trim_end()
                        )));
                    }
                }
            }
            samples.push(began.elapsed().as_nanos() as f64 / iterations.max(1) as f64);
        }
        let loop_end = case.source.counters();
        let mut sorted = samples.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let median = sorted[sorted.len() / 2];
        let shown: Vec<String> = samples
            .iter()
            .map(|sample| format!("{sample:.1}"))
            .collect();
        report.push_str(&format!(
            "{{\"metric\": \"{}\", \"unit\": \"ns_per_call\", \"median\": {median:.1}, \"rounds\": [{}], \"round_count\": {rounds}, \"iterations\": {iterations}, \"provider\": \"{}\", \"dependency_counts\": {{\"preflight\": {}, \"loop\": {}}}}}\n",
            case.metric,
            shown.join(", "),
            case.provider,
            counts_text(started[index], preflight[index]),
            counts_text(loop_start, loop_end)
        ));
    }
    Ok(report)
}
