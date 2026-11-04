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
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::report;
use crate::selftest::run_self_test;
use crate::terminal;
use crate::utc::parse_utc;
use std::io::{Read, Write};
use std::sync::Mutex;

const NATIVE_HELP: &str = "sweep_stop Rust-only commands (the legacy options are unchanged; see -h):\n  sweep_stop report stopped-by --stop-json PATH\n  sweep_stop report status-line --stop-json PATH\n  sweep_stop report declared-repeats --state PATH\n  sweep_stop evaluate-jsonl            JSON lines on stdin: {id, state, now_utc}\n  sweep_stop native-help               this text\n";
/// Parsing a document nested near CPython's limit (about 10 000 levels) recurses deeply; give that work room.
const WORKER_STACK_BYTES: usize = 512 * 1024 * 1024;

/// The three standard streams, injectable so the CLI can be exercised in-process.
pub struct Streams<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let first = args.first().and_then(PyStr::to_utf8);
    match (first.as_deref(), args.len()) {
        (Some("report"), _) => run_report(&args[1..], io),
        (Some("evaluate-jsonl"), 1) => run_jsonl(io),
        (Some("native-help"), 1) => {
            write_stdout(io, &PyStr::new(NATIVE_HELP));
            0
        }
        _ => run_legacy(args, io),
    }
}

/// Runs `work` on a thread with a large stack, falling back to the current thread if none can be created.
fn on_large_stack<T: Send>(work: impl FnOnce() -> T + Send) -> T {
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

/// Writes text to stdout like `print` on this interpreter. A lone surrogate other than `U+DC80..U+DCFF` makes
/// Python raise before anything is written; the failure is reported on stderr and `false` is returned.
fn write_stdout(io: &mut Streams<'_>, text: &PyStr) -> bool {
    match text.to_surrogateescape_bytes() {
        Ok(bytes) => {
            // A closed pipe must not turn a verdict into a panic; the exit status still carries the decision.
            let _ = io.stdout.write_all(&bytes);
            true
        }
        Err(failure) => {
            write_stderr(io, &PyStr::new(&format!("{failure}\n")));
            false
        }
    }
}

fn write_stderr(io: &mut Streams<'_>, text: &PyStr) {
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
            write_stdout(io, &PyStr::new(argv::help(terminal::columns())));
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
        write_stdout(io, &PyStr::new(&report.stdout));
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
        if !write_stdout(io, output) {
            return 1;
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
        Ok(mut line) => {
            line.push_str("\n");
            if write_stdout(io, &line) {
                0
            } else {
                1
            }
        }
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
    write_stdout(io, &PyStr::new(&output));
    0
}
