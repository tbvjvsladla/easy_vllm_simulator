//! The importable functions of the three modules by their `module.function` names, with Python's argument binding
//! (positional, keyword, defaults and the `TypeError`s of a wrong call). Used by the numeric API and the benchmark
//! program; the business functions themselves live in their own modules.

use crate::pyexpr::Res;
use crate::{estimate_vram as ev, quant_table as qt, rank_recipes as rr};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::ops::Index;

/// One parameter of an original function: its name and default (`None` = required).
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

fn two() -> PyValue {
    PyValue::int(2)
}

/// The most parameters any function has (`max_feasible_max_len` and `max_feasible_batch`); [`Arguments`] keeps them inline.
const MAX_PARAMS: usize = 8;
/// The most defaulted parameters any function has (`estimate_absolute`). Defaulted parameters always come last, as in Python.
const MAX_DEFAULTS: usize = 3;

/// The arguments of one call as the body sees them, in parameter order. A supplied argument is **borrowed** from the caller
/// (Python binds the same object, it does not copy it), so a large parsed or candidate structure is never duplicated for a
/// read-only body. A default is created for this call and owned here for as long as the body runs; the borrowed references
/// cannot outlive the call because the lifetime `'a` ties them to the caller's slices.
pub struct Arguments<'a> {
    supplied: [Option<&'a PyValue>; MAX_PARAMS],
    /// The index of the first defaulted parameter; `defaults[i - first_default]` is the default of parameter `i`.
    first_default: usize,
    defaults: [Option<PyValue>; MAX_DEFAULTS],
}

impl Index<usize> for Arguments<'_> {
    type Output = PyValue;

    /// Parameter `index`: the supplied argument, else the default made for this call.
    fn index(&self, index: usize) -> &PyValue {
        match self.supplied[index] {
            Some(value) => value,
            None => self.defaults[index - self.first_default]
                .as_ref()
                .expect("binding gave every parameter an argument or a default"),
        }
    }
}

type Body = fn(&Arguments<'_>) -> Res<PyValue>;

/// An original function: module, name, parameters and the native body.
pub struct Function {
    pub module: &'static str,
    pub name: &'static str,
    pub params: &'static [Param],
    pub body: Body,
}

/// The limit functions share the bound arguments `budget_gib, safety_margin, <context or batch>, quantization,
/// kv_dtype_bytes, weights_bytes, overhead_bytes` (positions 1, 2, 4, 5, 6, 7).
fn ceiling_of<'a>(a: &'a Arguments<'_>) -> ev::Ceiling<'a> {
    ev::Ceiling {
        budget_gib: &a[1],
        safety_margin: &a[2],
        quantization: &a[4],
        kv_dtype_bytes: &a[5],
        weights_bytes: &a[6],
        overhead_bytes: &a[7],
    }
}

fn big(number: num_bigint::BigInt) -> PyValue {
    PyValue::Int(number)
}

fn optional_big(number: Option<num_bigint::BigInt>) -> PyValue {
    number.map_or(PyValue::None, PyValue::Int)
}

pub const FUNCTIONS: &[Function] = &[
    Function {
        module: "quant_table",
        name: "is_offline_only_quant",
        params: &[required("quant")],
        body: |a| qt::is_offline_only_quant(&a[0]).map(PyValue::Bool),
    },
    Function {
        module: "quant_table",
        name: "vllm_quant_bpw",
        params: &[required("quant"), required("serve_bpw")],
        body: |a| qt::vllm_quant_bpw(&a[0], &a[1]),
    },
    Function {
        module: "quant_table",
        name: "vllm_quant_bpw_or_none",
        params: &[required("quant"), required("serve_bpw")],
        body: |a| qt::vllm_quant_bpw_or_none(&a[0], &a[1]),
    },
    Function {
        module: "quant_table",
        name: "dtype_bpw",
        params: &[required("dtype")],
        body: |a| qt::dtype_bpw(&a[0]),
    },
    Function {
        module: "estimate_vram",
        name: "kv_bearing_layers",
        params: &[required("parsed")],
        body: |a| ev::kv_bearing_layers(&a[0]).map(optional_big),
    },
    Function {
        module: "estimate_vram",
        name: "kv_layers_source",
        params: &[required("parsed")],
        body: |a| ev::kv_layers_source(&a[0]).map(PyValue::str),
    },
    Function {
        module: "estimate_vram",
        name: "_activation_bytes",
        params: &[required("eff_params"), required("max_model_len")],
        body: |a| ev::activation_bytes(&a[0], &a[1]).map(big),
    },
    Function {
        module: "estimate_vram",
        name: "estimate",
        params: &[
            required("parsed"),
            required("candidate"),
            required("tp"),
            required("budget_gib"),
            required("safety_margin"),
            optional("kv_cache_dtype_bytes", two),
        ],
        body: |a| ev::estimate(&a[0], &a[1], &a[2], &a[3], &a[4], &a[5]),
    },
    Function {
        module: "estimate_vram",
        name: "per_token_kv_bytes",
        params: &[required("parsed"), required("kv_dtype_bytes")],
        body: |a| ev::per_token_kv_bytes(&a[0], &a[1]).map(big),
    },
    Function {
        module: "estimate_vram",
        name: "required_kv_bytes",
        params: &[
            required("parsed"),
            required("max_model_len"),
            required("batch"),
            required("kv_dtype_bytes"),
        ],
        body: |a| ev::required_kv_bytes(&a[0], &a[1], &a[2], &a[3]).map(big),
    },
    Function {
        module: "estimate_vram",
        name: "max_safe_kv_bytes",
        params: &[
            required("budget_gib"),
            required("safety_margin"),
            required("weights_bytes"),
            required("overhead_bytes"),
        ],
        body: |a| ev::max_safe_kv_bytes(&a[0], &a[1], &a[2], &a[3]).map(big),
    },
    Function {
        module: "estimate_vram",
        name: "weights_bytes_for",
        params: &[required("parsed"), required("quantization")],
        body: |a| ev::weights_bytes_for(&a[0], &a[1]).map(PyValue::Float),
    },
    Function {
        module: "estimate_vram",
        name: "estimate_absolute",
        params: &[
            required("parsed"),
            required("candidate"),
            optional("weights_bytes", none),
            optional("overhead_bytes", none),
            optional("kv_dtype_bytes", two),
        ],
        body: |a| ev::estimate_absolute(&a[0], &a[1], &a[2], &a[3], &a[4]),
    },
    Function {
        module: "estimate_vram",
        name: "max_feasible_max_len",
        params: &[
            required("parsed"),
            required("budget_gib"),
            required("safety_margin"),
            required("batch"),
            required("quantization"),
            required("kv_dtype_bytes"),
            optional("weights_bytes", none),
            optional("overhead_bytes", none),
        ],
        body: |a| ev::max_feasible_max_len(&a[0], &a[3], &ceiling_of(a)).map(big),
    },
    Function {
        module: "estimate_vram",
        name: "max_feasible_batch",
        params: &[
            required("parsed"),
            required("budget_gib"),
            required("safety_margin"),
            required("max_model_len"),
            required("quantization"),
            required("kv_dtype_bytes"),
            optional("weights_bytes", none),
            optional("overhead_bytes", none),
        ],
        body: |a| ev::max_feasible_batch(&a[0], &a[3], &ceiling_of(a)).map(big),
    },
    Function {
        module: "rank_recipes",
        name: "auto_candidates",
        params: &[required("parsed"), required("tp")],
        body: |a| rr::auto_candidates(&a[0], &a[1]),
    },
    Function {
        module: "rank_recipes",
        name: "rank",
        params: &[
            required("parsed"),
            required("candidates"),
            required("tp"),
            required("budget_gib"),
            required("margin"),
            optional("kv_bytes", two),
        ],
        body: |a| rr::rank(&a[0], &a[1], &a[2], &a[3], &a[4], &a[5]),
    },
    Function {
        module: "rank_recipes",
        name: "_fmt_num",
        params: &[required("value"), optional("ndigits", two)],
        body: |a| rr::fmt_num(&a[0], &a[1]).map(PyValue::Str),
    },
    Function {
        module: "rank_recipes",
        name: "render_report",
        params: &[
            required("result"),
            required("parsed"),
            required("budget_gib"),
            required("margin"),
            required("tp"),
        ],
        body: |a| rr::render_report(&a[0], &a[1], &a[2], &a[3], &a[4]).map(PyValue::Str),
    },
];

/// The modules this package provides.
pub const MODULES: [&str; 3] = ["quant_table", "estimate_vram", "rank_recipes"];

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

/// Every function fits the inline [`Arguments`] storage, and its defaulted parameters are the last ones.
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
                assert!(
                    defaulted == 0,
                    "a required parameter follows a defaulted one"
                );
            }
            position += 1;
        }
        assert!(defaulted <= MAX_DEFAULTS);
        index += 1;
    }
};

