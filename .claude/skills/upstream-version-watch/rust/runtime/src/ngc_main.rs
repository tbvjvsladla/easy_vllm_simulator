//! `main()` of `resolve_ngc_tag.py`: the torch pin becomes its release text, the candidate tags (the explicit list, else the months counted down
//! from `--start`) are probed newest first through `inspect_env`, the first tag whose build version normalizes to the pin is printed as JSON,
//! the tuple-prefix matches are collected on the way, and no match ends with the error JSON on standard error.
//!
//! Exit status: 0 (a match); 2 (argparse, or neither `--candidates` nor `--start`); 1 (the architecture cannot be decided, or an exception nothing
//! handles); 4 (no exact match). The business keeps the original's order: `--arch` is validated after parsing and before anything else, `norm` of
//! the pin runs before the candidates are chosen, a truthy `--candidates` wins over `--start` (even when it holds only separators), and an exception
//! of `gen_candidates` or `inspect_env` ends the run uncaught.
//!
//! Outcomes this port does not decide are named, never disguised (status 70, [`Refusal`]): a month count above [`crate::ngc_tag::MONTHS_LIMIT`] (of a valid start), a non-ASCII
//! message on a strict ASCII standard error, and what the other CLI modules of this package already refuse.

use crate::host::Sink;
use crate::main_cmd::{argument_error, basename, intern, split_usage};
use crate::ngc_inspect::Docker;
use crate::ngc_tag::{inspect, norm_text, release_of_text, Raised, IMAGE};
use crate::ngc_traceback::{frames, print_match, Entry};
use crate::runtime_patch::StdioProfile;
use crate::thrown::Done;
use crate::torch_main::Refusal;
use crate::torch_traceback::Site;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Positional, Spec, Value,
};

pub const PROGRAM: &str = "resolve_ngc_tag.py";
const ARCHITECTURES: [&str; 2] = ["arm64", "amd64"];
const NO_CANDIDATES: &str = "--candidates 또는 --start 필요";
const PREFIX_NOTE: &str = "prefix-match — 최종 중재=스모크";
const NO_MATCH: &str = "매칭 NGC 태그 없음(완전일치) — 매칭 베이스 없음 ≠ 빌드 불가 — 후속: NGC release-notes 매트릭스 확인 + 더 새 베이스 승격 검토(라우팅: .claude/rules/workflow.md §실패 라우팅 'NGC base mismatch' — 후보 헤더/로그 증거 → HITL → 해소값 override → S3. 시도-빌드 우회는 .claude/skills/upstream-version-watch/references/source-build.md §3)";

/// What a run is on: the console, `sys.argv`, the terminal width, the stream profile, the Docker boundary and `platform.machine()`.
pub struct NgcWorld<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
    pub profile: StdioProfile,
    pub docker: &'a dyn Docker,
    /// `platform.machine()` (`uname -m`).
    pub machine: PyStr,
    /// The frames an uncaught exception shows, innermost first.
    pub frames: Vec<Site>,
}

/// How `main` stopped before returning.
pub enum Stop {
    /// `sys.exit(code)` or the `SystemExit` of argparse.
    Exit(i32),
    /// An exception nothing handles, as its `Class: message` block.
    Uncaught(PyStr),
    Refused(Refusal),
}

type Flow<T> = Result<T, Stop>;

/// The architecture `platform.machine()` selects by default (`None`: no entry for it).
pub fn host_architecture(machine: &PyStr) -> Option<&'static str> {
    let text = machine.to_utf8()?;
    Some(match text.as_str() {
        "aarch64" | "arm64" => "arm64",
        "x86_64" | "amd64" => "amd64",
        _ => return None,
    })
}

pub fn spec(prog: &str, host: Option<&str>) -> Spec {
    let host_text = host.unwrap_or("None");
    let arch_help: &'static str =
        Box::leak(format!("arm64 | amd64 (기본: 이 호스트 = {host_text})").into_boxed_str());
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
            option(&["--arch"], "arch", Kind::Text, Some(arch_help)),
            option(
                &["--start"],
                "start",
                Kind::Text,
                Some("YY.MM (가장 최신 후보). newest→oldest 프로빙"),
            ),
            option(
                &["--months"],
                "months",
                Kind::Int,
                Some("--start 부터 거슬러 프로빙할 개월수"),
            ),
            option(
                &["--candidates"],
                "candidates",
                Kind::Text,
                Some("명시 태그 목록(쉼표, newest-first)"),
            ),
        ],
        group_required: false,
        positional: Some(Positional {
            dest: "torch_pin",
            help: Some("예: 2.11.0"),
        }),
    }
}

