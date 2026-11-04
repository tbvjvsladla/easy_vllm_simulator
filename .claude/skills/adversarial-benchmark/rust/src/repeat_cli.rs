//! Command line of `repeat_axis`: `resolve`, `record-run`, `aggregate-level` and `--self-test`.
//!
//! The command line is the original program's `argparse` interface byte for byte: usage, help and error text for the
//! top parser and for each subparser (see [`crate::argv`]), the order in which `argparse` meets options before the
//! subcommand word, and the way an "unrecognized arguments" error of a subparser is raised by the top parser. A
//! word the original rejects is rejected here; the Rust-only commands live in the separate `repeat_axis_api`
//! program ([`crate::repeat_api_cli`]) so that they cannot change what this program answers.
//!
//! Exit status: 0 success, 2 for a refusal (`RepeatAxisError` or an argument error), 1 for a failed self-test or an
//! exception the original does not handle (reported as a traceback marker and the final `Class: message` line).

use crate::argv::{
    self, repeat_axis_args as arg, Matches, Outcome, Spec, REPEAT_AXIS,
    REPEAT_AXIS_AGGREGATE_LEVEL, REPEAT_AXIS_RECORD_RUN, REPEAT_AXIS_RESOLVE,
};
use crate::cli::{on_large_stack, write_stderr, PrintFailure, Streams, TRACEBACK_HEADER};
use crate::guidellm::Failure;
use crate::pystr::{is_decimal_digit, PyStr};
use crate::pyvalue::PyValue;
use crate::repeat_axis::REPEAT_KIND_WARM;
use crate::repeat_fs::{aggregate_level, record_run, AggregateRequest, RunBoundary};
use crate::repeat_owner::{resolve_repeats, PythonCampaignOwner, RepoRoot, ResolveRequest};
use crate::repeat_selftest::run_self_test;
use crate::terminal;
use num_bigint::BigInt;

/// What a run of the program is configured with besides its arguments.
#[derive(Clone, Debug)]
pub struct Runtime {
    pub repo_root: RepoRoot,
    pub owner: PythonCampaignOwner,
}

impl Runtime {
    pub fn from_environment() -> Self {
        Runtime {
            repo_root: RepoRoot::from_environment(),
            owner: PythonCampaignOwner::from_environment(),
        }
    }
}

const SUBCOMMANDS: [(&str, &Spec); 3] = [
    ("resolve", &REPEAT_AXIS_RESOLVE),
    ("record-run", &REPEAT_AXIS_RECORD_RUN),
    ("aggregate-level", &REPEAT_AXIS_AGGREGATE_LEVEL),
];
const UNRECOGNIZED: &str = "unrecognized arguments: ";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], runtime: &Runtime, io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, runtime, io);
    io.finish(status)
}

// ── argparse: top parser with subcommands ───────────────────────────────────────

enum Command {
    Resolve(Matches),
    RecordRun(Matches),
    AggregateLevel(Matches),
}

