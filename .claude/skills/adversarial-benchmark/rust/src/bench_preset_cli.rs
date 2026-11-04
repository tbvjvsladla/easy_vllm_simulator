//! Command line of `bench_preset`: `validate`, `default`, `resolve` and `--self-test`.
//!
//! The interface is the original program's `argparse` interface byte for byte (usage, help and error text of the top
//! parser and of each subparser, see [`crate::argv`]); the Rust-only commands live in the separate
//! `bench_preset_api` program ([`crate::bench_preset_api_cli`]).
//!
//! Where the original found its own files (`SKILL_DIR`, `REPO_ROOT`) this program is given them by
//! `BENCH_PRESET_SKILL_DIR` and `BENCH_PRESET_REPO_ROOT` (see [`Runtime`]); a command that needs one without it fails
//! by name. The campaign owner is the Python `campaign_init.py`, reached through the repeat axis's query adapter
//! (`REPEAT_AXIS_CAMPAIGN_ADAPTER`, `REPEAT_AXIS_PYTHON`).
//!
//! Exit status: 0 success, 2 for a refusal (`PresetError` or an argument error), 1 for a failed self-test or an
//! exception the original does not handle (a traceback marker and the final `Class: message` line).

use crate::argv::{
    self, bench_preset_args as arg, Matches, Outcome, Spec, BENCH_PRESET, BENCH_PRESET_DEFAULT,
    BENCH_PRESET_RESOLVE, BENCH_PRESET_VALIDATE,
};
use crate::bench_preset::{
    self, load_preset, parse_knob_reasons, parse_pins, preset_error, repo_root_from_environment,
    resolve, serving_caps, shell_text, CallDepth, ResolveArguments, SkillDir, BINDINGS_REL,
    DEFAULT_PRESET, FULL_BENCH, KNOB_SPECS, REPEAT_KIND,
};
use crate::bench_preset_selftest::run_self_test;
use crate::cli::{on_large_stack, write_stderr, PrintFailure, Streams, TRACEBACK_HEADER};
use crate::guidellm::Failure;
use crate::pyfs;
use crate::pystr::{is_decimal_digit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_owner::{PythonCampaignOwner, RepoRoot};
use crate::repeat_text::cat;
use crate::terminal;

/// What a run is configured with besides its arguments.
#[derive(Clone, Debug)]
pub struct Runtime {
    pub skill_dir: SkillDir,
    pub repo_root: RepoRoot,
    pub owner: PythonCampaignOwner,
}

impl Runtime {
    pub fn from_environment() -> Self {
        Runtime {
            skill_dir: SkillDir::from_environment(),
            repo_root: repo_root_from_environment(),
            owner: PythonCampaignOwner::from_environment(),
        }
    }
}

const SUBCOMMANDS: [(&str, &Spec); 3] = [
    ("validate", &BENCH_PRESET_VALIDATE),
    ("default", &BENCH_PRESET_DEFAULT),
    ("resolve", &BENCH_PRESET_RESOLVE),
];
const UNRECOGNIZED: &str = "unrecognized arguments: ";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], runtime: &Runtime, io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, runtime, io);
    io.finish(status)
}

// ── argparse: top parser with subcommands ───────────────────────────────────────

