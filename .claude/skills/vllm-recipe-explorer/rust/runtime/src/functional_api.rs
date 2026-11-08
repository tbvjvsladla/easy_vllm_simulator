//! `recipe_functional_api`: the six `functional_smoke` operations as a JSON-lines service, wire-compatible with the other recipe services.
//!
//! * `constants`: the four module constants, keyed by owner.
//! * `evaluate-jsonl`: one request per line `{id, operation, args, kwargs, globals?}` with `operation` one of
//!   `functional_smoke.{_post_chat, _first_choice, _check_completion, _check_reasoning, _check_tool_call, smoke}`; one record per request:
//!   `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message, exception_chain,
//!   stdout, stderr, args_after, kwargs_after}`. `globals` is a member of its own (never a business keyword): its members may only be the
//!   four module constants (any JSON value replaces one for the call). The HTTP exchange of `_post_chat`/`smoke` is real.

use crate::escalation::Raised;
use crate::functional_smoke::{call, Constants, Function, Observe, Plain, MODULE};
use crate::memory_api::{parse_request, Request};
use crate::model_api::encode;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The module constants, keyed by owner.
pub fn constants() -> PyValue {
    let mut all = PyDict::new();
    all.set(MODULE, Constants::canonical().to_value());
    PyValue::Dict(all)
}

/// Resolves `functional_smoke.function`.
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

/// One answer record: the outcome with the streams and the arguments after the call.
pub fn record(
    id: PyValue,
    outcome: Result<PyValue, Raised>,
    args: Vec<PyValue>,
    kwargs: PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(&value));
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

/// The constants a request names (`globals`) on top of the canonical ones; `Err` is the finished record.
fn constants_of(request: &Request) -> Result<Constants, PyValue> {
    let mut constants = Constants::canonical();
    if let Some(given) = request.members.get("globals") {
        constants
            .replace_from(given)
            .map_err(|problem| request_record(request.id.clone(), &problem))?;
    }
    Ok(constants)
}

/// Answers one request line (the HTTP it asks for is made).
pub fn answer(line: &PyStr, observer: &mut dyn Observe) -> PyValue {
    let request = match parse_request(line, &["globals"]) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let constants = match constants_of(&request) {
        Ok(constants) => constants,
        Err(row) => return row,
    };
    let outcome = resolve(&operation_of(&request)).and_then(|function| {
        call(
            function,
            observer,
            &constants,
            &request.args,
            &request.kwargs,
        )
    });
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
