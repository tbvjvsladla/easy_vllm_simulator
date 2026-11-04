//! `repeat_axis_api`: the Rust-only surface of the repeat axis, a separate program so that the `repeat_axis`
//! command line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - every public operation of the original module as a differential and bridge transport.
//!   Each stdin line is `{id, operation, arguments}`; each yields one line `{id, kind: value, value}` or
//!   `{id, kind: exception, type, message}`. Operations: `band_pct`, `parse_repeats`, `summarize`,
//!   `repetition_established`, `load_campaign_budgets`, `resolve_repeats`, `record_run`, `aggregate_level` and
//!   `constants`. Arguments follow the Python signatures (names, defaults, keyword-only parameters and the
//!   `TypeError` a wrong call raises). The operations that read the repository (`load_campaign_budgets`,
//!   `resolve_repeats`) take an optional `repo_root` argument, else `REPEAT_AXIS_REPO_ROOT`; path arguments are
//!   relative to the process's working directory.
//!   Opt-in: `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the
//!   result as compact ASCII JSON text in `value_json`), so the envelope never adds depth to a deep document.
//! * `constants` - the module constants as one JSON object, read from this crate (consumers' thin bridges ask
//!   here; no number or text is copied into a bridge).
//! * `native-help` - this list.

use crate::cli::{on_large_stack, write_stderr, PrintFailure, Streams, TRACEBACK_HEADER};
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads, loads_text, LoadError};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_axis::{
    band_pct, parse_repeats, repetition_established, summarize, SummarizeRequest, AGGREGATE_KEYS,
    BAND_FORMULA, BAND_METRIC, CAMPAIGN_INIT_REL, IMPLEMENTED_REPEAT_KINDS, REPEAT_KINDS,
    REPEAT_KIND_COLD, REPEAT_KIND_WARM, RUN_DIR_PATTERN, RUN_META_NAME, RUN_METRIC_KEYS,
    SCHEMA_REL, SIGNAL_RUN_FAILED, STOP_KIND_CLAMP, STOP_KIND_REPEAT_BREAK,
};
use crate::repeat_cli::Runtime;
use crate::repeat_fs::{aggregate_level, record_run, AggregateRequest, RunBoundary};
use crate::repeat_owner::{
    load_campaign_budgets, resolve_repeats, RepoRoot, ResolveRequest, CONFIGURATION_ERROR,
};
use crate::FULL_REPEATS_MIN;

const NATIVE_HELP: &str = "repeat_axis_api Rust-only commands:\n  repeat_axis_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  repeat_axis_api constants        the module constants as one JSON object\n  repeat_axis_api native-help      this text\n";
/// Wire encoding of the opt-in `result_json` transport; a bridge checks it to fail loudly against an older binary.
const JSON_VALUES_ENCODING: &str = "json-values-v1";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], runtime: &Runtime, io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(runtime, io),
        Some("constants") if args.len() == 1 => {
            print_text(io, &constants().dumps_ascii().unwrap_or_default())
        }
        Some("native-help") if args.len() == 1 => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure),
        },
        _ => {
            write_stderr(io, &PyStr::new(NATIVE_HELP));
            2
        }
    };
    io.finish(status)
}

fn print_text(io: &mut Streams<'_>, text: &str) -> i32 {
    match io.print(&PyStr::new(text)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure),
    }
}

fn print_failed(io: &mut Streams<'_>, failure: &PrintFailure) -> i32 {
    let mut text = PyStr::new(TRACEBACK_HEADER);
    text.push_str(&failure.line());
    text.push_str("\n");
    write_stderr(io, &text);
    1
}

fn strings(names: &[&str]) -> PyValue {
    PyValue::List(names.iter().map(|name| PyValue::str(name)).collect())
}

