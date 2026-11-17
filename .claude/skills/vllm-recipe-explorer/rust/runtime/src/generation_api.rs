//! `recipe_generation_api`: the functions of `gen_recipe_set.py` and `feedback_log.py` as a JSON-lines service, wire-compatible
//! with `recipe_model_api`. `evaluate-jsonl` reads one request per line, `{id, operation: "module.function", args, kwargs,
//! context?}`, and writes exactly one record per request, in order: `{id, kind: "value", value, stdout, stderr, args_after,
//! kwargs_after}` or `{id, kind: "exception", type, message, ...}`. A tuple result is `{"__tuple__": [...]}`. `stdout` and
//! `stderr` are what the call wrote to those streams (the generator's warnings go to `stderr`); `args_after` and `kwargs_after`
//! are the supplied arguments after the call (no function here changes them).
//!
//! Requests are decoded with the object-preserving JSON decoder, so a literal `NaN` is the one object `json.loads` returns for it
//! (two `NaN` keys of one `dict(...)` are one key, as in Python). `result_format` is an optional member of the request itself
//! (outside `kwargs` and `context`, which carry the caller's data) and is supported by `feedback_log._normalize` only: with
//! `"dict-items"` a successful reply carries `result_format: "dict-items"` after `kind` and `value` as the dict's ordered array of
//! `[key descriptor, encoded value]` pairs, the descriptor being `{"kind": "str", "value": text}` for a string key and
//! `{"kind": "scalar", "repr": the Python key's repr}` for any other JSON scalar key (so `1`, `True`, `1.0`, `nan`, `None` keep their
//! type through the JSON object; the repr is data and is never parsed here). Without the member every reply has the shape above.
//!
//! The context is explicit per call and never global: `now_iso` (a text) is the clock the functions that read the time use
//! for that call (without it they read the process's local clock); `skill_root` (a text) is the skill root `append` takes its
//! default log path from when no path is given (without it, the root the running binary is deployed in, see
//! [`crate::anchor`]). Arguments are bound before any body runs, with CPython's order and messages.

