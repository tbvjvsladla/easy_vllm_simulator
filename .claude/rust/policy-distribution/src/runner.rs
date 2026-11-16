//! `_run(name, argv, expected, cwd, env_overrides)`: one named child process with the original's environment
//! (inherited + `PYTHONDONTWRITEBYTECODE=1` + overrides), 180 s timeout, text-mode capture (UTF-8, universal newlines)
//! and the same result object.
use crate::pytext::{os_error, py_list_repr, tail_chars, universal_newlines};
use crate::{dict, list_ints, PyValue};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const TIMEOUT_SECS: u64 = 180;

/// Raw outcome of a child run (used by `run` and by the gitless hint check that parses the whole stdout).
pub enum Outcome {
    Done { rc: i64, stdout: String, stderr: String },
    Failed(String),
}

/// `sys.executable` analogue for the OTHER bundles' Python programs this driver still runs: the absolute path of
/// `python3` found on the caller's PATH (absolute so an override `PATH=/nonexistent` cannot hide it, exactly like
/// `sys.executable`), or the bare name when PATH has none.
pub fn python_executable() -> String {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("python3");
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    "python3".to_string()
}

/// `_child_python()`: the interpreter argv prefix. The Rust driver has no interpreter flags of its own to preserve
/// (`-I`/`-S`/`-O`/`-B` belonged to the Python driver process), so this is `[python3]`.
pub fn child_python() -> Vec<String> {
    vec![python_executable()]
}

fn decode(bytes: &[u8]) -> String {
    universal_newlines(&String::from_utf8_lossy(bytes))
}

/// Run `argv` in `cwd` with the original's environment construction and timeout.
pub fn execute(argv: &[String], cwd: &Path, env_overrides: &[(&str, &str)]) -> Outcome {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).current_dir(cwd).env("PYTHONDONTWRITEBYTECODE", "1");
    for (k, v) in env_overrides {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::inherit()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            // subprocess names the cwd when it is the missing path (the original passes a `Path`, so its repr is
            // `PosixPath('...')`), else the program.
            if !cwd.is_dir() {
                let err = os_error(&e, "");
                let message = match e.raw_os_error() {
                    Some(n) => format!("[Errno {n}] {}: PosixPath({})", crate::pytext::strerror(&e), crate::pytext::py_repr(&cwd.to_string_lossy())),
                    None => err.message,
                };
                return Outcome::Failed(format!("{}: {message}", err.type_name));
            }
            return Outcome::Failed(os_error(&e, &argv[0]).colon_space());
        }
    };
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out_pipe.read_to_end(&mut b);
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err_pipe.read_to_end(&mut b);
        b
    });
    let deadline = Instant::now() + Duration::from_secs(TIMEOUT_SECS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break child.wait().ok(),
        }
    };
    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    match status {
        None => Outcome::Failed(format!(
            "TimeoutExpired: Command '{}' timed out after {TIMEOUT_SECS} seconds",
            py_list_repr(argv)
        )),
        Some(s) => {
            use std::os::unix::process::ExitStatusExt;
            let rc = match s.code() {
                Some(c) => c as i64,
                None => -(s.signal().unwrap_or(0) as i64),
            };
            Outcome::Done { rc, stdout: decode(&stdout), stderr: decode(&stderr) }
        }
    }
}

/// `_run`.
pub fn run(name: &str, argv: &[String], expected: &[i64], cwd: &Path, env_overrides: &[(&str, &str)]) -> PyValue {
    let mut exp = expected.to_vec();
    exp.sort();
    match execute(argv, cwd, env_overrides) {
        Outcome::Done { rc, stdout, stderr } => dict(vec![
            ("name", PyValue::str(name)),
            ("ok", PyValue::Bool(exp.contains(&rc))),
            ("rc", PyValue::int(rc)),
            ("expected_rc", list_ints(&exp)),
            ("stdout_tail", PyValue::str(&tail_chars(&stdout, 1000))),
            ("stderr_tail", PyValue::str(&tail_chars(&stderr, 1000))),
        ]),
        Outcome::Failed(error) => dict(vec![
            ("name", PyValue::str(name)),
            ("ok", PyValue::Bool(false)),
            ("rc", PyValue::None),
            ("expected_rc", list_ints(&exp)),
            ("error", PyValue::str(&error)),
        ]),
    }
}
