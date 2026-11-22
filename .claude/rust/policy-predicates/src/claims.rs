//! `run_all_predicates()`: the exact {clause_id: predicate} mapping against the real registry, the "every predicate
//! has a direct explicit requirement" structural check, per-clause fail-closed execution and the stable JSON verdict.
//!
//! Clauses whose original drives another bundle's Python module in-process (import + call/monkeypatch) and whose
//! owner has no Rust entry point that answers the same question yet are `NotPorted`. Such a clause is NEVER reported
//! as passing: it is a failure entry (`error` starts with `NotPorted:` and names the owner it needs), so the verdict is
//! FAIL and the exit code 1 -- the same fail-closed outcome the original gives a clause that cannot be evaluated.
use crate::support::{json_str, Fail, R};
#[allow(unused_imports)]
use crate::{claims_hint as h, claims_recipe as rx, claims_selftest as st, claims_tf as tf, claims_upstream as up};
use crate::{claims_lib as l, claims_owner as o, claims_text as t};

pub enum Impl {
    Ported(fn() -> R),
    /// The owner module the clause still needs (in-process in the original).
    NotPorted(&'static str),
}

pub struct Entry {
    pub clause_id: &'static str,
    pub name: &'static str,
    pub imp: Impl,
}

macro_rules! p {
    ($cid:literal, $m:ident :: $f:ident) => {
        Entry { clause_id: $cid, name: stringify!($f), imp: Impl::Ported($m::$f) }
    };
}
#[allow(unused_macros)]
macro_rules! np {
    ($cid:literal, $f:literal, $why:expr) => {
        Entry { clause_id: $cid, name: $f, imp: Impl::NotPorted($why) }
    };
}


/// The mapping, in the original's `PREDICATES` order.
pub fn predicates() -> Vec<Entry> {
    vec![
        p!("BRANCH_CONSTITUTION_LAYERING.C1", l::predicate_BRANCH_CONSTITUTION_LAYERING_C1),
        p!("BRANCH_CONSTITUTION_LAYERING.C2", l::predicate_BRANCH_CONSTITUTION_LAYERING_C2),
        p!("BRANCH_CONSTITUTION_LAYERING.C3", t::predicate_BRANCH_CONSTITUTION_LAYERING_C3),
        p!("BRANCH_CONSTITUTION_LAYERING.C4", st::predicate_BRANCH_CONSTITUTION_LAYERING_C4),
        p!("BRANCH_CONSTITUTION_LAYERING.C5", st::predicate_BRANCH_CONSTITUTION_LAYERING_C5),
        p!("LIBRARY_GROUNDING_FAIL_CLOSED.C1", tf::predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C1),
        p!("LIBRARY_GROUNDING_FAIL_CLOSED.C2", tf::predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C2),
        p!("LIBRARY_GROUNDING_FAIL_CLOSED.C3", tf::predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C3),
        p!("LIBRARY_GROUNDING_FAIL_CLOSED.C4", tf::predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C4),
        p!("LIBRARY_GROUNDING_FAIL_CLOSED.C5", t::predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C5),
        p!("LIBRARY_GROUNDING_FAIL_CLOSED.C6", t::predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C6),
        p!("ROOT_SURFACE_REGISTRY.C1", st::predicate_ROOT_SURFACE_REGISTRY_C1),
        p!("ROOT_SURFACE_REGISTRY.C2", tf::predicate_ROOT_SURFACE_REGISTRY_C2),
        p!("ROOT_SURFACE_REGISTRY.C3", tf::predicate_ROOT_SURFACE_REGISTRY_C3),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C1", rx::predicate_HOST_SAFETY_LAYERED_DEFENSE_C1),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C2", t::predicate_HOST_SAFETY_LAYERED_DEFENSE_C2),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C3", o::predicate_HOST_SAFETY_LAYERED_DEFENSE_C3),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C4", rx::predicate_HOST_SAFETY_LAYERED_DEFENSE_C4),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C5", t::predicate_HOST_SAFETY_LAYERED_DEFENSE_C5),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C6", t::predicate_HOST_SAFETY_LAYERED_DEFENSE_C6),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C7", t::predicate_HOST_SAFETY_LAYERED_DEFENSE_C7),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C8", rx::predicate_HOST_SAFETY_LAYERED_DEFENSE_C8),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C9", t::predicate_HOST_SAFETY_LAYERED_DEFENSE_C9),
        p!("HOST_SAFETY_LAYERED_DEFENSE.C10", t::predicate_HOST_SAFETY_LAYERED_DEFENSE_C10),
        p!("VARIANT_IMAGE_BUILD_VS_SERVE_PLANE.C1", o::predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C1),
        p!("VARIANT_IMAGE_BUILD_VS_SERVE_PLANE.C2", t::predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C2),
        p!("VARIANT_IMAGE_BUILD_VS_SERVE_PLANE.C3", t::predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C3),
        p!("MODEL_TRIPLET_NO_SUB_PROPAGATION.C1", t::predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C1),
        p!("MODEL_TRIPLET_NO_SUB_PROPAGATION.C2", o::predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C2),
        p!("MODEL_TRIPLET_NO_SUB_PROPAGATION.C3", t::predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C3),
        p!("HINT_TAG_ACTIVATION_GATE.C1", h::predicate_HINT_TAG_ACTIVATION_GATE_C1),
        p!("HINT_TAG_ACTIVATION_GATE.C2", h::predicate_HINT_TAG_ACTIVATION_GATE_C2),
        p!("HINT_TAG_ACTIVATION_GATE.C3", h::predicate_HINT_TAG_ACTIVATION_GATE_C3),
        p!("HINT_TAG_ACTIVATION_GATE.C4", h::predicate_HINT_TAG_ACTIVATION_GATE_C4),
        p!("HINT_TAG_ACTIVATION_GATE.C5", h::predicate_HINT_TAG_ACTIVATION_GATE_C5),
        p!("HINT_TAG_ACTIVATION_GATE.C6", h::predicate_HINT_TAG_ACTIVATION_GATE_C6),
        p!("LAST_GOOD_ROLLBACK_ANCHOR.C1", t::predicate_LAST_GOOD_ROLLBACK_ANCHOR_C1),
        p!("LAST_GOOD_ROLLBACK_ANCHOR.C2", t::predicate_LAST_GOOD_ROLLBACK_ANCHOR_C2),
        p!("LAST_GOOD_ROLLBACK_ANCHOR.C3", h::predicate_LAST_GOOD_ROLLBACK_ANCHOR_C3),
        p!("MODEL_ACQUISITION_TERNARY_GATE.C1", o::predicate_MODEL_ACQUISITION_TERNARY_GATE_C1),
        p!("MODEL_ACQUISITION_TERNARY_GATE.C2", o::predicate_MODEL_ACQUISITION_TERNARY_GATE_C2),
        p!("MODEL_ACQUISITION_TERNARY_GATE.C3", rx::predicate_MODEL_ACQUISITION_TERNARY_GATE_C3),
        p!("MODEL_ACQUISITION_TERNARY_GATE.C4", tf::predicate_MODEL_ACQUISITION_TERNARY_GATE_C4),
        p!("MODEL_ACQUISITION_TERNARY_GATE.C5", rx::predicate_MODEL_ACQUISITION_TERNARY_GATE_C5),
        p!("KV_ABSOLUTE_CLAMP_PORTABILITY.C1", rx::predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C1),
        p!("KV_ABSOLUTE_CLAMP_PORTABILITY.C2", rx::predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C2),
        p!("KV_ABSOLUTE_CLAMP_PORTABILITY.C3", rx::predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C3),
        p!("KV_ABSOLUTE_CLAMP_PORTABILITY.C4", rx::predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C4),
        p!("RUNTIME_PATCH_NO_CARRY_FORWARD.C1", up::predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C1),
        p!("RUNTIME_PATCH_NO_CARRY_FORWARD.C2", t::predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C2),
        p!("RUNTIME_PATCH_NO_CARRY_FORWARD.C3", t::predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C3),
        p!("RUNTIME_PATCH_NO_CARRY_FORWARD.C4", up::predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C4),
        p!("SUB_SYNC_DIRTY_AUTOSAVE.C1", t::predicate_SUB_SYNC_DIRTY_AUTOSAVE_C1),
        p!("SUB_SYNC_DIRTY_AUTOSAVE.C2", t::predicate_SUB_SYNC_DIRTY_AUTOSAVE_C2),
        p!("SUB_SYNC_DIRTY_AUTOSAVE.C3", t::predicate_SUB_SYNC_DIRTY_AUTOSAVE_C3),
        p!("SUB_SYNC_DIRTY_AUTOSAVE.C4", t::predicate_SUB_SYNC_DIRTY_AUTOSAVE_C4),
        p!("SUB_GIT_LOCAL_ONLY.C1", t::predicate_SUB_GIT_LOCAL_ONLY_C1),
        p!("SUB_GIT_LOCAL_ONLY.C2", t::predicate_SUB_GIT_LOCAL_ONLY_C2),
        p!("SUB_GIT_LOCAL_ONLY.C3", t::predicate_SUB_GIT_LOCAL_ONLY_C3),
        p!("SUB_GIT_LOCAL_ONLY.C4", t::predicate_SUB_GIT_LOCAL_ONLY_C4),
        p!("TERRAFORM_FLAG_GATE.C1", tf::predicate_TERRAFORM_FLAG_GATE_C1),
        p!("TERRAFORM_FLAG_GATE.C2", tf::predicate_TERRAFORM_FLAG_GATE_C2),
        p!("TERRAFORM_FLAG_GATE.C3", tf::predicate_TERRAFORM_FLAG_GATE_C3),
        p!("TERRAFORM_FLAG_GATE.C4", o::predicate_TERRAFORM_FLAG_GATE_C4),
        p!("A2A_IDENTITY_PROOF_FAIL_CLOSED.C1", t::predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C1),
        p!("A2A_IDENTITY_PROOF_FAIL_CLOSED.C2", t::predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C2),
        p!("A2A_IDENTITY_PROOF_FAIL_CLOSED.C3", t::predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C3),
        p!("A2A_IDENTITY_PROOF_FAIL_CLOSED.C4", t::predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C4),
        p!("ARCH_WALL_VARIANT_LADDER.C1", up::predicate_ARCH_WALL_VARIANT_LADDER_C1),
        p!("ARCH_WALL_VARIANT_LADDER.C2", up::predicate_ARCH_WALL_VARIANT_LADDER_C2),
        p!("ARCH_WALL_VARIANT_LADDER.C3", up::predicate_ARCH_WALL_VARIANT_LADDER_C3),
        p!("ARCH_WALL_VARIANT_LADDER.C4", up::predicate_ARCH_WALL_VARIANT_LADDER_C4),
        p!("ARCH_WALL_VARIANT_LADDER.C5", up::predicate_ARCH_WALL_VARIANT_LADDER_C5),
        p!("GIT_SINGLE_AUTHORITY.C1", st::predicate_GIT_SINGLE_AUTHORITY_C1),
        p!("GIT_SINGLE_AUTHORITY.C2", st::predicate_GIT_SINGLE_AUTHORITY_C2),
        p!("ENV_SECRET_KEY_LEDGER.C1", h::predicate_ENV_SECRET_KEY_LEDGER_C1),
        p!("ENV_SECRET_KEY_LEDGER.C2", tf::predicate_ENV_SECRET_KEY_LEDGER_C2),
        p!("ENV_SECRET_KEY_LEDGER.C3", h::predicate_ENV_SECRET_KEY_LEDGER_C3),
        p!("ENV_SECRET_KEY_LEDGER.C4", tf::predicate_ENV_SECRET_KEY_LEDGER_C4),
    ]
}

