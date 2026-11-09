//! Private bridge of the three in-process importers (`relay`, `bootstrap_canary`, `selftest_harness_switch`) of
//! `turn_budget.py`: one request on standard input, one answer on standard output, selected by `TURN_BUDGET_BRIDGE` in the binary
//! `turn_budget` (so the public command line gains no argument).
//!
//! **Wire (`turn-budget-bridge/1`).** Every Python value is a typed object: `{"t":"none"}`, `{"t":"bool","v":b}`,
//! `{"t":"int","hex":"+1f"}` (sign and lowercase hexadecimal: there is no decimal conversion, so no 4300-digit limit in transport),
//! `{"t":"float","repr":"1.5"}`, `{"t":"str","v":text}` (JSON escapes carry lone surrogates), `{"t":"bytes","hex":..}`,
//! `{"t":"list"|"tuple","v":[..]}`, `{"t":"dict","v":[[key,value],..]}` (string keys, in insertion order) and
//! `{"t":"pathlike","fspath":value,"text":str}` (an `os.PathLike`: its `os.fspath()` and its `str()`). Booleans are tagged before
//! integers. The importer's side only marshals types; this side owns every decision.
//!
//! **Answers.** `returned` with a value; `raised` with the exception (`cls`, constructor `ctor` / `ctor_kwargs` that rebuild the
//! exact object, `context`, `suppress_context`; the frames are the CLI's business and are not carried); `unsupported` with the
//! reason for a shape the port does not carry (reported before any business; nothing is guessed). The text profile is the
//! environment's, as for the command line.

use crate::turn_budget_cli::{observe_event, Environment};
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr as Text;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::{BigInt, Sign};
use role_contract::pyprofile;
use role_contract::turn_budget::{self as budget, Declaration, Failure, PathValue, Raised};

pub const BRIDGE_ENV: &str = "TURN_BUDGET_BRIDGE";
pub const PROTOCOL: &str = "turn-budget-bridge/1";

fn tagged(tag: &str) -> PyDict {
    let mut dict = PyDict::new();
    dict.set("t", PyValue::str(tag));
    dict
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hexadecimal length".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// A Python object as its wire form.
fn encode(value: &PyValue) -> Result<PyValue, String> {
    let mut out;
    match value.peel() {
        PyValue::None => out = tagged("none"),
        PyValue::Bool(flag) => {
            out = tagged("bool");
            out.set("v", PyValue::Bool(*flag));
        }
        PyValue::Int(number) => {
            out = tagged("int");
            let sign = if number.sign() == Sign::Minus {
                "-"
            } else {
                "+"
            };
            out.set(
                "hex",
                PyValue::str(&format!("{sign}{:x}", number.magnitude())),
            );
        }
        PyValue::Float(number) => {
            out = tagged("float");
            out.set("repr", PyValue::str(&PyValue::Float(*number).repr()));
        }
        PyValue::Str(text) => {
            out = tagged("str");
            out.set("v", PyValue::Str(text.clone()));
        }
        PyValue::Bytes(bytes) => {
            out = tagged("bytes");
            out.set("hex", PyValue::str(&hex_bytes(bytes)));
        }
        PyValue::List(items) | PyValue::Tuple(items) => {
            out = tagged(if matches!(value.peel(), PyValue::List(_)) {
                "list"
            } else {
                "tuple"
            });
            let encoded: Result<Vec<PyValue>, String> = items.iter().map(encode).collect();
            out.set("v", PyValue::List(encoded?));
        }
        PyValue::Dict(dict) => {
            out = tagged("dict");
            let mut pairs = Vec::new();
            for (key, item) in dict.iter() {
                pairs.push(PyValue::List(vec![
                    encode(&PyValue::Str(key.clone()))?,
                    encode(item)?,
                ]));
            }
            out.set("v", PyValue::List(pairs));
        }
        other => {
            return Err(format!(
                "a value of type {} has no wire form",
                other.type_name()
            ))
        }
    }
    Ok(PyValue::Dict(out))
}

fn member<'a>(dict: &'a PyDict, name: &str) -> Result<&'a PyValue, String> {
    dict.get(name)
        .ok_or_else(|| format!("the wire object has no \"{name}\""))
}

