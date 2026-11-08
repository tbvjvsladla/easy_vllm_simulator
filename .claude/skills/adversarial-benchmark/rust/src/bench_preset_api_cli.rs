//! `bench_preset_api`: the Rust-only surface of the preset contract, a separate program so that the `bench_preset`
//! command line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - every public and needed private operation of the original module as a differential and bridge
//!   transport. Each stdin line is `{id, operation, arguments}`; each yields one line `{id, kind: value, value}` or
//!   `{id, kind: exception, type, message}`. Operations: `validate_preset`, `load_bindings`, `preset_path`,
//!   `load_preset`, `serving_caps`, `check_dataset_runtime`, `dataset_cross_problems`, `_dataset_shape_problem`,
//!   `_parse_cli_value`, `_norm`, `_parse_knob_reasons`, `_parse_pins`, `_campaign_pins`, `_grep_yaml_int`, `resolve`
//!   and `constants`. Arguments follow the Python signatures (names, defaults, keyword-only parameters and the
//!   `TypeError` a wrong call raises). Operations that read the repository take an optional `repo_root` (else
//!   `BENCH_PRESET_REPO_ROOT`, else `REPEAT_AXIS_REPO_ROOT`), operations that read the skill directory an optional
//!   `skill_dir` (else `BENCH_PRESET_SKILL_DIR`); path arguments are relative to the process's working directory.
//!   Opt-in: `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the result
//!   as compact ASCII JSON text in `value_json`), so the envelope never adds depth to a deep document.
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `native-help` - this list.

use crate::bench_preset::{
    campaign_pins, check_dataset_runtime, dataset_cross_problems, dataset_shape_problem,
    grep_yaml_int, load_bindings, load_preset, parse_cli_value, parse_knob_reasons, parse_pins,
    preset_path, repo_root_from_environment, resolve, serving_caps, validate_preset, CallDepth,
    ResolveArguments, SkillDir, BINDINGS_REL, CAP_INPUT_PLUS_OUTPUT, CAP_LE_MAX_NUM_SEQS, CAP_NONE,
    COLUMN_MAP_KEYS, DATASET_KINDS, DATASET_KNOBS, DATASET_SOURCES, DATASET_SYNTHETIC,
    DEFAULT_PRESET, DEFAULT_REASON, FULL_BENCH, GRADES, HUMAN_PIN_REASON, KNOB_KEYS, KNOB_SPECS,
    KNOWN_TOOLS, OPTION_KEYS, OWNERS, PREPROCESS_STRATEGIES, SCHEMA_VERSION, TOP_KEYS,
    VERDICT_LEVEL,
};
use crate::bench_preset_cli::Runtime;
use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, print_text, row_arguments, strings, value_record,
    Bound, Signature, Wire,
};
use crate::repeat_owner::RepoRoot;
use crate::FULL_REPEATS_MIN;

const NATIVE_HELP: &str = "bench_preset_api Rust-only commands:\n  bench_preset_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  bench_preset_api constants        the module constants as one JSON object\n  bench_preset_api native-help      this text\n";

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

