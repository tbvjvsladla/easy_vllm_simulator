//! `recipe_transform_api`: `rope_scaling_translate.translate` and `parse_vllm_log.parse_log` as a JSON-lines service, wire-compatible
//! with the other recipe services.
//!
//! * `constants`: the module constants of both owners, keyed by owner: `rope_scaling_translate` = `{ROPE_CONTAINERS,
//!   EXTENDABLE_ROPE_TYPES}`, `parse_vllm_log` = `{_NUM, _GIB}` (the two regex pieces; they are fixed and cannot be replaced).
//! * `evaluate-jsonl`: one request per line `{id, operation, args, kwargs, globals?}` with `operation` one of
//!   `rope_scaling_translate.translate` and `parse_vllm_log.parse_log`; one record per request: `{id, kind: "value", value, stdout,
//!   stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message, exception_chain, stdout, stderr, args_after,
//!   kwargs_after}`. `globals` is a member of its own (never a business keyword): for `translate` it replaces `ROPE_CONTAINERS` and/or
//!   `EXTENDABLE_ROPE_TYPES`; `parse_log` takes none. A request carries no other member. Neither function reads a file, the
//!   environment or the network, and neither writes: `stdout` and `stderr` of a record are empty.

use crate::escalation::Raised;
use crate::memory_api::{parse_request, Request};
use crate::model_api::encode;
use crate::rope_translate::{translate, Profile, MODULE as ROPE_MODULE};
use crate::vllm_log::{parse_log, GIB, MODULE as LOG_MODULE, NUM};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{optional, required, Param, Signature};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

fn none_value() -> PyValue {
    PyValue::None
}

const TRANSLATE_PARAMS: &[Param] = &[
    required("cfg"),
    required("target_len"),
    optional("factor", none_value),
];

const PARSE_PARAMS: &[Param] = &[required("text")];

/// The functions of the two owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    Translate,
    ParseLog,
}

/// The module constants of both owners, keyed by owner.
pub fn constants() -> PyValue {
    let mut log = PyDict::new();
    log.set("_NUM", PyValue::str(NUM));
    log.set("_GIB", PyValue::str(GIB));
    let mut all = PyDict::new();
    all.set(ROPE_MODULE, Profile::canonical().to_value());
    all.set(LOG_MODULE, PyValue::Dict(log));
    PyValue::Dict(all)
}

/// Resolves `module.function` of the two owners.
pub fn resolve(operation: &str) -> Result<Function, Raised> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    let (function, expected) = match module {
        ROPE_MODULE => (Function::Translate, "translate"),
        LOG_MODULE => (Function::ParseLog, "parse_log"),
        _ => {
            return Err(Raised::plain(
                "ModuleNotFoundError",
                &format!("No module named '{module}'"),
            ))
        }
    };
    if name != expected {
        return Err(Raised::plain(
            "AttributeError",
            &format!("module '{module}' has no attribute '{name}'"),
        ));
    }
    Ok(function)
}

/// Binds the arguments as Python does and runs the function.
pub fn call(
    function: Function,
    profile: &Profile,
    args: &[PyValue],
    kwargs: &PyDict,
) -> Result<PyDict, Raised> {
    match function {
        Function::Translate => {
            let signature = Signature {
                name: "translate",
                params: TRANSLATE_PARAMS,
                rest: false,
            };
            let bound = signature.bind(args, kwargs).map_err(Raised::from)?;
            translate(profile, &bound[0], &bound[1], &bound[2]).map_err(Raised::from)
        }
        Function::ParseLog => {
            let signature = Signature {
                name: "parse_log",
                params: PARSE_PARAMS,
                rest: false,
            };
            let bound = signature.bind(args, kwargs).map_err(Raised::from)?;
            parse_log(&bound[0]).map_err(Raised::from)
        }
    }
}

/// One answer record: the outcome with the streams and the arguments after the call.
pub fn record(
    id: PyValue,
    outcome: Result<PyDict, Raised>,
    args: Vec<PyValue>,
    kwargs: PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(&PyValue::Dict(value)));
        }
        Err(raised) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(&raised.class));
            row.set("message", PyValue::Str(raised.message.clone()));
            row.set("exception_chain", raised.chain_value());
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
        Err(Raised::from(Failure::new("RequestError", message))),
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

/// The globals a request names (`globals`) on top of the canonical ones; `Err` is the finished record.
fn profile_of(request: &Request) -> Result<Profile, PyValue> {
    let mut profile = Profile::canonical();
    if let Some(given) = request.members.get("globals") {
        if operation_of(request).split('.').next() == Some(LOG_MODULE) {
            return Err(request_record(
                request.id.clone(),
                "parse_vllm_log has no replaceable globals",
            ));
        }
        profile
            .replace_from(given)
            .map_err(|problem| request_record(request.id.clone(), &problem))?;
    }
    Ok(profile)
}

/// Answers one request line.
pub fn answer(line: &PyStr) -> PyValue {
    let request = match parse_request(line, &["globals"]) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let profile = match profile_of(&request) {
        Ok(profile) => profile,
        Err(row) => return row,
    };
    let outcome = resolve(&operation_of(&request))
        .and_then(|function| call(function, &profile, &request.args, &request.kwargs));
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
        out.extend_from_slice(render(&answer(&line))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
