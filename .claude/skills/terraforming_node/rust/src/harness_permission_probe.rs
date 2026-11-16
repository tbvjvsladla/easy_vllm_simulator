//! Port of `harness_permission_probe.py` — bounded work-permission observations executed INSIDE the requested native tool
//! (scratch write/git, workspace git metadata markers, write-open of protected/build files without modification, Docker read,
//! library query, hint clone markers). Same command line, exit codes and `HARNESS_PERMISSION_PROBE=<json>` line.
//!
//! Intentional difference: the original executed the delivered Python reader `manifest_contract_compat_owner.py` (after an
//! integrity check of its bytes) to read the manifest/compose YAML; the port reads them natively (`yaml.safe_load` semantics of
//! `agent_runtime_support::pyyaml`, `effective_model_source` inline), so there is no Python helper to verify or run.

use crate::argp::{self, store, Outcome, Spec, HELP};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{float_repr, PyValue};
use agent_runtime_support::pyyaml;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const MARKER: &str = "HARNESS_PERMISSION_PROBE=";
pub const SCHEMA_VERSION: i64 = 1;
pub const PROTECTED_FILES: [&str; 6] = ["AGENTS.md", "CLAUDE.md", "Agent_Card.json", ".claude/settings.local.json",
    ".codex/config.toml", ".agents/skills/wiki-desk/SKILL.md"];
pub const PROTECTED_FILES_MAIN: [&str; 2] = [".codex/config.toml", ".agents/skills/wiki-desk/SKILL.md"];
pub const BUILD_FILES: [&str; 2] = ["docker-compose.yaml", "requirements.txt"];
pub const DOCKERFILES: [&str; 3] = ["Dockerfile", "Dockerfile.source-build", "Dockerfile.source-build-upstage"];

pub fn protected_files(role: &str) -> &'static [&'static str] {
    if role == "main" { &PROTECTED_FILES_MAIN } else { &PROTECTED_FILES }
}

/// `protected_file_expectations(role)`: role ownership defines the write-open expectation.
pub fn protected_file_expectations(role: &str) -> Vec<(&'static str, &'static str)> {
    protected_files(role).iter().map(|p| (*p, if role == "main" { "allowed" } else { "denied" })).collect()
}

/// `_command(argv, cwd)`: (returncode, stdout) or (None, "") when the program cannot start or times out (20 s).
pub fn command(argv: &[&str], cwd: &Path) -> (Option<i32>, String) {
    let mut c = Command::new(argv[0]);
    c.args(&argv[1..]).current_dir(cwd).stdin(Stdio::inherit()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if argv[0] == "git" {
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().starts_with("GIT_") {
                c.env_remove(&k);
            }
        }
        c.env("GIT_CONFIG_NOSYSTEM", "1").env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_OPTIONAL_LOCKS", "0").env("GIT_TERMINAL_PROMPT", "0");
    }
    let Ok(mut child) = c.spawn() else { return (None, String::new()) };
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let to = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = out.read_to_end(&mut b);
        b
    });
    let te = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = err.read_to_end(&mut b);
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(_) => return (None, String::new()),
        }
        if start.elapsed() >= Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            return (None, String::new());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = String::from_utf8_lossy(&to.join().unwrap_or_default()).into_owned();
    let _ = te.join();
    use std::os::unix::process::ExitStatusExt;
    (Some(status.code().unwrap_or_else(|| -status.signal().unwrap_or(0))), stdout)
}

fn denied(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(13) | Some(1) | Some(30))
}

/// `_write_marker(directory, nonce)`: `Err` = the marker's cleanup failed (the original lets that OSError escape).
pub fn write_marker(directory: &Path, nonce: &str) -> Result<&'static str, std::io::Error> {
    let marker = directory.join(format!(".harness-permission-{nonce}"));
    let f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
        .custom_flags(libc_o_nofollow()).open(&marker);
    let mut f = match f {
        Ok(f) => f,
        Err(e) => return Ok(if denied(&e) { "denied" } else { "unobserved" }),
    };
    let payload = nonce.as_bytes();
    let outcome = (|| -> Result<&'static str, std::io::Error> {
        let n = f.write(payload)?;
        if n != payload.len() {
            return Ok("failed");
        }
        f.sync_all()?;
        Ok(if std::fs::read(&marker)? == payload { "allowed" } else { "failed" })
    })();
    drop(f);
    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => if denied(&e) { "denied" } else { "unobserved" },
    };
    std::fs::remove_file(&marker)?;
    Ok(outcome)
}

