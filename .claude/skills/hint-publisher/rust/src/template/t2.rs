//! template part 2 — port of `scripts/hintlib/template.py` lines 2187–4081: the document model (`_Doc`), template
//! loading, the restricted hint-event YAML, derived blocks, scaffold render/refresh/update, seal, brief, and the linter.
//!
//! Callable injection hooks of the Python (`substitute`, `source_text`, `resolve_stem`, `snapshot_origin`) are the
//! [`TemplateHooks`] of the `*_with` variants; the stub-signature functions (`lint`, `substituted_source`) take `&J` for
//! them and accept only `J::Null` (= the Python default `None`).
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::pyre::{group, named, Re};
use crate::value::{py_repr, Map, J};
use crate::{jd, jl};
use super::*;

// ── private copies of part-1 constants/patterns (same values as template.py:169–357) ──────────
// Part 1 owns these names; t2 keeps private copies so it compiles independently of how t1 spells them.
crate::re_static!(rx_fact_open, r"^<!--\s*FACT:([a-z0-9_]+)\s*-->\s*$");
crate::re_static!(rx_fact_close, r"^<!--\s*/FACT:([a-z0-9_]+)\s*-->\s*$");
crate::re_static!(rx_bench_mark, r"^<!--\s*BENCH_SECTION\s*-->\s*$");
crate::re_static!(rx_prompt_open, r"^<!--\s*PROMPT\s*$");
crate::re_static!(rx_h2, r"^##\s+(.+?)\s*$");
crate::re_static!(rx_chapter, r"^(\d+\.\d+)\s+(.+)$");
crate::re_static!(rx_fence, r"^\s*```");
crate::re_static!(rx_fence_close, r"^\s*```\s*$");
crate::re_static!(rx_event_fence, r"^```hint-event\s*$");
crate::re_static!(rx_excerpt_any, r"^>\s*\[원문\]");
crate::re_static!(rx_excerpt_head, r"^>\s*\[원문\]\s+(?P<stem>[^\s§]+)\s+§\s*(?P<section>\S.*?)\s*$");
crate::re_static!(rx_selfcheck_open, r"^<!--\s*SELFCHECK\s*$");
crate::re_static!(rx_selfcheck_residue, r"<!--\s*SELFCHECK\b");
crate::re_static!(rx_prompt_field, r"^([^\s:][^:]*?):\s*(.*)$");
crate::re_static!(rx_event_key, r"^([^\s:#][^\s:]*)\s*:\s*(.*)$");
crate::re_static!(rx_readme_token, r"\{\{([A-Z_]+)\}\}");
crate::re_static!(rx_ws, r"\s+");
crate::re_static!(rx_ellipsis, r"…|\.\.\.");
crate::re_static!(rx_md_heading, r"^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$");
crate::re_static!(rx_line_label, r"^L(\d+)-(\d+)$");
crate::re_static!(rx_snapshot_token, r"[^/\s@]+@[0-9a-f]{7,40}");
crate::re_static!(rx_snapshot_origin, r"^git:([0-9a-f]{40}):([A-Za-z0-9._/-]+)\Z");
crate::re_static!(rx_fact_placeholder, r"§\s*\d+\.[xX]\b");
// `_BRIEF_NUM` without its look-ahead group (the unit = the `[A-Za-z]*` run after the match, computed by hand).
crate::re_static!(rx_brief_num, r"(?<![0-9A-Za-z_.\-/])(\d[\d,]*(?:\.\d+)*)");
crate::re_static!(rx_any_num, r"\d[\d,]*(?:\.\d+)*");
crate::re_static!(rx_wall_id, r"(?<![A-Za-z0-9])W\d+(?![0-9])");
crate::re_static!(rx_block_item, r"([a-z-]+)\s*(?:>=\s*(\d+))?");
crate::re_static!(rx_block_split, r"[,·]");
crate::re_static!(rx_condition, r"([a-z_]+)\s*=\s*(\S+)");
// OWNER-MIRROR: hintlib/branch.py::PROMPT_RESIDUE_RE (the residue pattern's single owner; branch.rs is another agent's)
crate::re_static!(rx_prompt_residue, r"<!--\s*PROMPT\b");
crate::re_static!(_H3, r"^###\s+(.+?)\s*$");
crate::re_static!(_KEY_SPLIT, r"\s|—|·|\(|:");

const NUM_UNITS: [&str; 16] = ["", "MiB", "GiB", "KiB", "TiB", "MB", "GB", "KB", "TB", "B", "ms", "s", "us", "W", "MHz", "GHz"];
const CURRENT_STATUSES_T2: [&str; 4] = ["확증", "반증", "가설(강등)", "미결"];

fn payload_docs_all() -> Vec<String> {
    PAYLOAD_DOCS.iter().map(|n| n.to_string()).collect()
}

fn chapters_of(name: &str) -> Vec<String> {
    let (p, n) = match name {
        "00-hint" => (0, 9),
        "01-artifacts" => (1, 6),
        "02-narrative" => (2, 7),
        "03-benchmark" => (3, 5),
        _ => return Vec::new(),
    };
    (1..=n).map(|i| format!("{p}.{i}")).collect()
}

fn banner_lines() -> Vec<&'static str> {
    BANNER_LINES.to_vec()
}

/// `FACT_RENDERERS[fid]` — part 1's dispatch table (single owner).
fn fact_renderer(fid: &str) -> Option<FactRenderer> {
    FACT_RENDERERS().iter().find(|(k, _)| *k == fid).map(|(_, f)| *f)
}

// ── small Python-string helpers ──────────────────────────────────────────────────────────────
fn pws(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn pstrip(s: &str) -> &str {
    s.trim_matches(pws)
}
fn plstrip(s: &str) -> &str {
    s.trim_start_matches(pws)
}
fn prstrip(s: &str) -> &str {
    s.trim_end_matches(pws)
}
fn strip_set<'a>(s: &'a str, set: &str) -> &'a str {
    s.trim_matches(|c| set.contains(c))
}
/// `str.split()` (whitespace runs).
fn psplit(s: &str) -> Vec<&str> {
    s.split(pws).filter(|x| !x.is_empty()).collect()
}
fn head(s: &str, n: usize) -> String {
    core::head_chars(s, n)
}
fn clen(s: &str) -> usize {
    s.chars().count()
}
fn list_repr(v: &[String]) -> String {
    J::List(v.iter().map(|s| J::from(s.as_str())).collect()).repr()
}
fn tuple_repr(v: &[String]) -> String {
    let parts: Vec<String> = v.iter().map(|s| py_repr(s)).collect();
    if parts.len() == 1 {
        format!("({},)", parts[0])
    } else {
        format!("({})", parts.join(", "))
    }
}
fn casefold(s: &str) -> String {
    s.to_lowercase().replace('ß', "ss")
}
fn opt_repr(v: Option<&str>) -> String {
    v.map(py_repr).unwrap_or_else(|| "None".to_string())
}
fn cell(v: &J) -> HR<String> {
    _cell(v, _MISSING_TEXT)
}
fn tbl(headers: &[&str], rows: Vec<Vec<String>>) -> HR<Vec<String>> {
    let h: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
    let r = J::List(rows.into_iter().map(|row| J::List(row.into_iter().map(J::Str).collect())).collect());
    _table(&h, &r)
}
fn splice(lines: &mut Vec<String>, start: usize, end: usize, new: Vec<String>) {
    let start = start.min(lines.len());
    let end = end.max(start).min(lines.len());
    lines.splice(start..end, new);
}
/// Python's exception type name for an I/O error.
fn io_type(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        std::io::ErrorKind::IsADirectory => "IsADirectoryError",
        std::io::ErrorKind::NotADirectory => "NotADirectoryError",
        _ => "OSError",
    }
}
/// `Path.read_text(encoding="utf-8")`: OSError / UnicodeDecodeError escape as Python built-ins.
fn read_utf8(p: &Path) -> HR<String> {
    let bytes = std::fs::read(p).map_err(|e| HintError::py(io_type(&e), core::io_err_text(&e, p)))?;
    String::from_utf8(bytes).map_err(|e| {
        let pos = e.utf8_error().valid_up_to();
        let b = e.as_bytes().get(pos).copied().unwrap_or(0);
        HintError::py("UnicodeDecodeError", format!("'utf-8' codec can't decode byte 0x{b:02x} in position {pos}: invalid start byte"))
    })
}
fn write_text(p: &Path, s: &str) -> HR<()> {
    std::fs::write(p, s).map_err(|e| HintError::py(io_type(&e), core::io_err_text(&e, p)))
}
fn is_file(p: &Path) -> bool {
    p.is_file()
}
/// pathlib `PurePosixPath(s).parent.as_posix()`.
fn py_parent(s: &str) -> String {
    let abs = s.starts_with('/');
    let parts: Vec<&str> = s.split('/').filter(|x| !x.is_empty() && *x != ".").collect();
    if parts.len() <= 1 {
        return if abs { "/".into() } else { ".".into() };
    }
    let p = parts[..parts.len() - 1].join("/");
    if abs {
        format!("/{p}")
    } else {
        p
    }
}
/// pathlib `.name` of a posix string.
fn py_name(s: &str) -> String {
    s.split('/').filter(|x| !x.is_empty() && *x != ".").last().unwrap_or("").to_string()
}
fn int_of(s: &str) -> i64 {
    s.parse::<i64>().unwrap_or(i64::MAX)
}
/// Python `int(x)` of a regex digit group (Unicode decimal digits).
fn int_digits(s: &str) -> i64 {
    let mut v: i64 = 0;
    for c in s.chars() {
        let d = c.to_digit(10).map(i64::from).or_else(|| unicode_digit(c)).unwrap_or(0);
        v = v.saturating_mul(10).saturating_add(d);
    }
    v
}
fn unicode_digit(c: char) -> Option<i64> {
    // Unicode Nd blocks are runs of ten starting at a code point ending in 0 (hex): good enough for `\d` matches.
    let cp = c as u32;
    if c.is_numeric() {
        Some(i64::from((cp & 0xF) % 10))
    } else {
        None
    }
}
fn py_isdigit(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || (c.is_numeric() && unicode_digit(c).is_some() && !c.is_ascii()))
}

// ── 문서 모델 ─────────────────────────────────────────────────────────────────────────────────
/// `_Prompt`: a `<!-- PROMPT … -->` comment (0-based line indices, inclusive end).
#[derive(Clone, Debug, PartialEq)]
pub struct _Prompt {
    pub start: usize,
    pub end: usize,
    pub raw: Vec<String>,
    /// `dict[str, str]` (a `J::Dict` of strings).
    pub fields: J,
    pub chapter: String,
}

/// `_Fact`: a fact block. Usually start = opening marker line · end = closing marker line; open_ended (03 load curve ·
/// `<!-- BENCH_SECTION -->`) = start marker line · end = next chapter heading line (outside · or end of document). Either
/// way the body is `lines[start+1:end]`.
#[derive(Clone, Debug, PartialEq)]
pub struct _Fact {
    pub fid: String,
    pub start: usize,
    pub end: usize,
    pub body: String,
    pub chapter: String,
    pub open_ended: bool,
}

/// `_Doc`: the line model of one markdown document. masked = comments (incl. PROMPT) and fact blocks (machine area).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct _Doc {
    pub name: String,
    pub text: String,
    pub lines: Vec<String>,
    pub masked: Vec<bool>,
    pub comment: Vec<bool>,
    pub fence: Vec<bool>,
    pub chapter_of: Vec<String>,
    pub facts: Vec<_Fact>,
    pub prompts: Vec<_Prompt>,
    pub chapters: Vec<(usize, String, String)>,
    pub other_h2: Vec<(usize, String)>,
    pub structure: Vec<(String, usize, String)>,
    pub selfchecks: Vec<(usize, usize)>,
}

/// The loaded templates (`dict name -> _Doc` in `TEMPLATE_FILES` order) — the type of part 1's `_Ctx.templates`.
pub type _Templates = Vec<(String, _Doc)>;

fn tmpl<'a>(t: &'a [(String, _Doc)], name: &str) -> HR<&'a _Doc> {
    t.iter().find(|(n, _)| n == name).map(|(_, d)| d).ok_or_else(|| HintError::py("KeyError", py_repr(name)))
}

impl _Doc {
    /// `_Doc.parse(name, text)`.
    pub fn parse(name: &str, text: &str) -> _Doc {
        let mut d = _Doc { name: name.to_string(), text: text.to_string(), ..Default::default() };
        d.lines = text.split('\n').map(str::to_string).collect();
        let n = d.lines.len();
        d.masked = vec![false; n];
        d.comment = vec![false; n];
        d.fence = vec![false; n];
        d.chapter_of = vec!["preamble".to_string(); n];
        let l = d.lines.clone();
        let mut i = 0usize;
        let mut chapter = "preamble".to_string();
        let mut in_fence = false;
        while i < n {
            let line = &l[i];
            if !in_fence {
                if let Some(m) = rx_fact_open().match_at_start(line) {
                    let fid = group(&m, 1).unwrap_or("").to_string();
                    let mut j = i + 1;
                    while j < n && !rx_fact_close().is_match(&l[j]) && !rx_fact_open().is_match(&l[j]) {
                        j += 1;
                    }
                    let close = if j < n { rx_fact_close().match_at_start(&l[j]).and_then(|c| group(&c, 1).map(str::to_string)) } else { None };
                    if close.as_deref() == Some(fid.as_str()) {
                        for k in i..=j {
                            d.masked[k] = true;
                            d.chapter_of[k] = chapter.clone();
                        }
                        d.facts.push(_Fact { fid, start: i, end: j, body: l[i + 1..j].join("\n"), chapter: chapter.clone(), open_ended: false });
                        i = j + 1;
                        continue;
                    }
                    d.structure.push(("HINT_FACT_BLOCK_UNCLOSED".into(), i, format!("FACT:{fid} 의 닫힘 표지가 없다")));
                    d.masked[i] = true;
                    d.chapter_of[i] = chapter.clone();
                    i += 1;
                    continue;
                }
                if rx_fact_close().is_match(line) {
                    d.structure.push(("HINT_FACT_BLOCK_STRAY".into(), i, "여는 표지 없는 FACT 닫힘 표지".into()));
                    d.masked[i] = true;
                    d.chapter_of[i] = chapter.clone();
                    i += 1;
                    continue;
                }
                let stripped = plstrip(line);
                if stripped.starts_with("<!--") {
                    let mut j = i;
                    if !stripped[4..].contains("-->") {
                        j = i + 1;
                        while j < n && !l[j].contains("-->") {
                            j += 1;
                        }
                        if j >= n {
                            d.structure.push(("HINT_COMMENT_UNCLOSED".into(), i, "HTML 주석이 닫히지 않았다".into()));
                            j = n - 1;
                        }
                    }
                    for k in i..=j {
                        d.masked[k] = true;
                        d.comment[k] = true;
                        d.chapter_of[k] = chapter.clone();
                    }
                    if rx_prompt_open().is_match(line) && j > i {
                        d.prompts.push(_Prompt {
                            start: i,
                            end: j,
                            raw: l[i..=j].to_vec(),
                            fields: _prompt_fields(&l[i + 1..j]).unwrap_or_default(),
                            chapter: chapter.clone(),
                        });
                    }
                    if rx_selfcheck_open().is_match(line) && j > i {
                        d.selfchecks.push((i, j));
                    }
                    if j == i && rx_bench_mark().is_match(line) {
                        let mut k = i + 1;
                        while k < n {
                            if let Some(hm) = rx_h2().match_at_start(&l[k]) {
                                if rx_chapter().is_match(group(&hm, 1).unwrap_or("")) {
                                    break;
                                }
                            }
                            k += 1;
                        }
                        for x in i + 1..k {
                            d.masked[x] = true;
                            d.chapter_of[x] = chapter.clone();
                        }
                        d.facts.push(_Fact {
                            fid: BENCH_REGION_ID.into(),
                            start: i,
                            end: k,
                            body: l[i + 1..k].join("\n"),
                            chapter: chapter.clone(),
                            open_ended: true,
                        });
                        i = k;
                        continue;
                    }
                    i = j + 1;
                    continue;
                }
                if let Some(h) = rx_h2().match_at_start(line) {
                    let t = group(&h, 1).unwrap_or("");
                    if let Some(cm) = rx_chapter().match_at_start(t) {
                        chapter = group(&cm, 1).unwrap_or("").to_string();
                        d.chapters.push((i, chapter.clone(), pstrip(group(&cm, 2).unwrap_or("")).to_string()));
                    } else {
                        d.other_h2.push((i, t.to_string()));
                    }
                }
            }
            if rx_fence().is_match(line) {
                d.fence[i] = true;
                in_fence = !in_fence;
            } else if in_fence {
                d.fence[i] = true;
            }
            d.chapter_of[i] = chapter.clone();
            i += 1;
        }
        d
    }

    /// Body without comments (incl. PROMPT) — fact block bodies are published text and stay in.
    pub fn visible(&self) -> String {
        self.lines.iter().enumerate().filter(|(i, _)| !self.comment[*i]).map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("\n")
    }

    pub fn fact(&self, fid: &str) -> Vec<&_Fact> {
        self.facts.iter().filter(|f| f.fid == fid).collect()
    }

    pub fn chapter_lines(&self, chapter: &str) -> Vec<usize> {
        self.chapter_of.iter().enumerate().filter(|(_, c)| *c == chapter).map(|(i, _)| i).collect()
    }

    /// The capture shim's `to_json` of the dataclass.
    pub fn to_j(&self) -> J {
        let bools = |v: &Vec<bool>| J::List(v.iter().map(|b| J::Bool(*b)).collect());
        let strs = |v: &Vec<String>| J::List(v.iter().map(|s| J::from(s.as_str())).collect());
        jd! {
            "__class__" => "_Doc",
            "name" => self.name.as_str(),
            "text" => self.text.as_str(),
            "lines" => strs(&self.lines),
            "masked" => bools(&self.masked),
            "comment" => bools(&self.comment),
            "fence" => bools(&self.fence),
            "chapter_of" => strs(&self.chapter_of),
            "facts" => J::List(self.facts.iter().map(_Fact::to_j).collect()),
            "prompts" => J::List(self.prompts.iter().map(_Prompt::to_j).collect()),
            "chapters" => J::List(self.chapters.iter().map(|(i, a, b)| jl![*i as i64, a.as_str(), b.as_str()]).collect()),
            "other_h2" => J::List(self.other_h2.iter().map(|(i, a)| jl![*i as i64, a.as_str()]).collect()),
            "structure" => J::List(self.structure.iter().map(|(c, i, m)| jl![c.as_str(), *i as i64, m.as_str()]).collect()),
            "selfchecks" => J::List(self.selfchecks.iter().map(|(a, b)| jl![*a as i64, *b as i64]).collect()),
        }
    }
}

impl _Fact {
    pub fn to_j(&self) -> J {
        jd! {"__class__" => "_Fact", "fid" => self.fid.as_str(), "start" => self.start as i64, "end" => self.end as i64,
             "body" => self.body.as_str(), "chapter" => self.chapter.as_str(), "open_ended" => self.open_ended}
    }
}

impl _Prompt {
    pub fn to_j(&self) -> J {
        jd! {"__class__" => "_Prompt", "start" => self.start as i64, "end" => self.end as i64,
             "raw" => J::List(self.raw.iter().map(|s| J::from(s.as_str())).collect()), "fields" => self.fields.clone(),
             "chapter" => self.chapter.as_str()}
    }
}

