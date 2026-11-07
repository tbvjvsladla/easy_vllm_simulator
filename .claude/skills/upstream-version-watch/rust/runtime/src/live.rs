//! The inherited standard streams as `sys.stdout` and `sys.stderr` are, for the command line and for the checks' programs.
//!
//! Standard output goes through the text layer and the byte layer of CPython (`recipe_runtime::stdio::TextStdout`: nothing reaches the
//! descriptor until a layer overflows, a failure is reported by the write that hit it), standard error is unbuffered. A stream that was
//! closed at start is `None`: a `print` to it is a no-op, and `print(..., file=sys.stderr)` without a standard error goes to standard
//! output. The exit flush of standard output can still fail: the "Exception ignored" report and status 120.

use crate::host::{parser_write_failure, Host, Sink, YamlProfile};
use crate::thrown::{encode_thrown, write_thrown, Done, Thrown};
use agent_runtime_support::pystr::PyStr;
use recipe_runtime::escalation::Raised;
use recipe_runtime::memory_cli::write_real_stderr;
use recipe_runtime::memory_system::{startup_stdio, StdioAtStart};
use recipe_runtime::stdio::{inherited_stdout, os_failure, TextStdout};
use std::io::IsTerminal;

pub struct LiveConsole {
    pub start: StdioAtStart,
    stdout: Option<TextStdout<std::fs::File>>,
    open_error: Option<std::io::Error>,
    parser_failure: Option<Box<Thrown>>,
    profile: YamlProfile,
}

impl LiveConsole {
    pub fn new(profile: YamlProfile) -> LiveConsole {
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
            profile,
        }
    }

    /// The console of a program that imports nothing (the profile is the one of an installation that answers every import; it is never asked).
    pub fn for_streams() -> LiveConsole {
        LiveConsole::new(YamlProfile::Available)
    }

    /// The report of an exception nothing handled, on the real standard error (a failure to write it is ignored).
    pub fn write_uncaught(&mut self, text: &PyStr) {
        if self.start.stderr_open {
            let _ = write_real_stderr(text);
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

    fn parser_stdout(&mut self, text: &PyStr) {
        let outcome = self.out(text);
        if let Some(failure) = parser_write_failure(outcome) {
            self.parser_failure = Some(failure);
        }
    }
}

impl Sink for LiveConsole {
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

    fn parser_help(&mut self, help: &PyStr) {
        if self.start.stdout_open {
            self.parser_stdout(help);
        } else if self.start.stderr_open {
            let _ = write_real_stderr(help);
        }
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

    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        self.parser_failure.take()
    }
}

impl Host for LiveConsole {
    fn import_yaml(&mut self) -> Done<()> {
        self.profile.import_yaml()
    }
}
