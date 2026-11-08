//! The `repeat_axis` and `repeat_axis_api` programs against the ORIGINAL `repeat_axis.py`'s captured behaviour.
//!
//! * `repeat_axis/api.jsonl` - 103 pure API observations (`band_pct`, `parse_repeats`, `summarize`,
//!   `repetition_established`) fed to `evaluate-jsonl`; whole results compare by their ordered JSON text, so an
//!   `int`/`float`/`bool` or key-order difference is a difference and NaN compares by spelling.
//! * `repeat_axis/filesystem_cases.json` - 42 real CLI invocations with the whole before/after file tree
//!   (`kind`, bytes) compared, stdout and exit status exact; stderr exact except that an uncaught Python exception
//!   is compared in the text before the `Traceback` line and in its final `Class: message` line (Python frames are
//!   source-language specific).
//! * `repeat_axis/campaign_cases.json` - 82 `load_campaign_budgets` / `resolve_repeats` calls in temporary standard
//!   repository trees with the real `campaign_init.py` copied in (`@ROOT@` is the scratch root); the tree is
//!   unchanged by every call.
//! * `repeat_axis/cli_goldens.jsonl` - 454 argparse edge-case invocations captured from the original (top parser,
//!   subparsers, abbreviations, `--`, terminal widths).
//! * the program's own self-test, which must reproduce the original's transcript by really evaluating its checks,
//!   and must fail (not pass) with a missing schema or a missing owner source.
//!
//! The pending Python owner `campaign_init.py` is reached through the query adapter. The checks read where the
//! real repository and adapter can be given by `REPEAT_AXIS_REPO_ROOT` / `REPEAT_AXIS_CAMPAIGN_ADAPTER`.
//! Otherwise the installed owner layout supplies their paths. Missing actual files fail the check; no oracle is skipped.

#[path = "common.rs"]
mod common;

use common::{command, data_path};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

const API: &str = env!("CARGO_BIN_EXE_repeat_axis_api");
const CLI: &str = env!("CARGO_BIN_EXE_repeat_axis");
const TRACEBACK: &str = "Traceback (most recent call last):\n";
const OWNER_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py";
const SCHEMA_REL: &str = "campaigns/_template/campaign.schema.json";

// ── environment ─────────────────────────────────────────────────────────────────

fn real_repo() -> PathBuf {
    let repo = std::env::var_os("REPEAT_AXIS_REPO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .expect("installed owner layout has a repository root")
                .to_path_buf()
        });
    assert!(
        repo.join(OWNER_REL).is_file(),
        "NOT_RUN: no campaign_init.py under {}",
        repo.display()
    );
    repo
}

fn adapter() -> PathBuf {
    let adapter = std::env::var_os("REPEAT_AXIS_CAMPAIGN_ADAPTER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/repeat_axis_owner_query.py")
        });
    assert!(
        adapter.is_file(),
        "NOT_RUN: adapter missing: {}",
        adapter.display()
    );
    adapter
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let number = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "repeat-axis-check.{}.{tag}.{number}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn base64_decode(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        other => panic!("bad base64 byte {other}"),
    };
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for byte in text.bytes().filter(|b| *b != b'=') {
        buffer = (buffer << 6) | u32::from(value(byte));
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    out
}

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
}

