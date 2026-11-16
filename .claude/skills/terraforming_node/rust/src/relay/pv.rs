//! Python dict/list semantics over `PyValue` as relay.py uses them on JSON documents: `d.get(k)`, truthiness,
//! `==` (order-insensitive dicts, numeric int/float/bool), `json.dump(..., sort_keys=True, indent=2)`, `str()`/`repr()`.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub type V = PyValue;

pub static NONE: V = V::None;

/// `v.get(k)` on a dict; `None` for a missing key or a non-dict (relay always guards with `or {}` / `isinstance`).
pub fn g<'a>(v: &'a V, k: &str) -> &'a V {
    match v.peel() {
        V::Dict(d) => d.get(k).map(|x| x.peel()).unwrap_or(&NONE),
        _ => &NONE,
    }
}

/// `k in v` for a dict.
pub fn has(v: &V, k: &str) -> bool {
    matches!(v.peel(), V::Dict(d) if d.contains_key(k))
}

pub fn is_dict(v: &V) -> bool {
    matches!(v.peel(), V::Dict(_))
}
pub fn is_list(v: &V) -> bool {
    matches!(v.peel(), V::List(_))
}
pub fn is_none(v: &V) -> bool {
    matches!(v.peel(), V::None)
}
pub fn is_str(v: &V) -> bool {
    matches!(v.peel(), V::Str(_))
}
pub fn truthy(v: &V) -> bool {
    v.is_truthy()
}
/// `isinstance(v, int) and not isinstance(v, bool)`.
pub fn is_int(v: &V) -> bool {
    matches!(v.peel(), V::Int(_))
}
pub fn int(v: &V) -> Option<i64> {
    match v.peel() {
        V::Int(n) => i64::try_from(n).ok(),
        _ => None,
    }
}
/// A number (int or float, not bool) as f64.
pub fn num(v: &V) -> Option<f64> {
    match v.peel() {
        V::Int(n) => i64::try_from(n).ok().map(|x| x as f64),
        V::Float(f) => Some(*f),
        _ => None,
    }
}
pub fn text(s: &PyStr) -> String {
    crate::py::text(s)
}
/// The str value (any str, including empty).
pub fn st(v: &V) -> Option<String> {
    match v.peel() {
        V::Str(s) => Some(text(s)),
        _ => None,
    }
}
/// `v is True`.
pub fn is_true(v: &V) -> bool {
    matches!(v.peel(), V::Bool(true))
}
/// `v is False`.
pub fn is_false(v: &V) -> bool {
    matches!(v.peel(), V::Bool(false))
}

pub fn s(t: &str) -> V {
    V::str(t)
}
pub fn i(n: i64) -> V {
    V::int(n)
}
pub fn b(x: bool) -> V {
    V::Bool(x)
}
pub fn opt_s(t: Option<&str>) -> V {
    t.map_or(V::None, s)
}
pub fn opt_i(n: Option<i64>) -> V {
    n.map_or(V::None, i)
}
pub fn list(items: Vec<V>) -> V {
    V::List(items)
}
pub fn strs(items: &[String]) -> V {
    V::List(items.iter().map(|x| s(x)).collect())
}
pub fn obj(pairs: Vec<(&str, V)>) -> V {
    let mut d = PyDict::new();
    for (k, v) in pairs {
        d.set(k, v);
    }
    V::Dict(d)
}
pub fn empty() -> V {
    V::Dict(PyDict::new())
}

/// The list items (`v or []`); empty for a non-list.
pub fn items(v: &V) -> &[V] {
    match v.peel() {
        V::List(l) => l,
        _ => &[],
    }
}
/// Dict entries (key, value) in order.
pub fn entries(v: &V) -> Vec<(String, &V)> {
    match v.peel() {
        V::Dict(d) => d.iter().map(|(k, x)| (text(k), x.peel())).collect(),
        _ => vec![],
    }
}
pub fn keys(v: &V) -> Vec<String> {
    entries(v).into_iter().map(|(k, _)| k).collect()
}

/// `d[k] = val` (no-op on a non-dict; relay only assigns into dicts it built or loaded as dicts).
pub fn set(v: &mut V, k: &str, val: V) {
    if let V::Shared(_) = v {
        let inner = v.peel().clone();
        *v = inner;
    }
    if let V::Dict(d) = v {
        d.set(k, val);
    }
}
/// `d.pop(k, None)`.
pub fn pop(v: &mut V, k: &str) -> V {
    if let V::Shared(_) = v {
        let inner = v.peel().clone();
        *v = inner;
    }
    if let V::Dict(d) = v {
        if !d.contains_key(k) {
            return V::None;
        }
        let mut out = PyDict::new();
        let mut taken = V::None;
        for (key, x) in d.iter() {
            if key.equals(k) {
                taken = x.clone();
            } else {
                out.insert(key.clone(), x.clone());
            }
        }
        *d = out;
        return taken;
    }
    V::None
}
/// Mutable access to `d[k]` (inserting `default` when absent: `setdefault`).
pub fn setdefault<'a>(v: &'a mut V, k: &str, default: V) -> &'a mut V {
    if !has(v, k) {
        set(v, k, default);
    }
    match v {
        V::Dict(d) => d.get_mut(k).unwrap(),
        _ => unreachable!("setdefault on a non-dict"),
    }
}
pub fn get_mut<'a>(v: &'a mut V, k: &str) -> Option<&'a mut V> {
    match v {
        V::Dict(d) => d.get_mut(k),
        _ => None,
    }
}
/// `d.update(other)` (dict spread `{**d, **other}` when applied to a copy).
pub fn update(v: &mut V, other: &V) {
    for (k, x) in entries(other) {
        set(v, &k, x.clone());
    }
}
pub fn append(v: &mut V, x: V) {
    if let V::List(l) = v {
        l.push(x);
    }
}

