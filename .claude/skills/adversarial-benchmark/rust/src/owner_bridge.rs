//! A closed-operation session with the Python programs the lite and sweep-meta verifiers still depend on.
//!
//! The verifiers ask three owners that have not been ported (hint reader `render_bench_section.py`, evidence
//! publisher `evidence_publisher.py` and completion gate `completion_gate.py`) and must see what those owners really answer, so none of their rules is copied here. This is
//! transport only: [`OwnerBridge`] starts one Python interpreter that runs the program in `owner_bridge.py`
//! (compiled into this crate and handed over as `-c` text; there is no file to deploy), keeps it for the whole
//! verification run, and sends one named operation per request. The operations are the fixed methods below; no
//! caller supplied code or callable name is ever evaluated.
//!
//! One interpreter for the whole run matters: the owners keep module state (the evidence publisher imports the
//! reader again at every call), exactly as they did when the original
//! verifier called them in its own process. Every call is a real call: results are never cached or shared between
//! callers.
//!
//! The session's standard input, output and error are the verifier's own; its channel is a separate socket left open
//! across the spawn. Raw descriptor writes and inherited children of an owner therefore reach the real streams at
//! once and a child reads the real standard input. What an owner writes through the Python objects `sys.stdout` and
//! `sys.stderr` is sent as events (write, flush) while it runs; each event is applied here (stdout into this
//! process's own Python-faithful buffered stream, stderr at once) and acknowledged, and the owner waits for the
//! acknowledgement, so owner text, raw bytes and the verifier's own lines keep the order of one process whether the
//! stream is block-buffered, a terminal or unbuffered. Exceptions come back as [`Raised`] with the owner's class,
//! message, `repr` and traceback class name; a `SystemExit` keeps its string reason or exit status.
//!
//! The repeat sweep verifier (`selftest_sweep_repeats`) uses the same session for one late operation of the hint reader
//! ([`OwnerBridge::parse_sweep_report`]): it imports `render_bench_section` under that module name when the first report
//! check runs, not when the session starts, so the reader's import side effects and errors keep their original time.
//!
//! This transport is the *current Python dependency* of the verifiers (see PORTING.md): it goes away with the
//! stage that ports the owners. Nothing here falls back to the original verifier programs.

use crate::conformance::{OwnerHierarchy, Raised, SystemExitOutcome};
use crate::dataset_commands::spawn_failure;
use crate::dataset_prep::Console;
use crate::guidellm::Failure;
use crate::pyjson::loads_text;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Environment variable naming the interpreter that runs the session (default `python3` on `PATH`).
pub const PYTHON_ENV: &str = "OWNER_BRIDGE_PYTHON";

/// The session program, run as `python -c`.
const SESSION_PROGRAM: &str = include_str!("owner_bridge.py");

/// The class reported when the session itself breaks (it is not an exception of any original).
const SESSION_ERROR: &str = "OwnerBridgeError";

/// The three owner programs a session loads, each under the module name the original verifier gave it.
#[derive(Clone, Copy)]
pub enum OwnerRole {
    /// The hint reader `render_bench_section.py`.
    HintReader,
    /// The evidence publisher `evidence_publisher.py`.
    EvidencePublisher,
    /// The completion gate `completion_gate.py`.
    CompletionGate,
}

impl OwnerRole {
    fn key(self) -> &'static str {
        match self {
            OwnerRole::HintReader => "rbs",
            OwnerRole::EvidencePublisher => "ep",
            OwnerRole::CompletionGate => "gate",
        }
    }
}

/// What the reader returned for a sweep report (`render_bench_section.parse_report`): the `level` of each parsed
/// sweep row, the `truncated` value (`None` when the parsed report has no such key) and `repr(parsed)` for a
/// failure detail.
pub struct SweepReportParse {
    pub levels: PyValue,
    pub truncated: Option<PyValue>,
    pub shown: String,
}

fn py_text_of(value: &PyValue) -> String {
    value.display_text().to_utf8().unwrap_or_default()
}

