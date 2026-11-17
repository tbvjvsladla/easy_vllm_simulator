//! hintlib.catalog — 원격 발행 태그 → `hints/index.json` + `HINTS.md` 파생 · 수신자 `match`.
//! Port of `scripts/hintlib/catalog.py` (incl. `selftest`). The truth source is `git ls-remote`; the remote-advertised
//! tag object SHA is read directly (K5); a remote tag whose object is absent locally stops with
//! `EXIT_LOCAL_TAG_OBJECT_MISSING` unless `record_missing`; HINTS.md is rewritten only between its marker pair.
//!
//! Port notes: the Python lazy-import helpers `_hl`/`_core`/`_naming` (H5 file-path standalone load contract) have no
//! Rust counterpart — the constants are plain `pub const` and the sibling modules are ordinary crate paths. The H5
//! selftest probes (standalone interpreter load, AST import scan) are Python-module properties and are not ported.
#![allow(non_snake_case, non_upper_case_globals)]
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::io;
use crate::naming;
use crate::re_static;
use crate::value::{loads, py_repr, Map, J};
use crate::jd;

pub const EXIT_LOCAL_TAG_OBJECT_MISSING: i64 = 4;
pub const CENTRAL_FLAG: &str = "hints/.central_authority";
pub const HINTS_BEGIN_PREFIX: &str = "<!-- HINTS:BEGIN derived";
pub const HINTS_BEGIN: &str = "<!-- HINTS:BEGIN derived — `hint.sh catalog derive` 가 이 구간만 다시 쓴다(손으로 고치지 마라 · 구간 밖은 사람이 쓰는 서사다) -->";
pub const HINTS_END: &str = "<!-- HINTS:END -->";
pub const CATALOG_COLUMNS: [&str; 6] = ["태그", "arch", "vLLM", "판정", "bench_mode", "결손"];
pub const INDEX_SCHEMA: i64 = 5;
pub const OBJECT_ABSENT_LOCAL: &str = "absent-local";
pub const OBJECT_NOT_ANNOTATED: &str = "not-annotated";
pub const BRIEF_UNRECEIVED: &str = "—";
pub const MISSING_UNDECLARED: &str = "미선언";
pub const BENCH_MODE_ABSENT: &str = "미기재";
pub const BENCH_MODE_UNDETERMINED: &str = "미확정";
pub const BENCH_MODE_UNRECEIVED: &str = "미수령";
pub const VERDICTS: [&str; 3] = ["PASS", "REFUTE", "OBSERVATION-ONLY"];
pub const VERDICT_ABSENT: &str = "—";
pub const VERDICT_LEGACY_CELL: &str = "미기재(구판)";
pub const VERDICT_LITE: &str = "lite(기동 가능)";
pub const MODEL_UNKNOWN_GROUP: &str = "(모델 미상)";
re_static!(_KST_RE, r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}$");
re_static!(_REMOTE_NAME_RE, r"^[A-Za-z0-9][A-Za-z0-9._-]*$");
re_static!(_HTML_COMMENT, r"<!--.*?-->", crate::pyre::S);
re_static!(_PARA_SPLIT, r"\n[ \t]*\n");
re_static!(_HW_MULT, r"x\d+$");
pub const _OUT: &str = "[hint catalog]";

// ── Python str helpers (private) ───────────────────────────────────────────────────────────────
/// `str.isspace()` per character.
fn py_isspace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\x0b' | '\x0c' | '\r' | '\x1c' | '\x1d' | '\x1e' | '\x1f' | ' ' | '\u{85}') || (c > '\u{7f}' && c.is_whitespace())
}
/// `str.strip()`.
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}
/// `str.split()` (whitespace runs).
fn py_split_ws(s: &str) -> Vec<&str> {
    s.split(py_isspace).filter(|x| !x.is_empty()).collect()
}
/// `str.splitlines(keepends)`.
fn py_splitlines(s: &str, keepends: bool) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let is_sep = matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if !is_sep {
            continue;
        }
        let mut end = i + c.len_utf8();
        if c == '\r' {
            if let Some(&(_, '\n')) = it.peek() {
                it.next();
                end += 1;
            }
        }
        out.push(if keepends { &s[start..end] } else { &s[start..i] });
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}
fn chars_len(s: &str) -> usize {
    s.chars().count()
}
/// `d.get(k)` of a J that may not be a dict (`&J::Null` when absent).
fn jg<'a>(e: &'a J, k: &str) -> &'a J {
    e.get(k).unwrap_or(&crate::value::NULL)
}
/// `x or ""` rendered as `str()`.
fn or_empty(v: &J) -> String {
    if v.truthy() {
        v.display()
    } else {
        String::new()
    }
}

fn _fail<T>(code: &str, message: impl Into<String>, remedy: Option<&str>, exit_code: i64) -> HR<T> {
    core::fail_exit(code, message, remedy, exit_code as i32)
}


// ── 순수 헬퍼 ────────────────────────────────────────────────────────────────────────────────
pub fn extract_brief(body: &str) -> HR<String> {
    let text = _HTML_COMMENT().sub_literal(body, "");
    for raw in text.split('\n') {
        let line = py_strip(raw);
        if line.is_empty() {
            continue;
        }
        if ["<!--", ">", "#", "|", "-", "*", "```"].iter().any(|p| line.starts_with(p)) {
            continue;
        }
        if ["object ", "type ", "tag "].iter().any(|p| line.starts_with(p)) {
            continue;
        }
        return Ok(line.to_string());
    }
    Ok(String::new())
}

pub fn annotation_brief(body: &str) -> HR<String> {
    let sub = _HTML_COMMENT().sub_literal(body, "");
    let text = py_strip(&sub);
    for para in _PARA_SPLIT().split(text) {
        let lines: Vec<&str> = py_splitlines(&para, false).into_iter().map(py_strip).filter(|l| !l.is_empty()).collect();
        if !lines.is_empty() {
            return Ok(lines.join(" "));
        }
    }
    Ok(String::new())
}

pub fn md_cell(s: &J) -> HR<String> {
    let s = if s.is_null() { String::new() } else { s.display() };
    let s = s.replace('|', "\\|").replace('\n', " ").replace('\r', " ");
    let s = _HTML_COMMENT().sub_literal(&s, "").replace("<!--", "&lt;!--").replace("-->", "--&gt;");
    Ok(py_split_ws(&s).join(" "))
}
fn mdc(s: &J) -> String {
    md_cell(s).unwrap_or_default()
}

pub fn bench_mode_cell(doc: &J) -> HR<(String, String)> {
    if !doc.is_dict() {
        return Ok((BENCH_MODE_ABSENT.into(), "absent(PAYLOAD.json 을 읽지 못함 — 페이로드가 없는 태그)".into()));
    }
    let mc = jg(doc, "measurement_config");
    if !mc.is_dict() {
        return Ok((BENCH_MODE_ABSENT.into(), "absent(PAYLOAD.measurement_config 없음 — 측정 구성 기재 신설 전 페이로드)".into()));
    }
    let (mode, kind, reason) = (jg(mc, "bench_mode"), jg(mc, "bench_mode_kind"), jg(mc, "downgrade_reason"));
    let cell = if mode.as_str() == Some("full") {
        "full".to_string()
    } else if mode.as_str() == Some("lite") && reason.truthy() {
        format!("lite(강등·{})", reason.display())
    } else if mode.as_str() == Some("lite") && kind.as_str() == Some("declared-lite") {
        "lite(선언)".to_string()
    } else if mode.as_str() == Some("lite") {
        "lite".to_string()
    } else {
        BENCH_MODE_UNDETERMINED.to_string()
    };
    let src = jg(mc, "source");
    let src = if src.truthy() { src.display() } else { "출처 미표시".to_string() };
    Ok((cell, format!("payload(PAYLOAD.measurement_config · {src})")))
}

pub fn verdict_cell(doc: &J) -> HR<(String, String)> {
    if !doc.is_dict() {
        return Ok((VERDICT_ABSENT.into(), "absent(PAYLOAD.json 을 읽지 못함)".into()));
    }
    let m = jg(doc, "measurement");
    let v = if m.is_dict() { jg(m, "verdict") } else { &crate::value::NULL };
    let cg = jg(doc, "cell_grade");
    let lite = match cg.as_str() {
        Some(g) if crate::template::CELL_GRADES.contains(&g) => g == "lite",
        _ => m.is_dict() && m.get_str("benchmark_mode") == Some("lite") && crate::template::is_lite_cell(doc)?,
    };
    if lite {
        let grade_src = if cg.truthy() {
            "PAYLOAD.cell_grade"
        } else {
            "measurement.benchmark_mode/measurement_config(옛 페이로드 · 술어 파생)"
        };
        let vs = match v.as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => "—".to_string(),
        };
        return Ok((VERDICT_LITE.into(), format!("payload({grade_src} = lite · 기계 판정 measurement.verdict = {vs})")));
    }
    let vs = match v.as_str() {
        Some(s) if !py_strip(s).is_empty() => py_strip(s).to_string(),
        _ => return Ok((VERDICT_ABSENT.into(), "absent(PAYLOAD.measurement.verdict 없음 — 판정 기록 전 페이로드)".into())),
    };
    let per_key = jg(m, "sources");
    let pv = if per_key.is_dict() { jg(per_key, "verdict") } else { &crate::value::NULL };
    let origin = if pv.truthy() {
        pv.display()
    } else if jg(m, "source").truthy() {
        jg(m, "source").display()
    } else {
        "출처 미표시".to_string()
    };
    let src = format!("payload(PAYLOAD.measurement.verdict · {origin})");
    let cell = if VERDICTS.contains(&vs.as_str()) { vs } else { format!("{vs}(어휘 밖)") };
    Ok((cell, src))
}

re_static!(_TAGGER_RE, r"^tagger .* (\d+) [+-]\d{4}$");

pub fn tagger_utc(header: &str) -> HR<Option<String>> {
    for line in py_splitlines(header, false) {
        if let Some(m) = _TAGGER_RE().match_at_start(line) {
            let digits = crate::pyre::group(&m, 1).unwrap_or("");
            // int(): any Unicode decimal digit; fromtimestamp: year 1..9999 (else OverflowError/ValueError → None).
            let mut n: i128 = 0;
            for c in digits.chars() {
                let d = match c.to_digit(10) {
                    Some(d) => d as i128,
                    None => match unicode_decimal(c) {
                        Some(d) => d,
                        None => return Ok(None),
                    },
                };
                n = match n.checked_mul(10).and_then(|x| x.checked_add(d)) {
                    Some(x) => x,
                    None => return Ok(None),
                };
            }
            if n > 253_402_300_799 {
                return Ok(None);
            }
            return Ok(Some(format!("{}Z", core::Civil::from_epoch(n as i64).iso())));
        }
    }
    Ok(None)
}

/// Value of a non-ASCII Unicode decimal digit (Nd blocks are runs of 10 starting at a code point ending in 0 or 6).
fn unicode_decimal(c: char) -> Option<i128> {
    if !c.is_numeric() {
        return None;
    }
    // Nd ranges are contiguous 0..9 runs; find the zero by walking back while still numeric (≤ 9 steps).
    let cp = c as u32;
    let mut zero = cp;
    for _ in 0..9 {
        let prev = zero.checked_sub(1).and_then(char::from_u32)?;
        if prev.is_numeric() && (cp - (zero - 1)) <= 9 {
            zero -= 1;
        } else {
            break;
        }
    }
    Some((cp - zero) as i128)
}

pub fn query_slug(s: Option<&str>) -> HR<String> {
    let base = match s {
        Some(t) => {
            let t = py_strip(t).trim_end_matches('/');
            t.rsplit('/').next().unwrap_or("").to_string()
        }
        None => String::new(),
    };
    if base.is_empty() {
        Ok(String::new())
    } else {
        naming::norm_slug(&base)
    }
}
/// `query_slug(x)` for any value (`""` unless a str).
fn qs(v: &J) -> String {
    query_slug(v.as_str()).unwrap_or_default()
}

pub fn _remote_label(remote: &str) -> HR<String> {
    Ok(if _REMOTE_NAME_RE().is_match(remote) { remote.to_string() } else { "<경로·URL 원격 — 배포 파일에 적지 않는다>".to_string() })
}

