//! Rust port of `.claude/policies/runtime/completion_gate.py`: the manifest-driven 3-state completion gate
//! (`verify`) and the side-effect authorization gate (`authorize`). Same reason codes, messages, exit codes,
//! JSON (sort_keys, indent=2, ensure_ascii=False) and filesystem rules (descriptor-relative O_NOFOLLOW walks).

use std::path::{Component, Path, PathBuf};

use agent_runtime_support::pysort::py_eq;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use sha2::{Digest, Sha256};

use crate::fsx::{self, Fd};
use crate::markdown::{classify_link_target, scan_markdown_links, LinkKind};
use crate::pytext::{self, display, repr, repr_list, repr_opt};

pub use agent_runtime_support::schema::validate_against_schema;

pub const SCHEMA_VERSION: i64 = 1;

pub const ALLOWED_ACTIONS: [&str; 5] = ["sync_to_sub", "sync_branches", "hint_finalize", "hint_verify", "hint_push"];

pub const STRONG_IDENTITY_FIELDS: [&str; 6] = ["model", "gpu", "vllm", "quant", "topology", "tp"];
/// manifest identity field -> certificate artifact field name.
pub const CERTIFICATE_FIELD_MAP: [(&str, &str); 6] = [
    ("model", "model"),
    ("gpu", "gpu_model"),
    ("vllm", "vllm_version"),
    ("quant", "quantization"),
    ("topology", "topology"),
    ("tp", "tensor_parallel_size"),
];
pub const CERTIFICATE_RUN_KEY_FIELDS: [&str; 7] =
    ["model", "gpu_model", "vllm_version", "quantization", "topology", "tensor_parallel_size", "measured_utc"];
pub const LITE_CERT_VERDICT: &str = "not_applicable";
pub const RUBRIC_AUTHORITIES: [&str; 3] = ["weak", "explicit", "explore"];
pub const OBSERVATION_ONLY_VERDICT: &str = "OBSERVATION-ONLY";
pub const OBSERVATION_ONLY_JUDGE_VERDICT: &str = "NEEDS_RUBRIC";
pub const ANNOTATED_NON_PASS_VERDICTS: [&str; 2] = ["REFUTE", "FAIL"];
const OBSERVATION_ONLY_EXPECTED_RUBRIC_GAPS: [&str; 3] =
    ["MANIFEST_RUBRIC_FLOOR_INVALID", "MANIFEST_RUBRIC_RATIO_MISSING", "MANIFEST_RUBRIC_SOURCE_MISSING"];
pub const MANIFEST_RUBRIC_SOURCES: [&str; 1] = ["verdict_json"];
pub const PII_MACHINE_RAW_EXEMPTION_REASON: &str = "machine-raw(docs.md §PII 판정 대상 밖)";
pub const PII_EXEMPTIBLE_EVIDENCE_KEYS: [&str; 1] = ["simlog"];
pub const CERTIFICATE_ISSUING_AUTHORITIES: [&str; 1] = ["explicit"];
pub const CERTIFICATE_NON_ISSUING_AUTHORITIES: [&str; 2] = ["weak", "explore"];
pub const BENCH_REPORT_VERDICT_SECTION_PREFIX: &str = "## 판정 (표시만";
pub const PROMOTION_CAPPED_TASK_CLASSES: [&str; 3] = ["capacity_rejection", "minor_patch", "read_only_audit"];
pub const DIR_EVIDENCE_KEYS: [&str; 1] = ["simlog"];
pub const MARKDOWN_LIKE_EVIDENCE_KEYS: [&str; 7] =
    ["plan", "devlog", "testlog", "bench_report", "report", "verification", "commit"];

/// `BASE_REQUIRED_EVIDENCE[task_class]`.
pub fn base_required_evidence(task_class: &str) -> &'static [&'static str] {
    match task_class {
        "model_serving_strategy" => &["plan", "devlog", "simlog", "testlog"],
        "full_benchmark" => &["plan", "devlog", "simlog", "testlog", "bench_report"],
        "harness_change" => &["plan", "devlog", "testlog"],
        "capacity_rejection" => &["plan"],
        "minor_patch" => &["verification", "commit"],
        "read_only_audit" => &[],
        "hint_map_only" => &["plan", "devlog", "testlog"],
        _ => &[],
    }
}

// ============================================================================ value helpers

pub fn get<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(key),
        _ => None,
    }
}

fn get_dict<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    get(v, key).filter(|x| matches!(x.peel(), PyValue::Dict(_)))
}

pub fn truthy(v: Option<&PyValue>) -> bool {
    v.is_some_and(|x| x.is_truthy())
}

fn is_true(v: Option<&PyValue>) -> bool {
    matches!(v.map(|x| x.peel()), Some(PyValue::Bool(true)))
}

pub fn as_str(v: Option<&PyValue>) -> Option<String> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Str(s)) => Some(pytext::pystr_text(s)),
        _ => None,
    }
}

fn eq_str(v: Option<&PyValue>, s: &str) -> bool {
    matches!(v.map(|x| x.peel()), Some(PyValue::Str(t)) if t.equals(s))
}

fn s(text: &str) -> PyValue {
    PyValue::Str(PyStr::new(text))
}

fn opt_s(text: Option<&str>) -> PyValue {
    text.map(s).unwrap_or(PyValue::None)
}

fn opt_f(x: Option<f64>) -> PyValue {
    x.map(PyValue::Float).unwrap_or(PyValue::None)
}

fn list_s<S: AsRef<str>>(items: &[S]) -> PyValue {
    PyValue::List(items.iter().map(|x| s(x.as_ref())).collect())
}

fn float_repr(x: Option<f64>) -> String {
    match x {
        None => "None".into(),
        Some(v) => agent_runtime_support::pyvalue::float_repr(v),
    }
}

/// Builder for a Python dict literal.
pub struct D(PyDict);
impl D {
    pub fn new() -> D {
        D(PyDict::new())
    }
    pub fn put(mut self, k: &str, v: PyValue) -> D {
        self.0.set(k, v);
        self
    }
    pub fn v(self) -> PyValue {
        PyValue::Dict(self.0)
    }
}
impl Default for D {
    fn default() -> Self {
        D::new()
    }
}

fn dict_set(v: &mut PyValue, k: &str, x: PyValue) {
    if let PyValue::Dict(d) = v {
        d.set(k, x);
    }
}

fn dict_setdefault(v: &mut PyValue, k: &str, x: PyValue) {
    if let PyValue::Dict(d) = v {
        if !d.contains_key(k) {
            d.set(k, x);
        }
    }
}

/// `json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2)`.
pub fn dumps_sorted(v: &PyValue) -> String {
    let sorted = sort_keys(v);
    let text = sorted.dumps_pretty().expect("JSON-serializable output");
    pytext::pystr_text(&text)
}

fn sort_keys(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut items: Vec<(PyStr, PyValue)> = d.iter().map(|(k, x)| (k.clone(), sort_keys(x))).collect();
            items.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (k, x) in items {
                out.insert(k, x);
            }
            PyValue::Dict(out)
        }
        PyValue::List(items) => PyValue::List(items.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

// ============================================================================ path helpers

/// `_is_absolute_path_string`.
pub fn is_absolute_path_string(p: &str) -> bool {
    if p.starts_with('/') || p.starts_with('\\') {
        return true;
    }
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// `os.path.normpath` (POSIX).
pub fn normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let initial = if path.starts_with("//") && !path.starts_with("///") {
        2
    } else if path.starts_with('/') {
        1
    } else {
        0
    };
    let mut comps: Vec<&str> = Vec::new();
    for c in path.split('/') {
        if c.is_empty() || c == "." {
            continue;
        }
        if c != ".." || (initial == 0 && comps.is_empty()) || comps.last() == Some(&"..") {
            comps.push(c);
        } else if !comps.is_empty() {
            comps.pop();
        }
    }
    let joined = comps.join("/");
    let prefix = "/".repeat(initial);
    let out = format!("{prefix}{joined}");
    if out.is_empty() {
        ".".into()
    } else {
        out
    }
}

/// `Path(a) / b` as a string (b absolute replaces a).
fn path_join(a: &Path, b: &str) -> String {
    if b.starts_with('/') {
        return b.to_string();
    }
    let a = a.to_string_lossy();
    if b.is_empty() {
        return a.into_owned();
    }
    if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// `_lexical_components`: `None` = repo_escape, else the components from repo_root to the target.
pub fn lexical_components(manifest_dir: &Path, repo_root: &Path, rel_path: &str) -> Option<Vec<String>> {
    let lexical = normpath(&path_join(manifest_dir, rel_path));
    let lex_parts = posix_parts(&lexical);
    let root_parts = posix_parts(&repo_root.to_string_lossy());
    if lex_parts.len() < root_parts.len() || lex_parts[..root_parts.len()] != root_parts[..] {
        return None;
    }
    Some(lex_parts[root_parts.len()..].iter().filter(|p| !p.is_empty() && *p != ".").cloned().collect())
}

/// `PurePosixPath(s).parts`.
fn posix_parts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if text.starts_with("//") && !text.starts_with("///") {
        out.push("//".to_string());
    } else if text.starts_with('/') {
        out.push("/".to_string());
    }
    for c in text.split('/') {
        if !c.is_empty() && c != "." {
            out.push(c.to_string());
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walk {
    NotFound,
    NotADirectory,
    Symlink,
    OsError,
}

impl Walk {
    pub fn as_str(&self) -> &'static str {
        match self {
            Walk::NotFound => "not_found",
            Walk::NotADirectory => "not_a_directory",
            Walk::Symlink => "symlink",
            Walk::OsError => "os_error",
        }
    }
}

/// `_walk_nofollow`.
pub fn walk_nofollow(repo_root_fd: &Fd, parts: &[String]) -> Result<Fd, Walk> {
    if parts.is_empty() {
        return Err(Walk::NotFound);
    }
    let mut cur = repo_root_fd.dup().map_err(|_| Walk::OsError)?;
    for (i, part) in parts.iter().enumerate() {
        let last = i == parts.len() - 1;
        let mut flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK;
        if !last {
            flags |= libc::O_DIRECTORY;
        }
        let bytes = PyStr::new(part).to_surrogateescape_bytes().unwrap_or_else(|_| part.as_bytes().to_vec());
        match fsx::openat(cur.raw(), &bytes, flags) {
            Ok(next) => cur = next,
            Err(e) if e == libc::ENOENT => return Err(Walk::NotFound),
            Err(e) if e == libc::ENOTDIR => return Err(Walk::NotADirectory),
            Err(e) if e == libc::ELOOP => return Err(Walk::Symlink),
            Err(_) => return Err(Walk::OsError),
        }
    }
    Ok(cur)
}

fn hash_fd(fd: &Fd, capture: bool) -> (String, Option<Vec<u8>>) {
    let bytes = fsx::read_all_dup(fd).unwrap_or_default();
    let digest = hex(&Sha256::digest(&bytes));
    (digest, if capture { Some(bytes) } else { None })
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `_tree_fingerprint` -> Ok((hash, has_raw_evidence)) | Err(reason).
fn tree_fingerprint(dir: &Fd) -> Result<(String, bool), &'static str> {
    let mut names = fsx::listdir_fd(dir.raw()).map_err(|_| "tree_unreadable")?;
    names.sort_by(|a, b| PyStr::from_surrogateescape(a).code_points().cmp(PyStr::from_surrogateescape(b).code_points()));
    let mut records: Vec<u8> = Vec::new();
    let mut has_raw = false;
    for name in names {
        let st = fsx::lstatat(dir.raw(), &name).map_err(|_| "tree_unreadable")?;
        if fsx::is_lnk(&st) {
            return Err("tree_contains_symlink");
        }
        let len_prefix = (name.len() as u32).to_be_bytes();
        if fsx::is_dir(&st) {
            let sub = fsx::openat(dir.raw(), &name, libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .map_err(|_| "tree_unreadable")?;
            let (sub_hash, sub_raw) = tree_fingerprint(&sub)?;
            has_raw = has_raw || sub_raw;
            records.push(b'D');
            records.extend_from_slice(&len_prefix);
            records.extend_from_slice(&name);
            records.extend_from_slice(&unhex(&sub_hash));
        } else if fsx::is_reg(&st) {
            let fd = fsx::openat(dir.raw(), &name, libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .map_err(|_| "tree_unreadable")?;
            let (h, _) = hash_fd(&fd, false);
            if st.st_size > 0 {
                has_raw = true;
            }
            records.push(b'F');
            records.extend_from_slice(&len_prefix);
            records.extend_from_slice(&name);
            records.extend_from_slice(&unhex(&h));
        } else {
            return Err("tree_contains_special_file");
        }
    }
    Ok((hex(&Sha256::digest(&records)), has_raw))
}

fn unhex(h: &str) -> Vec<u8> {
    (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0)).collect()
}

/// The result dict of `resolve_and_stat_evidence`.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub status: &'static str,
    pub kind: Option<&'static str>,
    pub sha256: Option<String>,
    pub tree_sha256: Option<String>,
    pub content_bytes: Option<Vec<u8>>,
}

impl Resolved {
    pub fn ok(&self) -> bool {
        self.status == "ok"
    }
}

/// `resolve_and_stat_evidence`.
pub fn resolve_and_stat_evidence(
    repo_root_fd: &Fd,
    repo_root: &Path,
    manifest_dir: &Path,
    rel_path: &str,
    expect_dir: bool,
    capture_content: bool,
) -> Resolved {
    let mut r = Resolved::default();
    let parts = match lexical_components(manifest_dir, repo_root, rel_path) {
        None => {
            r.status = "repo_escape";
            return r;
        }
        Some(p) => p,
    };
    let fd = match walk_nofollow(repo_root_fd, &parts) {
        Err(w) => {
            r.status = w.as_str();
            return r;
        }
        Ok(fd) => fd,
    };
    let st = match fsx::fstat(fd.raw()) {
        Ok(st) => st,
        Err(_) => {
            r.status = "os_error";
            return r;
        }
    };
    if fsx::is_dir(&st) {
        r.kind = Some("dir");
        if !expect_dir {
            r.status = "wrong_type";
            return r;
        }
        match tree_fingerprint(&fd) {
            Err(_) => {
                r.status = "wrong_type";
            }
            Ok((h, has_raw)) => {
                r.tree_sha256 = Some(h);
                r.status = if has_raw { "ok" } else { "empty_tree" };
            }
        }
        return r;
    }
    if fsx::is_reg(&st) {
        r.kind = Some("file");
        if expect_dir || st.st_size == 0 {
            r.status = "wrong_type";
            return r;
        }
        let (d, c) = hash_fd(&fd, capture_content);
        r.status = "ok";
        r.sha256 = Some(d);
        r.content_bytes = c;
        return r;
    }
    r.status = "wrong_type";
    r
}

/// `_resolve_link_target`.
fn resolve_link_target(repo_root_fd: &Fd, repo_root: &Path, base_dir: &Path, rel_path: &str) -> &'static str {
    let parts = match lexical_components(base_dir, repo_root, rel_path) {
        None => return "repo_escape",
        Some(p) => p,
    };
    let fd = match walk_nofollow(repo_root_fd, &parts) {
        Err(w) => return w.as_str(),
        Ok(fd) => fd,
    };
    match fsx::fstat(fd.raw()) {
        Ok(st) if fsx::is_reg(&st) || fsx::is_dir(&st) => "ok",
        Ok(_) => "wrong_type",
        Err(_) => "wrong_type",
    }
}

/// `check_required_local_links` -> (unreadable, broken, invalid).
pub fn check_required_local_links(
    repo_root_fd: &Fd,
    repo_root: &Path,
    base_dir: &Path,
    content: &[u8],
) -> (bool, Vec<String>, Vec<String>) {
    let text = match std::str::from_utf8(content) {
        Ok(t) => t,
        Err(_) => return (true, vec![], vec![]),
    };
    let (raw, unresolved) = scan_markdown_links(text);
    let mut broken = Vec::new();
    let mut invalid = unresolved;
    for t in raw {
        match classify_link_target(&t) {
            LinkKind::Skip => {}
            LinkKind::Invalid(_) => invalid.push(t),
            LinkKind::Local(p) => {
                if resolve_link_target(repo_root_fd, repo_root, base_dir, &p) != "ok" {
                    broken.push(t);
                }
            }
        }
    }
    (false, broken, invalid)
}

// ============================================================================ flat certificate

/// Ordered `dict[str, str]` of a flat certificate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fields(pub Vec<(String, String)>);

impl Fields {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// `parse_flat_certificate` -> (fields, ok).
pub fn parse_flat_certificate(text: &str) -> (Fields, bool) {
    let mut fields = Fields::default();
    for line in pytext::splitlines(text) {
        let stripped = pytext::strip(line);
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        if line != stripped {
            return (Fields::default(), false);
        }
        let key_len = line.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
        if key_len == 0 || line.as_bytes().get(key_len) != Some(&b':') {
            return (Fields::default(), false);
        }
        let key = &line[..key_len];
        let rest = &line[key_len + 1..];
        let value = pytext::strip(rest);
        let mut value = value.to_string();
        let vc: Vec<char> = value.chars().collect();
        if vc.len() >= 2 && vc[0] == '"' && vc[vc.len() - 1] == '"' {
            value = vc[1..vc.len() - 1].iter().collect::<String>().replace("\\\"", "\"");
        }
        if fields.get(key).is_some() {
            return (Fields::default(), false);
        }
        fields.0.push((key.to_string(), value));
    }
    (fields, true)
}

/// `certificate_grade`: Some("full") | Some("lite") | None.
pub fn certificate_grade(fields: &Fields) -> Option<&'static str> {
    let mode = pytext::strip(fields.get("benchmark_mode").unwrap_or(""));
    match mode {
        "" | "full" => Some("full"),
        "lite" => Some("lite"),
        _ => None,
    }
}

/// `certificate_run_key`.
pub fn certificate_run_key(fields: &Fields) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for k in CERTIFICATE_RUN_KEY_FIELDS {
        let v = fields.get(k)?;
        let t = pytext::strip(v);
        if t.is_empty() {
            return None;
        }
        out.push(t.to_string());
    }
    Some(out)
}

/// Python `int(text)` for a str.
fn py_int(text: &str) -> Option<num_bigint::BigInt> {
    let t = pytext::strip(text);
    let (neg, body) = match t.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if body.is_empty() || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
        return None;
    }
    let mut digits = String::new();
    for c in body.chars() {
        if c == '_' {
            continue;
        }
        let d = agent_runtime_support::pystr::decimal_digit_value(c as u32)?;
        digits.push(char::from_digit(d, 10)?);
    }
    let n: num_bigint::BigInt = digits.parse().ok()?;
    Some(if neg { -n } else { n })
}

