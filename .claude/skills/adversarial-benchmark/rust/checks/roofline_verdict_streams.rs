//! When stdout cannot be written, `roofline` and `verdict_rule` follow the same rule as the other executables of
//! this crate (see `src/stdout.rs`): a result that fits the interpreter's buffer fails at exit (status 120,
//! `Exception ignored in: <_io.TextIOWrapper ...>`), 8191 bytes or more fail at the print (traceback, status 1).
//! The status of a verdict is never kept when its output was lost.

#[path = "common.rs"]
mod common;

use common::{command, roofline_inputs};
use std::fs::OpenOptions;
use std::io::Write;
use std::process::{Command, Stdio};

const ROOFLINE: &str = env!("CARGO_BIN_EXE_roofline");
const VERDICT_RULE: &str = env!("CARGO_BIN_EXE_verdict_rule");
const DEFERRED: &str = "Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\nOSError: [Errno 28] No space left on device\n";

fn dev_full() -> Stdio {
    Stdio::from(
        OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .expect("/dev/full"),
    )
}

fn gone_reader() -> Stdio {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    Stdio::from(writer)
}

#[test]
fn small_results_fail_at_exit_with_status_120() {
    let dense = roofline_inputs().join("root/dense");
    let cases: Vec<(&str, Vec<String>)> = vec![
        (
            ROOFLINE,
            vec![
                "--model-path".into(),
                dense.to_str().unwrap().into(),
                "--bandwidth-gbps".into(),
                "273".into(),
            ],
        ),
        (ROOFLINE, vec!["-h".into()]),
        (VERDICT_RULE, vec!["--self-test".into()]),
        (VERDICT_RULE, vec!["-h".into()]),
        (VERDICT_RULE, vec!["native-help".into()]),
        (ROOFLINE, vec!["native-help".into()]),
    ];
    for (bin, argv) in cases {
        let output = command(bin, None, &argv)
            .stdin(Stdio::null())
            .stdout(dev_full())
            .stderr(Stdio::piped())
            .output()
            .expect("run");
        let stderr = String::from_utf8_lossy(&output.stderr);
        // roofline prints a metadata warning for the dense fixture before the result; the failure line follows it
        assert_eq!(output.status.code(), Some(120), "{argv:?}: {stderr}");
        assert!(stderr.ends_with(DEFERRED), "{argv:?}: {stderr}");
    }
}

#[test]
fn a_verdict_status_does_not_survive_a_lost_output() {
    let dir = roofline_inputs();
    let measured = dir.join("root/measured.json");
    let roofline = dir.join("root/roofline.json");
    let args = vec![
        "--measured".to_string(),
        measured.to_str().unwrap().into(),
        "--roofline".into(),
        roofline.to_str().unwrap().into(),
    ];
    let ok = command(VERDICT_RULE, None, &args)
        .stdin(Stdio::null())
        .output()
        .expect("run");
    assert_eq!(
        ok.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    for sink in [dev_full(), gone_reader()] {
        let lost = command(VERDICT_RULE, None, &args)
            .stdin(Stdio::null())
            .stdout(sink)
            .stderr(Stdio::piped())
            .output()
            .expect("run");
        assert_eq!(
            lost.status.code(),
            Some(120),
            "{}",
            String::from_utf8_lossy(&lost.stderr)
        );
    }
}

#[test]
fn large_output_fails_at_the_print_with_status_1() {
    let row = r#"{"id": 1, "operation": "build_verdict", "measured": {"decode_tps": 34.0, "spec_on": true, "accept_len": 1.85, "completed": 16, "failed": 0, "measurement_ok": true}, "roofline": {"R_fp": 30.0, "R_token": 45.0, "expected_achievable": 12.6}, "opts": {"authority": "weak", "reference_tps": 41.0, "target_tps": null, "tolerance": 0.15, "node_vram": null, "balance_tol": 0.1, "spec_declared": "unknown", "spec_supported": false, "e_search": "no"}}"#;
    let payload = (0..20).map(|_| row.to_string() + "\n").collect::<String>();
    for sink in [dev_full(), gone_reader()] {
        let mut child = Command::new(VERDICT_RULE)
            .arg("evaluate-jsonl")
            .stdin(Stdio::piped())
            .stdout(sink)
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(payload.as_bytes())
            .expect("write");
        let output = child.wait_with_output().expect("wait");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(
            stderr.starts_with("Traceback (most recent call last):\n"),
            "{stderr}"
        );
    }
}
