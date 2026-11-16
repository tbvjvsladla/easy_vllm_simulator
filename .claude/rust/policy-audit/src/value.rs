//! Small Python-semantics helpers over `PyValue` (dict.get, truthiness, str/repr, equality with an int literal).

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;

pub static NONE: PyValue = PyValue::None;

/// `d.get(key)` (None when absent or when `v` is not a dict).
pub fn get<'a>(v: &'a PyValue, key: &str) -> &'a PyValue {
    match v.peel() {
        PyValue::Dict(d) => d.get(key).unwrap_or(&NONE),
        _ => &NONE,
    }
}

pub fn has(v: &PyValue, key: &str) -> bool {
    matches!(v.peel(), PyValue::Dict(d) if d.contains_key(key))
}

pub fn dict(v: &PyValue) -> Option<&PyDict> {
    match v.peel() {
        PyValue::Dict(d) => Some(d),
        _ => None,
    }
}

pub fn is_dict(v: &PyValue) -> bool {
    dict(v).is_some()
}

pub fn list(v: &PyValue) -> Option<&Vec<PyValue>> {
    match v.peel() {
        PyValue::List(l) => Some(l),
        _ => None,
    }
}

pub fn truthy(v: &PyValue) -> bool {
    v.is_truthy()
}

/// `x or []` iterated: a list's items; a truthy dict's keys; a truthy str's characters; anything else (falsy) nothing.
pub fn or_items(v: &PyValue) -> Vec<PyValue> {
    match v.peel() {
        PyValue::List(l) | PyValue::Tuple(l) => l.clone(),
        PyValue::Dict(d) => d.iter().map(|(k, _)| PyValue::Str(k.clone())).collect(),
        PyValue::Str(s) => s.code_points().iter().map(|&p| PyValue::Str(PyStr::from_code_points(vec![p]))).collect(),
        _ => Vec::new(),
    }
}

pub fn as_str(v: &PyValue) -> Option<String> {
    match v.peel() {
        PyValue::Str(s) => Some(s.to_utf8().unwrap_or_else(|| String::from_utf8_lossy(&s.to_backslashreplace_bytes()).into_owned())),
        _ => None,
    }
}

/// `str(v)`
pub fn py_str(v: &PyValue) -> String {
    let t = v.display_text();
    t.to_utf8().unwrap_or_else(|| String::from_utf8_lossy(&t.to_backslashreplace_bytes()).into_owned())
}

/// `repr(v)`
pub fn repr(v: &PyValue) -> String {
    v.repr()
}

/// `repr(s)` of a str
pub fn r(s: &str) -> String {
    PyStr::new(s).repr()
}

pub fn s(text: &str) -> PyValue {
    PyValue::Str(PyStr::new(text))
}

pub fn opt_s(text: Option<&str>) -> PyValue {
    text.map(s).unwrap_or(PyValue::None)
}

/// `repr(list_of_str)`
pub fn list_repr<I: IntoIterator<Item = S>, S: AsRef<str>>(items: I) -> String {
    let v: Vec<String> = items.into_iter().map(|x| r(x.as_ref())).collect();
    format!("[{}]", v.join(", "))
}

/// `v == n` for a Python int literal (bool and integral float compare equal).
pub fn eq_int(v: &PyValue, n: i64) -> bool {
    match v.peel() {
        PyValue::Int(i) => *i == BigInt::from(n),
        PyValue::Bool(b) => (*b as i64) == n,
        PyValue::Float(f) => *f == n as f64,
        _ => false,
    }
}

/// `isinstance(v, str) and v.strip()` style: a non-blank str.
pub fn nonblank_str(v: &PyValue) -> bool {
    match v.peel() {
        PyValue::Str(st) => !st.strip().is_empty(),
        _ => false,
    }
}

/// Python `sorted(keys)` of a dict's string keys / `set(d)`.
pub fn keys(d: &PyDict) -> Vec<String> {
    d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect()
}

/// `json.dumps(v, sort_keys=True, ensure_ascii=False)`: a deep copy with every dict's keys in sorted order, then the default
/// separators.
pub fn sorted_copy(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut entries: Vec<(PyStr, PyValue)> = d.iter().map(|(k, x)| (k.clone(), sorted_copy(x))).collect();
            entries.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (k, x) in entries {
                out.insert(k, x);
            }
            PyValue::Dict(out)
        }
        PyValue::List(l) => PyValue::List(l.iter().map(sorted_copy).collect()),
        PyValue::Tuple(l) => PyValue::Tuple(l.iter().map(sorted_copy).collect()),
        other => other.clone(),
    }
}

pub fn dumps_sorted_unicode(v: &PyValue) -> Vec<u8> {
    let text = sorted_copy(v).dumps_unicode().unwrap_or_default();
    match text.to_utf8() {
        Some(t) => t.into_bytes(),
        None => text.to_surrogateescape_bytes().unwrap_or_default(),
    }
}
