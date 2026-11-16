//! Negative scenarios of the recipe-explorer clauses (`src/claims_recipe.rs`; nothing here runs Python):
//!   * every clause PASSES on the real tree;
//!   * one RED mutation per clause, applied to a copy of the tracked tree (one copy, each mutation reverted after its run),
//!     makes the targeted clause fail with exactly the expected `<Class>: <message>`;
//!   * the two clauses whose inputs are only the owner library (C4 gate machine, KV C3 transfer) are run on another
//!     machine through their scenario functions and must fail with the expected requirement.
//! One sequential test: the predicates read the repository root from `POLICY_PREDICATES_REPO`, and C8 sets `PATH`.
use policy_predicates::claims::{self, Impl};
use policy_predicates::claims_recipe as rx;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn run_clause(clause: &str) -> Option<String> {
    let e = claims::predicates().into_iter().find(|e| e.clause_id == clause).unwrap();
    match e.imp {
        Impl::Ported(f) => f().err().map(|x| format!("{}: {}", x.kind, x.message)),
        Impl::NotPorted(_) => panic!("{clause} is not ported"),
    }
}

fn tracked_copy(dst: &Path) {
    let out = Command::new("git").arg("-C").arg(repo()).args(["ls-files", "-z"]).output().unwrap();
    for rel in String::from_utf8(out.stdout).unwrap().split('\0').filter(|r| !r.is_empty()) {
        let src = repo().join(rel);
        match std::fs::symlink_metadata(&src) {
            Ok(m) if m.is_file() => {}
            _ => continue,
        }
        let d = dst.join(rel);
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        std::fs::copy(&src, &d).unwrap();
    }
    for target in [".claude/skills/upstream-version-watch/rust/runtime/target", ".claude/skills/vllm-recipe-explorer/rust/runtime/target"] {
        let src = repo().join(target);
        if src.is_dir() {
            std::os::unix::fs::symlink(&src, dst.join(target)).unwrap();
        }
    }
}

const CLAUSES: [&str; 9] = [
    "HOST_SAFETY_LAYERED_DEFENSE.C1",
    "HOST_SAFETY_LAYERED_DEFENSE.C4",
    "HOST_SAFETY_LAYERED_DEFENSE.C8",
    "MODEL_ACQUISITION_TERNARY_GATE.C3",
    "MODEL_ACQUISITION_TERNARY_GATE.C5",
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C1",
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C2",
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C3",
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C4",
];

const RT: &str = ".claude/skills/vllm-recipe-explorer/rust/runtime/src";

