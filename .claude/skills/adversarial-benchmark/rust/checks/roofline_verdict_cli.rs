//! The `roofline` and `verdict_rule` command lines (and the `spec-row` computation of `judge_bench.sh`) against what
//! the ORIGINAL programs printed.
//!
//! `cli_cases.jsonl` holds the root's 184 frozen observations plus 429 more captured by the worker; `spec_row_cases.jsonl`
//! the root's 150 `SPEC_ROW` observations plus 368 more. Input files (models with safetensors headers, manifests,
//! measured/roofline documents) are recreated from `inputs.jsonl` in a scratch tree; `{DIR}` in a case means its root.
//! Exit status and stdout must match byte for byte; stderr too, except that an uncaught Python exception ends in a
//! traceback of source-language frames: before the `Traceback` line and in the final `Class: message` line it must
//! match. There is no accepted deviation. The one place where the ORIGINAL itself is not deterministic - the message
//! of a `sorted(set(...))` over mixed-type values depends on the hash seed - is limited to the two named probes and
//! checked against `set_order_criteria.json` (the final line must be one of the messages the original really printed;
//! exit status, empty stdout and the text before the traceback must still match exactly).

#[path = "common.rs"]
mod common;

use common::{command, load_jsonl, roofline_inputs, text_of};
use std::process::Stdio;
use sweep_stop::pyvalue::{PyDict, PyValue};

const TRACEBACK: &str = "Traceback (most recent call last):\n";

struct Case {
    id: String,
    tool: String,
    argv: Vec<String>,
    env: Vec<(String, String)>,
    exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    deviation: Option<String>,
}

fn bytes_of(value: Option<&PyValue>) -> Vec<u8> {
    match value {
        Some(PyValue::Str(text)) => text
            .to_surrogateescape_bytes()
            .expect("captured text is encodable"),
        other => panic!("expected a string, got {other:?}"),
    }
}

fn cases(file: &str) -> Vec<Case> {
    load_jsonl(file)
        .into_iter()
        .map(|record: PyDict| Case {
            id: text_of(record.get("id")),
            tool: text_of(record.get("tool")),
            argv: match record.get("argv") {
                Some(PyValue::List(items)) => items.iter().map(|a| text_of(Some(a))).collect(),
                other => panic!("argv must be a list, got {other:?}"),
            },
            env: match record.get("env") {
                Some(PyValue::Dict(map)) => map
                    .iter()
                    .map(|(k, v)| (k.to_utf8().unwrap(), text_of(Some(v))))
                    .collect(),
                _ => Vec::new(),
            },
            exit_code: record
                .get("exit_code")
                .and_then(PyValue::as_plain_int)
                .unwrap()
                .to_string()
                .parse()
                .unwrap(),
            stdout: bytes_of(record.get("stdout")),
            stderr: bytes_of(record.get("stderr")),
            deviation: match record.get("deviation") {
                Some(PyValue::Str(text)) => text.to_utf8(),
                _ => None,
            },
        })
        .collect()
}

