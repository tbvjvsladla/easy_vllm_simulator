//! `recipe_escalation_api`: `escalation_predicate.judge` as a JSON-lines service, wire-compatible with the other recipe services, and
//! the loader session for a caller that holds the actual Python loader.
//!
//! * `constants`: this owner's own three globals (the kill vocabulary is the classifier's and is not here).
//! * `evaluate-jsonl`: one request per line `{id, operation: "escalation_predicate.judge", args, kwargs, globals?}`, one record per
//!   request: `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message,
//!   exception_chain, stdout, stderr, args_after, kwargs_after}`. `globals` is a member of its own (never a business keyword): it
//!   replaces any of `MODEL_PLANE_SYMPTOMS`, `HOST_PLANE_SIGNS`, `REFUTED_BY_KILL`. A request carries no other member.
//! * `judge-session`: the same request, with the classifier loaded by the caller at the original's load point (see [`serve_session`]).

use crate::escalation::{judge, Dependency, Loaded, Raised, Vocabulary, MODULE};
use crate::memory_api::{parse_request, Request};
use crate::model_api::encode;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{optional, required, Param, Signature};
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::{BufRead, Write};

fn none_text() -> PyValue {
    PyValue::str("none")
}

const PARAMS: &[Param] = &[
    required("symptoms"),
    required("kill_hits"),
    optional("events_scanned", none_text),
];

/// The module constants: the three globals this owner defines.
pub fn constants() -> PyValue {
    Vocabulary::canonical().to_value()
}

/// Resolves `escalation_predicate.judge`.
fn resolve(operation: &str) -> Result<(), Raised> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Raised::plain(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    if name != "judge" {
        return Err(Raised::plain(
            "AttributeError",
            &format!("module '{module}' has no attribute '{name}'"),
        ));
    }
    Ok(())
}

/// Binds `judge`'s arguments as Python does and runs it.
pub fn call_judge(
    dependency: &mut dyn Dependency,
    vocabulary: &Vocabulary,
    args: &[PyValue],
    kwargs: &PyDict,
) -> Result<PyDict, Raised> {
    let signature = Signature {
        name: "judge",
        params: PARAMS,
        rest: false,
    };
    let bound = signature.bind(args, kwargs).map_err(Raised::from)?;
    judge(dependency, vocabulary, &bound[0], &bound[1], &bound[2])
}

fn record(
    id: PyValue,
    outcome: Result<PyDict, Raised>,
    stderr: PyStr,
    args: Vec<PyValue>,
    kwargs: PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(&PyValue::Dict(value)));
        }
        Err(raised) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(&raised.class));
            row.set("message", PyValue::Str(raised.message.clone()));
            row.set("exception_chain", raised.chain_value());
        }
    }
    row.set("stdout", PyValue::str(""));
    row.set("stderr", PyValue::Str(stderr));
    row.set("args_after", encode(&PyValue::List(args)));
    row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    PyValue::Dict(row)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Raised::from(Failure::new("RequestError", message))),
        PyStr::default(),
        Vec::new(),
        PyDict::new(),
    )
}

/// The vocabulary a request names (`globals`), or the canonical one; `Err` is the finished record.
fn vocabulary_of(request: &Request) -> Result<Vocabulary, PyValue> {
    let mut vocabulary = Vocabulary::canonical();
    if let Some(given) = request.members.get("globals") {
        vocabulary
            .replace_from(given)
            .map_err(|problem| request_record(request.id.clone(), &problem))?;
    }
    Ok(vocabulary)
}

fn operation_of(request: &Request) -> String {
    match request.members.get("operation") {
        Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
        _ => String::new(),
    }
}

/// Answers one request line, the classifier being `dependency`.
pub fn answer(line: &PyStr, dependency: &mut dyn Dependency) -> PyValue {
    let request = match parse_request(line, &["globals"]) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let vocabulary = match vocabulary_of(&request) {
        Ok(vocabulary) => vocabulary,
        Err(row) => return row,
    };
    let outcome = resolve(&operation_of(&request))
        .and_then(|()| call_judge(dependency, &vocabulary, &request.args, &request.kwargs));
    let mut stderr = PyStr::default();
    for text in dependency.take_stderr() {
        stderr.push_py(&text);
    }
    record(request.id, outcome, stderr, request.args, request.kwargs)
}

/// One output line of a record.
pub fn render(row: &PyValue) -> Result<String, Failure> {
    crate::api::render(row)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8], dependency: &mut dyn Dependency) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(render(&answer(&line, dependency))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}

// ── the loader session ───────────────────────────────────────────────────────────────────────────

const ABORTED: &str = "SessionAborted";

/// The dependency of a session: the load is the caller's (an event, then the acknowledgement that carries its result).
struct SessionLoader<'a> {
    input: &'a mut dyn BufRead,
    output: &'a mut dyn Write,
    sequence: u64,
}

