//! The command line of `cleanup_docker.py` (`main()` with `canonical_only=False`): `cleanup_docker [--apply] [--cache-age-hours N] [--keep REF]...`.
//!
//! The arguments are parsed as `argparse` does (abbreviations, the last of a repeated `--cache-age-hours`, repeated `--keep`, `--help`), the
//! original's table is printed, and with `--apply` the real `docker rmi` and `docker builder prune` run through the process's own `PATH`.
//! `main` returns `None`, so the status is 0 whatever was printed; a `docker image ls` that fails is `sys.exit(2)`; an exception nothing
//! handles is reported on standard error and is status 1. Standard output is written LIVE through CPython's finite checked buffering
//! ([`LiveConsole`]): a failing write is the exception of the `print` that made it and stops the business code at that point; a failure of the
//! final flush is the interpreter's "Exception ignored" report and status 120. The default `REPO` comes from where the INSTALLED binary lives: the skill root is
//! five directories up from the executable (lexically), `REPO` is three more up, as `os.path.abspath(<script dir>/../../../..)` is.

use crate::anchor::process_skill_root;
use crate::cleanup_docker::{
    encode_thrown, main, parser_write_failure, write_thrown, CleanupConsole, Done, Thrown, World,
};
use crate::escalation::Raised;
use crate::feedback::abspath;
use crate::memory_cli::write_real_stderr;
use crate::memory_system::{startup_stdio, StdioAtStart};
use crate::model_files::join;
use crate::pyexpr::Res;
use crate::stdio::{inherited_stdout, os_failure, TextStdout};
use crate::transform_cli::{read_process_stdin, Console};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::IsTerminal;

/// The program name of `sys.argv[0]` in the normal command line (the usage says `cleanup_docker.py`).
pub const PROGRAM: &str = "cleanup_docker.py";

/// The default `REPO` of the installed binary: `os.path.abspath(<skill root>/../../..)`.
pub fn default_repo() -> Res<PyStr> {
    let mut path = process_skill_root()?;
    for _ in 0..3 {
        path = join(&path, &PyStr::new(".."));
    }
    abspath(&PyValue::Str(path))
}

/// The inherited standard streams as `sys.stdout` and `sys.stderr` are: standard output through the text layer and the byte layer of
/// CPython (nothing reaches the descriptor until a layer overflows, a failure is reported by the write that hit it), standard error
/// unbuffered. A stream that was closed at start is `None`: a `print` to it is a no-op, and `print(..., file=sys.stderr)` without a
/// standard error goes to the standard output stream.
pub struct LiveConsole {
    start: StdioAtStart,
    stdout: Option<TextStdout<std::fs::File>>,
    open_error: Option<std::io::Error>,
    parser_failure: Option<Box<Thrown>>,
}

impl LiveConsole {
    pub fn new() -> LiveConsole {
        let start = startup_stdio();
        let (stdout, open_error) = if start.stdout_open {
            match inherited_stdout() {
                // A terminal makes the text layer line-buffered, as `sys.stdout` is when `isatty()` is true.
                Ok((file, capacity)) => (
                    Some(
                        TextStdout::new(file, capacity)
                            .line_buffered(std::io::stdout().is_terminal()),
                    ),
                    None,
                ),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };
        LiveConsole {
            start,
            stdout,
            open_error,
            parser_failure: None,
        }
    }

    /// One argparse write to standard output: a failure argparse does not swallow stops the parser (see `take_parser_failure`).
    pub fn parser_stdout(&mut self, text: &PyStr) {
        let outcome = self.out(text);
        if let Some(failure) = parser_write_failure(outcome) {
            self.parser_failure = Some(failure);
        }
    }

    /// The flush at interpreter exit: a failure is the "Exception ignored" report on standard error and status 120, whatever the status was.
    pub fn shutdown(&mut self, status: i32) -> i32 {
        let Some(stdout) = self.stdout.as_mut() else {
            return status;
        };
        match stdout.finish() {
            Ok(()) => status,
            Err(error) => {
                if self.start.stderr_open {
                    let mut report = PyStr::new(
                        "Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\n",
                    );
                    report.push_py(&os_failure(&error).line());
                    report.push_str("\n");
                    let _ = write_real_stderr(&report);
                }
                120
            }
        }
    }
}

impl Default for LiveConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupConsole for LiveConsole {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        if !self.start.stdout_open {
            return Ok(());
        }
        let bytes = text
            .to_surrogateescape_bytes()
            .map_err(|failure| encode_thrown(text, &failure))?;
        match (self.stdout.as_mut(), self.open_error.as_ref()) {
            (Some(stdout), _) => stdout.write(&bytes).map_err(|error| write_thrown(&error)),
            (None, Some(error)) => Err(write_thrown(error)),
            (None, None) => Ok(()),
        }
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        if self.start.stderr_open {
            write_real_stderr(text).map_err(|failure| Thrown::plain(Raised::from(failure)))
        } else {
            self.out(text)
        }
    }

    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        self.parser_failure.take()
    }
}

impl Console for LiveConsole {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        if self.start.stderr_open {
            write_real_stderr(text)
        } else {
            let _ = self.out(text);
            Ok(())
        }
    }

    fn write_stdout(&mut self, text: &PyStr) {
        let _ = self.out(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        if self.start.stderr_open {
            for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
                let _ = write_real_stderr(text);
            }
        } else if self.start.stdout_open && !usage.is_empty() {
            self.parser_stdout(usage);
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        if self.start.stdout_open {
            self.parser_stdout(help);
        } else if self.start.stderr_open {
            let _ = write_real_stderr(help);
        }
    }

    fn write_uncaught(&mut self, text: &PyStr) {
        if self.start.stderr_open {
            let _ = write_real_stderr(text);
        }
    }

    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        read_process_stdin()
    }
}

/// How a run of the command line ended.
pub enum End {
    /// `main` returned `None`.
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

/// Runs `main(canonical_only=False)` over `argv` (the whole `sys.argv`).
pub fn run(
    console: &mut dyn CleanupConsole,
    repo: &PyValue,
    argv: &[PyStr],
    columns: Option<&str>,
    tracing: bool,
) -> End {
    let outcome: Done<()> = main(
        &mut World { console, tracing },
        repo,
        argv,
        &PyValue::Bool(false),
        columns,
    );
    match outcome {
        Ok(()) => End::Returned,
        Err(thrown) if thrown.raised.class == "SystemExit" => End::Exited(thrown),
        Err(thrown) => End::Raised(thrown),
    }
}

/// The status of a run: an exception nothing handled is reported as the interpreter does, then the streams are closed as the
/// interpreter closes them (the final flush of standard output can still fail: status 120).
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

/// The program as a process: `args` are the arguments after the program name.
pub fn run_process(args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = LiveConsole::new();
    let repo = match default_repo() {
        Ok(repo) => repo,
        Err(failure) => {
            console.write_uncaught(&Raised::from(failure).report());
            return console.shutdown(1);
        }
    };
    let mut argv = vec![PyStr::new(PROGRAM)];
    argv.extend(args.iter().cloned());
    let end = run(&mut console, &PyValue::Str(repo), &argv, columns, false);
    conclude(&mut console, end)
}
