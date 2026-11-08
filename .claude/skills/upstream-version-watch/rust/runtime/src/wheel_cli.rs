//! The command line `resolve_wheel.py` as a process: `main()` over `sys.argv`, the status the interpreter would end with and the report of an
//! exception nothing handles. Standard output is written live through CPython's finite checked buffering ([`LiveConsole`]); the exit flush of a
//! closed pipe is the interpreter's status 120.
//!
//! **Entry.** The executable may be copied to a path that ends in `resolve_wheel.py` and executed directly (no interpreter): `argv[0]` is then the
//! logical script path and the frames of an uncaught exception name it. Under any other file name the usage text and the frames use the canonical
//! name. A relative script path is made absolute against the working directory; symbolic links are not resolved.
//!
//! The architecture default is the machine name of this host (`uname`), as `platform.machine()` returns it; `PYTHONIOENCODING` may be unset or
//! `ascii:strict` (any other value is refused by name).

use crate::live::LiveConsole;
use crate::ngc_cli::machine_name;
use crate::runtime_patch::StdioProfile;
use crate::torch_main::{Refusal, REFUSAL_STATUS};
use crate::wheel_fetch::fetch_json_process;
use crate::wheel_main::{main, script_file, ReleaseFetcher, Stop, WheelWorld, PROGRAM};
use agent_runtime_support::pystr::PyStr;
use std::os::unix::ffi::OsStrExt;

/// The named report of an outcome this port does not decide.
pub fn refusal_report(refusal: &Refusal) -> PyStr {
    PyStr::new(&format!(
        "{PROGRAM}: not decided by this port ({}): {}\n",
        refusal.boundary, refusal.detail
    ))
}

/// Runs the program over `argv` (the whole `sys.argv`) on `console` and ends the streams as the interpreter does.
pub fn run(
    console: &mut LiveConsole,
    argv: &[PyStr],
    columns: Option<&str>,
    profile: StdioProfile,
    machine: PyStr,
    fetch: ReleaseFetcher<'_>,
) -> i32 {
    let script = script_file(argv.first());
    let mut world = WheelWorld {
        host: console,
        argv,
        columns,
        profile,
        machine,
        fetch,
    };
    let end = main(&mut world);
    let status = match end {
        Ok(()) => 0,
        Err(Stop::Exit(code)) => code,
        Err(Stop::Refused(refusal)) => {
            console.write_uncaught(&refusal_report(&refusal));
            REFUSAL_STATUS
        }
        Err(stop) => {
            if let Some(text) = stop.report(&script) {
                console.write_uncaught(&text);
            }
            1
        }
    };
    console.shutdown(status)
}

/// The stack the JSON reader recurses on (one level per nesting of the release document).
const READER_STACK_BYTES: usize = 256 * 1024 * 1024;

/// The program as a process, on a thread with the reader's stack. `script` is the caller's `argv[0]`; `args` are the arguments after it.
pub fn run_process(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
    let (script, args, columns) = (script.cloned(), args.to_vec(), columns.map(str::to_string));
    let spawned = std::thread::Builder::new()
        .stack_size(READER_STACK_BYTES)
        .spawn(move || run_on_this_thread(script.as_ref(), &args, columns.as_deref()));
    let outcome = match spawned {
        Ok(handle) => handle.join().map_err(|_| "the run panicked".to_string()),
        Err(error) => Err(format!("no thread with the reader's stack: {error}")),
    };
    outcome.unwrap_or_else(|detail| {
        let mut console = LiveConsole::for_streams();
        console.write_uncaught(&refusal_report(&Refusal {
            boundary: "process_stack",
            detail,
        }));
        console.shutdown(REFUSAL_STATUS)
    })
}

fn run_on_this_thread(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = LiveConsole::for_streams();
    let setting = std::env::var_os("PYTHONIOENCODING");
    let profile = match StdioProfile::from_setting(setting.as_deref().map(|value| value.as_bytes()))
    {
        Ok(profile) => profile,
        Err(refusal) => {
            console.write_uncaught(&refusal_report(&refusal));
            return console.shutdown(REFUSAL_STATUS);
        }
    };
    let mut argv = vec![script_file(script)];
    argv.extend(args.iter().cloned());
    run(
        &mut console,
        &argv,
        columns,
        profile,
        machine_name(),
        &fetch_json_process,
    )
}
