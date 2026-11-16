//! `policy-audit`: the Rust port of `.claude/policies/runtime/policy_registry.py` (registry validator/auditor) and
//! `.claude/policies/runtime/harness_verify.py` (the three-stage completion gate). See README.md for the public API other
//! packages use.

pub mod arch;
pub mod argparse;
pub mod cli;
pub mod harness;
pub mod pathutil;
pub mod pydate;
pub mod pyids;
pub mod registry;
pub mod selftest;
pub mod value;

pub use pydate::Date;
pub use registry::{
    as_of_state_violations, check_removal_eligibility, check_removal_full, citation_violations, claim_binding_violations,
    clause_executable_evidence_violations, compute_contract_tree_sha256, evaluate_lifecycle, evidence_structural_violations,
    git_available, resolve_evidence_path, safe_load_json_object, scan_policy_citations, schema_meta_violations, schema_violations,
    Violation,
};
pub use arch::arch_variant_contract_violations;
pub use pyids::extract_assertion_ids;

/// `policy_registry._self_test()` over a repository root (prints `[policy_registry] --self-test PASS`, returns 0).
pub fn self_test(repo_root: &std::path::Path) -> i32 {
    selftest::self_test(repo_root)
}
