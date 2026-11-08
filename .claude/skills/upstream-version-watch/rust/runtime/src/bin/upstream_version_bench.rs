//! `upstream_version_bench --fixture FILE [--rounds 11] [--iterations 200]`: the frozen post-quality timing of the two ported functions
//! (see `upstream_runtime::version_bench`). The fixture is decoded once; BEFORE any timer every call of every row is run once and checked against its
//! saved whole value DTO and origin path (a mismatch, or a call that raises, ends the run with no report); then each row (its own `iterations`, otherwise
//! `--iterations`) is timed over `--rounds` rounds as whole iterations of its calls that build and drop the raw returns; the report is printed only when
//! every series succeeded. It never approves anything.

use recipe_runtime::stdio::os_failure;
use std::io::Write;
use std::process::ExitCode;
use upstream_runtime::version_bench::{load, run_rows};

fn usage() -> ExitCode {
    let _ = writeln!(
        std::io::stderr(),
        "usage: upstream_version_bench --fixture FILE [--rounds N] [--iterations N]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut fixture = None;
    let (mut rounds, mut iterations) = (11usize, 200usize);
    let mut arguments = std::env::args().skip(1);
    while let Some(flag) = arguments.next() {
        let value = arguments.next();
        match (flag.as_str(), value) {
            ("--fixture", Some(path)) => fixture = Some(path),
            ("--rounds", Some(count)) => match count.parse() {
                Ok(count) if count > 0 => rounds = count,
                _ => return usage(),
            },
            ("--iterations", Some(count)) => match count.parse() {
                Ok(count) if count > 0 => iterations = count,
                _ => return usage(),
            },
            _ => return usage(),
        }
    }
    let Some(path) = fixture else { return usage() };
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "upstream_version_bench: cannot read the fixture: {}",
                os_failure(&error).line().to_utf8().unwrap_or_default()
            );
            return ExitCode::from(1);
        }
    };
    let report = match load(&source).and_then(|rows| run_rows(&rows, rounds, iterations)) {
        Ok(report) => report,
        Err(failure) => {
            let _ = writeln!(
                std::io::stderr(),
                "upstream_version_bench: {}",
                failure.line().to_utf8().unwrap_or_default()
            );
            return ExitCode::from(1);
        }
    };
    let mut out = std::io::stdout().lock();
    match out.write_all(report.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "upstream_version_bench: cannot write standard output: {}",
                os_failure(&error).line().to_utf8().unwrap_or_default()
            );
            ExitCode::from(1)
        }
    }
}
