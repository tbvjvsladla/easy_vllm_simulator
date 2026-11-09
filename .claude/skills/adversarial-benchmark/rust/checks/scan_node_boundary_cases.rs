//! The two boundaries of the scan-node P1 migration against the real operating system, and the process boundary of the one new
//! binary: the local clock hour against `date` in several zones, the file faults (errno and message the OS reports), and the
//! `scan_node` executable itself (bridge only; any other start is a visible control failure; the route observation is written
//! before the business and again with the outcome). None of these expectations is taken from the original: the OS is the oracle.

use role_contract::scan_node_files::Files;
use role_contract::scan_node_manifest::Clock;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use sweep_stop::scan_node_boundary::{FixedClock, LocalClock, StdFiles};

fn date_hour(zone: &str) -> String {
    let output = Command::new("date")
        .arg("+%Y%m%d%H")
        .env("TZ", zone)
        .output()
        .expect("date");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_string()
}

#[test]
fn local_clock_follows_the_system_zone_like_date() {
    for zone in [
        "Asia/Seoul",
        "UTC",
        "America/New_York",
        "Pacific/Kiritimati",
    ] {
        // the hour may roll between the two reads: compare the neighbours as well
        std::env::set_var("TZ", zone);
        let before = date_hour(zone);
        let ours = LocalClock.local_hour().to_utf8().expect("ascii");
        let after = date_hour(zone);
        assert!(
            ours == before || ours == after,
            "{zone}: LocalClock {ours} is neither date's {before} nor {after}"
        );
        assert_eq!(ours.len(), 10, "{zone}: %Y%m%d%H is ten digits: {ours}");
    }
    std::env::remove_var("TZ");
}

#[test]
fn fixed_clock_hands_back_the_literal_it_was_given() {
    let clock = FixedClock {
        hour: "2026110910".into(),
        provenance: "a literal of the check",
    };
    assert_eq!(clock.local_hour().to_utf8().as_deref(), Some("2026110910"));
    assert_eq!(clock.provenance, "a literal of the check");
}

#[test]
fn files_report_the_operating_system_faults() {
    let root = std::env::temp_dir().join(format!("scan-node-boundary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let plain = root.join("plain.yaml");
    std::fs::write(&plain, b"topology: single\n").expect("write");
    let closed = root.join("closed.yaml");
    std::fs::write(&closed, b"x").expect("write");
    std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    let directory = root.join("dir");
    std::fs::create_dir_all(&directory).expect("dir");
    let bytes = |path: &std::path::Path| path.to_str().expect("utf-8").as_bytes().to_vec();
    assert!(StdFiles.exists(&bytes(&plain)));
    assert!(
        StdFiles.exists(&bytes(&closed)),
        "stat needs no read permission"
    );
    assert!(!StdFiles.exists(&bytes(&root.join("absent"))));
    assert_eq!(
        StdFiles.read(&bytes(&plain)).expect("read"),
        b"topology: single\n"
    );
    let denied = StdFiles.read(&bytes(&closed)).expect_err("mode 000");
    assert_eq!(
        (denied.errno, denied.strerror.as_str()),
        (13, "Permission denied")
    );
    let is_dir = StdFiles.read(&bytes(&directory)).expect_err("a directory");
    assert_eq!(
        (is_dir.errno, is_dir.strerror.as_str()),
        (21, "Is a directory")
    );
    let absent = StdFiles
        .read(&bytes(&root.join("absent")))
        .expect_err("absent");
    assert_eq!(
        (absent.errno, absent.strerror.as_str()),
        (2, "No such file or directory")
    );
    std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let _ = std::fs::remove_dir_all(&root);
}

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_scan_node")
}

#[test]
fn executable_is_the_bridge_only() {
    let output = Command::new(binary())
        .env_remove("SCAN_NODE_BRIDGE")
        .output()
        .expect("spawn");
    assert_eq!(
        output.status.code(),
        Some(3),
        "a start without the bridge is a visible control failure"
    );
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("serves the P1 bridge of the Python adapter only"));
    assert!(output.stdout.is_empty());
}

