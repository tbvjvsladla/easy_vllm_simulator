//! Command line of `conformance_full_superset_lite`: the original ignores its arguments entirely (`--help` included),
//! so this does too. Standard output is the report, one line per check and a summary; exit status 0 when every check
//! passed, 1 when one failed, and 1 with a traceback marker plus `Class: message` on standard error when a check
//! raised an exception the original does not handle (the lines printed before it stay on standard output). A reported
//! `SystemExit` is not a traceback: its string reason and a newline go to standard error, exit 1, no report.
//!
//! Status 2 is this port's: the scripts directory is not known (neither `CONFORMANCE_SCRIPTS_DIR` nor an installation
//! around the running binary), so nothing was checked.

use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::conformance::{Context, Raised};
use crate::conformance_checks::main;
use crate::dataset_prep_cli::StreamsConsole;
use crate::pystr::PyStr;

/// Runs the program and returns its exit status. `args` excludes the program name and is not read.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    run_with(args, Context::resolve().as_ref(), io)
}

/// [`run`] with an explicit context (`None` = no scripts directory is known).
pub fn run_with(_args: &[PyStr], context: Option<&Context>, io: &mut Streams<'_>) -> i32 {
    let Some(context) = context else {
        write_stderr(
            io,
            &PyStr::new("[conformance_full_superset_lite] ERROR CONFORMANCE_SCRIPTS_DIR 이 없고 실행파일이 설치 경로(rust/target/release) 안에 있지 않아 스크립트 디렉터리를 알 수 없다\n"),
        );
        return io.finish(2);
    };
    let mut console = StreamsConsole { io };
    let status = match main(context, &mut console) {
        Ok(status) => status,
        // `SystemExit(reason)` with a string reason (the protocol's `message` field is a string): the interpreter prints
        // the string and a newline to standard error, without a traceback, and exits 1. A reason such as "0" or ""
        // is still a string reason, never a numeric status.
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

/// `Class: message` as the interpreter prints the last traceback line (library classes carry their module).
pub(crate) fn traceback_line(raised: &Raised) -> PyStr {
    let failure = raised.failure();
    let class = match raised.qualified_class().unwrap_or(failure.class) {
        "JSONDecodeError" => "json.decoder.JSONDecodeError",
        "TimeoutExpired" => "subprocess.TimeoutExpired",
        "CalledProcessError" => "subprocess.CalledProcessError",
        other => other,
    };
    let mut text = PyStr::new(class);
    if !failure.message.is_empty() {
        text.push_str(": ");
        text.push_py(&failure.message);
    }
    text
}