/// The constants of the original module. The repeat floor is the one compiled in from the campaign schema.
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    map.set("BINDINGS_REL", PyValue::str(BINDINGS_REL));
    map.set("FULL_BENCH", PyValue::str(FULL_BENCH));
    map.set("DEFAULT_PRESET", PyValue::str(DEFAULT_PRESET));
    map.set("SCHEMA_VERSION", PyValue::int(SCHEMA_VERSION));
    map.set("GRADES", strings(&GRADES));
    map.set("OWNERS", strings(&OWNERS));
    map.set("CAP_LE_MAX_NUM_SEQS", PyValue::str(CAP_LE_MAX_NUM_SEQS));
    map.set("CAP_INPUT_PLUS_OUTPUT", PyValue::str(CAP_INPUT_PLUS_OUTPUT));
    map.set("CAP_NONE", PyValue::str(CAP_NONE));
    map.set("HUMAN_PIN_REASON", PyValue::str(HUMAN_PIN_REASON));
    map.set("DEFAULT_REASON", PyValue::str(DEFAULT_REASON));
    map.set("VERDICT_LEVEL", PyValue::int(VERDICT_LEVEL));
    map.set("TOP_KEYS", strings(&TOP_KEYS));
    map.set("KNOB_KEYS", strings(&KNOB_KEYS));
    map.set("OPTION_KEYS", strings(&OPTION_KEYS));
    let mut specs = PyDict::new();
    let mut flags = PyDict::new();
    let mut shell_keys = PyDict::new();
    for spec in &KNOB_SPECS {
        specs.set(
            spec.name,
            PyValue::List(vec![
                PyValue::str(spec.kind),
                PyValue::str(spec.value_type.name()),
                strings(spec.caps),
                PyValue::Bool(spec.no_default),
            ]),
        );
        let flag = match spec.flag {
            Some(flag) => flag.to_string(),
            None => format!("--pin {}", spec.name),
        };
        flags.set(spec.name, PyValue::str(&flag));
        if let Some(key) = spec.shell_key {
            shell_keys.set(spec.name, PyValue::str(key));
        }
    }
    map.set("KNOB_SPECS", PyValue::Dict(specs));
    map.set("DATASET_KNOBS", strings(&DATASET_KNOBS));
    map.set("DATASET_SYNTHETIC", PyValue::str(DATASET_SYNTHETIC));
    map.set("DATASET_KINDS", strings(&DATASET_KINDS));
    map.set("DATASET_SOURCES", strings(&DATASET_SOURCES));
    map.set("PREPROCESS_STRATEGIES", strings(&PREPROCESS_STRATEGIES));
    map.set("COLUMN_MAP_KEYS", strings(&COLUMN_MAP_KEYS));
    map.set("KNOWN_TOOLS", strings(&KNOWN_TOOLS));
    map.set("FLAG_OF", PyValue::Dict(flags));
    map.set("SHELL_KEY_OF", PyValue::Dict(shell_keys));
    map.set("FULL_REPEATS_MIN", PyValue::int(FULL_REPEATS_MIN));
    PyValue::Dict(map)
}

// ── Python call binding ─────────────────────────────────────────────────────────

const fn signature(
    function: &'static str,
    positional: &'static [&'static str],
    keyword_only: &'static [&'static str],
    optional: &'static [&'static str],
) -> Signature {
    Signature {
        function,
        positional,
        keyword_only,
        optional,
    }
}

const VALIDATE_PRESET: Signature = signature("validate_preset", &["doc", "bindings"], &[], &[]);
const LOAD_BINDINGS: Signature = signature("load_bindings", &["skill_dir"], &[], &["skill_dir"]);
const PRESET_PATH: Signature = signature(
    "preset_path",
    &["name", "skill_dir", "bindings"],
    &[],
    &["skill_dir", "bindings"],
);
const LOAD_PRESET: Signature = signature(
    "load_preset",
    &["name", "skill_dir"],
    &[],
    &["name", "skill_dir"],
);
const SERVING_CAPS: Signature = signature("serving_caps", &["serving_yaml", "dry_run"], &[], &[]);
const CHECK_DATASET_RUNTIME: Signature = signature("check_dataset_runtime", &["values"], &[], &[]);
const DATASET_CROSS_PROBLEMS: Signature =
    signature("dataset_cross_problems", &["values"], &[], &[]);
const DATASET_SHAPE_PROBLEM: Signature =
    signature("_dataset_shape_problem", &["name", "value"], &[], &[]);
