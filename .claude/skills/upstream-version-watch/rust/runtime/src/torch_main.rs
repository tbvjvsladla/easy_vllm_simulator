//! `main()` of `resolve_torch_pin.py`: the vLLM version given on the command line becomes the URL of its `pyproject.toml` (the tag, then the release
//! branch), the fetched text is parsed as TOML, the first `torch` requirement of `[build-system].requires` is read and the pin or the range bounds
//! are written as JSON. The business of the original is kept in its order: only the fetch and the TOML parse are inside the `try`, everything after
//! it is unprotected (a value of the wrong shape ends in the original's uncaught `AttributeError`/`TypeError`), a requirement the constructor rejects is
//! skipped (its warnings stay), and of several `==`/`===` specifiers the last one iterated wins, even an empty one.
//!
//! The pieces are the library's own: [`crate::fetch_https`] (real TLS, optional `CONNECT`), [`crate::tomllib::loads`], [`crate::requirement`],
//! [`crate::torch_pin`] (`norm_prefix`, `pick_bound`) and the argparse model of the recipe package. This module adds only what `main` itself does.
//!
//! Exit status: 0; 2 (`argparse`, or no `pyproject.toml` could be fetched and parsed); 3 (no `torch` requirement); 1 (an uncaught exception).
//! Outcomes the original has no counterpart for in this port are never disguised as one of those: a transport failure whose Python spelling was not
//! captured, a request outside a modelled domain and an invalid `PYTHONHASHSEED` end with status 70 and a named report ([`Refusal`]).

use crate::host::Sink;
use crate::http_direct::FetchFailure;
use crate::main_cmd::{argument_error, basename, intern, split_usage};
use crate::pyhash::HashSecret;
use crate::requirement::projection::{project_requirement, AcceptedRequirement, Projection};
use crate::thrown::{Done, Thrown};
use crate::tomllib::{loads, LoadsFailure, Table, TomlValue};
use crate::torch_pin::{norm_prefix, pick_bound, Raised as PinRaised};
use crate::torch_traceback::{
    json_default_frames, json_encoder_frames, Site, MAIN_BUILD_SYSTEM_GET, MAIN_REQUIRES_LOOP,
    MODULE_ENTRY,
};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{float_repr, PyDict, PyValue};
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Positional, Spec, Value,
};
use recipe_runtime::escalation::Raised;
use std::collections::BTreeMap;

/// The program name of every usage text (`sys.argv[0]`'s basename), whatever the executable is called.
pub const PROGRAM: &str = "resolve_torch_pin.py";

/// `REFS`: the tag first, then the release branch.
const REFS: [&str; 2] = ["v{v}", "releases/v{v}"];
const URL_PREFIX: &str = "https://raw.githubusercontent.com/vllm-project/vllm/";
const URL_SUFFIX: &str = "/pyproject.toml";
/// The status of an outcome this port does not decide.
pub const REFUSAL_STATUS: i32 = 70;

/// The source of the fetched text: `fetch(url)` of the original.
pub type Fetcher<'a> = &'a dyn Fn(&PyStr) -> Result<String, FetchFailure>;

/// What a call runs on: the console, `sys.argv`, the terminal width, the hash profile of the interpreter and the fetch.
pub struct TorchWorld<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
    pub secret: &'a HashSecret,
    pub fetch: Fetcher<'a>,
    /// The frames an uncaught exception shows, innermost first.
    pub frames: Vec<Site>,
}

/// An outcome this port names and does not guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub boundary: &'static str,
    pub detail: String,
}

/// How `main` stopped before returning.
pub enum Stop {
    /// `sys.exit(code)` or `SystemExit` of argparse.
    Exit(i32),
    /// An uncaught exception (its frames are in [`TorchWorld::frames`]).
    Uncaught(Box<Thrown>),
    Refused(Refusal),
}

impl From<Box<Thrown>> for Stop {
    fn from(thrown: Box<Thrown>) -> Stop {
        Stop::Uncaught(thrown)
    }
}

type Flow<T> = Result<T, Stop>;

