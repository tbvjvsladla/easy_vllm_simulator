//! Port of `check_sub_hint_equip.py` — the single sub's hint-publish equipment, judged and equipped on THAT node
//! (hint clone outside the workspace git, `hints/.push_authority`, `GITHUB_TOKEN` presence). Same command line, exit codes
//! (0 ready/equipped · 2 usage · 3 not ready · 4 equip failed), JSON (insertion order) and self-test lines. Secrets are never
//! printed. The self-test's cross-check with the hint publisher's token rule reads its Rust owner `hint-publisher/rust/src/tag.rs` as text (it is not run).
//!
//! plan_26112012 (h53 · h56: the sub never reaches GitHub over ssh; the `envs/.env` mode is narrowed, never widened):
//! * S1 — `--clone` pins the hint clone's push address `remote.origin.pushurl` to the credential-free https `--remote-url`, on a new
//!   clone and on an already equipped one alike (idempotent); any other scheme of `--remote-url` (ssh, scp-like, a local path,
//!   plain http) is a usage error.
//! * The verdict reads the push URLs too: `ok` = fetch and push https with a pinned push URL; an ssh or scp-like fetch or push is a
//!   `violation` (#7, the main escalates); an unpinned push URL is `pushurl_unpinned` (not ready, the sub reinstalls); a local path
//!   is `invalid` (a token is `not_required` only for such a self-test origin, and it never makes a verdict ready).
//! * `--push-transport` (M1-②) — the read-only answer the main's sync asks over `sub_run`: four keys, no URL, path or token; exit 0
//!   = https + pinned or no `HINT_CLONE_DIR` (not applicable), exit 3 = blocked (never the usage exit 2).
//! * #2 — `checks.env_file_mode` reports the file's kind and mode (lstat only); `--narrow-env-mode` narrows it to `mode & 0o600`
//!   (`role_contract::env_file_mode`, shared with the main's `scan_node --narrow-env-mode`).

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use role_contract::env_file_mode::{self, NarrowAction};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

const ENV_FILE_REL: &str = env_file_mode::ENV_FILE_REL;
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
/// `--push-transport` block: the "not ready" exit of this tool, never the usage exit 2 (which an executable that predates the mode
/// gives) -- the main's sync passes exit 0 only and names the block either way.
const EXIT_PUSH_TRANSPORT_BLOCKED: i32 = EXIT_NOT_READY;
const PUSHURL_KEY: &str = "remote.origin.pushurl";
const WORKSPACE_SHARED_CHOICES: &[&str] = &["shared", "distinct", "unknown"];
const REINSTALL_HOW: &str =
    "서브 재빌드(build_rust_packages.sh) 뒤 `--clone --remote-url <메인이 Task Message 로 준 자격 없는 https 프로젝트 원격>` 를 다시 돌린다(멱등 · push 주소 고정 S1)";

enum Fail {
    Usage(String),
    Equip(String),
    Raised(crate::raise::Raised),
}

fn default_workspace() -> PathBuf {
    crate::owner::parents("check_sub_hint_equip.sh", 4)
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

/// The hint clone's `origin` as git uses it: the fetch URL, every push URL (`remote.origin.pushurl`, else the url) and whether a push
/// URL is pinned. `Err` = the read failed: no origin (`OriginAbsent`) or a later git read failed (`Unreadable`).
struct OriginUrls {
    fetch: String,
    push: Vec<String>,
    pinned: bool,
}

enum OriginRead {
    OriginAbsent,
    /// The fetch URL was read; the push URLs or the pin were not.
    Unreadable { fetch: String },
}

fn origin_urls(clone: &Path) -> Result<OriginUrls, OriginRead> {
    let fetch = git(clone, &["remote", "get-url", "origin"]);
    let fetch_url = crate::py::strip(&fetch.stdout).to_string();
    if fetch.code != 0 || fetch_url.is_empty() {
        return Err(OriginRead::OriginAbsent);
    }
    let push = git(clone, &["remote", "get-url", "--push", "--all", "origin"]);
    let push_urls: Vec<String> =
        push.stdout.lines().map(|l| crate::py::strip(l).to_string()).filter(|l| !l.is_empty()).collect();
    if push.code != 0 || push_urls.is_empty() {
        return Err(OriginRead::Unreadable { fetch: fetch_url });
    }
    let pinned = match git(clone, &["config", "--get-all", PUSHURL_KEY]) {
        G { code: 0, stdout } => stdout.lines().any(|l| !crate::py::strip(l).is_empty()),
        G { code: 1, .. } => false,
        _ => return Err(OriginRead::Unreadable { fetch: fetch_url }),
    };
    Ok(OriginUrls { fetch: fetch_url, push: push_urls, pinned })
}

/// The push transport over every push URL: `https` only when all are https, otherwise the first other scheme.
fn push_scheme(urls: &[String]) -> &'static str {
    urls.iter().map(|u| scheme(u)).find(|sch| *sch != "https").unwrap_or("https")
}

