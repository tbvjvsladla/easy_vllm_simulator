//! Port of `.claude/policies/runtime/constitution_lint.py` (information, not a verdict: exit 0 unless the repo has no
//! CLAUDE.md, then 2). The specialised-layer path spec comes from its single owner, the topology parity library
//! (`role_contract::topology::LAYER_PATHSPEC`), not from a second literal here.
use crate::gitx;
use crate::pytext::{self, py_splitlines, py_strip, take_chars};
use crate::tempdir::TempDir;
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeSet;
use std::path::Path;

pub const EXIT_OK: i32 = 0;
pub const EXIT_UNREADABLE: i32 = 2;
pub const ALWAYS_LOADED: [&str; 3] = ["CLAUDE.md", ".claude/rules/workflow.md", ".claude/rules/docs.md"];
pub const MIN_SENTENCE_CHARS: usize = 40;

fn s(v: &str) -> PyValue {
    PyValue::Str(PyStr::new(v))
}

fn layer_pathspec() -> &'static str {
    role_contract::topology::LAYER_PATHSPEC
}

fn git_ls(root: &str, specs: &[&str]) -> Vec<String> {
    let mut a = vec!["ls-files", "-z", "--"];
    a.extend_from_slice(specs);
    let out = gitx::git(root, &a);
    if out.code != 0 {
        return Vec::new();
    }
    gitx::split_z(&out.stdout)
}

pub fn constitution_files(root: &str) -> Vec<String> {
    let set: BTreeSet<String> = git_ls(root, &["CLAUDE.md", ".claude/rules/*.md", layer_pathspec()]).into_iter().collect();
    set.into_iter().collect()
}

pub fn prose_files(root: &str) -> Vec<String> {
    let mut v = constitution_files(root);
    v.extend(git_ls(root, &[".claude/skills/*/SKILL.md", ".claude/skills/*/references/**/*.md"]));
    let set: BTreeSet<String> = v.into_iter().collect();
    set.into_iter().collect()
}

fn read_text(path: &Path) -> Option<String> {
    std::fs::read(path).ok().and_then(|b| String::from_utf8(b).ok())
}

fn normalize(line: &str) -> String {
    let ws = pytext::re(r"\s+");
    let t = ws.replace_all(py_strip(line), " ").into_owned();
    let marks = pytext::re(r"[`*_]");
    marks.replace_all(&t, "").into_owned()
}

pub struct Dup {
    pub sentence: String,
    pub files: Vec<String>,
}

pub fn duplicate_lines(root: &str) -> Vec<Dup> {
    let noise = pytext::re_match(r"^[\s|:#>*+\-=`_.]*$");
    let contract = pytext::re_match(r"^\*\*topology:");
    let mut index: Vec<(String, BTreeSet<String>)> = Vec::new();
    for rel in constitution_files(root) {
        let Some(text) = read_text(&Path::new(root).join(&rel)) else { continue };
        for raw in py_splitlines(&text) {
            let norm = normalize(raw);
            if norm.chars().count() < MIN_SENTENCE_CHARS || pytext::search(&noise, raw) {
                continue;
            }
            if pytext::search(&contract, py_strip(raw)) {
                continue;
            }
            match index.iter_mut().find(|(k, _)| *k == norm) {
                Some((_, f)) => {
                    f.insert(rel.clone());
                }
                None => {
                    let mut f = BTreeSet::new();
                    f.insert(rel.clone());
                    index.push((norm, f));
                }
            }
        }
    }
    let mut out: Vec<Dup> = index
        .into_iter()
        .filter(|(_, f)| f.len() > 1)
        .map(|(n, f)| Dup { sentence: take_chars(&n, 160), files: f.into_iter().collect() })
        .collect();
    // sorted(key=(-len(files), sentence)): stable, Python str ordering = code point order
    out.sort_by(|a, b| b.files.len().cmp(&a.files.len()).then_with(|| a.sentence.cmp(&b.sentence)));
    out
}

