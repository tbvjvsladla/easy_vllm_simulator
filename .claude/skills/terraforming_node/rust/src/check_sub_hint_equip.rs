//! Port of `check_sub_hint_equip.py` — the single sub's hint-publish equipment, judged and equipped on THAT node
//! (hint clone outside the workspace git, `hints/.push_authority`, `GITHUB_TOKEN` presence). Same command line, exit codes
//! (0 ready/equipped · 2 usage · 3 not ready · 4 equip failed), JSON (insertion order) and self-test lines. Secrets are never
//! printed. The self-test's cross-check with the hint publisher's token rule reads its Rust owner `hint-publisher/rust/src/tag.rs` as text (it is not run).

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

const ENV_FILE_REL: &str = "envs/.env";
const CLONE_DIR_KEY: &str = "HINT_CLONE_DIR";
const TOKEN_KEY: &str = "GITHUB_TOKEN";
const PUSH_FLAG_REL: &str = "hints/.push_authority";
const CENTRAL_FLAG_REL: &str = "hints/.central_authority";
const PII_TERMS_REL: &str = ".claude/pii_terms.txt";
const HINT_BRANCH: &str = "hint";
const REQUEST_TEMPLATE_REL: &str = ".claude/skills/terraforming_node/templates/requests/sub_hint_push_equip.md";
const EXIT_READY: i32 = 0;
const EXIT_USAGE: i32 = 2;
const EXIT_NOT_READY: i32 = 3;
const EXIT_EQUIP_FAILED: i32 = 4;

enum Fail {
    Usage(String),
    Equip(String),
    Raised(crate::raise::Raised),
}

fn default_workspace() -> PathBuf {
    crate::owner::parents("check_sub_hint_equip.py", 4)
}

/// `Path.resolve()` (non-strict): the longest existing prefix is canonicalised, the rest is appended lexically.
pub fn resolve(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut out = PathBuf::from("/");
    let mut exists = true;
    for c in abs.components() {
        match c {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(n) => {
                out.push(n);
                if exists {
                    match std::fs::canonicalize(&out) {
                        Ok(r) => out = r,
                        Err(_) => exists = false,
                    }
                }
            }
        }
    }
    out
}

fn disp(p: &Path) -> String {
    p.display().to_string()
}

// ── reads ────────────────────────────────────────────────────────────────────────────────────

fn io_class(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        _ => match e.raw_os_error() {
            Some(21) => "IsADirectoryError",
            Some(20) => "NotADirectoryError",
            _ => "OSError",
        },
    }
}

/// `read_env_key(ws, key)`.
fn read_env_key(ws: &Path, key: &str) -> Result<Option<String>, Fail> {
    let f = ws.join(ENV_FILE_REL);
    if !f.is_file() {
        return Ok(None);
    }
    let text = match std::fs::read(&f) {
        Ok(b) => match agent_runtime_support::pyjson::decode_file_text(&b) {
            Ok(t) => crate::py::text(&t),
            Err(_) => return Err(Fail::Usage(format!("{ENV_FILE_REL} 를 읽을 수 없다(UnicodeDecodeError) — 값은 출력하지 않는다"))),
        },
        Err(e) => return Err(Fail::Usage(format!("{ENV_FILE_REL} 를 읽을 수 없다({}) — 값은 출력하지 않는다", io_class(&e)))),
    };
    let prefix = format!("{key}=");
    for line in crate::py::splitlines(&text) {
        let st = crate::py::strip(&line);
        if st.starts_with('#') || !st.starts_with(&prefix) {
            continue;
        }
        let val = crate::py::strip(st.split_once('=').unwrap().1).trim_matches('"').trim_matches('\'');
        if !val.is_empty() {
            return Ok(Some(val.to_string()));
        }
    }
    Ok(None)
}

struct G {
    code: i32,
    stdout: String,
}

/// `_git(cwd, *args)`: repository/index/config redirection removed from the environment; never prompts.
fn git(cwd: &Path, args: &[&str]) -> G {
    let mut c = Command::new("git");
    c.args(args).current_dir(cwd).stdin(Stdio::inherit()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy().into_owned();
        if ["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES"].contains(&k.as_str()) || k.starts_with("GIT_CONFIG")
        {
            c.env_remove(&k);
        }
    }
    c.env("GIT_TERMINAL_PROMPT", "0").env("GIT_OPTIONAL_LOCKS", "0");
    match c.output() {
        Ok(o) => G { code: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).into_owned() },
        // the original lets FileNotFoundError (no git binary / cwd gone) escape; the port reports a failed read
        Err(_) => G { code: -1, stdout: String::new() },
    }
}