/// The parsed command line.
struct Arguments {
    torch_pin: PyStr,
    arch: Option<PyStr>,
    start: Option<PyStr>,
    months: BigInt,
    candidates: Option<PyStr>,
}

fn text_of(values: &std::collections::BTreeMap<&'static str, Value>, dest: &str) -> Option<PyStr> {
    match values.get(dest) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

fn arguments_of(
    values: &std::collections::BTreeMap<&'static str, Value>,
    host: Option<&str>,
) -> Option<Arguments> {
    let months = match values.get("months") {
        Some(Value::Int(number)) => number.clone(),
        _ => BigInt::from(18),
    };
    Some(Arguments {
        torch_pin: text_of(values, "torch_pin")?,
        arch: text_of(values, "arch").or_else(|| host.map(PyStr::new)),
        start: text_of(values, "start"),
        months,
        candidates: text_of(values, "candidates"),
    })
}

/// The usage and help name the required positional `torch_pin`; the recipe package's `cliparse` can only model an optional one (`[torch_pin]`),
/// whose brackets are removed from every text it formats (the same adaptation the `resolve_torch_pin` command makes).
fn requiring_positional(text: &PyStr) -> PyStr {
    let from: Vec<u32> = "[torch_pin]".chars().map(u32::from).collect();
    let to: Vec<u32> = "torch_pin".chars().map(u32::from).collect();
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

fn parse_arguments(world: &mut NgcWorld<'_>) -> Flow<Arguments> {
    let program = world
        .argv
        .first()
        .map_or_else(|| PROGRAM.to_string(), basename);
    let host = host_architecture(&world.machine);
    let spec = spec(&program, host);
    let width = terminal_width(world.columns);
    let usage = requiring_positional(&PyStr::new(&format_usage(&spec, width)));
    let required_error = || {
        PyStr::new(&format!(
            "{program}: error: the following arguments are required: torch_pin\n"
        ))
    };
    let (status, stdout, stderr, head, rest) = match parse(&spec, &world.argv[1..], world.columns) {
        Outcome::Values(values) => match arguments_of(&values, host) {
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
                    Outcome::Values(values) if arguments_of(&values, host).is_none());
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
        return Err(Stop::Uncaught(failure.raised.report()));
    }
    Err(Stop::Exit(status))
}

/// The position and characters of the first non-ASCII run in `text` as `UnicodeEncodeError('ascii', ...)` words them.
fn ascii_encode_error(text: &PyStr) -> Option<PyStr> {
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

fn exception_report(error: Raised, entry: Entry, world: &mut NgcWorld<'_>) -> Stop {
    match error {
        Raised::Exception(row) => {
            world.frames = frames(entry, row.at);
            let mut report = PyStr::new(&format!("{}: ", row.class));
            report.push_py(&row.message);
            report.push_str("\n");
            Stop::Uncaught(report)
        }
        Raised::Limit(detail) => Stop::Refused(Refusal {
            boundary: "months_limit",
            detail,
        }),
    }
}

/// `print(text)` on standard output: the text, then the line end.
fn print(world: &mut NgcWorld<'_>, text: &PyStr) -> Flow<()> {
    if world.profile == StdioProfile::AsciiStrict {
        if let Some(report) = ascii_encode_error(text) {
            world.frames = print_match();
            return Err(Stop::Uncaught(report));
        }
    }
    let written: Done<()> = world
        .host
        .out(text)
        .and_then(|()| world.host.out(&PyStr::new("\n")));
    written.map_err(|failure| {
        world.frames = print_match();
        Stop::Uncaught(failure.raised.report())
    })
}

/// `print(..., file=sys.stderr)`.
fn print_error(world: &mut NgcWorld<'_>, text: &PyStr) -> Flow<()> {
    if world.profile == StdioProfile::AsciiStrict && ascii_encode_error(text).is_some() {
        return Err(Stop::Refused(Refusal {
            boundary: "ascii_stream_message",
            detail:
                "the message holds a non-ASCII character that a strict ASCII standard error refuses"
                    .to_string(),
        }));
    }
    world
        .host
        .err(text)
        .and_then(|()| world.host.err(&PyStr::new("\n")))
        .map_err(|failure| Stop::Uncaught(failure.raised.report()))
}

fn tuple_is_prefix(a: Option<&[BigInt]>, b: Option<&[BigInt]>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    if a == b {
        return false;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() < long.len() && long[..short.len()] == *short
}

fn dict_of(members: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in members {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

pub fn main(world: &mut NgcWorld<'_>) -> Flow<()> {
    let arguments = parse_arguments(world)?;
    let architecture = match &arguments.arch {
        Some(arch) if ARCHITECTURES.iter().any(|known| arch.equals(known)) => arch.clone(),
        _ => {
            let mut message = PyStr::new("[ngc] FAIL: --arch 를 정할 수 없다(호스트 ");
            message.push_str(&world.machine.repr());
            message.push_str(") — arm64 | amd64 를 명시하라");
            print_error(world, &message)?;
            return Err(Stop::Exit(1));
        }
    };
    let target = norm_text(&arguments.torch_pin)
        .map_err(|error| exception_report(error, Entry::Target, world))?;
    let candidates: Vec<PyStr> = match (&arguments.candidates, &arguments.start) {
        (Some(list), _) if !list.is_empty() => list
            .code_points()
            .split(|&point| point == u32::from(','))
            .map(|piece| PyStr::from_code_points(piece.to_vec()).strip())
            .filter(|tag| !tag.is_empty())
            .collect(),
        // `candidates_of` validates `start` (line 68 of the original) before it sizes `range(months)` (line 70) and only then names the month cap, so
        // a malformed start ends in the original's immediate `ValueError` however large `--months` is.
        (_, Some(start)) if !start.is_empty() => crate::ngc_tag::candidates_of(
            &PyValue::Str(start.clone()),
            &PyValue::Int(arguments.months.clone()),
        )
        .map_err(|error| exception_report(error, Entry::Candidates, world))?,
        _ => {
            let report = dict_of(vec![("error", PyValue::str(NO_CANDIDATES))]);
            let text = report.dumps_unicode().map_err(|_| Stop::Exit(1))?;
            print_error(world, &text)?;
            return Err(Stop::Exit(2));
        }
    };
    let target_tuple = release_of_text(&arguments.torch_pin).ok().flatten();
    let mut probed: Vec<PyValue> = Vec::new();
    let mut prefix_candidates: Vec<PyValue> = Vec::new();
    for tag in candidates {
        let probe = inspect(
            world.docker,
            &PyValue::Str(tag.clone()),
            &PyValue::Str(architecture.clone()),
        )
        .map_err(|error| exception_report(error, Entry::Probe, world))?;
        let Some(env) = probe else {
            probed.push(dict_of(vec![
                ("tag", PyValue::Str(tag)),
                ("status", PyValue::str("unavailable")),
            ]));
            continue;
        };
        let build_version = match env.get("PYTORCH_BUILD_VERSION") {
            Some(value) if value.is_truthy() => Some(value.clone()),
            _ => env.get("PYTORCH_VERSION").cloned(),
        };
        let build_text = build_version
            .as_ref()
            .filter(|value| value.is_truthy())
            .map(PyValue::display_text);
        let prefix = match &build_text {
            Some(text) => Some(
                norm_text(text).map_err(|error| exception_report(error, Entry::Prefix, world))?,
            ),
            None => None,
        };
        let shown_build = build_version.clone().unwrap_or(PyValue::None);
        probed.push(dict_of(vec![
            ("tag", PyValue::Str(tag.clone())),
            ("build_version", shown_build.clone()),
            ("prefix", prefix.clone().map_or(PyValue::None, PyValue::Str)),
        ]));
        if prefix.as_ref() == Some(&target) {
            let mut image = PyStr::new(&format!("{IMAGE}:"));
            image.push_py(&tag);
            let report = dict_of(vec![
                ("torch_pin", PyValue::Str(arguments.torch_pin.clone())),
                ("torch_prefix", PyValue::Str(target.clone())),
                ("matched_tag", PyValue::Str(tag)),
                ("build_version", shown_build),
                (
                    "cuda_version",
                    env.get("CUDA_VERSION").cloned().unwrap_or(PyValue::None),
                ),
                ("image", PyValue::Str(image)),
                ("arch", PyValue::Str(architecture)),
                ("probed", PyValue::List(probed)),
                ("prefix_candidates", PyValue::List(prefix_candidates)),
            ]);
            let text = report.dumps_pretty().map_err(|_| Stop::Exit(1))?;
            return print(world, &text);
        }
        let build_release = match &build_text {
            Some(text) => release_of_text(text).ok().flatten(),
            None => None,
        };
        if tuple_is_prefix(target_tuple.as_deref(), build_release.as_deref()) {
            prefix_candidates.push(dict_of(vec![
                ("tag", PyValue::Str(tag)),
                ("build_version", shown_build),
                ("note", PyValue::str(PREFIX_NOTE)),
            ]));
        }
    }
    let report = dict_of(vec![
        ("error", PyValue::str(NO_MATCH)),
        ("torch_prefix", PyValue::Str(target)),
        ("prefix_candidates", PyValue::List(prefix_candidates)),
        ("probed", PyValue::List(probed)),
    ]);
    let text = report.dumps_pretty().map_err(|_| Stop::Exit(1))?;
    print_error(world, &text)?;
    Err(Stop::Exit(4))
}
