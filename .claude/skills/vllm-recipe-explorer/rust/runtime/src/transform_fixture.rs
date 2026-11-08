//! The controlled run of the two transform owners for the checks: the same library over an owned root, with the original collector's
//! control record. Only with the `fixture` feature; no production program has it.
//!
//! A case is `{id, owner, function?, args, kwargs, context, files?}` with `owner` one of `rope_scaling_translate` and `parse_vllm_log`.
//! `context` may replace the rope module's `ROPE_CONTAINERS` / `EXTENDABLE_ROPE_TYPES` before the call (the log owner takes none).
//! `@CASE@` in a text is the root; the files a case reads are plain files of the root that the caller wrote, and standard input is the
//! process's own. The control record records, in order, every file open and read, every standard-input read and every complete line
//! written to standard error. Nothing here is a transformation: every call is the existing owners'.

use crate::cli::{emit, Run};
use crate::escalation::Raised;
use crate::memory_cli::{print_to_stream, write_real_stderr};
use crate::memory_system::{startup_stdio, StdioAtStart};
use crate::model_api::encode;
use crate::model_cli::Streams;
use crate::pyexpr::Res;
use crate::rope_translate::{Profile, MODULE as ROPE_MODULE};
use crate::transform_api::{call, constants, Function};
use crate::transform_cli::{finish, log_execute, rope_execute, Console, Exit};
use crate::vllm_log::MODULE as LOG_MODULE;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

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

fn spec_error(message: &str) -> Failure {
    Failure::new("SpecError", message)
}

/// A case read from its JSON text.
pub struct Case {
    pub id: PyValue,
    owner: PyStr,
    function: PyStr,
    args: Vec<PyValue>,
    kwargs: PyDict,
    context: PyDict,
}

