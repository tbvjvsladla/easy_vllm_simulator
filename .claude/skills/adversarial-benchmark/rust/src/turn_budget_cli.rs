//! Public `turn_budget` command line: the original `turn_budget.py` `main()` answered natively, byte for byte as the owner's
//! process answers it (stdout, stderr and exit status), including the Python 3.12 traceback of every exception the owner does not
//! handle (frames of the owner entry points, of `json`, and of the codec - none is "not reproduced").
//!
//! **What is native.** The business (`schema_cap`, `declare`, the self-test, the `--caps` document) lives in
//! `role_contract::turn_budget`; this module adds only what a *process* of the owner shows around it: the argument grammar, the
//! chunked `json.dump` to a buffered stdout, the refusal (`SystemExit` message) wiring, the traceback and the exit statuses.
//!
//! **Identity and routes.** The launcher `turn_budget.sh` exports the owner path (`__file__` of the process this one stands for) in
//! `TURN_BUDGET_OWNER_PATH`; the default schema is derived from it lexically (parent x4, nothing resolved, the `--repo-root` of a
//! relay is irrelevant). There is no Python route: a shape this program does not model - an unset owner path, a text profile that
//! is not modelled - is a **visible control failure** (status 2, nothing run) and is recorded as one observer line when
//! `TURN_BUDGET_ROUTE_OBSERVER` names a file; an unwritable requested observer is also a control failure. Once the native business
//! has started nothing is retried.

use crate::argv::{self, turn_budget_args as arg, Outcome, TURN_BUDGET};
use crate::cli::{PrintFailure, Streams};
use crate::pystr::PyStr as CallerText;
use crate::stdout::OutputFailure;
use crate::terminal;
use agent_runtime_support::pystr::PyStr as Text;
use agent_runtime_support::pyvalue::{DumpError, PyDict, PyValue, StreamError};
use role_contract::pyprofile::{self, encode_stream, EncodeFailure, Errors, TextProfile};
use role_contract::topology::Frame;
use role_contract::topology_cli::push_frame;
use role_contract::turn_budget::{
    self as budget, Declaration, Failure, PathValue, PrintSite, Raised, SelfTestStop,
};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;

pub const OWNER_ENV: &str = "TURN_BUDGET_OWNER_PATH";
pub const OBSERVER_ENV: &str = "TURN_BUDGET_ROUTE_OBSERVER";
/// Newline-separated names of the module namespace of the Python program this one stands for (a fact the adapter marshals for
/// `_self_test`; this program owns the tripwire list, its order and the labels). Unset: the standalone program's own names.
pub const MODULE_NAMES_ENV: &str = "TURN_BUDGET_MODULE_NAMES";
/// Status of a run that could not be set up (private to this program; the owner never exits with 2 on these shapes except `--self-test`).
pub const EXIT_CONTROL_FAILURE: i32 = 2;
pub const EXIT_USAGE: i32 = 2;

const PROFILE_VARIABLES: [&str; 6] = [
    "LC_ALL",
    "LC_CTYPE",
    "LANG",
    "PYTHONUTF8",
    "PYTHONIOENCODING",
    "PYTHONCOERCECLOCALE",
];

/// What the process environment holds that this program reads.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    pub owner: Option<Vec<u8>>,
    pub observer: Option<OsString>,
    pub module_names: Option<Vec<Vec<u8>>>,
    pub text_variables: Vec<(String, Vec<u8>)>,
}

impl Environment {
    pub fn from_process() -> Self {
        let non_empty = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        Environment {
            owner: non_empty(OWNER_ENV).map(OsString::into_vec),
            observer: non_empty(OBSERVER_ENV),
            module_names: non_empty(MODULE_NAMES_ENV).map(|value| {
                value
                    .into_vec()
                    .split(|byte| *byte == b'\n')
                    .map(<[u8]>::to_vec)
                    .collect()
            }),
            text_variables: PROFILE_VARIABLES
                .iter()
                .filter_map(|name| {
                    std::env::var_os(name).map(|value| (name.to_string(), value.into_vec()))
                })
                .collect(),
        }
    }

    /// The text profile of the environment (no global setting is touched).
    pub fn text_profile(&self) -> Result<TextProfile, String> {
        TextProfile::from_environment(&|name| {
            self.text_variables
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        })
    }
}

