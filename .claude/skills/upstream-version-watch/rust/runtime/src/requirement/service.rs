//! The JSON-lines program of the component (`ABI.md`): `upstream_requirement_api evaluate-jsonl [--input-fd FD]`.
//!
//! One request per line: `{"id", "operation": "packaging24.requirement_main_projection", "input_dto": <typed DTO>, "hash_seed": 0 | 17}`, no other member.
//! One record per request:
//!
//! * business: `{id, scope, projection, input_before, input_after, stdout, stderr}`; `projection` is the typed dict of the captured originals,
//!   `input_before`/`input_after` the typed input around the call, `stdout`/`stderr` the text the construction wrote to those streams;
//! * refusal: `{id, scope, refusal: {kind, detail}}`, for a request this component cannot answer. `kind` is `malformed_request`,
//!   `unsupported_operation`, `unsupported_hash_seed`, `malformed_input_dto` or the name of an [`super::UnsupportedDomain`]. A refusal is never
//!   the business result `accepted: false` and no hash profile is substituted for an unsupported seed.
//!
//! The typed input DTO is the shared one (`crate::dto_decode`), with one widening: the captured two-part set form `["set", members]` (members strictly
//! ascending by compact ASCII JSON, no stated iteration order; membership is all this component observes) is read as `["set", members, "sorted"]`.

use super::projection::{project_requirement, secret_for_seed, SCOPE};
use crate::dto::encode;
use crate::dto_decode::decode;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_traits::ToPrimitive;

/// The one operation of this service.
pub const OPERATION: &str = "packaging24.requirement_main_projection";

const MEMBERS: [&str; 4] = ["id", "operation", "input_dto", "hash_seed"];

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

/// `dto` with every two-part set (`["set", members]`) widened to the three-part form the shared decoder reads, through lists, tuples, sets and dicts.
fn with_two_part_sets_widened(dto: &PyValue) -> PyValue {
    let PyValue::List(parts) = dto else {
        return dto.clone();
    };
    let tag = match parts.first() {
        Some(PyValue::Str(tag)) => tag.to_utf8().unwrap_or_default(),
        _ => return dto.clone(),
    };
    let rewrite_all = |data: &PyValue| match data {
        PyValue::List(items) => {
            PyValue::List(items.iter().map(with_two_part_sets_widened).collect())
        }
        other => other.clone(),
    };
    match (tag.as_str(), parts.as_slice()) {
        ("set", [tag, members]) => PyValue::List(vec![
            tag.clone(),
            rewrite_all(members),
            PyValue::str("sorted"),
        ]),
        ("list" | "tuple" | "set", [tag, members, rest @ ..]) => {
            let mut widened = vec![tag.clone(), rewrite_all(members)];
            widened.extend_from_slice(rest);
            PyValue::List(widened)
        }
        ("dict", [tag, PyValue::List(entries)]) => PyValue::List(vec![
            tag.clone(),
            PyValue::List(entries.iter().map(rewrite_all).collect()),
        ]),
        _ => dto.clone(),
    }
}

/// The hash seed of a request: a JSON integer.
fn seed_of(value: &PyValue) -> Option<u32> {
    match value {
        PyValue::Int(number) => number.to_u32(),
        _ => None,
    }
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
    let Some(seed_value) = members.get("hash_seed") else {
        return refusal(id, "malformed_request", "the request has no hash_seed");
    };
    let Some(secret) = seed_of(seed_value).and_then(secret_for_seed) else {
        return refusal(
            id,
            "unsupported_hash_seed",
            "hash_seed must be the integer 0 or 17: the only two explicit hash profiles this component supports",
        );
    };
    let Some(dto) = members.get("input_dto") else {
        return refusal(id, "malformed_request", "the request has no input_dto");
    };
    let input = match decode(&with_two_part_sets_widened(dto)) {
        Ok(input) => input,
        Err(problem) => {
            return refusal(id, "malformed_input_dto", &format!("input_dto: {problem}"))
        }
    };
    let Ok(input_before) = encode(&input) else {
        return refusal(
            id,
            "malformed_input_dto",
            "input_dto is outside the typed profile",
        );
    };
    let observation = match project_requirement(&input, &secret) {
        Ok(observation) => observation,
        Err(domain) => return refusal(id, domain.name(), domain.detail()),
    };
    let Ok(input_after) = encode(&input) else {
        return refusal(
            id,
            "malformed_input_dto",
            "input is outside the typed profile after the call",
        );
    };
    let Ok(projection) = encode(&observation.projection.to_value()) else {
        return refusal(
            id,
            "malformed_request",
            "the projection is outside the typed profile",
        );
    };
    let mut row = PyDict::new();
    row.set("id", id);
    row.set("scope", PyValue::str(SCOPE));
    row.set("projection", projection);
    row.set("input_before", input_before);
    row.set("input_after", input_after);
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(&observation.stderr));
    PyValue::Dict(row)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    crate::http_direct::jsonl::evaluate_lines(input, &answer)
}