/// PROMPT body → {key: value}. Indented lines · lines inside a code fence continue the previous field.
pub fn _prompt_fields(body: &[String]) -> HR<J> {
    let mut fields: Vec<(String, Vec<String>)> = Vec::new();
    fn slot<'a>(f: &'a mut Vec<(String, Vec<String>)>, k: &str) -> &'a mut Vec<String> {
        if let Some(i) = f.iter().position(|(n, _)| n == k) {
            return &mut f[i].1;
        }
        f.push((k.to_string(), Vec::new()));
        &mut f.last_mut().unwrap().1
    }
    let mut cur: Option<String> = None;
    let mut in_fence = false;
    for raw in body {
        if rx_fence().is_match(raw) {
            in_fence = !in_fence;
            if let Some(c) = &cur {
                if !c.is_empty() {
                    slot(&mut fields, c).push(raw.clone());
                }
            }
            continue;
        }
        let m = if in_fence { None } else { rx_prompt_field().match_at_start(raw) };
        let key = m.as_ref().and_then(|m| group(m, 1)).filter(|k| PROMPT_KEYS.contains(k));
        if let (Some(k), Some(m)) = (key, m.as_ref()) {
            if fields.iter().any(|(n, _)| n == k) {
                slot(&mut fields, "__dup__").push(k.to_string());
            }
            *slot(&mut fields, k) = vec![group(m, 2).unwrap_or("").to_string()];
            cur = Some(k.to_string());
        } else if let Some(c) = &cur {
            slot(&mut fields, c).push(raw.clone());
        } else if !pstrip(raw).is_empty() {
            slot(&mut fields, "__stray__").push(raw.clone());
        }
    }
    let mut out = Map::new();
    for (k, v) in fields {
        out.insert(k, pstrip(&v.join("\n")).to_string());
    }
    Ok(J::Dict(out))
}

pub fn _question(fields: &J) -> HR<String> {
    _norm(fields.get_str("질문").unwrap_or(""))
}

/// Parsed machine directives of a PROMPT.
#[derive(Clone, Debug, PartialEq)]
struct Dir {
    blocks: Vec<(String, i64)>,
    excerpts: i64,
    condition: Option<(String, String)>,
    optional: bool,
}

fn directives(fields: &J) -> HR<Dir> {
    let mut blocks: Vec<(String, i64)> = Vec::new();
    for item in rx_block_split().split(fields.get_str("블록").unwrap_or("")) {
        let item = pstrip(&item).to_string();
        if item.is_empty() {
            continue;
        }
        let m = rx_block_item().fullmatch(&item);
        let kind = m.as_ref().and_then(|m| group(m, 1)).filter(|k| EVENT_KINDS.contains(k));
        let Some(kind) = kind else {
            return core::fail(
                "HINT_TEMPLATE_DIRECTIVE_INVALID",
                format!("`블록:` 항목을 해석할 수 없다: {}", py_repr(&item)),
                Some(&format!("형식 `<kind>[>=N]` · kind ∈ {}", EVENT_KINDS.join(", "))),
            );
        };
        let v = m.as_ref().and_then(|m| group(m, 2)).map(int_digits).unwrap_or(0);
        match blocks.iter_mut().find(|(k, _)| k == kind) {
            Some(e) => e.1 = v,
            None => blocks.push((kind.to_string(), v)),
        }
    }
    let raw_ex = pstrip(fields.get_str("필수 발췌").unwrap_or("")).to_string();
    if !raw_ex.is_empty() && !py_isdigit(&raw_ex) {
        return core::fail("HINT_TEMPLATE_DIRECTIVE_INVALID", format!("`필수 발췌:` 는 정수다: {}", py_repr(&raw_ex)), None);
    }
    let mut cond = None;
    let raw_cond = pstrip(fields.get_str("조건").unwrap_or("")).to_string();
    if !raw_cond.is_empty() {
        let m = rx_condition().fullmatch(&raw_cond);
        let ok = m.as_ref().and_then(|m| group(m, 1)).filter(|k| _CONDITION_KEYS.contains(k));
        let (Some(k), Some(m)) = (ok, m.as_ref()) else {
            return core::fail(
                "HINT_TEMPLATE_DIRECTIVE_INVALID",
                format!("`조건:` 을 해석할 수 없다: {}", py_repr(&raw_cond)),
                Some(&format!("형식 `<key>=<value>` · key ∈ {}", _CONDITION_KEYS.join(", "))),
            );
        };
        cond = Some((k.to_string(), group(m, 2).unwrap_or("").to_string()));
    }
    let opt = pstrip(fields.get_str("선택").unwrap_or(""));
    Ok(Dir {
        blocks,
        excerpts: if raw_ex.is_empty() { 0 } else { int_digits(&raw_ex) },
        condition: cond,
        optional: matches!(opt, "예" | "yes" | "true"),
    })
}

fn dir_j(d: &Dir) -> J {
    let mut b = Map::new();
    for (k, v) in &d.blocks {
        b.insert(k.as_str(), *v);
    }
    jd! {
        "blocks" => J::Dict(b),
        "excerpts" => d.excerpts,
        "condition" => match &d.condition { Some((k, v)) => jl![k.as_str(), v.as_str()], None => J::Null },
        "optional" => d.optional,
    }
}

fn cond_of(cond: &J) -> Option<(String, String)> {
    let l = cond.as_list()?;
    Some((l.first()?.display(), l.get(1)?.display()))
}

/// Machine directive interpretation (fail-loud on a template defect). `condition` = `[key, value]` | null.
pub fn _directives(fields: &J) -> HR<J> {
    Ok(dir_j(&directives(fields)?))
}

fn cond_holds(cond: Option<&(String, String)>, facts: &J) -> HR<(bool, String)> {
    let Some((key, want)) = cond else { return Ok((true, String::new())) };
    let actual: J = match key.as_str() {
        "topology" => J::from(_topology(facts)?),
        "task_class" => J::from(_task_class(facts)?),
        "plane" => facts["plane"].clone(),
        "cell_grade" => J::from(cell_grade(facts)?),
        _ => return Err(HintError::py("KeyError", py_repr(key))),
    };
    if actual.is_null() || actual.as_str() == Some("") {
        return core::fail(
            "HINT_TEMPLATE_CONDITION_UNDECIDABLE",
            format!("조건 `{key}={want}` 를 판정할 사실이 없다(facts 의 {key} 부재)."),
            Some("evidence/artifacts 가 그 사실을 관측해 facts 에 넣어야 한다 — 추측으로 채우지 않는다."),
        );
    }
    Ok((actual.as_str() == Some(want.as_str()), actual.display()))
}

pub fn _condition_holds(cond: &J, facts: &J) -> HR<(bool, String)> {
    cond_holds(cond_of(cond).as_ref(), facts)
}

fn na_line(cond: &(String, String), actual: &str) -> String {
    format!("> 해당 없음 — 이 절의 조건 `{}={}` 이 이 셀에서 성립하지 않는다(`{}={actual}`).", cond.0, cond.1, cond.0)
}

pub fn _na_line(cond: &J, actual: &str) -> HR<String> {
    let c = cond_of(cond).ok_or_else(|| HintError::py("TypeError", "'NoneType' object is not subscriptable"))?;
    Ok(na_line(&c, actual))
}

// ── 템플릿 적재 ───────────────────────────────────────────────────────────────────────────────
pub fn _templates_dir(repo: &Path) -> HR<PathBuf> {
    Ok(repo.join(core::REL_SKILL).join("templates"))
}

/// Read and shape-check the repository's 4 templates (tripwire · defect = fail-loud).
pub fn _load_templates(repo: &Path) -> HR<_Templates> {
    let mut out: _Templates = Vec::new();
    let tdir = _templates_dir(repo)?;
    for name in TEMPLATE_FILES {
        let fname = format!("{name}.prompt.md");
        let p = tdir.join(&fname);
        if !p.is_file() {
            return core::fail("HINT_TEMPLATE_ABSENT", format!("prompt 템플릿 부재: {}/templates/{fname}", core::REL_SKILL), None);
        }
        let doc = _Doc::parse(name, &read_utf8(&p)?);
        if let Some((code, i, msg)) = doc.structure.first() {
            return core::fail("HINT_TEMPLATE_SHAPE", format!("{fname}:{}: {code} {msg}", i + 1), None);
        }
        let nums: Vec<String> = doc.chapters.iter().map(|(_, n, _)| n.clone()).collect();
        let want = chapters_of(name);
        if nums != want {
            return core::fail(
                "HINT_TEMPLATE_SHAPE",
                format!("{fname} 챕터 {} ≠ plan §4.2 {}", tuple_repr(&nums), tuple_repr(&want)),
                Some("챕터 목록은 plan §4.2 가 정본이다 — 템플릿과 CHAPTERS 를 함께 리뷰한다."),
            );
        }
        let ids: Vec<&str> = doc.facts.iter().map(|f| f.fid.as_str()).collect();
        for fid in &ids {
            if fact_renderer(fid).is_none() && !DERIVED_FACTS.contains(fid) {
                return core::fail("HINT_TEMPLATE_SHAPE", format!("{fname}: 렌더러 없는 FACT:{fid}"), None);
            }
            if ids.iter().filter(|x| *x == fid).count() > 1 {
                return core::fail("HINT_TEMPLATE_SHAPE", format!("{fname}: FACT:{fid} 가 두 번 있다"), None);
            }
            if !pstrip(&doc.facts.iter().find(|f| f.fid == *fid).unwrap().body).is_empty() {
                return core::fail("HINT_TEMPLATE_SHAPE", format!("{fname}: 템플릿의 FACT:{fid} 는 비어 있어야 한다"), None);
            }
        }
        for pr in &doc.prompts {
            if pr.fields.get("__dup__").is_some() || pr.fields.get("__stray__").is_some() {
                return core::fail("HINT_TEMPLATE_SHAPE", format!("{fname}:{}: PROMPT 필드 중복 또는 키 없는 줄", pr.start + 1), None);
            }
            let d = directives(&pr.fields)?;
            let need: Vec<&str> = if d.optional { vec!["질문", "기재"] } else { _PROMPT_REQUIRED_KEYS.to_vec() };
            let miss: Vec<String> = need.iter().filter(|k| !pr.fields[**k].truthy()).map(|k| k.to_string()).collect();
            if !miss.is_empty() {
                return core::fail("HINT_TEMPLATE_SHAPE", format!("{fname}:{}: PROMPT 필수 키 부재 {}", pr.start + 1, list_repr(&miss)), None);
            }
        }
        out.push((name.to_string(), doc));
    }
    Ok(out)
}

// ── hint-event 제한 YAML ─────────────────────────────────────────────────────────────────────
/// `#` comment removal — only outside quotes and after whitespace (or at line start).
pub fn _strip_comment(s: &str) -> HR<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut q: Option<char> = None;
    for (i, &ch) in chars.iter().enumerate() {
        if let Some(qc) = q {
            if ch == qc {
                q = None;
            }
        } else if (ch == '"' || ch == '\'') && (i == 0 || " \t[,".contains(chars[i - 1])) {
            q = Some(ch);
        } else if ch == '#' && (i == 0 || " \t".contains(chars[i - 1])) {
            return Ok(chars[..i].iter().collect());
        }
    }
    Ok(s.to_string())
}

pub fn _unquote(item: &str) -> HR<(Option<String>, Option<String>)> {
    let item = pstrip(item);
    let mut it = item.chars();
    if let Some(q) = it.next().filter(|c| *c == '"' || *c == '\'') {
        let rest: &str = &item[q.len_utf8()..];
        let Some(end) = rest.find(q) else {
            return Ok((None, Some("따옴표가 닫히지 않았다".into())));
        };
        if !pstrip(&rest[end + q.len_utf8()..]).is_empty() {
            return Ok((None, Some("닫는 따옴표 뒤에 글자가 있다(값 전체를 다른 따옴표로 감싼다)".into())));
        }
        return Ok((Some(rest[..end].to_string()), None));
    }
    Ok((Some(item.to_string()), None))
}

/// One value line → (str | list[str] | null on error, error).
pub fn _parse_value(raw: &str) -> HR<(J, Option<String>)> {
    let sc = _strip_comment(raw)?;
    let s = pstrip(&sc);
    let Some(first) = s.chars().next() else { return Ok((J::from(""), None)) };
    if first == '"' || first == '\'' {
        let (v, e) = _unquote(s)?;
        return Ok((J::from(v), e));
    }
    if first == '[' {
        if !s.ends_with(']') {
            return Ok((J::Null, Some("인라인 리스트가 `]` 로 닫히지 않았다".into())));
        }
        let inner: Vec<char> = s.chars().collect();
        let inner = &inner[1..inner.len() - 1];
        let mut items: Vec<String> = Vec::new();
        let mut cur: Vec<char> = Vec::new();
        let mut q: Option<char> = None;
        for &ch in inner {
            if let Some(qc) = q {
                cur.push(ch);
                if ch == qc {
                    q = None;
                }
                continue;
            }
            if (ch == '"' || ch == '\'') && pstrip(&cur.iter().collect::<String>()).is_empty() {
                q = Some(ch);
                cur.push(ch);
                continue;
            }
            if ch == ',' {
                items.push(cur.iter().collect());
                cur.clear();
                continue;
            }
            cur.push(ch);
        }
        if q.is_some() {
            return Ok((J::Null, Some("리스트 항목의 따옴표가 닫히지 않았다".into())));
        }
        items.push(cur.iter().collect());
        let mut out: Vec<J> = Vec::new();
        for it in items {
            let it = pstrip(&it);
            if it.is_empty() {
                continue;
            }
            if it.starts_with('[') || it.starts_with('{') {
                return Ok((J::Null, Some("중첩 리스트·매핑은 허용하지 않는다".into())));
            }
            let (v, err) = _unquote(it)?;
            if err.is_some() {
                return Ok((J::Null, err));
            }
            out.push(J::from(v));
        }
        return Ok((J::List(out), None));
    }
    if "{|>&*!".contains(first) {
        return Ok((
            J::Null,
            Some(format!("`{first}` 로 시작하는 값은 제한 YAML 밖이다(매핑·블록 스칼라·앵커 ✗) — 값 전체를 따옴표로 감싼다")),
        ));
    }
    Ok((J::from(s), None))
}

/// Restricted YAML: `key: value` per line · returns (event, [(code, body line offset, message)]).
pub fn _parse_event_body(body: &[String]) -> HR<(J, Vec<(String, i64, String)>)> {
    let mut ev = Map::new();
    let mut errs: Vec<(String, i64, String)> = Vec::new();
    for (off, raw) in body.iter().enumerate() {
        let off = off as i64;
        if pstrip(raw).is_empty() || plstrip(raw).starts_with('#') {
            continue;
        }
        if raw.chars().next().is_some_and(pws) {
            errs.push(("HINT_EVENT_INVALID".into(), off, "들여쓴 줄(중첩)은 제한 YAML 밖이다 — 값은 한 줄".into()));
            continue;
        }
        let Some(m) = rx_event_key().match_at_start(raw) else {
            errs.push(("HINT_EVENT_INVALID".into(), off, format!("`키: 값` 줄이 아니다: {}", py_repr(&head(pstrip(raw), 60)))));
            continue;
        };
        let key = group(&m, 1).unwrap_or("");
        if ev.contains_key(key) {
            errs.push(("HINT_EVENT_INVALID".into(), off, format!("중복 키: {key}")));
            continue;
        }
        let (val, err) = _parse_value(group(&m, 2).unwrap_or(""))?;
        if let Some(e) = err {
            errs.push(("HINT_EVENT_INVALID".into(), off, format!("{key}: {e}")));
            continue;
        }
        ev.insert(key, val);
    }
    Ok((J::Dict(ev), errs))
}

fn unhashable(v: &J) -> HintError {
    HintError::py("TypeError", format!("unhashable type: '{}'", v.type_name()))
}

pub fn _validate_event(ev: &J) -> HR<Vec<(String, String)>> {
    let mut errs: Vec<(String, String)> = Vec::new();
    let kind_j = &ev["kind"];
    let Some(kind) = kind_j.as_str().filter(|k| EVENT_KINDS.contains(k)) else {
        return Ok(vec![("HINT_EVENT_KIND_UNKNOWN".into(), format!("kind 어휘 밖: {} (∈ {})", kind_j.repr(), EVENT_KINDS.join(", ")))]);
    };
    for key in EVENT_REQUIRED()[kind].items() {
        let key = key.as_str().unwrap_or("");
        let v = &ev[key];
        let empty = match v {
            J::Null => true,
            J::Str(s) => pstrip(s).is_empty(),
            J::List(l) => l.is_empty(),
            _ => false,
        };
        if empty {
            errs.push(("HINT_EVENT_FIELD_MISSING".into(), format!("{kind} 필수 필드 부재: {key}")));
        }
    }
    for key in ["id", "kind", "서명", "노브", "값", "지위", "전이등급", "현재지위"] {
        if ev[key].is_list() {
            errs.push(("HINT_EVENT_INVALID".into(), format!("{key} 는 리스트가 아니라 한 값이다")));
        }
    }
    let prefix = EVENT_ID_PREFIX()[kind].as_str().unwrap_or("");
    if let Some(eid) = ev["id"].as_str() {
        if !eid.is_empty() && !Re::new(&format!("{prefix}\\d+"), 0).is_full(eid) {
            errs.push(("HINT_EVENT_ID_SHAPE".into(), format!("{kind} 의 id 는 `{prefix}<n>` 이다: {}", py_repr(eid))));
        }
    }
    let member = |v: &J, dict: &J| -> HR<bool> {
        match v {
            J::List(_) | J::Dict(_) => Err(unhashable(v)),
            J::Str(s) => Ok(dict.get(s).is_some()),
            _ => Ok(false),
        }
    };
    let tc = &ev["전이등급"];
    if (kind == "wall" || kind == "misdiagnosis") && tc.truthy() && !member(tc, TRANSFER_CLASSES())? {
        errs.push(("HINT_EVENT_VOCAB".into(), format!("전이등급 어휘 밖: {}", tc.repr())));
    }
    let st = &ev["지위"];
    if kind == "value-status" && st.truthy() && !member(st, VALUE_STATUSES())? {
        errs.push(("HINT_EVENT_VOCAB".into(), format!("지위 어휘 밖: {}", st.repr())));
    }
    let cs = &ev["현재지위"];
    if cs.truthy() && !cs.as_str().is_some_and(|s| CURRENT_STATUSES_T2.contains(&s)) {
        errs.push(("HINT_EVENT_VOCAB".into(), format!("현재지위 어휘 밖: {}", cs.repr())));
    }
    Ok(errs)
}

pub fn _sources_of(ev: &J) -> HR<Vec<String>> {
    let raw = &ev["출처"];
    let items: Vec<J> = match raw {
        J::List(l) => l.clone(),
        J::Str(s) if !pstrip(s).is_empty() => vec![raw.clone()],
        _ => Vec::new(),
    };
    Ok(items.iter().map(|x| pstrip(&x.display()).to_string()).filter(|x| !x.is_empty()).collect())
}

pub fn _source_token(entry: &str) -> HR<String> {
    let h = entry.split('§').next().unwrap_or("");
    let h = strip_set(pstrip(h), "`");
    Ok(psplit(h).first().map(|s| s.to_string()).unwrap_or_default())
}

pub const _TRIPLET_ARTIFACT_PREFIX: &str = "artifacts/triplet/";

/// A source location `(kind, key)`: kind ∈ artifact | snapshot | lineage.
pub type _Loc = (String, String);

fn is_requirement_artifact(loc: Option<&_Loc>) -> bool {
    loc.is_some_and(|(k, p)| k == "artifact" && !p.starts_with(_TRIPLET_ARTIFACT_PREFIX))
}

/// Could this source ground a declared-requirement — a payload artifacts file that is not the triplet. `loc` =
/// `[kind, key]` | null.
pub fn _is_requirement_artifact(loc: &J) -> HR<bool> {
    let l = loc.as_list().filter(|l| l.len() >= 2).map(|l| (l[0].display(), l[1].display()));
    Ok(is_requirement_artifact(l.as_ref()))
}

