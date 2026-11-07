//! JSON values with the Python types the original program observes: `int` is arbitrary precision and distinct
//! from `bool` and `float`, dicts keep insertion order, strings are Python strings.

use crate::pystr::PyStr;
use num_bigint::BigInt;
use num_traits::{FromPrimitive, Zero};
use std::fmt;
use std::sync::{Arc, OnceLock};

/// `sys.get_int_max_str_digits()` default: Python refuses to render longer integers.
const MAX_INT_DIGITS: usize = 4300;
/// Deepest container nesting `json.dumps(indent=2)` can write. With an indent Python uses its pure-Python
/// encoder, which recurses once per container, so the interpreter's frame limit applies. Measured on the
/// original CLI: a verdict whose deepest container is the 995th level prints, the 996th raises `RecursionError`.
const PRETTY_MAX_DEPTH: usize = 995;

/// Why a value could not be rendered as JSON; each case is an exception Python raises while printing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpError {
    /// An integer with more than 4300 digits (`ValueError` from `int.__repr__`).
    IntegerTooLong,
    /// Containers nested beyond what the pure-Python encoder can recurse through (`RecursionError`).
    TooDeep,
    /// A value of a type JSON cannot carry (`TypeError: Object of type date is not JSON serializable`); holds the
    /// type's short name.
    NotSerializable(&'static str),
    /// A value that contains itself (`ValueError: Circular reference detected`).
    Circular,
    /// A dict key JSON cannot spell (`TypeError: keys must be str, int, float, bool or None, not date`).
    KeyNotAllowed(&'static str),
}

impl DumpError {
    pub fn python_type(&self) -> &'static str {
        match self {
            DumpError::IntegerTooLong => "ValueError",
            DumpError::TooDeep => "RecursionError",
            DumpError::NotSerializable(_) => "TypeError",
            DumpError::Circular => "ValueError",
            DumpError::KeyNotAllowed(_) => "TypeError",
        }
    }
}

impl fmt::Display for DumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DumpError::IntegerTooLong => write!(
                f,
                "ValueError: Exceeds the limit ({MAX_INT_DIGITS} digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit"
            ),
            DumpError::TooDeep => f.write_str("RecursionError: maximum recursion depth exceeded"),
            DumpError::NotSerializable(name) => write!(f, "TypeError: Object of type {name} is not JSON serializable"),
            DumpError::Circular => f.write_str("ValueError: Circular reference detected"),
            DumpError::KeyNotAllowed(name) => write!(f, "TypeError: keys must be str, int, float, bool or None, not {name}"),
        }
    }
}

impl std::error::Error for DumpError {}

#[derive(Clone, Debug)]
pub enum PyValue {
    None,
    Bool(bool),
    Int(BigInt),
    Float(f64),
    Str(PyStr),
    List(Vec<PyValue>),
    Dict(PyDict),
    /// `bytes` (what YAML's `!!binary` makes). Never produced by the JSON reader.
    Bytes(Vec<u8>),
    /// A `tuple` (items of YAML's `!!omap` and `!!pairs`); JSON writes it as an array.
    Tuple(Vec<PyValue>),
    /// A Python object JSON cannot carry (`datetime.date`, `datetime.datetime`, `set`) known by its type name and `repr`.
    Opaque(Opaque),
    /// One Python object that several places of a YAML document refer to (an alias target). Python shares the object;
    /// so does this: the wrapper is cloned by reference, never by content. It is transparent - every operation below
    /// sees the value inside - and only the YAML reader produces it, around collections that more than one place of
    /// the document refers to. Code that matches on the variant of a value taken out of a YAML document calls
    /// [`PyValue::peel`] first.
    Shared(Arc<PyValue>),
}

impl PartialEq for PyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self.peel(), other.peel()) {
            (PyValue::None, PyValue::None) => true,
            (PyValue::Bool(a), PyValue::Bool(b)) => a == b,
            (PyValue::Int(a), PyValue::Int(b)) => a == b,
            (PyValue::Float(a), PyValue::Float(b)) => a == b,
            (PyValue::Str(a), PyValue::Str(b)) => a == b,
            (PyValue::List(a), PyValue::List(b)) => a == b,
            (PyValue::Dict(a), PyValue::Dict(b)) => a == b,
            (PyValue::Bytes(a), PyValue::Bytes(b)) => a == b,
            (PyValue::Tuple(a), PyValue::Tuple(b)) => a == b,
            (PyValue::Opaque(a), PyValue::Opaque(b)) => a == b,
            _ => false,
        }
    }
}

/// NaN objects. `nan != nan` for every pair of numbers, but Python's `in`, dict lookup and container equality first ask
/// whether both sides are the *same object*, so a NaN that two places share (`json.decoder`'s one `NaN` constant,
/// `SafeConstructor.nan_value`, a YAML node reached through an alias) is found where a fresh NaN is not.
///
/// An object is an `Arc` around the number, carried as [`PyValue::Shared`] (the carrier YAML aliases already use):
/// cloning a value clones the reference, so one object stays one object, and two separate `Arc`s are two objects. Its
/// identity is the allocation, so nothing is encoded in the number and nothing can collide. A bare `PyValue::Float`
/// is a NaN object nobody else refers to. Only the decoders that were asked for identity (see `pyjson::loads_objects`
/// and `pyyaml::safe_load_text`) produce the carrier; every other reader keeps plain floats.
pub fn nan_object() -> PyValue {
    PyValue::Shared(Arc::new(PyValue::Float(f64::NAN)))
}

fn singleton(cell: &'static OnceLock<Arc<PyValue>>) -> PyValue {
    PyValue::Shared(
        cell.get_or_init(|| Arc::new(PyValue::Float(f64::NAN)))
            .clone(),
    )
}

/// The one object every JSON document decodes the literal `NaN` to (`json.decoder`'s constant).
pub fn json_nan_constant() -> PyValue {
    static CELL: OnceLock<Arc<PyValue>> = OnceLock::new();
    singleton(&CELL)
}

/// The one object `.nan` is in every YAML document (`SafeConstructor.nan_value`); not JSON's.
pub fn yaml_nan_constant() -> PyValue {
    static CELL: OnceLock<Arc<PyValue>> = OnceLock::new();
    singleton(&CELL)
}

/// Python `a is b` for NaN objects: both are the same carrier. Never true for numbers that merely compare.
pub fn same_nan_object(a: &PyValue, b: &PyValue) -> bool {
    match (a, b) {
        (PyValue::Shared(x), PyValue::Shared(y)) => {
            Arc::ptr_eq(x, y) && matches!(x.peel(), PyValue::Float(number) if number.is_nan())
        }
        _ => false,
    }
}

/// The structured data of a YAML-only value, kept beside its `repr` so that a reader never has to parse a display string: the
/// `isoformat()` of a `datetime.date`/`datetime.datetime` as Python prints it, the members of a `set` (deduplicated like its `repr`).
/// Not part of equality (the `repr` already is).
#[derive(Clone, Debug, Default)]
pub enum Typed {
    #[default]
    Plain,
    Date(String),
    DateTime(String),
    Set(Vec<PyValue>),
}

