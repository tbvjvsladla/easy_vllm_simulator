//! `J`: the dynamic value the Python modules pass around (`dict`/`list`/`str`/`int`/`float`/`bool`/`None`), with
//! insertion-ordered dictionaries (Python 3.7+ `dict`), Python truthiness, and Python-exact `json.dumps` / `json.loads`
//! and `yaml.safe_load` (the readers are the shared `agent-runtime-support` ports).
//!
//! Conventions every ported module follows:
//! - a Python `dict` is `J::Dict(Map)`; key order is insertion order; assigning an existing key keeps its position;
//! - a Python `tuple` is a `J::List` (JSON writes both as arrays);
//! - `j["k"]` / `j.at(i)` never panic: a missing key/index or a non-container yields `&J::Null` (use `.get()` when the
//!   difference between "absent" and `None` matters);
//! - `dumps*` reproduce `json.dumps` byte-for-byte (separators, `ensure_ascii`, `sort_keys`, float repr).
use std::fmt::Write as _;
use std::ops::Index;

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{float_repr, PyDict, PyValue};

/// Insertion-ordered string-keyed map (a Python `dict` with `str` keys).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map(pub Vec<(String, J)>);

impl Map {
    pub fn new() -> Map {
        Map(Vec::new())
    }
    pub fn get(&self, k: &str) -> Option<&J> {
        self.0.iter().find(|(a, _)| a == k).map(|(_, v)| v)
    }
    pub fn get_mut(&mut self, k: &str) -> Option<&mut J> {
        self.0.iter_mut().find(|(a, _)| a == k).map(|(_, v)| v)
    }
    pub fn contains_key(&self, k: &str) -> bool {
        self.0.iter().any(|(a, _)| a == k)
    }
    /// `d[k] = v` (an existing key keeps its position).
    pub fn insert(&mut self, k: impl Into<String>, v: impl Into<J>) {
        let k = k.into();
        let v = v.into();
        if let Some(slot) = self.get_mut(&k) {
            *slot = v;
        } else {
            self.0.push((k, v));
        }
    }
    /// `d.pop(k, None)`.
    pub fn remove(&mut self, k: &str) -> Option<J> {
        let i = self.0.iter().position(|(a, _)| a == k)?;
        Some(self.0.remove(i).1)
    }
    /// `d.setdefault(k, default)`.
    pub fn setdefault(&mut self, k: &str, default: J) -> &mut J {
        if !self.contains_key(k) {
            self.0.push((k.to_string(), default));
        }
        self.get_mut(k).unwrap()
    }
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }
    pub fn values(&self) -> impl Iterator<Item = &J> {
        self.0.iter().map(|(_, v)| v)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &J)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// `{**self, **other}`.
    pub fn update(&mut self, other: &Map) {
        for (k, v) in &other.0 {
            self.insert(k.clone(), v.clone());
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<J>),
    Dict(Map),
}

pub static NULL: J = J::Null;

impl Default for J {
    fn default() -> J {
        J::Null
    }
}

