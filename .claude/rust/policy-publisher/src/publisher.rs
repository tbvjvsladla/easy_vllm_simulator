//! `evidence_publisher.py` subcommands. Every function keeps the original's order of checks, reason codes and message
//! texts; gate/naming rules are reached through [`crate::gate`] (the policy-gate crate), never re-implemented here.

use crate::bench_section;
use crate::classify;
use crate::fsx::{self, Fd};
use crate::gate;
use crate::json::*;
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 1;

pub const SCAFFOLDABLE_KINDS: [&str; 5] = ["plan", "devlog", "testlog", "simlog", "report"];
pub const DATED_KINDS: [&str; 3] = ["plan", "devlog", "testlog"];
pub const ALL_EVIDENCE_KINDS: [&str; 9] =
    ["plan", "devlog", "testlog", "simlog", "report", "verification", "commit", "bench_report", "certificate"];
pub const CAPACITY_REJECTION_OR_GROUP: [&str; 2] = ["devlog", "testlog"];
pub const NARRATIVE_BEGIN: &str = "<!-- NARRATIVE:BEGIN -->";
pub const NARRATIVE_END: &str = "<!-- NARRATIVE:END -->";
pub const PLACEHOLDER_MARK: &str = "_PLACEHOLDER --";
pub const PLACEHOLDER: &str = "_PLACEHOLDER -- agent-authored narrative pending. Do not treat as final evidence._";
pub const OPTIONAL_RECORD_FIELDS: [&str; 3] = ["reclassification", "promotion_target", "promotion_target_provenance"];
pub const DOWNGRADE_FROM_CLASSES: [&str; 1] = ["full_benchmark"];
pub const PROMOTION_TARGET_TAG_PREFIX: &str = "hint/";
pub const DOWNGRADE_TO_CLASS: &str = "hint_map_only";
pub const APPEND_RAW_KINDS: [&str; 7] = ["plan", "devlog", "testlog", "simlog", "report", "verification", "commit"];
pub const POINTER_ONLY_KINDS: [&str; 2] = ["verification", "commit"];
pub const NARRATIVE_KINDS: [&str; 4] = ["plan", "devlog", "testlog", "report"];
pub const NARRATIVE_STATUSES: [&str; 2] = ["authored", "bound"];
pub const BINDABLE_NARRATIVE_KINDS: [&str; 3] = ["plan", "devlog", "testlog"];
pub const AUTHOR_MAX_LEN: usize = 200;
pub const RUBRIC_RECORD_FIELDS: [&str; 5] =
    ["rubric_authority", "floor_tps", "ratio_M_over_primary", "primary_source", "rubric_source"];
pub const BENCHMARK_CARRY_FIELDS: [&str; 6] =
    ["rubric_authority", "floor_tps", "ratio_M_over_primary", "primary_source", "rubric_source", "measured_utc"];
pub const ALL_EVIDENCE_KEYS: [&str; 9] =
    ["plan", "devlog", "testlog", "simlog", "bench_report", "certificate", "verification", "commit", "report"];
pub const LITE_BENCH_MODE: &str = "lite";
const RECORD_REQUIRED_OBJECT_FIELDS: [&str; 6] =
    ["identity", "conditions", "benchmark", "scaffolded", "narrative_status", "raw_log_paths"];

/// The process outcome of one invocation: the JSON (or relayed) stdout text and the exit code.
pub struct Exit {
    pub code: i32,
    pub stdout: String,
}
pub type R<T> = Result<T, Exit>;

pub fn emit(obj_: &V, code: i32) -> Exit {
    Exit { code, stdout: format!("{}\n", dumps_pretty_sorted(obj_)) }
}
pub fn bare_error(code: &str, message: &str) -> V {
    obj(vec![
        ("schema_version", int(SCHEMA_VERSION)),
        ("ok", b(false)),
        ("reason_codes", list_s(&[code])),
        ("messages", obj(vec![(code, s(message))])),
    ])
}
pub fn fail<T>(code: &str, message: String) -> R<T> {
    Err(emit(&bare_error(code, &message), 2))
}
fn ok_head(topic: &str) -> Vec<(&'static str, V)> {
    vec![
        ("schema_version", int(SCHEMA_VERSION)),
        ("ok", b(true)),
        ("reason_codes", PyValue::List(vec![])),
        ("messages", obj(vec![])),
        ("publication_id", s(topic)),
    ]
}
fn success(topic: &str, rest: Vec<(&'static str, V)>) -> Exit {
    let mut items = ok_head(topic);
    items.extend(rest);
    emit(&obj(items), 0)
}

/// Where the program "lives": the original's `Path(__file__)` and `parents[3]`.
#[derive(Clone)]
pub struct Ctx {
    pub script: PathBuf,
    pub repo: PathBuf,
}

pub type Args = BTreeMap<&'static str, String>;
fn arg<'a>(a: &'a Args, flag: &str) -> Option<&'a str> {
    a.get(flag).map(|x| x.as_str())
}
fn req<'a>(a: &'a Args, flag: &str) -> &'a str {
    a.get(flag).map(|x| x.as_str()).unwrap_or("")
}

// ── small Python-shape helpers ─────────────────────────────────────────────────────────────────────────────────

fn is_utc_second(t: &str) -> bool {
    // re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", t) -- \d is Unicode Nd
    let c: Vec<char> = t.chars().collect();
    let shape = "dddd-dd-ddTdd:dd:ddZ";
    c.len() == shape.len()
        && shape.chars().zip(c.iter()).all(|(p, ch)| {
            if p == 'd' {
                agent_runtime_support::pystr::is_decimal_digit(*ch as u32)
            } else {
                p == *ch
            }
        })
}

/// `re.match(r"^<prefix>\d{8}(?:_\d{2}_\d{2})?_.+\.<ext>$", name)` (the optional group never changes the answer: the
/// `_.+` that follows accepts everything it would).
fn bench_name_matches(name: &str, prefix: &str, ext: &str) -> bool {
    let rest = match name.strip_prefix(prefix) {
        Some(r) => r,
        None => return false,
    };
    let chars: Vec<char> = rest.chars().collect();
    if chars.len() < 8 || !chars[..8].iter().all(|ch| agent_runtime_support::pystr::is_decimal_digit(*ch as u32)) {
        return false;
    }
    let tail: String = chars[8..].iter().collect();
    let tail = match tail.strip_prefix('_') {
        Some(t) => t.to_string(),
        None => return false,
    };
    let body = tail.strip_suffix('\n').unwrap_or(&tail);
    let suffix = format!(".{ext}");
    match body.strip_suffix(&suffix) {
        Some(y) => !y.is_empty() && !y.contains('\n') && !tail[..body.len()].contains('\n'),
        None => false,
    }
}
fn bench_pattern(kind: &str) -> &'static str {
    if kind == "bench_report" {
        r"^bench_report_\d{8}(?:_\d{2}_\d{2})?_.+\.md$"
    } else {
        r"^benchmark_\d{8}(?:_\d{2}_\d{2})?_.+\.yaml$"
    }
}
pub fn bench_src_matches(kind: &str, name: &str) -> bool {
    if kind == "bench_report" {
        bench_name_matches(name, "bench_report_", "md")
    } else {
        bench_name_matches(name, "benchmark_", "yaml")
    }
}

/// `Path(p).name`.
fn path_name(p: &str) -> String {
    p.split('/').filter(|c| !c.is_empty() && *c != ".").last().unwrap_or("").to_string()
}

/// `os.path.normpath` of an absolute POSIX path.
fn normpath(p: &str) -> String {
    let initial = if p.starts_with("//") && !p.starts_with("///") { "//" } else if p.starts_with('/') { "/" } else { "" };
    let mut out: Vec<&str> = Vec::new();
    for comp in p.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
            if !out.is_empty() && *out.last().unwrap() != ".." {
                out.pop();
            } else if initial.is_empty() {
                out.push("..");
            }
            continue;
        }
        out.push(comp);
    }
    let joined = out.join("/");
    let r = format!("{initial}{joined}");
    if r.is_empty() {
        ".".to_string()
    } else {
        r
    }
}
fn abspath(p: &str) -> String {
    if p.starts_with('/') {
        normpath(p)
    } else {
        let cwd = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        normpath(&format!("{cwd}/{p}"))
    }
}
/// `os.path.relpath(path, start)`.
pub fn relpath(path: &str, start: &str) -> String {
    let a = abspath(path);
    let s_ = abspath(start);
    let ap: Vec<&str> = a.split('/').filter(|x| !x.is_empty()).collect();
    let sp: Vec<&str> = s_.split('/').filter(|x| !x.is_empty()).collect();
    let mut i = 0;
    while i < ap.len() && i < sp.len() && ap[i] == sp[i] {
        i += 1;
    }
    let mut parts: Vec<&str> = vec![".."; sp.len() - i];
    parts.extend(&ap[i..]);
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}
fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// `Path(explicit).resolve()` (non-strict).
fn resolve_path(explicit: &str) -> PathBuf {
    let abs = PathBuf::from(abspath_raw(explicit));
    if let Ok(c) = std::fs::canonicalize(&abs) {
        return c;
    }
    // resolve the longest existing prefix, keep the rest lexically
    let mut existing = abs.clone();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        match (existing.file_name().map(|n| n.to_os_string()), existing.parent().map(|p| p.to_path_buf())) {
            (Some(n), Some(p)) => {
                tail.push(n);
                existing = p;
            }
            _ => break,
        }
    }
    let mut out = std::fs::canonicalize(&existing).unwrap_or(existing);
    for n in tail.into_iter().rev() {
        if n == ".." {
            out.pop();
        } else if n != "." {
            out.push(n);
        }
    }
    out
}
fn abspath_raw(p: &str) -> String {
    if p.starts_with('/') {
        p.to_string()
    } else {
        let cwd = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        format!("{cwd}/{p}")
    }
}

// ── JSON inputs ───────────────────────────────────────────────────────────────────────────────────────────────

/// `_load_json(path_str, label)`.
fn load_json(path: &str, label: &str) -> R<V> {
    let up = label.to_uppercase();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            let code = e.raw_os_error().unwrap_or(0);
            if code == libc::ENOENT {
                return fail(&format!("{up}_FILE_NOT_FOUND"), format!("{label} file not found: {path}"));
            }
            return fail(&format!("{up}_FILE_UNREADABLE"), format!("{label} file could not be read: {}", fsx::os_error(code, Some(path))));
        }
    };
    let text = match pyjson::decode_file_text(&bytes) {
        Ok(t) => t,
        Err(e) => return fail(&format!("{up}_INVALID_UTF8"), format!("{label} file is not valid UTF-8: {e}")),
    };
    match pyjson::loads_text(&text) {
        Ok(v) => Ok(v),
        Err(e) => fail(&format!("{up}_INVALID_JSON"), format!("{label} file is not valid JSON: {e}")),
    }
}

// ── hardened descriptor-relative I/O ────────────────────────────────────────────────────────────────────────────

fn open_repo_root_fd(repo_root: &Path) -> R<Fd> {
    let p = path_str(repo_root);
    fsx::open_path(&p, libc::O_RDONLY | libc::O_DIRECTORY).or_else(|e| {
        fail("REPO_ROOT_UNREADABLE", format!("could not open repo root {p}: {}", fsx::os_error(e, Some(&p))))
    })
}

/// `_safe_mkdir_chain`.
pub fn safe_mkdir_chain(root_fd: &Fd, parts: &[String], prefix: &str) -> R<Fd> {
    let mut cur = root_fd.dup().or_else(|e| fail(&format!("{prefix}_DIR_OPEN_FAILED"), fsx::os_error(e, None)))?;
    for part in parts {
        if let Err(e) = fsx::mkdirat(cur.raw(), part) {
            if e != libc::EEXIST {
                return fail(
                    &format!("{prefix}_DIR_CREATE_FAILED"),
                    format!("could not create directory component {}: {}", repr(part), fsx::os_error(e, Some(part))),
                );
            }
        }
        match fsx::openat(cur.raw(), part, libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW, 0) {
            Ok(next) => cur = next,
            Err(e) => {
                if e == libc::ELOOP {
                    return fail(
                        &format!("{prefix}_SYMLINK"),
                        format!("path component {} is a symlink -- refusing to write through it", repr(part)),
                    );
                }
                return fail(
                    &format!("{prefix}_DIR_OPEN_FAILED"),
                    format!("path component {} is unsafe or unreadable: {}", repr(part), fsx::os_error(e, Some(part))),
                );
            }
        }
    }
    Ok(cur)
}

/// `_safe_stat_in_dir`.
pub fn safe_stat_in_dir(dir: &Fd, name: &str) -> &'static str {
    match fsx::lstatat(dir.raw(), name) {
        Err(e) if e == libc::ENOENT => "missing",
        Err(_) => "other",
        Ok(st) if fsx::is_lnk(&st) => "symlink",
        Ok(st) if fsx::is_reg(&st) => "file",
        Ok(st) if fsx::is_dir(&st) => "dir",
        Ok(_) => "other",
    }
}

/// `_safe_listdir_names`.
pub fn safe_listdir_names(dir_fd: libc::c_int, prefix: &str) -> R<Vec<String>> {
    fsx::listdir_fd(dir_fd).or_else(|e| {
        fail(
            &format!("{prefix}_LISTDIR_FAILED"),
            format!(
                "could not list the destination directory (collision detection would be blind — refusing to write): [Errno {e}] {}: {dir_fd}",
                fsx::strerror(e)
            ),
        )
    })
}

/// `_safe_replace_file`.
fn safe_replace_file(dir: &Fd, name: &str, content: &[u8], prefix: &str) -> R<()> {
    if name.contains('/') || name == "." || name == ".." {
        return fail(&format!("{prefix}_BASENAME_INVALID"), format!("unsafe basename: {}", repr(name)));
    }
    let kind = safe_stat_in_dir(dir, name);
    if kind == "symlink" {
        return fail(&format!("{prefix}_TARGET_IS_SYMLINK"), format!("{} is a symlink -- refusing to write through it", repr(name)));
    }
    if kind == "dir" {
        return fail(&format!("{prefix}_TARGET_WRONG_TYPE"), format!("{} is a directory, expected a file", repr(name)));
    }
    let tmp = format!("{name}.tmp-{}", std::process::id());
    let fd = match fsx::openat(dir.raw(), &tmp, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_NOFOLLOW, 0o644) {
        Ok(f) => f,
        Err(e) => return fail(&format!("{prefix}_TMP_CREATE_FAILED"), format!("could not create temp file: {}", fsx::os_error(e, Some(&tmp)))),
    };
    let _ = fsx::write_all(fd.raw(), content);
    let _ = fsx::fsync(fd.raw());
    drop(fd);
    if let Err(e) = fsx::renameat(dir.raw(), &tmp, name) {
        eprintln!("OSError: {}", fsx::os_error(e, Some(&tmp)));
        return Err(Exit { code: 1, stdout: String::new() });
    }
    Ok(())
}

