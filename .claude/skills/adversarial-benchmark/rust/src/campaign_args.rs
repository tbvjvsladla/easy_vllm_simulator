//! The whole `campaign_init.py` command line, parsed the way CPython 3.12.3 `argparse` parses it.
//!
//! The parser of the original (`main`'s `ArgumentParser`, 109 actions) has only optionals: `-h/--help`, `store_true` flags,
//! `store` options (five with `choices`), `append` options and one `nargs="?"` option with `const="*"` (`--lifecycle`). Long-option
//! abbreviations are accepted (`allow_abbrev=True`), `--opt=value` is accepted, an option value may not look like an option, and
//! a word no option consumes is an unrecognized argument. Messages are argparse's own (`expected one argument`, `invalid choice`,
//! `ambiguous option`, `ignored explicit argument`, `unrecognized arguments`), printed after the usage block with status 2.
//!
//! The usage block is the first paragraph of the help text captured at argparse's default width (stdout not a terminal, COLUMNS
//! unset = 80 columns, `campaign_init_help::HELP`); another terminal width re-wraps argparse's text and that layout is not
//! reproduced (documented residual, the same one as the other ported parsers).

use crate::campaign_write::Args;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum K {
    Help,
    Flag,
    Value,
    Append,
    /// `nargs="?"`, `const="*"`.
    Optional,
}

