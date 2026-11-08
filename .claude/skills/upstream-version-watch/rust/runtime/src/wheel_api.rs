//! `fetch_json` and `main` of `resolve_wheel.py` as a JSON-lines service (the ABI is `ABI.md` of the R38 work): one request per line, one record per
//! request, in the typed-value conventions of the earlier owner services (the collision-free DTO of [`crate::dto`]).
//!
//! **Request** `{schema?, id, operation, args_dto?, kwargs_dto?, globals?}`: `operation` is `resolve_wheel.fetch_json` or `resolve_wheel.main`; the
//! arguments are bound as CPython binds them (so a wrong count or keyword is the interpreter's `TypeError`, raised by the call); `globals` may declare
//! `argv` (the list `main` parses; it defaults to `["resolve_wheel.py"]`) and `API`, which must equal the source's constant (it is never replaced).
//!
//! **Record** `{schema, id, kind, ...}` where `kind` is `value` (`value` DTO, `return_aliases` always empty: neither function returns an object it
//! was given), `exception` (`exception`: the typed exception graph below), `out_of_scope` (a named boundary of this component), `transport_failure`
//! (a socket failure whose Python spelling was not captured) or `request_error` (the call was not made). Every record carries `phase`,
//! `owner_call_attempted`, `args_after`, `kwargs_after`, `globals_after` and the business `stdout` / `stderr` (what `main` wrote; empty for `fetch_json`).
//!
//! **Exception graph** (the schema of the saved originals): `{type, module, message, args, suppress_context, relation, attributes, frames, constructor?,
//! http_headers?, next?}`. `args` and attribute values are DTOs, or `["exception", row]` for a wrapped exception; `attributes` lists the captured
//! names in the original order; `relation` is `cause`, `context` or null and `next` carries the linked exception; `http_headers` is the ordered header
//! list of an `HTTPError`. `frames` is always empty: interpreter stack frames inside the original's collector are auxiliary, not business values.

use crate::dto::{dict_of, encode};
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::http_direct::failure::ErrorValue;
use crate::http_direct::{Link, PyException};
use crate::ngc_cli::machine_name;
use crate::runtime_patch::StdioProfile;
use crate::thrown::Done;
use crate::wheel_fetch::{fetch_json_process, FetchError};
use crate::wheel_main::{main, ReleaseFetcher, Stop, WheelWorld, PROGRAM};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub const SCHEMA: &str = "resolve_wheel-api/1";
pub const MODULE: &str = "resolve_wheel";
/// The module constant as the original spells it (a `str.format` template).
pub const API_TEMPLATE: &str = "https://api.github.com/repos/vllm-project/vllm/releases/tags/v{v}";

/// The attributes an exception row lists, in the order the original's observer reads them.
const ATTRIBUTE_ORDER: [&str; 34] = [
    "name",
    "obj",
    "path",
    "errno",
    "strerror",
    "filename",
    "filename2",
    "code",
    "message",
    "field",
    "url",
    "reason",
    "line",
    "partial",
    "expected",
    "encoding",
    "object",
    "start",
    "end",
    "msg",
    "doc",
    "pattern",
    "pos",
    "lineno",
    "colno",
    "verify_code",
    "verify_message",
    "library",
    "content",
    "context",
    "context_mark",
    "problem",
    "problem_mark",
    "note",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    FetchJson,
    Main,
}

impl Operation {
    fn named(name: &str) -> Option<Operation> {
        match name {
            "fetch_json" => Some(Operation::FetchJson),
            "main" => Some(Operation::Main),
            _ => None,
        }
    }

    fn signature(self) -> Signature {
        const URL: &[Param] = &[required("url")];
        const NOTHING: &[Param] = &[];
        match self {
            Operation::FetchJson => Signature {
                name: "fetch_json",
                params: URL,
                rest: false,
            },
            Operation::Main => Signature {
                name: "main",
                params: NOTHING,
                rest: false,
            },
        }
    }
}

