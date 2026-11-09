//! The typed graph wire of the in-process `manifest_contract` bridge: the SAME encoding the original-capture observer writes for a Python
//! value (`{"t":"str","utf8_surrogatepass_hex":..}`, `{"t":"int","sign":..,"hex":..}`, containers with a traversal-order `id`, `{"t":"ref","id":n}`
//! for sharing and cycles, `{"t":"float","repr":..,"nan_id":n}` for NaN objects, ...), decoded into and encoded from the shared value model.
//!
//! A `ref` to a container that is still open is a CYCLE: it decodes to the shared model's cycle marker (`Opaque.cycle_up`: how many container
//! levels above the marker the target sits) and it encodes back to the `ref` of that ancestor. A `ref` to a finished container is the same
//! shared object. Nothing here normalises: lone surrogates, integer sizes, NaN identity and container order are carried as they are.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{nan_object, DictKey, Opaque, PyDict, PyValue, Typed};
use agent_runtime_support::pyyaml::{dict_key, safe_load_text};
use num_bigint::{BigInt, Sign};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub fn tagged(tag: &str) -> PyDict {
    let mut dict = PyDict::new();
    dict.set("t", PyValue::str(tag));
    dict
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return Err("hex text must be ASCII of even length".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// UTF-8 that also accepts the three-byte forms of lone surrogates (`utf-8` with `surrogatepass`).
pub fn wtf8_decode(bytes: &[u8]) -> Result<Vec<u32>, String> {
    let mut points = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let lead = bytes[at];
        let (need, init) = match lead {
            0x00..=0x7F => (0, u32::from(lead)),
            0xC2..=0xDF => (1, u32::from(lead & 0x1F)),
            0xE0..=0xEF => (2, u32::from(lead & 0x0F)),
            0xF0..=0xF4 => (3, u32::from(lead & 0x07)),
            _ => return Err(format!("invalid UTF-8 lead byte at {at}")),
        };
        let mut point = init;
        for step in 1..=need {
            let byte = *bytes.get(at + step).ok_or("truncated UTF-8 sequence")?;
            if byte & 0xC0 != 0x80 {
                return Err("invalid UTF-8 continuation".into());
            }
            point = (point << 6) | u32::from(byte & 0x3F);
        }
        points.push(point);
        at += need + 1;
    }
    Ok(points)
}

pub fn wtf8_encode(points: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    for &point in points {
        match point {
            0..=0x7F => out.push(point as u8),
            0x80..=0x7FF => {
                out.push(0xC0 | (point >> 6) as u8);
                out.push(0x80 | (point & 0x3F) as u8);
            }
            0x800..=0xFFFF => {
                out.push(0xE0 | (point >> 12) as u8);
                out.push(0x80 | ((point >> 6) & 0x3F) as u8);
                out.push(0x80 | (point & 0x3F) as u8);
            }
            _ => {
                out.push(0xF0 | (point >> 18) as u8);
                out.push(0x80 | ((point >> 12) & 0x3F) as u8);
                out.push(0x80 | ((point >> 6) & 0x3F) as u8);
                out.push(0x80 | (point & 0x3F) as u8);
            }
        }
    }
    out
}

pub fn field<'a>(value: &'a PyValue, name: &str) -> Result<&'a PyValue, String> {
    match value.peel() {
        PyValue::Dict(map) => map
            .get(name)
            .ok_or_else(|| format!("missing field {name:?}")),
        _ => Err(format!("an object with {name:?} was expected")),
    }
}

pub fn text_of(value: &PyValue) -> Result<String, String> {
    match value.peel() {
        PyValue::Str(text) => text
            .to_utf8()
            .ok_or_else(|| "text with a lone surrogate".to_string()),
        _ => Err("text expected".into()),
    }
}

pub fn int_of(value: &PyValue) -> Result<i64, String> {
    match value.peel() {
        PyValue::Int(number) => number
            .to_string()
            .parse()
            .map_err(|_| "integer out of range".to_string()),
        _ => Err("integer expected".into()),
    }
}

pub fn items_of(value: &PyValue) -> Result<&[PyValue], String> {
    match value.peel() {
        PyValue::List(items) => Ok(items),
        _ => Err("list expected".into()),
    }
}

