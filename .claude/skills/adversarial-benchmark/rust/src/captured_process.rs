//! Bounded capture of a child process, the `subprocess.run(command, capture_output=True, timeout=...)` of the
//! original programs: one thread serves both pipes through non-blocking reads against a single deadline that covers the
//! child's completion **and** both streams reaching end of file. Used by the image inspection of `bench_tool_pin`
//! (120 seconds) and by the topology resolver's `_run` (60 seconds by default, any positive number as a parameter).
//!
//! Nothing here decides what a failure means: the callers map [`Failure`]s (`TimeoutExpired`, `OSError` classes) to
//! the exception handling of their own original.

use crate::dataset_commands::spawn_failure;
use crate::guidellm::Failure;
use crate::pyjson::decode_file_text;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use rustix::event::{poll, PollFd, PollFlags};
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use rustix::io::{read, Errno};
use rustix::time::Timespec;
use std::ffi::OsString;
use std::os::fd::OwnedFd;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The most one read takes from a pipe before the deadline and the other stream get their turn.
pub(crate) const READ_BLOCK: usize = 65536;

/// How a captured child ended.
pub struct Captured {
    pub status: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// One captured pipe: read without blocking, so one thread can serve both streams against a single deadline.
struct Pipe {
    fd: OwnedFd,
    bytes: Vec<u8>,
    open: bool,
}

impl Pipe {
    fn new(fd: OwnedFd) -> Result<Pipe, Failure> {
        let flags = fcntl_getfl(&fd).map_err(pipe_failure)?;
        fcntl_setfl(&fd, flags | OFlags::NONBLOCK).map_err(pipe_failure)?;
        Ok(Pipe {
            fd,
            bytes: Vec::new(),
            open: true,
        })
    }

    /// One bounded non-blocking read (at most [`READ_BLOCK`] bytes): end of file closes the pipe, "no data yet" and
    /// an interrupted call return to the caller's deadline check, and nothing that was read is discarded. The caller
    /// iterates, so a continuously replenished stream never keeps it from the deadline or from the other stream.
    fn read_once(&mut self) -> Result<(), Failure> {
        if !self.open {
            return Ok(());
        }
        let mut block = [0_u8; READ_BLOCK];
        match read(&self.fd, &mut block) {
            Ok(0) => self.open = false,
            Ok(count) => self.bytes.extend_from_slice(&block[..count]),
            Err(Errno::INTR | Errno::AGAIN) => {}
            Err(error) => return Err(pipe_failure(error)),
        }
        Ok(())
    }
}

/// The child's standard input while `input` is being written: non-blocking, so one thread serves it and both output
/// streams against a single deadline.
struct InputPipe {
    fd: OwnedFd,
    pending: Vec<u8>,
    written: usize,
}

impl InputPipe {
    fn new(fd: OwnedFd, bytes: &[u8]) -> Result<InputPipe, Failure> {
        let flags = fcntl_getfl(&fd).map_err(pipe_failure)?;
        fcntl_setfl(&fd, flags | OFlags::NONBLOCK).map_err(pipe_failure)?;
        Ok(InputPipe {
            fd,
            pending: bytes.to_vec(),
            written: 0,
        })
    }

