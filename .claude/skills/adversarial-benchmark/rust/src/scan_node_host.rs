//! The real outside of the scanner probes: the production implementation of [`role_contract::scan_node_probe::Host`]. Every method
//! does what its Python counterpart did, against this machine: `shutil.which` walks `PATH`, `subprocess.run` starts the real
//! program (stdout and stderr captured, the `input` fed on a pipe, killed on timeout), `os.listdir`/`open` read the real files,
//! `socket.create_connection`/`getaddrinfo` use the real network, `urllib` HEAD is a real HTTPS request. Nothing here decides
//! anything about a scan result; the decisions are in `role_contract::scan_node_probe`.
//!
//! **Known differences from CPython** (each unmeasured by the closed P23 rows, none hidden): the child's standard input is
//! `/dev/null` (the original inherited the caller's), a child whose grandchild keeps a pipe open after the kill leaves the capture
//! bounded by one shared 2 s drain (CPython would wait for the pipe to close), the HTTPS client is `ureq`/rustls (the text of its errors is never used), the
//! proxy of the environment is honoured but `no_proxy` is matched only by the exact host, `getaddrinfo` returns one entry per
//! address (CPython lists every socket type), and the text encoding of the locale is taken to be UTF-8.

use crate::guidellm_release_http::{load_trust_roots, select_trust_sources, HttpEnvironment};
use agent_runtime_support::pystr::PyStr;
use role_contract::scan_node_probe::{Capture, Host};
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::Agent;

/// The plumbing variables the adapter/launcher add, hide or remove, with the carrier that holds the CALLER's value of each.
const PLUMBING: [(&str, &str); 3] = [
    ("SCAN_NODE_BRIDGE", "SCAN_NODE_CALLER_BRIDGE"),
    ("SCAN_NODE_OWNER_PATH", "SCAN_NODE_CALLER_OWNER_PATH"),
    ("SCAN_NODE_VALIDATE_ONLY", "SCAN_NODE_CALLER_VALIDATE_ONLY"),
];
const CALLER_FLAG: &str = "SCAN_NODE_CALLER_ENV";

/// A command the native probes start sees the environment of the CALLER of the program (what the original's child saw): when a launcher/adapter
/// start recorded the caller's values (`SCAN_NODE_CALLER_ENV`), each plumbing variable is first removed from the child and then restored from its
/// carrier if the caller had it (an empty or explicit user value included), and the carriers never reach the child. A start without a
/// transport (the executable run directly) has nothing to undo: the environment is inherited untouched.
fn caller_environment(command: &mut Command) {
    if std::env::var_os(CALLER_FLAG).is_none() {
        return;
    }
    for (name, carrier) in PLUMBING {
        command.env_remove(name);
        if let Some(value) = std::env::var_os(carrier) {
            command.env(name, value);
        }
        command.env_remove(carrier);
    }
    command.env_remove(CALLER_FLAG);
}

/// `os.defpath`, used when `PATH` is not set.
const DEFPATH: &[u8] = b"/bin:/usr/bin";

/// A process-wide host. The effects that are *not* returned (`print(..., file=sys.stderr)`, `PEER_SSH_LAST_ERROR[...] = ...`) are
/// either written at once (`direct`, the command line) or collected for the bridge to hand to the importer (`collected`).
pub struct StdHost {
    direct_stderr: bool,
    lines: Mutex<Vec<PyStr>>,
    errors: Mutex<Vec<(String, PyStr)>>,
}

impl StdHost {
    /// The command line: the diagnostic goes to this process's standard error immediately.
    pub fn direct() -> StdHost {
        StdHost {
            direct_stderr: true,
            lines: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
        }
    }

    /// The private bridge: effects are collected and returned with the answer.
    pub fn collecting() -> StdHost {
        StdHost {
            direct_stderr: false,
            lines: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
        }
    }

