//! Checks of the normal program `upstream_toml_api evaluate-jsonl [--input-fd FD]` (R35 default `tomllib.loads` component): the built binary is
//! started with the explicit environment `PATH=''` (an empty `PATH`, not an absent one; nothing else), and for every run its process id, argv,
//! environment, first exit status and raw streams are kept in a directory that is unique to the run
//! (`CARGO_TARGET_TMPDIR/r35-toml-program/<scenario>-<pid>-<nanoseconds>/`), so no run overwrites another. The records are compared with the saved
//! originals read through `R35_TOML_REFERENCE`. No Python is started and no original is replayed.

use agent_runtime_support::pyjson::{loads, loads_text};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use upstream_runtime::api::render;

const PROGRAM: &str = env!("CARGO_BIN_EXE_upstream_toml_api");
const OPERATION: &str = "tomllib.loads.default";

fn reference() -> PathBuf {
    PathBuf::from(
        std::env::var("R35_TOML_REFERENCE")
            .expect("R35_TOML_REFERENCE must name the saved reference directory"),
    )
}

fn read(path: PathBuf) -> PyValue {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    loads(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("not an object: {}", other.repr()),
    }
}

fn member<'a>(value: &'a PyValue, name: &str) -> &'a PyValue {
    dict(value)
        .get(name)
        .unwrap_or_else(|| panic!("no {name} in {}", shown(value)))
}

fn shown(value: &PyValue) -> String {
    render(value).expect("renderable")
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("not text: {}", other.repr()),
    }
}

fn cases() -> Vec<PyValue> {
    match read(reference().join("cases.json")) {
        PyValue::List(cases) => cases,
        _ => panic!("cases"),
    }
}

fn case_input(id: &str) -> PyValue {
    cases()
        .iter()
        .find(|case| text_of(member(case, "id")) == id)
        .map(|case| member(case, "input_dto").clone())
        .unwrap_or_else(|| panic!("no case {id}"))
}

fn request(id: &str, input: &PyValue) -> String {
    format!(
        r#"{{"id": {}, "operation": "{OPERATION}", "input_dto": {}}}"#,
        shown(&PyValue::str(id)),
        shown(input)
    )
}

