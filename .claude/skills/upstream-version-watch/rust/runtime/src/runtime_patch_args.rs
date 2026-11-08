//! `parser.parse_args(argv)` of `validate_runtime_patch.py`: the required `action` positional (`stamp`/`verify`), the required `--patch`,
//! `--topology {multi,single}`, `--config-dir`, `--env-dir` and `--resolution`, with argparse's abbreviations, `--opt=value`, usage, help and
//! error texts.
//!
//! The option grammar is the recipe package's [`cliparse`](recipe_runtime::cliparse) model. That model has an *optional* positional without
//! choices and prints `required` as brackets only through its own flag, so the program is parsed twice over the same arguments:
//! 1. a relaxed pass (nothing required) learns which options and which positional were given, and what errors the option grammar itself finds;
//! 2. the checks only this program has are applied in argparse's order: an invalid `action` choice, then the missing required arguments
//!    (`action` first, then the options in declaration order), then `unrecognized arguments`.
//!
//! The usage and help texts are formatted from the *required* spec, and the one thing the model cannot print, the positional without brackets, is
//! restored in the text it formats (`[{stamp,verify}]` to `{stamp,verify}`). A usage that would fit on one line without the two brackets but not
//! with them is the single-line text.
//!
//! Residual (named, not decided): when two *different* usage errors are present at once (for example an invalid `action` and an invalid
//! `--topology`), argparse reports the one met first while consuming the arguments left to right; this port reports the option-grammar error
//! first. Each error alone, and a missing argument with any other single error, are the argparse texts.

use crate::main_cmd::{basename, intern, split_usage};
use agent_runtime_support::pystr::PyStr;
use recipe_runtime::cliparse::{
    format_help, format_usage, parse, terminal_width, Kind, Opt, Outcome, Positional, Spec, Value,
};
use std::collections::BTreeMap;

pub const ACTION_CHOICES: &[&str] = &["stamp", "verify"];
const POSITIONAL: &str = "{stamp,verify}";
const TOPOLOGIES: &[&str] = &["multi", "single"];

/// The arguments of a parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    pub action: PyStr,
    pub patch: PyStr,
    pub topology: PyStr,
    pub config_dir: PyStr,
    pub env_dir: PyStr,
    pub resolution: PyStr,
}

/// What `parse_args` ended with when it did not return the arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    pub status: i32,
    /// The help text (written to standard output), empty for an error.
    pub help: PyStr,
    /// The usage and the one-line error (written to standard error), both empty for help.
    pub usage: PyStr,
    pub error: PyStr,
}

/// `(flags, dest, is a choice)` in declaration order.
const OPTIONS: [(&[&str], &str, bool); 5] = [
    (&["--patch"], "patch", false),
    (&["--topology"], "topology", true),
    (&["--config-dir"], "config_dir", false),
    (&["--env-dir"], "env_dir", false),
    (&["--resolution"], "resolution", false),
];

fn spec(program: &str, required: bool) -> Spec {
    let mut options = vec![Opt {
        flags: &["-h", "--help"],
        dest: "help",
        kind: Kind::Help,
        help: None,
        required: false,
        group: None,
    }];
    for (flags, dest, is_choice) in OPTIONS {
        options.push(Opt {
            flags,
            dest,
            kind: if is_choice {
                Kind::Choice(TOPOLOGIES)
            } else {
                Kind::Text
            },
            help: None,
            required,
            group: None,
        });
    }
    Spec {
        prog: intern(program),
        description: "",
        options,
        group_required: false,
        positional: Some(Positional {
            dest: POSITIONAL,
            help: None,
        }),
    }
}