/// Linux `O_NOFOLLOW` / `O_DIRECTORY` (the arm/aarch64 values differ from x86).
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
const O_NOFOLLOW: i32 = 0o100000;
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
const O_DIRECTORY: i32 = 0o40000;
#[cfg(not(any(target_arch = "aarch64", target_arch = "arm")))]
const O_NOFOLLOW: i32 = 0o400000;
#[cfg(not(any(target_arch = "aarch64", target_arch = "arm")))]
const O_DIRECTORY: i32 = 0o200000;

const fn libc_o_nofollow() -> i32 {
    O_NOFOLLOW
}

/// `_write_open(path)` → (outcome, preserved).
pub fn write_open(path: &Path) -> (&'static str, Option<bool>) {
    let meta = std::fs::symlink_metadata(path);
    if meta.as_ref().is_ok_and(|m| m.file_type().is_symlink()) || !path.is_file() {
        return ("unobserved", None);
    }
    let Ok(before) = std::fs::read(path) else { return ("unobserved", None) };
    let outcome = match std::fs::OpenOptions::new().write(true).custom_flags(libc_o_nofollow()).open(path) {
        Ok(_) => "allowed",
        Err(e) => if denied(&e) { "denied" } else { "unobserved" },
    };
    match std::fs::read(path) {
        Ok(after) => (outcome, Some(after == before)),
        Err(_) => ("unobserved", None),
    }
}

fn resolve(p: &Path) -> PathBuf {
    crate::check_sub_hint_equip::resolve(p)
}

pub fn git_dirs(root: &Path) -> Option<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = vec![];
    for flag in ["--absolute-git-dir", "--git-common-dir"] {
        let (rc, out) = command(&["git", "rev-parse", flag], root);
        let t = crate::py::strip(&out);
        if rc != Some(0) || t.is_empty() {
            return None;
        }
        let p = Path::new(t);
        let p = if p.is_absolute() { resolve(p) } else { resolve(&root.join(p)) };
        if !p.is_dir() {
            return None;
        }
        if !found.contains(&p) {
            found.push(p);
        }
    }
    Some(found)
}

pub fn hint_root(workspace: &Path) -> Option<PathBuf> {
    let b = std::fs::read(workspace.join("envs/.env")).ok()?;
    let text = crate::py::text(&agent_runtime_support::pyjson::decode_file_text(&b).ok()?);
    for line in crate::py::splitlines(&text) {
        let st = crate::py::strip(&line);
        if st.starts_with("HINT_CLONE_DIR=") {
            let value = crate::py::strip(st.split_once('=').unwrap().1).trim_matches(['"', '\'']).to_string();
            let p = PathBuf::from(&value);
            if value.is_empty() || !p.is_absolute() || !p.is_dir() {
                return None;
            }
            let p = resolve(&p);
            if p == workspace || p.starts_with(workspace) || workspace.starts_with(&p) {
                return None;
            }
            let (rc, out) = command(&["git", "rev-parse", "--show-toplevel"], &p);
            return if rc == Some(0) && resolve(Path::new(crate::py::strip(&out))) == p { Some(p) } else { None };
        }
    }
    None
}

/// The YAML reader the original borrowed from `manifest_contract_compat_owner._load_manifest`: `yaml.safe_load(f) or {}`.
/// `Ok(None)` = an OSError/ValueError the callers turn into "unobserved"; `Err` = a YAML error, which escaped the original.
fn load_manifest(path: &Path) -> Result<Option<PyValue>, crate::raise::Raised> {
    let Ok(b) = std::fs::read(path) else { return Ok(None) };
    if std::str::from_utf8(&b).is_err() {
        return Ok(None);
    }
    match pyyaml::safe_load(&b, &PyStr::new(&path.display().to_string())) {
        Ok(v) if v.is_truthy() => Ok(Some(v)),
        Ok(_) => Ok(Some(PyValue::Dict(Default::default()))),
        Err(e) => Err(crate::raise::Raised::yaml(&e)),
    }
}