// ── git 관측 ─────────────────────────────────────────────────────────────────────────────────
pub fn remote_hint_tags(repo: &Path, remote: &str, allow_empty: bool) -> HR<J> {
    if py_strip(remote).is_empty() || remote.starts_with('-') {
        return _fail(
            "HINT_CATALOG_REMOTE_INVALID",
            format!("원격 인자가 원격 이름·경로·URL 이 아니다: {}", py_repr(remote)),
            Some("`git remote -v` 의 이름(예 origin · github-ssh)을 넘긴다(`-` 로 시작하는 값은 git 옵션으로 해석된다)."),
            1,
        );
    }
    let p = core::git_nocheck(repo, &["ls-remote", "--tags", remote, "refs/tags/hint/*"])?;
    if p.returncode != 0 {
        return _fail(
            "HINT_CATALOG_REMOTE_QUERY_FAILED",
            format!("원격 태그 조회 실패({remote}) — 캐시로 대체하지 않는다(D1.1 fail-closed): {}", py_strip(&p.stderr)),
            Some("원격 이름·네트워크·자격을 확인하고 다시 실행한다. 낡은 카탈로그는 '기록이 원래 없었던 것'과 구분되지 않는다."),
            1,
        );
    }
    let mut tags: Map = Map::new();
    for line in py_splitlines(&p.stdout, false) {
        let Some((a, b)) = line.split_once('\t') else { continue };
        let (sha, rf) = (py_strip(a), py_strip(b));
        let Some(name) = rf.strip_prefix("refs/tags/") else { continue };
        if let Some(base) = name.strip_suffix("^{}") {
            tags.setdefault(base, J::dict()).set("peeled", sha);
        } else {
            tags.setdefault(name, J::dict()).set("object", sha);
        }
    }
    let mut names: Vec<String> = tags.keys().map(str::to_string).collect();
    names.sort();
    let mut out = Map::new();
    for name in names {
        let rec = tags.get(&name).unwrap();
        let Some(obj) = rec.get("object") else {
            return _fail("HINT_CATALOG_REMOTE_QUERY_FAILED", format!("원격 광고가 불완전하다(오브젝트 줄 없음): {name}"), None, 1);
        };
        out.insert(name.clone(), jd! {"object" => obj.clone(), "peeled" => rec.get("peeled").cloned().unwrap_or(J::Null)});
    }
    if out.is_empty() && !allow_empty {
        return _fail(
            "HINT_CATALOG_REMOTE_EMPTY",
            format!("원격 {remote} 에 hint 태그가 0건이다 — 빈 카탈로그를 발행하지 않는다."),
            Some("정말 0건이 맞다면(태그 전량 폐기 직후) allow_empty 로 명시한다."),
            1,
        );
    }
    Ok(J::Dict(out))
}

pub fn payload_doc(repo: &Path, obj: &str) -> HR<Option<J>> {
    let spec = format!("{obj}^{{}}:PAYLOAD.json");
    let Some(raw) = core::git_bytes(repo, &["cat-file", "-p", &spec], false)? else { return Ok(None) };
    let Ok(text) = String::from_utf8(raw) else { return Ok(None) };
    match loads(&text) {
        Ok(d @ J::Dict(_)) => Ok(Some(d)),
        _ => Ok(None),
    }
}

pub fn main_worktree(repo: &Path) -> HR<PathBuf> {
    let listing = core::git_nocheck(repo, &["worktree", "list", "--porcelain"])?;
    let first = if listing.returncode == 0 && !listing.stdout.is_empty() {
        py_splitlines(&listing.stdout, false).first().copied().unwrap_or("").to_string()
    } else {
        String::new()
    };
    Ok(match first.strip_prefix("worktree ") {
        Some(p) => PathBuf::from(p),
        None => repo.to_path_buf(),
    })
}

pub fn fetch_hint(repo: &Path, remote: Option<&str>) -> HR<String> {
    match remote {
        Some(r) if !r.is_empty() => Ok(format!(
            "git -C {} fetch {r} 'refs/tags/hint/*:refs/tags/hint/*'",
            core::posix(&main_worktree(repo)?)
        )),
        _ => Ok("git fetch <원격> 'refs/tags/hint/*:refs/tags/hint/*'".to_string()),
    }
}

// ── 파생 ─────────────────────────────────────────────────────────────────────────────────────
pub fn parse_name(name: &str) -> HR<J> {
    let grammar = match naming::parse_tag(name) {
        Ok(tn) => {
            return Ok(jd! {"grammar" => tn.grammar, "vllm" => tn.vllm, "model" => tn.model, "arch" => tn.arch, "recipe" => tn.recipe})
        }
        Err(e) if e.is_py() => return Err(e),
        Err(_) => naming::grammar_of(name)?,
    };
    let parts: Vec<&str> = name.split('/').collect();
    if grammar == naming::GRAMMAR_LEGACY_4SEG {
        return Ok(jd! {"grammar" => grammar, "vllm" => parts[1], "model" => parts[2], "arch" => parts[3], "recipe" => J::Null});
    }
    Ok(jd! {"grammar" => grammar, "vllm" => J::Null, "model" => J::Null, "arch" => J::Null, "recipe" => J::Null})
}

pub fn _payload_grammars() -> HR<Vec<String>> {
    Ok(vec![naming::GRAMMAR_V7.to_string(), naming::GRAMMAR_V6.to_string()])
}

fn in_grammars(parsed: &J, g: &[String]) -> bool {
    parsed.get_str("grammar").map(|s| g.iter().any(|x| x == s)).unwrap_or(false)
}

pub fn _base_row(name: &str, parsed: &J, payload_grammars: &[String]) -> HR<J> {
    let mut row = Map::new();
    row.insert("tag", name);
    if let Some(m) = parsed.as_dict() {
        row.update(m);
    }
    row.insert("published", true);
    row.insert("source", "remote-derived");
    if in_grammars(parsed, payload_grammars) {
        row.insert("base_model", J::Null);
    }
    Ok(J::Dict(row))
}

fn rec_object(rec: &J) -> HR<&J> {
    rec.get("object").ok_or_else(|| HintError::py("KeyError", "'object'"))
}

pub fn derive_entries(repo: &Path, remote_tags: &J, remote: Option<&str>, record_missing: bool) -> HR<(Vec<J>, J)> {
    let pay_g = _payload_grammars()?;
    let empty = Map::new();
    let tags = remote_tags.as_dict().unwrap_or(&empty);
    let mut parsed = Map::new();
    for name in tags.keys() {
        parsed.insert(name, parse_name(name)?);
    }
    let mut absent: Vec<String> = Vec::new();
    for (name, rec) in tags.iter() {
        let obj = rec_object(rec)?.display();
        if core::git_nocheck(repo, &["cat-file", "-e", &obj])?.returncode != 0 {
            absent.push(name.to_string());
        }
    }
    if !absent.is_empty() && !record_missing {
        let head: Vec<&str> = absent.iter().take(10).map(String::as_str).collect();
        return _fail(
            "HINT_CATALOG_LOCAL_OBJECT_MISSING",
            format!(
                "원격에 있으나 **로컬에 태그 오브젝트가 없다** — brief 를 지어낼 수 없다(합성 금지):\n  {}\n  (총 {}건)",
                head.join("\n  "),
                absent.len()
            ),
            Some(&format!(
                "`{}` 후 다시 실행하라(자동 fetch 하지 않는다). 부재를 행으로 기재하고 진행하려면 record-missing 모드(object: absent-local · brief '—')로 파생한다.",
                fetch_hint(repo, remote)?
            )),
            EXIT_LOCAL_TAG_OBJECT_MISSING,
        );
    }
    let mut entries: Vec<J> = Vec::new();
    let mut differs: Vec<String> = Vec::new();
    let mut not_annotated: Vec<String> = Vec::new();
    for (name, rec) in tags.iter() {
        let pn = parsed.get(name).unwrap();
        let mut row = _base_row(name, pn, &pay_g)?;
        let peeled = jg(rec, "peeled");
        if absent.iter().any(|a| a == name) {
            let anchor = if peeled.truthy() { peeled.clone() } else { rec_object(rec)?.clone() };
            for (k, v) in [
                ("brief", J::from(BRIEF_UNRECEIVED)),
                ("anchor", anchor),
                ("object", J::from(OBJECT_ABSENT_LOCAL)),
                ("missing", J::Null),
                ("bench_mode", J::from(BENCH_MODE_UNRECEIVED)),
                ("bench_mode_source", J::from("absent-local(로컬 태그 오브젝트 부재 — 조회한 원격에서 fetch 전 · 합성 ✗)")),
                ("verdict", J::from(BENCH_MODE_UNRECEIVED)),
                ("verdict_source", J::from("absent-local(로컬 태그 오브젝트 부재 · 합성 ✗)")),
                ("tagged_utc", J::Null),
            ] {
                row.set(k, v);
            }
            entries.push(row);
            continue;
        }
        let obj = rec_object(rec)?.display();
        let otype = core::git_out(repo, &["cat-file", "-t", &obj])?;
        if otype != "tag" && !record_missing {
            return _fail(
                "HINT_CATALOG_NOT_ANNOTATED",
                format!("{name} 가 annotated 태그가 아니다({otype}) — 본문이 없으면 brief 를 지어낼 수 없다."),
                Some("원격 태그는 교정하지 않는다(P1). 발행 경로 밖에서 만든 태그다 — 그 원격의 발행자에게 알린다. 행으로 기재하고 진행하려면 record-missing 모드(object: not-annotated · brief '—')로 파생한다."),
                1,
            );
        }
        let (anchor, brief, obj_cell, tagged): (J, String, String, Option<String>);
        if otype == "tag" {
            let raw = core::git_bytes(repo, &["cat-file", "tag", &obj], true)?.unwrap_or_default();
            let raw = String::from_utf8_lossy(&raw).into_owned();
            let (head, body) = match raw.find("\n\n") {
                Some(i) => (&raw[..i], &raw[i + 2..]),
                None => (raw.as_str(), ""),
            };
            tagged = tagger_utc(head)?;
            anchor = if peeled.truthy() { peeled.clone() } else { J::from(core::git_out(repo, &["rev-parse", &format!("{obj}^{{}}")])?) };
            brief = if in_grammars(pn, &pay_g) { annotation_brief(body)? } else { extract_brief(body)? };
            obj_cell = obj.clone();
        } else {
            not_annotated.push(name.to_string());
            anchor = J::from(obj.clone());
            brief = BRIEF_UNRECEIVED.into();
            obj_cell = OBJECT_NOT_ANNOTATED.into();
            tagged = None;
        }
        let doc = payload_doc(repo, &obj)?.unwrap_or(J::Null);
        let (bm, bm_src) = bench_mode_cell(&doc)?;
        let (vd, vd_src) = verdict_cell(&doc)?;
        let missing = match doc.get("missing") {
            Some(J::List(l)) => {
                let mut v: Vec<String> = l.iter().filter_map(|m| m.as_str().map(str::to_string)).collect();
                v.sort();
                J::from(v)
            }
            _ => J::Null,
        };
        for (k, v) in [
            ("brief", J::from(brief)),
            ("anchor", anchor),
            ("object", J::from(obj_cell)),
            ("missing", missing),
            ("bench_mode", J::from(bm)),
            ("bench_mode_source", J::from(bm_src)),
            ("verdict", J::from(vd)),
            ("verdict_source", J::from(vd_src)),
            ("tagged_utc", J::from(tagged)),
        ] {
            row.set(k, v);
        }
        if in_grammars(pn, &pay_g) {
            let ident = jg(&doc, "identity");
            let bmodel = if ident.is_dict() { jg(ident, "base_model") } else { &crate::value::NULL };
            let v = match bmodel.as_str() {
                Some(s) if !py_strip(s).is_empty() => J::from(s),
                _ => J::Null,
            };
            row.set("base_model", v);
        }
        let local = core::git_nocheck(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/tags/{name}")])?;
        let local = py_strip(&local.stdout);
        if !local.is_empty() && local != obj {
            differs.push(name.to_string());
        }
        entries.push(row);
    }
    let notes = jd! {"absent_local" => absent, "not_annotated" => not_annotated, "local_ref_differs" => differs};
    Ok((entries, notes))
}

pub fn build_index(entries: &J, remote: &str, generated_kst: &str) -> HR<J> {
    let note = format!(
        "카탈로그는 **원격 발행 태그에서 파생**한다(plan_26090107 D1.1 · plan_26092119). 손저작 금지 — 이 파일을 직접 편집하면 다음 derive 에서 사라진다. 진실원천은 원격의 `refs/tags/hint/*` 광고이고, brief·결손·bench_mode 는 원격이 광고한 태그 오브젝트(로컬 오브젝트 DB)에서 파싱한다(합성 금지). `object: {OBJECT_ABSENT_LOCAL}` 행은 로컬에 그 오브젝트가 없어 읽지 못한 원격 태그 · `object: {OBJECT_NOT_ANNOTATED}` 행은 annotation 없는 원격 태그다(둘 다 brief 합성 ✗). `tagged_utc` = 태그 오브젝트 tagger 시각(UTC · 읽지 못하면 null) — HINTS.md 최신순 정렬 키."
    );
    Ok(jd! {
        "_note" => note,
        "schema" => INDEX_SCHEMA,
        "source" => "remote-derived",
        "remote" => _remote_label(remote)?,
        "generated_kst" => generated_kst,
        "count" => entries.len(),
        "hints" => entries.clone(),
    })
}

pub fn _missing_cell(e: &J) -> HR<String> {
    let miss = jg(e, "missing");
    if miss.is_null() {
        return Ok(if jg(e, "object").as_str() == Some(OBJECT_ABSENT_LOCAL) { BENCH_MODE_UNRECEIVED } else { MISSING_UNDECLARED }.to_string());
    }
    if !miss.truthy() {
        return Ok("—".to_string());
    }
    let parts: Vec<String> = match miss {
        J::List(l) => {
            let mut v = Vec::new();
            for x in l {
                match x.as_str() {
                    Some(s) => v.push(s.to_string()),
                    None => {
                        return Err(HintError::py(
                            "TypeError",
                            format!("sequence item {}: expected str instance, {} found", v.len(), x.type_name()),
                        ))
                    }
                }
            }
            v
        }
        J::Str(s) => s.chars().map(String::from).collect(),
        other => other.display().chars().map(String::from).collect(),
    };
    md_cell(&J::from(parts.join(" · ")))
}

pub fn _verdict_display(e: &J) -> HR<String> {
    let v = jg(e, "verdict");
    let legacy = match v {
        J::Null => true,
        J::Str(s) => s.is_empty() || s == VERDICT_ABSENT,
        _ => false,
    };
    Ok(if legacy { VERDICT_LEGACY_CELL.to_string() } else { mdc(v) })
}

fn render_rows_v(entries: &[J]) -> HR<String> {
    let mut rows = vec![
        format!("| {} |", CATALOG_COLUMNS.join(" | ")),
        format!("|{}|", CATALOG_COLUMNS.iter().map(|_| "---").collect::<Vec<_>>().join("|")),
    ];
    for e in entries {
        let bm = jg(e, "bench_mode");
        let bm = if bm.truthy() { bm.clone() } else { J::from(BENCH_MODE_ABSENT) };
        let cells = [
            format!("`{}`", mdc(jg(e, "tag"))),
            mdc(jg(e, "arch")),
            mdc(jg(e, "vllm")),
            _verdict_display(e)?,
            mdc(&bm),
            _missing_cell(e)?,
        ];
        rows.push(format!("| {} |", cells.join(" | ")));
    }
    Ok(rows.join("\n"))
}

pub fn render_rows(entries: &J) -> HR<String> {
    render_rows_v(entries.items())
}

pub fn _group_key(e: &J) -> HR<String> {
    Ok(match jg(e, "model").as_str() {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => MODEL_UNKNOWN_GROUP.to_string(),
    })
}

fn order_entries_v(entries: &[J]) -> HR<Vec<(String, Vec<J>)>> {
    let mut groups: Vec<(String, Vec<J>)> = Vec::new();
    for e in entries {
        let k = _group_key(e)?;
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, rows)) => rows.push(e.clone()),
            None => groups.push((k, vec![e.clone()])),
        }
    }
    let t = |r: &J| or_empty(jg(r, "tagged_utc"));
    let mut ordered: Vec<(String, Vec<J>)> = Vec::new();
    for (g, mut rows) in groups {
        rows.sort_by_key(|r| or_empty(jg(r, "tag")));
        rows.sort_by(|a, b| t(b).cmp(&t(a)));
        ordered.push((g, rows));
    }
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let gmax = |gr: &(String, Vec<J>)| gr.1.iter().map(t).max().unwrap_or_default();
    ordered.sort_by(|a, b| gmax(b).cmp(&gmax(a)));
    Ok(ordered)
}

