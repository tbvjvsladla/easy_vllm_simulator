//! `norm_prefix(v)` and `pick_bound(versions, newest)` of `resolve_torch_pin.py` (stage A of that module; `fetch` and `main` are not ported).
//!
//! Each function is ONE business function that returns the typed result of the call: the Python return value and, in the same evaluation,
//! its **origin** in the call's argument graph (the path of the original input node that the returned object is, or none for a new value).
//! The JSON service and the benchmark both run exactly these functions.
//!
//! * `norm_prefix`: `Version(v)` is built (every field, in the original's order, so a digit-limit `ValueError` of any field is raised before the
//!   release is read); `InvalidVersion` alone is caught and gives back the ORIGINAL `v`; everything else (`TypeError` for a non-text, `ValueError`)
//!   propagates. The result is the whole release tuple joined with `.` (no padding, no truncation, leading zeros gone).
//! * `pick_bound`: a falsy `versions` is `None`; otherwise `sorted(versions, key=Version)` consumes the iterable, then builds the keys in order (the
//!   first `InvalidVersion` ends it and gives `versions[0]` of the original container, anything else propagates), sorts them stably in PEP 440 order and
//!   returns the RAW original element at the end (`newest` true) or the start of that order.

use crate::pep440::{Refusal, Version};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue, Typed as PyTyped};
use std::borrow::Cow;

pub const MODULE: &str = "resolve_torch_pin";

