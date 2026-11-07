//! The command line `resolve_build_track.py`: `main()` over `sys.argv`, the status the interpreter would end with, and the report of an exception
//! nothing handles. Standard output is written live through CPython's finite checked buffering ([`LiveConsole`]).

use crate::host::{Host, YamlProfile};
use crate::live::LiveConsole;
use crate::main_cmd::{main, PROGRAM};
use crate::manifest::World;
use crate::thrown::Thrown;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

/// How a run of the command line ended.
pub enum End {
    /// `main` returned.
    Returned,
    /// `SystemExit` (argparse, `sys.exit(2)`).
    Exited(Box<Thrown>),
    /// An exception nothing handled.
    Raised(Box<Thrown>),
}

/// The process status of a `SystemExit`: its integer code.
pub fn exit_status(exit: &Thrown) -> i32 {
    match exit.raised.code.as_deref().map(PyValue::peel) {
        Some(PyValue::Int(code)) => i32::try_from(code).unwrap_or(1),
        Some(PyValue::None) | None => 0,
        Some(_) => 1,
    }
}

/// Runs `main()` over `argv` (the whole `sys.argv`).
pub fn run(host: &mut dyn Host, argv: &[PyStr], columns: Option<&str>) -> End {
    match main(&mut World {
        host,
        argv,
        columns,
    }) {
        Ok(_) => End::Returned,
        Err(thrown) if thrown.raised.class == "SystemExit" => End::Exited(thrown),
        Err(thrown) => End::Raised(thrown),
    }
}

/// The status of a run: an exception nothing handled is reported as the interpreter does, then the streams are closed as the interpreter closes
/// them (the final flush of standard output can still fail: status 120).
pub fn conclude(console: &mut LiveConsole, end: End) -> i32 {
    let status = match end {
        End::Returned => 0,
        End::Exited(exit) => exit_status(&exit),
        End::Raised(thrown) => {
            console.write_uncaught(&thrown.raised.report());
            1
        }
    };
    console.shutdown(status)
}

/// The program as a process: `args` are the arguments after the program name, which is `resolve_build_track.py` whatever the executable is called.
pub fn run_process(args: &[PyStr], columns: Option<&str>, profile: YamlProfile) -> i32 {
    let mut console = LiveConsole::new(profile);
    let mut argv = vec![PyStr::new(PROGRAM)];
    argv.extend(args.iter().cloned());
    let end = run(&mut console, &argv, columns);
    conclude(&mut console, end)
}
