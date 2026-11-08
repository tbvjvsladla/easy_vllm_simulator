//! Observations of the ORIGINAL `render_sweep_map.py` on the two things its `json.dump(..., indent=2)` makes
//! visible: the interpreter's frame limit and chunk-by-chunk writing. The sizes below are the original's own
//! outputs for the same inputs (captured before this port existed); the inputs are built here, generically.
//!
//! * Nested `coordinates` of depth 992 are written whole, depth 993 and 994 leave the first 1 000 545 bytes of the
//!   JSON file before failing with `RecursionError`, depth 995 fails in `assert_no_ranking` before either output
//!   file is touched.
//! * A lone surrogate in a string fails at that chunk with an error position local to the chunk; the file keeps
//!   every earlier chunk, is the same file (inode, symbolic link) and the Markdown file is complete.

#[path = "reporting_common.rs"]
mod common;

use common::*;
use std::os::unix::fs::MetadataExt;
use std::process::Command;

struct Run {
    status: Option<i32>,
    stderr_last: String,
    markdown: Vec<u8>,
    json: Vec<u8>,
}

fn run_map(scratch: &Scratch, state: &str, markdown_before: &[u8], json_before: &[u8]) -> Run {
    let root = &scratch.root;
    std::fs::write(root.join("state.json"), state).unwrap();
    std::fs::write(root.join("stop.json"), "{}").unwrap();
    std::fs::write(root.join("map.md"), markdown_before).unwrap();
    std::fs::write(root.join("map.json"), json_before).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_render_sweep_map"))
        .current_dir(root)
        .args([
            "--state",
            "state.json",
            "--stop-json",
            "stop.json",
            "--out-md",
            "map.md",
            "--out-json",
            "map.json",
        ])
        .output()
        .expect("run");
    Run {
        status: output.status.code(),
        stderr_last: String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .lines()
            .last()
            .unwrap_or("")
            .to_string(),
        markdown: std::fs::read(root.join("map.md")).unwrap(),
        json: std::fs::read(root.join("map.json")).unwrap(),
    }
}

fn nested_state(depth: usize) -> String {
    format!(
        r#"{{"cells":[{{"cell_key":"c","coordinates":{}{}{}}}]}}"#,
        r#"{"x":"#.repeat(depth),
        "{}",
        "}".repeat(depth)
    )
}

const SURROGATE_STATE: &str = r#"{"sweep_id": "independent-map", "cells": [{"cell_key": "독립-cell", "coordinates": {"model": "m"}, "cell_outcome": "measured", "axis_citation": "original-owner-observation", "bench_mode": "full", "downgrade_reason": null, "downgrade_reason_source": "before\ud800after"}]}"#;
const SURROGATE_STOP: &str = r#"{"sweep_status": "incomplete", "stopped_by": ["budget"], "cells_planned": 2, "cells_attempted": 1, "cells_remaining": ["later"], "declared_budget": {"repeats": 3}}"#;

const RECURSION: &str = "RecursionError: maximum recursion depth exceeded";

#[test]
fn nesting_within_the_frame_limit_is_written_whole() {
    let scratch = Scratch::new("depth992");
    let run = run_map(&scratch, &nested_state(992), b"", b"");
    assert_eq!(run.status, Some(0));
    assert_eq!((run.markdown.len(), run.json.len()), (8093, 1_990_257));
}

#[test]
fn nesting_past_the_json_frame_limit_keeps_the_chunks_written_before_it() {
    for (depth, markdown_bytes) in [(993, 8100), (994, 8107)] {
        let scratch = Scratch::new("depth99x");
        let run = run_map(&scratch, &nested_state(depth), b"", b"");
        assert_eq!(run.status, Some(1), "depth {depth}");
        assert_eq!(run.stderr_last, RECURSION, "depth {depth}");
        assert_eq!(run.markdown.len(), markdown_bytes, "depth {depth}");
        assert_eq!(run.json.len(), 1_000_545, "depth {depth}");
    }
}

#[test]
fn nesting_past_the_ranking_check_limit_touches_no_output() {
    for depth in [995, 996, 997] {
        let scratch = Scratch::new("depth995");
        let run = run_map(
            &scratch,
            &nested_state(depth),
            b"before-md",
            b"before-json-content",
        );
        assert_eq!(run.status, Some(1), "depth {depth}");
        assert_eq!(run.stderr_last, RECURSION, "depth {depth}");
        assert_eq!(run.markdown, b"before-md", "depth {depth}");
        assert_eq!(run.json, b"before-json-content", "depth {depth}");
    }
}

