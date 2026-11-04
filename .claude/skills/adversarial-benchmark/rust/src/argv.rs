//! The original program's command line, parsed the way CPython 3.12.3 `argparse` parses it.
//!
//! Ported from `Lib/argparse.py` (<https://github.com/python/cpython/blob/v3.12.3/Lib/argparse.py>, PSF License;
//! see `notices/NOTICE.md`) for parsers made of `-h/--help`, value options and flags only, no positionals,
//! abbreviations allowed. Each original program contributes a [`Spec`] table: `sweep_stop.py` ([`SWEEP_STOP`]:
//! `--state STATE`, `--now-utc NOW_UTC`, `--self-test`) and `parse_bench.py` ([`PARSE_BENCH`]: required
//! `--bench-json`, `--engine-log`, `--json`) and `parse_guidellm.py` ([`PARSE_GUIDELLM`]: spec-axis, health and
//! prefix-cache inputs, `--benchmark-index`, `--max-error-rate`). Option strings are tried in the order argparse
//! registered them, and arguments are code points (Python decodes argv with `surrogateescape`).
//!
//! Usage and help text are laid out by argparse from the terminal width. For a fixed parser that layout is a
//! step function of `COLUMNS`, captured from the original program into `help_layouts*` and looked up by
//! [`Spec::usage`] / [`Spec::help`]; widths beyond the last captured step share its layout. How the width is found is in
//! [`crate::terminal`].

use crate::help_layouts::Layout;
use crate::pynumber::{float_from_str, int_from_str};
use crate::pystr::{is_decimal_digit, PyStr};
use num_bigint::BigInt;

/// The program name argparse derives from `sys.argv[0]` of the original `sweep_stop.py`.
pub const PROG: &str = "sweep_stop.py";

/// How an argparse action consumes arguments (`store` with one value, `store_true`, or `help`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Help,
    Value,
    Flag,
}

/// How `argparse` converts the text of a value option (`type=`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    Text,
    Float,
    Int,
}

/// One registered action. `name` is argparse's `_get_action_name`: the option strings joined with `/`.
#[derive(Clone, Copy, Debug)]
pub struct ActionDef {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub conversion: Conversion,
    /// `choices=`: the allowed texts (empty when unrestricted).
    pub choices: &'static [&'static str],
}

/// An action that takes no restrictions; the base of the tables below.
const PLAIN: ActionDef = ActionDef {
    name: "",
    kind: Kind::Flag,
    required: false,
    conversion: Conversion::Text,
    choices: &[],
};

const fn flag(name: &'static str) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Flag,
        ..PLAIN
    }
}

const fn text_option(name: &'static str, required: bool) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Value,
        required,
        ..PLAIN
    }
}

const fn number(name: &'static str, conversion: Conversion) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Value,
        conversion,
        ..PLAIN
    }
}

const fn choice(name: &'static str, choices: &'static [&'static str]) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Value,
        choices,
        ..PLAIN
    }
}

/// A fixed parser: its program name, actions, option strings in registration order and captured help layouts.
/// The generic part of the port is shared; each original program contributes only this table.
pub struct Spec {
    pub prog: &'static str,
    pub actions: &'static [ActionDef],
    /// Option strings in the order argparse registered them, each with the index of its action.
    pub options: &'static [(&'static str, usize)],
    layouts: &'static [Layout],
}

impl Spec {
    fn layout_for(&self, columns: u32) -> &'static Layout {
        self.layouts
            .iter()
            .rev()
            .find(|layout| layout.from_columns <= columns)
            .unwrap_or(&self.layouts[0])
    }

    /// `parser.format_usage()` at the given terminal width (in columns).
    pub fn usage(&self, columns: u32) -> &'static str {
        self.layout_for(columns).usage
    }

    /// `parser.format_help()` at the given terminal width (in columns).
    pub fn help(&self, columns: u32) -> &'static str {
        self.layout_for(columns).help
    }

    fn lookup(&self, option_string: &[u32]) -> Option<usize> {
        self.options
            .iter()
            .find(|(name, _)| equals(option_string, name))
            .map(|&(_, action)| action)
    }
}

