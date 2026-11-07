//! The five functions of `resolve_ngc_tag.py` (`release_tuple`, `norm`, `is_tuple_prefix`, `inspect_env`, `gen_candidates`) as business functions
//! over typed Python values. Each returns the typed result of the call (the value and, when it IS an input object, that input's origin) or the
//! exception the original raises, with the constructor arguments and the attributes the saved originals record (`name`/`obj` of an
//! `AttributeError`).
//!
//! Semantics kept from the original, not substituted by `packaging`:
//!
//! * `release_tuple`: `re.match(r"^\s*v?(\d+(?:\.\d+)*)", str(v))` of a `str` pattern (Unicode `\s` and `\d`), each dot-separated segment through
//!   `int()` (any Unicode decimal digit, the 4300-digit limit) in order, so a limit `ValueError` of the first offending segment is raised.
//! * `norm`: the joined release (`str` of each integer), or the ORIGINAL object (same identity) when nothing parses.
//! * `is_tuple_prefix`: Python `==`, `len()` and slicing of whatever it is given (a `TypeError` for an unsized or unsliceable operand).
//! * `gen_candidates`: `start.split(".")` unpacked from a lazy generator of `int(x)` (conversion, not-enough/too-many unpack errors in the
//!   original's order), `range(months)` (only an integer or a `bool`), `{n:02d}` formatting of arbitrary-precision numbers.
//! * `inspect_env`: the exact `docker buildx imagetools inspect` command through the [`Docker`] boundary, both streams strictly decoded with
//!   universal newlines before the return code is read, every failure of the run caught as `None`, a JSON error caught as `None`, and everything
//!   after the JSON (`.get` on a non-dict, iteration of a non-iterable, `in` on a non-container) left uncaught.
//!
//! Limits named here (not Python's): the number of months a `range` may yield is capped at [`MONTHS_LIMIT`] (the original would run until memory
//! ends); suggestions Python may print under an `AttributeError` in a traceback are not reproduced.

use crate::ngc_inspect::{decode_stream, Docker};
use crate::torch_pin::{Origin, Returned};
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pynumber::int_from_str;
use agent_runtime_support::pysort::{py_eq, same_or_eq};
use agent_runtime_support::pystr::{is_decimal_digit, is_python_space, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue, Typed};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

pub const MODULE: &str = "resolve_ngc_tag";
pub const IMAGE: &str = "nvcr.io/nvidia/pytorch";
/// Most months `gen_candidates` produces (a named limit of this port; Python has none).
pub const MONTHS_LIMIT: u64 = 20_000_000;
const DIGIT_LIMIT: usize = 4300;

/// The place in the original where an exception is raised (the traceback's innermost frames and the lines of the source).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    /// `release_tuple` line 30, inside `tuple(int(part) ...)`.
    ReleaseSegments,
    /// `gen_candidates` line 68: `start.split` raised.
    GenSplit,
    /// `gen_candidates` line 68: the unpack, with the generator's `int(x)` failing inside it.
    GenInt,
    /// `gen_candidates` line 68: the unpack found the wrong number of values.
    GenUnpack,
    /// `gen_candidates` line 70: `range(months)`.
    GenRange,
    /// `inspect_env` line 56: `img.get`.
    ImgGet,
    /// `inspect_env` line 57: `(cfg or {}).get`.
    CfgGet,
    /// `inspect_env` line 57: the `.get("Env", [])` of the config value.
    ConfigGet,
    /// `inspect_env` line 59: `for e in env`.
    EnvLoop,
    /// `inspect_env` line 60: `"=" in e`.
    EnvIn,
    /// `inspect_env` line 61: `e.split`.
    EnvSplit,
    /// `len(...)` or the slice of `is_tuple_prefix`, and the binding errors: no frame the command line can reach.
    Elsewhere,
}

/// One exception: class, `str`, constructor arguments, attributes (`name`, `obj` of an `AttributeError`) and where it came from.
#[derive(Clone, Debug)]
pub struct Row {
    pub class: &'static str,
    pub module: &'static str,
    pub message: PyStr,
    pub args: Vec<PyValue>,
    pub attributes: Vec<(&'static str, PyValue)>,
    pub at: Where,
}

/// What a call can end with besides a value.
#[derive(Clone, Debug)]
pub enum Raised {
    /// The exception Python raises.
    Exception(Box<Row>),
    /// A request outside a named limit of this port.
    Limit(String),
}

impl Raised {
    pub fn builtin(class: &'static str, message: &str, at: Where) -> Raised {
        let message = PyStr::new(message);
        Raised::Exception(Box::new(Row {
            class,
            module: "builtins",
            args: vec![PyValue::Str(message.clone())],
            message,
            attributes: Vec::new(),
            at,
        }))
    }