impl Function {
    /// Binds positional and keyword arguments as a Python call does, borrowing what was supplied and creating the defaults
    /// that are needed; the failures are CPython 3.12's `TypeError`s.
    pub fn bind_borrowed<'a>(&self, args: &'a [PyValue], kwargs: &'a PyDict) -> Res<Arguments<'a>> {
        let name = self.name;
        let total = self.params.len();
        // CPython assigns the positional arguments that fit, then walks the keywords in order (an unexpected or repeated
        // keyword is raised before anything else), and only then reports surplus positionals and missing parameters.
        let mut supplied: [Option<&'a PyValue>; MAX_PARAMS] = [None; MAX_PARAMS];
        for (slot, value) in supplied.iter_mut().zip(args.iter().take(total)) {
            *slot = Some(value);
        }
        for (key, value) in kwargs.iter() {
            let Some(position) = self.params.iter().position(|p| key.equals(p.name)) else {
                return Err(Failure::new(
                    "TypeError",
                    &format!("{name}() got an unexpected keyword argument {}", key.repr()),
                ));
            };
            if supplied[position].is_some() {
                let key_text = key.to_utf8().unwrap_or_default();
                return Err(Failure::new(
                    "TypeError",
                    &format!("{name}() got multiple values for argument '{key_text}'"),
                ));
            }
            supplied[position] = Some(value);
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
            .zip(&supplied)
            .filter(|(p, value)| value.is_none() && p.default.is_none())
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
        for (slot, (param, value)) in defaults.iter_mut().zip(
            self.params[first_default..]
                .iter()
                .zip(&supplied[first_default..]),
        ) {
            if value.is_none() {
                *slot = param.default.map(|make| make());
            }
        }
        Ok(Arguments {
            supplied,
            first_default,
            defaults,
        })
    }

    /// The bound arguments as owned values (a copy of what [`Function::bind_borrowed`] borrows); kept for callers that want
    /// them outside a call.
    pub fn bind(&self, args: &[PyValue], kwargs: &PyDict) -> Res<Vec<PyValue>> {
        let bound = self.bind_borrowed(args, kwargs)?;
        Ok((0..self.params.len())
            .map(|index| bound[index].clone())
            .collect())
    }

    /// Binds (borrowing) and runs.
    pub fn call(&self, args: &[PyValue], kwargs: &PyDict) -> Res<PyValue> {
        (self.body)(&self.bind_borrowed(args, kwargs)?)
    }
}