enum Parsed {
    /// `-h` was reached first: print that parser's help, exit 0.
    Help(&'static Spec),
    /// `parser.error(message)` of that parser: its usage and the message on stderr, exit 2.
    Failed(&'static Spec, PyStr),
    Run {
        self_test: bool,
        command: Option<Command>,
    },
}

fn points(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

/// `^-\d+$|^-\d*\.\d+$` with Unicode digits; `$` also matches before one trailing newline.
fn looks_like_negative_number(arg: &[u32]) -> bool {
    let candidates = [arg, arg.strip_suffix(&[0x0A]).unwrap_or(arg)];
    candidates.iter().any(|body| {
        let Some(rest) = body.strip_prefix(&[u32::from('-')]) else {
            return false;
        };
        let digits = |slice: &[u32]| slice.iter().all(|&c| is_decimal_digit(c));
        match rest.iter().position(|&c| c == u32::from('.')) {
            None => !rest.is_empty() && digits(rest),
            Some(dot) => {
                digits(&rest[..dot]) && !rest[dot + 1..].is_empty() && digits(&rest[dot + 1..])
            }
        }
    })
}

/// Whether the top parser classifies `arg` as an option (`O`) rather than a positional (`A`) - argparse's
/// `_parse_optional` against the options `-h`, `--help` and `--self-test`.
fn top_parser_sees_option(arg: &[u32]) -> bool {
    let dash = u32::from('-');
    // `--` is the separator argparse recognises before it classifies anything else.
    if arg.first() != Some(&dash) || arg == [dash, dash] {
        return false;
    }
    let known: Vec<Vec<u32>> = ["-h", "--help", "--self-test"]
        .iter()
        .map(|o| points(o))
        .collect();
    if known.iter().any(|option| option == arg) {
        return true;
    }
    if arg.len() == 1 {
        return false;
    }
    if let Some(at) = arg.iter().position(|&c| c == u32::from('=')) {
        if known.iter().any(|option| option == &arg[..at]) {
            return true;
        }
    }
    let prefix_matches = if arg[1] == dash {
        let prefix = match arg.iter().position(|&c| c == u32::from('=')) {
            Some(at) => &arg[..at],
            None => arg,
        };
        known.iter().any(|option| option.starts_with(prefix))
    } else {
        known
            .iter()
            .any(|option| option.as_slice() == &arg[..2] || option.starts_with(arg))
    };
    if prefix_matches {
        return true;
    }
    !(looks_like_negative_number(arg) || arg.contains(&u32::from(' ')))
}

fn error_text(message: &str) -> PyStr {
    PyStr::new(message)
}

fn unrecognized_tail(message: &PyStr) -> Option<PyStr> {
    let prefix = points(UNRECOGNIZED);
    message
        .code_points()
        .strip_prefix(prefix.as_slice())
        .map(|tail| PyStr::from_code_points(tail.to_vec()))
}

fn unrecognized(tail: &PyStr) -> PyStr {
    let mut message = PyStr::new(UNRECOGNIZED);
    message.push_py(tail);
    message
}

fn join_extras(first: Option<PyStr>, second: PyStr) -> PyStr {
    match first {
        Some(mut text) => {
            text.push_str(" ");
            text.push_py(&second);
            text
        }
        None => second,
    }
}

/// `ArgumentParser.parse_args` of the original: options of the top parser, then the subcommand word and the
/// subparser's arguments. Unrecognized arguments of either are reported together by the top parser after the
/// subparser has finished (so a subparser error or `-h` wins over them).
fn parse_command_line(args: &[PyStr]) -> Parsed {
    let split = args
        .iter()
        .position(|arg| !top_parser_sees_option(arg.code_points()))
        .unwrap_or(args.len());
    let (leading, rest) = args.split_at(split);

    let mut self_test = false;
    let mut leftover: Option<PyStr> = None;
    match argv::parse_with(&REPEAT_AXIS, leading) {
        Outcome::Help => return Parsed::Help(&REPEAT_AXIS),
        Outcome::Error(message) => match unrecognized_tail(&message) {
            Some(tail) => leftover = Some(tail),
            None => return Parsed::Failed(&REPEAT_AXIS, message),
        },
        Outcome::Run(matches) => self_test = matches.flag(arg::SELF_TEST),
    }

    let Some(word) = rest.first() else {
        return match leftover {
            Some(tail) => Parsed::Failed(&REPEAT_AXIS, unrecognized(&tail)),
            None => Parsed::Run {
                self_test,
                command: None,
            },
        };
    };
    if word.equals("--") && rest.len() == 1 {
        return Parsed::Failed(
            &REPEAT_AXIS,
            unrecognized(&join_extras(leftover, PyStr::new("--"))),
        );
    }
    let Some((name, spec)) = SUBCOMMANDS.iter().find(|(name, _)| word.equals(name)) else {
        let choices: Vec<String> = SUBCOMMANDS
            .iter()
            .map(|(name, _)| PyStr::new(name).repr())
            .collect();
        let mut message = error_text("argument cmd: invalid choice: ");
        message.push_str(&word.repr());
        message.push_str(&format!(" (choose from {})", choices.join(", ")));
        return Parsed::Failed(&REPEAT_AXIS, message);
    };
    let matches = match argv::parse_with(spec, &rest[1..]) {
        Outcome::Help => return Parsed::Help(spec),
        Outcome::Error(message) => match unrecognized_tail(&message) {
            Some(tail) => {
                return Parsed::Failed(&REPEAT_AXIS, unrecognized(&join_extras(leftover, tail)))
            }
            None => return Parsed::Failed(spec, message),
        },
        Outcome::Run(matches) => matches,
    };
    if let Some(tail) = leftover {
        return Parsed::Failed(&REPEAT_AXIS, unrecognized(&tail));
    }
    let command = match *name {
        "resolve" => Command::Resolve(matches),
        "record-run" => Command::RecordRun(matches),
        _ => Command::AggregateLevel(matches),
    };
    Parsed::Run {
        self_test,
        command: Some(command),
    }
}

// ── running ─────────────────────────────────────────────────────────────────────

fn print_failed(io: &mut Streams<'_>, failure: &PrintFailure) -> i32 {
    let mut text = PyStr::new(TRACEBACK_HEADER);
    text.push_str(&failure.line());
    text.push_str("\n");
    write_stderr(io, &text);
    1
}

fn dispatch(args: &[PyStr], runtime: &Runtime, io: &mut Streams<'_>) -> i32 {
    let (self_test, command) = match parse_command_line(args) {
        Parsed::Help(spec) => {
            io.print_help(spec.help(terminal::columns()));
            return 0;
        }
        Parsed::Failed(spec, message) => {
            let mut text = PyStr::new(spec.usage(terminal::columns()));
            text.push_str(spec.prog);
            text.push_str(": error: ");
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
        Parsed::Run { self_test, command } => (self_test, command),
    };
    if self_test {
        let report = on_large_stack(|| run_self_test(&runtime.repo_root, &runtime.owner));
        if let Err(failure) = io.print_lines(&PyStr::new(&report.stdout)) {
            return print_failed(io, &failure);
        }
        write_stderr(io, &PyStr::new(&report.stderr));
        return report.exit_code;
    }
    let Some(command) = command else {
        io.print_help(REPEAT_AXIS.help(terminal::columns()));
        return 2;
    };
    match on_large_stack(|| execute(&command, runtime)) {
        Ok(line) => match io.print(&line) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure),
        },
        Err(failure) if failure.class == "RepeatAxisError" => {
            let mut text = PyStr::new("[repeat_axis] ERROR ");
            text.push_py(&failure.message);
            text.push_str("\n");
            write_stderr(io, &text);
            2
        }
        Err(failure) => {
            let mut text = PyStr::new(TRACEBACK_HEADER);
            text.push_py(&traceback_line(&failure));
            text.push_str("\n");
            write_stderr(io, &text);
            1
        }
    }
}

/// The last line of the traceback of an uncaught exception: `Class: message`. For the `SyntaxError` family
/// (an owner whose source does not compile) the traceback shows the bare `msg`, while `str(exception)` - what the
/// owner adapter relays - appends ` (file, line N)`; the suffix is dropped again here.
fn traceback_line(failure: &Failure) -> PyStr {
    if !matches!(
        failure.class,
        "SyntaxError" | "IndentationError" | "TabError"
    ) {
        return failure.line();
    }
    let message = failure.message.to_utf8().unwrap_or_default();
    let shown = message
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" ("))
        .filter(|(_, location)| {
            let line = location
                .rsplit_once(", line ")
                .map_or(*location, |(_, line)| line);
            let line = line.strip_prefix("line ").unwrap_or(line);
            !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_digit())
        })
        .map_or(message.as_str(), |(msg, _)| msg);
    let mut text = PyStr::new(failure.class);
    if !shown.is_empty() {
        text.push_str(": ");
        text.push_str(shown);
    }
    text
}

