//! The controlled machine of the RAM-gate checks: provider injection for the fixture entry points, never for a production program.
//!
//! This module exists only when the crate is built with the `fixture` feature, which no production build enables. It lets a check
//! drive the same library code the production programs run (the five functions, the command line, the gate session) against a
//! machine a case describes instead of the real one, and records what the original recorded about such a run: the ordered trace of
//! provider, log and standard-error events, the provider counters, the streams, the arguments after the call and the exit.
//!
//! A case is a JSON object `{id, function?, args, kwargs, context}`. `context` declares the machine, with the original collector's
//! members: `mem` (the figures `gate` and the command line read, one per read), `drop` (what the one cache-drop attempt answers),
//! `callback` (`"default"`, `"collect"`, or `{raise_on, type, message}` for a callable log that raises on its n-th call),
//! `kind: "drop-provider"` with `helper_exists` and `run_outcome` (for `try_drop_caches` itself), and for `mem_available_mib` the
//! file `<root>/meminfo` that stands in for `/proc/meminfo`. Text containing `@CASE@` is rebased onto the case root. A provider
//! that the case did not describe, or that ran out of figures, is never answered from the real machine: it is a boundary
//! violation, the call is recorded as a collector error, and the program ends with status 77.

use crate::cli::{emit, Run};
use crate::memory::{
    DropSystem, HelperRequest, HelperRun, Host, LogChoice, MeminfoSource, Provider, DROP_HELPER,
    DROP_REQUEST, MEMINFO_PATH,
};
use crate::memory_api::{Environment, Function};
use crate::memory_cli::{execute, print_to_stream, uncaught_block, write_real_stderr, CliHost};
use crate::memory_system::{startup_stdio, FileMeminfo, StdioAtStart};
use crate::model_api::encode;
use crate::model_cli::Streams;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Read;

/// The exit status of a run that ended in a boundary violation.
pub const BOUNDARY_STATUS: i32 = 77;

/// Exception classes a callable log may be told to raise.
const CALLBACK_CLASSES: [&str; 7] = [
    "RuntimeError",
    "ValueError",
    "TypeError",
    "OSError",
    "KeyError",
    "AssertionError",
    "Exception",
];

/// How the case's log behaves.
#[derive(Clone)]
enum Callback {
    /// No callable: the caller's `log` argument (if any) is data.
    Default,
    /// A callable that records each message; the original value of `callback` is kept for the arguments-after projection.
    Callable {
        shown: PyValue,
        raise_on: Option<(u64, &'static str, PyStr)>,
    },
}

/// The machine of one case, and everything it observed.
pub struct Controlled {
    context: PyDict,
    meminfo: FileMeminfo,
    callback: Callback,
    trace: Vec<PyValue>,
    mem_consumed: u64,
    drop_calls: u64,
    callback_calls: u64,
    boundary: Option<String>,
    /// Text of the current standard-error line that is not complete yet (no event is recorded for it).
    pending: PyStr,
    /// Everything written to standard error through the program's own streams.
    stderr: PyStr,
    live_stderr: bool,
    start: StdioAtStart,
    streams: Streams,
}

fn event(name: &str, fields: Vec<(&str, PyValue)>) -> PyValue {
    let mut row = PyDict::new();
    row.set("event", PyValue::str(name));
    for (key, value) in fields {
        row.set(key, value);
    }
    PyValue::Dict(row)
}

fn boundary_failure(message: &str) -> Failure {
    Failure::new("BoundaryViolation", message)
}

impl Controlled {
    /// `root` is the case root (`@CASE@`); `live_stderr` passes standard error through as it is written (the command line).
    pub fn new(context: PyDict, root: &PyStr, live_stderr: bool) -> Res<Controlled> {
        let callback = match context.get("callback") {
            None => Callback::Default,
            Some(PyValue::Str(name)) if name.equals("default") => Callback::Default,
            Some(shown @ PyValue::Str(_)) => Callback::Callable {
                shown: shown.clone(),
                raise_on: None,
            },
            Some(shown @ PyValue::Dict(behavior)) => {
                let (
                    Some(PyValue::Int(at)),
                    Some(PyValue::Str(class)),
                    Some(PyValue::Str(message)),
                ) = (
                    behavior.get("raise_on"),
                    behavior.get("type"),
                    behavior.get("message"),
                )
                else {
                    return Err(boundary_failure("invalid callback behaviour"));
                };
                let class = CALLBACK_CLASSES
                    .iter()
                    .find(|known| class.equals(known))
                    .ok_or_else(|| boundary_failure("unknown callback exception type"))?;
                let at = u64::try_from(at.clone())
                    .map_err(|_| boundary_failure("invalid callback behaviour"))?;
                Callback::Callable {
                    shown: shown.clone(),
                    raise_on: Some((at, class, message.clone())),
                }
            }
            Some(_) => return Err(boundary_failure("invalid callback behaviour")),
        };
        let mut meminfo_path = root.clone();
        meminfo_path.push_str("/meminfo");
        Ok(Controlled {
            context,
            meminfo: FileMeminfo::new(meminfo_path),
            callback,
            trace: Vec::new(),
            mem_consumed: 0,
            drop_calls: 0,
            callback_calls: 0,
            boundary: None,
            pending: PyStr::default(),
            stderr: PyStr::default(),
            live_stderr,
            start: if live_stderr {
                startup_stdio()
            } else {
                StdioAtStart::OPEN
            },
            streams: Streams::default(),
        })
    }

