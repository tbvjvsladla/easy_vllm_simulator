//! Command line of `resolve_topology`: the original's argparse interface (`--repo REPO`, `--self-test`) with byte-for-byte
//! usage, help and error text (logical program name `resolve_topology.py`). The Rust-only surface is the separate
//! `resolve_topology_api`.
//!
//! Standard output is the resolved topology (one line), standard error is the origin line; exit status 0 resolved,
//! 2 usage, 3 not terraformed, 5 unresolved. An uncaught exception ends with a traceback marker and `Class: message`
//! on standard error and status 1.

use crate::argv::{self, resolve_topology_args as arg, Outcome, RESOLVE_TOPOLOGY};
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::dataset_prep::Console;
use crate::dataset_prep_cli::{exit_status, StreamsConsole};
use crate::guidellm::Failure;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::resolve_topology::{
    resolve, ResolveContext, EXIT_OK, EXIT_UNTERRAFORMED, EXIT_USAGE, TAG,
};
use crate::resolve_topology_selftest::{self_test, SelfTestSources};
use crate::terminal;

/// What the command line needs besides its arguments: the interpreter of the parity owner and the sources of the
/// self-test (both come from the environment of the process; checks build their own).
#[derive(Clone, Debug)]
pub struct Runtime {
    pub resolve: ResolveContext,
    pub sources: SelfTestSources,
}

impl Runtime {
    pub fn from_environment() -> Self {
        Runtime {
            resolve: ResolveContext::from_environment(),
            sources: SelfTestSources::from_environment(),
        }
    }
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_with(args, &Runtime::from_environment(), io)
}

/// [`run`] with an explicit runtime.
pub fn run_with(args: &[PyStr], runtime: &Runtime, io: &mut Streams<'_>) -> i32 {
    let mut console = StreamsConsole { io };
    let status = match main_with(args, runtime, &mut console) {
        Ok(status) => status,
        Err(failure) if failure.class == "SystemExit" => exit_status(&failure),
        Err(failure) => {
            let mut text = PyStr::new(TRACEBACK_HEADER);
            text.push_py(&traceback_line(&failure));
            text.push_str("\n");
            write_stderr(console.io, &text);
            1
        }
    };
    io.finish(status)
}

/// `Class: message` as the interpreter prints the last traceback line.
fn traceback_line(failure: &Failure) -> PyStr {
    let mut text = PyStr::new(failure.class);
    if !failure.message.is_empty() {
        text.push_str(": ");
        text.push_py(&failure.message);
    }
    text
}

/// `main(argv)`: the exit status, or the exception that ends the program (`SystemExit` included).
pub fn main_with(
    args: &[PyStr],
    runtime: &Runtime,
    console: &mut dyn Console,
) -> Result<i32, Failure> {
    let matches = match argv::parse_with(&RESOLVE_TOPOLOGY, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            console.help(RESOLVE_TOPOLOGY.help(terminal::columns()));
            return Err(Failure::new("SystemExit", "0"));
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(RESOLVE_TOPOLOGY.usage(terminal::columns()));
            text.push_str(RESOLVE_TOPOLOGY.prog);
            text.push_str(": error: ");
            text.push_py(&message);
            text.push_str("\n");
            console.error(&text);
            return Err(Failure::new("SystemExit", "2"));
        }
    };
    if matches.flag(arg::SELF_TEST) {
        return self_test(console, &runtime.resolve, &runtime.sources);
    }
    let repo = match matches.value(arg::REPO) {
        Some(repo) if !repo.is_empty() => repo.clone(),
        _ => {
            console.error(&PyStr::new(&format!("{TAG} ERROR --repo 가 필요하다\n")));
            return Ok(EXIT_USAGE);
        }
    };
    let outcome = resolve(&PyValue::Str(repo), &PyValue::None, &runtime.resolve)?;
    if outcome.code == EXIT_OK {
        let mut origin = PyStr::new(&format!("{TAG} topology="));
        origin.push_py(&outcome.topology.display_text());
        origin.push_str(" ← ");
        origin.push_py(&outcome.why);
        origin.push_str("\n");
        console.error(&origin);
        console.print(&outcome.topology.display_text())?;
    } else {
        let label = if outcome.code == EXIT_UNTERRAFORMED {
            "미테라포밍"
        } else {
            "해소 불가"
        };
        let mut refusal = PyStr::new(&format!("{TAG} {label} — "));
        refusal.push_py(&outcome.why);
        refusal.push_str("\n");
        console.error(&refusal);
    }
    Ok(outcome.code)
}
