//! Usage, help and the usage error at the terminal widths where the original's argparse layout changes (and at 80 and
//! 700): the binary is run with `COLUMNS` set and must print exactly what the original printed.

#[path = "bench_tool_common.rs"]
mod common;

use common::*;
use std::process::Command;

fn run(columns: &str, argument: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_resolve_bench_tool"))
        .arg(argument)
        .env("COLUMNS", columns)
        .env_remove("LINES")
        .output()
        .expect("run")
}

#[test]
fn help_and_usage_error_follow_the_terminal_width_like_the_original() {
    let cases = list(read_json("width_cases.json"));
    assert!(cases.len() > 60, "{} widths", cases.len());
    for case in &cases {
        let case = dict(case);
        let columns = field(case, "columns").repr();
        let help = run(&columns, "--help");
        assert_eq!(help.status.code(), Some(0), "help at {columns}");
        assert_eq!(
            String::from_utf8_lossy(&help.stdout),
            text(field(case, "help")),
            "help at {columns}"
        );
        let error = run(&columns, "--unknown");
        assert_eq!(error.status.code(), Some(2), "usage error at {columns}");
        assert_eq!(
            String::from_utf8_lossy(&error.stderr),
            text(field(case, "usage_error")),
            "usage error at {columns}"
        );
        assert!(
            error.stdout.is_empty() && help.stderr.is_empty(),
            "{columns}"
        );
    }
}
