//! Port of `.claude/policies/runtime/antipattern_scan.py`: harness antipattern inventory + removal tripwire +
//! public-surface tripwire. Messages, exit codes and the JSONL inventory are the original's.
use crate::gitx;
use crate::pyast::{self, Const, Kind};
use crate::pytext::{self, py_splitlines, py_strip, take_chars};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{float_repr, PyDict, PyValue};
use fancy_regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

pub const SELF_REL: &str = ".claude/policies/runtime/antipattern_scan.py";
pub const RC_OK: i32 = 0;
pub const RC_ARGS: i32 = 2;
pub const RC_REVIVED: i32 = 3;

pub struct Shape {
    pub id: &'static str,
    pub path_re: &'static str,
    pub pattern: &'static str,
    pub why: &'static str,
}

pub const REMOVED_SHAPES: [Shape; 9] = [
    Shape {
        id: "G-A1-sonnet-gate",
        path_re: r"^\.claude/skills/terraforming_node/scripts/(agent_control|providers/[a-z_]+)\.py$",
        pattern: r#"request\[["']model["']\]\s*!=\s*["']sonnet["']|REQUESTED_MODEL_NOT_SONNET"#,
        why: "모델 게이트 — 모델은 요청의 선언이고 실행 모델은 기록한다(G-A1)",
    },
    Shape {
        id: "G-A2-grade-table",
        path_re: concat!(
            r"^\.claude/skills/terraforming_node/scripts/(turn_budget|relay|bootstrap_canary)\.py$",
            r"|^\.claude/rust/role-contract/src/turn_budget\.rs$",
            r"|^\.claude/skills/adversarial-benchmark/rust/src/(bin/)?turn_budget[^/]*\.rs$"
        ),
        pattern: r"^GRADES\s*=|MAX_ATTEMPTS_BEFORE_HITL|ESCALATION_FACTOR|GRADE_MAX_TURNS",
        why: "턴 예산 등급표 — 예산은 선언이다(G-A2)",
    },
    Shape {
        id: "G-C3-bench-drift-gate",
        path_re: concat!(
            r"^\.claude/skills/adversarial-benchmark/scripts/resolve_bench_tool\.(py|sh)$",
            r"|^\.claude/skills/adversarial-benchmark/rust/src/(bin/)?(bench_tool|resolve_bench_tool)[^/]*\.rs$"
        ),
        pattern: r"RC_DRIFT",
        why: "벤치 도구 digest 드리프트 게이트 — digest 는 provenance 다(G-C3)",
    },
    Shape {
        id: "G-B1-overhead-default",
        path_re: r"^\.claude/(skills/.*/scripts/.*|policies/.*)\.(py|sh)$",
        pattern: r#"(overhead[_-]?mib\W{0,12}|OVERHEAD_MIB["']?\s*[:=-]{1,2}\s*)12288|:-12288"#,
        why: "예산 overhead 기본값 12288 — 선언 필수(G-B1 · 정상 로드 사살 실적)",
    },
    Shape {
        id: "G-B2-ttl-mirror",
        path_re: concat!(
            r"^\.claude/(skills/upstream-version-watch/scripts/(single_serve_up|multinode_serve_smoke)\.sh",
            r"|skills/vllm-recipe-explorer/scripts/run_trial\.py",
            r"|skills/terraforming_node/scripts/node_blackbox/budget_renew_loop\.sh)$"
        ),
        pattern: r"(TTL_S\s*=\s*7200|:-7200|BUDGET_TTL_MIN_S\s*=\s*7200)",
        why: "예산 TTL 손기재 — 단일 소유자(blackbox_session budget-defaults)에게 묻는다(G-B2)",
    },
    Shape {
        id: "G-B3-decl-const-mirror",
        path_re: r"^\.claude/skills/(terraforming_node/scripts/node_blackbox/.*|upstream-version-watch/scripts/multinode_serve_smoke\.sh)$",
        pattern: concat!(
            r#"(BB_DECL_(MARGIN|MIN_CEILING)_MIB="?\$\{BB_DECL_[A-Z_]+:-[0-9]"#,
            r"|_(WD|DECL)_FALLBACK\s*=\s*\{",
            r"|_WD_MARGIN=[0-9]|_WD_MIN_CEIL=[0-9])"
        ),
        why: "선언 상수 거울 — 정본은 blackbox_eta.DEFAULTS / eta_params.env 하나다(G-B3)",
    },
    Shape {
        id: "G-B6-ready-max-default",
        path_re: r"^\.claude/skills/upstream-version-watch/scripts/multinode_serve_smoke\.sh$",
        pattern: r"READY_MAX:-[0-9]",
        why: "READY_MAX 기본값 — 로드 시간은 모델·HW 의 함수라 선언한다(G-B6)",
    },
    Shape {
        id: "G-B8-topology-default",
        path_re: concat!(
            r"^\.claude/skills/(adversarial-benchmark/scripts/lite_metrics\.py",
            r"|terraforming_node/scripts/library_access\.py)$"
        ),
        pattern: r#"get\(\s*["']topology["']\s*,\s*["']single["']|get\(\s*["']role["']\s*,\s*["']main["']"#,
        why: "토폴로지·역할 기본값 — 인터뷰/manifest 만이 정한다(G-B8 · 헌법)",
    },
    Shape {
        id: "G-C1-worktree-drift-gate",
        path_re: r"^\.claude/policies/runtime/policy_registry\.py$",
        pattern: r"worktree_drift|_git_worktree_blob_sha",
        why: "정책 증거의 워크트리 대조 — 드리프트는 git status 가 답한다(G-C1)",
    },
];