fn string_of(value: &PyValue) -> Result<String, String> {
    match value.peel() {
        PyValue::Str(text) => text
            .to_utf8()
            .ok_or_else(|| "a wire word holds a lone surrogate".to_string()),
        _ => Err("a wire word is not a string".into()),
    }
}

/// A wire form as the Python object it stands for.
fn decode(value: &PyValue) -> Result<PyValue, String> {
    let PyValue::Dict(dict) = value.peel() else {
        return Err("a wire value is not an object".into());
    };
    match string_of(member(dict, "t")?)?.as_str() {
        "none" => Ok(PyValue::None),
        "bool" => match member(dict, "v")?.peel() {
            PyValue::Bool(flag) => Ok(PyValue::Bool(*flag)),
            _ => Err("a bool value is not a boolean".into()),
        },
        "int" => {
            let word = string_of(member(dict, "hex")?)?;
            let (negative, digits) = match word.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, word.strip_prefix('+').unwrap_or(&word)),
            };
            let magnitude = BigInt::parse_bytes(digits.as_bytes(), 16)
                .ok_or_else(|| "an int value is not hexadecimal".to_string())?;
            Ok(PyValue::Int(if negative { -magnitude } else { magnitude }))
        }
        "float" => {
            let word = string_of(member(dict, "repr")?)?;
            word.parse::<f64>()
                .map(PyValue::Float)
                .map_err(|_| format!("a float value {word:?} is not a number"))
        }
        "str" => match member(dict, "v")?.peel() {
            PyValue::Str(text) => Ok(PyValue::Str(text.clone())),
            _ => Err("a str value is not a string".into()),
        },
        "bytes" => Ok(PyValue::Bytes(unhex(&string_of(member(dict, "hex")?)?)?)),
        tag @ ("list" | "tuple") => {
            let PyValue::List(items) = member(dict, "v")?.peel() else {
                return Err("a sequence value is not a list".into());
            };
            let decoded: Result<Vec<PyValue>, String> = items.iter().map(decode).collect();
            Ok(if tag == "list" {
                PyValue::List(decoded?)
            } else {
                PyValue::Tuple(decoded?)
            })
        }
        "dict" => {
            let PyValue::List(pairs) = member(dict, "v")?.peel() else {
                return Err("a dict value is not a list of pairs".into());
            };
            let mut out = PyDict::new();
            for pair in pairs {
                let PyValue::List(pair) = pair.peel() else {
                    return Err("a dict pair is not a list".into());
                };
                let [key, item] = pair.as_slice() else {
                    return Err("a dict pair has not two members".into());
                };
                match decode(key)? {
                    PyValue::Str(key) => out.insert(key, decode(item)?),
                    _ => return Err("a dict key is not a string".into()),
                }
            }
            Ok(PyValue::Dict(out))
        }
        "pathlike" => Err("a path-like object is only allowed as a path".into()),
        other => Err(format!("unknown wire type {other:?}")),
    }
}

fn decode_path(value: &PyValue) -> Result<PathValue, String> {
    if let PyValue::Dict(dict) = value.peel() {
        if string_of(member(dict, "t")?)? == "pathlike" {
            let fspath = decode(member(dict, "fspath")?)?;
            let PyValue::Str(shown) = decode(member(dict, "text")?)? else {
                return Err("a path-like text is not a string".into());
            };
            return Ok(PathValue::Like {
                fspath,
                text: shown,
            });
        }
    }
    Ok(PathValue::Value(decode(value)?))
}

fn encode_raised(raised: &Raised) -> Result<PyValue, String> {
    let mut out = PyDict::new();
    out.set("cls", PyValue::str(raised.class));
    // `str(exception)` as the owner shows it (diagnostic: the importer rebuilds the object from `ctor`)
    out.set("message", PyValue::Str(raised.message.clone()));
    let ctor: Result<Vec<PyValue>, String> = raised.ctor.iter().map(encode).collect();
    out.set("ctor", PyValue::List(ctor?));
    let mut kwargs = PyDict::new();
    for (name, value) in raised.ctor_kwargs.iter() {
        kwargs.set(name, encode(value)?);
    }
    out.set("ctor_kwargs", PyValue::Dict(kwargs));
    out.set(
        "context",
        match &raised.context {
            Some(context) => encode_raised(context)?,
            None => PyValue::None,
        },
    );
    out.set("suppress_context", PyValue::Bool(raised.suppress_context));
    Ok(PyValue::Dict(out))
}

