//! The typed value DTO read back into values (`r34_codec.py` `decode`): the same collision-free tag arrays [`crate::dto::encode`] writes. A call's
//! arguments arrive this way, so `bytes`, tuples, typed dict keys and the scalar distinctions (`True` is not `1`, `1.0` is not `1`) survive.
//!
//! A `set` arrives in its INPUT form `["set", members_sorted, order]`: the canonical members (sorted by their compact ASCII JSON, as every output
//! writes them) plus the order a `for x in the_set` loop yields them, which is an input fact of the caller's live set (the iteration order of a
//! Python `set` depends on hashes this program does not compute). `order` is a permutation of the member indexes or the string `"sorted"`.

use agent_runtime_support::pydyn::{dict_key, ensure_hashable};
use agent_runtime_support::pyvalue::{DictKey, Opaque, PyDict, PyValue, Typed};
use num_bigint::BigInt;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Distinguishes the NaN objects of one process (a NaN key equals only itself).
static NAN_OBJECTS: AtomicUsize = AtomicUsize::new(0);

fn bad(message: &str) -> String {
    format!("not a valid DTO: {message}")
}

fn float_of(text: &str) -> Result<f64, String> {
    match text {
        "inf" => Ok(f64::INFINITY),
        "-inf" => Ok(f64::NEG_INFINITY),
        "nan" => Ok(f64::NAN),
        _ if !text.is_empty()
            && text
                .chars()
                .all(|c| matches!(c, '0'..='9' | '+' | '-' | '.' | 'e')) =>
        {
            text.parse::<f64>()
                .map_err(|_| bad("a float is its Python repr"))
        }
        _ => Err(bad("a float is its Python repr")),
    }
}

fn bytes_of(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad("bytes are an even number of hex digits"));
    }
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[2 * at..2 * at + 2], 16).map_err(|_| bad("hex digits")))
        .collect()
}

fn members(data: &PyValue) -> Result<&[PyValue], String> {
    match data {
        PyValue::List(items) => Ok(items),
        _ => Err(bad("a list or tuple holds an array of DTOs")),
    }
}

fn pair_of(entry: &PyValue) -> Result<(&PyValue, &PyValue), String> {
    match entry {
        PyValue::List(pair) if pair.len() == 2 => Ok((&pair[0], &pair[1])),
        _ => Err(bad("a dict entry is a [key, value] pair")),
    }
}

/// The identity of a set member under Python's `==` and `hash` (`1`, `1.0` and `True` are one member).
fn member_identity(member: &PyValue) -> Result<String, String> {
    ensure_hashable(member).map_err(|failure| failure.message.to_utf8().unwrap_or_default())?;
    let held = dict_key(member, NAN_OBJECTS.fetch_add(1, Ordering::Relaxed))
        .map_err(|failure| failure.message.to_utf8().unwrap_or_default())?;
    Ok(match held {
        DictKey::Str(text) => format!("str:{}", text.repr()),
        DictKey::Other(other) => other.identity,
    })
}

/// The input form of a set: the canonical members, strictly ascending by compact ASCII JSON, hashable and pairwise unequal; the order, a
/// permutation of their indexes or `"sorted"`; the value holds the members in that order.
fn decode_set(parts: &[PyValue]) -> Result<PyValue, String> {
    let [_, listed, order] = parts else {
        return Err(bad(
            "a set input is [\"set\", members_sorted, order]: its iteration order is an input fact",
        ));
    };
    let canonical = members(listed)?;
    let mut texts: Vec<String> = Vec::with_capacity(canonical.len());
    for member in canonical {
        texts.push(
            member
                .dumps_ascii_tight()
                .map_err(|error| bad(&error.to_string()))?,
        );
    }
    if texts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(bad(
            "the members of a set are strictly ascending by their compact ASCII JSON",
        ));
    }
    let decoded: Vec<PyValue> = canonical.iter().map(decode).collect::<Result<_, _>>()?;
    let mut identities: Vec<String> = Vec::with_capacity(decoded.len());
    for member in &decoded {
        let identity = member_identity(member)?;
        if identities.contains(&identity) {
            return Err(bad("two members of a set are equal under Python's =="));
        }
        identities.push(identity);
    }
    let sequence: Vec<usize> = match order {
        PyValue::Str(text) if text.equals("sorted") => (0..decoded.len()).collect(),
        PyValue::List(indexes) => {
            let mut sequence = Vec::with_capacity(indexes.len());
            for index in indexes {
                let PyValue::Int(number) = index else {
                    return Err(bad("the order of a set is integers"));
                };
                sequence.push(
                    usize::try_from(number.clone())
                        .map_err(|_| bad("an index of the order of a set is out of range"))?,
                );
            }
            let mut seen = vec![false; decoded.len()];
            if sequence.len() != decoded.len()
                || sequence
                    .iter()
                    .any(|&index| index >= seen.len() || std::mem::replace(&mut seen[index], true))
            {
                return Err(bad(
                    "the order of a set is a permutation of its member indexes",
                ));
            }
            sequence
        }
        _ => return Err(bad("the order of a set is a permutation or \"sorted\"")),
    };
    let observed: Vec<PyValue> = sequence
        .iter()
        .map(|&index| decoded[index].clone())
        .collect();
    let shown: Vec<String> = observed.iter().map(PyValue::repr).collect();
    Ok(PyValue::Opaque(Opaque {
        type_name: "set",
        repr: if shown.is_empty() {
            "set()".to_string()
        } else {
            format!("{{{}}}", shown.join(", "))
        },
        truthy: !shown.is_empty(),
        typed: Typed::Set(observed),
        identity: None,
    }))
}

