//! The pre-commit tripwires of `runtime_selftest.py` (`--tripwires-only`, ① … ⑩ + executor wiring), same order,
//! same messages, same exit codes. `Fail::Selftest` is the original's `RuntimeSelftestFailure` (caught and printed as
//! `[tripwire] FAIL …`); `Fail::Crash` is any other exception (the original's uncaught traceback, exit 1).
use crate::antipattern;
use crate::gitx;
use crate::pjson::{self, JV};
use crate::pyast::{self, CmpOp, Const, Kind, Node};
use crate::pytext::{self, py_repr_list, py_repr_str, py_splitlines, py_strip, take_chars};
use agent_runtime_support::pyvalue::PyValue;
use fancy_regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug)]
pub enum Fail {
    Selftest(String),
    Crash(String),
}

pub type R<T> = Result<T, Fail>;

pub fn require(cond: bool, msg: impl FnOnce() -> String) -> R<()> {
    if cond {
        Ok(())
    } else {
        Err(Fail::Selftest(msg()))
    }
}

pub const CANONICAL_MARKERS: [&str; 3] = ["CLAUDE.md", ".claude/rules/workflow.md", ".claude/policies/registry.yaml"];
pub const ALLOWED_BRANCHES: [&str; 3] = ["single-node", "multi-node", "hint"];
pub const ALLOWED_TAG_PREFIX: &str = "hint/";
/// `_BACKUP_SUFFIXES` / `_BACKUP_PATH_TOKENS`: the working-tree backup predicate (`백업` = this repo's actual Korean naming).
pub const BACKUP_SUFFIXES: [&str; 2] = [".bak", ".orig"];
pub const BACKUP_PATH_TOKENS: [&str; 2] = ["backup", "백업"];
pub const BACKUP_SCAN_PRUNE_TOP: [&str; 4] = [".git", "seed", "output", ".native-e2e"];
pub const RETIRED_HASH_MECHANISMS: [&str; 6] =
    ["tracked_index", "evidence_manifest", "governed_prose_snapshot", "plan_blob_sha1", "image_version_match", "patch_sha256"];
pub const PROSE_SCAN_EXTRA: [&str; 2] = ["CLAUDE.md", "README.md"];
pub const FORBIDDEN_SCANNER_REL: &str = ".claude/skills/wiki-desk/scripts/scan-forbidden-strings.ts";
pub const ROOT_REGISTRY_REL: &str = ".claude/policies/root_registry.json";
pub const REPO_STATE_ASSERTIONS: [&str; 11] = [
    "tripwire①no-backup-artifacts(+refs/heads·tags·.gitignore)",
    "tripwire②no-tracked-digest-rewrite",
    "tripwire③no-retired-hash-mechanism-prose",
    "tripwire④no-duplicate-certificates",
    "tripwire⑤no-pii-in-deployed-artifacts(추적물 전부)",
    "tripwire⑥no-revived-antipatterns",
    "tripwire⑦root-surface-registry",
    "tripwire⑧branch-constitution-layering(4자일치·공통층 어휘)",
    "tripwire⑨base_to_skill_edges(닫힌 목록 · 차단 검사 ②)",
    "tripwire⑩ghost_section_anchors·skill_literal_binding(차단 검사 ①④)",
    "executor-wiring(core.hooksPath·hook tracked)",
];

fn ps(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn resolve(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

// ───────────────────────────────────────────── canonical repository
pub fn canonical_repo_reason(root: &Path) -> Option<String> {
    let missing: Vec<String> = CANONICAL_MARKERS.iter().filter(|m| !root.join(m).is_file()).map(|m| m.to_string()).collect();
    if !missing.is_empty() {
        return Some(format!("missing canonical marker(s) {} under {}", py_repr_list(&missing), ps(root)));
    }
    let out = gitx::git(&ps(root), &["rev-parse", "--show-toplevel"]);
    if out.code != 0 {
        return Some(format!(
            "`git rev-parse --show-toplevel` failed in {} (rc={}): {}",
            ps(root),
            out.code,
            take_chars(py_strip(&out.err_text()), 200)
        ));
    }
    let toplevel = PathBuf::from(py_strip(&out.text()));
    if resolve(&toplevel) != resolve(root) {
        return Some(format!("{} is not a git work-tree root (toplevel={})", ps(root), ps(&toplevel)));
    }
    None
}

pub fn is_canonical(root: &Path) -> bool {
    canonical_repo_reason(root).is_none()
}

pub fn announce_non_canonical(root: &Path, prefix: &str) -> Option<String> {
    let reason = canonical_repo_reason(root)?;
    eprintln!(
        "[{prefix}] SKIPPED (non-canonical repo: {reason}) -- repo-state assertions NOT run: {}",
        REPO_STATE_ASSERTIONS.join(", ")
    );
    Some(reason)
}

fn git_out(root: &Path, args: &[&str]) -> R<String> {
    let out = gitx::git(&ps(root), args);
    if out.code != 0 {
        return Err(Fail::Selftest(format!("git {} failed in {}: {}", args.join(" "), ps(root), py_strip(&out.err_text()))));
    }
    Ok(out.text())
}

pub fn tracked_paths(root: &Path) -> R<Vec<String>> {
    let out = gitx::git(&ps(root), &["ls-files", "-z"]);
    if out.code != 0 {
        return Err(Fail::Selftest(format!("git ls-files -z failed in {}: {}", ps(root), py_strip(&out.err_text()))));
    }
    Ok(gitx::split_z(&out.stdout))
}

pub fn live_registered_worktree_branches(root: &Path) -> R<BTreeSet<String>> {
    let out = gitx::git(&ps(root), &["worktree", "list", "--porcelain", "-z"]);
    if out.code != 0 {
        return Err(Fail::Selftest(format!(
            "git worktree list --porcelain -z failed in {}: {}",
            ps(root),
            py_strip(&out.err_text())
        )));
    }
    let records: Vec<&[u8]> = out.stdout.split(|&b| b == 0).collect();
    let mut live = BTreeSet::new();
    for (start, attr) in records.iter().enumerate() {
        if !attr.starts_with(b"worktree ") {
            continue;
        }
        let mut stanza: Vec<&[u8]> = Vec::new();
        for item in &records[start..] {
            if item.is_empty() {
                break;
            }
            stanza.push(item);
        }
        let location = &attr[b"worktree ".len()..];
        let branch = stanza.iter().find(|i| i.starts_with(b"branch refs/heads/")).map(|i| &i[b"branch refs/heads/".len()..]);
        let prunable = stanza.iter().any(|i| i.starts_with(b"prunable"));
        let Some(branch) = branch else { continue };
        if branch.is_empty() || prunable {
            continue;
        }
        use std::os::unix::ffi::OsStrExt;
        let wt = PathBuf::from(std::ffi::OsStr::from_bytes(location));
        if !wt.is_dir() {
            continue;
        }
        // the worktree path is raw bytes (it may not be UTF-8): pass it to git as an OsStr, never a lossy string
        let check = std::process::Command::new("git").arg("-C").arg(&wt).args(["rev-parse", "--show-toplevel"]).output();
        let Ok(check) = check else { continue };
        let reported = check.stdout.strip_suffix(b"\n").unwrap_or(&check.stdout);
        if check.status.success() && resolve(&PathBuf::from(std::ffi::OsStr::from_bytes(reported))) == resolve(&wt) {
            live.insert(String::from_utf8_lossy(branch).into_owned());
        }
    }
    Ok(live)
}

// ───────────────────────────────────────────── ① backup artifacts
fn walk_backup(root: &Path, dir: &Path, rel_dir: &str, offenders: &mut Vec<String>, errors: &mut Vec<String>) {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            let where_ = if rel_dir.is_empty() { ".".to_string() } else { rel_dir.to_string() };
            errors.push(format!("{}: {}", where_, os_strerror(&e)));
            return;
        }
    };
    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        let p = ent.path();
        if p.is_dir() {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    if rel_dir.is_empty() {
        dirs.retain(|d| !BACKUP_SCAN_PRUNE_TOP.contains(&d.as_str()));
    } else if rel_dir == ".claude" {
        dirs.retain(|d| d != "runtime");
    } else if rel_dir == ".claude/worktrees" {
        dirs.clear();
    }
    for name in &files {
        let rel = if rel_dir.is_empty() { name.clone() } else { format!("{rel_dir}/{name}") };
        let low = rel.to_lowercase();
        if BACKUP_SUFFIXES.iter().any(|s| rel.ends_with(s)) || BACKUP_PATH_TOKENS.iter().any(|t| low.contains(t)) {
            let parts: Vec<&str> = rel.split('/').collect();
            if parts.len() > 2 && parts[0] == "__llm-wiki" && (parts[1] == "cards" || parts[1] == "archive") && BACKUP_SCAN_PRUNE_TOP.contains(&parts[2]) {
                continue;
            }
            offenders.push(rel);
        }
    }
    for d in &dirs {
        let p = dir.join(d);
        if std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            continue;
        }
        let rel = if rel_dir.is_empty() { d.clone() } else { format!("{rel_dir}/{d}") };
        walk_backup(root, &p, &rel, offenders, errors);
    }
}

