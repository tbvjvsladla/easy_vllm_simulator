//! Shared helpers for the integration checks (included with `#[path]`; not a test target itself).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use sweep_stop::pyjson::{decode_file_text, loads_text};
use sweep_stop::pyvalue::{PyDict, PyValue};

/// Directory holding the externally captured reference (`cases.json`, `expected.json`, `cli-reference.json`).
/// A missing directory fails the check: an absent oracle must never read as a pass.
pub fn oracle_dir() -> PathBuf {
    let dir = std::env::var_os("SWEEP_ORACLE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_path("reference"));
    assert!(
        dir.join("cases.json").is_file(),
        "NOT_RUN: reference data missing in {}",
        dir.display()
    );
    dir
}

pub fn data_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join(name)
}

pub fn load_json_file(path: &Path) -> PyValue {
    let bytes =
        std::fs::read(path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    let text = decode_file_text(&bytes).expect("reference files are UTF-8");
    loads_text(&text).unwrap_or_else(|err| panic!("{} is not valid JSON: {err}", path.display()))
}

pub fn load_jsonl(name: &str) -> Vec<PyDict> {
    let path = data_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    let records: Vec<PyDict> = text
        .lines()
        .map(
            |line| match sweep_stop::pyjson::loads(line).expect("data line is JSON") {
                PyValue::Dict(record) => record,
                other => panic!("data line is not an object: {}", other.repr()),
            },
        )
        .collect();
    assert!(!records.is_empty(), "{name} is empty");
    records
}

pub fn list(value: PyValue) -> Vec<PyValue> {
    match value {
        PyValue::List(items) => items,
        other => panic!("expected a JSON array, got {}", other.type_name()),
    }
}

pub fn dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("expected a JSON object, got {}", other.type_name()),
    }
}

pub fn text_of(value: Option<&PyValue>) -> String {
    match value {
        Some(PyValue::Str(text)) => text.to_utf8().expect("reference text is encodable"),
        other => panic!("expected a string field, got {other:?}"),
    }
}

pub fn hex_decode(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "hex text has an even length");
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex digits"))
        .collect()
}
