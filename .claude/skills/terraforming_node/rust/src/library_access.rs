//! `library_access.py` — issue/revoke the library access point (ssh forced-command gateway) of a Sub's wiki-desk.
//!
//! The library (`__llm-wiki`) is one, on the Main; access points are N. A Sub gets one librarian command, not a shell:
//! `command="<node> <repo>/.claude/skills/wiki-desk/src/gateway.ts --node <node_id>",restrict <sub key>`. `--apply`/`--revoke`
//! change the Main's authorized_keys and need the human approval transcript (`--approved-by`, HITL every time).
//! Exit: 0 success · 2 input error · 3 blocked (preflight failure · approval missing) · 1 uncaught exception of the original.

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::proc;
use crate::py::{self, get, s};
use crate::raise::Raised;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pyyaml;
use std::path::{Path, PathBuf};

pub const GATEWAY_REL: &str = ".claude/skills/wiki-desk/src/gateway.ts";
pub const NODE_MAJOR_MIN: i64 = 24;
pub const KEY_NAME: &str = "wiki_desk_ed25519";
pub const KNOWN_HOSTS_NAME: &str = "wiki_desk_known_hosts";
pub const MARKER: &str = "wiki-desk-gateway:";
pub const HOST_KEY: &str = "/etc/ssh/ssh_host_ed25519_key.pub";
pub const SSH_HARDENING: [&str; 15] = [
    "-F",
    "/dev/null",
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=8",
    "-o",
    "StrictHostKeyChecking=yes",
    "-o",
    "IdentitiesOnly=no",
    "-o",
    "ForwardAgent=no",
    "-o",
    "ClearAllForwardings=yes",
    "-o",
];
const SSH_LAST: &str = "RequestTTY=no";
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_BLOCKED: i32 = 3;

/// Why the program stops: a `Blocked` reason (exit 3) or an exception the original lets escape (exit 1).
pub enum Stop {
    Blocked(String),
    Raised(Raised),
}
impl From<Raised> for Stop {
    fn from(r: Raised) -> Self {
        Stop::Raised(r)
    }
}

pub fn repo() -> PathBuf {
    crate::owner::parents("library_access.py", 4)
}

/// The ssh transport (replaceable in the self-test, like the original's module-level `_ssh`).
pub type Ssh<'a> = &'a dyn Fn(&PyValue, &str, Option<&str>) -> Result<proc::Completed, Raised>;

pub fn real_ssh(
    sub: &PyValue,
    remote: &str,
    stdin: Option<&str>,
) -> Result<proc::Completed, Raised> {
    let mut argv: Vec<String> = vec!["ssh".into()];
    argv.extend(SSH_HARDENING.iter().map(|x| x.to_string()));
    argv.push(SSH_LAST.into());
    argv.push(format!(
        "{}@{}",
        py::display_opt(get(sub, "ssh_user")),
        py::display_opt(get(sub, "host"))
    ));
    argv.push(remote.into());
    proc::run(&argv, stdin, Some(60))
}

fn not_dict(v: &PyValue) -> Raised {
    Raised::new(
        "AttributeError",
        &format!("'{}' object has no attribute 'get'", v.type_name()),
    )
}

fn nodes(man: &PyValue, node_id: &str) -> Result<(PyValue, PyValue), Stop> {
    let list = match get(man, "nodes") {
        Some(v) if v.is_truthy() => match v {
            PyValue::List(l) => l.clone(),
            other => {
                return Err(Stop::Raised(Raised::new(
                    "TypeError",
                    &format!("'{}' object is not iterable", other.type_name()),
                )))
            }
        },
        _ => vec![],
    };
    for n in &list {
        if !matches!(n.peel(), PyValue::Dict(_)) {
            return Err(not_dict(n).into());
        }
    }
    let role =
        |n: &PyValue, r: &str| matches!(get(n, "role"), Some(PyValue::Str(x)) if x.equals(r));
    let main = list.iter().find(|n| role(n, "main")).cloned();
    let subs: Vec<&PyValue> = list.iter().filter(|n| role(n, "sub")).collect();
    let id_of = |n: &PyValue| match get(n, "node_id") {
        Some(v) if v.is_truthy() => v.clone(),
        _ => s("sub"),
    };
    let mut sub = subs
        .iter()
        .find(|n| matches!(id_of(n), PyValue::Str(x) if x.equals(node_id)))
        .map(|n| (*n).clone());
    if sub.is_none() && node_id == "sub" && subs.len() == 1 {
        sub = Some(subs[0].clone());
    }
    let (Some(main), Some(sub)) = (main, sub) else {
        return Err(Stop::Blocked(format!(
            "manifest nodes 에서 main/sub({node_id}) 를 찾지 못했다"
        )));
    };
    for (n, who) in [(&main, "main"), (&sub, node_id)] {
        for k in ["host", "ssh_user", "work_dir"] {
            if !py::truthy(get(n, k)) {
                return Err(Stop::Blocked(format!(
                    "manifest nodes[{who}].{k} 가 비었다"
                )));
            }
        }
    }
    Ok((main, sub))
}