pub fn spec(prog: &str) -> Spec {
    Spec {
        prog: intern(prog),
        description: "",
        options: vec![Opt {
            flags: &["-h", "--help"],
            dest: "help",
            kind: Kind::Help,
            help: None,
            required: false,
            group: None,
        }],
        group_required: false,
        positional: Some(Positional {
            dest: "vllm_version",
            help: Some("예: 0.21.0"),
        }),
    }
}

fn print(world: &mut TorchWorld<'_>, text: &PyStr) -> Done<()> {
    world.host.out(text)?;
    world.host.out(&PyStr::new("\n"))
}

fn print_error(world: &mut TorchWorld<'_>, text: &PyStr) -> Done<()> {
    world.host.err(text)?;
    world.host.err(&PyStr::new("\n"))
}

/// The common help formatter puts a description paragraph after the usage; this parser has none, so argparse prints the usage, one blank line and
/// the arguments: the four consecutive line breaks after the usage are two.
fn without_extra_blank_line(help: &PyStr) -> PyStr {
    let points = help.code_points();
    let quadruple: Vec<u32> = "\n\n\n\n".chars().map(u32::from).collect();
    match points
        .windows(4)
        .position(|window| window == quadruple.as_slice())
    {
        Some(at) => {
            let mut fixed = points[..at + 2].to_vec();
            fixed.extend_from_slice(&points[at + 4..]);
            PyStr::from_code_points(fixed)
        }
        None => help.clone(),
    }
}

/// The usage and the help of this parser name the required positional `vllm_version`; the recipe package's `cliparse` can only model an optional one
/// (`[vllm_version]`). The brackets are removed from every text it formats; the required-argument error is written here (see [`parse_arguments`]).
fn requiring_positional(text: &PyStr) -> PyStr {
    let from: Vec<u32> = "[vllm_version]".chars().map(u32::from).collect();
    let to: Vec<u32> = "vllm_version".chars().map(u32::from).collect();
    let points = text.code_points();
    let mut fixed = Vec::with_capacity(points.len());
    let mut at = 0;
    while at < points.len() {
        if points[at..].starts_with(&from) {
            fixed.extend_from_slice(&to);
            at += from.len();
        } else {
            fixed.push(points[at]);
            at += 1;
        }
    }
    PyStr::from_code_points(fixed)
}

/// The arguments the parser's own `unrecognized arguments:` message lists (argparse reports a missing required argument before them).
fn unrecognized_in(message: &PyStr) -> Vec<PyStr> {
    let text = message.to_utf8().unwrap_or_default();
    text.split_once("unrecognized arguments: ")
        .map(|(_, list)| {
            list.trim_end()
                .split(' ')
                .map(PyStr::new)
                .collect::<Vec<PyStr>>()
        })
        .unwrap_or_default()
}