#[derive(Clone, Debug, PartialEq)]
pub struct _RawEvent {
    /// 1-based line of the fence.
    pub line: usize,
    pub chapter: String,
    /// The parsed event dict (always a dict, possibly partial when `errors` is non-empty).
    pub event: J,
    pub errors: Vec<(String, usize, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct _Excerpt {
    pub line: usize,
    pub chapter: String,
    pub stem: String,
    pub section: String,
    pub body: Vec<String>,
}

/// hint-event fences and excerpts of the authored area. Returns (events, excerpts, 1-based lines of malformed heads).
pub fn _scan(doc: &_Doc) -> HR<(Vec<_RawEvent>, Vec<_Excerpt>, Vec<usize>)> {
    let l = &doc.lines;
    let n = l.len();
    let mut events = Vec::new();
    let mut exs = Vec::new();
    let mut malformed = Vec::new();
    let mut i = 0usize;
    while i < n {
        if doc.masked[i] {
            i += 1;
            continue;
        }
        let line = &l[i];
        if rx_fence().is_match(line) {
            let is_event = rx_event_fence().is_match(line);
            let mut j = i + 1;
            while j < n && !rx_fence_close().is_match(&l[j]) {
                j += 1;
            }
            if is_event {
                let body = &l[i + 1..j.min(n)];
                let (ev, errs0) = _parse_event_body(body)?;
                let mut errs: Vec<(String, usize, String)> = errs0.into_iter().map(|(c, off, m)| (c, i + 2 + off as usize, m)).collect();
                if j >= n {
                    errs.push(("HINT_EVENT_INVALID".into(), i + 1, "hint-event 펜스가 닫히지 않았다".into()));
                }
                if errs.is_empty() {
                    errs = _validate_event(&ev)?.into_iter().map(|(c, m)| (c, i + 1, m)).collect();
                }
                events.push(_RawEvent { line: i + 1, chapter: doc.chapter_of[i].clone(), event: ev, errors: errs });
            }
            i = j + 1;
            continue;
        }
        if rx_excerpt_any().is_match(line) {
            let hd = rx_excerpt_head().match_at_start(line);
            let mut j = i + 1;
            let mut body = Vec::new();
            while j < n && !doc.masked[j] && l[j].starts_with('>') && !rx_excerpt_any().is_match(&l[j]) {
                let rest = &l[j][1..];
                body.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
                j += 1;
            }
            match hd {
                Some(h) => exs.push(_Excerpt {
                    line: i + 1,
                    chapter: doc.chapter_of[i].clone(),
                    stem: named(&h, "stem").unwrap_or("").to_string(),
                    section: named(&h, "section").unwrap_or("").to_string(),
                    body,
                }),
                None => malformed.push(i + 1),
            }
            i = j;
            continue;
        }
        i += 1;
    }
    Ok((events, exs, malformed))
}

/// ` ```hint-event ` blocks → events (strict; first error = HintError). Each event gets `_line` and a list `출처`.
pub fn parse_hint_events(md: &str) -> HR<Vec<J>> {
    let doc = _Doc::parse("<md>", md);
    let (events, _, _) = _scan(&doc)?;
    let mut out = Vec::new();
    for raw in events {
        if let Some((code, line, msg)) = raw.errors.first() {
            return core::fail(code, format!("hint-event(줄 {line}): {msg}"), None);
        }
        let mut ev = raw.event.clone();
        let src = _sources_of(&ev)?;
        ev.set("출처", J::from(src));
        ev.set("_line", raw.line as i64);
        out.push(ev);
    }
    Ok(out)
}

/// Excerpts: `> [원문] <stem> §<절>` heads with the following `> ` lines.
pub fn excerpts(md: &str) -> HR<Vec<J>> {
    let doc = _Doc::parse("<md>", md);
    let (_, exs, _) = _scan(&doc)?;
    Ok(exs
        .iter()
        .map(|e| {
            jd! {"stem" => e.stem.as_str(), "section" => e.section.as_str(), "body" => e.body.join("\n"), "line" => e.line as i64,
                 "line_count" => e.body.len() as i64, "chapter" => e.chapter.as_str()}
        })
        .collect())
}

/// `<!-- FACT:<id> -->` … → {id: body} (first wins; the open-ended bench region is stripped of surrounding newlines).
pub fn fact_blocks(md: &str) -> HR<J> {
    let doc = _Doc::parse("<md>", md);
    let mut out = Map::new();
    for f in &doc.facts {
        if !out.contains_key(&f.fid) {
            out.insert(f.fid.as_str(), if f.open_ended { f.body.trim_matches('\n').to_string() } else { f.body.clone() });
        }
    }
    Ok(J::Dict(out))
}

// ── 파생 블록 (00 §0.3 · §0.5) ────────────────────────────────────────────────────────────────
pub fn _valid_events(md: &str) -> HR<Vec<J>> {
    let doc = _Doc::parse("<md>", md);
    let (events, _, _) = _scan(&doc)?;
    Ok(events.into_iter().filter(|r| r.errors.is_empty()).map(|r| r.event).collect())
}

pub fn _derive_from_texts(narrative: &str, artifacts_md: &str) -> HR<J> {
    let evs = _valid_events(narrative)?;
    let walls: Vec<&J> = evs.iter().filter(|e| e["kind"].as_str() == Some("wall")).collect();
    let miss: Vec<&J> = evs.iter().filter(|e| e["kind"].as_str() == Some("misdiagnosis")).collect();
    let mut wall: Vec<String> = if !walls.is_empty() {
        let mut rows = Vec::new();
        for (i, e) in walls.iter().enumerate() {
            rows.push(vec![(i + 1).to_string(), cell(&e["id"])?, cell(&e["증상"])?, cell(&e["해소"])?, cell(&e["전이등급"])?, cell(&e["검증"])?]);
        }
        tbl(&["순서", "id", "증상", "해소", "전이등급", "검증"], rows)?
    } else {
        vec!["_02-narrative.md §2.2 에 `kind: wall` 블록이 아직 없다 — 저작 뒤 `refresh` 가 채운다(린터: wall ≥ 1)._".into()]
    };
    if !miss.is_empty() {
        wall.extend(["".into(), "**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)".into(), "".into()]);
        let mut rows = Vec::new();
        for e in &miss {
            rows.push(vec![cell(&e["id"])?, cell(&e["증상"])?, _cell(&e["현재지위"], "—")?, cell(&e["검증"])?]);
        }
        wall.extend(tbl(&["id", "증상", "현재지위", "검증"], rows)?);
    }
    let vs: Vec<J> = _valid_events(artifacts_md)?.into_iter().filter(|e| e["kind"].as_str() == Some("value-status")).collect();
    let knob: Vec<String> = if !vs.is_empty() {
        let mut parts = Vec::new();
        for s in VALUE_STATUSES().as_dict().map(|m| m.keys().map(str::to_string).collect::<Vec<_>>()).unwrap_or_default() {
            let n = vs.iter().filter(|e| e["지위"].as_str() == Some(s.as_str())).count();
            if n > 0 {
                parts.push(format!("`{s}` {n}"));
            }
        }
        let mut k = vec![format!("노브 {}개 — {}.", vs.len(), parts.join(" · ")), String::new()];
        let mut rows = Vec::new();
        for e in &vs {
            rows.push(vec![format!("`{}`", cell(&e["노브"])?), cell(&e["값"])?, cell(&e["지위"])?, cell(&e["근거"])?, cell(&e["id"])?]);
        }
        k.extend(tbl(&["노브", "값", "지위", "근거", "id"], rows)?);
        k
    } else {
        vec!["_01-artifacts.md §1.3 에 `kind: value-status` 블록이 아직 없다 — 저작 뒤 `refresh` 가 채운다._".into()]
    };
    Ok(jd! {"wall_map" => wall.join("\n"), "knob_status" => knob.join("\n")})
}

/// Derived block bodies (pure): wall_map ← 02 · knob_status ← 01.
pub fn derive_blocks(payload_dir: &Path) -> HR<J> {
    let read = |name: &str| -> HR<String> {
        let p = payload_dir.join(name);
        if p.is_file() {
            read_utf8(&p)
        } else {
            Ok(String::new())
        }
    };
    _derive_from_texts(&read("02-narrative.md")?, &read("01-artifacts.md")?)
}

pub fn _replace_fact_bodies(text: &str, bodies: &J) -> HR<String> {
    let doc = _Doc::parse("<md>", text);
    let mut lines = doc.lines.clone();
    let mut facts: Vec<&_Fact> = doc.facts.iter().collect();
    facts.sort_by(|a, b| b.start.cmp(&a.start));
    for f in facts {
        if let Some(b) = bodies.get(&f.fid) {
            let b = b.display();
            let new: Vec<String> = if b.is_empty() { Vec::new() } else { b.split('\n').map(str::to_string).collect() };
            splice(&mut lines, f.start + 1, f.end, new);
        }
    }
    Ok(lines.join("\n"))
}

/// Re-fill 00's derived blocks from the current hint-events (writes). Returns the changed block ids.
pub fn refresh(payload_dir: &Path) -> HR<Vec<String>> {
    let p = payload_dir.join("00-hint.md");
    if !p.is_file() {
        return core::fail("HINT_DOCUMENT_ABSENT", format!("00-hint.md 부재: {}", core::posix(&p)), None);
    }
    let text = read_utf8(&p)?;
    let have = fact_blocks(&text)?;
    let want = derive_blocks(payload_dir)?;
    let absent: Vec<String> = DERIVED_FACTS.iter().filter(|k| have.get(k).is_none()).map(|k| k.to_string()).collect();
    if !absent.is_empty() {
        return core::fail(
            "HINT_FACT_BLOCK_ABSENT",
            format!("00-hint.md 에 파생 블록 표지가 없다: {}", list_repr(&absent)),
            Some("사실 블록 표지를 지우지 않는다 — publish 로 스캐폴드를 다시 만든다."),
        );
    }
    let changed: Vec<String> = DERIVED_FACTS.iter().filter(|k| have[**k] != want[**k]).map(|k| k.to_string()).collect();
    if !changed.is_empty() {
        let mut b = Map::new();
        for k in &changed {
            b.insert(k.as_str(), want[k.as_str()].clone());
        }
        write_text(&p, &_replace_fact_bodies(&text, &J::Dict(b))?)?;
    }
    Ok(changed)
}

// ── 렌더 ─────────────────────────────────────────────────────────────────────────────────────
/// Normalize like the snapshot round trip (`json.loads(core.dumps(facts))` — keys sorted).
pub fn _normalize_facts(facts: &J) -> HR<J> {
    if !facts.is_dict() {
        return core::fail("HINT_FACTS_SHAPE", format!("facts 는 dict 여야 한다: {}", facts.type_name()), None);
    }
    crate::value::loads(&core::dumps(facts)).map_err(|e| HintError::new("HINT_FACTS_SHAPE", format!("facts 를 JSON 으로 직렬화할 수 없다: {e}"), None))
}

pub fn _render_doc(ctx: &_Ctx, name: &str) -> HR<String> {
    let doc = tmpl(ctx.templates, name)?;
    let mut lines: Vec<String> = Vec::with_capacity(doc.lines.len());
    for (i, ln) in doc.lines.iter().enumerate() {
        lines.push(if doc.comment[i] { _cond_text(ln, ctx.facts)? } else { ln.clone() });
    }
    let mut edits: Vec<(usize, usize, Vec<String>)> = Vec::new();
    for f in &doc.facts {
        let body = match fact_renderer(&f.fid) {
            Some(r) => r(ctx)?,
            None => String::new(),
        };
        let mut new: Vec<String> = if body.is_empty() { Vec::new() } else { body.split('\n').map(str::to_string).collect() };
        if f.open_ended {
            new.insert(0, String::new());
            new.push(String::new());
        }
        edits.push((f.start + 1, f.end, new));
    }
    for pr in &doc.prompts {
        let cond = directives(&pr.fields)?.condition;
        let (ok, actual) = cond_holds(cond.as_ref(), ctx.facts)?;
        if ok {
            continue;
        }
        let mut end = pr.end;
        for k in pr.end + 1..lines.len() {
            if lines[k].starts_with(AGENT_MARK) {
                end = k;
                break;
            }
            if !pstrip(&lines[k]).is_empty() {
                break;
            }
        }
        edits.push((pr.start, end + 1, vec![na_line(cond.as_ref().unwrap(), &actual)]));
    }
    edits.sort_by(|a, b| b.0.cmp(&a.0));
    for (s, e, new) in edits {
        splice(&mut lines, s, e, new);
    }
    Ok(lines.join("\n"))
}

pub fn _render_readme(ctx: &_Ctx) -> HR<String> {
    let p = _templates_dir(ctx.repo)?.join(README_TEMPLATE);
    if !p.is_file() {
        return core::fail("HINT_TEMPLATE_ABSENT", format!("README 템플릿 부재: {}/templates/{README_TEMPLATE}", core::REL_SKILL), None);
    }
    let f = ctx.facts;
    let lite = is_lite_cell(f)?;
    let banner: Vec<String> =
        banner_lines().iter().enumerate().map(|(i, ln)| format!("{}{ln}", if i == 0 { "> ⚠ " } else { "> " })).collect();
    let refute = _dict(f, "measurement")?["verdict"].as_str() == Some(REFUTE_MARKER) && !lite;
    let mut tokens: Vec<(&str, String)> = vec![
        ("TAG", f["tag"].display()),
        ("FORMAT", FORMAT.to_string()),
        ("GRAMMAR", cell(&_dict(f, "naming")?["grammar"])?),
        ("GENERATED_UTC", f["generated_utc"].display()),
        ("BANNER", banner.join("\n")),
        ("SUB_RECIPE_ROW", if _topology(f)?.as_deref() == Some("multi") { " · (멀티) 서브 레시피 해설".into() } else { String::new() }),
        ("EVENT_KINDS", _event_kinds_md(ctx.templates)?),
        ("LEGEND", _legend_md()?),
        (
            "VERDICT",
            format!(
                "> **판정** {}{}",
                _verdict_text(f)?,
                if refute { format!(" · **{REFUTE_MARKER}** = 기각된 성능 판정(수치는 관측)") } else { String::new() }
            ),
        ),
        ("NAME_GUIDE", name_guide_md(f)?),
        ("CONTEXT_MAP", _context_map_summary(f)?),
    ];
    tokens.push((
        "BENCH_ROW",
        if lite {
            _README_DROP.into()
        } else {
            format!("| 4 | `{BENCH_DOC}` | 측정 — 결정론 표 · 부하 곡선 · 측정 구성 · 데이터셋 출처 · 측정 명령 원문 · like-with-like |")
        },
    ));
    tokens.push((
        "BENCH_LAYER_LINE",
        if lite {
            _README_DROP.into()
        } else {
            format!(
                "  `{BENCH_DOC}` §3.2 의 부하 곡선은 `<!-- BENCH_SECTION -->` 다음 줄부터 다음 챕터 헤딩 직전까지가 기계 렌더다 — \
                 원천 리포트를 가진 쪽은 `render_bench_section.sh --verify --section {BENCH_DOC} --report <리포트>` 로 diff 0 을 \
                 다시 확인할 수 있다."
            )
        },
    ));
    tokens.push((
        "BENCH_COMPARE_LINE",
        if lite {
            format!(
                "- 이 셀은 **`{LITE_LABEL}`** 셀이다 — `{BENCH_DOC}` 가 없다(성능 판정 · 부하 곡선 없음). lite 수치는 `00-hint.md` \
                 머리의 lite 측정 블록이며 이 HW 에서 기동 · 추론이 된다는 최소 증명일 뿐이다 — full 셀의 수치와 나란히 순위 매기지 않는다."
            )
        } else {
            format!(
                "- 성능 수치를 비교할 때는 `{BENCH_DOC}` 의 측정 구성 · 데이터셋 출처 · like-with-like 부터 본다 — 조건(입력 데이터셋 \
                 포함)이 다르면 같은 모델 · 같은 HW 라도 수치가 몇 배씩 갈린다."
            )
        },
    ));
    let src = read_utf8(&p)?;
    let mut text = String::with_capacity(src.len());
    let mut last = 0;
    for c in rx_readme_token().finditer(&src) {
        let m = c.get(0).unwrap();
        let key = group(&c, 1).unwrap_or("");
        let Some((_, v)) = tokens.iter().find(|(k, _)| *k == key) else {
            return core::fail("HINT_TEMPLATE_SHAPE", format!("README 템플릿의 모르는 자리표시: {}", m.as_str()), None);
        };
        text.push_str(&src[last..m.start()]);
        text.push_str(v);
        last = m.end();
    }
    text.push_str(&src[last..]);
    Ok(text.split('\n').filter(|ln| *ln != _README_DROP).collect::<Vec<_>>().join("\n"))
}

/// hintlib/template.py:2833 NAME_GENERATIONS (python literal as JSON)
pub fn NAME_GENERATIONS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("[[\"v7\", \"`q·len·kv` 결정론 + 근거 붙은 꼬리 + 중복 시 `-t<YYMMDDHHMM>` · 페이로드 커밋의 부모 = 안내 커밋\", \"2026-09-29 ~\"], [\"v6\", \"레시피 여섯 축 고정 `q…-len…-kv…-ple…-spec<n|off>-<graph|eager>`(v7 파서는 3축 + 꼬리 셋으로 읽는다)\", \"2026-09-22 ~ 09-28\"], [\"옛 5세그먼트(노드 축)\", \"arch `<hw>-<main|sub|cluster>-<target>`\", \"2026-09-06 ~ 09-21\"], [\"옛 5세그먼트\", \"arch `<hw>-<target>` · 노드 축 없음\", \"2026-09-04 ~ 09-06\"], [\"옛 4세그먼트\", \"`hint/<vllm>/<model>/<arch>` · 레시피 축 없음\", \"2026-09-04 이전\"]]").expect("constant"))
}
/// hintlib/template.py:2840 _SEGMENT_MEANINGS (python literal as JSON)
pub fn _SEGMENT_MEANINGS() -> &'static J {
    static CELL: std::sync::OnceLock<J> = std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::value::loads("{\"vllm\": \"빌드 입력 — 릴리스 태그면 그 릴리스, 커밋 핀이면 `<직전 릴리스>-g<커밋 12자>`(엔진 자기보고는 00 §0.4 의 다른 행)\", \"model\": \"체크포인트 이름(Hugging Face 등록명 소문자 · 양자화 접미사 그대로)\", \"q\": \"양자화 — 체크포인트가 **선언한** 방식(혼합 구성은 00 §0.4 양자화 구성 표)\", \"len\": \"최대 컨텍스트 길이(max-model-len)\", \"kv\": \"KV 캐시 dtype(서빙 설정 선언)\"}").expect("constant"))
}

fn axis2(v: &J) -> HR<(J, J)> {
    let a = _axis(v)?;
    Ok((a[0].clone(), a[1].clone()))
}