pub fn order_entries(entries: &J) -> HR<Vec<(String, Vec<J>)>> {
    order_entries_v(entries.items())
}

pub fn render_derived_block(index: &J) -> HR<String> {
    let entries: Vec<J> = jg(index, "hints").items().to_vec();
    let groups = order_entries_v(&entries)?;
    let absent = entries.iter().filter(|e| jg(e, "object").as_str() == Some(OBJECT_ABSENT_LOCAL)).count();
    let unannotated = entries.iter().filter(|e| jg(e, "object").as_str() == Some(OBJECT_NOT_ANNOTATED)).count();
    let mut extra = String::new();
    if absent > 0 {
        extra.push_str(&format!(" · 로컬 오브젝트 부재(미수령) {absent}건"));
    }
    if unannotated > 0 {
        extra.push_str(&format!(" · annotation 없는 태그 {unannotated}건"));
    }
    let mut lines = vec![
        format!(
            "> 원격 `{}` 의 발행 태그 **{}건** · 모델 {}종에서 파생 · 생성 `{}` KST{extra}.",
            mdc(jg(index, "remote")),
            entries.len(),
            groups.len(),
            mdc(jg(index, "generated_kst"))
        ),
        String::new(),
    ];
    for (g, rows) in &groups {
        lines.push(format!("### `{}` ({})", mdc(&J::from(g.as_str())), rows.len()));
        lines.push(String::new());
        lines.push(render_rows_v(rows)?);
        lines.push(String::new());
    }
    Ok(lines.join("\n") + "\n")
}

pub const HINTS_SCAFFOLD_HEAD: &str = "<!-- 이 파일에서 생성물은 아래 마커 구간뿐이다 — `hint.sh catalog derive` 가 그 구간만 다시 쓴다. 구간 밖은 사람이 쓰는 서사다. -->\n# hint 태그 카탈로그\n\n> 사람 서사가 아직 없다 — 파생기가 마커 쌍만 둔 골격이다.\n\n";

/// Byte offsets of the derived block content.
fn _locate_bytes(text: &str) -> HR<(usize, usize)> {
    let remedy = format!(
        "HINTS.md 에 줄 단독의 마커 쌍을 정확히 하나 둔다 — 여는 줄은 `{HINTS_BEGIN_PREFIX}` 로 시작하고(권장 전문 `{HINTS_BEGIN}`) 닫는 줄은 `{HINTS_END}` 다. 여는 줄이 먼저다. 옛 전량 생성 형식(`<!-- hint-index:rows -->` 마커 · 2026-10-11 이전)이면 사람 서사를 저작하고 그 아래에 이 마커 쌍을 둔다. 파생기는 그 사이만 바꾼다."
    );
    let mut begins: Vec<usize> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();
    let mut pos = 0;
    for line in py_splitlines(text, true) {
        let bare = line.trim_end_matches(['\r', '\n']);
        if bare.starts_with(HINTS_BEGIN_PREFIX) {
            begins.push(pos + line.len());
        } else if bare == HINTS_END {
            ends.push(pos);
        }
        pos += line.len();
    }
    let (raw_b, raw_e) = (text.matches("HINTS:BEGIN").count(), text.matches("HINTS:END").count());
    if raw_b != begins.len() || raw_e != ends.len() {
        return _fail(
            "HINT_CATALOG_MARKER_MALFORMED",
            format!(
                "HINTS.md 의 마커 문자열이 줄 단독이 아닌 자리에 있다(BEGIN 문자열 {raw_b} · 줄 단독 {} · END 문자열 {raw_e} · 줄 단독 {}) — 기존 내용을 지우지 않고 중단한다.",
                begins.len(),
                ends.len()
            ),
            Some(&remedy),
            1,
        );
    }
    if begins.is_empty() || ends.is_empty() {
        return _fail(
            "HINT_CATALOG_MARKER_ABSENT",
            format!(
                "HINTS.md 에 파생 구간 마커가 없다(BEGIN {} · END {}) — 쓸 자리가 없으므로 기존 내용을 지우지 않고 중단한다.",
                begins.len(),
                ends.len()
            ),
            Some(&remedy),
            1,
        );
    }
    if begins.len() > 1 || ends.len() > 1 {
        return _fail(
            "HINT_CATALOG_MARKER_DUPLICATE",
            format!(
                "HINTS.md 의 파생 구간 마커가 중복이다(BEGIN {} · END {}) — 어느 구간이 생성물인지 가를 수 없어 중단한다.",
                begins.len(),
                ends.len()
            ),
            Some(&remedy),
            1,
        );
    }
    if ends[0] < begins[0] {
        return _fail(
            "HINT_CATALOG_MARKER_MISORDERED",
            "HINTS.md 의 END 마커가 BEGIN 마커보다 앞에 있다 — 구간을 정할 수 없어 중단한다.",
            Some(&remedy),
            1,
        );
    }
    Ok((begins[0], ends[0]))
}

/// `(start, end)` in characters (Python string offsets).
pub fn locate_derived_block(text: &str) -> HR<(i64, i64)> {
    let (s, e) = _locate_bytes(text)?;
    Ok((chars_len(&text[..s]) as i64, chars_len(&text[..e]) as i64))
}

pub fn splice_hints_md(text: &str, block: &str) -> HR<String> {
    let (s, e) = _locate_bytes(text)?;
    Ok(format!("{}{}{}", &text[..s], block, &text[e..]))
}

pub fn _write(path: &Path, text: &str) -> HR<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| core::io_err(&e, parent))?;
    }
    std::fs::write(path, text).map_err(|e| core::io_err(&e, path))
}

pub fn derive(repo: &Path, remote: &str, generated_kst: &str, dry_run: bool, record_missing: bool, allow_empty: bool) -> HR<i64> {
    let repo = core::resolve(repo);
    if generated_kst.is_empty() || !_KST_RE().is_match(generated_kst) {
        return _fail(
            "HINT_TIME_NOT_INJECTED",
            format!("generated_kst 는 KST `YYYY-MM-DDTHH:MM:SS` 로 주입해야 한다: {}", py_repr(generated_kst)),
            Some("벽시계를 읽지 않는다 — 호출자가 시각을 넘긴다(예: core.kst_iso(<UTC>))."),
            1,
        );
    }
    if !repo.join(CENTRAL_FLAG).is_file() {
        return _fail(
            "HINT_CATALOG_CENTRAL_FLAG_ABSENT",
            format!("`{CENTRAL_FLAG}` 부재 — 카탈로그는 자기 원격의 색인·배포 권위를 가진 체크아웃만 갱신한다(D8 권한 비대칭)."),
            Some(&format!(
                "이 체크아웃이 자기 원격의 hint 카탈로그를 소유한다면 권위를 선언한다: printf '%s\\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' > {CENTRAL_FLAG} (비추적 — 배포본에 실리지 않으므로 각 저장소가 스스로 선언한다 · 선언은 권한이자 책임이다). 상류에 기여하는 입장이면 봉인까지만 하고 태그를 상류에 전달한다. 어느 쪽도 아니면 우회하지 말고 먼저 정한다."
            )),
            1,
        );
    }
    let remote_tags = remote_hint_tags(&repo, remote, allow_empty)?;
    let (entries, notes) = derive_entries(&repo, &remote_tags, Some(remote), record_missing)?;
    let index = build_index(&J::from(entries.clone()), remote, generated_kst)?;
    let index_text = core::dumps(&index);
    let (idx_p, md_p) = (repo.join(core::REL_INDEX), repo.join(core::REL_HINTS_MD));
    let scaffolded = !md_p.exists();
    let base = if scaffolded {
        format!("{HINTS_SCAFFOLD_HEAD}{HINTS_BEGIN}\n{HINTS_END}\n")
    } else {
        core::read_text(&md_p).map_err(|e| HintError::py("OSError", e))?
    };
    let md_text = splice_hints_md(&base, &render_derived_block(&index)?)?;
    let n_remote = remote_tags.len();
    if dry_run {
        let rows = render_rows_v(&entries)?;
        io::outln(&format!("{_OUT} DRY-RUN — 원격 {n_remote}건 · 파생 {}항목 · 쓰기 0", entries.len()));
        io::outln(&format!("{}{}", core::head_chars(&rows, 400), if chars_len(&rows) > 400 { "…" } else { "" }));
    } else {
        _write(&idx_p, &index_text)?;
        _write(&md_p, &md_text)?;
        io::outln(&format!(
            "{_OUT} 파생 완료 — 원격 {n_remote}건 → {} {}항목 · {} 마커 구간 {}행(구간 밖 바이트 불변)",
            core::REL_INDEX,
            entries.len(),
            core::REL_HINTS_MD,
            entries.len()
        ));
    }
    if scaffolded {
        io::errln(&format!(
            "{_OUT} NOTE: {} 가 없어 골격(머리 주석 + 마커 쌍)을 {} — 마커 밖 사람 서사는 비어 있다. 저작해 채운다.",
            core::REL_HINTS_MD,
            if dry_run { "만들 것이다(dry-run · 쓰기 0)" } else { "만들었다" }
        ));
    }
    let strs = |k: &str| -> Vec<String> { notes[k].items().iter().map(J::display).collect() };
    let absent = strs("absent_local");
    if !absent.is_empty() {
        io::errln(&format!(
            "{_OUT} NOTE: 로컬 태그 오브젝트 부재 {}건을 `object: {OBJECT_ABSENT_LOCAL}` 행으로 실었다(brief '{BRIEF_UNRECEIVED}' · 결손 '{BENCH_MODE_UNRECEIVED}' · 합성 ✗). 채우려면 `{}` 후 다시 파생한다.",
            absent.len(),
            fetch_hint(&repo, Some(remote))?
        ));
        for t in absent.iter().take(10) {
            io::errln(&format!("  {t}"));
        }
    }
    let na = strs("not_annotated");
    if !na.is_empty() {
        io::errln(&format!(
            "{_OUT} NOTE: annotated 가 아닌 원격 태그 {}건을 `object: {OBJECT_NOT_ANNOTATED}` 행으로 실었다(brief '{BRIEF_UNRECEIVED}' · 합성 ✗ · 원격 태그는 교정하지 않는다 — P1). 발행 경로 밖에서 만든 태그다 — 그 원격의 발행자에게 알린다.",
            na.len()
        ));
        for t in na.iter().take(10) {
            io::errln(&format!("  {t}"));
        }
    }
    let differs = strs("local_ref_differs");
    if !differs.is_empty() {
        io::errln(&format!(
            "{_OUT} 고지: 같은 이름의 로컬 태그가 원격과 **다른 오브젝트**를 가리키는 태그 {}건 — 카탈로그는 원격이 광고한 오브젝트에서 파생했다(로컬 ref 불변):",
            differs.len()
        ));
        for t in differs.iter().take(10) {
            io::errln(&format!("  {t}"));
        }
    }
    let listed = core::git(&repo, &["tag", "-l", "hint/*"])?.stdout;
    let mut local_only: Vec<String> = py_split_ws(&listed)
        .into_iter()
        .filter(|t| remote_tags.get(t).is_none())
        .map(str::to_string)
        .collect();
    local_only.sort();
    local_only.dedup();
    if !local_only.is_empty() {
        io::errln(&format!(
            "{_OUT} 고지: 로컬에만 있는 미발행 태그 {}건 — 카탈로그에 **들어가지 않았다**(발행 사실이 없다):",
            local_only.len()
        ));
        for t in local_only.iter().take(10) {
            io::errln(&format!("  {t}"));
        }
    }
    Ok(0)
}