fn scheme(url: &str) -> &'static str {
    if url.starts_with("https://") {
        return "https";
    }
    if url.starts_with("http://") {
        return "http";
    }
    if url.starts_with("ssh://") || scp_like(url) {
        return "ssh";
    }
    "local"
}

/// `re.match(r"^[^/:@]+@[^/:]+:", url)`.
fn scp_like(url: &str) -> bool {
    let Some(at) = url.find('@') else { return false };
    let user = &url[..at];
    if user.is_empty() || user.contains(['/', ':']) {
        return false;
    }
    let rest = &url[at + 1..];
    let host_len = rest.find(['/', ':']).unwrap_or(rest.len());
    host_len > 0 && rest[host_len..].starts_with(':')
}

/// `_CRED_IN_URL.match(url)`: `^[a-z][a-z0-9+.-]*://[^/@]+@` (case-insensitive).
fn cred_in_url(url: &str) -> bool {
    let Some(i) = url.find("://") else { return false };
    let sch = &url[..i];
    let mut ch = sch.chars();
    if !ch.next().is_some_and(|c| c.is_ascii_alphabetic()) || !ch.all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)) {
        return false;
    }
    let rest = &url[i + 3..];
    match rest.find(['/', '@']) {
        Some(j) => j > 0 && rest[j..].starts_with('@'),
        None => false,
    }
}

fn inside(child: &Path, parent: &Path) -> bool {
    resolve(child).starts_with(resolve(parent))
}

type D = Vec<(&'static str, PyValue)>;

fn s(t: &str) -> PyValue {
    PyValue::str(t)
}
fn obj(d: D) -> PyValue {
    let mut x = PyDict::new();
    for (k, v) in d {
        x.set(k, v);
    }
    PyValue::Dict(x)
}
fn status_of(d: &D) -> String {
    d.iter().find(|(k, _)| *k == "status").map(|(_, v)| crate::py::display(v)).unwrap_or_default()
}
fn field<'a>(d: &'a D, k: &str) -> Option<&'a PyValue> {
    d.iter().find(|(x, _)| *x == k).map(|(_, v)| v)
}