/// `ap.parse_args()` over `sys.argv[1:]`: the version, or the stop of help and of a usage error with what argparse wrote.
fn parse_arguments(world: &mut TorchWorld<'_>) -> Flow<PyStr> {
    let program = world
        .argv
        .first()
        .ok_or_else(|| Thrown::builtin("IndexError", "list index out of range"))?;
    let program = basename(program);
    let spec = spec(&program);
    let width = terminal_width(world.columns);
    let usage = requiring_positional(&PyStr::new(&format_usage(&spec, width)));
    let required_error = || {
        PyStr::new(&format!(
            "{program}: error: the following arguments are required: vllm_version\n"
        ))
    };
    let (status, stdout, stderr, head, rest) = match parse(&spec, &world.argv[1..], world.columns) {
        Outcome::Values(values) => match version_of(&values) {
            Some(version) => return Ok(version),
            None => (
                2,
                PyStr::default(),
                PyStr::default(),
                usage.clone(),
                required_error(),
            ),
        },
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            let (head, rest) = split_usage(&usage, &requiring_positional(&stderr));
            // the same arguments without the unrecognized ones: when no version is left, the required-argument error comes first
            let unrecognized = unrecognized_in(&rest);
            let remaining: Vec<PyStr> = world.argv[1..]
                .iter()
                .filter(|argument| !unrecognized.contains(argument))
                .cloned()
                .collect();
            let missing = !unrecognized.is_empty()
                && matches!(parse(&spec, &remaining, world.columns),
                    Outcome::Values(values) if version_of(&values).is_none());
            if missing {
                (
                    2,
                    PyStr::default(),
                    PyStr::default(),
                    usage.clone(),
                    required_error(),
                )
            } else {
                (status, requiring_positional(&stdout), stderr, head, rest)
            }
        }
    };
    let mut message = None;
    if !stdout.is_empty() {
        world.host.parser_help(&without_extra_blank_line(&stdout));
    }
    if !stderr.is_empty() || !rest.is_empty() {
        world.host.parser_error(&head, &rest);
        message = Some(rest);
    }
    // an output write that failed with an exception argparse does not swallow stopped the parse there
    if let Some(mut failure) = world.host.take_parser_failure() {
        if failure.raised.cause.is_none() {
            if let Some(context) = message.as_ref().and_then(argument_error) {
                failure.raised.cause = Some(Box::new(context));
                failure.raised.via_context = true;
            }
        }
        return Err(Stop::Uncaught(failure));
    }
    Err(Stop::Exit(status))
}