const HELP_ACTION: usize = 0;
const STATE: usize = 1;
const NOW_UTC: usize = 2;
const SELF_TEST: usize = 3;

/// The original `sweep_stop.py` parser (`-h/--help` is added first by `ArgumentParser`).
pub const SWEEP_STOP: Spec = Spec {
    prog: PROG,
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--state", false),
        text_option("--now-utc", false),
        flag("--self-test"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--state", STATE),
        ("--now-utc", NOW_UTC),
        ("--self-test", SELF_TEST),
    ],
    layouts: crate::help_layouts::LAYOUTS,
};

/// Index of the `--bench-json` action in [`PARSE_BENCH`].
pub const BENCH_JSON: usize = 1;
/// Index of the `--engine-log` action in [`PARSE_BENCH`].
pub const ENGINE_LOG: usize = 2;
/// Index of the (accepted, unused) `--json` flag in [`PARSE_BENCH`].
pub const JSON_FLAG: usize = 3;

/// The original `parse_bench.py` parser.
pub const PARSE_BENCH: Spec = Spec {
    prog: "parse_bench.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--bench-json", true),
        text_option("--engine-log", false),
        flag("--json"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--bench-json", BENCH_JSON),
        ("--engine-log", ENGINE_LOG),
        ("--json", JSON_FLAG),
    ],
    layouts: crate::help_layouts_parse_bench::LAYOUTS,
};

/// Indices of the `parse_guidellm.py` actions.
pub mod guidellm_args {
    pub const BENCHMARKS_JSON: usize = 1;
    pub const ENGINE_LOG: usize = 2;
    pub const BENCHMARK_INDEX: usize = 3;
    pub const ACCEPT_LEN_SRC: usize = 4;
    pub const SPEC_AXIS_ABSENT: usize = 5;
    pub const POST_HEALTH_JSON: usize = 6;
    pub const PREFIX_CACHE_JSON: usize = 7;
    pub const MAX_ERROR_RATE: usize = 8;
    pub const JSON: usize = 9;
    pub const SELF_TEST: usize = 10;
}

/// The original `parse_guidellm.py` parser.
pub const PARSE_GUIDELLM: Spec = Spec {
    prog: "parse_guidellm.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--benchmarks-json", false),
        text_option("--engine-log", false),
        number("--benchmark-index", Conversion::Int),
        text_option("--accept-len-src", false),
        flag("--spec-axis-absent"),
        text_option("--post-health-json", false),
        text_option("--prefix-cache-json", false),
        number("--max-error-rate", Conversion::Float),
        flag("--json"),
        flag("--self-test"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--benchmarks-json", 1),
        ("--engine-log", 2),
        ("--benchmark-index", 3),
        ("--accept-len-src", 4),
        ("--spec-axis-absent", 5),
        ("--post-health-json", 6),
        ("--prefix-cache-json", 7),
        ("--max-error-rate", 8),
        ("--json", 9),
        ("--self-test", 10),
    ],
    layouts: crate::help_layouts_parse_guidellm::LAYOUTS,
};

/// Indices of the `roofline.py` actions.
pub mod roofline_args {
    pub const MODEL_PATH: usize = 1;
    pub const NAS_ROOT: usize = 2;
    pub const QUANT_ROOT: usize = 3;
    pub const MANIFEST: usize = 4;
    pub const TP: usize = 5;
    pub const ACCEPT_LEN: usize = 6;
    pub const ACCEPT_LEN_SOURCE: usize = 7;
    pub const ACCEPT_LEN_EVIDENCE: usize = 8;
    pub const BANDWIDTH_GBPS: usize = 9;
    pub const INTERCONNECT_GBPS: usize = 10;
    pub const ACT_DTYPE_BYTES: usize = 11;
    pub const COMM_LATENCY_US_PER_LAYER: usize = 12;
    pub const REALISTIC_FRACTION: usize = 13;
    pub const JSON: usize = 14;
}

/// The `accept_len` provenance vocabulary (`roofline.ACCEPT_LEN_SOURCES`); one list for the parser and the library.
pub const ACCEPT_LEN_SOURCE_NAMES: &[&str] = &["measured", "declared", "declared-absent", "absent"];