/// The registry predicate count tripwire (`len(PREDICATES) != 79`).
pub const EXPECTED_PREDICATES: usize = 79;

/// The Rust sources of the ported predicates (the structural "direct explicit requirement" check reads them, as the
/// original reads `inspect.getsource(fn)`).
const SOURCES: [&str; 8] = [
    include_str!("claims_text.rs"),
    include_str!("claims_lib.rs"),
    include_str!("claims_owner.rs"),
    include_str!("claims_hint.rs"),
    include_str!("claims_recipe.rs"),
    include_str!("claims_selftest.rs"),
    include_str!("claims_tf.rs"),
    include_str!("claims_upstream.rs"),
];

/// Number of direct `req(` calls in the body of `pub fn <name>(` within the ported sources.
pub fn direct_requires(name: &str) -> Option<usize> {
    let head = format!("pub fn {name}(");
    for src in SOURCES {
        if let Some(start) = src.find(&head) {
            let rest = &src[start + head.len()..];
            let end = rest.find("\npub fn ").or_else(|| rest.find("\nfn ")).unwrap_or(rest.len());
            return Some(rest[..end].matches("req(").count());
        }
    }
    None
}

/// `_load_real_registry_clause_ids()`.
pub fn registry_clause_ids() -> R<std::collections::BTreeSet<String>> {
    use crate::support::*;
    let doc = json_load(".claude/policies/registry.yaml")?;
    let mut out = std::collections::BTreeSet::new();
    for p in jlist(jget(&doc, "policies")?) {
        if let Ok(clauses) = jget(&p, "clauses") {
            for c in jlist(clauses) {
                if let Some(id) = jget(&c, "clause_id").ok().and_then(jstr) {
                    out.insert(id);
                }
            }
        }
    }
    Ok(out)
}

