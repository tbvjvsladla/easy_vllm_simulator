//! `verify()` of verify_distribution.py: the static contract checks and the named child runs, in the original order.
use crate::fsglob::{glob, glob_child_literal, rglob};
use crate::pyassert::assert_lines;
use crate::pytext::{
    find, os_error, py_strip, read_text, read_text_replace, rel, sort_paths, splitlines, strip_chars, PyError,
};
use crate::runner::{execute, run, Outcome};
use crate::{dict, get, list_ints, list_strs, opt_str, update, PyValue};
use fancy_regex::Regex;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const EXPECTED_SKILLS: [&str; 6] = [
    "adversarial-benchmark",
    "hint-publisher",
    "terraforming_node",
    "upstream-version-watch",
    "vllm-recipe-explorer",
    "wiki-desk",
];
pub const TRUST_FILES: [&str; 2] = [".claude/policies/registry.yaml", ".claude/policies/claim_bindings.json"];
pub const LOCAL_TOMBSTONES: [&str; 17] = [
    "scripts/agent_control.py", "scripts/cleanup_docker.py", "scripts/completion_gate.py",
    "scripts/doc_naming.py", "scripts/engine_liveness_watchdog.sh",
    "scripts/evidence_publisher.py", "scripts/harness_verify.py", "scripts/hint_tag.py",
    "scripts/host/vllm-drop-caches.sh", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/policy_registry.py", "scripts/providers/claude_code.py",
    "scripts/smoke_clone.sh", "scripts/sync_branches.sh",
    "scripts/systemd/easy-vllm-memwatch.service",
    "scripts/templates/hint_recipe.template.md",
];
/// The exact `ROOT_RELOCATION_REPLACEMENTS` of sync_branches.sh (a tripwire list: where the old root `scripts/` files
/// went).
pub const LOCAL_REPLACEMENTS: [&str; 17] = [
    ".claude/skills/terraforming_node/scripts/agent_control.sh",
    ".claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.sh",
    ".claude/policies/runtime/completion_gate.sh",
    ".claude/policies/runtime/doc_naming.sh",
    ".claude/skills/vllm-recipe-explorer/scripts/engine_liveness_watchdog.sh",
    ".claude/policies/runtime/evidence_publisher.sh",
    ".claude/policies/runtime/harness_verify.sh",
    ".claude/skills/hint-publisher/scripts/hint.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/host/vllm-drop-caches.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh",
    ".claude/policies/runtime/policy_registry.sh",
    ".claude/skills/terraforming_node/rust/src/provider_claude_code.rs",
    ".claude/skills/upstream-version-watch/scripts/smoke_clone.sh",
    ".claude/skills/upstream-version-watch/scripts/sync_branches.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/systemd/easy-vllm-memwatch.service",
    ".claude/skills/hint-publisher/templates/00-hint.prompt.md",
];
pub const SUB_TOMBSTONES: [&str; 9] = [
    ".claude/rules/references.md", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/host/vllm-drop-caches.sh",
    "scripts/systemd/easy-vllm-memwatch.service", "scripts/smoke_clone.sh",
    "scripts/sync_branches.sh", "tasks/.gitkeep",
    ".claude/skills/upstream-version-watch/assets/current-production-resolution.json",
];
pub const SUB_RELOCATION_TOMBSTONES: [&str; 6] = [
    ".claude/rules/references.md", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/host/vllm-drop-caches.sh",
    "scripts/systemd/easy-vllm-memwatch.service", "tasks/.gitkeep",
];
pub const SUB_RETIREMENT_TOMBSTONES: [&str; 3] = [
    "scripts/smoke_clone.sh", "scripts/sync_branches.sh",
    ".claude/skills/upstream-version-watch/assets/current-production-resolution.json",
];
pub const SCRATCH_DIR_NAMES: [&str; 4] = ["temp", "tmp", "test", "tests"];

/// The Rust launchers of the policy programs this driver starts (repo-relative).
pub const POLICY_REGISTRY_LAUNCHER: &str = ".claude/policies/runtime/policy_registry.sh";
pub const RUNTIME_SELFTEST_LAUNCHER: &str = ".claude/policies/runtime/runtime_selftest.sh";
/// Tracked non-.py executable surfaces start no Python on the host/sub (container lines allow-listed with a reason).
pub const INLINE_PYTHON_SCAN_LAUNCHER: &str = ".claude/rust/py-boundary-guard/inline_python_scan.sh";
pub const VERDICT_CLOCK_SCAN_LAUNCHER: &str = ".claude/policies/runtime/verdict_clock_scan.sh";
pub const ANTIPATTERN_SCAN_LAUNCHER: &str = ".claude/policies/runtime/antipattern_scan.sh";
pub const CONSTITUTION_LINT_LAUNCHER: &str = ".claude/policies/runtime/constitution_lint.sh";
pub const DOC_NAMING_LAUNCHER: &str = ".claude/policies/runtime/doc_naming.sh";
pub const CLAIM_PREDICATES_LAUNCHER: &str = "claim_predicates.sh";
pub const HINT_LAUNCHER: &str = ".claude/skills/hint-publisher/scripts/hint.sh";

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}
fn sorted(s: &BTreeSet<String>) -> PyValue {
    list_strs(&s.iter().cloned().collect::<Vec<_>>())
}
fn s(v: &str) -> PyValue {
    PyValue::str(v)
}
fn b(v: bool) -> PyValue {
    PyValue::Bool(v)
}
fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static pattern")
}

/// `_walk_strings`: every string of a JSON value, keys included, depth-first in order.
pub fn walk_strings(v: &PyValue, out: &mut Vec<String>) {
    match v.peel() {
        PyValue::Str(t) => out.push(t.to_utf8().unwrap_or_default()),
        PyValue::List(items) | PyValue::Tuple(items) => items.iter().for_each(|i| walk_strings(i, out)),
        PyValue::Dict(d) => {
            for (k, item) in d.iter() {
                out.push(k.to_utf8().unwrap_or_default());
                walk_strings(item, out);
            }
        }
        _ => {}
    }
}

/// `_shell_array(text, name)`.
pub fn shell_array(text: &str, name: &str) -> BTreeSet<String> {
    let pattern = format!(r"(?ms)^{}=\(\n(.*?)^\)$", fancy_regex::escape(name));
    let caps = match Regex::new(&pattern).ok().and_then(|r| r.captures(text).ok().flatten()) {
        Some(c) => c,
        None => return BTreeSet::new(),
    };
    let body = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    splitlines(body)
        .into_iter()
        .filter_map(|line| {
            let head = line.split('#').next().unwrap_or("");
            if py_strip(head).is_empty() {
                None
            } else {
                Some(strip_chars(py_strip(head), "'\"").to_string())
            }
        })
        .collect()
}

/// `_ordered_between_detail` → (ok, detail dict).
pub fn ordered_between_detail(text: &str, marker: &str, end_marker: &str, tokens: &[&str]) -> (bool, PyValue) {
    let start = match text.find(marker) {
        Some(i) => i,
        None => return (false, dict(vec![("ok", b(false)), ("reason", s("start_anchor_missing")), ("missing", s(marker))])),
    };
    let end = match text[start + marker.len()..].find(end_marker) {
        Some(i) => start + marker.len() + i,
        None => return (false, dict(vec![("ok", b(false)), ("reason", s("end_anchor_missing")), ("missing", s(end_marker))])),
    };
    let block = &text[start..end];
    let mut cursor = 0usize;
    for token in tokens {
        match block[cursor..].find(token) {
            Some(i) => cursor = cursor + i + token.len(),
            None => {
                let reason = if block.contains(token) { "token_out_of_order" } else { "token_absent" };
                return (false, dict(vec![("ok", b(false)), ("reason", s(reason)), ("missing", s(token))]));
            }
        }
    }
    (true, dict(vec![("ok", b(true)), ("reason", PyValue::None)]))
}

fn order_check(name: &str, text: &str, marker: &str, end_marker: &str, tokens: &[&str]) -> PyValue {
    let (ok, detail) = ordered_between_detail(text, marker, end_marker, tokens);
    dict(vec![("name", s(name)), ("ok", b(ok)), ("order_detail", detail)])
}