fn get<'a>(v: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    crate::py::get(v, k)
}

/// `.values()` of a mapping; `None` when not a dict (the original's AttributeError).
fn values(v: Option<&PyValue>) -> Option<Vec<PyValue>> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Dict(d)) => Some(d.iter().map(|(_, x)| x.peel().clone()).collect()),
        _ => None,
    }
}

pub fn build_files(workspace: &Path, topology: &str) -> Result<Option<Vec<String>>, crate::raise::Raised> {
    let compose_path = workspace.join("output").join(topology).join("docker-compose.yaml");
    let Some(mut compose) = load_manifest(&compose_path)? else { return Ok(None) };
    let empty = PyValue::Dict(Default::default());
    let services = |c: &PyValue| values(Some(get(c, "services").unwrap_or(&empty)));
    let Some(svcs) = services(&compose) else { return Ok(None) };
    let interpolated = svcs.iter().any(|s| {
        matches!(get(s, "build"), Some(b @ PyValue::Dict(_))
            if crate::py::display(get(b, "dockerfile").unwrap_or(&PyValue::str(""))).contains('$'))
    });
    if interpolated {
        let cp = compose_path.display().to_string();
        let mut argv: Vec<String> = ["docker", "compose", "--profile", "*"].iter().map(|s| s.to_string()).collect();
        let env_path = compose_path.parent().unwrap().join(".env");
        if env_path.is_file() {
            argv.push("--env-file".into());
            argv.push(env_path.display().to_string());
        }
        argv.extend(["-f".into(), cp, "config".into(), "--format".into(), "json".into()]);
        let refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        let (rc, out) = command(&refs, workspace);
        if rc != Some(0) {
            return Ok(None);
        }
        match agent_runtime_support::pyjson::loads(&out) {
            Ok(v @ PyValue::Dict(_)) => compose = v,
            _ => return Ok(None),
        }
    }
    let Some(svcs) = services(&compose) else { return Ok(None) };
    let mut selected: Vec<String> = vec![];
    for s in svcs {
        if let Some(b @ PyValue::Dict(_)) = get(&s, "build") {
            let name = match get(b, "dockerfile") {
                None => "Dockerfile".to_string(),
                Some(PyValue::Str(t)) => crate::py::text(t),
                Some(_) => return Ok(None),
            };
            if !DOCKERFILES.contains(&name.as_str()) {
                return Ok(None);
            }
            if !selected.contains(&name) {
                selected.push(name);
            }
        }
    }
    if selected.is_empty() {
        return Ok(None);
    }
    selected.sort();
    let mut out: Vec<String> = BUILD_FILES.iter().map(|s| s.to_string()).collect();
    out.extend(selected);
    Ok(Some(out))
}

