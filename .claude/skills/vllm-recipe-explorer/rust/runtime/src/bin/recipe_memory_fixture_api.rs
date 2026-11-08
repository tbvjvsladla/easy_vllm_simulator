//! `recipe_memory_fixture_api case|session|runner-probe ...`: the RAM-gate API over a controlled machine, for the checks only (built
//! with the `fixture` feature, never a production program).
//!
//! * `case --spec FILE --root DIR --control FILE` runs one function case and writes its control record (trace, counters, streams,
//!   arguments after) to the new file `--control`; the exit status is 0, or 77 after a boundary violation.
//! * `session --spec FILE --root DIR --control FILE [--input-fd FD]` serves one gate session on standard output (see
//!   `recipe_runtime::memory_session`) with the machine the spec's `context` describes, then writes the provider trace and counters to
//!   `--control`. The requests and acknowledgements come from standard input, or, with `--input-fd`, from that passed descriptor
//!   exactly as the production service reads them (standard input is then left alone).
//! * `runner-probe --input-fd FD -- PROGRAM [ARGS...]` runs a harmless command through the helper runner the production machine uses
//!   (standard input inherited, output discarded, 20 s), then prints one JSON line `{"status": N | null, "protocol": TEXT}` where
//!   `TEXT` is what is still in the protocol descriptor: it proves the command inherited standard input, not the protocol channel.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::memory::HelperRun;
use recipe_runtime::memory_fixture::{run_api_case, Case, BOUNDARY_STATUS};
use recipe_runtime::memory_session::serve;
use recipe_runtime::memory_system::{
    adopt_input_fd, input_fd_option, run_with_timeout, HelperInput,
};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use std::time::Duration;

fn fail(message: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "recipe_memory_fixture_api: {message}");
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

/// The `--input-fd` value among the arguments, if any (`Err` is the usage problem).
fn descriptor_of(arguments: &[std::ffi::OsString]) -> Result<Option<i32>, String> {
    match arguments
        .iter()
        .position(|argument| argument == "--input-fd")
    {
        None => Ok(None),
        Some(at) => {
            let given: Vec<String> = arguments[at..]
                .iter()
                .take(2)
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect();
            if arguments[at + 1..]
                .iter()
                .any(|argument| argument == "--input-fd")
            {
                return Err("--input-fd given twice".to_string());
            }
            input_fd_option(&given)
        }
    }
}

fn runner_probe(arguments: &[std::ffi::OsString]) -> ExitCode {
    let descriptor = match descriptor_of(arguments) {
        Ok(Some(descriptor)) => descriptor,
        Ok(None) => return fail("runner-probe requires --input-fd FD"),
        Err(problem) => return fail(&problem),
    };
    let Some(at) = arguments.iter().position(|argument| argument == "--") else {
        return fail("runner-probe takes -- PROGRAM [ARGS...]");
    };
    let command: Vec<String> = arguments[at + 1..]
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    // SAFETY: `descriptor` came from `descriptor_of` (validated by `input_fd_option`, above 2), is the caller's protocol channel for
    // this probe, and is adopted exactly once in this process (`runner-probe` runs no other adoption and returns before `main`
    // reaches the session branch); the helper runner starts processes only afterwards, and the descriptor is close-on-exec.
    let mut protocol = match unsafe { adopt_input_fd(descriptor) } {
        Ok(file) => file,
        Err(problem) => {
            let _ = writeln!(std::io::stderr(), "recipe_memory_fixture_api: {problem}");
            return ExitCode::from(1);
        }
    };
    let words: Vec<&str> = command.iter().map(String::as_str).collect();
    let status = match run_with_timeout(&words, Duration::from_secs(20), HelperInput::Inherited) {
        HelperRun::Exited(code) => PyValue::int(code),
        HelperRun::Failed => PyValue::None,
    };
    let mut rest = Vec::new();
    if let Err(error) = protocol.read_to_end(&mut rest) {
        let _ = writeln!(std::io::stderr(), "recipe_memory_fixture_api: {error}");
        return ExitCode::from(1);
    }
    let mut row = PyDict::new();
    row.set("status", status);
    row.set("protocol", PyValue::Str(PyStr::from_surrogateescape(&rest)));
    match PyValue::Dict(row).dumps_ascii() {
        Ok(text) => {
            let _ = writeln!(std::io::stdout(), "{text}");
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error.to_string()),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.first().is_some_and(|mode| mode == "runner-probe") {
        return runner_probe(&arguments);
    }
    let (Some(mode), Some(spec), Some(root), Some(control)) = (
        arguments.first().cloned(),
        option(&arguments, "--spec"),
        option(&arguments, "--root"),
        option(&arguments, "--control"),
    ) else {
        return fail("usage: case|session --spec FILE --root DIR --control FILE [--input-fd FD] | runner-probe --input-fd FD -- PROGRAM");
    };
    let Ok(text) = std::fs::read(&spec) else {
        return fail("cannot read the spec");
    };
    let root = PyStr::from_surrogateescape(root.as_bytes());
    let case = match Case::parse(&PyStr::from_surrogateescape(&text)) {
        Ok(case) => case,
        Err(failure) => return fail(&failure.line().to_utf8().unwrap_or_default()),
    };
    match mode.as_os_str().as_bytes() {
        b"case" => match run_api_case(&case, &root) {
            Ok((record, boundary)) => match write_control(&control, &record) {
                Ok(()) if boundary => ExitCode::from(BOUNDARY_STATUS as u8),
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => fail(&format!("cannot write the control record: {error}")),
            },
            Err(failure) => fail(&failure.line().to_utf8().unwrap_or_default()),
        },
        b"session" => {
            let descriptor = match descriptor_of(&arguments) {
                Ok(descriptor) => descriptor,
                Err(problem) => return fail(&problem),
            };
            let mut machine = match case.machine(&root, false) {
                Ok(machine) => machine,
                Err(failure) => return fail(&failure.line().to_utf8().unwrap_or_default()),
            };
            let served = match descriptor {
                // SAFETY: `number` came from `descriptor_of` (validated by `input_fd_option`, above 2), it is the caller's protocol
                // channel for this session, this is the only adoption in the process (`runner-probe` returned earlier in `main`
                // and never reaches this branch), and nothing else wraps or closes that number.
                Some(number) => match unsafe { adopt_input_fd(number) } {
                    Ok(file) => serve(
                        &mut machine,
                        &mut std::io::BufReader::new(file),
                        &mut std::io::stdout().lock(),
                    ),
                    Err(problem) => {
                        let _ = writeln!(std::io::stderr(), "recipe_memory_fixture_api: {problem}");
                        return ExitCode::from(1);
                    }
                },
                None => serve(
                    &mut machine,
                    &mut std::io::stdin().lock(),
                    &mut std::io::stdout().lock(),
                ),
            };
            let (trace, state) = machine.observations();
            let mut record = PyDict::new();
            record.set("trace", trace);
            record.set("context_after", state);
            if let Err(error) = write_control(&control, &PyValue::Dict(record)) {
                return fail(&format!("cannot write the control record: {error}"));
            }
            match served {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => fail(&format!("session failed: {error}")),
            }
        }
        _ => fail("the mode is case, session or runner-probe"),
    }
}