pub fn always_loaded_bytes(root: &str) -> (Vec<(String, Option<u64>)>, u64) {
    let mut sizes: Vec<(String, Option<u64>)> = Vec::new();
    let mut put = |k: String, v: Option<u64>| {
        if let Some(e) = sizes.iter_mut().find(|(n, _)| *n == k) {
            e.1 = v;
        } else {
            sizes.push((k, v));
        }
    };
    for rel in ALWAYS_LOADED {
        let p = Path::new(root).join(rel);
        put(rel.to_string(), if p.is_file() { std::fs::metadata(&p).ok().map(|m| m.len()) } else { None });
    }
    for rel in git_ls(root, &[".claude/rules/*.topology.md"]) {
        let size = std::fs::metadata(Path::new(root).join(&rel)).map(|m| m.len()).ok();
        put(rel, size);
    }
    let total = sizes.iter().filter_map(|(_, v)| *v).sum();
    (sizes, total)
}

pub enum Ghosts {
    Error(String),
    Found(Vec<(String, Vec<String>, usize)>),
}

pub fn ghost_policy_ids(root: &str) -> Ghosts {
    let reg = Path::new(root).join(".claude/policies/registry.yaml");
    let declared: Result<BTreeSet<Option<String>>, String> = (|| {
        let bytes = std::fs::read(&reg).map_err(|e| {
            format!("[Errno {}] {}: {}", e.raw_os_error().unwrap_or(0), os_message(&e), pytext::py_repr_str(&reg.to_string_lossy()))
        })?;
        let text = pyjson::decode_file_text(&bytes).map_err(|e| e.to_string())?;
        let doc = pyjson::loads_text(&text).map_err(|e| e.to_string())?;
        let PyValue::Dict(d) = doc.peel() else { return Err("string indices must be integers, not 'str'".into()) };
        let Some(pol) = d.get("policies") else { return Err("'policies'".into()) };
        let PyValue::List(items) = pol.peel() else { return Err("'policies' is not a list".into()) };
        let mut out = BTreeSet::new();
        for p in items {
            match p.peel() {
                PyValue::Dict(pd) => out.insert(match pd.get("policy_id").map(|v| v.peel()) {
                    Some(PyValue::Str(t)) => t.to_utf8(),
                    _ => None,
                }),
                other => return Err(format!("'{}' object has no attribute 'get'", other.type_name())),
            };
        }
        Ok(out)
    })();
    let declared = match declared {
        Ok(d) => d,
        Err(e) => return Ghosts::Error(format!("registry 를 읽지 못했다({e}): {}", reg.display())),
    };
    let rx = pytext::re(r"policy:([A-Z][A-Z0-9_]{3,})");
    let mut ghosts: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for rel in prose_files(root) {
        let Some(text) = read_text(&Path::new(root).join(&rel)) else { continue };
        for id in pytext::findall(&rx, &text) {
            if !declared.contains(&Some(id.clone())) {
                ghosts.entry(id).or_default().push(rel.clone());
            }
        }
    }
    Ghosts::Found(
        ghosts
            .into_iter()
            .map(|(k, v)| {
                let n = v.len();
                let set: BTreeSet<String> = v.into_iter().collect();
                (k, set.into_iter().collect(), n)
            })
            .collect(),
    )
}

fn os_message(e: &std::io::Error) -> String {
    let t = e.to_string();
    match t.find(" (os error") {
        Some(p) => t[..p].to_string(),
        None => t,
    }
}

pub enum Router {
    Error(String),
    Obs { skill_md_lines: usize, dated: usize, ref_files: usize, ref_lines: usize },
}

pub fn skill_router_observation(root: &str) -> Router {
    let skill = Path::new(root).join(".claude/skills/terraforming_node/SKILL.md");
    let text = match std::fs::read(&skill) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(_) => return Router::Error(format!("부재: {}", skill.display())),
    };
    let lines = py_splitlines(&text);
    let refs = git_ls(
        root,
        &[".claude/skills/terraforming_node/references/**/*.md", ".claude/skills/terraforming_node/references/*.md"],
    );
    let mut ref_lines = 0;
    for rel in &refs {
        if let Some(t) = read_text(&Path::new(root).join(rel)) {
            ref_lines += py_splitlines(&t).len();
        }
    }
    let dated = pytext::re(r"\(20\d\d-\d\d-\d\d[^)]*(신설|개정|정정|명문화|이관|강등)");
    let set: BTreeSet<&String> = refs.iter().collect();
    Router::Obs {
        skill_md_lines: lines.len(),
        dated: lines.iter().filter(|l| pytext::search(&dated, l)).count(),
        ref_files: set.len(),
        ref_lines,
    }
}

pub struct Lint {
    pub dups: Vec<Dup>,
    pub sizes: Vec<(String, Option<u64>)>,
    pub total: u64,
    pub ghosts: Ghosts,
    pub router: Router,
}

