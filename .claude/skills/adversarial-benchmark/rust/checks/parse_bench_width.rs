//! `parse_bench` usage/help layout against terminal width: COLUMNS 1..200 goldens captured from the original
//! (`data/parse_bench/help_width_goldens.jsonl`), unusual COLUMNS spellings, and private pseudo-terminals that
//! show which stream's size is asked. The width resolution itself is shared with `sweep_stop` (`terminal.rs`).

#[path = "common.rs"]
mod common;

use common::{command, load_jsonl, run_on_pty, text_of, Attach};
use std::process::Stdio;
use sweep_stop::pyvalue::{PyDict, PyValue};

const BIN: &str = env!("CARGO_BIN_EXE_parse_bench");

struct Golden {
    env: Option<String>,
    argv: Vec<String>,
    rc: i32,
    stdout: String,
    stderr: String,
}

fn goldens() -> Vec<Golden> {
    load_jsonl("parse_bench/help_width_goldens.jsonl")
        .into_iter()
        .map(|record: PyDict| Golden {
            env: match record.get("env") {
                Some(PyValue::Str(_)) => Some(text_of(record.get("env"))),
                _ => None,
            },
            argv: match record.get("argv") {
                Some(PyValue::List(list)) => list.iter().map(|a| text_of(Some(a))).collect(),
                other => panic!("argv must be a list, got {other:?}"),
            },
            rc: record
                .get("rc")
                .and_then(PyValue::as_plain_int)
                .unwrap()
                .to_string()
                .parse()
                .unwrap(),
            stdout: text_of(record.get("stdout")),
            stderr: text_of(record.get("stderr")),
        })
        .collect()
}

#[test]
fn layouts_match_the_original_for_every_width_and_spelling() {
    let records = goldens();
    assert!(records.len() >= 420, "golden set shrank: {}", records.len());
    let mut failures = Vec::new();
    for golden in &records {
        let output = command(BIN, golden.env.as_deref(), &golden.argv)
            .stdin(Stdio::null())
            .output()
            .expect("run parse_bench");
        let got = (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        );
        if got
            != (
                Some(golden.rc),
                golden.stdout.clone(),
                golden.stderr.clone(),
            )
        {
            failures.push(format!(
                "COLUMNS={:?} {:?}: rc {:?} (want {})",
                golden.env, golden.argv, got.0, golden.rc
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        records.len(),
        failures.join("\n")
    );
}

fn golden_help(columns: &str) -> String {
    goldens()
        .into_iter()
        .find(|g| g.env.as_deref() == Some(columns) && g.argv == ["-h"])
        .unwrap_or_else(|| panic!("no golden for COLUMNS={columns}"))
        .stdout
}

#[test]
fn stdout_terminal_width_is_used_when_columns_is_unusable() {
    for width in [20u16, 55, 120] {
        let want = golden_help(&width.to_string());
        let (stdout, _) = run_on_pty(BIN, Attach::Stdout, 24, width, None, &["-h"]);
        assert_eq!(stdout, want, "pty width {width}, COLUMNS unset");
        for bad in ["abc", "0", "-3", "", "4__0"] {
            let (stdout, _) = run_on_pty(BIN, Attach::Stdout, 24, width, Some(bad), &["-h"]);
            assert_eq!(stdout, want, "pty width {width}, COLUMNS={bad:?}");
        }
    }
}

#[test]
fn a_usable_columns_value_wins_and_only_stdout_is_asked() {
    let (stdout, _) = run_on_pty(BIN, Attach::Stdout, 24, 55, Some("30"), &["-h"]);
    assert_eq!(stdout, golden_help("30"));
    // stdin and stderr are 40-column terminals but stdout is a pipe: Python falls back to 80, not 40.
    let (stdout, stderr) = run_on_pty(BIN, Attach::StdinAndStderr, 24, 40, None, &["-h"]);
    assert_eq!(stdout, golden_help("80"));
    assert_eq!(stderr, "");
}
