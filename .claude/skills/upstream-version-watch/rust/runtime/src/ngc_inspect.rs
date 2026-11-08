//! The one external boundary of `resolve_ngc_tag.py`: the `docker buildx imagetools inspect` child process of `inspect_env`
//! (`subprocess.run(argv, capture_output=True, text=True, timeout=90)`), and the strict text decoding `text=True` applies to its two streams.
//!
//! The production [`ProcessDocker`] starts the real executable named by `argv[0]` (searched on `PATH` of this process, as the child launcher of
//! the interpreter does), with the inherited standard input, both output streams piped and read to the end together, every descriptor above 2
//! closed in the child (`close_fds=True`), and kills the child when the timeout (90 s in the product; a parameter only so the process lifecycle
//! can be tested) ends first. A failure to start, or the timeout, is `None`: the original catches both in the same `except Exception`.
//!
//! The timeout is ONE deadline over everything `subprocess.run` waits for inside `communicate`: the end of both pipes (a descendant that keeps an
//! inherited pipe open after the direct child exited holds the pipe open, and the original then times out too) and then the direct child itself.
//! The two pipes are drained on the calling thread with `poll(2)`, so a deadline leaves no reader thread blocked on a pipe; only the direct child
//! is ever signalled (no process group, no descendant), and both pipe ends of this process close when the call returns.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// `timeout=90` of the original.
pub const DOCKER_TIMEOUT: Duration = Duration::from_secs(90);

/// What the finished child left: the return code (negative values and the code of a signal death are all non-zero) and the raw bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs the inspect command. `None`: the run raised (the program is missing or not executable, the timeout expired, ...).
pub trait Docker {
    fn run(&self, argv: &[Vec<u8>]) -> Option<Finished>;
}

/// The real child process.
#[derive(Clone, Copy, Debug)]
pub struct ProcessDocker {
    pub timeout: Duration,
}

impl Default for ProcessDocker {
    fn default() -> ProcessDocker {
        ProcessDocker {
            timeout: DOCKER_TIMEOUT,
        }
    }
}

extern "C" {
    fn close_range(first: u32, last: u32, flags: i32) -> i32;
    fn poll(descriptors: *mut PollDescriptor, count: std::ffi::c_ulong, timeout_ms: i32) -> i32;
}

/// `struct pollfd` of `poll(2)`.
#[repr(C)]
struct PollDescriptor {
    fd: i32,
    events: i16,
    revents: i16,
}

const POLL_IN: i16 = 0x001;

/// How long the direct child is polled for its exit once both pipes ended (`Popen.wait` of the original sleeps in small steps too).
const EXIT_POLL_STEP: Duration = Duration::from_millis(5);

const READ_CHUNK_BYTES: usize = 64 * 1024;

/// One piped output stream of the child: its read end while the child's side is still open, and the bytes seen so far.
struct Pipe {
    read_end: Option<File>,
    bytes: Vec<u8>,
}

impl Pipe {
    fn new(read_end: impl Into<OwnedFd>) -> Pipe {
        Pipe {
            read_end: Some(File::from(read_end.into())),
            bytes: Vec::new(),
        }
    }

    /// Takes what a ready descriptor holds with a single `read`, which does not block once `poll` reported the descriptor ready; end of file
    /// (or a read error, which ends the stream as `read_to_end` did) closes this end.
    fn take_ready_bytes(&mut self) {
        let Some(file) = self.read_end.as_mut() else {
            return;
        };
        let mut chunk = vec![0_u8; READ_CHUNK_BYTES];
        match file.read(&mut chunk) {
            Ok(0) => self.read_end = None,
            Ok(count) => self.bytes.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => self.read_end = None,
        }
    }
}