fn tag_of(wire: &PyValue) -> Result<String, String> {
    text_of(field(wire, "t")?)
}

/// Every `ref` id of a wire document.
pub fn scan_refs(wire: &PyValue, out: &mut HashSet<i64>) {
    match wire.peel() {
        PyValue::Dict(map) => {
            if matches!(map.get("t").map(PyValue::peel), Some(PyValue::Str(tag)) if tag.equals("ref"))
            {
                if let Some(Ok(id)) = map.get("id").map(int_of) {
                    out.insert(id);
                }
            }
            for (_, value) in map.iter() {
                scan_refs(value, out);
            }
        }
        PyValue::List(items) => items.iter().for_each(|item| scan_refs(item, out)),
        _ => {}
    }
}

fn nan_origin(key: &PyValue, index: usize) -> usize {
    match key {
        PyValue::Shared(object) => Arc::as_ptr(object) as *const () as usize,
        _ => usize::MAX - index,
    }
}

/// Decoder state for the arguments of ONE call (ids are shared across its arguments).
pub struct Decoder {
    refs: HashSet<i64>,
    done: HashMap<i64, PyValue>,
    nans: HashMap<i64, PyValue>,
    open: Vec<(i64, &'static str)>,
}

impl Decoder {
    pub fn new(documents: &[&PyValue]) -> Self {
        let mut refs = HashSet::new();
        documents.iter().for_each(|doc| scan_refs(doc, &mut refs));
        Decoder {
            refs,
            done: HashMap::new(),
            nans: HashMap::new(),
            open: Vec::new(),
        }
    }

    fn finish(&mut self, id: i64, value: PyValue) -> PyValue {
        if self.refs.contains(&id) {
            let shared = PyValue::Shared(Arc::new(value));
            self.done.insert(id, shared.clone());
            shared
        } else {
            value
        }
    }