fn open_evidence_dir_fd(repo_root: &Path, prefix: &str) -> R<Fd> {
    let root = open_repo_root_fd(repo_root)?;
    safe_mkdir_chain(&root, &["docs".to_string(), "_evidence".to_string()], prefix)
}

fn write_evidence_dir_file(repo_root: &Path, name: &str, content: &[u8], prefix: &str) -> R<()> {
    let dir = open_evidence_dir_fd(repo_root, prefix)?;
    safe_replace_file(&dir, name, content, prefix)
}

/// `_validate_prior_path`.
fn validate_prior_path(repo_root: &Path, prior: &str, expected: &[&str], prefix: &str) -> R<Vec<String>> {
    if gate::is_absolute_path_string(prior) {
        return fail(&format!("{prefix}_RECORD_PATH_ABSOLUTE"), format!("publication record's path {} is absolute -- refusing to trust it", repr(prior)));
    }
    let parts = match gate::lexical_components(repo_root, repo_root, prior) {
        Some(p) => p,
        None => {
            return fail(
                &format!("{prefix}_RECORD_PATH_ESCAPES_REPO"),
                format!("publication record's path {} escapes the repo root -- refusing to trust it", repr(prior)),
            )
        }
    };
    let n = expected.len().min(parts.len());
    if parts[..n].iter().map(|x| x.as_str()).collect::<Vec<_>>() != expected {
        return fail(
            &format!("{prefix}_RECORD_PATH_UNEXPECTED_LOCATION"),
            format!(
                "publication record's path {} is not under {}/ -- refusing to trust a tampered record",
                repr(prior),
                expected.join("/")
            ),
        );
    }
    if parts.len() <= expected.len() {
        return fail(
            &format!("{prefix}_RECORD_PATH_UNEXPECTED_LOCATION"),
            format!("publication record's path {} has no basename under {}/", repr(prior), expected.join("/")),
        );
    }
    Ok(parts)
}

fn save_record(repo_root: &Path, topic: &str, record: &V) -> R<PathBuf> {
    let content = dumps_pretty_sorted(record);
    write_evidence_dir_file(repo_root, &format!("{topic}.json"), content.as_bytes(), "PUBLICATION_RECORD")?;
    Ok(repo_root.join("docs").join("_evidence").join(format!("{topic}.json")))
}

fn repo_relative(repo_root: &Path, path: &Path) -> String {
    path_str(path.strip_prefix(repo_root).unwrap_or(path))
}

pub fn resolve_repo_root(ctx: &Ctx, explicit: Option<&str>) -> R<PathBuf> {
    if let Some(e) = explicit {
        if !e.is_empty() {
            return Ok(resolve_path(e));
        }
    }
    match gate::find_repo_root(&ctx.script) {
        Some(p) => Ok(p),
        None => fail(
            "REPO_ROOT_NOT_FOUND",
            "cannot locate repo root (.git not found) from script location; pass --repo-root explicitly".to_string(),
        ),
    }
}

// ── record shape ──────────────────────────────────────────────────────────────────────────────────────────────

fn str_list(value: &V) -> Option<Vec<String>> {
    match value.peel() {
        PyValue::List(items) => items.iter().map(text).collect(),
        _ => None,
    }
}
fn v_list(items: &[String]) -> V {
    list_s(items)
}
fn sorted_set(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = items.into_iter().collect();
    v.sort_by(|a, c| cmp_code_points(a, c));
    v.dedup();
    v
}
fn keys_of(d: &PyDict) -> Vec<String> {
    d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect()
}

fn reclassification_shape_error(value: &V, task_class: &str) -> Option<String> {
    let d = match dict(value) {
        Some(d) => d,
        None => return Some(reclass_keys_msg()),
    };
    let keys = sorted_set(keys_of(d));
    if keys != vec!["from", "reason", "reason_source", "to"] {
        return Some(reclass_keys_msg());
    }
    let from = d.get("from").cloned().unwrap_or(PyValue::None);
    let to = d.get("to").cloned().unwrap_or(PyValue::None);
    let from_ok = text(&from).map(|f| DOWNGRADE_FROM_CLASSES.contains(&f.as_str())).unwrap_or(false);
    if !from_ok || text(&to).as_deref() != Some(DOWNGRADE_TO_CLASS) || task_class != DOWNGRADE_TO_CLASS {
        return Some(format!(
            "reclassification must record ('full_benchmark',) -> {} on a {} record (got {} -> {} on {})",
            repr(DOWNGRADE_TO_CLASS),
            repr(DOWNGRADE_TO_CLASS),
            from.repr(),
            to.repr(),
            repr(task_class)
        ));
    }
    for k in ["reason", "reason_source"] {
        match d.get(k).and_then(text) {
            Some(t) if !t.is_empty() => {}
            _ => return Some("reclassification reason/reason_source must be non-empty strings".to_string()),
        }
    }
    None
}
fn reclass_keys_msg() -> String {
    "reclassification must be an object with keys ['from', 'reason', 'reason_source', 'to']".to_string()
}