/// A value of a type that only YAML produces; it has a type name, a `repr` and a truth value, and cannot be dumped.
#[derive(Clone, Debug)]
pub struct Opaque {
    pub type_name: &'static str,
    pub repr: String,
    pub truthy: bool,
    pub typed: Typed,
    /// What Python's `==`/`hash` make of the value as a dict key or set member, for the types whose equality is not their `repr`
    /// (`datetime.date`, `datetime.datetime`: an aware one is its UTC instant). `None` for a type that has no such identity here.
    /// Not part of this struct's own equality.
    pub identity: Option<String>,
}

impl PartialEq for Opaque {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name && self.repr == other.repr && self.truthy == other.truthy
    }
}

impl Eq for Opaque {}

/// A dict key that is not a string. YAML allows any hashable value as a key (`1: x`, `true: y`, `2026-01-01: z`);
/// JSON never produces one. It is told apart from strings by *type*, not by how it is spelled: a user string, whatever
/// characters it holds, is never one of these.
#[derive(Clone, Debug)]
pub struct NonStringKey {
    /// Equal exactly when Python's keys are equal (`1`, `1.0` and `True` share one; `nan` never equals itself).
    pub identity: String,
    /// What Python's `repr` prints for the key.
    pub repr: String,
    /// The key itself, where the reader that made it kept it (YAML): `None`, a number, a `bool`, `bytes`, a date. Not part of equality.
    pub value: Option<Box<PyValue>>,
}

impl PartialEq for NonStringKey {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity && self.repr == other.repr
    }
}

impl Eq for NonStringKey {}

/// A key as a dict is given it.
#[derive(Clone, Debug)]
pub enum DictKey {
    Str(PyStr),
    Other(NonStringKey),
}

/// Insertion-ordered dict. Assigning an existing key replaces the value and keeps the original position,
/// as Python does for duplicate keys in a JSON object.
///
/// Entries hold the key text (for a non-string key: its `repr`). Which entries are non-string keys is kept beside them,
/// by position and only for the dicts that have any, so a JSON object carries no extra data and no check on its keys.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PyDict {
    entries: Vec<(PyStr, PyValue)>,
    /// `(position in entries, key)` of the non-string keys, ascending.
    others: Vec<(usize, NonStringKey)>,
}

/// Whether no two entries share a key (compared by code points), in time linear in the number of entries.
fn keys_are_distinct(entries: &[(PyStr, PyValue)]) -> bool {
    let mut seen = std::collections::HashSet::with_capacity(entries.len());
    entries
        .iter()
        .all(|(key, _)| seen.insert(key.code_points()))
}

impl PyDict {
    pub fn new() -> Self {
        Self::default()
    }

    /// A dict display `{k1: v1, k2: v2, ...}` whose keys are strings written out in the source and differ from each
    /// other: there is nothing to replace, so the entries are taken as they are. (Debug builds check the claim.)
    pub fn from_distinct_entries<const N: usize>(items: [(PyStr, PyValue); N]) -> Self {
        debug_assert!(
            items
                .iter()
                .enumerate()
                .all(|(at, (key, _))| items[..at].iter().all(|(earlier, _)| earlier != key)),
            "dict display with a repeated key"
        );
        PyDict {
            entries: Vec::from(items),
            others: Vec::new(),
        }
    }

