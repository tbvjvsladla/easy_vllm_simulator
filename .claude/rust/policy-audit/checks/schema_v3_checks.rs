use agent_runtime_support::pyvalue::{PyDict, PyValue};
use policy_audit::{safe_load_json_object, schema_violations, Violation};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn schema() -> PyValue {
    safe_load_json_object(&repo().join(".claude/schemas/policy-registry.schema.json"), "SCHEMA").expect("tracked schema")
}

fn registry() -> PyValue {
    let text = std::fs::read_to_string(repo().join(".claude/policies/registry.yaml")).expect("tracked registry");
    agent_runtime_support::pyjson::loads(&text).expect("registry JSON")
}

fn top(doc: &mut PyValue) -> &mut PyDict {
    let PyValue::Dict(top) = doc else { panic!("the registry is an object") };
    top
}

fn first_policy(doc: &mut PyValue) -> &mut PyDict {
    let Some(PyValue::List(policies)) = top(doc).get_mut("policies") else { panic!("a policies list") };
    let Some(PyValue::Dict(policy)) = policies.first_mut() else { panic!("a first policy object") };
    policy
}

fn shown(violations: &[Violation]) -> String {
    violations.iter().map(|v| format!("{} {}", v.reason_code, v.message)).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_tracked_registry_is_a_version_3_document() {
    let doc = registry();
    let PyValue::Dict(top) = &doc else { panic!("the registry is an object") };
    assert!(top.get("schema_version").is_some_and(|v| v.equals_int(&3.into())), "schema_version is not 3");
    let violations = schema_violations(&doc, &schema());
    assert!(violations.is_empty(), "{}", shown(&violations));
}

#[test]
fn version_2_fields_are_refused() {
    let review = agent_runtime_support::pyjson::loads(
        "{\"state\": \"current\", \"last_reviewed_at\": \"2026-01-15\", \"next_review_due\": \"2026-04-15\", \"reviewer\": \"r\"}",
    )
    .expect("a review object");
    for (field, value) in [("added_at", PyValue::str("2026-01-01")), ("last_reproduced", PyValue::None), ("review", review)] {
        let mut doc = registry();
        first_policy(&mut doc).set(field, value);
        let violations = schema_violations(&doc, &schema());
        assert!(
            violations.iter().any(|v| v.reason_code.starts_with("SCHEMA:") && v.message.contains(field)),
            "{field}: {}",
            shown(&violations)
        );
    }
    let mut version_2 = registry();
    top(&mut version_2).set("schema_version", PyValue::int(2));
    let violations = schema_violations(&version_2, &schema());
    assert!(violations.iter().any(|v| v.reason_code.starts_with("SCHEMA:") && v.message.contains("schema_version")), "{}", shown(&violations));
}
