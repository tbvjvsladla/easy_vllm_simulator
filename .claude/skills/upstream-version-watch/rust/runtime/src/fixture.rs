//! The controlled run of `resolve_build_track` for the checks: the same library, with the original collector's control record. Only with the
//! `fixture` feature; no production program has it.
//!
//! A case is `{id, owner: "resolve_build_track", kind, function, args, kwargs, profile, argv?, files?, label?}`; `@CASE@` in any text of
//! `args`/`kwargs`/`argv` becomes the case root. `files` describes what the CALLER prepared and is never read. The manifest paths are relative to
//! the working directory the caller chose (the root). `profile` (`pyyaml-6.0.3` or `flat`) chooses the dependency the original's `import yaml`
//! meets; the record carries it in `globals_*` as the collector's does.

use crate::api::{call, resolve_operation, Capture, Function};
use crate::business::{manifest_sm_keys, source_build_min};
use crate::cli::{conclude, exit_status, run, End};
use crate::dto::{dict_of, encode};
use crate::host::{Host, Sink, YamlProfile};
use crate::live::LiveConsole;
use crate::manifest::World;
use crate::thrown::{Done, Thrown};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;

pub const OWNER: &str = "resolve_build_track";

pub(crate) fn rebase(value: &PyValue, root: &PyStr) -> PyValue {
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

pub(crate) fn spec_error(message: &str) -> Failure {
    Failure::new("SpecError", message)
}

/// A case read from its JSON text.
pub struct Case {
    pub id: PyValue,
    function: Option<PyStr>,
    args: Vec<PyValue>,
    kwargs: PyDict,
    argv: Vec<PyValue>,
    profile: YamlProfile,
    cli: bool,
}

impl Case {
    pub fn parse(text: &PyStr) -> Result<Case, Failure> {
        let PyValue::Dict(members) = loads_text_objects(text)
            .map_err(|error| spec_error(&format!("the spec is not JSON: {error}")))?
        else {
            return Err(spec_error("the spec is not an object"));
        };
        if !matches!(members.get("owner"), Some(PyValue::Str(owner)) if owner.equals(OWNER)) {
            return Err(spec_error("the owner is resolve_build_track"));
        }
        let profile = match members.get("profile") {
            Some(PyValue::Str(name)) => name
                .to_utf8()
                .and_then(|name| YamlProfile::from_declared(&name))
                .ok_or_else(|| spec_error("the profile is pyyaml-6.0.3 or flat"))?,
            _ => return Err(spec_error("the spec declares its profile")),
        };
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
        let mut argv = list("argv");
        if !members.contains_key("argv") {
            argv = vec![PyValue::str(crate::main_cmd::PROGRAM)];
        }
        Ok(Case {
            id: members.get("id").cloned().unwrap_or(PyValue::None),
            function,
            args: list("args"),
            kwargs,
            argv,
            profile,
            cli: matches!(members.get("kind"), Some(PyValue::Str(kind)) if kind.equals("cli")),
        })
    }

    pub fn is_cli(&self) -> bool {
        self.cli
    }

    fn argv(&self, root: &PyStr) -> Result<Vec<PyStr>, Failure> {
        self.argv
            .iter()
            .map(|item| match rebase(item, root) {
                PyValue::Str(text) => Ok(text),
                _ => Err(spec_error("argv holds text only")),
            })
            .collect()
    }

    fn function(&self) -> Result<Function, Raised> {
        match self.function.as_ref().and_then(|name| name.to_utf8()) {
            Some(name) => resolve_operation(&format!("{}.{name}", crate::api::MODULE)),
            None => Err(Raised::plain(
                "AttributeError",
                &format!(
                    "module '{}' has no attribute {}",
                    crate::api::MODULE,
                    self.function
                        .as_ref()
                        .map_or_else(|| "None".to_string(), PyStr::repr)
                ),
            )),
        }
    }
}

pub(crate) fn event(name: &str, fields: Vec<(&str, PyValue)>) -> PyValue {
    let mut row = PyDict::new();
    row.set("event", PyValue::str(name));
    for (key, value) in fields {
        row.set(key, value);
    }
    PyValue::Dict(row)
}

fn globals_of(profile: YamlProfile) -> PyValue {
    dict_of(vec![
        (
            "SOURCE_BUILD_MIN",
            encode(&source_build_min()).unwrap_or(PyValue::None),
        ),
        (
            "_MANIFEST_SM_KEYS",
            encode(&manifest_sm_keys()).unwrap_or(PyValue::None),
        ),
        ("yaml_profile", PyValue::str(profile.declared())),
    ])
}

pub(crate) fn argv_value(argv: &[PyStr]) -> PyValue {
    PyValue::List(argv.iter().cloned().map(PyValue::Str).collect())
}

pub(crate) fn put_outcome(row: &mut PyDict, cli: bool, outcome: &Done<PyValue>) {
    // a value outside the DTO profile is the program's refusal: a `TypeError` record, never a silent value
    let shown = match outcome {
        Ok(value) => encode(value).map_err(|refusal| Thrown::builtin("TypeError", &refusal)),
        Err(thrown) => Err(thrown.clone()),
    };
    match shown {
        Ok(value) => {
            row.set("kind", PyValue::str(if cli { "return" } else { "value" }));
            row.set("value", value);
        }
        Err(thrown) => {
            let exit = thrown.raised.class == "SystemExit";
            row.set(
                "kind",
                PyValue::str(if exit && cli { "exit" } else { "exception" }),
            );
            row.set("type", PyValue::str(&thrown.raised.class));
            row.set("module", PyValue::str(&thrown.raised.module));
            row.set("message", PyValue::Str(thrown.raised.message.clone()));
            row.set("exception_chain", thrown.chain());
            if let Some(code) = &thrown.raised.code {
                row.set("code", (**code).clone());
            }
        }
    }
}

/// Runs one API case: the control record in the original collector's format.
pub fn run_api_case(case: &Case, root: &PyStr, columns: Option<&str>) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(OWNER));
    let argv = match case.argv(root) {
        Ok(argv) => argv,
        Err(failure) => {
            let thrown = Thrown::plain(Raised::from(failure));
            put_outcome(&mut row, false, &Err(thrown));
            return PyValue::Dict(row);
        }
    };
    let PyValue::List(args) = rebase(&PyValue::List(case.args.clone()), root) else {
        unreachable!("a list stays a list")
    };
    let PyValue::Dict(kwargs) = rebase(&PyValue::Dict(case.kwargs.clone()), root) else {
        unreachable!("a dict stays a dict")
    };
    let mut capture = Capture::new(case.profile, true);
    let outcome: Done<PyValue> = match case.function() {
        Err(raised) => Err(Thrown::plain(raised)),
        Ok(function) => call(
            function,
            &mut World {
                host: &mut capture,
                argv: &argv,
                columns,
            },
            &args,
            &kwargs,
        ),
    };
    put_outcome(&mut row, false, &outcome);
    row.set("trace", PyValue::List(std::mem::take(&mut capture.trace)));
    row.set("globals_before", globals_of(case.profile));
    row.set("globals_after", globals_of(case.profile));
    row.set(
        "args_after",
        encode(&PyValue::List(args)).unwrap_or(PyValue::None),
    );
    row.set(
        "kwargs_after",
        encode(&PyValue::Dict(kwargs)).unwrap_or(PyValue::None),
    );
    row.set("argv_after", argv_value(&argv));
    row.set("stdout", PyValue::Str(capture.stdout.clone()));
    row.set("stderr", PyValue::Str(capture.stderr.clone()));
    PyValue::Dict(row)
}

