//! The publisher's only door to the constitution completion gate and the naming shim (the original's
//! `import completion_gate as gate` and `import doc_naming`). Everything here forwards to `policy_gate`; nothing is
//! re-implemented. The loaded owners (work-manifest schema, naming shim) are process-wide, as the imports were.

use crate::fsx::Fd;
use agent_runtime_support::pyvalue::PyValue;
use agent_runtime_support::schema;
use policy_gate::completion_gate as cg;
use policy_gate::doc_naming::{DocNaming, NamingError as DnError};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub use cg::Fields as CertFields;
pub use cg::{LITE_CERT_VERDICT, OBSERVATION_ONLY_JUDGE_VERDICT, OBSERVATION_ONLY_VERDICT, PII_MACHINE_RAW_EXEMPTION_REASON, RUBRIC_AUTHORITIES};

struct Owners {
    work_schema: PyValue,
    naming: DocNaming,
}
static OWNERS: OnceLock<Owners> = OnceLock::new();

/// The import-time work of the original: load `work-manifest.schema.json` (completion_gate) and the naming shim
/// (doc_naming). `Err` = the `ImportError`/`OSError` text the interpreter would print.
pub fn load(repo: &Path) -> Result<(), String> {
    if OWNERS.get().is_some() {
        return Ok(());
    }
    let work_schema = cg::load_schema(&repo.join(".claude").join("schemas"), "work-manifest.schema.json")?;
    let naming = DocNaming::load(repo).map_err(|e| format!("{}: {}", e.class(), e.message()))?;
    let _ = OWNERS.set(Owners { work_schema, naming });
    Ok(())
}
fn owners() -> &'static Owners {
    OWNERS.get().expect("gate::load ran at startup")
}

pub fn work_schema() -> &'static PyValue {
    &owners().work_schema
}

fn path_in(v: &PyValue, keys: &[&str]) -> PyValue {
    let mut cur = v.clone();
    for k in keys {
        cur = crate::json::get(&cur, k).unwrap_or(PyValue::None);
    }
    cur
}

fn violations(r: Result<Vec<(String, String)>, schema::SchemaError>) -> Vec<(String, String)> {
    match r {
        Ok(v) => v,
        Err(e) => {
            // the owner raises on a schema it cannot read; the original would die with that exception
            eprintln!("{}: {}", e.class, e.message);
            std::process::exit(1);
        }
    }
}

/// `validate_against_schema(value, WORK_SCHEMA["definitions"][name], WORK_SCHEMA, path)`.
pub fn validate_definition(value: &PyValue, name: &str, path: &str) -> Vec<(String, String)> {
    let root = work_schema();
    violations(schema::validate_against_schema_at(value, &path_in(root, &["definitions", name]), root, path))
}
/// `validate_against_schema(value, WORK_SCHEMA["properties"][name], WORK_SCHEMA, path)`.
pub fn validate_property(value: &PyValue, name: &str, path: &str) -> Vec<(String, String)> {
    let root = work_schema();
    violations(schema::validate_against_schema_at(value, &path_in(root, &["properties", name]), root, path))
}
/// `validate_against_schema(manifest, WORK_SCHEMA, WORK_SCHEMA, "$")`.
pub fn validate_whole(manifest: &PyValue) -> Vec<(String, String)> {
    let root = work_schema();
    violations(schema::validate_against_schema_at(manifest, root, root, "$"))
}
/// `WORK_SCHEMA["properties"][...]["enum"]` choices with falsy values dropped when `drop_null`.
pub fn schema_enum(keys: &[&str], drop_falsy: bool) -> Vec<String> {
    let v = path_in(work_schema(), keys);
    match v.peel() {
        PyValue::List(items) => items
            .iter()
            .filter(|x| !drop_falsy || x.is_truthy())
            .map(|x| x.display_text().to_utf8().unwrap_or_default())
            .collect(),
        _ => vec![],
    }
}
/// `list(WORK_SCHEMA["properties"]["evidence"]["properties"].keys())`.
pub fn evidence_kinds() -> Vec<String> {
    match path_in(work_schema(), &["properties", "evidence", "properties"]).peel() {
        PyValue::Dict(d) => d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect(),
        _ => vec![],
    }
}