fn check_clone(ws: &Path) -> Result<D, Fail> {
    let raw = read_env_key(ws, CLONE_DIR_KEY)?;
    let mut out: D = vec![("path_source", s(&format!("{ENV_FILE_REL}:{CLONE_DIR_KEY}"))), ("declared", PyValue::Bool(raw.is_some()))];
    let st = |mut out: D, status: &str, reason: &str| {
        out.push(("status", s(status)));
        out.push(("reason", s(reason)));
        out
    };
    let Some(raw) = raw else {
        return Ok(st(out, "missing", &format!("{CLONE_DIR_KEY} 미선언 — {ENV_FILE_REL} 에 절대경로 한 줄을 둔다")));
    };
    let p = PathBuf::from(&raw);
    if !p.is_absolute() {
        return Ok(st(out, "invalid", &format!("{CLONE_DIR_KEY} 는 절대경로여야 한다")));
    }
    if inside(&p, ws) {
        return Ok(st(out, "invalid", "hint 클론이 워크스페이스 안이다 — 워크스페이스 git 밖이어야 한다(SUB_GIT_LOCAL_ONLY)"));
    }
    if !p.is_dir() {
        return Ok(st(out, "missing", "선언된 경로에 클론이 없다 — --clone 으로 장착한다"));
    }
    let top = git(&p, &["rev-parse", "--show-toplevel"]);
    if top.code != 0 || resolve(Path::new(crate::py::strip(&top.stdout))) != resolve(&p) {
        return Ok(st(out, "invalid", "선언된 경로가 git 저장소 루트가 아니다"));
    }
    let url = git(&p, &["remote", "get-url", "origin"]);
    let u = crate::py::strip(&url.stdout).to_string();
    if url.code != 0 || u.is_empty() {
        out = st(out, "invalid", "hint 클론에 origin 이 없다");
        out.push(("origin_scheme", s("absent")));
        return Ok(out);
    }
    if cred_in_url(&u) {
        out = st(out, "invalid", "origin URL 에 자격증명이 박혀 있다 — 토큰은 envs/.env 로만");
        out.push(("origin_scheme", s(scheme(&u))));
        return Ok(out);
    }
    let mut r = git(&p, &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/origin/{HINT_BRANCH}")]);
    if r.code != 0 {
        r = git(&p, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{HINT_BRANCH}")]);
    }
    let hint_ref = if r.code == 0 { "present" } else { "absent" };
    let ok = hint_ref == "present" && scheme(&u) != "http";
    out.push(("status", s(if ok { "ok" } else { "invalid" })));
    out.push((
        "reason",
        if ok {
            PyValue::None
        } else if hint_ref == "absent" {
            s("hint 브랜치 ref 가 없다")
        } else {
            s("평문 http 원격은 거부한다")
        },
    ));
    out.push(("origin_scheme", s(scheme(&u))));
    out.push(("hint_ref", s(hint_ref)));
    out.push(("hint_clone_dir", s(&disp(&resolve(&p)))));
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    out.push(("observed_home_dir", s(&disp(&resolve(&home)))));
    for (key, args) in [
        ("git_dir", &["rev-parse", "--absolute-git-dir"][..]),
        ("git_common_dir", &["rev-parse", "--path-format=absolute", "--git-common-dir"][..]),
    ] {
        let o = git(&p, args);
        let v = crate::py::strip(&o.stdout).to_string();
        out.push((key, if o.code == 0 && Path::new(&v).is_absolute() { s(&disp(&resolve(Path::new(&v)))) } else { PyValue::None }));
    }
    Ok(out)
}

/// `check_clone(ws)` as a JSON object (insertion order) for the Rust self-test suites; `Err` = the failure text.
pub fn check_clone_value(ws: &Path) -> Result<PyValue, String> {
    match check_clone(ws) {
        Ok(d) => {
            let mut out = PyDict::new();
            for (k, v) in d {
                out.set(k, v);
            }
            Ok(PyValue::Dict(out))
        }
        Err(Fail::Usage(m)) | Err(Fail::Equip(m)) => Err(m),
        Err(Fail::Raised(r)) => Err(r.0),
    }
}

fn ignored(ws: &Path, rel: &str) -> Option<bool> {
    match git(ws, &["check-ignore", "-q", "--", rel]).code {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

fn check_push_authority(ws: &Path) -> D {
    let ign = ignored(ws, PUSH_FLAG_REL);
    let tracked = git(ws, &["ls-files", "--error-unmatch", "--", PUSH_FLAG_REL]).code == 0;
    let ign_v = ign.map_or(PyValue::None, PyValue::Bool);
    if tracked || ign == Some(false) {
        return vec![("status", s("tracked_risk")), ("ignored", ign_v), ("tracked", PyValue::Bool(tracked)),
            ("reason", s("역할 선언이 추적 대상이다 — 커밋되면 배포·복사된다(.gitignore 에 hints/.push_authority)"))];
    }
    if ign.is_none() {
        return vec![("status", s("invalid")), ("ignored", PyValue::None),
            ("reason", s("git check-ignore 판독 불가 — 워크스페이스 git 을 확인한다"))];
    }
    if !ws.join(PUSH_FLAG_REL).is_file() {
        return vec![("status", s("missing")), ("ignored", PyValue::Bool(true)), ("reason", s("--write-push-authority 로 장착한다"))];
    }
    vec![("status", s("ok")), ("ignored", PyValue::Bool(true))]
}

/// `environ`: `None` = the process environment.
fn check_token(ws: &Path, origin_scheme: Option<&str>, environ: Option<&[(&str, &str)]>) -> Result<D, Fail> {
    let env_val = match environ {
        None => std::env::var(TOKEN_KEY).ok(),
        Some(e) => e.iter().find(|(k, _)| *k == TOKEN_KEY).map(|(_, v)| v.to_string()),
    };
    let src = if env_val.is_some_and(|v| !crate::py::strip(&v).is_empty()) {
        Some("env")
    } else if read_env_key(ws, TOKEN_KEY)?.is_some() {
        Some(ENV_FILE_REL)
    } else {
        None
    };
    if let Some(src) = src {
        return Ok(vec![("status", s("present")), ("source", s(src))]);
    }
    if let Some(sch @ ("ssh" | "local")) = origin_scheme {
        return Ok(vec![("status", s("not_required")), ("source", PyValue::None),
            ("reason", s(&format!("origin 이 {sch} 이다 — https 원격만 토큰이 필요하다")))]);
    }
    Ok(vec![("status", s("absent")), ("source", PyValue::None),
        ("reason", s(&format!("{TOKEN_KEY} 가 환경변수에도 {ENV_FILE_REL} 에도 없다 — 사람이 그 노드에 둔다(배달 ✗)")))])
}

/// The verdict as a JSON-ready value plus `ready`.
fn evaluate(ws: &Path, environ: Option<&[(&str, &str)]>) -> Result<(PyValue, bool), Fail> {
    let ws = resolve(ws);
    let clone = check_clone(&ws)?;
    let remotes = git(&ws, &["remote"]);
    let local_only: D = if remotes.code != 0 {
        vec![("status", s("invalid")), ("reason", s("워크스페이스 git 판독 불가"))]
    } else {
        let n = remotes.stdout.split_whitespace().count();
        vec![("status", s(if n == 0 { "ok" } else { "violation" })), ("remotes", PyValue::int(n as i64))]
    };
    let push = check_push_authority(&ws);
    let central: D = if ws.join(CENTRAL_FLAG_REL).exists() {
        vec![("status", s("present_violation")), ("reason", s("카탈로그 권위는 메인 단독이다 — 서브에 이 선언이 있으면 안 된다"))]
    } else {
        vec![("status", s("absent"))]
    };
    let origin = field(&clone, "origin_scheme").map(crate::py::display);
    let token = check_token(&ws, origin.as_deref(), environ)?;
    let pii: D = vec![("status", s(if ws.join(PII_TERMS_REL).is_file() { "present" } else { "absent" }))];
    let checks: Vec<(&str, D)> = vec![
        ("hint_clone", clone),
        ("push_authority", push),
        ("central_authority", central),
        ("token", token),
        ("pii_terms", pii),
        ("workspace_git_local_only", local_only),
    ];
    let st = |k: &str| checks.iter().find(|(n, _)| *n == k).map(|(_, d)| status_of(d)).unwrap();
    let good: [(&str, &[&str]); 6] = [("hint_clone", &["ok"]), ("push_authority", &["ok"]), ("central_authority", &["absent"]),
        ("token", &["present", "not_required"]), ("pii_terms", &["present"]), ("workspace_git_local_only", &["ok"])];
    let ready = good.iter().all(|(k, v)| v.contains(&st(k).as_str()));
    let mut actions = vec![];
    if st("hint_clone") != "ok" {
        actions.push(obj(vec![("actor", s("sub-agent")), ("step", s("clone")),
            ("how", s(&format!("{ENV_FILE_REL} 에 {CLONE_DIR_KEY}=<워크스페이스 밖 절대경로> 를 두고 `--clone --remote-url <메인이 Task Message 로 준 프로젝트 원격>`")))]));
    }
    if st("push_authority") != "ok" {
        actions.push(obj(vec![("actor", s("sub-agent")), ("step", s("push_authority")), ("how", s("--write-push-authority"))]));
    }
    let tok_abs = st("token") == "absent";
    let pii_abs = st("pii_terms") == "absent";
    if tok_abs || pii_abs {
        let mut what = vec![];
        if tok_abs {
            what.push(s(&format!("{TOKEN_KEY}@{ENV_FILE_REL}")));
        }
        if pii_abs {
            what.push(s(PII_TERMS_REL));
        }
        actions.push(obj(vec![("actor", s("human")), ("step", s("place_secrets")), ("what", PyValue::List(what)),
            ("request_template", s(REQUEST_TEMPLATE_REL)),
            ("how", s("testlog 로 보고만 한다 — 메인이 docs/request/ 수행지시서를 발행하고 사람이 둔다"))]));
    }
    if st("central_authority") != "absent" || st("workspace_git_local_only") != "ok" {
        actions.push(obj(vec![("actor", s("main")), ("step", s("escalate")),
            ("how", s("역할·원격 위반은 서브가 스스로 고치지 않는다 — 보고하고 메인 HITL"))]));
    }
    let mut cd = PyDict::new();
    for (k, d) in checks {
        cd.set(k, obj(d));
    }
    Ok((
        obj(vec![("schema_version", PyValue::int(1)), ("tool", s("check_sub_hint_equip")), ("provenance", s("measured")),
            ("checks", PyValue::Dict(cd)), ("ready", PyValue::Bool(ready)), ("next_actions", PyValue::List(actions))]),
        ready,
    ))
}

// ── equip ────────────────────────────────────────────────────────────────────────────────────

fn equip_clone(ws: &Path, remote_url: &str) -> Result<&'static str, Fail> {
    if remote_url.is_empty() || cred_in_url(remote_url) {
        return Err(Fail::Usage("--remote-url 이 비었거나 자격증명이 박혀 있다 — 토큰은 envs/.env 로만".into()));
    }
    if scheme(remote_url) == "http" {
        return Err(Fail::Usage("평문 http 원격은 거부한다".into()));
    }
    let Some(raw) = read_env_key(ws, CLONE_DIR_KEY)? else {
        return Err(Fail::Usage(format!("{CLONE_DIR_KEY} 미선언 — {ENV_FILE_REL} 에 워크스페이스 밖 절대경로 한 줄을 먼저 둔다(선언 자리는 하나)")));
    };
    let p = PathBuf::from(&raw);
    if !p.is_absolute() || inside(&p, ws) {
        return Err(Fail::Usage(format!("{CLONE_DIR_KEY} 는 워크스페이스 밖 절대경로여야 한다(SUB_GIT_LOCAL_ONLY — 워크스페이스 git 에 원격 ✗)")));
    }
    if p.exists() {
        let mut rd = std::fs::read_dir(&p).map_err(|e| Fail::Raised(crate::raise::Raised::os(&e, &raw)))?;
        if rd.next().is_some() {
            if status_of(&check_clone(ws)?) == "ok" {
                return Ok("already");
            }
            return Err(Fail::Usage(format!("{CLONE_DIR_KEY} 경로가 비어 있지 않고 유효한 hint 클론도 아니다 — 사람이 정리한다(자동 삭제 ✗)")));
        }
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Fail::Raised(crate::raise::Raised::os(&e, &disp(parent))))?;
    }
    let r = Command::new("git")
        .args(["clone", "--single-branch", "--branch", HINT_BRANCH, "--depth", "1", "--no-tags", "--quiet", remote_url, &raw])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| Fail::Raised(crate::raise::Raised::os(&e, "git")))?;
    let code = r.status.code().unwrap_or(-1);
    if code != 0 {
        let err: String = crate::py::strip(&String::from_utf8_lossy(&r.stderr)).chars().take(300).collect();
        return Err(Fail::Equip(format!("git clone rc={code}: {err}")));
    }
    Ok("cloned")
}

fn equip_push_authority(ws: &Path, utc: Option<&str>) -> Result<&'static str, Fail> {
    let st = check_push_authority(ws);
    match status_of(&st).as_str() {
        "ok" => return Ok("already"),
        "missing" => {}
        _ => {
            let reason = field(&st, "reason").map_or("None".to_string(), crate::py::display);
            return Err(Fail::Usage(format!("push 역할 선언을 쓰지 않는다 — {reason}")));
        }
    }
    let f = ws.join(PUSH_FLAG_REL);
    let parent = f.parent().unwrap();
    std::fs::create_dir_all(parent).map_err(|e| Fail::Raised(crate::raise::Raised::os(&e, &disp(parent))))?;
    let body = format!(
        "# hint 태그 push 역할 선언(비추적 · 배달 ✗ · 카탈로그 권위 아님) — check_sub_hint_equip.py 가 썼다{}\n",
        utc.filter(|u| !u.is_empty()).map_or(String::new(), |u| format!(" · {u}"))
    );
    std::fs::write(&f, body).map_err(|e| Fail::Raised(crate::raise::Raised::os(&e, &disp(&f))))?;
    Ok("written")
}