/// Payload README "how to read this tag name" (deterministic from PAYLOAD.naming).
pub fn name_guide_md(f: &J) -> HR<String> {
    let nm = _dict(f, "naming")?;
    let seg = _dict(&nm, "segments")?;
    let axes = _dict(&nm, "axes")?;
    let sm = _SEGMENT_MEANINGS();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for k in ["vllm", "model"] {
        let (v, src) = axis2(&seg[k])?;
        rows.push(vec![format!("`<{k}>`"), format!("`{}`", cell(&v)?), sm[k].display(), cell(&src)?]);
    }
    let (av, asrc) = axis2(&seg["arch"])?;
    let mut parts: Map = Map::new();
    for k in ["hw", "gpus_per_node", "nodes", "role", "target", "plane"] {
        parts.insert(k, axis2(&axes[k])?.0);
    }
    let pj = J::Dict(parts);
    rows.push(vec![
        "`<arch>`".into(),
        format!("`{}`", cell(&av)?),
        format!(
            "`<hw>-<G>g<N>n-<role>-<target>[-bare]` — hw `{}` · 노드당 GPU {} · 노드 {} · 역할 `{}` · 타겟 `{}` · 평면 `{}`",
            cell(&pj["hw"])?,
            cell(&pj["gpus_per_node"])?,
            cell(&pj["nodes"])?,
            cell(&pj["role"])?,
            cell(&pj["target"])?,
            _cell(&pj["plane"], "docker(토큰 없음)")?
        ),
        cell(&asrc)?,
    ]);
    for k in ["q", "len", "kv"] {
        let (v, src) = axis2(&axes[k])?;
        rows.push(vec![format!("`{k}`"), format!("`{}`", cell(&v)?), sm[k].display(), cell(&src)?]);
    }
    let tail = &nm["tail"];
    if let J::List(tl) = tail {
        for r in tl {
            if r.is_dict() {
                rows.push(vec![
                    format!("꼬리 `{}`", cell(&r["token"])?),
                    format!("`{}`", cell(&r["token"])?),
                    cell(&r["meaning"])?,
                    format!("발행 Agent · 근거 {}", _tail_evidence_text(&r["evidence"])?),
                ]);
            }
        }
        if nm["timestamp"].truthy() {
            rows.push(vec![
                "timestamp".into(),
                format!("`{}`", cell(&nm["timestamp"])?),
                "같은 이름이 이미 있어 붙인 발행 시각(KST `YYMMDDHHMM`) — 같은 셀의 새 판".into(),
                "도구(generated_utc · 결정론)".into(),
            ]);
        }
    }
    let mut out = vec![format!("`{}`", cell(&f["tag"])?), String::new()];
    out.extend(tbl(&["자리", "값", "뜻", "출처"], rows)?);
    if !tail.is_list() {
        out.extend(["".into(), "_꼬리 미확정 — 이 README 는 publish 스캐폴드다(continue 가 꼬리를 확정하며 다시 렌더한다)._".into()]);
    }
    let target = if pj["target"].truthy() { pj["target"].display() } else { String::new() };
    out.extend([
        String::new(),
        format!(
            "- `native` = **실제 하드웨어에서 잰 것**(`sim-<타겟>` = 다른 GPU 의 메모리 예산 흉내의 반대) — Docker 여부와 **무관**하다. 이 태그: 타겟 `{}`.",
            cell(&J::from(target))?
        ),
        format!("- `-bare` = **Docker 없이**(호스트 venv) 서빙한 셀 · 토큰이 없으면 Docker 셀이다. 이 태그: 평면 `{}`.", _cell(&pj["plane"], "docker")?),
        "- 꼬리는 규칙 목록이 아니라 근거가 붙은 자유 기재다 — 비슷한 hint 는 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)로 먼저 맞춘다.".into(),
        String::new(),
        "**이름 세대**(옛 태그는 교정 · 리콜하지 않는다)".into(),
        String::new(),
    ]);
    let mut grows = Vec::new();
    for g in NAME_GENERATIONS().items() {
        let name = g[0].display();
        grows.push(vec![if name.starts_with('v') { format!("`{name}`") } else { name }, cell(&g[1])?, g[2].display()]);
    }
    out.extend(tbl(&["세대", "모양", "시기"], grows)?);
    Ok(out.join("\n"))
}

/// README slot → build-context summary (grouped by (zip folder, context folder)).
pub fn _context_map_summary(f: &J) -> HR<String> {
    let all: Vec<&J> = f["build_context_map"].items().iter().filter(|r| r.is_dict()).collect();
    let rows: Vec<&J> = all.iter().copied().filter(|r| r["slot_path"].truthy()).collect();
    if rows.is_empty() {
        return Ok("_매핑 없음 — 빌드 컨텍스트 매핑을 파생하지 못했다(01 §1.4)._".into());
    }
    let mut groups: BTreeMap<(String, String), i64> = BTreeMap::new();
    let mut outside = 0;
    for r in &rows {
        if !r["context_path"].truthy() {
            outside += 1;
            continue;
        }
        let cdir = py_parent(&r["context_path"].display());
        let root = &r["context_root"];
        let cname = if !root.truthy() {
            if cdir == "." {
                "(컨텍스트 루트)".to_string()
            } else {
                format!("{cdir}/")
            }
        } else if cdir == "." {
            format!("{}/", root.display())
        } else {
            format!("{}/{cdir}/", root.display())
        };
        let key = (format!("{}/", py_parent(&r["slot_path"].display())), cname);
        *groups.entry(key).or_insert(0) += 1;
    }
    let mut out = tbl(
        &["zip 폴더", "빌드 컨텍스트 자리", "파일 수"],
        groups.iter().map(|((a, b), n)| vec![format!("`{a}`"), format!("`{b}`"), n.to_string()]).collect(),
    )?;
    let mk: Vec<&J> = all.iter().copied().filter(|r| r["create"].as_str() == Some("empty-dir")).collect();
    let both: Vec<&J> = rows.iter().chain(mk.iter()).copied().collect();
    let roots: BTreeSet<String> = both.iter().filter(|r| r["context_root"].truthy()).map(|r| r["context_root"].display()).collect();
    let example = both.iter().any(|r| r["context_scope"].as_str() == Some("reconstruction_example"));
    if example && roots.is_empty() {
        out.extend(["".into(), "_매핑은 실린 레시피의 재구성 예시이며 실제 원본·후속층의 빌드 경로는 별도 실행 기록에서 확인한다._".into()]);
    }
    if !roots.is_empty() {
        let mut rs = Vec::new();
        for x in &roots {
            rs.push(format!("`{}/`", cell(&J::from(x.as_str()))?));
        }
        let joined = rs.join(" · ");
        out.push(String::new());
        out.push(if example {
            format!("_빌드 레시피 · 빌드 패치의 재구성 예시 통로 {joined}(실제 원본·후속층의 빌드 경로 증명 아님 · 별도 실행 기록 확인)._")
        } else {
            format!(
                "_⚠ 빌드 레시피 · 빌드 패치는 다른 컨텍스트 {joined}(이 이미지의 레시피 통로 · 01 §1.4 2단계 docker build 와 같은 자리 — compose build 는 동등 경로가 아니다)._"
            )
        });
    }
    if !mk.is_empty() {
        let mut items = Vec::new();
        for r in &mk {
            let pre = if r["context_root"].truthy() { cell(&J::from(format!("{}/", r["context_root"].display())))? } else { String::new() };
            items.push(format!("`{pre}{}`", cell(&r["context_path"])?));
        }
        out.extend([
            String::new(),
            format!("_빈 디렉터리로 만들 컨텍스트 자리(실린 Dockerfile 이 COPY · 실린 파일 없음): {} — 01 §1.4._", items.join(" · ")),
        ]);
    }
    if outside > 0 {
        out.extend(["".into(), format!("_빌드 컨텍스트 입력이 아닌 파일 {outside}개(기록 · pip freeze 등) — 01 §1.4 표의 `—` 행._")]);
    }
    Ok(out.join("\n"))
}

/// facts snapshot location = the draft's `inputs/`.
pub fn default_snapshot_path(payload_dir: &Path) -> HR<PathBuf> {
    Ok(payload_dir.parent().unwrap_or(Path::new("")).join("inputs").join(FACTS_SNAPSHOT))
}

pub fn load_snapshot(path: &Path) -> HR<J> {
    core::read_json_code(path, "HINT_FACT_SNAPSHOT_UNREADABLE")
}

/// Render the 4 templates + README into the payload directory. Returns `prompts(payload_dir)`.
pub fn render_scaffold(repo: &Path, facts: &J, payload_dir: &Path, snapshot_path: Option<&Path>) -> HR<Vec<J>> {
    let facts = _normalize_facts(facts)?;
    if !facts["tag"].as_str().is_some_and(|t| t.starts_with(core::HINT_TAG_PREFIX)) {
        return core::fail(
            "HINT_FACTS_SHAPE",
            format!("facts.tag 는 `{}` 로 시작하는 파생 이름이어야 한다: {}", core::HINT_TAG_PREFIX, facts["tag"].repr()),
            None,
        );
    }
    core::require_utc(facts["generated_utc"].as_str(), "facts.generated_utc")?;
    if _topology(&facts)?.is_none() {
        return core::fail("HINT_FACTS_SHAPE", "facts.identity.topology 가 single|multi 가 아니다 — 토폴로지는 manifest 에서 관측한다.", None);
    }
    let templates = _load_templates(repo)?;
    let ctx = _Ctx { repo, facts: &facts, templates: &templates };
    let docs = payload_docs(&facts)?;
    let mut targets: Vec<PathBuf> = payload_docs_all().iter().map(|n| payload_dir.join(n)).collect();
    targets.push(payload_dir.join(README_NAME));
    let exist: Vec<String> = targets
        .iter()
        .filter(|t| t.is_file() && std::fs::metadata(t).map(|m| m.len() > 0).unwrap_or(false))
        .map(|t| py_name(&core::posix(t)))
        .collect();
    if !exist.is_empty() {
        return core::fail(
            "HINT_SCAFFOLD_EXISTS",
            format!("기존 저작물을 덮지 않는다: {} ({})", core::posix(payload_dir), exist.join(", ")),
            Some("이어서 저작하려면 `hint.sh continue` · 새로 시작하려면 드래프트를 지우고 publish 를 다시 한다."),
        );
    }
    let mut rendered: Vec<(String, String)> = Vec::new();
    for name in TEMPLATE_FILES {
        if docs.contains(&format!("{name}.md")) {
            rendered.push((name.to_string(), _render_doc(&ctx, name)?));
        }
    }
    let readme = _render_readme(&ctx)?;
    std::fs::create_dir_all(payload_dir).map_err(|e| HintError::py(io_type(&e), core::io_err_text(&e, payload_dir)))?;
    for (name, text) in &rendered {
        write_text(&payload_dir.join(format!("{name}.md")), text)?;
    }
    write_text(&payload_dir.join(README_NAME), &readme)?;
    write_agent_requests(payload_dir, &facts["agent_requests"])?;
    let snap = match snapshot_path {
        Some(p) => p.to_path_buf(),
        None => default_snapshot_path(payload_dir)?,
    };
    core::write_json(&snap, &facts)?;
    refresh(payload_dir)?;
    prompts(payload_dir)
}

/// PROMPTs left in the payload.
pub fn prompts(payload_dir: &Path) -> HR<Vec<J>> {
    let mut out = Vec::new();
    for name in payload_docs_all() {
        let p = payload_dir.join(&name);
        if !p.is_file() {
            continue;
        }
        let doc = _Doc::parse(&name, &read_utf8(&p)?);
        for pr in &doc.prompts {
            let d = directives(&pr.fields)?;
            let title = doc.chapters.iter().rev().find(|(_, n, _)| *n == pr.chapter).map(|(_, _, t)| t.clone()).unwrap_or_default();
            let mut blocks = d.blocks.clone();
            blocks.sort();
            let mut bm = Map::new();
            for (k, v) in blocks {
                bm.insert(k, v);
            }
            out.push(jd! {
                "file" => name.as_str(),
                "section" => pr.chapter.as_str(),
                "title" => title,
                "line" => (pr.start + 1) as i64,
                "question" => _question(&pr.fields)?,
                "read" => _norm(pr.fields.get_str("읽을 것").unwrap_or(""))?,
                "instruction" => pr.fields.get_str("기재").unwrap_or(""),
                "required" => _norm(pr.fields.get_str("필수 필드").unwrap_or(""))?,
                "forbidden" => _norm(pr.fields.get_str("금지").unwrap_or(""))?,
                "blocks" => J::Dict(bm),
                "excerpts_required" => d.excerpts,
                "optional" => d.optional,
            });
        }
    }
    Ok(out)
}

/// `<!-- PROMPT … -->` → `> 이 절이 답하는 질문: <질문>` (idempotent; all checked before any write).
pub fn seal_prompts(payload_dir: &Path, docs: &[String]) -> HR<()> {
    let mut sealed: Vec<(PathBuf, String)> = Vec::new();
    for name in docs {
        let p = payload_dir.join(name);
        if !p.is_file() {
            return core::fail("HINT_DOCUMENT_ABSENT", format!("봉인할 문서 부재: {name}"), None);
        }
        let text = read_utf8(&p)?;
        if let Some(pos) = text.find(AGENT_MARK) {
            let line = text[..pos].matches('\n').count() + 1;
            return core::fail(
                "HINT_AGENT_PLACEHOLDER_RESIDUE",
                format!("{name}:{line}: 미저작 자리표시 `{AGENT_MARK}` 가 남아 있다"),
                Some("그 절을 PROMPT 지시대로 저작한 뒤 lint 를 통과시키고 봉인한다."),
            );
        }
        let doc = _Doc::parse(name, &text);
        if doc.prompts.is_empty() && doc.selfchecks.is_empty() {
            continue;
        }
        let mut lines = doc.lines.clone();
        let mut edits: Vec<(usize, usize, Vec<String>)> = Vec::new();
        for pr in &doc.prompts {
            let q = _question(&pr.fields)?;
            if q.is_empty() {
                return core::fail("HINT_PROMPT_QUESTION_ABSENT", format!("{name}:{}: `질문:` 없는 PROMPT", pr.start + 1), None);
            }
            let nxt = lines.get(pr.end + 1).cloned().unwrap_or_default();
            let mut new = vec![format!("{SEALED_PREFIX}{q}")];
            if !pstrip(&nxt).is_empty() {
                new.push(String::new());
            }
            edits.push((pr.start, pr.end + 1, new));
        }
        for &(a, b) in &doc.selfchecks {
            let mut end = b + 1;
            if end < lines.len() && pstrip(&lines[end]).is_empty() && (a == 0 || pstrip(&lines[a - 1]).is_empty()) {
                end += 1;
            }
            edits.push((a, end, Vec::new()));
        }
        edits.sort_by(|x, y| y.0.cmp(&x.0));
        for (a, b, new) in edits {
            splice(&mut lines, a, b, new);
        }
        sealed.push((p, lines.join("\n")));
    }
    for (p, text) in sealed {
        write_text(&p, &text)?;
    }
    Ok(())
}

/// A chapter's `### <subtitle>` and its first authored paragraph: [{title, key, text}].
pub fn subsections(payload_dir: &Path, doc: &str, chapter: &str) -> HR<Vec<J>> {
    let p = payload_dir.join(format!("{doc}.md"));
    if !p.is_file() {
        return Ok(Vec::new());
    }
    let d = _Doc::parse(&format!("{doc}.md"), &read_utf8(&p)?);
    struct Cur {
        title: String,
        key: String,
        para: Vec<String>,
        done: bool,
    }
    let mut out: Vec<Cur> = Vec::new();
    for i in d.chapter_lines(chapter) {
        let line = &d.lines[i];
        let m = if d.masked[i] || d.fence[i] { None } else { _H3().match_at_start(line) };
        if let Some(m) = m {
            let title = pstrip(group(&m, 1).unwrap_or("")).to_string();
            let t2 = pstrip(strip_set(&title, "`")).to_string();
            let first = _KEY_SPLIT().split(&t2).into_iter().next().unwrap_or_default();
            let key = pstrip(strip_set(&first, "`")).to_string();
            out.push(Cur { title, key, para: Vec::new(), done: false });
            continue;
        }
        let Some(cur) = out.last_mut() else { continue };
        if cur.done {
            continue;
        }
        let s = pstrip(line);
        if d.masked[i] || d.fence[i] || s.is_empty() || [">", "|", "#", "```", "<!--"].iter().any(|x| s.starts_with(x)) {
            if !cur.para.is_empty() {
                cur.done = true;
            }
            continue;
        }
        cur.para.push(s.to_string());
    }
    Ok(out
        .into_iter()
        .map(|c| {
            let t = c.para.join(" ");
            jd! {"title" => c.title, "key" => c.key, "text" => if t.is_empty() { J::Null } else { J::Str(t) }}
        })
        .collect())
}

/// 00-hint §0.1 first paragraph (≤ 3 lines) — the annotation brief.
pub fn brief(payload_dir: &Path) -> HR<String> {
    let p = payload_dir.join("00-hint.md");
    if !p.is_file() {
        return core::fail("HINT_BRIEF_ABSENT", "00-hint.md 부재", None);
    }
    let doc = _Doc::parse("00-hint.md", &read_utf8(&p)?);
    let idx = doc.chapter_lines("0.1");
    if idx.is_empty() {
        return core::fail("HINT_BRIEF_ABSENT", "00-hint.md §0.1 부재", None);
    }
    let mut para: Vec<String> = Vec::new();
    for &i in &idx[1..] {
        let line = &doc.lines[i];
        if doc.masked[i] && para.is_empty() && !plstrip(line).starts_with("<!-- FACT") {
            continue;
        }
        if para.is_empty() && (pstrip(line).is_empty() || line.starts_with(SEALED_PREFIX)) {
            continue;
        }
        if para.is_empty() {
            let s = plstrip(line);
            if doc.masked[i] || [">", "|", "#", "```", "<!--"].iter().any(|x| s.starts_with(x)) {
                return core::fail(
                    "HINT_BRIEF_SHAPE",
                    format!("00-hint.md:{}: §0.1 첫 문단이 인용·표·헤딩·주석·코드로 시작한다", i + 1),
                    Some("brief 는 평문 한 문단이다(추출기가 첫 문단을 그대로 annotation 에 싣는다)."),
                );
            }
        }
        if pstrip(line).is_empty() || doc.masked[i] {
            break;
        }
        para.push(prstrip(line).to_string());
    }
    if para.is_empty() {
        return core::fail("HINT_BRIEF_ABSENT", "00-hint.md §0.1 에 문단이 없다", None);
    }
    if para.iter().any(|l| l.contains(AGENT_MARK)) {
        return core::fail("HINT_BRIEF_ABSENT", "00-hint.md §0.1 이 아직 저작되지 않았다(`<<AGENT:` 잔존)", None);
    }
    if para.iter().any(|l| l.contains("<!--") || l.contains("-->")) {
        return core::fail(
            "HINT_BRIEF_SHAPE",
            "00-hint.md §0.1 첫 문단에 HTML 주석 경계(`<!--` · `-->`)가 있다",
            Some("brief 문단에서 주석 표기를 뺀다(annotation·카탈로그가 그대로 싣는다)."),
        );
    }
    if para.len() > 3 {
        return core::fail("HINT_BRIEF_TOO_LONG", format!("00-hint.md §0.1 첫 문단이 {}줄이다(최대 3줄)", para.len()), None);
    }
    Ok(para.join("\n"))
}

// ── 린터 ─────────────────────────────────────────────────────────────────────────────────────
/// `substitute(text) -> text` hook.
pub type SubstituteHook<'a> = &'a dyn Fn(&str) -> HR<String>;
/// `source_text(repo-relative path) -> text | None` hook.
pub type SourceTextHook<'a> = &'a dyn Fn(&str) -> HR<Option<String>>;
/// `resolve_stem(lineage, stem) -> path | None` hook.
pub type ResolveStemHook<'a> = &'a dyn Fn(&J, &str) -> HR<Option<String>>;
/// `snapshot_origin(rev, path) -> bytes | None` hook.
pub type SnapshotOriginHook<'a> = &'a dyn Fn(&str, &str) -> HR<Option<Vec<u8>>>;

/// The callable injection hooks of `lint` / `substituted_source` (None = the Python default).
#[derive(Clone, Copy, Default)]
pub struct TemplateHooks<'a> {
    pub substitute: Option<SubstituteHook<'a>>,
    pub source_text: Option<SourceTextHook<'a>>,
    pub resolve_stem: Option<ResolveStemHook<'a>>,
    pub snapshot_origin: Option<SnapshotOriginHook<'a>>,
}