/// The recording console of a command-line run: the production console's live standard output, the trace of the file steps.
pub(crate) struct Controlled {
    pub(crate) live: LiveConsole,
    pub(crate) trace: Vec<PyValue>,
}

impl Sink for Controlled {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.live.out(text)
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        self.live.err(text)
    }

    fn parser_help(&mut self, help: &PyStr) {
        self.live.parser_help(help);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        self.live.parser_error(usage, message);
    }

    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        self.live.take_parser_failure()
    }

    fn tracing(&self) -> bool {
        true
    }

    fn trace(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.trace.push(event(name, fields));
    }
}

impl Host for Controlled {
    fn import_yaml(&mut self) -> Done<()> {
        self.live.import_yaml()
    }
}

/// Runs one command-line case (the standard streams are the real ones): the exit status and the control record.
pub fn run_cli_case(case: &Case, root: &PyStr, columns: Option<&str>) -> (i32, PyValue) {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(OWNER));
    let argv = match case.argv(root) {
        Ok(argv) => argv,
        Err(failure) => {
            put_outcome(&mut row, true, &Err(Thrown::plain(Raised::from(failure))));
            row.set("status", PyValue::int(1));
            return (1, PyValue::Dict(row));
        }
    };
    let mut console = Controlled {
        live: LiveConsole::new(case.profile),
        trace: Vec::new(),
    };
    let end = run(&mut console, &argv, columns);
    let status = match &end {
        End::Returned => {
            put_outcome(&mut row, true, &Ok(PyValue::None));
            0
        }
        End::Exited(thrown) => {
            put_outcome(&mut row, true, &Err(thrown.clone()));
            exit_status(thrown)
        }
        End::Raised(thrown) => {
            put_outcome(&mut row, true, &Err(thrown.clone()));
            1
        }
    };
    let trace = std::mem::take(&mut console.trace);
    row.set("trace", PyValue::List(trace));
    row.set("globals_before", globals_of(case.profile));
    row.set("globals_after", globals_of(case.profile));
    row.set(
        "args_after",
        encode(&PyValue::List(Vec::new())).unwrap_or(PyValue::None),
    );
    row.set(
        "kwargs_after",
        encode(&PyValue::Dict(PyDict::new())).unwrap_or(PyValue::None),
    );
    row.set("argv_after", argv_value(&argv));
    row.set("status", PyValue::int(status));
    // The process ends as the interpreter ends it: an uncaught exception is reported, the streams are closed (the flush of standard output
    // can still fail: status 120 while the record above says what `main` itself did).
    let status = conclude(&mut console.live, end);
    (status, PyValue::Dict(row))
}
