//! The wiring between the installed shell scripts and the native `bench_shell`: what the scripts call, how the
//! launcher finds the program and what it does when the program is missing or cannot run. The installed skill is
//! the one `BENCH_SHELL_WIRING_ROOT` names, or else `BENCH_VERIFY_REPO_ROOT` (a missing variable fails the check; an
//! absent installation must never read as a pass).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const CALLERS: [&str; 6] = [
    "broad_search.sh",
    "judge_bench.sh",
    "lite_bench.sh",
    "max_envelope.sh",
    "run_bench.sh",
    "sweep_bench.sh",
];
const LAUNCHER: &str = "bench_shell.sh";
/// The only inline interpreter programs the six callers keep: the thermal-owner probe of `broad_search.sh` (a node
/// owner that is not part of this port) and the health probe `lite_bench.sh` runs inside the serving container.
const KEPT_INLINE: [(&str, &str); 2] = [
    ("broad_search.sh", "python3 - <<'PY'"),
    ("lite_bench.sh", "docker exec \"$CTR\" python3 -c"),
];

fn scripts_dir() -> PathBuf {
    let root = std::env::var_os("BENCH_SHELL_WIRING_ROOT")
        .or_else(|| std::env::var_os("BENCH_VERIFY_REPO_ROOT"))
        .unwrap_or_else(|| {
            panic!("NOT_RUN: BENCH_SHELL_WIRING_ROOT names no installed repository")
        });
    PathBuf::from(root).join(".claude/skills/adversarial-benchmark/scripts")
}

fn source(name: &str) -> String {
    let path = scripts_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("NOT_RUN: {}: {e}", path.display()))
}

/// The operation word after each `bench_shell.sh` (`bash "$SDIR/bench_shell.sh" <operation> ...`) of a script.
fn operations_called(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        let mut rest = line;
        while let Some(at) = rest.find("bench_shell.sh\"") {
            rest = &rest[at + "bench_shell.sh\"".len()..];
            if let Some(word) = rest.split_whitespace().next() {
                found.push(word.trim_end_matches([')', '"']).to_string());
            }
        }
    }
    found
}

#[test]
fn every_call_names_a_native_operation_and_every_operation_is_called() {
    let known: BTreeSet<String> = sweep_stop::bench_shell::operation_names()
        .map(str::to_string)
        .collect();
    let mut used = BTreeSet::new();
    for caller in CALLERS {
        for operation in operations_called(&source(caller)) {
            assert!(
                known.contains(&operation),
                "{caller} calls an operation the program does not have: {operation}"
            );
            used.insert(operation);
        }
    }
    let unused: Vec<_> = known.difference(&used).collect();
    assert!(
        unused.is_empty(),
        "native operations no installed script calls: {unused:?}"
    );
}

#[test]
fn each_caller_defines_the_directory_it_launches_from() {
    for caller in CALLERS {
        let text = source(caller);
        if operations_called(&text).is_empty() {
            continue;
        }
        let defined = text.lines().any(|line| {
            let line = line.trim_start();
            !line.starts_with('#') && line.starts_with("SDIR=")
        });
        assert!(defined, "{caller} calls \"$SDIR/bench_shell.sh\" but never assigns SDIR (set -u makes that fatal)");
    }
}

#[test]
fn the_replaced_inline_programs_are_gone() {
    for caller in CALLERS {
        let text = source(caller);
        for marker in ["python3 -c", "python3 - <<"] {
            for line in text
                .lines()
                .filter(|line| line.contains(marker) && !line.trim_start().starts_with('#'))
            {
                assert!(
                    KEPT_INLINE.iter().any(|&(name, kept)| name == caller
                        && line.contains(kept.trim_start_matches("docker exec \"$CTR\" "))),
                    "{caller} still starts an inline interpreter program: {line}"
                );
            }
        }
    }
}

fn launcher(directory: &Path, args: &[&str], bin: Option<&str>) -> (Option<i32>, String, String) {
    let mut command = Command::new("bash");
    command
        .arg(directory.join(LAUNCHER))
        .args(args)
        .env_remove("BENCH_SHELL_BIN");
    if let Some(bin) = bin {
        command.env("BENCH_SHELL_BIN", bin);
    }
    let output = command.output().expect("run the launcher");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_launcher_runs_the_override_and_refuses_a_missing_or_unrunnable_program() {
    let scripts = scripts_dir();
    let built = env!("CARGO_BIN_EXE_bench_shell");
    let (status, out, _) = launcher(&scripts, &["warm-fraction", "1", "4"], Some(built));
    assert_eq!((status, out.as_str()), (Some(0), "0.25\n"));
    let (status, out, err) = launcher(
        &scripts,
        &["warm-fraction", "1", "4"],
        Some("/nonexistent/bench_shell"),
    );
    assert_eq!(status, Some(2));
    assert!(out.is_empty());
    assert_eq!(
        err,
        "[bench_shell] ERROR Rust 실행파일이 없거나 실행할 수 없다: /nonexistent/bench_shell\n"
    );
    let directory =
        std::env::temp_dir().join(format!("bench-shell-launcher-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("scratch");
    let plain = directory.join("not-executable");
    std::fs::write(&plain, "#!/bin/sh\n").expect("file");
    let (status, _, err) = launcher(&scripts, &["max-level"], Some(plain.to_str().unwrap()));
    assert_eq!(status, Some(2));
    assert!(err.contains("실행할 수 없다"), "{err}");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn the_launcher_finds_the_installed_program_beside_the_scripts_by_default() {
    let directory =
        std::env::temp_dir().join(format!("bench-shell-default-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let scripts = directory.join("scripts");
    let release = directory.join("rust/target/release");
    std::fs::create_dir_all(&scripts).expect("scripts");
    std::fs::create_dir_all(&release).expect("release");
    std::fs::copy(scripts_dir().join(LAUNCHER), scripts.join(LAUNCHER)).expect("launcher");
    let (status, _, err) = launcher(&scripts, &["warm-fraction", "1", "4"], None);
    assert_eq!(status, Some(2), "no program installed yet");
    assert!(err.contains("rust/target/release/bench_shell"), "{err}");
    std::fs::copy(
        env!("CARGO_BIN_EXE_bench_shell"),
        release.join("bench_shell"),
    )
    .expect("install program");
    let (status, out, _) = launcher(&scripts, &["warm-fraction", "1", "4"], None);
    assert_eq!((status, out.as_str()), (Some(0), "0.25\n"));
    let _ = std::fs::remove_dir_all(&directory);
}
