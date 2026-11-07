//! The controlled run of the escalation owner for the checks: the same library over an owned root, with the original collector's
//! control record. Only with the `fixture` feature; no production program has it.
//!
//! The root is `<root>/node/.claude/skills/{vllm-recipe-explorer,adversarial-benchmark}` (the layout the original's fixture builds):
//! the classifier asset, its launcher and the event files are plain files of that tree, and the launcher's environment choices are the
//! caller's environment, as in the original. A case is `{id, function?, args, kwargs, context}`; `context` may add to the globals
//! (`model_add`, `host_add`) or replace `refuted` before the call. `@CASE@` in a text is the root. Nothing here is a classifier: every
//! launcher call is the existing owner's.

use crate::cli::{emit, Run};
use crate::escalation::{Dependency, Loaded, Raised, Vocabulary};
use crate::escalation_api::call_judge;
use crate::escalation_bridge::Bridge;
use crate::escalation_cli::{execute, CliHost};
use crate::memory_cli::{print_to_stream, write_real_stderr};
use crate::memory_system::{startup_stdio, StdioAtStart};
use crate::model_api::encode;
use crate::model_cli::Streams;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;

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
        let kwargs = match members.get("kwargs") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let context = match members.get("context") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        Ok(Case {
            id: members.get("id").cloned().unwrap_or(PyValue::None),
            args: list("args"),
            kwargs,
            context,
        })
    }

    /// The globals this case calls with: the module's, plus what the context adds.
    pub fn vocabulary(&self) -> Vocabulary {
        let mut vocabulary = Vocabulary::canonical();
        for (name, target) in [("model_add", true), ("host_add", false)] {
            if let Some(PyValue::Dict(added)) = self.context.get(name) {
                let table = if target {
                    &mut vocabulary.model
                } else {
                    &mut vocabulary.host
                };
                for (key, value) in added.iter() {
                    table.insert(key.clone(), value.clone());
                }
            }
        }
        if let Some(PyValue::List(items)) = self.context.get("refuted") {
            vocabulary.refuted = items.clone();
        }
        vocabulary
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

/// `<root>/node/.claude/skills/vllm-recipe-explorer`.
pub fn skill_root(root: &PyStr) -> PyStr {
    let mut path = root.clone();
    path.push_str("/node/.claude/skills/vllm-recipe-explorer");
    path
}

/// The recording host of one run.
pub struct Controlled {
    bridge: Bridge,
    streams: Streams,
    start: StdioAtStart,
    live: bool,
    captured: PyStr,
    pending: PyStr,
}

impl Controlled {
    pub fn new(root: &PyStr, live: bool) -> Controlled {
        Controlled {
            bridge: Bridge::installed(&skill_root(root)).recording(),
            streams: Streams::default(),
            start: if live {
                startup_stdio()
            } else {
                StdioAtStart::OPEN
            },
            live,
            captured: PyStr::default(),
            pending: PyStr::default(),
        }
    }

    fn stderr_event(&mut self, text: &PyStr) {
        self.captured.push_py(text);
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
            self.bridge
                .note("stderr", vec![("text", PyValue::Str(line))]);
        }
    }

    fn observations(&self) -> (PyValue, PyValue) {
        (
            PyValue::List(self.bridge.trace().to_vec()),
            PyValue::List(self.bridge.rpc().to_vec()),
        )
    }
}

impl Dependency for Controlled {
    fn load(&mut self) -> Result<Loaded, Raised> {
        self.bridge.load()
    }

    fn utc(&mut self, value: &PyStr, field: &str) -> Result<PyValue, Raised> {
        self.bridge.utc(value, field)
    }

    fn kill_events(
        &mut self,
        lines: &[PyStr],
        started: &PyValue,
        ended: &PyValue,
        tolerance_s: &BigInt,
    ) -> Result<PyValue, Raised> {
        self.bridge.kill_events(lines, started, ended, tolerance_s)
    }

    fn take_stderr(&mut self) -> Vec<PyStr> {
        self.bridge.take_stderr()
    }

    fn record_event(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.bridge.note(name, fields);
    }
}