fn anchor_ok(a: &str) -> bool {
    a.len() == 40 && a.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// `_promotion_target_error`.
fn promotion_target_error(value: &V) -> Option<String> {
    let errors = gate::validate_definition(value, "promotionTarget", "promotion_target");
    if let Some((p, m)) = errors.first() {
        return Some(format!("{p}: {m}"));
    }
    let anchor = get_str(value, "anchor").unwrap_or_default();
    if !anchor_ok(&anchor) {
        return Some(format!(
            "promotion_target.anchor must be the full 40-hex (lowercase) payload commit SHA, got {}",
            repr(&anchor)
        ));
    }
    let tag = get_str(value, "tag").unwrap_or_default();
    if !tag.starts_with(PROMOTION_TARGET_TAG_PREFIX) || tag.chars().any(|c| " \t\r\n*?[\\".contains(c)) {
        return Some(format!(
            "promotion_target.tag must be one exact tag name under {} (no whitespace/glob), got {}",
            repr(PROMOTION_TARGET_TAG_PREFIX),
            repr(&tag)
        ));
    }
    let topo = get_str(value, "topology").unwrap_or_default();
    if topo.contains('\r') || topo.contains('\n') || topo != py_strip(&topo) {
        return Some(format!("promotion_target.topology must be a single trimmed line, got {}", repr(&topo)));
    }
    None
}

pub fn py_strip(t: &str) -> &str {
    t.trim_matches(|c: char| agent_runtime_support::pystr::is_python_space(c as u32))
}

fn required_for(task_class: &str, conditions: &V, verdict: Option<String>, benchmark: &V) -> Vec<String> {
    gate::required_evidence_for(task_class, conditions, verdict.as_deref(), gate::certificate_requirement_authority(benchmark).as_deref())
}

/// `_validate_record_shape`: None or (code, message).
pub fn validate_record_shape(record: &V) -> Option<(String, String)> {
    let d = match dict(record) {
        Some(d) => d,
        None => {
            return Some((
                "PUBLICATION_RECORD_NOT_AN_OBJECT".into(),
                format!("publication record must be a JSON object, got {}", type_name(record)),
            ))
        }
    };
    let allowed: [&str; 14] = [
        "schema_version", "publication_id", "task_class", "generated_utc", "identity", "conditions",
        "benchmark", "required_evidence", "or_group_scaffolded", "capacity_rejection_required",
        "scaffolded", "narrative_status", "raw_log_paths", "capacity_rejection",
    ];
    let keys = keys_of(d);
    let unknown: Vec<String> = keys
        .iter()
        .filter(|k| !allowed.contains(&k.as_str()) && !OPTIONAL_RECORD_FIELDS.contains(&k.as_str()))
        .cloned()
        .collect();
    let missing: Vec<String> = allowed.iter().filter(|k| !keys.iter().any(|x| x == *k)).map(|k| k.to_string()).collect();
    if !unknown.is_empty() {
        return Some(("PUBLICATION_RECORD_UNKNOWN_FIELDS".into(), format!("publication record contains unknown fields {}", repr_sorted(&unknown))));
    }
    if !missing.is_empty() {
        return Some((
            "PUBLICATION_RECORD_MISSING_FIELDS".into(),
            format!("publication record is missing producer fields {}", repr_sorted(&missing)),
        ));
    }
    let g = |k: &str| d.get(k).cloned().unwrap_or(PyValue::None);
    match text(&g("publication_id")) {
        Some(t) if !t.is_empty() => {}
        _ => return Some(("PUBLICATION_RECORD_PUBLICATION_ID_INVALID".into(), "publication_id must be a non-empty string".into())),
    }
    match text(&g("generated_utc")) {
        Some(t) if is_utc_second(&t) => {}
        _ => return Some(("PUBLICATION_RECORD_GENERATED_UTC_INVALID".into(), "generated_utc must be a UTC second timestamp".into())),
    }
    for field in ["schema_version", "task_class", "identity", "conditions", "benchmark", "capacity_rejection"] {
        let v = g(field);
        if field == "capacity_rejection" && is_none(&v) {
            continue;
        }
        let errors = gate::validate_property(&v, field, field);
        if let Some((p, _)) = errors.first() {
            return Some(("PUBLICATION_RECORD_SCHEMA_INVALID".into(), p.clone()));
        }
    }
    for field in ["required_evidence", "or_group_scaffolded"] {
        let ok = match g(field).peel() {
            PyValue::List(items) => items.iter().all(|x| text(x).map(|t| ALL_EVIDENCE_KINDS.contains(&t.as_str())).unwrap_or(false)),
            _ => false,
        };
        if !ok {
            return Some((format!("PUBLICATION_RECORD_FIELD_INVALID:{field}"), format!("{field} must be a list of canonical evidence-kind strings")));
        }
    }
    if !matches!(g("capacity_rejection_required").peel(), PyValue::Bool(_)) {
        return Some(("PUBLICATION_RECORD_CAPACITY_FLAG_INVALID".into(), "capacity_rejection_required must be boolean".into()));
    }
    let task_class = text(&g("task_class")).unwrap_or_default();
    let benchmark = g("benchmark");
    let expected_required = required_for(&task_class, &g("conditions"), get_str(&benchmark, "verdict"), &benchmark);
    let expected_or: Vec<String> =
        if task_class == "capacity_rejection" { CAPACITY_REJECTION_OR_GROUP.iter().map(|x| x.to_string()).collect() } else { vec![] };
    let expected_cap = task_class == "capacity_rejection";
    if str_list(&g("required_evidence")) != Some(expected_required) {
        return Some((
            "PUBLICATION_RECORD_REQUIRED_EVIDENCE_MISMATCH".into(),
            "required_evidence does not match task_class/conditions/benchmark verdict".into(),
        ));
    }
    if str_list(&g("or_group_scaffolded")) != Some(expected_or) {
        return Some(("PUBLICATION_RECORD_OR_GROUP_MISMATCH".into(), "or_group_scaffolded does not match the task-class producer contract".into()));
    }
    if !matches!(g("capacity_rejection_required").peel(), PyValue::Bool(x) if *x == expected_cap) {
        return Some(("PUBLICATION_RECORD_CAPACITY_FLAG_MISMATCH".into(), "capacity_rejection_required does not match task_class".into()));
    }
    if text(&g("task_class")).is_none() {
        return Some((
            "PUBLICATION_RECORD_FIELD_WRONG_TYPE:task_class".into(),
            format!("publication record field 'task_class' must be a string, got {}", type_name(&g("task_class"))),
        ));
    }
    for field in RECORD_REQUIRED_OBJECT_FIELDS {
        if !is_dict(&g(field)) {
            return Some((
                format!("PUBLICATION_RECORD_FIELD_NOT_AN_OBJECT:{field}"),
                format!("publication record field {} must be a JSON object, got {}", repr(field), type_name(&g(field))),
            ));
        }
    }
    if d.contains_key("reclassification") {
        if let Some(e) = reclassification_shape_error(&g("reclassification"), &task_class) {
            return Some(("PUBLICATION_RECORD_RECLASSIFICATION_INVALID".into(), e));
        }
    }
    if d.contains_key("promotion_target") != d.contains_key("promotion_target_provenance") {
        return Some((
            "PUBLICATION_RECORD_PROMOTION_TARGET_INVALID".into(),
            "promotion_target and promotion_target_provenance must be present together (written only by `set-promotion-target`)".into(),
        ));
    }
    if d.contains_key("promotion_target") {
        if let Some(e) = promotion_target_error(&g("promotion_target")) {
            return Some(("PUBLICATION_RECORD_PROMOTION_TARGET_INVALID".into(), e));
        }
        let prov = g("promotion_target_provenance");
        let bad = match dict(&prov) {
            None => true,
            Some(pd) => {
                sorted_set(keys_of(pd)) != vec!["generated_utc", "rebound_from", "writer"]
                    || get_str(&prov, "writer").as_deref() != Some("set-promotion-target")
                    || !get_str(&prov, "generated_utc").map(|t| is_utc_second(&t)).unwrap_or(false)
                    || !matches!(pd.get("rebound_from").map(|x| x.peel()), Some(PyValue::None) | Some(PyValue::Dict(_)) | None)
            }
        };
        if bad {
            return Some((
                "PUBLICATION_RECORD_PROMOTION_TARGET_INVALID".into(),
                "promotion_target_provenance must be exactly ['generated_utc', 'rebound_from', 'writer'] with writer='set-promotion-target' and a UTC second timestamp".into(),
            ));
        }
    }
    let cr = g("capacity_rejection");
    if !is_none(&cr) && !is_dict(&cr) {
        return Some((
            "PUBLICATION_RECORD_FIELD_NOT_AN_OBJECT:capacity_rejection".into(),
            format!("publication record field 'capacity_rejection' must be a JSON object (or null), got {}", type_name(&cr)),
        ));
    }
    if let Some(sc) = dict(&g("scaffolded")) {
        let actual = keys_of(sc);
        let unknown: Vec<String> = actual.iter().filter(|k| !ALL_EVIDENCE_KINDS.contains(&k.as_str())).cloned().collect();
        let missing: Vec<String> = ALL_EVIDENCE_KINDS.iter().filter(|k| !actual.iter().any(|x| x == *k)).map(|x| x.to_string()).collect();
        if !unknown.is_empty() || !missing.is_empty() {
            return Some((
                "PUBLICATION_RECORD_SCAFFOLDED_KEYS_INVALID".into(),
                format!(
                    "scaffolded must contain the complete producer key set; unknown={} missing={}",
                    repr_sorted(&unknown),
                    repr_sorted(&missing)
                ),
            ));
        }
        for (k, v) in sc.iter() {
            let kind = k.to_utf8().unwrap_or_default();
            let ok = is_none(v) || matches!(text(v), Some(t) if !t.is_empty());
            if !ok {
                return Some((
                    format!("PUBLICATION_RECORD_SCAFFOLDED_VALUE_NOT_A_STRING:{kind}"),
                    format!(
                        "publication record field 'scaffolded.{kind}' must be a non-empty string path or null, got {}",
                        type_name(v)
                    ),
                ));
            }
        }
    }
    if let Some(ns) = dict(&g("narrative_status")) {
        let unknown: Vec<String> = keys_of(ns).into_iter().filter(|k| !SCAFFOLDABLE_KINDS.contains(&k.as_str())).collect();
        if !unknown.is_empty() {
            return Some((
                "PUBLICATION_RECORD_NARRATIVE_STATUS_UNKNOWN_KEYS".into(),
                format!("narrative_status contains unknown evidence kinds {}", repr_sorted(&unknown)),
            ));
        }
        for (k, v) in ns.iter() {
            let kind = k.to_utf8().unwrap_or_default();
            let vd = match dict(v) {
                Some(x) => x,
                None => {
                    return Some((
                        format!("PUBLICATION_RECORD_NARRATIVE_STATUS_VALUE_NOT_AN_OBJECT:{kind}"),
                        format!("publication record field 'narrative_status.{kind}' must be a JSON object, got {}", type_name(v)),
                    ))
                }
            };
            let status_ok = get_str(v, "status").map(|t| NARRATIVE_STATUSES.contains(&t.as_str())).unwrap_or(false);
            if sorted_set(keys_of(vd)) != vec!["author", "generated_utc", "source", "status"] || !status_ok {
                return Some((
                    format!("PUBLICATION_RECORD_NARRATIVE_STATUS_SHAPE_INVALID:{kind}"),
                    format!("narrative_status.{kind} must be the complete producer-emitted entry with status in ['authored', 'bound']"),
                ));
            }
            for key in ["author", "generated_utc", "source"] {
                if !matches!(get_str(v, key), Some(t) if !t.is_empty()) {
                    return Some((
                        format!("PUBLICATION_RECORD_NARRATIVE_STATUS_FIELD_INVALID:{kind}"),
                        format!("narrative_status.{kind} provenance fields must be non-empty strings"),
                    ));
                }
            }
            if !is_utc_second(&get_str(v, "generated_utc").unwrap_or_default()) {
                return Some((
                    format!("PUBLICATION_RECORD_NARRATIVE_STATUS_TIME_INVALID:{kind}"),
                    format!("narrative_status.{kind}.generated_utc must be a UTC second timestamp"),
                ));
            }
        }
    }
    if let Some(rl) = dict(&g("raw_log_paths")) {
        let unknown: Vec<String> = keys_of(rl).into_iter().filter(|k| !APPEND_RAW_KINDS.contains(&k.as_str())).collect();
        if !unknown.is_empty() {
            return Some((
                "PUBLICATION_RECORD_RAW_LOG_PATHS_UNKNOWN_KEYS".into(),
                format!("raw_log_paths contains unknown evidence kinds {}", repr_sorted(&unknown)),
            ));
        }
        let pid = text(&g("publication_id")).unwrap_or_default();
        for (k, v) in rl.iter() {
            let key = k.to_utf8().unwrap_or_default();
            let value = match text(v) {
                Some(t) if !t.is_empty() => t,
                _ => return Some((format!("PUBLICATION_RECORD_RAW_LOG_PATH_INVALID:{key}"), format!("raw_log_paths.{key} must be a nonempty string"))),
            };
            let expected: Option<String> = if key == "simlog" {
                get_str(&g("scaffolded"), "simlog")
            } else {
                Some(format!("docs/_evidence/{pid}.{key}.raw.jsonl"))
            };
            let good = matches!(&expected, Some(e) if !e.is_empty() && *e == value);
            if !good {
                let shown = match dict(&g("scaffolded")).and_then(|x| x.get("simlog")) {
                    _ if key != "simlog" => repr_opt(expected.as_deref()),
                    Some(v) => v.repr(),
                    None => "None".into(),
                };
                return Some((
                    format!("PUBLICATION_RECORD_RAW_LOG_PATH_MISMATCH:{key}"),
                    format!("raw_log_paths.{key} must equal the producer-derived path ({shown})"),
                ));
            }
        }
    }
    None
}

fn kind_name(k: &str) -> &str {
    k
}

/// `_load_record`.
pub fn load_record(repo_root: &Path, topic: &str) -> R<Option<V>> {
    let basename = format!("{topic}.json");
    let dir = open_evidence_dir_fd(repo_root, "PUBLICATION_RECORD")?;
    let kind = safe_stat_in_dir(&dir, &basename);
    if kind == "missing" {
        return Ok(None);
    }
    if kind == "symlink" {
        return fail(
            "PUBLICATION_RECORD_TARGET_IS_SYMLINK",
            format!("publication record for topic {} is a symlink -- refusing to read through it", repr(topic)),
        );
    }
    if kind != "file" {
        return fail(
            "PUBLICATION_RECORD_TARGET_WRONG_TYPE",
            format!("publication record for topic {} is not a regular file (kind={})", repr(topic), repr(kind_name(kind))),
        );
    }
    let fd = match fsx::openat(dir.raw(), &basename, libc::O_RDONLY | libc::O_NOFOLLOW, 0) {
        Ok(f) => f,
        Err(e) => {
            if e == libc::ELOOP {
                return fail(
                    "PUBLICATION_RECORD_TARGET_IS_SYMLINK",
                    format!("publication record for topic {} is a symlink -- refusing to read through it", repr(topic)),
                );
            }
            return fail(
                "PUBLICATION_RECORD_IO_ERROR",
                format!("could not open publication record for topic {}: {}", repr(topic), fsx::os_error(e, Some(&basename))),
            );
        }
    };
    let bytes = fsx::read_to_end(fd.raw()).unwrap_or_default();
    drop(fd);
    drop(dir);
    let text_ = match pyjson::decode_file_text(&bytes) {
        Ok(t) => t,
        Err(e) => {
            return fail(
                "PUBLICATION_RECORD_INVALID_UTF8",
                format!("publication record for topic {} is not valid UTF-8: {e}", repr(topic)),
            )
        }
    };
    let record = match pyjson::loads_text(&text_) {
        Ok(v) => v,
        Err(e) => {
            return fail(
                "PUBLICATION_RECORD_INVALID_JSON",
                format!("publication record for topic {} is not valid JSON: {e}", repr(topic)),
            )
        }
    };
    if let Some((code, message)) = validate_record_shape(&record) {
        return fail(&code, format!("publication record for topic {}: {message}", repr(topic)));
    }
    let pid = get_str(&record, "publication_id").unwrap_or_default();
    if pid != topic {
        return fail(
            "PUBLICATION_RECORD_TOPIC_MISMATCH",
            format!("publication record selected as topic {} declares publication_id {}", repr(topic), repr(&pid)),
        );
    }
    Ok(Some(record))
}

fn require_record(repo_root: &Path, topic: &str) -> R<V> {
    match load_record(repo_root, topic)? {
        Some(r) => Ok(r),
        None => fail("PUBLICATION_NOT_INITIALIZED", format!("no publication record for topic {} -- run `init` first", repr(topic))),
    }
}

fn validate_topic_or_die(topic: &str) -> R<()> {
    gate::validate_topic(topic).or_else(|m| fail("INVALID_TOPIC", m))
}

fn validate_required_utc(value: &str, arg_name: &str) -> R<()> {
    let (yymmddhh, _, _) = gate::kst_tokens(Some(value));
    if yymmddhh == "NA" {
        let code = format!("INVALID_{}", arg_name.to_uppercase().replace('-', "_"));
        return fail(&code, format!("--{arg_name} {} is not a valid UTC 'YYYY-MM-DDTHH:MM:SSZ' timestamp", repr(value)));
    }
    Ok(())
}

// ── scaffolds ─────────────────────────────────────────────────────────────────────────────────────────────────

fn scaffold_markdown(doc_kind: &str, topic: &str, task_class: &str, generated_utc: &str) -> String {
    format!(
        "# {doc_kind} -- {topic}\n\n<!-- evidence_publisher scaffold -- provenance -->\n- publication_id: {topic}\n- task_class: {task_class}\n- generated_utc: {generated_utc}\n- scaffold_created_by: evidence_publisher.sh init\n\n## Narrative\n\n{NARRATIVE_BEGIN}\n{PLACEHOLDER}\n{NARRATIVE_END}\n"
    )
}

fn strs(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|x| x.to_string()).collect()
}

/// Restores/creates a file scaffold at a prior recorded path (shared by the dated and report scaffolds).
fn ensure_prior_file(
    root_fd: &Fd,
    repo_root: &Path,
    prior: &str,
    expected: &[&str],
    prefix: &str,
    content: &str,
) -> R<(String, bool)> {
    let parts = validate_prior_path(repo_root, prior, expected, prefix)?;
    let dir = safe_mkdir_chain(root_fd, &parts[..parts.len() - 1], prefix)?;
    let base = &parts[parts.len() - 1];
    match safe_stat_in_dir(&dir, base) {
        "file" => return Ok((prior.to_string(), false)),
        "symlink" => return fail(&format!("{prefix}_TARGET_IS_SYMLINK"), format!("{} is a symlink -- refusing to write through it", repr(prior))),
        "dir" => return fail(&format!("{prefix}_TARGET_WRONG_TYPE"), format!("{} is a directory, expected a file", repr(prior))),
        _ => {}
    }
    safe_replace_file(&dir, base, content.as_bytes(), prefix)?;
    Ok((prior.to_string(), true))
}

fn ensure_dated_scaffold(root_fd: &Fd, repo_root: &Path, kind: &str, topic: &str, task_class: &str, utc: &str, prior: Option<&str>) -> R<(String, bool)> {
    let prefix = format!("SCAFFOLD_{}", kind.to_uppercase());
    let content = scaffold_markdown(kind, topic, task_class, utc);
    if let Some(p) = prior.filter(|p| !p.is_empty()) {
        return ensure_prior_file(root_fd, repo_root, p, &["docs", kind], &prefix, &content);
    }
    let dir = safe_mkdir_chain(root_fd, &strs(&["docs", kind]), &prefix)?;
    let existing = safe_listdir_names(dir.raw(), "EVIDENCE_DIR")?;
    let basename = match gate::dated_doc_basename(kind, utc, topic, &existing) {
        Ok(b) => b,
        Err(gate::NamingError::Collision(m)) => return fail(&format!("{prefix}_COLLISION_EXHAUSTED"), m),
        Err(gate::NamingError::Value(m)) => {
            eprintln!("ValueError: {m}");
            return Err(Exit { code: 1, stdout: String::new() });
        }
    };
    safe_replace_file(&dir, &basename, content.as_bytes(), &prefix)?;
    Ok((format!("docs/{kind}/{basename}"), true))
}

fn ensure_simlog_scaffold(root_fd: &Fd, repo_root: &Path, topic: &str, utc: &str, prior: Option<&str>) -> R<(String, bool)> {
    let prefix = "SCAFFOLD_SIMLOG";
    if let Some(p) = prior.filter(|p| !p.is_empty()) {
        let parts = validate_prior_path(repo_root, p, &["docs", "simlog"], prefix)?;
        let dir = safe_mkdir_chain(root_fd, &parts[..parts.len() - 1], prefix)?;
        let base = &parts[parts.len() - 1];
        match safe_stat_in_dir(&dir, base) {
            "dir" => return Ok((p.to_string(), false)),
            "symlink" => {
                return fail(
                    &format!("{prefix}_TARGET_IS_SYMLINK"),
                    format!("{} is a symlink -- refusing to use it as the simlog run directory", repr(p)),
                )
            }
            "file" => return fail(&format!("{prefix}_TARGET_WRONG_TYPE"), format!("{} is a file, expected a directory", repr(p))),
            _ => {}
        }
        if let Err(e) = fsx::mkdirat(dir.raw(), base) {
            if e != libc::EEXIST {
                return fail(&format!("{prefix}_DIR_CREATE_FAILED"), format!("could not restore simlog run directory: {}", fsx::os_error(e, Some(base))));
            }
        }
        return Ok((p.to_string(), true));
    }
    let dir = safe_mkdir_chain(root_fd, &strs(&["docs", "simlog"]), prefix)?;
    let existing = safe_listdir_names(dir.raw(), "EVIDENCE_DIR")?;
    let dirname = match gate::simlog_dirname(utc, topic, &existing) {
        Ok(b) => b,
        Err(gate::NamingError::Collision(m)) => return fail(&format!("{prefix}_COLLISION_EXHAUSTED"), m),
        Err(gate::NamingError::Value(m)) => {
            eprintln!("ValueError: {m}");
            return Err(Exit { code: 1, stdout: String::new() });
        }
    };
    if let Err(e) = fsx::mkdirat(dir.raw(), &dirname) {
        if e != libc::EEXIST {
            return fail(&format!("{prefix}_DIR_CREATE_FAILED"), format!("could not create simlog run directory: {}", fsx::os_error(e, Some(&dirname))));
        }
    }
    Ok((format!("docs/simlog/{dirname}"), true))
}

