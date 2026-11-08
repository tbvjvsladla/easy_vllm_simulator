//! `recipe_memory_api constants | evaluate-jsonl [--input-fd FD] | gate-session --input-fd FD` (see `recipe_runtime::memory_api` and
//! `memory_session`). The service runs against the real machine only: it has no option, variable or request member that substitutes it.
//!
//! `--input-fd FD` only chooses the channel the requests (and, in a session, the acknowledgements) are read from: a readable
//! descriptor the caller passed to this process (a pipe, `pass_fds`), above 2. With it standard input is left alone, so the cache
//! helper inherits the caller's own standard input as the original's does; the descriptor is close-on-exec, so the helper cannot
//! read the protocol. `evaluate-jsonl` without it reads standard input to its end (the helper then inherits that spent stream);
//! `gate-session` requires it, since a session's standard input must stay out of the protocol.

use recipe_runtime::memory_api::{constants, evaluate_jsonl, render};
use recipe_runtime::memory_session::serve;
use recipe_runtime::memory_system::{adopt_input_fd, input_fd_option, System};
use recipe_runtime::stdio::os_failure;
use std::io::{Read, Write};
use std::process::ExitCode;

const USAGE: &str =
    "usage: recipe_memory_api constants | evaluate-jsonl [--input-fd FD] | gate-session --input-fd FD";

/// Writes the whole answer to standard output; a failing descriptor is a deliberate diagnostic and a non-zero status.
fn deliver(bytes: &[u8]) -> ExitCode {
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let line = os_failure(&error).line().to_utf8().unwrap_or_default();
            let _ = writeln!(
                std::io::stderr(),
                "recipe_memory_api: cannot write standard output: {line}"
            );
            ExitCode::from(1)
        }
    }
}

fn usage(problem: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "recipe_memory_api: {problem}\n{USAGE}");
    ExitCode::from(2)
}

fn failed(problem: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "recipe_memory_api: {problem}");
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        return usage("a command is required");
    };
    match command.as_str() {
        "constants" if rest.is_empty() => match render(&constants()) {
            Ok(text) => deliver(format!("{text}\n").as_bytes()),
            Err(failure) => failed(&failure.line().to_utf8().unwrap_or_default()),
        },
        "evaluate-jsonl" => {
            let descriptor = match input_fd_option(rest) {
                Ok(descriptor) => descriptor,
                Err(problem) => return usage(&problem),
            };
            let mut bytes = Vec::new();
            let read = match descriptor {
                // SAFETY: `number` was validated by `input_fd_option` (decimal, above 2, so neither standard stream), it is the one
                // descriptor the caller passed as this command's protocol channel (the documented `pass_fds` contract), this
                // is the only adoption in the process (the `gate-session` branch below is the other arm of the same `match`), and
                // nothing else opens or wraps that number before or after.
                Some(number) => match unsafe { adopt_input_fd(number) } {
                    Ok(mut file) => file.read_to_end(&mut bytes),
                    Err(problem) => return failed(&problem),
                },
                None => std::io::stdin().lock().read_to_end(&mut bytes),
            };
            if let Err(error) = read {
                // The bytes read before the failure are not used: a request stream that broke is not a shorter stream.
                let line = os_failure(&error).line().to_utf8().unwrap_or_default();
                return failed(&format!("cannot read the requests: {line}"));
            }
            match evaluate_jsonl(&bytes, &mut System::new()) {
                Ok(out) => deliver(&out),
                Err(failure) => failed(&failure.line().to_utf8().unwrap_or_default()),
            }
        }
        "gate-session" => {
            let descriptor = match input_fd_option(rest) {
                Ok(Some(descriptor)) => descriptor,
                Ok(None) => return usage("gate-session requires --input-fd FD"),
                Err(problem) => return usage(&problem),
            };
            // SAFETY: as above: validated by `input_fd_option` (above 2), the caller's own protocol channel, the only adoption in this
            // process, and nothing else wraps or closes it.
            let file = match unsafe { adopt_input_fd(descriptor) } {
                Ok(file) => file,
                Err(problem) => return failed(&problem),
            };
            let mut input = std::io::BufReader::new(file);
            let mut output = std::io::stdout().lock();
            match serve(&mut System::new(), &mut input, &mut output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    let line = os_failure(&error).line().to_utf8().unwrap_or_default();
                    failed(&format!("session failed: {line}"))
                }
            }
        }
        _ => usage("unknown command or arguments"),
    }
}