/// `_active_retirement_consumers()` (Err = an OSError the original does not catch here but its caller does).
pub fn active_retirement_consumers(repo: &Path) -> Result<Vec<String>, PyError> {
    let mut hits: Vec<String> = Vec::new();
    let roots = [repo.join(".claude/rules"), repo.join("CLAUDE.md"), repo.join("HINTS.md")];
    let token = |c: u8| c.is_ascii_alphanumeric() || b"_./-".contains(&c);
    let stales = set(&SUB_RETIREMENT_TOMBSTONES);
    for root in &roots {
        let mut files = if root.is_dir() { rglob(root, "*") } else { vec![root.clone()] };
        if root.is_dir() {
            sort_paths(&mut files);
        }
        for file in files {
            if !file.is_file() {
                continue;
            }
            let text = match read_text(&file) {
                Ok(t) => t,
                Err(e) if e.is_unicode() => continue,
                Err(e) => return Err(e),
            };
            let rel_file = rel(&file, repo);
            let bytes = text.as_bytes();
            for stale in &stales {
                let pattern = re(&format!(
                    r"(?<![A-Za-z0-9_.-]){}(?![A-Za-z0-9_./-])",
                    fancy_regex::escape(stale)
                ));
                let owner = format!(".claude/skills/upstream-version-watch/{stale}");
                for m in pattern.find_iter(&text).flatten() {
                    let (mut lo, mut hi) = (m.start(), m.end());
                    while lo > 0 && token(bytes[lo - 1]) {
                        lo -= 1;
                    }
                    while hi < bytes.len() && token(bytes[hi]) {
                        hi += 1;
                    }
                    let mut tok = &text[lo..hi];
                    while let Some(rest) = tok.strip_prefix("./") {
                        tok = rest;
                    }
                    if tok != owner {
                        hits.push(rel_file.clone());
                        break;
                    }
                }
                if hits.last() == Some(&rel_file) {
                    break;
                }
            }
        }
    }
    Ok(hits)
}

fn git_out(repo: &Path, a: &[&str]) -> Option<std::process::Output> {
    std::process::Command::new("git").arg("-C").arg(repo).args(a).output().ok()
}

fn terraform_flag_issued(repo: &Path) -> bool {
    let mc = repo.join(".claude/skills/terraforming_node/scripts/manifest_contract.sh");
    if !mc.is_file() {
        return false;
    }
    let branch = git_out(repo, &["branch", "--show-current"])
        .map(|o| py_strip(&String::from_utf8_lossy(&o.stdout)).to_string())
        .unwrap_or_default();
    let topo = if branch == "multi-node" { "multi" } else { "single" };
    match std::process::Command::new("bash")
        .arg(&mc)
        .args(["--topology", topo, "--repo"])
        .arg(repo)
        .arg("--require-flag")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
    {
        Ok(o) => o.status.code() == Some(0),
        Err(_) => false,
    }
}

/// `_shipped_python_paths(root)`.
pub fn shipped_python_paths(root: &Path) -> Vec<PathBuf> {
    let excluded = [root.join(".claude/worktrees"), root.join(".claude/runtime")];
    let mut candidates: Vec<PathBuf> = rglob(&root.join(".claude"), "*.py")
        .into_iter()
        .filter(|p| !excluded.iter().any(|d| p.ancestors().skip(1).any(|a| a == d)))
        .collect();
    sort_paths(&mut candidates);
    if candidates.is_empty() {
        return candidates;
    }
    let mut stdin = Vec::new();
    for p in &candidates {
        use std::os::unix::ffi::OsStrExt;
        stdin.extend_from_slice(p.strip_prefix(root).unwrap_or(p).as_os_str().as_bytes());
        stdin.push(0);
    }
    let child = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "-z", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(_) => return candidates,
    };
    {
        use std::io::Write;
        let mut si = child.stdin.take().unwrap();
        let writer = std::thread::spawn(move || {
            let _ = si.write_all(&stdin);
        });
        let out = child.wait_with_output();
        let _ = writer.join();
        let out = match out {
            Ok(o) => o,
            Err(_) => return candidates,
        };
        if !matches!(out.status.code(), Some(0) | Some(1)) {
            return candidates;
        }
        use std::os::unix::ffi::OsStrExt;
        let ignored: BTreeSet<Vec<u8>> = out.stdout.split(|&c| c == 0).filter(|i| !i.is_empty()).map(|i| i.to_vec()).collect();
        candidates
            .into_iter()
            .filter(|p| !ignored.contains(p.strip_prefix(root).unwrap_or(p).as_os_str().as_bytes()))
            .collect()
    }
}

/// `_tracked_scratch_paths()`.
pub fn tracked_scratch_paths(repo: &Path) -> Vec<String> {
    let out = git_out(repo, &["ls-files", "-z"]);
    let paths: Vec<String> = match out {
        Some(o) if o.status.code() == Some(0) && repo.join(".git").exists() => String::from_utf8_lossy(&o.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect(),
        _ => rglob(repo, "*").into_iter().filter(|p| p.is_file()).map(|p| rel(&p, repo)).collect(),
    };
    let mut hits: Vec<String> = paths
        .into_iter()
        .filter(|p| {
            let parts: Vec<&str> = p.split('/').collect();
            parts[..parts.len() - 1].iter().any(|part| SCRATCH_DIR_NAMES.contains(&part.to_lowercase().as_str()))
        })
        .collect();
    hits.sort();
    hits
}

/// `_codex_discovery_checks(root)`.
pub fn codex_discovery_checks(root: &Path) -> PyValue {
    let mut defects: Vec<String> = Vec::new();
    for r in ["AGENTS.md", "CLAUDE.md", ".claude/rules/workflow.md", ".claude/rules/docs.md",
              ".claude/rules/strategy.topology.md", ".codex/config.toml"] {
        if !root.join(r).is_file() {
            defects.push(format!("missing:{r}"));
        }
    }
    let wrappers: BTreeSet<String> = glob_child_literal(&root.join(".agents/skills"), "SKILL.md")
        .iter()
        .filter_map(|p| p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()))
        .collect();
    if wrappers != set(&EXPECTED_SKILLS) {
        let shown: Vec<String> = wrappers.iter().map(|w| crate::pytext::py_repr(w)).collect();
        defects.push(format!("wrapper-set:[{}]", shown.join(", ")));
    }
    for name in &wrappers {
        let path = root.join(".agents/skills").join(name).join("SKILL.md");
        let expected = format!("../../../.claude/skills/{name}/SKILL.md");
        let text = match read_text(&path) {
            Ok(t) => t,
            Err(e) => {
                defects.push(format!("wrapper-unreadable:{name}:{}", e.type_name));
                continue;
            }
        };
        let is_link = fs::symlink_metadata(&path).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if is_link || !text.contains(&expected) || !path.parent().unwrap().join(&expected).is_file() || text.len() > 2048 {
            defects.push(format!("wrapper-closure:{name}"));
        }
    }
    let config_path = root.join(".codex/config.toml");
    match fs::read(&config_path) {
        Err(e) => {
            let pe = os_error(&e, &config_path.to_string_lossy());
            defects.push(format!("main-config:{}:{}", pe.type_name, pe.message));
        }
        Ok(bytes) => match crate::pytext::decode_strict(&bytes) {
            Err(e) => defects.push(format!("main-config:{}:{}", e.type_name, e.message)),
            Ok(text) => match crate::toml::loads(&text) {
                Err(e) => defects.push(format!("main-config:TOMLDecodeError:{}", e.0)),
                Ok(config) => {
                    use crate::toml::Value;
                    if config.get("default_permissions") != Some(&Value::Str(":danger-full-access".into())) {
                        defects.push("main-permission-profile".into());
                    }
                    if config.get("approval_policy") != Some(&Value::Str("never".into())) {
                        defects.push("main-approval-policy".into());
                    }
                    if config.contains_key("permissions") {
                        defects.push("unexpected-custom-permission-profiles".into());
                    }
                    if config.contains_key("sandbox_mode") || config.contains_key("sandbox_workspace_write") {
                        defects.push("legacy-sandbox-overrides-profile".into());
                    }
                }
            },
        },
    }
    dict(vec![("name", s("codex_entry_and_skill_closure")), ("ok", b(defects.is_empty())), ("defects", list_strs(&defects))])
}

fn check_ok(v: &PyValue) -> bool {
    get(v, "ok").map(|o| o.is_truthy()).unwrap_or(false)
}

fn defects_of(v: &PyValue) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(d) = get(v, "defects") {
        walk_strings(d, &mut out);
    }
    out
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), PyError> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| os_error(&e, &parent.to_string_lossy()))?;
    }
    let bytes = fs::read(src).map_err(|e| os_error(&e, &src.to_string_lossy()))?;
    fs::write(dst, bytes).map_err(|e| os_error(&e, &dst.to_string_lossy()))
}

