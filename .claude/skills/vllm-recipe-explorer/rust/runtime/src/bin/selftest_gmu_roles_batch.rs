//! `selftest_gmu_roles_batch`: the unit (U0-U9), CLI (E1-E17) and fake-engine (F1-F9) layers of gmu role separation, max-num-seqs derivation and lockset stamping,
//! run against the sibling installed `recipe` program over a throwaway copy of the owning skill (see `gmu_roles_selftest`).
use recipe_runtime::anchor::process_skill_root;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let skill = match process_skill_root().ok().and_then(|root| root.to_utf8()) {
        Some(root) => PathBuf::from(root),
        None => {
            eprintln!(
                "[selftest_gmu_roles_batch] ERROR cannot derive the skill root of this program"
            );
            return ExitCode::from(2);
        }
    };
    // <repo>/.claude/skills/<skill>: the repository is three directories above the skill root.
    let repo = skill.join("../../..");
    let recipe = std::env::current_exe()
        .map(|exe| exe.with_file_name("recipe"))
        .unwrap_or_else(|_| PathBuf::from("recipe"));
    let status = recipe_runtime::gmu_roles_selftest::main_with(&repo, &recipe);
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