// ── 수신자 쪽 (git 없이 동작) ─────────────────────────────────────────────────────────────────
pub fn load_index(repo: &Path) -> HR<J> {
    let p = repo.join(core::REL_INDEX);
    if !p.is_file() {
        return _fail(
            "HINT_CATALOG_INDEX_ABSENT",
            format!("{} 가 없다.", core::REL_INDEX),
            Some("중앙 저장소라면 `hint.sh catalog derive` 로 만든다. 배포본이라면 hints/index.json 이 함께 온다 — 체크아웃을 확인한다."),
            1,
        );
    }
    let doc = match core::read_text(&p).and_then(|t| loads(&t)) {
        Ok(d) => d,
        Err(e) => return _fail("HINT_CATALOG_INDEX_MALFORMED", format!("{} 를 읽을 수 없다: {e}", core::REL_INDEX), None, 1),
    };
    if !doc.is_dict() || !jg(&doc, "hints").is_list() {
        return _fail("HINT_CATALOG_INDEX_MALFORMED", format!("{} 에 `hints` 목록이 없다.", core::REL_INDEX), None, 1);
    }
    Ok(doc)
}

pub const REL_SAME: &str = "same-slug";
pub const REL_DERIVED: &str = "derived-from-query";
pub const REL_BASE: &str = "base-of-query";
pub const REL_SIBLING: &str = "shared-base";
fn _REL_RANK(rel: &str) -> i64 {
    match rel {
        REL_SAME => 3,
        REL_DERIVED | REL_BASE => 2,
        REL_SIBLING => 1,
        _ => 0,
    }
}
/// hintlib/catalog.py:735 _ARCH_RANK (python literal as JSON)
pub fn _ARCH_RANK() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"exact\": 3, \"prefix\": 2, \"hw\": 1, \"none\": 0, \"unspecified\": 0}").expect("constant"))
}

pub fn _hw_token(arch: &str) -> HR<String> {
    let head = arch.split('-').next().unwrap_or("").to_lowercase();
    Ok(_HW_MULT().sub_literal(&head, ""))
}

pub fn _arch_match(entry_arch: &str, query: Option<&str>) -> HR<String> {
    let q = match query {
        Some(q) if !q.is_empty() => py_strip(q).to_lowercase(),
        _ => return Ok("unspecified".into()),
    };
    let ea = entry_arch.to_lowercase();
    if ea == q {
        return Ok("exact".into());
    }
    if ea.starts_with(&format!("{q}-")) {
        return Ok("prefix".into());
    }
    let h = _hw_token(&ea)?;
    Ok(if !h.is_empty() && h == _hw_token(&q)? { "hw" } else { "none" }.into())
}

pub fn _vnorm(v: Option<&str>) -> HR<String> {
    let s = py_strip(v.unwrap_or(""));
    let mut it = s.chars();
    let first = it.next();
    let second = it.next();
    if matches!(first, Some('v') | Some('V')) && second.map(|c| c.is_ascii_digit() || (!c.is_ascii() && c.is_numeric())).unwrap_or(false) {
        return Ok(s[1..].to_string());
    }
    Ok(s.to_string())
}

pub fn r#match(index: &J, vllm: &str, model: &str, arch: Option<&str>, include_other: bool) -> HR<Vec<J>> {
    if py_strip(vllm).is_empty() || query_slug(Some(model))?.is_empty() {
        return _fail("HINT_CATALOG_MATCH_QUERY_EMPTY", "match 에는 --vllm 과 --model 이 필요하다.", None, 1);
    }
    let entries = match index.get("hints") {
        Some(J::List(l)) if index.is_dict() => l,
        _ => return _fail("HINT_CATALOG_INDEX_MALFORMED", "index 에 `hints` 목록이 없다.", None, 1),
    };
    let q = query_slug(Some(model))?;
    let mut q_bases: Vec<String> = entries
        .iter()
        .filter(|e| e.is_dict() && jg(e, "base_model").is_str() && qs(jg(e, "model")) == q)
        .map(|e| jg(e, "base_model").display())
        .collect();
    q_bases.sort();
    q_bases.dedup();
    let q_base_norms: Vec<String> = q_bases.iter().map(|b| query_slug(Some(b)).unwrap_or_default()).collect();
    let mut out: Vec<J> = Vec::new();
    for e in entries {
        if !e.is_dict() || !jg(e, "tag").is_str() {
            continue;
        }
        let em = qs(jg(e, "model"));
        let eb = jg(e, "base_model").as_str();
        let (rel, basis): (Option<&str>, J) = if !em.is_empty() && em == q {
            (Some(REL_SAME), J::from(format!("slug≡{}", jg(e, "model").display())))
        } else if eb.is_some() && !eb.unwrap().is_empty() && query_slug(eb)? == q {
            (Some(REL_DERIVED), J::from(format!("base_model={}", eb.unwrap())))
        } else if !em.is_empty() && q_base_norms.contains(&em) {
            (Some(REL_BASE), J::from(format!("질의 모델의 base_model={}", q_bases.join(" · "))))
        } else if eb.is_some() && !eb.unwrap().is_empty() && q_base_norms.contains(&query_slug(eb)?) {
            (Some(REL_SIBLING), J::from(format!("base_model={}", eb.unwrap())))
        } else {
            (None, J::Null)
        };
        if rel.is_none() && !include_other {
            continue;
        }
        let ea = jg(e, "arch");
        let am = _arch_match(&or_empty(ea), arch)?;
        let vm = _vnorm(jg(e, "vllm").as_str())? == _vnorm(Some(vllm))?;
        out.push(jd! {
            "tag" => jg(e, "tag").clone(), "grammar" => jg(e, "grammar").clone(), "vllm" => jg(e, "vllm").clone(),
            "model" => jg(e, "model").clone(), "arch" => ea.clone(), "recipe" => jg(e, "recipe").clone(),
            "brief" => jg(e, "brief").clone(), "base_model" => jg(e, "base_model").clone(), "object" => jg(e, "object").clone(),
            "missing" => jg(e, "missing").clone(), "bench_mode" => jg(e, "bench_mode").clone(), "verdict" => jg(e, "verdict").clone(),
            "relation" => rel.unwrap_or("other-model"), "relation_basis" => basis,
            "vllm_match" => vm, "arch_match" => am,
        });
    }
    let mut keyed: Vec<((i64, bool, i64, naming::VersionKey, String), J)> = Vec::new();
    for r in out {
        let rel = r.get_str("relation").unwrap_or("").to_string();
        let vk = naming::version_key(&or_empty(jg(&r, "vllm")))?;
        let ar = _ARCH_RANK().get(r.get_str("arch_match").unwrap_or("")).and_then(J::as_i64).unwrap_or(0);
        let key = (-_REL_RANK(&rel), !r["vllm_match"].truthy(), -ar, vk, r.get_str("tag").unwrap_or("").to_string());
        keyed.push((key, r));
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(keyed.into_iter().map(|(_, r)| r).collect())
}

pub fn format_match(index: &J, results: &J, model: &str, include_other: bool) -> HR<String> {
    let total = if index.is_dict() { jg(index, "hints").len() as i64 } else { 0 };
    let results = results.items();
    let mut lines: Vec<String> = Vec::new();
    for r in results {
        let mut guide: Vec<String> = Vec::new();
        let rel = jg(r, "relation").display();
        let vm = jg(r, "vllm_match").truthy();
        let am = jg(r, "arch_match").display();
        let am_ok = am == "exact" || am == "unspecified";
        if rel == REL_SAME && vm && am_ok {
            guide.push(
                if am == "exact" {
                    "정확 일치 — 그래도 네 스모크로 재검증"
                } else {
                    "모델·vLLM 일치(arch 미지정) — arch 를 대조하고 네 스모크로 재검증"
                }
                .to_string(),
            );
        } else {
            if rel == "other-model" {
                guide.push("다른 모델→서빙전략 독립(참고만)".to_string());
            } else if rel != REL_SAME {
                guide.push(format!("관계 모델({rel}: {})→서빙전략은 다시 세운다", jg(r, "relation_basis").display()));
            }
            if !vm {
                guide.push("다른 vLLM→릴리스 노트 재확인(carry-forward ✗)".to_string());
            }
            if !am_ok {
                guide.push("다른 arch→빌드트랙·벽지도 이식 가능 · KV/gmu/TORCH_CUDA_ARCH 재도출".to_string());
            }
        }
        let obj = jg(r, "object").as_str();
        if obj == Some(OBJECT_ABSENT_LOCAL) {
            guide.push("카탈로그 저장소에 오브젝트 미수령(brief 없음) — 원격에서 받아 읽는다".to_string());
        } else if obj == Some(OBJECT_NOT_ANNOTATED) {
            guide.push("annotation 없는 태그(brief 없음 · 발행 경로 밖) — 트리를 받아 직접 확인한다".to_string());
        }
        let gr = jg(r, "grammar");
        let gr = if gr.truthy() { gr.display() } else { "?".to_string() };
        lines.push(format!("[{rel}] {}  ({gr})  ·  {}", jg(r, "tag").display(), guide.join(" · ")));
    }
    let shown_other = results.iter().filter(|r| jg(r, "relation").as_str() == Some("other-model")).count() as i64;
    let hidden = if include_other { 0 } else { total - results.len() as i64 };
    let related = results.len() as i64 - shown_other;
    if related == 0 {
        lines.push(format!(
            "{_OUT} 같은 모델(정규화 슬러그 · base_model) 태그 없음: {}{}",
            py_repr(model),
            if hidden != 0 { format!(" · 다른 모델 {hidden}건은 --include-other 로.") } else { String::new() }
        ));
    } else if hidden != 0 {
        lines.push(format!("{_OUT} (관계없는 모델 {hidden}건 숨김 — --include-other)"));
    }
    Ok(lines.join("\n"))
}

// ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
/// `catalog.selftest()` — failure messages (empty = pass). Isolated temp repositories + bare remotes only.
pub fn selftest() -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    if let Err(e) = selftest_inner(&mut bad) {
        bad.push(format!("catalog: selftest 실행 예외: {}", e.render()));
    }
    bad
}

fn err_of<T>(f: impl FnOnce() -> HR<T>) -> (Option<HintError>, String) {
    let (r, o, e) = io::capture(f);
    (r.err(), o + &e)
}

fn lj(s: &str) -> J {
    loads(s).expect("selftest literal")
}

