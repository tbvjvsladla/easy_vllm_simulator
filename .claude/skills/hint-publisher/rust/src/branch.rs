//! hintlib.branch — hint 브랜치 배관 커밋. Port of `scripts/hintlib/branch.py` (whole file incl. `selftest()` and the
//! fixture helpers tag's selftest reuses).
//!
//! Pure plumbing (hash-object → mktree → commit-tree → update-ref) with the synthetic identity and the injected date:
//! the same payload, message, time and guide give the same commit SHA as the original byte for byte.
//!
//! Signature notes (deviations from the generated stubs — none of them had a caller outside this file):
//! - bytes are `Vec<u8>` (`read_blob_bytes`, `read_blobs`, `_transition_template_bytes`, `_transition_tree`, `scan_blobs`);
//!   `read_blob` keeps the stub form `HR<Option<J>>` with `J = {"__bytes__": hex}` (the bridge encoding).
//! - `payload_doc_problems`/`_meta_problems` take the PAYLOAD.json document as `&J` (the stub's `&Path` was a generator
//!   slip); `docs` is a `J` dict name → `Str` (bytes are decoded with `errors="replace"`, as the original does).
//! - `identity_env` returns the J dict; `identity_env_pairs` the env list `core::git_full` takes.
#![allow(non_snake_case, non_upper_case_globals, dead_code)]
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::pii;
use crate::value::{Map, J};
use crate::{jd, naming};

// ── 상수 ─────────────────────────────────────────────────────────────────────────────────────
pub const HINT_BRANCH_REF: &str = core::HINT_BRANCH_REF;
pub const PAYLOAD_JSON: &str = "PAYLOAD.json";
pub const PROVENANCE_JSON: &str = "PROVENANCE.json";
pub const ARTIFACTS_DIR: &str = "artifacts";
pub const PAYLOAD_ALLOWLIST_TOP: [&str; 8] = [
    "README.md",
    "00-hint.md",
    "01-artifacts.md",
    "02-narrative.md",
    "03-benchmark.md",
    PAYLOAD_JSON,
    "LINEAGE.json",
    PROVENANCE_JSON,
];
pub const PAYLOAD_REQUIRED_TOP: [&str; 8] = PAYLOAD_ALLOWLIST_TOP;
pub const LITE_OMITTED_TOP: [&str; 1] = ["03-benchmark.md"];

/// 이 페이로드의 필수 최상위 파일. PAYLOAD.json `cell_grade == "lite"` 일 때만 03 을 뺀다(모름 = full 필수).
pub fn required_top(payload_doc: &J) -> HR<Vec<String>> {
    let lite = payload_doc.is_dict() && payload_doc.get("cell_grade") == Some(&J::from("lite"));
    Ok(PAYLOAD_REQUIRED_TOP.iter().filter(|t| !(lite && LITE_OMITTED_TOP.contains(t))).map(|t| t.to_string()).collect())
}

pub const PAYLOAD_SLOT_DIRS: [&str; 7] = ["triplet", "runtime_patch", "build_patch_pre", "build_patch_post", "build_recipe", "compose", "fork_pin"];
pub const FORBIDDEN_SEGMENTS: [&str; 5] = [".git", ".claude", "__pycache__", ".gitattributes", ".gitmodules"];
pub const PAYLOAD_FORMAT: &str = "hint-payload/v7";
pub const PAYLOAD_DOCS: [&str; 4] = ["00-hint.md", "01-artifacts.md", "02-narrative.md", "03-benchmark.md"];
crate::re_static!(PROMPT_RESIDUE_RE, r"<!--\s*PROMPT\b");
crate::re_static!(_FULL_SHA_RE, r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$");
crate::re_static!(_IDENT_LINE_RE, r"^(author|committer|tagger) (.*) (\d+) ([+-]\d{4})$");
crate::re_static!(_CONTROL_RE, r"[\x00-\x1f\x7f]");
crate::re_static!(_FMT_SHAPE_RE, r"v[1-9][0-9]*");

// ── 작은 Python 동형 도우미 ───────────────────────────────────────────────────────────────────
fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_space)
}
fn py_lstrip(s: &str) -> &str {
    s.trim_start_matches(py_space)
}
/// `str.splitlines()`.
fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let br = matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if br {
            out.push(&s[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(j, '\n')) = it.peek() {
                    it.next();
                    end = j + 1;
                }
            }
            start = end;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}
fn strs_repr(v: &[String]) -> String {
    J::from(v.to_vec()).repr()
}
fn opt_repr(v: Option<&str>) -> String {
    match v {
        Some(s) => crate::value::py_repr(s),
        None => "None".into(),
    }
}
fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}
/// `{"__bytes__": hex}` (the bridge's bytes encoding).
pub fn bytes_j(b: &[u8]) -> J {
    jd! {"__bytes__" => hexs(b)}
}
/// Bytes of a `{"__bytes__": hex}` J (or the UTF-8 of a `Str`).
pub fn j_bytes(v: &J) -> Option<Vec<u8>> {
    match v {
        J::Str(s) => Some(s.as_bytes().to_vec()),
        J::Dict(m) => m.get("__bytes__").and_then(J::as_str).and_then(unhex),
        _ => None,
    }
}
fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
fn pyrepr_bytes(b: &[u8]) -> String {
    let q = if b.contains(&b'\'') && !b.contains(&b'"') { '"' } else { '\'' };
    let mut s = String::from("b");
    s.push(q);
    for &c in b {
        match c {
            b'\\' => s.push_str("\\\\"),
            b'\t' => s.push_str("\\t"),
            b'\n' => s.push_str("\\n"),
            b'\r' => s.push_str("\\r"),
            c if c as char == q => {
                s.push('\\');
                s.push(q)
            }
            32..=126 => s.push(c as char),
            c => s.push_str(&format!("\\x{c:02x}")),
        }
    }
    s.push(q);
    s
}
/// `repr(UnicodeDecodeError)` of `data.decode("utf-8")`.
fn utf8_error_repr(data: &[u8], e: &std::str::Utf8Error) -> String {
    let start = e.valid_up_to();
    let (end, reason) = match e.error_len() {
        None => (data.len(), "unexpected end of data"),
        Some(n) => {
            let lead = data[start];
            let r = if (0xC2..=0xF4).contains(&lead) { "invalid continuation byte" } else { "invalid start byte" };
            (start + n, r)
        }
    };
    format!("UnicodeDecodeError('utf-8', {}, {start}, {end}, '{reason}')", pyrepr_bytes(data))
}
/// `json.loads(data.decode("utf-8"))` → value, or the `repr` of the exception.
fn json_of_bytes(data: &[u8]) -> Result<J, String> {
    let text = std::str::from_utf8(data).map_err(|e| utf8_error_repr(data, &e))?;
    crate::value::loads(text).map_err(|m| format!("JSONDecodeError({})", crate::value::py_repr(&m)))
}

fn _log(msg: &str) -> HR<()> {
    crate::io::errln(&format!("[hint] {msg}"));
    Ok(())
}

// ── 신원 ─────────────────────────────────────────────────────────────────────────────────────
/// 배포 오브젝트(페이로드 커밋 author/committer · 태그 tagger)에 박히는 유일한 신원 `이름 <메일>`.
pub fn synthetic_identity() -> HR<String> {
    Ok(format!("{} <{}>", core::SYNTHETIC_NAME, core::synthetic_email()))
}

/// git 이 신원·시각을 ambient 설정이 아니라 이 값에서 읽게 하는 env (dict).
pub fn identity_env(generated_utc: &str) -> HR<J> {
    let mut m = Map::new();
    for (k, v) in identity_env_pairs(generated_utc)? {
        m.insert(k, v);
    }
    Ok(J::Dict(m))
}

/// [`identity_env`] as the env list `core::git_full` takes (same order).
pub fn identity_env_pairs(generated_utc: &str) -> HR<Vec<(String, String)>> {
    let d = core::git_date(generated_utc)?;
    let (n, e) = (core::SYNTHETIC_NAME.to_string(), core::synthetic_email());
    Ok(vec![
        ("GIT_AUTHOR_NAME".into(), n.clone()),
        ("GIT_AUTHOR_EMAIL".into(), e.clone()),
        ("GIT_AUTHOR_DATE".into(), d.clone()),
        ("GIT_COMMITTER_NAME".into(), n),
        ("GIT_COMMITTER_EMAIL".into(), e),
        ("GIT_COMMITTER_DATE".into(), d),
    ])
}

/// 커밋·태그 헤더의 author/committer/tagger 줄 → {role: {"ident", "epoch", "tz"}}.
pub fn parse_ident_headers(raw: &str) -> HR<J> {
    let mut out = Map::new();
    let head = raw.split("\n\n").next().unwrap_or("");
    for line in splitlines(head) {
        if let Some(m) = _IDENT_LINE_RE().match_at_start(line) {
            let role = m.get(1).unwrap().as_str();
            if out.contains_key(role) {
                continue;
            }
            let ds = m.get(3).unwrap().as_str();
            let epoch: i64 = ds.parse().unwrap_or_else(|_| ds.chars().fold(0i64, |a, c| a * 10 + c.to_digit(10).unwrap_or(0) as i64));
            out.insert(role, jd! {"ident" => m.get(2).unwrap().as_str(), "epoch" => epoch, "tz" => m.get(4).unwrap().as_str()});
        }
    }
    Ok(J::Dict(out))
}

/// `git cat-file <kind> <rev>` 원문(UTF-8 · replace). 없으면 None.
pub fn read_object_text(repo: &Path, kind: &str, rev: &str) -> HR<Option<String>> {
    Ok(core::git_bytes(repo, &["cat-file", kind, rev], false)?.map(|d| lossy(&d)))
}

pub fn commit_identity(repo: &Path, commit: &str) -> HR<J> {
    match read_object_text(repo, "commit", commit)? {
        None => core::fail("HINT_COMMIT_UNREADABLE", format!("커밋 오브젝트를 읽을 수 없다: {commit}"), None),
        Some(raw) => parse_ident_headers(&raw),
    }
}

fn ident_of(ids: &J, role: &str) -> Option<String> {
    ids.get(role).and_then(|r| r.get_str("ident")).map(str::to_string)
}
fn epoch_of(ids: &J, role: &str) -> Option<i64> {
    ids.get(role).and_then(|r| r.get("epoch")).and_then(J::as_i64)
}

// ── PII ──────────────────────────────────────────────────────────────────────────────────────
pub fn _fmt_hit(where_: &str, h: &pii::Hit) -> HR<String> {
    Ok(format!("{where_}:{}: {}:{}", h.line, h.pattern, core::head_chars(&h.r#match, 48)))
}
fn fmt_hit(where_: &str, h: &pii::Hit) -> String {
    _fmt_hit(where_, h).unwrap_or_default()
}

/// (상대경로, bytes) 열 → 배포 프로필(4종+리터럴) 적중 목록(git blob 용). 바이너리(UTF-8 해독 불가)는 건너뛴다.
pub fn scan_blobs(files: &[(String, Vec<u8>)], terms: &[String], mut exempt_log: Option<&mut Vec<J>>) -> HR<Vec<String>> {
    let mut hits = Vec::new();
    for (rel, data) in files {
        let Ok(text) = std::str::from_utf8(data) else { continue };
        for h in pii::scan_text(text, Some(terms), "deploy", &[], true, true, exempt_log.as_deref_mut())? {
            hits.push(fmt_hit(rel, &h));
        }
    }
    Ok(hits)
}

/// 신원 문자열의 PII(identity 프로필). ident=None = 합성 신원.
pub fn identity_pii_hits(terms: &[String], ident: Option<&str>) -> HR<Vec<String>> {
    let who = match ident {
        Some(s) => s.to_string(),
        None => synthetic_identity()?,
    };
    Ok(pii::scan_text(&who, Some(terms), "identity", &[], false, true, None)?.iter().map(|h| fmt_hit("identity", h)).collect())
}

// ── allowlist ────────────────────────────────────────────────────────────────────────────────
fn allow_why(rel: &str, utf8_ok: bool) -> Option<String> {
    if rel.is_empty() {
        return Some("빈 경로".into());
    }
    if rel.starts_with('/') || rel.contains('\\') {
        return Some("절대경로·역슬래시".into());
    }
    if _CONTROL_RE().is_search(rel) {
        return Some("제어문자 포함".into());
    }
    if !utf8_ok {
        return Some("UTF-8 로 표현할 수 없는 이름".into());
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.iter().any(|p| matches!(*p, "" | "." | "..")) {
        return Some("빈 세그먼트·상위탈출".into());
    }
    let bad: BTreeSet<&str> = parts.iter().copied().filter(|p| FORBIDDEN_SEGMENTS.contains(p)).collect();
    if !bad.is_empty() {
        let v: Vec<String> = bad.iter().map(|s| s.to_string()).collect();
        return Some(format!("금지 이름 {}(합격기준 A1 — archive 에 프로젝트 코드 0)", strs_repr(&v)));
    }
    if parts.len() == 1 {
        return if PAYLOAD_ALLOWLIST_TOP.contains(&rel) { None } else { Some("최상위 allowlist 밖".into()) };
    }
    if parts[0] == ARTIFACTS_DIR && parts.len() >= 3 && PAYLOAD_SLOT_DIRS.contains(&parts[1]) {
        return None;
    }
    Some(format!("{ARTIFACTS_DIR}/<{}>/** 밖", PAYLOAD_SLOT_DIRS.join("|")))
}

/// 페이로드 상대경로 하나의 판정. None = 허용. 순수 함수.
pub fn allowlist_violation(rel: &str) -> HR<Option<String>> {
    Ok(allow_why(rel, true))
}

/// A path name as Python's `os.walk` sees it: valid UTF-8 → itself; otherwise surrogate-escaped (`\udcXX` per byte).
#[derive(Clone)]
struct PyName {
    text: String, // invalid bytes as private-use sentinels U+10FF00+b
    utf8: bool,
}
impl PyName {
    fn from_bytes(b: &[u8]) -> PyName {
        match std::str::from_utf8(b) {
            Ok(s) => PyName { text: s.to_string(), utf8: true },
            Err(_) => {
                let mut text = String::new();
                let mut rest = b;
                while !rest.is_empty() {
                    match std::str::from_utf8(rest) {
                        Ok(s) => {
                            text.push_str(s);
                            break;
                        }
                        Err(e) => {
                            let ok = e.valid_up_to();
                            text.push_str(std::str::from_utf8(&rest[..ok]).unwrap());
                            let n = e.error_len().unwrap_or(rest.len() - ok);
                            for &x in &rest[ok..ok + n] {
                                text.push(char::from_u32(0x10FF00 + x as u32).unwrap());
                            }
                            rest = &rest[ok + n..];
                        }
                    }
                }
                PyName { text, utf8: false }
            }
        }
    }
    /// `repr()` of a str built around this name.
    fn repr_of(s: &str) -> String {
        let mut r = crate::value::py_repr(s);
        for x in 0x80u32..=0xFF {
            let c = char::from_u32(0x10FF00 + x).unwrap();
            let from = crate::value::py_repr(&c.to_string());
            let from = &from[1..from.len() - 1];
            if r.contains(from) {
                r = r.replace(from, &format!("\\udc{x:02x}"));
            }
        }
        r
    }
}
fn names_repr(v: &[String]) -> String {
    let parts: Vec<String> = v.iter().map(|s| PyName::repr_of(s)).collect();
    format!("[{}]", parts.join(", "))
}

fn pjoin(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for part in rel.split('/').filter(|x| !x.is_empty() && *x != ".") {
        p.push(part);
    }
    p
}

struct Walk {
    links: Vec<String>,
    odd: Vec<String>,
    outside: Vec<String>,
    rels: Vec<String>,
}

fn relname(root: &Path, p: &Path) -> PyName {
    let r = p.strip_prefix(root).unwrap_or(p);
    PyName::from_bytes(r.as_os_str().as_bytes())
}

fn walk_dir(root: &Path, dir: &Path, w: &mut Walk) -> HR<()> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Ok(()) };
    let mut dirs: Vec<OsString> = Vec::new();
    let mut files: Vec<OsString> = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if std::fs::metadata(&p).map(|m| m.is_dir()).unwrap_or(false) {
            dirs.push(e.file_name());
        } else {
            files.push(e.file_name());
        }
    }
    dirs.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    files.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let mut keep = Vec::new();
    for d in dirs {
        let p = dir.join(&d);
        if p.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            w.links.push(relname(root, &p).text);
        } else {
            keep.push(p);
        }
    }
    for f in files {
        let p = dir.join(&f);
        let name = relname(root, &p);
        let md = p.symlink_metadata().map_err(|e| core::io_err(&e, &p))?;
        if md.file_type().is_symlink() {
            w.links.push(name.text);
            continue;
        }
        if !md.file_type().is_file() {
            w.odd.push(name.text);
            continue;
        }
        if let Some(why) = allow_why(&name.text, name.utf8) {
            w.outside.push(format!("{} ({why})", name.text));
            continue;
        }
        w.rels.push(name.text);
    }
    for p in keep {
        walk_dir(root, &p, w)?;
    }
    Ok(())
}

