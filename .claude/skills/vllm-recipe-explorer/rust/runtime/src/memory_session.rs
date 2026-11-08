//! The gate session: `recipe_memory_api gate-session`, the one bidirectional service of the RAM gate.
//!
//! A caller that gives `gate` its own `log` callable cannot be served by a request/response line: the callable must run at the
//! moment the gate logs, before the gate takes its next step. The session keeps that order across two pipes (one bound request on
//! standard input, events on standard output, acknowledgements on standard input again):
//!
//! ```text
//! request  {"id": ..., "operation": "preload_ram_gate.gate", "args": [...], "kwargs": {...}, "log": DESCRIPTOR}
//! event    {"event": "log",    "seq": N, "message": TEXT}     the caller's callable receives TEXT
//!          {"event": "stderr", "seq": N, "text": TEXT}        the default log: the caller writes TEXT to its standard error
//! ack      {"ack": N, "action": "continue" | "abort"}         before the gate does anything further
//! final    {"id": ..., "kind": "value" | "exception", ...}    the record of `evaluate-jsonl`, or
//!          {"id": ..., "kind": "aborted", "reason": R, "seq": N}
//! ```
//!
//! `DESCRIPTOR` says what the caller's `log` argument is, since no JSON value can say so: `{"kind": "callable"}`,
//! `{"kind": "default"}` (a falsy `log`: the gate's own stderr line) or `{"kind": "noncallable", "type": NAME}` (a truthy object
//! that is not callable: calling it is `TypeError: 'NAME' object is not callable`, raised when the gate logs, with no event). The
//! `log` slot of `args`/`kwargs` holds any placeholder; it is never read when a descriptor is present, and the argument must be
//! supplied (so Python's binding errors, "multiple values for argument 'log'" among them, stay the gate's). Without a descriptor
//! the slot is plain data and `log` is classified like `evaluate-jsonl` does.
//!
//! Events are numbered from 1 in the order the gate emitted them, one at a time: the next machine operation (memory read, cache
//! drop) happens only after the acknowledgement of the event before it. What a callable returns is ignored and is not part of the
//! protocol; an exception it raises stays with the caller, which answers `abort`. An `abort`, an end of input and an
//! acknowledgement that is not exactly the two members above (or that names another `seq`) all stop the gate at once, with no
//! further machine operation, and the service ends with the `aborted` record (`reason`: `abort`, `eof`, `malformed_ack`, or
//! `write` when the event could not be delivered). The service ends after the final record in every case.

use crate::memory::{DropSystem, Host, LogChoice, MeminfoSource, Provider};
use crate::memory_api::{parse_request, record, Environment, Machine};
use crate::model_api::encode;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::{BufRead, Write};

/// The failure class that carries a stop request through the gate; it is never a Python exception.
const ABORTED: &str = "SessionAborted";

fn aborted(reason: &str) -> Failure {
    Failure::new(ABORTED, reason)
}

struct Session<'a, M: Machine> {
    machine: &'a mut M,
    input: &'a mut dyn BufRead,
    output: &'a mut dyn Write,
    sequence: u64,
}

impl<M: Machine> Session<'_, M> {
    /// Sends one event and waits for its acknowledgement.
    fn exchange(&mut self, mut event: PyDict, field: &str, text: &PyStr) -> Res<()> {
        self.sequence += 1;
        let sequence = self.sequence;
        event.set("seq", PyValue::int(sequence));
        event.set(field, PyValue::Str(text.clone()));
        let line = PyValue::Dict(event)
            .dumps_ascii()
            .map_err(|error| Failure::new(error.python_type(), &error.to_string()))?;
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
        match acknowledgement(&PyStr::from_surrogateescape(&reply), sequence) {
            Some(true) => Ok(()),
            Some(false) => Err(aborted("abort")),
            None => Err(aborted("malformed_ack")),
        }
    }

    fn event(name: &str) -> PyDict {
        let mut event = PyDict::new();
        event.set("event", PyValue::str(name));
        event
    }
}

