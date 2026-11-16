//! Small helpers over the Python-compatible values of `agent-runtime-support`: building objects, reading fields,
//! `repr`, and `json.dumps(..., ensure_ascii=False, sort_keys=True[, indent=2])`.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub type V = PyValue;

pub fn s(text: &str) -> V {
    PyValue::str(text)
}
pub fn opt_s(text: Option<&str>) -> V {
    text.map(s).unwrap_or(PyValue::None)
}
pub fn b(value: bool) -> V {
    PyValue::Bool(value)
}
pub fn int(value: i64) -> V {
    PyValue::int(value)
}
pub fn null() -> V {
    PyValue::None
}
pub fn list_s<S: AsRef<str>>(items: &[S]) -> V {
    PyValue::List(items.iter().map(|x| s(x.as_ref())).collect())
}

/// A dict display with string keys, in the given (insertion) order.
pub fn obj(items: Vec<(&str, V)>) -> V {
    let mut d = PyDict::new();
    for (k, v) in items {
        d.set(k, v);
    }
    PyValue::Dict(d)
}

pub fn dict(value: &V) -> Option<&PyDict> {
    match value.peel() {
        PyValue::Dict(d) => Some(d),
        _ => None,
    }
}
pub fn dict_mut(value: &mut V) -> Option<&mut PyDict> {
    if let PyValue::Shared(_) = value {
        let owned = value.peel().clone();
        *value = owned;
    }
    match value {
        PyValue::Dict(d) => Some(d),
        _ => None,
    }
}
pub fn text(value: &V) -> Option<String> {
    match value.peel() {
        PyValue::Str(t) => t.to_utf8(),
        _ => None,
    }
}
/// `d.get(key)` as an owned clone (absent = None).
pub fn get(d: &V, key: &str) -> Option<V> {
    dict(d).and_then(|m| m.get(key)).cloned()
}
pub fn get_str(d: &V, key: &str) -> Option<String> {
    dict(d).and_then(|m| m.get(key)).and_then(text)
}
pub fn is_none(value: &V) -> bool {
    matches!(value.peel(), PyValue::None)
}
pub fn is_dict(value: &V) -> bool {
    matches!(value.peel(), PyValue::Dict(_))
}

/// Python `repr(str)`.
pub fn repr(t: &str) -> String {
    PyStr::new(t).repr()
}
/// Python `repr(value)`.
pub fn repr_v(value: &V) -> String {
    value.repr()
}
/// Python `repr(x)` for `x: str | None`.
pub fn repr_opt(value: Option<&str>) -> String {
    match value {
        Some(t) => repr(t),
        None => "None".to_string(),
    }
}
/// `type(value).__name__`.
pub fn type_name(value: &V) -> &'static str {
    value.type_name()
}
/// `repr(sorted(list_of_str))`.
pub fn repr_sorted<S: AsRef<str>>(items: &[S]) -> String {
    let mut v: Vec<&str> = items.iter().map(|x| x.as_ref()).collect();
    v.sort_by(|a, c| cmp_code_points(a, c));
    PyValue::List(v.into_iter().map(s).collect()).repr()
}
pub fn cmp_code_points(a: &str, c: &str) -> std::cmp::Ordering {
    a.chars().cmp(c.chars())
}

/// Python `a == b` for JSON-shaped values: dicts compare by key set (order-free), numbers across int/float/bool.
pub fn py_eq(a: &V, c: &V) -> bool {
    use num_traits_free::as_f64;
    match (a.peel(), c.peel()) {
        (PyValue::Dict(x), PyValue::Dict(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k2 == k && py_eq(v, v2)))
        }
        (PyValue::List(x), PyValue::List(y)) | (PyValue::Tuple(x), PyValue::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| py_eq(p, q))
        }
        (PyValue::Int(x), PyValue::Int(y)) => x == y,
        (PyValue::Bool(x), PyValue::Bool(y)) => x == y,
        (PyValue::Int(_) | PyValue::Bool(_) | PyValue::Float(_), PyValue::Int(_) | PyValue::Bool(_) | PyValue::Float(_)) => {
            match (as_f64(a.peel()), as_f64(c.peel())) {
                (Some(p), Some(q)) => p == q,
                _ => false,
            }
        }
        _ => a == c,
    }
}
mod num_traits_free {
    use agent_runtime_support::pyvalue::PyValue;
    pub fn as_f64(v: &PyValue) -> Option<f64> {
        match v {
            PyValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            PyValue::Float(f) => Some(*f),
            PyValue::Int(n) => n.to_string().parse::<f64>().ok(),
            _ => None,
        }
    }
}

/// Recursively orders dict keys the way `sort_keys=True` does (string keys, code-point order).
pub fn sorted_keys(value: &V) -> V {
    match value.peel() {
        PyValue::Dict(d) => {
            let mut entries: Vec<(PyStr, V)> = d.iter().map(|(k, v)| (k.clone(), sorted_keys(v))).collect();
            entries.sort_by(|a, c| a.0.code_points().cmp(c.0.code_points()));
            let mut out = PyDict::new();
            for (k, v) in entries {
                out.insert(k, v);
            }
            PyValue::Dict(out)
        }
        PyValue::List(items) => PyValue::List(items.iter().map(sorted_keys).collect()),
        PyValue::Tuple(items) => PyValue::Tuple(items.iter().map(sorted_keys).collect()),
        other => other.clone(),
    }
}

fn utf8(text: PyStr) -> String {
    match text.to_utf8() {
        Some(t) => t,
        None => String::from_utf8_lossy(&text.to_backslashreplace_bytes()).into_owned(),
    }
}

/// `json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2)`.
pub fn dumps_pretty_sorted(value: &V) -> String {
    utf8(sorted_keys(value).dumps_indented(2).expect("json-serializable value"))
}
/// `json.dumps(value, ensure_ascii=False, sort_keys=True)`.
pub fn dumps_compact_sorted(value: &V) -> String {
    utf8(sorted_keys(value).dumps_unicode().expect("json-serializable value"))
}
/// `json.dumps(value)` (ensure_ascii=True, insertion order).
pub fn dumps_ascii(value: &V) -> String {
    value.dumps_ascii().expect("json-serializable value")
}