fn snapshot(root: &Path) -> BTreeMap<String, Entry> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Entry>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                out.insert(relative, Entry::Directory);
                walk(root, &path, out);
            } else {
                out.insert(relative, Entry::File(std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn as_dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("expected object, got {}", other.repr()),
    }
}

fn text(value: Option<&PyValue>) -> String {
    match value {
        Some(PyValue::Str(text)) => text.to_utf8().expect("reference text is encodable"),
        other => panic!("expected string, got {other:?}"),
    }
}

fn json_file(name: &str) -> PyValue {
    let raw = std::fs::read_to_string(data_path(&format!("repeat_axis/{name}"))).unwrap();
    loads(&raw).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// Ordered JSON spelling of a value (what the transport prints).
fn spelled(value: &PyValue) -> String {
    value.dumps_ascii().expect("reference values are printable")
}

fn tree_of(description: &PyDict) -> BTreeMap<String, Entry> {
    description
        .iter()
        .map(|(path, entry)| {
            let entry = as_dict(entry);
            let kind = text(entry.get("kind"));
            let value = match kind.as_str() {
                "directory" => Entry::Directory,
                "file" => Entry::File(base64_decode(&text(entry.get("bytes_base64")))),
                other => panic!("unknown entry kind {other}"),
            };
            (path.to_utf8().unwrap(), value)
        })
        .collect()
}

fn materialize(root: &Path, tree: &BTreeMap<String, Entry>) {
    for (relative, entry) in tree {
        let path = root.join(relative);
        match entry {
            Entry::Directory => std::fs::create_dir_all(&path).unwrap(),
            Entry::File(bytes) => {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, bytes).unwrap();
            }
        }
    }
}

/// A standard repository tree holding a copy of the real owner.
fn owner_root(tag: &str) -> Scratch {
    let scratch = Scratch::new(tag);
    let owner = scratch.0.join(OWNER_REL);
    std::fs::create_dir_all(owner.parent().unwrap()).unwrap();
    std::fs::copy(real_repo().join(OWNER_REL), owner).unwrap();
    scratch
}

struct Outcome {
    stdout: Vec<u8>,
    stderr: String,
    code: i32,
}

fn run(
    bin: &str,
    argv: &[String],
    cwd: &Path,
    columns: Option<&str>,
    repo: &Path,
    stdin: Option<&str>,
) -> Outcome {
    let mut child = command(bin, columns, argv)
        .current_dir(cwd)
        .env("REPEAT_AXIS_REPO_ROOT", repo)
        // the DEFAULT public configuration: no user-set adapter or interpreter, only the wrapper's default location
        .env_remove("REPEAT_AXIS_CAMPAIGN_ADAPTER")
        .env_remove("REPEAT_AXIS_PYTHON")
        .env("REPEAT_AXIS_COMPAT_ADAPTER", adapter())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    let output = child.wait_with_output().unwrap();
    Outcome {
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.status.code().unwrap_or(-1),
    }
}

/// stderr as compared with the original's: exact, except that frames after the `Traceback` header are dropped.
fn stderr_projection(stderr: &str) -> String {
    match stderr.split_once(TRACEBACK) {
        None => stderr.to_string(),
        Some((head, body)) => {
            let last = body
                .lines()
                .rfind(|line| !line.trim().is_empty())
                .unwrap_or("");
            format!("{head}{TRACEBACK}  (native: Python stack frames are not reproduced)\n{last}\n")
        }
    }
}

// ── 103 API observations ────────────────────────────────────────────────────────

#[test]
fn api_observations_match_the_original() {
    let raw = std::fs::read_to_string(data_path("repeat_axis/api.jsonl")).unwrap();
    let records: Vec<PyDict> = raw
        .lines()
        .map(|line| as_dict(&loads(line).unwrap()).clone())
        .collect();
    assert_eq!(records.len(), 103);
    let mut input = String::new();
    for record in &records {
        let mut row = PyDict::new();
        for key in ["id", "operation", "arguments"] {
            row.set(key, record.get(key).unwrap().clone());
        }
        input.push_str(&spelled(&PyValue::Dict(row)));
        input.push('\n');
    }
    let outcome = run(
        API,
        &["evaluate-jsonl".into()],
        Path::new("."),
        None,
        &real_repo(),
        Some(&input),
    );
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    let output = String::from_utf8(outcome.stdout).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), records.len(), "one answer per record");
    for (record, line) in records.iter().zip(lines) {
        let answer = loads(line).unwrap();
        let answer = as_dict(&answer);
        let expected = as_dict(record.get("expected").unwrap());
        let id = text(record.get("id"));
        assert_eq!(text(answer.get("id")), id);
        assert_eq!(
            text(answer.get("kind")),
            text(expected.get("kind")),
            "{id}: {line}"
        );
        if text(expected.get("kind")) == "value" {
            assert_eq!(
                spelled(answer.get("value").unwrap()),
                spelled(expected.get("value").unwrap()),
                "{id}"
            );
        } else {
            assert_eq!(text(answer.get("type")), text(expected.get("type")), "{id}");
            assert_eq!(
                text(answer.get("message")),
                text(expected.get("message")),
                "{id}"
            );
        }
    }
}

// ── 42 filesystem CLI cases ─────────────────────────────────────────────────────

#[test]
fn filesystem_cli_cases_match_the_original() {
    let cases = match json_file("filesystem_cases.json") {
        PyValue::List(cases) => cases,
        other => panic!("not a list: {}", other.type_name()),
    };
    assert_eq!(cases.len(), 42);
    // The `_bootstrap` owner context: a repository tree with the real owner and no ACTIVE pointer.
    let context = owner_root("context");
    for case in &cases {
        let case = as_dict(case);
        let id = text(case.get("id"));
        let argv: Vec<String> = match case.get("argv") {
            Some(PyValue::List(items)) => items.iter().map(|item| text(Some(item))).collect(),
            _ => panic!("{id}: argv"),
        };
        let scratch = Scratch::new("fs");
        materialize(&scratch.0, &tree_of(as_dict(case.get("before").unwrap())));
        let outcome = run(CLI, &argv, &scratch.0, None, &context.0, None);
        let expected_stdout = text(case.get("stdout"));
        assert_eq!(
            String::from_utf8_lossy(&outcome.stdout),
            expected_stdout,
            "{id}: stdout"
        );
        assert_eq!(
            stderr_projection(&outcome.stderr),
            stderr_projection(&text(case.get("stderr"))),
            "{id}: stderr"
        );
        let expected_code = match case.get("exit_code") {
            Some(PyValue::Int(code)) => code.to_string(),
            other => panic!("{id}: {other:?}"),
        };
        assert_eq!(outcome.code.to_string(), expected_code, "{id}: exit");
        assert_eq!(
            snapshot(&scratch.0),
            tree_of(as_dict(case.get("after").unwrap())),
            "{id}: tree"
        );
    }
}