/// `_certificate_strong_identity_matches` -> (mapped (field, cert value), mismatched fields).
pub fn certificate_strong_identity_matches(
    fields: &Fields,
    identity: &PyValue,
) -> (Vec<(&'static str, Option<String>)>, Vec<&'static str>) {
    let mut mapped = Vec::new();
    let mut mismatched = Vec::new();
    for (f, cert_key) in CERTIFICATE_FIELD_MAP {
        let cert_val = fields.get(cert_key).map(str::to_string);
        mapped.push((f, cert_val.clone()));
        if f == "tp" {
            let cert_tp = match cert_val.as_deref().and_then(py_int) {
                Some(n) => n,
                None => {
                    mismatched.push(f);
                    continue;
                }
            };
            let manifest_tp = get(identity, "tp");
            let same = match manifest_tp {
                Some(v) => py_eq(&PyValue::Int(cert_tp), v),
                None => false,
            };
            if !same {
                mismatched.push(f);
            }
        } else {
            let manifest_cmp = match get(identity, f) {
                None => "N/A".to_string(),
                Some(v) if matches!(v.peel(), PyValue::None) => "N/A".to_string(),
                Some(v) => display(v),
            };
            if cert_val.as_deref() != Some(manifest_cmp.as_str()) {
                mismatched.push(f);
            }
        }
    }
    (mapped, mismatched)
}

/// `_certificate_number` for a text value.
pub fn certificate_number_text(raw: Option<&str>) -> Option<f64> {
    let text = pytext::strip(raw?);
    if text.is_empty() || text.to_uppercase() == "N/A" {
        return None;
    }
    let v = py_float(text)?;
    if v.is_nan() || v.is_infinite() {
        return None;
    }
    Some(v)
}

/// `_certificate_number` for a JSON value (`str(raw)` first).
pub fn certificate_number(raw: Option<&PyValue>) -> Option<f64> {
    match raw {
        None => None,
        Some(v) if matches!(v.peel(), PyValue::None) => None,
        Some(v) => certificate_number_text(Some(&display(v))),
    }
}

/// Python `float(text)` (text already stripped).
fn py_float(text: &str) -> Option<f64> {
    let mut t = String::new();
    for c in text.chars() {
        if let Some(d) = agent_runtime_support::pystr::decimal_digit_value(c as u32) {
            if !c.is_ascii_digit() {
                t.push(char::from_digit(d, 10)?);
                continue;
            }
        }
        t.push(c);
    }
    if t.contains('_') {
        let cs: Vec<char> = t.chars().collect();
        for (i, c) in cs.iter().enumerate() {
            if *c == '_' {
                let ok = i > 0 && i + 1 < cs.len() && cs[i - 1].is_ascii_digit() && cs[i + 1].is_ascii_digit();
                if !ok {
                    return None;
                }
            }
        }
        t = t.replace('_', "");
    }
    let lower = t.to_ascii_lowercase();
    let unsigned = lower.trim_start_matches(['+', '-']);
    if lower.len() - unsigned.len() > 1 {
        return None;
    }
    if matches!(unsigned, "inf" | "infinity" | "nan") {
        return lower.parse::<f64>().ok().or(Some(f64::NAN));
    }
    if !unsigned.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | '+' | '-')) {
        return None;
    }
    t.parse::<f64>().ok()
}

/// `certificate_requirement_authority`.
pub fn certificate_requirement_authority(benchmark: &PyValue) -> Option<String> {
    let authority = as_str(get(benchmark, "rubric_authority"))?;
    let source = as_str(get(benchmark, "rubric_source"))?;
    let (a, so) = (pytext::strip(&authority), pytext::strip(&source));
    if !MANIFEST_RUBRIC_SOURCES.contains(&so) || !RUBRIC_AUTHORITIES.contains(&a) {
        return None;
    }
    Some(a.to_string())
}

/// `certificate_waived`.
pub fn certificate_waived(task_class: &str, verdict: Option<&PyValue>, authority: Option<&str>) -> bool {
    task_class == "full_benchmark"
        && eq_str(verdict, "PASS")
        && authority.is_some_and(|a| CERTIFICATE_NON_ISSUING_AUTHORITIES.contains(&a))
}

/// `required_evidence_for`.
pub fn required_evidence_for(
    task_class: &str,
    conditions: Option<&PyValue>,
    verdict: Option<&PyValue>,
    authority: Option<&str>,
) -> Vec<String> {
    let mut required: Vec<String> = base_required_evidence(task_class).iter().map(|s| s.to_string()).collect();
    let cond = conditions.filter(|c| c.is_truthy());
    let cget = |k: &str| cond.and_then(|c| get(c, k));
    if task_class == "harness_change" && is_true(cget("actual_trial")) {
        required.push("simlog".into());
    }
    if task_class == "read_only_audit" && is_true(cget("report_requested")) {
        required.push("report".into());
    }
    if task_class == "full_benchmark" && eq_str(verdict, "PASS") && !certificate_waived(task_class, verdict, authority) {
        required.push("certificate".into());
    }
    required
}