    fn missing_attribute(object: &PyValue, name: &'static str, at: Where) -> Raised {
        let mut error = Raised::builtin(
            "AttributeError",
            &format!("'{}' object has no attribute '{name}'", object.type_name()),
            at,
        );
        if let Raised::Exception(row) = &mut error {
            row.attributes = vec![("name", PyValue::str(name)), ("obj", object.clone())];
        }
        error
    }

    fn from_failure(failure: agent_runtime_support::failure::Failure) -> Raised {
        let message = failure.message;
        Raised::Exception(Box::new(Row {
            class: failure.class,
            module: "builtins",
            args: vec![PyValue::Str(message.clone())],
            message,
            attributes: Vec::new(),
            at: Where::Elsewhere,
        }))
    }
}

pub type Call<T> = Result<T, Raised>;

/// `str(exception)` of the integer conversion, as a row of the given place.
fn conversion_error(error: agent_runtime_support::pynumber::PyException, at: Where) -> Raised {
    let class = if error.class == "ValueError" {
        "ValueError"
    } else {
        "TypeError"
    };
    let mut raised = Raised::builtin(class, &error.message, at);
    if let Raised::Exception(row) = &mut raised {
        row.class = if error.class == "OverflowError" {
            "OverflowError"
        } else {
            class
        };
    }
    raised
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// release_tuple / norm
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// The integers of the leading release of `text` (`None`: the pattern does not match).
pub fn release_of_text(text: &PyStr) -> Call<Option<Vec<BigInt>>> {
    let points = text.code_points();
    let mut at = 0;
    while at < points.len() && is_python_space(points[at]) {
        at += 1;
    }
    if at < points.len() && points[at] == u32::from('v') {
        at += 1;
    }
    let digits_end = |from: usize| {
        let mut end = from;
        while end < points.len() && is_decimal_digit(points[end]) {
            end += 1;
        }
        end
    };
    let first_end = digits_end(at);
    if first_end == at {
        return Ok(None);
    }
    let mut segments = vec![&points[at..first_end]];
    let mut end = first_end;
    while end + 1 < points.len()
        && points[end] == u32::from('.')
        && is_decimal_digit(points[end + 1])
    {
        let next = digits_end(end + 1);
        segments.push(&points[end + 1..next]);
        end = next;
    }
    let mut release = Vec::with_capacity(segments.len());
    for segment in segments {
        let number = int_from_str(&PyStr::from_code_points(segment.to_vec()))
            .map_err(|error| conversion_error(error, Where::ReleaseSegments))?;
        release.push(number);
    }
    Ok(Some(release))
}

/// `release_tuple(v)` for any value: `str(v)` first.
pub fn release_of(value: &PyValue) -> Call<Option<Vec<BigInt>>> {
    release_of_text(&value.display_text())
}

/// `".".join(map(str, release))`.
pub fn release_text(release: &[BigInt]) -> String {
    release
        .iter()
        .map(BigInt::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

/// `norm(v)` of a text the caller holds (`v` is a `str` here).
pub fn norm_text(text: &PyStr) -> Call<PyStr> {
    Ok(match release_of_text(text)? {
        Some(release) => PyStr::new(&release_text(&release)),
        None => text.clone(),
    })
}

const V: &[Param] = &[required("v")];
const A_B: &[Param] = &[required("a"), required("b")];
const TAG_ARCH: &[Param] = &[required("tag"), required("arch")];
const START_MONTHS: &[Param] = &[required("start"), required("months")];
static RELEASE_TUPLE: Signature = Signature {
    name: "release_tuple",
    params: V,
    rest: false,
};
static NORM: Signature = Signature {
    name: "norm",
    params: V,
    rest: false,
};
static IS_TUPLE_PREFIX: Signature = Signature {
    name: "is_tuple_prefix",
    params: A_B,
    rest: false,
};
static INSPECT_ENV: Signature = Signature {
    name: "inspect_env",
    params: TAG_ARCH,
    rest: false,
};
static GEN_CANDIDATES: Signature = Signature {
    name: "gen_candidates",
    params: START_MONTHS,
    rest: false,
};

fn parameter_origin(args: &[PyValue], kwargs: &PyDict, index: usize, name: &str) -> Origin {
    if index < args.len() {
        Origin(vec![("args", None), ("item", Some(index))])
    } else {
        let at = kwargs
            .iter()
            .position(|(key, _)| key.equals(name))
            .expect("binding found the keyword");
        Origin(vec![("kwargs", None), ("value", Some(at))])
    }
}

fn plain(value: PyValue) -> Returned {
    Returned {
        value,
        origin: None,
    }
}

/// `release_tuple(v)`.
pub fn release_tuple(args: &[PyValue], kwargs: &PyDict) -> Call<Returned> {
    let bound = RELEASE_TUPLE
        .bind(args, kwargs)
        .map_err(Raised::from_failure)?;
    Ok(plain(match release_of(&bound[0])? {
        Some(release) => PyValue::Tuple(release.into_iter().map(PyValue::Int).collect()),
        None => PyValue::None,
    }))
}

/// `norm(v)`: the joined release, or the very argument.
pub fn norm(args: &[PyValue], kwargs: &PyDict) -> Call<Returned> {
    let bound = NORM.bind(args, kwargs).map_err(Raised::from_failure)?;
    let v = &bound[0];
    Ok(match release_of(v)? {
        Some(release) => plain(PyValue::str(&release_text(&release))),
        None => Returned {
            value: v.clone(),
            origin: Some(parameter_origin(args, kwargs, 0, "v")),
        },
    })
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// is_tuple_prefix
// ---------------------------------------------------------------------------------------------------------------------------------------------

fn length_of(value: &PyValue) -> Call<usize> {
    match value.peel() {
        PyValue::Str(text) => Ok(text.len()),
        PyValue::Bytes(bytes) => Ok(bytes.len()),
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.len()),
        PyValue::Dict(map) => Ok(map.len()),
        PyValue::Opaque(opaque) => match &opaque.typed {
            Typed::Set(items) => Ok(items.len()),
            _ => Err(no_len(value)),
        },
        other => Err(no_len(other)),
    }
}

fn no_len(value: &PyValue) -> Raised {
    Raised::builtin(
        "TypeError",
        &format!("object of type '{}' has no len()", value.type_name()),
        Where::Elsewhere,
    )
}

/// `value[:count]`.
fn leading(value: &PyValue, count: usize) -> Call<PyValue> {
    Ok(match value.peel() {
        PyValue::Str(text) => PyValue::Str(PyStr::from_code_points(
            text.code_points()[..count].to_vec(),
        )),
        PyValue::Bytes(bytes) => PyValue::Bytes(bytes[..count].to_vec()),
        PyValue::List(items) => PyValue::List(items[..count].to_vec()),
        PyValue::Tuple(items) => PyValue::Tuple(items[..count].to_vec()),
        PyValue::Dict(_) => {
            return Err(Raised::builtin(
                "TypeError",
                "unhashable type: 'slice'",
                Where::Elsewhere,
            ))
        }
        other => {
            return Err(Raised::builtin(
                "TypeError",
                &format!("'{}' object is not subscriptable", other.type_name()),
                Where::Elsewhere,
            ))
        }
    })
}

/// `is_tuple_prefix(a, b)`.
pub fn is_tuple_prefix(args: &[PyValue], kwargs: &PyDict) -> Call<Returned> {
    let bound = IS_TUPLE_PREFIX
        .bind(args, kwargs)
        .map_err(Raised::from_failure)?;
    let (a, b) = (bound[0].peel(), bound[1].peel());
    if matches!(a, PyValue::None) || matches!(b, PyValue::None) || py_eq(a, b) {
        return Ok(plain(PyValue::Bool(false)));
    }
    let (length_a, length_b) = (length_of(a)?, length_of(b)?);
    let (short, long, short_length, long_length) = if length_a <= length_b {
        (a, b, length_a, length_b)
    } else {
        (b, a, length_b, length_a)
    };
    if short_length >= long_length {
        return Ok(plain(PyValue::Bool(false)));
    }
    let head = leading(long, short_length)?;
    Ok(plain(PyValue::Bool(same_or_eq(&head, short))))
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// gen_candidates
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// `format(number, "02d")`.
fn two_digits(number: &BigInt) -> Call<String> {
    let digits = number.abs().to_string();
    if digits.len() > DIGIT_LIMIT {
        return Err(Raised::builtin(
            "ValueError",
            "Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit",
            Where::Elsewhere,
        ));
    }
    let sign = if number.is_negative() { "-" } else { "" };
    let width = digits.len() + sign.len();
    Ok(format!(
        "{sign}{}{digits}",
        "0".repeat(2usize.saturating_sub(width))
    ))
}

/// `start.split(".")`.
fn split_start(start: &PyValue) -> Call<Vec<PyStr>> {
    match start.peel() {
        PyValue::Str(text) => Ok(text
            .code_points()
            .split(|&point| point == u32::from('.'))
            .map(|piece| PyStr::from_code_points(piece.to_vec()))
            .collect()),
        PyValue::Bytes(_) => Err(Raised::builtin(
            "TypeError",
            "a bytes-like object is required, not 'str'",
            Where::GenSplit,
        )),
        other => Err(Raised::missing_attribute(other, "split", Where::GenSplit)),
    }
}

/// `yy, mm = (int(x) for x in start.split("."))`.
fn year_and_month(start: &PyValue) -> Call<(BigInt, BigInt)> {
    let parts = split_start(start)?;
    let mut numbers = Vec::with_capacity(2);
    for (at, part) in parts.iter().enumerate() {
        let number = int_from_str(part).map_err(|error| conversion_error(error, Where::GenInt))?;
        if at == 2 {
            return Err(Raised::builtin(
                "ValueError",
                "too many values to unpack (expected 2)",
                Where::GenUnpack,
            ));
        }
        numbers.push(number);
    }
    match <[BigInt; 2]>::try_from(numbers) {
        Ok([year, month]) => Ok((year, month)),
        Err(few) => Err(Raised::builtin(
            "ValueError",
            &format!(
                "not enough values to unpack (expected 2, got {})",
                few.len()
            ),
            Where::GenUnpack,
        )),
    }
}

/// `range(months)` for the count (only an `int` or `bool` is an index).
fn range_length(months: &PyValue) -> Call<u64> {
    let count = match months.peel() {
        PyValue::Int(number) => number.clone(),
        PyValue::Bool(flag) => BigInt::from(u8::from(*flag)),
        other => {
            return Err(Raised::builtin(
                "TypeError",
                &format!(
                    "'{}' object cannot be interpreted as an integer",
                    other.type_name()
                ),
                Where::GenRange,
            ))
        }
    };
    if !count.is_positive() {
        return Ok(0);
    }
    match count.to_u64() {
        Some(length) if length <= MONTHS_LIMIT => Ok(length),
        _ => Err(Raised::Limit(format!(
            "gen_candidates would build more than {MONTHS_LIMIT} months; Python would run until memory ends, this port refuses"
        ))),
    }
}

/// `gen_candidates(start, months)` over a text start (the command line's case) or any value (the API's).
pub fn candidates_of(start: &PyValue, months: &PyValue) -> Call<Vec<PyStr>> {
    let (mut year, mut month) = year_and_month(start)?;
    let length = range_length(months)?;
    let mut tags = Vec::new();
    for _ in 0..length {
        tags.push(PyStr::new(&format!(
            "{}.{}-py3",
            two_digits(&year)?,
            two_digits(&month)?
        )));
        month -= 1;
        if month.is_zero() {
            month = BigInt::from(12);
            year -= 1;
        }
    }
    Ok(tags)
}

/// `gen_candidates(start, months)`.
pub fn gen_candidates(args: &[PyValue], kwargs: &PyDict) -> Call<Returned> {
    let bound = GEN_CANDIDATES
        .bind(args, kwargs)
        .map_err(Raised::from_failure)?;
    Ok(plain(PyValue::List(
        candidates_of(&bound[0], &bound[1])?
            .into_iter()
            .map(PyValue::Str)
            .collect(),
    )))
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// inspect_env
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// `["docker", "buildx", "imagetools", "inspect", ref, "--format", "{{json .Image}}"]` as the bytes the child receives (`None`: the argument
/// cannot be encoded or holds a NUL, which `subprocess` refuses before it starts anything).
fn docker_argv(tag: &PyValue) -> Option<Vec<Vec<u8>>> {
    let mut reference = PyStr::new(&format!("{IMAGE}:"));
    reference.push_py(&tag.display_text());
    let mut argv = Vec::new();
    for word in [
        PyStr::new("docker"),
        PyStr::new("buildx"),
        PyStr::new("imagetools"),
        PyStr::new("inspect"),
        reference,
        PyStr::new("--format"),
        PyStr::new("{{json .Image}}"),
    ] {
        let bytes = word.to_surrogateescape_bytes().ok()?;
        if bytes.contains(&0) {
            return None;
        }
        argv.push(bytes);
    }
    Some(argv)
}

fn text_key(text: &str) -> PyStr {
    PyStr::new(text)
}

fn entry<'a>(map: &'a PyDict, key: &PyStr) -> Option<&'a PyValue> {
    map.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn or_value(candidate: Option<&PyValue>, fallback: PyValue) -> PyValue {
    match candidate {
        Some(value) if value.is_truthy() => value.clone(),
        _ => fallback,
    }
}

/// The environment variables of the image: everything after the successful JSON parse (unprotected in the original).
fn environment_of(image: &PyValue, arch: &PyValue) -> Call<PyDict> {
    let PyValue::Dict(root) = image.peel() else {
        return Err(Raised::missing_attribute(
            image.peel(),
            "get",
            Where::ImgGet,
        ));
    };
    let mut selector = PyStr::new("linux/");
    selector.push_py(&arch.display_text());
    let config_holder = or_value(entry(root, &selector), image.clone());
    let holder = or_value(Some(&config_holder), PyValue::Dict(PyDict::new()));
    let PyValue::Dict(holder_map) = holder.peel() else {
        return Err(Raised::missing_attribute(
            holder.peel(),
            "get",
            Where::CfgGet,
        ));
    };
    let config = or_value(
        entry(holder_map, &text_key("config")),
        PyValue::Dict(PyDict::new()),
    );
    let PyValue::Dict(config_map) = config.peel() else {
        return Err(Raised::missing_attribute(
            config.peel(),
            "get",
            Where::ConfigGet,
        ));
    };
    let env = or_value(
        entry(config_map, &text_key("Env")),
        PyValue::List(Vec::new()),
    );
    let members: Vec<PyValue> = match env.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items.clone(),
        PyValue::Str(text) => text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect(),
        PyValue::Dict(map) => map
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect(),
        other => {
            return Err(Raised::builtin(
                "TypeError",
                &format!("'{}' object is not iterable", other.type_name()),
                Where::EnvLoop,
            ))
        }
    };
    let equals = u32::from('=');
    let mut variables = PyDict::new();
    for member in &members {
        match member.peel() {
            PyValue::Str(text) => {
                if let Some(split) = text.code_points().iter().position(|&point| point == equals) {
                    let (key, value) = text.code_points().split_at(split);
                    variables.insert(
                        PyStr::from_code_points(key.to_vec()),
                        PyValue::Str(PyStr::from_code_points(value[1..].to_vec())),
                    );
                }
            }
            holder @ (PyValue::List(_) | PyValue::Tuple(_) | PyValue::Dict(_)) => {
                let contains = match holder {
                    PyValue::Dict(map) => map.iter().any(|(key, _)| key.equals("=")),
                    PyValue::List(items) | PyValue::Tuple(items) => {
                        let sign = PyValue::str("=");
                        items.iter().any(|item| same_or_eq(item, &sign))
                    }
                    _ => false,
                };
                if contains {
                    return Err(Raised::missing_attribute(holder, "split", Where::EnvSplit));
                }
            }
            other => {
                return Err(Raised::builtin(
                    "TypeError",
                    &format!("argument of type '{}' is not iterable", other.type_name()),
                    Where::EnvIn,
                ))
            }
        }
    }
    Ok(variables)
}

/// `inspect_env(tag, arch)`: `Ok(None)` is the original's `None` (the run or the JSON failed inside its `try`).
pub fn inspect(docker: &dyn Docker, tag: &PyValue, arch: &PyValue) -> Call<Option<PyDict>> {
    let Some(argv) = docker_argv(tag) else {
        return Ok(None);
    };
    let Some(finished) = docker.run(&argv) else {
        return Ok(None);
    };
    let Some(out) = decode_stream(&finished.stdout) else {
        return Ok(None);
    };
    if decode_stream(&finished.stderr).is_none() || finished.status != 0 {
        return Ok(None);
    }
    let Ok(image) = loads_text(&out) else {
        return Ok(None);
    };
    environment_of(&image, arch).map(Some)
}

/// `inspect_env(tag, arch)` as a called function.
pub fn inspect_env(docker: &dyn Docker, args: &[PyValue], kwargs: &PyDict) -> Call<Returned> {
    let bound = INSPECT_ENV
        .bind(args, kwargs)
        .map_err(Raised::from_failure)?;
    Ok(plain(match inspect(docker, &bound[0], &bound[1])? {
        Some(variables) => PyValue::Dict(variables),
        None => PyValue::None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> PyValue {
        PyValue::str(value)
    }

    fn release(value: &PyValue) -> Option<Vec<String>> {
        release_of(value)
            .expect("no exception")
            .map(|numbers| numbers.iter().map(BigInt::to_string).collect())
    }

    #[test]
    fn the_release_follows_the_str_pattern() {
        assert_eq!(
            release(&text(" \t\nv2.11")),
            Some(vec!["2".into(), "11".into()])
        );
        assert_eq!(
            release(&text("\u{1c}2.11")),
            Some(vec!["2".into(), "11".into()])
        );
        assert_eq!(
            release(&text("\u{662}.\u{661}\u{661}")),
            Some(vec!["2".into(), "11".into()])
        );
        assert_eq!(release(&text("2..11")), Some(vec!["2".into()]));
        assert_eq!(release(&text("2.11.")), Some(vec!["2".into(), "11".into()]));
        assert_eq!(release(&text("V2.11")), None);
        assert_eq!(release(&text("\u{b2}.11")), None);
        assert_eq!(release(&PyValue::int(211)), Some(vec!["211".into()]));
        assert_eq!(release(&PyValue::Bytes(b"2.11".to_vec())), None);
    }

    #[test]
    fn the_digit_limit_is_raised_by_the_first_long_segment() {
        let long = "7".repeat(4301);
        let Err(Raised::Exception(row)) = release_of(&text(&format!("1.{long}.{long}"))) else {
            panic!("limit")
        };
        assert_eq!(row.class, "ValueError");
        assert!(row
            .message
            .to_utf8()
            .unwrap()
            .starts_with("Exceeds the limit (4300 digits)"));
    }

    #[test]
    fn months_count_down_with_a_year_rollover_and_unvalidated_months() {
        let list = |start: &str, count: i64| {
            candidates_of(&text(start), &PyValue::int(count)).map(|tags| {
                tags.iter()
                    .map(|tag| tag.to_utf8().unwrap())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            list("26.01", 3).unwrap(),
            ["26.01-py3", "25.12-py3", "25.11-py3"]
        );
        assert_eq!(list("00.01", 2).unwrap(), ["00.01-py3", "-1.12-py3"]);
        assert_eq!(list("26.00", 2).unwrap(), ["26.00-py3", "26.-1-py3"]);
        assert!(list("bad", 0).is_err());
        assert!(list("26.05", -1).unwrap().is_empty());
    }

    #[test]
    fn unpack_errors_follow_the_generator_order() {
        let message = |start: &str| match candidates_of(&text(start), &PyValue::int(0)) {
            Err(Raised::Exception(row)) => row.message.to_utf8().unwrap(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            message("26"),
            "not enough values to unpack (expected 2, got 1)"
        );
        assert_eq!(message("1.2.3"), "too many values to unpack (expected 2)");
        assert_eq!(
            message("1.2.x"),
            "invalid literal for int() with base 10: 'x'"
        );
    }

    #[test]
    fn prefix_uses_python_equality_len_and_slicing() {
        let call = |a: PyValue, b: PyValue| is_tuple_prefix(&[a, b], &PyDict::new());
        let tuple =
            |items: &[i64]| PyValue::Tuple(items.iter().map(|&n| PyValue::int(n)).collect());
        let verdict = |result: Call<Returned>| matches!(result.unwrap().value, PyValue::Bool(true));
        assert!(verdict(call(tuple(&[2, 11]), tuple(&[2, 11, 0]))));
        assert!(verdict(call(tuple(&[2, 11, 0]), tuple(&[2, 11]))));
        assert!(!verdict(call(tuple(&[2, 11]), tuple(&[2, 11]))));
        assert!(!verdict(call(PyValue::None, tuple(&[2]))));
        assert!(verdict(call(
            PyValue::Tuple(vec![PyValue::Bool(true)]),
            tuple(&[1, 0])
        )));
        assert!(!verdict(call(
            PyValue::List(vec![PyValue::int(2)]),
            tuple(&[2, 0])
        )));
        let Err(Raised::Exception(row)) = call(PyValue::int(1), tuple(&[1])) else {
            panic!("len")
        };
        assert_eq!(
            row.message.to_utf8().unwrap(),
            "object of type 'int' has no len()"
        );
    }
}