/// `resolve_wheel.function`: the operation, or why the request cannot be answered.
pub fn resolve_operation(operation: &str) -> Result<Operation, String> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(format!("No module named '{module}'"));
    }
    Operation::named(name).ok_or_else(|| format!("module '{module}' has no attribute '{name}'"))
}

fn plain_dto(value: &PyValue) -> PyValue {
    encode(value).unwrap_or(PyValue::None)
}

fn error_value(value: &ErrorValue, from_none: bool) -> PyValue {
    match value {
        ErrorValue::Plain(plain) => plain_dto(plain),
        ErrorValue::Exception(error) => PyValue::List(vec![
            PyValue::str("exception"),
            PyValue::Dict(exception_row(error, from_none)),
        ]),
        ErrorValue::HttpMessage(items) => crate::http_direct::failure::http_message_dto(items),
        ErrorValue::YamlMark(mark) => crate::http_direct::failure::yaml_mark_dto(mark),
        ErrorValue::Tuple(members) => PyValue::List(vec![
            PyValue::str("tuple"),
            PyValue::List(
                members
                    .iter()
                    .map(|member| error_value(member, from_none))
                    .collect(),
            ),
        ]),
    }
}

/// The attributes of `error` the observer lists, with the two an `HTTPError` has through its file-object base (`name`, and `msg` = its reason).
fn captured_attributes(error: &PyException) -> Vec<(&'static str, ErrorValue)> {
    let mut attributes: Vec<(&'static str, ErrorValue)> = error.attributes.clone();
    if error.class == "HTTPError" {
        attributes.push(("name", PyValue::str("<urllib response>").into()));
        if let Some((_, reason)) = error.attributes.iter().find(|(name, _)| *name == "reason") {
            attributes.push(("msg", reason.clone()));
        }
    }
    attributes
}

fn attribute_of<'a>(error: &'a PyException, name: &str) -> Option<&'a ErrorValue> {
    error
        .attributes
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, value)| value)
}

/// The `constructor` member of the classes whose constructor needs more than the message.
fn constructor_of(error: &PyException) -> Option<PyValue> {
    let fields = |names: &[&str]| -> Option<PyValue> {
        let mut row = PyDict::new();
        for name in names {
            row.set(name, error_value(attribute_of(error, name)?, false));
        }
        Some(PyValue::Dict(row))
    };
    let kind = PyValue::str(error.class);
    match error.class {
        "SystemExit" => Some(dict_of(vec![
            ("kind", kind),
            (
                "args",
                PyValue::List(
                    error
                        .args
                        .iter()
                        .map(|value| error_value(value, false))
                        .collect(),
                ),
            ),
            (
                "code",
                attribute_of(error, "code")
                    .map_or(PyValue::None, |value| error_value(value, false)),
            ),
        ])),
        "UnicodeDecodeError" | "UnicodeEncodeError" => Some(dict_of(vec![
            ("kind", kind),
            (
                "fields",
                fields(&["encoding", "object", "start", "end", "reason"])?,
            ),
        ])),
        "error" if error.module == "re" => Some(dict_of(vec![
            ("kind", kind),
            ("module", PyValue::str("re")),
            ("fields", fields(&["msg", "pattern", "pos"])?),
        ])),
        "ContractViolation" => Some(dict_of(vec![
            ("kind", kind),
            ("module", PyValue::str(error.module)),
            ("fields", fields(&["code", "message", "field"])?),
        ])),
        "ScannerError" | "ParserError" | "ComposerError" | "ConstructorError" => {
            Some(dict_of(vec![
                ("kind", kind),
                ("module", PyValue::str(error.module)),
                (
                    "fields",
                    fields(&["context", "context_mark", "problem", "problem_mark", "note"])?,
                ),
            ]))
        }
        "JSONDecodeError" => Some(dict_of(vec![
            ("kind", kind),
            ("fields", fields(&["msg", "doc", "pos"])?),
        ])),
        _ => None,
    }
}

