//! `recipe_model_api`: the functions of `parse_model_config.py` and `crosscheck_model_card.py` as a JSON-lines service.
//!
//! `evaluate-jsonl` reads one request per line, `{id, operation: "module.function", args, kwargs, context?}`, and writes
//! exactly one record per request, in order: `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or
//! `{id, kind: "exception", type, message, stdout, stderr, args_after, kwargs_after}`. A tuple result is `{"__tuple__": [...]}`.
//! `args_after`/`kwargs_after` are the supplied arguments after the call: a warnings list a function wrote to is written to in
//! place, so its additions are there even when the call then failed. `context.hf_api_base` points that one call's HTTP lookups at
//! another base (never a global setting). Arguments are bound before any body runs, with CPython's order and messages.

use crate::model_card::{self, Origin, HF_API_BASE};
use crate::model_config::{self, DEFAULT_NAS_HOST_ROOT};
use crate::model_dyn::Warnings;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{DictKey, PyDict, PyValue};
use num_bigint::BigInt;
use std::ops::Index;

/// One parameter: its name and default (`None` = required).
pub struct Param {
    pub name: &'static str,
    pub default: Option<fn() -> PyValue>,
}

const fn required(name: &'static str) -> Param {
    Param {
        name,
        default: None,
    }
}

const fn optional(name: &'static str, default: fn() -> PyValue) -> Param {
    Param {
        name,
        default: Some(default),
    }
}

fn none() -> PyValue {
    PyValue::None
}

fn nas_host_root() -> PyValue {
    PyValue::str(DEFAULT_NAS_HOST_ROOT)
}

fn container_root() -> PyValue {
    PyValue::str("/app/models")
}

fn timeout() -> PyValue {
    PyValue::Float(8.0)
}

fn tolerance() -> PyValue {
    PyValue::Float(0.05)
}

const MAX_PARAMS: usize = 4;
const MAX_DEFAULTS: usize = 3;

#[derive(Clone, Copy)]
enum Source {
    Positional(usize),
    Keyword,
    Default,
}

/// The arguments of one call as the body sees them. Supplied arguments stay where the caller put them (so a body can write to
/// a list it was given); defaults are made for the call and owned here.
pub struct Call<'a> {
    function: &'static ModelFunction,
    args: &'a mut [PyValue],
    kwargs: &'a mut PyDict,
    sources: [Source; MAX_PARAMS],
    first_default: usize,
    defaults: [Option<PyValue>; MAX_DEFAULTS],
    /// The Hugging Face API base this call's lookups use.
    pub api_base: &'a str,
}

impl Index<usize> for Call<'_> {
    type Output = PyValue;

    fn index(&self, index: usize) -> &PyValue {
        match self.sources[index] {
            Source::Positional(at) => &self.args[at],
            Source::Keyword => self
                .kwargs
                .get(self.function.params[index].name)
                .expect("a keyword source names a present key"),
            Source::Default => self.defaults[index - self.first_default]
                .as_ref()
                .expect("binding made every default"),
        }
    }
}

impl Call<'_> {
    /// Parameter `index` for change in place (a warnings list).
    fn slot_mut(&mut self, index: usize) -> &mut PyValue {
        match self.sources[index] {
            Source::Positional(at) => &mut self.args[at],
            Source::Keyword => self
                .kwargs
                .get_mut(self.function.params[index].name)
                .expect("a keyword source names a present key"),
            Source::Default => self.defaults[index - self.first_default]
                .as_mut()
                .expect("binding made every default"),
        }
    }

    fn origin(&self) -> Origin<'_> {
        Origin {
            api_base: self.api_base,
        }
    }
}

type Body = fn(&mut Call<'_>) -> Res<PyValue>;

/// A function of one of the two modules.
pub struct ModelFunction {
    pub module: &'static str,
    pub name: &'static str,
    pub params: &'static [Param],
    pub body: Body,
}

fn dict(value: Res<PyDict>) -> Res<PyValue> {
    value.map(PyValue::Dict)
}

fn texts(items: Vec<agent_runtime_support::pystr::PyStr>) -> PyValue {
    PyValue::List(items.into_iter().map(PyValue::Str).collect())
}

fn optional_big(value: Option<BigInt>) -> PyValue {
    value.map_or(PyValue::None, PyValue::Int)
}

/// `fetch_hf_safetensors_total` as a closure over this call's origin and the default timeout.
fn lookup(base: &str) -> impl Fn(&PyValue) -> Res<PyValue> + '_ {
    move |repo_id| {
        model_card::fetch_hf_safetensors_total(
            repo_id,
            &PyValue::Float(8.0),
            &Origin { api_base: base },
        )
    }
}

