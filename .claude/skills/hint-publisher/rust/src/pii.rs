//! Port of `scripts/hintlib/pii.py` — PII 스캐너 단일본 · 발췌 기계 치환 (plan_26092119 §4.4 · SPEC §5.3).
//!
//! 판정은 배포 경로의 fail-closed 게이트다 — 원본과 **같은 검출 · 같은 순서 · 같은 줄 번호 · 같은 매치 원문 · 같은 면제 ·
//! 같은 치환**을 낸다(체계는 바꾸지 않는다 · P3). 강도 표 정본은 `.claude/rules/docs.md` §PII 스캔 적용 범위다.
//!
//! Python `re` 와의 의미 맞춤(엔진 차이를 패턴 철자로 메운다 — 판정은 그대로):
//! - `\b`: Python 의 단어 문자는 `str.isalnum()`(L* · N*) + `_` 이고 Rust `\w` 는 결합표지(M)·연결부호(Pc)를 넣고 No 를 뺀다.
//!   사설 IPv4 의 `\b` 는 언제나 숫자 바로 앞에 서므로 "앞 글자가 Python 단어 문자가 아니다" 와 같다 →
//!   `(?<![\p{L}\p{N}_])` 로 적는다.
//! - `\s`: Python(str.isspace) = Unicode White_Space + U+001C..U+001F → `[\s\x1c-\x1f]`.
//! - 위치는 바이트 오프셋으로 다룬다(정렬·줄 번호·접두 절단이 단조 대응이라 결과가 같다). 길이·절단(`[:48]`)·정렬 키의
//!   `len()` 은 code point 로 센다.
//!
//! 모듈 적재 부수효과 0 — 정규식은 첫 사용 때 컴파일된다(`re_static!`). 저장소는 언제나 인자로 받는다.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::pyre::Re;
use crate::re_static;
use crate::value::{Map, J};

// ── 정본 패턴 4종 (이 모듈이 유일한 소유자 · 이름 순서 = 원본 dict 순서) ─────────────────────────────────────────
re_static!(
    RE_PRIVATE_IPV4,
    concat!(
        r"(?<![\p{L}\p{N}_])(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}\.\d{1,3}",
        r"|(?<!==)(?<![\p{L}\p{N}_])(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}"
    )
);
re_static!(RE_EMAIL, r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}");
re_static!(RE_ABS_OP_PATH, r"/(?:mnt|home)/[A-Za-z0-9._/-]+");
re_static!(RE_SPARK_HOST, r"spark-[0-9a-f]{3,}");

/// `GENERIC_PII` 의 이름(순서 = 원본 dict 순서).
pub const GENERIC_PII_NAMES: [&str; 4] = ["private-ipv4", "email", "abs-op-path", "spark-host"];

/// `GENERIC_PII[name]`.
pub fn generic_pii(name: &str) -> Option<&'static Re> {
    match name {
        "private-ipv4" => Some(RE_PRIVATE_IPV4()),
        "email" => Some(RE_EMAIL()),
        "abs-op-path" => Some(RE_ABS_OP_PATH()),
        "spark-host" => Some(RE_SPARK_HOST()),
        _ => None,
    }
}

/// `GENERIC_PII` (이름, 패턴) — 원본 dict 순서.
#[allow(non_snake_case)]
pub fn GENERIC_PII() -> Vec<(&'static str, &'static Re)> {
    GENERIC_PII_NAMES.iter().map(|n| (*n, generic_pii(n).unwrap())).collect()
}

/// 리터럴 검출의 패턴 이름(Hit.pattern).
pub const TERM: &str = "term";

/// 강도 프로필 — 값은 GENERIC_PII 이름(순서 = GENERIC_PII 순서).
pub const PROFILES: [(&str, &[&str]); 3] = [
    ("deploy", &["private-ipv4", "email", "abs-op-path", "spark-host"]),
    ("nondeploy_prose", &["private-ipv4", "email", "spark-host"]),
    ("identity", &["private-ipv4", "abs-op-path", "spark-host"]),
];