struct Rx {
    shapes: Vec<(Regex, Regex)>,
    hard: Vec<(Regex, &'static str)>,
    hard_ascii: Vec<regex_automata::meta::Regex>,
    sh_default: Regex,
    exempt: Regex,
    surf_ref: Regex,
    surf_join: Regex,
    surf_token: Regex,
    quoted: Regex,
}

fn rx() -> &'static Rx {
    static R: OnceLock<Rx> = OnceLock::new();
    R.get_or_init(|| Rx {
        shapes: REMOVED_SHAPES.iter().map(|s| (pytext::re_match(s.path_re), pytext::re(s.pattern))).collect(),
        hard: vec![
            (pytext::re(r"(?<![\w.])\d+\.\d+\.\d+(?![\w.])"), "version"),
            (pytext::re(r"(?i)(easy-vllm:|nvcr\.io|ghcr\.io|docker\.io|guidellm)"), "image"),
            (pytext::re(r"(?i)\b(GB10|sm_?121a?|aarch64|dgx-spark|x86_64|cu13\d|cu12\d)\b"), "hw"),
            (pytext::re(r"(?i)\b(sonnet|opus|haiku|claude-[a-z0-9.-]+)\b"), "model"),
            (pytext::re(r"(?i)(\bmock\b|dry.?run|\bfake\b|\bstub\b|TEST_OVERRIDE|_OVERRIDE\b|DELEGATED=1)"), "mock"),
            (pytext::re(r"(?i)(hashlib|sha256|sha1\b|md5|\bdigest\b|blake2)"), "hash"),
            (pytext::re(r"(?i)(max_turns|MIN_TURNS|turn_budget|timeout_seconds|grade)"), "turn"),
        ],
        // the same patterns for pure-ASCII lines: there Unicode and ASCII word boundaries/digits coincide, and the
        // ASCII form runs on the DFA instead of the Unicode-boundary fallback engine
        hard_ascii: [
            r"[0-9]+\.[0-9]+\.[0-9]+", // unused: the version pattern is version_search()
            r"(?i)(easy-vllm:|nvcr\.io|ghcr\.io|docker\.io|guidellm)",
            r"(?i)\b(GB10|sm_?121a?|aarch64|dgx-spark|x86_64|cu13\d|cu12\d)\b",
            r"(?i)\b(sonnet|opus|haiku|claude-[a-z0-9.-]+)\b",
            r"(?i)(\bmock\b|dry.?run|\bfake\b|\bstub\b|TEST_OVERRIDE|_OVERRIDE\b|DELEGATED=1)",
            r"(?i)(hashlib|sha256|sha1\b|md5|\bdigest\b|blake2)",
            r"(?i)(max_turns|MIN_TURNS|turn_budget|timeout_seconds|grade)",
        ]
        .iter()
        .map(|p| regex_automata::meta::Regex::new(&p.replace(r"\b", r"(?-u:\b)").replace(r"\d", "[0-9]")).expect("ascii pattern"))
        .collect(),
        sh_default: pytext::re(r":-\s*([0-9]+)\}"),
        exempt: pytext::re(r"antipattern-ok:\s*([A-Za-z0-9-]+)\s*[—:-]\s*(\S.*)"),
        surf_ref: pytext::re(r"terraforming_node/scripts/([\w./\-]+)"),
        surf_join: pytext::re(r#"["']terraforming_node["']\s*[,/]\s*["']scripts["']((?:\s*[,/]\s*["'][\w.\-]+["'])+)"#),
        surf_token: pytext::re(r"`([^`]+)`"),
        quoted: pytext::re(r#"["']([\w.\-]+)["']"#),
    })
}

/// The `TRIVIAL` set membership of the original (`v not in TRIVIAL` with Python numeric equality).
fn trivial(c: &Const) -> bool {
    const INTS: [i64; 18] = [0, 1, 2, -1, 10, 100, 1000, 1024, 3, 4, 8, 16, 32, 64, 60, 0, 1, 0];
    match c {
        Const::Int(v) => INTS.iter().any(|i| num_bigint::BigInt::from(*i) == *v),
        Const::Float(f) => *f == 0.5 || INTS.iter().any(|i| *i as f64 == *f),
        _ => false,
    }
}

pub type Hit = Vec<(&'static str, PyValue)>;

fn s(v: &str) -> PyValue {
    PyValue::Str(PyStr::new(v))
}

fn get<'a>(hit: &'a Hit, key: &str) -> Option<&'a PyValue> {
    hit.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

fn get_str(hit: &Hit, key: &str) -> String {
    match get(hit, key) {
        Some(PyValue::Str(t)) => t.to_utf8().unwrap_or_default(),
        _ => String::new(),
    }
}

/// Raised where the original would raise (a constant json.dumps cannot write).
#[derive(Debug)]
pub struct Crash(pub String);

fn const_value(c: &Const) -> Result<PyValue, Crash> {
    Ok(match c {
        Const::None => PyValue::None,
        Const::Bool(b) => PyValue::Bool(*b),
        Const::Int(v) => PyValue::Int(v.clone()),
        Const::Float(f) => PyValue::Float(*f),
        Const::Str(t) => s(t),
        // a value json.dumps cannot write: kept as a marker carrying its Python type name; writing it raises
        Const::Bytes(_) => PyValue::Bytes(b"bytes".to_vec()),
        Const::Complex => PyValue::Bytes(b"complex".to_vec()),
        Const::Ellipsis => PyValue::Bytes(b"ellipsis".to_vec()),
    })
}

/// The `TypeError` json.dumps raises for a hit holding an unserializable constant.
pub fn unserializable(h: &Hit) -> Option<String> {
    h.iter().find_map(|(_, v)| match v {
        PyValue::Bytes(t) => Some(format!("TypeError: Object of type {} is not JSON serializable", String::from_utf8_lossy(t))),
        _ => None,
    })
}

pub fn harness_files(root: &str) -> Result<Vec<String>, String> {
    let out = gitx::git(root, &["ls-files", "--", ".claude", "CLAUDE.md"]);
    if out.code != 0 {
        return Err(format!("[antipattern_scan] FAIL: git ls-files 실패 — {}", take_chars(py_strip(&out.err_text()), 200)));
    }
    Ok(py_splitlines(&out.text())
        .into_iter()
        .filter(|p| p.ends_with(".py") || p.ends_with(".sh") || p.ends_with(".rs") || p.ends_with("/pre-commit"))
        .map(String::from)
        .collect())
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub fn disposition(hit: &Hit) -> &'static str {
    let path = get_str(hit, "file");
    let src = get_str(hit, "src").to_lowercase();
    if path.contains("/fixtures/") || basename(&path).to_lowercase().contains("test") {
        return "fixture";
    }
    if ["self_test", "_self_test", "selftest", "self-test", "fixture", "픽스처"].iter().any(|t| src.contains(t)) {
        return "fixture";
    }
    const LOUD: [&str; 14] =
        ["print", "log", "warn", "error", "fail", "die", "exit", "diag", "raise", "abort", "stderr", "write", "emit", "report"];
    if LOUD.iter().any(|n| src.contains(n)) {
        return "diagnostic";
    }
    if matches!(get(hit, "cat"), Some(PyValue::Str(t)) if t.equals("MAGIC_NUM_CONST"))
        && matches!(get(hit, "scope"), Some(PyValue::Str(t)) if t.equals("module"))
    {
        return "local-const";
    }
    "review"
}

/// `VERSION_RE.search(line)` for `(?<![\w.])\d+\.\d+\.\d+(?![\w.])` without a backtracking engine: a match can only
/// start at the first digit of a digit run (the char before a digit inside a run is `\w`) and every `\d+` must be
/// maximal (a shorter run is followed by a digit, which is neither `.` nor allowed by the lookahead).
pub fn version_search(line: &str) -> bool {
    use agent_runtime_support::pystr::{is_decimal_digit, is_python_word};
    let c: Vec<u32> = line.chars().map(|x| x as u32).collect();
    let dot = '.' as u32;
    let word_or_dot = |v: u32| v == dot || is_python_word(v);
    let mut i = 0;
    while i < c.len() {
        if is_decimal_digit(c[i]) && (i == 0 || !word_or_dot(c[i - 1])) {
            let mut j = i;
            let mut ok = true;
            for part in 0..3 {
                let st = j;
                while j < c.len() && is_decimal_digit(c[j]) {
                    j += 1;
                }
                if j == st {
                    ok = false;
                    break;
                }
                if part < 2 {
                    if j < c.len() && c[j] == dot {
                        j += 1;
                    } else {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && (j >= c.len() || !word_or_dot(c[j])) {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// `rx.search(line)` for one of the seven HARDCODE_STR patterns.
fn hard_search(r: &Regex, sub: &str, line: &str) -> bool {
    if sub == "version" {
        return version_search(line);
    }
    if line.is_ascii() {
        let k = rx().hard.iter().position(|(_, s)| *s == sub).unwrap();
        return rx().hard_ascii[k].is_match(line.as_bytes());
    }
    pytext::search(r, line)
}

fn hardcode_hits(rel: &str, lines: &[&str], hits: &mut Vec<Hit>) {
    for (i, line) in lines.iter().enumerate() {
        for (r, sub) in &rx().hard {
            let found = hard_search(r, sub, line);
            if found {
                hits.push(vec![
                    ("file", s(rel)),
                    ("line", PyValue::int(i as i64 + 1)),
                    ("cat", s("HARDCODE_STR")),
                    ("sub", s(sub)),
                    ("src", s(&take_chars(py_strip(line), 200))),
                ]);
            }
        }
    }
}

pub fn scan_py(rel: &str, text: &str) -> Result<Vec<Hit>, Crash> {
    let mut hits = Vec::new();
    let tree = match pyast::parse(text) {
        Ok(t) => t,
        Err(_) => return Ok(hits),
    };
    let lines = py_splitlines(text);
    let src = |i: usize| -> String {
        if i > 0 && i <= lines.len() {
            take_chars(py_strip(lines[i - 1]), 200)
        } else {
            String::new()
        }
    };
    for node in pyast::walk(&tree) {
        if node.kind == Kind::Assign {
            let value = &node.children[node.count];
            if value.kind == Kind::Constant {
                let c = value.constant.as_ref().unwrap();
                if matches!(c, Const::Int(_) | Const::Float(_)) && !trivial(c) {
                    for t in &node.children[..node.count] {
                        if t.kind == Kind::Name {
                            hits.push(vec![
                                ("file", s(rel)),
                                ("line", PyValue::int(node.line as i64)),
                                ("cat", s("MAGIC_NUM_CONST")),
                                ("sub", s("module")),
                                ("name", s(&t.name)),
                                ("value", const_value(c)?),
                                ("src", s(&src(node.line))),
                                ("scope", s("module")),
                            ]);
                        }
                    }
                }
            }
        }
        if node.kind == Kind::Call {
            let func = &node.children[0];
            let fname = match func.kind {
                Kind::Attribute | Kind::Name if !func.name.is_empty() => func.name.as_str(),
                _ => "",
            };
            if (fname == "get" || fname == "getenv") && node.count == 2 && node.children[2].kind == Kind::Constant {
                let c = node.children[2].constant.as_ref().unwrap();
                hits.push(vec![
                    ("file", s(rel)),
                    ("line", PyValue::int(node.line as i64)),
                    ("cat", s("DEFAULT_FALLBACK")),
                    ("sub", s(fname)),
                    ("value", const_value(c)?),
                    ("src", s(&src(node.line))),
                ]);
            }
        }
    }
    hardcode_hits(rel, &lines, &mut hits);
    Ok(hits)
}

pub fn scan_sh(rel: &str, text: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (i, line) in py_splitlines(text).iter().enumerate() {
        for (r, sub) in &rx().hard {
            if hard_search(r, sub, line) {
                hits.push(vec![
                    ("file", s(rel)),
                    ("line", PyValue::int(i as i64 + 1)),
                    ("cat", s("HARDCODE_STR")),
                    ("sub", s(sub)),
                    ("src", s(&take_chars(py_strip(line), 200))),
                ]);
            }
        }
        if let Ok(Some(m)) = rx().sh_default.captures(line) {
            let digits = m.get(1).unwrap().as_str();
            let v: num_bigint::BigInt = digits.parse().unwrap();
            if !trivial(&Const::Int(v.clone())) {
                hits.push(vec![
                    ("file", s(rel)),
                    ("line", PyValue::int(i as i64 + 1)),
                    ("cat", s("DEFAULT_FALLBACK")),
                    ("sub", s("shell-default")),
                    ("value", PyValue::Int(v)),
                    ("src", s(&take_chars(py_strip(line), 200))),
                ]);
            }
        }
    }
    hits
}

fn read_replace(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

pub fn inventory(root: &str, files: Option<Vec<String>>) -> Result<Vec<Hit>, String> {
    let files = match files {
        Some(f) if !f.is_empty() => f,
        _ => harness_files(root)?,
    };
    let mut hits = Vec::new();
    for rel in &files {
        let full = Path::new(root).join(rel);
        if !full.is_file() {
            continue;
        }
        let Some(text) = read_replace(&full) else { continue };
        if rel.ends_with(".py") {
            hits.extend(scan_py(rel, &text).map_err(|c| c.0)?);
        } else {
            hits.extend(scan_sh(rel, &text));
        }
    }
    for h in hits.iter_mut() {
        let d = disposition(h);
        h.push(("disposition", s(d)));
    }
    Ok(hits)
}

pub fn staged_files(root: &str) -> Vec<String> {
    let out = gitx::git(root, &["diff", "--cached", "--name-only", "--diff-filter=ACMR"]);
    if out.code != 0 {
        return Vec::new();
    }
    py_splitlines(&out.text())
        .into_iter()
        .filter(|p| p.ends_with(".py") || p.ends_with(".sh") || p.ends_with(".rs"))
        .map(String::from)
        .collect()
}

/// Code lines without comments and docstrings (the original's line heuristic).
pub fn code_lines(text: &str, is_py: bool) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut in_doc: Option<&str> = None;
    for (i, line) in py_splitlines(text).iter().enumerate() {
        let stripped = py_strip(line);
        if is_py {
            if let Some(q) = in_doc {
                if line.contains(q) {
                    in_doc = None;
                }
                continue;
            }
            if !stripped.starts_with('#') {
                let mut opened = None;
                for q in ["\"\"\"", "'''"] {
                    let cnt = line.matches(q).count();
                    if cnt > 0 && cnt % 2 == 1 {
                        opened = Some(q);
                        break;
                    }
                }
                if opened.is_some() {
                    in_doc = opened;
                    continue;
                }
            }
        }
        if stripped.starts_with('#') {
            continue;
        }
        out.push((i + 1, line.to_string()));
    }
    out
}

pub struct Violation {
    pub shape: &'static str,
    pub file: String,
    pub line: usize,
    pub why: &'static str,
    pub src: String,
}

pub fn tripwire(root: &str, files: Option<Vec<String>>) -> Vec<Violation> {
    let files = match files {
        Some(f) => f,
        None => staged_files(root),
    };
    let mut v = Vec::new();
    for rel in &files {
        if pytext::normpath(rel) == SELF_REL {
            continue;
        }
        let full = Path::new(root).join(rel);
        if !full.is_file() {
            continue;
        }
        let Some(text) = read_replace(&full) else { continue };
        for (k, shape) in REMOVED_SHAPES.iter().enumerate() {
            let (path_re, pat) = &rx().shapes[k];
            if !pytext::search(path_re, rel) {
                continue;
            }
            for (i, line) in code_lines(&text, rel.ends_with(".py")) {
                if !pytext::search(pat, &line) {
                    continue;
                }
                if let Ok(Some(ex)) = rx().exempt.captures(&line) {
                    if ex.get(1).map(|m| m.as_str()) == Some(shape.id) {
                        continue;
                    }
                }
                v.push(Violation {
                    shape: shape.id,
                    file: rel.clone(),
                    line: i,
                    why: shape.why,
                    src: take_chars(py_strip(&line), 160),
                });
            }
        }
    }
    v
}

pub const SURFACE_SKILL: &str = ".claude/skills/terraforming_node/";
pub const SURFACE_HEADING: &str = "## 4. 공개 표면";

pub fn surface_entries(skill_text: &str) -> Option<Vec<String>> {
    if !skill_text.contains(SURFACE_HEADING) {
        return None;
    }
    let after = skill_text.splitn(2, SURFACE_HEADING).nth(1).unwrap_or("");
    let sect = after.splitn(2, "\n## ").next().unwrap_or("");
    let mut out: Vec<String> = Vec::new();
    for row in py_splitlines(sect) {
        if !row.starts_with("| ") {
            continue;
        }
        let cell = row.split('|').nth(1).unwrap_or("");
        for tok in pytext::findall(&rx().surf_token, cell) {
            let key = format!("{SURFACE_SKILL}scripts/");
            // `tok.split(KEY, 1)[-1]`: the part after the first occurrence
            let mut tok = match tok.find(&key) {
                Some(p) => tok[p + key.len()..].to_string(),
                None => tok,
            };
            if let Some(rest) = tok.strip_prefix("scripts/") {
                tok = rest.to_string();
            }
            if !tok.is_empty() && (tok.contains('/') || tok.ends_with(".py") || tok.ends_with(".sh")) && !tok.contains(' ') {
                if !out.contains(&tok) {
                    out.push(tok);
                }
            }
        }
    }
    Some(out)
}

pub fn surface_covered(r: &str, entries: &[String]) -> bool {
    for e in entries {
        if e.ends_with('/') {
            if r == e.trim_end_matches('/') || r.starts_with(e.as_str()) {
                return true;
            }
        } else if r == e || e.starts_with(r) {
            return true;
        }
    }
    false
}

pub fn public_surface_violations(root: &str) -> Vec<String> {
    let skill = Path::new(root).join(SURFACE_SKILL).join("SKILL.md");
    let entries = match std::fs::read(&skill) {
        Ok(b) => match String::from_utf8(b) {
            Ok(t) => surface_entries(&t),
            // UnicodeDecodeError is not an OSError: the original crashes
            Err(_) => panic!("UnicodeDecodeError reading {}", skill.display()),
        },
        Err(_) => return vec![format!("{SURFACE_SKILL}SKILL.md 부재 — 공개 표면 표를 읽을 수 없다")],
    };
    let entries = match entries {
        Some(e) if !e.is_empty() => e,
        _ => return vec![format!("SKILL.md 에 '{SURFACE_HEADING}' 표가 없다(또는 비었다) — 외부 호출의 등재처가 사라졌다")],
    };
    let out = gitx::git(root, &["ls-files", "-z"]);
    let mut bad: BTreeMap<String, String> = BTreeMap::new();
    for rel in gitx::split_z(&out.stdout) {
        if rel.starts_with(SURFACE_SKILL) || rel.starts_with("docs/") || rel.starts_with("seed/") {
            continue;
        }
        // parity golden data = captured bytes of a program's output/input trees (e.g. a rendered Sub CLAUDE.md), not
        // callers: like docs/ and seed/ they are an inert plane (plan_26111615 · TB-09 first carried such a copy).
        if rel.starts_with(".claude/rust/parity/data/") {
            continue;
        }
        if rel == ".claude/policies/branch_layer_ledger.json" {
            continue;
        }
        let Ok(bytes) = std::fs::read(Path::new(root).join(&rel)) else { continue };
        let Ok(text) = String::from_utf8(bytes) else { continue };
        if !text.contains("terraforming_node") {
            continue;
        }
        for line in py_splitlines(&text) {
            if !line.contains("terraforming_node") {
                continue;
            }
            let mut refs: Vec<String> = pytext::findall(&rx().surf_ref, line)
                .into_iter()
                .map(|r| r.trim_end_matches(|c| ".,/:)`'\"".contains(c)).to_string())
                .collect();
            for g in pytext::findall(&rx().surf_join, line) {
                refs.push(pytext::findall(&rx().quoted, &g).join("/"));
            }
            for r in refs {
                if !r.is_empty() && !surface_covered(&r, &entries) {
                    bad.entry(r).or_insert_with(|| rel.clone());
                }
            }
        }
    }
    bad.into_iter()
        .map(|(r, f)| format!("공개 표면 밖 외부 호출: scripts/{r} (첫 호출자 {f}) — SKILL.md §4 표에 등재하거나 호출을 없앤다"))
        .collect()
}

pub fn dumps_hit(h: &Hit) -> String {
    let mut d = PyDict::new();
    for (k, v) in h {
        d.set(k, v.clone());
    }
    PyValue::Dict(d).dumps_unicode().ok().and_then(|t| t.to_utf8()).unwrap_or_default()
}

#[allow(dead_code)]
fn _float_repr_used(f: f64) -> String {
    float_repr(f)
}

pub use crate::tempdir::TempDir;

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(p, text).expect("write fixture");
}

fn shapes(v: &[Violation]) -> Vec<&'static str> {
    v.iter().map(|x| x.shape).collect()
}

/// `--self-test` (T1..T15 of the original, same names and output).
pub fn self_test(repo: &str) -> i32 {
    let mut failures: Vec<&str> = Vec::new();
    let mut check = |name: &'static str, cond: bool, detail: String| {
        println!("  [{}] {} {}", if cond { "PASS" } else { "FAIL" }, name, detail);
        if !cond {
            failures.push(name);
        }
    };
    {
        let d = TempDir::new();
        let ds = d.s();
        let rel = ".claude/skills/terraforming_node/scripts/turn_budget.py";
        write(d.path(), rel, "GRADES = {'S': {'max_turns': 10}}\n");
        let v = tripwire(&ds, Some(vec![rel.into()]));
        check("T1 등급표 부활을 잡는다", v.len() == 1 && v[0].shape == "G-A2-grade-table", String::new());
        write(d.path(), rel, "# GRADES 표는 2026-09-05 에 삭제됐다(G-A2)\n");
        check("T2 주석의 서술은 잡지 않는다(제거 사실을 적을 수 있어야 한다)", tripwire(&ds, Some(vec![rel.into()])).is_empty(), String::new());
        let rel2 = ".claude/skills/terraforming_node/scripts/providers/claude_code.py";
        write(d.path(), rel2, "if request[\"model\"] != \"sonnet\":\n    pass\n");
        check("T3 모델 게이트 부활을 잡는다", shapes(&tripwire(&ds, Some(vec![rel2.into()]))) == ["G-A1-sonnet-gate"], String::new());
        let rel3 = ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh";
        write(d.path(), rel3, "READY_MAX=\"${READY_MAX:-180}\"\n");
        check("T4 READY_MAX 기본값 부활을 잡는다", shapes(&tripwire(&ds, Some(vec![rel3.into()]))) == ["G-B6-ready-max-default"], String::new());
        let rel4 = ".claude/policies/runtime/doc_naming.py";
        write(d.path(), rel4, "GRADES = {\"a\": 1}\n");
        check("T5 무관 경로의 같은 문자열은 잡지 않는다", tripwire(&ds, Some(vec![rel4.into()])).is_empty(), String::new());
        let rust_rel = ".claude/skills/adversarial-benchmark/rust/src/bench_tool_pin.rs";
        write(d.path(), rust_rel, "pub const RC_DRIFT: i32 = 4;\n");
        check("T13 Rust resolver drift gate revival is rejected", shapes(&tripwire(&ds, Some(vec![rust_rel.into()]))) == ["G-C3-bench-drift-gate"], String::new());
        let other_rs = ".claude/skills/adversarial-benchmark/rust/src/unrelated.rs";
        write(d.path(), other_rs, "pub const RC_DRIFT: i32 = 4;\n");
        check("T14 unrelated Rust path is outside the resolver guard", tripwire(&ds, Some(vec![other_rs.into()])).is_empty(), String::new());
        let init = gitx::run("git", &["init", "-q", &ds], None, None);
        let add = gitx::git(&ds, &["add", "--", rust_rel, other_rs]);
        if init.code != 0 || add.code != 0 {
            eprintln!("subprocess.CalledProcessError: git init/add failed in self-test fixture");
            return 1;
        }
        let hf = harness_files(&ds).unwrap_or_default();
        check(
            "T15 Git discovery includes the staged Rust resolver",
            staged_files(&ds).iter().any(|x| x == rust_rel) && hf.iter().any(|x| x == rust_rel),
            String::new(),
        );
        let hits = inventory(&ds, Some(vec![rel3.into(), rel4.into()])).unwrap_or_default();
        check("T6 인벤토리 모든 행에 disposition 이 있다", !hits.is_empty() && hits.iter().all(|h| !get_str(h, "disposition").is_empty()), String::new());
        check(
            "T7 처분 어휘가 닫혀 있다",
            hits.iter().all(|h| ["fixture", "diagnostic", "local-const", "review"].contains(&get_str(h, "disposition").as_str())),
            String::new(),
        );
    }
    check("T7b 스캐너 자신은 대상에서 빠진다(형태의 정의 ≠ 형태의 사례)", tripwire(repo, Some(vec![SELF_REL.into()])).is_empty(), String::new());
    let live = match harness_files(repo) {
        Ok(f) => tripwire(repo, Some(f)),
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let live_shapes: Vec<String> = live.iter().take(5).map(|v| v.shape.to_string()).collect();
    check("T8 저장소 현재 상태가 제거 tripwire 를 통과한다", live.is_empty(), format!("(위반 {})", pytext::py_repr_list(&live_shapes)));
    let tbl = "x\n## 4. 공개 표면 — 전수\n\n| 진입점 | 소유 |\n|---|---|\n| `scripts/a.py` | §1 |\n| `scripts/sub/` | §2 |\n\n## 5. 금지\n";
    let ents = surface_entries(tbl).unwrap_or_default();
    let mut sorted = ents.clone();
    sorted.sort();
    // a Python set repr: element order is hash-seed dependent in the original; written sorted here
    let set_repr = format!("{{{}}}", sorted.iter().map(|x| pytext::py_repr_str(x)).collect::<Vec<_>>().join(", "));
    check("T9 공개 표면 표를 읽는다(파일·디렉터리 항목)", sorted == ["a.py", "sub/"], set_repr);
    check(
        "T10 표 안 호출은 통과 · 디렉터리 항목은 하위를 덮는다 · 정규식 리터럴 접두도 덮는다",
        surface_covered("a.py", &ents) && surface_covered("sub/x.sh", &ents) && surface_covered("a", &ents),
        String::new(),
    );
    check("T11 표 밖 호출은 잡는다", !surface_covered("b.py", &ents), String::new());
    let live_surf = public_surface_violations(repo);
    let first3: Vec<String> = live_surf.iter().take(3).cloned().collect();
    check("T12 저장소 현재 상태가 공개 표면 tripwire 를 통과한다", live_surf.is_empty(), pytext::py_repr_list(&first3));
    if failures.is_empty() {
        println!("[antipattern_scan --self-test] OK — T1~T15 전부 통과");
        0
    } else {
        let f: Vec<String> = failures.iter().map(|s| s.to_string()).collect();
        println!("[antipattern_scan --self-test] FAIL {}", pytext::py_repr_list(&f));
        1
    }
}

pub const USAGE: &str = "usage: antipattern_scan.py [-h] [--root ROOT] [--inventory OUT.jsonl]
                           [--tripwire] [--files [FILES ...]] [--self-test]
";
pub const HELP_TAIL: &str = "
하네스 안티패턴 인벤토리 + 제거 tripwire

options:
  -h, --help            show this help message and exit
  --root ROOT
  --inventory OUT.jsonl
  --tripwire
  --files [FILES ...]
  --self-test
";

/// The `--tripwire` branch of `main()`: (exit code, stderr text).
pub fn tripwire_run(root: &str, files: Option<Vec<String>>) -> (i32, String) {
    let mut err = String::new();
    let surf = public_surface_violations(root);
    for m in &surf {
        err.push_str(&format!("[antipattern_scan] SURFACE {m}\n"));
    }
    if !surf.is_empty() {
        return (RC_REVIVED, err);
    }
    // `--files` with no values is `[]` (scan nothing), absent is None (staged files)
    let v = tripwire(root, files);
    for x in &v {
        err.push_str(&format!("[antipattern_scan] REVIVED {} — {}:{}\n    {}\n    사유: {}\n", x.shape, x.file, x.line, x.src, x.why));
    }
    if !v.is_empty() {
        err.push_str(&format!("[antipattern_scan] ③ 에서 제거한 형태가 {}건 되돌아왔다 — 우회하지 말고 고쳐라(D3).\n", v.len()));
        return (RC_REVIVED, err);
    }
    (RC_OK, err)
}

/// `antipattern_scan.py --root <root> --tripwire` as tripwire ⑥ runs it.
pub fn tripwire_cli(root: &str) -> (i32, String) {
    tripwire_run(root, None)
}

/// `main()` of the original; returns the exit code.
pub fn main(argv: &[String], repo: &str) -> i32 {
    use crate::argp::{self, Kind as K, Opt};
    let opts = [
        Opt { long: "--root", kind: K::Value },
        Opt { long: "--inventory", kind: K::Value },
        Opt { long: "--tripwire", kind: K::Flag },
        Opt { long: "--files", kind: K::Star },
        Opt { long: "--self-test", kind: K::Flag },
    ];
    let help = format!("{USAGE}{HELP_TAIL}");
    let a = match argp::parse(argv, &opts, "antipattern_scan.py", USAGE, &help) {
        Ok(a) => a,
        Err(e) => return argp::finish(e),
    };
    let root = a.value("--root").unwrap_or(".").to_string();
    let files = a.list("--files").cloned();
    if a.flag("--self-test") {
        return self_test(repo);
    }
    if a.flag("--tripwire") {
        let (rc, err) = tripwire_run(&root, files);
        eprint!("{err}");
        return rc;
    }
    if let Some(out_path) = a.value("--inventory") {
        let hits = match inventory(&root, files) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
        let mut body = String::new();
        let mut crash = None;
        for h in &hits {
            if let Some(e) = unserializable(h) {
                crash = Some(e);
                break;
            }
            body.push_str(&dumps_hit(h));
            body.push('\n');
        }
        // the original opens the file first and writes row by row: a crash leaves the rows before it
        if let Err(e) = std::fs::write(out_path, body) {
            eprintln!("OSError: {e}: '{out_path}'");
            return 1;
        }
        if let Some(e) = crash {
            eprintln!("Traceback (most recent call last):\n  File \"antipattern_scan.py\", line 461, in main\n{e}");
            return 1;
        }
        let mut c: BTreeMap<String, usize> = BTreeMap::new();
        let mut byfile: Vec<String> = Vec::new();
        for h in &hits {
            *c.entry(get_str(h, "disposition")).or_default() += 1;
            let f = get_str(h, "file");
            if !byfile.contains(&f) {
                byfile.push(f);
            }
        }
        println!("hits={} files={} → {}", hits.len(), byfile.len(), out_path);
        for (k, n) in &c {
            println!("  {n:>6}  {k}");
        }
        return RC_OK;
    }
    print!("{help}");
    RC_ARGS
}