/// The original `roofline.py` parser.
pub const ROOFLINE: Spec = Spec {
    prog: "roofline.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--model-path", true),
        text_option("--nas-root", false),
        text_option("--quant-root", false),
        text_option("--manifest", false),
        number("--tp", Conversion::Int),
        number("--accept-len", Conversion::Float),
        choice("--accept-len-source", ACCEPT_LEN_SOURCE_NAMES),
        text_option("--accept-len-evidence", false),
        number("--bandwidth-gbps", Conversion::Float),
        number("--interconnect-gbps", Conversion::Float),
        number("--act-dtype-bytes", Conversion::Int),
        number("--comm-latency-us-per-layer", Conversion::Float),
        number("--realistic-fraction", Conversion::Float),
        flag("--json"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--model-path", 1),
        ("--nas-root", 2),
        ("--quant-root", 3),
        ("--manifest", 4),
        ("--tp", 5),
        ("--accept-len", 6),
        ("--accept-len-source", 7),
        ("--accept-len-evidence", 8),
        ("--bandwidth-gbps", 9),
        ("--interconnect-gbps", 10),
        ("--act-dtype-bytes", 11),
        ("--comm-latency-us-per-layer", 12),
        ("--realistic-fraction", 13),
        ("--json", 14),
    ],
    layouts: crate::help_layouts_roofline::LAYOUTS,
};

/// Indices of the `verdict_rule.py` actions.
pub mod verdict_args {
    pub const MEASURED: usize = 1;
    pub const ROOFLINE: usize = 2;
    pub const REFERENCE_TPS: usize = 3;
    pub const TARGET_TPS: usize = 4;
    pub const AUTHORITY: usize = 5;
    pub const TOLERANCE: usize = 6;
    pub const SPEC_SUPPORTED: usize = 7;
    pub const SPEC_DECLARED: usize = 8;
    pub const E_SEARCH: usize = 9;
    pub const NODE_VRAM_GIB: usize = 10;
    pub const BALANCE_TOL: usize = 11;
    pub const SELF_TEST: usize = 12;
    pub const JSON: usize = 13;
}

pub const AUTHORITY_NAMES: &[&str] = &["weak", "explicit", "explore"];
pub const SPEC_DECLARED_NAMES: &[&str] = &["on", "off", "unknown"];
pub const E_SEARCH_NAMES: &[&str] = &["hit", "empty", "no"];

/// The original `verdict_rule.py` parser.
pub const VERDICT_RULE: Spec = Spec {
    prog: "verdict_rule.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--measured", false),
        text_option("--roofline", false),
        number("--reference-tps", Conversion::Float),
        number("--target-tps", Conversion::Float),
        choice("--authority", AUTHORITY_NAMES),
        number("--tolerance", Conversion::Float),
        flag("--spec-supported"),
        choice("--spec-declared", SPEC_DECLARED_NAMES),
        choice("--e-search", E_SEARCH_NAMES),
        text_option("--node-vram-gib", false),
        number("--balance-tol", Conversion::Float),
        flag("--self-test"),
        flag("--json"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--measured", 1),
        ("--roofline", 2),
        ("--reference-tps", 3),
        ("--target-tps", 4),
        ("--authority", 5),
        ("--tolerance", 6),
        ("--spec-supported", 7),
        ("--spec-declared", 8),
        ("--e-search", 9),
        ("--node-vram-gib", 10),
        ("--balance-tol", 11),
        ("--self-test", 12),
        ("--json", 13),
    ],
    layouts: crate::help_layouts_verdict_rule::LAYOUTS,
};

/// `parser.format_usage()` of `sweep_stop.py` at the given terminal width.
pub fn usage(columns: u32) -> &'static str {
    SWEEP_STOP.usage(columns)
}

/// `parser.format_help()` of `sweep_stop.py` at the given terminal width.
pub fn help(columns: u32) -> &'static str {
    SWEEP_STOP.help(columns)
}