/// One exception of the graph; `from_none` marks an exception raised `from None` (its context is suppressed and absent).
pub fn exception_row(error: &PyException, from_none: bool) -> PyDict {
    let (relation, next) = match &error.link {
        Some(Link::Cause(next)) => (PyValue::str("cause"), Some(next)),
        Some(Link::Context(next)) => (PyValue::str("context"), Some(next)),
        None => (PyValue::None, None),
    };
    let mut row = PyDict::new();
    row.set("type", PyValue::str(error.class));
    row.set("module", PyValue::str(error.module));
    row.set("message", PyValue::Str(error.message.clone()));
    row.set(
        "args",
        PyValue::List(
            error
                .args
                .iter()
                .map(|value| error_value(value, false))
                .collect(),
        ),
    );
    row.set(
        "suppress_context",
        PyValue::Bool(from_none || matches!(error.link, Some(Link::Cause(_)))),
    );
    row.set("relation", relation);
    let captured = captured_attributes(error);
    let mut attributes = PyDict::new();
    for name in ATTRIBUTE_ORDER {
        if let Some((_, value)) = captured.iter().find(|(known, _)| *known == name) {
            attributes.set(name, error_value(value, false));
        }
    }
    row.set("attributes", PyValue::Dict(attributes));
    row.set("frames", PyValue::List(Vec::new()));
    if let Some(constructor) = constructor_of(error) {
        row.set("constructor", constructor);
    }
    if let Some(headers) = &error.http_headers {
        let pairs = headers
            .iter()
            .map(|(name, value)| PyValue::List(vec![PyValue::str(name), PyValue::str(value)]))
            .collect();
        row.set("http_headers", PyValue::List(pairs));
    }
    if let Some(next) = next {
        row.set("next", PyValue::Dict(exception_row(next, false)));
    }
    row
}

/// `SystemExit(code)` as `sys.exit(code)` and argparse raise it.
fn system_exit(code: i32) -> PyException {
    PyException {
        class: "SystemExit",
        module: "builtins",
        message: PyStr::new(&code.to_string()),
        args: vec![PyValue::int(code).into()],
        attributes: vec![("code", PyValue::int(code).into())],
        http_headers: None,
        link: None,
    }
}

fn binding_error(failure: Failure) -> PyException {
    PyException {
        class: failure.class,
        module: "builtins",
        message: failure.message.clone(),
        args: vec![PyValue::Str(failure.message).into()],
        attributes: Vec::new(),
        http_headers: None,
        link: None,
    }
}

/// How a call ended, before it is written as a record.
enum Ended {
    Value(PyValue),
    Exception {
        error: Box<PyException>,
        from_none: bool,
    },
    OutOfScope {
        boundary: String,
        detail: String,
    },
    Transport {
        phase: String,
        detail: String,
    },
}

impl From<FetchError> for Ended {
    fn from(error: FetchError) -> Ended {
        match error {
            FetchError::Raised { error, origin } => Ended::Exception {
                error,
                from_none: origin.suppresses_context(),
            },
            FetchError::OutOfScope(refusal) => Ended::OutOfScope {
                boundary: refusal.boundary.to_string(),
                detail: refusal.detail,
            },
            FetchError::Transport(transport) => Ended::Transport {
                phase: transport.phase.to_string(),
                detail: transport.detail,
            },
        }
    }
}

/// What `main` wrote and how it ended.
struct Streams {
    stdout: PyStr,
    stderr: PyStr,
}

/// A console that keeps what `main` writes; no write of it fails.
struct Capture {
    streams: Streams,
}

impl crate::host::Sink for Capture {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.streams.stdout.push_py(text);
        Ok(())
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        self.streams.stderr.push_py(text);
        Ok(())
    }

    fn parser_help(&mut self, help: &PyStr) {
        self.streams.stdout.push_py(help);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        self.streams.stderr.push_py(usage);
        self.streams.stderr.push_py(message);
    }
}

