//! The frozen original-only command-line criteria: 91 invocations of the five programs (help, usage errors,
//! `--self-test`, stdout and file output, partial failures, the naming self-test, a real campaign writer as the
//! evidence owner). Each runs the real binary in a scratch tree; exit status, stdout, stderr (traceback projected
//! as the references define it) and the whole tree afterwards are compared.

#[path = "reporting_common.rs"]
mod common;

use common::*;
use std::process::Command;
use sweep_stop::pyvalue::PyValue;

fn binary(owner: &str) -> &'static str {
    match owner {
        "render_report" => env!("CARGO_BIN_EXE_render_report"),
        "render_sweep_map" => env!("CARGO_BIN_EXE_render_sweep_map"),
        "render_max_report" => env!("CARGO_BIN_EXE_render_max_report"),
        "publish_benchmark_record" => env!("CARGO_BIN_EXE_publish_benchmark_record"),
        "doc_naming" => env!("CARGO_BIN_EXE_bench_doc_naming"),
        other => panic!("unknown owner {other}"),
    }
}

#[test]
fn every_invocation_matches_the_original_streams_status_and_tree() {
    let PyValue::List(cases) = read_json("cli_cases.json") else {
        panic!("cases are a list")
    };
    assert_eq!(cases.len(), 91);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("cli");
        scratch.populate(field(case, "before"));
        let PyValue::List(argv) = field(case, "argv") else {
            panic!("argv is a list")
        };
        let output = Command::new(binary(&text(field(case, "owner"))))
            .args(
                argv.iter()
                    .map(|a| text(a).replace("@ROOT@", &scratch.text())),
            )
            .current_dir(&scratch.root)
            .env("COLUMNS", "80")
            .output()
            .expect("run");
        let rebased =
            |bytes: &[u8]| String::from_utf8_lossy(bytes).replace(&scratch.text(), "@ROOT@");
        let status_ok = output.status.code()
            == Some(match field(case, "exit_code") {
                PyValue::Int(code) => i32::try_from(code.clone()).expect("exit code"),
                other => panic!("exit code {}", other.repr()),
            });
        let stdout_ok = rebased(&output.stdout) == text(field(case, "stdout"));
        let stderr_ok = traceback_projection(&rebased(&output.stderr))
            == traceback_projection(&text(field(case, "stderr")));
        let tree_ok = scratch.snapshot() == expected_tree(field(case, "after"));
        if !(status_ok && stdout_ok && stderr_ok && tree_ok) {
            failures.push(format!(
                "{id}: status={status_ok} stdout={stdout_ok} stderr={stderr_ok} tree={tree_ok}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "differing invocations:\n{}",
        failures.join("\n")
    );
}
