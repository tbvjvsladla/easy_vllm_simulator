//! The JSON-lines service of the direct-HTTP component (`ABI.md` of the R35 stage-B1 work): one request per line
//! `{id, operation, args_dto?, kwargs_dto?}`, one record per request, in the typed-value conventions of the stage-A service
//! (`crate::version_api`), whose request reader and refusal records are reused.
//!
//! Operation: `resolve_torch_pin.fetch_http_direct` with exactly one argument `url` (positional or keyword). The record states which of four
//! things happened (`kind`): `value` (the text), `exception` (the exception the original raises, nested through `next` as the saved
//! originals write it, without the interpreter's frames, which a native implementation does not have), `out_of_scope` (a named boundary of
//! this component), or `transport_failure` (a socket-level failure whose Python spelling was not captured).
//!
//! The record and request handling is shared with the HTTPS fetch service (`crate::fetch_https::jsonl`): a [`Service`] names the operation,
//! the function of the argument-count message and the scope, and the caller supplies the fetch.

use super::failure::ErrorValue;
use super::wire::Dial;
use super::{
    fetch_http_direct_text_with, FetchFailure, Link, ProxyEnvironment, PyException, UreqDialer,
    SCOPE,
};
use crate::dto::encode;
use crate::torch_pin::Raised;
use crate::version_api::{parse_request, record as refusal_record, Request};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The one operation of this service.
pub const OPERATION: &str = "resolve_torch_pin.fetch_http_direct";

/// The attributes the selected contract captures, in the order the saved originals list them.
const CAPTURED_ATTRIBUTES: [&str; 17] = [
    "errno",
    "strerror",
    "filename",
    "filename2",
    "code",
    "url",
    "reason",
    "verify_code",
    "verify_message",
    "library",
    "line",
    "partial",
    "expected",
    "encoding",
    "object",
    "start",
    "end",
];

/// What one request/response service answers for: its operation, the function its argument message names, and its scope.
pub(crate) struct Service {
    pub operation: &'static str,
    pub function: &'static str,
    pub scope: &'static str,
}

const HTTP_DIRECT: Service = Service {
    operation: OPERATION,
    function: "fetch_http_direct",
    scope: SCOPE,
};

fn dto(value: &PyValue) -> PyValue {
    encode(value).unwrap_or(PyValue::None)
}

/// An argument or attribute value; a wrapped exception is written as the saved originals write it, `["exception", {...}]`.
fn error_value(value: &ErrorValue) -> PyValue {
    match value {
        ErrorValue::Plain(plain) => dto(plain),
        ErrorValue::Exception(error) => {
            PyValue::List(vec![PyValue::str("exception"), exception_value(error)])
        }
    }
}

/// One exception of a chain as the saved originals write it (`next` carries the cause or context).
fn exception_value(error: &PyException) -> PyValue {
    let mut row = PyDict::new();
    row.set("type", PyValue::str(error.class));
    row.set("module", PyValue::str(error.module));
    row.set("message", PyValue::Str(error.message.clone()));
    row.set(
        "args",
        PyValue::List(error.args.iter().map(error_value).collect()),
    );
    row.set(
        "suppress_context",
        PyValue::Bool(matches!(error.link, Some(Link::Cause(_)))),
    );
    row.set(
        "relation",
        match &error.link {
            Some(Link::Cause(_)) => PyValue::str("cause"),
            Some(Link::Context(_)) => PyValue::str("context"),
            None => PyValue::None,
        },
    );
    if !error.attributes.is_empty() {
        let mut attributes = PyDict::new();
        for name in CAPTURED_ATTRIBUTES {
            if let Some((_, value)) = error.attributes.iter().find(|(known, _)| *known == name) {
                attributes.set(name, error_value(value));
            }
        }
        row.set("attributes", PyValue::Dict(attributes));
    }
    if let Some(headers) = &error.http_headers {
        row.set(
            "http_headers",
            PyValue::List(
                headers
                    .iter()
                    .map(|(name, value)| {
                        PyValue::List(vec![PyValue::str(name), PyValue::str(value)])
                    })
                    .collect(),
            ),
        );
    }
    if let Some(Link::Cause(next) | Link::Context(next)) = &error.link {
        row.set("next", exception_value(next));
    }
    PyValue::Dict(row)
}

