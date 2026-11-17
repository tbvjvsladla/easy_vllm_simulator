//! Public `manifest_contract`: the whole command line of the retired `manifest_contract.py` (`[-h] [--topology {single,multi}]
//! [--repo REPO] [--require-flag] [--json] [--self-test]`), answered natively byte for byte as the original's process answered it
//! (stdout, stderr and exit status). No shape starts a Python interpreter.
//!
//! **What is native.** The gate and the flow of `main` live in `role_contract::manifest_gate{,_cli}` and the self-test in
//! `role_contract::manifest_self_test`; this module adds what a *process* shows around them: argparse's surface (`-h`, usage
//! errors, abbreviations, `--x=y`; `campaign_args::parse_table`), the observer line, the stdout/stderr codecs of the text profile
//! and the write-failure tracebacks. The usage/help text is the original's at argparse's default width (80 columns); another
//! terminal width is not re-wrapped (documented residual).
//!
//! `MANIFEST_CONTRACT_COMPAT_OWNER` (exported by the launcher) names the program path the frames of a traceback show (the
//! `__file__` the original had); nothing requires that file to exist. A text profile this port does not model falls back to
//! UTF-8 (documented residual). An observer line is written when `MANIFEST_CONTRACT_ROUTE_OBSERVER` names a file.

use crate::cli::{PrintFailure, Streams};
use crate::pystr::PyStr as CallerText;
use crate::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pystr::PyStr as Text;
use role_contract::manifest_gate::BranchProbe;
use role_contract::manifest_gate_cli::{
    execute, parse_exact, print_failure, print_write_failure, Answer, PrintSite,
    Unsupported,
};
use role_contract::pyprofile::{self, encode_stream, Errors, TextProfile};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;

/// Names of the program-path and observer variables.
pub const OWNER_ENV: &str = "MANIFEST_CONTRACT_COMPAT_OWNER";
pub const OBSERVER_ENV: &str = "MANIFEST_CONTRACT_ROUTE_OBSERVER";
/// The launcher-only variables (declared here so the Rust, the launcher and the checks name the same set).
pub const BIN_ENV: &str = "MANIFEST_CONTRACT_BIN";
pub const VALIDATE_ONLY_ENV: &str = "MANIFEST_CONTRACT_VALIDATE_ONLY";
/// Status of a run that could not be set up or that met a shape without a captured original.
pub const EXIT_CONTROL_FAILURE: i32 = 2;

/// The original parser's actions in `add_argument` order.
pub const OPTIONS: [(&str, crate::campaign_args::K, &[&str]); 7] = [
    ("-h", crate::campaign_args::K::Help, &[]),
    ("--help", crate::campaign_args::K::Help, &[]),
    ("--topology", crate::campaign_args::K::Value, &["single", "multi"]),
    ("--repo", crate::campaign_args::K::Value, &[]),
    ("--require-flag", crate::campaign_args::K::Flag, &[]),
    ("--json", crate::campaign_args::K::Flag, &[]),
    ("--self-test", crate::campaign_args::K::Flag, &[]),
];

/// `manifest_contract.py -h` at 80 columns (captured from the original).
pub const HELP: &str = "usage: manifest_contract.sh [-h] [--topology {single,multi}] [--repo REPO]
                            [--require-flag] [--json] [--self-test]

테라포밍-완수 Flag 게이트 결정론 리더

options:
  -h, --help            show this help message and exit
  --topology {single,multi}
                        미지정 시 git 브랜치가 통로 선택
  --repo REPO           레포 루트(기본 .)
  --require-flag        Flag 미발급 시 비0종료(작업 스크립트 게이트용)
  --json                계약 JSON 을 stdout 으로
  --self-test           게이트/ TP 회귀(하드웨어 불요)
";

/// What a run reads from its process besides the arguments.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// The owner program (`__file__` of the owner process); `None` when the launcher did not export it.
    pub owner: Option<Vec<u8>>,
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
    /// The only route: this process answers every shape.
    NativeCurrentSource,
}

impl Route {
    pub fn name(&self) -> &'static str {
        "NativeCurrentSource"
    }
    fn reason(&self) -> Option<&str> {
        None
    }
}

/// The route of a command line: always native (there is no interpreter route any more).
pub fn choose_route(_args: &[Vec<u8>], _env: &Environment, _owner: &[u8]) -> Route {
    Route::NativeCurrentSource
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

/// Runs the program. `args` excludes the program name; the return value is the exit status. `probe` runs the original's one
/// `git rev-parse`.
pub fn run(args: &[Vec<u8>], env: &Environment, io: &mut Streams<'_>, probe: &mut dyn BranchProbe) -> i32 {
    let status = dispatch(args, env, io, probe);
    io.finish(status)
}

fn dispatch(args: &[Vec<u8>], env: &Environment, io: &mut Streams<'_>, probe: &mut dyn BranchProbe) -> i32 {
    let Some(owner) = env.owner.as_deref() else {
        return control_failure(
            io,
            &format!("{OWNER_ENV} is not set; the launcher manifest_contract.sh exports the program path beside it"),
        );
    };
    let profile = env.text_profile().unwrap_or_else(|_| TextProfile::utf8());
    let argv: Vec<String> = args.iter().map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    let namespace = match crate::campaign_args::parse_table(&OPTIONS, &argv) {
        Ok(Some(namespace)) => namespace,
        Ok(None) => {
            let _ = io.stdout.write_all(HELP.as_bytes());
            return 0;
        }
        Err(message) => {
            let usage = &HELP[..=HELP.find("\n\n").expect("usage paragraph")];
            let _ = io.stderr.write_all(format!("{usage}manifest_contract.sh: error: {message}\n").as_bytes());
            return 2;
        }
    };
    let last = |name: &str| namespace.get(name).and_then(|v| v.last()).cloned();
    if namespace.contains_key("--self-test") {
        let (text, status) = role_contract::manifest_self_test::run();
        let bytes = encode_stream(&Text::new(&text), profile.stdout, Errors::Strict).unwrap_or_else(|_| text.into_bytes());
        let _ = io.stdout.write_all(&bytes);
        return status;
    }
    let mut exact: Vec<Vec<u8>> = Vec::new();
    if let Some(topology) = last("--topology") {
        exact.push(b"--topology".to_vec());
        exact.push(topology.into_bytes());
    }
    let mut request = parse_exact(&exact).expect("a topology from the parser's choices");
    if let Some(repo) = last("--repo") {
        // a value that is not UTF-8 is decoded lossily (documented residual: the original kept it with surrogateescape)
        request.repo = repo.into_bytes();
    }
    request.require_flag = namespace.contains_key("--require-flag");
    request.json = namespace.contains_key("--json");
    let route = Route::NativeCurrentSource;
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
