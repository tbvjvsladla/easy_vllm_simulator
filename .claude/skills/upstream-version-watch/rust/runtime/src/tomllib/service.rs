//! The JSON-lines program of the component (`ABI.md`): `upstream_toml_api evaluate-jsonl [--input-fd FD]`.
//!
//! One request per line, exactly three members: `{"id", "operation": "tomllib.loads.default", "input_dto": ["str", text]}`. The reference run is
//! `PYTHONHASHSEED=0`; the parser's result does not depend on the hash seed (tables keep insertion order), so a request carries no seed.
//! One record per request:
//!
//! * business: `{id, scope, projection, input_before, input_after, stdout, stderr}` with exactly these seven members; `projection` is
//!   `{"kind": "value", "value": <typed TOML value>}` or `{"kind": "exception", type, module, message, args, exception_chain}` (see
//!   [`super::projection`]), `input_before`/`input_after` the typed input around the call, `stdout`/`stderr` the text the call wrote (always empty);
//! * refusal: `{id, scope, refusal: {kind, detail}}` with no projection, for a request this component cannot answer. `kind` is `malformed_request`,
//!   `unsupported_operation`, `malformed_input_dto`, `non_text_input` or the name of an [`super::UnsupportedDomain`]. A refusal is never the business
//!   result of a parse.

use super::projection::{observe_loads, SCOPE};
use crate::dto::encode;
use crate::dto_decode::decode;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The one operation of this service.
pub const OPERATION: &str = "tomllib.loads.default";

const MEMBERS: [&str; 3] = ["id", "operation", "input_dto"];

fn refusal(id: PyValue, kind: &str, detail: &str) -> PyValue {
    let mut why = PyDict::new();
    why.set("kind", PyValue::str(kind));
    why.set("detail", PyValue::str(detail));
    let mut row = PyDict::new();
    row.set("id", id);
    row.set("scope", PyValue::str(SCOPE));
    row.set("refusal", PyValue::Dict(why));
    PyValue::Dict(row)
}

/// Answers one request line.
pub fn answer(line: &PyStr) -> PyValue {
    let request = match loads_text_objects(line) {
        Ok(request) => request,
        Err(error) => {
            return refusal(
                PyValue::None,
                "malformed_request",
                &format!("not JSON: {error}"),
            )
        }
    };
    let PyValue::Dict(members) = request else {
        return refusal(
            PyValue::None,
            "malformed_request",
            "the request is not an object",
        );
    };
    let Some(id) = members.get("id").cloned() else {
        return refusal(PyValue::None, "malformed_request", "the request has no id");
    };
    if let Some((key, _)) = members
        .iter()
        .find(|(key, _)| !MEMBERS.iter().any(|name| key.equals(name)))
    {
        return refusal(
            id,
            "malformed_request",
            &format!(
                "the request has a member this service does not take: {}",
                key.repr()
            ),
        );
    }
    match members.get("operation") {
        Some(PyValue::Str(operation)) if operation.equals(OPERATION) => {}
        Some(PyValue::Str(operation)) => {
            return refusal(
                id,
                "unsupported_operation",
                &format!("operation must be '{OPERATION}', not {}", operation.repr()),
            )
        }
        _ => return refusal(id, "malformed_request", "operation must be a string"),
    }
    let Some(dto) = members.get("input_dto") else {
        return refusal(id, "malformed_request", "the request has no input_dto");
    };
    let input = match decode(dto) {
        Ok(input) => input,
        Err(problem) => {
            return refusal(id, "malformed_input_dto", &format!("input_dto: {problem}"))
        }
    };
    let PyValue::Str(text) = input.peel() else {
        return refusal(
            id,
            "non_text_input",
            "input_dto must be [\"str\", text]: the parser consumes the text its fetch returns, so no other argument type is modelled",
        );
    };
    let Ok(input_before) = encode(&input) else {
        return refusal(
            id,
            "malformed_input_dto",
            "input_dto is outside the typed profile",
        );
    };
    let projection = match observe_loads(text) {
        Ok(projection) => projection,
        Err(domain) => return refusal(id, domain.name(), domain.detail()),
    };
    let Ok(input_after) = encode(&input) else {
        return refusal(
            id,
            "malformed_input_dto",
            "input is outside the typed profile after the call",
        );
    };
    let mut row = PyDict::new();
    row.set("id", id);
    row.set("scope", PyValue::str(SCOPE));
    row.set("projection", projection);
    row.set("input_before", input_before);
    row.set("input_after", input_after);
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(""));
    PyValue::Dict(row)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    crate::http_direct::jsonl::evaluate_lines(input, &answer)
}
