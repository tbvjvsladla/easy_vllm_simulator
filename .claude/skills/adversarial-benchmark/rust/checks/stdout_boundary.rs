//! What happens when the program's stdout cannot be written, for both executables.
//!
//! `data/parse_bench/write_boundary_baseline.jsonl` holds what the ORIGINAL Python programs did (exit status and
//! stderr, tracebacks reduced to the compared parts) with stdout sent to `/dev/full`, to a pipe whose reader is
//! gone, to a file, and to a pseudo-terminal whose master is gone. The rule being reproduced (see `src/stdout.rs`):
//! output of up to 8190 bytes is still buffered when the program ends, so the failure surfaces at exit as
//! `Exception ignored in: <_io.TextIOWrapper ...>` with status 120 replacing the program's own; from 8191 bytes the
//! `print` itself fails with a traceback and status 1.
//!
//! One band of the original is intentionally not reproduced: output larger than the stream buffer but not yet
//! 8191 bytes is silently dropped by the interpreter's end-of-script flush, which clears the error, and the
//! original then exits with its normal status. Those rows are asserted against the baseline (status 0 / the
//! verdict's own status, nothing written) AND against the documented behaviour here (status 120).

#[path = "common.rs"]
mod common;

use common::{command, load_json_file, load_jsonl, open_pty, text_of};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pyvalue::{PyDict, PyValue};

const PARSE_BENCH: &str = env!("CARGO_BIN_EXE_parse_bench");
const SWEEP_STOP: &str = env!("CARGO_BIN_EXE_sweep_stop");
const TRACEBACK: &str = "Traceback (most recent call last):\n";
/// Text-layer chunk: this many pending bytes make the `print` hand its data over.
const IMMEDIATE_FROM_BODY: usize = 8191;
const PTY_BUFFER: usize = 1024;
const DEFAULT_BUFFER: usize = 4096;

struct Golden {
    label: String,
    program: String,
    redirect: String,
    exit_code: i32,
    stderr: String,
    body_bytes: Option<usize>,
    state_case: Option<String>,
    warn: bool,
}

fn goldens() -> Vec<Golden> {
    load_jsonl("parse_bench/write_boundary_baseline.jsonl")
        .into_iter()
        .map(|record: PyDict| Golden {
            label: text_of(record.get("label")),
            program: text_of(record.get("program")),
            redirect: text_of(record.get("redirect")),
            exit_code: int_of(record.get("exit_code")).unwrap() as i32,
            stderr: text_of(record.get("stderr")),
            body_bytes: int_of(record.get("body_bytes")),
            state_case: record.get("state_case").map(|v| text_of(Some(v))),
            warn: matches!(record.get("warn"), Some(PyValue::Bool(true))),
        })
        .collect()
}

