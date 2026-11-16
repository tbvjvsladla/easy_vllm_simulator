//! `subprocess.run(argv, cwd=..., capture_output=True, timeout=..., stdin=DEVNULL)` as the providers call it, behind an
//! injectable function type so the self-test can replace the process (the original monkeypatches `subprocess.run`).

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Completed {
    pub returncode: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// An `OSError` the original would see: class name, errno, strerror and `filename`.
#[derive(Debug, Clone)]
pub struct OsErr {
    pub class: &'static str,
    pub errno: i32,
    pub strerror: String,
    pub filename: String,
}

impl OsErr {
    pub fn new(errno: i32, filename: &str) -> Self {
        let (class, strerror) = match errno {
            2 => ("FileNotFoundError", "No such file or directory"),
            13 => ("PermissionError", "Permission denied"),
            20 => ("NotADirectoryError", "Not a directory"),
            8 => ("OSError", "Exec format error"),
            _ => ("OSError", "Unknown error"),
        };
        OsErr { class, errno, strerror: strerror.into(), filename: filename.into() }
    }
    /// `str(exc)`: `[Errno N] strerror: 'filename'`.
    pub fn text(&self) -> String {
        format!("[Errno {}] {}: {}", self.errno, self.strerror, crate::agent_control::pyhelp::repr_str(&self.filename))
    }
}

#[derive(Debug, Clone)]
pub enum RunError {
    /// `subprocess.TimeoutExpired` (the output read after the kill).
    Timeout { stdout: Vec<u8> },
    Os(OsErr),
}

/// The injectable `subprocess.run`: `(argv, cwd, timeout_seconds)`.
pub type RunFn<'a> = &'a dyn Fn(&[String], Option<&str>, Option<f64>) -> Result<Completed, RunError>;

fn is_exec(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Where `execvp` (after `chdir(cwd)`) would find `prog`; `Err(errno)` as the original's child reports it.
pub fn resolve(prog: &str, cwd: Option<&str>) -> Result<PathBuf, i32> {
    let base = match cwd {
        Some(c) => PathBuf::from(c),
        None => std::env::current_dir().unwrap_or_default(),
    };
    let base = if base.is_absolute() { base } else { std::env::current_dir().unwrap_or_default().join(base) };
    if prog.contains('/') {
        let p = base.join(prog);
        return if is_exec(&p) {
            Ok(p)
        } else if p.exists() {
            Err(13)
        } else {
            Err(2)
        };
    }
    let path = std::env::var("PATH").unwrap_or_else(|_| "/bin:/usr/bin".into());
    let mut denied = false;
    for dir in path.split(':') {
        let d = if dir.is_empty() { Path::new(".") } else { Path::new(dir) };
        let p = base.join(d).join(prog);
        if is_exec(&p) {
            return Ok(p);
        }
        if p.is_file() {
            denied = true;
        }
    }
    Err(if denied { 13 } else { 2 })
}

/// Check `cwd` the way the child's `chdir` would.
pub fn check_cwd(cwd: Option<&str>) -> Result<(), OsErr> {
    if let Some(c) = cwd {
        match std::fs::metadata(c) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(OsErr::new(20, c)),
            Err(e) => return Err(OsErr::new(e.raw_os_error().unwrap_or(2), c)),
        }
    }
    Ok(())
}

fn reader<R: Read + Send + 'static>(mut r: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut b = vec![];
        let _ = r.read_to_end(&mut b);
        b
    })
}

/// Spawn `argv` (cwd, stdin=DEVNULL, both outputs piped); `new_session` = `start_new_session=True`.
pub fn spawn(argv: &[String], cwd: Option<&str>, new_session: bool) -> Result<Child, OsErr> {
    check_cwd(cwd)?;
    let exe = resolve(&argv[0], cwd).map_err(|e| OsErr::new(e, &argv[0]))?;
    let mut cmd = Command::new(&exe);
    cmd.arg0(&argv[0]).args(&argv[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    if new_session {
        unsafe {
            cmd.pre_exec(|| {
                extern "C" {
                    fn setsid() -> i32;
                }
                setsid();
                Ok(())
            });
        }
    }
    cmd.spawn().map_err(|e| OsErr::new(e.raw_os_error().unwrap_or(2), &argv[0]))
}

/// Exit status as `Popen.returncode` (`-signal` for a signal death).
pub fn returncode(st: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    match st.code() {
        Some(c) => c,
        None => -st.signal().unwrap_or(0),
    }
}

/// The real `subprocess.run(argv, cwd=cwd, capture_output=True, text=True, timeout=timeout, stdin=DEVNULL)`.
pub fn subprocess_run(argv: &[String], cwd: Option<&str>, timeout: Option<f64>) -> Result<Completed, RunError> {
    let mut child = spawn(argv, cwd, false).map_err(RunError::Os)?;
    let out = reader(child.stdout.take().unwrap());
    let err = reader(child.stderr.take().unwrap());
    let start = Instant::now();
    loop {
        if let Ok(Some(st)) = child.try_wait() {
            return Ok(Completed {
                returncode: returncode(st),
                stdout: out.join().unwrap_or_default(),
                stderr: err.join().unwrap_or_default(),
            });
        }
        if let Some(t) = timeout {
            if start.elapsed().as_secs_f64() >= t {
                let _ = child.kill();
                let _ = child.wait();
                let stdout = out.join().unwrap_or_default();
                let _ = err.join();
                return Err(RunError::Timeout { stdout });
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