fn ensure_report_scaffold(root_fd: &Fd, repo_root: &Path, topic: &str, task_class: &str, utc: &str, slug: Option<&str>, prior: Option<&str>) -> R<(String, bool)> {
    let prefix = "SCAFFOLD_REPORT";
    let content = scaffold_markdown("report", topic, task_class, utc);
    if let Some(p) = prior.filter(|p| !p.is_empty()) {
        return ensure_prior_file(root_fd, repo_root, p, &["docs", "report"], prefix, &content);
    }
    let basename = match gate::report_basename(slug.unwrap_or("")) {
        Ok(b) => b,
        Err(m) => return fail("INIT_REPORT_SLUG_INVALID", m),
    };
    let dir = safe_mkdir_chain(root_fd, &strs(&["docs", "report"]), prefix)?;
    safe_replace_file(&dir, &basename, content.as_bytes(), prefix)?;
    Ok((format!("docs/report/{basename}"), true))
}

// ── raw-evidence source resolution ───────────────────────────────────────────────────────────────────────────

/// `_resolve_src` -> (bytes, sha256).
fn resolve_src(repo_root: &Path, src: &str, prefix: &str) -> R<(Vec<u8>, String)> {
    if gate::is_absolute_path_string(src) {
        return fail(&format!("{prefix}_ABSOLUTE_SRC"), format!("src path must be repo-relative, not absolute: {}", repr(src)));
    }
    let p = path_str(repo_root);
    let root = match fsx::open_path(&p, libc::O_RDONLY | libc::O_DIRECTORY) {
        Ok(f) => f,
        Err(e) => return fail(&format!("{prefix}_REPO_ROOT_UNREADABLE"), format!("could not open repo root {p}: {}", fsx::os_error(e, Some(&p)))),
    };
    let r = gate::resolve_and_stat_evidence(&root, repo_root, repo_root, src, false, true);
    drop(root);
    match r.status.as_str() {
        "repo_escape" => return fail(&format!("{prefix}_SRC_ESCAPES_REPO"), format!("src path escapes the repo root via '..' traversal: {}", repr(src))),
        "symlink" => return fail(&format!("{prefix}_SRC_SYMLINK"), format!("src path contains a symlink at some path component: {}", repr(src))),
        "not_found" | "not_a_directory" => {
            return fail(&format!("{prefix}_SRC_NOT_FOUND"), format!("src path does not resolve to an existing file: {}", repr(src)))
        }
        "wrong_type" => return fail(&format!("{prefix}_SRC_WRONG_TYPE"), format!("src path is not a non-empty regular file: {}", repr(src))),
        "os_error" => return fail(&format!("{prefix}_SRC_UNREADABLE"), format!("src path could not be read: {}", repr(src))),
        _ => {}
    }
    let content = r.content.unwrap_or_default();
    if let Err(e) = pyjson::decode_utf8_strict(&content) {
        return fail(&format!("{prefix}_SRC_INVALID_UTF8"), format!("src file is not valid UTF-8: {e}"));
    }
    Ok((content, r.sha256.unwrap_or_default()))
}

fn utf8_text(bytes: &[u8]) -> Option<String> {
    // bytes.decode("utf-8") (no newline translation)
    pyjson::decode_utf8_strict(bytes).ok().and_then(|t| t.to_utf8())
}

// ── append-raw ────────────────────────────────────────────────────────────────────────────────────────────────

fn validate_ledger(content: &[u8], prefix: &str) -> R<usize> {
    if content.is_empty() {
        return Ok(0);
    }
    if !content.ends_with(b"\n") {
        return fail(
            &format!("{prefix}_LEDGER_NOT_NEWLINE_TERMINATED"),
            "existing raw-evidence ledger does not end with a newline -- refusing to trust or append to a possibly-truncated/malformed ledger".into(),
        );
    }
    let text_ = match pyjson::decode_utf8_strict(content) {
        Ok(t) => t,
        Err(e) => return fail(&format!("{prefix}_LEDGER_INVALID_UTF8"), format!("existing raw-evidence ledger is not valid UTF-8: {e}")),
    };
    let t = text_.to_utf8().unwrap_or_default();
    let mut lines: Vec<&str> = t.split('\n').collect();
    lines.pop();
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        if line.is_empty() {
            return fail(&format!("{prefix}_LEDGER_BLANK_LINE"), format!("existing raw-evidence ledger has a blank line at position {n}"));
        }
        let entry = match pyjson::loads(line) {
            Ok(v) => v,
            Err(e) => return fail(&format!("{prefix}_LEDGER_MALFORMED_ENTRY"), format!("existing raw-evidence ledger line {n} is not valid JSON: {e}")),
        };
        if !is_dict(&entry) {
            return fail(&format!("{prefix}_LEDGER_ENTRY_NOT_AN_OBJECT"), format!("existing raw-evidence ledger line {n} is not a JSON object"));
        }
        let seq = get(&entry, "seq").unwrap_or(PyValue::None);
        let seq_int = match seq.peel() {
            PyValue::Int(x) => x.clone(),
            _ => return fail(&format!("{prefix}_LEDGER_SEQ_WRONG_TYPE"), format!("existing raw-evidence ledger line {n} has a non-integer 'seq'")),
        };
        if seq_int != num_bigint::BigInt::from(n) {
            return fail(
                &format!("{prefix}_LEDGER_SEQ_NOT_CONTIGUOUS"),
                format!("existing raw-evidence ledger line {n} has seq={seq_int}, expected {n} -- ledger must be duplicate-free, gap-free, and in order"),
            );
        }
    }
    Ok(lines.len())
}

fn append_jsonl_locked(dir: &Fd, name: &str, entry: Vec<(&str, V)>, prefix: &str) -> R<i64> {
    let kind = safe_stat_in_dir(dir, name);
    if kind == "symlink" {
        return fail(&format!("{prefix}_TARGET_IS_SYMLINK"), format!("{} is a symlink -- refusing to append through it", repr(name)));
    }
    if kind != "missing" && kind != "file" {
        return fail(&format!("{prefix}_TARGET_WRONG_TYPE"), format!("{} is not a regular file (kind={})", repr(name), repr(kind)));
    }
    let fd = match fsx::openat(dir.raw(), name, libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW, 0o644) {
        Ok(f) => f,
        Err(e) => {
            if e == libc::ELOOP {
                return fail(&format!("{prefix}_TARGET_IS_SYMLINK"), format!("{} is a symlink -- refusing to append through it", repr(name)));
            }
            return fail(&format!("{prefix}_IO_ERROR"), format!("could not open {}: {}", repr(name), fsx::os_error(e, Some(name))));
        }
    };
    fsx::flock(fd.raw(), libc::LOCK_EX);
    let result = (|| {
        fsx::lseek(fd.raw(), libc::SEEK_SET);
        let content = fsx::read_to_end(fd.raw()).unwrap_or_default();
        let prior = validate_ledger(&content, prefix)?;
        let seq = prior as i64 + 1;
        let mut items = entry;
        items.push(("seq", int(seq)));
        let line = format!("{}\n", dumps_compact_sorted(&obj(items)));
        fsx::lseek(fd.raw(), libc::SEEK_END);
        let _ = fsx::write_all(fd.raw(), line.as_bytes());
        let _ = fsx::fsync(fd.raw());
        Ok(seq)
    })();
    fsx::flock(fd.raw(), libc::LOCK_UN);
    result
}

fn set_path(record: &mut V, outer: &str, key: &str, value: V) {
    let d = dict_mut(record).expect("record is a dict");
    if d.get(outer).is_none() {
        d.set(outer, obj(vec![]));
    }
    let inner = d.get_mut(outer).expect("present");
    if let Some(m) = dict_mut(inner) {
        m.set(key, value);
    }
}
fn set_top(record: &mut V, key: &str, value: V) {
    dict_mut(record).expect("record is a dict").set(key, value);
}
fn sub_get(record: &V, outer: &str, key: &str) -> Option<V> {
    get(record, outer).and_then(|o| get(&o, key))
}
fn sub_str(record: &V, outer: &str, key: &str) -> Option<String> {
    sub_get(record, outer, key).and_then(|v| text(&v))
}

pub fn cmd_append_raw(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    let recorded = req(a, "--recorded-utc");
    validate_required_utc(recorded, "recorded-utc")?;
    let mut record = require_record(&repo_root, topic)?;
    let kind = req(a, "--kind");
    if !APPEND_RAW_KINDS.contains(&kind) {
        return fail(
            "APPEND_RAW_UNSUPPORTED_KIND",
            format!(
                "kind {} is not accepted by append-raw (bench_report/certificate come only from `publish-benchmark`, which validates them against the real benchmark verdict rather than accepting arbitrary raw text)",
                repr(kind)
            ),
        );
    }
    let src = req(a, "--src");
    let (content, sha) = resolve_src(&repo_root, src, "RAW_EVIDENCE")?;
    if kind == "simlog" {
        let simlog_rel = match sub_str(&record, "scaffolded", "simlog").filter(|x| !x.is_empty()) {
            Some(x) => x,
            None => {
                return fail(
                    "RAW_EVIDENCE_SIMLOG_NOT_SCAFFOLDED",
                    "simlog run directory not scaffolded yet for this topic -- run `init` with a task_class/conditions that require simlog first".into(),
                )
            }
        };
        let prefix = "RAW_EVIDENCE_SIMLOG";
        let parts = validate_prior_path(&repo_root, &simlog_rel, &["docs", "simlog"], prefix)?;
        let root = open_repo_root_fd(&repo_root)?;
        let dir = safe_mkdir_chain(&root, &parts, prefix)?;
        drop(root);
        let dest = match arg(a, "--dest-name").filter(|x| !x.is_empty()) {
            Some(d) => d.to_string(),
            None => path_name(src),
        };
        if dest.is_empty() || dest.contains('/') || dest == "." || dest == ".." {
            return fail("RAW_EVIDENCE_DEST_NAME_INVALID", format!("invalid --dest-name: {}", repr(&dest)));
        }
        if safe_stat_in_dir(&dir, &dest) != "missing" {
            return fail(
                "RAW_EVIDENCE_DEST_COLLISION",
                format!(
                    "a file named {} already exists in the simlog run directory -- append-only: pass a distinct --dest-name rather than overwriting existing raw evidence",
                    repr(&dest)
                ),
            );
        }
        let fd = match fsx::openat(dir.raw(), &dest, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW, 0o644) {
            Ok(f) => f,
            Err(e) if e == libc::EEXIST => {
                return fail(
                    "RAW_EVIDENCE_DEST_COLLISION",
                    format!("a file named {} was just created concurrently -- pass a distinct --dest-name", repr(&dest)),
                )
            }
            Err(e) => {
                eprintln!("OSError: {}", fsx::os_error(e, Some(&dest)));
                return Err(Exit { code: 1, stdout: String::new() });
            }
        };
        let _ = fsx::write_all(fd.raw(), &content);
        let _ = fsx::fsync(fd.raw());
        drop(fd);
        drop(dir);
        let recorded_rel = format!("{simlog_rel}/{dest}");
        set_path(&mut record, "raw_log_paths", "simlog", s(&simlog_rel));
        save_record(&repo_root, topic, &record)?;
        return Ok(success(topic, vec![("kind", s(kind)), ("recorded_path", s(&recorded_rel)), ("sha256", s(&sha))]));
    }
    let sidecar = repo_root.join("docs").join("_evidence").join(format!("{topic}.{kind}.raw.jsonl"));
    let sidecar_name = format!("{topic}.{kind}.raw.jsonl");
    let prefix = "RAW_EVIDENCE_SIDECAR";
    let dir = open_evidence_dir_fd(&repo_root, prefix)?;
    let seq = append_jsonl_locked(&dir, &sidecar_name, vec![("src", s(src)), ("sha256", s(&sha)), ("recorded_utc", s(recorded))], prefix)?;
    drop(dir);
    if POINTER_ONLY_KINDS.contains(&kind) {
        set_path(&mut record, "scaffolded", kind, s(src));
    }
    let rel = repo_relative(&repo_root, &sidecar);
    set_path(&mut record, "raw_log_paths", kind, s(&rel));
    save_record(&repo_root, topic, &record)?;
    Ok(success(topic, vec![("kind", s(kind)), ("recorded_path", s(&rel)), ("sha256", s(&sha)), ("seq", int(seq))]))
}

// ── set-narrative ─────────────────────────────────────────────────────────────────────────────────────────────

fn reject_comment_breaking(value: &str, code_prefix: &str, description: &str) -> R<()> {
    if value.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f) {
        return fail(
            &format!("{code_prefix}_CONTAINS_CONTROL_CHARACTER"),
            format!("{description} must not contain control characters or newlines (would corrupt the provenance HTML comment onto more than one physical line)"),
        );
    }
    for forbidden in ["-->", "<!--"] {
        if value.contains(forbidden) {
            return fail(
                &format!("{code_prefix}_CONTAINS_COMMENT_DELIMITER"),
                format!("{description} must not contain {} (would terminate or forge an HTML comment inside the provenance header)", repr(forbidden)),
            );
        }
    }
    Ok(())
}

fn validate_author(author: &str) -> R<()> {
    if author.is_empty() {
        return fail("NARRATIVE_AUTHOR_INVALID", "--author must be a non-empty string".into());
    }
    let n = author.chars().count();
    if n > AUTHOR_MAX_LEN {
        return fail("NARRATIVE_AUTHOR_TOO_LONG", format!("--author must be at most {AUTHOR_MAX_LEN} characters, got {n}"));
    }
    reject_comment_breaking(author, "NARRATIVE_AUTHOR", "--author")
}

fn canonical_narrative_src(repo_root: &Path, src: &str, kind: &str) -> Option<String> {
    if !BINDABLE_NARRATIVE_KINDS.contains(&kind) || gate::is_absolute_path_string(src) {
        return None;
    }
    let parts = gate::lexical_components(repo_root, repo_root, src)?;
    if parts.len() != 3 || parts[0] != "docs" || parts[1] != kind {
        return None;
    }
    if gate::is_dated_doc_basename(kind, &parts[2]) {
        Some(parts.join("/"))
    } else {
        None
    }
}