fn int_of(value: Option<&PyValue>) -> Option<usize> {
    value
        .and_then(PyValue::as_plain_int)
        .map(|n| n.to_string().parse().unwrap())
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("stdout_boundary")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// The stream the child gets as stdout, plus anything that must outlive the child.
enum Sink {
    DevFull,
    GoneReader,
    GonePtyMaster,
    File(PathBuf),
}

impl Sink {
    fn named(redirect: &str, dir: &Path) -> Option<Sink> {
        match redirect {
            "devfull" => Some(Sink::DevFull),
            "epipe" => Some(Sink::GoneReader),
            "pty-gone" => Some(Sink::GonePtyMaster),
            "file" => Some(Sink::File(dir.join("stdout.out"))),
            _ => None,
        }
    }

    fn stdio(&self) -> Stdio {
        match self {
            Sink::DevFull => Stdio::from(
                OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .expect("/dev/full"),
            ),
            Sink::GoneReader => {
                let (reader, writer) = std::io::pipe().expect("pipe");
                drop(reader);
                Stdio::from(writer)
            }
            Sink::GonePtyMaster => {
                let pty = open_pty(24, 80);
                drop(pty.master);
                Stdio::from(pty.slave)
            }
            Sink::File(path) => Stdio::from(File::create(path).expect("output file")),
        }
    }

    /// The `BufferedWriter` size the interpreter gets for this sink.
    fn buffer(&self) -> usize {
        match self {
            Sink::GonePtyMaster => PTY_BUFFER,
            _ => DEFAULT_BUFFER,
        }
    }
}

struct Seen {
    exit_code: i32,
    stderr: String,
}

fn run(bin: &str, args: &[String], sink: &Sink) -> Seen {
    let output = command(bin, None, args)
        .stdin(Stdio::null())
        .stdout(sink.stdio())
        .stderr(Stdio::piped())
        .output()
        .expect("run");
    Seen {
        exit_code: output
            .status
            .code()
            .expect("exited normally (no signal, no panic)"),
        stderr: String::from_utf8(output.stderr).expect("UTF-8 stderr"),
    }
}

/// Head before the traceback, the marker, and the final line: what the compatibility contract compares.
fn project(stderr: &str) -> String {
    match stderr.split_once(TRACEBACK) {
        None => stderr.to_string(),
        Some((head, tail)) => {
            let last = tail.trim_end_matches('\n').rsplit('\n').next().unwrap();
            format!("{head}{TRACEBACK}{last}\n")
        }
    }
}

fn bench_file(dir: &Path, body: usize) -> PathBuf {
    // Same document the baseline used; its output body is 436 bytes plus the length of `num_prompts`.
    let path = dir.join(format!("b{body}.json"));
    let text = format!(
        "{{\"median_tpot_ms\": 20.0, \"completed\": 4, \"num_prompts\": \"{}\"}}",
        "a".repeat(body - 436)
    );
    std::fs::write(&path, text).expect("bench file");
    path
}

fn pipe_output(bin: &str, args: &[String]) -> Vec<u8> {
    let output = command(bin, None, args)
        .stdin(Stdio::null())
        .output()
        .expect("run");
    assert!(
        output.status.code().is_some_and(|c| c == 0 || c == 3),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn sweep_states() -> Vec<(String, PathBuf, String)> {
    let reference = common::list(load_json_file(&common::data_path("reference/cases.json")));
    let cli = common::list(load_json_file(&common::data_path(
        "reference/cli-reference.json",
    )));
    let field = |value: &PyValue, key: &str| match value {
        PyValue::Dict(map) => map.get(key).cloned().unwrap(),
        other => panic!("not an object: {other:?}"),
    };
    let find = |id: &str| {
        reference
            .iter()
            .find(|case| text_of(Some(&field(case, "id"))) == id)
            .unwrap_or_else(|| panic!("no reference case {id}"))
            .clone()
    };
    let stop_id = cli
        .iter()
        .find(|c| {
            field(c, "exit_code")
                .as_plain_int()
                .is_some_and(|n| n.to_string() == "3")
        })
        .map(|c| text_of(Some(&field(c, "case"))))
        .expect("a stopping case");
    let dir = scratch("states");
    let mut out = Vec::new();
    for (name, case) in [
        ("continue", find("continue-empty")),
        ("stop", find(&stop_id)),
        ("big-continue", find("continue-empty")),
    ] {
        let mut state = match field(&case, "state") {
            PyValue::Dict(map) => map,
            other => panic!("state is not an object: {other:?}"),
        };
        if name == "big-continue" {
            let names = (0..1000).map(|n| PyValue::str(&format!("c{n}"))).collect();
            state.set("cells_remaining", PyValue::List(names));
        }
        let path = dir.join(format!("state-{name}.json"));
        std::fs::write(&path, PyValue::Dict(state).dumps_ascii().unwrap()).expect("state file");
        out.push((
            name.to_string(),
            path,
            text_of(Some(&field(&case, "now_utc"))),
        ));
    }
    out
}

/// Expected behaviour of this implementation for a program whose output body has `body` bytes.
fn expected_here(body: usize, sink_text: &str, own_status: i32) -> (i32, String) {
    if body >= IMMEDIATE_FROM_BODY {
        (1, format!("{TRACEBACK}{sink_text}\n"))
    } else {
        let _ = own_status;
        (
            120,
            format!("Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\n{sink_text}\n"),
        )
    }
}

fn sink_text(redirect: &str) -> &'static str {
    match redirect {
        "devfull" => "OSError: [Errno 28] No space left on device",
        "epipe" => "BrokenPipeError: [Errno 32] Broken pipe",
        "pty-gone" => "OSError: [Errno 5] Input/output error",
        other => panic!("not a failing sink: {other}"),
    }
}

/// True when the original drops the data and exits normally: more than the buffer, less than 8191 bytes.
fn original_swallows(body: usize, sink: &Sink) -> bool {
    body + 1 > sink.buffer() && body < IMMEDIATE_FROM_BODY
}

#[test]
fn parse_bench_reports_a_failed_stdout_like_the_original() {
    let dir = scratch("parse_bench");
    let mut checked = 0;
    let mut diverged = 0;
    for golden in goldens()
        .iter()
        .filter(|g| g.program == "parse_bench" && g.body_bytes.is_some())
    {
        let Some(sink) = Sink::named(&golden.redirect, &dir) else {
            continue;
        };
        if matches!(sink, Sink::File(_)) {
            continue;
        }
        let body = golden.body_bytes.unwrap();
        let mut args = vec![
            "--bench-json".to_string(),
            bench_file(&dir, body).to_str().unwrap().to_string(),
        ];
        let missing = dir.join("no-such-engine.log");
        if golden.warn {
            args.extend([
                "--engine-log".to_string(),
                missing.to_str().unwrap().to_string(),
            ]);
        }
        let seen = run(PARSE_BENCH, &args, &sink);
        let (want_exit, want_tail) = expected_here(body, sink_text(&golden.redirect), 0);
        let warning = if golden.warn {
            format!(
                "[parse_bench] WARN engine-log: [Errno 2] No such file or directory: '{}'\n",
                missing.display()
            )
        } else {
            String::new()
        };
        let mut want_stderr = format!("{warning}{want_tail}");
        if want_exit == 1 {
            want_stderr = format!("{warning}{TRACEBACK}{}\n", sink_text(&golden.redirect));
        }
        assert_eq!(
            (seen.exit_code, project(&seen.stderr)),
            (want_exit, want_stderr),
            "{}",
            golden.label
        );
        if original_swallows(body, &sink) {
            diverged += 1;
            assert_eq!(
                golden.exit_code, 0,
                "{}: the original swallows this band",
                golden.label
            );
            assert_eq!(golden.stderr, "", "{}", golden.label);
        } else {
            // Everywhere else the original and this program agree on status and on the compared stderr.
            let want = golden.stderr.replace("{DIR}", dir.to_str().unwrap());
            assert_eq!(
                (seen.exit_code, project(&seen.stderr)),
                (golden.exit_code, want),
                "{}",
                golden.label
            );
        }
        checked += 1;
    }
    assert!(checked >= 20, "boundary set shrank: {checked}");
    assert!(
        diverged >= 4,
        "the swallowed band is no longer exercised: {diverged}"
    );
}

#[test]
fn sweep_stop_reports_a_failed_stdout_like_the_original() {
    let dir = scratch("sweep_stop");
    let states = sweep_states();
    let mut checked = 0;
    for golden in goldens().iter().filter(|g| g.state_case.is_some()) {
        let Some(sink) = Sink::named(&golden.redirect, &dir) else {
            continue;
        };
        let (_, path, now) = states
            .iter()
            .find(|(name, ..)| Some(name) == golden.state_case.as_ref())
            .unwrap();
        let args = vec![
            "--state".to_string(),
            path.to_str().unwrap().into(),
            "--now-utc".into(),
            now.clone(),
        ];
        let pipe = pipe_output(SWEEP_STOP, &args);
        let body = pipe.len() - 1;
        if let Sink::File(file) = &sink {
            let seen = run(SWEEP_STOP, &args, &sink);
            assert_eq!(seen.exit_code, golden.exit_code, "{}", golden.label);
            assert_eq!(
                std::fs::read(file).unwrap(),
                pipe,
                "{}: output bytes",
                golden.label
            );
            checked += 1;
            continue;
        }
        let seen = run(SWEEP_STOP, &args, &sink);
        let (want_exit, want_tail) = expected_here(body, sink_text(&golden.redirect), 0);
        let want_stderr = if want_exit == 1 {
            format!("{TRACEBACK}{}\n", sink_text(&golden.redirect))
        } else {
            want_tail
        };
        assert_eq!(
            (seen.exit_code, project(&seen.stderr)),
            (want_exit, want_stderr),
            "{}",
            golden.label
        );
        assert_eq!(
            (golden.exit_code, golden.stderr.as_str()),
            (want_exit, project(&seen.stderr).as_str()),
            "{}: the original agrees",
            golden.label
        );
        checked += 1;
    }
    assert!(checked >= 9, "boundary set shrank: {checked}");
}

#[test]
fn self_test_and_help_fail_at_exit_for_both_programs() {
    let dir = scratch("selftests");
    for (bin, args, label) in [
        (SWEEP_STOP, vec!["--self-test"], "ss-selftest"),
        (SWEEP_STOP, vec!["-h"], "ss-help"),
        (SWEEP_STOP, vec!["native-help"], "ss-native-help"),
        (PARSE_BENCH, vec!["self-test"], "pb-selftest"),
        (PARSE_BENCH, vec!["-h"], "pb-help"),
        (PARSE_BENCH, vec!["native-help"], "pb-native-help"),
    ] {
        for redirect in ["devfull", "epipe"] {
            let sink = Sink::named(redirect, &dir).unwrap();
            let seen = run(
                bin,
                &args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
                &sink,
            );
            let (_, want) = expected_here(0, sink_text(redirect), 0);
            assert_eq!(
                (seen.exit_code, seen.stderr.as_str()),
                (120, want.as_str()),
                "{label} {redirect}"
            );
        }
    }
    // The original's rows for the same invocations, where it has them.
    for golden in goldens().iter().filter(|g| {
        matches!(
            g.label.as_str(),
            "ss-selftest-devfull" | "ss-selftest-epipe" | "ss-help-devfull" | "pb-help-devfull"
        )
    }) {
        assert_eq!(golden.exit_code, 120, "{}", golden.label);
        assert!(golden.stderr.starts_with("Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\n"), "{}", golden.label);
    }
}

#[test]
fn a_failed_flush_never_turns_a_verdict_into_a_normal_status() {
    let dir = scratch("verdict");
    for (name, path, now) in sweep_states().iter().take(2) {
        let args = vec![
            "--state".to_string(),
            path.to_str().unwrap().into(),
            "--now-utc".into(),
            now.clone(),
        ];
        let normal = run(
            SWEEP_STOP,
            &args,
            &Sink::File(dir.join(format!("{name}.out"))),
        )
        .exit_code;
        assert!(normal == 0 || normal == 3, "{name}");
        for redirect in ["devfull", "epipe"] {
            let seen = run(SWEEP_STOP, &args, &Sink::named(redirect, &dir).unwrap());
            assert_eq!(
                seen.exit_code, 120,
                "{name} {redirect}: status {normal} must not survive a lost output"
            );
        }
    }
}

#[test]
fn successful_output_to_a_file_is_complete_and_unchanged() {
    let dir = scratch("success");
    for body in [436usize, 8190, 8191, 10456] {
        let args = vec![
            "--bench-json".to_string(),
            bench_file(&dir, body).to_str().unwrap().into(),
        ];
        let pipe = pipe_output(PARSE_BENCH, &args);
        assert_eq!(pipe.len() - 1, body);
        let sink = Sink::File(dir.join(format!("out{body}.txt")));
        let seen = run(PARSE_BENCH, &args, &sink);
        assert_eq!(
            (seen.exit_code, seen.stderr.as_str()),
            (0, ""),
            "body {body}"
        );
        if let Sink::File(path) = &sink {
            assert_eq!(std::fs::read(path).unwrap(), pipe, "body {body}");
        }
    }
}

#[test]
fn a_closed_stdout_descriptor_is_not_an_error() {
    // Rust's runtime reopens a closed descriptor 1 as /dev/null before main, so the program sees a writable
    // stream: output is discarded and the status is the program's own (as with the original, where stdout is None).
    let dir = scratch("closed");
    let args = vec![
        "--bench-json".to_string(),
        bench_file(&dir, 10456).to_str().unwrap().into(),
    ];
    let status = Command::new("sh")
        .arg("-c")
        .arg("exec \"$0\" \"$@\" >&-")
        .arg(PARSE_BENCH)
        .args(&args)
        .stdin(Stdio::null())
        .status()
        .expect("sh");
    assert_eq!(status.code(), Some(0));
}
