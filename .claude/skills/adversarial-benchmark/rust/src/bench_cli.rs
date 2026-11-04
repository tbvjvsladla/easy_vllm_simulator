//! Command line of `parse_bench`.
//!
//! The legacy surface is the original program's argparse interface ([`crate::argv::PARSE_BENCH`]): `--bench-json
//! PATH` (required), `--engine-log PATH`, the accepted-but-unused `--json`, `-h`. Usage, help and argument-error
//! text is byte-for-byte the original's, including the logical program name `parse_bench.py`.
//!
//! Rust-only additions are positional words, which the original rejected, so they cannot collide with a legacy
//! invocation: `self-test` (native bootstrap check, see [`crate::bench_selftest`]) and `native-help`.
//!
//! Failure timing follows the original. Everything up to and including loading the bench file is inside the
//! original's `try/except Exception`: its failures print `[parse_bench] ERROR bench-json: ...` and exit 2. What
//! happens after the load is not guarded: those exceptions end the program with status 1 and a traceback, of
//! which only the final `Class: message` line is reproduced. Engine-log problems are warnings.

use crate::argv::{self, Outcome, BENCH_JSON, ENGINE_LOG, PARSE_BENCH};
use crate::bench_selftest::run_self_test;
use crate::cli::{
    on_large_stack, print_failed, read_path, write_stderr, Streams, TRACEBACK_HEADER,
};
use crate::parse_bench::measure;
use crate::pyjson::{decode_file_text, loads_text, LoadError};
use crate::pystr::PyStr;
use crate::terminal;

const NATIVE_HELP: &str = "parse_bench Rust-only commands (the legacy options are unchanged; see -h):\n  parse_bench self-test     native checks on in-memory fixtures (not a measurement)\n  parse_bench native-help   this text\n";

/// What the heavy part of a run hands back to the thread that owns the streams.
struct Finished {
    exit_code: i32,
    stdout: Option<PyStr>,
    stderr: String,
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, io);
    io.finish(status)
}

fn dispatch(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let first = args.first().and_then(PyStr::to_utf8);
    match (first.as_deref(), args.len()) {
        (Some("self-test"), 1) => {
            let report = run_self_test();
            if let Err(failure) = io.print_lines(&PyStr::new(&report.stdout)) {
                return print_failed(io, &failure, "");
            }
            write_stderr(io, &PyStr::new(&report.stderr));
            report.exit_code
        }
        (Some("native-help"), 1) => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
        _ => run_legacy(args, io),
    }
}

fn run_legacy(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match argv::parse_with(&PARSE_BENCH, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            io.print_help(PARSE_BENCH.help(terminal::columns()));
            return 0;
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{}: error: ",
                PARSE_BENCH.usage(terminal::columns()),
                PARSE_BENCH.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
    };
    let bench_path = matches
        .value(BENCH_JSON)
        .cloned()
        .expect("argparse enforces --bench-json");
    // `if args.engine_log:` - an empty path means "not given".
    let engine_path = matches
        .value(ENGINE_LOG)
        .filter(|path| !path.is_empty())
        .cloned();

    let bytes = match read_path(&bench_path) {
        Ok(bytes) => bytes,
        Err(message) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[parse_bench] ERROR bench-json: {message}\n")),
            );
            return 2;
        }
    };
    let finished = on_large_stack(move || process(&bytes, engine_path));
    if let Some(output) = &finished.stdout {
        if let Err(failure) = io.print_body(output) {
            return print_failed(io, &failure, &finished.stderr);
        }
    }
    write_stderr(io, &PyStr::new(&finished.stderr));
    finished.exit_code
}

fn bench_json_error(message: &str) -> Finished {
    Finished {
        exit_code: 2,
        stdout: None,
        stderr: format!("[parse_bench] ERROR bench-json: {message}\n"),
    }
}

/// Decodes and loads the bench file, measures, and renders the result. Runs on the large-stack thread because the
/// document may be nested close to CPython's limit.
fn process(bytes: &[u8], engine_path: Option<PyStr>) -> Finished {
    let loaded = decode_file_text(bytes).and_then(|text| loads_text(&text));
    let document = match loaded {
        Ok(document) => document,
        // `except Exception` also catches the RecursionError of a too deeply nested document.
        Err(error @ LoadError::TooDeep { .. }) => {
            let text = error.to_string();
            return bench_json_error(text.strip_prefix("RecursionError: ").unwrap_or(&text));
        }
        Err(error) => return bench_json_error(&error.to_string()),
    };
    let source = engine_path.map(|path| move || read_path(&path));
    let outcome = match &source {
        Some(read) => measure(&document, Some(read)),
        None => measure(&document, None),
    };
    let mut stderr: String = outcome
        .warnings
        .iter()
        .map(|warning| format!("[parse_bench] WARN engine-log: {warning}\n"))
        .collect();
    let failure = |stderr: &mut String, text: String| {
        stderr.push_str(TRACEBACK_HEADER);
        stderr.push_str(&text);
        stderr.push('\n');
        Finished {
            exit_code: 1,
            stdout: None,
            stderr: std::mem::take(stderr),
        }
    };
    let measurement = match outcome.result {
        Ok(measurement) => measurement,
        Err(error) => return failure(&mut stderr, error.to_string()),
    };
    match measurement.to_value().dumps_pretty() {
        Ok(mut json) => {
            json.push_str("\n");
            Finished {
                exit_code: 0,
                stdout: Some(json),
                stderr,
            }
        }
        Err(error) => failure(&mut stderr, error.to_string()),
    }
}