fn version_of(values: &BTreeMap<&'static str, Value>) -> Option<PyStr> {
    match values.get("vllm_version") {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

/// `f"https://raw.githubusercontent.com/vllm-project/vllm/{ref.format(v=version)}/pyproject.toml"`.
fn candidate_url(template: &str, version: &PyStr) -> PyStr {
    let mut url = PyStr::new(URL_PREFIX);
    let (before, after) = template.split_once("{v}").expect("every ref names {v}");
    url.push_str(before);
    url.push_py(version);
    url.push_str(after);
    url.push_str(URL_SUFFIX);
    url
}

/// The `{"url": ..., "error": str(e)}` of the last failed ref.
struct LastError {
    url: PyStr,
    error: PyStr,
}

fn refusal(boundary: &'static str, detail: impl Into<String>) -> Stop {
    Stop::Refused(Refusal {
        boundary,
        detail: detail.into(),
    })
}

/// The `for ref in REFS` loop: the first ref whose text is fetched and parsed. `Err(Ok(last))` is "no ref worked" (`data is None`).
fn fetch_pyproject(
    world: &mut TorchWorld<'_>,
    version: &PyStr,
) -> Flow<Result<(TomlValue, PyStr), LastError>> {
    let mut last = None;
    for template in REFS {
        let url = candidate_url(template, version);
        let failed = |error: PyStr, url: &PyStr| LastError {
            url: url.clone(),
            error,
        };
        match (world.fetch)(&url) {
            Ok(text) => {
                let points: Vec<u32> = text.chars().map(u32::from).collect();
                match loads(&points) {
                    Ok(data) => return Ok(Ok((TomlValue::Table(data), url))),
                    Err(LoadsFailure::Raised(raised)) => {
                        last = Some(failed(PyStr::new(&raised.message), &url));
                    }
                    Err(LoadsFailure::Unsupported(domain)) => {
                        return Err(refusal("tomllib", domain.detail()));
                    }
                }
            }
            Err(FetchFailure::Raised(error)) => last = Some(failed(error.message.clone(), &url)),
            Err(FetchFailure::OutOfScope(scope)) => {
                return Err(refusal(
                    "fetch_https",
                    format!("{}: {}", scope.boundary, scope.detail),
                ));
            }
            Err(FetchFailure::Transport(transport)) => {
                return Err(refusal(
                    "fetch_transport",
                    format!(
                        "{} failed: {} (its Python spelling was not captured)",
                        transport.phase, transport.detail
                    ),
                ));
            }
        }
    }
    Ok(Err(last.expect("REFS is not empty")))
}

/// The type name `AttributeError` and `TypeError` use for a parsed value.
fn type_name(value: &TomlValue) -> &'static str {
    match value {
        TomlValue::Bool(_) => "bool",
        TomlValue::Int(_) => "int",
        TomlValue::Float(_) => "float",
        TomlValue::Str(_) => "str",
        TomlValue::Date(_) => "datetime.date",
        TomlValue::Time(_) => "datetime.time",
        TomlValue::DateTime(_) => "datetime.datetime",
        TomlValue::Array(_) => "list",
        TomlValue::Table(_) => "dict",
    }
}

fn uncaught(world: &mut TorchWorld<'_>, frames: &[Site], class: &str, message: &str) -> Stop {
    world.frames = frames.to_vec();
    Stop::Uncaught(Thrown::builtin(class, message))
}

fn key(name: &str) -> Vec<u32> {
    name.chars().map(u32::from).collect()
}

/// `data.get("build-system", {}).get("requires", [])`: unprotected, so a `build-system` that is not a table is the original's `AttributeError`.
fn requires_of(world: &mut TorchWorld<'_>, data: &TomlValue) -> Flow<TomlValue> {
    let TomlValue::Table(table) = data else {
        unreachable!("tomllib.loads returns a dict")
    };
    let empty = TomlValue::Table(Table::new());
    let build_system = table.get(&key("build-system")).unwrap_or(&empty);
    let TomlValue::Table(build_system) = build_system else {
        return Err(uncaught(
            world,
            &[MAIN_BUILD_SYSTEM_GET, MODULE_ENTRY],
            "AttributeError",
            &format!(
                "'{}' object has no attribute 'get'",
                type_name(build_system)
            ),
        ));
    };
    Ok(build_system
        .get(&key("requires"))
        .cloned()
        .unwrap_or(TomlValue::Array(Vec::new())))
}

/// `for r in requires`: a list yields its items, a table its keys, a string its characters; anything else is the original's `TypeError`.
fn iterate(world: &mut TorchWorld<'_>, requires: &TomlValue) -> Flow<Vec<TomlValue>> {
    match requires {
        TomlValue::Array(items) => Ok(items.clone()),
        TomlValue::Table(table) => Ok(table
            .iter()
            .map(|(name, _)| TomlValue::Str(name.clone()))
            .collect()),
        TomlValue::Str(points) => Ok(points.iter().map(|ch| TomlValue::Str(vec![*ch])).collect()),
        other => Err(uncaught(
            world,
            &[MAIN_REQUIRES_LOOP, MODULE_ENTRY],
            "TypeError",
            &format!("'{}' object is not iterable", type_name(other)),
        )),
    }
}

/// The first accepted requirement named `torch` (compared lower-cased); a rejected one is skipped after its warnings were written.
fn first_torch_requirement(
    world: &mut TorchWorld<'_>,
    items: &[TomlValue],
) -> Flow<Option<AcceptedRequirement>> {
    for item in items {
        // a value that is not a str makes the constructor raise `TypeError`, an `Exception` the original skips
        let argument = match item {
            TomlValue::Str(points) => PyValue::Str(PyStr::from_code_points(points.clone())),
            _ => PyValue::None,
        };
        let observation = project_requirement(&argument, world.secret)
            .map_err(|domain| refusal("requirement", domain.detail()))?;
        if !observation.stderr.is_empty() {
            world.host.err(&PyStr::new(&observation.stderr))?;
        }
        let Projection::Accepted(accepted) = observation.projection else {
            continue;
        };
        let name = accepted.name.to_utf8().unwrap_or_default();
        if agent_runtime_support::pylower::python_lower(&name) == "torch" {
            return Ok(Some(accepted));
        }
    }
    Ok(None)
}

// ── JSON as `json.dumps(..., ensure_ascii=False)` writes it ──────────────────────────────────────

const DIGIT_LIMIT: usize = 4300;
const DIGIT_LIMIT_MESSAGE: &str = "Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit";

/// `ensure_ascii=False` string: `"`, `\`, the named escapes and the other controls as `\u00XX`; everything else (a lone surrogate too) is kept.
fn json_string(text: &PyStr, out: &mut PyStr) {
    let mut points: Vec<u32> = vec![u32::from('"')];
    for &point in text.code_points() {
        match char::from_u32(point) {
            Some('"') => points.extend("\\\"".chars().map(u32::from)),
            Some('\\') => points.extend("\\\\".chars().map(u32::from)),
            Some('\n') => points.extend("\\n".chars().map(u32::from)),
            Some('\r') => points.extend("\\r".chars().map(u32::from)),
            Some('\t') => points.extend("\\t".chars().map(u32::from)),
            Some('\u{8}') => points.extend("\\b".chars().map(u32::from)),
            Some('\u{c}') => points.extend("\\f".chars().map(u32::from)),
            _ if point < 0x20 => points.extend(format!("\\u{point:04x}").chars().map(u32::from)),
            _ => points.push(point),
        }
    }
    points.push(u32::from('"'));
    out.push_py(&PyStr::from_code_points(points));
}

enum DumpFailure {
    /// `TypeError` of `JSONEncoder.default` (a date or time).
    Unserializable(&'static str),
    /// `ValueError` of the integer conversion.
    IntegerDigitLimit,
}

/// The compact (`", "` and `": "`) serialization of a parsed value, left to right: the first value that cannot be written is the failure.
fn dump_value(value: &TomlValue, out: &mut PyStr) -> Result<(), DumpFailure> {
    match value {
        TomlValue::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        TomlValue::Int(number) => {
            let digits = number.magnitude().to_str_radix(10);
            if digits.len() > DIGIT_LIMIT {
                return Err(DumpFailure::IntegerDigitLimit);
            }
            out.push_str(&number.to_string());
        }
        TomlValue::Float(number) => out.push_str(&if number.is_nan() {
            "NaN".to_string()
        } else if number.is_infinite() {
            if *number > 0.0 {
                "Infinity"
            } else {
                "-Infinity"
            }
            .to_string()
        } else {
            float_repr(*number)
        }),
        TomlValue::Str(points) => json_string(&PyStr::from_code_points(points.clone()), out),
        TomlValue::Date(_) => return Err(DumpFailure::Unserializable("date")),
        TomlValue::Time(_) => return Err(DumpFailure::Unserializable("time")),
        TomlValue::DateTime(_) => return Err(DumpFailure::Unserializable("datetime")),
        TomlValue::Array(items) => {
            out.push_str("[");
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    out.push_str(", ");
                }
                dump_value(item, out)?;
            }
            out.push_str("]");
        }
        TomlValue::Table(table) => {
            out.push_str("{");
            for (position, (name, item)) in table.iter().enumerate() {
                if position > 0 {
                    out.push_str(", ");
                }
                json_string(&PyStr::from_code_points(name.clone()), out);
                out.push_str(": ");
                dump_value(item, out)?;
            }
            out.push_str("}");
        }
    }
    Ok(())
}

