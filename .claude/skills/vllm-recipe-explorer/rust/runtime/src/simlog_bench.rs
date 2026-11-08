//! The frozen post-quality measurement of the simlog writer: one preflight call per row, whole value and written files checked before
//! any timer, measured loops of the whole public function, and the buffered report (the generic harness of [`crate::bench`]).
//!
//! A fixture row is `{metric, operation, args, kwargs, globals?, expected, expected_files}`: `operation` is one of the five
//! `simlog_writer.*` functions, `globals` may hold `yaml_enabled` (a bool), `expected` is the **whole value** of a saved original
//! result and `expected_files` an array of `{path, bytes_hex}`: the files the preflight call must have left, byte for byte. The
//! preflight call is a real call of the public function (it writes the real files), its complete value (types, key order) is compared
//! with `expected` and then every expected file is read back; a failure or a difference ends the run before any timer. Inside a
//! measured loop each call is the whole public function over the real paths (join, `makedirs`, open and truncate, chunked encoding,
//! write, close) and creates and drops its complete result; nothing is cached or traced. The arguments are decoded once, before any
//! timer. A row whose call appends grows its file on every call. Nothing is reported unless every series succeeded.

use crate::bench::{run_cases_checked, Case};
use crate::pyexpr::Res;
use crate::simlog::{call, Function, Plain, MODULE};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

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

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len() / 2)
        .map(|at| u8::from_str_radix(text.get(2 * at..2 * at + 2)?, 16).ok())
        .collect()
}

/// The fixture: the cases, and for each the files its preflight call must leave.
pub struct Fixture {
    cases: Vec<Case>,
    files: Vec<Vec<(PyStr, Vec<u8>)>>,
}

/// Reads the fixture: a JSON array of rows, parsed once.
pub fn load(source: &str) -> Res<Fixture> {
    let PyValue::List(rows) =
        loads(source).map_err(|error| benchmark_error(format!("fixture is not JSON: {error}")))?
    else {
        return Err(benchmark_error("the fixture is not an array".to_string()));
    };
    let mut cases = Vec::new();
    let mut files = Vec::new();
    for row in rows {
        let PyValue::Dict(members) = row else {
            return Err(benchmark_error(
                "a fixture row is not an object".to_string(),
            ));
        };
        let metric = text_of(&members, "metric")?;
        let operation = text_of(&members, "operation")?;
        let function = operation
            .strip_prefix(&format!("{MODULE}."))
            .and_then(Function::named)
            .ok_or_else(|| benchmark_error(format!("{metric}: unknown operation {operation}")))?;
        let args = match members.get("args") {
            Some(PyValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let kwargs = match members.get("kwargs") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let yaml_enabled = match members.get("globals") {
            None => true,
            Some(PyValue::Dict(map)) if map.is_empty() => true,
            Some(PyValue::Dict(map))
                if map.len() == 1 && matches!(map.get("yaml_enabled"), Some(PyValue::Bool(_))) =>
            {
                matches!(map.get("yaml_enabled"), Some(PyValue::Bool(true)))
            }
            Some(_) => {
                return Err(benchmark_error(format!(
                    "{metric}: globals holds only yaml_enabled (a bool)"
                )))
            }
        };
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        let mut expected_files = Vec::new();
        match members.get("expected_files") {
            Some(PyValue::List(entries)) => {
                for entry in entries {
                    let PyValue::Dict(entry) = entry else {
                        return Err(benchmark_error(format!(
                            "{metric}: an expected file is not an object"
                        )));
                    };
                    let (Some(PyValue::Str(path)), Some(PyValue::Str(hex))) =
                        (entry.get("path"), entry.get("bytes_hex"))
                    else {
                        return Err(benchmark_error(format!(
                            "{metric}: an expected file has a path and bytes_hex"
                        )));
                    };
                    let bytes = hex
                        .to_utf8()
                        .as_deref()
                        .and_then(hex_bytes)
                        .ok_or_else(|| {
                            benchmark_error(format!("{metric}: bytes_hex is not hexadecimal"))
                        })?;
                    expected_files.push((path.clone(), bytes));
                }
            }
            _ => {
                return Err(benchmark_error(format!(
                    "{metric}: the row has no expected_files"
                )))
            }
        }
        cases.push(Case {
            metric,
            call: Box::new(move || {
                call(function, &mut Plain, yaml_enabled, &args, &kwargs).map_err(|failure| {
                    Failure::with_message("BenchmarkError", failure.raised.report())
                })
            }),
            expected,
        });
        files.push(expected_files);
    }
    Ok(Fixture { cases, files })
}

/// The preflight (one call per row, whole value, then the files), the measured loops and the report text.
pub fn run(fixture: &Fixture, rounds: usize, iterations: usize) -> Res<String> {
    run_cases_checked(&fixture.cases, rounds, iterations, &mut |index| {
        let metric = &fixture.cases[index].metric;
        for (path, expected) in &fixture.files[index] {
            let shown = path.to_utf8().unwrap_or_default();
            let found = path
                .to_surrogateescape_bytes()
                .ok()
                .and_then(|bytes| {
                    use std::os::unix::ffi::OsStrExt;
                    std::fs::read(std::ffi::OsStr::from_bytes(&bytes)).ok()
                })
                .ok_or_else(|| {
                    benchmark_error(format!(
                        "{metric}: the expected file {shown} is not readable"
                    ))
                })?;
            if &found != expected {
                return Err(benchmark_error(format!(
                    "{metric}: the file {shown} differs from the saved original bytes"
                )));
            }
        }
        Ok(())
    })
}
