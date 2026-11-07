//! The two functions of `resolve_torch_pin.py` that stage A ports (`norm_prefix`, `pick_bound`) as a JSON-lines service (`ABI.md` of the R35 round):
//! one request per line `{id, operation, args_dto?, kwargs_dto?}` with `operation` `resolve_torch_pin.<function>`; one record per request, with the
//! typed value (or the exception with its whole chain and constructor arguments), `input_ref` (the origin of an original-input return), the two
//! streams (always empty: the functions print nothing) and the arguments after the call (never modified).

use crate::dto::encode;
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::torch_pin::{norm_prefix, pick_bound, Raised, Returned, MODULE};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The two functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    NormPrefix,
    PickBound,
}

impl Function {
    pub fn named(name: &str) -> Option<Function> {
        Some(match name {
            "norm_prefix" => Function::NormPrefix,
            "pick_bound" => Function::PickBound,
            _ => return None,
        })
    }

    /// Runs the function: the typed result of the call.
    pub fn call(self, args: &[PyValue], kwargs: &PyDict) -> Result<Returned, Raised> {
        match self {
            Function::NormPrefix => norm_prefix(args, kwargs),
            Function::PickBound => pick_bound(args, kwargs),
        }
    }
}

/// Resolves `resolve_torch_pin.function` (`Err` is the exception the failed lookup raises).
pub fn resolve_operation(operation: &str) -> Result<Function, Raised> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Raised::builtin(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    Function::named(name).ok_or_else(|| {
        Raised::builtin(
            "AttributeError",
            &format!("module '{module}' has no attribute '{name}'"),
        )
    })
}

/// A request that passed the shape checks.
pub struct Request {
    pub id: PyValue,
    pub operation: String,
    pub args: Vec<PyValue>,
    pub kwargs: PyDict,
}

fn refused(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Raised::builtin("RequestError", message)),
        &[],
        &PyDict::new(),
    )
}

/// Reads one request line. `Err` is the finished record to answer with.
pub fn parse_request(line: &PyStr) -> Result<Request, PyValue> {
    let request = match loads_text_objects(line) {
        Ok(request) => request,
        Err(error) => return Err(refused(PyValue::None, &format!("not JSON: {error}"))),
    };
    let PyValue::Dict(members) = request else {
        return Err(refused(PyValue::None, "the request is not an object"));
    };
    let Some(id) = members.get("id").cloned() else {
        return Err(refused(PyValue::None, "the request has no id"));
    };
    for (key, _) in members.iter() {
        let known = ["id", "operation", "args_dto", "kwargs_dto"]
            .iter()
            .any(|name| key.equals(name));
        if !known {
            return Err(refused(
                id,
                &format!(
                    "the request has a member this service does not take: {}",
                    key.repr()
                ),
            ));
        }
    }
    let Some(PyValue::Str(operation)) = members.get("operation") else {
        return Err(refused(id, "operation must be a string 'module.function'"));
    };
    let args = match members.get("args_dto") {
        None => Vec::new(),
        Some(dto) => decode_args(dto)
            .map_err(|problem| refused(id.clone(), &format!("args_dto: {problem}")))?,
    };
    let kwargs = match members.get("kwargs_dto") {
        None => PyDict::new(),
        Some(dto) => decode_kwargs(dto)
            .map_err(|problem| refused(id.clone(), &format!("kwargs_dto: {problem}")))?,
    };
    Ok(Request {
        id,
        operation: operation.to_utf8().unwrap_or_default(),
        args,
        kwargs,
    })
}

/// The typed record of a returned value: `{value, input_ref}` members as the service writes them (also the benchmark's per-call result).
pub fn typed_result(returned: &Returned) -> Result<(PyValue, PyValue), String> {
    let value = encode(&returned.value)?;
    let origin = match &returned.origin {
        Some(origin) => origin.to_value(),
        None => PyValue::None,
    };
    Ok((value, origin))
}

/// `exception_chain`: one row per exception, the saved originals' shape.
fn chain_value(error: &Raised) -> PyValue {
    let last = error.rows.len() - 1;
    PyValue::List(
        error
            .rows
            .iter()
            .enumerate()
            .map(|(at, row)| {
                let mut out = PyDict::new();
                out.set("type", PyValue::str(&row.class));
                out.set("module", PyValue::str(&row.module));
                out.set("message", PyValue::Str(row.message.clone()));
                out.set("suppress_context", PyValue::Bool(false));
                out.set(
                    "relation",
                    if at == last {
                        PyValue::None
                    } else {
                        PyValue::str("context")
                    },
                );
                out.set(
                    "args",
                    encode(&PyValue::Tuple(row.args.clone())).unwrap_or(PyValue::None),
                );
                PyValue::Dict(out)
            })
            .collect(),
    )
}

/// The answer of a call.
pub fn record(
    id: PyValue,
    outcome: Result<Returned, Raised>,
    args: &[PyValue],
    kwargs: &PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    let shown = outcome.and_then(|returned| {
        typed_result(&returned).map_err(|refusal| Raised::builtin("TypeError", &refusal))
    });
    match shown {
        Ok((value, origin)) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", value);
            row.set("input_ref", origin);
        }
        Err(error) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(&error.rows[0].class));
            row.set("module", PyValue::str(&error.rows[0].module));
            row.set("message", PyValue::Str(error.rows[0].message.clone()));
            row.set("exception_chain", chain_value(&error));
        }
    }
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(""));
    row.set(
        "args_after",
        encode(&PyValue::List(args.to_vec())).unwrap_or(PyValue::None),
    );
    row.set(
        "kwargs_after",
        encode(&PyValue::Dict(kwargs.clone())).unwrap_or(PyValue::None),
    );
    PyValue::Dict(row)
}

/// Answers one request line.
pub fn answer(line: &PyStr) -> PyValue {
    let request = match parse_request(line) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let outcome = match resolve_operation(&request.operation) {
        Err(error) => Err(error),
        Ok(function) => function.call(&request.args, &request.kwargs),
    };
    record(request.id, outcome, &request.args, &request.kwargs)
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
        out.extend_from_slice(crate::api::render(&answer(&line))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
