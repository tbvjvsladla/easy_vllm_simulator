//! The `parse_guidellm` binary against the ORIGINAL program's captured behaviour.
//!
//! * `guidellm/api.jsonl`: the 145 API observations (build 84 · inherit_accept_len 27 · classify_errors 34) of the
//!   frozen external reference, fed to `evaluate-jsonl` and compared as whole ordered records (an `int`/`float`/
//!   `bool` or key-order difference is a difference; NaN compares as its JSON spelling).
//! * `guidellm/cases.jsonl`: CLI 140, engine-log 13, malformed engine-log 3 and `spec-source` helper 18 cases of the
//!   frozen reference. Input files live in `guidellm/inputs/`; the `@DIR@` token (in argv, expected text and the
//!   JSON inputs that name other inputs) is the scratch directory the case runs in.
//! * `guidellm/depth_cases.jsonl`: 92 nesting-boundary cases for the four JSON read contexts (benchmarks, accept
//!   source, post-health, prefix cache); the input is a head text plus N nested arrays around a `0`, generated here.
//! * `guidellm/extra_cases.jsonl`: 379 counterexamples captured from the original by the worker where the frozen
//!   set does not pin interpreter behaviour (odd types, argparse forms, Unicode lower-casing, huge integers, ...).
//! * `guidellm/help_goldens.jsonl`: usage/help/argument errors at 22 `COLUMNS` values (including beyond the last
//!   captured step).
//!
//! Uncaught Python exceptions end in a traceback whose frames are source-language specific: stderr is compared
//! before the `Traceback` line and in the final `Class: message` line only (the packaged expectations already hold
//! exactly that projection). Nothing is skipped or loosened per case.

#[path = "common.rs"]
mod common;

use common::{command, data_path};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use sweep_stop::pyjson::loads;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{PyDict, PyValue};

const BIN: &str = env!("CARGO_BIN_EXE_parse_guidellm");
const TRACEBACK: &str = "Traceback (most recent call last):\n";

/// The owner's real GuideLLM report that G11/G29 read at run time: the explicit variable, else the owner layout
/// `<package>/../fixtures/` of the promoted crate. An absent oracle is NOT_RUN, never a pass.
fn fixture() -> PathBuf {
    let path = std::env::var_os("PARSE_GUIDELLM_FIXTURE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("fixtures")
                .join("guidellm_benchmarks_sample.json")
        });
    assert!(
        path.is_file(),
        "NOT_RUN: owner fixture missing ({}); set PARSE_GUIDELLM_FIXTURE or run inside the owner layout",
        path.display()
    );
    path
}

fn jsonl(name: &str) -> Vec<PyDict> {
    let text = std::fs::read_to_string(data_path(&format!("guidellm/{name}")))
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    text.lines()
        .map(|line| match loads(line).expect("data line is JSON") {
            PyValue::Dict(record) => record,
            other => panic!("not an object: {}", other.repr()),
        })
        .collect()
}

fn s(record: &PyDict, key: &str) -> PyStr {
    match record.get(key) {
        Some(PyValue::Str(text)) => text.clone(),
        other => panic!("{key}: expected a string, got {other:?}"),
    }
}

fn argv_of(record: &PyDict) -> Vec<PyStr> {
    match record.get("argv") {
        Some(PyValue::List(items)) => items
            .iter()
            .map(|item| match item {
                PyValue::Str(text) => text.clone(),
                other => panic!("argv item {other:?}"),
            })
            .collect(),
        other => panic!("argv: {other:?}"),
    }
}

fn exit_of(record: &PyDict) -> i32 {
    record
        .get("exit_code")
        .and_then(PyValue::as_plain_int)
        .and_then(|n| n.to_string().parse().ok())
        .expect("exit_code")
}

fn expand(text: &PyStr, dir: &str) -> PyStr {
    let token: Vec<u32> = "@DIR@".chars().map(u32::from).collect();
    let points = text.code_points();
    let mut out = Vec::with_capacity(points.len());
    let mut at = 0;
    while at < points.len() {
        if points[at..].starts_with(&token) {
            out.extend(dir.chars().map(u32::from));
            at += token.len();
        } else {
            out.push(points[at]);
            at += 1;
        }
    }
    PyStr::from_code_points(out)
}

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("guidellm_cases")
        .join(label);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// Copies the packaged inputs to `dst`, expanding `@DIR@` in text files (binary files stay as they are).