    /// One bounded write (at most `PIPE_BUF` bytes, so a full pipe never blocks): `true` when the input is finished
    /// and the pipe is to be closed. A child that closed its input early (`EPIPE`) ends the input silently, as
    /// `Popen.communicate` ignores `BrokenPipeError` of the input.
    fn write_once(&mut self) -> Result<bool, Failure> {
        let end = (self.written + PIPE_ATOMIC_WRITE).min(self.pending.len());
        match rustix::io::write(&self.fd, &self.pending[self.written..end]) {
            Ok(count) => self.written += count,
            Err(Errno::INTR | Errno::AGAIN) => {}
            Err(Errno::PIPE) => return Ok(true),
            Err(error) => return Err(pipe_failure(error)),
        }
        Ok(self.written >= self.pending.len())
    }
}

/// `select.PIPE_BUF` on Linux: the most that one write to a pipe takes atomically.
const PIPE_ATOMIC_WRITE: usize = 4096;

pub(crate) fn pipe_failure(error: Errno) -> Failure {
    let io = std::io::Error::from(error);
    Failure::new(
        crate::pyfs::os_error_class(&io),
        &crate::cli::os_error_text(&io, &PyStr::new("pipe")),
    )
}

/// `str(timeout)` as the `TimeoutExpired` message shows it: an integral number of seconds has no fraction.
pub fn seconds_text(limit: Duration) -> String {
    if limit.subsec_nanos() == 0 {
        limit.as_secs().to_string()
    } else {
        format!("{}", limit.as_secs_f64())
    }
}

/// `subprocess.run(command, capture_output=True, timeout=limit)` with the limit and the text of the caller's
/// number (`str(timeout)` of the `TimeoutExpired` message: `120`, `0.2`, `5.0`).
///
/// One deadline covers the child's completion **and** both streams reaching end of file: a descendant that
/// inherited a pipe and keeps it open after the child exited holds the call up to the deadline, exactly as
/// `Popen.communicate` does, and then ends it with `TimeoutExpired`. The child shares this process's input. On the
/// deadline the direct child is killed and reaped (never its descendants or any process group); both pipes are closed
/// when this function returns, so no reader outlives the call.
pub fn run_captured_within(
    command: &[OsString],
    shown: &[PyValue],
    limit: Duration,
    limit_text: &str,
) -> Result<Captured, Failure> {
    run_captured_in(command, shown, limit, limit_text, &ChildSettings::default())
}

/// What a caller may set on the child beyond its command line: `subprocess.run(..., cwd=..., env=...)`. The
/// environment starts as this process's own; names to remove and values to set are applied in that order.
#[derive(Default)]
pub struct ChildSettings {
    pub current_dir: Option<std::path::PathBuf>,
    pub env_remove: Vec<OsString>,
    pub env_set: Vec<(OsString, OsString)>,
    /// `subprocess.run(..., input=...)`: bytes written to the child's standard input, which is then closed. `None`
    /// leaves the child sharing this process's input.
    pub input: Option<Vec<u8>>,
    /// Run the child as the leader of its own process group and, at the deadline or on a failure, kill the whole group (the
    /// child's descendants included) instead of the child alone. For a child that starts a program of its own.
    pub own_process_group: bool,
    /// Receives the child's PID right after it started (observation only: a caller that must report the actual PID of a
    /// child that is later killed at the deadline reads it here).
    pub pid_sink: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
    /// Give the child `/dev/null` as standard input instead of sharing this process's input.
    pub null_stdin: bool,
    /// Start the child as a member of this EXISTING process group (the group of a supervisor the caller keeps and reaps) and
    /// kill that whole group at the deadline or on a failure. Takes the place of `own_process_group` when set.
    pub join_process_group: Option<i32>,
}

/// [`run_captured_within`] with the child's working directory and environment settings.
pub fn run_captured_in(
    command: &[OsString],
    shown: &[PyValue],
    limit: Duration,
    limit_text: &str,
    settings: &ChildSettings,
) -> Result<Captured, Failure> {
    run_child(command, shown, Some((limit, limit_text)), settings)
}

/// `subprocess.run(command, capture_output=True)` with **no** time limit (`timeout=None`): the same single
/// communication loop (input, both output streams) and the same cleanup, but the loop waits for the child as long as
/// it takes and the final wait for the exit status blocks instead of polling. For callers whose original has no
/// timeout; every bounded caller keeps [`run_captured_within`] / [`run_captured_in`].
pub fn run_captured_unbounded(
    command: &[OsString],
    shown: &[PyValue],
    settings: &ChildSettings,
) -> Result<Captured, Failure> {
    run_child(command, shown, None, settings)
}

/// The shared body: `limit` is the duration and the text of its `str()`, or `None` for no limit.
fn run_child(
    command: &[OsString],
    shown: &[PyValue],
    limit: Option<(Duration, &str)>,
    settings: &ChildSettings,
) -> Result<Captured, Failure> {
    let mut builder = Command::new(&command[0]);
    builder
        .args(&command[1..])
        .stdin(if settings.input.is_some() {
            Stdio::piped()
        } else if settings.null_stdin {
            Stdio::null()
        } else {
            Stdio::inherit()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(directory) = &settings.current_dir {
        builder.current_dir(directory);
    }
    for name in &settings.env_remove {
        builder.env_remove(name);
    }
    for (name, value) in &settings.env_set {
        builder.env(name, value);
    }
    if let Some(group) = settings.join_process_group {
        std::os::unix::process::CommandExt::process_group(&mut builder, group);
    } else if settings.own_process_group {
        std::os::unix::process::CommandExt::process_group(&mut builder, 0);
    }
    let mut child = builder
        .spawn()
        .map_err(|error| spawn_failure(&error, &command[0]))?;
    if let Some(sink) = &settings.pid_sink {
        sink.store(child.id(), std::sync::atomic::Ordering::SeqCst);
    }
    let kill = |child: &mut Child| {
        let group = settings
            .join_process_group
            .or(settings.own_process_group.then_some(child.id() as i32));
        if let Some(group) = group.and_then(rustix::process::Pid::from_raw) {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
        let _ = child.kill();
    };
    let deadline = limit.map(|(duration, _)| Instant::now() + duration);
    let outcome = capture_until(&mut child, deadline, &command[0], settings.input.as_deref());
    match outcome {
        Ok(Some(captured)) => Ok(captured),
        Ok(None) => {
            kill(&mut child);
            let _ = child.wait();
            let cause = PyValue::List(shown.to_vec()).repr();
            let limit_text = limit.map_or("", |(_, text)| text);
            Err(Failure::new(
                "TimeoutExpired",
                &format!("Command '{cause}' timed out after {limit_text} seconds"),
            ))
        }
        Err(failure) => {
            kill(&mut child);
            let _ = child.wait();
            Err(failure)
        }
    }
}

/// One communication loop, as `Popen.communicate` runs it: the pending input is written, both output streams are
/// read, all against one `deadline`, until the input is written and both streams are at end of file; then the exit
/// status. The input is served for as long as it is pending, **including after the child closed both outputs**
/// (a child may close its outputs and still read its input to the end). `None` as the result = the deadline passed;
/// `deadline: None` = no limit (the poll waits for readiness and the final wait blocks).
fn capture_until(
    child: &mut Child,
    deadline: Option<Instant>,
    program: &OsString,
    input: Option<&[u8]>,
) -> Result<Option<Captured>, Failure> {
    let take = |fd: Option<OwnedFd>| fd.expect("both streams are piped");
    let mut stdout = Pipe::new(take(child.stdout.take().map(OwnedFd::from)))?;
    let mut stderr = Pipe::new(take(child.stderr.take().map(OwnedFd::from)))?;
    let mut stdin = match (input, child.stdin.take()) {
        (Some(bytes), Some(pipe)) if !bytes.is_empty() => {
            Some(InputPipe::new(OwnedFd::from(pipe), bytes)?)
        }
        // an empty input (or none) closes the child's input at once, as `communicate` does
        _ => None,
    };
    while stdout.open || stderr.open || stdin.is_some() {
        let wait = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(None);
                }
                Some(Timespec {
                    tv_sec: remaining.as_secs().try_into().unwrap_or(i64::MAX),
                    tv_nsec: remaining.subsec_nanos().into(),
                })
            }
            None => None,
        };
        let mut watched: Vec<PollFd<'_>> = Vec::with_capacity(3);
        for pipe in [&stdout, &stderr] {
            if pipe.open {
                watched.push(PollFd::new(&pipe.fd, PollFlags::IN));
            }
        }
        if let Some(pipe) = &stdin {
            watched.push(PollFd::new(&pipe.fd, PollFlags::OUT));
        }
        match poll(&mut watched, wait.as_ref()) {
            Ok(_) | Err(Errno::INTR) => {}
            Err(error) => return Err(pipe_failure(error)),
        }
        if let Some(pipe) = &mut stdin {
            if pipe.write_once()? {
                stdin = None;
            }
        }
        stdout.read_once()?;
        stderr.read_once()?;
    }
    let Some(deadline) = deadline else {
        // no limit: every pipe is finished, so the child's end is one blocking wait away
        let status = child
            .wait()
            .map_err(|error| spawn_failure(&error, program))?;
        return Ok(Some(captured(status, stdout, stderr)));
    };
    let mut delay = 0.0005_f64;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                return Err(spawn_failure(&error, program));
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(None);
        }
        delay = (delay * 2.0).min(0.05).min(remaining.as_secs_f64());
        std::thread::sleep(Duration::from_secs_f64(delay));
    };
    Ok(Some(captured(status, stdout, stderr)))
}