fn selftest_inner(bad: &mut Vec<String>) -> HR<()> {
    let mut ck = |name: &str, cond: bool| {
        if !cond {
            bad.push(format!("catalog: {name}"));
        }
    };
    let s = |x: &str| J::from(x);
    // ── 순수 헬퍼 (⑤ · ⑬) ──
    let warn = "<!-- ⚠ 절 헤딩(`## 1.` ~ `## 7.`)을 지우지 마라 — seal 의 린터 L1 이 fail-closed 로 -->\n\n진짜 brief 한 줄\n\n## 1. 벽 지도\n";
    ck("★⑤회귀 HTML 주석을 brief 로 캐지 않는다", extract_brief(warn)? == "진짜 brief 한 줄");
    ck("★⑤회귀 cat-file 헤더를 brief 로 캐지 않는다", extract_brief("object abc\ntype commit\ntag hint/a/b/c/d\n\n실제 brief\n")? == "실제 brief");
    ck("인용부호 줄은 brief 아님", extract_brief("> ⚠ 유효맥락 …\n\nbrief 다\n")? == "brief 다");
    ck("헤딩은 brief 아님", extract_brief("# 제목\n\nbrief\n")? == "brief");
    ck("본문이 비면 빈 문자열", extract_brief("")?.is_empty());
    ck(
        "v6 brief = 첫 문단(여러 줄은 한 줄로)",
        annotation_brief("첫 줄 요약\n둘째 줄\n\n전체는 zip 의 00-hint.md\n\n<!-- hint-evidence-binding:v1\nx\n-->\n")? == "첫 줄 요약 둘째 줄",
    );
    ck("v6 brief 는 앞선 HTML 주석을 건너뛴다", annotation_brief("<!-- c -->\n\n요약\n")? == "요약");
    ck("★파이프 이스케이프", md_cell(&s("a | b"))? == "a \\| b");
    ck("★미닫힘 주석 무해화", !md_cell(&s("깨진 <!-- 주석"))?.contains("<!--"));
    ck("줄바꿈 평탄화", !md_cell(&s("a\nb"))?.contains('\n'));
    let head_txt = "<!-- 사람 머리 -->\r\n# 서사\n\n본문 — 고르는 법 · 받아 읽기\n";
    let tail_txt = "\n## 꼬리 서사\n끝(개행 없음)";
    let begin_custom = format!("{HINTS_BEGIN_PREFIX} — 사람이 고친 꼬리 설명 -->\n");
    let doc_txt = format!("{head_txt}{begin_custom}| 옛 행 |\n{HINTS_END}\n{tail_txt}");
    let spliced = splice_hints_md(&doc_txt, "새 구간\n")?;
    ck(
        "★마커 구간만 교체 · 구간 밖(BEGIN 줄 꼬리 설명 · CRLF · 끝 개행 없음 포함) 바이트 불변",
        spliced == format!("{head_txt}{begin_custom}새 구간\n{HINTS_END}\n{tail_txt}"),
    );
    ck("교체는 멱등(같은 구간 → 같은 바이트)", splice_hints_md(&spliced, "새 구간\n")? == spliced);
    ck(
        "빈 구간(BEGIN 바로 뒤 END)도 교체된다",
        splice_hints_md(&format!("{HINTS_BEGIN}\n{HINTS_END}\n"), "x\n")? == format!("{HINTS_BEGIN}\nx\n{HINTS_END}\n"),
    );
    let bads: Vec<(String, &str)> = vec![
        ("머리만\n".into(), "HINT_CATALOG_MARKER_ABSENT"),
        (format!("머리\n{HINTS_BEGIN}\n꼬리\n"), "HINT_CATALOG_MARKER_ABSENT"),
        (format!("머리\n{HINTS_END}\n꼬리\n"), "HINT_CATALOG_MARKER_ABSENT"),
        ("머리\n<!-- hint-index:rows -->\n| 옛 |\n<!-- hint-index:rows -->\n".into(), "HINT_CATALOG_MARKER_ABSENT"),
        (format!("{HINTS_BEGIN}\na\n{HINTS_END}\n{HINTS_BEGIN}\nb\n{HINTS_END}\n"), "HINT_CATALOG_MARKER_DUPLICATE"),
        (format!("{HINTS_BEGIN}\na\n{HINTS_END}\n{HINTS_END}\n"), "HINT_CATALOG_MARKER_DUPLICATE"),
        (format!("{HINTS_END}\na\n{HINTS_BEGIN}\n"), "HINT_CATALOG_MARKER_MISORDERED"),
        (format!("서사 중간 {HINTS_BEGIN}\na\n{HINTS_END}\n"), "HINT_CATALOG_MARKER_MALFORMED"),
        (format!("{HINTS_BEGIN}\na {HINTS_END}\n"), "HINT_CATALOG_MARKER_MALFORMED"),
    ];
    for (bad_doc, code) in &bads {
        let (e, _) = err_of(|| splice_hints_md(bad_doc, "x\n"));
        ck(
            &format!("★음성대조 마커 이상 → {code}(쓰기 전 중단 · 처방 동반)"),
            e.as_ref().map(|e| e.code == *code && e.remedy.as_deref().unwrap_or("").contains(HINTS_END)).unwrap_or(false),
        );
    }
    let ents_o = lj(r#"[{"tag": "hint/1/m-a/x/r2", "model": "m-a", "tagged_utc": "2026-09-01T00:00:00Z"},
        {"tag": "hint/1/m-a/x/r1", "model": "m-a", "tagged_utc": "2026-09-01T00:00:00Z"},
        {"tag": "hint/1/m-a/x/r0", "model": "m-a", "tagged_utc": null},
        {"tag": "hint/1/m-a/x/r9", "model": "m-a", "tagged_utc": "2026-10-01T00:00:00Z"},
        {"tag": "hint/1/m-b/x/r", "model": "m-b", "tagged_utc": "2026-10-05T00:00:00Z"},
        {"tag": "hint/1/m-c/x/r", "model": "m-c", "tagged_utc": "2026-10-05T00:00:00Z"},
        {"tag": "hint/z", "model": null, "tagged_utc": null},
        {"tag": "hint/1/m-d/x/r", "model": "m-d", "tagged_utc": null}]"#);
    let tags_of = |o: Vec<(String, Vec<J>)>| -> Vec<(String, Vec<String>)> {
        o.into_iter().map(|(g, rs)| (g, rs.iter().map(|r| r["tag"].display()).collect())).collect()
    };
    let order = tags_of(order_entries(&ents_o)?);
    let want: Vec<(String, Vec<String>)> = vec![
        ("m-b".into(), vec!["hint/1/m-b/x/r".into()]),
        ("m-c".into(), vec!["hint/1/m-c/x/r".into()]),
        ("m-a".into(), vec!["hint/1/m-a/x/r9".into(), "hint/1/m-a/x/r1".into(), "hint/1/m-a/x/r2".into(), "hint/1/m-a/x/r0".into()]),
        (MODEL_UNKNOWN_GROUP.into(), vec!["hint/z".into()]),
        ("m-d".into(), vec!["hint/1/m-d/x/r".into()]),
    ];
    ck(&format!("★결정론 정렬: 묶음 = 최신 시각 내림차순→제목순 · 묶음 안 = 시각 내림차순→이름순 · 시각 없음 맨 뒤({order:?})"), order == want);
    let rev = J::from(ents_o.items().iter().rev().cloned().collect::<Vec<J>>());
    ck("정렬은 입력 순서에 무관(뒤집어 넣어도 같다)", tags_of(order_entries(&rev)?) == order);
    ck(
        "★판정 칸 없는 구판 행 = 미기재(구판) · 미수령·REFUTE 는 그대로",
        _verdict_display(&jd! {"verdict" => VERDICT_ABSENT})? == VERDICT_LEGACY_CELL
            && VERDICT_LEGACY_CELL == "미기재(구판)"
            && _verdict_display(&J::dict())? == VERDICT_LEGACY_CELL
            && _verdict_display(&jd! {"verdict" => "미수령"})? == "미수령"
            && _verdict_display(&jd! {"verdict" => "REFUTE"})? == "REFUTE",
    );
    ck(
        "tagger 줄 → UTC(tz 무관 · 이름·메일 버림) · 없으면 None",
        tagger_utc("object a\ntype commit\ntag t\ntagger 누구 <> 1760140800 +0900")?.as_deref() == Some("2025-10-11T00:00:00Z")
            && tagger_utc("object a\ntype commit\n")?.is_none(),
    );
    for (doc, want) in [
        (r#"{"measurement_config": {"bench_mode": "lite", "bench_mode_kind": "declared-lite", "downgrade_reason": null, "source": "bench_report(x.md)"}}"#, "lite(선언)"),
        (r#"{"measurement_config": {"bench_mode": "lite", "bench_mode_kind": "downgraded-lite", "downgrade_reason": "blackbox_kill"}}"#, "lite(강등·blackbox_kill)"),
        (r#"{"measurement_config": {"bench_mode": "full"}}"#, "full"),
        (r#"{"measurement_config": {"bench_mode": "lite"}}"#, "lite"),
        (r#"{"measurement_config": {"bench_mode": null}}"#, BENCH_MODE_UNDETERMINED),
        (r#"{"missing": []}"#, BENCH_MODE_ABSENT),
        ("null", BENCH_MODE_ABSENT),
    ] {
        ck(&format!("bench_mode 파생 칸: {want}"), bench_mode_cell(&lj(doc))?.0 == want);
    }
    for (doc, want) in [
        (r#"{"measurement": {"verdict": "REFUTE", "sources": {"verdict": "sweep verdict.json"}}}"#, "REFUTE"),
        (r#"{"format": "hint-payload/v6", "measurement": {"verdict": "PASS", "source": "certificate"}}"#, "PASS"),
        (r#"{"measurement": {"verdict": "OBSERVATION-ONLY"}}"#, "OBSERVATION-ONLY"),
        (r#"{"measurement": {"source": "absent(인증서·스윕 모두 없다)"}}"#, VERDICT_ABSENT),
        (r#"{"measurement": {"verdict": ""}}"#, VERDICT_ABSENT),
        ("{}", VERDICT_ABSENT),
        ("null", VERDICT_ABSENT),
    ] {
        ck(&format!("판정 파생 칸: {want}"), verdict_cell(&lj(doc))?.0 == want);
    }
    ck(
        "판정 출처: 키별 출처(sources.verdict)가 있으면 그것",
        verdict_cell(&lj(r#"{"measurement": {"verdict": "REFUTE", "sources": {"verdict": "sweep verdict.json"}}}"#))?.1.contains("sweep verdict.json"),
    );
    ck("★판정 어휘 밖 값은 접지 않고 표시한다", verdict_cell(&lj(r#"{"measurement": {"verdict": "MAYBE"}}"#))?.0 == "MAYBE(어휘 밖)");
    let (lv, lsrc) = verdict_cell(&lj(r#"{"cell_grade": "lite", "measurement": {"verdict": "OBSERVATION-ONLY"}}"#))?;
    ck(
        "lite 셀 판정 칸 = lite(기동 가능)(= template.LITE_LABEL) · 출처에 기계 판정 OBSERVATION-ONLY",
        lv == VERDICT_LITE
            && VERDICT_LITE == crate::template::LITE_LABEL
            && lsrc.contains("OBSERVATION-ONLY")
            && _verdict_display(&jd! {"verdict" => lv.clone()})? == VERDICT_LITE,
    );
    ck(
        "옛 lite 페이로드(cell_grade 없음 · benchmark_mode lite) = lite(기동 가능)",
        verdict_cell(&lj(r#"{"measurement": {"verdict": "OBSERVATION-ONLY", "benchmark_mode": "lite"}}"#))?.0 == VERDICT_LITE,
    );
    ck(
        "★강등 셀(downgraded-lite)은 lite 라벨로 접지 않는다(판정 그대로)",
        verdict_cell(&lj(
            r#"{"measurement": {"verdict": "PASS", "benchmark_mode": "lite"}, "measurement_config": {"bench_mode": "lite", "bench_mode_kind": "downgraded-lite"}}"#,
        ))?
        .0 == "PASS",
    );
    ck("사람 목록 6열(plan_26101104 Step 2 — 태그·arch·vLLM·판정·bench_mode·결손)", CATALOG_COLUMNS == ["태그", "arch", "vLLM", "판정", "bench_mode", "결손"]);
    ck("중앙 권위 경로 교차검증(core.REL_CENTRAL_FLAG)", CENTRAL_FLAG == core::REL_CENTRAL_FLAG);
    ck("정규화 슬러그 동치(2026-08-20 gemma 철자 갈림)", query_slug(Some("gemma-4-E2B-it"))? == query_slug(Some("gemma-4-e2b-it"))?);
    ck("검색 키는 Org/Name 의 basename 을 본다", query_slug(Some("Qwen/Qwen3.8-Flash-Next"))? == query_slug(Some("qwen3.8-flash-next"))?);
    ck("★값 없는 모델은 빈 키(None 을 'none' 슬러그로 접지 않는다)", query_slug(None)?.is_empty() && query_slug(Some(""))?.is_empty());
    let mut vs: Vec<(naming::VersionKey, &str)> = Vec::new();
    for v in ["0.29.0-g0123456789ab", "0.29.0", "0.29.0rc6", "0.1.1.dev53+g30118ba27", "0.29.0rc5", "0.19.0"] {
        vs.push((naming::version_key(v)?, v));
    }
    vs.sort_by(|a, b| a.0.cmp(&b.0));
    ck(
        "match 가 쓰는 버전 키의 시간축(dev<rc<릴리스<커밋 핀 · naming.version_key)",
        vs.iter().map(|x| x.1).collect::<Vec<_>>() == ["0.1.1.dev53+g30118ba27", "0.19.0", "0.29.0rc5", "0.29.0rc6", "0.29.0", "0.29.0-g0123456789ab"],
    );
    ck(
        "이름 해체: 4세그먼트 = legacy-4seg 읽기 전용(recipe 없음) · 그 밖 = unknown(추측 채움 ✗)",
        parse_name("hint/0.23.0/deepseek-v4-flash/gb10")?
            == jd! {"grammar" => naming::GRAMMAR_LEGACY_4SEG, "vllm" => "0.23.0", "model" => "deepseek-v4-flash", "arch" => "gb10", "recipe" => J::Null}
            && parse_name("hint/0.19.1/gpt-oss-120b")?
                == jd! {"grammar" => naming::GRAMMAR_UNKNOWN, "vllm" => J::Null, "model" => J::Null, "arch" => J::Null, "recipe" => J::Null},
    );
    ck(
        "★원격 경로·URL 은 배포 파일에 적지 않는다",
        _remote_label("/srv/x/remote.git")?.starts_with('<')
            && _remote_label("ssh://example.invalid/o/r.git")?.starts_with('<')
            && _remote_label("github-ssh")? == "github-ssh",
    );
    // (H5 standalone-load probes are Python-module properties — not applicable to the Rust port.)

    // ── 격리 저장소 · bare 원격 ──
    let date = core::git_date("2026-09-21T00:00:00Z")?;
    let email = core::synthetic_email();
    let ident: Vec<(String, String)> = vec![
        ("GIT_AUTHOR_NAME".into(), core::SYNTHETIC_NAME.into()),
        ("GIT_AUTHOR_EMAIL".into(), email.clone()),
        ("GIT_COMMITTER_NAME".into(), core::SYNTHETIC_NAME.into()),
        ("GIT_COMMITTER_EMAIL".into(), email.clone()),
        ("GIT_AUTHOR_DATE".into(), date.clone()),
        ("GIT_COMMITTER_DATE".into(), date.clone()),
    ];
    let uname = format!("user.name={}", core::SYNTHETIC_NAME);
    let umail = format!("user.email={email}");
    let cfg: Vec<String> = ["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", "-c", "core.hooksPath=/dev/null", "-c", &uname, "-c", &umail]
        .iter()
        .map(|x| x.to_string())
        .collect();
    let g_in = |repo: &Path, args: &[&str], input: Option<&str>| -> HR<String> {
        let mut a: Vec<&str> = cfg.iter().map(String::as_str).collect();
        a.extend_from_slice(args);
        Ok(py_strip(&core::git_full(repo, &a, true, input, &ident)?.stdout).to_string())
    };
    let g = |repo: &Path, args: &[&str]| g_in(repo, args, None);
    let payload_commit = |repo: &Path, payload: Option<&J>, marker: &str| -> HR<String> {
        let mut files: Vec<(String, String)> = vec![("marker.txt".into(), format!("{marker}\n"))];
        if let Some(p) = payload {
            files.push(("PAYLOAD.json".into(), core::dumps(p)));
        }
        files.sort();
        let mut lines = Vec::new();
        for (fname, content) in &files {
            let blob = g_in(repo, &["hash-object", "-w", "--stdin"], Some(content))?;
            lines.push(format!("100644 blob {blob}\t{fname}"));
        }
        let tree = g_in(repo, &["mktree"], Some(&(lines.join("\n") + "\n")))?;
        g(repo, &["commit-tree", &tree, "-m", &format!("hint: {marker}")])
    };
    let atag = |repo: &Path, name: &str, target: &str, message: &str| -> HR<()> {
        g_in(repo, &["tag", "-a", "-F", "-", "--cleanup=verbatim", name, target], Some(message)).map(|_| ())
    };
    let ps = |p: &Path| core::posix(p);
    let read_s = |p: &Path| core::read_text(p).unwrap_or_default();
    let read_b = |p: &Path| std::fs::read(p).ok();
    let wr = |p: &Path, t: &str| std::fs::write(p, t).map_err(|e| core::io_err(&e, p));

    let kst = "2026-09-21T19:00:00";
    let v6_tag = "hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-spec3-eager";
    let legacy_tag = "hint/0.19.0/gpt-oss-120b/gb10x2-cluster-native/qmxfp4-len131072-kvfp8";
    let remote_only = "hint/0.27.1/qwen3-4b/h10080gb-main-native/len32768-kvauto-pleresident";
    let v7_tag = "hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-spec7-t2609290823";
    let v7_annotation = format!(
        "DS4F 1M 컨텍스트 spec7 셀 — 루브릭 바닥 미달(REFUTE).\n\n전체 지도·서사·재현 키트는 이 태그의 zip(archive) 안에 있다 — `00-hint.md` 부터 읽는다.\n\n<!-- hint-evidence-binding:v2\nversion: 2\ntag: {v7_tag}\n-->\n"
    );
    let v6_annotation = format!(
        "NVFP4 체크포인트를 GB10 2노드(TP=2 · Ray)에서 262144 컨텍스트로 서빙한 여정.\nPLE mmap 이 생사를 가른 축이었다.\n\n전체 지도·서사·재현 키트는 이 태그의 zip(archive) 안에 있다 — `00-hint.md` 부터 읽는다.\n\n<!-- hint-evidence-binding:v1\nversion: 1\ntag: {v6_tag}\n-->\n"
    );
    let td = core::TempDir::new("hintlib-catalog-");
    let root = core::resolve(td.path());
    let repo = root.join("main");
    std::fs::create_dir_all(&repo).map_err(|e| core::io_err(&e, &repo))?;
    g(&repo, &["init", "-q", "-b", "main"])?;
    g(&repo, &["commit", "--allow-empty", "-q", "-m", "base"])?;
    std::fs::create_dir_all(repo.join("hints")).map_err(|e| core::io_err(&e, &repo))?;
    wr(&repo.join(CENTRAL_FLAG), "selftest — 이 체크아웃이 자기 원격의 hint 색인 권위다.\n")?;
    let md_head = "<!-- 머리 주석 -->\n# 사람 서사\n\n고르는 법 — 손으로 쓴 문단\n\n";
    let md_tail = "\n## 꼬리 서사\n손으로 쓴 꼬리\n";
    wr(&repo.join(core::REL_HINTS_MD), &format!("{md_head}{HINTS_BEGIN}\n| 옛 |\n{HINTS_END}\n{md_tail}"))?;
    let mut v6_payload = lj(r#"{"schema_version": 2, "format": "hint-payload/v6", "tag": "",
        "identity": {"model": "qwen3.8-flash-next-nvfp4", "base_model": "Qwen/Qwen3.8-Flash-Next", "hf_repo": "Org/Qwen3.8-Flash-Next-NVFP4"},
        "missing": ["BENCH_MODE_LITE"],
        "measurement_config": {"bench_mode": "lite", "bench_mode_kind": "declared-lite", "downgrade_reason": null, "source": "bench_report(r.md)"}}"#);
    v6_payload.set("tag", v6_tag);
    let v6_commit = payload_commit(&repo, Some(&v6_payload), "v6")?;
    atag(&repo, v6_tag, &v6_commit, &v6_annotation)?;
    let mut v7_payload = lj(r#"{"schema_version": 2, "format": "hint-payload/v7", "tag": "",
        "identity": {"model": "deepseek-v4-flash-0731", "base_model": "deepseek-ai/DeepSeek-V4-Flash"},
        "missing": [], "measurement": {"verdict": "REFUTE", "sources": {"verdict": "sweep verdict.json"}},
        "measurement_config": {"bench_mode": "full", "source": "bench_mode.json"}}"#);
    v7_payload.set("tag", v7_tag);
    let v7_commit = payload_commit(&repo, Some(&v7_payload), "v7")?;
    atag(&repo, v7_tag, &v7_commit, &v7_annotation)?;
    let legacy_commit = payload_commit(
        &repo,
        Some(&lj(r#"{"missing": ["HINT_MISSING_SLAVE_ATTESTATION"], "identity": {"base_model": "openai/should-not-be-read"}}"#)),
        "legacy",
    )?;
    atag(&repo, legacy_tag, &legacy_commit, "<!-- ⚠ 헤딩을 지우지 마라 -->\nGB10 x2 클러스터 · 네이티브 예산\n\n## 1. 벽 지도\n본문\n")?;
    let bare = root.join("remote.git");
    g(&root, &["init", "--bare", "-q", &ps(&bare)])?;
    g(&repo, &["remote", "add", "fx", &ps(&bare)])?;
    let rs = |t: &str| format!("refs/tags/{t}:refs/tags/{t}");
    g(&repo, &["push", "-q", "fx", &rs(v6_tag), &rs(legacy_tag), &rs(v7_tag)])?;
    let (idx_p, md_p) = (repo.join(core::REL_INDEX), repo.join(core::REL_HINTS_MD));
    let hints_by = |p: &Path| -> std::collections::HashMap<String, J> {
        let d = loads(&read_s(p)).unwrap_or(J::Null);
        d["hints"].items().iter().map(|h| (h["tag"].display(), h.clone())).collect()
    };

    let (e, _) = err_of(|| derive(&repo, "fx", kst, false, false, false));
    ck("파생 성공(rc 0 · 예외 없음)", e.is_none());
    if e.is_none() {
        let (idx_b1, md_b1) = (read_b(&idx_p), read_b(&md_p));
        let (e2, _) = err_of(|| derive(&repo, "fx", kst, false, false, false));
        ck("★결정론: 같은 입력으로 두 번 파생 = index.json·HINTS.md 바이트 동일", e2.is_none() && read_b(&idx_p) == idx_b1 && read_b(&md_p) == md_b1);
        let idx_text = String::from_utf8(idx_b1.clone().unwrap_or_default()).unwrap_or_default();
        let idx = loads(&idx_text).unwrap_or(J::Null);
        let md = String::from_utf8(md_b1.clone().unwrap_or_default()).unwrap_or_default();
        let by = hints_by(&idx_p);
        let nul = J::dict();
        let (h6, hl, h7) = (by.get(v6_tag).unwrap_or(&nul), by.get(legacy_tag).unwrap_or(&nul), by.get(v7_tag).unwrap_or(&nul));
        ck(
            "index schema 5 · 출처 remote-derived · 개수",
            idx["schema"].as_i64() == Some(INDEX_SCHEMA) && INDEX_SCHEMA == 5 && idx.get_str("source") == Some("remote-derived")
                && idx["count"].as_i64() == Some(3) && idx.get_str("remote") == Some("fx"),
        );
        let mut keys = vec![
            "tag", "grammar", "vllm", "model", "arch", "recipe", "brief", "anchor", "object", "missing", "bench_mode", "bench_mode_source",
            "verdict", "verdict_source", "base_model", "published", "source", "tagged_utc",
        ];
        keys.sort();
        let keyset = |h: &J| -> Vec<String> {
            let mut k: Vec<String> = h.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default();
            k.sort();
            k
        };
        ck("index 항목 키 = SPEC 목록(v6·v7 · verdict 추가 외 호환)", keyset(h6) == keys && keyset(h7) == keys);
        ck(
            "문법 열: v7 태그 = v7 · base_model · brief = 첫 문단",
            h7.get_str("grammar") == Some(naming::GRAMMAR_V7)
                && h7.get_str("base_model") == Some("deepseek-ai/DeepSeek-V4-Flash")
                && h7.get_str("brief") == Some("DS4F 1M 컨텍스트 spec7 셀 — 루브릭 바닥 미달(REFUTE)."),
        );
        ck(
            "★판정 열: v7 REFUTE 가 기계 표면에 보인다(출처 = 키별 출처)",
            h7.get_str("verdict") == Some("REFUTE") && h7.get_str("verdict_source").unwrap_or("").contains("sweep verdict.json"),
        );
        ck(
            "tagged_utc = 태그 오브젝트 tagger 시각(UTC · 픽스처 고정 시각)",
            h6.get_str("tagged_utc") == Some("2026-09-21T00:00:00Z") && hl.get_str("tagged_utc") == Some("2026-09-21T00:00:00Z"),
        );
        ck(
            "판정 열: 판정 칸 없는 v6 페이로드 = '—'(재판정 ✗ · P1)",
            h6.get_str("verdict") == Some(VERDICT_ABSENT) && hl.get_str("verdict") == Some(VERDICT_ABSENT),
        );
        ck("문법 열: v6 태그 = v6", h6.get_str("grammar") == Some(naming::GRAMMAR_V6));
        ck(
            "문법 열: 옛 태그 = 옛 세대(읽기 전용 수용)",
            matches!(hl.get_str("grammar"), Some(g) if g == naming::GRAMMAR_LEGACY_ARCH_NODE || g == naming::GRAMMAR_LEGACY_ARCH),
        );
        ck("v6 base_model = PAYLOAD.identity.base_model", h6.get_str("base_model") == Some("Qwen/Qwen3.8-Flash-Next"));
        ck("★옛 태그에는 base_model 을 싣지 않는다(v6 만 · 옛 PAYLOAD 를 v6 로 읽지 않는다)", hl.get("base_model").is_none());
        ck(
            "v6 brief = annotation 첫 문단(두 줄 → 한 줄 · 포인터·footer 제외)",
            h6.get_str("brief") == Some("NVFP4 체크포인트를 GB10 2노드(TP=2 · Ray)에서 262144 컨텍스트로 서빙한 여정. PLE mmap 이 생사를 가른 축이었다."),
        );
        ck("옛 태그 brief = 옛 규칙(주석 건너뜀)", hl.get_str("brief") == Some("GB10 x2 클러스터 · 네이티브 예산"));
        ck(
            "anchor = 페이로드 커밋 · object = 태그 오브젝트",
            h6.get_str("anchor") == Some(v6_commit.as_str()) && h6.get_str("object") == Some(g(&repo, &["rev-parse", &format!("refs/tags/{v6_tag}")])?.as_str()),
        );
        ck(
            "★페이로드 lite 선언 → bench_mode lite(선언) · 결손 BENCH_MODE_LITE",
            h6.get_str("bench_mode") == Some("lite(선언)")
                && h6.get("missing") == Some(&J::from(vec!["BENCH_MODE_LITE"]))
                && h6.get_str("bench_mode_source").unwrap_or("").starts_with("payload("),
        );
        ck(
            "★과거 태그(measurement_config 없음) bench_mode = 미기재(full·lite 로 접지 않는다)",
            hl.get_str("bench_mode") == Some(BENCH_MODE_ABSENT) && hl.get_str("bench_mode_source").unwrap_or("").starts_with("absent("),
        );
        ck("발행 사실 = 원격 존재", h6.get("published") == Some(&J::Bool(true)) && h6.get_str("source") == Some("remote-derived"));
        let pre = format!("{md_head}{HINTS_BEGIN}\n");
        let post = format!("{HINTS_END}\n{md_tail}");
        ck("★마커 밖 사람 서사(머리·꼬리) 바이트 불변 · 구간 안 옛 행은 사라진다", md.starts_with(&pre) && md.ends_with(&post) && !md.contains("| 옛 |"));
        let blk = md.get(pre.len()..md.len().saturating_sub(post.len())).unwrap_or("").to_string();
        ck("구간 = render_derived_block(index) 그대로", blk == render_derived_block(&idx)?);
        let blines = py_splitlines(&blk, false);
        let first = blines.first().copied().unwrap_or("");
        ck("구간 머리 한 줄: 태그 수 · 원격 이름 · 주입 KST", first.starts_with("> 원격 `fx` 의 발행 태그 **3건**") && first.contains(&format!("`{kst}` KST")));
        let heads: Vec<&str> = blines.iter().copied().filter(|l| l.starts_with("### ")).collect();
        ck(
            &format!("모델별 묶음 · 동시각이면 제목순({heads:?})"),
            heads == ["### `deepseek-v4-flash-0731` (1)", "### `gpt-oss-120b` (1)", "### `qwen3.8-flash-next-nvfp4` (1)"],
        );
        let rows: Vec<&str> = blines.iter().copied().filter(|l| l.starts_with("| `hint/")).collect();
        let hdrs: Vec<&str> = blines.iter().copied().filter(|l| l.starts_with("| 태그 |")).collect();
        ck(
            "묶음마다 헤더 1 · 행 3 · 헤더 = 6열",
            rows.len() == 3 && hdrs.len() == 3 && hdrs[0] == format!("| {} |", CATALOG_COLUMNS.join(" | ")),
        );
        ck("HINTS.md 표에 v7 행의 REFUTE", rows.iter().any(|rw| rw.contains(v7_tag) && rw.contains("| REFUTE |")));
        let legacy_cell = format!("| {VERDICT_LEGACY_CELL} |");
        ck(
            "★판정 칸 없는 v6·구판 행 = 미기재(구판)(index 값 '—' 는 그대로)",
            rows.iter().any(|rw| rw.contains(v6_tag) && rw.contains(&legacy_cell)) && rows.iter().any(|rw| rw.contains(legacy_tag) && rw.contains(&legacy_cell)),
        );
        ck(
            "카탈로그 행은 헤더와 같은 열 수",
            !rows.is_empty() && rows.iter().chain(hdrs.iter()).all(|rw| rw.replace("\\|", "").matches('|').count() == CATALOG_COLUMNS.len() + 1),
        );
        ck("★격리 경로가 배포 파일에 새지 않는다", !md.contains(&ps(&root)) && !idx_text.contains(&ps(&root)));

        // ★K5
        let other = root.join("other-pc");
        g(&root, &["clone", "-q", &ps(&bare), &ps(&other)])?;
        let k5_tag = "hint/0.29.0/qwen3-4b/gb10-1g1n-main-native/qbf16-len32768-kvauto-plenone-specoff-graph";
        let k5c = payload_commit(&other, Some(&lj(r#"{"format": "hint-payload/v6", "identity": {"base_model": null}}"#)), "k5-remote")?;
        atag(&other, k5_tag, &k5c, "원격이 광고한 brief\n\n포인터\n")?;
        g(&other, &["push", "-q", "origin", &rs(k5_tag)])?;
        g(&repo, &["fetch", "-q", "--no-tags", "fx", &format!("refs/tags/{k5_tag}:refs/hint-remote-selftest/{k5_tag}")])?;
        let k5l = payload_commit(&repo, None, "k5-local")?;
        atag(&repo, k5_tag, &k5l, "로컬의 다른 brief\n")?;
        let (e, out) = err_of(|| derive(&repo, "fx", kst, false, false, false));
        let k5 = if e.is_none() { hints_by(&idx_p).get(k5_tag).cloned().unwrap_or(J::dict()) } else { J::dict() };
        ck(
            "★K5 로컬 ref ≠ 원격 → 원격 광고 오브젝트에서 파생 + 고지",
            e.is_none()
                && k5.get_str("brief") == Some("원격이 광고한 brief")
                && out.contains("다른 오브젝트")
                && k5.get_str("object") == Some(g(&repo, &["rev-parse", &format!("refs/hint-remote-selftest/{k5_tag}")])?.as_str()),
        );

        // ★원격 전용 태그
        let roc = payload_commit(&other, None, "remote-only")?;
        atag(&other, remote_only, &roc, "원격 전용 brief\n")?;
        g(&other, &["push", "-q", "origin", &rs(remote_only)])?;
        let (idx_b, md_b) = (read_b(&idx_p), read_b(&md_p));
        let (e, _) = err_of(|| derive(&repo, "fx", kst, false, false, false));
        ck(
            "★원격 전용 태그 w/o 로컬 오브젝트 → 전용 종료코드 4",
            e.as_ref().map(|e| e.code == "HINT_CATALOG_LOCAL_OBJECT_MISSING" && e.exit_code as i64 == EXIT_LOCAL_TAG_OBJECT_MISSING).unwrap_or(false),
        );
        ck("★rc 4 경로는 아무것도 쓰지 않는다(index.json·HINTS.md 바이트 불변)", read_b(&idx_p) == idx_b && read_b(&md_p) == md_b);
        let text = e.as_ref().map(HintError::render).unwrap_or_default();
        ck(
            "★S7 해소 명령은 조회한 그 원격 · 주 워크트리를 가리킨다(기본 원격 전량 태그 fetch 가 아니다)",
            text.contains(&format!("git -C {} fetch fx 'refs/tags/hint/*:refs/tags/hint/*'", ps(&repo))) && !text.contains("--tags"),
        );
        ck(
            "★자동 fetch 하지 않는다(원격 전용 태그는 여전히 로컬에 없다)",
            core::git_nocheck(&repo, &["rev-parse", "--verify", "--quiet", &format!("refs/tags/{remote_only}")])?.returncode != 0,
        );
        let wt = root.join("main.wt-other");
        g(&repo, &["worktree", "add", "-q", "--detach", &ps(&wt)])?;
        let _ = std::fs::create_dir_all(wt.join("hints"));
        wr(&wt.join(CENTRAL_FLAG), "selftest\n")?;
        let (e, _) = err_of(|| derive(&wt, "fx", kst, false, false, false));
        let text = e.as_ref().map(HintError::render).unwrap_or_default();
        ck(
            "★S7 임시 워크트리에서 파생해도 fetch 안내는 주 워크트리(곧 지워질 경로를 찍지 않는다)",
            e.as_ref().map(|e| e.exit_code as i64 == EXIT_LOCAL_TAG_OBJECT_MISSING).unwrap_or(false)
                && text.contains(&format!("git -C {} fetch fx", ps(&repo)))
                && !text.contains(&ps(&wt)),
        );
        g(&repo, &["worktree", "remove", "--force", &ps(&wt)])?;
        let (e, _) = err_of(|| derive(&repo, "fx", kst, true, false, false));
        ck("dry-run 도 부재를 삼키지 않는다(rc 4)", e.as_ref().map(|e| e.exit_code as i64 == EXIT_LOCAL_TAG_OBJECT_MISSING).unwrap_or(false));

        // --record-missing
        let (e, out) = err_of(|| derive(&repo, "fx", kst, false, true, false));
        let ro = if e.is_none() { hints_by(&idx_p).get(remote_only).cloned().unwrap_or(J::dict()) } else { J::dict() };
        ck(
            "record-missing → rc 0 · absent-local 행(brief '—' · missing None · 미수령)",
            e.is_none()
                && ro.get_str("object") == Some(OBJECT_ABSENT_LOCAL)
                && ro.get_str("brief") == Some(BRIEF_UNRECEIVED)
                && ro.get("missing") == Some(&J::Null)
                && ro.get_str("bench_mode") == Some(BENCH_MODE_UNRECEIVED)
                && ro.get("published") == Some(&J::Bool(true)),
        );
        ck("record-missing NOTE 가 부재와 해소 명령을 말한다", out.contains("NOTE") && out.contains("fetch fx"));
        let md_rm = read_s(&md_p);
        ck(
            "record-missing 행이 HINTS.md 에 미수령으로 실린다(합성 ✗)",
            md_rm.contains(&format!("`{remote_only}`")) && !md_rm.contains("원격 전용 brief") && md_rm.contains("미수령"),
        );
        let idx_b = read_b(&idx_p);
        let shaken = md_rm.replace("미수령", "미수령 ");
        wr(&md_p, &shaken)?;
        let (e, _) = err_of(|| derive(&repo, "fx", kst, true, true, false));
        ck("dry-run 은 쓰지 않는다", e.is_none() && read_b(&idx_p) == idx_b && read_s(&md_p) == shaken);
        wr(&md_p, &md_rm)?;

        // match
        let index = load_index(&repo)?;
        let res = r#match(&index, "0.29.0rc6", "Qwen3.8-Flash-Next", None, false)?;
        ck(
            "★match 가 base_model 관계 태그를 찾는다(질의 = 기반 모델)",
            res.iter().map(|r| r["tag"].display()).collect::<Vec<_>>() == [v6_tag] && res[0].get_str("relation") == Some(REL_DERIVED),
        );
        let res = r#match(&index, "0.29.0rc6", "Qwen3.8-Flash-Next-NVFP4", Some("gb10-1g2n-cluster-native"), false)?;
        ck(
            "match 정규화 슬러그 동치(대소문자) · 정확 일치",
            !res.is_empty()
                && res[0].get_str("tag") == Some(v6_tag)
                && res[0].get_str("relation") == Some(REL_SAME)
                && res[0]["vllm_match"].truthy()
                && res[0].get_str("arch_match") == Some("exact"),
        );
        ck(
            "★match 는 관계없는 모델을 기본으로 숨긴다",
            res.iter().all(|r| r.get_str("relation") != Some("other-model")) && !res.iter().any(|r| r.get_str("tag") == Some(legacy_tag)),
        );
        let res_all = r#match(&index, "0.29.0rc6", "qwen3.8-flash-next-nvfp4", None, true)?;
        ck(
            "--include-other 면 다른 모델도 보이고 관계 행이 먼저 온다",
            res_all.iter().any(|r| r.get_str("tag") == Some(legacy_tag)) && res_all.first().and_then(|r| r.get_str("tag")) == Some(v6_tag),
        );
        let sib_index = jd! {"hints" => vec![
            by.get(v6_tag).cloned().unwrap_or(J::dict()),
            lj(r#"{"tag": "hint/x/qwen3.8-flash-next-fp8/a/r", "model": "qwen3.8-flash-next-fp8", "vllm": "0.29.0", "arch": "a", "base_model": "Qwen/Qwen3.8-Flash-Next"}"#),
            lj(r#"{"tag": "hint/y/qwen3.8-flash-next/a/r", "model": "qwen3.8-flash-next", "vllm": "0.29.0", "arch": "a"}"#),
        ]};
        let rels: std::collections::HashMap<String, String> = r#match(&sib_index, "0.29.0", "qwen3.8-flash-next-nvfp4", None, false)?
            .iter()
            .map(|r| (r["tag"].display(), r["relation"].display()))
            .collect();
        ck(
            "match 형제(shared-base) · 기반(base-of-query) 관계 · schema 2 항목(grammar 없음) 수용",
            rels.get("hint/x/qwen3.8-flash-next-fp8/a/r").map(String::as_str) == Some(REL_SIBLING)
                && rels.get("hint/y/qwen3.8-flash-next/a/r").map(String::as_str) == Some(REL_BASE),
        );
        let m2 = r#match(&index, "0.29.0rc6", "qwen3.8-flash-next-nvfp4", None, false)?;
        ck("format_match: 숨김 수·안내", format_match(&index, &J::from(m2), "qwen3.8-flash-next-nvfp4", false)?.contains("숨김"));
        let (e, _) = err_of(|| r#match(&index, "", "m", None, false));
        ck("★match 빈 질의 거부", e.as_ref().map(|e| e.code == "HINT_CATALOG_MATCH_QUERY_EMPTY").unwrap_or(false));

        // ★중앙 권위 플래그 부재
        let _ = std::fs::remove_file(repo.join(CENTRAL_FLAG));
        let (idx_b, md_b) = (read_b(&idx_p), read_b(&md_p));
        let (e, _) = err_of(|| derive(&repo, "fx", kst, false, true, false));
        ck(
            "★중앙 권위 플래그 부재 → 파생 거부(D8 권한 비대칭) · 여는 법을 말한다",
            e.as_ref().map(|e| e.code == "HINT_CATALOG_CENTRAL_FLAG_ABSENT" && e.remedy.as_deref().unwrap_or("").contains("printf")).unwrap_or(false),
        );
        ck("★플래그 부재 거부는 아무것도 쓰지 않는다", read_b(&idx_p) == idx_b && read_b(&md_p) == md_b);
        wr(&repo.join(CENTRAL_FLAG), "selftest\n")?;

        // ★기존 HINTS.md 마커 손상
        wr(&md_p, "# 카탈로그가 아닌 파일\n")?;
        let idx_b = read_b(&idx_p);
        let (e, _) = err_of(|| derive(&repo, "fx", kst, false, true, false));
        ck(
            "★기존 HINTS.md 마커 부재 → 중단 · index.json 도 쓰지 않는다",
            e.as_ref().map(|e| e.code == "HINT_CATALOG_MARKER_ABSENT").unwrap_or(false) && read_b(&idx_p) == idx_b && read_s(&md_p) == "# 카탈로그가 아닌 파일\n",
        );
        let (e, _) = err_of(|| derive(&repo, "fx", kst, true, true, false));
        ck(
            "★dry-run 도 마커 부재에서 멈춘다(실제 실행의 중단을 초록으로 예측하지 않는다)",
            e.as_ref().map(|e| e.code == "HINT_CATALOG_MARKER_ABSENT").unwrap_or(false),
        );
        let dup = format!("# 서사\n{HINTS_BEGIN}\na\n{HINTS_END}\n{HINTS_BEGIN}\nb\n{HINTS_END}\n");
        wr(&md_p, &dup)?;
        let (e, _) = err_of(|| derive(&repo, "fx", kst, false, true, false));
        ck(
            "★derive: 마커 중복 → 중단 · 두 파일 모두 쓰지 않는다",
            e.as_ref().map(|e| e.code == "HINT_CATALOG_MARKER_DUPLICATE").unwrap_or(false) && read_b(&idx_p) == idx_b && read_s(&md_p) == dup,
        );
        let (e, _) = err_of(|| derive(&repo, "--upload-pack=touch x", kst, false, false, false));
        ck(
            "★'-' 로 시작하는 원격 인자는 git 옵션이 되기 전에 거부",
            e.as_ref().map(|e| e.code == "HINT_CATALOG_REMOTE_INVALID").unwrap_or(false) && !repo.join("x").exists(),
        );
        let (e, _) = err_of(|| derive(&repo, "fx", "2026-09-21 19:00", false, false, false));
        ck("★주입 시각 형식 위반 거부", e.as_ref().map(|e| e.code == "HINT_TIME_NOT_INJECTED").unwrap_or(false));
    }

    // ★원격 조회 실패
    let nope = ps(&root.join("does-not-exist.git"));
    let (e, _) = err_of(|| remote_hint_tags(&repo, &nope, false));
    ck("★음성대조 원격 조회 실패 → 중단(캐시 폴백 금지)", e.as_ref().map(|e| e.code == "HINT_CATALOG_REMOTE_QUERY_FAILED").unwrap_or(false));
    let (e, _) = err_of(|| remote_hint_tags(&repo, &nope, true));
    ck("★음성대조 allow_empty 여도 원격 조회 실패는 중단", e.as_ref().map(|e| e.code == "HINT_CATALOG_REMOTE_QUERY_FAILED").unwrap_or(false));
    let idx_b = read_b(&idx_p);
    let (e, _) = err_of(|| derive(&repo, &nope, kst, false, false, false));
    ck("★원격 조회 실패 derive 는 쓰지 않는다", e.is_some() && read_b(&idx_p) == idx_b);
    let empty = root.join("empty.git");
    g(&root, &["init", "--bare", "-q", &ps(&empty)])?;
    let (e, _) = err_of(|| remote_hint_tags(&repo, &ps(&empty), false));
    ck("★음성대조 원격 hint 태그 0건 → 빈 카탈로그 거부", e.as_ref().map(|e| e.code == "HINT_CATALOG_REMOTE_EMPTY").unwrap_or(false));
    let (e, _) = err_of(|| remote_hint_tags(&repo, &ps(&empty), true));
    ck("allow_empty=True → 0건 정상 통과", e.is_none() && remote_hint_tags(&repo, &ps(&empty), true)? == J::dict());
    let live = remote_hint_tags(&repo, &ps(&bare), false)?;
    ck(
        "살아 있는 원격에서 (오브젝트, 피일) 광고를 읽는다",
        live.get(v6_tag).is_some()
            && live[v6_tag].get_str("object") == Some(g(&repo, &["rev-parse", &format!("refs/tags/{v6_tag}")])?.as_str())
            && live[v6_tag].get_str("peeled") == Some(v6_commit.as_str()),
    );

    // ★옛 4세그먼트 · 규약 밖 이름
    let odd = root.join("odd.git");
    g(&root, &["init", "--bare", "-q", &ps(&odd)])?;
    for nm in ["hint/0.1/m/a", "hint/0.2/m/a/r/extra"] {
        atag(&repo, nm, "HEAD", "옛 이름의 brief\n")?;
        g(&repo, &["push", "-q", &ps(&odd), &rs(nm)])?;
    }
    let rows_of = |remote: &Path, record_missing: bool| -> (Option<HintError>, std::collections::HashMap<String, J>) {
        let r = remote_hint_tags(&repo, &ps(remote), false)
            .and_then(|t| derive_entries(&repo, &t, Some(&ps(remote)), record_missing));
        match r {
            Ok((ents, _)) => (None, ents.iter().map(|x| (x["tag"].display(), x.clone())).collect()),
            Err(e) => (Some(e), Default::default()),
        }
    };
    let (e, ents) = rows_of(&odd, false);
    let nul = J::dict();
    let ex = ents.get("hint/0.2/m/a/r/extra").unwrap_or(&nul);
    ck(
        "★옛 4세그먼트 이름은 legacy-4seg 행 · 규약 밖 이름은 unknown 행(필드 추측 ✗) — 중단 ✗",
        e.is_none()
            && ents.get("hint/0.1/m/a").and_then(|r| r.get_str("grammar")) == Some(naming::GRAMMAR_LEGACY_4SEG)
            && ents.get("hint/0.1/m/a").and_then(|r| r.get_str("brief")) == Some("옛 이름의 brief")
            && ex.get_str("grammar") == Some(naming::GRAMMAR_UNKNOWN)
            && ex["model"].is_null()
            && ex.get("base_model").is_none(),
    );
    let no_pl = ents.get("hint/0.1/m/a").cloned().unwrap_or(J::dict());
    ck(
        "★PAYLOAD.json 없는 태그의 결손은 선언 0(`[]`·`—`)이 아니라 None · 표는 '미선언'",
        e.is_none()
            && no_pl["missing"].is_null()
            && no_pl.get_str("bench_mode") == Some(BENCH_MODE_ABSENT)
            && no_pl.get_str("bench_mode_source").unwrap_or("").contains("PAYLOAD.json 을 읽지 못함")
            && render_rows_v(&[no_pl.clone()])?.contains(&format!("| {MISSING_UNDECLARED} |")),
    );
    let old_fmt = root.join("oldfmt.git");
    g(&root, &["init", "--bare", "-q", &ps(&old_fmt)])?;
    let (nokey_tag, zero_tag) = ("hint/0.18.0/m/gb10-sim-h100/r", "hint/0.18.0/m/gb10-main-native/r");
    let c1 = payload_commit(&repo, Some(&lj(r#"{"identity": {"gpu": "NVIDIA GB10"}}"#)), "nokey")?;
    atag(&repo, nokey_tag, &c1, "옛 형식\n")?;
    let c2 = payload_commit(&repo, Some(&lj(r#"{"missing": []}"#)), "zero")?;
    atag(&repo, zero_tag, &c2, "결손 0 선언\n")?;
    g(&repo, &["push", "-q", &ps(&old_fmt), &rs(nokey_tag), &rs(zero_tag)])?;
    let (e, of) = rows_of(&old_fmt, false);
    let nk = of.get(nokey_tag).cloned().unwrap_or(J::dict());
    let zr = of.get(zero_tag).cloned().unwrap_or(J::dict());
    ck(
        "★missing 키 없는 옛 페이로드 = None('미선언') · 빈 목록 선언 = [](`—`)",
        e.is_none()
            && nk["missing"].is_null()
            && zr.get("missing") == Some(&J::list())
            && render_rows_v(&[nk.clone()])?.contains(&format!("| {MISSING_UNDECLARED} |"))
            && render_rows_v(&[zr.clone()])?.contains("| — |"),
    );
    // ★lightweight
    let lw = root.join("lw.git");
    g(&root, &["init", "--bare", "-q", &ps(&lw)])?;
    let lw_tag = "hint/0.19.0/gpt-oss-20b/gb10-main-native/qmxfp4-len131072-kvfp8";
    g(&repo, &["tag", lw_tag])?;
    g(&repo, &["push", "-q", &ps(&lw), &rs(lw_tag)])?;
    let (e, _) = err_of(|| derive(&repo, &ps(&lw), kst, false, false, false));
    ck(
        "★음성대조 lightweight 태그 거부 · 전용 종료코드가 아니다(원인을 섞지 않는다 · S7)",
        e.as_ref().map(|e| e.code == "HINT_CATALOG_NOT_ANNOTATED" && e.exit_code as i64 != EXIT_LOCAL_TAG_OBJECT_MISSING).unwrap_or(false),
    );
    let (e, lw_rows) = rows_of(&lw, true);
    let lwr = lw_rows.get(lw_tag).cloned().unwrap_or(J::dict());
    let head_sha = g(&repo, &["rev-parse", "HEAD"])?;
    ck(
        "★record-missing 은 lightweight 를 not-annotated 행(brief '—' · 합성 ✗)으로 싣는다 · 중단 ✗",
        e.is_none()
            && lwr.get_str("object") == Some(OBJECT_NOT_ANNOTATED)
            && lwr.get_str("brief") == Some(BRIEF_UNRECEIVED)
            && lwr.get_str("anchor") == Some(head_sha.as_str())
            && lwr.get("published") == Some(&J::Bool(true)),
    );
    let lw_abs = "hint/0.19.0/gpt-oss-20b/gb10-sub-native/qmxfp4-len131072-kvfp8";
    let lw_src = root.join("lw-src");
    std::fs::create_dir_all(&lw_src).map_err(|e| core::io_err(&e, &lw_src))?;
    g(&lw_src, &["init", "-q"])?;
    let other_c = payload_commit(&lw_src, None, "lw-remote-only")?;
    g(&lw_src, &["tag", lw_abs, &other_c])?;
    g(&lw_src, &["push", "-q", &ps(&lw), &rs(lw_abs)])?;
    let (e, la_rows) = rows_of(&lw, true);
    let la = la_rows.get(lw_abs).cloned().unwrap_or(J::dict());
    ck(
        "★로컬 부재 lightweight 원격 태그의 anchor = 광고 오브젝트(피일 줄 없음 ≠ 대상 모름)",
        e.is_none() && la.get_str("object") == Some(OBJECT_ABSENT_LOCAL) && la.get_str("anchor") == Some(other_c.as_str()),
    );
    let _ = std::fs::remove_file(&md_p);
    let (e, out) = err_of(|| derive(&repo, &ps(&lw), kst, true, true, false));
    ck("record-missing 의 not-annotated NOTE 가 소리낸다", e.is_none() && out.contains("annotated 가 아닌 원격 태그 1건"));
    ck("HINTS.md 부재 + dry-run = 골격 NOTE · 쓰기 0", e.is_none() && out.contains("골격") && !md_p.exists());
    let (e, out) = err_of(|| derive(&repo, &ps(&lw), kst, false, true, false));
    let md_new = if md_p.exists() { read_s(&md_p) } else { String::new() };
    ck(
        "HINTS.md 부재 → 골격(머리 주석 + 마커 쌍) 생성 + NOTE(서사를 지어내지 않는다)",
        e.is_none()
            && out.contains("골격")
            && md_new.starts_with(&format!("{HINTS_SCAFFOLD_HEAD}{HINTS_BEGIN}\n"))
            && md_new.ends_with(&format!("{HINTS_END}\n"))
            && !md_new.split(HINTS_BEGIN).next().unwrap_or("").contains("| 미기재(구판) |"),
    );
    drop(td);
    Ok(())
}