/// The values a successful parse produced, indexed by action: the last value given, and whether it was seen.
#[derive(Debug)]
pub struct Matches {
    values: Vec<Option<PyStr>>,
    seen: Vec<bool>,
    numbers: Vec<Option<ArgNumber>>,
}

/// The converted value of a `type=float` / `type=int` option.
#[derive(Clone, Debug, PartialEq)]
pub enum ArgNumber {
    Float(f64),
    Int(BigInt),
}

impl Matches {
    fn new(spec: &Spec) -> Self {
        Matches {
            values: vec![None; spec.actions.len()],
            seen: vec![false; spec.actions.len()],
            numbers: vec![None; spec.actions.len()],
        }
    }

    /// The value of a `Kind::Value` action, if given.
    pub fn value(&self, action: usize) -> Option<&PyStr> {
        self.values[action].as_ref()
    }

    /// The converted value of a `type=float` option, if given.
    pub fn float(&self, action: usize) -> Option<f64> {
        match self.numbers[action] {
            Some(ArgNumber::Float(number)) => Some(number),
            _ => None,
        }
    }

    /// The converted value of a `type=int` option, if given.
    pub fn int(&self, action: usize) -> Option<&BigInt> {
        match &self.numbers[action] {
            Some(ArgNumber::Int(number)) => Some(number),
            _ => None,
        }
    }

    /// Whether a `Kind::Flag` action was given.
    pub fn flag(&self, action: usize) -> bool {
        self.seen[action]
    }
}

#[derive(Debug)]
pub enum Outcome {
    Run(Matches),
    /// `-h` was reached before any error: print help and exit 0.
    Help,
    /// `parser.error(message)`: usage and `prog: error: message` on stderr, exit 2.
    Error(PyStr),
}

#[derive(Debug, Default)]
pub struct LegacyArgs {
    pub state: Option<PyStr>,
    pub now_utc: Option<PyStr>,
    pub self_test: bool,
}

#[derive(Debug)]
pub enum Parsed {
    Run(LegacyArgs),
    /// `-h` was reached before any error: print help and exit 0.
    Help,
    /// `parser.error(message)`: usage and `prog: error: message` on stderr, exit 2.
    Error(PyStr),
}

type Points = Vec<u32>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Separator {
    Absent,
    /// A short option followed directly by more characters (`-hx`): argparse's empty string.
    Joined,
    Equals,
}

struct OptionTuple {
    action: Option<usize>,
    option_string: Points,
    separator: Separator,
    explicit: Option<Points>,
}

fn points(text: &str) -> Points {
    text.chars().map(u32::from).collect()
}

fn equals(candidate: &[u32], text: &str) -> bool {
    candidate.iter().copied().eq(text.chars().map(u32::from))
}

fn starts_with(candidate: &[u32], prefix: &[u32]) -> bool {
    candidate.starts_with(prefix)
}

fn text(parts: &[&[u32]]) -> PyStr {
    PyStr::from_code_points(parts.concat())
}

fn quoted(value: &[u32]) -> String {
    PyStr::from_code_points(value.to_vec()).repr()
}