/// One step of a path in the argument graph: `("args", None)`, `("kwargs", None)`, `("item", Some(i))`, `("key", Some(i))`, `("value", Some(i))`,
/// `("set_item", Some(i))`.
pub type Step = (&'static str, Option<usize>);

/// The path of an input node (`ABI.md`, `input_ref`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin(pub Vec<Step>);

impl Origin {
    fn with(&self, kind: &'static str, index: usize) -> Origin {
        let mut steps = self.0.clone();
        steps.push((kind, Some(index)));
        Origin(steps)
    }

    /// The path as the saved originals write it: a list of steps, each a list (`["item", 0]`).
    pub fn to_value(&self) -> PyValue {
        PyValue::List(
            self.0
                .iter()
                .map(|(kind, index)| {
                    let mut step = vec![PyValue::str(kind)];
                    if let Some(index) = index {
                        step.push(PyValue::int(*index));
                    }
                    PyValue::List(step)
                })
                .collect(),
        )
    }
}

/// The typed result of a call.
#[derive(Clone, Debug)]
pub struct Returned {
    pub value: PyValue,
    pub origin: Option<Origin>,
}

/// One exception of a chain: the class, its module, `str(exception)` and its constructor arguments.
#[derive(Clone, Debug)]
pub struct Row {
    pub class: String,
    pub module: String,
    pub message: PyStr,
    pub args: Vec<PyValue>,
}

/// An exception with its context chain, top exception first (every row but the last has the next one as its implicit `__context__`).
#[derive(Clone, Debug)]
pub struct Raised {
    pub rows: Vec<Row>,
}

impl Raised {
    fn builtin_text(class: &str, message: PyStr) -> Row {
        Row {
            class: class.to_string(),
            module: "builtins".to_string(),
            args: vec![PyValue::Str(message.clone())],
            message,
        }
    }

    pub fn builtin(class: &str, message: &str) -> Raised {
        Raised {
            rows: vec![Raised::builtin_text(class, PyStr::new(message))],
        }
    }

    fn from_failure(failure: Failure) -> Raised {
        Raised {
            rows: vec![Raised::builtin_text(failure.class, failure.message)],
        }
    }

    fn in_context_of(mut self, context: Row) -> Raised {
        self.rows.push(context);
        self
    }
}

fn invalid_version(text: &[u32]) -> Row {
    let mut message: Vec<u32> = "Invalid version: '".chars().map(u32::from).collect();
    message.extend_from_slice(text);
    message.push(u32::from('\''));
    let message = PyStr::from_code_points(message);
    Row {
        class: "InvalidVersion".to_string(),
        module: "packaging.version".to_string(),
        args: vec![PyValue::Str(message.clone())],
        message,
    }
}

const V: &[Param] = &[required("v")];
const VERSIONS_NEWEST: &[Param] = &[required("versions"), required("newest")];
static NORM_PREFIX: Signature = Signature {
    name: "norm_prefix",
    params: V,
    rest: false,
};
static PICK_BOUND: Signature = Signature {
    name: "pick_bound",
    params: VERSIONS_NEWEST,
    rest: false,
};

/// The path of parameter `index` (named `name`) as the call supplied it: a positional argument, else the keyword's value.
fn parameter_origin(args: &[PyValue], kwargs: &PyDict, index: usize, name: &str) -> Origin {
    if index < args.len() {
        Origin(vec![("args", None), ("item", Some(index))])
    } else {
        let at = kwargs
            .iter()
            .position(|(key, _)| key.equals(name))
            .expect("binding found the keyword");
        Origin(vec![("kwargs", None), ("value", Some(at))])
    }
}

/// `Version(value)` for any Python value.
/// `None` is `InvalidVersion`.
fn version_of(value: &PyValue) -> Result<Option<Version>, Raised> {
    match value.peel() {
        PyValue::Str(text) => match Version::parse(text.code_points()) {
            Ok(version) => Ok(Some(version)),
            Err(Refusal::Invalid) => Ok(None),
            Err(Refusal::Thrown(thrown)) => Err(Raised::builtin(
                &thrown.raised.class,
                &thrown.raised.message.to_utf8().unwrap_or_default(),
            )),
        },
        PyValue::Bytes(_) => Err(Raised::builtin(
            "TypeError",
            "cannot use a string pattern on a bytes-like object",
        )),
        other => Err(Raised::builtin(
            "TypeError",
            &format!(
                "expected string or bytes-like object, got '{}'",
                other.type_name()
            ),
        )),
    }
}

/// `norm_prefix(v)`.
pub fn norm_prefix(args: &[PyValue], kwargs: &PyDict) -> Result<Returned, Raised> {
    let bound = NORM_PREFIX
        .bind(args, kwargs)
        .map_err(Raised::from_failure)?;
    let v = &bound[0];
    match version_of(v)? {
        Some(version) => Ok(Returned {
            value: PyValue::str(&version.release_text()),
            origin: None,
        }),
        None => Ok(Returned {
            value: v.clone(),
            origin: Some(parameter_origin(args, kwargs, 0, "v")),
        }),
    }
}

/// One element of the iterated container: the object and the step that reaches it in the graph (none: it is created by the iteration).
struct Element<'a> {
    value: Cow<'a, PyValue>,
    step: Option<(&'static str, usize)>,
}

fn not_iterable(value: &PyValue) -> Raised {
    Raised::builtin(
        "TypeError",
        &format!("'{}' object is not iterable", value.type_name()),
    )
}

/// `iter(versions)` consumed: the members in iteration order (a dict gives its keys, a set the order it was observed in).
fn elements(versions: &PyValue) -> Result<Vec<Element<'_>>, Raised> {
    Ok(match versions {
        PyValue::List(items) | PyValue::Tuple(items) => items
            .iter()
            .enumerate()
            .map(|(at, item)| Element {
                value: Cow::Borrowed(item),
                step: Some(("item", at)),
            })
            .collect(),
        PyValue::Str(text) => text
            .code_points()
            .iter()
            .map(|&point| Element {
                value: Cow::Owned(PyValue::Str(PyStr::from_code_points(vec![point]))),
                step: None,
            })
            .collect(),
        PyValue::Bytes(bytes) => bytes
            .iter()
            .map(|&byte| Element {
                value: Cow::Owned(PyValue::int(byte)),
                step: None,
            })
            .collect(),
        PyValue::Dict(map) => map
            .keyed_entries()
            .enumerate()
            .map(|(at, (other, key, _))| Element {
                value: match other.and_then(|other| other.value.as_deref()) {
                    Some(typed) => Cow::Borrowed(typed),
                    None => Cow::Owned(PyValue::Str(key.clone())),
                },
                step: Some(("key", at)),
            })
            .collect(),
        PyValue::Opaque(opaque) => match &opaque.typed {
            PyTyped::Set(items) => items
                .iter()
                .enumerate()
                .map(|(at, item)| Element {
                    value: Cow::Borrowed(item),
                    step: Some(("set_item", at)),
                })
                .collect(),
            _ => return Err(not_iterable(versions)),
        },
        other => return Err(not_iterable(other)),
    })
}

