//! The command line `check_spec_layout.py`: `main()` over `sys.argv`, the status the interpreter would end with, and the report of an exception
//! nothing handles. Standard output is written live through CPython's finite checked buffering ([`LiveConsole`]), as the resolver's is.
//!
//! An exception that escapes `main` through a site the generated metadata knows is reported as the interpreter reports it (the `Traceback` block
//! of compatibility frames, see [`crate::layout_traceback`]); through any other site, as before, by its `Class: message` block alone.

use crate::cli::{exit_status, End};
use crate::host::Sink;
use crate::layout_files::LayoutWorld;
use crate::layout_main::{main, PROGRAM};
use crate::layout_sites::SiteId;
use crate::layout_traceback::render;
use crate::live::LiveConsole;
use agent_runtime_support::pystr::PyStr;

/// How a run of the command line ended, with what its report needs.
pub struct Run {
    pub end: End,
    /// The frames the exception passed through, innermost first (empty: none is known).
    frames: Vec<SiteId>,
    /// The file name the script's own frames show.
    script: PyStr,
}

/// Runs `main()` over `argv` (the whole `sys.argv`).
pub fn run(host: &mut dyn Sink, argv: &[PyStr], columns: Option<&str>) -> Run {
    let mut world = LayoutWorld::new(host, argv, columns);
    let script = world.script_file();
    let end = match main(&mut world) {
        Ok(_) => End::Returned,
        Err(thrown) if thrown.raised.class == "SystemExit" => End::Exited(thrown),
        Err(thrown) => End::Raised(thrown),
    };
    let mut frames = std::mem::take(&mut world.frames);
    if matches!(end, End::Raised(_)) && !frames.is_empty() {
        // the script's own `<module>` frame calls `main()`
        frames.push(SiteId::ModuleEntry);
    }
    Run {
        end,
        frames,
        script,
    }
}

impl Run {
    /// The status of a run: an exception nothing handled is reported as the interpreter does, then the streams are closed as the interpreter closes
    /// them (the final flush of standard output can still fail: status 120).
    pub fn conclude(self, console: &mut LiveConsole) -> i32 {
        let status = match self.end {
            End::Returned => 0,
            End::Exited(exit) => exit_status(&exit),
            End::Raised(thrown) => {
                let plain = thrown.raised.report();
                // a chained exception is not reachable through the known sites; it keeps the plain report
                let text = if self.frames.is_empty() || thrown.raised.cause.is_some() {
                    plain
                } else {
                    render(&self.frames, &self.script, &plain)
                };
                console.write_uncaught(&text);
                1
            }
        };
        console.shutdown(status)
    }
}

/// The program as a process. `script` is the caller's `argv[0]` (the Python script's absolute path when the caller is the legacy entry); `args`
/// are the arguments after it. The program name of every usage text is `check_spec_layout.py` whatever the executable is called.
pub fn run_process(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = LiveConsole::for_streams();
    let program = match script {
        Some(path) if crate::main_cmd::basename(path) == PROGRAM => path.clone(),
        _ => PyStr::new(PROGRAM),
    };
    let mut argv = vec![program];
    argv.extend(args.iter().cloned());
    let ran = run(&mut console, &argv, columns);
    ran.conclude(&mut console)
}
