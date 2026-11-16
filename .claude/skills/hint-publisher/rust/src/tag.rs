//! tag — port of `scripts/hintlib/tag.py`: hint 태그 봉인 · 이 태그 하나의 로컬 검증 · 정확한 refspec 1개 push.
//!
//! annotation(brief 1문단 + zip 포인터 + 증거 footer v2)을 짓고, 안내 커밋 위의 페이로드 커밋에 annotated 태그로 봉인하고,
//! push 전에 그 태그 하나만 로컬에서 검증하고, `refs/tags/<tag>:refs/tags/<tag>` 하나만 원격에 민다. 불변식의 서사는 원본
//! 모듈 docstring 이 정본이다(footer v2 · 봉인 뒤 재현성 · 앵커 · verbatim · 판정은 code 로만 · push 범위 · 자격증명).
//!
//! Rust 표현:
//! - `HintProblemsError` = `HintError` 에 `problems: Some(목록)` (호출부는 [`problem_codes`] 로 code 를 구조로 받는다).
//! - `HintEvidenceBindingError` = `HintError`(code `HINT_EVIDENCE_BINDING_*` · remedy [`EVIDENCE_BINDING_REMEDY`]) —
//!   [`is_evidence_binding_error`] 로 가린다.
//! - `subprocess.CompletedProcess` 결과 = `J` `{"args", "returncode", "stdout", "stderr"}`.
//! - `read_tag()["raw"]` = `{"__bytes__": <hex>}`([`tag_raw_bytes`] 로 바이트를 되찾는다).
#![allow(non_snake_case, non_upper_case_globals, dead_code)]
use std::cell::RefCell;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::core::{self, HintError, ProcBytes, HR};
use crate::value::{py_repr, Map, J};
use crate::{branch, jd, naming, pii};

// ── footer ───────────────────────────────────────────────────────────────────────────────────
pub const FOOTER_MARKER_PREFIX: &str = "<!-- hint-evidence-binding:";
pub const FOOTER_MARKER_OPEN_V1: &str = "<!-- hint-evidence-binding:v1";
pub const FOOTER_MARKER_OPEN: &str = "<!-- hint-evidence-binding:v2 · 주소는 발행 저장소 로컬 경로(수신자 해소 대상 아님)";
pub const FOOTER_MARKER_CLOSE: &str = "-->";
pub const FOOTER_VERSION: &str = "2";
pub const FOOTER_FIELDS_V1: [&str; 6] = ["version", "tag", "topology", "anchor", "manifest_ref", "certificate_ref"];
pub const FOOTER_FIELDS: [&str; 7] = ["version", "tag", "topology", "anchor", "manifest_ref", "bench_ref", "bench_kind"];
pub const BENCH_KINDS: [&str; 2] = ["certificate", "bench_report"];
/// 판 → (여는 줄, 필드, 상대경로여야 하는 주소 필드).
pub fn _FOOTER_SPECS(ver: &str) -> Option<(&'static str, &'static [&'static str], &'static [&'static str])> {
    match ver {
        "1" => Some((FOOTER_MARKER_OPEN_V1, &FOOTER_FIELDS_V1, &["manifest_ref", "certificate_ref"])),
        "2" => Some((FOOTER_MARKER_OPEN, &FOOTER_FIELDS, &["manifest_ref", "bench_ref"])),
        _ => None,
    }
}
const FOOTER_VERSIONS: [&str; 2] = ["1", "2"];
pub const RETIRED_FOOTER_KEYS: [&str; 3] = ["certificate_sha256", "identity_sha256", "manifest_sha256"];
crate::re_static!(_FOOTER_LINE_RE, r"^([a-z0-9_]+): (.*)$");
crate::re_static!(_SHA40_RE, r"^[0-9a-f]{40}$");
crate::re_static!(_BENCH_CERT_RE, r"^benchmark_[^/]+\.yaml$");
crate::re_static!(_BENCH_REPORT_RE, r"^bench_report_[^/]+\.md$");

// ── annotation ───────────────────────────────────────────────────────────────────────────────
pub const ANNOTATION_POINTER: &str = "전체 지도·서사·재현 키트는 이 태그의 zip(archive) 안에 있다 — `00-hint.md` 부터 읽는다.";
pub const ANNOTATION_BRIEF_MAX_LINES: i64 = 3;

// ── push 자격증명 ──────────────────────────────────────────────────────────────────────────────
pub const TOKEN_KEY: &str = "GITHUB_TOKEN";
pub const TOKEN_ENV_FILE_REL: &str = "envs/.env";
pub const PUSH_TOKEN_ENV: &str = "HINT_PUSH_TOKEN";
pub const _GLOB_CHARS: [char; 3] = ['*', '?', '['];
crate::re_static!(_SCHEME_RE, r"^([A-Za-z][A-Za-z0-9+.-]*)://");
/// hintlib/tag.py:128 _SCHEME_TRANSPORT (python literal as JSON)
pub fn _SCHEME_TRANSPORT() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"https\": \"https\", \"http\": \"http\", \"ssh\": \"ssh\", \"git+ssh\": \"ssh\", \"ssh+git\": \"ssh\", \"file\": \"local\", \"git\": \"git\"}").expect("constant"))
}

// ── Python 문자열 도우미 ─────────────────────────────────────────────────────────────────────────
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}
fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(py_isspace)
}
fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}') {
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
fn lst(v: &[&str]) -> String {
    J::from(v.to_vec()).repr()
}
fn lst_s(v: &[String]) -> String {
    J::from(v.to_vec()).repr()
}
/// `bytes.decode("utf-8", "replace")` + text-mode universal newlines (`subprocess.run(text=True)`).
fn text_mode(b: &[u8]) -> String {
    String::from_utf8_lossy(b).replace("\r\n", "\n").replace('\r', "\n")
}

pub fn _log(msg: &str) -> HR<()> {
    crate::io::errln(&format!("[hint] {msg}"));
    Ok(())
}
fn log(msg: &str) {
    let _ = _log(msg);
}

pub fn _problem(code: &str, msg: &str) -> HR<String> {
    Ok(prob(code, msg))
}
fn prob(code: &str, msg: &str) -> String {
    format!("{code}: {msg}")
}

/// verify_local 결과 → code 목록. **판정은 이것으로만 한다**(메시지 substring ✗).
pub fn problem_codes(problems: &[String]) -> HR<Vec<String>> {
    let mut v: Vec<String> = problems.iter().map(|p| p.split(':').next().unwrap_or("").to_string()).collect();
    v.sort();
    v.dedup();
    Ok(v)
}

/// `HintProblemsError(code, "<what> <n>건:\n  …", remedy, problems)`.
pub fn _fail_problems<T>(code: &str, what: &str, problems: &[String], remedy: &str) -> HR<T> {
    let mut e = HintError::new(code, format!("{what} {}건:\n  {}", problems.len(), problems.join("\n  ")), Some(remedy));
    e.problems = Some(problems.to_vec());
    Err(e)
}

/// `HintEvidenceBindingError` 의 고정 remedy.
pub const EVIDENCE_BINDING_REMEDY: &str = "annotation 은 손으로 쓰지 않는다 — tag.annotation(brief, fields) 로 짓는다.";

fn binding_err<T>(code: &str, msg: impl Into<String>) -> HR<T> {
    Err(HintError::new(code, msg, Some(EVIDENCE_BINDING_REMEDY)))
}

/// 이 HintError 가 원본의 `HintEvidenceBindingError` 인가.
pub fn is_evidence_binding_error(e: &HintError) -> bool {
    !e.is_py() && e.code.starts_with("HINT_EVIDENCE_BINDING_") && e.remedy.as_deref() == Some(EVIDENCE_BINDING_REMEDY)
}

pub fn _malformed<T>(msg: &str) -> HR<T> {
    binding_err("HINT_EVIDENCE_BINDING_MALFORMED", msg)
}

/// bench_ref 파일 이름에서 **파생**되는 bench_kind. 판정할 수 없는 이름 = None.
pub fn bench_kind_of(ref_: &J) -> HR<Option<String>> {
    let Some(s) = ref_.as_str() else { return Ok(None) };
    let st = py_strip(s);
    if st.is_empty() {
        return Ok(None);
    }
    let norm = st.replace('\\', "/");
    let name = norm.rsplit('/').next().unwrap_or("");
    if _BENCH_CERT_RE().is_match(name) {
        return Ok(Some("certificate".into()));
    }
    if _BENCH_REPORT_RE().is_match(name) {
        return Ok(Some("bench_report".into()));
    }
    Ok(None)
}

/// 필드 정합(빌드·파싱 공용). 판은 `version` 값이 고른다(v1·v2).
pub fn _checked_fields(fields: &J) -> HR<J> {
    let Some(m) = fields.as_dict() else {
        return _malformed(&format!("footer 필드는 사전이어야 한다: {}", fields.type_name()));
    };
    let ver_raw = m.get("version").cloned().unwrap_or(J::Null);
    let ver = match &ver_raw {
        J::Str(s) => J::Str(py_strip(s).to_string()),
        o => o.clone(),
    };
    if matches!(ver, J::List(_) | J::Dict(_)) {
        return Err(HintError::py("TypeError", format!("unhashable type: '{}'", ver.type_name())));
    }
    let spec = ver.as_str().and_then(_FOOTER_SPECS);
    let Some((_open, names, rel_fields)) = spec else {
        return _malformed(&format!("지원하지 않는 footer version: {}", ver.repr()));
    };
    let ver = ver.as_str().unwrap().to_string();
    let missing: Vec<&str> = names.iter().copied().filter(|k| !m.contains_key(k)).collect();
    let mut extra: Vec<String> = m.keys().filter(|k| !names.contains(k)).map(str::to_string).collect();
    extra.sort();
    extra.dedup();
    if !missing.is_empty() || !extra.is_empty() {
        return _malformed(&format!("footer v{ver} 필드 불일치 — 누락 {} · 초과 {}", lst(&missing), lst_s(&extra)));
    }
    let mut out = Map::new();
    for k in names.iter() {
        let v = m.get(k).unwrap();
        let Some(s) = v.as_str() else {
            return _malformed(&format!("footer 필드 {} 는 문자열이어야 한다: {}", py_repr(k), v.type_name()));
        };
        let s = py_strip(s);
        if ["\n", "\r", "-->", "<!--"].iter().any(|b| s.contains(b)) {
            return _malformed(&format!("footer 필드 {} 에 줄바꿈·주석 경계 문자열이 있다", py_repr(k)));
        }
        out.insert(*k, s);
    }
    let get = |k: &str| out.get(k).and_then(J::as_str).unwrap_or("").to_string();
    if !_SHA40_RE().is_full(&get("anchor")) {
        return _malformed(&format!("anchor 는 40자 커밋 SHA 여야 한다: {}", py_repr(&get("anchor"))));
    }
    for k in names.iter() {
        if *k != "version" && *k != "anchor" && get(k).is_empty() {
            return _malformed(&format!("footer 필드 {} 가 비었다", py_repr(k)));
        }
    }
    for k in rel_fields.iter() {
        if get(k).starts_with('/') {
            return _malformed(&format!("footer 필드 {} 는 상대경로여야 한다", py_repr(k)));
        }
    }
    if ver == "2" {
        let kind = get("bench_kind");
        if !BENCH_KINDS.contains(&kind.as_str()) {
            return _malformed(&format!("bench_kind 는 {} 중 하나다: {}", lst(&BENCH_KINDS), py_repr(&kind)));
        }
        if let Some(derived) = bench_kind_of(&J::Str(get("bench_ref")))? {
            if derived != kind {
                return _malformed(&format!(
                    "bench_kind={} 가 bench_ref 이름에서 파생되는 {} 와 다르다 — REFUTE·lite 셀의 리포트를 인증서라 부르던 v1 의 거짓을 되살리지 않는다(plan_26092908 §4.4 · V2).",
                    py_repr(&kind),
                    py_repr(&derived)
                ));
            }
        }
    }
    Ok(J::Dict(out))
}

/// footer 블록(끝 개행 포함). `fields` = `**fields` 키워드 사전.
pub fn build_footer(fields: &J) -> HR<String> {
    let clean = _checked_fields(fields)?;
    let (opener, names, _rel) = _FOOTER_SPECS(clean.get_str("version").unwrap_or("")).unwrap();
    let mut parts = vec![opener.to_string()];
    for k in names {
        parts.push(format!("{k}: {}", clean.get_str(k).unwrap_or("")));
    }
    parts.push(FOOTER_MARKER_CLOSE.to_string());
    Ok(parts.join("\n") + "\n")
}

/// footer 를 **정확히 하나** 엄격 파싱(v1·v2). MISSING = 여는 표지 부재 · 그 밖의 모든 결함 = MALFORMED.
pub fn parse_footer(text: &str, retired_ok: &[String]) -> HR<J> {
    let opens = text.matches(FOOTER_MARKER_PREFIX).count();
    if opens == 0 {
        return binding_err("HINT_EVIDENCE_BINDING_MISSING", "hint-evidence-binding footer 가 없다");
    }
    if opens != 1 {
        return _malformed(&format!("footer 가 둘 이상이다(여는 표지 {opens}개)"));
    }
    let i = text.find(FOOTER_MARKER_PREFIX).unwrap();
    if i > 0 && text.as_bytes()[i - 1] != b'\n' {
        return _malformed("footer 여는 표지가 줄 머리에 있지 않다");
    }
    let Some(nl_rel) = text[i..].find('\n') else {
        return _malformed("footer 여는 표지는 있으나 닫는 `-->` 줄이 없다");
    };
    let nl = i + nl_rel;
    let opener = text[i..nl].trim_end_matches('\r');
    let ver = FOOTER_VERSIONS.iter().copied().find(|v| _FOOTER_SPECS(v).map(|s| s.0) == Some(opener));
    let Some(ver) = ver else {
        return _malformed(&format!("알 수 없는 footer 여는 줄(판·문구): {}", py_repr(&core::head_chars(opener, 100))));
    };
    let lines: Vec<&str> = text[nl + 1..].split('\n').collect();
    let Some(close) = lines.iter().position(|ln| ln.trim_end_matches('\r') == FOOTER_MARKER_CLOSE) else {
        return _malformed("footer 여는 표지는 있으나 닫는 `-->` 줄이 없다");
    };
    let names = _FOOTER_SPECS(ver).unwrap().1;
    let mut fields = Map::new();
    for raw in &lines[..close] {
        let line = raw.trim_end_matches('\r');
        if py_strip(line).is_empty() {
            continue;
        }
        let Some(m) = _FOOTER_LINE_RE().match_at_start(line) else {
            return _malformed(&format!("해석할 수 없는 footer 줄: {}", py_repr(&core::head_chars(line, 80))));
        };
        let key = m.get(1).map(|x| x.as_str()).unwrap_or("").to_string();
        let value = py_strip(m.get(2).map(|x| x.as_str()).unwrap_or("")).to_string();
        if !names.contains(&key.as_str()) {
            if RETIRED_FOOTER_KEYS.contains(&key.as_str()) && retired_ok.iter().any(|r| *r == key) {
                continue;
            }
            return _malformed(&format!("알 수 없는 footer 키(v{ver}): {}", py_repr(&key)));
        }
        if fields.contains_key(&key) {
            return _malformed(&format!("중복 footer 키: {}", py_repr(&key)));
        }
        fields.insert(key, value);
    }
    let missing: Vec<&str> = names.iter().copied().filter(|k| !fields.contains_key(k)).collect();
    if !missing.is_empty() {
        return _malformed(&format!("footer 필수 필드 누락: {}", lst(&missing)));
    }
    let fv = fields.get("version").and_then(J::as_str).unwrap_or("").to_string();
    if fv != ver {
        return _malformed(&format!("footer 여는 줄은 v{ver} 인데 본문 version={} 이다", py_repr(&fv)));
    }
    _checked_fields(&J::Dict(fields))
}

/// 판 무관 읽기 → {"ref", "kind", "kind_source"}.
pub fn bench_binding(footer: &J) -> HR<J> {
    if !footer.is_dict() {
        return _malformed(&format!("footer 사전이 아니다: {}", footer.type_name()));
    }
    if footer.get("version") == Some(&J::Str("2".into())) {
        return Ok(jd! {
            "ref" => footer.get("bench_ref").cloned().unwrap_or(J::Null),
            "kind" => footer.get("bench_kind").cloned().unwrap_or(J::Null),
            "kind_source" => "declared",
        });
    }
    let r = footer.get("certificate_ref").cloned().unwrap_or(J::Null);
    let kind = bench_kind_of(&r)?;
    let src = if kind.as_deref().map_or(false, |k| !k.is_empty()) { "derived-from-name" } else { "underivable" };
    Ok(jd! {"ref" => r, "kind" => J::from(kind), "kind_source" => src})
}

// ── annotation ───────────────────────────────────────────────────────────────────────────────
/// brief 정규형: 앞뒤 공백 제거 · 줄 끝 공백 제거 · 한 문단 · ≤3줄 · 주석 경계·포인터 문구 ✗.
pub fn _canonical_brief(brief: &J) -> HR<String> {
    let s = match brief.as_str() {
        Some(s) if !py_strip(s).is_empty() => s,
        _ => {
            return core::fail(
                "HINT_ANNOTATION_BRIEF_ABSENT",
                "annotation brief 가 비었다(00-hint.md §0.1 첫 문단).",
                Some("template.brief(payload_dir) 로 기계 추출한다."),
            )
        }
    };
    let lines: Vec<&str> = py_strip(s).split('\n').map(py_rstrip).collect();
    if lines.iter().any(|ln| py_strip(ln).is_empty()) {
        return core::fail("HINT_ANNOTATION_BRIEF_SHAPE", "brief 는 한 문단이다(빈 줄 ✗) — 카탈로그는 첫 문단만 읽는다.", None);
    }
    if lines.len() as i64 > ANNOTATION_BRIEF_MAX_LINES {
        return core::fail(
            "HINT_ANNOTATION_BRIEF_TOO_LONG",
            format!("brief 는 최대 {ANNOTATION_BRIEF_MAX_LINES}줄이다({}줄).", lines.len()),
            None,
        );
    }
    let text = lines.join("\n");
    if text.contains("<!--") || text.contains("-->") || text.contains('\r') || text.contains(ANNOTATION_POINTER) {
        return core::fail("HINT_ANNOTATION_BRIEF_SHAPE", "brief 에 주석 경계·CR·포인터 문구가 있다.", None);
    }
    Ok(text)
}

/// 태그 메시지 정본: `<brief>\n\n<포인터>\n\n<footer>`.
pub fn annotation(brief: &str, fields: &J) -> HR<String> {
    annotation_j(&J::Str(brief.to_string()), fields)
}

