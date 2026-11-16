//! Scenario checks of the clauses ported against the `policy_selftest` tripwires (claims_selftest.rs): every clause
//! passes on the real tree, and every clause turns RED with its exact failure when the guard it drives is replaced
//! by a mutant (a tripwire that never fires / always fires, a scanner that is blind / over-eager, a parity predicate
//! that edits its input) or when the tracked registry it reads loses a material atom. No Python runs.
use agent_runtime_support::pyvalue::PyValue;
use policy_predicates::claims_selftest as st;
use policy_predicates::support::Fail;
use policy_selftest::tripwires as tw;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

fn err(r: Result<(), Fail>) -> String {
    match r {
        Ok(()) => "OK".into(),
        Err(f) => format!("{}: {}", f.kind, f.message),
    }
}

fn never(_: &Path) -> tw::R<()> {
    Ok(())
}
fn always(_: &Path) -> tw::R<()> {
    Err(tw::Fail::Selftest("mutant".into()))
}
fn blind(_: &Path) -> Vec<String> {
    Vec::new()
}
fn eager(_: &Path) -> Vec<String> {
    vec!["x".into()]
}
/// A parity predicate that "auto-corrects": it answers truthfully, then touches the specialised constitution.
fn editing(root: &Path) -> Result<PyValue, Fail> {
    let res = st::parity_evaluate(root)?;
    let spec = root.join(".claude/rules/strategy.topology.md");
    if let Ok(t) = std::fs::read_to_string(&spec) {
        std::fs::write(&spec, format!("{t}\n")).unwrap();
    }
    Ok(res)
}

#[test]
fn real_tree_and_registry_mutation() {
    std::env::set_var("POLICY_PREDICATES_REPO", repo());
    for (id, f) in [
        ("BRANCH_CONSTITUTION_LAYERING.C4", st::predicate_BRANCH_CONSTITUTION_LAYERING_C4 as fn() -> _),
        ("BRANCH_CONSTITUTION_LAYERING.C5", st::predicate_BRANCH_CONSTITUTION_LAYERING_C5),
        ("ROOT_SURFACE_REGISTRY.C1", st::predicate_ROOT_SURFACE_REGISTRY_C1),
        ("GIT_SINGLE_AUTHORITY.C1", st::predicate_GIT_SINGLE_AUTHORITY_C1),
        ("GIT_SINGLE_AUTHORITY.C2", st::predicate_GIT_SINGLE_AUTHORITY_C2),
    ] {
        assert_eq!(err(f()), "OK", "{id} must pass on the real tree");
    }
    // ROOT_SURFACE_REGISTRY.C1 reads the real registry: drop the `tasks` tombstone in a copy.
    let tmp = std::env::temp_dir().join(format!("pp-st-reg-{}", std::process::id()));
    let reg_rel = ".claude/policies/root_registry.json";
    std::fs::create_dir_all(tmp.join(".claude/policies")).unwrap();
    let text = std::fs::read_to_string(repo().join(reg_rel)).unwrap();
    assert!(text.contains("\"tasks\""), "mutation anchor missing");
    std::fs::write(tmp.join(reg_rel), text.replacen("\"tasks\"", "\"tasks-renamed\"", 1)).unwrap();
    std::env::set_var("POLICY_PREDICATES_REPO", &tmp);
    let got = err(st::predicate_ROOT_SURFACE_REGISTRY_C1());
    std::env::set_var("POLICY_PREDICATES_REPO", repo());
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(got, "PredicateFailure: 폐지된 `tasks/` 가 tombstone 으로 남아 있어야 한다");
}

#[test]
fn git_single_authority_c1_mutants() {
    assert_eq!(err(st::git_single_authority_c1_cycle(never)), "PredicateFailure: a *.bak working-tree copy must fail tripwire (1)");
    assert_eq!(
        err(st::git_single_authority_c1_cycle(always)),
        "PredicateFailure: a clean canonical fixture repo must pass tripwire (1) -- GREEN baseline"
    );
}

#[test]
fn git_single_authority_c2_mutants() {
    assert_eq!(
        err(st::git_single_authority_c2_cycle(never)),
        "PredicateFailure: a tracked ledger restating the git blob sha1 of a tracked blob must fail tripwire (2)"
    );
    assert_eq!(
        err(st::git_single_authority_c2_cycle(always)),
        "PredicateFailure: a fixture repo with no transcribed digest must pass tripwire (2) -- GREEN baseline"
    );
}

#[test]
fn root_surface_registry_c1_mutants() {
    assert_eq!(err(st::root_surface_registry_c1_cycle(never)), "PredicateFailure: 미등재 루트 추적물은 tripwire ⑦ 를 RED 로 만들어야 한다");
    assert_eq!(err(st::root_surface_registry_c1_cycle(always)), "PredicateFailure: 등록부와 인덱스가 일치하면 tripwire ⑦ 는 GREEN 이어야 한다");
}

#[test]
fn layering_c4_mutants() {
    assert_eq!(err(st::layering_c4_cycle(blind, "Ray 워커")), "PredicateFailure: 공통층의 토폴로지 전용 어휘가 발화하지 않았다");
    assert_eq!(err(st::layering_c4_cycle(eager, "Ray 워커")), "PredicateFailure: 음성대조 실패 -- 어휘가 없는데 발화했다");
}

#[test]
fn layering_c5_mutants() {
    assert_eq!(
        err(st::layering_c5_cycle(st::parity_evaluate, never)),
        "PredicateFailure: tripwire 가 불일치에서 fail-closed 하지 않았다"
    );
    assert_eq!(
        err(st::layering_c5_cycle(editing, tw::topology_layer_parity)),
        "PredicateFailure: 술어가 파일을 고쳤다 -- 이 술어는 읽기만 해야 한다"
    );
}