/// The constants of the original module. The floor is the one compiled in from the campaign schema.
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    map.set("FULL_REPEATS_MIN", PyValue::int(FULL_REPEATS_MIN));
    map.set("REPEAT_KIND_WARM", PyValue::str(REPEAT_KIND_WARM));
    map.set("REPEAT_KIND_COLD", PyValue::str(REPEAT_KIND_COLD));
    map.set("REPEAT_KINDS", strings(&REPEAT_KINDS));
    map.set(
        "IMPLEMENTED_REPEAT_KINDS",
        strings(&IMPLEMENTED_REPEAT_KINDS),
    );
    map.set("RUN_META_NAME", PyValue::str(RUN_META_NAME));
    map.set("RUN_DIR_PATTERN", PyValue::str(RUN_DIR_PATTERN));
    map.set("SIGNAL_RUN_FAILED", PyValue::str(SIGNAL_RUN_FAILED));
    map.set(
        "STOP_KIND_REPEAT_BREAK",
        PyValue::str(STOP_KIND_REPEAT_BREAK),
    );
    map.set("STOP_KIND_CLAMP", PyValue::str(STOP_KIND_CLAMP));
    map.set("RUN_METRIC_KEYS", strings(&RUN_METRIC_KEYS));
    map.set("BAND_METRIC", PyValue::str(BAND_METRIC));
    map.set("BAND_FORMULA", PyValue::str(BAND_FORMULA));
    map.set("AGGREGATE_KEYS", strings(&AGGREGATE_KEYS));
    map.set("CAMPAIGN_INIT_REL", PyValue::str(CAMPAIGN_INIT_REL));
    map.set("SCHEMA_REL", PyValue::str(SCHEMA_REL));
    PyValue::Dict(map)
}

// ── Python call binding ─────────────────────────────────────────────────────────

/// The parameters of one Python function: `positional` may be given by keyword here (there is no other way on the
/// wire), `keyword_only` follow `*`. The names of parameters with defaults are listed so that an absent one is fine.
struct Signature {
    function: &'static str,
    positional: &'static [&'static str],
    keyword_only: &'static [&'static str],
    /// Parameters (of either kind) that have a default.
    optional: &'static [&'static str],
}

fn name_list(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("'{name}'")).collect();
    match quoted.len() {
        1 => quoted[0].clone(),
        2 => format!("{} and {}", quoted[0], quoted[1]),
        _ => format!(
            "{}, and {}",
            quoted[..quoted.len() - 1].join(", "),
            quoted[quoted.len() - 1]
        ),
    }
}

impl Signature {
    /// The call `function(**arguments)`: an unexpected name first (in argument order), then the missing
    /// parameters, as CPython reports them.
    fn bind<'a>(&self, arguments: &'a PyDict) -> Result<Bound<'a>, Failure> {
        for (name, _) in arguments.iter() {
            let known = self
                .positional
                .iter()
                .chain(self.keyword_only)
                .any(|parameter| name.equals(parameter));
            if !known {
                return Err(Failure::new(
                    "TypeError",
                    &format!(
                        "{}() got an unexpected keyword argument {}",
                        self.function,
                        name.repr()
                    ),
                ));
            }
        }
        let missing = |names: &[&'static str]| -> Vec<&'static str> {
            names
                .iter()
                .copied()
                .filter(|name| !self.optional.contains(name) && !arguments.contains_key(name))
                .collect()
        };
        let lacking = missing(self.positional);
        if !lacking.is_empty() {
            return Err(Failure::new(
                "TypeError",
                &format!(
                    "{}() missing {} required positional argument{}: {}",
                    self.function,
                    lacking.len(),
                    if lacking.len() == 1 { "" } else { "s" },
                    name_list(&lacking)
                ),
            ));
        }
        let lacking = missing(self.keyword_only);
        if !lacking.is_empty() {
            return Err(Failure::new(
                "TypeError",
                &format!(
                    "{}() missing {} required keyword-only argument{}: {}",
                    self.function,
                    lacking.len(),
                    if lacking.len() == 1 { "" } else { "s" },
                    name_list(&lacking)
                ),
            ));
        }
        Ok(Bound(arguments))
    }
}

