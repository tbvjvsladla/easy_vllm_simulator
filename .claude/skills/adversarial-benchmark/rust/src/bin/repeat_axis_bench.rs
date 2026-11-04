//! Warm timing harness for the pure repeat-axis API (measurement only; it makes no claim about other programs).
//!
//! The cases come from a case file in the standard observation format (`{id, operation, arguments, expected}` per
//! line); `--ids` selects them (comma list or `@file`, in the order given; all lines when absent). Every case gets
//! its own honest preparation *outside* the timer: the arguments are already parsed and bound to the operation's
//! parameters (`band_pct(values)`, `parse_repeats(raw, where)`, `summarize(levels, requested, ...)`,
//! `repetition_established(summary, judgment_level)`). A timed call builds the complete result - the typed result
//! and its full ordered value, the counterpart of the Python function's return value - consumes it with
//! `black_box` and drops it. Nothing is cached between calls and no field is omitted.
//!
//! Before any timing, every selected case's result is rendered and compared with the case's `expected` (the
//! harness refuses to time a case it does not reproduce).
//!
//! Usage: `repeat_axis_bench --cases FILE [--ids a,b,c|@FILE] [--rounds 11] [--iterations 200]`.
//! One JSON document on stdout: nanoseconds per case call (the median of the rounds) and per pass over all cases.

use std::hint::black_box;
use std::time::Instant;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};
use sweep_stop::repeat_axis::{
    band_pct, parse_repeats, repetition_established, summarize, SummarizeRequest,
};

/// One prepared case: the operation and its bound arguments.
enum Prepared {
    BandPct {
        values: PyValue,
    },
    ParseRepeats {
        raw: PyValue,
        where_: PyValue,
    },
    Summarize {
        levels: PyValue,
        requested: PyValue,
        requested_source: PyValue,
        kind: PyValue,
        clamp_run: PyValue,
    },
    RepetitionEstablished {
        summary: PyValue,
        judgment_level: PyValue,
    },
}

struct Case {
    id: String,
    prepared: Prepared,
    expected: PyDict,
}

/// The complete result of one call: a value or the exception it raised.
fn call(prepared: &Prepared) -> Result<PyValue, sweep_stop::guidellm::Failure> {
    match prepared {
        Prepared::BandPct { values } => Ok(band_pct(values)?.to_value()),
        Prepared::ParseRepeats { raw, where_ } => Ok(PyValue::Int(parse_repeats(raw, where_)?)),
        Prepared::Summarize {
            levels,
            requested,
            requested_source,
            kind,
            clamp_run,
        } => Ok(summarize(&SummarizeRequest {
            levels,
            requested,
            requested_source,
            kind,
            clamp_run,
        })?
        .to_value()),
        Prepared::RepetitionEstablished {
            summary,
            judgment_level,
        } => Ok(repetition_established(summary, judgment_level)?.to_value()),
    }
}

fn argument(arguments: &PyDict, name: &str) -> PyValue {
    arguments.get(name).cloned().unwrap_or(PyValue::None)
}

fn prepare(operation: &str, arguments: &PyDict) -> Result<Prepared, String> {
    Ok(match operation {
        "band_pct" => Prepared::BandPct {
            values: argument(arguments, "values"),
        },
        "parse_repeats" => Prepared::ParseRepeats {
            raw: argument(arguments, "raw"),
            where_: argument(arguments, "where"),
        },
        "summarize" => Prepared::Summarize {
            levels: argument(arguments, "levels"),
            requested: argument(arguments, "requested"),
            requested_source: argument(arguments, "requested_source"),
            kind: argument(arguments, "kind"),
            clamp_run: argument(arguments, "clamp_run"),
        },
        "repetition_established" => Prepared::RepetitionEstablished {
            summary: argument(arguments, "summary"),
            judgment_level: argument(arguments, "judgment_level"),
        },
        other => return Err(format!("operation {other:?} is not a pure API operation")),
    })
}

fn text_of(map: &PyDict, key: &str) -> String {
    match map.get(key) {
        Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
        _ => String::new(),
    }
}

struct Settings {
    cases: String,
    ids: Option<String>,
    rounds: usize,
    iterations: usize,
}