/// `_resolvers` result: (substitute | None, resolve_stem, source_text).
pub struct _Resolvers<'a> {
    pub substitute: Option<Box<dyn Fn(&str) -> HR<String> + 'a>>,
    pub resolve_stem: Box<dyn Fn(&J, &str) -> HR<Option<String>> + 'a>,
    pub source_text: Box<dyn Fn(&str) -> HR<Option<String>> + 'a>,
}

/// `pii._table_rows` on a JSON table (rows = 2-lists of str, non-empty literal).
// OWNER-MIRROR: hintlib/pii.py::_table_rows (the shape check of a table that arrives as a J value)
fn subst_rows(table: &J) -> HR<Vec<(String, String)>> {
    let rows: Vec<J> = match table {
        J::List(l) => l.clone(),
        J::Dict(m) => m.keys().map(J::from).collect(),
        J::Str(s) => s.chars().map(|c| J::from(c.to_string())).collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for r in rows {
        match r.as_list() {
            Some(l) if l.len() == 2 && l[0].is_str() && l[1].is_str() && !l[0].as_str().unwrap().is_empty() => {
                out.push((l[0].as_str().unwrap().to_string(), l[1].as_str().unwrap().to_string()));
            }
            _ => {
                return core::fail(
                    "HINT_PII_SUBST_TABLE_SHAPE",
                    "치환표 행은 (비어 있지 않은 리터럴, 표지) 문자열 쌍이다",
                    Some("substitution_table() 의 반환값을 그대로 넘긴다."),
                )
            }
        }
    }
    Ok(out)
}

fn resolvers<'a>(repo: &'a Path, subst_table: &'a J, hooks: &TemplateHooks<'a>) -> HR<_Resolvers<'a>> {
    let substitute: Option<Box<dyn Fn(&str) -> HR<String> + 'a>> = match hooks.substitute {
        Some(f) => Some(Box::new(f)),
        None if !subst_table.is_null() => {
            if !matches!(subst_table, J::List(_) | J::Dict(_) | J::Str(_)) {
                return Err(HintError::py("TypeError", format!("'{}' object is not iterable", subst_table.type_name())));
            }
            Some(Box::new(move |t: &str| {
                let rows = subst_rows(subst_table)?;
                crate::pii::substitute(t, &rows, true)
            }))
        }
        None => None,
    };
    let resolve_stem: Box<dyn Fn(&J, &str) -> HR<Option<String>> + 'a> = match hooks.resolve_stem {
        Some(f) => Box::new(f),
        None => Box::new(|l: &J, s: &str| crate::lineage::resolve_stem(l, s)),
    };
    let source_text: Box<dyn Fn(&str) -> HR<Option<String>> + 'a> = match hooks.source_text {
        Some(f) => Box::new(f),
        None => Box::new(move |rel: &str| Ok(read_source_raw(&repo.join(rel), "replace").ok())),
    };
    Ok(_Resolvers { substitute, resolve_stem, source_text })
}

fn no_callable(name: &str, v: &J) -> HR<()> {
    if v.is_null() {
        Ok(())
    } else {
        Err(HintError::py("TypeError", format!("{name}: a callable hook cannot be passed as a value (use the *_with variant)")))
    }
}

/// `_resolvers(repo, subst_table, substitute, resolve_stem, source_text)` — the hooks must be null (defaults); the typed
/// form is used internally by `lint_with` / `substituted_source_with`.
pub fn _resolvers(repo: &Path, subst_table: &J, substitute: &J, resolve_stem: &J, source_text: &J) -> HR<J> {
    no_callable("substitute", substitute)?;
    no_callable("resolve_stem", resolve_stem)?;
    no_callable("source_text", source_text)?;
    let r = resolvers(repo, subst_table, &TemplateHooks::default())?;
    Ok(jl![if r.substitute.is_some() { J::from("<substitute>") } else { J::Null }, "<resolve_stem>", "<source_text>"])
}

/// The default snapshot-origin reader: `git show <rev>:<path>` bytes · failure / no git = None.
pub fn git_origin_read(repo: &Path, rev: &str, path: &str) -> HR<Option<Vec<u8>>> {
    Ok(core::git_bytes(repo, &["show", &format!("{rev}:{path}")], false).unwrap_or(None))
}

/// `_git_origin_reader(repo)` — the reader is [`git_origin_read`]; this stub form returns a marker value.
pub fn _git_origin_reader(repo: &Path) -> HR<J> {
    Ok(J::from(format!("<git-origin-reader {}>", core::posix(repo))))
}

/// Excerpt authoring helper: source token → the **substituted** full source text the linter compares against.
pub fn substituted_source(
    repo: &Path,
    payload_dir: &Path,
    token: &str,
    lineage: &J,
    subst_table: &J,
    substitute: &J,
    source_text: &J,
    resolve_stem: &J,
    draft_dir: Option<&Path>,
    snapshot_origin: &J,
) -> HR<String> {
    no_callable("substitute", substitute)?;
    no_callable("source_text", source_text)?;
    no_callable("resolve_stem", resolve_stem)?;
    no_callable("snapshot_origin", snapshot_origin)?;
    substituted_source_with(repo, payload_dir, token, lineage, subst_table, draft_dir, &TemplateHooks::default())
}

/// `substituted_source` with injectable hooks.
pub fn substituted_source_with(
    repo: &Path,
    payload_dir: &Path,
    token: &str,
    lineage: &J,
    subst_table: &J,
    draft_dir: Option<&Path>,
    hooks: &TemplateHooks,
) -> HR<String> {
    let r = resolvers(repo, subst_table, hooks)?;
    let Some(substitute) = r.substitute.as_ref() else {
        return core::fail(
            "HINT_EXCERPT_VERIFIER_ABSENT",
            "치환표(subst_table) 또는 치환 함수가 없다 — 치환 후 원문을 만들 수 없다.",
            Some("pii.substitution_table(repo, manifest) 결과를 넘긴다."),
        );
    };
    let empty = J::dict();
    let lin = if lineage.truthy() { lineage } else { &empty };
    let mut src = _Sources::new(repo, payload_dir, lin, &*r.resolve_stem, &*r.source_text, &**substitute, draft_dir, hooks.snapshot_origin)?;
    let Some(loc) = src.locate(&_source_token(token)?)? else {
        return core::fail(
            "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE",
            format!("출처 `{token}` 가 LINEAGE · artifacts 밖이다(또는 모호)."),
            Some("LINEAGE.json 의 문서·evidence_candidates 경로 또는 artifacts 파일명을 쓴다 · 빠진 계보는 --lineage-add."),
        );
    };
    if let Some((c, m)) = src.snapshot_problem(&loc)? {
        return core::fail(&c, m, Some("스냅샷은 publish 가 `git show` 로 쓴 바이트 그대로여야 한다 — 고쳤다면 publish 를 새 draft 로 다시 한다."));
    }
    let Some(text) = src.text(&loc)? else {
        return core::fail("HINT_EXCERPT_SOURCE_UNREADABLE", format!("출처를 읽을 수 없다: {}", loc.1), None);
    };
    substitute(&text)
}

/// Source resolution (excerpts · signatures · source fields): payload artifacts → LINEAGE.
pub struct _Sources<'a> {
    repo: PathBuf,
    payload: PathBuf,
    lineage: &'a J,
    resolve_stem: &'a dyn Fn(&J, &str) -> HR<Option<String>>,
    source_text: &'a dyn Fn(&str) -> HR<Option<String>>,
    substitute: &'a dyn Fn(&str) -> HR<String>,
    origin: Option<SnapshotOriginHook<'a>>,
    art: Vec<String>,
    by_name: Vec<(String, Vec<String>)>,
    draft: PathBuf,
    snap_origin: Vec<(String, String)>,
    snap: Vec<(String, String)>,
    snap_problem_c: Vec<(String, Option<(String, String)>)>,
    text_c: Vec<(_Loc, Option<String>)>,
    normsub_c: Vec<(_Loc, Option<String>)>,
    sublines_c: Vec<(_Loc, Option<Vec<String>>)>,
}

fn walk_files(root: &Path, base: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let ft = match e.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_dir() {
            walk_files(&p, base, out);
        } else if p.is_file() {
            if let Ok(r) = p.strip_prefix(base) {
                out.push(core::posix(r));
            }
        }
    }
}

impl<'a> _Sources<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: &Path,
        payload_dir: &Path,
        lineage: &'a J,
        resolve_stem: &'a dyn Fn(&J, &str) -> HR<Option<String>>,
        source_text: &'a dyn Fn(&str) -> HR<Option<String>>,
        substitute: &'a dyn Fn(&str) -> HR<String>,
        draft_dir: Option<&Path>,
        snapshot_origin: Option<SnapshotOriginHook<'a>>,
    ) -> HR<_Sources<'a>> {
        let root = payload_dir.join("artifacts");
        let mut art = Vec::new();
        if root.is_dir() {
            walk_files(&root, payload_dir, &mut art);
            art.sort();
        }
        let mut by_name: Vec<(String, Vec<String>)> = Vec::new();
        for r in &art {
            let n = py_name(r);
            match by_name.iter_mut().find(|(k, _)| *k == n) {
                Some(e) => e.1.push(r.clone()),
                None => by_name.push((n, vec![r.clone()])),
            }
        }
        let draft = match draft_dir {
            Some(d) => d.to_path_buf(),
            None => payload_dir.parent().unwrap_or(Path::new("")).to_path_buf(),
        };
        let cands: Vec<&J> = lineage["evidence_candidates"].items().iter().filter(|x| x.is_dict()).collect();
        let listed: BTreeSet<String> = cands.iter().map(|x| x["path"].display()).collect();
        let mut snap_origin: Vec<(String, String)> = Vec::new();
        for x in &cands {
            let p = x["path"].display();
            let pv = if x["path"].truthy() { p.clone() } else { String::new() };
            if pv.starts_with(&format!("{SNAPSHOT_DIR}/")) {
                let o = if x["origin"].truthy() { x["origin"].display() } else { String::new() };
                match snap_origin.iter_mut().find(|(k, _)| *k == p) {
                    Some(e) => e.1 = o,
                    None => snap_origin.push((p, o)),
                }
            }
        }
        let sdir = draft.join(SNAPSHOT_DIR);
        let mut snap = Vec::new();
        if sdir.is_dir() {
            let mut names: Vec<PathBuf> = std::fs::read_dir(&sdir)
                .map_err(|e| HintError::py(io_type(&e), core::io_err_text(&e, &sdir)))?
                .flatten()
                .map(|e| e.path())
                .collect();
            names.sort();
            for q in names {
                let name = q.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let rel = format!("{SNAPSHOT_DIR}/{name}");
                let is_link = std::fs::symlink_metadata(&q).map(|m| m.file_type().is_symlink()).unwrap_or(false);
                if q.is_file() && !is_link && listed.contains(&rel) {
                    snap.push((name, rel));
                }
            }
        }
        Ok(_Sources {
            repo: repo.to_path_buf(),
            payload: payload_dir.to_path_buf(),
            lineage,
            resolve_stem,
            source_text,
            substitute,
            origin: snapshot_origin,
            art,
            by_name,
            draft,
            snap_origin,
            snap,
            snap_problem_c: Vec::new(),
            text_c: Vec::new(),
            normsub_c: Vec::new(),
            sublines_c: Vec::new(),
        })
    }

    pub fn locate(&self, token: &str) -> HR<Option<_Loc>> {
        let t = strip_set(pstrip(token), "`");
        if t.is_empty() {
            return Ok(None);
        }
        let pre = format!("{SNAPSHOT_DIR}/");
        if t.starts_with(&pre) || rx_snapshot_token().is_full(t) {
            let name = if t.starts_with(&pre) { &t[pre.len()..] } else { t };
            return Ok(self.snap.iter().find(|(n, _)| n == name).map(|(_, r)| ("snapshot".to_string(), r.clone())));
        }
        if self.art.iter().any(|a| a == t) {
            return Ok(Some(("artifact".into(), t.to_string())));
        }
        let at = format!("artifacts/{t}");
        if self.art.contains(&at) {
            return Ok(Some(("artifact".into(), at)));
        }
        if let Some((_, names)) = self.by_name.iter().find(|(n, _)| n == t) {
            if names.len() == 1 {
                return Ok(Some(("artifact".into(), names[0].clone())));
            }
        }
        let path = (self.resolve_stem)(self.lineage, t)?;
        Ok(path.filter(|p| !p.is_empty()).map(|p| ("lineage".to_string(), p)))
    }

    pub fn text(&mut self, loc: &_Loc) -> HR<Option<String>> {
        if let Some((_, v)) = self.text_c.iter().find(|(l, _)| l == loc) {
            return Ok(v.clone());
        }
        let (kind, key) = loc;
        let val: Option<String> = match kind.as_str() {
            "artifact" => read_source_raw(&self.payload.join(key), "strict").ok(),
            "snapshot" => {
                if self.snapshot_problem(loc)?.is_none() {
                    read_source_raw(&self.draft.join(key), "replace").ok()
                } else {
                    None
                }
            }
            _ => (self.source_text)(key)?,
        };
        let v = match val {
            Some(t) => Some(normalize_source(&t)?),
            None => None,
        };
        self.text_c.push((loc.clone(), v.clone()));
        Ok(v)
    }

    /// Snapshot origin check: None = identical bytes · (code, reason) otherwise. Non-snapshot = None.
    pub fn snapshot_problem(&mut self, loc: &_Loc) -> HR<Option<(String, String)>> {
        let (kind, key) = loc;
        if kind != "snapshot" {
            return Ok(None);
        }
        if let Some((_, v)) = self.snap_problem_c.iter().find(|(k, _)| k == key) {
            return Ok(v.clone());
        }
        let origin_txt = self.snap_origin.iter().find(|(k, _)| k == key).map(|(_, o)| o.clone());
        let ostr = origin_txt.clone().unwrap_or_default();
        let m = rx_snapshot_origin().match_at_start(&ostr);
        let mut prob: Option<(String, String)> = None;
        let data = match std::fs::read(self.draft.join(key)) {
            Ok(d) => Some(d),
            Err(e) => {
                prob = Some(("HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE".into(), format!("스냅샷 `{key}` 를 읽을 수 없다: {}", io_type(&e))));
                None
            }
        };
        if prob.is_none() && m.is_none() {
            prob = Some((
                "HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE".into(),
                format!("스냅샷 `{key}` 의 LINEAGE origin 이 `git:<40자 SHA>:<저장소 경로>` 가 아니다: {}", opt_repr(origin_txt.as_deref())),
            ));
        }
        if prob.is_none() {
            let m = m.unwrap();
            let rev = group(&m, 1).unwrap_or("");
            let path = group(&m, 2).unwrap_or("");
            let want = match self.origin {
                Some(f) => f(rev, path)?,
                None => git_origin_read(&self.repo, rev, path)?,
            };
            let r12 = head(rev, 12);
            match want {
                None => {
                    prob = Some((
                        "HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE".into(),
                        format!("스냅샷 `{key}` 의 origin `git show {r12}:{path}` 을 읽을 수 없다(커밋 부재 · git 아님)"),
                    ))
                }
                Some(w) if Some(&w) != data.as_ref() => {
                    prob = Some((
                        "HINT_EXCERPT_SNAPSHOT_DRIFT".into(),
                        format!(
                            "스냅샷 `{key}` 가 origin `git show {r12}:{path}` 과 바이트가 다르다 — 스냅샷을 고치지 \
                             않는다(publish 가 쓴 git 바이트가 발췌의 원천이다 · 다시 받으려면 publish 를 새로)"
                        ),
                    ))
                }
                _ => {}
            }
        }
        self.snap_problem_c.push((key.clone(), prob.clone()));
        Ok(prob)
    }

    pub fn sublines(&mut self, loc: &_Loc) -> HR<Option<Vec<String>>> {
        if let Some((_, v)) = self.sublines_c.iter().find(|(l, _)| l == loc) {
            return Ok(v.clone());
        }
        let v = match self.text(loc)? {
            Some(t) => Some((self.substitute)(&t)?.split('\n').map(str::to_string).collect()),
            None => None,
        };
        self.sublines_c.push((loc.clone(), v.clone()));
        Ok(v)
    }

    pub fn normsub(&mut self, loc: &_Loc) -> HR<Option<String>> {
        if let Some((_, v)) = self.normsub_c.iter().find(|(l, _)| l == loc) {
            return Ok(v.clone());
        }
        let v = match self.text(loc)? {
            Some(t) => Some(_norm(&(self.substitute)(&t)?)?),
            None => None,
        };
        self.normsub_c.push((loc.clone(), v.clone()));
        Ok(v)
    }
}

pub fn _heading_key(text: &str) -> HR<String> {
    let s = rx_ws().sub_literal(text, " ");
    let s = pstrip(&s).trim_start_matches('§');
    let s = strip_set(pstrip(s), "`*");
    Ok(casefold(pstrip(s)))
}

fn section_in_headings(label: &str, text: &str, substitute: Option<&dyn Fn(&str) -> HR<String>>) -> HR<bool> {
    let want = _heading_key(label)?;
    if want.is_empty() {
        return Ok(false);
    }
    let mut heads: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in text.split('\n') {
        if rx_fence().is_match(line) {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            if let Some(m) = rx_md_heading().match_at_start(line) {
                let g = group(&m, 1).unwrap_or("");
                if !pstrip(g).is_empty() {
                    heads.push(g.to_string());
                }
            }
        }
    }
    let mut keys: Vec<String> = Vec::new();
    for h in &heads {
        keys.push(_heading_key(h)?);
    }
    if let Some(s) = substitute {
        for h in &heads {
            keys.push(_heading_key(&s(h)?)?);
        }
    }
    Ok(keys.iter().any(|k| k.starts_with(&want)))
}

/// Does excerpt `§<section>` prefix-match a heading line of the markdown source (raw or substituted)? `substitute`
/// must be null here (pass the typed hook through `lint_with`).
pub fn _section_in_headings(label: &str, text: &str, substitute: &J) -> HR<bool> {
    no_callable("substitute", substitute)?;
    section_in_headings(label, text, None)
}

/// `lint(...)` with the stub signature (hooks must be null = defaults · `pii_terms` null = `_UNSET`, a list = terms).
#[allow(clippy::too_many_arguments)]
pub fn lint(
    repo: &Path,
    payload_dir: &Path,
    lineage: &J,
    subst_table: &J,
    manifest: &J,
    bench_report: Option<&Path>,
    facts: &J,
    sealed: bool,
    historical_verify: bool,
    substitute: &J,
    source_text: &J,
    resolve_stem: &J,
    pii_terms: &J,
    draft_dir: Option<&Path>,
    snapshot_origin: &J,
) -> HR<Vec<J>> {
    no_callable("substitute", substitute)?;
    no_callable("source_text", source_text)?;
    no_callable("resolve_stem", resolve_stem)?;
    no_callable("snapshot_origin", snapshot_origin)?;
    let terms: Option<Vec<String>> = pii_terms.as_list().map(|l| l.iter().map(J::display).collect());
    let terms_arg = if pii_terms.is_null() { None } else { Some(terms.as_deref()) };
    lint_with(repo, payload_dir, lineage, subst_table, manifest, bench_report, facts, sealed, historical_verify, terms_arg, draft_dir, &TemplateHooks::default())
}

/// The bench-source failure as Python's `f"{type(e).__name__}: {e}"`.
fn exc_text(e: &HintError) -> String {
    if let Some(t) = e.code.strip_prefix("py:") {
        format!("{t}: {}", e.message)
    } else {
        format!("HintError: {}", e.str())
    }
}

