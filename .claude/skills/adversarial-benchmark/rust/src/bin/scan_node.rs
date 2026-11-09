use std::io::{stderr, stdin, stdout, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::cli::on_large_stack;
use sweep_stop::cli::Streams;
use sweep_stop::pystr::PyStr;
use sweep_stop::scan_node_boundary::{LocalClock, StdFiles};
use sweep_stop::scan_node_bridge::{serve, Environment, BRIDGE_ENV, EXIT_CONTROL_FAILURE};
use sweep_stop::scan_node_cli::{run as run_cli, StdLocal, World};
use sweep_stop::scan_node_host::StdHost;
use sweep_stop::scan_node_selfcheck::report;
use sweep_stop::stdout::StdoutProfile;

/// `scan_node`: the private typed bridge of the Python adapter (P1 and P2: `SCAN_NODE_BRIDGE` set), `--p1-self-check`, and the scanner
/// command line itself (P3, no bridge variable): the original's argparse interface and flow, `--self-test` being a named boundary.
fn main() -> ExitCode {
    // `--p1-self-check`: the native P1 rows against the embedded saved raw (the program's own check; no bridge, no Python)
    let mut args = std::env::args_os().skip(1);
    if args.next().is_some_and(|arg| arg == "--p1-self-check") && args.next().is_none() {
        let (lines, ok) = report();
        for line in lines {
            let _ = writeln!(stdout(), "[scan_node] {line}");
        }
        return if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    let environment = Environment::from_process();
    if !std::env::var_os(BRIDGE_ENV).is_some_and(|value| !value.is_empty()) {
        // the scanner command line (P3): native, with the original's argparse interface; `--self-test` is a named boundary there
        let arguments: Vec<PyStr> = std::env::args_os()
            .skip(1)
            .map(|arg| PyStr::from_surrogateescape(arg.as_bytes()))
            .collect();
        let (mut input, mut output, mut errors) =
            (stdin().lock(), stdout().lock(), stderr().lock());
        let (host, files, clock, local) = (StdHost::direct(), StdFiles, LocalClock, StdLocal);
        let world = World {
            host: &host,
            files: &files,
            clock: &clock,
            local: &local,
        };
        let status = run_cli(
            &arguments,
            &mut Streams::new(
                &mut input,
                &mut output,
                &mut errors,
                StdoutProfile::of_process(),
            ),
            &world,
        );
        return ExitCode::from(u8::try_from(status).unwrap_or(1));
    }
    let mut request = Vec::new();
    if let Err(error) = stdin().lock().read_to_end(&mut request) {
        let _ = writeln!(
            stderr(),
            "[scan_node] ERROR the bridge request cannot be read: {error}"
        );
        return ExitCode::from(EXIT_CONTROL_FAILURE as u8);
    }
    match on_large_stack(move || serve(&request, &environment, &StdFiles, &LocalClock)) {
        Ok(answer) => {
            let mut out = stdout().lock();
            if out
                .write_all(answer.as_bytes())
                .and_then(|()| out.write_all(b"\n"))
                .is_err()
            {
                return ExitCode::from(EXIT_CONTROL_FAILURE as u8);
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            let _ = writeln!(stderr(), "[scan_node] ERROR bridge: {message}");
            ExitCode::from(EXIT_CONTROL_FAILURE as u8)
        }
    }
}