/// `bench_report_verdict_table` -> (table as ordered (field, value), state).
pub fn bench_report_verdict_table(text: Option<&str>) -> (Option<Vec<(&'static str, Option<String>)>>, String) {
    let text = match text {
        None => return (None, "unparseable(본문이 UTF-8 문자열이 아니다)".into()),
        Some(t) => t,
    };
    let lines = pytext::splitlines(text);
    let start = match lines.iter().position(|ln| pytext::strip(ln).starts_with(BENCH_REPORT_VERDICT_SECTION_PREFIX)) {
        None => return (None, "absent(판정 절 없음)".into()),
        Some(i) => i,
    };
    let rows: [(&str, &'static str); 2] = [("verdict", "verdict"), ("루브릭 권한", "rubric_authority")];
    let mut out: Vec<(&'static str, Option<String>)> = Vec::new();
    for ln in &lines[start + 1..] {
        let stripped = pytext::strip(ln);
        if stripped.starts_with("## ") {
            break;
        }
        if !stripped.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = stripped.trim_matches('|').split('|').map(pytext::strip).collect();
        if cells.len() < 2 {
            continue;
        }
        let field = match rows.iter().find(|(k, _)| *k == cells[0]) {
            None => continue,
            Some((_, f)) => *f,
        };
        if out.iter().any(|(f, _)| *f == field) {
            return (None, format!("unparseable(판정 표에 {} 행이 두 번 있다)", repr(cells[0])));
        }
        let value = pytext::strip(pytext::strip(cells[1]).trim_matches('*'));
        let up = value.to_uppercase();
        out.push((field, if up.is_empty() || up == "N/A" { None } else { Some(value.to_string()) }));
    }
    let missing: Vec<&str> = rows.iter().filter(|(_, f)| !out.iter().any(|(g, _)| g == f)).map(|(k, _)| *k).collect();
    if !missing.is_empty() {
        return (None, format!("unparseable(판정 표에 {} 행이 없다)", repr_list(&missing)));
    }
    (Some(out), "parsed".into())
}

fn table_get<'a>(t: &'a [(&'static str, Option<String>)], k: &str) -> Option<&'a str> {
    t.iter().find(|(f, _)| *f == k).and_then(|(_, v)| v.as_deref())
}

fn table_repr(t: &Option<Vec<(&'static str, Option<String>)>>) -> String {
    match t {
        None => "None".into(),
        Some(rows) => {
            let parts: Vec<String> = rows
                .iter()
                .map(|(k, v)| format!("{}: {}", repr(k), v.as_deref().map(repr).unwrap_or_else(|| "None".into())))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// `_manifest_rubric_contract`.
pub struct ManifestRubric {
    pub declared: bool,
    pub ok: bool,
    pub authority: Option<String>,
    pub floor_tps: Option<f64>,
    pub ratio: Option<f64>,
    pub primary_source: Option<String>,
    pub problems: Vec<(String, String)>,
}

fn strip_str_or_empty(v: Option<&PyValue>) -> String {
    as_str(v).map(|x| pytext::strip(&x).to_string()).unwrap_or_default()
}

pub fn manifest_rubric_contract(benchmark: &PyValue) -> ManifestRubric {
    let mut out = ManifestRubric {
        declared: false,
        ok: false,
        authority: None,
        floor_tps: None,
        ratio: None,
        primary_source: None,
        problems: vec![],
    };
    if !matches!(benchmark.peel(), PyValue::Dict(_)) {
        return out;
    }
    let raw_authority = strip_str_or_empty(get(benchmark, "rubric_authority"));
    if raw_authority.is_empty() || raw_authority.to_uppercase() == "N/A" {
        return out;
    }
    out.declared = true;
    let mut problems = Vec::new();
    if RUBRIC_AUTHORITIES.contains(&raw_authority.as_str()) {
        out.authority = Some(raw_authority.clone());
    } else {
        problems.push((
            "MANIFEST_RUBRIC_AUTHORITY_UNKNOWN".to_string(),
            format!(
                "benchmark.rubric_authority={} is not one of {} -- an unknown rubric authority cannot be reasoned about, fail closed",
                repr(&raw_authority),
                repr_list(&RUBRIC_AUTHORITIES)
            ),
        ));
    }
    let raw_source = strip_str_or_empty(get(benchmark, "rubric_source"));
    if !MANIFEST_RUBRIC_SOURCES.contains(&raw_source.as_str()) {
        problems.push((
            "MANIFEST_RUBRIC_PROVENANCE_MISSING".to_string(),
            format!(
                "benchmark.rubric_source={} is not one of {} -- 출처 표시가 없는 rubric 값은 판정기 산출물 파생분과 손저작을 데이터에서 구분할 수 없다(헌법 §결정론 규율). 승격을 열지 않는다",
                repr_opt(get(benchmark, "rubric_source")),
                repr_list(&MANIFEST_RUBRIC_SOURCES)
            ),
        ));
    }
    let floor = certificate_number(get(benchmark, "floor_tps"));
    out.floor_tps = floor;
    if floor.is_none_or(|f| f <= 0.0) {
        problems.push((
            "MANIFEST_RUBRIC_FLOOR_INVALID".to_string(),
            format!(
                "benchmark.floor_tps={} is not a finite positive number -- floor<=0 means the rubric imposed NO threshold (공허 PASS): every measurement passes. Not promotable in ANY authority.",
                repr_opt(get(benchmark, "floor_tps"))
            ),
        ));
    }
    let ratio = certificate_number(get(benchmark, "ratio_M_over_primary"));
    out.ratio = ratio;
    if ratio.is_none() {
        problems.push((
            "MANIFEST_RUBRIC_RATIO_MISSING".to_string(),
            format!(
                "benchmark.ratio_M_over_primary={} is not a finite number -- the measurement-vs-rubric indicator is absent, so the verdict cannot be audited",
                repr_opt(get(benchmark, "ratio_M_over_primary"))
            ),
        ));
    }
    let ps = strip_str_or_empty(get(benchmark, "primary_source"));
    out.primary_source = if ps.is_empty() { None } else { Some(ps.clone()) };
    if ps.is_empty() || ps.to_uppercase() == "N/A" {
        problems.push((
            "MANIFEST_RUBRIC_SOURCE_MISSING".to_string(),
            format!(
                "benchmark.primary_source={} is absent -- which rubric slot won cannot be established (surfaced, never matched as a string: the numeric floor/ratio contract above is the gate)",
                repr_opt(get(benchmark, "primary_source"))
            ),
        ));
    }
    out.ok = problems.is_empty();
    out.problems = problems;
    out
}

// ============================================================================ simlog termination

fn read_json_file(p: &Path) -> Result<PyValue, String> {
    let bytes = std::fs::read(p).map_err(|e| match e.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EPERM) => "PermissionError".to_string(),
        Some(libc::ENOENT) => "FileNotFoundError".to_string(),
        Some(libc::EISDIR) => "IsADirectoryError".to_string(),
        Some(libc::ENOTDIR) => "NotADirectoryError".to_string(),
        _ => "OSError".to_string(),
    })?;
    let text = agent_runtime_support::pyjson::decode_file_text(&bytes).map_err(|_| "UnicodeDecodeError".to_string())?;
    agent_runtime_support::pyjson::loads_text(&text).map_err(|e| match e {
        agent_runtime_support::pyjson::LoadError::Decode { .. } => "JSONDecodeError".to_string(),
        _ => "ValueError".to_string(),
    })
}

/// Python `Path.resolve()` (non-strict).
pub fn resolve_lenient(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")).join(p)
    };
    if let Ok(c) = std::fs::canonicalize(&abs) {
        return c;
    }
    let mut out = PathBuf::from("/");
    let mut comps: Vec<Component> = abs.components().collect();
    comps.reverse();
    while let Some(c) = comps.pop() {
        match c {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(n) => {
                let cand = out.join(n);
                match std::fs::read_link(&cand) {
                    Ok(target) => {
                        let t = if target.is_absolute() { target } else { out.join(target) };
                        if let Ok(c2) = std::fs::canonicalize(&t) {
                            out = c2;
                        } else {
                            out = resolve_lenient(&t);
                        }
                    }
                    Err(_) => out = cand,
                }
            }
        }
    }
    out
}

fn simlog_terminated(repo_root: &Path, manifest_dir: &Path, rel_path: &str) -> (bool, String) {
    let base = resolve_lenient(Path::new(&path_join(manifest_dir, rel_path)));
    let root = resolve_lenient(repo_root);
    if !base.starts_with(&root) {
        return (false, "vault 경로가 저장소 밖이다".into());
    }
    let rs = base.join("run_summary.json");
    if rs.is_file() {
        let doc = match read_json_file(&rs) {
            Ok(d) => d,
            Err(cls) => return (false, format!("run_summary.json 판독 실패({cls})")),
        };
        if !matches!(doc.peel(), PyValue::Dict(_)) {
            return (false, "run_summary.json 이 객체가 아니다".into());
        }
        let fc = get(&doc, "final_class");
        let fc_present = match fc {
            None => false,
            Some(v) => !(py_eq(v, &PyValue::None) || py_eq(v, &s(""))),
        };
        if is_true(get(&doc, "converged")) || fc_present {
            return (true, "trial vault: run_summary 가 종결을 말한다".into());
        }
        return (
            false,
            format!(
                "trial vault 인데 run_summary 가 종결을 말하지 않는다(converged={} final_class={})",
                repr_opt(get(&doc, "converged")),
                repr_opt(fc)
            ),
        );
    }
    let mut first: Vec<PathBuf> = Vec::new();
    let mut second: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&base) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".json") {
                if stem.contains("measured") {
                    first.push(base.join(&name));
                }
            }
            let sub = base.join(&name);
            if sub.is_dir() {
                let m = sub.join("measured.json");
                if std::fs::symlink_metadata(&m).is_ok() && m.exists() {
                    second.push(m);
                }
            }
        }
    }
    let key = |p: &PathBuf| p.to_string_lossy().split('/').map(|x| x.to_string()).collect::<Vec<_>>();
    first.sort_by_key(key);
    second.sort_by_key(key);
    for m in first.iter().chain(second.iter()) {
        if let Ok(doc) = read_json_file(m) {
            if let PyValue::Dict(d) = doc.peel() {
                if d.contains_key("measurement_ok") {
                    let name = m.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    return (true, format!("bench vault: {name} 이 측정 종결을 말한다"));
                }
            }
        }
    }
    (
        false,
        "run_summary.json 도 없고 measurement_ok 를 담은 measured 산출물도 없다 — 이 vault 는 자기가 끝났는지를 말하지 않는다".into(),
    )
}

// ============================================================================ CLI plumbing

/// Paths this program locates relative to the original script (`Path(__file__)`).
#[derive(Clone)]
pub struct Env {
    /// `Path(__file__).resolve()` of completion_gate.py.
    pub script: PathBuf,
    /// argv[0] to give the child verify run (promotion mode).
    pub argv0: String,
}

impl Env {
    pub fn script_dir(&self) -> PathBuf {
        self.script.parent().map(Path::to_path_buf).unwrap_or_default()
    }
    /// `Path(__file__).resolve().parents[3]`.
    pub fn repo_of_script(&self) -> PathBuf {
        let mut p = self.script.clone();
        for _ in 0..4 {
            p.pop();
        }
        p
    }
    pub fn schema_dir(&self) -> PathBuf {
        let mut p = self.script.clone();
        for _ in 0..3 {
            p.pop();
        }
        p.join("schemas")
    }
}

/// Load one schema file (`_load_schema`).
pub fn load_schema(schema_dir: &Path, name: &str) -> Result<PyValue, String> {
    let p = schema_dir.join(name);
    let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let text = agent_runtime_support::pyjson::decode_file_text(&bytes).map_err(|e| e.to_string())?;
    agent_runtime_support::pyjson::loads_text(&text).map_err(|e| e.to_string())
}

pub struct Schemas {
    pub work: PyValue,
    pub completion: PyValue,
    pub side_effect: PyValue,
}

impl Schemas {
    pub fn load(env: &Env) -> Result<Schemas, String> {
        let d = env.schema_dir();
        Ok(Schemas {
            work: load_schema(&d, "work-manifest.schema.json")?,
            completion: load_schema(&d, "completion-manifest.schema.json")?,
            side_effect: load_schema(&d, "side-effect-authorization.schema.json")?,
        })
    }
}

/// A finished run: what to print on stdout, the exit code.
pub struct Exit {
    pub stdout: String,
    pub code: i32,
}

/// `_emit_with_schema`: set exit_code, self-validate, render. Err = the internal-bug RuntimeError text.
pub fn emit_with_schema(mut obj: PyValue, exit_code: i32, schema: &PyValue) -> Result<Exit, String> {
    dict_set(&mut obj, "exit_code", PyValue::int(exit_code));
    let violations = validate_against_schema(&obj, schema).map_err(|e| format!("{}: {}", e.class, e.message))?;
    if !violations.is_empty() {
        let title = as_str(get(schema, "title")).unwrap_or_else(|| "?".into());
        let joined: Vec<String> = violations.iter().map(|(c, m)| format!("{c}: {m}")).collect();
        return Err(format!(
            "completion_gate: internal bug -- own output violates {title}.schema.json: {}",
            joined.join("; ")
        ));
    }
    Ok(Exit { stdout: dumps_sorted(&obj) + "\n", code: exit_code })
}

type R = Result<Exit, String>;

fn emit(sch: &Schemas, mut obj: PyValue, code: i32) -> R {
    dict_setdefault(&mut obj, "certificate", PyValue::None);
    emit_with_schema(obj, code, &sch.completion)
}

fn emit_authorization(sch: &Schemas, mut obj: PyValue, code: i32) -> R {
    dict_setdefault(&mut obj, "messages", PyValue::Dict(PyDict::new()));
    dict_setdefault(&mut obj, "propagation_scope", PyValue::None);
    emit_with_schema(obj, code, &sch.side_effect)
}

pub fn bare_result(code: &str, message: &str) -> PyValue {
    D::new()
        .put("schema_version", PyValue::int(SCHEMA_VERSION))
        .put("task_class", PyValue::None)
        .put("state", PyValue::None)
        .put("eligible_for_promotion", PyValue::Bool(false))
        .put("reason_codes", list_s(&[code]))
        .put("messages", D::new().put(code, s(message)).v())
        .put("checked_evidence", D::new().v())
        .put("identity", PyValue::None)
        .put("certificate", PyValue::None)
        .v()
}

pub fn authorize_bare_result(mode: Option<&str>, action: Option<&str>, code: &str, message: &str) -> PyValue {
    D::new()
        .put("schema_version", PyValue::int(SCHEMA_VERSION))
        .put("mode", opt_s(mode))
        .put("action", opt_s(action))
        .put("task_class", PyValue::None)
        .put("authorization_state", PyValue::None)
        .put("allowed", PyValue::Bool(false))
        .put("reason_codes", list_s(&[code]))
        .put("messages", D::new().put(code, s(message)).v())
        .put("identity", PyValue::None)
        .v()
}

struct LoadErr {
    code: &'static str,
    message: String,
}

fn read_manifest_text(path_arg: &str) -> Result<PyStr, LoadErr> {
    let shown = pytext::pure_path_str(path_arg);
    let bytes = match std::fs::read(&shown) {
        Ok(b) => b,
        Err(e) => {
            let code = e.raw_os_error().unwrap_or(libc::EIO);
            return Err(match code {
                libc::ENOENT => LoadErr { code: "MANIFEST_FILE_NOT_FOUND", message: format!("manifest file not found: {shown}") },
                libc::EISDIR => LoadErr {
                    code: "MANIFEST_IS_A_DIRECTORY",
                    message: format!("manifest path is a directory, not a file: {shown}"),
                },
                _ => LoadErr {
                    code: "MANIFEST_IO_ERROR",
                    message: format!("could not read manifest file {shown}: {}", fsx::os_error_text(code, &shown)),
                },
            });
        }
    };
    agent_runtime_support::pyjson::decode_file_text(&bytes).map_err(|e| LoadErr {
        code: "MANIFEST_INVALID_UTF8",
        message: format!("manifest file is not valid UTF-8: {e}"),
    })
}

fn parse_manifest_json(text: &PyStr) -> Result<PyValue, Result<LoadErr, String>> {
    match agent_runtime_support::pyjson::loads_text(text) {
        Ok(v) => Ok(v),
        Err(e @ agent_runtime_support::pyjson::LoadError::Decode { .. }) => Err(Ok(LoadErr {
            code: "MANIFEST_INVALID_JSON",
            message: format!("manifest is not valid JSON: {e}"),
        })),
        Err(e) => Err(Err(format!("{e}"))),
    }
}

fn load_manifest(path_arg: &str) -> Result<PyValue, Result<LoadErr, String>> {
    let text = read_manifest_text(path_arg).map_err(Ok)?;
    parse_manifest_json(&text)
}

/// `_find_repo_root`.
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let cur = resolve_lenient(start);
    let mut c: Option<&Path> = Some(cur.as_path());
    while let Some(p) = c {
        if p.join(".git").exists() {
            return Some(p.to_path_buf());
        }
        c = p.parent();
    }
    None
}

/// `_is_valid_utc_timestamp`.
pub fn is_valid_utc_timestamp(v: Option<&PyValue>) -> bool {
    let text = match as_str(v) {
        Some(t) => t,
        None => return false,
    };
    let mut t: &str = &text;
    if let Some(x) = t.strip_suffix('\n') {
        t = x;
    }
    let cs: Vec<char> = t.chars().collect();
    let dig = |c: char| agent_runtime_support::pystr::decimal_digit_value(c as u32);
    let num = |a: usize, b: usize| -> Option<u32> {
        let mut n = 0u32;
        for c in &cs[a..b] {
            n = n * 10 + dig(*c)?;
        }
        Some(n)
    };
    if cs.len() < 20 {
        return false;
    }
    let seps = [(4, '-'), (7, '-'), (10, 'T'), (13, ':'), (16, ':')];
    if seps.iter().any(|(i, c)| cs[*i] != *c) {
        return false;
    }
    let fields = [(0, 4), (5, 7), (8, 10), (11, 13), (14, 16), (17, 19)];
    let mut vals = [0u32; 6];
    for (k, (a, b)) in fields.iter().enumerate() {
        match num(*a, *b) {
            Some(n) => vals[k] = n,
            None => return false,
        }
    }
    let rest = &cs[19..];
    let ok_tail = if rest == ['Z'] {
        true
    } else if rest.first() == Some(&'.') && rest.last() == Some(&'Z') {
        let frac = &rest[1..rest.len() - 1];
        (1..=6).contains(&frac.len()) && frac.iter().all(|c| dig(*c).is_some())
    } else {
        false
    };
    if !ok_tail {
        return false;
    }
    let [y, mo, d, h, mi, se] = vals;
    if !(1..=9999).contains(&y) || !(1..=12).contains(&mo) || h > 23 || mi > 59 || se > 59 {
        return false;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(mo - 1) as usize];
    (1..=dim).contains(&d)
}

fn ascii_token(t: &str, extra: &[u8]) -> bool {
    t.bytes().all(|b| b.is_ascii_alphanumeric() || extra.contains(&b))
}

fn first_alnum(t: &str) -> bool {
    t.as_bytes().first().is_some_and(|b| b.is_ascii_alphanumeric())
}

fn propagation_scope_error(scope: &PyValue) -> Option<(&'static str, &'static str)> {
    let node_id = as_str(get(scope, "node_id")).unwrap_or_default();
    let ssh_host = as_str(get(scope, "ssh_host")).unwrap_or_default();
    let work_dir = as_str(get(scope, "work_dir")).unwrap_or_default();
    if !(first_alnum(&node_id) && ascii_token(&node_id, b"._-")) {
        return Some(("PROPAGATION_AUTHORIZATION_NODE_ID_INVALID", "node_id must contain no whitespace or shell metacharacters"));
    }
    let host_ok = match ssh_host.split_once('@') {
        Some((u, h)) => first_alnum(u) && ascii_token(u, b"._-") && first_alnum(h) && ascii_token(h, b".-"),
        None => false,
    };
    if !host_ok || ssh_host.contains("..") {
        return Some(("PROPAGATION_AUTHORIZATION_SSH_HOST_INVALID", "ssh_host must be a normalized user@host token"));
    }
    let wd_ok = work_dir.starts_with('/')
        && work_dir.len() > 1
        && work_dir[1..].split('/').all(|seg| !seg.is_empty() && ascii_token(seg, b"._-"));
    if !wd_ok
        || work_dir.contains("//")
        || work_dir.contains("/./")
        || work_dir.contains("/../")
        || work_dir.ends_with("/.")
        || work_dir.ends_with("/..")
    {
        return Some(("PROPAGATION_AUTHORIZATION_WORK_DIR_INVALID", "work_dir must be a normalized non-root absolute safe path"));
    }
    let expected: PyValue =
        if eq_str(get(scope, "topology"), "single") { list_s(&["overlay"]) } else { list_s(&["band2", "overlay"]) };
    if !get(scope, "planes").is_some_and(|p| py_eq(p, &expected)) {
        return Some(("PROPAGATION_AUTHORIZATION_PLANES_INVALID", "planes must be canonical for topology"));
    }
    None
}

fn propagation_scope_atoms(scope: &PyValue) -> Vec<String> {
    let f = |k: &str| get(scope, k).map(display).unwrap_or_default();
    let planes: Vec<String> = match get(scope, "planes").map(|p| p.peel()) {
        Some(PyValue::List(items)) => items.iter().map(display).collect(),
        _ => vec![],
    };
    vec![
        format!("propagation_topology: {}", f("topology")),
        format!("propagation_node_id: {}", f("node_id")),
        format!("propagation_ssh_host: {}", f("ssh_host")),
        format!("propagation_work_dir: {}", f("work_dir")),
        format!("propagation_planes: {}", planes.join(",")),
    ]
}

/// `_execution_approval_section_atoms`.
fn execution_approval_section_atoms(plan: &[u8], anchor: &str) -> Option<Vec<String>> {
    let text = std::str::from_utf8(plan).ok()?;
    let lines = pytext::splitlines(text);
    let mut sections: Vec<Vec<String>> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for (i, line) in lines.iter().enumerate() {
        let stripped = line.trim_start_matches([' ', '\t']);
        let marker = stripped.chars().next();
        let run = match marker {
            Some(m @ ('`' | '~')) => stripped.chars().take_while(|c| *c == m).count(),
            _ => 0,
        };
        let remainder: String = stripped.chars().skip(run).collect();
        let valid_opener = run >= 3 && (marker != Some('`') || !remainder.contains('`'));
        if fence.is_none() && valid_opener {
            fence = Some((marker.unwrap(), run));
            continue;
        }
        if let Some((fm, fl)) = fence {
            if marker == Some(fm) && run >= fl && remainder.trim_matches([' ', '\t']).is_empty() {
                fence = None;
            }
            continue;
        }
        if *line == anchor {
            let mut section = Vec::new();
            for cand in &lines[i + 1..] {
                if cand.starts_with('#') {
                    break;
                }
                if !pytext::strip(cand).is_empty() {
                    section.push(cand.to_string());
                }
            }
            sections.push(section);
        }
    }
    if sections.len() == 1 {
        sections.pop()
    } else {
        None
    }
}

fn plan_path_reason(status: &str) -> &'static str {
    match status {
        "repo_escape" => "EXECUTION_APPROVAL_PLAN_PATH_ESCAPES_REPO",
        "symlink" => "EXECUTION_APPROVAL_PLAN_PATH_CONTAINS_SYMLINK",
        "not_found" | "not_a_directory" => "EXECUTION_APPROVAL_PLAN_PATH_NOT_FOUND",
        "wrong_type" => "EXECUTION_APPROVAL_PLAN_PATH_WRONG_TYPE",
        _ => "EXECUTION_APPROVAL_PLAN_PATH_UNREADABLE",
    }
}

/// `(task_class_echo, identity_echo)`.
fn echo(manifest: &PyValue) -> (PyValue, PyValue) {
    let tc = get(manifest, "task_class").filter(|v| matches!(v.peel(), PyValue::Str(_))).cloned().unwrap_or(PyValue::None);
    let id = get(manifest, "identity").filter(|v| matches!(v.peel(), PyValue::Dict(_))).cloned().unwrap_or(PyValue::None);
    (tc, id)
}

/// Reason collector (`reason_codes` list + `messages` dict).
#[derive(Default)]
struct Reasons {
    codes: Vec<String>,
    messages: Vec<(String, String)>,
}

impl Reasons {
    fn add(&mut self, code: impl Into<String>, msg: impl Into<String>) {
        let code = code.into();
        let msg = msg.into();
        self.codes.push(code.clone());
        if let Some(slot) = self.messages.iter_mut().find(|(c, _)| *c == code) {
            slot.1 = msg;
        } else {
            self.messages.push((code, msg));
        }
    }
    fn sorted_codes(&self) -> PyValue {
        let mut c = self.codes.clone();
        c.sort();
        list_s(&c)
    }
    fn messages(&self) -> PyValue {
        let mut d = PyDict::new();
        for (c, m) in &self.messages {
            d.set(c, s(m));
        }
        PyValue::Dict(d)
    }
}

fn shape_violations(manifest: &PyValue, schema: &PyValue) -> Result<Vec<(String, String)>, String> {
    validate_against_schema(manifest, schema).map_err(|e| format!("{}: {}", e.class, e.message))
}

fn violations_obj(v: &[(String, String)]) -> (PyValue, PyValue) {
    let mut codes: Vec<String> = v.iter().map(|(c, _)| c.clone()).collect();
    codes.sort();
    let mut d = PyDict::new();
    for (c, m) in v {
        d.set(c, s(m));
    }
    (list_s(&codes), PyValue::Dict(d))
}

fn open_dir(p: &Path) -> Result<Fd, i32> {
    fsx::open_path(p, libc::O_RDONLY | libc::O_DIRECTORY)
}

/// The `authorize --mode experimental` body.
fn authorize_experimental(sch: &Schemas, mode: &str, action: &str, manifest_arg: &str, repo_root: &Path) -> R {
    let manifest = match load_manifest(manifest_arg) {
        Ok(m) => m,
        Err(Ok(e)) => return emit_authorization(sch, authorize_bare_result(Some(mode), Some(action), e.code, &e.message), 2),
        Err(Err(fatal)) => return Err(fatal),
    };
    let sv = shape_violations(&manifest, &sch.work)?;
    if !sv.is_empty() {
        let (tc, id) = echo(&manifest);
        let (codes, msgs) = violations_obj(&sv);
        return emit_authorization(
            sch,
            D::new()
                .put("schema_version", PyValue::int(SCHEMA_VERSION))
                .put("mode", s(mode))
                .put("action", s(action))
                .put("task_class", tc)
                .put("authorization_state", PyValue::None)
                .put("allowed", PyValue::Bool(false))
                .put("reason_codes", codes)
                .put("messages", msgs)
                .put("identity", id)
                .v(),
            2,
        );
    }
    let task_class = get(&manifest, "task_class").cloned().unwrap_or(PyValue::None);
    let identity = get(&manifest, "identity").cloned().unwrap_or(PyValue::None);
    let evidence = get(&manifest, "evidence").filter(|e| e.is_truthy()).cloned().unwrap_or(PyValue::Dict(PyDict::new()));
    let ea = get(&manifest, "execution_approval").filter(|v| !matches!(v.peel(), PyValue::None)).cloned();
    let mut rs = Reasons::default();
    let fail = |rs: &Reasons, code: i32| {
        emit_authorization(
            sch,
            D::new()
                .put("schema_version", PyValue::int(SCHEMA_VERSION))
                .put("mode", s(mode))
                .put("action", s(action))
                .put("task_class", task_class.clone())
                .put("authorization_state", PyValue::None)
                .put("allowed", PyValue::Bool(false))
                .put("reason_codes", rs.sorted_codes())
                .put("messages", rs.messages())
                .put("identity", identity.clone())
                .v(),
            code,
        )
    };
    let pa = get(&manifest, "propagation_authorization").filter(|v| !matches!(v.peel(), PyValue::None)).cloned();
    let pa_active = action == "sync_to_sub" && pa.is_some();
    if pa_active {
        let pa = pa.as_ref().unwrap();
        let ea = match &ea {
            None => {
                rs.add("PROPAGATION_AUTHORIZATION_EXECUTION_APPROVAL_ABSENT", "propagation_authorization requires a verified execution_approval");
                return fail(&rs, 1);
            }
            Some(e) => e,
        };
        if !is_valid_utc_timestamp(get(pa, "approved_utc")) {
            rs.add("PROPAGATION_AUTHORIZATION_TIMESTAMP_INVALID", "propagation_authorization.approved_utc is not a valid UTC timestamp");
            return fail(&rs, 2);
        }
        if let Some((c, m)) = propagation_scope_error(pa) {
            rs.add(c, m);
            return fail(&rs, 2);
        }
        if !py_eq(get(pa, "approved_utc").unwrap_or(&PyValue::None), get(ea, "approved_at_utc").unwrap_or(&PyValue::None)) {
            rs.add("PROPAGATION_AUTHORIZATION_TIMESTAMP_MISMATCH", "propagation authorization time must equal execution approval time");
            return fail(&rs, 2);
        }
        if !py_eq(get(pa, "topology").unwrap_or(&PyValue::None), get(&identity, "topology").unwrap_or(&PyValue::None)) {
            rs.add("PROPAGATION_AUTHORIZATION_TOPOLOGY_MISMATCH", "propagation_authorization.topology must equal identity.topology");
            return fail(&rs, 2);
        }
    }
    let ea = match ea {
        None => {
            rs.add("EXECUTION_APPROVAL_ABSENT", "manifest has no execution_approval or approved propagation_authorization block");
            return fail(&rs, 1);
        }
        Some(e) => e,
    };
    let plan_path = as_str(get(&ea, "plan_path")).unwrap_or_default();
    if is_absolute_path_string(&plan_path) {
        rs.add(
            "EXECUTION_APPROVAL_PLAN_PATH_ABSOLUTE",
            format!("execution_approval.plan_path {} must be relative to the manifest, not absolute", repr(&plan_path)),
        );
        return fail(&rs, 2);
    }
    let approved_at = get(&ea, "approved_at_utc");
    if !is_valid_utc_timestamp(approved_at) {
        rs.add(
            "EXECUTION_APPROVAL_TIMESTAMP_INVALID",
            format!(
                "execution_approval.approved_at_utc {} is not a valid UTC timestamp (expected YYYY-MM-DDTHH:MM:SS[.ffffff]Z)",
                repr_opt(approved_at)
            ),
        );
        return fail(&rs, 2);
    }
    if let Some(pe) = get(&evidence, "plan").filter(|p| p.is_truthy()) {
        if let Some(pp) = get(pe, "path").filter(|p| p.is_truthy()) {
            if !py_eq(pp, &s(&plan_path)) {
                rs.add(
                    "EXECUTION_APPROVAL_PLAN_PATH_MISMATCH",
                    format!("execution_approval.plan_path {} does not match evidence.plan.path {}", repr(&plan_path), pp.repr()),
                );
                return fail(&rs, 2);
            }
        }
    }
    let root_fd = open_dir(repo_root).map_err(|e| {
        format!(
            "Traceback (most recent call last):\n{}",
            fsx::os_error_text(e, &repo_root.to_string_lossy())
        )
    })?;
    let manifest_dir = resolve_lenient(Path::new(&pytext::pure_path_str(manifest_arg))).parent().map(Path::to_path_buf).unwrap_or_default();
    let r = resolve_and_stat_evidence(&root_fd, repo_root, &manifest_dir, &plan_path, false, true);
    drop(root_fd);
    if !r.ok() {
        rs.add(
            plan_path_reason(r.status),
            format!(
                "execution_approval.plan_path {} could not be resolved as a safe, existing, non-empty regular file (status={})",
                repr(&plan_path),
                r.status
            ),
        );
        return fail(&rs, 2);
    }
    if !eq_str(get(&ea, "plan_sha256"), r.sha256.as_deref().unwrap_or("")) {
        rs.add("EXECUTION_APPROVAL_PLAN_DIGEST_MISMATCH", "execution_approval.plan_sha256 does not match the resolved plan bytes");
        return fail(&rs, 2);
    }
    let mut expected: Vec<String> = vec![
        format!("approved_by: {}", get(&ea, "approved_by").map(display).unwrap_or_default()),
        format!("approved_at_utc: {}", get(&ea, "approved_at_utc").map(display).unwrap_or_default()),
    ];
    let allowed: Vec<PyValue> = match get(&ea, "allowed_actions").map(|x| x.peel()) {
        Some(PyValue::List(items)) => items.clone(),
        _ => vec![],
    };
    for a in &allowed {
        expected.push(format!("allowed_action: {}", display(a)));
    }
    if pa_active {
        expected.extend(propagation_scope_atoms(pa.as_ref().unwrap()));
    }
    let expected_v = list_s(&expected);
    if !get(&ea, "approval_atoms").is_some_and(|x| py_eq(x, &expected_v)) {
        rs.add("EXECUTION_APPROVAL_ATOMS_INVALID", "approval_atoms must exactly equal the deterministic atoms derived from approver, time, and actions");
        return fail(&rs, 2);
    }
    let plan_bytes = match r.content_bytes {
        Some(b) => b,
        None => {
            rs.add("EXECUTION_APPROVAL_PLAN_PATH_UNREADABLE", "resolved plan produced no content bytes to verify the approval anchor against");
            return fail(&rs, 2);
        }
    };
    if std::str::from_utf8(&plan_bytes).is_err() {
        rs.add("EXECUTION_APPROVAL_PLAN_UTF8_INVALID", "execution approval plan must be valid UTF-8");
        return fail(&rs, 2);
    }
    let anchor = as_str(get(&ea, "approval_anchor")).unwrap_or_default();
    if execution_approval_section_atoms(&plan_bytes, &anchor).as_deref() != Some(&expected[..]) {
        rs.add(
            "EXECUTION_APPROVAL_PLAN_ATOMS_MISSING",
            "plan must contain exactly one non-fenced Execution approval section with contiguous canonical approval atoms",
        );
        return fail(&rs, 2);
    }
    if !is_true(get(&ea, "approved")) {
        rs.add("EXECUTION_APPROVAL_NOT_APPROVED", "execution_approval.approved is not true");
        return fail(&rs, 1);
    }
    if !allowed.iter().any(|a| py_eq(a, &s(action))) {
        rs.add(
            "EXECUTION_APPROVAL_ACTION_NOT_ALLOWED",
            format!(
                "action {} is not in execution_approval.allowed_actions {}",
                repr(action),
                repr_opt(get(&ea, "allowed_actions"))
            ),
        );
        return fail(&rs, 1);
    }
    if pa_active && !get(pa.as_ref().unwrap(), "approval_atoms").is_some_and(|x| py_eq(x, &expected_v)) {
        rs.add(
            "PROPAGATION_AUTHORIZATION_APPROVAL_ATOMS_MISMATCH",
            "propagation_authorization.approval_atoms must exactly repeat execution_approval approval atoms",
        );
        return fail(&rs, 2);
    }
    emit_authorization(
        sch,
        D::new()
            .put("schema_version", PyValue::int(SCHEMA_VERSION))
            .put("mode", s(mode))
            .put("action", s(action))
            .put("task_class", task_class.clone())
            .put("authorization_state", s("execution-approved"))
            .put("allowed", PyValue::Bool(true))
            .put("reason_codes", PyValue::List(vec![]))
            .put("messages", D::new().v())
            .put("identity", identity.clone())
            .put("propagation_scope", if pa_active { pa.unwrap() } else { PyValue::None })
            .v(),
        0,
    )
}

/// Run a child with a timeout; Ok((code, stdout, stderr)) or Err(message) (spawn failure / timeout).
pub fn run_capture(
    mut cmd: std::process::Command,
    timeout_s: u64,
) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    use std::io::Read;
    use std::process::Stdio;
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let to = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let te = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_s);
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Command timed out after {timeout_s} seconds"));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let o = to.join().unwrap_or_default();
    let e = te.join().unwrap_or_default();
    use std::os::unix::process::ExitStatusExt;
    let code = status.code().unwrap_or_else(|| -status.signal().unwrap_or(0));
    Ok((code, o, e))
}