fn os_strerror(e: &std::io::Error) -> String {
    let t = e.to_string();
    match t.find(" (os error") {
        Some(p) => t[..p].to_string(),
        None => t,
    }
}

pub fn no_backup_artifacts(root: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let mut offenders = Vec::new();
    let mut errors = Vec::new();
    walk_backup(root, root, "", &mut offenders, &mut errors);
    require(offenders.is_empty(), || {
        format!("backup artifact in working tree (백업 금지 · 숨김 금지): {}", py_repr_list(&offenders[..offenders.len().min(20)]))
    })?;
    require(errors.is_empty(), || {
        format!(
            "backup scan could not read part of its own scope (스캔 실패 ≠ 없음): {}",
            py_repr_list(&errors[..errors.len().min(20)])
        )
    })?;
    let branches: BTreeSet<String> = git_out(root, &["for-each-ref", "--format=%(refname:short)", "refs/heads"])?
        .split('\n')
        .filter(|b| !b.is_empty())
        .map(String::from)
        .collect();
    let registered = live_registered_worktree_branches(root)?;
    let stray: Vec<String> =
        branches.iter().filter(|b| !ALLOWED_BRANCHES.contains(&b.as_str()) && !registered.contains(*b)).cloned().collect();
    require(stray.is_empty(), || {
        format!("refs/heads must be allowed durable branches or live registered worktrees: {}", py_repr_list(&stray))
    })?;
    let tags: BTreeSet<String> = git_out(root, &["tag", "-l"])?.split('\n').filter(|t| !t.is_empty()).map(String::from).collect();
    let bad: Vec<String> = tags.iter().filter(|t| !t.starts_with(ALLOWED_TAG_PREFIX)).cloned().collect();
    require(bad.is_empty(), || format!("tags must all be under {}: {}", py_repr_str(ALLOWED_TAG_PREFIX), py_repr_list(&bad)))?;
    let gi = std::fs::read(root.join(".gitignore"))
        .map_err(|e| Fail::Crash(format!("FileNotFoundError: [Errno {}] {}: '{}'", e.raw_os_error().unwrap_or(0), os_strerror(&e), ps(&root.join(".gitignore")))))?;
    let gi = String::from_utf8(gi).map_err(|_| Fail::Crash("UnicodeDecodeError: .gitignore".into()))?;
    let lines: BTreeSet<&str> = py_splitlines(&gi).into_iter().map(py_strip).collect();
    let resurrected: Vec<String> = ["*.bak", "*.orig"].iter().filter(|x| lines.contains(*x)).map(|x| x.to_string()).collect();
    require(resurrected.is_empty(), || {
        format!(".gitignore must not hide backup artifacts (deleted 2026-09-03): {}", py_repr_list(&resurrected))
    })
}

// ───────────────────────────────────────────── ② digests
/// `_HEX_CONST_RE`: a git blob sha1 (40-hex) or sha256 (64-hex) constant, not part of a longer hex run.
pub const HEX_CONST_PATTERN: &str = r"(?<![0-9a-fA-F])(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})(?![0-9a-fA-F])";

pub fn hex_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| pytext::re(HEX_CONST_PATTERN))
}

/// `_HEX_CONST_RE.findall(text)`.
pub fn hex_const_findall(text: &str) -> Vec<String> {
    findall_hex(text)
}