/// The record fields every kind shares.
fn finish(mut row: PyDict, scope: &str, args: &[PyValue], kwargs: &PyDict) -> PyValue {
    row.set("scope", PyValue::str(scope));
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::str(""));
    row.set("args_after", dto(&PyValue::List(args.to_vec())));
    row.set("kwargs_after", dto(&PyValue::Dict(kwargs.clone())));
    PyValue::Dict(row)
}

fn record_of(
    id: PyValue,
    outcome: Result<String, FetchFailure>,
    scope: &str,
    args: &[PyValue],
    kwargs: &PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(text) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", dto(&PyValue::Str(PyStr::new(&text))));
        }
        Err(FetchFailure::Raised(error)) => {
            row.set("kind", PyValue::str("exception"));
            row.set("exception", exception_value(&error));
        }
        Err(FetchFailure::OutOfScope(refusal)) => {
            row.set("kind", PyValue::str("out_of_scope"));
            row.set("boundary", PyValue::str(refusal.boundary));
            row.set("detail", PyValue::str(&refusal.detail));
        }
        Err(FetchFailure::Transport(failure)) => {
            row.set("kind", PyValue::str("transport_failure"));
            row.set("phase", PyValue::str(failure.phase));
            row.set("detail", PyValue::str(&failure.detail));
        }
    }
    finish(row, scope, args, kwargs)
}

/// The `url` of a request: one positional or one keyword argument, nothing else.
fn url_argument<'a>(request: &'a Request, function: &str) -> Result<&'a PyValue, String> {
    match (request.args.as_slice(), request.kwargs.len()) {
        ([url], 0) => Ok(url),
        ([], 1) => request
            .kwargs
            .get("url")
            .ok_or_else(|| "the only keyword argument is 'url'".to_string()),
        _ => Err(format!("{function} takes exactly one argument: url")),
    }
}

/// Answers one request line of `service` with `fetch` as the one function it serves.
pub(crate) fn answer_request(
    line: &PyStr,
    service: &Service,
    fetch: &dyn Fn(&str) -> Result<String, FetchFailure>,
) -> PyValue {
    let request = match parse_request(line) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let refuse = |message: &str| {
        refusal_record(
            request.id.clone(),
            Err(Raised::builtin("RequestError", message)),
            &request.args,
            &request.kwargs,
        )
    };
    if request.operation != service.operation {
        return refuse(&format!(
            "operation must be '{}', not {}",
            service.operation,
            PyStr::new(&request.operation).repr()
        ));
    }
    let url = match url_argument(&request, service.function) {
        Ok(url) => url,
        Err(message) => return refuse(&message),
    };
    let outcome = match url {
        PyValue::Str(text) => match text.to_utf8() {
            Some(url) => fetch(&url),
            None => Err(super::failure::out_of_scope(
                "non_str_url",
                "a str with lone surrogates cannot be sent as a URL by this component",
            )),
        },
        other => Err(super::failure::out_of_scope(
            "non_str_url",
            format!(
                "the URL must be a plain str (got {}); non-str URLs and Request objects are outside this component",
                other.repr()
            ),
        )),
    };
    record_of(
        request.id,
        outcome,
        service.scope,
        &request.args,
        &request.kwargs,
    )
}

/// Answers one request line with the proxy environment and dialer given.
pub fn answer_with(line: &PyStr, proxies: &ProxyEnvironment, dialer: &dyn Dial) -> PyValue {
    answer_request(line, &HTTP_DIRECT, &|url| {
        fetch_http_direct_text_with(url, proxies, dialer)
    })
}

/// Answers one request line in this process's environment over real connections.
pub fn answer(line: &PyStr) -> PyValue {
    answer_with(line, &ProxyEnvironment::from_process(), &UreqDialer::new())
}

/// Every non-blank line of `input` answered in order by `answer`, one JSON line each.
pub(crate) fn evaluate_lines(
    input: &[u8],
    answer: &dyn Fn(&PyStr) -> PyValue,
) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(crate::api::render(&answer(&line))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    let dialer = UreqDialer::new();
    let proxies = ProxyEnvironment::from_process();
    evaluate_lines(input, &|line| answer_with(line, &proxies, &dialer))
}
