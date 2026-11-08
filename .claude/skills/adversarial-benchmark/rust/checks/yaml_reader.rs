//! The manifest YAML reader against what the real PyYAML 6.0.3 (`yaml.safe_load(open(path, encoding="utf-8"))`)
//! printed: for every document `repr` of the value, or `Class: message` of the exception, byte for byte.
//!
//! `yaml_reader_cases2.jsonl` adds `!!binary` scalars (1 400 random and 84 fixed base64 strings, 23 non-ASCII
//! ones: the message of `str.encode('ascii')` and `binascii.a2b_base64`'s non-strict rules) and 20 alias-sharing documents.
//! `yaml_reader_cases.jsonl` holds 204 hand-written documents (every construct the reader supports - anchors, merge
//! keys, tags, block and multi-line scalars, timestamps, directives, several documents, chunk boundaries of the reader
//! and of the text decoder, nesting limits - and every error family) and 1 500 documents combined from a fixed seed of
//! fragments. They go through the `yaml-load` transport of the `roofline` binary. The expected text contains the file
//! name PyYAML shows in its marks; `{DIR}` is the scratch directory the documents are written to.

#[path = "common.rs"]
mod common;

use common::{command, hex_decode, load_jsonl, text_of};
use std::process::Stdio;

#[test]
fn every_document_reads_like_pyyaml() {
    let mut rows = load_jsonl("roofline_verdict/yaml_reader_cases.jsonl");
    rows.extend(load_jsonl("roofline_verdict/yaml_reader_cases2.jsonl"));
    assert!(rows.len() >= 3208, "corpus shrank to {}", rows.len());
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("yaml_reader_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let mut failures = Vec::new();
    for row in &rows {
        let name = text_of(row.get("name"));
        let path = dir.join(format!("{name}.yaml"));
        std::fs::write(&path, hex_decode(&text_of(row.get("hex")))).expect("document");
        let output = command(
            env!("CARGO_BIN_EXE_roofline"),
            None,
            &["yaml-load".to_string(), path.to_str().unwrap().to_string()],
        )
        .stdin(Stdio::null())
        .output()
        .expect("run");
        let got = String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_string();
        let want = text_of(row.get("want")).replace("{DIR}", dir.to_str().unwrap());
        if got != want {
            failures.push(format!("{name}: want {want:?} got {got:?}"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} of {} documents differ:\n{}",
        failures.len(),
        rows.len(),
        failures[..failures.len().min(8)].join("\n")
    );
}
