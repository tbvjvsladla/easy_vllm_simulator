//! `recipe_numeric_api`: the numeric functions as a JSON-lines service for callers that keep their own process.
//!
//! `constants` prints the original modules' constants as one JSON object. `evaluate-jsonl` reads one request object per
//! line (`{id, operation: "module.function", args, kwargs}`) and writes exactly one record per request, in order:
//! `{id, kind: "value", value, stdout, stderr}` or `{id, kind: "exception", type, message, stdout, stderr}`. A tuple
//! result is `{"__tuple__": [...]}`; every other value is plain JSON (NaN, Infinity, large integers and lone surrogates
//! as the common reader and writer carry them). A request that is not a usable request object is answered by a record
//! of kind `exception` with a `RequestError` class, never skipped and never mixed with a function's own exception.

use crate::operations::resolve;
use crate::{estimate_vram, quant_table};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The constants object (`quant_table` constants plus the two of `estimate_vram`).
pub fn constants() -> PyValue {
    let mut all = quant_table::constants();
    all.set("GIB", PyValue::int(estimate_vram::GIB));
    all.set(
        "KV_BLOCK_ALIGN_BUFFER",
        PyValue::Float(estimate_vram::KV_BLOCK_ALIGN_BUFFER),
    );
    PyValue::Dict(all)
}

/// Tuples become `{"__tuple__": [...]}`, recursively.
fn encode(value: &PyValue) -> PyValue {
    match value {
        PyValue::Tuple(items) => {
            let mut tagged = PyDict::new();
            tagged.set(
                "__tuple__",
                PyValue::List(items.iter().map(encode).collect()),
            );
            PyValue::Dict(tagged)
        }
        PyValue::List(items) => PyValue::List(items.iter().map(encode).collect()),
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, item) in map.iter() {
                copy.insert(key.clone(), encode(item));
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

fn record(id: PyValue, outcome: Result<PyValue, Failure>) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(&value));
        }
        Err(failure) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(failure.class));
            row.set("message", PyValue::Str(failure.message));
        }
    }
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(""));
    PyValue::Dict(row)
}

fn request_error(message: &str) -> Failure {
    Failure::new("RequestError", message)
}

/// Answers one request line.
pub fn answer(line: &PyStr) -> PyValue {
    let request = match loads_text(line) {
        Ok(request) => request,
        Err(error) => {
            return record(
                PyValue::None,
                Err(request_error(&format!("not JSON: {error}"))),
            )
        }
    };
    let PyValue::Dict(members) = &request else {
        return record(
            PyValue::None,
            Err(request_error("the request is not an object")),
        );
    };
    let Some(id) = members.get("id").cloned() else {
        return record(PyValue::None, Err(request_error("the request has no id")));
    };
    let outcome = (|| -> Result<PyValue, Failure> {
        let Some(PyValue::Str(operation)) = members.get("operation") else {
            return Err(request_error(
                "operation must be a string 'module.function'",
            ));
        };
        let args = match members.get("args") {
            None => Vec::new(),
            Some(PyValue::List(items)) => items.clone(),
            Some(_) => return Err(request_error("args must be an array")),
        };
        let kwargs = match members.get("kwargs") {
            None => PyDict::new(),
            Some(PyValue::Dict(map)) => map.clone(),
            Some(_) => return Err(request_error("kwargs must be an object")),
        };
        let function = resolve(&operation.to_utf8().unwrap_or_default())?;
        function.call(&args, &kwargs)
    })();
    record(id, outcome)
}

/// One output line of a record.
pub fn render(row: &PyValue) -> Result<String, Failure> {
    row.dumps_ascii()
        .map_err(|error| Failure::new(error.python_type(), &error.to_string()))
}