fn text(value: &CallerText) -> Text {
    Text::from_code_points(value.code_points().to_vec())
}

// ── the owner's own source lines that appear in a traceback ──────────────────────────────

fn main_frame(line: u32, code: &'static str, expr: &'static str) -> Frame {
    Frame {
        file: Vec::new(),
        line,
        function: "main",
        code,
        expr,
    }
}

fn module_frame() -> Frame {
    Frame {
        file: Vec::new(),
        line: 179,
        function: "<module>",
        code: "sys.exit(main())",
        expr: "main()",
    }
}

/// `main` line 164, `return _self_test()`.
fn self_test_call() -> Frame {
    main_frame(164, "return _self_test()", "_self_test()")
}

const DUMP_CAPS_CODE: &str = "json.dump({\"max_turns\": schema_cap(\"max_turns\"),";
const DUMP_DECLARE_CODE: &str =
    "json.dump(declare(a.max_turns, a.timeout_seconds, source=a.source or \"\"),";

/// `main` line 172: the call of `declare` inside the `json.dump(...)` statement.
fn declare_call() -> Frame {
    main_frame(
        172,
        DUMP_DECLARE_CODE,
        "declare(a.max_turns, a.timeout_seconds, source=a.source or \"\")",
    )
}

const ARGPARSE: &[u8] = b"/usr/lib/python3.12/argparse.py";

fn argparse_frame(
    line: u32,
    function: &'static str,
    code: &'static str,
    expr: &'static str,
) -> Frame {
    Frame {
        file: ARGPARSE.to_vec(),
        line,
        function,
        code,
        expr,
    }
}

/// Frames of `ap.parse_args()` reaching `_HelpAction` and the write of the help text. They were first DERIVED from the argparse
/// 3.12 source lines of this host; the ONE actual capture R49C40 (`--help` under `PYTHONIOENCODING=ascii`) shows the same eight
/// argparse rows and the owner rows 162/179, so the table is now OBSERVED for this branch (golden `c40.json`). That covers this
/// one branch, not every codec or width.
fn help_frames() -> Vec<Frame> {
    vec![
        main_frame(162, "a = ap.parse_args()", "ap.parse_args()"),
        argparse_frame(
            1908,
            "parse_args",
            "args, argv = self.parse_known_args(args, namespace)",
            "self.parse_known_args(args, namespace)",
        ),
        argparse_frame(
            1941,
            "parse_known_args",
            "namespace, args = self._parse_known_args(args, namespace)",
            "self._parse_known_args(args, namespace)",
        ),
        argparse_frame(
            2162,
            "_parse_known_args",
            "start_index = consume_optional(start_index)",
            "consume_optional(start_index)",
        ),
        argparse_frame(
            2102,
            "consume_optional",
            "take_action(action, args, option_string)",
            "take_action(action, args, option_string)",
        ),
        argparse_frame(
            2017,
            "take_action",
            "action(self, namespace, argument_values, option_string)",
            "action(self, namespace, argument_values, option_string)",
        ),
        argparse_frame(
            1156,
            "__call__",
            "parser.print_help()",
            "parser.print_help()",
        ),
        argparse_frame(
            2648,
            "print_help",
            "self._print_message(self.format_help(), file)",
            "self._print_message(self.format_help(), file)",
        ),
        argparse_frame(
            2654,
            "_print_message",
            "file.write(message)",
            "file.write(message)",
        ),
    ]
}

/// `json.dump` at line 180: `fp.write(chunk)` (observed). An `OSError` raised inside the call spans the whole line (no carets); a `None` stream fails at the
/// attribute `fp.write` (carets under `fp.write`, observed in the closed-stdout row).
fn dump_write_frame(absent: bool) -> Frame {
    Frame {
        file: b"/usr/lib/python3.12/json/__init__.py".to_vec(),
        line: 180,
        function: "dump",
        code: "fp.write(chunk)",
        expr: if absent { "fp.write" } else { "" },
    }
}

