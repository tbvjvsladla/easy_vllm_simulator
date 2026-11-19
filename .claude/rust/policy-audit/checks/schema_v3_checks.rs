use policy_audit::{safe_load_json_object, schema_violations, Violation};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn schema() -> agent_runtime_support::pyvalue::PyValue {
    safe_load_json_object(&repo().join(".claude/schemas/policy-registry.schema.json"), "SCHEMA").expect("tracked schema")
}

fn registry_text() -> String {
    std::fs::read_to_string(repo().join(".claude/policies/registry.yaml")).expect("tracked registry")
}

fn violations_of(text: &str) -> Vec<Violation> {
    let doc = agent_runtime_support::pyjson::loads(text).expect("registry JSON");
    schema_violations(&doc, &schema())
}

fn shown(violations: &[Violation]) -> String {
    violations.iter().map(|v| format!("{} {}", v.reason_code, v.message)).collect::<Vec<_>>().join("\n")
}

fn with_first_policy_field(text: &str, field: &str) -> String {
    let at = text.find("\"policy_id\":").expect("a policy");
    format!("{}{field},\n      {}", &text[..at], &text[at..])
}

#[test]
fn the_tracked_registry_is_a_version_3_document() {
    let text = registry_text();
    assert!(text.contains("\"schema_version\": 3"), "registry.yaml is not version 3");
    let violations = violations_of(&text);
    assert!(violations.is_empty(), "{}", shown(&violations));
}

#[test]
fn version_2_fields_are_refused() {
    let text = registry_text();
    for (field, name) in [
        ("\"added_at\": \"2026-01-01\"", "added_at"),
        ("\"last_reproduced\": null", "last_reproduced"),
        ("\"review\": {\"state\": \"current\", \"last_reviewed_at\": \"2026-01-15\", \"next_review_due\": \"2026-04-15\", \"reviewer\": \"r\"}", "review"),
    ] {
        let violations = violations_of(&with_first_policy_field(&text, field));
        assert!(
            violations.iter().any(|v| v.reason_code.starts_with("SCHEMA:") && v.message.contains(name)),
            "{name}: {}",
            shown(&violations)
        );
    }
    let version_2 = text.replacen("\"schema_version\": 3", "\"schema_version\": 2", 1);
    let violations = violations_of(&version_2);
    assert!(violations.iter().any(|v| v.reason_code.starts_with("SCHEMA:") && v.message.contains("schema_version")), "{}", shown(&violations));
}
