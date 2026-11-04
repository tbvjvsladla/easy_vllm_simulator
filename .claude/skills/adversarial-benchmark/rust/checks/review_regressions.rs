//! Regressions for the independent review: argument semantics, stream byte handling, surrogate separators and
//! nesting depth. Every golden record is the exact output of the original CLI on this interpreter
//! (`sweep_stop.py` run by CPython 3.12.3, stdin/stdout `errors=surrogateescape`), captured by
//! the dev-time generator `gen_review_goldens.py`; the binary under test must reproduce rc, stdout and stderr bytes.
//! Where Python died with a traceback (rc 1) only its last line is compared.

#[path = "common.rs"]
mod common;

use common::{hex_decode, load_jsonl, text_of};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads_text;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;

/// Two independent 64-bit checksums of very large stdout goldens (stored instead of the bytes).
fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xCBF2_9CE4_8422_2325, |h, &byte| {
        (h ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

fn djb2(data: &[u8]) -> u64 {
    data.iter().fold(5381, |h: u64, &byte| {
        h.wrapping_mul(33).wrapping_add(u64::from(byte))
    })
}

fn last_line(bytes: &[u8]) -> Vec<u8> {
    let trimmed = bytes.trim_ascii_end();
    trimmed
        .rsplit(|&b| b == b'\n')
        .next()
        .unwrap_or_default()
        .to_vec()
}

#[test]
fn cli_matches_the_original_byte_for_byte() {
    let records = load_jsonl("cli_review_goldens.jsonl");
    assert!(records.len() >= 100, "golden set shrank: {}", records.len());
    let scratch = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("review_regressions");
    std::fs::create_dir_all(&scratch).unwrap();
    let state_path = scratch.join("state.json");
    let placeholder = b"@FILE@";
    let path_bytes = state_path.as_os_str().as_encoded_bytes().to_vec();
    let substitute = |bytes: &[u8]| -> Vec<u8> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at..].starts_with(placeholder) {
                out.extend_from_slice(&path_bytes);
                at += placeholder.len();
            } else {
                out.push(bytes[at]);
                at += 1;
            }
        }
        out
    };

    let mut failures = Vec::new();
    for record in &records {
        let name = text_of(record.get("name"));
        let args: Vec<OsString> = match record.get("args") {
            Some(PyValue::List(list)) => list
                .iter()
                .map(|a| OsString::from_vec(substitute(&hex_decode(&text_of(Some(a))))))
                .collect(),
            other => panic!("args must be a list, got {other:?}"),
        };
        let stdin = match record.get("stdin_repeat") {
            Some(PyValue::Dict(recipe)) => {
                let byte = hex_decode(&text_of(recipe.get("byte_hex")))[0];
                let count: usize = recipe
                    .get("count")
                    .and_then(PyValue::as_plain_int)
                    .unwrap()
                    .to_string()
                    .parse()
                    .unwrap();
                vec![byte; count]
            }
            _ => hex_decode(&text_of(record.get("stdin_hex"))),
        };
        let via_file = text_of(record.get("via")) == "file";
        let feed = if via_file {
            std::fs::write(&state_path, &stdin).unwrap();
            Vec::new()
        } else {
            stdin
        };
        let mut child = Command::new(env!("CARGO_BIN_EXE_sweep_stop"))
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        let mut input = child.stdin.take().unwrap();
        let writer = std::thread::spawn(move || {
            let _ = input.write_all(&feed);
        });
        let output = child.wait_with_output().expect("wait");
        writer.join().unwrap();

        let want_rc: i32 = record
            .get("rc")
            .and_then(PyValue::as_plain_int)
            .unwrap()
            .to_string()
            .parse()
            .unwrap();
        let want_stdout = substitute(&hex_decode(&text_of(record.get("stdout_hex"))));
        let stdout_ok = match record.get("stdout_len") {
            Some(len) => {
                let number = |key: &str| {
                    record
                        .get(key)
                        .and_then(PyValue::as_plain_int)
                        .unwrap()
                        .to_string()
                };
                output.stdout.len().to_string() == len.repr()
                    && fnv1a(&output.stdout).to_string() == number("stdout_fnv1a")
                    && djb2(&output.stdout).to_string() == number("stdout_djb2")
            }
            None => output.stdout == want_stdout,
        };
        let want_stderr = substitute(&hex_decode(&text_of(record.get("stderr_hex"))));
        let stderr_ok = if text_of(record.get("stderr_mode")) == "last_line" {
            last_line(&output.stderr) == last_line(&want_stderr)
        } else {
            output.stderr == want_stderr
        };
        if output.status.code() != Some(want_rc) || !stdout_ok || !stderr_ok {
            failures.push(format!(
                "{name}: rc {:?} (want {want_rc}); stdout equal={}; stderr got {:?} want {:?}",
                output.status.code(),
                stdout_ok,
                String::from_utf8_lossy(&last_line(&output.stderr)),
                String::from_utf8_lossy(&last_line(&want_stderr)),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        records.len(),
        failures.join("\n")
    );
}

/// Runs `work` on a thread with a large stack, as `cli::run` does for deep documents.
fn on_large_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn parser_depth_limit_is_cpythons_for_arrays_objects_and_mixtures() {
    let outcome = |text: String| {
        on_large_stack(move || match loads_text(&PyStr::new(&text)) {
            Ok(_) => "ok".to_string(),
            Err(error) => error.to_string(),
        })
    };
    let array = |n: usize| format!("{}1{}", "[".repeat(n), "]".repeat(n));
    let object = |n: usize| format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n));
    assert_eq!(outcome(array(9997)), "ok");
    assert_eq!(outcome(object(9997)), "ok");
    assert_eq!(
        outcome(array(9998)),
        "RecursionError: maximum recursion depth exceeded while decoding a JSON array from a unicode string"
    );
    assert_eq!(
        outcome(object(9998)),
        "RecursionError: maximum recursion depth exceeded while decoding a JSON object from a unicode string"
    );
    let mixed = |pairs: usize, leaf: &str| {
        format!("{}{leaf}{}", "[{\"a\":".repeat(pairs), "}]".repeat(pairs))
    };
    assert_eq!(outcome(mixed(4998, "1")), "ok");
    assert!(
        outcome(mixed(4999, "1")).contains("while decoding a JSON object"),
        "levels alternate array, object, ...: the 9998th is an object"
    );
    // Invalid input far beyond the limit fails after the limit, without recursing further.
    assert!(outcome("[".repeat(3_000_000)).starts_with("RecursionError"));
}

