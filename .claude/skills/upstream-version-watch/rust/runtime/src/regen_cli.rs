//! The command line `regen_requirements.py` as a process: `main()` over `sys.argv`, the status the interpreter would end with and the report of an
//! exception nothing handles. Standard output goes through CPython's finite checked buffering ([`LiveConsole`]); the exit flush of a closed pipe is the
//! interpreter's status 120.
//!
//! **Entry.** The executable may be copied to a path that ends in `regen_requirements.py` and executed directly (no interpreter): `argv[0]` is then the
//! logical script path and the frames of an uncaught exception name it. Under any other file name the usage text and the frames use the canonical name.
//! `PYTHONPATH` (the declared installed-distribution search path), `TMPDIR`/`TEMP`/`TMP`, the proxy variables and the trust variables are read from
//! this process's environment; `PYTHONIOENCODING` may be unset or `ascii:strict`. `urlretrieve` runs without a socket timeout, as in the original.

use crate::fetch_https::FetchEnvironment;
use crate::http_direct::UreqDialer;
use crate::live::LiveConsole;
use crate::regen_main::{main, script_file, BaseChoice, RegenWorld, Stop, PROGRAM};
use crate::runtime_patch::StdioProfile;
use crate::torch_main::{Refusal, REFUSAL_STATUS};
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
) -> i32 {
    let script = script_file(argv.first());
    let environment = FetchEnvironment::from_process();
    let dialer = UreqDialer::unbounded();
    let pythonpath = std::env::var_os("PYTHONPATH");
    let end = {
        let mut world = RegenWorld::new(
            console,
            argv,
            columns,
            profile,
            &environment,
            &dialer,
            pythonpath,
            BaseChoice::Process,
        );
        main(&mut world)
    };
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

/// The program as a process. `script` is the caller's `argv[0]`; `args` are the arguments after it.
pub fn run_process(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
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
    run(&mut console, &argv, columns, profile)
}
