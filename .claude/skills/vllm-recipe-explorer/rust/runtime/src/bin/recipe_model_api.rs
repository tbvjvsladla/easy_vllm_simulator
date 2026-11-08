use agent_runtime_support::pystr::PyStr;
use recipe_runtime::api::render;
use recipe_runtime::model_api::{answer, constants};
use recipe_runtime::stdio::os_failure;
use std::io::{Read, Write};
use std::process::ExitCode;

/// Writes the whole answer to standard output; a failing descriptor is a deliberate diagnostic and a non-zero status.
fn deliver(bytes: &[u8]) -> ExitCode {
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let line = os_failure(&error).line().to_utf8().unwrap_or_default();
            let _ = writeln!(
                std::io::stderr(),
                "recipe_model_api: cannot write standard output: {line}"
            );
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("constants") => match render(&constants()) {
            Ok(text) => deliver(format!("{text}\n").as_bytes()),
            Err(failure) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "{}",
                    failure.line().to_utf8().unwrap_or_default()
                );
                ExitCode::from(1)
            }
        },
        Some("evaluate-jsonl") => {
            let mut bytes = Vec::new();
            if let Err(error) = std::io::stdin().lock().read_to_end(&mut bytes) {
                // The bytes read before the failure are not used: a request stream that broke is not a shorter stream.
                let line = os_failure(&error).line().to_utf8().unwrap_or_default();
                let _ = writeln!(
                    std::io::stderr(),
                    "recipe_model_api: cannot read standard input: {line}"
                );
                return ExitCode::from(1);
            }
            let text = PyStr::from_surrogateescape(&bytes);
            let mut out = Vec::new();
            let newline = u32::from('\n');
            for piece in text.code_points().split(|&point| point == newline) {
                let line = PyStr::from_code_points(piece.to_vec());
                if line.strip().is_empty() {
                    continue;
                }
                match render(&answer(&line)) {
                    Ok(row) => {
                        out.extend_from_slice(row.as_bytes());
                        out.push(b'\n');
                    }
                    Err(failure) => {
                        let _ = writeln!(
                            std::io::stderr(),
                            "recipe_model_api: {}",
                            failure.line().to_utf8().unwrap_or_default()
                        );
                        return ExitCode::from(1);
                    }
                }
            }
            deliver(&out)
        }
        _ => {
            let _ = writeln!(
                std::io::stderr(),
                "usage: recipe_model_api constants | evaluate-jsonl"
            );
            ExitCode::from(2)
        }
    }
}