fn bind_existing_narrative(repo_root: &Path, mut record: V, a: &Args, bind_rel: &str, prior_target: &str, prefix: &str) -> R<Exit> {
    let kind = req(a, "--kind");
    let topic = req(a, "--topic");
    let (content, _) = resolve_src(repo_root, bind_rel, "NARRATIVE")?;
    let t = utf8_text(&content).unwrap_or_default();
    if t.contains(PLACEHOLDER_MARK) || t.contains(NARRATIVE_BEGIN) {
        return fail(
            "NARRATIVE_BIND_TARGET_IS_PLACEHOLDER",
            format!(
                "{} still carries publisher scaffold markers/placeholder -- bind only an authored document (or use the injection path with a temporary source)",
                repr(bind_rel)
            ),
        );
    }
    let mut removed = false;
    if prior_target != bind_rel {
        let status = sub_get(&record, "narrative_status", kind).and_then(|v| get_str(&v, "status"));
        let parts = validate_prior_path(repo_root, prior_target, &["docs", kind], prefix)?;
        let root = open_repo_root_fd(repo_root)?;
        let dir = safe_mkdir_chain(&root, &parts[..parts.len() - 1], prefix)?;
        drop(root);
        let base = &parts[parts.len() - 1];
        let authored = matches!(status.as_deref(), Some("authored") | Some("bound"));
        if safe_stat_in_dir(&dir, base) == "file" && !authored {
            if let Ok(fd) = fsx::openat(dir.raw(), base, libc::O_RDONLY | libc::O_NOFOLLOW, 0) {
                let old = fsx::read_once(fd.raw(), 1 << 20).unwrap_or_default();
                drop(fd);
                let old_text = String::from_utf8_lossy(&old);
                if old_text.contains(PLACEHOLDER_MARK) && old_text.contains(NARRATIVE_BEGIN) {
                    let _ = fsx::unlinkat(dir.raw(), base);
                    removed = true;
                }
            }
        }
    }
    let author = req(a, "--author");
    let utc = req(a, "--generated-utc");
    set_path(&mut record, "scaffolded", kind, s(bind_rel));
    set_path(
        &mut record,
        "narrative_status",
        kind,
        obj(vec![("status", s("bound")), ("author", s(author)), ("generated_utc", s(utc)), ("source", s(bind_rel))]),
    );
    save_record(repo_root, topic, &record)?;
    Ok(success(
        topic,
        vec![
            ("kind", s(kind)),
            ("path", s(bind_rel)),
            ("author", s(author)),
            ("status", s("bound")),
            (
                "binding",
                obj(vec![
                    ("source", s("path")),
                    ("scaffold_removed", b(removed)),
                    ("previous_scaffold", if prior_target == bind_rel { null() } else { s(prior_target) }),
                ]),
            ),
        ],
    ))
}

pub fn cmd_set_narrative(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    validate_required_utc(req(a, "--generated-utc"), "generated-utc")?;
    validate_author(req(a, "--author"))?;
    let nfile = req(a, "--narrative-file");
    reject_comment_breaking(nfile, "NARRATIVE_SOURCE", "--narrative-file")?;
    let mut record = require_record(&repo_root, topic)?;
    let kind = req(a, "--kind");
    if !NARRATIVE_KINDS.contains(&kind) {
        return fail("NARRATIVE_UNSUPPORTED_KIND", format!("kind must be one of ('plan', 'devlog', 'testlog', 'report'), got {}", repr(kind)));
    }
    let target = match sub_str(&record, "scaffolded", kind).filter(|x| !x.is_empty()) {
        Some(t) => t,
        None => return fail("NARRATIVE_TARGET_NOT_SCAFFOLDED", format!("no scaffolded '{kind}' document for this topic -- run `init` first")),
    };
    let prefix = format!("NARRATIVE_{}", kind.to_uppercase());
    if let Some(bind) = canonical_narrative_src(&repo_root, nfile, kind) {
        return bind_existing_narrative(&repo_root, record, a, &bind, &target, &prefix);
    }
    let root = open_repo_root_fd(&repo_root)?;
    let r = gate::resolve_and_stat_evidence(&root, &repo_root, &repo_root, &target, false, true);
    drop(root);
    if r.status != "ok" {
        return fail(
            &format!("{prefix}_TARGET_UNSAFE_OR_MISSING"),
            format!(
                "scaffolded '{kind}' path {} is missing or unsafe (status={}) -- refusing to trust the persisted record",
                repr(&target),
                r.status
            ),
        );
    }
    let (content, _) = resolve_src(&repo_root, nfile, "NARRATIVE")?;
    let narrative = utf8_text(&content).unwrap_or_default();
    if narrative.contains(NARRATIVE_BEGIN) || narrative.contains(NARRATIVE_END) {
        return fail(
            "NARRATIVE_CONTAINS_RESERVED_MARKER",
            format!("supplied narrative text must not contain the reserved {}/{} marker strings", repr(NARRATIVE_BEGIN), repr(NARRATIVE_END)),
        );
    }
    let scaffold = utf8_text(&r.content.unwrap_or_default()).unwrap_or_default();
    let bc = scaffold.matches(NARRATIVE_BEGIN).count();
    let ec = scaffold.matches(NARRATIVE_END).count();
    if bc != 1 || ec != 1 {
        return fail(
            "NARRATIVE_MARKERS_MALFORMED",
            format!(
                "scaffold file {} must contain exactly one BEGIN and one END narrative marker (found {bc} BEGIN, {ec} END) -- refusing to guess which occurrence is the real one",
                repr(&target)
            ),
        );
    }
    let bi = scaffold.find(NARRATIVE_BEGIN).unwrap();
    let ei = scaffold.find(NARRATIVE_END).unwrap();
    if ei < bi {
        return fail(
            "NARRATIVE_MARKERS_MISSING",
            format!(
                "scaffold file {} is missing its narrative markers -- it was not created by evidence_publisher.sh init and cannot be safely narrated",
                repr(&target)
            ),
        );
    }
    let author = req(a, "--author");
    let utc = req(a, "--generated-utc");
    let replacement = format!(
        "{NARRATIVE_BEGIN}\n<!-- provenance: author={author} generated_utc={utc} source={nfile} -->\n{}\n{NARRATIVE_END}",
        narrative.trim_end_matches('\n')
    );
    let new_text = format!("{}{}{}", &scaffold[..bi], replacement, &scaffold[ei + NARRATIVE_END.len()..]);
    let parts = validate_prior_path(&repo_root, &target, &["docs", kind], &prefix)?;
    let root = open_repo_root_fd(&repo_root)?;
    let dir = safe_mkdir_chain(&root, &parts[..parts.len() - 1], &prefix)?;
    drop(root);
    safe_replace_file(&dir, &parts[parts.len() - 1], new_text.as_bytes(), &prefix)?;
    drop(dir);
    set_path(
        &mut record,
        "narrative_status",
        kind,
        obj(vec![("status", s("authored")), ("author", s(author)), ("generated_utc", s(utc)), ("source", s(nfile))]),
    );
    save_record(&repo_root, topic, &record)?;
    Ok(success(topic, vec![("kind", s(kind)), ("path", s(&target)), ("author", s(author)), ("status", s("authored"))]))
}

// ── record-capacity-rejection ─────────────────────────────────────────────────────────────────────────────────

pub fn cmd_record_capacity_rejection(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    let recorded = req(a, "--recorded-utc");
    validate_required_utc(recorded, "recorded-utc")?;
    let mut record = require_record(&repo_root, topic)?;
    let tc = get(&record, "task_class").unwrap_or(PyValue::None);
    if text(&tc).as_deref() != Some("capacity_rejection") {
        return fail(
            "CAPACITY_REJECTION_WRONG_TASK_CLASS",
            format!("topic {} was initialized with task_class={}, not 'capacity_rejection'", repr(topic), tc.repr()),
        );
    }
    let (content, _) = resolve_src(&repo_root, req(a, "--gate-evidence-src"), "CAPACITY_REJECTION_GATE_EVIDENCE")?;
    let name = format!("{topic}_capacity_gate.txt");
    write_evidence_dir_file(&repo_root, &name, &content, "CAPACITY_REJECTION_GATE_EVIDENCE")?;
    set_top(
        &mut record,
        "capacity_rejection",
        obj(vec![("rejected", b(true)), ("gate_evidence", s(&name)), ("reason", s(req(a, "--reason"))), ("recorded_utc", s(recorded))]),
    );
    save_record(&repo_root, topic, &record)?;
    Ok(success(topic, vec![("gate_evidence", s(&name)), ("rejected", b(true))]))
}

// ── publish-benchmark ─────────────────────────────────────────────────────────────────────────────────────────

fn require_canonical_bench_src(repo_root: &Path, src: &str, kind: &str, prefix: &str) -> R<String> {
    let code = format!("{prefix}_SRC_NOT_CANONICAL");
    if gate::is_absolute_path_string(src) {
        return fail(&code, format!("{kind} src must be repo-relative under docs/benchmark/, got absolute {}", repr(src)));
    }
    let parts = gate::lexical_components(repo_root, repo_root, src);
    let ok = matches!(&parts, Some(p) if p.len() == 3 && p[0] == "docs" && p[1] == "benchmark");
    if !ok {
        return fail(
            &code,
            format!("{kind} src must be a file directly under docs/benchmark/ (the bench skill is the sole issuer — publisher binds, never copies): {}", repr(src)),
        );
    }
    let parts = parts.unwrap();
    if !bench_src_matches(kind, &parts[2]) {
        return fail(
            &code,
            format!("{kind} src basename does not follow the naming canon ({}): {}", bench_pattern(kind), repr(&parts[2])),
        );
    }
    Ok(parts.join("/"))
}

fn bench_stem(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let name = match name.rfind('.') {
        Some(i) => &name[..i],
        None => name,
    };
    if name.starts_with("benchmark_") {
        name.splitn(2, '_').nth(1).unwrap_or("").to_string()
    } else {
        name.splitn(3, '_').nth(2).unwrap_or("").to_string()
    }
}

fn certificates_matching_key(repo_root: &Path, key: &[String], prefix: &str) -> R<Vec<String>> {
    let root = open_repo_root_fd(repo_root)?;
    let dir = safe_mkdir_chain(&root, &strs(&["docs", "benchmark"]), prefix)?;
    drop(root);
    let mut names: Vec<String> = safe_listdir_names(dir.raw(), prefix)?.into_iter().filter(|n| bench_src_matches("benchmark", n)).collect();
    names.sort_by(|a, c| cmp_code_points(a, c));
    drop(dir);
    let root = open_repo_root_fd(repo_root)?;
    let mut matches = Vec::new();
    for name in names {
        let rel = format!("docs/benchmark/{name}");
        let r = gate::resolve_and_stat_evidence(&root, repo_root, repo_root, &rel, false, true);
        if r.status != "ok" {
            eprintln!(
                "[evidence_publisher] WARN certificate {rel} unreadable ({}) — excluded from key resolution (not silently treated as non-matching)",
                r.status
            );
            continue;
        }
        let t = match utf8_text(&r.content.unwrap_or_default()) {
            Some(t) => t,
            None => {
                eprintln!("[evidence_publisher] WARN certificate {rel} not UTF-8 — excluded");
                continue;
            }
        };
        let (fields, ok) = gate::parse_flat_certificate(&t);
        let other = if ok { gate::certificate_run_key(&fields) } else { None };
        if other.as_deref() == Some(key) {
            matches.push(rel);
        }
    }
    Ok(matches)
}

fn py_number(v: Option<V>) -> V {
    match v.map(|x| x.peel().clone()) {
        Some(PyValue::Int(n)) => PyValue::Int(n),
        Some(PyValue::Float(f)) => PyValue::Float(f),
        _ => PyValue::None,
    }
}

fn rubric_from_verdict_json(repo_root: &Path, src: &str, cli_verdict: &str, prefix: &str) -> R<Vec<(&'static str, V)>> {
    let (content, _) = resolve_src(repo_root, src, prefix)?;
    let t = pyjson::decode_utf8_strict(&content);
    let doc = match t.as_ref().map_err(|e| e.to_string()).and_then(|t| pyjson::loads_text(t).map_err(|e| e.to_string())) {
        Ok(d) => d,
        Err(e) => {
            return fail(
                &format!("{prefix}_UNPARSEABLE"),
                format!("--verdict-json-src {} is not parseable JSON: {e} -- refusing to derive rubric facts from an unreadable artifact", repr(src)),
            )
        }
    };
    if !is_dict(&doc) {
        return fail(&format!("{prefix}_WRONG_SHAPE"), format!("--verdict-json-src {} must contain a JSON object, got {}", repr(src), type_name(&doc)));
    }
    let doc_verdict = get(&doc, "verdict").unwrap_or(PyValue::None);
    let rub = match get(&doc, "rubric") {
        Some(r) if is_dict(&r) => r,
        _ => obj(vec![]),
    };
    let rub_get = |k: &str| get(&rub, k).unwrap_or(PyValue::None);
    if cli_verdict == gate::OBSERVATION_ONLY_VERDICT {
        let reason_code = get(&doc, "reason_code").unwrap_or(PyValue::None);
        let no_baseline =
            text(&doc_verdict).as_deref() == Some(gate::OBSERVATION_ONLY_JUDGE_VERDICT) && is_none(&rub_get("primary")) && is_none(&reason_code);
        if !no_baseline {
            return fail(
                &format!("{prefix}_VERDICT_MISMATCH"),
                format!(
                    "--verdict='OBSERVATION-ONLY' 는 판정기 산출물이 사다리 공백의 {}(rubric.primary=null ∧ reason_code=null)일 때만 대응한다 — verdict JSON 은 verdict={} rubric.primary={} reason_code={} 를 말한다",
                    repr(gate::OBSERVATION_ONLY_JUDGE_VERDICT),
                    doc_verdict.repr(),
                    rub_get("primary").repr(),
                    reason_code.repr()
                ),
            );
        }
    } else if text(&doc_verdict).as_deref() != Some(cli_verdict) {
        return fail(
            &format!("{prefix}_VERDICT_MISMATCH"),
            format!(
                "verdict JSON says verdict={} but --verdict={} -- refusing to record rubric facts from a contradictory artifact",
                doc_verdict.repr(),
                repr(cli_verdict)
            ),
        );
    }
    let raw_authority = rub_get("authority");
    let authority = text(&raw_authority).map(|t| py_strip(&t).to_string());
    if let Some(au) = authority.as_deref().filter(|x| !x.is_empty()) {
        if !gate::RUBRIC_AUTHORITIES.contains(&au) {
            return fail(
                &format!("{prefix}_AUTHORITY_UNKNOWN"),
                format!(
                    "verdict JSON rubric.authority={} is not one of {} -- fail closed rather than record an authority no consumer can reason about",
                    raw_authority.repr(),
                    list_s(&gate::RUBRIC_AUTHORITIES).repr()
                ),
            );
        }
    }
    let primary = text(&rub_get("source")).map(|t| py_strip(&t).to_string());
    Ok(vec![
        ("rubric_authority", authority.filter(|x| !x.is_empty()).map(|x| s(&x)).unwrap_or(PyValue::None)),
        ("floor_tps", py_number(get(&rub, "floor"))),
        ("ratio_M_over_primary", py_number(get(&rub, "ratio_M_over_primary"))),
        ("primary_source", primary.filter(|x| !x.is_empty()).map(|x| s(&x)).unwrap_or(PyValue::None)),
        ("rubric_source", s("verdict_json")),
    ])
}