/// `{"url": ..., "error": ...}` and the `{"error": "pyproject.toml fetch/parse 실패", "detail": ...}` around it.
fn fetch_failure_report(last: &LastError) -> PyStr {
    let mut text =
        PyStr::new("{\"error\": \"pyproject.toml fetch/parse 실패\", \"detail\": {\"url\": ");
    json_string(&last.url, &mut text);
    text.push_str(", \"error\": ");
    json_string(&last.error, &mut text);
    text.push_str("}}");
    text
}

fn no_torch_report(world: &mut TorchWorld<'_>, requires: &TomlValue) -> Flow<PyStr> {
    let mut text =
        PyStr::new("{\"error\": \"build-system.requires 에 torch 없음\", \"requires\": ");
    match dump_value(requires, &mut text) {
        Ok(()) => {
            text.push_str("}");
            Ok(text)
        }
        Err(DumpFailure::Unserializable(class)) => Err(uncaught(
            world,
            &json_default_frames(),
            "TypeError",
            &format!("Object of type {class} is not JSON serializable"),
        )),
        Err(DumpFailure::IntegerDigitLimit) => Err(uncaught(
            world,
            &json_encoder_frames(),
            "ValueError",
            DIGIT_LIMIT_MESSAGE,
        )),
    }
}

/// The output object, in the original's key order, written with `indent=2`.
enum Json {
    Null,
    Text(PyStr),
    Object(Vec<(&'static str, Json)>),
}

impl Json {
    fn optional(text: Option<PyStr>) -> Json {
        text.map_or(Json::Null, Json::Text)
    }

    fn write(&self, depth: usize, out: &mut PyStr) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Text(text) => json_string(text, out),
            Json::Object(fields) => {
                out.push_str("{");
                let inner = "  ".repeat(depth + 1);
                for (position, (name, value)) in fields.iter().enumerate() {
                    out.push_str(if position == 0 { "\n" } else { ",\n" });
                    out.push_str(&inner);
                    json_string(&PyStr::new(name), out);
                    out.push_str(": ");
                    value.write(depth + 1, out);
                }
                out.push_str("\n");
                out.push_str(&"  ".repeat(depth));
                out.push_str("}");
            }
        }
    }
}

