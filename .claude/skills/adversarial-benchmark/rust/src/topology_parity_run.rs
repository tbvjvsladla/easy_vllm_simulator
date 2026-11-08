//! The program body of the private helper `topology_parity`: the owner CLI of `topology_parity.py` over the native predicate,
//! or - for a source that is not the reviewed one - the *original program itself* on an explicit compatibility route.
//!
//! ```text
//! topology_parity [PRIVATE FLAGS] <owner CLI args>
//!   --source-owner PATH       logical owner path shown in traceback frames: metadata only, never read, never run, never a
//!                             substitute for the real program
//!   --compat-owner PATH       the REAL original program this run stands for: its bytes decide the route, and it is what the
//!                             compatibility route runs; a named program that does not exist is an unmet condition
//!   --compat-python INTERP    the explicitly configured interpreter of the compatibility route
//!   --compat-deadline-ms N    private transport limit of the compatibility child in ms (default 60000; 0 = an already expired
//!                             deadline; the resolver passes 50000, strictly below its own 60 s limit)
//!   --route-sidecar PATH      optional exclusive private observation file (JSON)
//! ```
//!
//! **Routes.** `NativeCurrentSource`: the named owner and the checkout's `campaign_init.py` are byte-identical to the reviewed
//! canonical originals (or the campaign module is absent, which the original treats as an absent leg): the native predicate
//! answers and no Python runs. `LegacySourceVariant`: one of them is a different program: `INTERP OWNER <args>` runs with the
//! same args, environment and standard input through the bounded capture (own process group, killed with all descendants at
//! the limit); its raw stdout, stderr and status are forwarded unchanged. Without `--compat-python`, without a real
//! `--compat-owner`, or when the owner named does not exist, the run is a concrete unmet condition (status 2); a compat child
//! that hits the transport limit or cannot be run is a control failure (status 2), never an original business result.
//! Route, child PID, interpreter, owner, status or signal and control failures go only to the sidecar - never into the business
//! bytes.
//!
//! **Nested cancellation.** A private watchdog (this binary again: `--compat-watchdog HELPER_PID`) is started first in a process
//! group of its own and leads the compat group; the compat Python joins that group, so watchdog, Python and descendants share
//! one group. The watchdog polls its parent PID every 20 ms and, when it is no longer the helper (outer deadline kill of the
//! helper's group, SIGKILL, crash; a reparent to a subreaper counts too), kills the whole group including itself: a hung changed
//! source never outlives the helper. The helper kills the group at its own inner deadline, on a control failure and after
//! normal completion (leftover descendants), then reaps the watchdog (an unreaped child keeps its PID, so the group ID cannot be
//! reused underneath the kill). Not proven: a SIGKILL aimed at the watchdog itself (outside the supervised failure set) and
//! any whole-host claim.
//!
//! **Output.** Business bytes go out through the interpreter's stdout model ([`crate::stdout`]): a write that fails at once is
//! the traceback and status 1 of CPython, a failure when the buffered rest is flushed is its status 120. A compat child that was
//! killed by a signal is propagated: the helper raises the same signal on itself after delivering the output (`Popen`
//! returncode `-N` for the caller); where that signal does not end the process the status is `128+N` and the sidecar says so.
//! Status 2 outside argparse usage errors is this helper's private domain (unsupported text profile, platform limit, unmet
//! compatibility, control failure).

use crate::captured_process::{run_captured_in, ChildSettings};
use crate::cli::{write_stderr, Streams};
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::repeat_api_cli::print_failed;
use role_contract::pyprofile::TextProfile;
use role_contract::source_identity::{route, Route};
use role_contract::topology_cli::{evaluate_target, run as run_native, Environment};
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_OWNER: &[u8] = b".claude/policies/runtime/topology_parity.py";
/// The private transport limit of a compatibility child when none is given (the resolver's `timeout=60`).
pub const COMPAT_LIMIT_MS: u64 = 60_000;
/// How often the watchdog compares its parent PID.
const WATCHDOG_POLL: Duration = Duration::from_millis(20);

/// The watchdog mode (`--compat-watchdog HELPER_PID`): never returns. It is ended by the helper's group kill, or it kills its own
/// group (watchdog, compat Python, descendants) once its parent is no longer the helper that started it.
pub fn watchdog(helper: i32) -> ! {
    loop {
        if rustix::process::getppid().map(|p| p.as_raw_nonzero().get()) != Some(helper) {
            let own = rustix::process::getpid();
            let _ = rustix::process::kill_process_group(own, rustix::process::Signal::KILL);
            std::process::exit(137);
        }
        std::thread::sleep(WATCHDOG_POLL);
    }
}

#[derive(Default)]
struct Private {
    source_owner: Option<Vec<u8>>,
    compat_owner: Option<Vec<u8>>,
    compat_python: Option<Vec<u8>>,
    compat_deadline_ms: Option<Vec<u8>>,
    sidecar: Option<Vec<u8>>,
}