fn identity_of(record: &V) -> V {
    match get(record, "identity") {
        Some(v) if v.is_truthy() => v,
        _ => obj(vec![]),
    }
}

pub fn cmd_publish_benchmark(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    validate_required_utc(req(a, "--generated-utc"), "generated-utc")?;
    let mut record = require_record(&repo_root, topic)?;
    let tc = get(&record, "task_class").unwrap_or(PyValue::None);
    if text(&tc).as_deref() != Some("full_benchmark") {
        return fail(
            "PUBLISH_BENCHMARK_WRONG_TASK_CLASS",
            format!("topic {} was initialized with task_class={}, not 'full_benchmark'", repr(topic), tc.repr()),
        );
    }
    let verdict = req(a, "--verdict");
    let cert_src_arg = arg(a, "--certificate-src").filter(|x| !x.is_empty());
    if verdict != "PASS" && cert_src_arg.is_some() {
        return fail(
            "PUBLISH_BENCHMARK_CERTIFICATE_WITHOUT_PASS",
            format!(
                "--certificate-src was supplied but --verdict={} != 'PASS' -- certificates are published PASS-only (matches work-manifest.schema.json's own CERTIFICATE_PRESENT_WITHOUT_PASS_VERDICT structural contract)",
                repr(verdict)
            ),
        );
    }
    let identity = identity_of(&record);
    let scaffolded_before = get(&record, "scaffolded").unwrap_or_else(|| obj(vec![]));
    let report_prefix = "PUBLISH_BENCHMARK_BENCH_REPORT";
    let cert_prefix = "PUBLISH_BENCHMARK_CERTIFICATE";
    let report_rel = require_canonical_bench_src(&repo_root, req(a, "--bench-report-src"), "bench_report", report_prefix)?;
    resolve_src(&repo_root, &report_rel, report_prefix)?;
    let vjs = arg(a, "--verdict-json-src").filter(|x| !x.is_empty());
    let mut rubric: Vec<(&'static str, V)> = RUBRIC_RECORD_FIELDS.iter().map(|f| (*f, PyValue::None)).collect();
    if verdict == gate::OBSERVATION_ONLY_VERDICT && vjs.is_none() {
        return fail(
            "PUBLISH_BENCHMARK_OBSERVATION_ONLY_REQUIRES_VERDICT_JSON",
            "--verdict OBSERVATION-ONLY requires --verdict-json-src (verdict_rule.sh 산출물의 NEEDS_RUBRIC · 사다리 공백이 근거다 — 선언만으로 판정을 회피할 수 없다)".into(),
        );
    }
    if let Some(v) = vjs {
        rubric = rubric_from_verdict_json(&repo_root, v, verdict, "PUBLISH_BENCHMARK_VERDICT_JSON")?;
    } else if verdict != "PASS" {
        eprintln!("[evidence_publisher] WARN PUBLISH_BENCHMARK_RUBRIC_CARRIER_ABSENT: non-PASS publication without --verdict-json-src — rubric authority will not reach the promotion gate");
    }
    let prior_cert = get(&scaffolded_before, "certificate").unwrap_or(PyValue::None);
    let prior_cert_s = text(&prior_cert);
    let clear_prior = verdict != "PASS" && prior_cert.is_truthy();
    let mut cert_rel: Option<String> = None;
    let mut cert_key: Option<Vec<String>> = None;
    let mut cert_measured: Option<String> = None;
    if verdict == "PASS" {
        if let Some(csrc) = cert_src_arg {
            let rel = require_canonical_bench_src(&repo_root, csrc, "benchmark", cert_prefix)?;
            let (cbytes, _) = resolve_src(&repo_root, &rel, cert_prefix)?;
            let ctext = utf8_text(&cbytes);
            let (fields, ok) = match &ctext {
                Some(t) => gate::parse_flat_certificate(t),
                None => (gate::CertFields::default(), false),
            };
            if !ok {
                return fail(
                    &format!("{cert_prefix}_UNPARSEABLE"),
                    format!(
                        "--certificate-src {} is not a recognized flat-scalar certificate artifact -- refusing to publish it (never publish an unverifiable file as if it were a real certificate)",
                        repr(csrc)
                    ),
                );
            }
            if fields.get("verdict") != Some("PASS") {
                return fail(
                    &format!("{cert_prefix}_VERDICT_MISMATCH"),
                    format!("certificate verdict={} (must be 'PASS') -- refusing to publish a contradictory certificate", repr_opt(fields.get("verdict"))),
                );
            }
            if gate::certificate_grade(&fields) != Some("full") {
                return fail(
                    &format!("{cert_prefix}_MODE_MISMATCH"),
                    format!(
                        "certificate benchmark_mode={} (must be 'full' or absent) -- refusing to publish a contradictory certificate",
                        repr_opt(fields.get("benchmark_mode"))
                    ),
                );
            }
            let mismatched = gate::certificate_strong_identity_mismatches(&fields, &identity);
            if !mismatched.is_empty() {
                return fail(
                    &format!("{cert_prefix}_IDENTITY_MISMATCH"),
                    format!(
                        "certificate identity field(s) {} do not match this publication's recorded identity -- refusing to publish a mismatched certificate",
                        list_s(&mismatched).repr()
                    ),
                );
            }
            let key = gate::certificate_run_key(&fields);
            cert_measured = fields.get("measured_utc").map(|x| py_strip(x).to_string()).filter(|x| !x.is_empty());
            let key = match key {
                Some(k) => k,
                None => {
                    return fail(
                        &format!("{cert_prefix}_KEY_UNRESOLVABLE"),
                        format!("certificate {} lacks a strong identity field or measured_utc -- cannot identify the measurement it certifies (fail-closed)", repr(&rel)),
                    )
                }
            };
            let matches = certificates_matching_key(&repo_root, &key, cert_prefix)?;
            if matches != vec![rel.clone()] {
                let same = matches.iter().filter(|m| **m != rel).all(|m| std::fs::read(repo_root.join(m)).map(|x| x == cbytes).unwrap_or(false));
                return fail(
                    &format!("{cert_prefix}_AMBIGUOUS"),
                    format!(
                        "measurement {}@{} is certified by {} tracked files, not one: {} -- {}",
                        key[0],
                        key[key.len() - 1],
                        matches.len(),
                        list_s(&matches).repr(),
                        if same {
                            "byte-identical: a duplicate layer; delete all but the original"
                        } else {
                            "DIFFERENT bytes for the same measurement: publication-chain defect"
                        }
                    ),
                );
            }
            if bench_stem(&rel) != bench_stem(&report_rel) {
                return fail(
                    "PUBLISH_BENCHMARK_REPORT_CERTIFICATE_STEM_MISMATCH",
                    format!(
                        "bench_report {} and certificate {} do not share a stem -- they were not issued for the same measurement",
                        repr(&report_rel),
                        repr(&rel)
                    ),
                );
            }
            cert_rel = Some(rel);
            cert_key = Some(key);
        }
    }
    // COMMIT PHASE
    let mut events: Vec<(&'static str, V)> = Vec::new();
    let before_report = get(&scaffolded_before, "bench_report").unwrap_or(PyValue::None);
    if !is_none(&before_report) && text(&before_report).as_deref() != Some(report_rel.as_str()) {
        events.push(("bench_report_rebound_from", before_report.clone()));
    }
    set_path(&mut record, "scaffolded", "bench_report", s(&report_rel));
    if clear_prior {
        set_path(&mut record, "scaffolded", "certificate", PyValue::None);
        events.push(("certificate_unbound_from", prior_cert.clone()));
    }
    let mut certificate_rel: V = if verdict == "PASS" { sub_get(&record, "scaffolded", "certificate").unwrap_or(PyValue::None) } else { PyValue::None };
    let rubric_v = obj(rubric.clone());
    let tc_s = text(&tc).unwrap_or_default();
    let conditions = get(&record, "conditions").unwrap_or(PyValue::None);
    let mut pending = required_for(&tc_s, &conditions, Some(verdict.to_string()), &rubric_v).iter().any(|x| x == "certificate");
    if let Some(rel) = &cert_rel {
        if !is_none(&prior_cert) && prior_cert_s.as_deref() != Some(rel.as_str()) {
            events.push(("certificate_rebound_from", prior_cert.clone()));
        }
        set_path(&mut record, "scaffolded", "certificate", s(rel));
        certificate_rel = s(rel);
        pending = false;
    }
    let mut bench_items: Vec<(&str, V)> = vec![("mode", s("full")), ("verdict", s(verdict))];
    bench_items.extend(rubric.iter().map(|(k, v)| (*k, v.clone())));
    bench_items.push(("measured_utc", opt_s(cert_measured.as_deref())));
    let bench = obj(bench_items);
    set_top(&mut record, "benchmark", bench.clone());
    let required = required_for(&tc_s, &conditions, Some(verdict.to_string()), &bench);
    set_top(&mut record, "required_evidence", v_list(&required));
    let or_group: Vec<String> = if tc_s == "capacity_rejection" { strs(&CAPACITY_REJECTION_OR_GROUP) } else { vec![] };
    set_top(&mut record, "or_group_scaffolded", v_list(&or_group));
    set_top(&mut record, "capacity_rejection_required", b(tc_s == "capacity_rejection"));
    save_record(&repo_root, topic, &record)?;
    let mut binding: Vec<(&str, V)> = vec![
        ("bench_report", s("bound")),
        ("certificate", if certificate_rel.is_truthy() { s("bound") } else { PyValue::None }),
        ("certificate_key", cert_key.map(|k| list_s(&k)).unwrap_or(PyValue::None)),
    ];
    binding.extend(events);
    Ok(success(
        topic,
        vec![
            ("verdict", s(verdict)),
            ("bench_report_path", s(&report_rel)),
            ("certificate_path", certificate_rel),
            ("pending_certificate", b(pending)),
            ("rubric", rubric_v),
            ("binding", obj(binding)),
        ],
    ))
}

// ── set-promotion-target ──────────────────────────────────────────────────────────────────────────────────────

pub fn cmd_set_promotion_target(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    let utc = req(a, "--generated-utc");
    validate_required_utc(utc, "generated-utc")?;
    let mut record = require_record(&repo_root, topic)?;
    let target = obj(vec![("kind", s("hint")), ("tag", s(req(a, "--tag"))), ("topology", s(req(a, "--topology"))), ("anchor", s(req(a, "--anchor")))]);
    if let Some(e) = promotion_target_error(&target) {
        return fail("PROMOTION_TARGET_INVALID", e);
    }
    let prior = get(&record, "promotion_target").filter(is_dict);
    let prior_prov = get(&record, "promotion_target_provenance").filter(is_dict);
    if prior.as_ref().map(|p| py_eq(p, &target)).unwrap_or(false) && prior_prov.is_some() {
        let rec_utc = get(prior_prov.as_ref().unwrap(), "generated_utc").unwrap_or(PyValue::None);
        return Ok(success(
            topic,
            vec![
                ("promotion_target", target),
                ("binding", obj(vec![("state", s("unchanged")), ("rebound_from", PyValue::None), ("recorded_utc", rec_utc)])),
            ],
        ));
    }
    let rebound = prior.clone().unwrap_or(PyValue::None);
    set_top(&mut record, "promotion_target", target.clone());
    set_top(
        &mut record,
        "promotion_target_provenance",
        obj(vec![("writer", s("set-promotion-target")), ("generated_utc", s(utc)), ("rebound_from", rebound.clone())]),
    );
    save_record(&repo_root, topic, &record)?;
    Ok(success(
        topic,
        vec![
            ("promotion_target", target),
            (
                "binding",
                obj(vec![
                    ("state", s(if prior.is_some() { "rebound" } else { "bound" })),
                    ("rebound_from", rebound),
                    ("recorded_utc", s(utc)),
                ]),
            ),
        ],
    ))
}

// ── finalize ──────────────────────────────────────────────────────────────────────────────────────────────────

fn run_with_timeout(mut cmd: std::process::Command, secs: u64) -> std::io::Result<(i32, Vec<u8>)> {
    use std::process::Stdio;
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut out, &mut v);
        v
    });
    let t_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut err, &mut v);
        v
    });
    let start = std::time::Instant::now();
    loop {
        if let Some(st) = child.try_wait()? {
            let o = t_out.join().unwrap_or_default();
            let _ = t_err.join();
            use std::os::unix::process::ExitStatusExt;
            let code = st.code().unwrap_or_else(|| -st.signal().unwrap_or(1));
            return Ok((code, o));
        }
        if start.elapsed().as_secs() >= secs {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out"));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

pub fn cmd_finalize(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    let record = require_record(&repo_root, topic)?;
    let pii = match arg(a, "--pii-scan-json").filter(|x| !x.is_empty()) {
        Some(p) => load_json(p, "pii_scan")?,
        None => obj(vec![("passed", b(false))]),
    };
    let runtime = match arg(a, "--runtime-json").filter(|x| !x.is_empty()) {
        Some(p) => Some(load_json(p, "runtime")?),
        None => None,
    };
    let capacity = match arg(a, "--capacity-rejection-json").filter(|x| !x.is_empty()) {
        Some(p) => load_json(p, "capacity_rejection")?,
        None => get(&record, "capacity_rejection").unwrap_or(PyValue::None),
    };
    let manifest_dir = repo_root.join("docs").join("_evidence");
    let scaffolded = get(&record, "scaffolded").unwrap_or_else(|| obj(vec![]));
    let narrative_status = get(&record, "narrative_status").unwrap_or_else(|| obj(vec![]));
    let mut evidence = PyDict::new();
    let root = open_repo_root_fd(&repo_root)?;
    for key in ALL_EVIDENCE_KEYS {
        let rel = match get(&scaffolded, key).filter(|v| v.is_truthy()).and_then(|v| text(&v)) {
            Some(r) => r,
            None => {
                evidence.set(key, PyValue::None);
                continue;
            }
        };
        if NARRATIVE_KINDS.contains(&key) {
            let st = get(&narrative_status, key).and_then(|v| get_str(&v, "status"));
            if !st.map(|x| NARRATIVE_STATUSES.contains(&x.as_str())).unwrap_or(false) {
                evidence.set(key, PyValue::None);
                continue;
            }
            let r = gate::resolve_and_stat_evidence(&root, &repo_root, &repo_root, &rel, false, true);
            if r.status != "ok" {
                evidence.set(key, PyValue::None);
                continue;
            }
            match utf8_text(&r.content.unwrap_or_default()) {
                Some(t) if !t.contains(PLACEHOLDER_MARK) => {}
                _ => {
                    evidence.set(key, PyValue::None);
                    continue;
                }
            }
        }
        let joined = format!("{}/{}", path_str(&repo_root), rel);
        evidence.set(key, obj(vec![("path", s(&relpath(&joined, &path_str(&manifest_dir))))]));
    }
    drop(root);
    let tc = text(&get(&record, "task_class").unwrap_or(PyValue::None)).unwrap_or_default();
    let mut items: Vec<(&str, V)> = vec![
        ("schema_version", int(SCHEMA_VERSION)),
        ("task_class", s(&tc)),
        ("identity", get(&record, "identity").unwrap_or(PyValue::None)),
        ("evidence", PyValue::Dict(evidence)),
        ("conditions", get(&record, "conditions").filter(|v| v.is_truthy()).unwrap_or_else(|| obj(vec![]))),
        (
            "benchmark",
            get(&record, "benchmark").filter(|v| v.is_truthy()).unwrap_or_else(|| obj(vec![("mode", PyValue::None), ("verdict", PyValue::None)])),
        ),
        ("pii_scan", pii),
    ];
    if tc == "capacity_rejection" {
        items.push(("capacity_rejection", capacity));
    } else if let Some(rt) = runtime {
        items.push(("runtime", rt));
    }
    if let Some(pt) = get(&record, "promotion_target").filter(is_dict) {
        items.push(("promotion_target", pt));
    }
    let manifest = obj(items);
    let errors = gate::validate_whole(&manifest);
    if !errors.is_empty() {
        let details: Vec<String> = errors.iter().map(|(p, m)| format!("{p}: {m}")).collect();
        return fail("FINALIZE_WORK_MANIFEST_SCHEMA_INVALID", details.join("; "));
    }
    let name = format!("{topic}.work-manifest.json");
    write_evidence_dir_file(&repo_root, &name, dumps_pretty_sorted(&manifest).as_bytes(), "WORK_MANIFEST")?;
    let manifest_path = manifest_dir.join(&name);
    // C4 library ingest
    let wd = repo_root.join(".claude/skills/wiki-desk/src/cli.ts");
    if wd.is_file() {
        let o = std::process::Command::new("node")
            .arg(&wd)
            .args(["sync", "--apply", "--root"])
            .arg(&repo_root)
            .current_dir(&repo_root)
            .stdin(std::process::Stdio::inherit())
            .output();
        let o = match o {
            Ok(o) => o,
            Err(e) => {
                eprintln!("FileNotFoundError: {e}");
                return Err(Exit { code: 1, stdout: String::new() });
            }
        };
        let rc = o.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
        let parsed = if rc == 0 || rc == 4 { pyjson::loads(&stdout).ok() } else { None };
        let msg = match parsed {
            None => {
                let src = if !stderr.is_empty() { stderr } else { stdout };
                let t = py_strip(&src);
                let chars: Vec<char> = t.chars().collect();
                let tail: String = chars[chars.len().saturating_sub(200)..].iter().collect();
                format!("입고 실패(rc={rc}): {tail}")
            }
            Some(out) => {
                if get_str(&out, "ingest_status").as_deref() == Some("not-yet-ingested") {
                    format!("not-yet-ingested — 서고 미도달(stage={}) · 다음 입고가 따라잡는다", get(&out, "stage").map(|v| v.display_text().to_utf8().unwrap_or_default()).unwrap_or("None".into()))
                } else {
                    let n = |k: &str| match get(&out, k).map(|v| v.peel().clone()) {
                        Some(PyValue::List(x)) => x.len(),
                        Some(PyValue::Dict(x)) => x.len(),
                        Some(PyValue::Str(x)) => x.len(),
                        _ => 0,
                    };
                    format!("입고 완료 — created={} updated={} archived={}", n("created"), n("updated"), n("archived"))
                }
            }
        };
        eprintln!("[evidence_publisher] 서가 입고(C4): {msg}");
    } else {
        eprintln!("[evidence_publisher] 서가 입고 건너뜀 — {} 부재(침묵 누락 ✗)", path_str(&wd));
    }
    let mut cmd = std::process::Command::new("bash");
    cmd.arg(ctx.repo.join(".claude/policies/runtime/completion_gate.sh"))
        .args(["verify", "--manifest"])
        .arg(&manifest_path)
        .arg("--repo-root")
        .arg(&repo_root);
    match run_with_timeout(cmd, 60) {
        Ok((code, out)) => Err(Exit { code, stdout: String::from_utf8_lossy(&out).into_owned() }),
        Err(e) => {
            eprintln!("subprocess.TimeoutExpired: completion_gate verify: {e}");
            Err(Exit { code: 1, stdout: String::new() })
        }
    }
}

// ── init ──────────────────────────────────────────────────────────────────────────────────────────────────────

fn bound_certificate_grade(repo_root: &Path, rel: &str) -> Option<&'static str> {
    let bytes = std::fs::read(repo_root.join(rel)).ok()?;
    let t = pyjson::decode_file_text(&bytes).ok()?.to_utf8()?;
    let (fields, ok) = gate::parse_flat_certificate(&t);
    if ok {
        gate::certificate_grade(&fields)
    } else {
        None
    }
}

pub fn measurement_config(text_: &str) -> (Option<V>, String) {
    match bench_section::parse_measurement_config(text_) {
        Err(e) => (None, format!("unparseable({e})")),
        Ok(None) => (None, "absent(측정 구성 표 없음 — 표 신설 전 리포트)".to_string()),
        Ok(Some(mc)) => (Some(mc), "parsed".to_string()),
    }
}

fn load_classify(ctx: &Ctx) -> R<classify::Owner> {
    classify::load(&ctx.repo).or_else(|e| {
        fail(
            "DOWNGRADE_REASON_VOCABULARY_UNAVAILABLE",
            format!(
                "cannot load the downgrade vocabulary owner {}: {e} -- refusing to continue without it",
                path_str(&ctx.repo.join(classify::NATIVE_REL))
            ),
        )
    })
}

fn owner_crash(e: String) -> Exit {
    eprintln!("RuntimeError: {e}");
    Exit { code: 1, stdout: String::new() }
}

pub fn cmd_init(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    let utc = req(a, "--generated-utc");
    validate_required_utc(utc, "generated-utc")?;
    let identity = load_json(req(a, "--identity-json"), "identity")?;
    if !is_dict(&identity) {
        return fail("INIT_IDENTITY_NOT_AN_OBJECT", format!("--identity-json must contain a JSON object, got {}", type_name(&identity)));
    }
    let conditions = match arg(a, "--conditions-json").filter(|x| !x.is_empty()) {
        Some(p) => load_json(p, "conditions")?,
        None => obj(vec![]),
    };
    if !is_dict(&conditions) {
        return fail("INIT_CONDITIONS_NOT_AN_OBJECT", format!("--conditions-json must contain a JSON object, got {}", type_name(&conditions)));
    }
    let mut errs = gate::validate_definition(&identity, "identity", "$.identity");
    errs.extend(gate::validate_property(&conditions, "conditions", "$.conditions"));
    if !errs.is_empty() {
        let details: Vec<String> = errs.iter().map(|(p, m)| format!("{p}: {m}")).collect();
        return fail("INIT_INPUT_SCHEMA_INVALID", details.join("; "));
    }
    let task_class = req(a, "--task-class");
    let prior_opt = load_record(&repo_root, topic)?;
    let prior = prior_opt.clone().unwrap_or_else(|| obj(vec![]));
    let has_prior = prior_opt.is_some();
    let mut reclass = get(&prior, "reclassification").unwrap_or(PyValue::None);
    let dfrom = arg(a, "--downgrade-from");
    let dreason = arg(a, "--downgrade-reason");
    let bmode = arg(a, "--benchmark-mode");
    let bverdict = arg(a, "--benchmark-verdict");
    if dreason.is_some() && dfrom.is_none() {
        return fail("INIT_DOWNGRADE_REASON_WITHOUT_FROM", "--downgrade-reason is only meaningful with --downgrade-from".into());
    }
    if let Some(from) = dfrom {
        if task_class != DOWNGRADE_TO_CLASS {
            return fail(
                "INIT_DOWNGRADE_TARGET_INVALID",
                format!("--downgrade-from {} reclassifies only into --task-class {}, got {}", repr(from), repr(DOWNGRADE_TO_CLASS), repr(task_class)),
            );
        }
        let reason = match dreason.filter(|x| !x.is_empty()) {
            Some(r) => r,
            None => {
                return fail(
                    "INIT_DOWNGRADE_REASON_REQUIRED",
                    "--downgrade-from requires --downgrade-reason (a machine-event downgrade reason recorded by classify_cell -- reclassification without a reason is a silent class change)".into(),
                )
            }
        };
        let owner = load_classify(ctx)?;
        if !owner.downgrade_reasons.iter().any(|r| r == reason) {
            return fail(
                "INIT_DOWNGRADE_REASON_UNKNOWN",
                format!(
                    "--downgrade-reason {} is not a machine-event downgrade reason {} (variance is never a downgrade reason)",
                    repr(reason),
                    list_s(&owner.downgrade_reasons).repr()
                ),
            );
        }
        if !has_prior {
            return fail(
                "INIT_DOWNGRADE_PRIOR_ABSENT",
                format!(
                    "topic {} has no publication record -- nothing to downgrade; init a {} topic directly for a declared lite-only cell",
                    repr(topic),
                    repr(DOWNGRADE_TO_CLASS)
                ),
            );
        }
        let prior_class = get(&prior, "task_class").unwrap_or(PyValue::None);
        let pcs = text(&prior_class);
        if pcs.as_deref() == Some(from) {
            reclass = obj(vec![("from", s(from)), ("to", s(DOWNGRADE_TO_CLASS)), ("reason", s(reason)), ("reason_source", PyValue::None)]);
        } else if pcs.as_deref() == Some(DOWNGRADE_TO_CLASS)
            && is_dict(&reclass)
            && get_str(&reclass, "from").as_deref() == Some(from)
            && get_str(&reclass, "reason").as_deref() == Some(reason)
        {
        } else {
            let extra = if is_dict(&reclass) {
                format!(
                    " (already reclassified from {} with reason {})",
                    get(&reclass, "from").unwrap_or(PyValue::None).repr(),
                    get(&reclass, "reason").unwrap_or(PyValue::None).repr()
                )
            } else {
                String::new()
            };
            return fail(
                "INIT_DOWNGRADE_PRIOR_MISMATCH",
                format!("topic {} was initialized with task_class={}{extra} -- --downgrade-from {} does not apply", repr(topic), prior_class.repr(), repr(from)),
            );
        }
        if let Some(cert) = sub_get(&prior, "scaffolded", "certificate").filter(|v| v.is_truthy()) {
            let rel = text(&cert).unwrap_or_default();
            if bound_certificate_grade(&repo_root, &rel) != Some("lite") {
                return fail(
                    "INIT_DOWNGRADE_CERTIFICATE_BOUND",
                    format!(
                        "topic {} has a bound certificate {} -- a downgraded cell never receives a full certificate; refusing to reclassify a contradictory record",
                        repr(topic),
                        cert.repr()
                    ),
                );
            }
        }
        if !matches!(bmode, None | Some("lite")) {
            return fail(
                "INIT_DOWNGRADE_MODE_CONTRADICTION",
                format!("a downgrade reclassifies the measurement to benchmark.mode='lite'; --benchmark-mode {} contradicts it", repr(bmode.unwrap_or(""))),
            );
        }
        if pcs.as_deref() == Some(from) {
            let report_rel = match sub_get(&prior, "scaffolded", "bench_report").filter(|v| v.is_truthy()).and_then(|v| text(&v)) {
                Some(r) => r,
                None => {
                    return fail(
                        "INIT_DOWNGRADE_REPORT_ABSENT",
                        format!(
                            "topic {} has no bound bench_report -- a downgrade is recorded in the sweep report's measurement-config table (publish-benchmark binds it); nothing shows this cell was downgraded",
                            repr(topic)
                        ),
                    )
                }
            };
            let (rbytes, _) = resolve_src(&repo_root, &report_rel, "INIT_DOWNGRADE_REPORT")?;
            let (mc, mc_state) = measurement_config(&utf8_text(&rbytes).unwrap_or_default());
            let owner = load_classify(ctx)?;
            let kind_v = owner.bench_mode_kind(mc.as_ref().unwrap_or(&PyValue::None)).map_err(owner_crash)?;
            let mc_get = |k: &str| mc.as_ref().and_then(|m| get(m, k)).unwrap_or(PyValue::None);
            let good = mc.is_some()
                && text(&kind_v).as_deref() == Some("downgraded-lite")
                && text(&mc_get("downgrade_reason")).as_deref() == Some(reason);
            if !good {
                return fail(
                    "INIT_DOWNGRADE_EVIDENCE_MISMATCH",
                    format!(
                        "bound bench_report {} does not record this downgrade -- measurement-config table state={mc_state}, bench_mode={}, kind={}, downgrade_reason={} vs --downgrade-reason {}; a downgrade comes only from classify_cell's machine-event record, never from the operator's word",
                        repr(&report_rel),
                        mc_get("bench_mode").repr(),
                        kind_v.repr(),
                        mc_get("downgrade_reason").repr(),
                        repr(reason)
                    ),
                );
            }
            let kind_t = kind_v.display_text().to_utf8().unwrap_or_default();
            if let Some(d) = dict_mut(&mut reclass) {
                d.set(
                    "reason_source",
                    s(&format!("derived(bench_report({report_rel}) 측정 구성 표 · kind={kind_t} · downgrade_reason={reason} = --downgrade-reason 대조 일치)")),
                );
            }
        }
    }
    let prior_benchmark_value = get(&prior, "benchmark");
    let pbr = prior_benchmark_value.clone().filter(is_dict).unwrap_or_else(|| obj(vec![]));
    let verdict: Option<String> = match bverdict {
        Some(v) => Some(v.to_string()),
        None => get_str(&pbr, "verdict"),
    };
    let required = required_for(task_class, &conditions, verdict.clone(), &pbr);
    let or_group: Vec<String> = if task_class == "capacity_rejection" { strs(&CAPACITY_REJECTION_OR_GROUP) } else { vec![] };
    let cap_req = task_class == "capacity_rejection";
    let slug = arg(a, "--report-slug").filter(|x| !x.is_empty());
    if required.iter().any(|x| x == "report") && slug.is_none() {
        return fail(
            "INIT_REPORT_SLUG_REQUIRED",
            "task_class/conditions require a 'report' evidence item -- pass --report-slug (<분류>_<YYMMDDHH>[_MM_SS]_<한글제목> per docs.md's report/ rule)".into(),
        );
    }
    if has_prior {
        let mut mism: Vec<String> = Vec::new();
        for (field, value) in [("task_class", s(task_class)), ("generated_utc", s(utc)), ("identity", identity.clone()), ("conditions", conditions.clone())] {
            if !get(&prior, field).map(|p| py_eq(&p, &value)).unwrap_or(false) {
                mism.push(field.to_string());
            }
        }
        if dfrom.is_some() {
            mism.retain(|x| x != "task_class");
        }
        if let Some(pb) = get(&prior, "benchmark").filter(is_dict) {
            for (field, value) in [("mode", bmode), ("verdict", bverdict)] {
                if dfrom.is_some() && field == "mode" {
                    continue;
                }
                if let Some(v) = value {
                    let pv = get(&pb, field).unwrap_or(PyValue::None);
                    if !is_none(&pv) && text(&pv).as_deref() != Some(v) {
                        mism.push(format!("benchmark.{field}"));
                    }
                }
            }
        }
        if !mism.is_empty() {
            mism.sort_by(|a, c| cmp_code_points(a, c));
            return fail("INIT_IMMUTABLE_REBIND", format!("existing publication identity/time/conditions/benchmark cannot be rebound: {}", mism.join(", ")));
        }
    }
    let prior_scaffolded = get(&prior, "scaffolded").unwrap_or_else(|| obj(vec![]));
    let mut kinds: Vec<String> = required.iter().filter(|k| SCAFFOLDABLE_KINDS.contains(&k.as_str())).cloned().collect();
    for k in &or_group {
        if !kinds.contains(k) {
            kinds.push(k.clone());
        }
    }
    let mut scaffolded = PyDict::new();
    for k in ["plan", "devlog", "testlog", "simlog", "report", "bench_report", "certificate", "verification", "commit"] {
        scaffolded.set(k, get(&prior_scaffolded, k).unwrap_or(PyValue::None));
    }
    let mut created: Vec<String> = Vec::new();
    let mut existed: Vec<String> = Vec::new();
    let root = open_repo_root_fd(&repo_root)?;
    let prior_ns = get(&prior, "narrative_status").unwrap_or_else(|| obj(vec![]));
    for kind in &kinds {
        let prior_path_v = get(&prior_scaffolded, kind).unwrap_or(PyValue::None);
        let prior_path = text(&prior_path_v).filter(|x| !x.is_empty());
        let status = get(&prior_ns, kind).filter(|v| v.is_truthy()).and_then(|v| get_str(&v, "status"));
        if status.as_deref() == Some("bound") && prior_path_v.is_truthy() {
            scaffolded.set(kind, prior_path_v.clone());
            existed.push(kind.clone());
            continue;
        }
        let (rel, was_created) = if DATED_KINDS.contains(&kind.as_str()) {
            ensure_dated_scaffold(&root, &repo_root, kind, topic, task_class, utc, prior_path.as_deref())?
        } else if kind == "simlog" {
            ensure_simlog_scaffold(&root, &repo_root, topic, utc, prior_path.as_deref())?
        } else {
            ensure_report_scaffold(&root, &repo_root, topic, task_class, utc, slug, prior_path.as_deref())?
        };
        scaffolded.set(kind, s(&rel));
        if was_created { created.push(kind.clone()) } else { existed.push(kind.clone()) }
    }
    drop(root);
    let pending = sorted_set(required.iter().filter(|k| !SCAFFOLDABLE_KINDS.contains(&k.as_str())).cloned());
    let prior_benchmark = match get(&prior, "benchmark") {
        Some(v) if is_dict(&v) => v,
        _ => obj(vec![("mode", PyValue::None), ("verdict", PyValue::None)]),
    };
    let mut bench = PyDict::new();
    bench.set("mode", bmode.map(s).unwrap_or_else(|| get(&prior_benchmark, "mode").unwrap_or(PyValue::None)));
    bench.set(
        "verdict",
        if bverdict.is_some() { opt_s(verdict.as_deref()) } else { get(&prior_benchmark, "verdict").unwrap_or(PyValue::None) },
    );
    for f in BENCHMARK_CARRY_FIELDS {
        if let Some(v) = get(&prior_benchmark, f).filter(|v| !is_none(v)) {
            bench.set(f, v);
        }
    }
    if is_dict(&reclass) {
        bench.set("mode", s("lite"));
    }
    let mut rec = PyDict::new();
    rec.set("schema_version", int(SCHEMA_VERSION));
    rec.set("publication_id", s(topic));
    rec.set("task_class", s(task_class));
    rec.set("generated_utc", s(utc));
    rec.set("identity", identity);
    rec.set("conditions", conditions);
    rec.set("benchmark", PyValue::Dict(bench));
    rec.set("required_evidence", v_list(&required));
    rec.set("or_group_scaffolded", v_list(&or_group));
    rec.set("capacity_rejection_required", b(cap_req));
    rec.set("scaffolded", PyValue::Dict(scaffolded.clone()));
    rec.set("narrative_status", get(&prior, "narrative_status").unwrap_or_else(|| obj(vec![])));
    rec.set("raw_log_paths", get(&prior, "raw_log_paths").unwrap_or_else(|| obj(vec![])));
    rec.set("capacity_rejection", get(&prior, "capacity_rejection").unwrap_or(PyValue::None));
    if is_dict(&reclass) {
        rec.set("reclassification", reclass.clone());
    }
    if let (Some(pt), Some(pp)) = (get(&prior, "promotion_target").filter(is_dict), get(&prior, "promotion_target_provenance").filter(is_dict)) {
        rec.set("promotion_target", pt);
        rec.set("promotion_target_provenance", pp);
    }
    let record = PyValue::Dict(rec);
    let path = save_record(&repo_root, topic, &record)?;
    created.sort_by(|a, c| cmp_code_points(a, c));
    existed.sort_by(|a, c| cmp_code_points(a, c));
    Ok(success(
        topic,
        vec![
            ("task_class", s(task_class)),
            ("required_evidence", v_list(&required)),
            ("or_group_scaffolded", v_list(&or_group)),
            ("capacity_rejection_required", b(cap_req)),
            ("scaffolded", PyValue::Dict(scaffolded)),
            ("pending_evidence", v_list(&pending)),
            ("created_now", v_list(&created)),
            ("already_existed", v_list(&existed)),
            ("record_path", s(&repo_relative(&repo_root, &path))),
            ("reclassification", if is_dict(&reclass) { reclass } else { PyValue::None }),
        ],
    ))
}

// ── publish-lite-report ───────────────────────────────────────────────────────────────────────────────────────

pub fn cmd_publish_lite_report(ctx: &Ctx, a: &Args) -> R<Exit> {
    let repo_root = resolve_repo_root(ctx, arg(a, "--repo-root"))?;
    let topic = req(a, "--topic");
    validate_topic_or_die(topic)?;
    validate_required_utc(req(a, "--generated-utc"), "generated-utc")?;
    let mut record = require_record(&repo_root, topic)?;
    let tc = get(&record, "task_class").unwrap_or(PyValue::None);
    if text(&tc).as_deref() != Some(DOWNGRADE_TO_CLASS) {
        return fail(
            "PUBLISH_LITE_REPORT_WRONG_TASK_CLASS",
            format!(
                "topic {} was initialized with task_class={}, not {} (full_benchmark binds its report with publish-benchmark)",
                repr(topic),
                tc.repr(),
                repr(DOWNGRADE_TO_CLASS)
            ),
        );
    }
    let prefix = "PUBLISH_LITE_REPORT_BENCH_REPORT";
    let report_rel = require_canonical_bench_src(&repo_root, req(a, "--bench-report-src"), "bench_report", prefix)?;
    let (rbytes, _) = resolve_src(&repo_root, &report_rel, prefix)?;
    let (mc, state) = measurement_config(&utf8_text(&rbytes).unwrap_or_default());
    let mode = mc.as_ref().and_then(|m| get(m, "bench_mode")).unwrap_or(PyValue::None);
    if !(mc.is_some() && text(&mode).as_deref() == Some(LITE_BENCH_MODE)) {
        return fail(
            "PUBLISH_LITE_REPORT_NOT_LITE",
            format!(
                "{} measurement-config table does not say bench_mode=lite (state={state}, bench_mode={}) -- refusing to bind a non-lite (or pre-contract) report as the lite evidence of a map-only topic",
                repr(&report_rel),
                mode.repr()
            ),
        );
    }
    let mut cert_rel: Option<String> = None;
    let mut cert_measured: Option<String> = None;
    if let Some(csrc) = arg(a, "--certificate-src").filter(|x| !x.is_empty()) {
        let cp = "PUBLISH_LITE_REPORT_CERTIFICATE";
        let rel = require_canonical_bench_src(&repo_root, csrc, "benchmark", cp)?;
        let (cbytes, _) = resolve_src(&repo_root, &rel, cp)?;
        let (fields, ok) = match utf8_text(&cbytes) {
            Some(t) => gate::parse_flat_certificate(&t),
            None => (gate::CertFields::default(), false),
        };
        if !ok {
            return fail(&format!("{cp}_UNPARSEABLE"), format!("--certificate-src {} is not a recognized flat-scalar certificate", repr(csrc)));
        }
        if gate::certificate_grade(&fields) != Some("lite") || fields.get("verdict") != Some(gate::LITE_CERT_VERDICT) || fields.get("lite_verdict") != Some("pass") {
            return fail(
                &format!("{cp}_GRADE_MISMATCH"),
                format!(
                    "certificate benchmark_mode={} verdict={} lite_verdict={} -- a map-only topic binds only a lite-grade certificate (lite · not_applicable · pass)",
                    repr_opt(fields.get("benchmark_mode")),
                    repr_opt(fields.get("verdict")),
                    repr_opt(fields.get("lite_verdict"))
                ),
            );
        }
        let mism = gate::certificate_strong_identity_mismatches(&fields, &identity_of(&record));
        if !mism.is_empty() {
            return fail(
                &format!("{cp}_IDENTITY_MISMATCH"),
                format!("certificate identity field(s) {} do not match this publication's identity", list_s(&mism).repr()),
            );
        }
        let key = gate::certificate_run_key(&fields);
        cert_measured = fields.get("measured_utc").map(|x| py_strip(x).to_string()).filter(|x| !x.is_empty());
        let key = match key {
            Some(k) => k,
            None => return fail(&format!("{cp}_KEY_UNRESOLVABLE"), format!("certificate {} lacks a strong identity field or measured_utc", repr(&rel))),
        };
        let matches = certificates_matching_key(&repo_root, &key, cp)?;
        if matches != vec![rel.clone()] {
            return fail(
                &format!("{cp}_AMBIGUOUS"),
                format!("measurement {}@{} is certified by {} files: {}", key[0], key[key.len() - 1], matches.len(), list_s(&matches).repr()),
            );
        }
        if bench_stem(&rel) != bench_stem(&report_rel) {
            return fail(
                &format!("{cp}_STEM_MISMATCH"),
                format!(
                    "bench_report {} and certificate {} do not share a stem -- they were not issued for the same measurement",
                    repr(&report_rel),
                    repr(&rel)
                ),
            );
        }
        cert_rel = Some(rel);
    }
    let mut events: Vec<(&str, V)> = Vec::new();
    let prev_report = sub_get(&record, "scaffolded", "bench_report").unwrap_or(PyValue::None);
    if !is_none(&prev_report) && text(&prev_report).as_deref() != Some(report_rel.as_str()) {
        events.push(("bench_report_rebound_from", prev_report));
    }
    set_path(&mut record, "scaffolded", "bench_report", s(&report_rel));
    let mut bench = get(&record, "benchmark").filter(|v| v.is_truthy()).and_then(|v| dict(&v).cloned()).unwrap_or_default();
    bench.set("mode", s("lite"));
    if bench.get("verdict").is_none() {
        bench.set("verdict", PyValue::None);
    }
    if let Some(rel) = &cert_rel {
        let prev = sub_get(&record, "scaffolded", "certificate").unwrap_or(PyValue::None);
        if !is_none(&prev) && text(&prev).as_deref() != Some(rel.as_str()) {
            events.push(("certificate_rebound_from", prev));
        }
        set_path(&mut record, "scaffolded", "certificate", s(rel));
        bench.set("measured_utc", opt_s(cert_measured.as_deref()));
        events.push(("certificate", s("bound")));
    }
    let bench_v = PyValue::Dict(bench);
    set_top(&mut record, "benchmark", bench_v.clone());
    let tcs = text(&tc).unwrap_or_default();
    let conditions = get(&record, "conditions").unwrap_or(PyValue::None);
    let required = required_for(&tcs, &conditions, get_str(&bench_v, "verdict"), &bench_v);
    set_top(&mut record, "required_evidence", v_list(&required));
    save_record(&repo_root, topic, &record)?;
    let mut binding: Vec<(&str, V)> = vec![("bench_report", s("bound"))];
    binding.extend(events);
    Ok(success(
        topic,
        vec![
            ("bench_report_path", s(&report_rel)),
            ("benchmark_mode", s("lite")),
            ("certificate_path", opt_s(cert_rel.as_deref())),
            ("binding", obj(binding)),
        ],
    ))
}
