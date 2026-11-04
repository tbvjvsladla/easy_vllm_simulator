//! Command line of `sweep_stop`.
//!
//! The legacy surface is the original program's argparse interface (see [`crate::argv`]): `--state PATH|-`,
//! `--now-utc TEXT`, `--self-test`, `-h`. Its usage, help and argument-error text is byte-for-byte the original's,
//! including the logical program name `sweep_stop.py`.
//!
//! Rust-only additions are positional subcommands, which the original rejected, so they cannot collide with any
//! legacy invocation or abbreviation; `native-help` lists them:
//!
//! * `report stopped-by|status-line --stop-json PATH`, `report declared-repeats --state PATH`: replacements
//!   for the `python3 -c` snippets in `broad_search.sh`.
//! * `evaluate-jsonl`: differential-test transport (stdin lines in, one ASCII-safe result line out each).
//!
//! Stream encodings follow the interpreter the original ran on: stdin is UTF-8 with `surrogateescape`, stdout is
//! UTF-8 with `surrogateescape`, stderr is UTF-8 with `backslashreplace`. A `--state` *file* is opened with
//! strict UTF-8, as the original opens it.

use crate::argv::{self, LegacyArgs, Parsed, PROG};
use crate::evaluate::evaluate;
use crate::jsonl::evaluate_lines;
use crate::pyjson::{decode_file_text, decode_stdin_text, loads_text, LoadError};
use crate::pystr::{EncodeFailure, PyStr};
use crate::pyvalue::PyValue;
use crate::report;
use crate::selftest::run_self_test;
use crate::stdout::{OutputFailure, StdoutModel, StdoutProfile, EXIT_FLUSH_FAILED};
use crate::terminal;
use crate::utc::parse_utc;
use std::io::{Read, Write};
use std::sync::Mutex;

const NATIVE_HELP: &str = "sweep_stop Rust-only commands (the legacy options are unchanged; see -h):\n  sweep_stop report stopped-by --stop-json PATH\n  sweep_stop report status-line --stop-json PATH\n  sweep_stop report declared-repeats --state PATH\n  sweep_stop evaluate-jsonl            JSON lines on stdin: {id, state, now_utc}\n  sweep_stop native-help               this text\n";
/// Parsing a document nested near CPython's limit (about 10 000 levels) recurses deeply; give that work room.
const WORKER_STACK_BYTES: usize = 512 * 1024 * 1024;

/// The three standard streams, injectable so the CLI can be exercised in-process. Output to `stdout` goes through
/// the model of the interpreter's buffering ([`crate::stdout`]), which also decides when a failed write is reported.
pub struct Streams<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    model: StdoutModel,
}

impl<'a> Streams<'a> {
    pub fn new(
        stdin: &'a mut dyn Read,
        stdout: &'a mut dyn Write,
        stderr: &'a mut dyn Write,
        profile: StdoutProfile,
    ) -> Self {
        Streams {
            stdin,
            stdout,
            stderr,
            model: StdoutModel::new(profile),
        }
    }

    /// `print(text)`: the text, then a newline, as two writes. `text` carries no trailing newline of its own.
    pub(crate) fn print(&mut self, text: &PyStr) -> Result<(), PrintFailure> {
        let bytes = text
            .to_surrogateescape_bytes()
            .map_err(PrintFailure::Encode)?;
        self.model
            .write(self.stdout, &bytes)
            .map_err(PrintFailure::Write)?;
        self.model
            .write(self.stdout, b"\n")
            .map_err(PrintFailure::Write)
    }

    /// `sys.stdout.write(text)`: one write, no newline added.
    pub(crate) fn write_plain(&mut self, text: &PyStr) -> Result<(), PrintFailure> {
        let bytes = text
            .to_surrogateescape_bytes()
            .map_err(PrintFailure::Encode)?;
        self.model
            .write(self.stdout, &bytes)
            .map_err(PrintFailure::Write)
    }

    /// One `print` of a text whose last character is the newline `print` would add.
    pub(crate) fn print_body(&mut self, text: &PyStr) -> Result<(), PrintFailure> {
        let mut points = text.code_points().to_vec();
        if points.last() == Some(&u32::from('\n')) {
            points.pop();
        }
        self.print(&PyStr::from_code_points(points))
    }

    /// One `print` per line of a text made of newline-terminated lines (a transcript).
    pub(crate) fn print_lines(&mut self, text: &PyStr) -> Result<(), PrintFailure> {
        let newline = u32::from('\n');
        let mut points = text.code_points();
        while !points.is_empty() {
            let end = points
                .iter()
                .position(|&c| c == newline)
                .unwrap_or(points.len());
            self.print(&PyStr::from_code_points(points[..end].to_vec()))?;
            points = &points[(end + 1).min(points.len())..];
        }
        Ok(())
    }