    /// The first boundary violation of the run, if any.
    pub fn boundary(&self) -> Option<&str> {
        self.boundary.as_deref()
    }

    fn violate(&mut self, message: &str) -> Failure {
        if self.boundary.is_none() {
            self.boundary = Some(message.to_string());
        }
        boundary_failure(message)
    }

    /// The caller's `log` is a recording callable.
    pub fn log_choice(&self) -> Option<LogChoice> {
        matches!(self.callback, Callback::Callable { .. }).then_some(LogChoice::Callback)
    }

    /// What the arguments hold in place of the recording callable.
    pub fn callback_reference(&self) -> Option<PyValue> {
        match &self.callback {
            Callback::Default => None,
            Callback::Callable { shown, .. } => {
                let mut reference = PyDict::new();
                reference.set("callback_ref", shown.clone());
                Some(PyValue::Dict(reference))
            }
        }
    }

    /// `trace`, `context_after`: the observations of the run.
    pub fn observations(&self) -> (PyValue, PyValue) {
        let mut state = PyDict::new();
        state.set("mem_consumed", PyValue::int(self.mem_consumed));
        state.set("drop_calls", PyValue::int(self.drop_calls));
        state.set("callback_calls", PyValue::int(self.callback_calls));
        (PyValue::List(self.trace.clone()), PyValue::Dict(state))
    }

    pub fn captured_stderr(&self) -> PyStr {
        self.stderr.clone()
    }

    fn flag(&self, name: &str) -> Option<bool> {
        match self.context.get(name) {
            Some(PyValue::Bool(value)) => Some(*value),
            _ => None,
        }
    }

    fn is_drop_provider(&self) -> bool {
        matches!(self.context.get("kind"), Some(PyValue::Str(kind)) if kind.equals("drop-provider"))
    }
}

impl Provider for Controlled {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        let Some(PyValue::List(figures)) = self.context.get("mem") else {
            return Err(self.violate("unspecified mem provider access"));
        };
        let Some(value) = figures.get(self.mem_consumed as usize).cloned() else {
            return Err(self.violate("mem provider sequence exhausted"));
        };
        self.mem_consumed += 1;
        self.trace
            .push(event("mem", vec![("value", value.clone())]));
        Ok(value)
    }

    fn try_drop_caches(&mut self) -> Res<bool> {
        let Some(value) = self.flag("drop") else {
            return Err(self.violate("unspecified or invalid drop provider access"));
        };
        self.drop_calls += 1;
        self.trace
            .push(event("drop", vec![("value", PyValue::Bool(value))]));
        Ok(value)
    }
}

