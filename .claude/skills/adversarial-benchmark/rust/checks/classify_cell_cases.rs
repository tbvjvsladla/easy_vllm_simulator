//! The frozen originals of `classify_cell.py` (head `831068cb5777798cc8209374ecb42e0657d671a4`) replayed against this
//! crate: the 121 pure API rows, the 45 owner/record/FS rows with the 8 durable-record trees, the 20 command-line
//! cases (17 + 3 valid bench-mode runs) with complete before/after trees, and the original's own `--self-test`
//! transcript. Data lives in `checks/data/classify_cell` (`@REF@` in a row is the staged reference tree, `@ROOT@` the
//! staged case directory).
//!
//! The rows that ask the not-yet-ported owners (`node_identity.sh`, `node_role_contract.py`) need the installed owners:
//! `CLASSIFY_CELL_SCRIPTS_DIR` (the scripts directory the original found from its own file) and
//! `CLASSIFY_CELL_ROLE_ADAPTER`. A missing variable fails the check (`NOT_RUN`), it never skips a row.
//!
//! A traceback is compared by what precedes it and its final `Class: message` line (frames are not reproduced).

#[path = "common.rs"]
mod common;

use common::{command, data_path, load_json_file, same};
use std::path::{Path, PathBuf};
use sweep_stop::classify_cell::Diagnostics;
use sweep_stop::classify_cell_api_cli::evaluate_operation;
use sweep_stop::classify_cell_owner::{Owners, ROLE_ADAPTER_ENV, SCRIPTS_DIR_ENV};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn base64_decode(text: &str) -> Vec<u8> {
    let value = |byte: u8| match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        other => panic!("not base64: {other}"),
    };
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for byte in text.bytes().take_while(|&byte| byte != b'=') {
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

fn get<'a>(map: &'a PyDict, key: &str) -> &'a PyValue {
    map.get(key)
        .unwrap_or_else(|| panic!("record has no {key}"))
}

fn text(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("UTF-8 text"),
        other => panic!("not a string: {}", other.repr()),
    }
}

fn replace_bytes(haystack: &[u8], needle: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut at = 0;
    while at < haystack.len() {
        if haystack[at..].starts_with(needle) {
            out.extend_from_slice(with);
            at += needle.len();
        } else {
            out.push(haystack[at]);
            at += 1;
        }
    }
    out
}

