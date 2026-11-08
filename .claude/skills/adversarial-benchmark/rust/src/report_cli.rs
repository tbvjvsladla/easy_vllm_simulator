//! What the four reporting programs share at the process boundary: collecting the output of a run, ending it the
//! way the interpreter would (`SystemExit` codes, an uncaught exception's traceback marker and status 1), and
//! writing the collected streams.
//!
//! A run never touches the real streams while it works. It records what the original would have written to
//! `sys.stdout` (as the two kinds of write the original uses) and to `sys.stderr`; [`finish`] then writes both,
//! which keeps everything written before a failure visible, as it is in the original.

use crate::argv::{self, Outcome, Spec};
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::guidellm::Failure;
use crate::pystr::PyStr;
use crate::report_support::Diagnostics;
use crate::terminal;

/// One write to `sys.stdout`.
pub enum StdoutWrite {
    /// `sys.stdout.write(text)`.
    Plain(PyStr),
    /// `print(text)`.
    Line(PyStr),
}

/// The output of one run, filled in as the program proceeds.
#[derive(Default)]
pub struct Run {
    pub stdout: Vec<StdoutWrite>,
    pub diagnostics: Diagnostics,
}

impl Run {
    pub fn print(&mut self, text: PyStr) {
        self.stdout.push(StdoutWrite::Line(text));
    }

    pub fn write_plain(&mut self, text: PyStr) {
        self.stdout.push(StdoutWrite::Plain(text));
    }

    pub fn warn(&mut self, text: &str) {
        self.diagnostics.write_str(text);
    }
}

/// How a run that ended with `failure` leaves the process: a `SystemExit` is its code (a text message is printed and
/// is status 1); any other exception is the traceback marker and `Class: message`, status 1.
fn status_of(run: &mut Run, failure: &Failure) -> i32 {
    if failure.class == "SystemExit" {
        return match failure
            .message
            .to_utf8()
            .and_then(|m| m.parse::<i32>().ok())
        {
            Some(code) => code,
            None => {
                if failure.message.equals("None") {
                    0
                } else {
                    run.diagnostics.write(&failure.message);
                    run.diagnostics.write_str("\n");
                    1
                }
            }
        };
    }
    run.diagnostics.write_str(TRACEBACK_HEADER);
    // Exceptions defined in the imported naming module print with the module's name; those of `__main__` do not.
    if matches!(
        failure.class,
        crate::bench_naming::NAMING_COLLISION_EXHAUSTED
            | crate::bench_naming::NAMING_SOURCE_UNREADABLE
    ) {
        run.diagnostics.write_str("doc_naming.");
    }
    run.diagnostics.write(&failure.line());
    run.diagnostics.write_str("\n");
    1
}

/// Ends the run: the status the program exits with, and the streams written.
pub fn finish(run: Run, outcome: Result<i32, Failure>, io: &mut Streams<'_>) -> i32 {
    let mut run = run;
    let status = match outcome {
        Ok(code) => code,
        Err(failure) => status_of(&mut run, &failure),
    };
    for write in &run.stdout {
        let result = match write {
            StdoutWrite::Plain(text) => io.write_plain(text),
            StdoutWrite::Line(text) => io.print(text),
        };
        if let Err(failure) = result {
            let so_far = run.diagnostics.text().to_utf8().unwrap_or_default();
            return crate::cli::print_failed(io, &failure, &so_far);
        }
    }
    write_stderr(io, run.diagnostics.text());
    status
}

/// Parses `args` with the original's argparse `spec`. `Ok` carries the matches; the other outcomes are already
/// complete runs (help, or the usage and error text with status 2).
pub fn parse_or_finish(
    spec: &Spec,
    args: &[PyStr],
    io: &mut Streams<'_>,
) -> Result<argv::Matches, i32> {
    match argv::parse_with(spec, args) {
        Outcome::Run(matches) => Ok(matches),
        Outcome::Help => {
            io.print_help(spec.help(terminal::columns()));
            Err(io.finish(0))
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{}: error: ",
                spec.usage(terminal::columns()),
                spec.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            Err(io.finish(2))
        }
    }
}

/// `parser.error(message)` after a successful parse: usage, `prog: error: message`, status 2.
pub fn parser_error(spec: &Spec, message: &str) -> (PyStr, i32) {
    let text = format!(
        "{}{}: error: {message}\n",
        spec.usage(terminal::columns()),
        spec.prog
    );
    (PyStr::new(&text), 2)
}
