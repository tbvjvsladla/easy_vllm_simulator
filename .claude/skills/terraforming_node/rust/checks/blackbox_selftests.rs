//! The node blackbox ports' `--self-test` output compared with the original Python self-test captured once
//! (`checks/data/selftest/<prog>.{stdout,stderr,exit_code}`, PATH without host tools, so nvidia-smi/docker/systemctl
//! are never reached). Every check label and verdict line must be the same. Scratch directories differ per run, so a
//! `/tmp/<name>` path segment is compared as `/tmp/<T>`. No Python is executed here.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap()
}

/// `/tmp/<segment>` → `/tmp/<T>` (tempfile names are random on both sides).
fn norm(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let mut rest: &str = &s;
    while let Some(i) = rest.find("/tmp/") {
        out.push_str(&rest[..i + 5]);
        rest = &rest[i + 5..];
        let end = rest
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"' || c == ')')
            .unwrap_or(rest.len());
        out.push_str("<T>");
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn check(prog: &str, exe: &str) -> Vec<String> {
    let repo = repo();
    let nb = repo.join(".claude/skills/terraforming_node/scripts/node_blackbox");
    let data = repo.join(".claude/skills/terraforming_node/rust/checks/data/selftest");
    let o = Command::new(exe)
        .arg("--self-test")
        .current_dir(&nb)
        .env("PATH", "/nonexistent-bin")
        .env("TERRAFORMING_OWNER_PATH", nb.join(format!("{prog}.py")))
        .env_remove("COLUMNS")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let rd = |n: &str| std::fs::read(data.join(format!("{prog}.{n}"))).unwrap();
    let mut bad = vec![];
    for (name, want, got) in [("stdout", rd("stdout"), &o.stdout), ("stderr", rd("stderr"), &o.stderr)] {
        let (w, g) = (norm(&want), norm(got));
        if w != g {
            let line = w
                .lines()
                .zip(g.lines())
                .position(|(a, b)| a != b)
                .map_or(format!("line count {} != {}", w.lines().count(), g.lines().count()), |i| {
                    format!(
                        "line {}: golden={:?} actual={:?}",
                        i + 1,
                        w.lines().nth(i).unwrap_or(""),
                        g.lines().nth(i).unwrap_or("")
                    )
                });
            bad.push(format!("{prog}: {name}: {line}"));
        }
    }
    let gexit: i32 = String::from_utf8_lossy(&rd("exit_code")).trim().parse().unwrap();
    if Some(gexit) != o.status.code() {
        bad.push(format!("{prog}: exit golden={gexit} actual={:?}", o.status.code()));
    }
    bad
}

#[test]
fn blackbox_self_tests_match_the_original() {
    let progs: &[(&str, &str)] = &[
        ("blackbox_eta", env!("CARGO_BIN_EXE_blackbox_eta")),
        ("blackbox_session", env!("CARGO_BIN_EXE_blackbox_session")),
        ("blackbox_collect", env!("CARGO_BIN_EXE_blackbox_collect")),
        ("blackbox_thermal", env!("CARGO_BIN_EXE_blackbox_thermal")),
        ("blackbox_events", env!("CARGO_BIN_EXE_blackbox_events")),
        ("regen_envelope", env!("CARGO_BIN_EXE_regen_envelope")),
        ("agent_guard", env!("CARGO_BIN_EXE_agent_guard")),
    ];
    let mut bad = vec![];
    for (p, exe) in progs {
        bad.extend(check(p, exe));
    }
    assert!(bad.is_empty(), "self-test mismatches:\n{}", bad.join("\n"));
}