#[test]
fn a_separator_surrogate_is_accepted_only_at_the_separator_positions() {
    use sweep_stop::parse_utc;
    let parse = |points: &[u32]| {
        parse_utc(
            &PyValue::Str(PyStr::from_code_points(points.to_vec())),
            "started_utc",
        )
    };
    let with = |before: &str, surrogate: u32, after: &str| {
        let mut points: Vec<u32> = before.chars().map(u32::from).collect();
        points.push(surrogate);
        points.extend(after.chars().map(u32::from));
        points
    };
    for (before, after) in [
        ("2026-09-04", "09:00:00+00:00"),
        ("20260904", "09:00:00+00:00"),
        ("2026-W36", "09:00:00+00:00"),
        ("2026W36", "09:00+00:00"),
        ("2026W365", "09:00:00+00:00"),
        ("2026-W36-5", "09:00:00+00:00"),
    ] {
        for surrogate in [0xD800, 0xDBFF, 0xDC00, 0xDFFF] {
            assert!(
                parse(&with(before, surrogate, after)).is_ok(),
                "{before} U+{surrogate:04X} {after}"
            );
        }
    }
    let rejected = parse(&with("2026-09-04T09:00:", 0xD800, "00+00:00"))
        .unwrap_err()
        .message();
    assert!(
        rejected.contains("Invalid isoformat string: '2026-09-04T09:00:\\ud80000+00:00'"),
        "{rejected}"
    );
}