/// `re.match(r"v?(\d+)\.", version.strip())`.
pub fn node_major(version: &str) -> Option<i64> {
    let t = py::strip(version);
    let t = t.strip_prefix('v').unwrap_or(t);
    let digits: String = t.chars().take_while(|c| is_unicode_decimal(*c)).collect();
    if digits.is_empty() || !t[digits.len()..].starts_with('.') {
        return None;
    }
    digits.chars().try_fold(0i64, |acc, c| {
        Some(acc.checked_mul(10)? + decimal_value(c)? as i64)
    })
}
fn is_unicode_decimal(c: char) -> bool {
    agent_runtime_support::pystr::is_decimal_digit(c as u32)
}
fn decimal_value(c: char) -> Option<u32> {
    agent_runtime_support::pystr::decimal_digit_value(c as u32)
}

/// `re.search(r"^(PATTERN)$", out, re.M)` for the two line patterns of the preflight: the first line that matches.
fn line_match(out: &str, pred: impl Fn(&str) -> bool) -> Option<&str> {
    out.split('\n').find(|l| pred(l))
}
fn nonspace_run(t: &str) -> bool {
    !t.is_empty() && !t.chars().any(py::is_space)
}
fn is_version_line(l: &str) -> bool {
    // v\d+\.\S+
    let Some(r) = l.strip_prefix('v') else {
        return false;
    };
    let d = r
        .chars()
        .take_while(|c| is_unicode_decimal(*c))
        .map(char::len_utf8)
        .sum::<usize>();
    d > 0 && r[d..].starts_with('.') && nonspace_run(&r[d + 1..])
}
fn home_of(l: &str) -> Option<&str> {
    let r = l.strip_prefix("HOME=")?;
    if r.starts_with('/') && nonspace_run(r) {
        Some(r)
    } else {
        None
    }
}