/// Splits the private leading flags from the owner's command line.
fn split_private(mut args: Vec<Vec<u8>>) -> (Private, Vec<Vec<u8>>) {
    let mut private = Private::default();
    loop {
        let slot = match args.first().map(Vec::as_slice) {
            Some(b"--source-owner") => &mut private.source_owner,
            Some(b"--compat-owner") => &mut private.compat_owner,
            Some(b"--compat-python") => &mut private.compat_python,
            Some(b"--compat-deadline-ms") => &mut private.compat_deadline_ms,
            Some(b"--route-sidecar") => &mut private.sidecar,
            _ => break,
        };
        if args.len() < 2 {
            break;
        }
        *slot = Some(args[1].clone());
        args.drain(..2);
    }
    (private, args)
}

fn json_text(bytes: &[u8]) -> String {
    crate::pyvalue::PyValue::Str(PyStr::from_surrogateescape(bytes))
        .dumps_ascii()
        .unwrap_or_else(|_| "\"?\"".to_string())
}

/// The sidecar record (exclusive creation; a failure to create it is the helper's private error, not a business result).
fn write_sidecar(path: &[u8], fields: &[(&str, String)]) -> Result<(), String> {
    use std::io::Write;
    let body: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("\"{k}\": {v}"))
        .collect();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(std::path::Path::new(std::ffi::OsStr::from_bytes(path)))
        .map_err(|error| format!("route sidecar cannot be created: {error}"))?;
    file.write_all(format!("{{{}}}\n", body.join(", ")).as_bytes())
        .map_err(|error| error.to_string())
}

fn private_error(io: &mut Streams<'_>, text: &str) -> i32 {
    write_stderr(io, &PyStr::new(&format!("topology_parity: {text}\n")));
    io.finish(2)
}

/// Delivers business bytes: stderr directly (a failure is the exit-time 120), stdout through the interpreter model: a write that
/// fails at once is CPython's traceback and status 1, a failure of the final flush its status 120.
fn deliver(io: &mut Streams<'_>, stdout: &[u8], stderr: &[u8], status: i32) -> i32 {
    if io.stderr.write_all(stderr).is_err() || io.stderr.flush().is_err() {
        return 120;
    }
    if !stdout.is_empty() {
        if let Err(failure) = io.write_plain(&PyStr::from_surrogateescape(stdout)) {
            let status = print_failed(io, &failure);
            return io.finish(status);
        }
    }
    io.finish(status)
}

