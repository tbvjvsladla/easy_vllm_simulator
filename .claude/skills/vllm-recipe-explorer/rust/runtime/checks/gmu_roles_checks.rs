//! The gmu-roles / max-num-seqs / lockset-stamp CLI layer (E1-E17) against this crate's `recipe` program, over a throwaway
//! copy of the live skill (the same cases `selftest_gmu_roles_batch` runs on a node).

use recipe_runtime::gmu_roles_selftest::cli_layer;
use recipe_runtime::model_files::PrivateDirectory;
use std::path::Path;

#[test]
fn the_cli_layer_of_the_gmu_roles_self_test_passes_against_the_rust_recipe() {
    // <repo>/.claude/skills/vllm-recipe-explorer/rust/runtime: the repository is five directories up.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let dir = PrivateDirectory::create("gmu-roles-checks-").expect("a private directory");
    let report = cli_layer(dir.path(), &repo, Path::new(env!("CARGO_BIN_EXE_recipe")));
    assert!(report.lines.len() >= 31, "{}", report.lines.join("\n"));
    assert!(report.failures.is_empty(), "{}", report.lines.join("\n"));
}