fn stage_inputs(dst: &Path) {
    fn walk(src: &Path, root: &Path, dst: &Path, dir_text: &str) {
        for entry in std::fs::read_dir(src).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, root, dst, dir_text);
                continue;
            }
            let target = dst.join(path.strip_prefix(root).expect("inside root"));
            std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
            let bytes = std::fs::read(&path).expect("read input");
            let bytes = match String::from_utf8(bytes.clone()) {
                Ok(text) => text.replace("@DIR@", dir_text).into_bytes(),
                Err(_) => bytes,
            };
            std::fs::write(target, bytes).expect("write input");
        }
    }
    let root = data_path("guidellm/inputs");
    walk(&root, &root, dst, dst.to_str().expect("UTF-8 scratch path"));
}

struct Seen {
    exit: Option<i32>,
    stdout: PyStr,
    stderr: PyStr,
}

fn run(dir: &Path, columns: Option<&str>, args: &[PyStr], envs: &[(&str, &Path)]) -> Seen {
    use std::os::unix::ffi::OsStringExt;
    let mut cmd = std::process::Command::new(BIN);
    cmd.current_dir(dir)
        .env_remove("COLUMNS")
        .env_remove("LINES")
        .env_remove("PARSE_GUIDELLM_FIXTURE");
    if let Some(columns) = columns {
        cmd.env("COLUMNS", columns);
    }
    for (key, value) in envs {
        cmd.env(key, value);
    }
    for arg in args {
        cmd.arg(std::ffi::OsString::from_vec(
            arg.to_surrogateescape_bytes().expect("argv is encodable"),
        ));
    }
    let output = cmd
        .stdin(Stdio::null())
        .output()
        .expect("run parse_guidellm");
    Seen {
        exit: output.status.code(),
        stdout: PyStr::from_surrogateescape(&output.stdout),
        stderr: PyStr::from_surrogateescape(&output.stderr),
    }
}

fn split_traceback(stderr: &PyStr) -> (String, Option<String>) {
    let text = String::from_utf8_lossy(&stderr.to_backslashreplace_bytes()).into_owned();
    match text.split_once(TRACEBACK) {
        Some((head, tail)) => (
            head.to_string(),
            tail.trim_end_matches('\n')
                .rsplit('\n')
                .next()
                .map(str::to_string),
        ),
        None => (text, None),
    }
}

fn differences(
    want_exit: i32,
    want_out: &PyStr,
    want_err: &PyStr,
    seen: &Seen,
) -> Vec<&'static str> {
    let mut problems = Vec::new();
    if seen.exit != Some(want_exit) {
        problems.push("exit_code");
    }
    if &seen.stdout != want_out {
        problems.push("stdout");
    }
    if split_traceback(want_err) != split_traceback(&seen.stderr) {
        problems.push("stderr");
    }
    problems
}

fn fail_if_any(label: &str, total: usize, failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} of {total} {label} cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn api_observations_match_as_whole_ordered_records() {
    let rows = jsonl("api.jsonl");
    assert_eq!(rows.len(), 145, "the frozen API set shrank");
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut input = String::new();
    let mut expected = Vec::new();
    for row in &rows {
        let operation = s(row, "operation").to_utf8().unwrap();
        *counts.entry(operation).or_default() += 1;
        let mut request = PyDict::new();
        for key in ["id", "operation", "arguments"] {
            request.set(key, row.get(key).cloned().expect("field"));
        }
        input.push_str(&PyValue::Dict(request).dumps_ascii().unwrap());
        input.push('\n');
        let mut want = PyDict::new();
        want.set("id", row.get("id").cloned().unwrap());
        let PyValue::Dict(outcome) = row.get("expected").cloned().expect("expected") else {
            panic!("expected is an object")
        };
        for (key, value) in outcome.iter() {
            want.set(&key.to_utf8().unwrap(), value.clone());
        }
        expected.push(PyValue::Dict(want).dumps_ascii().unwrap());
    }
    assert_eq!(
        counts,
        BTreeMap::from([
            ("build".to_string(), 84),
            ("classify_errors".to_string(), 34),
            ("inherit_accept_len".to_string(), 27)
        ])
    );
    let mut child = std::process::Command::new(BIN)
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual = String::from_utf8(output.stdout).unwrap();
    let actual: Vec<&str> = actual.lines().collect();
    assert_eq!(actual.len(), expected.len());
    let failures: Vec<String> = expected
        .iter()
        .zip(&actual)
        .enumerate()
        .filter(|(_, (want, got))| want != got)
        .map(|(at, (want, got))| format!("api-{at:04}\n  want {want}\n  got  {got}"))
        .collect();
    fail_if_any("API", expected.len(), failures);
}