const PARSE_CLI_VALUE: Signature = signature("_parse_cli_value", &["name", "raw"], &[], &[]);
const NORM: Signature = signature("_norm", &["v"], &[], &[]);
const PARSE_KNOB_REASONS: Signature = signature("_parse_knob_reasons", &["items"], &[], &[]);
const PARSE_PINS: Signature = signature("_parse_pins", &["items"], &[], &[]);
const CAMPAIGN_PINS: Signature =
    signature("_campaign_pins", &["campaign_id", "repo_root"], &[], &[]);
const GREP_YAML_INT: Signature = signature("_grep_yaml_int", &["text", "key"], &[], &[]);
const RESOLVE: Signature = signature(
    "resolve",
    &["preset_doc", "preset_name"],
    &[
        "tool",
        "cli",
        "cli_pins",
        "pinned_by",
        "knob_reasons",
        "approved_by",
        "campaign_id",
        "serving",
        "repo_root",
        "repeats_source",
        "dry_run",
    ],
    &[
        "cli_pins",
        "pinned_by",
        "knob_reasons",
        "approved_by",
        "campaign_id",
        "repo_root",
        "repeats_source",
        "dry_run",
    ],
);
const CONSTANTS: Signature = signature("constants", &[], &[], &[]);

/// The skill directory of a call: the argument when given, else the process's configured one.
fn skill_dir_of(call: &Bound<'_>, arguments: &PyDict) -> SkillDir {
    match arguments.get("skill_dir") {
        Some(value) => SkillDir::from_value(value),
        None => {
            let _ = call;
            SkillDir::from_environment()
        }
    }
}

/// The repository root of a call: the argument when given, else the process's configured one.
fn repo_root_of(arguments: &PyDict) -> RepoRoot {
    match arguments.get("repo_root") {
        Some(value) => RepoRoot::from_value(value),
        None => repo_root_from_environment(),
    }
}

fn problems_value(problems: Vec<PyStr>) -> PyValue {
    PyValue::List(problems.into_iter().map(PyValue::Str).collect())
}

