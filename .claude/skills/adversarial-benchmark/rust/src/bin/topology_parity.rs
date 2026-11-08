//! Private native helper of the topology resolution (see `sweep_stop::topology_parity_run` for the interface).

use std::io::{stderr, stdin, stdout};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::process::ExitCode;
use sweep_stop::cli::Streams;
use sweep_stop::stdout::StdoutProfile;
use sweep_stop::topology_parity_run::run;

fn main() -> ExitCode {
    let mut words = std::env::args_os().skip(1);
    if words.next().is_some_and(|a| a == "--compat-watchdog") {
        let helper = words
            .next()
            .and_then(|v| v.to_str().and_then(|t| t.parse::<i32>().ok()));
        match helper {
            Some(helper) => sweep_stop::topology_parity_run::watchdog(helper),
            None => return ExitCode::from(2),
        }
    }
    let args: Vec<Vec<u8>> = std::env::args_os().skip(1).map(|a| a.into_vec()).collect();
    let env = |name: &str| std::env::var_os(name).map(|v| v.as_bytes().to_vec());
    let (mut input, mut output, mut errors) = (stdin().lock(), stdout().lock(), stderr().lock());
    let status = run(
        args,
        &mut Streams::new(
            &mut input,
            &mut output,
            &mut errors,
            StdoutProfile::of_process(),
        ),
        &env,
    );
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
