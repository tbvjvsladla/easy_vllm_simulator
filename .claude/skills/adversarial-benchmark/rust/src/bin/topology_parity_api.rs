use std::io::{stderr, stdin, stdout};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::cli::{on_large_stack, Streams};
use sweep_stop::pystr::PyStr;
use sweep_stop::stdout::StdoutProfile;
use sweep_stop::topology_parity_api::run;

fn main() -> ExitCode {
    let args: Vec<PyStr> = std::env::args_os()
        .skip(1)
        .map(|arg| PyStr::from_surrogateescape(arg.as_bytes()))
        .collect();
    let status = on_large_stack(move || {
        let (mut input, mut output, mut errors) =
            (stdin().lock(), stdout().lock(), stderr().lock());
        let env = |name: &str| std::env::var_os(name).map(|v| v.as_bytes().to_vec());
        run(
            &args,
            &mut Streams::new(
                &mut input,
                &mut output,
                &mut errors,
                StdoutProfile::of_process(),
            ),
            &env,
        )
    });
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
