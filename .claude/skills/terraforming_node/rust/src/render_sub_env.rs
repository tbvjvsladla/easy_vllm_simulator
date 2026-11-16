//! Port of `render_sub_env.py` — terraforming_node's deterministic renderer (substitution only, no judgement): the main's
//! `output/<topology>/manifest.yaml` node identity is substituted into the PII-free `{{ ... }}` templates of `sub_node/`
//! and the sub-node agent environment is written to the gitignored staging tree `output/<topology>/sub_provision/`.
//! Same command line, exit codes (0 ok · 2 required manifest field missing · 3 input/IO · 5 topology-rules mismatch),
//! standard output/error text and files as the original.
//!
//! The in-process imports of the original are answered natively: `node_role_contract` and `topology_parity` by the
//! `role-contract` library, `manifest_contract._load_manifest` / `evaluate_contract` by `role_contract::manifest_gate`,
//! `agent_card_contract` by [`crate::agent_card`]. No Python is started.
//!
//! **Delivery lists (intentional change, plan_26111615).** The sub-node delivery carries no `.py` at all: every harness
//! program reaches the Sub as its `.sh` launcher plus the Rust crate SOURCES that build it (no `target/` — each node builds
//! for its own architecture with the delivered `build_rust_packages.sh` / adversarial-benchmark `build_rust.sh`). The
//! runtime-block copies skip tracked `.py` files ([`is_retired_py`]); the explicit lists name launchers only
//! (campaign tools, contract readers, the policy launchers `completion_gate.sh` / `evidence_publisher.sh` / `doc_naming.sh` /
//! `topology_parity.sh`, node_blackbox, the hint-equip judge and the permission probe), and the crates they need
//! (runtime-support, role-contract, parity, policy-gate, policy-publisher, hint-publisher, terraforming_node, and the owner
//! crates of the runtime blocks) are delivered with their path-dependency closure checked ([`assert_rust_path_closure`]).

mod selftest;
pub mod textre;
pub mod toml;

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::raise::Raised;
use crate::sysexit::Exit;
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DOC_TYPES: [&str; 5] = ["plan", "devlog", "testlog", "simlog", "benchmark"];

pub const RUNTIME_BLOCK_NAMES: [&str; 5] = [
    "vllm-recipe-explorer",
    "adversarial-benchmark",
    "upstream-version-watch",
    "hint-publisher",
    "wiki-desk",
];

pub const HINT_ENGINE_DEPENDENCIES: [&str; 8] = [
    "hints/HINT_ISSUANCE_CONTRACT.md",
    "hints/vocab.json",
    ".claude/policies/runtime/completion_gate.sh",
    ".claude/policies/runtime/evidence_publisher.sh",
    ".claude/schemas/work-manifest.schema.json",
    ".claude/schemas/completion-manifest.schema.json",
    ".claude/schemas/side-effect-authorization.schema.json",
    ".claude/policies/runtime/doc_naming.sh",
];
/// The 4-way parity predicate's launcher (Rust `topology_parity` of the adversarial-benchmark crate; was `topology_parity.py`).
pub const SUB_RUST_BUILD_INPUTS: [&str; 1] = [".claude/policies/runtime/topology_parity.sh"];
/// Files the B self-checks read besides the delivered trees. Empty since plan_26111615 pass 3: the hint reader those checks
/// asked (`render_bench_section.py`) is the hint-publisher Rust library, delivered as [`HINT_RUST_LIBRARY`].
pub const SUB_B_CHECK_INPUTS: [&str; 0] = [];
pub fn sub_b_evidence_deps() -> Vec<&'static str> {
    HINT_ENGINE_DEPENDENCIES.iter().copied().filter(|r| !r.starts_with("hints/")).collect()
}
pub const SUB_BENCH_FOUNDATION_RUNTIME: [&str; 3] = [
    ".claude/policies/runtime/topology_parity.sh",
    ".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh",
    ".claude/skills/terraforming_node/scripts/scan_node.sh",
];
pub const HINT_NEVER_DELIVER: [&str; 5] =
    ["hints/index.json", "HINTS.md", "hints/.central_authority", "hints/.push_authority", "hints/.drafts"];
pub const HINT_ENGINE_TOLERATED_ABSENT: [&str; 1] = [".claude/rules/docs.md"];
/// The hint-equip judge, delivered as its Rust launcher (was `check_sub_hint_equip.py`).
pub const SUB_HINT_EQUIP_TOOLS: [&str; 1] = ["check_sub_hint_equip.sh"];
/// The campaign state writer/validator launchers (Rust `campaign_init` / `campaign_template_validator` of the
/// adversarial-benchmark crate).
pub const CAMPAIGN_TOOLS: [&str; 2] = ["campaign_init.sh", "campaign_template_validator.sh"];
/// The contract reader launchers (Rust `manifest_contract` of the adversarial-benchmark crate, `node_role_contract` of the
/// upstream runtime crate).
pub const SUB_CONTRACT_READERS: [&str; 2] = ["manifest_contract.sh", "node_role_contract.sh"];
/// The local permission canary, delivered as its Rust launcher (was `harness_permission_probe.py`).
pub const SUB_PERMISSION_PROBES: [&str; 1] = ["harness_permission_probe.sh"];
pub const SUB_RUST_BOOTSTRAP: [&str; 3] = ["rust_bootstrap.sh", "rust_toolchain.pin", "build_rust_packages.sh"];
pub const RUST_OWNER_MANIFESTS: [(&str, &str); 3] = [
    ("adversarial-benchmark", ".claude/skills/adversarial-benchmark/rust/Cargo.toml"),
    ("vllm-recipe-explorer", ".claude/skills/vllm-recipe-explorer/rust/runtime/Cargo.toml"),
    ("upstream-version-watch", ".claude/skills/upstream-version-watch/rust/runtime/Cargo.toml"),
];
pub const SHARED_RUST_SUPPORT: &str = ".claude/rust/runtime-support";
pub const ROLE_RUST_SUPPORT: &str = ".claude/rust/role-contract";
pub const PARITY_RUST_TOOL: &str = ".claude/rust/parity";
/// The terraforming_node Rust crate (launchers of the delivered terraforming programs + `rust_package_check`).
pub const TERRAFORMING_RUST: &str = ".claude/skills/terraforming_node/rust";
/// Policy crates the adversarial-benchmark crate path-depends on.
pub const POLICY_RUST_LIBRARIES: [&str; 2] = [".claude/rust/policy-gate", ".claude/rust/policy-publisher"];
/// The hint-publisher crate (library `hintlib`): the adversarial-benchmark crate path-depends on it for the hint reader its
/// lite/sweep self-checks ask (plan_26111615 pass 3), so it is delivered with the B crate whether or not the hint-publisher
/// skill (its scripts and the `hints/` plane) is.
pub const HINT_RUST_LIBRARY: &str = ".claude/skills/hint-publisher/rust";
pub const RUST_ONLY_PREFIXES: [&str; 2] = ["rust/", "scripts/"];
pub fn rust_only_inputs(name: &str) -> &'static [&'static str] {
    match name {
        "adversarial-benchmark" => &["config/", "fixtures/", "bench_tool_pin.json"],
        "upstream-version-watch" => &["failure_patterns.yaml"],
        _ => &[],
    }
}
/// `SHARED_RUST_LIBRARIES` (copy order).
pub fn shared_rust_libraries() -> Vec<&'static str> {
    let mut v = vec![SHARED_RUST_SUPPORT, ROLE_RUST_SUPPORT, PARITY_RUST_TOOL];
    v.extend(POLICY_RUST_LIBRARIES);
    v.push(HINT_RUST_LIBRARY);
    v.push(TERRAFORMING_RUST);
    v
}
pub const RUST_RUNTIME_REFERENCE: &str = ".claude/skills/terraforming_node/references/install/rust_runtime.md";
pub const UPSTREAM_EXCLUDES: [&str; 17] = [
    "assets/current-production-resolution.json",
    "scripts/sync_to_sub.sh",
    "scripts/sync_branches.sh",
    "scripts/fetch_sub_docs.sh",
    "scripts/smoke_clone.sh",
    "scripts/multinode_comms_smoke.sh",
    "scripts/multinode_serve_smoke.sh",
    "scripts/native_multinode_serve.py",
    "scripts/native_wheelhouse.sh",
    "scripts/native_multinode_serve.sh",
    "scripts/selftest_branch_sync.py",
    "scripts/selftest_branch_sync.sh",
    "scripts/closing_sequence.sh",
    "scripts/layer_ledger.py",
    "scripts/layer_ledger.sh",
    "scripts/push_branches.py",
    "scripts/push_branches.sh",
];
pub fn runtime_block_excludes(name: &str) -> Option<&'static [&'static str]> {
    if name == "upstream-version-watch" {
        Some(&UPSTREAM_EXCLUDES)
    } else {
        None
    }
}
pub const KNOWN_SUB_MODES: [&str; 2] = ["ray-worker", "a2a-agent"];
pub const RECIPE_REFERENCE_REL: &str = ".claude/skills/wiki-desk/reference/references.md";

// ── errors ─────────────────────────────────────────────────────────────────────────────────────────────────────────

/// What escapes a step: a `SystemExit` or an uncaught exception (traceback).
#[derive(Debug)]
pub enum Fail {
    Sys(Exit),
    Raise(Raised),
}
impl From<Exit> for Fail {
    fn from(e: Exit) -> Self {
        Fail::Sys(e)
    }
}
impl From<Raised> for Fail {
    fn from(e: Raised) -> Self {
        Fail::Raise(e)
    }
}
impl Fail {
    /// `str(e)` of a caught `SystemExit`.
    pub fn text(&self) -> String {
        match self {
            Fail::Sys(Exit::Msg(m)) => m.clone(),
            Fail::Sys(Exit::Code(c)) => c.to_string(),
            Fail::Raise(r) => r.0.clone(),
        }
    }
    fn finish(&self) -> i32 {
        match self {
            Fail::Sys(e) => e.finish(),
            Fail::Raise(r) => crate::raise::exit(r),
        }
    }
}
pub type Res<T> = Result<T, Fail>;

fn sys(m: impl Into<String>) -> Fail {
    Fail::Sys(Exit::Msg(m.into()))
}