/// [`annotation`] with an arbitrary brief value (the bridge passes non-strings too).
pub fn annotation_j(brief: &J, fields: &J) -> HR<String> {
    let b = _canonical_brief(brief)?;
    Ok(format!("{b}\n\n{ANNOTATION_POINTER}\n\n{}", build_footer(fields)?))
}

/// annotation → {"brief", "footer"}.
pub fn parse_annotation(body: &str) -> HR<J> {
    let footer = parse_footer(body, &[])?;
    let sep = format!("\n\n{ANNOTATION_POINTER}\n\n");
    let Some(i) = body.find(&sep) else {
        return core::fail("HINT_ANNOTATION_SHAPE", "zip 포인터 문단이 없다 — 수신자는 zip 을 열 이유를 모른다.", None);
    };
    let brief = &body[..i];
    let canonical = match annotation(brief, &footer) {
        Ok(c) => c,
        Err(e) if !e.is_py() => {
            return core::fail("HINT_ANNOTATION_SHAPE", format!("brief 가 정본 규칙을 어긴다({}: {})", e.code, e.message), None)
        }
        Err(e) => return Err(e),
    };
    if canonical != body {
        return core::fail(
            "HINT_ANNOTATION_SHAPE",
            "annotation 이 정본 모양(brief · 포인터 · footer)과 바이트가 다르다 — 손 편집·서명·덧붙임은 봉인 산출물이 아니다.",
            None,
        );
    }
    Ok(jd! {"brief" => _canonical_brief(&J::Str(brief.to_string()))?, "footer" => footer})
}

// ── 프로세스 ───────────────────────────────────────────────────────────────────────────────────
thread_local! {
    /// 자체검사 전용 git 서브프로세스 기록(원본 `_spy_git`).
    static SPY: RefCell<Option<Vec<J>>> = const { RefCell::new(None) };
}

fn spy_log(argv: &[String], env: &J) {
    SPY.with(|s| {
        if let Some(v) = s.borrow_mut().as_mut() {
            v.push(jd! {"argv" => argv.to_vec(), "env" => env.clone()});
        }
    });
}

/// `dict(os.environ)`.
fn environ() -> J {
    let mut m = Map::new();
    for (k, v) in std::env::vars_os() {
        m.insert(k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned());
    }
    J::Dict(m)
}

/// `subprocess.run(argv, cwd=repo, env=env, capture_output=True[, input=…])` with an explicit whole environment.
fn run_env(argv: &[String], cwd: &Path, env: &J, input: Option<&[u8]>) -> Result<ProcBytes, String> {
    spy_log(argv, env);
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).current_dir(cwd).env_clear();
    if let Some(m) = env.as_dict() {
        for (k, v) in m.iter() {
            cmd.env(k, v.as_str().unwrap_or(""));
        }
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().map_err(|e| core::os_error_text(&e, &argv[0]))?;
    let h = input.map(|data| {
        let mut si = child.stdin.take().unwrap();
        let data = data.to_vec();
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = si.write_all(&data);
        })
    });
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    if let Some(h) = h {
        let _ = h.join();
    }
    Ok(ProcBytes { returncode: o.status.code().unwrap_or(-1), stdout: o.stdout, stderr: o.stderr })
}

fn proc_j(args: &[String], rc: i32, stdout: &str, stderr: &str) -> J {
    jd! {"args" => args.to_vec(), "returncode" => rc as i64, "stdout" => stdout, "stderr" => stderr}
}

/// `repr(subprocess.CompletedProcess)` of a [`proc_j`] value.
pub fn completed_process_repr(p: &J) -> String {
    format!(
        "CompletedProcess(args={}, returncode={}, stdout={}, stderr={})",
        p["args"].repr(),
        p["returncode"].repr(),
        p["stdout"].repr(),
        p["stderr"].repr()
    )
}

// ── 태그 오브젝트 읽기 ─────────────────────────────────────────────────────────────────────────
pub fn _tag_ref(tag: &str) -> HR<String> {
    Ok(tref(tag))
}
fn tref(tag: &str) -> String {
    format!("refs/tags/{tag}")
}

/// 바이트 입출력 git(결과를 strip 하지 않는다). `env_extra` 는 `os.environ` 위에 덮는다.
pub fn _git_b(repo: &Path, args: &[&str], input_bytes: Option<&[u8]>, env_extra: &[(String, String)]) -> HR<ProcBytes> {
    let mut env = environ();
    env.set("GIT_TERMINAL_PROMPT", "0");
    for (k, v) in env_extra {
        env.set(k.clone(), v.clone());
    }
    let mut argv = vec!["git".to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    match run_env(&argv, repo, &env, input_bytes) {
        Ok(p) => Ok(p),
        Err(e) => core::fail("HINT_GIT_UNAVAILABLE", format!("git 실행 불가: {e}"), None),
    }
}

pub fn _ref_sha(repo: &Path, ref_: &str) -> HR<Option<String>> {
    let r = core::git_nocheck(repo, &["rev-parse", "--verify", "--quiet", ref_])?;
    let sha = py_strip(&r.stdout).to_string();
    Ok(if r.returncode == 0 && !sha.is_empty() { Some(sha) } else { None })
}

/// refs/tags/<tag> 가 직접 가리키는 오브젝트 종류(`tag` · `commit` · None = 부재).
pub fn tag_ref_kind(repo: &Path, tag: &str) -> HR<Option<String>> {
    let Some(sha) = _ref_sha(repo, &tref(tag))? else { return Ok(None) };
    let t = py_strip(&core::git_nocheck(repo, &["cat-file", "-t", &sha])?.stdout).to_string();
    Ok(if t.is_empty() { None } else { Some(t) })
}

fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).filter_map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()).collect()
}

/// `read_tag()["raw"]` → bytes.
pub fn tag_raw_bytes(t: &J) -> Vec<u8> {
    unhex(t["raw"].get_str("__bytes__").unwrap_or(""))
}

/// annotated 태그 → {object_sha, target, target_type, name, tagger, body, raw}. 부재·lightweight = None.
pub fn read_tag(repo: &Path, tag: &str) -> HR<Option<J>> {
    let sha = _ref_sha(repo, &tref(tag))?;
    let Some(sha) = sha else { return Ok(None) };
    if py_strip(&core::git_nocheck(repo, &["cat-file", "-t", &sha])?.stdout) != "tag" {
        return Ok(None);
    }
    let raw = core::git_bytes(repo, &["cat-file", "tag", &sha], true)?.unwrap_or_default();
    let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") else {
        return core::fail("HINT_TAG_OBJECT_MALFORMED", format!("태그 오브젝트에 헤더/본문 구분이 없다: {tag}"), None);
    };
    let (head, body) = (&raw[..pos], &raw[pos + 2..]);
    let (head_text, body_text) = match (std::str::from_utf8(head), std::str::from_utf8(body)) {
        (Ok(h), Ok(b)) => (h.to_string(), b.to_string()),
        _ => return core::fail("HINT_TAG_OBJECT_MALFORMED", format!("태그 오브젝트가 UTF-8 이 아니다: {tag}"), None),
    };
    let mut headers = Map::new();
    for line in head_text.split('\n') {
        let (k, v) = line.split_once(' ').unwrap_or((line, ""));
        if !headers.contains_key(k) {
            headers.insert(k, v);
        }
    }
    let target = headers.get("object").and_then(J::as_str).unwrap_or("").to_string();
    if !_SHA40_RE().is_full(&target) {
        return core::fail("HINT_TAG_OBJECT_MALFORMED", format!("태그 대상이 40자 SHA 가 아니다: {}", py_repr(&target)), None);
    }
    let tagger = branch::parse_ident_headers(&head_text)?.get("tagger").cloned().unwrap_or(J::Null);
    Ok(Some(jd! {
        "object_sha" => sha,
        "target" => target,
        "target_type" => headers.get("type").cloned().unwrap_or(J::Null),
        "name" => headers.get("tag").cloned().unwrap_or(J::Null),
        "tagger" => tagger,
        "body" => body_text,
        "raw" => jd! {"__bytes__" => hexs(&raw)},
    }))
}

// ── 이름 · ref 형식 ───────────────────────────────────────────────────────────────────────────
/// push·봉인 대상 태그 이름의 **ref 수준** 검사.
pub fn check_ref_format(repo: &Path, tag: &str) -> HR<()> {
    if tag.is_empty() {
        return core::fail("HINT_TAG_NAME_ABSENT", "태그 이름이 비었다.", None);
    }
    if tag.starts_with("refs/") || !tag.starts_with(core::HINT_TAG_PREFIX) {
        return core::fail(
            "HINT_TAG_NAMESPACE",
            format!("hint/ 밖 이름은 다루지 않는다(브랜치·다른 태그 ✗): {}", py_repr(tag)),
            Some("태그 이름은 `hint/<vllm>/<model>/<arch>/<recipe>` 이다(refs/ 접두 없이)."),
        );
    }
    if tag.chars().any(|c| _GLOB_CHARS.contains(&c)) {
        return core::fail(
            "HINT_TAG_GLOB_FORBIDDEN",
            format!("glob 문자가 있는 태그 이름: {}", py_repr(tag)),
            Some("정확한 태그 1개만 다룬다(glob 선택과 git glob 은 `?`·`[…]` 에서 갈린다)."),
        );
    }
    if tag.contains(':') || tag.chars().any(|c| py_isspace(c) || (c as u32) < 0x20) {
        return core::fail("HINT_TAG_REF_FORMAT", format!("refspec 구분자·공백·제어문자가 있는 태그 이름: {}", py_repr(tag)), None);
    }
    if core::git_nocheck(repo, &["check-ref-format", &tref(tag)])?.returncode != 0 {
        return core::fail("HINT_TAG_REF_FORMAT", format!("git check-ref-format 이 거부한다: {}", py_repr(tag)), None);
    }
    Ok(())
}

pub fn _local_hint_refs(repo: &Path) -> HR<Vec<String>> {
    let out = core::git_nocheck(repo, &["for-each-ref", "--format=%(refname)", "refs/tags/hint"])?;
    Ok(py_splitlines(&out.stdout).into_iter().map(py_strip).filter(|s| !s.is_empty()).map(str::to_string).collect())
}

/// 디렉터리/파일 충돌(`a/b` 와 `a/b/c` 는 공존할 수 없다).
pub fn _prefix_conflict(ref_: &str, refs: &[String]) -> HR<Option<String>> {
    for r in refs {
        if r != ref_ && (r.starts_with(&format!("{ref_}/")) || ref_.starts_with(&format!("{r}/"))) {
            return Ok(Some(r.clone()));
        }
    }
    Ok(None)
}

/// 이름 충돌: `local` · `local-prefix` · `remote` · `remote-prefix` · None.
pub fn name_collision(repo: &Path, tag: &str, remote: Option<&str>) -> HR<Option<String>> {
    check_ref_format(repo, tag)?;
    let r = tref(tag);
    if _ref_sha(repo, &r)?.is_some() {
        return Ok(Some("local".into()));
    }
    if _prefix_conflict(&r, &_local_hint_refs(repo)?)?.is_some() {
        return Ok(Some("local-prefix".into()));
    }
    if let Some(remote) = remote {
        let refs = _ls_remote_refs(repo, remote, "refs/tags/hint/*")?;
        let keys: Vec<String> = refs.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default();
        if keys.contains(&r) {
            return Ok(Some("remote".into()));
        }
        if _prefix_conflict(&r, &keys)?.is_some() {
            return Ok(Some("remote-prefix".into()));
        }
    }
    Ok(None)
}

// ── 봉인 ─────────────────────────────────────────────────────────────────────────────────────
/// `git tag -a` 가 이 입력으로 써야 하는 오브젝트 바이트(기대값).
pub fn _tag_object_bytes(anchor: &str, tag: &str, utc: &str, message: &str) -> HR<Vec<u8>> {
    let epoch = core::parse_utc(utc)?.epoch();
    let head = format!("object {anchor}\ntype commit\ntag {tag}\ntagger {} {epoch} +0000\n\n", branch::synthetic_identity()?);
    let mut out = head.into_bytes();
    out.extend_from_slice(message.as_bytes());
    Ok(out)
}

pub fn _hash_tag_object(repo: &Path, data: &[u8]) -> HR<String> {
    let r = _git_b(repo, &["hash-object", "-t", "tag", "--stdin"], Some(data), &[])?;
    let sha = py_strip(&String::from_utf8_lossy(&r.stdout)).to_string();
    if r.returncode != 0 || !_SHA40_RE().is_full(&sha) {
        return core::fail(
            "HINT_TAG_OBJECT_MALFORMED",
            format!("기대 태그 오브젝트를 해시할 수 없다: {}", py_strip(&String::from_utf8_lossy(&r.stderr))),
            None,
        );
    }
    Ok(sha)
}

/// annotation brief == 페이로드 트리 00-hint.md §0.1 첫 문단의 기계 추출(`template.brief`).
pub fn _brief_problems(repo: &Path, anchor: &str, brief: &str) -> HR<Vec<String>> {
    // = branch.read_blob(repo, anchor, "00-hint.md") (같은 배관 명령)
    let Some(data) = core::git_bytes(repo, &["cat-file", "blob", &format!("{anchor}:00-hint.md")], false)? else {
        return Ok(vec![]);
    };
    let td = core::TempDir::new("hint-brief-");
    let extracted = (|| -> HR<String> {
        let p = td.path().join("00-hint.md");
        std::fs::write(&p, &data).map_err(|e| core::io_err(&e, &p))?;
        _canonical_brief(&J::Str(crate::template::brief(td.path())?))
    })();
    let extracted = match extracted {
        Ok(x) => x,
        Err(e) if !e.is_py() => {
            return Ok(vec![prob(&e.code, &format!("00-hint.md §0.1 에서 brief 를 추출할 수 없다 — {}", e.message))])
        }
        Err(e) => {
            let ty = e.code.trim_start_matches("py:").to_string();
            if ty == "UnicodeDecodeError" || ty.ends_with("Error") && is_oserror(&ty) {
                return Ok(vec![prob("HINT_BRIEF_ABSENT", &format!("00-hint.md 를 읽을 수 없다({ty})"))]);
            }
            return Err(e);
        }
    };
    if extracted != brief {
        return Ok(vec![prob(
            "HINT_ANNOTATION_BRIEF_MISMATCH",
            "annotation brief 가 페이로드 00-hint.md §0.1 첫 문단의 추출과 다르다 — brief 는 손으로 쓰지 않는다.",
        )]);
    }
    Ok(vec![])
}

fn is_oserror(ty: &str) -> bool {
    matches!(
        ty,
        "OSError" | "FileNotFoundError" | "PermissionError" | "IsADirectoryError" | "NotADirectoryError" | "FileExistsError"
    )
}

/// PAYLOAD.naming 에 naming facts 가 있으면 어휘표로 **다시 파생**해 태그의 기본 이름과 대조한다.
pub fn _facts_problems(repo: &Path, anchor: &str, tag: &str) -> HR<Vec<String>> {
    let (doc, _err) = branch::read_json_blob(repo, anchor, branch::PAYLOAD_JSON)?;
    let doc = doc.unwrap_or(J::Null);
    let facts = doc.get("naming").filter(|n| n.is_dict()).and_then(|n| n.get("facts")).cloned().unwrap_or(J::Null);
    if !facts.is_dict() {
        return Ok(vec![]);
    }
    let base = match naming::base_name(tag) {
        Ok(b) => b,
        Err(e) if !e.is_py() => {
            if e.code == "HINT_NAME_GRAMMAR_OLD" {
                return Ok(vec![]);
            }
            return Ok(vec![prob(&e.code, &e.message)]);
        }
        Err(e) => return Err(e),
    };
    let derived = match naming::load_vocab(repo).and_then(|v| naming::derive_name(&facts, &v)) {
        Ok(d) => d,
        Err(e) if !e.is_py() => return Ok(vec![prob(&e.code, &format!("PAYLOAD.naming.facts 재파생 실패 — {}", e.message))]),
        Err(e) => return Err(e),
    };
    if derived.tag != base {
        return Ok(vec![prob(
            "HINT_DERIVED_NAME_MISMATCH",
            &format!("naming facts 재파생 {} ≠ 태그의 기본 이름 {}(태그 {})", py_repr(&derived.tag), py_repr(&base), py_repr(tag)),
        )]);
    }
    Ok(vec![])
}

/// 봉인 전·후 공용: 페이로드 커밋 계약 + brief 원천 + annotation PII + 신원 tripwire + facts + 스냅샷 모양.
pub fn _anchor_problems(repo: &Path, tag: &str, anchor: &str, brief: Option<&str>, body: &str, terms: &[String]) -> HR<Vec<String>> {
    let mut probs = branch::commit_violations(repo, anchor, tag, terms)?;
    if let Some(b) = brief {
        probs.extend(_brief_problems(repo, anchor, b)?);
    }
    let hits = pii::scan_text(body, Some(terms), "deploy", &[], false, true, None)?;
    if !hits.is_empty() {
        probs.push(prob("HINT_TAG_PII", &format!("annotation PII(배포 4종+리터럴): {}", pii::render_hits(&hits, 5)?)));
    }
    if !branch::identity_pii_hits(terms, None)?.is_empty() {
        probs.push(prob("HINT_TAGGER_IDENTITY_PII", "합성 tagger 신원이 PII 목록에 걸린다(tripwire)"));
    }
    probs.extend(_facts_problems(repo, anchor, tag)?);
    let (_snap, snap_probs) = sealed_sources(repo, anchor)?;
    probs.extend(snap_probs);
    Ok(probs)
}

// ── 봉인 출처 스냅샷 ──────────────────────────────────────────────────────────────────────────
pub const SEALED_SOURCES_KEY: &str = "sealed_sources";
pub const SEALED_SOURCES_FILE: &str = "LINEAGE.json";
/// 출처의 현재 바이트에 의존하는 린트 코드 — 닫힌 목록(tripwire).
pub const SOURCE_DEPENDENT_LINT_CODES: [&str; 11] = [
    "HINT_EXCERPT_MISMATCH",
    "HINT_EXCERPT_SOURCE_UNREADABLE",
    "HINT_EXCERPT_LINES_OUT_OF_RANGE",
    "HINT_EXCERPT_OUTSIDE_LINES",
    "HINT_EXCERPT_SNAPSHOT_DRIFT",
    "HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE",
    "HINT_SIGNATURE_MISMATCH",
    "HINT_SIGNATURE_UNVERIFIABLE",
    "HINT_BENCH_SECTION_DRIFT",
    "HINT_BENCH_SECTION_UNVERIFIABLE",
    "HINT_LINEAGE_DOC_UNREADABLE",
];
pub const INFO_SOURCE_CHANGED: &str = "INFO: 출처 변경됨(봉인 뒤)";
crate::re_static!(_SHA256_RE, r"^[0-9a-f]{64}$");

