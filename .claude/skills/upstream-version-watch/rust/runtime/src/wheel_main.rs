//! `main()` of `resolve_wheel.py` (lines 25-94): the command line, the release lookup, the wheel selection and the report.
//!
//! ```text
//! resolve_wheel.py [-h] [--arch ARCH] [--cuda CUDA] vllm_version
//! ```
//!
//! Exit status: 0 and the JSON of the chosen wheel on standard output; 4 (no wheel asset for the architecture, JSON error on standard error);
//! 5 (an explicit CUDA text without an exact asset, JSON with the neighbours on standard error); 2 (argparse); 1 (an exception nothing handles, as a
//! traceback on standard error). The business keeps the original's order: the arguments are parsed, then the release is fetched, then the names are
//! read, matched and chosen; `--arch` defaults to `platform.machine()` as given (no mapping), and an empty `--cuda` is no request.
//!
//! Outcomes this port does not decide are named, never disguised (status 70, [`Refusal`]): a fetch outside the scope of [`crate::wheel_fetch`], a
//! non-ASCII error report on a strict ASCII standard error, and what the other CLI modules of this package already refuse.

use crate::host::Sink;
use crate::http_direct::PyException;
use crate::main_cmd::{argument_error, basename, intern, split_usage};
use crate::runtime_patch::StdioProfile;
use crate::thrown::Done;
use crate::torch_main::Refusal;
use crate::wheel_asset::{candidate_values, select, RaiseSite, Raised, Selection};
use crate::wheel_fetch::{FetchError, Origin};
use crate::wheel_traceback::{
    fetch_report, print_report, script_report, Frame, HIGHEST_KEY, MAIN_GET, MAIN_HIGHEST,
    MAIN_ITERATE, MAIN_MATCH, MAIN_NAME, MAIN_NEAREST, MAIN_PRINT_NO_ASSETS, MAIN_PRINT_NO_CUDA,
    MAIN_PRINT_RESULT, MAIN_REQUESTED_MAJOR, MAIN_REQUESTED_MINOR, NEAREST_GENERATOR,
};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Positional, Spec, Value,
};
use std::collections::BTreeMap;

pub const PROGRAM: &str = "resolve_wheel.sh";

/// The release-by-tag endpoint of the original (`API`, a module constant that is never replaced).
pub const API: &str = "https://api.github.com/repos/vllm-project/vllm/releases/tags/v";
const DOWNLOAD_BASE: &str = "https://github.com/vllm-project/vllm/releases/download/v";
const NO_WHEEL_ASSETS: &str = "arch 매칭 wheel 자산 없음(+cu/무접미어 모두) — 자산 명명 스킴 변경 가능성 — vLLM release 페이지를 사람이 재확인";
const FORWARD_COMPAT_NOTE: &str = "CUDA minor 전방호환으로 동작하는 경우 많음 — 사전 기각 금지, 설치 시도 후 스모크/classify_failure 가 중재";

/// The release lookup of one run: `fetch_json(url)`.
pub type ReleaseFetcher<'a> = &'a dyn Fn(&PyValue) -> Result<PyValue, FetchError>;

/// What a run is on: the console, `sys.argv`, the terminal width, the stream profile, `platform.machine()` and the release lookup.
pub struct WheelWorld<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
    pub profile: StdioProfile,
    /// `platform.machine()` (`uname -m`).
    pub machine: PyStr,
    pub fetch: ReleaseFetcher<'a>,
}

/// Where the traceback of an uncaught exception comes from.
#[derive(Clone, Debug)]
pub enum Trace {
    /// Raised inside `fetch_json`.
    Fetch(Origin),
    /// Raised by the script's own code; the frames below `main`, innermost first.
    Script(Vec<Frame>),
}

impl Trace {
    /// Whether the exception was raised `from None` (its context is suppressed). Only an exception that came out of `fetch_json` can be:
    /// `main` itself raises nothing with `from None`, and an exception passing through it keeps what it had.
    pub fn suppresses_context(&self) -> bool {
        match self {
            Trace::Fetch(origin) => origin.suppresses_context(),
            Trace::Script(_) => false,
        }
    }
}

/// How `main` stopped before returning.
pub enum Stop {
    /// `sys.exit(code)` or the `SystemExit` of argparse.
    Exit(i32),
    /// An exception nothing handles, with where its traceback starts.
    Raised {
        error: Box<PyException>,
        trace: Trace,
    },
    /// A report already in its final text: a failed write of a `print` or of argparse's output.
    Rendered(PyStr),
    Refused(Refusal),
}

