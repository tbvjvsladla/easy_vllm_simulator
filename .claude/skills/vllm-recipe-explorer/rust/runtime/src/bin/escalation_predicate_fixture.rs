//! `escalation_predicate_fixture --spec FILE --root DIR --control FILE`: the `escalation_predicate` command line over an owned root,
//! for the checks only (built with the `fixture` feature, never a production program). The arguments are the spec's `args`; standard
//! output, standard error and the exit status are the program's own; the control record goes to the new file `--control`.

use agent_runtime_support::pystr::PyStr;
use recipe_runtime::escalation_fixture::{run_cli_case, Case};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

fn fail(message: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "escalation_predicate_fixture: {message}");
    ExitCode::from(2)
}

fn option(arguments: &[std::ffi::OsString], name: &str) -> Option<std::ffi::OsString> {
    arguments
        .iter()
        .position(|argument| argument == name)
        .and_then(|at| arguments.get(at + 1).cloned())
}

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let (Some(spec), Some(root), Some(control)) = (
        option(&arguments, "--spec"),
        option(&arguments, "--root"),
        option(&arguments, "--control"),
    ) else {
        return fail("usage: --spec FILE --root DIR --control FILE");
    };
    let Ok(text) = std::fs::read(&spec) else {
        return fail("cannot read the spec");
    };
    let root = PyStr::from_surrogateescape(root.as_bytes());
    let case = match Case::parse(&PyStr::from_surrogateescape(&text)) {
        Ok(case) => case,
        Err(failure) => return fail(&failure.line().to_utf8().unwrap_or_default()),
    };
    let args = match case.cli_arguments(&root) {
        Ok(args) => args,
        Err(failure) => return fail(&failure.line().to_utf8().unwrap_or_default()),
    };
    let columns =
        recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref());
    let (status, record) = run_cli_case(&case, &root, &args, Some(&columns));
    let written = record
        .dumps_ascii()
        .map_err(|error| error.to_string())
        .and_then(|text| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&control)
                .map_err(|error| error.to_string())?;
            writeln!(file, "{text}").map_err(|error| error.to_string())
        });
    match written {
        Ok(()) => ExitCode::from(u8::try_from(status).unwrap_or(1)),
        Err(error) => fail(&format!("cannot write the control record: {error}")),
    }
}