/// 페이로드 `LINEAGE.json` 의 `sealed_sources` → (목록 | None, 모양 결함 문제 목록).
pub fn sealed_sources(repo: &Path, anchor: &str) -> HR<(Option<Vec<J>>, Vec<String>)> {
    let (doc, _err) = branch::read_json_blob(repo, anchor, SEALED_SOURCES_FILE)?;
    let Some(doc) = doc else { return Ok((None, vec![])) };
    let Some(raw) = doc.get(SEALED_SOURCES_KEY) else { return Ok((None, vec![])) };
    let Some(items) = raw.as_list() else {
        return Ok((None, vec![prob("HINT_SEALED_SOURCES_SHAPE", &format!("{SEALED_SOURCES_FILE}.{SEALED_SOURCES_KEY} 가 목록이 아니다"))]));
    };
    let mut bad: Vec<String> = Vec::new();
    let mut out: Vec<J> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (i, e) in items.iter().enumerate() {
        let path = if e.is_dict() { e.get("path").cloned().unwrap_or(J::Null) } else { J::Null };
        let sha = if e.is_dict() { e.get("sha256").cloned().unwrap_or(J::Null) } else { J::Null };
        let path_ok = match path.as_str() {
            Some(p) => !p.is_empty() && !p.starts_with('/') && !p.contains('\\') && !p.split('/').any(|s| s.is_empty() || s == "." || s == ".."),
            None => false,
        };
        if !path_ok {
            bad.push(format!("[{i}] path={}", path.repr()));
        } else if !sha.as_str().map_or(false, |s| _SHA256_RE().is_full(s)) {
            bad.push(format!("[{i}] sha256 모양({})", path.as_str().unwrap()));
        } else if seen.iter().any(|s| s == path.as_str().unwrap()) {
            bad.push(format!("[{i}] 중복 path {}", path.as_str().unwrap()));
        } else {
            seen.push(path.as_str().unwrap().to_string());
            out.push(jd! {"path" => path.clone(), "sha256" => sha.clone()});
        }
    }
    if !bad.is_empty() {
        let head: Vec<String> = bad.iter().take(5).cloned().collect();
        return Ok((
            None,
            vec![prob(
                "HINT_SEALED_SOURCES_SHAPE",
                &format!("{SEALED_SOURCES_FILE}.{SEALED_SOURCES_KEY} 항목 결함 {}건: {}", bad.len(), lst_s(&head)),
            )],
        ));
    }
    Ok((Some(out), vec![]))
}

/// 스냅샷 중 **지금** 바이트가 다른 출처(삭제·읽기 불가 포함)의 path 목록.
pub fn changed_sealed_sources(repo: &Path, snapshot: &[J]) -> HR<Vec<String>> {
    let mut out = Vec::new();
    for e in snapshot {
        let path = e.get_str("path").unwrap_or("").to_string();
        let f = repo.join(&path);
        let cur = if f.is_file() { std::fs::read(&f).ok().map(|b| core::sha256_bytes(&b)) } else { None };
        if cur.as_deref() != e.get_str("sha256") {
            out.push(path);
        }
    }
    Ok(out)
}

/// 이 호출이 방금 만든 태그 하나를 CAS 로 지운다.
pub fn _rollback_created_tag(repo: &Path, tag: &str, sha: &str) -> HR<()> {
    core::git_nocheck(repo, &["update-ref", "-d", &tref(tag), sha])?;
    Ok(())
}

/// annotated 태그 봉인 → 태그 오브젝트 SHA. 모든 거부는 태그 생성 전(부작용 0)에 난다.
pub fn seal(repo: &Path, tag: &str, anchor: &str, message: &str, generated_utc: &str, remote: Option<&str>) -> HR<String> {
    check_ref_format(repo, tag)?;
    naming::validate_new_name(tag)?;
    let utc = core::require_utc(Some(generated_utc), "--generated-utc")?;
    if !_SHA40_RE().is_full(anchor) {
        return core::fail("HINT_ANCHOR_SHAPE", format!("앵커는 40자 커밋 SHA 여야 한다: {}", py_repr(anchor)), None);
    }
    let parsed = parse_annotation(message)?;
    let footer = &parsed["footer"];
    let fs = |k: &str| footer.get_str(k).unwrap_or("").to_string();
    if fs("version") != FOOTER_VERSION {
        return core::fail(
            "HINT_EVIDENCE_BINDING_VERSION",
            format!("신규 봉인의 footer 는 v{FOOTER_VERSION} 이어야 한다(받은 것 v{}).", fs("version")),
            Some("fields 를 tag.FOOTER_FIELDS(bench_ref · bench_kind = tag.bench_kind_of(bench_ref))로 짓는다."),
        );
    }
    if fs("tag") != tag {
        return core::fail("HINT_EVIDENCE_BINDING_TAG_MISMATCH", format!("footer.tag={} ≠ 봉인 대상 {}", py_repr(&fs("tag")), py_repr(tag)), None);
    }
    if fs("anchor") != anchor {
        return core::fail(
            "HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH",
            format!(
                "footer.anchor={} ≠ 봉인 앵커 {} — footer 의 anchor 는 이 태그가 가리키는 **페이로드 커밋**이다(K3).",
                core::head_chars(&fs("anchor"), 12),
                core::head_chars(anchor, 12)
            ),
            None,
        );
    }
    let ident = branch::synthetic_identity()?;
    if let Some(cur) = _ref_sha(repo, &tref(tag))? {
        let old = read_tag(repo, tag)?;
        let tagger_ident = |o: &J| o["tagger"].get_str("ident").map(str::to_string);
        if let Some(o) = &old {
            if o.get_str("target") == Some(anchor) && o.get_str("body") == Some(message) && tagger_ident(o).as_deref() == Some(ident.as_str()) {
                let same_time = cur == _hash_tag_object(repo, &_tag_object_bytes(anchor, tag, &utc, message)?)?;
                log(&format!(
                    "{tag} 는 이미 이 페이로드·annotation 으로 봉인돼 있다({}{} — 재개(새 오브젝트 없음).",
                    core::head_chars(&cur, 12),
                    if same_time { ")" } else { " · tagger 시각만 다르다 — 기존 봉인 유지)" }
                ));
                return Ok(cur);
            }
        }
        let why = match &old {
            None => "lightweight 태그".to_string(),
            Some(o) => {
                let parts: Vec<&str> = [
                    ("대상 커밋", o.get_str("target") != Some(anchor)),
                    ("본문", o.get_str("body") != Some(message)),
                    ("tagger", tagger_ident(o).as_deref() != Some(ident.as_str())),
                ]
                .iter()
                .filter(|x| x.1)
                .map(|x| x.0)
                .collect();
                if parts.is_empty() { "오브젝트".to_string() } else { parts.join(" · ") }
            }
        };
        return core::fail(
            "HINT_NAME_COLLISION",
            format!("로컬에 같은 이름의 다른 태그가 있다({tag} · 다른 것: {why}) — 태그는 불변이다."),
            Some("이 draft 가 봉인한 것이 아니다. 같은 셀의 개정판은 새 이름으로 발행한다(P1 리콜 금지 · 덮어쓰기 ✗)."),
        );
    }
    if let Some(remote) = remote {
        if let Some(where_) = name_collision(repo, tag, Some(remote))? {
            return core::fail(
                "HINT_NAME_COLLISION",
                format!("봉인 직전 원격 재조회: {tag} 가 원격 {} 에 이미 있다({where_}) — 봉인하지 않았다.", redact(remote)),
                Some("봉인은 개명하지 않는다(이름은 페이로드 안에 있다). hint.py continue 가 이름 확정 단계에서 `-t<YYMMDDHHMM>` 를 붙여 새 페이로드로 다시 짓는다(원격 중복 시만 · §4.1 U7)."),
            );
        }
    }
    branch::require_payload_anchor(repo, tag, anchor)?;
    let terms = pii::require_terms(repo)?;
    let mut probs = _anchor_problems(repo, tag, anchor, parsed.get_str("brief"), message, &terms)?;
    probs.sort();
    probs.dedup();
    if !probs.is_empty() {
        return _fail_problems(
            "HINT_SEAL_REFUSED",
            "봉인 전 검사",
            &probs,
            "각 항목의 code 를 고친다(페이로드는 hint.py publish 스캐폴드 · 커밋은 hint.py continue).",
        );
    }
    let sha = _create_tag(repo, tag, anchor, message, &utc)?;
    log(&format!(
        "봉인 {tag} → {} (대상 {} · tagger {})",
        core::head_chars(&sha, 12),
        core::head_chars(anchor, 12),
        core::SYNTHETIC_NAME
    ));
    Ok(sha)
}

/// git 수준 생성 + 사후조건. 반환 = 태그 오브젝트 SHA.
pub fn _create_tag(repo: &Path, tag: &str, anchor: &str, message: &str, utc: &str) -> HR<String> {
    let expected = _tag_object_bytes(anchor, tag, utc, message)?;
    let want = _hash_tag_object(repo, &expected)?;
    let env = vec![
        ("GIT_COMMITTER_NAME".to_string(), core::SYNTHETIC_NAME.to_string()),
        ("GIT_COMMITTER_EMAIL".to_string(), core::synthetic_email()),
        ("GIT_COMMITTER_DATE".to_string(), core::git_date(utc)?),
    ];
    let r = _git_b(
        repo,
        &["-c", "tag.gpgSign=false", "-c", "tag.forceSignAnnotated=false", "tag", "-a", "--no-sign", "--cleanup=verbatim", "-F", "-", tag, anchor],
        Some(message.as_bytes()),
        &env,
    )?;
    if r.returncode != 0 {
        if _ref_sha(repo, &tref(tag))?.is_some() {
            return core::fail("HINT_NAME_COLLISION", format!("봉인 도중 같은 이름의 태그가 생겼다(경합): {tag}"), None);
        }
        return core::fail(
            "HINT_TAG_CREATE_FAILED",
            format!("git tag 실패(rc={}): {}", r.returncode, py_strip(&String::from_utf8_lossy(&r.stderr))),
            None,
        );
    }
    let got = _ref_sha(repo, &tref(tag))?;
    let raw = match &got {
        Some(g) => core::git_bytes(repo, &["cat-file", "tag", g], false)?,
        None => None,
    };
    if got.as_deref() != Some(want.as_str()) || raw.as_deref() != Some(expected.as_slice()) {
        if let Some(g) = &got {
            _rollback_created_tag(repo, tag, g)?;
        }
        let got_s = got.clone().unwrap_or_else(|| "None".into());
        return core::fail(
            "HINT_TAG_SEAL_POSTCONDITION",
            format!(
                "봉인된 오브젝트가 기대 바이트와 다르다({tag} · 기대 {} · 실제 {}) — 방금 만든 태그를 되돌렸다(부분 적용 ✗).",
                core::head_chars(&want, 12),
                core::head_chars(&got_s, 12)
            ),
            Some("git 설정(서명·cleanup·신원)이 끼어들었는지 확인한다."),
        );
    }
    Ok(want)
}

// ── 로컬 검증 ────────────────────────────────────────────────────────────────────────────────
/// 서사 린터 주입구: `lint_fn(repo, anchor, tag)` → 발견 목록(J list: dict{code,message} · `CODE: 설명` 문자열).
pub type LintFn<'a> = &'a dyn Fn(&Path, &str, &str) -> HR<J>;

pub fn _lint_findings(fn_: LintFn<'_>, repo: &Path, anchor: &str, tag: &str) -> HR<Vec<String>> {
    let found = match fn_(repo, anchor, tag) {
        Ok(f) => f,
        Err(e) if !e.is_py() => return Ok(vec![prob(&e.code, &e.message)]),
        Err(e) => return Err(e),
    };
    let items: Vec<J> = if found.truthy() { found.items().to_vec() } else { vec![] };
    let mut out = Vec::new();
    for f in &items {
        let (code, msg) = match f {
            J::Dict(_) => (f.get("code").cloned().unwrap_or(J::Null), f.get("message").cloned().unwrap_or(J::Str(String::new()))),
            J::Str(s) => {
                let (c, m) = s.split_once(':').unwrap_or((s.as_str(), ""));
                (J::Str(c.to_string()), J::Str(m.to_string()))
            }
            _ => (J::Null, J::Str(String::new())),
        };
        let code_s = if code.truthy() { code.display() } else { "HINT_LINT_FINDING".to_string() };
        out.push(prob(&code_s, py_strip(&msg.display())));
    }
    Ok(out)
}

/// push 전 로컬 봉인 검증 — **이 태그 하나만**. 반환 = `CODE: 설명` 정렬 목록(빈 = 통과). 읽기만 한다.
pub fn verify_local(repo: &Path, tag: &str, lint_fn: Option<LintFn<'_>>, mut infos: Option<&mut Vec<J>>, source_repo: Option<&Path>) -> HR<Vec<String>> {
    if let Err(e) = check_ref_format(repo, tag) {
        if e.is_py() {
            return Err(e);
        }
        return Ok(vec![prob(&e.code, &e.message)]);
    }
    let kind = tag_ref_kind(repo, tag)?;
    let Some(kind) = kind else { return Ok(vec![prob("HINT_TAG_ABSENT", &format!("로컬에 태그가 없다: {tag}"))]) };
    if kind != "tag" {
        return Ok(vec![prob(
            "HINT_TAG_NOT_ANNOTATED",
            &format!("annotated 태그가 아니다({kind}) — 본문(footer)이 없는 태그는 봉인 산출물이 아니다."),
        )]);
    }
    let obj = match read_tag(repo, tag) {
        Ok(o) => o,
        Err(e) if !e.is_py() => return Ok(vec![prob(&e.code, &e.message)]),
        Err(e) => return Err(e),
    };
    let Some(obj) = obj else { return Ok(vec![prob("HINT_TAG_NOT_ANNOTATED", &format!("태그 오브젝트를 읽을 수 없다: {tag}"))]) };
    let mut probs: Vec<String> = Vec::new();
    macro_rules! catch {
        ($e:expr) => {
            match $e {
                Ok(v) => Some(v),
                Err(e) if !e.is_py() => {
                    probs.push(prob(&e.code, &e.message));
                    None
                }
                Err(e) => return Err(e),
            }
        };
    }
    catch!(naming::validate_new_name(tag));
    if obj.get_str("name") != Some(tag) {
        probs.push(prob(
            "HINT_TAG_OBJECT_NAME_MISMATCH",
            &format!("태그 오브젝트의 이름 {} ≠ ref 이름 {}(다른 태그 오브젝트를 옮겨 단 ref)", obj["name"].repr(), py_repr(tag)),
        ));
    }
    let terms: Vec<String> = match catch!(pii::load_pii_terms(repo)) {
        Some(Some(t)) => t,
        Some(None) => {
            probs.push(prob(
                "HINT_PII_TERMS_ABSENT",
                &format!("`{}` 부재 — 리터럴 없이 PII-clean 을 인증하지 않는다(4종 패턴은 그대로 검사했다).", core::REL_PII_TERMS),
            ));
            vec![]
        }
        None => vec![],
    };
    let tagger = obj["tagger"].clone();
    let ident = branch::synthetic_identity()?;
    if !tagger.truthy() || tagger.get_str("ident") != Some(ident.as_str()) {
        probs.push(prob(
            "HINT_TAGGER_NOT_SYNTHETIC",
            &format!(
                "tagger 가 합성 신원이 아니다({}) — 운영자 신원이 배포된다(2026-09-01 실명 유출 선례).",
                if !tagger.truthy() { "부재" } else { "다른 신원" }
            ),
        ));
    }
    if tagger.truthy() && !branch::identity_pii_hits(&terms, tagger.get_str("ident"))?.is_empty() {
        probs.push(prob("HINT_TAGGER_IDENTITY_PII", "tagger 신원이 PII 목록에 걸린다"));
    }
    let body = obj.get_str("body").unwrap_or("").to_string();
    let mut brief: Option<String> = None;
    let mut footer: Option<J> = None;
    match parse_annotation(&body) {
        Ok(p) => {
            brief = p.get_str("brief").map(str::to_string);
            footer = Some(p["footer"].clone());
        }
        Err(e) if !e.is_py() => {
            probs.push(prob(&e.code, &e.message));
            if e.code == "HINT_ANNOTATION_SHAPE" {
                match parse_footer(&body, &[]) {
                    Ok(f) => footer = Some(f),
                    Err(e2) if !e2.is_py() => {}
                    Err(e2) => return Err(e2),
                }
            }
        }
        Err(e) => return Err(e),
    }
    let target = obj.get_str("target").unwrap_or("").to_string();
    if let Some(f) = &footer {
        if f.get_str("tag") != Some(tag) {
            probs.push(prob("HINT_EVIDENCE_BINDING_TAG_MISMATCH", "footer.tag ≠ 이 태그"));
        }
        if f.get_str("anchor") != Some(target.as_str()) {
            probs.push(prob(
                "HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH",
                "footer.anchor ≠ 태그가 가리키는 페이로드 커밋(ⓑ축 — 옛날엔 한 번도 검사되지 않았다 · K3)",
            ));
        }
    }
    if obj.get_str("target_type") != Some("commit") {
        probs.push(prob("HINT_TAG_TARGET_NOT_COMMIT", &format!("태그 대상이 커밋이 아니다({})", obj["target_type"].display())));
        let hits = pii::scan_text(&body, Some(&terms), "deploy", &[], false, true, None)?;
        if !hits.is_empty() {
            probs.push(prob("HINT_TAG_PII", &format!("annotation PII: {}", pii::render_hits(&hits, 5)?)));
        }
    } else {
        catch!(branch::require_payload_anchor(repo, tag, &target));
        if let Some(more) = catch!(_anchor_problems(repo, tag, &target, brief.as_deref(), &body, &terms)) {
            probs.extend(more);
        }
        if let Some(lf) = lint_fn {
            let found = _lint_findings(lf, repo, &target, tag)?;
            probs.extend(_downgrade_source_drift(repo, &target, &found, infos.as_deref_mut(), source_repo)?);
        }
    }
    probs.sort();
    probs.dedup();
    Ok(probs)
}