    /// The dict of entries that are already settled: string keys that differ pairwise (by code points), in insertion order. Nothing is replaced and no
    /// key is looked up, so the entries are taken as they are.
    ///
    /// Crate-private, with a precondition the caller owns: **no key may repeat**. The JSON object reader settles repeated keys (first position, last
    /// value) before it calls this. Debug builds check the precondition in linear time with a set of the keys' code points.
    pub(crate) fn from_settled_entries(entries: Vec<(PyStr, PyValue)>) -> Self {
        debug_assert!(
            keys_are_distinct(&entries),
            "settled entries with a repeated key"
        );
        PyDict {
            entries,
            others: Vec::new(),
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    pub fn with_capacity(capacity: usize) -> Self {
        PyDict {
            entries: Vec::with_capacity(capacity),
            others: Vec::new(),
        }
    }

    /// Whether the entry at `at` has a non-string key.
    fn is_other(&self, at: usize) -> bool {
        !self.others.is_empty()
            && self
                .others
                .binary_search_by_key(&at, |(index, _)| *index)
                .is_ok()
    }

    /// Insert under a string key; a non-string key never equals it.
    pub fn insert(&mut self, key: PyStr, value: PyValue) {
        let found = self
            .entries
            .iter()
            .enumerate()
            .position(|(at, (existing, _))| *existing == key && !self.is_other(at));
        match found {
            Some(at) => self.entries[at].1 = value,
            None => self.entries.push((key, value)),
        }
    }

    /// Insert under a key of either kind.
    pub fn insert_key(&mut self, key: DictKey, value: PyValue) {
        match key {
            DictKey::Str(text) => self.insert(text, value),
            DictKey::Other(other) => {
                let found = self
                    .others
                    .iter()
                    .find(|(_, existing)| existing.identity == other.identity)
                    .map(|(at, _)| *at);
                match found {
                    Some(at) => self.entries[at].1 = value,
                    None => {
                        self.others.push((self.entries.len(), other.clone()));
                        self.entries.push((PyStr::new(&other.repr), value));
                    }
                }
            }
        }
    }

    /// Convenience for keys known at compile time. `#[inline]` for the same reason as `PyStr::is_blank`: object builders in
    /// the owner crates call it many times in a row.
    #[inline]
    pub fn set(&mut self, key: &str, value: PyValue) {
        self.insert(PyStr::new(key), value);
    }

    pub fn get(&self, key: &str) -> Option<&PyValue> {
        self.entries
            .iter()
            .enumerate()
            .find(|(at, (existing, _))| existing.equals(key) && !self.is_other(*at))
            .map(|(_, (_, value))| value)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// The value under a string key, for in-place change.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut PyValue> {
        let at = self
            .entries
            .iter()
            .enumerate()
            .position(|(at, (existing, _))| existing.equals(key) && !self.is_other(at))?;
        Some(&mut self.entries[at].1)
    }

    /// `dict.setdefault(key, default)` for a key of either kind: the stored value (kept when the key is present, even if
    /// it is `None`), else `default` stored and returned.
    pub fn setdefault_key(&mut self, key: DictKey, default: PyValue) -> &mut PyValue {
        let present = match &key {
            DictKey::Str(text) => self
                .entries
                .iter()
                .enumerate()
                .position(|(at, (existing, _))| existing == text && !self.is_other(at)),
            DictKey::Other(other) => self
                .others
                .iter()
                .find(|(_, existing)| existing.identity == other.identity)
                .map(|(at, _)| *at),
        };
        let at = match present {
            Some(at) => at,
            None => {
                self.insert_key(key, default);
                self.entries.len() - 1
            }
        };
        &mut self.entries[at].1
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries in order, keys as text (a non-string key by its `repr`; see [`PyDict::keyed_entries`]).
    pub fn iter(&self) -> impl Iterator<Item = &(PyStr, PyValue)> {
        self.entries.iter()
    }

    /// The entries in order with each key's kind.
    pub fn keyed_entries(&self) -> impl Iterator<Item = (Option<&NonStringKey>, &PyStr, &PyValue)> {
        let mut others = self.others.iter().peekable();
        self.entries
            .iter()
            .enumerate()
            .map(move |(at, (key, value))| {
                let other = match others.peek() {
                    Some((index, _)) if *index == at => others.next().map(|(_, other)| other),
                    _ => None,
                };
                (other, key, value)
            })
    }

    /// Hands the dict's entries over by value, in insertion order, when every key is a string.
    ///
    /// On success the keys and values are moved out of the dict, not cloned (the entry vector itself is the iterator's
    /// storage), so the caller owns each `(PyStr, PyValue)` pair as stored. Values the caller does not take from the
    /// iterator are dropped normally when the iterator is dropped.
    ///
    /// A dict with any non-string key is refused: it comes back unchanged as `Err(self)`, with every entry and all of
    /// its non-string key metadata (identity, `repr`, kept value) kept, and nothing is allocated, converted or modified.
    /// An empty dict has only string keys and yields an empty iterator.
    pub fn try_into_string_entries(
        self,
    ) -> Result<impl ExactSizeIterator<Item = (PyStr, PyValue)>, Self> {
        if self.others.is_empty() {
            Ok(self.entries.into_iter())
        } else {
            Err(self)
        }
    }
}

impl PyValue {
    /// The value without the sharing wrapper.
    pub fn peel(&self) -> &PyValue {
        let mut value = self;
        while let PyValue::Shared(inner) = value {
            value = inner;
        }
        value
    }

    pub fn int(value: impl Into<BigInt>) -> PyValue {
        PyValue::Int(value.into())
    }

    pub fn str(text: &str) -> PyValue {
        PyValue::Str(PyStr::new(text))
    }

    /// Python `type(value).__name__`.
    pub fn type_name(&self) -> &'static str {
        match self {
            PyValue::None => "NoneType",
            PyValue::Bool(_) => "bool",
            PyValue::Int(_) => "int",
            PyValue::Float(_) => "float",
            PyValue::Str(_) => "str",
            PyValue::List(_) => "list",
            PyValue::Dict(_) => "dict",
            PyValue::Bytes(_) => "bytes",
            PyValue::Tuple(_) => "tuple",
            PyValue::Opaque(opaque) => opaque.type_name,
            PyValue::Shared(inner) => inner.type_name(),
        }
    }

    /// Python `bool(value)`.
    pub fn is_truthy(&self) -> bool {
        match self {
            PyValue::None => false,
            PyValue::Bool(flag) => *flag,
            PyValue::Int(number) => !number.is_zero(),
            PyValue::Float(number) => *number != 0.0,
            PyValue::Str(text) => !text.is_empty(),
            PyValue::List(items) => !items.is_empty(),
            PyValue::Dict(map) => !map.is_empty(),
            PyValue::Bytes(bytes) => !bytes.is_empty(),
            PyValue::Tuple(items) => !items.is_empty(),
            PyValue::Opaque(opaque) => opaque.truthy,
            PyValue::Shared(inner) => inner.is_truthy(),
        }
    }

    /// The value when it is a real `int` (`bool` is excluded, as `isinstance(v, int) and not isinstance(v, bool)`).
    pub fn as_plain_int(&self) -> Option<&BigInt> {
        match self.peel() {
            PyValue::Int(number) => Some(number),
            _ => None,
        }
    }

    /// Python `value == number` for a non-negative `int` the caller holds.
    pub fn equals_int(&self, number: &BigInt) -> bool {
        match self.peel() {
            PyValue::Int(own) => own == number,
            PyValue::Bool(flag) => BigInt::from(u8::from(*flag)) == *number,
            PyValue::Float(own) => {
                own.is_finite()
                    && own.fract() == 0.0
                    && BigInt::from_f64(*own).as_ref() == Some(number)
            }
            _ => false,
        }
    }

    /// Python `repr(value)`.
    pub fn repr(&self) -> String {
        match self {
            PyValue::None => "None".to_string(),
            PyValue::Bool(true) => "True".to_string(),
            PyValue::Bool(false) => "False".to_string(),
            PyValue::Int(number) => number.to_string(),
            PyValue::Float(number) => float_repr(*number),
            PyValue::Str(text) => text.repr(),
            PyValue::List(items) => {
                let inner: Vec<String> = items.iter().map(PyValue::repr).collect();
                format!("[{}]", inner.join(", "))
            }
            PyValue::Dict(map) => {
                let inner: Vec<String> = map
                    .keyed_entries()
                    .map(|(other, key, value)| {
                        let key = other.map_or_else(|| key.repr(), |other| other.repr.clone());
                        format!("{key}: {}", value.repr())
                    })
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            PyValue::Bytes(bytes) => bytes_repr(bytes),
            PyValue::Tuple(items) => {
                let inner: Vec<String> = items.iter().map(PyValue::repr).collect();
                if inner.len() == 1 {
                    format!("({},)", inner[0])
                } else {
                    format!("({})", inner.join(", "))
                }
            }
            PyValue::Opaque(opaque) => opaque.repr.clone(),
            PyValue::Shared(inner) => inner.repr(),
        }
    }

    /// Python `str(value)`: a string is itself, everything else is its repr.
    pub fn display_text(&self) -> PyStr {
        match self.peel() {
            PyValue::Str(text) => text.clone(),
            other => PyStr::new(&other.repr()),
        }
    }

    /// `json.dumps(value, ensure_ascii=False, indent=2)`; the text may still hold lone surrogates.
    pub fn dumps_pretty(&self) -> Result<PyStr, DumpError> {
        self.dumps_indented(2)
    }

    /// `json.dumps(value, ensure_ascii=False, indent=indent)` for a positive integer `indent`; the text may still
    /// hold lone surrogates.
    pub fn dumps_indented(&self, indent: usize) -> Result<PyStr, DumpError> {
        let mut out = PyStr::default();
        self.write_json(&mut out, Layout::Pretty { level: 0, indent }, false)?;
        Ok(out)
    }

    /// `json.dumps(value, ensure_ascii=False)`: default separators, non-ASCII text kept as is (it may still hold lone
    /// surrogates).
    pub fn dumps_unicode(&self) -> Result<PyStr, DumpError> {
        let mut out = PyStr::default();
        self.write_json(&mut out, Layout::Compact, false)?;
        Ok(out)
    }

    /// `json.dumps(value, separators=(",", ":"))`: the tightest layout with `ensure_ascii=True`, so the text is pure ASCII.
    pub fn dumps_ascii_tight(&self) -> Result<String, DumpError> {
        let mut out = PyStr::default();
        self.write_json(&mut out, Layout::Tight, true)?;
        Ok(out.to_utf8().expect("ensure_ascii output is ASCII"))
    }

    /// `json.dumps(value)`: default separators and `ensure_ascii=True`, so the text is pure ASCII.
    pub fn dumps_ascii(&self) -> Result<String, DumpError> {
        let mut out = PyStr::default();
        self.write_json(&mut out, Layout::Compact, true)?;
        Ok(out.to_utf8().expect("ensure_ascii output is ASCII"))
    }

    fn write_json(&self, out: &mut PyStr, layout: Layout, ascii: bool) -> Result<(), DumpError> {
        match self {
            PyValue::None => out.push_str("null"),
            PyValue::Bool(true) => out.push_str("true"),
            PyValue::Bool(false) => out.push_str("false"),
            PyValue::Int(number) => {
                let digits = number.to_string();
                if digits.trim_start_matches('-').len() > MAX_INT_DIGITS {
                    return Err(DumpError::IntegerTooLong);
                }
                out.push_str(&digits);
            }
            PyValue::Float(number) => out.push_str(&float_json(*number)),
            PyValue::Str(text) => text.write_json(out, ascii),
            PyValue::List(items) => {
                write_sequence(out, layout, ('[', ']'), items.len(), |out, index, inner| {
                    items[index].write_json(out, inner, ascii)
                })?
            }
            PyValue::Bytes(_) => return Err(DumpError::NotSerializable("bytes")),
            PyValue::Tuple(items) => {
                write_sequence(out, layout, ('[', ']'), items.len(), |out, index, inner| {
                    items[index].write_json(out, inner, ascii)
                })?
            }
            PyValue::Shared(inner) => inner.write_json(out, layout, ascii)?,
            PyValue::Opaque(opaque) if opaque.repr == "[...]" || opaque.repr == "{...}" => {
                return Err(DumpError::Circular)
            }
            PyValue::Opaque(opaque) => {
                return Err(DumpError::NotSerializable(
                    opaque
                        .type_name
                        .rsplit('.')
                        .next()
                        .unwrap_or(opaque.type_name),
                ))
            }
            PyValue::Dict(map) => {
                let entries: Vec<(Option<&NonStringKey>, &PyStr, &PyValue)> =
                    map.keyed_entries().collect();
                write_sequence(
                    out,
                    layout,
                    ('{', '}'),
                    entries.len(),
                    |out, index, inner| {
                        let (other, key, value) = entries[index];
                        match other {
                            Some(other) => json_key(other)?.write_json(out, ascii),
                            None => key.write_json(out, ascii),
                        }
                        out.push_str(if matches!(inner, Layout::Tight) {
                            ":"
                        } else {
                            ": "
                        });
                        value.write_json(out, inner, ascii)
                    },
                )?
            }
        }
        Ok(())
    }
}

/// Why a streamed JSON dump stopped: the value could not be encoded (what `json.dump` raises while it iterates), or the
/// caller's sink failed on a chunk (the text file's `write`).
#[derive(Debug)]
pub enum StreamError<E> {
    Dump(DumpError),
    Sink(E),
}

/// The chunk source of `json.dump(value, fp, ensure_ascii=False, indent=indent)`: `json.dump` iterates the pure-Python
/// `json.encoder._make_iterencode` (never the one-shot C encoder) and writes each chunk it yields, so the chunk boundaries
/// decide what a file holds when a later chunk fails. A list yields `[` joined to its first newline-and-indent and each item
/// joined to its separator (a nested container is preceded by its own separator chunk), a dict yields `{`, the newline-and-indent,
/// each separator, each key, `: ` and each value as chunks of their own; an empty container is one chunk. A scalar's text is
/// computed before the chunk that carries it is yielded, so an integer too long to print fails without its separator.
struct ChunkWriter<'a, E> {
    sink: &'a mut dyn FnMut(&PyStr) -> Result<(), E>,
    indent: usize,
}

impl<E> ChunkWriter<'_, E> {
    fn emit(&mut self, text: &PyStr) -> Result<(), StreamError<E>> {
        (self.sink)(text).map_err(StreamError::Sink)
    }

    fn emit_str(&mut self, text: &str) -> Result<(), StreamError<E>> {
        self.emit(&PyStr::new(text))
    }

    fn newline_indent(&self, level: usize) -> String {
        format!("\n{}", " ".repeat(self.indent * level))
    }

    /// The text of a scalar item, or `None` for a container (or a value `json` cannot encode, which fails after the chunk before it).
    fn scalar(value: &PyValue) -> Result<Option<PyStr>, StreamError<E>> {
        Ok(Some(match value.peel() {
            PyValue::Str(text) => {
                let mut out = PyStr::default();
                text.write_json(&mut out, false);
                out
            }
            PyValue::None => PyStr::new("null"),
            PyValue::Bool(true) => PyStr::new("true"),
            PyValue::Bool(false) => PyStr::new("false"),
            PyValue::Int(number) => {
                let digits = number.to_string();
                if digits.trim_start_matches('-').len() > MAX_INT_DIGITS {
                    return Err(StreamError::Dump(DumpError::IntegerTooLong));
                }
                PyStr::new(&digits)
            }
            PyValue::Float(number) => PyStr::new(&float_json(*number)),
            _ => return Ok(None),
        }))
    }

    /// A value that is not a scalar: a list or dict recursion, or the failure for a value of another type.
    fn container(&mut self, value: &PyValue, level: usize) -> Result<(), StreamError<E>> {
        match value.peel() {
            PyValue::List(items) | PyValue::Tuple(items) => self.list(items, level),
            PyValue::Dict(map) => self.dict(map, level),
            PyValue::Bytes(_) => Err(StreamError::Dump(DumpError::NotSerializable("bytes"))),
            PyValue::Opaque(opaque) if opaque.repr == "[...]" || opaque.repr == "{...}" => {
                Err(StreamError::Dump(DumpError::Circular))
            }
            PyValue::Opaque(opaque) => Err(StreamError::Dump(DumpError::NotSerializable(
                opaque
                    .type_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(opaque.type_name),
            ))),
            _ => unreachable!("scalars never reach here"),
        }
    }

    fn too_deep(level: usize) -> Result<(), StreamError<E>> {
        if level + 1 > PRETTY_MAX_DEPTH {
            return Err(StreamError::Dump(DumpError::TooDeep));
        }
        Ok(())
    }

    fn list(&mut self, items: &[PyValue], level: usize) -> Result<(), StreamError<E>> {
        Self::too_deep(level)?;
        if items.is_empty() {
            return self.emit_str("[]");
        }
        let inner = level + 1;
        let newline_indent = self.newline_indent(inner);
        let separator = format!(",{newline_indent}");
        let mut buffer = format!("[{newline_indent}");
        for (index, value) in items.iter().enumerate() {
            if index > 0 {
                buffer = separator.clone();
            }
            match Self::scalar(value)? {
                Some(text) => {
                    let mut chunk = PyStr::new(&buffer);
                    chunk.push_py(&text);
                    self.emit(&chunk)?;
                }
                None => {
                    self.emit_str(&buffer)?;
                    self.container(value, inner)?;
                }
            }
        }
        let closing = self.newline_indent(level);
        self.emit_str(&closing)?;
        self.emit_str("]")
    }

    fn dict(&mut self, map: &PyDict, level: usize) -> Result<(), StreamError<E>> {
        Self::too_deep(level)?;
        if map.is_empty() {
            return self.emit_str("{}");
        }
        self.emit_str("{")?;
        let inner = level + 1;
        let newline_indent = self.newline_indent(inner);
        let separator = format!(",{newline_indent}");
        self.emit_str(&newline_indent)?;
        for (index, (other, key, value)) in map.keyed_entries().enumerate() {
            let key_text = match other {
                Some(other) => json_key(other).map_err(StreamError::Dump)?,
                None => key.clone(),
            };
            if index > 0 {
                self.emit_str(&separator)?;
            }
            let mut quoted = PyStr::default();
            key_text.write_json(&mut quoted, false);
            self.emit(&quoted)?;
            self.emit_str(": ")?;
            match Self::scalar(value)? {
                Some(text) => self.emit(&text)?,
                None => self.container(value, inner)?,
            }
        }
        let closing = self.newline_indent(level);
        self.emit_str(&closing)?;
        self.emit_str("}")
    }
}

impl PyValue {
    /// `json.dump(self, fp, ensure_ascii=False, indent=indent)` for a positive `indent`: every chunk the pure-Python encoder yields,
    /// in order, is handed to `sink` (the file's `write`); the first sink failure or encoding failure ends the stream, the chunks
    /// before it having been handed over. The whole-text methods above are unchanged.
    pub fn json_dump_chunks<E>(
        &self,
        indent: usize,
        sink: &mut dyn FnMut(&PyStr) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let mut writer = ChunkWriter { sink, indent };
        match ChunkWriter::<E>::scalar(self)? {
            Some(text) => writer.emit(&text),
            None => writer.container(self, 0),
        }
    }
}

#[derive(Clone, Copy)]
enum Layout {
    Compact,
    /// `separators=(",", ":")`.
    Tight,
    /// `level` nesting depth; `indent` spaces per level.
    Pretty {
        level: usize,
        indent: usize,
    },
}

impl Layout {
    fn nested(self) -> Layout {
        match self {
            Layout::Compact => Layout::Compact,
            Layout::Tight => Layout::Tight,
            Layout::Pretty { level, indent } => Layout::Pretty {
                level: level + 1,
                indent,
            },
        }
    }
}

fn write_sequence(
    out: &mut PyStr,
    layout: Layout,
    (open, close): (char, char),
    count: usize,
    mut write_item: impl FnMut(&mut PyStr, usize, Layout) -> Result<(), DumpError>,
) -> Result<(), DumpError> {
    if matches!(layout, Layout::Pretty { level, .. } if level + 1 > PRETTY_MAX_DEPTH) {
        return Err(DumpError::TooDeep);
    }
    out.push_code_point(u32::from(open));
    if count == 0 {
        out.push_code_point(u32::from(close));
        return Ok(());
    }
    let inner = layout.nested();
    for index in 0..count {
        match (layout, index) {
            (Layout::Compact | Layout::Tight, 0) => {}
            (Layout::Compact, _) => out.push_str(", "),
            (Layout::Tight, _) => out.push_str(","),
            (Layout::Pretty { .. }, 0) => push_indent(out, inner),
            (Layout::Pretty { .. }, _) => {
                out.push_code_point(u32::from(','));
                push_indent(out, inner);
            }
        }
        write_item(out, index, inner)?;
    }
    if matches!(layout, Layout::Pretty { .. }) {
        push_indent(out, layout);
    }
    out.push_code_point(u32::from(close));
    Ok(())
}

fn push_indent(out: &mut PyStr, layout: Layout) {
    if let Layout::Pretty { level, indent } = layout {
        out.push_code_point(u32::from('\n'));
        for _ in 0..level * indent {
            out.push_code_point(u32::from(' '));
        }
    }
}

/// Digits and decimal exponent of a finite positive double in scientific form, as `format!("{:.*e}")` renders it.
fn scientific_digits(magnitude: f64, precision: usize) -> (String, i32) {
    let text = format!("{magnitude:.precision$e}");
    let (mantissa, exponent) = text.split_once('e').expect("{:e} always has an exponent");
    (
        mantissa.chars().filter(|c| c.is_ascii_digit()).collect(),
        exponent.parse().expect("exponent is an integer"),
    )
}

/// Python's `repr` takes, among the shortest digit strings that read back as the same double, the one closest to
/// the exact value and, when two are equally close (the exact value ends in a 5 right after them), the one with
/// the even last digit: `repr(2175058994972888.25)` is `2175058994972888.2`. Rust's shortest formatter breaks that
/// tie upward (`...8.3`). `digits` is Rust's shortest form; the Python choice is returned.
fn settle_exact_tie(magnitude: f64, digits: &str, exponent: i32) -> Option<(String, i32)> {
    if !tie_possible(magnitude) {
        return None;
    }
    Some(settle_exact_tie_unfiltered(
        magnitude,
        digits.to_string(),
        exponent,
    ))
}

/// A tie needs the exact value to be a decimal of at most 18 significant digits. With `m` the odd part of the mantissa
/// and `e` the matching binary exponent (value = m·2^e): for `e < -40` the digits of `m·5^-e` (odd, 28 or more digits,
/// no trailing zeros) are all significant; for `e > 120` the digits of `2^(e-22)` (at least 29 digits) are, after the at
/// most 22 trailing zeros `m` can contribute. Either way far more than 18, so no tie and nothing to settle.
fn tie_possible(magnitude: f64) -> bool {
    let bits = magnitude.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7FF) as i64;
    let fraction = bits & ((1u64 << 52) - 1);
    let (mantissa, exponent) = if exponent_bits == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), exponent_bits - 1075)
    };
    if mantissa == 0 {
        return false;
    }
    let odd_exponent = exponent + i64::from(mantissa.trailing_zeros());
    (-40..=120).contains(&odd_exponent)
}