// ── paths and files (Python `os.path` / `shutil` over text paths) ───────────────────────────────────────────────────

/// `os.path.join(a, b)`.
pub fn pj(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        b.to_string()
    } else if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn isfile(p: &str) -> bool {
    Path::new(p).is_file()
}
pub fn isdir(p: &str) -> bool {
    Path::new(p).is_dir()
}
pub fn exists(p: &str) -> bool {
    Path::new(p).exists()
}

fn os_err(e: &std::io::Error, p: &str) -> Fail {
    Fail::Raise(Raised::os(e, p))
}

/// Universal-newline text read (`open(path, encoding="utf-8").read()`).
pub fn read_text(p: &str) -> Res<String> {
    let b = std::fs::read(p).map_err(|e| os_err(&e, p))?;
    let s = String::from_utf8(b).map_err(|e| {
        Fail::Raise(Raised::new(
            "UnicodeDecodeError",
            &format!("'utf-8' codec can't decode byte 0x{:02x} in position {}: invalid start byte", e.as_bytes()[e.utf8_error().valid_up_to()], e.utf8_error().valid_up_to()),
        ))
    })?;
    Ok(s.replace("\r\n", "\n").replace('\r', "\n"))
}

pub fn read_bytes(p: &str) -> Res<Vec<u8>> {
    std::fs::read(p).map_err(|e| os_err(&e, p))
}

/// `os.makedirs(p, exist_ok=True)`.
pub fn makedirs(p: &str) -> Res<()> {
    if p.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(p).map_err(|e| os_err(&e, p))
}

pub fn dirname(p: &str) -> String {
    match p.rfind('/') {
        Some(0) => "/".into(),
        Some(i) => p[..i].trim_end_matches('/').to_string().or_root(),
        None => String::new(),
    }
}
trait OrRoot {
    fn or_root(self) -> String;
}
impl OrRoot for String {
    fn or_root(self) -> String {
        if self.is_empty() {
            "/".into()
        } else {
            self
        }
    }
}

/// `open(dst, "w").write(text)`.
pub fn write_text(p: &str, text: &str) -> Res<()> {
    std::fs::write(p, text).map_err(|e| os_err(&e, p))
}

/// `shutil.copyfile(src, dst)`: content only.
pub fn copyfile(src: &str, dst: &str) -> Res<()> {
    let b = read_bytes(src)?;
    std::fs::write(dst, b).map_err(|e| os_err(&e, dst))
}

/// `shutil.copy2(src, dst)`: content, permission bits and timestamps.
pub fn copy2(src: &str, dst: &str) -> Res<()> {
    std::fs::copy(src, dst).map_err(|e| os_err(&e, src))?;
    if let Ok(meta) = std::fs::metadata(src) {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(dst) {
            let mut times = std::fs::FileTimes::new();
            if let Ok(m) = meta.modified() {
                times = times.set_modified(m);
            }
            if let Ok(a) = meta.accessed() {
                times = times.set_accessed(a);
            }
            let _ = f.set_times(times);
        }
    }
    Ok(())
}

pub fn chmod(p: &str, mode: u32) -> Res<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).map_err(|e| os_err(&e, p))
}

/// `os.walk(top)` (top-down, `os.scandir` order): (root, dirs, files).
pub fn walk(top: &str) -> Vec<(String, Vec<String>, Vec<String>)> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(top) else {
        return out;
    };
    let (mut dirs, mut files) = (vec![], vec![]);
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let is_dir = e.file_type().map(|t| {
            if t.is_symlink() {
                Path::new(&pj(top, &name)).is_dir()
            } else {
                t.is_dir()
            }
        });
        if is_dir.unwrap_or(false) {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    out.push((top.to_string(), dirs.clone(), files));
    for d in dirs {
        let p = pj(top, &d);
        // os.walk does not follow directory symlinks by default
        if std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            continue;
        }
        out.extend(walk(&p));
    }
    out
}

/// `os.path.relpath(path, start)` for `path` under `start` (both absolute or both relative to the same base).
pub fn relpath(path: &str, start: &str) -> String {
    let norm = |s: &str| crate::pathfmt::py_path(s);
    let (p, s) = (norm(path), norm(start));
    let pp: Vec<&str> = p.split('/').filter(|x| !x.is_empty()).collect();
    let sp: Vec<&str> = s.split('/').filter(|x| !x.is_empty()).collect();
    let mut i = 0;
    while i < pp.len() && i < sp.len() && pp[i] == sp[i] {
        i += 1;
    }
    let mut parts: Vec<&str> = std::iter::repeat("..").take(sp.len() - i).collect();
    parts.extend(&pp[i..]);
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// `shutil.rmtree(p)`.
fn rmtree(p: &str) -> Res<()> {
    std::fs::remove_dir_all(p).map_err(|e| os_err(&e, p))
}

// ── the program's layout ─────────────────────────────────────────────────────────────────────────────────────────────

/// `HERE`, `REPO` and the paths derived from them.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub here: String,
    pub repo: String,
}

impl Ctx {
    pub fn from_owner() -> Ctx {
        let owner = crate::owner::owner_path("render_sub_env.py");
        let here = owner.parent().map(Path::to_path_buf).unwrap_or_default();
        let repo: PathBuf = here.ancestors().nth(4).map(Path::to_path_buf).unwrap_or_default();
        Ctx { here: here.display().to_string(), repo: repo.display().to_string() }
    }
    pub fn skill_dir(&self) -> String {
        dirname(&self.here)
    }
    pub fn subnode_dir(&self) -> String {
        pj(&self.skill_dir(), "sub_node")
    }
    pub fn topology_rules(&self) -> String {
        pj(&self.repo, ".claude/rules/strategy.topology.md")
    }
    pub fn handoff_reference(&self) -> String {
        pj(&self.repo, ".claude/policies/handoff_exception.topology.md")
    }
    pub fn doc_skeletons(&self) -> String {
        pj(&self.skill_dir(), "templates/document_skeletons")
    }
    pub fn runtime_block_path(&self, name: &str) -> Option<String> {
        RUNTIME_BLOCK_NAMES.contains(&name).then(|| pj(&self.repo, &format!(".claude/skills/{name}")))
    }
}

// ── MODE blocks / prefixes ───────────────────────────────────────────────────────────────────────────────────────────

fn tuple_repr(items: &[&str]) -> String {
    format!("({})", items.iter().map(|s| PyStr::new(s).repr()).collect::<Vec<_>>().join(", "))
}

pub fn list_repr<S: AsRef<str>>(items: &[S]) -> String {
    format!("[{}]", items.iter().map(|s| PyStr::new(s.as_ref()).repr()).collect::<Vec<_>>().join(", "))
}

/// `apply_mode_blocks(text, sub_mode)`.
pub fn apply_mode_blocks(text: &str, sub_mode: &str) -> Res<String> {
    let mut seen: Vec<String> = vec![];
    let out = textre::sub_mode_blocks(text, |mode, body| {
        if !seen.iter().any(|s| s == mode) {
            seen.push(mode.to_string());
        }
        if !KNOWN_SUB_MODES.contains(&mode) {
            return Err(sys(format!(
                "[render] FAIL: 미지의 MODE 블록 '{mode}' — 알려진 모드 {}",
                tuple_repr(&KNOWN_SUB_MODES)
            )));
        }
        Ok(if mode == sub_mode { body.to_string() } else { String::new() })
    })?;
    if !seen.is_empty() && !seen.iter().any(|s| s == sub_mode) {
        let mut sorted = seen.clone();
        sorted.sort();
        return Err(sys(format!(
            "[render] FAIL: sub_mode='{sub_mode}' 용 MODE 블록이 템플릿에 없다(있는 것: {})",
            list_repr(&sorted)
        )));
    }
    Ok(out)
}

/// `apply_mode_prefixes(obj, sub_mode)` (recursive over dicts and lists).
pub fn apply_mode_prefixes(obj: &PyValue, sub_mode: &str) -> Res<PyValue> {
    Ok(match obj.peel() {
        PyValue::Dict(d) => {
            let mut nd = PyDict::new();
            for (k, v) in d.iter() {
                nd.insert(k.clone(), apply_mode_prefixes(v, sub_mode)?);
            }
            PyValue::Dict(nd)
        }
        PyValue::List(l) => {
            let mut out = vec![];
            for item in l {
                if let PyValue::Str(s) = item.peel() {
                    if let Some((mode, rest)) = textre::mode_prefix(&crate::py::text(s)) {
                        if !KNOWN_SUB_MODES.contains(&mode.as_str()) {
                            return Err(sys(format!(
                                "[render] FAIL: 미지의 MODE 접두 '{mode}' — 알려진 모드 {}",
                                tuple_repr(&KNOWN_SUB_MODES)
                            )));
                        }
                        if mode != sub_mode {
                            continue;
                        }
                        out.push(PyValue::str(&rest));
                        continue;
                    }
                }
                out.push(apply_mode_prefixes(item, sub_mode)?);
            }
            PyValue::List(out)
        }
        other => other.clone(),
    })
}

// ── manifest parsing (the original's own minimal reader, not YAML) ───────────────────────────────────────────────────

/// One value of the minimal manifest reader: text, a nested mapping section, or the `nodes` list.
#[derive(Clone, Debug, PartialEq)]
pub enum MVal {
    Str(String),
    Map(Vec<(String, MVal)>),
    List(Vec<Vec<(String, MVal)>>),
}

/// The ordered dict the original builds (`data`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Man(pub Vec<(String, MVal)>);