/// A `tempfile.TemporaryDirectory(prefix=...)` (removed on drop).
pub struct TempDir(pub PathBuf);
impl TempDir {
    pub fn new(prefix: &str) -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        for attempt in 0..100u32 {
            let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            let p = base.join(format!("{prefix}{}{:x}{attempt}", std::process::id(), n));
            match fs::create_dir(&p) {
                Ok(()) => {
                    let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o700));
                    return Ok(TempDir(p));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("no temp dir"))
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn codex_discovery_selftest(repo: &Path) -> PyValue {
    let name = "codex_gitless_discovery_selftest";
    let td = match TempDir::new("codex-discovery-contract.") {
        Ok(t) => t,
        Err(e) => {
            let pe = os_error(&e, "");
            return dict(vec![("name", s(name)), ("ok", b(false)), ("error", s(&format!("{}:{}", pe.type_name, pe.message)))]);
        }
    };
    let root = td.0.clone();
    let body = || -> Result<PyValue, PyError> {
        let mut paths: Vec<String> = args(&["AGENTS.md", "CLAUDE.md", ".codex/config.toml", ".claude/rules/workflow.md",
                                            ".claude/rules/docs.md", ".claude/rules/strategy.topology.md"]);
        for n in EXPECTED_SKILLS {
            paths.push(format!(".agents/skills/{n}/SKILL.md"));
            paths.push(format!(".claude/skills/{n}/SKILL.md"));
        }
        for r in &paths {
            copy_file(&repo.join(r), &root.join(r))?;
        }
        let good = check_ok(&codex_discovery_checks(&root));
        let wrapper = root.join(".agents/skills/wiki-desk/SKILL.md");
        let wt = read_text(&wrapper)?;
        fs::write(&wrapper, wt.replace("../../../.claude/skills/wiki-desk/SKILL.md", "../../../missing/SKILL.md"))
            .map_err(|e| os_error(&e, &wrapper.to_string_lossy()))?;
        let missing_rejected = !check_ok(&codex_discovery_checks(&root));
        copy_file(&repo.join(".agents/skills/wiki-desk/SKILL.md"), &wrapper)?;
        let config = root.join(".codex/config.toml");
        let ct = read_text(&config)?;
        let w = |text: String| fs::write(&config, text).map_err(|e| os_error(&e, &config.to_string_lossy()));
        w(format!("sandbox_mode = \"workspace-write\"\n{ct}"))?;
        let legacy_rejected = !check_ok(&codex_discovery_checks(&root));
        let original = read_text(&repo.join(".codex/config.toml"))?;
        w(original.replace("\":danger-full-access\"", "\"vllm-main\""))?;
        let retired_rejected = defects_of(&codex_discovery_checks(&root)).contains(&"main-permission-profile".to_string());
        w(format!("{original}\n[permissions.vllm-main]\nextends = \":workspace\"\n"))?;
        let custom_rejected =
            defects_of(&codex_discovery_checks(&root)).contains(&"unexpected-custom-permission-profiles".to_string());
        Ok(dict(vec![
            ("name", s(name)),
            ("ok", b(good && missing_rejected && legacy_rejected && retired_rejected && custom_rejected)),
            ("gitless_ok", b(good)),
            ("missing_target_rejected", b(missing_rejected)),
            ("legacy_override_rejected", b(legacy_rejected)),
            ("retired_profile_rejected", b(retired_rejected)),
            ("custom_profiles_rejected", b(custom_rejected)),
        ]))
    };
    match body() {
        Ok(v) => v,
        Err(e) => dict(vec![("name", s(name)), ("ok", b(false)), ("error", s(&format!("{}:{}", e.type_name, e.message)))]),
    }
}

/// `_policy_registry_check(policy_runner)`: `policy_registry.sh verify --repo-root <repo>` through `runner` (`_run`).
pub fn policy_registry_check<F>(policy_runner: &Path, repo: &Path, runner: F) -> PyValue
where
    F: FnOnce(&str, &[String], &[i64]) -> PyValue,
{
    let argv = vec![
        "bash".to_string(),
        policy_runner.to_string_lossy().into_owned(),
        "verify".into(),
        "--repo-root".into(),
        repo.to_string_lossy().into_owned(),
    ];
    runner("policy_registry_verify", &argv, &[0])
}

fn copytree_skipping(src: &Path, dst: &Path, skip: &[&Path]) -> Result<(), PyError> {
    fs::create_dir_all(dst).map_err(|e| os_error(&e, &dst.to_string_lossy()))?;
    let rd = fs::read_dir(src).map_err(|e| os_error(&e, &src.to_string_lossy()))?;
    let mut items: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    items.sort();
    for p in items {
        let name = p.file_name().unwrap();
        if name == "__pycache__" || skip.iter().any(|s| *s == p.as_path()) {
            continue;
        }
        let target = dst.join(name);
        if p.is_dir() {
            copytree_skipping(&p, &target, skip)?;
        } else {
            fs::copy(&p, &target).map_err(|e| os_error(&e, &p.to_string_lossy()))?;
        }
    }
    Ok(())
}

fn gitless_hint_match_checks(repo: &Path) -> Vec<PyValue> {
    let skill = ".claude/skills/hint-publisher";
    let td = match TempDir::new("easy-vllm-gitless-hint.") {
        Ok(t) => t,
        Err(e) => {
            let pe = os_error(&e, "");
            return vec![dict(vec![("name", s("gitless_hint_match")), ("ok", b(false)), ("error", s(&pe.colon_space()))])];
        }
    };
    let root = td.0.clone();
    let prepared = (|| -> Result<PyValue, PyError> {
        copytree_skipping(&repo.join(skill), &root.join(skill), &[&repo.join(skill).join("rust/target")])?;
        let bin_rel = format!("{skill}/rust/target/release/hint");
        fs::create_dir_all(root.join(&bin_rel).parent().unwrap()).map_err(|e| os_error(&e, &bin_rel))?;
        fs::copy(repo.join(&bin_rel), root.join(&bin_rel)).map_err(|e| os_error(&e, &repo.join(&bin_rel).to_string_lossy()))?;
        for r in ["hints/index.json", "hints/vocab.json"] {
            copy_file(&repo.join(r), &root.join(r))?;
        }
        let text = read_text(&root.join("hints/index.json"))?;
        let doc = agent_runtime_support::pyjson::loads(&text)
            .map_err(|e| PyError::new("JSONDecodeError", e.to_string()))?;
        match get(&doc, "hints") {
            Some(v) => Ok(v.clone()),
            None if matches!(doc.peel(), PyValue::Dict(_)) => Ok(PyValue::None),
            None => Err(PyError::new("AttributeError", format!("'{}' object has no attribute 'get'", doc.type_name()))),
        }
    })();
    let rows = match prepared {
        Ok(r) => r,
        Err(e) => return vec![dict(vec![("name", s("gitless_hint_match")), ("ok", b(false)), ("error", s(&e.colon_space()))])],
    };
    let row_list: Option<&Vec<PyValue>> = match rows.peel() {
        PyValue::List(l) => Some(l),
        _ => None,
    };
    let field = |r: &PyValue, k: &str| -> Option<String> {
        match get(r, k).map(|v| v.peel()) {
            Some(PyValue::Str(t)) if !t.is_empty() => t.to_utf8(),
            _ => None,
        }
    };
    let probe = row_list.and_then(|l| {
        l.iter().find(|r| matches!(r.peel(), PyValue::Dict(_)) && ["tag", "vllm", "model", "arch"].iter().all(|k| field(r, k).is_some()))
    });
    let probe = match probe {
        Some(p) => p,
        None => {
            return vec![dict(vec![
                ("name", s("gitless_hint_match")),
                ("ok", b(false)),
                ("error", s("hints/index.json 에 질의할 행(tag·vllm·model·arch)이 없다 — 파생 질의를 만들 수 없다")),
            ])]
        }
    };
    let (pv, pm, pa, pt) = (field(probe, "vllm").unwrap(), field(probe, "model").unwrap(), field(probe, "arch").unwrap(), field(probe, "tag").unwrap());
    let skill_dir = root.join(skill).to_string_lossy().into_owned();
    let gitless = [("PATH", "/nonexistent"), ("HINT_SKILL_DIR", skill_dir.as_str())];
    let cli = vec![root.join(format!("{skill}/rust/target/release/hint")).to_string_lossy().into_owned(), "match".into(), "--vllm".into(), pv.clone(), "--model".into(), pm.clone()];
    let mut first = run("gitless_hint_match", &cli, &[0], &root, &gitless);
    let run_json = |extra: &[&str]| -> (Option<i64>, Option<String>, Option<PyValue>, Option<String>) {
        let mut argv = cli.clone();
        argv.extend(extra.iter().map(|e| e.to_string()));
        argv.push("--json".into());
        match execute(&argv, &root, &gitless) {
            Outcome::Failed(err) => (None, None, None, Some(err)),
            Outcome::Done { rc, stdout, stderr } => {
                let parsed = agent_runtime_support::pyjson::loads(&stdout).ok();
                (Some(rc), Some(stderr), parsed, None)
            }
        }
    };
    let (rc, _stderr, got, err) = run_json(&["--arch", &pa]);
    let hit = match got.as_ref().map(|g| g.peel()) {
        Some(PyValue::List(items)) => items.iter().any(|g| {
            matches!(g.peel(), PyValue::Dict(_))
                && get(g, "tag").map(|t| matches!(t.peel(), PyValue::Str(x) if x.equals(&pt))).unwrap_or(false)
                && get(g, "arch_match").map(|t| matches!(t.peel(), PyValue::Str(x) if x.equals("exact"))).unwrap_or(false)
        }),
        _ => false,
    };
    let first_ok = check_ok(&first) && rc == Some(0) && hit && !root.join(".git").exists();
    update(&mut first, vec![
        ("probe", dict(vec![("vllm", s(&pv)), ("model", s(&pm)), ("arch", s(&pa)), ("tag", s(&pt))])),
        ("json_rc", rc.map(PyValue::int).unwrap_or(PyValue::None)),
        ("json_error", opt_str(&err)),
        ("probe_row_returned", b(hit)),
        ("ok", b(first_ok)),
    ]);
    let mut second = dict(vec![("name", s("gitless_hint_match_reads_index")), ("ok", b(false)), ("expected_rc", list_ints(&[0]))]);
    let (rc, stderr, got, err) = run_json(&["--include-other"]);
    let rc = match rc {
        None => {
            update(&mut second, vec![("error", opt_str(&err))]);
            return vec![first, second];
        }
        Some(rc) => rc,
    };
    let got_len = match got.as_ref().map(|g| g.peel()) {
        Some(PyValue::List(l)) => Some(l.len()),
        _ => None,
    };
    let rows_len = row_list.map(|l| l.len());
    let stderr = stderr.unwrap_or_default();
    update(&mut second, vec![
        ("rc", PyValue::int(rc)),
        ("stderr_tail", s(&crate::pytext::tail_chars(&stderr, 1000))),
        ("index_rows", rows_len.map(|n| PyValue::int(n as i64)).unwrap_or(PyValue::None)),
        ("matched_rows", got_len.map(|n| PyValue::int(n as i64)).unwrap_or(PyValue::None)),
        ("ok", b(rc == 0 && got_len.is_some() && rows_len.is_some() && got_len == rows_len)),
    ]);
    vec![first, second]
}

fn mode_is_nonexec(p: &Path) -> bool {
    fs::metadata(p).map(|m| m.permissions().mode() & 0o7777 & 0o111 == 0).unwrap_or(false)
}
fn nonempty(p: &Path) -> bool {
    fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false)
}
fn is_symlink(p: &Path) -> bool {
    fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

/// An uncaught exception of the original (a traceback, exit 1): the driver stops with that error.
#[derive(Debug)]
pub struct Uncaught(pub String);

fn must(r: Result<String, PyError>) -> Result<String, Uncaught> {
    r.map_err(|e| Uncaught(format!("{}: {}", e.type_name, e.message)))
}

fn sub_checks(repo: &Path, sub_text: &str) -> Result<Vec<PyValue>, PyError> {
    let sub_actual = shell_array(sub_text, "OVERLAY_STALE_PATHS");
    let sub_relocations = shell_array(sub_text, "OVERLAY_RELOCATION_STALE_PATHS");
    let sub_retirements = shell_array(sub_text, "OVERLAY_RETIREMENT_STALE_PATHS");
    let additive_body = re(r"(?ms)^deliver_overlay\(\).*?^}\n")
        .find(sub_text)
        .ok()
        .flatten()
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    let order: &[&str] = &["verify_checksums ", "verify_destination_runner_modes ",
        "verify_destination_host_safety_modes", "verify_destination_retirement_consumers",
        "apply_overlay_tombstones", "git add -A"];
    let tx0 = ordered_between_detail(sub_text, "begin_remote_transaction \"$t\" 0", "deliver_build \"$t\" 0",
        &["begin_remote_transaction \"$t\" 0", "git checkout -q $t"]);
    let tx1 = ordered_between_detail(sub_text, "PROVISION\" != \"1\"", "if [ $HAS_GIT = 0 ]; then",
        &["begin_remote_transaction \"${BOOTSTRAP_POPULATE:-${TARGETS[0]}}\" 1", "sub_run_mk"]);
    let foreign = ordered_between_detail(sub_text, "sub_assert_existing_workdir_not_foreign || exit $?", "if [ $HAS_GIT = 0 ]; then",
        &["PROVISION\" != \"1\"", "begin_remote_transaction \"${BOOTSTRAP_POPULATE:-${TARGETS[0]}}\" 1", "sub_run_mk"]);
    let foreign_tokens: &[&str] = &["\\\"\\$d\\\"/.[!.]* \\\"\\$d\\\"/..?*", "R3_NONEMPTY_NONGIT_WORKDIR", "R3_WORKDIR_STATE_UNREADABLE",
        "sub_assert_existing_workdir_not_foreign || echo"];
    let has = |t: &str| sub_text.contains(t);
    let sub_runner = repo.join(".claude/skills/upstream-version-watch/rust/runtime/src/bin/sub_runner.rs");
    let runner_text = if sub_runner.is_file() { read_text(&sub_runner)? } else { String::new() };
    let template = repo.join(".claude/skills/terraforming_node/sub_node/CLAUDE.template.md");
    let template_text = if template.is_file() { read_text(&template)? } else { String::new() };
    let consumers = active_retirement_consumers(repo)?;
    let consumers2 = active_retirement_consumers(repo)?;
    let disjoint = sub_relocations.is_disjoint(&sub_retirements);
    let union: BTreeSet<String> = sub_relocations.union(&sub_retirements).cloned().collect();
    let retire_calls = re(r"verify_destination_retirement_consumers\s+\S+\s*\|\|").find_iter(sub_text).flatten().count();
    let band2 = re(r"(?m)^BAND2_RUNTIME_PATCH_STEMS=\(\s*[a-z0-9][a-z0-9.-]*(?:\s+[a-z0-9][a-z0-9.-]*)*\s*\)")
        .is_match(sub_text)
        .unwrap_or(false);
    Ok(vec![
        dict(vec![("name", s("sub_exact_relocation_tombstones")), ("ok", b(sub_actual == set(&SUB_TOMBSTONES))),
                  ("actual", sorted(&sub_actual)), ("expected", sorted(&set(&SUB_TOMBSTONES)))]),
        dict(vec![("name", s("sub_relocation_retirement_partition")),
                  ("ok", b(sub_relocations == set(&SUB_RELOCATION_TOMBSTONES) && sub_retirements == set(&SUB_RETIREMENT_TOMBSTONES)
                         && disjoint && union == set(&SUB_TOMBSTONES))),
                  ("actual", dict(vec![("relocations", sorted(&sub_relocations)), ("retirements", sorted(&sub_retirements))]))]),
        dict(vec![("name", s("sub_additive_overlay_has_no_apply_delete")),
                  ("ok", b(!additive_body.is_empty() && !additive_body.contains("rm -f")))]),
        order_check("sub_bootstrap_replacement_before_tombstone", sub_text, "deliver_build multi 0", "REMOTES=\"$(sub_run", order),
        order_check("sub_incremental_replacement_before_tombstone", sub_text, "deliver_build \"$t\" 0", "REST_BRANCH=", order),
        dict(vec![("name", s("sub_render_uses_transactional_source")),
                  ("ok", b(["prepare_transactional_source", "mktemp -d", "CANONICAL_SRC", "SRC=\"$TRANSACTIONAL_SRC/\""].iter().all(|t| has(t))
                         && find(sub_text, "prepare_transactional_source\n") < find(sub_text, "# ═══════════════════════ DRY-RUN")))]),
        dict(vec![("name", s("sub_transactional_source_uses_git_index")),
                  ("ok", b(has("checkout-index -z --stdin")
                         && has("filesystem bytes are excluded in favor of index authority")
                         && has("ls-files -z -- .claude CLAUDE.md AGENTS.md .agents .codex .gitignore campaigns hints output/multi output/single")
                         && has("render_input=\"manifest.yaml\"")
                         && has("render_input=\"sub_manifest.yaml\"")
                         && !has("a2a_signing/main_ed25519.pem")
                         && has("install -m 0600 \"$src_input\" \"$dst_input\"")
                         && !has("\"${CANONICAL_SRC}output/$topology/\"")))]),
        dict(vec![("name", s("sub_runtime_patch_transfer_is_owner_allowlisted")),
                  ("ok", b(band2 && !has("--include='/configs/*_patch.py'") && has("\"$build_assets\"/runtime_patches/*")))]),
        dict(vec![("name", s("sub_retirement_consumer_audit_before_tombstone")),
                  ("ok", b(retire_calls == 2 && has("retirement-scan") && has("scanner/transport failed for $stale")
                         && runner_text.contains("let owner = format!(")
                         && runner_text.contains("while lo > 0 && is_chars(cs[lo - 1])")
                         && runner_text.contains("if token != owner")))]),
        dict(vec![("name", s("sub_constitution_runtime_block_is_derived")),
                  ("ok", b(template_text.contains("{{ TOOL_PLANE_JSON }}")
                         && !template_text.contains("\"skills_loaded\": [\"vllm-recipe-explorer\"]")))]),
        dict(vec![("name", s("sub_topology_aware_retirement_wired")),
                  ("ok", b(has("retire_runtime_block_residue()")
                         && [".agents/skills", ".codex"].iter().all(|r| shell_array(sub_text, "RUNTIME_BLOCK_OWNED_ROOTS").contains(*r))
                         && has("RUNTIME_BLOCK_OWNED_ROOTS[@]")
                         && has("[ \"${RETIRE_ALLOW:-}\" != \"$n\" ]")
                         && has("--retire-residue) RETIRE_RESIDUE=1")
                         && has("RETIRE_RESIDUE=0")
                         && has("git -c core.quotePath=false ls-tree -r --name-only '$t'")
                         && has("렌더 실패를 잔재로 읽지 않는다")))]),
        dict(vec![("name", s("no_active_sub_retirement_consumers")), ("ok", b(consumers.is_empty())), ("details", list_strs(&consumers2))]),
        dict(vec![("name", s("sub_invocation_rollback_transaction")),
                  ("ok", b(["begin_remote_transaction \"$_bs_t\" 1", "begin_remote_transaction \"$t\" 0",
                            "rollback_remote_transactions", "finalize_remote_transactions",
                            "workdir-absent", "workdir-backup",
                            "trap 'transactional_exit $?' EXIT", "rollback failed; recovery backups retained",
                            "rollback_remote_transactions || rc=11"].iter().all(|t| has(t))))]),
        dict(vec![("name", s("sub_transaction_precedes_remote_mutation")), ("ok", b(tx0.0 && tx1.0)),
                  ("order_detail", PyValue::List(vec![tx0.1, tx1.1]))]),
        dict(vec![("name", s("sub_foreign_workdir_guard_before_provision")),
                  ("ok", b(foreign.0 && foreign_tokens.iter().all(|t| has(t)))),
                  ("order_detail", foreign.1),
                  ("missing_tokens", list_strs(&foreign_tokens.iter().filter(|t| !has(t)).map(|t| t.to_string()).collect::<Vec<_>>()))]),
        dict(vec![("name", s("sub_deletion_inventory_and_brake_fail_closed")),
                  ("ok", b(["deletion inventory dry-run failed", "deletion brake dry-run failed before apply",
                            "validate_remote_deletion_tree"].iter().all(|t| has(t))))]),
        dict(vec![("name", s("sub_path_protocol_closed")),
                  ("ok", b(["validate_inventory_relative_path", "[[:space:][:cntrl:]]",
                            "empty directories are undeclared transfer artifacts"].iter().all(|t| has(t))))]),
        order_check("sub_cleanup_is_postsuccess_best_effort", sub_text, "finalize_remote_transactions()", "transactional_exit()",
                    &["REMOTE_TX_ACTIVE=0", "successful sync left recovery backup"]),
    ])
}

/// The three side-effecting seams of `verify()` (the original's `_run`, `_gitless_hint_match_checks` and
/// `_terraform_flag_issued`), injectable so a golden check can run every static check without starting children.
pub struct Hooks<'a> {
    pub run: &'a dyn Fn(&str, &[String], &[i64], &Path, &[(&str, &str)]) -> PyValue,
    pub gitless: &'a dyn Fn(&Path) -> Vec<PyValue>,
    pub flag_issued: &'a dyn Fn(&Path) -> bool,
}

