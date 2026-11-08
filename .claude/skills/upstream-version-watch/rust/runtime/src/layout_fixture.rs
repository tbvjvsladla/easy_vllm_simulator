//! The controlled run of `check_spec_layout` for the checks: the same library, with the original collector's control record. Only with the `fixture`
//! feature; no production program has it.
//!
//! A case is `{id, owner: "check_spec_layout", kind, function, args_dto, kwargs_dto, argv?, files?, label?}`; `@CASE@` in any text of the DTOs or `argv`
//! becomes the case root. `files` describes what the CALLER prepared and is never read. The program reads real files under the root.

use crate::api::Capture;
use crate::cli::{exit_status, End};
use crate::dto::encode;
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::fixture::{argv_value, put_outcome, rebase, spec_error, Controlled};
use crate::host::YamlProfile;
use crate::layout_api::{call, resolve_operation, source_constants, Function, MODULE};
use crate::layout_cli::run;
use crate::layout_files::LayoutWorld;
use crate::layout_main::PROGRAM;
use crate::live::LiveConsole;
use crate::thrown::{Done, Thrown};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;

pub const OWNER: &str = "check_spec_layout";

/// A case read from its JSON text.
pub struct Case {
    pub id: PyValue,
    function: Option<PyStr>,
    args: PyValue,
    kwargs: PyValue,
    argv: Vec<PyValue>,
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
            return Err(spec_error("the owner is check_spec_layout"));
        }
        let function = match members.get("function") {
            Some(PyValue::Str(name)) => Some(name.clone()),
            _ => None,
        };
        let argv = match members.get("argv") {
            Some(PyValue::List(items)) => items.clone(),
            Some(_) => return Err(spec_error("argv is an array")),
            None => vec![PyValue::str(PROGRAM)],
        };
        Ok(Case {
            id: members.get("id").cloned().unwrap_or(PyValue::None),
            function,
            args: members.get("args_dto").cloned().unwrap_or_else(|| {
                PyValue::List(vec![PyValue::str("list"), PyValue::List(Vec::new())])
            }),
            kwargs: members.get("kwargs_dto").cloned().unwrap_or_else(|| {
                PyValue::List(vec![PyValue::str("dict"), PyValue::List(Vec::new())])
            }),
            argv,
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
            Some(name) => resolve_operation(&format!("{MODULE}.{name}")),
            None => Err(Raised::plain(
                "AttributeError",
                &format!(
                    "module '{MODULE}' has no attribute {}",
                    self.function
                        .as_ref()
                        .map_or_else(|| "None".to_string(), PyStr::repr)
                ),
            )),
        }
    }

    fn arguments(&self, root: &PyStr) -> Result<(Vec<PyValue>, PyDict), String> {
        Ok((
            decode_args(&rebase(&self.args, root))?,
            decode_kwargs(&rebase(&self.kwargs, root))?,
        ))
    }
}

fn globals_of() -> PyValue {
    source_constants().unwrap_or(PyValue::None)
}

/// Runs one API case: the control record in the original collector's format.
pub fn run_api_case(case: &Case, root: &PyStr, columns: Option<&str>) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", case.id.clone());
    row.set("owner", PyValue::str(OWNER));
    let prepared = case
        .argv(root)
        .map_err(|failure| failure.line().to_utf8().unwrap_or_default())
        .and_then(|argv| {
            case.arguments(root)
                .map(|(args, kwargs)| (argv, args, kwargs))
        });
    let (argv, args, kwargs) = match prepared {
        Ok(prepared) => prepared,
        Err(problem) => {
            put_outcome(
                &mut row,
                false,
                &Err(Thrown::builtin("SpecError", &problem)),
            );
            return PyValue::Dict(row);
        }
    };
    let mut capture = Capture::new(YamlProfile::Available, true);
    let outcome: Done<PyValue> = match case.function() {
        Err(raised) => Err(Thrown::plain(raised)),
        Ok(function) => call(
            function,
            &mut LayoutWorld::new(&mut capture, &argv, columns),
            &args,
            &kwargs,
        ),
    };
    put_outcome(&mut row, false, &outcome);
    row.set("trace", PyValue::List(std::mem::take(&mut capture.trace)));
    row.set("globals_before", globals_of());
    row.set("globals_after", globals_of());
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
        live: LiveConsole::for_streams(),
        trace: Vec::new(),
    };
    let ran = run(&mut console, &argv, columns);
    let status = match &ran.end {
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
    row.set("trace", PyValue::List(std::mem::take(&mut console.trace)));
    row.set("globals_before", globals_of());
    row.set("globals_after", globals_of());
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
    let status = ran.conclude(&mut console.live);
    (status, PyValue::Dict(row))
}