impl MeminfoSource for Controlled {
    fn open(&mut self) -> Res<Box<dyn Read>> {
        let mut keywords = PyDict::new();
        keywords.set("encoding", PyValue::str("ascii"));
        self.trace.push(event(
            "open",
            vec![
                ("path", PyValue::str(MEMINFO_PATH)),
                ("args", PyValue::List(Vec::new())),
                ("kwargs", PyValue::Dict(keywords)),
            ],
        ));
        self.meminfo.open()
    }
}

impl DropSystem for Controlled {
    fn helper_exists(&mut self) -> bool {
        if !self.is_drop_provider() {
            self.violate("unspecified helper exists request");
            return false;
        }
        let Some(value) = self.flag("helper_exists") else {
            self.violate("missing or invalid helper existence result");
            return false;
        };
        self.trace.push(event(
            "exists",
            vec![
                ("path", PyValue::str(DROP_HELPER)),
                ("value", PyValue::Bool(value)),
            ],
        ));
        value
    }

    fn run_helper(&mut self, request: &HelperRequest) -> HelperRun {
        if !self.is_drop_provider() || *request != DROP_REQUEST {
            self.violate("unexpected subprocess request");
            return HelperRun::Failed;
        }
        let Some(PyValue::Dict(outcome)) = self.context.get("run_outcome") else {
            self.violate("unspecified run provider outcome");
            return HelperRun::Failed;
        };
        let exception = match (
            outcome.len(),
            outcome.get("exception"),
            outcome.get("returncode"),
        ) {
            (1, Some(PyValue::Str(name)), None)
                if [
                    "FileNotFoundError",
                    "PermissionError",
                    "TimeoutExpired",
                    "OSError",
                ]
                .iter()
                .any(|known| name.equals(known)) =>
            {
                true
            }
            (1, None, Some(PyValue::Int(_))) => false,
            _ => {
                self.violate("missing or invalid run provider result");
                return HelperRun::Failed;
            }
        };
        let argv: Vec<PyValue> = request.argv.iter().map(|part| PyValue::str(part)).collect();
        self.trace.push(event(
            "run",
            vec![
                ("argv", PyValue::List(argv)),
                ("stdout", PyValue::str("DEVNULL")),
                ("stderr", PyValue::str("DEVNULL")),
                ("timeout", PyValue::int(request.timeout_seconds)),
            ],
        ));
        if exception {
            return HelperRun::Failed;
        }
        match outcome.get("returncode") {
            Some(PyValue::Int(code)) => {
                i64::try_from(code.clone()).map_or(HelperRun::Failed, HelperRun::Exited)
            }
            _ => HelperRun::Failed,
        }
    }
}

impl Host for Controlled {
    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        // A write that fails is not an event: nothing is recorded for it and the failure ends the call where the write was made. With
        // no standard error at start (command line only) the text is printed on the standard output stream, or nowhere: it is not
        // standard error, so it is neither an event nor captured stderr.
        if self.live_stderr {
            if !self.start.stderr_open {
                if self.start.stdout_open {
                    print_to_stream(&mut self.streams, text);
                }
                return Ok(());
            }
            write_real_stderr(text)?;
        }
        self.stderr.push_py(text);
        self.pending.push_py(text);
        let newline = u32::from('\n');
        loop {
            let at = self
                .pending
                .code_points()
                .iter()
                .position(|&point| point == newline);
            let Some(at) = at else { break };
            let points = self.pending.code_points().to_vec();
            let line = PyStr::from_code_points(points[..=at].to_vec());
            self.pending = PyStr::from_code_points(points[at + 1..].to_vec());
            self.trace
                .push(event("stderr", vec![("text", PyValue::Str(line))]));
        }
        Ok(())
    }

    fn call_log(&mut self, message: &PyStr) -> Res<()> {
        self.callback_calls += 1;
        self.trace.push(event(
            "callback",
            vec![("message", PyValue::Str(message.clone()))],
        ));
        if let Callback::Callable {
            raise_on: Some((at, class, text)),
            ..
        } = &self.callback
        {
            if self.callback_calls == *at {
                return Err(Failure::with_message(class, text.clone()));
            }
        }
        Ok(())
    }
}