/// `^-\d+$|^-\d*\.\d+$` with Unicode digits; `$` also matches before one trailing newline.
fn is_negative_number(arg: &[u32]) -> bool {
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

/// `_get_option_tuples` for an argument that starts with `-` and is at least two characters long.
fn option_tuples(spec: &Spec, arg: &[u32]) -> Vec<OptionTuple> {
    let dash = u32::from('-');
    let mut result = Vec::new();
    if arg[1] == dash {
        let (prefix, separator, explicit) = match arg.iter().position(|&c| c == u32::from('=')) {
            Some(at) => (&arg[..at], Separator::Equals, Some(arg[at + 1..].to_vec())),
            None => (arg, Separator::Absent, None),
        };
        for &(name, action) in spec.options {
            if starts_with(&points(name), prefix) {
                result.push(OptionTuple {
                    action: Some(action),
                    option_string: points(name),
                    separator,
                    explicit: explicit.clone(),
                });
            }
        }
    } else {
        let (short_prefix, short_explicit) = arg.split_at(2);
        for &(name, action) in spec.options {
            let name_points = points(name);
            if name_points == short_prefix {
                result.push(OptionTuple {
                    action: Some(action),
                    option_string: name_points,
                    separator: Separator::Joined,
                    explicit: Some(short_explicit.to_vec()),
                });
            } else if starts_with(&name_points, arg) {
                result.push(OptionTuple {
                    action: Some(action),
                    option_string: name_points,
                    separator: Separator::Absent,
                    explicit: None,
                });
            }
        }
    }
    result
}

/// `_parse_optional`: `Ok(None)` means the argument is positional-like. An ambiguous prefix is a parser error.
fn parse_optional(spec: &Spec, arg: &[u32]) -> Result<Option<OptionTuple>, PyStr> {
    if arg.first() != Some(&u32::from('-')) {
        return Ok(None);
    }
    if let Some(action) = spec.lookup(arg) {
        return Ok(Some(OptionTuple {
            action: Some(action),
            option_string: arg.to_vec(),
            separator: Separator::Absent,
            explicit: None,
        }));
    }
    if arg.len() == 1 {
        return Ok(None);
    }
    if let Some(at) = arg.iter().position(|&c| c == u32::from('=')) {
        if let Some(action) = spec.lookup(&arg[..at]) {
            return Ok(Some(OptionTuple {
                action: Some(action),
                option_string: arg[..at].to_vec(),
                separator: Separator::Equals,
                explicit: Some(arg[at + 1..].to_vec()),
            }));
        }
    }
    let mut candidates = option_tuples(spec, arg);
    if candidates.len() > 1 {
        let names: Vec<String> = candidates
            .iter()
            .map(|tuple| {
                PyStr::from_code_points(tuple.option_string.clone())
                    .to_utf8()
                    .unwrap_or_default()
            })
            .collect();
        let joined = points(&names.join(", "));
        return Err(text(&[
            &points("ambiguous option: ")[..],
            arg,
            &points(" could match ")[..],
            &joined[..],
        ]));
    }
    if let Some(only) = candidates.pop() {
        return Ok(Some(only));
    }
    if is_negative_number(arg) || arg.contains(&u32::from(' ')) {
        return Ok(None);
    }
    Ok(Some(OptionTuple {
        action: None,
        option_string: arg.to_vec(),
        separator: Separator::Absent,
        explicit: None,
    }))
}

fn argument_error(spec: &Spec, action: usize, message: &str) -> PyStr {
    PyStr::new(&format!(
        "argument {}: {message}",
        spec.actions[action].name
    ))
}

/// What `consume_optional` decided for one option argument.
enum Consumed {
    Next(usize),
    Help,
    Error(PyStr),
}

/// Parses the arguments of the original `sweep_stop.py` (without the program name).
pub fn parse(args: &[PyStr]) -> Parsed {
    match parse_with(&SWEEP_STOP, args) {
        Outcome::Help => Parsed::Help,
        Outcome::Error(message) => Parsed::Error(message),
        Outcome::Run(matches) => Parsed::Run(LegacyArgs {
            state: matches.values[STATE].clone(),
            now_utc: matches.values[NOW_UTC].clone(),
            self_test: matches.flag(SELF_TEST),
        }),
    }
}

/// Parses `args` (without the program name) the way `ArgumentParser.parse_args` of the given parser does.
pub fn parse_with(spec: &Spec, args: &[PyStr]) -> Outcome {
    let args: Vec<&[u32]> = args.iter().map(PyStr::code_points).collect();
    let mut pattern: Vec<char> = Vec::with_capacity(args.len());
    let mut tuples: Vec<Option<OptionTuple>> = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        if equals(args[index], "--") {
            pattern.push('-');
            tuples.push(None);
            for _ in index + 1..args.len() {
                pattern.push('A');
                tuples.push(None);
            }
            break;
        }
        match parse_optional(spec, args[index]) {
            Err(message) => return Outcome::Error(message),
            Ok(None) => {
                pattern.push('A');
                tuples.push(None);
            }
            Ok(Some(tuple)) => {
                pattern.push('O');
                tuples.push(Some(tuple));
            }
        }
        index += 1;
    }

    let mut parsed = Matches::new(spec);
    let mut extras: Vec<PyStr> = Vec::new();
    let last_option = tuples.iter().rposition(Option::is_some);
    let mut start = 0;
    while last_option.is_some_and(|last| start <= last) {
        let next = (start..tuples.len())
            .find(|&i| tuples[i].is_some())
            .expect("an option remains");
        extras.extend(
            args[start..next]
                .iter()
                .map(|a| PyStr::from_code_points(a.to_vec())),
        );
        start = next;
        match consume_optional(
            spec,
            start,
            &args,
            &pattern,
            &mut tuples,
            &mut extras,
            &mut parsed,
        ) {
            Consumed::Next(stop) => start = stop,
            Consumed::Help => return Outcome::Help,
            Consumed::Error(message) => return Outcome::Error(message),
        }
    }
    extras.extend(
        args[start..]
            .iter()
            .map(|a| PyStr::from_code_points(a.to_vec())),
    );
    // argparse checks required actions before `parse_args` looks at the leftover arguments.
    let missing: Vec<&str> = spec
        .actions
        .iter()
        .enumerate()
        .filter(|(index, action)| action.required && !parsed.seen[*index])
        .map(|(_, action)| action.name)
        .collect();
    if !missing.is_empty() {
        return Outcome::Error(PyStr::new(&format!(
            "the following arguments are required: {}",
            missing.join(", ")
        )));
    }
    if extras.is_empty() {
        return Outcome::Run(parsed);
    }
    let mut message = PyStr::new("unrecognized arguments: ");
    for (position, extra) in extras.iter().enumerate() {
        if position > 0 {
            message.push_str(" ");
        }
        message.push_py(extra);
    }
    Outcome::Error(message)
}

