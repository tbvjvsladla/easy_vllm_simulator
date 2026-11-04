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
        .env("REPEAT_AXIS_CAMPAIGN_ADAPTER", adapter())
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
    let argv: Vec<String> = ["resolve", "--preset-default", "3", "--preset-source", "p"]
        .map(String::from)
        .to_vec();
    // The adapter is not configured: the owner exists but cannot be asked.
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