/// `poll(2)` for readable (or hung-up) descriptors; `Ok(0)` is the timeout.
fn wait_readable(descriptors: &mut [PollDescriptor], timeout: Duration) -> std::io::Result<usize> {
    // rounded up, so the wait never ends before the deadline and never spins on a sub-millisecond remainder
    let timeout_ms = i32::try_from(timeout.as_millis().saturating_add(1)).unwrap_or(i32::MAX);
    // SAFETY: `descriptors` is a valid, exclusively borrowed array of `descriptors.len()` initialised `struct pollfd` values for the whole call.
    let ready = unsafe {
        poll(
            descriptors.as_mut_ptr(),
            descriptors.len() as std::ffi::c_ulong,
            timeout_ms,
        )
    };
    usize::try_from(ready).map_err(|_| std::io::Error::last_os_error())
}

/// Reads both pipes to their end. `false`: the deadline came first (or `poll` itself failed, which the original also treats as a raised error).
fn drain_until(deadline: Instant, pipes: &mut [Pipe; 2]) -> bool {
    loop {
        let open: Vec<usize> = (0..pipes.len())
            .filter(|&at| pipes[at].read_end.is_some())
            .collect();
        if open.is_empty() {
            return true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        let mut descriptors: Vec<PollDescriptor> = open
            .iter()
            .filter_map(|&at| pipes[at].read_end.as_ref())
            .map(|file| PollDescriptor {
                fd: file.as_raw_fd(),
                events: POLL_IN,
                revents: 0,
            })
            .collect();
        match wait_readable(&mut descriptors, remaining) {
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return false,
        }
        for (&at, descriptor) in open.iter().zip(&descriptors) {
            if descriptor.revents != 0 {
                pipes[at].take_ready_bytes();
            }
        }
    }
}

/// The exit status of the direct child, polled until the deadline. `None`: the deadline came first (or the status could not be read).
fn wait_until(child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return None;
                }
                std::thread::sleep(remaining.min(EXIT_POLL_STEP));
            }
            Err(_) => return None,
        }
    }
}

impl Docker for ProcessDocker {
    fn run(&self, argv: &[Vec<u8>]) -> Option<Finished> {
        let (program, rest) = argv.split_first()?;
        let mut command = Command::new(std::ffi::OsStr::from_bytes(program));
        command
            .args(rest.iter().map(|word| std::ffi::OsStr::from_bytes(word)))
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: the closure runs between fork and exec and only calls `close_range`, an async-signal-safe system call wrapper that
        // allocates nothing and touches no Rust state; its failure (no such call in an old kernel) leaves the descriptors as they were.
        unsafe {
            command.pre_exec(|| {
                close_range(3, u32::MAX, 0);
                Ok(())
            });
        }
        let mut child = command.spawn().ok()?;
        let started = Instant::now();
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        let mut pipes = [Pipe::new(stdout), Pipe::new(stderr)];
        let deadline = started + self.timeout;
        let status = if drain_until(deadline, &mut pipes) {
            wait_until(&mut child, deadline)
        } else {
            None
        };
        let Some(status) = status else {
            // `subprocess.run` on a raised `communicate`: kill the direct child, reap it, and let the error become `None`. `kill` is a no-op
            // for a child already reaped; the pipe ends close when `pipes` drops, whoever still holds the other end.
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        let [stdout, stderr] = pipes;
        Some(Finished {
            status: status.code().unwrap_or(-1),
            stdout: stdout.bytes,
            stderr: stderr.bytes,
        })
    }
}

/// `data.decode(locale encoding, strict)` of the UTF-8 locale and the universal-newline translation of `text=True` (`\r\n` and `\r` become
/// `\n`). `None`: the bytes are not valid UTF-8 (the `UnicodeDecodeError` the original catches).
pub fn decode_stream(bytes: &[u8]) -> Option<agent_runtime_support::pystr::PyStr> {
    let text = std::str::from_utf8(bytes).ok()?;
    Some(agent_runtime_support::pystr::PyStr::new(
        &text.replace("\r\n", "\n").replace('\r', "\n"),
    ))
}
