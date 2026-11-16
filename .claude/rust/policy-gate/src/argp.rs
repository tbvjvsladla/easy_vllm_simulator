//! The slice of CPython 3.12 `argparse` these two command lines use: long/short optionals (flag or one value,
//! `--opt=value`, unique-prefix abbreviation, ambiguity error, `choices`, `required`), `-h/--help`, an optional
//! subcommand positional (nargs=PARSER) and the "unrecognized arguments" collection. Error texts are argparse's.

use crate::pytext::repr;

#[derive(Clone)]
pub struct Opt {
    pub strings: &'static [&'static str],
    pub dest: &'static str,
    pub takes_value: bool,
    pub required: bool,
    pub choices: Option<&'static [&'static str]>,
    pub help: bool,
}

impl Opt {
    pub const fn help() -> Opt {
        Opt { strings: &["-h", "--help"], dest: "help", takes_value: false, required: false, choices: None, help: true }
    }
    pub const fn flag(strings: &'static [&'static str], dest: &'static str) -> Opt {
        Opt { strings, dest, takes_value: false, required: false, choices: None, help: false }
    }
    pub const fn value(strings: &'static [&'static str], dest: &'static str, required: bool) -> Opt {
        Opt { strings, dest, takes_value: true, required, choices: None, help: false }
    }
    pub const fn choice(strings: &'static [&'static str], dest: &'static str, required: bool, choices: &'static [&'static str]) -> Opt {
        Opt { strings, dest, takes_value: true, required, choices: Some(choices), help: false }
    }
    fn name(&self) -> String {
        self.strings.join("/")
    }
}

/// Why parsing stopped: help requested (print it, exit 0) or a usage error (`message`) raised by `prog`.
#[derive(Debug)]
pub enum Stop {
    Help { prog: String },
    Error { prog: String, message: String },
}