/// The finished child: exit code, or the negated signal number (`subprocess`'s `returncode`).
fn captured(status: std::process::ExitStatus, stdout: Pipe, stderr: Pipe) -> Captured {
    use std::os::unix::process::ExitStatusExt;
    Captured {
        status: status
            .code()
            .map(i64::from)
            .or_else(|| status.signal().map(|signal| -i64::from(signal)))
            .unwrap_or(-1),
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    }
}

/// A captured stream as `text=True` yields it: strict UTF-8, universal newlines.
pub fn captured_text(bytes: &[u8]) -> Result<PyStr, Failure> {
    decode_file_text(bytes).map_err(|error| crate::repeat_api_cli::load_failure(&error))
}

#[cfg(test)]
mod tests {
    //! The production capture ([`run_captured_within`]) against real child processes (`/bin/sh`), with a short limit
    //! injected at its private boundary. Every child and every descendant has a bounded lifetime.

    use super::*;

    fn shell(script: &str, folder: &std::path::Path) -> (Vec<OsString>, Vec<PyValue>) {
        let words = ["/bin/sh", "-c", script, "sh"];
        let mut command: Vec<OsString> = words.iter().map(OsString::from).collect();
        command.push(folder.as_os_str().to_owned());
        let shown = command
            .iter()
            .map(|word| PyValue::str(&word.to_string_lossy()))
            .collect();
        (command, shown)
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let folder = std::env::temp_dir().join(format!(
            "bench_tool_capture_{label}_{}_{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&folder).expect("scratch folder");
        folder
    }

    fn recorded_pid(folder: &std::path::Path, name: &str) -> u32 {
        std::fs::read_to_string(folder.join(name))
            .expect("pid file")
            .trim()
            .parse()
            .expect("pid")
    }

    fn gone(pid: u32) -> bool {
        !std::path::Path::new(&format!("/proc/{pid}")).exists()
    }

    fn timed_out(outcome: Result<Captured, Failure>, limit_text: &str) {
        let failure = outcome.err().expect("the deadline must end the call");
        assert_eq!(failure.class, "TimeoutExpired");
        let message = failure.message.to_utf8().expect("text");
        assert!(
            message.starts_with("Command '['/bin/sh', '-c', "),
            "{message}"
        );
        assert!(
            message.ends_with(&format!("timed out after {limit_text} seconds")),
            "{message}"
        );
    }

    /// The reported defect: the direct child exits at once, a descendant keeps the inherited pipes open.
    #[test]
    fn a_descendant_holding_the_pipes_is_bounded_by_the_deadline_after_the_child_exited() {
        let folder = scratch("holder");
        let script = r#"printf '["x@sha256:a"]'; (sleep 4) & echo $$ > "$1/child.pid"; exit 0"#;
        let (command, shown) = shell(script, &folder);
        let started = Instant::now();
        let outcome = run_captured_within(&command, &shown, Duration::from_millis(300), "0.3");
        let elapsed = started.elapsed();
        timed_out(outcome, "0.3");
        assert!(elapsed >= Duration::from_millis(300), "{elapsed:?}");
        assert!(elapsed < Duration::from_millis(2500), "{elapsed:?}");
        assert!(
            gone(recorded_pid(&folder, "child.pid")),
            "the child is reaped"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A live direct child that never answers is killed and reaped at the deadline.
    #[test]
    fn a_silent_live_child_is_killed_and_reaped_at_the_deadline() {
        let folder = scratch("live");
        let (command, shown) = shell(r#"echo $$ > "$1/child.pid"; exec sleep 4"#, &folder);
        let started = Instant::now();
        let outcome = run_captured_within(&command, &shown, Duration::from_millis(300), "0.3");
        let elapsed = started.elapsed();
        timed_out(outcome, "0.3");
        assert!(elapsed < Duration::from_millis(2500), "{elapsed:?}");
        assert!(
            gone(recorded_pid(&folder, "child.pid")),
            "the child is reaped"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A child that closes both streams but keeps running is bounded as well (completion is under the deadline too).
    #[test]
    fn a_child_that_closed_its_streams_but_runs_on_is_bounded_by_the_deadline() {
        let folder = scratch("closed");
        let script = r#"exec >&- 2>&-; echo $$ > "$1/child.pid"; exec sleep 4"#;
        let (command, shown) = shell(script, &folder);
        let started = Instant::now();
        let outcome = run_captured_within(&command, &shown, Duration::from_millis(300), "0.3");
        timed_out(outcome, "0.3");
        assert!(started.elapsed() < Duration::from_millis(2500));
        assert!(gone(recorded_pid(&folder, "child.pid")));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Positive controls: normal completion keeps both streams whole (more than a pipe buffer), the exit status, the
    /// signal number, and does not wait for a descendant that holds no pipe.
    #[test]
    fn normal_completion_keeps_whole_streams_and_the_exit_status() {
        let folder = scratch("normal");
        let generous = Duration::from_secs(30);
        let script = r#"printf out; printf err >&2; head -c 300000 /dev/zero | tr '\0' a; exit 3"#;
        let (command, shown) = shell(script, &folder);
        let done = run_captured_within(&command, &shown, generous, "30").expect("completes");
        assert_eq!(done.status, 3);
        assert_eq!(done.stderr, b"err");
        assert_eq!(done.stdout.len(), 3 + 300_000);
        assert!(done.stdout.starts_with(b"out") && done.stdout.ends_with(b"aaaa"));

        let (command, shown) = shell("kill -9 $$", &folder);
        let killed = run_captured_within(&command, &shown, generous, "30").expect("completes");
        assert_eq!(killed.status, -9);

        let started = Instant::now();
        let script = r#"printf ok; (sleep 3 >/dev/null 2>&1 </dev/null &) ; exit 0"#;
        let (command, shown) = shell(script, &folder);
        let done = run_captured_within(&command, &shown, generous, "30").expect("completes");
        assert_eq!((done.status, done.stdout.as_slice()), (0, &b"ok"[..]));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "no wait for the descendant"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A producer that keeps replenishing stdout faster than it is read (a bounded 64 MB at most, written by the
    /// direct child itself, which the deadline kills): the sole deadline still ends the call, promptly.
    #[test]
    fn a_continuously_replenished_stream_does_not_defeat_the_deadline() {
        let folder = scratch("flood");
        let script = r#"echo $$ > "$1/child.pid"; i=0; while [ $i -lt 1000000 ]; do printf '%063d\n' 0; i=$((i+1)); done"#;
        let (command, shown) = shell(script, &folder);
        let started = Instant::now();
        let outcome = run_captured_within(&command, &shown, Duration::from_millis(100), "0.1");
        let elapsed = started.elapsed();
        timed_out(outcome, "0.1");
        assert!(elapsed < Duration::from_millis(1500), "{elapsed:?}");
        assert!(
            gone(recorded_pid(&folder, "child.pid")),
            "the producer is reaped"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// One read takes at most one block, so a stream that is always ready cannot hold the loop: after a single call
    /// the other stream and the deadline are consulted. Nothing is lost - the bytes accumulate across calls.
    #[test]
    fn one_read_is_bounded_and_keeps_every_byte() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "head -c 1000000 /dev/zero"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn");
        let mut pipe = Pipe::new(OwnedFd::from(child.stdout.take().expect("piped"))).expect("pipe");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut calls = 0_usize;
        while pipe.open && Instant::now() < deadline {
            let before = pipe.bytes.len();
            pipe.read_once().expect("read");
            assert!(
                pipe.bytes.len() - before <= READ_BLOCK,
                "one call took more than a block"
            );
            calls += 1;
        }
        assert!(!pipe.open, "end of file reached");
        assert_eq!(pipe.bytes.len(), 1_000_000);
        assert!(calls >= 1_000_000 / READ_BLOCK, "{calls} calls");
        let _ = child.wait();
    }

    /// More than any pipe holds (the default is 64 KiB), and not a repeating block, so a lost or reordered chunk shows.
    fn large_input() -> Vec<u8> {
        (0..1_048_576_u32).map(|n| (n % 251) as u8).collect()
    }

    fn settings_with(input: Vec<u8>) -> ChildSettings {
        ChildSettings {
            input: Some(input),
            ..ChildSettings::default()
        }
    }

    /// The regression of the first R14 build: a child closes both of its outputs and then reads its input to the end.
    /// Once the outputs are at end of file the loop must keep serving the pending input; a loop that only runs while an
    /// output is open stops after the first pipe-full and the child waits for the rest until the deadline.
    #[test]
    fn input_is_still_served_after_the_child_closed_both_outputs() {
        let folder = scratch("input-after-outputs");
        let script = r#"exec >&- 2>&-; cat > "$1/received.bin""#;
        let (command, shown) = shell(script, &folder);
        let input = large_input();
        let started = Instant::now();
        let done = run_captured_in(
            &command,
            &shown,
            Duration::from_secs(10),
            "10",
            &settings_with(input.clone()),
        )
        .expect("completes before the limit");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
        assert_eq!(done.status, 0);
        assert!(done.stdout.is_empty() && done.stderr.is_empty());
        let received = std::fs::read(folder.join("received.bin")).expect("received input");
        assert_eq!(received.len(), input.len(), "every byte reached the child");
        assert!(received == input, "bytes arrived intact and in order");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The same child with a limit it cannot meet is still ended at the limit (the input service is under the one
    /// deadline): a child that closes its outputs and never reads is killed and reaped.
    #[test]
    fn an_input_nobody_reads_is_bounded_by_the_deadline_after_the_outputs_closed() {
        let folder = scratch("input-unread");
        let script = r#"exec >&- 2>&-; echo $$ > "$1/child.pid"; exec sleep 4"#;
        let (command, shown) = shell(script, &folder);
        let started = Instant::now();
        let outcome = run_captured_in(
            &command,
            &shown,
            Duration::from_millis(300),
            "0.3",
            &settings_with(large_input()),
        );
        timed_out(outcome, "0.3");
        assert!(started.elapsed() < Duration::from_millis(2500));
        assert!(gone(recorded_pid(&folder, "child.pid")));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Unchanged semantics around the input: an empty input is an immediate end of file, a child that closes its input
    /// at once ends the input silently (no error from the broken pipe), and a child that echoes its input back
    /// while it is being written (output and input interleave in the one loop) returns it whole.
    #[test]
    fn empty_input_early_close_and_echo_keep_their_semantics() {
        let folder = scratch("input-forms");
        let generous = Duration::from_secs(30);
        let (command, shown) = shell(r#"cat > "$1/received.bin""#, &folder);
        let done = run_captured_in(&command, &shown, generous, "30", &settings_with(Vec::new()))
            .expect("completes");
        assert_eq!(done.status, 0);
        assert_eq!(
            std::fs::read(folder.join("received.bin")).expect("file"),
            b""
        );

        let (command, shown) = shell("exec <&-; printf done", &folder);
        let done = run_captured_in(
            &command,
            &shown,
            generous,
            "30",
            &settings_with(large_input()),
        )
        .expect("a closed input is not an error");
        assert_eq!((done.status, done.stdout.as_slice()), (0, &b"done"[..]));

        let (command, shown) = shell("cat", &folder);
        let input = large_input();
        let done = run_captured_in(
            &command,
            &shown,
            generous,
            "30",
            &settings_with(input.clone()),
        )
        .expect("completes");
        assert_eq!(done.status, 0);
        assert!(done.stdout == input, "the echo is whole");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The no-limit path (the original's call without `timeout`): the same loop serves input and outputs and the
    /// final wait blocks. The child closes its outputs, reads everything, keeps running for a while and ends with a
    /// status; nothing here depends on a limit.
    #[test]
    fn the_unbounded_capture_serves_input_and_waits_for_the_child() {
        let folder = scratch("unbounded");
        let script = r#"exec >&- 2>&-; cat > "$1/received.bin"; sleep 0.4; exit 5"#;
        let (command, shown) = shell(script, &folder);
        let input = large_input();
        let started = Instant::now();
        let done = run_captured_unbounded(&command, &shown, &settings_with(input.clone()))
            .expect("completes");
        assert_eq!(done.status, 5);
        assert!(
            started.elapsed() >= Duration::from_millis(400),
            "the wait was not cut short"
        );
        let received = std::fs::read(folder.join("received.bin")).expect("received input");
        assert!(received == input, "every byte arrived intact");

        let (command, shown) = shell("cat; printf err >&2; exit 2", &folder);
        let done = run_captured_unbounded(&command, &shown, &settings_with(input.clone()))
            .expect("completes");
        assert_eq!((done.status, done.stderr.as_slice()), (2, &b"err"[..]));
        assert!(done.stdout == input);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