/// `sys.argv` as a request declares it.
struct Globals {
    argv: Vec<PyStr>,
}

fn read_globals(given: Option<&PyValue>) -> Result<Globals, String> {
    let mut globals = Globals {
        argv: vec![PyStr::new(PROGRAM)],
    };
    let Some(given) = given else {
        return Ok(globals);
    };
    let PyValue::Dict(members) = given else {
        return Err("globals must be an object".to_string());
    };
    for (key, value) in members.iter() {
        if key.equals("argv") {
            let PyValue::List(items) = value else {
                return Err("argv must be an array of strings".to_string());
            };
            globals.argv = items
                .iter()
                .map(|item| match item {
                    PyValue::Str(text) => Ok(text.clone()),
                    _ => Err("argv must be an array of strings".to_string()),
                })
                .collect::<Result<_, _>>()?;
        } else if key.equals("API") {
            let canonical = encode(&PyValue::str(API_TEMPLATE))?;
            if *value != canonical {
                return Err(
                    "API is the source's constant (a str DTO): no other value is modelled"
                        .to_string(),
                );
            }
        } else {
            return Err(format!("globals holds argv and API only: {}", key.repr()));
        }
    }
    Ok(globals)
}

fn request_error(id: PyValue, message: &str) -> PyValue {
    let mut row = PyDict::new();
    row.set("schema", PyValue::str(SCHEMA));
    row.set("id", id);
    row.set("kind", PyValue::str("request_error"));
    row.set("message", PyValue::str(message));
    row.set("phase", PyValue::str("request"));
    row.set("owner_call_attempted", PyValue::Bool(false));
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(""));
    PyValue::Dict(row)
}

fn finish_record(
    id: PyValue,
    ended: Ended,
    streams: Streams,
    args: &[PyValue],
    kwargs: &PyDict,
    globals: &Globals,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("schema", PyValue::str(SCHEMA));
    row.set("id", id);
    match ended {
        Ended::Value(value) => match encode(&value) {
            Ok(shown) => {
                row.set("kind", PyValue::str("value"));
                row.set("value", shown);
                row.set("return_aliases", PyValue::List(Vec::new()));
            }
            Err(refusal) => {
                row.set("kind", PyValue::str("exception"));
                let message = PyException {
                    class: "TypeError",
                    module: "builtins",
                    message: PyStr::new(&refusal),
                    args: vec![PyValue::str(&refusal).into()],
                    attributes: Vec::new(),
                    http_headers: None,
                    link: None,
                };
                row.set("exception", PyValue::Dict(exception_row(&message, false)));
            }
        },
        Ended::Exception { error, from_none } => {
            row.set("kind", PyValue::str("exception"));
            row.set("exception", PyValue::Dict(exception_row(&error, from_none)));
        }
        Ended::OutOfScope { boundary, detail } => {
            row.set("kind", PyValue::str("out_of_scope"));
            row.set("boundary", PyValue::str(&boundary));
            row.set("detail", PyValue::str(&detail));
        }
        Ended::Transport { phase, detail } => {
            row.set("kind", PyValue::str("transport_failure"));
            row.set("phase", PyValue::str(&phase));
            row.set("detail", PyValue::str(&detail));
        }
    }
    if !row.contains_key("phase") {
        row.set("phase", PyValue::str("direct_call_attempt"));
    }
    row.set("owner_call_attempted", PyValue::Bool(true));
    row.set("args_after", plain_dto(&PyValue::List(args.to_vec())));
    row.set("kwargs_after", plain_dto(&PyValue::Dict(kwargs.clone())));
    row.set(
        "globals_after",
        dict_of(vec![
            (
                "argv",
                PyValue::List(globals.argv.iter().cloned().map(PyValue::Str).collect()),
            ),
            ("API", plain_dto(&PyValue::str(API_TEMPLATE))),
        ]),
    );
    row.set("stdout", PyValue::Str(streams.stdout));
    row.set("stderr", PyValue::Str(streams.stderr));
    PyValue::Dict(row)
}