fn settle_exact_tie_unfiltered(magnitude: f64, digits: String, exponent: i32) -> (String, i32) {
    let length = digits.len();
    let (wide, wide_exponent) = scientific_digits(magnitude, length);
    if wide.len() != length + 1 || !wide.ends_with('5') {
        return (digits, exponent);
    }
    // Only an expansion that stops right after that 5 is a tie; a double has at most 767 significant digits.
    let (exact, _) = scientific_digits(magnitude, 800);
    if exact[length + 1..].bytes().any(|digit| digit != b'0') {
        return (digits, exponent);
    }
    let lower = &wide[..length];
    let mut chosen = lower.to_string();
    let mut chosen_exponent = wide_exponent;
    if (lower.as_bytes()[length - 1] - b'0') % 2 == 1 {
        let mut bytes = lower.as_bytes().to_vec();
        let mut index = length;
        loop {
            if index == 0 {
                bytes.insert(0, b'1');
                bytes.truncate(length);
                chosen_exponent += 1;
                break;
            }
            index -= 1;
            if bytes[index] == b'9' {
                bytes[index] = b'0';
            } else {
                bytes[index] += 1;
                break;
            }
        }
        chosen = String::from_utf8(bytes).expect("ASCII digits");
    }
    let trimmed = chosen.trim_end_matches('0');
    let trimmed = if trimmed.is_empty() { "0" } else { trimmed };
    let reads_back = format!("{trimmed}e{}", chosen_exponent - (trimmed.len() as i32 - 1))
        .parse::<f64>()
        .is_ok_and(|value| value == magnitude);
    if reads_back {
        (trimmed.to_string(), chosen_exponent)
    } else {
        (digits, exponent)
    }
}

