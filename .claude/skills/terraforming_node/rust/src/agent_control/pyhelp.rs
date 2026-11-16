//! Python behaviour the agent-control plane (orchestrator + providers) relies on: `==`/`in` over JSON values, the
//! `json.dumps` variants it calls, `shlex.quote`/`shlex.split`, strict `base64`, `hashlib.sha256().hexdigest()`.
//! Public so relay (which uses the same idioms) can share them.

use agent_runtime_support::pyops::{compare, Compare};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{float_repr, PyDict, PyValue};

pub use crate::logs_lifecycle::sort_keys;

/// Python `a == b` for JSON-shaped values: numbers compare across bool/int/float, dicts ignore order.
pub fn py_eq(a: &PyValue, b: &PyValue) -> bool {
    let (a, b) = (a.peel(), b.peel());
    let num = |v: &PyValue| matches!(v, PyValue::Bool(_) | PyValue::Int(_) | PyValue::Float(_));
    if num(a) && num(b) {
        return compare(Compare::LessEqual, a, b).unwrap_or(false)
            && compare(Compare::GreaterEqual, a, b).unwrap_or(false);
    }
    match (a, b) {
        (PyValue::None, PyValue::None) => true,
        (PyValue::Str(x), PyValue::Str(y)) => x == y,
        (PyValue::List(x), PyValue::List(y)) | (PyValue::Tuple(x), PyValue::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q))
        }
        (PyValue::Dict(x), PyValue::Dict(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.iter().any(|(k2, v2)| k == k2 && py_eq(v, v2))
                })
        }
        _ => a == b,
    }
}

/// Python `value in items`.
pub fn py_in(value: &PyValue, items: &[PyValue]) -> bool {
    items.iter().any(|x| py_eq(value, x))
}

/// `json.dumps(v, ensure_ascii=False, sort_keys=True, indent=2)`.
pub fn dumps_sorted_pretty(v: &PyValue) -> String {
    crate::py::dumps_pretty(&sort_keys(v))
}

/// `json.dumps(v, ensure_ascii=False, sort_keys=True)`.
pub fn dumps_sorted_unicode(v: &PyValue) -> String {
    crate::py::dumps_unicode(&sort_keys(v))
}

/// `json.dumps(v, ensure_ascii=False)`.
pub fn dumps_unicode(v: &PyValue) -> String {
    crate::py::dumps_unicode(v)
}

/// `json.dumps(v)` (ASCII, default separators).
pub fn dumps_ascii(v: &PyValue) -> String {
    v.dumps_ascii().expect("serialisable value")
}

/// `json.dumps(v, separators=(",", ":"))` (ASCII).
pub fn dumps_ascii_tight(v: &PyValue) -> String {
    v.dumps_ascii_tight().expect("serialisable value")
}

fn write_tight(v: &PyValue, out: &mut PyStr) {
    match v.peel() {
        PyValue::None => out.push_str("null"),
        PyValue::Bool(true) => out.push_str("true"),
        PyValue::Bool(false) => out.push_str("false"),
        PyValue::Int(n) => out.push_str(&n.to_string()),
        PyValue::Float(f) => out.push_str(&if f.is_nan() {
            "NaN".to_string()
        } else if f.is_infinite() {
            if *f > 0.0 { "Infinity".into() } else { "-Infinity".into() }
        } else {
            float_repr(*f)
        }),
        PyValue::Str(s) => s.write_json(out, false),
        PyValue::List(items) | PyValue::Tuple(items) => {
            out.push_str("[");
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",");
                }
                write_tight(x, out);
            }
            out.push_str("]");
        }
        PyValue::Dict(d) => {
            out.push_str("{");
            for (i, (k, x)) in d.iter().enumerate() {
                if i > 0 {
                    out.push_str(",");
                }
                k.write_json(out, false);
                out.push_str(":");
                write_tight(x, out);
            }
            out.push_str("}");
        }
        other => out.push_str(&other.repr()),
    }
}

/// `json.dumps(v, sort_keys=True, ensure_ascii=False, separators=(",", ":"))`.
pub fn dumps_sorted_tight_unicode(v: &PyValue) -> String {
    let mut out = PyStr::default();
    write_tight(&sort_keys(v), &mut out);
    crate::py::text(&out)
}

/// `hashlib.sha256(text.encode("utf-8")).hexdigest()`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let d = sha2::Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// `shlex.quote(s)`.
pub fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    let safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if safe {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}

