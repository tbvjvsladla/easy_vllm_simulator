//! The command line `resolve_torch_pin.py` as a process: `main()` over `sys.argv`, the hash randomization the interpreter would have started with,
//! the status the interpreter would end with and the report of an exception nothing handles. Standard output is written live through CPython's finite
//! checked buffering ([`LiveConsole`]).
//!
//! **Entry.** The executable may be copied to a path that ends in `resolve_torch_pin.py` and executed directly (no interpreter): `argv[0]` is then the
//! logical script path and the frames of an uncaught exception name it, exactly as the interpreter names the script it runs. Under any other file
//! name the usage text still says `resolve_torch_pin.py` and the frames name that canonical name. A relative script path is made absolute against the
//! working directory (the interpreter's `__file__` is absolute); symbolic links are not resolved.
//!
//! **Hash randomization.** `PYTHONHASHSEED` selects the secret of `str` hashing once per process, before `main` runs ([`hash_secret_for_process`]):
//! unset, empty or `random` fills it from the operating system, a plain integer in `[0; 4294967295]` fixes it (`0` disables randomization). An invalid
//! value is refused by name (status 70). The `-R` interpreter option and the other start-up options are not part of this program.

use crate::cli::exit_status;
use crate::fetch_https::{fetch_https_text_of, FetchEnvironment};
use crate::host::Sink;
use crate::http_direct::UreqDialer;
use crate::live::LiveConsole;
use crate::pyhash::{os_entropy, parse_hash_seed, HashSecret};
use crate::thrown::Thrown;
use crate::torch_main::{main, Fetcher, Refusal, Stop, TorchWorld, PROGRAM, REFUSAL_STATUS};
use crate::torch_traceback::{render, Site};
use agent_runtime_support::pystr::PyStr;
use std::os::unix::ffi::OsStrExt;

/// How a run ended, with what its report needs.
pub struct Run {
    end: Result<(), Stop>,
    frames: Vec<Site>,
    script: PyStr,
}

/// The file name the script's own frames show: the caller's `argv[0]` when it names the script, else the canonical logical name.
fn script_file(argv0: Option<&PyStr>) -> PyStr {
    let Some(path) = argv0.filter(|path| crate::main_cmd::basename(path) == PROGRAM) else {
        return PyStr::new(PROGRAM);
    };
    let is_relative = path.code_points().first() != Some(&u32::from('/'));
    let has_directory = path.code_points().contains(&u32::from('/'));
    match std::env::current_dir() {
        Ok(cwd) if is_relative && has_directory => {
            let mut absolute = PyStr::from_surrogateescape(cwd.as_os_str().as_bytes());
            absolute.push_str("/");
            absolute.push_py(path);
            absolute
        }
        _ => path.clone(),
    }
}

/// Runs `main()` over `argv` (the whole `sys.argv`).
pub fn run(
    host: &mut dyn Sink,
    argv: &[PyStr],
    columns: Option<&str>,
    secret: &HashSecret,
    fetch: Fetcher<'_>,
) -> Run {
    let script = script_file(argv.first());
    let mut world = TorchWorld {
        host,
        argv,
        columns,
        secret,
        fetch,
        frames: Vec::new(),
    };
    let end = main(&mut world);
    let frames = std::mem::take(&mut world.frames);
    Run {
        end,
        frames,
        script,
    }
}

impl Run {
    /// The status of a run: an uncaught exception is reported as the interpreter does, then the streams are closed as the interpreter closes them
    /// (the final flush of standard output can still fail: status 120).
    pub fn conclude(self, console: &mut LiveConsole) -> i32 {
        let status = match self.end {
            Ok(()) => 0,
            Err(Stop::Exit(code)) => code,
            Err(Stop::Uncaught(thrown)) => {
                report_uncaught(console, &thrown, &self.frames, &self.script)
            }
            Err(Stop::Refused(refusal)) => {
                console.write_uncaught(&refusal_report(&refusal));
                REFUSAL_STATUS
            }
        };
        console.shutdown(status)
    }
}

fn report_uncaught(
    console: &mut LiveConsole,
    thrown: &Thrown,
    frames: &[Site],
    script: &PyStr,
) -> i32 {
    if thrown.raised.class == "SystemExit" {
        return exit_status(thrown);
    }
    let plain = thrown.raised.report();
    // a chained exception is not reachable through the known sites; it keeps the plain report
    let text = if frames.is_empty() || thrown.raised.cause.is_some() {
        plain
    } else {
        render(frames, script, &plain)
    };
    console.write_uncaught(&text);
    1
}

/// The named report of an outcome this port does not decide.
pub fn refusal_report(refusal: &Refusal) -> PyStr {
    PyStr::new(&format!(
        "{PROGRAM}: not decided by this port ({}): {}\n",
        refusal.boundary, refusal.detail
    ))
}

/// The hash secret of this process from `PYTHONHASHSEED` and the operating system's entropy (read once, before `main`).
pub fn hash_secret_for_process() -> Result<HashSecret, Refusal> {
    let value = std::env::var_os("PYTHONHASHSEED");
    let setting = parse_hash_seed(value.as_deref().map(|text| text.as_bytes())).map_err(|refused| Refusal {
        boundary: "PYTHONHASHSEED",
        detail: format!(
            "{:?} is not unset, empty, \"random\" or an integer in [0; 4294967295]; the interpreter's start-up failure for it is not reproduced",
            String::from_utf8_lossy(&refused.text)
        ),
    })?;
    HashSecret::for_setting(setting, &mut os_entropy).map_err(|error| Refusal {
        boundary: "hash_entropy",
        detail: format!("failed to get random numbers to initialize Python: {error}"),
    })
}

/// The program as a process. `script` is the caller's `argv[0]`; `args` are the arguments after it.
pub fn run_process(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = LiveConsole::for_streams();
    let secret = match hash_secret_for_process() {
        Ok(secret) => secret,
        Err(refusal) => {
            console.write_uncaught(&refusal_report(&refusal));
            return console.shutdown(REFUSAL_STATUS);
        }
    };
    let program = match script {
        Some(path) if crate::main_cmd::basename(path) == PROGRAM => script_file(Some(path)),
        _ => PyStr::new(PROGRAM),
    };
    let mut argv = vec![program];
    argv.extend(args.iter().cloned());
    let environment = FetchEnvironment::from_process();
    let dialer = UreqDialer::new();
    let fetch = |url: &PyStr| fetch_https_text_of(url, &environment, &dialer);
    let ran = run(&mut console, &argv, columns, &secret, &fetch);
    ran.conclude(&mut console)
}