// ── self-test ────────────────────────────────────────────────────────────────────────────────

fn tempdir(prefix: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let p = base.join(format!("{prefix}{}{n:x}", std::process::id()));
    std::fs::create_dir_all(&p).expect("tempdir");
    p
}

fn self_test() -> i32 {
    let mut fails: Vec<String> = vec![];
    let mut ran = 0;
    let mut ck = |name: &str, cond: bool, detail: &str| {
        ran += 1;
        println!("  [{}] {name}{}", if cond { "PASS" } else { "FAIL" }, if cond { String::new() } else { format!(" — {detail}") });
        if !cond {
            fails.push(name.to_string());
        }
    };
    let g = |cwd: &Path, a: &[&str]| {
        let st = Command::new("git").args(a).current_dir(cwd)
            .env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t.invalid")
            .env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t.invalid").env("GIT_TERMINAL_PROMPT", "0")
            .stdout(Stdio::null()).stderr(Stdio::null()).status().expect("git");
        assert!(st.success(), "git {a:?} failed");
    };
    let secret = "ghp_SELFTEST_SECRET_VALUE_9f2c";
    let empty: &[(&str, &str)] = &[];
    let ev = |ws: &Path, env: &[(&str, &str)]| evaluate(ws, Some(env)).ok().map(|x| x.0);
    let at = |v: &Option<PyValue>, path: &[&str]| -> String {
        let mut cur = match v { Some(x) => x, None => return "<err>".into() };
        for k in path {
            match crate::py::get(cur, k) {
                Some(x) => cur = x,
                None => return "<absent>".into(),
            }
        }
        crate::py::display(cur)
    };
    let ready = |v: &Option<PyValue>| v.as_ref().is_some_and(|x| crate::py::truthy(crate::py::get(x, "ready")));
    let actors = |v: &Option<PyValue>| -> Vec<(String, String)> {
        match v.as_ref().and_then(|x| crate::py::get(x, "next_actions")) {
            Some(PyValue::List(l)) => l.iter().map(|a| (crate::py::display_opt(crate::py::get(a, "actor")),
                crate::py::display_opt(crate::py::get(a, "request_template")))).collect(),
            _ => vec![],
        }
    };
    let dump = |v: &Option<PyValue>| v.as_ref().map_or(String::new(), crate::py::dumps_unicode);
    let td = tempdir("hint-equip-");
    let td = resolve(&td);
    let origin = td.join("origin.git");
    let seed = td.join("seed");
    std::fs::create_dir(&seed).unwrap();
    g(&seed, &["init", "-q", "-b", HINT_BRANCH]);
    std::fs::write(seed.join("README.md"), "<!-- hint-branch-format: v7 -->\n").unwrap();
    g(&seed, &["add", "README.md"]);
    g(&seed, &["commit", "-q", "-m", "guide"]);
    g(&td, &["clone", "-q", "--bare", &disp(&seed), &disp(&origin)]);
    let ws = td.join("ws");
    std::fs::create_dir_all(ws.join("envs")).unwrap();
    g(&ws, &["init", "-q"]);
    std::fs::write(ws.join(".gitignore"), "envs/.env\nhints/.push_authority\nhints/.drafts/\n.claude/pii_terms.txt\n").unwrap();
    let clone_dir = td.join("hint-clone");
    let o = disp(&origin);
    let r0 = ev(&ws, empty);
    ck("미장착 → ready=False · 클론 missing(미선언) · 토큰 absent · 마커 missing",
        r0.is_some() && !ready(&r0) && at(&r0, &["checks", "hint_clone", "status"]) == "missing"
            && at(&r0, &["checks", "hint_clone", "declared"]) == "False"
            && at(&r0, &["checks", "token", "status"]) == "absent"
            && at(&r0, &["checks", "push_authority", "status"]) == "missing", &dump(&r0));
    ck("미장착 → 다음 행동에 사람 요청(템플릿 경로 포함)",
        actors(&r0).iter().any(|(a, t)| a == "human" && t == REQUEST_TEMPLATE_REL), "");
    ck("미선언 --clone 거부", matches!(equip_clone(&ws, &o), Err(Fail::Usage(_))), "통과했다");
    std::fs::write(ws.join("envs/.env"), format!("{CLONE_DIR_KEY}={}\n", disp(&ws.join("inner")))).unwrap();
    ck("워크스페이스 안 클론 경로 거부", matches!(equip_clone(&ws, &o), Err(Fail::Usage(_))), "통과했다");
    ck("워크스페이스 안 선언 → 판정 invalid", check_clone(&ws).ok().is_some_and(|d| status_of(&d) == "invalid"), "");
    std::fs::write(ws.join("envs/.env"), format!("{CLONE_DIR_KEY}={}\n", disp(&clone_dir))).unwrap();
    ck("자격증명 박힌 URL 거부", matches!(equip_clone(&ws, "https://user:tok@example.invalid/x.git"), Err(Fail::Usage(_))), "통과했다");
    ck("--clone 정상(로컬 원격)", matches!(equip_clone(&ws, &o), Ok("cloned")) && clone_dir.is_dir(), "");
    ck("--clone 멱등(already)", matches!(equip_clone(&ws, &o), Ok("already")), "");
    ck("--write-push-authority 정상", matches!(equip_push_authority(&ws, Some("2026-10-11T00:00:00Z")), Ok("written")), "");
    ck("--write-push-authority 멱등", matches!(equip_push_authority(&ws, None), Ok("already")), "");
    let _ = std::fs::create_dir(ws.join(".claude"));
    std::fs::write(ws.join(PII_TERMS_REL), "").unwrap();
    let r1 = ev(&ws, empty);
    ck("로컬 원격 · 마커 · pii 있음 → ready(토큰 not_required)",
        ready(&r1) && at(&r1, &["checks", "token", "status"]) == "not_required"
            && at(&r1, &["checks", "hint_clone", "hint_ref"]) == "present", &dump(&r1));
    g(&clone_dir, &["remote", "set-url", "origin", "https://example.invalid/proj.git"]);
    let r2 = ev(&ws, empty);
    ck("https 원격 · 토큰 없음 → absent · ready=False", !ready(&r2) && at(&r2, &["checks", "token", "status"]) == "absent", &dump(&r2));
    {
        use std::io::Write;
        let mut fh = std::fs::OpenOptions::new().append(true).open(ws.join("envs/.env")).unwrap();
        write!(fh, "# 주석\n{TOKEN_KEY}='{secret}'\n").unwrap();
    }
    let r3 = ev(&ws, empty);
    let tok = r3.as_ref().and_then(|x| crate::py::get(x, "checks")).and_then(|x| crate::py::get(x, "token")).map(crate::py::dumps_unicode);
    ck("토큰 present(envs/.env) · ready",
        ready(&r3) && tok.as_deref() == Some("{\"status\": \"present\", \"source\": \"envs/.env\"}"), &dump(&r3));
    ck("★토큰 값은 출력 어디에도 없다", !dump(&r3).contains(secret), "");
    ck("환경변수 토큰 → source=env", at(&ev(&ws, &[(TOKEN_KEY, "x")]), &["checks", "token", "source"]) == "env", "");
    // Cross-check with the hint publisher's rule: its Rust owner `hint-publisher/rust/src/tag.rs` (the port of `hintlib/tag.py`,
    // plan_26111615 pass 3) must declare the same key and file and keep its reader (read as text — the publisher is not run
    // here), and this port's reader must return the secret it reads.
    let tag = default_workspace().join(".claude/skills/hint-publisher/rust/src/tag.rs");
    let tag_ok = std::fs::read_to_string(&tag).is_ok_and(|t| {
        t.contains(&format!("pub const TOKEN_KEY: &str = \"{TOKEN_KEY}\";"))
            && t.contains(&format!("pub const TOKEN_ENV_FILE_REL: &str = \"{ENV_FILE_REL}\";"))
            && t.contains("pub fn read_push_token(")
    });
    ck("hintlib.tag 와 토큰 키·파일·판독 규칙 일치(교차검증)",
        tag_ok && read_env_key(&ws, TOKEN_KEY).ok().flatten().as_deref() == Some(secret), "hintlib 적재 실패 또는 규칙 불일치");
    g(&clone_dir, &["remote", "set-url", "origin", &format!("https://x:{secret}@example.invalid/p.git")]);
    let r4 = ev(&ws, empty);
    ck("origin URL 자격증명 → invalid · 출력 무비밀",
        at(&r4, &["checks", "hint_clone", "status"]) == "invalid" && !dump(&r4).contains(secret), "");
    g(&clone_dir, &["remote", "set-url", "origin", "https://example.invalid/proj.git"]);
    std::fs::write(ws.join(CENTRAL_FLAG_REL), "x\n").unwrap();
    let r5 = ev(&ws, empty);
    ck("서브의 .central_authority → present_violation · ready=False · 메인 에스컬레이션",
        !ready(&r5) && at(&r5, &["checks", "central_authority", "status"]) == "present_violation"
            && actors(&r5).iter().any(|(a, _)| a == "main"), "");
    let _ = std::fs::remove_file(ws.join(CENTRAL_FLAG_REL));
    g(&ws, &["remote", "add", "origin", &o]);
    let r6 = ev(&ws, empty);
    let lo = r6.as_ref().and_then(|x| crate::py::get(x, "checks")).and_then(|x| crate::py::get(x, "workspace_git_local_only")).map(crate::py::dumps_unicode);
    ck("워크스페이스 원격 → violation(SUB_GIT_LOCAL_ONLY)",
        !ready(&r6) && lo.as_deref() == Some("{\"status\": \"violation\", \"remotes\": 1}"), "");
    g(&ws, &["remote", "remove", "origin"]);
    let ws2 = td.join("ws2");
    std::fs::create_dir(&ws2).unwrap();
    g(&ws2, &["init", "-q"]);
    ck("비-ignore 워크스페이스 마커 쓰기 거부",
        matches!(equip_push_authority(&ws2, None), Err(Fail::Usage(_))) && !ws2.join(PUSH_FLAG_REL).exists(), "통과했다");
    ck("판정 결정론(같은 입력 → 같은 JSON)", dump(&ev(&ws, empty)) == dump(&ev(&ws, empty)), "");
    let _ = std::fs::remove_dir_all(&td);
    let dw = default_workspace();
    let main_tree = dw.join(".claude/skills/terraforming_node/SKILL.md").is_file();
    ck("사람 요청 템플릿이 실재한다(메인 트리 · 서브 렌더 트리는 메인 전용이라 부재 정상)",
        dw.join(REQUEST_TEMPLATE_REL).is_file() || !main_tree, REQUEST_TEMPLATE_REL);
    println!("--- {}/{} PASS", ran - fails.len(), ran);
    if fails.is_empty() { 0 } else { 1 }
}