use crate::anchor::{default_log_path, process_skill_root};
use crate::feedback::{self, Clock, Estimate, ESTIMATED_FIELDS, NULLABLE_FIELDS};
use crate::model_api::encode;
use crate::pyexpr::Res;
use crate::recipe_set::{self, EnvSettings, Request};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{optional, required, Bound, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// What one call may know beyond its arguments: its clock, and the skill root for the default log path.
pub struct Context {
    pub clock: Clock,
    pub skill_root: PyStr,
}

type Body = fn(&Bound<'_>, &Context, &mut PyStr) -> Res<PyValue>;

/// A function of one of the two modules.
pub struct Function {
    pub module: &'static str,
    pub name: &'static str,
    pub params: &'static [Param],
    pub rest: bool,
    pub body: Body,
}

fn none() -> PyValue {
    PyValue::None
}

fn single() -> PyValue {
    PyValue::str("single")
}

fn false_() -> PyValue {
    PyValue::Bool(false)
}

fn true_() -> PyValue {
    PyValue::Bool(true)
}

fn port_default() -> PyValue {
    PyValue::int(8000)
}

fn texts(items: Vec<PyStr>) -> PyValue {
    PyValue::List(items.into_iter().map(PyValue::Str).collect())
}

pub const FUNCTIONS: &[Function] = &[
    Function {
        module: "gen_recipe_set",
        name: "_is_native_or_none",
        params: &[required("quant")],
        rest: false,
        body: |c, _, _| recipe_set::is_native_or_none(&c[0]).map(PyValue::Bool),
    },
    Function {
        module: "gen_recipe_set",
        name: "_build_vram_breakdown_block",
        params: &[required("breakdown")],
        rest: false,
        body: |c, _, _| recipe_set::build_vram_breakdown_block(&c[0]).map(texts),
    },
    Function {
        module: "gen_recipe_set",
        name: "_build_yaml",
        params: &[
            required("parsed"),
            required("recipe"),
            required("served_model_name"),
            optional("port", port_default),
        ],
        rest: false,
        body: |c, _, _| recipe_set::build_yaml(&c[0], &c[1], &c[3]).map(PyValue::Str),
    },
    Function {
        module: "gen_recipe_set",
        name: "recipe_from_candidate",
        params: &[required("candidate")],
        rest: true,
        body: |c, _, _| recipe_set::recipe_from_candidate(&c[0], &c.rest()).map(PyValue::Dict),
    },
    Function {
        module: "gen_recipe_set",
        name: "serve_env_pairs",
        params: &[required("value")],
        rest: false,
        body: |c, _, _| {
            recipe_set::serve_env_pairs(&c[0]).map(|pairs| {
                PyValue::List(
                    pairs
                        .into_iter()
                        .map(|(name, value)| {
                            PyValue::Tuple(vec![PyValue::Str(name), PyValue::Str(value)])
                        })
                        .collect(),
                )
            })
        },
    },
    Function {
        module: "gen_recipe_set",
        name: "assert_serve_knob_parity",
        params: &[
            required("run_trial_source"),
            optional("raise_on_gap", true_),
        ],
        rest: false,
        body: |c, _, _| recipe_set::assert_serve_knob_parity(&c[0], &c[1]).map(texts),
    },
    Function {
        module: "gen_recipe_set",
        name: "_build_sh",
        params: &[
            required("name"),
            required("served_model_name"),
            optional("recipe", none),
        ],
        rest: false,
        body: |c, _, _| recipe_set::build_sh(&c[0], &c[2]).map(PyValue::Str),
    },
    Function {
        module: "gen_recipe_set",
        name: "_build_env",
        params: &[
            required("name"),
            required("served_model_name"),
            required("port"),
            optional("image", none),
            optional("topology", single),
            optional("vllm_version", none),
            optional("build_dockerfile", none),
            // plan_26111708 B L10: the source track's version identity (after the original's parameters).
            optional("vllm_ref", none),
        ],
        rest: false,
        body: |c, _, stderr| {
            let settings = EnvSettings {
                name: &c[0],
                served_model_name: &c[1],
                port: &c[2],
                image: &c[3],
                topology: &c[4],
                vllm_version: &c[5],
                build_dockerfile: &c[6],
                vllm_ref: &c[7],
            };
            recipe_set::build_env(&settings, stderr).map(PyValue::Str)
        },
    },
    Function {
        module: "gen_recipe_set",
        name: "generate",
        params: &[
            required("parsed"),
            required("recipe"),
            required("name"),
            required("repo_root"),
            required("port"),
            required("served_model_name"),
            optional("force", false_),
            optional("topology", single),
            optional("vllm_version", none),
            optional("build_dockerfile", none),
            optional("image", none),
            // plan_26111708 B L10: the source track's version identity (after the original's parameters).
            optional("vllm_ref", none),
        ],
        rest: false,
        body: |c, _, stderr| {
            let request = Request {
                parsed: &c[0],
                recipe: &c[1],
                name: &c[2],
                repo_root: &c[3],
                port: &c[4],
                served_model_name: &c[5],
                force: &c[6],
                topology: &c[7],
                vllm_version: &c[8],
                build_dockerfile: &c[9],
                image: &c[10],
                vllm_ref: &c[11],
            };
            recipe_set::generate(&request, stderr).map(texts)
        },
    },
    Function {
        module: "gen_recipe_set",
        name: "_load_json",
        params: &[required("path")],
        rest: false,
        body: |c, _, _| recipe_set::load_json(&c[0]),
    },
    Function {
        module: "feedback_log",
        name: "build_record",
        params: &[
            required("model_id"),
            required("quantization"),
            required("max_model_len"),
            required("gpu_memory_utilization"),
            required("vram_budget_gb"),
            required("estimated_vram_gb"),
            required("tensor_parallel_size"),
            required("safety_margin_threshold"),
            optional("selection_timestamp", none),
        ],
        rest: false,
        body: |c, context, _| {
            let estimate = Estimate {
                model_id: &c[0],
                quantization: &c[1],
                max_model_len: &c[2],
                gpu_memory_utilization: &c[3],
                vram_budget_gb: &c[4],
                estimated_vram_gb: &c[5],
                tensor_parallel_size: &c[6],
                safety_margin_threshold: &c[7],
                selection_timestamp: &c[8],
            };
            Ok(PyValue::Dict(feedback::build_record(
                &estimate,
                &context.clock,
            )))
        },
    },
    Function {
        module: "feedback_log",
        name: "_normalize",
        params: &[required("record")],
        rest: false,
        body: |c, context, _| feedback::normalize(&c[0], &context.clock).map(PyValue::Dict),
    },
    Function {
        module: "feedback_log",
        name: "append",
        params: &[required("record"), optional("path", none)],
        rest: false,
        body: |c, context, _| {
            // The default path is the module's `DEFAULT_LOG_PATH`: it applies only when no path was given at all.
            let default = PyValue::Str(default_log_path(&context.skill_root));
            let path = if c.is_supplied(1) { &c[1] } else { &default };
            feedback::append(&c[0], path, &context.clock)
        },
    },
];

/// The modules this package provides.
pub const MODULES: [&str; 2] = ["gen_recipe_set", "feedback_log"];

/// Resolves `module.function`; the failure is the `AttributeError`/`ModuleNotFoundError` of the lookup.
pub fn resolve(operation: &str) -> Result<&'static Function, Failure> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if !MODULES.contains(&module) {
        return Err(Failure::new(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    FUNCTIONS
        .iter()
        .find(|function| function.module == module && function.name == name)
        .ok_or_else(|| {
            Failure::new(
                "AttributeError",
                &format!("module '{module}' has no attribute '{name}'"),
            )
        })
}

impl Function {
    /// Binds and runs; the arguments are left as the body left them (no body changes them). What the call wrote to standard
    /// error is appended to `stderr`, also when the call then fails.
    pub fn call(
        &self,
        args: &[PyValue],
        kwargs: &PyDict,
        context: &Context,
        stderr: &mut PyStr,
    ) -> Res<PyValue> {
        let signature = Signature {
            name: self.name,
            params: self.params,
            rest: self.rest,
        };
        let bound = signature.bind(args, kwargs)?;
        (self.body)(&bound, context, stderr)
    }
}

/// The module constants of both scripts as one flat object (tuples and sets as arrays, in declaration order).
pub fn constants(skill_root: &PyStr) -> PyValue {
    let names =
        |items: &[&str]| PyValue::List(items.iter().map(|item| PyValue::str(item)).collect());
    let mut all = PyDict::new();
    all.set("SERVE_KNOB_KEYS", names(&recipe_set::SERVE_KNOB_KEYS));
    let mut native = vec![PyValue::None];
    native.extend(
        recipe_set::NATIVE_QUANT_TEXTS
            .iter()
            .map(|item| PyValue::str(item)),
    );
    all.set("_NATIVE_QUANT_VALUES", PyValue::List(native));
    all.set("_PARITY_EXEMPT", names(&recipe_set::PARITY_EXEMPT));
    let mut probe = PyDict::new();
    for name in recipe_set::PROBE_NAMES {
        probe.set(name, recipe_set::probe_value(name));
    }
    all.set("_PROBE", PyValue::Dict(probe));
    all.set("ESTIMATED_FIELDS", names(&ESTIMATED_FIELDS));
    all.set("NULLABLE_FIELDS", names(&NULLABLE_FIELDS));
    all.set(
        "DEFAULT_LOG_PATH",
        PyValue::Str(default_log_path(skill_root)),
    );
    PyValue::Dict(all)
}

/// The result of `feedback_log._normalize` as an ordered array of `[key descriptor, encoded value]` pairs
/// (`result_format: "dict-items"`), so that the keys' types survive the JSON object: a string key is
/// `{"kind": "str", "value": text}`, any other (JSON scalar) key is `{"kind": "scalar", "repr": the Python key's repr}` (the
/// representation the dict kept when equal keys such as `1`, `True` and `1.0` collided; never parsed here). The values are
/// encoded as in every other reply. `None` for a value that is not a dict.
fn dict_items(value: &PyValue) -> Option<PyValue> {
    let PyValue::Dict(map) = value else {
        return None;
    };
    Some(PyValue::List(
        map.keyed_entries()
            .map(|(other, key, item)| {
                let mut descriptor = PyDict::new();
                match other {
                    None => {
                        descriptor.set("kind", PyValue::str("str"));
                        descriptor.set("value", PyValue::Str(key.clone()));
                    }
                    Some(other) => {
                        descriptor.set("kind", PyValue::str("scalar"));
                        descriptor.set("repr", PyValue::str(&other.repr));
                    }
                }
                PyValue::List(vec![PyValue::Dict(descriptor), encode(item)])
            })
            .collect(),
    ))
}

/// The record of one request; `items` asks for the `dict-items` form of a successful dict result.
fn record(
    id: PyValue,
    outcome: Result<PyValue, Failure>,
    stderr: PyStr,
    args_after: PyValue,
    kwargs_after: PyValue,
    items: bool,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            match items.then(|| dict_items(&value)).flatten() {
                Some(pairs) => {
                    row.set("result_format", PyValue::str("dict-items"));
                    row.set("value", pairs);
                }
                None => row.set("value", encode(&value)),
            }
        }
        Err(failure) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(failure.class));
            row.set("message", PyValue::Str(failure.message));
        }
    }
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::Str(stderr));
    row.set("args_after", args_after);
    row.set("kwargs_after", kwargs_after);
    PyValue::Dict(row)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Failure::new("RequestError", message)),
        PyStr::default(),
        PyValue::List(Vec::new()),
        PyValue::Dict(PyDict::new()),
        false,
    )
}

