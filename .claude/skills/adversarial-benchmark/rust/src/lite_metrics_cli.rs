//! Command line of `lite_metrics`: only the original program's argparse interface ([`crate::argv::LITE_METRICS`]):
//! `--raw-json PATH` (required), `--json`, `--judge`, `-h`. Usage, help and argument-error text is byte-for-byte the
//! original's, including the logical program name `lite_metrics.py`. Nothing Rust-only is added here, so an unknown
//! word is the original's `unrecognized arguments` error; the Rust-only surface is the separate `lite_metrics_api`.
//!
//! Failure timing follows the original:
//!
//! * an unreadable or non-JSON raw reading is `[lite_metrics] raw JSON 로드 실패: PATH` on stderr, status 2;
//! * `build` raising `SystemExit(message)` prints the message and ends with status 1; any other exception ends with
//!   a traceback (of which only the final `Class: message` line is reproduced) and status 1;
//! * `--judge` writes the verdict into the raw reading with `open(PATH + ".tmp", "w")`, `json.dump(..., indent=1)` and
//!   `os.replace` — in that order and incrementally, see [`rewrite_raw_reading`] — and prints the table, a blank
//!   line, the verdict line and one line per reason; the status stays 0 whatever the verdict (the caller derives
//!   the exit code from the stored verdict).

use crate::argv::{self, lite_args, Outcome, LITE_METRICS};
use crate::cli::{
    on_large_stack, os_error_text, print_failed, write_stderr, Streams, TRACEBACK_HEADER,
};
use crate::guidellm::Failure;
use crate::lite_metrics::{build, judge_lite, load_json};
use crate::pyfs::{os_error_class, os_string};
use crate::pystr::{EncodeFailure, PyStr};
use crate::pyvalue::PyValue;
use crate::terminal;
use std::io::Write;
use std::path::Path;

/// What the heavy part of a run hands back to the thread that owns the streams: the lines `print` is called with,
/// in order, then the stderr text and the status.
struct Finished {
    prints: Vec<PyStr>,
    stderr: PyStr,
    exit_code: i32,
}

impl Finished {
    fn fail(exit_code: i32, stderr: PyStr) -> Finished {
        Finished {
            prints: Vec::new(),
            stderr,
            exit_code,
        }
    }

    /// The program ends on an uncaught exception, `SystemExit(message)` apart.
    fn raised(failure: &Failure) -> Finished {
        if failure.class == "SystemExit" {
            let mut text = failure.message.clone();
            text.push_str("\n");
            return Finished::fail(1, text);
        }
        let mut text = PyStr::new(TRACEBACK_HEADER);
        text.push_py(&failure.line());
        text.push_str("\n");
        Finished::fail(1, text)
    }
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = run_legacy(args, io);
    io.finish(status)
}

fn run_legacy(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match argv::parse_with(&LITE_METRICS, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            io.print_help(LITE_METRICS.help(terminal::columns()));
            return 0;
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{}: error: ",
                LITE_METRICS.usage(terminal::columns()),
                LITE_METRICS.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
    };
    let raw_path = matches
        .value(lite_args::RAW_JSON)
        .cloned()
        .expect("argparse enforces --raw-json");
    let as_json = matches.flag(lite_args::JSON);
    let judge = matches.flag(lite_args::JUDGE);
    let finished = on_large_stack(move || process(&raw_path, as_json, judge));
    for line in &finished.prints {
        if let Err(failure) = io.print(line) {
            let so_far = finished.stderr.to_utf8().unwrap_or_default();
            return print_failed(io, &failure, &so_far);
        }
    }
    write_stderr(io, &finished.stderr);
    finished.exit_code
}