struct Bound<'a>(&'a PyDict);

impl Bound<'_> {
    fn get(&self, name: &str) -> PyValue {
        self.0.get(name).cloned().unwrap_or(PyValue::None)
    }

    fn get_or(&self, name: &str, default: PyValue) -> PyValue {
        self.0.get(name).cloned().unwrap_or(default)
    }

    /// `repo_root`: the argument when given, else the process's configured root.
    fn repo_root(&self) -> RepoRoot {
        match self.0.get("repo_root") {
            Some(value) => RepoRoot::from_value(value),
            None => RepoRoot::from_environment(),
        }
    }
}

const BAND_PCT: Signature = Signature {
    function: "band_pct",
    positional: &["values"],
    keyword_only: &[],
    optional: &[],
};
const PARSE_REPEATS: Signature = Signature {
    function: "parse_repeats",
    positional: &["raw", "where"],
    keyword_only: &[],
    optional: &[],
};
const SUMMARIZE: Signature = Signature {
    function: "summarize",
    positional: &["levels"],
    keyword_only: &["requested", "requested_source", "kind", "clamp_run"],
    optional: &["requested", "requested_source", "kind", "clamp_run"],
};
const REPETITION_ESTABLISHED: Signature = Signature {
    function: "repetition_established",
    positional: &["summary", "judgment_level"],
    keyword_only: &[],
    optional: &[],
};
const LOAD_CAMPAIGN_BUDGETS: Signature = Signature {
    function: "load_campaign_budgets",
    positional: &["campaign_id"],
    keyword_only: &["repo_root"],
    optional: &["campaign_id", "repo_root"],
};
const RESOLVE_REPEATS: Signature = Signature {
    function: "resolve_repeats",
    positional: &["argv_value"],
    keyword_only: &[
        "argv_source",
        "campaign_id",
        "repo_root",
        "preset_default",
        "preset_source",
    ],
    optional: &[
        "argv_value",
        "argv_source",
        "campaign_id",
        "repo_root",
        "preset_default",
        "preset_source",
    ],
};
const RECORD_RUN: Signature = Signature {
    function: "record_run",
    positional: &["run_dir"],
    keyword_only: &[
        "level",
        "run",
        "started_utc",
        "ended_utc",
        "run_bench_rc",
        "parse_rc",
    ],
    optional: &[],
};
const AGGREGATE_LEVEL: Signature = Signature {
    function: "aggregate_level",
    positional: &["level_dir"],
    keyword_only: &["requested", "requested_source", "kind"],
    optional: &["kind"],
};
const CONSTANTS: Signature = Signature {
    function: "constants",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};

fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    runtime: &Runtime,
) -> Result<PyValue, Failure> {
    match operation {
        "band_pct" => {
            let call = BAND_PCT.bind(arguments)?;
            Ok(band_pct(&call.get("values"))?.to_value())
        }
        "parse_repeats" => {
            let call = PARSE_REPEATS.bind(arguments)?;
            Ok(PyValue::Int(parse_repeats(
                &call.get("raw"),
                &call.get("where"),
            )?))
        }
        "summarize" => {
            let call = SUMMARIZE.bind(arguments)?;
            let (levels, requested, source, kind, clamp_run) = (
                call.get("levels"),
                call.get("requested"),
                call.get("requested_source"),
                call.get("kind"),
                call.get("clamp_run"),
            );
            let summary = summarize(&SummarizeRequest {
                levels: &levels,
                requested: &requested,
                requested_source: &source,
                kind: &kind,
                clamp_run: &clamp_run,
            })?;
            Ok(summary.to_value())
        }
        "repetition_established" => {
            let call = REPETITION_ESTABLISHED.bind(arguments)?;
            Ok(
                repetition_established(&call.get("summary"), &call.get("judgment_level"))?
                    .to_value(),
            )
        }
        "load_campaign_budgets" => {
            let call = LOAD_CAMPAIGN_BUDGETS.bind(arguments)?;
            Ok(
                load_campaign_budgets(&call.get("campaign_id"), &call.repo_root(), &runtime.owner)?
                    .to_value(),
            )
        }
        "resolve_repeats" => {
            let call = RESOLVE_REPEATS.bind(arguments)?;
            let request = ResolveRequest {
                argv_value: call.get("argv_value"),
                argv_source: call.get("argv_source"),
                campaign_id: call.get("campaign_id"),
                preset_default: call.get("preset_default"),
                preset_source: call.get("preset_source"),
            };
            Ok(resolve_repeats(&request, &call.repo_root(), &runtime.owner)?.to_value())
        }
        "record_run" => {
            let call = RECORD_RUN.bind(arguments)?;
            let boundary = RunBoundary {
                level: call.get("level"),
                run: call.get("run"),
                started_utc: call.get("started_utc"),
                ended_utc: call.get("ended_utc"),
                run_bench_rc: call.get("run_bench_rc"),
                parse_rc: call.get("parse_rc"),
            };
            Ok(PyValue::Str(record_run(&call.get("run_dir"), &boundary)?))
        }
        "aggregate_level" => {
            let call = AGGREGATE_LEVEL.bind(arguments)?;
            let (requested, source) = (call.get("requested"), call.get("requested_source"));
            let kind = call.get_or("kind", PyValue::str(REPEAT_KIND_WARM));
            aggregate_level(
                &call.get("level_dir"),
                &AggregateRequest {
                    requested: &requested,
                    requested_source: &source,
                    kind: &kind,
                },
            )
        }
        "constants" => {
            CONSTANTS.bind(arguments)?;
            Ok(constants())
        }
        other => Err(Failure::new(
            "ValueError",
            &format!("unknown operation {}", PyStr::new(other).repr()),
        )),
    }
}

// ── evaluate-jsonl ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wire {
    /// `{id, kind: value, value}`: the result is a nested JSON value.
    Values,
    /// `result_json: true`: `{id, encoding, kind: value, value_json}`.
    JsonText,
}

fn exception_record(id: PyValue, failure: &Failure, wire: Wire) -> PyValue {
    let mut map = PyDict::new();
    map.set("id", id);
    if wire == Wire::JsonText {
        map.set("encoding", PyValue::str(JSON_VALUES_ENCODING));
    }
    map.set("kind", PyValue::str("exception"));
    map.set("type", PyValue::str(failure.class));
    map.set("message", PyValue::Str(failure.message.clone()));
    PyValue::Dict(map)
}

fn value_record(id: PyValue, value: PyValue, wire: Wire) -> PyValue {
    let mut map = PyDict::new();
    map.set("id", id.clone());
    match wire {
        Wire::Values => {
            map.set("kind", PyValue::str("value"));
            map.set("value", value);
        }
        Wire::JsonText => match value.dumps_ascii() {
            Ok(text) => {
                map.set("encoding", PyValue::str(JSON_VALUES_ENCODING));
                map.set("kind", PyValue::str("value"));
                map.set("value_json", PyValue::str(&text));
            }
            Err(error) => {
                let text = error.to_string();
                let message = text
                    .strip_prefix(&format!("{}: ", error.python_type()))
                    .unwrap_or(&text)
                    .to_string();
                return exception_record(id, &Failure::new(error.python_type(), &message), wire);
            }
        },
    }
    PyValue::Dict(map)
}

fn transport_error(message: &str) -> Failure {
    Failure::new("TypeError", message)
}

fn load_failure(error: &LoadError) -> Failure {
    let text = error.to_string();
    match error {
        LoadError::TooDeep { .. } => Failure::new(
            "RecursionError",
            text.strip_prefix("RecursionError: ").unwrap_or(&text),
        ),
        LoadError::Decode { .. } => Failure::new("JSONDecodeError", &text),
        LoadError::Encoding(_) => Failure::new("UnicodeDecodeError", &text),
        LoadError::IntegerTooLong { .. } => Failure::new("ValueError", &text),
    }
}

