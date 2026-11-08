//! `recipe_classifier_api`: the three functions of `sim_classify.py` as a JSON-lines service, wire-compatible with the other
//! recipe services. `constants` writes one flat object (`GIB`, `OOM_SIGNATURES`); `evaluate-jsonl` reads one request per line,
//! `{id, operation: "sim_classify.<function>", args, kwargs}`, and writes exactly one record per request, in order:
//! `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message, ...}`.
//! Requests are decoded with the object-preserving JSON decoder (a literal `NaN` is the one object `json.loads` returns). The
//! functions are pure: `stdout` and `stderr` are empty, and the arguments are left as they were supplied. Arguments are bound
//! before any body runs, with CPython's order and messages.

use crate::classifier::{self, GIB, OOM_SIGNATURES};
use crate::model_api::encode;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{optional, required, Bound, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

type Body = fn(&Bound<'_>) -> Res<PyValue>;

/// A function of `sim_classify`.
pub struct Function {
    pub name: &'static str,
    pub params: &'static [Param],
    pub body: Body,
}

fn none() -> PyValue {
    PyValue::None
}

pub const FUNCTIONS: &[Function] = &[
    Function {
        name: "_profile_gib",
        params: &[required("profile"), required("key")],
        body: |c| {
            classifier::profile_gib(&c[0], &c[1])
                .map(|found| found.map_or(PyValue::None, PyValue::Float))
        },
    },
    Function {
        name: "kv_fit_check",
        params: &[required("trial_result"), required("typical_request_tokens")],
        body: |c| classifier::kv_fit_check(&c[0], &c[1]).map(PyValue::Dict),
    },
    Function {
        name: "classify",
        params: &[
            required("trial_result"),
            required("budget_gib"),
            required("safety_margin"),
            optional("typical_request_tokens", none),
        ],
        body: |c| classifier::classify(&c[0], &c[1], &c[2], &c[3]).map(PyValue::Dict),
    },
];

/// The module `sim_classify`.
pub const MODULE: &str = "sim_classify";

/// Resolves `sim_classify.function`; the failure is the `AttributeError`/`ModuleNotFoundError` of the lookup.
pub fn resolve(operation: &str) -> Result<&'static Function, Failure> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Failure::new(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    FUNCTIONS
        .iter()
        .find(|function| function.name == name)
        .ok_or_else(|| {
            Failure::new(
                "AttributeError",
                &format!("module '{module}' has no attribute '{name}'"),
            )
        })
}

impl Function {
    /// Binds and runs.
    pub fn call(&self, args: &[PyValue], kwargs: &PyDict) -> Res<PyValue> {
        let signature = Signature {
            name: self.name,
            params: self.params,
            rest: false,
        };
        let bound = signature.bind(args, kwargs)?;
        (self.body)(&bound)
    }
}

/// The module constants as one flat object.
pub fn constants() -> PyValue {
    let mut all = PyDict::new();
    all.set("GIB", PyValue::int(GIB));
    all.set(
        "OOM_SIGNATURES",
        PyValue::List(
            OOM_SIGNATURES
                .iter()
                .map(|text| PyValue::str(text))
                .collect(),
        ),
    );
    PyValue::Dict(all)
}

fn record(
    id: PyValue,
    outcome: Result<PyValue, Failure>,
    args_after: PyValue,
    kwargs_after: PyValue,
) -> PyValue {
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
    row.set("args_after", args_after);
    row.set("kwargs_after", kwargs_after);
    PyValue::Dict(row)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Failure::new("RequestError", message)),
        PyValue::List(Vec::new()),
        PyValue::Dict(PyDict::new()),
    )
}

/// Answers one request line.
pub fn answer(line: &PyStr) -> PyValue {
    let request = match loads_text_objects(line) {
        Ok(request) => request,
        Err(error) => return request_record(PyValue::None, &format!("not JSON: {error}")),
    };
    let PyValue::Dict(members) = &request else {
        return request_record(PyValue::None, "the request is not an object");
    };
    let Some(id) = members.get("id").cloned() else {
        return request_record(PyValue::None, "the request has no id");
    };
    let Some(PyValue::Str(operation)) = members.get("operation") else {
        return request_record(id, "operation must be a string 'module.function'");
    };
    let args = match members.get("args") {
        None => Vec::new(),
        Some(PyValue::List(items)) => items.clone(),
        Some(_) => return request_record(id, "args must be an array"),
    };
    let kwargs = match members.get("kwargs") {
        None => PyDict::new(),
        Some(PyValue::Dict(map)) => map.clone(),
        Some(_) => return request_record(id, "kwargs must be an object"),
    };
    let outcome = resolve(&operation.to_utf8().unwrap_or_default())
        .and_then(|function| function.call(&args, &kwargs));
    record(
        id,
        outcome,
        encode(&PyValue::List(args)),
        encode(&PyValue::Dict(kwargs)),
    )
}
