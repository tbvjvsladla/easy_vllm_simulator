//! The controlled run of `functional_smoke` for the checks: the same library, with the original collector's control record. Only with
//! the `fixture` feature; no production program has it.
//!
//! A case is `{id, owner: "functional_smoke", function?, args, kwargs, context}`; `@CASE@` and `@BASE@` in any text of `args`/`kwargs`
//! become the case root and the owned peer's base URL. `context.python_script_path` is the Python import routing and does not exist
//! here (the record has no `python_routing`); it is accepted and ignored. The HTTP is real (the base URL is the caller's own peer); the
//! record's `trace` lists the HTTP steps and, for the command line, the file reads and the standard-error lines in order.

use crate::cli::{emit, Run};
use crate::escalation::Raised;
use crate::functional_cli::execute;
use crate::functional_smoke::{call, Constants, Function, Observe, MODULE};
use crate::memory_cli::{print_to_stream, write_real_stderr};
use crate::memory_system::{startup_stdio, StdioAtStart};
use crate::model_api::encode;
use crate::model_cli::Streams;
use crate::pyexpr::Res;
use crate::transform_cli::{finish, Console, Exit};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

fn rebase(value: &PyValue, root: &PyStr, base: &PyStr) -> PyValue {
    match value {
        PyValue::Str(text) => {
            let replace = |points: &[u32], marker: &str, with: &PyStr| -> Vec<u32> {
                let marker: Vec<u32> = marker.chars().map(u32::from).collect();
                let mut out = Vec::with_capacity(points.len());
                let mut at = 0;
                while at < points.len() {
                    if points[at..].starts_with(&marker) {
                        out.extend_from_slice(with.code_points());
                        at += marker.len();
                    } else {
                        out.push(points[at]);
                        at += 1;
                    }
                }
                out
            };
            let first = replace(text.code_points(), "@CASE@", root);
            PyValue::Str(PyStr::from_code_points(replace(&first, "@BASE@", base)))
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(|item| rebase(item, root, base)).collect())
        }
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, item) in map.iter() {
                copy.insert(key.clone(), rebase(item, root, base));
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
    function: Option<PyStr>,
    args: Vec<PyValue>,
    kwargs: PyDict,
}

impl Case {
    pub fn parse(text: &PyStr) -> Res<Case> {
        let PyValue::Dict(members) = loads_text_objects(text)
            .map_err(|error| spec_error(&format!("the spec is not JSON: {error}")))?
        else {
            return Err(spec_error("the spec is not an object"));
        };
        if !matches!(members.get("owner"), Some(PyValue::Str(owner)) if owner.equals(MODULE)) {
            return Err(spec_error("the owner is functional_smoke"));
        }
        if let Some(PyValue::Dict(context)) = members.get("context") {
            if context
                .iter()
                .any(|(key, _)| !key.equals("python_script_path"))
            {
                return Err(spec_error("the context holds python_script_path only"));
            }
        }
        let list = |key: &str| match members.get(key) {
            Some(PyValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let kwargs = match members.get("kwargs") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let function = match members.get("function") {
            Some(PyValue::Str(name)) => Some(name.clone()),
            _ => None,
        };
        Ok(Case {
            id: members.get("id").cloned().unwrap_or(PyValue::None),
            function,
            args: list("args"),
            kwargs,
        })
    }

    pub fn cli_arguments(&self, root: &PyStr, base: &PyStr) -> Res<Vec<PyStr>> {
        self.args
            .iter()
            .map(|argument| match rebase(argument, root, base) {
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

/// The recording observer of an API run.
struct Recorder {
    trace: Vec<PyValue>,
}

impl Observe for Recorder {
    fn enabled(&self) -> bool {
        true
    }

    fn record(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.trace.push(event(name, fields));
    }
}

fn finished(
    mut row: PyDict,
    trace: Vec<PyValue>,
    streams: Option<(Vec<PyValue>, PyDict)>,
) -> PyValue {
    let constants = Constants::canonical().to_value();
    row.set("trace", PyValue::List(trace));
    row.set("globals_before", constants.clone());
    row.set("globals_after", constants);
    if let Some((args, kwargs)) = streams {
        row.set("stdout", PyValue::str(""));
        row.set("stderr", PyValue::str(""));
        row.set("args_after", encode(&PyValue::List(args)));
        row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    }
    PyValue::Dict(row)
}

/// Runs one API case: the control record in the original collector's format (without `python_routing`).
pub fn run_api_case(case: &Case, root: &PyStr, base: &PyStr) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(MODULE));
    let PyValue::List(args) = rebase(&PyValue::List(case.args.clone()), root, base) else {
        unreachable!("a list stays a list")
    };
    let PyValue::Dict(kwargs) = rebase(&PyValue::Dict(case.kwargs.clone()), root, base) else {
        unreachable!("a dict stays a dict")
    };
    let mut recorder = Recorder { trace: Vec::new() };
    let function = case
        .function
        .as_ref()
        .and_then(|name| name.to_utf8())
        .as_deref()
        .and_then(Function::named)
        .ok_or_else(|| {
            Raised::plain(
                "AttributeError",
                &format!(
                    "module '{MODULE}' has no attribute {}",
                    case.function
                        .as_ref()
                        .map_or_else(|| "None".to_string(), PyStr::repr)
                ),
            )
        });
    let outcome = function.and_then(|function| {
        call(
            function,
            &mut recorder,
            &Constants::canonical(),
            &args,
            &kwargs,
        )
    });
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(&value));
        }
        Err(raised) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(&raised.class));
            row.set("message", PyValue::Str(raised.message.clone()));
            row.set("exception_chain", raised.chain_value());
        }
    }
    finished(row, recorder.trace, Some((args, kwargs)))
}

/// The recording console of a command-line run.
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

/// Runs one command-line case (the standard streams are the real ones): the exit status and the control record.
pub fn run_cli_case(case: &Case, args: &[PyStr], columns: Option<&str>) -> (i32, PyValue) {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(MODULE));
    let mut console = Controlled::new();
    let outcome = execute(&mut console, args, columns);
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
    (status, finished(row, trace, None))
}
