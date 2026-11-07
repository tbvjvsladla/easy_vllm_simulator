//! The command line shared by the normal JSON-lines programs of the fetch components: `<program> evaluate-jsonl [--input-fd FD]`.
//!
//! `--input-fd FD` only chooses the channel the requests are read from: a readable descriptor the caller passed to this process, above 2;
//! standard input is then left alone. Nothing else selects a file, a program or a provider.

use agent_runtime_support::failure::Failure;
use recipe_runtime::memory_system::{adopt_input_fd, input_fd_option};
use recipe_runtime::stdio::os_failure;
use std::io::{Read, Write};
use std::process::ExitCode;

/// What a program turns the request bytes into: all its response lines, or the failure to render them.
pub type Evaluate = fn(&[u8]) -> Result<Vec<u8>, Failure>;

fn deliver(program: &str, bytes: &[u8]) -> ExitCode {
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let line = os_failure(&error).line().to_utf8().unwrap_or_default();
            let _ = writeln!(
                std::io::stderr(),
                "{program}: cannot write standard output: {line}"
            );
            ExitCode::from(1)
        }
    }
}

fn usage(program: &str, problem: &str) -> ExitCode {
    let _ = writeln!(
        std::io::stderr(),
        "{program}: {problem}\nusage: {program} evaluate-jsonl [--input-fd FD]"
    );
    ExitCode::from(2)
}

fn failed(program: &str, problem: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "{program}: {problem}");
    ExitCode::from(1)
}

/// Runs `program`'s command line over this process's arguments, standard streams and descriptors.
pub fn run(program: &str, evaluate: Evaluate) -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        return usage(program, "a command is required");
    };
    if command != "evaluate-jsonl" {
        return usage(program, "unknown command or arguments");
    }
    let descriptor = match input_fd_option(rest) {
        Ok(descriptor) => descriptor,
        Err(problem) => return usage(program, &problem),
    };
    let mut bytes = Vec::new();
    let read = match descriptor {
        // SAFETY: `number` was validated by `input_fd_option` (decimal, above 2, so neither standard stream); it is the one
        // descriptor the caller passed as this command's protocol channel (the documented `pass_fds` contract), this is the
        // only adoption in the process, and nothing else wraps it.
        Some(number) => match unsafe { adopt_input_fd(number) } {
            Ok(mut file) => file.read_to_end(&mut bytes),
            Err(problem) => return failed(program, &problem),
        },
        None => std::io::stdin().lock().read_to_end(&mut bytes),
    };
    if let Err(error) = read {
        let line = os_failure(&error).line().to_utf8().unwrap_or_default();
        return failed(program, &format!("cannot read the requests: {line}"));
    }
    match evaluate(&bytes) {
        Ok(out) => deliver(program, &out),
        Err(failure) => failed(program, &failure.line().to_utf8().unwrap_or_default()),
    }
}
