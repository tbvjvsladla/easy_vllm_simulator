//! The controlled run of `cleanup_docker` for the checks: the same library, with the original collector's control record. Only with the
//! `fixture` feature; no production program has it.
//!
//! A case is `{id, owner: "cleanup_docker", function?, args, kwargs, context, ...}`; `@CASE@` in any text of `args`/`kwargs`/`context`
//! becomes the case root. `files`, `process`, `label`, `observation`, `stdin_hex` describe what the CALLER prepared or why the case exists
//! and are never read. The default `REPO` is `ROOT/node` (the layout the collector's source file implies), `context.REPO` replaces it,
//! `context.argv` (API) follows the program name, the command line's `args` are the arguments; `context.python_script_path` is the Python
//! import routing and does not exist here (the record has no `python_routing`). The file reads and the child processes are real: the
//! caller runs the fixture with its own stand-in `docker` as `PATH`.

use crate::cleanup_api::{call, resolve, Capture, Globals};
use crate::cleanup_cli::{conclude, exit_status, run, End, LiveConsole};
use crate::cleanup_docker::{dto, CleanupConsole, Done, Thrown, World, MODULE};
use crate::escalation::Raised;
use crate::memory_cli::write_real_stderr;
use crate::memory_system::{startup_stdio, StdioAtStart};
use crate::model_api::encode;
use crate::model_files::join;
use crate::pyexpr::Res;
use crate::transform_cli::Console;
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
    function: Option<PyStr>,
    args: Vec<PyValue>,
    kwargs: PyDict,
    repo: Option<PyValue>,
    argv: Vec<PyValue>,
}

impl Case {
    pub fn parse(text: &PyStr) -> Res<Case> {
        let PyValue::Dict(members) = loads_text_objects(text)
            .map_err(|error| spec_error(&format!("the spec is not JSON: {error}")))?
        else {
            return Err(spec_error("the spec is not an object"));
        };
        if !matches!(members.get("owner"), Some(PyValue::Str(owner)) if owner.equals(MODULE)) {
            return Err(spec_error("the owner is cleanup_docker"));
        }
        let (mut repo, mut argv) = (None, Vec::new());
        if let Some(PyValue::Dict(context)) = members.get("context") {
            for (key, value) in context.iter() {
                if key.equals("REPO") {
                    repo = Some(value.clone());
                } else if key.equals("argv") {
                    let PyValue::List(items) = value else {
                        return Err(spec_error("context.argv is a list"));
                    };
                    argv = items.clone();
                } else if !key.equals("python_script_path") {
                    return Err(spec_error(
                        "the context holds REPO, argv and python_script_path only",
                    ));
                }
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
            repo,
            argv,
        })
    }

    /// The globals of the case over `root`: the default `REPO` (`root/node`), the script path as `sys.argv[0]`, then `context.argv`.
    fn globals(&self, root: &PyStr) -> Res<Globals> {
        let node = join(root, &PyStr::new("node"));
        let mut script = node.clone();
        for part in [
            ".claude",
            "skills",
            "vllm-recipe-explorer",
            "scripts",
            "cleanup_docker.py",
        ] {
            script = join(&script, &PyStr::new(part));
        }
        let mut argv = vec![script];
        for item in &self.argv {
            match rebase(item, root) {
                PyValue::Str(text) => argv.push(text),
                _ => return Err(spec_error("context.argv holds text only")),
            }
        }
        Ok(Globals {
            repo: self
                .repo
                .as_ref()
                .map_or(PyValue::Str(node), |repo| rebase(repo, root)),
            argv,
        })
    }