enum Command {
    Validate(Matches),
    Default(Matches),
    Resolve(Matches),
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

/// Whether the top parser classifies `arg` as an option rather than a positional (`_parse_optional` against the
/// options `-h`, `--help` and `--self-test`).
fn top_parser_sees_option(arg: &[u32]) -> bool {
    let dash = u32::from('-');
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
    match argv::parse_with(&BENCH_PRESET, leading) {
        Outcome::Help => return Parsed::Help(&BENCH_PRESET),
        Outcome::Error(message) => match unrecognized_tail(&message) {
            Some(tail) => leftover = Some(tail),
            None => return Parsed::Failed(&BENCH_PRESET, message),
        },
        Outcome::Run(matches) => self_test = matches.flag(arg::SELF_TEST),
    }

    let Some(word) = rest.first() else {
        return match leftover {
            Some(tail) => Parsed::Failed(&BENCH_PRESET, unrecognized(&tail)),
            None => Parsed::Run {
                self_test,
                command: None,
            },
        };
    };
    if word.equals("--") && rest.len() == 1 {
        return Parsed::Failed(
            &BENCH_PRESET,
            unrecognized(&join_extras(leftover, PyStr::new("--"))),
        );
    }
    let Some((name, spec)) = SUBCOMMANDS.iter().find(|(name, _)| word.equals(name)) else {
        let choices: Vec<String> = SUBCOMMANDS
            .iter()
            .map(|(name, _)| PyStr::new(name).repr())
            .collect();
        let mut message = PyStr::new("argument cmd: invalid choice: ");
        message.push_str(&word.repr());
        message.push_str(&format!(" (choose from {})", choices.join(", ")));
        return Parsed::Failed(&BENCH_PRESET, message);
    };
    let matches = match argv::parse_with(spec, &rest[1..]) {
        Outcome::Help => return Parsed::Help(spec),
        Outcome::Error(message) => match unrecognized_tail(&message) {
            Some(tail) => {
                return Parsed::Failed(&BENCH_PRESET, unrecognized(&join_extras(leftover, tail)))
            }
            None => return Parsed::Failed(spec, message),
        },
        Outcome::Run(matches) => matches,
    };
    if let Some(tail) = leftover {
        return Parsed::Failed(&BENCH_PRESET, unrecognized(&tail));
    }
    let command = match *name {
        "validate" => Command::Validate(matches),
        "default" => Command::Default(matches),
        _ => Command::Resolve(matches),
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
        let report = on_large_stack(|| run_self_test(runtime));
        if let Err(failure) = io.print_lines(&PyStr::new(&report.stdout)) {
            return print_failed(io, &failure);
        }
        write_stderr(io, &PyStr::new(&report.stderr));
        return report.exit_code;
    }
    let Some(command) = command else {
        io.print_help(BENCH_PRESET.help(terminal::columns()));
        return 2;
    };
    // The output lines are collected while the command runs and printed one by one afterwards; a refusal part way
    // leaves what the command did to the file system, but the original prints only after its last refusal point.
    match on_large_stack(|| execute(&command, runtime)) {
        Ok(lines) => {
            for line in &lines {
                if let Err(failure) = io.print(line) {
                    return print_failed(io, &failure);
                }
            }
            0
        }
        Err(failure) if failure.class == bench_preset::PRESET_ERROR => {
            let mut text = PyStr::new("[bench_preset] ERROR ");
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
/// the traceback shows the bare `msg` while `str(exception)` appends ` (file, line N)`; the suffix is dropped.
fn traceback_line(failure: &Failure) -> PyStr {
    // An exception class of another module is printed with its module: the original's `repeat_axis.RepeatAxisError`
    // (a `PresetError` is defined in the program itself, `__main__`, and is never printed bare because it is caught).
    if failure.class == "RepeatAxisError" {
        let mut text = PyStr::new("repeat_axis.RepeatAxisError");
        if !failure.message.is_empty() {
            text.push_str(": ");
            text.push_py(&failure.message);
        }
        return text;
    }
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

fn texts_value(matches: &Matches, action: usize) -> PyValue {
    PyValue::List(
        matches
            .values_of(action)
            .iter()
            .map(|text| PyValue::Str(text.clone()))
            .collect(),
    )
}

/// What the command prints (one `print` per line); the file side effects have happened by then.
fn execute(command: &Command, runtime: &Runtime) -> Result<Vec<PyStr>, Failure> {
    match command {
        Command::Validate(matches) => {
            let preset = text_value(matches, arg::validate::PRESET);
            let loaded = load_preset(&preset, &runtime.skill_dir, CallDepth::PRESET_COMMAND)?;
            Ok(vec![cat!("[bench_preset] OK — ", loaded.path)])
        }
        Command::Default(matches) => default_line(matches, runtime),
        Command::Resolve(matches) => resolve_lines(matches, runtime),
    }
}

fn default_line(matches: &Matches, runtime: &Runtime) -> Result<Vec<PyStr>, Failure> {
    let preset = text_value(matches, arg::default::PRESET);
    let loaded = load_preset(&preset, &runtime.skill_dir, CallDepth::PRESET_COMMAND)?;
    let knob = matches
        .value(arg::default::KNOB)
        .cloned()
        .unwrap_or_default();
    let knobs = match &loaded.document {
        PyValue::Dict(map) => map.get("knobs").cloned().unwrap_or(PyValue::None),
        other => other.clone(),
    };
    let PyValue::Dict(knob_map) = knobs.peel() else {
        unreachable!("a validated preset has a knobs object")
    };
    let Some(knob_doc) = knob.to_utf8().and_then(|name| knob_map.get(&name)) else {
        let mut names: Vec<PyValue> = knob_map
            .iter()
            .map(|(name, _)| PyValue::Str(name.clone()))
            .collect();
        let keys = names.clone();
        names = crate::pysort::sorted_by_key(names, &keys)?;
        return Err(preset_error(cat!(
            "knob ",
            knob.repr(),
            " 를 모른다(",
            PyValue::List(names).repr(),
            ")"
        )));
    };
    let PyValue::Dict(knob_doc) = knob_doc.peel() else {
        unreachable!("a validated knob is an object")
    };
    let default = knob_doc.get("default").cloned().unwrap_or(PyValue::None);
    if matches!(default.peel(), PyValue::None) {
        return Err(preset_error(cat!(
            "knob ",
            knob.repr(),
            " 는 기본값이 없다(기본값 없음 결정)"
        )));
    }
    let shown = match default.peel() {
        PyValue::List(items) => {
            let mut text = PyStr::default();
            for (index, entry) in items.iter().enumerate() {
                if index > 0 {
                    text.push_str(",");
                }
                text.push_py(&entry.display_text());
            }
            text
        }
        other => other.display_text(),
    };
    let name = if preset.is_truthy() {
        preset.display_text()
    } else {
        PyStr::new(DEFAULT_PRESET)
    };
    Ok(vec![cat!(shown, "\u{1f}", name)])
}

/// `str(path.relative_to(root))`, or `None` when `path` is not under `root`.
fn relative_to(path: &PyStr, root: &PyStr) -> Option<PyStr> {
    let path = pyfs::normalize(path);
    let root = pyfs::normalize(root);
    if path == root {
        return Some(PyStr::new("."));
    }
    let mut prefix = root.clone();
    if !prefix.ends_with_code_point(u32::from('/')) {
        prefix.push_str("/");
    }
    path.code_points()
        .strip_prefix(prefix.code_points())
        .map(|rest| PyStr::from_code_points(rest.to_vec()))
}

fn resolve_lines(matches: &Matches, runtime: &Runtime) -> Result<Vec<PyStr>, Failure> {
    use arg::resolve as option;
    let blank = |value: &PyValue| matches!(value, PyValue::Str(text) if text.strip().is_empty());
    let pinned_by = text_value(matches, option::PINNED_BY);
    if blank(&pinned_by) {
        return Err(preset_error(PyStr::new(
            "--pinned-by 는 사람 발화의 전사다 — 빈 값은 귀속이 아니다",
        )));
    }
    let approved_by = text_value(matches, option::APPROVED_BY);
    if blank(&approved_by) {
        return Err(preset_error(PyStr::new(
            "--approved-by 는 사람 발화의 전사다 — 빈 값은 승인이 아니다",
        )));
    }
    let preset = text_value(matches, option::PRESET);
    let loaded = load_preset(&preset, &runtime.skill_dir, CallDepth::RESOLVE_COMMAND)?;
    let name = if preset.is_truthy() {
        preset.clone()
    } else {
        PyValue::str(DEFAULT_PRESET)
    };
    let mut cli = PyDict::new();
    for spec in &KNOB_SPECS {
        let action = match spec.name {
            "levels" => Some(option::LEVELS),
            "input_len" => Some(option::INPUT_LEN),
            "output_len" => Some(option::OUTPUT_LEN),
            "num_prompts" => Some(option::NUM_PROMPTS),
            "warmups" => Some(option::WARMUPS),
            "repeats" => Some(option::REPEATS),
            "backend" => Some(option::BACKEND),
            "bench_budget_mib" => Some(option::BENCH_BUDGET_MIB),
            _ => None,
        };
        cli.set(
            spec.name,
            action.map_or(PyValue::None, |action| text_value(matches, action)),
        );
    }
    let dry_run = PyValue::Bool(matches.flag(option::DRY_RUN));
    let serving = serving_caps(&text_value(matches, option::SERVING_YAML), &dry_run)?;
    let pins = parse_pins(&texts_value(matches, option::PIN))?;
    let reasons = parse_knob_reasons(&texts_value(matches, option::KNOB_REASON))?;
    let cli = PyValue::Dict(cli);
    let (pins, reasons) = (PyValue::Dict(pins), PyValue::Dict(reasons));
    let (tool, campaign_id, repeats_source) = (
        text_value(matches, option::TOOL),
        text_value(matches, option::CAMPAIGN_ID),
        text_value(matches, option::REPEATS_SOURCE),
    );
    let resolution = resolve(
        &ResolveArguments {
            preset_doc: &loaded.document,
            preset_name: &name,
            tool: &tool,
            cli: &cli,
            cli_pins: &pins,
            pinned_by: &pinned_by,
            knob_reasons: &reasons,
            approved_by: &approved_by,
            campaign_id: &campaign_id,
            serving: &serving,
            repo_root: &runtime.repo_root,
            repeats_source: &repeats_source,
            dry_run: &dry_run,
        },
        &runtime.owner,
    )?;
    let mut block = resolution.block;
    let relative = relative_to(&loaded.path, runtime.repo_root.get()?);
    let shown_path = relative.unwrap_or_else(|| loaded.path.clone());
    block.set("preset_path", PyValue::Str(shown_path.clone()));
    let bound = match &loaded.bindings {
        PyValue::Dict(map) => map
            .get(FULL_BENCH)
            .map_or(PyStr::new("None"), PyValue::display_text),
        _ => PyStr::new("None"),
    };
    block.set(
        "binding",
        PyValue::Str(cat!(
            FULL_BENCH,
            " → config/",
            bound,
            "/ (",
            BINDINGS_REL,
            ")"
        )),
    );
    let record_out = crate::pyfs::normalize(
        &matches
            .value(option::RECORD_OUT)
            .cloned()
            .unwrap_or_default(),
    );
    pyfs::mkdir_parents_exist_ok(&pyfs::parent(&record_out))?;
    let mut text = PyValue::Dict(block.clone())
        .dumps_pretty()
        .map_err(|error| Failure::new(error.python_type(), &error.to_string()))?;
    text.push_str("\n");
    pyfs::write_text(&record_out, &text)?;
    let mut lines = Vec::new();
    for spec in &KNOB_SPECS {
        if let Some(shell_key) = spec.shell_key {
            let value = resolution
                .values
                .get(spec.name)
                .cloned()
                .unwrap_or(PyValue::None);
            lines.push(cat!(shell_key, "\u{1f}", shell_text(&value)));
        }
    }
    let repeats_source_text = block
        .get("knobs")
        .and_then(|knobs| match knobs {
            PyValue::Dict(map) => map.get("repeats").cloned(),
            _ => None,
        })
        .and_then(|record| match record {
            PyValue::Dict(map) => map.get("repeats_source").cloned(),
            _ => None,
        })
        .map_or(PyStr::default(), |value| value.display_text());
    let cleaned: Vec<u32> = repeats_source_text
        .code_points()
        .iter()
        .map(|&p| if p == 0x1F { u32::from(' ') } else { p })
        .collect();
    lines.push(cat!(
        "REPEATS_SOURCE\u{1f}",
        PyStr::from_code_points(cleaned)
    ));
    lines.push(cat!("REPEAT_KIND\u{1f}", REPEAT_KIND));
    lines.push(cat!("PRESET_PATH\u{1f}", shown_path));
    lines.push(cat!(
        "DATASET_KIND\u{1f}",
        resolution
            .values
            .get("dataset_kind")
            .map_or(PyStr::new("None"), PyValue::display_text)
    ));
    Ok(lines)
}