pub struct Failure {
    pub clause_id: String,
    pub error: String,
    /// `None` for the mapping entries (they carry only clause_id + error).
    pub tails: Option<(String, String)>,
}

fn run_one(e: &Entry) -> Option<Failure> {
    let failure = |error: String| Some(Failure { clause_id: e.clause_id.to_string(), error, tails: Some((String::new(), String::new())) });
    match &e.imp {
        Impl::NotPorted(why) => failure(format!("NotPorted: {} -- this Rust runner cannot evaluate the clause; it needs {why}", e.name)),
        Impl::Ported(f) => {
            match direct_requires(e.name) {
                Some(n) if n >= 1 => {}
                _ => return failure(format!("PredicateFailure: {} has no direct explicit _require call", e.name)),
            }
            match std::panic::catch_unwind(f) {
                Ok(Ok(())) => None,
                Ok(Err(Fail { kind, message })) => failure(format!("{kind}: {message}")),
                Err(p) => {
                    let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                    failure(format!("Exception: panic: {msg}"))
                }
            }
        }
    }
}

/// Run the selected clauses (all when `only` is empty); returns (payload JSON text, exit code).
pub fn run_all(only: &[String]) -> (String, i32) {
    let entries = predicates();
    let mut failures: Vec<Failure> = Vec::new();
    match registry_clause_ids() {
        Ok(ids) => {
            let mine: std::collections::BTreeSet<String> = entries.iter().map(|e| e.clause_id.to_string()).collect();
            if entries.len() != EXPECTED_PREDICATES || mine != ids {
                failures.push(Failure { clause_id: "__mapping__".into(), error: format!("predicate/registry mismatch predicates={} registry={}", entries.len(), ids.len()), tails: None });
            }
        }
        Err(Fail { kind, message }) => {
            failures.push(Failure { clause_id: "__mapping__".into(), error: format!("{kind}: {message}"), tails: None });
        }
    }
    let names: std::collections::BTreeSet<&str> = entries.iter().map(|e| e.name).collect();
    if names.len() != entries.len() {
        failures.push(Failure { clause_id: "__mapping__".into(), error: "predicate functions must be distinct".into(), tails: None });
    }
    let mut sorted: Vec<&Entry> = entries.iter().filter(|e| only.is_empty() || only.iter().any(|o| o == e.clause_id)).collect();
    sorted.sort_by(|a, b| a.clause_id.cmp(b.clause_id));
    for e in sorted {
        if let Some(f) = run_one(e) {
            failures.push(f);
        }
    }
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"failed_count\": {},\n", failures.len()));
    if failures.is_empty() {
        out.push_str("  \"failures\": [],\n");
    } else {
        out.push_str("  \"failures\": [\n");
        let items: Vec<String> = failures.iter().map(|f| {
            let mut s = format!("    {{\n      \"clause_id\": {},\n      \"error\": {}", json_str(&f.clause_id), json_str(&f.error));
            if let Some((o, e)) = &f.tails {
                s.push_str(&format!(",\n      \"stderr_tail\": {},\n      \"stdout_tail\": {}", json_str(e), json_str(o)));
            }
            s.push_str("\n    }");
            s
        }).collect();
        out.push_str(&items.join(",\n"));
        out.push_str("\n  ],\n");
    }
    out.push_str(&format!("  \"predicate_count\": {},\n", entries.len()));
    out.push_str("  \"schema_version\": 1,\n");
    out.push_str(&format!("  \"verdict\": \"{}\"\n}}", if failures.is_empty() { "PASS" } else { "FAIL" }));
    let code = if failures.is_empty() { 0 } else { 1 };
    (out, code)
}

/// `--list`: the port status of every clause (one JSON object per line).
pub fn list() -> String {
    predicates().iter().map(|e| match &e.imp {
        Impl::Ported(_) => format!("{{\"clause_id\": {}, \"function\": {}, \"status\": \"ported\"}}", json_str(e.clause_id), json_str(e.name)),
        Impl::NotPorted(w) => format!("{{\"clause_id\": {}, \"function\": {}, \"status\": \"not_ported\", \"needs\": {}}}", json_str(e.clause_id), json_str(e.name), json_str(w)),
    }).collect::<Vec<_>>().join("\n")
}