/// A running session. Dropping it ends the interpreter.
pub struct OwnerBridge {
    child: Child,
    /// The requests and acknowledgements go out on this end of the socket pair whose other end the session holds.
    requests: Option<UnixStream>,
    replies: BufReader<UnixStream>,
}

/// Whether an I/O error on the channel means the session process is gone.
fn session_is_gone(error: &std::io::Error) -> bool {
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof};
    matches!(
        error.kind(),
        BrokenPipe | ConnectionAborted | ConnectionReset | UnexpectedEof
    )
}

fn session_error(message: &str) -> Raised {
    Raised::new(SESSION_ERROR, message)
}

/// The static class name of an exception an owner raised: the common names are fixed, any other is kept as it
/// arrived (an error path only; the leaked text is small and lives as long as the process).
fn exception_class(name: &str) -> &'static str {
    const KNOWN: [&str; 16] = [
        "AttributeError",
        "FileNotFoundError",
        "ImportError",
        "IndexError",
        "JSONDecodeError",
        "KeyError",
        "ModuleNotFoundError",
        "NameError",
        "OSError",
        "RuntimeError",
        "SyntaxError",
        "SystemExit",
        "TypeError",
        "UnicodeDecodeError",
        "ValueError",
        "ZeroDivisionError",
    ];
    KNOWN
        .iter()
        .find(|known| **known == name)
        .copied()
        .unwrap_or_else(|| Box::leak(name.to_string().into_boxed_str()))
}

fn object_of(reply: &PyValue) -> Result<&PyDict, Raised> {
    match reply.peel() {
        PyValue::Dict(map) => Ok(map),
        _ => Err(session_error(
            "the owner session answered with something that is not a record",
        )),
    }
}

fn text_of(map: &PyDict, key: &str) -> Result<PyStr, Raised> {
    match map.get(key).map(PyValue::peel) {
        Some(PyValue::Str(text)) => Ok(text.clone()),
        _ => Err(session_error(&format!(
            "the owner session reply lacks the text field `{key}`"
        ))),
    }
}

/// The `Raised` of the failure record of a reply (`exc`).
fn raised_of(exception: &PyValue) -> Result<Raised, Raised> {
    let map = object_of(exception)?;
    let class = text_of(map, "class")?.to_utf8().unwrap_or_default();
    let message = text_of(map, "message")?;
    let repr = text_of(map, "repr")?.to_utf8().unwrap_or_default();
    let failure = Failure {
        class: exception_class(&class),
        message,
    };
    let raised = Raised::with_repr(failure, repr);
    // The owner's class as a traceback prints it (`module.qualname` unless builtin or `__main__`).
    let raised = match text_of(map, "qualified") {
        Ok(qualified) => raised.with_qualified_class(qualified.to_utf8().unwrap_or(class)),
        Err(_) => raised,
    };
    // Its place in the hierarchy as the owner's interpreter answers it (`isinstance(error, Exception)` and
    // `isinstance(error, SystemExit)`); these facts, not the class name, decide what the verifier may catch.
    let fact = |name: &str| match map.get(name).map(PyValue::peel) {
        Some(PyValue::Bool(answer)) => Some(*answer),
        _ => None,
    };
    let raised = match (fact("is_exception"), fact("is_system_exit")) {
        (Some(is_exception), Some(is_system_exit)) => raised.with_owner_hierarchy(OwnerHierarchy {
            is_exception,
            is_system_exit,
        }),
        _ => raised,
    };
    // What a `SystemExit` of the owner carried (`error.code`), as the interpreter's own record gives it.
    Ok(match map.get("exit").map(PyValue::peel) {
        Some(PyValue::Dict(payload)) => match exit_outcome_of(payload) {
            Some(outcome) => raised.with_system_exit_outcome(outcome),
            None => raised,
        },
        _ => raised,
    })
}

/// The typed outcome of a `SystemExit` payload record: `{"status": <int>}` or `{"message": <text>}`.
fn exit_outcome_of(payload: &PyDict) -> Option<SystemExitOutcome> {
    match (
        payload.get("status").map(PyValue::peel),
        payload.get("message").map(PyValue::peel),
    ) {
        (Some(PyValue::Int(status)), None) => {
            Some(SystemExitOutcome::Status(process_status(status)))
        }
        (None, Some(PyValue::Str(text))) => Some(SystemExitOutcome::Message(text.clone())),
        _ => None,
    }
}

