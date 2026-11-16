//! hintlib.lineage — 계보 파생: **발행 시점 파일**에서 mention 그래프를 계산해 `LINEAGE.json` 을 만든다.
//! Port of `scripts/hintlib/lineage.py` (plan_26092119 §4.3 · SPEC X1·X17). The invariants, resolution rules and
//! traversal are documented in the original module docstring; this port keeps them byte for byte.
//!
//! Owner boundary (the naming grammar): the original loaded two owners' Python modules in-process (`_grammar`):
//! the wiki-desk prose naming (through the shim `.claude/policies/runtime/doc_naming.py`, which itself reads the TS
//! `PATTERNS` with `node`) and the adversarial-benchmark bench naming (`scripts/doc_naming.py`). This port never starts
//! Python or node. It reads the pattern **literals** from the owners' source files at run time (the TS `PATTERNS` object
//! and `DATED_DOC_TYPES` array; the Python `_BENCH_NAME_RE` raw strings and `BENCH_KIND_DEFAULT_EXT` keys) and compiles
//! them itself, so a change of the owner's pattern text is picked up and a vanished literal fails closed with
//! `HINT_LINEAGE_GRAMMAR_DRIFT`. The owners' *functions* the original called (`is_dated_doc_basename`,
//! `validate_topic`, `simlog_dirname` — TS semantics) are mirrored (`OWNER-MIRROR` markers below).
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use crate::core::{self, fail, HintError, HR};
use crate::pyre::{self, Re};
use crate::value::{dumps_with, py_repr, Dump, Map, J};
use crate::{jd, re_static};

// ── 계약 상수 ────────────────────────────────────────────────────────────────────────────────
pub const SCHEMA_VERSION: i64 = 1;
pub const DEFAULT_DEPTH: i64 = 4;
pub const DEFAULT_ROOTS: [&str; 6] = ["docs/plan", "docs/devlog", "docs/testlog", "docs/report", "docs/benchmark", "docs/simlog"];
pub const EXCLUDED_ROOTS: [&str; 4] = ["seed/", "sync_staging/", ".claude/", "CLAUDE.md"];
pub const HEADER_LINES: usize = 12;
pub const SWEEP_MAP_PREFIX: &str = "sweep_map";
pub const EXCLUDED_BASENAMES: [&str; 1] = ["example.md"];
pub const EXPAND_MAX_FILES: usize = 64;
const DIR_CANDIDATE_KINDS: [&str; 3] = ["simlog", "raw_dir", "engine_failure_logs"];
pub const MODEL_PRIOR_SOURCE: &str = "model_prior_publication:";
pub const MODEL_PRIOR_KINDS: [&str; 4] = ["plan", "devlog", "testlog", "report"];
pub const TOOL_SOURCE_KIND: &str = "tool-source@rev";
pub const TOOL_SNAPSHOT_DIR: &str = "inputs/sources";
pub const DOCUMENT_KINDS: [&str; 9] =
    ["plan", "devlog", "testlog", "request", "checklist", "report", "bench_report", "max_envelope", "sweep_map"];
pub const DATA_KINDS: [&str; 7] = ["simlog", "certificate", "sweep_json", "engine_log", "raw_json", "raw_file", "raw_dir"];
pub const IDENTITY_MATCH_FIELDS: [&str; 4] = ["model", "gpu", "topology", "tp"];
pub const RESOLUTIONS: [&str; 3] = ["exact", "unique", "ambiguous"];
const TEXT_SUFFIXES: [&str; 8] = [".md", ".html", ".json", ".jsonl", ".yaml", ".yml", ".txt", ".log"];
const STRIP_EXTS: [&str; 5] = [".md", ".html", ".yaml", ".yml", ".json"];
const DOC_EXTS: [&str; 2] = [".md", ".html"];
const NAME_EXTRA: &str = "_-+";

pub const LINEAGE_FULL_NAME: &str = "LINEAGE.full.json";
pub const SUMMARY_SCHEMA_VERSION: i64 = 3;
pub const SUMMARY_KIND: &str = "receiver-summary";
pub const SUMMARY_MAX_BYTES: usize = 20 * 1024;
pub const SEALED_SOURCES_KEY: &str = "sealed_sources";

fn res_rank(r: &str) -> usize {
    RESOLUTIONS.iter().position(|x| *x == r).unwrap_or(99)
}

fn kind_rank(k: &str) -> i64 {
    match k {
        "plan" => 0,
        "testlog" => 1,
        "devlog" => 2,
        "report" => 3,
        "bench_report" => 4,
        "max_envelope" => 5,
        "sweep_map" => 6,
        "request" => 7,
        "checklist" => 8,
        _ => 99,
    }
}

re_static!(tool_snapshot_re, r"^inputs/sources/([A-Za-z0-9][A-Za-z0-9._-]*)@([0-9a-f]{12})\Z");
re_static!(tool_origin_re, r"^git:([0-9a-f]{40}):([A-Za-z0-9._/-]+)\Z");
re_static!(sweep_map_re, r"^sweep_map_(\d{8})(?:_(\d{2})_(\d{2}))?_(.+)\.(md|json)\Z");
re_static!(simlog_modern_re, r"^(\d{8})(?:_(\d{2})_(\d{2}))?_(.+)\Z");
re_static!(simlog_legacy_re, r"^(\d{10})_(\d+)_(.+)\Z");
re_static!(simlog_token_re, r"(?<![A-Za-z0-9_])docs/simlog/(?P<tt>\d{10}|\d{8})(?![0-9])");
re_static!(undated_path_re, r#"(?<![A-Za-z0-9_])docs/(?P<dir>[a-z_]+)/(?P<rest>[^\s`'"<>()\[\]|,;*]+)"#);
re_static!(output_path_re, r#"(?<![A-Za-z0-9_./-])output/[a-z]+/benchlog/[^\s`'"<>()\[\]|,;*{}]+"#);
re_static!(mmss_at_re, r"_(\d{2})_(\d{2})(?![0-9])");
re_static!(supersede_mark_re, r"SUPERSEDED(?:-IN-PART)?");
re_static!(header_label_re, r"^\s*>\s*\**([가-힣A-Za-z]+)\**\s*[:：]?\s*`?");
re_static!(
    generic_doc_token_re,
    r#"(?<![A-Za-z0-9_])(?:docs/[a-z_]+/)?(?P<tok>[a-z][a-z0-9]*(?:_[a-z][a-z0-9]*)?_(?:\d{8}|NA)(?![0-9])(?:_[^\s`'"<>()\[\]|,;*§]+)?)"#
);
re_static!(bare_stem_re, r"^(?:[a-z][a-z0-9_]*_(?:\d{8}|NA)|\d{8}|\d{10})(?:_\d{2}_\d{2})?\Z");
re_static!(kst_token_re, r"^\d{8}\Z");
re_static!(docs_dir_prefix_re, r"docs/[a-z_]+/");
re_static!(report_mmss_re, r"(\d{2})_(\d{2})_(.+)\Z");
re_static!(sep_split_re, r"[-_.\s]+");
re_static!(norm_key_re, r"[-.\s]+");
re_static!(id_split_re, r"[^0-9a-z]+");
re_static!(campaign_id_token_re, r"^camp\d*\Z");
re_static!(typed_split_re, r"_\d{8}(?:_\d{2}_\d{2})?_?");
re_static!(dash_us_re, r"[-_]");
re_static!(cell_split_re, r"[-_.]");
re_static!(simlog_tail_re, r#"[^\s`'"<>()\[\]|,;*§/]+"#);
re_static!(hex40_re, r"[0-9a-f]{40}");

// ── Python 문자열 도우미 ──────────────────────────────────────────────────────────────────────
fn py_ws(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_ws)
}
fn py_lstrip(s: &str) -> &str {
    s.trim_start_matches(py_ws)
}
fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}
fn rstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_end_matches(|c| chars.contains(c))
}
fn clen(s: &str) -> usize {
    s.chars().count()
}
/// Python `str.isalnum()` for one character.
fn py_isalnum(c: char) -> bool {
    c.is_alphanumeric()
}
/// `str(x)` of a value.
fn pystr(v: &J) -> String {
    v.display()
}
/// `x in (None, "")`.
fn none_or_empty(v: Option<&J>) -> bool {
    matches!(v, None | Some(J::Null)) || v.and_then(J::as_str) == Some("")
}

/// `PurePosixPath(p).parts` without the root.
fn pparts(p: &str) -> Vec<&str> {
    p.split('/').filter(|x| !x.is_empty() && *x != ".").collect()
}
/// `PurePosixPath(p).name`.
fn pname(p: &str) -> String {
    pparts(p).last().map(|s| s.to_string()).unwrap_or_default()
}
/// `PurePosixPath(p).parent.name`.
fn pparent_name(p: &str) -> String {
    let v = pparts(p);
    if v.len() >= 2 {
        v[v.len() - 2].to_string()
    } else {
        String::new()
    }
}
/// `posixpath.dirname(p)`.
fn pdirname(p: &str) -> String {
    match p.rfind('/') {
        Some(i) => {
            let head = &p[..=i];
            let trimmed = head.trim_end_matches('/');
            if trimmed.is_empty() {
                head.to_string()
            } else {
                trimmed.to_string()
            }
        }
        None => String::new(),
    }
}
/// `posixpath.normpath(p)`.
fn normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let mut initial = if path.starts_with('/') { 1 } else { 0 };
    if initial == 1 && path.starts_with("//") && !path.starts_with("///") {
        initial = 2;
    }
    let mut new: Vec<&str> = Vec::new();
    for comp in path.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp != ".." || (initial == 0 && new.is_empty()) || new.last() == Some(&"..") {
            new.push(comp);
        } else if !new.is_empty() {
            new.pop();
        }
    }
    let mut out = "/".repeat(initial);
    out.push_str(&new.join("/"));
    if out.is_empty() {
        ".".to_string()
    } else {
        out
    }
}
/// `posixpath.join(a, b)` for relative b.
fn pjoin(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        b.to_string()
    } else if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}
/// Python `Path.suffix`.
fn py_suffix(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => &name[i..],
        _ => "",
    }
}
/// `"{:,}"`.
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
/// Universal newlines (`open(..., newline=None)`): `\r\n` and `\r` become `\n`.
fn universal_newlines(s: String) -> String {
    if !s.contains('\r') {
        return s;
    }
    s.replace("\r\n", "\n").replace('\r', "\n")
}
/// `Path.read_text(encoding="utf-8", errors="replace")`.
fn read_text_replace(p: &Path) -> Result<String, std::io::Error> {
    let b = std::fs::read(p)?;
    Ok(universal_newlines(String::from_utf8_lossy(&b).into_owned()))
}
/// Python exception class name for an `OSError` errno.
fn os_exc_name(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(2) => "FileNotFoundError",
        Some(1) | Some(13) => "PermissionError",
        Some(21) => "IsADirectoryError",
        Some(20) => "NotADirectoryError",
        _ => "OSError",
    }
}
/// `Path.read_text(encoding="utf-8")`: `Err((exception type, str(exc)))`.
fn read_text_strict(p: &Path) -> Result<String, (String, String)> {
    let b = std::fs::read(p).map_err(|e| (os_exc_name(&e).to_string(), core::io_err_text(&e, p)))?;
    match String::from_utf8(b) {
        Ok(s) => Ok(universal_newlines(s)),
        Err(e) => {
            let bytes = e.as_bytes();
            let u = e.utf8_error();
            let pos = u.valid_up_to();
            let b0 = bytes[pos];
            let msg = match u.error_len() {
                None => {
                    let end = bytes.len() - 1;
                    if end > pos {
                        format!("'utf-8' codec can't decode bytes in position {pos}-{end}: unexpected end of data")
                    } else {
                        format!("'utf-8' codec can't decode byte 0x{b0:02x} in position {pos}: unexpected end of data")
                    }
                }
                Some(_) => {
                    let valid_start = matches!(b0, 0xC2..=0xF4);
                    let why = if valid_start { "invalid continuation byte" } else { "invalid start byte" };
                    format!("'utf-8' codec can't decode byte 0x{b0:02x} in position {pos}: {why}")
                }
            };
            Err(("UnicodeDecodeError".to_string(), msg))
        }
    }
}

fn os_py(e: &std::io::Error, p: &Path) -> HintError {
    HintError::py(os_exc_name(e), core::io_err_text(e, p))
}

/// git blob SHA-1 of the bytes.
fn git_blob_sha(data: &[u8]) -> String {
    let mut buf = format!("blob {}\0", data.len()).into_bytes();
    buf.extend_from_slice(data);
    let d = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &buf);
    d.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// A regex literal escaped for this engine (every non-word ASCII character as `\x{..}`). `re.escape` is the text form.
fn rx_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || !c.is_ascii() {
            out.push(c);
        } else {
            out.push_str(&format!("\\x{{{:X}}}", c as u32));
        }
    }
    out
}
/// Python `re.escape` (3.7+): the pattern **text** the original reports.
fn py_re_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `re.split(p, s, maxsplit=1)`.
fn split_once_re(re: &Re, s: &str) -> Vec<String> {
    match re.search(s) {
        Some(c) => {
            let m = c.get(0).unwrap();
            vec![s[..m.start()].to_string(), s[m.end()..].to_string()]
        }
        None => vec![s.to_string()],
    }
}

// ── 문법 적재 (SSOT 2종 · 소유자 원천의 리터럴 · fail-closed) ──────────────────────────────────────
struct Grammar {
    dated_types: Vec<String>,
    /// `_DATED_DOC_RE` as the Python shim compiles it (Python semantics).
    dated_re: Re,
    /// The same pattern with the TS (JavaScript `u`) semantics `is_dated_doc_basename` uses.
    dated_js: Re,
    report_re: Re,
    bench_re: Re,
    bench_kinds: Vec<String>,
    /// TS `TOPIC_RE` (JavaScript semantics).
    topic_js: Re,
}

static GRAMMAR_CACHE: OnceLock<Mutex<HashMap<String, Arc<Grammar>>>> = OnceLock::new();

fn own_checkout() -> PathBuf {
    let s = core::skill_dir();
    s.ancestors().nth(3).map(Path::to_path_buf).unwrap_or(s)
}

const DRIFT_REMEDY: &str = "명명 SSOT 가 바뀌었다 — hintlib/lineage.py 의 문법 적재(_grammar)를 새 공개 심볼에 맞춘다.";

fn owner_missing<T>(rel: &str) -> HR<T> {
    fail(
        "HINT_OWNER_MODULE_MISSING",
        format!("소유 모듈 부재: {rel}"),
        Some("대체 구현으로 계속하지 않는다 — 그 스킬이 설치된 체크아웃에서 실행한다."),
    )
}

/// Read a JS string literal starting at the opening quote; returns the decoded value.
fn js_string_at(s: &str) -> Option<String> {
    let mut chars = s.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// `PATTERNS.<key>` literal of the TS source.
fn ts_pattern(src: &str, key: &str) -> Option<String> {
    let start = src.find("export const PATTERNS")?;
    let body = &src[start..];
    let end = body.find("} as const").unwrap_or(body.len());
    let body = &body[..end];
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix(&format!("{key}:")) {
            return js_string_at(rest.trim_start());
        }
    }
    None
}

