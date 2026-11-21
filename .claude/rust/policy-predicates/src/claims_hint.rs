//! Clauses ported against the hint-publisher owner (HINT_TAG_ACTIVATION_GATE.C1-C6, LAST_GOOD_ROLLBACK_ANCHOR.C3).
//!
//! The original loads `hintlib` + `hint.py` in-process and calls them in an isolated git sandbox; this port calls the
//! same functions of the hint-publisher Rust crate (`hintlib`) in-process, in the same sandbox shape
//! (`branch::selftest_env` + `branch::selftest_repo`, every remote a bare repo inside the temp dir). Where the original
//! reads a module's SOURCE (the AST of `tag.py`'s `git tag -a` call site, `hint.py`'s approval call graph, the str
//! literals of the engine), the same property is judged on the Rust sources of the hint crate through a small Rust
//! lexer ([`lex`]) -- comments (doc comments included) never count, like docstrings in the original.
//! Other owners: render_sub_env (terraforming_runtime), node_role_contract (role_contract), campaign_init /
//! campaign_template_validator (sweep_stop), completion_gate (policy_gate).
#![allow(unused_imports)]
use crate::support::*;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use hintlib::core::{HintError, HR};
use hintlib::value::{Map, J};
use hintlib::{branch, cli, core, evidence, jd, pii, tag, template};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const HINT_CRATE_SRC: &str = ".claude/skills/hint-publisher/rust/src";
const HINT_CRATE_TOML: &str = ".claude/skills/hint-publisher/rust/Cargo.toml";
const HINT_TAG_RS: &str = ".claude/skills/hint-publisher/rust/src/tag.rs";
const HINT_CATALOG_RS: &str = ".claude/skills/hint-publisher/rust/src/catalog.rs";
const HINT_CLI_DIR: &str = ".claude/skills/hint-publisher/rust/src/cli";
const HINT_CLI_MOD_RS: &str = ".claude/skills/hint-publisher/rust/src/cli/mod.rs";
const HINT_SKILL_REL: &str = ".claude/skills/hint-publisher/SKILL.md";
const HINT_TEMPLATE_00_REL: &str = ".claude/skills/hint-publisher/templates/00-hint.prompt.md";
const UPSTREAM_SKILL_REL: &str = ".claude/skills/upstream-version-watch/SKILL.md";
const SYNC_TO_SUB: &str = ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh";
const FIXTURE_TAG: &str = "hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-mmp-eager";
const FIXTURE_UTC: &str = "2026-09-21T10:21:01Z";
const OPERATOR_NAME: &str = "Operator Realname";

fn operator_email() -> String {
    ["operator.realname", "corp.example"].join("@")
}

fn operator_env() -> Vec<(String, String)> {
    let e = operator_email();
    vec![
        ("GIT_AUTHOR_NAME".into(), OPERATOR_NAME.into()),
        ("GIT_AUTHOR_EMAIL".into(), e.clone()),
        ("GIT_COMMITTER_NAME".into(), OPERATOR_NAME.into()),
        ("GIT_COMMITTER_EMAIL".into(), e),
    ]
}

// ---------------------------------------------------------------------------------------------- owner error plumbing

/// An uncaught owner error fails the clause the way the original's uncaught `HintError` does.
fn hx<T>(r: HR<T>) -> R<T> {
    r.map_err(|e| Fail::new(if e.is_py() { e.code.trim_start_matches("py:") } else { "HintError" }, e.str()))
}

fn code_of<T>(f: impl FnOnce() -> HR<T>) -> Option<String> {
    branch::code_of(f)
}