/// `_get_values` for a value option: the `type=` conversion, then the `choices` check.
fn convert_value(definition: &ActionDef, text: &PyStr) -> Result<Option<ArgNumber>, PyStr> {
    let invalid = |kind: &str| {
        PyStr::new(&format!(
            "argument {}: invalid {kind} value: {}",
            definition.name,
            text.repr()
        ))
    };
    let number = match definition.conversion {
        Conversion::Text => None,
        Conversion::Float => Some(ArgNumber::Float(
            float_from_str(text).map_err(|_| invalid("float"))?,
        )),
        Conversion::Int => Some(ArgNumber::Int(
            int_from_str(text).map_err(|_| invalid("int"))?,
        )),
    };
    if !definition.choices.is_empty()
        && !definition.choices.iter().any(|choice| text.equals(choice))
    {
        let allowed: Vec<String> = definition
            .choices
            .iter()
            .map(|choice| PyStr::new(choice).repr())
            .collect();
        return Err(PyStr::new(&format!(
            "argument {}: invalid choice: {} (choose from {})",
            definition.name,
            text.repr(),
            allowed.join(", ")
        )));
    }
    Ok(number)
}

fn consume_optional(
    spec: &Spec,
    start: usize,
    args: &[&[u32]],
    pattern: &[char],
    tuples: &mut [Option<OptionTuple>],
    extras: &mut Vec<PyStr>,
    parsed: &mut Matches,
) -> Consumed {
    let tuple = tuples[start].take().expect("start indexes an option");
    let (mut action, mut option_string, mut separator, mut explicit) = (
        tuple.action,
        tuple.option_string,
        tuple.separator,
        tuple.explicit,
    );
    let mut queued: Vec<(usize, Vec<PyStr>)> = Vec::new();
    let stop;
    loop {
        let Some(current) = action else {
            extras.push(PyStr::from_code_points(args[start].to_vec()));
            return Consumed::Next(start + 1);
        };
        if let Some(value) = explicit.clone() {
            let takes = usize::from(spec.actions[current].kind == Kind::Value);
            if takes == 0 && option_string[1] != u32::from('-') && !value.is_empty() {
                if separator == Separator::Equals || value[0] == u32::from('-') {
                    return Consumed::Error(argument_error(
                        spec,
                        current,
                        &format!("ignored explicit argument {}", quoted(&value)),
                    ));
                }
                queued.push((current, Vec::new()));
                let dash = option_string[0];
                option_string = vec![dash, value[0]];
                if let Some(next_action) = spec.lookup(&option_string) {
                    action = Some(next_action);
                    let rest = value[1..].to_vec();
                    if rest.is_empty() {
                        separator = Separator::Absent;
                        explicit = None;
                    } else if rest[0] == u32::from('=') {
                        separator = Separator::Equals;
                        explicit = Some(rest[1..].to_vec());
                    } else {
                        separator = Separator::Joined;
                        explicit = Some(rest);
                    }
                } else {
                    extras.push(PyStr::from_code_points([&[dash][..], &value].concat()));
                    stop = start + 1;
                    break;
                }
            } else if takes == 1 {
                stop = start + 1;
                queued.push((current, vec![PyStr::from_code_points(value)]));
                break;
            } else {
                return Consumed::Error(argument_error(
                    spec,
                    current,
                    &format!("ignored explicit argument {}", quoted(&value)),
                ));
            }
        } else {
            let wanted = usize::from(spec.actions[current].kind == Kind::Value);
            if wanted == 1 && pattern.get(start + 1) != Some(&'A') {
                return Consumed::Error(argument_error(spec, current, "expected one argument"));
            }
            stop = start + 1 + wanted;
            let values = args[start + 1..stop]
                .iter()
                .map(|a| PyStr::from_code_points(a.to_vec()))
                .collect();
            queued.push((current, values));
            break;
        }
    }
    for (action, mut values) in queued {
        let definition = &spec.actions[action];
        match definition.kind {
            Kind::Help => return Consumed::Help,
            Kind::Value => {
                let value = values.pop();
                if let Some(text) = &value {
                    match convert_value(definition, text) {
                        Ok(number) => parsed.numbers[action] = number,
                        Err(message) => return Consumed::Error(message),
                    }
                }
                parsed.values[action] = value;
            }
            Kind::Flag => {}
        }
        parsed.seen[action] = true;
    }
    Consumed::Next(stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Parsed {
        parse(&args.iter().map(|a| PyStr::new(a)).collect::<Vec<_>>())
    }

    fn error_of(args: &[&str]) -> String {
        match parse_strs(args) {
            Parsed::Error(message) => message.to_utf8().unwrap(),
            Parsed::Help => "HELP".to_string(),
            Parsed::Run(_) => "RUN".to_string(),
        }
    }

    #[test]
    fn terminator_keeps_the_dash_dash_and_everything_after_it_as_extras() {
        assert_eq!(
            error_of(&["--", "--help"]),
            "unrecognized arguments: -- --help"
        );
        assert_eq!(
            error_of(&["--state", "-", "--now-utc", "x", "--", "--self-test"]),
            "unrecognized arguments: -- --self-test"
        );
    }

    #[test]
    fn explicit_argument_rules_for_flags() {
        assert_eq!(
            error_of(&["--help=unexpected"]),
            "argument -h/--help: ignored explicit argument 'unexpected'"
        );
        assert_eq!(
            error_of(&["--self-test=true"]),
            "argument --self-test: ignored explicit argument 'true'"
        );
        assert_eq!(error_of(&["-hh"]), "HELP");
        assert_eq!(error_of(&["-hbad"]), "HELP");
        assert_eq!(
            error_of(&["--s"]),
            "ambiguous option: --s could match --state, --self-test"
        );
    }

    #[test]
    fn values_and_negative_numbers() {
        assert!(matches!(
            parse_strs(&["--state", "-5", "--now-utc", "x"]),
            Parsed::Run(_)
        ));
        assert!(
            matches!(
                parse_strs(&["--state", "-\u{661}", "--now-utc", "x"]),
                Parsed::Run(_)
            ),
            "Arabic-Indic digits are digits"
        );
        assert_eq!(
            error_of(&["--state"]),
            "argument --state: expected one argument"
        );
        assert_eq!(
            error_of(&["--state", "--", "x"]),
            "argument --state: expected one argument"
        );
    }
}
