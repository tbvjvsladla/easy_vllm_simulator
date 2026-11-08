//! The frozen post-quality measurement of the generation owner's functions: preflight, measured loops and the buffered report.
//!
//! The fixture rows carry the operation, its arguments and the **expected whole value** (and standard error text) taken from a
//! saved original result. Two groups exist. A *pure* row calls a function that touches no file. An *io* row calls a function that
//! really formats, creates directories, stats, opens, writes, closes and changes modes (`generate` with `force`, `append`
//! to a JSONL log), and names what the files must be afterwards: `files` (path, exact text, mode) and, for a log, `log` (path,
//! the exact line one call appends, mode).
//!
//! Before any loop runs, every row is called once outside the timer and its complete value, its standard error and (io) its files
//! are compared with the expected ones; a log is then cleared (truncated, its mode kept) outside every timer, so the measured
//! calls start from an empty own log. In a measured loop each call's complete value is kept alive with `black_box`, and a failing
//! call ends the run. After all series, the files are checked again and a log must hold exactly one expected line per measured
//! call. Nothing is reported unless every series and every check succeeded: all series and the header are buffered. The calls
//! are the real functions: arguments are bound at each call, and nothing about the files is cached.

use crate::feedback::Clock;
use crate::generation_api::{resolve, Context, Function};
use crate::model_api::encode;
use crate::observe::same_value;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::hint::black_box;
use std::os::unix::fs::PermissionsExt;
use std::time::Instant;

/// One file an io row must leave behind.
pub struct FileExpectation {
    path: String,
    bytes: Vec<u8>,
    mode: u32,
}

/// The JSONL log an io row appends to: one call adds `line`.
pub struct LogExpectation {
    path: String,
    line: Vec<u8>,
    mode: u32,
}

/// One measured function call with the result it must produce.
pub struct Case {
    pub metric: String,
    function: &'static Function,
    args: Vec<PyValue>,
    kwargs: PyDict,
    context: Context,
    pub expected: PyValue,
    expected_stderr: PyStr,
    files: Vec<FileExpectation>,
    log: Option<LogExpectation>,
}

impl Case {
    fn call(&self) -> Res<PyValue> {
        let mut stderr = PyStr::default();
        self.function
            .call(&self.args, &self.kwargs, &self.context, &mut stderr)
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

fn mode_of(members: &PyDict) -> Res<u32> {
    match members.get("mode") {
        Some(PyValue::Int(number)) => u32::try_from(number.clone())
            .map_err(|_| benchmark_error("a mode is out of range".to_string())),
        _ => Err(benchmark_error(
            "a file expectation has no mode".to_string(),
        )),
    }
}

fn dict_of(value: &PyValue) -> Res<&PyDict> {
    match value {
        PyValue::Dict(map) => Ok(map),
        _ => Err(benchmark_error(
            "a fixture member is not an object".to_string(),
        )),
    }
}

/// Reads the fixture: a JSON array of `{metric, operation, args, kwargs, context?, expected, expected_stderr?, files?, log?}`,
/// parsed once.
pub fn load(source: &str) -> Res<Vec<Case>> {
    let PyValue::List(rows) = loads_objects(source)
        .map_err(|error| benchmark_error(format!("fixture is not JSON: {error}")))?
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
        let mut context = Context {
            clock: Clock::Local,
            skill_root: PyStr::default(),
        };
        if let Some(PyValue::Dict(given)) = members.get("context") {
            if let Some(PyValue::Str(now)) = given.get("now_iso") {
                context.clock = Clock::Fixed(now.clone());
            }
            if let Some(PyValue::Str(root)) = given.get("skill_root") {
                context.skill_root = root.clone();
            }
        }
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        let expected_stderr = match members.get("expected_stderr") {
            Some(PyValue::Str(text)) => text.clone(),
            _ => PyStr::default(),
        };
        let mut files = Vec::new();
        if let Some(PyValue::List(list)) = members.get("files") {
            for item in list {
                let item = dict_of(item)?;
                files.push(FileExpectation {
                    path: text_of(item, "path")?,
                    bytes: text_of(item, "text")?.into_bytes(),
                    mode: mode_of(item)?,
                });
            }
        }
        let log = match members.get("log") {
            Some(value @ PyValue::Dict(_)) => {
                let item = dict_of(value)?;
                Some(LogExpectation {
                    path: text_of(item, "path")?,
                    line: text_of(item, "line")?.into_bytes(),
                    mode: mode_of(item)?,
                })
            }
            _ => None,
        };
        cases.push(Case {
            metric,
            function,
            args,
            kwargs,
            context,
            expected,
            expected_stderr,
            files,
            log,
        });
    }
    Ok(cases)
}

fn check_files(case: &Case) -> Res<()> {
    for file in &case.files {
        let bytes = std::fs::read(&file.path).map_err(|error| {
            benchmark_error(format!(
                "{}: cannot read {}: {error}",
                case.metric, file.path
            ))
        })?;
        let mode = std::fs::metadata(&file.path)
            .map(|metadata| metadata.permissions().mode() & 0o7777)
            .map_err(|error| benchmark_error(format!("{}: {}: {error}", case.metric, file.path)))?;
        if bytes != file.bytes || mode != file.mode {
            return Err(benchmark_error(format!(
                "{}: the file {} differs from the saved original result",
                case.metric, file.path
            )));
        }
    }
    Ok(())
}

fn check_log(case: &Case, lines: usize) -> Res<()> {
    let Some(log) = &case.log else {
        return Ok(());
    };
    let bytes = std::fs::read(&log.path).map_err(|error| {
        benchmark_error(format!(
            "{}: cannot read {}: {error}",
            case.metric, log.path
        ))
    })?;
    let mode = std::fs::metadata(&log.path)
        .map(|metadata| metadata.permissions().mode() & 0o7777)
        .map_err(|error| benchmark_error(format!("{}: {}: {error}", case.metric, log.path)))?;
    if mode != log.mode
        || bytes.len() != log.line.len() * lines
        || !bytes.chunks(log.line.len()).all(|row| row == log.line)
    {
        return Err(benchmark_error(format!(
            "{}: the log {} does not hold exactly {lines} expected line(s)",
            case.metric, log.path
        )));
    }
    Ok(())
}

/// The preflight, the measured loops and the report text; an `Err` means no report exists.
pub fn run_cases(cases: &[Case], rounds: usize, iterations: usize) -> Res<String> {
    for case in cases {
        let mut stderr = PyStr::default();
        let value = case
            .function
            .call(&case.args, &case.kwargs, &case.context, &mut stderr)
            .map_err(|failure| {
                benchmark_error(format!(
                    "{} fails before measurement: {}",
                    case.metric,
                    failure.line().to_utf8().unwrap_or_default()
                ))
            })?;
        let value = encode(&value);
        if !same_value(&case.expected, &value) || stderr != case.expected_stderr {
            return Err(benchmark_error(format!(
                "{}: the value or the standard error differs from the saved original result: {}",
                case.metric,
                value.repr().chars().take(160).collect::<String>()
            )));
        }
        check_files(case)?;
        check_log(case, 1)?;
        if let Some(log) = &case.log {
            // Outside every timer: the own benchmark log starts empty again, its mode kept.
            let file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&log.path)
                .map_err(|error| {
                    benchmark_error(format!("{}: {}: {error}", case.metric, log.path))
                })?;
            drop(file);
        }
    }
    let mut report = format!("{{\"rounds\": {rounds}, \"iterations\": {iterations}}}\n");
    for case in cases {
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
    for case in cases {
        check_files(case)?;
        check_log(case, rounds * iterations)?;
    }
    Ok(report)
}
