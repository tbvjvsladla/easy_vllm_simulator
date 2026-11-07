//! The controlled run of the simlog writer for the checks: the same library over an owned root, with the original collector's control
//! record. Only with the `fixture` feature; no production program has it.
//!
//! A case is `{id, owner: "simlog_writer", function, args, kwargs, context}`; `context` may hold `yaml_enabled` (a bool: the module's
//! `_yaml` is `None` when it is `false`). `@CASE@` in a text is the root. The writes are the real ones (the process's working directory
//! and umask apply); the record's `trace` lists, in order, every `makedirs`, file open, failing write, close and error the library made.
//! Nothing here writes or compares anything: it is the library's own call.

use crate::escalation::Raised;
use crate::model_api::encode;
use crate::pyexpr::Res;
use crate::simlog::{call, profile_value, Function, Host, MODULE};
use crate::simlog_api::chain_of;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

fn rebase(value: &PyValue, root: &PyStr) -> PyValue {
    match value {
        PyValue::Str(text) => {
            let marker: Vec<u32> = "@CASE@".chars().map(u32::from).collect();
            let points = text.code_points();
            let mut out = Vec::with_capacity(points.len());
            let mut at = 0;
            while at < points.len() {
                if points[at..].starts_with(&marker) {
                    out.extend_from_slice(root.code_points());
                    at += marker.len();
                } else {
                    out.push(points[at]);
                    at += 1;
                }
            }
            PyValue::Str(PyStr::from_code_points(out))
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(|item| rebase(item, root)).collect())
        }
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, item) in map.iter() {
                copy.insert(key.clone(), rebase(item, root));
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

fn spec_error(message: &str) -> Failure {
    Failure::new("SpecError", message)
}

/// A case read from its JSON text.
pub struct Case {
    pub id: PyValue,
    function: PyStr,
    args: Vec<PyValue>,
    kwargs: PyDict,
    yaml_enabled: bool,
}

impl Case {
    pub fn parse(text: &PyStr) -> Res<Case> {
        let PyValue::Dict(members) = loads_text_objects(text)
            .map_err(|error| spec_error(&format!("the spec is not JSON: {error}")))?
        else {
            return Err(spec_error("the spec is not an object"));
        };
        if !matches!(members.get("owner"), Some(PyValue::Str(owner)) if owner.equals(MODULE)) {
            return Err(spec_error("the owner is simlog_writer"));
        }
        let list = |key: &str| match members.get(key) {
            Some(PyValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let kwargs = match members.get("kwargs") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let mut yaml_enabled = true;
        if let Some(PyValue::Dict(context)) = members.get("context") {
            for (key, value) in context.iter() {
                match (key.to_utf8().as_deref(), value) {
                    (Some("yaml_enabled"), PyValue::Bool(flag)) => yaml_enabled = *flag,
                    _ => return Err(spec_error("the context holds yaml_enabled (a bool) only")),
                }
            }
        }
        Ok(Case {
            id: members.get("id").cloned().unwrap_or(PyValue::None),
            function: match members.get("function") {
                Some(PyValue::Str(name)) => name.clone(),
                _ => return Err(spec_error("the spec names no function")),
            },
            args: list("args"),
            kwargs,
            yaml_enabled,
        })
    }
}

fn event(name: &str, fields: Vec<(&str, PyValue)>) -> PyValue {
    let mut row = PyDict::new();
    row.set("event", PyValue::str(name));
    for (key, value) in fields {
        row.set(key, value);
    }
    PyValue::Dict(row)
}

/// The recording host of one run.
struct Recorder {
    trace: Vec<PyValue>,
}

impl Host for Recorder {
    fn enabled(&self) -> bool {
        true
    }

    fn record(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.trace.push(event(name, fields));
    }
}

/// Runs one case in `root`: the control record in the original collector's format.
pub fn run_case(case: &Case, root: &PyStr) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(MODULE));
    let before = profile_value(case.yaml_enabled);
    let PyValue::List(args) = rebase(&PyValue::List(case.args.clone()), root) else {
        unreachable!("a list stays a list")
    };
    let PyValue::Dict(kwargs) = rebase(&PyValue::Dict(case.kwargs.clone()), root) else {
        unreachable!("a dict stays a dict")
    };
    let mut recorder = Recorder { trace: Vec::new() };
    let function = case
        .function
        .to_utf8()
        .as_deref()
        .and_then(Function::named)
        .ok_or_else(|| {
            crate::simlog::Thrown::plain(Raised::plain(
                "AttributeError",
                &format!(
                    "module '{MODULE}' has no attribute {}",
                    case.function.repr()
                ),
            ))
        });
    let outcome = function
        .and_then(|function| call(function, &mut recorder, case.yaml_enabled, &args, &kwargs));
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
    row.set("trace", PyValue::List(recorder.trace));
    row.set("globals_before", before);
    row.set("globals_after", profile_value(case.yaml_enabled));
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(""));
    row.set("args_after", encode(&PyValue::List(args)));
    row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    PyValue::Dict(row)
}