/// 페이로드 디렉터리의 파일 전수(정렬된 posix 상대경로). 목록 밖·심링크·비정규 파일·필수 부재 = 차단.
pub fn payload_files(payload_dir: &Path) -> HR<Vec<String>> {
    let root = payload_dir;
    if !root.is_dir() {
        return core::fail("HINT_PAYLOAD_DIR_ABSENT", format!("페이로드 디렉터리가 없다: {}", core::posix(root)), None);
    }
    let mut w = Walk { links: vec![], odd: vec![], outside: vec![], rels: vec![] };
    walk_dir(root, root, &mut w)?;
    if !w.links.is_empty() {
        return core::fail("HINT_PAYLOAD_SYMLINK", format!("심링크는 페이로드에 담지 않는다(대상이 트리 밖일 수 있다): {}", names_repr(&w.links)), None);
    }
    if !w.odd.is_empty() {
        return core::fail("HINT_PAYLOAD_NOT_REGULAR", format!("정규 파일이 아닌 항목: {}", names_repr(&w.odd)), None);
    }
    if !w.outside.is_empty() {
        let head: Vec<String> = w.outside.iter().take(20).cloned().collect();
        let top: Vec<String> = PAYLOAD_ALLOWLIST_TOP.iter().map(|s| s.to_string()).collect();
        return core::fail(
            "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST",
            format!("allowlist 밖 파일 {}건: {}", w.outside.len(), names_repr(&head)),
            Some(&format!("페이로드에는 최상위 {} 와 {ARTIFACTS_DIR}/<slot>/** 만 둔다(hintlib.branch.PAYLOAD_*).", strs_repr(&top))),
        );
    }
    let mut pdoc = J::Null;
    if w.rels.iter().any(|r| r == PAYLOAD_JSON) {
        if let Ok(t) = core::read_text(&root.join(PAYLOAD_JSON)) {
            pdoc = crate::value::loads(&t).unwrap_or(J::Null);
        }
    }
    let missing: Vec<String> = required_top(&pdoc)?.into_iter().filter(|t| !w.rels.contains(t)).collect();
    if !missing.is_empty() {
        return core::fail(
            "HINT_PAYLOAD_REQUIRED_ABSENT",
            format!("필수 최상위 파일 부재: {}", strs_repr(&missing)),
            Some("hint.sh publish 가 스캐폴드를 만든다 — 지우지 않았는지 확인한다."),
        );
    }
    let mut rels = w.rels;
    rels.sort();
    Ok(rels)
}

// ── 트리 짓기 ────────────────────────────────────────────────────────────────────────────────
pub fn _mode_of(p: &Path) -> HR<String> {
    let md = p.symlink_metadata().map_err(|e| core::io_err(&e, p))?;
    Ok(if md.permissions().mode() & 0o100 != 0 { "100755" } else { "100644" }.to_string())
}

/// blob 을 object DB 에 쓴다(`--no-filters`). 반환 = {rel: sha} (J dict · 입력 순서).
pub fn _hash_files(repo: &Path, root: &Path, rels: &[String]) -> HR<J> {
    let input: String = rels.iter().map(|r| format!("{}\n", core::posix(&pjoin(root, r)))).collect();
    let out = core::git_full(repo, &["hash-object", "-w", "--no-filters", "--stdin-paths"], true, Some(&input), &[])?.stdout;
    let shas: Vec<&str> = out.split_whitespace().collect();
    if shas.len() != rels.len() {
        return core::fail("HINT_TREE_BUILD_FAILED", format!("hash-object 출력 {}건 ≠ 입력 {}건", shas.len(), rels.len()), None);
    }
    let mut m = Map::new();
    for (r, s) in rels.iter().zip(shas) {
        m.insert(r.clone(), s);
    }
    Ok(J::Dict(m))
}

/// `git mktree -z` of `(mode, type, sha, name)` entries.
pub fn _mktree(repo: &Path, entries: &[(String, String, String, String)]) -> HR<String> {
    let payload: String = entries.iter().map(|(m, t, s, n)| format!("{m} {t} {s}\t{n}\0")).collect();
    Ok(core::git_full(repo, &["mktree", "-z"], true, Some(&payload), &[])?.stdout.trim().to_string())
}

enum Node {
    Dir(Vec<(String, Node)>),
    File(String),
}

fn fold(repo: &Path, node: &[(String, Node)], modes: &BTreeMap<String, String>, shas: &J) -> HR<String> {
    let mut entries = Vec::new();
    for (name, v) in node {
        match v {
            Node::Dir(ch) => entries.push(("040000".to_string(), "tree".to_string(), fold(repo, ch, modes, shas)?, name.clone())),
            Node::File(rel) => entries.push((modes[rel].clone(), "blob".into(), shas.get_str(rel).unwrap_or_default().to_string(), name.clone())),
        }
    }
    _mktree(repo, &entries)
}

/// allowlist 목록으로 트리를 새로 짓고, 지은 결과를 다시 읽어 입력과 대조한다(C4).
pub fn build_tree(repo: &Path, payload_dir: &Path, rels: &[String]) -> HR<String> {
    let root = payload_dir;
    for rel in rels {
        if let Some(why) = allow_why(rel, true) {
            return core::fail("HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST", format!("{rel}: {why}"), None);
        }
        let p = pjoin(root, rel);
        let link = p.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if link || !p.is_file() {
            return core::fail("HINT_PAYLOAD_FILE_ABSENT", format!("목록에 있으나 정규 파일 실물이 없다: {rel} — 부재를 성공으로 넘기지 않는다"), None);
        }
    }
    let shas = _hash_files(repo, root, rels)?;
    let mut modes = BTreeMap::new();
    for r in rels {
        modes.insert(r.clone(), _mode_of(&pjoin(root, r))?);
    }
    let mut nested: Vec<(String, Node)> = Vec::new();
    for rel in rels {
        let parts: Vec<&str> = rel.split('/').collect();
        let (name, dirs) = parts.split_last().unwrap();
        let mut node = &mut nested;
        for d in dirs {
            let idx = match node.iter().position(|(k, _)| k == d) {
                Some(i) => i,
                None => {
                    node.push((d.to_string(), Node::Dir(Vec::new())));
                    node.len() - 1
                }
            };
            node = match &mut node[idx].1 {
                Node::Dir(ch) => ch,
                Node::File(_) => return core::fail("HINT_TREE_BUILD_FAILED", format!("파일과 디렉터리 이름 충돌: {rel}"), None),
            };
        }
        if node.iter().any(|(k, _)| k == name) {
            return core::fail("HINT_TREE_BUILD_FAILED", format!("중복 경로: {rel}"), None);
        }
        node.push((name.to_string(), Node::File(rel.clone())));
    }
    let tree = fold(repo, &nested, &modes, &shas)?;
    let got: BTreeMap<String, (String, String)> = entries(repo, &tree)?.into_iter().map(|e| (e.path, (e.mode, e.sha))).collect();
    let want: BTreeMap<String, (String, String)> =
        rels.iter().map(|r| (r.clone(), (modes[r].clone(), shas.get_str(r).unwrap_or_default().to_string()))).collect();
    if got != want {
        let extra: Vec<String> = got.keys().filter(|k| !want.contains_key(*k)).cloned().collect();
        let lack: Vec<String> = want.keys().filter(|k| !got.contains_key(*k)).cloned().collect();
        let diff: Vec<String> = got.keys().filter(|k| want.get(*k).map(|w| w != &got[*k]).unwrap_or(false)).cloned().collect();
        return core::fail(
            "HINT_TREE_BUILD_FAILED",
            format!("트리 전수 대조 실패 — 초과 {} · 부족 {} · 모드/blob 불일치 {}", strs_repr(&extra), strs_repr(&lack), strs_repr(&diff)),
            None,
        );
    }
    Ok(tree)
}

/// One `ls-tree` record.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeEnt {
    pub mode: String,
    pub typ: String,
    pub sha: String,
    pub path: String,
}
impl TreeEnt {
    pub fn to_j(&self) -> J {
        jd! {"mode" => self.mode.clone(), "type" => self.typ.clone(), "sha" => self.sha.clone(), "path" => self.path.clone()}
    }
}

/// Typed [`tree_entries`].
pub fn entries(repo: &Path, rev: &str) -> HR<Vec<TreeEnt>> {
    let raw = core::git_bytes(repo, &["ls-tree", "-r", "-z", "--full-tree", rev], true)?.unwrap_or_default();
    let mut out = Vec::new();
    for rec in raw.split(|b| *b == 0) {
        if rec.is_empty() {
            continue;
        }
        let (meta, path) = match rec.iter().position(|b| *b == b'\t') {
            Some(i) => (&rec[..i], &rec[i + 1..]),
            None => (rec, &b""[..]),
        };
        let meta = lossy(meta);
        let f: Vec<&str> = meta.split(' ').collect();
        if f.len() != 3 {
            return Err(HintError::py("ValueError", format!("not enough values to unpack (expected 3, got {})", f.len())));
        }
        out.push(TreeEnt { mode: f[0].into(), typ: f[1].into(), sha: f[2].into(), path: lossy(path) });
    }
    Ok(out)
}

/// `ls-tree -r -z` → [{mode, type, sha, path}].
pub fn tree_entries(repo: &Path, rev: &str) -> HR<Vec<J>> {
    Ok(entries(repo, rev)?.iter().map(TreeEnt::to_j).collect())
}

pub fn tree_paths(repo: &Path, rev: &str) -> HR<Vec<String>> {
    let mut v: Vec<String> = entries(repo, rev)?.into_iter().map(|e| e.path).collect();
    v.sort();
    Ok(v)
}

/// 이미 있는 커밋·트리가 allowlist 계약을 지키는지. `CODE: 메시지` 목록.
pub fn tree_violations(repo: &Path, rev: &str) -> HR<Vec<String>> {
    let mut probs = Vec::new();
    let mut paths = Vec::new();
    for e in entries(repo, rev)? {
        paths.push(e.path.clone());
        if e.typ != "blob" {
            probs.push(format!("HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST: {} (blob 이 아닌 항목 {})", e.path, e.typ));
            continue;
        }
        if e.mode == "120000" {
            probs.push(format!("HINT_PAYLOAD_SYMLINK: {}", e.path));
            continue;
        }
        if let Some(why) = allow_why(&e.path, true) {
            probs.push(format!("HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST: {} ({why})", e.path));
        }
    }
    let mut pdoc = J::Null;
    if paths.iter().any(|p| p == PAYLOAD_JSON) {
        if let Some(raw) = read_blob_bytes(repo, rev, PAYLOAD_JSON)? {
            pdoc = json_of_bytes(&raw).unwrap_or(J::Null);
        }
    }
    let missing: Vec<String> = required_top(&pdoc)?.into_iter().filter(|t| !paths.contains(t)).collect();
    if !missing.is_empty() {
        probs.push(format!("HINT_PAYLOAD_REQUIRED_ABSENT: 필수 최상위 파일 부재 {}", strs_repr(&missing)));
    }
    Ok(probs)
}

// ── 페이로드 문서 정합 ─────────────────────────────────────────────────────────────────────────
pub fn _read_json_file(path: &Path, code: &str) -> HR<J> {
    let doc = core::read_json_code(path, code)?;
    if !doc.is_dict() {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return core::fail(code, format!("JSON 객체가 아니다: {name}"), None);
    }
    Ok(doc)
}

/// 페이로드 커밋 메시지를 파생한다.
pub fn commit_message(payload_dir: &Path, generated_utc: &str) -> HR<String> {
    let payload = _read_json_file(&payload_dir.join(PAYLOAD_JSON), "HINT_PAYLOAD_JSON_UNREADABLE")?;
    let prov = _read_json_file(&payload_dir.join(PROVENANCE_JSON), "HINT_PROVENANCE_UNREADABLE")?;
    let sa = prov.get("source_anchor").cloned().unwrap_or(J::Null);
    let tag = payload.get("tag").cloned().unwrap_or(J::Null).display();
    let sa_s = if sa.truthy() { sa.display() } else { "none".into() };
    let fmt = payload.get("format").cloned().unwrap_or(J::Null).display();
    let utc = core::require_utc(Some(generated_utc), "--generated-utc")?;
    Ok(format!("hint: {tag}\n\nsource_anchor: {sa_s}\npayload_format: {fmt}\ngenerated_utc: {utc}\n"))
}

/// PAYLOAD.json · PROVENANCE.json · 커밋 메시지 · 서사 4종의 정합(순수). `CODE: 설명` 목록(빈 = 정합).
pub fn payload_doc_problems(payload: &J, prov: &J, paths: &J, message: &J, docs: &J, expect_tag: Option<&str>) -> HR<Vec<String>> {
    let mut out = _meta_problems(payload, prov, paths, message, expect_tag)?;
    out.extend(doc_residue_problems(docs)?);
    Ok(out)
}

fn path_set(paths: &J) -> BTreeSet<String> {
    paths.items().iter().map(J::display).collect()
}

pub fn _meta_problems(payload: &J, prov: &J, paths: &J, message: &J, expect_tag: Option<&str>) -> HR<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    macro_rules! add {
        ($code:expr, $msg:expr $(,)?) => {
            out.push(format!("{}: {}", $code, $msg))
        };
    }
    if !payload.is_dict() {
        add!("HINT_PAYLOAD_JSON_UNREADABLE", "PAYLOAD.json 이 JSON 객체가 아니다.");
        return Ok(out);
    }
    if !prov.is_dict() {
        add!("HINT_PROVENANCE_UNREADABLE", "PROVENANCE.json 이 JSON 객체가 아니다.");
        return Ok(out);
    }
    let tag_j = payload.get("tag").cloned().unwrap_or(J::Null);
    let tag = match tag_j.as_str() {
        Some(t) if t.starts_with(core::HINT_TAG_PREFIX) => t.to_string(),
        _ => {
            add!("HINT_PAYLOAD_TAG_ABSENT", format!("PAYLOAD.tag 가 hint 태그가 아니다: {}", tag_j.repr()));
            return Ok(out);
        }
    };
    let tr = crate::value::py_repr(&tag);
    if let Some(e) = expect_tag {
        if e != tag {
            add!("HINT_PAYLOAD_TAG_MISMATCH", format!("PAYLOAD.tag={tr} ≠ 봉인 대상 {} — 남의 페이로드다.", crate::value::py_repr(e)));
        }
    }
    let ptag = prov.get("tag").cloned().unwrap_or(J::Null);
    if ptag != J::from(tag.as_str()) {
        add!("HINT_PAYLOAD_TAG_MISMATCH", format!("PROVENANCE.tag={} ≠ PAYLOAD.tag={tr} — 남의 페이로드다.", ptag.repr()));
    }
    match naming::validate_new_name(&tag) {
        Ok(()) => {}
        Err(e) if e.is_py() => return Err(e),
        Err(e) => add!(&e.code, e.message.clone()),
    }
    let fmt = payload.get("format").cloned().unwrap_or(J::Null);
    if fmt != J::from(PAYLOAD_FORMAT) {
        add!(
            "HINT_PAYLOAD_FORMAT",
            format!("PAYLOAD.format={} ≠ {} — 이 브랜치에 새로 얹는 페이로드는 신 형식만이다.", fmt.repr(), crate::value::py_repr(PAYLOAD_FORMAT)),
        );
    }
    let declared = prov.get("payload_files").cloned().unwrap_or(J::Null);
    let decl_ok = match &declared {
        J::List(l) => l.iter().all(J::is_str) && l.iter().map(J::display).collect::<BTreeSet<_>>().len() == l.len(),
        _ => false,
    };
    if !decl_ok {
        add!("HINT_PROVENANCE_SHAPE", "PROVENANCE.payload_files 는 중복 없는 문자열 목록이어야 한다.");
    } else {
        let mut d: BTreeSet<String> = declared.items().iter().map(J::display).collect();
        let p = path_set(paths);
        let only_d: Vec<String> = d.difference(&p).cloned().collect();
        d.insert(PROVENANCE_JSON.into());
        if d != p {
            let only_p: Vec<String> = p.iter().filter(|x| !d.contains(*x)).cloned().collect();
            add!(
                "HINT_PROVENANCE_FILES_MISMATCH",
                format!("PROVENANCE.payload_files ≠ 실물 — 선언만 {} · 실물만 {}", strs_repr(&only_d), strs_repr(&only_p)),
            );
        }
    }
    let mut sa: Option<String> = None;
    match prov.get("source_anchor") {
        None => add!("HINT_PROVENANCE_SHAPE", "PROVENANCE.source_anchor 키가 없다(값이 없으면 null 로 명시한다)."),
        Some(J::Null) => {}
        Some(v) => match v.as_str() {
            Some(s) if _FULL_SHA_RE().is_match(s) => sa = Some(s.to_string()),
            _ => add!("HINT_PROVENANCE_SHAPE", format!("PROVENANCE.source_anchor 가 전체 SHA 가 아니다: {}", v.repr())),
        },
    }
    match message.as_str() {
        Some(m) if !py_strip(m).is_empty() && m.contains(tag.as_str()) => {
            if let Some(s) = sa.as_deref().filter(|s| !s.is_empty()) {
                if !m.contains(s) {
                    add!("HINT_ANCHOR_TRIPLE_MISMATCH", format!("커밋 메시지에 PROVENANCE.source_anchor({})가 없다.", core::head_chars(s, 12)));
                }
            }
        }
        _ => add!("HINT_COMMIT_MESSAGE_SHAPE", "커밋 메시지는 비어 있지 않고 태그 이름을 담아야 한다(branch.commit_message 로 파생한다)."),
    }
    match payload.get("naming") {
        Some(nm) if nm.is_dict() => match naming::compose_from_payload(nm) {
            Ok(got) => {
                if got != tag {
                    add!("HINT_DERIVED_NAME_MISMATCH", format!("PAYLOAD.naming 재조립 {} ≠ PAYLOAD.tag {tr}", crate::value::py_repr(&got)));
                }
            }
            Err(e) if e.is_py() => return Err(e),
            Err(e) => add!(&e.code, e.message.clone()),
        },
        _ => add!("HINT_PAYLOAD_NAMING_ABSENT", "PAYLOAD.naming(축별 {값, 출처})이 없다 — 이름 재파생 대조가 불가능하다."),
    }
    Ok(out)
}