/// The status the operating system sees for `SystemExit(status)`: CPython 3.12 takes the integer as a C `long` and
/// passes it to `exit`, which keeps the low eight bits; an integer that does not fit a `long` makes `PyLong_AsLong`
/// answer -1, so the status is 255.
fn process_status(status: &num_bigint::BigInt) -> u8 {
    use num_traits::ToPrimitive;
    match status.to_i64() {
        Some(small) => u8::try_from(small.rem_euclid(256)).expect("a remainder below 256"),
        None => 255,
    }
}

impl OwnerBridge {
    /// Starts the session. `scripts_dir` is the directory of the deployed benchmark scripts: what the first entry
    /// of `sys.path` is when the original verifier is run as a script.
    pub fn start(scripts_dir: &Path, console: &mut dyn Console) -> Result<Self, Raised> {
        let python = std::env::var_os(PYTHON_ENV)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| OsString::from("python3"));
        // The protocol runs on a socket pair that is neither standard stream: descriptors 0, 1 and 2 stay the
        // verifier's own, so raw writes, inherited children and reads of standard input behave as in the original
        // process. The session's end of the pair is left open across the spawn (its close-on-exec flag cleared) and
        // named by number; the verifier closes its copy right after, so the session's exit ends the channel.
        let (ours, theirs) = UnixStream::pair().map_err(|error| {
            session_error(&format!("cannot create the owner session channel: {error}"))
        })?;
        rustix::io::fcntl_setfd(&theirs, rustix::io::FdFlags::empty()).map_err(|error| {
            session_error(&format!(
                "cannot hand the owner session channel over: {error}"
            ))
        })?;
        let replies = BufReader::new(ours.try_clone().map_err(|error| {
            session_error(&format!("cannot share the owner session channel: {error}"))
        })?);
        let child = Command::new(&python)
            .arg("-c")
            .arg(SESSION_PROGRAM)
            .arg(std::os::fd::AsRawFd::as_raw_fd(&theirs).to_string())
            // The owners are imported from the deployed tree; the session must not write caches into it.
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| Raised::from_failure(spawn_failure(&error, &python)))?;
        drop(theirs);
        let mut bridge = OwnerBridge {
            child,
            requests: Some(ours),
            replies,
        };
        bridge.call(
            console,
            "start",
            vec![("scripts_dir", text_value(scripts_dir))],
        )?;
        Ok(bridge)
    }

    /// One request and its reply. The owner's stream events are applied and acknowledged as they arrive; the reply is
    /// the result or the exception.
    fn call(
        &mut self,
        console: &mut dyn Console,
        operation: &str,
        arguments: Vec<(&str, PyValue)>,
    ) -> Result<PyValue, Raised> {
        self.call_split(console, operation, arguments)?
    }

    /// [`call`](Self::call) with the two kinds of failure kept apart: the outer `Err` is the transport (the session
    /// itself broke), the inner `Err` is an exception the owner raised, carrying its hierarchy fact.
    fn call_split(
        &mut self,
        console: &mut dyn Console,
        operation: &str,
        arguments: Vec<(&str, PyValue)>,
    ) -> Result<Result<PyValue, Raised>, Raised> {
        let mut request = PyDict::new();
        request.set("op", PyValue::str(operation));
        for (name, value) in arguments {
            request.set(name, value);
        }
        let line = PyValue::Dict(request)
            .dumps_ascii()
            .map_err(|error| session_error(&error.to_string()))?;
        let input = self
            .requests
            .as_mut()
            .ok_or_else(|| session_error("the owner session is closed"))?;
        input
            .write_all(line.as_bytes())
            .and_then(|()| input.write_all(b"\n"))
            .and_then(|()| input.flush())
            .map_err(|error| {
                if session_is_gone(&error) {
                    session_error(&format!(
                        "the owner session ended before it answered `{operation}`"
                    ))
                } else {
                    session_error(&format!("cannot send `{operation}`: {error}"))
                }
            })?;
        loop {
            let mut answer = String::new();
            let count = match self.replies.read_line(&mut answer) {
                Ok(count) => count,
                // A session that is gone resets or closes the channel; both are "it ended", not a read failure.
                Err(error) if session_is_gone(&error) => 0,
                Err(error) => {
                    return Err(session_error(&format!(
                        "cannot read the reply to `{operation}`: {error}"
                    )))
                }
            };
            if count == 0 {
                return Err(session_error(&format!(
                    "the owner session ended before it answered `{operation}`"
                )));
            }
            let reply = loads_text(&PyStr::new(answer.trim_end())).map_err(|error| {
                session_error(&format!(
                    "the reply to `{operation}` is not JSON: {error:?}"
                ))
            })?;
            let map = object_of(&reply)?;
            if map.contains_key("event") {
                // A stream event of the owner: applied first, then acknowledged; the owner waits for the
                // acknowledgement, so it cannot do its next raw descriptor write before this is done.
                let outcome = apply_event(console, map)?;
                self.acknowledge(operation, outcome)?;
                continue;
            }
            let succeeded = matches!(map.get("ok").map(PyValue::peel), Some(PyValue::Bool(true)));
            return if succeeded {
                Ok(Ok(map.get("value").cloned().unwrap_or(PyValue::None)))
            } else {
                match map.get("exc") {
                    Some(exception) => Ok(Err(raised_of(exception)?)),
                    None => Err(session_error(&format!(
                        "`{operation}` failed without a reason"
                    ))),
                }
            };
        }
    }

    /// The acknowledgement of a stream event: `{"ack": true}`, or `{"error": text}` when the event could not be
    /// applied (the owner's write then raises).
    fn acknowledge(&mut self, operation: &str, outcome: Result<(), Failure>) -> Result<(), Raised> {
        let mut record = PyDict::new();
        match outcome {
            Ok(()) => record.set("ack", PyValue::Bool(true)),
            Err(failure) => record.set("error", PyValue::Str(failure.message)),
        }
        let line = PyValue::Dict(record)
            .dumps_ascii()
            .map_err(|error| session_error(&error.to_string()))?;
        let input = self
            .requests
            .as_mut()
            .ok_or_else(|| session_error("the owner session is closed"))?;
        input
            .write_all(line.as_bytes())
            .and_then(|()| input.write_all(b"\n"))
            .and_then(|()| input.flush())
            .map_err(|error| {
                session_error(&format!(
                    "cannot acknowledge an event of `{operation}`: {error}"
                ))
            })
    }

    /// Loads an owner program the way the original `_load` did: its directory goes first on `sys.path`, then the
    /// file is executed as a module. A failing owner top level is the owner's exception.
    pub fn load(
        &mut self,
        console: &mut dyn Console,
        role: OwnerRole,
        path: &Path,
    ) -> Result<(), Raised> {
        self.call(
            console,
            "load",
            vec![
                ("role", PyValue::str(role.key())),
                ("path", text_value(path)),
            ],
        )
        .map(|_| ())
    }

    /// `rbs.parse_measurement_config(text)`.
    pub fn parse_measurement_config(
        &mut self,
        console: &mut dyn Console,
        text: &str,
    ) -> Result<PyValue, Raised> {
        self.call(
            console,
            "rbs_parse_measurement_config",
            vec![("text", PyValue::str(text))],
        )
    }

    /// `rbs.is_lite_report(text)`.
    pub fn is_lite_report(
        &mut self,
        console: &mut dyn Console,
        text: &str,
    ) -> Result<bool, Raised> {
        self.call(
            console,
            "rbs_is_lite_report",
            vec![("text", PyValue::str(text))],
        )
        .map(|value| value.is_truthy())
    }

    /// `rbs.lite_metrics_present(text)`.
    pub fn lite_metrics_present(
        &mut self,
        console: &mut dyn Console,
        text: &str,
    ) -> Result<bool, Raised> {
        self.call(
            console,
            "rbs_lite_metrics_present",
            vec![("text", PyValue::str(text))],
        )
        .map(|value| value.is_truthy())
    }

    /// `rbs.render_lite(rbs.parse_lite_report(text), name)`.
    pub fn render_lite(
        &mut self,
        console: &mut dyn Console,
        text: &str,
        name: &str,
    ) -> Result<PyValue, Raised> {
        self.call(
            console,
            "rbs_render_lite",
            vec![("text", PyValue::str(text)), ("name", PyValue::str(name))],
        )
    }

    /// `len(rbs.parse_report(text)["levels"])`.
    pub fn parse_report_level_count(
        &mut self,
        console: &mut dyn Console,
        text: &str,
    ) -> Result<PyValue, Raised> {
        self.call(
            console,
            "rbs_parse_report_level_count",
            vec![("text", PyValue::str(text))],
        )
    }

    /// The late reader check of `selftest_sweep_repeats` (N4): inserts `directory` first on `sys.path`, imports
    /// `render_bench_section` under its own name and parses the report. The owner's exception (a missing reader, a
    /// failing import, a parse error) comes back as the inner `Err` with its place in the hierarchy
    /// ([`Raised::is_exception`]); a failure of the session itself is the outer `Err`.
    pub fn parse_sweep_report(
        &mut self,
        console: &mut dyn Console,
        directory: &Path,
        text: &str,
    ) -> Result<Result<SweepReportParse, Raised>, Raised> {
        let reply = match self.call_split(
            console,
            "rbs_parse_sweep_report",
            vec![
                ("directory", text_value(directory)),
                ("text", PyValue::str(text)),
            ],
        )? {
            Ok(reply) => reply,
            Err(owner_exception) => return Ok(Err(owner_exception)),
        };
        let map = object_of(&reply)?;
        let field = |name: &str| {
            map.get(name).cloned().ok_or_else(|| {
                session_error(&format!("the reader answer lacks the field `{name}`"))
            })
        };
        Ok(Ok(SweepReportParse {
            levels: field("levels")?,
            truncated: matches!(field("has_truncated")?.peel(), PyValue::Bool(true))
                .then(|| field("truncated"))
                .transpose()?,
            shown: py_text_of(&field("shown")?),
        }))
    }

    /// An attribute of the reader that holds a contract constant (`MEASUREMENT_CONFIG_TITLE`, ...).
    pub fn reader_constant(
        &mut self,
        console: &mut dyn Console,
        name: &str,
    ) -> Result<PyValue, Raised> {
        self.call(console, "rbs_constant", vec![("name", PyValue::str(name))])
    }

    /// `ep._BENCH_SRC_NAME_RE[kind].match(name) is not None`.
    pub fn publisher_name_matches(
        &mut self,
        console: &mut dyn Console,
        kind: &str,
        name: &str,
    ) -> Result<bool, Raised> {
        self.call(
            console,
            "ep_bench_name_matches",
            vec![("kind", PyValue::str(kind)), ("name", PyValue::str(name))],
        )
        .map(|value| value.is_truthy())
    }

    /// `ep.report_measurement_config(text)[0]`. Each call is a call of the owner (it may import the reader again).
    pub fn publisher_measurement_config(
        &mut self,
        console: &mut dyn Console,
        text: &str,
    ) -> Result<PyValue, Raised> {
        self.call(
            console,
            "ep_report_measurement_config",
            vec![("text", PyValue::str(text))],
        )
    }

    /// An attribute of the publisher that holds a contract constant (`LITE_BENCH_MODE`).
    pub fn publisher_constant(
        &mut self,
        console: &mut dyn Console,
        name: &str,
    ) -> Result<PyValue, Raised> {
        self.call(console, "ep_constant", vec![("name", PyValue::str(name))])
    }

    /// `gate.parse_flat_certificate(text)` as `(fields, ok)`.
    pub fn parse_flat_certificate(
        &mut self,
        console: &mut dyn Console,
        text: &str,
    ) -> Result<(PyValue, bool), Raised> {
        let pair = self.call(
            console,
            "gate_parse_flat_certificate",
            vec![("text", PyValue::str(text))],
        )?;
        match pair.peel() {
            PyValue::List(items) if items.len() == 2 => {
                Ok((items[0].clone(), items[1].is_truthy()))
            }
            _ => Err(session_error(
                "the completion gate did not answer (fields, ok)",
            )),
        }
    }

    /// `gate.certificate_run_key(fields) is not None`.
    pub fn certificate_run_key_present(
        &mut self,
        console: &mut dyn Console,
        fields: &PyValue,
    ) -> Result<bool, Raised> {
        self.call(
            console,
            "gate_certificate_run_key_present",
            vec![("fields", fields.clone())],
        )
        .map(|value| value.is_truthy())
    }
}