/// The hint clone's verdict over its transports (S1 · #7): fetch and push https with a pinned push URL is the only `ok`.
fn clone_verdict(fetch: &str, push: &str, pinned: bool, hint_ref_present: bool) -> (&'static str, Option<&'static str>) {
    if fetch == "ssh" || push == "ssh" {
        return ("violation", Some("hint 클론의 fetch·push 가 ssh 다 — 서브는 GitHub 에 ssh 로 닿지 않는다(h53 · S1 https 재장착 · 메인 HITL)"));
    }
    if !hint_ref_present {
        return ("invalid", Some("hint 브랜치 ref 가 없다"));
    }
    if fetch == "http" || push == "http" {
        return ("invalid", Some("평문 http 원격은 거부한다"));
    }
    if fetch != "https" || push != "https" {
        return ("invalid", Some("hint 클론 원격이 https 가 아니다(로컬 경로) — 자격 없는 https URL 로 재장착한다(S1)"));
    }
    if !pinned {
        return ("pushurl_unpinned", Some("push 주소(remote.origin.pushurl)가 고정되지 않았다 — 재장착한다(S1 · 멱등)"));
    }
    ("ok", None)
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
    let urls = match origin_urls(&p) {
        Ok(urls) => urls,
        Err(OriginRead::OriginAbsent) => {
            out = st(out, "invalid", "hint 클론에 origin 이 없다");
            out.push(("origin_scheme", s("absent")));
            return Ok(out);
        }
        Err(OriginRead::Unreadable { fetch }) => {
            out = st(out, "invalid", "hint 클론의 push 주소를 읽을 수 없다(git 판독 실패)");
            out.push(("origin_scheme", s(scheme(&fetch))));
            return Ok(out);
        }
    };
    let (fetch_scheme, push_sch) = (scheme(&urls.fetch), push_scheme(&urls.push));
    if cred_in_url(&urls.fetch) || urls.push.iter().any(|u| cred_in_url(u)) {
        out = st(out, "invalid", "origin URL 에 자격증명이 박혀 있다 — 토큰은 envs/.env 로만");
        out.push(("origin_scheme", s(fetch_scheme)));
        out.push(("push_scheme", s(push_sch)));
        return Ok(out);
    }
    let mut r = git(&p, &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/origin/{HINT_BRANCH}")]);
    if r.code != 0 {
        r = git(&p, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{HINT_BRANCH}")]);
    }
    let hint_ref = if r.code == 0 { "present" } else { "absent" };
    let (status, reason) = clone_verdict(fetch_scheme, push_sch, urls.pinned, hint_ref == "present");
    out.push(("status", s(status)));
    out.push(("reason", reason.map_or(PyValue::None, s)));
    out.push(("origin_scheme", s(fetch_scheme)));
    out.push(("push_scheme", s(push_sch)));
    out.push(("pushurl_pinned", PyValue::Bool(urls.pinned)));
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

fn fail_text(fail: Fail) -> String {
    match fail {
        Fail::Usage(m) | Fail::Equip(m) => m,
        Fail::Raised(r) => r.0,
    }
}

/// `check_clone(ws)` as a JSON object (insertion order) for the Rust self-test suites; `Err` = the failure text.
pub fn check_clone_value(ws: &Path) -> Result<PyValue, String> {
    check_clone(ws).map(obj).map_err(fail_text)
}

/// `--clone --remote-url URL` (S1) for the Rust self-test suites and the policy predicates: the result word (`cloned`, `already`) or
/// the failure text.
pub fn equip_clone_value(ws: &Path, remote_url: &str) -> Result<&'static str, String> {
    equip_clone(ws, remote_url).map_err(fail_text)
}

/// `--push-transport` (M1-②) as its four-key answer and exit status, for the same callers.
pub fn push_transport_value(ws: &Path) -> (PyValue, i32) {
    push_transport(ws)
}

/// `--narrow-env-mode [--sub-workspace-shared V]` (#2 · #2b) as its correction record and whether it acted, for the same callers.
pub fn narrow_env_value(ws: &Path, workspace_shared: Option<&str>) -> (PyValue, bool) {
    narrow_env_record(ws, workspace_shared)
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
    // #7: only a local-path origin (the self-test's isolated fixture, which never reaches GitHub) needs no token; an ssh origin is a
    // clone violation, not a reason to skip the token. `not_required` never makes a verdict ready (ready needs an https clone).
    if origin_scheme == Some("local") {
        return Ok(vec![("status", s("not_required")), ("source", PyValue::None),
            ("reason", s("origin 이 로컬 경로다(자체검사 격리) — GitHub 에 닿지 않아 토큰이 필요 없다 · ready 는 https 클론이 필요하다"))]);
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
    let env_mode = env_file_mode::observe(&ws);
    // kind and mode class only (lstat): the octal digits belong to the correction record (`--narrow-env-mode`), not to the verdict
    let env: D = vec![("path_rel", s(ENV_FILE_REL)), ("status", s(env_mode.status()))];
    let checks: Vec<(&str, D)> = vec![
        ("hint_clone", clone),
        ("push_authority", push),
        ("central_authority", central),
        ("token", token),
        ("pii_terms", pii),
        ("workspace_git_local_only", local_only),
        ("env_file_mode", env),
    ];
    let st = |k: &str| checks.iter().find(|(n, _)| *n == k).map(|(_, d)| status_of(d)).unwrap();
    // `env_file_mode` is reported and acted on (next_actions) but is not a readiness condition of the hint equipment.
    let good: [(&str, &[&str]); 6] = [("hint_clone", &["ok"]), ("push_authority", &["ok"]), ("central_authority", &["absent"]),
        ("token", &["present"]), ("pii_terms", &["present"]), ("workspace_git_local_only", &["ok"])];
    let ready = good.iter().all(|(k, v)| v.contains(&st(k).as_str()));
    let mut actions = vec![];
    match st("hint_clone").as_str() {
        "ok" | "violation" => {}
        "pushurl_unpinned" => actions.push(obj(vec![("actor", s("sub-agent")), ("step", s("reinstall")), ("how", s(REINSTALL_HOW))])),
        _ => actions.push(obj(vec![("actor", s("sub-agent")), ("step", s("clone")),
            ("how", s(&format!("{ENV_FILE_REL} 에 {CLONE_DIR_KEY}=<워크스페이스 밖 절대경로> 를 두고 `--clone --remote-url <메인이 Task Message 로 준 자격 없는 https 프로젝트 원격>`")))])),
    }
    if st("push_authority") != "ok" {
        actions.push(obj(vec![("actor", s("sub-agent")), ("step", s("push_authority")), ("how", s("--write-push-authority"))]));
    }
    if st("env_file_mode") == "wide" {
        actions.push(obj(vec![("actor", s("sub-agent")), ("step", s("narrow_env_mode")),
            ("how", s("--narrow-env-mode (mode & 0o600 · 넓히지 않는다 · 내용 비열람)"))]));
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
    if st("central_authority") != "absent" || st("workspace_git_local_only") != "ok" || st("hint_clone") == "violation" {
        actions.push(obj(vec![("actor", s("main")), ("step", s("escalate")),
            ("how", s("역할·원격 위반은 서브가 스스로 고치지 않는다 — 보고하고 메인 HITL"))]));
    }
    if st("env_file_mode") == "unknown" {
        actions.push(obj(vec![("actor", s("main")), ("step", s("escalate_env_mode")),
            ("how", s("envs/.env 가 일반 파일이 아니거나 stat 판독 불가 — 600 으로 간주하지 않고 고치지 않는다 · 보고하고 메인 HITL"))]));
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
    match scheme(remote_url) {
        "https" => {}
        "http" => return Err(Fail::Usage("평문 http 원격은 거부한다".into())),
        _ => return Err(Fail::Usage(
            "--remote-url 은 자격 없는 https URL 이어야 한다 — ssh·scp형·로컬 경로는 받지 않는다(S1 · 서브는 GitHub 에 ssh 로 닿지 않는다)".into(),
        )),
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
            if repinnable(&check_clone(ws)?) {
                pin_push_url(&p, remote_url)?;
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
    pin_push_url(&p, remote_url)?;
    Ok("cloned")
}

/// An equipped clone the reinstall may pin: an https fetch -- `ok`, `pushurl_unpinned`, or a `violation` that comes from the push URL
/// alone (an ssh push address beside an https fetch, which the pin replaces). Anything else stays a human's to clear.
fn repinnable(clone: &D) -> bool {
    let fetch_https = field(clone, "origin_scheme").map(crate::py::display).as_deref() == Some("https");
    match status_of(clone).as_str() {
        "ok" | "pushurl_unpinned" => true,
        "violation" => fetch_https,
        _ => false,
    }
}

/// S1: `remote.origin.pushurl` = the credential-free https `url` (every earlier push URL replaced), written by git inside the clone.
fn pin_push_url(clone: &Path, url: &str) -> Result<(), Fail> {
    let r = git(clone, &["config", "--replace-all", PUSHURL_KEY, url]);
    if r.code != 0 {
        return Err(Fail::Equip(format!("push 주소 고정 실패(git config {PUSHURL_KEY} rc={})", r.code)));
    }
    Ok(())
}

/// `--push-transport` (M1-②): the four keys and the exit code. A read that fails (an unreadable `envs/.env`, a declared path that is no
/// git root, an unreadable origin) is a block, never a usage error.
fn push_transport(ws: &Path) -> (PyValue, i32) {
    let answer = |declared: PyValue, fetch: PyValue, push: PyValue, pinned: PyValue| {
        obj(vec![("hint_clone_declared", declared), ("fetch_scheme", fetch), ("push_scheme", push), ("pushurl_pinned", pinned)])
    };
    let unreadable = |declared: PyValue| (answer(declared, PyValue::None, PyValue::None, PyValue::None), EXIT_PUSH_TRANSPORT_BLOCKED);
    let raw = match read_env_key(ws, CLONE_DIR_KEY) {
        Ok(Some(raw)) => raw,
        Ok(None) => return (answer(PyValue::Bool(false), PyValue::None, PyValue::None, PyValue::None), EXIT_READY),
        Err(_) => return unreadable(PyValue::None),
    };
    let p = PathBuf::from(&raw);
    if !p.is_absolute() || !p.is_dir() {
        return unreadable(PyValue::Bool(true));
    }
    let top = git(&p, &["rev-parse", "--show-toplevel"]);
    if top.code != 0 || resolve(Path::new(crate::py::strip(&top.stdout))) != resolve(&p) {
        return unreadable(PyValue::Bool(true));
    }
    let Ok(urls) = origin_urls(&p) else { return unreadable(PyValue::Bool(true)) };
    let (fetch, push) = (scheme(&urls.fetch), push_scheme(&urls.push));
    let ok = fetch == "https" && push == "https" && urls.pinned;
    (
        answer(PyValue::Bool(true), s(fetch), s(push), PyValue::Bool(urls.pinned)),
        if ok { EXIT_READY } else { EXIT_PUSH_TRANSPORT_BLOCKED },
    )
}

/// `--narrow-env-mode [--sub-workspace-shared V]`: the correction record and whether it acted (`unknown` = not corrected, exit 4).
/// `covers_topologies` says which topologies' sub file this one correction is (#2b): both when the main's manifest-pair verdict
/// relayed by the reinstall instruction is `shared`, the single sub's own file otherwise; `covers_source` names that relay.
fn narrow_env_record(ws: &Path, workspace_shared: Option<&str>) -> (PyValue, bool) {
    let narrowing = env_file_mode::narrow(ws);
    let mut record = match env_file_mode::narrowing_value(&narrowing) {
        PyValue::Dict(d) => d,
        _ => PyDict::new(),
    };
    let covers: Vec<PyValue> = if workspace_shared == Some("shared") { vec![s("single"), s("multi")] } else { vec![s("single")] };
    record.set("covers_topologies", PyValue::List(covers));
    record.set(
        "covers_source",
        s(&workspace_shared.map_or("not-relayed".to_string(), |v| format!("relayed:sub_workspace_shared={v}"))),
    );
    (PyValue::Dict(record), narrowing.action != NarrowAction::Unknown)
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
        "# hint 태그 push 역할 선언(비추적 · 배달 ✗ · 카탈로그 권위 아님) — check_sub_hint_equip.sh 가 썼다{}\n",
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

/// Scopes the process environment for the self-test's fresh https clone: `url.<local bare>.insteadOf` reaches a local bare without a
/// network (the clone's child `git` reads `GIT_CONFIG_*`; the verdict's own reads strip it). Restored on drop.
struct InsteadOf {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl InsteadOf {
    const KEYS: [&'static str; 3] = ["GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0"];

    fn set(local_bare: &Path, https_url: &str) -> InsteadOf {
        let saved = Self::KEYS.iter().map(|k| (*k, std::env::var_os(k))).collect();
        std::env::set_var("GIT_CONFIG_COUNT", "1");
        std::env::set_var("GIT_CONFIG_KEY_0", format!("url.{}.insteadOf", disp(local_bare)));
        std::env::set_var("GIT_CONFIG_VALUE_0", https_url);
        InsteadOf { saved }
    }
}

impl Drop for InsteadOf {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
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
    let steps = |v: &Option<PyValue>| -> Vec<String> {
        match v.as_ref().and_then(|x| crate::py::get(x, "next_actions")) {
            Some(PyValue::List(l)) => l.iter().map(|a| crate::py::display_opt(crate::py::get(a, "step"))).collect(),
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
    let https = "https://example.invalid/proj.git";
    let scp = "git@example.invalid:x/proj.git";
    let pushurl = |c: &Path| git(c, &["config", "--get-all", PUSHURL_KEY]).stdout.trim().to_string();
    let _net = InsteadOf::set(&origin, https);
    let r0 = ev(&ws, empty);
    ck("미장착 → ready=False · 클론 missing(미선언) · 토큰 absent · 마커 missing",
        r0.is_some() && !ready(&r0) && at(&r0, &["checks", "hint_clone", "status"]) == "missing"
            && at(&r0, &["checks", "hint_clone", "declared"]) == "False"
            && at(&r0, &["checks", "token", "status"]) == "absent"
            && at(&r0, &["checks", "push_authority", "status"]) == "missing", &dump(&r0));
    ck("미장착 → 다음 행동에 사람 요청(템플릿 경로 포함)",
        actors(&r0).iter().any(|(a, t)| a == "human" && t == REQUEST_TEMPLATE_REL), "");
    let (pt0, pt0_code) = push_transport(&ws);
    ck("--push-transport 미선언 → exit 0(not_applicable) · hint_clone_declared=False",
        pt0_code == EXIT_READY && at(&Some(pt0.clone()), &["hint_clone_declared"]) == "False", &crate::py::dumps_unicode(&pt0));
    ck("미선언 --clone 거부", matches!(equip_clone(&ws, https), Err(Fail::Usage(_))), "통과했다");
    std::fs::write(ws.join("envs/.env"), format!("{CLONE_DIR_KEY}={}\n", disp(&ws.join("inner")))).unwrap();
    ck("워크스페이스 안 클론 경로 거부", matches!(equip_clone(&ws, https), Err(Fail::Usage(_))), "통과했다");
    ck("워크스페이스 안 선언 → 판정 invalid", check_clone(&ws).ok().is_some_and(|d| status_of(&d) == "invalid"), "");
    std::fs::write(ws.join("envs/.env"), format!("{CLONE_DIR_KEY}={}\n", disp(&clone_dir))).unwrap();
    ck("자격증명 박힌 URL 거부", matches!(equip_clone(&ws, "https://user:tok@example.invalid/x.git"), Err(Fail::Usage(_))), "통과했다");
    ck("★--remote-url ssh·scp형·로컬 경로 → 사용 오류(S1) · 클론 미생성",
        [scp, "ssh://git@example.invalid/x/proj.git", o.as_str()].iter().all(|u| matches!(equip_clone(&ws, u), Err(Fail::Usage(_))))
            && !clone_dir.exists(), "통과했다");
    ck("★--clone 정상(https · 네트워크 없이 insteadOf 로 로컬 bare) → push 주소 고정",
        matches!(equip_clone(&ws, https), Ok("cloned")) && clone_dir.is_dir() && pushurl(&clone_dir) == https, &pushurl(&clone_dir));
    ck("--clone 멱등(already)", matches!(equip_clone(&ws, https), Ok("already")) && pushurl(&clone_dir) == https, "");
    ck("--write-push-authority 정상", matches!(equip_push_authority(&ws, Some("2026-10-11T00:00:00Z")), Ok("written")), "");
    ck("--write-push-authority 멱등", matches!(equip_push_authority(&ws, None), Ok("already")), "");
    let _ = std::fs::create_dir(ws.join(".claude"));
    std::fs::write(ws.join(PII_TERMS_REL), "").unwrap();
    let r1 = ev(&ws, empty);
    ck("★https 클론 + push 주소 고정 → hint_clone ok · push_scheme https · 토큰 없음 → absent · ready=False",
        !ready(&r1) && at(&r1, &["checks", "hint_clone", "status"]) == "ok"
            && at(&r1, &["checks", "hint_clone", "push_scheme"]) == "https"
            && at(&r1, &["checks", "hint_clone", "pushurl_pinned"]) == "True"
            && at(&r1, &["checks", "hint_clone", "hint_ref"]) == "present"
            && at(&r1, &["checks", "token", "status"]) == "absent", &dump(&r1));
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
    let (pt1, pt1_code) = push_transport(&ws);
    let pt1_text = crate::py::dumps_unicode(&pt1);
    let keys = |v: &PyValue| match v { PyValue::Dict(d) => d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect::<Vec<_>>(), _ => vec![] };
    ck("★--push-transport https+고정 → exit 0 · 네 키뿐 · URL·경로·토큰 0",
        pt1_code == EXIT_READY
            && keys(&pt1) == ["hint_clone_declared", "fetch_scheme", "push_scheme", "pushurl_pinned"]
            && at(&Some(pt1.clone()), &["push_scheme"]) == "https" && at(&Some(pt1.clone()), &["pushurl_pinned"]) == "True"
            && !pt1_text.contains("example.invalid") && !pt1_text.contains(&disp(&td)) && !pt1_text.contains(secret), &pt1_text);
    g(&clone_dir, &["config", "--unset-all", PUSHURL_KEY]);
    let r_unpinned = ev(&ws, empty);
    ck("★push 주소 없음 → pushurl_unpinned · ready=False · 다음 행동 sub-agent 재장착",
        !ready(&r_unpinned) && at(&r_unpinned, &["checks", "hint_clone", "status"]) == "pushurl_unpinned"
            && at(&r_unpinned, &["checks", "hint_clone", "pushurl_pinned"]) == "False"
            && steps(&r_unpinned).iter().any(|x| x == "reinstall"), &dump(&r_unpinned));
    let (pt2, pt2_code) = push_transport(&ws);
    ck("--push-transport 미고정 → exit 3(차단, usage 2 아님)",
        pt2_code == EXIT_PUSH_TRANSPORT_BLOCKED && pt2_code != EXIT_USAGE && at(&Some(pt2.clone()), &["pushurl_pinned"]) == "False",
        &crate::py::dumps_unicode(&pt2));
    ck("★재장착(already 경로)이 push 주소를 다시 고정 → ok",
        matches!(equip_clone(&ws, https), Ok("already")) && pushurl(&clone_dir) == https
            && check_clone(&ws).ok().is_some_and(|d| status_of(&d) == "ok"), &pushurl(&clone_dir));
    g(&clone_dir, &["config", "--replace-all", PUSHURL_KEY, scp]);
    let r_ssh_push = ev(&ws, empty);
    let (pt3, pt3_code) = push_transport(&ws);
    ck("★push ssh(scp형) → violation · ready=False · 메인 에스컬레이션 · --push-transport exit 3 push_scheme ssh",
        !ready(&r_ssh_push) && at(&r_ssh_push, &["checks", "hint_clone", "status"]) == "violation"
            && at(&r_ssh_push, &["checks", "hint_clone", "push_scheme"]) == "ssh"
            && actors(&r_ssh_push).iter().any(|(a, _)| a == "main")
            && pt3_code == EXIT_PUSH_TRANSPORT_BLOCKED && at(&Some(pt3.clone()), &["push_scheme"]) == "ssh"
            && !crate::py::dumps_unicode(&pt3).contains("example.invalid"), &dump(&r_ssh_push));
    ck("재장착이 ssh push 주소를 https 로 바꾼다(fetch 가 https 일 때)",
        matches!(equip_clone(&ws, https), Ok("already")) && pushurl(&clone_dir) == https, &pushurl(&clone_dir));
    g(&clone_dir, &["remote", "set-url", "origin", scp]);
    let r_ssh = ev(&ws, empty);
    ck("★ssh origin 픽스처 → violation · ready: false",
        !ready(&r_ssh) && at(&r_ssh, &["checks", "hint_clone", "status"]) == "violation"
            && at(&r_ssh, &["checks", "hint_clone", "origin_scheme"]) == "ssh"
            && at(&r_ssh, &["checks", "token", "status"]) == "present", &dump(&r_ssh));
    ck("ssh fetch 클론은 재장착이 고정하지 않는다(사람·메인)", matches!(equip_clone(&ws, https), Err(Fail::Usage(_))), "통과했다");
    g(&clone_dir, &["remote", "set-url", "origin", &o]);
    g(&clone_dir, &["config", "--replace-all", PUSHURL_KEY, &o]);
    let r_local = ev(&ws, &[]);
    let r_local_notoken = {
        let saved = std::fs::read_to_string(ws.join("envs/.env")).unwrap();
        std::fs::write(ws.join("envs/.env"), format!("{CLONE_DIR_KEY}={}\n", disp(&clone_dir))).unwrap();
        let r = ev(&ws, empty);
        std::fs::write(ws.join("envs/.env"), saved).unwrap();
        r
    };
    ck("로컬 경로 원격 → invalid · ready=False · 토큰 없으면 not_required(로컬만 · #7)",
        !ready(&r_local) && at(&r_local, &["checks", "hint_clone", "status"]) == "invalid"
            && at(&r_local_notoken, &["checks", "token", "status"]) == "not_required", &dump(&r_local_notoken));
    g(&clone_dir, &["remote", "set-url", "origin", &format!("https://x:{secret}@example.invalid/p.git")]);
    g(&clone_dir, &["config", "--replace-all", PUSHURL_KEY, https]);
    let r4 = ev(&ws, empty);
    ck("origin URL 자격증명 → invalid · 출력 무비밀",
        at(&r4, &["checks", "hint_clone", "status"]) == "invalid" && !dump(&r4).contains(secret), "");
    g(&clone_dir, &["remote", "set-url", "origin", https]);
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
    // #2 envs/.env mode (stat only, narrow only): fresh fixture files whose mode alone is changed.
    let env_secret = "HF_TOKEN=hf_SELFTEST_ENV_VALUE_77aa";
    let mode_ws = td.join("mode-ws");
    std::fs::create_dir_all(mode_ws.join("envs")).unwrap();
    let env_file = mode_ws.join(ENV_FILE_REL);
    let set_mode = |mode: u32| {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&env_file, format!("{env_secret}\n")).unwrap();
        std::fs::set_permissions(&env_file, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    let mode_of = || {
        use std::os::unix::fs::PermissionsExt;
        std::fs::symlink_metadata(&env_file).map(|m| m.permissions().mode() & 0o7777).ok()
    };
    let record = |shared: Option<&str>| narrow_env_record(&mode_ws, shared);
    let field_of = |v: &PyValue, k: &str| crate::py::display_opt(crate::py::get(v, k));
    set_mode(0o664);
    let r_wide = ev(&mode_ws, empty);
    ck("판정 env_file_mode 664 → wide · 다음 행동 sub-agent --narrow-env-mode · 판정은 고치지 않는다",
        at(&r_wide, &["checks", "env_file_mode", "status"]) == "wide"
            && steps(&r_wide).iter().any(|x| x == "narrow_env_mode") && mode_of() == Some(0o664), &dump(&r_wide));
    let mut outputs = String::new();
    for (from, to, action) in [(0o664, 0o600, "narrowed"), (0o640, 0o600, "narrowed"), (0o700, 0o600, "narrowed"),
                               (0o600, 0o600, "unchanged"), (0o400, 0o400, "unchanged")] {
        set_mode(from);
        let (rec, acted) = record(None);
        outputs.push_str(&crate::py::dumps_unicode(&rec));
        ck(&format!("--narrow-env-mode {from:o} → {to:o} {action}"),
            acted && field_of(&rec, "action") == action && field_of(&rec, "before") == format!("{from:o}")
                && field_of(&rec, "after") == format!("{to:o}") && mode_of() == Some(to)
                && field_of(&rec, "provenance") == "measured" && field_of(&rec, "path_rel") == ENV_FILE_REL,
            &crate::py::dumps_unicode(&rec));
    }
    std::fs::remove_file(&env_file).unwrap();
    let (rec_absent, acted_absent) = record(None);
    ck("--narrow-env-mode 부재 → absent", acted_absent && field_of(&rec_absent, "action") == "absent"
        && field_of(&rec_absent, "before") == "None", &crate::py::dumps_unicode(&rec_absent));
    let target = mode_ws.join("real.env");
    std::fs::write(&target, format!("{env_secret}\n")).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o664)).unwrap();
    }
    std::os::unix::fs::symlink(&target, &env_file).unwrap();
    let (rec_link, acted_link) = record(None);
    let target_mode = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(&target).map(|m| m.permissions().mode() & 0o7777).ok()
    };
    ck("--narrow-env-mode 심볼릭 링크 → unknown · 따라가지 않는다(대상 664 그대로) · 교정 ✗",
        !acted_link && field_of(&rec_link, "action") == "unknown" && target_mode == Some(0o664)
            && at(&ev(&mode_ws, empty), &["checks", "env_file_mode", "status"]) == "unknown", &crate::py::dumps_unicode(&rec_link));
    outputs.push_str(&crate::py::dumps_unicode(&rec_absent));
    outputs.push_str(&crate::py::dumps_unicode(&rec_link));
    std::fs::remove_file(&env_file).unwrap();
    set_mode(0o664);
    let (rec_shared, _) = record(Some("shared"));
    let (rec_single, _) = record(Some("distinct"));
    outputs.push_str(&crate::py::dumps_unicode(&rec_shared));
    outputs.push_str(&dump(&r_wide));
    ck("covers_topologies: 메인 판정 shared → [single, multi] · 그 밖 → [single] · 출처 relayed",
        crate::py::display_opt(crate::py::get(&rec_shared, "covers_topologies")) == "['single', 'multi']"
            && field_of(&rec_shared, "covers_source") == "relayed:sub_workspace_shared=shared"
            && crate::py::display_opt(crate::py::get(&rec_single, "covers_topologies")) == "['single']"
            && field_of(&record(None).0, "covers_source") == "not-relayed", &crate::py::dumps_unicode(&rec_shared));
    ck("★envs/.env 내용(키 이름·값) 0바이트 출력 · 경로 값 0",
        !outputs.contains("HF_TOKEN") && !outputs.contains("hf_SELFTEST") && !outputs.contains(&disp(&td)), &outputs);
    drop(_net);
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
    prog: "check_sub_hint_equip.sh",
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
        flag(&["--push-transport"]),
        flag(&["--narrow-env-mode"]),
        store(&["--sub-workspace-shared"]).choices(WORKSPACE_SHARED_CHOICES),
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
    let (push_transport_mode, narrow_mode) = (a.flag(7), a.flag(8));
    let run = || -> Result<i32, Fail> {
        if push_transport_mode && (a.flag(2) || a.flag(4) || narrow_mode) {
            return Err(Fail::Usage("--push-transport 는 읽기 전용 판정이다 — 장착 플래그(--clone·--write-push-authority·--narrow-env-mode)와 함께 쓰지 않는다".into()));
        }
        if a.text(9).is_some() && !narrow_mode {
            return Err(Fail::Usage("--sub-workspace-shared 는 --narrow-env-mode 의 기록에만 실린다".into()));
        }
        if push_transport_mode {
            let (answer, code) = push_transport(&ws);
            println!("{}", crate::py::dumps_unicode(&answer));
            return Ok(code);
        }
        if a.flag(2) {
            let r = equip_clone(&ws, a.text(3).unwrap_or(""))?;
            println!("{{\"equip\": \"clone\", \"result\": \"{r}\"}}");
        }
        if a.flag(4) {
            let r = equip_push_authority(&ws, a.text(5))?;
            println!("{{\"equip\": \"push_authority\", \"result\": \"{r}\"}}");
        }
        let mut corrected = true;
        if narrow_mode {
            let (record, acted) = narrow_env_record(&ws, a.text(9));
            println!("{}", crate::py::dumps_unicode(&record));
            corrected = acted;
        }
        if a.flag(2) || a.flag(4) || narrow_mode {
            return Ok(if corrected { EXIT_READY } else { EXIT_EQUIP_FAILED });
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