/// The payload linter (fail-closed · side-effect free). `facts` null = read the default snapshot; `pii_terms` None =
/// `_UNSET` (load `.claude/pii_terms.txt`), `Some(None)` = explicit None (= HINT_MISSING_PII_TERMS).
#[allow(clippy::too_many_arguments)]
pub fn lint_with(
    repo: &Path,
    payload_dir: &Path,
    lineage: &J,
    subst_table: &J,
    manifest: &J,
    bench_report: Option<&Path>,
    facts: &J,
    sealed: bool,
    historical_verify: bool,
    pii_terms: Option<Option<&[String]>>,
    draft_dir: Option<&Path>,
    hooks: &TemplateHooks,
) -> HR<Vec<J>> {
    let mut issues: Vec<(String, String, i64, String)> = Vec::new();
    let mut add = |code: &str, file: &str, line: i64, msg: String| {
        issues.push((code.to_string(), file.to_string(), if line == 0 { 1 } else { line }, msg));
    };
    // `except core.HintError` catches HintError only — a Python built-in escapes.
    fn caught(e: HintError) -> HR<HintError> {
        if e.is_py() {
            Err(e)
        } else {
            Ok(e)
        }
    }

    let templates = _load_templates(repo)?;
    let mut facts_v: Option<J> = None;
    if facts.is_null() {
        let snap = default_snapshot_path(payload_dir)?;
        if snap.is_file() {
            facts_v = Some(load_snapshot(&snap)?);
        } else {
            add("HINT_FACT_SNAPSHOT_ABSENT", "-", 1, format!("facts 스냅샷 부재({}) — 사실 블록 재생성 diff 를 검사할 수 없다", core::posix(&snap)));
        }
    } else {
        facts_v = Some(facts.clone());
    }
    let facts: Option<J> = match facts_v {
        Some(f) => Some(_normalize_facts(&f)?),
        None => None,
    };
    let ctx = facts.as_ref().map(|f| _Ctx { repo, facts: f, templates: &templates });
    let null = J::Null;
    let fj: &J = facts.as_ref().unwrap_or(&null);

    let empty = J::dict();
    let lin = if lineage.truthy() { lineage } else { &empty };
    if !lin["documents"].truthy() {
        add("HINT_LINEAGE_EMPTY", "LINEAGE.json", 1, "계보 문서 0건 — 서사 원재료가 없다".into());
    }

    let r = resolvers(repo, subst_table, hooks)?;
    let ident = |t: &str| -> HR<String> { Ok(t.to_string()) };
    let subst_ref: &dyn Fn(&str) -> HR<String> = match r.substitute.as_ref() {
        Some(b) => &**b,
        None => &ident,
    };
    let verifier = r.substitute.is_some();
    let mut sources = _Sources::new(repo, payload_dir, lin, &*r.resolve_stem, &*r.source_text, subst_ref, draft_dir, hooks.snapshot_origin)?;

    let mut docs: Vec<(String, _Doc)> = Vec::new();
    let want_docs = if facts.is_some() { payload_docs(fj)? } else { payload_docs_all() };
    for name in TEMPLATE_FILES {
        let p = payload_dir.join(format!("{name}.md"));
        let fname = format!("{name}.md");
        if !want_docs.contains(&fname) {
            if p.is_file() {
                add(
                    "HINT_DOCUMENT_UNEXPECTED",
                    &fname,
                    1,
                    format!("lite 셀(cell_grade=lite)은 `{fname}` 를 싣지 않는다 — lite 판정은 `{LITE_LABEL}` 이며 부하 곡선 · like-with-like 가 없다"),
                );
            }
            continue;
        }
        if !p.is_file() {
            add("HINT_DOCUMENT_ABSENT", &fname, 1, "필수 문서 부재".into());
            continue;
        }
        docs.push((name.to_string(), _Doc::parse(&fname, &read_utf8(&p)?)));
    }
    let doc_of = |n: &str| docs.iter().find(|(k, _)| k == n).map(|(_, d)| d);

    let mut all_events: Vec<(String, _RawEvent)> = Vec::new();
    let mut all_excerpts: Vec<(String, _Excerpt)> = Vec::new();
    for (name, doc) in &docs {
        let fname = doc.name.clone();
        let tdoc = tmpl(&templates, name)?;
        for (code, i, msg) in &doc.structure {
            add(code, &fname, (*i + 1) as i64, msg.clone());
        }
        // ① chapters
        let want: Vec<(String, String)> = tdoc.chapters.iter().map(|(_, n, t)| (n.clone(), t.clone())).collect();
        let got: Vec<(String, String)> = doc.chapters.iter().map(|(_, n, t)| (n.clone(), t.clone())).collect();
        let got_line = |num: &str| doc.chapters.iter().rev().find(|(_, n, _)| n == num).map(|(i, _, _)| *i).unwrap_or(0);
        for (num, title) in &want {
            let c = got.iter().filter(|x| x.0 == *num && x.1 == *title).count();
            if c == 0 {
                add("HINT_SECTION_ABSENT", &fname, 1, format!("필수 챕터 부재: `## {num} {title}`"));
            } else if c > 1 {
                add("HINT_SECTION_DUPLICATE", &fname, (got_line(num) + 1) as i64, format!("챕터 중복: {num}"));
            }
        }
        let order: Vec<&(String, String)> = got.iter().filter(|x| want.contains(x)).collect();
        let want_in: Vec<&(String, String)> = want.iter().filter(|x| order.contains(x)).collect();
        if order != want_in {
            add("HINT_SECTION_ORDER", &fname, 1, "챕터 순서가 템플릿과 다르다".into());
        }
        for (i, num, title) in &doc.chapters {
            if !want.iter().any(|x| x.0 == *num && x.1 == *title) {
                add("HINT_SECTION_UNKNOWN", &fname, (*i + 1) as i64, format!("템플릿에 없는 챕터: `## {num} {title}`"));
            }
        }
        for (i, text) in &doc.other_h2 {
            add("HINT_SECTION_UNKNOWN", &fname, (*i + 1) as i64, format!("템플릿에 없는 `## ` 헤딩: {} (소제목은 `###`)", py_repr(text)));
        }
        // ② fact blocks
        let mut tfacts: Vec<(String, String)> = Vec::new();
        for f in &tdoc.facts {
            match tfacts.iter_mut().find(|(k, _)| *k == f.fid) {
                Some(e) => e.1 = f.chapter.clone(),
                None => tfacts.push((f.fid.clone(), f.chapter.clone())),
            }
        }
        for f in &doc.facts {
            if !tfacts.iter().any(|(k, _)| *k == f.fid) {
                add("HINT_FACT_BLOCK_UNKNOWN", &fname, (f.start + 1) as i64, format!("템플릿에 없는 사실 블록 FACT:{}", f.fid));
            }
        }
        for (fid, chapter) in &tfacts {
            let found = doc.fact(fid);
            if found.is_empty() {
                add("HINT_FACT_BLOCK_ABSENT", &fname, 1, format!("사실 블록 부재 FACT:{fid}"));
                continue;
            }
            if found.len() > 1 {
                add("HINT_FACT_BLOCK_DUPLICATE", &fname, (found[1].start + 1) as i64, format!("사실 블록 중복 FACT:{fid}"));
            }
            let f0 = found[0];
            let ln0 = (f0.start + 1) as i64;
            if f0.chapter != *chapter {
                add("HINT_FACT_BLOCK_MISPLACED", &fname, ln0, format!("FACT:{fid} 는 §{chapter} 에 있어야 한다"));
            }
            let Some(c) = ctx.as_ref() else { continue };
            if DERIVED_FACTS.contains(&fid.as_str()) {
                continue;
            }
            let rf = fact_renderer(fid).ok_or_else(|| HintError::py("KeyError", py_repr(fid)))?;
            let expected = match rf(c) {
                Ok(s) => s,
                Err(e) => {
                    let e = caught(e)?;
                    add(&e.code, &fname, ln0, format!("FACT:{fid} 재생성 실패: {}", e.message));
                    continue;
                }
            };
            if f0.body.trim_matches('\n') != expected.trim_matches('\n') {
                add(
                    "HINT_FACT_DRIFT",
                    &fname,
                    ln0,
                    format!("기계 사실 블록 FACT:{fid} 가 재생성 결과와 다르다(손으로 고치지 않는다 — 값이 틀리면 증거를 고친다)"),
                );
            }
            if let Some(ph) = rx_fact_placeholder().search(&expected) {
                add(
                    "HINT_FACT_PLACEHOLDER",
                    &fname,
                    ln0,
                    format!("기계 사실 블록 FACT:{fid} 에 절 자리표시 `{}` 가 남아 있다 — 생산자(facts 조립)를 고친다", group(&ph, 0).unwrap_or("")),
                );
            }
        }
        // ③ PROMPT
        let mut tprompts: Vec<(String, &_Prompt)> = Vec::new();
        for p in &tdoc.prompts {
            match tprompts.iter_mut().find(|(k, _)| *k == p.chapter) {
                Some(e) => e.1 = p,
                None => tprompts.push((p.chapter.clone(), p)),
            }
        }
        let mut pprompts: Vec<(String, Vec<&_Prompt>)> = Vec::new();
        if sealed {
            for (i, line) in doc.lines.iter().enumerate() {
                if !doc.fence[i] && rx_prompt_residue().is_search(line) {
                    add("HINT_PROMPT_RESIDUE", &fname, (i + 1) as i64, "봉인 뒤 PROMPT 잔존".into());
                }
                if !doc.fence[i] && rx_selfcheck_residue().is_search(line) {
                    add("HINT_SELFCHECK_RESIDUE", &fname, (i + 1) as i64, "봉인 뒤 저작 자기점검(SELFCHECK) 주석 잔존 — 배포하지 않는다".into());
                }
            }
        }
        for pr in &doc.prompts {
            match pprompts.iter_mut().find(|(k, _)| *k == pr.chapter) {
                Some(e) => e.1.push(pr),
                None => pprompts.push((pr.chapter.clone(), vec![pr])),
            }
            if !tprompts.iter().any(|(k, _)| *k == pr.chapter) {
                add("HINT_PROMPT_TAMPERED", &fname, (pr.start + 1) as i64, format!("템플릿에 없는 PROMPT(§{})", pr.chapter));
            }
        }
        for (chapter, tp) in &tprompts {
            let d = directives(&tp.fields)?;
            let (mut ok, mut actual) = (true, String::new());
            if let Some(c) = ctx.as_ref() {
                match cond_holds(d.condition.as_ref(), c.facts) {
                    Ok(v) => (ok, actual) = v,
                    Err(e) => {
                        let e = caught(e)?;
                        add(&e.code, &fname, (tp.start + 1) as i64, e.message);
                    }
                }
            }
            let lines_in = doc.chapter_lines(chapter);
            let mine: Vec<&_Prompt> = pprompts.iter().find(|(k, _)| k == chapter).map(|(_, v)| v.clone()).unwrap_or_default();
            if !ok {
                let na = na_line(d.condition.as_ref().unwrap(), &actual);
                if !lines_in.iter().any(|i| doc.lines[*i] == na) {
                    add("HINT_PROMPT_ABSENT", &fname, 1, format!("§{chapter}: 조건 불성립인데 '해당 없음' 줄이 없다"));
                }
                for pr in &mine {
                    add("HINT_PROMPT_TAMPERED", &fname, (pr.start + 1) as i64, format!("§{chapter}: 조건 불성립 절에 PROMPT 가 있다"));
                }
                continue;
            }
            let sealed_line = format!("{SEALED_PREFIX}{}", _question(&tp.fields)?);
            let has_sealed = lines_in.iter().any(|i| doc.lines[*i] == sealed_line);
            let mut want_raw: Vec<String> = Vec::new();
            for x in &tp.raw {
                want_raw.push(prstrip(&_cond_text(x, fj)?).to_string());
            }
            let is_same = |pr: &_Prompt| pr.raw.iter().map(|x| prstrip(x).to_string()).collect::<Vec<_>>() == want_raw;
            let same_n = mine.iter().filter(|p| is_same(p)).count();
            let mut tampered = 0;
            for pr in &mine {
                if !is_same(pr) {
                    tampered += 1;
                    add("HINT_PROMPT_TAMPERED", &fname, (pr.start + 1) as i64, format!("§{chapter}: PROMPT 가 템플릿과 다르다(지시를 고치지 않는다)"));
                }
            }
            if sealed && !has_sealed {
                add("HINT_PROMPT_ABSENT", &fname, 1, format!("§{chapter}: 봉인 질문 줄이 없다"));
            } else if !sealed && same_n == 0 && !has_sealed && tampered == 0 {
                add("HINT_PROMPT_ABSENT", &fname, 1, format!("§{chapter}: PROMPT(또는 봉인 질문 줄)가 없다"));
            }
            // ④ authored
            if !d.optional {
                let stat: BTreeSet<&str> = tdoc
                    .lines
                    .iter()
                    .enumerate()
                    .filter(|(i, l)| tdoc.chapter_of[*i] == *chapter && !tdoc.masked[*i] && !pstrip(l).is_empty() && !l.starts_with(AGENT_MARK))
                    .map(|(_, l)| pstrip(l))
                    .collect();
                let heading_idx: BTreeSet<usize> = doc.chapters.iter().filter(|(_, n, _)| n == chapter).map(|(i, _, _)| *i).collect();
                let authored = lines_in.iter().any(|&i| {
                    let l = &doc.lines[i];
                    !doc.masked[i]
                        && !heading_idx.contains(&i)
                        && !pstrip(l).is_empty()
                        && *l != sealed_line
                        && !stat.contains(pstrip(l))
                        && !l.starts_with(AGENT_MARK)
                });
                if !authored {
                    add(
                        "HINT_SECTION_UNAUTHORED",
                        &fname,
                        lines_in.first().map(|i| (*i + 1) as i64).unwrap_or(1),
                        format!("§{chapter}: 저작된 내용이 없다(PROMPT 지시대로 기재한다)"),
                    );
                }
            }
        }
        // ⑤ placeholders
        for (i, line) in doc.lines.iter().enumerate() {
            if line.contains(AGENT_MARK) {
                add("HINT_AGENT_PLACEHOLDER_RESIDUE", &fname, (i + 1) as i64, "미저작 자리표시 `<<AGENT:` 잔존".into());
            }
        }
        // ⑥ events · excerpts · directives
        let (events, exs, malformed) = _scan(doc)?;
        for ln in &malformed {
            add("HINT_EXCERPT_HEADER_SHAPE", &fname, *ln as i64, "발췌 머리는 `> [원문] <stem> §<절>` 이다".into());
        }
        let mut allowed: Vec<(String, Dir)> = Vec::new();
        for p in &tdoc.prompts {
            let d = directives(&p.fields)?;
            match allowed.iter_mut().find(|(k, _)| *k == p.chapter) {
                Some(e) => e.1 = d,
                None => allowed.push((p.chapter.clone(), d)),
            }
        }
        for raw in &events {
            for (code, line, msg) in &raw.errors {
                add(code, &fname, *line as i64, msg.clone());
            }
            if let Some(kind) = raw.event["kind"].as_str().filter(|k| EVENT_KINDS.contains(k)) {
                let ok = allowed.iter().find(|(k, _)| *k == raw.chapter).is_some_and(|(_, d)| d.blocks.iter().any(|(b, _)| b == kind));
                if !ok {
                    add(
                        "HINT_EVENT_KIND_MISPLACED",
                        &fname,
                        raw.line as i64,
                        format!("§{} 에는 `kind: {kind}` 블록이 올 수 없다(템플릿 `블록:` 지시)", raw.chapter),
                    );
                }
            }
        }
        for (chapter, d) in &allowed {
            if let (Some(c), Some(_)) = (ctx.as_ref(), d.condition.as_ref()) {
                match cond_holds(d.condition.as_ref(), c.facts) {
                    Ok((false, _)) => continue,
                    Ok(_) => {}
                    Err(e) => {
                        caught(e)?;
                        continue;
                    }
                }
            }
            for (kind, need) in &d.blocks {
                let have = events
                    .iter()
                    .filter(|r| r.chapter == *chapter && r.errors.is_empty() && r.event["kind"].as_str() == Some(kind.as_str()))
                    .count() as i64;
                if have < *need {
                    let code = if kind == "wall" { "HINT_JOURNEY_WALL_ABSENT" } else { "HINT_EVENT_BLOCK_COUNT" };
                    add(code, &fname, 1, format!("§{chapter}: `kind: {kind}` 블록 {have}개 < {need}"));
                }
            }
            let have_ex = exs.iter().filter(|e| e.chapter == *chapter).count() as i64;
            if have_ex < d.excerpts {
                add("HINT_EXCERPT_REQUIRED", &fname, 1, format!("§{chapter}: 원문 발췌 {have_ex}개 < {}", d.excerpts));
            }
        }
        all_events.extend(events.into_iter().map(|e| (fname.clone(), e)));
        all_excerpts.extend(exs.into_iter().map(|e| (fname.clone(), e)));
    }

    // ⑦ journey
    if doc_of("02-narrative").is_some()
        && !all_events.iter().any(|(f, r)| f == "02-narrative.md" && r.errors.is_empty() && r.event["kind"].as_str() == Some("wall"))
    {
        add("HINT_JOURNEY_WALL_ABSENT", "02-narrative.md", 1, "`kind: wall` 블록이 0개 — 여정이 없다".into());
    }

    // ⑧ ids · sources · signatures
    let mut seen: Vec<(J, (String, usize))> = Vec::new();
    for (fname, raw) in &all_events {
        if !raw.errors.is_empty() {
            continue;
        }
        let ev = &raw.event;
        let eid = ev["id"].clone();
        let eid_s = eid.display();
        let ln = raw.line as i64;
        if let Some((_, (sf, sl))) = seen.iter().find(|(k, _)| *k == eid) {
            add("HINT_EVENT_ID_DUPLICATE", fname, ln, format!("id 중복: {eid_s}(처음 {sf}:{sl})"));
        } else {
            seen.push((eid.clone(), (fname.clone(), raw.line)));
        }
        let mut locs: Vec<_Loc> = Vec::new();
        for entry in _sources_of(ev)? {
            match sources.locate(&_source_token(&entry)?)? {
                None => add(
                    "HINT_EVENT_SOURCE_OUTSIDE_LINEAGE",
                    fname,
                    ln,
                    format!("{eid_s}: 출처 `{entry}` 가 LINEAGE 문서 · evidence_candidates · artifacts 밖이다(또는 모호)"),
                ),
                Some(loc) => match sources.snapshot_problem(&loc)? {
                    Some((c, m)) => add(&c, fname, ln, format!("{eid_s}: {m}")),
                    None => locs.push(loc),
                },
            }
        }
        let kind = ev["kind"].as_str().unwrap_or("");
        if let Some(sig) = ev["서명"].as_str().filter(|s| !pstrip(s).is_empty()) {
            if kind != "wall" && kind != "misdiagnosis" {
                continue;
            }
            if !verifier {
                add("HINT_EXCERPT_VERIFIER_ABSENT", fname, ln, format!("{eid_s}: 치환 함수가 없어 서명을 대조할 수 없다"));
                continue;
            }
            let mut texts: Vec<String> = Vec::new();
            for loc in &locs {
                if let Some(t) = sources.normsub(loc)? {
                    texts.push(t);
                }
            }
            if texts.is_empty() {
                add("HINT_SIGNATURE_UNVERIFIABLE", fname, ln, format!("{eid_s}: 읽을 수 있는 출처가 없어 서명을 대조할 수 없다"));
                continue;
            }
            for piece in rx_ellipsis().split(sig) {
                if pstrip(&piece).is_empty() {
                    continue;
                }
                if (clen(&rx_ws().sub_literal(&piece, "")) as i64) < MIN_SIGNATURE_CHARS {
                    add(
                        "HINT_SIGNATURE_TOO_SHORT",
                        fname,
                        ln,
                        format!(
                            "{eid_s}: 서명 조각이 {MIN_SIGNATURE_CHARS}자(공백 제외) 미만이라 원문 증거가 못 된다: {}",
                            py_repr(&head(pstrip(&piece), 60))
                        ),
                    );
                    continue;
                }
                let frag = _norm(&subst_ref(&piece)?)?;
                if !frag.is_empty() && !texts.iter().any(|t| t.contains(&frag)) {
                    add(
                        "HINT_SIGNATURE_MISMATCH",
                        fname,
                        ln,
                        format!("{eid_s}: 서명 조각이 출처 원문에 없다 — 기억으로 재구성하지 않는다: {}", py_repr(&head(&frag, 60))),
                    );
                }
            }
        }
    }

    // ⑨ excerpt integrity · limits
    let mut per_source: BTreeMap<String, i64> = BTreeMap::new();
    let mut total: i64 = 0;
    for (fname, ex) in &all_excerpts {
        let ln = ex.line as i64;
        total += ex.body.len() as i64;
        let Some(loc) = sources.locate(&ex.stem)? else {
            add("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE", fname, ln, format!("발췌 출처 `{}` 가 LINEAGE · artifacts 밖이다(또는 모호)", ex.stem));
            continue;
        };
        *per_source.entry(loc.1.clone()).or_insert(0) += ex.body.len() as i64;
        if let Some((c, m)) = sources.snapshot_problem(&loc)? {
            add(&c, fname, ln, m);
            continue;
        }
        let quoted = ex.body.join("\n");
        if _norm(&quoted)?.is_empty() {
            add("HINT_EXCERPT_EMPTY", fname, ln, "발췌 본문이 비었다".into());
            continue;
        }
        if !verifier {
            add("HINT_EXCERPT_VERIFIER_ABSENT", fname, ln, "치환 함수가 없어 발췌 무결성을 검사할 수 없다".into());
            continue;
        }
        let Some(src) = sources.normsub(&loc)? else {
            add("HINT_EXCERPT_SOURCE_UNREADABLE", fname, ln, format!("발췌 출처를 읽을 수 없다: {}", loc.1));
            continue;
        };
        let want_q = _norm(&subst_ref(&quoted)?)?;
        let lower = loc.1.to_lowercase();
        if loc.0 != "snapshot" && _MD_SOURCE_EXTS.iter().any(|x| lower.ends_with(x)) {
            let t = sources.text(&loc)?.unwrap_or_default();
            if !section_in_headings(&ex.section, &t, Some(subst_ref))? {
                add(
                    "HINT_EXCERPT_SECTION_UNKNOWN",
                    fname,
                    ln,
                    format!("발췌 머리 `§{}` 가 출처 `{}` 의 제목 줄(`#`…)과 맞지 않는다 — 제목 줄의 앞부분 그대로 적는다", ex.section, loc.1),
                );
            }
        } else {
            let sec = pstrip(&ex.section).to_string();
            let lm = rx_line_label().fullmatch(&sec);
            let lines_s = sources.sublines(&loc)?.unwrap_or_default();
            match lm {
                None => add(
                    "HINT_EXCERPT_SECTION_SHAPE",
                    fname,
                    ln,
                    format!("비-마크다운 출처 `{}` 의 발췌 머리는 `§L<a>-<b>`(줄 범위)다: `§{}` — `hint.sh excerpt --lines a-b` 번호", loc.1, ex.section),
                ),
                Some(lm) => {
                    let a = int_of(group(&lm, 1).unwrap_or("0"));
                    let b = int_of(group(&lm, 2).unwrap_or("0"));
                    let n = lines_s.len() as i64;
                    if a < 1 || b < a || b > n {
                        add(
                            "HINT_EXCERPT_LINES_OUT_OF_RANGE",
                            fname,
                            ln,
                            format!("`§L{a}-{b}` 가 출처 `{}`(정규화 뒤 {n}줄)의 범위 밖이거나 거꾸로다", loc.1),
                        );
                    } else if b - a + 1 > ex.body.len() as i64 + LINE_LABEL_SLACK {
                        add(
                            "HINT_EXCERPT_LINES_TOO_WIDE",
                            fname,
                            ln,
                            format!("`§L{a}-{b}`({}줄)가 인용 {}줄 + {LINE_LABEL_SLACK} 보다 넓다 — 인용한 줄만 가리킨다", b - a + 1, ex.body.len()),
                        );
                    } else if !_norm(&lines_s[(a - 1) as usize..b as usize].join("\n"))?.contains(&want_q) && src.contains(&want_q) {
                        add(
                            "HINT_EXCERPT_OUTSIDE_LINES",
                            fname,
                            ln,
                            format!("인용은 출처 `{}` 에 있으나 `§L{a}-{b}` 줄 안이 아니다 — `hint.sh excerpt --lines` 번호로 고친다", loc.1),
                        );
                    }
                }
            }
        }
        if !src.contains(&want_q) {
            add(
                "HINT_EXCERPT_MISMATCH",
                fname,
                ln,
                format!("발췌가 출처 `{}`(정규화 · 치환 후)의 부분 문자열이 아니다 — 원문 그대로 옮긴다(ANSI · `\\r` 조각은 정규화가 지운다)", loc.1),
            );
        }
    }
    for (k, count) in &per_source {
        if *count > MAX_EXCERPT_LINES_PER_SOURCE {
            add("HINT_EXCERPT_SOURCE_LIMIT", "-", 1, format!("`{k}` 발췌 {count}줄 > {MAX_EXCERPT_LINES_PER_SOURCE}"));
        }
    }
    if total > MAX_EXCERPT_LINES_TOTAL {
        add("HINT_EXCERPT_TOTAL_LIMIT", "-", 1, format!("발췌 합 {total}줄 > {MAX_EXCERPT_LINES_TOTAL}"));
    }

    // ⑩ value-status coverage
    if facts.is_some() {
        let mut cands: Vec<(String, String)> = Vec::new();
        for c in _candidates(fj)? {
            let k = _knob(&c)?;
            let v = _value_text(&c["value"])?;
            match cands.iter_mut().find(|(x, _)| *x == k) {
                Some(e) => e.1 = v,
                None => cands.push((k, v)),
            }
        }
        let mcodes = _missing_codes(&fj["missing"])?;
        if cands.is_empty() && !mcodes.iter().any(|c| _TRIPLET_ABSENT_CODES.contains(&c.as_str())) {
            add(
                "HINT_VALUE_STATUS_CANDIDATES_ABSENT",
                "01-artifacts.md",
                1,
                "서빙 노브 후보 0개인데 트리플렛 부재(`HINT_MISSING_TRIPLET`)가 기재되지 않았다 — value-status 커버리지를 \
                 판정할 수 없다(artifacts.value_status_candidates 가 서빙 yaml 을 읽었는지 확인한다)"
                    .into(),
            );
        }
        let mut blocks: BTreeMap<String, Vec<(String, &_RawEvent)>> = BTreeMap::new();
        for (fname, raw) in &all_events {
            if raw.errors.is_empty() && raw.event["kind"].as_str() == Some("value-status") {
                blocks.entry(pstrip(&raw.event["노브"].display()).to_string()).or_default().push((fname.clone(), raw));
            }
        }
        for (knob, want_value) in &cands {
            let Some(got_b) = blocks.get(knob) else {
                add("HINT_VALUE_STATUS_GAP", "01-artifacts.md", 1, format!("서빙 노브 `{knob}` 의 value-status 블록이 없다"));
                continue;
            };
            if got_b.len() > 1 {
                add("HINT_VALUE_STATUS_DUPLICATE", &got_b[1].0, got_b[1].1.line as i64, format!("노브 `{knob}` 의 value-status 블록이 둘 이상"));
            }
            let v = &got_b[0].1.event["값"];
            if _value_text(v)? != *want_value {
                add(
                    "HINT_VALUE_STATUS_VALUE_MISMATCH",
                    &got_b[0].0,
                    got_b[0].1.line as i64,
                    format!("노브 `{knob}` 값 {} ≠ 표의 값 {}(글자 그대로 적는다)", v.repr(), py_repr(want_value)),
                );
            }
        }
        for (knob, got_b) in &blocks {
            if !cands.iter().any(|(k, _)| k == knob) {
                add("HINT_VALUE_STATUS_UNKNOWN_KNOB", &got_b[0].0, got_b[0].1.line as i64, format!("1.3 표에 없는 노브: `{knob}`"));
            }
        }
    }

    // ⑩a declared-requirement grounding
    let wall_ids: BTreeSet<String> = all_events
        .iter()
        .filter(|(_, r)| r.errors.is_empty() && r.event["kind"].as_str() == Some("wall"))
        .map(|(_, r)| r.event["id"].display())
        .collect();
    for (fname, raw) in &all_events {
        if !raw.errors.is_empty() || raw.event["kind"].as_str() != Some("value-status") || raw.event["지위"].as_str() != Some("declared-requirement") {
            continue;
        }
        let ev = &raw.event;
        let srcs = _sources_of(ev)?;
        let mut parts = vec![if ev["근거"].truthy() { ev["근거"].display() } else { String::new() }];
        parts.extend(srcs.iter().cloned());
        let text = parts.join(" ");
        let cited = rx_wall_id().findall(&text).into_iter().any(|w| wall_ids.contains(&w));
        let mut art_src = false;
        for e in &srcs {
            if is_requirement_artifact(sources.locate(&_source_token(e)?)?.as_ref()) {
                art_src = true;
            }
        }
        if !cited && !art_src {
            add(
                "HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED",
                fname,
                raw.line as i64,
                format!(
                    "{}: declared-requirement 인데 관측된 실패가 없다 — 이 페이로드의 wall id(W<n>)를 근거에 적거나 실패를 \
                     기록한 artifacts/ 파일(트리플렛 밖 — 값을 선언한 파일 자신은 근거가 아니다)을 출처로 단다(없으면 inherited)",
                    ev["id"].display()
                ),
            );
        }
    }

    // ⑩c result_source vocabulary
    if facts.is_some() {
        let ap = _dict(fj, "applied_set")?;
        for pt in ap["patches"].items() {
            let rs = if pt.is_dict() { pt["result_source"].clone() } else { J::Null };
            if rs.truthy() {
                let known = match &rs {
                    J::List(_) | J::Dict(_) => return Err(unhashable(&rs)),
                    J::Str(s) => RESULT_SOURCE_MEANINGS().get(s).is_some(),
                    _ => false,
                };
                if !known {
                    add(
                        "HINT_RESULT_SOURCE_UNKNOWN",
                        "01-artifacts.md",
                        1,
                        format!("적용 판정 결과 출처 `{}` 가 template.RESULT_SOURCE_MEANINGS 에 없다({}) — 어휘를 맞춘다", rs.display(), pt["file"].display()),
                    );
                }
            }
        }
    }

    // ⑩b missing-code vocabulary
    if facts.is_some() {
        let mcodes = _missing_codes(&fj["missing"])?;
        if !mcodes.is_empty() {
            match _missing_meanings() {
                Err(e) => {
                    let e = caught(e)?;
                    add(&e.code, "00-hint.md", 1, e.message);
                }
                Ok(reg) => {
                    let registered: BTreeSet<String> = reg.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default();
                    let mut un: Vec<&String> = mcodes.iter().filter(|c| !registered.contains(*c)).collect();
                    un.sort();
                    un.dedup();
                    for code in un {
                        add(
                            "HINT_MISSING_CODE_UNREGISTERED",
                            "00-hint.md",
                            1,
                            format!("결손 코드 `{code}` 가 evidence.MISSING_CODES 에 없다(코드표에 뜻을 등재하거나 evidence 가 쓰는 코드를 고친다)"),
                        );
                    }
                }
            }
        }
    }

    // ⑪ derived block freshness
    if let Some(d00) = doc_of("00-hint") {
        let t01 = doc_of("01-artifacts").map(|d| d.text.as_str()).unwrap_or("");
        let t02 = doc_of("02-narrative").map(|d| d.text.as_str()).unwrap_or("");
        let want_d = _derive_from_texts(t02, t01)?;
        for fid in DERIVED_FACTS {
            if let Some(g) = d00.fact(fid).first() {
                if g.body.trim_matches('\n') != want_d[fid].display().trim_matches('\n') {
                    add(
                        "HINT_DERIVED_BLOCK_STALE",
                        "00-hint.md",
                        (g.start + 1) as i64,
                        format!("파생 블록 FACT:{fid} 가 현재 hint-event 와 다르다 — `refresh` 를 먼저 부른다"),
                    );
                }
            }
        }
    }

    // ⑫ 03 load-curve re-render diff 0
    if let Some(d03) = doc_of("03-benchmark") {
        let got_b = d03.fact(BENCH_REGION_ID);
        let body = got_b.first().map(|f| f.body.trim_matches('\n').to_string()).unwrap_or_default();
        let gl = got_b.first().map(|f| (f.start + 1) as i64).unwrap_or(1);
        if let Some(br) = bench_report {
            let res = (|| -> HR<(String, Option<String>)> {
                let expected_b = _render_bench_source(repo, &J::from(core::posix(br)))?.trim_matches('\n').to_string();
                let first = pstrip(expected_b.split('\n').next().unwrap_or("")).to_string();
                let cut = crate::render_bench_section::extract_section(&d03.text, &first)?;
                Ok((expected_b, cut))
            })();
            match res {
                Err(e) => add("HINT_BENCH_SECTION_UNVERIFIABLE", "03-benchmark.md", 1, exc_text(&e)),
                Ok((expected_b, cli_cut)) => {
                    use crate::render_bench_section::{CURRENT_CAPTION, LEGACY_CAPTION};
                    let legacy_b = if historical_verify && sealed && expected_b.matches(CURRENT_CAPTION).count() == 1 {
                        Some(expected_b.replacen(CURRENT_CAPTION, LEGACY_CAPTION, 1))
                    } else {
                        None
                    };
                    let accepted_b = match &legacy_b {
                        Some(l) if body == *l => l.clone(),
                        _ => expected_b.clone(),
                    };
                    if body != accepted_b {
                        add(
                            "HINT_BENCH_SECTION_DRIFT",
                            "03-benchmark.md",
                            gl,
                            format!("부하 곡선이 원천(`{}`) 재렌더와 다르다", py_name(&core::posix(br))),
                        );
                    } else if cli_cut.as_deref().map(|c| pstrip(c) != pstrip(&accepted_b)).unwrap_or(true) {
                        add(
                            "HINT_BENCH_SECTION_DRIFT",
                            "03-benchmark.md",
                            gl,
                            "render_bench_section --verify 의 절 추출(첫 제목 줄 ~ 다음 `## `)이 재렌더와 다르다 — 같은 절 제목 \
                             줄이 문서 앞쪽에 한 번 더 있다(CLI 는 첫 줄을 잡는다)"
                                .into(),
                        );
                    }
                }
            }
        } else if facts.is_some() && fj["bench_section_md"].as_str().is_some_and(|s| !pstrip(s).is_empty()) {
            add("HINT_BENCH_SECTION_UNVERIFIABLE", "03-benchmark.md", 1, "부하 곡선이 실렸는데 재렌더할 원천(bench_report)이 주어지지 않았다".into());
        }
    }

    // ⑬ grade markers · banners
    let text00 = doc_of("00-hint").map(|d| d.visible()).unwrap_or_default();
    let tc_m: J = if manifest.is_dict() { manifest["task_class"].clone() } else { J::Null };
    let tc_f: Option<String> = if facts.is_some() { _task_class(fj)? } else { None };
    if tc_m.truthy() && facts.is_some() && tc_m.as_str() != tc_f.as_deref() {
        add("HINT_TASK_CLASS_CONFLICT", "00-hint.md", 1, format!("manifest task_class {} ≠ facts {}", tc_m.repr(), opt_repr(tc_f.as_deref())));
    }
    let map_only = tc_m.as_str() == Some("hint_map_only") || tc_f.as_deref() == Some("hint_map_only");
    if map_only && !text00.contains(MAP_ONLY_MARKER) {
        add(
            "HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING",
            "00-hint.md",
            1,
            format!("task_class=hint_map_only 인데 00 본문에 `{MAP_ONLY_MARKER}` 가 없다(관측 게재 — baseline·권고 ✗)"),
        );
    }
    let bench_m = if manifest.is_dict() { _dict(manifest, "benchmark")? } else { J::dict() };
    let meas_f = if facts.is_some() { _dict(fj, "measurement")? } else { J::dict() };
    if !map_only
        && (bench_m["verdict"].as_str() == Some(MAP_ONLY_MARKER) || meas_f["verdict"].as_str() == Some(MAP_ONLY_MARKER))
        && !text00.contains(MAP_ONLY_MARKER)
    {
        add(
            "HINT_OBSERVATION_ONLY_MARKER_MISSING",
            "00-hint.md",
            1,
            format!("판정 = {MAP_ONLY_MARKER}(기준 c·E·expected 부재 full 셀)인데 00 본문에 `{MAP_ONLY_MARKER}` 가 없다(판정 주장 ✗ — 관측)"),
        );
    }
    if let Some(d00) = doc_of("00-hint") {
        let visible = d00.lines.iter().enumerate().filter(|(i, _)| !d00.masked[*i]).map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("\n");
        for b in banner_lines() {
            if !visible.contains(b) {
                add("HINT_BANNER_ABSENT", "00-hint.md", 1, format!("필수 배너 부재(PROMPT·주석 밖 고정 문구): {b}"));
            }
        }
        match brief(payload_dir) {
            Err(e) => {
                let e = caught(e)?;
                add(&e.code, "00-hint.md", 1, e.message);
            }
            Ok(btext) => {
                let known = fact_numbers(&docs);
                for tok in _brief_numbers(&btext)? {
                    if !known.contains(&tok) {
                        add(
                            "HINT_BRIEF_NUMBER_UNGROUNDED",
                            "00-hint.md",
                            1,
                            format!("brief 수치 `{tok}` 가 사실 블록(FACT · 03 표)에 없다 — 표의 값을 글자 그대로 쓰거나 수치를 빼고 V/W id 로 가리킨다"),
                        );
                    }
                }
            }
        }
    }

    // ⑭ README re-render diff 0
    let readme = payload_dir.join(README_NAME);
    if !readme.is_file() {
        add("HINT_DOCUMENT_ABSENT", README_NAME, 1, "README.md 부재".into());
    } else if let Some(c) = ctx.as_ref() {
        match _render_readme(c) {
            Err(e) => {
                let e = caught(e)?;
                add(&e.code, README_NAME, 1, e.message);
            }
            Ok(w) => {
                if read_utf8(&readme)? != w {
                    add("HINT_README_DRIFT", README_NAME, 1, "README.md 가 형식 버전 렌더와 다르다(손으로 고치지 않는다)".into());
                }
            }
        }
    }

    // ⑮ PII
    let loaded: Option<Vec<String>>;
    let terms: Option<&[String]> = match pii_terms {
        None => {
            loaded = crate::pii::load_pii_terms(repo)?;
            loaded.as_deref()
        }
        Some(t) => t,
    };
    if terms.is_none() {
        add("HINT_MISSING_PII_TERMS", "-", 1, format!("`{}` 부재 — 배포 PII 스캔은 리터럴 없이 통과시키지 않는다", core::REL_PII_TERMS));
    }
    let tv: Vec<String> = terms.map(<[String]>::to_vec).unwrap_or_default();
    let mut names = payload_docs_all();
    names.push(README_NAME.to_string());
    for name in &names {
        let p = payload_dir.join(name);
        if p.is_file() {
            for hit in crate::pii::scan_text(&read_utf8(&p)?, Some(&tv), "deploy", &[], false, true, None)? {
                add("HINT_PII", name, hit.line, format!("배포 PII 검출(pattern={}) — 발췌는 치환 후 원문을 옮긴다", hit.pattern));
            }
        }
    }

    // ⑯ tag literal in prose
    if let Some(base) = fj["base_tag"].as_str().filter(|b| b.starts_with(core::HINT_TAG_PREFIX)) {
        for (_, doc) in &docs {
            for (i, line) in doc.lines.iter().enumerate() {
                if !doc.masked[i] && line.contains(base) {
                    add(
                        "HINT_TAG_LITERAL_IN_PROSE",
                        &doc.name,
                        (i + 1) as i64,
                        "저작 영역에 태그 이름 리터럴이 있다 — 이름은 사실 블록(00 §0.1 머리 · §0.9)이 소유한다(꼬리 확정 전 이름이 남는다)".into(),
                    );
                }
            }
        }
    }

    // ⑰ file verification marks · ⑱ agent requests
    if facts.is_some() {
        for (_slot, rec) in _file_records(&_dict(fj, "slots")?)? {
            for msg in _mark_problems(payload_dir, &rec)? {
                add("HINT_VERIFICATION_MARK_MISMATCH", &rec["path"].display(), 1, msg);
            }
        }
        for req in fj["agent_requests"].items() {
            if req.is_dict() && req["path"].truthy() {
                for (code, msg) in agent_request_problems(payload_dir, req)? {
                    add(&code, &req["path"].display(), 1, msg);
                }
            }
        }
    }

    drop(add);
    issues.sort_by(|a, b| (&a.1, a.2, &a.0, &a.3).cmp(&(&b.1, b.2, &b.0, &b.3)));
    issues.dedup();
    Ok(issues.into_iter().map(|(c, f, l, m)| jd! {"code" => c, "file" => f, "line" => l, "message" => m}).collect())
}