impl Stop {
    /// The text the interpreter writes to standard error for an uncaught exception (`script` is the path the frames name).
    pub fn report(&self, script: &PyStr) -> Option<PyStr> {
        match self {
            Stop::Raised {
                error,
                trace: Trace::Fetch(origin),
            } => Some(fetch_report(error, *origin, script)),
            Stop::Raised {
                error,
                trace: Trace::Script(frames),
            } => Some(script_report(error, frames, script)),
            Stop::Rendered(text) => Some(text.clone()),
            Stop::Exit(_) | Stop::Refused(_) => None,
        }
    }
}

type Flow<T> = Result<T, Stop>;

pub fn spec(prog: &str) -> Spec {
    let option = |flags: &'static [&'static str], dest, kind, help| Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    };
    Spec {
        prog: intern(prog),
        description: "",
        options: vec![
            option(&["-h", "--help"], "help", Kind::Help, None),
            option(
                &["--arch"],
                "arch",
                Kind::Text,
                Some("uname -m 값 (aarch64 | x86_64 · 기본: 이 호스트)"),
            ),
            option(
                &["--cuda"],
                "cuda",
                Kind::Text,
                Some("예 129. 미지정 시 자산에서 최고 cuXXX 자동"),
            ),
        ],
        group_required: false,
        positional: Some(Positional {
            dest: "vllm_version",
            help: Some("예: 0.21.0"),
        }),
    }
}

/// The parsed command line.
struct Arguments {
    vllm_version: PyStr,
    arch: PyStr,
    cuda: Option<PyStr>,
}

fn text_of(values: &BTreeMap<&'static str, Value>, dest: &str) -> Option<PyStr> {
    match values.get(dest) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

fn arguments_of(values: &BTreeMap<&'static str, Value>, machine: &PyStr) -> Option<Arguments> {
    Some(Arguments {
        vllm_version: text_of(values, "vllm_version")?,
        arch: text_of(values, "arch").unwrap_or_else(|| machine.clone()),
        cuda: text_of(values, "cuda"),
    })
}

/// The usage and help name the required positional `vllm_version`; the recipe package's `cliparse` can only model an optional one
/// (`[vllm_version]`), whose brackets are removed from every text it formats (the adaptation `ngc_main` makes for its positional).
fn requiring_positional(text: &PyStr) -> PyStr {
    let from: Vec<u32> = "[vllm_version]".chars().map(u32::from).collect();
    let to: Vec<u32> = "vllm_version".chars().map(u32::from).collect();
    let points = text.code_points();
    let mut fixed = Vec::with_capacity(points.len());
    let mut at = 0;
    while at < points.len() {
        if points[at..].starts_with(&from) {
            fixed.extend_from_slice(&to);
            at += from.len();
        } else {
            fixed.push(points[at]);
            at += 1;
        }
    }
    PyStr::from_code_points(fixed)
}

/// The common help formatter puts a description paragraph after the usage; this parser has none: the four line breaks after the usage are two.
fn without_extra_blank_line(help: &PyStr) -> PyStr {
    let points = help.code_points();
    let quadruple: Vec<u32> = "\n\n\n\n".chars().map(u32::from).collect();
    match points
        .windows(4)
        .position(|window| window == quadruple.as_slice())
    {
        Some(at) => {
            let mut fixed = points[..at + 2].to_vec();
            fixed.extend_from_slice(&points[at + 4..]);
            PyStr::from_code_points(fixed)
        }
        None => help.clone(),
    }
}

/// The arguments the parser's own `unrecognized arguments:` message lists (argparse reports a missing required argument before them).
fn unrecognized_in(message: &PyStr) -> Vec<PyStr> {
    let text = message.to_utf8().unwrap_or_default();
    text.split_once("unrecognized arguments: ")
        .map(|(_, list)| list.trim_end().split(' ').map(PyStr::new).collect())
        .unwrap_or_default()
}

fn parse_arguments(world: &mut WheelWorld<'_>) -> Flow<Arguments> {
    let program = world
        .argv
        .first()
        .map_or_else(|| PROGRAM.to_string(), basename);
    let spec = spec(&program);
    let width = terminal_width(world.columns);
    let usage = requiring_positional(&PyStr::new(&format_usage(&spec, width)));
    let required_error = || {
        PyStr::new(&format!(
            "{program}: error: the following arguments are required: vllm_version\n"
        ))
    };
    let (status, stdout, stderr, head, rest) = match parse(&spec, &world.argv[1..], world.columns) {
        Outcome::Values(values) => match arguments_of(&values, &world.machine) {
            Some(arguments) => return Ok(arguments),
            None => (
                2,
                PyStr::default(),
                PyStr::default(),
                usage.clone(),
                required_error(),
            ),
        },
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            let (head, rest) = split_usage(&usage, &requiring_positional(&stderr));
            let unrecognized = unrecognized_in(&rest);
            let remaining: Vec<PyStr> = world.argv[1..]
                .iter()
                .filter(|argument| !unrecognized.contains(argument))
                .cloned()
                .collect();
            let missing = !unrecognized.is_empty()
                && matches!(parse(&spec, &remaining, world.columns),
                    Outcome::Values(values) if arguments_of(&values, &world.machine).is_none());
            if missing {
                (
                    2,
                    PyStr::default(),
                    PyStr::default(),
                    usage.clone(),
                    required_error(),
                )
            } else {
                (status, requiring_positional(&stdout), stderr, head, rest)
            }
        }
    };
    let mut message = None;
    if !stdout.is_empty() {
        world.host.parser_help(&without_extra_blank_line(&stdout));
    }
    if !stderr.is_empty() || !rest.is_empty() {
        world.host.parser_error(&head, &rest);
        message = Some(rest);
    }
    if let Some(mut failure) = world.host.take_parser_failure() {
        if failure.raised.cause.is_none() {
            if let Some(context) = message.as_ref().and_then(argument_error) {
                failure.raised.cause = Some(Box::new(context));
                failure.raised.via_context = true;
            }
        }
        return Err(Stop::Rendered(failure.raised.report()));
    }
    Err(Stop::Exit(status))
}

