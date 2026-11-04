//! The PTY helper of the checks must not leak its master into children that other test threads exec.
//!
//! Mechanism (reproduced by an independent investigation of the failure `pb-pty-gone-8191`): a master created without
//! close-on-exec is inherited by any child exec'd while it is open. When the owner drops the master the terminal is
//! still alive, so a writer on the slave sees a live terminal (status 0) instead of the "master gone" EIO the check
//! means to test. These checks exercise the real thing, in one test function so that no sibling test can interfere:
//!
//! 1. a really exec'd child's descriptor table is read from `/proc`: a plain master IS inherited (positive control, so
//!    the observation is able to see a leak), the helper's master is NOT;
//! 2. the observable consequence: with a live unrelated exec'd child around, the real `parse_bench` writing an
//!    8191-byte body to a slave whose master was dropped still fails with the EIO traceback (status 1).

#[path = "common.rs"]
mod common;

use common::{command, open_pty};
use rustix::pty::{grantpt, openpt, unlockpt, OpenptFlags};
use std::fs::File;
use std::io::Write;
use std::process::{Child, Stdio};

/// A child that has exec'd (`spawn` returns after the exec succeeded) and stays alive until its stdin closes.
fn holder() -> Child {
    std::process::Command::new("sh")
        .args(["-c", "read line"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn holder")
}

fn release(mut child: Child) {
    drop(child.stdin.take());
    child.wait().expect("holder exits");
}

/// Descriptors of `pid` that refer to the terminal multiplexer (a master PTY).
fn master_descriptors(pid: u32) -> Vec<String> {
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .expect("/proc/<pid>/fd")
        .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
        .map(|target| target.to_string_lossy().to_string())
        .filter(|target| target == "/dev/ptmx")
        .collect()
}

#[test]
fn pty_master_is_not_inherited_and_a_dropped_master_stays_gone() {
    // 1a. positive control: a master without close-on-exec is inherited by a really exec'd child.
    {
        let plain = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("openpt");
        grantpt(&plain).expect("grantpt");
        unlockpt(&plain).expect("unlockpt");
        let child = holder();
        let inherited = master_descriptors(child.id());
        release(child);
        assert_eq!(
            inherited.len(),
            1,
            "the control must observe the leak it is meant to detect: {inherited:?}"
        );
    }
    // 1b. the helper's master is closed by exec.
    let pty = open_pty(24, 80);
    let child = holder();
    let inherited = master_descriptors(child.id());
    release(child);
    assert!(
        inherited.is_empty(),
        "the helper's PTY master leaked into an exec'd child: {inherited:?}"
    );
    drop(pty);

    // 2. consequence: a live unrelated child must not keep the dropped master's terminal alive.
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("pty_inheritance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let bench = dir.join("b8191.json");
    // The document stdout_boundary uses: the output body is 436 bytes plus the length of `num_prompts`.
    let body = 8191;
    let text = format!(
        "{{\"median_tpot_ms\": 20.0, \"completed\": 4, \"num_prompts\": \"{}\"}}",
        "a".repeat(body - 436)
    );
    File::create(&bench)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .expect("bench file");
    let pty = open_pty(24, 80);
    let alive = holder();
    drop(pty.master);
    let args = vec![
        "--bench-json".to_string(),
        bench.to_string_lossy().to_string(),
    ];
    let output = command(env!("CARGO_BIN_EXE_parse_bench"), None, &args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(pty.slave))
        .stderr(Stdio::piped())
        .output()
        .expect("run parse_bench");
    release(alive);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr
            .trim_end()
            .ends_with("OSError: [Errno 5] Input/output error"),
        "{stderr}"
    );
}
