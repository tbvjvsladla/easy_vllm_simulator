//! The `parse_bench` binary against the ORIGINAL program's captured behaviour.
//!
//! `data/parse_bench/cases.jsonl` holds 624 cases: the 221 of the externally captured, immutable root reference
//! (`source: root-reference-v1`) and 403 more counterexamples captured from the original by the worker
//! (`source: worker-extra`) where the root cases do not pin interpreter behaviour. Each case recreates its input
//! files in a package-local scratch directory, runs the binary with the recorded argv and compares exit status,
//! stdout bytes and stderr. Uncaught Python exceptions end in a traceback whose frames are source-language
//! specific: for those, stderr must match before the `Traceback` line and in the final `Class: message` line.
//! Two worker-extra cases exercise the Rust-only words (`self-test`, `native-help`) the original rejects; their
//! expected values are the original's, so they are listed as accepted divergences and asserted separately.

#[path = "common.rs"]
mod common;

use common::{command, hex_decode, load_jsonl, text_of};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;

const BIN: &str = env!("CARGO_BIN_EXE_parse_bench");
const TRACEBACK: &str = "Traceback (most recent call last):\n";
const RUST_ONLY: [&str; 2] = ["x0400-cli-native-word", "x0402-cli-native-help"];

struct Case {
    id: String,
    source: String,
    files: BTreeMap<String, Vec<u8>>,
    argv: Vec<String>,
    exit_code: i32,
    stdout: PyStr,
    stderr: PyStr,
}

/// The recorded text as the original produced it: stdout may carry bytes that Python decoded with
/// `surrogateescape`, which only a code-point string can hold.
fn py_text(value: Option<&PyValue>) -> PyStr {
    match value {
        Some(PyValue::Str(text)) => text.clone(),
        other => panic!("expected a string field, got {other:?}"),
    }
}

/// Replaces every `{DIR}` placeholder by the scratch directory.
fn expand(text: &PyStr, dir: &str) -> PyStr {
    let placeholder: Vec<u32> = "{DIR}".chars().map(u32::from).collect();
    let points = text.code_points();
    let mut out = Vec::with_capacity(points.len());
    let mut at = 0;
    while at < points.len() {
        if points[at..].starts_with(&placeholder) {
            out.extend(dir.chars().map(u32::from));
            at += placeholder.len();
        } else {
            out.push(points[at]);
            at += 1;
        }
    }
    PyStr::from_code_points(out)
}

fn cases() -> Vec<Case> {
    load_jsonl("parse_bench/cases.jsonl")
        .into_iter()
        .map(|record| {
            let files = match record.get("files") {
                Some(PyValue::Dict(map)) => map
                    .iter()
                    .map(|(name, hex)| (name.to_utf8().unwrap(), hex_decode(&text_of(Some(hex)))))
                    .collect(),
                other => panic!("files must be an object, got {other:?}"),
            };
            let argv = match record.get("argv") {
                Some(PyValue::List(list)) => list.iter().map(|a| text_of(Some(a))).collect(),
                other => panic!("argv must be a list, got {other:?}"),
            };
            Case {
                id: text_of(record.get("id")),
                source: text_of(record.get("source")),
                files,
                argv,
                exit_code: record
                    .get("exit_code")
                    .and_then(PyValue::as_plain_int)
                    .unwrap()
                    .to_string()
                    .parse()
                    .unwrap(),
                stdout: py_text(record.get("stdout")),
                stderr: py_text(record.get("stderr")),
            }
        })
        .collect()
}

