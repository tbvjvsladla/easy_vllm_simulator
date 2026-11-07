//! What a call runs on: the two real writes the business code makes, the parser's own sinks, the trace of the file steps and the one dependency
//! import. The business code knows nothing else of the world.

use crate::thrown::{Done, Thrown};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use recipe_runtime::escalation::Raised;

/// The installation the original's `import yaml` meets: the bundled PyYAML 6.0.3, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YamlProfile {
    Available,
    Absent,
}

impl YamlProfile {
    /// `UPSTREAM_RUNTIME_YAML_PROFILE`: `flat` is an installation without PyYAML, anything else (or nothing) the bundled one.
    pub fn from_setting(setting: Option<&str>) -> YamlProfile {
        if setting == Some("flat") {
            YamlProfile::Absent
        } else {
            YamlProfile::Available
        }
    }

    /// The declared profile of a saved case: `pyyaml-6.0.3` or `flat`.
    pub fn from_declared(name: &str) -> Option<YamlProfile> {
        match name {
            "pyyaml-6.0.3" => Some(YamlProfile::Available),
            "flat" => Some(YamlProfile::Absent),
            _ => None,
        }
    }

    pub fn declared(self) -> &'static str {
        match self {
            YamlProfile::Available => "pyyaml-6.0.3",
            YamlProfile::Absent => "flat",
        }
    }

    /// `import yaml` against this installation.
    pub fn import_yaml(self) -> Done<()> {
        match self {
            YamlProfile::Available => Ok(()),
            YamlProfile::Absent => Err(Box::new(Thrown {
                raised: Raised::plain("ModuleNotFoundError", "No module named 'yaml'"),
                ctors: Vec::new(),
                bases: Some(
                    [
                        "ModuleNotFoundError",
                        "ImportError",
                        "Exception",
                        "BaseException",
                    ]
                    .iter()
                    .map(|name| name.to_string())
                    .collect(),
                ),
            })),
        }
    }
}

/// The streams and the trace of a call.
pub trait Sink {
    /// One `sys.stdout.write(text)`. The failure is the exception of that write (a `print` propagates it into the business code).
    fn out(&mut self, text: &PyStr) -> Done<()>;
    /// One `sys.stderr.write(text)` of a `print(..., file=sys.stderr)`.
    fn err(&mut self, text: &PyStr) -> Done<()>;
    /// argparse's help write (`_print_message`): a failure it swallows is dropped here, any other stops the parser (see `take_parser_failure`).
    fn parser_help(&mut self, help: &PyStr);
    /// argparse's usage and error-message writes, in that order, the second only when the first did not stop the parser.
    fn parser_error(&mut self, usage: &PyStr, message: &PyStr);
    /// The exception an argparse output write raised that argparse does NOT swallow: the parser stopped at that write, so it, not the
    /// `SystemExit`, is the outcome of the parse. Taken once.
    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        None
    }
    /// Whether the file steps are recorded (the control record of a check; nothing in a production run).
    fn tracing(&self) -> bool {
        false
    }
    fn trace(&mut self, _name: &str, _fields: Vec<(&str, PyValue)>) {}
}

/// A sink that also answers the original's `import yaml` at the point the original makes it.
pub trait Host: Sink {
    fn import_yaml(&mut self) -> Done<()>;
}

/// `except (AttributeError, OSError): pass` of `ArgumentParser._print_message`: whether the exception is one of those or a subclass.
pub fn argparse_swallows(thrown: &Thrown) -> bool {
    thrown.is_a("OSError") || thrown.is_a("AttributeError")
}

/// The outcome of one argparse output write: `Some` only for an exception argparse does not swallow.
pub fn parser_write_failure(outcome: Done<()>) -> Option<Box<Thrown>> {
    outcome.err().filter(|thrown| !argparse_swallows(thrown))
}
