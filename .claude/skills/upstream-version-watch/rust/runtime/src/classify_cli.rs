//! The command line `classify_failure.py` as a process: `main()` over `sys.argv` and the inherited streams, the status the interpreter would end
//! with and the report of an exception nothing handles. Standard output goes through CPython's finite checked buffering ([`LiveConsole`]); the
//! exit flush of a closed pipe is the interpreter's status 120.
//!
//! **Entry.** The executable may be copied to a path that ends in `classify_failure.py` and executed directly: `argv[0]` is then the logical script
//! path (frames of an uncaught exception name it and the default registry is `dirname(argv[0])/../failure_patterns.yaml`, read from the file system as
//! the original reads it). `PYTHONIOENCODING` may be unset or `ascii:strict`.

use crate::classify_main::{main, script_file, ClassifyWorld, Stop, PROGRAM};
use crate::live::LiveConsole;
use crate::runtime_patch::StdioProfile;
use crate::torch_main::{Refusal, REFUSAL_STATUS};
use agent_runtime_support::pystr::PyStr;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;

pub fn refusal_report(refusal: &Refusal) -> PyStr {
    PyStr::new(&format!(
        "{PROGRAM}: not decided by this port ({}): {}\n",
        refusal.boundary, refusal.detail
    ))
}

/// Runs the program over `argv` (the whole `sys.argv`) on `console`, reading standard input through `read_stdin`.
pub fn run(
    console: &mut LiveConsole,
    argv: &[PyStr],
    columns: Option<&str>,
    profile: StdioProfile,
    read_stdin: &dyn Fn() -> std::io::Result<Vec<u8>>,
) -> i32 {
    let end = {
        let mut world = ClassifyWorld {
            host: console,
            argv,
            columns,
            profile,
            read_stdin,
        };
        main(&mut world)
    };
    let status = match end {
        Ok(()) => 0,
        Err(Stop::Exit(code)) => code,
        Err(Stop::Refused(refusal)) => {
            console.write_uncaught(&refusal_report(&refusal));
            REFUSAL_STATUS
        }
        Err(Stop::Rendered(text)) => {
            console.write_uncaught(&text);
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
    let stdin = || -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        std::io::stdin().lock().read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    run(&mut console, &argv, columns, profile, &stdin)
}
