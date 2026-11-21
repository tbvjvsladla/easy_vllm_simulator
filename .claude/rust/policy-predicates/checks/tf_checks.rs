//! Scenario checks of the clauses ported against the terraforming_node owners (`src/claims_tf.rs`, ENV_SECRET_KEY_LEDGER C2 and C4
//! included), run without Python:
//!   * the real tree: every clause passes;
//!   * negative controls: one mutation per clause, applied to a copy of the tracked tree (Rust build outputs linked in),
//!     makes exactly that clause fail with the asserted `<Class>: <message>` prefix.
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
        let meta = match std::fs::symlink_metadata(&src) { Ok(m) => m, Err(_) => continue };
        if !meta.is_file() {
            continue;
        }
        let d = dst.join(rel);
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        std::fs::copy(&src, &d).unwrap();
    }
    for target in [".claude/skills/upstream-version-watch/rust/runtime/target", ".claude/skills/adversarial-benchmark/rust/target",
                   ".claude/skills/vllm-recipe-explorer/rust/runtime/target"] {
        let src = repo().join(target);
        if src.is_dir() {
            std::fs::create_dir_all(dst.join(target).parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&src, dst.join(target)).unwrap();
        }
    }
}

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("pp-tf-{tag}-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn run_clause(clause: &str) -> Option<String> {
    let e = claims::predicates().into_iter().find(|e| e.clause_id == clause).unwrap();
    match e.imp {
        Impl::Ported(f) => f().err().map(|x| format!("{}: {}", x.kind, x.message)),
        Impl::NotPorted(_) => panic!("{clause} is not ported"),
    }
}

const CLAUSES: [&str; 12] = [
    "MODEL_ACQUISITION_TERNARY_GATE.C4",
    "ENV_SECRET_KEY_LEDGER.C2",
    "ENV_SECRET_KEY_LEDGER.C4",
    "TERRAFORM_FLAG_GATE.C1",
    "TERRAFORM_FLAG_GATE.C2",
    "TERRAFORM_FLAG_GATE.C3",
    "LIBRARY_GROUNDING_FAIL_CLOSED.C1",
    "LIBRARY_GROUNDING_FAIL_CLOSED.C2",
    "LIBRARY_GROUNDING_FAIL_CLOSED.C3",
    "LIBRARY_GROUNDING_FAIL_CLOSED.C4",
    "ROOT_SURFACE_REGISTRY.C2",
    "ROOT_SURFACE_REGISTRY.C3",
];