/// The path the traceback frames name for the script: `argv[0]` when it ends in the script's name (made absolute against the working
/// directory when it is relative and has a directory part), else the canonical name.
pub fn script_file(argv0: Option<&PyStr>) -> PyStr {
    let Some(path) = argv0.filter(|path| basename(path) == PROGRAM) else {
        return PyStr::new(PROGRAM);
    };
    let points = path.code_points();
    let is_relative = points.first() != Some(&u32::from('/'));
    let has_directory = points.contains(&u32::from('/'));
    match std::env::current_dir() {
        Ok(cwd) if is_relative && has_directory => {
            use std::os::unix::ffi::OsStrExt;
            let mut absolute = PyStr::from_surrogateescape(cwd.as_os_str().as_bytes());
            absolute.push_str("/");
            absolute.push_py(path);
            absolute
        }
        _ => path.clone(),
    }
}

/// The position and characters of the first non-ASCII run in `text` as `UnicodeEncodeError('ascii', ...)` words them.
pub(crate) fn ascii_encode_error(text: &PyStr) -> Option<PyStr> {
    let points = text.code_points();
    let start = points.iter().position(|point| *point > 127)?;
    let end = start
        + points[start..]
            .iter()
            .take_while(|point| **point > 127)
            .count();
    let escaped = |point: u32| match point {
        0..=0xff => format!("\\x{point:02x}"),
        0x100..=0xffff => format!("\\u{point:04x}"),
        _ => format!("\\U{point:08x}"),
    };
    let subject = if end == start + 1 {
        format!("character '{}' in position {start}", escaped(points[start]))
    } else {
        format!("characters in position {start}-{}", end - 1)
    };
    Some(PyStr::new(&format!(
        "UnicodeEncodeError: 'ascii' codec can't encode {subject}: ordinal not in range(128)\n"
    )))
}

fn refusal(boundary: &'static str, detail: impl Into<String>) -> Stop {
    Stop::Refused(Refusal {
        boundary,
        detail: detail.into(),
    })
}

/// The frames of an exception the script's own code raised, innermost first.
fn raise_frames(site: RaiseSite) -> Vec<Frame> {
    match site {
        RaiseSite::ReleaseGet => vec![MAIN_GET],
        RaiseSite::AssetsIteration => vec![MAIN_ITERATE],
        RaiseSite::AssetName => vec![MAIN_NAME],
        RaiseSite::NameMatch => vec![MAIN_MATCH],
        RaiseSite::RequestedMajor => vec![MAIN_REQUESTED_MAJOR],
        RaiseSite::RequestedMinor => vec![MAIN_REQUESTED_MINOR],
        RaiseSite::NearestGenerator => vec![NEAREST_GENERATOR, MAIN_NEAREST],
        RaiseSite::HighestKey => vec![HIGHEST_KEY, MAIN_HIGHEST],
    }
}

fn dumps(report: &PyValue) -> Flow<PyStr> {
    report
        .dumps_pretty()
        .map_err(|failure| refusal("json_dumps", format!("{failure:?}")))
}

/// `print(text)` on standard output at line 89: the text, then the line end.
fn print_result(world: &mut WheelWorld<'_>, text: &PyStr, script: &PyStr) -> Flow<()> {
    if world.profile == StdioProfile::AsciiStrict {
        if let Some(block) = ascii_encode_error(text) {
            return Err(Stop::Rendered(print_report(
                MAIN_PRINT_RESULT,
                &block,
                script,
            )));
        }
    }
    let written: Done<()> = world
        .host
        .out(text)
        .and_then(|()| world.host.out(&PyStr::new("\n")));
    written.map_err(|failure| {
        Stop::Rendered(print_report(
            MAIN_PRINT_RESULT,
            &failure.raised.report(),
            script,
        ))
    })
}