/// `_model_asset_read` → the check's extra fields (outcome first).
fn model_asset_read(workspace: &Path, topology: &str, role: &str) -> Result<Vec<(&'static str, PyValue)>, crate::raise::Raised> {
    let unavailable = || vec![("outcome", PyValue::str("unobserved")), ("operation", PyValue::str("model declaration/read unavailable"))];
    let Some(man) = load_manifest(&workspace.join("output").join(topology).join("manifest.yaml"))? else {
        return Ok(unavailable());
    };
    if !matches!(man.peel(), PyValue::Dict(_)) {
        return Ok(unavailable());
    }
    let nodes = match get(&man, "nodes") {
        None => vec![],
        Some(PyValue::List(l)) | Some(PyValue::Tuple(l)) => l.clone(),
        Some(PyValue::Dict(d)) => d.iter().map(|(k, _)| PyValue::Str(k.clone())).collect(),
        Some(PyValue::Str(s)) => s.code_points().iter().map(|c| PyValue::Str(PyStr::from_code_points(vec![*c]))).collect(),
        Some(_) => return Ok(unavailable()),
    };
    let mut node = PyValue::Dict(Default::default());
    for n in &nodes {
        if !matches!(n.peel(), PyValue::Dict(_)) {
            return Ok(unavailable());
        }
        if get(n, "role").is_some_and(|r| crate::py::display(r) == role && matches!(r, PyValue::Str(_))) {
            node = n.peel().clone();
            break;
        }
    }
    let top = get(&man, "model_source").cloned().unwrap_or(PyValue::None);
    let mode = if topology == "single" {
        match get(&node, "model_source") {
            Some(v) if v.is_truthy() => v.clone(),
            _ => top,
        }
    } else {
        top
    };
    let ms = match &mode {
        PyValue::Str(s) => crate::py::text(s),
        _ => String::new(),
    };
    if ms == "ephemeral" {
        return Ok(vec![("outcome", PyValue::str("not_applicable")), ("model_source", mode),
            ("operation", PyValue::str("manifest-declared ephemeral; no model acquisition attempted"))]);
    }
    let paths: Vec<PyValue> = if ms == "managed" {
        let mut p = vec![get(&man, "nas_model_path").cloned().unwrap_or(PyValue::None)];
        if let Some(q) = get(&man, "quant_model_path").filter(|q| q.is_truthy()) {
            p.push(q.clone());
        }
        p
    } else if let (true, Some(PyValue::Dict(d))) = (ms == "custom", get(&man, "custom_model_paths")) {
        d.iter().map(|(_, v)| v.peel().clone()).collect()
    } else {
        vec![]
    };
    let strs: Option<Vec<String>> = paths.iter().map(|p| match p {
        PyValue::Str(s) if Path::new(&crate::py::text(s)).is_absolute() => Some(crate::py::text(s)),
        _ => None,
    }).collect();
    let Some(strs) = strs.filter(|v| !v.is_empty()) else {
        return Ok(vec![("outcome", PyValue::str("unobserved")), ("model_source", mode)]);
    };
    for p in &strs {
        let ok = std::fs::OpenOptions::new().read(true).custom_flags(O_DIRECTORY).open(p).is_ok();
        if !ok {
            return Ok(unavailable());
        }
    }
    Ok(vec![("outcome", PyValue::str("allowed")), ("model_source", mode),
        ("operation", PyValue::str("read-open declared model root directories; no weight bytes")),
        ("root_count", PyValue::int(strs.len() as i64))])
}

/// A check row: name, axis, outcome, extras (in order).
pub struct Check(Vec<(String, PyValue)>);

fn row(name: &str, axis: &str, fields: Vec<(&'static str, PyValue)>) -> Check {
    let mut v = vec![("name".to_string(), PyValue::str(name)), ("axis".to_string(), PyValue::str(axis))];
    let outcome = fields.iter().find(|(k, _)| *k == "outcome").map(|(_, x)| x.clone()).unwrap_or(PyValue::None);
    v.push(("outcome".into(), outcome));
    for (k, x) in fields {
        if k != "outcome" {
            v.push((k.to_string(), x));
        }
    }
    Check(v)
}
fn simple(name: &str, outcome: &str) -> Check {
    row(name, "functional", vec![("outcome", PyValue::str(outcome))])
}
fn wo(name: &str, path: &Path) -> Check {
    let (o, p) = write_open(path);
    row(name, "os_enforcement", vec![("outcome", PyValue::str(o)), ("preserved", p.map_or(PyValue::None, PyValue::Bool))])
}

fn scratch_dir(logs: &Path) -> std::io::Result<PathBuf> {
    for i in 0..100u32 {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos() ^ (std::process::id() << 8) ^ i;
        let p = logs.join(format!("harness-permission-{n:08x}"));
        match std::fs::create_dir(&p) {
            Ok(()) => {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700))?;
                return Ok(p);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists))
}