fn thrown_from_pin(raised: PinRaised) -> Stop {
    let row = raised
        .rows
        .into_iter()
        .next()
        .expect("a raised exception has a row");
    let mut error = Raised::new(&row.class, &row.module, row.message);
    error.code = None;
    // an exception raised inside `packaging.version` passes through frames this port has no metadata for: it is reported by its text alone
    Stop::Uncaught(Thrown::plain(error))
}

fn text_of(value: PyValue) -> Option<PyStr> {
    match value {
        PyValue::Str(text) => Some(text),
        _ => None,
    }
}

/// The version list of `pick_bound` as the original passes it.
fn bound(versions: &[PyStr], newest: bool) -> Flow<Option<PyStr>> {
    let mut keywords = PyDict::new();
    keywords.set("newest", PyValue::Bool(newest));
    let list = PyValue::List(versions.iter().cloned().map(PyValue::Str).collect());
    pick_bound(&[list], &keywords)
        .map(|returned| text_of(returned.value))
        .map_err(thrown_from_pin)
}

/// `torch_prefix`/`range_bounds` and the rest of the output for the chosen requirement.
fn output_of(
    world: &mut TorchWorld<'_>,
    version: &PyStr,
    used_url: PyStr,
    requirement: &AcceptedRequirement,
) -> Flow<Json> {
    let mut pin: Option<&PyStr> = None;
    for (operator, specifier_version) in &requirement.specifiers {
        if operator.to_utf8().as_deref() == Some("==")
            || operator.to_utf8().as_deref() == Some("===")
        {
            pin = Some(specifier_version);
        }
    }
    let _ = world;
    // `norm_prefix(pin) if pin else None`: an empty pin is falsy but not `None`
    let prefix = match pin.filter(|pin| !pin.is_empty()) {
        Some(pin) => {
            let returned = norm_prefix(&[PyValue::Str(pin.clone())], &PyDict::new())
                .map_err(thrown_from_pin)?;
            text_of(returned.value)
        }
        None => None,
    };
    let mut fields = vec![
        ("vllm_version", Json::Text(version.clone())),
        ("torch_spec", Json::Text(requirement.specifier_text.clone())),
        ("torch_pin", Json::optional(pin.cloned())),
        ("torch_prefix", Json::optional(prefix)),
        ("source_url", Json::Text(used_url)),
    ];
    if pin.is_none() {
        let versions_with = |operators: &[&str]| -> Vec<PyStr> {
            requirement
                .specifiers
                .iter()
                .filter(|(operator, _)| {
                    operator
                        .to_utf8()
                        .is_some_and(|text| operators.contains(&text.as_str()))
                })
                .map(|(_, specifier_version)| specifier_version.clone())
                .collect()
        };
        let lowers = versions_with(&[">=", ">", "~="]);
        let uppers = versions_with(&["<=", "<"]);
        fields.push((
            "torch_specifier",
            Json::Text(requirement.specifier_text.clone()),
        ));
        let lower = bound(&lowers, true)?;
        let upper = bound(&uppers, false)?;
        fields.push((
            "range_bounds",
            Json::Object(vec![
                ("lower", Json::optional(lower)),
                ("upper", Json::optional(upper)),
            ]),
        ));
        fields.push((
            "warning",
            Json::Text(PyStr::new(
                "범위 스펙 감지 — 정확 핀 부재. 하한/상한을 NGC 매칭 후보로 제시(범위 교차), null 전파 아님",
            )),
        ));
    }
    Ok(Json::Object(fields))
}