pub fn tracked_blob_digests(root: &Path) -> R<BTreeSet<String>> {
    let out = gitx::git(&ps(root), &["ls-files", "-s", "-z"]);
    if out.code != 0 {
        return Err(Fail::Selftest(format!("git ls-files -s -z failed in {}: {}", ps(root), py_strip(&out.err_text()))));
    }
    let mut sha1s: BTreeSet<String> = BTreeSet::new();
    for entry in gitx::split_z(&out.stdout) {
        let meta: Vec<&str> = entry.splitn(2, '\t').next().unwrap_or("").split_whitespace().collect();
        if meta.len() < 2 || meta[0] == "160000" {
            continue;
        }
        sha1s.insert(meta[1].to_string());
    }
    let mut digests = sha1s.clone();
    if sha1s.is_empty() {
        return Ok(digests);
    }
    let input: String = sha1s.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n") + "\n";
    let proc_ = gitx::git_stdin(&ps(root), &["cat-file", "--batch"], input.as_bytes());
    let buf = proc_.stdout;
    let mut pos = 0;
    let mut resolved = 0;
    let mut unresolved: Vec<String> = Vec::new();
    while pos < buf.len() {
        let Some(nl) = buf[pos..].iter().position(|&b| b == b'\n').map(|x| x + pos) else {
            unresolved.push(format!("<truncated batch output at byte {pos}>"));
            break;
        };
        let header_text = String::from_utf8_lossy(&buf[pos..nl]).into_owned();
        let header: Vec<&str> = header_text.split_whitespace().collect();
        if header.len() < 3 || header[1] != "blob" || !header[2].chars().all(|c| c.is_ascii_digit()) || header[2].is_empty() {
            unresolved.push(if header.is_empty() { "<empty header>".into() } else { header.join(" ") });
            pos = nl + 1;
            continue;
        }
        let size: usize = header[2].parse().unwrap_or(0);
        let end = (nl + 1 + size).min(buf.len());
        let body = &buf[nl + 1..end];
        digests.insert(hex(&Sha256::digest(body)));
        resolved += 1;
        pos = nl + 1 + size + 1;
    }
    require(unresolved.is_empty() && resolved == sha1s.len(), || {
        format!(
            "git cat-file --batch dropped tracked blobs — digest 집합이 불완전하면 tripwire ② 가 조용히 공허통과한다(침묵 폴백 금지): resolved={}/{} unresolved={}",
            resolved,
            sha1s.len(),
            py_repr_list(&unresolved[..unresolved.len().min(10)])
        )
    })?;
    Ok(digests)
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn findall_hex(text: &str) -> Vec<String> {
    pytext::findall(hex_re(), text)
}

fn json_strings(v: &JV, out: &mut Vec<String>) {
    match v {
        JV::Str(s) => out.push(s.clone()),
        JV::Obj(items) => {
            for (k, c) in items {
                out.push(k.clone());
                json_strings(c, out);
            }
        }
        JV::Arr(items) => {
            for c in items {
                json_strings(c, out);
            }
        }
        _ => {}
    }
}

/// `_digest_identity_literals(text, rel)`.
pub fn digest_identity_literals(text: &str, rel: &str) -> BTreeSet<String> {
    let raw: BTreeSet<String> = findall_hex(text).into_iter().collect();
    if !rel.ends_with(".json") || !format!("/{rel}").contains("/rust/checks/data/") {
        return raw;
    }
    let Ok(parsed) = pjson::loads(text) else { return raw };
    let JV::Arr(rows) = &parsed.value else { return raw };
    if parsed.duplicate_keys {
        return raw;
    }
    let full = pytext::re(r"\A(?:(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64}))\z");
    let full = {
        // `_HEX_CONST_RE.fullmatch(...)`
        let _ = &full;
        full
    };
    let mut material: Vec<String> = parsed.numeric_tokens.clone();
    for row in rows {
        let mut row = row.clone();
        if let JV::Obj(_) = &row {
            let args = row.get("arguments");
            let before = row.get("before");
            let expected = row.get("expected");
            let ephemeral = matches!(args, Some(JV::Obj(a)) if a.iter().any(|(_, v)| matches!(v, JV::Str(s) if s == "@ROOT@" || s.starts_with("@ROOT@/"))));
            let readonly = matches!(before, Some(JV::Obj(_)))
                && (matches!(row.get("after"), Some(JV::Str(s)) if s == "same")
                    || row.get("after").map_or(false, |a| py_eq_opt(a, before)));
            let op_ok = matches!(row.get("operation"), Some(JV::Str(s)) if !s.is_empty());
            let exp_ok = match expected {
                Some(JV::Obj(_)) => {
                    let e = expected.unwrap();
                    matches!(e.get("kind"), Some(JV::Str(k)) if k == "value")
                        && matches!(e.get("value"), Some(JV::Str(v)) if pytext::search(&full, v))
                }
                _ => false,
            };
            if op_ok && ephemeral && readonly && exp_ok {
                if let JV::Obj(items) = &mut row {
                    for (k, v) in items.iter_mut() {
                        if k == "expected" {
                            if let JV::Obj(e) = v {
                                e.retain(|(ek, _)| ek != "value");
                            }
                        }
                    }
                }
            }
        }
        json_strings(&row, &mut material);
    }
    material.iter().flat_map(|v| findall_hex(v)).collect()
}

fn py_eq_opt(a: &JV, b: Option<&JV>) -> bool {
    b.map_or(false, |b| pjson::py_eq(a, b))
}

pub fn digest_data_roles() -> R<()> {
    use agent_runtime_support::pystr::PyStr;
    use agent_runtime_support::pyvalue::PyDict;
    let digest = hex(&Sha256::digest(b""));
    let rel = "package/rust/checks/data/observations.json";
    let st = |v: &str| PyValue::Str(PyStr::new(v));
    let obj = |pairs: Vec<(&str, PyValue)>| {
        let mut d = PyDict::new();
        for (k, v) in pairs {
            d.set(k, v);
        }
        PyValue::Dict(d)
    };
    let expected = obj(vec![("kind", st("value")), ("value", st(&digest))]);
    let row_pairs = |exp: PyValue, args: PyValue, after: PyValue, extra: Option<(&str, PyValue)>| {
        let mut v = vec![
            ("id", st("empty")),
            ("operation", st("directory_digest")),
            ("arguments", args),
            ("before", obj(vec![])),
            ("after", after),
            ("expected", exp),
        ];
        if let Some(e) = extra {
            v.push(e);
        }
        obj(v)
    };
    let args0 = obj(vec![("d", st("@ROOT@"))]);
    let row = row_pairs(expected.clone(), args0.clone(), st("same"), None);
    let dumps = |v: &PyValue| v.dumps_ascii().unwrap_or_default();
    let check = |data: &PyValue, path: &str| digest_identity_literals(&dumps(data), path).contains(&digest);
    let list = |v: Vec<PyValue>| PyValue::List(v);
    require(!check(&list(vec![row.clone()]), rel), || "algorithm golden result was classified as tracked-file integrity metadata".into())?;
    let with_sha = row_pairs(expected.clone(), args0.clone(), st("same"), Some(("content_sha256", st(&digest))));
    require(check(&list(vec![with_sha]), rel), || "same-row integrity metadata escaped through an oracle".into())?;
    require(check(&list(vec![row.clone(), obj(vec![("content_sha256", st(&digest))])]), rel), || {
        "neighbouring integrity metadata escaped through an oracle".into()
    })?;
    let exp_extra = obj(vec![("kind", st("value")), ("value", st(&digest)), ("content_sha256", st(&digest))]);
    require(check(&list(vec![row_pairs(exp_extra.clone(), args0.clone(), st("same"), None)]), rel), || {
        "extra expected metadata escaped through an oracle".into()
    })?;
    require(check(&list(vec![row.clone()]), ".claude/policies/integrity.json"), || "ordinary data acquired the Rust fixture role".into())?;
    require(check(&list(vec![row.clone()]), "package/notrust/checks/data/observations.json"), || {
        "partial directory name acquired the Rust fixture role".into()
    })?;
    require(
        check(&list(vec![row_pairs(expected.clone(), obj(vec![("d", st(".claude/tracked.py"))]), st("same"), None)]), rel),
        || "tracked input acquired a generated-root role".into(),
    )?;
    require(
        check(&list(vec![row_pairs(expected.clone(), args0.clone(), obj(vec![("new", obj(vec![("kind", st("file"))]))]), None)]), rel),
        || "writing operation acquired a read-only role".into(),
    )?;
    require(check(&obj(vec![("expected", expected.clone())]), rel), || "ordinary expected metadata escaped".into())?;
    let dq = format!("\"{digest}\"");
    let duplicate = format!("\"content_sha256\":{dq},\"content_sha256\":\"later-value\"");
    require(digest_identity_literals(&format!("[{{\"metadata\":{{{duplicate}}}}}]"), rel).contains(&digest), || {
        "earlier duplicate-key metadata was lost while parsing JSON".into()
    })?;
    let dup_exp = dumps(&list(vec![row_pairs(exp_extra, args0, st("same"), None)]))
        .replace(&format!("\"content_sha256\": {dq}"), &duplicate);
    require(digest_identity_literals(&dup_exp, rel).contains(&digest), || {
        "earlier duplicate-key metadata in expected escaped through an oracle".into()
    })?;
    let numeric = "1".repeat(40);
    require(digest_identity_literals(&format!("[{{\"metadata\":{numeric}}}]"), rel).contains(&numeric), || {
        "numeric literal was lost while parsing JSON".into()
    })
}

pub fn no_tracked_digest_rewrite(root: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let digests = tracked_blob_digests(root)?;
    let mut offenders = Vec::new();
    for rel in tracked_paths(root)? {
        if !(rel.ends_with(".json") || rel.ends_with(".yaml") || rel.ends_with(".yml")) {
            continue;
        }
        let path = root.join(&rel);
        if !path.is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(text) = String::from_utf8(bytes) else { continue };
        let mut hits: Vec<String> =
            findall_hex(&text).into_iter().filter(|m| digests.contains(&m.to_lowercase())).collect::<BTreeSet<_>>().into_iter().collect();
        if !hits.is_empty() {
            hits = digest_identity_literals(&text, &rel).into_iter().filter(|m| digests.contains(&m.to_lowercase())).collect();
        }
        if !hits.is_empty() {
            offenders.push(format!("{}:{} (e.g. {})", rel, hits.len(), hits[0]));
        }
    }
    require(offenders.is_empty(), || {
        format!("tracked data re-states a digest git already owns (single authority = git): {}", py_repr_list(&offenders))
    })
}

// ───────────────────────────────────────────── ③ retired prose
fn fnmatch_star_suffix(name: &str, pattern: &str) -> bool {
    fnmatch(name, pattern)
}

/// `fnmatch.fnmatch` (POSIX: case-sensitive) for `*`, `?`, `[seq]`, `[!seq]`.
pub fn fnmatch(name: &str, pat: &str) -> bool {
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pat.chars().collect();
    fn go(n: &[char], p: &[char]) -> bool {
        if p.is_empty() {
            return n.is_empty();
        }
        match p[0] {
            '*' => (0..=n.len()).any(|k| go(&n[k..], &p[1..])),
            '?' => !n.is_empty() && go(&n[1..], &p[1..]),
            '[' => {
                let mut j = 1;
                let neg = j < p.len() && p[j] == '!';
                if neg {
                    j += 1;
                }
                let start = j;
                if j < p.len() && p[j] == ']' {
                    j += 1;
                }
                while j < p.len() && p[j] != ']' {
                    j += 1;
                }
                if j >= p.len() {
                    // no closing bracket: literal '['
                    return !n.is_empty() && n[0] == '[' && go(&n[1..], &p[1..]);
                }
                if n.is_empty() {
                    return false;
                }
                let set = &p[start..j];
                let mut hit = false;
                let mut k = 0;
                while k < set.len() {
                    if k + 2 < set.len() && set[k + 1] == '-' {
                        if set[k] <= n[0] && n[0] <= set[k + 2] {
                            hit = true;
                        }
                        k += 3;
                    } else {
                        if set[k] == n[0] {
                            hit = true;
                        }
                        k += 1;
                    }
                }
                hit != neg && go(&n[1..], &p[j + 1..])
            }
            c => !n.is_empty() && n[0] == c && go(&n[1..], &p[1..]),
        }
    }
    go(&n, &p)
}

fn drop_git_ignored(root: &Path, paths: Vec<PathBuf>) -> Vec<PathBuf> {
    if paths.is_empty() {
        return paths;
    }
    let mut stdin = Vec::new();
    for p in &paths {
        use std::os::unix::ffi::OsStrExt;
        stdin.extend_from_slice(p.strip_prefix(root).unwrap_or(p).as_os_str().as_bytes());
        stdin.push(0);
    }
    let out = gitx::git_stdin(&ps(root), &["check-ignore", "-z", "--stdin"], &stdin);
    if out.code != 0 && out.code != 1 {
        return paths;
    }
    let ignored: BTreeSet<String> = gitx::split_z(&out.stdout).into_iter().collect();
    paths
        .into_iter()
        .filter(|p| !ignored.contains(&p.strip_prefix(root).unwrap_or(p).to_string_lossy().into_owned()))
        .collect()
}

/// `_walk_governing_files(root, pattern)` (os.walk order, then git-ignored paths dropped).
pub fn walk_governing_files(root: &Path, pattern: &str) -> Vec<PathBuf> {
    let claude = root.join(".claude");
    let mut found = Vec::new();
    fn rec(dir: &Path, top: &Path, pattern: &str, found: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut dirs = Vec::new();
        for ent in rd.flatten() {
            let p = ent.path();
            let name = ent.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                dirs.push((name, p));
            } else if fnmatch_star_suffix(&name, pattern) {
                found.push(p);
            }
        }
        if dir == top {
            dirs.retain(|(n, _)| n != "worktrees" && n != "runtime");
        }
        for (_, p) in dirs {
            if std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
                continue;
            }
            rec(&p, top, pattern, found);
        }
    }
    rec(&claude, &claude, pattern, &mut found);
    drop_git_ignored(root, found)
}