/// `Some(true)` for `continue`, `Some(false)` for `abort`, `None` for anything that is not exactly the acknowledgement of
/// event `sequence`.
fn acknowledgement(line: &PyStr, sequence: u64) -> Option<bool> {
    let PyValue::Dict(members) = loads_text(line).ok()? else {
        return None;
    };
    if members.len() != 2 {
        return None;
    }
    let numbered =
        matches!(members.get("ack"), Some(PyValue::Int(number)) if *number == sequence.into());
    match members.get("action") {
        Some(PyValue::Str(action)) if numbered && action.equals("continue") => Some(true),
        Some(PyValue::Str(action)) if numbered && action.equals("abort") => Some(false),
        _ => None,
    }
}

impl<M: Machine> Provider for Session<'_, M> {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        self.machine.mem_available_mib()
    }

    fn try_drop_caches(&mut self) -> Res<bool> {
        self.machine.try_drop_caches()
    }
}

impl<M: Machine> Host for Session<'_, M> {
    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        self.exchange(Self::event("stderr"), "text", text)
    }

    fn call_log(&mut self, message: &PyStr) -> Res<()> {
        self.exchange(Self::event("log"), "message", message)
    }
}

impl<M: Machine> Environment for Session<'_, M> {
    fn meminfo(&mut self) -> &mut dyn MeminfoSource {
        self.machine
    }

    fn drops(&mut self) -> &mut dyn DropSystem {
        self.machine
    }
}

/// The caller's description of its `log` argument; `None` when the request has none.
fn descriptor(members: &PyDict) -> Result<Option<LogChoice>, Failure> {
    let refused = || Failure::new("RequestError", "log must be a descriptor object");
    let Some(given) = members.get("log") else {
        return Ok(None);
    };
    let PyValue::Dict(parts) = given else {
        return Err(refused());
    };
    let kind = match parts.get("kind") {
        Some(PyValue::Str(kind)) => kind.to_utf8().unwrap_or_default(),
        _ => return Err(refused()),
    };
    match (kind.as_str(), parts.len(), parts.get("type")) {
        ("callable", 1, None) => Ok(Some(LogChoice::Callback)),
        ("default", 1, None) => Ok(Some(LogChoice::Default)),
        ("noncallable", 2, Some(PyValue::Str(name))) => Ok(Some(LogChoice::NotCallable(
            name.to_utf8().ok_or_else(refused)?,
        ))),
        _ => Err(refused()),
    }
}

fn write_final(output: &mut dyn Write, row: &PyValue) -> std::io::Result<()> {
    let line = row
        .dumps_ascii()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    writeln!(output, "{line}")?;
    output.flush()
}

/// Serves one gate session on `input`/`output` against `machine`. An `Err` is a final record that could not be written.
pub fn serve<M: Machine>(
    machine: &mut M,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> std::io::Result<()> {
    let mut first = Vec::new();
    let request = match input.read_until(b'\n', &mut first)? {
        0 => Err(record(
            PyValue::None,
            Err(Failure::new("RequestError", "no request")),
            (PyStr::default(), PyStr::default()),
            (PyValue::List(Vec::new()), PyValue::Dict(PyDict::new())),
        )),
        _ => parse_request(&PyStr::from_surrogateescape(&first), &["log"]),
    };
    let mut request = match request {
        Ok(request) => request,
        Err(row) => return write_final(output, &row),
    };
    let mut session = Session {
        machine,
        input,
        output,
        sequence: 0,
    };
    let outcome = descriptor(&request.members).and_then(|log| {
        let function = request.function()?;
        if function.name != "gate" {
            return Err(Failure::new(
                "RequestError",
                "the session serves preload_ram_gate.gate only",
            ));
        }
        function.call(&mut request.args, &mut request.kwargs, &mut session, log)
    });
    let sequence = session.sequence;
    let row = match outcome {
        Err(failure) if failure.class == ABORTED => {
            let mut row = PyDict::new();
            row.set("id", request.id);
            row.set("kind", PyValue::str("aborted"));
            row.set("reason", PyValue::Str(failure.message));
            row.set("seq", PyValue::int(sequence));
            PyValue::Dict(row)
        }
        other => record(
            request.id,
            other,
            (PyStr::default(), PyStr::default()),
            (
                encode(&PyValue::List(request.args)),
                encode(&PyValue::Dict(request.kwargs)),
            ),
        ),
    };
    write_final(output, &row)
}