/// `versions[0]` of the original container, as the `except InvalidVersion` branch returns it; `bad` is the text of the failing element.
fn first_of(versions: &PyValue, base: &Origin, bad: &PyValue) -> Result<Returned, Raised> {
    let text = |value: &PyValue| match value.peel() {
        PyValue::Str(text) => text.code_points().to_vec(),
        _ => Vec::new(),
    };
    let context = || invalid_version(&text(bad));
    match versions {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(Returned {
            value: items[0].clone(),
            origin: Some(base.with("item", 0)),
        }),
        PyValue::Str(characters) => Ok(Returned {
            value: PyValue::Str(PyStr::from_code_points(vec![characters.code_points()[0]])),
            origin: None,
        }),
        PyValue::Bytes(bytes) => Ok(Returned {
            value: PyValue::int(bytes[0]),
            origin: None,
        }),
        PyValue::Dict(map) => {
            // `versions[0]`: the key `0` (an int, a float or a bool equal to it) gives its VALUE
            let found = map
                .keyed_entries()
                .enumerate()
                .find(|(_, (other, _, _))| other.is_some_and(|other| other.identity == "num:0"));
            match found {
                Some((at, (_, _, value))) => Ok(Returned {
                    value: value.clone(),
                    origin: Some(base.with("value", at)),
                }),
                None => {
                    let mut error = Raised::builtin("KeyError", "0");
                    error.rows[0].args = vec![PyValue::int(0)];
                    Err(error.in_context_of(context()))
                }
            }
        }
        other => Err(Raised::builtin(
            "TypeError",
            &format!("'{}' object is not subscriptable", other.type_name()),
        )
        .in_context_of(context())),
    }
}

/// `pick_bound(versions, newest)`.
pub fn pick_bound(args: &[PyValue], kwargs: &PyDict) -> Result<Returned, Raised> {
    let bound = PICK_BOUND
        .bind(args, kwargs)
        .map_err(Raised::from_failure)?;
    let versions = bound[0].peel();
    let base = parameter_origin(args, kwargs, 0, "versions");
    if !versions.is_truthy() {
        // `return None`: the singleton. When `versions` IS `None` the returned object is that very argument (the saved originals list it as an alias);
        // other `None` nodes of the graph (a `None` newest, a `None` inside a container) are not reported (see `ABI.md`, scope gaps).
        return Ok(Returned {
            value: PyValue::None,
            origin: matches!(versions, PyValue::None).then_some(base),
        });
    }
    let items = elements(versions)?;
    let mut keys = Vec::with_capacity(items.len());
    for item in &items {
        match version_of(&item.value)? {
            Some(version) => keys.push(version),
            None => return first_of(versions, &base, &item.value),
        }
    }
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| keys[a].order(&keys[b]));
    let picked = if bound[1].is_truthy() {
        order[order.len() - 1]
    } else {
        order[0]
    };
    let element = &items[picked];
    Ok(Returned {
        value: element.value.as_ref().clone(),
        origin: element.step.map(|(kind, at)| base.with(kind, at)),
    })
}