pub const FUNCTIONS: &[ModelFunction] = &[
    ModelFunction {
        module: "parse_model_config",
        name: "resolve_kv_layers",
        params: &[
            required("layer_types"),
            required("num_hidden_layers"),
            optional("full_attention_interval", none),
        ],
        body: |c| {
            model_config::resolve_kv_layers(&c[0], &c[1], &c[2])
                .map(model_config::KvLayers::into_value)
        },
    },
    ModelFunction {
        module: "parse_model_config",
        name: "_resolve_host_path",
        params: &[
            required("model_path"),
            required("nas_host_root"),
            optional("nas_container_root", container_root),
        ],
        body: |c| model_config::resolve_host_path(&c[0], &c[1], &c[2]).map(PyValue::Str),
    },
    ModelFunction {
        module: "parse_model_config",
        name: "_model_id_from_path",
        params: &[required("model_path")],
        body: |c| model_config::model_id_from_path(&c[0]).map(PyValue::Str),
    },
    ModelFunction {
        module: "parse_model_config",
        name: "_read_safetensors_header",
        params: &[required("st_path")],
        body: |c| {
            model_config::read_safetensors_header(&c[0])
                .map(|header| header.unwrap_or(PyValue::None))
        },
    },
    ModelFunction {
        module: "parse_model_config",
        name: "_measure_disk_dtype",
        params: &[required("st_path")],
        body: |c| {
            model_config::measure_disk_dtype(&c[0]).map(|(dtype, bpw)| {
                PyValue::Tuple(vec![
                    dtype.unwrap_or(PyValue::None),
                    bpw.map_or(PyValue::None, PyValue::Float),
                ])
            })
        },
    },
    ModelFunction {
        module: "parse_model_config",
        name: "_native_weight_bytes",
        params: &[required("host_path"), required("warnings")],
        body: |c| {
            let host = c[0].clone();
            let mut sink = Warnings::of(c.slot_mut(1));
            model_config::native_weight_bytes(&host, &mut sink).map(optional_big)
        },
    },
    ModelFunction {
        module: "parse_model_config",
        name: "offloadable_weight_bytes",
        params: &[
            required("host_path"),
            required("mechanism"),
            required("warnings"),
        ],
        body: |c| {
            let (host, mechanism) = (c[0].clone(), c[1].clone());
            let mut sink = Warnings::of(c.slot_mut(2));
            model_config::offloadable_weight_bytes(&host, &mechanism, &mut sink).map(optional_big)
        },
    },
    ModelFunction {
        module: "parse_model_config",
        name: "_largest_safetensors",
        params: &[required("host_path")],
        body: |c| {
            model_config::largest_safetensors(&c[0])
                .map(|path| path.map_or(PyValue::None, PyValue::Str))
        },
    },
    ModelFunction {
        module: "parse_model_config",
        name: "_count_params_from_headers",
        params: &[required("host_path")],
        body: |c| model_config::count_params_from_headers(&c[0]).map(optional_big),
    },
    ModelFunction {
        module: "parse_model_config",
        name: "count_mxfp4_params",
        params: &[required("host_path")],
        body: |c| dict(model_config::count_mxfp4_params(&c[0])),
    },
    ModelFunction {
        module: "parse_model_config",
        name: "parse",
        params: &[
            required("model_path"),
            optional("nas_host_root", nas_host_root),
            optional("nas_container_root", container_root),
        ],
        body: |c| dict(model_config::parse(&c[0], &c[1], &c[2])),
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "_read_st_header",
        params: &[required("path")],
        body: |c| model_card::read_st_header(&c[0]),
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "_canonical_shards",
        params: &[required("model_dir")],
        body: |c| model_card::canonical_shards(&c[0]).map(texts),
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "scan_dtypes",
        params: &[required("model_dir"), optional("mxfp4", none)],
        body: |c| dict(model_card::scan_dtypes(&c[0], &c[1])),
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "config_precision",
        params: &[required("cfg")],
        body: |c| model_card::config_precision(&c[0]).map(PyValue::Str),
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "card_precision",
        params: &[required("card"), required("model_name")],
        body: |c| dict(model_card::card_precision(&c[0], &c[1])),
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "read_inference_deps",
        params: &[required("model_dir")],
        body: |c| model_card::read_inference_deps(&c[0]).map(PyValue::List),
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "fetch_hf_safetensors_total",
        params: &[required("repo_id"), optional("timeout", timeout)],
        body: |c| {
            let origin = c.origin();
            model_card::fetch_hf_safetensors_total(&c[0], &c[1], &origin)
        },
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "crosscheck_external_vram",
        params: &[
            required("local_num_params"),
            required("local_native_weight_bytes"),
            required("repo_id"),
            optional("tolerance", tolerance),
        ],
        body: |c| {
            let fetch = lookup(c.api_base);
            dict(model_card::crosscheck_external_vram(
                &c[0], &c[2], &c[3], &fetch,
            ))
        },
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "crosscheck",
        params: &[
            required("model_dir"),
            optional("hf_repo_id", none),
            optional("local_num_params", none),
            optional("local_parse_error", none),
        ],
        body: |c| {
            let fetch = lookup(c.api_base);
            dict(model_card::crosscheck(&c[0], &c[1], &c[2], &c[3], &fetch))
        },
    },
    ModelFunction {
        module: "crosscheck_model_card",
        name: "_human",
        params: &[required("r")],
        body: |c| model_card::human(&c[0]).map(PyValue::Str),
    },
];