/// The installed owner configuration; absent variables fail the check.
fn installed_owners() -> Owners {
    for name in [SCRIPTS_DIR_ENV, ROLE_ADAPTER_ENV] {
        assert!(
            std::env::var_os(name).is_some_and(|value| !value.is_empty()),
            "NOT_RUN: {name} is not set; the owner rows need the installed owners (no row is skipped)"
        );
    }
    Owners::from_environment()
}

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("classify_cell_cases")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// A directory tree as `(relative path, None for a directory | Some(bytes))`, sorted, with the root's path replaced.
fn snapshot(root: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    fn walk(root: &Path, directory: &Path, out: &mut Vec<(String, Option<Vec<u8>>)>) {
        let mut names: Vec<PathBuf> = std::fs::read_dir(directory)
            .expect("list")
            .map(|entry| entry.expect("entry").path())
            .collect();
        names.sort();
        for path in names {
            let relative = path
                .strip_prefix(root)
                .expect("inside")
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                out.push((relative, None));
                walk(root, &path, out);
            } else {
                let bytes = std::fs::read(&path).expect("read");
                let normalised =
                    replace_bytes(&bytes, root.to_str().expect("UTF-8").as_bytes(), b"@ROOT@");
                out.push((relative, Some(normalised)));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// A tree description `{name: {kind, bytes_base64}}` as a sorted list.
fn described_tree(tree: &PyDict) -> Vec<(String, Option<Vec<u8>>)> {
    let mut out: Vec<(String, Option<Vec<u8>>)> = tree
        .iter()
        .map(|(name, node)| {
            let PyValue::Dict(node) = node else {
                panic!("node")
            };
            let bytes = match text(get(node, "kind")).as_str() {
                "directory" => None,
                "file" => Some(base64_decode(&text(get(node, "bytes_base64")))),
                other => panic!("unsupported node kind {other}"),
            };
            (name.to_utf8().expect("name"), bytes)
        })
        .collect();
    out.sort();
    out
}

fn stage_tree(root: &Path, tree: &PyDict) {
    std::fs::create_dir_all(root).expect("directory");
    let root_text = root.to_str().expect("UTF-8 path").to_string();
    for (name, node) in tree.iter() {
        let PyValue::Dict(node) = node else {
            panic!("node")
        };
        let path = root.join(name.to_utf8().expect("name"));
        if text(get(node, "kind")) == "directory" {
            std::fs::create_dir_all(&path).expect("directory");
        } else {
            let bytes = replace_bytes(
                &base64_decode(&text(get(node, "bytes_base64"))),
                b"@ROOT@",
                root_text.as_bytes(),
            );
            std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
            std::fs::write(&path, bytes).expect("stage file");
        }
    }
}

fn evaluate_rows(
    file: &str,
    owners: &Owners,
    reference: Option<&str>,
) -> Vec<(String, PyDict, bool)> {
    let source = std::fs::read_to_string(data_path(file)).expect("api rows");
    let mut results = Vec::new();
    for line in source.lines() {
        let line = match reference {
            Some(root) => line.replace("@REF@", &root.replace('\\', "\\\\")),
            None => line.to_string(),
        };
        let PyValue::Dict(row) = loads(&line).expect("row") else {
            panic!("row is not an object")
        };
        let (id, operation) = (text(get(&row, "id")), text(get(&row, "operation")));
        let PyValue::Dict(arguments) = get(&row, "arguments").clone() else {
            panic!("arguments are not an object")
        };
        let mut diagnostics = Diagnostics::default();
        let mut actual = PyDict::new();
        match evaluate_operation(&operation, &arguments, owners, &mut diagnostics) {
            Ok(value) => {
                actual.set("kind", PyValue::str("value"));
                actual.set("value", value);
            }
            Err(failure) => {
                actual.set("kind", PyValue::str("exception"));
                actual.set("type", PyValue::str(failure.class));
                actual.set("message", PyValue::Str(failure.message));
            }
        }
        // The wire is JSON text; tuples are lists there.
        let wire = PyValue::Dict(actual).dumps_ascii().expect("serialisable");
        let actual = loads(&wire).expect("round trip");
        let expected = get(&row, "expected").clone();
        let quiet = diagnostics.stderr.is_empty();
        let PyValue::Dict(actual) = actual else {
            panic!()
        };
        let mut compare = PyDict::new();
        compare.set("id", PyValue::str(&id));
        for (key, value) in actual.iter() {
            compare.set(&key.to_utf8().expect("key"), value.clone());
        }
        let mut want = PyDict::new();
        want.set("id", PyValue::str(&id));
        let PyValue::Dict(expected) = expected else {
            panic!()
        };
        for (key, value) in expected.iter() {
            want.set(&key.to_utf8().expect("key"), value.clone());
        }
        results.push((
            format!("{id} {operation}"),
            compare.clone(),
            same(&PyValue::Dict(compare), &PyValue::Dict(want)) && quiet,
        ));
    }
    results
}

/// The 121 frozen rows of the pure functions (`classify`, `declared_lite_record`, `bench_mode_kind`,
/// `events_scan_summary`): complete values and the exceptions of the original, nothing from the owners.
#[test]
fn pure_rows_equal_the_original() {
    let owners = Owners::new(None, "python3", None);
    let results = evaluate_rows("classify_cell/pure_api.jsonl", &owners, None);
    assert_eq!(results.len(), 121, "the frozen contract has 121 pure rows");
    let failed: Vec<String> = results
        .iter()
        .filter(|(_, _, passed)| !passed)
        .map(|(label, actual, _)| format!("{label}: {}", PyValue::Dict(actual.clone()).repr()))
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The 45 rows that read manifests, events and records (asking the real owners) and the two record writers, with the
/// complete trees of the eight durable-record directories before and after.
#[test]
fn owner_rows_and_records_equal_the_original() {
    let owners = installed_owners();
    let data = load_json_file(&data_path("classify_cell/owner_trees.json"));
    let PyValue::Dict(data) = data else {
        panic!("owner_trees.json is not an object")
    };
    let reference = scratch("owner");
    let PyValue::Dict(owner_tree) = get(&data, "owner_tree") else {
        panic!("owner_tree")
    };
    stage_tree(&reference.join("owner-tree"), owner_tree);
    let PyValue::List(records) = get(&data, "records") else {
        panic!("records")
    };
    for record in records {
        let PyValue::Dict(record) = record else {
            panic!("record")
        };
        let PyValue::Dict(before) = get(record, "before") else {
            panic!("before")
        };
        stage_tree(&reference.join(text(get(record, "dir"))), before);
    }
    let tree_before = snapshot(&reference.join("owner-tree"));
    let reference_text = reference.to_str().expect("UTF-8 path").to_string();
    let results = evaluate_rows(
        "classify_cell/owner_api.jsonl",
        &owners,
        Some(&reference_text),
    );
    assert_eq!(results.len(), 45, "the frozen contract has 45 owner rows");
    let mut failed: Vec<String> = results
        .iter()
        .filter(|(_, _, passed)| !passed)
        .map(|(label, actual, _)| format!("{label}: {}", PyValue::Dict(actual.clone()).repr()))
        .collect();
    assert_eq!(
        snapshot(&reference.join("owner-tree")),
        tree_before,
        "the planning and scanning rows must not change the repository tree"
    );
    for record in records {
        let PyValue::Dict(record) = record else {
            panic!()
        };
        let PyValue::Dict(after) = get(record, "after") else {
            panic!("after")
        };
        let directory = reference.join(text(get(record, "dir")));
        if snapshot(&directory) != described_tree(after) {
            failed.push(format!(
                "{}: the tree of {} differs from the original",
                text(get(record, "id")),
                text(get(record, "dir"))
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

const TRACEBACK: &[u8] = b"Traceback (most recent call last):\n";

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn last_line(bytes: &[u8]) -> Vec<u8> {
    bytes
        .split(|&byte| byte == b'\n')
        .rfind(|line| !line.iter().all(u8::is_ascii_whitespace))
        .unwrap_or_default()
        .to_vec()
}

fn stderr_equal(actual: &[u8], expected: &[u8]) -> bool {
    match find(expected, TRACEBACK) {
        Some(at) => {
            find(actual, TRACEBACK) == Some(at)
                && actual[..at] == expected[..at]
                && last_line(actual) == last_line(expected)
        }
        None => actual == expected,
    }
}

/// The 17 frozen command-line runs and the 3 valid bench-mode runs: exit status, stdout, stderr and the complete
/// after-tree of the staged directory (the program runs with `COLUMNS=80`).
#[test]
fn cli_cases_equal_the_original() {
    let owners = installed_owners();
    let _ = owners;
    let cases = match load_json_file(&data_path("classify_cell/cli_cases.json")) {
        PyValue::List(cases) => cases,
        other => panic!("cases are not a list: {}", other.type_name()),
    };
    assert_eq!(cases.len(), 20, "the frozen contract has 17 + 3 CLI cases");
    let bin = env!("CARGO_BIN_EXE_classify_cell");
    let mut mismatches = Vec::new();
    for case in &cases {
        let PyValue::Dict(case) = case else {
            panic!("case")
        };
        let id = text(get(case, "id"));
        let root = scratch(&format!("cli_{id}"));
        let root_text = root.to_str().expect("UTF-8 path").to_string();
        let PyValue::Dict(before) = get(case, "before") else {
            panic!("before")
        };
        stage_tree(&root, before);
        let PyValue::List(argv) = get(case, "argv") else {
            panic!("argv")
        };
        let argv: Vec<String> = argv
            .iter()
            .map(|arg| text(arg).replace("@ROOT@", &root_text))
            .collect();
        let output = command(bin, Some("80"), &argv)
            .current_dir(&root)
            .output()
            .expect("run");
        let normalise = |bytes: &[u8]| replace_bytes(bytes, root_text.as_bytes(), b"@ROOT@");
        let (stdout, stderr) = (normalise(&output.stdout), normalise(&output.stderr));
        let want_stdout = text(get(case, "stdout")).into_bytes();
        let want_stderr = text(get(case, "stderr")).into_bytes();
        let PyValue::Dict(after) = get(case, "after") else {
            panic!("after")
        };
        let checks = [
            (
                "exit",
                output.status.code()
                    == get(case, "exit_code")
                        .as_plain_int()
                        .and_then(|number| i32::try_from(number.clone()).ok()),
            ),
            ("stdout", stdout == want_stdout),
            ("stderr", stderr_equal(&stderr, &want_stderr)),
            ("tree", snapshot(&root) == described_tree(after)),
        ];
        for (what, ok) in checks {
            if !ok {
                mismatches.push(format!("{id}: {what} differs from the original"));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// The original's `--self-test` transcript, reproduced by the program's own run of its checks against the installed
/// owners: every line, the `OK` summary, status 0, and no `[SKIP]` (the installed tree has `fetch_sub_docs.sh`).
#[test]
fn self_test_transcript_equals_the_original() {
    let _ = installed_owners();
    let bin = env!("CARGO_BIN_EXE_classify_cell");
    let output = command(bin, Some("80"), &["--self-test".to_string()])
        .output()
        .expect("run");
    let expected =
        std::fs::read(data_path("classify_cell/selftest_original.stdout")).expect("transcript");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("[SKIP]"),
        "a check was skipped"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&expected)
    );
}

/// Two valid multi-node manifests whose unused metadata (an integer key too large for a float; a self-referencing
/// anchor) once made the provisional role-owner transport refuse them, while the original returns a valid plan
/// (originals recorded by the root's review, `role_transport_cases.json`). The third row is a 5000-digit decimal VALUE:
/// the owner's YAML parser refuses it under the interpreter's inherited limit, and so does the plan - the adapter must
/// not change that process-wide setting. The real owner and the query adapter run.
#[test]
fn valid_manifests_with_unread_metadata_equal_the_original() {
    let owners = installed_owners();
    let cases = match load_json_file(&data_path("classify_cell/role_transport_cases.json")) {
        PyValue::List(cases) => cases,
        other => panic!("cases are not a list: {}", other.type_name()),
    };
    assert_eq!(
        cases.len(),
        3,
        "two unread-metadata rows and the inherited-digit-limit row"
    );
    let mut failed = Vec::new();
    for case in &cases {
        let PyValue::Dict(case) = case else {
            panic!("case")
        };
        let id = text(get(case, "id"));
        let root = scratch(&format!("role_{id}"));
        let root_text = root.to_str().expect("UTF-8 path").to_string();
        let manifest = root.join("output/multi/manifest.yaml");
        std::fs::create_dir_all(manifest.parent().expect("parent")).expect("directory");
        std::fs::write(&manifest, text(get(case, "manifest"))).expect("manifest");
        let PyValue::Dict(arguments) = get(case, "arguments").clone() else {
            panic!("arguments")
        };
        let mut staged = PyDict::new();
        for (name, value) in arguments.iter() {
            let value = match value {
                PyValue::Str(path) => {
                    PyValue::str(&path.to_utf8().expect("text").replace("@ROOT@", &root_text))
                }
                other => other.clone(),
            };
            staged.insert(name.clone(), value);
        }
        let mut diagnostics = Diagnostics::default();
        let value = evaluate_operation(
            &text(get(case, "operation")),
            &staged,
            &owners,
            &mut diagnostics,
        )
        .unwrap_or_else(|failure| panic!("{id}: {}", failure.line().to_utf8().expect("text")));
        let PyValue::Dict(expected) = get(case, "expected") else {
            panic!("expected")
        };
        let wanted = loads(
            &PyValue::Dict(expected.clone())
                .dumps_ascii()
                .expect("json")
                .replace("@ROOT@", &root_text.replace('\\', "\\\\")),
        )
        .expect("expected");
        let PyValue::Dict(wanted) = wanted else {
            panic!()
        };
        if !same(&value, get(&wanted, "value")) {
            failed.push(format!("{id}: {}", value.repr()));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