/// The traceback text: `Traceback...`, each frame (owner frames get the owner path), then `Class: message`.
fn traceback(owner: &[u8], frames: &[Frame], class: &str, message: &Text) -> Text {
    let mut out = Text::new("Traceback (most recent call last):\n");
    for frame in std::iter::once(&module_frame()).chain(frames.iter()) {
        let mut shown = frame.clone();
        if shown.file.is_empty() {
            shown.file = owner.to_vec();
        }
        push_frame(&mut out, &shown);
    }
    let printed = if class == "JSONDecodeError" {
        "json.decoder.JSONDecodeError"
    } else {
        class
    };
    out.push_str(printed);
    if !message.is_empty() {
        out.push_str(": ");
        out.push_py(message);
    }
    out.push_str("\n");
    out
}

// ── streams ───────────────────────────────────────────────────────

/// The traceback of an unhandled exception: the interpreter flushes `sys.stdout` first (so buffered stdout precedes it on a merged stream), then writes stderr.
fn write_traceback(io: &mut Streams<'_>, out: &Text, profile: &TextProfile) {
    io.flush_before_traceback();
    write_error_stream(io, out, profile);
}

/// stderr: the stdout codec with `backslashreplace` (never fails).
fn write_error_stream(io: &mut Streams<'_>, out: &Text, profile: &TextProfile) {
    let bytes = encode_stream(out, profile.stdout, Errors::BackslashReplace).unwrap_or_default();
    let _ = io.stderr.write_all(&bytes);
}

/// Why a write to stdout did not complete.
enum Stop {
    Encode(EncodeFailure),
    Write(OutputFailure),
    /// `sys.stdout is None` (descriptor 1 closed when the process started): `json.dump` / `sys.stdout.write` fail at the attribute.
    Absent,
}

impl Stop {
    fn line(&self) -> (&'static str, Text) {
        match self {
            Stop::Encode(failure) => ("UnicodeEncodeError", Text::new(&failure.message)),
            Stop::Write(failure) => (failure.class, Text::new(&failure.message)),
            Stop::Absent => (
                "AttributeError",
                Text::new("'NoneType' object has no attribute 'write'"),
            ),
        }
    }
}

/// One `write` of a text to `sys.stdout`: the profile's codec, then the buffered layers.
fn write_text(io: &mut Streams<'_>, out: &Text, profile: &TextProfile) -> Result<(), Stop> {
    if io.stdout_is_absent() {
        return Err(Stop::Absent);
    }
    let bytes = encode_stream(out, profile.stdout, profile.stdout_errors).map_err(Stop::Encode)?;
    io.write_encoded(&bytes).map_err(Stop::Write)
}

/// `print(out)`: the text, then the newline, as two writes; a line-buffered terminal flushes after it.
fn print_line(io: &mut Streams<'_>, out: &Text, profile: &TextProfile) -> Result<(), Stop> {
    // `print` with `sys.stdout` None does nothing and fails nothing
    if io.stdout_is_absent() {
        return Ok(());
    }
    write_text(io, out, profile)?;
    write_text(io, &Text::new("\n"), profile)?;
    match io.flush_if_terminal() {
        Ok(()) => Ok(()),
        Err(PrintFailure::Write(failure)) => Err(Stop::Write(failure)),
        Err(PrintFailure::Encode(_)) => unreachable!("flushing encodes nothing"),
    }
}

/// `json.dump(value, sys.stdout, ensure_ascii=False, indent=2)`: chunk by chunk, so a failing chunk leaves the chunks before it.
fn dump(io: &mut Streams<'_>, value: &PyValue, profile: &TextProfile) -> Result<(), DumpStop> {
    let mut sink = |chunk: &Text| write_text(io, chunk, profile);
    value
        .json_dump_chunks(2, &mut sink)
        .map_err(|error| match error {
            StreamError::Dump(error) => DumpStop::Value(error),
            StreamError::Sink(stop) => DumpStop::Stream(stop),
        })
}

enum DumpStop {
    Value(DumpError),
    Stream(Stop),
}

// ── observer ────────────────────────────────────────────────────

/// The route event written *before* the business starts (`start`): which route, which shape, by which process.
fn observe(
    env: &Environment,
    route: &str,
    reason: Option<&str>,
    owner: Option<&[u8]>,
    shape: Option<&str>,
) -> Result<(), String> {
    observe_event(env, "start", route, reason, owner, shape, None)
}