    /// `parser.print_help()`: a single write that `argparse` does not let fail the program (it ignores an
    /// `OSError` there), and that goes to stderr when `sys.stdout` is `None`.
    pub(crate) fn print_help(&mut self, text: &str) {
        if self.model.is_absent() {
            write_stderr(self, &PyStr::new(text));
            return;
        }
        let _ = self.model.write(self.stdout, text.as_bytes());
    }

    /// Interpreter exit: flush what is still buffered. A failure replaces the status with 120, as CPython does.
    pub(crate) fn finish(&mut self, status: i32) -> i32 {
        match self.model.flush(self.stdout) {
            Ok(()) => status,
            Err(failure) => {
                let text = format!(
                    "Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\n{failure}\n"
                );
                write_stderr(self, &PyStr::new(&text));
                EXIT_FLUSH_FAILED
            }
        }
    }
}

/// Why a `print` did not complete.
pub(crate) enum PrintFailure {
    /// A lone surrogate cannot be encoded: `UnicodeEncodeError`, nothing was written.
    Encode(EncodeFailure),
    /// The operating system refused the write that the `print` triggered.
    Write(OutputFailure),
}

impl PrintFailure {
    /// The final traceback line, `Class: message`.
    pub(crate) fn line(&self) -> String {
        match self {
            PrintFailure::Encode(failure) => failure.to_string(),
            PrintFailure::Write(failure) => failure.to_string(),
        }
    }
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, io);
    io.finish(status)
}

fn dispatch(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let first = args.first().and_then(PyStr::to_utf8);
    match (first.as_deref(), args.len()) {
        (Some("report"), _) => run_report(&args[1..], io),
        (Some("evaluate-jsonl"), 1) => run_jsonl(io),
        (Some("native-help"), 1) => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
        _ => run_legacy(args, io),
    }
}

/// Runs `work` on a thread with a large stack, falling back to the current thread if none can be created.
pub(crate) fn on_large_stack<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    let slot = Mutex::new(Some(work));
    let take = || {
        slot.lock()
            .expect("work slot")
            .take()
            .expect("work runs once")
    };
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .stack_size(WORKER_STACK_BYTES)
            .spawn_scoped(scope, || take()())
        {
            Ok(handle) => handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
            Err(_) => take()(),
        }
    })
}

// ── output ──────────────────────────────────────────────────────────────────────

pub(crate) fn write_stderr(io: &mut Streams<'_>, text: &PyStr) {
    let _ = io.stderr.write_all(&text.to_backslashreplace_bytes());
}

/// An `OSError` message: `[Errno N] <strerror>: '<path>'`.
pub(crate) fn os_error_text(error: &std::io::Error, path: &PyStr) -> String {
    let detail = error.to_string();
    let message = detail.split(" (os error").next().unwrap_or(&detail);
    match error.raw_os_error() {
        Some(errno) => format!("[Errno {errno}] {message}: {}", path.repr()),
        None => format!("{message}: {}", path.repr()),
    }
}

/// The traceback an uncaught exception ends the program with. Source-language frames are not reproduced; the
/// compatibility contract compares the text before it and the final `Class: message` line.
pub(crate) const TRACEBACK_HEADER: &str =
    "Traceback (most recent call last):\n  (native: Python stack frames are not reproduced)\n";

/// The program dies on a failed `print`: `stderr_so_far` (warnings already written), the traceback, status 1.
pub(crate) fn print_failed(
    io: &mut Streams<'_>,
    failure: &PrintFailure,
    stderr_so_far: &str,
) -> i32 {
    let text = format!("{stderr_so_far}{TRACEBACK_HEADER}{}\n", failure.line());
    write_stderr(io, &PyStr::new(&text));
    1
}

/// The program dies on an uncaught exception: `stderr_so_far`, the traceback marker, `Class: message`, status 1.
pub(crate) fn raise(
    io: &mut Streams<'_>,
    stderr_so_far: &str,
    error: &crate::pynumber::PyException,
) -> i32 {
    let text = format!("{stderr_so_far}{TRACEBACK_HEADER}{error}\n");
    write_stderr(io, &PyStr::new(&text));
    1
}

/// Bytes of a file named by a Python string (`os.fsencode`, i.e. UTF-8 with `surrogateescape`).
pub(crate) fn read_path(path: &PyStr) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStringExt;
    let bytes = path
        .to_surrogateescape_bytes()
        .map_err(|failure| failure.message())?;
    std::fs::read(std::ffi::OsString::from_vec(bytes)).map_err(|error| os_error_text(&error, path))
}

// ── legacy interface ─────────────────────────────────────────────────────────────

/// What the heavy part of a run hands back to the thread that owns the streams.
struct Finished {
    exit_code: i32,
    stdout: Option<PyStr>,
    stderr: String,
}

impl Finished {
    fn failure(exit_code: i32, stderr: String) -> Self {
        Finished {
            exit_code,
            stdout: None,
            stderr,
        }
    }
}

