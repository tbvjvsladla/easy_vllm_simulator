//! Public `campaign_read`: the read shapes `--active` / `--derive` of `campaign_init`, answered natively; any other vector is a
//! control failure (status 2) - the full command line is the `campaign_init` program. No shape starts a Python interpreter.
use std::io::{stderr, stdin, stdout};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::campaign_read::{run, Environment};
use sweep_stop::cli::Streams;
use sweep_stop::stdout::StdoutProfile;

fn main() -> ExitCode {
    let args: Vec<Vec<u8>> = std::env::args_os().skip(1).map(|arg| arg.as_bytes().to_vec()).collect();
    let environment = Environment::from_process();
    let (mut input, mut output, mut errors) = (stdin().lock(), stdout().lock(), stderr().lock());
    let status = run(
        &args,
        &environment,
        &mut Streams::new(&mut input, &mut output, &mut errors, StdoutProfile::of_process()),
    );
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
