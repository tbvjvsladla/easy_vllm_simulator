//! The `argparse` surface of `evidence_publisher.py` (CPython 3.12 semantics for the shapes this parser uses):
//! a top-level parser (`-h/--help`, `--self-test`, subcommand `cmd`) and eight subparsers whose options all take
//! exactly one value. Long options may be abbreviated to a unique prefix and written `--opt=value`. Every parse error is
//! returned as the argparse message text (the caller wraps it in `CLI_USAGE_ERROR`); `-h` returns the help text.

use std::collections::BTreeMap;

pub struct Opt {
    pub flag: &'static str,
    pub required: bool,
    pub choices: Option<Vec<String>>,
}

pub struct Sub {
    pub name: &'static str,
    pub opts: Vec<Opt>,
}

pub enum Parsed {
    /// Top-level `-h`: help text for the root (None) or a subcommand.
    Help(Option<&'static str>),
    Ok { self_test: bool, cmd: Option<&'static str>, values: BTreeMap<&'static str, String> },
}

fn is_negative_number(arg: &str) -> bool {
    // argparse `_negative_number_matcher = re.compile(r'^-\d+$|^-\d*\.\d+$')`
    let rest = match arg.strip_prefix('-') {
        Some(r) => r,
        None => return false,
    };
    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if let Some((a, c)) = rest.split_once('.') {
        return a.chars().all(|x| x.is_ascii_digit()) && !c.is_empty() && c.chars().all(|x| x.is_ascii_digit());
    }
    false
}

enum Kind {
    /// a known option (index into the flag list) with an optional explicit `=value`
    Opt(usize, Option<String>),
    /// option-like string no action knows
    UnknownOpt,
    Positional,
}

/// `ArgumentParser._parse_optional` for long options + `-h`.
fn classify(arg: &str, flags: &[&'static str]) -> Result<Kind, String> {
    if !arg.starts_with('-') || arg == "-" {
        return Ok(Kind::Positional);
    }
    if let Some(i) = flags.iter().position(|f| *f == arg) {
        return Ok(Kind::Opt(i, None));
    }
    if arg.len() == 1 {
        return Ok(Kind::Positional);
    }
    if let Some((head, value)) = arg.split_once('=') {
        if let Some(i) = flags.iter().position(|f| *f == head) {
            return Ok(Kind::Opt(i, Some(value.to_string())));
        }
    }
    // abbreviations (allow_abbrev=True): only `--` options are prefix-matched
    if arg.starts_with("--") {
        let (head, explicit) = match arg.split_once('=') {
            Some((h, v)) => (h, Some(v.to_string())),
            None => (arg, None),
        };
        let matches: Vec<usize> = flags
            .iter()
            .enumerate()
            .filter(|(_, f)| f.starts_with("--") && f.starts_with(head))
            .map(|(i, _)| i)
            .collect();
        if matches.len() > 1 {
            let names: Vec<&str> = matches.iter().map(|i| flags[*i]).collect();
            return Err(format!("ambiguous option: {} could match {}", arg, names.join(", ")));
        }
        if matches.len() == 1 {
            return Ok(Kind::Opt(matches[0], explicit));
        }
    } else if !arg.starts_with("--") {
        // single-dash: `-hVALUE` style is not used by this parser; `-h` exact handled above
        let head = &arg[..2];
        if let Some(i) = flags.iter().position(|f| *f == head) {
            if head == "-h" {
                return Ok(Kind::Opt(i, Some(arg[2..].to_string())));
            }
        }
    }
    if is_negative_number(arg) {
        return Ok(Kind::Positional);
    }
    if arg.contains(' ') {
        return Ok(Kind::Positional);
    }
    Ok(Kind::UnknownOpt)
}

fn quote_choices(choices: &[String]) -> String {
    choices.iter().map(|c| crate::json::repr(c)).collect::<Vec<_>>().join(", ")
}

/// Splits on the first `--` exactly as 3.12 does: options after it are positionals (and the `--` itself is dropped
/// from the values handed to the consuming action).
fn kinds(args: &[String], flags: &[&'static str]) -> Result<Vec<Kind>, String> {
    let mut out = Vec::with_capacity(args.len());
    let mut after_dd = false;
    for a in args {
        if after_dd {
            out.push(Kind::Positional);
            continue;
        }
        if a == "--" {
            after_dd = true;
            out.push(Kind::Positional);
            continue;
        }
        out.push(classify(a, flags)?);
    }
    Ok(out)
}

pub fn parse(argv: &[String], subs: &[Sub]) -> Result<Parsed, String> {
    let top_flags: Vec<&'static str> = vec!["-h", "--help", "--self-test"];
    let ks = kinds(argv, &top_flags)?;
    let mut extras: Vec<String> = Vec::new();
    let mut self_test = false;
    let mut i = 0;
    while i < argv.len() {
        match &ks[i] {
            Kind::Opt(idx, explicit) => {
                match top_flags[*idx] {
                    "-h" | "--help" => {
                        if let Some(v) = explicit {
                            if !v.is_empty() || argv[i].contains('=') {
                                return Err(format!("argument -h/--help: ignored explicit argument {}", crate::json::repr(v)));
                            }
                        }
                        return Ok(Parsed::Help(None));
                    }
                    _ => {
                        if let Some(v) = explicit {
                            return Err(format!("argument --self-test: ignored explicit argument {}", crate::json::repr(v)));
                        }
                        self_test = true;
                    }
                }
                i += 1;
            }
            Kind::UnknownOpt => {
                extras.push(argv[i].clone());
                i += 1;
            }
            Kind::Positional => {
                if argv[i] == "--" && i + 1 == argv.len() {
                    // a trailing `--` alone: nothing positional remains
                    i += 1;
                    continue;
                }
                let start = if argv[i] == "--" { i + 1 } else { i };
                let name = &argv[start];
                let sub = match subs.iter().find(|s| s.name == name) {
                    Some(s) => s,
                    None => {
                        let names: Vec<String> = subs.iter().map(|s| s.name.to_string()).collect();
                        return Err(format!(
                            "argument cmd: invalid choice: {} (choose from {})",
                            crate::json::repr(name),
                            quote_choices(&names)
                        ));
                    }
                };
                let rest: Vec<String> = argv[start + 1..].to_vec();
                let (values, sub_extras) = match parse_sub(&rest, sub)? {
                    SubOut::Help => return Ok(Parsed::Help(Some(sub.name))),
                    SubOut::Values(v, e) => (v, e),
                };
                extras.extend(sub_extras);
                if !extras.is_empty() {
                    return Err(format!("unrecognized arguments: {}", extras.join(" ")));
                }
                return Ok(Parsed::Ok { self_test, cmd: Some(sub.name), values });
            }
        }
    }
    if !extras.is_empty() {
        return Err(format!("unrecognized arguments: {}", extras.join(" ")));
    }
    Ok(Parsed::Ok { self_test, cmd: None, values: BTreeMap::new() })
}

enum SubOut {
    Help,
    Values(BTreeMap<&'static str, String>, Vec<String>),
}

fn parse_sub(args: &[String], sub: &Sub) -> Result<SubOut, String> {
    let mut flags: Vec<&'static str> = vec!["-h", "--help"];
    flags.extend(sub.opts.iter().map(|o| o.flag));
    let ks = kinds(args, &flags)?;
    let mut values: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut extras = Vec::new();
    let mut seen_dd = false;
    let mut i = 0;
    while i < args.len() {
        match &ks[i] {
            Kind::Opt(idx, explicit) => {
                let flag = flags[*idx];
                if flag == "-h" || flag == "--help" {
                    if let Some(v) = explicit {
                        if !v.is_empty() || args[i].contains('=') {
                            return Err(format!("argument -h/--help: ignored explicit argument {}", crate::json::repr(v)));
                        }
                    }
                    return Ok(SubOut::Help);
                }
                let opt = &sub.opts[*idx - 2];
                let value = match explicit {
                    Some(v) => {
                        i += 1;
                        v.clone()
                    }
                    None => {
                        let next_ok = i + 1 < args.len() && matches!(ks[i + 1], Kind::Positional) && !(args[i + 1] == "--" && !seen_dd);
                        if !next_ok {
                            return Err(format!("argument {}: expected one argument", opt.flag));
                        }
                        i += 2;
                        args[i - 1].clone()
                    }
                };
                if let Some(choices) = &opt.choices {
                    if !choices.iter().any(|c| *c == value) {
                        return Err(format!(
                            "argument {}: invalid choice: {} (choose from {})",
                            opt.flag,
                            crate::json::repr(&value),
                            quote_choices(choices)
                        ));
                    }
                }
                values.insert(opt.flag, value);
            }
            Kind::UnknownOpt => {
                extras.push(args[i].clone());
                i += 1;
            }
            Kind::Positional => {
                if args[i] == "--" && !seen_dd {
                    seen_dd = true;
                } else {
                    extras.push(args[i].clone());
                }
                i += 1;
            }
        }
    }
    let missing: Vec<&str> = sub.opts.iter().filter(|o| o.required && !values.contains_key(o.flag)).map(|o| o.flag).collect();
    if !missing.is_empty() {
        return Err(format!("the following arguments are required: {}", missing.join(", ")));
    }
    Ok(SubOut::Values(values, extras))
}