fn tail_chars(t: &str, n: usize) -> String {
    let c: Vec<char> = t.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

/// Both sides' preflight; every missing piece is reported at once (facts dict in the original's key order).
pub fn preflight(sub: &PyValue, repo: &Path, ssh: Ssh) -> Result<PyDict, Raised> {
    let mut reasons: Vec<String> = vec![];
    let mut facts = PyDict::new();
    let node = proc::which("node");
    facts.set("main_node", py::opt_str(node.as_deref()));
    match &node {
        None => reasons.push(format!("메인에 node 가 없다(>= {NODE_MAJOR_MIN} 필요)")),
        Some(n) => {
            let v = proc::run(&[n.clone(), "--version".into()], None, None)?.stdout;
            if node_major(&v).unwrap_or(0) < NODE_MAJOR_MIN {
                reasons.push(format!(
                    "메인 node 가 낮다: {} (>= {NODE_MAJOR_MIN})",
                    py::strip(&v)
                ));
            }
        }
    }
    for tool in ["flock", "rsync", "rrsync"] {
        if proc::which(tool).is_none() {
            reasons.push(format!("메인에 {tool} 가 없다(동시성·반입 규정 집행 불가)"));
        }
    }
    if !repo.join("__llm-wiki").join("contract.json").is_file() {
        reasons.push(
            "메인 서고가 초기화되지 않았다(__llm-wiki/contract.json 부재 · wiki-desk init 선행)"
                .into(),
        );
    }
    let r = ssh(sub, "bash -lc 'command -v node >/dev/null && node --version; command -v rsync >/dev/null && echo rsync-ok; echo HOME=$HOME'", None)?;
    if r.returncode != 0 {
        reasons.push(format!(
            "서브에 ssh 로 닿지 못했다(rc={}): {}",
            r.returncode,
            tail_chars(py::strip(&r.stderr), 200)
        ));
    } else {
        let out = &r.stdout;
        let m = line_match(out, is_version_line);
        facts.set("sub_node_version", py::opt_str(m));
        if m.is_none_or(|v| node_major(v).unwrap_or(0) < NODE_MAJOR_MIN) {
            reasons.push(format!(
                "서브 node 가 없거나 낮다({}) — Node.js >= {NODE_MAJOR_MIN} 를 사람이 설치한다(자동 설치 ✗)",
                m.unwrap_or("None")
            ));
        }
        if !out.contains("rsync-ok") {
            reasons.push("서브에 rsync 가 없다(반입 불가)".into());
        }
        let h = out.split('\n').find_map(home_of);
        facts.set("sub_home", py::opt_str(h));
        if h.is_none() {
            reasons.push("서브 HOME 을 읽지 못했다".into());
        }
    }
    facts.set("blocked", py::list_str(&reasons));
    Ok(facts)
}

fn is_node_id(id: &str) -> bool {
    let c: Vec<char> = id.chars().collect();
    let first = |x: char| x.is_ascii_lowercase() || x.is_ascii_digit();
    !c.is_empty()
        && c.len() <= 63
        && first(c[0])
        && c[1..].iter().all(|&x| first(x) || x == '_' || x == '-')
}

/// One authorized_keys line — only values that cannot carry shell metacharacters.
pub fn gateway_line(
    repo: &Path,
    node_bin: &str,
    node_id: &str,
    pubkey: &str,
) -> Result<String, String> {
    if !is_node_id(node_id) || node_id == "main" {
        return Err(format!("node_id 형식 오류: {}", PyStr::new(node_id).repr()));
    }
    let parts = py::split_ws(pubkey);
    if parts.len() < 2 || parts[0] != "ssh-ed25519" {
        return Err("서브 공개키가 ed25519 한 줄이 아니다".into());
    }
    let cmd = format!(
        "{node_bin} {}/{GATEWAY_REL} --node {node_id}",
        repo.display()
    );
    if cmd.contains('"') || cmd.contains('\\') {
        return Err("게이트웨이 명령 경로에 인용 문자가 있다".into());
    }
    Ok(format!(
        "command=\"{cmd}\",restrict {} {} {MARKER}{node_id}",
        parts[0], parts[1]
    ))
}

/// Drop the line with the same marker (if any) and append `line`; `None` = revoke (drop only).
pub fn replace_marked(text: &str, node_id: &str, line: Option<&str>) -> String {
    let marker = format!("{MARKER}{node_id}");
    let mut kept: Vec<String> = py::splitlines(text)
        .into_iter()
        .filter(|l| !l.trim_end_matches(py::is_space).ends_with(&marker))
        .collect();
    if let Some(l) = line {
        kept.push(l.to_string());
    }
    let nl = if kept.is_empty() { "" } else { "\n" };
    format!("{}{nl}", kept.join("\n"))
}

/// Replace the YAML top-level `key:` block (until the next top-level key) by appending `block`; comments and other keys stay.
pub fn replace_top_block(text: &str, key: &str, block: Option<&str>) -> String {
    let mut out: Vec<String> = vec![];
    let mut skipping = false;
    let head = format!("{key}:");
    for ln in py::splitlines(text) {
        if ln.starts_with(&head) {
            skipping = true;
            continue;
        }
        if skipping && (ln.starts_with(' ') || ln.starts_with('\t') || py::strip(&ln).is_empty()) {
            continue;
        }
        skipping = false;
        out.push(ln);
    }
    while out.last().is_some_and(|l| py::strip(l).is_empty()) {
        out.pop();
    }
    let body = format!("{}\n", out.join("\n"));
    match block {
        Some(b) if !b.is_empty() => format!("{body}\n{b}"),
        _ => body,
    }
}

pub fn key_name(node_id: &str, virtual_: bool) -> String {
    if virtual_ {
        format!("wiki_desk_{node_id}_ed25519")
    } else {
        KEY_NAME.into()
    }
}

fn jd(v: Option<&PyValue>) -> Result<String, Raised> {
    match v {
        Some(x) => x
            .dumps_unicode()
            .map(|t| py::text(&t))
            .map_err(|e| Raised::new("TypeError", &e.to_string())),
        None => Err(Raised::new("KeyError", "missing key")),
    }
}

fn key_err(k: &str) -> Raised {
    Raised::new("KeyError", &PyStr::new(k).repr())
}
fn idx<'a>(d: &'a PyValue, k: &str) -> Result<&'a PyValue, Raised> {
    get(d, k).ok_or_else(|| key_err(k))
}

