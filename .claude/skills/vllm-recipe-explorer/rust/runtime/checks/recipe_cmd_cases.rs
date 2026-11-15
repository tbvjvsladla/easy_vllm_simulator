//! `recipe.py estimate` port: the Flag gate runs before the config is read (exit 4 on a tree with no manifest), the unported
//! sub-commands stop with status 2 and a named message, and an empty command line is a usage error.

use agent_runtime_support::pystr::PyStr;
use recipe_runtime::recipe_cmd::recipe_main;

fn args(items: &[&str]) -> Vec<PyStr> {
    items.iter().map(|a| PyStr::new(a)).collect()
}

/// A skill root three directories below a directory that is not a git tree, so the repo root falls back to that directory
/// (the original's abspath 3-up) and holds no `output/<topology>/manifest.yaml`.
fn bare_skill_root() -> (std::path::PathBuf, PyStr) {
    let base = std::env::temp_dir().join(format!("recipe_cmd_cases_{}", std::process::id()));
    let skill = base.join("x").join("y").join("z");
    std::fs::create_dir_all(&skill).unwrap();
    let text = skill.to_str().unwrap().to_string();
    (base, PyStr::new(&text))
}

#[test]
fn flag_gate_precedes_config_load() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&args(&["estimate", "--auto", "--config", "does/not/exist.yaml"]), Some("80"), &root);
    assert_eq!(run.status, 4);
    assert!(run.stderr.to_utf8().unwrap().contains("manifest 부재"));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn unported_subcommands_stop_with_status_two() {
    let (base, root) = bare_skill_root();
    for name in ["generate", "simulate"] {
        let run = recipe_main(&args(&[name]), Some("80"), &root);
        assert_eq!(run.status, 2);
        assert!(run.stderr.to_utf8().unwrap().contains("미이식"));
    }
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn missing_command_is_a_usage_error() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&[], Some("80"), &root);
    assert_eq!(run.status, 2);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn estimate_requires_a_candidate_source() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&args(&["estimate", "--config", "c.yaml"]), Some("80"), &root);
    assert_eq!(run.status, 2);
    let _ = std::fs::remove_dir_all(base);
}
