//! `subprocess.run(cmd, stdout=PIPE, stderr=PIPE, timeout=timeout)` of CPython 3.12 for the `_run` of `cleanup_docker.py`, with the exceptions it raises.
//!
//! The order is the original's: `run` creates the pipes, `Popen._execute_child` normalises the command (`list(args)`: a `str` is one
//! argument, a mapping is its keys; `args[0]` is the executable), tells the audit hook (`process_popen`), converts the executable and every
//! argument as a file-system path (`TypeError`, `ValueError: embedded null byte`), then forks and execs each candidate in turn (the
//! executable itself when it has a `/`, else one per directory of the child's `PATH`); the first error that is not `ENOENT`/`ENOTDIR` (else
//! the last) is the `OSError` named with the original executable. Only then does `communicate(timeout=...)` start: `time + timeout` (a
//! `TypeError` for a timeout that is not a number, raised AFTER the child exists, which `run` then kills and reaps), the `poll` loop that
//! drains both pipes at once in 32 KiB reads, the deadline check after every `poll`, and the wait for the exit with the time left. A
//! timeout kills and reaps the child and is `TimeoutExpired` with the bytes read so far (`None` for a pipe nothing was read from).
//!
//! Standard input is the caller's own (inherited); the environment is the caller's. Not claimed: a closed standard input, signal masks,
//! `ENOEXEC` scripts without an interpreter line, timeouts above about 24 days (the `poll` limit), a descendant that outlives the child
//! and keeps a pipe open (as in Python, only the child is killed and reaped).

use crate::escalation::Raised;
use crate::functional_smoke::Observe;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

extern "C" {
    fn poll(fds: *mut PollFd, count: std::os::raw::c_ulong, timeout: i32) -> i32;
}

const POLLIN: i16 = 0x001;