/// Every function fits the inline storage, and its defaulted parameters are the last ones.
const _: () = {
    let mut index = 0;
    while index < FUNCTIONS.len() {
        let params = FUNCTIONS[index].params;
        assert!(params.len() <= MAX_PARAMS);
        let mut position = 0;
        let mut defaulted = 0;
        while position < params.len() {
            if params[position].default.is_some() {
                defaulted += 1;
            } else {
                assert!(defaulted == 0);
            }
            position += 1;
        }
        assert!(defaulted <= MAX_DEFAULTS);
        index += 1;
    }
};

/// The modules this package provides.
pub const MODULES: [&str; 2] = ["parse_model_config", "crosscheck_model_card"];

/// Resolves `module.function`; the failure is the `AttributeError`/`ModuleNotFoundError` of the lookup.
pub fn resolve(operation: &str) -> Result<&'static ModelFunction, Failure> {
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

fn join_names(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("'{name}'")).collect();
    match quoted.len() {
        1 => quoted[0].clone(),
        2 => format!("{} and {}", quoted[0], quoted[1]),
        _ => format!(
            "{}, and {}",
            quoted[..quoted.len() - 1].join(", "),
            quoted[quoted.len() - 1]
        ),
    }
}

impl ModelFunction {
    /// Binds positional and keyword arguments as a Python call does; the failures are CPython 3.12's `TypeError`s.
    fn bind<'a>(
        &'static self,
        args: &'a mut [PyValue],
        kwargs: &'a mut PyDict,
        api_base: &'a str,
    ) -> Result<Call<'a>, Failure> {
        let name = self.name;
        let total = self.params.len();
        let mut sources: [Option<Source>; MAX_PARAMS] = [None; MAX_PARAMS];
        for (index, slot) in sources.iter_mut().enumerate().take(total.min(args.len())) {
            *slot = Some(Source::Positional(index));
        }
        for (key, _) in kwargs.iter() {
            let Some(position) = self.params.iter().position(|p| key.equals(p.name)) else {
                return Err(Failure::new(
                    "TypeError",
                    &format!("{name}() got an unexpected keyword argument {}", key.repr()),
                ));
            };
            if sources[position].is_some() {
                let key_text = key.to_utf8().unwrap_or_default();
                return Err(Failure::new(
                    "TypeError",
                    &format!("{name}() got multiple values for argument '{key_text}'"),
                ));
            }
            sources[position] = Some(Source::Keyword);
        }
        if args.len() > total {
            let required_count = self.params.iter().filter(|p| p.default.is_none()).count();
            let given = args.len();
            let takes = if required_count == total {
                format!(
                    "{total} positional argument{}",
                    if total == 1 { "" } else { "s" }
                )
            } else {
                format!("from {required_count} to {total} positional arguments")
            };
            let verb = if given == 1 { "was" } else { "were" };
            return Err(Failure::new(
                "TypeError",
                &format!("{name}() takes {takes} but {given} {verb} given"),
            ));
        }
        let missing: Vec<&str> = self
            .params
            .iter()
            .zip(&sources)
            .filter(|(p, source)| source.is_none() && p.default.is_none())
            .map(|(p, _)| p.name)
            .collect();
        if !missing.is_empty() {
            let count = missing.len();
            return Err(Failure::new(
                "TypeError",
                &format!(
                    "{name}() missing {count} required positional argument{}: {}",
                    if count == 1 { "" } else { "s" },
                    join_names(&missing)
                ),
            ));
        }
        let first_default = self.params.partition_point(|p| p.default.is_none());
        let mut defaults: [Option<PyValue>; MAX_DEFAULTS] = [const { None }; MAX_DEFAULTS];
        let mut bound = [Source::Default; MAX_PARAMS];
        for (index, source) in sources.iter().enumerate().take(total) {
            if let Some(source) = source {
                bound[index] = *source;
            } else if let Some(make) = self.params[index].default {
                defaults[index - first_default] = Some(make());
            }
        }
        Ok(Call {
            function: self,
            args,
            kwargs,
            sources: bound,
            first_default,
            defaults,
            api_base,
        })
    }

    /// Binds and runs; the arguments are left as the body left them.
    pub fn call(
        &'static self,
        args: &mut [PyValue],
        kwargs: &mut PyDict,
        api_base: &str,
    ) -> Result<PyValue, Failure> {
        let mut call = self.bind(args, kwargs, api_base)?;
        (self.body)(&mut call)
    }
}