/// (clause, file, old, new, expected failure)
fn mutations() -> Vec<(&'static str, String, &'static str, &'static str, &'static str)> {
    vec![
        ("HOST_SAFETY_LAYERED_DEFENSE.C1", format!("{RT}/run_trial_session.rs"), "        .arg(container)\n", "        .arg(\"fixed\")\n",
         "PredicateFailure: watchdog Popen must bind the resolved script and this trial's container identity"),
        ("HOST_SAFETY_LAYERED_DEFENSE.C1", ".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh".into(), "THRESH_MIB=\"${2:-10240}\"", "THRESH_MIB=\"${2:-4096}\"",
         "PredicateFailure: default MemAvailable threshold must be 10240 MiB"),
        ("HOST_SAFETY_LAYERED_DEFENSE.C8", format!("{RT}/cleanup_docker.rs"), "Some(\"dry-run 표의 항목을 실제 삭제(사람 승인 후)\")", "Some(\"dry-run 표의 항목을 실제 삭제\")",
         "PredicateFailure: predicate requirement failed at original line 450"),
        ("HOST_SAFETY_LAYERED_DEFENSE.C8", format!("{RT}/cleanup_cli.rs"), "pub fn default_repo()", "fn _tick() { std::thread::sleep(std::time::Duration::from_secs(3600)); }\npub fn default_repo()",
         "PredicateFailure: no scheduling library may be imported -- this routine is never periodic"),
        ("MODEL_ACQUISITION_TERNARY_GATE.C3", format!("{RT}/model_card.rs"), "pub fn crosscheck(\n    model_dir: &PyValue,", "pub fn crosscheck(\n    model_dir: &PyValue,\n    model_source: &PyValue,",
         "PredicateFailure: predicate requirement failed at original line 1164"),
        ("MODEL_ACQUISITION_TERNARY_GATE.C5", ".claude/skills/vllm-recipe-explorer/SKILL.md".into(), "사전추정 결과(대략 GiB)를 사용자에게 함께 제시할 것", "사전추정 결과를 참고할 것",
         "PredicateFailure: the doc must mandate presenting the estimate (in GiB) to the user at approval time"),
        ("KV_ABSOLUTE_CLAMP_PORTABILITY.C1", format!("{RT}/recipe_cmd.rs"), "            7,\n            \"config.target_gpu 정의됨 — Phase-1(generate)", "            3,\n            \"config.target_gpu 정의됨 — Phase-1(generate)",
         "PredicateFailure: predicate requirement failed at original line 1278"),
        ("KV_ABSOLUTE_CLAMP_PORTABILITY.C2", ".claude/skills/wiki-desk/reference/references.md".into(), "sm_120)\n- per-card VRAM (GiB): 96", "sm_120)\n- per-card VRAM (GiB): 48",
         "PredicateFailure: discrete GPU: no hard clamp, per-card VRAM from references.md"),
        ("KV_ABSOLUTE_CLAMP_PORTABILITY.C4", format!("{RT}/recipe_cmd.rs"), "    let key = gpu_model.trim().to_lowercase();\n", "    let key = gpu_model.trim().to_lowercase(); // manifest.yaml\n",
         "PredicateFailure: _lookup_gpu_spec must never read manifest state"),
        ("KV_ABSOLUTE_CLAMP_PORTABILITY.C4", format!("{RT}/recipe_cmd.rs"), "    let (man, _) = read_manifest(root)?;\n    if starts_with(&truthy_get(&man, \"topology\"), \"single\") {", "    let man = empty_dict();\n    if starts_with(&truthy_get(&man, \"topology\"), \"single\") {",
         "PredicateFailure: _target_tp must read manifest state (node_count) -- its contrast with resolve_target_gpu_budget's manifest-free source is the proof of a deliberate split, not an accidental absence of manifest access anywhere in the module"),
    ]
}

#[test]
fn recipe_clauses() {
    std::env::set_var("POLICY_PREDICATES_REPO", repo());
    for c in CLAUSES {
        assert_eq!(run_clause(c), None, "{c} must pass on the real tree");
    }
    // the owner-library-only halves on another machine
    assert_eq!(
        rx::host_safety_c4_scenario("/nonexistent/vllm-drop-caches", 10_000_000).err().map(|f| format!("{}: {}", f.kind, f.message)).as_deref(),
        Some("PredicateFailure: refusal must be preserved even when the drop helper is absent")
    );
    assert_eq!(
        rx::kv_c3_scenario(50.0, 0.9, 20.0).err().map(|f| format!("{}: {}", f.kind, f.message)).as_deref(),
        Some("PredicateFailure: tp_divisor=1: undivided host weights/overhead must exceed the target budget for this required KV -- if this doesn't fail, the transfer isn't using the real host measurement")
    );

    let root = std::env::temp_dir().join(format!("pp-recipe-checks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    tracked_copy(&root);
    std::env::set_var("POLICY_PREDICATES_REPO", &root);
    for (clause, file, old, new, want) in mutations() {
        let path = root.join(&file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(old), "{clause}: mutation anchor missing in {file}");
        std::fs::write(&path, text.replacen(old, new, 1)).unwrap();
        let got = run_clause(clause);
        std::fs::write(&path, &text).unwrap();
        assert_eq!(got.as_deref(), Some(want), "{clause} mutation of {file}");
        assert_eq!(run_clause(clause), None, "{clause} must pass again after the revert");
    }
    std::env::set_var("POLICY_PREDICATES_REPO", repo());
    let _ = std::fs::remove_dir_all(&root);
}