/// What `subprocess.run` returned.
pub struct Completed {
    pub returncode: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// The bytes a `TimeoutExpired` carries: `(stdout, stderr)`, each `None` when nothing was read from that pipe.
pub type Partial = (Option<Vec<u8>>, Option<Vec<u8>>);

/// What `subprocess.run` raised: the exception, and for `TimeoutExpired` the bytes it carries (both boxed: the failure is a small `Err`).
pub struct RunFailure {
    pub raised: Box<Raised>,
    /// `Some` only for `TimeoutExpired`.
    pub output: Option<Box<Partial>>,
}

fn failure(raised: Raised) -> RunFailure {
    RunFailure {
        raised: Box::new(raised),
        output: None,
    }
}

fn plain(class: &str, message: &str) -> RunFailure {
    failure(Raised::plain(class, message))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn optional_hex(bytes: &Option<Vec<u8>>) -> PyValue {
    bytes
        .as_ref()
        .map_or(PyValue::None, |bytes| PyValue::str(&hex(bytes)))
}

fn note(observer: &mut dyn Observe, name: &str, fields: Vec<(&str, PyValue)>) {
    if observer.enabled() {
        observer.record(name, fields);
    }
}

/// `os.fsencode(value)`: a `str` (lone surrogates that are escaped bytes become those bytes), else the `TypeError` of a non-path.
fn fsencode(value: &PyValue) -> Result<Vec<u8>, RunFailure> {
    match value.peel() {
        PyValue::Str(text) => text
            .to_surrogateescape_bytes()
            .map_err(|encode| plain("UnicodeEncodeError", &encode.message())),
        other => Err(plain(
            "TypeError",
            &format!(
                "expected str, bytes or os.PathLike object, not {}",
                other.type_name()
            ),
        )),
    }
}

/// `list(args)` of `_execute_child`.
fn normalise(cmd: &PyValue) -> Result<Vec<PyValue>, RunFailure> {
    match cmd.peel() {
        PyValue::Str(_) => Ok(vec![cmd.clone()]),
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Dict(map) => Ok(map
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        other => Err(plain(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

/// `os.get_exec_path()` joined with the executable: one candidate per directory of `PATH` (`os.defpath` when it is not set).
fn candidates(executable: &[u8]) -> Vec<Vec<u8>> {
    if executable.contains(&b'/') {
        return vec![executable.to_vec()];
    }
    let path = std::env::var_os("PATH")
        .map(|value| value.as_bytes().to_vec())
        .unwrap_or_else(|| b"/bin:/usr/bin".to_vec());
    path.split(|&byte| byte == b':')
        .map(|directory| {
            let mut joined = directory.to_vec();
            if !joined.is_empty() && !joined.ends_with(b"/") {
                joined.push(b'/');
            }
            joined.extend_from_slice(executable);
            joined
        })
        .collect()
}

/// The `fork_exec` attempts: the child is created by the first candidate that executes; otherwise the `errno` Python reports.
fn spawn(candidates: &[Vec<u8>], arguments: &[Vec<u8>]) -> Result<Child, i32> {
    let (mut saved, mut last) = (0, 0);
    for candidate in candidates {
        let mut program = candidate.clone();
        if !program.contains(&b'/') {
            let mut relative = b"./".to_vec();
            relative.extend_from_slice(&program);
            program = relative;
        }
        let mut command = Command::new(std::ffi::OsStr::from_bytes(&program));
        command
            .arg0(std::ffi::OsStr::from_bytes(&arguments[0]))
            .args(
                arguments[1..]
                    .iter()
                    .map(|a| std::ffi::OsStr::from_bytes(a)),
            )
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error) => {
                let errno = error.raw_os_error().unwrap_or(0);
                last = errno;
                if errno != 2 && errno != 20 && saved == 0 {
                    saved = errno;
                }
            }
        }
    }
    Err(if saved != 0 { saved } else { last })
}

/// A pipe being drained.
struct Pipe<R: Read + AsRawFd> {
    reader: Option<R>,
    chunks: Vec<Vec<u8>>,
}

impl<R: Read + AsRawFd> Pipe<R> {
    fn bytes(&self) -> Option<Vec<u8>> {
        if self.chunks.is_empty() {
            None
        } else {
            Some(self.chunks.concat())
        }
    }
}

fn expired(
    cmd: &PyValue,
    timeout: &PyValue,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
) -> RunFailure {
    let mut message = PyStr::new("Command '");
    message.push_py(&cmd.display_text());
    message.push_str("' timed out after ");
    message.push_py(&timeout.display_text());
    message.push_str(" seconds");
    RunFailure {
        raised: Box::new(Raised::new("TimeoutExpired", "subprocess", message)),
        output: Some(Box::new((stdout, stderr))),
    }
}

/// The timeout as the number `time + timeout` makes of it (`None` is no deadline).
fn deadline_of(timeout: &PyValue) -> Result<Option<f64>, RunFailure> {
    match timeout.peel() {
        PyValue::None => Ok(None),
        PyValue::Bool(flag) => Ok(Some(f64::from(u8::from(*flag)))),
        PyValue::Int(number) => agent_runtime_support::pynumber::float_from_int(number)
            .map(Some)
            .map_err(|_| plain("OverflowError", "int too large to convert to float")),
        PyValue::Float(number) => Ok(Some(*number)),
        other => Err(plain(
            "TypeError",
            &format!(
                "unsupported operand type(s) for +: 'float' and '{}'",
                other.type_name()
            ),
        )),
    }
}

fn io_failure(error: &std::io::Error) -> RunFailure {
    failure(Raised::from(crate::model_files::os_failure(error, None)))
}

/// `subprocess.run` after the child exists: drain, wait, and kill and reap on any failure.
fn communicate(
    mut child: Child,
    cmd: &PyValue,
    timeout: &PyValue,
) -> Result<Completed, RunFailure> {
    let outcome = communicate_inner(&mut child, cmd, timeout);
    if outcome.is_err() {
        // `process.kill()`, then `Popen.__exit__` waits: the child is reaped whatever failed.
        let _ = child.kill();
        let _ = child.wait();
    }
    outcome
}

fn communicate_inner(
    child: &mut Child,
    cmd: &PyValue,
    timeout: &PyValue,
) -> Result<Completed, RunFailure> {
    let deadline = deadline_of(timeout)?;
    let started = Instant::now();
    let mut out = Pipe {
        reader: child.stdout.take(),
        chunks: Vec::new(),
    };
    let mut err = Pipe {
        reader: child.stderr.take(),
        chunks: Vec::new(),
    };
    let remaining =
        |deadline: Option<f64>| deadline.map(|limit| limit - started.elapsed().as_secs_f64());
    while out.reader.is_some() || err.reader.is_some() {
        let left = remaining(deadline);
        if left.is_some_and(|seconds| seconds < 0.0) {
            return Err(expired(cmd, timeout, out.bytes(), err.bytes()));
        }
        let wait_ms = match left {
            None => -1,
            Some(seconds) if seconds.is_nan() => {
                return Err(plain("ValueError", "cannot convert float NaN to integer"))
            }
            Some(seconds) if seconds.is_infinite() => {
                return Err(plain(
                    "OverflowError",
                    "cannot convert float infinity to integer",
                ))
            }
            Some(seconds) if seconds <= 0.0 => 0,
            Some(seconds) => (seconds * 1e3).ceil().min(f64::from(i32::MAX)) as i32,
        };
        let mut fds: Vec<PollFd> = Vec::new();
        let mut which: Vec<bool> = Vec::new(); // true = stdout
        if let Some(reader) = &out.reader {
            fds.push(PollFd {
                fd: reader.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            });
            which.push(true);
        }
        if let Some(reader) = &err.reader {
            fds.push(PollFd {
                fd: reader.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            });
            which.push(false);
        }
        loop {
            // SAFETY: `fds` is a live array of `count` initialised `pollfd` records for descriptors owned by the pipes above.
            let ready = unsafe {
                poll(
                    fds.as_mut_ptr(),
                    fds.len() as std::os::raw::c_ulong,
                    wait_ms,
                )
            };
            if ready >= 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(io_failure(&error));
            }
        }
        if let Some(limit) = deadline {
            if started.elapsed().as_secs_f64() > limit {
                return Err(expired(cmd, timeout, out.bytes(), err.bytes()));
            }
        }
        for (entry, is_out) in fds.iter().zip(&which) {
            if entry.revents == 0 {
                continue;
            }
            let mut buffer = vec![0u8; 32768];
            let read = if *is_out {
                out.reader.as_mut().map(|reader| reader.read(&mut buffer))
            } else {
                err.reader.as_mut().map(|reader| reader.read(&mut buffer))
            };
            let Some(read) = read else { continue };
            let count = match read {
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(io_failure(&error)),
            };
            buffer.truncate(count);
            if *is_out {
                if count == 0 {
                    out.reader = None;
                }
                out.chunks.push(buffer);
            } else {
                if count == 0 {
                    err.reader = None;
                }
                err.chunks.push(buffer);
            }
        }
    }
    // `self.wait(timeout=remaining)`
    let status = match remaining(deadline) {
        None => child.wait().map_err(|error| io_failure(&error))?,
        Some(left) => {
            let mut delay = 0.0005_f64;
            let end = started.elapsed().as_secs_f64() + left;
            loop {
                if let Some(status) = child.try_wait().map_err(|error| io_failure(&error))? {
                    break status;
                }
                let now_left = end - started.elapsed().as_secs_f64();
                if now_left <= 0.0 {
                    let mut failed = expired(cmd, &PyValue::Float(left), None, None);
                    failed.output = Some(Box::new((None, None)));
                    return Err(failed);
                }
                delay = (delay * 2.0).min(now_left).min(0.05);
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }
    };
    let returncode = status
        .code()
        .map(i64::from)
        .or_else(|| status.signal().map(|signal| -i64::from(signal)))
        .unwrap_or(0);
    Ok(Completed {
        returncode,
        stdout: out.chunks.concat(),
        stderr: err.chunks.concat(),
    })
}

fn run_inner(
    observer: &mut dyn Observe,
    cmd: &PyValue,
    timeout: &PyValue,
) -> Result<Completed, RunFailure> {
    let args = normalise(cmd)?;
    let Some(executable) = args.first().cloned() else {
        return Err(plain("IndexError", "list index out of range"));
    };
    note(
        observer,
        "process_popen",
        vec![
            ("executable", executable.clone()),
            ("args", PyValue::List(args.clone())),
            ("cwd", PyValue::None),
            ("env", PyValue::None),
        ],
    );
    let program = fsencode(&executable)?;
    let mut converted = Vec::with_capacity(args.len());
    for argument in &args {
        converted.push(fsencode(argument)?);
    }
    if program.contains(&0) || converted.iter().any(|argument| argument.contains(&0)) {
        return Err(plain("ValueError", "embedded null byte"));
    }
    let list = candidates(&program);
    let child = spawn(&list, &converted).map_err(|errno| {
        let filename = match executable.peel() {
            PyValue::Str(text) => text.clone(),
            _ => PyStr::default(),
        };
        failure(Raised::from(crate::model_files::os_failure(
            &std::io::Error::from_raw_os_error(errno),
            Some(&filename),
        )))
    })?;
    communicate(child, cmd, timeout)
}

/// `subprocess.run(cmd, stdout=PIPE, stderr=PIPE, timeout=timeout)`, told to `observer` (`process_run`, `process_popen`, then
/// `process_return` or `process_error`).
pub fn run(
    observer: &mut dyn Observe,
    cmd: &PyValue,
    timeout: &PyValue,
) -> Result<Completed, RunFailure> {
    note(
        observer,
        "process_run",
        vec![
            ("cmd", cmd.clone()),
            ("timeout", timeout.clone()),
            ("stdout", PyValue::str("PIPE")),
            ("stderr", PyValue::str("PIPE")),
        ],
    );
    let outcome = run_inner(observer, cmd, timeout);
    match &outcome {
        Ok(done) => note(
            observer,
            "process_return",
            vec![
                ("returncode", PyValue::int(done.returncode)),
                ("stdout_hex", PyValue::str(&hex(&done.stdout))),
                ("stderr_hex", PyValue::str(&hex(&done.stderr))),
            ],
        ),
        Err(failed) => {
            let mut fields = vec![
                ("type", PyValue::str(&failed.raised.class)),
                ("module", PyValue::str(&failed.raised.module)),
                ("message", PyValue::Str(failed.raised.message.clone())),
            ];
            if let Some(partial) = &failed.output {
                let (stdout, stderr) = &**partial;
                fields.push(("stdout_hex", optional_hex(stdout)));
                fields.push(("stderr_hex", optional_hex(stderr)));
            }
            note(observer, "process_error", fields);
        }
    }
    outcome
}

/// The `_run` of the original: `(returncode, stdout decoded as UTF-8 with replacement)`, or `(1, str(error))` for any ordinary exception.
pub fn run_text(observer: &mut dyn Observe, cmd: &PyValue, timeout: &PyValue) -> (i64, PyStr) {
    match run(observer, cmd, timeout) {
        Ok(done) => (
            done.returncode,
            PyStr::new(&String::from_utf8_lossy(&done.stdout)),
        ),
        Err(failed) => (1, failed.raised.message),
    }
}