/// Every option string of the original parser in `add_argument` order (the order argparse lists ambiguous matches in).
pub type Table = [(&'static str, K, &'static [&'static str])];

pub const ACTIONS: [(&str, K, &[&str]); 110] = [
    ("-h", K::Help, &[]),
    ("--help", K::Help, &[]),
    ("--active", K::Flag, &[]),
    ("--derive", K::Value, &[]),
    ("--cell", K::Value, &[]),
    ("--sweep", K::Value, &[]),
    ("--context", K::Value, &[]),
    ("--campaign-id", K::Value, &[]),
    ("--verify-purge-gate", K::Value, &[]),
    ("--close", K::Value, &[]),
    ("--residue-scan", K::Flag, &[]),
    ("--init", K::Value, &[]),
    ("--plan-ref", K::Value, &[]),
    ("--purge-previous", K::Append, &[]),
    ("--from-slice", K::Value, &[]),
    ("--ground", K::Flag, &[]),
    ("--terms", K::Value, &[]),
    ("--no-warm-start", K::Flag, &[]),
    ("--grounding-check", K::Flag, &[]),
    ("--lockset-precheck", K::Flag, &[]),
    ("--escalation-summary", K::Flag, &[]),
    ("--warm-start-library", K::Flag, &[]),
    ("--backfill-from-docs", K::Flag, &[]),
    ("--assigned-cells", K::Value, &[]),
    ("--emit-slice", K::Value, &[]),
    ("--out", K::Value, &[]),
    ("--import-sub", K::Value, &[]),
    ("--write-brief", K::Flag, &[]),
    ("--apply", K::Flag, &[]),
    ("--selftest", K::Flag, &[]),
    ("--suspend-current", K::Flag, &[]),
    ("--derived-from", K::Value, &[]),
    ("--preempt", K::Flag, &[]),
    ("--release", K::Append, &[]),
    ("--dispose", K::Append, &[]),
    ("--target-topology", K::Value, &["single", "multi"]),
    ("--reason", K::Value, &[]),
    ("--suspend", K::Value, &[]),
    ("--resume-campaign", K::Value, &[]),
    ("--accept-drift", K::Flag, &[]),
    ("--abandon", K::Value, &[]),
    ("--summary-testlog", K::Value, &[]),
    ("--answer-query", K::Value, &[]),
    ("--query-decision", K::Value, &["keep"]),
    ("--sub-echo", K::Value, &[]),
    ("--control", K::Value, &["suspend", "resume", "close", "abandon", "reslice"]),
    ("--sub-unconverged-accept", K::Value, &[]),
    ("--lifecycle", K::Optional, &[]),
    ("--phase-set", K::Value, &[]),
    ("--node", K::Value, &[]),
    ("--state", K::Value, &[]),
    ("--proof-predicate", K::Value, &[]),
    ("--proof-ok", K::Flag, &[]),
    ("--proof-source", K::Value, &[]),
    ("--started-utc", K::Value, &[]),
    ("--ended-utc", K::Value, &[]),
    ("--cell-set", K::Value, &[]),
    ("--cell-materialize", K::Value, &[]),
    ("--set", K::Append, &[]),
    ("--outcome", K::Value, &[]),
    ("--version", K::Value, &[]),
    ("--model", K::Value, &[]),
    ("--measurement-decode-tps", K::Value, &[]),
    ("--measurement-source", K::Value, &[]),
    ("--sweep-state", K::Value, &[]),
    ("--escalation-add", K::Flag, &[]),
    ("--symptom", K::Append, &[]),
    ("--events", K::Append, &[]),
    ("--window-start-utc", K::Value, &[]),
    ("--window-end-utc", K::Value, &[]),
    ("--image-digest", K::Value, &[]),
    ("--image-tag", K::Value, &[]),
    ("--authored-by", K::Value, &[]),
    ("--void-reason", K::Value, &[]),
    ("--void-reason-source", K::Value, &[]),
    ("--lite-raw", K::Value, &[]),
    ("--reentry-decide", K::Value, &[]),
    ("--decision", K::Value, &["approve", "decline"]),
    ("--route", K::Value, &["explorer", "upstream"]),
    ("--axis-citation", K::Value, &[]),
    ("--next-intent", K::Value, &[]),
    ("--evidence-add", K::Flag, &[]),
    ("--kind", K::Value, &[]),
    ("--path", K::Value, &[]),
    ("--unfreeze", K::Flag, &[]),
    ("--evidence-untag", K::Flag, &[]),
    ("--untag-reason", K::Value, &[]),
    ("--evidence-relocate", K::Flag, &[]),
    ("--to", K::Value, &[]),
    ("--relocate-reason", K::Value, &[]),
    ("--correct-reference", K::Flag, &[]),
    ("--evidence-prune-stubs", K::Flag, &[]),
    ("--freeze-evidence", K::Flag, &[]),
    ("--assign-set", K::Value, &[]),
    ("--assign-cells", K::Value, &[]),
    ("--assign-reason", K::Value, &[]),
    ("--assign-evidence", K::Value, &[]),
    ("--assign-approved-by", K::Value, &[]),
    ("--slice-update", K::Value, &[]),
    ("--revise", K::Flag, &[]),
    ("--revise-set", K::Append, &[]),
    ("--revise-reason", K::Value, &[]),
    ("--revise-evidence", K::Value, &[]),
    ("--revise-approved-by", K::Value, &[]),
    ("--hint-approve", K::Flag, &[]),
    ("--cells", K::Value, &[]),
    ("--approved-by", K::Value, &[]),
    ("--source", K::Value, &[]),
    ("--utc", K::Value, &[]),
    ("--resume-brief", K::Flag, &[]),
];

pub const PROG: &str = "campaign_init.sh";

/// `parser.format_usage()`: the first paragraph of the captured help.
pub fn usage() -> &'static str {
    let help = crate::campaign_init_help::HELP;
    match help.find("\n\n") {
        Some(at) => &help[..=at],
        None => help,
    }
}

/// How the command line ended.
pub enum Parsed {
    Args(Args),
    /// `-h`/`--help` was reached: print the help, status 0.
    Help,
    /// `parser.error(message)`: usage + `campaign_init.py: error: MESSAGE`, status 2.
    Error(String),
}

fn repr(text: &str) -> String {
    crate::pystr::PyStr::new(text).repr()
}

/// `_negative_number_matcher` = `^-\d+$|^-\d*\.\d+$`.
fn is_negative_number(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('-') else { return false };
    let digits = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    if !rest.is_empty() && digits(rest) {
        return true;
    }
    match rest.split_once('.') {
        Some((a, b)) => digits(a) && !b.is_empty() && digits(b),
        None => false,
    }
}

enum Class {
    /// An argument (`A`).
    Arg,
    /// An option: (action index or `None` for an unknown option, explicit argument).
    Opt(Option<usize>, Option<String>),
}

/// `_parse_optional` (3.12.3).
fn classify(table: &Table, s: &str) -> Result<Class, String> {
    if s.is_empty() || !s.starts_with('-') {
        return Ok(Class::Arg);
    }
    if let Some(i) = table.iter().position(|a| a.0 == s) {
        return Ok(Class::Opt(Some(i), None));
    }
    if s.chars().count() == 1 {
        return Ok(Class::Arg);
    }
    if let Some((o, v)) = s.split_once('=') {
        if let Some(i) = table.iter().position(|a| a.0 == o) {
            return Ok(Class::Opt(Some(i), Some(v.to_string())));
        }
    }
    let mut matches: Vec<(usize, Option<String>)> = Vec::new();
    if s.starts_with("--") {
        let (prefix, explicit) = match s.split_once('=') {
            Some((p, v)) => (p, Some(v.to_string())),
            None => (s, None),
        };
        for (i, a) in table.iter().enumerate() {
            if a.0.starts_with(prefix) {
                matches.push((i, explicit.clone()));
            }
        }
    } else {
        let short: String = s.chars().take(2).collect();
        let short_explicit: String = s.chars().skip(2).collect();
        for (i, a) in table.iter().enumerate() {
            if a.0 == short {
                matches.push((i, Some(short_explicit.clone())));
            } else if a.0.starts_with(s) {
                matches.push((i, None));
            }
        }
    }
    if matches.len() > 1 {
        let names: Vec<&str> = matches.iter().map(|m| table[m.0].0).collect();
        return Err(format!("ambiguous option: {} could match {}", s, names.join(", ")));
    }
    if let Some((i, e)) = matches.pop() {
        return Ok(Class::Opt(Some(i), e));
    }
    if is_negative_number(s) || s.contains(' ') {
        return Ok(Class::Arg);
    }
    Ok(Class::Opt(None, None))
}

/// `argument NAME`: argparse names an optional by its option strings joined with `/`.
fn name(table: &Table, i: usize) -> String {
    match table[i].1 {
        K::Help => "-h/--help".to_string(),
        _ => table[i].0.to_string(),
    }
}

/// `parser.parse_args(argv)` of the original.
pub fn parse(argv: &[String]) -> Parsed {
    match parse_table(&ACTIONS, argv) {
        Ok(Some(values)) => Parsed::Args(Args::from_values(values)),
        Ok(None) => Parsed::Help,
        Err(message) => Parsed::Error(message),
    }
}

/// The namespace of [`parse_table`]: option string -> values.
pub type Namespace = BTreeMap<&'static str, Vec<String>>;

/// `parse_args` of a parser made only of the optionals in `table` (each action one option string, `-h`/`--help` as two rows):
/// `Ok(Some(namespace))` (a flag holds one empty value, an option given twice keeps the last, an append option every value),
/// `Ok(None)` when the help action was reached, `Err(message)` for `parser.error(message)`.
pub fn parse_table(table: &'static Table, argv: &[String]) -> Result<Option<Namespace>, String> {
    let mut values: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut extras: Vec<String> = Vec::new();
    let mut dashdash = false;
    let mut at = 0;
    while at < argv.len() {
        let word = &argv[at];
        if dashdash {
            extras.push(word.clone());
            at += 1;
            continue;
        }
        if word == "--" {
            dashdash = true;
            extras.push(word.clone());
            at += 1;
            continue;
        }
        let class = classify(table, word)?;
        let (index, explicit) = match class {
            Class::Arg | Class::Opt(None, _) => {
                extras.push(word.clone());
                at += 1;
                continue;
            }
            Class::Opt(Some(i), e) => (i, e),
        };
        let (_, kind, choices) = table[index];
        let next_is_arg = |at: usize| match argv.get(at + 1) {
            None => false,
            Some(nx) if nx == "--" => false,
            Some(nx) => matches!(classify(table, nx), Ok(Class::Arg)),
        };
        let value: Option<String> = match kind {
            // `-hX` (a single-dash help with attached characters): CPython 3.12.3 runs the help (measured, TC-CI-short-explicit).
            K::Help if !word.starts_with("--") => return Ok(None),
            K::Help | K::Flag => {
                if let Some(e) = explicit {
                    if !e.is_empty() || word.starts_with("--") {
                        return Err(format!("argument {}: ignored explicit argument {}", name(table, index), repr(&e)));
                    }
                }
                at += 1;
                None
            }
            K::Value | K::Append => {
                if let Some(e) = explicit {
                    at += 1;
                    Some(e)
                } else if next_is_arg(at) {
                    let v = argv[at + 1].clone();
                    at += 2;
                    Some(v)
                } else {
                    return Err(format!("argument {}: expected one argument", name(table, index)));
                }
            }
            K::Optional => {
                if let Some(e) = explicit {
                    at += 1;
                    Some(e)
                } else if next_is_arg(at) {
                    let v = argv[at + 1].clone();
                    at += 2;
                    Some(v)
                } else {
                    at += 1;
                    Some("*".to_string())
                }
            }
        };
        if let Some(v) = &value {
            if !choices.is_empty() && !choices.contains(&v.as_str()) {
                let listed: Vec<String> = choices.iter().map(|c| repr(c)).collect();
                return Err(format!(
                    "argument {}: invalid choice: {} (choose from {})",
                    name(table, index),
                    repr(v),
                    listed.join(", ")
                ));
            }
        }
        match kind {
            K::Help => return Ok(None),
            K::Flag => {
                values.insert(table[index].0, vec![String::new()]);
            }
            K::Value | K::Optional => {
                values.insert(table[index].0, vec![value.unwrap_or_default()]);
            }
            K::Append => values.entry(table[index].0).or_default().push(value.unwrap_or_default()),
        }
    }
    if !extras.is_empty() {
        return Err(format!("unrecognized arguments: {}", extras.join(" ")));
    }
    Ok(Some(values))
}
