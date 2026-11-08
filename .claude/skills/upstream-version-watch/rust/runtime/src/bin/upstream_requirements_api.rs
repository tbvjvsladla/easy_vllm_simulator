//! `upstream_requirements_api evaluate-jsonl [--input-fd FD]` (see `upstream_runtime::ngc_api`): the same channel rules as `upstream_version_api`.

use recipe_runtime::memory_system::{adopt_input_fd, input_fd_option};
use recipe_runtime::stdio::os_failure;
use std::io::{Read, Write};
use std::process::ExitCode;
use upstream_runtime::regen_api::evaluate_jsonl;

const USAGE: &str = "usage: upstream_requirements_api evaluate-jsonl [--input-fd FD]";

fn deliver(bytes: &[u8]) -> ExitCode {
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let line = os_failure(&error).line().to_utf8().unwrap_or_default();
            let _ = writeln!(
                std::io::stderr(),
                "upstream_requirements_api: cannot write standard output: {line}"
            );
            ExitCode::from(1)
        }
    }
}

fn usage(problem: &str) -> ExitCode {
    let _ = writeln!(
        std::io::stderr(),
        "upstream_requirements_api: {problem}\n{USAGE}"
    );
    ExitCode::from(2)
}

fn failed(problem: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "upstream_requirements_api: {problem}");
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        return usage("a command is required");
    };
    match command.as_str() {
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
            // the request is read recursively: one level of stack per nesting
            let answered = std::thread::Builder::new()
                .stack_size(256 * 1024 * 1024)
                .spawn(move || evaluate_jsonl(&bytes))
                .map_err(|error| error.to_string())
                .and_then(|handle| {
                    handle
                        .join()
                        .map_err(|_| "the evaluation panicked".to_string())
                });
            let answered = match answered {
                Ok(answered) => answered,
                Err(problem) => return failed(&problem),
            };
            match answered {
                Ok(out) => deliver(&out),
                Err(failure) => failed(&failure.line().to_utf8().unwrap_or_default()),
            }
        }
        _ => usage("unknown command or arguments"),
    }
}