#[test]
fn frozen_cli_engine_and_spec_source_cases_match() {
    let dir = scratch("frozen");
    stage_inputs(&dir);
    let dir_text = dir.to_str().unwrap().to_string();
    let cases = jsonl("cases.jsonl");
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for case in &cases {
        let group = s(case, "group").to_utf8().unwrap();
        *tally.entry(group.clone()).or_default() += 1;
        let argv: Vec<PyStr> = argv_of(case).iter().map(|a| expand(a, &dir_text)).collect();
        // The self-test reads the owner's real report at run time (the launcher supplies this variable).
        let sample = argv.iter().any(|a| a.equals("--self-test")).then(fixture);
        let envs: Vec<(&str, &Path)> = sample
            .iter()
            .map(|path| ("PARSE_GUIDELLM_FIXTURE", path.as_path()))
            .collect();
        let seen = run(&dir, None, &argv, &envs);
        let problems = differences(
            exit_of(case),
            &expand(&s(case, "stdout"), &dir_text),
            &expand(&s(case, "stderr"), &dir_text),
            &seen,
        );
        if !problems.is_empty() {
            failures.push(format!(
                "{group}/{}: {}",
                s(case, "id").to_utf8().unwrap(),
                problems.join(", ")
            ));
        }
    }
    assert_eq!(
        tally,
        BTreeMap::from([
            ("cli".to_string(), 140),
            ("engine".to_string(), 13),
            ("engine-malformed".to_string(), 3),
            ("spec-source".to_string(), 18)
        ])
    );
    fail_if_any("frozen", cases.len(), failures);
}

#[test]
fn self_test_is_the_original_transcript_for_the_real_fixture() {
    let sample = fixture();
    let dir = scratch("selftest");
    let seen = run(
        &dir,
        None,
        &[PyStr::new("--self-test")],
        &[("PARSE_GUIDELLM_FIXTURE", &sample)],
    );
    let original = std::fs::read(data_path("guidellm/original-selftest.stdout")).unwrap();
    assert_eq!(seen.exit, Some(0), "{:?}", seen.stderr.to_utf8());
    assert_eq!(seen.stdout, PyStr::from_surrogateescape(&original));
    assert!(seen.stderr.is_empty());
    assert!(
        seen.stdout.to_utf8().unwrap().contains("G1~G29 전부 통과"),
        "all 29 scenarios run"
    );
}

#[test]
fn a_missing_fixture_is_an_actual_failed_self_test() {
    let dir = scratch("selftest-missing");
    let absent = dir.join("nothing.json");
    let seen = run(
        &dir,
        None,
        &[PyStr::new("--self-test")],
        &[("PARSE_GUIDELLM_FIXTURE", &absent)],
    );
    let stdout = seen.stdout.to_utf8().unwrap();
    assert_eq!(seen.exit, Some(1));
    assert!(stdout.contains(
        "[FAIL] G11 실측 산출물 픽스처 존재 (fixtures/guidellm_benchmarks_sample.json 부재)"
    ));
    assert!(
        stdout.contains("[PASS] G14") && !stdout.contains("G15"),
        "the run stops at the first gate, as the original does"
    );
    assert_eq!(
        seen.stderr.to_utf8().unwrap(),
        "[parse_guidellm --self-test] FAIL 1 건: ['G11 실측 산출물 픽스처 존재']\n"
    );
}

#[test]
fn json_nesting_boundary_matches_in_all_four_read_contexts() {
    let dir = scratch("depth");
    stage_inputs(&dir);
    let depth_dir = dir.join("depth");
    std::fs::create_dir_all(&depth_dir).unwrap();
    let heads =
        match loads(&std::fs::read_to_string(data_path("guidellm/depth_heads.json")).unwrap())
            .unwrap()
        {
            PyValue::Dict(map) => map,
            other => panic!("{other:?}"),
        };
    let dir_text = dir.to_str().unwrap().to_string();
    let cases = jsonl("depth_cases.jsonl");
    assert_eq!(cases.len(), 92);
    let mut failures = Vec::new();
    let mut rejected = 0;
    for case in &cases {
        let kind = s(case, "kind").to_utf8().unwrap();
        let depth: usize = case
            .get("depth")
            .and_then(PyValue::as_plain_int)
            .unwrap()
            .to_string()
            .parse()
            .unwrap();
        let head = s(&heads, &kind).to_utf8().unwrap();
        let text = format!("{head}{}0{}}}", "[".repeat(depth), "]".repeat(depth));
        std::fs::write(depth_dir.join(format!("{kind}-{depth}.json")), text).unwrap();
        let argv: Vec<PyStr> = argv_of(case).iter().map(|a| expand(a, &dir_text)).collect();
        let seen = run(&dir, None, &argv, &[]);
        if exit_of(case) == 1 {
            rejected += 1;
        }
        let problems = differences(
            exit_of(case),
            &expand(&s(case, "stdout"), &dir_text),
            &expand(&s(case, "stderr"), &dir_text),
            &seen,
        );
        if !problems.is_empty() {
            failures.push(format!(
                "{}: {}",
                s(case, "id").to_utf8().unwrap(),
                problems.join(", ")
            ));
        }
        let _ = std::fs::remove_file(depth_dir.join(format!("{kind}-{depth}.json")));
    }
    assert!(
        rejected >= 4,
        "the rejecting side of the boundary is exercised"
    );
    fail_if_any("depth", cases.len(), failures);
}