/// The `authorize --mode promotion` body: run this program's own `verify` as a read-only child.
fn authorize_promotion(sch: &Schemas, env: &Env, mode: &str, action: &str, manifest_arg: &str, repo_root: &Path) -> R {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg0(&env.argv0);
    cmd.args(["verify", "--manifest", &pytext::pure_path_str(manifest_arg), "--repo-root", &repo_root.to_string_lossy()]);
    let (rc, out, err) = match run_capture(cmd, 120) {
        Ok(x) => x,
        Err(e) => {
            return emit_authorization(
                sch,
                authorize_bare_result(Some(mode), Some(action), "PROMOTION_GATE_VERIFY_UNAVAILABLE", &format!("could not execute internal verify evaluation: {e}")),
                2,
            )
        }
    };
    let out_text = String::from_utf8_lossy(&out).into_owned();
    let verify_out = match agent_runtime_support::pyjson::loads(&out_text) {
        Ok(v) => v,
        Err(_) => {
            let e = String::from_utf8_lossy(&err).into_owned();
            let e = pytext::strip(&e);
            let trunc: String = e.chars().take(500).collect();
            return emit_authorization(
                sch,
                authorize_bare_result(
                    Some(mode),
                    Some(action),
                    "PROMOTION_GATE_VERIFY_UNREADABLE",
                    &format!(
                        "internal verify subprocess (exit={rc}) did not produce parseable JSON stdout; stderr={}",
                        repr(&trunc)
                    ),
                ),
                2,
            );
        }
    };
    let (tc, id) = echo(&verify_out);
    let mut verify_codes: Vec<String> = match get(&verify_out, "reason_codes").map(|x| x.peel()) {
        Some(PyValue::List(items)) => items.iter().filter_map(|x| as_str(Some(x))).collect(),
        _ => vec![],
    };
    let verify_messages: PyDict = match get(&verify_out, "messages").map(|x| x.peel()) {
        Some(PyValue::Dict(d)) => d.clone(),
        _ => PyDict::new(),
    };
    let exit_is = |n: i64| get(&verify_out, "exit_code").is_some_and(|x| py_eq(x, &PyValue::int(n)));
    let eligible = rc == 0 && exit_is(0) && eq_str(get(&verify_out, "state"), "promotion-ready") && is_true(get(&verify_out, "eligible_for_promotion"));
    if eligible {
        return emit_authorization(
            sch,
            D::new()
                .put("schema_version", PyValue::int(SCHEMA_VERSION))
                .put("mode", s(mode))
                .put("action", s(action))
                .put("task_class", tc)
                .put("authorization_state", s("promotion-ready"))
                .put("allowed", PyValue::Bool(true))
                .put("reason_codes", PyValue::List(vec![]))
                .put("messages", D::new().v())
                .put("identity", id)
                .v(),
            0,
        );
    }
    let invalid = rc == 2 || exit_is(2);
    let wrap_code = if invalid { "PROMOTION_GATE_INVALID_MANIFEST" } else { "PROMOTION_GATE_NOT_ELIGIBLE" };
    let wrap_message = if invalid {
        "underlying verify evaluation reported invalid input -- see other reason codes for detail".to_string()
    } else {
        format!(
            "underlying verify state={} eligible_for_promotion={} -- promotion side effects blocked",
            repr_opt(get(&verify_out, "state")),
            repr_opt(get(&verify_out, "eligible_for_promotion"))
        )
    };
    let mut messages = verify_messages;
    messages.set(wrap_code, s(&wrap_message));
    verify_codes.push(wrap_code.to_string());
    verify_codes.sort();
    verify_codes.dedup();
    emit_authorization(
        sch,
        D::new()
            .put("schema_version", PyValue::int(SCHEMA_VERSION))
            .put("mode", s(mode))
            .put("action", s(action))
            .put("task_class", tc)
            .put("authorization_state", PyValue::None)
            .put("allowed", PyValue::Bool(false))
            .put("reason_codes", list_s(&verify_codes))
            .put("messages", PyValue::Dict(messages))
            .put("identity", id)
            .v(),
        if invalid { 2 } else { 1 },
    )
}

