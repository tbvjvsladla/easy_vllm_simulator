//! `recipe_cleanup_fixture_api case --spec FILE --root CASE_ROOT --control FILE --base OWNED_BASE`: one `cleanup_docker` operation over an
//! owned root and an owned peer, for the checks only (built with the `fixture` feature, never a production program). Writes the control
//! record (the HTTP trace, the module constants before and after, the streams, the arguments after) in the original collector's format,
//! without the Python import routing, to the new file `--control`.

use agent_runtime_support::pystr::PyStr;
use recipe_runtime::cleanup_fixture::{run_api_case, Case};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

fn fail(message: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "recipe_cleanup_fixture_api: {message}");
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
    let (Some(mode), Some(spec), Some(root), Some(control)) = (
        arguments.first().cloned(),
        option(&arguments, "--spec"),
        option(&arguments, "--root"),
        option(&arguments, "--control"),
    ) else {
        return fail("usage: case --spec FILE --root ROOT --control FILE");
    };
    if mode != "case" {
        return fail("the mode is case");
    }
    let Ok(text) = std::fs::read(&spec) else {
        return fail("cannot read the spec");
    };
    let root = PyStr::from_surrogateescape(root.as_bytes());
    let case = match Case::parse(&PyStr::from_surrogateescape(&text)) {
        Ok(case) => case,
        Err(failure) => return fail(&failure.line().to_utf8().unwrap_or_default()),
    };
    let columns =
        recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref());
    let record = run_api_case(&case, &root, Some(&columns));
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
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail(&format!("cannot write the control record: {error}")),
    }
}