/// Python `==`: numeric across int/float/bool, dicts order-insensitive, lists/tuples elementwise.
pub fn eq(a: &V, b: &V) -> bool {
    let (a, b) = (a.peel(), b.peel());
    let numeric = |v: &V| match v {
        V::Bool(x) => Some(if *x { 1.0 } else { 0.0 }),
        V::Int(n) => i64::try_from(n).ok().map(|x| x as f64),
        V::Float(f) => Some(*f),
        _ => None,
    };
    match (a, b) {
        (V::Dict(x), V::Dict(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, xv)| match y.get(&text(k)) {
                    Some(yv) => eq(xv, yv),
                    None => false,
                })
        }
        (V::List(x), V::List(y)) | (V::Tuple(x), V::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| eq(p, q))
        }
        (V::Int(x), V::Int(y)) => x == y,
        _ => match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}
pub fn ne(a: &V, b: &V) -> bool {
    !eq(a, b)
}
/// `x in seq` (list of values).
pub fn contains(seq: &V, x: &V) -> bool {
    items(seq).iter().any(|y| eq(y, x))
}
/// `"text" in v` for a list of str (e.g. `"TIMEOUT" in codes`).
pub fn contains_str(seq: &V, x: &str) -> bool {
    items(seq).iter().any(|y| matches!(y.peel(), V::Str(t) if t.equals(x)))
}
/// `v == "text"`.
pub fn is(v: &V, t: &str) -> bool {
    matches!(v.peel(), V::Str(x) if x.equals(t))
}
/// `v in ("a", "b", ...)`.
pub fn is_any(v: &V, ts: &[&str]) -> bool {
    ts.iter().any(|t| is(v, t))
}

/// Python `str(v)`.
pub fn py_str(v: &V) -> String {
    crate::py::display(v)
}
/// Python `repr(v)`.
pub fn py_repr(v: &V) -> String {
    v.repr()
}
/// `repr()` of a text.
pub fn repr_s(t: &str) -> String {
    PyStr::new(t).repr()
}
/// `f"{x}"` of an optional text (`None` when absent).
pub fn fmt_opt(t: &Option<String>) -> String {
    t.clone().unwrap_or_else(|| "None".into())
}

/// Recursively sort dict keys (`sort_keys=True`).
pub fn sorted_keys(v: &V) -> V {
    match v.peel() {
        V::Dict(d) => {
            let mut es: Vec<(PyStr, V)> = d.iter().map(|(k, x)| (k.clone(), sorted_keys(x))).collect();
            es.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (k, x) in es {
                out.insert(k, x);
            }
            V::Dict(out)
        }
        V::List(l) => V::List(l.iter().map(sorted_keys).collect()),
        V::Tuple(l) => V::Tuple(l.iter().map(sorted_keys).collect()),
        other => other.clone(),
    }
}

/// `json.dumps(v, ensure_ascii=False, indent=2, sort_keys=True)`.
pub fn dumps_sorted_pretty(v: &V) -> String {
    crate::py::dumps_pretty(&sorted_keys(v))
}
/// `json.dumps(v, ensure_ascii=False, indent=2)`.
pub fn dumps_pretty(v: &V) -> String {
    crate::py::dumps_pretty(v)
}
/// `json.dumps(v, ensure_ascii=False)`.
pub fn dumps_u(v: &V) -> String {
    crate::py::dumps_unicode(v)
}
/// `json.dumps(v)` (ASCII).
pub fn dumps_a(v: &V) -> String {
    v.dumps_ascii().expect("serialisable value")
}
/// `json.dumps(v, sort_keys=True)` (ASCII, default separators).
pub fn dumps_sorted_a(v: &V) -> String {
    sorted_keys(v).dumps_ascii().expect("serialisable value")
}
/// `json.dumps(v, sort_keys=True, ensure_ascii=False, separators=(",", ":"))`.
pub fn dumps_sorted_tight_u(v: &V) -> String {
    // dumps_unicode uses ", " and ": "; rebuild the tight layout over the sorted value.
    tight(&sorted_keys(v))
}
fn tight(v: &V) -> String {
    match v.peel() {
        V::Dict(d) => {
            let parts: Vec<String> = d
                .iter()
                .map(|(k, x)| format!("{}:{}", dumps_u(&V::Str(k.clone())), tight(x)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        V::List(l) | V::Tuple(l) => format!("[{}]", l.iter().map(tight).collect::<Vec<_>>().join(",")),
        other => dumps_u(other),
    }
}

/// `json.loads(text)` → value or the `ValueError` text.
pub fn loads(t: &str) -> Result<V, String> {
    agent_runtime_support::pyjson::loads(t).map_err(|e| e.to_string())
}
/// `json.load(open(path, encoding="utf-8"))`; `Err` = (exception class, message).
pub fn load_file(path: &std::path::Path) -> Result<V, (String, String)> {
    let bytes = std::fs::read(path).map_err(|e| ("OSError".to_string(), crate::pathfmt::os_error(&e, &path.display().to_string())))?;
    let t = String::from_utf8(bytes).map_err(|e| ("UnicodeDecodeError".to_string(), e.to_string()))?;
    agent_runtime_support::pyjson::loads(&t).map_err(|e| ("json.decoder.JSONDecodeError".to_string(), e.to_string()))
}

/// `sha256(bytes).hexdigest()`.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// `uuid.uuid4().hex` (random 128 bits with the version-4/variant bits set).
pub fn uuid4_hex() -> String {
    use std::io::Read;
    let mut r = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut r);
    }
    r[6] = (r[6] & 0x0f) | 0x40;
    r[8] = (r[8] & 0x3f) | 0x80;
    r.iter().map(|b| format!("{b:02x}")).collect()
}