impl J {
    pub fn dict() -> J {
        J::Dict(Map::new())
    }
    pub fn list() -> J {
        J::List(Vec::new())
    }
    pub fn str(s: impl Into<String>) -> J {
        J::Str(s.into())
    }
    pub fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }
    pub fn is_dict(&self) -> bool {
        matches!(self, J::Dict(_))
    }
    pub fn is_list(&self) -> bool {
        matches!(self, J::List(_))
    }
    pub fn is_str(&self) -> bool {
        matches!(self, J::Str(_))
    }
    /// `isinstance(x, bool)`.
    pub fn is_bool(&self) -> bool {
        matches!(self, J::Bool(_))
    }
    /// `isinstance(x, int) and not isinstance(x, bool)`.
    pub fn is_int(&self) -> bool {
        matches!(self, J::Int(_))
    }
    /// `isinstance(x, (int, float)) and not isinstance(x, bool)`.
    pub fn is_number(&self) -> bool {
        matches!(self, J::Int(_) | J::Float(_))
    }
    pub fn as_str(&self) -> Option<&str> {
        if let J::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let J::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        if let J::Int(i) = self {
            Some(*i)
        } else {
            None
        }
    }
    /// int or float as f64 (bool excluded).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            J::Int(i) => Some(*i as f64),
            J::Float(f) => Some(*f),
            _ => None,
        }
    }
    pub fn as_dict(&self) -> Option<&Map> {
        if let J::Dict(m) = self {
            Some(m)
        } else {
            None
        }
    }
    pub fn as_dict_mut(&mut self) -> Option<&mut Map> {
        if let J::Dict(m) = self {
            Some(m)
        } else {
            None
        }
    }
    pub fn as_list(&self) -> Option<&Vec<J>> {
        if let J::List(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_list_mut(&mut self) -> Option<&mut Vec<J>> {
        if let J::List(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// Items of a list (empty for anything else) — `for x in (v if isinstance(v, list) else [])`.
    pub fn items(&self) -> &[J] {
        match self {
            J::List(v) => v,
            _ => &[],
        }
    }
    /// `d.get(k)` for a dict, `None` otherwise.
    pub fn get(&self, k: &str) -> Option<&J> {
        self.as_dict().and_then(|m| m.get(k))
    }
    pub fn get_mut(&mut self, k: &str) -> Option<&mut J> {
        self.as_dict_mut().and_then(|m| m.get_mut(k))
    }
    /// `x[i]` for a list (`&J::Null` when out of range).
    pub fn at(&self, i: usize) -> &J {
        self.items().get(i).unwrap_or(&NULL)
    }
    /// `d.get(k)` that is a `str`.
    pub fn get_str(&self, k: &str) -> Option<&str> {
        self.get(k).and_then(J::as_str)
    }
    /// Set a key on a dict value (no-op on anything else).
    pub fn set(&mut self, k: impl Into<String>, v: impl Into<J>) {
        if let J::Dict(m) = self {
            m.insert(k, v);
        }
    }
    pub fn push(&mut self, v: impl Into<J>) {
        if let J::List(l) = self {
            l.push(v.into());
        }
    }
    /// Python truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            J::Null => false,
            J::Bool(b) => *b,
            J::Int(i) => *i != 0,
            J::Float(f) => *f != 0.0,
            J::Str(s) => !s.is_empty(),
            J::List(v) => !v.is_empty(),
            J::Dict(m) => !m.is_empty(),
        }
    }
    /// Python `len()` of str (code points), list, dict; 0 otherwise.
    pub fn len(&self) -> usize {
        match self {
            J::Str(s) => s.chars().count(),
            J::List(v) => v.len(),
            J::Dict(m) => m.len(),
            _ => 0,
        }
    }
    /// Python type name (`type(x).__name__`).
    pub fn type_name(&self) -> &'static str {
        match self {
            J::Null => "NoneType",
            J::Bool(_) => "bool",
            J::Int(_) => "int",
            J::Float(_) => "float",
            J::Str(_) => "str",
            J::List(_) => "list",
            J::Dict(_) => "dict",
        }
    }
    /// Python `repr(x)`.
    pub fn repr(&self) -> String {
        match self {
            J::Null => "None".into(),
            J::Bool(true) => "True".into(),
            J::Bool(false) => "False".into(),
            J::Int(i) => i.to_string(),
            J::Float(f) => float_repr(*f),
            J::Str(s) => py_repr(s),
            J::List(v) => {
                let parts: Vec<String> = v.iter().map(J::repr).collect();
                format!("[{}]", parts.join(", "))
            }
            J::Dict(m) => {
                let parts: Vec<String> = m.0.iter().map(|(k, v)| format!("{}: {}", py_repr(k), v.repr())).collect();
                format!("{{{}}}", parts.join(", "))
            }
        }
    }
    /// Python `str(x)`: a string is itself, everything else its repr.
    pub fn display(&self) -> String {
        match self {
            J::Str(s) => s.clone(),
            other => other.repr(),
        }
    }
}

impl Index<&str> for J {
    type Output = J;
    fn index(&self, k: &str) -> &J {
        self.get(k).unwrap_or(&NULL)
    }
}

impl Index<usize> for J {
    type Output = J;
    fn index(&self, i: usize) -> &J {
        self.at(i)
    }
}