impl Environment for Controlled {
    fn meminfo(&mut self) -> &mut dyn MeminfoSource {
        self
    }

    fn drops(&mut self) -> &mut dyn DropSystem {
        self
    }
}

impl CliHost for Controlled {
    fn write_stdout(&mut self, text: &PyStr) {
        self.streams.write(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        if self.start.stderr_open {
            // Two suppressed writes: a failed one is not an event and does not stop the other.
            for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
                let _ = self.write_stderr(text);
            }
        } else if self.start.stdout_open && !usage.is_empty() {
            self.streams.write(usage);
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        if self.start.stdout_open {
            self.streams.write(help);
        } else if self.start.stderr_open {
            let _ = self.write_stderr(help);
        }
    }

    fn write_uncaught(&mut self, text: &PyStr) {
        // The interpreter's report after the program stopped: a failure to write it is ignored and nothing restarts.
        if self.start.stderr_open {
            let _ = write_real_stderr(text);
        }
    }
}

// ── cases ────────────────────────────────────────────────────────────────────────────────────────

/// `@CASE@` replaced by the case root, in every text of the value.
fn rebase(value: &PyValue, root: &PyStr) -> PyValue {
    match value {
        PyValue::Str(text) => {
            let marker: Vec<u32> = "@CASE@".chars().map(u32::from).collect();
            let points = text.code_points();
            let mut out = Vec::with_capacity(points.len());
            let mut at = 0;
            while at < points.len() {
                if points[at..].starts_with(&marker) {
                    out.extend_from_slice(root.code_points());
                    at += marker.len();
                } else {
                    out.push(points[at]);
                    at += 1;
                }
            }
            PyValue::Str(PyStr::from_code_points(out))
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(|item| rebase(item, root)).collect())
        }
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, item) in map.iter() {
                copy.insert(key.clone(), rebase(item, root));
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

/// A case read from its JSON text.
pub struct Case {
    pub id: PyValue,
    function: Option<String>,
    args: Vec<PyValue>,
    kwargs: PyDict,
    context: PyDict,
}

fn spec_error(message: &str) -> Failure {
    Failure::new("SpecError", message)
}

impl Case {
    pub fn parse(text: &PyStr) -> Res<Case> {
        let PyValue::Dict(members) = loads_text_objects(text)
            .map_err(|error| spec_error(&format!("the spec is not JSON: {error}")))?
        else {
            return Err(spec_error("the spec is not an object"));
        };
        let id = members.get("id").cloned().unwrap_or(PyValue::None);
        let function = match members.get("function") {
            Some(PyValue::Str(name)) => name.to_utf8(),
            _ => None,
        };
        let args = match members.get("args") {
            Some(PyValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let kwargs = match members.get("kwargs") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let context = match members.get("context") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => return Err(spec_error("the spec has no context object")),
        };
        Ok(Case {
            id,
            function,
            args,
            kwargs,
            context,
        })
    }

    /// The machine this case describes.
    pub fn machine(&self, root: &PyStr, live_stderr: bool) -> Res<Controlled> {
        Controlled::new(self.context.clone(), root, live_stderr)
    }

    pub fn function(&self) -> Option<&str> {
        self.function.as_deref()
    }

    /// The command-line arguments of a command-line case, rebased onto `root`.
    pub fn cli_arguments(&self, root: &PyStr) -> Res<Vec<PyStr>> {
        self.args
            .iter()
            .map(|argument| match rebase(argument, root) {
                PyValue::Str(text) => Ok(text),
                _ => Err(spec_error("a command-line argument is not text")),
            })
            .collect()
    }
}

fn function_named(name: &str) -> Result<&'static Function, Failure> {
    crate::memory_api::resolve(&format!("{}.{name}", crate::memory_api::MODULE))
}

fn finish_record(
    mut row: PyDict,
    machine: &Controlled,
    streams: Option<(PyValue, PyValue, PyValue, PyValue)>,
) -> PyValue {
    let (trace, context_after) = machine.observations();
    row.set("trace", trace);
    row.set("context_after", context_after);
    if let Some((stdout, stderr, args_after, kwargs_after)) = streams {
        row.set("stdout", stdout);
        row.set("stderr", stderr);
        row.set("args_after", args_after);
        row.set("kwargs_after", kwargs_after);
    }
    PyValue::Dict(row)
}

/// Runs one API case in `root`: the control record (the original collector's, without the Python routing) and whether a boundary
/// was violated.
pub fn run_api_case(case: &Case, root: &PyStr) -> Res<(PyValue, bool)> {
    let mut machine = case.machine(root, false)?;
    let PyValue::List(mut args) = rebase(&PyValue::List(case.args.clone()), root) else {
        unreachable!("a list stays a list")
    };
    let PyValue::Dict(mut kwargs) = rebase(&PyValue::Dict(case.kwargs.clone()), root) else {
        unreachable!("a dict stays a dict")
    };
    let function = function_named(case.function().unwrap_or(""));
    let log = match &function {
        Ok(function) if function.name == "gate" => machine.log_choice(),
        _ => None,
    };
    if machine.log_choice().is_some() {
        kwargs.set("log", PyValue::None);
    }
    let outcome =
        function.and_then(|function| function.call(&mut args, &mut kwargs, &mut machine, log));
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    if let Some(message) = machine.boundary() {
        row.set("collector_error", PyValue::str(message));
        let streams = (
            PyValue::str(""),
            PyValue::Str(machine.captured_stderr()),
            PyValue::List(Vec::new()),
            PyValue::Dict(PyDict::new()),
        );
        return Ok((finish_record(row, &machine, Some(streams)), true));
    }
    match &outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(value));
        }
        Err(failure) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(failure.class));
            row.set("message", PyValue::Str(failure.message.clone()));
        }
    }
    if let Some(reference) = machine.callback_reference() {
        if kwargs.get("log").is_some() {
            kwargs.set("log", reference);
        }
    }
    let streams = (
        PyValue::str(""),
        PyValue::Str(machine.captured_stderr()),
        encode(&PyValue::List(args)),
        encode(&PyValue::Dict(kwargs)),
    );
    Ok((finish_record(row, &machine, Some(streams)), false))
}