/// `decode(dto)`.
pub fn decode(dto: &PyValue) -> Result<PyValue, String> {
    let PyValue::List(parts) = dto else {
        return Err(bad("a DTO is a [tag, data] array"));
    };
    let Some(PyValue::Str(tag)) = parts.first() else {
        return Err(bad("a DTO starts with its tag"));
    };
    let tag = tag.to_utf8().unwrap_or_default();
    if tag == "none" {
        return if parts.len() == 1 {
            Ok(PyValue::None)
        } else {
            Err(bad("none has no data"))
        };
    }
    if tag == "set" {
        return decode_set(parts);
    }
    let [_, data] = parts.as_slice() else {
        return Err(bad("a DTO is a [tag, data] array"));
    };
    match (tag.as_str(), data) {
        ("bool", PyValue::Bool(flag)) => Ok(PyValue::Bool(*flag)),
        ("int", PyValue::Str(text)) => {
            let shown = text.to_utf8().unwrap_or_default();
            let digits = shown.strip_prefix('-').unwrap_or(&shown);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(bad("an int is a decimal string"));
            }
            BigInt::parse_bytes(shown.as_bytes(), 10)
                .map(PyValue::Int)
                .ok_or_else(|| bad("an int is a decimal string"))
        }
        ("float", PyValue::Str(text)) => {
            float_of(&text.to_utf8().unwrap_or_default()).map(PyValue::Float)
        }
        ("str", PyValue::Str(text)) => Ok(PyValue::Str(text.clone())),
        ("bytes", PyValue::Str(hex)) => {
            bytes_of(&hex.to_utf8().unwrap_or_default()).map(PyValue::Bytes)
        }
        ("list", data) => members(data)?
            .iter()
            .map(decode)
            .collect::<Result<_, _>>()
            .map(PyValue::List),
        ("tuple", data) => members(data)?
            .iter()
            .map(decode)
            .collect::<Result<_, _>>()
            .map(PyValue::Tuple),
        ("dict", data) => {
            let mut map = PyDict::new();
            for entry in members(data)? {
                let (key, value) = pair_of(entry)?;
                let (key, value) = (decode(key)?, decode(value)?);
                ensure_hashable(&key)
                    .map_err(|failure| failure.message.to_utf8().unwrap_or_default())?;
                match &key {
                    PyValue::Str(text) => map.insert(text.clone(), value),
                    other => {
                        let fresh = NAN_OBJECTS.fetch_add(1, Ordering::Relaxed);
                        let mut held = dict_key(other, fresh)
                            .map_err(|failure| failure.message.to_utf8().unwrap_or_default())?;
                        if let DictKey::Other(own) = &mut held {
                            own.value = Some(Box::new(other.clone()));
                        }
                        map.insert_key(held, value);
                    }
                }
            }
            Ok(PyValue::Dict(map))
        }
        _ => Err(bad("unknown tag or data of the wrong type")),
    }
}

/// The arguments of a call: `["list", [...]]` of DTOs.
pub fn decode_args(dto: &PyValue) -> Result<Vec<PyValue>, String> {
    match decode(dto)? {
        PyValue::List(items) => Ok(items),
        _ => Err(bad("args_dto is a list DTO")),
    }
}

/// The keyword arguments of a call: `["dict", [...]]` of DTOs.
pub fn decode_kwargs(dto: &PyValue) -> Result<PyDict, String> {
    match decode(dto)? {
        PyValue::Dict(map) => Ok(map),
        _ => Err(bad("kwargs_dto is a dict DTO")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::encode;
    use agent_runtime_support::pyjson::loads;

    fn dto(text: &str) -> PyValue {
        loads(text).expect("JSON")
    }

    #[test]
    fn every_tag_is_read_back_and_written_the_same() {
        let text = r#"["list", [["none"], ["bool", true], ["int", "-12345678901234567890"], ["float", "1.0"], ["float", "-inf"], ["str", "a\udc80"], ["bytes", "00ff"], ["tuple", [["int", "1"], ["bool", false]]], ["dict", [[["str", "k"], ["int", "1"]], [["int", "1"], ["str", "one"]], [["tuple", [["int", "2"]]], ["none"]]]]]]"#;
        let value = decode(&dto(text)).expect("decodes");
        let again = encode(&value).expect("encodes");
        assert_eq!(
            again.dumps_ascii().unwrap(),
            dto(text).dumps_ascii().unwrap()
        );
        // the scalar distinctions survive: 1, 1.0 and True are three different values
        let one = decode(&dto(r#"["dict", [[["int", "1"], ["str", "a"]], [["float", "1.0"], ["str", "b"]], [["bool", true], ["str", "c"]]]]"#)).unwrap();
        let PyValue::Dict(map) = &one else {
            panic!("a dict")
        };
        // Python holds ONE key for 1, 1.0 and True; the first object stays and the last value wins
        assert_eq!(map.len(), 1);
        assert_eq!(
            encode(&one).unwrap().dumps_ascii().unwrap(),
            r#"["dict", [[["int", "1"], ["str", "c"]]]]"#
        );
    }

    #[test]
    fn a_malformed_dto_an_unhashable_key_and_a_set_are_refused() {
        for text in [
            r#"["int", "1.5"]"#,
            r#"["int", 1]"#,
            r#"["float", "nope"]"#,
            r#"["bytes", "abc"]"#,
            r#"["bool", "true"]"#,
            r#"["none", 1]"#,
            r#"["dict", [[["list", []], ["none"]]]]"#,
            r#"["dict", [["str", "k"]]]"#,
            r#"["set", [["int", "1"]]]"#,
            r#"["unknown", 1]"#,
            r#""str""#,
        ] {
            assert!(decode(&dto(text)).is_err(), "{text}");
        }
    }
}