fn text_value(matches: &Matches, action: usize) -> PyValue {
    matches
        .value(action)
        .map_or(PyValue::None, |text| PyValue::Str(text.clone()))
}

fn int_value(matches: &Matches, action: usize) -> PyValue {
    PyValue::Int(
        matches
            .int(action)
            .cloned()
            .unwrap_or_else(|| BigInt::from(0)),
    )
}

/// The line the command prints.
fn execute(command: &Command, runtime: &Runtime) -> Result<PyStr, Failure> {
    match command {
        Command::Resolve(matches) => {
            use arg::resolve as option;
            let request = ResolveRequest {
                argv_value: text_value(matches, option::REPEATS),
                argv_source: text_value(matches, option::REPEATS_SOURCE),
                campaign_id: text_value(matches, option::CAMPAIGN_ID),
                preset_default: text_value(matches, option::PRESET_DEFAULT),
                preset_source: text_value(matches, option::PRESET_SOURCE),
            };
            let resolved = resolve_repeats(&request, &runtime.repo_root, &runtime.owner)?;
            let mut line = PyStr::new(&resolved.count.to_string());
            line.push_str("\u{1f}");
            let source: Vec<u32> = resolved
                .source
                .code_points()
                .iter()
                .map(|&point| if point == 0x1F { u32::from(' ') } else { point })
                .collect();
            line.push_py(&PyStr::from_code_points(source));
            line.push_str("\u{1f}");
            line.push_str(REPEAT_KIND_WARM);
            Ok(line)
        }
        Command::RecordRun(matches) => {
            use arg::record_run as option;
            let boundary = RunBoundary {
                level: int_value(matches, option::LEVEL),
                run: int_value(matches, option::RUN),
                started_utc: text_value(matches, option::STARTED_UTC),
                ended_utc: text_value(matches, option::ENDED_UTC),
                run_bench_rc: int_value(matches, option::RUN_BENCH_RC),
                parse_rc: int_value(matches, option::PARSE_RC),
            };
            record_run(&text_value(matches, option::RUN_DIR), &boundary)
        }
        Command::AggregateLevel(matches) => {
            use arg::aggregate_level as option;
            let kind = matches.value(option::KIND).map_or_else(
                || PyValue::str(REPEAT_KIND_WARM),
                |text| PyValue::Str(text.clone()),
            );
            let requested = int_value(matches, option::REQUESTED);
            let document = aggregate_level(
                &text_value(matches, option::LEVEL_DIR),
                &AggregateRequest {
                    requested: &requested,
                    requested_source: &text_value(matches, option::REQUESTED_SOURCE),
                    kind: &kind,
                },
            )?;
            let PyValue::Dict(document) = document else {
                unreachable!("aggregate_level returns the document")
            };
            let field = |key: &str| {
                document
                    .get(key)
                    .map_or_else(|| PyStr::new("None"), PyValue::display_text)
            };
            let mut line = PyStr::new("[repeat_axis] ");
            line.push_py(
                &matches
                    .value(option::LEVEL_DIR)
                    .cloned()
                    .unwrap_or_default(),
            );
            line.push_str(" 완주 ");
            line.push_py(&field("repeats_completed"));
            line.push_str("/");
            line.push_py(&requested.display_text());
            line.push_str(" · 밴드 ");
            line.push_py(&field("repro_band_pct"));
            line.push_str(" (");
            line.push_py(&field("repro_band_source"));
            line.push_str(")");
            Ok(line)
        }
    }
}
