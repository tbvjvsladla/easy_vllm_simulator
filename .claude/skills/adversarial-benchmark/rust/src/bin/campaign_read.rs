use std::io::{stderr, stdin, stdout};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::campaign_read::{run, Environment, ExecLegacy};
use sweep_stop::cli::Streams;
use sweep_stop::stdout::StdoutProfile;

fn main() -> ExitCode {
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
    );
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