pub fn map_get<'a>(m: &'a [(String, MVal)], k: &str) -> Option<&'a MVal> {
    m.iter().find(|(a, _)| a == k).map(|(_, v)| v)
}
pub fn map_set(m: &mut Vec<(String, MVal)>, k: &str, v: MVal) {
    if let Some(slot) = m.iter_mut().find(|(a, _)| a == k) {
        slot.1 = v;
    } else {
        m.push((k.to_string(), v));
    }
}
pub fn map_str(m: &[(String, MVal)], k: &str) -> String {
    match map_get(m, k) {
        Some(MVal::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

impl Man {
    pub fn get(&self, k: &str) -> Option<&MVal> {
        map_get(&self.0, k)
    }
    pub fn set(&mut self, k: &str, v: MVal) {
        map_set(&mut self.0, k, v)
    }
    pub fn pop(&mut self, k: &str) {
        self.0.retain(|(a, _)| a != k);
    }
    pub fn s(&self, k: &str) -> String {
        map_str(&self.0, k)
    }
    pub fn section(&self, k: &str) -> Vec<(String, MVal)> {
        match self.get(k) {
            Some(MVal::Map(m)) => m.clone(),
            _ => vec![],
        }
    }
    pub fn nodes(&self) -> Vec<Vec<(String, MVal)>> {
        match self.get("nodes") {
            Some(MVal::List(l)) => l.clone(),
            _ => vec![],
        }
    }
    /// `_node(data, role)` (a copy; `node_mut` for in-place changes).
    pub fn node(&self, role: &str) -> Vec<(String, MVal)> {
        self.nodes().into_iter().find(|n| map_str(n, "role") == role).unwrap_or_default()
    }
    pub fn node_mut(&mut self, role: &str) -> Option<&mut Vec<(String, MVal)>> {
        match self.0.iter_mut().find(|(a, _)| a == "nodes") {
            Some((_, MVal::List(l))) => l.iter_mut().find(|n| map_str(n, "role") == role),
            _ => None,
        }
    }
    pub fn to_py(&self) -> PyValue {
        fn conv(v: &MVal) -> PyValue {
            match v {
                MVal::Str(s) => PyValue::str(s),
                MVal::Map(m) => map_py(m),
                MVal::List(l) => PyValue::List(l.iter().map(|n| map_py(n)).collect()),
            }
        }
        fn map_py(m: &[(String, MVal)]) -> PyValue {
            let mut d = PyDict::new();
            for (k, v) in m {
                d.set(k, conv(v));
            }
            PyValue::Dict(d)
        }
        map_py(&self.0)
    }
}

fn pystrip(s: &str) -> &str {
    crate::py::strip(s)
}

/// `_clean(v)`.
fn clean(v: &str) -> String {
    let v = pystrip(v.split('#').next().unwrap_or(""));
    let v = pystrip(v).trim_matches('"').trim_matches('\'');
    pystrip(v).to_string()
}

/// `parse_manifest(path)`.
pub fn parse_manifest(path: &str) -> Res<Man> {
    let text = read_text(path)?;
    parse_manifest_text(&text)
}

pub fn parse_manifest_text(text: &str) -> Res<Man> {
    let mut data = Man(vec![
        ("interconnect".into(), MVal::Map(vec![])),
        ("network".into(), MVal::Map(vec![])),
        ("nodes".into(), MVal::List(vec![])),
    ]);
    let mut section: Option<&str> = None;
    let mut have_cur = false;
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    if lines.is_empty() {
        lines = vec![];
    }
    for raw in lines {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let lstripped = line.trim_start_matches(crate::py::is_space);
        if pystrip(line).is_empty() || lstripped.starts_with('#') {
            continue;
        }
        let indent = line.chars().count() - lstripped.chars().count();
        let s = pystrip(line);
        if indent == 0 {
            if s.starts_with("interconnect:") {
                section = Some("interconnect");
                have_cur = false;
                continue;
            }
            if s.starts_with("network:") {
                section = Some("network");
                have_cur = false;
                continue;
            }
            if s.starts_with("nodes:") {
                section = Some("nodes");
                have_cur = false;
                continue;
            }
            section = None;
            have_cur = false;
            let (k, v) = partition(s);
            data.set(pystrip(k), MVal::Str(clean(v)));
        } else if let Some(sec @ ("interconnect" | "network")) = section {
            let (k, v) = partition(s);
            if let Some((_, MVal::Map(m))) = data.0.iter_mut().find(|(a, _)| a == sec) {
                map_set(m, pystrip(k), MVal::Str(clean(v)));
            } else {
                // the section key was overwritten by a later top-level key: `data[section][k] = ...` on a str
                return Err(Fail::Raise(Raised::new("TypeError", "'str' object does not support item assignment")));
            }
        } else if section == Some("nodes") {
            if s.starts_with("- role:") {
                let role = clean(s.split_once(':').map(|x| x.1).unwrap_or(""));
                match data.0.iter_mut().find(|(a, _)| a == "nodes") {
                    Some((_, MVal::List(l))) => l.push(vec![("role".into(), MVal::Str(role))]),
                    _ => return Err(Fail::Raise(Raised::new("AttributeError", "'str' object has no attribute 'append'"))),
                }
                have_cur = true;
            } else if have_cur && s.contains(':') {
                let (k, v) = partition(s);
                if let Some((_, MVal::List(l))) = data.0.iter_mut().find(|(a, _)| a == "nodes") {
                    if let Some(cur) = l.last_mut() {
                        map_set(cur, pystrip(k), MVal::Str(clean(v)));
                    }
                }
            }
        }
    }
    Ok(data)
}

fn partition(s: &str) -> (&str, &str) {
    match s.split_once(':') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    }
}

// ── placeholders ──────────────────────────────────────────────────────────────────────────────────────────────────────

pub type Ph = BTreeMap<String, String>;

fn or(a: String, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        a
    }
}

/// `build_placeholders(data, sub_manifest)` → (placeholders, missing_required).
pub fn build_placeholders(ctx: &Ctx, data: &Man, sub_manifest: Option<&Man>) -> Res<(Ph, Vec<String>)> {
    let hw = match sub_manifest {
        Some(m) if !m.0.is_empty() => m,
        _ => data,
    };
    let ic = data.section("interconnect");
    let sub = data.node("sub");
    let main = data.node("main");
    let sub_host = map_str(&sub, "host");
    let work_dir = or(map_str(&sub, "work_dir"), &map_str(&main, "work_dir"));
    let ssh_user = or(map_str(&sub, "ssh_user"), &map_str(&main, "ssh_user"));
    let mut ph = Ph::new();
    let mut put = |k: &str, v: String| {
        ph.insert(k.to_string(), v);
    };
    put("SUB_HOST", sub_host.clone());
    put("SUB_HOSTNAME", or(map_str(&sub, "hostname"), &sub_host));
    put("MASTER_HOST", map_str(&main, "host"));
    put("SSH_USER", ssh_user);
    put("WORKSPACE_PATH", work_dir);
    put("NAS_MOUNT", hw.s("nas_model_path"));
    put("CPU_ARCH", hw.s("cpu_arch"));
    put("TOPOLOGY", data.s("topology"));
    put("INTERCONNECT", map_str(&ic, "type"));
    put("INTERCONNECT_IFACE", map_str(&ic, "socket_iface"));
    put("INTERCONNECT_MTU", or(map_str(&ic, "mtu"), "9000"));
    put("GID_INDEX", or(map_str(&ic, "gid_index"), "null"));
    put("HCA_DEVICES", or(map_str(&ic, "hca_devices"), "[]"));
    put("PLATFORM_PRESET", map_str(&ic, "platform_preset"));
    put("RAY_PORT", or(data.s("ray_port"), "6379"));
    put("GPU_MODEL", hw.s("gpu_model"));
    put("SUB_HW_VERIFIED", map_str(&sub, "hw_verified"));
    // `(hw.get("network") or {}).get("egress") or "unknown"`
    let egress = match hw.get("network") {
        Some(MVal::Map(m)) => map_str(m, "egress"),
        _ => String::new(),
    };
    put("EGRESS_STATE", or(egress, "unknown"));
    for (k, v) in contract_placeholders(ctx, data)? {
        ph.insert(k, v);
    }
    let required = [
        "SUB_HOST", "MASTER_HOST", "SSH_USER", "WORKSPACE_PATH", "NAS_MOUNT", "CPU_ARCH", "GPU_MODEL",
        "INTERCONNECT", "INTERCONNECT_IFACE", "SUB_MODE", "SUB_MODE_SOURCE", "SUB_RANK_SOURCE",
    ];
    let missing = required
        .iter()
        .filter(|k| ph.get(**k).map_or(true, |v| v.is_empty()))
        .map(|k| k.to_string())
        .collect();
    Ok((ph, missing))
}

fn pv_text(v: Option<&PyValue>) -> String {
    match v.map(PyValue::peel) {
        Some(x) if x.is_truthy() => crate::py::text(&x.display_text()),
        _ => String::new(),
    }
}

fn pv_strs(v: Option<&PyValue>) -> Vec<String> {
    match v.map(PyValue::peel) {
        Some(PyValue::List(l)) => l.iter().map(|x| crate::py::text(&x.display_text())).collect(),
        _ => vec![],
    }
}

/// `node_role_contract.evaluate_manifest(data, topology=data.get("topology") or None, role="sub")` as a dict.
pub fn evaluate_contract_manifest(data: &Man, topology: Option<&str>) -> PyValue {
    let topo = match topology {
        Some(t) if !t.is_empty() => PyValue::str(t),
        _ => PyValue::None,
    };
    role_contract::evaluate_manifest(&data.to_py(), &topo, &PyValue::str("sub")).to_value()
}

/// `_contract_placeholders(data)`.
pub fn contract_placeholders(ctx: &Ctx, data: &Man) -> Res<Vec<(String, String)>> {
    let topo = data.s("topology");
    let res = evaluate_contract_manifest(data, Some(&topo));
    let sec = |k: &str| crate::py::get(&res, k).cloned().unwrap_or(PyValue::None);
    let sub_mode = sec("sub_mode");
    let rank = sec("rank");
    let tp = sec("tool_plane");
    let ia = sec("identity_authority");
    let dp = sec("delivery_plane");
    let tp_value = pv_strs(crate::py::get(&tp, "value"));
    let rank_value = crate::py::get(&rank, "value").cloned().unwrap_or(PyValue::None);
    let rank_text = match rank_value.peel() {
        PyValue::None => "null".to_string(),
        PyValue::Int(n) => n.to_string(),
        PyValue::Bool(b) => (if *b { "1" } else { "0" }).to_string(),
        PyValue::Float(f) => format!("{}", f.trunc() as i64),
        other => crate::py::text(&other.display_text()),
    };
    let tp_json = PyValue::List(tp_value.iter().map(|s| PyValue::str(s)).collect());
    let support = support_delivery(ctx, &tp_value)?;
    let support_json = PyValue::List(support.iter().map(|s| PyValue::str(s)).collect());
    let viol = match crate::py::get(&res, "violations").map(PyValue::peel) {
        Some(PyValue::List(l)) => l
            .iter()
            .map(|v| crate::py::get_str(v, "code").unwrap_or_else(|| "?".into()))
            .collect::<Vec<_>>()
            .join(","),
        _ => String::new(),
    };
    let dumps = |v: &PyValue| crate::py::text(&v.dumps_unicode().expect("list of str"));
    Ok(vec![
        ("SUB_MODE".into(), pv_text(crate::py::get(&sub_mode, "value"))),
        ("SUB_MODE_SOURCE".into(), pv_text(crate::py::get(&sub_mode, "source"))),
        ("IDENTITY_AUTHORITY".into(), pv_text(crate::py::get(&ia, "value"))),
        ("DELIVERY_PLANE".into(), pv_text(crate::py::get(&dp, "value"))),
        ("TOOL_PLANE_JSON".into(), dumps(&tp_json)),
        ("TOOL_PLANE".into(), tp_value.join(",")),
        ("TOOL_PLANE_SOURCE".into(), pv_text(crate::py::get(&tp, "source"))),
        ("SUB_RANK".into(), rank_text),
        ("SUB_RANK_SOURCE".into(), pv_text(crate::py::get(&rank, "source"))),
        ("SUPPORT_DELIVERY_JSON".into(), dumps(&support_json)),
        ("SUB_CONTRACT_VIOLATIONS".into(), viol),
    ])
}

/// A retired Python original (plan_26111615): its Rust launcher is the owner and no `.py` is delivered to a Sub, whether or
/// not this checkout still carries it — the delivered tree is the same with or without the originals. Tracked `.py` files
/// inside a copied tree (runtime blocks, crate check data) are skipped; the build of every delivered crate is `.py`-free.
pub fn is_retired_py(rel: &str) -> bool {
    rel.ends_with(".py")
}

fn tn_script(t: &str) -> String {
    format!(".claude/skills/terraforming_node/scripts/{t}")
}

fn delivered_tn_tools() -> Vec<&'static str> {
    let mut v: Vec<&str> = CAMPAIGN_TOOLS.to_vec();
    v.extend(SUB_CONTRACT_READERS);
    v.extend(SUB_PERMISSION_PROBES);
    v.extend(SUB_RUST_BOOTSTRAP);
    v
}

