//! `preload_ram_gate_fixture --spec FILE --root DIR --control FILE`: the `preload_ram_gate` command line over a controlled machine,
//! for the checks only (built with the `fixture` feature, never a production program). The arguments are the spec's `args`; standard
//! output, standard error and the exit status are the program's own, and the control record (trace, counters, exit) goes to the new
//! file `--control`. A boundary violation ends the run with status 77.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use recipe_runtime::memory_fixture::{run_cli_case, Case};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

fn fail(message: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "preload_ram_gate_fixture: {message}");
    ExitCode::from(2)
}

fn option(arguments: &[std::ffi::OsString], name: &str) -> Option<std::ffi::OsString> {
    arguments
        .iter()
        .position(|argument| argument == name)
        .and_then(|at| arguments.get(at + 1).cloned())
}

fn write_control(path: &std::ffi::OsStr, record: &PyValue) -> Result<(), String> {
    let text = record.dumps_ascii().map_err(|error| error.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    writeln!(file, "{text}").map_err(|error| error.to_string())
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
    let result = Case::parse(&PyStr::from_surrogateescape(&text)).and_then(|case| {
        let args = case.cli_arguments(&root)?;
        let mut skill_root = root.clone();
        skill_root.push_str("/node/.claude/skills/vllm-recipe-explorer");
        let columns =
            recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref());
        run_cli_case(&case, &root, &args, &skill_root, Some(&columns))
    });
    match result {
        Ok((status, record)) => match write_control(&control, &record) {
            Ok(()) => ExitCode::from(u8::try_from(status).unwrap_or(1)),
            Err(error) => fail(&format!("cannot write the control record: {error}")),
        },
        Err(failure) => fail(&failure.line().to_utf8().unwrap_or_default()),
    }
}