fn path_parts_key(p: &Path) -> Vec<String> {
    p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect()
}

pub fn no_retired_hash_mechanism_prose(root: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let mut targets = walk_governing_files(root, "*.md");
    targets.sort_by_key(|p| path_parts_key(p));
    targets.extend(PROSE_SCAN_EXTRA.iter().map(|n| root.join(n)));
    let mut offenders = Vec::new();
    for path in &targets {
        if !path.is_file() {
            continue;
        }
        let Ok(b) = std::fs::read(path) else { continue };
        let Ok(text) = String::from_utf8(b) else { continue };
        for (i, line) in py_splitlines(&text).iter().enumerate() {
            for name in RETIRED_HASH_MECHANISMS {
                if line.contains(name) {
                    offenders.push(format!("{}:{}:{}", ps(path.strip_prefix(root).unwrap_or(path)), i + 1, name));
                }
            }
        }
    }
    require(offenders.is_empty(), || {
        format!("retired hash mechanism named in governing prose: {}", py_repr_list(&offenders[..offenders.len().min(20)]))
    })
}

// ───────────────────────────────────────────── ④ duplicate certificates
pub type CertKey = Vec<String>;

/// `duplicate_certificate_groups(certs)`; `certs` in insertion order.
pub fn duplicate_certificate_groups(certs: &[(String, String)]) -> (Vec<(CertKey, Vec<String>)>, Vec<String>) {
    let mut groups: Vec<(CertKey, Vec<String>)> = Vec::new();
    let mut unkeyed = Vec::new();
    for (rel, text) in certs {
        let key = crate::certkey::certificate_key(text);
        match key {
            None => unkeyed.push(rel.clone()),
            Some(k) => match groups.iter_mut().find(|(g, _)| *g == k) {
                Some((_, v)) => v.push(rel.clone()),
                None => groups.push((k, vec![rel.clone()])),
            },
        }
    }
    let dups = groups
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(k, mut v)| {
            v.sort();
            (k, v)
        })
        .collect();
    unkeyed.sort();
    (dups, unkeyed)
}

pub fn no_duplicate_certificates(root: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let mut certs: Vec<(String, String)> = Vec::new();
    for rel in tracked_paths(root)? {
        if !(rel.starts_with("docs/benchmark/benchmark_") && rel.ends_with(".yaml")) {
            continue;
        }
        let path = root.join(&rel);
        if !path.is_file() {
            continue;
        }
        match std::fs::read(&path).map(String::from_utf8) {
            Ok(Ok(t)) => certs.push((rel, t)),
            Ok(Err(e)) => return Err(Fail::Selftest(format!("tracked certificate unreadable — fail-closed: {rel}: {e}"))),
            Err(e) => return Err(Fail::Selftest(format!("tracked certificate unreadable — fail-closed: {rel}: {e}"))),
        }
    }
    let (mut dups, unkeyed) = duplicate_certificate_groups(&certs);
    require(unkeyed.is_empty(), || {
        format!(
            "tracked certificate(s) cannot be keyed (flat parse failed or measured_utc/strong key missing) — fail-closed, not 'no match': {}",
            py_repr_list(&unkeyed)
        )
    })?;
    dups.sort_by(|a, b| a.1[0].cmp(&b.1[0]));
    let mut lines = Vec::new();
    for (key, rels) in &dups {
        let digests: BTreeSet<String> =
            rels.iter().map(|r| hex(&Sha256::digest(std::fs::read(root.join(r)).unwrap_or_default()))).collect();
        let kind = if digests.len() == 1 {
            "byte-identical (duplicate layer — delete all but one)"
        } else {
            "DIFFERENT bytes for the same measurement (publication-chain defect)"
        };
        lines.push(format!("{}@{} [{}]: {}", key[0], key[key.len() - 1], kind, py_repr_list(rels)));
    }
    require(dups.is_empty(), || {
        format!(
            "same measurement tracked under more than one certificate file (single authority = one file per (identity, measured_utc)):\n  {}",
            lines.join("\n  ")
        )
    })
}

// ───────────────────────────────────────────── ⑤ deployed PII
pub struct TsScanner {
    pub path: PathBuf,
    pub exempt_mark: String,
    pub max_bytes: i64,
}

impl TsScanner {
    fn node(path: &Path, args: &[&str], stdin: Option<&[u8]>) -> R<gitx::Out> {
        let mut a = vec![path.to_str().unwrap_or("")];
        a.extend_from_slice(args);
        let out = gitx::run("node", &a, None, stdin);
        if out.code == -1 && out.stdout.is_empty() {
            return Err(Fail::Crash(format!("FileNotFoundError: [Errno 2] No such file or directory: 'node' ({})", out.err_text())));
        }
        if out.code != 0 && out.code != 1 {
            let e = out.err_text();
            let e = py_strip(&e);
            let tail: String = {
                let c: Vec<char> = e.chars().collect();
                c[c.len().saturating_sub(300)..].iter().collect()
            };
            return Err(Fail::Selftest(format!("forbidden-string scanner failed (exit {}): {}", out.code, tail)));
        }
        Ok(out)
    }

    pub fn new(path: PathBuf) -> R<TsScanner> {
        let out = Self::node(&path, &["--meta"], None)?;
        let meta = agent_runtime_support::pyjson::loads(&out.text()).map_err(|e| Fail::Crash(format!("json.decoder.JSONDecodeError: {e}")))?;
        let PyValue::Dict(d) = meta.peel() else { return Err(Fail::Crash("TypeError: meta".into())) };
        let exempt_mark = match d.get("exempt_mark").map(|v| v.peel()) {
            Some(PyValue::Str(s)) => s.to_utf8().unwrap_or_default(),
            _ => return Err(Fail::Crash("KeyError: 'exempt_mark'".into())),
        };
        let max_bytes = match d.get("max_bytes").and_then(|v| v.as_plain_int()) {
            Some(n) => n.to_string().parse().unwrap_or(0),
            None => return Err(Fail::Crash("KeyError: 'max_bytes'".into())),
        };
        Ok(TsScanner { path, exempt_mark, max_bytes })
    }