pub fn main_library_block(main: &PyValue, aps: &PyDict) -> Result<String, Raised> {
    let mut lines = vec![
        "library:   # 서고 접근자(wiki-desk) — 메인은 local · 서브 접근점은 library_access.py 가 발급한다(plan_26101912)".to_string(),
        "  transport: local".into(),
        "  wiki_dir: __llm-wiki".into(),
        format!("  repo_root: {}", jd(Some(idx(main, "work_dir")?))?),
    ];
    if !aps.is_empty() {
        lines.push("  access_points:".into());
        let mut ids: Vec<&PyStr> = aps.iter().map(|(k, _)| k).collect();
        ids.sort_by(|a, b| a.code_points().cmp(b.code_points()));
        for nid in ids {
            let ap = aps.get(&py::text(nid)).expect("key present");
            lines.push(format!("    {}:", py::text(nid)));
            lines.push(format!(
                "      identity_file: {}",
                jd(Some(idx(ap, "identity_file")?))?
            ));
            lines.push(format!(
                "      known_hosts: {}",
                jd(Some(idx(ap, "known_hosts")?))?
            ));
            if py::truthy(get(ap, "host_of")) {
                lines.push(format!(
                    "      host_of: {}",
                    py::display_opt(get(ap, "host_of"))
                ));
            }
        }
    }
    Ok(format!("{}\n", lines.join("\n")))
}

pub fn sub_library_block(main: &PyValue, node_id: &str, ap: &PyValue) -> Result<String, Raised> {
    Ok(format!(
        "{}\n",
        [
            "library:   # 서고 접근자 — 메인 서고에 ssh 강제명령 게이트웨이로 닿는다(경로만 다름 · 메인이 발급 · plan_26101912)".to_string(),
            "  transport: ssh".into(),
            format!("  host: {}", jd(Some(idx(main, "host")?))?),
            format!("  user: {}", jd(Some(idx(main, "ssh_user")?))?),
            format!("  repo_root: {}", jd(Some(idx(main, "work_dir")?))?),
            "  wiki_dir: __llm-wiki".into(),
            format!("  identity_file: {}", jd(Some(idx(ap, "identity_file")?))?),
            format!("  known_hosts: {}", jd(Some(idx(ap, "known_hosts")?))?),
            format!("  node_id: {node_id}"),
        ]
        .join("\n")
    ))
}

/// `shlex.quote`.
pub fn shquote(t: &str) -> String {
    if t.is_empty() {
        return "''".into();
    }
    let safe = t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c));
    if safe {
        t.to_string()
    } else {
        format!("'{}'", t.replace('\'', "'\"'\"'"))
    }
}

fn read_text(p: &Path) -> Result<String, Raised> {
    let b = std::fs::read(p).map_err(|e| Raised::os(&e, &p.display().to_string()))?;
    String::from_utf8(b).map_err(|e| {
        Raised::new(
            "UnicodeDecodeError",
            &format!("'utf-8' codec can't decode bytes: {e}"),
        )
    })
}
fn write_text(p: &Path, t: &str) -> Result<(), Raised> {
    std::fs::write(p, t).map_err(|e| Raised::os(&e, &p.display().to_string()))
}

fn load_yaml(p: &Path) -> Result<PyValue, Raised> {
    let b = std::fs::read(p).map_err(|e| Raised::os(&e, &p.display().to_string()))?;
    let v = pyyaml::safe_load(&b, &PyStr::new(&p.display().to_string()))
        .map_err(|e| Raised::yaml(&e))?;
    Ok(if v.is_truthy() {
        v
    } else {
        PyValue::Dict(PyDict::new())
    })
}

pub struct Args {
    pub topology: String,
    pub node: String,
    pub plan: bool,
    pub revoke: bool,
    pub approved_by: Option<String>,
    pub host_of: Option<String>,
}

fn rel(repo: &Path, p: &Path) -> String {
    p.strip_prefix(repo)
        .map(|x| x.display().to_string())
        .unwrap_or_default()
}