impl From<&str> for J {
    fn from(s: &str) -> J {
        J::Str(s.to_string())
    }
}
impl From<String> for J {
    fn from(s: String) -> J {
        J::Str(s)
    }
}
impl From<&String> for J {
    fn from(s: &String) -> J {
        J::Str(s.clone())
    }
}
impl From<bool> for J {
    fn from(b: bool) -> J {
        J::Bool(b)
    }
}
impl From<i64> for J {
    fn from(i: i64) -> J {
        J::Int(i)
    }
}
impl From<i32> for J {
    fn from(i: i32) -> J {
        J::Int(i as i64)
    }
}
impl From<usize> for J {
    fn from(i: usize) -> J {
        J::Int(i as i64)
    }
}
impl From<f64> for J {
    fn from(f: f64) -> J {
        J::Float(f)
    }
}
impl From<Map> for J {
    fn from(m: Map) -> J {
        J::Dict(m)
    }
}
impl From<Vec<J>> for J {
    fn from(v: Vec<J>) -> J {
        J::List(v)
    }
}
impl From<Vec<String>> for J {
    fn from(v: Vec<String>) -> J {
        J::List(v.into_iter().map(J::Str).collect())
    }
}
impl From<&[String]> for J {
    fn from(v: &[String]) -> J {
        J::List(v.iter().cloned().map(J::Str).collect())
    }
}
impl From<Vec<&str>> for J {
    fn from(v: Vec<&str>) -> J {
        J::List(v.into_iter().map(J::from).collect())
    }
}
impl<T: Into<J>> From<Option<T>> for J {
    fn from(o: Option<T>) -> J {
        match o {
            Some(v) => v.into(),
            None => J::Null,
        }
    }
}

/// Build a `J::Dict` in insertion order: `jd!{"a" => 1, "b" => "x"}`.
#[macro_export]
macro_rules! jd {
    () => { $crate::value::J::Dict($crate::value::Map::new()) };
    ($($k:expr => $v:expr),+ $(,)?) => {{
        let mut m = $crate::value::Map::new();
        $( m.insert($k, $crate::value::J::from($v)); )+
        $crate::value::J::Dict(m)
    }};
}

/// Build a `J::List`: `jl![1, "x"]`.
#[macro_export]
macro_rules! jl {
    () => { $crate::value::J::List(Vec::new()) };
    ($($v:expr),+ $(,)?) => { $crate::value::J::List(vec![$($crate::value::J::from($v)),+]) };
}

// ── repr ───────────────────────────────────────────────────────────────────────────────────────
/// Python `repr(str)`.
pub fn py_repr(s: &str) -> String {
    PyStr::new(s).repr()
}

// ── conversion from the shared Python-compatible readers ───────────────────────────────────────
pub fn from_py(v: &PyValue) -> J {
    match v.peel() {
        PyValue::None => J::Null,
        PyValue::Bool(b) => J::Bool(*b),
        PyValue::Int(i) => {
            use num_traits::ToPrimitive;
            match i.to_i64() {
                Some(x) => J::Int(x),
                None => J::Float(i.to_f64().unwrap_or(f64::INFINITY)),
            }
        }
        PyValue::Float(f) => J::Float(*f),
        PyValue::Str(s) => J::Str(pystr_lossy(s)),
        PyValue::List(v) | PyValue::Tuple(v) => J::List(v.iter().map(from_py).collect()),
        PyValue::Dict(d) => J::Dict(from_pydict(d)),
        PyValue::Bytes(b) => J::Str(String::from_utf8_lossy(b).into_owned()),
        PyValue::Opaque(o) => J::Str(o.repr.clone()),
        PyValue::Shared(inner) => from_py(inner),
    }
}

fn from_pydict(d: &PyDict) -> Map {
    let mut m = Map::new();
    for (other, k, v) in d.keyed_entries() {
        let key = match other {
            Some(_) => pystr_lossy(k),
            None => pystr_lossy(k),
        };
        m.insert(key, from_py(v));
    }
    m
}

pub fn pystr_lossy(s: &PyStr) -> String {
    match s.to_utf8() {
        Some(t) => t,
        None => s.code_points().iter().map(|&c| char::from_u32(c).unwrap_or('\u{FFFD}')).collect(),
    }
}

/// `json.loads(text)`; the error text is Python's `str(JSONDecodeError)` (e.g. `Expecting value: line 1 column 1 (char 0)`).
pub fn loads(text: &str) -> Result<J, String> {
    agent_runtime_support::pyjson::loads(text).map(|v| from_py(&v)).map_err(|e| e.to_string())
}

/// `yaml.safe_load(text)` of a `str`; the error text is `str(exc)` of the PyYAML error.
pub fn yaml_loads(text: &str) -> Result<J, String> {
    agent_runtime_support::pyyaml::safe_load_text(&PyStr::new(text), 3).map(|v| from_py(&v)).map_err(|e| pystr_lossy(&e.message))
}