/// `print(..., file=sys.stderr)` of an error report, from the `print` at `frame`.
fn print_error(world: &mut WheelWorld<'_>, text: &PyStr, frame: Frame, script: &PyStr) -> Flow<()> {
    if world.profile == StdioProfile::AsciiStrict && ascii_encode_error(text).is_some() {
        return Err(refusal(
            "ascii_stream_message",
            "the message holds a non-ASCII character that a strict ASCII standard error refuses",
        ));
    }
    world
        .host
        .err(text)
        .and_then(|()| world.host.err(&PyStr::new("\n")))
        .map_err(|failure| Stop::Rendered(print_report(frame, &failure.raised.report(), script)))
}

fn dict_of(members: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in members {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

fn texts(values: &[PyStr]) -> PyValue {
    PyValue::List(values.iter().cloned().map(PyValue::Str).collect())
}

fn script_failure(error: Raised) -> Stop {
    Stop::Raised {
        error: error.error,
        trace: Trace::Script(raise_frames(error.site)),
    }
}

fn fetch_failure(error: FetchError) -> Stop {
    match error {
        FetchError::Raised { error, origin } => Stop::Raised {
            error,
            trace: Trace::Fetch(origin),
        },
        FetchError::OutOfScope(scope) => refusal(
            "fetch_json",
            format!("{}: {}", scope.boundary, scope.detail),
        ),
        FetchError::Transport(transport) => refusal(
            "fetch_transport",
            format!(
                "{} failed: {} (its Python spelling was not captured)",
                transport.phase, transport.detail
            ),
        ),
    }
}

pub fn main(world: &mut WheelWorld<'_>) -> Flow<()> {
    let script = script_file(world.argv.first());
    let arguments = parse_arguments(world)?;
    let mut url = PyStr::new(API);
    url.push_py(&arguments.vllm_version);
    let release = (world.fetch)(&PyValue::Str(url)).map_err(fetch_failure)?;
    let selection = select(
        &release,
        &arguments.vllm_version,
        &arguments.arch,
        arguments.cuda.as_ref(),
    )
    .map_err(script_failure)?;
    match selection {
        Selection::NoWheelAssets { assets } => {
            let report = dict_of(vec![
                ("error", PyValue::str(NO_WHEEL_ASSETS)),
                ("arch", PyValue::Str(arguments.arch.clone())),
                ("assets", texts(&assets)),
            ]);
            let text = dumps(&report)?;
            print_error(world, &text, MAIN_PRINT_NO_ASSETS, &script)?;
            Err(Stop::Exit(4))
        }
        Selection::NoExactCuda {
            requested,
            available,
            default_variants,
            nearest,
        } => {
            let mut error = PyStr::new("cu");
            error.push_py(&requested);
            error.push_str(" 정확일치 자산 없음");
            let report = dict_of(vec![
                ("error", PyValue::Str(error)),
                ("available", candidate_values(&available)),
                ("default_variants", candidate_values(&default_variants)),
                (
                    "forward_compat_hint",
                    dict_of(vec![
                        ("nearest_cu_candidates", candidate_values(&nearest)),
                        ("note", PyValue::str(FORWARD_COMPAT_NOTE)),
                    ]),
                ),
            ]);
            let text = dumps(&report)?;
            print_error(world, &text, MAIN_PRINT_NO_CUDA, &script)?;
            Err(Stop::Exit(5))
        }
        Selection::Chosen {
            chosen,
            candidates,
            default_variants,
        } => {
            let mut wheel_url = PyStr::new(DOWNLOAD_BASE);
            wheel_url.push_py(&arguments.vllm_version);
            wheel_url.push_str("/");
            wheel_url.push_py(&chosen.name);
            let report = dict_of(vec![
                ("vllm_version", PyValue::Str(arguments.vllm_version.clone())),
                ("arch", PyValue::Str(arguments.arch.clone())),
                (
                    "cuda_version",
                    chosen.cuda.clone().map_or(PyValue::None, PyValue::Str),
                ),
                ("manylinux", PyValue::Str(chosen.manylinux.clone())),
                ("asset_name", PyValue::Str(chosen.name.clone())),
                ("wheel_url", PyValue::Str(wheel_url)),
                ("all_arch_assets", candidate_values(&candidates)),
                ("default_variants", candidate_values(&default_variants)),
            ]);
            let text = dumps(&report)?;
            print_result(world, &text, &script)
        }
    }
}