#[derive(Debug, Default, Clone)]
pub struct Values {
    pub set: Vec<(&'static str, Option<String>)>,
}

impl Values {
    pub fn get(&self, dest: &str) -> Option<&str> {
        self.set.iter().rev().find(|(d, _)| *d == dest).and_then(|(_, v)| v.as_deref())
    }
    pub fn flag(&self, dest: &str) -> bool {
        self.set.iter().any(|(d, _)| *d == dest)
    }
}

enum Kind {
    DoubleDash,
    Arg,
    Opt(usize, Option<String>),
    Unknown,
}

fn is_negative_number(s: &str) -> bool {
    // '^-\d+$|^-\d*\.\d+$'
    let body = match s.strip_prefix('-') {
        Some(b) => b,
        None => return false,
    };
    let digit = |c: char| agent_runtime_support::pystr::is_decimal_digit(c as u32);
    if !body.is_empty() && body.chars().all(digit) {
        return true;
    }
    if let Some((a, b)) = body.split_once('.') {
        return a.chars().all(digit) && !b.is_empty() && b.chars().all(digit);
    }
    false
}

fn classify(arg: &str, opts: &[Opt]) -> Result<Kind, String> {
    if arg == "--" {
        return Ok(Kind::DoubleDash);
    }
    if !arg.starts_with('-') || arg == "-" {
        return Ok(Kind::Arg);
    }
    let all: Vec<(usize, &str)> =
        opts.iter().enumerate().flat_map(|(i, o)| o.strings.iter().map(move |s| (i, *s))).collect();
    if let Some((i, _)) = all.iter().find(|(_, s)| *s == arg) {
        return Ok(Kind::Opt(*i, None));
    }
    if let Some((prefix, explicit)) = arg.split_once('=') {
        if let Some((i, _)) = all.iter().find(|(_, s)| *s == prefix) {
            return Ok(Kind::Opt(*i, Some(explicit.to_string())));
        }
    }
    // _get_option_tuples
    let mut matches: Vec<(usize, &str, Option<String>)> = Vec::new();
    if arg.starts_with("--") {
        let (prefix, explicit) = match arg.split_once('=') {
            Some((p, e)) => (p, Some(e.to_string())),
            None => (arg, None),
        };
        for (i, s) in &all {
            if s.starts_with(prefix) {
                matches.push((*i, s, explicit.clone()));
            }
        }
    } else {
        let short_prefix: String = arg.chars().take(2).collect();
        let short_explicit: String = arg.chars().skip(2).collect();
        for (i, s) in &all {
            if *s == short_prefix {
                matches.push((*i, s, Some(short_explicit.clone())));
            } else if s.starts_with(arg) {
                matches.push((*i, s, None));
            }
        }
    }
    if matches.len() > 1 {
        let names: Vec<&str> = matches.iter().map(|(_, s, _)| *s).collect();
        return Err(format!("ambiguous option: {} could match {}", arg, names.join(", ")));
    }
    if let Some((i, _, explicit)) = matches.into_iter().next() {
        return Ok(Kind::Opt(i, explicit));
    }
    if is_negative_number(arg) {
        return Ok(Kind::Arg);
    }
    if arg.contains(' ') {
        return Ok(Kind::Arg);
    }
    Ok(Kind::Unknown)
}

/// Result of one parser level: values, unrecognized strings, and (top level only) the subcommand and its strings.
pub struct Level {
    pub values: Values,
    pub extras: Vec<String>,
    pub positional: Option<(String, Vec<String>)>,
}

/// Parse one level. `subcommand = Some(choices)` declares one PARSER positional named `cmd`.
pub fn parse_level(prog: &str, opts: &[Opt], args: &[String], subcommand: Option<&[&str]>) -> Result<Level, Stop> {
    let err = |m: String| Stop::Error { prog: prog.to_string(), message: m };
    let mut values = Values::default();
    let mut extras = Vec::new();
    let mut i = 0;
    let mut after_dd = false;
    let mut positional = None;
    while i < args.len() {
        let arg = &args[i];
        let kind = if after_dd { Kind::Arg } else { classify(arg, opts).map_err(err)? };
        match kind {
            Kind::DoubleDash => {
                if let Some(choices) = subcommand {
                    // nargs=PARSER keeps '--' as its first value
                    let rest = args[i + 1..].to_vec();
                    if !choices.contains(&arg.as_str()) {
                        return Err(err(invalid_choice("cmd", arg, choices)));
                    }
                    positional = Some((arg.clone(), rest));
                    break;
                }
                after_dd = true;
                extras.push(arg.clone());
                i += 1;
            }
            Kind::Arg => {
                if let Some(choices) = subcommand {
                    if !choices.contains(&arg.as_str()) {
                        return Err(err(invalid_choice("cmd", arg, choices)));
                    }
                    positional = Some((arg.clone(), args[i + 1..].to_vec()));
                    break;
                }
                extras.push(arg.clone());
                i += 1;
            }
            Kind::Unknown => {
                extras.push(arg.clone());
                i += 1;
            }
            Kind::Opt(idx, explicit) => {
                let opt = &opts[idx];
                if !opt.takes_value {
                    if let Some(e) = explicit {
                        return Err(err(format!("argument {}: ignored explicit argument {}", opt.name(), repr(&e))));
                    }
                    if opt.help {
                        return Err(Stop::Help { prog: prog.to_string() });
                    }
                    values.set.push((opt.dest, None));
                    i += 1;
                    continue;
                }
                let value = match explicit {
                    Some(e) => {
                        i += 1;
                        e
                    }
                    None => {
                        let next = args.get(i + 1);
                        let ok = match next {
                            Some(n) => matches!(classify(n, opts), Ok(Kind::Arg)),
                            None => false,
                        };
                        if !ok {
                            return Err(err(format!("argument {}: expected one argument", opt.name())));
                        }
                        i += 2;
                        next.unwrap().clone()
                    }
                };
                if let Some(choices) = opt.choices {
                    if !choices.contains(&value.as_str()) {
                        return Err(err(invalid_choice(&opt.name(), &value, choices)));
                    }
                }
                values.set.push((opt.dest, Some(value)));
            }
        }
    }
    let mut missing: Vec<String> =
        opts.iter().filter(|o| o.required && !values.flag(o.dest)).map(|o| o.name()).collect();
    if subcommand.is_some() && positional.is_none() {
        missing.push("cmd".to_string());
    }
    if !missing.is_empty() {
        return Err(err(format!("the following arguments are required: {}", missing.join(", "))));
    }
    Ok(Level { values, extras, positional })
}

fn invalid_choice(name: &str, value: &str, choices: &[&str]) -> String {
    let listed: Vec<String> = choices.iter().map(|c| repr(c)).collect();
    format!("argument {}: invalid choice: {} (choose from {})", name, repr(value), listed.join(", "))
}

/// Python's `os.path.basename(sys.argv[0])`.
pub fn prog_name(argv0: &str, fallback: &str) -> String {
    let base = argv0.rsplit('/').next().unwrap_or(argv0);
    if base.is_empty() {
        fallback.to_string()
    } else {
        base.to_string()
    }
}
