use std::io::{stderr, stdin, stdout, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::cli::{on_large_stack, Streams};
use sweep_stop::stdout::StdoutProfile;
use sweep_stop::turn_budget_bridge::{serve, BRIDGE_ENV};
use sweep_stop::turn_budget_cli::{run, Environment, EXIT_CONTROL_FAILURE};

/// Load-time constructor: records which standard descriptors the CALLER left closed, before Rust's start-up re-opens them as `/dev/null`
/// (`sys.stdout` is `None` in a Python started with descriptor 1 closed, a real stream with `/dev/null`: the difference is observable).
#[used]
#[cfg_attr(target_os = "linux", link_section = ".init_array")]
static RECORD_STANDARD_DESCRIPTORS: extern "C" fn() = record_at_load;

extern "C" fn record_at_load() {
    sweep_stop::startup_descriptors::record_standard_descriptors();
}

fn main() -> ExitCode {
    let environment = Environment::from_process();
    // Private transport of the in-process importers (selected by the environment so that the command line gains no argument).
    if std::env::var_os(BRIDGE_ENV).is_some_and(|value| !value.is_empty()) {
        let mut request = Vec::new();
        if let Err(error) = stdin().lock().read_to_end(&mut request) {
            let _ = writeln!(
                stderr(),
                "[turn_budget] ERROR the bridge request cannot be read: {error}"
            );
            return ExitCode::from(EXIT_CONTROL_FAILURE as u8);
        }
        return match on_large_stack(move || serve(&request, &environment)) {
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
                let _ = writeln!(stderr(), "[turn_budget] ERROR bridge: {message}");
                ExitCode::from(EXIT_CONTROL_FAILURE as u8)
            }
        };
    }
    // The owner's argv is decoded with the file-system codec; the grammar sees the raw bytes.
    let args: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.as_bytes().to_vec())
        .collect();
    let status = on_large_stack(move || {
        let (mut input, mut output, mut errors) =
            (stdin().lock(), stdout().lock(), stderr().lock());
        run(
            &args,
            &environment,
            &mut Streams::new(&mut input, &mut output, &mut errors, process_profile()),
        )
    });
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}

/// The stdout profile of this process: descriptor 1 as the program sees it now, and `absent` when the caller started it with descriptor 1 closed.
fn process_profile() -> StdoutProfile {
    let mut profile = StdoutProfile::of_process();
    if sweep_stop::startup_descriptors::closed_at_start(1) == Some(true) {
        profile.absent = true;
    }
    profile
}