fn profile(name: &str) -> Option<&'static [&'static str]> {
    PROFILES.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

// 문서 절번호(§10.1.x · "#### 10.1 …")는 사설 IPv4 가 아니다 — 같은 줄 안 매치 바로 앞의 텍스트로 가른다(tripwire).
re_static!(RE_SECTION_ANCHOR, r"(?:§[\s\x1c-\x1f]*|^#{1,6}[\s\x1c-\x1f]+)$");
const ANCHOR_EXCLUDED: [&str; 1] = ["private-ipv4"];
// `${VAR:-…}` 기본값의 구조적 판별자(매치 직전 텍스트가 `${…:-` 로 끝날 때만 면제 · 면제는 기록한다).
re_static!(RE_SHELL_DEFAULT, r"\$\{[A-Za-z_][A-Za-z0-9_]*:-$");
const SHELL_DEFAULT_PATTERNS: [&str; 1] = ["abs-op-path"];

/// 표시용 매치 절단 길이(code point). Hit.match 자체는 자르지 않는다.
pub const SHOW: usize = 48;

// ── Python 문자열 의미 ───────────────────────────────────────────────────────────────────────
/// `str.isspace()` of one character.
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}
fn py_lstrip(s: &str) -> &str {
    s.trim_start_matches(py_isspace)
}
fn clen(s: &str) -> usize {
    s.chars().count()
}
/// `str.splitlines()` (no keepends).
fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let brk = matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if brk {
            out.push(&s[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(_, '\n')) = it.peek() {
                    it.next();
                    end += 1;
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
/// Python `type(x).__name__` of a bridged value (`{"__bytes__"}`/`{"__tuple__"}`/`{"__set__"}` decode to bytes/tuple/set).
pub fn py_type(v: &J) -> &'static str {
    if let J::Dict(m) = v {
        if m.len() == 1 {
            match m.0[0].0.as_str() {
                "__bytes__" => return "bytes",
                "__tuple__" => return "tuple",
                "__set__" => return "set",
                "__path__" => return "PosixPath",
                _ => {}
            }
        }
    }
    v.type_name()
}
/// `re.escape(s)` text (Python 3.7+ special set) — the text the original builds and shows.
fn py_re_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\u{0b}\u{0c}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
/// A literal-matching escape this engine compiles unambiguously (every ASCII punctuation/space as `\xHH`).
fn engine_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_alphanumeric() && c != '_' {
            out.push_str(&format!("\\x{:02X}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}
fn os_error_type(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(1) | Some(13) => "PermissionError",
        Some(2) => "FileNotFoundError",
        Some(17) => "FileExistsError",
        Some(20) => "NotADirectoryError",
        Some(21) => "IsADirectoryError",
        Some(4) => "InterruptedError",
        Some(11) => "BlockingIOError",
        _ => "OSError",
    }
}

// ── Hit ───────────────────────────────────────────────────────────────────────────────────────
/// 검출 1건. pattern = GENERIC_PII 이름 또는 `"term"` · match = 매치 원문 · line = 1-기반 줄 번호(LF 개수로 센다).
/// `Display` = `"<pattern>:<match>"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub pattern: String,
    pub r#match: String,
    pub line: i64,
}

impl Hit {
    pub fn new(pattern: &str, m: &str, line: i64) -> Hit {
        Hit { pattern: pattern.to_string(), r#match: m.to_string(), line }
    }
    /// Bridge encoding (`{"__class__": "Hit", ...}`).
    pub fn to_j(&self) -> J {
        crate::jd! {"__class__" => "Hit", "pattern" => self.pattern.clone(), "match" => self.r#match.clone(), "line" => self.line}
    }
}

impl std::fmt::Display for Hit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.pattern, self.r#match)
    }
}

/// One item of `render_hits`' iterable: a bare `Hit` or a `(rel, Hit)` tuple (the original accepts a mix).
#[derive(Clone, Debug)]
pub enum HitItem {
    Bare(Hit),
    Located(String, Hit),
}

/// `render_hits` over a possibly mixed list (`limit` keeps Python slice semantics, negative included).
pub fn _render_items(items: &[HitItem], limit: i64) -> String {
    let n = items.len() as i64;
    let take = if limit >= 0 { limit.min(n) } else { (n + limit).max(0) } as usize;
    let mut rows: Vec<String> = Vec::new();
    for item in &items[..take] {
        match item {
            HitItem::Located(w, h) => rows.push(format!("{w}:{}: {}:{}", h.line, h.pattern, core::head_chars(&h.r#match, SHOW))),
            HitItem::Bare(h) => rows.push(format!("{}: {}:{}", h.line, h.pattern, core::head_chars(&h.r#match, SHOW))),
        }
    }
    if n > limit {
        rows.push(format!("… 외 {}건", n - limit));
    }
    rows.join("\n  ")
}

/// 차단 메시지용 표시(`line: pattern:match48`). 초과분은 개수만 적는다. (Python 기본 limit=20)
pub fn render_hits(hits: &[Hit], limit: usize) -> HR<String> {
    let items: Vec<HitItem> = hits.iter().cloned().map(HitItem::Bare).collect();
    Ok(_render_items(&items, limit as i64))
}

/// `render_hits` 의 `(rel, Hit)` 형(`rel:line: pattern:match48`).
pub fn render_located_hits(hits: &[(String, Hit)], limit: usize) -> HR<String> {
    let items: Vec<HitItem> = hits.iter().map(|(r, h)| HitItem::Located(r.clone(), h.clone())).collect();
    Ok(_render_items(&items, limit as i64))
}

// ── 용어 파일 ──────────────────────────────────────────────────────────────────────────────────
/// `.claude/pii_terms.txt` 의 리터럴 목록. **부재 = None** · 있는데 읽을 수 없으면 결함(HINT_PII_TERMS_UNREADABLE).
pub fn load_pii_terms(repo: &Path) -> HR<Option<Vec<String>>> {
    let p = core::aux_root(repo).join(core::REL_PII_TERMS);
    if !p.exists() {
        return Ok(None);
    }
    if !p.is_file() {
        return core::fail(
            "HINT_PII_TERMS_UNREADABLE",
            format!("{} 가 파일이 아니다", core::REL_PII_TERMS),
            Some("한 줄에 리터럴 하나를 둔 텍스트 파일이어야 한다."),
        );
    }
    let unreadable = |ty: &str| -> HR<Option<Vec<String>>> {
        core::fail(
            "HINT_PII_TERMS_UNREADABLE",
            format!("{} 를 읽을 수 없다: {ty}", core::REL_PII_TERMS),
            Some("UTF-8 텍스트로 고친다(리터럴 검사를 조용히 끄지 않는다)."),
        )
    };
    let raw = match std::fs::read(&p) {
        Ok(b) => match String::from_utf8(b) {
            Ok(s) => s,
            Err(_) => return unreadable("UnicodeDecodeError"),
        },
        Err(e) => return unreadable(os_error_type(&e)),
    };
    let mut terms = Vec::new();
    for line in py_splitlines(&raw) {
        let s = py_strip(line);
        if !s.is_empty() && !s.starts_with('#') {
            terms.push(s.to_string());
        }
    }
    Ok(Some(terms))
}

/// 배포면용 — 용어 파일 부재 = 차단(HINT_PII_TERMS_ABSENT). 빈 파일(리터럴 0개)은 "있음" 이다.
pub fn require_terms(repo: &Path) -> HR<Vec<String>> {
    match load_pii_terms(repo)? {
        Some(t) => Ok(t),
        None => core::fail(
            "HINT_PII_TERMS_ABSENT",
            format!("`{}` 부재 — 배포면 PII-clean 을 인증할 수 없다(fail-closed)", core::REL_PII_TERMS),
            Some(&format!(
                "`{}` 를 만든다(한 줄에 리터럴 하나 · `#` 주석). 비추적 파일이라 새 클론의 기본 상태는 부재다(감사 ⑫). 리터럴이 정말 없으면 빈 파일로 그 사실을 선언한다.",
                core::REL_PII_TERMS
            )),
        ),
    }
}

// ── 스캐너 ─────────────────────────────────────────────────────────────────────────────────────
fn str_list_repr<S: AsRef<str>>(v: &[S]) -> String {
    let parts: Vec<String> = v.iter().map(|s| crate::value::py_repr(s.as_ref())).collect();
    format!("[{}]", parts.join(", "))
}

/// 프로필의 패턴 이름(순서 = GENERIC_PII 순서)에서 skip 을 뺀 것. 모르는 프로필·제외 이름은 소리내어 실패한다.
pub fn _patterns_for(profile_name: &str, skip_generic: &[String]) -> HR<Vec<&'static str>> {
    let Some(names) = profile(profile_name) else {
        let mut keys: Vec<&str> = PROFILES.iter().map(|(n, _)| *n).collect();
        keys.sort();
        return core::fail(
            "HINT_PII_PROFILE_UNKNOWN",
            format!("PII 프로필 {} 은 없다", crate::value::py_repr(profile_name)),
            Some(&format!("프로필은 {} 중 하나다(강도 표 정본 = docs.md §PII 스캔 적용 범위).", str_list_repr(&keys))),
        );
    };
    let skip: BTreeSet<&str> = skip_generic.iter().map(String::as_str).collect();
    let unknown: Vec<&str> = skip.iter().copied().filter(|s| !GENERIC_PII_NAMES.contains(s)).collect();
    if !unknown.is_empty() {
        return core::fail(
            "HINT_PII_PATTERN_UNKNOWN",
            format!("skip_generic 에 없는 패턴 이름: {}", str_list_repr(&unknown)),
            Some(&format!("패턴 이름은 {} 중 하나다.", str_list_repr(&GENERIC_PII_NAMES))),
        );
    }
    Ok(names.iter().copied().filter(|n| !skip.contains(n)).collect())
}

fn line_prefix(text: &str, start: usize) -> &str {
    let from = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    &text[from..start]
}

/// `_line_prefix(text, start)` — `start` is a byte offset here.
pub fn _line_prefix(text: &str, start: usize) -> String {
    line_prefix(text, start).to_string()
}

/// `start` 의 매치가 같은 줄 안에서 `§` 또는 마크다운 제목 표지 바로 뒤에 있으면 참(절번호).
pub fn _is_section_anchored(text: &str, start: usize) -> bool {
    RE_SECTION_ANCHOR().is_search(line_prefix(text, start))
}

pub fn _is_shell_default(text: &str, start: usize) -> bool {
    RE_SHELL_DEFAULT().is_search(line_prefix(text, start))
}

/// 매치가 주석 줄 안에 있고 같은 텍스트가 그 리터럴을 셸 기본값(`${VAR:-<리터럴>}`)으로 이미 싣고 있으면 참.
pub fn _is_comment_echo_of_default(text: &str, start: usize, literal: &str) -> bool {
    let line = line_prefix(text, start);
    if !py_lstrip(line).starts_with('#') {
        return false;
    }
    let pat = format!(r"\$\{{[A-Za-z_][A-Za-z0-9_]*:-{}[}}]", engine_escape(literal));
    match Re::try_new(&pat, 0) {
        Ok(r) => r.is_search(text),
        Err(_) => false,
    }
}

/// 면제는 조용히 넘기지 않는다 — exempt_log 가 있으면 거기, 없으면 stderr.
fn note_exempt(exempt_log: Option<&mut Vec<J>>, kind: &str, record: &str) {
    match exempt_log {
        Some(log) => log.push(J::Str(format!("{kind} {record}"))),
        None => crate::io::errln(&format!("[hint] EXEMPT({kind}) {record}")),
    }
}

/// The `terms` argument as the original sees it: `Ok(None)` = no literals, `Ok(Some(list))`, or the shape error the
/// original raises at the moment it iterates the argument (`_dedupe`).
pub type TermsArg = Result<Option<Vec<String>>, HintError>;

fn shape_single_string() -> HintError {
    HintError::new("HINT_PII_TERMS_SHAPE", "리터럴 목록 자리에 문자열 하나가 왔다", Some("[리터럴] 목록으로 넘긴다."))
}

/// Decode a bridged `terms` value exactly as `_dedupe` would iterate it.
pub fn _terms_from_j(v: &J) -> TermsArg {
    let items: Vec<J> = match v {
        J::Null => return Ok(None),
        J::Str(_) => return Err(shape_single_string()),
        J::Dict(_) if py_type(v) == "bytes" => return Err(shape_single_string()),
        J::Dict(m) if matches!(py_type(v), "tuple" | "set") => m.0[0].1.items().to_vec(),
        J::Dict(m) => m.keys().map(J::from).collect(),
        J::List(l) => l.clone(),
        J::Bool(false) | J::Int(0) => return Ok(None),
        J::Float(f) if *f == 0.0 => return Ok(None),
        other => return Err(HintError::py("TypeError", format!("'{}' object is not iterable", other.type_name()))),
    };
    let mut out = Vec::new();
    for t in &items {
        match t {
            J::Str(s) => out.push(s.clone()),
            other => {
                return Err(HintError::new("HINT_PII_TERMS_SHAPE", format!("리터럴은 문자열이어야 한다: {}", py_type(other)), None));
            }
        }
    }
    Ok(Some(out))
}

/// `_dedupe(terms)` — 빈 리터럴 제외 · 첫 등장 순서 보존.
pub fn _dedupe(terms: &TermsArg) -> HR<Vec<String>> {
    let list = match terms {
        Err(e) => return Err(e.clone()),
        Ok(None) => return Ok(Vec::new()),
        Ok(Some(l)) => l,
    };
    let mut out: Vec<String> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for t in list {
        if !t.is_empty() && seen.insert(t.as_str()) {
            out.push(t.clone());
        }
    }
    Ok(out)
}

/// `scan_text` for a bridged (possibly non-str) `text`: the original's `HINT_PII_INPUT_NOT_TEXT` guard.
pub fn _text_arg<'a>(text: &'a J, func: &str) -> HR<&'a str> {
    match text {
        J::Str(s) => Ok(s),
        other if func == "scan_text" => core::fail(
            "HINT_PII_INPUT_NOT_TEXT",
            format!("scan_text 는 str 을 받는다: {}", py_type(other)),
            Some("바이트는 호출부가 UTF-8 로 해독한다(해독 불가 = 바이너리 → scan_tree 규칙)."),
        ),
        other => core::fail("HINT_PII_INPUT_NOT_TEXT", format!("{func} 는 str 을 받는다: {}", py_type(other)), None),
    }
}

/// `scan_text` with the `terms` argument as the original receives it (see [`TermsArg`]).
#[allow(clippy::too_many_arguments)]
pub fn _scan_text(
    text: &str,
    terms: &TermsArg,
    profile_name: &str,
    skip_generic: &[String],
    shell_default_exempt: bool,
    section_anchor_exempt: bool,
    mut exempt_log: Option<&mut Vec<J>>,
) -> HR<Vec<Hit>> {
    let names = _patterns_for(profile_name, skip_generic)?;
    let mut found: Vec<(usize, usize, Hit)> = Vec::new();
    let lits = _dedupe(terms)?;
    let line_of = |pos: usize| text.as_bytes()[..pos].iter().filter(|&&b| b == b'\n').count() as i64 + 1;
    for (order, t) in lits.iter().enumerate() {
        let mut from = 0;
        while let Some(i) = text[from..].find(t.as_str()) {
            let pos = from + i;
            found.push((pos, order, Hit::new(TERM, t, line_of(pos))));
            from = pos + t.len();
        }
    }
    let base = lits.len();
    for (idx, name) in GENERIC_PII_NAMES.iter().enumerate() {
        if !names.contains(name) {
            continue;
        }
        for m in generic_pii(name).unwrap().finditer(text) {
            let m0 = m.get(0).unwrap();
            let start = m0.start();
            let mtext = m0.as_str();
            if section_anchor_exempt && ANCHOR_EXCLUDED.contains(name) && _is_section_anchored(text, start) {
                continue; // 문서 절번호(§10.x) — 무손실 증명된 예외(plan_26081516 H1)
            }
            let line = line_of(start);
            if shell_default_exempt && SHELL_DEFAULT_PATTERNS.contains(name) && _is_shell_default(text, start) {
                note_exempt(exempt_log.as_deref_mut(), "shell-default", &format!("{line}: {name}:{}", core::head_chars(mtext, SHOW)));
                continue;
            }
            if shell_default_exempt && SHELL_DEFAULT_PATTERNS.contains(name) && _is_comment_echo_of_default(text, start, mtext) {
                note_exempt(
                    exempt_log.as_deref_mut(),
                    "comment-echo-of-default",
                    &format!("{line}: {name}:{}", core::head_chars(mtext, SHOW)),
                );
                continue;
            }
            found.push((start, base + idx, Hit::new(name, mtext, line)));
        }
    }
    found.sort_by_key(|x| (x.0, x.1));
    Ok(found.into_iter().map(|x| x.2).collect())
}

/// 텍스트 하나를 리터럴 + 프로필 패턴으로 스캔해 **전부**를 돌려준다(빈 목록 = 무검출). 반환 순서 = (위치, 리터럴 순서 →
/// GENERIC_PII 순서). Python 기본값: profile="deploy", skip_generic=∅, shell_default_exempt=False, section_anchor_exempt=True.
pub fn scan_text(
    text: &str,
    terms: Option<&[String]>,
    profile: &str,
    skip_generic: &[String],
    shell_default_exempt: bool,
    section_anchor_exempt: bool,
    exempt_log: Option<&mut Vec<J>>,
) -> HR<Vec<Hit>> {
    _scan_text(text, &Ok(terms.map(<[String]>::to_vec)), profile, skip_generic, shell_default_exempt, section_anchor_exempt, exempt_log)
}

// ── 트리 스캔 ─────────────────────────────────────────────────────────────────────────────────
/// `Path(s)` (pathlib normalisation: repeated `/` and `.` parts dropped; empty = `.`).
fn pypath(s: &str) -> PathBuf {
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let mut out = String::new();
    if s.starts_with('/') {
        out.push('/');
    }
    out.push_str(&parts.join("/"));
    if out.is_empty() {
        out.push('.');
    }
    PathBuf::from(out)
}
/// `Path.name`.
fn py_name(p: &Path) -> String {
    match p.components().next_back() {
        Some(Component::Normal(s)) => s.to_string_lossy().into_owned(),
        Some(Component::ParentDir) => "..".into(),
        _ => String::new(),
    }
}
/// `Path.parent`.
fn py_parent(p: &Path) -> PathBuf {
    match p.parent() {
        Some(q) if !q.as_os_str().is_empty() => q.to_path_buf(),
        Some(_) => PathBuf::from("."),
        None => p.to_path_buf(),
    }
}
/// `root / r` with pathlib normalisation.
fn pjoin(root: &Path, r: &str) -> PathBuf {
    let mut p = if root == Path::new(".") { PathBuf::new() } else { root.to_path_buf() };
    for part in r.split('/').filter(|x| !x.is_empty() && *x != ".") {
        p.push(part);
    }
    if p.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        p
    }
}
fn rel_posix(p: &Path, base: &Path) -> String {
    let r = if base == Path::new(".") { p } else { p.strip_prefix(base).unwrap_or(p) };
    core::posix(r)
}

/// 스캔할 텍스트: 심링크 = 링크 문자열 · 파일 = UTF-8 해독 · 해독 불가 = None(바이너리).
pub fn _read_scannable(path: &Path) -> HR<Option<String>> {
    if path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return std::fs::read_link(path).map(|t| Some(core::posix(&t))).map_err(|e| core::io_err(&e, path));
    }
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            return core::fail("HINT_PII_PATH_UNREADABLE", format!("스캔 대상을 읽을 수 없다: {} ({})", py_name(path), os_error_type(&e)), None)
        }
    };
    Ok(String::from_utf8(data).ok())
}

/// 결정론 순서(이름 정렬)의 재귀 목록. 디렉터리 심링크는 따라가지 않는다 · 특수 파일은 거부.
pub fn _walk(root: &Path, base: &Path) -> HR<Vec<(String, PathBuf)>> {
    let rd = std::fs::read_dir(root).map_err(|e| core::io_err(&e, root))?;
    let mut entries: Vec<(String, std::fs::FileType)> = Vec::new();
    for e in rd {
        let e = e.map_err(|e| core::io_err(&e, root))?;
        let ft = e.file_type().map_err(|err| core::io_err(&err, &e.path()))?;
        entries.push((e.file_name().to_string_lossy().into_owned(), ft));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = Vec::new();
    for (name, ft) in entries {
        let p = if root == Path::new(".") { PathBuf::from(&name) } else { root.join(&name) };
        if ft.is_symlink() {
            out.push((rel_posix(&p, base), p));
        } else if ft.is_dir() {
            out.extend(_walk(&p, base)?);
        } else if ft.is_file() {
            out.push((rel_posix(&p, base), p));
        } else {
            return core::fail(
                "HINT_PII_UNSCANNABLE_FILE",
                format!("일반 파일이 아닌 항목: {}", rel_posix(&p, base)),
                Some("스캔 트리에서 특수 파일(FIFO·소켓·장치)을 치운다."),
            );
        }
    }
    Ok(out)
}

/// rels(루트 상대) → 목록. 절대경로·상위탈출·부재는 거부 · 디렉터리는 펼친다. (`rels` 항목은 원본처럼 str 이 아닐 수 있다.)
pub fn _selected(root: &Path, rels: &[J]) -> HR<Vec<(String, PathBuf)>> {
    let root = pypath(&core::posix(root));
    let root_res = core::resolve(&root);
    let mut out: BTreeMap<String, PathBuf> = BTreeMap::new();
    for rj in rels {
        let r = match rj {
            J::Str(s) if !s.is_empty() && !s.starts_with('/') && !s.split('/').any(|p| p == "..") => s.as_str(),
            other => {
                return core::fail("HINT_PII_PATH_INVALID", format!("스캔 경로는 루트 상대여야 한다(절대·상위탈출·빈 값 ✗): {}", other.repr()), None)
            }
        };
        let p = pjoin(&root, r);
        let is_link = p.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if !(is_link || p.exists()) {
            return core::fail("HINT_PII_PATH_ABSENT", format!("스캔 대상 부재: {r}"), None);
        }
        if !core::resolve(&py_parent(&p)).starts_with(&root_res) {
            return core::fail("HINT_PII_PATH_INVALID", format!("스캔 경로가 루트 밖으로 이어진다(상위 심링크): {r}"), None);
        }
        if p.is_dir() && !is_link {
            for (rr, pp) in _walk(&p, &root)? {
                out.insert(rr, pp);
            }
        } else {
            out.insert(core::posix(&pypath(r)), p);
        }
    }
    Ok(out.into_iter().collect())
}

/// `scan_tree` with the `terms`/`rels` arguments as the original receives them.
#[allow(clippy::too_many_arguments)]
pub fn _scan_tree(
    root: &Path,
    terms: &TermsArg,
    profile_name: &str,
    skip_generic: &[String],
    shell_default_exempt: bool,
    section_anchor_exempt: bool,
    rels: Option<&[J]>,
    mut exempt_log: Option<&mut Vec<J>>,
) -> HR<Vec<(String, Hit)>> {
    let root = pypath(&core::posix(root));
    if !root.is_dir() {
        return core::fail("HINT_PII_ROOT_ABSENT", format!("스캔 루트가 디렉터리가 아니다: {}", py_name(&root)), None);
    }
    _patterns_for(profile_name, skip_generic)?; // 빈 트리여도 프로필 오타는 잡는다
    let entries = match rels {
        None => _walk(&root, &root)?,
        Some(r) => _selected(&root, r)?,
    };
    let lits = _dedupe(terms)?;
    let lits_arg: TermsArg = Ok(Some(lits));
    let mut out = Vec::new();
    for (rel_path, path) in entries {
        let Some(text) = _read_scannable(&path)? else {
            if let Some(log) = exempt_log.as_deref_mut() {
                log.push(J::Str(format!("binary {rel_path}: UTF-8 해독 불가 — 건너뜀")));
            }
            continue;
        };
        let mut local: Vec<J> = Vec::new();
        let hits = _scan_text(&text, &lits_arg, profile_name, skip_generic, shell_default_exempt, section_anchor_exempt, Some(&mut local))?;
        for rec in &local {
            let rec = rec.as_str().unwrap_or_default();
            let (kind, rest) = rec.split_once(' ').unwrap_or((rec, ""));
            note_exempt(exempt_log.as_deref_mut(), kind, &format!("{rel_path}:{rest}"));
        }
        out.extend(hits.into_iter().map(|h| (rel_path.clone(), h)));
    }
    Ok(out)
}

/// 트리(또는 그 안의 rels)를 스캔해 `(루트 상대 posix 경로, Hit)` 전부를 돌려준다. Python 기본값: profile="deploy",
/// skip_generic=∅, **shell_default_exempt=True**, section_anchor_exempt=True, rels=None.
#[allow(clippy::too_many_arguments)]
pub fn scan_tree(
    root: &Path,
    terms: Option<&[String]>,
    profile: &str,
    skip_generic: &[String],
    shell_default_exempt: bool,
    section_anchor_exempt: bool,
    rels: Option<&[String]>,
    exempt_log: Option<&mut Vec<J>>,
) -> HR<Vec<(String, Hit)>> {
    let rels_j: Option<Vec<J>> = rels.map(|r| r.iter().map(J::from).collect());
    _scan_tree(
        root,
        &Ok(terms.map(<[String]>::to_vec)),
        profile,
        skip_generic,
        shell_default_exempt,
        section_anchor_exempt,
        rels_j.as_deref(),
        exempt_log,
    )
}

// ── 발췌 기계 치환 (plan §4.4) ─────────────────────────────────────────────────────────────────
pub const PH_REPO: &str = "<repo>";
pub const PH_HOME: &str = "<home>";
pub const PH_PRIV_IP: &str = "<priv-ip>";
pub const PH_HOST: &str = "<host>";
pub const PH_ABS_PATH: &str = "<abs-path>";
pub const PH_REDACTED: &str = "<redacted>";
/// 치환표가 놓친 잔여를 4종 패턴으로 가리는 표지(순서 = 원본 dict 순서).
pub const RESIDUAL_PLACEHOLDERS: [(&str, &str); 4] =
    [("private-ipv4", PH_PRIV_IP), ("email", PH_REDACTED), ("abs-op-path", PH_ABS_PATH), ("spark-host", PH_HOST)];

pub fn node_placeholder(label: &str) -> HR<String> {
    Ok(format!("<node:{label}>"))
}
pub fn manifest_placeholder(field: &str) -> HR<String> {
    Ok(format!("<manifest.{field}>"))
}
pub fn nic_placeholder(label: &str) -> HR<String> {
    Ok(format!("<nic:{label}>"))
}

// NIC·HCA 장치 이름(닫힌 키 토큰 · tripwire).
const NIC_KEY_TOKENS: [&str; 10] = ["iface", "ifaces", "ifname", "interface", "interfaces", "nic", "nics", "netdev", "hca", "hcas"];
re_static!(RE_NIC_VALUE, r"[A-Za-z][A-Za-z0-9._:-]{1,31}");
re_static!(RE_NIC_KEY_SUFFIX, r"\[[^\]]*\]$");
pub const NIC_LABEL_SHARED: &str = "cluster";

pub fn _is_nic_key(key: &str) -> bool {
    let base = RE_NIC_KEY_SUFFIX().sub_literal(key, "").to_lowercase();
    base.split('_').any(|tok| NIC_KEY_TOKENS.contains(&tok))
}

/// NIC 필드 값 → 장치 이름 목록(문자열 · 문자열 목록 · NCCL 쉼표 목록). 모양이 아니거나 숫자가 없으면 버린다.
pub fn _nic_values(val: &J) -> Vec<String> {
    let items: Vec<J> = match val {
        J::List(l) => l.clone(),
        other => vec![other.clone()],
    };
    let mut out = Vec::new();
    for it in &items {
        let Some(s) = it.as_str() else { continue };
        for part in s.split(',') {
            let v = py_strip(py_strip(part).trim_start_matches(['^', '=']));
            if RE_NIC_VALUE().is_full(v) && v.chars().any(|c| c.is_ascii_digit()) && !_is_ipv4(v) {
                out.push(v.to_string());
            }
        }
    }
    out
}

fn sorted_keys(m: &Map) -> Vec<&str> {
    let mut k: Vec<&str> = m.keys().collect();
    k.sort();
    k
}

/// dict 를 훑어 NIC 키의 값을 모은다(최상위 nodes 목록은 호출부가 따로 — 라벨이 다르다).
pub fn _walk_nic(obj: &J, key: &str, out: &mut Vec<String>) {
    match obj {
        J::Dict(m) => {
            for k in sorted_keys(m) {
                if k == "nodes" && key.is_empty() {
                    continue;
                }
                let v = m.get(k).unwrap();
                let shaped = match v {
                    J::Str(_) => true,
                    J::List(l) => l.iter().all(J::is_str),
                    _ => false,
                };
                if _is_nic_key(k) && shaped {
                    out.extend(_nic_values(v));
                } else {
                    _walk_nic(v, k, out);
                }
            }
        }
        J::List(l) => {
            for item in l {
                _walk_nic(item, key, out);
            }
        }
        _ => {}
    }
}

// 치환표 행의 우선순위(같은 리터럴이면 낮은 값이 이긴다 · 동률은 표지 사전순).
const PRIO_REPO: u8 = 0;
const PRIO_NODE: u8 = 1;
const PRIO_NIC: u8 = 2;
const PRIO_FIELD: u8 = 3;
const PRIO_HOME: u8 = 4;
const PRIO_GENERIC: u8 = 5;
const PRIO_TERM: u8 = 6;
fn prio_name(p: u8) -> &'static str {
    match p {
        PRIO_REPO => "repo",
        PRIO_NODE => "manifest.nodes",
        PRIO_NIC => "manifest NIC 필드",
        PRIO_FIELD => "manifest 경로 필드",
        PRIO_HOME => "home",
        PRIO_GENERIC => "manifest 값",
        _ => "pii_terms",
    }
}
re_static!(RE_IPV4, r"\d{1,3}(?:\.\d{1,3}){3}");
re_static!(RE_HOSTNAME, r"[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?");
re_static!(RE_ROLE, r"[a-z][a-z0-9_-]*");
re_static!(RE_USER, r"[A-Za-z0-9._-]+");
re_static!(RE_ND, r"\d");
const LOOPBACK: [&str; 2] = ["localhost", "0.0.0.0"];

/// Decimal value of a Unicode `Nd` character (`int()` accepts every Nd digit). Unicode encodes Nd digits in contiguous
/// runs of whole 0..9 blocks, so the value is the position in the run modulo 10.
fn nd_value(c: char) -> u32 {
    if c.is_ascii_digit() {
        return c as u32 - '0' as u32;
    }
    let mut n = 0u32;
    let mut x = c as u32;
    while x > 0 {
        x -= 1;
        match char::from_u32(x) {
            Some(p) if RE_ND().is_full(p.encode_utf8(&mut [0u8; 4])) => n += 1,
            _ => break,
        }
    }
    n % 10
}

pub fn _is_ipv4(v: &str) -> bool {
    RE_IPV4().is_full(v) && v.split('.').all(|o| o.chars().fold(0u32, |acc, c| acc * 10 + nd_value(c)) <= 255)
}

pub fn _is_host_key(key: &str) -> bool {
    let k = key.to_lowercase();
    k == "host" || k == "hostname" || k == "ssh_host" || k.ends_with("_host") || k.ends_with("_hostname")
}

/// 운영자 절대경로 후보: `/` 로 시작 · 성분 2개 이상 · 공백 없음.
pub fn _is_abs_path(v: &str) -> bool {
    if !v.starts_with('/') || v.starts_with("//") || v.chars().any(py_isspace) {
        return false;
    }
    v.split('/').filter(|p| !p.is_empty()).count() >= 2
}

/// 저장소 경로의 사용자 홈 접두 → 그 홈(저장소 경로에서 파생 · 환경변수 HOME 을 읽지 않는다).
pub fn _home_of(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0].is_empty() && parts[1] == "home" && RE_USER().is_full(parts[2]) {
        return Some(format!("/{}/{}", "home", parts[2]));
    }
    None
}

/// 노드 표지 라벨: role · 같은 role 이 둘 이상이면 서수 · role 이 없거나 이상하면 인덱스.
pub fn _node_labels(nodes: &[J]) -> Vec<String> {
    let ok: Vec<Option<&str>> = nodes
        .iter()
        .map(|n| match n {
            J::Dict(m) => match m.get("role") {
                Some(J::Str(r)) if RE_ROLE().is_full(r) => Some(r.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in ok.iter().flatten() {
        *counts.entry(r).or_insert(0) += 1;
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut labels = Vec::new();
    for (i, r) in ok.iter().enumerate() {
        match r {
            None => labels.push(i.to_string()),
            Some(r) if counts[r] == 1 => labels.push(r.to_string()),
            Some(r) => {
                let s = seen.entry(r).or_insert(0);
                *s += 1;
                labels.push(format!("{r}{s}"));
            }
        }
    }
    labels
}

/// manifest 의 모든 문자열 값을 (점 경로, 값) 으로 — 키 정렬. 최상위 nodes 목록은 role 라벨로 색인한다.
pub fn _walk_strings(obj: &J, prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    match obj {
        J::Dict(m) => {
            for key in sorted_keys(m) {
                let path = if prefix.is_empty() { key.to_string() } else { format!("{prefix}.{key}") };
                let v = m.get(key).unwrap();
                match v {
                    J::List(items) if key == "nodes" && prefix.is_empty() => {
                        let labels = _node_labels(items);
                        for (i, item) in items.iter().enumerate() {
                            out.extend(_walk_strings(item, &format!("nodes[{}]", labels[i])));
                        }
                    }
                    _ => out.extend(_walk_strings(v, &path)),
                }
            }
        }
        J::List(l) => {
            for (i, item) in l.iter().enumerate() {
                out.extend(_walk_strings(item, &format!("{prefix}[{i}]")));
            }
        }
        J::Str(s) => out.push((prefix.to_string(), s.clone())),
        _ => {}
    }
    out
}

/// `os.path.abspath` (lexical).
fn abspath(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut out = PathBuf::from("/");
    for c in abs.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
            _ => {}
        }
    }
    out
}

/// 발췌 기계 치환표 `[(리터럴, 표지)]` — 결정론 · 긴 리터럴 우선(길이 내림차순 → 사전순). `manifest` = J::Null 이면 repo·home·terms 만.
pub fn substitution_table(repo: &Path, manifest: &J) -> HR<Vec<(String, String)>> {
    let mut best: BTreeMap<String, (u8, String)> = BTreeMap::new();
    let mut add = |prio: u8, literal: &str, ph: String| {
        if literal.is_empty() {
            return;
        }
        match best.get(literal) {
            Some(cur) if (prio, ph.as_str()) >= (cur.0, cur.1.as_str()) => {}
            _ => {
                best.insert(literal.to_string(), (prio, ph));
            }
        }
    };

    let mut repo_forms: Vec<String> = Vec::new();
    for f in [core::posix(&abspath(repo)), core::posix(&core::resolve(repo))] {
        if !repo_forms.contains(&f) {
            repo_forms.push(f);
        }
    }
    for rp in &repo_forms {
        if _is_abs_path(rp) {
            add(PRIO_REPO, rp.trim_end_matches('/'), PH_REPO.to_string());
        }
        if let Some(home) = _home_of(rp) {
            add(PRIO_HOME, &home, PH_HOME.to_string());
        }
    }

    if !manifest.is_null() {
        let J::Dict(mm) = manifest else {
            return core::fail("HINT_PII_MANIFEST_SHAPE", format!("manifest 는 dict 여야 한다: {}", py_type(manifest)), None);
        };
        let nodes_j = mm.get("nodes").filter(|v| v.truthy()).cloned().unwrap_or(J::List(Vec::new()));
        let J::List(nodes) = &nodes_j else {
            return core::fail("HINT_PII_MANIFEST_SHAPE", "manifest.nodes 는 목록이어야 한다", None);
        };
        let labels = _node_labels(nodes);
        let mut nic_owner: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (i, node) in nodes.iter().enumerate() {
            let J::Dict(nm) = node else {
                return core::fail("HINT_PII_MANIFEST_SHAPE", format!("manifest.nodes[{i}] 가 mapping 이 아니다"), None);
            };
            for key in sorted_keys(nm) {
                let Some(val) = nm.get(key).and_then(J::as_str) else { continue };
                let v = py_strip(val);
                if LOOPBACK.contains(&v) || v.starts_with("127.") {
                    continue; // 루프백은 노드 지문이 아니다
                }
                if _is_ipv4(v) || (_is_host_key(key) && clen(v) >= 3 && RE_HOSTNAME().is_full(v)) {
                    add(PRIO_NODE, v, node_placeholder(&labels[i])?);
                }
                if key == "ssh_user" && RE_USER().is_full(v) {
                    add(PRIO_HOME, &format!("/{}/{v}", "home"), PH_HOME.to_string());
                }
            }
            let mut node_nics = Vec::new();
            _walk_nic(node, "node", &mut node_nics);
            for nic in node_nics {
                nic_owner.entry(nic).or_default().insert(labels[i].clone());
            }
        }
        let mut top_nics = Vec::new();
        _walk_nic(manifest, "", &mut top_nics);
        for nic in top_nics {
            nic_owner.entry(nic).or_default().insert(NIC_LABEL_SHARED.to_string());
        }
        for (nic, owners) in &nic_owner {
            let label = if owners.len() == 1 { owners.iter().next().unwrap().as_str() } else { NIC_LABEL_SHARED };
            add(PRIO_NIC, nic, nic_placeholder(label)?);
        }
        for (field, val) in _walk_strings(manifest, "") {
            let v = py_strip(&val);
            if _is_abs_path(v) {
                add(PRIO_FIELD, v.trim_end_matches('/'), manifest_placeholder(&field)?);
            } else if _is_ipv4(v) && RE_PRIVATE_IPV4().is_full(v) {
                add(PRIO_GENERIC, v, PH_PRIV_IP.to_string());
            } else if RE_SPARK_HOST().is_full(v) {
                add(PRIO_GENERIC, v, PH_HOST.to_string());
            }
        }
    }

    for t in _dedupe(&Ok(load_pii_terms(repo)?))? {
        add(PRIO_TERM, &t, PH_REDACTED.to_string());
    }

    let mut placeholders: BTreeSet<String> = best.values().map(|(_, ph)| ph.clone()).collect();
    for (_, ph) in RESIDUAL_PLACEHOLDERS {
        placeholders.insert(ph.to_string());
    }
    for ph in [PH_REPO, PH_HOME, PH_PRIV_IP, PH_HOST, PH_ABS_PATH, PH_REDACTED] {
        placeholders.insert(ph.to_string());
    }
    for (literal, (prio, _)) in &best {
        if let Some(clash) = placeholders.iter().find(|ph| ph.contains(literal.as_str())) {
            return core::fail(
                "HINT_PII_SUBST_COLLISION",
                format!(
                    "{} 리터럴(길이 {})이 치환 표지 {clash} 안에 들어 있다 — 치환이 멱등이 아니게 되고 배포 스캔이 치환된 본문을 다시 잡는다",
                    prio_name(*prio),
                    clen(literal)
                ),
                Some(&format!("`{}` 의 그 리터럴을 더 구체적으로 적거나 manifest 값을 확인한다.", core::REL_PII_TERMS)),
            );
        }
    }
    let mut rows: Vec<(String, String)> = best.into_iter().map(|(lit, (_, ph))| (lit, ph)).collect();
    rows.sort_by(|a, b| clen(&b.0).cmp(&clen(&a.0)).then_with(|| a.0.cmp(&b.0)));
    Ok(rows)
}

fn literal_regex_with(literal: &str, placeholder: Option<&str>, esc: fn(&str) -> String) -> String {
    let e = esc(literal);
    if placeholder.is_some_and(|p| p.starts_with("<nic:")) {
        return format!("(?<![A-Za-z0-9_]){e}(?![A-Za-z0-9_])");
    }
    if _is_ipv4(literal) {
        return format!(r"(?<![0-9.]){e}(?![0-9]|\.[0-9])");
    }
    if literal.starts_with('/') {
        return format!("{e}(?![A-Za-z0-9._-])");
    }
    e
}

/// 리터럴의 경계 규칙(모양으로 정한다) — 원본이 짓는 정규식 텍스트(`re.escape` 철자).
pub fn _literal_regex(literal: &str, placeholder: Option<&str>) -> String {
    literal_regex_with(literal, placeholder, py_re_escape)
}

fn table_shape_error() -> HintError {
    HintError::new(
        "HINT_PII_SUBST_TABLE_SHAPE",
        "치환표 행은 (비어 있지 않은 리터럴, 표지) 문자열 쌍이다",
        Some("substitution_table() 의 반환값을 그대로 넘긴다."),
    )
}

/// `_table_rows(table)` for a bridged table value (rows = lists/tuples of two str, non-empty literal).
pub fn _table_rows_j(table: &J) -> HR<Vec<(String, String)>> {
    let items: Vec<J> = match table {
        J::List(l) => l.clone(),
        J::Dict(m) if matches!(py_type(table), "tuple" | "set") => m.0[0].1.items().to_vec(),
        J::Dict(m) if py_type(table) == "bytes" => {
            if m.0[0].1.as_str().unwrap_or_default().is_empty() {
                Vec::new()
            } else {
                return Err(table_shape_error());
            }
        }
        J::Dict(m) => m.keys().map(J::from).collect(),
        J::Str(s) => s.chars().map(|c| J::Str(c.to_string())).collect(),
        other => return Err(HintError::py("TypeError", format!("'{}' object is not iterable", other.type_name()))),
    };
    let mut rows = Vec::new();
    for row in &items {
        let pair: Vec<J> = match row {
            J::List(l) => l.clone(),
            J::Dict(m) if py_type(row) == "tuple" => m.0[0].1.items().to_vec(),
            _ => return Err(table_shape_error()),
        };
        match pair.as_slice() {
            [J::Str(a), J::Str(b)] if !a.is_empty() => rows.push((a.clone(), b.clone())),
            _ => return Err(table_shape_error()),
        }
    }
    Ok(rows)
}

fn table_rows(table: &[(String, String)]) -> HR<Vec<(String, String)>> {
    if table.iter().any(|(a, _)| a.is_empty()) {
        return Err(table_shape_error());
    }
    Ok(table.to_vec())
}

/// `_compiled(rows)` → (원본 정규식 텍스트, 이 엔진의 정규식, 리터럴→표지(첫 등장 순서)).
pub fn _compiled(rows: &[(String, String)]) -> HR<(String, Re, Vec<(String, String)>)> {
    let mut repl: Vec<(String, String)> = Vec::new();
    for (lit, ph) in rows {
        match repl.iter().find(|(l, _)| l == lit) {
            Some((_, cur)) if cur != ph => {
                return core::fail("HINT_PII_SUBST_TABLE_SHAPE", format!("같은 리터럴(길이 {})에 표지가 둘이다", clen(lit)), None);
            }
            Some(_) => {}
            None => repl.push((lit.clone(), ph.clone())),
        }
    }
    let mut order: Vec<&(String, String)> = repl.iter().collect();
    order.sort_by(|a, b| clen(&b.0).cmp(&clen(&a.0)).then_with(|| a.0.cmp(&b.0)));
    let py_text: Vec<String> = order.iter().map(|(s, ph)| literal_regex_with(s, Some(ph), py_re_escape)).collect();
    let eng_text: Vec<String> = order.iter().map(|(s, ph)| literal_regex_with(s, Some(ph), engine_escape)).collect();
    let re = Re::try_new(&eng_text.join("|"), 0).map_err(|e| HintError::py("re.error", e))?;
    Ok((py_text.join("|"), re, repl))
}

/// 표가 놓친 잔여를 패턴 `name` 으로 가린다(절번호 예외는 스캐너와 같다).
pub fn _mask_residual(text: &str, name: &str, placeholder: &str) -> String {
    let Some(pat) = generic_pii(name) else { return text.to_string() };
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in pat.finditer(text) {
        let m0 = m.get(0).unwrap();
        if ANCHOR_EXCLUDED.contains(&name) && _is_section_anchored(text, m0.start()) {
            continue;
        }
        out.push_str(&text[last..m0.start()]);
        out.push_str(placeholder);
        last = m0.end();
    }
    out.push_str(&text[last..]);
    out
}

/// 발췌 기계 치환(결정론 · 멱등): ① 치환표 한 번 통과(긴 리터럴 우선 · 연쇄 치환 ✗) ② residual(기본 켬) — 4종 잔여 가림.
pub fn substitute(text: &str, table: &[(String, String)], residual: bool) -> HR<String> {
    let rows = table_rows(table)?;
    let mut text = text.to_string();
    if !rows.is_empty() {
        let (_, rx, repl) = _compiled(&rows)?;
        text = rx.sub_fn(&text, |c| {
            let m = c.get(0).unwrap().as_str();
            repl.iter().find(|(l, _)| l == m).map(|(_, p)| p.clone()).unwrap_or_else(|| m.to_string())
        });
    }
    if residual {
        for (name, ph) in RESIDUAL_PLACEHOLDERS {
            text = _mask_residual(&text, name, ph);
        }
    }
    Ok(text)
}

// ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
/// 픽스처 주소 조립기(추적 파일에 IPv4 모양 리터럴을 쓰지 않는다).
pub fn _ip(octets: &[i64]) -> String {
    octets.iter().map(i64::to_string).collect::<Vec<_>>().join(".")
}

extern "C" {
    fn mkfifo(path: *const std::os::raw::c_char, mode: u32) -> i32;
}

fn make_fifo(p: &Path) -> bool {
    let Ok(c) = std::ffi::CString::new(core::posix(p)) else { return false };
    // SAFETY: `c` is a valid NUL-terminated path for the duration of the call.
    unsafe { mkfifo(c.as_ptr(), 0o644) == 0 }
}

/// Rust sources of a module (`src/<m>.rs`, or every file under `src/<m>/`).
fn module_sources(m: &str) -> Vec<PathBuf> {
    let src = core::skill_dir().join("rust").join("src");
    let file = src.join(format!("{m}.rs"));
    if file.is_file() || !src.join(m).is_dir() {
        return vec![file];
    }
    let mut out = Vec::new();
    let mut stack = vec![src.join(m)];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

/// 실패 메시지 목록(빈 목록 = 통과). 라이브 용어 파일·manifest·태그에 의존하지 않는다(임시 디렉터리 픽스처).
/// 픽스처 값은 합성이고 패턴에 걸리는 문자열은 조각으로 조립한다(이 파일 자체가 배포 4종 자기스캔 대상이다).
pub fn selftest() -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    let mut ck = |name: &str, cond: bool| {
        if !cond {
            bad.push(format!("pii: {name}"));
        }
    };
    fn code_of<T>(r: HR<T>) -> Option<String> {
        r.err().map(|e| e.code)
    }
    let s = |x: &str| x.to_string();
    let sv = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
    let none: &[String] = &[];
    let st = |t: &str, terms: Option<&[String]>| scan_text(t, terms, "deploy", &[], false, true, None).unwrap_or_default();
    let names = |h: &[Hit]| h.iter().map(|x| x.pattern.clone()).collect::<Vec<_>>();

    // 2026-09-23 N1: pip 핀 버전(3옥텟) 오탐 · 실제 주소는 그대로 잡는다
    let v4 = |t: &str| st(t, Some(none)).into_iter().filter(|h| h.pattern == "private-ipv4").count();
    let ten = format!("{}{}", "1", "0.");
    let c168 = format!("{}{}", "19", "2.168.");
    ck("pip 핀 버전(==10.x.y) 은 private-ipv4 가 아니다", v4(&format!("setuptools-scm=={ten}2.3")) == 0);
    ck(
        "★음성대조 ==뒤 4옥텟 · host= 뒤 주소 · 3옥텟 대역 표기는 잡는다",
        [format!("a=={c168}1.1"), format!("host={ten}0.0.1"), format!("net {c168}100")].iter().all(|t| v4(t) > 0),
    );

    let terms = sv(&["forbidden-secret-token"]);
    let tm = Some(terms.as_slice());
    let genuine_ip = _ip(&[192, 168, 1, 5]);
    let ten_ip = _ip(&[10, 20, 30, 40]);
    let s172_ip = _ip(&[172, 20, 1, 2]);
    let sec = format!("{}{}", "10.", "1.2");
    let nas = format!("/{}", "mnt/fixture-nas/Model/x");
    let spark = format!("{}{}", "spark-", "0f0f");
    let email = format!("{}@{}", "bob", "example.com");

    // ── 정본 4종 · 프로필 구조
    let op = format!("/{}/{}", "mnt", "models");
    let cmp = format!("x:\n  - ${{PLE:-{op}}}:/app/p:ro\n  # 미선언이면 기본값 {op} 로 무해\n");
    ck(
        "주석이 같은 파일의 셸 기본값을 되풀이하면 abs-op-path 면제(shell_default_exempt)",
        scan_text(&cmp, Some(none), "deploy", &[], true, true, None).map(|h| h.is_empty()).unwrap_or(false),
    );
    ck(
        "★음성대조 기본값으로 실리지 않은 경로를 주석에만 적으면 잡는다",
        scan_text(&format!("# 데이터는 /{}/{} 에\n", "mnt", "secret"), Some(none), "deploy", &[], true, true, None)
            .map(|h| names(&h) == sv(&["abs-op-path"]))
            .unwrap_or(false),
    );
    ck(
        "★음성대조 shell_default_exempt 가 꺼지면 주석 되풀이도 잡는다",
        st(&cmp, Some(none)).iter().map(|h| h.line).collect::<Vec<_>>() == vec![2, 3],
    );
    ck("정본 패턴 4종(이름·순서)", GENERIC_PII().iter().map(|(n, _)| *n).collect::<Vec<_>>() == ["private-ipv4", "email", "abs-op-path", "spark-host"]);
    ck("deploy = 4종 전부", profile("deploy") == Some(&GENERIC_PII_NAMES[..]));
    ck(
        "nondeploy_prose = deploy − abs-op-path",
        profile("nondeploy_prose").map(<[&str]>::to_vec) == Some(GENERIC_PII_NAMES.iter().copied().filter(|n| *n != "abs-op-path").collect()),
    );
    ck(
        "identity = deploy − email",
        profile("identity").map(<[&str]>::to_vec) == Some(GENERIC_PII_NAMES.iter().copied().filter(|n| *n != "email").collect()),
    );

    // ── claim C2 픽스처 의미 보존
    ck("C2 용어 양성", !st("this text contains forbidden-secret-token here", tm).is_empty());
    ck("C2 RFC1918 양성", !st(&format!("{genuine_ip} is a private ip"), tm).is_empty());
    ck("C2 깨끗한 본문 음성", st("nothing sensitive here at all", tm).is_empty());
    ck("★C2 §-앵커 절번호는 사설 IPv4 가 아니다", st(&format!("본문 §{sec} 를 참조"), tm).is_empty());
    ck("★C2 제목-앵커 절번호는 사설 IPv4 가 아니다", st(&format!("#### {sec} 관측된 부작용"), tm).is_empty());
    let after = st(&format!("§{sec} 요약\n서브 노드 {genuine_ip} 도달"), tm);
    ck(
        "★C2 앵커 뒤(다음 줄) 진짜 IP 는 여전히 양성",
        after.iter().map(Hit::to_string).filter(|x| x.starts_with("private-ipv4:")).collect::<Vec<_>>() == vec![format!("private-ipv4:{genuine_ip}")],
    );
    let same_line = st(&format!("§{sec} 와 {genuine_ip}"), tm);
    ck("★앵커 뒤(같은 줄) 진짜 IP 양성 — 첫 매치에서 멈추지 않는다", names(&same_line) == sv(&["private-ipv4"]));
    ck("★윗줄의 홀로 선 § 는 아랫줄 진짜 IP 를 억제하지 못한다(줄 시작 절단)", names(&st(&format!("§\n{ten_ip}"), None)) == sv(&["private-ipv4"]));
    let skip_email = sv(&["email"]);
    let alice = sv(&["Alice"]);
    let ident = scan_text(&format!("Alice <{}@{}>", "alice", "example.com"), Some(&alice), "deploy", &skip_email, false, true, None).unwrap_or_default();
    ck("C2 신원 검사: email 제외해도 리터럴은 잡는다", ident.iter().any(|h| h.to_string().starts_with("term:")));
    ck(
        "C2 신원 검사: skip_generic email",
        !scan_text(&email, Some(none), "deploy", &skip_email, false, true, None).unwrap_or_default().iter().any(|h| h.pattern == "email"),
    );
    ck("identity 프로필은 email 을 보지 않는다", scan_text(&email, Some(none), "identity", &[], false, true, None).map(|h| h.is_empty()).unwrap_or(false));
    ck("★음성대조 deploy 프로필은 email 을 잡는다", names(&st(&email, Some(none))) == sv(&["email"]));
    let syn = format!("{} <{}>", core::SYNTHETIC_NAME, core::synthetic_email());
    ck("합성 신원은 identity 프로필 무검출", scan_text(&syn, tm, "identity", &[], false, true, None).map(|h| h.is_empty()).unwrap_or(false));
    ck("★합성 신원도 deploy 프로필 email 에는 걸린다(그래서 identity 프로필이 있다)", names(&st(&syn, tm)) == sv(&["email"]));

    // ── 옛 hint_tag 자체검사 이관
    ck("★PII 절대경로 검출", st(&format!("경로 {nas} 참조"), None).iter().any(|h| h.pattern == "abs-op-path"));
    ck("★PII 호스트명 검출(합성 값)", st(&format!("노드 {spark} 에서"), None).iter().any(|h| h.pattern == "spark-host"));
    ck("깨끗한 본문은 무검출", st("GB10 2노드 TP=2 · 53.92 t/s", None).is_empty());
    ck("버전 문자열은 IPv4 가 아니다", st("vllm 0.29.0rc6 · torch 2.11.0 · CUDA 13.2", None).is_empty());
    ck("172.15 는 사설 대역 밖", st(&_ip(&[172, 15, 0, 1]), None).is_empty());
    ck("★10.x · 172.16-31 사설 대역 양성", names(&st(&format!("{ten_ip} {s172_ip}"), None)) == sv(&["private-ipv4", "private-ipv4"]));

    // ── 전부 반환 · 줄 번호 · 결정론 순서
    let b_term = sv(&["b "]);
    let multi = st(&format!("a {nas}\nb {nas}2 {spark}\n"), Some(&b_term));
    ck(
        "검출은 전부 반환(패턴마다 1건에서 멈추지 않는다)",
        multi.iter().map(|h| (h.pattern.as_str(), h.line)).collect::<Vec<_>>()
            == vec![("abs-op-path", 1), ("term", 2), ("abs-op-path", 2), ("spark-host", 2)],
    );
    ck("str(Hit) = pattern:match", Hit::new("term", "x", 1).to_string() == "term:x");
    let long = format!("/{}{}", "mnt/", "y".repeat(80));
    ck(
        "render_hits 절단·경로 표기",
        render_located_hits(&[(s("a.md"), Hit::new("abs-op-path", &long, 3))], 20).unwrap_or_default()
            == format!("a.md:3: abs-op-path:{}", core::head_chars(&long, SHOW)),
    );

    // ── 비배포 산문 프로필(docs.md 3종)
    ck("nondeploy_prose 는 abs-op-path 를 보지 않는다", scan_text(&nas, None, "nondeploy_prose", &[], false, true, None).map(|h| h.is_empty()).unwrap_or(false));
    ck(
        "★nondeploy_prose 도 사설 IP·호스트명은 잡는다",
        scan_text(&format!("{genuine_ip} {spark}"), None, "nondeploy_prose", &[], false, true, None).map(|h| names(&h)).unwrap_or_default()
            == sv(&["private-ipv4", "spark-host"]),
    );

    // ── 셸 기본값 예외
    let compose_line = format!("- ${{NAS_MODEL_PATH:-/{}}}:/app/models:ro\n", "mnt/models");
    let mut log: Vec<J> = Vec::new();
    ck("★shell 기본값은 면제된다(플래그 켬)", scan_text(&compose_line, None, "deploy", &[], true, true, Some(&mut log)).map(|h| h.is_empty()).unwrap_or(false));
    ck(
        "면제는 조용히 넘기지 않는다(EXEMPT 기록)",
        log.len() == 1 && log[0].as_str().unwrap_or_default().starts_with("shell-default 1: abs-op-path:"),
    );
    ck("★플래그 끔(기본)이면 셸 기본값도 검출", names(&st(&compose_line, None)) == sv(&["abs-op-path"]));
    let mut sink: Vec<J> = Vec::new();
    let baked = format!("NAS_MODEL_PATH=/{}\n", "mnt/fixture-nas/Model/real");
    ck(
        "★음성대조 baking 된 실경로는 여전히 잡힌다",
        scan_text(&baked, None, "deploy", &[], true, true, Some(&mut sink)).map(|h| names(&h)).unwrap_or_default() == sv(&["abs-op-path"]),
    );
    let near = format!("설명: ${{VAR}} 뒤에 /{} 이 있다\n", "mnt/fixture-nas/Model/real");
    ck(
        "★음성대조 치환구문 근처라도 기본값이 아니면 잡힌다",
        scan_text(&near, None, "deploy", &[], true, true, Some(&mut sink)).map(|h| names(&h)).unwrap_or_default() == sv(&["abs-op-path"]),
    );

    // ── 엄격 모드(섹션앵커 예외 없음)
    ck(
        "★엄격 모드(section_anchor_exempt=False)는 절번호도 잡는다",
        scan_text(&format!("§{sec}"), None, "deploy", &[], false, false, None).map(|h| names(&h)).unwrap_or_default() == sv(&["private-ipv4"]),
    );

    // ── 입력 검증(소리내어 실패)
    ck("★모르는 프로필 차단", code_of(scan_text("x", None, "deploy_body", &[], false, true, None)).as_deref() == Some("HINT_PII_PROFILE_UNKNOWN"));
    ck("★모르는 skip 이름 차단", code_of(scan_text("x", None, "deploy", &sv(&["mail"]), false, true, None)).as_deref() == Some("HINT_PII_PATTERN_UNKNOWN"));
    // 원본 `scan_text(b"x", None)`: Rust 의 `&str` 인자는 바이트를 받지 못한다 — 다리(J 값)의 같은 가드를 잰다.
    ck("★바이트 입력 차단", code_of(_text_arg(&crate::jd! {"__bytes__" => "78"}, "scan_text")).as_deref() == Some("HINT_PII_INPUT_NOT_TEXT"));

    // ── 자기스캔: 이 모듈과 함께 배포되는 명명·카탈로그 소스와 어휘표에 4종 패턴 리터럴 0.
    //    원본은 자기 .py(pii·naming·catalog)를 읽었다 — 이 이식의 배포 소스는 Rust 모듈 파일이다.
    let mut own_files: Vec<PathBuf> = Vec::new();
    for m in ["pii", "naming", "catalog"] {
        own_files.extend(module_sources(m));
    }
    own_files.push(core::skill_dir().ancestors().nth(3).map(Path::to_path_buf).unwrap_or_default().join(core::REL_VOCAB));
    for f in &own_files {
        let fname = py_name(f);
        match std::fs::read(f).ok().and_then(|b| String::from_utf8(b).ok()) {
            Some(text) => {
                let hits = st(&text, None);
                ck(&format!("자기스캔 — {fname} 에 4종 패턴 리터럴 0({})", render_hits(&hits, 3).unwrap_or_default()), hits.is_empty());
            }
            None => ck(&format!("자기스캔 — {fname} 읽기"), false),
        }
    }
    // 원본의 "import 부수효과 0" 탐침(새 Python 프로세스에서 모듈을 적재해 프로세스 실행·파일 열기를 관측)은 Rust 에
    // 대응물이 없다 — 이 모듈은 적재 시 아무것도 하지 않는다(정규식은 첫 사용 때 `OnceLock` 으로 컴파일 · 정적 초기화 0).
    // Python 을 띄우는 탐침은 이 패키지의 불변식(Python 미실행)과 충돌하므로 옮기지 않는다.

    let td = core::TempDir::new("hintpii-");
    let base = td.path().to_path_buf();
    // ── 용어 파일
    let repo = base.join("repo");
    let _ = std::fs::create_dir_all(repo.join(".claude"));
    ck("용어 파일 부재 = None", matches!(load_pii_terms(&repo), Ok(None)));
    ck("★용어 파일 부재 = 배포면 차단(K11)", code_of(require_terms(&repo)).as_deref() == Some("HINT_PII_TERMS_ABSENT"));
    let _ = std::fs::write(repo.join(core::REL_PII_TERMS), "# 주석\n\n  forbidden-secret-token  \nOpFixtureName\n");
    let parsed = sv(&["forbidden-secret-token", "OpFixtureName"]);
    ck("용어 파일 파싱(주석·빈 줄·공백)", load_pii_terms(&repo).ok().flatten() == Some(parsed.clone()));
    ck("require_terms 는 목록", require_terms(&repo).ok() == Some(parsed.clone()));
    let broken = base.join("broken");
    let _ = std::fs::create_dir_all(broken.join(".claude"));
    let _ = std::fs::write(broken.join(core::REL_PII_TERMS), b"\xff\xfe\x00bad");
    ck("★깨진 용어 파일은 부재가 아니라 결함", code_of(load_pii_terms(&broken)).as_deref() == Some("HINT_PII_TERMS_UNREADABLE"));
    let empty = base.join("empty");
    let _ = std::fs::create_dir_all(empty.join(".claude"));
    let _ = std::fs::write(empty.join(core::REL_PII_TERMS), "# 리터럴 없음\n");
    ck("빈 용어 파일은 '있음'(선언)", require_terms(&empty).ok() == Some(Vec::new()));

    // ── 트리 스캔
    let pay = base.join("payload");
    let _ = std::fs::create_dir_all(pay.join("artifacts").join("compose"));
    let _ = std::fs::write(pay.join("00-hint.md"), format!("# 지도\n§{sec} 참조\n경로 {nas}\n"));
    let _ = std::fs::write(pay.join("artifacts/compose/docker-compose.yaml"), &compose_line);
    let mut blob = b"\xff\xfe\x00".to_vec();
    blob.extend_from_slice(nas.as_bytes());
    let _ = std::fs::write(pay.join("artifacts/blob.bin"), blob);
    let _ = std::fs::write(pay.join("clean.md"), "정상 문서\n");
    let tree = |root: &Path, terms: Option<&[String]>, prof: &str, shell: bool, rels: Option<&[String]>, log: Option<&mut Vec<J>>| {
        scan_tree(root, terms, prof, &[], shell, true, rels, log)
    };
    let mut tlog: Vec<J> = Vec::new();
    let th = tree(&pay, tm, "deploy", true, None, Some(&mut tlog)).unwrap_or_default();
    ck(
        "트리 스캔: 상대경로·줄·패턴",
        th.iter().map(|(r, h)| (r.as_str(), h.pattern.as_str(), h.line)).collect::<Vec<_>>() == vec![("00-hint.md", "abs-op-path", 3)],
    );
    let tlog_s: Vec<String> = tlog.iter().filter_map(|e| e.as_str().map(str::to_string)).collect();
    ck("트리 스캔: 셸 기본값 면제(기본 켬) 기록", tlog_s.iter().any(|e| e.starts_with("shell-default artifacts/compose/docker-compose.yaml:1:")));
    ck("트리 스캔: 바이너리 건너뜀 기록", tlog_s.iter().any(|e| e.starts_with("binary artifacts/blob.bin")));
    let mut sink2: Vec<J> = Vec::new();
    ck(
        "★트리 스캔 셸 기본값 면제를 끄면 compose 도 검출",
        tree(&pay, None, "deploy", false, None, Some(&mut sink2))
            .unwrap_or_default()
            .iter()
            .any(|(r, h)| r == "artifacts/compose/docker-compose.yaml" && h.pattern == "abs-op-path"),
    );
    ck(
        "트리 스캔: 결정론(두 번 같음)",
        tree(&pay, None, "deploy", true, None, Some(&mut sink2)).ok() == tree(&pay, None, "deploy", true, None, Some(&mut sink2)).ok(),
    );
    let _ = std::os::unix::fs::symlink(format!("/{}/{}", "home", "op-fixture/secret"), pay.join("link"));
    ck(
        "★심링크는 링크 문자열(git 이 싣는 바이트)을 스캔한다",
        tree(&pay, None, "deploy", true, None, Some(&mut sink2)).unwrap_or_default().iter().any(|(r, h)| r == "link" && h.pattern == "abs-op-path"),
    );
    let _ = std::fs::remove_file(pay.join("link"));
    ck(
        "rels 한정 스캔",
        tree(&pay, None, "deploy", true, Some(&sv(&["clean.md", "00-hint.md"])), Some(&mut sink2))
            .unwrap_or_default()
            .iter()
            .map(|(r, _)| r.clone())
            .collect::<Vec<_>>()
            == sv(&["00-hint.md"]),
    );
    ck(
        "rels 디렉터리는 펼친다",
        tree(&pay, None, "deploy", true, Some(&sv(&["artifacts"])), Some(&mut sink2)).map(|h| h.is_empty()).unwrap_or(false),
    );
    ck("★rels 절대경로 거부", code_of(tree(&pay, None, "deploy", true, Some(&sv(&["/etc/x"])), None)).as_deref() == Some("HINT_PII_PATH_INVALID"));
    ck("★rels 상위탈출 거부", code_of(tree(&pay, None, "deploy", true, Some(&sv(&["../x"])), None)).as_deref() == Some("HINT_PII_PATH_INVALID"));
    ck("★rels 부재는 무검출이 아니라 차단", code_of(tree(&pay, None, "deploy", true, Some(&sv(&["nope.md"])), None)).as_deref() == Some("HINT_PII_PATH_ABSENT"));
    ck("★루트 부재 차단", code_of(tree(&base.join("none"), None, "deploy", true, None, None)).as_deref() == Some("HINT_PII_ROOT_ABSENT"));
    ck("nondeploy_prose 트리", tree(&pay, None, "nondeploy_prose", true, None, Some(&mut sink2)).map(|h| h.is_empty()).unwrap_or(false));
    let fifo_ok = make_fifo(&pay.join("fifo"));
    ck(
        "★특수 파일(FIFO)은 열지 않고 거부",
        fifo_ok && code_of(tree(&pay, None, "deploy", true, None, Some(&mut sink2))).as_deref() == Some("HINT_PII_UNSCANNABLE_FILE"),
    );
    let _ = std::fs::remove_file(pay.join("fifo"));

    // ── 치환표 · 치환
    let (main_ip, sub_ip) = (_ip(&[10, 250, 0, 1]), _ip(&[10, 250, 0, 2]));
    let lan_ip = _ip(&[192, 168, 7, 9]);
    let other_ip = _ip(&[172, 16, 9, 9]);
    let sub_host = format!("{}{}", "spark-", "0e0e");
    let home = format!("/{}/{}", "home", "op-fixture");
    let sub_work = format!("{home}/easy_vllm_sub");
    let nas_root = format!("/{}", "mnt/fixture-nas/Models");
    let tik = format!("/{}", "mnt/fixture-nas/tiktoken/");
    let repo_s = core::posix(&core::resolve(&repo));
    let mut manifest = crate::jd! {
        "topology" => "multi",
        "nas_model_path" => nas_root.clone(),
        "tiktoken_host_path" => tik.clone(),
        "nodes" => vec![
            crate::jd! {"role" => "main", "host" => main_ip.clone(), "hostname" => spark.clone(), "ssh_user" => "op-fixture", "work_dir" => repo_s.clone()},
            crate::jd! {"role" => "sub", "host" => sub_ip.clone(), "hostname" => sub_host.clone(), "ssh_user" => "op-fixture", "work_dir" => sub_work.clone()},
        ],
        "propagation_authorization" => crate::jd! {"ssh_host" => sub_ip.clone(), "work_dir" => sub_work.clone(), "misc_ip" => other_ip.clone()},
        "interconnect" => crate::jd! {"socket_iface" => "enp1s0f0np0", "gid_index" => 3, "type" => "RoCE v2",
                                      "hca_devices" => vec!["rocep1s0f0", "roceP2p1s0f0"], "nccl_socket_ifname" => "^lo,enp9s0"},
        "self_role" => "main",
        "serving_ip" => "0.0.0.0",
    };
    if let Some(J::List(ns)) = manifest.get_mut("nodes") {
        ns[1].set("socket_iface", "enp7s0f1np1"); // 노드별 NIC 필드 = 그 노드 라벨
    }
    let table = substitution_table(&repo, &manifest).unwrap_or_default();
    let tmap: BTreeMap<String, String> = table.iter().cloned().collect();
    let tget = |k: &str| tmap.get(k).map(String::as_str);
    ck("치환표 결정론(두 번 같음)", substitution_table(&repo, &manifest).ok() == Some(table.clone()));
    let lens: Vec<usize> = table.iter().map(|(l, _)| clen(l)).collect();
    let mut lens_sorted = lens.clone();
    lens_sorted.sort_by(|a, b| b.cmp(a));
    ck("치환표 정렬 = 긴 리터럴 우선", lens == lens_sorted);
    ck(
        "노드 IP·호스트명 → <node:role>",
        tget(&main_ip) == Some("<node:main>") && tget(&sub_ip) == Some("<node:sub>") && tget(&spark) == Some("<node:main>") && tget(&sub_host) == Some("<node:sub>"),
    );
    ck("repo 가 노드 work_dir 보다 우선", tget(&repo_s) == Some(PH_REPO));
    ck("같은 리터럴은 사전순 앞 표지(nodes[sub] < propagation_authorization)", tget(&sub_work) == Some("<manifest.nodes[sub].work_dir>"));
    ck(
        "manifest 경로 필드(끝 / 제거)",
        tget(&nas_root) == Some("<manifest.nas_model_path>") && tget(tik.trim_end_matches('/')) == Some("<manifest.tiktoken_host_path>"),
    );
    ck("ssh_user 홈 → <home>", tget(&home) == Some(PH_HOME));
    ck("nodes 밖 사설 IP → <priv-ip>", tget(&other_ip) == Some(PH_PRIV_IP));
    ck("pii_terms → <redacted>", tget("OpFixtureName") == Some(PH_REDACTED));
    ck("★루프백·비경로 값·ssh_user 맨몸은 행이 되지 않는다", tget("0.0.0.0").is_none() && tget("op-fixture").is_none());
    ck(
        "★NIC 최상위(interconnect · 클러스터 공통) → <nic:cluster>(self_role=main 이어도 <nic:main> ✗)",
        tget("enp1s0f0np0") == Some("<nic:cluster>")
            && tget("rocep1s0f0") == Some("<nic:cluster>")
            && tget("roceP2p1s0f0") == Some("<nic:cluster>")
            && !tmap.values().any(|v| v == "<nic:main>"),
    );
    ck(
        "NIC NCCL 목록 문법(^lo,enp9s0) → 장치만 · `lo` 는 숫자가 없어 행이 아니다",
        tget("enp9s0") == Some("<nic:cluster>") && tget("lo").is_none() && tget("^lo").is_none(),
    );
    ck("NIC nodes[sub] 에만 있는 이름 → <nic:sub>", tget("enp7s0f1np1") == Some("<nic:sub>"));
    let mut both = manifest.clone();
    if let Some(J::List(ns)) = both.get_mut("nodes") {
        ns[0].set("socket_iface", "enp7s0f1np1");
        ns[0].set("rdma_hca", "mlx5x9");
    }
    let tb: BTreeMap<String, String> = substitution_table(&repo, &both).unwrap_or_default().into_iter().collect();
    ck(
        "★두 노드가 같은 이름을 가지면 <nic:cluster>(사전순으로 <nic:main> 이 이기지 않는다) · 한 노드만 = 그 노드",
        tb.get("enp7s0f1np1").map(String::as_str) == Some("<nic:cluster>") && tb.get("mlx5x9").map(String::as_str) == Some("<nic:main>"),
    );
    ck("★NIC 키의 낱말 값(`RoCE v2` · gid_index 숫자)은 행이 아니다", tget("RoCE v2").is_none() && tget("RoCE").is_none() && tget("3").is_none());
    let nic_doc = "NET/IB : Using [0]rocep1s0f0:1/RoCE [1]roceP2p1s0f0:1/RoCE ; OOB enp1s0f0np0:x · enp1s0f0np01 · xenp1s0f0np0";
    let nic_out = substitute(nic_doc, &table, true).unwrap_or_default();
    ck(
        &format!("NIC 치환: 로그 줄의 장치 이름 → 자리표시({nic_out})"),
        nic_out.starts_with("NET/IB : Using [0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE ; OOB <nic:cluster>:x"),
    );
    ck("★NIC 경계: 닮은 이름(뒤·앞에 식별자 문자가 붙은 것)은 깎지 않는다", nic_out.ends_with("· enp1s0f0np01 · xenp1s0f0np0"));
    let mut no_self = manifest.clone();
    if let J::Dict(m) = &mut no_self {
        m.remove("self_role");
    }
    ck("self_role 유무와 무관(최상위 = 공통 설정) — 치환표가 같다", substitution_table(&repo, &no_self).ok() == Some(table.clone()));
    ck("★NIC 는 스캐너 체계 밖(치환 추가일 뿐 · P3) — 장치 이름 본문은 deploy 스캔 무검출", st("OOB enp1s0f0np0 rocep1s0f0", None).is_empty());
    ck("_home_of 는 저장소 경로에서 파생", _home_of(&format!("{home}/ws/repo")) == Some(home.clone()) && _home_of("/srv/x/y").is_none());

    let doc = [
        format!("메인 {main_ip} 서브 {sub_ip} 호스트 {spark}"),
        format!("모델 {nas_root}/qwen3 · tiktoken {tik}o200k"),
        format!("작업 {repo_s}/docs/testlog/x.md"),
        format!("옆 경로 {nas_root}2/other"),
        format!("LAN {lan_ip} · 닮은 주소 {main_ip}2"),
        format!("§{sec} 참조"),
        format!("작성 OpFixtureName <op@{}>", "fixture.example"),
        format!("홈 {home}/other · 서브 {sub_work}/logs"),
        format!("- ${{NAS_MODEL_PATH:-/{}}}", "mnt/models"),
    ]
    .join("\n")
        + "\n";
    let out = substitute(&doc, &table, true).unwrap_or_default();
    ck("치환: 노드", out.contains("메인 <node:main> 서브 <node:sub> 호스트 <node:main>"));
    ck("치환: manifest 경로 필드", out.contains("<manifest.nas_model_path>/qwen3") && out.contains("<manifest.tiktoken_host_path>/o200k"));
    ck("치환: repo 가 home 보다 긴 리터럴로 이긴다", out.contains("<repo>/docs/testlog/x.md"));
    ck("★경로 경계: 닮은 경로는 필드로 오귀속되지 않는다(잔여 가림)", out.contains("옆 경로 <abs-path>"));
    ck("★IP 경계: 닮은 주소는 노드로 오귀속되지 않는다(잔여 가림)", out.contains("LAN <priv-ip> · 닮은 주소 <priv-ip>"));
    ck("★절번호는 치환하지 않는다(스캐너와 같은 예외)", out.contains(&format!("§{sec} 참조")));
    ck("치환: 사람 식별자·email 제거", out.contains("작성 <redacted> <<redacted>>"));
    ck("치환: home·서브 work_dir", out.contains("홈 <home>/other · 서브 <manifest.nodes[sub].work_dir>/logs"));
    ck("치환: 셸 기본값 잔여도 가린다", out.contains("${NAS_MODEL_PATH:-<abs-path>}"));
    let req = require_terms(&repo).unwrap_or_default();
    ck("★치환 결과는 deploy 스캔(용어 포함) 무검출", st(&out, Some(&req)).is_empty());
    ck(
        "★치환 결과는 엄격 스캔에서도 절번호만 남는다",
        scan_text(&out, None, "deploy", &[], false, false, None).unwrap_or_default().iter().map(Hit::to_string).collect::<Vec<_>>()
            == vec![format!("private-ipv4:{sec}")],
    );
    ck("치환 멱등", substitute(&out, &table, true).ok().as_deref() == Some(out.as_str()));
    ck("치환 결정론", substitute(&doc, &table, true).ok().as_deref() == Some(out.as_str()));
    ck("★residual 끄면 표 밖 잔여가 남는다(잔여 단계가 실제로 일한다)", substitute(&doc, &table, false).unwrap_or_default().contains(&lan_ip));
    let allowed = [PH_REPO, PH_HOME, PH_REDACTED];
    ck(
        "manifest 없는 치환표 = repo·home·terms",
        substitution_table(&repo, &J::Null).map(|t| t.iter().all(|(_, ph)| allowed.contains(&ph.as_str()))).unwrap_or(false),
    );
    ck(
        "★치환표 행 모양 검사",
        code_of(substitute("x", &[(s(""), s("<a>"))], true)).as_deref() == Some("HINT_PII_SUBST_TABLE_SHAPE")
            && code_of(_table_rows_j(&crate::jl![crate::jd! {"a" => 1}])).as_deref() == Some("HINT_PII_SUBST_TABLE_SHAPE"),
    );
    ck(
        "★같은 리터럴에 표지 둘 차단",
        code_of(substitute("x", &[(s("a"), s("<p>")), (s("a"), s("<q>"))], true)).as_deref() == Some("HINT_PII_SUBST_TABLE_SHAPE"),
    );
    ck(
        "★리터럴 목록 자리의 문자열 하나 차단",
        code_of(_scan_text("x", &_terms_from_j(&J::from("abc")), "deploy", &[], false, true, None)).as_deref() == Some("HINT_PII_TERMS_SHAPE"),
    );
    ck(
        "★manifest 모양 검사",
        code_of(substitution_table(&repo, &crate::jd! {"nodes" => "x"})).as_deref() == Some("HINT_PII_MANIFEST_SHAPE"),
    );

    // 같은 role 둘 → 서수 라벨
    let third = _ip(&[10, 250, 0, 3]);
    let dup = crate::jd! {"nodes" => vec![
        crate::jd! {"role" => "main", "host" => main_ip.clone()},
        crate::jd! {"role" => "sub", "host" => sub_ip.clone()},
        crate::jd! {"role" => "sub", "host" => third.clone()},
    ]};
    let dmap: BTreeMap<String, String> = substitution_table(&repo, &dup).unwrap_or_default().into_iter().collect();
    ck(
        "같은 role 둘 → sub1·sub2",
        dmap.get(&sub_ip).map(String::as_str) == Some("<node:sub1>") && dmap.get(&third).map(String::as_str) == Some("<node:sub2>"),
    );

    // ★용어가 표지 안에 들어가면 차단(멱등 붕괴 · 치환된 본문이 다시 걸림)
    let coll = base.join("coll");
    let _ = std::fs::create_dir_all(coll.join(".claude"));
    let _ = std::fs::write(coll.join(core::REL_PII_TERMS), "node\n");
    let r = substitution_table(&coll, &manifest);
    ck("★용어-표지 충돌 차단", code_of(r.clone()).as_deref() == Some("HINT_PII_SUBST_COLLISION"));
    if let Err(e) = r {
        ck("충돌 메시지는 출처 범주·길이로 말한다(리터럴 인용 ✗)", e.message.contains("pii_terms 리터럴(길이 4)"));
    }
    drop(td);
    bad
}