fn scratch_dir(id: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("parse_bench_cases")
        .join(id);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// Text before the traceback and the final line of it (`None` when the program did not die on an exception).
fn split_traceback(stderr: &str) -> (String, Option<String>) {
    match stderr.split_once(TRACEBACK) {
        Some((head, tail)) => (
            head.to_string(),
            tail.trim_end_matches('\n')
                .rsplit('\n')
                .next()
                .map(str::to_string),
        ),
        None => (stderr.to_string(), None),
    }
}

struct Observed {
    exit_code: Option<i32>,
    stdout: PyStr,
    stderr: PyStr,
    scratch: String,
}

fn run_case(case: &Case) -> Observed {
    let dir = scratch_dir(&case.id);
    for (name, bytes) in &case.files {
        std::fs::write(dir.join(name), bytes).expect("write input");
    }
    let dir_text = dir.to_str().expect("UTF-8 scratch path").to_string();
    let argv: Vec<String> = case
        .argv
        .iter()
        .map(|a| a.replace("{DIR}", &dir_text))
        .collect();
    let output = command(BIN, None, &argv)
        .stdin(Stdio::null())
        .output()
        .expect("run parse_bench");
    let _ = std::fs::remove_dir_all(&dir);
    Observed {
        exit_code: output.status.code(),
        stdout: PyStr::from_surrogateescape(&output.stdout),
        stderr: PyStr::from_surrogateescape(&output.stderr),
        scratch: dir_text,
    }
}

fn differences(case: &Case, seen: &Observed) -> Vec<&'static str> {
    let mut problems = Vec::new();
    if seen.exit_code != Some(case.exit_code) {
        problems.push("exit_code");
    }
    if seen.stdout != expand(&case.stdout, &seen.scratch) {
        problems.push("stdout");
    }
    let (want, got) = (expand(&case.stderr, &seen.scratch), &seen.stderr);
    // Tracebacks carry source-language frames; everything else must be identical.
    let comparable =
        |text: &PyStr| split_traceback(&String::from_utf8_lossy(&text.to_backslashreplace_bytes()));
    if comparable(&want) != comparable(got) {
        problems.push("stderr");
    }
    problems
}

#[test]
fn every_captured_case_matches_the_original() {
    let all = cases();
    assert_eq!(
        all.iter()
            .filter(|c| c.source == "root-reference-v1")
            .count(),
        221,
        "the root reference shrank"
    );
    assert!(all.len() >= 624, "case set shrank: {}", all.len());
    let mut failures = Vec::new();
    let mut compared = 0;
    for case in &all {
        if RUST_ONLY.contains(&case.id.as_str()) {
            continue;
        }
        compared += 1;
        let problems = differences(case, &run_case(case));
        if !problems.is_empty() {
            failures.push(format!(
                "{} ({}): {}",
                case.id,
                case.source,
                problems.join(", ")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {compared} cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn exit_status_mix_of_the_root_reference_is_as_captured() {
    let mut by_exit: BTreeMap<i32, usize> = BTreeMap::new();
    for case in cases().iter().filter(|c| c.source == "root-reference-v1") {
        *by_exit.entry(case.exit_code).or_default() += 1;
    }
    assert_eq!(by_exit, BTreeMap::from([(0, 197), (1, 17), (2, 7)]));
}

#[test]
fn rust_only_words_are_the_documented_divergence() {
    for case in cases()
        .iter()
        .filter(|c| RUST_ONLY.contains(&c.id.as_str()))
    {
        let seen = run_case(case);
        let original_stderr = case.stderr.to_utf8().unwrap();
        assert_eq!(
            case.exit_code, 2,
            "the original rejects the positional word"
        );
        assert!(original_stderr.contains("the following arguments are required: --bench-json"));
        assert_eq!(seen.exit_code, Some(0), "{}", case.id);
        assert!(seen.stderr.is_empty(), "{}", case.id);
        let first_line = if case.argv == ["self-test"] {
            "[parse_bench self-test] PASS"
        } else {
            "parse_bench Rust-only commands"
        };
        let stdout = seen.stdout.to_utf8().unwrap();
        assert!(stdout.starts_with(first_line), "{}: {stdout}", case.id);
    }
}

#[test]
fn self_test_identifies_itself_and_is_not_a_measurement() {
    let output = command(BIN, None, &["self-test".to_string()])
        .stdin(Stdio::null())
        .output()
        .expect("run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{stdout}");
    assert!(
        stdout
            .lines()
            .all(|line| line.starts_with("[parse_bench self-test]")),
        "{stdout}"
    );
    assert!(
        stdout.trim_end().ends_with("서빙 측정 증거가 아니다)"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"decode_tps\"") && !stdout.lines().any(|line| line.starts_with('{')),
        "no measurement object is printed"
    );
}
