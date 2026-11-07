//! `recipe_cleanup_api`: the four `cleanup_docker` operations as a JSON-lines service, wire-compatible with the other recipe services
//! (the transport is `ABI.md` of the R32 round; the final text is in `PORTING.md`).
//!
//! * `constants`: `{"cleanup_docker": {"REPO": <default REPO of the installed binary>}}`.
//! * `evaluate-jsonl`: one request per line `{id, operation, args, kwargs, globals?}` with `operation` one of
//!   `cleanup_docker.{_run, preserve_set, list_images, main}`; one record per request:
//!   `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message, exception_chain, stdout,
//!   stderr, args_after, kwargs_after}`. `globals` is a member of its own (never a business keyword): `REPO` (any JSON value, the module
//!   global as a path value) and `argv` (the whole `sys.argv`, strings). The file reads and the child processes are real.

use crate::cleanup_cli::PROGRAM;
use crate::cleanup_docker::{
    dto, list_images, main, preserve_set, CleanupConsole, Done, Thrown, World, MODULE,
};
use crate::cleanup_process::run_text;
use crate::escalation::Raised;
use crate::memory_api::{parse_request, Request};
use crate::model_api::encode;
use crate::pyexpr::Res;
use crate::transform_cli::Console;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{optional, required, Param, Signature};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

fn sixty() -> PyValue {
    PyValue::int(60)
}

fn falsy() -> PyValue {
    PyValue::Bool(false)
}

const RUN: &[Param] = &[required("cmd"), optional("timeout", sixty)];
const CANONICAL: &[Param] = &[optional("canonical_only", falsy)];
const NOTHING: &[Param] = &[];

/// The four operations.
#[derive(Clone, Copy)]
pub enum Function {
    Run,
    Preserve,
    ListImages,
    Main,
}

impl Function {
    pub fn named(name: &str) -> Option<Function> {
        Some(match name {
            "_run" => Function::Run,
            "preserve_set" => Function::Preserve,
            "list_images" => Function::ListImages,
            "main" => Function::Main,
            _ => return None,
        })
    }

    fn signature(self) -> Signature {
        let (name, params) = match self {
            Function::Run => ("_run", RUN),
            Function::Preserve => ("preserve_set", CANONICAL),
            Function::ListImages => ("list_images", NOTHING),
            Function::Main => ("main", CANONICAL),
        };
        Signature {
            name,
            params,
            rest: false,
        }
    }
}

/// The two module values a call can read: `REPO` and `sys.argv`.
#[derive(Clone)]
pub struct Globals {
    pub repo: PyValue,
    pub argv: Vec<PyStr>,
}

impl Globals {
    /// The defaults: the given default `REPO` and the program name alone.
    pub fn defaults(repo: &PyStr) -> Globals {
        Globals {
            repo: PyValue::Str(repo.clone()),
            argv: vec![PyStr::new(PROGRAM)],
        }
    }

    pub fn to_value(&self) -> PyValue {
        let mut map = PyDict::new();
        map.set("REPO", self.repo.clone());
        map.set(
            "argv",
            PyValue::List(self.argv.iter().cloned().map(PyValue::Str).collect()),
        );
        PyValue::Dict(map)
    }

    /// Takes the members a request's `globals` names; anything else is refused with the reason.
    pub fn replace_from(&mut self, given: &PyValue) -> Result<(), String> {
        let PyValue::Dict(members) = given else {
            return Err("globals must be an object".to_string());
        };
        for (key, _) in members.iter() {
            if !key.equals("REPO") && !key.equals("argv") {
                return Err(format!("globals holds REPO and argv only: {}", key.repr()));
            }
        }
        if let Some(repo) = members.get("REPO") {
            self.repo = repo.clone();
        }
        if let Some(argv) = members.get("argv") {
            let PyValue::List(items) = argv else {
                return Err("argv must be an array of strings".to_string());
            };
            let mut strings = Vec::new();
            for item in items {
                let PyValue::Str(text) = item else {
                    return Err("argv must be an array of strings".to_string());
                };
                strings.push(text.clone());
            }
            self.argv = strings;
        }
        Ok(())
    }
}

/// The constants, keyed by owner.
pub fn constants(repo: &PyStr) -> PyValue {
    let mut module = PyDict::new();
    module.set("REPO", PyValue::Str(repo.clone()));
    let mut all = PyDict::new();
    all.set(MODULE, PyValue::Dict(module));
    PyValue::Dict(all)
}

/// Resolves `cleanup_docker.function`.
pub fn resolve(operation: &str) -> Result<Function, Raised> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Raised::plain(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    Function::named(name).ok_or_else(|| {
        Raised::plain(
            "AttributeError",
            &format!("module '{module}' has no attribute '{name}'"),
        )
    })
}

fn event(name: &str, fields: Vec<(&str, PyValue)>) -> PyValue {
    let mut row = PyDict::new();
    row.set("event", PyValue::str(name));
    for (key, value) in fields {
        row.set(key, value);
    }
    PyValue::Dict(row)
}