/// The skill root a request without `context.skill_root` uses: the running binary's own deployment root.
pub fn deployed_root() -> PyStr {
    process_skill_root().unwrap_or_default()
}

/// Answers one request line; `default_root` is the root a request without its own `context.skill_root` gets.
pub fn answer(line: &PyStr, default_root: &PyStr) -> PyValue {
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
    let mut context = Context {
        clock: Clock::Local,
        skill_root: default_root.clone(),
    };
    match members.get("context") {
        None | Some(PyValue::None) => {}
        Some(PyValue::Dict(given)) => {
            match given.get("now_iso") {
                None | Some(PyValue::None) => {}
                Some(PyValue::Str(text)) => context.clock = Clock::Fixed(text.clone()),
                Some(_) => return request_record(id, "context.now_iso must be a string"),
            }
            match given.get("skill_root") {
                None | Some(PyValue::None) => {}
                Some(PyValue::Str(text)) => context.skill_root = text.clone(),
                Some(_) => return request_record(id, "context.skill_root must be a string"),
            }
        }
        Some(_) => return request_record(id, "context must be an object"),
    }
    // `result_format` is a member of the request itself (never of `kwargs` or `context`, which carry the caller's data).
    let items = match members.get("result_format") {
        None => false,
        Some(PyValue::Str(format)) if format.equals("dict-items") => {
            if !operation.equals("feedback_log._normalize") {
                return request_record(
                    id,
                    "result_format \"dict-items\" is supported only by feedback_log._normalize",
                );
            }
            true
        }
        Some(_) => return request_record(id, "result_format must be the string \"dict-items\""),
    };
    let mut stderr = PyStr::default();
    let outcome = resolve(&operation.to_utf8().unwrap_or_default())
        .and_then(|function| function.call(&args, &kwargs, &context, &mut stderr));
    record(
        id,
        outcome,
        stderr,
        encode(&PyValue::List(args)),
        encode(&PyValue::Dict(kwargs)),
        items,
    )
}