fn run_main(
    globals: &Globals,
    columns: Option<&str>,
    fetch: ReleaseFetcher<'_>,
) -> (Ended, Streams) {
    let mut capture = Capture {
        streams: Streams {
            stdout: PyStr::default(),
            stderr: PyStr::default(),
        },
    };
    let ended = {
        let mut world = WheelWorld {
            host: &mut capture,
            argv: &globals.argv,
            columns,
            profile: StdioProfile::Default,
            machine: machine_name(),
            fetch,
        };
        match main(&mut world) {
            Ok(()) => Ended::Value(PyValue::None),
            Err(Stop::Exit(code)) => Ended::Exception {
                error: Box::new(system_exit(code)),
                from_none: false,
            },
            Err(Stop::Raised { error, trace }) => Ended::Exception {
                error,
                from_none: trace.suppresses_context(),
            },
            Err(Stop::Rendered(text)) => Ended::OutOfScope {
                boundary: "rendered_report".to_string(),
                detail: text.to_utf8().unwrap_or_default(),
            },
            Err(Stop::Refused(refusal)) => Ended::OutOfScope {
                boundary: refusal.boundary.to_string(),
                detail: refusal.detail,
            },
        }
    };
    (ended, capture.streams)
}

/// A request line that passed the shape checks.
struct Request {
    id: PyValue,
    members: PyDict,
    args: Vec<PyValue>,
    kwargs: PyDict,
}

const REQUEST_MEMBERS: [&str; 6] = [
    "schema",
    "id",
    "operation",
    "args_dto",
    "kwargs_dto",
    "globals",
];

/// Reads one request line; `Err` is the finished record to answer with (the call is not made).
fn parse_line(line: &PyStr) -> Result<Request, PyValue> {
    let parsed = loads_text_objects(line)
        .map_err(|error| request_error(PyValue::None, &format!("not JSON: {error}")))?;
    let PyValue::Dict(members) = parsed else {
        return Err(request_error(PyValue::None, "the request is not an object"));
    };
    let Some(id) = members.get("id").cloned() else {
        return Err(request_error(PyValue::None, "the request has no id"));
    };
    if let Some((key, _)) = members
        .iter()
        .find(|(key, _)| !REQUEST_MEMBERS.iter().any(|known| key.equals(known)))
    {
        return Err(request_error(
            id,
            &format!(
                "the request has a member this service does not take: {}",
                key.repr()
            ),
        ));
    }
    let args = match members.get("args_dto") {
        None => Vec::new(),
        Some(dto) => decode_args(dto)
            .map_err(|problem| request_error(id.clone(), &format!("args_dto: {problem}")))?,
    };
    let kwargs = match members.get("kwargs_dto") {
        None => PyDict::new(),
        Some(dto) => decode_kwargs(dto)
            .map_err(|problem| request_error(id.clone(), &format!("kwargs_dto: {problem}")))?,
    };
    Ok(Request {
        id,
        members,
        args,
        kwargs,
    })
}