#[test]
fn a_surrogate_chunk_fails_locally_after_the_earlier_chunks_through_a_symlink() {
    let scratch = Scratch::new("surrogate");
    let root = &scratch.root;
    let (state, stop) = (SURROGATE_STATE, SURROGATE_STOP);
    std::fs::write(root.join("state.json"), state).unwrap();
    std::fs::write(root.join("stop.json"), stop).unwrap();
    std::fs::write(root.join("map.md"), "old").unwrap();
    std::fs::write(root.join("json-target"), "x".repeat(36)).unwrap();
    std::os::unix::fs::symlink("json-target", root.join("map.json")).unwrap();
    let inode = std::fs::metadata(root.join("json-target")).unwrap().ino();
    let output = Command::new(env!("CARGO_BIN_EXE_render_sweep_map"))
        .current_dir(root)
        .args([
            "--state",
            "state.json",
            "--stop-json",
            "stop.json",
            "--out-md",
            "map.md",
            "--out-json",
            "map.json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim_end().ends_with("UnicodeEncodeError: 'utf-8' codec can't encode character '\\ud800' in position 7: surrogates not allowed"),
        "{stderr}"
    );
    assert_eq!(std::fs::read(root.join("map.md")).unwrap().len(), 1110);
    let target = std::fs::read(root.join("json-target")).unwrap();
    assert_eq!(target.len(), 1000);
    assert!(target.ends_with(b"\"downgrade_reason_source\": "));
    assert_eq!(
        std::fs::metadata(root.join("json-target")).unwrap().ino(),
        inode
    );
    assert!(std::fs::symlink_metadata(root.join("map.json"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn the_exported_constants_carry_the_original_title_and_lite_tool() {
    let output = Command::new(env!("CARGO_BIN_EXE_bench_reporting_api"))
        .arg("constants")
        .output()
        .unwrap();
    let constants = sweep_stop::pyjson::loads(&String::from_utf8(output.stdout).unwrap()).unwrap();
    let constants = dict(&constants);
    assert_eq!(
        text(field(constants, "MEASUREMENT_CONFIG_TITLE")),
        "## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)"
    );
    assert_eq!(
        text(field(constants, "LITE_BENCH_TOOL")),
        "vllm-bench-serve"
    );
    let sweep_msg = "bench_report";
    assert!(dict(field(constants, "BENCH_KIND_DEFAULT_EXT"))
        .get(sweep_msg)
        .is_some());
}

/// `--out-json /dev/full`: the stream accepts every write into its buffer and fails when the file is closed (the
/// flush). The original's `with open(...)` closes while unwinding, so that close failure - `OSError` 28 - is the
/// final exception even when the dump itself had already failed on a lone surrogate.
fn run_against_full_disk(
    scratch: &Scratch,
    state: &str,
    stop: &str,
) -> (Option<i32>, String, usize) {
    let root = &scratch.root;
    assert!(
        std::path::Path::new("/dev/full").exists(),
        "NOT_RUN: /dev/full is required"
    );
    std::fs::write(root.join("state.json"), state).unwrap();
    std::fs::write(root.join("stop.json"), stop).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_render_sweep_map"))
        .current_dir(root)
        .args([
            "--state",
            "state.json",
            "--stop-json",
            "stop.json",
            "--out-md",
            "map.md",
            "--out-json",
            "/dev/full",
        ])
        .output()
        .unwrap();
    assert!(output.stdout.is_empty());
    let last = String::from_utf8_lossy(&output.stderr)
        .trim_end()
        .lines()
        .last()
        .unwrap_or("")
        .to_string();
    (
        output.status.code(),
        last,
        std::fs::read(root.join("map.md")).unwrap().len(),
    )
}

const NO_SPACE: &str = "OSError: [Errno 28] No space left on device";

#[test]
fn a_close_failure_replaces_an_earlier_dump_failure_like_the_original() {
    let scratch = Scratch::new("fullsurrogate");
    let (status, last, markdown) = run_against_full_disk(&scratch, SURROGATE_STATE, SURROGATE_STOP);
    assert_eq!((status, last.as_str(), markdown), (Some(1), NO_SPACE, 1110));
}

#[test]
fn a_plain_dump_to_a_full_disk_fails_with_the_same_close_error() {
    let scratch = Scratch::new("fullplain");
    let state = SURROGATE_STATE.replace("before\\ud800after", "before-after");
    assert_ne!(state, SURROGATE_STATE);
    let (status, last, _) = run_against_full_disk(&scratch, &state, SURROGATE_STOP);
    assert_eq!((status, last.as_str()), (Some(1), NO_SPACE));
}