#[test]
fn worker_extra_counterexamples_match() {
    let cases = jsonl("extra_cases.jsonl");
    assert!(cases.len() >= 379, "case set shrank: {}", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let id = s(case, "id").to_utf8().unwrap();
        let dir = scratch(&format!("extra-{id}"));
        let binary: Vec<String> = match case.get("binary") {
            Some(PyValue::Dict(map)) => map.iter().map(|(k, _)| k.to_utf8().unwrap()).collect(),
            _ => Vec::new(),
        };
        if let Some(PyValue::Dict(files)) = case.get("files") {
            for (name, content) in files.iter() {
                let name = name.to_utf8().unwrap();
                let PyValue::Str(content) = content else {
                    panic!("file text")
                };
                let bytes = if binary.contains(&name) {
                    content
                        .code_points()
                        .iter()
                        .map(|&p| u8::try_from(p).unwrap())
                        .collect()
                } else {
                    // text files may hold lone surrogates written as the interpreter wrote them
                    content_bytes(content)
                };
                let dir_text = dir.to_str().unwrap();
                let bytes = if binary.contains(&name) {
                    bytes
                } else {
                    String::from_utf8_lossy(&bytes)
                        .replace("@D@", dir_text)
                        .into_bytes()
                };
                std::fs::write(dir.join(&name), bytes).unwrap();
            }
        }
        let dir_text = dir.to_str().unwrap().to_string();
        let expand_d = |text: &PyStr| {
            let token: Vec<u32> = "@D@".chars().map(u32::from).collect();
            let points = text.code_points();
            let mut out = Vec::with_capacity(points.len());
            let mut at = 0;
            while at < points.len() {
                if points[at..].starts_with(&token) {
                    out.extend(dir_text.chars().map(u32::from));
                    at += token.len();
                } else {
                    out.push(points[at]);
                    at += 1;
                }
            }
            PyStr::from_code_points(out)
        };
        let argv: Vec<PyStr> = argv_of(case).iter().map(&expand_d).collect();
        let seen = run(&dir, None, &argv, &[("PARSE_GUIDELLM_FIXTURE", &fixture())]);
        let problems = differences(
            exit_of(case),
            &expand_d(&s(case, "stdout")),
            &expand_d(&s(case, "stderr")),
            &seen,
        );
        if !problems.is_empty() {
            failures.push(format!("{id}: {}", problems.join(", ")));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    fail_if_any("worker-extra", cases.len(), failures);
}

/// The UTF-8 bytes of a text file as the interpreter's `open(..., 'w')` wrote them in the capture (a lone
/// surrogate cannot be written there; the capture used the `surrogatepass` spelling, which is what this emits).
fn content_bytes(text: &PyStr) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &point in text.code_points() {
        match char::from_u32(point) {
            Some(character) => {
                let mut buffer = [0u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
            None => {
                bytes.push(0xE0 | (point >> 12) as u8);
                bytes.push(0x80 | ((point >> 6) & 0x3F) as u8);
                bytes.push(0x80 | (point & 0x3F) as u8);
            }
        }
    }
    bytes
}

#[test]
fn usage_help_and_argument_errors_follow_the_terminal_width() {
    let goldens = jsonl("help_goldens.jsonl");
    assert_eq!(goldens.len(), 132);
    let dir = scratch("help");
    let mut failures = Vec::new();
    for case in &goldens {
        let columns = s(case, "columns").to_utf8().unwrap();
        let seen = run(&dir, Some(&columns), &argv_of(case), &[]);
        let problems = differences(exit_of(case), &s(case, "stdout"), &s(case, "stderr"), &seen);
        if !problems.is_empty() {
            failures.push(format!(
                "COLUMNS={columns:?} {:?}: {}",
                argv_of(case),
                problems.join(", ")
            ));
        }
    }
    fail_if_any("help", goldens.len(), failures);
}

#[test]
fn native_words_bench_tool_and_helper_usage() {
    let dir = scratch("native");
    let seen = run(&dir, None, &[PyStr::new("bench-tool")], &[]);
    assert_eq!(
        (
            seen.exit,
            seen.stdout.to_utf8().unwrap().as_str(),
            seen.stderr.is_empty()
        ),
        (Some(0), "guidellm\n", true)
    );
    let seen = run(&dir, None, &[PyStr::new("native-help")], &[]);
    let help = seen.stdout.to_utf8().unwrap();
    assert_eq!(seen.exit, Some(0));
    for word in [
        "evaluate-jsonl",
        "spec-source --lite-raw PATH",
        "bench-tool",
        "native-help",
    ] {
        assert!(help.contains(word), "{word} is listed: {help}");
    }
    // extra arguments after a Rust-only word fall to the legacy parser, as for every other native binary
    let seen = run(
        &dir,
        None,
        &[PyStr::new("bench-tool"), PyStr::new("x")],
        &[],
    );
    assert_eq!(seen.exit, Some(2));
    let seen = run(&dir, None, &[PyStr::new("spec-source")], &[]);
    assert_eq!(seen.exit, Some(2));
    assert_eq!(
        seen.stderr.to_utf8().unwrap(),
        "usage: parse_guidellm spec-source --lite-raw PATH\n"
    );
    let _ = command; // shared helper module is compiled with every check
}

// ── raw-argument API, descriptor pointers and the JSON transport ─────────────────────────────────────────────

/// Feeds `evaluate-jsonl` the rows of an API observation file and returns (expected, actual) record lines.
fn api_round_trip(name: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let rows = jsonl(name);
    let mut input = String::new();
    let (mut ids, mut expected) = (Vec::new(), Vec::new());
    for row in &rows {
        let mut request = PyDict::new();
        for key in ["id", "operation", "arguments"] {
            request.set(key, row.get(key).cloned().expect("field"));
        }
        input.push_str(&PyValue::Dict(request).dumps_ascii().unwrap());
        input.push('\n');
        let mut want = PyDict::new();
        want.set("id", row.get("id").cloned().unwrap());
        let PyValue::Dict(outcome) = row.get("expected").cloned().expect("expected") else {
            panic!("expected is an object")
        };
        for (key, value) in outcome.iter() {
            want.set(&key.to_utf8().unwrap(), value.clone());
        }
        ids.push(s(row, "id").to_utf8().unwrap());
        expected.push(PyValue::Dict(want).dumps_ascii().unwrap());
    }
    let mut child = std::process::Command::new(BIN)
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let actual: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(actual.len(), expected.len());
    (ids, expected, actual)
}

fn assert_records_equal(label: &str, ids: &[String], expected: &[String], actual: &[String]) {
    let failures: Vec<String> = (0..expected.len())
        .filter(|&at| expected[at] != actual[at])
        .map(|at| {
            format!(
                "{}\n  want {}\n  got  {}",
                ids[at], expected[at], actual[at]
            )
        })
        .collect();
    fail_if_any(label, expected.len(), failures);
}

#[test]
fn raw_json_argument_types_reach_the_operation_they_belong_to() {
    // 12 observations of the original with liveness, index, engine, spec and rate values of every JSON type
    // (the source accepts them, or fails at the operation that needs them, in its own order) ...
    let (ids, expected, actual) = api_round_trip("api_type_cases.jsonl");
    assert_eq!(ids.len(), 12);
    assert_records_equal("api-type", &ids, &expected, &actual);
    // path and index arguments of any type (`%s` of the path, `list[index]` caught as `None`) ...
    let (ids, expected, actual) = api_round_trip("api_path_index_cases.jsonl");
    assert_eq!(ids.len(), 6);
    assert_records_equal("api-path-index", &ids, &expected, &actual);
    // ... and 1200 more, sampled from 8388 differential cases (build / classify_errors / inherit_accept_len).
    let (ids, expected, actual) = api_round_trip("api_raw_cases.jsonl");
    assert_eq!(ids.len(), 1200);
    assert_records_equal("api-raw", &ids, &expected, &actual);
}

#[test]
fn liveness_zero_is_unobserved_and_a_float_index_fails_at_list_indexing() {
    let dir = scratch("api-two");
    let doc = r#"{"benchmarks":[{"requests":{"errored":[{"info":{"error":"Unexpected token 1 while expecting start token 2"},"output_tokens":5}]},"metrics":{}}]}"#;
    let request = |name: &str, extra: &str| {
        format!(
            r#"{{"id":"{name}","operation":"classify_errors","arguments":{{"run":{doc}{extra}}}}}"#
        )
    };
    let mut input = request("zero", r#","server_alive_at_bench_end":0"#);
    input.push('\n');
    input.push_str(
        r#"{"id":"float-index","operation":"build","arguments":{"doc":{"benchmarks":[{"metrics":{}},{"metrics":{}}]},"benchmark_index":1.0}}"#,
    );
    input.push('\n');
    let mut child = std::process::Command::new(BIN)
        .arg("evaluate-jsonl")
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let lines: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        lines[0],
        r#"{"id": "zero", "kind": "value", "value": {"tool_boundary": 1, "server": 0, "unclassified_samples": [], "server_alive_at_bench_end": 0, "boundary_exemption": "enabled(liveness unobserved)"}}"#
    );
    assert_eq!(
        lines[1],
        r#"{"id": "float-index", "kind": "exception", "type": "TypeError", "message": "list indices must be integers or slices, not float"}"#
    );
}

/// Runs the helper the way the original was observed: a shell opens descriptor 3 on the data file (read or
/// write-only), moves the shared offset, starts the helper with the descriptor inherited, then reports how many
/// bytes are still unread behind the shared offset.
fn run_with_descriptor(case: &PyDict, dir: &Path) -> (Seen, usize) {
    let hex = s(case, "data_hex").to_utf8().unwrap();
    let data: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect();
    let id = s(case, "id").to_utf8().unwrap();
    let warm = dir.join(format!("{id}.data"));
    let raw = dir.join(format!("{id}.json"));
    std::fs::write(&warm, &data).unwrap();
    std::fs::write(&raw, s(case, "raw").to_utf8().unwrap()).unwrap();
    let offset: usize = case
        .get("offset")
        .and_then(PyValue::as_plain_int)
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    let open = if s(case, "mode").equals("w") {
        "exec 3>>\"$1\""
    } else {
        "exec 3<\"$1\""
    };
    let skip = if offset > 0 {
        format!("read -r -N {offset} -u 3 _skipped;")
    } else {
        String::new()
    };
    let script = format!(
        "{open}; {skip} \"$2\" spec-source --lite-raw \"$3\" >\"$4\" 2>\"$5\"; echo $? >\"$6\"; cat <&3 2>/dev/null | wc -c >\"$7\""
    );
    let artefact = |suffix: &str| dir.join(format!("{id}.{suffix}"));
    let status = std::process::Command::new("bash")
        .args(["-c", &script, "bash"])
        .arg(&warm)
        .arg(BIN)
        .arg(&raw)
        .args([
            artefact("out"),
            artefact("err"),
            artefact("rc"),
            artefact("left"),
        ])
        .current_dir(dir)
        .stdin(Stdio::null())
        .status()
        .expect("bash");
    assert!(status.success(), "{id}: harness shell failed");
    let read = |suffix: &str| std::fs::read(artefact(suffix)).unwrap();
    let exit = String::from_utf8(read("rc"))
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    let left: usize = String::from_utf8(read("left"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    (
        Seen {
            exit: Some(exit),
            stdout: PyStr::from_surrogateescape(&read("out")),
            stderr: PyStr::from_surrogateescape(&read("err")),
        },
        left,
    )
}

#[test]
fn a_descriptor_pointer_reads_the_current_offset_of_the_shared_description() {
    let dir = scratch("descriptor");
    let cases = jsonl("fd_cases.jsonl");
    assert_eq!(cases.len(), 5);
    let mut failures = Vec::new();
    for case in &cases {
        let (seen, left) = run_with_descriptor(case, &dir);
        let after: usize = case
            .get("after_offset")
            .and_then(PyValue::as_plain_int)
            .unwrap()
            .to_string()
            .parse()
            .unwrap();
        let size: usize = case
            .get("size")
            .and_then(PyValue::as_plain_int)
            .unwrap()
            .to_string()
            .parse()
            .unwrap();
        let mut problems =
            differences(exit_of(case), &s(case, "stdout"), &s(case, "stderr"), &seen);
        // The helper moved the shared offset exactly as the original's file object did.
        if left != size - after && !s(case, "mode").equals("w") {
            problems.push("shared offset");
        }
        if !problems.is_empty() {
            failures.push(format!(
                "{}: {} (unread {left}, expected {})",
                s(case, "id").to_utf8().unwrap(),
                problems.join(", "),
                size - after
            ));
        }
    }
    fail_if_any("descriptor", cases.len(), failures);
}

#[test]
fn numeric_pointers_outside_a_descriptor_keep_the_original_errors() {
    let dir = scratch("numeric");
    let cases: [(&str, &str, i32, &str, &str); 7] = [
        (
            "closed",
            "9",
            1,
            "",
            "ValueError: bench_warm_json 부재/빈 파일: 9\n",
        ),
        (
            "negative",
            "-1",
            1,
            "",
            "ValueError: bench_warm_json 부재/빈 파일: -1\n",
        ),
        (
            "max",
            "2147483647",
            1,
            "",
            "ValueError: bench_warm_json 부재/빈 파일: 2147483647\n",
        ),
        (
            "over",
            "2147483648",
            1,
            "",
            "OverflowError: fd is greater than maximum\n",
        ),
        (
            "under",
            "-2147483649",
            1,
            "",
            "OverflowError: fd is less than minimum\n",
        ),
        (
            "float",
            "3.0",
            1,
            "",
            "TypeError: stat: path should be string, bytes, os.PathLike or integer, not float\n",
        ),
        (
            "zero",
            "0",
            1,
            "",
            "ValueError: lite raw 에 bench_warm_json 이 없다\n",
        ),
    ];
    for (id, pointer, exit, stdout, stderr) in cases {
        let raw = dir.join(format!("{id}.json"));
        std::fs::write(&raw, format!("{{\"bench_warm_json\": {pointer}}}")).unwrap();
        let seen = run(
            &dir,
            None,
            &[
                PyStr::new("spec-source"),
                PyStr::new("--lite-raw"),
                PyStr::new(raw.to_str().unwrap()),
            ],
            &[],
        );
        let want_err = PyStr::new(stderr);
        let problems = differences(exit, &PyStr::new(stdout), &want_err, &seen);
        let hit_traceback = seen.stderr.to_utf8().unwrap().contains("Traceback");
        // Uncaught ones (TypeError/OverflowError) end in a traceback whose last line is compared.
        let problems = if hit_traceback {
            differences(
                exit,
                &PyStr::new(stdout),
                &PyStr::new(&format!("{TRACEBACK}{}", stderr)),
                &seen,
            )
        } else {
            problems
        };
        assert!(
            problems.is_empty(),
            "{id}: {problems:?} {:?}",
            seen.stderr.to_utf8()
        );
    }
}

/// A source-accepted document at the nesting limit: `head` + N arrays around `0`, as the depth cases do.
fn deep_document(depth: usize) -> String {
    let heads =
        match loads(&std::fs::read_to_string(data_path("guidellm/depth_heads.json")).unwrap())
            .unwrap()
        {
            PyValue::Dict(map) => map,
            other => panic!("{other:?}"),
        };
    format!(
        "{}{}0{}}}",
        s(&heads, "bench").to_utf8().unwrap(),
        "[".repeat(depth),
        "]".repeat(depth)
    )
}

fn transport(input: &str) -> Vec<PyValue> {
    let mut child = std::process::Command::new(BIN)
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| loads(line).expect("record JSON"))
        .collect()
}

fn record_field(record: &PyValue, key: &str) -> PyValue {
    match record {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_json_transport_carries_a_document_the_envelope_cannot_nest() {
    let document = deep_document(9996);
    let arguments_json = format!(
        "{{\"doc\": {}, \"spec\": \"{{\\\"accept_len\\\": null, \\\"source\\\": \\\"declared-absent\\\"}}\"}}",
        serde_free_quote(&document)
    );
    let opt_in = format!(
        "{{\"id\":\"deep\",\"operation\":\"build\",\"arguments_json\":{arguments_json},\"result_json\":true}}\n"
    );
    // the standard form nests the same document two levels deeper and exceeds the reader's limit
    let standard = format!(
        "{{\"id\":\"deep-standard\",\"operation\":\"build\",\"arguments\":{{\"doc\":{document}}}}}\n"
    );
    let records = transport(&format!("{opt_in}{standard}"));
    let ok = &records[0];
    assert!(record_field(ok, "encoding").equals_str("json-values-v1"));
    assert!(record_field(ok, "kind").equals_str("value"));
    let PyValue::Str(text) = record_field(ok, "value_json") else {
        panic!("value_json is text")
    };
    let value = loads(&text.to_utf8().unwrap()).unwrap();
    assert_eq!(record_field(&value, "completed"), PyValue::int(16));
    assert!(record_field(&value, "bench_tool").equals_str("guidellm"));
    // the standard record keeps its schema: no encoding field, and the same document is a RecursionError there
    assert!(matches!(
        record_field(&records[1], "encoding"),
        PyValue::None
    ));
    assert!(record_field(&records[1], "type").equals_str("RecursionError"));
}

trait EqualsStr {
    fn equals_str(&self, text: &str) -> bool;
}

impl EqualsStr for PyValue {
    fn equals_str(&self, text: &str) -> bool {
        matches!(self, PyValue::Str(own) if own.equals(text))
    }
}

/// A JSON string literal of `text` (ASCII-only escapes are enough for these inputs).
fn serde_free_quote(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[test]
fn the_json_transport_preserves_specials_and_errors() {
    let base = "{\"benchmarks\":[{\"metrics\":{\"time_per_output_token_ms\":{\"successful\":{\"median\":25.0}},\"request_totals\":{\"successful\":1,\"errored\":0,\"incomplete\":0,\"total\":1}}}]}";
    let quoted = serde_free_quote(base);
    let lines = [
        // NaN / Infinity / a lone surrogate / a huge integer in raw argument texts and in the result text
        format!("{{\"id\":1,\"operation\":\"build\",\"arguments_json\":{{\"doc\":{quoted},\"max_error_rate\":\"NaN\",\"engine_max\":\"Infinity\"}},\"result_json\":true}}"),
        "{\"id\":2,\"operation\":\"inherit_accept_len\",\"arguments_json\":{\"src\":\"{\\\"completed\\\":1}\",\"path\":\"\\\"a\\\\ud800b\\\"\"},\"result_json\":true}".to_string(),
        // errors keep type/message and gain the encoding marker
        "{\"id\":3,\"operation\":\"build\",\"arguments_json\":{\"doc\":\"null\"},\"result_json\":true}".to_string(),
        "{\"id\":4,\"operation\":\"build\",\"arguments_json\":{},\"result_json\":true}".to_string(),
        "{\"id\":5,\"operation\":\"build\",\"arguments_json\":{\"doc\":\"{\"},\"result_json\":true}".to_string(),
        "{\"id\":6,\"operation\":\"build\",\"arguments\":{\"doc\":null},\"arguments_json\":{\"doc\":\"null\"},\"result_json\":true}".to_string(),
        "{\"id\":7,\"operation\":\"build\",\"arguments\":{\"doc\":null}}".to_string(),
    ];
    let records = transport(&(lines.join("\n") + "\n"));
    let PyValue::Str(text) = record_field(&records[0], "value_json") else {
        panic!("{:?}", records[0])
    };
    let first = loads(&text.to_utf8().unwrap()).unwrap();
    assert!(
        matches!(record_field(&first, "max_error_rate_declared"), PyValue::Float(rate) if rate.is_nan())
    );
    assert!(
        matches!(record_field(&first, "engine_gen_throughput_max"), PyValue::Float(engine) if engine.is_infinite())
    );
    let PyValue::Str(text) = record_field(&records[1], "value_json") else {
        panic!("{:?}", records[1])
    };
    let inherited = loads(&text.to_utf8().unwrap()).unwrap();
    let PyValue::Str(source) = record_field(&inherited, "source") else {
        panic!()
    };
    assert_eq!(
        source.code_points(),
        &[105, 110, 104, 101, 114, 105, 116, 101, 100, 40, 97, 0xD800, 98, 41]
    );
    for (at, class, message) in [
        (2, "ValueError", "benchmarks.json 은 JSON 객체여야 한다"),
        (3, "KeyError", "'doc'"),
    ] {
        assert!(
            record_field(&records[at], "encoding").equals_str("json-values-v1"),
            "{at}"
        );
        assert!(record_field(&records[at], "type").equals_str(class), "{at}");
        assert!(
            record_field(&records[at], "message").equals_str(message),
            "{at}"
        );
    }
    assert!(record_field(&records[4], "type").equals_str("JSONDecodeError"));
    assert!(record_field(&records[5], "message")
        .equals_str("arguments and arguments_json are mutually exclusive"));
    assert!(matches!(
        record_field(&records[6], "encoding"),
        PyValue::None
    ));
    assert!(record_field(&records[6], "type").equals_str("ValueError"));
}
