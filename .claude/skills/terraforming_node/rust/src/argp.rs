//! The command lines of the ported programs, parsed the way CPython 3.12.3 `argparse` parses them.
//!
//! Scope: parsers made of `-h/--help`, `store` options (text or `type=int`, optional `choices`, `required`), `store_true` flags,
//! mutually exclusive groups, and at most one level of sub-commands (`add_subparsers(dest=...)`, not required). Long-option
//! abbreviations are accepted (`allow_abbrev=True`), `--opt=value` is accepted, an option value may not look like an option.
//! Messages are argparse's own (`expected one argument`, `invalid choice`, `invalid int value`, `not allowed with argument`,
//! `ambiguous option`, `the following arguments are required`, `unrecognized arguments`, `ignored explicit argument`).
//!
//! Usage/help text is the original program's text captured once at argparse's default width (stdout not a terminal,
//! `COLUMNS` unset = 80 columns) and kept in `help/`. A different terminal width re-wraps argparse's text; that layout is
//! not reproduced (documented residual).

use agent_runtime_support::pystr::PyStr;
use std::io::Write;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Help,
    Flag,
    Store,
    /// `action="append"`: every occurrence kept in order.
    Append,
}

#[derive(Clone, Copy, Debug)]
pub struct Arg {
    pub flags: &'static [&'static str],
    pub act: Act,
    pub int: bool,
    pub float: bool,
    pub required: bool,
    pub choices: &'static [&'static str],
    /// Mutually exclusive group number (0 = none).
    pub group: u8,
}

pub const HELP: Arg = Arg {
    flags: &["-h", "--help"],
    act: Act::Help,
    int: false,
    float: false,
    required: false,
    choices: &[],
    group: 0,
};

pub const fn flag(flags: &'static [&'static str]) -> Arg {
    Arg {
        flags,
        act: Act::Flag,
        int: false,
        float: false,
        required: false,
        choices: &[],
        group: 0,
    }
}
pub const fn store(flags: &'static [&'static str]) -> Arg {
    Arg {
        flags,
        act: Act::Store,
        int: false,
        float: false,
        required: false,
        choices: &[],
        group: 0,
    }
}
impl Arg {
    pub const fn int(mut self) -> Self {
        self.int = true;
        self
    }
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub const fn choices(mut self, c: &'static [&'static str]) -> Self {
        self.choices = c;
        self
    }
    pub const fn group(mut self, g: u8) -> Self {
        self.group = g;
        self
    }
    /// `type=float`.
    pub const fn float(mut self) -> Self {
        self.float = true;
        self
    }
    pub const fn append(mut self) -> Self {
        self.act = Act::Append;
        self
    }
    fn name(&self) -> String {
        self.flags.join("/")
    }
}

pub struct Spec {
    pub prog: &'static str,
    pub usage: &'static str,
    pub help: &'static str,
    pub args: &'static [Arg],
    /// Sub-commands: (dest, [(name, spec)]).
    pub subs: Option<(&'static str, &'static [(&'static str, &'static Spec)])>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Text(String),
    Int(i64),
    Float(f64),
    Many(Vec<String>),
}

#[derive(Debug, Default)]
pub struct Parsed {
    pub vals: Vec<Option<Val>>,
    pub flags: Vec<bool>,
    pub sub: Option<(String, Box<Parsed>)>,
}

impl Parsed {
    fn new(n: usize) -> Self {
        Parsed {
            vals: vec![None; n],
            flags: vec![false; n],
            sub: None,
        }
    }
    pub fn flag(&self, i: usize) -> bool {
        self.flags[i]
    }
    pub fn text(&self, i: usize) -> Option<&str> {
        match &self.vals[i] {
            Some(Val::Text(s)) => Some(s),
            _ => None,
        }
    }
    pub fn int(&self, i: usize) -> Option<i64> {
        match &self.vals[i] {
            Some(Val::Int(n)) => Some(*n),
            _ => None,
        }
    }
    pub fn float(&self, i: usize) -> Option<f64> {
        match &self.vals[i] {
            Some(Val::Float(x)) => Some(*x),
            _ => None,
        }
    }
    pub fn many(&self, i: usize) -> &[String] {
        match &self.vals[i] {
            Some(Val::Many(v)) => v,
            _ => &[],
        }
    }
}

/// How parsing ended without a namespace: `Exit(code)` after printing help (0) or an error (2).
pub enum Outcome {
    Ok(Parsed),
    Exit(i32),
}

pub fn repr(s: &str) -> String {
    PyStr::new(s).repr()
}

/// `parser.error(message)`: usage + `PROG: error: MESSAGE` on standard error, status 2.
pub fn error(spec: &Spec, message: &str) -> i32 {
    let mut e = std::io::stderr().lock();
    let _ = writeln!(e, "{}{}: error: {}", spec.usage, spec.prog, message);
    2
}

pub fn print_usage_stderr(spec: &Spec) {
    let _ = std::io::stderr().lock().write_all(spec.usage.as_bytes());
}