impl Case {
    pub fn parse(text: &PyStr) -> Res<Case> {
        let PyValue::Dict(members) = loads_text_objects(text)
            .map_err(|error| spec_error(&format!("the spec is not JSON: {error}")))?
        else {
            return Err(spec_error("the spec is not an object"));
        };
        let list = |key: &str| match members.get(key) {
            Some(PyValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let table = |key: &str| match members.get(key) {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let text_member = |key: &str| match members.get(key) {
            Some(PyValue::Str(text)) => text.clone(),
            _ => PyStr::default(),
        };
        let owner = text_member("owner");
        if !owner.equals(ROPE_MODULE) && !owner.equals(LOG_MODULE) {
            return Err(spec_error(
                "the owner is rope_scaling_translate or parse_vllm_log",
            ));
        }
        Ok(Case {
            id: members.get("id").cloned().unwrap_or(PyValue::None),
            owner,
            function: text_member("function"),
            args: list("args"),
            kwargs: table("kwargs"),
            context: table("context"),
        })
    }

    fn is_rope(&self) -> bool {
        self.owner.equals(ROPE_MODULE)
    }

    /// The globals this case calls with: the module's, plus what the context replaces.
    fn profile(&self) -> Res<Profile> {
        let mut profile = Profile::canonical();
        if self.is_rope() {
            if !self.context.is_empty() {
                profile
                    .replace_from(&PyValue::Dict(self.context.clone()))
                    .map_err(|problem| spec_error(&problem))?;
            }
        } else if !self.context.is_empty() {
            return Err(spec_error("the log owner's globals are not replaceable"));
        }
        Ok(profile)
    }

    /// The module globals as the collector records them.
    fn globals_value(&self, profile: &Profile) -> PyValue {
        let PyValue::Dict(all) = constants() else {
            unreachable!("the constants are an object")
        };
        if self.is_rope() {
            profile.to_value()
        } else {
            all.get(LOG_MODULE).cloned().unwrap_or(PyValue::None)
        }
    }

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

fn event(name: &str, fields: Vec<(&str, PyValue)>) -> PyValue {
    let mut row = PyDict::new();
    row.set("event", PyValue::str(name));
    for (key, value) in fields {
        row.set(key, value);
    }
    PyValue::Dict(row)
}

/// The recording console of one run.
struct Controlled {
    streams: Streams,
    start: StdioAtStart,
    trace: Vec<PyValue>,
    pending: PyStr,
}

impl Controlled {
    fn new() -> Controlled {
        Controlled {
            streams: Streams::default(),
            start: startup_stdio(),
            trace: Vec::new(),
            pending: PyStr::default(),
        }
    }

    fn stderr_event(&mut self, text: &PyStr) {
        self.pending.push_py(text);
        let newline = u32::from('\n');
        while let Some(at) = self
            .pending
            .code_points()
            .iter()
            .position(|&point| point == newline)
        {
            let points = self.pending.code_points().to_vec();
            let line = PyStr::from_code_points(points[..=at].to_vec());
            self.pending = PyStr::from_code_points(points[at + 1..].to_vec());
            self.trace
                .push(event("stderr", vec![("text", PyValue::Str(line))]));
        }
    }

    /// A write to standard error that is an event when it succeeded.
    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        write_real_stderr(text)?;
        self.stderr_event(text);
        Ok(())
    }
}

impl Console for Controlled {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        if !self.start.stderr_open {
            if self.start.stdout_open {
                print_to_stream(&mut self.streams, text);
            }
            return Ok(());
        }
        self.write_stderr(text)
    }

    fn write_stdout(&mut self, text: &PyStr) {
        self.streams.write(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        if self.start.stderr_open {
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
        if self.start.stderr_open {
            let _ = write_real_stderr(text);
        }
    }

    fn observe(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.trace.push(event(name, fields));
    }

    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        crate::transform_cli::read_process_stdin()
    }
}

fn finished(
    mut row: PyDict,
    trace: Vec<PyValue>,
    globals: &PyValue,
    streams: Option<(Vec<PyValue>, PyDict)>,
) -> PyValue {
    row.set("trace", PyValue::List(trace));
    row.set("globals_before", globals.clone());
    row.set("globals_after", globals.clone());
    if let Some((args, kwargs)) = streams {
        row.set("stdout", PyValue::str(""));
        row.set("stderr", PyValue::str(""));
        row.set("args_after", encode(&PyValue::List(args)));
        row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    }
    PyValue::Dict(row)
}

/// Runs one API case in `root`: the control record in the original collector's format.
pub fn run_api_case(case: &Case, root: &PyStr) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::Str(case.owner.clone()));
    let profile = match case.profile() {
        Ok(profile) => profile,
        Err(failure) => {
            row.set("kind", PyValue::str("collector_error"));
            row.set(
                "message",
                PyValue::str(&failure.line().to_utf8().unwrap_or_default()),
            );
            return PyValue::Dict(row);
        }
    };
    let PyValue::List(args) = rebase(&PyValue::List(case.args.clone()), root) else {
        unreachable!("a list stays a list")
    };
    let PyValue::Dict(kwargs) = rebase(&PyValue::Dict(case.kwargs.clone()), root) else {
        unreachable!("a dict stays a dict")
    };
    let function = match (case.is_rope(), case.function.to_utf8().as_deref()) {
        (true, Some("translate")) => Ok(Function::Translate),
        (false, Some("parse_log")) => Ok(Function::ParseLog),
        _ => Err(Raised::plain(
            "AttributeError",
            &format!(
                "module '{}' has no attribute {}",
                case.owner.to_utf8().unwrap_or_default(),
                case.function.repr()
            ),
        )),
    };
    let outcome = function.and_then(|function| call(function, &profile, &args, &kwargs));
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
    finished(
        row,
        Vec::new(),
        &case.globals_value(&profile),
        Some((args, kwargs)),
    )
}

/// Runs one command-line case in `root` (the standard streams are the real ones): the exit status and the control record.
pub fn run_cli_case(
    case: &Case,
    root: &PyStr,
    args: &[PyStr],
    columns: Option<&str>,
) -> (i32, PyValue) {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::Str(case.owner.clone()));
    let profile = match case.profile() {
        Ok(profile) => profile,
        Err(failure) => {
            row.set("kind", PyValue::str("collector_error"));
            row.set(
                "message",
                PyValue::str(&failure.line().to_utf8().unwrap_or_default()),
            );
            return (2, PyValue::Dict(row));
        }
    };
    let _ = root;
    let mut console = Controlled::new();
    let outcome = if case.is_rope() {
        rope_execute(&mut console, &profile, args, columns)
    } else {
        log_execute(&mut console, args, columns)
    };
    let (status, kind, raised) = match outcome {
        Ok(Exit { status, context }) => {
            let mut exit = Raised::new("SystemExit", "builtins", PyStr::new(&status.to_string()));
            exit.code = Some(Box::new(PyValue::int(status)));
            if let Some(context) = context {
                exit.cause = Some(Box::new(context));
                exit.via_context = true;
            }
            (status, "exit", exit)
        }
        Err(raised) => (1, "exception", raised),
    };
    row.set("kind", PyValue::str(kind));
    row.set("type", PyValue::str(&raised.class));
    row.set("message", PyValue::Str(raised.message.clone()));
    row.set("exception_chain", raised.chain_value());
    row.set("status", PyValue::int(status));
    let uncaught = if kind == "exception" {
        Err(raised)
    } else {
        Ok(Exit {
            status,
            context: None,
        })
    };
    let status = finish(&mut console, uncaught);
    let trace = std::mem::take(&mut console.trace);
    let streams = std::mem::take(&mut console.streams);
    let status = emit(&Run {
        status,
        stdout: streams.stdout,
        stderr: PyStr::default(),
        writes: streams.writes,
    });
    (
        status,
        finished(row, trace, &case.globals_value(&profile), None),
    )
}