pub fn run(a: &Args, repo: &Path, ssh: Ssh) -> Result<i32, Stop> {
    let man_path = repo.join("output").join(&a.topology).join("manifest.yaml");
    let man = load_yaml(&man_path)?;
    if !matches!(man.peel(), PyValue::Dict(_)) {
        return Err(not_dict(&man).into());
    }
    let mut aps: PyDict = match get(&man, "library") {
        Some(l) if l.is_truthy() => {
            if !matches!(l, PyValue::Dict(_)) {
                return Err(not_dict(l).into());
            }
            match get(l, "access_points") {
                Some(PyValue::Dict(d)) => d.clone(),
                Some(x) if x.is_truthy() => {
                    return Err(Raised::new("TypeError", "access_points is not a mapping").into())
                }
                _ => PyDict::new(),
            }
        }
        _ => PyDict::new(),
    };
    let host_of: Option<String> = match a.host_of.as_deref().filter(|x| !x.is_empty()) {
        Some(h) => Some(h.to_string()),
        None if a.revoke => aps
            .get(&a.node)
            .filter(|x| x.is_truthy())
            .and_then(|ap| get(ap, "host_of"))
            .filter(|x| x.is_truthy())
            .map(py::display),
        None => None,
    };
    let virtual_ = host_of.is_some();
    if virtual_ && host_of.as_deref() == Some(a.node.as_str()) {
        return Err(Stop::Blocked("--host-of 는 다른 실노드여야 한다".into()));
    }
    let (main, sub) = nodes(&man, host_of.as_deref().unwrap_or(&a.node))?;
    let sub_man_path = man_path.parent().unwrap().join("sub_manifest.yaml");
    if a.plan {
        let facts = preflight(&sub, repo, ssh)?;
        let blocked = matches!(facts.get("blocked"), Some(PyValue::List(l)) if !l.is_empty());
        let will = vec![
            "서브 전용 키 생성(없으면) · 공개키 회수".to_string(),
            "메인 호스트 키 → 서브 known_hosts".into(),
            format!("메인 authorized_keys 강제명령 줄({MARKER}{})", a.node),
            format!(
                "manifest library 블록: {} · {}",
                rel(repo, &man_path),
                rel(repo, &sub_man_path)
            ),
        ];
        let out = py::dict(vec![
            ("mode", s("plan")),
            ("node_id", s(&a.node)),
            ("preflight", PyValue::Dict(facts)),
            ("will", py::list_str(&will)),
        ]);
        println!("{}", py::dumps_pretty(&out));
        return Ok(if blocked { EXIT_BLOCKED } else { 0 });
    }
    let approved = a.approved_by.clone().unwrap_or_default();
    if py::strip(&approved).is_empty() {
        return Err(Stop::Blocked(
            "메인 authorized_keys 변경은 사람 승인 전사(--approved-by)가 있어야 한다(HITL · D11)"
                .into(),
        ));
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let ak = Path::new(&home).join(".ssh").join("authorized_keys");
    if a.revoke {
        if ak.is_file() {
            write_text(&ak, &replace_marked(&read_text(&ak)?, &a.node, None))?;
        }
        let gone = remove(&mut aps, &a.node)
            .filter(|x| x.is_truthy())
            .unwrap_or(PyValue::Dict(PyDict::new()));
        if virtual_ {
            let kf = match get(&gone, "identity_file") {
                Some(x) if x.is_truthy() => py::display(x),
                _ => format!("~/.ssh/{}", key_name(&a.node, true)),
            };
            let r = ssh(
                &sub,
                &format!("rm -f {} {}", shquote(&kf), shquote(&format!("{kf}.pub"))),
                None,
            )?;
            if r.returncode != 0 {
                return Err(Stop::Blocked(format!(
                    "가상 접근점 키 삭제 실패: {}",
                    tail_chars(py::strip(&r.stderr), 200)
                )));
            }
        }
        let block = main_library_block(&main, &aps)?;
        write_text(
            &man_path,
            &replace_top_block(&read_text(&man_path)?, "library", Some(&block)),
        )?;
        let out = py::dict(vec![
            ("mode", s("revoke")),
            ("node_id", s(&a.node)),
            ("approved_by", s(&approved)),
        ]);
        println!("{}", py::dumps_unicode(&out));
        return Ok(0);
    }
    let facts = preflight(&sub, repo, ssh)?;
    if let Some(PyValue::List(l)) = facts.get("blocked") {
        if !l.is_empty() {
            let reasons: Vec<String> = l.iter().map(py::display).collect();
            return Err(Stop::Blocked(reasons.join("; ")));
        }
    }
    let home_sub = py::display_opt(facts.get("sub_home"));
    let key = format!("{home_sub}/.ssh/{}", key_name(&a.node, virtual_));
    let r = ssh(
        &sub,
        &format!(
            "umask 077; mkdir -p ~/.ssh; test -f {} || ssh-keygen -q -t ed25519 -N '' -C {} -f {}; cat {}",
            shquote(&key),
            shquote(&format!("{MARKER}{}", a.node)),
            shquote(&key),
            shquote(&format!("{key}.pub"))
        ),
        None,
    )?;
    if r.returncode != 0 {
        return Err(Stop::Blocked(format!(
            "서브 키 생성·회수 실패: {}",
            tail_chars(py::strip(&r.stderr), 200)
        )));
    }
    let pub_lines = py::splitlines(py::strip(&r.stdout));
    let Some(pubkey) = pub_lines.last().cloned() else {
        return Err(Raised::new("IndexError", "list index out of range").into());
    };
    let hk_path = Path::new(HOST_KEY);
    if !hk_path.is_file() {
        return Err(Stop::Blocked(format!("메인 호스트 키가 없다: {HOST_KEY}")));
    }
    let hk_text = read_text(hk_path)?;
    let hk = py::split_ws(&hk_text);
    if hk.len() < 2 {
        return Err(Raised::new("IndexError", "list index out of range").into());
    }
    let kh = format!("{home_sub}/.ssh/{KNOWN_HOSTS_NAME}");
    let q = shquote(&kh);
    let main_host = py::display_opt(get(&main, "host"));
    let r = ssh(
        &sub,
        &format!("umask 077; touch {q}; grep -v -F {} {q} > {q}.tmp || true; cat >> {q}.tmp; mv {q}.tmp {q}", shquote(&format!("{main_host} "))),
        Some(&format!("{main_host} {} {}\n", hk[0], hk[1])),
    )?;
    if r.returncode != 0 {
        return Err(Stop::Blocked(format!(
            "서브 known_hosts 기록 실패: {}",
            tail_chars(py::strip(&r.stderr), 200)
        )));
    }
    let main_node = py::display_opt(facts.get("main_node"));
    let line = gateway_line(repo, &main_node, &a.node, &pubkey).map_err(Stop::Blocked)?;
    if let Some(parent) = ak.parent() {
        if !parent.is_dir() {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(parent)
                .map_err(|e| Raised::os(&e, &parent.display().to_string()))?;
        }
    }
    let before = if ak.is_file() {
        read_text(&ak)?
    } else {
        String::new()
    };
    write_text(&ak, &replace_marked(&before, &a.node, Some(&line)))?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ak, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| Raised::os(&e, &ak.display().to_string()))?;
    }
    let mut ap = vec![("identity_file", s(&key)), ("known_hosts", s(&kh))];
    if let Some(h) = &host_of {
        ap.push(("host_of", s(h)));
    }
    let ap = py::dict(ap);
    aps.set(&a.node, ap.clone());
    let block = main_library_block(&main, &aps)?;
    write_text(
        &man_path,
        &replace_top_block(&read_text(&man_path)?, "library", Some(&block)),
    )?;
    if let Some(h) = &host_of {
        let out = py::dict(vec![
            ("mode", s("apply")),
            ("node_id", s(&a.node)),
            ("host_of", s(h)),
            ("approved_by", s(&approved)),
            ("authorized_keys_marker", s(&format!("{MARKER}{}", a.node))),
            (
                "virtual_library_block",
                s(&sub_library_block(&main, &a.node, &ap)?),
            ),
        ]);
        println!("{}", py::dumps_pretty(&out));
        return Ok(0);
    }
    if sub_man_path.is_file() {
        let blk = sub_library_block(&main, &a.node, &ap)?;
        write_text(
            &sub_man_path,
            &replace_top_block(&read_text(&sub_man_path)?, "library", Some(&blk)),
        )?;
    }
    let out = py::dict(vec![
        ("mode", s("apply")),
        ("node_id", s(&a.node)),
        ("approved_by", s(&approved)),
        ("preflight", PyValue::Dict(facts)),
        ("authorized_keys_marker", s(&format!("{MARKER}{}", a.node))),
        ("sub_manifest_updated", PyValue::Bool(sub_man_path.is_file())),
        ("next", s("render_sub_env.py --sub-manifest + sync_to_sub.sh --provision 로 서브 manifest·wiki-desk 를 배달한다")),
    ]);
    println!("{}", py::dumps_pretty(&out));
    Ok(0)
}