/// 서사 4종의 저작 잔재(PROMPT 지시 · `<<AGENT:` 자리표시). docs = {파일명: Str | {"__bytes__"}} · 부재 항목은 건너뛴다.
pub fn doc_residue_problems(docs: &J) -> HR<Vec<String>> {
    let mark = crate::template::AGENT_MARK;
    let mut out = Vec::new();
    for name in PAYLOAD_DOCS {
        let data = match docs.get(name) {
            None | Some(J::Null) => continue,
            Some(d) => d,
        };
        let text = match data {
            J::Dict(m) if m.contains_key("__bytes__") => lossy(&j_bytes(data).unwrap_or_default()),
            other => other.display(),
        };
        if let Some(m) = PROMPT_RESIDUE_RE().search(&text) {
            let st = m.get(0).unwrap().start();
            let ln = text[..st].matches('\n').count() + 1;
            out.push(format!("HINT_PROMPT_RESIDUE: {name}:{ln}: 기재 지시(PROMPT)가 남았다 — template.seal_prompts 전이거나 저작이 끝나지 않았다."));
        }
        if let Some(i) = text.find(mark) {
            let ln = text[..i].matches('\n').count() + 1;
            out.push(format!("HINT_AGENT_PLACEHOLDER_RESIDUE: {name}:{ln}: 미저작 자리표시 `{mark}` 가 남았다 — 그 절을 저작한 뒤 커밋한다."));
        }
    }
    Ok(out)
}

fn docs_j(docs: &[(String, Vec<u8>)]) -> J {
    let mut m = Map::new();
    for (k, v) in docs {
        m.insert(k.clone(), lossy(v));
    }
    J::Dict(m)
}

/// 커밋 전 정합(디스크). 반환 = 페이로드가 말하는 태그.
pub fn _validate_payload_docs(root: &Path, rels: &[String], message: &str) -> HR<String> {
    let payload = _read_json_file(&root.join(PAYLOAD_JSON), "HINT_PAYLOAD_JSON_UNREADABLE")?;
    let prov = _read_json_file(&root.join(PROVENANCE_JSON), "HINT_PROVENANCE_UNREADABLE")?;
    let mut docs = Vec::new();
    for n in PAYLOAD_DOCS {
        if rels.iter().any(|r| r == n) {
            let p = root.join(n);
            docs.push((n.to_string(), std::fs::read(&p).map_err(|e| core::io_err(&e, &p))?));
        }
    }
    let probs = payload_doc_problems(&payload, &prov, &J::from(rels.to_vec()), &J::from(message), &docs_j(&docs), None)?;
    if !probs.is_empty() {
        let code = probs[0].split(':').next().unwrap_or("").to_string();
        return core::fail(
            &code,
            format!("페이로드 문서 정합 {}건:\n  {}", probs.len(), probs.join("\n  ")),
            Some("hint.sh publish 가 만든 스캐폴드를 손으로 고치지 않았는지 확인하고, 저작을 마친 뒤 hint.sh continue 로 다시 실행한다."),
        );
    }
    Ok(payload["tag"].display())
}

// ── ref ───────────────────────────────────────────────────────────────────────────────────────
/// 로컬 `refs/heads/hint`(형식 전환·자체검사 전용).
pub fn hint_tip(repo: &Path) -> HR<Option<String>> {
    let tip = core::git_nocheck(repo, &["rev-parse", "--verify", "--quiet", HINT_BRANCH_REF])?.stdout;
    let tip = py_strip(&tip);
    Ok(if tip.is_empty() { None } else { Some(tip.to_string()) })
}

pub fn _require_hint_not_checked_out(repo: &Path) -> HR<()> {
    let out = core::git_nocheck(repo, &["worktree", "list", "--porcelain"])?;
    if out.returncode != 0 {
        return core::fail("HINT_GIT_FAILED", format!("git worktree list 실패(rc={}): {}", out.returncode, py_strip(&out.stderr)), None);
    }
    let want = format!("branch {HINT_BRANCH_REF}");
    if splitlines(&out.stdout).iter().any(|ln| py_strip(ln) == want) {
        return core::fail(
            "HINT_HINT_BRANCH_CHECKED_OUT",
            format!("{HINT_BRANCH_REF} 가 워크트리에 체크아웃돼 있다 — 배관으로 전진하면 그 워크트리가 깨진다."),
            Some("그 워크트리를 다른 브랜치로 옮긴 뒤 다시 실행한다(hint 브랜치는 체크아웃하지 않는다)."),
        );
    }
    Ok(())
}

/// compare-and-swap(없을 때는 zero-oid 생성 전용 CAS).
pub fn _update_hint_ref(repo: &Path, new: &str, expected_old: Option<&str>) -> HR<()> {
    let old = match expected_old {
        Some(o) if !o.is_empty() => o.to_string(),
        _ => "0".repeat(new.chars().count()),
    };
    let msg = format!("hint: branch {}", core::head_chars(new, 12));
    let r = core::git_nocheck(repo, &["update-ref", "-m", &msg, HINT_BRANCH_REF, new, &old])?;
    if r.returncode != 0 {
        return core::fail(
            "HINT_BRANCH_CAS_CONFLICT",
            format!("{HINT_BRANCH_REF} 가 그 사이 바뀌었다(기대 {}) — 덮지 않는다: {}", core::head_chars(&old, 12), py_strip(&r.stderr)),
            Some("다른 발행이 끼어들었다. 브랜치 상태를 확인하고 hint.sh continue 를 다시 실행한다."),
        );
    }
    Ok(())
}

// ── 안내 커밋 ─────────────────────────────────────────────────────────────────────────────────
pub const BRANCH_FORMAT: &str = "v7";
pub const GUIDE_README: &str = "README.md";
crate::re_static!(_GUIDE_MARKER_RE, r"^<!-- hint-branch-format: (v[1-9][0-9]*) -->$");

/// 안내 README 첫 줄(개행 없음).
pub fn guide_marker(fmt: &str) -> HR<String> {
    if !_FMT_SHAPE_RE().is_full(fmt) {
        return core::fail("HINT_BRANCH_FORMAT_SHAPE", format!("형식 버전 모양이 아니다: {}", crate::value::py_repr(fmt)), None);
    }
    Ok(format!("<!-- hint-branch-format: {fmt} -->"))
}

/// README 텍스트 → 첫 줄 형식 마커의 버전 · 마커 없음 = None.
pub fn readme_format_text(text: &str) -> Option<String> {
    let first = text.split('\n').next().unwrap_or("").trim_end_matches('\r');
    _GUIDE_MARKER_RE().match_at_start(first).map(|m| m.get(1).unwrap().as_str().to_string())
}

/// README 바이트(`{"__bytes__"}`)/문자열(`Str`) → 형식 · None = 마커 없음.
pub fn readme_format(data: &J) -> HR<Option<String>> {
    Ok(match data {
        J::Null => None,
        J::Dict(m) if m.contains_key("__bytes__") => readme_format_text(&lossy(&j_bytes(data).unwrap_or_default())),
        other => readme_format_text(&other.display()),
    })
}
fn readme_format_bytes(data: Option<&[u8]>) -> Option<String> {
    data.and_then(|d| readme_format_text(&lossy(d)))
}

pub fn _format_num(fmt: Option<&str>) -> HR<i64> {
    Ok(match fmt {
        Some(f) if !f.is_empty() => {
            let d: String = f.chars().skip(1).collect();
            d.parse().map_err(|_| HintError::py("ValueError", format!("invalid literal for int() with base 10: {}", crate::value::py_repr(&d))))?
        }
        _ => 0,
    })
}

pub fn _commit_parents(raw: &str) -> HR<Vec<String>> {
    let head = raw.split("\n\n").next().unwrap_or("");
    Ok(splitlines(head).into_iter().filter(|l| l.starts_with("parent ")).map(|l| l[7..].to_string()).collect())
}
fn parents_of(raw: &str) -> Vec<String> {
    _commit_parents(raw).unwrap_or_default()
}

/// 로컬 커밋 하나가 안내 커밋인가 → (형식 | None, 안내가 아닌 사유 | None).
pub fn guide_info(repo: &Path, sha: &str) -> HR<(Option<String>, Option<String>)> {
    let raw = if sha.is_empty() { None } else { read_object_text(repo, "commit", sha)? };
    if raw.is_none() {
        return Ok((None, Some(format!("커밋 오브젝트가 로컬에 없다({})", core::head_chars(sha, 12)))));
    }
    let ents = entries(repo, sha)?;
    if ents.len() != 1 || ents[0].path != GUIDE_README || ents[0].typ != "blob" || ents[0].mode != "100644" {
        return Ok((None, Some(format!("트리가 {GUIDE_README} 하나(100644)가 아니다 — 안내 커밋이 아니다(항목 {}개)", ents.len()))));
    }
    match readme_format_bytes(read_blob_bytes(repo, sha, GUIDE_README)?.as_deref()) {
        None => Ok((None, Some("README 첫 줄에 형식 마커(`<!-- hint-branch-format: vN -->`)가 없다 — 마커 도입 전(v6 이하) 안내다".into()))),
        Some(f) => Ok((Some(f), None)),
    }
}

pub fn _format_mismatch(where_: &str, sha: &str, fmt: Option<&str>, why: Option<&str>) -> HR<J> {
    let tail = match why {
        Some(w) if !w.is_empty() => w.to_string(),
        _ => format!("안내 형식 {} ≠ 발행기 형식 {BRANCH_FORMAT}", fmt.unwrap_or("None")),
    };
    core::fail(
        "HINT_BRANCH_FORMAT_MISMATCH",
        format!("{where_} {} 는 발행기 형식 {BRANCH_FORMAT} 의 안내 커밋이 아니다 — {tail}", core::head_chars(sha, 12)),
        Some(
            "형식 전환이 필요하다: 사람 승인(G2) 뒤 `hint.sh branch-transition --remote <원격> --generated-utc <UTC> \
             --approved-by \"<사람 발화 전사>\" --approved-utc <UTC>` 가 그 위에 v7 안내 커밋을 얹고 exact refspec 로 \
             non-force push 한다(발행은 브랜치를 밀지 않는다).",
        ),
    )
}

/// sha 가 발행기 형식의 안내 커밋인가(로컬 오브젝트). 아니면 HINT_BRANCH_FORMAT_MISMATCH. (Python 기본 where_="안내 커밋")
pub fn require_guide(repo: &Path, sha: &str, where_: &str) -> HR<String> {
    if !_FULL_SHA_RE().is_match(sha) {
        return core::fail("HINT_BRANCH_GUIDE_SHAPE", format!("안내 커밋은 전체 SHA 여야 한다: {}", crate::value::py_repr(sha)), None);
    }
    let (fmt, why) = guide_info(repo, sha)?;
    if fmt.as_deref() != Some(BRANCH_FORMAT) {
        _format_mismatch(where_, sha, fmt.as_deref(), why.as_deref())?;
    }
    Ok(sha.to_string())
}

fn has_commit(repo: &Path, sha: &str) -> HR<bool> {
    Ok(core::git_nocheck(repo, &["cat-file", "-e", &format!("{sha}^{{commit}}")])?.returncode == 0)
}

/// 원격 tip 오브젝트가 로컬에 없으면 그 한 ref 만 fetch 한다.
pub fn _ensure_local_commit(repo: &Path, remote: &str, sha: &str) -> HR<()> {
    if has_commit(repo, sha)? {
        return Ok(());
    }
    crate::tag::fetch_ref_objects(repo, remote, HINT_BRANCH_REF)?;
    if !has_commit(repo, sha)? {
        return core::fail(
            "HINT_BRANCH_GUIDE_UNREADABLE",
            format!("원격 {HINT_BRANCH_REF} tip {} 을 fetch 뒤에도 로컬에서 읽을 수 없다 — 그 사이 원격 tip 이 바뀌었을 수 있다.", core::head_chars(sha, 12)),
            Some("다시 실행한다(조회와 fetch 사이 경합)."),
        );
    }
    Ok(())
}

fn remote_bad(remote: &str) -> bool {
    py_strip(remote).is_empty() || py_lstrip(remote).starts_with('-')
}

/// 페이로드 커밋의 부모가 될 안내 커밋 → (sha, 형식).
pub fn guide_commit(repo: &Path, remote: &str, pinned: Option<&str>) -> HR<(String, String)> {
    if let Some(p) = pinned {
        return Ok((require_guide(repo, p, "지정 안내 커밋(pinned)")?, BRANCH_FORMAT.to_string()));
    }
    if remote_bad(remote) {
        return core::fail("HINT_BRANCH_REMOTE_INVALID", format!("안내 커밋을 읽을 원격 인자가 올바르지 않다: {}", crate::value::py_repr(remote)), None);
    }
    let tip = match crate::tag::remote_ref_object(repo, remote, HINT_BRANCH_REF)? {
        Some(t) if !t.is_empty() => t,
        _ => {
            return core::fail(
                "HINT_BRANCH_GUIDE_ABSENT",
                format!("원격 {} 에 {HINT_BRANCH_REF} 가 없다 — 페이로드 커밋의 부모(안내 커밋)가 없다.", crate::value::py_repr(remote)),
                Some("형식 전환 명령(`hint.sh branch-transition` · 사람 승인)으로 안내 커밋을 만든다."),
            )
        }
    };
    _ensure_local_commit(repo, remote, &tip)?;
    let (fmt, why) = guide_info(repo, &tip)?;
    if fmt.as_deref() != Some(BRANCH_FORMAT) {
        _format_mismatch(&format!("원격 {HINT_BRANCH_REF} tip"), &tip, fmt.as_deref(), why.as_deref())?;
    }
    Ok((tip, fmt.unwrap_or_default()))
}

// ── 형식 전환 ────────────────────────────────────────────────────────────────────────────────
pub const _BRANCH_TRANSITION_MESSAGE: &str = "hint: branch transition → v7\n";
pub const _BRANCH_TEMPLATE_REL: &str = ".claude/skills/hint-publisher/templates/hint-branch-README.md";

/// 추적 HEAD 템플릿 바이트 · 첫 줄 = 형식 마커 · 배포면 PII 거부.
pub fn _transition_template_bytes(repo: &Path) -> HR<Vec<u8>> {
    let data = match core::git_bytes(repo, &["show", &format!("HEAD:{_BRANCH_TEMPLATE_REL}")], false)? {
        Some(d) => d,
        None => return core::fail("HINT_BRANCH_TEMPLATE_ABSENT", format!("추적 전환 템플릿이 없다: {_BRANCH_TEMPLATE_REL}"), None),
    };
    let text = match std::str::from_utf8(&data) {
        Ok(t) => t,
        Err(_) => return core::fail("HINT_BRANCH_TEMPLATE_UNREADABLE", format!("전환 템플릿이 UTF-8이 아니다: {_BRANCH_TEMPLATE_REL}"), None),
    };
    let f = readme_format_text(text);
    if f.as_deref() != Some(BRANCH_FORMAT) {
        let m = guide_marker(BRANCH_FORMAT)?;
        return core::fail(
            "HINT_BRANCH_TEMPLATE_FORMAT",
            format!("전환 템플릿 첫 줄이 형식 마커 `{m}` 가 아니다(읽힌 형식 {}).", opt_repr(f.as_deref())),
            Some(&format!("`{_BRANCH_TEMPLATE_REL}` 첫 줄에 `{m}` 를 두고 커밋한다(마커를 도구가 덧붙이지 않는다).")),
        );
    }
    let terms = pii::require_terms(repo)?;
    let hits = pii::scan_text(text, Some(&terms), "deploy", &[], false, true, None)?;
    if !hits.is_empty() {
        return core::fail("HINT_BRANCH_TEMPLATE_PII", format!("전환 README 배포 PII: {}", pii::render_hits(&hits, 20)?), None);
    }
    Ok(data)
}

