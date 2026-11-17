//! `judge_version_delta.py`: the version-delta impact judge (P1 D0+D1 · P2 D2). Ported: the closed plane enumeration, axis A, axis B (port scope
//! cross + silent-revert probe), the manual-block audit, the verdict synthesis, fixture replay (`--fixture`, `--check-fixture`), the live run over a
//! local clone (`--repo-cache`, git spawned exactly as the original) and `--write-resolved`.
//! Not ported (exit 2, fail loud): the GitHub compare-API / raw.githubusercontent.com fallback (a live run without a usable local clone),
//! `--emit-fixture` (stays in Python); `--self-test` is ported (`judge_delta/selftest.rs`).
//! Exit status: 0 NO_IMPACT · 1 IMPACT · 4 UNDETERMINED · 2 usage / not ported · 3 delta collection failed · 1 for a failed `--check-fixture`.

use crate::gate_cli::{self, Values};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::cliparse::{Kind, Opt, Spec};
use ring::digest::{digest, SHA256};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

mod selftest;
use std::process::Command;

pub const PROGRAM: &str = "judge_version_delta.sh";
const SCHEMA_VERSION: i64 = 1;
const DEFAULT_REPO: &str = "vllm-project/vllm";

const PLANE_RULES: &[(&str, &str)] = &[
    ("pyproject.toml", "BUILD_INPUT"),
    ("setup.py", "BUILD_INPUT"),
    ("use_existing_torch.py", "BUILD_INPUT"),
    ("CMakeLists.txt", "BUILD_INPUT"),
    ("MANIFEST.in", "BUILD_INPUT"),
    ("build_rust.sh", "BUILD_INPUT"),
    ("build_vllm_ppc64le.sh", "BUILD_INPUT"),
    ("rust-toolchain.toml", "BUILD_INPUT"),
    ("cmake/**", "BUILD_INPUT"),
    ("csrc/**", "BUILD_INPUT"),
    ("requirements/**", "BUILD_INPUT"),
    ("rust/**", "BUILD_INPUT"),
    ("vllm/**.cu", "BUILD_INPUT"),
    ("vllm/**.cuh", "BUILD_INPUT"),
    ("vllm/**.cpp", "BUILD_INPUT"),
    ("vllm/**.cc", "BUILD_INPUT"),
    ("vllm/**.h", "BUILD_INPUT"),
    ("vllm/**.hpp", "BUILD_INPUT"),
    ("vllm/**.rs", "BUILD_INPUT"),
    ("vllm/**", "RUNTIME_PY"),
    ("docker/**", "UPSTREAM_OWN_DOCKER"),
    (".dockerignore", "UPSTREAM_OWN_DOCKER"),
    (".buildkite/**", "CI"),
    (".github/**", "CI"),
    (".pre-commit-config.yaml", "CI"),
    (".clang-format", "CI"),
    (".markdownlint.yaml", "CI"),
    (".shellcheckrc", "CI"),
    (".coveragerc", "CI"),
    ("codecov.yml", "CI"),
    (".gitignore", "CI"),
    (".git-blame-ignore-revs", "CI"),
    (".readthedocs.yaml", "CI"),
    ("tests/**", "TESTS"),
    ("docs/**", "DOCS"),
    ("mkdocs.yaml", "DOCS"),
    ("LICENSE", "DOCS"),
    ("DCO", "DOCS"),
    ("*.md", "DOCS"),
    ("benchmarks/**", "TOOLING"),
    ("examples/**", "TOOLING"),
    ("scripts/**", "TOOLING"),
    ("tools/**", "TOOLING"),
    (".claude/**", "REPO_META"),
    (".gemini/**", "REPO_META"),
    (".agents/skills/**", "REPO_META"),
];
const UNKNOWN_PLANE: &str = "UNKNOWN_PLANE";

fn verdict_rank(v: &str) -> i32 {
    match v {
        "NO_IMPACT" => 0,
        "IMPACT" => 1,
        _ => 2,
    }
}

fn verdict_exit(v: &str) -> i32 {
    match v {
        "NO_IMPACT" => 0,
        "IMPACT" => 1,
        _ => 4,
    }
}

// ── glob (`**` crosses `/`, `*` and `?` do not) over code points ────────────────────────────────────
fn glob_match(pat: &[char], text: &[char]) -> bool {
    match pat.first() {
        None => text.is_empty(),
        Some('*') => {
            if pat.get(1) == Some(&'*') {
                (0..=text.len()).any(|k| glob_match(&pat[2..], &text[k..]))
            } else {
                let mut k = 0;
                loop {
                    if glob_match(&pat[1..], &text[k..]) {
                        return true;
                    }
                    if k < text.len() && text[k] != '/' {
                        k += 1;
                    } else {
                        return false;
                    }
                }
            }
        }
        Some('?') => !text.is_empty() && text[0] != '/' && glob_match(&pat[1..], &text[1..]),
        Some(c) => text.first() == Some(c) && glob_match(&pat[1..], &text[1..]),
    }
}

fn classify_plane(path: &str) -> (&'static str, Option<&'static str>) {
    let t: Vec<char> = path.chars().collect();
    for (glob, plane) in PLANE_RULES {
        let p: Vec<char> = glob.chars().collect();
        if glob_match(&p, &t) {
            return (plane, Some(glob));
        }
    }
    (UNKNOWN_PLANE, None)
}

// ── value helpers ───────────────────────────────────────────────────────────────────────────────────
fn pstr(t: &str) -> PyValue {
    PyValue::str(t)
}
fn opt_s(t: &Option<String>) -> PyValue {
    t.as_deref().map(pstr).unwrap_or(PyValue::None)
}
fn int(n: i64) -> PyValue {
    PyValue::int(n)
}
fn list_s(v: &[String]) -> PyValue {
    PyValue::List(v.iter().map(|s| pstr(s)).collect())
}
fn dget<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(key).map(PyValue::peel),
        _ => None,
    }
}
fn sval(v: &PyValue) -> Option<String> {
    match v.peel() {
        PyValue::Str(s) => s.to_utf8(),
        _ => None,
    }
}
/// Python `str(value)` for the JSON scalars the judge formats.
fn display(v: &PyValue) -> String {
    match v.peel() {
        PyValue::None => "None".into(),
        PyValue::Bool(b) => (if *b { "True" } else { "False" }).into(),
        PyValue::Str(s) => s.to_utf8().unwrap_or_default(),
        other => other.repr(),
    }
}
fn dict(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in entries {
        d.set(k, v);
    }
    PyValue::Dict(d)
}
fn sha256_hex(bytes: &[u8]) -> String {
    digest(&SHA256, bytes).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

struct Fail(i32, String);

fn die_text(msg: &str) -> String {
    let mut d = PyDict::new();
    d.set("error", pstr(msg));
    PyValue::Dict(d).dumps_pretty().map(|t| t.to_utf8().unwrap_or_default()).unwrap_or_default()
}

fn norm_status(raw: Option<&str>) -> String {
    let Some(r) = raw.filter(|r| !r.is_empty()) else { return "unknown".into() };
    let first = r.chars().next().unwrap();
    let table = |c: char| match c {
        'M' => Some("modified"),
        'A' => Some("added"),
        'D' => Some("removed"),
        'T' => Some("typechanged"),
        'C' => Some("copied"),
        'R' => Some("renamed"),
        _ => None,
    };
    let rest = &r[first.len_utf8()..];
    if let Some(name) = table(first) {
        if rest.is_empty() || (rest.chars().all(|c| c.is_ascii_digit())) {
            return name.into();
        }
    }
    match r {
        "modified" | "added" | "removed" | "renamed" | "copied" | "unchanged" => r.into(),
        "changed" => "modified".into(),
        _ => r.into(),
    }
}

// ── D0 · delta collection from a local clone ─────────────────────────────────────────────────────────
fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let sep = matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if sep {
            out.push(&text[start..i]);
            let mut next = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(j, '\n')) = chars.peek() {
                    chars.next();
                    next = j + 1;
                }
            }
            start = next;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

