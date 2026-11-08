//! `recipe_simlog_api`: the five `simlog_writer` functions as a JSON-lines service, wire-compatible with the other recipe services.
//!
//! * `constants`: the module's profile and the formats it writes (`W/ABI.md`).
//! * `evaluate-jsonl`: one request per line `{id, operation, args, kwargs, globals?}` with `operation` one of
//!   `simlog_writer.{new_run_dir, _trial_prefix, write_trial, append_correction, write_summary}`; one record per request:
//!   `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message, exception_chain,
//!   stdout, stderr, args_after, kwargs_after}`. `globals` is a member of its own (never a business keyword): its only member is
//!   `yaml_enabled`, a bool (absent = `true`, the module's default PyYAML; `false` = the original's `_yaml = None`). A row of the
//!   chain whose type is `UnicodeEncodeError` also carries `constructor_args`, the five arguments of the builtin class.
//!
//! The paths of a request are used as given, with the process's own working directory and umask; the writes are the real ones.

use crate::escalation::Raised;
use crate::memory_api::{parse_request, Request};
use crate::model_api::encode;
use crate::simlog::{call, Done, Function, Host, Plain, Thrown, MODULE, YAML_VERSION};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The module's profile and formats.
pub fn constants() -> PyValue {
    let mut globals = PyDict::new();
    globals.set("yaml_enabled", PyValue::Bool(true));
    globals.set("yaml_version", PyValue::str(YAML_VERSION));
    let mut yaml = PyDict::new();
    yaml.set("emitter", PyValue::str("PyYAML 6.0.3 SafeDumper"));
    yaml.set("allow_unicode", PyValue::Bool(true));
    yaml.set("sort_keys", PyValue::Bool(false));
    yaml.set("default_flow_style", PyValue::Bool(false));
    yaml.set("indent", PyValue::int(2));
    yaml.set("width", PyValue::int(80));
    yaml.set("line_break", PyValue::str("\n"));
    let mut json = PyDict::new();
    json.set("ensure_ascii", PyValue::Bool(false));
    json.set("indent", PyValue::int(2));
    let mut line = PyDict::new();
    line.set("ensure_ascii", PyValue::Bool(false));
    line.set(
        "separators",
        PyValue::List(vec![PyValue::str(", "), PyValue::str(": ")]),
    );
    let mut formats = PyDict::new();
    formats.set("candidate_yaml", PyValue::Dict(yaml));
    formats.set("json", PyValue::Dict(json));
    formats.set("append_line", PyValue::Dict(line));
    let mut module = PyDict::new();
    module.set("globals", PyValue::Dict(globals));
    module.set("formats", PyValue::Dict(formats));
    let mut all = PyDict::new();
    all.set(MODULE, PyValue::Dict(module));
    PyValue::Dict(all)
}

/// Resolves `simlog_writer.function`.
pub fn resolve(operation: &str) -> Result<Function, Raised> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Raised::plain(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    Function::named(name).ok_or_else(|| {
        Raised::plain(
            "AttributeError",
            &format!("module '{module}' has no attribute '{name}'"),
        )
    })
}

/// The `exception_chain` of a thrown exception: the chain rows, each `UnicodeEncodeError` row with its constructor arguments.
pub fn chain_of(thrown: &Thrown) -> PyValue {
    let mut chain = thrown.raised.chain_value();
    if let PyValue::List(rows) = &mut chain {
        for (index, row) in rows.iter_mut().enumerate() {
            let Some(Some(args)) = thrown.row_args.get(index) else {
                continue;
            };
            let PyValue::Dict(row) = row else { continue };
            if matches!(row.get("type"), Some(PyValue::Str(name)) if name.equals("UnicodeEncodeError"))
            {
                row.set(
                    "constructor_args",
                    PyValue::List(vec![
                        PyValue::str("utf-8"),
                        PyValue::Str(args.object.clone()),
                        PyValue::int(args.start),
                        PyValue::int(args.end),
                        PyValue::str("surrogates not allowed"),
                    ]),
                );
            }
        }
    }
    chain
}

/// One answer record: the outcome with the streams and the arguments after the call.
pub fn record(id: PyValue, outcome: Done<PyValue>, args: Vec<PyValue>, kwargs: PyDict) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(&value));
        }
        Err(failure) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(&failure.raised.class));
            row.set("message", PyValue::Str(failure.raised.message.clone()));
            row.set("exception_chain", chain_of(&failure));
        }
    }
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(""));
    row.set("args_after", encode(&PyValue::List(args)));
    row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    PyValue::Dict(row)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Thrown::plain(Raised::from(Failure::new(
            "RequestError",
            message,
        )))),
        Vec::new(),
        PyDict::new(),
    )
}

fn operation_of(request: &Request) -> String {
    match request.members.get("operation") {
        Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
        _ => String::new(),
    }
}

/// The `yaml_enabled` a request names (`globals`), or the default; `Err` is the finished record.
fn yaml_of(request: &Request) -> Result<bool, PyValue> {
    let Some(given) = request.members.get("globals") else {
        return Ok(true);
    };
    let PyValue::Dict(members) = given else {
        return Err(request_record(
            request.id.clone(),
            "globals must be an object",
        ));
    };
    let mut enabled = true;
    for (name, value) in members.iter() {
        match (name.to_utf8().as_deref(), value) {
            (Some("yaml_enabled"), PyValue::Bool(flag)) => enabled = *flag,
            (Some("yaml_enabled"), _) => {
                return Err(request_record(
                    request.id.clone(),
                    "yaml_enabled must be a bool",
                ))
            }
            _ => {
                return Err(request_record(
                    request.id.clone(),
                    &format!("globals member {} is not usable", name.repr()),
                ))
            }
        }
    }
    Ok(enabled)
}

/// Answers one request line (the writes it asks for are made).
pub fn answer(line: &PyStr, host: &mut dyn Host) -> PyValue {
    let request = match parse_request(line, &["globals"]) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let yaml_enabled = match yaml_of(&request) {
        Ok(flag) => flag,
        Err(row) => return row,
    };
    let outcome = match resolve(&operation_of(&request)) {
        Ok(function) => call(function, host, yaml_enabled, &request.args, &request.kwargs),
        Err(raised) => Err(Thrown::plain(raised)),
    };
    record(request.id, outcome, request.args, request.kwargs)
}

/// One output line of a record.
pub fn render(row: &PyValue) -> Result<String, Failure> {
    crate::api::render(row)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(render(&answer(&line, &mut Plain))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
