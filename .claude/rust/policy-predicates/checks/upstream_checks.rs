//! Scenario checks of the clauses ported against the upstream-version-watch owner (claims_upstream.rs):
//! each clause passes on the real tree, and at least one mutation of an input it reads (applied to a copy of the
//! tracked tree) makes it fail with exactly the expected `<Class>: <message>`. Nothing here runs Python.
//! One sequential test: the predicates read the repository root from `POLICY_PREDICATES_REPO`.
use policy_predicates::claims::{self, Impl};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
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
    for target in [".claude/skills/upstream-version-watch/rust/runtime/target"] {
        let src = repo().join(target);
        if src.is_dir() {
            std::os::unix::fs::symlink(&src, dst.join(target)).unwrap();
        }
    }
}

fn run_clause(clause: &str) -> Option<String> {
    let e = claims::predicates().into_iter().find(|e| e.clause_id == clause).unwrap();
    match e.imp {
        Impl::Ported(f) => f().err().map(|x| format!("{}: {}", x.kind, x.message)),
        Impl::NotPorted(_) => panic!("{clause} is not ported"),
    }
}

const CLAUSES: [&str; 7] = [
    "RUNTIME_PATCH_NO_CARRY_FORWARD.C1",
    "RUNTIME_PATCH_NO_CARRY_FORWARD.C4",
    "ARCH_WALL_VARIANT_LADDER.C1",
    "ARCH_WALL_VARIANT_LADDER.C2",
    "ARCH_WALL_VARIANT_LADDER.C3",
    "ARCH_WALL_VARIANT_LADDER.C4",
    "ARCH_WALL_VARIANT_LADDER.C5",
];

/// (id, file, old, new, clause, expected failure)
const MUTATIONS: [(&str, &str, &str, &str, &str, &str); 9] = [
    ("U01-band2-configs-patch", ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh",
     "BAND2_CONFIGS=(", "BAND2_CONFIGS=(x_patch.py ",
     "RUNTIME_PATCH_NO_CARRY_FORWARD.C1", "PredicateFailure: runtime patches must remain separate from generic runner configs"),
    ("U02-build-context-model-param", ".claude/skills/upstream-version-watch/rust/runtime/src/render_template.rs",
     "pub fn build_context(manifest: &V, resolved: &V, allow_unvalidated: bool)",
     "pub fn build_context(manifest: &V, resolved: &V, model_name: &str, allow_unvalidated: bool)",
     "RUNTIME_PATCH_NO_CARRY_FORWARD.C1", "PredicateFailure: the image-tag builder must never accept a model/patch parameter -- a model-derived image would require exactly that, and its absence is what forces the stock-image, no-rebuild path"),
    ("U03-validated-keys-static-mut", ".claude/skills/upstream-version-watch/rust/runtime/src/render_template.rs",
     "pub const VALIDATED_SOURCE_BUILD_KEYS: ", "pub static mut VALIDATED_SOURCE_BUILD_KEYS: ",
     "RUNTIME_PATCH_NO_CARRY_FORWARD.C4", "PredicateFailure: the validated source-build key set must be a compile-time constant of render_template.rs -- freezing a key must be a source edit"),
    ("U04-classifier-signature", ".claude/skills/upstream-version-watch/failure_patterns.yaml",
     "  - signature: \"ModuleNotFoundError: No module named '([^']+)'\"\n    class: requirements-fixable",
     "  - signature: \"ModuleNotFoundError: No module named '([^']+)'\"\n    class: unknown-class",
     "ARCH_WALL_VARIANT_LADDER.C1", "PredicateFailure: predicate requirement failed at original line 2753"),
    ("U05-ledger-model-keyed", ".claude/policies/arch_variant_ledger.json",
     "\"image_tag\": \"easy-vllm:0.23.0-cu132-aarch64-source-sm12x\"", "\"image_tag\": \"easy-vllm:deepseek-v4-flash-source-sm12x\"",
     "ARCH_WALL_VARIANT_LADDER.C1", "PredicateFailure: predicate requirement failed at original line 2757"),
    ("U05b-ledger-subject-renamed", ".claude/policies/arch_variant_ledger.json",
     "\"deepseek-v4-flash-sm12x\": {", "\"deepseek-v4-flash-sm12x-renamed\": {",
     "ARCH_WALL_VARIANT_LADDER.C2", "KeyError: 'deepseek-v4-flash-sm12x'"),
    ("U06-ledger-subject-renamed", ".claude/policies/arch_variant_ledger.json",
     "\"deepseek-v4-flash-sm12x\": {", "\"deepseek-v4-flash-sm12x-renamed\": {",
     "ARCH_WALL_VARIANT_LADDER.C3", "KeyError: 'deepseek-v4-flash-sm12x'"),
    ("U07-provenance-gone", ".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md",
     "", "", "ARCH_WALL_VARIANT_LADDER.C4", "FileNotFoundError"),
    ("U08-ledger-subject-renamed", ".claude/policies/arch_variant_ledger.json",
     "\"deepseek-v4-flash-sm12x\": {", "\"deepseek-v4-flash-sm12x-renamed\": {",
     "ARCH_WALL_VARIANT_LADDER.C5", "KeyError: 'deepseek-v4-flash-sm12x'"),
];

#[test]
fn upstream_clauses() {
    std::env::set_var("POLICY_PREDICATES_REPO", repo());
    for c in CLAUSES {
        assert_eq!(run_clause(c), None, "{c} must pass on the real tree");
    }
    for (id, file, old, new, clause, want) in MUTATIONS {
        let root = std::env::temp_dir().join(format!("pp-up-{id}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        tracked_copy(&root);
        let path = root.join(file);
        if old.is_empty() {
            std::fs::remove_file(&path).unwrap();
        } else {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.contains(old), "{id}: mutation anchor missing");
            std::fs::write(&path, text.replacen(old, new, 1)).unwrap();
        }
        std::env::set_var("POLICY_PREDICATES_REPO", &root);
        let got = run_clause(clause);
        std::env::set_var("POLICY_PREDICATES_REPO", repo());
        let _ = std::fs::remove_dir_all(&root);
        let got = got.unwrap_or_else(|| panic!("{id}: {clause} must fail"));
        assert!(got.starts_with(want), "{id}: {clause}: got {got:?}, want prefix {want:?}");
    }
}
