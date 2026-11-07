//! The five functions of `resolve_ngc_tag.py` as a JSON-lines service (the ABI of `version_api`, one request per line
//! `{id, operation, args_dto?, kwargs_dto?}` with `operation` `resolve_ngc_tag.<function>`): one record per request with the typed value (or the
//! exception with its constructor arguments, its attributes such as `name`/`obj` of an `AttributeError`, and `input_ref`, the origin of an
//! original-input return), the two streams (always empty: the functions print nothing) and the arguments after the call (never modified).
//! `inspect_env` runs the real `docker` found on this process's `PATH`; the timeout is the original's 90 seconds.

use crate::dto::encode;
use crate::ngc_inspect::{Docker, ProcessDocker};
use crate::ngc_tag::{
    gen_candidates, inspect_env, is_tuple_prefix, norm, release_tuple, Call, Raised, Where, MODULE,
};
use crate::torch_pin::Returned;
use crate::version_api::{parse_request, typed_result};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    ReleaseTuple,
    Norm,
    IsTuplePrefix,
    InspectEnv,
    GenCandidates,
}

impl Function {
    pub fn named(name: &str) -> Option<Function> {
        Some(match name {
            "release_tuple" => Function::ReleaseTuple,
            "norm" => Function::Norm,
            "is_tuple_prefix" => Function::IsTuplePrefix,
            "inspect_env" => Function::InspectEnv,
            "gen_candidates" => Function::GenCandidates,
            _ => return None,
        })
    }

    pub fn call(self, docker: &dyn Docker, args: &[PyValue], kwargs: &PyDict) -> Call<Returned> {
        match self {
            Function::ReleaseTuple => release_tuple(args, kwargs),
            Function::Norm => norm(args, kwargs),
            Function::IsTuplePrefix => is_tuple_prefix(args, kwargs),
            Function::InspectEnv => inspect_env(docker, args, kwargs),
            Function::GenCandidates => gen_candidates(args, kwargs),
        }
    }
}

/// Resolves `resolve_ngc_tag.function`.
pub fn resolve_operation(operation: &str) -> Call<Function> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Raised::builtin(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
            Where::Elsewhere,
        ));
    }
    Function::named(name).ok_or_else(|| {
        Raised::builtin(
            "AttributeError",
            &format!("module '{module}' has no attribute '{name}'"),
            Where::Elsewhere,
        )
    })
}

fn chain_row(error: &Raised) -> PyValue {
    let mut out = PyDict::new();
    let (class, module, message, args, attributes) = match error {
        Raised::Exception(row) => (
            row.class.to_string(),
            row.module.to_string(),
            row.message.clone(),
            PyValue::Tuple(row.args.clone()),
            row.attributes.clone(),
        ),
        Raised::Limit(detail) => (
            "ResourceLimit".to_string(),
            "resolve_ngc_tag_port".to_string(),
            PyStr::new(detail),
            PyValue::Tuple(vec![PyValue::str(detail)]),
            Vec::new(),
        ),
    };
    let mut shown = PyDict::new();
    for (name, value) in attributes {
        shown.set(name, encode(&value).unwrap_or(PyValue::None));
    }
    out.set("type", PyValue::str(&class));
    out.set("module", PyValue::str(&module));
    out.set("message", PyValue::Str(message));
    out.set("suppress_context", PyValue::Bool(false));
    out.set("relation", PyValue::None);
    out.set("args", encode(&args).unwrap_or(PyValue::None));
    out.set("attributes", PyValue::Dict(shown));
    out.set("dict", PyValue::Dict(PyDict::new()));
    PyValue::Dict(out)
}

/// The answer of a call.
pub fn record(id: PyValue, outcome: Call<Returned>, args: &[PyValue], kwargs: &PyDict) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    let shown = outcome.and_then(|returned| {
        typed_result(&returned)
            .map_err(|refusal| Raised::builtin("TypeError", &refusal, Where::Elsewhere))
    });
    match shown {
        Ok((value, origin)) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", value);
            row.set("input_ref", origin);
        }
        Err(error) => {
            let top = chain_row(&error);
            row.set("kind", PyValue::str("exception"));
            if let PyValue::Dict(members) = &top {
                for key in ["type", "module", "message"] {
                    row.set(key, members.get(key).cloned().unwrap_or(PyValue::None));
                }
            }
            row.set("exception_chain", PyValue::List(vec![top]));
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

/// Answers one request line on `docker`.
pub fn answer_with(line: &PyStr, docker: &dyn Docker) -> PyValue {
    let request = match parse_request(line) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let outcome = resolve_operation(&request.operation)
        .and_then(|function| function.call(docker, &request.args, &request.kwargs));
    record(request.id, outcome, &request.args, &request.kwargs)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each, with the real Docker boundary.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    evaluate_with(input, &ProcessDocker::default())
}

/// [`evaluate_jsonl`] on any Docker boundary (the checks' stand-in).
pub fn evaluate_with(input: &[u8], docker: &dyn Docker) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(crate::api::render(&answer_with(&line, docker))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
