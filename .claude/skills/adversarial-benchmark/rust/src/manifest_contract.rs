//! Public `manifest_contract`: the Flag-gate reader `manifest_contract.py --topology T --repo R [--require-flag] [--json]` that the
//! two shell callers (`run_bench.sh`, `lite_bench.sh`) use, answered natively byte for byte as the owner's process answers it
//! (stdout, stderr and exit status).
//!
//! **What is native.** The owner is the reviewed `manifest_contract.py` (byte-identical to `REVIEWED_MANIFEST_CONTRACT`). The gate
//! and the flow of `main` live in `role_contract::manifest_gate{,_cli}`; this module adds only what a *process* shows around
//! them: the routes, the observer line, the stdout/stderr codecs of the text profile and the write-failure tracebacks. The
//! in-process Python importers of the owner (`render_sub_env`, `scan_node`, `claim_predicates`, ...) keep importing the Python
//! file; they are not served by this program.
//!
//! **Routes** (decided before any native business, recorded as one observer line when `MANIFEST_CONTRACT_ROUTE_OBSERVER` names
//! a file; an unwritable requested file is a control failure and nothing else happens):
//!
//! | route | when | what runs |
//! |---|---|---|
//! | `LegacyExplicit` | `MANIFEST_CONTRACT_PYTHON` is non-empty | that interpreter on the owner, any shape |
//! | `LegacyShape` | the arguments are not an exact handled shape (`-h`, `--self-test`, abbreviations, `--x=y`, duplicates, ...) | the compatibility interpreter on the owner |
//! | `LegacySourceVariant` | the owner is missing, unreadable or a different program | the compatibility interpreter on it |
//! | `LegacyProfile` | the text profile of the environment is not modelled | the compatibility interpreter on the owner |
//! | `NativeCurrentSource` | none of the above | this process, to the end |
//!
//! A legacy route replaces this process (`exec`): same PID, descriptors, environment and arguments, no child and no capture. Once
//! the native business has started nothing is retried in Python: a refusal, a crash, an encode failure, a write failure and a
//! shape without a captured original ([`Unsupported`], visible exit 2) are answered natively. The compatibility interpreter is
//! `MANIFEST_CONTRACT_COMPAT_PYTHON` (default `python3`); the owner is `MANIFEST_CONTRACT_COMPAT_OWNER`, which the launcher exports and
//! which is also the `__file__` of the owner process.

use crate::campaign_read::{Legacy, LegacyExec};
use crate::cli::{PrintFailure, Streams};
use crate::pystr::PyStr as CallerText;
use crate::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pystr::PyStr as Text;
use role_contract::manifest_gate::BranchProbe;
use role_contract::manifest_gate_cli::{
    execute, parse_exact, print_failure, print_write_failure, Answer, ChildEnv, PrintSite,
    Unsupported,
};
use role_contract::pyprofile::{self, encode_stream, Errors, TextProfile};
use role_contract::source_identity::{identify, Identity, REVIEWED_MANIFEST_CONTRACT};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};

/// Names of the interpreter, owner, compatibility interpreter and observer variables.
pub const PYTHON_ENV: &str = "MANIFEST_CONTRACT_PYTHON";
pub const COMPAT_PYTHON_ENV: &str = "MANIFEST_CONTRACT_COMPAT_PYTHON";
pub const OWNER_ENV: &str = "MANIFEST_CONTRACT_COMPAT_OWNER";
pub const OBSERVER_ENV: &str = "MANIFEST_CONTRACT_ROUTE_OBSERVER";
/// The launcher-only variables (declared here so the Rust, the shim and the checks name the same set).
pub const BIN_ENV: &str = "MANIFEST_CONTRACT_BIN";
pub const VALIDATE_ONLY_ENV: &str = "MANIFEST_CONTRACT_VALIDATE_ONLY";
/// Launcher-set: the public program path the legacy owner runs under (its `sys.argv[0]`), see [`ExecLegacy`].
pub const PUBLIC_ARGV0_ENV: &str = "MANIFEST_CONTRACT_PUBLIC_ARGV0";
/// `python -c` program: run the owner file as `__main__` with `sys.argv = [public, *args]` and the owner's directory first on the path, like
/// `python owner args` would (`runpy.run_path` would overwrite `sys.argv[0]` with the owner path, so the file is compiled and executed directly).
const LEGACY_TRAMPOLINE: &str = "import os, sys\nowner = sys.argv[1]\nsys.argv = sys.argv[2:]\nsys.path[0] = os.path.dirname(owner)\nexec(compile(open(owner, 'rb').read(), owner, 'exec'), {'__name__': '__main__', '__file__': owner})";
/// Status of a run that could not be set up or that met a shape without a captured original (the owner exits 2 only for usage
/// shapes that never reach this program's native side).
pub const EXIT_CONTROL_FAILURE: i32 = 2;

