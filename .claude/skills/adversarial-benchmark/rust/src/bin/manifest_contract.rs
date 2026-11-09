use std::io::Write;
use std::io::{stderr, stdin, stdout};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::cli::Streams;
use sweep_stop::manifest_contract::{run, Environment, ExecLegacy};
use sweep_stop::manifest_contract_bridge::{read_request, serve, BRIDGE_ENV};
use sweep_stop::stdout::StdoutProfile;

fn main() -> ExitCode {
    // The private bridge of the in-process importers: selected by the environment so the public command line gains no argument.
    if std::env::var_os(BRIDGE_ENV).is_some() {
        let request = match read_request() {
            Ok(request) => request,
            Err(why) => {
                let _ = writeln!(stderr(), "[manifest_contract] BRIDGE ERROR {why}");
                return ExitCode::from(2);
            }
        };
        return match serve(&request, &Environment::from_process()) {
            Ok(answer) => match stdout().lock().write_all(answer.as_bytes()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(2),
            },
            Err(why) => {
                let _ = writeln!(stderr(), "[manifest_contract] BRIDGE ERROR {why}");
                ExitCode::from(2)
            }
        };
    }
    // The owner's argv is decoded with `surrogateescape`; the grammar sees the raw bytes.
    let args: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.as_bytes().to_vec())
        .collect();
    let environment = Environment::from_process();
    let (mut input, mut output, mut errors) = (stdin().lock(), stdout().lock(), stderr().lock());
    let status = run(
        &args,
        &environment,
        &mut Streams::new(
            &mut input,
            &mut output,
            &mut errors,
            StdoutProfile::of_process(),
        ),
        &mut ExecLegacy,
        &mut role_contract::manifest_gate_cli::GitBranch::from_process(),
    );
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