    /// `scan_lines(text, terms)` -> [(line, what, exempt)].
    pub fn scan_lines(&self, text: &str, terms: &[&str]) -> R<Vec<(i64, String, bool)>> {
        let mut args = vec!["--stdin-text"];
        for t in terms {
            args.push("--term");
            args.push(t);
        }
        let out = Self::node(&self.path, &args, Some(text.as_bytes()))?;
        let v = agent_runtime_support::pyjson::loads(&out.text()).map_err(|e| Fail::Crash(format!("JSONDecodeError: {e}")))?;
        let PyValue::List(items) = v.peel() else { return Err(Fail::Crash("TypeError: scan_lines".into())) };
        let mut res = Vec::new();
        for h in items {
            let PyValue::Dict(d) = h.peel() else { continue };
            let line = d.get("line").and_then(|x| x.as_plain_int()).map(|n| n.to_string().parse().unwrap_or(0)).unwrap_or(0);
            let what = match d.get("what").map(|x| x.peel()) {
                Some(PyValue::Str(s)) => s.to_utf8().unwrap_or_default(),
                _ => String::new(),
            };
            let exempt = matches!(d.get("exempt").map(|x| x.peel()), Some(PyValue::Bool(true)));
            res.push((line, what, exempt));
        }
        Ok(res)
    }

    pub fn scan_files(&self, root: &Path, rels: &[String]) -> R<PyValue> {
        let stdin: Vec<String> = rels.iter().map(|r| ps(&root.join(r))).collect();
        let out = Self::node(&self.path, &["--files-from", "-", "--json"], Some(stdin.join("\n").as_bytes()))?;
        agent_runtime_support::pyjson::loads(&out.text()).map_err(|e| Fail::Crash(format!("JSONDecodeError: {e}")))
    }
}

fn list_of<'a>(v: &'a PyValue, key: &str) -> R<&'a Vec<PyValue>> {
    match v.peel() {
        PyValue::Dict(d) => match d.get(key).map(|x| x.peel()) {
            Some(PyValue::List(l)) => Ok(l),
            _ => Err(Fail::Crash(format!("KeyError: '{key}'"))),
        },
        _ => Err(Fail::Crash("TypeError: report".into())),
    }
}

fn str_of(v: &PyValue) -> String {
    match v.peel() {
        PyValue::Str(s) => s.to_utf8().unwrap_or_default(),
        other => other.repr(),
    }
}

pub fn no_pii_in_deployed_artifacts(root: &Path, scanner_repo: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let scanner = TsScanner::new(scanner_repo.join(FORBIDDEN_SCANNER_REL))?;
    let targets = tracked_paths(root)?;
    require(!targets.is_empty(), || format!("tripwire5 scanned nothing under {} -- deployed surface enumeration is empty", ps(root)))?;
    let report = scanner.scan_files(root, &targets)?;
    for p in list_of(&report, "skippedLarge")? {
        eprintln!("[tripwire] WARN 5 skipped (too large) {}", str_of(p));
    }
    for p in list_of(&report, "unreadable")? {
        eprintln!("[tripwire] WARN 5 unreadable {}", str_of(p));
    }
    let mut offenders = Vec::new();
    for h in list_of(&report, "hits")? {
        let PyValue::Dict(d) = h.peel() else { continue };
        let path = d.get("path").map(str_of).unwrap_or_default();
        let rel = Path::new(&path).strip_prefix(root).map(ps).unwrap_or(path.clone());
        let line = d.get("line").map(|x| x.repr()).unwrap_or_default();
        let what = d.get("what").map(str_of).unwrap_or_default();
        offenders.push(format!("{rel}:{line}:{what}"));
    }
    require(offenders.is_empty(), || {
        format!(
            "operator PII in tracked deployed artifacts ({}): {}",
            offenders.len(),
            py_repr_list(&offenders[..offenders.len().min(20)])
        )
    })
}

// ───────────────────────────────────────────── ⑥ revived antipatterns
pub fn no_revived_antipatterns(root: &Path) -> R<()> {
    let (rc, stderr) = antipattern::tripwire_cli(&ps(root));
    if rc != 0 {
        let e = py_strip(&stderr).to_string();
        let c: Vec<char> = e.chars().collect();
        let tail: String = c[c.len().saturating_sub(1200)..].iter().collect();
        return Err(Fail::Selftest(format!("제거한 안티패턴이 되돌아왔다(rc={rc}):\n{tail}")));
    }
    Ok(())
}

// ───────────────────────────────────────────── ⑦ root registry
pub fn load_root_registry(root: &Path) -> R<PyValue> {
    let path = root.join(ROOT_REGISTRY_REL);
    require(path.is_file(), || format!("root registry missing -- tripwire7 has no authority to compare against: {}", ps(&path)))?;
    let bytes = std::fs::read(&path).map_err(|e| Fail::Selftest(format!("root registry unreadable: {e}")))?;
    let text = agent_runtime_support::pyjson::decode_file_text(&bytes).map_err(|e| Fail::Selftest(format!("root registry unreadable: {e}")))?;
    let doc = agent_runtime_support::pyjson::loads_text(&text).map_err(|e| Fail::Selftest(format!("root registry unreadable: {e}")))?;
    let ok = matches!(doc.peel(), PyValue::Dict(d) if matches!(d.get("entries").map(|x| x.peel()), Some(PyValue::List(_))));
    require(ok, || "root registry must carry an `entries` list".into())?;
    Ok(doc)
}

pub fn root_names_of(paths: &[String]) -> BTreeSet<String> {
    paths.iter().filter(|r| !r.is_empty()).map(|r| r.split('/').next().unwrap().to_string()).collect()
}

fn dict_str(d: &agent_runtime_support::pyvalue::PyDict, k: &str) -> Option<String> {
    match d.get(k).map(|v| v.peel()) {
        Some(PyValue::Str(s)) => s.to_utf8(),
        _ => None,
    }
}

pub fn root_surface_registry(root: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let doc = load_root_registry(root)?;
    let PyValue::Dict(d) = doc.peel() else { unreachable!() };
    let PyValue::List(entries) = d.get("entries").unwrap().peel() else { unreachable!() };
    let mut declared: Vec<(String, Option<String>)> = Vec::new();
    for e in entries {
        if let PyValue::Dict(ed) = e.peel() {
            if let Some(n) = dict_str(ed, "name") {
                let g = dict_str(ed, "git");
                match declared.iter_mut().find(|(x, _)| *x == n) {
                    Some(slot) => slot.1 = g,
                    None => declared.push((n, g)),
                }
            }
        }
    }
    let trackable: BTreeSet<String> =
        declared.iter().filter(|(_, g)| matches!(g.as_deref(), Some("tracked") | Some("either"))).map(|(n, _)| n.clone()).collect();
    let must_exist: BTreeSet<String> = declared.iter().filter(|(_, g)| g.as_deref() == Some("tracked")).map(|(n, _)| n.clone()).collect();
    let tracked = tracked_paths(root)?;
    require(!tracked.is_empty(), || format!("tripwire7 saw an empty index under {} -- the comparison would be vacuous", ps(root)))?;
    let index_roots = root_names_of(&tracked);
    let stray: Vec<String> = index_roots.difference(&trackable).cloned().collect();
    require(stray.is_empty(), || {
        format!(
            "tracked root entries absent from {ROOT_REGISTRY_REL} ({}): {} -- an unlisted root artifact is a placement error, not a registry addition (D3)",
            stray.len(),
            py_repr_list(&stray[..stray.len().min(20)])
        )
    })?;
    let vanished: Vec<String> = must_exist.difference(&index_roots).cloned().collect();
    require(vanished.is_empty(), || {
        format!("registry declares these root entries tracked but the index has none: {}", py_repr_list(&vanished))
    })?;
    let mut tombstoned = Vec::new();
    if let Some(PyValue::List(ts)) = d.get("tombstones").map(|x| x.peel()) {
        for t in ts {
            let PyValue::Dict(td) = t.peel() else { continue };
            let Some(name) = dict_str(td, "name") else { continue };
            let retired = td.get("retired_utc").map(|v| str_of(v)).unwrap_or_else(|| "None".into());
            let succ = td.get("successor").map(|v| str_of(v)).unwrap_or_else(|| "None".into());
            for n in index_roots.iter().filter(|n| fnmatch(n, &name)) {
                tombstoned.push(format!("{n} (retired {retired} -> {succ})"));
            }
        }
    }
    require(tombstoned.is_empty(), || {
        format!(
            "retired root locations are tracked again ({}): {}",
            tombstoned.len(),
            py_repr_list(&tombstoned[..tombstoned.len().min(20)])
        )
    })
}

