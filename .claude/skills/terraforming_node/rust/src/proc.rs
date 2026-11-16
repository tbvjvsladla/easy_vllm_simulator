//! `subprocess.run(..., capture_output=True, text=True[, input=..., timeout=...])` and `shutil.which` as the originals use them.

use crate::raise::Raised;
use agent_runtime_support::pystr::PyStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Completed {
    pub returncode: i32,
    pub stdout: String,
    pub stderr: String,
}

fn list_repr(argv: &[String]) -> String {
    format!(
        "[{}]",
        argv.iter()
            .map(|a| PyStr::new(a).repr())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Run `argv` with captured, text-decoded output (lossy UTF-8: a documented residual where Python would raise).
/// `Err` = the exception the original's `subprocess.run` raises (program missing, timeout).
pub fn run(
    argv: &[String],
    input: Option<&str>,
    timeout: Option<u64>,
) -> Result<Completed, Raised> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::inherit()
    });
    let mut child = cmd.spawn().map_err(|e| Raised::os(&e, &argv[0]))?;
    if let Some(t) = input {
        let mut si = child.stdin.take().expect("piped stdin");
        let data = t.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
    }
    let mut out = child.stdout.take().expect("piped stdout");
    let mut err = child.stderr.take().expect("piped stderr");
    let to = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = out.read_to_end(&mut b);
        b
    });
    let te = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = err.read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(Raised::new("OSError", &e.to_string())),
        }
        if let Some(t) = timeout {
            if start.elapsed() >= Duration::from_secs(t) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Raised(format!(
                    "subprocess.TimeoutExpired: Command '{}' timed out after {} seconds",
                    list_repr(argv),
                    t
                )));
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = String::from_utf8_lossy(&to.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&te.join().unwrap_or_default()).into_owned();
    use std::os::unix::process::ExitStatusExt;
    let returncode = status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or(0));
    Ok(Completed {
        returncode,
        stdout,
        stderr,
    })
}

/// `shutil.which(name)` (PATH search; executable regular files only).
pub fn which(name: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let ok = |p: &Path| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            && is_executable(p)
    };
    if name.contains('/') {
        return if ok(Path::new(name)) {
            Some(name.to_string())
        } else {
            None
        };
    }
    let path = std::env::var("PATH").unwrap_or_else(|_| "/bin:/usr/bin".into());
    for dir in path.split(':') {
        // os.path.join(dir, name)
        let joined = if dir.is_empty() || dir.ends_with('/') {
            format!("{dir}{name}")
        } else {
            format!("{dir}/{name}")
        };
        let p: PathBuf = PathBuf::from(&joined);
        if ok(&p) {
            return Some(joined);
        }
    }
    None
}

fn is_executable(p: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = CString::new(p.as_os_str().as_bytes()) else {
        return false;
    };
    extern "C" {
        fn access(path: *const std::os::raw::c_char, mode: i32) -> i32;
    }
    unsafe { access(c.as_ptr(), 1) == 0 }
}
