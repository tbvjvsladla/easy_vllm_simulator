//! Shared helpers for the integration checks (included with `#[path]`; not a test target itself).

#![allow(dead_code)]

use rustix::pty::{grantpt, openpt, ptsname, unlockpt, OpenptFlags};
use rustix::termios::{tcsetwinsize, Winsize};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

// ── running a program under a controlled width ───────────────────────────────────

/// A command with `COLUMNS` set (or removed) and `LINES` removed, per child, never in the test process.
pub fn command(bin: &str, env: Option<&str>, argv: &[String]) -> Command {
    let mut command = Command::new(bin);
    command.args(argv).env_remove("COLUMNS").env_remove("LINES");
    if let Some(value) = env {
        command.env("COLUMNS", value);
    }
    command
}

pub struct Pty {
    pub master: File,
    pub slave: File,
}

/// A pseudo-terminal pair. The master is created close-on-exec: tests run on parallel threads and spawn children, and
/// an exec child that inherited this master would keep the terminal alive after the owner dropped it ("master gone"
/// is then false), which turned a deterministic EIO into a normal exit in the `pty-gone` checks.
pub fn open_pty(rows: u16, columns: u16) -> Pty {
    let master =
        openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC).expect("openpt");
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
pub fn drain(mut master: File) -> String {
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

pub enum Attach {
    /// stdout is the pty; stderr is a pipe, stdin is null.
    Stdout,
    /// stdout is a pipe; stdin and stderr are the pty.
    StdinAndStderr,
}

/// Runs `sweep_stop` with the chosen streams on a pty of the given size; returns (stdout, stderr) as text.
pub fn run_on_pty(
    bin: &str,
    attach: Attach,
    rows: u16,
    columns: u16,
    env: Option<&str>,
    argv: &[&str],
) -> (String, String) {
    let pty = open_pty(rows, columns);
    let argv: Vec<String> = argv.iter().map(|a| a.to_string()).collect();
    let mut child = command(bin, env, &argv);
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

// ── roofline / verdict_rule fixtures ────────────────────────────────────────────

/// Writes the fixture input files (`data/roofline_verdict/inputs.jsonl`) into a scratch tree under the build
/// directory, once per test binary, and returns its root. `{DIR}/<kind>/…` in the case files means this root.
pub fn roofline_inputs() -> &'static Path {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("roofline_verdict_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for record in load_jsonl("roofline_verdict/inputs.jsonl") {
            let path = root.join(text_of(record.get("path")));
            if matches!(record.get("dir"), Some(PyValue::Bool(true))) {
                std::fs::create_dir_all(&path).expect("input directory");
                continue;
            }
            std::fs::create_dir_all(path.parent().expect("parent")).expect("input directory");
            std::fs::write(&path, hex_decode(&text_of(record.get("hex")))).expect("input file");
        }
        root
    })
}

/// Type-aware equality of two parsed JSON documents: key order matters, NaN equals NaN, `1` differs from `1.0`.
pub fn same(left: &PyValue, right: &PyValue) -> bool {
    match (left, right) {
        (PyValue::Float(a), PyValue::Float(b)) => {
            a == b && a.is_sign_negative() == b.is_sign_negative() || (a.is_nan() && b.is_nan())
        }
        (PyValue::List(a), PyValue::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
        }
        (PyValue::Dict(a), PyValue::Dict(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
        }
        _ => left == right,
    }
}
