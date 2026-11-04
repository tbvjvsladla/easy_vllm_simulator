//! Usage/help layout against terminal width.
//!
//! Goldens are the original CLI's own output (`gen_width_goldens.py`): `-h` stdout and the usage + error stderr
//! for COLUMNS 1..200, for unusual COLUMNS spellings and for several error kinds. The environment of each child
//! is set per process (`Command::env`), never in the test process, so concurrent tests do not interfere.
//! Pseudo-terminal cases open a private pty pair, set its size and attach it to the child; no real terminal is
//! touched.

#[path = "common.rs"]
mod common;

use common::{load_jsonl, text_of};
use rustix::pty::{grantpt, openpt, ptsname, unlockpt, OpenptFlags};
use rustix::termios::{tcsetwinsize, Winsize};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};
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

fn command(env: Option<&str>, argv: &[String]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sweep_stop"));
    command.args(argv).env_remove("COLUMNS").env_remove("LINES");
    if let Some(value) = env {
        command.env("COLUMNS", value);
    }
    command
}

#[test]
fn layouts_match_the_original_for_every_width_and_spelling() {
    let records = goldens();
    assert!(records.len() >= 450, "golden set shrank: {}", records.len());
    let mut failures = Vec::new();
    for golden in &records {
        let output = command(golden.env.as_deref(), &golden.argv)
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

struct Pty {
    master: File,
    slave: File,
}

fn open_pty(rows: u16, columns: u16) -> Pty {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("openpt");
    grantpt(&master).expect("grantpt");
    unlockpt(&master).expect("unlockpt");
    let name = ptsname(&master, Vec::new()).expect("ptsname");
    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .open(std::ffi::OsStr::from_bytes(name.to_bytes()))
        .expect("open pty slave");
    tcsetwinsize(
        &master,
        Winsize {
            ws_row: rows,
            ws_col: columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .expect("set pty size");
    Pty {
        master: File::from(master),
        slave,
    }
}

/// Everything written to the terminal side, with the terminal's `\n` -> `\r\n` translation undone.
fn drain(mut master: File) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match master.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(error) if error.raw_os_error() == Some(5) => break, // EIO: every slave descriptor is closed
            Err(error) => panic!("reading the pty master: {error}"),
        }
    }
    String::from_utf8(bytes)
        .expect("UTF-8 output")
        .replace("\r\n", "\n")
}

enum Attach {
    /// stdout is the pty; stderr is a pipe, stdin is null.
    Stdout,
    /// stdout is a pipe; stdin and stderr are the pty.
    StdinAndStderr,
}

/// Runs `sweep_stop` with the chosen streams on a pty of the given size; returns (stdout, stderr) as text.
fn run_on_pty(
    attach: Attach,
    rows: u16,
    columns: u16,
    env: Option<&str>,
    argv: &[&str],
) -> (String, String) {
    let pty = open_pty(rows, columns);
    let argv: Vec<String> = argv.iter().map(|a| a.to_string()).collect();
    let mut child = command(env, &argv);
    match attach {
        Attach::Stdout => {
            child
                .stdin(Stdio::null())
                .stdout(Stdio::from(pty.slave.try_clone().unwrap()))
                .stderr(Stdio::piped());
        }
        Attach::StdinAndStderr => {
            child
                .stdin(Stdio::from(pty.slave.try_clone().unwrap()))
                .stdout(Stdio::piped())
                .stderr(Stdio::from(pty.slave.try_clone().unwrap()));
        }
    }
    let handle = child.spawn().expect("spawn");
    drop(child);
    let Pty { master, slave } = pty;
    drop(slave);
    let output = handle.wait_with_output().expect("wait");
    let terminal_text = drain(master);
    match attach {
        Attach::Stdout => (terminal_text, String::from_utf8(output.stderr).unwrap()),
        Attach::StdinAndStderr => (String::from_utf8(output.stdout).unwrap(), terminal_text),
    }
}

#[test]
fn stdout_terminal_width_is_used_when_columns_is_unusable() {
    for (rows, width) in [(24, 55), (0, 55), (24, 20), (24, 120)] {
        let want = golden_text(&width.to_string(), "-h");
        let (stdout, _) = run_on_pty(Attach::Stdout, rows, width, None, &["-h"]);
        assert_eq!(stdout, want, "pty {rows}x{width}, COLUMNS unset");
        for bad in ["abc", "0", "-3", "", "1e2", "4__0"] {
            let (stdout, _) = run_on_pty(Attach::Stdout, rows, width, Some(bad), &["-h"]);
            assert_eq!(stdout, want, "pty {rows}x{width}, COLUMNS={bad:?}");
        }
    }
}

#[test]
fn a_usable_columns_value_wins_over_the_terminal() {
    let (stdout, _) = run_on_pty(Attach::Stdout, 24, 55, Some("30"), &["-h"]);
    assert_eq!(stdout, golden_text("30", "-h"));
    let (stdout, _) = run_on_pty(Attach::Stdout, 24, 55, Some("٣٠"), &["-h"]);
    assert_eq!(
        stdout,
        golden_text("30", "-h"),
        "Arabic-Indic digits are digits"
    );
}

#[test]
fn a_terminal_reporting_zero_columns_means_eighty() {
    let (stdout, _) = run_on_pty(Attach::Stdout, 24, 0, None, &["-h"]);
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
    let (stdout, stderr) = run_on_pty(Attach::StdinAndStderr, 24, 40, None, &["-h"]);
    assert_eq!(stdout, golden_text("80", "-h"));
    assert_eq!(stderr, "");
    // The error text goes to stderr (a pipe) while stdout is the 40-column terminal: the layout follows stdout.
    let (_, stderr) = run_on_pty(Attach::Stdout, 24, 40, None, &["--bogus"]);
    assert_eq!(stderr, golden_text("40", "--bogus"));
}