/// lint 발견 → (FAIL 로 남길 것). 출처 의존 코드 ∧ 봉인 스냅샷 존재 ∧ 바뀐 출처 ≥1 인 발견만 INFO 로 옮긴다.
pub fn _downgrade_source_drift(repo: &Path, anchor: &str, findings: &[String], infos: Option<&mut Vec<J>>, source_repo: Option<&Path>) -> HR<Vec<String>> {
    let cand: Vec<&String> = findings.iter().filter(|f| SOURCE_DEPENDENT_LINT_CODES.contains(&f.split(':').next().unwrap_or(""))).collect();
    if cand.is_empty() {
        return Ok(findings.to_vec());
    }
    let (snap, snap_probs) = match sealed_sources(repo, anchor) {
        Ok(x) => x,
        Err(e) if !e.is_py() => return Ok(findings.to_vec()),
        Err(e) => return Err(e),
    };
    let Some(snap) = snap else { return Ok(findings.to_vec()) };
    if !snap_probs.is_empty() {
        return Ok(findings.to_vec());
    }
    let changed = changed_sealed_sources(source_repo.unwrap_or(repo), &snap)?;
    if changed.is_empty() {
        return Ok(findings.to_vec());
    }
    let head: Vec<String> = changed.iter().take(5).cloned().collect();
    let more = if changed.len() > 5 { " …" } else { "" };
    let mut keep = Vec::new();
    let mut infos = infos;
    for f in findings {
        if cand.contains(&f) {
            let (code, msg) = f.split_once(':').unwrap_or((f.as_str(), ""));
            let line = format!("{INFO_SOURCE_CHANGED} — {code} · 바뀐 출처 {}{more} ·{msg}", lst_s(&head));
            match infos.as_deref_mut() {
                Some(v) => v.push(J::Str(line)),
                None => log(&line),
            }
        } else {
            keep.push(f.clone());
        }
    }
    Ok(keep)
}

// ── 원격 · 자격증명 ───────────────────────────────────────────────────────────────────────────
crate::re_static!(_USERINFO_RE, r"(://)[^/@\s]*@");

/// 메시지용 URL(userinfo 제거).
pub fn _redact_url(url: &str) -> HR<String> {
    Ok(redact(url))
}
fn redact(url: &str) -> String {
    _USERINFO_RE().sub_fn(url, |c| format!("{}***@", c.get(1).map(|m| m.as_str()).unwrap_or("")))
}

pub fn _require_remote_arg(remote: &str) -> HR<()> {
    if py_strip(remote).is_empty() {
        return core::fail("HINT_PUSH_REMOTE_UNRESOLVED", "원격 이름(또는 URL·경로)이 비었다.", None);
    }
    if remote.starts_with('-') || remote.chars().any(|c| (c as u32) < 0x20) {
        return core::fail("HINT_PUSH_REMOTE_UNRESOLVED", "원격 인자가 옵션처럼 보이거나 제어문자를 담았다(옵션 주입 ✗).", None);
    }
    Ok(())
}