/// `shlex.split(s)` (POSIX mode, no comments) — enough for the self-test's re-reading of emitted commands.
pub fn shlex_split(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut have = false;
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\'' => {
                have = true;
                for d in it.by_ref() {
                    if d == '\'' {
                        break;
                    }
                    cur.push(d);
                }
            }
            '"' => {
                have = true;
                while let Some(d) = it.next() {
                    match d {
                        '"' => break,
                        '\\' => match it.peek() {
                            Some(&e) if "\\\"$`\n".contains(e) => {
                                cur.push(e);
                                it.next();
                            }
                            _ => cur.push('\\'),
                        },
                        _ => cur.push(d),
                    }
                }
            }
            '\\' => {
                have = true;
                if let Some(e) = it.next() {
                    cur.push(e);
                }
            }
            c if c.is_whitespace() => {
                if have {
                    out.push(std::mem::take(&mut cur));
                    have = false;
                }
            }
            c => {
                have = true;
                cur.push(c);
            }
        }
    }
    if have {
        out.push(cur);
    }
    out
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `base64.b64encode(data).decode("ascii")`.
pub fn b64encode(data: &[u8]) -> String {
    let mut o = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        o.push(B64[(n >> 18) as usize & 63] as char);
        o.push(B64[(n >> 12) as usize & 63] as char);
        o.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    o
}

/// `base64.b64decode(text, validate=True)` (strict: alphabet only, correct padding at the end); `None` = binascii.Error.
pub fn b64decode_strict(text: &PyStr) -> Option<Vec<u8>> {
    let pts = text.code_points();
    if pts.len() % 4 != 0 {
        return None;
    }
    let val = |p: u32| -> Option<u32> {
        let c = char::from_u32(p)?;
        B64.iter().position(|&b| b as char == c).map(|x| x as u32)
    };
    let mut out = vec![];
    for (ci, chunk) in pts.chunks(4).enumerate() {
        let last = ci + 1 == pts.len() / 4;
        let pad = chunk.iter().rev().take_while(|&&p| p == u32::from('=')).count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for (i, &p) in chunk.iter().enumerate() {
            let v = if i >= 4 - pad { 0 } else { val(p)? };
            n = (n << 6) | v;
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

pub fn s(t: &str) -> PyValue {
    PyValue::str(t)
}

pub fn int(n: i64) -> PyValue {
    PyValue::int(n)
}

/// A dict literal in insertion order.
pub fn dict(pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in pairs {
        d.set(k, v);
    }
    PyValue::Dict(d)
}

/// `v.get(key)` on a dict (no peel of the result's carrier); `None` for a non-dict or a missing key.
pub fn get<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(key).map(|x| x.peel()),
        _ => None,
    }
}

/// `key in v` for a dict.
pub fn has(v: &PyValue, key: &str) -> bool {
    matches!(v.peel(), PyValue::Dict(d) if d.contains_key(key))
}

/// `v.get(key)` as text when it is a str.
pub fn get_text(v: &PyValue, key: &str) -> Option<String> {
    match get(v, key) {
        Some(PyValue::Str(t)) => Some(crate::py::text(t)),
        _ => None,
    }
}

/// `d[key] = value` on a dict value (no-op on a non-dict).
pub fn set(v: &mut PyValue, key: &str, value: PyValue) {
    if let PyValue::Dict(d) = v {
        d.set(key, value);
    }
}

/// `dict(v)` without `key`.
pub fn without(v: &PyValue, key: &str) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut o = PyDict::new();
            for (k, x) in d.iter() {
                if !k.equals(key) {
                    o.insert(k.clone(), x.clone());
                }
            }
            PyValue::Dict(o)
        }
        other => other.clone(),
    }
}

/// `isinstance(v, int) and not isinstance(v, bool)` and the value fits i64.
pub fn as_int(v: Option<&PyValue>) -> Option<i64> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Int(n)) => i64::try_from(n.clone()).ok(),
        _ => None,
    }
}

pub fn is_plain_int(v: Option<&PyValue>) -> bool {
    matches!(v.map(|x| x.peel()), Some(PyValue::Int(_)))
}

/// `str(v)` (text).
pub fn str_of(v: &PyValue) -> String {
    crate::py::display(v)
}

/// `repr(v)`.
pub fn repr(v: &PyValue) -> String {
    v.repr()
}

/// `repr(text)` of a Rust string.
pub fn repr_str(t: &str) -> String {
    PyStr::new(t).repr()
}

/// The `[-n:]` / `[:n]` code-point slices of a text.
pub fn tail_chars(t: &str, n: usize) -> String {
    let c: Vec<char> = t.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}
pub fn head_chars(t: &str, n: usize) -> String {
    t.chars().take(n).collect()
}