fn run_git_bytes(repo: &Path, args: &[&str], input: &[u8]) -> Result<core::ProcBytes, String> {
    let mut argv = vec!["git".to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    core::run_capture(&argv, Some(repo), &[("GIT_TERMINAL_PROMPT".into(), "0".into())], &[], Some(input))
}

/// README.md 한 개(mode 100644) tree SHA. `write=false` 는 object DB 쓰기 0.
pub fn _transition_tree(repo: &Path, readme: &[u8], write: bool) -> HR<String> {
    let mut args = vec!["hash-object"];
    if write {
        args.push("-w");
    }
    args.extend(["--no-filters", "--stdin"]);
    let out = match run_git_bytes(repo, &args, readme) {
        Ok(o) => o,
        Err(e) => return core::fail("HINT_GIT_UNAVAILABLE", format!("git 실행 불가: {e}"), None),
    };
    if out.returncode != 0 {
        return core::fail("HINT_GIT_FAILED", format!("전환 README blob hash 실패: {}", py_strip(&lossy(&out.stderr))), None);
    }
    let sha = py_strip(&lossy(&out.stdout)).to_string();
    if write {
        let tree = _mktree(repo, &[("100644".into(), "blob".into(), sha.clone(), GUIDE_README.into())])?;
        let want = vec![TreeEnt { mode: "100644".into(), typ: "blob".into(), sha, path: GUIDE_README.into() }];
        if entries(repo, &tree)? != want {
            return core::fail("HINT_BRANCH_TRANSITION_TREE_MISMATCH", "전환 트리가 README.md 하나(mode 100644)가 아니다.", None);
        }
        return Ok(tree);
    }
    let Some(bin) = unhex(&sha) else {
        return core::fail("HINT_GIT_UNAVAILABLE", "전환 tree SHA 계산 실패: non-hexadecimal number found in fromhex() arg", None);
    };
    let mut data = b"100644 README.md\0".to_vec();
    data.extend(bin);
    let tr = match run_git_bytes(repo, &["hash-object", "-t", "tree", "--stdin"], &data) {
        Ok(o) => o,
        Err(e) => return core::fail("HINT_GIT_UNAVAILABLE", format!("전환 tree SHA 계산 실패: {e}"), None),
    };
    if tr.returncode != 0 {
        return core::fail("HINT_GIT_FAILED", format!("전환 tree SHA 계산 실패: {}", py_strip(&lossy(&tr.stderr))), None);
    }
    Ok(py_strip(&lossy(&tr.stdout)).to_string())
}

fn commit_matches(repo: &Path, commit: &str, tree: &str, parent: &str, message: &str, epoch: i64) -> HR<bool> {
    let raw = read_object_text(repo, "commit", commit)?.unwrap_or_default();
    let ids = parse_ident_headers(&raw)?;
    let me = synthetic_identity()?;
    let body = raw.split_once("\n\n").map(|x| x.1).unwrap_or("");
    Ok(raw.starts_with(&format!("tree {tree}\n"))
        && parents_of(&raw) == vec![parent.to_string()]
        && body == message
        && ["author", "committer"].iter().all(|r| ident_of(&ids, r).as_deref() == Some(me.as_str()) && epoch_of(&ids, r) == Some(epoch)))
}

/// 결정론·합성 신원·단일 부모 전환 커밋을 오브젝트로만 만든다.
pub fn _transition_commit(repo: &Path, parent: &str, tree: &str, utc: &str) -> HR<String> {
    let env = identity_env_pairs(utc)?;
    let commit = core::git_full(
        repo,
        &["-c", "i18n.commitEncoding=UTF-8", "commit-tree", tree, "--no-gpg-sign", "-p", parent],
        true,
        Some(_BRANCH_TRANSITION_MESSAGE),
        &env,
    )?
    .stdout
    .trim()
    .to_string();
    let epoch = core::parse_utc(utc)?.epoch();
    if !commit_matches(repo, &commit, tree, parent, _BRANCH_TRANSITION_MESSAGE, epoch)? {
        return core::fail("HINT_BRANCH_TRANSITION_RESULT_MISMATCH", "전환 커밋이 요청한 트리·단일 parent·합성 신원·주입 시각과 다르다.", None);
    }
    Ok(commit)
}

pub fn _remote_hint_tip(repo: &Path, remote: &str) -> HR<Option<String>> {
    crate::tag::remote_ref_object(repo, remote, HINT_BRANCH_REF)
}

/// 이미 있는 commit 이 이 README 전환인지 구조로 판정한다(parent=None 이면 부모는 묻지 않는다).
pub fn _existing_transition(repo: &Path, commit: &str, parent: Option<&str>, expected_tree: &str) -> HR<bool> {
    let raw = read_object_text(repo, "commit", commit)?.unwrap_or_default();
    let body = raw.split_once("\n\n").map(|x| x.1).unwrap_or("");
    if !raw.starts_with(&format!("tree {expected_tree}\n")) || body != _BRANCH_TRANSITION_MESSAGE {
        return Ok(false);
    }
    let parents = parents_of(&raw);
    let ids = parse_ident_headers(&raw)?;
    let me = synthetic_identity()?;
    Ok(parents.len() == 1
        && parent.map(|p| parents[0] == p).unwrap_or(true)
        && ["author", "committer"].iter().all(|r| ident_of(&ids, r).as_deref() == Some(me.as_str())))
}

/// 로컬 hint 를 옮기면 태그도 원격 tip 도 붙잡지 않게 되는 페이로드 커밋 목록.
pub fn _orphan_payloads(repo: &Path, local: &str, keep: &str) -> HR<Vec<String>> {
    let r = core::git_nocheck(repo, &["rev-list", local, "--not", keep, "--glob=refs/tags/hint/*"])?;
    if r.returncode != 0 {
        return core::fail("HINT_GIT_FAILED", format!("rev-list 실패(rc={}): {}", r.returncode, py_strip(&r.stderr)), None);
    }
    let mut out = Vec::new();
    for c in r.stdout.split_whitespace() {
        if read_blob_bytes(repo, c, PAYLOAD_JSON)?.is_some() {
            out.push(c.to_string());
        }
    }
    Ok(out)
}

pub fn _require_approval(approved_by: Option<&str>, approved_utc: Option<&str>) -> HR<String> {
    if approved_by.map(|s| py_strip(s).is_empty()).unwrap_or(true) || approved_utc.is_none() {
        return core::fail(
            "HINT_APPROVAL_ABSENT",
            "형식 전환은 원격 브랜치를 바꾼다 — 사람 승인 전사가 필요하다(--approved-by · --approved-utc · plan_26092908 §6 G2). 어느 ref 도 쓰지 않았다.",
            Some("사람의 승인 발화를 전사해 --approved-by \"…\" --approved-utc <UTC> 로 다시 실행한다."),
        );
    }
    core::require_utc(approved_utc, "--generated-utc")
}

fn orphan_fail(prefix: &str, orphans: &[String]) -> HR<()> {
    let short: Vec<String> = orphans.iter().take(10).map(|c| core::head_chars(c, 12)).collect();
    core::fail(
        "HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD",
        format!("로컬 {HINT_BRANCH_REF} {prefix} 태그가 붙잡지 않는 페이로드 커밋 {}개가 고아가 된다: {} — 아무것도 쓰지 않았다.", orphans.len(), strs_repr(&short)),
        Some("그 커밋을 가리키는 태그가 있어야 하는지 확인한다(태그 없는 페이로드 커밋 = 발행되지 않은 것)."),
    )
}

fn with(base: &Map, extra: Vec<(&str, J)>) -> J {
    let mut m = base.clone();
    for (k, v) in extra {
        m.insert(k, v);
    }
    J::Dict(m)
}

/// 형식 전환(브랜치 push 의 유일한 경로 · 사람 승인 필수).
pub fn branch_transition(repo: &Path, remote: &str, generated_utc: &str, approved_by: Option<&str>, approved_utc: Option<&str>) -> HR<J> {
    let utc = core::require_utc(Some(generated_utc), "--generated-utc")?;
    if remote_bad(remote) {
        return core::fail("HINT_BRANCH_REMOTE_INVALID", format!("전환 remote 인자가 올바르지 않다: {}", crate::value::py_repr(remote)), None);
    }
    let appr_utc = _require_approval(approved_by, approved_utc)?;
    _require_hint_not_checked_out(repo)?;
    let readme = _transition_template_bytes(repo)?;
    let expected_tree = _transition_tree(repo, &readme, false)?;
    let refspec = format!("{HINT_BRANCH_REF}:{HINT_BRANCH_REF}");
    let remote_tip = match _remote_hint_tip(repo, remote)? {
        Some(t) if !t.is_empty() => t,
        _ => {
            return core::fail(
                "HINT_BRANCH_TRANSITION_REMOTE_ABSENT",
                format!("원격 {}에 {HINT_BRANCH_REF} tip이 없다 — 첫 브랜치 생성은 이 명령의 범위 밖이다(사람이 원격을 준비한다).", crate::value::py_repr(remote)),
                None,
            )
        }
    };
    _ensure_local_commit(repo, remote, &remote_tip)?;
    let local = hint_tip(repo)?;
    let mut base = Map::new();
    base.insert("remote", remote);
    base.insert("refspec", refspec.clone());
    base.insert("format", BRANCH_FORMAT);
    base.insert("approved_by", py_strip(approved_by.unwrap_or("")));
    base.insert("approved_utc", appr_utc);
    base.insert("local_before", J::from(local.clone()));
    base.insert("remote_before", remote_tip.clone());
    let (remote_fmt, _why) = guide_info(repo, &remote_tip)?;
    if _format_num(remote_fmt.as_deref())? > _format_num(Some(BRANCH_FORMAT))? {
        return core::fail(
            "HINT_BRANCH_TRANSITION_DOWNGRADE",
            format!(
                "원격 안내 {} 의 형식 {} 가 이 발행기 {BRANCH_FORMAT} 보다 새것이다 — 되돌리지 않는다.",
                core::head_chars(&remote_tip, 12),
                remote_fmt.as_deref().unwrap_or("None")
            ),
            Some("발행기(저장소)를 갱신한다."),
        );
    }
    if remote_fmt.as_deref() == Some(BRANCH_FORMAT) && _existing_transition(repo, &remote_tip, None, &expected_tree)? {
        if local.as_deref() == Some(remote_tip.as_str()) {
            return Ok(with(
                &base,
                vec![("status", "already-transitioned".into()), ("new", remote_tip.clone().into()), ("local_after", J::from(local.clone())), ("remote_after", remote_tip.clone().into())],
            ));
        }
        let orphans = match &local {
            Some(l) => _orphan_payloads(repo, l, &remote_tip)?,
            None => vec![],
        };
        if !orphans.is_empty() {
            orphan_fail("를 원격 안내로 맞추면", &orphans)?;
        }
        _update_hint_ref(repo, &remote_tip, local.as_deref())?;
        return Ok(with(
            &base,
            vec![("status", "local-synced".into()), ("new", remote_tip.clone().into()), ("local_after", remote_tip.clone().into()), ("remote_after", remote_tip.clone().into())],
        ));
    }
    if let Some(l) = &local {
        if _existing_transition(repo, l, Some(&remote_tip), &expected_tree)? {
            crate::tag::git_push_authenticated(repo, remote, &refspec, false)?;
            let after = _remote_hint_tip(repo, remote)?;
            if after.as_deref() != Some(l.as_str()) {
                return core::fail(
                    "HINT_BRANCH_TRANSITION_REMOTE_SHA_MISMATCH",
                    format!("재개 push 뒤 원격 {HINT_BRANCH_REF}={} ≠ local new {}", core::head_chars(after.as_deref().unwrap_or("None"), 12), core::head_chars(l, 12)),
                    None,
                );
            }
            return Ok(with(
                &base,
                vec![("status", "push-resumed".into()), ("old", remote_tip.clone().into()), ("new", l.clone().into()), ("local_after", l.clone().into()), ("remote_after", J::from(after))],
            ));
        }
        let orphans = _orphan_payloads(repo, l, &remote_tip)?;
        if !orphans.is_empty() {
            orphan_fail("를 옮기면", &orphans)?;
        }
    }
    let tree = _transition_tree(repo, &readme, true)?;
    let new = _transition_commit(repo, &remote_tip, &tree, &utc)?;
    _update_hint_ref(repo, &new, local.as_deref())?;
    crate::tag::git_push_authenticated(repo, remote, &refspec, false)?;
    let after = _remote_hint_tip(repo, remote)?;
    if after.as_deref() != Some(new.as_str()) {
        return core::fail(
            "HINT_BRANCH_TRANSITION_REMOTE_SHA_MISMATCH",
            format!("push 뒤 원격 {HINT_BRANCH_REF}={} ≠ local new {}", core::head_chars(after.as_deref().unwrap_or("None"), 12), core::head_chars(&new, 12)),
            None,
        );
    }
    Ok(with(
        &base,
        vec![("status", "transitioned".into()), ("old", remote_tip.into()), ("new", new.clone().into()), ("local_after", new.into()), ("remote_after", J::from(after))],
    ))
}

/// 페이로드 → allowlist 트리 → 합성 신원·주입 시각 커밋(부모 = 안내 커밋) → 커밋 SHA. ref 는 쓰지 않는다.
pub fn commit_payload(repo: &Path, payload_dir: &Path, message: &str, generated_utc: &str, guide: Option<&str>, remote: Option<&str>) -> HR<String> {
    let utc = core::require_utc(Some(generated_utc), "--generated-utc")?;
    if guide.is_none() && remote.is_none() {
        return core::fail(
            "HINT_BRANCH_GUIDE_REQUIRED",
            "페이로드 커밋의 부모(안내 커밋)가 주어지지 않았다 — guide=<SHA> 또는 remote=<원격> 이 필요하다.",
            Some("branch.guide_commit(repo, remote) 로 원격 hint tip(안내 커밋)을 읽어 넘긴다(plan_26092908 §4.7)."),
        );
    }
    let root = core::resolve(payload_dir);
    let rels = payload_files(&root)?;
    let tag = _validate_payload_docs(&root, &rels, message)?;
    let terms = pii::require_terms(repo)?;
    let hits = pii::scan_tree(&root, Some(&terms), "deploy", &[], true, true, Some(&rels), None)?;
    if !hits.is_empty() {
        return core::fail(
            "HINT_PAYLOAD_PII",
            format!("페이로드 PII {}건(배포면 = 4종 전부 fail-closed):\n  {}", hits.len(), pii::render_located_hits(&hits, 20)?),
            Some("발췌·산출물을 pii.substitute 치환표로 다시 만든다."),
        );
    }
    let ph: Vec<String> = pii::scan_text(&rels.join("\n"), Some(&terms), "deploy", &[], false, true, None)?.iter().map(|h| fmt_hit("path", h)).collect();
    if !ph.is_empty() {
        return core::fail(
            "HINT_PAYLOAD_PATH_PII",
            format!("페이로드 경로 이름 PII: {}", ph.join("; ")),
            Some("산출물 파일 이름에서 호스트·주소·운영자 경로를 걷어낸다(artifacts 수집기가 이름을 짓는다)."),
        );
    }
    let mh: Vec<String> = pii::scan_text(message, Some(&terms), "deploy", &[], false, true, None)?.iter().map(|h| fmt_hit("message", h)).collect();
    if !mh.is_empty() {
        return core::fail("HINT_COMMIT_MESSAGE_PII", format!("커밋 메시지 PII: {}", mh.join("; ")), None);
    }
    let ih = identity_pii_hits(&terms, None)?;
    if !ih.is_empty() {
        return core::fail("HINT_COMMIT_IDENTITY_PII", format!("합성 신원이 PII 에 걸린다: {}", ih.join("; ")), None);
    }
    let parent = match guide {
        Some(g) => {
            let parent = require_guide(repo, g, "지정 안내 커밋")?;
            if let Some(r) = remote {
                let (live, _f) = guide_commit(repo, r, None)?;
                if live != parent {
                    return core::fail(
                        "HINT_BRANCH_GUIDE_MOVED",
                        format!(
                            "지정 안내 {} ≠ 원격 {} 의 현재 안내 {} — 그 사이 형식 전환이 있었다.",
                            core::head_chars(&parent, 12),
                            crate::value::py_repr(r),
                            core::head_chars(&live, 12)
                        ),
                        Some("새 안내로 다시 발행한다(draft state 의 안내를 갱신)."),
                    );
                }
            }
            parent
        }
        None => guide_commit(repo, remote.unwrap_or(""), None)?.0,
    };
    let tree = build_tree(repo, &root, &rels)?;
    let want_epoch = core::parse_utc(&utc)?.epoch();
    let env = identity_env_pairs(&utc)?;
    let commit = core::git_full(repo, &["-c", "i18n.commitEncoding=UTF-8", "commit-tree", &tree, "--no-gpg-sign", "-p", &parent], true, Some(message), &env)?
        .stdout
        .trim()
        .to_string();
    if !commit_matches(repo, &commit, &tree, &parent, message, want_epoch)? {
        return core::fail(
            "HINT_COMMIT_RESULT_MISMATCH",
            format!("지은 커밋 {} 이 요청(트리·부모=안내·메시지·합성 신원·주입 시각)과 다르다.", core::head_chars(&commit, 12)),
            None,
        );
    }
    _log(&format!(
        "페이로드 커밋 {} ({tag} · {} 파일 · 부모 = 안내 {} · 브랜치 불이동 — 태그만 가리킨다)",
        core::head_chars(&commit, 12),
        rels.len(),
        core::head_chars(&parent, 12)
    ))?;
    Ok(commit)
}

// ── 읽기 · 앵커 게이트 ─────────────────────────────────────────────────────────────────────────
/// `<rev>:<path>` blob 바이트. 없거나 blob 이 아니면 None.
pub fn read_blob_bytes(repo: &Path, rev: &str, path: &str) -> HR<Option<Vec<u8>>> {
    core::git_bytes(repo, &["cat-file", "blob", &format!("{rev}:{path}")], false)
}

/// [`read_blob_bytes`] as `{"__bytes__": hex}`.
pub fn read_blob(repo: &Path, rev: &str, path: &str) -> HR<Option<J>> {
    Ok(read_blob_bytes(repo, rev, path)?.map(|b| bytes_j(&b)))
}

/// (문서, 오류). 부재 = (None, "absent") · 깨짐 = (None, "unreadable: …").
pub fn read_json_blob(repo: &Path, rev: &str, path: &str) -> HR<(Option<J>, Option<String>)> {
    let Some(raw) = read_blob_bytes(repo, rev, path)? else {
        return Ok((None, Some("absent".into())));
    };
    match json_of_bytes(&raw) {
        Err(e) => Ok((None, Some(format!("unreadable: {e}")))),
        Ok(d) if !d.is_dict() => Ok((None, Some("unreadable: JSON 객체가 아니다".into()))),
        Ok(d) => Ok((Some(d), None)),
    }
}

/// 계약 §6 집행 — 태그는 페이로드 커밋을 가리키고, 그 커밋의 부모는 안내 커밋이다.
pub fn require_payload_anchor(repo: &Path, tag: &str, anchor: &str) -> HR<()> {
    let mut commit = String::new();
    if !anchor.is_empty() {
        commit = py_strip(&core::git_nocheck(repo, &["rev-parse", "--verify", "--quiet", &format!("{anchor}^{{commit}}")])?.stdout).to_string();
    }
    if commit.is_empty() {
        return core::fail("HINT_ANCHOR_UNRESOLVED", format!("앵커를 커밋으로 해소하지 못했다: {}", crate::value::py_repr(anchor)), None);
    }
    let c12 = core::head_chars(&commit, 12);
    let parents = parents_of(&read_object_text(repo, "commit", &commit)?.unwrap_or_default());
    let why = if parents.len() != 1 {
        Some(format!("부모가 {}개다(페이로드 커밋의 부모는 안내 커밋 하나)", parents.len()))
    } else {
        guide_info(repo, &parents[0])?.1
    };
    if let Some(why) = why.filter(|w| !w.is_empty()) {
        return core::fail(
            "HINT_ANCHOR_PARENT_NOT_GUIDE",
            format!(
                "앵커 {c12} 의 부모가 안내 커밋이 아니다 — {why}. 계약 §6: 태그는 안내 커밋 위의 페이로드 커밋을 \
                 가리킨다. 소스 트리 커밋에 봉인하면 zip 에 산출물 대신 저장소 소스가 담긴다(2026-09-07 실증: native 3종)."
            ),
            Some("hint.sh continue 로 페이로드 커밋부터 만든다(부모 = 원격 hint tip 의 안내 커밋 · 봉인은 그 커밋에만 한다)."),
        );
    }
    if read_blob_bytes(repo, &commit, PAYLOAD_JSON)?.is_none() {
        return core::fail("HINT_ANCHOR_PAYLOAD_ABSENT", format!("앵커 {c12} 트리에 PAYLOAD.json 이 없다 — 페이로드 커밋이 아니다."), None);
    }
    let (prov, err) = read_json_blob(repo, &commit, PROVENANCE_JSON)?;
    if err.as_deref() == Some("absent") {
        return core::fail(
            "HINT_ANCHOR_PROVENANCE_ABSENT",
            format!("앵커 {c12} 트리에 PROVENANCE.json 이 없다 — 어느 태그를 위한 페이로드인지 말하지 않는다."),
            None,
        );
    }
    let Some(prov) = prov else {
        return core::fail("HINT_ANCHOR_PROVENANCE_UNREADABLE", format!("앵커 {c12} PROVENANCE.json {}", err.unwrap_or_default()), None);
    };
    let pt = prov.get("tag").cloned().unwrap_or(J::Null);
    if pt != J::from(tag) {
        return core::fail(
            "HINT_ANCHOR_PROVENANCE_TAG_MISMATCH",
            format!("앵커의 PROVENANCE.tag={} 인데 봉인하려는 태그는 {} 이다 — 남의 페이로드다.", pt.repr(), crate::value::py_repr(tag)),
            None,
        );
    }
    Ok(())
}

/// blob SHA 여럿 → {sha: bytes}(`cat-file --batch` 한 번).
pub fn read_blobs(repo: &Path, shas: &[String]) -> HR<BTreeMap<String, Vec<u8>>> {
    let uniq: BTreeSet<&String> = shas.iter().collect();
    let mut out = BTreeMap::new();
    if uniq.is_empty() {
        return Ok(out);
    }
    let input: String = uniq.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n") + "\n";
    let r = run_git_bytes(repo, &["cat-file", "--batch"], input.as_bytes()).map_err(|e| HintError::py("FileNotFoundError", e))?;
    if r.returncode != 0 {
        return core::fail("HINT_GIT_FAILED", format!("git cat-file --batch 실패(rc={}): {}", r.returncode, py_strip(&lossy(&r.stderr))), None);
    }
    let data = &r.stdout;
    let mut pos = 0usize;
    for want in uniq {
        let nl = data[pos.min(data.len())..].iter().position(|b| *b == b'\n').map(|i| i + pos);
        let head: Vec<String> = match nl {
            Some(n) => lossy(&data[pos..n]).split_whitespace().map(str::to_string).collect(),
            None => vec![],
        };
        if head.len() != 3 || &head[0] != want || head[1] != "blob" || head[2].is_empty() || !head[2].chars().all(|c| c.is_ascii_digit()) {
            let shown = if head.is_empty() { "출력 없음".to_string() } else { head.join(" ") };
            return core::fail("HINT_TREE_READ_FAILED", format!("blob 을 읽을 수 없다: {want} → {shown}"), None);
        }
        let n = nl.unwrap();
        let size: usize = head[2].parse().unwrap_or(0);
        let end = (n + 1 + size).min(data.len());
        out.insert(want.clone(), data[n + 1..end].to_vec());
        pos = n + 1 + size + 1;
    }
    Ok(out)
}

/// `<rev>` 트리를 빈 디렉터리 `dest` 에 파일로 풀어 쓴다. 반환 = 쓴 상대경로(정렬).
pub fn export_tree(repo: &Path, rev: &str, dest: &Path) -> HR<Vec<String>> {
    let root = dest;
    if root.exists() && (!root.is_dir() || std::fs::read_dir(root).map(|mut d| d.next().is_some()).unwrap_or(false)) {
        return core::fail("HINT_EXPORT_DEST_NOT_EMPTY", format!("풀어 쓸 자리가 비어 있지 않다: {}", core::posix(root)), None);
    }
    let ents = entries(repo, rev)?;
    let bad: Vec<String> = ents.iter().filter(|e| e.typ != "blob" || !(e.mode == "100644" || e.mode == "100755")).map(|e| e.path.clone()).collect();
    if !bad.is_empty() {
        return core::fail("HINT_EXPORT_ENTRY_UNSUPPORTED", format!("블롭(100644·100755)이 아닌 항목: {}", strs_repr(&bad[..bad.len().min(10)])), None);
    }
    let outside: Vec<String> = ents.iter().filter_map(|e| allow_why(&e.path, true).map(|w| format!("{} ({w})", e.path))).collect();
    if !outside.is_empty() {
        return core::fail(
            "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST",
            format!("allowlist 밖 경로를 풀어 쓰지 않는다: {}", strs_repr(&outside[..outside.len().min(10)])),
            None,
        );
    }
    let blobs = read_blobs(repo, &ents.iter().map(|e| e.sha.clone()).collect::<Vec<_>>())?;
    std::fs::create_dir_all(root).map_err(|e| core::io_err(&e, root))?;
    for e in &ents {
        let out = pjoin(root, &e.path);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|er| core::io_err(&er, p))?;
        }
        std::fs::write(&out, &blobs[&e.sha]).map_err(|er| core::io_err(&er, &out))?;
        if e.mode == "100755" {
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755)).map_err(|er| core::io_err(&er, &out))?;
        }
    }
    let mut v: Vec<String> = ents.into_iter().map(|e| e.path).collect();
    v.sort();
    Ok(v)
}