pub fn lint(root: &str) -> Lint {
    let dups = duplicate_lines(root);
    let (sizes, total) = always_loaded_bytes(root);
    let ghosts = ghost_policy_ids(root);
    let router = skill_router_observation(root);
    Lint { dups, sizes, total, ghosts, router }
}

fn strs(v: &[String]) -> PyValue {
    PyValue::List(v.iter().map(|x| s(x)).collect())
}

pub fn to_value(r: &Lint) -> PyValue {
    let mut d = PyDict::new();
    d.set("schema_version", PyValue::int(1));
    d.set("verdict", s("INFO"));
    d.set(
        "duplicate_sentences",
        PyValue::List(
            r.dups
                .iter()
                .map(|x| {
                    let mut e = PyDict::new();
                    e.set("sentence", s(&x.sentence));
                    e.set("files", strs(&x.files));
                    PyValue::Dict(e)
                })
                .collect(),
        ),
    );
    let mut al = PyDict::new();
    let mut files = PyDict::new();
    for (k, v) in &r.sizes {
        files.set(k, v.map_or(PyValue::None, |n| PyValue::int(n as i64)));
    }
    al.set("files", PyValue::Dict(files));
    al.set("total_bytes", PyValue::int(r.total as i64));
    al.set("note", s("임계 없음 -- 숫자는 보여 주되 이 린트는 차단하지 않는다"));
    d.set("always_loaded", PyValue::Dict(al));
    d.set(
        "ghost_policy_ids",
        match &r.ghosts {
            Ghosts::Error(e) => {
                let mut g = PyDict::new();
                g.set("error", s(e));
                PyValue::List(vec![PyValue::Dict(g)])
            }
            Ghosts::Found(v) => PyValue::List(
                v.iter()
                    .map(|(k, files, n)| {
                        let mut g = PyDict::new();
                        g.set("policy_id", s(k));
                        g.set("referenced_in", strs(files));
                        g.set("count", PyValue::int(*n as i64));
                        PyValue::Dict(g)
                    })
                    .collect(),
            ),
        },
    );
    let mut sr = PyDict::new();
    match &r.router {
        Router::Error(e) => sr.set("error", s(e)),
        Router::Obs { skill_md_lines, dated, ref_files, ref_lines } => {
            sr.set("skill_md_lines", PyValue::int(*skill_md_lines as i64));
            sr.set("dated_narrative_lines", PyValue::int(*dated as i64));
            sr.set("references_files", PyValue::int(*ref_files as i64));
            sr.set("references_lines", PyValue::int(*ref_lines as i64));
            sr.set("note", s("관측만 -- 줄 수·서사 비율은 합격 조건이 아니다(plan_26091210 §3.8)"));
        }
    }
    d.set("skill_router", PyValue::Dict(sr));
    PyValue::Dict(d)
}