    pub fn decode(&mut self, wire: &PyValue) -> Result<PyValue, String> {
        let tag = tag_of(wire)?;
        Ok(match tag.as_str() {
            "none" => PyValue::None,
            "bool" => match field(wire, "v")?.peel() {
                PyValue::Bool(flag) => PyValue::Bool(*flag),
                _ => return Err("bool expected".into()),
            },
            "int" => {
                let magnitude = BigInt::parse_bytes(text_of(field(wire, "hex")?)?.as_bytes(), 16)
                    .ok_or("hex integer expected")?;
                let negative = text_of(field(wire, "sign")?)? == "-";
                PyValue::Int(if negative { -magnitude } else { magnitude })
            }
            "float" => {
                let repr = text_of(field(wire, "repr")?)?;
                let number: f64 = repr.parse().map_err(|_| format!("float repr {repr:?}"))?;
                match wire.peel() {
                    PyValue::Dict(map) if number.is_nan() => match map.get("nan_id") {
                        Some(id) => {
                            let id = int_of(id)?;
                            self.nans.entry(id).or_insert_with(nan_object).clone()
                        }
                        None => PyValue::Float(number),
                    },
                    _ => PyValue::Float(number),
                }
            }
            "str" => {
                let bytes = hex_decode(&text_of(field(wire, "utf8_surrogatepass_hex")?)?)?;
                PyValue::Str(PyStr::from_code_points(wtf8_decode(&bytes)?))
            }
            "bytes" => PyValue::Bytes(hex_decode(&text_of(field(wire, "hex")?)?)?),
            "list" | "tuple" => {
                let id = int_of(field(wire, "id")?)?;
                self.open.push((id, "list"));
                let mut out = Vec::new();
                for item in items_of(field(wire, "v")?)? {
                    out.push(self.decode(item)?);
                }
                self.open.pop();
                let value = if tag == "list" {
                    PyValue::List(out)
                } else {
                    PyValue::Tuple(out)
                };
                self.finish(id, value)
            }
            "dict" => {
                let id = int_of(field(wire, "id")?)?;
                self.open.push((id, "dict"));
                let mut map = PyDict::new();
                for (index, pair) in items_of(field(wire, "pairs")?)?.iter().enumerate() {
                    let pair = items_of(pair)?;
                    if pair.len() != 2 {
                        return Err("a pair has two elements".into());
                    }
                    let key = self.decode(&pair[0])?;
                    let value = self.decode(&pair[1])?;
                    let key = match key.peel() {
                        PyValue::Str(text) => DictKey::Str(text.clone()),
                        _ => dict_key(&key, nan_origin(&key, index)),
                    };
                    map.insert_key(key, value);
                }
                self.open.pop();
                self.finish(id, PyValue::Dict(map))
            }
            "set" | "frozenset" => {
                let id = int_of(field(wire, "id")?)?;
                self.open.push((id, "set"));
                let mut members = Vec::new();
                for item in items_of(field(wire, "members")?)? {
                    members.push(self.decode(item)?);
                }
                self.open.pop();
                let reprs: Vec<String> = members.iter().map(PyValue::repr).collect();
                let value = PyValue::Opaque(Opaque {
                    type_name: "set",
                    repr: if reprs.is_empty() {
                        "set()".to_string()
                    } else {
                        format!("{{{}}}", reprs.join(", "))
                    },
                    truthy: !members.is_empty(),
                    typed: Typed::Set(members),
                    identity: None,
                });
                self.finish(id, value)
            }
            "date" | "datetime" => {
                let iso = text_of(field(wire, "iso")?)?;
                let loaded = safe_load_text(&PyStr::new(&iso), 0).map_err(|error| {
                    format!(
                        "timestamp {iso:?}: {}",
                        error.message.to_utf8().unwrap_or_default()
                    )
                })?;
                match loaded.peel() {
                    PyValue::Opaque(opaque)
                        if matches!(
                            (&opaque.typed, tag.as_str()),
                            (Typed::Date(_), "date") | (Typed::DateTime(_), "datetime")
                        ) =>
                    {
                        loaded.clone()
                    }
                    _ => return Err(format!("{iso:?} is not a {tag}")),
                }
            }
            "ref" => {
                let id = int_of(field(wire, "id")?)?;
                if let Some(at) = self.open.iter().position(|(open, _)| *open == id) {
                    let kind = self.open[at].1;
                    let (type_name, repr) = match kind {
                        "dict" => ("dict", "{...}"),
                        "set" => ("set", "set(...)"),
                        _ => ("list", "[...]"),
                    };
                    PyValue::Opaque(Opaque {
                        type_name,
                        repr: repr.to_string(),
                        truthy: true,
                        typed: Typed::Cycle(self.open.len() - at),
                        identity: None,
                    })
                } else {
                    self.done
                        .get(&id)
                        .cloned()
                        .ok_or_else(|| format!("ref {id} points to nothing built yet"))?
                }
            }
            other => {
                return Err(format!(
                    "wire tag {other:?} is outside the supported typed set"
                ))
            }
        })
    }
}

/// Encoder state for ONE answer.
#[derive(Default)]
pub struct Encoder {
    next: i64,
    shared: HashMap<usize, i64>,
    nans: HashMap<usize, i64>,
    open: Vec<i64>,
}

fn str_wire(text: &PyStr) -> PyValue {
    let mut out = tagged("str");
    out.set(
        "utf8_surrogatepass_hex",
        PyValue::str(&hex_encode(&wtf8_encode(text.code_points()))),
    );
    out.set("codepoints", PyValue::int(text.len() as i64));
    PyValue::Dict(out)
}

pub fn str_value(text: &str) -> PyValue {
    str_wire(&PyStr::new(text))
}

impl Encoder {
    pub fn new() -> Self {
        Encoder::default()
    }

    fn fresh(&mut self) -> i64 {
        self.next += 1;
        self.next
    }

    fn nan_id(&mut self, key: usize) -> i64 {
        let next = self.nans.len() as i64 + 1;
        *self.nans.entry(key).or_insert(next)
    }

    pub fn encode(&mut self, value: &PyValue) -> Result<PyValue, String> {
        self.encode_with(value, None)
    }