// ───────────────────────────────────────────── ⑧ topology layer parity
pub const TOPOLOGY_ONLY_VOCABULARY: [(&str, &[&str]); 2] = [
    ("multi", &["Ray 워커", "ray-worker", "집단 연산", "빌드킷 배달", "동일 ABI", "슬레이브", "양노드", "쌍노드"]),
    ("single", &["a2a-agent", "A2A 원격 에이전트", "위임 셀", "push-attestation"]),
];
/// `(rel, term, line fragment, reason)` — an exemption is admitted only with its reason (a reasonless one is a silent hole).
pub const TOPOLOGY_VOCABULARY_EXEMPTIONS: [(&str, &str, &str, &str); 1] = [(
    ".claude/rules/workflow.md",
    "양노드",
    "clean 빌드(양노드) → 스모크",
    "policy_registry.sh 의 ARCH_WALL_VARIANT_LADDER 사다리 검사가 이 문구를 verbatim 으로 \
요구한다 — 고치면 정책 검증이 깨진다. 어휘는 남지만 그 문장은 사다리 순서 서술이라 \
양 토폴로지가 함께 읽는다.",
)];
pub const LAYER_EXCLUDE_PATHSPEC: &str = ":(exclude)*.topology.md";

pub fn common_layer_prose(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.join("CLAUDE.md")];
    let rules = root.join(".claude/rules");
    if rules.is_dir() {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&rules)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        let n = p.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
                        fnmatch(&n, "*.md") && !n.ends_with(".topology.md")
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|p| path_parts_key(p));
        out.extend(v);
    }
    out.into_iter().filter(|p| p.is_file()).collect()
}

pub fn vocabulary_offenders(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for path in common_layer_prose(root) {
        let rel = ps(path.strip_prefix(root).unwrap_or(&path));
        let Ok(b) = std::fs::read(&path) else { continue };
        let Ok(text) = String::from_utf8(b) else { continue };
        let lines = py_splitlines(&text);
        for (topo, terms) in TOPOLOGY_ONLY_VOCABULARY {
            for term in terms.iter() {
                for (i, line) in lines.iter().enumerate() {
                    if !line.contains(term) {
                        continue;
                    }
                    if TOPOLOGY_VOCABULARY_EXEMPTIONS.iter().any(|(r, t, f, _)| *r == rel && t == term && line.contains(f)) {
                        continue;
                    }
                    out.push(format!("{}:{} '{}' ({} 전용)", rel, i + 1, term, topo));
                }
            }
        }
    }
    out
}

/// The four-way parity verdict document of the owner library (`topology_parity.evaluate(root)`).
pub fn parity_evaluate(root: &Path) -> R<(String, Vec<String>, Vec<String>, PyValue)> {
    use std::os::unix::ffi::OsStrExt;
    let res = role_contract::topology::evaluate(root.as_os_str().as_bytes(), None)
        .map_err(|c| Fail::Crash(format!("{}: {}", c.class, c.message.to_utf8().unwrap_or_default())))?;
    let PyValue::Dict(d) = res.peel() else { return Err(Fail::Crash("TypeError: parity".into())) };
    let verdict = d.get("verdict").map(str_of).unwrap_or_default();
    let codes = match d.get("reason_codes").map(|x| x.peel()) {
        Some(PyValue::List(l)) => l.iter().map(str_of).collect(),
        _ => vec![],
    };
    let reasons = match d.get("reasons").map(|x| x.peel()) {
        Some(PyValue::List(l)) => l.iter().map(str_of).collect(),
        _ => vec![],
    };
    Ok((verdict, codes, reasons, res.clone()))
}

pub fn topology_layer_parity(root: &Path) -> R<()> {
    let (verdict, codes, reasons, _) = parity_evaluate(root)?;
    require(verdict == "PASS", || format!("topology parity RED ({}): {}", codes.join(", "), reasons.join(" · ")))?;
    let offenders = vocabulary_offenders(root);
    require(offenders.is_empty(), || {
        format!(
            "공통층 산문에 토폴로지 전용 어휘가 있다 ({}건) — 그 문장은 특화층으로 가거나 양 토폴로지를 함께 말하도록 고쳐야 한다: {}",
            offenders.len(),
            py_repr_list(&offenders[..offenders.len().min(12)])
        )
    })?;
    let sync = root.join(".claude/skills/upstream-version-watch/scripts/sync_branches.sh");
    if sync.is_file() {
        let src = std::fs::read(&sync).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
        require(src.contains(LAYER_EXCLUDE_PATHSPEC), || {
            format!(
                "동기화가 특화층을 제외하지 않는다 — {} 리터럴이 sync_branches.sh 에 없다. 이 두 자리가 갈라지면 다음 sync 가 특화 파일을 반대 브랜치로 실어 두 브랜치를 다시 같게 만든다",
                py_repr_str(LAYER_EXCLUDE_PATHSPEC)
            )
        })?;
    }
    Ok(())
}

// ───────────────────────────────────────────── executor wiring
pub fn tripwire_executor_wiring(root: &Path) -> Vec<String> {
    let mut w = Vec::new();
    if !is_canonical(root) {
        return w;
    }
    let out = gitx::git(&ps(root), &["config", "--get", "core.hooksPath"]);
    let configured = py_strip(&out.text()).to_string();
    if configured != ".claude/hooks" {
        w.push(format!(
            "core.hooksPath is {}, expected '.claude/hooks' — run: git config core.hooksPath .claude/hooks",
            py_repr_str(&configured)
        ));
    }
    if !root.join(".claude/hooks/pre-commit").is_file() {
        w.push(".claude/hooks/pre-commit is missing".into());
    } else {
        let t = gitx::git(&ps(root), &["ls-files", "--error-unmatch", ".claude/hooks/pre-commit"]);
        if t.code != 0 {
            w.push(".claude/hooks/pre-commit exists but is NOT tracked (untracked hooks do not reach a clone)".into());
        }
    }
    w
}

// ───────────────────────────────────────────── ⑨ base → skill edges
pub const BASE_SKILL_EDGES_REL: &str = ".claude/policies/base_skill_edges.json";