// ── CLI ──────────────────────────────────────────────────────────────────────────────────────

pub static SPEC: Spec = Spec {
    prog: "check_sub_hint_equip.py",
    usage: include_str!("../help/check_sub_hint_equip.usage"),
    help: include_str!("../help/check_sub_hint_equip.help"),
    args: &[
        HELP,
        store(&["--workspace"]),
        flag(&["--clone"]),
        store(&["--remote-url"]),
        flag(&["--write-push-authority"]),
        store(&["--utc"]),
        flag(&["--self-test"]),
    ],
    subs: None,
};

pub fn main(args: &[String]) -> i32 {
    let a = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    if a.flag(6) {
        return self_test();
    }
    let ws = resolve(&a.text(1).map(PathBuf::from).unwrap_or_else(default_workspace));
    let run = || -> Result<i32, Fail> {
        if a.flag(2) {
            let r = equip_clone(&ws, a.text(3).unwrap_or(""))?;
            println!("{{\"equip\": \"clone\", \"result\": \"{r}\"}}");
        }
        if a.flag(4) {
            let r = equip_push_authority(&ws, a.text(5))?;
            println!("{{\"equip\": \"push_authority\", \"result\": \"{r}\"}}");
        }
        if a.flag(2) || a.flag(4) {
            return Ok(EXIT_READY);
        }
        let (res, ready) = evaluate(&ws, None)?;
        println!("{}", crate::py::dumps_pretty(&res));
        Ok(if ready { EXIT_READY } else { EXIT_NOT_READY })
    };
    match run() {
        Ok(c) => c,
        Err(Fail::Usage(m)) => {
            eprintln!("[check_sub_hint_equip] FAIL: {m}");
            EXIT_USAGE
        }
        Err(Fail::Equip(m)) => {
            eprintln!("[check_sub_hint_equip] FAIL(장착): {m}");
            EXIT_EQUIP_FAILED
        }
        Err(Fail::Raised(r)) => crate::raise::exit(&r),
    }
}