/// 이미 있는 페이로드 커밋 하나가 배포 계약을 지키는가. `CODE: 설명` 정렬 목록(빈 = 계약 준수).
pub fn commit_violations(repo: &Path, commit: &str, tag: &str, terms: &[String]) -> HR<Vec<String>> {
    let mut probs: Vec<String> = Vec::new();
    let Some(raw) = read_object_text(repo, "commit", commit)? else {
        return Ok(vec![format!("HINT_COMMIT_UNREADABLE: 커밋 오브젝트를 읽을 수 없다: {commit}")]);
    };
    probs.extend(tree_violations(repo, commit)?);
    let ids = parse_ident_headers(&raw)?;
    let me = synthetic_identity()?;
    for role in ["author", "committer"] {
        let who = ident_of(&ids, role);
        if who.as_deref() != Some(me.as_str()) {
            probs.push(format!(
                "HINT_COMMIT_IDENTITY_NOT_SYNTHETIC: {role} 가 합성 신원이 아니다({}) — 운영자 신원이 zip 수신자에게 배포된다(K1).",
                if who.is_none() { "부재" } else { "다른 신원" }
            ));
        } else if !identity_pii_hits(terms, who.as_deref())?.is_empty() {
            probs.push(format!("HINT_COMMIT_IDENTITY_PII: {role} 신원이 PII 목록에 걸린다"));
        }
    }
    let message = raw.split_once("\n\n").map(|x| x.1).unwrap_or("").to_string();
    let mh = pii::scan_text(&message, Some(terms), "deploy", &[], false, true, None)?;
    if !mh.is_empty() {
        probs.push(format!("HINT_COMMIT_MESSAGE_PII: {}", pii::render_hits(&mh, 5)?));
    }
    let all = entries(repo, commit)?;
    let mut all_paths: Vec<String> = all.iter().map(|e| e.path.clone()).collect();
    all_paths.sort();
    let ents: Vec<&TreeEnt> = all.iter().filter(|e| e.typ == "blob" && e.mode != "120000").collect();
    let blobs = read_blobs(repo, &ents.iter().map(|e| e.sha.clone()).collect::<Vec<_>>())?;
    let mut by_path: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for e in &ents {
        by_path.insert(e.path.clone(), blobs[&e.sha].clone());
    }
    let js = |p: &str| -> J { by_path.get(p).and_then(|d| json_of_bytes(d).ok()).unwrap_or(J::Null) };
    let docs: Vec<(String, Vec<u8>)> = PAYLOAD_DOCS.iter().filter_map(|n| by_path.get(*n).map(|d| (n.to_string(), d.clone()))).collect();
    if by_path.contains_key(PAYLOAD_JSON) && by_path.contains_key(PROVENANCE_JSON) {
        probs.extend(payload_doc_problems(&js(PAYLOAD_JSON), &js(PROVENANCE_JSON), &J::from(all_paths.clone()), &J::from(message), &docs_j(&docs), Some(tag))?);
    } else {
        probs.extend(doc_residue_problems(&docs_j(&docs))?);
    }
    let files: Vec<(String, Vec<u8>)> = by_path.into_iter().collect();
    let hits = scan_blobs(&files, terms, None)?;
    if !hits.is_empty() {
        probs.push(format!("HINT_PAYLOAD_PII: {}건 — {}", hits.len(), hits.iter().take(10).cloned().collect::<Vec<_>>().join("; ")));
    }
    let ph = pii::scan_text(&all_paths.join("\n"), Some(terms), "deploy", &[], false, true, None)?;
    if !ph.is_empty() {
        probs.push(format!("HINT_PAYLOAD_PATH_PII: {}", ph.iter().take(10).map(|h| fmt_hit("path", h)).collect::<Vec<_>>().join("; ")));
    }
    let set: BTreeSet<String> = probs.into_iter().collect();
    Ok(set.into_iter().collect())
}

// ── 자체검사 픽스처 (tag.selftest 공유) ─────────────────────────────────────────────────────────
pub const FIXTURE_TERM: &str = "fixture-pii-term-zeta";
pub const _FX_OPERATOR_NAME: &str = "Operator Realname";
/// `"@".join(["operator.realname", "corp.example"])`.
pub fn _FX_OPERATOR_EMAIL() -> String {
    ["operator.realname", "corp.example"].join("@")
}
pub const _FX_UTC: &str = "2026-09-21T10:21:01Z";
pub const _FX_TAG: &str = "hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-mmp-eager";
pub const _SCRUB_ENV: [&str; 19] = ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_COMMON_DIR", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_PREFIX", "GIT_NAMESPACE", "GIT_CONFIG", "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS", "GITHUB_TOKEN", "HINT_PUSH_TOKEN", "GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE"];

/// `selftest_env` context: the process env is restored when this is dropped.
pub struct SelftestEnv {
    saved: Vec<(String, Option<OsString>)>,
}
impl Drop for SelftestEnv {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
}

/// 자체검사 격리: 운영자 전역·시스템 git 설정과 토큰 env 를 끊는다(guard 가 drop 되면 원복). extra = 일부러 심을 env.
pub fn selftest_env(tmp: &Path, extra: &[(String, String)]) -> HR<SelftestEnv> {
    let gcfg = tmp.join("gitconfig");
    let text = format!(
        "[init]\n\tdefaultBranch = main\n[commit]\n\tgpgSign = false\n[tag]\n\tgpgSign = false\n[core]\n\thooksPath = {}\n",
        core::posix(&tmp.join("no-hooks"))
    );
    std::fs::write(&gcfg, text).map_err(|e| core::io_err(&e, &gcfg))?;
    let mut keys: BTreeSet<String> = _SCRUB_ENV.iter().map(|s| s.to_string()).collect();
    keys.insert("GIT_CONFIG_GLOBAL".into());
    keys.insert("GIT_CONFIG_NOSYSTEM".into());
    for (k, _) in extra {
        keys.insert(k.clone());
    }
    let saved = keys.into_iter().map(|k| {
        let v = std::env::var_os(&k);
        (k, v)
    }).collect();
    for k in _SCRUB_ENV {
        std::env::remove_var(k);
    }
    std::env::set_var("GIT_CONFIG_GLOBAL", &gcfg);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    for (k, v) in extra {
        std::env::set_var(k, v);
    }
    Ok(SelftestEnv { saved })
}

fn write_text(p: &Path, text: &str) -> HR<()> {
    std::fs::write(p, text).map_err(|e| core::io_err(&e, p))
}
fn mkdirs(p: &Path) -> HR<()> {
    std::fs::create_dir_all(p).map_err(|e| core::io_err(&e, p))
}

/// 격리 저장소: main 에 추적 파일 1 커밋(운영자 신원 설정) + 비추적 픽스처 pii_terms. (Python 기본 name="repo")
pub fn selftest_repo(tmp: &Path, name: &str) -> HR<PathBuf> {
    let repo = tmp.join(name);
    if repo.exists() {
        return Err(HintError::py("FileExistsError", format!("[Errno 17] File exists: {}", crate::value::py_repr(&core::posix(&repo)))));
    }
    mkdirs(&repo)?;
    core::git(&repo, &["init", "-q", "-b", "main"])?;
    core::git(&repo, &["config", "user.name", _FX_OPERATOR_NAME])?;
    core::git(&repo, &["config", "user.email", &_FX_OPERATOR_EMAIL()])?;
    write_text(&repo.join("src.txt"), "source\n")?;
    core::git(&repo, &["add", "src.txt"])?;
    core::git(&repo, &["commit", "-qm", "source"])?;
    let terms = repo.join(core::REL_PII_TERMS);
    mkdirs(terms.parent().unwrap())?;
    write_text(&terms, &format!("# fixture\n{FIXTURE_TERM}\n"))?;
    Ok(repo)
}

pub const FX_GUIDE_BODY: &str = "# hint 브랜치 안내(픽스처)\n\n이 브랜치는 hint 태그 페이로드 전용이다.\n";

/// 자체검사용 안내 커밋 오브젝트(ref 이동 ✗ · 결정론). fmt=None = 마커 없는 옛 안내.
/// (Python 기본: fmt=BRANCH_FORMAT, parent=None, body=FX_GUIDE_BODY, utc=None)
pub fn selftest_guide(repo: &Path, fmt: Option<&str>, parent: Option<&str>, body: &str, utc: Option<&str>) -> HR<String> {
    let readme = match fmt.filter(|f| !f.is_empty()) {
        Some(f) => format!("{}\n{body}", guide_marker(f)?),
        None => body.to_string(),
    };
    let blob = core::git_full(repo, &["hash-object", "-w", "--no-filters", "--stdin"], true, Some(&readme), &[])?.stdout.trim().to_string();
    let tree = _mktree(repo, &[("100644".into(), "blob".into(), blob, GUIDE_README.into())])?;
    let mut args = vec!["-c", "i18n.commitEncoding=UTF-8", "commit-tree", tree.as_str(), "--no-gpg-sign"];
    if let Some(p) = parent.filter(|p| !p.is_empty()) {
        args.extend(["-p", p]);
    }
    let env = identity_env_pairs(utc.filter(|u| !u.is_empty()).unwrap_or(_FX_UTC))?;
    Ok(core::git_full(repo, &args, true, Some("hint: guide (fixture)\n"), &env)?.stdout.trim().to_string())
}

pub const FX_BRIEF: &str = "픽스처 셀의 요약 문단이다 — 지도이지 정답이 아니다.";