    /// The collected `print(..., file=sys.stderr)` texts, in order.
    pub fn take_stderr_lines(&self) -> Vec<PyStr> {
        std::mem::take(
            &mut *self
                .lines
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// The collected `PEER_SSH_LAST_ERROR` assignments, in order.
    pub fn take_peer_errors(&self) -> Vec<(String, PyStr)> {
        std::mem::take(
            &mut *self
                .errors
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

fn is_executable_file(path: &Path) -> bool {
    use rustix::fs::{access, Access};
    match std::fs::metadata(path) {
        Ok(meta) => !meta.is_dir() && access(path, Access::EXEC_OK).is_ok(),
        Err(_) => false,
    }
}

fn which_in(program: &[u8], path: Option<&[u8]>) -> bool {
    if program.is_empty() {
        return false;
    }
    if program.contains(&b'/') {
        return is_executable_file(Path::new(OsStr::from_bytes(program)));
    }
    let search = path.unwrap_or(DEFPATH);
    if search.is_empty() {
        return false;
    }
    search.split(|&b| b == b':').any(|dir| {
        let mut full = dir.to_vec();
        if !dir.is_empty() {
            full.push(b'/');
        }
        full.extend_from_slice(program);
        is_executable_file(Path::new(OsStr::from_bytes(&full)))
    })
}

fn errno_class(code: i32) -> &'static str {
    match code {
        2 => "FileNotFoundError",
        1 | 13 => "PermissionError",
        20 => "NotADirectoryError",
        _ => "OSError",
    }
}

fn spawn_failure(error: &std::io::Error, program: &[u8]) -> Capture {
    let code = error.raw_os_error().unwrap_or(0);
    let shown = error.to_string();
    let message = shown
        .split(" (os error")
        .next()
        .unwrap_or(&shown)
        .to_string();
    let subject = PyStr::from_surrogateescape(program).repr();
    Capture::Failed {
        class: errno_class(code).to_string(),
        message: format!("[Errno {code}] {message}: {subject}"),
    }
}

fn drain(mut stream: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        bytes
    })
}

fn join_capped(handle: std::thread::JoinHandle<Vec<u8>>, cap: Duration) -> Vec<u8> {
    let started = Instant::now();
    while !handle.is_finished() && started.elapsed() < cap {
        std::thread::sleep(Duration::from_millis(5));
    }
    if handle.is_finished() {
        handle.join().unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn duration_of(seconds: Option<f64>) -> Option<Duration> {
    seconds.map(|value| {
        if value.is_finite() && value > 0.0 {
            Duration::from_secs_f64(value.min(1.0e9))
        } else {
            Duration::from_nanos(1)
        }
    })
}

impl Host for StdHost {
    fn which(&self, program: &[u8]) -> bool {
        let path = std::env::var_os("PATH");
        which_in(program, path.as_ref().map(|value| value.as_bytes()))
    }

    fn capture(&self, argv: &[Vec<u8>], stdin: Option<&[u8]>, timeout: Option<f64>) -> Capture {
        let Some(program) = argv.first() else {
            return Capture::Failed {
                class: "IndexError".into(),
                message: "list index out of range".into(),
            };
        };
        let mut command = Command::new(OsStr::from_bytes(program));
        command.args(argv[1..].iter().map(|arg| OsStr::from_bytes(arg)));
        caller_environment(&mut command);
        command.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return spawn_failure(&error, program),
        };
        let writer = child.stdin.take().map(|mut pipe| {
            let input = stdin.unwrap_or_default().to_vec();
            std::thread::spawn(move || {
                let _ = pipe.write_all(&input);
            })
        });
        let out = child.stdout.take().map(drain);
        let err = child.stderr.take().map(drain);
        let deadline = duration_of(timeout).map(|limit| Instant::now() + limit);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(_) => break None,
            }
            if deadline.is_some_and(|end| Instant::now() >= end) {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        // one shared drain bound (2 s) for both pipes: a grandchild that keeps a pipe open must not hold the scan (CPython would wait for EOF)
        let drain_end = Instant::now() + Duration::from_secs(2);
        let remaining = || drain_end.saturating_duration_since(Instant::now());
        let stdout = out
            .map(|handle| join_capped(handle, remaining()))
            .unwrap_or_default();
        let stderr = err
            .map(|handle| join_capped(handle, remaining()))
            .unwrap_or_default();
        if let Some(handle) = writer {
            let _ = handle.join();
        }
        match status {
            Some(status) => Capture::Exited {
                code: status
                    .code()
                    .unwrap_or_else(|| -status.signal().unwrap_or(0)),
                stdout,
                stderr,
            },
            None => Capture::TimedOut,
        }
    }

    fn list_dir(&self, path: &[u8]) -> Option<Vec<Vec<u8>>> {
        let entries = std::fs::read_dir(Path::new(OsStr::from_bytes(path))).ok()?;
        Some(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().as_bytes().to_vec())
                .collect(),
        )
    }

    fn read_file(&self, path: &[u8]) -> Option<Vec<u8>> {
        std::fs::read(Path::new(OsStr::from_bytes(path))).ok()
    }

    fn tcp_connect(&self, host: &str, port: u16, timeout: Option<f64>) -> bool {
        let Ok(addresses) = (host, port).to_socket_addrs() else {
            return false;
        };
        let limit = duration_of(timeout);
        for address in addresses {
            let outcome = match limit {
                Some(limit) => TcpStream::connect_timeout(&address, limit),
                None => TcpStream::connect(address),
            };
            if outcome.is_ok() {
                return true;
            }
        }
        false
    }

    fn resolve(&self, name: &str) -> Option<Vec<String>> {
        let addresses = (name, 0u16).to_socket_addrs().ok()?;
        Some(addresses.map(|address| address.ip().to_string()).collect())
    }

    fn https_head_ok(&self, authority: &str, timeout: Option<f64>) -> bool {
        let environment = HttpEnvironment::from_process();
        let roots = load_trust_roots(&select_trust_sources(&environment));
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(RootCerts::new_with_certs(&roots))
            .build();
        let limit = duration_of(timeout);
        let host_only = authority.split(':').next().unwrap_or(authority);
        let proxy = environment
            .https_proxy
            .as_deref()
            .filter(|_| {
                !environment
                    .no_proxy
                    .as_deref()
                    .is_some_and(|list| list.split(',').any(|entry| entry.trim() == host_only))
            })
            .and_then(|setting| ureq::Proxy::new(setting).ok());
        let config = Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(10)
            .max_redirects_will_error(true)
            .timeout_per_read(limit)
            .timeout_per_write(limit)
            .timeout_connect(limit)
            .proxy(proxy)
            .tls_config(tls)
            .build();
        let agent = Agent::new_with_config(config);
        match agent.head(format!("https://{authority}")).call() {
            Ok(response) => (200..300).contains(&response.status().as_u16()),
            Err(_) => false,
        }
    }

    fn machine(&self) -> String {
        rustix::system::uname()
            .machine()
            .to_string_lossy()
            .into_owned()
    }

    fn stderr_line(&self, text: &PyStr) {
        if self.direct_stderr {
            let mut bytes = text.to_backslashreplace_bytes();
            bytes.push(b'\n');
            let _ = std::io::stderr().write_all(&bytes);
        } else {
            self.lines
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(text.clone());
        }
    }

    fn peer_error(&self, what: &str, cause: &PyStr) {
        self.errors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((what.to_string(), cause.clone()));
    }
}