/// One file record's verification mark check (pure read).
pub fn _mark_problems(payload_dir: &Path, rec: &J) -> HR<Vec<String>> {
    let path = payload_dir.join(rec["path"].display());
    if !path.is_file() {
        return Ok(vec![format!("파일 기록이 있는데 페이로드에 파일이 없다: {}", rec["path"].display())]);
    }
    let text = match read_utf8(&path) {
        Ok(t) => t,
        Err(e) => {
            let t = e.code.strip_prefix("py:").unwrap_or("OSError").to_string();
            return Ok(vec![format!("파일을 읽을 수 없다({t}) — 검증 표시를 대조할 수 없다")]);
        }
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let ver = &rec["verification"];
    let gen = &rec["generated_by"];
    let hdr = &rec["header"];
    if ver.as_str() == Some("generated-unverified") {
        let Some(want) = hdr.get("text").and_then(J::as_str).filter(|_| hdr.is_dict()) else {
            return Ok(vec!["generated-unverified 인데 파일 기록에 header 가 없다(표시 3자리 중 파일 첫 줄 부재)".into()]);
        };
        if !want.contains(UNVERIFIED_MARK) || !want.contains(&format!("· 생성: {}", gen.display())) {
            return Ok(vec![format!(
                "header 문구가 기록(verification={} · generated_by={})과 다르다: {}",
                ver.display(),
                gen.display(),
                py_repr(&head(want, 80))
            )]);
        }
        if hdr["style"].as_str() == Some("json-key") {
            let got = match crate::value::loads(&text) {
                Ok(J::Dict(m)) => m.get(&hdr["key"].display()).cloned().unwrap_or(J::Null),
                _ => J::Null,
            };
            return Ok(if got.as_str() == Some(want) {
                Vec::new()
            } else {
                vec![format!("JSON 키 `{}` 의 경고가 기록과 다르다: {}", hdr["key"].display(), py_repr(&head(&got.display(), 60)))]
            });
        }
        let ln = &hdr["line"];
        let li = match ln {
            J::Int(i) => Some(*i),
            J::Bool(b) => Some(*b as i64),
            _ => None,
        };
        let ok = li.is_some_and(|i| i >= 1 && i as usize <= lines.len() && pstrip(lines[(i - 1) as usize]) == pstrip(want));
        if !ok {
            return Ok(vec![format!("파일 {}행이 기록된 경고 줄과 다르다 — 경고를 지우거나 옮기지 않는다", ln.display())]);
        }
        return Ok(Vec::new());
    }
    if lines.iter().take(3).any(|l| l.contains(UNVERIFIED_MARK)) {
        return Ok(vec![format!("verification={} 인데 파일 머리에 `{UNVERIFIED_MARK}` 경고가 있다(기록과 파일이 갈렸다)", ver.display())]);
    }
    Ok(Vec::new())
}

/// One-line comment in the file's format (md/html = HTML comment).
pub fn _comment_line(name: &str, text: &str) -> HR<String> {
    Ok(if [".md", ".markdown", ".html"].iter().any(|x| name.ends_with(x)) { format!("<!-- {text} -->") } else { format!("# {text}") })
}

fn or_display(a: &J, b: &J) -> String {
    if a.truthy() {
        a.display()
    } else {
        b.display()
    }
}

pub fn agent_request_header_line(req: &J) -> HR<String> {
    let header = if req["header"].truthy() { req["header"].display() } else { String::new() };
    _comment_line(&or_display(&req["name"], &req["path"]), &header)
}

/// Agent authoring request scaffold: warning line + `<<AGENT:` placeholder.
pub fn agent_request_scaffold(req: &J) -> HR<String> {
    let inputs_j = &req["inputs"];
    let items: Vec<String> = match inputs_j {
        J::Str(s) => s.chars().map(|c| c.to_string()).collect(),
        J::Dict(m) => m.keys().map(str::to_string).collect(),
        other => other.items().iter().map(J::display).collect(),
    };
    let mut inputs = items.join(" · ");
    if inputs.is_empty() {
        inputs = "PAYLOAD · artifacts".into();
    }
    Ok(format!(
        "{}\n{AGENT_MARK} {} — 입력: {inputs}. 이 줄을 파일 본문으로 바꿔라(증거에서 옮긴 것만 · 실행해 보지 않은 절차를 검증된 것처럼 쓰지 않는다 · 첫 줄 경고는 지우지 않는다)>>\n",
        agent_request_header_line(req)?,
        req["why"].display()
    ))
}

/// Write scaffolds (publish only · never overwrite). Returns the payload-relative paths written.
pub fn write_agent_requests(payload_dir: &Path, reqs: &J) -> HR<Vec<String>> {
    let mut out = Vec::new();
    for req in reqs.items() {
        if !req.is_dict() || !req["path"].truthy() || !req["header"].truthy() {
            continue;
        }
        let rel = req["path"].display();
        if rel.starts_with('/') || rel.split('/').any(|x| x == "..") || !rel.starts_with("artifacts/") {
            return core::fail("HINT_AGENT_REQUEST_PATH", format!("Agent 저작 요청 경로가 artifacts/ 상대가 아니다: {}", py_repr(&rel)), None);
        }
        if rel.ends_with(".json") {
            return core::fail("HINT_AGENT_REQUEST_PATH", format!("JSON 파일은 주석 경고를 달 수 없다 — Agent 저작 요청으로 받지 않는다: {rel}"), None);
        }
        let dst = payload_dir.join(&rel);
        if dst.exists() {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| HintError::py(io_type(&e), core::io_err_text(&e, parent)))?;
        }
        write_text(&dst, &agent_request_scaffold(req)?)?;
        out.push(rel);
    }
    Ok(out)
}