fn run_legacy(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let LegacyArgs {
        state,
        now_utc,
        self_test,
    } = match argv::parse(args) {
        Parsed::Run(parsed) => parsed,
        Parsed::Help => {
            io.print_help(argv::help(terminal::columns()));
            return 0;
        }
        Parsed::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{PROG}: error: ",
                argv::usage(terminal::columns())
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
    };
    if self_test {
        let report = run_self_test();
        if let Err(failure) = io.print_lines(&PyStr::new(&report.stdout)) {
            return print_failed(io, &failure, "");
        }
        write_stderr(io, &PyStr::new(&report.stderr));
        return report.exit_code;
    }
    let (Some(state), Some(now_utc)) = (
        state.filter(|s| !s.is_empty()),
        now_utc.filter(|s| !s.is_empty()),
    ) else {
        write_stderr(
            io,
            &PyStr::new("[sweep_stop] ERROR --state 와 --now-utc 는 필수다(--self-test 제외)\n"),
        );
        return 2;
    };

    let from_stdin = state.equals("-");
    let raw = if from_stdin {
        let mut bytes = Vec::new();
        io.stdin
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|error| error.to_string())
    } else {
        read_path(&state)
    };
    let bytes = match raw {
        Ok(bytes) => bytes,
        Err(message) => {
            write_stderr(
                io,
                &PyStr::new(&format!(
                    "[sweep_stop] ERROR 상태를 읽을 수 없다: {message}\n"
                )),
            );
            return 2;
        }
    };

    let finished = on_large_stack(move || judge(&bytes, from_stdin, now_utc));
    if let Some(output) = &finished.stdout {
        if let Err(failure) = io.print_body(output) {
            return print_failed(io, &failure, &finished.stderr);
        }
    }
    write_stderr(io, &PyStr::new(&finished.stderr));
    finished.exit_code
}

/// Decodes and parses the state, evaluates it and renders the verdict; all of the original's `main` after the
/// argument checks. Runs on the large-stack thread because the parsed state may be deeply nested.
fn judge(bytes: &[u8], from_stdin: bool, now_utc: PyStr) -> Finished {
    let text = if from_stdin {
        Ok(decode_stdin_text(bytes))
    } else {
        decode_file_text(bytes)
    };
    let state = text.and_then(|text| loads_text(&text));
    let state = match state {
        Ok(state) => state,
        Err(error @ LoadError::TooDeep { .. }) => {
            return Finished::failure(1, format!("{error}\n"))
        }
        Err(error) => {
            return Finished::failure(
                2,
                format!("[sweep_stop] ERROR 상태를 읽을 수 없다: {error}\n"),
            );
        }
    };
    let verdict =
        parse_utc(&PyValue::Str(now_utc), "--now-utc").and_then(|now| evaluate(&state, &now));
    match verdict {
        Ok(evaluation) => match evaluation.to_value().dumps_pretty() {
            Ok(mut json) => {
                json.push_str("\n");
                Finished {
                    exit_code: evaluation.exit_code(),
                    stdout: Some(json),
                    stderr: String::new(),
                }
            }
            Err(error) => Finished::failure(1, format!("{error}\n")),
        },
        Err(error) if error.is_declaration_error() => {
            Finished::failure(2, format!("[sweep_stop] ERROR {error}\n"))
        }
        Err(error) => Finished::failure(1, format!("{}: {error}\n", error.python_type())),
    }
}

// ── Rust-only subcommands ────────────────────────────────────────────────────────

fn run_report(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let usage = "usage: sweep_stop report (stopped-by|status-line) --stop-json PATH | report declared-repeats --state PATH\n";
    let [kind, flag, path] = args else {
        write_stderr(io, &PyStr::new(usage));
        return 2;
    };
    let (Some(kind), Some(flag)) = (kind.to_utf8(), flag.to_utf8()) else {
        write_stderr(io, &PyStr::new(usage));
        return 2;
    };
    let line = match (kind.as_str(), flag.as_str()) {
        ("stopped-by", "--stop-json") => on_large_stack(|| report::stopped_by_line(path)),
        ("status-line", "--stop-json") => on_large_stack(|| report::status_line(path)),
        ("declared-repeats", "--state") => on_large_stack(|| report::declared_repeats_line(path)),
        _ => {
            write_stderr(io, &PyStr::new(usage));
            return 2;
        }
    };
    match line {
        Ok(line) => match io.print(&line) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
        Err(error) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[sweep_stop] ERROR report {kind}: {error}\n")),
            );
            1
        }
    }
}

fn run_jsonl(io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[sweep_stop] ERROR stdin: {error}\n")),
        );
        return 2;
    }
    // The transport is deliberately strict UTF-8 in and ASCII out; it is not the legacy stream behaviour.
    let text = match decode_file_text(&bytes) {
        Ok(text) => text
            .to_utf8()
            .expect("strict decoding yields no surrogates"),
        Err(error) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[sweep_stop] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| evaluate_lines(&text));
    match io.print_lines(&PyStr::new(&output)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure, ""),
    }
}
