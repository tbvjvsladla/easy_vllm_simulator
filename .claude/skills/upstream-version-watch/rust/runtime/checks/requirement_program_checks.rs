//! Checks of the normal program `upstream_requirement_api evaluate-jsonl [--input-fd FD]` (R35 Requirement constructor projection): the built binary is
//! started with an empty environment (no `PATH`, no Python), its process id, argv, environment, exit status and raw streams are kept under
//! `CARGO_TARGET_TMPDIR/r35-requirement-program/<scenario>/`, and the records are compared with the saved originals read through
//! `R35_REQUIREMENT_REFERENCE`. No Python is started and no original is replayed.

use agent_runtime_support::pyjson::{loads, loads_text};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use upstream_runtime::api::render;

const PROGRAM: &str = env!("CARGO_BIN_EXE_upstream_requirement_api");
const OPERATION: &str = "packaging24.requirement_main_projection";

fn reference() -> PathBuf {
    PathBuf::from(
        std::env::var("R35_REQUIREMENT_REFERENCE")
            .expect("R35_REQUIREMENT_REFERENCE must name the saved reference directory"),
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
        .unwrap_or_else(|| panic!("no {name} in {}", render(value).unwrap()))
}

fn shown(value: &PyValue) -> String {
    render(value).expect("renderable")
}

/// The saved input DTO of a case.
fn case_input(id: &str) -> PyValue {
    let cases = read(reference().join("cases.json"));
    let PyValue::List(cases) = cases else {
        panic!("cases")
    };
    cases
        .iter()
        .find(|case| member(case, "id").equals_text(id))
        .map(|case| member(case, "input_dto").clone())
        .unwrap_or_else(|| panic!("no case {id}"))
}

trait EqualsText {
    fn equals_text(&self, text: &str) -> bool;
}

impl EqualsText for PyValue {
    fn equals_text(&self, text: &str) -> bool {
        matches!(self, PyValue::Str(own) if own.equals(text))
    }
}

fn request(id: &str, input: &PyValue, seed: u32) -> String {
    format!(
        r#"{{"id": {}, "operation": "{OPERATION}", "input_dto": {}, "hash_seed": {seed}}}"#,
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

fn keep(scenario: &str, name: &str, bytes: &[u8]) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("r35-requirement-program")
        .join(scenario);
    std::fs::create_dir_all(&dir).expect("scenario directory");
    std::fs::write(dir.join(name), bytes).expect("kept file");
}

/// Runs the program with an empty environment. `through_descriptor` delivers the bytes on descriptor 3 (`--input-fd 3`, the caller's
/// `pass_fds` contract) with descriptor 0 at `/dev/null`; otherwise they go to standard input.
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("start the program");
    let pid = child.id();
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(bytes)
        .expect("requests");
    let output = child.wait_with_output().expect("wait");
    let process = format!(
        "{{\"pid\": {pid}, \"argv\": {:?}, \"environment\": {{}}, \"through_descriptor\": {through_descriptor}, \"first_exit_code\": {:?}}}\n",
        std::iter::once(PROGRAM).chain(args.iter().copied()).collect::<Vec<_>>(),
        output.status.code()
    );
    print!("[{scenario}] {process}");
    keep(scenario, "request.jsonl", bytes);
    keep(scenario, "process.json", process.as_bytes());
    keep(scenario, "stdout.bin", &output.stdout);
    keep(scenario, "stderr.bin", &output.stderr);
    Run {
        exit_code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

fn saved(seed: u32, id: &str) -> PyValue {
    let number = id.strip_prefix("requirement-").expect("case id");
    read(reference().join(format!("seed{seed}-requirement-{number}/owner-record.json")))
}

const REPRESENTATIVE: [&str; 9] = [
    "requirement-001",
    "requirement-027",
    "requirement-050",
    "requirement-053",
    "requirement-054",
    "requirement-093",
    "requirement-098",
    "requirement-100",
    "requirement-112",
];

fn compare_with_saved(records: &[PyValue], expected: &[(u32, &str)]) {
    assert_eq!(records.len(), expected.len());
    for (record, (seed, id)) in records.iter().zip(expected) {
        let original = saved(*seed, id);
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
                "{id} seed {seed}: {name}"
            );
        }
        assert!(member(record, "id").equals_text(id));
    }
}

fn lines_for(expected: &[(u32, &str)]) -> Vec<u8> {
    let mut text = String::new();
    for (seed, id) in expected {
        text.push_str(&request(id, &case_input(id), *seed));
        text.push('\n');
    }
    text.into_bytes()
}

#[test]
fn representative_cases_through_standard_input_match_the_saved_originals_under_both_seeds() {
    let expected: Vec<(u32, &str)> = REPRESENTATIVE
        .iter()
        .flat_map(|id| [(0, *id), (17, *id)])
        .collect();
    let run = run(
        "stdin_representative",
        &["evaluate-jsonl"],
        &lines_for(&expected),
        false,
    );
    assert_eq!(run.exit_code, Some(0));
    assert!(
        run.stderr.is_empty(),
        "the program itself writes nothing to standard error"
    );
    compare_with_saved(&run.records(), &expected);
}

#[test]
fn the_same_requests_over_descriptor_three_give_the_same_bytes() {
    let expected: Vec<(u32, &str)> = REPRESENTATIVE.iter().map(|id| (17, *id)).collect();
    let bytes = lines_for(&expected);
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
    compare_with_saved(&through.records(), &expected);
}

#[test]
fn the_seed_profiles_differ_where_the_originals_differ() {
    let expected = [(0, "requirement-053"), (17, "requirement-053")];
    let run = run(
        "seed_order",
        &["evaluate-jsonl"],
        &lines_for(&expected),
        false,
    );
    let records = run.records();
    assert_ne!(
        shown(member(&records[0], "projection")),
        shown(member(&records[1], "projection"))
    );
    compare_with_saved(&records, &expected);
}

#[test]
fn protocol_problems_are_refusal_records_with_a_clean_exit() {
    let good = shown(&PyValue::List(vec![
        PyValue::str("str"),
        PyValue::str("torch"),
    ]));
    let text = format!(
        "this is not json\n{}\n{}\n{}\n{}\n",
        format_args!(
            r#"{{"id": "s", "operation": "{OPERATION}", "input_dto": {good}, "hash_seed": 5}}"#
        ),
        format_args!(
            r#"{{"id": "d", "operation": "{OPERATION}", "input_dto": ["nonsense"], "hash_seed": 0}}"#
        ),
        format_args!(r#"{{"id": "o", "operation": "wrong", "input_dto": {good}, "hash_seed": 0}}"#),
        format_args!(
            r#"{{"id": "ok", "operation": "{OPERATION}", "input_dto": {good}, "hash_seed": 0}}"#
        ),
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
            Some(refusal) => shown(member(refusal, "kind")),
            None => "business".to_string(),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "\"malformed_request\"",
            "\"unsupported_hash_seed\"",
            "\"malformed_input_dto\"",
            "\"unsupported_operation\"",
            "business"
        ]
    );
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