fn process(raw_path: &PyStr, as_json: bool, judge: bool) -> Finished {
    let mut raw = match load_json(&PyValue::Str(raw_path.clone())) {
        Ok(Some(raw)) => raw,
        Ok(None) => {
            let mut text = PyStr::new("[lite_metrics] raw JSON 로드 실패: ");
            text.push_py(raw_path);
            text.push_str("\n");
            return Finished::fail(2, text);
        }
        Err(failure) => return Finished::raised(&failure),
    };
    let metrics = match build(&raw) {
        Ok(metrics) => metrics,
        Err(failure) => return Finished::raised(&failure),
    };
    let ok = |prints: Vec<PyStr>| Finished {
        prints,
        stderr: PyStr::default(),
        exit_code: 0,
    };
    if judge {
        let judgement = match judge_lite(&raw, &metrics.to_value()) {
            Ok(judgement) => judgement,
            Err(failure) => return Finished::raised(&failure),
        };
        if let (PyValue::Dict(map), PyValue::Dict(verdict)) = (&mut raw, judgement.to_value()) {
            for (key, value) in verdict.iter() {
                map.insert(key.clone(), value.clone());
            }
        }
        if let Err(failure) = rewrite_raw_reading(raw_path, &raw) {
            return Finished::raised(&failure);
        }
        let mut prints = vec![metrics.table.clone(), PyStr::default()];
        prints.push(PyStr::new(&format!(
            "lite_verdict: {} (exit {})",
            judgement.verdict.label(),
            judgement.verdict.exit_code()
        )));
        for reason in &judgement.reasons {
            let mut line = PyStr::new("  - ");
            line.push_py(reason);
            prints.push(line);
        }
        return ok(prints);
    }
    if as_json {
        return match metrics.to_value().dumps_pretty() {
            Ok(text) => ok(vec![text]),
            Err(error) => Finished::raised(&Failure::new(error.python_type(), &error.to_string())),
        };
    }
    ok(vec![metrics.table])
}

fn os_failure(error: &std::io::Error, path: &PyStr) -> Failure {
    Failure::new(os_error_class(error), &os_error_text(error, path))
}

fn is_surrogate(code_point: u32) -> bool {
    (0xD800..=0xDFFF).contains(&code_point)
}

/// Where `json.dump` stops when the document holds a lone surrogate: every string (key or value) is one chunk
/// that `TextIOWrapper.write` encodes whole, so the file keeps everything before the opening quote of the first
/// string that cannot be encoded. Returns that offset and the failure (positions are within the chunk).
fn unencodable_chunk(points: &[u32]) -> Option<(usize, EncodeFailure)> {
    let surrogate = points.iter().position(|&point| is_surrogate(point))?;
    let quote = u32::from('"');
    let backslash = u32::from('\\');
    let chunk_start = (0..surrogate)
        .rev()
        .find(|&index| {
            points[index] == quote
                && points[..index]
                    .iter()
                    .rev()
                    .take_while(|&&point| point == backslash)
                    .count()
                    % 2
                    == 0
        })
        .unwrap_or(0);
    let run = points[surrogate..]
        .iter()
        .take_while(|&&point| is_surrogate(point))
        .count();
    Some((
        chunk_start,
        EncodeFailure {
            start: surrogate - chunk_start,
            end: surrogate - chunk_start + run,
            first: points[surrogate],
        },
    ))
}

/// ```python
/// tmp = args.raw_json + ".tmp"
/// with open(tmp, "w", encoding="utf-8") as f:
///     json.dump(raw, f, ensure_ascii=False, indent=1)
/// os.replace(tmp, args.raw_json)
/// ```
///
/// The file is opened (created or truncated, following a symlink) before anything is encoded; the document is
/// written chunk by chunk, so a string that cannot be encoded leaves the text before it in the temporary file and
/// no replacement happens. `os.replace` moves the temporary name itself (a symlink moves as a symlink).
fn rewrite_raw_reading(raw_path: &PyStr, raw: &PyValue) -> Result<(), Failure> {
    let mut temporary = raw_path.clone();
    temporary.push_str(".tmp");
    let to_failure = |error: crate::pyfs::FsFailure| error.into_failure();
    let temporary_os = os_string(&temporary).map_err(to_failure)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Path::new(&temporary_os))
        .map_err(|error| os_failure(&error, &temporary))?;
    let text = raw
        .dumps_indented(1)
        .map_err(|error| Failure::new(error.python_type(), &error.to_string()))?;
    let points = text.code_points();
    let (writable, encoding_failure) = match unencodable_chunk(points) {
        Some((chunk_start, failure)) => (&points[..chunk_start], Some(failure)),
        None => (points, None),
    };
    let bytes = PyStr::from_code_points(writable.to_vec())
        .to_utf8()
        .expect("text before the first surrogate is UTF-8");
    let written = file.write_all(bytes.as_bytes());
    drop(file);
    written.map_err(|error| os_failure(&error, &temporary))?;
    if let Some(failure) = encoding_failure {
        return Err(Failure::new("UnicodeEncodeError", &failure.message()));
    }
    let raw_os = os_string(raw_path).map_err(to_failure)?;
    std::fs::rename(Path::new(&temporary_os), Path::new(&raw_os)).map_err(|error| {
        let single = os_error_text(&error, &temporary);
        let mut message = single;
        message.push_str(" -> ");
        message.push_str(&raw_path.repr());
        Failure::new(os_error_class(&error), &message)
    })
}
