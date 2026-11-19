//! The registry validator/auditor (`policy_registry.py`): violations, citation scanning, safe JSON loading, schema-meta
//! and schema-shape validation, lifecycle rules, git-native evidence admissibility, assertion ids, claim bindings,
//! contract-tree identity and removal eligibility. Every message is the original's text.

use crate::pathutil;
use crate::pyids;
use crate::value::*;
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const ENVELOPE_SCHEMA_VERSION: i64 = 2;
pub const CLAIM_BINDINGS_REL: &str = ".claude/policies/claim_bindings.json";
/// The validator source whose bytes enter the contract-tree digest (`compute_contract_tree_sha256`). The original
/// hashed its own program text (`policy_registry.py`); the validator is now this Rust module. No removal artifact is
/// recorded in the registry (no policy carries `removal_evidence`), so no stored digest depends on the old input.
pub const CONTRACT_SCRIPT_REL: &str = ".claude/rust/policy-audit/src/registry.rs";

/// Directory of the claim predicates port; a clause's dedicated predicate lives in one of its `claims*.rs` files.
pub const CLAIM_PREDICATES_SRC_DIR: &str = ".claude/rust/policy-predicates/src/";

/// `<CLAIM_PREDICATES_SRC_DIR>claims*.rs` (one path component, no traversal).
pub fn is_claim_predicate_source(path: &str) -> bool {
    match path.strip_prefix(CLAIM_PREDICATES_SRC_DIR) {
        Some(name) => name.starts_with("claims") && name.ends_with(".rs") && !name.contains('/') && !name.contains(".."),
        None => false,
    }
}
const CONTRACT_TREE_SCOPE: &str = ".claude";
const LAZY_PLACEHOLDERS: &[&str] = &["", "tbd", "todo", "unknown", "n/a", "na", "none"];
const MIN_SUBSTANTIVE_LEN: usize = 40;
const MIN_SUBSTANTIVE_WORDS: usize = 6;
const MIN_SUBSTANTIVE_DISTINCT_CHARS: usize = 8;
const TRAILING_PUNCT: &[char] = &['.', ',', ';', ':', '!', '?', '…', '。'];
pub const POLICY_ID_PATTERN: &str = "^[A-Z][A-Z0-9_]*$";
pub const CLAUSE_ID_PATTERN: &str = "^[A-Z][A-Z0-9_]*\\.C[0-9]+$";

#[derive(Clone, Debug, PartialEq)]
pub struct Violation {
    pub reason_code: String,
    pub message: String,
    pub policy_id: PyValue,
    pub clause_id: PyValue,
    pub path: PyValue,
}

impl Violation {
    pub fn new(reason_code: impl Into<String>, message: impl Into<String>) -> Self {
        Violation { reason_code: reason_code.into(), message: message.into(), policy_id: PyValue::None, clause_id: PyValue::None, path: PyValue::None }
    }
    pub fn pid(mut self, v: &PyValue) -> Self {
        self.policy_id = v.clone();
        self
    }
    pub fn cid(mut self, v: &PyValue) -> Self {
        self.clause_id = v.clone();
        self
    }
    pub fn path(mut self, v: &PyValue) -> Self {
        self.path = v.clone();
        self
    }
    pub fn path_s(mut self, p: impl AsRef<str>) -> Self {
        self.path = s(p.as_ref());
        self
    }
    /// `Violation.to_dict()` with the keys in sorted order (the CLI dumps with `sort_keys=True`).
    pub fn to_dict(&self) -> PyValue {
        let mut d = PyDict::new();
        d.insert(PyStr::new("clause_id"), self.clause_id.clone());
        d.insert(PyStr::new("message"), s(&self.message));
        d.insert(PyStr::new("path"), self.path.clone());
        d.insert(PyStr::new("policy_id"), self.policy_id.clone());
        d.insert(PyStr::new("reason_code"), s(&self.reason_code));
        PyValue::Dict(d)
    }
}

// ── regex-equivalents ─────────────────────────────────────────────────────────────────────────

/// `re.match(r"^...$")` semantics: `$` also matches before one trailing newline.
fn strip_one_nl(s: &str) -> &str {
    s.strip_suffix('\n').unwrap_or(s)
}

pub fn policy_id_match(s: &str) -> bool {
    let s = strip_one_nl(s);
    let mut it = s.chars();
    matches!(it.next(), Some('A'..='Z')) && it.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

pub fn clause_id_match(s: &str) -> bool {
    let s = strip_one_nl(s);
    let Some(dot) = s.rfind(".C") else { return false };
    let (head, tail) = (&s[..dot], &s[dot + 2..]);
    policy_id_match_no_nl(head) && !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit())
}

fn policy_id_match_no_nl(s: &str) -> bool {
    let mut it = s.chars();
    matches!(it.next(), Some('A'..='Z')) && it.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn is_citation_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || ('가'..='힣').contains(&c)
}

fn is_py_ws(c: char) -> bool {
    pyids::is_py_space(c)
}

fn line_of(chars: &[char], pos: usize) -> usize {
    chars[..pos].iter().filter(|&&c| c == '\n').count() + 1
}