/// Answers one request line with `fetch` as the release lookup.
pub fn answer_with(line: &PyStr, columns: Option<&str>, fetch: ReleaseFetcher<'_>) -> PyValue {
    let request = match parse_line(line) {
        Ok(request) => request,
        Err(row) => return row,
    };
    if let Some(schema) = request.members.get("schema") {
        if *schema != PyValue::str(SCHEMA) {
            return request_error(request.id, &format!("schema must be '{SCHEMA}'"));
        }
    }
    let Some(PyValue::Str(operation)) = request.members.get("operation") else {
        return request_error(request.id, "operation must be a string 'module.function'");
    };
    let operation = match resolve_operation(&operation.to_utf8().unwrap_or_default()) {
        Ok(operation) => operation,
        Err(message) => return request_error(request.id, &message),
    };
    let globals = match read_globals(request.members.get("globals")) {
        Ok(globals) => globals,
        Err(message) => return request_error(request.id, &message),
    };
    let signature = operation.signature();
    let (ended, streams) = match signature.bind(&request.args, &request.kwargs) {
        Err(failure) => (
            Ended::Exception {
                error: Box::new(binding_error(failure)),
                from_none: false,
            },
            Streams {
                stdout: PyStr::default(),
                stderr: PyStr::default(),
            },
        ),
        Ok(bound) => match operation {
            Operation::FetchJson => {
                let ended = match fetch(&bound[0]) {
                    Ok(value) => Ended::Value(value),
                    Err(error) => Ended::from(error),
                };
                (
                    ended,
                    Streams {
                        stdout: PyStr::default(),
                        stderr: PyStr::default(),
                    },
                )
            }
            Operation::Main => run_main(&globals, columns, fetch),
        },
    };
    finish_record(
        request.id,
        ended,
        streams,
        &request.args,
        &request.kwargs,
        &globals,
    )
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each, over real connections.
pub fn evaluate_jsonl(input: &[u8], columns: Option<&str>) -> Result<Vec<u8>, Failure> {
    evaluate_with(input, columns, &fetch_json_process)
}

/// [`evaluate_jsonl`] over any release lookup (the checks' stand-in, or the real one).
pub fn evaluate_with(
    input: &[u8],
    columns: Option<&str>,
    fetch: ReleaseFetcher<'_>,
) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(crate::api::render(&answer_with(&line, columns, fetch))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wheel_fetch::FetchError;

    fn answer_record(request: &str, fetch: ReleaseFetcher<'_>) -> PyDict {
        match answer_with(&PyStr::new(request), Some("80"), fetch) {
            PyValue::Dict(record) => record,
            other => panic!("a record object, not {other:?}"),
        }
    }

    fn kind(record: &PyDict) -> String {
        match record.get("kind") {
            Some(PyValue::Str(text)) => text.to_utf8().unwrap(),
            other => panic!("no kind: {other:?}"),
        }
    }

    fn exception_of(record: &PyDict) -> &PyDict {
        match record.get("exception") {
            Some(PyValue::Dict(row)) => row,
            other => panic!("no exception: {other:?}"),
        }
    }

    fn message_of(row: &PyDict) -> String {
        match row.get("message") {
            Some(PyValue::Str(text)) => text.to_utf8().unwrap(),
            other => panic!("no message: {other:?}"),
        }
    }

    fn unreachable_fetch(_: &PyValue) -> Result<PyValue, FetchError> {
        panic!("no fetch expected")
    }

    #[test]
    fn a_binding_error_is_the_interpreters_type_error_of_the_call() {
        let line = r#"{"id":"a","operation":"resolve_wheel.fetch_json","args_dto":["list",[["str","u"],["none"]]]}"#;
        let record = answer_record(line, &unreachable_fetch);
        assert_eq!(kind(&record), "exception");
        assert_eq!(
            message_of(exception_of(&record)),
            "fetch_json() takes 1 positional argument but 2 were given"
        );
        let line = r#"{"id":"b","operation":"resolve_wheel.fetch_json","args_dto":["list",[["str","u"]]],"kwargs_dto":["dict",[[["str","unexpected"],["none"]]]]}"#;
        let record = answer_record(line, &unreachable_fetch);
        assert_eq!(
            message_of(exception_of(&record)),
            "fetch_json() got an unexpected keyword argument 'unexpected'"
        );
        let record = answer_record(
            r#"{"id":"c","operation":"resolve_wheel.fetch_json"}"#,
            &unreachable_fetch,
        );
        assert_eq!(
            message_of(exception_of(&record)),
            "fetch_json() missing 1 required positional argument: 'url'"
        );
    }

    #[test]
    fn a_keyword_url_reaches_the_fetch_and_the_value_comes_back_typed() {
        let seen = std::cell::RefCell::new(None);
        let fetch = |url: &PyValue| -> Result<PyValue, FetchError> {
            *seen.borrow_mut() = Some(url.clone());
            let mut object = PyDict::new();
            object.set("x", PyValue::int(1));
            Ok(PyValue::Dict(object))
        };
        let record = answer_record(
            r#"{"id":7,"operation":"resolve_wheel.fetch_json","kwargs_dto":["dict",[[["str","url"],["str","u"]]]]}"#,
            &fetch,
        );
        assert_eq!(kind(&record), "value");
        assert_eq!(seen.borrow().clone(), Some(PyValue::str("u")));
        assert_eq!(
            record.get("value"),
            Some(&plain_dto(&{
                let mut object = PyDict::new();
                object.set("x", PyValue::int(1));
                PyValue::Dict(object)
            }))
        );
        assert_eq!(record.get("stdout"), Some(&PyValue::str("")));
    }

    #[test]
    fn main_runs_over_the_declared_argv_and_keeps_its_streams_apart() {
        let release = |_: &PyValue| -> Result<PyValue, FetchError> {
            let mut asset = PyDict::new();
            asset.set(
                "name",
                PyValue::str("vllm-0.32.0+cu129-cp38-abi3-manylinux_2_34_aarch64.whl"),
            );
            let mut document = PyDict::new();
            document.set("assets", PyValue::List(vec![PyValue::Dict(asset)]));
            Ok(PyValue::Dict(document))
        };
        let line = r#"{"id":1,"operation":"resolve_wheel.main","globals":{"argv":["resolve_wheel.py","0.32.0","--arch","aarch64"]}}"#;
        let record = answer_record(line, &release);
        assert_eq!(kind(&record), "value");
        assert_eq!(
            record.get("value"),
            Some(&PyValue::List(vec![PyValue::str("none")]))
        );
        let Some(PyValue::Str(stdout)) = record.get("stdout") else {
            panic!("stdout")
        };
        assert!(stdout.to_utf8().unwrap().contains(
            "\"asset_name\": \"vllm-0.32.0+cu129-cp38-abi3-manylinux_2_34_aarch64.whl\""
        ));
        assert_eq!(record.get("stderr"), Some(&PyValue::str("")));
    }

    #[test]
    fn sys_exit_is_a_system_exit_with_its_constructor_and_a_changed_api_constant_is_refused() {
        let empty =
            |_: &PyValue| -> Result<PyValue, FetchError> { Ok(PyValue::Dict(PyDict::new())) };
        let line = r#"{"id":1,"operation":"resolve_wheel.main","globals":{"argv":["resolve_wheel.py","0.32.0","--arch","aarch64"]}}"#;
        let record = answer_record(line, &empty);
        let row = exception_of(&record);
        assert_eq!(row.get("type"), Some(&PyValue::str("SystemExit")));
        let Some(PyValue::Dict(constructor)) = row.get("constructor") else {
            panic!("constructor")
        };
        assert_eq!(constructor.get("kind"), Some(&PyValue::str("SystemExit")));
        assert_eq!(constructor.get("code"), Some(&plain_dto(&PyValue::int(4))));
        let Some(PyValue::Str(stderr)) = record.get("stderr") else {
            panic!("stderr")
        };
        assert!(stderr.to_utf8().unwrap().contains("\"assets\": []"));
        let altered = r#"{"id":2,"operation":"resolve_wheel.main","globals":{"API":["str","https://elsewhere/{v}"]}}"#;
        assert_eq!(
            kind(&answer_record(altered, &unreachable_fetch)),
            "request_error"
        );
        let unknown = r#"{"id":3,"operation":"resolve_wheel.other"}"#;
        assert_eq!(
            kind(&answer_record(unknown, &unreachable_fetch)),
            "request_error"
        );
    }

    #[test]
    fn a_json_error_raised_from_none_is_marked_suppressed_with_its_constructor_fields() {
        let doc = PyStr::new("{\"x\":[");
        let error = crate::wheel_json::json_decode_error("Expecting value", &doc, 6, 1, 7);
        let row = exception_row(&error, true);
        assert_eq!(row.get("suppress_context"), Some(&PyValue::Bool(true)));
        assert_eq!(row.get("relation"), Some(&PyValue::None));
        assert!(row.get("next").is_none());
        let Some(PyValue::Dict(constructor)) = row.get("constructor") else {
            panic!("constructor")
        };
        assert!(constructor.get("fields").is_some());
        let Some(PyValue::Dict(attributes)) = row.get("attributes") else {
            panic!("attributes")
        };
        let order: Vec<String> = attributes
            .iter()
            .map(|(name, _)| name.to_utf8().unwrap())
            .collect();
        assert_eq!(order, ["msg", "doc", "pos", "lineno", "colno"]);
    }

    /// The release lookup of a run whose response body is `body`: the real byte decoding and scanning of `fetch_json`, failure mapping included.
    fn lookup_of_body(body: &'static [u8]) -> impl Fn(&PyValue) -> Result<PyValue, FetchError> {
        move |_| crate::wheel_json::loads_bytes(body).map_err(FetchError::from)
    }

    const MAIN_REQUEST: &str = r#"{"id":1,"operation":"resolve_wheel.main","globals":{"argv":["resolve_wheel.py","0.32.0","--arch","aarch64"]}}"#;
    const FETCH_REQUEST: &str =
        r#"{"id":1,"operation":"resolve_wheel.fetch_json","args_dto":["list",[["str","u"]]]}"#;

    fn suppress_context_of(record: &PyDict) -> PyValue {
        exception_of(record)
            .get("suppress_context")
            .cloned()
            .expect("suppress_context")
    }

    #[test]
    fn main_reports_the_context_suppression_of_a_json_error_exactly_as_fetch_json_does() {
        // `raise JSONDecodeError("Expecting value", ...) from None` (every "Expecting value"): context suppressed, no linked exception
        let lookup = lookup_of_body(b"{\"assets\":[");
        let through_main = answer_record(MAIN_REQUEST, &lookup);
        let through_fetch = answer_record(FETCH_REQUEST, &lookup);
        let row = exception_of(&through_main);
        assert_eq!(row.get("type"), Some(&PyValue::str("JSONDecodeError")));
        assert_eq!(suppress_context_of(&through_main), PyValue::Bool(true));
        assert_eq!(row.get("relation"), Some(&PyValue::None));
        assert!(row.get("next").is_none());
        // the same owner failure reaches the same public graph through both public functions
        assert_eq!(row, exception_of(&through_fetch));
    }

    #[test]
    fn main_does_not_suppress_the_context_of_json_errors_the_interpreter_raises_without_from_none()
    {
        for body in [&b"{\"a\" 1}"[..], b"{} x"] {
            let lookup = lookup_of_body(body.to_vec().leak());
            let through_main = answer_record(MAIN_REQUEST, &lookup);
            assert_eq!(suppress_context_of(&through_main), PyValue::Bool(false));
            assert_eq!(
                exception_of(&through_main),
                exception_of(&answer_record(FETCH_REQUEST, &lookup))
            );
        }
    }

    #[test]
    fn main_exceptions_of_the_scripts_own_code_and_exits_do_not_suppress_the_context() {
        // `rel.get` on a list: raised by the script itself, not by `fetch_json`
        let list_document = lookup_of_body(b"[]");
        let record = answer_record(MAIN_REQUEST, &list_document);
        assert_eq!(
            exception_of(&record).get("type"),
            Some(&PyValue::str("AttributeError"))
        );
        assert_eq!(suppress_context_of(&record), PyValue::Bool(false));
        // `sys.exit(4)`
        let no_assets = lookup_of_body(b"{}");
        let record = answer_record(MAIN_REQUEST, &no_assets);
        assert_eq!(
            exception_of(&record).get("type"),
            Some(&PyValue::str("SystemExit"))
        );
        assert_eq!(suppress_context_of(&record), PyValue::Bool(false));
    }
}