/// 자체검사용 PAYLOAD.json(v7 · naming 재조립 검사를 통과하는 모양).
pub fn fixture_payload_doc(tag: &str) -> HR<J> {
    let t = naming::parse_tag(tag)?;
    let arch = naming::parse_arch(&t.arch, None)?.unwrap_or_else(J::dict);
    let sp = naming::split_recipe(&t.recipe)?.unwrap_or_else(J::dict);
    let mut axes = Map::new();
    for k in naming::ARCH_AXES {
        axes.insert(k, jd! {"value" => arch.get(k).map(J::display).unwrap_or_default(), "source" => "fixture:manifest"});
    }
    for k in naming::RECIPE_AXES {
        axes.insert(k, jd! {"value" => sp.get(k).map(J::display).unwrap_or_default(), "source" => "fixture:lockset"});
    }
    let recipe_v = sp.get("base").cloned().unwrap_or_else(|| J::from(t.recipe.clone()));
    let doc = jd! {
        "grammar" => naming::GRAMMAR_V7,
        "segments" => jd!{
            "vllm" => jd!{"value" => t.vllm.clone(), "source" => "fixture:vllm_build_input"},
            "model" => jd!{"value" => t.model.clone(), "source" => "fixture:hf_repo"},
            "arch" => jd!{"value" => t.arch.clone(), "source" => "derived"},
            "recipe" => jd!{"value" => recipe_v, "source" => "derived(q·len·kv)"}},
        "axes" => J::Dict(axes),
        "vllm_build_input" => jd!{"kind" => "release", "ref" => format!("v{}", t.vllm), "sha" => J::Null, "prev_release" => J::Null}
    };
    if t.base_tag().is_none() {
        return Ok(jd! {"schema_version" => 2i64, "format" => PAYLOAD_FORMAT, "tag" => tag, "generated_utc" => _FX_UTC, "naming" => doc});
    }
    let tail: Vec<J> = match sp.get("tail") {
        Some(J::List(l)) if !l.is_empty() => {
            l.iter().map(|tok| jd! {"token" => tok.clone(), "meaning" => format!("픽스처 꼬리 {}", tok.display()), "evidence" => J::Null}).collect()
        }
        _ => vec![],
    };
    let ts = sp.get("timestamp").and_then(J::as_str).map(str::to_string);
    let nm = naming::apply_tail(&doc, &J::List(tail), ts.as_deref())?;
    Ok(jd! {"schema_version" => 2i64, "format" => PAYLOAD_FORMAT, "tag" => tag, "generated_utc" => _FX_UTC, "naming" => nm})
}

/// 00-hint.md 픽스처. (Python 기본 brief=FX_BRIEF)
pub fn fixture_hint_doc(brief: &str) -> HR<String> {
    Ok(format!("# hint\n\n## 0.1 요약\n\n{brief}\n\n## 0.2 유효맥락\n\n픽스처 본문.\n"))
}

fn rglob_files(root: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let is_link = e.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        if p.is_dir() {
            if !is_link {
                rglob_files(root, &p, out);
            }
        } else if p.is_file() {
            out.insert(core::posix(p.strip_prefix(root).unwrap_or(&p)));
        }
    }
}

/// v6 최상위 8종 + 슬롯 산출물(실행비트·4단 중첩·compose 셸 기본값) 픽스처.
/// (Python 기본: extra_files=None, payload_doc=None, brief=FX_BRIEF)
pub fn write_fixture_payload(root: &Path, tag: &str, source_anchor: Option<&str>, extra_files: Option<&[(String, String)]>, payload_doc: Option<&J>, brief: &str) -> HR<()> {
    if root.exists() {
        std::fs::remove_dir_all(root).map_err(|e| core::io_err(&e, root))?;
    }
    mkdirs(root)?;
    let shell_default = format!("${{NAS_MODEL_PATH:-{}}}", ["", "mnt", "models"].join("/"));
    let mut files: Vec<(String, String)> = vec![
        ("README.md".into(), "# hint payload\n".into()),
        ("00-hint.md".into(), fixture_hint_doc(brief)?),
        ("01-artifacts.md".into(), "## 1.1 적용 판정\n".into()),
        ("02-narrative.md".into(), "## 2.1 출발점\n".into()),
        ("03-benchmark.md".into(), "## 3 측정\n".into()),
        ("LINEAGE.json".into(), core::dumps(&jd! {"schema_version" => 1i64, "documents" => J::list()})),
        ("artifacts/triplet/model.yaml".into(), "max-model-len: 262144\n".into()),
        ("artifacts/compose/docker-compose.yaml".into(), format!("volumes:\n  - {shell_default}:/models:ro\n")),
        ("artifacts/build_recipe/a/b/c/deep.txt".into(), "4단 중첩\n".into()),
    ];
    for (k, v) in extra_files.unwrap_or(&[]) {
        match files.iter_mut().find(|(n, _)| n == k) {
            Some(slot) => slot.1 = v.clone(),
            None => files.push((k.clone(), v.clone())),
        }
    }
    for (rel, text) in &files {
        let p = pjoin(root, rel);
        mkdirs(p.parent().unwrap())?;
        write_text(&p, text)?;
    }
    let runner = root.join("artifacts/build_patch_post/30-fixture.sh");
    mkdirs(runner.parent().unwrap())?;
    write_text(&runner, "#!/bin/sh\necho patch\n")?;
    std::fs::set_permissions(&runner, std::fs::Permissions::from_mode(0o755)).map_err(|e| core::io_err(&e, &runner))?;
    let pd = match payload_doc {
        Some(d) => d.clone(),
        None => fixture_payload_doc(tag)?,
    };
    core::write_json(&root.join(PAYLOAD_JSON), &pd)?;
    let mut rels = BTreeSet::new();
    rglob_files(root, root, &mut rels);
    rels.insert(PROVENANCE_JSON.to_string());
    let rels: Vec<String> = rels.into_iter().collect();
    core::write_json(
        &root.join(PROVENANCE_JSON),
        &jd! {"schema_version" => 2i64, "tag" => tag, "source_anchor" => source_anchor, "source_anchor_is_head" => true,
              "assembly_branch" => "main", "payload_files" => rels, "generated_utc" => _FX_UTC},
    )
}

/// f() 이 낸 HintError 의 code(성공 = None).
pub fn code_of<T>(f: impl FnOnce() -> HR<T>) -> Option<String> {
    f().err().map(|e| e.code)
}

pub fn expect_code<T>(f: impl FnOnce() -> HR<T>, code: &str) -> bool {
    code_of(f).as_deref() == Some(code)
}

// ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
struct Ck(Vec<String>);
impl Ck {
    fn ck(&mut self, name: &str, cond: bool) {
        if !cond {
            self.0.push(format!("branch: {name}"));
        }
    }
}

fn op_env() -> Vec<(String, String)> {
    vec![
        ("GIT_AUTHOR_NAME".into(), _FX_OPERATOR_NAME.into()),
        ("GIT_AUTHOR_EMAIL".into(), _FX_OPERATOR_EMAIL()),
        ("GIT_COMMITTER_NAME".into(), _FX_OPERATOR_NAME.into()),
        ("GIT_COMMITTER_EMAIL".into(), _FX_OPERATOR_EMAIL()),
    ]
}

/// 실패 메시지 목록(빈 목록 = 통과). 격리 임시 저장소만 쓴다.
pub fn selftest() -> Vec<String> {
    let mut c = Ck(Vec::new());
    let av = |r: &str| allow_why(r, true);
    c.ck("allowlist 최상위 허용", av("00-hint.md").is_none());
    c.ck("allowlist 슬롯 허용", av("artifacts/compose/sub_recipe.json").is_none());
    c.ck("allowlist 슬롯 안 숨김파일 허용(env 형상)", av("artifacts/triplet/.env.m").is_none());
    c.ck("★allowlist 밖 최상위 차단", av("notes.txt").is_some());
    c.ck("★allowlist 미지 슬롯 차단", av("artifacts/unknown/x").is_some());
    c.ck("★allowlist 슬롯 디렉터리 자체 파일 차단", av("artifacts/triplet").is_some());
    c.ck("★.claude 차단(A1)", av(".claude/x.md").is_some());
    c.ck("★중첩 .git 도 차단(옛 FORBIDDEN_TOP 은 최상위만 봤다)", av("artifacts/triplet/.git/config").is_some());
    c.ck("★상위탈출·절대경로 차단", av("../x").is_some() && av("/x").is_some());
    c.ck(
        "★슬롯 안 .gitattributes·.gitmodules 차단(archive 가 export-ignore 로 zip ≠ 검증 트리)",
        av("artifacts/compose/.gitattributes").is_some() && av("artifacts/triplet/.gitmodules").is_some(),
    );
    let sur = PyName::from_bytes(b"artifacts/triplet/x\xff.yaml");
    c.ck("★UTF-8 로 표현할 수 없는 이름 차단(surrogate)", allow_why(&sur.text, sur.utf8).is_some());
    c.ck("합성 신원 = core 상수", synthetic_identity().ok() == Some(format!("{} <{}>", core::SYNTHETIC_NAME, core::synthetic_email())));
    c.ck(
        "신원 헤더 파서",
        parse_ident_headers("tree t\nauthor A B <c> 10 +0000\n\nmsg").map(|j| j["author"].get_str("ident") == Some("A B <c>")).unwrap_or(false),
    );
    c.ck("PAYLOAD_SLOT_DIRS == artifacts.SLOTS", crate::artifacts::SLOTS == PAYLOAD_SLOT_DIRS);

    let td = core::TempDir::new("hint-branch-selftest-");
    let tmp = td.path().to_path_buf();
    let ((), _o, _e) = crate::io::capture(|| {
        let r = (|| -> HR<()> {
            let _g = selftest_env(&tmp, &[])?;
            _selftest_git(&tmp, &mut c)?;
            _transition_selftest(&tmp, &mut c)?;
            Ok(())
        })();
        if let Err(e) = r {
            if e.is_py() {
                c.0.push(format!("branch: 자체검사 중 예외 {}: {}", e.code.trim_start_matches("py:"), core::head_chars(&e.message, 300)));
            } else {
                c.0.push(format!("branch: 자체검사 중 예기치 않은 HintError {}: {}", e.code, core::head_chars(&e.message, 300)));
            }
        }
    });
    crate::io::out(&_o);
    c.0
}

fn first_tree(repo: &Path, commit: &str) -> HR<String> {
    let raw = read_object_text(repo, "commit", commit)?.unwrap_or_default();
    Ok(raw.split('\n').next().unwrap_or("").chars().skip(5).collect())
}
fn parents_c(repo: &Path, commit: &str) -> HR<Vec<String>> {
    Ok(parents_of(&read_object_text(repo, "commit", commit)?.unwrap_or_default()))
}
fn refs_snapshot(repo: &Path) -> HR<String> {
    core::git_out(repo, &["for-each-ref", "--format=%(refname) %(objectname)"])
}
fn commit_tree_raw(repo: &Path, args: &[&str], msg: &str, env: &[(String, String)]) -> HR<String> {
    let mut a = vec!["commit-tree"];
    a.extend_from_slice(args);
    Ok(core::git_full(repo, &a, true, Some(msg), env)?.stdout.trim().to_string())
}
fn exf(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}
fn set_key(doc: &J, k: &str, v: J) -> J {
    let mut d = doc.clone();
    d.set(k, v);
    d
}