/// The module constants of both scripts as one object.
pub fn constants() -> PyValue {
    let mut all = PyDict::new();
    all.set("DEFAULT_NAS_HOST_ROOT", PyValue::str(DEFAULT_NAS_HOST_ROOT));
    all.set(
        "_SAFETENSORS_MAX_HEADER",
        PyValue::int(BigInt::from(model_config::SAFETENSORS_MAX_HEADER)),
    );
    let table = |rows: &[(&str, f64)]| {
        let mut dict = PyDict::new();
        for (name, value) in rows {
            dict.set(name, PyValue::Float(*value));
        }
        PyValue::Dict(dict)
    };
    all.set(
        "_SAFETENSORS_DTYPE_BPW",
        table(&model_config::SAFETENSORS_DTYPE_BPW),
    );
    let mut elem = PyDict::new();
    for (name, bytes) in model_config::SAFETENSORS_DTYPE_ELEM_BYTES {
        elem.set(name, PyValue::int(BigInt::from(bytes)));
    }
    all.set("_SAFETENSORS_DTYPE_ELEM_BYTES", PyValue::Dict(elem));
    all.set(
        "_KV_FREE_LAYER_TYPES",
        PyValue::List(
            model_config::KV_FREE_LAYER_TYPES
                .iter()
                .map(|name| PyValue::str(name))
                .collect(),
        ),
    );
    all.set(
        "_KV_FULL_LAYER_TYPE",
        PyValue::str(model_config::KV_FULL_LAYER_TYPE),
    );
    let mut patterns = PyDict::new();
    for (name, pattern) in model_config::MMAP_OFFLOADABLE_PATTERNS {
        patterns.set(name, PyValue::str(pattern));
    }
    all.set("MMAP_OFFLOADABLE_PATTERNS", PyValue::Dict(patterns));
    all.set("HF_API_BASE", PyValue::str(HF_API_BASE));
    let mut deps = PyDict::new();
    for (name, why) in model_card::KNOWN_SPECIAL_DEPS {
        deps.set(name, PyValue::str(why));
    }
    all.set("KNOWN_SPECIAL_DEPS", PyValue::Dict(deps));
    let set_of =
        |names: &[&str]| PyValue::List(names.iter().map(|name| PyValue::str(name)).collect());
    all.set("_FP8", set_of(&model_card::FP8_DTYPES));
    all.set("_SCALE", set_of(&model_card::SCALE_DTYPES));
    all.set("_HIGH", set_of(&model_card::HIGH_DTYPES));
    all.set("_INTPACK", set_of(&model_card::INTPACK_DTYPES));
    PyValue::Dict(all)
}

/// Tuples become `{"__tuple__": [...]}`, recursively.
pub fn encode(value: &PyValue) -> PyValue {
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
            for (other, key, item) in map.keyed_entries() {
                match other {
                    Some(other) => copy.insert_key(DictKey::Other(other.clone()), encode(item)),
                    None => copy.insert(key.clone(), encode(item)),
                }
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

/// The record of one request.
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

fn request_error(message: &str) -> Failure {
    Failure::new("RequestError", message)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(request_error(message)),
        PyValue::List(Vec::new()),
        PyValue::Dict(PyDict::new()),
    )
}

/// Answers one request line.
pub fn answer(line: &PyStr) -> PyValue {
    let request = match loads_text(line) {
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
    let mut args = match members.get("args") {
        None => Vec::new(),
        Some(PyValue::List(items)) => items.clone(),
        Some(_) => return request_record(id, "args must be an array"),
    };
    let mut kwargs = match members.get("kwargs") {
        None => PyDict::new(),
        Some(PyValue::Dict(map)) => map.clone(),
        Some(_) => return request_record(id, "kwargs must be an object"),
    };
    let api_base = match members.get("context") {
        None | Some(PyValue::None) => HF_API_BASE.to_string(),
        Some(PyValue::Dict(context)) => match context.get("hf_api_base") {
            Some(PyValue::Str(base)) => base.to_utf8().unwrap_or_default(),
            None => HF_API_BASE.to_string(),
            Some(_) => return request_record(id, "context.hf_api_base must be a string"),
        },
        Some(_) => return request_record(id, "context must be an object"),
    };
    let outcome = resolve(&operation.to_utf8().unwrap_or_default())
        .and_then(|function| function.call(&mut args, &mut kwargs, &api_base));
    record(
        id,
        outcome,
        encode(&PyValue::List(args)),
        encode(&PyValue::Dict(kwargs)),
    )
}
