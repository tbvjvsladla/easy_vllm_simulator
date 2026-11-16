//! The gmu-roles / max-num-seqs / lockset-stamp self-test layers against this crate: the CLI layer (E1-E17) against the `recipe`
//! program, the unit layer (U0-U9) over the native owners and the fake-engine layer (F1-F9) through the library's check-only
//! engine seam, each over a throwaway copy of the live skill (the same cases `selftest_gmu_roles_batch` runs on a node).

use recipe_runtime::gmu_roles_selftest::{cli_layer, engine_layer};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::recipe_cmd::gmu_unit_layer;
use std::path::{Path, PathBuf};

/// <repo>/.claude/skills/vllm-recipe-explorer/rust/runtime: the repository is five directories up.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..")
}

#[test]
fn the_cli_layer_of_the_gmu_roles_self_test_passes_against_the_rust_recipe() {
    let dir = PrivateDirectory::create("gmu-roles-checks-").expect("a private directory");
    let report = cli_layer(dir.path(), &repo(), Path::new(env!("CARGO_BIN_EXE_recipe")));
    assert!(report.lines.len() >= 30, "{}", report.lines.join("\n"));
    assert!(report.failures.is_empty(), "{}", report.lines.join("\n"));
}

#[test]
fn the_unit_layer_of_the_gmu_roles_self_test_passes_over_the_native_owners() {
    let dir = PrivateDirectory::create("gmu-roles-units-").expect("a private directory");
    let report = gmu_unit_layer(dir.path(), &repo());
    assert_eq!(report.lines.len(), 41, "{}", report.lines.join("\n"));
    assert!(report.failures.is_empty(), "{}", report.lines.join("\n"));
}

#[test]
fn the_fake_engine_layer_of_the_gmu_roles_self_test_passes() {
    let dir = PrivateDirectory::create("gmu-roles-engine-").expect("a private directory");
    let report = engine_layer(dir.path(), &repo());
    assert_eq!(report.lines.len(), 12, "{}", report.lines.join("\n"));
    assert!(report.failures.is_empty(), "{}", report.lines.join("\n"));
}
