//! `conformance_full_superset_lite_api` as a program: the frozen helper and transport cases through the
//! `evaluate-jsonl` envelope (the `stdout` field included, also for an exception), the shape of the envelope for every
//! kind of failure, the opt-in JSON forms, the constants and the usage text.

#[path = "conformance_common.rs"]
mod common;

use common::*;
use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

const API: &str = env!("CARGO_BIN_EXE_conformance_full_superset_lite_api");

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str], stdin: &str, scripts_dir: Option<&std::path::Path>) -> Run {
    let mut command = Command::new(API);
    command
        .args(args)
        .env_remove("CONFORMANCE_SCRIPTS_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = scripts_dir {
        command.env("CONFORMANCE_SCRIPTS_DIR", dir);
    }
    let mut child = command.spawn().expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    Run {
        status: output.status.code().expect("status"),
        stdout: String::from_utf8(output.stdout).expect("UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("UTF-8"),
    }
}

fn records(run: &Run) -> Vec<PyDict> {
    run.stdout
        .lines()
        .map(|line| match loads(line).expect("JSON line") {
            PyValue::Dict(map) => map,
            other => panic!("not an object: {}", other.repr()),
        })
        .collect()
}

fn row(id: &str, operation: &str, arguments: &str) -> String {
    format!("{{\"id\": \"{id}\", \"operation\": \"{operation}\", \"arguments\": {arguments}}}\n")
}

fn arguments_json(value: &PyValue) -> String {
    value.dumps_ascii().expect("JSON")
}

#[test]
fn frozen_helper_and_transport_cases_pass_through_the_envelope() {
    for file in ["helper_cases.json", "transport_cases.json"] {
        for case in list(read_json(file)) {
            let case = dict(&case);
            let tree = Tree::new("api");
            tree.materialize(dict(field(case, "before")));
            let id = text(field(case, "id"));
            let input = row(
                &id,
                &text(field(case, "operation")),
                &arguments_json(field(case, "arguments")),
            );
            let outcome = run(&["evaluate-jsonl"], &input, Some(&tree.root));
            assert_eq!(outcome.status, 0, "{id}");
            assert_eq!(outcome.stderr, "", "{id}");
            let found = records(&outcome);
            assert_eq!(found.len(), 1, "{id}");
            assert_eq!(text(field(&found[0], "id")), id);
            expected_matches(&found[0], dict(field(case, "expected")))
                .unwrap_or_else(|why| panic!("{id}: {why}"));
        }
    }
}

#[test]
fn a_printing_operation_keeps_the_text_printed_before_its_exception() {
    let input = row(
        "short",
        "_emit",
        r#"{"results": [["a", true, ""], ["b", false, "x"], ["short", false]]}"#,
    ) + &row(
        "bad",
        "_emit",
        r#"{"results": [["a", true, ""], ["b", false, {"k": 1}]]}"#,
    );
    let found = records(&run(&["evaluate-jsonl"], &input, None));
    assert_eq!(text(field(&found[0], "kind")), "exception");
    assert_eq!(text(field(&found[0], "type")), "ValueError");
    assert_eq!(text(field(&found[0], "stdout")), "PASS a\nFAIL b  — x\n");
    assert_eq!(text(field(&found[1], "type")), "TypeError");
    assert_eq!(text(field(&found[1], "stdout")), "PASS a\n");
}

#[test]
fn main_through_the_api_returns_the_status_and_the_whole_report() {
    let tree = Tree::new("api-main");
    for name in [
        "sweep_bench.sh",
        "publish_benchmark_record.sh",
        "lite_metrics_api.sh",
    ] {
        tree.write(name, b"");
    }
    let found = records(&run(
        &["evaluate-jsonl"],
        &row("m", "main", "{}"),
        Some(&tree.root),
    ));
    assert_eq!(field(&found[0], "value"), &PyValue::int(1));
    assert!(text(field(&found[0], "stdout")).ends_with("full ⊇ lite conformance: 3/4 FAIL\n"));
}

#[test]
fn operations_that_need_the_scripts_directory_say_so_when_it_is_not_set() {
    let input = row("r", "read", r#"{"name": "x"}"#) + &row("e", "_emit", r#"{"results": []}"#);
    let found = records(&run(&["evaluate-jsonl"], &input, None));
    assert_eq!(text(field(&found[0], "type")), "RuntimeError");
    assert!(text(field(&found[0], "message")).starts_with("CONFORMANCE_SCRIPTS_DIR is not set"));
    assert_eq!(field(&found[1], "value"), &PyValue::Bool(true));
}

#[test]
fn argument_binding_failures_are_the_python_type_errors() {
    let tree = Tree::new("api-binding");
    let input = row("a", "read", "{}")
        + &row("b", "read", r#"{"name": 3}"#)
        + &row("c", "_emit", r#"{"results": [], "extra": 1}"#)
        + &row("d", "native_lite_certificate_gates", r#"{"x": 1}"#)
        + &row("e", "_emit", r#"{"results": 5}"#)
        + &row("f", "no_such", "{}");
    let found = records(&run(&["evaluate-jsonl"], &input, Some(&tree.root)));
    let message = |at: usize| text(field(&found[at], "message"));
    assert_eq!(
        message(0),
        "read() missing 1 required positional argument: 'name'"
    );
    assert_eq!(
        message(1),
        "join() argument must be str, bytes, or os.PathLike object, not 'int'"
    );
    assert_eq!(
        message(2),
        "_emit() got an unexpected keyword argument 'extra'"
    );
    assert_eq!(
        message(3),
        "native_lite_certificate_gates() got an unexpected keyword argument 'x'"
    );
    assert_eq!(message(4), "'int' object is not iterable");
    assert_eq!(message(5), "unknown operation 'no_such'");
    assert_eq!(text(field(&found[4], "stdout")), "");
    assert!(found[5].get("stdout").is_none());
}

#[test]
fn rows_that_are_not_operations_get_an_exception_record() {
    let outcome = run(&["evaluate-jsonl"], "[1]\nnot json\n{\"id\": 1}\n", None);
    let found = records(&outcome);
    assert_eq!(found.len(), 3);
    assert_eq!(
        text(field(&found[0], "message")),
        "operation row is not an object"
    );
    assert_eq!(text(field(&found[1], "type")), "JSONDecodeError");
    assert_eq!(text(field(&found[2], "type")), "KeyError");
    assert_eq!(field(&found[2], "id"), &PyValue::int(1));
}

#[test]
fn the_json_forms_are_opt_in_and_decoded_per_argument() {
    let input = "{\"id\": \"j\", \"operation\": \"_emit\", \"arguments_json\": {\"results\": \"[[\\\"a\\\", 1, \\\"\\\"]]\"}, \"result_json\": true}\n";
    let found = records(&run(&["evaluate-jsonl"], input, None));
    assert_eq!(text(field(&found[0], "encoding")), "json-values-v1");
    assert_eq!(text(field(&found[0], "value_json")), "true");
    assert_eq!(
        text(field(&found[0], "stdout")),
        "PASS a\n\nfull ⊇ lite conformance: 1/1 PASS\n"
    );
}

#[test]
fn constants_and_help_and_usage() {
    let constants = run(&["constants"], "", None);
    assert_eq!(constants.status, 0);
    assert_eq!(
        constants.stdout,
        "{\"LITE_METRIC_KEYS\": [\"gen_tps\", \"cold_ttft_ms\", \"kv_gib\", \"capacity\", \"accept_len\"], \"CERT_LITE_FIELDS\": [\"lite_included\", \"lite_gen_tps_warm\", \"lite_cold_ttft_ms\", \"lite_kv_gib\", \"lite_gpu_occupancy\", \"lite_ram_occupancy\", \"lite_accept_len\"]}\n"
    );
    let help = run(&["native-help"], "", None);
    assert_eq!(help.status, 0);
    assert!(help.stdout.contains("evaluate-jsonl"));
    for arguments in [&[][..], &["--help"], &["constants", "extra"], &["other"]] {
        let outcome = run(arguments, "", None);
        assert_eq!(outcome.status, 2, "{arguments:?}");
        assert_eq!(outcome.stdout, "");
        assert!(outcome.stderr.contains("Rust-only commands"));
    }
}

#[test]
fn the_api_runs_the_real_launchers_of_a_private_installation() {
    let tree = Tree::new("api-installed");
    let scripts = install_copy(&tree);
    let mut command = Command::new(API);
    with_overrides(
        &mut command,
        env!("CARGO_BIN_EXE_bench_reporting_api"),
        env!("CARGO_BIN_EXE_publish_benchmark_record"),
        env!("CARGO_BIN_EXE_lite_metrics_api"),
    );
    let mut child = command
        .arg("evaluate-jsonl")
        .env("CONFORMANCE_SCRIPTS_DIR", &scripts)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let input = row("g", "native_lite_certificate_gates", "{}")
        + &row(
            "v",
            "native_value",
            r#"{"operation": "render_report.build_md", "arguments": {"index": {"meta": {}, "levels": [], "lite": {"table": "T-ABLE"}}, "verdict": {"verdict": "PASS"}, "roofline": null}}"#,
        )
        + &row("main", "main", "{}");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let found = records(&Run {
        status: 0,
        stdout: String::from_utf8(output.stdout).expect("UTF-8"),
        stderr: String::new(),
    });
    assert_eq!(field(&found[0], "value"), &PyValue::Bool(true));
    assert!(text(field(&found[1], "value")).contains("T-ABLE"));
    assert_eq!(field(&found[2], "value"), &PyValue::int(0));
    assert_eq!(
        text(field(&found[2], "stdout")),
        read_text("normal_stdout.txt")
    );
}

#[test]
fn a_reported_system_exit_is_an_exception_envelope_not_a_process_exit() {
    let tree = Tree::new("api-system-exit");
    let scripts = install_copy(&tree);
    let record = "{\"id\":\"python-consumer\",\"encoding\":\"json-values-v1\",\"kind\":\"exception\",\"type\":\"SystemExit\",\"message\":\"0\"}";
    std::fs::write(
        scripts.join("lite_metrics_api.sh"),
        format!("#!/bin/bash\nif [ \"$1\" = constants ]; then\nprintf '%s\\n' '{{\"LITE_VERDICTS\":[\"pass\",\"measurement_path_failed\",\"server_failed\"],\"LITE_EXIT\":{{\"pass\":0,\"measurement_path_failed\":6,\"server_failed\":7}}}}'\nelse\ncat >/dev/null\nprintf '%s\\n' '{record}'\nfi\n"),
    )
    .expect("double");
    let mut command = Command::new(API);
    with_overrides(
        &mut command,
        env!("CARGO_BIN_EXE_bench_reporting_api"),
        env!("CARGO_BIN_EXE_publish_benchmark_record"),
        env!("CARGO_BIN_EXE_lite_metrics_api"),
    );
    let mut child = command
        .arg("evaluate-jsonl")
        .env("CONFORMANCE_SCRIPTS_DIR", &scripts)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(row("m", "main", "{}").as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(
        output.status.code(),
        Some(0),
        "the API process itself does not exit with the reason"
    );
    assert!(output.stderr.is_empty());
    let found = records(&Run {
        status: 0,
        stdout: String::from_utf8(output.stdout).expect("UTF-8"),
        stderr: String::new(),
    });
    assert_eq!(text(field(&found[0], "kind")), "exception");
    assert_eq!(text(field(&found[0], "type")), "SystemExit");
    assert_eq!(text(field(&found[0], "message")), "0");
    assert_eq!(text(field(&found[0], "stdout")), "");
}