/// (clause, file, old, new, expected error prefix)
const MUTATIONS: [(&str, &str, &str, &str, &str); 15] = [
    // plan_26112012 #11: the judgement prints nothing of the file; the remote probe answers by an exit code only.
    ("MODEL_ACQUISITION_TERNARY_GATE.C4", ".claude/rust/role-contract/src/env_secret.rs",
     "        Err(_) => false,\n", "        Err(error) => {\n            eprintln!(\"{error}\");\n            false\n        }\n",
     "PredicateFailure: the HF_TOKEN judgement (env_secret::hf_token_present) must print or return nothing of the file"),
    ("MODEL_ACQUISITION_TERNARY_GATE.C4", ".claude/rust/role-contract/src/scan_node_probe.rs",
     "crate::env_secret::remote_presence_test(", "format!(\"cat {}/envs/.env && echo TOK=1 # {}\", \"\", ",
     "PredicateFailure: the remote probe must test HF_TOKEN with env_secret::remote_presence_test (an exit code only)"),
    // plan_26112012 #4 (K1): the template and every reader of envs/.env name ledger keys only; the HF_TOKEN pointer holds only while
    // the rule it points to names the ledger back and passes.
    ("ENV_SECRET_KEY_LEDGER.C2", "envs/.env.example",
     "GITHUB_TOKEN=\"깃허브 PAT 토큰이라면 여기에 기재\"", "GITHUB_TOKEN=\"깃허브 PAT 토큰이라면 여기에 기재\"\nOPENAI_API_KEY=\"x\"",
     "PredicateFailure: envs/.env.example must name ledger keys only"),
    ("ENV_SECRET_KEY_LEDGER.C2", ".claude/skills/terraforming_node/rust/src/harness_permission_probe.rs",
     "starts_with(\"HINT_CLONE_DIR=\")", "starts_with(\"HINT_ROOT_DIR=\")",
     "PredicateFailure: .claude/skills/terraforming_node/rust/src/harness_permission_probe.rs reads a key outside the envs/.env ledger: ['HINT_ROOT_DIR']"),
    ("ENV_SECRET_KEY_LEDGER.C4", ".claude/policies/registry.yaml",
     "(a fixed path; ENV_SECRET_KEY_LEDGER)", "(a fixed path)",
     "PredicateFailure: MODEL_ACQUISITION_TERNARY_GATE.C4 must hold the HF_TOKEN rule over envs/.env and name ENV_SECRET_KEY_LEDGER back"),
    ("ENV_SECRET_KEY_LEDGER.C4", ".claude/rust/role-contract/src/env_secret.rs",
     "        Err(_) => false,\n", "        Err(error) => {\n            eprintln!(\"{error}\");\n            false\n        }\n",
     "PredicateFailure: the HF_TOKEN judgement (env_secret::hf_token_present) must print or return nothing of the file"),
    ("TERRAFORM_FLAG_GATE.C1", ".claude/skills/adversarial-benchmark/rust/src/scan_node_cli.rs",
     "if a.emit_manifest && gate_ok && (a.topology", "if a.emit_manifest && (a.topology",
     "PredicateFailure: the Flag-attestation emit call must be gated on gate.status=='ok'"),
    ("TERRAFORM_FLAG_GATE.C2", ".claude/skills/vllm-recipe-explorer/rust/runtime/src/recipe_cmd.rs",
     "Some(\"simulate\") => {", "Some(\"simulate\") | Some(\"lookup\") => {",
     "PredicateFailure: the live dispatch source must gate exactly {estimate, generate, simulate}"),
    ("TERRAFORM_FLAG_GATE.C3", ".claude/skills/vllm-recipe-explorer/rust/runtime/src/recipe_cmd.rs",
     "Some(\"simulate\") => {", "Some(\"simulate\") | Some(\"lookup\") => {",
     "PredicateFailure: the live dispatch source must gate exactly {estimate, generate, simulate}"),
    ("LIBRARY_GROUNDING_FAIL_CLOSED.C1", ".claude/skills/adversarial-benchmark/rust/src/campaign_write.rs",
     "doc.set(\"export\", PyValue::Dict(export));", "let _ = export;",
     "PredicateFailure: 그라운딩 기록에 export 메시지가 없다"),
    ("LIBRARY_GROUNDING_FAIL_CLOSED.C2", ".claude/skills/upstream-version-watch/scripts/single_serve_up.sh",
     "if ! bash \"$_CI\" --grounding-check; then", "if false; then",
     "PredicateFailure: .claude/skills/upstream-version-watch/scripts/single_serve_up.sh 에 진입 백스톱 호출이 없다"),
    ("LIBRARY_GROUNDING_FAIL_CLOSED.C3", ".claude/skills/adversarial-benchmark/rust/src/campaign_write.rs",
     "grounding.set(\"status\", py(\"absent\"));", "grounding.set(\"status\", PyValue::None);",
     "PredicateFailure: 셀 상태 writer 가 그라운딩 자기표시(부재 = absent)를 쓰지 않는다"),
    ("LIBRARY_GROUNDING_FAIL_CLOSED.C4", ".claude/rust/policy-publisher/src/publisher.rs",
     ".args([\"sync\", \"--apply\", \"--root\"])", ".args([\"query\", \"--root\"])",
     "PredicateFailure: 발행기 종료부가 입고를 부르지 않는다"),
    ("ROOT_SURFACE_REGISTRY.C2", ".gitignore",
     "!/campaigns/README.md\n", "",
     "PredicateFailure: 뼈대(_template/ · README.md)만 추적하고 인스턴스는 휘발이어야 한다(.gitignore)"),
    ("ROOT_SURFACE_REGISTRY.C3", "campaigns/_template/campaign.yaml",
     "\"hint_targets\": [],", "\"hint_targets\": [{\"node_id\": \"main\"}],",
     "PredicateFailure: 뼈대가 무너졌다"),
];

#[test]
fn tf_clauses() {
    std::env::set_var("POLICY_PREDICATES_REPO", repo());
    for c in CLAUSES {
        assert_eq!(run_clause(c), None, "{c} must pass on the real tree");
    }
    for (i, (clause, file, old, new, want)) in MUTATIONS.iter().enumerate() {
        let root = scratch(&format!("m{i}"));
        tracked_copy(&root);
        let path = root.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(old), "{clause}: mutation anchor missing in {file}");
        std::fs::write(&path, text.replacen(old, new, 1)).unwrap();
        std::env::set_var("POLICY_PREDICATES_REPO", &root);
        let got = run_clause(clause);
        std::env::set_var("POLICY_PREDICATES_REPO", repo());
        let _ = std::fs::remove_dir_all(&root);
        let got = got.unwrap_or_else(|| panic!("{clause}: mutation of {file} must turn the clause RED"));
        assert!(got.starts_with(want), "{clause}: {got}");
    }
}
