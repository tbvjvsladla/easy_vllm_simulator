//! The operating-system side of the RAM gate: the adapters production programs hand to [`crate::memory`].
//!
//! [`System`] reads the real `/proc/meminfo`, tests for the real helper and runs the real `sudo -n` command; it is the only
//! provider the production programs know. [`FileMeminfo`] is the same reader over any file, and [`run_with_timeout`] the same
//! runner for any command, so the file parser and the timeout, kill and reap handling run identically wherever they are driven.
//! The protocol descriptor of the services (`--input-fd`) is validated and adopted here too.

use crate::memory::{
    mem_available_mib, open_text_file, try_drop_caches, DropSystem, HelperRequest, HelperRun,
    MeminfoSource, Provider, DROP_HELPER, MEMINFO_PATH,
};
use crate::model_files::exists;
use crate::pyexpr::Res;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// A memory file read in place of `/proc/meminfo`.
pub struct FileMeminfo {
    path: PyStr,
}

impl FileMeminfo {
    pub fn new(path: PyStr) -> Self {
        FileMeminfo { path }
    }
}

impl MeminfoSource for FileMeminfo {
    fn open(&mut self) -> Res<Box<dyn Read>> {
        Ok(Box::new(open_text_file(&self.path)?))
    }
}

/// What the helper's standard input is: the caller's own (as `subprocess.run` without `stdin=` leaves it), or the null device
/// for a program whose standard input is a protocol stream the helper must not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperInput {
    Inherited,
    Null,
}

/// The status `subprocess` reports: the exit code, or minus the signal number.
fn status_code(status: ExitStatus) -> i64 {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => i64::from(code),
        (None, Some(signal)) => -i64::from(signal),
        (None, None) => -1,
    }
}

/// `subprocess.run(argv, stdout=DEVNULL, stderr=DEVNULL, timeout=...)`: the command is started, waited for and, past the
/// limit, killed and reaped before the failure is reported (no process is left behind). A command that cannot be started, or
/// that cannot be waited for, is a failed run like a timeout is.
pub fn run_with_timeout(argv: &[&str], timeout: Duration, input: HelperInput) -> HelperRun {
    let Some((program, arguments)) = argv.split_first() else {
        return HelperRun::Failed;
    };
    let stdin = match input {
        HelperInput::Inherited => Stdio::inherit(),
        HelperInput::Null => Stdio::null(),
    };
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .stdin(stdin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return HelperRun::Failed;
    };
    let deadline = Instant::now() + timeout;
    let mut pause = Duration::from_millis(1);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return HelperRun::Exited(status_code(status)),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(pause.min(deadline.saturating_duration_since(Instant::now())));
                pause = (pause * 2).min(Duration::from_millis(20));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return HelperRun::Failed;
            }
        }
    }
}

/// The machine the production programs run on. The helper inherits this process's standard input, exactly as the original's
/// `subprocess.run` leaves it: the transport of a program never takes standard input away from the helper (see [`adopt_input_fd`]).
pub struct System;

impl System {
    pub fn new() -> Self {
        System
    }
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

impl MeminfoSource for System {
    fn open(&mut self) -> Res<Box<dyn Read>> {
        Ok(Box::new(open_text_file(&PyStr::new(MEMINFO_PATH))?))
    }
}

impl DropSystem for System {
    fn helper_exists(&mut self) -> bool {
        exists(&PyStr::new(DROP_HELPER))
    }

    fn run_helper(&mut self, request: &HelperRequest) -> HelperRun {
        run_with_timeout(
            &request.argv,
            Duration::from_secs(request.timeout_seconds),
            HelperInput::Inherited,
        )
    }
}

impl Provider for System {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        mem_available_mib(self)
    }

    fn try_drop_caches(&mut self) -> Res<bool> {
        Ok(try_drop_caches(self))
    }
}

// ── the protocol descriptor ──────────────────────────────────────────────────────────────────────

extern "C" {
    fn fcntl(descriptor: i32, command: i32, ...) -> i32;
}

const F_GETFD: i32 = 1;
const F_SETFD: i32 = 2;
const F_GETFL: i32 = 3;
const FD_CLOEXEC: i32 = 1;
const ACCESS_MODE_MASK: i32 = 3;
const WRITE_ONLY: i32 = 1;

/// `[]` or `["--input-fd", N]`: the transport option of a service command. `N` is decimal digits only, a descriptor number
/// above 2 (standard input, output and error are never the protocol channel). Anything else is the usage error's text.
pub fn input_fd_option(arguments: &[String]) -> Result<Option<i32>, String> {
    match arguments {
        [] => Ok(None),
        [flag, number] if flag == "--input-fd" => {
            if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!("--input-fd takes decimal digits, not {number:?}"));
            }
            match number.parse::<i32>() {
                Ok(descriptor) if descriptor > 2 => Ok(Some(descriptor)),
                Ok(_) => Err("--input-fd must be above 2: descriptors 0, 1 and 2 are never the protocol channel".to_string()),
                Err(_) => Err(format!("--input-fd {number} is not a descriptor number")),
            }
        }
        _ => Err("the only option is --input-fd FD".to_string()),
    }
}