// ── 82 campaign-owner cases ─────────────────────────────────────────────────────

#[test]
fn campaign_declaration_cases_match_the_original() {
    let cases = match json_file("campaign_cases.json") {
        PyValue::List(cases) => cases,
        other => panic!("not a list: {}", other.type_name()),
    };
    assert_eq!(cases.len(), 82);
    let owner_source = std::fs::read(real_repo().join(OWNER_REL)).unwrap();
    for case in &cases {
        let case = as_dict(case);
        let id = text(case.get("id"));
        let scratch = Scratch::new("camp");
        if let Some(PyValue::List(directories)) = case.get("directories") {
            for directory in directories {
                std::fs::create_dir_all(scratch.0.join(text(Some(directory)))).unwrap();
            }
        }
        for (relative, encoded) in as_dict(case.get("files").unwrap()).iter() {
            let path = scratch.0.join(relative.to_utf8().unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, base64_decode(&text(Some(encoded)))).unwrap();
        }
        if matches!(case.get("owner_present"), Some(PyValue::Bool(true))) {
            let owner = scratch.0.join(OWNER_REL);
            std::fs::create_dir_all(owner.parent().unwrap()).unwrap();
            std::fs::write(owner, &owner_source).unwrap();
        }
        let before = snapshot(&scratch.0);
        let mut arguments = as_dict(case.get("arguments").unwrap()).clone();
        arguments.set("repo_root", PyValue::str(&scratch.0.to_string_lossy()));
        let mut row = PyDict::new();
        row.set("id", PyValue::str(&id));
        row.set("operation", case.get("operation").unwrap().clone());
        row.set("arguments", PyValue::Dict(arguments));
        let outcome = run(
            API,
            &["evaluate-jsonl".into()],
            Path::new("."),
            None,
            &real_repo(),
            Some(&format!("{}\n", spelled(&PyValue::Dict(row)))),
        );
        assert_eq!(outcome.code, 0, "{id}: {}", outcome.stderr);
        let answer = loads(String::from_utf8(outcome.stdout).unwrap().trim_end()).unwrap();
        let answer = as_dict(&answer);
        let expected = as_dict(case.get("expected").unwrap());
        assert_eq!(
            text(answer.get("kind")),
            text(expected.get("kind")),
            "{id}: {answer:?}"
        );
        if text(expected.get("kind")) == "value" {
            assert_eq!(
                spelled(answer.get("value").unwrap()),
                spelled(expected.get("value").unwrap()),
                "{id}"
            );
        } else {
            assert_eq!(text(answer.get("type")), text(expected.get("type")), "{id}");
            let message = text(answer.get("message"))
                .replace(&scratch.0.to_string_lossy().to_string(), "@ROOT@");
            assert_eq!(message, text(expected.get("message")), "{id}");
        }
        assert_eq!(
            snapshot(&scratch.0),
            before,
            "{id}: a query must not change the tree"
        );
    }
}

// ── 454 argparse goldens ────────────────────────────────────────────────────────

#[test]
fn argparse_edge_cases_match_the_original() {
    let raw = std::fs::read_to_string(data_path("repeat_axis/cli_goldens.jsonl")).unwrap();
    // `--s` (an abbreviation of `--self-test`) really runs the self-test, which reads the real repository.
    let repo = real_repo();
    let mut count = 0;
    for line in raw.lines() {
        let row = loads(line).unwrap();
        let row = as_dict(&row);
        let argv: Vec<String> = match row.get("argv") {
            Some(PyValue::List(items)) => items.iter().map(|item| text(Some(item))).collect(),
            _ => panic!("argv"),
        };
        let columns = match row.get("columns") {
            Some(PyValue::Str(columns)) => Some(columns.to_utf8().unwrap()),
            _ => None,
        };
        let scratch = Scratch::new("golden");
        let outcome = run(CLI, &argv, &scratch.0, columns.as_deref(), &repo, None);
        let label = format!("argv={:?} columns={columns:?}", &argv[..argv.len().min(8)]);
        assert_eq!(
            String::from_utf8_lossy(&outcome.stdout),
            text(row.get("stdout")),
            "{label}: stdout"
        );
        assert_eq!(
            stderr_projection(&outcome.stderr),
            text(row.get("stderr")),
            "{label}: stderr"
        );
        let expected = match row.get("exit_code") {
            Some(PyValue::Int(code)) => code.to_string(),
            other => panic!("{other:?}"),
        };
        assert_eq!(outcome.code.to_string(), expected, "{label}: exit");
        count += 1;
    }
    assert_eq!(count, 454);
}

