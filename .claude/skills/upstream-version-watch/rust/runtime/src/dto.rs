//! The typed value DTO: the collision-free tag arrays of the declared profile (`r33_codec.py`). Scalar types, bytes, dates, tuples, dict
//! order and set membership stay observable; an alias is its value; a value that contains itself is outside the profile.

use agent_runtime_support::pyvalue::{float_repr, PyDict, PyValue, Typed};

fn tag(name: &str, data: PyValue) -> PyValue {
    PyValue::List(vec![PyValue::str(name), data])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The refusal of a value outside the profile (a program's refusal, never a business value).
pub fn refusal(message: &str) -> String {
    message.to_string()
}

/// `encode(value)`.
pub fn encode(value: &PyValue) -> Result<PyValue, String> {
    Ok(match value.peel() {
        PyValue::None => PyValue::List(vec![PyValue::str("none")]),
        PyValue::Bool(flag) => tag("bool", PyValue::Bool(*flag)),
        PyValue::Int(number) => tag("int", PyValue::str(&number.to_string())),
        PyValue::Float(number) => tag("float", PyValue::str(&float_repr(*number))),
        PyValue::Str(text) => tag("str", PyValue::Str(text.clone())),
        PyValue::Bytes(bytes) => tag("bytes", PyValue::str(&hex(bytes))),
        PyValue::List(items) => tag("list", members(items)?),
        PyValue::Tuple(items) => tag("tuple", members(items)?),
        PyValue::Dict(map) => {
            let mut pairs = Vec::new();
            for (other, key, item) in map.keyed_entries() {
                let key = match other {
                    None => tag("str", PyValue::Str(key.clone())),
                    Some(other) => match &other.value {
                        Some(typed) => encode(typed)?,
                        None => {
                            return Err(refusal(
                                "a dict key without its typed value is outside the DTO profile",
                            ))
                        }
                    },
                };
                pairs.push(PyValue::List(vec![key, encode(item)?]));
            }
            tag("dict", PyValue::List(pairs))
        }
        PyValue::Opaque(opaque) => match &opaque.typed {
            Typed::Date(iso) => tag("date", PyValue::str(iso)),
            Typed::DateTime(iso) => tag("datetime", PyValue::str(iso)),
            Typed::Set(items) => {
                let mut encoded = Vec::new();
                for item in items {
                    let one = encode(item)?;
                    let key = one
                        .dumps_ascii_tight()
                        .map_err(|error| refusal(&error.to_string()))?;
                    encoded.push((key, one));
                }
                encoded.sort_by(|a, b| a.0.cmp(&b.0));
                tag(
                    "set",
                    PyValue::List(encoded.into_iter().map(|(_, one)| one).collect()),
                )
            }
            Typed::Plain => {
                return Err(refusal(
                    if opaque.repr == "[...]" || opaque.repr == "{...}" || opaque.repr == "set(...)"
                    {
                        "cyclic value outside R33 DTO profile"
                    } else {
                        "undeclared DTO type"
                    },
                ))
            }
        },
        PyValue::Shared(_) => unreachable!("peel removes the shared carrier"),
    })
}

/// `encode(&value)` for a value the caller OWNS and no longer needs (a call's fresh return value): the same tags and the same refusals, but a fresh
/// `str`, `list` or `tuple` is **moved** into its tag instead of copied, recursively. Every other value (dict, shared carrier, opaque value, set,
/// bytes, number, bool, `None`, date) goes through the borrowed [`encode`] unchanged, so its type, order, text, float repr, alias-as-value and
/// refusal wording are exactly the borrowed encoder's. An `Err` is the first refusal in member order, as in [`encode`]; the value is consumed
/// either way. Nothing is cached and no value is inspected beyond its type.
pub fn encode_owned(value: PyValue) -> Result<PyValue, String> {
    match value {
        PyValue::Str(text) => Ok(tag("str", PyValue::Str(text))),
        PyValue::List(items) => Ok(tag("list", members_owned(items)?)),
        PyValue::Tuple(items) => Ok(tag("tuple", members_owned(items)?)),
        other => encode(&other),
    }
}

fn members_owned(items: Vec<PyValue>) -> Result<PyValue, String> {
    items
        .into_iter()
        .map(encode_owned)
        .collect::<Result<Vec<_>, _>>()
        .map(PyValue::List)
}

fn members(items: &[PyValue]) -> Result<PyValue, String> {
    items
        .iter()
        .map(encode)
        .collect::<Result<Vec<_>, _>>()
        .map(PyValue::List)
}

/// A dict as the original's globals hold it: `{name: value}` in the given order, each value a DTO.
pub fn dict_of(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::new();
    for (name, value) in entries {
        map.set(name, value);
    }
    PyValue::Dict(map)
}