pub fn print_help_stdout(spec: &Spec) {
    let mut o = std::io::stdout().lock();
    let _ = o.write_all(spec.help.as_bytes());
    let _ = o.flush();
}

/// Python `int(text)` for the decimal texts argparse hands to `type=int` (sign, surrounding whitespace, single `_`
/// between digits). Values outside i64 are refused like other invalid texts (documented residual).
pub fn py_int(text: &str) -> Option<i64> {
    let t = text.trim_matches(|c: char| c.is_whitespace());
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if body.is_empty() || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
        return None;
    }
    let mut n: i64 = 0;
    for c in body.chars() {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(10)? as i64;
        n = n.checked_mul(10)?.checked_add(d)?;
    }
    Some(if neg { -n } else { n })
}

fn is_negative_number(s: &str) -> bool {
    // argparse `_negative_number_matcher`: '^-\d+$|^-\d*\.\d+$'
    let b = &s[1..];
    if !s.starts_with('-') || b.is_empty() {
        return false;
    }
    let digits = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit());
    if digits(b) {
        return true;
    }
    if let Some((l, r)) = b.split_once('.') {
        return (l.is_empty() || digits(l)) && digits(r);
    }
    false
}

enum Classified {
    Positional,
    /// (arg index or None for unknown, option string, explicit value)
    Opt(Option<usize>, String, Option<String>),
}

fn classify(spec: &Spec, s: &str) -> Result<Classified, String> {
    if s.is_empty() || !s.starts_with('-') {
        return Ok(Classified::Positional);
    }
    for (i, a) in spec.args.iter().enumerate() {
        if a.flags.contains(&s) {
            return Ok(Classified::Opt(Some(i), s.to_string(), None));
        }
    }
    if s.len() == 1 {
        return Ok(Classified::Positional);
    }
    if let Some((o, v)) = s.split_once('=') {
        for (i, a) in spec.args.iter().enumerate() {
            if a.flags.contains(&o) {
                return Ok(Classified::Opt(Some(i), o.to_string(), Some(v.to_string())));
            }
        }
    }
    // `_get_option_tuples`
    let mut matches: Vec<(usize, &str, Option<String>)> = vec![];
    if s.starts_with("--") {
        let (prefix, explicit) = match s.split_once('=') {
            Some((p, v)) => (p, Some(v.to_string())),
            None => (s, None),
        };
        for (i, a) in spec.args.iter().enumerate() {
            for f in a.flags {
                if f.starts_with(prefix) {
                    matches.push((i, f, explicit.clone()));
                }
            }
        }
    } else {
        let short = if s.chars().count() >= 2 {
            s.chars().take(2).collect::<String>()
        } else {
            s.to_string()
        };
        let short_explicit: String = s.chars().skip(2).collect();
        for (i, a) in spec.args.iter().enumerate() {
            for f in a.flags {
                if *f == short {
                    matches.push((i, f, Some(short_explicit.clone())));
                } else if f.starts_with(s) {
                    matches.push((i, f, None));
                }
            }
        }
    }
    if matches.len() > 1 {
        let names: Vec<&str> = matches.iter().map(|m| m.1).collect();
        return Err(format!(
            "ambiguous option: {} could match {}",
            s,
            names.join(", ")
        ));
    }
    if let Some((i, f, e)) = matches.pop() {
        return Ok(Classified::Opt(Some(i), f.to_string(), e));
    }
    if is_negative_number(s) {
        return Ok(Classified::Positional);
    }
    if s.contains(' ') {
        return Ok(Classified::Positional);
    }
    Ok(Classified::Opt(None, s.to_string(), None))
}