/// Runs one command-line case in `root` (standard streams are the real ones): the exit status and the control record.
pub fn run_cli_case(
    case: &Case,
    root: &PyStr,
    args: &[PyStr],
    skill_root: &PyStr,
    columns: Option<&str>,
) -> Res<(i32, PyValue)> {
    let mut machine = case.machine(root, true)?;
    let outcome = execute(&mut machine, skill_root, args, columns);
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    let status = match &outcome {
        Ok(status) => {
            row.set("kind", PyValue::str("exit"));
            row.set("type", PyValue::str("SystemExit"));
            row.set("message", PyValue::str(&status.to_string()));
            row.set("status", PyValue::int(*status));
            *status
        }
        Err(failure) => {
            machine.write_uncaught(&uncaught_block(failure));
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(failure.class));
            row.set("message", PyValue::Str(failure.message.clone()));
            row.set("status", PyValue::int(1));
            1
        }
    };
    let streams = std::mem::take(&mut machine.streams);
    let status = emit(&Run {
        status,
        stdout: streams.stdout,
        stderr: PyStr::default(),
        writes: streams.writes,
    });
    if let Some(message) = machine.boundary() {
        let mut collector = PyDict::new();
        collector.set("id", case.id.clone());
        collector.set("collector_error", PyValue::str(message));
        return Ok((BOUNDARY_STATUS, finish_record(collector, &machine, None)));
    }
    Ok((status, finish_record(row, &machine, None)))
}