fn skill_rx() -> &'static (Regex, Regex, Regex) {
    static R: OnceLock<(Regex, Regex, Regex)> = OnceLock::new();
    R.get_or_init(|| {
        (
            pytext::re(r"(?:(?<=\.claude/)|(?<![\w./]))skills/([\w*\-{}]+)((?:/[\w.*\-{}]+)*)"),
            pytext::re(r#"["']skills["']\s*([,/])\s*["']([\w\-]+)["']((?:\s*[,/]\s*["'][\w.\-]+["'])*)"#),
            pytext::re(r#"["']([\w.\-]+)["']"#),
        )
    })
}

pub fn skill_edge_targets(line: &str) -> BTreeSet<String> {
    let (slash, join, q) = skill_rx();
    let mut out = BTreeSet::new();
    for c in slash.captures_iter(line).flatten() {
        let t = format!("{}{}", c.get(1).map_or("", |m| m.as_str()), c.get(2).map_or("", |m| m.as_str()));
        out.insert(t.trim_end_matches(|ch| ch == '.' || ch == '/').to_string());
    }
    for c in join.captures_iter(line).flatten() {
        let mut parts = vec![c.get(2).map_or("", |m| m.as_str()).to_string()];
        parts.extend(pytext::findall(q, c.get(3).map_or("", |m| m.as_str())));
        out.insert(parts.join("/"));
    }
    out
}

/// The base-layer files that can carry a base→skill edge (plan_26111615 D1: the policies bundle is Rust).
/// `.claude/policies/**`: the launchers (`.sh`) and the data/prose planes (`.json`/`.yaml`/`.yml`/`.md`);
/// `.claude/rust/policy-*/src/**`: the Rust sources that now implement the policies runtime/predicates
/// (`.rs`, and the `.txt` help/doc texts they `include_str!`). Harness `.py` files are NOT callers any more:
/// they are reference originals pending physical deletion (P6), so the ledger is the same with or without them.
/// Golden/check data (`.claude/rust/policy-*/checks/**`, captured outputs) is outside the plane.
pub fn is_base_edge_source(rel: &str) -> bool {
    if rel.starts_with(".claude/policies/") {
        return [".sh", ".json", ".yaml", ".yml", ".md"].iter().any(|s| rel.ends_with(s));
    }
    if let Some(rest) = rel.strip_prefix(".claude/rust/policy-") {
        return rest.split_once('/').map_or(false, |(_, tail)| tail.starts_with("src/")) && [".rs", ".txt"].iter().any(|s| rel.ends_with(s));
    }
    false
}

pub fn scan_base_skill_edges(root: &Path) -> BTreeSet<(String, String)> {
    let out = gitx::git(&ps(root), &["ls-files", "-z", "--", ".claude/policies", ".claude/rust"]);
    let mut listed = gitx::split_z(&out.stdout);
    listed.sort();
    let mut found = BTreeSet::new();
    for rel in listed {
        if rel == BASE_SKILL_EDGES_REL || rel == ".claude/policies/branch_layer_ledger.json" {
            continue;
        }
        if !is_base_edge_source(&rel) {
            continue;
        }
        let path = root.join(&rel);
        if !path.is_file() {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default()).into_owned();
        for line in py_splitlines(&text) {
            if line.contains("skills") {
                for t in skill_edge_targets(line) {
                    found.insert((rel.clone(), t));
                }
            }
        }
    }
    found
}

fn repr_opt(v: &Option<String>) -> String {
    v.as_ref().map_or("None".into(), |s| py_repr_str(s))
}

pub fn base_skill_edge_violations(root: &Path) -> R<Vec<String>> {
    let lp = root.join(BASE_SKILL_EDGES_REL);
    if !lp.is_file() {
        return Ok(vec![format!("{BASE_SKILL_EDGES_REL} 부재 — 기초층→스킬 간선의 닫힌 목록이 없다")]);
    }
    let text = String::from_utf8(std::fs::read(&lp).unwrap_or_default()).map_err(|_| Fail::Crash("UnicodeDecodeError".into()))?;
    let data = agent_runtime_support::pyjson::loads(&text).map_err(|e| Fail::Crash(format!("json.decoder.JSONDecodeError: {e}")))?;
    let mut listed: BTreeSet<(Option<String>, Option<String>)> = BTreeSet::new();
    let mut problems = Vec::new();
    let edges = match data.peel() {
        PyValue::Dict(d) => match d.get("edges").map(|x| x.peel()) {
            Some(PyValue::List(l)) => l.clone(),
            None => vec![],
            _ => return Err(Fail::Crash("TypeError: edges".into())),
        },
        _ => return Err(Fail::Crash("AttributeError: 'list' object has no attribute 'get'".into())),
    };
    for row in &edges {
        let PyValue::Dict(rd) = row.peel() else { return Err(Fail::Crash("AttributeError: row".into())) };
        let key = (dict_str(rd, "caller"), dict_str(rd, "callee"));
        let phase = dict_str(rd, "phase");
        let reason_ok = match rd.get("reason").map(|v| v.peel()) {
            None | Some(PyValue::None) => false,
            Some(v) => {
                let t = if let PyValue::Str(s) = v { s.to_utf8().unwrap_or_default() } else { v.repr() };
                !py_strip(&t).is_empty() && v.is_truthy()
            }
        };
        if !matches!(phase.as_deref(), Some("runtime_operational") | Some("verification_plane")) || !reason_ok {
            problems.push(format!("원장 행에 phase/reason 이 없다: ({}, {})", repr_opt(&key.0), repr_opt(&key.1)));
        }
        listed.insert(key);
    }
    let found: BTreeSet<(Option<String>, Option<String>)> =
        scan_base_skill_edges(root).into_iter().map(|(a, b)| (Some(a), Some(b))).collect();
    for (c, e) in found.difference(&listed).take(12) {
        problems.push(format!(
            "미등재 간선 {} → skill:{} (이유를 달아 {BASE_SKILL_EDGES_REL} 에 등재하거나 간선을 없앤다)",
            c.clone().unwrap_or_default(),
            e.clone().unwrap_or_default()
        ));
    }
    for (c, e) in listed.difference(&found).take(12) {
        problems.push(format!(
            "사라진 간선이 원장에 남았다 {} → skill:{} (원장 행을 지운다)",
            c.clone().unwrap_or_else(|| "None".into()),
            e.clone().unwrap_or_else(|| "None".into())
        ));
    }
    Ok(problems)
}

pub fn base_skill_edges(root: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let p = base_skill_edge_violations(root)?;
    require(p.is_empty(), || format!("tripwire⑨ base_to_skill_edges: {}", p.join(" | ")))
}

// ───────────────────────────────────────────── ⑩ router bindings
pub const TN_SKILL_REL: &str = ".claude/skills/terraforming_node/SKILL.md";
pub const TN_DIR_REL: &str = ".claude/skills/terraforming_node/";

fn tn_rx() -> &'static (Regex, Regex, Regex) {
    static R: OnceLock<(Regex, Regex, Regex)> = OnceLock::new();
    R.get_or_init(|| {
        (
            pytext::re(r"terraforming_node(?:/SKILL\.md)?`?\s*(?:SKILL\.md)?`?\s*\**\s*§\s*([0-9]+S?(?:\.[0-9]+)*[a-z]?)"),
            pytext::re(r"terraforming_node/references/([\w./\-]+?\.md)"),
            pytext::re(r"(?<![\w/.])references/([\w./\-]+?\.md)"),
        )
    })
}

fn re_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' {
            o.push(c);
        } else {
            o.push('\\');
            o.push(c);
        }
    }
    o
}

pub fn anchor_token_present(anchor: &str, text: &str) -> bool {
    let r = pytext::re(&format!(r"(?<![\d.]){}(?!\.?\d)", re_escape(anchor)));
    pytext::search(&r, text)
}

pub fn router_binding_violations(root: &Path) -> R<Vec<String>> {
    let skill_path = root.join(TN_SKILL_REL);
    if !skill_path.is_file() {
        return Ok(vec![format!("{TN_SKILL_REL} 부재")]);
    }
    let skill = String::from_utf8(std::fs::read(&skill_path).unwrap_or_default()).map_err(|_| Fail::Crash("UnicodeDecodeError: SKILL.md".into()))?;
    let grep = gitx::git(&ps(root), &["grep", "-lz", "-I", "-e", "terraforming_node", "--", ".", ":!docs", ":!seed"]);
    let sk = gitx::git(&ps(root), &["ls-files", "-z", "--", TN_DIR_REL]);
    let mut rels: BTreeSet<String> = gitx::split_z(&grep.stdout).into_iter().collect();
    rels.extend(gitx::split_z(&sk.stdout));
    let (anchor_re, abs_re, rel_re) = tn_rx();
    let mut problems: Vec<String> = Vec::new();
    for rel in &rels {
        if rel.starts_with("docs/") || rel.starts_with("seed/") || rel == ".claude/policies/branch_layer_ledger.json" {
            continue;
        }
        let Ok(b) = std::fs::read(root.join(rel)) else { continue };
        let Ok(text) = String::from_utf8(b) else { continue };
        let in_skill = rel.starts_with(TN_DIR_REL);
        if !text.contains("terraforming_node") && !(in_skill && text.contains("references/")) {
            continue;
        }
        for line in py_splitlines(&text) {
            for a in pytext::findall(anchor_re, line) {
                if !anchor_token_present(&a, &skill) {
                    problems.push(format!("① dangling §앵커 {rel}: SKILL.md §{a}"));
                }
            }
            let mut refs = pytext::findall(abs_re, line);
            if in_skill && rel.ends_with(".md") {
                refs.extend(pytext::findall(rel_re, line));
            }
            for r in refs {
                if !root.join(TN_DIR_REL).join("references").join(&r).is_file() {
                    problems.push(format!("① dangling references 경로 {rel}: references/{r}"));
                }
            }
        }
    }
    problems.extend(skill_literal_binding_violations(root, &skill)?);
    let set: BTreeSet<String> = problems.into_iter().collect();
    Ok(set.into_iter().collect())
}

fn text_span(lines: &[&str], n: &Node) -> String {
    let a = n.line.saturating_sub(1).min(lines.len());
    let b = n.end_line.min(lines.len()).max(a);
    lines[a..b].join("\n")
}

fn walk_all(n: &Node) -> Vec<&Node> {
    pyast::walk(n)
}