fn parse_settings() -> Result<Settings, String> {
    let mut settings = Settings {
        cases: String::new(),
        ids: None,
        rounds: 11,
        iterations: 200,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--cases" => settings.cases = value,
            "--ids" => settings.ids = Some(value),
            "--rounds" => {
                settings.rounds = value.parse().map_err(|_| format!("bad --rounds {value}"))?
            }
            "--iterations" => {
                settings.iterations = value
                    .parse()
                    .map_err(|_| format!("bad --iterations {value}"))?
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if settings.cases.is_empty() {
        return Err("--cases FILE is required".to_string());
    }
    Ok(settings)
}

fn load_cases(settings: &Settings) -> Result<Vec<Case>, String> {
    let raw = std::fs::read_to_string(&settings.cases)
        .map_err(|error| format!("{}: {error}", settings.cases))?;
    let mut by_id: Vec<(String, PyDict)> = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        match loads(line) {
            Ok(PyValue::Dict(record)) => by_id.push((text_of(&record, "id"), record)),
            _ => return Err("a case line is not a JSON object".to_string()),
        }
    }
    let selected: Vec<String> = match &settings.ids {
        None => by_id.iter().map(|(id, _)| id.clone()).collect(),
        Some(list) => match list.strip_prefix('@') {
            Some(path) => std::fs::read_to_string(path)
                .map_err(|error| format!("{path}: {error}"))?
                .split_whitespace()
                .map(str::to_string)
                .collect(),
            None => list.split(',').map(str::to_string).collect(),
        },
    };
    selected
        .iter()
        .map(|id| {
            let (_, record) = by_id
                .iter()
                .find(|(own, _)| own == id)
                .ok_or_else(|| format!("no case {id}"))?;
            let arguments = match record.get("arguments") {
                Some(PyValue::Dict(arguments)) => arguments,
                _ => return Err(format!("{id}: arguments is not an object")),
            };
            let expected = match record.get("expected") {
                Some(PyValue::Dict(expected)) => expected.clone(),
                _ => return Err(format!("{id}: no expected")),
            };
            Ok(Case {
                id: id.clone(),
                prepared: prepare(&text_of(record, "operation"), arguments)?,
                expected,
            })
        })
        .collect()
}

/// The case's result rendered as the observation spells it, compared with `expected`.
fn verify(case: &Case) -> Result<(), String> {
    let spelled = |value: &PyValue| value.dumps_ascii().map_err(|error| error.to_string());
    match (
        call(&case.prepared),
        text_of(&case.expected, "kind").as_str(),
    ) {
        (Ok(value), "value") => {
            let want = case.expected.get("value").ok_or("expected has no value")?;
            if spelled(&value)? == spelled(want)? {
                Ok(())
            } else {
                Err(format!(
                    "{}: value differs: {} != {}",
                    case.id,
                    spelled(&value)?,
                    spelled(want)?
                ))
            }
        }
        (Err(failure), "exception") => {
            if failure.class == text_of(&case.expected, "type")
                && failure.message.to_utf8().unwrap_or_default()
                    == text_of(&case.expected, "message")
            {
                Ok(())
            } else {
                Err(format!(
                    "{}: exception differs: {}",
                    case.id,
                    failure.line().to_utf8().unwrap_or_default()
                ))
            }
        }
        _ => Err(format!("{}: outcome kind differs", case.id)),
    }
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

fn main() {
    let cases =
        parse_settings().and_then(|settings| load_cases(&settings).map(|cases| (settings, cases)));
    let (settings, cases) = match cases {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("repeat_axis_bench: {message}");
            std::process::exit(2);
        }
    };
    for case in &cases {
        if let Err(message) = verify(case) {
            eprintln!("repeat_axis_bench: refusing to time: {message}");
            std::process::exit(1);
        }
    }
    let mut per_call = Vec::with_capacity(settings.rounds);
    let mut per_pass = Vec::with_capacity(settings.rounds);
    for _ in 0..settings.rounds {
        let started = Instant::now();
        for _ in 0..settings.iterations {
            for case in &cases {
                drop(black_box(call(black_box(&case.prepared))));
            }
        }
        let elapsed = started.elapsed().as_nanos() as f64;
        per_call.push(elapsed / (settings.iterations * cases.len()) as f64);
        per_pass.push(elapsed / settings.iterations as f64);
    }
    let list = |samples: &[f64]| {
        samples
            .iter()
            .map(|s| format!("{s:.3}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!(
        "{{\n  \"cases\": {},\n  \"rounds\": {},\n  \"iterations_per_round\": {},\n  \"boundary\": \"prepared arguments -> complete typed result and ordered value, black_box, drop\",\n  \"ns_per_case_call\": {{\"median\": {:.3}, \"rounds\": [{}]}},\n  \"ns_per_pass\": {{\"median\": {:.3}, \"rounds\": [{}]}}\n}}",
        cases.len(),
        settings.rounds,
        settings.iterations,
        median(&per_call),
        list(&per_call),
        median(&per_pass),
        list(&per_pass)
    );
}