/// What a run reads from its process besides the arguments.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// The owner program (`__file__` of the owner process); `None` when the launcher did not export it.
    pub owner: Option<Vec<u8>>,
    /// A non-empty `MANIFEST_CONTRACT_PYTHON`: explicit user intent.
    pub explicit_python: Option<OsString>,
    /// The interpreter of the other legacy routes.
    pub compat_python: OsString,
    pub observer: Option<OsString>,
    /// The text-profile variables as the environment holds them (name -> bytes).
    pub text_variables: Vec<(String, Vec<u8>)>,
}

const PROFILE_VARIABLES: [&str; 6] = [
    "LC_ALL",
    "LC_CTYPE",
    "LANG",
    "PYTHONUTF8",
    "PYTHONIOENCODING",
    "PYTHONCOERCECLOCALE",
];

impl Environment {
    pub fn from_process() -> Self {
        let non_empty = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        Environment {
            owner: non_empty(OWNER_ENV).map(OsString::into_vec),
            explicit_python: non_empty(PYTHON_ENV),
            compat_python: non_empty(COMPAT_PYTHON_ENV)
                .unwrap_or_else(|| OsString::from("python3")),
            observer: non_empty(OBSERVER_ENV),
            text_variables: PROFILE_VARIABLES
                .iter()
                .filter_map(|name| {
                    std::env::var_os(name).map(|value| (name.to_string(), value.into_vec()))
                })
                .collect(),
        }
    }