pub fn observe(workspace: &Path, topology: &str, role: &str, nonce: &str) -> Result<PyValue, crate::raise::Raised> {
    let workspace = resolve(workspace);
    let mut checks: Vec<Check> = vec![];
    let scratch_block = (|| -> std::io::Result<()> {
        let logs = workspace.join("docs/logs");
        std::fs::create_dir_all(&logs)?;
        let scratch = scratch_dir(&logs)?;
        let inner = (|| -> std::io::Result<()> {
            checks.push(simple("scratch_write_cleanup", write_marker(&scratch, nonce)?));
            let initialized = command(&["git", "init", "--quiet", "--template="], &scratch).0 == Some(0);
            let dirs = if initialized { git_dirs(&scratch) } else { None };
            let (rc, top) = if initialized { command(&["git", "rev-parse", "--show-toplevel"], &scratch) } else { (None, String::new()) };
            let sr = resolve(&scratch);
            let bound = rc == Some(0) && resolve(Path::new(crate::py::strip(&top))) == sr
                && dirs.as_ref().is_some_and(|d| !d.is_empty() && d.iter().all(|p| p != &sr && p.starts_with(&sr)));
            let cmds: [&[&str]; 2] = [
                &["git", "-c", "user.name=Harness Canary", "-c", "user.email=harness-canary@invalid", "-c",
                  "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "-m",
                  "Disposable permission canary"],
                &["git", "branch", "canary-local-only"],
            ];
            let ok = bound && cmds.iter().all(|a| command(a, &scratch).0 == Some(0));
            checks.push(simple("scratch_local_git", if ok { "allowed" } else { "failed" }));
            Ok(())
        })();
        let removed = std::fs::remove_dir_all(&scratch);
        inner?;
        removed?;
        checks.push(simple("scratch_removed", if !scratch.exists() { "allowed" } else { "failed" }));
        Ok(())
    })();
    if scratch_block.is_err() {
        checks.push(simple("scratch_write_cleanup", "unobserved"));
    }
    match git_dirs(&workspace) {
        None => checks.push(simple("workspace_git_metadata", "unobserved")),
        Some(dirs) => {
            for (n, d) in dirs.iter().enumerate() {
                let o = write_marker(d, nonce).unwrap_or("failed");
                checks.push(simple(&format!("workspace_git_metadata{}", if n > 0 { "_common" } else { "" }), o));
            }
        }
    }
    for rel in protected_files(role) {
        checks.push(wo(&format!("protected:{rel}"), &workspace.join(rel)));
    }
    let bf = build_files(&workspace, topology)?;
    checks.push(simple("buildkit_declaration", if bf.as_ref().is_some_and(|b| !b.is_empty()) { "allowed" } else { "unobserved" }));
    let names: Vec<String> = bf.clone().unwrap_or_else(|| BUILD_FILES.iter().map(|s| s.to_string()).collect());
    for name in &names {
        checks.push(wo(&format!("buildkit:{name}"), &workspace.join("output").join(topology).join(name)));
    }
    checks.push(row("model_asset_read", "functional", model_asset_read(&workspace, topology, role)?));
    let (rc, out) = command(&["docker", "version", "--format", "{{json .Server.Version}}"], &workspace);
    let server_ok = matches!(agent_runtime_support::pyjson::loads(&out), Ok(PyValue::Str(s)) if !s.is_empty());
    checks.push(simple("docker_daemon_read", if rc == Some(0) && server_ok { "allowed" } else { "unobserved" }));
    let (rc, out) = command(&["docker", "compose", "version", "--short"], &workspace);
    checks.push(simple("docker_compose_read", if rc == Some(0) && !crate::py::strip(&out).is_empty() { "allowed" } else { "unobserved" }));
    let term = format!("harness-permission-canary-{nonce}");
    let (rc, out) = command(&["node", ".claude/skills/wiki-desk/src/cli.ts", "query", "--terms", &term, "--limit", "1"], &workspace);
    let q_ok = agent_runtime_support::pyjson::loads(&out).ok().is_some_and(|q| {
        matches!(q.peel(), PyValue::Dict(_)) && matches!(get(&q, "query_tokens"), Some(PyValue::List(l)) if !l.is_empty())
    });
    checks.push(simple("library_query", if rc == Some(0) && q_ok { "allowed" } else { "unobserved" }));
    if topology == "single" && role == "sub" {
        match hint_root(&workspace) {
            None => checks.push(simple("hint_clone_write", "unobserved")),
            Some(clone) => {
                let dirs = git_dirs(&clone);
                let mut roots = vec![clone.clone()];
                for d in dirs.clone().unwrap_or_default() {
                    if !roots.contains(&d) {
                        roots.push(d);
                    }
                }
                checks.push(simple("hint_clone_git_discovery", if dirs.as_ref().is_some_and(|d| !d.is_empty()) { "allowed" } else { "unobserved" }));
                for (n, d) in roots.iter().enumerate() {
                    let o = write_marker(d, nonce).unwrap_or("failed");
                    checks.push(simple(&format!("hint_clone_write{}", if n > 0 { format!("_git_{n}") } else { String::new() }), o));
                }
            }
        }
    }
    let plane = if topology == "single" && role == "sub" {
        "node-local external clone"
    } else if role == "main" {
        "workspace git"
    } else {
        "not applicable"
    };
    let mut d = agent_runtime_support::pyvalue::PyDict::new();
    d.set("schema_version", PyValue::int(SCHEMA_VERSION));
    d.set("nonce", PyValue::str(nonce));
    d.set("topology", PyValue::str(topology));
    d.set("role", PyValue::str(role));
    d.set("workspace", PyValue::str(&workspace.display().to_string()));
    d.set("provenance", PyValue::str("observed_local_process"));
    d.set("build_files", bf.map_or(PyValue::None, |b| crate::py::list_str(&b)));
    d.set("checks", PyValue::List(checks.into_iter().map(|c| {
        let mut x = agent_runtime_support::pyvalue::PyDict::new();
        for (k, v) in c.0 {
            x.set(&k, v);
        }
        PyValue::Dict(x)
    }).collect()));
    d.set("hint_work_plane", PyValue::str(plane));
    d.set("limits", crate::py::list_str(&[
        "write-open is not file modification/rename/create confinement proof".into(),
        "main canonical write access is a positive role check, not proof of outside-workspace denial".into(),
        "Docker read access does not prove build/run or daemon confinement".into(),
        "library query can populate its existing derived cache; no model or remote git".into(),
        "hint markers prove local clone/git writes, not publication or network push".into(),
        "provider tool restrictions and OS enforcement are separate surfaces".into(),
    ]));
    Ok(PyValue::Dict(d))
}