// ── owner boundary: import errors, missing or odd attributes, raising functions ─

#[test]
fn owner_variants_behave_as_the_original_importing_them() {
    let raw =
        std::fs::read_to_string(data_path("repeat_axis/owner_variant_goldens.jsonl")).unwrap();
    let mut count = 0;
    for line in raw.lines() {
        let row = loads(line).unwrap();
        let row = as_dict(&row);
        let scratch = Scratch::new("variant");
        let owner = scratch.0.join(OWNER_REL);
        std::fs::create_dir_all(owner.parent().unwrap()).unwrap();
        std::fs::write(&owner, text(row.get("owner_source"))).unwrap();
        for (relative, encoded) in as_dict(row.get("files").unwrap()).iter() {
            let path = scratch.0.join(relative.to_utf8().unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, base64_decode(&text(Some(encoded)))).unwrap();
        }
        let argv: Vec<String> = match row.get("argv") {
            Some(PyValue::List(items)) => items.iter().map(|item| text(Some(item))).collect(),
            _ => panic!("argv"),
        };
        let label = format!(
            "{} {:?}",
            text(row.get("variant")),
            &argv[..3.min(argv.len())]
        );
        let outcome = run(CLI, &argv, &scratch.0, None, &scratch.0, None);
        let root = scratch.0.to_string_lossy().to_string();
        assert_eq!(
            String::from_utf8_lossy(&outcome.stdout).replace(&root, "@ROOT@"),
            text(row.get("stdout")),
            "{label}: stdout"
        );
        assert_eq!(
            stderr_projection(&outcome.stderr).replace(&root, "@ROOT@"),
            text(row.get("stderr")),
            "{label}: stderr"
        );
        let expected = match row.get("exit_code") {
            Some(PyValue::Int(code)) => code.to_string(),
            other => panic!("{other:?}"),
        };
        assert_eq!(outcome.code.to_string(), expected, "{label}: exit");
        count += 1;
    }
    assert_eq!(count, 60);
}

// ── constants ───────────────────────────────────────────────────────────────────

#[test]
fn native_constants_equal_the_original_modules() {
    let outcome = run(
        API,
        &["constants".into()],
        Path::new("."),
        None,
        &real_repo(),
        None,
    );
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    let native = loads(String::from_utf8(outcome.stdout).unwrap().trim_end()).unwrap();
    let golden = json_file("constants.json");
    // The module's constants are a set of names; the order of the capture is not a property of the original.
    let (native_map, golden_map) = (as_dict(&native), as_dict(&golden));
    assert_eq!(native_map.len(), golden_map.len(), "same constants");
    for (name, value) in golden_map.iter() {
        let own = native_map
            .get(&name.to_utf8().unwrap())
            .unwrap_or_else(|| panic!("constant {name:?} is missing"));
        assert_eq!(spelled(own), spelled(value), "{name:?}");
    }
    // The floor is the runtime schema's, not a copy: the schema of the real repository says the same.
    let schema = loads(&std::fs::read_to_string(real_repo().join(SCHEMA_REL)).unwrap()).unwrap();
    let minimum = as_dict(
        as_dict(
            as_dict(
                as_dict(as_dict(&schema).get("definitions").unwrap())
                    .get("budgets")
                    .unwrap(),
            )
            .get("properties")
            .unwrap(),
        )
        .get("repeats")
        .unwrap(),
    )
    .get("minimum")
    .unwrap()
    .clone();
    assert_eq!(
        spelled(as_dict(&native).get("FULL_REPEATS_MIN").unwrap()),
        spelled(&minimum)
    );
}

// ── own self-test ───────────────────────────────────────────────────────────────

#[test]
fn self_test_evaluates_its_checks_and_reproduces_the_original_transcript() {
    let scratch = Scratch::new("selftest");
    let outcome = run(
        CLI,
        &["--self-test".into()],
        &scratch.0,
        None,
        &real_repo(),
        None,
    );
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    let original =
        std::fs::read_to_string(data_path("repeat_axis/original-selftest.stdout")).unwrap();
    assert_eq!(String::from_utf8_lossy(&outcome.stdout), original);
    assert_eq!(outcome.stderr, "");
}