/// One raw route event appended to the file `TURN_BUDGET_ROUTE_OBSERVER` names (nothing is written when it is unset):
/// `event` (`start` before the business, `end` with the outcome), `route`, `shape`, the Rust process's own `pid` and `ppid`,
/// the declared `owner_path` and the `outcome`. It is the only way to attribute a call to this program; nothing in it is
/// derived from the business output.
pub fn observe_event(
    env: &Environment,
    event: &str,
    route: &str,
    reason: Option<&str>,
    owner: Option<&[u8]>,
    shape: Option<&str>,
    outcome: Option<&str>,
) -> Result<(), String> {
    let Some(path) = &env.observer else {
        return Ok(());
    };
    let mut line = PyDict::new();
    line.set("event", PyValue::str(event));
    line.set("route", PyValue::str(route));
    line.set("reason", reason.map_or(PyValue::None, PyValue::str));
    line.set("pid", PyValue::int(std::process::id()));
    line.set("ppid", PyValue::int(std::os::unix::process::parent_id()));
    line.set(
        "owner_path",
        owner.map_or(PyValue::None, |bytes| {
            PyValue::Str(Text::from_surrogateescape(bytes))
        }),
    );
    line.set("shape", shape.map_or(PyValue::None, PyValue::str));
    line.set("outcome", outcome.map_or(PyValue::None, PyValue::str));
    let rendered = PyValue::Dict(line)
        .dumps_ascii()
        .map_err(|error| error.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{rendered}"))
        .map_err(|error| format!("the route observation cannot be written: {error}"))
}

fn control_failure(io: &mut Streams<'_>, message: &str) -> i32 {
    let _ = io
        .stderr
        .write_all(format!("[turn_budget] ERROR {message}\n").as_bytes());
    EXIT_CONTROL_FAILURE
}

// ── the run ────────────────────────────────────────────────────

/// Runs the program. `args` excludes the program name; the return value is the exit status.
pub fn run(args: &[Vec<u8>], env: &Environment, io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, env, io);
    // the exit-120 notice names the codec the text layer really has
    let codec = env
        .text_profile()
        .map_or("utf-8", |profile| profile.stdout.name());
    let status = io.finish_with_codec(status, codec);
    let route = if env.owner.is_some() && env.text_profile().is_ok() {
        "NativeCurrentSource"
    } else {
        "ControlFailure"
    };
    let outcome = format!("exit:{status}");
    if let Err(message) = observe_event(
        env,
        "end",
        route,
        None,
        env.owner.as_deref(),
        None,
        Some(&outcome),
    ) {
        // the output is already out: the status stays, the lost observation is reported
        let _ = io
            .stderr
            .write_all(format!("[turn_budget] ERROR {message}\n").as_bytes());
    }
    status
}

fn dispatch(args: &[Vec<u8>], env: &Environment, io: &mut Streams<'_>) -> i32 {
    let Some(owner) = env.owner.as_deref() else {
        let _ = observe(env, "ControlFailure", Some("owner path unset"), None, None);
        return control_failure(
            io,
            &format!("{OWNER_ENV} is not set; the launcher turn_budget.sh exports the owner program beside it"),
        );
    };
    let profile = match env.text_profile() {
        Ok(profile) => profile,
        Err(reason) => {
            if let Err(message) =
                observe(env, "UnsupportedProfile", Some(&reason), Some(owner), None)
            {
                return control_failure(io, &message);
            }
            return control_failure(
                io,
                &format!("the text profile of this environment is not modelled ({reason}); no other program is run"),
            );
        }
    };
    pyprofile::with_profile(&profile, || native(args, env, owner, &profile, io))
}

