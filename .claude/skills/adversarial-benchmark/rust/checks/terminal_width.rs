//! Usage/help layout against terminal width.
//!
//! Goldens are the original CLI's own output (`gen_width_goldens.py`): `-h` stdout and the usage + error stderr
//! for COLUMNS 1..200, for unusual COLUMNS spellings and for several error kinds. The environment of each child
//! is set per process (`Command::env`), never in the test process, so concurrent tests do not interfere.
//! Pseudo-terminal cases open a private pty pair, set its size and attach it to the child; no real terminal is
//! touched.

#[path = "common.rs"]
mod common;

use common::{command, load_jsonl, run_on_pty, text_of, Attach};
use std::process::Stdio;

const BIN: &str = env!("CARGO_BIN_EXE_sweep_stop");
use sweep_stop::pyvalue::{PyDict, PyValue};

struct Golden {
    env: Option<String>,
    argv: Vec<String>,
    rc: i32,
    stdout: String,
    stderr: String,
}

fn goldens() -> Vec<Golden> {
    load_jsonl("help_width_goldens.jsonl")
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
    assert!(records.len() >= 450, "golden set shrank: {}", records.len());
    let mut failures = Vec::new();
    for golden in &records {
        let output = command(BIN, golden.env.as_deref(), &golden.argv)
            .stdin(Stdio::null())
            .output()
            .expect("run sweep_stop");
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
                "COLUMNS={:?} {:?}: rc {:?} (want {}), stdout equal={}, stderr equal={}",
                golden.env,
                golden.argv,
                got.0,
                golden.rc,
                got.1 == golden.stdout,
                got.2 == golden.stderr
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

/// The original's `-h` stdout (or, for `--bogus`, stderr) at `COLUMNS=columns`; `None` env means "unset".
fn golden_text(columns: &str, argv: &str) -> String {
    let record = goldens()
        .into_iter()
        .find(|g| g.env.as_deref() == Some(columns) && g.argv == [argv])
        .unwrap_or_else(|| panic!("no golden for COLUMNS={columns} {argv}"));
    if argv == "-h" {
        record.stdout
    } else {
        record.stderr
    }
}

#[test]
fn stdout_terminal_width_is_used_when_columns_is_unusable() {
    for (rows, width) in [(24, 55), (0, 55), (24, 20), (24, 120)] {
        let want = golden_text(&width.to_string(), "-h");
        let (stdout, _) = run_on_pty(BIN, Attach::Stdout, rows, width, None, &["-h"]);
        assert_eq!(stdout, want, "pty {rows}x{width}, COLUMNS unset");
        for bad in ["abc", "0", "-3", "", "1e2", "4__0"] {
            let (stdout, _) = run_on_pty(BIN, Attach::Stdout, rows, width, Some(bad), &["-h"]);
            assert_eq!(stdout, want, "pty {rows}x{width}, COLUMNS={bad:?}");
        }
    }
}

#[test]
fn a_usable_columns_value_wins_over_the_terminal() {
    let (stdout, _) = run_on_pty(BIN, Attach::Stdout, 24, 55, Some("30"), &["-h"]);
    assert_eq!(stdout, golden_text("30", "-h"));
    let (stdout, _) = run_on_pty(BIN, Attach::Stdout, 24, 55, Some("٣٠"), &["-h"]);
    assert_eq!(
        stdout,
        golden_text("30", "-h"),
        "Arabic-Indic digits are digits"
    );
}

#[test]
fn a_terminal_reporting_zero_columns_means_eighty() {
    let (stdout, _) = run_on_pty(BIN, Attach::Stdout, 24, 0, None, &["-h"]);
    assert_eq!(
        stdout,
        golden_text("200", "-h"),
        "80 columns share the widest layout"
    );
    assert_eq!(golden_text("80", "-h"), golden_text("200", "-h"));
}

#[test]
fn only_stdout_is_asked_never_stdin_or_stderr() {
    // stdin and stderr are 40-column terminals but stdout is a pipe: Python falls back to 80, not 40.
    let (stdout, stderr) = run_on_pty(BIN, Attach::StdinAndStderr, 24, 40, None, &["-h"]);
    assert_eq!(stdout, golden_text("80", "-h"));
    assert_eq!(stderr, "");
    // The error text goes to stderr (a pipe) while stdout is the 40-column terminal: the layout follows stdout.
    let (_, stderr) = run_on_pty(BIN, Attach::Stdout, 24, 40, None, &["--bogus"]);
    assert_eq!(stderr, golden_text("40", "--bogus"));
}
