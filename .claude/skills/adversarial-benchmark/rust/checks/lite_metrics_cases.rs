//! The 132 API operations, 306 further odd-input operations and 18 command-line runs captured from the original `lite_metrics.py` (head
//! `831068cb5777798cc8209374ecb42e0657d671a4`), replayed against this crate.
//!
//! API rows are `{id, operation, arguments, expected}`; the engine inputs are unit fixtures under
//! `checks/data/lite_metrics/inputs` (`@INPUTS@` in a row stands for that directory), not measurements. CLI cases
//! stage a complete before-tree, run the program with `COLUMNS=80` and compare exit status, stdout, stderr and the
//! complete after-tree (`@ROOT@` stands for the staged directory). The original builds `engine` from a set union,
//! so the order of its five keys differs between processes: that one object is compared unordered (also in
//! `--json` output); every other field, the table text and the formatting are compared in order and exactly.
//! A traceback is compared by what precedes it and its final `Class: message` line (frames are not reproduced).

#[path = "common.rs"]
mod common;

use common::{command, data_path, load_json_file, same};
use std::path::{Path, PathBuf};
use sweep_stop::lite_metrics_api_cli::evaluate_operation;
use sweep_stop::pyjson::{loads, loads_text};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn inputs_dir() -> String {
    data_path("lite_metrics/inputs")
        .to_str()
        .expect("UTF-8 path")
        .to_string()
}

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

/// `value` with the five-key `engine` object put in key order (the only unordered object of the contract).
fn engine_sorted(mut value: PyValue) -> PyValue {
    if let PyValue::Dict(map) = &mut value {
        if let Some(PyValue::Dict(engine)) = map.get("engine").cloned() {
            let mut entries: Vec<(PyStr, PyValue)> = engine.iter().cloned().collect();
            entries.sort_by_key(|(key, _)| key.to_utf8());
            let mut sorted = PyDict::new();
            for (key, entry) in entries {
                sorted.insert(key, entry);
            }
            map.set("engine", PyValue::Dict(sorted));
        }
    }
    value
}

/// The 132 frozen rows of the root contract.
#[test]
fn api_rows_equal_the_original() {
    check_api_rows("lite_metrics/api.jsonl", 132);
}

/// 306 further rows captured from the same original for odd operand types and values (`checks/data/lite_metrics`:
/// `odd_cases.jsonl`, written by the worker's capture script, not part of the root's frozen set): numeric text,
/// huge and negative integers, `NaN`/`inf`/`-0.0`, descriptors and non-path file arguments, engine logs with CRLF,
/// Unicode digits and invalid UTF-8, compensated float sums, exact integer division, and every wrong-type
/// exception those operations raise.
#[test]
fn odd_api_rows_equal_the_original() {
    check_api_rows("lite_metrics/odd_cases.jsonl", 306);
}

fn check_api_rows(file: &str, expected_rows: usize) {
    let source = std::fs::read_to_string(data_path(file)).expect("api rows");
    let rows: Vec<&str> = source.lines().collect();
    assert_eq!(rows.len(), expected_rows, "{file} row count");
    let inputs = inputs_dir();
    let mut mismatches = Vec::new();
    for line in rows {
        let PyValue::Dict(row) = loads(&line.replace("@INPUTS@", &inputs)).expect("row") else {
            panic!("row is not an object")
        };
        let (id, operation) = (text(get(&row, "id")), text(get(&row, "operation")));
        let PyValue::Dict(arguments) = get(&row, "arguments").clone() else {
            panic!("arguments are not an object")
        };
        let mut actual = PyDict::new();
        match evaluate_operation(&operation, &arguments) {
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
        let mut actual = loads(&wire).expect("round trip");
        let mut expected = get(&row, "expected").clone();
        if operation == "build" {
            let (PyValue::Dict(a), PyValue::Dict(e)) = (&mut actual, &mut expected) else {
                panic!()
            };
            if let (Some(value), Some(want)) = (a.get("value").cloned(), e.get("value").cloned()) {
                a.set("value", engine_sorted(value));
                e.set("value", engine_sorted(want));
            }
        }
        if !same(&actual, &expected) {
            mismatches.push(format!(
                "{id} {operation}: {} != {}",
                actual.repr(),
                expected.repr()
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

struct Tree(Vec<(String, Option<Vec<u8>>)>);

fn snapshot(root: &Path) -> Tree {
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
    Tree(out)
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

fn expected_tree(tree: &PyDict) -> Tree {
    let mut out: Vec<(String, Option<Vec<u8>>)> = tree
        .iter()
        .map(|(name, node)| {
            let PyValue::Dict(node) = node else {
                panic!("node")
            };
            let bytes = match text(get(node, "kind")).as_str() {
                "directory" => None,
                _ => Some(base64_decode(&text(get(node, "bytes_base64")))),
            };
            (name.to_utf8().expect("name"), bytes)
        })
        .collect();
    out.sort();
    Tree(out)
}

fn last_line(bytes: &[u8]) -> Vec<u8> {
    bytes
        .split(|&byte| byte == b'\n')
        .rfind(|line| !line.iter().all(u8::is_ascii_whitespace))
        .unwrap_or_default()
        .to_vec()
}

const TRACEBACK: &[u8] = b"Traceback (most recent call last):\n";

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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

/// `--json` output: same fields in order, `engine` unordered, and the canonical `indent=2` formatting.
fn json_stdout_equal(actual: &[u8], expected: &[u8]) -> bool {
    let parse = |bytes: &[u8]| {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|text| loads_text(&PyStr::new(text)).ok())
    };
    let (Some(a), Some(e)) = (parse(actual), parse(expected)) else {
        return false;
    };
    let canonical = |value: &PyValue| {
        let mut text = value
            .dumps_pretty()
            .expect("pretty")
            .to_utf8()
            .expect("UTF-8");
        text.push('\n');
        text.into_bytes()
    };
    if canonical(&a) != actual || canonical(&e) != expected {
        return false;
    }
    same(&engine_sorted(a), &engine_sorted(e))
}

#[test]
fn cli_cases_equal_the_original() {
    let cases = match load_json_file(&data_path("lite_metrics/cli_cases.json")) {
        PyValue::List(cases) => cases,
        other => panic!("cases are not a list: {}", other.type_name()),
    };
    assert_eq!(cases.len(), 18, "the frozen contract has 18 CLI cases");
    let bin = env!("CARGO_BIN_EXE_lite_metrics");
    let mut mismatches = Vec::new();
    for case in &cases {
        let PyValue::Dict(case) = case else {
            panic!("case")
        };
        let id = text(get(case, "id"));
        let root =
            std::env::temp_dir().join(format!("lite_metrics_case_{}_{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch");
        let root_text = root.to_str().expect("UTF-8 path").to_string();
        let PyValue::Dict(before) = get(case, "before") else {
            panic!("before")
        };
        for (name, node) in before.iter() {
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
        let json_run = argv.iter().any(|arg| arg == "--json") && output.status.code() == Some(0);
        let stdout_ok = if json_run {
            json_stdout_equal(&stdout, &want_stdout)
        } else {
            stdout == want_stdout
        };
        let checks = [
            (
                "exit",
                output.status.code()
                    == get(case, "exit_code")
                        .as_plain_int()
                        .and_then(|n| i32::try_from(n.clone()).ok()),
            ),
            ("stdout", stdout_ok),
            ("stderr", stderr_equal(&stderr, &want_stderr)),
            ("tree", snapshot(&root).0 == expected_tree(after).0),
        ];
        for (name, ok) in checks {
            if !ok {
                mismatches.push(format!("{id}: {name}"));
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