fn s(v: &str) -> String {
    v.to_string()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// `_hint_footer(anchor, tag)`.
fn hint_footer(anchor: &str) -> J {
    jd! {"version" => "2", "tag" => FIXTURE_TAG, "topology" => "multi TP=2(Ray)", "anchor" => anchor,
         "manifest_ref" => "docs/_evidence/fixture.work-manifest.json",
         "bench_ref" => "../benchmark/benchmark_fixture.yaml", "bench_kind" => "certificate"}
}

/// `_hint_sandbox(prefix)`: temp dir + `branch.selftest_env` isolation with the operator identity planted in env.
struct Sandbox {
    // drop order: the env guard first, then the directory
    _env: branch::SelftestEnv,
    dir: TempDir,
}

impl Sandbox {
    fn new(prefix: &str) -> R<Sandbox> {
        let dir = TempDir::new(prefix)?;
        let env = hx(branch::selftest_env(dir.path(), &operator_env()))?;
        Ok(Sandbox { _env: env, dir })
    }
    fn tmp(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }
}

struct Fx {
    repo: PathBuf,
    source: String,
    payload: PathBuf,
    anchor: String,
    guide: String,
}

/// `_hint_payload_commit(tmp, name=, extra_files=, repo=)`.
fn payload_commit(tmp: &Path, name: &str, extra: Option<&[(String, String)]>, repo: Option<&Path>) -> HR<Fx> {
    let repo = match repo {
        Some(r) => r.to_path_buf(),
        None => branch::selftest_repo(tmp, "repo")?,
    };
    let source = core::git_out(&repo, &["rev-parse", "HEAD"])?;
    let guide = branch::selftest_guide(&repo, Some(branch::BRANCH_FORMAT), None, branch::FX_GUIDE_BODY, None)?;
    let pdir = tmp.join(name);
    branch::write_fixture_payload(&pdir, FIXTURE_TAG, Some(&source), extra, None, branch::FX_BRIEF)?;
    let message = branch::commit_message(&pdir, FIXTURE_UTC)?;
    let anchor = branch::commit_payload(&repo, &pdir, &message, FIXTURE_UTC, Some(&guide), None)?;
    Ok(Fx { repo, source, payload: pdir, anchor, guide })
}

fn git_ok(repo: &Path, args: &[&str]) -> R<String> {
    hx(core::git_out(repo, args))
}

fn problems_of(e: &HintError) -> Vec<String> {
    tag::problem_codes(e.problems.as_deref().unwrap_or(&[])).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------- Rust source lexer

#[derive(Clone, Debug, PartialEq)]
pub enum Tk {
    Ident,
    Str,
    Punct,
    Other,
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub k: Tk,
    /// identifier / punct char / the decoded string literal value
    pub s: String,
    pub line: usize,
    /// byte offsets of the token in the source
    pub start: usize,
    pub end: usize,
}

fn decode_escapes(raw: &str) -> String {
    let mut out = String::new();
    let mut it = raw.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('x') => {
                let h: String = [it.next(), it.next()].iter().flatten().collect();
                if let Ok(v) = u8::from_str_radix(&h, 16) {
                    out.push(v as char);
                }
            }
            Some('u') => {
                let mut h = String::new();
                if it.peek() == Some(&'{') {
                    it.next();
                    for d in it.by_ref() {
                        if d == '}' {
                            break;
                        }
                        h.push(d);
                    }
                }
                if let Some(ch) = u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some('\n') => {
                while matches!(it.peek(), Some(w) if w.is_whitespace()) {
                    it.next();
                }
            }
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Tokenize Rust source: comments (line, nested block, doc) are skipped; string literals (normal, raw, byte) are
/// decoded; char literals and lifetimes are `Other`.
pub fn lex(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut out = Vec::new();
    let count_lines = |a: usize, z: usize| src[a..z].matches('\n').count();
    while i < n {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            let start = i;
            let mut depth = 0i32;
            while i < n {
                if b[i] == b'/' && i + 1 < n && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && i + 1 < n && b[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            line += count_lines(start, i.min(n));
            continue;
        }
        // raw strings r"..." r#"..."# br"..."
        let raw_at = |j: usize| -> Option<(usize, usize)> {
            // returns (hashes, index of opening quote)
            let mut k = j;
            if k < n && b[k] == b'b' {
                k += 1;
            }
            if k < n && b[k] == b'r' {
                k += 1;
                let mut h = 0;
                while k < n && b[k] == b'#' {
                    h += 1;
                    k += 1;
                }
                if k < n && b[k] == b'"' {
                    return Some((h, k));
                }
            }
            None
        };
        let prev_ident = out.last().map(|t: &Tok| t.end == i && t.k == Tk::Ident).unwrap_or(false);
        if !prev_ident && (c == b'r' || c == b'b') {
            if let Some((h, q)) = raw_at(i) {
                let close: String = std::iter::once('"').chain(std::iter::repeat('#').take(h)).collect();
                let body_start = q + 1;
                let end_rel = src[body_start..].find(&close).unwrap_or(n - body_start);
                let body = &src[body_start..body_start + end_rel];
                let end = (body_start + end_rel + close.len()).min(n);
                out.push(Tok { k: Tk::Str, s: body.to_string(), line, start: i, end });
                line += count_lines(i, end);
                i = end;
                continue;
            }
            if c == b'b' && i + 1 < n && b[i + 1] == b'"' {
                i += 1; // byte string: fall through to the normal string branch
            }
        }
        if c == b'"' || (b[i] == b'"') {
            let start = if c == b'b' { i - 1 } else { i };
            let mut j = i + 1;
            while j < n && b[j] != b'"' {
                if b[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            let body = &src[i + 1..j.min(n)];
            let end = (j + 1).min(n);
            out.push(Tok { k: Tk::Str, s: decode_escapes(body), line, start, end });
            line += count_lines(i, end);
            i = end;
            continue;
        }
        if c == b'\'' {
            // char literal or lifetime
            let rest = &src[i + 1..];
            let mut chars = rest.char_indices();
            let first = chars.next();
            let is_char = match first {
                Some((_, '\\')) => true,
                Some((_, ch)) => rest[ch.len_utf8()..].starts_with('\''),
                None => false,
            };
            if is_char {
                let mut j = i + 1;
                if b[j] == b'\\' {
                    j += 2;
                    while j < n && b[j] != b'\'' {
                        j += 1;
                    }
                } else {
                    j += first.map(|(_, ch)| ch.len_utf8()).unwrap_or(1);
                }
                let end = (j + 1).min(n);
                out.push(Tok { k: Tk::Other, s: src[i..end].to_string(), line, start: i, end });
                i = end;
            } else {
                let mut j = i + 1;
                while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                out.push(Tok { k: Tk::Other, s: src[i..j].to_string(), line, start: i, end: j });
                i = j;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            let mut j = i;
            if c >= 0x80 {
                // non-ASCII outside literals/comments: skip the char
                let ch = src[i..].chars().next().unwrap();
                i += ch.len_utf8();
                continue;
            }
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            // r#ident
            out.push(Tok { k: Tk::Ident, s: src[i..j].to_string(), line, start: i, end: j });
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || (b[j] == b'.' && j + 1 < n && b[j + 1].is_ascii_digit())) {
                j += 1;
            }
            out.push(Tok { k: Tk::Other, s: src[i..j].to_string(), line, start: i, end: j });
            i = j;
            continue;
        }
        out.push(Tok { k: Tk::Punct, s: (c as char).to_string(), line, start: i, end: i + 1 });
        i += 1;
    }
    out
}

fn is_p(t: &Tok, p: &str) -> bool {
    t.k == Tk::Punct && t.s == p
}

fn is_id(t: &Tok, p: &str) -> bool {
    t.k == Tk::Ident && t.s == p
}

/// The token index of the bracket closing the one at `open` (`(`/`[`/`{`).
fn matching(toks: &[Tok], open: usize) -> usize {
    let (o, c) = match toks[open].s.as_str() {
        "(" => ("(", ")"),
        "[" => ("[", "]"),
        _ => ("{", "}"),
    };
    let mut depth = 0i64;
    for (j, t) in toks.iter().enumerate().skip(open) {
        if is_p(t, o) {
            depth += 1;
        } else if is_p(t, c) {
            depth -= 1;
            if depth == 0 {
                return j;
            }
        }
    }
    toks.len() - 1
}

/// One `fn` item of a source: its name and the token range of its body (`{` .. `}` inclusive).
#[derive(Clone, Debug)]
pub struct RsFn {
    pub name: String,
    pub sig: (usize, usize),
    pub body: (usize, usize),
}

pub fn functions(toks: &[Tok]) -> Vec<RsFn> {
    let mut out = Vec::new();
    for i in 0..toks.len().saturating_sub(1) {
        if !(is_id(&toks[i], "fn") && toks[i + 1].k == Tk::Ident) {
            continue;
        }
        let mut j = i + 2;
        let mut depth = 0i64;
        let mut open = None;
        while j < toks.len() {
            let t = &toks[j];
            if is_p(t, "(") || is_p(t, "[") || is_p(t, "<") {
                depth += 1;
            } else if is_p(t, ")") || is_p(t, "]") || (is_p(t, ">") && !(j > 0 && is_p(&toks[j - 1], "-"))) {
                depth -= 1;
            } else if depth <= 0 && is_p(t, ";") {
                break;
            } else if depth <= 0 && is_p(t, "{") {
                open = Some(j);
                break;
            }
            j += 1;
        }
        if let Some(o) = open {
            out.push(RsFn { name: toks[i + 1].s.clone(), sig: (i, o), body: (o, matching(toks, o)) });
        }
    }
    out
}

/// Every `(line, callee)` in a token range: an identifier directly followed by `(` that is not a definition, a
/// keyword or a macro (a macro is `name!(`).
fn calls_in(toks: &[Tok], range: (usize, usize)) -> Vec<(usize, String)> {
    const KW: [&str; 12] = ["if", "while", "match", "for", "return", "fn", "loop", "in", "as", "let", "mut", "move"];
    let mut out = Vec::new();
    for j in range.0..range.1 {
        let t = &toks[j];
        if t.k == Tk::Ident && is_p(&toks[j + 1], "(") && !KW.contains(&t.s.as_str()) && !(j > 0 && is_id(&toks[j - 1], "fn")) {
            out.push((t.line, t.s.clone()));
        }
    }
    out
}

fn strings_in(toks: &[Tok], range: (usize, usize)) -> Vec<String> {
    toks[range.0..=range.1.min(toks.len() - 1)].iter().filter(|t| t.k == Tk::Str).map(|t| t.s.clone()).collect()
}

/// Every Rust source of a directory tree (sorted, repository-relative).
fn rs_files(rel_dir: &str) -> Vec<String> {
    let root = repo_root();
    let mut out: Vec<String> = rglob_files(&root.join(rel_dir))
        .into_iter()
        .filter(|r| r.ends_with(".rs"))
        .map(|r| format!("{rel_dir}/{r}"))
        .collect();
    out.sort();
    out
}

fn test_plane(name: &str) -> bool {
    name.contains("selftest") || name.contains("self_test") || name.starts_with("_fx") || name.starts_with("_FX")
}

// ---------------------------------------------------------------------------------------------- O6 approval order

const APPROVAL_CALLS: [&str; 1] = ["approval_for"];
const SIDE_EFFECT_CALLS: [&str; 4] = ["commit_payload", "seal", "set_promotion_target", "push_tag"];
const GATED_CALLS: [&str; 3] = ["commit_payload", "seal", "set_promotion_target"];

/// `_approval_order_problems(src)` over the hint CLI's Rust sources (`src/cli/*.rs` except the self-test files),
/// judged transitively over the CLI's own functions. Same rules (A)(B)(C) as the original.
/// The dispatcher `main` is not judged and not counted as a caller: its `match` arms are alternatives, not a sequence
/// (an earlier `cmd_publish` arm that reaches the approval says nothing about the `cmd_continue` arm). This keeps the
/// original's shape, where the argparse handlers (`set_defaults(fn=...)`) are never called by name and so are the
/// entry points.
pub fn approval_order_problems(sources: &[String]) -> Vec<String> {
    let mut fns: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    for src in sources {
        let toks = lex(src);
        for f in functions(&toks) {
            if test_plane(&f.name) || f.name == "main" {
                continue;
            }
            let mut calls = calls_in(&toks, f.body);
            calls.sort();
            fns.entry(f.name.clone()).or_default().extend(calls);
        }
    }
    let closure = |seed: &BTreeSet<String>| -> BTreeSet<String> {
        let mut out: BTreeSet<String> = fns.iter().filter(|(_, c)| c.iter().any(|(_, x)| seed.contains(x))).map(|(n, _)| n.clone()).collect();
        loop {
            let more: BTreeSet<String> = fns
                .iter()
                .filter(|(n, c)| !out.contains(*n) && c.iter().any(|(_, x)| out.contains(x)))
                .map(|(n, _)| n.clone())
                .collect();
            if more.is_empty() {
                return out;
            }
            out.extend(more);
        }
    };
    let set = |v: &[&str]| -> BTreeSet<String> { v.iter().map(|x| x.to_string()).collect() };
    let targets: BTreeMap<&str, BTreeSet<String>> =
        [("approval", set(&APPROVAL_CALLS)), ("side", set(&SIDE_EFFECT_CALLS)), ("gated", set(&GATED_CALLS))].into_iter().collect();
    let reach: BTreeMap<&str, BTreeSet<String>> = targets.iter().map(|(k, v)| (*k, closure(v))).collect();
    let hits = |callee: &str, kind: &str| targets[kind].contains(callee) || reach[kind].contains(callee);
    let first_ok: BTreeMap<String, Option<usize>> =
        fns.iter().map(|(n, c)| (n.clone(), c.iter().filter(|(_, x)| hits(x, "approval")).map(|(l, _)| *l).min())).collect();
    let mut problems = Vec::new();
    let mut guarded_somewhere = false;
    for (name, calls) in &fns {
        let ok = first_ok[name];
        for (ln, c) in calls {
            if hits(c, "side") && !hits(c, "approval") {
                if let Some(o) = ok {
                    if *ln < o {
                        problems.push(format!("{name}: {c} (line {ln}) runs before the approval is consulted (line {o})"));
                    }
                }
            }
            if hits(c, "gated") {
                if let Some(o) = ok {
                    if o < *ln {
                        guarded_somewhere = true;
                    }
                }
            }
        }
    }
    let gated = set(&GATED_CALLS);
    let before_ok = |n: &str, ln: usize| first_ok[n].map(|o| ln < o).unwrap_or(true);
    let mut exposed: BTreeSet<String> =
        fns.iter().filter(|(n, c)| c.iter().any(|(ln, x)| gated.contains(x) && before_ok(n, *ln))).map(|(n, _)| n.clone()).collect();
    loop {
        let more: BTreeSet<String> = fns
            .iter()
            .filter(|(n, c)| !exposed.contains(*n) && c.iter().any(|(ln, x)| exposed.contains(x) && before_ok(n, *ln)))
            .map(|(n, _)| n.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        exposed.extend(more);
    }
    let called: BTreeSet<String> = fns.values().flat_map(|c| c.iter().map(|(_, x)| x.clone())).collect();
    for name in exposed.difference(&called) {
        problems.push(format!("{name}: an entry point reaches a branch/tag side effect without consulting the approval"));
    }
    for g in ["commit_payload", "seal"] {
        if !fns.values().any(|c| c.iter().any(|(_, x)| x == g)) {
            problems.push(format!("no production call to {g} -- the approval order cannot be judged (vacuous)"));
        }
    }
    if !guarded_somewhere {
        problems.push("no production function consults the approval before a gated side effect".into());
    }
    problems
}

/// The production sources of the hint CLI (`src/cli/*.rs` without the self-test files).
fn cli_sources() -> R<Vec<String>> {
    let mut out = Vec::new();
    for rel in rs_files(HINT_CLI_DIR) {
        let base = rel.rsplit('/').next().unwrap_or("");
        if test_plane(base) {
            continue;
        }
        out.push(read(&rel)?);
    }
    req(!out.is_empty(), format!("{HINT_CLI_DIR}/*.rs is absent -- the single hint-publisher CLI (SPEC §4) is missing"))?;
    Ok(out)
}

/// The text of `fn <name>` (signature + body) in a Rust source.
fn fn_text(src: &str, name: &str) -> R<String> {
    let toks = lex(src);
    let f = functions(&toks).into_iter().find(|f| f.name == name);
    req(f.is_some(), format!("function {} not found", repr(name)))?;
    let f = f.unwrap();
    Ok(src[toks[f.sig.0].start..toks[f.body.1].end].to_string())
}

/// The str literals of `fn <name>`'s body (the `_string_constants(inspect.getsource(fn))` of the original).
fn fn_literals(src: &str, name: &str) -> R<BTreeSet<String>> {
    let toks = lex(src);
    let f = functions(&toks).into_iter().find(|f| f.name == name);
    req(f.is_some(), format!("function {} not found", repr(name)))?;
    Ok(strings_in(&toks, f.unwrap().body).into_iter().collect())
}

/// The declared type of parameter `param` of `fn <name>` (`None` = no such parameter).
fn param_type(src: &str, name: &str, param: &str) -> R<Option<String>> {
    let toks = lex(src);
    let f = functions(&toks).into_iter().find(|f| f.name == name);
    req(f.is_some(), format!("function {} not found", repr(name)))?;
    let f = f.unwrap();
    let mut j = f.sig.0;
    while j < f.sig.1 && !is_p(&toks[j], "(") {
        j += 1;
    }
    let close = matching(&toks, j);
    let mut k = j + 1;
    while k < close {
        if toks[k].k == Tk::Ident && toks[k].s == param && is_p(&toks[k + 1], ":") && !is_p(&toks[k + 2], ":") {
            let mut e = k + 2;
            let mut depth = 0i64;
            while e < close {
                let t = &toks[e];
                if is_p(t, "<") || is_p(t, "(") || is_p(t, "[") {
                    depth += 1;
                } else if is_p(t, ">") || is_p(t, ")") || is_p(t, "]") {
                    depth -= 1;
                } else if depth == 0 && is_p(t, ",") {
                    break;
                }
                e += 1;
            }
            return Ok(Some(src[toks[k + 2].start..toks[e - 1].end].to_string()));
        }
        k += 1;
    }
    Ok(None)
}

// ---------------------------------------------------------------------------------------------- tar listing

/// The regular-file member names of a tar stream (ustar `prefix/name`, pax `path` records).
fn tar_file_names(data: &[u8]) -> R<Vec<String>> {
    let mut out = Vec::new();
    let mut off = 0usize;
    let mut pax_path: Option<String> = None;
    let cstr = |b: &[u8]| -> String {
        let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
        String::from_utf8_lossy(&b[..end]).into_owned()
    };
    while off + 512 <= data.len() {
        let h = &data[off..off + 512];
        if h.iter().all(|&x| x == 0) {
            break;
        }
        let size_s = cstr(&h[124..136]);
        let size = usize::from_str_radix(size_s.trim(), 8).map_err(|_| Fail::predicate("unreadable tar header size"))?;
        let typ = h[156];
        let body_start = off + 512;
        let body = &data[body_start..(body_start + size).min(data.len())];
        match typ {
            b'x' => {
                for rec in String::from_utf8_lossy(body).split('\n') {
                    if let Some((_, kv)) = rec.split_once(' ') {
                        if let Some(p) = kv.strip_prefix("path=") {
                            pax_path = Some(p.to_string());
                        }
                    }
                }
            }
            b'g' => {}
            b'0' | 0 => {
                let name = match pax_path.take() {
                    Some(p) => p,
                    None => {
                        let prefix = cstr(&h[345..500]);
                        let nm = cstr(&h[0..100]);
                        if prefix.is_empty() { nm } else { format!("{prefix}/{nm}") }
                    }
                };
                out.push(name);
            }
            _ => {
                pax_path = None;
            }
        }
        off = body_start + size.div_ceil(512) * 512;
    }
    out.sort();
    Ok(out)
}

// =============================================================================
// HINT_TAG_ACTIVATION_GATE
// =============================================================================

/// The single production `git tag -a` call site of the hint crate's `tag.rs` (argv string arrays of non-test-plane
/// functions that carry both `tag` and `-a`).
fn tag_a_call_sites(src: &str) -> Vec<Vec<String>> {
    let toks = lex(src);
    let mut out = Vec::new();
    for f in functions(&toks) {
        if f.name.contains("selftest") {
            continue;
        }
        let mut j = f.body.0;
        while j < f.body.1 {
            if is_p(&toks[j], "[") {
                let close = matching(&toks, j);
                let consts: Vec<String> = (j + 1..close).filter(|&k| toks[k].k == Tk::Str).map(|k| toks[k].s.clone()).collect();
                if consts.iter().any(|c| c == "tag") && consts.iter().any(|c| c == "-a") {
                    out.push(consts);
                }
            }
            j += 1;
        }
    }
    out
}

pub fn predicate_HINT_TAG_ACTIVATION_GATE_C1() -> R {
    // The scaffold is authored in a gitignored draft (X2) -- a payload never stands on a tracked path.
    req(core::REL_DRAFTS == "hints/.drafts", format!("the scaffold staging dir moved: {}", repr(core::REL_DRAFTS)))?;
    req(read(".gitignore")?.contains("hints/.drafts/"), "the scaffold staging dir must be gitignored (never at HEAD)")?;
    let allow = strs(&branch::PAYLOAD_ALLOWLIST_TOP);
    req(
        allow == strs(&["README.md", "00-hint.md", "01-artifacts.md", "02-narrative.md", "03-benchmark.md", "PAYLOAD.json", "LINEAGE.json", "PROVENANCE.json"]),
        format!("payload top-level allowlist drifted: {}", repr_list(&allow)),
    )?;
    let lite_req: Vec<String> = allow.iter().filter(|t| *t != "03-benchmark.md").cloned().collect();
    req(
        hx(branch::required_top(&J::Null))? == allow && hx(branch::required_top(&jd! {"cell_grade" => "lite"}))? == lite_req,
        "payload required set must equal the allowlist except 03-benchmark.md for a lite-only cell",
    )?;
    for rel in ["notes.txt", ".claude/skills/x.py", "artifacts/triplet/.git/config", "artifacts/unknown/x", "artifacts/compose/.gitattributes"] {
        req(hx(branch::allowlist_violation(rel))?.is_some(), format!("{} must be outside the payload allowlist", repr(rel)))?;
    }
    req(hx(branch::allowlist_violation("artifacts/compose/sub_recipe.json"))?.is_none(), "a slot file must be admitted")?;

    let fields = hint_footer(&"a".repeat(40));
    let field_keys: Vec<String> = fields.as_dict().map(|m| m.keys().map(String::from).collect()).unwrap_or_default();
    let want = strs(&["version", "tag", "topology", "anchor", "manifest_ref", "bench_ref", "bench_kind"]);
    req(strs(&tag::FOOTER_FIELDS) == want && want == field_keys, "footer v2 contract must be exactly the 7 address fields, in order")?;
    req(strs(&tag::BENCH_KINDS) == strs(&["certificate", "bench_report"]), format!("bench_kind vocabulary drifted: {}", repr_list(&tag::BENCH_KINDS)))?;
    let footer = hx(tag::build_footer(&fields))?;
    req(hx(tag::parse_footer(&footer, &[]))? == fields, "round-tripped footer must equal the original fields exactly")?;
    let missing = footer.replace(&format!("anchor: {}\n", "a".repeat(40)), "");
    req(
        code_of(|| tag::parse_footer(&missing, &[])).as_deref() == Some("HINT_EVIDENCE_BINDING_MALFORMED"),
        "a footer missing a required field must be rejected, not accepted",
    )?;
    let retired = footer.replace("bench_ref:", &format!("certificate_sha256: {}\nbench_ref:", "b".repeat(64)));
    req(code_of(|| tag::parse_footer(&retired, &[])).as_deref() == Some("HINT_EVIDENCE_BINDING_MALFORMED"), "a retired digest key must stay rejected (F-6a)")?;
    let mut lie = fields.clone();
    lie.set("bench_ref", "../benchmark/bench_report_fixture.md");
    req(
        code_of(|| tag::build_footer(&lie)).as_deref() == Some("HINT_EVIDENCE_BINDING_MALFORMED"),
        "a bench_report bound as bench_kind=certificate must be refused",
    )?;
    let brief = "픽스처 셀의 요약 문단이다.";
    let msg = hx(tag::annotation(brief, &fields))?;
    req(msg == format!("{brief}\n\n{}\n\n{footer}", tag::ANNOTATION_POINTER), "the annotation must be exactly brief + zip pointer + footer")?;
    req(tag::ANNOTATION_POINTER.contains("00-hint.md"), "the pointer must send the reader to 00-hint.md inside the zip")?;
    let extra = format!("{msg}덧붙임\n");
    req(code_of(|| tag::parse_annotation(&extra)).as_deref() == Some("HINT_ANNOTATION_SHAPE"), "any byte beyond the canonical annotation shape must be rejected")?;

    // The one `git tag -a` call site streams the message through stdin (`-F -`) with --cleanup=verbatim
    // (2026-08-20: git's default cleanup=strip dropped every `## ` line). Judged on the Rust source of tag.rs.
    let tag_calls = tag_a_call_sites(&read(HINT_TAG_RS)?);
    req(tag_calls.len() == 1, format!("exactly one production `git tag -a` call site must create hint tags: {tag_calls:?}"))?;
    let argv = &tag_calls[0];
    req(argv.iter().any(|a| a == "--cleanup=verbatim"), "the tag message must be kept verbatim (2026-08-20 `## ` loss)")?;
    let fpos = argv.iter().position(|a| a == "-F");
    req(
        fpos.map(|p| argv.get(p + 1).map(String::as_str) == Some("-")).unwrap_or(false),
        "the tag message must be streamed via stdin (`-F -`), never written to a tracked file",
    )?;

    let sb = Sandbox::new("hint-c1-")?;
    let tmp = sb.tmp();
    let fx = hx(payload_commit(&tmp, "payload", None, None))?;
    let (repo, source, anchor, guide) = (&fx.repo, &fx.source, &fx.anchor, &fx.guide);
    req(&git_ok(repo, &["rev-parse", "HEAD"])? == source, "the payload commit must not move HEAD")?;
    req(
        hx(core::git_nocheck(repo, &["diff", "--quiet"]))?.returncode == 0 && hx(core::git_nocheck(repo, &["diff", "--cached", "--quiet"]))?.returncode == 0,
        "the payload commit must not touch the main working tree or index",
    )?;
    let tracked: Vec<String> = git_ok(repo, &["ls-files"])?.split_whitespace().map(String::from).collect();
    req(tracked == vec![s("src.txt")], "no payload file may become a tracked source file")?;
    let parents: Vec<String> = git_ok(repo, &["rev-parse", &format!("{anchor}^@")])?.split_whitespace().map(String::from).collect();
    req(parents == vec![guide.clone()], "the payload commit must have exactly one parent: the guide commit")?;
    req(hx(branch::guide_info(repo, guide))?.0.as_deref() == Some(branch::BRANCH_FORMAT), "the parent must be a guide commit carrying the current format marker")?;
    req(hx(branch::hint_tip(repo))?.is_none(), "a publication must never move (or create) refs/heads/hint")?;
    let src_msg = hx(tag::annotation(branch::FX_BRIEF, &hint_footer(source)))?;
    req(
        code_of(|| tag::seal(repo, FIXTURE_TAG, source, &src_msg, FIXTURE_UTC, None)).as_deref() == Some("HINT_ANCHOR_PARENT_NOT_GUIDE"),
        "sealing a hint tag on a source-tree commit must be refused",
    )?;
    let commit_msg = hx(branch::commit_message(&fx.payload, FIXTURE_UTC))?;
    req(
        code_of(|| {
            let old = branch::selftest_guide(repo, None, None, branch::FX_GUIDE_BODY, None)?;
            branch::commit_payload(repo, &fx.payload, &commit_msg, FIXTURE_UTC, Some(&old), None)
        })
        .as_deref()
            == Some("HINT_BRANCH_FORMAT_MISMATCH"),
        "a guide commit without the format marker must be refused as a payload parent",
    )?;
    let mut v1 = hint_footer(anchor);
    if let Some(m) = v1.as_dict_mut() {
        m.remove("bench_ref");
        m.remove("bench_kind");
    }
    v1.set("version", "1");
    v1.set("certificate_ref", "../benchmark/benchmark_fixture.yaml");
    req(
        code_of(|| {
            let m = tag::annotation(branch::FX_BRIEF, &v1)?;
            tag::seal(repo, FIXTURE_TAG, anchor, &m, FIXTURE_UTC, None)
        })
        .as_deref()
            == Some("HINT_EVIDENCE_BINDING_VERSION"),
        "a new seal with a v1 footer must be refused (v1 stays readable for old tags only)",
    )?;
    let notes = vec![(s("notes.txt"), s("x\n"))];
    req(
        code_of(|| payload_commit(&tmp, "outside", Some(&notes), Some(repo))).as_deref() == Some("HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"),
        "a payload file outside the allowlist must be refused",
    )?;
    let hand = hx(tag::annotation("손으로 쓴 다른 요약이다.", &hint_footer(anchor)))?;
    let refused = tag::seal(repo, FIXTURE_TAG, anchor, &hand, FIXTURE_UTC, None).err();
    req(
        refused.as_ref().map(|e| e.code == "HINT_SEAL_REFUSED" && problems_of(e).iter().any(|c| c == "HINT_ANNOTATION_BRIEF_MISMATCH")).unwrap_or(false),
        format!("an annotation brief that is not the 00-hint.md §0.1 extraction must be refused: {refused:?}"),
    )?;
    req(
        hx(branch::hint_tip(repo))?.is_none() && hx(tag::tag_ref_kind(repo, FIXTURE_TAG))?.is_none(),
        "refusals must leave the hint branch and the tag namespace untouched",
    )?;

    let message = hx(tag::annotation(branch::FX_BRIEF, &hint_footer(anchor)))?;
    hx(tag::seal(repo, FIXTURE_TAG, anchor, &message, FIXTURE_UTC, None))?;
    let obj = hx(tag::read_tag(repo, FIXTURE_TAG))?.unwrap_or_else(J::dict);
    req(obj.get_str("target") == Some(anchor.as_str()) && obj.get_str("target_type") == Some("commit"), "the tag must point at the hint-branch payload commit")?;
    req(obj.get_str("body") == Some(message.as_str()), "the tag body must be exactly the annotation bytes (brief + pointer + footer)")?;
    req(hx(tag::verify_local(repo, FIXTURE_TAG, None, None, None))?.is_empty(), "a freshly sealed tag must verify locally")?;
    let arc = hx(core::git_bytes(repo, &["archive", "--format=tar", FIXTURE_TAG], false))?;
    req(arc.is_some(), "git archive of the sealed tag failed")?;
    let names = tar_file_names(&arc.unwrap())?;
    let prov = hx(core::read_json(&fx.payload.join("PROVENANCE.json")))?;
    let mut declared: Vec<String> = prov.get("payload_files").map(|l| l.items().iter().map(J::display).collect()).unwrap_or_default();
    declared.sort();
    req(names == declared, format!("the tag archive must be exactly the payload tree: {} != {}", repr_list(&names), repr_list(&declared)))?;
    req(
        branch::PAYLOAD_ALLOWLIST_TOP.iter().all(|t| names.iter().any(|n| n == t)) && !names.iter().any(|n| n == "src.txt") && !names.iter().any(|n| n.starts_with(".claude/")),
        "the archive must carry the whole kit (00-hint.md first) and no repository source",
    )?;
    Ok(())
}

fn scan_hits(text: &str, terms: &[String], profile: &str) -> HR<Vec<String>> {
    Ok(pii::scan_text(text, Some(terms), profile, &[], false, true, None)?.iter().map(|h| h.to_string()).collect())
}

pub fn predicate_HINT_TAG_ACTIVATION_GATE_C2() -> R {
    let terms = vec![s("forbidden-secret-token")];
    // Single RFC1918 fixture literal (reused by every positive case).
    let genuine_ip = "192.168.1.5"; // pii-scan-fixture: 탐지기 양성 케이스 — 삭제하면 시험이 죽는다
    let hits = |t: &str| hx(scan_hits(t, &terms, "deploy"));
    req(!hits("this text contains forbidden-secret-token here")?.is_empty(), "a literal PII term must be detected")?;
    req(!hits(&format!("{genuine_ip} is a private ip"))?.is_empty(), "an RFC1918 address must be detected")?;
    req(hits("nothing sensitive here at all")?.is_empty(), "clean text must not be flagged")?;
    // The section number is assembled from fragments (self-reference guard of the tracked file).
    let sec = format!("10.{}", "1.2");
    req(hits(&format!("본문 §{sec} 를 참조"))?.is_empty(), "a §-anchored document section number must not be flagged as a private ipv4")?;
    req(hits(&format!("#### {sec} 관측된 부작용"))?.is_empty(), "a heading-anchored section number must not be flagged as a private ipv4")?;
    req(
        hits(&format!("§{sec} 요약\n서브 노드 {genuine_ip} 도달"))?.iter().any(|h| h.starts_with("private-ipv4:")),
        "a genuine private ipv4 following a section anchor must still be flagged",
    )?;
    let deploy: BTreeSet<&str> = pii::PROFILES.iter().find(|(n, _)| *n == "deploy").map(|(_, v)| v.iter().copied().collect()).unwrap_or_default();
    let generic: BTreeSet<&str> = pii::GENERIC_PII_NAMES.iter().copied().collect();
    let four: BTreeSet<&str> = ["private-ipv4", "email", "abs-op-path", "spark-host"].into_iter().collect();
    req(deploy == generic && generic == four, format!("the deployment profile must carry all 4 generic patterns: {deploy:?}"))?;
    let email = ["bob", "example.com"].join("@");
    req(
        hx(scan_hits("Alice <x>", &[s("Alice")], "identity"))?.iter().any(|h| h.starts_with("term:")),
        "the identity scan must still catch a known PII literal",
    )?;
    req(
        !hx(scan_hits(&email, &terms, "identity"))?.iter().any(|h| h.starts_with("email:")) && hits(&email)?.iter().any(|h| h.starts_with("email:")),
        "only the identity profile may skip the email pattern",
    )?;

    let sb = Sandbox::new("hint-c2-")?;
    let tmp = sb.tmp();
    let fx = hx(payload_commit(&tmp, "payload", None, None))?;
    let (repo, anchor) = (&fx.repo, &fx.anchor);
    let raw_commit = hx(core::git_bytes(repo, &["cat-file", "commit", anchor], true))?.unwrap_or_default();
    let ids = hx(branch::parse_ident_headers(&String::from_utf8_lossy(&raw_commit)))?;
    let synth = hx(branch::synthetic_identity())?;
    let ident = |role: &str| ids.get(role).and_then(|r| r.get_str("ident")).map(String::from);
    req(
        ident("author").as_deref() == Some(synth.as_str()) && ident("committer").as_deref() == Some(synth.as_str()),
        "the payload commit author/committer must be the synthetic project identity",
    )?;
    let contains = |hay: &[u8], needle: &str| hay.windows(needle.len().max(1)).any(|w| w == needle.as_bytes());
    req(
        !contains(&raw_commit, OPERATOR_NAME) && !contains(&raw_commit, &operator_email()),
        "the operator identity must never ship inside the payload commit (2026-09-21 K1: 57/57 did)",
    )?;
    let leak = vec![(s("artifacts/triplet/leak.yaml"), format!("note: {}\n", branch::FIXTURE_TERM))];
    req(
        code_of(|| payload_commit(&tmp, "leak", Some(&leak), Some(repo))).as_deref() == Some("HINT_PAYLOAD_PII"),
        "a PII literal in the payload tree must block the payload commit",
    )?;
    req(hx(branch::hint_tip(repo))?.is_none(), "a PII hit must stop BEFORE any ref exists (and a publication never moves refs/heads/hint)")?;
    // file NAMES ship too: a hand-built commit whose tree gains a clean-bodied file with a PII NAME.
    let blob = hx(core::git_full(repo, &["hash-object", "-w", "--stdin"], true, Some("k: v\n"), &[]))?.stdout.trim().to_string();
    let idx = vec![(s("GIT_INDEX_FILE"), tmp.join("c2-index").display().to_string())];
    hx(core::git_full(repo, &["read-tree", anchor], true, None, &idx))?;
    let cache = format!("100644,{blob},artifacts/triplet/{}.yaml", branch::FIXTURE_TERM);
    hx(core::git_full(repo, &["update-index", "--add", "--cacheinfo", &cache], true, None, &idx))?;
    let named_tree = hx(core::git_full(repo, &["write-tree"], true, None, &idx))?.stdout.trim().to_string();
    let id_env = hx(branch::identity_env_pairs(FIXTURE_UTC))?;
    let tag_line = format!("{FIXTURE_TAG}\n");
    let named = hx(core::git_full(repo, &["commit-tree", &named_tree, "-p", &fx.guide, "--no-gpg-sign"], true, Some(&tag_line), &id_env))?.stdout.trim().to_string();
    let terms_repo = hx(pii::require_terms(repo))?;
    let v = hx(branch::commit_violations(repo, &named, FIXTURE_TAG, &terms_repo))?;
    req(
        hx(tag::problem_codes(&v))?.iter().any(|c| c == "HINT_PAYLOAD_PATH_PII"),
        "a PII literal in a payload file NAME must be refused even when every file body is clean",
    )?;
    let tree = git_ok(repo, &["rev-parse", &format!("{anchor}^{{tree}}")])?;
    let op_commit = hx(core::git_full(repo, &["commit-tree", &tree, "-p", &fx.guide, "--no-gpg-sign"], true, Some(&tag_line), &operator_env()))?.stdout.trim().to_string();
    let v = hx(branch::commit_violations(repo, &op_commit, FIXTURE_TAG, &terms_repo))?;
    req(
        hx(tag::problem_codes(&v))?.iter().any(|c| c == "HINT_COMMIT_IDENTITY_NOT_SYNTHETIC"),
        "a payload commit carrying the operator identity must be refused even when its tree is clean",
    )?;
    let leak_msg = hx(tag::annotation(&format!("요약 {} 누출", branch::FIXTURE_TERM), &hint_footer(anchor)))?;
    let refused = tag::seal(repo, FIXTURE_TAG, anchor, &leak_msg, FIXTURE_UTC, None).err();
    req(
        refused.as_ref().map(|e| e.code == "HINT_SEAL_REFUSED" && problems_of(e).iter().any(|c| c == "HINT_TAG_PII")).unwrap_or(false),
        format!("a PII hit in the annotation must refuse the seal: {refused:?}"),
    )?;
    req(hx(tag::tag_ref_kind(repo, FIXTURE_TAG))?.is_none(), "a refused seal must leave no tag behind")?;
    // a tag whose TAGGER is the operator fails local verification, so it can never be pushed.
    let message = hx(tag::annotation(branch::FX_BRIEF, &hint_footer(anchor)))?;
    let epoch = hx(core::parse_utc(FIXTURE_UTC))?.epoch();
    let head = format!("object {anchor}\ntype commit\ntag {FIXTURE_TAG}\ntagger {OPERATOR_NAME} <{}> {epoch} +0000\n\n", operator_email());
    let mk = hx(core::git_bytes_env(repo, &["mktag"], false, &[], Some(format!("{head}{message}").as_bytes())))?;
    req(mk.is_some(), "git mktag of the operator-tagged fixture failed")?;
    let op_tag = String::from_utf8_lossy(&mk.unwrap()).trim().to_string();
    let tref = format!("refs/tags/{FIXTURE_TAG}");
    hx(core::git(repo, &["update-ref", &tref, &op_tag]))?;
    let probs = hx(tag::verify_local(repo, FIXTURE_TAG, None, None, None))?;
    req(
        hx(tag::problem_codes(&probs))?.iter().any(|c| c == "HINT_TAGGER_NOT_SYNTHETIC"),
        "a tag whose tagger is not the synthetic identity must fail local verification",
    )?;
    let bare = tmp.join("origin.git");
    let bare_s = bare.display().to_string();
    hx(core::git(&tmp, &["init", "-q", "--bare", &bare_s]))?;
    req(
        code_of(|| tag::push_tag(repo, &bare_s, FIXTURE_TAG, false, None, None, None)).as_deref() == Some("HINT_PUSH_UNVERIFIED"),
        "an unverified (operator-tagged) tag must never be pushed",
    )?;
    req(git_ok(&bare, &["for-each-ref", "--format=%(refname)"])?.is_empty(), "the refused push must leave the remote empty")?;
    hx(core::git(repo, &["update-ref", "-d", &tref, &op_tag]))?;
    hx(tag::seal(repo, FIXTURE_TAG, anchor, &message, FIXTURE_UTC, None))?;
    let sealed = hx(tag::read_tag(repo, FIXTURE_TAG))?.unwrap_or_else(J::dict);
    req(
        sealed.get("tagger").and_then(|t| t.get_str("ident")) == Some(synth.as_str()),
        "the sealed tag tagger must be the synthetic project identity",
    )?;
    req(!contains(&tag::tag_raw_bytes(&sealed), &operator_email()), "the operator identity must not ship in the tag object")?;
    Ok(())
}

/// `re.sub(r"<!--.*?-->", "", tpl, flags=re.S)`.
fn strip_html_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("<!--") {
        match rest[i + 4..].find("-->") {
            Some(j) => {
                out.push_str(&rest[..i]);
                rest = &rest[i + 4 + j + 3..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// `re.sub(r"<<AGENT:.*?>>", repl, raw)` (no DOTALL).
fn sub_agent(raw: &str, repl: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(i) = rest.find("<<AGENT:") {
        let tail = &rest[i + 8..];
        let nl = tail.find('\n').unwrap_or(tail.len());
        match tail[..nl].find(">>") {
            Some(j) => {
                out.push_str(&rest[..i]);
                out.push_str(repl);
                rest = &tail[j + 2..];
            }
            None => {
                out.push_str(&rest[..i + 8]);
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn predicate_HINT_TAG_ACTIVATION_GATE_C3() -> R {
    let banners = [
        "이 자료는 **지도이지 정답이 아니다.**",
        "네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).",
        "복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).",
    ];
    req(template::REVALIDATION_BANNERS == banners, format!("the C3 banner lines drifted: {}", repr_list(&template::REVALIDATION_BANNERS)))?;
    req(banners.iter().all(|b| template::BANNER_LINES.contains(b)), "the linted banner set must include the C3 banner lines")?;
    let tpl = read(HINT_TEMPLATE_00_REL)?;
    let visible = strip_html_comments(&tpl);
    for line in banners {
        req(visible.contains(line), format!("00-hint template must carry the banner outside every PROMPT block/comment: {}", repr(line)))?;
    }
    let td = TempDir::new("hint-c3-")?;
    let payload = td.path().join("payload");
    mkdirs(&payload)?;
    let root = repo_root();
    let identity = |t: &str| -> HR<String> { Ok(t.to_string()) };
    let lint_codes = |text00: &str| -> R<BTreeSet<String>> {
        write(&payload.join("00-hint.md"), text00)?;
        let hooks = template::TemplateHooks { substitute: Some(&identity), ..Default::default() };
        let found = hx(template::lint_with(
            &root, &payload, &J::dict(), &J::Null, &J::Null, None, &J::dict(), false, false, Some(Some(&[])), None, &hooks,
        ))?;
        Ok(found.iter().filter_map(|f| f.get_str("code").map(String::from)).collect())
    };
    req(!lint_codes(&tpl)?.contains("HINT_BANNER_ABSENT"), "the shipped 00-hint template must satisfy the banner rule")?;
    for line in banners {
        let src_line = splitlines(&tpl).into_iter().find(|l| l.contains(line)).unwrap_or("").to_string();
        req(
            lint_codes(&tpl.replacen(&format!("{src_line}\n"), "", 1))?.contains("HINT_BANNER_ABSENT"),
            format!("removing a banner line must block the payload: {}", repr(line)),
        )?;
        req(
            lint_codes(&tpl.replacen(&src_line, &format!("<!-- {src_line} -->"), 1))?.contains("HINT_BANNER_ABSENT"),
            format!("a banner hidden inside a comment must not count: {}", repr(line)),
        )?;
    }
    for name in template::TEMPLATE_FILES {
        let raw = read(&format!(".claude/skills/hint-publisher/templates/{name}.prompt.md"))?;
        write(&payload.join(format!("{name}.md")), &sub_agent(&raw, "픽스처 산문."))?;
    }
    let docs: Vec<String> = template::TEMPLATE_FILES.iter().map(|n| format!("{n}.md")).collect();
    hx(template::seal_prompts(&payload, &docs))?;
    let sealed = read_path(&payload.join("00-hint.md"))?;
    req(!sealed.contains("<!-- PROMPT"), "seal must replace every PROMPT block")?;
    req(banners.iter().all(|l| sealed.contains(l)), "the banner must survive sealing verbatim")?;
    Ok(())
}

/// A copy of terraforming_node render_sub_env's self-test fixture manifest (the owner keeps it in a private module).
const RENDER_FIXTURE_MANIFEST: &str = "topology: multi
cpu_arch: \"aarch64\"
cuda_version: \"132\"
gpus_per_node: 1
gpu_model: \"TEST-GPU\"
nas_model_path: /srv/test-models
model_source: nas
interconnect:
  type: RoCE v2
  hca_devices: [testhca0, testhca1]
  gid_index: 3
  socket_iface: testif0
  mtu: 9000
  bandwidth_gbps: 200
  platform_preset: test-preset
nodes:
  - role: main
    host: 203.0.113.10
    ssh_user: tester
    work_dir: /srv/tester/ws
  - role: sub
    host: 203.0.113.11
    ssh_user: tester
    work_dir: /srv/tester/ws
";

fn rf<T>(r: terraforming_runtime::render_sub_env::Res<T>) -> R<T> {
    r.map_err(|f| Fail::new("Exception", f.text()))
}

fn pv_str_list(v: &PyValue) -> Vec<String> {
    match v.peel() {
        PyValue::List(items) => items.iter().filter_map(jstr).collect(),
        _ => Vec::new(),
    }
}

/// `campaign_init --emit-slice <node>` of the owner (sweep_stop), in-process over a fixture checkout.
fn emit_slice(root: &Path, camp: &str, node: &str) -> R<String> {
    use sweep_stop::campaign_write::{run_argv, Ctx, Next, Sinks};
    let ctx = Ctx { root: root.to_path_buf() };
    let argv: Vec<String> = vec![s("--emit-slice"), s(node), s("--campaign-id"), s(camp), s("--utc"), s(FIXTURE_UTC)];
    let (mut o, mut e): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
    let next = {
        let mut io = Sinks { out: &mut o, err: &mut e };
        run_argv(&ctx, &argv, &mut io)
    };
    let rc = match next {
        Next::Done(c) => c,
        _ => -1,
    };
    req(rc == 0, format!("campaign_init --emit-slice {node} failed (rc={rc}): {}", tail_chars(&String::from_utf8_lossy(&e), 400)))?;
    Ok(String::from_utf8_lossy(&o).into_owned())
}

/// The approval the hint engine reads for (cell, node) from a campaign instance document (`evidence.approval_for`
/// over a fixture checkout holding the instance and the validator's native launcher). Refusal = no approval.
fn approval_from(doc_text: &str, cell: &str, node: &str) -> R<Option<J>> {
    let td = TempDir::new("hint-c4-appr-")?;
    let repo = td.path();
    let launcher = core::owner_launcher(&repo_root(), core::REL_CAMPAIGN_VALIDATOR);
    let rel_launcher = launcher.strip_prefix(repo_root()).map(Path::to_path_buf).unwrap_or_else(|_| PathBuf::from(core::REL_CAMPAIGN_VALIDATOR));
    write(&repo.join(&rel_launcher), &read_path(&launcher)?)?;
    write(&repo.join("campaigns/camp-fx/campaign.yaml"), doc_text)?;
    Ok(evidence::approval_for(repo, &J::from("camp-fx"), &J::from(cell), &J::from(node)).ok().flatten())
}

pub fn predicate_HINT_TAG_ACTIVATION_GATE_C4() -> R {
    // (1) the hint engine never invokes SSH or the sub delivery/relay tooling. Engine = the launcher + every Rust
    //     source of the hint crate (+ its declared dependencies).
    req(repo_root().join(core::REL_HINT_CLI).is_file(), format!("{} is absent -- the engine to inspect is incomplete", core::REL_HINT_CLI))?;
    let engine = rs_files(HINT_CRATE_SRC);
    req(!engine.is_empty(), format!("{HINT_CRATE_SRC}/*.rs is absent -- the engine to inspect is incomplete"))?;
    let remote_exec = ["ssh", "scp", "sftp", "rsync", "sshpass"];
    let banned_modules = ["paramiko", "fabric", "asyncssh", "relay", "render_sub_env", "node_role_contract", "library_access",
                          "terraforming_runtime", "ssh2", "openssh", "russh", "thrussh"];
    let sub_tooling = ["sync_to_sub.sh", "relay.sh", "relay.py", "render_sub_env", "fetch_sub_docs.sh", "library_access", "node_role_contract", "a2a_delegation"];
    let toml = read(HINT_CRATE_TOML)?;
    for line in splitlines(&toml) {
        let key = strip(line.split('=').next().unwrap_or("")).replace('-', "_");
        req(!banned_modules.contains(&key.as_str()), format!("{HINT_CRATE_TOML} must not depend on a remote-shell/sub-delivery crate ({})", repr(&key)))?;
    }
    for rel in &engine {
        let src = read(rel)?;
        let toks = lex(&src);
        for (j, t) in toks.iter().enumerate() {
            // `use a::b::{c, d};` and `a::b(...)` paths: no remote-shell/sub-delivery module or crate.
            if is_id(t, "use") {
                let mut k = j + 1;
                while k < toks.len() && !is_p(&toks[k], ";") {
                    if toks[k].k == Tk::Ident {
                        req(!banned_modules.contains(&toks[k].s.as_str()), format!("{rel} must not import a remote-shell/sub-delivery module ({})", repr(&toks[k].s)))?;
                    }
                    k += 1;
                }
            }
            if t.k == Tk::Ident && j + 2 < toks.len() && is_p(&toks[j + 1], ":") && is_p(&toks[j + 2], ":") {
                req(!banned_modules.contains(&t.s.as_str()), format!("{rel} must not import a remote-shell/sub-delivery module ({})", repr(&t.s)))?;
            }
            // An argv literal led by a remote-shell program is refused wherever it is built.
            if is_p(t, "[") && j + 1 < toks.len() && toks[j + 1].k == Tk::Str {
                req(!remote_exec.contains(&toks[j + 1].s.as_str()),
                    format!("{rel} must never build a remote-shell argv ({}) -- publication is local", repr(&toks[j + 1].s)))?;
            }
            // A process spawned with a remote-shell program or command string.
            if is_id(t, "Command") && j + 5 < toks.len() && is_p(&toks[j + 1], ":") && is_id(&toks[j + 3], "new") && is_p(&toks[j + 4], "(") && toks[j + 5].k == Tk::Str {
                let first = lstrip(&toks[j + 5].s).to_string();
                req(!remote_exec.contains(&first.as_str()) && !remote_exec.iter().any(|x| first.starts_with(&format!("{x} "))),
                    format!("{rel} must never execute a remote shell command string"))?;
            }
        }
        for literal in toks.iter().filter(|t| t.k == Tk::Str) {
            let head: String = literal.s.chars().take(80).collect();
            req(!sub_tooling.iter().any(|tok| literal.s.contains(tok)), format!("{rel} must never reference sub delivery/A2A tooling ({})", repr(&head)))?;
        }
    }

    // (2) tool plane -- derived from the identity contract, both directions.
    let tp = |topo: &str, mode: &str| -> R<Vec<String>> {
        role_contract::tool_plane(&PyValue::str(topo), &PyValue::str(mode))
            .map(|v| pv_str_list(&v.value))
            .map_err(|_| Fail::new("ValueError", format!("tool_plane({topo}, {mode}) refused")))
    };
    req(tp("single", "a2a-agent")?.iter().any(|x| x == "hint-publisher"), "the single-node (a2a-agent) sub must receive hint-publisher")?;
    req(!tp("multi", "ray-worker")?.iter().any(|x| x == "hint-publisher"), "a multi-node (ray-worker) sub must never receive hint-publisher")?;
    let ssp = |topo: &str| role_contract::sub_self_publishes(&PyValue::str(topo), &PyValue::None).ok().map(|v| matches!(v.value.peel(), PyValue::Bool(true)));
    req(ssp("single") == Some(true) && ssp("multi") == Some(false), "sub_self_publishes must be the tool-plane derivation (single yes, multi no)")?;

    // (3) the real renderer: engine + contract/vocab for single only; catalog and role markers never.
    use terraforming_runtime::render_sub_env as rse;
    let never: Vec<&str> = rse::HINT_NEVER_DELIVER.to_vec();
    for must in ["hints/index.json", "HINTS.md", "hints/.central_authority", "hints/.push_authority"] {
        req(never.contains(&must), format!("render_sub_env.HINT_NEVER_DELIVER must list {}", repr(must)))?;
    }
    req(
        ["hints/HINT_ISSUANCE_CONTRACT.md", "hints/vocab.json"].iter().all(|x| rse::HINT_ENGINE_DEPENDENCIES.contains(x)),
        "the engine dependencies must carry the hints/ contract and vocabulary",
    )?;
    let root = repo_root();
    let ctx = rse::Ctx { here: root.join(".claude/skills/terraforming_node/scripts").display().to_string(), repo: root.display().to_string() };
    let td = TempDir::new("hint-c4-render-")?;
    let mpath = td.path().join("manifest.yaml");
    write(&mpath, RENDER_FIXTURE_MANIFEST)?;
    for (topology, want) in [("single", true), ("multi", false)] {
        let mut data = rf(rse::parse_manifest(&mpath.display().to_string()))?;
        data.set("topology", rse::MVal::Str(topology.into()));
        let (ph, missing) = rf(rse::build_placeholders(&ctx, &data, None))?;
        req(missing.is_empty(), format!("fixture placeholders incomplete for {topology}: {}", repr_list(&missing)))?;
        let out = td.path().join(format!("out_{topology}"));
        rf(rse::render_tree(&ctx, &ph, &out.display().to_string(), true, None, None, None))?;
        let got_engine = out.join(core::REL_HINT_CLI).is_file();
        let got_vocab = out.join("hints/vocab.json").is_file();
        req(got_engine == want && got_vocab == want, format!("{topology}: engine staged={got_engine} vocab staged={got_vocab}, expected {want}"))?;
        let leaked: Vec<&str> = never.iter().copied().filter(|r| out.join(r).exists()).collect();
        req(leaked.is_empty(), format!("{topology}: catalog/role-marker paths must never be staged: {}", repr_list(&leaked)))?;
    }
    let sync_src = read(SYNC_TO_SUB)?;
    req(
        !["HINTS.md", "hints/index.json", ".central_authority", ".push_authority"].iter().any(|t| sync_src.contains(t)),
        "sync_to_sub.sh must never carry the catalog or a role marker of its own -- delivery is the render staging",
    )?;

    // (4) the derived slice carries exactly that node's approval for a self-publishing sub, and nothing else.
    let approval = jd! {"approved_by" => "fixture transcription", "approved_utc" => FIXTURE_UTC, "source" => "declaration-popup"};
    let decl_for = |topology: &str| -> J {
        jd! {"schema_version" => 1i64, "id" => "camp-fx", "plan_ref" => "docs/plan/p.md", "declared_utc" => FIXTURE_UTC,
             "nodes" => vec![jd!{"node_id" => "main", "role" => "main", "topology" => topology, "hw" => "gb10"},
                             jd!{"node_id" => "sub", "role" => "sub", "topology" => topology, "hw" => "gb10"}],
             "assignments" => jd!{"main" => vec![jd!{"cell" => "cell-a"}], "sub" => vec![jd!{"cell" => "cell-s"}]},
             "hint_targets" => vec![jd!{"node_id" => "main", "cells" => vec!["cell-a"], "approval" => approval.clone()},
                                    jd!{"node_id" => "sub", "cells" => vec!["cell-s"], "approval" => approval.clone()}]}
    };
    let mut sliced: BTreeMap<&str, String> = BTreeMap::new();
    for topology in ["single", "multi"] {
        let fixture = TempDir::new("hint-c4-")?;
        let froot = fixture.path();
        // the owner refuses to carry an approval without the role-contract launcher beside it (presence only)
        let contract = ".claude/skills/terraforming_node/scripts/node_role_contract.sh";
        write(&froot.join(contract), &read(contract)?)?;
        write(&froot.join("campaigns/camp-fx/campaign.yaml"), &core::dumps(&decl_for(topology)))?;
        sliced.insert(topology, emit_slice(froot, "camp-fx", "sub")?);
    }
    let single = hx(hintlib::value::loads(&sliced["single"]).map_err(|e| HintError::py("ValueError", e)))?;
    let multi = hx(hintlib::value::loads(&sliced["multi"]).map_err(|e| HintError::py("ValueError", e)))?;
    let own = J::List(vec![jd! {"node_id" => "sub", "cells" => vec!["cell-s"], "approval" => approval.clone()}]);
    req(
        single.get_str("self_role") == Some("sub") && single.get("hint_targets").map(core::dumps) == Some(core::dumps(&own)),
        format!("the single sub slice must carry exactly its own approved entry: {}", single.get("hint_targets").map(J::repr).unwrap_or_default()),
    )?;
    let reasons = |text: &str| -> Vec<String> {
        match sweep_stop::pyjson::loads(text).ok().as_ref().map(|v| v.peel()) {
            Some(sweep_stop::pyvalue::PyValue::Dict(d)) => sweep_stop::campaign_validator::hint_target_reasons(d),
            _ => vec![s("not an object")],
        }
    };
    let single_text = core::dumps(&single);
    req(
        reasons(&single_text).is_empty() && approval_from(&single_text, "cell-s", "sub")?.is_some(),
        "the single sub instance must accept and read its own carried approval",
    )?;
    let mut smuggled = single.clone();
    let mut targets = single.get("hint_targets").map(|t| t.items().to_vec()).unwrap_or_default();
    targets.push(decl_for("single")["hint_targets"].at(0).clone());
    smuggled.set("hint_targets", J::List(targets));
    let smuggled_text = core::dumps(&smuggled);
    req(
        !reasons(&smuggled_text).is_empty() && approval_from(&smuggled_text, "cell-s", "sub")?.is_none(),
        "a sub instance holding another node's (main's) entry must be refused and yield no approval",
    )?;
    req(
        multi.get("hint_targets") == Some(&J::List(vec![])),
        format!("a ray-worker sub slice must carry no hint_targets: {}", multi.get("hint_targets").map(J::repr).unwrap_or_default()),
    )?;

    let mut smuggled_m = multi.clone();
    smuggled_m.set("hint_targets", single.get("hint_targets").cloned().unwrap_or(J::Null));
    let smuggled_m_text = core::dumps(&smuggled_m);
    req(
        !reasons(&smuggled_m_text).is_empty() && approval_from(&smuggled_m_text, "cell-s", "sub")?.is_none(),
        "a multi-node sub instance holding an approval must be refused -- a ray-worker never self-publishes",
    )?;

    // (5) M1-② (plan_26112012 #3 S4, h90-h92): on every sync the main judges the sub's hint-clone push transport AFTER the
    //     delivery, strictly (only exit 0 passes), never calls it on a ray-worker, and ends the sync non-zero only after the
    //     delivery transaction is closed -- an exit inside the transaction would roll the delivery back.
    let judge = extract_bash_function(&sync_src, "check_sub_push_transport")?;
    req(
        judge.lines().any(|l| l.contains("sub_run \"bash ") && l.contains("/check_sub_hint_equip.sh --push-transport\"")),
        "check_sub_push_transport must ask the sub's equip tool (check_sub_hint_equip.sh --push-transport) over sub_run",
    )?;
    let (ray, call) = (judge.find("\"ray-worker\""), judge.find("--push-transport"));
    req(
        matches!((ray, call), (Some(r), Some(c)) if r < c) && judge.contains("push-transport($t): not_applicable"),
        "a ray-worker sub has no equip tool: check_sub_push_transport must record not_applicable before (instead of) calling it",
    )?;
    req(
        judge.contains("if [ \"$rc\" = \"0\" ]; then") && judge.contains("PUSH_TRANSPORT_BLOCKED+=(\"$t\")"),
        "only exit 0 of --push-transport passes; every other result (an old sub executable included) is a block",
    )?;
    req(search("\\bexit\\b", &judge).is_none(), "check_sub_push_transport must not end the sync itself -- the block is recorded and the exit comes after the transaction closes")?;
    let b1 = index(&sync_src, "# ── B1 per-branch 증분 싱크 ──")?;
    let checked = index_from(&sync_src, "verify_checksums \"$t\"", b1)?;
    let judged = index_from(&sync_src, "check_sub_push_transport \"$t\"", b1);
    let loop_end = index_from(&sync_src, "LAST_DELIVERED=\"$t\"", b1)?;
    req(
        judged.as_ref().map_or(false, |&j| checked < j && j < loop_end),
        "the M1-② push-transport judgement must run in each B1 target after verify_checksums (after the delivery)",
    )?;
    let closed = rindex(&sync_src, "\nfinalize_remote_transactions\n")?;
    req(
        count(&sync_src, "exit \"$PUSH_TRANSPORT_EXIT\"") == 1 && index(&sync_src, "exit \"$PUSH_TRANSPORT_EXIT\"")? > closed,
        "the M1-② block must end the sync only after finalize_remote_transactions, so the delivered content stays",
    )?;
    Ok(())
}

pub fn predicate_HINT_TAG_ACTIVATION_GATE_C5() -> R {
    // Static scope = the push path only (str literals of these three functions in the Rust tag.rs).
    let tag_src = read(HINT_TAG_RS)?;
    for f in ["push_tag", "git_push_authenticated", "_require_safe_refspec"] {
        let literals = fn_literals(&tag_src, f)?;
        for widening in ["--tags", "--all", "--mirror", "--follow-tags", "--force", "refs/tags/*", "refs/tags/hint/*"] {
            req(!literals.contains(widening), format!("{f} must never carry the widening argument {}", repr(widening)))?;
        }
    }
    req(tag::PUSH_SCOPE_ARGS.contains(&"--no-follow-tags"), "every push must pin --no-follow-tags (2026-09-22: 1 tag pushed 3)")?;
    req(!fn_text(&tag_src, "verify_local")?.contains("last-good"), "verify must not re-introduce an origin-side last-good-* scan (F-6c)")?;

    let sb = Sandbox::new("hint-c5-")?;
    let tmp = sb.tmp();
    let fx = hx(payload_commit(&tmp, "payload", None, None))?;
    let (repo, anchor) = (&fx.repo, &fx.anchor);
    let message = hx(tag::annotation(branch::FX_BRIEF, &hint_footer(anchor)))?;
    hx(tag::seal(repo, FIXTURE_TAG, anchor, &message, FIXTURE_UTC, None))?;
    let bare = tmp.join("origin.git");
    let bare_s = bare.display().to_string();
    hx(core::git(&tmp, &["init", "-q", "--bare", &bare_s]))?;
    for (bad, want) in [("hint/*", "HINT_TAG_GLOB_FORBIDDEN"), ("hint/a?b", "HINT_TAG_GLOB_FORBIDDEN"), ("refs/heads/hint", "HINT_TAG_NAMESPACE"),
                        ("refs/tags/hint/x", "HINT_TAG_NAMESPACE"), ("last-good-fixture", "HINT_TAG_NAMESPACE")] {
        req(
            code_of(|| tag::push_tag(repo, &bare_s, bad, false, None, None, None)).as_deref() == Some(want),
            format!("push --tag {} must be refused with {want}", repr(bad)),
        )?;
    }
    for spec in ["refs/tags/*:refs/tags/*", "--tags", "+refs/tags/x:refs/tags/x", "refs/tags/a:refs/tags/b"] {
        req(
            code_of(|| tag::git_push_authenticated(repo, &bare_s, spec, true)).as_deref() == Some("HINT_PUSH_REFSPEC_FORBIDDEN"),
            format!("refspec {} must be refused by the push API", repr(spec)),
        )?;
    }
    // Try to widen the push from the outside: push.followTags=true plus a local last-good-* anchor and another
    // annotated hint tag, both reachable from the payload commit.
    hx(core::git(repo, &["config", "push.followTags", "true"]))?;
    hx(core::git(repo, &["tag", "-a", "-m", "local rollback anchor", "last-good-fixture", anchor]))?;
    let unsealed = FIXTURE_TAG.replace("len262144", "len1024");
    hx(core::git(repo, &["tag", "-a", "-m", "unverified", &unsealed, anchor]))?;
    tag::spy_start();
    let result = tag::push_tag(repo, &bare_s, FIXTURE_TAG, false, None, None, None);
    let calls = tag::spy_take();
    let result = hx(result)?;
    let want = format!("refs/tags/{FIXTURE_TAG}:refs/tags/{FIXTURE_TAG}");
    let pushes: Vec<Vec<String>> = calls
        .iter()
        .map(|c| c.get("argv").map(|a| a.items().iter().map(J::display).collect::<Vec<String>>()).unwrap_or_default())
        .filter(|argv| argv.first().map(String::as_str) == Some("git") && argv.iter().any(|x| x == "push"))
        .collect();
    req(
        pushes.len() == 1 && pushes[0].len() >= 2 && pushes[0][pushes[0].len() - 2..] == [bare_s.clone(), want.clone()],
        format!("exactly one push with exactly one tag refspec: {pushes:?}"),
    )?;
    req(
        !pushes[0].iter().any(|a| a == "--tags") && pushes[0].iter().any(|a| a == "--no-follow-tags") && !pushes[0].iter().any(|a| a.contains("refs/heads/")),
        format!("the push argv must not widen: {:?}", pushes[0]),
    )?;
    let remote_refs: Vec<String> = git_ok(&bare, &["for-each-ref", "--format=%(refname)"])?.split_whitespace().map(String::from).collect();
    req(
        remote_refs == vec![format!("refs/tags/{FIXTURE_TAG}")],
        format!("only the one hint tag may reach the remote (no last-good-*, no other tag, no branch): {}", repr_list(&remote_refs)),
    )?;
    req(
        result.get_str("status") == Some("pushed") && result.get_str("refspec") == Some(want.as_str()) && result.get("remote_object") == result.get("local_object"),
        format!("the remote object must be checked against the local tag object: {}", result.repr()),
    )?;
    req(
        code_of(|| tag::push_tag(repo, &bare_s, &unsealed, false, None, None, None)).as_deref() == Some("HINT_PUSH_UNVERIFIED"),
        "an annotated hint tag that fails local verification must never be pushed",
    )?;
    let lightweight = FIXTURE_TAG.replace("len262144", "len2048");
    hx(core::git(repo, &["tag", &lightweight, anchor]))?;
    req(
        code_of(|| tag::push_tag(repo, &bare_s, &lightweight, false, None, None, None)).as_deref() == Some("HINT_TAG_NOT_ANNOTATED"),
        "a lightweight hint tag (no footer, no tagger) must never be pushed",
    )?;
    let after: Vec<String> = git_ok(&bare, &["for-each-ref", "--format=%(refname)"])?.split_whitespace().map(String::from).collect();
    req(after == remote_refs, "refused pushes must leave the remote exactly as it was")?;
    Ok(())
}

/// `continue` of the real CLI dispatch (`hint.sh` main, in-process): the failure code it reports (`None` = success).
fn cli_continue_code(argv: &[String]) -> Option<String> {
    let (rc, _o, e) = hintlib::io::capture(|| cli::main(argv));
    if rc == 0 {
        return None;
    }
    for line in e.lines() {
        if let Some(rest) = line.strip_prefix("[hint] FAIL ") {
            return Some(rest.split(':').next().unwrap_or("").to_string());
        }
    }
    Some(format!("rc={rc}"))
}

pub fn predicate_HINT_TAG_ACTIVATION_GATE_C6() -> R {
    // Only the side-effecting commands are gated; discovery/authoring stay ungated (proposal step).
    let actions: Vec<(String, String)> = cli::HINT_ACTION_FOR_CMD.iter().map(|(c, a)| (s(c), s(a))).collect();
    req(
        actions == vec![(s("continue"), s("hint_finalize")), (s("verify"), s("hint_verify")), (s("push"), s("hint_push"))],
        format!("hint.sh HINT_ACTION_FOR_CMD drifted: {actions:?}"),
    )?;
    let shape = cli::parser_shape();
    let subs: BTreeSet<String> = shape.iter().map(|(n, _)| n.clone()).collect();
    req(
        ["publish", "continue", "lint", "name", "verify", "push", "catalog", "match"].iter().all(|x| subs.contains(*x)),
        format!("hint.sh subcommands drifted: {}", repr_list(&subs.iter().cloned().collect::<Vec<_>>())),
    )?;
    req(
        actions.iter().all(|(_, a)| policy_gate::completion_gate::ALLOWED_ACTIONS.contains(&a.as_str())),
        "every gated hint action must be a completion_gate action (else authorize can never allow it)",
    )?;
    let cont_opts: BTreeSet<String> = shape.iter().find(|(n, _)| n == "continue").map(|(_, o)| o.iter().map(|(x, _)| x.clone()).collect()).unwrap_or_default();
    req(
        cont_opts.contains("--approved-by") && cont_opts.contains("--approved-utc"),
        "continue must take the human Y/N transcription for a publication outside a campaign",
    )?;
    // continue consults the O6 approval before the payload commit / seal / push -- judged over the CLI's own call
    // graph (Rust sources), then EXECUTED below.
    let sources = cli_sources()?;
    let consts: BTreeSet<String> = sources.iter().flat_map(|src| lex(src).into_iter().filter(|t| t.k == Tk::Str).map(|t| t.s)).collect();
    req(consts.contains("HINT_APPROVAL_ABSENT"), "continue must refuse with HINT_APPROVAL_ABSENT")?;
    let order = approval_order_problems(&sources);
    req(
        order.is_empty(),
        format!("hint.sh must consult the recorded Y/N (evidence.approval_for) before any branch/tag/push side effect: {}", repr_list(&order)),
    )?;
    // executed through the real CLI dispatch: `continue` with no recorded approval refuses with HINT_APPROVAL_ABSENT
    // and moves no ref.
    let real_launcher = core::owner_launcher(&repo_root(), core::REL_CAMPAIGN_VALIDATOR);
    let rel_launcher = real_launcher
        .strip_prefix(repo_root())
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(core::REL_CAMPAIGN_VALIDATOR));
    let launcher_text = read_path(&real_launcher)?;
    let approval = jd! {"approved_by" => "사용자 발화 전사", "approved_utc" => FIXTURE_UTC, "source" => "declaration-popup"};
    let decl = jd! {"id" => "camp-fx", "nodes" => vec![jd!{"node_id" => "main", "role" => "main", "topology" => "single", "hw" => "gb10"}],
                    "assignments" => jd!{"main" => vec![jd!{"cell" => "cell-a"}, jd!{"cell" => "cell-b"}]},
                    "hint_targets" => vec![jd!{"node_id" => "main", "cells" => vec!["cell-a"], "approval" => approval.clone()}]};
    let dump = |v: &J| core::dumps(v);
    {
        let sb = Sandbox::new("hint-c6-noappr-")?;
        let tmp = sb.tmp();
        let repo = hx(branch::selftest_repo(&tmp, "repo"))?;
        write(&repo.join(&rel_launcher), &launcher_text)?;
        write(&repo.join("campaigns/camp-fx/campaign.yaml"), &dump(&decl))?;
        let draft = repo.join(core::REL_DRAFTS).join("fx");
        mkdirs(&draft)?;
        let base = jd! {"schema_version" => cli::STATE_SCHEMA, "stage" => "scaffolded", "tag" => FIXTURE_TAG,
                        "topic" => "fx-topic", "topology_label" => "single TP=1",
                        "manifest" => "docs/_evidence/fx-topic.work-manifest.json", "steps" => J::dict()};
        let with = |mode: &str, sel: J| {
            let mut st = base.clone();
            st.set("mode", mode);
            st.set("selectors", sel);
            st
        };
        let probes: Vec<(&str, J, Vec<String>)> = vec![
            ("campaign cell outside the approved list",
             with("campaign", jd! {"campaign" => "camp-fx", "cell" => "cell-b", "node" => "main"}), vec![]),
            ("publication outside a campaign, no transcription",
             with("publication-replay", jd! {"publication" => "fx-topic", "node" => "main"}), vec![]),
            ("publication outside a campaign, transcription without its time",
             with("publication-replay", jd! {"publication" => "fx-topic", "node" => "main"}), vec![s("--approved-by"), s("사용자 발화 전사")]),
        ];
        for (what, state, extra) in probes {
            let sp = draft.join(cli::STATE_NAME);
            write(&sp, &dump(&state))?;
            let refs = || git_ok(&repo, &["for-each-ref", "--format=%(refname) %(objectname)"]);
            let before = (refs()?, std::fs::read(&sp).unwrap_or_default());
            let mut argv = vec![s("--repo"), repo.display().to_string(), s("continue"), s("--draft"), draft.display().to_string(), s("--generated-utc"), s(FIXTURE_UTC)];
            argv.extend(extra);
            let got = cli_continue_code(&argv);
            req(got.as_deref() == Some("HINT_APPROVAL_ABSENT"), format!("continue must refuse without a recorded Y/N ({what}): {got:?}"))?;
            let after = (refs()?, std::fs::read(&sp).unwrap_or_default());
            req(after == before && hx(branch::hint_tip(&repo))?.is_none(), format!("a refused continue must move no ref and write no state ({what})"))?;
        }
    }

    // The per-cell trigger and the Y/N rule live in the owner skill doc.
    let skill = read(HINT_SKILL_REL)?;
    for phrase in ["그 셀의 서빙·측정·문서 완료 후", "**제안(Y/N)**", "무인 자동 태깅 ✗", "hint_targets[].approval"] {
        req(skill.contains(phrase), format!("{HINT_SKILL_REL} must state the activation rule: {}", repr(phrase)))?;
    }
    req(!read(UPSTREAM_SKILL_REL)?.contains("서빙성공+커밋+문서+전파까지 끝난"), "the retired whole-campaign hint trigger must not survive as a competing rule")?;

    // O6 executed: the recorded approval is read through the engine's one reader of its shape.
    {
        let td = TempDir::new("hint-c6-")?;
        let repo = td.path();
        write(&repo.join(&rel_launcher), &launcher_text)?;
        let approval_of = |doc: &J, cell: &str| -> HR<Option<J>> {
            std::fs::create_dir_all(repo.join("campaigns/camp-fx")).ok();
            std::fs::write(repo.join("campaigns/camp-fx/campaign.yaml"), dump(doc)).ok();
            evidence::approval_for(repo, &J::from("camp-fx"), &J::from(cell), &J::from("main"))
        };
        let got = hx(approval_of(&decl, "cell-a"))?.unwrap_or_else(J::dict);
        req(
            got.get_str("approved_by") == approval.get_str("approved_by") && got.get_str("approved_utc") == Some(FIXTURE_UTC)
                && got.get_str("source") == Some("campaign:hint_targets"),
            format!("an approved cell must yield its recorded Y/N: {}", got.repr()),
        )?;
        req(hx(approval_of(&decl, "cell-b"))?.is_none(), "a cell outside the approved list has no approval")?;
        let mut half = decl.clone();
        if let Some(t) = half.get_mut("hint_targets").and_then(J::as_list_mut).and_then(|l| l.get_mut(0)) {
            if let Some(a) = t.get_mut("approval").and_then(J::as_dict_mut) {
                a.remove("approved_utc");
            }
        }
        req(code_of(|| approval_of(&half, "cell-a")).as_deref() == Some("HINT_APPROVAL_INVALID"), "a half approval is not an approval")?;
        let mut blank = decl.clone();
        if let Some(t) = blank.get_mut("hint_targets").and_then(J::as_list_mut).and_then(|l| l.get_mut(0)) {
            t.set("cells", J::List(vec![]));
        }
        req(code_of(|| approval_of(&blank, "cell-a")).as_deref() == Some("HINT_APPROVAL_INVALID"), "an empty cell list is a blanket approval and must be refused")?;
    }

    // A published tag is immutable: same name = collision (locally and on the remote), a revision is a new tag.
    let sb = Sandbox::new("hint-c6-dup-")?;
    let tmp = sb.tmp();
    let fx = hx(payload_commit(&tmp, "payload", None, None))?;
    let (repo, anchor) = (&fx.repo, &fx.anchor);
    let message = hx(tag::annotation(branch::FX_BRIEF, &hint_footer(anchor)))?;
    hx(tag::seal(repo, FIXTURE_TAG, anchor, &message, FIXTURE_UTC, None))?;
    req(hx(tag::name_collision(repo, FIXTURE_TAG, None))?.as_deref() == Some("local"), "an existing local name must be a collision")?;
    let extra = vec![(s("artifacts/triplet/extra.yaml"), s("k: v\n"))];
    let revised = hx(payload_commit(&tmp, "revised", Some(&extra), Some(repo)))?;
    req(
        code_of(|| {
            let m = tag::annotation(branch::FX_BRIEF, &hint_footer(&revised.anchor))?;
            tag::seal(repo, FIXTURE_TAG, &revised.anchor, &m, FIXTURE_UTC, None)
        })
        .as_deref()
            == Some("HINT_NAME_COLLISION"),
        "a revised payload must never be re-stamped under a published name",
    )?;
    req(
        hx(tag::read_tag(repo, FIXTURE_TAG))?.and_then(|t| t.get_str("target").map(String::from)).as_deref() == Some(anchor.as_str()),
        "the published tag must be untouched",
    )?;
    let bare = tmp.join("origin.git");
    let bare_s = bare.display().to_string();
    hx(core::git(&tmp, &["init", "-q", "--bare", &bare_s]))?;
    hx(tag::push_tag(repo, &bare_s, FIXTURE_TAG, false, None, None, None))?;
    let other = hx(branch::selftest_repo(&tmp, "other-pc"))?;
    req(
        hx(tag::name_collision(&other, FIXTURE_TAG, Some(&bare_s)))?.as_deref() == Some("remote"),
        "a name already published on the remote must be a collision for every other publisher",
    )?;
    Ok(())
}

// =============================================================================
// LAST_GOOD_ROLLBACK_ANCHOR
// =============================================================================

pub fn predicate_LAST_GOOD_ROLLBACK_ANCHOR_C3() -> R {
    let tmpl = read("manifest.template.yaml")?;
    req(search("(?m)^origin_url:\\s*\"\"", &tmpl).is_some(), "origin_url must default empty (user-supplied)")?;
    // The push/remote code paths never hardcode a project remote URL (the hint CLI + tag + catalog Rust sources,
    // the hint launcher and sync_branches.sh).
    let mut paths: Vec<String> = vec![s(core::REL_HINT_CLI), s(HINT_TAG_RS), s(HINT_CATALOG_RS)];
    paths.extend(rs_files(HINT_CLI_DIR));
    paths.push(s(".claude/skills/upstream-version-watch/scripts/sync_branches.sh"));
    for rel in &paths {
        req(repo_root().join(rel).is_file(), format!("{rel} is absent -- the remote-handling path cannot be checked"))?;
        let src = read(rel)?;
        req(!src.contains("github.com/") && !src.contains("git@github.com"), format!("{rel} must not hardcode a project remote URL"))?;
    }
    // The library never picks a remote by itself: every push/query entry point takes it as a required argument.
    let tag_src = read(HINT_TAG_RS)?;
    for f in ["push_tag", "git_push_authenticated", "remote_tag_object"] {
        let ty = param_type(&tag_src, f, "remote")?;
        req(ty.as_deref() == Some("&str"), format!("hintlib.tag.{f} must not default the remote -- the operator names it"))?;
    }
    let mod_src = read(HINT_CLI_MOD_RS)?;
    req(fn_text(&mod_src, "build_parser")?.contains(".default(\"origin\")"), "the remote name defaults to the generic git alias, not a URL (SPEC §4 literal)")?;
    let shape = cli::parser_shape();
    for name in ["continue", "push"] {
        let sub = shape.iter().find(|(n, _)| n == name);
        req(sub.is_some(), format!("hint.sh has no {} subcommand", repr(name)))?;
        let remote = sub.unwrap().1.iter().find(|(o, _)| o == "--remote");
        req(
            remote.map(|(_, d)| d.as_deref() == Some("origin")).unwrap_or(false),
            format!("hint.sh {name} --remote must default to the alias \"origin\" (the operator's own remote), not a URL"),
        )?;
    }
    // recovery restores the working tree to that anchor commit -- performed for REAL.
    let td = TempDir::new("lg-c3-")?;
    let tmp = td.path();
    let env = [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")];
    let g = |args: &[&str]| -> R<String> {
        let mut argv = vec!["git", "-C"];
        let t = tmp.display().to_string();
        argv.push(&t);
        argv.extend_from_slice(args);
        let p = run(&argv, None, &env, None, Some(30))?;
        req(p.code == 0, p.stderr.clone())?;
        Ok(p.stdout.trim().to_string())
    };
    g(&["init", "-q"])?;
    write(&tmp.join("f.txt"), "smoke-passed-state")?;
    g(&["add", "f.txt"])?;
    g(&["commit", "-q", "-m", "last-good"])?;
    g(&["tag", "-a", "last-good-recover-test", "-m", "anchor"])?;
    write(&tmp.join("f.txt"), "broken-followup-change")?;
    g(&["add", "f.txt"])?;
    g(&["commit", "-q", "-m", "broken follow-up"])?;
    req(read_path(&tmp.join("f.txt"))? == "broken-followup-change", "predicate requirement failed at original line 1054")?;
    g(&["checkout", "-q", "last-good-recover-test", "--", "f.txt"])?;
    req(read_path(&tmp.join("f.txt"))? == "smoke-passed-state", "recovery must actually restore the working tree content to the anchor commit")?;
    Ok(())
}