fn answer(status: &str, key: &str, value: PyValue) -> PyValue {
    let mut out = PyDict::new();
    out.set("protocol", PyValue::str(PROTOCOL));
    out.set("status", PyValue::str(status));
    out.set(key, value);
    PyValue::Dict(out)
}

fn finish(result: Result<PyValue, Failure>) -> Result<PyValue, String> {
    match result {
        Ok(value) => Ok(answer("returned", "value", encode(&value)?)),
        Err(Failure::Raised(raised)) => Ok(answer("raised", "exc", encode_raised(&raised)?)),
        Err(Failure::Unsupported(why)) => Ok(answer("unsupported", "reason", PyValue::str(&why))),
    }
}

fn serve_request(request: &PyValue) -> Result<PyValue, String> {
    let PyValue::Dict(request) = request.peel() else {
        return Err("the request is not an object".into());
    };
    if string_of(member(request, "protocol")?)? != PROTOCOL {
        return Err(format!("the request does not speak {PROTOCOL}"));
    }
    match string_of(member(request, "op")?)?.as_str() {
        "schema_cap" => {
            let field = decode(member(request, "field")?)?;
            let path = decode_path(member(request, "path")?)?;
            finish(budget::schema_cap(&field, &path))
        }
        "declare" => {
            let mut declaration = Declaration::new(
                decode(member(request, "max_turns")?)?,
                decode(member(request, "timeout_seconds")?)?,
                decode(member(request, "source")?)?,
            );
            declaration.caps = decode(member(request, "caps")?)?;
            declaration.unit = decode(member(request, "unit")?)?;
            let default_schema = decode_path(member(request, "default_schema")?)?;
            finish(budget::declare(&declaration, &default_schema))
        }
        other => Err(format!("unknown operation {other:?}")),
    }
}

/// The outcome word of an answer: `returned`, `unsupported` or `raised:<class>`. A `raised` answer travels with transport exit 0:
/// what the Python importer finally observes is the exception its adapter raises, not this process's status.
fn outcome_of(answer: &PyValue) -> String {
    let PyValue::Dict(answer) = answer.peel() else {
        return "malformed".into();
    };
    let status = answer
        .get("status")
        .and_then(|s| string_of(s).ok())
        .unwrap_or_default();
    if status != "raised" {
        return status;
    }
    let class = match answer.get("exc").map(PyValue::peel) {
        Some(PyValue::Dict(exc)) => exc
            .get("cls")
            .and_then(|c| string_of(c).ok())
            .unwrap_or_default(),
        _ => String::new(),
    };
    format!("raised:{class}")
}

/// Serves one request. The answer is JSON text (ASCII); a protocol error is an `Err` the binary reports on standard error.
/// The route is recorded (when `TURN_BUDGET_ROUTE_OBSERVER` names a file) before the business starts and again with the outcome;
/// an unwritable requested observer is a control failure.
pub fn serve(input: &[u8], env: &Environment) -> Result<String, String> {
    let profile = env
        .text_profile()
        .map_err(|reason| format!("the text profile is not modelled: {reason}"))?;
    let text = Text::from_surrogateescape(input);
    let request = loads_text(&text).map_err(|error| format!("the request is not JSON: {error}"))?;
    let shape = match request.peel() {
        PyValue::Dict(dict) => dict.get("op").and_then(|op| string_of(op).ok()),
        _ => None,
    };
    let owner = env.owner.as_deref();
    observe_event(
        env,
        "start",
        "NativeBridge",
        None,
        owner,
        shape.as_deref(),
        None,
    )?;
    let answer = pyprofile::with_profile(&profile, || serve_request(&request))?;
    observe_event(
        env,
        "end",
        "NativeBridge",
        None,
        owner,
        shape.as_deref(),
        Some(&outcome_of(&answer)),
    )?;
    answer.dumps_ascii().map_err(|error| error.to_string())
}