impl CliHost for Controlled {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        if self.live && !self.start.stderr_open {
            if self.start.stdout_open {
                print_to_stream(&mut self.streams, text);
            }
            return Ok(());
        }
        if self.live {
            write_real_stderr(text)?;
        }
        self.stderr_event(text);
        Ok(())
    }

    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        if self.live && !self.start.stderr_open {
            return Err(Failure::new(
                "AttributeError",
                "'NoneType' object has no attribute 'write'",
            ));
        }
        if self.live {
            write_real_stderr(text)?;
        }
        self.stderr_event(text);
        Ok(())
    }

    fn write_stdout(&mut self, text: &PyStr) {
        self.streams.write(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        if self.start.stderr_open {
            for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
                let _ = self.print_stderr_raw(text);
            }
        } else if self.start.stdout_open && !usage.is_empty() {
            self.streams.write(usage);
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        if self.start.stdout_open {
            self.streams.write(help);
        } else if self.start.stderr_open {
            let _ = self.print_stderr_raw(help);
        }
    }

    fn write_uncaught(&mut self, text: &PyStr) {
        if self.start.stderr_open {
            let _ = write_real_stderr(text);
        }
    }
}

impl Controlled {
    /// A write to standard error that is an event when it succeeded (the parser's own writes).
    fn print_stderr_raw(&mut self, text: &PyStr) -> Res<()> {
        if self.live {
            write_real_stderr(text)?;
        }
        self.stderr_event(text);
        Ok(())
    }
}

fn finish(
    mut row: PyDict,
    host: &Controlled,
    vocabulary: &Vocabulary,
    streams: Option<(PyStr, Vec<PyValue>, PyDict)>,
) -> PyValue {
    let (trace, rpc) = host.observations();
    row.set("trace", trace);
    row.set("rpc", rpc);
    row.set("globals_before", vocabulary.to_value());
    row.set("globals_after", vocabulary.to_value());
    if let Some((stderr, args, kwargs)) = streams {
        row.set("stdout", PyValue::str(""));
        row.set("stderr", PyValue::Str(stderr));
        row.set("args_after", encode(&PyValue::List(args)));
        row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    }
    PyValue::Dict(row)
}

/// Runs one API case in `root`: the control record in the original collector's format.
pub fn run_api_case(case: &Case, root: &PyStr) -> PyValue {
    let mut host = Controlled::new(root, false);
    let vocabulary = case.vocabulary();
    let PyValue::List(args) = rebase(&PyValue::List(case.args.clone()), root) else {
        unreachable!("a list stays a list")
    };
    let PyValue::Dict(kwargs) = rebase(&PyValue::Dict(case.kwargs.clone()), root) else {
        unreachable!("a dict stays a dict")
    };
    let outcome = call_judge(&mut host, &vocabulary, &args, &kwargs);
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
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
    let stderr = host.captured.clone();
    finish(row, &host, &vocabulary, Some((stderr, args, kwargs)))
}

/// Runs one command-line case in `root` (the standard streams are the real ones): the exit status and the control record.
pub fn run_cli_case(
    case: &Case,
    root: &PyStr,
    args: &[PyStr],
    columns: Option<&str>,
) -> (i32, PyValue) {
    let mut host = Controlled::new(root, true);
    let vocabulary = case.vocabulary();
    let outcome = execute(&mut host, &vocabulary, args, columns);
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    let status = match outcome {
        Ok(done) => {
            let status = done.status;
            let mut exit = Raised::new("SystemExit", "builtins", PyStr::new(&status.to_string()));
            exit.code = Some(Box::new(PyValue::int(status)));
            if let Some(context) = done.context {
                exit.cause = Some(Box::new(context));
                exit.via_context = true;
            }
            row.set("kind", PyValue::str("exit"));
            row.set("type", PyValue::str("SystemExit"));
            row.set("message", PyValue::Str(exit.message.clone()));
            row.set("exception_chain", exit.chain_value());
            row.set("status", PyValue::int(status));
            status
        }
        Err(raised) => {
            let report = if raised.class == "SystemExit" {
                let mut text = raised.message.clone();
                text.push_str("\n");
                text
            } else {
                raised.report()
            };
            host.write_uncaught(&report);
            row.set(
                "kind",
                PyValue::str(if raised.class == "SystemExit" {
                    "exit"
                } else {
                    "exception"
                }),
            );
            row.set("type", PyValue::str(&raised.class));
            row.set("message", PyValue::Str(raised.message.clone()));
            row.set("exception_chain", raised.chain_value());
            row.set("status", PyValue::int(1));
            1
        }
    };
    let streams = std::mem::take(&mut host.streams);
    let status = emit(&Run {
        status,
        stdout: streams.stdout,
        stderr: PyStr::default(),
        writes: streams.writes,
    });
    (status, finish(row, &host, &vocabulary, None))
}