#[test]
fn executable_answers_one_request_and_observes_its_route() {
    let observer =
        std::env::temp_dir().join(format!("scan-node-route-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&observer);
    let mut child = Command::new(binary())
        .env("SCAN_NODE_BRIDGE", "1")
        .env("SCAN_NODE_OWNER_PATH", "/lexical/owner/scan_node.py")
        .env("SCAN_NODE_ROUTE_OBSERVER", &observer)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"protocol":"scan-node-bridge/1","op":"valid_host","params":{"value":{"t":"str","v":"203.0.113.11"}}}"#)
        .expect("request");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    let answer = String::from_utf8(output.stdout).expect("utf-8");
    assert_eq!(
        answer.trim_end(),
        r#"{"protocol": "scan-node-bridge/1", "status": "returned", "value": {"t": "bool", "v": true}}"#
    );
    let lines: Vec<String> = std::fs::read_to_string(&observer)
        .expect("observer")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].contains(r#""event": "start""#) && lines[0].contains(r#""outcome": null"#),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(r#""event": "end""#) && lines[1].contains(r#""outcome": "returned""#),
        "{}",
        lines[1]
    );
    for line in &lines {
        assert!(line.contains(r#""route": "NativeBridge""#));
        assert!(line.contains(r#""owner_path": "/lexical/owner/scan_node.py""#));
        assert!(line.contains(r#""shape": "valid_host""#));
        assert!(line.contains(r#""pid": "#) && line.contains(r#""ppid": "#));
    }
    let _ = std::fs::remove_file(&observer);
}

#[test]
fn a_protocol_error_is_a_control_failure_not_a_python_outcome() {
    for request in [
        &b"not json"[..],
        &br#"{"protocol":"other/9","op":"valid_host","params":{}}"#[..],
        &br#"{"protocol":"scan-node-bridge/1","op":"no_such_op","params":{}}"#[..],
    ] {
        let mut child = Command::new(binary())
            .env("SCAN_NODE_BRIDGE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(request)
            .expect("request");
        let output = child.wait_with_output().expect("wait");
        assert_eq!(
            output.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(request)
        );
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("[scan_node] ERROR bridge:"));
    }
}

// ── the launcher: availability probe for the command line, never for the bridge ─────────────────────────────────────────

fn launcher() -> std::path::PathBuf {
    std::fs::canonicalize(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../terraforming_node/scripts/scan_node.sh"),
    )
    .expect("launcher")
}

fn run_launcher(env: &[(&str, &str)], stdin: Option<&[u8]>) -> std::process::Output {
    let mut command = Command::new("bash");
    command
        .arg(launcher())
        .env_remove("SCAN_NODE_BRIDGE")
        .env_remove("SCAN_NODE_VALIDATE_ONLY")
        .env_remove("SCAN_NODE_BIN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn");
    let mut input = child.stdin.take().expect("stdin");
    if let Some(bytes) = stdin {
        input.write_all(bytes).expect("request");
    }
    drop(input);
    child.wait_with_output().expect("wait")
}

const VALID_HOST_REQUEST: &[u8] =
    br#"{"protocol":"scan-node-bridge/1","op":"valid_host","params":{"value":{"t":"str","v":"203.0.113.11"}}}"#;
const VALID_HOST_ANSWER: &str = r#"{"protocol": "scan-node-bridge/1", "status": "returned", "value": {"t": "bool", "v": true}}"#;

#[test]
fn the_command_line_availability_probe_still_answers_without_business() {
    let output = run_launcher(
        &[
            ("SCAN_NODE_BIN", binary()),
            ("SCAN_NODE_VALIDATE_ONLY", "1"),
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    // a missing program is still the visible status-2 failure, probe or not
    let output = run_launcher(
        &[
            ("SCAN_NODE_BIN", "/nonexistent/scan_node"),
            ("SCAN_NODE_VALIDATE_ONLY", "1"),
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("[scan_node] ERROR"));
}

#[test]
fn a_bridge_start_reaches_the_rust_program_even_when_the_caller_exported_the_probe_variable() {
    for probe in [None, Some("1")] {
        let mut env = vec![("SCAN_NODE_BIN", binary()), ("SCAN_NODE_BRIDGE", "1")];
        if let Some(value) = probe {
            env.push(("SCAN_NODE_VALIDATE_ONLY", value));
        }
        let output = run_launcher(&env, Some(VALID_HOST_REQUEST));
        assert_eq!(
            output.status.code(),
            Some(0),
            "probe={probe:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("utf-8").trim_end(),
            VALID_HOST_ANSWER,
            "probe={probe:?}"
        );
    }
    // the ELF check still comes first in bridge mode
    let output = run_launcher(
        &[
            ("SCAN_NODE_BIN", "/nonexistent/scan_node"),
            ("SCAN_NODE_BRIDGE", "1"),
            ("SCAN_NODE_VALIDATE_ONLY", "1"),
        ],
        Some(VALID_HOST_REQUEST),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