/// `_authorize_tripwire_failure`. The pre-commit tripwires run through the Rust `runtime_selftest.sh` launcher
/// beside this program (the Python `runtime_selftest.py` is no longer started). Diagnostics only on stderr.
fn authorize_tripwire_failure(env: &Env, repo_root: &Path) -> Result<Option<String>, String> {
    let selftest = env.script_dir().join("runtime_selftest.sh");
    if !selftest.is_file() {
        eprintln!(
            "[authorize] tripwire SKIPPED (runtime_selftest.sh absent beside completion_gate.py — isolated fixture copy); pre-side-effect tripwires NOT run"
        );
        return Ok(None);
    }
    let own_repo = env.repo_of_script();
    if resolve_lenient(repo_root) != own_repo {
        eprintln!(
            "[authorize] tripwire SKIPPED (--repo-root {} is not this file's repo {} — clean-checkout export); pre-side-effect tripwires NOT run",
            repo_root.display(),
            own_repo.display()
        );
        return Ok(None);
    }
    let mut cmd = std::process::Command::new("bash");
    cmd.arg(&selftest).arg("--tripwires-only").current_dir(repo_root);
    let (rc, out, err) = run_capture(cmd, 120).map_err(|e| format!("subprocess.TimeoutExpired: {e}"))?;
    if rc == 0 {
        return Ok(None);
    }
    let text = if !err.is_empty() { String::from_utf8_lossy(&err).into_owned() } else { String::from_utf8_lossy(&out).into_owned() };
    if text.is_empty() {
        return Ok(Some(format!("runtime_selftest --tripwires-only exited {rc}")));
    }
    let stripped = pytext::strip(&text);
    match pytext::splitlines(stripped).last() {
        Some(l) => Ok(Some(l.to_string())),
        None => Err("IndexError: list index out of range".into()),
    }
}

/// `cmd_authorize`.
pub fn cmd_authorize(sch: &Schemas, env: &Env, mode: &str, action: &str, manifest: &str, repo_root_arg: Option<&str>) -> R {
    let repo_root = match repo_root_arg.filter(|r| !r.is_empty()) {
        Some(r) => resolve_lenient(Path::new(r)),
        None => match find_repo_root(&env.script) {
            Some(p) => p,
            None => {
                return emit_authorization(
                    sch,
                    authorize_bare_result(
                        Some(mode),
                        Some(action),
                        "REPO_ROOT_NOT_FOUND",
                        "cannot locate repo root (.git not found) from script location; pass --repo-root explicitly",
                    ),
                    2,
                )
            }
        },
    };
    if let Some(f) = authorize_tripwire_failure(env, &repo_root)? {
        return emit_authorization(
            sch,
            authorize_bare_result(Some(mode), Some(action), "TRIPWIRE_FAILED", &format!("pre-side-effect tripwire failed: {f}")),
            2,
        );
    }
    if mode == "promotion" {
        authorize_promotion(sch, env, mode, action, manifest, &repo_root)
    } else {
        authorize_experimental(sch, mode, action, manifest, &repo_root)
    }
}

/// `_sibling_certificates_with_key`.
fn sibling_certificates_with_key(
    root_fd: &Fd,
    repo_root: &Path,
    manifest_dir: &Path,
    cert_rel: &str,
    run_key: &[String],
) -> Vec<(String, String)> {
    let parts = match lexical_components(manifest_dir, repo_root, cert_rel) {
        Some(p) if !p.is_empty() => p,
        _ => return vec![],
    };
    let (dir_parts, self_name) = (&parts[..parts.len() - 1], &parts[parts.len() - 1]);
    let mut dir_path = repo_root.to_path_buf();
    for p in dir_parts {
        dir_path.push(p);
    }
    let names = match fsx::open_path(&dir_path, libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW) {
        Err(_) => return vec![],
        Ok(fd) => fsx::listdir_fd(fd.raw()).unwrap_or_default(),
    };
    let mut names: Vec<String> = names.iter().map(|n| pytext::pystr_text(&PyStr::from_surrogateescape(n))).collect();
    names.sort_by(|a, b| a.chars().map(u32::from).cmp(b.chars().map(u32::from)));
    let mut out = Vec::new();
    for name in names {
        if name == *self_name || !(name.starts_with("benchmark_") && name.ends_with(".yaml")) {
            continue;
        }
        let mut rel_parts: Vec<String> = dir_parts.to_vec();
        rel_parts.push(name.clone());
        let rel = rel_parts.join("/");
        let r = resolve_and_stat_evidence(root_fd, repo_root, repo_root, &rel, false, true);
        if !r.ok() {
            eprintln!(
                "[completion_gate] WARN sibling certificate {rel} unreadable ({}) -- excluded from run-key resolution (not silently treated as non-matching)",
                r.status
            );
            continue;
        }
        let (fields, ok) = match std::str::from_utf8(r.content_bytes.as_deref().unwrap_or(b"")) {
            Ok(t) => parse_flat_certificate(t),
            Err(_) => (Fields::default(), false),
        };
        if ok && certificate_run_key(&fields).as_deref() == Some(run_key) {
            out.push((rel, r.sha256.unwrap_or_default()));
        }
    }
    out
}

fn resolved_out(r: Option<&Resolved>) -> (PyValue, PyValue, PyValue) {
    match r {
        None => (PyValue::None, PyValue::None, PyValue::None),
        Some(r) => (opt_s(r.kind), opt_s(r.sha256.as_deref()), opt_s(r.tree_sha256.as_deref())),
    }
}