/// `main()` over `sys.argv`.
pub fn main(world: &mut TorchWorld<'_>) -> Flow<()> {
    let version = parse_arguments(world)?;
    let (data, used_url) = match fetch_pyproject(world, &version)? {
        Ok(found) => found,
        Err(last) => {
            print_error(world, &fetch_failure_report(&last))?;
            return Err(Stop::Exit(2));
        }
    };
    let requires = requires_of(world, &data)?;
    let items = iterate(world, &requires)?;
    let Some(requirement) = first_torch_requirement(world, &items)? else {
        let report = no_torch_report(world, &requires)?;
        print_error(world, &report)?;
        return Err(Stop::Exit(3));
    };
    let output = output_of(world, &version, used_url, &requirement)?;
    let mut text = PyStr::default();
    output.write(0, &mut text);
    print(world, &text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tomllib::CivilDate;
    use num_bigint::BigInt;

    fn dumped(value: &TomlValue) -> Result<String, &'static str> {
        let mut out = PyStr::default();
        match dump_value(value, &mut out) {
            Ok(()) => Ok(out.to_utf8().expect("text")),
            Err(DumpFailure::Unserializable(class)) => Err(class),
            Err(DumpFailure::IntegerDigitLimit) => Err("digit limit"),
        }
    }

    #[test]
    fn json_text_keeps_non_ascii_and_escapes_controls() {
        let mut out = PyStr::default();
        json_string(&PyStr::new("a\"\\\n\u{1}에"), &mut out);
        assert_eq!(out.to_utf8().unwrap(), "\"a\\\"\\\\\\n\\u0001에\"");
    }

    #[test]
    fn values_are_dumped_left_to_right_and_the_first_unserializable_one_fails() {
        let list = TomlValue::Array(vec![
            TomlValue::Int(BigInt::from(1)),
            TomlValue::Float(f64::NAN),
            TomlValue::Float(f64::NEG_INFINITY),
            TomlValue::Bool(true),
            TomlValue::Date(CivilDate::validated(1979, 5, 27).unwrap()),
            TomlValue::Int(BigInt::from(10).pow(5000)),
        ]);
        assert_eq!(dumped(&list), Err("date"));
        let ok = TomlValue::Array(vec![
            TomlValue::Float(f64::NAN),
            TomlValue::Float(f64::NEG_INFINITY),
            TomlValue::Float(1.0),
            TomlValue::Str(vec![0x61]),
        ]);
        assert_eq!(dumped(&ok).unwrap(), "[NaN, -Infinity, 1.0, \"a\"]");
        assert_eq!(
            dumped(&TomlValue::Int(BigInt::from(10).pow(5000))),
            Err("digit limit")
        );
        assert!(dumped(&TomlValue::Int(BigInt::from(10).pow(4299))).is_ok());
    }

    #[test]
    fn the_required_positional_loses_its_brackets_everywhere_it_is_formatted() {
        let text = requiring_positional(&PyStr::new("usage: p [-h] [vllm_version]\n"));
        assert_eq!(text.to_utf8().unwrap(), "usage: p [-h] vllm_version\n");
    }

    #[test]
    fn a_candidate_url_inserts_the_version_into_the_ref() {
        let url = candidate_url(REFS[1], &PyStr::new("0.21.0"));
        assert_eq!(
            url.to_utf8().unwrap(),
            "https://raw.githubusercontent.com/vllm-project/vllm/releases/v0.21.0/pyproject.toml"
        );
    }
}
