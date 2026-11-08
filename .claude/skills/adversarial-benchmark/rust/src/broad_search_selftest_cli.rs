//! Command lines of the two Broad Search verification programs, `selftest_broad_search_cell_args` and
//! `selftest_broad_search_precheck`. Like the originals they ignore their arguments entirely (`--help` included).
//!
//! Standard output is the report, one line per check and a summary. Exit status 0 when every check passed, 1 when
//! one failed, and 1 with a traceback marker plus `Class: message` on standard error when the run raised an exception
//! the original does not handle (the lines printed before it stay on standard output). A `SystemExit` with a string
//! reason is not a traceback: the string and a newline go to standard error, exit 1. The absence of `git` is the
//! original's own named failure (a line on standard error, exit 1, nothing checked).
//!
//! Status 2 is this port's: the repository the deployed files are copied from is not known (neither
//! `BENCH_VERIFY_REPO_ROOT` nor an installation around the running binary), so nothing was checked.

use crate::broad_search_cell_args_selftest as cell_args;
use crate::broad_search_precheck_selftest as precheck;
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::conformance::Raised;
use crate::conformance_cli::traceback_line;
use crate::dataset_prep::Console;
use crate::dataset_prep_cli::StreamsConsole;
use crate::pystr::PyStr;
use crate::verify_fixture::resolve_repo_root;
use std::path::Path;

/// The deployed files `selftest_broad_search_cell_args` copies into its checkout, as repository-relative paths.
pub const CELL_ARGS_DEPLOYED_FILES: [&str; 35] = cell_args::COPIES;

/// The deployed files `selftest_broad_search_precheck` copies into its checkout, as repository-relative paths.
pub const PRECHECK_DEPLOYED_FILES: [&str; 35] = precheck::COPIES;

type Verifier = fn(Option<&Path>, &mut dyn Console) -> Result<i32, Raised>;

/// `selftest_broad_search_cell_args`: returns the exit status. `args` excludes the program name and is not read.
pub fn run_cell_args(_args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_verifier(cell_args::main, io)
}

/// `selftest_broad_search_precheck`: returns the exit status. `args` excludes the program name and is not read.
pub fn run_precheck(_args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_verifier(precheck::main, io)
}

fn run_verifier(verifier: Verifier, io: &mut Streams<'_>) -> i32 {
    let repo_root = resolve_repo_root();
    let mut console = StreamsConsole { io };
    let status = match verifier(repo_root.as_deref(), &mut console) {
        Ok(status) => status,
        // `SystemExit(reason)` with a string reason: the interpreter prints the string and a newline to standard
        // error, without a traceback, and exits 1.
        Err(raised) if raised.is_system_exit() => {
            let mut text = raised.failure().message.clone();
            text.push_str("\n");
            write_stderr(console.io, &text);
            1
        }
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
