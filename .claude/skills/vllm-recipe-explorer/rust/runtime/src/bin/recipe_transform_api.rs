//! `recipe_transform_api constants | evaluate-jsonl [--input-fd FD]` (see `recipe_runtime::transform_api`).
//! `--input-fd FD` only chooses the channel the requests are read from: a readable descriptor the caller passed to this process,
//! above 2; standard input is then left alone. Nothing else selects a file, a program or a provider.

use recipe_runtime::memory_system::{adopt_input_fd, input_fd_option};
use recipe_runtime::stdio::os_failure;
use recipe_runtime::transform_api::{constants, evaluate_jsonl, render};
use std::io::{Read, Write};
use std::process::ExitCode;

const USAGE: &str = "usage: recipe_transform_api constants | evaluate-jsonl [--input-fd FD]";

fn deliver(bytes: &[u8]) -> ExitCode {
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let line = os_failure(&error).line().to_utf8().unwrap_or_default();
            let _ = writeln!(
                std::io::stderr(),
                "recipe_transform_api: cannot write standard output: {line}"
            );
            ExitCode::from(1)
        }
    }
}

fn usage(problem: &str) -> ExitCode {
    let _ = writeln!(
        std::io::stderr(),
        "recipe_transform_api: {problem}\n{USAGE}"
    );
    ExitCode::from(2)
}

fn failed(problem: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "recipe_transform_api: {problem}");
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
                // SAFETY: `number` was validated by `input_fd_option` (decimal, above 2, so neither standard stream); it is the one
                // descriptor the caller passed as this command's protocol channel (the documented `pass_fds` contract), this is the
                // only adoption in the process, and nothing else wraps it.
                Some(number) => match unsafe { adopt_input_fd(number) } {
                    Ok(mut file) => file.read_to_end(&mut bytes),
                    Err(problem) => return failed(&problem),
                },
                None => std::io::stdin().lock().read_to_end(&mut bytes),
            };
            if let Err(error) = read {
                let line = os_failure(&error).line().to_utf8().unwrap_or_default();
                return failed(&format!("cannot read the requests: {line}"));
            }
            match evaluate_jsonl(&bytes) {
                Ok(out) => deliver(&out),
                Err(failure) => failed(&failure.line().to_utf8().unwrap_or_default()),
            }
        }
        _ => usage("unknown command or arguments"),
    }
}