/// `json.dumps(v, ensure_ascii=False, separators=(",", ":"))`.
pub fn dumps_tight_unicode(v: &PyValue) -> String {
    let mut out = PyStr::default();
    fn w(v: &PyValue, o: &mut PyStr) {
        match v.peel() {
            PyValue::None => o.push_str("null"),
            PyValue::Bool(b) => o.push_str(if *b { "true" } else { "false" }),
            PyValue::Int(n) => o.push_str(&n.to_string()),
            PyValue::Float(f) => o.push_str(&if f.is_nan() { "NaN".into() } else if f.is_infinite() {
                if *f > 0.0 { "Infinity".into() } else { "-Infinity".into() }
            } else { float_repr(*f) }),
            PyValue::Str(s) => s.write_json(o, false),
            PyValue::List(l) | PyValue::Tuple(l) => {
                o.push_str("[");
                for (i, x) in l.iter().enumerate() {
                    if i > 0 {
                        o.push_str(",");
                    }
                    w(x, o);
                }
                o.push_str("]");
            }
            PyValue::Dict(d) => {
                o.push_str("{");
                for (i, (k, x)) in d.iter().enumerate() {
                    if i > 0 {
                        o.push_str(",");
                    }
                    k.write_json(o, false);
                    o.push_str(":");
                    w(x, o);
                }
                o.push_str("}");
            }
            other => PyStr::new(&other.repr()).write_json(o, false),
        }
    }
    w(v, &mut out);
    crate::py::text(&out)
}

pub static SPEC: Spec = Spec {
    prog: "harness_permission_probe.py",
    usage: include_str!("../help/harness_permission_probe.usage"),
    help: include_str!("../help/harness_permission_probe.help"),
    args: &[
        HELP,
        store(&["--workspace"]).required(),
        store(&["--topology"]).choices(&["single", "multi"]).required(),
        store(&["--role"]).choices(&["main", "sub"]).required(),
        store(&["--nonce"]).required(),
    ],
    subs: None,
};

pub fn main(args: &[String]) -> i32 {
    let a = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    let ws = a.text(1).unwrap_or("");
    let nonce = a.text(4).unwrap_or("");
    let hex = nonce.len() == 32 && nonce.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !Path::new(ws).is_absolute() || !hex {
        return argp::error(&SPEC, "absolute workspace and a fresh 32-hex nonce are required");
    }
    match observe(Path::new(ws), a.text(2).unwrap(), a.text(3).unwrap(), nonce) {
        Ok(v) => {
            println!("{MARKER}{}", dumps_tight_unicode(&v));
            0
        }
        Err(r) => crate::raise::exit(&r),
    }
}