pub fn skill_literal_binding_violations(root: &Path, skill: &str) -> R<Vec<String>> {
    let out = gitx::git(&ps(root), &["ls-files", "-z", "--", ".claude/policies"]);
    let mut rels: Vec<String> = gitx::split_z(&out.stdout).into_iter().filter(|x| x.ends_with(".py")).collect();
    rels.sort();
    let lit = "terraforming_node/SKILL.md";
    let mut res = Vec::new();
    for rel in rels {
        // A listed `.py` that is not on disk (a reference original removed ahead of its index entry, plan_26111615 P6)
        // binds nothing -- the same rule tripwire ⑨ applies to an absent source.
        if !root.join(&rel).is_file() {
            continue;
        }
        let bytes = std::fs::read(root.join(&rel)).map_err(|e| Fail::Crash(format!("OSError: {e}: {rel}")))?;
        let src = String::from_utf8(bytes).map_err(|_| Fail::Crash(format!("UnicodeDecodeError: {rel}")))?;
        if !src.contains(lit) {
            continue;
        }
        let tree = pyast::parse(&src).map_err(|e| Fail::Crash(format!("SyntaxError: {e} ({rel})")))?;
        let lines = py_splitlines(&src);
        for f in walk_all(&tree) {
            if !matches!(f.kind, Kind::FunctionDef | Kind::AsyncFunctionDef) {
                continue;
            }
            if !text_span(&lines, f).contains(lit) {
                continue;
            }
            let mut names: Vec<String> = Vec::new();
            for node in walk_all(f) {
                if node.kind == Kind::Assign && node.count == 1 && node.children[0].kind == Kind::Name {
                    let seg = text_span(&lines, &node.children[1]);
                    let hit = seg.contains(lit)
                        || names.iter().any(|v| pytext::search(&pytext::re(&format!(r"\b{}\.read_text\b", re_escape(v))), &seg));
                    if hit && !names.contains(&node.children[0].name) {
                        names.push(node.children[0].name.clone());
                    }
                }
            }
            if names.is_empty() {
                continue;
            }
            for node in walk_all(f) {
                if node.kind == Kind::Compare
                    && node.ops.len() == 1
                    && node.children[0].kind == Kind::Constant
                    && node.children[1].kind == Kind::Name
                    && names.contains(&node.children[1].name)
                {
                    if let Some(Const::Str(l)) = &node.children[0].constant {
                        if node.ops[0] == CmpOp::In && !skill.contains(l.as_str()) {
                            res.push(format!("④ 결속 리터럴 부재 {}:{}: {}", rel, node.line, py_repr_str(l)));
                        }
                        if node.ops[0] == CmpOp::NotIn && skill.contains(l.as_str()) {
                            res.push(format!("④ 금지 리터럴 존재 {}:{}: {}", rel, node.line, py_repr_str(l)));
                        }
                    }
                }
            }
        }
    }
    res.extend(rust_literal_binding_violations(root, skill, lit)?);
    Ok(res)
}

/// ⑩④ over the Rust policy sources (plan_26111615: the predicates/runtime that read the terraforming_node SKILL.md are
/// Rust now). Same rule as the `.py` scan: inside one top-level `fn`, a variable bound by a `let` statement that names
/// the SKILL.md (directly, or by reading a variable already bound to it) is a binding; `v.contains("L")` requires `L` in
/// the SKILL.md, `!v.contains("L")` forbids it. Item = the column-0 `fn` head up to its column-0 `}` (rustfmt layout).
pub fn rust_literal_binding_violations(root: &Path, skill: &str, lit: &str) -> R<Vec<String>> {
    let out = gitx::git(&ps(root), &["ls-files", "-z", "--", ".claude/rust"]);
    let mut rels: Vec<String> = gitx::split_z(&out.stdout)
        .into_iter()
        .filter(|x| x.ends_with(".rs") && is_base_edge_source(x))
        .collect();
    rels.sort();
    let let_re = pytext::re(r"let\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?::[^=;]+)?=([^;]*);");
    let mut res = Vec::new();
    for rel in rels {
        let bytes = std::fs::read(root.join(&rel)).map_err(|e| Fail::Crash(format!("OSError: {e}: {rel}")))?;
        let src = String::from_utf8(bytes).map_err(|_| Fail::Crash(format!("UnicodeDecodeError: {rel}")))?;
        if !src.contains(lit) {
            continue;
        }
        let lines: Vec<&str> = src.split('\n').collect();
        let mut i = 0;
        while i < lines.len() {
            let head = lines[i];
            let is_fn = ["pub fn ", "fn ", "pub(crate) fn "].iter().any(|p| head.starts_with(p));
            if !is_fn {
                i += 1;
                continue;
            }
            let end = (i..lines.len()).find(|&j| lines[j] == "}" || (j == i && head.trim_end().ends_with('}'))).unwrap_or(lines.len() - 1);
            let body = lines[i..=end].join("\n");
            if body.contains(lit) {
                let mut names: Vec<String> = Vec::new();
                for c in let_re.captures_iter(&body).flatten() {
                    let (name, rhs) = (c.get(1).map_or("", |m| m.as_str()), c.get(2).map_or("", |m| m.as_str()));
                    let hit = rhs.contains(lit) || names.iter().any(|v| pytext::search(&pytext::re(&format!(r"\b{}\b", re_escape(v))), rhs));
                    if hit && !names.iter().any(|n| n == name) {
                        names.push(name.to_string());
                    }
                }
                for (k, line) in lines[i..=end].iter().enumerate() {
                    for v in &names {
                        let call = pytext::re(&format!(r#"(!?)\b{}\.contains\("((?:[^"\\]|\\.)*)"\)"#, re_escape(v)));
                        for c in call.captures_iter(line).flatten() {
                            let neg = c.get(1).map_or(false, |m| m.as_str() == "!");
                            let l = rust_unescape(c.get(2).map_or("", |m| m.as_str()));
                            if !neg && !skill.contains(l.as_str()) {
                                res.push(format!("④ 결속 리터럴 부재 {}:{}: {}", rel, i + k + 1, py_repr_str(&l)));
                            }
                            if neg && skill.contains(l.as_str()) {
                                res.push(format!("④ 금지 리터럴 존재 {}:{}: {}", rel, i + k + 1, py_repr_str(&l)));
                            }
                        }
                    }
                }
            }
            i = end + 1;
        }
    }
    Ok(res)
}

/// The value of a Rust `"…"` literal body (the escapes the policy sources use: `\"`, `\\`, `\n`, `\t`).
fn rust_unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

pub fn router_bindings(root: &Path) -> R<()> {
    if !is_canonical(root) {
        return Ok(());
    }
    let p = router_binding_violations(root)?;
    require(p.is_empty(), || {
        format!("tripwire⑩ ghost_section_anchors/skill_literal_binding: {}", p[..p.len().min(12)].join(" | "))
    })
}

// ───────────────────────────────────────────── entry
fn print_crash(text: &str) {
    eprintln!("Traceback (most recent call last):\n  File \"runtime_selftest.py\", in run_tripwires\n{text}");
}

/// `run_tripwires(root)`: the pre-commit entry. `repo` is where the scanner programs live (the original's
/// `CLAUDE_DIR.parent`).
pub fn run_tripwires(root: &Path, repo: &Path) -> i32 {
    if announce_non_canonical(root, "tripwire").is_some() {
        return 0;
    }
    let steps: Vec<Box<dyn Fn() -> R<()>>> = vec![
        Box::new(|| no_backup_artifacts(root)),
        Box::new(digest_data_roles),
        Box::new(|| no_tracked_digest_rewrite(root)),
        Box::new(|| no_retired_hash_mechanism_prose(root)),
        Box::new(|| no_duplicate_certificates(root)),
        Box::new(|| no_pii_in_deployed_artifacts(root, repo)),
        Box::new(|| no_revived_antipatterns(root)),
        Box::new(|| root_surface_registry(root)),
        Box::new(|| topology_layer_parity(root)),
        Box::new(|| base_skill_edges(root)),
        Box::new(|| router_bindings(root)),
    ];
    for step in steps {
        match step() {
            Ok(()) => {}
            Err(Fail::Selftest(m)) => {
                eprintln!("[tripwire] FAIL {m}");
                return 1;
            }
            Err(Fail::Crash(t)) => {
                print_crash(&t);
                return 1;
            }
        }
    }
    for w in tripwire_executor_wiring(root) {
        eprintln!("[tripwire] WARN {w}");
    }
    eprintln!("[tripwire] PASS");
    0
}

#[allow(dead_code)]
fn _btree_used(_: BTreeMap<String, String>) {}
