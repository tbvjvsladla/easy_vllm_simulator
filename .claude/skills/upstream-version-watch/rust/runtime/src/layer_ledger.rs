//! `layer_ledger.py` (non-self-test part): the branch-layer classification ledger and the common-layer sync point / divergence judge.
//!
//! Subcommands of the original: `common-layer`, `sync-point`, `divergence`, `validate`, `append`, `merge` (same arguments, same JSON on
//! stdout in the same key order, exit codes 0/2/5). Four new subcommands replace the inline Python of `closing_sequence.sh`:
//! `emit-skeleton` (the classification blank), `entry-id`, `evidence-rels`, `utc-to-kst`.
//! Port boundary: `--self-test` is not ported (exit 2; run `layer_ledger.py --self-test`); argparse's own usage/error text is approximated.

use agent_runtime_support::pyjson::{self, LoadError};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use fancy_regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const LEDGER_REL: &str = ".claude/policies/branch_layer_ledger.json";
const SYNC_SCRIPT_REL: &str = ".claude/skills/upstream-version-watch/scripts/sync_branches.sh";
const VERDICTS_REPR: &str = "('common_promote', 'branch_only')";
const FILL_MARK: &str = "<FILL";
const MAX_LANDING_SCAN: usize = 400;
const EMPTY_TREE_FALLBACK: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// A Python exception that the original's `main` reports (or lets escape).
struct Exc {
    kind: String,
    msg: String,
}

impl Exc {
    fn new(kind: &str, msg: impl Into<String>) -> Self {
        Exc { kind: kind.to_string(), msg: msg.into() }
    }
}

fn os_exc(err: &std::io::Error, path: &str) -> Exc {
    let text = err.to_string();
    let desc = match text.rfind(" (os error") {
        Some(at) => text[..at].to_string(),
        None => text,
    };
    let code = err.raw_os_error().unwrap_or(0);
    let kind = match err.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ if code == 21 => "IsADirectoryError",
        _ => "OSError",
    };
    Exc::new(kind, format!("[Errno {code}] {desc}: '{path}'"))
}

fn load_exc(err: &LoadError) -> Exc {
    let kind = match err {
        LoadError::Decode { .. } => "JSONDecodeError",
        LoadError::IntegerTooLong { .. } => "ValueError",
        LoadError::Encoding(_) => "UnicodeDecodeError",
        LoadError::TooDeep { .. } => "RecursionError",
    };
    Exc::new(kind, err.to_string())
}

// ---------------------------------------------------------------- value helpers

fn s(t: &str) -> PyValue {
    PyValue::str(t)
}

fn n(i: usize) -> PyValue {
    PyValue::int(i as i64)
}

fn obj(items: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in items {
        d.set(k, v);
    }
    PyValue::Dict(d)
}

fn str_list(items: &[String]) -> PyValue {
    PyValue::List(items.iter().map(|x| s(x)).collect())
}

fn dget<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(key),
        _ => None,
    }
}

fn dstr(v: &PyValue, key: &str) -> Option<String> {
    match dget(v, key).map(|x| x.peel()) {
        Some(PyValue::Str(t)) => t.to_utf8(),
        _ => None,
    }
}

fn truthy(v: Option<&PyValue>) -> bool {
    v.map(|x| x.is_truthy()).unwrap_or(false)
}

fn text_of(v: &PyValue) -> String {
    v.display_text().to_utf8().unwrap_or_default()
}

/// Python `==` on JSON values: dicts compare without regard to key order.
fn jeq(a: &PyValue, b: &PyValue) -> bool {
    match (a.peel(), b.peel()) {
        (PyValue::Dict(x), PyValue::Dict(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| match k.to_utf8().and_then(|key| y.get(&key)) {
                    Some(w) => jeq(v, w),
                    None => false,
                })
        }
        (PyValue::List(x), PyValue::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| jeq(p, q)),
        (PyValue::Int(x), PyValue::Bool(y)) | (PyValue::Bool(y), PyValue::Int(x)) => x == &num_bigint::BigInt::from(*y as i64),
        (PyValue::Int(x), PyValue::Float(y)) | (PyValue::Float(y), PyValue::Int(x)) => {
            num_traits::ToPrimitive::to_f64(x).map(|f| f == *y).unwrap_or(false)
        }
        (x, y) => x == y,
    }
}

fn dumps(v: &PyValue) -> String {
    match v.dumps_pretty() {
        Ok(t) => t.to_utf8().unwrap_or_default(),
        Err(e) => format!("{e:?}"),
    }
}

fn sort_key(e: &PyValue) -> (String, String) {
    let part = |k: &str| match dget(e, k) {
        Some(v) if v.is_truthy() => text_of(v),
        _ => String::new(),
    };
    (part("utc"), part("entry_id"))
}

fn is_zero_sha(t: &str) -> bool {
    !t.is_empty() && t.bytes().all(|b| b == b'0')
}

fn splitlines(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\n' => out.push(std::mem::take(&mut cur)),
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn py_split_ws(t: &str) -> Vec<&str> {
    t.split_whitespace().collect()
}

fn short(t: &str, n: usize) -> String {
    t.chars().take(n).collect()
}

// ---------------------------------------------------------------- git

struct Out {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Out {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    fn err_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).trim().to_string()
    }
}

fn git<S: AsRef<str>>(repo: &Path, args: &[S]) -> Out {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo);
    for a in args {
        cmd.arg(a.as_ref());
    }
    match cmd.output() {
        Ok(o) => Out { code: o.status.code().unwrap_or(-1), stdout: o.stdout, stderr: o.stderr },
        Err(e) => Out { code: 127, stdout: Vec::new(), stderr: e.to_string().into_bytes() },
    }
}

fn sv(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| x.to_string()).collect()
}

fn with_spec(head: &[&str], mid: &[&str], spec: &[String]) -> Vec<String> {
    let mut v = sv(head);
    v.extend(mid.iter().map(|x| x.to_string()));
    v.push("--".to_string());
    v.extend(spec.iter().cloned());
    v
}

// ---------------------------------------------------------------- sync script declarations

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static pattern")
}