/// Python `repr(float)`: shortest round-trip digits, exponent form outside `1e-4 <= |x| < 1e16`.
pub fn float_repr(number: f64) -> String {
    use std::fmt::Write;
    if number.is_nan() {
        return "nan".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if number == 0.0 {
        return if number.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_string();
    }
    // Rust's shortest digits, written to the stack: at most 17 digits, a point and an exponent
    let mut scientific = StackText::default();
    write!(scientific, "{:e}", number.abs()).expect("fits: 17 digits, point, exponent");
    let (mantissa, exponent) = scientific
        .as_str()
        .split_once('e')
        .expect("{:e} always has an exponent");
    let mut exponent: i32 = exponent.parse().expect("exponent is an integer");
    let mut digits = StackText::default();
    for digit in mantissa.chars().filter(char::is_ascii_digit) {
        digits.push(digit);
    }
    let settled;
    let mut digits = digits.as_str();
    if let Some((text, adjusted)) = settle_exact_tie(number.abs(), digits, exponent) {
        settled = text;
        digits = &settled;
        exponent = adjusted;
    }
    let mut out = String::with_capacity(32);
    if number < 0.0 {
        out.push('-');
    }
    if !(-4..16).contains(&exponent) {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        let _ = write!(out, "{:02}", exponent.abs());
    } else if exponent >= 0 {
        let integer_len = exponent as usize + 1;
        if digits.len() <= integer_len {
            out.push_str(digits);
            out.extend(std::iter::repeat_n('0', integer_len - digits.len()));
            out.push_str(".0");
        } else {
            out.push_str(&digits[..integer_len]);
            out.push('.');
            out.push_str(&digits[integer_len..]);
        }
    } else {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
        out.push_str(digits);
    }
    out
}

/// A short ASCII text on the stack (at most 40 bytes), for formatting without an allocation.
struct StackText {
    bytes: [u8; 40],
    len: usize,
}

impl Default for StackText {
    fn default() -> Self {
        StackText {
            bytes: [0; 40],
            len: 0,
        }
    }
}

impl StackText {
    fn push(&mut self, c: char) {
        self.bytes[self.len] = c as u8;
        self.len += 1;
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("ASCII")
    }
}

impl std::fmt::Write for StackText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.len + text.len();
        if end > self.bytes.len() {
            return Err(std::fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn float_json(number: f64) -> String {
    if number.is_nan() {
        "NaN".to_string()
    } else if number.is_infinite() {
        if number > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        }
        .to_string()
    } else {
        float_repr(number)
    }
}

/// Python `repr(bytes)`.
pub fn bytes_repr(bytes: &[u8]) -> String {
    let quote = if bytes.contains(&b'\'') && !bytes.contains(&b'"') {
        '"'
    } else {
        '\''
    };
    let mut out = format!("b{quote}");
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b if char::from(b) == quote => {
                out.push('\\');
                out.push(quote);
            }
            0x20..=0x7E => out.push(char::from(byte)),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out.push(quote);
    out
}

/// What `json.dumps` writes for a dict key: strings as they are, `None`/`bool`/`int`/`float` as their JSON spelling,
/// anything else is the `TypeError` Python raises.
pub fn json_key(key: &NonStringKey) -> Result<PyStr, DumpError> {
    let repr = key.repr.as_str();
    let spelled = match repr {
        "None" => "null".to_string(),
        "True" => "true".to_string(),
        "False" => "false".to_string(),
        "inf" => "Infinity".to_string(),
        "-inf" => "-Infinity".to_string(),
        "nan" => "NaN".to_string(),
        other if other.parse::<f64>().is_ok() => other.to_string(),
        other if other.starts_with("datetime.datetime") => {
            return Err(DumpError::KeyNotAllowed("datetime"))
        }
        other if other.starts_with("datetime.date") => {
            return Err(DumpError::KeyNotAllowed("date"))
        }
        _ => return Err(DumpError::KeyNotAllowed("bytes")),
    };
    Ok(PyStr::new(&spelled))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_repr_reference(number: f64) -> String {
        if number.is_nan() {
            return "nan".to_string();
        }
        if number.is_infinite() {
            return if number > 0.0 { "inf" } else { "-inf" }.to_string();
        }
        if number == 0.0 {
            return if number.is_sign_negative() {
                "-0.0"
            } else {
                "0.0"
            }
            .to_string();
        }
        let scientific = format!("{:e}", number.abs());
        let (mantissa, exponent) = scientific
            .split_once('e')
            .expect("{:e} always has an exponent");
        let exponent: i32 = exponent.parse().expect("exponent is an integer");
        let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
        let (digits, exponent) = settle_exact_tie_unfiltered(number.abs(), digits, exponent);
        let sign = if number < 0.0 { "-" } else { "" };
        if !(-4..16).contains(&exponent) {
            let fraction = if digits.len() > 1 {
                format!(".{}", &digits[1..])
            } else {
                String::new()
            };
            let exponent_sign = if exponent < 0 { '-' } else { '+' };
            return format!(
                "{sign}{}{fraction}e{exponent_sign}{:02}",
                &digits[..1],
                exponent.abs()
            );
        }
        if exponent >= 0 {
            let integer_len = exponent as usize + 1;
            if digits.len() <= integer_len {
                format!("{sign}{digits}{}.0", "0".repeat(integer_len - digits.len()))
            } else {
                format!(
                    "{sign}{}.{}",
                    &digits[..integer_len],
                    &digits[integer_len..]
                )
            }
        } else {
            format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize))
        }
    }

    fn other(identity: &str, repr: &str) -> DictKey {
        DictKey::Other(NonStringKey {
            identity: identity.to_string(),
            repr: repr.to_string(),
            value: None,
        })
    }

    #[test]
    fn string_keys_compare_by_content_and_non_string_keys_by_identity() {
        let mut map = PyDict::new();
        map.insert(PyStr::new("a"), PyValue::int(1));
        map.insert(PyStr::new("a"), PyValue::int(2));
        map.insert(PyStr::new("b"), PyValue::int(3));
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("a").unwrap().repr(), "2");
        // equal Python keys (1, True) are one entry: first position and spelling, last value
        map.insert_key(other("num:1", "True"), PyValue::int(10));
        map.insert_key(other("num:1", "1"), PyValue::int(11));
        map.insert_key(other("num:2", "2"), PyValue::int(12));
        assert_eq!(map.len(), 4);
        assert_eq!(
            PyValue::Dict(map.clone()).repr(),
            "{'a': 2, 'b': 3, True: 11, 2: 12}"
        );
        assert_eq!(
            PyValue::Dict(map.clone())
                .dumps_pretty()
                .unwrap()
                .to_utf8()
                .unwrap()
                .replace(['\n', ' '], ""),
            "{\"a\":2,\"b\":3,\"true\":11,\"2\":12}"
        );
    }

    #[test]
    fn a_string_that_looks_like_a_marker_is_only_a_string() {
        // the spelling the old private encoding used: it must neither collide with a real key nor be taken for one
        let lookalike = "\u{0}yaml-key:num:1\u{0}True";
        let mut map = PyDict::new();
        map.insert(PyStr::new(lookalike), PyValue::int(5));
        assert_eq!(map.get(lookalike).unwrap().repr(), "5");
        assert_eq!(
            PyValue::Dict(map.clone()).repr(),
            "{'\\x00yaml-key:num:1\\x00True': 5}"
        );
        assert_eq!(
            PyValue::Dict(map.clone())
                .dumps_pretty()
                .unwrap()
                .to_utf8()
                .unwrap()
                .replace(['\n', ' '], ""),
            "{\"\\u0000yaml-key:num:1\\u0000True\":5}"
        );
        // two different strings that share the inner part stay two keys, and a real True key joins them as a third
        map.insert(PyStr::new("\u{0}yaml-key:num:1\u{0}1"), PyValue::int(6));
        map.insert_key(other("num:1", "True"), PyValue::int(7));
        map.insert_key(other("num:1", "1"), PyValue::int(8));
        assert_eq!(map.len(), 3);
        // a non-string key is not reachable by name, not even by its own repr
        assert!(map.get("True").is_none() && map.get("1").is_none());
        // a string equal to a non-string key's repr stays separate from it
        map.insert(PyStr::new("True"), PyValue::int(9));
        assert_eq!(map.len(), 4);
        assert_eq!(map.get("True").unwrap().repr(), "9");
        // a lone surrogate inside such text is still just text
        let mut broken = PyStr::new(lookalike);
        broken.push_code_point(0xD800);
        map.insert(broken.clone(), PyValue::int(1));
        map.insert(broken, PyValue::int(2));
        assert_eq!(map.len(), 5);
    }

    #[test]
    fn float_repr_equals_the_allocating_reference() {
        let mut state = 0x1234_5678_9ABC_DEF1u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut numbers = vec![
            0.1,
            1.5,
            100.0,
            1e16,
            1e15,
            1e-4,
            1e-5,
            123456789012345680.0,
            5e-324,
            f64::MAX,
            f64::MIN_POSITIVE,
            8700235979891553.0 / 4.0,
            2.675,
            0.30000000000000004,
            -0.0,
            0.0,
            1e22,
            1e23,
            273.0,
            12.6,
        ];
        for _ in 0..40000 {
            let value = f64::from_bits(next());
            if value.is_finite() {
                numbers.push(value);
                numbers.push(-value);
            }
            numbers.push((next() % 100_000) as f64 / 100.0);
            numbers.push((next() % 1_000_000) as f64 * 2f64.powi((next() % 120) as i32 - 60));
        }
        for number in numbers {
            assert_eq!(
                float_repr(number),
                float_repr_reference(number),
                "{number:e}"
            );
        }
    }

    #[test]
    fn skipping_impossible_ties_changes_nothing() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut numbers = vec![
            8700235979891553.0 / 4.0,
            0.5,
            0.25,
            1e22,
            1e23,
            5e-324,
            2f64.powi(80),
            2f64.powi(-60),
            273.0,
            12.6,
        ];
        for _ in 0..30000 {
            let value = f64::from_bits(next());
            if value.is_finite() {
                numbers.push(value.abs());
            }
            // dyadic values with few significant bits: where ties live
            let bits = (next() % (1 << 20)) as f64;
            numbers.push(bits * 2f64.powi((next() % 90) as i32 - 50));
            numbers.push((next() >> 11) as f64 * 2f64.powi(-((next() % 12) as i32)));
        }
        for number in numbers {
            if number == 0.0 || !number.is_finite() {
                continue;
            }
            let scientific = format!("{:e}", number);
            let (mantissa, exp) = scientific.split_once('e').unwrap();
            let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
            let exp: i32 = exp.parse().unwrap();
            let full = settle_exact_tie_unfiltered(number, digits.clone(), exp);
            assert_eq!(
                settle_exact_tie(number, &digits, exp).unwrap_or((digits, exp)),
                full,
                "{number:e}"
            );
        }
    }

    #[test]
    fn float_repr_matches_python() {
        let cases = [
            (1.0, "1.0"),
            (0.1, "0.1"),
            (1.5, "1.5"),
            (100.0, "100.0"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (123456789012345680.0, "1.2345678901234568e+17"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (-2.5, "-2.5"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            // Two shortest strings read back as this double and are equally close; Python takes the even one.
            (2_175_058_994_972_888.0 + 0.25, "2175058994972888.2"),
            (2_175_058_994_972_888.0 + 0.75, "2175058994972888.8"),
            (9007199254740993.0, "9007199254740992.0"),
        ];
        for (number, expected) in cases {
            assert_eq!(float_repr(number), expected, "{number:e}");
        }
        assert_eq!(float_repr(f64::INFINITY), "inf");
        assert_eq!(float_json(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(float_json(f64::NAN), "NaN");
    }

    #[test]
    fn equality_follows_python_numeric_rules() {
        let three = BigInt::from(3);
        assert!(PyValue::Float(3.0).equals_int(&three));
        assert!(!PyValue::Float(3.5).equals_int(&three));
        assert!(!PyValue::Bool(true).equals_int(&three));
        assert!(PyValue::Bool(true).equals_int(&BigInt::from(1)));
        assert!(!PyValue::str("3").equals_int(&three));
        assert!(!PyValue::None.equals_int(&three));
        assert!(PyValue::Float(2f64.powi(80)).equals_int(&(BigInt::from(1) << 80)));
    }

    #[test]
    fn dumps_match_python_layouts() {
        let mut inner = PyDict::new();
        inner.set("k", PyValue::List(vec![]));
        inner.set("n", PyValue::int(2));
        let value = PyValue::List(vec![PyValue::Dict(inner), PyValue::Dict(PyDict::new())]);
        assert_eq!(
            value.dumps_pretty().unwrap().to_utf8().unwrap(),
            "[\n  {\n    \"k\": [],\n    \"n\": 2\n  },\n  {}\n]"
        );
        assert_eq!(value.dumps_ascii().unwrap(), "[{\"k\": [], \"n\": 2}, {}]");
    }

    fn json_object(pairs: &[(String, usize)]) -> String {
        let body: Vec<String> = pairs
            .iter()
            .map(|(key, number)| format!("\"{key}\": {number}"))
            .collect();
        format!("{{{}}}", body.join(", "))
    }

    fn dict_of(value: PyValue) -> PyDict {
        match value {
            PyValue::Dict(dict) => dict,
            _ => panic!("expected a dict"),
        }
    }

    /// Takes the owned entries, or fails the test when the dict was refused (the opaque iterator has no `Debug`).
    fn owned_entries(dict: PyDict) -> Vec<(PyStr, PyValue)> {
        match dict.try_into_string_entries() {
            Ok(entries) => entries.collect(),
            Err(_) => panic!("a string-keyed dict must hand over its entries"),
        }
    }

    fn other_key(identity: &str, repr: &str, value: Option<PyValue>) -> DictKey {
        DictKey::Other(NonStringKey {
            identity: identity.to_string(),
            repr: repr.to_string(),
            value: value.map(Box::new),
        })
    }

    #[test]
    fn string_entries_move_out_in_order_for_json_built_dicts() {
        for size in [0usize, 1, 16, 17, 64, 2048] {
            let pairs: Vec<(String, usize)> = (0..size).map(|n| (format!("k{n}"), n)).collect();
            let text = json_object(&pairs);
            let dict = dict_of(crate::pyjson::loads(&text).unwrap());
            let snapshot: Vec<(PyStr, PyValue)> = dict.iter().cloned().collect();
            let entries = match dict.try_into_string_entries() {
                Ok(entries) => entries,
                Err(_) => panic!("size {size}: a JSON object has only string keys"),
            };
            assert_eq!(entries.len(), size);
            let taken: Vec<(PyStr, PyValue)> = entries.collect();
            assert_eq!(taken, snapshot, "size {size}");
            for (at, (key, value)) in taken.iter().enumerate() {
                assert_eq!(*key, PyStr::new(&pairs[at].0));
                assert_eq!(*value, PyValue::int(pairs[at].1));
            }
        }
    }

    #[test]
    fn string_entries_keep_escaped_empty_long_and_lone_surrogate_keys() {
        let long = "x".repeat(5000);
        let text = format!(
            "{{\"a\\n\\t\\\"q\\u00e9\": 1, \"\": 2, \"\\ud800\": 3, \"{long}\": 4, \"\\ud83d\\ude00\": 5}}"
        );
        let dict = dict_of(crate::pyjson::loads(&text).unwrap());
        let snapshot: Vec<(PyStr, PyValue)> = dict.iter().cloned().collect();
        let taken = owned_entries(dict);
        assert_eq!(taken, snapshot);
        let keys: Vec<Vec<u32>> = taken
            .iter()
            .map(|(key, _)| key.code_points().to_vec())
            .collect();
        assert_eq!(
            keys,
            vec![
                vec![0x61, 0x0a, 0x09, 0x22, 0x71, 0xe9],
                vec![],
                vec![0xd800],
                vec![0x78; 5000],
                vec![0x1f600],
            ]
        );
    }

    #[test]
    fn string_entries_of_duplicates_keep_first_position_and_last_value() {
        let from_json = dict_of(crate::pyjson::loads("{\"a\": 1, \"b\": 2, \"a\": 3}").unwrap());
        let taken = owned_entries(from_json);
        assert_eq!(
            taken,
            vec![
                (PyStr::new("a"), PyValue::int(3)),
                (PyStr::new("b"), PyValue::int(2))
            ]
        );
        let mut built = PyDict::new();
        built.set("b", PyValue::int(1));
        built.set("a", PyValue::int(2));
        built.set("b", PyValue::int(9));
        assert_eq!(
            owned_entries(built),
            vec![
                (PyStr::new("b"), PyValue::int(9)),
                (PyStr::new("a"), PyValue::int(2))
            ]
        );
    }

    #[test]
    fn string_entries_of_an_empty_dict_is_an_empty_iterator() {
        match PyDict::new().try_into_string_entries() {
            Ok(entries) => {
                assert_eq!(entries.len(), 0);
                assert_eq!(entries.count(), 0);
            }
            Err(_) => panic!("an empty dict has only string keys"),
        }
    }

    #[test]
    fn string_entries_move_the_key_buffers_without_cloning() {
        let long = "k".repeat(300);
        let mut dict = PyDict::new();
        dict.set(&long, PyValue::List(vec![PyValue::int(1)]));
        dict.set("short", PyValue::None);
        let before: Vec<*const u32> = dict
            .iter()
            .map(|(key, _)| key.code_points().as_ptr())
            .collect();
        let taken = owned_entries(dict);
        let after: Vec<*const u32> = taken
            .iter()
            .map(|(key, _)| key.code_points().as_ptr())
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn string_entries_values_not_taken_drop_normally() {
        let shared = std::sync::Arc::new(PyValue::int(7));
        let mut dict = PyDict::new();
        dict.set("one", PyValue::Shared(shared.clone()));
        dict.set("two", PyValue::Shared(shared.clone()));
        assert_eq!(std::sync::Arc::strong_count(&shared), 3);
        match dict.try_into_string_entries() {
            Ok(mut entries) => {
                let first = entries.next().unwrap();
                assert_eq!(first.0, PyStr::new("one"));
                assert_eq!(std::sync::Arc::strong_count(&shared), 3);
                drop(first);
                assert_eq!(std::sync::Arc::strong_count(&shared), 2);
            }
            Err(_) => panic!("a string-keyed dict must hand over its entries"),
        }
        assert_eq!(std::sync::Arc::strong_count(&shared), 1);
    }

    #[test]
    fn string_entries_keep_nested_values_whole() {
        let mut inner = PyDict::new();
        inner.set("n", PyValue::Float(f64::NAN));
        let value = PyValue::List(vec![
            PyValue::Dict(inner),
            PyValue::str("s"),
            PyValue::Bool(true),
        ]);
        let mut dict = PyDict::new();
        dict.set("v", value.clone());
        let taken = owned_entries(dict);
        assert_eq!(taken.len(), 1);
        match (&taken[0].1, &value) {
            (PyValue::List(got), PyValue::List(want)) => {
                assert_eq!(got.len(), want.len());
                assert_eq!(got[1], want[1]);
                assert_eq!(got[2], want[2]);
                match &got[0] {
                    PyValue::Dict(inner) => assert!(matches!(
                        inner.get("n"),
                        Some(PyValue::Float(number)) if number.is_nan()
                    )),
                    _ => panic!("expected the nested dict"),
                }
            }
            _ => panic!("expected a list"),
        }
    }

    #[test]
    fn dict_with_a_non_string_key_is_refused_unchanged() {
        let mut dict = PyDict::new();
        dict.set("a", PyValue::int(1));
        dict.insert_key(
            other_key("num:1", "1", Some(PyValue::int(1))),
            PyValue::str("one"),
        );
        dict.insert_key(other_key("none", "None", None), PyValue::Bool(false));
        dict.set("1", PyValue::int(5));
        let original = dict.clone();
        let refused = match dict.try_into_string_entries() {
            Ok(_) => panic!("a non-string key must refuse"),
            Err(back) => back,
        };
        assert_eq!(refused, original);
        let kept: Vec<_> = refused.keyed_entries().collect();
        let wanted: Vec<_> = original.keyed_entries().collect();
        assert_eq!(kept.len(), 4);
        assert_eq!(kept.len(), wanted.len());
        for (got, want) in kept.iter().zip(wanted.iter()) {
            assert_eq!(got.1, want.1);
            assert_eq!(got.2, want.2);
            match (got.0, want.0) {
                (None, None) => {}
                (Some(g), Some(w)) => {
                    assert_eq!(g.identity, w.identity);
                    assert_eq!(g.repr, w.repr);
                    assert_eq!(g.value, w.value);
                }
                _ => panic!("key kind changed"),
            }
        }
        // The kept metadata: a number with its value, a `None` without one, and the string "1" a separate plain entry.
        assert_eq!(kept[1].0.unwrap().value.as_deref(), Some(&PyValue::int(1)));
        assert!(kept[2].0.unwrap().value.is_none());
        assert!(kept[3].0.is_none());
        assert_eq!(*kept[3].1, PyStr::new("1"));
    }

    #[test]
    fn dict_of_only_non_string_keys_is_refused() {
        let mut dict = PyDict::new();
        dict.insert_key(
            other_key("num:1", "True", Some(PyValue::Bool(true))),
            PyValue::int(1),
        );
        dict.insert_key(
            other_key("num:1", "1", Some(PyValue::int(1))),
            PyValue::int(2),
        );
        // `True` and `1` are one Python key: first key metadata, last value.
        assert_eq!(dict.len(), 1);
        let original = dict.clone();
        match dict.try_into_string_entries() {
            Ok(_) => panic!("a non-string key must refuse"),
            Err(back) => {
                assert_eq!(back, original);
                let entry = back.keyed_entries().next().unwrap();
                assert_eq!(entry.0.unwrap().repr, "True");
                assert_eq!(
                    entry.0.unwrap().value.as_deref(),
                    Some(&PyValue::Bool(true))
                );
                assert_eq!(*entry.2, PyValue::int(2));
            }
        }
    }
}