struct Run {
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Run {
    fn records(&self) -> Vec<PyValue> {
        String::from_utf8(self.stdout.clone())
            .expect("utf-8 records")
            .lines()
            .map(|line| loads_text(&PyStr::new(line)).expect("a record line"))
            .collect()
    }
}

fn unique_directory(scenario: &str, pid: u32) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("r35-toml-program")
        .join(format!("{scenario}-{pid}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("run directory");
    dir
}

/// Runs the program with the environment `PATH=''` and nothing else. `through_descriptor` delivers the bytes on descriptor 3 (`--input-fd 3`, the
/// caller's `pass_fds` contract) with descriptor 0 at `/dev/null`; otherwise they go to standard input.
fn run(scenario: &str, args: &[&str], bytes: &[u8], through_descriptor: bool) -> Run {
    let mut command = if through_descriptor {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "exec \"$@\" 3<&0 0</dev/null", "sh", PROGRAM])
            .args(args);
        command
    } else {
        let mut command = Command::new(PROGRAM);
        command.args(args);
        command
    };
    command
        .env_clear()
        .env("PATH", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let argv: Vec<String> = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    let environment: Vec<(String, String)> = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value
                    .map(|v| v.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(environment, [("PATH".to_string(), String::new())]);
    let mut child = command.spawn().expect("start the program");
    let pid = child.id();
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(bytes)
        .expect("requests");
    let output = child.wait_with_output().expect("wait");
    let dir = unique_directory(scenario, pid);
    let process = format!(
        "{{\"pid\": {pid}, \"argv\": {argv:?}, \"environment\": {{\"PATH\": \"\"}}, \"through_descriptor\": {through_descriptor}, \"first_exit_code\": {:?}}}\n",
        output.status.code()
    );
    print!("[{scenario}] {process}");
    for (name, content) in [
        ("request.jsonl", bytes),
        ("process.json", process.as_bytes()),
        ("stdout.bin", &output.stdout),
        ("stderr.bin", &output.stderr),
    ] {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(name))
            .expect("a run directory is never reused");
        file.write_all(content).expect("kept file");
    }
    Run {
        exit_code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

fn saved(id: &str) -> PyValue {
    let number = id.strip_prefix("toml-").expect("case id");
    read(reference().join(format!("seed0-toml-{number}/owner-record.json")))
}

const MEMBERS: [&str; 7] = [
    "id",
    "scope",
    "projection",
    "input_before",
    "input_after",
    "stdout",
    "stderr",
];

fn compare_with_saved(records: &[PyValue], ids: &[String]) {
    assert_eq!(records.len(), ids.len());
    for (record, id) in records.iter().zip(ids) {
        let original = saved(id);
        let names: Vec<String> = dict(record)
            .iter()
            .map(|(k, _)| k.to_utf8().unwrap())
            .collect();
        assert_eq!(names, MEMBERS, "{id}");
        for name in [
            "projection",
            "input_before",
            "input_after",
            "stdout",
            "stderr",
        ] {
            assert_eq!(
                shown(member(record, name)),
                shown(member(&original, name)),
                "{id}: {name}"
            );
        }
        assert_eq!(text_of(member(record, "id")), *id);
    }
}

fn lines_for(ids: &[String]) -> Vec<u8> {
    let mut text = String::new();
    for id in ids {
        text.push_str(&request(id, &case_input(id)));
        text.push('\n');
    }
    text.into_bytes()
}

fn ids(numbers: &[u32]) -> Vec<String> {
    numbers.iter().map(|n| format!("toml-{n:03}")).collect()
}

/// A value, an exception, time kinds, bigints (decimal limit and hexadecimal), signed NaN, multiline strings, CRLF, a Unicode error column and an
/// exception with a cause.
const REPRESENTATIVE: [u32; 15] = [1, 9, 24, 25, 26, 29, 30, 33, 36, 38, 41, 42, 44, 61, 65];

#[test]
fn all_68_cases_through_standard_input_match_the_saved_originals() {
    let all: Vec<String> = (1..=68)
        .collect::<Vec<u32>>()
        .iter()
        .map(|n| format!("toml-{n:03}"))
        .collect();
    let run = run("stdin_all68", &["evaluate-jsonl"], &lines_for(&all), false);
    assert_eq!(run.exit_code, Some(0));
    assert!(
        run.stderr.is_empty(),
        "the program itself writes nothing to standard error"
    );
    compare_with_saved(&run.records(), &all);
}

#[test]
fn representative_cases_through_standard_input_match_the_saved_originals() {
    let wanted = ids(&REPRESENTATIVE);
    let run = run(
        "stdin_representative",
        &["evaluate-jsonl"],
        &lines_for(&wanted),
        false,
    );
    assert_eq!(run.exit_code, Some(0));
    assert!(run.stderr.is_empty());
    compare_with_saved(&run.records(), &wanted);
}

#[test]
fn the_same_requests_over_descriptor_three_give_the_same_bytes() {
    let wanted = ids(&REPRESENTATIVE);
    let bytes = lines_for(&wanted);
    let direct = run("fd3_reference_stdin", &["evaluate-jsonl"], &bytes, false);
    let through = run(
        "fd3_descriptor",
        &["evaluate-jsonl", "--input-fd", "3"],
        &bytes,
        true,
    );
    assert_eq!(through.exit_code, Some(0));
    assert_eq!(through.stdout, direct.stdout);
    assert!(through.stderr.is_empty());
    compare_with_saved(&through.records(), &wanted);
}

#[test]
fn a_multiline_request_stream_with_blank_lines_and_crlf_separators_is_answered_in_order() {
    let wanted = ids(&[24, 42, 65]);
    let stream = lines_for(&wanted);
    let text = String::from_utf8(stream)
        .unwrap()
        .replace('\n', "\r\n\r\n   \r\n");
    let run = run(
        "crlf_separated_requests",
        &["evaluate-jsonl"],
        text.as_bytes(),
        false,
    );
    assert_eq!(run.exit_code, Some(0));
    compare_with_saved(&run.records(), &wanted);
}

#[test]
fn protocol_problems_are_refusal_records_with_a_clean_exit_and_no_projection() {
    let good = r#"["str", "a=1\n"]"#;
    let deep = format!("a={}{}\\n", "[".repeat(600), "]".repeat(600));
    let text = format!(
        "this is not json\n{}\n{}\n{}\n{}\n{}\n{}\n",
        format_args!(
            r#"{{"id": "x", "operation": "{OPERATION}", "input_dto": {good}, "hash_seed": 0}}"#
        ),
        format_args!(r#"{{"id": "o", "operation": "wrong", "input_dto": {good}}}"#),
        format_args!(r#"{{"id": "d", "operation": "{OPERATION}", "input_dto": ["nonsense"]}}"#),
        format_args!(r#"{{"id": "n", "operation": "{OPERATION}", "input_dto": ["int", "7"]}}"#),
        format_args!(
            r#"{{"id": "r", "operation": "{OPERATION}", "input_dto": ["str", "{deep}"]}}"#
        ),
        format_args!(r#"{{"id": "ok", "operation": "{OPERATION}", "input_dto": {good}}}"#),
    );
    let run = run(
        "protocol_refusals",
        &["evaluate-jsonl"],
        text.as_bytes(),
        false,
    );
    assert_eq!(run.exit_code, Some(0));
    let records = run.records();
    let kinds: Vec<String> = records
        .iter()
        .map(|record| match dict(record).get("refusal") {
            Some(refusal) => text_of(member(refusal, "kind")),
            None => "business".to_string(),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "malformed_request",
            "malformed_request",
            "unsupported_operation",
            "malformed_input_dto",
            "non_text_input",
            "nesting_beyond_recursion_limit",
            "business"
        ]
    );
    for record in &records[..6] {
        assert!(dict(record).get("projection").is_none());
    }
}

#[test]
fn usage_problems_exit_with_a_message_and_no_records() {
    for (scenario, args) in [
        ("usage_none", vec![]),
        ("usage_unknown", vec!["bogus"]),
        ("usage_low_fd", vec!["evaluate-jsonl", "--input-fd", "1"]),
    ] {
        let run = run(scenario, &args, b"", false);
        assert_eq!(run.exit_code, Some(2), "{scenario}");
        assert!(run.stdout.is_empty(), "{scenario}");
        assert!(!run.stderr.is_empty(), "{scenario}");
    }
}

#[test]
fn an_unreadable_descriptor_fails_before_any_record() {
    let run = run(
        "closed_descriptor",
        &["evaluate-jsonl", "--input-fd", "9"],
        b"",
        false,
    );
    assert_eq!(run.exit_code, Some(1));
    assert!(run.stdout.is_empty());
    assert!(!run.stderr.is_empty());
}