    fn encode_with(&mut self, value: &PyValue, forced: Option<i64>) -> Result<PyValue, String> {
        if let PyValue::Shared(object) = value {
            let key = Arc::as_ptr(object) as *const () as usize;
            if let PyValue::Float(number) = object.as_ref() {
                if number.is_nan() {
                    let mut out = tagged("float");
                    out.set("repr", PyValue::str("nan"));
                    out.set("nan_id", PyValue::int(self.nan_id(key)));
                    return Ok(PyValue::Dict(out));
                }
            }
            if let Some(id) = self.shared.get(&key) {
                let mut out = tagged("ref");
                out.set("id", PyValue::int(*id));
                return Ok(PyValue::Dict(out));
            }
            let id = self.fresh();
            self.shared.insert(key, id);
            return self.encode_with(object.as_ref(), Some(id));
        }
        let mut out;
        match value {
            PyValue::None => out = tagged("none"),
            PyValue::Bool(flag) => {
                out = tagged("bool");
                out.set("v", PyValue::Bool(*flag));
            }
            PyValue::Int(number) => {
                out = tagged("int");
                out.set(
                    "sign",
                    PyValue::str(if number.sign() == Sign::Minus {
                        "-"
                    } else {
                        "+"
                    }),
                );
                out.set("hex", PyValue::str(&number.magnitude().to_str_radix(16)));
            }
            PyValue::Float(number) => {
                out = tagged("float");
                out.set("repr", PyValue::str(&value.repr()));
                if number.is_nan() {
                    // a bare NaN is an object nobody else refers to
                    out.set(
                        "nan_id",
                        PyValue::int(self.nan_id(value as *const PyValue as usize)),
                    );
                }
            }
            PyValue::Str(text) => return Ok(str_wire(text)),
            PyValue::Bytes(bytes) => {
                out = tagged("bytes");
                out.set("hex", PyValue::str(&hex_encode(bytes)));
            }
            PyValue::List(items) | PyValue::Tuple(items) => {
                let id = forced.unwrap_or_else(|| self.fresh());
                out = tagged(if matches!(value, PyValue::List(_)) {
                    "list"
                } else {
                    "tuple"
                });
                out.set("id", PyValue::int(id));
                self.open.push(id);
                let mut wire = Vec::new();
                for item in items {
                    wire.push(self.encode(item)?);
                }
                self.open.pop();
                out.set("v", PyValue::List(wire));
            }
            PyValue::Dict(map) => {
                let id = forced.unwrap_or_else(|| self.fresh());
                out = tagged("dict");
                out.set("id", PyValue::int(id));
                self.open.push(id);
                let mut pairs = Vec::new();
                for (other, key, item) in map.keyed_entries() {
                    let key_wire = match other {
                        None => str_wire(key),
                        Some(non_string) => match non_string.value.as_deref() {
                            Some(inner) => self.encode(inner)?,
                            None => {
                                return Err(
                                    "a dictionary key without its value cannot be carried".into()
                                )
                            }
                        },
                    };
                    pairs.push(PyValue::List(vec![key_wire, self.encode(item)?]));
                }
                self.open.pop();
                out.set("pairs", PyValue::List(pairs));
            }
            PyValue::Opaque(opaque) => match &opaque.typed {
                Typed::Cycle(up) => {
                    let target = self
                        .open
                        .len()
                        .checked_sub(*up)
                        .and_then(|at| self.open.get(at))
                        .ok_or("a cycle marker points above the document")?;
                    out = tagged("ref");
                    out.set("id", PyValue::int(*target));
                }
                Typed::Date(iso) => {
                    out = tagged("date");
                    out.set("iso", PyValue::str(iso));
                }
                Typed::DateTime(iso) => {
                    out = tagged("datetime");
                    out.set("iso", PyValue::str(iso));
                }
                Typed::Set(members) => {
                    let id = forced.unwrap_or_else(|| self.fresh());
                    out = tagged("set");
                    out.set("id", PyValue::int(id));
                    self.open.push(id);
                    let mut wire = Vec::new();
                    for member in members {
                        wire.push(self.encode(member)?);
                    }
                    self.open.pop();
                    out.set("members", PyValue::List(wire));
                }
                _ => {
                    return Err(format!(
                        "a value of type {} cannot be carried",
                        opaque.type_name
                    ))
                }
            },
            PyValue::Shared(_) => unreachable!("handled above"),
        }
        Ok(PyValue::Dict(out))
    }
}