/// Runs the helper: `args` excludes the program name; `env` reads one environment variable.
pub fn run(args: Vec<Vec<u8>>, io: &mut Streams<'_>, env: &dyn Fn(&str) -> Option<Vec<u8>>) -> i32 {
    let (private, args) = split_private(args);
    let profile = match TextProfile::from_environment(env) {
        Ok(profile) => profile,
        Err(why) => return private_error(io, &format!("unsupported text profile: {why}")),
    };
    let environment = Environment {
        profile,
        source_owner: private
            .source_owner
            .clone()
            .unwrap_or_else(|| DEFAULT_OWNER.to_vec()),
        tmpdir: env("TMPDIR"),
    };
    // the route: which program the sources of this run call for
    let root = evaluate_target(&args).map(|repo| {
        role_contract::pyprofile::with_profile(&environment.profile, || {
            role_contract::topology::resolve(&repo)
        })
    });
    let sidecar = |fields: &[(&str, String)]| -> Result<(), String> {
        match &private.sidecar {
            Some(path) => write_sidecar(path, fields),
            None => Ok(()),
        }
    };
    match route(private.compat_owner.as_deref(), root.as_deref()) {
        Route::NativeCurrentSource => {
            if let Err(why) = sidecar(&[
                ("route", "\"NativeCurrentSource\"".to_string()),
                ("native", "true".to_string()),
                ("python_children", "0".to_string()),
            ]) {
                return private_error(io, &why);
            }
            let out = run_native(&args, &environment);
            deliver(io, &out.stdout, &out.stderr, out.status)
        }
        Route::Unmet { reason } => {
            let _ = sidecar(&[
                ("route", "\"UnmetCompatibility\"".to_string()),
                ("reason", json_text(reason.as_bytes())),
            ]);
            private_error(
                io,
                &format!("source version compatibility cannot be served ({reason})"),
            )
        }
        Route::LegacySourceVariant { reason } => {
            // the REAL program only: the logical --source-owner is metadata and never stands in for it
            let (Some(python), Some(owner)) =
                (private.compat_python.clone(), private.compat_owner.clone())
            else {
                let _ = sidecar(&[
                    ("route", "\"UnmetCompatibility\"".to_string()),
                    ("reason", json_text(reason.as_bytes())),
                ]);
                return private_error(io, &format!("source version compatibility is required ({reason}) but no explicit interpreter (--compat-python) and real original program (--compat-owner) are configured; the current-source answer is not given"));
            };
            let limit_ms = match private
                .compat_deadline_ms
                .as_deref()
                .map(|v| String::from_utf8_lossy(v).parse::<u64>())
            {
                None => COMPAT_LIMIT_MS,
                Some(Ok(ms)) => ms,
                Some(Err(_)) => {
                    return private_error(
                        io,
                        "--compat-deadline-ms must be a non-negative integer (0 is an already expired deadline)",
                    )
                }
            };
            // the supervising watchdog first: its own process group, leader of the compat group
            let exe = match std::env::current_exe() {
                Ok(exe) => exe,
                Err(error) => {
                    return private_error(
                        io,
                        &format!("the helper cannot find itself to start its watchdog: {error}"),
                    )
                }
            };
            let mut watchdog = match std::process::Command::new(&exe)
                .arg("--compat-watchdog")
                .arg(std::process::id().to_string())
                .process_group(0)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(child) => child,
                Err(error) => {
                    return private_error(
                        io,
                        &format!("the compatibility watchdog cannot be started: {error}"),
                    )
                }
            };
            let watchdog_pid = watchdog.id();
            let mut words: Vec<OsString> = vec![
                OsString::from(std::ffi::OsStr::from_bytes(&python)),
                OsString::from(std::ffi::OsStr::from_bytes(&owner)),
            ];
            words.extend(
                args.iter()
                    .map(|a| OsString::from(std::ffi::OsStr::from_bytes(a))),
            );
            let shown: Vec<PyValue> = words
                .iter()
                .map(|w| PyValue::Str(PyStr::from_surrogateescape(w.as_bytes())))
                .collect();
            let pid = Arc::new(AtomicU32::new(0));
            let settings = ChildSettings {
                join_process_group: Some(watchdog_pid as i32),
                pid_sink: Some(pid.clone()),
                ..ChildSettings::default()
            };
            let limit_text = format!("{}", limit_ms as f64 / 1000.0);
            let outcome = run_captured_in(
                &words,
                &shown,
                Duration::from_millis(limit_ms),
                &limit_text,
                &settings,
            );
            let child_pid = pid.load(Ordering::SeqCst);
            // leftovers and the watchdog: kill the whole compat group (the watchdog is still our unreaped child, so its PID is
            // not reusable), then reap it
            if let Some(group) = rustix::process::Pid::from_raw(watchdog_pid as i32) {
                let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
            }
            let _ = watchdog.kill();
            let _ = watchdog.wait();
            let captured = match outcome {
                Ok(captured) => captured,
                Err(failure) => {
                    // a timeout or a control failure of the transport: incomplete, never an original business result
                    let _ = sidecar(&[
                        ("route", "\"LegacySourceVariant\"".to_string()),
                        ("watchdog_pid", watchdog_pid.to_string()),
                        ("compat_group", watchdog_pid.to_string()),
                        ("native", "false".to_string()),
                        (
                            "python_children",
                            if child_pid == 0 { "0" } else { "1" }.to_string(),
                        ),
                        (
                            "child_pid",
                            if child_pid == 0 {
                                "null".to_string()
                            } else {
                                child_pid.to_string()
                            },
                        ),
                        ("control_failure", json_text(failure.class.as_bytes())),
                        ("complete", "false".to_string()),
                        ("limit_ms", limit_ms.to_string()),
                        ("interpreter", json_text(&python)),
                        ("owner", json_text(&owner)),
                        ("reason", json_text(reason.as_bytes())),
                    ]);
                    return private_error(
                        io,
                        &format!(
                            "the compatibility child did not complete ({}: {}); no business result",
                            failure.class,
                            failure.message.to_utf8().unwrap_or_default()
                        ),
                    );
                }
            };
            let signal = (captured.status < 0).then(|| (-captured.status) as i32);
            let status = if captured.status < 0 {
                128 + (-captured.status) as i32
            } else {
                captured.status as i32
            };
            let mut record = vec![
                ("route", "\"LegacySourceVariant\"".to_string()),
                ("watchdog_pid", watchdog_pid.to_string()),
                ("compat_group", watchdog_pid.to_string()),
                ("native", "false".to_string()),
                ("python_children", "1".to_string()),
                ("child_pid", child_pid.to_string()),
                ("child_status", captured.status.to_string()),
                ("interpreter", json_text(&python)),
                ("owner", json_text(&owner)),
                ("reason", json_text(reason.as_bytes())),
                ("complete", "true".to_string()),
            ];
            if let Some(number) = signal {
                record.push(("child_signal", number.to_string()));
            }
            if let Err(why) = sidecar(&record) {
                return private_error(io, &why);
            }
            let delivered = deliver(
                io,
                &captured.stdout,
                &captured.stderr,
                if signal.is_some() { 0 } else { status },
            );
            if let Some(number) = signal {
                if delivered == 0 {
                    // propagate the child's signal as the caller's `Popen` would have seen it (-N)
                    if let Some(named) = rustix::process::Signal::from_named_raw(number) {
                        let _ = rustix::process::kill_process(rustix::process::getpid(), named);
                        std::thread::sleep(Duration::from_millis(200));
                    }
                    return 128 + number;
                }
            }
            delivered
        }
    }
}