/// Parses like `parse_known_args`; `Err(message)` is an argparse error of THIS parser; extras are returned.
#[allow(clippy::type_complexity)]
fn parse_known(
    spec: &'static Spec,
    args: &[String],
) -> Result<Result<(Parsed, Vec<String>), i32>, (String, &'static Spec)> {
    let me = spec;
    let n = spec.args.len();
    let mut p = Parsed::new(n);
    let mut extras: Vec<String> = vec![];
    let mut seen = vec![false; n];
    let mut non_default = vec![false; n];
    let mut i = 0;
    let mut dashdash = false;
    while i < args.len() {
        let s = &args[i];
        if dashdash {
            extras.push(s.clone());
            i += 1;
            continue;
        }
        if s == "--" {
            dashdash = true;
            extras.push(s.clone());
            i += 1;
            continue;
        }
        match classify(spec, s).map_err(|m| (m, me))? {
            Classified::Positional => {
                if let Some((dest, subs)) = spec.subs {
                    let name = s.as_str();
                    let Some((_, sub)) = subs.iter().find(|(nm, _)| *nm == name) else {
                        let choices: Vec<String> = subs.iter().map(|(nm, _)| repr(nm)).collect();
                        return Err((
                            format!(
                                "argument {}: invalid choice: {} (choose from {})",
                                dest,
                                repr(name),
                                choices.join(", ")
                            ),
                            me,
                        ));
                    };
                    match parse_known(sub, &args[i + 1..])? {
                        Ok((sp, sx)) => {
                            p.sub = Some((name.to_string(), Box::new(sp)));
                            extras.extend(sx);
                        }
                        Err(code) => return Ok(Err(code)),
                    }
                    i = args.len();
                    continue;
                }
                extras.push(s.clone());
                i += 1;
            }
            Classified::Opt(None, _, _) => {
                extras.push(s.clone());
                i += 1;
            }
            Classified::Opt(Some(ai), _opt, explicit) => {
                let a = &spec.args[ai];
                let value: Option<String>;
                match a.act {
                    Act::Help | Act::Flag => {
                        if let Some(e) = explicit {
                            if !e.is_empty() || _opt.starts_with("--") {
                                return Err((
                                    format!(
                                        "argument {}: ignored explicit argument {}",
                                        a.name(),
                                        repr(&e)
                                    ),
                                    me,
                                ));
                            }
                        }
                        value = None;
                        i += 1;
                    }
                    Act::Store | Act::Append => {
                        if let Some(e) = explicit {
                            value = Some(e);
                            i += 1;
                        } else {
                            let next = args.get(i + 1);
                            let ok = match next {
                                None => false,
                                Some(nx) if nx == "--" => false,
                                Some(nx) => {
                                    matches!(classify(spec, nx), Ok(Classified::Positional))
                                }
                            };
                            if !ok {
                                return Err((
                                    format!("argument {}: expected one argument", a.name()),
                                    me,
                                ));
                            }
                            value = Some(next.unwrap().clone());
                            i += 2;
                        }
                    }
                }
                // _get_values: type conversion then choices
                let mut converted: Option<Val> = None;
                if let Some(v) = &value {
                    if a.int {
                        match py_int(v) {
                            Some(x) => converted = Some(Val::Int(x)),
                            None => {
                                return Err((
                                    format!(
                                        "argument {}: invalid int value: {}",
                                        a.name(),
                                        repr(v)
                                    ),
                                    me,
                                ))
                            }
                        }
                    } else if a.float {
                        match agent_runtime_support::pynumber::float_from_str(&PyStr::new(v)) {
                            Ok(x) => converted = Some(Val::Float(x)),
                            Err(_) => {
                                return Err((
                                    format!(
                                        "argument {}: invalid float value: {}",
                                        a.name(),
                                        repr(v)
                                    ),
                                    me,
                                ))
                            }
                        }
                    } else {
                        converted = Some(Val::Text(v.clone()));
                    }
                    if !a.choices.is_empty() && !a.choices.contains(&v.as_str()) {
                        let c: Vec<String> = a.choices.iter().map(|x| repr(x)).collect();
                        return Err((
                            format!(
                                "argument {}: invalid choice: {} (choose from {})",
                                a.name(),
                                repr(v),
                                c.join(", ")
                            ),
                            me,
                        ));
                    }
                }
                seen[ai] = true;
                if a.group != 0 {
                    non_default[ai] = true;
                    for (j, b) in spec.args.iter().enumerate() {
                        if j != ai && b.group == a.group && non_default[j] {
                            return Err((
                                format!(
                                    "argument {}: not allowed with argument {}",
                                    a.name(),
                                    b.name()
                                ),
                                me,
                            ));
                        }
                    }
                }
                match a.act {
                    Act::Help => {
                        print_help_stdout(spec);
                        return Ok(Err(0));
                    }
                    Act::Flag => p.flags[ai] = true,
                    Act::Store => p.vals[ai] = converted,
                    Act::Append => {
                        let t = value.unwrap();
                        match &mut p.vals[ai] {
                            Some(Val::Many(v)) => v.push(t),
                            _ => p.vals[ai] = Some(Val::Many(vec![t])),
                        }
                    }
                }
            }
        }
    }
    let missing: Vec<String> = spec
        .args
        .iter()
        .enumerate()
        .filter(|(j, a)| a.required && !seen[*j])
        .map(|(_, a)| a.name())
        .collect();
    if !missing.is_empty() {
        return Err((
            format!(
                "the following arguments are required: {}",
                missing.join(", ")
            ),
            me,
        ));
    }
    Ok(Ok((p, extras)))
}

/// `parser.parse_args(argv)`.
pub fn parse(spec: &'static Spec, args: &[String]) -> Outcome {
    match parse_known(spec, args) {
        Err((m, which)) => Outcome::Exit(error(which, &m)),
        Ok(Err(code)) => Outcome::Exit(code),
        Ok(Ok((p, extras))) => {
            if !extras.is_empty() {
                return Outcome::Exit(error(
                    spec,
                    &format!("unrecognized arguments: {}", extras.join(" ")),
                ));
            }
            Outcome::Ok(p)
        }
    }
}

/// The process arguments after the program name (lossy UTF-8; argv bytes outside UTF-8 are a documented residual).
pub fn argv() -> Vec<String> {
    std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}