/// What an API call printed (and, in a check, the ordered record of its file and process steps).
#[derive(Default)]
pub struct Capture {
    pub stdout: PyStr,
    pub stderr: PyStr,
    pub tracing: bool,
    pub trace: Vec<PyValue>,
    pending: PyStr,
}

impl Capture {
    pub fn new(tracing: bool) -> Capture {
        Capture {
            tracing,
            ..Capture::default()
        }
    }

    fn write_stderr(&mut self, text: &PyStr) {
        self.stderr.push_py(text);
        if !self.tracing {
            return;
        }
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
}

impl CleanupConsole for Capture {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.stdout.push_py(text);
        Ok(())
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        self.write_stderr(text);
        Ok(())
    }
}

impl Console for Capture {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        self.write_stderr(text);
        Ok(())
    }

    fn write_stdout(&mut self, text: &PyStr) {
        self.stdout.push_py(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
            self.write_stderr(text);
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        self.stdout.push_py(help);
    }

    fn write_uncaught(&mut self, _text: &PyStr) {}

    fn observe(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        if self.tracing {
            self.trace.push(event(name, fields));
        }
    }

    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        Ok(PyStr::default())
    }
}

/// Binds the arguments as Python does and runs the function; the value is the function's typed DTO.
pub fn call(
    function: Function,
    world: &mut World<'_>,
    globals: &Globals,
    columns: Option<&str>,
    args: &[PyValue],
    kwargs: &PyDict,
) -> Done<PyValue> {
    let signature = function.signature();
    let bound = signature
        .bind(args, kwargs)
        .map_err(|failure| Thrown::plain(Raised::from(failure)))?;
    match function {
        Function::Run => {
            let (code, out) = run_text(world, &bound[0], &bound[1]);
            Ok(dto::run_tuple(code, &out))
        }
        Function::Preserve => {
            preserve_set(world, &globals.repo, &bound[0]).map(|keep| dto::preserve_tuple(&keep))
        }
        Function::ListImages => list_images(world).map(dto::json),
        Function::Main => {
            main(world, &globals.repo, &globals.argv, &bound[0], columns).map(|()| dto::none())
        }
    }
}

/// The answer of a call: the outcome with the streams and the arguments after it.
pub fn record(
    id: PyValue,
    outcome: Done<PyValue>,
    stdout: &PyStr,
    stderr: &PyStr,
    args: Vec<PyValue>,
    kwargs: PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", value);
        }
        Err(thrown) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(&thrown.raised.class));
            row.set("message", PyValue::Str(thrown.raised.message.clone()));
            row.set("exception_chain", thrown.chain());
        }
    }
    row.set("stdout", PyValue::Str(stdout.clone()));
    row.set("stderr", PyValue::Str(stderr.clone()));
    row.set("args_after", encode(&PyValue::List(args)));
    row.set("kwargs_after", encode(&PyValue::Dict(kwargs)));
    PyValue::Dict(row)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Thrown::plain(Raised::from(Failure::new(
            "RequestError",
            message,
        )))),
        &PyStr::default(),
        &PyStr::default(),
        Vec::new(),
        PyDict::new(),
    )
}

fn operation_of(request: &Request) -> String {
    match request.members.get("operation") {
        Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
        _ => String::new(),
    }
}

/// What a service run is configured with: the default `REPO` and the terminal width.
pub struct Defaults {
    pub repo: PyStr,
    pub columns: Option<String>,
}

/// Answers one request line on `console` (the files are read, the processes run and every real write goes through `console`). The
/// streams of the record are empty: what the call wrote is whatever `console` did with it.
pub fn answer_on(line: &PyStr, defaults: &Defaults, console: &mut dyn CleanupConsole) -> PyValue {
    let request = match parse_request(line, &["globals"]) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let mut globals = Globals::defaults(&defaults.repo);
    if let Some(given) = request.members.get("globals") {
        if let Err(problem) = globals.replace_from(given) {
            return request_record(request.id.clone(), &problem);
        }
    }
    let outcome = match resolve(&operation_of(&request)) {
        Err(raised) => Err(Thrown::plain(raised)),
        Ok(function) => call(
            function,
            &mut World {
                console,
                tracing: false,
            },
            &globals,
            defaults.columns.as_deref(),
            &request.args,
            &request.kwargs,
        ),
    };
    record(
        request.id,
        outcome,
        &PyStr::default(),
        &PyStr::default(),
        request.args,
        request.kwargs,
    )
}

/// Answers one request line (the files are read and the processes run); what the call printed is in the record.
pub fn answer(line: &PyStr, defaults: &Defaults) -> PyValue {
    let mut capture = Capture::new(false);
    let mut row = answer_on(line, defaults, &mut capture);
    if let PyValue::Dict(map) = &mut row {
        if map.contains_key("stdout") {
            map.set("stdout", PyValue::Str(capture.stdout.clone()));
            map.set("stderr", PyValue::Str(capture.stderr.clone()));
        }
    }
    row
}

/// One output line of a record.
pub fn render(row: &PyValue) -> Result<String, Failure> {
    crate::api::render(row)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8], defaults: &Defaults) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(render(&answer(&line, defaults))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