/// One operation of the module by name, with the arguments as a Python call gives them.
pub fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    runtime: &Runtime,
) -> Result<PyValue, Failure> {
    let _ = runtime;
    match operation {
        "validate_preset" => {
            let call = VALIDATE_PRESET.bind(arguments)?;
            Ok(problems_value(validate_preset(
                &call.get("doc"),
                &call.get("bindings"),
            )?))
        }
        "load_bindings" => {
            let call = LOAD_BINDINGS.bind(arguments)?;
            load_bindings(&skill_dir_of(&call, arguments), CallDepth::API)
        }
        "preset_path" => {
            let call = PRESET_PATH.bind(arguments)?;
            Ok(PyValue::Str(preset_path(
                &call.get("name"),
                &skill_dir_of(&call, arguments),
                &call.get("bindings"),
                CallDepth::API,
            )?))
        }
        "load_preset" => {
            let call = LOAD_PRESET.bind(arguments)?;
            let loaded = load_preset(
                &call.get("name"),
                &skill_dir_of(&call, arguments),
                CallDepth::API,
            )?;
            Ok(PyValue::Tuple(vec![
                PyValue::Str(loaded.path),
                loaded.document,
                loaded.bindings,
            ]))
        }
        "serving_caps" => {
            let call = SERVING_CAPS.bind(arguments)?;
            serving_caps(&call.get("serving_yaml"), &call.get("dry_run"))
        }
        "check_dataset_runtime" => {
            let call = CHECK_DATASET_RUNTIME.bind(arguments)?;
            check_dataset_runtime(&call.get("values"))
        }
        "dataset_cross_problems" => {
            let call = DATASET_CROSS_PROBLEMS.bind(arguments)?;
            Ok(problems_value(dataset_cross_problems(&call.get("values"))?))
        }
        "_dataset_shape_problem" => {
            let call = DATASET_SHAPE_PROBLEM.bind(arguments)?;
            let name = call.get("name");
            let name_text = match name.peel() {
                PyValue::Str(text) => text.to_utf8().unwrap_or_default(),
                other => return Err(Failure::with_message("KeyError", PyStr::new(&other.repr()))),
            };
            Ok(dataset_shape_problem(&name_text, &call.get("value"))?
                .map_or(PyValue::None, PyValue::Str))
        }
        "_parse_cli_value" => {
            let call = PARSE_CLI_VALUE.bind(arguments)?;
            parse_cli_value(&call.get("name"), &call.get("raw"))
        }
        "_norm" => {
            let call = NORM.bind(arguments)?;
            crate::bench_preset::norm_value(&call.get("v"))
        }
        "_parse_knob_reasons" => {
            let call = PARSE_KNOB_REASONS.bind(arguments)?;
            Ok(PyValue::Dict(parse_knob_reasons(&call.get("items"))?))
        }
        "_parse_pins" => {
            let call = PARSE_PINS.bind(arguments)?;
            Ok(PyValue::Dict(parse_pins(&call.get("items"))?))
        }
        "_campaign_pins" => {
            let call = CAMPAIGN_PINS.bind(arguments)?;
            let found = campaign_pins(
                &call.get("campaign_id"),
                &repo_root_of(arguments),
                &runtime.owner,
            )?;
            Ok(PyValue::Tuple(vec![
                found.campaign.unwrap_or(PyValue::None),
                PyValue::Dict(found.pins),
                PyValue::Str(found.note),
            ]))
        }
        "_grep_yaml_int" => {
            let call = GREP_YAML_INT.bind(arguments)?;
            let text = match call.get("text").peel() {
                PyValue::Str(text) => text.clone(),
                other => {
                    return Err(Failure::new(
                        "TypeError",
                        &format!(
                            "expected string or bytes-like object, got '{}'",
                            other.type_name()
                        ),
                    ))
                }
            };
            let key = call.get("key").display_text().to_utf8().unwrap_or_default();
            grep_yaml_int(&text, &key)
        }
        "resolve" => {
            let call = RESOLVE.bind(arguments)?;
            let (preset_doc, preset_name) = (call.get("preset_doc"), call.get("preset_name"));
            let (tool, cli, serving) = (call.get("tool"), call.get("cli"), call.get("serving"));
            let (cli_pins, pinned_by, knob_reasons) = (
                call.get("cli_pins"),
                call.get("pinned_by"),
                call.get("knob_reasons"),
            );
            let (approved_by, campaign_id, repeats_source) = (
                call.get("approved_by"),
                call.get("campaign_id"),
                call.get("repeats_source"),
            );
            let dry_run = call.get_or("dry_run", PyValue::Bool(false));
            let repo_root = repo_root_of(arguments);
            let resolution = resolve(
                &ResolveArguments {
                    preset_doc: &preset_doc,
                    preset_name: &preset_name,
                    tool: &tool,
                    cli: &cli,
                    cli_pins: &cli_pins,
                    pinned_by: &pinned_by,
                    knob_reasons: &knob_reasons,
                    approved_by: &approved_by,
                    campaign_id: &campaign_id,
                    serving: &serving,
                    repo_root: &repo_root,
                    repeats_source: &repeats_source,
                    dry_run: &dry_run,
                },
                &runtime.owner,
            )?;
            Ok(PyValue::Tuple(vec![
                PyValue::Dict(resolution.values),
                PyValue::Dict(resolution.block),
            ]))
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

fn evaluate_row(row: &PyValue, runtime: &Runtime) -> PyValue {
    let PyValue::Dict(map) = row else {
        return exception_record(
            PyValue::None,
            &Failure::new("TypeError", "operation row is not an object"),
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
        evaluate_operation(&operation, &row_arguments(map, true)?, runtime)
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
            &PyStr::new(&format!("[bench_preset_api] ERROR stdin: {error}\n")),
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
                &PyStr::new(&format!("[bench_preset_api] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads_objects(line) {
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