impl Drop for OwnerBridge {
    fn drop(&mut self) {
        // Closing the request channel ends the session; a session that does not end is killed.
        if let Some(requests) = self.requests.take() {
            let _ = requests.shutdown(std::net::Shutdown::Both);
        }
        for _ in 0..200 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text_value(path: &Path) -> PyValue {
    use std::os::unix::ffi::OsStrExt;
    PyValue::Str(PyStr::from_surrogateescape(path.as_os_str().as_bytes()))
}

/// Applies one stream event of the owner: text its `sys.stdout` wrote goes into the verifier's own buffered stream
/// (no newline added or removed), text of `sys.stderr` is written at once, a `flush` flushes standard output. Raw
/// descriptor writes and inherited children never come here: they went to the real streams while the owner ran.
/// The outer error is a malformed event; the inner one is what the console refused (the owner's write raises it).
fn apply_event(console: &mut dyn Console, event: &PyDict) -> Result<Result<(), Failure>, Raised> {
    let kind = text_of(event, "event")?.to_utf8().unwrap_or_default();
    match kind.as_str() {
        "out" => Ok(console.write(&text_of(event, "text")?)),
        "err" => {
            console.error(&text_of(event, "text")?);
            Ok(Ok(()))
        }
        "flush" => {
            let stream = text_of(event, "stream")?.to_utf8().unwrap_or_default();
            Ok(if stream == "out" {
                console.flush()
            } else {
                Ok(())
            })
        }
        other => Err(session_error(&format!("unknown stream event `{other}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(entries: Vec<(&str, PyValue)>) -> PyDict {
        let mut map = PyDict::new();
        for (key, value) in entries {
            map.set(key, value);
        }
        map
    }

    #[test]
    fn a_status_payload_is_what_the_operating_system_sees() {
        // Confirmed against CPython 3.12.3 (`logs/python-exit-status-probe.txt` of the worker's check directory).
        let status = |text: &str| {
            let number: num_bigint::BigInt = text.parse().expect("an integer");
            process_status(&number)
        };
        for (given, seen) in [
            ("17", 17),
            ("273", 17),
            ("-1", 255),
            ("-256", 0),
            ("2147483653", 5),
            ("4294967313", 17),
            ("9223372036854775807", 255),
            ("9223372036854775808", 255),
            ("1180591620717411303424", 255),
            ("0", 0),
            ("1", 1),
        ] {
            assert_eq!(status(given), seen, "SystemExit({given})");
        }
    }

    #[test]
    fn a_payload_is_a_status_or_a_reason_and_nothing_else() {
        assert_eq!(
            exit_outcome_of(&payload(vec![("status", PyValue::int(17))])),
            Some(SystemExitOutcome::Status(17))
        );
        assert_eq!(
            exit_outcome_of(&payload(vec![("message", PyValue::str(""))])),
            Some(SystemExitOutcome::Message(PyStr::new("")))
        );
        assert_eq!(exit_outcome_of(&payload(vec![])), None);
        assert_eq!(
            exit_outcome_of(&payload(vec![
                ("status", PyValue::int(1)),
                ("message", PyValue::str("both"))
            ])),
            None
        );
        // a boolean is not read as an integer here: the session turns `True` into the integer 1 before it speaks
        assert_eq!(
            exit_outcome_of(&payload(vec![("status", PyValue::Bool(true))])),
            None
        );
    }
}