fn aborted(reason: &str) -> Raised {
    Raised::plain(ABORTED, reason)
}

impl Dependency for SessionLoader<'_> {
    fn load(&mut self) -> Result<Loaded, Raised> {
        self.sequence += 1;
        let sequence = self.sequence;
        let mut event = PyDict::new();
        event.set("event", PyValue::str("load"));
        event.set("seq", PyValue::int(sequence));
        let line = PyValue::Dict(event)
            .dumps_ascii()
            .map_err(|error| Raised::from(Failure::new(error.python_type(), &error.to_string())))?;
        if writeln!(self.output, "{line}")
            .and_then(|()| self.output.flush())
            .is_err()
        {
            return Err(aborted("write"));
        }
        let mut reply = Vec::new();
        match self.input.read_until(b'\n', &mut reply) {
            Ok(0) | Err(_) => return Err(aborted("eof")),
            Ok(_) => {}
        }
        let malformed = || aborted("malformed_ack");
        let PyValue::Dict(ack) =
            loads_text(&PyStr::from_surrogateescape(&reply)).map_err(|_| malformed())?
        else {
            return Err(malformed());
        };
        if !matches!(ack.get("ack"), Some(PyValue::Int(number)) if *number == sequence.into()) {
            return Err(malformed());
        }
        match (ack.get("action"), ack.len()) {
            (Some(PyValue::Str(action)), 2) if action.equals("abort") => Err(aborted("abort")),
            (Some(PyValue::Str(action)), 3) if action.equals("loaded") => {
                match ack.get("kill_event_kinds") {
                    Some(PyValue::List(kinds)) => Ok(Loaded {
                        kill_event_kinds: Some(kinds.clone()),
                    }),
                    _ => Err(malformed()),
                }
            }
            (Some(PyValue::Str(action)), 3) if action.equals("raise") => ack
                .get("error")
                .and_then(Raised::from_chain)
                .map_or_else(|| Err(malformed()), Err),
            _ => Err(malformed()),
        }
    }

    fn utc(&mut self, _value: &PyStr, _field: &str) -> Result<PyValue, Raised> {
        Err(Raised::plain(
            "RuntimeError",
            "a loader session serves judge only",
        ))
    }

    fn kill_events(
        &mut self,
        _lines: &[PyStr],
        _started: &PyValue,
        _ended: &PyValue,
        _tolerance_s: &num_bigint::BigInt,
    ) -> Result<PyValue, Raised> {
        Err(Raised::plain(
            "RuntimeError",
            "a loader session serves judge only",
        ))
    }
}

fn write_final(output: &mut dyn Write, row: &PyValue) -> std::io::Result<()> {
    let line = row
        .dumps_ascii()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    writeln!(output, "{line}")?;
    output.flush()
}

/// Serves one `judge-session` on `input`/`output`: the request is bound and its lists converted first (an invalid one fails with no
/// event); at the original's load point one event `{"event": "load", "seq": 1}` is written and the acknowledgement is read:
/// `{"ack": 1, "action": "loaded", "kill_event_kinds": [...]}` (the caller's actual module's tuple; the judgment continues),
/// `{"ack": 1, "action": "raise", "error": EXCEPTION_CHAIN}` (the actual loader raised; the call's exception), or
/// `{"ack": 1, "action": "abort"}`. An abort, an end of input or any other acknowledgement ends the call with
/// `{"id", "kind": "aborted", "reason": "abort" | "eof" | "malformed_ack" | "write", "seq": 1}`. The final record is that of
/// `evaluate-jsonl`.
pub fn serve_session(input: &mut dyn BufRead, output: &mut dyn Write) -> std::io::Result<()> {
    let mut first = Vec::new();
    let request = match input.read_until(b'\n', &mut first)? {
        0 => Err(request_record(PyValue::None, "no request")),
        _ => parse_request(&PyStr::from_surrogateescape(&first), &["globals"]),
    };
    let request = match request {
        Ok(request) => request,
        Err(row) => return write_final(output, &row),
    };
    let vocabulary = match vocabulary_of(&request) {
        Ok(vocabulary) => vocabulary,
        Err(row) => return write_final(output, &row),
    };
    let mut loader = SessionLoader {
        input: &mut *input,
        output: &mut *output,
        sequence: 0,
    };
    let outcome = resolve(&operation_of(&request))
        .and_then(|()| call_judge(&mut loader, &vocabulary, &request.args, &request.kwargs));
    let sequence = loader.sequence;
    let row = match outcome {
        Err(raised) if raised.class == ABORTED => {
            let mut row = PyDict::new();
            row.set("id", request.id);
            row.set("kind", PyValue::str("aborted"));
            row.set("reason", PyValue::Str(raised.message));
            row.set("seq", PyValue::int(sequence));
            PyValue::Dict(row)
        }
        other => record(
            request.id,
            other,
            PyStr::default(),
            request.args,
            request.kwargs,
        ),
    };
    write_final(output, &row)
}