fn native(
    args: &[Vec<u8>],
    env: &Environment,
    owner: &[u8],
    profile: &TextProfile,
    io: &mut Streams<'_>,
) -> i32 {
    let decoded: Vec<CallerText> = args
        .iter()
        .map(|arg| {
            let decoded = pyprofile::fs_decode(arg);
            CallerText::from_code_points(decoded.code_points().to_vec())
        })
        .collect();
    let columns = terminal::columns();
    let matches = match argv::parse_with(&TURN_BUDGET, &decoded) {
        Outcome::Help => {
            if let Err(message) =
                observe(env, "NativeCurrentSource", None, Some(owner), Some("help"))
            {
                return control_failure(io, &message);
            }
            let help = TURN_BUDGET.help(columns);
            // `_HelpAction` -> `print_help` -> `file.write(message)`: a stdout codec that cannot spell the help raises the
            // owner's `UnicodeEncodeError` (argparse swallows only `OSError` there) and the program dies with status 1.
            // With no stdout (`None`) argparse writes the help to stderr instead and nothing is encoded for stdout.
            if !io.stdout_is_absent() {
                if let Err(failure) =
                    encode_stream(&Text::new(help), profile.stdout, profile.stdout_errors)
                {
                    let stop = Stop::Encode(failure);
                    let (class, message) = stop.line();
                    write_traceback(
                        io,
                        &traceback(owner, &help_frames(), class, &message),
                        profile,
                    );
                    return 1;
                }
            }
            io.print_help(help);
            return 0;
        }
        Outcome::Error(message) => {
            if let Err(message) = observe(
                env,
                "NativeCurrentSource",
                None,
                Some(owner),
                Some("usage-error"),
            ) {
                return control_failure(io, &message);
            }
            let mut out = Text::new(TURN_BUDGET.usage(columns));
            out.push_str(TURN_BUDGET.prog);
            out.push_str(": error: ");
            out.push_py(&text(&message));
            out.push_str("\n");
            write_error_stream(io, &out, profile);
            return EXIT_USAGE;
        }
        Outcome::Run(matches) => matches,
    };
    let shape = if matches.flag(arg::SELF_TEST) {
        "self-test"
    } else if matches.flag(arg::CAPS) {
        "caps"
    } else {
        "declare"
    };
    if let Err(message) = observe(env, "NativeCurrentSource", None, Some(owner), Some(shape)) {
        return control_failure(io, &message);
    }
    let repo = budget::repo_of(owner);
    let schema = PathValue::text_of(&pyprofile::fs_decode(&budget::request_schema_of(&repo)));
    match shape {
        "self-test" => run_self_test(owner, env, &schema, profile, io),
        "caps" => run_caps(owner, &schema, profile, io),
        _ => run_declare(owner, &matches, &schema, profile, io),
    }
}

fn int_of(matches: &argv::Matches, action: usize) -> PyValue {
    matches
        .int(action)
        .map_or(PyValue::None, |number| PyValue::int(number.clone()))
}

/// The owner dies with an uncaught exception: the traceback on stderr, status 1. A `SystemExit` with a message prints just the message.
fn die(
    io: &mut Streams<'_>,
    owner: &[u8],
    profile: &TextProfile,
    caller: Vec<Frame>,
    raised: &Raised,
) -> i32 {
    if raised.is_system_exit() {
        let mut out = raised.message.clone();
        out.push_str("\n");
        write_error_stream(io, &out, profile);
        return 1;
    }
    let mut frames = caller;
    frames.extend(raised.frames.iter().cloned());
    write_traceback(
        io,
        &traceback(owner, &frames, raised.class, &raised.message),
        profile,
    );
    1
}

/// A shape this program does not carry (never reached by this command line; kept visible).
fn unsupported(io: &mut Streams<'_>, why: &str) -> i32 {
    control_failure(io, &format!("unsupported shape: {why}"))
}

/// `json.dump` ended without a write failure: the value could not be spelled. Not reachable from this command line's real shapes:
/// the `--caps` document holds two values read by `json.load` (an integer of at most 4300 digits, which `str()` also prints, or a
/// `bool`) and a fixed string, and the `declare` result holds `int()`-parsed argparse integers (the same 4300-digit bound) or
/// `None` and a string, all at nesting depth one - so neither `ValueError` (digit limit) nor `RecursionError` can arise at the
/// dump. The importers' API never dumps (integers cross the bridge as hexadecimal), and the original business limits of the
/// f-strings stay where reachable (`repr_checked`). Kept as a visible control failure so a future shape cannot pass unnoticed.
fn dump_failed(
    io: &mut Streams<'_>,
    owner: &[u8],
    profile: &TextProfile,
    call: Frame,
    stop: DumpStop,
) -> i32 {
    match stop {
        DumpStop::Stream(stop) => {
            let absent = matches!(stop, Stop::Absent);
            let (class, message) = stop.line();
            write_traceback(
                io,
                &traceback(owner, &[call, dump_write_frame(absent)], class, &message),
                profile,
            );
            1
        }
        DumpStop::Value(error) => control_failure(
            io,
            &format!("json.dump failed in a shape this program does not model: {error}"),
        ),
    }
}