    pub fn text_profile(&self) -> Result<TextProfile, String> {
        TextProfile::from_environment(&|name| {
            self.text_variables
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    NativeCurrentSource,
    LegacyExplicit,
    LegacyShape,
    LegacySourceVariant { reason: String },
    LegacyProfile { reason: String },
}

impl Route {
    pub fn name(&self) -> &'static str {
        match self {
            Route::NativeCurrentSource => "NativeCurrentSource",
            Route::LegacyExplicit => "LegacyExplicit",
            Route::LegacyShape => "LegacyShape",
            Route::LegacySourceVariant { .. } => "LegacySourceVariant",
            Route::LegacyProfile { .. } => "LegacyProfile",
        }
    }
    fn reason(&self) -> Option<&str> {
        match self {
            Route::LegacySourceVariant { reason } | Route::LegacyProfile { reason } => Some(reason),
            _ => None,
        }
    }
}

/// `exec`: the same PID, descriptors, environment and working directory; no child.
pub struct ExecLegacy;

impl Legacy for ExecLegacy {
    fn replace_process(&mut self, spec: &LegacyExec) -> i32 {
        use std::os::unix::process::CommandExt;
        // The launcher may name the PUBLIC program path (`MANIFEST_CONTRACT_PUBLIC_ARGV0`): the reviewed original now lives in the
        // sibling compat owner file, and argparse derives `prog` from `sys.argv[0]`, so the owner runs under the public name and the
        // usage text stays the public command line's. Without the variable the owner is started directly, as before.
        let public = std::env::var_os(PUBLIC_ARGV0_ENV).filter(|value| !value.is_empty());
        let mut command = std::process::Command::new(&spec.interpreter);
        // The legacy owner (and the `git` it runs) sees the caller's own plumbing variables, not the launcher's rewrites.
        let caller = ChildEnv::from_process();
        caller.apply(&mut command);
        if let Some(public) = public {
            if caller.is_empty() {
                // no recorded caller facts (the program was started directly): the old behaviour, the variable is the launcher's alone
                command.env_remove(PUBLIC_ARGV0_ENV);
            }
            command
                .arg("-c")
                .arg(LEGACY_TRAMPOLINE)
                .arg(OsStr::from_bytes(&spec.owner))
                .arg(public);
        } else {
            command.arg(OsStr::from_bytes(&spec.owner));
        }
        let error = command
            .args(spec.args.iter().map(|a| OsStr::from_bytes(a)))
            .exec();
        let status = if error.kind() == std::io::ErrorKind::PermissionDenied {
            126
        } else {
            127
        };
        eprintln!(
            "[manifest_contract] ERROR 호환 인터프리터를 시작할 수 없다: {}: {error}",
            spec.interpreter.to_string_lossy()
        );
        status
    }
}

/// The routes that precede any native business, in the order they are asked.
pub fn choose_route(args: &[Vec<u8>], env: &Environment, owner: &[u8]) -> Route {
    if env.explicit_python.is_some() {
        return Route::LegacyExplicit;
    }
    if parse_exact(args).is_none() {
        return Route::LegacyShape;
    }
    match identify(owner, REVIEWED_MANIFEST_CONTRACT) {
        Identity::Reviewed => {}
        Identity::Missing => {
            return Route::LegacySourceVariant {
                reason: "manifest_contract.py is missing".to_string(),
            }
        }
        Identity::Different => {
            return Route::LegacySourceVariant {
                reason: "manifest_contract.py is not the reviewed source".to_string(),
            }
        }
        Identity::Unreadable(why) => {
            return Route::LegacySourceVariant {
                reason: format!("manifest_contract.py cannot be read ({why})"),
            }
        }
    }
    match env.text_profile() {
        Ok(_) => Route::NativeCurrentSource,
        Err(reason) => Route::LegacyProfile { reason },
    }
}

/// How the native answer ended, for the observer line.
fn outcome(answer: &Answer) -> &'static str {
    if answer.crashed {
        "crash"
    } else if answer.status == 0 {
        "success"
    } else {
        "refusal"
    }
}

fn observe(
    env: &Environment,
    route: &Route,
    owner: Option<&[u8]>,
    native_outcome: Option<&str>,
    args: &[Vec<u8>],
) -> Result<(), String> {
    let Some(path) = &env.observer else {
        return Ok(());
    };
    let mut line = PyDict::new();
    line.set("route", PyValue::str(route.name()));
    line.set(
        "legacy_reason",
        route.reason().map_or(PyValue::None, PyValue::str),
    );
    line.set(
        "native_outcome",
        native_outcome.map_or(PyValue::None, PyValue::str),
    );
    line.set("pid", PyValue::int(std::process::id()));
    line.set(
        "owner_path",
        owner.map_or(PyValue::None, |bytes| {
            PyValue::Str(CallerText::from_surrogateescape(bytes))
        }),
    );
    line.set(
        "shape",
        if parse_exact(args).is_some() {
            PyValue::str("gate")
        } else {
            PyValue::None
        },
    );
    let text = PyValue::Dict(line)
        .dumps_ascii()
        .map_err(|e| e.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{text}"))
        .map_err(|error| format!("the route observation cannot be written: {error}"))
}

fn control_failure(io: &mut Streams<'_>, message: &str) -> i32 {
    let _ = io
        .stderr
        .write_all(format!("[manifest_contract] ERROR {message}\n").as_bytes());
    EXIT_CONTROL_FAILURE
}

/// Runs the program. `args` excludes the program name; the return value is the exit status (a legacy route that starts its
/// interpreter never returns). `probe` runs the owner's one `git rev-parse`.
pub fn run(
    args: &[Vec<u8>],
    env: &Environment,
    io: &mut Streams<'_>,
    legacy: &mut dyn Legacy,
    probe: &mut dyn BranchProbe,
) -> i32 {
    let status = dispatch(args, env, io, legacy, probe);
    io.finish(status)
}

fn dispatch(
    args: &[Vec<u8>],
    env: &Environment,
    io: &mut Streams<'_>,
    legacy: &mut dyn Legacy,
    probe: &mut dyn BranchProbe,
) -> i32 {
    let Some(owner) = env.owner.as_deref() else {
        return control_failure(
            io,
            &format!("{OWNER_ENV} is not set; the launcher manifest_contract.sh exports the owner program beside it"),
        );
    };
    let route = choose_route(args, env, owner);
    if route != Route::NativeCurrentSource {
        if let Err(message) = observe(env, &route, Some(owner), None, args) {
            return control_failure(io, &message);
        }
        let interpreter = env
            .explicit_python
            .clone()
            .unwrap_or_else(|| env.compat_python.clone());
        return legacy.replace_process(&LegacyExec {
            interpreter,
            owner: owner.to_vec(),
            args: args.to_vec(),
        });
    }
    let request = parse_exact(args).expect("the route checked the grammar");
    let profile = env
        .text_profile()
        .expect("the route checked the text profile");
    let answered = pyprofile::with_profile(&profile, || execute(&request, owner, probe));
    let answer = match answered {
        Ok(answer) => answer,
        Err(Unsupported { boundary, detail }) => {
            return control_failure(
                io,
                &format!("unsupported shape ({boundary}): {detail} — no original was captured for it; nothing was answered"),
            )
        }
    };
    if let Err(message) = observe(env, &route, Some(owner), Some(outcome(&answer)), args) {
        return control_failure(io, &message);
    }
    emit(io, owner, &answer, &profile)
}

/// Writes the answer the way the owner's process does and returns the status.
fn emit(io: &mut Streams<'_>, owner: &[u8], answer: &Answer, profile: &TextProfile) -> i32 {
    if let Some(printed) = &answer.stdout {
        match encode_stream(&printed.text, profile.stdout, profile.stdout_errors) {
            Ok(bytes) => {
                if let Err(failure) = io.print_encoded(&bytes) {
                    return write_failed(io, owner, printed.site, profile, &failure);
                }
            }
            Err(failure) => {
                let crash = print_failure(owner, printed.site, &failure.message);
                if let Some(text) = &crash.stderr {
                    write_error_stream(io, text, profile);
                }
                return crash.status;
            }
        }
    }
    if let Some(text) = &answer.stderr {
        write_error_stream(io, text, profile);
    }
    answer.status
}

/// stderr: the stdout codec with `backslashreplace` (never fails).
fn write_error_stream(io: &mut Streams<'_>, text: &Text, profile: &TextProfile) {
    let bytes = encode_stream(text, profile.stdout, Errors::BackslashReplace).unwrap_or_default();
    let _ = io.stderr.write_all(&bytes);
}

/// A write of the one output line failed at once: the owner dies with the traceback of that `print` and status 1.
fn write_failed(
    io: &mut Streams<'_>,
    owner: &[u8],
    site: PrintSite,
    profile: &TextProfile,
    failure: &PrintFailure,
) -> i32 {
    let text = print_write_failure(owner, site, &Text::new(&failure.line()));
    write_error_stream(io, &text, profile);
    1
}
