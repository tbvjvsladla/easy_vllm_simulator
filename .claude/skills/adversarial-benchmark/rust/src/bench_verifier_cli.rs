//! Command lines of the four judge, lite and sweep-meta verification programs: `selftest_judge_bench`,
//! `selftest_lite_report`, `selftest_lite_gate` and `selftest_sweep_meta`.
//!
//! Like the originals, the judge and the lite gate programs ignore their arguments entirely (`--help` included); the
//! lite report and the sweep meta programs look only for `-v` among them. Standard output is the check list and the
//! closing line. Exit status 0 when every check passed, 1 when one failed, and 1 with a traceback marker plus
//! `Class: message` on standard error when the run raised an exception the original does not handle (the lines
//! printed before it stay on standard output). A `SystemExit` with a string reason is not a traceback: the string and a
//! newline go to standard error, exit 1; one with a status ends the run with that status.
//!
//! Status 2 is this port's: the repository the deployed files are copied from is not known (neither
//! `BENCH_VERIFY_REPO_ROOT` nor an installation around the running binary), so nothing was checked.

use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::conformance::Raised;
use crate::conformance_cli::traceback_line;
use crate::dataset_prep::Console;
use crate::dataset_prep_cli::StreamsConsole;
use crate::pystr::PyStr;
use crate::verify_fixture::resolve_repo_root;
use crate::{selftest_judge_bench, selftest_lite_gate, selftest_lite_report, selftest_sweep_meta};
use std::path::Path;

/// `selftest_judge_bench`: returns the exit status. `args` excludes the program name and is not read.
pub fn run_judge_bench(_args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_verifier(io, selftest_judge_bench::main)
}

/// `selftest_lite_report`: returns the exit status. `-v` among `args` asks for the verbose lines.
pub fn run_lite_report(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let verbose = has_verbose_flag(args);
    run_verifier(io, move |root, console| {
        selftest_lite_report::main(root, verbose, console)
    })
}

/// `selftest_lite_gate`: returns the exit status. `args` excludes the program name and is not read.
pub fn run_lite_gate(_args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_verifier(io, selftest_lite_gate::main)
}

/// `selftest_sweep_meta`: returns the exit status. `-v` among `args` asks for the verbose lines.
pub fn run_sweep_meta(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let verbose = has_verbose_flag(args);
    run_verifier(io, move |root, console| {
        selftest_sweep_meta::main(root, verbose, console)
    })
}

fn has_verbose_flag(args: &[PyStr]) -> bool {
    args.iter().any(|argument| argument.equals("-v"))
}

/// The status a `SystemExit` carries when it is not a message: `SystemExit(<number>)`.
fn exit_status_of(raised: &Raised) -> Option<i32> {
    let inner = raised
        .repr()
        .strip_prefix("SystemExit(")?
        .strip_suffix(')')?;
    inner.parse::<i32>().ok()
}

fn run_verifier(
    io: &mut Streams<'_>,
    verifier: impl FnOnce(Option<&Path>, &mut dyn Console) -> Result<i32, Raised>,
) -> i32 {
    let repo_root = resolve_repo_root();
    let mut console = StreamsConsole { io };
    let status = match verifier(repo_root.as_deref(), &mut console) {
        Ok(status) => status,
        Err(raised) if raised.is_system_exit() => match exit_status_of(&raised) {
            Some(status) => status,
            None => {
                // `SystemExit(reason)` with a string reason: the interpreter prints the string and a newline to
                // standard error, without a traceback, and exits 1.
                let mut text = raised.failure().message.clone();
                text.push_str("\n");
                write_stderr(console.io, &text);
                1
            }
        },
        Err(raised) => {
            let mut text = PyStr::new(TRACEBACK_HEADER);
            text.push_py(&traceback_line(&raised));
            text.push_str("\n");
            write_stderr(console.io, &text);
            1
        }
    };
    io.finish(status)
}