/// The arguments of a row: `arguments` (nested JSON) or `arguments_json` (each argument a raw JSON text decoded on
/// its own); the two are mutually exclusive.
fn row_arguments(row: &PyDict) -> Result<PyDict, Failure> {
    match (row.get("arguments"), row.get("arguments_json")) {
        (Some(_), Some(_)) => Err(transport_error(
            "arguments and arguments_json are mutually exclusive",
        )),
        (Some(PyValue::Dict(arguments)), None) => Ok(arguments.clone()),
        (Some(other), None) => Err(transport_error(&format!(
            "arguments must be an object, got {}",
            other.type_name()
        ))),
        (None, Some(PyValue::Dict(texts))) => {
            let mut decoded = PyDict::new();
            for (name, text) in texts.iter() {
                let PyValue::Str(text) = text else {
                    return Err(transport_error(&format!(
                        "arguments_json values must be JSON texts, got {}",
                        text.type_name()
                    )));
                };
                decoded.insert(
                    name.clone(),
                    loads_text(text).map_err(|error| load_failure(&error))?,
                );
            }
            Ok(decoded)
        }
        (None, Some(other)) => Err(transport_error(&format!(
            "arguments_json must be an object, got {}",
            other.type_name()
        ))),
        (None, None) => Ok(PyDict::new()),
    }
}

fn evaluate_row(row: &PyValue, runtime: &Runtime) -> PyValue {
    let PyValue::Dict(map) = row else {
        return exception_record(
            PyValue::None,
            &transport_error("operation row is not an object"),
            Wire::Values,
        );
    };
    let id = map.get("id").cloned().unwrap_or(PyValue::None);
    let wire = match map.get("result_json") {
        Some(PyValue::Bool(true)) => Wire::JsonText,
        _ => Wire::Values,
    };
    let outcome = (|| {
        let operation = match map.get("operation") {
            Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
            Some(other) => other.repr(),
            None => return Err(Failure::new("KeyError", "'operation'")),
        };
        evaluate_operation(&operation, &row_arguments(map)?, runtime)
    })();
    match outcome {
        Ok(value) => value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    }
}

fn run_jsonl(runtime: &Runtime, io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[repeat_axis_api] ERROR stdin: {error}\n")),
        );
        return 2;
    }
    // The transport is deliberately strict UTF-8 in and ASCII out; it is not the legacy stream behaviour.
    let text = match decode_file_text(&bytes) {
        Ok(text) => text
            .to_utf8()
            .expect("strict decoding yields no surrogates"),
        Err(error) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[repeat_axis_api] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads(line) {
                Ok(row) => evaluate_row(&row, runtime),
                Err(error) => exception_record(PyValue::None, &load_failure(&error), Wire::Values),
            };
            match record.dumps_ascii() {
                Ok(json) => out.push_str(&json),
                Err(error) => {
                    let id = match &record {
                        PyValue::Dict(map) => map.get("id").cloned().unwrap_or(PyValue::None),
                        _ => PyValue::None,
                    };
                    let text = error.to_string();
                    let message = text
                        .strip_prefix(&format!("{}: ", error.python_type()))
                        .unwrap_or(&text);
                    out.push_str(
                        &exception_record(
                            id,
                            &Failure::new(error.python_type(), message),
                            Wire::Values,
                        )
                        .dumps_ascii()
                        .unwrap_or_default(),
                    );
                }
            }
            out.push('\n');
        }
        out
    });
    match io.print_lines(&PyStr::new(&output)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure),
    }
}

/// Whether a failure is the configuration refusal of a missing repository root (for callers that map it).
pub fn is_configuration_error(failure: &Failure) -> bool {
    failure.class == CONFIGURATION_ERROR
}