/// `support_delivery(tool_plane)`.
pub fn support_delivery(ctx: &Ctx, tool_plane: &[String]) -> Res<Vec<String>> {
    let mut out: Vec<String> = delivered_tn_tools().iter().map(|t| tn_script(t)).collect();
    out.push(RUST_RUNTIME_REFERENCE.into());
    out.push(format!("{TERRAFORMING_RUST}/"));
    out.extend(SUB_RUST_BUILD_INPUTS.iter().map(|s| s.to_string()));
    let rules = ctx.topology_rules();
    if isfile(&rules) && read_text(&rules)?.contains("handoff_exception.topology.md") {
        out.push(".claude/policies/handoff_exception.topology.md".into());
    }
    for r in SUB_BENCH_FOUNDATION_RUNTIME {
        if !out.iter().any(|x| x == r) {
            out.push(r.into());
        }
    }
    for r in sub_b_evidence_deps() {
        if !out.iter().any(|x| x == r) {
            out.push(r.into());
        }
    }
    if tool_plane.iter().any(|t| t == "hint-publisher") {
        for r in HINT_ENGINE_DEPENDENCIES {
            if !out.iter().any(|x| x == r) {
                out.push(r.into());
            }
        }
        out.extend(SUB_HINT_EQUIP_TOOLS.iter().map(|t| tn_script(t)));
    }
    Ok(out)
}

fn git(repo: &str, args: &[&str]) -> Result<crate::proc::Completed, Raised> {
    let mut argv: Vec<String> = vec!["git".into(), "-C".into(), repo.into()];
    argv.extend(args.iter().map(|s| s.to_string()));
    crate::proc::run(&argv, None, Some(20))
}

/// `_tracked_or_die(rel, tracked_list)`.
pub fn tracked_or_die(ctx: &Ctx, rel: &str, tracked: Option<&[String]>) -> Res<()> {
    if let Some(list) = tracked {
        if !list.iter().any(|x| x == rel) {
            return Err(sys(format!("[render] FAIL: hint 엔진 의존이 추적물이 아니다(주입 목록에 없음): {rel}")));
        }
        return Ok(());
    }
    let out = git(&ctx.repo, &["ls-files", "--error-unmatch", "--", rel])?;
    if out.returncode != 0 {
        return Err(sys(format!(
            "[render] FAIL: hint 엔진 의존이 추적물이 아니다(git ls-files rc={}): {rel}",
            out.returncode
        )));
    }
    Ok(())
}

pub fn render_text(text: &str, ph: &Ph) -> String {
    textre::render_text(text, |k| ph.get(k).cloned())
}

/// `_unrendered(text)`: sorted unique placeholder keys left.
pub fn unrendered(text: &str) -> Vec<String> {
    let mut v: Vec<String> = textre::placeholders(text).into_iter().map(|(_, _, k)| k).collect();
    v.sort();
    v.dedup();
    v
}

// ── hint write scopes ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The scope verdict: `status` and (when declared) the write paths.
#[derive(Clone, Debug, PartialEq)]
pub struct Scopes {
    pub status: &'static str,
    pub write_paths: Vec<String>,
}

fn pure_posix_parts(v: &str) -> Vec<String> {
    let p = crate::pathfmt::py_path(v);
    let mut parts = vec![];
    if p.starts_with('/') {
        parts.push("/".to_string());
    }
    parts.extend(p.split('/').filter(|x| !x.is_empty()).map(String::from));
    parts
}

/// `child == parent or parent in child.parents` for canonical absolute paths.
fn inside(child: &str, parent: &str) -> bool {
    let c = pure_posix_parts(child);
    let p = pure_posix_parts(parent);
    c.len() >= p.len() && c[..p.len()] == p[..]
}

