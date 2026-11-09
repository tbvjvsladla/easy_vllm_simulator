use std::io::{stderr, stdin, stdout, Read, Write};
use std::process::ExitCode;
use sweep_stop::cli::on_large_stack;
use sweep_stop::scan_node_boundary::{LocalClock, StdFiles};
use sweep_stop::scan_node_bridge::{serve, Environment, BRIDGE_ENV, EXIT_CONTROL_FAILURE};
use sweep_stop::scan_node_selfcheck::report;

/// `scan_node` P1: the private typed bridge of the Python adapter only. The scanner's command line, hardware probes and network
/// collection are other phases and keep their own explicit compat route in the adapter; this binary never stands in for them.
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
        let _ = writeln!(
            stderr(),
            "[scan_node] ERROR this executable serves the P1 bridge of the Python adapter only (the scanner command line is not native in this phase)"
        );
        return ExitCode::from(EXIT_CONTROL_FAILURE as u8);
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