/// `scan_policy_citations(text)`: `(?<![A-Za-z0-9_가-힣])policy:([^\s`)\]},]*)` occurrences as (line, token).
pub fn scan_policy_citations(text: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let lit: Vec<char> = "policy:".chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + lit.len() <= chars.len() {
        if chars[i..i + lit.len()] == lit[..] && (i == 0 || !is_citation_word(chars[i - 1])) {
            let start = i + lit.len();
            let mut j = start;
            while j < chars.len() && !(is_py_ws(chars[j]) || matches!(chars[j], '`' | ')' | ']' | '}' | ',')) {
                j += 1;
            }
            out.push((line_of(&chars, i), chars[start..j].iter().collect()));
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    out
}

/// `(?<=[A-Za-z0-9_가-힣])policy:([A-Z][A-Z0-9_]*)(?![A-Za-z0-9_-])` (backtracking: the id may shorten to satisfy the lookahead).
fn embedded_policy_citations(text: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let lit: Vec<char> = "policy:".chars().collect();
    let mut out = Vec::new();
    let mut i = 1;
    while i + lit.len() <= chars.len() {
        if chars[i..i + lit.len()] == lit[..] && is_citation_word(chars[i - 1]) {
            let start = i + lit.len();
            if start < chars.len() && chars[start].is_ascii_uppercase() {
                let mut j = start + 1;
                while j < chars.len() && (chars[j].is_ascii_uppercase() || chars[j].is_ascii_digit() || chars[j] == '_') {
                    j += 1;
                }
                // greedy, then backtrack until the lookahead holds
                let mut end = j;
                loop {
                    let next = chars.get(end).copied();
                    let ok = !matches!(next, Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-');
                    if ok {
                        break;
                    }
                    end -= 1;
                    if end == start {
                        break;
                    }
                }
                if end > start {
                    out.push((line_of(&chars, i), chars[start..end].iter().collect()));
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn split_trailing_punct(token: &str) -> (String, String) {
    let chars: Vec<char> = token.chars().collect();
    let mut end = chars.len();
    while end > 0 && TRAILING_PUNCT.contains(&chars[end - 1]) {
        end -= 1;
    }
    (chars[..end].iter().collect(), chars[end..].iter().collect())
}

fn policies_by_id(doc: &PyValue) -> Vec<(PyValue, PyValue)> {
    let mut out: Vec<(PyValue, PyValue)> = Vec::new();
    for p in or_items(get(doc, "policies")) {
        if !is_dict(&p) {
            continue;
        }
        let id = get(&p, "policy_id").clone();
        if let Some(slot) = out.iter_mut().find(|(k, _)| *k == id) {
            slot.1 = p.clone();
        } else {
            out.push((id, p.clone()));
        }
    }
    out
}

fn live_entry(by_id: &[(PyValue, PyValue)], id: &str) -> bool {
    match by_id.iter().find(|(k, _)| as_str(k).as_deref() == Some(id)) {
        Some((_, e)) => as_str(get(e, "status")).as_deref() != Some("retired"),
        None => false,
    }
}

pub fn citation_violations(text: &str, registry_doc: &PyValue, source_label: Option<&str>) -> Vec<Violation> {
    let by_id = policies_by_id(registry_doc);
    let label = opt_s(source_label);
    let mut out = Vec::new();
    for (line_no, token) in scan_policy_citations(text) {
        if token.is_empty() {
            out.push(Violation::new("EMPTY_POLICY_CITATION", format!("bare 'policy:' citation with no id at line {line_no}")).path(&label));
            continue;
        }
        let (core, _) = split_trailing_punct(&token);
        if core.is_empty() || !policy_id_match(&core) {
            out.push(
                Violation::new("MALFORMED_CITATION_SYNTAX", format!("policy: citation {} at line {line_no} is not a syntactically valid policy_id", r(&token)))
                    .pid(&s(&token))
                    .path(&label),
            );
            continue;
        }
        if !live_entry(&by_id, &core) {
            out.push(
                Violation::new("UNRESOLVED_CITATION", format!("policy: citation {} at line {line_no} does not resolve to a live (non-retired) registry entry", r(&core)))
                    .pid(&s(&core))
                    .path(&label),
            );
        }
    }
    for (line_no, token) in embedded_policy_citations(text) {
        if !live_entry(&by_id, &token) {
            out.push(
                Violation::new("UNRESOLVED_CITATION", format!("embedded citation-shaped policy id {} at line {line_no} does not resolve to a live registry entry", r(&token)))
                    .pid(&s(&token))
                    .path(&label),
            );
        }
    }
    out
}

// ── safe loading ──────────────────────────────────────────────────────────────────────────────

fn py_type_name(v: &PyValue) -> &'static str {
    v.type_name()
}

/// `safe_load_json_object(path, label)`; the Err is the original's Violation (message incl. `{path}` = `str(path)`).
pub fn safe_load_json_object(path: &Path, label: &str) -> Result<PyValue, Violation> {
    let shown = path.to_string_lossy();
    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return Err(Violation::new(format!("{label}_LOAD_ERROR"), format!("could not read {shown}: {}", pathutil::os_error_text(&e, path)))),
    };
    let text = match std::str::from_utf8(&raw) {
        Ok(t) => t,
        Err(e) => {
            return Err(Violation::new(format!("{label}_INVALID_UTF8"), format!("{shown} is not valid UTF-8: {}", pyjson::utf8_error_text(&raw, &e))));
        }
    };
    let doc = match pyjson::loads(text) {
        Ok(d) => d,
        Err(e) => {
            let msg = match &e {
                pyjson::LoadError::TooDeep { container } => {
                    format!("maximum recursion depth exceeded while decoding a JSON {container} from a unicode string")
                }
                other => other.to_string(),
            };
            if matches!(e, pyjson::LoadError::IntegerTooLong { .. }) {
                // ValueError other than JSONDecodeError escapes the original's handler; reported the same way here.
            }
            return Err(Violation::new(format!("{label}_INVALID_JSON"), format!("{shown} is not valid JSON: {msg}")));
        }
    };
    if !is_dict(&doc) {
        return Err(Violation::new(
            format!("{label}_ROOT_NOT_OBJECT"),
            format!("{shown} must parse to a JSON object at the root, got {}", py_type_name(&doc)),
        ));
    }
    Ok(doc)
}

// ── schema ────────────────────────────────────────────────────────────────────────────────────

const SUPPORTED_TYPES: &[&str] = &["object", "array", "string", "integer", "number", "boolean", "null"];
const UNSUPPORTED_KEYWORDS: &[&str] = &["oneOf", "anyOf", "not", "patternProperties", "pattern", "additionalItems"];

pub fn schema_meta_violations(schema: &PyValue, path: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(d) = dict(schema) else {
        out.push(Violation::new("SCHEMA_META_NOT_OBJECT", format!("{path}: schema node must be a JSON object, got {}", py_type_name(schema))).path_s(path));
        return out;
    };
    if path == "$" && d.is_empty() {
        out.push(
            Violation::new(
                "SCHEMA_META_EMPTY_ROOT",
                "root schema document is empty ({}) -- every keyword-presence check in the generic engine is skipped, which would silently disable validation entirely",
            )
            .path_s(path),
        );
        return out;
    }
    for kw in UNSUPPORTED_KEYWORDS {
        if d.contains_key(kw) {
            out.push(
                Violation::new("SCHEMA_META_UNSUPPORTED_KEYWORD", format!("{path}: {} is not a supported keyword of this project's Draft-07-subset engine", r(kw)))
                    .path_s(format!("{path}.{kw}")),
            );
        }
    }
    if let Some(t) = d.get("type") {
        let types: Vec<PyValue> = match t.peel() {
            PyValue::List(l) => l.clone(),
            other => vec![other.clone()],
        };
        let shape_ok = matches!(t.peel(), PyValue::Str(_) | PyValue::List(_)) && types.iter().all(|x| matches!(x.peel(), PyValue::Str(_)));
        if !shape_ok {
            out.push(Violation::new("SCHEMA_META_BAD_TYPE_KEYWORD", format!("{path}.type must be a string or a list of strings, got {}", t.repr())).path_s(format!("{path}.type")));
        } else {
            let bad: Vec<String> = types.iter().filter_map(as_str).filter(|x| !SUPPORTED_TYPES.contains(&x.as_str())).collect();
            if !bad.is_empty() {
                out.push(Violation::new("SCHEMA_META_UNSUPPORTED_TYPE_VALUE", format!("{path}.type contains unsupported value(s) {}", list_repr(&bad))).path_s(format!("{path}.type")));
            }
        }
    }
    if let Some(req) = d.get("required") {
        let ok = matches!(req.peel(), PyValue::List(l) if l.iter().all(|x| matches!(x.peel(), PyValue::Str(_))));
        if !ok {
            out.push(Violation::new("SCHEMA_META_BAD_REQUIRED", format!("{path}.required must be a list of strings, got {}", req.repr())).path_s(format!("{path}.required")));
        }
    }
    if let Some(props) = d.get("properties") {
        match dict(props) {
            None => out.push(Violation::new("SCHEMA_META_BAD_PROPERTIES", format!("{path}.properties must be an object, got {}", props.repr())).path_s(format!("{path}.properties"))),
            Some(pd) => {
                for (k, sub) in pd.iter() {
                    out.extend(schema_meta_violations(sub, &format!("{path}.properties.{}", k.to_utf8().unwrap_or_default())));
                }
            }
        }
    }
    if let Some(defs) = d.get("definitions") {
        match dict(defs) {
            None => out.push(Violation::new("SCHEMA_META_BAD_DEFINITIONS", format!("{path}.definitions must be an object, got {}", defs.repr())).path_s(format!("{path}.definitions"))),
            Some(dd) => {
                for (k, sub) in dd.iter() {
                    out.extend(schema_meta_violations(sub, &format!("{path}.definitions.{}", k.to_utf8().unwrap_or_default())));
                }
            }
        }
    }
    if let Some(items) = d.get("items") {
        out.extend(schema_meta_violations(items, &format!("{path}.items")));
    }
    if let Some(ap) = d.get("additionalProperties") {
        if is_dict(ap) {
            out.extend(schema_meta_violations(ap, &format!("{path}.additionalProperties")));
        } else if !matches!(ap.peel(), PyValue::Bool(_)) {
            out.push(
                Violation::new("SCHEMA_META_BAD_ADDITIONAL_PROPERTIES", format!("{path}.additionalProperties must be a boolean or an object, got {}", ap.repr()))
                    .path_s(format!("{path}.additionalProperties")),
            );
        }
    }
    if let Some(allof) = d.get("allOf") {
        match list(allof) {
            None => out.push(Violation::new("SCHEMA_META_BAD_ALLOF", format!("{path}.allOf must be a list, got {}", allof.repr())).path_s(format!("{path}.allOf"))),
            Some(l) => {
                for (i, sub) in l.iter().enumerate() {
                    out.extend(schema_meta_violations(sub, &format!("{path}.allOf[{i}]")));
                }
            }
        }
    }
    for kw in ["if", "then", "else"] {
        if let Some(sub) = d.get(kw) {
            out.extend(schema_meta_violations(sub, &format!("{path}.{kw}")));
        }
    }
    if let Some(rf) = d.get("$ref") {
        let ok = matches!(rf.peel(), PyValue::Str(st) if st.to_utf8().map_or(false, |t| t.starts_with("#/")));
        if !ok {
            out.push(Violation::new("SCHEMA_META_BAD_REF", format!("{path}.$ref must be a local '#/...' string, got {}", rf.repr())).path_s(format!("{path}.$ref")));
        }
    }
    if let Some(en) = d.get("enum") {
        if list(en).is_none() {
            out.push(Violation::new("SCHEMA_META_BAD_ENUM", format!("{path}.enum must be a list, got {}", en.repr())).path_s(format!("{path}.enum")));
        }
    }
    for kw in ["minLength", "minimum", "minItems"] {
        if let Some(v) = d.get(kw) {
            if !matches!(v.peel(), PyValue::Int(_) | PyValue::Float(_)) {
                out.push(Violation::new("SCHEMA_META_BAD_NUMERIC_KEYWORD", format!("{path}.{kw} must be numeric, got {}", v.repr())).path_s(format!("{path}.{kw}")));
            }
        }
    }
    out
}

pub fn schema_violations(doc: &PyValue, schema: &PyValue) -> Vec<Violation> {
    let meta = schema_meta_violations(schema, "$");
    if !meta.is_empty() {
        return meta;
    }
    match agent_runtime_support::schema::validate_against_schema(doc, schema) {
        Err(e) => vec![Violation::new("SCHEMA_ENGINE_ERROR", format!("schema validation raised an unexpected exception: {}: {}", e.class, e.message))],
        Ok(raw) => raw
            .into_iter()
            .map(|(code, msg)| {
                let field = code.split_once(':').map(|(_, f)| f.to_string()).unwrap_or_default();
                let v = Violation::new(format!("SCHEMA:{code}"), msg);
                if field.is_empty() {
                    v
                } else {
                    v.path_s(field)
                }
            })
            .collect(),
    }
}

// ── per-policy checks ─────────────────────────────────────────────────────────────────────────

fn pol_path(pid: &PyValue, tail: &str) -> String {
    format!("$.policies[{}]{tail}", py_str(pid))
}

fn sort_values(mut v: Vec<PyValue>) -> Vec<PyValue> {
    v.sort_by(|a, b| match (a.peel(), b.peel()) {
        (PyValue::Str(x), PyValue::Str(y)) => x.code_points().cmp(y.code_points()),
        _ => a.repr().cmp(&b.repr()),
    });
    v
}

pub fn duplicate_id_violations(policies: &[PyValue]) -> Vec<Violation> {
    let mut seen: Vec<PyValue> = Vec::new();
    let mut dupes: Vec<PyValue> = Vec::new();
    for p in policies {
        let pid = get(p, "policy_id").clone();
        if seen.contains(&pid) && !dupes.contains(&pid) {
            dupes.push(pid.clone());
        }
        seen.push(pid);
    }
    sort_values(dupes.into_iter().filter(truthy).collect())
        .into_iter()
        .map(|pid| {
            Violation::new("DUPLICATE_POLICY_ID", format!("policy_id {} appears more than once", pid.repr())).pid(&pid).path_s(pol_path(&pid, ".policy_id"))
        })
        .collect()
}

pub fn id_pattern_violations(policies: &[PyValue]) -> Vec<Violation> {
    let mut out = Vec::new();
    for p in policies {
        let pid = get(p, "policy_id");
        if !policy_id_match(&py_str(pid)) {
            out.push(
                Violation::new("BAD_POLICY_ID_PATTERN", format!("{} does not match {}", pid.repr(), POLICY_ID_PATTERN)).pid(pid).path_s(pol_path(pid, ".policy_id")),
            );
        }
        for c in or_items(get(p, "clauses")) {
            let cid = if is_dict(&c) { get(&c, "clause_id").clone() } else { PyValue::None };
            let cpath = format!("$.policies[{}].clauses[{}].clause_id", py_str(pid), py_str(&cid));
            if !clause_id_match(&py_str(&cid)) {
                out.push(Violation::new("BAD_CLAUSE_ID_PATTERN", format!("{} does not match {}", cid.repr(), CLAUSE_ID_PATTERN)).pid(pid).cid(&cid).path_s(cpath));
            } else if truthy(&cid) && !as_str(&cid).unwrap_or_default().starts_with(&format!("{}.", py_str(pid))) {
                out.push(
                    Violation::new("CLAUSE_ID_POLICY_MISMATCH", format!("clause_id {} is not namespaced under policy_id {}", cid.repr(), pid.repr()))
                        .pid(pid)
                        .cid(&cid)
                        .path_s(cpath),
                );
            }
        }
    }
    out
}

pub fn provenance_violations(policies: &[PyValue]) -> Vec<Violation> {
    policies
        .iter()
        .filter(|p| truthy(get(p, "origin_failure")) == truthy(get(p, "universal_invariant")))
        .map(|p| {
            let pid = get(p, "policy_id");
            Violation::new("PROVENANCE_VIOLATION", "exactly one of origin_failure/universal_invariant is required").pid(pid).path_s(pol_path(pid, ""))
        })
        .collect()
}

pub fn is_substantive(v: &PyValue) -> bool {
    let Some(text) = as_str(v) else { return false };
    let stripped = pyids::py_strip(&text);
    let norm = agent_runtime_support::pylower::python_lower(stripped.trim_matches('.'));
    if LAZY_PLACEHOLDERS.contains(&norm.as_str()) {
        return false;
    }
    if stripped.chars().count() < MIN_SUBSTANTIVE_LEN {
        return false;
    }
    // re.findall(r"[A-Za-z0-9가-힣]+")
    let mut words = 0;
    let mut in_word = false;
    for c in stripped.chars() {
        let w = c.is_ascii_alphanumeric() || ('가'..='힣').contains(&c);
        if w && !in_word {
            words += 1;
        }
        in_word = w;
    }
    if words < MIN_SUBSTANTIVE_WORDS {
        return false;
    }
    let lower = agent_runtime_support::pylower::python_lower(stripped);
    let distinct: BTreeSet<char> = lower.chars().filter(|c| !matches!(c, ' ' | '\t' | '\n')).collect();
    distinct.len() >= MIN_SUBSTANTIVE_DISTINCT_CHARS
}

pub fn remove_when_violations(policies: &[PyValue]) -> Vec<Violation> {
    policies
        .iter()
        .filter(|p| !is_substantive(get(p, "remove_when")))
        .map(|p| {
            let pid = get(p, "policy_id");
            Violation::new("LAZY_REMOVE_WHEN", "remove_when is not substantive").pid(pid).path_s(pol_path(pid, ".remove_when"))
        })
        .collect()
}

pub fn status_enum_violations(policies: &[PyValue]) -> Vec<Violation> {
    policies
        .iter()
        .filter(|p| !matches!(as_str(get(p, "status")).as_deref(), Some("active" | "candidate" | "retired")))
        .map(|p| {
            let pid = get(p, "policy_id");
            Violation::new("STATUS_VIOLATION", "status is not one of active/candidate/retired").pid(pid).path_s(pol_path(pid, ".status"))
        })
        .collect()
}

pub fn retirement_metadata_closure_violations(policies: &[PyValue]) -> Vec<Violation> {
    let mut out = Vec::new();
    let ids: Vec<PyValue> = policies.iter().map(|p| get(p, "policy_id").clone()).collect();
    let mut edges: Vec<(PyValue, PyValue)> = Vec::new();
    for p in policies {
        let pid = get(p, "policy_id");
        let status = as_str(get(p, "status"));
        if matches!(status.as_deref(), Some("active" | "candidate")) {
            for (key, label) in [("retirement_reason", "retirement_reason"), ("superseded_by", "superseded_by"), ("removal_evidence", "removal_evidence")] {
                if has(p, key) {
                    out.push(
                        Violation::new("RETIREMENT_METADATA_ON_LIVE_POLICY", format!("active/candidate entry carries a {label} key (even if null)"))
                            .pid(pid)
                            .path_s(pol_path(pid, &format!(".{key}"))),
                    );
                }
            }
        }
        if status.as_deref() == Some("retired") {
            let successor = get(p, "superseded_by");
            if truthy(successor) {
                if let Some(slot) = edges.iter_mut().find(|(k, _)| k == pid) {
                    slot.1 = successor.clone();
                } else {
                    edges.push((pid.clone(), successor.clone()));
                }
                if !ids.contains(successor) {
                    out.push(
                        Violation::new("DANGLING_SUPERSEDE", format!("superseded_by {} does not resolve to a real policy_id", successor.repr()))
                            .pid(pid)
                            .path_s(pol_path(pid, ".superseded_by")),
                    );
                }
            } else if !is_substantive(get(p, "retirement_reason")) {
                out.push(
                    Violation::new("RETIRED_WITHOUT_REASON", "retired with neither a substantive retirement_reason nor a superseded_by")
                        .pid(pid)
                        .path_s(pol_path(pid, ".retirement_reason")),
                );
            }
        }
    }
    for (start, _) in &edges {
        let mut seen: Vec<PyValue> = Vec::new();
        let mut cur = start.clone();
        while let Some((_, next)) = edges.iter().find(|(k, _)| *k == cur) {
            if seen.contains(&cur) {
                out.push(
                    Violation::new("SUPERSEDE_CYCLE", format!("superseded_by cycle detected starting at {}", start.repr()))
                        .pid(start)
                        .path_s(pol_path(start, ".superseded_by")),
                );
                break;
            }
            seen.push(cur.clone());
            cur = next.clone();
        }
    }
    out
}

// ── git-native evidence resolution ───────────────────────────────────────────────────────────

/// `_git_available(repo_root)`
pub fn git_available(repo_root: &Path) -> bool {
    if !pathutil::which("git") {
        return false;
    }
    repo_root.join(".git").exists()
}

fn git(repo_root: &Path, args: &[&str]) -> Option<std::process::Output> {
    Command::new("git").args(args).current_dir(repo_root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().ok()
}

pub fn git_inside_work_tree(repo_root: &Path) -> bool {
    if !git_available(repo_root) {
        return false;
    }
    match git(repo_root, &["rev-parse", "--is-inside-work-tree"]) {
        Some(o) => o.status.code() == Some(0) && pyids::py_strip(&String::from_utf8_lossy(&o.stdout)) == "true",
        None => false,
    }
}

fn git_staged_blob_sha(repo_root: &Path, rel: &str) -> Option<String> {
    if !git_available(repo_root) {
        return None;
    }
    let o = git(repo_root, &["ls-files", "--stage", "--", rel])?;
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let line = pyids::py_strip(&out);
    if line.is_empty() {
        return None;
    }
    let fields: Vec<&str> = line.split(is_py_ws).filter(|x| !x.is_empty()).collect();
    fields.get(1).map(|x| x.to_string())
}

fn has_symlink_component(repo_root: &Path, rel: &str) -> bool {
    let mut cur = repo_root.to_path_buf();
    for part in pathutil::parts(rel) {
        cur = cur.join(part);
        match std::fs::symlink_metadata(&cur) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    return true;
                }
            }
            Err(e) => {
                let ignored = matches!(e.raw_os_error(), Some(libc::ENOENT | libc::ENOTDIR | libc::EBADF | libc::ELOOP));
                if !ignored {
                    return true;
                }
            }
        }
    }
    false
}

/// The status names of `_EvidenceResolution` (`ok`, `absent`, `untracked`, `path_escape`, `symlink`, `missing`, `git_unavailable`).
pub fn resolve_evidence_path(repo_root: &Path, rel_path: &PyValue) -> &'static str {
    let Some(rel) = as_str(rel_path) else { return "absent" };
    if rel.is_empty() {
        return "absent";
    }
    if rel.starts_with('/') || rel.starts_with('~') {
        return "untracked";
    }
    let parts = pathutil::parts(&rel);
    if parts.is_empty() || parts.iter().any(|p| p == ".." || p == "." || p.is_empty()) {
        return "path_escape";
    }
    if has_symlink_component(repo_root, &rel) {
        return "symlink";
    }
    if !std::fs::metadata(repo_root.join(&rel)).map(|m| m.is_file()).unwrap_or(false) {
        return "missing";
    }
    if !git_inside_work_tree(repo_root) {
        return "git_unavailable";
    }
    if git_staged_blob_sha(repo_root, &rel).is_none() {
        return "untracked";
    }
    "ok"
}

pub fn resolution_reason(status: &str) -> &'static str {
    match status {
        "absent" => "EVIDENCE_PATH_ABSENT",
        "untracked" => "EVIDENCE_UNTRACKED",
        "path_escape" => "EVIDENCE_PATH_ESCAPES_REPO",
        "symlink" => "EVIDENCE_PATH_CONTAINS_SYMLINK",
        "missing" => "EVIDENCE_MISSING_FILE",
        _ => "EVIDENCE_GIT_UNAVAILABLE",
    }
}

fn clause_id_set(p: &PyValue) -> Vec<PyValue> {
    let mut out: Vec<PyValue> = Vec::new();
    for c in or_items(get(p, "clauses")) {
        if is_dict(&c) {
            let id = get(&c, "clause_id").clone();
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

pub fn evidence_structural_violations(policies: &[PyValue], repo_root: &Path) -> Vec<Violation> {
    let mut out = Vec::new();
    for p in policies {
        let pid = get(p, "policy_id");
        let clause_ids = clause_id_set(p);
        let evidence = or_items(get(p, "evidence"));
        if evidence.is_empty() {
            out.push(Violation::new("EVIDENCE_EMPTY", "no evidence entries at all").pid(pid).path_s(pol_path(pid, ".evidence")));
        }
        let mut supported: Vec<PyValue> = Vec::new();
        for e in &evidence {
            if !is_dict(e) {
                continue;
            }
            let path = get(e, "path");
            let supports = or_items(get(e, "supports"));
            let assertion_ids = or_items(get(e, "assertion_ids"));
            let status = resolve_evidence_path(repo_root, path);
            if status != "ok" {
                out.push(Violation::new(resolution_reason(status), format!("{}: {status}", path.repr())).pid(pid).path(path));
            }
            if assertion_ids.is_empty() {
                out.push(Violation::new("EVIDENCE_ASSERTION_IDS_EMPTY", "evidence entry has no assertion_ids").pid(pid).path(path));
            } else if status == "ok" {
                let rel = as_str(path).unwrap_or_default();
                match pyids::extract_assertion_ids(&repo_root.join(&rel)) {
                    None => out.push(
                        Violation::new("EVIDENCE_ASSERTION_KIND_UNSUPPORTED", format!("{} has no supported assertion-id extraction for its file kind", path.repr()))
                            .pid(pid)
                            .path(path),
                    ),
                    Some(available) => {
                        for aid in &assertion_ids {
                            let found = as_str(aid).map_or(false, |a| available.contains(&a));
                            if !found {
                                out.push(
                                    Violation::new(
                                        "EVIDENCE_ASSERTION_ID_NOT_FOUND",
                                        format!("assertion id {} is not an exact identifier found in {}", aid.repr(), path.repr()),
                                    )
                                    .pid(pid)
                                    .path(path),
                                );
                            }
                        }
                    }
                }
            }
            for cid in &supports {
                if !clause_ids.contains(cid) {
                    out.push(
                        Violation::new("EVIDENCE_DANGLING_CLAUSE_REF", format!("supports references clause_id {} which this policy does not declare", cid.repr()))
                            .pid(pid)
                            .cid(cid)
                            .path(path),
                    );
                } else if !supported.contains(cid) {
                    supported.push(cid.clone());
                }
            }
        }
        let missing: Vec<PyValue> = clause_ids.iter().filter(|c| !supported.contains(c)).cloned().collect();
        for cid in sort_values(missing) {
            out.push(
                Violation::new("CLAUSE_WITHOUT_EVIDENCE", format!("clause {} has no supporting evidence entry", cid.repr()))
                    .pid(pid)
                    .cid(&cid)
                    .path_s(format!("$.policies[{}].clauses[{}]", py_str(pid), py_str(&cid))),
            );
        }
    }
    out
}

/// Clause order follows the sorted clause ids (the original iterates a Python `set`, whose order is hash-randomized).
pub fn clause_executable_evidence_violations(policies: &[PyValue]) -> Vec<Violation> {
    let mut out = Vec::new();
    for p in policies {
        let pid = get(p, "policy_id");
        let clause_ids = sort_values(clause_id_set(p));
        let mut by_clause: Vec<(PyValue, Vec<PyValue>)> = clause_ids.iter().map(|c| (c.clone(), Vec::new())).collect();
        for e in or_items(get(p, "evidence")) {
            if !is_dict(&e) {
                continue;
            }
            for cid in or_items(get(&e, "supports")) {
                if let Some(slot) = by_clause.iter_mut().find(|(k, _)| *k == cid) {
                    slot.1.push(e.clone());
                }
            }
        }
        for (cid, entries) in &by_clause {
            let all_md = !entries.is_empty()
                && entries.iter().all(|e| {
                    let v = get(e, "path");
                    let text = if truthy(v) { py_str(v) } else { String::new() };
                    agent_runtime_support::pylower::python_lower(&text).ends_with(".md")
                });
            if all_md {
                out.push(
                    Violation::new(
                        "CLAUSE_LACKS_EXECUTABLE_EVIDENCE",
                        format!("clause {} is supported only by prose (.md) evidence -- prose cannot be the sole evidence for a clause", cid.repr()),
                    )
                    .pid(pid)
                    .cid(cid)
                    .path_s(format!("$.policies[{}].clauses[{}]", py_str(pid), py_str(cid))),
                );
            }
        }
    }
    out
}

type Tup = (PyValue, PyValue);

fn tup_sort(mut v: Vec<Tup>) -> Vec<Tup> {
    let key = |x: &PyValue| match x.peel() {
        PyValue::Str(s) => (0u8, s.code_points().to_vec(), String::new()),
        other => (1u8, Vec::new(), other.repr()),
    };
    v.sort_by(|a, b| (key(&a.0), key(&a.1)).cmp(&(key(&b.0), key(&b.1))));
    v
}

fn tup_list_repr(v: &[Tup]) -> String {
    PyValue::List(v.iter().map(|(a, b)| PyValue::Tuple(vec![a.clone(), b.clone()])).collect()).repr()
}

pub fn claim_binding_violations(policies: &[PyValue], claim_bindings: &PyValue) -> Vec<Violation> {
    let mut canonical: BTreeSet<String> = BTreeSet::new();
    for p in policies {
        if !is_dict(p) {
            continue;
        }
        for c in or_items(get(p, "clauses")) {
            if !is_dict(&c) {
                continue;
            }
            if let Some(id) = as_str(get(&c, "clause_id")) {
                if !id.starts_with("TEST_") {
                    canonical.insert(id);
                }
            }
        }
    }
    let mut out = Vec::new();
    let cb_path = ".claude/policies/claim_bindings.json";
    if !canonical.is_empty() {
        let shape_ok = match dict(claim_bindings) {
            Some(d) => {
                let ks: BTreeSet<String> = keys(d).into_iter().collect();
                ks == ["bindings", "schema_version"].iter().map(|x| x.to_string()).collect::<BTreeSet<_>>()
                    && eq_int(get(claim_bindings, "schema_version"), 1)
                    && is_dict(get(claim_bindings, "bindings"))
            }
            None => false,
        };
        if !shape_ok {
            return vec![Violation::new("CLAIM_BINDINGS_INVALID", "canonical claim_bindings must be an exact schema_version=1 bindings object").path_s(cb_path)];
        }
        let cand = dict(get(claim_bindings, "bindings")).unwrap();
        let cand_keys: BTreeSet<String> = keys(cand).into_iter().collect();
        if cand_keys != canonical {
            let missing: Vec<&String> = canonical.difference(&cand_keys).collect();
            let extra: Vec<&String> = cand_keys.difference(&canonical).collect();
            out.push(
                Violation::new(
                    "CLAIM_BINDINGS_INCOMPLETE",
                    format!("canonical binding keys must equal all clause IDs; missing={} extra={}", list_repr(missing), list_repr(extra)),
                )
                .path_s(format!("{cb_path}.bindings")),
            );
        }
        for (cid, entries) in cand.iter() {
            let cid = cid.to_utf8().unwrap_or_default();
            let mut valid = matches!(entries.peel(), PyValue::List(l) if !l.is_empty());
            if valid {
                for entry in list(entries).unwrap() {
                    let ok = match dict(entry) {
                        Some(ed) => {
                            let ks: BTreeSet<String> = keys(ed).into_iter().collect();
                            ks == ["assertion_ids", "path"].iter().map(|x| x.to_string()).collect::<BTreeSet<_>>()
                                && as_str(get(entry, "path")).map_or(false, |x| !x.is_empty())
                                && matches!(get(entry, "assertion_ids").peel(), PyValue::List(l) if !l.is_empty()
                                    && l.iter().all(|a| as_str(a).map_or(false, |x| !x.is_empty())))
                        }
                        None => false,
                    };
                    if !ok {
                        valid = false;
                        break;
                    }
                }
            }
            if !valid {
                out.push(
                    Violation::new("CLAIM_BINDINGS_INVALID", format!("clause {} has malformed binding entries", r(&cid)))
                        .cid(&s(&cid))
                        .path_s(format!("{cb_path}.bindings.{cid}")),
                );
            }
        }
    }
    let bindings: PyValue = match dict(claim_bindings) {
        Some(d) => {
            let b = d.get("bindings").cloned().unwrap_or_else(|| claim_bindings.clone());
            if is_dict(&b) {
                b
            } else {
                PyValue::Dict(PyDict::new())
            }
        }
        None => PyValue::Dict(PyDict::new()),
    };
    let bmap = dict(&bindings).unwrap();
    for p in policies {
        let pid = get(p, "policy_id");
        let clause_ids = clause_id_set(p);
        let mut actual: Vec<(PyValue, Vec<Tup>)> = clause_ids.iter().map(|c| (c.clone(), Vec::new())).collect();
        for e in or_items(get(p, "evidence")) {
            if !is_dict(&e) {
                continue;
            }
            let path = get(&e, "path").clone();
            for cid in or_items(get(&e, "supports")) {
                if let Some(slot) = actual.iter_mut().find(|(k, _)| *k == cid) {
                    for aid in or_items(get(&e, "assertion_ids")) {
                        let t = (path.clone(), aid);
                        if !slot.1.contains(&t) {
                            slot.1.push(t);
                        }
                    }
                }
            }
        }
        for cid in sort_values(clause_ids.clone()) {
            let Some(cid_s) = as_str(&cid) else { continue };
            let Some(allowed_entries) = bmap.get(&cid_s) else { continue };
            let mut allowed: Vec<Tup> = Vec::new();
            if let Some(l) = list(allowed_entries) {
                for entry in l {
                    if !is_dict(entry) {
                        continue;
                    }
                    let epath = get(entry, "path").clone();
                    for aid in or_items(get(entry, "assertion_ids")) {
                        let t = (epath.clone(), aid);
                        if !allowed.contains(&t) {
                            allowed.push(t);
                        }
                    }
                }
            }
            let act = actual.iter().find(|(k, _)| *k == cid).map(|(_, v)| v.clone()).unwrap_or_default();
            let same = act.len() == allowed.len() && act.iter().all(|t| allowed.contains(t));
            let cpath = format!("$.policies[{}].clauses[{}]", py_str(pid), cid_s);
            if !same {
                let extra = tup_sort(act.iter().filter(|t| !allowed.contains(t)).cloned().collect());
                let missing = tup_sort(allowed.iter().filter(|t| !act.contains(t)).cloned().collect());
                out.push(
                    Violation::new(
                        "CLAIM_BINDING_MISMATCH",
                        format!(
                            "clause {} evidence tuples do not exactly match claim_bindings.json -- extra={} missing={}",
                            r(&cid_s),
                            tup_list_repr(&extra),
                            tup_list_repr(&missing)
                        ),
                    )
                    .pid(pid)
                    .cid(&cid)
                    .path_s(cpath),
                );
                continue;
            }
            if !cid_s.starts_with("TEST_") {
                // The dedicated predicate is the Rust `predicate_<cid>` fn of the claim predicates port
                // (`.claude/rust/policy-predicates/src/claims*.rs`; the original required the retired
                // `claim_predicates.py`). Which `claims*.rs` file is free; that the id really is a line-start fn of
                // that file is the ordinary evidence check (EVIDENCE_ASSERTION_ID_NOT_FOUND / path resolution).
                let want_id = format!("predicate_{}", cid_s.replace('.', "_"));
                let found = act.iter().any(|(p, a)| {
                    as_str(a).as_deref() == Some(want_id.as_str()) && as_str(p).as_deref().map_or(false, is_claim_predicate_source)
                });
                if !found {
                    let dedicated = (s(&format!("{CLAIM_PREDICATES_SRC_DIR}claims*.rs")), s(&want_id));
                    let drepr = PyValue::Tuple(vec![dedicated.0.clone(), dedicated.1.clone()]).repr();
                    out.push(
                        Violation::new("CLAIM_BINDING_MISSING_DEDICATED_PREDICATE", format!("canonical clause {} lacks required dedicated predicate {drepr}", r(&cid_s)))
                            .pid(pid)
                            .cid(&cid)
                            .path_s(cpath),
                    );
                }
            }
        }
    }
    out
}

// ── lifecycle ─────────────────────────────────────────────────────────────────────────────────

pub fn policies_of(doc: &PyValue) -> Vec<PyValue> {
    or_items(get(doc, "policies")).into_iter().filter(is_dict).collect()
}

pub fn evaluate_lifecycle(doc: &PyValue, repo_root: &Path, claim_bindings: Option<&PyValue>) -> Vec<Violation> {
    let policies = policies_of(doc);
    let mut out = Vec::new();
    out.extend(duplicate_id_violations(&policies));
    out.extend(id_pattern_violations(&policies));
    out.extend(provenance_violations(&policies));
    out.extend(remove_when_violations(&policies));
    out.extend(status_enum_violations(&policies));
    out.extend(retirement_metadata_closure_violations(&policies));
    out.extend(evidence_structural_violations(&policies, repo_root));
    out.extend(clause_executable_evidence_violations(&policies));
    if let Some(cb) = claim_bindings {
        out.extend(claim_binding_violations(&policies, cb));
    }
    if policies.iter().any(|p| as_str(get(p, "policy_id")).as_deref() == Some("ARCH_WALL_VARIANT_LADDER")) {
        out.extend(crate::arch::arch_variant_contract_violations(repo_root));
    }
    out
}

// ── removal ───────────────────────────────────────────────────────────────────────────────────

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn git_contract_tree_digest(repo_root: &Path, exclude: &BTreeSet<String>) -> String {
    if !git_inside_work_tree(repo_root) {
        return "unavailable".into();
    }
    let Some(o) = git(repo_root, &["ls-tree", "-r", "-z", "HEAD", "--", CONTRACT_TREE_SCOPE]) else { return "unavailable".into() };
    if o.status.code() != Some(0) {
        return "unavailable".into();
    }
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    let mut lines: Vec<String> = Vec::new();
    for record in text.split('\0') {
        if record.is_empty() {
            continue;
        }
        let (meta, path) = record.split_once('\t').unwrap_or((record, ""));
        let fields: Vec<&str> = meta.split(is_py_ws).filter(|x| !x.is_empty()).collect();
        if fields.len() < 3 || path.is_empty() || exclude.contains(path) {
            continue;
        }
        lines.push(format!("{} {path}", fields[2]));
    }
    lines.sort();
    sha256_hex(lines.join("\n").as_bytes())
}

pub fn compute_contract_tree_sha256(doc: &PyValue, schema: &PyValue, claim_bindings: &PyValue, repo_root: &Path, selected_artifact: Option<&str>) -> String {
    let exclude: BTreeSet<String> = selected_artifact.map(|x| x.to_string()).into_iter().collect();
    let script = std::fs::read(repo_root.join(CONTRACT_SCRIPT_REL)).unwrap_or_default();
    let mut parts = vec![
        format!("registry:{}", sha256_hex(&dumps_sorted_unicode(doc))),
        format!("schema:{}", sha256_hex(&dumps_sorted_unicode(schema))),
        format!("claim_bindings:{}", sha256_hex(&dumps_sorted_unicode(claim_bindings))),
        format!("git_tree:{}", git_contract_tree_digest(repo_root, &exclude)),
        format!("policy_registry.sh:{}", sha256_hex(&script)),
    ];
    parts.sort();
    sha256_hex(parts.join("\n").as_bytes())
}

fn load_removal_artifact(repo_root: &Path, rel: &PyValue) -> Result<PyValue, &'static str> {
    let status = resolve_evidence_path(repo_root, rel);
    if status != "ok" {
        return Err(resolution_reason(status));
    }
    safe_load_json_object(&repo_root.join(as_str(rel).unwrap_or_default()), "REMOVAL_EVIDENCE_ARTIFACT").map_err(|_| "REMOVAL_EVIDENCE_ARTIFACT_UNREADABLE")
}

fn is_tree_sha(v: &PyValue) -> bool {
    match as_str(v) {
        Some(t) => {
            let t = strip_one_nl(&t);
            t.len() == 64 && t.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        }
        None => false,
    }
}

pub fn check_removal_eligibility(policy: &PyValue, repo_root: &Path, expected_tree: Option<&str>) -> Vec<Violation> {
    let pid = get(policy, "policy_id");
    let mut out = Vec::new();
    if as_str(get(policy, "status")).as_deref() != Some("retired") {
        out.push(Violation::new("REMOVAL_NOT_ELIGIBLE", "policy is not retired").pid(pid).path_s(pol_path(pid, ".status")));
        return out;
    }
    let re_v = get(policy, "removal_evidence");
    let removal = if truthy(re_v) { re_v.clone() } else { PyValue::Dict(PyDict::new()) };
    if as_str(get(&removal, "status")).as_deref() != Some("contract_e2e_green") {
        out.push(Violation::new("REMOVAL_NOT_ELIGIBLE", "removal_evidence.status is not contract_e2e_green").pid(pid).path_s(pol_path(pid, ".removal_evidence.status")));
        return out;
    }
    let path = get(&removal, "path").clone();
    let artifact = match load_removal_artifact(repo_root, &path) {
        Ok(a) => a,
        Err(code) => {
            out.push(
                Violation::new(code, "removal_evidence.path does not resolve to a readable, tracked, digest-matching JSON artifact").pid(pid).path(&path),
            );
            return out;
        }
    };
    let a_pid = get(&artifact, "policy_id");
    if a_pid != pid {
        out.push(
            Violation::new("REMOVAL_EVIDENCE_POLICY_ID_MISMATCH", format!("removal artifact policy_id {} != {}", a_pid.repr(), pid.repr())).pid(pid).path(&path),
        );
    }
    let result = get(&artifact, "result");
    if as_str(result).as_deref() != Some("PASS") {
        out.push(
            Violation::new("REMOVAL_EVIDENCE_RESULT_NOT_PASS", format!("removal artifact result {} is not 'PASS'", result.repr())).pid(pid).path(&path),
        );
    }
    let tree = get(&artifact, "tree_sha256");
    if !is_tree_sha(tree) {
        out.push(
            Violation::new("REMOVAL_EVIDENCE_DIGEST_FIELD_INVALID", format!("removal artifact tree_sha256 {} is not a well-formed 64-hex digest", tree.repr()))
                .pid(pid)
                .path(&path),
        );
    } else if let Some(exp) = expected_tree {
        let got = as_str(tree).unwrap_or_default();
        if got != exp {
            out.push(
                Violation::new(
                    "REMOVAL_EVIDENCE_TREE_MISMATCH",
                    format!("removal artifact tree_sha256 does not match the exact contract tree that was checked (expected {exp}, got {got})"),
                )
                .pid(pid)
                .path(&path),
            );
        }
    }
    out
}

pub fn check_removal_full(doc: &PyValue, policy_id: &str, schema: &PyValue, repo_root: &Path, claim_bindings: &PyValue) -> Vec<Violation> {
    let shape = schema_violations(doc, schema);
    if !shape.is_empty() {
        return shape;
    }
    let lifecycle = evaluate_lifecycle(doc, repo_root, Some(claim_bindings));
    if !lifecycle.is_empty() {
        return lifecycle;
    }
    let by_id = policies_by_id(doc);
    let Some((_, policy)) = by_id.iter().find(|(k, _)| as_str(k).as_deref() == Some(policy_id)) else {
        return vec![Violation::new("UNKNOWN_POLICY_ID", format!("{} not found in registry", r(policy_id))).path_s("$.policies")];
    };
    let removal = get(policy, "removal_evidence");
    let artifact = if truthy(removal) { as_str(get(removal, "path")) } else { None };
    let expected = compute_contract_tree_sha256(doc, schema, claim_bindings, repo_root, artifact.as_deref());
    check_removal_eligibility(policy, repo_root, Some(&expected))
}

// ── misc used by the arch contract ────────────────────────────────────────────────────────────

pub fn read_text(path: &Path) -> Result<String, String> {
    match std::fs::read(path) {
        Ok(b) => match pyjson::decode_file_text(&b) {
            Ok(t) => Ok(t.to_utf8().unwrap_or_default()),
            Err(e) => Err(e.to_string()),
        },
        Err(e) => Err(pathutil::os_error_text(&e, path)),
    }
}

pub fn repo_rel(root: &Path, rel: &str) -> PathBuf {
    root.join(rel)
}

pub type StrMap = BTreeMap<String, String>;