/// Takes ownership of descriptor `descriptor` as the protocol's input: the request and the acknowledgements are read from it, so
/// standard input stays what the caller gave the process (the cache helper inherits it, as in the original).
///
/// The descriptor must already be open and readable in this process (a caller passes it on purpose, e.g. `pass_fds`); a closed,
/// invalid or write-only one is the error returned, never silently replaced by another source and never reopened by path. It is
/// marked close-on-exec before anything else can start a process, so the helper cannot inherit it and cannot read the protocol.
/// It is owned from here on and closed when the returned file is dropped.
///
/// # Safety
///
/// The caller transfers `descriptor` to this call: it must be a descriptor this process was handed on purpose to own (an inherited
/// protocol channel, above 2, see [`input_fd_option`]), not owned or in use by any other `File`, `OwnedFd` or handle in the process
/// (`as_raw_fd` of a live handle is not such a descriptor), and it must not be passed here a second time or used or closed by the
/// caller afterwards. `fcntl` can tell that the number is open and readable; it cannot tell whether anything else owns it, which is
/// why this function is `unsafe` and the guarantee lives at each call.
pub unsafe fn adopt_input_fd(descriptor: i32) -> Result<std::fs::File, String> {
    use std::os::fd::FromRawFd;
    let refuse = |what: &str| {
        format!(
            "input fd {descriptor}: {what}: {}",
            std::io::Error::last_os_error()
        )
    };
    // SAFETY: `fcntl` with these commands only queries or sets descriptor flags; an invalid number is reported as -1 (EBADF).
    let flags = unsafe { fcntl(descriptor, F_GETFL) };
    if flags < 0 {
        return Err(refuse("not an open descriptor"));
    }
    if flags & ACCESS_MODE_MASK == WRITE_ONLY {
        return Err(format!(
            "input fd {descriptor}: not readable (opened write-only)"
        ));
    }
    // SAFETY: as above.
    let marks = unsafe { fcntl(descriptor, F_GETFD) };
    if marks < 0 {
        return Err(refuse("cannot read the descriptor flags"));
    }
    // SAFETY: as above.
    if unsafe { fcntl(descriptor, F_SETFD, marks | FD_CLOEXEC) } < 0 {
        return Err(refuse("cannot mark the descriptor close-on-exec"));
    }
    // SAFETY: the descriptor is open (just queried) and, by the safety contract of this function, the caller has handed it over to
    // be owned exactly once; no other `File` in the process wraps it.
    Ok(unsafe { std::fs::File::from_raw_fd(descriptor) })
}

// ── standard output and error at program start ───────────────────────────────────────────────────

/// Which of the standard output and error the process was given (descriptor 1 and 2 open) when it started. Python's `sys.stdout` /
/// `sys.stderr` are `None` for a descriptor that was closed at start; Rust's runtime reopens a closed descriptor 0, 1 or 2 on the null
/// device before `main`, so that fact is only visible in a snapshot taken before it: [`startup_stdio`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StdioAtStart {
    pub stdout_open: bool,
    pub stderr_open: bool,
}

impl StdioAtStart {
    /// Both open: what every call sees where no snapshot was taken.
    pub const OPEN: StdioAtStart = StdioAtStart {
        stdout_open: true,
        stderr_open: true,
    };
}

const SNAPSHOT_TAKEN: u8 = 0b1000;
const STDIN_BIT: u8 = 0b0001;
const STDOUT_BIT: u8 = 0b0010;
const STDERR_BIT: u8 = 0b0100;
static OPEN_AT_START: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Runs before `main` (and so before the Rust runtime reopens a closed standard descriptor): records which of descriptors 0, 1, 2
/// are open. It does no allocation and no input or output, only `fcntl` and one atomic store.
extern "C" fn capture_stdio() {
    let mut bits = SNAPSHOT_TAKEN;
    for descriptor in 0..3 {
        // SAFETY: `F_GETFD` only queries a descriptor's flags; a closed descriptor is reported as -1.
        if unsafe { fcntl(descriptor, F_GETFD) } >= 0 {
            bits |= 1 << descriptor;
        }
    }
    OPEN_AT_START.store(bits, std::sync::atomic::Ordering::Relaxed);
}

/// The loader calls this once before `main` (Linux with the GNU or musl loader).
#[cfg(target_os = "linux")]
#[used]
#[link_section = ".init_array"]
static CAPTURE_STDIO_AT_LOAD: extern "C" fn() = capture_stdio;

/// Whether descriptor 0 was open when the process started (`sys.stdin` is `None` in Python when it was closed). The snapshot already
/// records it; [`StdioAtStart`] stays as it was, so this is a separate accessor. `true` where no snapshot was taken.
pub fn startup_stdin_open() -> bool {
    let bits = OPEN_AT_START.load(std::sync::atomic::Ordering::Relaxed);
    bits & SNAPSHOT_TAKEN == 0 || bits & STDIN_BIT != 0
}

/// What the process had open when it started, `OPEN` where no snapshot was taken (another platform, or a library user whose
/// executable never ran the loader hook).
pub fn startup_stdio() -> StdioAtStart {
    let bits = OPEN_AT_START.load(std::sync::atomic::Ordering::Relaxed);
    if bits & SNAPSHOT_TAKEN == 0 {
        return StdioAtStart::OPEN;
    }
    StdioAtStart {
        stdout_open: bits & STDOUT_BIT != 0,
        stderr_open: bits & STDERR_BIT != 0,
    }
}