#[test]
fn self_test_fails_when_the_schema_fixture_is_missing() {
    let root = owner_root("noschema"); // owner present, no campaigns/_template schema
    let outcome = run(CLI, &["--self-test".into()], &root.0, None, &root.0, None);
    assert_eq!(outcome.code, 1);
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(
        stdout.starts_with("  [FAIL] A1 ★교차검증 campaign.schema.json budgets.repeats.minimum == FULL_REPEATS_MIN (schema=None · 상수=3)\n"),
        "{stdout}"
    );
    assert!(
        outcome
            .stderr
            .starts_with("[repeat_axis --self-test] FAIL 1 건: ['A1 "),
        "{}",
        outcome.stderr
    );
    assert!(!stdout.contains("전부 통과"));
}

#[test]
fn self_test_fails_when_the_owner_source_is_missing() {
    let root = Scratch::new("noowner");
    let schema = root.0.join(SCHEMA_REL);
    std::fs::create_dir_all(schema.parent().unwrap()).unwrap();
    std::fs::copy(real_repo().join(SCHEMA_REL), schema).unwrap();
    let outcome = run(CLI, &["--self-test".into()], &root.0, None, &root.0, None);
    assert_eq!(outcome.code, 1);
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(
        stdout.lines().last().unwrap().contains("P2 3·'5' 수용"),
        "{stdout}"
    );
    let last = outcome.stderr.lines().rfind(|l| !l.is_empty()).unwrap();
    assert!(
        last.starts_with("FileNotFoundError: [Errno 2] No such file or directory: '"),
        "{last}"
    );
    assert!(last.ends_with("campaign_init.py'"), "{last}");
}

// ── no original fallback; the pending owner is reported by name ─────────────────

#[test]
fn an_unreachable_owner_is_a_named_failure_and_never_the_original_script() {
    let root = owner_root("noadapter");
    // A different program (the reviewed owner would now answer natively without any adapter): it exists but, with no
    // adapter configured anywhere, it cannot be asked.
    std::fs::write(root.0.join(OWNER_REL), b"# a different program\n").unwrap();
    let argv: Vec<String> = ["resolve", "--preset-default", "3", "--preset-source", "p"]
        .map(String::from)
        .to_vec();
    let mut command = command(CLI, None, &argv);
    let output = command
        .current_dir(&root.0)
        .env("REPEAT_AXIS_REPO_ROOT", &root.0)
        .env_remove("REPEAT_AXIS_CAMPAIGN_ADAPTER")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CampaignOwnerAdapterError: the campaign_init owner exists at"),
        "{stderr}"
    );
    assert!(stderr.contains("REPEAT_AXIS_CAMPAIGN_ADAPTER"), "{stderr}");
    assert!(output.stdout.is_empty());
    // A configured adapter that cannot start is reported too.
    let output = command_with_broken_interpreter(&root.0, &argv);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("CampaignOwnerAdapterError: cannot start the campaign owner adapter"));
    // No owner file: the original's loud substitution needs no adapter at all.
    let empty = Scratch::new("emptyroot");
    let output = command_without_adapter(&empty.0, &argv);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "3\u{1f}defaulted(preset p 기본 3 · 캠페인 해소자 부재: .claude/skills/terraforming_node/scripts/campaign_init.py — 선언을 읽지 못했다)\u{1f}warm-rerun\n"
    );
}

fn command_with_broken_interpreter(root: &Path, argv: &[String]) -> std::process::Output {
    command(CLI, None, argv)
        .current_dir(root)
        .env("REPEAT_AXIS_REPO_ROOT", root)
        .env("REPEAT_AXIS_CAMPAIGN_ADAPTER", adapter())
        .env("REPEAT_AXIS_PYTHON", root.join("no-such-interpreter"))
        .output()
        .unwrap()
}

fn command_without_adapter(root: &Path, argv: &[String]) -> std::process::Output {
    command(CLI, None, argv)
        .current_dir(root)
        .env("REPEAT_AXIS_REPO_ROOT", root)
        .env_remove("REPEAT_AXIS_CAMPAIGN_ADAPTER")
        .output()
        .unwrap()
}