fn _selftest_git(tmp: &Path, c: &mut Ck) -> HR<()> {
    let repo = selftest_repo(tmp, "repo")?;
    let repo = repo.as_path();
    let source = core::git_out(repo, &["rev-parse", "HEAD"])?;
    let pay = tmp.join("payload");
    let pay = pay.as_path();
    write_fixture_payload(pay, _FX_TAG, Some(&source), None, None, FX_BRIEF)?;
    let rels = payload_files(pay)?;
    let mut sorted = rels.clone();
    sorted.sort();
    c.ck("payload_files = 최상위 8 + 슬롯 4", rels.len() == 12 && rels == sorted);

    write_text(&pay.join("notes.txt"), "x\n")?;
    c.ck("★allowlist 밖 파일 = HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST", expect_code(|| payload_files(pay), "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"));
    let _ = std::fs::remove_file(pay.join("notes.txt"));
    mkdirs(&pay.join("artifacts/unknown"))?;
    write_text(&pay.join("artifacts/unknown/x.sh"), "x\n")?;
    c.ck("★미지 슬롯 = HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST", expect_code(|| payload_files(pay), "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"));
    let _ = std::fs::remove_dir_all(pay.join("artifacts/unknown"));
    std::os::unix::fs::symlink(repo.join("src.txt"), pay.join("artifacts/triplet/link.yaml")).map_err(|e| core::io_err(&e, pay))?;
    c.ck("★심링크 = HINT_PAYLOAD_SYMLINK", expect_code(|| payload_files(pay), "HINT_PAYLOAD_SYMLINK"));
    let _ = std::fs::remove_file(pay.join("artifacts/triplet/link.yaml"));
    let lin = std::fs::read(pay.join("LINEAGE.json")).unwrap_or_default();
    let _ = std::fs::remove_file(pay.join("LINEAGE.json"));
    c.ck("★필수 부재 = HINT_PAYLOAD_REQUIRED_ABSENT", expect_code(|| payload_files(pay), "HINT_PAYLOAD_REQUIRED_ABSENT"));
    let _ = std::fs::write(pay.join("LINEAGE.json"), &lin);
    let b03 = std::fs::read(pay.join("03-benchmark.md")).unwrap_or_default();
    let pj = std::fs::read(pay.join(PAYLOAD_JSON)).unwrap_or_default();
    let _ = std::fs::remove_file(pay.join("03-benchmark.md"));
    c.ck("★등급 미기재 페이로드의 03 부재 = HINT_PAYLOAD_REQUIRED_ABSENT(모름 = full 필수)", expect_code(|| payload_files(pay), "HINT_PAYLOAD_REQUIRED_ABSENT"));
    let pjd = json_of_bytes(&pj).unwrap_or(J::Null);
    core::write_json(&pay.join(PAYLOAD_JSON), &set_key(&pjd, "cell_grade", "lite".into()))?;
    let full: Vec<String> = PAYLOAD_REQUIRED_TOP.iter().map(|s| s.to_string()).collect();
    let no03: Vec<String> = full.iter().filter(|t| *t != "03-benchmark.md").cloned().collect();
    c.ck(
        "lite 셀(cell_grade=lite)은 03 없이 통과 · allowlist 는 03 을 계속 허용",
        payload_files(pay).map(|r| !r.contains(&"03-benchmark.md".to_string())).unwrap_or(false)
            && PAYLOAD_ALLOWLIST_TOP.contains(&"03-benchmark.md")
            && required_top(&jd! {"cell_grade" => "lite"})? == no03
            && required_top(&J::Null)? == full
            && required_top(&jd! {"cell_grade" => "full"})? == full,
    );
    let _ = std::fs::write(pay.join(PAYLOAD_JSON), &pj);
    let _ = std::fs::write(pay.join("03-benchmark.md"), &b03);

    let msg = commit_message(pay, _FX_UTC)?;
    c.ck("파생 커밋 메시지에 태그·소스 앵커", msg.contains(_FX_TAG) && msg.contains(&source));

    let plus = |x: &str| {
        let mut v = rels.clone();
        v.push(x.to_string());
        v
    };
    c.ck("★build_tree 실물 부재 = HINT_PAYLOAD_FILE_ABSENT", expect_code(|| build_tree(repo, pay, &plus("artifacts/triplet/nope.yaml")), "HINT_PAYLOAD_FILE_ABSENT"));
    c.ck("★build_tree 중복 경로 = HINT_TREE_BUILD_FAILED", expect_code(|| build_tree(repo, pay, &plus(&rels[0])), "HINT_TREE_BUILD_FAILED"));
    c.ck("★build_tree allowlist 밖 = HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST", expect_code(|| build_tree(repo, pay, &plus("../src.txt")), "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"));

    let guide = selftest_guide(repo, Some(BRANCH_FORMAT), None, FX_GUIDE_BODY, None)?;
    let old_guide = selftest_guide(repo, None, None, FX_GUIDE_BODY, None)?;
    let v6_guide = selftest_guide(repo, Some("v6"), None, FX_GUIDE_BODY, None)?;
    let mk = guide_marker(BRANCH_FORMAT)?;
    c.ck(
        "readme_format: 마커 첫 줄 = v7 · 마커 없음 = None · 둘째 줄 마커는 무효",
        readme_format_text(&format!("{mk}\n# x\n")).as_deref() == Some("v7")
            && readme_format_text("# x\n").is_none()
            && readme_format_text(&format!("# x\n{mk}\n")).is_none(),
    );
    c.ck("guide_info(v7 안내) = (v7, None)", guide_info(repo, &guide)? == (Some("v7".into()), None));
    let gi_old = guide_info(repo, &old_guide)?;
    c.ck("★마커 없는 옛 안내 = 안내 아님(사유에 '마커')", gi_old.0.is_none() && gi_old.1.unwrap_or_default().contains("마커"));
    c.ck("★소스 커밋(트리가 README 하나가 아님) = 안내 아님", guide_info(repo, &source)?.0.is_none());
    c.ck("★require_guide: 마커 없는 옛 안내 = HINT_BRANCH_FORMAT_MISMATCH", expect_code(|| require_guide(repo, &old_guide, "안내 커밋"), "HINT_BRANCH_FORMAT_MISMATCH"));
    c.ck("★require_guide: 다른 형식(v6 마커) = HINT_BRANCH_FORMAT_MISMATCH", expect_code(|| require_guide(repo, &v6_guide, "안내 커밋"), "HINT_BRANCH_FORMAT_MISMATCH"));
    c.ck("guide_commit(pinned) = (sha, v7) · 네트워크 0", guide_commit(repo, "unused", Some(&guide))? == (guide.clone(), "v7".into()));
    c.ck("★부모 미지정 = HINT_BRANCH_GUIDE_REQUIRED", expect_code(|| commit_payload(repo, pay, &msg, _FX_UTC, None, None), "HINT_BRANCH_GUIDE_REQUIRED"));
    c.ck("★마커 없는 옛 안내 위에는 얹지 않는다 = HINT_BRANCH_FORMAT_MISMATCH", expect_code(|| commit_payload(repo, pay, &msg, _FX_UTC, Some(&old_guide), None), "HINT_BRANCH_FORMAT_MISMATCH"));
    c.ck("★소스 커밋을 안내로 = HINT_BRANCH_FORMAT_MISMATCH", expect_code(|| commit_payload(repo, pay, &msg, _FX_UTC, Some(&source), None), "HINT_BRANCH_FORMAT_MISMATCH"));
    c.ck("거부들 뒤 로컬 hint 부재 그대로", hint_tip(repo)?.is_none());

    let snap = || -> HR<(String, String, String, String)> {
        let idx = std::fs::read(repo.join(".git/index")).unwrap_or_default();
        Ok((core::git_out(repo, &["status", "--porcelain"])?, core::git_out(repo, &["rev-parse", "HEAD"])?, core::sha256_bytes(&idx), core::git_out(repo, &["symbolic-ref", "HEAD"])?))
    };
    let before = snap()?;
    let c1 = {
        let _g = selftest_env(tmp, &op_env())?;
        commit_payload(repo, pay, &msg, _FX_UTC, Some(&guide), None)?
    };
    c.ck("메인 status·HEAD·인덱스·체크아웃 불변", snap()? == before);
    c.ck("★로컬 hint 는 움직이지 않는다(부재 그대로 · 페이로드 커밋은 태그만 가리킨다)", hint_tip(repo)?.is_none());
    c.ck("★부모 = 안내 커밋(하나)", parents_c(repo, &c1)? == vec![guide.clone()]);
    let ids = commit_identity(repo, &c1)?;
    let me = synthetic_identity()?;
    c.ck(
        "★author/committer = 합성 신원(운영자 config·env 무시)",
        ident_of(&ids, "author").as_deref() == Some(me.as_str()) && ident_of(&ids, "committer").as_deref() == Some(me.as_str()),
    );
    let raw1 = core::git_bytes(repo, &["cat-file", "commit", &c1], true)?.unwrap_or_default();
    let contains = |hay: &[u8], n: &[u8]| hay.windows(n.len()).any(|w| w == n);
    c.ck("★운영자 신원이 커밋 오브젝트에 없다", !contains(&raw1, _FX_OPERATOR_NAME.as_bytes()) && !contains(&raw1, _FX_OPERATOR_EMAIL().as_bytes()));
    c.ck("커밋 시각 = 주입 시각", epoch_of(&ids, "author") == Some(core::parse_utc(_FX_UTC)?.epoch()));
    c.ck("트리 = payload_files", tree_paths(repo, &c1)? == rels);
    let modes: BTreeMap<String, String> = entries(repo, &c1)?.into_iter().map(|e| (e.path, e.mode)).collect();
    c.ck("실행비트 보존 100755", modes.get("artifacts/build_patch_post/30-fixture.sh").map(String::as_str) == Some("100755"));
    c.ck("일반파일 100644", modes.get("00-hint.md").map(String::as_str) == Some("100644"));
    c.ck("★회귀 4단 중첩 보존", modes.contains_key("artifacts/build_recipe/a/b/c/deep.txt"));
    let exp = tmp.join("export");
    let got = export_tree(repo, &c1, &exp)?;
    let x_ok = std::fs::metadata(exp.join("artifacts/build_patch_post/30-fixture.sh")).map(|m| m.permissions().mode() & 0o100 != 0).unwrap_or(false);
    c.ck(
        "export_tree = 트리 전수 · 바이트 동일 · 실행비트",
        got == rels && rels.iter().all(|r| std::fs::read(exp.join(r)).ok() == std::fs::read(pay.join(r)).ok()) && x_ok,
    );
    c.ck("★export_tree 비어 있지 않은 자리 = 거부", expect_code(|| export_tree(repo, &c1, &exp), "HINT_EXPORT_DEST_NOT_EMPTY"));
    c.ck("compose 셸 기본값은 커밋을 막지 않았다", modes.contains_key("artifacts/compose/docker-compose.yaml"));
    c.ck("tree_violations 0", tree_violations(repo, &c1)?.is_empty());
    c.ck("read_blob 실재", read_blob_bytes(repo, &c1, "README.md")? == Some(b"# hint payload\n".to_vec()));
    c.ck("read_blob 부재 = None", read_blob_bytes(repo, &c1, "nope.md")?.is_none());

    let repo2 = tmp.join("repo2");
    mkdirs(&repo2)?;
    core::git(&repo2, &["init", "-q", "-b", "main"])?;
    mkdirs(repo2.join(core::REL_PII_TERMS).parent().unwrap())?;
    write_text(&repo2.join(core::REL_PII_TERMS), &format!("{FIXTURE_TERM}\n"))?;
    let guide2 = selftest_guide(&repo2, Some(BRANCH_FORMAT), None, FX_GUIDE_BODY, None)?;
    c.ck("결정론: 안내 픽스처도 저장소와 무관하게 같은 SHA", guide2 == guide);
    c.ck("★결정론: 같은 입력·주입 시각·안내 = 같은 커밋 SHA", commit_payload(&repo2, pay, &msg, _FX_UTC, Some(&guide2), None)? == c1);

    let refs_before = refs_snapshot(repo)?;
    c.ck(
        "★결정론 SHA 재개(같은 페이로드·메시지·시각·안내 = 같은 SHA · ref 불변)",
        commit_payload(repo, pay, &msg, _FX_UTC, Some(&guide), None)? == c1 && refs_snapshot(repo)? == refs_before,
    );
    let g_rev = selftest_guide(repo, Some(BRANCH_FORMAT), None, &format!("{FX_GUIDE_BODY}개정\n"), None)?;
    c.ck("★다른 안내 = 다른 SHA(부모가 SHA 에 들어간다)", commit_payload(repo, pay, &msg, _FX_UTC, Some(&g_rev), None)? != c1);

    match require_payload_anchor(repo, _FX_TAG, &c1) {
        Ok(()) => c.ck("앵커 게이트 통과", true),
        Err(e) => c.ck(&format!("앵커 게이트 통과({})", e.code), false),
    }
    c.ck("★소스 커밋 봉인 = HINT_ANCHOR_PARENT_NOT_GUIDE", expect_code(|| require_payload_anchor(repo, _FX_TAG, &source), "HINT_ANCHOR_PARENT_NOT_GUIDE"));
    let t1 = first_tree(repo, &c1)?;
    let fx_env = identity_env_pairs(_FX_UTC)?;
    let old_chain = commit_tree_raw(repo, &[&t1, "--no-gpg-sign", "-p", &old_guide], &msg, &fx_env)?;
    c.ck(
        "★마커 없는 옛 안내 위의 페이로드 커밋(v6 체인형) = HINT_ANCHOR_PARENT_NOT_GUIDE",
        expect_code(|| require_payload_anchor(repo, _FX_TAG, &old_chain), "HINT_ANCHOR_PARENT_NOT_GUIDE"),
    );
    let merge = commit_tree_raw(repo, &[&t1, "--no-gpg-sign", "-p", &guide, "-p", &source], &msg, &fx_env)?;
    c.ck("★부모 둘 = HINT_ANCHOR_PARENT_NOT_GUIDE", expect_code(|| require_payload_anchor(repo, _FX_TAG, &merge), "HINT_ANCHOR_PARENT_NOT_GUIDE"));
    c.ck(
        "다른 형식(v6 마커) 안내 위 태그도 안내 위이면 유효(형식 전환 뒤 앞 형식 태그 보존)",
        code_of(|| {
            let x = commit_tree_raw(repo, &[&t1, "--no-gpg-sign", "-p", &v6_guide], &msg, &fx_env)?;
            require_payload_anchor(repo, _FX_TAG, &x)
        })
        .is_none(),
    );
    c.ck(
        "★남의 페이로드 = HINT_ANCHOR_PROVENANCE_TAG_MISMATCH",
        expect_code(|| require_payload_anchor(repo, &format!("{_FX_TAG}x"), &c1), "HINT_ANCHOR_PROVENANCE_TAG_MISMATCH"),
    );
    c.ck("★빈 앵커 = HINT_ANCHOR_UNRESOLVED", expect_code(|| require_payload_anchor(repo, _FX_TAG, ""), "HINT_ANCHOR_UNRESOLVED"));

    let tag2 = _FX_TAG.replace("len262144", "len131072");
    let pay2 = tmp.join("payload2");
    write_fixture_payload(&pay2, &tag2, None, None, None, FX_BRIEF)?;
    let c2 = commit_payload(repo, &pay2, &commit_message(&pay2, _FX_UTC)?, _FX_UTC, Some(&guide), None)?;
    c.ck("★두 번째 페이로드도 부모 = 안내(직전 페이로드 ✗)", parents_c(repo, &c2)? == vec![guide.clone()] && hint_tip(repo)?.is_none());

    _update_hint_ref(repo, &guide, None)?;
    c.ck("★CAS 충돌 = HINT_BRANCH_CAS_CONFLICT", expect_code(|| _update_hint_ref(repo, &c1, Some(&c1)), "HINT_BRANCH_CAS_CONFLICT") && hint_tip(repo)? == Some(guide.clone()));
    c.ck("★생성 전용 CAS: 이미 있으면 거부", expect_code(|| _update_hint_ref(repo, &c1, None), "HINT_BRANCH_CAS_CONFLICT") && hint_tip(repo)? == Some(guide.clone()));

    let pay3 = tmp.join("payload3");
    write_fixture_payload(&pay3, &tag2, None, None, None, FX_BRIEF)?;
    let _ = std::fs::remove_file(pay3.join(PAYLOAD_JSON));
    let mut r3 = BTreeSet::new();
    rglob_files(&pay3, &pay3, &mut r3);
    let r3: Vec<String> = r3.into_iter().collect();
    let t3 = build_tree(repo, &pay3, &r3)?;
    let c3 = commit_tree_raw(repo, &[&t3, "-p", &guide, "--no-gpg-sign"], "raw\n", &fx_env)?;
    c.ck("★PAYLOAD 없는 페이로드 커밋 = HINT_ANCHOR_PAYLOAD_ABSENT", expect_code(|| require_payload_anchor(repo, &tag2, &c3), "HINT_ANCHOR_PAYLOAD_ABSENT"));

    let tip_before = hint_tip(repo)?;
    let tag4 = _FX_TAG.replace("len262144", "len65536");
    let pay4 = tmp.join("payload4");
    let pay4 = pay4.as_path();
    let ip = ["10", "1", "2", "3"].join(".");
    let home = format!("NAS={}/someone/models\n", ["", "home"].join("/"));
    for (label, rel, text) in [
        ("리터럴 term", "02-narrative.md", format!("운영 메모 {FIXTURE_TERM}\n")),
        ("사설 IPv4", "01-artifacts.md", format!("노드 {ip} 에서 실행\n")),
        ("baking 된 운영자 경로", "artifacts/triplet/.env.m", home.clone()),
    ] {
        write_fixture_payload(pay4, &tag4, None, Some(&exf(&[(rel, &text)])), None, FX_BRIEF)?;
        c.ck(
            &format!("★페이로드 PII({label}) = HINT_PAYLOAD_PII"),
            expect_code(|| commit_payload(repo, pay4, &commit_message(pay4, _FX_UTC)?, _FX_UTC, Some(&guide), None), "HINT_PAYLOAD_PII"),
        );
    }
    c.ck("★PII 차단 뒤 ref 불변", hint_tip(repo)? == tip_before);
    write_fixture_payload(pay4, &tag4, None, None, None, FX_BRIEF)?;
    c.ck(
        "★커밋 메시지 PII = HINT_COMMIT_MESSAGE_PII",
        expect_code(|| commit_payload(repo, pay4, &format!("hint: {tag4}\n{FIXTURE_TERM}\n"), _FX_UTC, Some(&guide), None), "HINT_COMMIT_MESSAGE_PII"),
    );
    c.ck(
        "★합성 신원이 PII 목록에 걸리면 tripwire 가 울린다",
        !identity_pii_hits(&[core::SYNTHETIC_NAME.to_string()], None)?.is_empty() && identity_pii_hits(&[FIXTURE_TERM.to_string()], None)?.is_empty(),
    );
    let terms_path = repo.join(core::REL_PII_TERMS);
    std::fs::rename(&terms_path, tmp.join("terms.bak")).map_err(|e| core::io_err(&e, &terms_path))?;
    c.ck(
        "★pii_terms 부재 = HINT_PII_TERMS_ABSENT(fail-closed)",
        expect_code(|| commit_payload(repo, pay4, &commit_message(pay4, _FX_UTC)?, _FX_UTC, Some(&guide), None), "HINT_PII_TERMS_ABSENT"),
    );
    std::fs::rename(tmp.join("terms.bak"), &terms_path).map_err(|e| core::io_err(&e, &terms_path))?;

    write_fixture_payload(pay4, &tag4, Some(&source), None, None, FX_BRIEF)?;
    c.ck(
        "★메시지에 source_anchor 없음 = HINT_ANCHOR_TRIPLE_MISMATCH",
        expect_code(|| commit_payload(repo, pay4, &format!("hint: {tag4}\n"), _FX_UTC, Some(&guide), None), "HINT_ANCHOR_TRIPLE_MISMATCH"),
    );
    let prov = core::read_json(&pay4.join(PROVENANCE_JSON))?;
    let cp4 = || commit_payload(repo, pay4, &commit_message(pay4, _FX_UTC)?, _FX_UTC, Some(&guide), None);
    core::write_json(&pay4.join(PROVENANCE_JSON), &set_key(&prov, "tag", _FX_TAG.into()))?;
    c.ck("★PROVENANCE.tag ≠ PAYLOAD.tag = HINT_PAYLOAD_TAG_MISMATCH", expect_code(cp4, "HINT_PAYLOAD_TAG_MISMATCH"));
    let pf: Vec<J> = prov["payload_files"].items().iter().skip(1).cloned().collect();
    core::write_json(&pay4.join(PROVENANCE_JSON), &set_key(&prov, "payload_files", J::List(pf)))?;
    c.ck("★PROVENANCE.payload_files ≠ 실물 = HINT_PROVENANCE_FILES_MISMATCH", expect_code(cp4, "HINT_PROVENANCE_FILES_MISMATCH"));
    let s12 = core::head_chars(&source, 12);
    core::write_json(&pay4.join(PROVENANCE_JSON), &set_key(&prov, "source_anchor", s12.clone().into()))?;
    c.ck(
        "★PROVENANCE.source_anchor 비-전체 SHA = HINT_PROVENANCE_SHAPE",
        expect_code(|| commit_payload(repo, pay4, &format!("hint: {tag4}\nsource_anchor: {s12}\n"), _FX_UTC, Some(&guide), None), "HINT_PROVENANCE_SHAPE"),
    );
    c.ck("★비-UTC 시각 = HINT_TIME_NOT_INJECTED", expect_code(|| commit_payload(repo, pay, &msg, "2026-09-21 10:21", Some(&guide), None), "HINT_TIME_NOT_INJECTED"));

    let prompt = "## 2.1 출발점\n<!-- PROMPT\n질문: 무엇이 벽이었나\n-->\n";
    write_fixture_payload(pay4, &tag4, None, Some(&exf(&[("02-narrative.md", prompt)])), None, FX_BRIEF)?;
    c.ck("★PROMPT 잔재 = HINT_PROMPT_RESIDUE", expect_code(cp4, "HINT_PROMPT_RESIDUE"));
    write_fixture_payload(pay4, &tag4, None, Some(&exf(&[("01-artifacts.md", "## 1.1 적용 판정\n<<AGENT: 이 줄을 바꿔라>>\n")])), None, FX_BRIEF)?;
    c.ck("★미저작 자리표시 잔재 = HINT_AGENT_PLACEHOLDER_RESIDUE", expect_code(cp4, "HINT_AGENT_PLACEHOLDER_RESIDUE"));
    write_fixture_payload(pay4, &tag4, None, None, Some(&set_key(&fixture_payload_doc(&tag4)?, "format", "hint/v5".into())), FX_BRIEF)?;
    c.ck("★옛 형식 표지 = HINT_PAYLOAD_FORMAT", expect_code(cp4, "HINT_PAYLOAD_FORMAT"));
    let mut no_naming = fixture_payload_doc(&tag4)?;
    no_naming.as_dict_mut().unwrap().remove("naming");
    write_fixture_payload(pay4, &tag4, None, None, Some(&no_naming), FX_BRIEF)?;
    c.ck("★PAYLOAD.naming 부재 = HINT_PAYLOAD_NAMING_ABSENT(대조 부재 ≠ 대조 통과)", expect_code(cp4, "HINT_PAYLOAD_NAMING_ABSENT"));
    write_fixture_payload(pay4, &tag4, None, None, Some(&set_key(&fixture_payload_doc(&tag2)?, "tag", tag4.clone().into())), FX_BRIEF)?;
    c.ck("★naming 재조립 ≠ PAYLOAD.tag = HINT_DERIVED_NAME_MISMATCH", expect_code(cp4, "HINT_DERIVED_NAME_MISMATCH"));
    let mut bad_axes = fixture_payload_doc(&tag4)?;
    if let Some(ax) = bad_axes.get_mut("naming").and_then(|n| n.get_mut("axes")) {
        ax.set("len", jd! {"value" => "4096", "source" => "fixture:tamper"});
    }
    write_fixture_payload(pay4, &tag4, None, None, Some(&bad_axes), FX_BRIEF)?;
    c.ck("★naming 축 ≠ 세그먼트 = HINT_NAMING_INCONSISTENT", expect_code(cp4, "HINT_NAMING_INCONSISTENT"));
    write_fixture_payload(pay4, &tag4, None, Some(&exf(&[(&format!("artifacts/triplet/{FIXTURE_TERM}.env"), "A=1\n")])), None, FX_BRIEF)?;
    let cc = code_of(cp4);
    c.ck("★경로 이름 PII 는 커밋을 막는다", matches!(cc.as_deref(), Some("HINT_PAYLOAD_PII" | "HINT_PAYLOAD_PATH_PII")));
    let legacy = "hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8";
    write_fixture_payload(pay4, legacy, None, None, None, FX_BRIEF)?;
    c.ck("★옛 문법 이름(노드축 없음)은 새로 얹지 않는다 = HINT_ARCH_NODE_AXIS_ABSENT", expect_code(cp4, "HINT_ARCH_NODE_AXIS_ABSENT"));
    c.ck("거부들 뒤 ref 불변", hint_tip(repo)? == tip_before);

    write_fixture_payload(pay4, &tag4, None, None, None, FX_BRIEF)?;
    _update_hint_ref(repo, &c2, tip_before.as_deref())?;
    let c4 = cp4()?;
    c.ck("★부모 = 지정 안내(로컬 hint tip 무관) · 로컬 hint 불이동", parents_c(repo, &c4)? == vec![guide.clone()] && hint_tip(repo)? == Some(c2.clone()));
    _update_hint_ref(repo, tip_before.as_deref().unwrap_or(""), Some(&c2))?;

    let terms = vec![FIXTURE_TERM.to_string()];
    c.ck("commit_violations(정상 페이로드 커밋) = 0", commit_violations(repo, &c1, _FX_TAG, &terms)?.is_empty());
    c.ck("★commit_violations 남의 태그 = HINT_PAYLOAD_TAG_MISMATCH", commit_violations(repo, &c1, &tag2, &terms)?.iter().any(|p| p.starts_with("HINT_PAYLOAD_TAG_MISMATCH:")));
    c.ck("★commit_violations PAYLOAD 없는 커밋 = HINT_PAYLOAD_REQUIRED_ABSENT", commit_violations(repo, &c3, &tag2, &terms)?.iter().any(|p| p.starts_with("HINT_PAYLOAD_REQUIRED_ABSENT:")));
    let mut oe = op_env();
    oe.push(("GIT_AUTHOR_DATE".into(), core::git_date(_FX_UTC)?));
    oe.push(("GIT_COMMITTER_DATE".into(), core::git_date(_FX_UTC)?));
    let op = commit_tree_raw(repo, &[&t1, "--no-gpg-sign"], &msg, &oe)?;
    c.ck(
        "★운영자 신원 커밋 = HINT_COMMIT_IDENTITY_NOT_SYNTHETIC(목록 스캔이 아니라 동일성 판정)",
        commit_violations(repo, &op, _FX_TAG, &terms)?.iter().any(|p| p.starts_with("HINT_COMMIT_IDENTITY_NOT_SYNTHETIC:")),
    );
    let pay5 = tmp.join("payload5");
    write_fixture_payload(&pay5, &tag4, None, Some(&exf(&[("02-narrative.md", prompt)])), Some(&J::list()), FX_BRIEF)?;
    let t5 = build_tree(repo, &pay5, &payload_files(&pay5)?)?;
    let c5 = commit_tree_raw(repo, &[&t5, "--no-gpg-sign"], &format!("hint: {tag4}\n"), &fx_env)?;
    let v5: BTreeSet<String> = commit_violations(repo, &c5, &tag4, &terms)?.iter().map(|p| p.split(':').next().unwrap_or("").to_string()).collect();
    c.ck("★객체 아닌 PAYLOAD.json = UNREADABLE 이고 PROMPT 잔재도 여전히 잡힌다", v5.contains("HINT_PAYLOAD_JSON_UNREADABLE") && v5.contains("HINT_PROMPT_RESIDUE"));
    c.ck(
        "★commit_violations 는 리터럴 PII 도 본다",
        commit_violations(repo, &c1, _FX_TAG, &["지도이지 정답".to_string()])?.iter().any(|p| p.starts_with("HINT_PAYLOAD_PII:")),
    );
    Ok(())
}