fn expand(bytes: &[u8], dir: &str) -> Vec<u8> {
    let needle = b"{DIR}";
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(needle) {
            out.extend_from_slice(dir.as_bytes());
            at += needle.len();
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

fn bin(tool: &str) -> &'static str {
    match tool {
        "roofline" => env!("CARGO_BIN_EXE_roofline"),
        "verdict_rule" => env!("CARGO_BIN_EXE_verdict_rule"),
        other => panic!("unknown tool {other}"),
    }
}

/// Text before the traceback, and its final line (`None` when the program did not die on an exception).
fn projection(stderr: &[u8]) -> (String, Option<String>) {
    let text = String::from_utf8_lossy(stderr).into_owned();
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

fn set_order_lines(id: &str) -> Option<Vec<String>> {
    let criteria = common::load_json_file(&common::data_path(
        "roofline_verdict/set_order_criteria.json",
    ));
    let PyValue::Dict(root) = criteria else {
        panic!("criteria object")
    };
    let Some(PyValue::Dict(allowed)) = root.get("allowed_final_exception_lines") else {
        panic!("allowed lines")
    };
    let name = id.split_once('-').map_or(id, |(_, rest)| rest);
    let key = allowed
        .iter()
        .map(|(k, _)| k.to_utf8().unwrap())
        .find(|k| k.split_once('-').map_or(k.as_str(), |(_, r)| r) == name)?;
    match allowed.get(&key) {
        Some(PyValue::List(lines)) => Some(lines.iter().map(|l| text_of(Some(l))).collect()),
        _ => None,
    }
}

fn run_case(case: &Case, dir: &str) -> Vec<String> {
    let argv: Vec<String> = case.argv.iter().map(|a| a.replace("{DIR}", dir)).collect();
    let mut child = command(bin(&case.tool), None, &argv);
    for (key, value) in &case.env {
        child.env(key, value);
    }
    let output = child.stdin(Stdio::null()).output().expect("run");
    let mut problems = Vec::new();
    let code = output.status.code();
    if code != Some(case.exit_code) {
        problems.push(format!("exit {code:?} want {}", case.exit_code));
    }
    let want_stdout = expand(&case.stdout, dir);
    if output.stdout != want_stdout {
        problems.push("stdout".to_string());
    }
    let want_stderr = expand(&case.stderr, dir);
    let (got_head, got_last) = projection(&output.stderr);
    let (want_head, want_last) = projection(&want_stderr);
    if let Some(allowed) = set_order_lines(&case.id) {
        // the original's own message varies with the hash seed: any message it really printed is accepted
        if got_head != want_head || !got_last.as_ref().is_some_and(|last| allowed.contains(last)) {
            problems.push(format!(
                "set-order stderr: got {:?} allowed {allowed:?}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else if (got_head, got_last) != (want_head, want_last) {
        problems.push(format!(
            "stderr: got {:?}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    problems
}

fn check(file: &str, at_least: usize) {
    let dir = roofline_inputs()
        .to_str()
        .expect("UTF-8 scratch path")
        .to_string();
    let all = cases(file);
    assert!(
        all.len() >= at_least,
        "{file}: case set shrank to {}",
        all.len()
    );
    let failures: Vec<String> = all
        .iter()
        .filter_map(|case| {
            let problems = run_case(case, &dir);
            (!problems.is_empty()).then(|| format!("{}: {}", case.id, problems.join("; ")))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        all.len(),
        failures.join("\n")
    );
}

#[test]
fn every_captured_command_line_matches_the_original() {
    check("roofline_verdict/cli_cases.jsonl", 613);
}

#[test]
fn every_captured_spec_row_matches_the_original() {
    check("roofline_verdict/spec_row_cases.jsonl", 518);
}

#[test]
fn root_reference_mix_is_as_frozen() {
    let mut by_exit = std::collections::BTreeMap::new();
    let mut count = 0;
    for record in load_jsonl("roofline_verdict/cli_cases.jsonl") {
        if text_of(record.get("source")) == "root-reference" {
            count += 1;
            *by_exit
                .entry(
                    record
                        .get("exit_code")
                        .and_then(PyValue::as_plain_int)
                        .unwrap()
                        .to_string(),
                )
                .or_insert(0) += 1;
        }
    }
    assert_eq!(count, 184);
    assert_eq!(
        by_exit,
        std::collections::BTreeMap::from([
            ("0".to_string(), 133),
            ("1".to_string(), 4),
            ("2".to_string(), 47)
        ])
    );
}

#[test]
fn no_case_is_allowed_to_deviate() {
    let all = cases("roofline_verdict/cli_cases.jsonl");
    assert!(
        all.iter().all(|case| case.deviation.is_none()),
        "a case carries a deviation marker"
    );
    let named: Vec<&str> = all
        .iter()
        .filter(|c| set_order_lines(&c.id).is_some())
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        named.len(),
        2,
        "only the two set-order probes use the hash-seed criteria: {named:?}"
    );
}

#[test]
fn a_malformed_manifest_ends_with_a_parser_error_and_never_hangs() {
    let dir = roofline_inputs();
    let manifest = dir.join("extra2/hang.yaml");
    assert_eq!(
        std::fs::read(&manifest).unwrap(),
        b"gpu_model: [}\n",
        "the 14-byte manifest of the independent report"
    );
    let mut child = command(
        bin("roofline"),
        None,
        &[
            "--model-path".to_string(),
            dir.join("extra2/dense").to_str().unwrap().into(),
            "--manifest".into(),
            manifest.to_str().unwrap().into(),
            "--bandwidth-gbps".into(),
            "273".into(),
        ],
    )
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("spawn");
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the process did not end"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(2));
    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut stderr).unwrap();
    assert!(
        stderr.contains(
            "ParserError: while parsing a flow node\nexpected the node content, but found '}'"
        ),
        "{stderr}"
    );
}

#[test]
fn self_test_passes_and_prints_its_transcript_line() {
    let output = command(bin("verdict_rule"), None, &["--self-test".to_string()])
        .stdin(Stdio::null())
        .output()
        .expect("run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[verdict --self-test] OK — T1~T23 전부 통과(공허 PASS 경로 부재 + explore 밸런스=서술 + spec 축 SPEC_ACCEPT_LEN_MISSING 단언 포함)\n"
    );
}

#[test]
fn authority_check_replaces_the_broad_search_snippet() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("authority_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let run = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).expect("state file");
        let output = command(
            bin("verdict_rule"),
            None,
            &[
                "authority-check".to_string(),
                "--state".to_string(),
                path.to_str().unwrap().to_string(),
            ],
        )
        .stdin(Stdio::null())
        .output()
        .expect("run");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    assert_eq!(
        run("ok.json", r#"{"rubric_authority": "explore"}"#),
        (Some(0), "explore\n".into(), String::new())
    );
    let (code, out, err) = run("bad.json", r#"{"rubric_authority": "bad"}"#);
    assert_eq!((code, out.as_str()), (Some(2), ""));
    assert_eq!(err, "[broad_search] ERROR 상태 파일의 rubric_authority='bad' 가 없거나 값역 weak|explicit|explore 밖이다 — 권한 없이 판정하지 않는다(기본값을 넣지 않는다)\n");
    assert_eq!(run("empty.json", "{}").0, Some(2));
    assert_eq!(
        run("null.json", "null").0,
        Some(2),
        "a falsy document is read as {{}}"
    );
    assert_eq!(
        run("list.json", "[1]").0,
        Some(1),
        "a list has no get: uncaught exception"
    );
    assert_eq!(run("broken.json", "{bad").0, Some(1));
}