#[test]
fn a_missing_repository_root_is_a_named_refusal_only_where_it_is_needed() {
    let scratch = Scratch::new("noroot");
    let run_with = |argv: &[&str]| {
        let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        command(CLI, None, &argv)
            .current_dir(&scratch.0)
            .env_remove("REPEAT_AXIS_REPO_ROOT")
            .output()
            .unwrap()
    };
    // An explicit count never touches the root.
    assert_eq!(
        run_with(&["resolve", "--repeats", "3"]).status.code(),
        Some(0)
    );
    let output = run_with(&["resolve", "--preset-default", "3", "--preset-source", "p"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("RepeatAxisConfigurationError: the repository root is not configured"));
}

// ── R44: default routing of the campaign owner (own synthetic fixtures; not Root's recorded matrices) ─────────────────────

mod r44_routes {
    use super::{adapter, command, owner_root, Scratch, CLI, OWNER_REL};
    use std::path::Path;
    use std::process::Command;
    use sweep_stop::pystr::PyStr;
    use sweep_stop::pyvalue::{PyDict, PyValue};
    use sweep_stop::repeat_owner::{CampaignOwner, ConfiguredCampaignOwner, PythonCampaignOwner};

    fn argv() -> Vec<String> {
        ["resolve", "--preset-default", "3", "--preset-source", "p"]
            .map(String::from)
            .to_vec()
    }

    /// One CLI run in the default public configuration (only the wrapper's default adapter location) with a route observer;
    /// `configure` may add user intent.
    fn observed(
        root: &Path,
        configure: impl FnOnce(&mut Command),
    ) -> (std::process::Output, Vec<String>) {
        let observer = root.join("route.jsonl");
        let _ = std::fs::remove_file(&observer);
        let mut cmd = command(CLI, None, &argv());
        cmd.current_dir(root)
            .env("REPEAT_AXIS_REPO_ROOT", root)
            .env_remove("REPEAT_AXIS_CAMPAIGN_ADAPTER")
            .env_remove("REPEAT_AXIS_PYTHON")
            .env("REPEAT_AXIS_COMPAT_ADAPTER", adapter())
            .env("REPEAT_AXIS_ROUTE_OBSERVER", &observer);
        configure(&mut cmd);
        let output = cmd.output().unwrap();
        let lines = std::fs::read_to_string(&observer)
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect();
        (output, lines)
    }

    #[test]
    fn the_default_run_of_the_reviewed_owner_is_native_and_answers_what_the_adapter_answers() {
        let root = owner_root("r44_native");
        let (native, native_lines) = observed(&root.0, |_| {});
        assert_eq!(
            native.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            native_lines.len(),
            1,
            "one open, one line: {native_lines:?}"
        );
        let line = &native_lines[0];
        assert!(
            line.contains("\"route\": \"NativeCurrentSource\"")
                && line.contains("\"python_child\": false")
                && line.contains("\"child_pid\": null")
                && line.contains("\"opened\": true"),
            "{line}"
        );
        // user intent: the explicit adapter runs the same file through Python and answers byte for byte the same
        let (explicit, explicit_lines) = observed(&root.0, |cmd| {
            cmd.env("REPEAT_AXIS_CAMPAIGN_ADAPTER", adapter());
        });
        assert!(
            explicit_lines[0].contains("\"route\": \"LegacyExplicit\"")
                && !explicit_lines[0].contains("\"child_pid\": null"),
            "{explicit_lines:?}"
        );
        assert_eq!(native.stdout, explicit.stdout);
        // an explicit interpreter ALONE is explicit intent too (the default adapter location runs under it)
        let (python_only, python_lines) = observed(&root.0, |cmd| {
            cmd.env("REPEAT_AXIS_PYTHON", "python3");
        });
        assert!(
            python_lines[0].contains("\"route\": \"LegacyExplicit\""),
            "{python_lines:?}"
        );
        assert_eq!(native.stdout, python_only.stdout);
    }

    #[test]
    fn an_empty_explicit_interpreter_keeps_its_existing_failure() {
        let root = owner_root("r44_emptypy");
        let (output, lines) = observed(&root.0, |cmd| {
            cmd.env("REPEAT_AXIS_PYTHON", "");
        });
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("CampaignOwnerAdapterError: cannot start the campaign owner adapter"));
        assert!(
            lines[0].contains("\"route\": \"LegacyExplicit\"")
                && lines[0].contains("\"opened\": false"),
            "{lines:?}"
        );
    }

    #[test]
    fn a_different_or_unreadable_owner_runs_the_default_compat_adapter_and_bytes_decide() {
        let canonical = std::fs::read(super::real_repo().join(OWNER_REL)).unwrap();
        let mut variants: Vec<(&str, Vec<u8>)> = Vec::new();
        let mut commented = canonical.clone();
        commented.extend_from_slice(b"\n# one more byte of comment\n");
        variants.push(("comment", commented));
        let text = String::from_utf8(canonical.clone()).unwrap();
        variants.push((
            "bootstrap",
            text.replace("BOOTSTRAP = \"_bootstrap\"", "BOOTSTRAP = \"_boot2\"")
                .into_bytes(),
        ));
        let reference = {
            let root = owner_root("r44_ref");
            observed(&root.0, |_| {}).0.stdout
        };
        for (tag, bytes) in variants {
            let root = owner_root(&format!("r44_variant_{tag}"));
            std::fs::write(root.0.join(OWNER_REL), bytes).unwrap();
            let (output, lines) = observed(&root.0, |_| {});
            assert_eq!(
                output.status.code(),
                Some(0),
                "{tag}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                lines[0].contains("\"route\": \"LegacySourceVariant\"")
                    && lines[0].contains("\"python_child\": true")
                    && !lines[0].contains("\"child_pid\": null"),
                "{tag}: {lines:?}"
            );
            if tag == "comment" {
                assert_eq!(
                    output.stdout, reference,
                    "same apparent semantics, now answered by Python"
                );
            }
        }
        // unreadable (mode 000, uid != 0): stat succeeds, the bytes cannot be read => identity unknown => legacy compat
        if !is_root() {
            use std::os::unix::fs::PermissionsExt;
            let root = owner_root("r44_unreadable");
            let owner = root.0.join(OWNER_REL);
            std::fs::set_permissions(&owner, std::fs::Permissions::from_mode(0o000)).unwrap();
            let (output, lines) = observed(&root.0, |_| {});
            assert!(
                lines[0].contains("\"route\": \"LegacySourceVariant\"")
                    && lines[0].contains("cannot be read"),
                "{lines:?}"
            );
            assert_eq!(
                output.status.code(),
                Some(1),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("PermissionError"),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn is_root() -> bool {
        // uid 0 reads mode-000 files; the check needs a real permission failure
        std::fs::read_to_string("/proc/self/status")
            .unwrap()
            .lines()
            .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
    }

    #[test]
    fn an_absent_owner_is_decided_by_the_old_file_check_and_opens_no_program() {
        let empty = Scratch::new("r44_absent");
        let (output, lines) = observed(&empty.0, |_| {});
        assert_eq!(output.status.code(), Some(0));
        assert!(
            lines.len() == 1
                && lines[0].contains("\"route\": \"Absent\"")
                && lines[0].contains("\"opened\": false")
                && lines[0].contains("\"python_child\": false"),
            "{lines:?}"
        );
        // a directory at the owner path, and a non-directory parent: not regular files => absent as before
        let dir = Scratch::new("r44_dirowner");
        std::fs::create_dir_all(dir.0.join(OWNER_REL)).unwrap();
        let (_, lines) = observed(&dir.0, |_| {});
        assert!(lines[0].contains("\"route\": \"Absent\""), "{lines:?}");
        let blocked = Scratch::new("r44_notdir");
        std::fs::write(blocked.0.join(".claude"), b"a file").unwrap();
        let (output, lines) = observed(&blocked.0, |_| {});
        assert_eq!(output.status.code(), Some(0));
        assert!(lines[0].contains("\"route\": \"Absent\""), "{lines:?}");
    }

    #[test]
    fn a_requested_route_record_that_cannot_be_written_is_a_control_failure_not_a_route() {
        let root = owner_root("r44_observer_fail");
        std::fs::create_dir_all(root.0.join("route.jsonl")).unwrap();
        let mut cmd = command(CLI, None, &argv());
        let output = cmd
            .current_dir(&root.0)
            .env("REPEAT_AXIS_REPO_ROOT", &root.0)
            .env_remove("REPEAT_AXIS_CAMPAIGN_ADAPTER")
            .env_remove("REPEAT_AXIS_PYTHON")
            .env("REPEAT_AXIS_ROUTE_OBSERVER", root.0.join("route.jsonl"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("CampaignRouteObserverError"));
    }

    // ---- Native against the live adapter, on own fixtures: every answer and every failure must be equal -----------------

    fn native_owner() -> ConfiguredCampaignOwner {
        ConfiguredCampaignOwner::new(None, PythonCampaignOwner::new("python3", None))
    }

    fn adapter_owner() -> PythonCampaignOwner {
        PythonCampaignOwner::new("python3", Some(adapter().into_os_string()))
    }

    fn show(value: &PyValue) -> String {
        match value.peel() {
            PyValue::Str(text) => format!("s{:?}", text.code_points()),
            PyValue::List(items) => {
                format!("[{}]", items.iter().map(show).collect::<Vec<_>>().join(","))
            }
            other => format!("<{}>", other.type_name()),
        }
    }

    fn camp_ids() -> Vec<PyValue> {
        let text = |points: &[u32]| PyValue::Str(PyStr::from_code_points(points.to_vec()));
        let mut dict = PyDict::new();
        dict.set("k", PyValue::int(1));
        vec![
            PyValue::int(5),
            PyValue::int(0),
            PyValue::Bool(true),
            PyValue::Bool(false),
            PyValue::Float(1.5),
            PyValue::Float(0.0),
            text(&[]),
            PyValue::List(Vec::new()),
            PyValue::List(vec![PyValue::int(1)]),
            PyValue::Dict(PyDict::new()),
            PyValue::Dict(dict),
            PyValue::None,
            text(&[0x63, 0x61, 0x6d, 0x70, 0x2d, 0x78]),
            text(&[0x2e, 0x2e, 0x2f, 0x78]),
            text(&[0x2f, 0x61, 0x62, 0x73, 0x2f, 0x74]),
            text(&[0x2f, 0x2f, 0x61]),
            text(&[0xCA0, 0xD504]),
            text(&[0x61, 0, 0x62]),
            text(&[0xDCFF]),
            text(&[0xD800]),
        ]
    }

    /// Everything one session can be asked, rendered comparably.
    fn observe(owner: &dyn CampaignOwner, repo: &Path) -> Vec<String> {
        let repo =
            PyStr::from_surrogateescape(std::os::unix::ffi::OsStrExt::as_bytes(repo.as_os_str()));
        let mut seen = Vec::new();
        let mut session = match owner.open(&repo) {
            Ok(Some(session)) => session,
            Ok(None) => return vec!["absent".into()],
            Err(failure) => {
                return vec![format!(
                    "open-failed {} {:?}",
                    failure.class,
                    failure.message.code_points()
                )]
            }
        };
        let note = |r: Result<PyValue, sweep_stop::guidellm::Failure>| match r {
            Ok(value) => format!("ok {}", show(&value)),
            Err(failure) => format!("err {} {:?}", failure.class, failure.message.code_points()),
        };
        seen.push(note(session.active_campaign_id()));
        seen.push(note(session.bootstrap_id()));
        seen.push(note(session.reserved_ids()));
        for id in camp_ids() {
            seen.push(note(session.derive_root_path(&id).map(PyValue::Str)));
        }
        seen
    }

    fn fixture(tag: &str, active: Option<&[u8]>, dirs: &[&str]) -> Scratch {
        let root = owner_root(tag);
        for dir in dirs {
            std::fs::create_dir_all(root.0.join("campaigns").join(dir)).unwrap();
        }
        if let Some(bytes) = active {
            std::fs::create_dir_all(root.0.join("campaigns")).unwrap();
            std::fs::write(root.0.join("campaigns/ACTIVE"), bytes).unwrap();
        }
        root
    }

    #[test]
    fn native_and_the_live_adapter_answer_every_question_and_failure_alike() {
        use std::os::unix::fs::PermissionsExt;
        let mut fixtures: Vec<(&str, Scratch)> = vec![
            ("plain", fixture("r44d_plain", None, &[])),
            (
                "active_valid",
                fixture("r44d_valid", Some(b"camp-x\n"), &["camp-x"]),
            ),
            (
                "active_bootstrap",
                fixture("r44d_boot", Some(b"_bootstrap"), &["_bootstrap"]),
            ),
            (
                "active_missing_dir",
                fixture("r44d_gone", Some(b"gone\n"), &[]),
            ),
            ("active_empty", fixture("r44d_empty", Some(b""), &[])),
            (
                "active_whitespace_crlf",
                fixture("r44d_crlf", Some(b"  camp-x \r\n"), &["camp-x"]),
            ),
            (
                "active_invalid_utf8",
                fixture("r44d_utf8", Some(&[0xff, 0xfe, 0x0a]), &[]),
            ),
            (
                "active_dir_named_by_pointer_with_slash",
                fixture("r44d_slash", Some(b"a/b\n"), &["a/b"]),
            ),
        ];
        let unreadable = fixture("r44d_mode0", Some(b"camp-x\n"), &["camp-x"]);
        if !is_root() {
            std::fs::set_permissions(
                unreadable.0.join("campaigns/ACTIVE"),
                std::fs::Permissions::from_mode(0o000),
            )
            .unwrap();
            fixtures.push(("active_mode000", unreadable));
        }
        // the module's root is where the owner path resolves: a repo reached through a symlink, and an owner linked into
        // another checkout whose ACTIVE pointer differs from the caller's
        let real = fixture("r44d_real", Some(b"camp-x\n"), &["camp-x"]);
        let link = Scratch::new("r44d_linkparent");
        let alias = link.0.join("alias");
        std::os::unix::fs::symlink(&real.0, &alias).unwrap();
        let other = fixture("r44d_other", Some(b"camp-target\n"), &["camp-target"]);
        let caller = fixture("r44d_caller", Some(b"camp-caller\n"), &["camp-caller"]);
        std::fs::remove_file(caller.0.join(OWNER_REL)).unwrap();
        std::os::unix::fs::symlink(other.0.join(OWNER_REL), caller.0.join(OWNER_REL)).unwrap();
        let mut compared = 0;
        let mut check = |label: &str, repo: &Path| {
            // the Native route needs the reviewed bytes: through a link they are the linked file's, which is the same file
            let native = observe(&native_owner(), repo);
            let legacy = observe(&adapter_owner(), repo);
            assert_eq!(native, legacy, "{label}");
            assert!(
                native.len() > 3 || native[0].starts_with("open-failed"),
                "{label}: {native:?}"
            );
            compared += 1;
        };
        for (label, scratch) in &fixtures {
            check(label, &scratch.0);
        }
        check("repo_symlink", &alias);
        check("owner_symlink_to_other_root", &caller.0);
        assert_eq!(compared, fixtures.len() + 2);
    }
}