fn _transition_selftest(tmp: &Path, c: &mut Ck) -> HR<()> {
    let template = core::templates_dir().join("hint-branch-README.md");
    let mut tpl_body = core::read_text(&template).map_err(|e| HintError::py("OSError", e))?;
    if readme_format_text(&tpl_body).is_some() {
        tpl_body = tpl_body.split_once('\n').map(|x| x.1.to_string()).unwrap_or_default();
    }
    let marker = guide_marker(BRANCH_FORMAT)?;
    let mk_repo = |name: &str, with_marker: bool| -> HR<PathBuf> {
        let r = selftest_repo(tmp, name)?;
        let target = r.join(_BRANCH_TEMPLATE_REL);
        mkdirs(target.parent().unwrap())?;
        write_text(&target, &format!("{}{tpl_body}", if with_marker { format!("{marker}\n") } else { String::new() }))?;
        core::git(&r, &["add", _BRANCH_TEMPLATE_REL])?;
        core::git(&r, &["commit", "-qm", "tracked transition template"])?;
        Ok(r)
    };
    let mk_bare = |name: &str| -> HR<PathBuf> {
        let b = tmp.join(format!("{name}.git"));
        core::git(tmp, &["init", "-q", "--bare", &core::posix(&b)])?;
        Ok(b)
    };
    let refspec = format!("{HINT_BRANCH_REF}:{HINT_BRANCH_REF}");
    let ab = Some("픽스처 사람 발화 전사");
    let au = Some(_FX_UTC);
    let fx_env = identity_env_pairs(_FX_UTC)?;

    let repo = mk_repo("transition-repo", true)?;
    let repo = repo.as_path();
    let bare = mk_bare("transition-remote")?;
    let bare_s = core::posix(&bare);
    core::git(repo, &["remote", "add", "origin", &bare_s])?;
    let old = selftest_guide(repo, None, None, FX_GUIDE_BODY, None)?;
    _update_hint_ref(repo, &old, None)?;
    core::git(repo, &["push", "-q", "origin", &refspec])?;
    let pay = tmp.join("transition-payload");
    write_fixture_payload(&pay, _FX_TAG, None, None, None, FX_BRIEF)?;
    let chain_tree = build_tree(repo, &pay, &payload_files(&pay)?)?;
    let chain = commit_tree_raw(repo, &[&chain_tree, "--no-gpg-sign", "-p", &old], &commit_message(&pay, _FX_UTC)?, &fx_env)?;
    _update_hint_ref(repo, &chain, Some(&old))?;
    c.ck(
        "★v7 발행기: 원격 tip 이 마커 없는 옛 안내 = HINT_BRANCH_FORMAT_MISMATCH(처방 = branch-transition)",
        expect_code(|| guide_commit(repo, "origin", None), "HINT_BRANCH_FORMAT_MISMATCH"),
    );
    if let Err(e) = guide_commit(repo, "origin", None) {
        let r = e.remedy.unwrap_or_default();
        c.ck("형식 불일치 처방 문구에 branch-transition · 사람 승인", r.contains("branch-transition") && r.contains("승인"));
    }
    let absent = core::posix(&tmp.join("absent.git"));
    c.ck("★원격 조회 실패 = HINT_REMOTE_QUERY_FAILED(fail-closed)", expect_code(|| guide_commit(repo, &absent, None), "HINT_REMOTE_QUERY_FAILED"));
    let empty = core::posix(&mk_bare("transition-empty")?);
    c.ck("★원격에 hint 브랜치 없음 = HINT_BRANCH_GUIDE_ABSENT", expect_code(|| guide_commit(repo, &empty, None), "HINT_BRANCH_GUIDE_ABSENT"));
    c.ck(
        "★commit_payload(remote=) 도 같은 거부 · 로컬 hint 불변",
        expect_code(|| commit_payload(repo, &pay, &commit_message(&pay, _FX_UTC)?, _FX_UTC, None, Some("origin")), "HINT_BRANCH_FORMAT_MISMATCH")
            && hint_tip(repo)? == Some(chain.clone()),
    );

    let refs0 = refs_snapshot(repo)?;
    c.ck("★승인 전사 없음 = HINT_APPROVAL_ABSENT", expect_code(|| branch_transition(repo, "origin", _FX_UTC, None, None), "HINT_APPROVAL_ABSENT"));
    c.ck(
        "★태그가 붙잡지 않는 로컬 페이로드 = HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD",
        expect_code(|| branch_transition(repo, "origin", _FX_UTC, ab, au), "HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD"),
    );
    c.ck("거부들 뒤 ref·원격 불변", refs_snapshot(repo)? == refs0 && _remote_hint_tip(repo, "origin")? == Some(old.clone()));
    let nomark = mk_repo("transition-nomarker", false)?;
    core::git(&nomark, &["remote", "add", "origin", &bare_s])?;
    c.ck(
        "★템플릿 첫 줄에 마커 없음 = HINT_BRANCH_TEMPLATE_FORMAT(도구가 덧붙이지 않는다)",
        expect_code(|| branch_transition(&nomark, "origin", _FX_UTC, ab, au), "HINT_BRANCH_TEMPLATE_FORMAT"),
    );

    core::git(repo, &["tag", "-a", "-m", "fixture", _FX_TAG, &chain])?;
    let result = branch_transition(repo, "origin", _FX_UTC, ab, au)?;
    let new = hint_tip(repo)?.unwrap_or_default();
    let ents = entries(repo, &new)?;
    c.ck(
        "branch-transition: README 하나 mode 100644 · 첫 줄 v7 마커",
        result.get_str("status") == Some("transitioned")
            && ents.len() == 1
            && ents[0].mode == "100644"
            && ents[0].path == GUIDE_README
            && readme_format_bytes(read_blob_bytes(repo, &new, GUIDE_README)?.as_deref()).as_deref() == Some(BRANCH_FORMAT),
    );
    c.ck(
        "branch-transition: 부모 = 원격의 옛 안내(로컬 체인 ✗) · remote == local",
        parents_c(repo, &new)? == vec![old.clone()] && _remote_hint_tip(repo, "origin")? == Some(new.clone()),
    );
    let track = |r: &Path| -> HR<String> { Ok(py_strip(&core::git_nocheck(r, &["rev-parse", "--verify", "--quiet", "refs/remotes/origin/hint"])?.stdout).to_string()) };
    c.ck("★원격 추적 ref 도 새 안내(push 가 갱신 · 낡은 origin/hint 해소)", track(repo)? == new);
    c.ck("★옛 체인 태그는 여전히 자기 커밋을 든다(R6)", core::git_out(repo, &["rev-parse", &format!("{_FX_TAG}^{{commit}}")])? == chain);
    let rawnew = core::git_bytes(repo, &["cat-file", "commit", &new], false)?.unwrap_or_default();
    let abb = ab.unwrap().as_bytes();
    c.ck(
        "★승인 전사는 반환값에만(배포 오브젝트 ✗)",
        result.get_str("approved_by") == ab && !rawnew.windows(abb.len()).any(|w| w == abb),
    );
    c.ck(
        "branch-transition: 재실행 = already-transitioned(멱등 · 재시도 UTC 무관)",
        branch_transition(repo, "origin", "2026-09-22T00:00:00Z", ab, au)?.get_str("status") == Some("already-transitioned") && hint_tip(repo)? == Some(new.clone()),
    );
    c.ck("전환 뒤 guide_commit(origin) = (새 안내, v7)", guide_commit(repo, "origin", None)? == (new.clone(), BRANCH_FORMAT.into()));

    let clone = selftest_repo(tmp, "transition-clone")?;
    core::git(&clone, &["remote", "add", "origin", &bare_s])?;
    c.ck("대조: 클론에는 안내 오브젝트가 없다", !has_commit(&clone, &new)?);
    let hr = |r: &Path| core::git_out(r, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/heads", "refs/tags"]);
    let refs_c = hr(&clone)?;
    c.ck("★guide_commit: 한 ref fetch 뒤 (새 안내, v7)", guide_commit(&clone, "origin", None)? == (new.clone(), BRANCH_FORMAT.into()));
    c.ck(
        "★fetch 는 로컬 브랜치·태그를 만들지 않는다(추적 ref 만)",
        hr(&clone)? == refs_c && hint_tip(&clone)?.is_none() && track(&clone)? == new,
    );
    let clone2 = selftest_repo(tmp, "transition-clone-url")?;
    c.ck(
        "URL·경로 원격(추적 ref 없음)도 오브젝트만 받는다",
        guide_commit(&clone2, &bare_s, None)? == (new.clone(), BRANCH_FORMAT.into()) && core::git_out(&clone2, &["for-each-ref", "--format=%(refname)", "refs/remotes"])?.is_empty(),
    );
    let pay_c = tmp.join("transition-clone-payload");
    write_fixture_payload(&pay_c, _FX_TAG, None, None, None, FX_BRIEF)?;
    let cc = commit_payload(&clone, &pay_c, &commit_message(&pay_c, _FX_UTC)?, _FX_UTC, None, Some("origin"))?;
    c.ck("★commit_payload(remote=): 부모 = 원격 안내 · 로컬 hint 불이동", parents_c(&clone, &cc)? == vec![new.clone()] && hint_tip(&clone)?.is_none());
    let sg = selftest_guide(&clone, Some(BRANCH_FORMAT), None, FX_GUIDE_BODY, None)?;
    c.ck(
        "★guide= 와 remote= 가 다르면 = HINT_BRANCH_GUIDE_MOVED",
        expect_code(|| commit_payload(&clone, &pay_c, &commit_message(&pay_c, _FX_UTC)?, _FX_UTC, Some(&sg), Some("origin")), "HINT_BRANCH_GUIDE_MOVED"),
    );

    let stale = mk_repo("transition-stale", true)?;
    core::git(&stale, &["remote", "add", "origin", &bare_s])?;
    let sold = selftest_guide(&stale, None, None, FX_GUIDE_BODY, None)?;
    _update_hint_ref(&stale, &sold, None)?;
    let synced = branch_transition(&stale, "origin", _FX_UTC, ab, au)?;
    c.ck(
        "★원격 v7 · 로컬 옛 값 = local-synced(로컬만 원격 tip 으로 · push 0)",
        synced.get_str("status") == Some("local-synced") && hint_tip(&stale)? == Some(new.clone()) && _remote_hint_tip(&stale, "origin")? == Some(new.clone()),
    );

    let repo2 = mk_repo("transition-resume", true)?;
    let bare2 = mk_bare("transition-resume")?;
    core::git(&repo2, &["remote", "add", "origin", &core::posix(&bare2)])?;
    let old2 = selftest_guide(&repo2, None, None, FX_GUIDE_BODY, None)?;
    _update_hint_ref(&repo2, &old2, None)?;
    core::git(&repo2, &["push", "-q", "origin", &refspec])?;
    let tb = _transition_template_bytes(&repo2)?;
    let staged = _transition_commit(&repo2, &old2, &_transition_tree(&repo2, &tb, true)?, _FX_UTC)?;
    _update_hint_ref(&repo2, &staged, Some(&old2))?;
    let resumed = branch_transition(&repo2, "origin", "2026-09-22T00:00:00Z", ab, au)?;
    c.ck(
        "branch-transition: local=후보 remote=old → push-resumed(재시도 UTC 달라도 구조 판정)",
        resumed.get_str("status") == Some("push-resumed") && _remote_hint_tip(&repo2, "origin")? == Some(staged.clone()),
    );

    let repo3 = mk_repo("transition-downgrade", true)?;
    let bare3 = mk_bare("transition-downgrade")?;
    core::git(&repo3, &["remote", "add", "origin", &core::posix(&bare3)])?;
    let v8 = selftest_guide(&repo3, Some("v8"), None, FX_GUIDE_BODY, None)?;
    _update_hint_ref(&repo3, &v8, None)?;
    core::git(&repo3, &["push", "-q", "origin", &refspec])?;
    c.ck(
        "★원격 v8 안내 = HINT_BRANCH_TRANSITION_DOWNGRADE · 원격 불변",
        expect_code(|| branch_transition(&repo3, "origin", _FX_UTC, ab, au), "HINT_BRANCH_TRANSITION_DOWNGRADE") && _remote_hint_tip(&repo3, "origin")? == Some(v8.clone()),
    );
    c.ck("★v7 발행기에게 v8 안내도 형식 불일치", expect_code(|| guide_commit(&repo3, "origin", None), "HINT_BRANCH_FORMAT_MISMATCH"));

    let repo4 = mk_repo("transition-payloadtip", true)?;
    let bare4 = core::posix(&mk_bare("transition-payloadtip")?);
    let pay4 = tmp.join("transition-payload4");
    write_fixture_payload(&pay4, _FX_TAG, None, None, None, FX_BRIEF)?;
    let pt = build_tree(&repo4, &pay4, &payload_files(&pay4)?)?;
    let ptip = commit_tree_raw(&repo4, &[&pt, "--no-gpg-sign"], "x\n", &fx_env)?;
    _update_hint_ref(&repo4, &ptip, None)?;
    core::git(&repo4, &["push", "-q", &bare4, &refspec])?;
    c.ck(
        "★원격 tip = 페이로드 커밋 = HINT_BRANCH_FORMAT_MISMATCH(트리가 README 하나가 아님)",
        expect_code(|| guide_commit(&repo4, &bare4, None), "HINT_BRANCH_FORMAT_MISMATCH"),
    );
    Ok(())
}
