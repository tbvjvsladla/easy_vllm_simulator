//! The observation of one `tomllib.loads(text)` call: the typed value, or the exception with its cause chain, in the tag arrays the saved originals
//! use. Every integer is `["int_hex", signed lowercase hex]` (no digit limit applies to it), a float is `["float", repr, big-endian IEEE-754 hex]`
//! (the NaN sign and `-0.0` stay visible), and the three temporal kinds are distinct tags with the `isoformat()` text.

use super::failure::{LoadsFailure, Raised, UnsupportedDomain};
use super::parser::loads;
use super::value::{Table, TomlValue};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{float_repr, PyDict, PyValue};
use num_bigint::Sign;

/// The scope string of every record of this component.
pub const SCOPE: &str = "tomllib-default-loads-cpython3123";

fn tag(name: &str, data: PyValue) -> PyValue {
    PyValue::List(vec![PyValue::str(name), data])
}

fn text_value(points: &[u32]) -> PyValue {
    tag(
        "str",
        PyValue::Str(PyStr::from_code_points(points.to_vec())),
    )
}

/// `hex(value)`: `0x2a`, `-0x2a`, `0x0`.
fn signed_hex(number: &num_bigint::BigInt) -> String {
    let sign = if number.sign() == Sign::Minus {
        "-"
    } else {
        ""
    };
    format!("{sign}0x{}", number.magnitude().to_str_radix(16))
}

fn table_value(table: &Table) -> PyValue {
    let pairs = table
        .iter()
        .map(|(key, value)| PyValue::List(vec![text_value(key), typed(value)]))
        .collect();
    tag("dict", PyValue::List(pairs))
}

/// The typed observation of a parsed value.
pub fn typed(value: &TomlValue) -> PyValue {
    match value {
        TomlValue::Bool(flag) => tag("bool", PyValue::Bool(*flag)),
        TomlValue::Int(number) => tag("int_hex", PyValue::str(&signed_hex(number))),
        TomlValue::Float(number) => PyValue::List(vec![
            PyValue::str("float"),
            PyValue::str(&float_repr(*number)),
            PyValue::str(&format!("{:016x}", number.to_bits())),
        ]),
        TomlValue::Str(points) => text_value(points),
        TomlValue::Date(date) => tag("date", PyValue::str(&date.isoformat())),
        TomlValue::Time(time) => tag("time", PyValue::str(&time.isoformat())),
        TomlValue::DateTime(moment) => tag("datetime", PyValue::str(&moment.isoformat())),
        TomlValue::Array(items) => tag("list", PyValue::List(items.iter().map(typed).collect())),
        TomlValue::Table(table) => table_value(table),
    }
}

/// `encode(exception.args)`: `["tuple", [["str", message]]]`.
fn args_value(raised: &Raised) -> PyValue {
    tag(
        "tuple",
        PyValue::List(vec![tag("str", PyValue::str(&raised.message))]),
    )
}

/// One row of `exception_chain`.
fn chain_row(raised: &Raised) -> PyValue {
    let mut row = PyDict::new();
    row.set("type", PyValue::str(raised.class.name()));
    row.set("module", PyValue::str(raised.class.module()));
    row.set("message", PyValue::str(&raised.message));
    row.set("suppress_context", PyValue::Bool(raised.suppress_context));
    row.set(
        "relation",
        if raised.cause.is_some() {
            PyValue::str("cause")
        } else {
            PyValue::None
        },
    );
    row.set("args", args_value(raised));
    PyValue::Dict(row)
}

/// The projection of a raised exception: `{kind: "exception", type, module, message, args, exception_chain}`.
fn exception_projection(raised: &Raised) -> PyValue {
    let mut chain = Vec::new();
    let mut current = Some(raised);
    while let Some(error) = current {
        chain.push(chain_row(error));
        current = error.cause.as_deref();
    }
    let mut projection = PyDict::new();
    projection.set("kind", PyValue::str("exception"));
    projection.set("type", PyValue::str(raised.class.name()));
    projection.set("module", PyValue::str(raised.class.module()));
    projection.set("message", PyValue::str(&raised.message));
    projection.set("args", args_value(raised));
    projection.set("exception_chain", PyValue::List(chain));
    PyValue::Dict(projection)
}

/// `tomllib.loads(source)` observed. `Err` is a domain this component does not model (a refusal, not a parser exception).
pub fn observe_loads(source: &PyStr) -> Result<PyValue, UnsupportedDomain> {
    match loads(source.code_points()) {
        Ok(table) => {
            let mut projection = PyDict::new();
            projection.set("kind", PyValue::str("value"));
            projection.set("value", table_value(&table));
            Ok(PyValue::Dict(projection))
        }
        Err(LoadsFailure::Raised(raised)) => Ok(exception_projection(&raised)),
        Err(LoadsFailure::Unsupported(domain)) => Err(domain),
    }
}