fn replace_all(text: &PyStr, from: &str, to: &str) -> PyStr {
    let from: Vec<u32> = from.chars().map(u32::from).collect();
    let to: Vec<u32> = to.chars().map(u32::from).collect();
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

fn without_brackets(text: &PyStr) -> PyStr {
    replace_all(text, &format!("[{POSITIONAL}]"), POSITIONAL)
}

/// The usage argparse formats: the model's text without the positional's brackets; a text the model wrapped only because of those two characters
/// is the single line.
fn usage_text(spec: &Spec, width: usize) -> PyStr {
    let one_line = without_brackets(&PyStr::new(&format_usage(spec, 100_000)));
    let columns = one_line.len().saturating_sub(1);
    if columns <= width {
        return one_line;
    }
    without_brackets(&PyStr::new(&format_usage(spec, width)))
}

/// The common help formatter puts a description paragraph after the usage; this parser has none, so the four line breaks after the usage are two.
fn without_extra_blank_line(help: &PyStr) -> PyStr {
    replace_all(help, "\n\n\n\n", "\n\n")
}

fn text_of(values: &BTreeMap<&'static str, Value>, key: &str) -> Option<PyStr> {
    match values.get(key) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

/// The message an unrecognized-arguments error lists, and the arguments in it.
fn unrecognized_in(message: &PyStr) -> Vec<PyStr> {
    let text = message.to_utf8().unwrap_or_default();
    text.split_once("unrecognized arguments: ")
        .map(|(_, list)| list.trim_end().split(' ').map(PyStr::new).collect())
        .unwrap_or_default()
}

/// The two checks only this program has, over a relaxed parse that returned values: an invalid `action`, then the missing required arguments.
fn check(values: &BTreeMap<&'static str, Value>) -> Result<Arguments, String> {
    let action = text_of(values, POSITIONAL);
    if let Some(action) = &action {
        if !ACTION_CHOICES.iter().any(|choice| action.equals(choice)) {
            let shown: Vec<String> = ACTION_CHOICES
                .iter()
                .map(|choice| PyStr::new(choice).repr())
                .collect();
            return Err(format!(
                "argument action: invalid choice: {} (choose from {})",
                action.repr(),
                shown.join(", ")
            ));
        }
    }
    let mut missing: Vec<&str> = Vec::new();
    if action.is_none() {
        missing.push("action");
    }
    for (flags, dest, _) in OPTIONS {
        if text_of(values, dest).is_none() {
            missing.push(flags[0]);
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "the following arguments are required: {}",
            missing.join(", ")
        ));
    }
    let take = |key: &str| text_of(values, key).expect("checked present");
    Ok(Arguments {
        action: action.expect("checked present"),
        patch: take("patch"),
        topology: take("topology"),
        config_dir: take("config_dir"),
        env_dir: take("env_dir"),
        resolution: take("resolution"),
    })
}

/// `parser.parse_args(argv[1:])` for `argv[0]` = the program path (its basename is the program name argparse prints).
pub fn parse_arguments(argv: &[PyStr], columns: Option<&str>) -> Result<Arguments, Exit> {
    let program = argv.first().map(basename).unwrap_or_default();
    let width = terminal_width(columns);
    let required = spec(&program, true);
    let relaxed = spec(&program, false);
    let usage = usage_text(&required, width);
    let error_exit = |message: &str| Exit {
        status: 2,
        help: PyStr::default(),
        usage: usage.clone(),
        error: PyStr::new(&format!("{program}: error: {message}\n")),
    };
    let arguments = &argv[argv.len().min(1)..];
    let relaxed_usage = PyStr::new(&format_usage(&relaxed, width));
    let outcome = match parse(&relaxed, arguments, columns) {
        Outcome::Exit {
            status: 2, stderr, ..
        } => {
            let (_, rest) = split_usage(&relaxed_usage, &stderr);
            // argparse reports a missing argument and an invalid choice before `unrecognized arguments`
            let unrecognized = unrecognized_in(&rest);
            let remaining: Vec<PyStr> = arguments
                .iter()
                .filter(|argument| !unrecognized.contains(argument))
                .cloned()
                .collect();
            if !unrecognized.is_empty() {
                if let Outcome::Values(values) = parse(&relaxed, &remaining, columns) {
                    return Err(match check(&values) {
                        Ok(_) => Exit {
                            usage: usage.clone(),
                            error: rest.clone(),
                            ..error_exit("")
                        },
                        Err(found) => error_exit(&found),
                    });
                }
            }
            return Err(Exit {
                status: 2,
                help: PyStr::default(),
                usage,
                error: rest,
            });
        }
        other => other,
    };
    match outcome {
        Outcome::Values(values) => check(&values).map_err(|message| error_exit(&message)),
        Outcome::Exit { status, .. } => Err(Exit {
            status,
            // the relaxed pass printed its own help; the program's help is the required spec's
            help: without_extra_blank_line(&without_brackets(&format_help(&required, width))),
            usage: PyStr::default(),
            error: PyStr::default(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<PyStr> {
        items.iter().map(|item| PyStr::new(item)).collect()
    }

    const FULL: [&str; 11] = [
        "verify",
        "--patch",
        "p",
        "--topology",
        "single",
        "--config-dir",
        "c",
        "--env-dir",
        "e",
        "--resolution",
        "r",
    ];

    fn program_and(items: &[&str]) -> Vec<PyStr> {
        let mut all = vec!["/x/validate_runtime_patch.py"];
        all.extend_from_slice(items);
        argv(&all)
    }

    #[test]
    fn a_full_command_line_parses_and_options_may_be_abbreviated_or_use_equals() {
        let parsed = parse_arguments(&program_and(&FULL), Some("80")).expect("values");
        assert_eq!(parsed.action.to_utf8().as_deref(), Some("verify"));
        let abbreviated = [
            "--env-d=e",
            "verify",
            "--conf=c",
            "--top=single",
            "--pat=p",
            "--res=r",
        ];
        let parsed = parse_arguments(&program_and(&abbreviated), Some("80")).expect("values");
        assert_eq!(parsed.env_dir.to_utf8().as_deref(), Some("e"));
    }

    #[test]
    fn no_arguments_lists_the_positional_first_then_every_option_in_declaration_order() {
        let exit = parse_arguments(&program_and(&[]), Some("80")).expect_err("exit");
        assert_eq!(exit.status, 2);
        assert_eq!(
            exit.error.to_utf8().unwrap(),
            "validate_runtime_patch.py: error: the following arguments are required: action, --patch, --topology, --config-dir, --env-dir, --resolution\n"
        );
        assert_eq!(
            exit.usage.to_utf8().unwrap(),
            "usage: validate_runtime_patch.py [-h] --patch PATCH --topology {multi,single}\n                                 --config-dir CONFIG_DIR --env-dir ENV_DIR\n                                 --resolution RESOLUTION\n                                 {stamp,verify}\n"
        );
    }

    #[test]
    fn an_invalid_action_is_named_before_the_missing_options() {
        let exit = parse_arguments(&program_and(&["unknown", "--patch", "p"]), Some("80"))
            .expect_err("exit");
        assert_eq!(
            exit.error.to_utf8().unwrap(),
            "validate_runtime_patch.py: error: argument action: invalid choice: 'unknown' (choose from 'stamp', 'verify')\n"
        );
    }

    #[test]
    fn an_unrecognized_argument_comes_after_a_missing_required_one() {
        let mut items = FULL.to_vec();
        items.push("--unknown");
        let exit = parse_arguments(&program_and(&items), Some("80")).expect_err("exit");
        assert!(exit
            .error
            .to_utf8()
            .unwrap()
            .ends_with("unrecognized arguments: --unknown\n"));
        let exit = parse_arguments(&program_and(&["--unknown"]), Some("80")).expect_err("exit");
        assert!(exit
            .error
            .to_utf8()
            .unwrap()
            .ends_with("the following arguments are required: action, --patch, --topology, --config-dir, --env-dir, --resolution\n"));
    }

    #[test]
    fn help_is_the_usage_a_blank_line_and_the_argument_sections() {
        let exit = parse_arguments(&program_and(&["-h"]), Some("80")).expect_err("exit");
        assert_eq!(exit.status, 0);
        let help = exit.help.to_utf8().unwrap();
        assert!(help.starts_with("usage: validate_runtime_patch.py [-h] --patch PATCH"));
        assert!(
            help.contains("\n\npositional arguments:\n  {stamp,verify}\n\noptions:\n  -h, --help")
        );
    }

    #[test]
    fn a_wide_terminal_prints_the_usage_on_one_line() {
        let exit = parse_arguments(&program_and(&[]), Some("400")).expect_err("exit");
        assert_eq!(exit.usage.to_utf8().unwrap().matches('\n').count(), 1);
    }
}
