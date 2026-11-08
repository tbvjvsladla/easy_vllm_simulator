//! The manifest readers beyond the frozen cases, through the `resolve_topology_api` binary: the incremental text reader
//! at its 8 KiB read boundaries (a character or a CRLF split across two reads, a bad byte at the start of the second
//! read, a value line that completes in the same read as the bad byte, an incomplete sequence at the end of the
//! file), the value syntax of the `topology:` line, and glob patterns in the repository path.
//!
//! The expectations at the boundaries were observed on CPython 3.12's own text iteration of the same byte layouts
//! (a private probe of the built-in `open()`, not of the resolver): an earlier read's lines are delivered before a
//! later read is decoded, and a read that holds a bad byte delivers none of its lines.

#[path = "resolve_topology_common.rs"]
mod common;

use common::*;
use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;

fn ask(scratch: &Scratch, operation: &str, argument: &str, value: &str) -> (String, String) {
    let row = format!(
        r#"{{"id":"x","operation":"{operation}","arguments":{{"{argument}":{}}}}}"#,
        PyValue::str(value).dumps_ascii().expect("row")
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_resolve_topology_api"))
        .arg("evaluate-jsonl")
        .current_dir(&scratch.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    writeln!(child.stdin.take().expect("stdin"), "{row}").expect("row");
    let output = child.wait_with_output().expect("output");
    let answer = loads(String::from_utf8(output.stdout).expect("ASCII").trim()).expect("JSON");
    let answer = dict(&answer);
    match text(field(answer, "kind")).as_str() {
        "value" => ("value".to_string(), key(field(answer, "value"))),
        _ => (text(field(answer, "type")), text(field(answer, "message"))),
    }
}

fn line_of(bytes: &[u8]) -> (String, String) {
    let scratch = Scratch::new("reader");
    std::fs::write(scratch.root.join("manifest.yaml"), bytes).expect("manifest");
    ask(
        &scratch,
        "_manifest_topology_line",
        "path",
        &format!("{}/manifest.yaml", scratch.text()),
    )
}

fn comment(length: usize) -> Vec<u8> {
    let mut line = vec![b'#'; length];
    line.push(b'\n');
    line
}

fn with(mut first: Vec<u8>, rest: &[u8]) -> Vec<u8> {
    first.extend_from_slice(rest);
    first
}

fn value(shown: &str) -> (String, String) {
    ("value".to_string(), shown.to_string())
}

fn exception(class: &str, message: &str) -> (String, String) {
    (class.to_string(), message.to_string())
}

#[test]
fn a_character_or_a_line_ending_split_across_two_reads_is_read_whole() {
    assert_eq!(
        line_of(&with(comment(8180), "topology: 한글\n".as_bytes())),
        value("\"\\ud55c\\uae00\"")
    );
    // the CR is the last byte of the first read, its LF the first of the second
    assert_eq!(
        line_of(&with(comment(8179), b"topology: x\r\nafter\n")),
        value("\"x\"")
    );
    // lone CR line ends
    assert_eq!(line_of(b"a: b\rtopology: y\r"), value("\"y\""));
}

#[test]
fn a_bad_byte_fails_only_the_read_that_holds_it() {
    // in the second read, with the value already delivered by the first
    assert_eq!(
        line_of(&with(
            b"topology: x\n".to_vec(),
            &[&[b'#'; 8200][..], b"\xff\n"].concat()
        )),
        value("\"x\"")
    );
    // at the very start of the second read, before any value
    assert_eq!(
        line_of(&with(comment(8191), b"\xff\ntopology: x\n")),
        exception(
            "UnicodeDecodeError",
            "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"
        )
    );
    // the value line completes only in the second read, which holds the bad byte: the line is lost with its read
    assert_eq!(
        line_of(&with(
            comment(8170),
            b"topology: abcdefghijklmnopqrstuvwxyz\n\xff\n"
        )),
        exception(
            "UnicodeDecodeError",
            "'utf-8' codec can't decode byte 0xff in position 16: invalid start byte"
        )
    );
}

#[test]
fn an_incomplete_sequence_at_the_end_of_the_file_is_an_error_only_when_no_value_came_first() {
    assert_eq!(line_of(b"topology: x\n\xe2"), value("\"x\""));
    assert_eq!(
        line_of(b"a: b\n\xe2\x82"),
        exception(
            "UnicodeDecodeError",
            "'utf-8' codec can't decode bytes in position 0-1: unexpected end of data"
        )
    );
    assert_eq!(line_of(b""), value("null"));
    assert_eq!(line_of(b"a: b"), value("null"));
}

#[test]
fn the_value_is_what_follows_the_first_colon_up_to_the_first_hash_without_white_space_or_quotes() {
    for (file, shown) in [
        ("topology:a:b\n", "\"a:b\""),
        ("topology: x#y\n", "\"x\""),
        ("topology:#\n", "null"),
        ("topology: ''\n", "null"),
        ("topology: \"\"\"\"\n", "null"),
        ("topology: 'a\"b'\n", "\"a\\\"b\""),
        ("topology:\u{a0}x\u{a0}\n", "\"x\""),
        ("topology: \u{200b}x\n", "\"\\u200bx\""),
        ("topology:\t single \t\n", "\"single\""),
        ("topology: 'x\n", "\"x\""),
        ("topology:\n", "null"),
        ("topology", "null"),
        ("Topology: x\n", "null"),
    ] {
        assert_eq!(line_of(file.as_bytes()), value(shown), "{file:?}");
    }
}

#[test]
fn glob_characters_in_the_repository_path_are_patterns_as_in_the_original() {
    let scratch = Scratch::new("glob");
    for (directory, channel) in [("a[b]c", "single"), ("abc", "other")] {
        let folder = scratch.root.join(directory).join("output").join(channel);
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("manifest.yaml"), "topology: x\n").expect("manifest");
    }
    let pattern = format!("{}/a[b]c", scratch.text());
    // the pattern `a[b]c` matches the folder `abc` and not the folder named with the brackets
    assert_eq!(
        ask(&scratch, "_manifests", "repo", &pattern),
        value(&format!(
            "[\"{}/abc/output/other/manifest.yaml\"]",
            scratch.text()
        ))
    );
    assert_eq!(
        ask(&scratch, "_manifests", "repo", &format!("{pattern}/")),
        value(&format!(
            "[\"{}/abc/output/other/manifest.yaml\"]",
            scratch.text()
        ))
    );
}

#[test]
fn a_path_that_is_not_text_is_the_open_error_and_a_nul_is_a_value_error() {
    let scratch = Scratch::new("shapes");
    assert_eq!(
        ask(&scratch, "_manifest_topology_line", "path", "a\u{0}b"),
        exception("ValueError", "embedded null byte")
    );
    // a directory and a missing file are OSErrors: no value
    assert_eq!(
        ask(&scratch, "_manifest_topology_line", "path", &scratch.text()),
        value("null")
    );
    assert_eq!(
        ask(
            &scratch,
            "_manifest_topology_line",
            "path",
            "/nonexistent-topology-file"
        ),
        value("null")
    );
}

/// Runs one `_manifest_topology_line` row in an API child whose address space is limited to 96 MiB and whose core
/// dumps are disabled (the shell's `ulimit` of this child only; nothing global changes). Returns the status, the
/// answer line and the standard error text.
fn limited_ask(scratch: &Scratch, path: &str) -> (Option<i32>, String, String) {
    let row = format!(
        r#"{{"id":"x","operation":"_manifest_topology_line","arguments":{{"path":{}}}}}"#,
        PyValue::str(path).dumps_ascii().expect("row")
    );
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"ulimit -c 0 && ulimit -v 98304 && exec "$0" evaluate-jsonl"#)
        .arg(env!("CARGO_BIN_EXE_resolve_topology_api"))
        .current_dir(&scratch.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    writeln!(child.stdin.take().expect("stdin"), "{row}").expect("row");
    let output = child.wait_with_output().expect("output");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The reader holds the lines of the read it is examining, not the lines already passed (`for line in handle`
/// discards each line once examined). 1,638,400 short lines (18,022,400 bytes) with no `topology:` key take far more
/// than 96 MiB if every completed line is retained; read incrementally they need a block and the unfinished line.
#[test]
fn many_short_lines_are_read_in_memory_bounded_by_the_block_not_by_the_file() {
    let scratch = Scratch::new("reader-memory");
    let path = scratch.root.join("manifest.yaml");
    let body: String = (0..1_638_400).map(|_| "key: value\n").collect();
    assert_eq!(body.len(), 18_022_400);
    std::fs::write(&path, &body).expect("manifest");
    let shown = path.to_string_lossy().into_owned();
    let (status, answer, stderr) = limited_ask(&scratch, &shown);
    assert_eq!(status, Some(0), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert_eq!(answer, r#"{"id": "x", "kind": "value", "value": null}"#);
    // positive control under the same limit: a value after all those lines is still found, and an empty one stops the read
    std::fs::write(&path, format!("{body}topology: multi\ntopology: single\n")).expect("manifest");
    let (status, answer, stderr) = limited_ask(&scratch, &shown);
    assert_eq!(status, Some(0), "{stderr}");
    assert_eq!(answer, r#"{"id": "x", "kind": "value", "value": "multi"}"#);
    std::fs::write(&path, format!("{body}topology: # none\ntopology: single\n")).expect("manifest");
    let (_, answer, _) = limited_ask(&scratch, &shown);
    assert_eq!(answer, r#"{"id": "x", "kind": "value", "value": null}"#);
}