fn thousands(n: u64) -> String {
    let t = n.to_string();
    let mut out = String::new();
    for (i, c) in t.chars().enumerate() {
        if i > 0 && (t.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn render(r: &Lint) -> String {
    let mut out = vec!["[constitution_lint] 정보 -- 차단하지 않는다".to_string()];
    out.push(format!("  상시 로드 합계 {} bytes", thousands(r.total)));
    for (rel, size) in &r.sizes {
        match size {
            Some(n) => out.push(format!("    {:>9} {}", thousands(*n), rel)),
            None => out.push(format!("    {:>9} {}", "부재", rel)),
        }
    }
    let n = r.dups.len();
    out.push(format!("  파일 간 중복 문장 {}건{}", n, if n > 0 { " -- red flag: 같은 규칙이 두 층에 있다" } else { "" }));
    for d in r.dups.iter().take(20) {
        out.push(format!("    {}", d.files.join(" · ")));
        out.push(format!("      {}", d.sentence));
    }
    if n > 20 {
        out.push(format!("    ... 외 {}건(전수는 --format json)", n - 20));
    }
    if let Router::Obs { skill_md_lines, dated, ref_files, ref_lines } = &r.router {
        out.push(format!(
            "  terraforming_node 라우터(관측) SKILL.md {skill_md_lines}줄 · 날짜 개정 서사 {dated}줄 · references {ref_files}파일/{ref_lines}줄"
        ));
    }
    match &r.ghosts {
        Ghosts::Error(e) => {
            out.push("  유령 정책 ID 1건".into());
            out.push(format!("    {e} -- None"));
        }
        Ghosts::Found(v) => {
            out.push(format!("  유령 정책 ID {}건", v.len()));
            for (k, files, _) in v {
                out.push(format!("    {} -- {}", k, pytext::py_repr_list(files)));
            }
        }
    }
    out.join("\n")
}

fn fail(name: &str, got: &str) -> i32 {
    eprintln!("Traceback (most recent call last):\nRuntimeError: [constitution_lint] self-test FAIL: {name} (got={got})");
    1
}

pub fn self_test() -> i32 {
    let td = TempDir::new();
    let root = td.s();
    let init = gitx::run("git", &["init", "-q", "-b", "single-node", &root], None, None);
    if init.code != 0 {
        eprint!("{}", init.err_text());
        return 1;
    }
    let w = |rel: &str, text: &str| {
        let p = td.path().join(rel);
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        std::fs::write(p, text).expect("fixture");
    };
    w(".claude/policies/registry.yaml", r#"{"schema_version": 2, "policies": [{"policy_id": "REAL_ONE"}]}"#);
    let shared = "이 문장은 두 파일에 똑같이 적혀 있어 중복으로 잡혀야 하는 충분히 긴 문장이다.";
    w("CLAUDE.md", &format!("# c\n{shared}\npolicy:REAL_ONE 과 policy:GHOST_ONE 과 policy:REAL_ONE.C2 를 쓴다.\n"));
    w(".claude/rules/workflow.md", &format!("# w\n{shared}\n"));
    let _ = gitx::git(&root, &["add", "-A"]);
    let res = lint(&root);
    let dup_files: Vec<Vec<String>> = res.dups.iter().map(|d| d.files.clone()).collect();
    let want = vec![".claude/rules/workflow.md".to_string(), "CLAUDE.md".to_string()];
    if !dup_files.contains(&want) {
        return fail("양성: 파일 간 중복 문장 검출", &format!("{dup_files:?}"));
    }
    let ids: Vec<String> = match &res.ghosts {
        Ghosts::Found(v) => v.iter().map(|g| g.0.clone()).collect(),
        Ghosts::Error(_) => vec![],
    };
    if ids != ["GHOST_ONE"] {
        return fail("유령 ID 는 GHOST_ONE 하나 -- 절 접미사 인용은 위양성이 아니다", &format!("{ids:?}"));
    }
    if res.total == 0 {
        return fail("상시 로드 바이트 > 0", "''");
    }
    w(".claude/rules/workflow.md", "# w\n다른 내용이며 충분히 길지만 중복은 아닌 이 문장 하나만 남긴다.\n");
    if !lint(&root).dups.is_empty() {
        return fail("음성: 중복이 없으면 0건", "''");
    }
    eprintln!("[constitution_lint] self-test PASS");
    EXIT_OK
}

pub const USAGE: &str = "usage: constitution_lint.sh [-h] [--repo REPO] [--format {json,text}]
                            [--self-test]
";

pub fn main(argv: &[String]) -> i32 {
    use crate::argp::{self, Kind as K, Opt};
    let opts = [
        Opt { long: "--repo", kind: K::Value },
        Opt { long: "--format", kind: K::Choice(&["json", "text"]) },
        Opt { long: "--self-test", kind: K::Flag },
    ];
    let help = format!("{USAGE}{}", include_str!("constitution_lint_help.txt"));
    let a = match argp::parse(argv, &opts, "constitution_lint.sh", USAGE, &help) {
        Ok(a) => a,
        Err(e) => return argp::finish(e),
    };
    if a.flag("--self-test") {
        return self_test();
    }
    let repo = a.value("--repo").unwrap_or(".");
    let root = std::fs::canonicalize(repo).unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(repo));
    let root_s = root.to_string_lossy().into_owned();
    if !root.join("CLAUDE.md").is_file() {
        eprintln!("[constitution_lint] CLAUDE.md 가 없다: {root_s}");
        return EXIT_UNREADABLE;
    }
    let res = lint(&root_s);
    if a.value("--format") == Some("json") {
        let text = to_value(&res).dumps_indented(2).ok().and_then(|t| t.to_utf8()).unwrap_or_default();
        println!("{text}");
    } else {
        println!("{}", render(&res));
    }
    EXIT_OK
}