/// One Agent-authored file → [(code, message)].
pub fn agent_request_problems(payload_dir: &Path, req: &J) -> HR<Vec<(String, String)>> {
    let p = payload_dir.join(req["path"].display());
    if !p.is_file() {
        return Ok(vec![("HINT_AGENT_ARTIFACT_ABSENT".into(), "Agent 저작 요청 파일이 페이로드에 없다(publish 가 스캐폴드를 둔다 — 지우지 않는다)".into())]);
    }
    let text = match read_utf8(&p) {
        Ok(t) => t,
        Err(e) => {
            let t = e.code.strip_prefix("py:").unwrap_or("OSError").to_string();
            return Ok(vec![("HINT_AGENT_ARTIFACT_ABSENT".into(), format!("읽을 수 없다({t})"))]);
        }
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let hi = if lines.first().is_some_and(|l| l.starts_with("#!")) { 1 } else { 0 };
    let want = agent_request_header_line(req)?;
    if lines.len() <= hi || pstrip(lines[hi]) != pstrip(&want) {
        out.push(("HINT_VERIFICATION_MARK_MISMATCH".into(), format!("{}행이 경고 줄 `{}…` 이 아니다 — 첫 줄 경고는 지우지 않는다", hi + 1, head(&want, 60))));
    }
    if lines.iter().any(|l| l.contains(AGENT_MARK)) {
        out.push(("HINT_AGENT_PLACEHOLDER_RESIDUE".into(), format!("미저작 자리표시 `{AGENT_MARK}` 잔존")));
    }
    let body = lines.iter().enumerate().any(|(i, l)| !pstrip(l).is_empty() && i != hi && !(i == 0 && hi == 1) && !l.contains(AGENT_MARK));
    if !body {
        out.push(("HINT_AGENT_ARTIFACT_UNAUTHORED".into(), "경고 줄 밖 본문이 없다 — 증거에서 파일 본문을 쓴다".into()));
    }
    Ok(out)
}

/// The LINEAGE sources the payload cites: `{"excerpt_counts": {path: n}, "paths": [...]}`.
pub fn cited_sources(repo: &Path, payload_dir: &Path, lineage: &J, draft_dir: Option<&Path>) -> HR<J> {
    let empty = J::dict();
    let lin = if lineage.truthy() { lineage } else { &empty };
    let rs = |l: &J, s: &str| crate::lineage::resolve_stem(l, s);
    let st = |_: &str| -> HR<Option<String>> { Ok(None) };
    let sb = |t: &str| -> HR<String> { Ok(t.to_string()) };
    let src = _Sources::new(repo, payload_dir, lin, &rs, &st, &sb, draft_dir, None)?;
    let mut counts = Map::new();
    let mut paths: BTreeSet<String> = BTreeSet::new();
    for name in payload_docs_all() {
        let p = payload_dir.join(&name);
        if !p.is_file() {
            continue;
        }
        let (events, exs, _) = _scan(&_Doc::parse(&name, &read_utf8(&p)?))?;
        for e in &exs {
            if let Some((k, path)) = src.locate(&e.stem)? {
                if k == "lineage" {
                    let n = counts.get(&path).and_then(J::as_i64).unwrap_or(0);
                    counts.insert(path.as_str(), n + 1);
                    paths.insert(path);
                }
            }
        }
        for r in &events {
            if !r.errors.is_empty() {
                continue;
            }
            for entry in _sources_of(&r.event)? {
                if let Some((k, path)) = src.locate(&_source_token(&entry)?)? {
                    if k == "lineage" {
                        paths.insert(path);
                    }
                }
            }
        }
    }
    Ok(jd! {"excerpt_counts" => J::Dict(counts), "paths" => paths.into_iter().collect::<Vec<_>>()})
}

pub fn _num_key(tok: &str) -> HR<String> {
    Ok(tok.replace(',', "").trim_end_matches('.').to_string())
}

/// Numbers of the brief to check (decimals · integers of ≥ 3 digits · unit-only suffix).
pub fn _brief_numbers(text: &str) -> HR<Vec<String>> {
    let mut out = Vec::new();
    for m in rx_brief_num().finditer(text) {
        let g = m.get(1).unwrap();
        let tok = g.as_str().trim_end_matches(['.', ',']).to_string();
        let unit: String = text[g.end()..].chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let key = _num_key(&tok)?;
        if tok.matches('.').count() > 1 {
            continue;
        }
        if !key.contains('.') && !NUM_UNITS.contains(&unit.as_str()) {
            continue;
        }
        if key.contains('.') || clen(&key) >= 3 {
            out.push(key);
        }
    }
    Ok(out)
}

fn fact_numbers(docs: &[(String, _Doc)]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, d) in docs {
        for f in &d.facts {
            if DERIVED_FACTS.contains(&f.fid.as_str()) {
                continue;
            }
            for m in rx_any_num().finditer(&f.body) {
                out.insert(m.get(0).unwrap().as_str().replace(',', "").trim_end_matches('.').to_string());
            }
        }
    }
    out
}

/// All number tokens of the machine fact blocks (derived blocks excluded). `docs` = `{name: markdown text}` (the Python
/// takes `{name: _Doc}`; the typed form is internal).
pub fn _fact_numbers(docs: &J) -> HR<Vec<String>> {
    let parsed: Vec<(String, _Doc)> = docs.as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), _Doc::parse(k, &v.display()))).collect()).unwrap_or_default();
    Ok(fact_numbers(&parsed).into_iter().collect())
}

/// Render the 03 load-curve section from its source (bench_report md · sweep_index json).
pub fn render_bench_source(repo: &Path, source: &J) -> HR<String> {
    _render_bench_source(repo, source)
}

pub const _LEGACY_SLOT_INTRO: &str = "> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.";

/// The uniquely owned first visible line of chapter 1.1; reject ambiguous structure.
pub fn slot_intro(text: &str) -> HR<(i64, String)> {
    let doc = _Doc::parse("01-artifacts", text);
    let heads: Vec<usize> = doc.chapters.iter().filter(|(_, c, _)| c == "1.1").map(|(i, _, _)| *i).collect();
    if !doc.structure.is_empty() || heads.len() != 1 {
        return core::fail("HINT_SLOT_INTRO_AMBIGUOUS", "01-artifacts §1.1 구조가 유일하지 않다.", None);
    }
    let mut pos = heads[0] + 1;
    while pos < doc.lines.len() && pstrip(&doc.lines[pos]).is_empty() {
        pos += 1;
    }
    if pos >= doc.lines.len() || doc.chapter_of[pos] != "1.1" || doc.masked[pos] || doc.fence[pos] {
        return core::fail("HINT_SLOT_INTRO_AMBIGUOUS", "01-artifacts §1.1 도입 줄을 식별할 수 없다.", None);
    }
    Ok((pos as i64, doc.lines[pos].clone()))
}

/// Overwrite facts with `changes` and re-render only the machine fact blocks (+ README). Returns rewritten ids (sorted).
pub fn update_facts(repo: &Path, payload_dir: &Path, changes: &J, snapshot_path: Option<&Path>) -> HR<Vec<String>> {
    let snap = match snapshot_path {
        Some(p) => p.to_path_buf(),
        None => default_snapshot_path(payload_dir)?,
    };
    if !snap.is_file() {
        return core::fail("HINT_FACT_SNAPSHOT_ABSENT", format!("facts 스냅샷 부재: {}", core::posix(&snap)), Some("hint.sh publish 가 만든 드래프트에서 실행한다."));
    }
    let old = _normalize_facts(&load_snapshot(&snap)?)?;
    let Some(ch) = changes.as_dict() else {
        return core::fail("HINT_FACTS_SHAPE", "changes 는 dict 여야 한다", None);
    };
    let mut merged = old.as_dict().cloned().unwrap_or_default();
    for (k, v) in ch.iter() {
        merged.insert(k, v.clone());
    }
    let new = _normalize_facts(&J::Dict(merged))?;
    let p01 = payload_dir.join("01-artifacts.md");
    let templates = _load_templates(repo)?;
    let (_, fixed_new) = slot_intro(&tmpl(&templates, "01-artifacts")?.text)?;
    let (intro_index, intro) = if p01.is_file() {
        let (i, s) = slot_intro(&read_utf8(&p01)?)?;
        (Some(i as usize), Some(s))
    } else {
        (None, None)
    };
    let intro_ok = intro.as_deref().is_some_and(|s| s == _LEGACY_SLOT_INTRO || s == fixed_new);
    if !intro_ok {
        return core::fail("HINT_SLOT_INTRO_AMBIGUOUS", "01-artifacts §1.1 도입이 알려진 이전 문구나 현행 템플릿과 다르다.", None);
    }
    let intro = intro.unwrap();
    let migrate = intro == _LEGACY_SLOT_INTRO && intro != fixed_new;
    if new == old && !migrate {
        return Ok(Vec::new());
    }
    let ctx = _Ctx { repo, facts: &new, templates: &templates };
    let mut writes: Vec<(PathBuf, String)> = Vec::new();
    let mut changed: Vec<String> = Vec::new();
    let pdocs = payload_docs(&new)?;
    for name in TEMPLATE_FILES {
        let p = payload_dir.join(format!("{name}.md"));
        if !pdocs.contains(&format!("{name}.md")) {
            continue;
        }
        if !p.is_file() {
            return core::fail("HINT_DOCUMENT_ABSENT", format!("사실 블록을 다시 쓸 문서 부재: {name}.md"), None);
        }
        let doc = _Doc::parse(name, &read_utf8(&p)?);
        let mut lines = doc.lines.clone();
        let mig_here = name == "01-artifacts" && migrate;
        if mig_here {
            if let Some(ix) = intro_index {
                if ix < lines.len() {
                    lines[ix] = fixed_new.clone();
                }
            }
            changed.push("01-artifacts:fixed-introduction".into());
        }
        let mut edits: Vec<(usize, usize, Vec<String>)> = Vec::new();
        for f in &doc.facts {
            if DERIVED_FACTS.contains(&f.fid.as_str()) {
                continue;
            }
            let Some(rf) = fact_renderer(&f.fid) else { continue };
            let want = rf(&ctx)?;
            if f.body.trim_matches('\n') == want.trim_matches('\n') {
                continue;
            }
            let body: Vec<String> = if want.is_empty() { Vec::new() } else { want.split('\n').map(str::to_string).collect() };
            let body = if f.open_ended {
                let mut b = vec![String::new()];
                b.extend(body);
                b.push(String::new());
                b
            } else {
                body
            };
            edits.push((f.start + 1, f.end, body));
            changed.push(f.fid.clone());
        }
        if !edits.is_empty() || mig_here {
            edits.sort_by(|a, b| b.0.cmp(&a.0));
            for (s, e, b) in edits {
                splice(&mut lines, s, e, b);
            }
            writes.push((p, lines.join("\n")));
        }
    }
    let readme = payload_dir.join(README_NAME);
    let want_readme = _render_readme(&ctx)?;
    if readme.is_file() && read_utf8(&readme)? != want_readme {
        writes.push((readme, want_readme));
        changed.push("README".into());
    }
    for (p, text) in &writes {
        write_text(p, text)?;
    }
    core::write_json(&snap, &new)?;
    let set: BTreeSet<String> = changed.into_iter().collect();
    Ok(set.into_iter().collect())
}

/// Render with render_bench_section's public functions (same precedence as its CLI · source name = file name).
pub fn _render_bench_source(repo: &Path, source: &J) -> HR<String> {
    use crate::render_bench_section as rbs;
    let s = match source {
        J::Dict(m) => m.get("__path__").map(J::display).unwrap_or_default(),
        other => other.display(),
    };
    let p0 = PathBuf::from(&s);
    let p = if p0.is_absolute() { p0 } else { repo.join(p0) };
    let text = read_utf8(&p)?;
    let pname = py_name(&core::posix(&p));
    // pathlib `suffix == ".json"`: the last dot-suffix of a name that is not only a leading dot.
    let suffix_json = pname.ends_with(".json") && pname.trim_start_matches('.').contains('.') && pname.trim_start_matches('.') != "json";
    if suffix_json {
        let doc = crate::value::loads(&text).map_err(|e| HintError::py("JSONDecodeError", e))?;
        return rbs::render(&rbs::parse_sweep_index(&doc)?, &pname);
    }
    if rbs::is_lite_report(&text)? {
        return rbs::render_lite(&rbs::parse_lite_report(&text)?, &pname);
    }
    rbs::render(&rbs::parse_report(&text)?, &pname)
}