/// git 규칙(remote.c valid_remote_nick).
pub fn _is_remote_nick(name: &str) -> HR<bool> {
    Ok(nick(name))
}
fn nick(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

/// **선언된** URL(insteadOf 전개 전).
pub fn _declared_urls(repo: &Path, remote: &str, push: bool) -> HR<Vec<String>> {
    if nick(remote) {
        let mut keys = Vec::new();
        if push {
            keys.push(format!("remote.{remote}.pushurl"));
        }
        keys.push(format!("remote.{remote}.url"));
        for key in keys {
            let r = core::git_nocheck(repo, &["config", "--get-all", &key])?;
            let vals: Vec<String> = py_splitlines(&r.stdout).into_iter().map(py_strip).filter(|v| !v.is_empty()).map(str::to_string).collect();
            if !vals.is_empty() {
                return Ok(vals);
            }
        }
    }
    Ok(vec![remote.to_string()])
}

/// `urllib.parse.urlsplit(url)` 의 (username, password) — ValueError 문구 포함(Python 3.12).
fn urlsplit_userinfo(url: &str) -> Result<(Option<String>, Option<String>), String> {
    let rest = match url.find(':') {
        Some(i) => &url[i + 1..],
        None => url,
    };
    if !rest.starts_with("//") {
        return Ok((None, None));
    }
    let after = &rest[2..];
    let end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let netloc = &after[..end];
    let (lb, rb) = (netloc.contains('['), netloc.contains(']'));
    if lb != rb {
        return Err("Invalid IPv6 URL".into());
    }
    if lb && rb {
        let hp = netloc.rsplit_once('@').map(|x| x.1).unwrap_or(netloc);
        let hostname = match hp.split_once('[') {
            Some((before, bracketed)) => {
                if !before.is_empty() {
                    return Err("Invalid IPv6 URL".into());
                }
                let (h, port) = bracketed.split_once(']').unwrap_or((bracketed, ""));
                if !port.is_empty() && !port.starts_with(':') {
                    return Err("Invalid IPv6 URL".into());
                }
                h.to_string()
            }
            None => hp.split_once(':').map(|x| x.0).unwrap_or(hp).to_string(),
        };
        if hostname.starts_with('v') {
            let ok = crate::pyre::Re::new(r"\Av[a-fA-F0-9]+\..+\Z", 0).is_match(&hostname);
            if !ok {
                return Err("IPvFuture address is invalid".into());
            }
        } else {
            match hostname.parse::<std::net::IpAddr>() {
                Ok(std::net::IpAddr::V4(_)) => return Err("An IPv4 address cannot be in brackets".into()),
                Ok(_) => {}
                Err(_) => return Err(format!("{} does not appear to be an IPv4 or IPv6 address", py_repr(&hostname))),
            }
        }
    }
    match netloc.rsplit_once('@') {
        Some((userinfo, _)) => match userinfo.split_once(':') {
            Some((u, p)) => Ok((Some(u.to_string()), Some(p.to_string()))),
            None => Ok((Some(userinfo.to_string()), None)),
        },
        None => Ok((None, None)),
    }
}

/// 선언 URL → `https`·`http`·`ssh`·`local`·`git`. 판정할 수 없으면 거부.
pub fn _transport_of(url: &str) -> HR<String> {
    if let Some(m) = _SCHEME_RE().match_at_start(url) {
        let scheme = m.get(1).unwrap().as_str().to_string();
        let Some(t) = _SCHEME_TRANSPORT().get_str(&scheme.to_lowercase()).map(str::to_string) else {
            return core::fail(
                "HINT_PUSH_REMOTE_SCHEME_UNKNOWN",
                format!("판정할 수 없는 원격 스킴: {}", py_repr(&scheme)),
                Some("https · ssh · scp형(user@host:path) · 로컬 경로만 다룬다."),
            );
        };
        let (user, pw) = match urlsplit_userinfo(url) {
            Ok(x) => x,
            Err(e) => {
                return core::fail(
                    "HINT_PUSH_REMOTE_URL_INVALID",
                    format!("원격 URL 을 해석할 수 없다({e}): {}", redact(url)),
                    Some("remote.<이름>.url 을 고친다."),
                )
            }
        };
        if pw.is_some() || ((t == "https" || t == "http") && user.is_some()) {
            return core::fail(
                "HINT_PUSH_URL_EMBEDS_CREDENTIAL",
                format!("원격 URL 에 자격증명(userinfo)이 박혀 있다: {}", redact(url)),
                Some(&format!(
                    "remote.url 에서 userinfo 를 걷어내고 `{TOKEN_ENV_FILE_REL}`(비추적)의 `{TOKEN_KEY}` 를 쓴다 — URL 에 토큰을 박으면 reflog·설정·로그에 남는다."
                )),
            );
        }
        return Ok(t);
    }
    if url.contains("::") {
        return core::fail("HINT_PUSH_REMOTE_SCHEME_UNKNOWN", "원격 helper 형식(`<transport>::<address>`)은 판정하지 않는다.", None);
    }
    if let Some((head, _)) = url.split_once(':') {
        if !head.contains('/') {
            return Ok("ssh".into());
        }
    }
    Ok("local".into())
}

/// 선언 URL 들의 전송 집합(정렬 목록 = Python set).
pub fn _remote_transports(repo: &Path, remote: &str, push: bool) -> HR<Vec<String>> {
    _require_remote_arg(remote)?;
    let mut out = Vec::new();
    for u in _declared_urls(repo, remote, push)? {
        out.push(_transport_of(&u)?);
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// push 에 토큰이 필요한가: https 만 참.
pub fn remote_needs_token(repo: &Path, remote: &str) -> HR<bool> {
    let ts = _remote_transports(repo, remote, true)?;
    if ts.iter().any(|t| t == "http") {
        return core::fail(
            "HINT_PUSH_INSECURE_TRANSPORT",
            "평문 http 원격에는 push 자격증명을 보내지 않는다.",
            Some("https 또는 ssh 원격을 쓴다."),
        );
    }
    Ok(ts.iter().any(|t| t == "https"))
}

/// push 용 PAT: 환경변수 `GITHUB_TOKEN` → `<aux_root>/envs/.env` 의 같은 키. 없으면 None. `environ` = J dict 또는 Null(os.environ).
pub fn read_push_token(repo: &Path, environ_: &J) -> HR<Option<String>> {
    let direct = if environ_.is_null() {
        std::env::var(TOKEN_KEY).unwrap_or_default()
    } else {
        let v = environ_.get(TOKEN_KEY).cloned().unwrap_or(J::Null);
        if v.truthy() { v.display() } else { String::new() }
    };
    let direct = py_strip(&direct);
    if !direct.is_empty() {
        return Ok(Some(direct.to_string()));
    }
    let f = core::aux_root(repo).join(TOKEN_ENV_FILE_REL);
    if !f.is_file() {
        return Ok(None);
    }
    let text = match std::fs::read(&f) {
        Err(e) => {
            let ty = match e.kind() {
                std::io::ErrorKind::PermissionDenied => "PermissionError",
                std::io::ErrorKind::NotFound => "FileNotFoundError",
                _ => "OSError",
            };
            return token_unreadable(ty);
        }
        Ok(b) => match String::from_utf8(b) {
            Ok(t) => t,
            Err(_) => return token_unreadable("UnicodeDecodeError"),
        },
    };
    let prefix = format!("{TOKEN_KEY}=");
    for line in py_splitlines(&text) {
        let st = py_strip(line);
        if st.starts_with('#') || !st.starts_with(&prefix) {
            continue;
        }
        let val = py_strip(st.split_once('=').map(|x| x.1).unwrap_or("")).trim_matches('"').trim_matches('\'');
        if !val.is_empty() {
            return Ok(Some(val.to_string()));
        }
    }
    Ok(None)
}

fn token_unreadable<T>(ty: &str) -> HR<T> {
    core::fail(
        "HINT_PUSH_TOKEN_FILE_UNREADABLE",
        format!("`{TOKEN_ENV_FILE_REL}` 를 읽을 수 없다({ty})"),
        Some("UTF-8 텍스트 `GITHUB_TOKEN=<PAT>` 한 줄로 고친다(값은 출력하지 않는다)."),
    )
}

/// `git -c` 인자만 산출한다(토큰 없음).
pub fn push_credential_args() -> HR<Vec<String>> {
    let helper = format!("!f() {{ test \"$1\" = get || exit 0; echo username=x-access-token; echo \"password=${PUSH_TOKEN_ENV}\"; }}; f");
    Ok(vec!["-c".into(), "credential.helper=".into(), "-c".into(), format!("credential.helper={helper}")])
}

pub fn _scrub(text: &str, token: Option<&str>) -> HR<String> {
    Ok(scrub(text, token))
}
fn scrub(text: &str, token: Option<&str>) -> String {
    match token {
        Some(t) if !t.is_empty() && !text.is_empty() => text.replace(t, "***"),
        _ => text.to_string(),
    }
}

/// (env, git 앞 인자, 토큰). https 이고 토큰이 있으면 helper 를 싣는다. 토큰은 env 에만 둔다.
pub fn _remote_env(repo: &Path, remote: &str, push: bool) -> HR<(J, Vec<String>, Option<String>)> {
    let ts = _remote_transports(repo, remote, push)?;
    let mut env = environ();
    if let Some(m) = env.as_dict_mut() {
        m.remove(PUSH_TOKEN_ENV);
    }
    env.set("GIT_TERMINAL_PROMPT", "0");
    let token = if ts.iter().any(|t| t == "https") { read_push_token(repo, &J::Null)? } else { None };
    if let Some(t) = token.filter(|t| !t.is_empty()) {
        env.set(PUSH_TOKEN_ENV, t.clone());
        return Ok((env, push_credential_args()?, Some(t)));
    }
    Ok((env, vec![], None))
}

fn run_git_text(repo: &Path, pre: &[String], args: &[&str], env: &J) -> HR<(Vec<String>, i32, String, String)> {
    let mut argv = vec!["git".to_string()];
    argv.extend(pre.iter().cloned());
    argv.extend(args.iter().map(|s| s.to_string()));
    match run_env(&argv, repo, env, None) {
        Ok(p) => Ok((argv, p.returncode, text_mode(&p.stdout), text_mode(&p.stderr))),
        Err(e) => core::fail("HINT_GIT_UNAVAILABLE", format!("git 실행 불가: {e}"), None),
    }
}

/// `git ls-remote --refs <remote> <pattern>` → {ref: 직접 오브젝트 SHA}(읽기).
pub fn _ls_remote_refs(repo: &Path, remote: &str, pattern: &str) -> HR<J> {
    let (env, pre, token) = _remote_env(repo, remote, false)?;
    let (_argv, rc, stdout, stderr) = run_git_text(repo, &pre, &["ls-remote", "--refs", remote, pattern], &env)?;
    if rc != 0 {
        return core::fail(
            "HINT_REMOTE_QUERY_FAILED",
            format!("ls-remote {} 실패(rc={rc}): {}", redact(remote), core::tail_chars(py_strip(&scrub(&stderr, token.as_deref())), 600)),
            Some("원격 이름·경로와 네트워크를 확인한다(https 비공개 원격이면 자격증명 — envs/.env 의 GITHUB_TOKEN). 조회 실패는 '없음' 으로 읽지 않는다."),
        );
    }
    let mut out = Map::new();
    for line in py_splitlines(&stdout) {
        let (sha, r) = line.split_once('\t').unwrap_or((line, ""));
        if !r.is_empty() && _SHA40_RE().is_full(py_strip(sha)) {
            out.insert(py_strip(r), py_strip(sha));
        }
    }
    Ok(J::Dict(out))
}

/// 원격의 refs/tags/<tag> 가 광고하는 **피일 전** 오브젝트 SHA.
pub fn remote_tag_object(repo: &Path, remote: &str, tag: &str) -> HR<Option<String>> {
    check_ref_format(repo, tag)?;
    let r = tref(tag);
    Ok(_ls_remote_refs(repo, remote, &r)?.get_str(&r).map(str::to_string))
}

/// 원격의 완전 ref 하나를 읽는다.
pub fn remote_ref_object(repo: &Path, remote: &str, ref_: &str) -> HR<Option<String>> {
    if !ref_.starts_with("refs/") || ref_.contains(':') || ref_.chars().any(py_isspace) {
        return core::fail("HINT_BRANCH_REF_INVALID", format!("원격 branch ref 형식이 올바르지 않다: {}", py_repr(ref_)), None);
    }
    if core::git_nocheck(repo, &["check-ref-format", ref_])?.returncode != 0 {
        return core::fail("HINT_BRANCH_REF_INVALID", format!("git check-ref-format 이 원격 branch ref를 거부했다: {}", py_repr(ref_)), None);
    }
    Ok(_ls_remote_refs(repo, remote, ref_)?.get_str(ref_).map(str::to_string))
}

/// 원격의 완전 ref **하나**의 오브젝트를 받아 온다. 반환 = 갱신한 원격 추적 ref 또는 None.
pub fn fetch_ref_objects(repo: &Path, remote: &str, ref_: &str) -> HR<Option<String>> {
    if !ref_.starts_with("refs/heads/") || ref_.contains(':') || ref_.chars().any(py_isspace) {
        return core::fail("HINT_BRANCH_REF_INVALID", format!("fetch 할 branch ref 형식이 올바르지 않다: {}", py_repr(ref_)), None);
    }
    if core::git_nocheck(repo, &["check-ref-format", ref_])?.returncode != 0 {
        return core::fail("HINT_BRANCH_REF_INVALID", format!("git check-ref-format 이 fetch ref 를 거부했다: {}", py_repr(ref_)), None);
    }
    let (env, pre, token) = _remote_env(repo, remote, false)?;
    let mut tracking = None;
    if nick(remote) && !py_strip(&core::git_nocheck(repo, &["config", "--get", &format!("remote.{remote}.url")])?.stdout).is_empty() {
        tracking = Some(format!("refs/remotes/{remote}/{}", &ref_["refs/heads/".len()..]));
    }
    let spec = match &tracking {
        Some(t) => format!("+{ref_}:{t}"),
        None => ref_.to_string(),
    };
    let (_argv, rc, _stdout, stderr) =
        run_git_text(repo, &pre, &["fetch", "--no-tags", "--no-recurse-submodules", "--no-write-fetch-head", remote, &spec], &env)?;
    if rc != 0 {
        return core::fail(
            "HINT_REMOTE_QUERY_FAILED",
            format!("fetch {} {ref_} 실패(rc={rc}): {}", redact(remote), core::tail_chars(py_strip(&scrub(&stderr, token.as_deref())), 600)),
            Some("원격 이름·경로와 네트워크를 확인한다(https 비공개 원격이면 envs/.env 의 GITHUB_TOKEN)."),
        );
    }
    Ok(tracking)
}

/// 공개 push API 의 refspec 규율: `refs/…:refs/…` 한 쌍 · 양쪽 같음 · 강제·옵션·glob ✗ · ref 형식 통과.
pub fn _require_safe_refspec(repo: &Path, refspec: &str) -> HR<()> {
    let mut bad: Option<&str> = None;
    if refspec.is_empty() {
        bad = Some("비었다");
    } else if refspec.starts_with('+') || refspec.starts_with('-') {
        bad = Some("강제(+)·옵션(-) 형태(`--tags` 포함)");
    } else if refspec.chars().any(|c| _GLOB_CHARS.contains(&c)) {
        bad = Some("glob 문자");
    } else if refspec.matches(':').count() != 1 || refspec.chars().any(py_isspace) {
        bad = Some("`src:dst` 한 쌍이 아니다");
    } else {
        let (src, dst) = refspec.split_once(':').unwrap();
        if src != dst || !src.starts_with("refs/") {
            bad = Some("src ≠ dst 이거나 `refs/` 전체 이름이 아니다(이름 바꿔 밀기 ✗)");
        } else if core::git_nocheck(repo, &["check-ref-format", src])?.returncode != 0 {
            bad = Some("git check-ref-format 거부");
        }
    }
    if let Some(b) = bad {
        return core::fail(
            "HINT_PUSH_REFSPEC_FORBIDDEN",
            format!("push refspec 거부({b}): {}", py_repr(refspec)),
            Some("정확한 ref 1개를 `refs/<…>:refs/<…>` 로 민다."),
        );
    }
    Ok(())
}

/// push 자격증명 판정 + env(한 자리). https 인데 토큰 없음 = HINT_PUSH_CREDENTIAL_ABSENT.
pub fn _push_env(repo: &Path, remote: &str) -> HR<(J, Vec<String>, Option<String>)> {
    let needs = remote_needs_token(repo, remote)?;
    let (env, pre, token) = _remote_env(repo, remote, true)?;
    if needs && token.is_none() {
        return core::fail(
            "HINT_PUSH_CREDENTIAL_ABSENT",
            format!(
                "https 원격 push 자격증명이 없다 — `{TOKEN_ENV_FILE_REL}`(비추적)에 `{TOKEN_KEY}=<PAT>` 한 줄을 두거나 환경변수 `{TOKEN_KEY}` 를 준다. 이 배선이 없으면 무인 실행이 `could not read Username` 으로 멈춘다."
            ),
            Some("ssh 원격(scp형 포함)을 쓰면 토큰이 필요 없다."),
        );
    }
    Ok((env, pre, token))
}

/// push 에 늘 붙이는 옵션(upstream push_branches 의 자체검사가 tag.py 의 이 리터럴을 읽는다).
pub const PUSH_SCOPE_ARGS: [&str; 2] = ["--no-follow-tags", "--no-recurse-submodules"];

/// **공개 API**. 스킴 인식 자격증명으로 refspec 하나를 민다. 성공 = `{"args","returncode","stdout","stderr"}`
/// (토큰 문자열은 가린다) · 실패 = HintError(HINT_PUSH_FAILED · dry-run 도 같다).
pub fn git_push_authenticated(repo: &Path, remote: &str, refspec: &str, dry_run: bool) -> HR<J> {
    _require_remote_arg(remote)?;
    _require_safe_refspec(repo, refspec)?;
    let (env, pre, token) = _push_env(repo, remote)?;
    let mut args: Vec<&str> = vec!["push"];
    args.extend(PUSH_SCOPE_ARGS);
    if dry_run {
        args.push("--dry-run");
    }
    args.push(remote);
    args.push(refspec);
    let (argv, rc, stdout, stderr) = run_git_text(repo, &pre, &args, &env)?;
    let (stdout, stderr) = (scrub(&stdout, token.as_deref()), scrub(&stderr, token.as_deref()));
    if rc != 0 {
        return core::fail(
            "HINT_PUSH_FAILED",
            format!(
                "git push{} {} {refspec} → rc={rc}: {}",
                if dry_run { " --dry-run" } else { "" },
                redact(remote),
                core::tail_chars(py_strip(&stderr), 800)
            ),
            Some("원격 거부·인증·네트워크를 확인한다(dry-run 실패도 실패다 — 삼키지 않는다)."),
        );
    }
    Ok(proc_j(&argv, rc, &stdout, &stderr))
}

/// 태그 **하나**를 정확한 refspec `refs/tags/<tag>:refs/tags/<tag>` 로 민다.
/// 반환 {remote, tag, refspec, dry_run, local_object, remote_object, status}.
pub fn push_tag(repo: &Path, remote: &str, tag: &str, dry_run: bool, lint_fn: Option<LintFn<'_>>, infos: Option<&mut Vec<J>>, source_repo: Option<&Path>) -> HR<J> {
    check_ref_format(repo, tag)?;
    match tag_ref_kind(repo, tag)? {
        None => return core::fail("HINT_TAG_ABSENT", format!("로컬에 태그가 없다: {tag}"), None),
        Some(k) if k != "tag" => {
            return core::fail("HINT_TAG_NOT_ANNOTATED", format!("annotated 태그가 아니다({k}) — lightweight 태그는 밀지 않는다."), None)
        }
        _ => {}
    }
    let probs = verify_local(repo, tag, lint_fn, infos, source_repo)?;
    if !probs.is_empty() {
        return _fail_problems(
            "HINT_PUSH_UNVERIFIED",
            "push 전 로컬 검증(이 태그)",
            &probs,
            "hint.py verify --tag <태그> 로 확인하고 고친다 — 검증되지 않은 것은 밀지 않는다.",
        );
    }
    let r = tref(tag);
    let refspec = format!("{r}:{r}");
    if refspec.matches(':').count() != 1 || refspec.contains("refs/heads/") || refspec.chars().any(|c| _GLOB_CHARS.contains(&c)) {
        return core::fail("HINT_PUSH_REFSPEC_FORBIDDEN", format!("태그 refspec 가 정확한 태그 1개가 아니다: {}", py_repr(&refspec)), None);
    }
    _require_remote_arg(remote)?;
    _push_env(repo, remote)?;
    let local_obj = read_tag(repo, tag)?.map(|t| t["object_sha"].display()).unwrap_or_default();
    let before = remote_tag_object(repo, remote, tag)?;
    let mut result = jd! {
        "remote" => remote, "tag" => tag, "refspec" => refspec.clone(), "dry_run" => dry_run,
        "local_object" => local_obj.clone(), "remote_object" => J::from(before.clone()),
    };
    if let Some(b) = &before {
        if *b == local_obj {
            log(&format!("{} 에 이미 같은 오브젝트가 있다({}) — push 생략(재개).", redact(remote), core::head_chars(&local_obj, 12)));
            result.set("status", "already-on-remote");
            return Ok(result);
        }
        return core::fail(
            "HINT_REMOTE_TAG_CONFLICT",
            format!(
                "원격 {} 의 {r} 가 다른 오브젝트다(원격 {} ≠ 로컬 {}) — 덮어쓰지 않는다.",
                redact(remote),
                core::head_chars(b, 12),
                core::head_chars(&local_obj, 12)
            ),
            Some("태그는 불변이다(P1 리콜 금지 · 강제 push ✗). 개정판은 새 이름으로 발행한다."),
        );
    }
    log(&format!("push {} {refspec}{} — 태그 1개 · --tags ✗ · 브랜치 ✗", redact(remote), if dry_run { " (dry-run)" } else { "" }));
    git_push_authenticated(repo, remote, &refspec, dry_run)?;
    if dry_run {
        result.set("status", "dry-run");
        return Ok(result);
    }
    let after = remote_tag_object(repo, remote, tag)?;
    if after.as_deref() != Some(local_obj.as_str()) {
        return core::fail(
            "HINT_REMOTE_SHA_MISMATCH",
            format!("push 뒤 원격 {r} = {} ≠ 로컬 {}", core::head_chars(&after.clone().unwrap_or_else(|| "None".into()), 12), core::head_chars(&local_obj, 12)),
            None,
        );
    }
    log(&format!("pushed {refspec} → {} ({})", redact(remote), core::head_chars(&local_obj, 12)));
    result.set("status", "pushed");
    result.set("remote_object", J::from(after));
    Ok(result)
}

// ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
pub const _FX_UTC: &str = "2026-09-21T10:21:01Z";
pub const _FX_UTC2: &str = "2026-09-21T11:00:00Z";
pub fn _FX_TOKEN() -> String {
    format!("fixture-token-{}", "5f3a9c0e71d24b68")
}
pub const _FX_HOST: &str = "fixture.invalid";

fn _fx_fields(tag: &str, anchor: &str) -> J {
    jd! {"version" => "2", "tag" => tag, "topology" => "multi TP=2(Ray)", "anchor" => anchor,
         "manifest_ref" => "docs/_evidence/fixture.work-manifest.json",
         "bench_ref" => "../benchmark/benchmark_fixture.yaml", "bench_kind" => "certificate"}
}

/// 옛 태그(footer v1) 모양 — 읽기 호환 대조용.
fn _fx_fields_v1(tag: &str, anchor: &str) -> J {
    jd! {"version" => "1", "tag" => tag, "topology" => "multi TP=2(Ray)", "anchor" => anchor,
         "manifest_ref" => "docs/_evidence/fixture.work-manifest.json",
         "certificate_ref" => "../benchmark/bench_report_fixture.md"}
}

fn with(d: &J, k: &str, v: impl Into<J>) -> J {
    let mut d = d.clone();
    d.set(k, v.into());
    d
}

struct Ck(Vec<String>);
impl Ck {
    fn ck(&mut self, name: &str, cond: bool) {
        if !cond {
            self.0.push(format!("tag: {name}"));
        }
    }
}

fn code<T>(r: HR<T>) -> Option<String> {
    r.err().map(|e| e.code)
}

/// `tag.selftest()` — 실패 메시지 목록(빈 목록 = 통과). 격리 임시 저장소 + 로컬 bare 원격만 쓴다.
pub fn selftest() -> Vec<String> {
    let mut c = Ck(Vec::new());
    if let Err(e) = selftest_pure(&mut c) {
        c.0.push(format!("tag: 순수 자체검사 중 예외 {}: {}", e.code, core::head_chars(&e.message, 300)));
    }
    crate::io::quiet_stderr(|| {
        let td = core::TempDir::new("hint-tag-selftest-");
        let tmp = td.path().to_path_buf();
        let guard = branch::selftest_env(&tmp, &[]);
        match guard {
            Err(e) => c.0.push(format!("tag: 자체검사 중 예기치 않은 HintError {}: {}", e.code, core::head_chars(&e.message, 400))),
            Ok(_g) => {
                if let Err(e) = selftest_git(&tmp, &mut c) {
                    if e.is_py() {
                        c.0.push(format!("tag: 자체검사 중 예외 {}: {}", e.code.trim_start_matches("py:"), core::head_chars(&e.message, 300)));
                    } else {
                        c.0.push(format!("tag: 자체검사 중 예기치 않은 HintError {}: {}", e.code, core::head_chars(&e.message, 400)));
                    }
                }
            }
        }
        spy_take();
    });
    c.0
}

fn selftest_pure(c: &mut Ck) -> HR<()> {
    let fields = _fx_fields("hint/x/y/z/w", &"a".repeat(40));
    let wire = build_footer(&fields)?;
    c.ck("footer 왕복", parse_footer(&wire, &[]).ok().as_ref() == Some(&fields));
    c.ck(
        "FOOTER_FIELDS 순서 고정(claim C3 · v2)",
        FOOTER_FIELDS == ["version", "tag", "topology", "anchor", "manifest_ref", "bench_ref", "bench_kind"]
            && FOOTER_FIELDS_V1 == ["version", "tag", "topology", "anchor", "manifest_ref", "certificate_ref"],
    );
    c.ck(
        "v2 여는 줄에 '로컬 경로 · 수신자 해소 대상 아님' 고정 문구",
        wire.starts_with(&format!("{FOOTER_MARKER_OPEN}\n")) && FOOTER_MARKER_OPEN.contains("수신자 해소 대상 아님"),
    );
    c.ck("★footer 부재 = MISSING", code(parse_footer("요약만 있다\n", &[])).as_deref() == Some("HINT_EVIDENCE_BINDING_MISSING"));
    let a40 = "a".repeat(40);
    let malformed: Vec<(&str, String)> = vec![
        ("닫는 줄 없음", wire.replace("\n-->", "")),
        ("중복 키", wire.replacen("tag: ", "tag: a\ntag: ", 1)),
        ("미지 키", wire.replace("version: 2\n", "version: 2\nextra_key: x\n")),
        ("짧은 anchor", wire.replace(&format!("anchor: {a40}"), &format!("anchor: {}", "a".repeat(12)))),
        ("빈 필드", wire.replace("topology: multi TP=2(Ray)", "topology: ")),
        ("본문 version ≠ 여는 줄 판", wire.replace("version: 2", "version: 1")),
        ("모르는 판 v3", wire.replace("version: 2", "version: 3").replace(FOOTER_MARKER_OPEN, "<!-- hint-evidence-binding:v3")),
        ("여는 줄 문구 변조", wire.replace(FOOTER_MARKER_OPEN, &format!("{FOOTER_MARKER_OPEN} 덧붙임"))),
        ("필드 누락", wire.replace("bench_kind: certificate\n", "")),
        ("v2 에 v1 키(certificate_ref)", wire.replace("bench_ref:", "certificate_ref:")),
        ("bench_kind 어휘 밖", wire.replace("bench_kind: certificate", "bench_kind: cert")),
        ("bench_kind ≠ 이름 파생(리포트를 인증서라 부름)", wire.replace("benchmark_fixture.yaml", "bench_report_fixture.md")),
        ("footer 둘", format!("{wire}{wire}")),
        ("해석 불가 줄", wire.replace("version: 2\n", "version: 2\nnot a field line\n")),
        ("절대경로 주소", wire.replace("manifest_ref: docs/", "manifest_ref: /docs/")),
    ];
    for (label, text) in &malformed {
        c.ck(
            &format!("★footer {label} = MALFORMED(≠ MISSING)"),
            code(parse_footer(text, &[])).as_deref() == Some("HINT_EVIDENCE_BINDING_MALFORMED"),
        );
    }
    c.ck(
        "footer 결함은 HintError 로도 잡힌다(CLI 단일 처리)",
        is_evidence_binding_error(&parse_footer("x", &[]).unwrap_err()),
    );
    let retired: Vec<String> = RETIRED_FOOTER_KEYS.iter().map(|s| s.to_string()).collect();
    let legacy = wire.replace("bench_ref:", &format!("manifest_sha256: {}\nbench_ref:", "0".repeat(64)));
    c.ck(
        "★명시 인자가 있으면 폐기 키를 읽고 버린다",
        parse_footer(&legacy, &retired).map(|f| f.get("manifest_sha256").is_none()).unwrap_or(false),
    );
    c.ck("★음성대조 명시 인자 없으면 폐기 키는 여전히 차단", code(parse_footer(&legacy, &[])).as_deref() == Some("HINT_EVIDENCE_BINDING_MALFORMED"));
    c.ck(
        "★음성대조 폐기 키가 아닌 미지 키는 명시 인자가 있어도 차단",
        code(parse_footer(&wire.replace("version: 2\n", "version: 2\ntotally_unknown: x\n"), &retired)).as_deref()
            == Some("HINT_EVIDENCE_BINDING_MALFORMED"),
    );
    c.ck(
        "★봉인이 폐기 키를 발행하는 경로는 없다",
        !FOOTER_FIELDS.iter().any(|f| RETIRED_FOOTER_KEYS.contains(f)) && !RETIRED_FOOTER_KEYS.iter().any(|k| wire.contains(k)),
    );
    let f1 = _fx_fields_v1("hint/x/y/z/w", &a40);
    let w1 = build_footer(&f1)?;
    c.ck(
        "footer v1 렌더 = 옛 wire 모양(여는 줄 v1)",
        w1.starts_with(&format!("{FOOTER_MARKER_OPEN_V1}\n")) && w1.contains("certificate_ref: "),
    );
    c.ck("footer v1 파싱 왕복(읽기 호환)", parse_footer(&w1, &[]).ok().as_ref() == Some(&f1));
    c.ck(
        "★v1 여는 줄에 v2 필드 = MALFORMED",
        code(parse_footer(&w1.replace("certificate_ref:", "bench_ref:"), &[])).as_deref() == Some("HINT_EVIDENCE_BINDING_MALFORMED"),
    );
    c.ck(
        "bench_binding(v2) = 선언",
        bench_binding(&fields)? == jd! {"ref" => "../benchmark/benchmark_fixture.yaml", "kind" => "certificate", "kind_source" => "declared"},
    );
    c.ck(
        "bench_binding(v1 · 리포트 이름) = 이름 파생 bench_report(v1 의 거짓 이름을 바로잡아 읽는다)",
        bench_binding(&f1)? == jd! {"ref" => "../benchmark/bench_report_fixture.md", "kind" => "bench_report", "kind_source" => "derived-from-name"},
    );
    c.ck(
        "bench_binding(v1 · 판정 불가 이름) = kind None · underivable",
        bench_binding(&with(&f1, "certificate_ref", "x/other.txt"))?.get_str("kind_source") == Some("underivable"),
    );
    c.ck(
        "bench_kind_of: 인증서·리포트·기타",
        bench_kind_of(&J::from("../benchmark/benchmark_a_b.yaml"))?.as_deref() == Some("certificate")
            && bench_kind_of(&J::from("docs/benchmark/bench_report_a.md"))?.as_deref() == Some("bench_report")
            && bench_kind_of(&J::from("x.md"))?.is_none(),
    );
    let undet = with(&with(&fields, "bench_ref", "x/other.md"), "bench_kind", "bench_report");
    c.ck(
        "판정 불가 이름이면 선언 kind 를 받는다",
        parse_footer(&build_footer(&undet)?, &[])?.get_str("bench_kind") == Some("bench_report"),
    );
    let mut no_tag = fields.clone();
    no_tag.as_dict_mut().unwrap().remove("tag");
    for (label, bad) in [
        ("여러 줄 값", with(&fields, "topology", "a\nb")),
        ("주석 경계 값", with(&fields, "tag", "x-->")),
        ("필드 누락", no_tag),
        ("필드 초과", with(&fields, "extra", "x")),
    ] {
        c.ck(&format!("★build_footer {label} 거부"), code(build_footer(&bad)).as_deref() == Some("HINT_EVIDENCE_BINDING_MALFORMED"));
    }
    let msg = annotation("한 줄 요약", &fields)?;
    c.ck("annotation 정본 모양(SPEC §2.2)", msg == format!("한 줄 요약\n\n{ANNOTATION_POINTER}\n\n{wire}"));
    c.ck("parse_annotation 왕복", parse_annotation(&msg)? == jd! {"brief" => "한 줄 요약", "footer" => fields.clone()});
    c.ck("brief 줄 끝 공백 정규화", annotation("첫 줄  \n둘째 줄", &fields)?.starts_with("첫 줄\n둘째 줄\n\n"));
    c.ck("★brief 4줄 = TOO_LONG", code(annotation("1\n2\n3\n4", &fields)).as_deref() == Some("HINT_ANNOTATION_BRIEF_TOO_LONG"));
    c.ck("★빈 brief = ABSENT", code(annotation("  \n", &fields)).as_deref() == Some("HINT_ANNOTATION_BRIEF_ABSENT"));
    c.ck("★brief 속 주석 경계 = SHAPE", code(annotation("요약 <!-- x", &fields)).as_deref() == Some("HINT_ANNOTATION_BRIEF_SHAPE"));
    c.ck("★brief 속 빈 줄(두 문단) = SHAPE", code(annotation("a\n\nb", &fields)).as_deref() == Some("HINT_ANNOTATION_BRIEF_SHAPE"));
    c.ck("★뒤꼬리 덧붙임 = HINT_ANNOTATION_SHAPE", code(parse_annotation(&format!("{msg}덧붙임\n"))).as_deref() == Some("HINT_ANNOTATION_SHAPE"));
    c.ck(
        "★포인터 부재 = HINT_ANNOTATION_SHAPE",
        code(parse_annotation(&msg.replace(ANNOTATION_POINTER, "다른 문장"))).as_deref() == Some("HINT_ANNOTATION_SHAPE"),
    );
    c.ck(
        "★footer 없는 annotation = MISSING",
        code(parse_annotation(&format!("요약\n\n{ANNOTATION_POINTER}\n"))).as_deref() == Some("HINT_EVIDENCE_BINDING_MISSING"),
    );
    c.ck(
        "카탈로그 brief 추출 = annotation brief(다줄은 공백 이음)",
        crate::catalog::annotation_brief(&annotation("첫 줄\n둘째 줄", &fields)?)? == "첫 줄 둘째 줄",
    );
    // push 토큰 판독
    {
        let td = core::TempDir::new("hint-tag-token-");
        let t = td.path();
        let envf = t.join(TOKEN_ENV_FILE_REL);
        let empty = J::dict();
        c.ck("파일 없으면 None", read_push_token(t, &empty)?.is_none());
        std::fs::create_dir_all(envf.parent().unwrap()).map_err(|e| core::io_err(&e, t))?;
        let wr = |s: &str| std::fs::write(&envf, s).map_err(|e| core::io_err(&e, &envf));
        wr("# 주석\nHF_TOKEN=hf_x\nGITHUB_TOKEN=ghp_FILE\n")?;
        c.ck("envs/.env 에서 판독", read_push_token(t, &empty)?.as_deref() == Some("ghp_FILE"));
        c.ck("★환경변수가 파일을 이긴다", read_push_token(t, &jd! {TOKEN_KEY => "ghp_ENV"})?.as_deref() == Some("ghp_ENV"));
        wr("GITHUB_TOKEN=\n")?;
        c.ck("★음성대조 빈 값은 토큰이 아니다", read_push_token(t, &empty)?.is_none());
        wr("#GITHUB_TOKEN=ghp_commented\n")?;
        c.ck("★음성대조 주석 처리된 줄은 무시", read_push_token(t, &empty)?.is_none());
        wr("GITHUB_TOKEN=\"ghp_Q\"\n")?;
        c.ck("따옴표 제거", read_push_token(t, &empty)?.as_deref() == Some("ghp_Q"));
    }
    let args = push_credential_args()?;
    c.ck("상속 helper 를 먼저 비운다", args[..2] == ["-c".to_string(), "credential.helper=".to_string()]);
    let helper = if args.len() == 4 { args[3].clone() } else { String::new() };
    c.ck("인자는 `-c` 두 쌍이다", args.len() == 4 && args[0] == "-c" && args[2] == "-c");
    c.ck("우리 helper 가 환경변수에서 읽는다", helper.contains(&format!("${PUSH_TOKEN_ENV}")));
    c.ck("★토큰 값이 argv 에 없다", !args.iter().any(|a| a.contains("ghp_") || a.contains(&_FX_TOKEN())));
    c.ck("helper 는 get 이외 연산에 응답하지 않는다", helper.contains("test \"$1\" = get"));
    let scp = format!("git{}{_FX_HOST}:org/r.git", "@");
    let table: Vec<(String, &str)> = vec![
        (format!("https://{_FX_HOST}/r.git"), "https"),
        (scp, "ssh"),
        (format!("ssh://{_FX_HOST}/r.git"), "ssh"),
        ("/srv/remote/r.git".into(), "local"),
        ("../r.git".into(), "local"),
        ("r.git".into(), "local"),
        ("file:///srv/r.git".into(), "local"),
        (format!("git://{_FX_HOST}/r.git"), "git"),
        (format!("http://{_FX_HOST}/r.git"), "http"),
    ];
    for (url, want) in &table {
        let head = url.split(':').next().unwrap_or("");
        c.ck(&format!("스킴 판정 {head}… = {want}"), _transport_of(url).ok().as_deref() == Some(*want));
    }
    let cred_url = format!("https://user:secret{}{_FX_HOST}/r.git", "@");
    let tok_url = format!("https://tok{}{_FX_HOST}/r.git", "@");
    c.ck("★URL 속 user:pw = HINT_PUSH_URL_EMBEDS_CREDENTIAL", code(_transport_of(&cred_url)).as_deref() == Some("HINT_PUSH_URL_EMBEDS_CREDENTIAL"));
    c.ck("★URL 속 토큰 사용자명 = HINT_PUSH_URL_EMBEDS_CREDENTIAL", code(_transport_of(&tok_url)).as_deref() == Some("HINT_PUSH_URL_EMBEDS_CREDENTIAL"));
    c.ck(
        "★ssh URL 의 비밀번호 자리 = HINT_PUSH_URL_EMBEDS_CREDENTIAL",
        code(_transport_of(&format!("ssh://git:pw{}{_FX_HOST}/r.git", "@"))).as_deref() == Some("HINT_PUSH_URL_EMBEDS_CREDENTIAL"),
    );
    c.ck("ssh URL 의 사용자명만은 정상", _transport_of(&format!("ssh://git{}{_FX_HOST}/r.git", "@")).ok().as_deref() == Some("ssh"));
    c.ck("★원격 helper 형식 = SCHEME_UNKNOWN", code(_transport_of("ext::sh -c x")).as_deref() == Some("HINT_PUSH_REMOTE_SCHEME_UNKNOWN"));
    c.ck("★미지 스킴 = SCHEME_UNKNOWN", code(_transport_of("foo://x/r")).as_deref() == Some("HINT_PUSH_REMOTE_SCHEME_UNKNOWN"));
    c.ck("메시지용 URL 은 userinfo 를 가린다", !redact(&cred_url).contains("secret"));
    c.ck(
        "★해석 불가 URL(닫히지 않은 IPv6) = HINT_PUSH_REMOTE_URL_INVALID(예외로 새지 않는다)",
        code(_transport_of("https://[::1/r.git")).as_deref() == Some("HINT_PUSH_REMOTE_URL_INVALID"),
    );
    Ok(())
}

fn rglob(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let link = e.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        if p.is_dir() && !link {
            rglob(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn s(p: &Path) -> String {
    core::posix(p)
}

fn selftest_git(tmp: &Path, c: &mut Ck) -> HR<()> {
    let u = _FX_UTC;
    let repo = branch::selftest_repo(tmp, "repo")?;
    let (op_name, op_email) = (branch::_FX_OPERATOR_NAME.to_string(), branch::_FX_OPERATOR_EMAIL());
    let op_env: Vec<(String, String)> = vec![
        ("GIT_AUTHOR_NAME".into(), op_name.clone()),
        ("GIT_AUTHOR_EMAIL".into(), op_email.clone()),
        ("GIT_COMMITTER_NAME".into(), op_name.clone()),
        ("GIT_COMMITTER_EMAIL".into(), op_email.clone()),
    ];
    let source = core::git_out(&repo, &["rev-parse", "HEAD"])?;
    let tag = branch::_FX_TAG;
    let variant = |n: i64| tag.replace("len262144", &format!("len{n}"));
    let fxb = branch::FX_BRIEF;
    let commit_new = |r: &Path, t: &str, pdir: &Path, brief: &str, extra: Option<&[(String, String)]>, sa: Option<&str>| -> HR<String> {
        branch::write_fixture_payload(pdir, t, sa, extra, None, brief)?;
        let g = branch::selftest_guide(r, Some(branch::BRANCH_FORMAT), None, branch::FX_GUIDE_BODY, None)?;
        branch::commit_payload(r, pdir, &branch::commit_message(pdir, u)?, u, Some(&g), None)
    };
    let guide = branch::selftest_guide(&repo, Some(branch::BRANCH_FORMAT), None, branch::FX_GUIDE_BODY, None)?;
    let epoch = core::parse_utc(u)?.epoch();
    let ident = branch::synthetic_identity()?;

    let (anchor, msg, obj);
    {
        let _g = branch::selftest_env(tmp, &op_env)?;
        anchor = commit_new(&repo, tag, &tmp.join("p1"), fxb, None, Some(&source))?;
        msg = annotation(fxb, &_fx_fields(tag, &anchor))?;
        c.ck(
            "★footer.tag ≠ 봉인 대상 = TAG_MISMATCH",
            code(seal(&repo, tag, &anchor, &annotation(fxb, &_fx_fields(&variant(1024), &anchor))?, u, None)).as_deref()
                == Some("HINT_EVIDENCE_BINDING_TAG_MISMATCH"),
        );
        c.ck(
            "★footer.anchor ≠ 봉인 앵커 = ANCHOR_MISMATCH",
            code(seal(&repo, tag, &anchor, &annotation(fxb, &_fx_fields(tag, &source))?, u, None)).as_deref()
                == Some("HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH"),
        );
        c.ck(
            "★소스 커밋에 봉인 = HINT_ANCHOR_PARENT_NOT_GUIDE(2026-09-07 native 3종 · §4.7)",
            code(seal(&repo, tag, &source, &annotation(fxb, &_fx_fields(tag, &source))?, u, None)).as_deref()
                == Some("HINT_ANCHOR_PARENT_NOT_GUIDE"),
        );
        c.ck(
            "★신규 봉인에 footer v1 = HINT_EVIDENCE_BINDING_VERSION(v1 은 읽기 전용)",
            code(seal(&repo, tag, &anchor, &annotation(fxb, &_fx_fields_v1(tag, &anchor))?, u, None)).as_deref()
                == Some("HINT_EVIDENCE_BINDING_VERSION"),
        );
        let refused = seal(&repo, tag, &anchor, &annotation("손으로 쓴 다른 요약", &_fx_fields(tag, &anchor))?, u, None).err();
        c.ck(
            "★brief 가 00-hint §0.1 추출과 다르면 봉인 거부",
            refused.as_ref().map_or(false, |e| {
                e.code == "HINT_SEAL_REFUSED"
                    && e.problems.as_ref().map_or(false, |p| problem_codes(p).unwrap_or_default().contains(&"HINT_ANNOTATION_BRIEF_MISMATCH".to_string()))
            }),
        );
        c.ck(
            "★정본 아닌 annotation(덧붙임) = HINT_ANNOTATION_SHAPE",
            code(seal(&repo, tag, &anchor, &format!("{msg}덧붙임\n"), u, None)).as_deref() == Some("HINT_ANNOTATION_SHAPE"),
        );
        c.ck(
            "★옛 문법 이름은 봉인하지 않는다",
            code(seal(&repo, "hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8", &anchor, &msg, u, None)).as_deref()
                == Some("HINT_ARCH_NODE_AXIS_ABSENT"),
        );
        c.ck("★비-UTC 시각 = HINT_TIME_NOT_INJECTED", code(seal(&repo, tag, &anchor, &msg, "2026-09-21 10:21", None)).as_deref() == Some("HINT_TIME_NOT_INJECTED"));
        c.ck("거부들 뒤 태그 없음(부작용 0)", tag_ref_kind(&repo, tag)?.is_none());
        obj = seal(&repo, tag, &anchor, &msg, u, None)?;
        c.ck("멱등 재봉인 = 같은 오브젝트", seal(&repo, tag, &anchor, &msg, u, None).ok().as_deref() == Some(obj.as_str()));
    }

    let rt = read_tag(&repo, tag)?.unwrap_or_else(J::dict);
    c.ck("annotated 태그가 생겼다", tag_ref_kind(&repo, tag)?.as_deref() == Some("tag") && rt.get_str("object_sha") == Some(obj.as_str()));
    c.ck("★본문 바이트 = 봉인 메시지(끝 개행까지 · 2026-09-22 사후조건 결함 회귀)", rt.get_str("body") == Some(msg.as_str()));
    c.ck("태그 대상 = 페이로드 커밋", rt.get_str("target") == Some(anchor.as_str()) && rt.get_str("target_type") == Some("commit"));
    let tg = &rt["tagger"];
    c.ck(
        "tagger = 합성 신원 · 시각 = 주입",
        tg.get_str("ident") == Some(ident.as_str()) && tg["epoch"].as_i64() == Some(epoch) && tg.get_str("tz") == Some("+0000"),
    );
    let raw_commit = core::git_bytes(&repo, &["cat-file", "commit", &anchor], true)?.unwrap_or_default();
    let raw_tag = tag_raw_bytes(&rt);
    let has = |hay: &[u8], n: &str| hay.windows(n.len().max(1)).any(|w| w == n.as_bytes());
    c.ck(
        "★운영자 신원이 태그·커밋 오브젝트 어디에도 없다(config·env 에 심었는데도)",
        [&raw_tag, &raw_commit].iter().all(|b| !has(b, &op_name) && !has(b, &op_email)),
    );
    c.ck("verify_local = 0", verify_local(&repo, tag, None, None, None)?.is_empty());
    c.ck(
        "★부재 태그 = HINT_TAG_ABSENT",
        problem_codes(&verify_local(&repo, &variant(2048), None, None, None)?)? == ["HINT_TAG_ABSENT"],
    );

    // verbatim
    let tag_v = variant(8192);
    let msg_v = "## 헤딩처럼 보이는 줄\n# 주석처럼 보이는 줄\n\n\n줄끝 공백   \n  \n끝\n\n";
    let sha_v = _create_tag(&repo, &tag_v, &anchor, msg_v, u)?;
    c.ck(
        "★verbatim: `#` 줄·빈 줄·공백이 본문에 바이트 그대로 남는다",
        core::git_bytes(&repo, &["cat-file", "tag", &sha_v], true)? == Some(_tag_object_bytes(&anchor, &tag_v, u, msg_v)?),
    );
    core::git(&repo, &["update-ref", "-d", &tref(&tag_v), &sha_v])?;

    // 결정론
    let repo2 = tmp.join("repo2");
    std::fs::create_dir_all(&repo2).map_err(|e| core::io_err(&e, &repo2))?;
    core::git(&repo2, &["init", "-q", "-b", "main"])?;
    let terms2 = repo2.join(core::REL_PII_TERMS);
    std::fs::create_dir_all(terms2.parent().unwrap()).map_err(|e| core::io_err(&e, &terms2))?;
    std::fs::write(&terms2, format!("{}\n", branch::FIXTURE_TERM)).map_err(|e| core::io_err(&e, &terms2))?;
    let anchor2 = commit_new(&repo2, tag, &tmp.join("p2"), fxb, None, Some(&source))?;
    c.ck(
        "★결정론: 태그 SHA 가 저장소·운영자와 무관하다",
        anchor2 == anchor && seal(&repo2, tag, &anchor2, &msg, u, None).ok().as_deref() == Some(obj.as_str()),
    );

    // X13
    c.ck(
        "★같은 앵커·다른 brief = HINT_NAME_COLLISION",
        code(seal(&repo, tag, &anchor, &annotation("다른 요약", &_fx_fields(tag, &anchor))?, u, None)).as_deref() == Some("HINT_NAME_COLLISION"),
    );
    c.ck(
        "같은 앵커·같은 annotation·다른 봉인 시각 = 재개(기존 오브젝트 유지 · 끊긴 continue 재실행)",
        seal(&repo, tag, &anchor, &msg, _FX_UTC2, None).ok().as_deref() == Some(obj.as_str())
            && _ref_sha(&repo, &tref(tag))?.as_deref() == Some(obj.as_str()),
    );
    let mut data = format!("object {anchor}\ntype commit\ntag {tag}\ntagger {op_name} <{op_email}> {epoch} +0000\n\n").into_bytes();
    data.extend_from_slice(msg.as_bytes());
    let t_op = py_strip(&String::from_utf8_lossy(&_git_b(&repo, &["mktag"], Some(&data), &[])?.stdout)).to_string();
    core::git(&repo, &["update-ref", &tref(tag), &t_op, &obj])?;
    let r = code(seal(&repo, tag, &anchor, &msg, u, None));
    core::git(&repo, &["update-ref", &tref(tag), &obj, &t_op])?;
    c.ck("★같은 앵커·같은 본문이라도 tagger 가 합성 신원이 아니면 재개가 아니다 = HINT_NAME_COLLISION", r.as_deref() == Some("HINT_NAME_COLLISION"));
    let extra_b = pairs(&[("artifacts/triplet/extra.yaml", "k: v\n")]);
    let anchor_b = commit_new(&repo, tag, &tmp.join("pb"), fxb, Some(&extra_b), Some(&source))?;
    c.ck(
        "★다른 앵커(같은 태그의 다른 페이로드 커밋) = HINT_NAME_COLLISION",
        code(seal(&repo, tag, &anchor_b, &annotation(fxb, &_fx_fields(tag, &anchor_b))?, u, None)).as_deref() == Some("HINT_NAME_COLLISION"),
    );
    c.ck(
        "★로컬 hint 브랜치 상태와 무관 — 부재여도 verify_local = 0(§4.7)",
        branch::hint_tip(&repo)?.is_none() && verify_local(&repo, tag, None, None, None)?.is_empty(),
    );

    // lint_fn 주입구
    let seen: RefCell<Option<(std::path::PathBuf, String, String)>> = RefCell::new(None);
    let lint_ok = |r: &Path, a: &str, t: &str| -> HR<J> {
        *seen.borrow_mut() = Some((r.to_path_buf(), a.to_string(), t.to_string()));
        Ok(crate::jl![jd! {"code" => "HINT_LINT_FIXTURE", "message" => "픽스처 발견"}])
    };
    let lint_boom = |_r: &Path, _a: &str, _t: &str| -> HR<J> { core::fail("HINT_LINT_BOOM", "린터 자체 실패", None) };
    c.ck(
        "lint_fn 발견 = 문제(code 보존)",
        problem_codes(&verify_local(&repo, tag, Some(&lint_ok), None, None)?)?.contains(&"HINT_LINT_FIXTURE".to_string()),
    );
    c.ck(
        "lint_fn 인자 = repo·anchor·tag",
        seen.borrow().as_ref().map_or(false, |(r, a, t)| *a == anchor && t == tag && r == &repo),
    );
    c.ck(
        "★lint_fn 의 HintError 도 문제로 남는다",
        problem_codes(&verify_local(&repo, tag, Some(&lint_boom), None, None)?)?.contains(&"HINT_LINT_BOOM".to_string()),
    );

    // 이름·ref 형식
    c.ck("name_collision 로컬", name_collision(&repo, tag, None)?.as_deref() == Some("local"));
    c.ck("name_collision 없음", name_collision(&repo, &variant(4096), None)?.is_none());
    c.ck("★D/F 접두 충돌", name_collision(&repo, &format!("{tag}/x"), None)?.as_deref() == Some("local-prefix"));
    for (badname, want) in [
        ("other/x", "HINT_TAG_NAMESPACE"),
        ("refs/heads/hint", "HINT_TAG_NAMESPACE"),
        ("refs/tags/hint/x", "HINT_TAG_NAMESPACE"),
        ("hint/*", "HINT_TAG_GLOB_FORBIDDEN"),
        ("hint/a?b", "HINT_TAG_GLOB_FORBIDDEN"),
        ("hint/[x]", "HINT_TAG_GLOB_FORBIDDEN"),
        ("hint/a b", "HINT_TAG_REF_FORMAT"),
        ("hint/a:b", "HINT_TAG_REF_FORMAT"),
        ("hint/a..b", "HINT_TAG_REF_FORMAT"),
        ("hint/x.lock", "HINT_TAG_REF_FORMAT"),
        ("", "HINT_TAG_NAME_ABSENT"),
    ] {
        c.ck(&format!("★ref 형식 거부 {} = {want}", py_repr(badname)), code(check_ref_format(&repo, badname)).as_deref() == Some(want));
    }

    // push: 로컬 bare 원격
    let bare = tmp.join("remote.git");
    let bare_s = s(&bare);
    core::git(tmp, &["init", "-q", "--bare", &bare_s])?;
    spy_start();
    let dry = push_tag(&repo, &bare_s, tag, true, None, None, None);
    let dry_ok = dry.as_ref().map_or(false, |d| d.get_str("status") == Some("dry-run")) && remote_tag_object(&repo, &bare_s, tag)?.is_none();
    let res = push_tag(&repo, &bare_s, tag, false, None, None, None)?;
    let calls = spy_take();
    c.ck("dry-run 은 원격을 바꾸지 않는다", dry_ok);
    let argv_of = |cl: &J| -> Vec<String> { cl["argv"].items().iter().map(J::display).collect() };
    let pushes: Vec<Vec<String>> = calls.iter().map(argv_of).filter(|a| a.iter().any(|x| x == "push")).collect();
    let want_spec = format!("refs/tags/{tag}:refs/tags/{tag}");
    c.ck(
        "push 2회(dry-run·실제) 모두 정확한 refspec 1개",
        pushes.len() == 2
            && pushes.iter().all(|p| p.iter().filter(|a| **a == want_spec).count() == 1 && !p.iter().any(|a| a.starts_with("refs/") && *a != want_spec)),
    );
    c.ck(
        "★push argv 에 --tags·refs/heads 없음",
        pushes.iter().all(|p| !p.iter().any(|a| a == "--tags") && !p.iter().any(|a| a.contains("refs/heads"))),
    );
    c.ck(
        "★push argv 가 설정의 태그 딸려보내기를 끈다(--no-follow-tags)",
        !pushes.is_empty() && pushes.iter().all(|p| PUSH_SCOPE_ARGS.iter().all(|o| p.iter().any(|a| a == o))),
    );
    c.ck(
        "pushed · 원격 오브젝트 = 로컬 오브젝트",
        res.get_str("status") == Some("pushed") && res.get_str("remote_object") == Some(obj.as_str())
            && remote_tag_object(&repo, &bare_s, tag)?.as_deref() == Some(obj.as_str()),
    );
    c.ck("원격 태그 바이트 = 로컬 태그 바이트", core::git_bytes(&bare, &["cat-file", "tag", &obj], true)? == Some(raw_tag.clone()));
    spy_start();
    let again = push_tag(&repo, &bare_s, tag, false, None, None, None)?;
    let calls = spy_take();
    c.ck(
        "재push = already-on-remote(push 생략 · 재개)",
        again.get_str("status") == Some("already-on-remote") && !calls.iter().any(|cl| argv_of(cl).iter().any(|a| a == "push")),
    );
    let repo3 = tmp.join("repo3");
    std::fs::create_dir_all(&repo3).map_err(|e| core::io_err(&e, &repo3))?;
    core::git(&repo3, &["init", "-q", "-b", "main"])?;
    c.ck("name_collision 원격", name_collision(&repo3, tag, Some(&bare_s))?.as_deref() == Some("remote"));
    c.ck("★name_collision 원격 D/F 접두", name_collision(&repo3, &format!("{tag}/x"), Some(&bare_s))?.as_deref() == Some("remote-prefix"));
    let absent = s(&tmp.join("absent.git"));
    c.ck("★원격 조회 실패는 '없음' 이 아니다", code(name_collision(&repo3, tag, Some(&absent))).as_deref() == Some("HINT_REMOTE_QUERY_FAILED"));

    // 원격에 같은 이름의 다른 오브젝트
    let repo4 = tmp.join("repo4");
    std::fs::create_dir_all(&repo4).map_err(|e| core::io_err(&e, &repo4))?;
    core::git(&repo4, &["init", "-q", "-b", "main"])?;
    let terms4 = repo4.join(core::REL_PII_TERMS);
    std::fs::create_dir_all(terms4.parent().unwrap()).map_err(|e| core::io_err(&e, &terms4))?;
    std::fs::write(&terms4, format!("{}\n", branch::FIXTURE_TERM)).map_err(|e| core::io_err(&e, &terms4))?;
    let b4 = "다른 PC 가 같은 셀을 다르게 요약했다.";
    let a4 = commit_new(&repo4, tag, &tmp.join("p4"), b4, None, Some(&source))?;
    let m4 = annotation(b4, &_fx_fields(tag, &a4))?;
    c.ck(
        "★봉인 직전 원격 재조회: 원격에 같은 이름 = HINT_NAME_COLLISION(막기만 · 개명 ✗ · 로컬 태그 0)",
        code(seal(&repo4, tag, &a4, &m4, u, Some(&bare_s))).as_deref() == Some("HINT_NAME_COLLISION") && tag_ref_kind(&repo4, tag)?.is_none(),
    );
    c.ck(
        "★봉인 직전 원격 조회 실패 = HINT_REMOTE_QUERY_FAILED(없음으로 접지 않는다)",
        code(seal(&repo4, tag, &a4, &m4, u, Some(&absent))).as_deref() == Some("HINT_REMOTE_QUERY_FAILED"),
    );
    let t_r = variant(1038);
    let a_r = commit_new(&repo4, &t_r, &tmp.join("p4r"), fxb, None, Some(&source))?;
    let m_r = annotation(fxb, &_fx_fields(&t_r, &a_r))?;
    let o_r = seal(&repo4, &t_r, &a_r, &m_r, u, Some(&bare_s))?;
    c.ck(
        "원격에 없으면 봉인 진행 · 재봉인(로컬 재개)은 원격 조회 전에 끝난다",
        tag_ref_kind(&repo4, &t_r)?.as_deref() == Some("tag") && seal(&repo4, &t_r, &a_r, &m_r, u, Some(&absent)).ok().as_deref() == Some(o_r.as_str()),
    );
    seal(&repo4, tag, &a4, &m4, u, None)?;
    c.ck(
        "★원격 동명 다른 오브젝트 = HINT_REMOTE_TAG_CONFLICT(강제 ✗)",
        code(push_tag(&repo4, &bare_s, tag, false, None, None, None)).as_deref() == Some("HINT_REMOTE_TAG_CONFLICT")
            && remote_tag_object(&repo, &bare_s, tag)?.as_deref() == Some(obj.as_str()),
    );

    // refspec·이름 규율
    c.ck("★push_tag glob = HINT_TAG_GLOB_FORBIDDEN", code(push_tag(&repo, &bare_s, "hint/*", false, None, None, None)).as_deref() == Some("HINT_TAG_GLOB_FORBIDDEN"));
    c.ck("★push_tag hint/ 밖 = HINT_TAG_NAMESPACE", code(push_tag(&repo, &bare_s, "other/x", false, None, None, None)).as_deref() == Some("HINT_TAG_NAMESPACE"));
    c.ck("★push_tag 브랜치 = HINT_TAG_NAMESPACE", code(push_tag(&repo, &bare_s, "refs/heads/hint", false, None, None, None)).as_deref() == Some("HINT_TAG_NAMESPACE"));
    for spec in [
        format!("+{want_spec}"),
        "refs/tags/hint/*:refs/tags/hint/*".to_string(),
        "--tags".to_string(),
        format!("refs/heads/hint:refs/tags/{tag}"),
        "refs/tags/a:refs/tags/a refs/tags/b:refs/tags/b".to_string(),
        format!("refs/tags/{tag}"),
    ] {
        c.ck(
            &format!("★git_push_authenticated refspec 거부 {}", py_repr(&core::head_chars(&spec, 28))),
            code(git_push_authenticated(&repo, &bare_s, &spec, true)).as_deref() == Some("HINT_PUSH_REFSPEC_FORBIDDEN"),
        );
    }
    c.ck(
        "★원격 인자 옵션 주입 거부",
        code(git_push_authenticated(&repo, "--mirror", &want_spec, true)).as_deref() == Some("HINT_PUSH_REMOTE_UNRESOLVED"),
    );
    let tag_lw = variant(512);
    core::git(&repo, &["tag", &tag_lw, &anchor])?;
    c.ck("★lightweight 태그는 밀지 않는다", code(push_tag(&repo, &bare_s, &tag_lw, false, None, None, None)).as_deref() == Some("HINT_TAG_NOT_ANNOTATED"));
    c.ck(
        "★lightweight 태그 verify = HINT_TAG_NOT_ANNOTATED",
        problem_codes(&verify_local(&repo, &tag_lw, None, None, None)?)? == ["HINT_TAG_NOT_ANNOTATED"],
    );
    c.ck(
        "★dry-run 도 실패를 삼키지 않는다(C-3)",
        code(git_push_authenticated(&repo, &absent, &want_spec, true)).as_deref() == Some("HINT_PUSH_FAILED"),
    );

    // 스킴 인식 자격증명
    let https_url = format!("https://{_FX_HOST}/r.git");
    core::git(&repo, &["remote", "add", "https-fixture", &https_url])?;
    core::git(&repo, &["config", &format!("url.{bare_s}.insteadOf"), &https_url])?;
    let tag_c = variant(16384);
    let a_c = commit_new(&repo, &tag_c, &tmp.join("pc"), fxb, None, Some(&source))?;
    seal(&repo, &tag_c, &a_c, &annotation(fxb, &_fx_fields(&tag_c, &a_c))?, u, None)?;
    c.ck("https 선언 원격은 토큰이 필요하다(insteadOf 전개 전 판정)", remote_needs_token(&repo, "https-fixture")?);
    let spec_c = format!("refs/tags/{tag_c}:refs/tags/{tag_c}");
    let absent_e = git_push_authenticated(&repo, "https-fixture", &spec_c, true).err();
    c.ck(
        "★https·토큰 없음 = HINT_PUSH_CREDENTIAL_ABSENT(메시지에 '자격증명')",
        absent_e.as_ref().map_or(false, |e| e.code == "HINT_PUSH_CREDENTIAL_ABSENT" && e.message.contains("자격증명")),
    );
    c.ck(
        "★push_tag 도 같은 거부 · 원격 불변",
        code(push_tag(&repo, "https-fixture", &tag_c, false, None, None, None)).as_deref() == Some("HINT_PUSH_CREDENTIAL_ABSENT")
            && remote_tag_object(&repo, &bare_s, &tag_c)?.is_none(),
    );
    let tok = _FX_TOKEN();
    let (res_c, proc_, calls) = {
        let _g = branch::selftest_env(tmp, &[(TOKEN_KEY.to_string(), tok.clone())])?;
        spy_start();
        let a = push_tag(&repo, "https-fixture", &tag_c, false, None, None, None);
        let b = git_push_authenticated(&repo, "https-fixture", &spec_c, false);
        (a, b, spy_take())
    };
    let res_c = res_c?;
    let proc_ = proc_?;
    c.ck(
        "토큰이 있으면 https 원격 push 성공",
        res_c.get_str("status") == Some("pushed")
            && remote_tag_object(&repo, &bare_s, &tag_c)? == read_tag(&repo, &tag_c)?.and_then(|t| t.get_str("object_sha").map(str::to_string)),
    );
    let pc: Vec<&J> = calls.iter().filter(|cl| argv_of(cl).iter().any(|a| a == "push")).collect();
    c.ck("★토큰 값이 어떤 git argv 에도 없다", !calls.is_empty() && !calls.iter().any(|cl| argv_of(cl).iter().any(|a| a.contains(&tok))));
    let cred = push_credential_args()?;
    c.ck(
        "토큰은 push 프로세스 env 로만 · 상속 helper 비움 · 프롬프트 ✗",
        !pc.is_empty()
            && pc.iter().all(|cl| {
                cl["env"].get_str(PUSH_TOKEN_ENV) == Some(tok.as_str())
                    && cl["env"].get_str("GIT_TERMINAL_PROMPT") == Some("0")
                    && argv_of(cl).get(1..5).map_or(false, |x| x == cred.as_slice())
            }),
    );
    c.ck("★remote.url 은 그대로(토큰 ✗)", core::git_out(&repo, &["config", "--get-all", "remote.https-fixture.url"])? == https_url);
    c.ck(
        "★push 출력에 토큰 없음",
        !format!("{}{}", proc_["stdout"].display(), proc_["stderr"].display()).contains(&tok),
    );
    let mut files = Vec::new();
    rglob(&repo.join(".git"), &mut files);
    rglob(&bare, &mut files);
    let leaked: Vec<_> = files.iter().filter(|p| std::fs::read(p).map_or(false, |b| has(&b, &tok))).collect();
    c.ck("★토큰이 reflog·설정·ref 어디에도 없다(두 저장소 전수)", leaked.is_empty());
    core::git(&repo, &["config", "credential.helper", "!f() { echo username=leak; echo password=LEAKED; }; f"])?;
    let probe = format!("protocol=https\nhost={_FX_HOST}\n\n");
    let env_t = pairs(&[(PUSH_TOKEN_ENV, tok.as_str()), ("GIT_TERMINAL_PROMPT", "0")]);
    let mut ours_args: Vec<&str> = cred.iter().map(String::as_str).collect();
    ours_args.extend(["credential", "fill"]);
    let ours = core::git_full(&repo, &ours_args, false, Some(&probe), &env_t)?;
    let ctrl = core::git_full(&repo, &["credential", "fill"], false, Some(&probe), &pairs(&[("GIT_TERMINAL_PROMPT", "0")]))?;
    c.ck(
        "★우리 helper 만 응답한다(상속 helper 비움)",
        ours.stdout.contains(&format!("password={tok}")) && !ours.stdout.contains("LEAKED"),
    );
    c.ck("대조: 비우지 않으면 상속 helper 가 응답한다(픽스처가 살아 있다)", ctrl.stdout.contains("LEAKED"));
    core::git(&repo, &["config", "--unset", "credential.helper"])?;
    core::git(&repo, &["remote", "add", "http-fixture", &format!("http://{_FX_HOST}/r.git")])?;
    c.ck(
        "★평문 http = HINT_PUSH_INSECURE_TRANSPORT",
        code(git_push_authenticated(&repo, "http-fixture", &want_spec, true)).as_deref() == Some("HINT_PUSH_INSECURE_TRANSPORT"),
    );
    core::git(&repo, &["remote", "add", "cred-fixture", &format!("https://user:secret{}{_FX_HOST}/r.git", "@")])?;
    c.ck(
        "★URL 에 자격증명이 박힌 원격 = 거부",
        code(git_push_authenticated(&repo, "cred-fixture", &want_spec, true)).as_deref() == Some("HINT_PUSH_URL_EMBEDS_CREDENTIAL"),
    );
    for (name, url) in [("ssh-fixture", format!("ssh://{_FX_HOST}/r.git")), ("scp-fixture", format!("git{}{_FX_HOST}:r.git", "@"))] {
        core::git(&repo, &["remote", "add", name, &url])?;
        core::git(&repo, &["config", "--add", &format!("url.{bare_s}.insteadOf"), &url])?;
        c.ck(&format!("{name}: 토큰 불요"), !remote_needs_token(&repo, name)?);
        c.ck(
            &format!("★{name}: 토큰 없이 push(dry-run) 가 돈다(옛 코드는 SSH 여도 토큰 부재로 죽었다 · B12)"),
            code(git_push_authenticated(&repo, name, &want_spec, true)).is_none(),
        );
    }

    // 자격증명 판정은 원격 조회보다 먼저
    let dead = format!("https://{_FX_HOST}/dead.git");
    core::git(&repo, &["remote", "add", "dead-https", &dead])?;
    core::git(&repo, &["config", "--add", &format!("url.{}.insteadOf", s(&tmp.join("no-such-remote.git"))), &dead])?;
    c.ck(
        "대조: 그 원격은 실제로 조회되지 않는다(픽스처가 살아 있다)",
        code(remote_tag_object(&repo, "dead-https", &tag_c)).as_deref() == Some("HINT_REMOTE_QUERY_FAILED"),
    );
    c.ck(
        "★https·토큰 없음 · 원격 조회 불가여도 push_tag = HINT_PUSH_CREDENTIAL_ABSENT(≠ REMOTE_QUERY_FAILED)",
        code(push_tag(&repo, "dead-https", &tag_c, true, None, None, None)).as_deref() == Some("HINT_PUSH_CREDENTIAL_ABSENT"),
    );

    // push.followTags
    core::git(&repo, &["tag", "-a", "-m", "local rollback anchor", "last-good-fixture", &guide])?;
    core::git(&repo, &["config", "push.followTags", "true"])?;
    let follow = (|| -> HR<()> {
        let (ctrl_bare, ft_bare) = (tmp.join("ctrl-follow.git"), tmp.join("follow.git"));
        for b in [&ctrl_bare, &ft_bare] {
            core::git(tmp, &["init", "-q", "--bare", &s(b)])?;
        }
        core::git(&repo, &["push", "-q", &s(&ctrl_bare), &spec_c])?;
        let ctrl_refs = core::git_out(&ctrl_bare, &["for-each-ref", "--format=%(refname)"])?;
        c.ck(
            "대조: 옵션 없는 git push 는 followTags 로 다른 태그까지 올린다(픽스처가 살아 있다)",
            ctrl_refs.lines().any(|l| l == "refs/tags/last-good-fixture"),
        );
        let ft = push_tag(&repo, &s(&ft_bare), &tag_c, false, None, None, None)?;
        let ft_refs: Vec<String> = core::git_out(&ft_bare, &["for-each-ref", "--format=%(refname)"])?.lines().map(str::to_string).collect();
        c.ck(
            "★followTags 설정에서도 push_tag 는 정확히 그 태그 1개만 올린다(last-good·옛 hint 태그 ✗)",
            ft.get_str("status") == Some("pushed") && ft_refs == [format!("refs/tags/{tag_c}")],
        );
        let br_bare = tmp.join("follow-branch.git");
        core::git(tmp, &["init", "-q", "--bare", &s(&br_bare)])?;
        branch::_update_hint_ref(&repo, &guide, None)?;
        git_push_authenticated(&repo, &s(&br_bare), &format!("{}:{}", core::HINT_BRANCH_REF, core::HINT_BRANCH_REF), false)?;
        let br_refs: Vec<String> = core::git_out(&br_bare, &["for-each-ref", "--format=%(refname)"])?.lines().map(str::to_string).collect();
        c.ck("★브랜치 push(공개 API · push_branches 경로)도 태그를 딸려 보내지 않는다", br_refs == [core::HINT_BRANCH_REF]);
        Ok(())
    })();
    core::git(&repo, &["config", "--unset", "push.followTags"])?;
    follow?;

    // 봉인 뒤 변조 → verify_local
    let mktag = |body: &str, tagger: Option<&str>, target: &str, name: &str| -> HR<String> {
        let who = tagger.map(str::to_string).unwrap_or_else(|| ident.clone());
        let mut d = format!("object {target}\ntype commit\ntag {name}\ntagger {who} {epoch} +0000\n\n").into_bytes();
        d.extend_from_slice(body.as_bytes());
        Ok(py_strip(&String::from_utf8_lossy(&_git_b(&repo, &["mktag"], Some(&d), &[])?.stdout)).to_string())
    };
    let f_ok = _fx_fields(tag, &anchor);
    let op_who = format!("{op_name} <{op_email}>");
    let tampers: Vec<(&str, String, Option<&str>, &str)> = vec![
        ("footer.anchor", annotation(fxb, &with(&f_ok, "anchor", source.as_str()))?, None, "HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH"),
        ("footer.tag", annotation(fxb, &with(&f_ok, "tag", variant(1024)))?, None, "HINT_EVIDENCE_BINDING_TAG_MISMATCH"),
        ("footer 삭제", format!("{fxb}\n\n{ANNOTATION_POINTER}\n"), None, "HINT_EVIDENCE_BINDING_MISSING"),
        ("중복 키", msg.replacen("topology: ", "topology: x\ntopology: ", 1), None, "HINT_EVIDENCE_BINDING_MALFORMED"),
        ("폐기 키", msg.replace("bench_ref:", &format!("manifest_sha256: {}\nbench_ref:", "0".repeat(64))), None, "HINT_EVIDENCE_BINDING_MALFORMED"),
        ("뒤꼬리", format!("{msg}덧붙임\n"), None, "HINT_ANNOTATION_SHAPE"),
        ("brief 변조", annotation("변조된 요약", &f_ok)?, None, "HINT_ANNOTATION_BRIEF_MISMATCH"),
        ("brief PII", annotation(&format!("요약 {}", branch::FIXTURE_TERM), &f_ok)?, None, "HINT_TAG_PII"),
        ("운영자 tagger", msg.clone(), Some(op_who.as_str()), "HINT_TAGGER_NOT_SYNTHETIC"),
    ];
    for (label, body, who, want) in &tampers {
        let t_sha = mktag(body, *who, &anchor, tag)?;
        core::git(&repo, &["update-ref", &tref(tag), &t_sha, &obj])?;
        let res = (|| -> HR<()> {
            let codes = problem_codes(&verify_local(&repo, tag, None, None, None)?)?;
            c.ck(&format!("★변조({label}) → {want}"), codes.iter().any(|x| x == want));
            if *label == "footer.anchor" {
                c.ck(
                    "★변조된 태그는 push 되지 않는다",
                    code(push_tag(&repo, &s(&tmp.join("other.git")), tag, false, None, None, None)).as_deref() == Some("HINT_PUSH_UNVERIFIED"),
                );
            }
            Ok(())
        })();
        core::git(&repo, &["update-ref", &tref(tag), &obj, &t_sha])?;
        res?;
    }
    c.ck("변조 복원 뒤 verify_local = 0", verify_local(&repo, tag, None, None, None)?.is_empty());

    // 하위 판정의 HintError 는 문제 목록으로
    let tp = repo.join(core::REL_PII_TERMS);
    let moved = tmp.join("terms.moved");
    std::fs::rename(&tp, &moved).map_err(|e| core::io_err(&e, &tp))?;
    std::fs::create_dir(&tp).map_err(|e| core::io_err(&e, &tp))?;
    let vr = verify_local(&repo, tag, None, None, None);
    let _ = std::fs::remove_dir(&tp);
    std::fs::rename(&moved, &tp).map_err(|e| core::io_err(&e, &tp))?;
    c.ck(
        "★깨진 pii_terms(디렉터리) → verify 결과에 HINT_PII_TERMS_UNREADABLE(예외 ✗)",
        vr.map_or(false, |v| problem_codes(&v).unwrap_or_default().iter().any(|x| x == "HINT_PII_TERMS_UNREADABLE")),
    );

    // 안내 밖 페이로드 커밋
    let t_off = variant(1029);
    let p_off = tmp.join("p-off");
    branch::write_fixture_payload(&p_off, &t_off, Some(&source), None, None, fxb)?;
    let off_tree = branch::build_tree(&repo, &p_off, &branch::payload_files(&p_off)?)?;
    let id_env = branch::identity_env_pairs(u)?;
    let c_off = core::git_full(&repo, &["commit-tree", &off_tree, "--no-gpg-sign"], true, Some(&branch::commit_message(&p_off, u)?), &id_env)?
        .stdout
        .trim()
        .to_string();
    let m_off = annotation(fxb, &_fx_fields(&t_off, &c_off))?;
    c.ck(
        "★안내 밖 페이로드 커밋 봉인 = HINT_ANCHOR_PARENT_NOT_GUIDE",
        code(seal(&repo, &t_off, &c_off, &m_off, u, None)).as_deref() == Some("HINT_ANCHOR_PARENT_NOT_GUIDE") && tag_ref_kind(&repo, &t_off)?.is_none(),
    );
    let off_sha = mktag(&m_off, None, &c_off, &t_off)?;
    core::git(&repo, &["update-ref", &tref(&t_off), &off_sha, &"0".repeat(40)])?;
    c.ck(
        "★안내 밖 페이로드 커밋 태그 verify = HINT_ANCHOR_PARENT_NOT_GUIDE",
        problem_codes(&verify_local(&repo, &t_off, None, None, None)?)? == ["HINT_ANCHOR_PARENT_NOT_GUIDE"],
    );

    // blob 이 없는 커밋
    let t_miss = variant(1028);
    let m_tree = core::git_full(&repo, &["mktree", "--missing"], true, Some(&format!("100644 blob {}\tPAYLOAD.json\n", "1".repeat(40))), &[])?
        .stdout
        .trim()
        .to_string();
    let c_miss = core::git_full(&repo, &["commit-tree", &m_tree, "--no-gpg-sign"], true, Some(&format!("hint: {t_miss}\n")), &id_env)?
        .stdout
        .trim()
        .to_string();
    let miss_sha = mktag(&annotation(fxb, &_fx_fields(&t_miss, &c_miss))?, None, &c_miss, &t_miss)?;
    core::git(&repo, &["update-ref", &tref(&t_miss), &miss_sha, &"0".repeat(40)])?;
    let vm = verify_local(&repo, &t_miss, None, None, None);
    c.ck(
        "★누락 blob → verify 결과에 HINT_TREE_READ_FAILED(예외 ✗ · 통과 ✗)",
        vm.map_or(false, |v| problem_codes(&v).unwrap_or_default().iter().any(|x| x == "HINT_TREE_READ_FAILED")),
    );

    // 도구를 우회한 페이로드 커밋
    let plumb = |t: &str, pdir: &Path, extra: Option<&[(String, String)]>, env: Option<&[(String, String)]>, outside: bool| -> HR<String> {
        branch::write_fixture_payload(pdir, t, None, extra, None, fxb)?;
        let mut all = Vec::new();
        rglob(pdir, &mut all);
        let mut rels: Vec<String> = all.iter().map(|p| s(p.strip_prefix(pdir).unwrap_or(p))).collect();
        rels.sort();
        let mut tree = branch::build_tree(&repo, pdir, &rels)?;
        if outside {
            let blob = core::git_full(&repo, &["hash-object", "-w", "--stdin"], true, Some("메모\n"), &[])?.stdout.trim().to_string();
            let lines = core::git(&repo, &["ls-tree", &tree])?.stdout;
            tree = core::git_full(&repo, &["mktree"], true, Some(&format!("{lines}100644 blob {blob}\tnotes.txt\n")), &[])?.stdout.trim().to_string();
        }
        let env = env.map(|e| e.to_vec()).unwrap_or_else(|| id_env.clone());
        Ok(core::git_full(&repo, &["commit-tree", &tree, "-p", &guide, "--no-gpg-sign"], true, Some(&branch::commit_message(pdir, u)?), &env)?
            .stdout
            .trim()
            .to_string())
    };
    let mut op_commit_env = op_env.clone();
    op_commit_env.push(("GIT_AUTHOR_DATE".into(), core::git_date(u)?));
    op_commit_env.push(("GIT_COMMITTER_DATE".into(), core::git_date(u)?));
    let pii_extra = pairs(&[("02-narrative.md", &format!("메모 {}\n", branch::FIXTURE_TERM))]);
    let prompt_extra = pairs(&[("02-narrative.md", "<!-- PROMPT\n질문\n-->\n")]);
    let plumbs: Vec<(i64, &str, Option<&[(String, String)]>, Option<&[(String, String)]>, bool, &str)> = vec![
        (1030, "allowlist 밖 파일", None, None, true, "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"),
        (1031, "페이로드 PII", Some(&pii_extra), None, false, "HINT_PAYLOAD_PII"),
        (1032, "운영자 신원 커밋", None, Some(&op_commit_env), false, "HINT_COMMIT_IDENTITY_NOT_SYNTHETIC"),
        (1033, "PROMPT 잔재", Some(&prompt_extra), None, false, "HINT_PROMPT_RESIDUE"),
    ];
    for (n, label, extra, env, outside, want) in plumbs {
        let tb = variant(n);
        let cb = plumb(&tb, &tmp.join(format!("pp{n}")), extra, env, outside)?;
        let mb = annotation(fxb, &_fx_fields(&tb, &cb))?;
        let err = seal(&repo, &tb, &cb, &mb, u, None).err();
        c.ck(
            &format!("★봉인 거부({label}) → {want}"),
            err.as_ref().map_or(false, |e| {
                e.code == "HINT_SEAL_REFUSED" && e.problems.as_ref().map_or(false, |p| problem_codes(p).unwrap_or_default().iter().any(|x| x == want))
            }) && tag_ref_kind(&repo, &tb)?.is_none(),
        );
        let t_sha = mktag(&mb, None, &cb, &tb)?;
        core::git(&repo, &["update-ref", &tref(&tb), &t_sha, &"0".repeat(40)])?;
        c.ck(
            &format!("★verify 검출({label}) → {want}"),
            problem_codes(&verify_local(&repo, &tb, None, None, None)?)?.iter().any(|x| x == want),
        );
    }

    // footer v1 읽기 호환
    let t_v1 = variant(1034);
    let a_v1 = commit_new(&repo, &t_v1, &tmp.join("pv1"), fxb, None, Some(&source))?;
    let v1_sha = mktag(&annotation(fxb, &_fx_fields_v1(&t_v1, &a_v1))?, None, &a_v1, &t_v1)?;
    core::git(&repo, &["update-ref", &tref(&t_v1), &v1_sha, &"0".repeat(40)])?;
    c.ck("★footer v1 태그 verify = 0(읽기 호환 · 판정은 구조만)", verify_local(&repo, &t_v1, None, None, None)?.is_empty());

    // 봉인 뒤 재현성
    let src_rel = "docs/devlog/devlog_fixture_source.md";
    let src = repo.join(src_rel);
    std::fs::create_dir_all(src.parent().unwrap()).map_err(|e| core::io_err(&e, &src))?;
    std::fs::write(&src, "봉인 시점 출처 본문\n").map_err(|e| core::io_err(&e, &src))?;
    let snap = vec![jd! {"path" => src_rel, "sha256" => core::sha256_file(&src)?}];
    let lineage = core::dumps(&jd! {"schema_version" => 1i64, "documents" => J::list(), SEALED_SOURCES_KEY => J::List(snap.clone())});
    let t_s = variant(1035);
    let extra_s = pairs(&[("LINEAGE.json", &lineage)]);
    let a_s = commit_new(&repo, &t_s, &tmp.join("ps"), fxb, Some(&extra_s), Some(&source))?;
    let m_s = annotation(fxb, &_fx_fields(&t_s, &a_s))?;
    seal(&repo, &t_s, &a_s, &m_s, u, None)?;
    c.ck("sealed_sources 판독", sealed_sources(&repo, &a_s)? == (Some(snap.clone()), vec![]));
    let lint_src = |_r: &Path, _a: &str, _t: &str| -> HR<J> {
        Ok(crate::jl![
            jd! {"code" => "HINT_EXCERPT_MISMATCH", "message" => "발췌가 출처와 다르다"},
            jd! {"code" => "HINT_SIGNATURE_MISMATCH", "message" => "서명 M3 불일치"}
        ])
    };
    let lint_struct = |_r: &Path, _a: &str, _t: &str| -> HR<J> { Ok(crate::jl![jd! {"code" => "HINT_PROMPT_TAMPERED", "message" => "PROMPT 변조"}]) };
    let has_codes = |v: &[String], want: &[&str]| -> bool {
        let codes = problem_codes(v).unwrap_or_default();
        want.iter().all(|w| codes.iter().any(|x| x == w))
    };
    let mut inf: Vec<J> = Vec::new();
    let v = verify_local(&repo, &t_s, Some(&lint_src), Some(&mut inf), None)?;
    c.ck(
        "대조: 출처 그대로 · 출처 의존 발견 = FAIL(시간이 만든 것이 아니다)",
        has_codes(&v, &["HINT_EXCERPT_MISMATCH", "HINT_SIGNATURE_MISMATCH"]) && inf.is_empty(),
    );
    std::fs::write(&src, "봉인 13초 뒤 정정된 출처 본문\n").map_err(|e| core::io_err(&e, &src))?;
    let mut inf: Vec<J> = Vec::new();
    let v = verify_local(&repo, &t_s, Some(&lint_src), Some(&mut inf), None)?;
    c.ck(
        "★봉인 뒤 출처 변경 = verify PASS · INFO 2건(FAIL ✗)",
        v.is_empty() && inf.len() == 2 && inf.iter().all(|x| x.display().starts_with(INFO_SOURCE_CHANGED) && x.display().contains(src_rel)),
    );
    c.ck("INFO 에 원 code 보존", inf.iter().any(|x| x.display().contains("HINT_SIGNATURE_MISMATCH")));
    c.ck("★infos=None 이어도 FAIL 로 되돌아가지 않는다(stderr 기록)", verify_local(&repo, &t_s, Some(&lint_src), None, None)?.is_empty());
    let mut sink: Vec<J> = Vec::new();
    c.ck(
        "★출처가 바뀌어도 출처 비의존 린트는 FAIL",
        problem_codes(&verify_local(&repo, &t_s, Some(&lint_struct), Some(&mut sink), None)?)? == ["HINT_PROMPT_TAMPERED"],
    );
    let src_bare = tmp.join("src-remote.git");
    core::git(tmp, &["init", "-q", "--bare", &s(&src_bare)])?;
    let mut inf: Vec<J> = Vec::new();
    let pr = push_tag(&repo, &s(&src_bare), &t_s, true, Some(&lint_src), Some(&mut inf), None)?;
    c.ck("★push_tag 도 같은 강등(infos 전달 · verify 통과 → dry-run)", pr.get_str("status") == Some("dry-run") && inf.len() == 2);
    std::fs::remove_file(&src).map_err(|e| core::io_err(&e, &src))?;
    let mut inf: Vec<J> = Vec::new();
    let v = verify_local(&repo, &t_s, Some(&lint_src), Some(&mut inf), None)?;
    c.ck("★출처 삭제도 '바뀐 출처'(부재 ≠ 일치)", v.is_empty() && inf.len() == 2);
    let obj_s = _ref_sha(&repo, &tref(&t_s))?.unwrap_or_default();
    let t_sha = mktag(&annotation(fxb, &with(&_fx_fields(&t_s, &a_s), "anchor", source.as_str()))?, None, &a_s, &t_s)?;
    core::git(&repo, &["update-ref", &tref(&t_s), &t_sha, &obj_s])?;
    let mut sink: Vec<J> = Vec::new();
    let vv = verify_local(&repo, &t_s, Some(&lint_src), Some(&mut sink), None);
    core::git(&repo, &["update-ref", &tref(&t_s), &obj_s, &t_sha])?;
    c.ck("★출처가 바뀌었어도 구조 결함(footer.anchor 변조)은 FAIL", has_codes(&vv?, &["HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH"]));
    let mut inf: Vec<J> = Vec::new();
    let v = verify_local(&repo, tag, Some(&lint_src), Some(&mut inf), None)?;
    c.ck("★스냅샷 없는 태그(v6 이하)는 강등하지 않는다(판정 불가 ≠ 출처 변경)", has_codes(&v, &["HINT_EXCERPT_MISMATCH"]) && inf.is_empty());
    let t_b = variant(1036);
    let bad_lineage = core::dumps(&jd! {"schema_version" => 1i64, "documents" => J::list(),
        SEALED_SOURCES_KEY => crate::jl![jd!{"path" => format!("/{}", "etc/x"), "sha256" => "0".repeat(64)}]});
    let extra_bad = pairs(&[("LINEAGE.json", &bad_lineage)]);
    let a_b = commit_new(&repo, &t_b, &tmp.join("pbad"), fxb, Some(&extra_bad), Some(&source))?;
    let err = seal(&repo, &t_b, &a_b, &annotation(fxb, &_fx_fields(&t_b, &a_b))?, u, None).err();
    c.ck(
        "★봉인 출처 스냅샷 모양 결함(절대경로) = 봉인 거부 HINT_SEALED_SOURCES_SHAPE",
        err.as_ref().map_or(false, |e| e.problems.as_ref().map_or(false, |p| problem_codes(p).unwrap_or_default().iter().any(|x| x == "HINT_SEALED_SOURCES_SHAPE"))),
    );
    c.ck(
        "SOURCE_DEPENDENT_LINT_CODES 에 구조 코드가 섞이지 않는다(tripwire)",
        !["HINT_PROMPT_TAMPERED", "HINT_PROMPT_RESIDUE", "HINT_TAG_PII", "HINT_PAYLOAD_PII", "HINT_ANCHOR_PARENT_NOT_GUIDE", "HINT_EVIDENCE_BINDING_MALFORMED"]
            .iter()
            .any(|x| SOURCE_DEPENDENT_LINT_CODES.contains(x)),
    );
    Ok(())
}

/// 자체검사용 spy 시작/회수(원본 `_spy_git`).
fn spy_start() {
    SPY.with(|s| *s.borrow_mut() = Some(Vec::new()));
}
fn spy_take() -> Vec<J> {
    SPY.with(|s| s.borrow_mut().take().unwrap_or_default())
}