fn mapped_identity(mapped: &[(&'static str, Option<String>)]) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in mapped {
        d.set(k, opt_s(v.as_deref()));
    }
    PyValue::Dict(d)
}

/// `cmd_verify`.
pub fn cmd_verify(sch: &Schemas, env: &Env, manifest_arg: &str, repo_root_arg: Option<&str>) -> R {
    let repo_root = match repo_root_arg.filter(|r| !r.is_empty()) {
        Some(r) => resolve_lenient(Path::new(r)),
        None => match find_repo_root(&env.script) {
            Some(p) => p,
            None => {
                return emit(
                    sch,
                    bare_result(
                        "REPO_ROOT_NOT_FOUND",
                        "cannot locate repo root (.git not found) from script location; pass --repo-root explicitly",
                    ),
                    2,
                )
            }
        },
    };
    let manifest = match load_manifest(manifest_arg) {
        Ok(m) => m,
        Err(Ok(e)) => return emit(sch, bare_result(e.code, &e.message), 2),
        Err(Err(fatal)) => return Err(fatal),
    };
    let sv = shape_violations(&manifest, &sch.work)?;
    if !sv.is_empty() {
        let (tc, id) = echo(&manifest);
        let (codes, msgs) = violations_obj(&sv);
        return emit(
            sch,
            D::new()
                .put("schema_version", PyValue::int(SCHEMA_VERSION))
                .put("task_class", tc)
                .put("state", PyValue::None)
                .put("eligible_for_promotion", PyValue::Bool(false))
                .put("reason_codes", codes)
                .put("messages", msgs)
                .put("checked_evidence", D::new().v())
                .put("identity", id)
                .v(),
            2,
        );
    }
    let task_class_v = get(&manifest, "task_class").cloned().unwrap_or(PyValue::None);
    let task_class = as_str(Some(&task_class_v)).unwrap_or_default();
    let identity = get(&manifest, "identity").cloned().unwrap_or(PyValue::None);
    let empty = PyValue::Dict(PyDict::new());
    let evidence = get(&manifest, "evidence").filter(|e| e.is_truthy()).cloned().unwrap_or_else(|| empty.clone());
    let runtime = get(&manifest, "runtime").cloned().filter(|v| !matches!(v.peel(), PyValue::None));
    let capacity = get(&manifest, "capacity_rejection").cloned().filter(|v| !matches!(v.peel(), PyValue::None));
    let benchmark = get(&manifest, "benchmark").filter(|e| e.is_truthy()).cloned().unwrap_or_else(|| empty.clone());
    let conditions = get(&manifest, "conditions").filter(|e| e.is_truthy()).cloned().unwrap_or_else(|| empty.clone());
    let pii_scan = get(&manifest, "pii_scan").cloned().unwrap_or_else(|| empty.clone());
    let verdict = get(&benchmark, "verdict").cloned();
    let mode = get(&benchmark, "mode").cloned();
    let verdict_ref = verdict.as_ref();

    let mut rs = Reasons::default();
    let mut checked = PyDict::new();
    let base_obj = |rs: &Reasons, checked: &PyDict| {
        D::new()
            .put("schema_version", PyValue::int(SCHEMA_VERSION))
            .put("task_class", task_class_v.clone())
            .put("state", PyValue::None)
            .put("eligible_for_promotion", PyValue::Bool(false))
            .put("reason_codes", rs.sorted_codes())
            .put("messages", rs.messages())
            .put("checked_evidence", PyValue::Dict(checked.clone()))
            .put("identity", identity.clone())
            .v()
    };

    let ev_items: Vec<(String, PyValue)> = match evidence.peel() {
        PyValue::Dict(d) => d.iter().map(|(k, v)| (pytext::pystr_text(k), v.clone())).collect(),
        _ => vec![],
    };
    let item_path = |item: Option<&PyValue>| -> Option<String> {
        let item = item.filter(|i| i.is_truthy())?;
        get(item, "path").filter(|p| p.is_truthy()).and_then(|p| as_str(Some(p)))
    };

    // ---- absolute-path input contract
    let mut absolute: Vec<(String, String)> = Vec::new();
    for (key, item) in &ev_items {
        if let Some(p) = item_path(Some(item)) {
            if is_absolute_path_string(&p) {
                absolute.push((
                    format!("ABSOLUTE_EVIDENCE_PATH:{key}"),
                    format!("evidence '{key}' path {} must be relative to the manifest, not absolute", repr(&p)),
                ));
            }
        }
    }
    let gate_evidence = capacity.as_ref().and_then(|c| get(c, "gate_evidence")).filter(|g| g.is_truthy()).and_then(|g| as_str(Some(g)));
    if let Some(g) = &gate_evidence {
        if is_absolute_path_string(g) {
            absolute.push((
                "ABSOLUTE_CAPACITY_GATE_EVIDENCE_PATH".into(),
                format!("capacity_rejection.gate_evidence path {} must be relative to the manifest, not absolute", repr(g)),
            ));
        }
    }
    let list_of = |v: Option<&PyValue>| -> Vec<PyValue> {
        match v.filter(|x| x.is_truthy()).map(|x| x.peel()) {
            Some(PyValue::List(items)) => items.clone(),
            _ => vec![],
        }
    };
    let scanned_list = list_of(get(&pii_scan, "scanned_paths"));
    for (idx, sp) in scanned_list.iter().enumerate() {
        if let Some(t) = as_str(Some(sp)) {
            if is_absolute_path_string(&t) {
                absolute.push((
                    format!("ABSOLUTE_PII_SCANNED_PATH:{idx}"),
                    format!("pii_scan.scanned_paths[{idx}] {} must be relative to the manifest, not absolute", repr(&t)),
                ));
            }
        }
    }
    let exempt_list = list_of(get(&pii_scan, "exempt_paths"));
    for (idx, ex) in exempt_list.iter().enumerate() {
        if let Some(t) = as_str(get(ex, "path")) {
            if is_absolute_path_string(&t) {
                absolute.push((
                    format!("ABSOLUTE_PII_EXEMPT_PATH:{idx}"),
                    format!("pii_scan.exempt_paths[{idx}].path {} must be relative to the manifest, not absolute", repr(&t)),
                ));
            }
        }
    }
    if !absolute.is_empty() {
        for (c, m) in absolute {
            rs.add(c, m);
        }
        return emit(sch, base_obj(&rs, &checked), 2);
    }

    if truthy(get(&evidence, "certificate")) && !eq_str(verdict_ref, "PASS") && task_class != "hint_map_only" {
        rs.add(
            "CERTIFICATE_PRESENT_WITHOUT_PASS_VERDICT",
            format!(
                "benchmark.verdict={} but evidence.certificate is present (certificates are published PASS-only)",
                repr_opt(verdict_ref)
            ),
        );
        return emit(sch, base_obj(&rs, &checked), 2);
    }

    let root_fd = match open_dir(&repo_root) {
        Ok(fd) => fd,
        Err(e) => {
            let shown = repo_root.to_string_lossy().into_owned();
            return emit(
                sch,
                bare_result("REPO_ROOT_UNREADABLE", &format!("could not open repo root {shown}: {}", fsx::os_error_text(e, &shown))),
                2,
            );
        }
    };
    let manifest_dir = resolve_lenient(Path::new(&pytext::pure_path_str(manifest_arg))).parent().map(Path::to_path_buf).unwrap_or_default();

    let requirement_authority = certificate_requirement_authority(&benchmark);
    let req_auth = requirement_authority.as_deref();
    let certificate_is_waived = certificate_waived(&task_class, verdict_ref, req_auth);
    let required_keys = required_evidence_for(&task_class, Some(&conditions), verdict_ref, req_auth);

    let mut all_keys: Vec<String> = Vec::new();
    for k in required_keys.iter().cloned().chain(ev_items.iter().map(|(k, _)| k.clone())) {
        if !all_keys.contains(&k) {
            all_keys.push(k);
        }
    }
    let ev_get = |k: &str| ev_items.iter().find(|(x, _)| x == k).map(|(_, v)| v);
    let mut resolved: Vec<(String, Option<Resolved>)> = Vec::new();
    let mut escapes: Vec<(String, String)> = Vec::new();
    for key in &all_keys {
        let item = ev_get(key);
        let path = match item_path(item) {
            None => {
                resolved.push((key.clone(), None));
                continue;
            }
            Some(p) => p,
        };
        let need = key == "certificate" || (MARKDOWN_LIKE_EVIDENCE_KEYS.contains(&key.as_str()) && required_keys.contains(key));
        let r = resolve_and_stat_evidence(&root_fd, &repo_root, &manifest_dir, &path, DIR_EVIDENCE_KEYS.contains(&key.as_str()), need);
        if r.status == "repo_escape" {
            escapes.push((
                format!("EVIDENCE_PATH_ESCAPES_REPO:{key}"),
                format!("evidence '{key}' path {} escapes the repo root via '..' traversal", repr(&path)),
            ));
        } else if r.status == "symlink" {
            escapes.push((
                format!("EVIDENCE_PATH_CONTAINS_SYMLINK:{key}"),
                format!("evidence '{key}' path {} contains a symlink at some path component", repr(&path)),
            ));
        }
        resolved.push((key.clone(), Some(r)));
    }
    let res_get = |k: &str| -> Option<&Resolved> { resolved.iter().find(|(x, _)| x == k).and_then(|(_, r)| r.as_ref()) };
    let mut cr_gate: Option<Resolved> = None;
    if let Some(g) = &gate_evidence {
        let r = resolve_and_stat_evidence(&root_fd, &repo_root, &manifest_dir, g, false, false);
        if r.status == "repo_escape" {
            escapes.push((
                "EVIDENCE_PATH_ESCAPES_REPO:capacity_rejection.gate_evidence".into(),
                format!("capacity_rejection.gate_evidence path {} escapes the repo root via '..' traversal", repr(g)),
            ));
        } else if r.status == "symlink" {
            escapes.push((
                "EVIDENCE_PATH_CONTAINS_SYMLINK:capacity_rejection.gate_evidence".into(),
                format!("capacity_rejection.gate_evidence path {} contains a symlink at some path component", repr(g)),
            ));
        }
        cr_gate = Some(r);
    }
    if !escapes.is_empty() {
        for (c, m) in escapes {
            rs.add(c, m);
        }
        return emit(sch, base_obj(&rs, &checked), 2);
    }
    if let Some(c) = &capacity {
        let (ty, sha, tree) = resolved_out(cr_gate.as_ref());
        checked.set(
            "capacity_rejection.gate_evidence",
            D::new()
                .put("required", PyValue::Bool(task_class == "capacity_rejection"))
                .put("path", get(c, "gate_evidence").cloned().unwrap_or(PyValue::None))
                .put("exists", PyValue::Bool(cr_gate.as_ref().is_some_and(|r| r.ok())))
                .put("type", ty)
                .put("sha256", sha)
                .put("tree_sha256", tree)
                .put("identity_checked", PyValue::Bool(false))
                .put("identity_match", PyValue::None)
                .v(),
        );
    }

    // ---- runtime tier
    let mut runtime_ok = false;
    if task_class == "capacity_rejection" {
        runtime_ok = capacity.as_ref().is_some_and(|c| is_true(get(c, "rejected")))
            && cr_gate.as_ref().is_some_and(|r| r.ok());
        if !runtime_ok {
            rs.add("PRE_RUNTIME_NOT_HEALTHY", "no valid capacity_rejection substitute (rejected != true, or gate_evidence missing/unresolvable)");
        }
    } else {
        let rt = runtime.as_ref().filter(|r| r.is_truthy());
        let containers = rt.map(|r| list_of(get(r, "containers"))).unwrap_or_default();
        if rt.is_none() || !is_true(rt.and_then(|r| get(r, "health_ok"))) {
            rs.add("PRE_RUNTIME_NOT_HEALTHY", "runtime.health_ok is not true");
        } else if containers.iter().any(|c| truthy(get(c, "oom_killed"))) {
            rs.add("PRE_RUNTIME_NOT_HEALTHY", "a runtime container was oom_killed");
        } else if !is_true(rt.and_then(|r| get(r, "functional_smoke_passed"))) {
            rs.add("RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED", "runtime.functional_smoke_passed is not true");
        } else {
            match rt.and_then(|r| get_dict(r, "identity")) {
                None => rs.add("RUNTIME_IDENTITY_MISSING", "runtime.identity is absent"),
                Some(rid) => {
                    let none = PyValue::None;
                    let mismatched: Vec<&str> = STRONG_IDENTITY_FIELDS
                        .iter()
                        .copied()
                        .filter(|f| !py_eq(get(rid, f).unwrap_or(&none), get(&identity, f).unwrap_or(&none)))
                        .collect();
                    if mismatched.is_empty() {
                        runtime_ok = true;
                    } else {
                        for f in mismatched {
                            rs.add(
                                format!("RUNTIME_IDENTITY_MISMATCH:{f}"),
                                format!(
                                    "runtime.identity.{f}={} != manifest identity.{f}={}",
                                    repr_opt(get(rid, f)),
                                    repr_opt(get(&identity, f))
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
    if !runtime_ok {
        return emit(sch, base_obj(&rs, &checked), 1);
    }

    // ---- evidence tier
    let mut all_present = true;
    let mut identity_ok = true;
    let mut links_ok = true;
    let mut certificate_output: Option<PyValue> = None;
    let mut cert_grade_lite_ok = false;
    let mut cert_is_lite = false;
    let mut cert_floor: Option<f64> = None;
    let mut cert_ratio: Option<f64> = None;
    let mut cert_primary: Option<String> = None;
    let mut cert_rubric_ok = false;
    let mut cert_rubric_authority: Option<String> = None;

    for key in &all_keys {
        let item = ev_get(key);
        let required = required_keys.contains(key);
        let r = res_get(key);
        let exists = r.is_some_and(|r| r.ok());
        let mut identity_checked = false;
        let mut identity_match = PyValue::None;
        let ipath = item.and_then(|i| get(i, "path")).cloned().unwrap_or(PyValue::None);
        let ipath_repr = ipath.repr();

        if key == "certificate" && exists {
            let r = r.unwrap();
            identity_checked = true;
            let content = r.content_bytes.as_deref().unwrap_or(b"");
            let (cf, parse_ok) = match std::str::from_utf8(content) {
                Ok(t) => parse_flat_certificate(t),
                Err(_) => (Fields::default(), false),
            };
            if !parse_ok {
                identity_ok = false;
                identity_match = PyValue::Bool(false);
                rs.add("CERTIFICATE_UNPARSEABLE", format!("certificate at {ipath_repr} is not in the recognized flat-scalar format"));
                certificate_output = Some(
                    D::new()
                        .put("path", ipath.clone())
                        .put("parsed", PyValue::Bool(false))
                        .put("identity", PyValue::None)
                        .put("verdict", PyValue::None)
                        .put("benchmark_mode", PyValue::None)
                        .put("identity_match", PyValue::None)
                        .put("rubric_authority", PyValue::None)
                        .put("floor_tps", PyValue::None)
                        .put("ratio_M_over_primary", PyValue::None)
                        .put("primary_source", PyValue::None)
                        .put("rubric_contract_ok", PyValue::Bool(false))
                        .put("run_key", PyValue::None)
                        .put("duplicates", PyValue::List(vec![]))
                        .v(),
                );
            } else {
                let (mapped, mismatched) = certificate_strong_identity_matches(&cf, &identity);
                identity_match = PyValue::Bool(mismatched.is_empty());
                if !mismatched.is_empty() {
                    identity_ok = false;
                    for f in &mismatched {
                        let cert_key = CERTIFICATE_FIELD_MAP.iter().find(|(m, _)| m == f).unwrap().1;
                        let mv = mapped.iter().find(|(m, _)| m == f).and_then(|(_, v)| v.as_deref());
                        rs.add(
                            format!("IDENTITY_MISMATCH:{f}"),
                            format!(
                                "certificate {cert_key}={} != manifest identity.{f}={}",
                                mv.map(repr).unwrap_or_else(|| "None".into()),
                                repr_opt(get(&identity, f))
                            ),
                        );
                    }
                }
                let run_key = certificate_run_key(&cf);
                let mut duplicates: Vec<(String, String)> = vec![];
                match &run_key {
                    None => {
                        identity_ok = false;
                        rs.add(
                            "CERTIFICATE_RUN_KEY_UNRESOLVABLE",
                            format!("certificate at {ipath_repr} lacks a strong identity field or measured_utc -- the measurement it certifies cannot be identified (fail-closed)"),
                        );
                    }
                    Some(rk) => {
                        let declared = get(&benchmark, "measured_utc").filter(|v| v.is_truthy());
                        if let Some(d) = declared {
                            if !py_eq(d, &s(&rk[rk.len() - 1])) {
                                identity_ok = false;
                                rs.add(
                                    "CERTIFICATE_RUN_DRIFT",
                                    format!(
                                        "manifest benchmark.measured_utc={} != certificate measured_utc={} -- the manifest binds a different measurement than the file at {ipath_repr} certifies",
                                        d.repr(),
                                        repr(&rk[rk.len() - 1])
                                    ),
                                );
                            }
                        }
                        let cpath = as_str(Some(&ipath)).unwrap_or_default();
                        duplicates = sibling_certificates_with_key(&root_fd, &repo_root, &manifest_dir, &cpath, rk);
                        if !duplicates.is_empty() {
                            identity_ok = false;
                            let same = duplicates.iter().all(|(_, d)| Some(d.as_str()) == r.sha256.as_deref());
                            let dpaths: Vec<&str> = duplicates.iter().map(|(p, _)| p.as_str()).collect();
                            rs.add(
                                "CERTIFICATE_RUN_AMBIGUOUS",
                                format!(
                                    "measurement {}@{} is certified by {} tracked files ({ipath_repr} + {}) -- {}",
                                    rk[0],
                                    rk[rk.len() - 1],
                                    duplicates.len() + 1,
                                    repr_list(&dpaths),
                                    if same {
                                        "byte-identical: duplicate layer, delete all but the original"
                                    } else {
                                        "DIFFERENT bytes for the same measurement: publication-chain defect"
                                    }
                                ),
                            );
                        }
                    }
                }
                let cert_verdict = cf.get("verdict");
                let cert_mode = cf.get("benchmark_mode");
                let grade = certificate_grade(&cf);
                let run_key_v = run_key.as_ref().map(|k| list_s(k)).unwrap_or(PyValue::None);
                let dup_v = list_s(&duplicates.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>());
                let opt_repr = |x: Option<&str>| x.map(repr).unwrap_or_else(|| "None".into());
                if grade == Some("lite") {
                    let mut lite_problems: Vec<(&str, String)> = vec![];
                    if cert_verdict != Some(LITE_CERT_VERDICT) {
                        lite_problems.push((
                            "CERTIFICATE_VERDICT_MISMATCH",
                            format!("lite certificate verdict={} (must be {} — verdict 칸은 full 루브릭 전용)", opt_repr(cert_verdict), repr(LITE_CERT_VERDICT)),
                        ));
                    }
                    if cf.get("lite_verdict") != Some("pass") {
                        lite_problems.push((
                            "CERTIFICATE_LITE_VERDICT_NOT_PASS",
                            format!("lite certificate lite_verdict={} (must be 'pass' — lite 불통과는 인증서를 열지 않는다)", opt_repr(cf.get("lite_verdict"))),
                        ));
                    }
                    if task_class != "hint_map_only" {
                        lite_problems.push((
                            "CERTIFICATE_GRADE_NOT_FULL",
                            format!(
                                "task_class={} 에 lite 등급 인증서 — 성능 완료·full 승격은 benchmark_mode=full 인증서만 연다(lite 는 hint 지도 발행의 근거)",
                                repr(&task_class)
                            ),
                        ));
                    }
                    let lite_ok = lite_problems.is_empty();
                    for (c, m) in lite_problems {
                        identity_ok = false;
                        rs.add(c, m);
                    }
                    cert_is_lite = true;
                    cert_grade_lite_ok = lite_ok;
                    certificate_output = Some(
                        D::new()
                            .put("path", ipath.clone())
                            .put("parsed", PyValue::Bool(true))
                            .put("identity", mapped_identity(&mapped))
                            .put("verdict", opt_s(cert_verdict))
                            .put("benchmark_mode", opt_s(cert_mode))
                            .put("grade", s("lite"))
                            .put("lite_verdict", opt_s(cf.get("lite_verdict")))
                            .put("lite_contract_ok", PyValue::Bool(lite_ok))
                            .put("identity_match", identity_match.clone())
                            .put("rubric_authority", PyValue::None)
                            .put("floor_tps", PyValue::None)
                            .put("ratio_M_over_primary", PyValue::None)
                            .put("primary_source", PyValue::None)
                            .put("rubric_contract_ok", PyValue::Bool(false))
                            .put("run_key", run_key_v)
                            .put("duplicates", dup_v)
                            .v(),
                    );
                } else {
                    if task_class == "hint_map_only" {
                        identity_ok = false;
                        rs.add(
                            "CERTIFICATE_GRADE_MISMATCH",
                            format!(
                                "hint_map_only 에 full 등급 인증서(benchmark_mode={}) — 지도 발행 통로는 lite 등급 인증서를 받는다(강등 셀은 full 인증서를 받지 않는다)",
                                opt_repr(cert_mode)
                            ),
                        );
                    }
                    if cert_verdict != Some("PASS") {
                        identity_ok = false;
                        rs.add("CERTIFICATE_VERDICT_MISMATCH", format!("certificate verdict={} (must be 'PASS')", opt_repr(cert_verdict)));
                    }
                    if grade != Some("full") {
                        identity_ok = false;
                        rs.add("CERTIFICATE_BENCHMARK_MODE_MISMATCH", format!("certificate benchmark_mode={} (must be 'full' or absent)", opt_repr(cert_mode)));
                    }
                    let floor = certificate_number_text(cf.get("floor_tps"));
                    let ratio = certificate_number_text(cf.get("ratio_M_over_primary"));
                    let primary = pytext::strip(cf.get("primary_source").unwrap_or("")).to_string();
                    let raw_auth = pytext::strip(cf.get("rubric_authority").unwrap_or("")).to_string();
                    cert_rubric_ok = true;
                    if floor.is_none_or(|f| f <= 0.0) {
                        cert_rubric_ok = false;
                        identity_ok = false;
                        rs.add(
                            "CERTIFICATE_RUBRIC_FLOOR_INVALID",
                            format!(
                                "certificate floor_tps={} is not a finite positive number -- floor<=0 means the rubric imposed NO threshold (공허 PASS): every measurement passes. Not promotable in ANY authority.",
                                opt_repr(cf.get("floor_tps"))
                            ),
                        );
                    }
                    if ratio.is_none() {
                        cert_rubric_ok = false;
                        identity_ok = false;
                        rs.add(
                            "CERTIFICATE_RUBRIC_RATIO_MISSING",
                            format!(
                                "certificate ratio_M_over_primary={} is not a finite number -- the measurement-vs-rubric indicator is absent, so the PASS cannot be audited",
                                opt_repr(cf.get("ratio_M_over_primary"))
                            ),
                        );
                    }
                    if primary.is_empty() || primary.to_uppercase() == "N/A" {
                        cert_rubric_ok = false;
                        identity_ok = false;
                        rs.add(
                            "CERTIFICATE_RUBRIC_SOURCE_MISSING",
                            format!(
                                "certificate primary_source={} is absent -- which rubric slot won cannot be established (surfaced, never matched as a string: the numeric floor/ratio contract above is the gate)",
                                opt_repr(cf.get("primary_source"))
                            ),
                        );
                    }
                    if !raw_auth.is_empty() && raw_auth.to_uppercase() != "N/A" {
                        if RUBRIC_AUTHORITIES.contains(&raw_auth.as_str()) {
                            cert_rubric_authority = Some(raw_auth.clone());
                        } else {
                            cert_rubric_ok = false;
                            identity_ok = false;
                            rs.add(
                                "CERTIFICATE_RUBRIC_AUTHORITY_UNKNOWN",
                                format!(
                                    "certificate rubric_authority={} is not one of {} -- an unknown rubric authority cannot be reasoned about, fail closed",
                                    repr(&raw_auth),
                                    repr_list(&RUBRIC_AUTHORITIES)
                                ),
                            );
                        }
                    }
                    cert_floor = floor;
                    cert_ratio = ratio;
                    cert_primary = if primary.is_empty() { None } else { Some(primary.clone()) };
                    certificate_output = Some(
                        D::new()
                            .put("path", ipath.clone())
                            .put("parsed", PyValue::Bool(true))
                            .put("identity", mapped_identity(&mapped))
                            .put("verdict", opt_s(cert_verdict))
                            .put("benchmark_mode", opt_s(cert_mode))
                            .put("grade", opt_s(grade))
                            .put("identity_match", identity_match.clone())
                            .put("rubric_authority", opt_s(cert_rubric_authority.as_deref()))
                            .put("floor_tps", opt_f(floor))
                            .put("ratio_M_over_primary", opt_f(ratio))
                            .put("primary_source", opt_s(cert_primary.as_deref()))
                            .put("rubric_contract_ok", PyValue::Bool(cert_rubric_ok))
                            .put("run_key", run_key_v)
                            .put("duplicates", dup_v)
                            .v(),
                    );
                }
            }
        }

        if MARKDOWN_LIKE_EVIDENCE_KEYS.contains(&key.as_str()) && required && exists {
            let r = r.unwrap();
            let p = as_str(Some(&ipath)).unwrap_or_default();
            let base_dir = match lexical_components(&manifest_dir, &repo_root, &p) {
                Some(parts) => {
                    let mut b = repo_root.clone();
                    for x in &parts[..parts.len().saturating_sub(1)] {
                        b.push(x);
                    }
                    b
                }
                None => manifest_dir.clone(),
            };
            let (unreadable, broken, invalid) =
                check_required_local_links(&root_fd, &repo_root, &base_dir, r.content_bytes.as_deref().unwrap_or(b""));
            if unreadable {
                links_ok = false;
                rs.add(
                    format!("EVIDENCE_LINK_SCAN_FAILED:{key}"),
                    format!("evidence '{key}' is not valid UTF-8 -- cannot scan for local links, failing closed"),
                );
            } else {
                if !broken.is_empty() {
                    links_ok = false;
                    rs.add(
                        format!("EVIDENCE_BROKEN_LINK:{key}"),
                        format!("evidence '{key}' contains broken local link target(s): {}", repr_list(&broken)),
                    );
                }
                if !invalid.is_empty() {
                    links_ok = false;
                    rs.add(
                        format!("EVIDENCE_INVALID_LINK:{key}"),
                        format!("evidence '{key}' contains invalid local link target(s): {}", repr_list(&invalid)),
                    );
                }
            }
        }

        let (ty, sha, tree) = resolved_out(r);
        checked.set(
            key,
            D::new()
                .put("required", PyValue::Bool(required))
                .put("path", item.filter(|i| i.is_truthy()).and_then(|i| get(i, "path")).cloned().unwrap_or(PyValue::None))
                .put("exists", PyValue::Bool(exists))
                .put("type", ty)
                .put("sha256", sha)
                .put("tree_sha256", tree)
                .put("identity_checked", PyValue::Bool(identity_checked))
                .put("identity_match", identity_match.clone())
                .v(),
        );
        if required && !exists {
            all_present = false;
            let status = r.map(|r| r.status).unwrap_or("absent");
            rs.add(format!("EVIDENCE_MISSING:{key}"), format!("required evidence '{key}' missing/unresolvable (status={status})"));
        } else if required && key == "simlog" && exists {
            let p = as_str(Some(&ipath)).unwrap_or_default();
            let (term, why) = simlog_terminated(&repo_root, &manifest_dir, &p);
            if !term {
                all_present = false;
                rs.add(
                    "EVIDENCE_SIMLOG_RUN_UNTERMINATED",
                    format!(
                        "simlog vault {} 가 **종결을 말하지 않는다** — {why}. 파일이 있다는 것과 run 이 끝났다는 것은 다른 사실이다(2026-09-07 · 유예 결함 ⑥).",
                        ipath_repr
                    ),
                );
            }
        }
    }

    let certificate_carried = res_get("certificate").is_some_and(|r| r.ok());
    let report_table = || -> (Option<Vec<(&'static str, Option<String>)>>, String) {
        match res_get("bench_report") {
            Some(r) if r.ok() => {
                let text = std::str::from_utf8(r.content_bytes.as_deref().unwrap_or(b"")).ok();
                bench_report_verdict_table(text)
            }
            _ => (None, "absent(bench_report 미해소)".into()),
        }
    };
    let verdict_s = verdict_ref.and_then(|v| as_str(Some(v)));
    if certificate_is_waived && !certificate_carried {
        let (table, state) = report_table();
        let corroborated = table.as_ref().is_some_and(|t| {
            let tv = table_get(t, "verdict");
            let ta = table_get(t, "rubric_authority");
            // table values are str|None; verdict is the manifest JSON value
            let v_ok = match (tv, verdict_ref) {
                (Some(a), Some(b)) => py_eq(&s(a), b),
                (None, Some(b)) => matches!(b.peel(), PyValue::None),
                (None, None) => true,
                (Some(_), None) => false,
            };
            v_ok && ta == req_auth
        });
        if !corroborated {
            all_present = false;
            rs.add(
                "CERTIFICATE_WAIVER_UNCORROBORATED",
                format!(
                    "benchmark verdict={} ∧ rubric_authority={}(비발행 권한)라 인증서를 요구하지 않지만, bench_report 판정 표가 그것을 말하지 않는다 (state={state}, table={}) — 면제 근거가 서지 않는다",
                    repr_opt(verdict_ref),
                    req_auth.map(repr).unwrap_or_else(|| "None".into()),
                    table_repr(&table)
                ),
            );
        } else {
            rs.add(
                "CERTIFICATE_WAIVED_NON_ISSUING_AUTHORITY",
                format!(
                    "benchmark verdict='PASS' ∧ rubric_authority={} — 벤치 스킬은 이 판정에 인증서를 발행하지 않는다(자동 발행 = explicit ∧ PASS). 요구 증거는 bench_report 이며 그 판정 표가 같은 verdict·권한을 말한다(교차검증 일치 · state={state})",
                    req_auth.map(repr).unwrap_or_else(|| "None".into())
                ),
            );
        }
    }
    if task_class == "full_benchmark" && verdict_s.as_deref() == Some(OBSERVATION_ONLY_VERDICT) {
        let (table, state) = report_table();
        let ok = table
            .as_ref()
            .is_some_and(|t| matches!(table_get(t, "verdict"), Some(OBSERVATION_ONLY_JUDGE_VERDICT) | Some(OBSERVATION_ONLY_VERDICT)));
        if !ok {
            all_present = false;
            rs.add(
                "BENCHMARK_OBSERVATION_ONLY_UNCORROBORATED",
                format!(
                    "benchmark.verdict='OBSERVATION-ONLY' 인데 bench_report 판정 표가 루브릭 부재({})를 말하지 않는다(state={state}, table={}) — 관측 전용의 근거가 서지 않는다",
                    repr(OBSERVATION_ONLY_JUDGE_VERDICT),
                    table_repr(&table)
                ),
            );
        }
    }

    let mut or_group_key: Option<&str> = None;
    if task_class == "capacity_rejection" {
        let dl = res_get("devlog").is_some_and(|r| r.ok());
        let tl = res_get("testlog").is_some_and(|r| r.ok());
        if !(dl || tl) {
            all_present = false;
            rs.add("EVIDENCE_MISSING:devlog_or_testlog", "capacity_rejection requires at least one of devlog/testlog");
        }
        or_group_key = if dl { Some("devlog") } else if tl { Some("testlog") } else { None };
    }

    let pii_ok = is_true(get(&pii_scan, "passed"));
    let scanned: Vec<PyValue> = scanned_list.clone();
    let in_scanned = |p: &str| scanned.iter().any(|x| py_eq(x, &s(p)));
    let mut exempt: Vec<(String, PyValue)> = Vec::new();
    for e in &exempt_list {
        if let Some(p) = as_str(get(e, "path")).filter(|_| matches!(e.peel(), PyValue::Dict(_))) {
            let reason = get(e, "reason").cloned().unwrap_or(PyValue::None);
            if let Some(slot) = exempt.iter_mut().find(|(k, _)| *k == p) {
                slot.1 = reason;
            } else {
                exempt.push((p, reason));
            }
        }
    }
    let mut pii_keys: Vec<String> = required_keys.clone();
    if let Some(k) = or_group_key {
        pii_keys.push(k.to_string());
    }
    let mut pii_coverage_ok = true;
    for key in &pii_keys {
        let p = match item_path(ev_get(key)) {
            None => continue,
            Some(p) => p,
        };
        if in_scanned(&p) {
            continue;
        }
        let ex = exempt.iter().find(|(k, _)| *k == p).map(|(_, r)| r);
        if PII_EXEMPTIBLE_EVIDENCE_KEYS.contains(&key.as_str()) && ex.is_some_and(|r| eq_str(Some(r), PII_MACHINE_RAW_EXEMPTION_REASON)) {
            continue;
        }
        pii_coverage_ok = false;
        let note = match ex {
            None => String::new(),
            Some(reason) => format!(
                " (exempt_paths entry present, but only {} evidence with reason {} counts as coverage; got key={} reason={})",
                repr_list(&PII_EXEMPTIBLE_EVIDENCE_KEYS),
                repr(PII_MACHINE_RAW_EXEMPTION_REASON),
                repr(key),
                reason.repr()
            ),
        };
        rs.add(
            format!("PII_SCAN_COVERAGE_INCOMPLETE:{key}"),
            format!("required evidence '{key}' path {} not present in pii_scan.scanned_paths{note}", repr(&p)),
        );
    }
    if task_class == "capacity_rejection" {
        if let Some(g) = &gate_evidence {
            if !in_scanned(g) {
                pii_coverage_ok = false;
                rs.add(
                    "PII_SCAN_COVERAGE_INCOMPLETE:capacity_rejection.gate_evidence",
                    format!("required capacity_rejection.gate_evidence path {} not present in pii_scan.scanned_paths", repr(g)),
                );
            }
        }
    }
    if !pii_ok {
        rs.add("PII_SCAN_FAILED", "pii_scan.passed is not true -- promotion/evidence-complete side effects blocked");
    }
    let mut state = if all_present && identity_ok && links_ok && pii_ok && pii_coverage_ok { "evidence-complete" } else { "runtime-ready" };
    let capped = PROMOTION_CAPPED_TASK_CLASSES.contains(&task_class.as_str());
    if state == "evidence-complete" && capped {
        rs.add(
            format!("TASK_CLASS_CAPS_BELOW_PROMOTION:{task_class}"),
            format!("task_class '{task_class}' architecturally cannot reach promotion-ready"),
        );
    }

    // ---- rubric resolution
    let certificate_present = certificate_carried;
    let mr = manifest_rubric_contract(&benchmark);
    let mut rubric_authority = cert_rubric_authority.clone();
    let mut rubric_ok = cert_rubric_ok;
    let mut rubric_floor = cert_floor;
    let mut rubric_ratio = cert_ratio;
    let mut rubric_primary = cert_primary.clone();
    let mut rubric_source: Option<&str> = if certificate_present { Some("certificate") } else { None };
    if certificate_output.is_none() || cert_is_lite {
        rubric_floor = None;
        rubric_ratio = None;
        rubric_primary = None;
    }
    if mr.declared {
        if certificate_present {
            if let (Some(ma), Some(ca)) = (&mr.authority, &cert_rubric_authority) {
                if ma != ca {
                    rs.add(
                        "RUBRIC_AUTHORITY_CARRIER_DISAGREEMENT",
                        format!(
                            "manifest benchmark.rubric_authority={} != certificate rubric_authority={} -- 인증서(디스크 아티팩트)를 정본으로 판정한다. 두 carrier 가 갈라진 것은 발행 체인의 결함이므로 확인하라.",
                            repr(ma),
                            repr(ca)
                        ),
                    );
                }
            }
        } else {
            for (c, m) in &mr.problems {
                if verdict_s.as_deref() == Some(OBSERVATION_ONLY_VERDICT) && OBSERVATION_ONLY_EXPECTED_RUBRIC_GAPS.contains(&c.as_str()) {
                    continue;
                }
                rs.add(c.clone(), m.clone());
            }
            rubric_authority = mr.authority.clone();
            rubric_ok = mr.ok;
            rubric_floor = mr.floor_tps;
            rubric_ratio = mr.ratio;
            rubric_primary = mr.primary_source.clone();
            rubric_source = Some("manifest_benchmark");
        }
    }
    let rubric_output = if certificate_present || mr.declared {
        D::new()
            .put("authority", opt_s(rubric_authority.as_deref()))
            .put("authority_source", opt_s(rubric_source))
            .put("contract_ok", PyValue::Bool(rubric_ok))
            .put("floor_tps", opt_f(rubric_floor))
            .put("ratio_M_over_primary", opt_f(rubric_ratio))
            .put("primary_source", opt_s(rubric_primary.as_deref()))
            .v()
    } else {
        PyValue::None
    };

    // ---- promotion tier
    let mut eligible = false;
    let mode_repr = repr_opt(mode.as_ref());
    let verdict_repr = repr_opt(verdict_ref);
    let src_repr = rubric_source.map(repr).unwrap_or_else(|| "None".into());
    if state == "evidence-complete" && !capped {
        if task_class == "hint_map_only" {
            let lite_cert_ok = certificate_present && certificate_output.is_some() && cert_is_lite && cert_grade_lite_ok;
            if !lite_cert_ok {
                rs.add(
                    "HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE",
                    "hint_map_only 발행 자격은 lite 등급 인증서(benchmark_mode: lite · verdict: not_applicable · lite_verdict: pass)가 연다 — evidence.certificate 가 없거나 lite 등급이 아니다. lite_bench.sh --publish-report(β) 또는 강등 셀 판정(γ)이 발행하고 evidence_publisher.py publish-lite-report --certificate-src 로 바인딩한다.",
                );
            } else {
                eligible = true;
                rs.add(
                    "HINT_MAP_ONLY_PROMOTION",
                    format!(
                        "task_class='hint_map_only' → 지도 발행 통로. mode={mode_repr} verdict={verdict_repr} 는 **관측으로 게재**되며 baseline 승격은 막혀 있다. §3·§4 구성 사실은 hint-publisher hintlib.artifacts 의 면제불가 3슬롯이 그대로 집행하고, 본문 OBSERVATION-ONLY 마커는 hintlib.template 린트(00-hint.md)가 집행한다."
                    ),
                );
            }
        } else if task_class != "full_benchmark" {
            rs.add(
                "PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS",
                format!("task_class={} can never reach promotion-ready -- only 'full_benchmark' may", repr(&task_class)),
            );
        } else if !eq_str(mode.as_ref(), "full") {
            rs.add("BENCHMARK_MODE_NOT_FULL", format!("benchmark.mode={mode_repr} (must be 'full' for promotion)"));
        } else if !eq_str(verdict_ref, "PASS") {
            let smoke_ok = runtime.as_ref().is_some_and(|r| r.is_truthy() && is_true(get(r, "functional_smoke_passed")));
            let annotated = verdict_s.as_deref().is_some_and(|v| ANNOTATED_NON_PASS_VERDICTS.contains(&v));
            if verdict_s.as_deref() == Some(OBSERVATION_ONLY_VERDICT) {
                let mut probs: Vec<String> = vec![];
                if !smoke_ok {
                    probs.push("runtime.functional_smoke_passed 가 true 가 아니다".into());
                }
                let rsrc = get(&benchmark, "rubric_source").filter(|v| v.is_truthy());
                if !rsrc.is_some_and(|v| eq_str(Some(v), MANIFEST_RUBRIC_SOURCES[0])) {
                    probs.push(format!(
                        "benchmark.rubric_source={} — 판정기 산출물 파생 표시({})가 없다",
                        repr_opt(get(&benchmark, "rubric_source")),
                        repr_list(&MANIFEST_RUBRIC_SOURCES)
                    ));
                }
                let mut claimed = PyDict::new();
                for k in ["floor_tps", "ratio_M_over_primary", "primary_source"] {
                    if let Some(v) = get(&benchmark, k) {
                        if !(py_eq(v, &PyValue::None) || py_eq(v, &s(""))) {
                            claimed.set(k, v.clone());
                        }
                    }
                }
                if !claimed.is_empty() {
                    probs.push(format!("루브릭 수치가 실려 있다({}) — 관측 전용 셀은 문턱을 주장하지 않는다", PyValue::Dict(claimed).repr()));
                }
                if !probs.is_empty() {
                    rs.add("BENCHMARK_OBSERVATION_ONLY_INVALID", format!("benchmark.verdict='OBSERVATION-ONLY' 승격 불가 — {}", probs.join(" · ")));
                } else {
                    eligible = true;
                    rs.add(
                        "BENCHMARK_OBSERVATION_ONLY_PROMOTION",
                        "benchmark.verdict='OBSERVATION-ONLY'(기준 c·E·expected 부재 — 판정기 NEEDS_RUBRIC) → 서빙 성립 + bench_report 바인딩으로 **관측 발행**. 판정을 주장하지 않는다(인증서 ✗ · OBSERVATION-ONLY 배너 · 루프 없음).",
                    );
                }
            } else if annotated && rubric_ok && smoke_ok {
                eligible = true;
                rs.add(
                    "BENCHMARK_VERDICT_ANNOTATED",
                    format!(
                        "benchmark.verdict={verdict_repr} · rubric_authority={} (carrier={src_repr}) → 발행 게이트는 성능 판정이 아니라 **서빙 성립(functional_smoke_passed) + 유효 측정**(floor_tps={}, ratio={})이다. 판정은 표시만 된다(태그 배너·카탈로그 판정 열) — loop-until-done 은 성능 게이트로 남는다. 인증서는 미발행.",
                        rubric_authority.as_deref().map(repr).unwrap_or_else(|| "None".into()),
                        float_repr(rubric_floor),
                        float_repr(rubric_ratio)
                    ),
                );
            } else if annotated {
                let mut unmet: Vec<String> = vec![];
                if !smoke_ok {
                    unmet.push("runtime.functional_smoke_passed 가 true 가 아니다".into());
                }
                if !rubric_ok {
                    unmet.push(format!(
                        "rubric 계약 미충족(carrier={src_repr}, floor_tps={}, ratio={}, primary_source={}) — floor 유한 양수 ∧ ratio 유한 ∧ primary_source ∧ rubric_source 출처 표시가 필요하다",
                        float_repr(rubric_floor),
                        float_repr(rubric_ratio),
                        rubric_primary.as_deref().map(repr).unwrap_or_else(|| "None".into())
                    ));
                }
                rs.add(
                    "BENCHMARK_MEASUREMENT_INVALID",
                    format!("benchmark.verdict={verdict_repr} — 판정은 발행을 막지 않지만 **유효 측정 ∧ 서빙 성립**이 서지 않는다: {}", unmet.join(" · ")),
                );
            } else {
                rs.add(
                    "BENCHMARK_VERDICT_ABSENT",
                    format!("benchmark.verdict={verdict_repr} — 판정이 없는 full 셀은 무엇을 쟀는지 말하지 않는다 (PASS·REFUTE·FAIL·OBSERVATION-ONLY 중 하나가 필요하다)"),
                );
            }
        } else if certificate_is_waived && !certificate_present && !rubric_ok {
            rs.add(
                "CERTIFICATE_WAIVED_RUBRIC_CONTRACT_UNMET",
                format!(
                    "benchmark.verdict='PASS' ∧ rubric_authority={} → 인증서 면제. 그러나 manifest rubric 계약이 서지 않는다(carrier={src_repr}, floor_tps={}, ratio={}) — 승격을 열지 않는다",
                    req_auth.map(repr).unwrap_or_else(|| "None".into()),
                    float_repr(rubric_floor),
                    float_repr(rubric_ratio)
                ),
            );
        } else {
            eligible = true;
            if rubric_authority.as_deref() == Some("explore") {
                rs.add(
                    "BENCHMARK_EXPLORE_AUTHORITY_PROMOTION",
                    format!(
                        "rubric_authority='explore'(carrier={src_repr}) -- 승격 근거는 성능 판정이 아니라 서빙 성립 + 유효 측정이다(floor_tps={}, ratio={}). verdict={verdict_repr} 는 벽 지도 데이터로 기록된다.",
                        float_repr(rubric_floor),
                        float_repr(rubric_ratio)
                    ),
                );
            }
        }
    }
    if eligible {
        state = "promotion-ready";
    }
    let out = D::new()
        .put("schema_version", PyValue::int(SCHEMA_VERSION))
        .put("task_class", task_class_v.clone())
        .put("state", s(state))
        .put("eligible_for_promotion", PyValue::Bool(eligible))
        .put("reason_codes", rs.sorted_codes())
        .put("messages", rs.messages())
        .put("checked_evidence", PyValue::Dict(checked))
        .put("identity", identity.clone())
        .put("rubric", rubric_output)
        .put("certificate", certificate_output.unwrap_or(PyValue::None))
        .v();
    drop(root_fd);
    emit(sch, out, if eligible { 0 } else { 1 })
}