    /// The command line's arguments (after the program name), rebased.
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

fn thrown_fields(row: &mut PyDict, thrown: &Thrown) {
    row.set("type", PyValue::str(&thrown.raised.class));
    row.set("message", PyValue::Str(thrown.raised.message.clone()));
    row.set("exception_chain", thrown.chain());
}

/// Runs one API case: the control record in the original collector's format (without `python_routing`).
pub fn run_api_case(case: &Case, root: &PyStr, columns: Option<&str>) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(MODULE));
    let globals = match case.globals(root) {
        Ok(globals) => globals,
        Err(failure) => {
            row.set("kind", PyValue::str("exception"));
            thrown_fields(&mut row, &Thrown::plain(Raised::from(failure)));
            return PyValue::Dict(row);
        }
    };
    let PyValue::List(args) = rebase(&PyValue::List(case.args.clone()), root) else {
        unreachable!("a list stays a list")
    };
    let PyValue::Dict(kwargs) = rebase(&PyValue::Dict(case.kwargs.clone()), root) else {
        unreachable!("a dict stays a dict")
    };
    let mut capture = Capture::new(true);
    let function = case
        .function
        .as_ref()
        .and_then(|name| name.to_utf8())
        .map(|name| format!("{MODULE}.{name}"))
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
        })
        .and_then(|operation| resolve(&operation));
    let outcome: Done<PyValue> = match function {
        Err(raised) => Err(Thrown::plain(raised)),
        Ok(function) => call(
            function,
            &mut World {
                console: &mut capture,
                tracing: true,
            },
            &globals,
            columns,
            &args,
            &kwargs,
        ),
    };
    match &outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", value.clone());
        }
        Err(thrown) => {
            row.set("kind", PyValue::str("exception"));
            thrown_fields(&mut row, thrown);
        }
    }
    row.set("trace", PyValue::List(std::mem::take(&mut capture.trace)));
    row.set("globals_before", globals.to_value());
    row.set("globals_after", globals.to_value());
    row.set("stdout", PyValue::Str(capture.stdout.clone()));
    row.set("stderr", PyValue::Str(capture.stderr.clone()));
    row.set("args_after", encode(&PyValue::List(args)));
    row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    PyValue::Dict(row)
}

/// The recording console of a command-line run: the production console's live standard output, the trace of the steps.
struct Controlled {
    live: LiveConsole,
    start: StdioAtStart,
    trace: Vec<PyValue>,
    pending: PyStr,
}

impl Controlled {
    fn new() -> Controlled {
        Controlled {
            live: LiveConsole::new(),
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

impl CleanupConsole for Controlled {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.live.out(text)
    }

    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        self.live.take_parser_failure()
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        if self.start.stderr_open {
            self.write_stderr(text)
                .map_err(|failure| Thrown::plain(Raised::from(failure)))
        } else {
            self.live.out(text)
        }
    }
}

impl Console for Controlled {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        if self.start.stderr_open {
            self.write_stderr(text)
        } else {
            let _ = self.live.out(text);
            Ok(())
        }
    }

    fn write_stdout(&mut self, text: &PyStr) {
        let _ = self.live.out(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        if self.start.stderr_open {
            for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
                let _ = self.write_stderr(text);
            }
        } else if self.start.stdout_open && !usage.is_empty() {
            self.live.parser_stdout(usage);
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        if self.start.stdout_open {
            self.live.parser_stdout(help);
        } else if self.start.stderr_open {
            let _ = self.write_stderr(help);
        }
    }

    fn write_uncaught(&mut self, text: &PyStr) {
        self.live.write_uncaught(text);
    }

    fn observe(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.trace.push(event(name, fields));
    }

    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        crate::transform_cli::read_process_stdin()
    }
}

/// Runs one command-line case (the standard streams are the real ones): the exit status and the control record.
pub fn run_cli_case(case: &Case, root: &PyStr, columns: Option<&str>) -> (i32, PyValue) {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(MODULE));
    let setup = case.globals(root).and_then(|mut globals| {
        let arguments = case.cli_arguments(root)?;
        globals.argv.truncate(1);
        globals.argv.extend(arguments);
        Ok(globals)
    });
    let globals = match setup {
        Ok(globals) => globals,
        Err(failure) => {
            row.set("kind", PyValue::str("exception"));
            thrown_fields(&mut row, &Thrown::plain(Raised::from(failure)));
            row.set("status", PyValue::int(1));
            return (1, PyValue::Dict(row));
        }
    };
    let mut console = Controlled::new();
    let end = run(&mut console, &globals.repo, &globals.argv, columns, true);
    let status = match &end {
        End::Returned => {
            row.set("kind", PyValue::str("return"));
            row.set("value", dto::none());
            0
        }
        End::Exited(thrown) => {
            row.set("kind", PyValue::str("exit"));
            thrown_fields(&mut row, thrown);
            exit_status(thrown)
        }
        End::Raised(thrown) => {
            row.set("kind", PyValue::str("exception"));
            thrown_fields(&mut row, thrown);
            1
        }
    };
    row.set("status", PyValue::int(status));
    // The process ends as the interpreter ends it: an uncaught exception is reported, the streams are closed (the flush of standard
    // output can still fail: status 120 while the record above says what `main` itself did).
    let status = conclude(&mut console.live, end);
    let trace = std::mem::take(&mut console.trace);
    row.set("trace", PyValue::List(trace));
    row.set("globals_before", globals.to_value());
    row.set("globals_after", globals.to_value());
    (status, PyValue::Dict(row))
}