fn shell_array(src: &str, name: &str) -> Vec<String> {
    let pattern = format!(r"(?sm)^{}=\((.*?)^\)", fancy_regex::escape(name));
    let Ok(Some(c)) = re(&pattern).captures(src) else { return Vec::new() };
    let mut out = Vec::new();
    for line in splitlines(c.get(1).map(|m| m.as_str()).unwrap_or("")) {
        let line = line.split('#').next().unwrap_or("").trim().to_string();
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

fn exclude_pathspecs(src: &str) -> Vec<String> {
    let any = matches!(re(r"(?s)PATHS\+=\((.*?)\)").find(src), Ok(Some(_)));
    if !any {
        return Vec::new();
    }
    let toks = re(r"':\(exclude\)[^']*'");
    toks.find_iter(src).filter_map(|m| m.ok()).map(|m| m.as_str().to_string()).collect()
}

fn docs_case_arms(src: &str) -> Vec<(Vec<String>, String)> {
    let arm = re(r"^\s*((?:docs/[^\s|)]+)(?:\|docs/[^\s|)]+)*)\)(.*)$");
    let lines = splitlines(src);
    let mut arms = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Ok(Some(c)) = arm.captures(&lines[i]) else {
            i += 1;
            continue;
        };
        let mut body = vec![c.get(2).map(|m| m.as_str()).unwrap_or("").to_string()];
        let mut j = i;
        while !body.last().unwrap().contains(";;") && j + 1 < lines.len() {
            j += 1;
            body.push(lines[j].clone());
        }
        let pats = c.get(1).map(|m| m.as_str()).unwrap_or("").split('|').map(|x| x.to_string()).collect();
        arms.push((pats, body.join("\n")));
        i = j + 1;
    }
    arms
}

struct CommonLayer {
    include: Vec<String>,
    include_docs: Vec<String>,
    union_docs: Vec<String>,
    copy_only_docs: Vec<String>,
    exclude: Vec<String>,
    pathspec: Vec<String>,
    reach_pathspec: Vec<String>,
}

fn common_layer_data(repo: &Path) -> Result<CommonLayer, Exc> {
    let path = repo.join(SYNC_SCRIPT_REL);
    let bytes = std::fs::read(&path).map_err(|e| os_exc(&e, &path.to_string_lossy()))?;
    let src = pyjson::decode_file_text(&bytes).map_err(|e| load_exc(&e))?.to_utf8().unwrap_or_default();
    let include = shell_array(&src, "ALLOWLIST");
    let arms = docs_case_arms(&src);
    let pick = |f: &dyn Fn(&str) -> bool| -> Vec<String> {
        arms.iter().filter(|(_, body)| f(body)).flat_map(|(pats, _)| pats.iter().cloned()).collect()
    };
    let copy_arms = pick(&|b| b.contains("PATHS+=(\"$f\")") && !b.contains("REPORT_ADD"));
    let union_arms = pick(&|b| b.contains("REPORT_ADD"));
    let mirror_arms = pick(&|b| b.contains("DELETE_PATHS+="));
    if include.is_empty() || copy_arms.is_empty() || union_arms.is_empty() || mirror_arms.is_empty() {
        let repr = |v: &Vec<String>| PyValue::List(v.iter().map(|x| s(x)).collect()).repr();
        return Err(Exc::new(
            "RuntimeError",
            format!(
                "{SYNC_SCRIPT_REL} 에서 공통층 선언을 읽지 못했다 (ALLOWLIST={} · 복사 arm={} · 합집합 arm={} · 미러 arm={})",
                include.len(),
                repr(&copy_arms),
                repr(&union_arms),
                repr(&mirror_arms)
            ),
        ));
    }
    let include_docs: Vec<String> = copy_arms.iter().filter(|p| mirror_arms.contains(p)).cloned().collect();
    let copy_only_docs: Vec<String> = copy_arms.iter().filter(|p| !mirror_arms.contains(p)).cloned().collect();
    let mut union_docs = union_arms.clone();
    union_docs.extend(copy_only_docs.iter().cloned());
    let exclude: Vec<String> = exclude_pathspecs(&src).iter().map(|t| t.trim_matches('\'').to_string()).collect();
    let mut pathspec = include.clone();
    pathspec.extend(include_docs.iter().cloned());
    pathspec.extend(exclude.iter().cloned());
    pathspec.extend(union_docs.iter().map(|p| format!(":(exclude){p}")));
    let mut reach = include.clone();
    reach.extend(include_docs.iter().cloned());
    reach.extend(copy_only_docs.iter().cloned());
    reach.extend(exclude.iter().cloned());
    reach.extend(union_arms.iter().map(|p| format!(":(exclude){p}")));
    Ok(CommonLayer { include, include_docs, union_docs, copy_only_docs, exclude, pathspec, reach_pathspec: reach })
}

fn common_layer(repo: &Path) -> Result<PyValue, Exc> {
    let c = common_layer_data(repo)?;
    Ok(obj(vec![
        ("include", str_list(&c.include)),
        ("include_docs", str_list(&c.include_docs)),
        ("union_docs", str_list(&c.union_docs)),
        ("copy_only_docs", str_list(&c.copy_only_docs)),
        ("exclude", str_list(&c.exclude)),
        ("pathspec", str_list(&c.pathspec)),
        ("reach_pathspec", str_list(&c.reach_pathspec)),
        ("source", s(SYNC_SCRIPT_REL)),
    ]))
}

// ---------------------------------------------------------------- ledger read

fn parse_ledger_text(text: &str) -> Result<PyValue, String> {
    let doc = match pyjson::loads(text) {
        Ok(d) => d,
        Err(e) => return Err(format!("JSON 파싱 실패: {e}")),
    };
    let ok_shape = match doc.peel() {
        PyValue::Dict(d) => match d.get("entries") {
            None => true,
            Some(v) => matches!(v.peel(), PyValue::List(_)),
        },
        _ => false,
    };
    if !ok_shape {
        return Err("최상위가 {entries: [...]} 모양이 아니다".to_string());
    }
    if let Some(PyValue::List(items)) = dget(&doc, "entries").map(|v| v.peel()) {
        if items.iter().any(|e| !matches!(e.peel(), PyValue::Dict(_))) {
            return Err("entries 에 객체가 아닌 항목이 있다".to_string());
        }
    }
    Ok(doc)
}

fn entries_of(doc: &PyValue) -> Vec<PyValue> {
    match dget(doc, "entries").map(|v| v.peel()) {
        Some(PyValue::List(items)) => items.clone(),
        _ => Vec::new(),
    }
}

struct Ledgers {
    entries: Vec<PyValue>,
    sources: Vec<PyValue>,
    conflicts: Vec<PyValue>,
}

#[derive(Default)]
struct Take {
    sources: Vec<PyValue>,
    merged: Vec<(PyValue, PyValue)>,
    conflicts: Vec<PyValue>,
}

fn take(st: &mut Take, label: &str, text: Option<String>, status_if_none: &str, detail: &str) {
    let Some(text) = text else {
        let mut items = vec![("source", s(label)), ("status", s(status_if_none)), ("entries", n(0))];
        if !detail.is_empty() {
            items.push(("detail", s(detail)));
        }
        st.sources.push(obj(items));
        return;
    };
    match parse_ledger_text(&text) {
        Err(why) => st.sources.push(obj(vec![("source", s(label)), ("status", s("unreadable")), ("detail", s(&why))])),
        Ok(doc) => {
            let ents = entries_of(&doc);
            st.sources.push(obj(vec![("source", s(label)), ("status", s("ok")), ("entries", n(ents.len()))]));
            for e in ents {
                let eid = dget(&e, "entry_id").cloned().unwrap_or(PyValue::None);
                if let Some((_, prior)) = st.merged.iter().find(|(k, _)| jeq(k, &eid)) {
                    if !jeq(prior, &e) && !st.conflicts.iter().any(|c| jeq(c, &eid)) {
                        st.conflicts.push(eid);
                    }
                    continue;
                }
                st.merged.push((eid, e));
            }
        }
    }
}

fn read_ledgers(repo: &Path, refs: &[String]) -> Ledgers {
    let mut st = Take::default();
    let path = repo.join(LEDGER_REL);
    if path.is_file() {
        match std::fs::read(&path) {
            Err(e) => st.sources.push(obj(vec![
                ("source", s("worktree")),
                ("status", s("unreadable")),
                ("detail", s(&os_exc(&e, &path.to_string_lossy()).msg)),
            ])),
            Ok(bytes) => match pyjson::decode_file_text(&bytes) {
                Err(e) => st.sources.push(obj(vec![
                    ("source", s("worktree")),
                    ("status", s("unreadable")),
                    ("detail", s(&e.to_string())),
                ])),
                Ok(t) => take(&mut st, "worktree", Some(t.to_utf8().unwrap_or_default()), "absent", ""),
            },
        }
    } else {
        take(&mut st, "worktree", None, "absent", "");
    }
    for r in refs {
        let label = format!("ref:{r}");
        if git(repo, &["rev-parse", "--verify", "--quiet", &format!("{r}^{{commit}}")]).code != 0 {
            take(&mut st, &label, None, "ref-unresolved", "ref 를 커밋으로 풀 수 없다");
            continue;
        }
        let out = git(repo, &["show", &format!("{r}:{LEDGER_REL}")]);
        take(&mut st, &label, if out.code == 0 { Some(out.text()) } else { None }, "absent", "");
    }
    let mut entries: Vec<PyValue> = st.merged.into_iter().map(|(_, e)| e).collect();
    entries.sort_by_key(sort_key);
    Ledgers { entries, sources: st.sources, conflicts: st.conflicts }
}

// ---------------------------------------------------------------- sync point

fn empty_tree(repo: &Path) -> String {
    let out = git(repo, &["hash-object", "-t", "tree", "/dev/null"]);
    if out.code == 0 {
        out.text().trim().to_string()
    } else {
        EMPTY_TREE_FALLBACK.to_string()
    }
}

struct FpCache {
    empty: Option<String>,
    map: HashMap<String, Option<String>>,
}

fn fingerprint(repo: &Path, commit: &str, spec: &[String], cache: &mut FpCache) -> Option<String> {
    if let Some(v) = cache.map.get(commit) {
        return v.clone();
    }
    if cache.empty.is_none() {
        cache.empty = Some(empty_tree(repo));
    }
    let empty = cache.empty.clone().unwrap();
    let out = git(repo, &with_spec(&["diff", "--raw", "-z", "--no-abbrev", "--no-renames"], &[&empty, commit], spec));
    let fp = if out.code == 0 {
        let d = ring::digest::digest(&ring::digest::SHA256, &out.stdout);
        Some(d.as_ref().iter().map(|b| format!("{b:02x}")).collect::<String>())
    } else {
        None
    };
    cache.map.insert(commit.to_string(), fp.clone());
    fp
}

fn find_landing(
    repo: &Path,
    spec: &[String],
    departure: &str,
    dep_commit: &str,
    target: &str,
    cache: &mut FpCache,
) -> Option<(String, String)> {
    if git(repo, &["rev-parse", "--verify", "--quiet", &format!("{dep_commit}^{{commit}}")]).code != 0 {
        return None;
    }
    let mut line: Vec<String> = py_split_ws(&git(repo, &["rev-list", "--ancestry-path", &format!("{dep_commit}..{departure}")]).text())
        .iter()
        .map(|x| x.to_string())
        .collect();
    line.push(git(repo, &["rev-parse", &format!("{dep_commit}^{{commit}}")]).text().trim().to_string());
    let mut by_fp: HashMap<String, (usize, String)> = HashMap::new();
    for (rank, sha) in line.iter().enumerate() {
        if let Some(fp) = fingerprint(repo, sha, spec, cache) {
            by_fp.entry(fp).or_insert((rank, sha.clone()));
        }
    }
    let mut best: Option<(usize, String, String)> = None;
    let listing = git(repo, &["rev-list", &format!("--max-count={MAX_LANDING_SCAN}"), target]).text();
    for sha in py_split_ws(&listing) {
        let Some(fp) = fingerprint(repo, sha, spec, cache) else { continue };
        let Some((rank, dep_sha)) = by_fp.get(&fp) else { continue };
        if best.as_ref().map(|b| *rank < b.0).unwrap_or(true) {
            best = Some((*rank, dep_sha.clone(), sha.to_string()));
            if *rank == 0 {
                break;
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

fn sync_point(repo: &Path, branch: &str, counterpart: Option<&str>, refs: &[String]) -> Result<PyValue, Exc> {
    let led = read_ledgers(repo, refs);
    let base = |extra: Vec<(&str, PyValue)>| -> PyValue {
        let mut items = vec![("sources", PyValue::List(led.sources.clone()))];
        items.extend(extra);
        obj(items)
    };
    if !led.conflicts.is_empty() {
        return Ok(base(vec![
            ("status", s("ledger-conflict")),
            ("commit", PyValue::None),
            ("conflicts", PyValue::List(led.conflicts.clone())),
            ("detail", s("같은 entry_id 가 원장 출처마다 다른 내용이다 -- append-only 가 깨졌다. 어느 쪽도 고르지 않는다")),
        ]));
    }
    let bad: Vec<String> = led
        .sources
        .iter()
        .filter(|row| matches!(dstr(row, "status").as_deref(), Some("unreadable") | Some("ref-unresolved")))
        .map(|row| {
            format!(
                "{}={}({})",
                dstr(row, "source").unwrap_or_default(),
                dstr(row, "status").unwrap_or_default(),
                dstr(row, "detail").unwrap_or_default()
            )
        })
        .collect();
    if !bad.is_empty() {
        return Ok(base(vec![
            ("status", s("ledger-unreadable")),
            ("commit", PyValue::None),
            ("detail", s(&format!("원장을 읽지 못했다 -- 빈 원장으로 접지 않는다: {}", bad.join("; ")))),
        ]));
    }
    if led.entries.is_empty() {
        return Ok(base(vec![
            ("status", s("empty-ledger")),
            ("commit", PyValue::None),
            ("detail", s("원장에 항목이 없다 -- 첫 동기화(부트스트랩)다")),
        ]));
    }
    let spec = common_layer_data(repo)?.pathspec;
    let mut cache = FpCache { empty: None, map: HashMap::new() };
    let mut searched: Vec<PyValue> = Vec::new();
    for entry in led.entries.iter().rev() {
        let eid = dget(entry, "entry_id").cloned().unwrap_or(PyValue::None);
        let dep_v = dget(entry, "departure");
        let dc_v = dget(entry, "departure_commit");
        if !truthy(dep_v) || !truthy(dc_v) {
            searched.push(obj(vec![("entry_id", eid), ("result", s("출발 필드 결손 -- 건너뜀"))]));
            continue;
        }
        let dep = text_of(dep_v.unwrap());
        let dc = text_of(dc_v.unwrap());
        if branch != dep && counterpart.is_some() && Some(dep.as_str()) != counterpart {
            searched.push(obj(vec![("entry_id", eid), ("result", s(&format!("다른 브랜치 쌍({dep}) -- 건너뜀")))]));
            continue;
        }
        if branch == dep && counterpart.is_none() {
            return Ok(base(vec![
                ("status", s("departure")),
                ("commit", s(&dc)),
                ("entry_id", eid),
                ("landing_verified", PyValue::Bool(false)),
                ("detail", s("출발 브랜치이고 --counterpart 가 없어 착지를 확인하지 않았다 -- 기록된 출발 커밋이다")),
            ]));
        }
        let target = if branch == dep { counterpart.unwrap().to_string() } else { branch.to_string() };
        let Some((landed_dep, landed_target)) = find_landing(repo, &spec, &dep, &dc, &target, &mut cache) else {
            searched.push(obj(vec![
                ("entry_id", eid),
                ("result", s(&format!("{target} 에 착지 흔적(공통층 트리 동치) 없음"))),
            ]));
            continue;
        };
        let drift = fingerprint(repo, &dc, &spec, &mut cache) != fingerprint(repo, &landed_dep, &spec, &mut cache);
        let cp_commit = if branch == dep { landed_target.clone() } else { landed_dep.clone() };
        let mut items = vec![
            ("entry_id", eid),
            ("recorded_departure_commit", s(&dc)),
            ("landed_departure_commit", s(&landed_dep)),
            ("landing_commit", s(&landed_target)),
            ("counterpart_commit", s(&cp_commit)),
            ("departure_drift", PyValue::Bool(drift)),
            ("landing_verified", PyValue::Bool(true)),
            ("skipped", PyValue::List(searched.clone())),
        ];
        if drift {
            items.push((
                "drift_note",
                s("원장 기록 커밋과 실제로 내려앉은 출발 트리가 다르다 -- 같은 분류표로 재실행한 회차다(과거 항목은 고치지 않는다 · append-only)"),
            ));
        }
        if branch == dep {
            items.push(("status", s("departure-landed")));
            items.push(("commit", s(&landed_dep)));
        } else {
            items.push(("status", s("converged")));
            items.push(("commit", s(&landed_target)));
        }
        return Ok(base(items));
    }
    Ok(base(vec![
        ("status", s("unresolved")),
        ("commit", PyValue::None),
        ("searched", PyValue::List(searched)),
        (
            "detail",
            s("원장 항목 중 어느 것도 이 브랜치 쌍에 내려앉은 흔적(공통층 트리 동치)을 찾지 못했다 -- 모든 회차가 중단됐거나 이력이 다시 쓰였다"),
        ),
    ]))
}

// ---------------------------------------------------------------- divergence

fn parse_raw_z(text: &str) -> Vec<(String, String, String)> {
    let tokens: Vec<&str> = text.split('\0').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i].trim_start_matches('\n');
        if tok.starts_with(':') && i + 1 < tokens.len() {
            let meta = py_split_ws(&tok[1..]);
            if meta.len() >= 5 {
                out.push((meta[2].to_string(), meta[3].to_string(), tokens[i + 1].to_string()));
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    out
}

struct Hist {
    new: HashMap<String, i64>,
    old: HashSet<String>,
    deleted: Option<i64>,
}

fn history(repo: &Path, rev: &str, paths: &[String]) -> Option<HashMap<String, Hist>> {
    if paths.is_empty() {
        return Some(HashMap::new());
    }
    let lit: Vec<String> = paths.iter().map(|p| format!(":(literal){p}")).collect();
    let out = git(
        repo,
        &with_spec(&["log", "--topo-order", "--format=%x01", "--raw", "-z", "--no-abbrev", "--no-renames", "-m"], &[rev], &lit),
    );
    if out.code != 0 {
        return None;
    }
    let text = out.text();
    let mut res: HashMap<String, Hist> = HashMap::new();
    let mut idx: i64 = -1;
    let tokens: Vec<&str> = text.split('\0').collect();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i].trim_start_matches('\n');
        if tok.starts_with('\u{1}') {
            idx += 1;
            i += 1;
            continue;
        }
        if tok.starts_with(':') && i + 1 < tokens.len() {
            let meta = py_split_ws(&tok[1..]);
            if meta.len() >= 5 {
                let (old, new, path) = (meta[2], meta[3], tokens[i + 1]);
                let h = res.entry(path.to_string()).or_insert_with(|| Hist { new: HashMap::new(), old: HashSet::new(), deleted: None });
                if is_zero_sha(new) {
                    if h.deleted.is_none() {
                        h.deleted = Some(idx);
                    }
                } else {
                    h.new.entry(new.to_string()).or_insert(idx);
                }
                if !is_zero_sha(old) {
                    h.old.insert(old.to_string());
                }
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    Some(res)
}

fn pending(repo: &Path, counterpart: &str, branch: &str, spec: &[String]) -> Option<BTreeMap<String, String>> {
    let out = git(repo, &with_spec(&["diff", "--raw", "-z", "--no-abbrev", "--no-renames"], &[counterpart, branch], spec));
    if out.code != 0 {
        return None;
    }
    Some(parse_raw_z(&out.text()).into_iter().map(|(_o, new, path)| (path, new)).collect())
}

fn unjudged(basis: &str, sp: &PyValue, detail: &str) -> PyValue {
    obj(vec![
        ("status", s("unjudged")),
        ("diverged", PyValue::None),
        ("basis", s(basis)),
        ("base", PyValue::None),
        ("files", PyValue::List(vec![])),
        ("commits", PyValue::List(vec![])),
        ("sync_point", sp.clone()),
        ("detail", s(detail)),
    ])
}

fn blob_ancestry(repo: &Path, branch: &str, counterpart: &str, spec: &[String], sp: &PyValue) -> PyValue {
    let Some(pend) = pending(repo, counterpart, branch, spec) else {
        return unjudged("blob-ancestry", sp, &format!("git diff {counterpart} {branch} 실패"));
    };
    let paths: Vec<String> = pend.keys().cloned().collect();
    let (cp_hist, tg_hist) = (history(repo, counterpart, &paths), history(repo, branch, &paths));
    let (Some(cp_hist), Some(tg_hist)) = (cp_hist, tg_hist) else {
        return unjudged("blob-ancestry", sp, "git log 실패(경로 이력을 읽지 못했다)");
    };
    let mut target_only: Vec<String> = Vec::new();
    let mut reasons: Vec<(String, String)> = Vec::new();
    for path in &paths {
        let blob = &pend[path];
        let Some(th) = tg_hist.get(path) else { continue };
        let ch = cp_hist.get(path);
        let deleted = is_zero_sha(blob);
        let mut here: Option<i64> = if deleted { ch.and_then(|c| c.deleted) } else { ch.and_then(|c| c.new.get(blob).copied()) };
        if !deleted && here.is_none() {
            if let Some(c) = ch {
                if c.old.contains(blob) {
                    here = Some(c.new.values().copied().max().unwrap_or(0) + 1);
                }
            }
        }
        let mut held: Vec<&String> = th.new.keys().collect();
        held.extend(th.old.iter());
        let mut newer: Vec<String> = Vec::new();
        if let (Some(h), Some(c)) = (here, ch) {
            let mut seen = HashSet::new();
            for b in held {
                if seen.insert(b.clone()) && c.new.get(b).copied().unwrap_or(h) < h {
                    newer.push(b.clone());
                }
            }
        }
        newer.sort();
        if here.is_none() || !newer.is_empty() {
            target_only.push(path.clone());
            let mut why = String::from(if deleted { "대상 삭제" } else { "대상 바이트" });
            if here.is_none() {
                why.push_str(" · 상대 이력에 없음");
            } else {
                why.push_str(&format!(" · 상대 이력의 더 새 판본({})을 든 뒤 옛 판본으로 되돌림", short(&newer[0], 12)));
            }
            reasons.push((path.clone(), why));
        }
    }
    let diverged = !target_only.is_empty();
    let sp_status = match dstr(sp, "status") {
        Some(t) => t,
        None => "None".to_string(),
    };
    let mut rmap = PyDict::new();
    for (k, v) in &reasons {
        rmap.set(k, s(v));
    }
    let mut items = vec![
        ("status", s(if diverged { "diverged" } else { "clean" })),
        ("diverged", PyValue::Bool(diverged)),
        ("basis", s("blob-ancestry")),
        ("base", PyValue::None),
        ("files", str_list(&target_only)),
        ("commits", PyValue::List(vec![])),
        ("sync_point", sp.clone()),
        ("file_reasons", PyValue::Dict(rmap)),
        (
            "detail",
            s(&format!(
                "동기 지점이 없어({sp_status}) blob 조상으로 판정했다 -- {branch} 의 공통층 중 {counterpart} 과 다른 {}건 · 동기 지점이 없어 '언제부터' 를 모르는 약한 판정이다",
                paths.len()
            )),
        ),
    ];
    if diverged {
        items.push((
            "remediation",
            s(&format!(
                "{branch} 의 위 파일 변경은 {counterpart} 가 받지 않았다 -- 덮으면 사라진다. 보존할 변경이면 {counterpart} 로 먼저 옮기고(cherry-pick), 버릴 변경이면 사람이 {branch} 에서 되돌린 뒤 다시 동기화하라."
            )),
        ));
    }
    obj(items)
}

fn divergence(repo: &Path, branch: &str, since: Option<&str>, counterpart: Option<&str>, refs: &[String]) -> Result<PyValue, Exc> {
    let mut sp = PyValue::None;
    let mut base: Option<String> = since.map(|x| x.to_string()).filter(|x| !x.is_empty());
    let basis = if base.is_some() { "since" } else { "ledger-sync-point" };
    if base.is_none() {
        sp = sync_point(repo, branch, counterpart, refs)?;
        let st = dstr(&sp, "status").unwrap_or_default();
        if st == "ledger-conflict" || st == "ledger-unreadable" {
            return Ok(unjudged(
                "ledger-integrity",
                &sp,
                &format!(
                    "원장 무결성 결함({st}) -- 동기 지점을 정할 수 없어 판정하지 않는다. 원장을 고친 뒤(append-only 복원 · ref 확인) 다시 판정하라"
                ),
            ));
        }
        base = dstr(&sp, "commit").filter(|x| !x.is_empty());
    }
    let spec = common_layer_data(repo)?.pathspec;
    let Some(base) = base else {
        let Some(cp) = counterpart else {
            return Ok(obj(vec![
                ("status", s("no-sync-point")),
                ("diverged", PyValue::None),
                ("sync_point", sp),
                ("detail", s("동기 지점이 없고 --counterpart 가 없어 blob 조상 판정도 할 수 없다 (첫 실행이거나 중단된 동기화)")),
            ]));
        };
        return Ok(blob_ancestry(repo, branch, cp, &spec, &sp));
    };
    let clean = |base: &str, absorbed: Vec<PyValue>, sp: &PyValue| {
        obj(vec![
            ("status", s("clean")),
            ("diverged", PyValue::Bool(false)),
            ("basis", s(basis)),
            ("base", s(base)),
            ("files", PyValue::List(vec![])),
            ("commits", PyValue::List(vec![])),
            ("absorbed", PyValue::List(absorbed)),
            ("sync_point", sp.clone()),
        ])
    };
    let quiet = git(repo, &with_spec(&["diff", "--quiet"], &[&base, branch], &spec));
    if quiet.code != 0 && quiet.code != 1 {
        let e = quiet.err_text();
        return Ok(unjudged(basis, &sp, &format!("git diff {base} {branch} 실패: {}", short(&e, 300))));
    }
    if quiet.code == 0 {
        return Ok(clean(&base, vec![], &sp));
    }
    let names = git(repo, &with_spec(&["diff", "--name-only", "-z"], &[&base, branch], &spec)).text();
    let changed: Vec<String> = names.split('\0').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect();
    let mut absorbed: Vec<PyValue> = Vec::new();
    let files: Vec<String>;
    match counterpart {
        None => files = changed,
        Some(cp) => {
            let Some(pend) = pending(repo, cp, branch, &spec) else {
                return Ok(unjudged(basis, &sp, &format!("git diff {cp} {branch} 실패")));
            };
            let cp_base = dstr(&sp, "counterpart_commit").filter(|x| !x.is_empty());
            let cand: Vec<(&String, &String)> = changed.iter().filter_map(|p| pend.get(p).map(|b| (p, b))).collect();
            let mut carried: HashSet<String> = HashSet::new();
            if let (false, Some(cb)) = (cand.is_empty(), cp_base.as_ref()) {
                let mut sorted: Vec<String> = cand.iter().map(|(p, _)| (*p).clone()).collect();
                sorted.sort();
                let Some(hist) = history(repo, &format!("{cb}..{cp}"), &sorted) else {
                    return Ok(unjudged(basis, &sp, &format!("git log {cb}..{cp} 실패")));
                };
                for (p, blob) in &cand {
                    if let Some(h) = hist.get(*p) {
                        let hit = if is_zero_sha(blob) { h.deleted.is_some() } else { h.new.contains_key(*blob) };
                        if hit {
                            carried.insert((*p).clone());
                        }
                    }
                }
            }
            files = changed.iter().filter(|p| pend.contains_key(*p) && !carried.contains(*p)).cloned().collect();
            for p in changed.iter().filter(|p| !files.contains(p)) {
                let why = if !pend.contains_key(p) {
                    "상대와 바이트 동일 -- 동기화가 바꿀 것이 없다"
                } else {
                    "동기 지점 이후 상대 계보에 같은 바이트가 들어왔다(옮겨졌다)"
                };
                absorbed.push(obj(vec![("file", s(p)), ("why", s(why))]));
            }
        }
    }
    if files.is_empty() {
        return Ok(clean(&base, absorbed, &sp));
    }
    let lit: Vec<String> = files.iter().map(|f| format!(":(literal){f}")).collect();
    let log = git(repo, &with_spec(&["log", "--oneline"], &[&format!("{base}..{branch}")], &lit)).text();
    let commits: Vec<String> = splitlines(&log);
    Ok(obj(vec![
        ("status", s("diverged")),
        ("diverged", PyValue::Bool(true)),
        ("basis", s(basis)),
        ("base", s(&base)),
        ("files", str_list(&files)),
        ("commits", str_list(&commits)),
        ("absorbed", PyValue::List(absorbed)),
        ("sync_point", sp),
        (
            "remediation",
            s("대상 브랜치의 공통층이 출발 브랜치가 받지 않은 변경을 들고 있다. 덮어쓰면 사라진다 -- 보존할 변경이면 위 커밋을 출발 브랜치로 먼저 옮기고(cherry-pick · 옮기면 이 판정이 흡수로 인식한다), 버릴 변경이면 사람이 대상 브랜치에서 되돌린 뒤 다시 동기화하라."),
        ),
    ]))
}

// ---------------------------------------------------------------- ledger write

fn load(repo: &Path) -> Result<PyValue, String> {
    let path = repo.join(LEDGER_REL);
    if !path.is_file() {
        return Ok(obj(vec![("schema_version", PyValue::int(1)), ("entries", PyValue::List(vec![]))]));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("{LEDGER_REL}: {}", os_exc(&e, &path.to_string_lossy()).msg))?;
    let text = pyjson::decode_file_text(&bytes).map_err(|e| format!("{LEDGER_REL}: {e}"))?;
    parse_ledger_text(&text.to_utf8().unwrap_or_default()).map_err(|why| format!("{LEDGER_REL}: {why}"))
}

fn save(repo: &Path, doc: &PyValue) -> Result<(), Exc> {
    let path = repo.join(LEDGER_REL);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os_exc(&e, &parent.to_string_lossy()))?;
    }
    let mut text = dumps(doc);
    text.push('\n');
    std::fs::write(&path, text).map_err(|e| os_exc(&e, &path.to_string_lossy()))
}

fn fill_re() -> Regex {
    re(&format!(r"(?<!<){}(?::[^<>]*)?>(?!>)", fancy_regex::escape(FILL_MARK)))
}

fn placeholder_paths(re: &Regex, v: &PyValue, where_: &str, out: &mut Vec<String>) {
    match v.peel() {
        PyValue::Str(t) => {
            let text = t.to_utf8().unwrap_or_default();
            if matches!(re.is_match(&text), Ok(true)) {
                out.push(if where_.is_empty() { "<root>".to_string() } else { where_.to_string() });
            }
        }
        PyValue::Dict(d) => {
            for (k, val) in d.iter() {
                let key = k.to_utf8().unwrap_or_default();
                let w = if where_.is_empty() { key } else { format!("{where_}.{key}") };
                placeholder_paths(re, val, &w, out);
            }
        }
        PyValue::List(items) => {
            for (i, val) in items.iter().enumerate() {
                placeholder_paths(re, val, &format!("{where_}[{i}]"), out);
            }
        }
        _ => {}
    }
}

fn validate_entry(entry: &PyValue) -> Vec<String> {
    let PyValue::Dict(_) = entry.peel() else {
        return vec![format!("항목이 객체가 아니다: {}", entry.type_name())];
    };
    let mut problems = Vec::new();
    for key in ["entry_id", "utc", "departure", "departure_commit", "approved_by", "approved_utc", "classification"] {
        if !truthy(dget(entry, key)) {
            problems.push(format!("필수 필드 없음: {key}"));
        }
    }
    let mut holes = Vec::new();
    placeholder_paths(&fill_re(), entry, "", &mut holes);
    for w in holes {
        problems.push(format!("자리표시자가 남아 있다({FILL_MARK}…>): {w} -- ③ 승인 전에 채워야 한다"));
    }
    let rows: Vec<PyValue> = match dget(entry, "classification").map(|v| v.peel()) {
        Some(PyValue::List(items)) => items.clone(),
        Some(PyValue::Str(t)) => t.code_points().iter().map(|c| s(&char::from_u32(*c).unwrap_or('?').to_string())).collect(),
        Some(PyValue::Dict(d)) => d.iter().map(|(k, _)| PyValue::Str(k.clone())).collect(),
        _ => Vec::new(),
    };
    for row in &rows {
        let PyValue::Dict(_) = row.peel() else {
            problems.push(format!("분류 행이 객체가 아니다: {}", row.repr()));
            continue;
        };
        if !truthy(dget(row, "file")) {
            problems.push("분류 행에 file 이 없다".to_string());
        }
        let verdict = dget(row, "verdict");
        let ok_verdict = matches!(verdict.map(|v| v.peel()), Some(PyValue::Str(t)) if t.equals("common_promote") || t.equals("branch_only"));
        if !ok_verdict {
            let repr = verdict.map(|v| v.repr()).unwrap_or_else(|| "None".to_string());
            problems.push(format!("판정은 {VERDICTS_REPR} 중 하나여야 한다: {repr}"));
        }
        if !truthy(dget(row, "reason")) {
            let repr = dget(row, "file").map(|v| v.repr()).unwrap_or_else(|| "None".to_string());
            problems.push(format!("분류 행에 근거가 없다: {repr} -- 근거 없는 판정은 사람이 검토할 수 없다"));
        }
        let prov_ok = matches!(dget(row, "provenance").map(|v| v.peel()), Some(PyValue::Str(t)) if t.equals("agent-judged"));
        if !prov_ok {
            let repr = dget(row, "provenance").map(|v| v.repr()).unwrap_or_else(|| "None".to_string());
            problems.push(format!("provenance 는 'agent-judged' 여야 한다: {repr}"));
        }
    }
    problems
}

fn set_key(doc: &mut PyValue, key: &str, val: PyValue) {
    if let PyValue::Dict(d) = doc {
        d.set(key, val);
    }
}

fn append(repo: &Path, entry: &PyValue) -> Result<PyValue, Exc> {
    let problems = validate_entry(entry);
    if !problems.is_empty() {
        return Ok(obj(vec![("status", s("rejected")), ("problems", str_list(&problems))]));
    }
    let mut doc = match load(repo) {
        Ok(d) => d,
        Err(e) => {
            return Ok(obj(vec![
                ("status", s("rejected")),
                ("problems", str_list(&[format!("기존 원장을 읽을 수 없어 쓰지 않는다(덮어쓰기 방지): {e}")])),
            ]))
        }
    };
    let mut entries = entries_of(&doc);
    let eid = dget(entry, "entry_id").cloned().unwrap_or(PyValue::None);
    for prior in &entries {
        let pid = dget(prior, "entry_id").cloned().unwrap_or(PyValue::None);
        if !jeq(&pid, &eid) {
            continue;
        }
        if jeq(prior, entry) {
            return Ok(obj(vec![
                ("status", s("already-recorded")),
                ("entry_id", eid),
                ("total", n(entries.len())),
                ("path", s(LEDGER_REL)),
            ]));
        }
        return Ok(obj(vec![
            ("status", s("rejected")),
            (
                "problems",
                str_list(&[format!(
                    "entry_id 중복이고 내용이 다르다: {} -- 원장은 append-only 다(재실행이면 같은 바이트여야 한다)",
                    text_of(&eid)
                )]),
            ),
        ]));
    }
    entries.push(entry.clone());
    let total = entries.len();
    set_key(&mut doc, "entries", PyValue::List(entries));
    set_key(&mut doc, "schema_version", PyValue::int(1));
    save(repo, &doc)?;
    Ok(obj(vec![("status", s("appended")), ("entry_id", eid), ("total", n(total)), ("path", s(LEDGER_REL))]))
}

fn merge(repo: &Path, from_ref: &str) -> Result<PyValue, Exc> {
    let out = git(repo, &["show", &format!("{from_ref}:{LEDGER_REL}")]);
    if out.code != 0 {
        return Ok(obj(vec![("status", s("absent")), ("detail", s(&format!("{from_ref} 에 원장이 없다"))), ("added", n(0))]));
    }
    let other = match parse_ledger_text(&out.text()) {
        Ok(d) => d,
        Err(why) => return Ok(obj(vec![("status", s("unreadable")), ("detail", s(&why)), ("added", n(0))])),
    };
    let mut doc = match load(repo) {
        Ok(d) => d,
        Err(e) => {
            return Ok(obj(vec![
                ("status", s("unreadable")),
                ("detail", s(&format!("기존 원장을 읽을 수 없어 쓰지 않는다: {e}"))),
                ("added", n(0)),
            ]))
        }
    };
    let mut entries = entries_of(&doc);
    let have: Vec<PyValue> = entries.iter().map(|e| dget(e, "entry_id").cloned().unwrap_or(PyValue::None)).collect();
    let added: Vec<PyValue> = entries_of(&other)
        .into_iter()
        .filter(|e| {
            let id = dget(e, "entry_id").cloned().unwrap_or(PyValue::None);
            !have.iter().any(|h| jeq(h, &id))
        })
        .collect();
    if !added.is_empty() {
        entries.extend(added.iter().cloned());
        entries.sort_by_key(sort_key);
        set_key(&mut doc, "entries", PyValue::List(entries.clone()));
        set_key(&mut doc, "schema_version", PyValue::int(1));
        save(repo, &doc)?;
    }
    let ids: Vec<PyValue> = added.iter().map(|e| dget(e, "entry_id").cloned().unwrap_or(PyValue::None)).collect();
    Ok(obj(vec![
        ("status", s("merged")),
        ("added", n(added.len())),
        ("added_ids", PyValue::List(ids)),
        ("total", n(entries.len())),
    ]))
}

// ---------------------------------------------------------------- closing_sequence.sh replacements

fn read_json_file(path: &str) -> Result<PyValue, Exc> {
    let bytes = std::fs::read(path).map_err(|e| os_exc(&e, path))?;
    let text = pyjson::decode_file_text(&bytes).map_err(|e| load_exc(&e))?;
    pyjson::loads_text(&text).map_err(|e| load_exc(&e))
}

fn emit_skeleton(repo: &Path, cur: &str, other: &str) -> Result<PyValue, Exc> {
    let sp = sync_point(repo, cur, Some(other), &[other.to_string()])?;
    let base = dstr(&sp, "commit").filter(|x| !x.is_empty());
    let spec = common_layer_data(repo)?;
    let names = {
        let out = git(repo, &with_spec(&["diff", "--name-only", "-z"], &[other, "HEAD"], &spec.reach_pathspec));
        if out.code == 0 { out.text() } else { String::new() }
    };
    let files: Vec<String> = names.split('\0').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect();
    let status = dstr(&sp, "status").unwrap_or_else(|| "None".to_string());
    let reach = match &base {
        Some(b) => format!("{other} vs HEAD 트리차이 · 동기 지점 {}({status})", short(b, 12)),
        None => format!("{other} vs HEAD 트리차이 (동기 지점 없음 — {status})"),
    };
    let f = FILL_MARK;
    let rows: Vec<PyValue> = files
        .iter()
        .map(|file| {
            obj(vec![
                ("file", s(file)),
                ("verdict", s("<common_promote|branch_only>")),
                ("reason", s(&format!("{f}>"))),
                ("provenance", s("agent-judged")),
            ])
        })
        .collect();
    Ok(obj(vec![
        ("entry_id", s(&format!("{f}: 예 sync-YYYYMMDDHHMM>"))),
        ("utc", s(&format!("{f}: 시각은 주입만 받는다>"))),
        ("departure", s(cur)),
        (
            "departure_commit",
            s(&format!("{f}: 분류한 시점의 출발 브랜치 HEAD — 원장 파일은 동기화 제외라 ④ 커밋과 공통층이 같다>")),
        ),
        ("approved_by", s(&format!("{f}: 사람>"))),
        ("approved_utc", s(&format!("{f}>"))),
        ("_reach", s(&reach)),
        ("classification", PyValue::List(rows)),
    ]))
}

fn realpath(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    if let Ok(c) = std::fs::canonicalize(&abs) {
        return c;
    }
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = abs.clone();
    loop {
        if let Ok(c) = std::fs::canonicalize(&cur) {
            let mut out = c;
            for part in parts.iter().rev() {
                match Path::new(part).components().next() {
                    Some(Component::ParentDir) => {
                        out.pop();
                    }
                    Some(Component::CurDir) | None => {}
                    _ => out.push(part),
                }
            }
            return out;
        }
        match (cur.file_name().map(|x| x.to_os_string()), cur.parent().map(|x| x.to_path_buf())) {
            (Some(name), Some(parent)) => {
                parts.push(name);
                cur = parent;
            }
            _ => {
                if let Some(Component::ParentDir) = cur.components().next_back() {
                    cur.pop();
                    parts.push(std::ffi::OsString::from(".."));
                    continue;
                }
                return abs;
            }
        }
    }
}

fn relpath(path: &Path, start: &Path) -> String {
    let a: Vec<_> = path.components().collect();
    let b: Vec<_> = start.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".to_string(); b.len() - common];
    parts.extend(a[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    if parts.is_empty() { ".".to_string() } else { parts.join("/") }
}

/// Lines to print; `Err` carries the message of `sys.exit(msg)` (exit code 1).
fn evidence_rels(repo: &str, cwd: &str, arg: &str) -> Result<Vec<String>, String> {
    let repo = realpath(Path::new(repo));
    let man_in = if Path::new(arg).is_absolute() { PathBuf::from(arg) } else { Path::new(cwd).join(arg) };
    let man = realpath(&man_in);
    if !man.is_file() {
        return Err(format!("work-manifest 가 없다: {}", man.display()));
    }
    let rel_under = |path: &Path, what: &str| -> Result<String, String> {
        let rel = relpath(path, &repo);
        if rel.starts_with("..") {
            return Err(format!("{what} 가 저장소 밖이다(게이트가 해소하지 못한다): {}", path.display()));
        }
        Ok(rel)
    };
    let mut out = vec![rel_under(&man, "work-manifest")?];
    let doc = read_json_file(&man.to_string_lossy()).map_err(|e| format!("{}: {}", e.kind, e.msg))?;
    let approval = dget(&doc, "execution_approval").filter(|v| v.is_truthy());
    let plan = approval.and_then(|a| dget(a, "plan_path")).filter(|v| v.is_truthy());
    if let Some(p) = plan {
        let PyValue::Str(t) = p.peel() else {
            return Err("TypeError: plan_path is not a string".to_string());
        };
        let joined = man.parent().unwrap_or(Path::new("/")).join(t.to_utf8().unwrap_or_default());
        out.push(rel_under(&realpath(&joined), "plan")?);
    }
    Ok(out)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// `strptime(utc, "%Y-%m-%dT%H:%M:%SZ")` (1-2 digit fields as `strptime` takes them) converted UTC → KST and formatted `%Y-%m-%dT%H:%M:%S`.
fn utc_to_kst(utc: &str) -> Result<String, String> {
    let fail = || format!("time data '{utc}' does not match format '%Y-%m-%dT%H:%M:%SZ'");
    let b = utc.as_bytes();
    let mut i = 0;
    let num = |min: usize, max: usize, i: &mut usize| -> Option<i64> {
        let start = *i;
        while *i < b.len() && *i - start < max && b[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i - start < min {
            return None;
        }
        utc[start..*i].parse().ok()
    };
    let lit = |c: u8, i: &mut usize| -> bool {
        if *i < b.len() && b[*i] == c {
            *i += 1;
            true
        } else {
            false
        }
    };
    let y = num(4, 4, &mut i).ok_or_else(fail)?;
    if !lit(b'-', &mut i) {
        return Err(fail());
    }
    let mo = num(1, 2, &mut i).ok_or_else(fail)?;
    if !lit(b'-', &mut i) {
        return Err(fail());
    }
    let d = num(1, 2, &mut i).ok_or_else(fail)?;
    if !lit(b'T', &mut i) {
        return Err(fail());
    }
    let h = num(1, 2, &mut i).ok_or_else(fail)?;
    if !lit(b':', &mut i) {
        return Err(fail());
    }
    let mi = num(1, 2, &mut i).ok_or_else(fail)?;
    if !lit(b':', &mut i) {
        return Err(fail());
    }
    let se = num(1, 2, &mut i).ok_or_else(fail)?;
    if !lit(b'Z', &mut i) || i != b.len() {
        return Err(if i < b.len() { format!("unconverted data remains: {}", &utc[i..]) } else { fail() });
    }
    let dim = [31, if leap(y) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if y < 1 || !(1..=12).contains(&mo) || d < 1 || d > dim[(mo - 1) as usize] || h > 23 || mi > 59 || se > 59 {
        return Err("day is out of range for month".to_string());
    }
    let total = days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se + 9 * 3600;
    let (days, rem) = (total.div_euclid(86400), total.rem_euclid(86400));
    let (yy, mm, dd) = civil_from_days(days);
    if yy > 9999 {
        return Err("date value out of range".to_string());
    }
    Ok(format!("{yy:04}-{mm:02}-{dd:02}T{:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60))
}

#[path = "layer_ledger_selftest.rs"]
mod selftest;

// ---------------------------------------------------------------- command line

fn usage_err(msg: &str) -> i32 {
    eprintln!("usage: layer_ledger.py [-h] [--self-test] {{common-layer,sync-point,divergence,validate,append,merge,emit-skeleton,entry-id,evidence-rels,utc-to-kst}} ...");
    eprintln!("layer_ledger.py: error: {msg}");
    2
}

fn resolve(p: &str) -> PathBuf {
    realpath(Path::new(p))
}

fn print_json(v: &PyValue) {
    println!("{}", dumps(v));
}

pub fn run(args: &[String]) -> i32 {
    let mut i = 0;
    let mut cmd: Option<String> = None;
    while i < args.len() {
        match args[i].as_str() {
            "--self-test" => {
                return selftest::self_test();
            }
            "-h" | "--help" => {
                eprintln!("layer_ledger.py: see the Python docstring (common-layer · sync-point · divergence · validate · append · merge · emit-skeleton · entry-id · evidence-rels · utc-to-kst)");
                return 0;
            }
            other if other.starts_with('-') => return usage_err(&format!("unrecognized arguments: {other}")),
            other => {
                cmd = Some(other.to_string());
                i += 1;
                break;
            }
        }
    }
    let Some(cmd) = cmd else {
        eprintln!("usage: layer_ledger.py [-h] [--self-test] {{common-layer,sync-point,divergence,validate,append,merge}} ...");
        return 2;
    };
    let known = [
        "common-layer", "sync-point", "divergence", "validate", "append", "merge", "emit-skeleton", "entry-id", "evidence-rels",
        "utc-to-kst",
    ];
    if !known.contains(&cmd.as_str()) {
        return usage_err(&format!("argument cmd: invalid choice: '{cmd}'"));
    }
    let allowed: &[&str] = match cmd.as_str() {
        "common-layer" => &["--repo"],
        "sync-point" => &["--repo", "--branch", "--counterpart", "--ledger-ref"],
        "divergence" => &["--repo", "--branch", "--counterpart", "--ledger-ref", "--since"],
        "validate" => &["--entry-file"],
        "append" => &["--repo", "--entry-file"],
        "merge" => &["--repo", "--from-ref"],
        "emit-skeleton" => &["--repo", "--branch", "--counterpart"],
        "entry-id" => &["--entry-file"],
        "evidence-rels" => &["--repo", "--cwd", "--manifest"],
        _ => &["--entry-file"],
    };
    let mut opts: HashMap<String, String> = HashMap::new();
    let mut ledger_refs: Vec<String> = Vec::new();
    while i < args.len() {
        let a = &args[i];
        let (name, inline) = match a.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_string(), Some(v.to_string())),
            _ => (a.clone(), None),
        };
        if !allowed.contains(&name.as_str()) {
            return usage_err(&format!("unrecognized arguments: {a}"));
        }
        let value = match inline {
            Some(v) => v,
            None => {
                i += 1;
                match args.get(i) {
                    Some(v) => v.clone(),
                    None => return usage_err(&format!("argument {name}: expected one argument")),
                }
            }
        };
        if name == "--ledger-ref" {
            ledger_refs.push(value);
        } else {
            opts.insert(name, value);
        }
        i += 1;
    }
    let need = |names: &[&str]| -> Option<String> {
        let miss: Vec<&str> = names.iter().copied().filter(|k| !opts.contains_key(*k)).collect();
        if miss.is_empty() { None } else { Some(format!("the following arguments are required: {}", miss.join(", "))) }
    };
    let required: &[&str] = match cmd.as_str() {
        "sync-point" | "divergence" | "emit-skeleton" => &["--branch"],
        "validate" | "append" | "entry-id" | "utc-to-kst" => &["--entry-file"],
        "merge" => &["--from-ref"],
        "evidence-rels" => &["--cwd", "--manifest"],
        _ => &[],
    };
    if let Some(m) = need(required) {
        return usage_err(&m);
    }
    let get = |k: &str| opts.get(k).cloned();
    let repo_arg = get("--repo").unwrap_or_else(|| ".".to_string());

    // The three shell replacements keep their original, plain behaviour: stdout text, exit 1 on failure.
    match cmd.as_str() {
        "entry-id" => {
            return match read_json_file(&get("--entry-file").unwrap()) {
                Ok(doc) => match dget(&doc, "entry_id") {
                    Some(v) => {
                        println!("{}", text_of(v));
                        0
                    }
                    None => {
                        eprintln!("KeyError: 'entry_id'");
                        1
                    }
                },
                Err(e) => {
                    eprintln!("{}: {}", e.kind, e.msg);
                    1
                }
            };
        }
        "utc-to-kst" => {
            return match read_json_file(&get("--entry-file").unwrap()) {
                Ok(doc) => match dget(&doc, "utc").map(|v| v.peel()) {
                    Some(PyValue::Str(t)) => match utc_to_kst(&t.to_utf8().unwrap_or_default()) {
                        Ok(k) => {
                            println!("{k}");
                            0
                        }
                        Err(m) => {
                            eprintln!("ValueError: {m}");
                            1
                        }
                    },
                    Some(_) => {
                        eprintln!("TypeError: strptime() argument 1 must be str");
                        1
                    }
                    None => {
                        eprintln!("KeyError: 'utc'");
                        1
                    }
                },
                Err(e) => {
                    eprintln!("{}: {}", e.kind, e.msg);
                    1
                }
            };
        }
        "evidence-rels" => {
            return match evidence_rels(&repo_arg, &get("--cwd").unwrap(), &get("--manifest").unwrap()) {
                Ok(lines) => {
                    for l in lines {
                        println!("{l}");
                    }
                    0
                }
                Err(m) => {
                    eprintln!("{m}");
                    1
                }
            };
        }
        _ => {}
    }

    let result: Result<PyValue, Exc> = (|| {
        match cmd.as_str() {
            "validate" => {
                let entry = read_json_file(&get("--entry-file").unwrap())?;
                let problems = validate_entry(&entry);
                Ok(obj(vec![
                    ("status", s(if problems.is_empty() { "valid" } else { "rejected" })),
                    ("problems", str_list(&problems)),
                ]))
            }
            "common-layer" => common_layer(&resolve(&repo_arg)),
            "sync-point" => {
                let b = get("--branch").unwrap();
                sync_point(&resolve(&repo_arg), &b, get("--counterpart").as_deref(), &ledger_refs)
            }
            "divergence" => {
                let b = get("--branch").unwrap();
                divergence(&resolve(&repo_arg), &b, get("--since").as_deref(), get("--counterpart").as_deref(), &ledger_refs)
            }
            "append" => {
                let entry = read_json_file(&get("--entry-file").unwrap())?;
                append(&resolve(&repo_arg), &entry)
            }
            "merge" => merge(&resolve(&repo_arg), &get("--from-ref").unwrap()),
            _ => {
                let b = get("--branch").unwrap();
                let other = get("--counterpart").unwrap_or_default();
                emit_skeleton(&resolve(&repo_arg), &b, &other)
            }
        }
    })();
    match result {
        Err(e) => {
            let mut d = PyDict::new();
            d.set("status", s("error"));
            d.set("detail", s(&format!("{}: {}", e.kind, e.msg)));
            let text = PyValue::Dict(d).dumps_unicode().ok().and_then(|t| t.to_utf8()).unwrap_or_default();
            println!("{text}");
            2
        }
        Ok(res) => {
            print_json(&res);
            let st = dstr(&res, "status");
            let truthy_div = matches!(dget(&res, "diverged").map(|v| v.peel()), Some(PyValue::Bool(true)));
            if cmd == "divergence" && truthy_div {
                5
            } else if (cmd == "append" || cmd == "validate") && st.as_deref() == Some("rejected") {
                5
            } else {
                0
            }
        }
    }
}