/// `DATED_DOC_TYPES` array literal of the TS source.
fn ts_dated_types(src: &str) -> Option<Vec<String>> {
    let start = src.find("export const DATED_DOC_TYPES = [")?;
    let rest = &src[start + "export const DATED_DOC_TYPES = [".len()..];
    let end = rest.find(']')?;
    let mut out = Vec::new();
    let inner = &rest[..end];
    let mut i = 0;
    while let Some(q) = inner[i..].find('"') {
        let s = js_string_at(&inner[i + q..])?;
        i += q + s.len() + 2;
        out.push(s);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// `_BENCH_NAME_RE = _re.compile(r"..." r"...")` raw-string pieces of the Python source.
fn py_bench_name_re(src: &str) -> Option<String> {
    let start = src.find("_BENCH_NAME_RE = _re.compile(")?;
    let rest = &src[start + "_BENCH_NAME_RE = _re.compile(".len()..];
    let mut out = String::new();
    let mut s = rest;
    loop {
        let t = s.trim_start();
        if let Some(r) = t.strip_prefix("r\"") {
            let end = r.find('"')?;
            out.push_str(&r[..end]);
            s = &r[end + 1..];
        } else if t.starts_with(')') {
            break;
        } else {
            return None;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// `BENCH_KIND_DEFAULT_EXT = {...}` keys of the Python source.
/// The bench naming owner's Rust source (adversarial-benchmark `bench_naming`).
pub const REL_BENCH_NAMING_RS: &str = core::REL_DOC_NAMING_BENCH;

/// The bench naming grammar of the Rust owner, rendered in the shape the `.py` readers below parse:
/// kinds = `KIND_DEFAULT_EXTENSIONS` (in order), name pattern = the declared matcher grammar of `parse_conventional`
/// (its doc line), its four capture groups named tok/mm/ss/combo in order. The alternation of the pattern must equal
/// the kinds and every default extension must be admitted, else `None` (= grammar drift, fail closed).
fn rs_bench_as_py(src: &str) -> Option<String> {
    let start = src.find("pub const KIND_DEFAULT_EXTENSIONS")?;
    let body = &src[start..start + src[start..].find("];")?];
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("(\"") {
            let (k, rest) = rest.split_once("\", \"")?;
            let (e, _) = rest.split_once("\")")?;
            pairs.push((k.to_string(), e.to_string()));
        }
    }
    if pairs.is_empty() {
        return None;
    }
    let fn_at = src.find("\nfn parse_conventional(")?;
    let doc = src[..fn_at].lines().last()?.trim();
    let pat = doc.strip_prefix("/// `")?.split_once('`')?.0;
    let kinds: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
    if !pat.starts_with(&format!("^(?:{})_", kinds.join("|"))) {
        return None;
    }
    let exts = pat.rsplit_once("\\.(?:")?.1.strip_suffix(")$")?;
    if !pairs.iter().all(|(_, e)| exts.split('|').any(|x| x == e)) {
        return None;
    }
    let names = ["tok", "mm", "ss", "combo"];
    let (mut out, mut n, mut chars) = (String::new(), 0usize, pat.chars().peekable());
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '\\' {
            if let Some(x) = chars.next() {
                out.push(x);
            }
        } else if c == '(' && chars.peek() != Some(&'?') {
            out.push_str(&format!("?P<{}>", names.get(n)?));
            n += 1;
        }
    }
    if n != names.len() {
        return None;
    }
    let dict: Vec<String> = pairs.iter().map(|(k, e)| format!("\"{k}\": \"{e}\"")).collect();
    Some(format!("BENCH_KIND_DEFAULT_EXT = {{{}}}\n_BENCH_NAME_RE = _re.compile(r\"{out}\")\n", dict.join(", ")))
}

fn py_bench_kinds(src: &str) -> Option<Vec<String>> {
    let line = src.lines().find(|l| l.starts_with("BENCH_KIND_DEFAULT_EXT = {"))?;
    let body = &line[line.find('{')? + 1..line.rfind('}')?];
    let mut out = Vec::new();
    for item in body.split(',') {
        let k = item.split(':').next()?.trim();
        out.push(k.strip_prefix('"')?.strip_suffix('"')?.to_string());
    }
    Some(out)
}

/// JavaScript (`u` flag) semantics in this engine's syntax: `\d` is ASCII, `.` excludes the JS line terminators,
/// `$` is the end of input (the engine's own `$` without `m`), named groups `(?<x>` become `(?P<x>`.
fn js_to_engine(p: &str) -> String {
    let mut out = String::new();
    let mut chars = p.chars().peekable();
    let mut in_class = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('d') => out.push_str(if in_class { "0-9" } else { "[0-9]" }),
                Some(n) => {
                    out.push('\\');
                    out.push(n);
                }
                None => out.push('\\'),
            },
            '[' if !in_class => {
                in_class = true;
                out.push('[');
            }
            ']' if in_class => {
                in_class = false;
                out.push(']');
            }
            '.' if !in_class => out.push_str("[^\\n\\r\\x{2028}\\x{2029}]"),
            '(' if !in_class && chars.peek() == Some(&'?') => {
                out.push('(');
                out.push(chars.next().unwrap());
                if chars.peek() == Some(&'<') {
                    let mut look = chars.clone();
                    look.next();
                    if !matches!(look.peek(), Some('=') | Some('!')) {
                        out.push('P');
                    }
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn group_names(re: &Re) -> BTreeSet<String> {
    re.raw().capture_names().flatten().map(str::to_string).collect()
}

/// The naming grammar of `repo` (cached per resolved root).
fn grammar(repo: Option<&Path>) -> HR<Arc<Grammar>> {
    let root = core::resolve(&repo.map(Path::to_path_buf).unwrap_or_else(own_checkout));
    let key = core::posix(&root);
    let cache = GRAMMAR_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(g) = cache.lock().unwrap().get(&key) {
        return Ok(g.clone());
    }
    // OWNER-MIRROR: .claude/skills/hint-publisher/scripts/hintlib/core.py::load_owner_module (bench first, then prose).
    // plan_26111615 D4: the bench naming owner is the adversarial-benchmark Rust `bench_naming` module — the only source
    // read (no `.py` · absent = owner missing, fail closed).
    let bench_rs = root.join(REL_BENCH_NAMING_RS);
    if !bench_rs.is_file() {
        return owner_missing(REL_BENCH_NAMING_RS);
    }
    let ts_path = root.join(core::REL_DOC_NAMING_TS);
    if !ts_path.is_file() {
        return owner_missing(core::REL_DOC_NAMING_TS);
    }
    let bench_src = {
        let rs = core::read_text(&bench_rs)
            .or_else(|e| fail("HINT_OWNER_MODULE_IMPORT_FAILED", format!("{REL_BENCH_NAMING_RS} import 실패: {e}"), None))?;
        rs_bench_as_py(&rs).unwrap_or_default()
    };
    let ts_src = core::read_text(&ts_path)
        .or_else(|e| fail("HINT_OWNER_MODULE_IMPORT_FAILED", format!("{} import 실패: {e}", core::REL_DOC_NAMING_TS), None))?;
    let dated_types = ts_dated_types(&ts_src);
    let topic = ts_pattern(&ts_src, "topic");
    let dated = ts_pattern(&ts_src, "dated_doc");
    let report = ts_pattern(&ts_src, "report");
    let mut absent: Vec<&str> = Vec::new();
    if dated_types.is_none() {
        absent.push("DATED_DOC_TYPES");
    }
    if dated.is_none() {
        absent.extend(["_DATED_DOC_RE", "is_dated_doc_basename"]);
    }
    if report.is_none() {
        absent.push("_REPORT_RE");
    }
    if topic.is_none() {
        absent.extend(["simlog_dirname", "validate_topic"]);
    }
    if !absent.is_empty() {
        return fail(
            "HINT_LINEAGE_GRAMMAR_DRIFT",
            format!("{} 에 계보가 쓰는 명명 심볼이 없다: {}", core::REL_DOC_NAMING_TS, J::from(absent).repr()),
            Some(DRIFT_REMEDY),
        );
    }
    let bench_pat = py_bench_name_re(&bench_src);
    let bench_kinds = py_bench_kinds(&bench_src);
    let mut absent: Vec<&str> = Vec::new();
    if bench_pat.is_none() {
        absent.push("_BENCH_NAME_RE");
    }
    if bench_kinds.is_none() {
        absent.push("BENCH_KIND_DEFAULT_EXT");
    }
    if !absent.is_empty() {
        return fail(
            "HINT_LINEAGE_GRAMMAR_DRIFT",
            format!("{} 에 계보가 쓰는 명명 심볼이 없다: {}", REL_BENCH_NAMING_RS, J::from(absent).repr()),
            Some(DRIFT_REMEDY),
        );
    }
    let (dated, report, topic, bench_pat) = (dated.unwrap(), report.unwrap(), topic.unwrap(), bench_pat.unwrap());
    let compile = |p: &str| Re::try_new(p, 0);
    let drift_compile = |what: &str, e: String| -> HintError {
        HintError::new("HINT_LINEAGE_GRAMMAR_DRIFT", format!("명명 SSOT 정규식을 컴파일할 수 없다({what}): {e}"), Some(DRIFT_REMEDY))
    };
    // The Python shim: `re.compile(js.replace("(?<", "(?P<"))`.
    let dated_re = compile(&dated.replace("(?<", "(?P<")).map_err(|e| drift_compile("dated_doc", e))?;
    let dated_js = compile(&js_to_engine(&dated)).map_err(|e| drift_compile("dated_doc", e))?;
    let report_re = compile(&report.replace("(?<", "(?P<")).map_err(|e| drift_compile("report", e))?;
    let topic_js = compile(&js_to_engine(&topic)).map_err(|e| drift_compile("topic", e))?;
    let bench_re = compile(&bench_pat).map_err(|e| drift_compile("_BENCH_NAME_RE", e))?;
    let dg = group_names(&dated_re);
    let bg = group_names(&bench_re);
    if !(dg.contains("hour") && dg.contains("topic")) || !["tok", "mm", "ss", "combo"].iter().all(|n| bg.contains(*n)) {
        return fail(
            "HINT_LINEAGE_GRAMMAR_DRIFT",
            "명명 SSOT 정규식의 그룹 이름이 바뀌었다(hour/topic · tok/mm/ss/combo)",
            Some("hintlib/lineage.py 의 _parse_with 를 새 그룹에 맞춘다."),
        );
    }
    let g = Arc::new(Grammar {
        dated_types: dated_types.unwrap(),
        dated_re,
        dated_js,
        report_re,
        bench_re,
        bench_kinds: bench_kinds.unwrap(),
        topic_js,
    });
    cache.lock().unwrap().insert(key, g.clone());
    Ok(g)
}

impl Grammar {
    /// OWNER-MIRROR: .claude/skills/wiki-desk/src/doc_naming.ts::isDatedDocBasename
    fn is_dated(&self, doc_type: &str, basename: &str) -> bool {
        if !self.dated_types.iter().any(|t| t == doc_type) || basename.is_empty() || basename.contains('/') {
            return false;
        }
        match self.dated_js.match_at_start(basename) {
            Some(c) => pyre::named(&c, "type") == Some(doc_type),
            None => false,
        }
    }
    /// OWNER-MIRROR: .claude/skills/wiki-desk/src/doc_naming.ts::validateTopic
    fn validate_topic(&self, topic: &str) -> Result<(), String> {
        if topic.is_empty() || !self.topic_js.is_search(topic) || topic == "." || topic == ".." {
            return Err(format!(
                "doc_naming: topic must be a non-empty run of [A-Za-z0-9_. 한글] with no path separators/whitespace, got {}",
                crate::value::dumps_unicode(&J::from(topic))
            ));
        }
        Ok(())
    }
    /// OWNER-MIRROR: .claude/skills/wiki-desk/src/doc_naming.ts::simlogDirname (with kstTokens · suffixed)
    fn simlog_dirname(&self, utc: &str, topic: &str, existing: &[String]) -> Result<String, String> {
        self.validate_topic(topic)?;
        let (hour, mm, ss) = js_kst_tokens(utc);
        let base = format!("{hour}_{topic}");
        let with_suffix = format!("{hour}_{mm}_{ss}_{topic}");
        let prefix = format!("{hour}_");
        if !existing.iter().any(|n| n.starts_with(&prefix)) {
            return Ok(base);
        }
        if existing.iter().any(|n| *n == with_suffix) {
            return Err("NamingCollisionExhausted".to_string());
        }
        Ok(with_suffix)
    }
}

/// OWNER-MIRROR: .claude/skills/wiki-desk/src/doc_naming.ts::kstTokens
fn js_kst_tokens(utc: &str) -> (String, String, String) {
    let na = ("NA".to_string(), "00".to_string(), "00".to_string());
    let b = utc.as_bytes();
    let ok_shape = b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18].iter().all(|&i| b[i].is_ascii_digit());
    if !ok_shape {
        return na;
    }
    match core::parse_civil(&utc[..19]) {
        Some(c) if c.year >= 100 && c.second <= 59 => {
            let k = core::Civil::from_epoch(c.epoch() + 9 * 3600);
            (
                format!("{:02}{:02}{:02}{:02}", k.year.rem_euclid(100), k.month, k.day, k.hour),
                format!("{:02}", k.minute),
                format!("{:02}", k.second),
            )
        }
        _ => na,
    }
}

// ── 작은 도우미 ──────────────────────────────────────────────────────────────────────────────
pub fn _strip_ext(name: &str) -> String {
    for ext in STRIP_EXTS {
        if name.ends_with(ext) && name.len() > ext.len() {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    name.to_string()
}

fn ext_of(name: &str) -> String {
    for ext in STRIP_EXTS {
        if name.ends_with(ext) && name.len() > ext.len() {
            return ext.to_string();
        }
    }
    String::new()
}

/// `_char_at(text, i)` on a byte offset (always a char boundary here).
fn char_at(text: &str, i: usize) -> Option<char> {
    text.get(i..).and_then(|s| s.chars().next())
}

fn is_name_char(ch: Option<char>) -> bool {
    matches!(ch, Some(c) if py_isalnum(c) || NAME_EXTRA.contains(c))
}

fn ext_at(text: &str, i: usize) -> String {
    if char_at(text, i) != Some('.') {
        return String::new();
    }
    let mut j = i + 1;
    while let Some(c) = char_at(text, j) {
        if c.is_ascii_alphanumeric() {
            j += 1;
        } else {
            break;
        }
    }
    text[i..j].to_string()
}

fn excluded_in<S: AsRef<str>>(rel_path: &str, exclude: &[S]) -> bool {
    for e in exclude {
        let e = e.as_ref();
        if e.ends_with('/') {
            if rel_path == e.trim_end_matches('/') || rel_path.starts_with(e) {
                return true;
            }
        } else if rel_path == e || rel_path.starts_with(&format!("{e}/")) {
            return true;
        }
    }
    false
}

fn excluded(rel_path: &str) -> bool {
    excluded_in(rel_path, &EXCLUDED_ROOTS)
}

/// `_norm_key(s)`.
pub fn _norm_key(s: &str) -> String {
    let low = py_strip(s).to_lowercase();
    strip_chars(&norm_key_re().sub_literal(&low, "_"), "_").to_string()
}

fn norm_val(v: &J) -> String {
    py_strip(&pystr(v)).to_lowercase()
}

pub fn _within_ceiling(yymmddhh: Option<&str>, publish_kst: &str) -> bool {
    match yymmddhh {
        None => true,
        Some(y) => y <= publish_kst,
    }
}

fn ascii_num(s: &str) -> Option<u32> {
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

fn kst_parts_to_utc(tt: &str, mm: &str, ss: &str) -> Option<String> {
    let t: Vec<char> = tt.chars().collect();
    if t.len() != 8 {
        return None;
    }
    let part = |a: usize, b: usize| ascii_num(&t[a..b].iter().collect::<String>());
    let (yy, mo, d, h) = (part(0, 2)?, part(2, 4)?, part(4, 6)?, part(6, 8)?);
    let (mi, s) = (ascii_num(mm)?, ascii_num(ss)?);
    let text = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", 2000 + yy, mo, d, h, mi, s);
    let c = core::parse_civil(&text)?;
    if c.second > 59 {
        return None;
    }
    Some(core::Civil::from_epoch(c.epoch() - 9 * 3600).iso() + "Z")
}

fn simlog_conformant(g: &Grammar, name: &str, tt: &str, mm: Option<&str>, ss: Option<&str>, topic: &str) -> bool {
    if g.validate_topic(topic).is_err() {
        return false;
    }
    let Some(utc) = kst_parts_to_utc(tt, mm.unwrap_or("00"), ss.unwrap_or("00")) else { return false };
    let existing: Vec<String> = if mm.is_none() { vec![] } else { vec![format!("{tt}_{topic}")] };
    match g.simlog_dirname(&utc, topic, &existing) {
        Ok(n) => n == name,
        Err(_) => false,
    }
}

/// `_ev(ev, name)`.
fn ev_get<'a>(ev: &'a J, name: &str) -> &'a J {
    ev.get(name).unwrap_or(&J::Null)
}

// ── 문서 이름 파싱 ────────────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug, Default)]
struct Info {
    kind: Option<String>,
    class: Option<String>,
    prefix: Option<String>,
    token: Option<String>,
    yymmddhh: Option<String>,
    mmss: Option<String>,
    topic: Option<String>,
    legacy: bool,
    stem: String,
    ext: String,
}

impl Info {
    fn to_j(&self) -> J {
        jd! {
            "kind" => self.kind.clone(), "class" => self.class.clone(), "prefix" => self.prefix.clone(),
            "token" => self.token.clone(), "yymmddhh" => self.yymmddhh.clone(), "mmss" => self.mmss.clone(),
            "topic" => self.topic.clone(), "legacy" => self.legacy, "stem" => self.stem.clone(), "ext" => self.ext.clone(),
        }
    }
}

fn sub_chars(s: &str, a: usize, b: Option<usize>) -> String {
    let it = s.chars().skip(a);
    match b {
        Some(b) => it.take(b.saturating_sub(a)).collect(),
        None => it.collect(),
    }
}

fn parse_with(g: &Grammar, path: &str) -> Info {
    let p = path.replace('\\', "/");
    let name = pname(&p);
    let parent = pparent_name(&p);
    let stem = _strip_ext(&name);
    let mut out = Info { stem: stem.clone(), ext: ext_of(&name), ..Default::default() };
    if g.dated_types.iter().any(|t| *t == parent) {
        let m = g.dated_re.match_at_start(&name);
        if let (Some(m), true) = (&m, g.is_dated(&parent, &name)) {
            let hour = m.name("hour").unwrap();
            let topic = m.name("topic").unwrap();
            let mid = &name[hour.end()..topic.start()];
            let mmss = strip_chars(mid, "_");
            out.kind = Some(parent.clone());
            out.prefix = Some(parent.clone());
            out.token = Some(hour.as_str().to_string());
            out.yymmddhh = Some(hour.as_str().to_string());
            out.mmss = if mmss.is_empty() { None } else { Some(mmss.to_string()) };
            out.topic = Some(topic.as_str().to_string());
        } else if name.ends_with(".md") {
            out.kind = Some(parent.clone());
            out.legacy = true;
            out.topic = Some(stem.clone());
        }
        return out;
    }
    if parent == "report" {
        if !DOC_EXTS.iter().any(|e| name.ends_with(e)) {
            return out;
        }
        if g.report_re.is_match(&stem) {
            let (cls, rest) = stem.split_once('_').unwrap();
            let tt = sub_chars(rest, 0, Some(8));
            let rest2 = sub_chars(rest, 9, None);
            out.kind = Some("report".into());
            out.class = Some(cls.to_string());
            out.prefix = Some(cls.to_string());
            out.token = Some(tt.clone());
            out.yymmddhh = Some(tt);
            match report_mmss_re().match_at_start(&rest2) {
                Some(mm) => {
                    out.mmss = Some(format!("{}_{}", &mm[1], &mm[2]));
                    out.topic = Some(mm[3].to_string());
                }
                None => {
                    out.mmss = None;
                    out.topic = Some(rest2);
                }
            }
        } else {
            out.kind = Some("report".into());
            out.legacy = true;
            out.topic = Some(stem.clone());
        }
        return out;
    }
    if parent == "benchmark" {
        if name.starts_with('.') {
            return out;
        }
        if let Some(m) = g.bench_re.match_at_start(&name) {
            let tokm = m.name("tok").unwrap();
            let kraw = &name[..tokm.start() - 1];
            if g.bench_kinds.iter().any(|k| k == kraw) {
                let tok = tokm.as_str().to_string();
                out.kind = Some(if kraw == "benchmark" { "certificate".to_string() } else { kraw.to_string() });
                out.prefix = Some(kraw.to_string());
                out.yymmddhh = if tok == "NA" { None } else { Some(tok.clone()) };
                out.token = Some(tok);
                out.mmss = m.name("mm").map(|mm| format!("{}_{}", mm.as_str(), m.name("ss").map(|x| x.as_str()).unwrap_or("")));
                out.topic = m.name("combo").map(|x| x.as_str().to_string());
            }
            return out;
        }
        if let Some(m) = sweep_map_re().match_at_start(&name) {
            out.kind = Some(if &m[5] == "md" { "sweep_map".into() } else { "sweep_json".into() });
            out.prefix = Some(SWEEP_MAP_PREFIX.into());
            out.token = Some(m[1].to_string());
            out.yymmddhh = Some(m[1].to_string());
            out.mmss = m.get(2).map(|a| format!("{}_{}", a.as_str(), &m[3]));
            out.topic = Some(m[4].to_string());
        }
        return out;
    }
    if parent == "simlog" {
        if let Some(m) = simlog_modern_re().match_at_start(&name) {
            let tt = m[1].to_string();
            let mm = m.get(2).map(|x| x.as_str().to_string());
            let ss = m.get(3).map(|x| x.as_str().to_string());
            let topic = m[4].to_string();
            out.kind = Some("simlog".into());
            out.prefix = Some("simlog".into());
            out.token = Some(tt.clone());
            out.yymmddhh = Some(tt.clone());
            out.mmss = mm.as_ref().map(|a| format!("{a}_{}", ss.clone().unwrap_or_default()));
            out.legacy = !simlog_conformant(g, &name, &tt, mm.as_deref(), ss.as_deref(), &topic);
            out.topic = Some(topic);
            return out;
        }
        if let Some(m) = simlog_legacy_re().match_at_start(&name) {
            out.kind = Some("simlog".into());
            out.prefix = Some("simlog-legacy".into());
            out.token = Some(m[1].to_string());
            out.yymmddhh = Some(sub_chars(&m[1], 2, None));
            out.topic = Some(m[3].to_string());
            out.legacy = true;
        }
        return out;
    }
    out
}

/// `parse_doc_name(path, repo=...)`.
pub fn parse_doc_name(path: &str, repo: Option<&Path>) -> HR<J> {
    let g = grammar(repo)?;
    Ok(parse_with(&g, path).to_j())
}

// ── 코퍼스 ─────────────────────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
struct Node {
    kind: String,
    klass: Option<String>,
    prefix: Option<String>,
    token: Option<String>,
    yymmddhh: Option<String>,
    mmss: Option<String>,
    stem: String,
    ext: String,
    #[allow(dead_code)]
    is_dir: bool,
}

impl Node {
    fn is_document(&self) -> bool {
        DOCUMENT_KINDS.contains(&self.kind.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Mention {
    to: String,
    edge: String,
    token: String,
    resolution: String,
}

/// (start, targets, resolution, token)
type Resolved = (usize, Vec<String>, &'static str, String);

/// A compiled filter pattern: the Python pattern text + the compiled engine regex.
pub struct Pat {
    pub pattern: String,
    re: Re,
}

impl Pat {
    pub fn search(&self, s: &str) -> bool {
        self.re.is_search(s)
    }
}

type AxisPat = (String, Rc<Re>);

fn banner_line_indices(lines: &[&str]) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    let n = lines.len().min(HEADER_LINES);
    let mut i = 0;
    while i < n {
        if supersede_mark_re().is_search(lines[i]) {
            out.insert(i);
            if py_lstrip(lines[i]).starts_with('>') {
                let mut j = i + 1;
                while j < n {
                    let l = py_lstrip(lines[j]);
                    if l.starts_with('>') && !py_strip(&l[1..]).is_empty() {
                        out.insert(j);
                        j += 1;
                    } else {
                        break;
                    }
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

struct Excl {
    from: BTreeSet<String>,
    token: String,
    reason: String,
}

struct SiblingFilter {
    parts: BTreeSet<String>,
    keys: Vec<String>,
    pats: Vec<AxisPat>,
    excluded: BTreeMap<String, Excl>,
    kept: BTreeSet<String>,
}

impl SiblingFilter {
    fn new(key_strings: &[J]) -> SiblingFilter {
        let keys: BTreeSet<String> = key_strings
            .iter()
            .filter(|k| !k.is_null() && clen(py_strip(&pystr(k))) >= 2)
            .map(|k| py_strip(&pystr(k)).to_lowercase())
            .collect();
        let parts: BTreeSet<String> =
            keys.iter().flat_map(|k| sep_split_re().split(k)).filter(|t| clen(t) >= 2).collect();
        let all: BTreeSet<String> = keys.union(&parts).cloned().collect();
        let keys: Vec<String> = all.into_iter().collect();
        let pats = axis_patterns(keys.iter().map(|k| J::Str(k.clone())).collect::<Vec<_>>().as_slice());
        SiblingFilter { parts, keys, pats, excluded: BTreeMap::new(), kept: BTreeSet::new() }
    }
}

fn axis_patterns(tokens: &[J]) -> Vec<AxisPat> {
    let set: BTreeSet<String> = tokens
        .iter()
        .filter(|t| !t.is_null() && clen(py_strip(&pystr(t))) >= 2)
        .map(|t| py_strip(&pystr(t)).to_lowercase())
        .collect();
    let mut out = Vec::new();
    for t in set {
        let body: Vec<String> = dash_us_re().split(&t).into_iter().filter(|p| !p.is_empty()).map(|p| rx_escape(&p)).collect();
        if !body.is_empty() {
            let p = format!("(?<![0-9a-z]){}(?![0-9a-z])", body.join("[-_]"));
            out.push((t, Rc::new(Re::new(&p, pyre::I))));
        }
    }
    out
}

struct Corpus {
    repo: PathBuf,
    g: Arc<Grammar>,
    exclude: Vec<String>,
    sf: RefCell<Option<SiblingFilter>>,
    nodes: BTreeMap<String, Node>,
    by_key: HashMap<(String, String), Vec<String>>,
    undated: HashMap<String, Vec<String>>,
    roots_absent: Vec<String>,
    text_cache: RefCell<HashMap<String, String>>,
    mentions_cache: RefCell<HashMap<String, Vec<Mention>>>,
    narrators_cache: RefCell<Option<BTreeMap<String, Vec<(String, String, String)>>>>,
    slug_cache: RefCell<HashMap<(String, String), bool>>,
    stem_re: Re,
}

/// The entries below `d` (`Path.rglob("*")`: symlinked directories are listed, not entered), in Python 3.12 `Path`
/// order (by parts).
fn rglob(d: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            out.push(p.clone());
            if let Ok(md) = std::fs::symlink_metadata(&p) {
                if md.is_dir() {
                    walk(&p, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(d, &mut out);
    out.sort_by(|a, b| {
        let pa: Vec<String> = a.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        let pb: Vec<String> = b.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        pa.cmp(&pb)
    });
    out
}

/// Relative posix path of `f` below `base` (both built from the same root).
fn rel_below(f: &Path, base: &Path) -> Vec<String> {
    f.strip_prefix(base)
        .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect())
        .unwrap_or_default()
}

fn join_rel(prefix: &str, sub: &[String]) -> String {
    let s = sub.join("/");
    if prefix.is_empty() || prefix == "." {
        s
    } else {
        format!("{prefix}/{s}")
    }
}

impl Corpus {
    fn new<S: AsRef<str>, E: AsRef<str>>(repo: &Path, roots: &[S], exclude: &[E]) -> HR<Corpus> {
        let g = grammar(Some(repo))?;
        let mut c = Corpus {
            repo: repo.to_path_buf(),
            g: g.clone(),
            exclude: exclude.iter().map(|e| e.as_ref().to_string()).collect(),
            sf: RefCell::new(None),
            nodes: BTreeMap::new(),
            by_key: HashMap::new(),
            undated: HashMap::new(),
            roots_absent: Vec::new(),
            text_cache: RefCell::new(HashMap::new()),
            mentions_cache: RefCell::new(HashMap::new()),
            narrators_cache: RefCell::new(None),
            slug_cache: RefCell::new(HashMap::new()),
            stem_re: Re::new("x", 0),
        };
        for root in roots {
            let root = strip_chars(root.as_ref(), "/").to_string();
            if excluded_in(&root, &c.exclude) {
                continue;
            }
            let d = repo.join(&root);
            if !d.is_dir() {
                c.roots_absent.push(root);
                continue;
            }
            let dir_nodes = pname(&root) == "simlog";
            let mut children: Vec<(String, bool)> = std::fs::read_dir(&d)
                .map(|rd| rd.flatten().map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().is_dir())).collect())
                .unwrap_or_default();
            children.sort();
            for (name, is_dir) in children {
                if name.starts_with('.') || EXCLUDED_BASENAMES.contains(&name.as_str()) {
                    continue;
                }
                if dir_nodes != is_dir {
                    continue;
                }
                c.add(&format!("{root}/{name}"));
            }
        }
        let mut prefixes: BTreeSet<String> = g.dated_types.iter().cloned().collect();
        prefixes.extend(g.bench_kinds.iter().cloned());
        prefixes.insert(SWEEP_MAP_PREFIX.to_string());
        prefixes.extend(c.nodes.values().filter_map(|n| n.klass.clone()));
        let mut pv: Vec<String> = prefixes.into_iter().collect();
        pv.sort_by(|a, b| clen(b).cmp(&clen(a)).then(a.cmp(b)));
        let alt: Vec<String> = pv.iter().map(|p| rx_escape(p)).collect();
        c.stem_re = Re::new(
            &format!(r"(?<![A-Za-z0-9_])(?:docs/(?P<dir>[a-z_]+)/)?(?P<pfx>{})_(?P<tt>\d{{8}}|NA)(?![0-9])", alt.join("|")),
            0,
        );
        Ok(c)
    }

    fn add(&mut self, rel_path: &str) -> Option<Node> {
        if let Some(n) = self.nodes.get(rel_path) {
            return Some(n.clone());
        }
        if excluded_in(rel_path, &self.exclude) {
            return None;
        }
        let info = parse_with(&self.g, rel_path);
        let kind = info.kind.clone()?;
        let is_dir = self.repo.join(rel_path).is_dir();
        if (kind == "simlog") != is_dir {
            return None;
        }
        let node = Node {
            kind,
            klass: info.class,
            prefix: info.prefix,
            token: info.token,
            yymmddhh: info.yymmddhh,
            mmss: info.mmss,
            stem: info.stem,
            ext: info.ext,
            is_dir,
        };
        match (&node.prefix, &node.token) {
            (Some(p), Some(t)) if !p.is_empty() && !t.is_empty() => {
                self.by_key.entry((p.clone(), t.clone())).or_default().push(rel_path.to_string());
            }
            _ => self.undated.entry(pparent_name(rel_path)).or_default().push(rel_path.to_string()),
        }
        self.nodes.insert(rel_path.to_string(), node.clone());
        Some(node)
    }

    fn ensure(&mut self, rel_path: &str) -> Option<Node> {
        if !self.repo.join(rel_path).exists() {
            return None;
        }
        self.add(rel_path)
    }

    fn text(&self, rel_path: &str) -> HR<String> {
        if let Some(t) = self.text_cache.borrow().get(rel_path) {
            return Ok(t.clone());
        }
        let p = self.repo.join(rel_path);
        let t = if p.is_dir() {
            String::new()
        } else {
            match read_text_replace(&p) {
                Ok(t) => t,
                Err(e) => {
                    return fail(
                        "HINT_LINEAGE_DOC_UNREADABLE",
                        format!("계보 입력 문서를 읽을 수 없다: {rel_path}: {}", core::io_err_text(&e, &p)),
                        None,
                    )
                }
            }
        };
        self.text_cache.borrow_mut().insert(rel_path.to_string(), t.clone());
        Ok(t)
    }

    fn slug_hit(&self, rel_path: &str, pat: &Pat) -> HR<bool> {
        let key = (rel_path.to_string(), pat.pattern.clone());
        if let Some(v) = self.slug_cache.borrow().get(&key) {
            return Ok(*v);
        }
        let p = self.repo.join(rel_path);
        let mut hit = false;
        if p.is_dir() {
            for f in rglob(&p) {
                let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if f.is_file() && TEXT_SUFFIXES.contains(&py_suffix(&name)) {
                    match read_text_replace(&f) {
                        Ok(t) => {
                            if pat.search(&t) {
                                hit = true;
                                break;
                            }
                        }
                        Err(e) => {
                            return fail(
                                "HINT_LINEAGE_DOC_UNREADABLE",
                                format!("계보 후보를 읽을 수 없다: {}: {}", core::posix(&f), core::io_err_text(&e, &f)),
                                None,
                            )
                        }
                    }
                }
            }
        } else {
            hit = pat.search(&self.text(rel_path)?);
        }
        self.slug_cache.borrow_mut().insert(key, hit);
        Ok(hit)
    }

    fn pick(&self, text: &str, start: usize, after_tt: usize, cands: &[String]) -> Option<Resolved> {
        let mut sorted: Vec<&String> = cands.iter().collect();
        sorted.sort_by(|a, b| clen(&self.nodes[*b].stem).cmp(&clen(&self.nodes[*a].stem)).then(a.cmp(b)));
        for c in sorted {
            let st = &self.nodes[c].stem;
            if text[start..].starts_with(st.as_str()) && !is_name_char(char_at(text, start + st.len())) {
                let mut twins: Vec<String> = cands.iter().filter(|x| self.nodes[*x].stem == *st).cloned().collect();
                if twins.len() > 1 {
                    let ext = ext_at(text, start + st.len());
                    let f: Vec<String> = twins.iter().filter(|x| self.nodes[*x].ext == ext).cloned().collect();
                    if !f.is_empty() {
                        twins = f;
                    }
                }
                twins.sort();
                return Some((start, twins, "exact", st.clone()));
            }
        }
        let mut pos = after_tt;
        let mut mmss: Option<String> = None;
        if let Some(m) = mmss_at_re().match_at_start(&text[pos..]) {
            mmss = Some(format!("{}_{}", &m[1], &m[2]));
            pos += m.get(0).unwrap().end();
        }
        let topic_carrying = char_at(text, pos) == Some('_') && is_name_char(char_at(text, pos + 1));
        let mut sib: Vec<String> =
            cands.iter().filter(|c| mmss.is_none() || self.nodes[*c].mmss == mmss).cloned().collect();
        sib.sort();
        if sib.is_empty() {
            return None;
        }
        if topic_carrying {
            let mut end = pos + 1;
            while let Some(ch) = char_at(text, end) {
                let next = char_at(text, end + ch.len_utf8());
                if is_name_char(Some(ch)) || (ch == '.' && is_name_char(next)) {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            return Some((start, sib, "ambiguous", text[start..end].to_string()));
        }
        let res = if sib.len() == 1 { "unique" } else { "ambiguous" };
        Some((start, sib, res, text[start..pos].to_string()))
    }

    fn resolve_text(&self, text: &str) -> Vec<Resolved> {
        let mut out: Vec<Resolved> = Vec::new();
        for m in self.stem_re.finditer(text) {
            let pfx = m.name("pfx").unwrap();
            let tt = m.name("tt").unwrap();
            let mut cands: Vec<String> =
                self.by_key.get(&(pfx.as_str().to_string(), tt.as_str().to_string())).cloned().unwrap_or_default();
            if let Some(dir) = m.name("dir") {
                let pre = format!("docs/{}/", dir.as_str());
                cands.retain(|c| c.starts_with(&pre));
            }
            if !cands.is_empty() {
                if let Some(r) = self.pick(text, pfx.start(), tt.end(), &cands) {
                    out.push(r);
                }
            }
        }
        for m in simlog_token_re().finditer(text) {
            let tt = m.name("tt").unwrap();
            let prefix = if clen(tt.as_str()) == 10 { "simlog-legacy" } else { "simlog" };
            let cands = self.by_key.get(&(prefix.to_string(), tt.as_str().to_string())).cloned().unwrap_or_default();
            if !cands.is_empty() {
                if let Some(r) = self.pick(text, tt.start(), tt.end(), &cands) {
                    out.push(r);
                }
            }
        }
        for m in undated_path_re().finditer(text) {
            let mut paths = self.undated.get(m.name("dir").unwrap().as_str()).cloned().unwrap_or_default();
            let start = m.name("rest").unwrap().start();
            paths.sort_by(|a, b| clen(&pname(b)).cmp(&clen(&pname(a))).then(a.cmp(b)));
            for c in paths {
                let mut hit = None;
                for cand_name in [pname(&c), self.nodes[&c].stem.clone()] {
                    if text[start..].starts_with(cand_name.as_str()) && !is_name_char(char_at(text, start + cand_name.len())) {
                        hit = Some(cand_name);
                        break;
                    }
                }
                if let Some(h) = hit {
                    out.push((start, vec![c], "exact", h));
                    break;
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.3.cmp(&b.3)));
        out
    }

    fn sf_keep(&self, src: &str, token: &str, members: &BTreeSet<String>) -> HR<BTreeSet<String>> {
        let (has_pats, parts, keys) = {
            let sf = self.sf.borrow();
            let sf = sf.as_ref().unwrap();
            (!sf.pats.is_empty(), sf.parts.clone(), sf.keys.clone())
        };
        if members.len() < 2 || !has_pats {
            self.sf.borrow_mut().as_mut().unwrap().kept.extend(members.iter().cloned());
            return Ok(members.clone());
        }
        let typed = split_once_re(typed_split_re(), token);
        let extra: Vec<AxisPat> = if typed.len() == 2 && !typed[1].is_empty() {
            let toks: Vec<J> = sep_split_re()
                .split(&typed[1].to_lowercase())
                .into_iter()
                .filter(|t| clen(t) >= 2)
                .map(J::Str)
                .collect();
            axis_patterns(&toks)
        } else {
            Vec::new()
        };
        let mut scores: BTreeMap<String, (i64, i64)> = BTreeMap::new();
        for m in members {
            let info = parse_with(&self.g, m);
            let topic = info
                .topic
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| Some(info.stem.clone()).filter(|s| !s.is_empty()))
                .unwrap_or_else(|| pname(m))
                .to_lowercase();
            let head = if self.repo.join(m).is_dir() {
                String::new()
            } else {
                let t = self.text(m)?;
                t.split('\n').take(HEADER_LINES).collect::<Vec<_>>().join("\n")
            };
            let sf = self.sf.borrow();
            let pats = sf.as_ref().unwrap().pats.iter().chain(extra.iter());
            let hits = pats.filter(|p| p.1.is_search(&topic) || p.1.is_search(&head)).count() as i64;
            let mut all_parts = parts.clone();
            all_parts.extend(extra.iter().map(|(t, _)| t.clone()));
            let foreign =
                sep_split_re().split(&topic).iter().filter(|t| clen(t) >= 2 && !all_parts.contains(*t)).count() as i64;
            scores.insert(m.clone(), (hits, foreign));
        }
        let best_h = scores.values().map(|(h, _)| *h).max().unwrap_or(0);
        let mut sf = self.sf.borrow_mut();
        let sf = sf.as_mut().unwrap();
        if best_h == 0 {
            sf.kept.extend(members.iter().cloned());
            return Ok(members.clone());
        }
        let best_f = scores.values().filter(|(h, _)| *h == best_h).map(|(_, f)| *f).min().unwrap_or(0);
        let kept: BTreeSet<String> =
            scores.iter().filter(|(_, (h, f))| *h == best_h && *f == best_f).map(|(m, _)| m.clone()).collect();
        sf.kept.extend(kept.iter().cloned());
        for m in members {
            if kept.contains(m) {
                continue;
            }
            let (h, f) = scores[m];
            let reason = format!(
                "모호 해소(`{token}` · 형제 {}건) — 셀 키 적중 {h}·이질 토큰 {f} < 최선 형제 {best_h}·{best_f}(셀 키 {})",
                members.len(),
                J::from(keys.clone()).repr()
            );
            match sf.excluded.get_mut(m) {
                None => {
                    let mut from = BTreeSet::new();
                    from.insert(src.to_string());
                    sf.excluded.insert(m.clone(), Excl { from, token: token.to_string(), reason });
                }
                Some(rec) => {
                    if (token, reason.as_str()) < (rec.token.as_str(), rec.reason.as_str()) {
                        rec.token = token.to_string();
                        rec.reason = reason;
                    }
                    rec.from.insert(src.to_string());
                }
            }
        }
        Ok(kept)
    }

    fn mentions(&self, rel_path: &str) -> HR<Vec<Mention>> {
        if let Some(v) = self.mentions_cache.borrow().get(rel_path) {
            return Ok(v.clone());
        }
        let node = self.nodes.get(rel_path);
        if node.map_or(true, |n| !n.is_document()) {
            self.mentions_cache.borrow_mut().insert(rel_path.to_string(), Vec::new());
            return Ok(Vec::new());
        }
        let text = self.text(rel_path)?;
        let lines: Vec<&str> = text.split('\n').collect();
        let mut starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        let banner = banner_line_indices(&lines);
        let mut seen: BTreeSet<(String, String, String, String)> = BTreeSet::new();
        for (start, targets, res, token) in self.resolve_text(&text) {
            let li = starts.partition_point(|&s| s <= start) - 1;
            let edge = if banner.contains(&li) {
                "supersede".to_string()
            } else if li < HEADER_LINES {
                let mut edge = "header".to_string();
                if let Some(lm) = header_label_re().match_at_start(lines[li]) {
                    let lm_end = lm.get(0).unwrap().end();
                    let tok_at = start as i64 - starts[li] as i64 - lm_end as i64;
                    if tok_at >= 0 {
                        let line = lines[li];
                        let hi = (lm_end + tok_at as usize).min(line.len());
                        let gap = line[lm_end..hi].replace('`', "");
                        let gap = py_strip(&gap);
                        if gap.is_empty() || docs_dir_prefix_re().is_full(gap) {
                            edge = format!("header:{}", &lm[1]);
                        }
                    }
                }
                edge
            } else {
                "mention".to_string()
            };
            for t in targets {
                if t != rel_path {
                    seen.insert((t, edge.clone(), token.clone(), res.to_string()));
                }
            }
        }
        if self.sf.borrow().is_some() {
            let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for (t, _e, k, r) in &seen {
                if r == "ambiguous" {
                    groups.entry(k.clone()).or_default().insert(t.clone());
                }
            }
            let mut drop: BTreeSet<(String, String)> = BTreeSet::new();
            for (k, members) in &groups {
                let keep = self.sf_keep(rel_path, k, members)?;
                for m in members.difference(&keep) {
                    drop.insert((m.clone(), k.clone()));
                }
            }
            seen.retain(|row| !(row.3 == "ambiguous" && drop.contains(&(row.0.clone(), row.2.clone()))));
        }
        let out: Vec<Mention> =
            seen.into_iter().map(|(to, edge, token, resolution)| Mention { to, edge, token, resolution }).collect();
        self.mentions_cache.borrow_mut().insert(rel_path.to_string(), out.clone());
        Ok(out)
    }

    fn output_mentions(&self, rel_path: &str) -> HR<Vec<String>> {
        match self.nodes.get(rel_path) {
            Some(n) if n.is_document() => {}
            _ => return Ok(Vec::new()),
        }
        let mut found = BTreeSet::new();
        let text = self.text(rel_path)?;
        for m in output_path_re().finditer(&text) {
            let s = rstrip_chars(rstrip_chars(m.get(0).unwrap().as_str(), ".,:;…·"), "/").to_string();
            if s.split('/').any(|x| x == "..") || excluded_in(&s, &self.exclude) {
                continue;
            }
            if self.repo.join(&s).exists() {
                found.insert(s);
            }
        }
        Ok(found.into_iter().collect())
    }

    fn narrators(&self) -> HR<BTreeMap<String, Vec<(String, String, String)>>> {
        if let Some(n) = self.narrators_cache.borrow().as_ref() {
            return Ok(n.clone());
        }
        let mut idx: BTreeMap<String, BTreeSet<(String, String, String)>> = BTreeMap::new();
        for (p, n) in &self.nodes {
            if n.kind != "devlog" {
                continue;
            }
            for e in self.mentions(p)? {
                if self.nodes[&e.to].kind == "testlog" {
                    idx.entry(e.to.clone()).or_default().insert((p.clone(), e.token.clone(), e.resolution.clone()));
                }
            }
        }
        let out: BTreeMap<String, Vec<(String, String, String)>> =
            idx.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect();
        *self.narrators_cache.borrow_mut() = Some(out.clone());
        Ok(out)
    }
}

fn mention_j(e: &Mention) -> J {
    jd! {"to" => e.to.clone(), "edge" => e.edge.clone(), "token" => e.token.clone(), "resolution" => e.resolution.clone()}
}

// ── 공개: 그래프 · 배너 ─────────────────────────────────────────────────────────────────────────
/// `mention_graph(repo, roots=..., exclude=...)`.
pub fn mention_graph(repo: &Path, roots: &[String], exclude: &[String]) -> HR<J> {
    let corpus = Corpus::new(repo, roots, exclude)?;
    let mut out = Map::new();
    for (p, n) in &corpus.nodes {
        if !n.is_document() {
            continue;
        }
        let m = corpus.mentions(p)?;
        if !m.is_empty() {
            out.insert(p.clone(), J::List(m.iter().map(mention_j).collect()));
        }
    }
    Ok(J::Dict(out))
}

pub fn supersede_banners(path: &Path) -> HR<Vec<String>> {
    let text = match read_text_replace(path) {
        Ok(t) => t,
        Err(e) => {
            return fail(
                "HINT_LINEAGE_DOC_UNREADABLE",
                format!("배너를 읽을 수 없다: {}: {}", core::posix(path), core::io_err_text(&e, path)),
                None,
            )
        }
    };
    let lines: Vec<&str> = text.split('\n').take(HEADER_LINES).collect();
    let self_stem = _strip_ext(&path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let mut out: Vec<String> = Vec::new();
    for i in banner_line_indices(&lines) {
        for m in generic_doc_token_re().finditer(lines[i]) {
            let tok = _strip_ext(rstrip_chars(m.name("tok").unwrap().as_str(), ".,:;…·"));
            if !tok.is_empty() && tok != self_stem && !out.contains(&tok) {
                out.push(tok);
            }
        }
        for m in simlog_token_re().finditer(lines[i]) {
            let tt = m.name("tt").unwrap();
            let tok = match simlog_tail_re().match_at_start(&lines[i][tt.start()..]) {
                Some(t) => rstrip_chars(t.get(0).unwrap().as_str(), ".,:;…·").to_string(),
                None => tt.as_str().to_string(),
            };
            if !out.contains(&tok) {
                out.push(tok);
            }
        }
    }
    Ok(out)
}

// ── 발행 기록 → 시드 ──────────────────────────────────────────────────────────────────────────
pub fn load_publication_records(repo: &Path) -> HR<Vec<J>> {
    let d = repo.join(core::REL_EVIDENCE_DIR);
    let mut out = Vec::new();
    if !d.is_dir() {
        return Ok(out);
    }
    let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(&d)
        .map(|rd| {
            rd.flatten()
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
                .filter(|(n, _)| n.ends_with(".json") && n.len() > 0)
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for (name, f) in files {
        if name.ends_with(".work-manifest.json") {
            continue;
        }
        let stem = name[..name.len() - ".json".len()].to_string();
        let data = match read_text_strict(&f) {
            Ok(t) => match crate::value::loads(&t) {
                Ok(v) => v,
                Err(e) => {
                    out.push(jd! {"_id" => stem, "_unreadable" => format!("JSONDecodeError: {e}")});
                    continue;
                }
            },
            Err((ty, msg)) => {
                out.push(jd! {"_id" => stem, "_unreadable" => format!("{ty}: {msg}")});
                continue;
            }
        };
        match data {
            J::Dict(mut rec) => {
                rec.insert("_id", stem);
                out.push(J::Dict(rec));
            }
            other => {
                out.push(jd! {"_id" => stem, "_unreadable" => format!("최상위가 객체가 아니다({})", other.type_name())});
            }
        }
    }
    Ok(out)
}

fn record_pointers(rec: &J) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    if let Some(ns) = rec.get("narrative_status").and_then(J::as_dict) {
        let mut keys: Vec<&str> = ns.keys().collect();
        keys.sort();
        for k in keys {
            if let Some(s) = ns.get(k).and_then(|v| v.get_str("source")) {
                out.push((format!("narrative_status.{k}"), s.to_string(), String::new()));
            }
        }
    }
    for group in ["scaffolded", "raw_log_paths"] {
        if let Some(g) = rec.get(group).and_then(J::as_dict) {
            let mut keys: Vec<&str> = g.keys().collect();
            keys.sort();
            for k in keys {
                if let Some(s) = g.get(k).and_then(J::as_str) {
                    out.push((format!("{group}.{k}"), s.to_string(), String::new()));
                }
            }
        }
    }
    if let Some(ev) = rec.get("evidence").and_then(J::as_dict) {
        let mut keys: Vec<&str> = ev.keys().collect();
        keys.sort();
        for k in keys {
            if let Some(s) = ev.get(k).and_then(|v| v.get_str("path")) {
                out.push((format!("evidence.{k}"), s.to_string(), core::REL_EVIDENCE_DIR.to_string()));
            }
        }
    }
    out
}

/// `_norm_pointer(repo, raw, base)` → (path | None, reason | None).
pub fn _norm_pointer(repo: &Path, raw: &str, base: &str) -> (Option<String>, Option<String>) {
    let s = py_strip(raw);
    if s.is_empty() {
        return (None, Some("empty".into()));
    }
    if s.starts_with('/') {
        return match core::rel(repo, Path::new(s)) {
            Ok(r) => (Some(r), None),
            Err(_) => (None, Some("outside-repo".into())),
        };
    }
    let s = normpath(&if base.is_empty() { s.to_string() } else { pjoin(base, s) });
    if s == ".." || s.starts_with("../") {
        return (None, Some("outside-repo".into()));
    }
    (Some(s), None)
}

fn identity_eq(a: &J, b: &J) -> bool {
    let (Some(a), Some(b)) = (a.as_dict(), b.as_dict()) else { return false };
    for f in IDENTITY_MATCH_FIELDS {
        if none_or_empty(a.get(f)) || none_or_empty(b.get(f)) {
            return false;
        }
        if norm_val(a.get(f).unwrap()) != norm_val(b.get(f).unwrap()) {
            return false;
        }
    }
    true
}

pub fn identity_equal(a: &J, b: &J) -> HR<bool> {
    Ok(identity_eq(a, b))
}

fn record_kst(rec: &J) -> HR<Option<String>> {
    match rec.get("generated_utc") {
        Some(J::Str(u)) => match core::kst_token(u) {
            Ok(k) => Ok(Some(k)),
            Err(e) if e.is_py() => Err(e),
            Err(_) => Ok(None),
        },
        _ => Ok(None),
    }
}

/// `r.get("_id") or r.get("publication_id")`.
fn rec_id(r: &J) -> J {
    match r.get("_id") {
        Some(v) if v.truthy() => v.clone(),
        _ => r.get("publication_id").cloned().unwrap_or(J::Null),
    }
}

fn by_id_map(records: &J) -> BTreeMap<String, J> {
    let mut by_id: BTreeMap<String, J> = BTreeMap::new();
    for r in records.items() {
        if r.is_dict() {
            if let J::Str(rid) = rec_id(r) {
                if !rid.is_empty() {
                    by_id.entry(rid).or_insert_with(|| r.clone());
                }
            }
        }
    }
    by_id
}

fn missing_j(path: &str, source: &str, reason: &str) -> J {
    jd! {"path" => path, "source" => source, "reason" => reason}
}

fn seed_j(path: &str, source: &str, pointer: &str) -> J {
    jd! {"path" => path, "source" => source, "pointer" => pointer}
}

fn seeds_from_publications_j(repo: &Path, records: &J, this_topic: &J, identity: &J, cell_key: &J, publish_kst: Option<&str>) -> HR<J> {
    let mut seeds: Vec<J> = Vec::new();
    let mut missing: Vec<J> = Vec::new();
    let by_id = by_id_map(records);
    let topic_str = this_topic.as_str().map(str::to_string);
    let this = if this_topic.truthy() { topic_str.as_ref().and_then(|t| by_id.get(t)) } else { None };
    let this_ok = this.is_some_and(|t| t.get("_unreadable").is_none());
    if this_topic.truthy() && this.is_none() {
        missing.push(missing_j(
            &format!("{}/{}.json", core::REL_EVIDENCE_DIR, pystr(this_topic)),
            &format!("publication:{}", pystr(this_topic)),
            "record-absent",
        ));
    } else if this_topic.truthy() && !this_ok {
        missing.push(missing_j(
            &format!("{}/{}.json", core::REL_EVIDENCE_DIR, pystr(this_topic)),
            &format!("publication:{}", pystr(this_topic)),
            "record-unreadable",
        ));
    }
    let take = |rec: &J, source: &str, seeds: &mut Vec<J>, missing: &mut Vec<J>| {
        for (pointer, raw, base) in record_pointers(rec) {
            match _norm_pointer(repo, &raw, &base) {
                (Some(p), _) => seeds.push(seed_j(&p, source, &pointer)),
                (None, why) => missing.push(missing_j(&raw, source, &why.unwrap_or_default())),
            }
        }
    };
    if this_ok {
        take(this.unwrap(), &format!("publication:{}", pystr(this_topic)), &mut seeds, &mut missing);
    }
    let ident: J = if identity.as_dict().is_some_and(|m| !m.is_empty()) {
        identity.clone()
    } else if this_ok {
        this.unwrap().get("identity").cloned().unwrap_or(J::Null)
    } else {
        J::Null
    };
    let ck: Option<String> = if cell_key.truthy() { Some(_norm_key(&pystr(cell_key))) } else { None };
    let mut prior: Vec<String> = Vec::new();
    let mut late: Vec<String> = Vec::new();
    let mut how = Map::new();
    let mut cell_mismatch: Vec<String> = Vec::new();
    if let (true, Some(ck)) = (ident.truthy(), &ck) {
        for (rid, rec) in &by_id {
            if Some(rid) == topic_str.as_ref() || rec.get("_unreadable").is_some() {
                continue;
            }
            if !identity_eq(&rec["identity"], &ident) {
                continue;
            }
            let (via, mismatch) = record_cell_match_inner(repo, rid, rec, ck);
            let Some(via) = via else {
                if mismatch {
                    cell_mismatch.push(rid.clone());
                }
                continue;
            };
            let k = record_kst(rec)?;
            if let (Some(pk), Some(k)) = (publish_kst.filter(|s| !s.is_empty()), &k) {
                if k.as_str() > pk {
                    late.push(rid.clone());
                    continue;
                }
            }
            prior.push(rid.clone());
            how.insert(rid.clone(), via);
            take(rec, &format!("prior_publication:{rid}"), &mut seeds, &mut missing);
        }
    }
    Ok(jd! {
        "seeds" => seeds, "missing" => missing,
        "this" => if this_ok { this_topic.clone() } else { J::Null },
        "prior" => prior, "prior_after_ceiling" => late, "cell_key" => ck, "prior_match" => J::Dict(how),
        "prior_cell_mismatch" => cell_mismatch,
    })
}

/// `seeds_from_publications(repo, records, this_topic=, identity=, cell_key=, publish_kst=)`.
pub fn seeds_from_publications(
    repo: &Path,
    records: &J,
    this_topic: Option<&str>,
    identity: &J,
    cell_key: Option<&str>,
    publish_kst: Option<&str>,
) -> HR<J> {
    seeds_from_publications_j(repo, records, &J::from(this_topic), identity, &J::from(cell_key), publish_kst)
}

fn id_tokens(rid: &str) -> Vec<String> {
    id_split_re().split(&rid.to_lowercase()).into_iter().filter(|t| !t.is_empty()).collect()
}

fn alias_token_ok(tok: &str) -> bool {
    clen(tok) >= 4
        && tok.chars().any(|c| c.is_alphabetic())
        && tok.chars().any(|c| c.is_ascii_digit())
        && !campaign_id_token_re().is_match(tok)
}

/// `model_aliases(records, pat)`.
pub fn model_aliases(records: &J, pat: &Pat) -> HR<J> {
    let mut same: Vec<(String, Vec<String>)> = Vec::new();
    let mut other: BTreeSet<String> = BTreeSet::new();
    for r in records.items() {
        if !r.is_dict() || r.get("_unreadable").is_some() {
            continue;
        }
        let J::Str(rid) = rec_id(r) else { continue };
        if rid.is_empty() {
            continue;
        }
        let model = r.get("identity").and_then(|i| i.get("model")).filter(|_| r["identity"].is_dict());
        let toks = id_tokens(&rid);
        match model.and_then(J::as_str) {
            Some(m) if !py_strip(m).is_empty() && pat.search(m) => {
                if !toks.is_empty() && alias_token_ok(&toks[0]) {
                    match same.iter_mut().find(|(t, _)| *t == toks[0]) {
                        Some((_, v)) => v.push(rid.clone()),
                        None => same.push((toks[0].clone(), vec![rid.clone()])),
                    }
                }
            }
            Some(m) if !py_strip(m).is_empty() => other.extend(toks),
            _ => {}
        }
    }
    let mut aliases: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (t, ids) in same {
        if ids.len() >= 2 && !other.contains(&t) {
            let mut ids = ids;
            ids.sort();
            aliases.insert(t, ids);
        }
    }
    let mut ev = Map::new();
    for (t, ids) in &aliases {
        ev.insert(t.clone(), ids.clone());
    }
    Ok(jd! {"aliases" => aliases.keys().cloned().collect::<Vec<String>>(), "evidence" => J::Dict(ev)})
}

/// `seeds_from_model_priors(repo, records, this_topic=, identity=, pat=, publish_kst=, exclude=)`.
pub fn seeds_from_model_priors(
    repo: &Path,
    records: &J,
    this_topic: &J,
    identity: &J,
    pat: &Pat,
    publish_kst: Option<&str>,
    exclude: &[String],
) -> HR<J> {
    let al = model_aliases(records, pat)?;
    let alias_set: BTreeSet<String> = al["aliases"].items().iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
    let empty = J::dict();
    let ident = if identity.is_dict() { identity } else { &empty };
    let gpu = ident.get("gpu");
    let excl: BTreeSet<&String> = exclude.iter().collect();
    let mut seeds = Vec::new();
    let mut missing = Vec::new();
    let mut took: Vec<String> = Vec::new();
    let mut how = Map::new();
    let mut late: Vec<String> = Vec::new();
    let mut gpu_mis: Vec<String> = Vec::new();
    let by_id = by_id_map(records);
    for (rid, rec) in &by_id {
        if this_topic.as_str() == Some(rid.as_str()) || excl.contains(rid) || rec.get("_unreadable").is_some() {
            continue;
        }
        let ri = if rec["identity"].is_dict() { &rec["identity"] } else { &empty };
        let via = match ri.get("model").and_then(J::as_str).filter(|m| !py_strip(m).is_empty()) {
            Some(m) => {
                if !pat.search(m) {
                    continue;
                }
                "identity.model~base_slug".to_string()
            }
            None => {
                let toks = id_tokens(rid);
                if toks.is_empty() || !alias_set.contains(&toks[0]) {
                    continue;
                }
                format!("id-alias:{}(identity.model 부재)", toks[0])
            }
        };
        if !none_or_empty(gpu) && !none_or_empty(ri.get("gpu")) && norm_val(&ri["gpu"]) != norm_val(gpu.unwrap()) {
            gpu_mis.push(rid.clone());
            continue;
        }
        let k = record_kst(rec)?;
        if let (Some(pk), Some(k)) = (publish_kst.filter(|s| !s.is_empty()), &k) {
            if k.as_str() > pk {
                late.push(rid.clone());
                continue;
            }
        }
        took.push(rid.clone());
        how.insert(rid.clone(), via);
        let src = format!("{MODEL_PRIOR_SOURCE}{rid}");
        for (pointer, raw, base) in record_pointers(rec) {
            match _norm_pointer(repo, &raw, &base) {
                (Some(p), _) => seeds.push(seed_j(&p, &src, &pointer)),
                (None, why) => missing.push(missing_j(&raw, &src, &why.unwrap_or_default())),
            }
        }
    }
    Ok(jd! {
        "seeds" => seeds, "missing" => missing, "records" => took, "match" => J::Dict(how), "after_ceiling" => late,
        "gpu_mismatch" => gpu_mis, "aliases" => al,
    })
}

fn record_cell_match_inner(repo: &Path, rid: &str, rec: &J, cell_key: &str) -> (Option<String>, bool) {
    let ck = _norm_key(cell_key);
    let nid = _norm_key(rid);
    let looks = nid == ck || nid.ends_with(&format!("_{ck}"));
    if let Some(observed) = record_cell(repo, rec) {
        if _norm_key(&observed) != ck {
            return (None, looks);
        }
        return (Some("simlog:sweep_index.meta.config_name".into()), false);
    }
    (if looks { Some("publication-id-suffix(simlog 미관측)".into()) } else { None }, false)
}

pub fn record_cell_match(repo: &Path, rid: &str, rec: &J, cell_key: &str) -> HR<(Option<String>, bool)> {
    Ok(record_cell_match_inner(repo, rid, rec, cell_key))
}

fn record_cell(repo: &Path, rec: &J) -> Option<String> {
    for group in ["raw_log_paths", "scaffolded"] {
        let raw = rec.get(group).filter(|g| g.is_dict()).and_then(|g| g.get("simlog"));
        let Some(raw) = raw.and_then(J::as_str).filter(|r| !py_strip(r).is_empty()) else { continue };
        let (path, _) = _norm_pointer(repo, raw, "");
        let Some(path) = path.filter(|p| !excluded(p)) else { continue };
        let f = repo.join(&path).join("sweep_index.json");
        let doc = if f.is_file() { read_text_strict(&f).ok().and_then(|t| crate::value::loads(&t).ok()) } else { None };
        if let Some(doc) = doc.filter(J::is_dict) {
            let empty = J::dict();
            let meta = if doc["meta"].is_dict() { &doc["meta"] } else { &empty };
            let name = match meta.get("config_name") {
                Some(v) if v.truthy() => v.clone(),
                _ => doc.get("config").cloned().unwrap_or(J::Null),
            };
            if let Some(n) = name.as_str().filter(|n| !py_strip(n).is_empty()) {
                return Some(py_strip(n).to_string());
            }
        }
    }
    None
}

// ── 필터 · 관련도 ─────────────────────────────────────────────────────────────────────────────
pub fn slug_pattern(base_slug: &str) -> HR<Pat> {
    let toks: Vec<String> = id_split_re().split(&base_slug.to_lowercase()).into_iter().filter(|t| !t.is_empty()).collect();
    if toks.is_empty() {
        return fail(
            "HINT_LINEAGE_FILTER_ABSENT",
            format!("계보 필터 base 슬러그가 비었다: {}", py_repr(base_slug)),
            Some("PAYLOAD.identity.base_slug(naming.base_slug 파생)를 넘긴다."),
        );
    }
    let text = format!(
        "(?<![0-9a-z]){}(?![0-9a-z])",
        toks.iter().map(|t| py_re_escape(t)).collect::<Vec<_>>().join("[-_. ]?")
    );
    let engine = format!("(?<![0-9a-z]){}(?![0-9a-z])", toks.iter().map(|t| rx_escape(t)).collect::<Vec<_>>().join("[-_. ]?"));
    Ok(Pat { pattern: text, re: Re::new(&engine, pyre::I) })
}

/// A `Pat` from a Python pattern text known to be a `slug_pattern` result (IGNORECASE) — bridge decoding only.
pub fn pat_from_text(text: &str) -> HR<Pat> {
    let re = Re::try_new(text, pyre::I).map_err(|e| HintError::py("re.error", e))?;
    Ok(Pat { pattern: text.to_string(), re })
}

// ── 선언(--lineage-add) ──────────────────────────────────────────────────────────────────────
fn py_str_or_empty(v: Option<&J>) -> String {
    match v {
        Some(x) if x.truthy() => pystr(x),
        _ => String::new(),
    }
}

pub fn _parse_declared(repo: &Path, declared: &J) -> HR<Vec<(String, String)>> {
    let mut out: BTreeSet<(String, String)> = BTreeSet::new();
    for item in declared.items() {
        let (path, reason) = match item {
            J::Str(s) => match s.split_once('=') {
                Some((a, b)) => (a.to_string(), b.to_string()),
                None => (s.clone(), String::new()),
            },
            J::Dict(m) => (py_str_or_empty(m.get("path")), py_str_or_empty(m.get("reason"))),
            J::List(l) if l.len() == 2 => (py_str_or_empty(Some(&l[0])), py_str_or_empty(Some(&l[1]))),
            _ => {
                return fail(
                    "HINT_LINEAGE_DECLARED_SHAPE",
                    format!("--lineage-add 모양이 아니다: {}", item.repr()),
                    Some("PATH=REASON 로 넘긴다."),
                )
            }
        };
        let (path, reason) = (py_strip(&path).to_string(), py_strip(&reason).to_string());
        if path.is_empty() || reason.is_empty() {
            return fail(
                "HINT_LINEAGE_DECLARED_REASON_ABSENT",
                format!("보충 계보에 경로·사유가 모두 있어야 한다: {}", item.repr()),
                Some("--lineage-add PATH=REASON — 사유 없는 보충은 출처 없는 추가다(plan §4.3)."),
            );
        }
        let rel = core::rel(repo, Path::new(&path)).map_err(|mut e| {
            if e.code == "HINT_PATH_OUTSIDE_REPO" {
                e.message = format!("저장소 밖 경로: {path}");
            }
            e
        })?;
        if excluded(&rel) {
            return fail(
                "HINT_LINEAGE_DECLARED_EXCLUDED",
                format!("배제 root 의 문서는 계보에 넣을 수 없다: {rel}"),
                Some(&format!(
                    "배제 root = {} (헌법 비색인 · 백업/미러 섀도잉). docs/ 의 정본 경로를 넣는다.",
                    EXCLUDED_ROOTS.join(", ")
                )),
            );
        }
        if !repo.join(&rel).exists() {
            return fail("HINT_LINEAGE_DECLARED_ABSENT", format!("보충 계보 경로가 없다: {rel}"), Some("저장소 상대 경로를 확인한다."));
        }
        out.insert((rel, reason));
    }
    Ok(out.into_iter().collect())
}

pub fn tool_snapshot_rel(name: &str, rev: &str) -> HR<String> {
    let rev12: String = rev.chars().take(12).collect();
    let rel_path = format!("{TOOL_SNAPSHOT_DIR}/{name}@{rev12}");
    if !tool_snapshot_re().is_match(&rel_path) || !hex40_re().is_full(rev) {
        return fail(
            "HINT_LINEAGE_TOOL_SOURCE_SHAPE",
            format!("도구 스냅숏 이름·커밋 모양이 아니다: {}@{}", py_repr(name), py_repr(&rev12)),
            Some("이름은 파일 basename, 커밋은 40자 SHA 여야 한다."),
        );
    }
    Ok(rel_path)
}

pub fn _tool_candidate_problem(repo: &Path, c: &J) -> HR<Option<String>> {
    let path = py_str_or_empty(c.get("path"));
    let origin = py_str_or_empty(c.get("origin"));
    let (Some(pm), Some(om)) = (tool_snapshot_re().match_at_start(&path), tool_origin_re().match_at_start(&origin)) else {
        return Ok(Some(
            "tool-source-shape(path 는 inputs/sources/<이름>@<rev12> · origin 은 git:<40자 SHA>:<저장소 상대 경로>)".into(),
        ));
    };
    let rev = om[1].to_string();
    let rpath = om[2].to_string();
    if normpath(&rpath) != rpath || rpath.starts_with("../") || rpath.starts_with('/') || rpath == ".." {
        return Ok(Some("tool-source-origin-path(정규화된 저장소 상대 경로가 아니다)".into()));
    }
    if pname(&rpath) != pm[1] || rev[..12] != pm[2] {
        return Ok(Some("tool-source-mismatch(경로의 이름·rev12 가 origin 과 다르다)".into()));
    }
    if !repo.join(".git").exists() {
        return Ok(Some("tool-source-unverifiable(git 저장소가 아니다 — origin 바이트를 확인할 수 없다)".into()));
    }
    let spec = format!("{rev}:{rpath}");
    if core::git_nocheck(repo, &["cat-file", "-e", &spec])?.returncode != 0 {
        return Ok(Some("git-object-absent(origin 커밋에 그 경로가 없다)".into()));
    }
    Ok(None)
}

fn data_kind_of_path(rel_path: &str, is_dir: bool) -> &'static str {
    if is_dir {
        "raw_dir"
    } else if rel_path.ends_with(".log") {
        "engine_log"
    } else if rel_path.ends_with(".json") || rel_path.ends_with(".jsonl") {
        "raw_json"
    } else {
        "raw_file"
    }
}

struct Cand {
    kind: String,
    sources: BTreeSet<String>,
    origin: Option<String>,
}

fn add_cand(cand: &mut BTreeMap<String, Cand>, path: &str, kind: &str, source: &str, origin: Option<&str>) -> HR<()> {
    let c = cand.entry(path.to_string()).or_insert_with(|| Cand { kind: kind.to_string(), sources: BTreeSet::new(), origin: None });
    c.sources.insert(source.to_string());
    if let Some(o) = origin {
        if c.origin.as_deref().is_some_and(|x| x != o) {
            return fail(
                "HINT_LINEAGE_TOOL_ORIGIN_CONFLICT",
                format!("같은 후보 경로에 origin 이 둘이다: {path}"),
                Some("evidence.tool_snapshots 의 이름·커밋이 겹친다 — 같은 basename 의 두 도구를 한 draft 에 싣지 않는다."),
            );
        }
        c.origin = Some(o.to_string());
    }
    Ok(())
}

fn expand_dir_candidates(repo: &Path, cand: &mut BTreeMap<String, Cand>, cell: &J) -> HR<BTreeMap<String, J>> {
    let mut out = BTreeMap::new();
    let key = if cell.truthy() { Some(_norm_key(&pystr(cell))) } else { None };
    let keys: Vec<String> = cand.keys().cloned().collect();
    for p in keys {
        let d = repo.join(&p);
        if !d.is_dir() || d.is_symlink() {
            continue;
        }
        let mut files: Vec<(String, usize, bool)> = Vec::new();
        for f in rglob(&d) {
            if f.is_symlink() || !f.is_file() {
                continue;
            }
            let parts = rel_below(&f, &d);
            if parts.iter().any(|x| x.starts_with('.')) {
                continue;
            }
            let r = join_rel(&p, &parts);
            let mine = key.as_ref().is_some_and(|k| !k.is_empty() && _norm_key(&parts.join("/")).contains(k.as_str()));
            files.push((r, parts.len(), mine));
        }
        files.sort_by(|a, b| (!a.2, a.1, &a.0).cmp(&(!b.2, b.1, &b.0)));
        let eligible: Vec<&String> = files.iter().filter(|(r, depth, mine)| (*mine || *depth == 1) && !excluded(r)).map(|(r, _, _)| r).collect();
        let take: Vec<String> = eligible.iter().take(EXPAND_MAX_FILES).map(|s| s.to_string()).collect();
        for r in &take {
            add_cand(cand, r, data_kind_of_path(r, false), &format!("expanded:{p}"), None)?;
        }
        out.insert(
            p.clone(),
            jd! {"listed" => take.len(), "total" => files.len(), "unlisted" => files.len() - take.len(),
            "truncated" => eligible.len() > EXPAND_MAX_FILES},
        );
    }
    Ok(out)
}

fn via_j(f: &str, e: &str, t: &str, r: &str) -> J {
    jd! {"from" => f, "edge" => e, "token" => t, "resolution" => r}
}

fn dumps_sorted_unicode(v: &J) -> String {
    dumps_with(v, Dump { ensure_ascii: false, sort_keys: true, ..Dump::DEFAULT })
}

// ── 파생 ─────────────────────────────────────────────────────────────────────────────────────
#[allow(clippy::too_many_arguments)]
pub fn derive(
    repo: &Path,
    ev: &J,
    base_slug: &str,
    publish_kst: &str,
    depth: i64,
    declared: &J,
    records: &J,
    this_topic: Option<&str>,
    identity: &J,
    cell_key: Option<&str>,
    axis_tokens: &[String],
    base_slug_source: &str,
) -> HR<J> {
    let axis: Vec<J> = axis_tokens.iter().map(|s| J::Str(s.clone())).collect();
    derive_j(
        repo,
        ev,
        base_slug,
        &J::from(publish_kst),
        &J::Int(depth),
        declared,
        records,
        &J::from(this_topic),
        identity,
        &J::from(cell_key),
        &axis,
        base_slug_source,
    )
}

/// `derive` with the loosely typed arguments of the Python signature (`None` = `J::Null`).
#[allow(clippy::too_many_arguments)]
pub fn derive_j(
    repo: &Path,
    ev: &J,
    base_slug: &str,
    publish_kst_j: &J,
    depth_j: &J,
    declared: &J,
    records: &J,
    this_topic: &J,
    identity: &J,
    cell_key: &J,
    axis_tokens: &[J],
    base_slug_source: &str,
) -> HR<J> {
    let publish_kst = match publish_kst_j.as_str() {
        Some(s) if kst_token_re().is_match(s) => s.to_string(),
        _ => {
            return fail(
                "HINT_TIME_NOT_INJECTED",
                format!("publish_kst 는 KST `YYMMDDHH` 로 주입해야 한다: {}", publish_kst_j.repr()),
                Some("core.kst_token(--generated-utc) 를 넘긴다(벽시계 금지)."),
            )
        }
    };
    let depth = match depth_j {
        J::Int(d) if *d >= 0 => *d,
        _ => return fail("HINT_LINEAGE_DEPTH_INVALID", format!("depth 는 0 이상 정수: {}", depth_j.repr()), None),
    };
    let pat = slug_pattern(base_slug)?;
    let mut corpus = Corpus::new(repo, &DEFAULT_ROOTS, &EXCLUDED_ROOTS)?;
    let decl = _parse_declared(repo, declared)?;
    for (rel, _) in &decl {
        if let Some(node) = corpus.ensure(rel) {
            if !_within_ceiling(node.yymmddhh.as_deref(), &publish_kst) {
                return fail(
                    "HINT_LINEAGE_DECLARED_AFTER_CEILING",
                    format!("보충 계보가 발행 시각 상한({publish_kst}) 뒤의 문서다: {rel}"),
                    Some("발행 시점에 없던 문서는 계보가 아니다(D-W8) — 발행 시각을 확인한다."),
                );
            }
        }
    }

    let empty = J::dict();
    let ls = match ev_get(ev, "lineage_seeds") {
        v @ J::Dict(_) => v,
        _ => &empty,
    };
    let pub_ = ev_get(ev, "publication");
    let topic: J = if this_topic.truthy() {
        this_topic.clone()
    } else {
        match pub_.get("topic") {
            Some(t) if pub_.is_dict() && t.truthy() => t.clone(),
            _ => ls.get("this_topic").cloned().unwrap_or(J::Null),
        }
    };
    let loaded;
    let recs: &J = if !records.is_null() {
        records
    } else {
        match ls.get("publication_records") {
            Some(r) if !r.is_null() => r,
            _ => {
                loaded = J::List(load_publication_records(repo)?);
                &loaded
            }
        }
    };
    let ident: J = if !identity.is_null() { identity.clone() } else { ls.get("identity").cloned().unwrap_or(J::Null) };
    let cell = ev_get(ev, "cell").clone();
    let ck: J = if cell_key.truthy() {
        cell_key.clone()
    } else {
        match ls.get("cell_key") {
            Some(v) if v.truthy() => v.clone(),
            _ => cell.clone(),
        }
    };
    let cfg = ev_get(ev, "cell_config");
    let decl_axes = match cfg.get("declared_axes") {
        Some(d @ J::Dict(_)) => d.clone(),
        _ => J::dict(),
    };
    let mut sib_keys = vec![ck.clone()];
    for v in decl_axes.as_dict().unwrap().values() {
        if matches!(v, J::Str(_) | J::Int(_)) && !pystr(v).contains("<<FILL>>") {
            sib_keys.push(v.clone());
        }
    }
    *corpus.sf.borrow_mut() = if ck.truthy() { Some(SiblingFilter::new(&sib_keys)) } else { None };
    let sp = seeds_from_publications_j(repo, recs, &topic, &ident, &ck, Some(&publish_kst))?;

    let this_rec = recs.items().iter().find(|r| r.is_dict() && rec_id(r) == topic && r.get("_unreadable").is_none());
    let mp_ident: J = if ident.as_dict().is_some_and(|m| !m.is_empty()) {
        ident.clone()
    } else {
        match this_rec.and_then(|r| r.get("identity")) {
            Some(v) if v.truthy() => v.clone(),
            _ => J::dict(),
        }
    };
    let sp_prior: Vec<String> = sp["prior"].items().iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
    let mp = seeds_from_model_priors(repo, recs, &topic, &mp_ident, &pat, Some(&publish_kst), &sp_prior)?;

    let mut raw_seeds: BTreeSet<(String, String, String, bool)> = BTreeSet::new();
    for s in sp["seeds"].items().iter().chain(mp["seeds"].items()) {
        raw_seeds.insert((pystr(&s["path"]), pystr(&s["source"]), pystr(&s["pointer"]), true));
    }
    let mut seeds_missing: Vec<(String, String, String)> = Vec::new();
    for m in sp["missing"].items().iter().chain(mp["missing"].items()) {
        seeds_missing.push((pystr(&m["path"]), pystr(&m["source"]), pystr(&m["reason"])));
    }
    let mut model_prior_skipped: BTreeSet<String> = BTreeSet::new();
    for k in ["bench_report_path", "certificate_path"] {
        if let Some(v) = ev_get(ev, k).as_str().filter(|v| !v.is_empty()) {
            match _norm_pointer(repo, v, "") {
                (Some(p), _) => {
                    raw_seeds.insert((p, format!("evidence:{k}"), k.to_string(), true));
                }
                (None, why) => seeds_missing.push((v.to_string(), format!("evidence:{k}"), why.unwrap_or_default())),
            }
        }
    }
    for ptr in ev_get(ev, "pointers").items() {
        let Some(pp) = ptr.get("path").and_then(J::as_str) else { continue };
        let cid = ptr.get("cell_id").cloned().unwrap_or(J::Null);
        if !none_or_empty(Some(&cid)) && cell.truthy() && cid != cell {
            continue;
        }
        let kind = ptr.get("kind").cloned().unwrap_or(J::Null);
        let src = format!("campaign_evidence_pointer:{}", if kind.truthy() { pystr(&kind) } else { "unknown".into() });
        match _norm_pointer(repo, pp, "") {
            (Some(p), _) => {
                raw_seeds.insert((p, src, format!("pointers[{}]", pystr(&kind)), true));
            }
            (None, why) => seeds_missing.push((pp.to_string(), src, why.unwrap_or_default())),
        }
    }
    for (rel, reason) in &decl {
        raw_seeds.insert((rel.clone(), "declared".into(), reason.clone(), true));
    }
    let mut cite_sources: Vec<(String, String)> = Vec::new();
    for k in ["lockset", "cell_config", "declaration"] {
        let v = ev_get(ev, k);
        if v.truthy() {
            cite_sources.push((format!("{k}_citation"), dumps_sorted_unicode(v)));
        }
    }
    if let Some(trip) = ev_get(ev, "triplet").as_dict() {
        let mut keys: Vec<&str> = trip.keys().collect();
        keys.sort();
        for k in keys {
            let Some(tv) = trip.get(k).and_then(J::as_str) else { continue };
            let (tpath, why) = _norm_pointer(repo, tv, "");
            let Some(tpath) = tpath else {
                seeds_missing.push((tv.to_string(), format!("triplet_citation:{k}"), why.unwrap_or_default()));
                continue;
            };
            let f = repo.join(&tpath);
            if f.is_file() {
                cite_sources.push((format!("triplet_citation:{k}"), read_text_replace(&f).map_err(|e| os_py(&e, &f))?));
            }
        }
    }
    let reserved = ["publication_records", "this_topic", "identity", "cell_key", "axis_tokens", "evidence_candidates", "attestation"];
    if let Some(lsm) = ls.as_dict() {
        let mut keys: Vec<&str> = lsm.keys().collect();
        keys.sort();
        for k in keys {
            let v = lsm.get(k).unwrap();
            if reserved.contains(&k) || !v.truthy() {
                continue;
            }
            cite_sources.push((format!("lineage_seeds:{k}"), dumps_sorted_unicode(v)));
            let vals: Vec<J> = if let J::List(l) = v { l.clone() } else { vec![v.clone()] };
            for x in vals {
                let Some(xs) = x.as_str().filter(|s| !py_strip(s).is_empty()) else { continue };
                let (vp, _) = _norm_pointer(repo, xs, "");
                let Some(vp) = vp else { continue };
                let f = repo.join(&vp);
                if excluded(&vp) || !f.is_file() {
                    continue;
                }
                let size = std::fs::metadata(&f).map_err(|e| os_py(&e, &f))?.len();
                if corpus.nodes.contains_key(&vp) || size > (1 << 20) {
                    continue;
                }
                cite_sources.push((format!("lineage_seeds:{k}:file"), read_text_replace(&f).map_err(|e| os_py(&e, &f))?));
            }
        }
    }
    for (src, text) in &cite_sources {
        for (_, targets, res, token) in corpus.resolve_text(text) {
            let targets: Vec<String> = if res == "ambiguous" && corpus.sf.borrow().is_some() {
                corpus.sf_keep(src, &token, &targets.iter().cloned().collect())?.into_iter().collect()
            } else {
                targets
            };
            for t in targets {
                raw_seeds.insert((t, src.clone(), token.clone(), false));
            }
        }
    }

    // 시드 분류
    let mut adopted: BTreeMap<String, i64> = BTreeMap::new();
    let mut seed_via: BTreeMap<String, BTreeSet<(String, String, String, String)>> = BTreeMap::new();
    let mut seed_rows: BTreeSet<(String, String)> = BTreeSet::new();
    let mut cand: BTreeMap<String, Cand> = BTreeMap::new();
    let mut rej_filter: BTreeSet<String> = BTreeSet::new();
    let mut rej_ceiling: BTreeSet<String> = BTreeSet::new();
    for (path, source, pointer, exempt) in &raw_seeds {
        if excluded(path) {
            seeds_missing.push((path.clone(), source.clone(), "excluded-root".into()));
            continue;
        }
        if !repo.join(path).exists() {
            seeds_missing.push((path.clone(), source.clone(), "file-absent".into()));
            continue;
        }
        let node = corpus.ensure(path);
        if source.starts_with(MODEL_PRIOR_SOURCE) && node.as_ref().map_or(true, |n| !MODEL_PRIOR_KINDS.contains(&n.kind.as_str())) {
            model_prior_skipped.insert(path.clone());
            continue;
        }
        if let Some(n) = &node {
            if !_within_ceiling(n.yymmddhh.as_deref(), &publish_kst) {
                seeds_missing.push((path.clone(), source.clone(), "after-publish-ceiling".into()));
                continue;
            }
        }
        if let Some(n) = node.as_ref().filter(|n| n.is_document()) {
            let _ = n;
            if !exempt && !corpus.slug_hit(path, &pat)? {
                rej_filter.insert(path.clone());
                continue;
            }
            adopted.insert(path.clone(), 0);
            seed_rows.insert((path.clone(), source.clone()));
            seed_via.entry(path.clone()).or_default().insert((source.clone(), "seed".into(), pointer.clone(), "exact".into()));
        } else {
            let kind = match &node {
                Some(n) => n.kind.clone(),
                None => data_kind_of_path(path, repo.join(path).is_dir()).to_string(),
            };
            if *exempt || corpus.slug_hit(path, &pat)? {
                add_cand(&mut cand, path, &kind, source, None)?;
            } else {
                rej_filter.insert(path.clone());
            }
        }
    }

    // BFS
    let narr = corpus.narrators()?;
    let mut transit: BTreeMap<String, i64> = BTreeMap::new();
    let mut frontier: Vec<String> = adopted.keys().cloned().collect();
    let mut frontier_transit: Vec<String> = Vec::new();
    for d in 1..=depth {
        let mut reach: BTreeMap<String, bool> = BTreeMap::new();
        for u in &frontier {
            let mut nbrs: Vec<String> = corpus.mentions(u)?.into_iter().map(|e| e.to).collect();
            if corpus.nodes[u].kind == "testlog" {
                nbrs.extend(narr.get(u).map(|v| v.iter().map(|x| x.0.clone()).collect::<Vec<_>>()).unwrap_or_default());
            }
            for v in nbrs {
                reach.insert(v, true);
            }
        }
        for u in &frontier_transit {
            for e in corpus.mentions(u)? {
                reach.entry(e.to).or_insert(false);
            }
        }
        let mut nxt = Vec::new();
        let mut nxt_transit = Vec::new();
        for (v, from_adopted) in &reach {
            if adopted.contains_key(v) || transit.contains_key(v) || rej_ceiling.contains(v) {
                continue;
            }
            let node = &corpus.nodes[v];
            if !node.is_document() {
                continue;
            }
            if !_within_ceiling(node.yymmddhh.as_deref(), &publish_kst) {
                rej_ceiling.insert(v.clone());
                continue;
            }
            if corpus.slug_hit(v, &pat)? {
                rej_filter.remove(v);
                adopted.insert(v.clone(), d);
                nxt.push(v.clone());
            } else if *from_adopted && d < depth {
                rej_filter.remove(v);
                transit.insert(v.clone(), d);
                nxt_transit.push(v.clone());
            } else {
                rej_filter.insert(v.clone());
            }
        }
        nxt.sort();
        nxt_transit.sort();
        frontier = nxt;
        frontier_transit = nxt_transit;
    }

    // 데이터 후보
    let adopted_keys: Vec<String> = adopted.keys().cloned().collect();
    for u in &adopted_keys {
        for e in corpus.mentions(u)? {
            let node = &corpus.nodes[&e.to];
            if node.is_document() || rej_filter.contains(&e.to) || rej_ceiling.contains(&e.to) {
                continue;
            }
            if !_within_ceiling(node.yymmddhh.as_deref(), &publish_kst) {
                rej_ceiling.insert(e.to.clone());
                continue;
            }
            if cand.contains_key(&e.to) || corpus.slug_hit(&e.to, &pat)? {
                let kind = node.kind.clone();
                add_cand(&mut cand, &e.to, &kind, &format!("mention:{u}"), None)?;
            } else {
                rej_filter.insert(e.to.clone());
            }
        }
        for op in corpus.output_mentions(u)? {
            add_cand(&mut cand, &op, data_kind_of_path(&op, repo.join(&op).is_dir()), &format!("mention:{u}"), None)?;
        }
    }
    let sweep = ev_get(ev, "sweep");
    if let Some(sdir) = sweep.get("dir").and_then(J::as_str).filter(|s| !s.is_empty()) {
        let srel = core::rel(repo, Path::new(sdir)).map_err(|mut e| {
            if e.code == "HINT_PATH_OUTSIDE_REPO" {
                e.message = format!("저장소 밖 경로: {sdir}");
            }
            e
        })?;
        let sp_dir = repo.join(&srel);
        if sp_dir.is_dir() {
            for f in rglob(&sp_dir) {
                let parts = rel_below(&f, &sp_dir);
                if !f.is_file() || parts.iter().any(|p| p.starts_with('.')) {
                    continue;
                }
                let r = join_rel(&srel, &parts);
                let name = parts.last().cloned().unwrap_or_default();
                match py_suffix(&name) {
                    ".log" => add_cand(&mut cand, &r, "engine_log", "sweep_dir", None)?,
                    ".json" => {
                        let k = if name == "sweep_index.json" && parts.len() == 1 { "sweep_json" } else { "raw_json" };
                        add_cand(&mut cand, &r, k, "sweep_dir", None)?
                    }
                    _ => {}
                }
            }
        } else {
            seeds_missing.push((srel, "evidence:sweep.dir".into(), "file-absent".into()));
        }
    }
    for c in ls["evidence_candidates"].items() {
        if c.is_dict() && c.get_str("kind") == Some(TOOL_SOURCE_KIND) {
            match _tool_candidate_problem(repo, c)? {
                None => add_cand(&mut cand, &pystr(&c["path"]), TOOL_SOURCE_KIND, "lineage_seeds:evidence_candidates", Some(pystr(&c["origin"]).as_str()))?,
                Some(prob) => seeds_missing.push((pystr(&c["path"]), "lineage_seeds:evidence_candidates".into(), prob)),
            }
            continue;
        }
        if let Some(cp) = c.get("path").and_then(J::as_str) {
            let (path, why) = _norm_pointer(repo, cp, "");
            match &path {
                Some(p) if !excluded(p) && repo.join(p).exists() => {
                    let kind = match c.get("kind") {
                        Some(k) if k.truthy() => pystr(k),
                        _ => data_kind_of_path(p, repo.join(p).is_dir()).to_string(),
                    };
                    add_cand(&mut cand, p, &kind, "lineage_seeds:evidence_candidates", None)?;
                }
                _ => {
                    let reason = why.unwrap_or_else(|| {
                        if path.as_deref().is_some_and(excluded) { "excluded-root".into() } else { "file-absent".into() }
                    });
                    seeds_missing.push((cp.to_string(), "lineage_seeds:evidence_candidates".into(), reason));
                }
            }
        }
    }
    let expanded = expand_dir_candidates(repo, &mut cand, &cell)?;

    // via · 뒤집힘
    let carriers: BTreeSet<String> = adopted.keys().chain(transit.keys()).cloned().collect();
    let mut via: BTreeMap<String, BTreeSet<(String, String, String, String)>> =
        carriers.iter().map(|p| (p.clone(), seed_via.get(p).cloned().unwrap_or_default())).collect();
    for u in &carriers {
        for e in corpus.mentions(u)? {
            if carriers.contains(&e.to) {
                via.get_mut(&e.to).unwrap().insert((u.clone(), e.edge.clone(), e.token.clone(), e.resolution.clone()));
            }
        }
        if adopted.contains_key(u) && corpus.nodes[u].kind == "testlog" {
            for (dv, tok, res) in narr.get(u).cloned().unwrap_or_default() {
                if carriers.contains(&dv) {
                    via.get_mut(&dv).unwrap().insert((u.clone(), "narrates".into(), tok, res));
                }
            }
        }
    }
    let mut overturned_by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let all_nodes: Vec<String> = corpus.nodes.keys().cloned().collect();
    for q in &all_nodes {
        let qn = corpus.nodes[q].clone();
        if !qn.is_document() || !_within_ceiling(qn.yymmddhh.as_deref(), &publish_kst) {
            continue;
        }
        for e in corpus.mentions(q)? {
            if e.edge != "supersede" || e.resolution == "ambiguous" || !adopted.contains_key(&e.to) {
                continue;
            }
            let t = &corpus.nodes[&e.to];
            if let (Some(qy), Some(ty)) = (&qn.yymmddhh, &t.yymmddhh) {
                if !qy.is_empty() && !ty.is_empty() && qy > ty {
                    overturned_by.entry(e.to.clone()).or_default().insert(q.clone());
                }
            }
        }
    }
    let mut axis: Vec<J> = axis_tokens.to_vec();
    match &ls["axis_tokens"] {
        J::List(l) => axis.extend(l.iter().cloned()),
        J::Dict(m) => axis.extend(m.keys().map(J::from)),
        J::Str(s) => axis.extend(s.chars().map(|c| J::Str(c.to_string()))),
        _ => {}
    }
    if cell.truthy() {
        axis.push(cell.clone());
        axis.extend(cell_split_re().split(&pystr(&cell)).into_iter().filter(|t| !t.is_empty()).map(J::Str));
    }
    let axis_pats = axis_patterns(&axis);
    let tracked = tracked_blobs(repo, &adopted_keys)?;

    let mut documents: Vec<(J, (String, String, i64, String, String))> = Vec::new();
    for p in &adopted_keys {
        let node = corpus.nodes[p].clone();
        let f = repo.join(p);
        let data = std::fs::read(&f).map_err(|e| os_py(&e, &f))?;
        let mut commit: Option<String> = None;
        if tracked.get(p).is_some_and(|b| *b == git_blob_sha(&data)) {
            commit = last_commit(repo, p)?;
        }
        let mut sup: BTreeSet<String> = overturned_by.get(p).cloned().unwrap_or_default();
        for e in corpus.mentions(p)? {
            if e.edge != "supersede" || e.resolution == "ambiguous" {
                continue;
            }
            let t = &corpus.nodes[&e.to];
            if !t.is_document() || !_within_ceiling(t.yymmddhh.as_deref(), &publish_kst) {
                continue;
            }
            if let (Some(ny), Some(ty)) = (&node.yymmddhh, &t.yymmddhh) {
                if !ny.is_empty() && !ty.is_empty() && ty < ny {
                    continue;
                }
            }
            sup.insert(e.to.clone());
        }
        let body = corpus.text(p)?;
        let vset = &via[p];
        let resolution = vset.iter().map(|v| v.3.as_str()).min_by_key(|r| res_rank(r)).unwrap_or("exact").to_string();
        let rel_toks: Vec<String> =
            axis_pats.iter().filter(|(_, rx)| rx.is_search(&body) || rx.is_search(p)).map(|(t, _)| t.clone()).collect();
        let doc = jd! {
            "path" => p.clone(), "kind" => node.kind.clone(), "class" => node.klass.clone(), "date_key" => node.yymmddhh.clone(),
            "depth" => adopted[p], "bytes" => data.len(),
            "sha256" => if commit.is_some() { J::Null } else { J::Str(core::sha256_bytes(&data)) },
            "commit" => commit.clone(),
            "via" => vset.iter().map(|(f, e, t, r)| via_j(f, e, t, r)).collect::<Vec<J>>(),
            "relevance" => jd!{"axis_tokens" => rel_toks},
            "superseded_by" => sup.into_iter().collect::<Vec<String>>(),
            "resolution" => resolution,
        };
        let key = (
            node.yymmddhh.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "99999999".into()),
            node.mmss.clone().unwrap_or_default(),
            kind_rank(&node.kind),
            pname(p),
            p.clone(),
        );
        documents.push((doc, key));
    }
    documents.sort_by(|a, b| a.1.cmp(&b.1));
    let documents: Vec<J> = documents.into_iter().map(|(d, _)| d).collect();

    let mut transit_sorted: Vec<(&String, &i64)> = transit.iter().collect();
    transit_sorted.sort_by(|a, b| (a.1, a.0).cmp(&(b.1, b.0)));
    let mut transit_out = Vec::new();
    for (p, d) in transit_sorted {
        let tn = &corpus.nodes[p];
        let v: Vec<J> = via.get(p).map(|s| s.iter().map(|(f, e, t, r)| via_j(f, e, t, r)).collect()).unwrap_or_default();
        transit_out.push(jd! {
            "path" => p.clone(), "kind" => tn.kind.clone(), "class" => tn.klass.clone(), "date_key" => tn.yymmddhh.clone(),
            "depth" => *d, "reason" => "filter-miss(base 슬러그 부재) — 경유만 · 읽기 목록 밖", "via" => v,
        });
    }

    let dedup_missing: BTreeSet<(String, String, String)> = seeds_missing
        .into_iter()
        .map(|(p, s, r)| (if r == "outside-repo" { "<outside-repo>".to_string() } else { p }, s, r))
        .collect();
    let mut amb_out = Vec::new();
    let sf_ref = corpus.sf.borrow();
    if let Some(sf) = sf_ref.as_ref() {
        for (p, rec) in &sf.excluded {
            if adopted.contains_key(p) || transit.contains_key(p) || cand.contains_key(p) || sf.kept.contains(p) {
                continue;
            }
            let node = corpus.nodes.get(p);
            amb_out.push(jd! {
                "path" => p.clone(), "kind" => node.map(|n| n.kind.clone()), "token" => rec.token.clone(),
                "from" => rec.from.iter().cloned().collect::<Vec<String>>(), "reason" => rec.reason.clone(),
            });
        }
    }
    let key_tokens: Vec<String> = sf_ref.as_ref().map(|s| s.keys.clone()).unwrap_or_default();
    drop(sf_ref);

    let cand_rows: Vec<J> = cand
        .iter()
        .map(|(p, c)| {
            let mut row = Map::new();
            row.insert("path", p.clone());
            row.insert("kind", c.kind.clone());
            row.insert("sources", c.sources.iter().cloned().collect::<Vec<String>>());
            if let Some(o) = c.origin.as_ref().filter(|o| !o.is_empty()) {
                row.insert("origin", o.clone());
            }
            if let Some(x) = expanded.get(p) {
                row.insert("expanded", x.clone());
            }
            J::Dict(row)
        })
        .collect();

    let mut roots_absent = corpus.roots_absent.clone();
    roots_absent.sort();
    let method = jd! {
        "graph" => "mention(files@publish)",
        "traversal" => "ancestors+testlog-narrators+filter-transit(1hop)",
        "depth" => depth,
        "filter" => jd!{"base_slug" => base_slug, "source" => base_slug_source, "pattern" => pat.pattern.clone()},
        "publish_kst" => publish_kst.clone(),
        "roots" => J::from(DEFAULT_ROOTS.to_vec()),
        "excluded_roots" => J::from(EXCLUDED_ROOTS.to_vec()),
        "resolution_order" => J::from(RESOLUTIONS.to_vec()),
        "undated" => "pass-ceiling",
        "grammar" => jd!{"prose" => core::REL_DOC_NAMING_TS, "bench" => REL_BENCH_NAMING_RS,
                         "sweep_map" => "docs.md §명명 SSOT(국소 상수 SWEEP_MAP_PREFIX)"},
        "seed_records" => jd!{"this" => sp["this"].clone(), "prior" => sp["prior"].clone(),
                              "prior_after_ceiling" => sp["prior_after_ceiling"].clone(),
                              "prior_match" => sp["prior_match"].clone(), "prior_cell_mismatch" => sp["prior_cell_mismatch"].clone(),
                              "cell_key" => sp["cell_key"].clone(), "identity_fields" => J::from(IDENTITY_MATCH_FIELDS.to_vec())},
        "model_prior_records" => jd!{
            "rule" => concat!("같은 모델의 앞선 셀 발행 기록(S3) — identity.model 이 base 슬러그에 걸림 또는 identity.model 이 ",
                              "빈 기록의 id 첫 토큰이 관측 별칭 · gpu 같음(양쪽에 있을 때) · 발행 시각 상한 이내 · S2 기록 제외 · ",
                              "서사 문서 포인터만(plan·devlog·testlog·report · 필터 면제) · 다른 셀의 측정 표면·데이터 포인터는 싣지 않는다"),
            "records" => mp["records"].clone(), "match" => mp["match"].clone(), "after_ceiling" => mp["after_ceiling"].clone(),
            "gpu_mismatch" => mp["gpu_mismatch"].clone(), "aliases" => mp["aliases"]["aliases"].clone(),
            "alias_evidence" => mp["aliases"]["evidence"].clone(),
            "skipped_non_narrative" => model_prior_skipped.into_iter().collect::<Vec<String>>(),
        },
        "supersede" => concat!("헤더 12줄 배너(SUPERSEDED·SUPERSEDED-IN-PART 어느 꼴이든) 양쪽 관행 · 방향 = 날짜 휴리스틱(뒤에 태어난 ",
                               "쪽이 뒤집은 쪽 · 같은 시각은 판정 안 함) · 모호 해소 간선은 쓰지 않는다"),
        "stats" => jd!{"rejected_filter" => rej_filter.len(), "rejected_ceiling" => rej_ceiling.len(),
                       "transit" => transit.len(), "roots_absent" => roots_absent, "ambiguous_excluded" => amb_out.len()},
        "ambiguous_siblings" => jd!{
            "rule" => concat!("모호 간선의 형제 무리마다 셀 키 토큰 적중(이름·헤더 12줄) 최대 → 이름의 이질 토큰 최소만 유지 · ",
                              "무신호(적중 0)면 전부 유지 · 정확 인용은 무관 · 다른 자리에서 남겨진 형제는 제외 목록에 적지 않는다 · ",
                              "여러 무리에서 걸러지면 사전순 최소 (토큰, 사유)"),
            "key_tokens" => key_tokens,
        },
        "tool_sources" => jd!{"kind" => TOOL_SOURCE_KIND, "path" => format!("draft 상대 {TOOL_SNAPSHOT_DIR}/<이름>@<rev12>"),
                              "origin" => "git:<rev>:<저장소 경로>(git show 바이트 · 측정 시각 직전 판본)",
                              "graph" => "mention 그래프 입력 아님(발췌 출처 전용)"},
        "dir_expansion" => jd!{"max_files_per_dir" => EXPAND_MAX_FILES,
                               "scope" => "디렉터리 바로 아래 파일 + 디렉터리 안 경로에 셀 키를 든 파일(나머지는 디렉터리 후보 경유 전체 경로로만)",
                               "order" => "셀 키 파일 먼저 → 얕은 깊이 → 경로 사전순 · 숨김 경로·심링크 제외"},
    };
    Ok(jd! {
        "schema_version" => SCHEMA_VERSION,
        "method" => method,
        "seeds" => seed_rows.iter().map(|(p, s)| jd!{"path" => p.clone(), "source" => s.clone()}).collect::<Vec<J>>(),
        "documents" => documents,
        "transit" => transit_out,
        "evidence_candidates" => cand_rows,
        "ambiguous_excluded" => amb_out,
        "declared" => decl.iter().map(|(p, r)| jd!{"path" => p.clone(), "source" => "declared", "reason" => r.clone()}).collect::<Vec<J>>(),
        "seeds_missing" => dedup_missing.iter().map(|(p, s, r)| missing_j(p, s, r)).collect::<Vec<J>>(),
    })
}

fn tracked_blobs(repo: &Path, paths: &[String]) -> HR<HashMap<String, String>> {
    if paths.is_empty() || !repo.join(".git").exists() {
        return Ok(HashMap::new());
    }
    let specs: BTreeSet<String> = paths.iter().map(|p| {
        let d = pdirname(p);
        if d.is_empty() { ".".to_string() } else { d }
    }).collect();
    let mut args: Vec<&str> = vec!["-c", "core.quotePath=false", "ls-tree", "-r", "-z", "HEAD", "--"];
    args.extend(specs.iter().map(String::as_str));
    let out = core::git_nocheck(repo, &args)?;
    if out.returncode != 0 {
        return Ok(HashMap::new());
    }
    let mut blobs = HashMap::new();
    for rec in out.stdout.split('\0') {
        let Some((meta, path)) = rec.split_once('\t') else { continue };
        let parts: Vec<&str> = meta.split_whitespace().collect();
        if parts.len() == 3 && parts[1] == "blob" {
            blobs.insert(path.to_string(), parts[2].to_string());
        }
    }
    Ok(blobs)
}

fn last_commit(repo: &Path, path: &str) -> HR<Option<String>> {
    let out = core::git_nocheck(
        repo,
        &["-c", "core.quotePath=false", "-c", "log.showSignature=false", "log", "-1", "--format=%H", "--", path],
    )?;
    let sha = if out.returncode == 0 { py_strip(&out.stdout).to_string() } else { String::new() };
    Ok(if sha.is_empty() { None } else { Some(sha) })
}

// ── 소비자 도우미 ─────────────────────────────────────────────────────────────────────────────
fn j_cmp(a: &J, b: &J) -> std::cmp::Ordering {
    match (a, b) {
        (J::Str(x), J::Str(y)) => x.cmp(y),
        _ => match (a.as_f64().or(a.as_bool().map(|x| x as i64 as f64)), b.as_f64().or(b.as_bool().map(|x| x as i64 as f64))) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            _ => std::cmp::Ordering::Equal,
        },
    }
}

pub fn reading_list(lineage: &J) -> HR<Vec<J>> {
    let mut items = Vec::new();
    for d in lineage["documents"].items() {
        let toks: Vec<J> = d["relevance"]["axis_tokens"].items().to_vec();
        let path = d.get("path").cloned().ok_or_else(|| HintError::py("KeyError", "'path'"))?;
        let tier = if d.get_str("resolution") == Some("ambiguous") { "ambiguous" } else { "primary" };
        items.push(jd! {
            "path" => path, "kind" => d["kind"].clone(), "class" => d["class"].clone(), "date_key" => d["date_key"].clone(),
            "depth" => d.get("depth").cloned().unwrap_or(J::Int(0)), "bytes" => d.get("bytes").cloned().unwrap_or(J::Int(0)),
            "relevance" => toks.len(), "axis_tokens" => toks, "resolution" => d["resolution"].clone(),
            "superseded_by" => d["superseded_by"].items().to_vec(), "tier" => tier,
        });
    }
    items.sort_by(|a, b| {
        (a["tier"].as_str() != Some("primary"))
            .cmp(&(b["tier"].as_str() != Some("primary")))
            .then_with(|| j_cmp(&a["depth"], &b["depth"]))
            .then_with(|| b["relevance"].as_i64().cmp(&a["relevance"].as_i64()))
            .then_with(|| {
                let da = if a["date_key"].truthy() { a["date_key"].clone() } else { J::from("99999999") };
                let db = if b["date_key"].truthy() { b["date_key"].clone() } else { J::from("99999999") };
                j_cmp(&da, &db)
            })
            .then_with(|| j_cmp(&a["path"], &b["path"]))
    });
    Ok(items)
}

pub fn resolve_stem(lineage: &J, stem: &str) -> HR<Option<String>> {
    if py_strip(stem).is_empty() {
        return Ok(None);
    }
    let mut s = strip_chars(py_strip(stem), "`").to_string();
    let docs: Vec<String> = lineage["documents"].items().iter().map(|d| pystr(&d["path"])).collect();
    let cands = lineage["evidence_candidates"].items();
    let mut paths = docs.clone();
    paths.extend(cands.iter().map(|c| pystr(&c["path"])));
    if s.contains('/') {
        let sn = normpath(&s);
        if paths.contains(&sn) {
            return Ok(Some(sn));
        }
        for c in cands {
            let kind_ok = c.get_str("kind").is_some_and(|k| DIR_CANDIDATE_KINDS.contains(&k)) || c["expanded"].truthy();
            if kind_ok && sn.starts_with(&format!("{}/", pystr(&c["path"]))) {
                return Ok(Some(sn));
            }
        }
        s = pname(&sn);
    }
    let base = _strip_ext(&s);
    let exact: BTreeSet<&String> = paths.iter().filter(|p| _strip_ext(&pname(p)) == base).collect();
    if exact.len() == 1 {
        return Ok(exact.into_iter().next().cloned());
    }
    if exact.len() > 1 {
        let named: Vec<&&String> = exact.iter().filter(|p| pname(p) == s).collect();
        return Ok(if named.len() == 1 { Some((**named[0]).clone()) } else { None });
    }
    if bare_stem_re().is_match(&base) {
        let pre = format!("{base}_");
        let sib: BTreeSet<&String> = paths.iter().filter(|p| _strip_ext(&pname(p)).starts_with(&pre)).collect();
        if sib.len() == 1 {
            return Ok(sib.into_iter().next().cloned());
        }
    }
    Ok(None)
}

pub fn require_documents(lineage: &J) -> HR<()> {
    if !lineage["documents"].truthy() {
        return fail(
            "HINT_LINEAGE_EMPTY",
            "LINEAGE 에 계보 문서가 0건이다 — 서사를 쓸 원재료가 없다.",
            Some("발행 기록의 plan·devlog·testlog 바인딩을 확인하거나 `--lineage-add PATH=REASON` 으로 보충한다."),
        );
    }
    Ok(())
}

// ── 수신자 요약 · 봉인 출처 스냅샷 ──────────────────────────────────────────────────────────────
pub fn receiver_summary(full: &J, excerpt_counts: &J, sealed_sources: &J) -> HR<J> {
    let Some(_) = full.as_dict() else {
        return fail("HINT_LINEAGE_UNREADABLE", format!("LINEAGE 전체가 사전이 아니다: {}", full.type_name()), None);
    };
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    let mut counts_order: Vec<String> = Vec::new();
    match excerpt_counts {
        J::Dict(m) => {
            for (k, v) in m.iter() {
                let n = match v {
                    J::Int(i) => *i,
                    J::Bool(b) => *b as i64,
                    _ => continue,
                };
                if n > 0 {
                    if !counts.contains_key(k) {
                        counts_order.push(k.to_string());
                    }
                    counts.insert(k.to_string(), n);
                }
            }
        }
        v if !v.truthy() => {}
        v => return Err(HintError::py("AttributeError", format!("'{}' object has no attribute 'items'", v.type_name()))),
    }
    let _ = counts_order;
    let mut docs = Vec::new();
    let mut doc_paths: BTreeSet<String> = BTreeSet::new();
    for d in full["documents"].items() {
        if !d.is_dict() || !d["path"].truthy() {
            continue;
        }
        let p = pystr(&d["path"]);
        doc_paths.insert(p.clone());
        docs.push(jd! {"path" => p.clone(), "role" => d["kind"].clone(), "date" => d["date_key"].clone(),
                       "depth" => d["depth"].clone(), "excerpts" => counts.get(&p).copied().unwrap_or(0)});
    }
    let ncands = full["evidence_candidates"].items().iter().filter(|c| c.is_dict() && c["path"].truthy()).count();
    let cited: Vec<J> = counts
        .iter()
        .filter(|(p, _)| !doc_paths.contains(*p))
        .map(|(p, n)| jd! {"path" => p.clone(), "excerpts" => *n})
        .collect();
    let empty = J::dict();
    let method = if full["method"].is_dict() { &full["method"] } else { &empty };
    let mut out = Map::new();
    out.insert("schema_version", SUMMARY_SCHEMA_VERSION);
    out.insert("kind", SUMMARY_KIND);
    out.insert(
        "note",
        concat!(
            "수신자 요약 — 서사가 읽은 계보 문서의 경로(발췌 머리 stem = 파일명) · 역할 · 날짜 · 이 페이로드의 발췌 수. 해시 · 간선 · 후보 전체 · 모호 해소 ",
            "기록은 발행자 평면(draft inputs/LINEAGE.full.json)에 있다(배포 ✗ · plan_26092908 §4.6)."
        ),
    );
    out.insert("publish_kst", method["publish_kst"].clone());
    out.insert("depth", method["depth"].clone());
    out.insert("documents", docs);
    out.insert("evidence_candidates", jd! {"count" => ncands, "cited" => cited});
    if !sealed_sources.is_null() {
        let mut rows = Vec::new();
        for x in sealed_sources.items() {
            let p = x.get("path").ok_or_else(|| HintError::py("KeyError", "'path'"))?;
            let h = x.get("sha256").ok_or_else(|| HintError::py("KeyError", "'sha256'"))?;
            rows.push(jd! {"path" => pystr(p), "sha256" => pystr(h)});
        }
        out.insert(SEALED_SOURCES_KEY, rows);
    }
    let out = J::Dict(out);
    let size = core::dumps(&out).len();
    if size > SUMMARY_MAX_BYTES {
        return fail(
            "HINT_LINEAGE_SUMMARY_TOO_LARGE",
            format!(
                "LINEAGE 수신자 요약이 {} B > 상한 {} B 다 — 요약이 아니라 전사다.",
                thousands(size),
                thousands(SUMMARY_MAX_BYTES)
            ),
            Some("계보 문서 수(깊이 · --lineage-add)를 확인한다 — 요약 칸을 늘리지 않는다(plan_26092908 §7 AC9)."),
        );
    }
    Ok(out)
}

pub fn sealed_sources(repo: &Path, paths: &J) -> HR<Vec<J>> {
    let set: BTreeSet<String> = paths.items().iter().filter(|x| x.truthy()).map(pystr).collect();
    let mut out = Vec::new();
    for p in set {
        if p.starts_with('/') || p.contains('\\') || p.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..") {
            return fail("HINT_SEALED_SOURCE_UNREADABLE", format!("봉인 출처 경로가 저장소 상대 posix 가 아니다: {}", py_repr(&p)), None);
        }
        let f = repo.join(&p);
        let data = if f.is_file() { std::fs::read(&f).ok() } else { None };
        let Some(data) = data else {
            return fail(
                "HINT_SEALED_SOURCE_UNREADABLE",
                format!("봉인 출처를 읽을 수 없다: {p}"),
                Some("린트가 통과한 출처가 봉인 직전에 사라졌다 — 출처를 복원하고 continue 를 다시 실행한다."),
            );
        };
        out.push(jd! {"path" => p, "sha256" => core::sha256_bytes(&data)});
    }
    Ok(out)
}

// ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
mod selftest_impl;
pub use selftest_impl::selftest;