fn join_posix(a: &str, b: &str) -> String {
    if a == "/" {
        format!("/{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// `datetime.fromisoformat(value.replace("Z", "+00:00"))` succeeds with a UTC offset of zero.
fn iso_utc_zero(value: &str) -> bool {
    let s: Vec<char> = value.replace('Z', "+00:00").chars().collect();
    let num = |a: usize, n: usize| -> Option<u32> {
        let t: String = s.get(a..a + n)?.iter().collect();
        if t.chars().all(|c| c.is_ascii_digit()) {
            t.parse().ok()
        } else {
            None
        }
    };
    // date: YYYY-MM-DD or YYYYMMDD
    let (y, mo, d, mut i) = if s.get(4) == Some(&'-') {
        match (num(0, 4), num(5, 2), num(8, 2)) {
            (Some(y), Some(m), Some(d)) if s.get(7) == Some(&'-') => (y, m, d, 10),
            _ => return false,
        }
    } else {
        match (num(0, 4), num(4, 2), num(6, 2)) {
            (Some(y), Some(m), Some(d)) => (y, m, d, 8),
            _ => return false,
        }
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if y < 1 || !(1..=12).contains(&mo) || d < 1 || d > mdays[mo as usize - 1] {
        return false;
    }
    if i >= s.len() {
        return false; // a date alone has no offset
    }
    i += 1; // any single separator character
    let rest: String = s[i..].iter().collect();
    // split the time from the offset at the first '+' or '-'
    let (time, tz) = match rest.find(['+', '-']) {
        Some(k) => (&rest[..k], Some(&rest[k..])),
        None => (&rest[..], None),
    };
    let tc: Vec<char> = time.chars().collect();
    let tnum = |a: usize| -> Option<u32> {
        let t: String = tc.get(a..a + 2)?.iter().collect();
        if t.chars().all(|c| c.is_ascii_digit()) { t.parse().ok() } else { None }
    };
    let ok_time = (|| {
        let h = tnum(0)?;
        let mut j = 2;
        let (mut mi, mut se) = (0, 0);
        let colon = tc.get(2) == Some(&':');
        if j < tc.len() {
            if colon {
                j += 1;
            }
            mi = tnum(j)?;
            j += 2;
            if j < tc.len() {
                if colon {
                    if tc.get(j) != Some(&':') {
                        return None;
                    }
                    j += 1;
                }
                se = tnum(j)?;
                j += 2;
                if j < tc.len() {
                    if tc[j] != '.' && tc[j] != ',' {
                        return None;
                    }
                    j += 1;
                    let f = &tc[j..];
                    if f.is_empty() || !f.iter().all(|c| c.is_ascii_digit()) {
                        return None;
                    }
                }
            }
        }
        (h < 24 && mi < 60 && se < 60).then_some(())
    })()
    .is_some();
    if !ok_time {
        return false;
    }
    let Some(tz) = tz else { return false };
    let z: Vec<char> = tz[1..].chars().collect();
    let zn = |a: usize| -> Option<u32> {
        let t: String = z.get(a..a + 2)?.iter().collect();
        if t.chars().all(|c| c.is_ascii_digit()) { t.parse().ok() } else { None }
    };
    let parsed = (|| {
        let hh = zn(0)?;
        let (mm, ss);
        if z.len() == 2 {
            mm = 0;
            ss = 0;
        } else if z.get(2) == Some(&':') {
            mm = zn(3)?;
            if z.len() == 5 {
                ss = 0;
            } else if z.get(5) == Some(&':') && z.len() >= 8 {
                ss = zn(6)?;
                if z.len() > 8 {
                    if z[8] != '.' || z.len() == 9 || !z[9..].iter().all(|c| c.is_ascii_digit()) {
                        return None;
                    }
                }
            } else {
                return None;
            }
        } else if z.len() == 4 {
            mm = zn(2)?;
            ss = 0;
        } else {
            return None;
        }
        (hh < 24 && mm < 60 && ss < 60).then_some(hh * 3600 + mm * 60 + ss)
    })();
    matches!(parsed, Some(0))
}

/// `validate_hint_write_scopes(ph, declaration)`.
pub fn validate_hint_write_scopes(ph: &Ph, declaration: Option<&PyValue>) -> Res<Scopes> {
    let Some(decl) = declaration else {
        return Ok(Scopes { status: "not-declared", write_paths: vec![] });
    };
    let require = |ok: bool, reason: &str| -> Res<()> {
        if ok {
            Ok(())
        } else {
            Err(sys(format!("[render] HINT_WRITE_SCOPES_INVALID: {reason}")))
        }
    };
    const KEYS: [&str; 12] = [
        "schema_version", "topology", "target", "observed_home_dir", "hint_clone_dir", "git_dir",
        "git_common_dir", "observation_source", "observed_utc", "approved_by", "approved_utc", "evidence",
    ];
    let d = match decl.peel() {
        PyValue::Dict(d) => Some(d),
        _ => None,
    };
    let keys_ok = d.map_or(false, |d| {
        d.len() == KEYS.len() && d.iter().all(|(k, _)| KEYS.iter().any(|x| k.equals(x)))
    });
    require(keys_ok, "explicit observation, target and existing approval fields are required")?;
    let d = d.unwrap();
    let g = |k: &str| d.get(k).cloned().unwrap_or(PyValue::None);
    let sv = g("schema_version");
    require(matches!(sv.peel(), PyValue::Int(n) if n.to_string() == "1"), "schema_version must be 1")?;
    let gs = |k: &str| match g(k).peel() {
        PyValue::Str(s) => Some(crate::py::text(s)),
        _ => None,
    };
    let phg = |k: &str| ph.get(k).cloned().unwrap_or_default();
    require(
        phg("TOPOLOGY") == "single" && phg("SUB_MODE") == "a2a-agent" && gs("topology").as_deref() == Some("single"),
        "external hint writes are single a2a-agent only",
    )?;
    let expected = [("host", phg("SUB_HOST")), ("ssh_user", phg("SSH_USER")), ("work_dir", phg("WORKSPACE_PATH"))];
    let target_ok = match g("target").peel() {
        PyValue::Dict(t) => {
            t.len() == 3
                && expected.iter().all(|(k, v)| matches!(t.get(k).map(PyValue::peel), Some(PyValue::Str(s)) if s.equals(v)))
        }
        _ => false,
    };
    require(target_ok, "target must match this exact manifest sub endpoint/workspace")?;
    for key in ["observation_source", "approved_by", "evidence"] {
        let ok = match gs(key) {
            Some(v) => {
                v == pystrip(&v)
                    && !v.is_empty()
                    && !v.chars().any(|c| (c as u32) < 32)
                    && !["unknown", "pending", "tbd", "none"].contains(&crate::py::text(&PyStr::new(&v)).to_lowercase().as_str())
                    && !v.chars().any(|c| c == '<' || c == '>')
            }
            None => false,
        };
        require(ok, &format!("{key} must cite an actual source/decision"))?;
    }
    for key in ["observed_utc", "approved_utc"] {
        let ok = match gs(key) {
            Some(v) => v.ends_with('Z') && iso_utc_zero(&v),
            None => false,
        };
        require(ok, &format!("{key} must be a UTC timestamp"))?;
    }
    let path = |value: Option<String>, name: &str| -> Res<String> {
        let ok = match &value {
            Some(v) => {
                v.starts_with('/')
                    && !v.starts_with("//")
                    && crate::pathfmt::py_path(v) == *v
                    && !v.chars().any(|c| "\\*?[]{}".contains(c))
                    && !v.chars().any(|c| (c as u32) < 32 || c as u32 == 127)
                    && !pure_posix_parts(v).iter().any(|p| p == "..")
            }
            None => false,
        };
        require(ok, &format!("{name} must be a canonical literal absolute POSIX path (no globs/escapes/traversal)"))?;
        Ok(value.unwrap())
    };
    let workspace = path(Some(phg("WORKSPACE_PATH")), "target.work_dir")?;
    let home = path(gs("observed_home_dir"), "observed_home_dir")?;
    require(!["/", "/home", "/srv", "/tmp", "/var"].contains(&home.as_str()), "home must be the reported account home")?;
    let roots = ["/", "/home", "/root", "/Users", "/tmp", "/var", "/var/lib", "/var/cache", "/srv", "/mnt", "/media", "/opt"];
    let mut protected = vec![workspace];
    for p in [".ssh", ".aws", ".codex", ".claude", ".config"] {
        protected.push(join_posix(&home, p));
    }
    let nas = phg("NAS_MOUNT");
    if !nas.is_empty() {
        protected.push(path(Some(nas), "NAS_MOUNT")?);
    }
    let mut writes: Vec<String> = vec![];
    for key in ["hint_clone_dir", "git_dir", "git_common_dir"] {
        let p = path(gs(key), key)?;
        let parts = pure_posix_parts(&p);
        let account_home = parts.len() == 3 && (parts[1] == "home" || parts[1] == "Users");
        require(
            parts.len() >= 3 && !roots.contains(&p.as_str()) && !account_home && !inside(&home, &p),
            &format!("{key} cannot grant a root or whole account home"),
        )?;
        require(
            !["/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64", "/proc", "/sys", "/dev", "/boot", "/run"]
                .iter()
                .any(|pre| inside(&p, pre)),
            &format!("{key} cannot grant system policy/runtime trees"),
        )?;
        require(
            !protected.iter().any(|q| inside(&p, q) || inside(q, &p)),
            &format!("{key} overlaps workspace, NAS or private configuration"),
        )?;
        writes.push(p);
    }
    let (clone, git_dir, common) = (&writes[0], &writes[1], &writes[2]);
    require(
        git_dir != clone && common != clone && inside(git_dir, common),
        "git_dir must be the actual metadata directory at or beneath git_common_dir, distinct from clone",
    )?;
    let mut uniq: Vec<String> = vec![];
    for w in &writes {
        if !uniq.contains(w) {
            uniq.push(w.clone());
        }
    }
    Ok(Scopes { status: "declared", write_paths: uniq })
}

/// `load_hint_write_scopes(path)`.
pub fn load_hint_write_scopes(path: Option<&str>) -> Res<Option<PyValue>> {
    let Some(path) = path else { return Ok(None) };
    let fail = |cls: &str| sys(format!("[render] HINT_WRITE_SCOPES_INVALID: cannot read declaration ({cls})"));
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            let cls = match e.raw_os_error() {
                Some(2) => "FileNotFoundError",
                Some(13) => "PermissionError",
                Some(21) => "IsADirectoryError",
                Some(20) => "NotADirectoryError",
                _ => "OSError",
            };
            return Err(fail(cls));
        }
    };
    let Ok(text) = String::from_utf8(bytes) else { return Err(fail("UnicodeDecodeError")) };
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    match pyjson::loads_text_unique(&PyStr::new(&text)) {
        Ok(v) => {
            if matches!(v.peel(), PyValue::Dict(_)) {
                Ok(Some(v))
            } else {
                Err(fail("ValueError"))
            }
        }
        Err(pyjson::UniqueError::DuplicateKey(_)) => Err(fail("ValueError")),
        Err(pyjson::UniqueError::Load(e)) => Err(fail(match e {
            pyjson::LoadError::Decode { .. } => "JSONDecodeError",
            _ if e.is_value_error() => "ValueError",
            _ => "RecursionError",
        })),
    }
}

// ── topology rules ────────────────────────────────────────────────────────────────────────────────────────────────────

/// `topology_rules_mismatch(topology, rules_path)`.
pub fn topology_rules_mismatch(ctx: &Ctx, topology: &str, rules_path: Option<&str>) -> Option<String> {
    let path = rules_path.map(String::from).unwrap_or_else(|| ctx.topology_rules());
    if !isfile(&path) {
        return Some(format!("특화헌법이 없다({path}) — 서브 '{topology}' 브랜치에 배달할 자기선언이 없다"));
    }
    let (declared, why) = role_contract::topology::read_layer_header(path.as_bytes());
    match declared {
        None => Some(format!("특화헌법 자기선언을 읽지 못했다({path}): {}", crate::py::text(&why))),
        Some(d) if d != topology => Some(format!(
            "특화헌법({path})은 topology={d} 를 선언하는데 렌더 통로는 --topology {topology} 다 — \
             이 트리의 특화층을 서브 '{topology}' 브랜치로 보내면 반대 토폴로지 헌법이 배달된다"
        )),
        Some(_) => None,
    }
}

// ── the copy of tracked runtime blocks ─────────────────────────────────────────────────────────────────────────────────

/// `_copy_tracked(src, dst, tracked_list, excludes, only)` with `src = REPO/<rel>`; returns (count, source).
pub fn copy_tracked(
    repo: &str,
    rel: &str,
    dst: &str,
    tracked: Option<&[String]>,
    excludes: Option<&[&str]>,
    only: Option<&[&str]>,
) -> Res<(usize, &'static str)> {
    let rel = crate::pathfmt::py_path(rel);
    let src_skill = pj(repo, &rel);
    let (files, source): (Vec<String>, &'static str) = if let Some(list) = tracked {
        let prefix = format!("{}/", rel.trim_end_matches('/'));
        (list.iter().filter(|f| **f == rel || f.starts_with(&prefix)).cloned().collect(), "injected:tracked-list")
    } else {
        let out = match git(repo, &["ls-files", "-z", "--", &rel]) {
            Ok(o) => o,
            Err(r) => {
                let (cls, msg) = r.0.split_once(": ").unwrap_or(("OSError", &r.0));
                let cls = cls.rsplit('.').next().unwrap_or(cls);
                return Err(sys(format!(
                    "[render] FAIL: git ls-files 실행 불가({cls}: {msg}) — tracked 목록 없이 런타임블럭을 복제하지 않는다({rel})."
                )));
            }
        };
        if out.returncode != 0 {
            let err: String = pystrip(&out.stderr).chars().take(300).collect();
            return Err(sys(format!(
                "[render] FAIL: git ls-files rc={} — tracked 목록 없이 복제하지 않는다({rel}).\n          stderr: {err}\n          (git 없는 트리에서 렌더한다면 호출부가 tracked_list 를 주입해야 한다.)",
                out.returncode
            )));
        }
        (
            out.stdout.split('\0').filter(|f| !pystrip(f).is_empty()).map(String::from).collect(),
            "measured:git-ls-files",
        )
    };
    if files.is_empty() {
        return Err(sys(format!("[render] FAIL: 런타임블럭 {rel} 의 tracked 파일이 0개 — 배달할 것이 없다(계약 위반).")));
    }
    let mut cnt = 0;
    for rf in &files {
        let srcf = pj(repo, rf);
        if !isfile(&srcf) || is_retired_py(rf) {
            continue;
        }
        let sub_rel = relpath(&srcf, &src_skill);
        if excludes.map_or(false, |ex| ex.contains(&sub_rel.as_str())) {
            continue;
        }
        if let Some(only) = only {
            if !only.iter().any(|pre| sub_rel.starts_with(pre)) {
                continue;
            }
        }
        let destf = pj(dst, &sub_rel);
        makedirs(&dirname(&destf))?;
        copy2(&srcf, &destf)?;
        cnt += 1;
    }
    Ok((cnt, source))
}

/// `Path(p).resolve()` (non-strict): the longest existing prefix canonicalized, the rest normalized lexically.
pub fn resolve_path(p: &str) -> String {
    let abs = if p.starts_with('/') {
        p.to_string()
    } else {
        pj(&std::env::current_dir().unwrap_or_default().display().to_string(), p)
    };
    let mut out = PathBuf::from("/");
    let parts: Vec<&str> = abs.split('/').filter(|x| !x.is_empty() && *x != ".").collect();
    let mut real = true;
    for part in parts {
        if part == ".." {
            out.pop();
            continue;
        }
        out.push(part);
        if real {
            match std::fs::canonicalize(&out) {
                Ok(c) => out = c,
                Err(_) => real = false,
            }
        }
    }
    out.display().to_string()
}

/// `_assert_rust_path_closure(out_dir, names)`.
pub fn assert_rust_path_closure(out_dir: &str) -> Res<()> {
    let root = resolve_path(out_dir);
    let mut pending: Vec<String> = RUST_OWNER_MANIFESTS.iter().map(|(_, m)| pj(&root, m)).collect();
    // every delivered crate is a root: each is built on the Sub, so each one's own path dependencies must be delivered too
    for lib in shared_rust_libraries() {
        pending.push(pj(&root, &format!("{lib}/Cargo.toml")));
    }
    let mut seen: Vec<String> = vec![];
    while let Some(m) = pending.pop() {
        let manifest = resolve_path(&m);
        let rel_ok = manifest == root || manifest.starts_with(&format!("{}/", root.trim_end_matches('/')));
        if !rel_ok || !isfile(&manifest) {
            return Err(sys(format!("[render] FAIL: Rust path dependency missing/outside delivery: {manifest}")));
        }
        if seen.contains(&manifest) {
            continue;
        }
        seen.push(manifest.clone());
        let text = String::from_utf8_lossy(&read_bytes(&manifest)?).into_owned();
        let data = toml::loads(&text).map_err(|e| Fail::Raise(Raised::new("tomllib.TOMLDecodeError", &e)))?;
        let mut tables = vec![data.clone()];
        if let Some(toml::Toml::Table(t)) = data.get("target") {
            tables.extend(t.iter().map(|(_, v)| v.clone()));
        }
        let parent = dirname(&manifest);
        for table in &tables {
            for kind in ["dependencies", "build-dependencies", "dev-dependencies"] {
                if let Some(toml::Toml::Table(deps)) = table.get(kind) {
                    for (_, dep) in deps {
                        if let Some(toml::Toml::Str(p)) = dep.get("path") {
                            pending.push(pj(&pj(&parent, p), "Cargo.toml"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// ── render_tree ─────────────────────────────────────────────────────────────────────────────────────────────────────────

pub struct RenderOut {
    pub out_dir: String,
    pub produced: Vec<String>,
    pub scopes: Scopes,
}

/// `json.load(open(path))` (errors as the traceback's last line).
pub fn load_json(path: &str) -> Res<PyValue> {
    crate::agent_card::load_card(path).map_err(Fail::Raise)
}

fn json_dumps_pretty(v: &PyValue) -> String {
    crate::py::text(&v.dumps_pretty().expect("JSON-safe"))
}

fn load_failure(e: role_contract::load::LoadFailure) -> Fail {
    use role_contract::load::LoadFailure as L;
    Fail::Raise(match e {
        L::Os { error, filename } => Raised::os(&error, &crate::py::text(&filename)),
        L::Yaml(y) => Raised::yaml(&y),
        L::EmbeddedNull => Raised::new("ValueError", "embedded null byte"),
        L::NotPathLike(t) => Raised::new("TypeError", &format!("expected str, bytes or os.PathLike object, not {t}")),
        L::Unsupported { boundary, detail } => Raised::new("RuntimeError", &format!("unsupported {boundary}: {detail}")),
    })
}

/// `render_tree(ph, out_dir, copy_runtime_block, tracked_list, sub_manifest_path, hint_write_scopes)`.
pub fn render_tree(
    ctx: &Ctx,
    ph: &Ph,
    out_dir: &str,
    copy_runtime_block: bool,
    tracked: Option<&[String]>,
    sub_manifest_path: Option<&str>,
    hint_write_scopes: Option<&PyValue>,
) -> Res<RenderOut> {
    let scopes = validate_hint_write_scopes(ph, hint_write_scopes)?;
    if isdir(out_dir) {
        rmtree(out_dir)?;
    }
    let claude = pj(out_dir, ".claude");
    makedirs(&pj(&claude, "rules"))?;
    makedirs(&pj(&claude, "schemas"))?;
    let mut produced: Vec<String> = vec![];
    let sub_mode = ph.get("SUB_MODE").cloned().unwrap_or_default();
    let subnode = ctx.subnode_dir();

    let render_file = |tmpl: &str, dest_rel: &str, kind: &str, produced: &mut Vec<String>| -> Res<String> {
        let mut raw = read_text(&pj(&subnode, tmpl))?;
        if kind == "md" || kind == "toml" {
            raw = textre::strip_template_header(&raw);
            raw = apply_mode_blocks(&raw, &sub_mode)?;
        }
        let mut txt = render_text(&raw, ph);
        let left = unrendered(&txt);
        if !left.is_empty() {
            return Err(sys(format!("[render] FAIL: 미치환 placeholder {} in {tmpl}", list_repr(&left))));
        }
        if kind == "json" {
            let obj = pyjson::loads(&txt).map_err(|e| Fail::Raise(Raised::new("json.decoder.JSONDecodeError", &e.to_string())))?;
            let obj = apply_mode_prefixes(&obj, &sub_mode)?;
            let obj = match obj {
                PyValue::Dict(d) => {
                    let mut nd = PyDict::new();
                    for (k, v) in d.iter() {
                        if !crate::py::text(k).starts_with('_') {
                            nd.insert(k.clone(), v.clone());
                        }
                    }
                    PyValue::Dict(nd)
                }
                other => other,
            };
            txt = json_dumps_pretty(&obj) + "\n";
        }
        if kind == "toml" {
            toml::loads(&txt).map_err(|e| Fail::Raise(Raised::new("tomllib.TOMLDecodeError", &e)))?;
        }
        let dest = pj(out_dir, dest_rel);
        makedirs(&dirname(&dest))?;
        write_text(&dest, &txt)?;
        produced.push(dest_rel.to_string());
        Ok(dest)
    };

    // 1) capability metadata and local policy
    render_file("CLAUDE.template.md", "CLAUDE.md", "md", &mut produced)?;
    render_file("AGENTS.template.md", "AGENTS.md", "md", &mut produced)?;
    let card_path = render_file("Agent_Card.template.json", "Agent_Card.json", "json", &mut produced)?;
    render_file("settings.local.template.json", ".claude/settings.local.json", "json", &mut produced)?;
    render_file("codex.config.template.toml", ".codex/config.toml", "toml", &mut produced)?;
    let codex_rules = pj(out_dir, ".codex/rules/sub.rules");
    makedirs(&dirname(&codex_rules))?;
    copyfile(&pj(&subnode, "codex.sub.rules"), &codex_rules)?;
    produced.push(".codex/rules/sub.rules".into());

    // 1.5) Agent Card structural validation
    let card = load_json(&card_path)?;
    let mut violations = crate::agent_card::validate_card(&card)?;
    violations.extend(crate::agent_card::require_skills(&card)?);
    if !violations.is_empty() {
        return Err(sys(format!(
            "[render] FAIL: Agent Card capability contract violation — {}",
            violations.join("; ")
        )));
    }

    // 1.6) sub manifest → output/<topology>/manifest.yaml
    if let Some(smp) = sub_manifest_path {
        // `_load_manifest(path)`: `yaml.safe_load(open(path, "r", encoding="utf-8")) or {}`
        let sub_man = role_contract::load::load_yaml_objects(&PyValue::str(smp)).map_err(load_failure)?;
        let sub_man = if sub_man.is_truthy() { sub_man } else { PyValue::Dict(PyDict::new()) };
        let get = |k: &str| match sub_man.peel() {
            PyValue::Dict(d) => Ok(d.get(k).cloned().unwrap_or(PyValue::None)),
            other => Err(Fail::Raise(Raised::new(
                "AttributeError",
                &format!("'{}' object has no attribute 'get'", other.type_name()),
            ))),
        };
        let self_role = get("self_role")?;
        if !matches!(self_role.peel(), PyValue::Str(s) if s.equals("sub")) {
            return Err(sys(format!(
                "[render] FAIL: 서브 manifest 의 self_role 이 'sub' 가 아니다({}) — {smp}",
                self_role.repr()
            )));
        }
        let t = get("topology")?;
        let topo = if t.is_truthy() {
            t
        } else {
            PyValue::str(ph.get("TOPOLOGY").map(String::as_str).unwrap_or(""))
        };
        let res = role_contract::manifest_gate::evaluate_contract_value(&sub_man, &topo).map_err(|e| {
            Fail::Raise(match e {
                role_contract::manifest_gate::GateFailure::Raised(r) => {
                    Raised::new(r.class, &crate::py::text(&r.message))
                }
                role_contract::manifest_gate::GateFailure::Unsupported { boundary, detail } => {
                    Raised::new("RuntimeError", &format!("unsupported {boundary}: {detail}"))
                }
            })
        })?;
        if !res.flag() {
            let reason = match res.res.get("reason") {
                Some(v) => crate::py::text(&v.display_text()),
                None => "None".into(),
            };
            return Err(sys(format!("[render] FAIL: 서브 manifest 가 테라포밍 계약을 통과하지 못한다 — {reason} ({smp})")));
        }
        let topo_text = match topo.peel() {
            PyValue::Str(s) => crate::py::text(s),
            other => {
                return Err(Fail::Raise(Raised::new(
                    "TypeError",
                    &format!("join() argument must be str, bytes, or os.PathLike object, not '{}'", other.type_name()),
                )))
            }
        };
        let dest_rel = pj(&pj("output", &topo_text), "manifest.yaml");
        let dest = pj(out_dir, &dest_rel);
        makedirs(&dirname(&dest))?;
        copyfile(smp, &dest)?;
        produced.push(format!("{dest_rel} (서브 manifest · terraforming 실측 · Flag issued_by=main)"));
    }

    // 2) static contracts (comms · schema)
    let comms = apply_mode_blocks(&read_text(&pj(&subnode, "comms.md"))?, &sub_mode)?;
    write_text(&pj(&claude, "rules/comms.md"), &comms)?;
    produced.push(".claude/rules/comms.md".into());
    for schema_name in ["task-report.schema.json"] {
        let schema_dst = pj(&claude, &format!("schemas/{schema_name}"));
        copyfile(&pj(&subnode, schema_name), &schema_dst)?;
        load_json(&schema_dst)?;
        produced.push(format!(".claude/schemas/{schema_name}"));
    }
    // topology constitution layer
    let rules = ctx.topology_rules();
    if isfile(&rules) {
        copyfile(&rules, &pj(&claude, "rules/strategy.topology.md"))?;
        produced.push(".claude/rules/strategy.topology.md".into());
        if read_text(&rules)?.contains("handoff_exception.topology.md") {
            let target = pj(&claude, "policies/handoff_exception.topology.md");
            makedirs(&dirname(&target))?;
            copyfile(&ctx.handoff_reference(), &target)?;
            produced.push(".claude/policies/handoff_exception.topology.md".into());
        }
    }

    // 3) runtime blocks (tracked only) — what to give is the contract's tool_plane
    if copy_runtime_block {
        let wanted = ph.get("TOOL_PLANE").cloned().unwrap_or_default();
        let names: Vec<String> = wanted.split(',').map(pystrip).filter(|x| !x.is_empty()).map(String::from).collect();
        for name in &names {
            let Some(_rb) = ctx.runtime_block_path(name) else {
                return Err(sys(format!("[render] FAIL: tool_plane 이 지목한 런타임블럭 경로를 모른다: {name}")));
            };
            let dst_skill = pj(&claude, &format!("skills/{name}"));
            let ex = runtime_block_excludes(name);
            let (n, _) = copy_tracked(&ctx.repo, &format!(".claude/skills/{name}"), &dst_skill, tracked, ex, None)?;
            produced.push(format!(
                ".claude/skills/{name}/ ({n} tracked files{}",
                match ex {
                    Some(ex) => format!(" · 메인전용 {}건 제외)", ex.len()),
                    None => ")".into(),
                }
            ));
            let wrapper_dst = pj(out_dir, &format!(".agents/skills/{name}"));
            copy_tracked(&ctx.repo, &format!(".agents/skills/{name}"), &wrapper_dst, tracked, None, None)?;
            let wrapper_path = pj(&wrapper_dst, "SKILL.md");
            let canonical_path = pj(&dst_skill, "SKILL.md");
            if !isfile(&wrapper_path) || !isfile(&canonical_path) {
                return Err(sys(format!("[render] FAIL: skill discovery closure missing for {name}")));
            }
            let expected_link = format!("../../../.claude/skills/{name}/SKILL.md");
            if !read_text(&wrapper_path)?.contains(&expected_link) {
                return Err(sys(format!("[render] FAIL: wrapper does not link to canonical skill: {name}")));
            }
            produced.push(format!(".agents/skills/{name}/SKILL.md"));
            if name == "vllm-recipe-explorer" {
                let feedback_rel = ".claude/skills/vllm-recipe-explorer/feedback/.gitkeep";
                let marker = pj(out_dir, feedback_rel);
                makedirs(&dirname(&marker))?;
                write_text(&marker, "Node-local generated recipe feedback; ignored by git.\n")?;
                produced.push(feedback_rel.into());
            }
        }
        for (name, _) in RUST_OWNER_MANIFESTS {
            if names.iter().any(|n| n == name) {
                continue;
            }
            let mut only: Vec<&str> = RUST_ONLY_PREFIXES.to_vec();
            only.extend(rust_only_inputs(name));
            let (n, _) = copy_tracked(
                &ctx.repo,
                &format!(".claude/skills/{name}"),
                &pj(&claude, &format!("skills/{name}")),
                tracked,
                runtime_block_excludes(name),
                Some(&only),
            )?;
            produced.push(format!(".claude/skills/{name}/ (Rust 전용 · {n} tracked files · 스킬 아님)"));
        }
        for support in shared_rust_libraries() {
            let (count, _) = copy_tracked(&ctx.repo, support, &pj(out_dir, support), tracked, None, None)?;
            produced.push(format!("{support}/ ({count} tracked files)"));
        }
        assert_rust_path_closure(out_dir)?;
        for rel in SUB_RUST_BUILD_INPUTS {
            let src = pj(&ctx.repo, rel);
            if !isfile(&src) {
                return Err(sys(format!("[render] FAIL: Rust 빌드 입력이 없다 — {rel}")));
            }
            tracked_or_die(ctx, rel, tracked)?;
            let dst = pj(out_dir, rel);
            makedirs(&dirname(&dst))?;
            copy2(&src, &dst)?;
            produced.push(rel.into());
        }
        for rel in SUB_BENCH_FOUNDATION_RUNTIME {
            let src = pj(&ctx.repo, rel);
            if !isfile(&src) {
                return Err(sys(format!("[render] FAIL: 벤치 owner 자체검사 의존이 없다 — {rel}")));
            }
            tracked_or_die(ctx, rel, tracked)?;
            let dst = pj(out_dir, rel);
            makedirs(&dirname(&dst))?;
            copy2(&src, &dst)?;
            produced.push(rel.into());
        }
        for rel in SUB_B_CHECK_INPUTS {
            let src = pj(&ctx.repo, rel);
            if !isfile(&src) {
                return Err(sys(format!("[render] FAIL: B 자체검사 입력이 없다 — {rel}")));
            }
            tracked_or_die(ctx, rel, tracked)?;
            let dst = pj(out_dir, rel);
            makedirs(&dirname(&dst))?;
            copy2(&src, &dst)?;
            produced.push(rel.into());
        }
        let hint = names.iter().any(|n| n == "hint-publisher");
        for rel in HINT_ENGINE_DEPENDENCIES {
            if rel.starts_with("hints/") && !hint {
                continue;
            }
            let src = pj(&ctx.repo, rel);
            if !isfile(&src) {
                return Err(sys(format!(
                    "[render] FAIL: hint 엔진 의존이 없다 — {rel}\n   스킬을 주고 그 의존을 안 주면 그 스킬은 없는 것이다(2026-09-29 계약 리더 선례).\n   트랜잭션 소스에서 도는 중이면 sync_to_sub 의 checkout-index 경로 목록에 `hints` 가 있는지 본다."
                )));
            }
            tracked_or_die(ctx, rel, tracked)?;
            let dst = pj(out_dir, rel);
            makedirs(&dirname(&dst))?;
            copy2(&src, &dst)?;
            produced.push(rel.into());
        }
        if hint {
            for tool in SUB_HINT_EQUIP_TOOLS {
                let src = pj(&ctx.here, tool);
                if !isfile(&src) {
                    return Err(sys(format!("[render] FAIL: hint 장착 판정 도구가 없다 — {src}")));
                }
                let dst = pj(&claude, &format!("skills/terraforming_node/scripts/{tool}"));
                makedirs(&dirname(&dst))?;
                copy2(&src, &dst)?;
                produced.push(tn_script(tool));
            }
        }
        let leaked: Vec<&str> = HINT_NEVER_DELIVER.iter().copied().filter(|r| exists(&pj(out_dir, r))).collect();
        if !leaked.is_empty() {
            return Err(sys(format!(
                "[render] FAIL: 서브로 가면 안 되는 hint 평면 경로가 스테이징에 있다: {}\n   색인·카탈로그는 메인 단독 파생물이고 역할 마커는 각 체크아웃의 자기 선언이다.",
                list_repr(&leaked)
            )));
        }
        if names.is_empty() {
            let src = ph.get("TOOL_PLANE_SOURCE").cloned().unwrap_or_default();
            produced.push(format!(
                ".claude/skills/ (런타임블럭 0종 — tool_plane={})",
                if src.is_empty() { "unknown".to_string() } else { src }
            ));
        }
    }

    // 4) campaign workspace skeleton
    let tpl_src = pj(&ctx.repo, "campaigns/_template");
    if !isdir(&tpl_src) {
        return Err(sys(format!(
            "[render] FAIL: 캠페인 뼈대가 없다 — {tpl_src}\n   서브도 메인과 같은 뼈대를 받아야 자기 campaigns/<id>/ 를 저작할 수 있다.\n   트랜잭션 소스에서 도는 중이라면 sync_to_sub 의 checkout-index 경로 목록에\n   `campaigns` 가 들어 있는지 확인하라(부재는 침묵 누락이 된다)."
        )));
    }
    for (root, _dirs, files) in walk(&tpl_src) {
        for f in files {
            let abs = pj(&root, &f);
            let rel = relpath(&abs, &ctx.repo);
            let dst = pj(out_dir, &rel);
            makedirs(&dirname(&dst))?;
            copy2(&abs, &dst)?;
            produced.push(rel);
        }
    }
    // 4.1) campaign state writer/validator, contract readers, permission probe, Rust bootstrap
    let ci_src = pj(&ctx.repo, ".claude/skills/terraforming_node/scripts");
    let ci_dst = pj(&claude, "skills/terraforming_node/scripts");
    makedirs(&ci_dst)?;
    for tool in delivered_tn_tools() {
        let abs = pj(&ci_src, tool);
        if !isfile(&abs) {
            return Err(sys(format!(
                "[render] FAIL: 캠페인 도구가 없다 — {abs}\n   뼈대만 보내고 채우는 손을 안 보내면 phases/sub가 실행자 0이 된다."
            )));
        }
        copy2(&abs, &pj(&ci_dst, tool))?;
        produced.push(tn_script(tool));
    }
    let rust_ref = pj(&ctx.repo, RUST_RUNTIME_REFERENCE);
    tracked_or_die(ctx, RUST_RUNTIME_REFERENCE, tracked)?;
    let rust_ref_dst = pj(out_dir, RUST_RUNTIME_REFERENCE);
    makedirs(&dirname(&rust_ref_dst))?;
    copy2(&rust_ref, &rust_ref_dst)?;
    produced.push(RUST_RUNTIME_REFERENCE.into());

    let readme_src = pj(&ctx.repo, "campaigns/README.md");
    if !isfile(&readme_src) {
        return Err(sys(format!(
            "[render] FAIL: 캠페인 읽기 규약이 없다 — {readme_src}\n   뼈대만 보내고 읽는 법을 안 보내면 서브는 빈칸의 **의미**를 모른 채 채운다."
        )));
    }
    let readme_dst = pj(out_dir, "campaigns/README.md");
    makedirs(&dirname(&readme_dst))?;
    copy2(&readme_src, &readme_dst)?;
    produced.push("campaigns/README.md".into());

    // 4.5) host safety
    let hs_src = pj(&ctx.repo, ".claude/skills/terraforming_node/scripts/host_safety");
    let hs_files: [(&str, u32); 5] = [
        ("mem_watchdog.sh", 0o755),
        ("install_host_safety.sh", 0o755),
        ("install_netconsole.sh", 0o755),
        ("systemd/easy-vllm-memwatch.service", 0o644),
        ("host/vllm-drop-caches.sh", 0o755),
    ];
    for (rel_src, mode) in hs_files {
        let src = pj(&hs_src, rel_src);
        if !isfile(&src) {
            return Err(sys(format!(
                "[render_sub_env] HOST_SAFETY_SOURCE_MISSING: canonical terraforming asset absent: {rel_src}"
            )));
        }
        let rel = format!(".claude/runtime/host_safety/{rel_src}");
        let dst = pj(out_dir, &rel);
        makedirs(&dirname(&dst))?;
        copyfile(&src, &dst)?;
        chmod(&dst, mode)?;
        produced.push(rel);
    }
    // 4.6) node blackbox
    let bb_src = pj(&ctx.repo, ".claude/skills/terraforming_node/scripts/node_blackbox");
    for (rel_src, mode) in BLACKBOX_FILES {
        let src = pj(&bb_src, rel_src);
        if !isfile(&src) {
            return Err(sys(format!(
                "[render_sub_env] NODE_BLACKBOX_SOURCE_MISSING: canonical terraforming asset absent: {rel_src}"
            )));
        }
        let rel = format!(".claude/runtime/node_blackbox/{rel_src}");
        let dst = pj(out_dir, &rel);
        makedirs(&dirname(&dst))?;
        copyfile(&src, &dst)?;
        chmod(&dst, mode)?;
        produced.push(rel);
    }
    // 5) sub-local .gitignore
    let gi_src = pj(&subnode, "gitignore.template");
    if isfile(&gi_src) {
        copyfile(&gi_src, &pj(out_dir, ".gitignore"))?;
        produced.push(".gitignore".into());
    }
    // 6) docs skeletons
    for dtype in DOC_TYPES {
        let ex = pj(&ctx.doc_skeletons(), &format!("{dtype}/example.md"));
        if !isfile(&ex) {
            return Err(sys(format!("[render] FAIL: docs skeleton missing: {ex}")));
        }
        let dst = pj(out_dir, &format!("docs/{dtype}/example.md"));
        makedirs(&dirname(&dst))?;
        copyfile(&ex, &dst)?;
    }
    produced.push(format!("docs/*/example.md ({} skeletons)", DOC_TYPES.len()));
    Ok(RenderOut { out_dir: out_dir.to_string(), produced, scopes })
}

/// The node blackbox delivered to a Sub under `.claude/runtime/node_blackbox/`: shell tools and the Rust launchers (plan_26111615
/// pass 3 — the launchers run the ELFs of the terraforming crate built on the Sub; no `.py` tool and no Python on the Sub).
pub const BLACKBOX_FILES: [(&str, u32); 17] = [
    ("mem_watchdog_eta.sh", 0o755),
    ("blackbox_collect.sh", 0o755),
    ("blackbox_events.sh", 0o755),
    ("blackbox_eta.sh", 0o755),
    ("blackbox_session.sh", 0o755),
    ("logs_lifecycle.sh", 0o755),
    ("seed_from_journal.sh", 0o755),
    ("install_node_blackbox.sh", 0o755),
    ("verify_node_blackbox.sh", 0o755),
    ("purge_host_safety.sh", 0o755),
    ("agent_guard.sh", 0o755),
    ("adversarial_stress.sh", 0o755),
    ("regen_envelope.sh", 0o755),
    ("node_identity.sh", 0o755),
    ("budget_renew_loop.sh", 0o755),
    ("blackbox_thermal.sh", 0o755),
    ("thermal_watchdog.sh", 0o755),
];

// ── main ────────────────────────────────────────────────────────────────────────────────────────────────────────────────

pub static SPEC: Spec = Spec {
    prog: "render_sub_env.py",
    usage: include_str!("../help/render_sub_env.usage"),
    help: include_str!("../help/render_sub_env.help"),
    args: &[
        HELP,
        store(&["--topology"]).choices(&["single", "multi"]),
        store(&["--manifest"]),
        store(&["--out"]),
        flag(&["--no-runtime-block"]),
        store(&["--tracked-list"]),
        store(&["--sub-manifest"]),
        store(&["--hint-write-scopes"]),
        flag(&["--self-test"]),
    ],
    subs: None,
};

pub fn main(argv: &[String]) -> i32 {
    let a = match argp::parse(&SPEC, argv) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    let ctx = Ctx::from_owner();
    if a.flag(8) {
        return selftest::self_test(&ctx);
    }
    match run(&ctx, &a) {
        Ok(c) => c,
        Err(f) => f.finish(),
    }
}

fn run(ctx: &Ctx, a: &argp::Parsed) -> Res<i32> {
    let topology = a.text(1).unwrap_or("multi").to_string();
    let mut mismatch = topology_rules_mismatch(ctx, &topology, None);
    if mismatch.is_none() && read_text(&ctx.topology_rules())?.contains("handoff_exception.topology.md") {
        mismatch = topology_rules_mismatch(ctx, &topology, Some(&ctx.handoff_reference()));
    }
    if let Some(m) = mismatch {
        eprintln!(
            "[render] STOP(TOPOLOGY_RULES_MISMATCH): {m}\n        해소: '{topology}' 토폴로지 브랜치를 체크아웃한 트리에서 --topology {topology} 로 렌더하라(자동 교정·우회 인자 없음 · policy:BRANCH_CONSTITUTION_LAYERING)"
        );
        return Ok(5);
    }
    let manifest = a.text(2).map(String::from).unwrap_or_else(|| pj(&ctx.repo, &format!("output/{topology}/manifest.yaml")));
    let out_dir = a.text(3).map(String::from).unwrap_or_else(|| pj(&ctx.repo, &format!("output/{topology}/sub_provision")));
    if !isfile(&manifest) {
        eprintln!("[render] FAIL: manifest 없음 — {manifest} (terraforming_node 스캔/인터뷰로 먼저 채우세요)");
        return Ok(3);
    }
    let mut sub_manifest =
        Some(a.text(6).map(String::from).unwrap_or_else(|| pj(&ctx.repo, &format!("output/{topology}/sub_manifest.yaml"))));
    if !isfile(sub_manifest.as_deref().unwrap()) {
        if topology == "single" {
            let sm = sub_manifest.unwrap();
            eprintln!(
                "[render] FAIL: 서브 manifest 파일이 아직 생성되지 않았다 — {sm}\n        single 의 서브는 자기 manifest 를 갖는다(terraforming 실측·생성): bash .claude/skills/terraforming_node/scripts/scan_node.sh --topology single --peer-ssh <user@sub> --model-source <managed|ephemeral|custom> --emit-sub-manifest {sm}"
            );
            return Ok(2);
        }
        sub_manifest = None;
    }
    let mut data = parse_manifest(&manifest)?;
    if data.get("topology").is_none() {
        data.set("topology", MVal::Str(topology.clone()));
    }
    let sub_parsed = match &sub_manifest {
        Some(p) => Some(parse_manifest(p)?),
        None => None,
    };
    let (ph, missing) = build_placeholders(ctx, &data, sub_parsed.as_ref())?;
    if !missing.is_empty() {
        eprintln!("[render] FAIL: manifest 필수 필드 누락 {} — 무증거 빈 정체성 렌더 금지.", list_repr(&missing));
        let v = ph.get("SUB_CONTRACT_VIOLATIONS").cloned().unwrap_or_default();
        if !v.is_empty() {
            eprintln!(
                "[render]       ↳ 토폴로지 축 계약 위반: {v} (bash .claude/skills/terraforming_node/scripts/node_role_contract.sh evaluate --manifest {manifest} 로 상세 확인)"
            );
        }
        return Ok(2);
    }
    let tracked: Option<Vec<String>> = match a.text(5) {
        Some(p) => {
            let blob = read_text(p)?;
            let items: Vec<String> = if blob.contains('\0') {
                blob.split('\0').map(String::from).collect()
            } else {
                crate::py::splitlines(&blob)
            };
            Some(items.into_iter().filter(|x| !pystrip(x).is_empty()).collect())
        }
        None => None,
    };
    let scopes = load_hint_write_scopes(a.text(7))?;
    let res = render_tree(
        ctx,
        &ph,
        &out_dir,
        !a.flag(4),
        tracked.as_deref(),
        sub_manifest.as_deref(),
        scopes.as_ref(),
    )?;
    println!("[render] OK → {}", res.out_dir);
    println!("[render] external hint write scopes: {} (native access unverified)", res.scopes.status);
    for p in &res.produced {
        println!("   + {p}");
    }
    println!("[render] 다음(HITL · 인가 인자 필수):");
    println!("   bash .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh \\");
    println!("        --mode experimental --manifest <work-manifest.json> --apply --provision --branch {topology}");
    println!("   ↑ work-manifest 발행 절차 = terraforming_node SKILL.md §2.3 '인가 체인'");
    Ok(0)
}