/// `dict.pop(key, None)` keeping the order of the other entries.
fn remove(d: &mut PyDict, key: &str) -> Option<PyValue> {
    let found = d.get(key).cloned()?;
    let mut rest = PyDict::new();
    for (k, v) in d.iter() {
        if !k.equals(key) {
            rest.insert(k.clone(), v.clone());
        }
    }
    *d = rest;
    Some(found)
}

fn self_test(repo: &Path) -> i32 {
    let mut ok = true;
    let mut chk = |c: bool, what: &str| {
        println!("{}{}", if c { "  [PASS] " } else { "  [FAIL] " }, what);
        ok = ok && c;
    };
    let pubk = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIexample host";
    let line = gateway_line(repo, "/usr/bin/node", "sub", pubk).unwrap_or_default();
    chk(
        line.starts_with("command=\"/usr/bin/node ")
            && line.contains(",restrict ssh-ed25519 ")
            && line.ends_with(&format!("{MARKER}sub")),
        "강제명령 줄 = command + restrict + 마커",
    );
    chk(
        line.contains(" --node sub\""),
        "node_id 는 메인이 줄에 묶는다",
    );
    for bad in ["main", "Sub", "a;rm", ""] {
        chk(
            gateway_line(repo, "/usr/bin/node", bad, pubk).is_err(),
            &format!("잘못된 node_id 거부: {}", PyStr::new(bad).repr()),
        );
    }
    chk(
        gateway_line(repo, "/usr/bin/node", "sub", "ssh-rsa AAAA x").is_err(),
        "ed25519 외 키 거부",
    );
    let ak =
        "ssh-ed25519 AAAA me@main\ncommand=\"x\",restrict ssh-ed25519 OLD wiki-desk-gateway:sub\n";
    let new = replace_marked(ak, "sub", Some(&line));
    chk(
        new.matches(&format!("{MARKER}sub")).count() == 1
            && !new.contains("OLD")
            && new.contains("me@main"),
        "같은 마커 줄만 교체(멱등) · 남의 줄 보존",
    );
    let gone = replace_marked(&new, "sub", None);
    chk(
        !gone.contains(&format!("{MARKER}sub")) && gone.contains("me@main"),
        "회수 = 그 줄만 삭제",
    );
    let y = "self_role: main\n# 주석\nlibrary:\n  transport: old\n  wiki_dir: x\nnodes:\n  - role: main\n";
    let out = replace_top_block(y, "library", Some("library:\n  transport: local\n"));
    chk(
        !out.contains("transport: old")
            && out.contains("# 주석")
            && out.contains("nodes:\n  - role: main")
            && out.trim_end().ends_with("transport: local"),
        "manifest library 블록만 교체(주석·다른 키 보존)",
    );
    let main = py::dict(vec![
        ("host", s("203.0.113.10")),
        ("ssh_user", s("u")),
        ("work_dir", s("/w/r")),
    ]);
    let blk = sub_library_block(
        &main,
        "sub",
        &py::dict(vec![
            ("identity_file", s("/h/.ssh/k")),
            ("known_hosts", s("/h/.ssh/kh")),
        ]),
    )
    .unwrap_or_default();
    let lib = pyyaml::safe_load(blk.as_bytes(), &PyStr::new("m.yaml"))
        .ok()
        .and_then(|v| get(&v, "library").cloned());
    let want = py::dict(vec![
        ("transport", s("ssh")),
        ("host", s("203.0.113.10")),
        ("user", s("u")),
        ("repo_root", s("/w/r")),
        ("wiki_dir", s("__llm-wiki")),
        ("identity_file", s("/h/.ssh/k")),
        ("known_hosts", s("/h/.ssh/kh")),
        ("node_id", s("sub")),
    ]);
    chk(lib.as_ref() == Some(&want), "서브 library 블록 = wiki-desk accessor.ts 스키마(transport·host·user·repo_root·wiki_dir·identity_file·known_hosts·node_id)");
    let fake_sub = py::dict(vec![("ssh_user", s("u")), ("host", s("h"))]);
    for (out, want) in [
        ("rsync-ok\nHOME=/h\n", "서브 node 가 없거나 낮다"),
        ("v22.1.0\nrsync-ok\nHOME=/h\n", "서브 node 가 없거나 낮다"),
        ("v26.7.0\nHOME=/h\n", "서브에 rsync 가 없다"),
    ] {
        let fake = move |_: &PyValue, _: &str, _: Option<&str>| {
            Ok(proc::Completed {
                returncode: 0,
                stdout: out.to_string(),
                stderr: String::new(),
            })
        };
        let blocked = preflight(&fake_sub, repo, &fake)
            .ok()
            .and_then(|f| f.get("blocked").cloned());
        let hit = matches!(&blocked, Some(PyValue::List(l)) if l.iter().any(|r| py::display(r).contains(want)));
        chk(
            hit,
            &format!("사전검사 차단 사유: {want} ({})", py::split_ws(out)[0]),
        );
    }
    let fine = |_: &PyValue, _: &str, _: Option<&str>| {
        Ok(proc::Completed {
            returncode: 0,
            stdout: "v26.7.0\nrsync-ok\nHOME=/h\n".into(),
            stderr: String::new(),
        })
    };
    let blocked = preflight(&fake_sub, repo, &fine)
        .ok()
        .and_then(|f| f.get("blocked").cloned());
    let none_sub = matches!(&blocked, Some(PyValue::List(l)) if !l.iter().any(|r| py::display(r).contains("서브")));
    chk(none_sub, "서브 조건 충족 → 서브 쪽 차단 사유 없음");
    chk(
        key_name("sub", false) == KEY_NAME && key_name("sub2", true) == "wiki_desk_sub2_ed25519",
        "가상 접근점 키 이름 = node_id 별",
    );
    let mut aps = PyDict::new();
    aps.set(
        "sub",
        py::dict(vec![("identity_file", s("/k")), ("known_hosts", s("/kh"))]),
    );
    aps.set(
        "sub2",
        py::dict(vec![
            ("identity_file", s("/k2")),
            ("known_hosts", s("/kh")),
            ("host_of", s("sub")),
        ]),
    );
    let mb = main_library_block(&py::dict(vec![("work_dir", s("/w/r"))]), &aps).unwrap_or_default();
    chk(
        mb.contains("    sub2:\n")
            && mb.contains("      host_of: sub\n")
            && mb.matches("host_of").count() == 1,
        "가상 접근점은 host_of 출처를 남긴다(실노드는 없음)",
    );
    println!(
        "[library_access] self-test {}",
        if ok { "PASS" } else { "FAIL" }
    );
    if ok {
        0
    } else {
        1
    }
}

