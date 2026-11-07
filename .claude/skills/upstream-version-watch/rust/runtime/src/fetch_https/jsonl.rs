//! The JSON-lines service of the HTTPS fetch component: one request per line `{id, operation, args_dto?, kwargs_dto?}`, one record per
//! request, with the request reader, the typed-value conventions and the record layout of [`crate::http_direct::jsonl`] (which this service
//! reuses; only the operation name and the scope differ).
//!
//! Operation: `resolve_torch_pin.fetch_https` with exactly one argument `url` (positional or keyword). `kind` is `value` (the text),
//! `exception` (the exception the original raises, nested through `next`, its constructor arguments and captured attributes, including the
//! three SSL attributes of a certificate failure), `out_of_scope` (a named boundary) or `transport_failure` (a socket or TLS failure whose
//! Python spelling was not captured).

use super::{fetch_https_text_with, FetchEnvironment, SCOPE};
use crate::http_direct::jsonl::{answer_request, evaluate_lines, Service};
use crate::http_direct::{Dial, UreqDialer};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

/// The one operation of this service.
pub const OPERATION: &str = "resolve_torch_pin.fetch_https";

const SERVICE: Service = Service {
    operation: OPERATION,
    function: "fetch_https",
    scope: SCOPE,
};

/// Answers one request line with the environment and dialer given.
pub fn answer_with(line: &PyStr, environment: &FetchEnvironment, dialer: &dyn Dial) -> PyValue {
    answer_request(line, &SERVICE, &|url| {
        fetch_https_text_with(url, environment, dialer)
    })
}

/// Answers one request line in this process's environment over real connections.
pub fn answer(line: &PyStr) -> PyValue {
    answer_with(line, &FetchEnvironment::from_process(), &UreqDialer::new())
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    let environment = FetchEnvironment::from_process();
    let dialer = UreqDialer::new();
    evaluate_lines(input, &|line| answer_with(line, &environment, &dialer))
}