// ── json.dumps ─────────────────────────────────────────────────────────────────────────────────
/// Options of `json.dumps`.
#[derive(Clone, Copy, Debug)]
pub struct Dump {
    pub indent: Option<usize>,
    pub item_sep: &'static str,
    pub key_sep: &'static str,
    pub ensure_ascii: bool,
    pub sort_keys: bool,
}

impl Dump {
    /// `json.dumps(x)` defaults (`ensure_ascii=True`, `", "`/`": "`).
    pub const DEFAULT: Dump = Dump { indent: None, item_sep: ", ", key_sep: ": ", ensure_ascii: true, sort_keys: false };
}

/// `json.dumps(x)`.
pub fn dumps_default(v: &J) -> String {
    dumps_with(v, Dump::DEFAULT)
}

/// `json.dumps(x, ensure_ascii=False)`.
pub fn dumps_unicode(v: &J) -> String {
    dumps_with(v, Dump { ensure_ascii: false, ..Dump::DEFAULT })
}

/// `json.dumps(x, ensure_ascii=False, indent=2)` (Python: with indent, the item separator is `","`).
pub fn dumps_pretty(v: &J) -> String {
    dumps_with(v, Dump { indent: Some(2), item_sep: ",", key_sep: ": ", ensure_ascii: false, sort_keys: false })
}

/// `json.dumps(x, ensure_ascii=False, indent=2, sort_keys=True)`.
pub fn dumps_pretty_sorted(v: &J) -> String {
    dumps_with(v, Dump { indent: Some(2), item_sep: ",", key_sep: ": ", ensure_ascii: false, sort_keys: true })
}

/// `json.dumps(x, ensure_ascii=False, separators=(",", ":"), sort_keys=True)`.
pub fn dumps_tight_sorted(v: &J) -> String {
    dumps_with(v, Dump { indent: None, item_sep: ",", key_sep: ":", ensure_ascii: false, sort_keys: true })
}

pub fn dumps_with(v: &J, o: Dump) -> String {
    let mut out = String::new();
    write_value(&mut out, v, &o, 0);
    out
}

fn write_value(out: &mut String, v: &J, o: &Dump, level: usize) {
    match v {
        J::Null => out.push_str("null"),
        J::Bool(true) => out.push_str("true"),
        J::Bool(false) => out.push_str("false"),
        J::Int(i) => {
            let _ = write!(out, "{i}");
        }
        J::Float(f) => out.push_str(&float_json(*f)),
        J::Str(s) => write_str(out, s, o.ensure_ascii),
        J::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(o.item_sep);
                }
                newline(out, o, level + 1);
                write_value(out, x, o, level + 1);
            }
            newline(out, o, level);
            out.push(']');
        }
        J::Dict(m) => {
            if m.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut entries: Vec<&(String, J)> = m.0.iter().collect();
            if o.sort_keys {
                // Python sorts str keys by code point; UTF-8 byte order is the same order.
                entries.sort_by(|a, b| a.0.cmp(&b.0));
            }
            out.push('{');
            for (i, (k, x)) in entries.into_iter().map(|e| (&e.0, &e.1)).enumerate() {
                if i > 0 {
                    out.push_str(o.item_sep);
                }
                newline(out, o, level + 1);
                write_str(out, k, o.ensure_ascii);
                out.push_str(o.key_sep);
                write_value(out, x, o, level + 1);
            }
            newline(out, o, level);
            out.push('}');
        }
    }
}

fn newline(out: &mut String, o: &Dump, level: usize) {
    if let Some(n) = o.indent {
        out.push('\n');
        for _ in 0..n * level {
            out.push(' ');
        }
    }
}

fn float_json(f: f64) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        float_repr(f)
    }
}

fn write_str(out: &mut String, s: &str, ascii: bool) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c if ascii && (c as u32) > 0x7e => {
                let cp = c as u32;
                if cp > 0xFFFF {
                    let v = cp - 0x10000;
                    let _ = write!(out, "\\u{:04x}\\u{:04x}", 0xD800 + (v >> 10), 0xDC00 + (v & 0x3FF));
                } else {
                    let _ = write!(out, "\\u{:04x}", cp);
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `sorted(list_of_str)` / `sorted(set_of_str)` (code point order).
pub fn sorted_strs<I: IntoIterator<Item = S>, S: Into<String>>(it: I) -> Vec<String> {
    let mut v: Vec<String> = it.into_iter().map(Into::into).collect();
    v.sort();
    v
}