pub static SPEC: Spec = Spec {
    prog: "library_access.py",
    usage: include_str!("../help/library_access.usage"),
    help: include_str!("../help/library_access.help"),
    args: &[
        HELP,
        store(&["--topology"]).choices(&["single", "multi"]),
        store(&["--node"]),
        flag(&["--plan"]).group(1),
        flag(&["--apply"]).group(1),
        flag(&["--revoke"]).group(1),
        flag(&["--self-test"]).group(1),
        store(&["--approved-by"]),
        store(&["--host-of"]),
    ],
    subs: None,
};

pub fn main(args: &[String]) -> i32 {
    let p = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    let repo = repo();
    if p.flag(6) {
        return self_test(&repo);
    }
    let topology = p.text(1).unwrap_or("");
    if topology.is_empty() || !(p.flag(3) || p.flag(4) || p.flag(5)) {
        argp::print_usage_stderr(&SPEC);
        return EXIT_USAGE;
    }
    let a = Args {
        topology: topology.to_string(),
        node: p.text(2).unwrap_or("sub").to_string(),
        plan: p.flag(3),
        revoke: p.flag(5),
        approved_by: p.text(7).map(String::from),
        host_of: p.text(8).map(String::from),
    };
    match run(&a, &repo, &real_ssh) {
        Ok(c) => c,
        Err(Stop::Blocked(m)) => {
            eprintln!("[library_access] BLOCKED: {m}");
            EXIT_BLOCKED
        }
        Err(Stop::Raised(r)) => crate::raise::exit(&r),
    }
}