struct GitOut {
    rc: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn git_raw(cache: &str, args: &[&str]) -> Result<GitOut, String> {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.quotePath=false", "-C", cache]).args(args);
    let p = cmd.output().map_err(|e| e.to_string())?;
    Ok(GitOut { rc: p.status.code().unwrap_or(-1), stdout: p.stdout, stderr: p.stderr })
}

fn git(cache: &str, args: &[&str]) -> Result<GitOut, String> {
    let p = git_raw(cache, args)?;
    if p.rc != 0 {
        return Err(format!(
            "git 실패(rc={}): {}\n{}",
            p.rc,
            args.join(" "),
            String::from_utf8_lossy(&p.stderr).trim()
        ));
    }
    Ok(p)
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn abspath(p: &str) -> String {
    let base = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let mut out = PathBuf::new();
    for c in base.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().into_owned()
}

struct Delta {
    files: Vec<PyValue>,
    from_sha: Option<String>,
    to_sha: Option<String>,
    delta_source: PyValue,
    delta_source_detail: PyValue,
    ahead_by: PyValue,
    total_commits: PyValue,
}

fn collect_delta_git(cache: &str, from_ref: &str, to_ref: &str) -> Result<Delta, String> {
    let rev = |r: &str| -> Result<String, String> {
        let p = git(cache, &["rev-parse", &format!("{r}^{{commit}}")])?;
        Ok(lossy(&p.stdout).trim().to_string())
    };
    let from_sha = rev(from_ref)?;
    let to_sha = rev(to_ref)?;
    let rng = format!("{from_ref}..{to_ref}");

    let raw = lossy(&git(cache, &["diff", "--name-status", "-z", &rng])?.stdout);
    let toks: Vec<&str> = raw.split('\0').filter(|t| !t.is_empty()).collect();
    let mut status_by_path: BTreeMap<String, String> = BTreeMap::new();
    let mut prev_by_path: BTreeMap<String, String> = BTreeMap::new();
    let mut i = 0;
    while i < toks.len() {
        let st = toks[i];
        if st.starts_with('R') || st.starts_with('C') {
            let (old, new) = (toks.get(i + 1).ok_or("name-status 토큰 부족")?, toks.get(i + 2).ok_or("name-status 토큰 부족")?);
            status_by_path.insert(new.to_string(), st.to_string());
            prev_by_path.insert(new.to_string(), old.to_string());
            i += 3;
        } else {
            let p = toks.get(i + 1).ok_or("name-status 토큰 부족")?;
            status_by_path.insert(p.to_string(), st.to_string());
            i += 2;
        }
    }

    let raw = lossy(&git(cache, &["diff", "--numstat", "-z", &rng])?.stdout);
    let toks: Vec<&str> = raw.split('\0').filter(|t| !t.is_empty()).collect();
    let mut nums: BTreeMap<String, (PyValue, PyValue)> = BTreeMap::new();
    let mut i = 0;
    while i < toks.len() {
        let parts: Vec<&str> = toks[i].split('\t').collect();
        let (add, dele) = (parts[0], parts.get(1).ok_or("numstat 형식 오류")?);
        let path;
        if parts.len() >= 3 && !parts[2].is_empty() {
            path = parts[2].to_string();
            i += 1;
        } else {
            path = toks.get(i + 2).ok_or("numstat 토큰 부족")?.to_string();
            i += 3;
        }
        let conv = |s: &str| -> Result<PyValue, String> {
            if s == "-" {
                Ok(PyValue::None)
            } else {
                s.parse::<i64>().map(int).map_err(|_| format!("invalid literal for int() with base 10: '{s}'"))
            }
        };
        nums.insert(path, (conv(add)?, conv(dele)?));
    }

    let diff_txt = lossy(&git(cache, &["diff", "-U0", "--no-color", &rng])?.stdout);
    let mut hunks: BTreeMap<String, i64> = BTreeMap::new();
    let mut cur: Option<String> = None;
    for ln in splitlines(&diff_txt) {
        if let Some(rest) = ln.strip_prefix("diff --git a/") {
            cur = rest.rfind(" b/").map(|k| rest[k + 3..].to_string());
            if let Some(c) = &cur {
                hunks.entry(c.clone()).or_insert(0);
            }
        } else if ln.starts_with("@@ ") {
            if let Some(c) = &cur {
                *hunks.entry(c.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut paths: Vec<&String> = status_by_path.keys().chain(nums.keys()).collect();
    paths.sort();
    paths.dedup();
    let mut files = Vec::new();
    for path in paths {
        let (add, dele) = nums.get(path).cloned().unwrap_or((PyValue::None, PyValue::None));
        let raw_status = status_by_path.get(path).map(String::as_str);
        let mut rec = PyDict::new();
        rec.set("path", pstr(path));
        rec.set("status", pstr(&norm_status(raw_status)));
        rec.set("status_raw", raw_status.map(pstr).unwrap_or(PyValue::None));
        rec.set("additions", add);
        rec.set("deletions", dele);
        rec.set("hunks", hunks.get(path).map(|h| int(*h)).unwrap_or(PyValue::None));
        if let Some(old) = prev_by_path.get(path) {
            rec.set("previous_path", pstr(old));
        }
        files.push(PyValue::Dict(rec));
    }
    Ok(Delta {
        files,
        from_sha: Some(from_sha),
        to_sha: Some(to_sha),
        delta_source: pstr("git-local"),
        delta_source_detail: pstr(&abspath(cache)),
        ahead_by: PyValue::None,
        total_commits: PyValue::None,
    })
}

// ── upstream blob lookup ─────────────────────────────────────────────────────────────────────────────
struct BlobReader<'a> {
    cache: Option<&'a str>,
    recorded: Option<&'a PyValue>,
    to_sha_hint: Option<String>,
}

type Looked = (&'static str, Option<String>);

impl BlobReader<'_> {
    fn replaying(&self) -> bool {
        self.recorded.is_some_and(PyValue::is_truthy)
    }

    fn sha(&self, reference: &str, path: &str) -> Looked {
        if let Some(rec) = self.recorded.filter(|r| r.is_truthy()) {
            let table = dget(rec, reference);
            return match table.and_then(|t| dget(t, path)) {
                None => ("error", Some(format!("fixture 에 {reference}:{path} 기록 없음"))),
                Some(PyValue::None) => ("absent", None),
                Some(v) => ("ok", Some(display(v))),
            };
        }
        if let Some(cache) = self.cache {
            let spec = format!("{reference}:{path}");
            return match git_raw(cache, &["cat-file", "-e", &spec]) {
                Err(e) => ("error", Some(format!("git-local: {e}"))),
                Ok(p) if p.rc != 0 => ("absent", None),
                Ok(_) => match git(cache, &["show", &spec]) {
                    Ok(p) => ("ok", Some(sha256_hex(&p.stdout))),
                    Err(e) => ("error", Some(format!("git-local: {e}"))),
                },
            };
        }
        ("error", Some("raw: raw.githubusercontent.com 조회는 이식되지 않았다(--repo-cache 필요)".into()))
    }

    fn source_label(&self) -> &'static str {
        if self.replaying() {
            "fixture-recorded"
        } else if self.cache.is_some() {
            "git-local"
        } else {
            "raw.githubusercontent.com"
        }
    }
}

// ── D1 · plane classification + axis A ───────────────────────────────────────────────────────────────
fn classify_files(delta_files: &[PyValue]) -> Result<Vec<PyValue>, Fail> {
    let mut out = Vec::new();
    for rec in delta_files {
        let path = dget(rec, "path").and_then(sval).ok_or_else(|| Fail(1, "KeyError: 'path'".into()))?;
        let (plane, glob) = classify_plane(&path);
        let PyValue::Dict(d) = rec.peel() else { return Err(Fail(1, "delta file record is not an object".into())) };
        let mut r = d.clone();
        r.set("plane", pstr(plane));
        r.set("plane_source", pstr("closed-enum"));
        r.set("plane_rule", glob.map(pstr).unwrap_or(PyValue::None));
        out.push(PyValue::Dict(r));
    }
    Ok(out)
}

fn file_path(f: &PyValue) -> String {
    dget(f, "path").and_then(sval).unwrap_or_default()
}
fn file_plane(f: &PyValue) -> String {
    dget(f, "plane").and_then(sval).unwrap_or_default()
}

fn axis_a(files: &[PyValue]) -> (PyValue, Vec<String>) {
    let unknown: Vec<String> = files.iter().filter(|f| file_plane(f) == UNKNOWN_PLANE).map(file_path).collect();
    let delta: Vec<String> = files.iter().filter(|f| file_plane(f) == "BUILD_INPUT").map(file_path).collect();
    let verdict = if !unknown.is_empty() {
        "UNDETERMINED"
    } else if !delta.is_empty() {
        "IMPACT"
    } else {
        "NO_IMPACT"
    };
    let checked: Vec<String> = PLANE_RULES.iter().filter(|(_, p)| *p == "BUILD_INPUT").map(|(g, _)| g.to_string()).collect();
    let note = if !unknown.is_empty() {
        "열거 밖 경로가 있어 빌드입력 무영향을 주장할 수 없다(계획 E6)."
    } else {
        "상류 delta 중 우리 빌드입력 평면에 드는 파일 없음."
    };
    let v = dict(vec![
        ("verdict", pstr(verdict)),
        ("delta", list_s(&delta)),
        ("checked", list_s(&checked)),
        ("checked_source", pstr("closed-enum(PLANE_RULES BUILD_INPUT)")),
        ("unknown_plane_paths", list_s(&unknown)),
        ("note", pstr(note)),
    ]);
    (v, unknown)
}

// ── D2 · port-scope cross + silent-revert probe ──────────────────────────────────────────────────────
fn str_eq(v: &PyValue, s: &Option<String>) -> bool {
    match (v.peel(), s) {
        (PyValue::Str(a), Some(b)) => a.equals(b),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn axis_b(
    files: &[PyValue],
    provenance: Option<&PyValue>,
    bundle_files_root: Option<&Path>,
    reader: &BlobReader,
    from_ref: &str,
    to_ref: &str,
    scope_origin: Option<&PyValue>,
    scope_path: Option<&PyValue>,
) -> (PyValue, Vec<String>) {
    let Some(provenance) = provenance else {
        return (
            dict(vec![
                ("verdict", pstr("NO_IMPACT")),
                ("applicable", PyValue::Bool(false)),
                ("overlap", PyValue::List(vec![])),
                ("probes", PyValue::List(vec![])),
                ("silent_revert_risk", PyValue::List(vec![])),
                ("note", pstr("이식 번들(PROVENANCE) 미제공 — stock 트랙. 교차할 스코프가 구조적으로 없다.")),
            ]),
            vec![],
        );
    };
    let scope_val = dget(provenance, "files");
    let scope: Option<&PyDict> = match scope_val {
        Some(PyValue::Dict(d)) if !d.is_empty() => Some(d),
        _ => None,
    };
    let Some(scope) = scope else {
        return (
            dict(vec![
                ("verdict", pstr("UNDETERMINED")),
                ("applicable", PyValue::Bool(true)),
                ("overlap", PyValue::List(vec![])),
                ("probes", PyValue::List(vec![])),
                ("silent_revert_risk", PyValue::List(vec![])),
                ("note", pstr("PROVENANCE.files 가 비었거나 dict 가 아님 — 스코프 미해소.")),
            ]),
            vec!["PROVENANCE.files 미해소".into()],
        );
    };
    let psrc = dget(provenance, "source").filter(|v| v.is_truthy());
    let base_tag = psrc.and_then(|s| dget(s, "upstream_base_tag")).cloned().unwrap_or(PyValue::None);
    let base_sha = psrc.and_then(|s| dget(s, "upstream_base_sha")).cloned().unwrap_or(PyValue::None);

    let mut overlap: Vec<String> = files.iter().map(file_path).filter(|p| scope.contains_key(p)).collect();
    overlap.sort();
    overlap.dedup();
    let mut risks: Vec<PyValue> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    let mut verdicts: Vec<String> = Vec::new();
    for path in &overlap {
        let mut entry = PyDict::new();
        entry.set("path", pstr(path));
        let mut bundle_sha: Option<PyValue> = None;
        let mut bsrc = PyValue::None;
        if let Some(root) = bundle_files_root {
            let fp = root.join(path);
            let is_link = std::fs::symlink_metadata(&fp).map(|m| m.file_type().is_symlink()).unwrap_or(false);
            if fp.is_file() && !is_link {
                if let Ok(bytes) = std::fs::read(&fp) {
                    bundle_sha = Some(pstr(&sha256_hex(&bytes)));
                    bsrc = pstr("bundle-file");
                }
            }
        }
        if bundle_sha.is_none() {
            let declared = scope.get(path).cloned().unwrap_or(PyValue::None);
            bundle_sha = if matches!(declared, PyValue::None) { None } else { Some(declared) };
            bsrc = pstr("provenance-declared");
        }
        entry.set("bundle_sha", bundle_sha.clone().unwrap_or(PyValue::None));
        entry.set("bundle_sha_source", bsrc);
        let (st_to, sha_to) = reader.sha(to_ref, path);
        let (st_from, sha_from) = reader.sha(from_ref, path);
        entry.set("upstream_to_sha", opt_s(&sha_to));
        entry.set("upstream_from_sha", opt_s(&sha_from));
        entry.set("upstream_sha_source", pstr(reader.source_label()));
        let none_txt = |s: &Option<String>| s.clone().unwrap_or_else(|| "None".into());
        let (verdict, reason): (&str, String) = if bundle_sha.is_none() {
            ("UNDETERMINED", "번들 사본 sha 미해소(PROVENANCE 선언·실파일 모두 부재)".into())
        } else if st_to == "error" || st_from == "error" {
            (
                "UNDETERMINED",
                format!("상류 조회 실패(to={st_to}:{} from={st_from}:{})", none_txt(&sha_to), none_txt(&sha_from)),
            )
        } else if st_to == "absent" {
            (
                "UNDETERMINED",
                "상류 @to_ref 에 파일 부재 — 번들이 삭제된 파일을 되살린다. 자동 판정 대상 아님(사람 판정 필요)".into(),
            )
        } else if str_eq(bundle_sha.as_ref().unwrap(), &sha_to) {
            ("NO_REVERT", "번들 사본이 상류 @to_ref 와 byte-identical".into())
        } else if st_from == "absent" {
            ("WOULD_REVERT", "상류 @to_ref 신규 파일을 번들이 다른 내용으로 덮는다".into())
        } else if sha_from == sha_to {
            (
                "DIVERGENT_NO_DELTA",
                "상류 내용이 from↔to 동일(rename/mode 변경 등) — 번들이 상류와 다르지만 되돌릴 델타가 없다".into(),
            )
        } else {
            (
                "WOULD_REVERT",
                "번들 사본이 상류 @to_ref 와 다르고 상류가 from→to 사이에 변했다 → 이식본을 그대로 얹으면 그 변경이 조용히 되돌아간다"
                    .into(),
            )
        };
        entry.set("verdict", pstr(verdict));
        entry.set("reason", pstr(&reason));
        entry.set("bundle_equals_upstream_from", PyValue::Bool(bundle_sha.as_ref().is_some_and(|b| str_eq(b, &sha_from))));
        entry.set("bundle_equals_upstream_to", PyValue::Bool(bundle_sha.as_ref().is_some_and(|b| str_eq(b, &sha_to))));
        entry.set("bundle_base_tag", base_tag.clone());
        entry.set("bundle_base_sha", base_sha.clone());
        entry.set("bundle_base_source", pstr("PROVENANCE.source (선언 · 미검증)"));
        let base_is_to_ref = if !(base_tag.is_truthy() || base_sha.is_truthy()) {
            PyValue::None
        } else {
            let to_ref_opt = Some(to_ref.to_string());
            PyValue::Bool(
                str_eq(&base_tag, &to_ref_opt)
                    || (!matches!(base_sha, PyValue::None) && str_eq(&base_sha, &reader.to_sha_hint)),
            )
        };
        let is_true = matches!(base_is_to_ref, PyValue::Bool(true));
        entry.set("bundle_base_is_to_ref", base_is_to_ref);
        if verdict == "WOULD_REVERT" && is_true {
            entry.set(
                "note",
                pstr("번들이 to_ref 에서 파생됐다고 **선언**한다 → 되돌림이 아니라 **파생 변종**일 수 있다. 그러나 선언은 증명이 아니므로 verdict 는 fail-closed 로 유지한다. 확증은 내용 술어(예 plan_26082111 S2-c)와 스모크가 한다."),
            );
        }
        if verdict == "UNDETERMINED" {
            unknown.push(format!("axis_B:{path}: {reason}"));
        }
        verdicts.push(verdict.to_string());
        risks.push(PyValue::Dict(entry));
    }

    let at_risk: Vec<PyValue> = risks
        .iter()
        .zip(&verdicts)
        .filter(|(_, v)| *v == "WOULD_REVERT" || *v == "UNDETERMINED")
        .map(|(r, _)| r.clone())
        .collect();
    let verdict = if verdicts.iter().any(|v| v == "UNDETERMINED") {
        "UNDETERMINED"
    } else if !overlap.is_empty() {
        "IMPACT"
    } else {
        "NO_IMPACT"
    };
    let note = if !overlap.is_empty() {
        "이식 스코프와 델타가 겹친다 → `regen_build_patches_src.sh derive` 재파생 필수. 재파생 후 silent_revert_risk == [] 가 될 때까지 진행 금지(계획 §5.3)."
    } else {
        "이식 스코프와 델타 교차 없음."
    };
    let origin = match scope_origin {
        Some(v) if v.is_truthy() => v.clone(),
        _ => pstr("in-tree path"),
    };
    (
        dict(vec![
            ("verdict", pstr(verdict)),
            ("applicable", PyValue::Bool(true)),
            ("scope_files", int(scope.len() as i64)),
            ("scope_source", pstr("PROVENANCE.files")),
            ("scope_path", scope_path.cloned().unwrap_or(PyValue::None)),
            ("scope_origin", origin),
            ("scope_base_tag", base_tag),
            ("scope_base_sha", base_sha),
            ("overlap", list_s(&overlap)),
            ("probes", PyValue::List(risks)),
            ("silent_revert_risk", PyValue::List(at_risk)),
            ("note", pstr(note)),
        ]),
        unknown,
    )
}

// ── hand-authored block audit ────────────────────────────────────────────────────────────────────────
/// `^\s*\+(\d+)\s*/\s*[-−](\d+)\s*$`
fn parse_delta(text: &str) -> Option<(i64, i64)> {
    let t = text.trim_matches(|c: char| c.is_whitespace());
    let t = t.strip_prefix('+')?;
    let digits = |s: &str| -> (String, usize) {
        let d: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n = d.len();
        (d, n)
    };
    let (a, n) = digits(t);
    if a.is_empty() {
        return None;
    }
    let rest = t[n..].trim_start();
    let rest = rest.strip_prefix('/')?.trim_start();
    let rest = rest.strip_prefix('-').or_else(|| rest.strip_prefix('−'))?;
    let (b, m) = digits(rest);
    if b.is_empty() || !rest[m..].trim().is_empty() {
        return None;
    }
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn int_of(v: &PyValue) -> Option<i64> {
    match v.peel() {
        PyValue::Int(n) => n.to_string().parse().ok(),
        PyValue::Bool(b) => Some(*b as i64),
        PyValue::Float(f) => Some(*f as i64),
        PyValue::Str(s) => s.to_utf8()?.trim().parse().ok(),
        _ => None,
    }
}

fn audit_manual_block(resolved: Option<&PyValue>, files: &[PyValue]) -> Result<Option<PyValue>, Fail> {
    let Some(resolved) = resolved.filter(|r| r.is_truthy()) else { return Ok(None) };
    let PyValue::Dict(rd) = resolved.peel() else { return Ok(None) };
    let Some(key) = rd.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).find(|k| k.starts_with("upstream_delta_") && k != "upstream_delta") else {
        return Ok(None);
    };
    let block = rd.get(&key).filter(|b| b.is_truthy()).cloned().unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    let mut measured: Vec<(String, &PyValue)> = Vec::new();
    for f in files {
        let p = file_path(f);
        if let Some(slot) = measured.iter_mut().find(|(k, _)| *k == p) {
            slot.1 = f;
        } else {
            measured.push((p, f));
        }
    }
    let detail: Vec<PyValue> = match dget(&block, "detail") {
        Some(PyValue::List(l)) => l.clone(),
        _ => vec![],
    };
    let mut disc: Vec<PyValue> = Vec::new();
    for d in &detail {
        let path_v = dget(d, "file").cloned().unwrap_or(PyValue::None);
        let path = sval(&path_v);
        let m = path.as_ref().and_then(|p| measured.iter().find(|(k, _)| k == p)).map(|(_, f)| *f);
        let Some(m) = m else {
            disc.push(dict(vec![
                ("path", path_v),
                ("field", pstr("presence")),
                ("manual", pstr("present")),
                ("measured", pstr("absent")),
                ("note", pstr("손저작에 있으나 실측 델타에 없음")),
            ]));
            continue;
        };
        let add = dget(m, "additions").cloned().unwrap_or(PyValue::None);
        let del = dget(m, "deletions").cloned().unwrap_or(PyValue::None);
        let meas = format!("+{}/-{}", display(&add), display(&del));
        let delta_v = dget(d, "delta").cloned();
        let delta_txt = delta_v.as_ref().map(display).unwrap_or_default();
        match parse_delta(&delta_txt) {
            None => disc.push(dict(vec![
                ("path", path_v),
                ("field", pstr("delta")),
                ("manual", delta_v.unwrap_or(PyValue::None)),
                ("measured", pstr(&meas)),
                ("note", pstr("손저작 delta 문자열 파싱 불가")),
            ])),
            Some((ma, md)) => {
                let same = int_of(&add) == Some(ma) && int_of(&del) == Some(md);
                if !same {
                    disc.push(dict(vec![
                        ("path", path_v),
                        ("field", pstr("delta")),
                        ("manual", pstr(&format!("+{ma}/-{md}"))),
                        ("measured", pstr(&meas)),
                        ("note", pstr("전사 오류 — 사실 행은 스크립트 JSON 을 그대로 테이블화해야 한다(resolve-and-render.md §2 사실/판단 분리)")),
                    ]));
                }
            }
        }
    }
    let manual_files: Vec<Option<String>> = detail.iter().map(|d| dget(d, "file").and_then(sval)).collect();
    for (path, _) in &measured {
        if !manual_files.iter().any(|m| m.as_deref() == Some(path.as_str())) {
            disc.push(dict(vec![
                ("path", pstr(path)),
                ("field", pstr("presence")),
                ("manual", pstr("absent")),
                ("measured", pstr("present")),
                ("note", pstr("실측 델타에 있으나 손저작에 없음")),
            ]));
        }
    }
    if let Some(nf) = dget(&block, "files").filter(|v| !matches!(v, PyValue::None)) {
        let n = int_of(nf).ok_or_else(|| Fail(1, format!("int() 변환 불가: {}", display(nf))))?;
        if n != files.len() as i64 {
            disc.push(dict(vec![
                ("path", pstr("(총계)")),
                ("field", pstr("files")),
                ("manual", nf.clone()),
                ("measured", int(files.len() as i64)),
                ("note", pstr("파일 수 불일치")),
            ]));
        }
    }
    let note = if disc.is_empty() {
        "손저작 블록이 실측과 일치".to_string()
    } else {
        format!("손저작 블록에서 {}건 불일치 검출 — 스크립트 산출로 대체해야 한다", disc.len())
    };
    Ok(Some(dict(vec![
        ("manual_key", pstr(&key)),
        ("manual_source", dget(&block, "_source").cloned().unwrap_or(PyValue::None)),
        ("match", PyValue::Bool(disc.is_empty())),
        ("note", pstr(&note)),
    ]))
    .map(|v| {
        // insertion order of the original: manual_key, manual_source, discrepancies, match, note
        let PyValue::Dict(mut d) = v else { unreachable!() };
        let mut ordered = PyDict::new();
        ordered.set("manual_key", d.get("manual_key").cloned().unwrap());
        ordered.set("manual_source", d.get("manual_source").cloned().unwrap());
        ordered.set("discrepancies", PyValue::List(disc));
        ordered.set("match", d.get("match").cloned().unwrap());
        ordered.set("note", d.get("note").cloned().unwrap());
        d = ordered;
        PyValue::Dict(d)
    }))
}

// ── D5 · verdict synthesis ───────────────────────────────────────────────────────────────────────────
fn synthesize(meta: PyDict, files: Vec<PyValue>, ax_a: PyValue, ax_b: PyValue, unknown: Vec<String>, manual: Option<PyValue>) -> PyValue {
    let va = dget(&ax_a, "verdict").and_then(sval).unwrap_or_default();
    let vb = dget(&ax_b, "verdict").and_then(sval).unwrap_or_default();
    let verdict = if !unknown.is_empty() {
        "UNDETERMINED".to_string()
    } else if verdict_rank(&vb) > verdict_rank(&va) {
        vb
    } else {
        va
    };
    let mut out = meta;
    out.set("files", PyValue::List(files));
    out.set("axis_A_build_input", ax_a);
    out.set("axis_B_port_scope", ax_b);
    out.set(
        "axis_C_model_path",
        dict(vec![
            ("verdict", pstr("NOT_IMPLEMENTED")),
            ("implemented", PyValue::Bool(false)),
            ("excluded_from_global", PyValue::Bool(true)),
            ("note", pstr("P3(D3 MCPS 폐포 + D4 심볼 교차) 미구현 — 별도 승인 대기(plan_26082112 §6 P3, advisory-only 착수 예정). **C축 미구현은 '기능 무영향'을 뜻하지 않는다**; 그 판정은 이 attestation 밖에 있다.")),
        ]),
    );
    out.set("axes_in_global", list_s(&["axis_A_build_input".into(), "axis_B_port_scope".into()]));
    out.set("verdict", pstr(&verdict));
    out.set("unknown", list_s(&unknown));
    out.set(
        "limitations",
        list_s(&[
            "C축(모델 코드경로 도달성) 미구현 — 전역 verdict 에 기능 무영향 근거는 들어있지 않다.".into(),
            "정적 판정은 증명이 아니다(E3 Hyrum · E9 정적 불건전성) — 스모크가 최종 중재자다.".into(),
            "NO_IMPACT 는 '모든 구현된 축이 완전 커버리지로 계산됨' 을 뜻하며 스모크 면제가 아니다.".into(),
        ]),
    );
    if let Some(m) = manual {
        out.set("manual_block_audit", m);
    }
    PyValue::Dict(out)
}

// ── pipeline ─────────────────────────────────────────────────────────────────────────────────────────
struct Inputs<'a> {
    repo: &'a str,
    from_ref: &'a str,
    to_ref: &'a str,
    generated_kst: &'a PyValue,
    repo_cache: Option<&'a str>,
    provenance: Option<&'a PyValue>,
    bundle_files_root: Option<&'a Path>,
    resolved: Option<&'a PyValue>,
    replay: Option<&'a PyValue>,
    scope_origin: Option<&'a PyValue>,
    scope_path: Option<&'a PyValue>,
}

fn run_pipeline(i: &Inputs) -> Result<PyValue, Fail> {
    let delta;
    let reader;
    if let Some(rp) = i.replay {
        let files = match dget(rp, "delta_files") {
            Some(PyValue::List(l)) => l.clone(),
            _ => return Err(Fail(1, "KeyError: 'delta_files'".into())),
        };
        let g = |k: &str| dget(rp, k).cloned().unwrap_or(PyValue::None);
        delta = Delta {
            files,
            from_sha: None,
            to_sha: None,
            delta_source: dget(rp, "delta_source").cloned().unwrap_or_else(|| pstr("unknown")),
            delta_source_detail: g("delta_source_detail"),
            ahead_by: g("ahead_by"),
            total_commits: g("total_commits"),
        };
        reader = BlobReader { cache: None, recorded: dget(rp, "upstream_blob_sha"), to_sha_hint: dget(rp, "to_sha").and_then(sval) };
    } else {
        let Some(cache) = i.repo_cache else {
            return Err(Fail(3, die_text("델타 수집 실패 — --repo-cache 가 없다. GitHub compare API 폴백은 이식되지 않았다(Python judge_version_delta.sh 사용). **추정하지 않는다**(계획 D0/R4).")));
        };
        match collect_delta_git(cache, i.from_ref, i.to_ref) {
            Ok(d) => delta = d,
            Err(e) => {
                gate_cli::write_err(&format!("[judge] WARN git-local 델타 수집 실패 → API 폴백: {e}\n"));
                return Err(Fail(3, die_text(&format!("델타 수집 실패 — 로컬 clone 실패, GitHub compare API 폴백은 이식되지 않았다. **추정하지 않는다**(계획 D0/R4). git-local: {e}"))));
            }
        }
        reader = BlobReader { cache: Some(cache), recorded: None, to_sha_hint: delta.to_sha.clone() };
    }
    let (from_sha_v, to_sha_v) = match i.replay {
        Some(rp) => (dget(rp, "from_sha").cloned().unwrap_or(PyValue::None), dget(rp, "to_sha").cloned().unwrap_or(PyValue::None)),
        None => (opt_s(&delta.from_sha), opt_s(&delta.to_sha)),
    };

    let files = classify_files(&delta.files)?;
    let (ax_a, unknown_a) = axis_a(&files);
    let (ax_b, unk_b) = axis_b(&files, i.provenance, i.bundle_files_root, &reader, i.from_ref, i.to_ref, i.scope_origin, i.scope_path);
    let mut unknown: Vec<String> = unknown_a.iter().map(|p| format!("UNKNOWN_PLANE: {p}")).collect();
    unknown.extend(unk_b);
    let src_is_git = matches!(&delta.delta_source, PyValue::Str(s) if s.equals("git-local"));
    if !src_is_git && files.len() >= 300 {
        unknown.push("INCOMPLETE_DELTA: 300 or more files from GitHub compare or an unverified source cannot establish completeness — use --repo-cache with a complete SHA-bound git diff".into());
    }

    let mut meta = PyDict::new();
    meta.set("schema_version", int(SCHEMA_VERSION));
    meta.set("from_ref", pstr(i.from_ref));
    meta.set("to_ref", pstr(i.to_ref));
    meta.set("from_sha", from_sha_v);
    meta.set("to_sha", to_sha_v);
    meta.set("provenance", pstr("measured"));
    meta.set("delta_source", delta.delta_source.clone());
    meta.set("delta_source_detail", delta.delta_source_detail.clone());
    meta.set("repo", pstr(i.repo));
    meta.set("generated_kst", i.generated_kst.clone());
    meta.set("judge", pstr("judge_version_delta.sh (P1:D0+D1 · P2:D2)"));
    if !matches!(delta.ahead_by, PyValue::None) {
        meta.set("ahead_by", delta.ahead_by.clone());
    }
    if !matches!(delta.total_commits, PyValue::None) {
        meta.set("total_commits", delta.total_commits.clone());
    }
    let manual = audit_manual_block(i.resolved, &files)?;
    Ok(synthesize(meta, files, ax_a, ax_b, unknown, manual))
}

// ── fixture ──────────────────────────────────────────────────────────────────────────────────────────
fn json_short(v: &PyValue) -> String {
    let t = v.dumps_unicode().ok().and_then(|t| t.to_utf8()).unwrap_or_default();
    t.chars().take(120).collect()
}

fn is_num(v: &PyValue) -> bool {
    matches!(v.peel(), PyValue::Int(_) | PyValue::Float(_) | PyValue::Bool(_))
}

fn type_name(v: &PyValue) -> &'static str {
    v.peel().type_name()
}

fn deep_diff(a: &PyValue, b: &PyValue, path: &str) -> Vec<String> {
    let (a, b) = (a.peel(), b.peel());
    let same_type = std::mem::discriminant(a) == std::mem::discriminant(b);
    if !same_type && !(is_num(a) && is_num(b)) {
        return vec![format!("{path}: 타입 불일치 {} vs {}", type_name(a), type_name(b))];
    }
    let mut out = Vec::new();
    match (a, b) {
        (PyValue::Dict(da), PyValue::Dict(db)) => {
            let mut keys: Vec<PyStr> = da.iter().map(|(k, _)| k.clone()).collect();
            for (k, _) in db.iter() {
                if !keys.iter().any(|x| x == k) {
                    keys.push(k.clone());
                }
            }
            keys.sort_by(|x, y| x.code_points().cmp(y.code_points()));
            for k in keys {
                let name = k.to_utf8().unwrap_or_default();
                match (da.get(&name), db.get(&name)) {
                    (None, Some(vb)) => out.push(format!("{path}.{name}: expected 에만 있음 → {}", json_short(vb))),
                    (Some(va), None) => out.push(format!("{path}.{name}: actual 에만 있음 → {}", json_short(va))),
                    (Some(va), Some(vb)) => out.extend(deep_diff(va, vb, &format!("{path}.{name}"))),
                    (None, None) => {}
                }
            }
        }
        (PyValue::List(la), PyValue::List(lb)) => {
            if la.len() != lb.len() {
                out.push(format!("{path}: 길이 {} vs {}", la.len(), lb.len()));
            }
            for k in 0..la.len().min(lb.len()) {
                out.extend(deep_diff(&la[k], &lb[k], &format!("{path}[{k}]")));
            }
        }
        _ => {
            if a != b {
                out.push(format!("{path}: {} vs {}", json_short(a), json_short(b)));
            }
        }
    }
    out
}

fn strip_volatile(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut o = PyDict::new();
            for (k, val) in d.iter() {
                if !k.equals("replay_of") {
                    o.insert(k.clone(), val.clone());
                }
            }
            PyValue::Dict(o)
        }
        other => other.clone(),
    }
}

fn replay_from_fixture(fx: &PyValue) -> Result<PyValue, Fail> {
    let inp = dget(fx, "inputs").ok_or_else(|| Fail(1, "KeyError: 'inputs'".into()))?;
    let need = |k: &str| dget(inp, k).ok_or_else(|| Fail(1, format!("KeyError: '{k}'")));
    let repo = dget(inp, "repo").and_then(sval).unwrap_or_else(|| DEFAULT_REPO.into());
    let from_ref = sval(need("from_ref")?).unwrap_or_default();
    let to_ref = sval(need("to_ref")?).unwrap_or_default();
    let generated = need("generated_kst")?;
    let provenance = dget(inp, "provenance").filter(|v| !matches!(v, PyValue::None));
    let resolved = dget(inp, "resolved_manual_block");
    let scope_origin = dget(inp, "scope_origin");
    let scope_path = dget(inp, "scope_path");
    let mut out = run_pipeline(&Inputs {
        repo: &repo,
        from_ref: &from_ref,
        to_ref: &to_ref,
        generated_kst: generated,
        repo_cache: None,
        provenance,
        bundle_files_root: None,
        resolved,
        replay: Some(inp),
        scope_origin,
        scope_path,
    })?;
    if let PyValue::Dict(d) = &mut out {
        d.set(
            "replay_of",
            dict(vec![
                ("fixture", dget(fx, "case").cloned().unwrap_or(PyValue::None)),
                ("recorded_kst", dget(fx, "recorded_kst").cloned().unwrap_or(PyValue::None)),
                ("note", pstr("이 실행은 fixture 기록 사실을 재생한 것이다(재측정 아님).")),
            ]),
        );
    }
    Ok(out)
}

// ── command line ─────────────────────────────────────────────────────────────────────────────────────
fn opt(flags: &'static [&'static str], dest: &'static str, kind: Kind, help: &'static str) -> Opt {
    Opt { flags, dest, kind, help: Some(help), required: false, group: None }
}

pub fn spec() -> Spec {
    Spec {
        prog: PROGRAM,
        description: "judge_version_delta.sh — 버전 델타 영향판정 Judge (P1:D0+D1 · P2:D2)",
        options: vec![
            Opt { flags: &["-h", "--help"], dest: "help", kind: Kind::Help, help: None, required: false, group: None },
            opt(&["--repo"], "repo", Kind::Text, "상류 리포 owner/name"),
            opt(&["--from-ref"], "from_ref", Kind::Text, "기준 ref (예 v0.27.0)"),
            opt(&["--to-ref"], "to_ref", Kind::Text, "대상 ref (예 v0.27.1)"),
            opt(&["--generated-kst"], "generated_kst", Kind::Text, "발행 시각 YYMMDDHH (주입 필수 — 벽시계 금지)"),
            opt(&["--repo-cache"], "repo_cache", Kind::Text, "로컬 vLLM clone (1차 권위). 없으면 compare API 폴백"),
            opt(&["--provenance"], "provenance", Kind::Text, "이식 번들 PROVENANCE.json (이식 트랙일 때만)"),
            opt(&["--bundle-root"], "bundle_root", Kind::Text, "build_patches_src 루트(기본=--provenance 의 디렉터리)"),
            opt(&["--provenance-origin"], "provenance_origin", Kind::Text, "PROVENANCE 의 출처 라벨(예 'git:<sha>:<path>') — in-tree 경로가 아닐 때 기재"),
            opt(&["--no-bundle-files"], "no_bundle_files", Kind::Flag, "번들 실파일을 읽지 않고 PROVENANCE 선언 sha 만 쓴다"),
            opt(&["--resolved"], "resolved", Kind::Text, "resolved.json — 손저작 블록 감사/대체 대상"),
            opt(&["--write-resolved"], "write_resolved", Kind::Flag, "resolved.json 에 upstream_delta 를 써넣는다(손저작 블록은 _superseded_ 로 보존)"),
            opt(&["--emit-fixture"], "emit_fixture", Kind::Text, "이번 실행을 골든 fixture 로 기록"),
            opt(&["--fixture"], "fixture", Kind::Text, "fixture 재생(판정만 출력)"),
            opt(&["--check-fixture"], "check_fixture", Kind::Text, "fixture 재생 후 expected 와 대조 (0=PASS)"),
            opt(&["--self-test"], "self_test", Kind::Flag, ""),
        ],
        group_required: false,
        positional: None,
    }
}

fn load_json(path: &str) -> Result<PyValue, Fail> {
    let bytes = std::fs::read(path).map_err(|e| Fail(1, format!("FileNotFoundError: {e}: '{path}'")))?;
    let text = agent_runtime_support::pyjson::decode_file_text(&bytes).map_err(|e| Fail(1, e.to_string()))?;
    agent_runtime_support::pyjson::loads_text(&text).map_err(|e| Fail(1, e.to_string()))
}

fn print_json(v: &PyValue) {
    let t = v.dumps_pretty().map(|t| t.to_utf8().unwrap_or_default()).unwrap_or_default();
    gate_cli::write_out(&format!("{t}\n"));
}

pub fn run(script: Option<&PyStr>, args: &[PyStr]) -> i32 {
    let values: Values = match gate_cli::parse_args(&spec(), args) {
        Ok(v) => v,
        Err(status) => return status,
    };
    match run_inner(script, &values) {
        Ok(code) => code,
        Err(Fail(code, text)) => {
            gate_cli::write_err(&format!("{}\n", text.trim_end_matches('\n')));
            code
        }
    }
}

fn run_inner(script: Option<&PyStr>, values: &Values) -> Result<i32, Fail> {
    let not_ported = |what: &str| {
        Err(Fail(2, format!("[judge] {what} 는 Rust 로 이식되지 않았다 — judge_version_delta.sh 를 사용한다")))
    };
    if gate_cli::flag(values, "self_test") {
        return Ok(selftest::run_selftest());
    }
    let fixture = gate_cli::text(values, "fixture");
    let check_fixture = gate_cli::text(values, "check_fixture");
    if fixture.is_some() || check_fixture.is_some() {
        let path = fixture.or_else(|| check_fixture.clone()).unwrap();
        let fx = load_json(&path)?;
        let out = replay_from_fixture(&fx)?;
        if check_fixture.is_some() {
            let expected = dget(&fx, "expected").ok_or_else(|| Fail(1, "KeyError: 'expected'".into()))?;
            let diffs = deep_diff(&strip_volatile(&out), &strip_volatile(expected), "$");
            let report = dict(vec![
                ("fixture", pstr(&path)),
                ("case", dget(&fx, "case").cloned().unwrap_or(PyValue::None)),
                ("result", pstr(if diffs.is_empty() { "PASS" } else { "FAIL" })),
                ("diff_count", int(diffs.len() as i64)),
                ("diffs", PyValue::List(diffs.iter().take(60).map(|d| pstr(d)).collect())),
            ]);
            print_json(&report);
            return Ok(if diffs.is_empty() { 0 } else { 1 });
        }
        print_json(&out);
        return Ok(verdict_exit(&dget(&out, "verdict").and_then(sval).unwrap_or_default()));
    }

    for (key, flag) in [("from_ref", "--from-ref"), ("to_ref", "--to-ref"), ("generated_kst", "--generated-kst")] {
        if gate_cli::text(values, key).is_none() {
            return Err(Fail(2, die_text(&format!("{flag} 필요 (벽시계·추정 금지)"))));
        }
    }
    if gate_cli::text(values, "emit_fixture").is_some() {
        return not_ported("--emit-fixture");
    }
    let repo = gate_cli::text(values, "repo").unwrap_or_else(|| DEFAULT_REPO.into());
    let from_ref = gate_cli::text(values, "from_ref").unwrap();
    let to_ref = gate_cli::text(values, "to_ref").unwrap();
    let generated = pstr(&gate_cli::text(values, "generated_kst").unwrap());
    let repo_cache = gate_cli::text(values, "repo_cache");
    let provenance_arg = gate_cli::text(values, "provenance");

    let mut provenance: Option<PyValue> = None;
    let mut bundle_root: Option<PathBuf> = None;
    if let Some(p) = &provenance_arg {
        provenance = Some(load_json(p)?);
        if !gate_cli::flag(values, "no_bundle_files") {
            let root = match gate_cli::text(values, "bundle_root") {
                Some(r) => PathBuf::from(r),
                None => PathBuf::from(abspath(p)).parent().map(Path::to_path_buf).unwrap_or_default(),
            };
            let cand = root.join("files");
            if cand.is_dir() {
                bundle_root = Some(cand);
            }
        }
    }
    let resolved_path = gate_cli::text(values, "resolved");
    let resolved_obj = match &resolved_path {
        Some(p) => Some(load_json(p)?),
        None => None,
    };
    // `_portable_path`: relative to the fourth parent of the script file (the `.claude` directory), else None.
    let scope_path: PyValue = match &provenance_arg {
        Some(p) => {
            let scripts = gate_cli::scripts_dir(script, "judge_version_delta");
            let base = scripts.ancestors().nth(3).map(Path::to_path_buf).unwrap_or(scripts);
            match PathBuf::from(abspath(p)).strip_prefix(&base) {
                Ok(rel) => pstr(&rel.to_string_lossy()),
                Err(_) => PyValue::None,
            }
        }
        None => PyValue::None,
    };
    let origin = gate_cli::text(values, "provenance_origin").map(|o| pstr(&o));

    if gate_cli::flag(values, "write_resolved") && resolved_path.is_none() {
        return Err(Fail(2, die_text("--write-resolved 는 --resolved 가 필요하다")));
    }
    let out = run_pipeline(&Inputs {
        repo: &repo,
        from_ref: &from_ref,
        to_ref: &to_ref,
        generated_kst: &generated,
        repo_cache: repo_cache.as_deref(),
        provenance: provenance.as_ref(),
        bundle_files_root: bundle_root.as_deref(),
        resolved: resolved_obj.as_ref(),
        replay: None,
        scope_origin: origin.as_ref(),
        scope_path: Some(&scope_path),
    })?;

    if gate_cli::flag(values, "write_resolved") {
        let path = resolved_path.unwrap();
        let PyValue::Dict(rd) = resolved_obj.as_ref().unwrap().peel() else {
            return Err(Fail(1, "resolved.json is not an object".into()));
        };
        let moved: Vec<String> = rd
            .iter()
            .filter_map(|(k, _)| k.to_utf8())
            .filter(|k| k.starts_with("upstream_delta_") && k != "upstream_delta")
            .collect();
        let mut nd = PyDict::new();
        for (k, v) in rd.iter() {
            if !moved.iter().any(|m| k.equals(m)) {
                nd.insert(k.clone(), v.clone());
            }
        }
        for m in &moved {
            nd.set(&format!("_superseded_manual_{m}"), rd.get(m).cloned().unwrap_or(PyValue::None));
        }
        nd.set("upstream_delta", out.clone());
        let text = PyValue::Dict(nd).dumps_pretty().map_err(|e| Fail(1, e.to_string()))?;
        let mut bytes = text.to_utf8().unwrap_or_default().into_bytes();
        bytes.push(b'\n');
        std::fs::write(&path, bytes).map_err(|e| Fail(1, format!("OSError: {e}: '{path}'")))?;
        gate_cli::write_err(&format!("[judge] resolved.json 갱신: {path}#upstream_delta\n"));
    }
    print_json(&out);
    Ok(verdict_exit(&dget(&out, "verdict").and_then(sval).unwrap_or_default()))
}
