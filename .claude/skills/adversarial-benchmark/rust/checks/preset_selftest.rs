//! The program's own self-test must reproduce the original's transcript by really evaluating its checks, and must
//! fail (not pass) when the installed config, the campaign owner or the skill directory binding is missing.

#[path = "preset_common.rs"]
mod preset_common;

use preset_common::*;

const CLI: &str = env!("CARGO_BIN_EXE_bench_preset");

fn self_test(repo: &std::path::Path) -> std::process::Output {
    let mut cmd = command(CLI, repo);
    cmd.arg("--self-test");
    run(cmd, b"")
}

#[test]
fn self_test_reproduces_the_original_transcript() {
    let out = self_test(&real_repo());
    let want = std::fs::read(data_path("selftest_transcript.txt")).expect("NOT_RUN: transcript");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&want)
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn self_test_fails_without_the_installed_owner_or_config() {
    let repo = real_repo();
    // a repository with the skill but without the campaign owner: the resolution checks cannot run
    let root = scratch("selftest-no-owner");
    copy_dir(
        &repo.join(SKILL_REL).join("config"),
        &root.join(SKILL_REL).join("config"),
    );
    std::fs::create_dir_all(root.join("campaigns/_template")).unwrap();
    std::fs::copy(
        repo.join("campaigns/_template/campaign.schema.json"),
        root.join("campaigns/_template/campaign.schema.json"),
    )
    .unwrap();
    let out = self_test(&root);
    assert_ne!(out.status.code(), Some(0), "a missing owner must not pass");
    // a repository without the preset config
    let bare = scratch("selftest-no-config");
    let out = self_test(&bare);
    assert_ne!(out.status.code(), Some(0), "a missing config must not pass");
    assert!(!String::from_utf8_lossy(&out.stdout).contains("전부 통과"));
}

#[test]
fn unbound_skill_directory_fails_by_name() {
    let mut cmd = command(CLI, &real_repo());
    cmd.env_remove("BENCH_PRESET_SKILL_DIR").arg("validate");
    let out = run(cmd, b"");
    assert_ne!(out.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("BenchPresetConfigurationError"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The suite reaches the preset program as the module `bench_preset`, so a `PresetError` that ends it (a deployed
/// preset that breaks the contract) is shown as `bench_preset.PresetError`, with the whole multi-line message, and the
/// run fails without printing a check or the closing line.
#[test]
fn an_uncaught_preset_error_is_shown_with_the_module_of_the_original() {
    let repo = real_repo();
    let root = scratch("selftest-bad-preset");
    copy_dir(
        &repo.join(SKILL_REL).join("config"),
        &root.join(SKILL_REL).join("config"),
    );
    let preset = root
        .join(SKILL_REL)
        .join("config/guide-llm/default_preset.yaml");
    let text = std::fs::read_to_string(&preset).expect("NOT_RUN: preset");
    // Drop the `warmups` knob: its key line and the lines indented under it.
    let mut kept = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line == "  warmups:" {
            inside = true;
            continue;
        }
        if inside && line.starts_with("    ") {
            continue;
        }
        inside = false;
        kept.push(line);
    }
    assert!(
        kept.len() + 2 < text.lines().count(),
        "the knob block was removed"
    );
    std::fs::write(&preset, kept.join("\n") + "\n").expect("write");

    let out = self_test(&root);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "no check ran");
    let stderr = String::from_utf8(out.stderr).expect("UTF-8");
    assert!(
        stderr.starts_with(
            "Traceback (most recent call last):\n  (native: Python stack frames are not reproduced)\nbench_preset.PresetError: 프리셋 "
        ),
        "{stderr}"
    );
    assert!(
        stderr.ends_with(
            "default_preset.yaml 계약 위반 1건:\n  - knobs 키 계약 위반(누락 ['warmups'] · 미지 [])\n"
        ),
        "{stderr}"
    );
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
