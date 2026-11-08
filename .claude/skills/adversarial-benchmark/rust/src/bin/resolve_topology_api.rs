use std::io::{stderr, stdin, stdout};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::cli::{on_large_stack, Streams};
use sweep_stop::pystr::PyStr;
use sweep_stop::resolve_topology_api::run;
use sweep_stop::stdout::StdoutProfile;

fn main() -> ExitCode {
    // Python decodes argv with `surrogateescape`; so do we, so odd bytes in arguments stay representable.
    let args: Vec<PyStr> = std::env::args_os()
        .skip(1)
        .map(|arg| PyStr::from_surrogateescape(arg.as_bytes()))
        .collect();
    // Deeply nested JSON documents recurse far; the whole run is on a large stack so output order stays that of one thread.
    let status = on_large_stack(move || {
        let (mut input, mut output, mut errors) =
            (stdin().lock(), stdout().lock(), stderr().lock());
        run(
            &args,
            &mut Streams::new(
                &mut input,
                &mut output,
                &mut errors,
                StdoutProfile::of_process(),
            ),
        )
    });
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