pub fn required_evidence_for(task_class: &str, conditions: &PyValue, verdict: Option<&str>, authority: Option<&str>) -> Vec<String> {
    let v = verdict.map(PyValue::str);
    cg::required_evidence_for(task_class, Some(conditions), v.as_ref(), authority)
}
pub fn certificate_requirement_authority(benchmark: &PyValue) -> Option<String> {
    cg::certificate_requirement_authority(benchmark)
}
pub fn parse_flat_certificate(text: &str) -> (CertFields, bool) {
    cg::parse_flat_certificate(text)
}
pub fn certificate_grade(fields: &CertFields) -> Option<&'static str> {
    cg::certificate_grade(fields)
}
pub fn certificate_run_key(fields: &CertFields) -> Option<Vec<String>> {
    cg::certificate_run_key(fields)
}
/// The mismatched field names of `_certificate_strong_identity_matches`.
pub fn certificate_strong_identity_mismatches(fields: &CertFields, identity: &PyValue) -> Vec<String> {
    cg::certificate_strong_identity_matches(fields, identity).1.into_iter().map(|x| x.to_string()).collect()
}
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    cg::find_repo_root(start)
}
pub fn is_absolute_path_string(p: &str) -> bool {
    cg::is_absolute_path_string(p)
}
pub fn lexical_components(manifest_dir: &Path, repo_root: &Path, rel: &str) -> Option<Vec<String>> {
    cg::lexical_components(manifest_dir, repo_root, rel)
}

pub struct Resolved {
    pub status: String,
    pub sha256: Option<String>,
    pub content: Option<Vec<u8>>,
}
pub fn resolve_and_stat_evidence(root: &Fd, repo_root: &Path, manifest_dir: &Path, rel: &str, expect_dir: bool, capture: bool) -> Resolved {
    let dup = match root.dup() {
        Ok(d) => d,
        Err(_) => return Resolved { status: "os_error".into(), sha256: None, content: None },
    };
    let theirs = policy_gate::fsx::Fd(dup.into_raw());
    let r = cg::resolve_and_stat_evidence(&theirs, repo_root, manifest_dir, rel, expect_dir, capture);
    Resolved { status: r.status.to_string(), sha256: r.sha256, content: r.content_bytes }
}

// ── naming ────────────────────────────────────────────────────────────────────────────────────────────────────

pub enum NamingError {
    Collision(String),
    Value(String),
}
fn naming_fatal(e: DnError) -> ! {
    // RuntimeError / transport failure: the original lets it escape as a traceback.
    eprintln!("{}: {}", e.class(), e.message());
    std::process::exit(1);
}
fn map_err(e: DnError) -> NamingError {
    match e {
        DnError::CollisionExhausted(m) => NamingError::Collision(m),
        DnError::Value(m) | DnError::SourceUnreadable(m) => NamingError::Value(m),
        other => naming_fatal(other),
    }
}

/// `doc_naming.validate_topic(topic)` (ValueError -> Err(message)).
pub fn validate_topic(topic: &str) -> Result<(), String> {
    owners().naming.validate_topic(topic).map_err(|e| match map_err(e) {
        NamingError::Collision(m) | NamingError::Value(m) => m,
    })
}
pub fn kst_tokens(value: Option<&str>) -> (String, String, String) {
    let v = value.map(PyValue::str).unwrap_or(PyValue::None);
    match owners().naming.kst_tokens(&v) {
        Ok(t) => t,
        Err(e) => naming_fatal(e),
    }
}
pub fn dated_doc_basename(kind: &str, utc: &str, topic: &str, existing: &[String]) -> Result<String, NamingError> {
    owners().naming.dated_doc_basename(kind, &PyValue::str(utc), topic, existing, "md").map_err(map_err)
}
pub fn simlog_dirname(utc: &str, topic: &str, existing: &[String]) -> Result<String, NamingError> {
    owners().naming.simlog_dirname(&PyValue::str(utc), topic, existing).map_err(map_err)
}
pub fn report_basename(slug: &str) -> Result<String, String> {
    owners().naming.report_basename(slug, "html").map_err(|e| match map_err(e) {
        NamingError::Collision(m) | NamingError::Value(m) => m,
    })
}
pub fn is_dated_doc_basename(kind: &str, name: &str) -> bool {
    match owners().naming.is_dated_doc_basename(kind, name) {
        Ok(b) => b,
        Err(e) => match map_err(e) {
            NamingError::Collision(m) | NamingError::Value(m) => {
                eprintln!("ValueError: {m}");
                std::process::exit(1);
            }
        },
    }
}