fn write_newline(
    io: &mut Streams<'_>,
    owner: &[u8],
    profile: &TextProfile,
    line: u32,
) -> Option<i32> {
    match write_text(io, &Text::new("\n"), profile) {
        Ok(()) => None,
        Err(stop) => {
            let absent = matches!(stop, Stop::Absent);
            let (class, message) = stop.line();
            let frame = main_frame(
                line,
                "sys.stdout.write(\"\\n\")",
                if absent { "sys.stdout.write" } else { "" },
            );
            write_traceback(io, &traceback(owner, &[frame], class, &message), profile);
            Some(1)
        }
    }
}

fn run_caps(owner: &[u8], schema: &PathValue, profile: &TextProfile, io: &mut Streams<'_>) -> i32 {
    let document = match budget::caps_document(schema) {
        Ok(document) => document,
        Err(Failure::Raised(raised)) => return die(io, owner, profile, vec![], &raised),
        Err(Failure::Unsupported(why)) => return unsupported(io, &why),
    };
    if let Err(stop) = dump(io, &document, profile) {
        return dump_failed(
            io,
            owner,
            profile,
            main_frame(166, DUMP_CAPS_CODE, ""),
            stop,
        );
    }
    write_newline(io, owner, profile, 170).unwrap_or(0)
}

fn run_declare(
    owner: &[u8],
    matches: &argv::Matches,
    schema: &PathValue,
    profile: &TextProfile,
    io: &mut Streams<'_>,
) -> i32 {
    let source = match matches.value(arg::SOURCE) {
        Some(value) if !value.is_empty() => PyValue::Str(text(value)),
        _ => PyValue::str(""),
    };
    let declaration = Declaration::new(
        int_of(matches, arg::MAX_TURNS),
        int_of(matches, arg::TIMEOUT_SECONDS),
        source,
    );
    let declared = match budget::declare(&declaration, schema) {
        Ok(declared) => declared,
        Err(Failure::Raised(raised)) => {
            return die(io, owner, profile, vec![declare_call()], &raised)
        }
        Err(Failure::Unsupported(why)) => return unsupported(io, &why),
    };
    if let Err(stop) = dump(io, &declared, profile) {
        return dump_failed(
            io,
            owner,
            profile,
            main_frame(172, DUMP_DECLARE_CODE, ""),
            stop,
        );
    }
    write_newline(io, owner, profile, 174).unwrap_or(0)
}

fn run_self_test(
    owner: &[u8],
    env: &Environment,
    schema: &PathValue,
    profile: &TextProfile,
    io: &mut Streams<'_>,
) -> i32 {
    let names: Option<Vec<Text>> = env.module_names.as_ref().map(|names| {
        names
            .iter()
            .map(|name| pyprofile::fs_decode(name))
            .collect()
    });
    let mut emit = |line: &Text, _site: PrintSite| print_line(io, line, profile);
    match budget::self_test(schema, names.as_deref(), &mut emit) {
        Ok(status) => status,
        Err(SelfTestStop::Raised(raised)) => {
            die(io, owner, profile, vec![self_test_call()], &raised)
        }
        Err(SelfTestStop::Unsupported(why)) => unsupported(io, &why),
        Err(SelfTestStop::Print(stop, site)) => {
            let (class, message) = stop.line();
            let mut frames = vec![
                self_test_call(),
                Frame {
                    file: Vec::new(),
                    line: site.line,
                    function: "_self_test",
                    code: site.code,
                    expr: "",
                },
            ];
            if site.line != 151 {
                // every other print is the one inside the local helper `chk`
                frames.push(Frame {
                    file: Vec::new(),
                    line: 98,
                    function: "chk",
                    code: "print(\"  [%s] %s\" % (\"PASS\" if cond else \"FAIL\", label))",
                    expr: "",
                });
            }
            write_traceback(io, &traceback(owner, &frames, class, &message), profile);
            1
        }
    }
}