/// `verify()`. Uncaught for an exception the original does not catch (its traceback ends the run with exit 1).
pub fn verify(repo: &Path) -> Result<PyValue, Uncaught> {
    let hooks = Hooks { run: &run, gitless: &gitless_hint_match_checks, flag_issued: &terraform_flag_issued };
    verify_with(repo, &hooks)
}

/// `verify()` with explicit seams.
pub fn verify_with(repo: &Path, hooks: &Hooks) -> Result<PyValue, Uncaught> {
    let run = |name: &str, argv: &[String], expected: &[i64], cwd: &Path, env: &[(&str, &str)]| (hooks.run)(name, argv, expected, cwd, env);
    let rr = |name: &str, argv: &[&str], expected: &[i64]| run(name, &args(argv), expected, repo, &[]);
    let mut checks: Vec<PyValue> = Vec::new();
    checks.push(codex_discovery_checks(repo));
    checks.push(codex_discovery_selftest(repo));
    let dockerignore = must(read_text(&repo.join(".dockerignore")))?;
    let lines: BTreeSet<&str> = splitlines(&dockerignore).into_iter().map(py_strip).collect();
    checks.push(dict(vec![("name", s("codex_build_context_excludes")), ("ok", b(lines.contains(".agents/") && lines.contains(".codex/")))]));
    let scratch = tracked_scratch_paths(repo);
    checks.push(dict(vec![("name", s("scratch_dirs_untracked")), ("ok", b(scratch.is_empty())),
                          ("found", list_strs(&scratch.iter().take(20).cloned().collect::<Vec<_>>()))]));
    checks.push(dict(vec![("name", s("root_absent:scripts")), ("ok", b(!repo.join("scripts").exists()))]));

    let skills: BTreeSet<String> = glob_child_literal(&repo.join(".claude/skills"), "SKILL.md")
        .iter()
        .filter_map(|p| p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()))
        .collect();
    checks.push(dict(vec![("name", s("exact_public_skill_set")), ("ok", b(skills == set(&EXPECTED_SKILLS))),
                          ("actual", sorted(&skills)), ("expected", sorted(&set(&EXPECTED_SKILLS)))]));

    for rf in TRUST_FILES {
        let name = format!("trust_owner_paths:{rf}");
        let loaded = read_text(&repo.join(rf)).and_then(|t| {
            agent_runtime_support::pyjson::loads(&t).map_err(|e| PyError::new("JSONDecodeError", e.to_string()))
        });
        match loaded {
            Ok(doc) => {
                let mut all = Vec::new();
                walk_strings(&doc, &mut all);
                let stale: BTreeSet<String> = all.into_iter().filter(|x| x.starts_with("scripts/")).collect();
                checks.push(dict(vec![("name", s(&name)), ("ok", b(stale.is_empty())), ("stale_root_paths", sorted(&stale))]));
            }
            Err(e) => checks.push(dict(vec![("name", s(&name)), ("ok", b(false)), ("error", s(&e.colon_space()))])),
        }
    }

    let mut bare: Vec<String> = Vec::new();
    for path in shipped_python_paths(repo) {
        let rp = rel(&path, repo);
        if is_symlink(&path) {
            bare.push(format!("{rp}:OSError:shipped Python symlink is forbidden"));
            continue;
        }
        match read_text(&path) {
            Err(e) => bare.push(format!("{rp}:{}:{}", e.type_name, e.message)),
            Ok(text) => match assert_lines(&text, &rp) {
                Ok(lines) => bare.extend(lines.into_iter().map(|l| format!("{rp}:{l}"))),
                Err(msg) => bare.push(format!("{rp}:SyntaxError:{msg}")),
            },
        }
    }
    checks.push(dict(vec![("name", s("shipped_python_has_no_bare_assert")), ("ok", b(bare.is_empty())), ("violations", list_strs(&bare))]));

    let local_sync = repo.join(".claude/skills/upstream-version-watch/scripts/sync_branches.sh");
    let sub_sync = repo.join(".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh");
    match read_text(&local_sync) {
        Ok(local) => {
            let allow = shell_array(&local, "ALLOWLIST");
            let mirror = shell_array(&local, "MIRROR_DIRS");
            checks.push(dict(vec![("name", s("codex_shared_branch_sync_closure")),
                ("ok", b(["AGENTS.md", ".agents/skills", ".codex"].iter().all(|x| allow.contains(*x))
                       && [".agents/skills", ".codex"].iter().all(|x| mirror.contains(*x))
                       && local.contains("PATHS+=(':(exclude)*.topology.md')")))]));
            let local_actual = shell_array(&local, "ROOT_RELOCATION_TOMBSTONES");
            let replacements = shell_array(&local, "ROOT_RELOCATION_REPLACEMENTS");
            let local_order: &[&str] = &["git checkout ", "covered=0", "source_meta=\"$(git ls-tree",
                                         "materialized_mode=\"$(stat -c", "git rm --ignore-unmatch"];
            checks.push(dict(vec![("name", s("local_exact_relocation_tombstones")), ("ok", b(local_actual == set(&LOCAL_TOMBSTONES))),
                                  ("actual", sorted(&local_actual)), ("expected", sorted(&set(&LOCAL_TOMBSTONES)))]));
            checks.push(dict(vec![("name", s("local_exact_relocation_replacements")),
                                  ("ok", b(replacements == set(&LOCAL_REPLACEMENTS) && replacements.len() == local_actual.len())),
                                  ("actual", sorted(&replacements)), ("expected", sorted(&set(&LOCAL_REPLACEMENTS)))]));
            checks.push(order_check("local_replacement_integrity_before_tombstone", &local,
                                    "git checkout \"$SRC_BRANCH\" -- \"${PATHS[@]}\"", "echo \"[sync-branches] 완료", local_order));
        }
        Err(e) => checks.push(dict(vec![("name", s("local_exact_relocation_tombstones")), ("ok", b(false)), ("error", s(&e.colon_space()))])),
    }

    match read_text(&sub_sync).and_then(|t| sub_checks(repo, &t)) {
        Ok(items) => checks.extend(items),
        Err(e) => checks.push(dict(vec![("name", s("sub_relocation_contract")), ("ok", b(false)), ("error", s(&e.colon_space()))])),
    }

    let skeleton_root = repo.join(".claude/skills/terraforming_node/templates/document_skeletons");
    let skeletons: Vec<PathBuf> = ["plan", "devlog", "testlog", "simlog", "benchmark"].iter().map(|k| skeleton_root.join(k).join("example.md")).collect();
    checks.push(dict(vec![("name", s("terraform_owner_doc_skeletons")),
                          ("ok", b(skeletons.iter().all(|p| p.is_file() && nonempty(p)))),
                          ("paths", list_strs(&skeletons.iter().map(|p| rel(p, repo)).collect::<Vec<_>>()))]));
    let build_asset_root = repo.join(".claude/skills/upstream-version-watch/assets/build_plane");
    let mut build_assets = vec![build_asset_root.join("Dockerfile.source-build-upstage")];
    for stem in ["exaone45-33b", "hy3"] {
        for suffix in ["sh", "yaml"] {
            build_assets.push(build_asset_root.join("model_inputs/configs").join(format!("{stem}.{suffix}")));
        }
    }
    for stem in ["exaone45-33b", "hy3"] {
        build_assets.push(build_asset_root.join("model_inputs/envs").join(format!(".env.{stem}")));
    }
    checks.push(dict(vec![("name", s("upstream_owner_build_plane_assets")),
                          ("ok", b(build_assets.iter().all(|p| p.is_file() && !is_symlink(p) && nonempty(p) && mode_is_nonexec(p)))),
                          ("paths", list_strs(&build_assets.iter().map(|p| rel(p, repo)).collect::<Vec<_>>()))]));

    let node_identity_keys = ["MASTER_HOST_IP", "SLAVE_HOST_IP", "SSH_USER"];
    let pii = hintlib::pii::GENERIC_PII();
    let mut content_defects: Vec<String> = Vec::new();
    let content = (|| -> Result<(), PyError> {
        let mut envs = glob(&build_asset_root.join("model_inputs/envs"), ".env.*");
        sort_paths(&mut envs);
        for p in envs {
            let rp = rel(&p, repo);
            let text = read_text(&p)?;
            for (i, line) in splitlines(&text).into_iter().enumerate() {
                let key = py_strip(line.split('=').next().unwrap_or(""));
                if node_identity_keys.contains(&key) {
                    content_defects.push(format!("{rp}:{} Band2 노드정체성 키 '{key}' — 정본은 manifest.nodes[] → .env.cluster", i + 1));
                }
            }
        }
        let mut all = rglob(&build_asset_root.join("model_inputs"), "*");
        sort_paths(&mut all);
        for p in all {
            if !p.is_file() {
                continue;
            }
            let rp = rel(&p, repo);
            let text = read_text_replace(&p)?;
            for (i, line) in splitlines(&text).into_iter().enumerate() {
                for (name, pat) in &pii {
                    match pat.try_find_spans(line) {
                        Ok(spans) => {
                            if let Some(&(s, e)) = spans.first() {
                                content_defects.push(format!("{rp}:{} 배포 PII {name}: {}", i + 1, &line[s..e]));
                            }
                        }
                        Err(err) => content_defects.push(format!("{rp}:{} 배포 PII {name}: 스캔 불가 {err}", i + 1)),
                    }
                }
            }
        }
        Ok(())
    })();
    if let Err(e) = content {
        content_defects.push(e.colon_space());
    }
    checks.push(dict(vec![("name", s("upstream_owner_build_plane_content")), ("ok", b(content_defects.is_empty())),
                          ("defects", list_strs(&content_defects))]));

    let patch_root = build_asset_root.join("runtime_patches");
    let mut patch_defects: Vec<String> = Vec::new();
    let well_formed = |p: &Path| p.is_file() && !is_symlink(p) && nonempty(p) && mode_is_nonexec(p);
    let mut pairs_checked = 0i64;
    if patch_root.is_dir() {
        let mut pys = glob(&patch_root, "*_patch.py");
        sort_paths(&mut pys);
        for py_file in pys {
            pairs_checked += 1;
            let fname = py_file.file_name().unwrap().to_string_lossy().into_owned();
            let sidecar = py_file.with_file_name(format!("{}.provenance.json", &fname[..fname.len() - 3]));
            if !sidecar.is_file() {
                patch_defects.push(format!("{}:missing-provenance", rel(&py_file, repo)));
                continue;
            }
            for x in [&py_file, &sidecar] {
                if !well_formed(x) {
                    patch_defects.push(format!("{}:malformed", rel(x, repo)));
                }
            }
        }
        let mut sides = glob(&patch_root, "*_patch.provenance.json");
        sort_paths(&mut sides);
        for sidecar in sides {
            let fname = sidecar.file_name().unwrap().to_string_lossy().into_owned();
            let py_file = sidecar.with_file_name(format!("{}.py", &fname[..fname.len() - ".provenance.json".len()]));
            if !py_file.is_file() {
                patch_defects.push(format!("{}:orphan-provenance", rel(&sidecar, repo)));
            }
        }
    }
    if build_asset_root.is_dir() {
        for pattern in ["*_patch.py", "*_patch.provenance.json"] {
            let mut strays = rglob(&build_asset_root, pattern);
            sort_paths(&mut strays);
            for stray in strays {
                if stray.parent() != Some(patch_root.as_path()) {
                    patch_defects.push(format!("{}:misplaced-outside-runtime_patches", rel(&stray, repo)));
                }
            }
        }
    }
    let mut guard = false;
    match read_text(&sub_sync) {
        Ok(t) => guard = t.contains("compgen -G \"$build_assets/runtime_patches/*\"") && t.contains("\"$build_assets\"/runtime_patches/*"),
        Err(e) => patch_defects.push(format!("sync_to_sub.sh:unreadable:{}", e.type_name)),
    }
    if !guard {
        patch_defects.push("sync_to_sub.sh:regulated-empty-guard-missing — policy:RUNTIME_PATCH_NO_CARRY_FORWARD.C2 의 '0건 = 정상' 이 배달부에서 깨진다".into());
    }
    let mut present: Vec<String> = if patch_root.is_dir() {
        glob(&patch_root, "*_patch.py").iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect()
    } else {
        Vec::new()
    };
    present.sort();
    checks.push(dict(vec![("name", s("upstream_owner_runtime_patch_pairing")), ("ok", b(patch_defects.is_empty())),
                          ("defects", list_strs(&patch_defects)),
                          ("subject_state", s(if pairs_checked > 0 { "evaluated" } else { "regulated-empty" })),
                          ("pairs_checked", PyValue::int(pairs_checked)), ("present", list_strs(&present))]));
    let provenance = repo.join(".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md");
    checks.push(dict(vec![("name", s("shipped_arch_approval_provenance")), ("ok", b(provenance.is_file() && nonempty(&provenance)))]));
    let mut skeleton_parts = Vec::new();
    for p in &skeletons {
        if p.is_file() {
            skeleton_parts.push(must(read_text(p))?);
        }
    }
    let skeleton_text = skeleton_parts.join("\n");
    let terraforming_text = must(read_text(&repo.join(".claude/skills/terraforming_node/SKILL.md")))?;
    checks.push(dict(vec![("name", s("owner_host_safety_documentation")),
        ("ok", b(terraforming_text.contains(".claude/skills/terraforming_node/scripts/host_safety/")
               && !terraforming_text.contains("호스트 안전체계(레포 루트 `scripts/`)")))]));
    checks.push(dict(vec![("name", s("owner_doc_naming_documentation")),
        ("ok", b(!skeleton_text.contains("`scripts/doc_naming.py`") && skeleton_text.contains(".claude/policies/runtime/doc_naming.sh")))]));

    let tn = ".claude/skills/terraforming_node/scripts";
    checks.push(rr("hint_selftest", &["bash", HINT_LAUNCHER, "--self-test"], &[0]));
    checks.push(rr("hint_image_git_release_selftest", &["bash", ".claude/skills/terraforming_node/scripts/rust_bootstrap.sh", "exec", "--",
                    "cargo", "test", "--offline", "--locked", "--release", "--manifest-path", ".claude/skills/hint-publisher/rust/Cargo.toml",
                    "--target-dir", ".claude/skills/hint-publisher/rust/target", "--test", "artifacts_cases", "image_git_release"], &[0]));
    checks.push(rr("hint_bench_section_selftest", &["bash", ".claude/skills/hint-publisher/scripts/render_bench_section.sh", "--selftest"], &[0]));
    checks.push(rr("terraform_scan_selftest_legacy", &["bash", &format!("{tn}/scan_node.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_scan_p1_native_selfcheck", &["bash", &format!("{tn}/scan_node.sh"), "--p1-self-check"], &[0]));
    checks.push(rr("terraform_render_selftest", &["bash", &format!("{tn}/render_sub_env.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_node_role_contract_selftest", &["bash", &format!("{tn}/node_role_contract.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_library_access_selftest", &["bash", &format!("{tn}/library_access.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_staleness_gate_selftest", &["bash", &format!("{tn}/staleness_gate.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_manifest_contract_selftest", &["bash", &format!("{tn}/manifest_contract.sh"), "--self-test"], &[0]));
    checks.push(rr("antipattern_scan_selftest", &["bash", ANTIPATTERN_SCAN_LAUNCHER, "--self-test"], &[0]));
    checks.push(rr("constitution_lint_selftest", &["bash", CONSTITUTION_LINT_LAUNCHER, "--self-test"], &[0]));
    checks.push(rr("agent_control_selftest", &["bash", &format!("{tn}/agent_control.sh"), "--self-test"], &[0]));
    checks.push(rr("harness_switch_integration_selftest", &["bash", &format!("{tn}/selftest_harness_switch.sh")], &[0]));
    checks.push(rr("claude_native_events_selftest", &["bash", &format!("{tn}/selftest_claude_native_events.sh")], &[0]));
    checks.push(rr("terraform_turn_budget_selftest", &["bash", &format!("{tn}/turn_budget.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_relay_selftest", &["bash", &format!("{tn}/relay.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_bootstrap_canary_selftest", &["bash", &format!("{tn}/bootstrap_canary.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_bootstrap_permissions_selftest", &["bash", &format!("{tn}/selftest_bootstrap_permissions.sh")], &[0]));
    checks.push(rr("terraform_rust_bootstrap_selftest", &["bash", &format!("{tn}/selftest_rust_bootstrap.sh")], &[0]));
    checks.push(rr("terraform_sub_hint_write_scopes_selftest", &["bash", &format!("{tn}/selftest_sub_hint_write_scopes.sh")], &[0]));
    checks.push(rr("terraform_check_sub_hint_equip_selftest", &["bash", &format!("{tn}/check_sub_hint_equip.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_agent_guard_selftest", &["bash", &format!("{tn}/node_blackbox/agent_guard.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_node_identity_selftest", &["bash", &format!("{tn}/node_blackbox/node_identity.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_host_safety_mem_watchdog_selftest", &["bash", &format!("{tn}/host_safety/mem_watchdog.sh"), "--self-test"], &[0]));
    checks.push(rr("terraform_budget_renew_loop_selftest", &["bash", &format!("{tn}/node_blackbox/budget_renew_loop.sh"), "--self-test"], &[0]));
    checks.push(rr("runtime_regression_selftest", &["bash", RUNTIME_SELFTEST_LAUNCHER], &[0]));
    checks.push(rr("inline_python_scan", &["bash", INLINE_PYTHON_SCAN_LAUNCHER, "check", "--repo", "."], &[0]));
    checks.push(rr("verdict_clock_scan", &["bash", VERDICT_CLOCK_SCAN_LAUNCHER, "check", "--repo", "."], &[0]));
    checks.push(rr("verdict_clock_scan_selftest", &["bash", VERDICT_CLOCK_SCAN_LAUNCHER, "--self-test"], &[0]));
    checks.extend((hooks.gitless)(repo));
    let uw = ".claude/skills/upstream-version-watch/scripts";
    checks.push(rr("upstream_slave_forward_selftest", &["bash", &format!("{uw}/slave_forward.sh"), "--self-test"], &[0]));
    checks.push(rr("upstream_render_dockerfile_selftest", &["bash", &format!("{uw}/render_dockerfile.sh"), "--self-test"], &[0]));
    for filename in ["resolve_torch_pin.sh", "resolve_ngc_tag.sh", "resolve_build_track.sh", "resolve_wheel.sh", "render_dockerfile.sh",
                     "regen_requirements.sh", "classify_failure.sh", "check_smoke_model.sh", "slave_forward.sh"] {
        checks.push(rr(&format!("upstream_help:{filename}"), &["bash", &format!("{uw}/{filename}"), "--help"], &[0]));
    }
    if (hooks.flag_issued)(repo) {
        let na = "terraform Flag 발급 완료 — exit 4 게이트는 미테라포밍 환경에서만 발화. fresh-clone 실행 검사는 smoke_clone.sh A8 소유";
        checks.push(dict(vec![("name", s("recipe_info_only_gate")), ("ok", b(true)), ("not_applicable", s(na))]));
        checks.push(dict(vec![("name", s("benchmark_info_only_gate")), ("ok", b(true)), ("not_applicable", s(na))]));
    } else {
        checks.push(rr("recipe_info_only_gate", &["bash", ".claude/skills/vllm-recipe-explorer/scripts/recipe.sh",
                      "estimate", "--config", "/nonexistent/config.yaml", "--auto"], &[4]));
        checks.push(rr("benchmark_info_only_gate", &["bash", ".claude/skills/adversarial-benchmark/scripts/run_bench.sh", "freshclone-probe"], &[4]));
    }
    checks.push(rr("recipe_serve_knob_parity", &["bash", ".claude/skills/vllm-recipe-explorer/scripts/gen_recipe_set.sh", "--check-parity"], &[0]));
    let ab = ".claude/skills/adversarial-benchmark/scripts";
    let rx = ".claude/skills/vllm-recipe-explorer/scripts";
    let bash = |name: &str, script: &str, rest: &[&str], expected: &[i64]| -> PyValue {
        let mut argv = vec!["bash".to_string(), script.to_string()];
        argv.extend(rest.iter().map(|x| x.to_string()));
        run(name, &argv, expected, repo, &[])
    };
    checks.push(bash("benchmark_verdict_fixture", &format!("{ab}/verdict_rule.sh"), &["--measured",
        ".claude/skills/adversarial-benchmark/fixtures/measured_pass.json", "--roofline",
        ".claude/skills/adversarial-benchmark/fixtures/roofline_sample.json"], &[0]));
    checks.push(bash("benchmark_verdict_selftest", &format!("{ab}/verdict_rule.sh"), &["--self-test"], &[0]));
    checks.push(bash("recipe_parse_config_selftest", &format!("{rx}/parse_model_config.sh"), &["--self-test"], &[0]));
    checks.push(bash("recipe_estimate_vram_selftest", &format!("{rx}/estimate_vram.sh"), &["--self-test"], &[0]));
    checks.push(bash("recipe_run_trial_selftest", &format!("{rx}/run_trial.sh"), &["--self-test"], &[0]));
    checks.push(bash("campaign_init_selftest", &format!("{tn}/campaign_init.sh"), &["--selftest"], &[0]));
    checks.push(bash("campaign_lifecycle_selftest", &format!("{tn}/selftest_campaign_lifecycle.sh"), &[], &[0]));
    checks.push(bash("campaign_template_validator_selftest", &format!("{tn}/campaign_template_validator.sh"), &["--selftest"], &[0]));
    checks.push(rr("blackbox_session_selftest", &["bash", &format!("{tn}/node_blackbox/blackbox_session.sh"), "--self-test"], &[0]));
    checks.push(bash("benchmark_sweep_meta_selftest", &format!("{ab}/selftest_sweep_meta.sh"), &[], &[0]));
    checks.push(bash("benchmark_sweep_stop_selftest", &format!("{ab}/sweep_stop.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_parse_bench_native_selftest", &format!("{ab}/parse_bench.sh"), &["self-test"], &[0]));
    checks.push(bash("benchmark_roofline_native_selftest", &format!("{ab}/roofline.sh"), &["self-test"], &[0]));
    checks.push(bash("benchmark_judge_authority_required", &format!("{ab}/judge_bench.sh"), &["_probe", "--check-args"], &[2]));
    checks.push(bash("benchmark_judge_authority_accepted", &format!("{ab}/judge_bench.sh"), &["_probe", "--authority", "weak", "--check-args"], &[0]));
    checks.push(bash("benchmark_judge_bench_selftest", &format!("{ab}/judge_bench.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_bench_tool_pin_selftest", &format!("{ab}/resolve_bench_tool.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_guidellm_version_selftest", &format!("{ab}/resolve_guidellm.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_parse_guidellm_selftest", &format!("{ab}/parse_guidellm.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_classify_cell_selftest", &format!("{ab}/classify_cell.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_sweep_map_selftest", &format!("{ab}/render_sweep_map.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_naming_selftest", &format!("{ab}/bench_doc_naming.sh"), &[], &[0]));
    checks.push(bash("benchmark_dataset_prep_selftest", &format!("{ab}/dataset_prep.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_repeat_axis_selftest", &format!("{ab}/repeat_axis.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_sweep_repeats_selftest", &format!("{ab}/selftest_sweep_repeats.sh"), &[], &[0]));
    checks.push(bash("benchmark_bench_preset_selftest", &format!("{ab}/bench_preset.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_lite_report_selftest", &format!("{ab}/selftest_lite_report.sh"), &[], &[0]));
    checks.push(bash("benchmark_lite_gate_conformance", &format!("{ab}/conformance_full_superset_lite.sh"), &[], &[0]));
    checks.push(bash("benchmark_lite_gate_selftest", &format!("{ab}/selftest_lite_gate.sh"), &[], &[0]));
    checks.push(bash("upstream_single_up_requires_config", &format!("{uw}/single_serve_up.sh"), &[], &[3]));
    checks.push(bash("upstream_single_up_rejects_unknown_arg", &format!("{uw}/single_serve_up.sh"), &["_probe", "--bogus"], &[3]));
    checks.push(bash("upstream_inventory_rejects_unknown_topology", &format!("{uw}/container_inventory.sh"), &["--topology", "bogus"], &[2]));
    checks.push(bash("upstream_budget_preflight_selftest", &format!("{uw}/budget_preflight.sh"), &["--self-test"], &[0]));
    checks.push(bash("upstream_layer_ledger_selftest", &format!("{uw}/layer_ledger.sh"), &["--self-test"], &[0]));
    checks.push(bash("upstream_push_branches_selftest", &format!("{uw}/push_branches.sh"), &["--self-test"], &[0]));
    checks.push(bash("upstream_branch_sync_mechanics_selftest", &format!("{uw}/selftest_branch_sync.sh"), &[], &[0]));
    checks.push(bash("recipe_preload_ram_gate_selftest", &format!("{rx}/preload_ram_gate.sh"), &["--self-test"], &[0]));
    checks.push(bash("recipe_escalation_predicate_selftest", &format!("{rx}/escalation_predicate.sh"), &["--self-test"], &[0]));
    checks.push(bash("recipe_rope_translate_selftest", &format!("{rx}/rope_scaling_translate.sh"), &["--self-test"], &[0]));
    checks.push(bash("benchmark_broad_search_confirm_gate", &format!("{ab}/broad_search.sh"), &["cell",
        "--state", "/nonexistent/bs.json", "--now-utc", "2026-01-01T00:00:00Z",
        "--cell-key", "k", "--config", "c", "--axis-citation", "x", "--bench-budget-mib", "1"], &[5]));
    checks.push(bash("benchmark_broad_search_provenance_precheck", &format!("{ab}/selftest_broad_search_precheck.sh"), &[], &[0]));
    checks.push(bash("benchmark_broad_search_cell_args", &format!("{ab}/selftest_broad_search_cell_args.sh"), &[], &[0]));
    checks.push(bash("benchmark_resolve_topology_selftest", &format!("{ab}/resolve_topology.sh"), &["--self-test"], &[0]));
    checks.push(bash("recipe_gmu_roles_batch_selftest", &format!("{rx}/selftest_gmu_roles_batch.sh"), &[], &[0]));

    let wd = ".claude/skills/wiki-desk";
    checks.push(run("wiki_node_test", &args(&["node", "--test"]), &[0], &repo.join(wd), &[]));
    checks.push(run("wiki_frontmatter_check", &args(&["node", "--test", "checks/frontmatter.check.ts"]), &[0], &repo.join(wd), &[]));
    checks.push(rr("wiki_doc_naming_selftest", &["node", &format!("{wd}/src/doc_naming.ts"), "--self-test"], &[0]));
    checks.push(rr("doc_naming_shim_selftest", &["bash", DOC_NAMING_LAUNCHER, "--self-test"], &[0]));

    let policy_runner = repo.join(POLICY_REGISTRY_LAUNCHER);
    if policy_runner.is_file() {
        checks.push(policy_registry_check(&policy_runner, repo, |n, a, e| run(n, a, e, repo, &[])));
    } else {
        checks.push(dict(vec![("name", s("policy_registry_verify")), ("ok", b(false)),
                              ("error", s(&format!("missing {POLICY_REGISTRY_LAUNCHER}")))]));
    }

    let ssu = repo.join(".claude/skills/upstream-version-watch/scripts/single_serve_up.sh");
    match read_text(&ssu) {
        Ok(t) => {
            checks.push(order_check("single_up_grounding_before_any_state", &t,
                "echo \"$TAG 1/7 통로·전제      : DONE", "echo \"$TAG 3/7 예산 선언       : DONE\"",
                &["bash \"$_CI\" --grounding-check", "check_smoke_model.sh\" \"$CONFIG\"", "bash \"$SESSION_SH\" \"${DECL_ARGS[@]}\""]));
            checks.push(order_check("single_up_engine_log_saved_before_rollback", &t,
                "echo \"$TAG 7/7 health 대기     : FAIL — 컨테이너가 종료됐다\"", "rollback; exit 5",
                &["_save_serve_logs exited", "rollback; exit 2", "_save_serve_logs timeout"]));
        }
        Err(e) => checks.push(dict(vec![("name", s("single_up_grounding_before_any_state")), ("ok", b(false)), ("error", s(&e.colon_space()))])),
    }

    let predicate_dir = repo.join(".claude/policies/predicates");
    if predicate_dir.is_dir() {
        let pd = predicate_dir.to_string_lossy().into_owned();
        checks.push(run("production_claim_predicates", &["bash".into(), format!("{pd}/{CLAIM_PREDICATES_LAUNCHER}")], &[0], repo, &[]));
        // `python3 -m unittest discover -s <dir> -p '*_predicate.py'` → every `*_predicate.sh` launcher in name order
        // (exit 5 when there is none, unittest's "no tests ran" code).
        let loop_script = "set -u; n=0; for f in \"$1\"/*_predicate.sh; do [ -f \"$f\" ] || continue; n=$((n+1)); bash \"$f\" || exit $?; done; [ \"$n\" -gt 0 ] || exit 5";
        checks.push(run("production_companion_predicates", &["bash".into(), "-c".into(), loop_script.into(), "bash".into(), pd], &[0], repo, &[]));
    } else {
        checks.push(dict(vec![("name", s("production_claim_predicates")), ("ok", b(false)), ("error", s("missing .claude/policies/predicates"))]));
    }

    let all_ok = checks.iter().all(check_ok);
    Ok(dict(vec![
        ("schema_version", PyValue::int(1)),
        ("repo", s(&repo.to_string_lossy())),
        ("verdict", s(if all_ok { "PASS" } else { "FAIL" })),
        ("checks", PyValue::List(checks)),
    ]))
}
