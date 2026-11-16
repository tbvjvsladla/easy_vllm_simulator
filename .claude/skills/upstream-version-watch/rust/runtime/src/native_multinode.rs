//! `native_multinode_serve.py` (part A of the port): constants and validation, the marker-owned tree primitives, the process primitives,
//! the runners (local and SSH fixed-runner), `load_ctx`, the pure argv/env builders, the dry-run `plan`, `client` and `down`.
//!
//! `up --apply` (`NativeServe`) lives in the child module `up`, `--self-test` in `self_test`. The fixed runner on the sub node is
//! the Rust `native_remote` (child module `remote`) and the session tool is the `blackbox_session.sh` launcher on both nodes —
//! no Python on the Sub (plan_26111615 pass 3; the original sent a `python3 -c` program and ran `blackbox_session.py`).
//! Deviations on purpose: `check_smoke_model` / `budget_preflight` argv name their `.sh` launchers; the wheelhouse tool is the Rust
//! `native_wheelhouse` (main: its launcher · sub: the main node's ELF put on the run root) — no `native_wheelhouse.py` on either node.

use crate::gate_cli::{self, Values};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::cliparse::{Kind, Opt, Spec};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub mod pii;
pub mod up;
mod self_test;
mod remote;
/// The `native_remote` executable (the fixed runner the `Ssh` runner executes on the Sub).
pub use remote::main as remote_main;

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_FAIL: i32 = 3;
pub const EXIT_UNKNOWN: i32 = 4;
pub const MARKER: &str = ".native-multinode-owned.json";
pub const CREATED_BY: &str = "native_multinode_serve.py/v1";
pub const TOPOLOGY: &str = "multi";
pub const SHORT_BASE: &str = "/tmp/native-mn";
pub const AF_UNIX_BUDGET: usize = 107;
pub const HEALTH_INTERVAL_S: u64 = 5;
pub const RAY_JOIN_MAX_S: u64 = 300;
pub const WATCHDOG_THRESH_MIB: u64 = 10240;
pub const WATCHDOG_INTERVAL_S: u64 = 2;
pub const STOP_WAIT_S: u64 = 30;
pub const NATIVE_LAUNCH_FILE: &str = "native-launch.md";
pub const CLIENT_REL: &str = "bin/vllm-client";
pub const WHEEL_ACCEPT_REL: &str = ".claude/skills/upstream-version-watch/references/native_wheelhouse_accept.json";
/// The main node runs the wheelhouse tool through its Rust launcher (repo-relative · `build` and `verify`).
pub const WHEEL_LAUNCHER_REL: &str = ".claude/skills/upstream-version-watch/scripts/native_wheelhouse.sh";
/// The node-built ELF the launcher starts; its bytes are what the main node puts on the sub run root (no Python on either node).
pub const WHEEL_ELF_REL: &str = ".claude/skills/upstream-version-watch/rust/runtime/target/release/native_wheelhouse";
/// Where the sub runs the put ELF, under its run root.
pub const SUB_WHEEL_TOOL: &str = "tools/native_wheelhouse";
const RAY_SESSION_SOCKET_TAIL: &str = "/session_2026-09-23_12-30-45_123456_1234567/sockets/plasma_store";

/// `ServeError` (status 3) and its subclass `UnknownState` (status 4).
#[derive(Debug, Clone)]
pub enum Err {
    Serve(String),
    Unknown(String),
}
impl Err {
    pub fn msg(&self) -> &str {
        match self {
            Err::Serve(m) | Err::Unknown(m) => m,
        }
    }
}
pub type R<T> = Result<T, Err>;
fn serve<T>(m: impl Into<String>) -> R<T> {
    Err(Err::Serve(m.into()))
}
fn unknown<T>(m: impl Into<String>) -> R<T> {
    Err(Err::Unknown(m.into()))
}

// ── small Python-compatible helpers ────────────────────────────────────────────────────────────────

fn is_run_id(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && b.len() <= 64 && b[0].is_ascii_alphanumeric() && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}
fn is_host(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'@' | b':' | b'-'))
}
pub fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'@' | b'%' | b'+' | b'=' | b':' | b',' | b'.' | b'/' | b'-')) {
        return s.into();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}
pub fn shlex_join(v: &[String]) -> String {
    v.iter().map(|s| shlex_quote(s)).collect::<Vec<_>>().join(" ")
}
fn py_list_repr(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| PyStr::new(s).repr()).collect::<Vec<_>>().join(", "))
}
fn tail_chars(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for ch in data.chunks(3) {
        let n = (u32::from(ch[0]) << 16) | (u32::from(*ch.get(1).unwrap_or(&0)) << 8) | u32::from(*ch.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if ch.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if ch.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn vs(s: &str) -> PyValue {
    PyValue::str(s)
}
fn vi(n: i64) -> PyValue {
    PyValue::int(n)
}
fn dict(items: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in items {
        d.set(k, v);
    }
    PyValue::Dict(d)
}
/// Recursively sorted keys (`sort_keys=True`).
fn sorted(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut e: Vec<(PyStr, PyValue)> = d.iter().map(|(k, x)| (k.clone(), sorted(x))).collect();
            e.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (k, x) in e {
                out.insert(k, x);
            }
            PyValue::Dict(out)
        }
        PyValue::List(l) => PyValue::List(l.iter().map(sorted).collect()),
        o => o.clone(),
    }
}
fn dumps_pretty(v: &PyValue) -> String {
    sorted(v).dumps_pretty().ok().and_then(|s| s.to_utf8()).unwrap_or_default() + "\n"
}
fn dumps_tight(v: &PyValue) -> String {
    sorted(v).dumps_ascii_tight().unwrap_or_default()
}
fn dget<'a>(v: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(k).map(PyValue::peel),
        _ => None,
    }
}
fn dpath<'a>(v: &'a PyValue, ks: &[&str]) -> Option<&'a PyValue> {
    let mut cur = v;
    for k in ks {
        cur = dget(cur, k)?;
    }
    Some(cur)
}
fn as_str(v: Option<&PyValue>) -> Option<String> {
    match v?.peel() {
        PyValue::Str(s) => s.to_utf8(),
        _ => None,
    }
}
fn py_str(v: Option<&PyValue>) -> String {
    match v {
        None => "None".into(),
        Some(x) => x.display_text().to_utf8().unwrap_or_default(),
    }
}
fn as_i64(v: Option<&PyValue>) -> Option<i64> {
    match v?.peel() {
        PyValue::Int(n) => num_traits::ToPrimitive::to_i64(n),
        PyValue::Float(f) => Some(*f as i64),
        PyValue::Str(s) => s.to_utf8()?.trim().parse().ok(),
        _ => None,
    }
}
fn truthy(v: Option<&PyValue>) -> bool {
    v.is_some_and(PyValue::is_truthy)
}
fn list_items(v: Option<&PyValue>) -> Vec<&PyValue> {
    match v.map(PyValue::peel) {
        Some(PyValue::List(l)) => l.iter().map(PyValue::peel).collect(),
        _ => Vec::new(),
    }
}
fn load_yaml(path: &Path) -> R<PyValue> {
    let bytes = std::fs::read(path).map_err(|e| Err::Serve(format!("{}: {e}", path.display())))?;
    agent_runtime_support::pyyaml::safe_load(&bytes, &PyStr::new(&path.to_string_lossy())).map_err(|_| Err::Serve(format!("yaml 해석 실패: {}", path.display())))
}
pub fn utc(t: f64) -> String {
    let secs = t as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub trait Clock {
    fn now(&self) -> f64;
    fn mono(&self) -> f64;
    fn sleep(&self, s: f64);
}
pub struct RealClock;
impl Clock for RealClock {
    fn now(&self) -> f64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
    }
    fn mono(&self) -> f64 {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64()
    }
    fn sleep(&self, s: f64) {
        std::thread::sleep(Duration::from_secs_f64(s));
    }
}

// ── paths and the marker-owned tree ────────────────────────────────────────────────────────────────

fn resolve_nonstrict(p: &Path, depth: u32) -> PathBuf {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut cur = PathBuf::from("/");
    for comp in abs.components() {
        use std::path::Component::*;
        match comp {
            RootDir | Prefix(_) => cur = PathBuf::from("/"),
            CurDir => {}
            ParentDir => {
                cur.pop();
            }
            Normal(n) => {
                let next = cur.join(n);
                match std::fs::symlink_metadata(&next) {
                    Ok(m) if m.file_type().is_symlink() && depth < 40 => {
                        let t = std::fs::read_link(&next).unwrap_or_default();
                        let t = if t.is_absolute() { t } else { cur.join(t) };
                        cur = resolve_nonstrict(&t, depth + 1);
                    }
                    _ => cur = next,
                }
            }
        }
    }
    cur
}
pub fn beneath(root: &Path, path: &Path) -> R<PathBuf> {
    let root = resolve_nonstrict(root, 0);
    let path = resolve_nonstrict(path, 0);
    if path.strip_prefix(&root).is_err() {
        return serve(format!("root escape rejected: {}", path.display()));
    }
    Ok(path)
}
pub fn validate_remote_root(root: &str, run_id: &str) -> R<String> {
    let p = Path::new(root);
    let bad_part = p.components().any(|c| matches!(c, std::path::Component::CurDir | std::path::Component::ParentDir));
    if !p.is_absolute() || p.file_name().and_then(|n| n.to_str()) != Some(run_id) || bad_part || root.contains("/./") || root.contains("/../") {
        return serve("remote root must be absolute, traversal-free, and basename == run-id");
    }
    Ok(root.to_string())
}
pub fn marker_bytes(run_id: &str) -> Vec<u8> {
    let d = dict(vec![("run_id", vs(run_id)), ("created_by", vs(CREATED_BY))]);
    (dumps_tight(&d) + "\n").into_bytes()
}
fn ismount(p: &Path) -> bool {
    let (Ok(a), Ok(b)) = (std::fs::symlink_metadata(p), std::fs::symlink_metadata(p.join(".."))) else { return false };
    a.dev() != b.dev() || a.ino() == b.ino()
}
fn is_symlink(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}
fn lexists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

pub fn safe_remove_owned_tree(root: &Path, expected_run_id: &str) -> R<()> {
    if root.file_name().and_then(|n| n.to_str()) != Some(expected_run_id) || !is_run_id(expected_run_id) {
        return serve("owned-tree basename/run-id mismatch");
    }
    if is_symlink(root) || !root.is_dir() {
        return serve("owned-tree root missing, symlink, or non-directory");
    }
    if ismount(root) {
        return serve("owned-tree root is a mount point");
    }
    match std::fs::read(root.join(MARKER)) {
        Ok(b) if b == marker_bytes(expected_run_id) => {}
        Ok(_) => return serve("owned-tree marker is absent or not exact"),
        Err(e) => return serve(format!("owned-tree marker unreadable: {e}")),
    }
    let canonical = resolve_nonstrict(root, 0);
    fn remove(canonical: &Path, directory: &Path) -> R<()> {
        let directory = beneath(canonical, directory)?;
        if is_symlink(&directory) || ismount(&directory) {
            return serve(format!("owned-tree entry is symlink/mount point: {}", directory.display()));
        }
        let entries: Vec<_> = std::fs::read_dir(&directory).map_err(|e| Err::Serve(e.to_string()))?.filter_map(Result::ok).collect();
        for entry in entries {
            let name = entry.file_name();
            if directory == canonical && name == MARKER {
                continue;
            }
            let info = std::fs::symlink_metadata(entry.path()).map_err(|e| Err::Serve(e.to_string()))?;
            let ft = info.file_type();
            if ft.is_symlink() {
                std::fs::remove_file(directory.join(&name)).map_err(|e| Err::Serve(e.to_string()))?;
                continue;
            }
            let child = beneath(canonical, &directory.join(&name))?;
            if ft.is_dir() {
                remove(canonical, &child)?;
            } else if ft.is_file() || ft.is_socket() || ft.is_fifo() {
                std::fs::remove_file(&child).map_err(|e| Err::Serve(e.to_string()))?;
            } else {
                return serve(format!("owned-tree special file rejected: {}", child.display()));
            }
        }
        if directory == canonical {
            std::fs::remove_file(canonical.join(MARKER)).map_err(|e| Err::Serve(e.to_string()))?;
        }
        std::fs::remove_dir(&directory).map_err(|e| Err::Serve(e.to_string()))
    }
    remove(&canonical, &canonical)?;
    if lexists(root) {
        return unknown("owned tree remains after depth-first removal");
    }
    Ok(())
}

// ── process primitives (the PROC program of the original, natively) ────────────────────────────────

#[derive(Clone, Debug)]
pub struct ProcRow {
    pub pid: i64,
    pub ppid: i64,
    pub pgid: i64,
    pub starttime: String,
    pub argv: String,
}
impl ProcRow {
    fn value(&self) -> PyValue {
        dict(vec![("pid", vi(self.pid)), ("ppid", vi(self.ppid)), ("pgid", vi(self.pgid)), ("starttime", vs(&self.starttime)), ("argv", vs(&self.argv))])
    }
}
fn stat_tail(pid: &str) -> Option<Vec<String>> {
    let text = String::from_utf8(std::fs::read(format!("/proc/{pid}/stat")).ok()?).ok()?;
    Some(text.rsplit_once(')')?.1.split_whitespace().map(String::from).collect())
}
pub fn starttime(pid: i64) -> Option<String> {
    let t = stat_tail(&pid.to_string())?;
    if t.is_empty() || t[0] == "Z" || t[0] == "X" {
        return None;
    }
    t.get(19).cloned()
}
fn procs() -> BTreeMap<i64, ProcRow> {
    let mut out = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return out };
    for e in rd.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.is_empty() || !name.bytes().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Some(t) = stat_tail(&name) else { continue };
        if t.len() < 20 || t[0] == "Z" || t[0] == "X" {
            continue;
        }
        let argv = std::fs::read(format!("/proc/{name}/cmdline")).map_or(String::new(), |b| {
            let s = String::from_utf8_lossy(&b).replace('\0', " ");
            s.trim().chars().take(300).collect()
        });
        let (Ok(pid), Ok(ppid), Ok(pgid)) = (name.parse(), t[1].parse(), t[2].parse()) else { continue };
        out.insert(pid, ProcRow { pid, ppid, pgid, starttime: t[19].clone(), argv });
    }
    out
}
pub fn tree(pids: &[i64], pgids: &[i64]) -> Vec<ProcRow> {
    let ps = procs();
    let mut kids: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for p in ps.values() {
        kids.entry(p.ppid).or_default().push(p.pid);
    }
    let mut seen = BTreeSet::new();
    let mut todo: Vec<i64> = pids.iter().copied().filter(|p| ps.contains_key(p)).collect();
    while let Some(x) = todo.pop() {
        if !seen.insert(x) {
            continue;
        }
        todo.extend(kids.get(&x).cloned().unwrap_or_default());
    }
    for p in ps.values() {
        if pgids.contains(&p.pgid) {
            seen.insert(p.pid);
        }
    }
    seen.into_iter().filter_map(|x| ps.get(&x).cloned()).collect()
}
enum Cmd {
    Done(i32, String, String),
    Timeout,
    NotFound,
    Other(String),
}
fn run_cmd(argv: &[String], timeout_s: u64, input: Option<&str>) -> Cmd {
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..]).stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Cmd::NotFound,
        Err(e) => return Cmd::Other(e.to_string()),
    };
    if let (Some(data), Some(mut si)) = (input, child.stdin.take()) {
        let data = data.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
    }
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let ho = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let he = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let end = Instant::now() + Duration::from_secs(timeout_s);
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let (o, e) = (ho.join().unwrap_or_default(), he.join().unwrap_or_default());
                return Cmd::Done(st.code().unwrap_or(-1), String::from_utf8_lossy(&o).into_owned(), String::from_utf8_lossy(&e).into_owned());
            }
            Ok(None) if Instant::now() >= end => {
                let _ = child.kill();
                let _ = child.wait();
                return Cmd::Timeout;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Cmd::Other(e.to_string()),
        }
    }
}
fn gpu_query() -> Result<Vec<String>, String> {
    let argv: Vec<String> = ["nvidia-smi", "--query-compute-apps=pid,process_name,used_memory", "--format=csv,noheader,nounits"].map(String::from).to_vec();
    match run_cmd(&argv, 20, None) {
        Cmd::Done(0, o, _) => Ok(o.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()),
        Cmd::Done(rc, ..) => Err(format!("gpu-query rc={rc}")),
        Cmd::Timeout => Err("gpu-query timeout".into()),
        Cmd::NotFound => Err("nvidia-smi not found".into()),
        Cmd::Other(e) => Err(e),
    }
}
fn ports_query() -> Result<Vec<i64>, String> {
    let argv: Vec<String> = ["ss", "-ltnH"].map(String::from).to_vec();
    match run_cmd(&argv, 20, None) {
        Cmd::Done(0, o, _) => {
            let mut out = BTreeSet::new();
            for line in o.lines() {
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() >= 4 && f[3].contains(':') {
                    if let Ok(p) = f[3].rsplit(':').next().unwrap_or("").parse() {
                        out.insert(p);
                    }
                }
            }
            Ok(out.into_iter().collect())
        }
        Cmd::Done(rc, ..) => Err(format!("port-query rc={rc}")),
        Cmd::Timeout => Err("port-query timeout".into()),
        Cmd::NotFound => Err("ss not found".into()),
        Cmd::Other(e) => Err(e),
    }
}
/// `stop(pid, pgid, start, wait, pid_only)`: same-starttime TERM then KILL; `Err` = the original's RuntimeError text.
pub fn stop_process(pid: i64, pgid: i64, start: &str, wait: u64, pid_only: bool) -> Result<&'static str, String> {
    let Some(got) = starttime(pid) else { return Ok("already-gone") };
    if got != start {
        return Err("pid-reuse-before-term".into());
    }
    let sig = |s: i32| {
        // SAFETY: plain signal delivery to a pid/pgid whose starttime was just compared.
        unsafe {
            if pid_only {
                libc::kill(pid as i32, s);
            } else {
                libc::killpg(pgid as i32, s);
            }
        }
    };
    sig(libc::SIGTERM);
    let end = Instant::now() + Duration::from_secs(wait);
    while Instant::now() < end && starttime(pid).as_deref() == Some(start) {
        std::thread::sleep(Duration::from_millis(100));
    }
    match starttime(pid) {
        None => return Ok("term-exited"),
        Some(g) if g != start => return Err("pid-reuse-before-kill".into()),
        _ => {}
    }
    sig(libc::SIGKILL);
    let end = Instant::now() + Duration::from_secs(wait.min(10));
    while Instant::now() < end && starttime(pid).as_deref() == Some(start) {
        std::thread::sleep(Duration::from_millis(100));
    }
    if starttime(pid).as_deref() == Some(start) {
        return Err("survived-kill".into());
    }
    Ok("kill-exited")
}

// ── fixed sub-node runner ──────────────────────────────────────────────────────────────────────────
// The Sub runs the Rust `native_remote` (child module `remote`) — the same operations and refusals as the original's
// `python3 -c BOOT REMOTE` program, without Python on the Sub. Its ELF is the upstream runtime's, delivered to the Sub at
// the same repository-relative place (`sync_to_sub.sh` `SUB_ELF_NAMES`, or the Sub's own `build_rust_packages.sh`).

/// `<sub work_dir>/<this>` is the fixed runner the `Ssh` runner executes on the Sub.
pub const SUB_REMOTE_REL: &str = ".claude/skills/upstream-version-watch/rust/runtime/target/release/native_remote";

// ── identity · runners ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Identity {
    pub node: String,
    pub role: String,
    pub pid: i64,
    pub pgid: i64,
    pub starttime: String,
    pub log_rel: String,
}
impl Identity {
    pub fn row(&self) -> PyValue {
        dict(vec![
            ("node", vs(&self.node)),
            ("role", vs(&self.role)),
            ("pid", vi(self.pid)),
            ("pgid", vi(self.pgid)),
            ("starttime", vs(&self.starttime)),
            ("log_rel", vs(&self.log_rel)),
        ])
    }
    fn from_row(row: &PyValue) -> R<Identity> {
        let g = |k: &str| dget(row, k);
        match (as_str(g("node")), as_str(g("role")), as_i64(g("pid")), as_i64(g("pgid")), g("starttime"), as_str(g("log_rel"))) {
            (Some(node), Some(role), Some(pid), Some(pgid), Some(st), Some(log_rel)) => {
                Ok(Identity { node, role, pid, pgid, starttime: py_str(Some(st)), log_rel })
            }
            _ => serve("state identity row is incomplete"),
        }
    }
}
pub struct CmdResult {
    pub rc: i32,
    pub out: String,
    pub err: String,
}

pub trait Runner {
    fn node(&self) -> &str;
    fn root(&self) -> &str;
    fn init_root(&self, ensure_parent: bool) -> R<()>;
    fn put(&self, rel: &str, data: &[u8], mode: u32) -> R<()>;
    fn run(&self, argv: &[String], timeout: u64, check: bool) -> R<CmdResult>;
    fn start(&self, argv: &[String], log_rel: &str, role: &str) -> R<Identity>;
    fn stop(&self, ident: &Identity, wait: u64, pid_only: bool) -> R<PyValue>;
    fn text(&self, rel: &str) -> R<String>;
    fn tree(&self, pids: &[i64], pgids: &[i64]) -> R<Vec<ProcRow>>;
    fn gpu(&self) -> R<Vec<String>>;
    fn ports(&self) -> R<Vec<i64>>;
    fn exists(&self) -> R<bool>;
    fn remove_owned_tree(&self, run_id: &str) -> R<()>;
}
fn check_argv(argv: &[String]) -> R<()> {
    if argv.is_empty() || argv.iter().any(|a| a.contains('\0')) {
        return serve("invalid exact argv");
    }
    Ok(())
}
fn finish(node: &str, argv: &[String], r: CmdResult, check: bool) -> R<CmdResult> {
    if check && r.rc != 0 {
        return serve(format!("{node}: rc={}: {}: {}", r.rc, py_list_repr(&argv[..argv.len().min(4)]), tail_chars(&r.err, 800)));
    }
    Ok(r)
}

pub struct Local {
    root: String,
    node: String,
}
impl Local {
    pub fn new(root: &str, node: &str) -> Local {
        Local { root: root.into(), node: node.into() }
    }
    fn p(&self, rel: &str) -> R<PathBuf> {
        if rel.is_empty() || rel.starts_with('/') {
            return serve("bad-relative-path");
        }
        beneath(Path::new(&self.root), &Path::new(&self.root).join(rel))
    }
}
fn mkdir_mode(p: &Path, mode: u32, parents: bool) -> std::io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(parents).mode(mode).create(p)
}
impl Runner for Local {
    fn node(&self) -> &str {
        &self.node
    }
    fn root(&self) -> &str {
        &self.root
    }
    fn init_root(&self, ensure_parent: bool) -> R<()> {
        let root = Path::new(&self.root);
        if lexists(root) {
            return serve(format!("{}: root already exists: {}", self.node, self.root));
        }
        let parent = root.parent().unwrap_or(Path::new("/"));
        if ensure_parent {
            mkdir_mode(parent, 0o700, true).map_err(|e| Err::Serve(e.to_string()))?;
        }
        if is_symlink(parent) || !parent.is_dir() {
            return serve(format!("{}: bad parent {}", self.node, parent.display()));
        }
        mkdir_mode(root, 0o700, false).map_err(|e| Err::Serve(e.to_string()))?;
        let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("");
        std::fs::write(root.join(MARKER), marker_bytes(name)).map_err(|e| Err::Serve(e.to_string()))
    }
    fn put(&self, rel: &str, data: &[u8], mode: u32) -> R<()> {
        let p = self.p(rel)?;
        if let Some(par) = p.parent() {
            mkdir_mode(par, 0o700, true).map_err(|e| Err::Serve(e.to_string()))?;
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW)
            .mode(mode)
            .open(&p)
            .map_err(|e| Err::Serve(e.to_string()))?;
        f.write_all(data).map_err(|e| Err::Serve(e.to_string()))
    }
    fn run(&self, argv: &[String], timeout: u64, check: bool) -> R<CmdResult> {
        check_argv(argv)?;
        match run_cmd(argv, timeout, None) {
            Cmd::Timeout => unknown(format!("{}: bounded command timed out: {}", self.node, py_list_repr(&argv[..argv.len().min(3)]))),
            Cmd::NotFound => serve(format!("{}: executable not found: {}", self.node, argv[0])),
            Cmd::Other(e) => serve(format!("{}: {e}", self.node)),
            Cmd::Done(rc, out, err) => finish(&self.node, argv, CmdResult { rc, out, err }, check),
        }
    }
    fn start(&self, argv: &[String], log_rel: &str, role: &str) -> R<Identity> {
        check_argv(argv)?;
        let log = self.p(log_rel)?;
        if let Some(par) = log.parent() {
            mkdir_mode(par, 0o700, true).map_err(|e| Err::Serve(e.to_string()))?;
        }
        let f = std::fs::OpenOptions::new().write(true).create_new(true).open(&log).map_err(|e| Err::Serve(e.to_string()))?;
        let f2 = f.try_clone().map_err(|e| Err::Serve(e.to_string()))?;
        let mut c = Command::new(&argv[0]);
        c.args(&argv[1..]).stdin(Stdio::null()).stdout(Stdio::from(f)).stderr(Stdio::from(f2));
        // SAFETY: setsid in the forked child before exec — `start_new_session=True` of the original.
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let child = c.spawn().map_err(|e| Err::Serve(format!("{}: {e}", self.node)))?;
        let pid = i64::from(child.id());
        let Some(st) = starttime(pid) else { return serve(format!("{}: {role} exited at start (see {log_rel})", self.node)) };
        // SAFETY: getpgid on a child we just spawned.
        let pgid = i64::from(unsafe { libc::getpgid(pid as i32) });
        Ok(Identity { node: self.node.clone(), role: role.into(), pid, pgid, starttime: st, log_rel: log_rel.into() })
    }
    fn stop(&self, i: &Identity, wait: u64, pid_only: bool) -> R<PyValue> {
        match stop_process(i.pid, i.pgid, &i.starttime, wait, pid_only) {
            Ok(status) => Ok(dict(vec![("node", vs(&self.node)), ("role", vs(&i.role)), ("pid", vi(i.pid)), ("term_status", vs(status)), ("starttime_match", PyValue::Bool(true))])),
            Err(e) => unknown(format!("{}: {}: {e}", self.node, i.role)),
        }
    }
    fn text(&self, rel: &str) -> R<String> {
        let p = self.p(rel)?;
        std::fs::read(&p).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| Err::Serve(format!("{}: {e}", p.display())))
    }
    fn tree(&self, pids: &[i64], pgids: &[i64]) -> R<Vec<ProcRow>> {
        Ok(tree(pids, pgids))
    }
    fn gpu(&self) -> R<Vec<String>> {
        gpu_query().map_err(|e| Err::Unknown(format!("{}: fresh GPU query failed: {e}", self.node)))
    }
    fn ports(&self) -> R<Vec<i64>> {
        ports_query().map_err(|e| Err::Unknown(format!("{}: fresh port query failed: {e}", self.node)))
    }
    fn exists(&self) -> R<bool> {
        Ok(lexists(Path::new(&self.root)))
    }
    fn remove_owned_tree(&self, run_id: &str) -> R<()> {
        safe_remove_owned_tree(Path::new(&self.root), run_id)
    }
}

pub struct Ssh {
    root: String,
    node: String,
    transport: Vec<String>,
    /// The fixed runner on the Sub (`<work_dir>/SUB_REMOTE_REL`).
    remote: String,
}
impl Ssh {
    pub fn new(host: &str, root: &str, node: &str, sub_work_dir: &str) -> R<Ssh> {
        if !is_host(host) {
            return serve("unsafe sub host");
        }
        if !sub_work_dir.starts_with('/') {
            return serve("manifest nodes[sub].work_dir 가 절대경로가 아니다");
        }
        let transport = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", host].map(String::from).to_vec();
        let remote = format!("{}/{SUB_REMOTE_REL}", sub_work_dir.trim_end_matches('/'));
        Ok(Ssh { root: root.into(), node: node.into(), transport, remote })
    }
    fn call(&self, op: &str, bound_s: u64, extra: Vec<(&str, PyValue)>) -> R<PyValue> {
        const ALLOWED: [&str; 11] = ["init", "put", "run", "start", "stop", "text", "tree", "gpu", "ports", "exists", "remove_owned_tree"];
        if !ALLOWED.contains(&op) {
            return serve(format!("sub fixed runner operation outside allowlist: {op}"));
        }
        let run_id = Path::new(&self.root).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let mut items = extra;
        items.extend([("op", vs(op)), ("root", vs(&self.root)), ("run_id", vs(&run_id)), ("marker", vs(MARKER)), ("created_by", vs(CREATED_BY))]);
        let payload = b64(dumps_tight(&dict(items)).as_bytes());
        let inner: Vec<String> = vec![self.remote.clone(), "-".into()];
        let mut argv = self.transport.clone();
        argv.push(shlex_join(&inner));
        match run_cmd(&argv, bound_s + 13, Some(&payload)) {
            Cmd::Timeout => unknown(format!("{}: fixed runner bounded timeout ({op})", self.node)),
            Cmd::NotFound | Cmd::Other(_) => unknown(format!("{}: fixed runner rejected {op}: transport unavailable", self.node)),
            Cmd::Done(rc, out, err) => {
                if rc != 0 {
                    return unknown(format!("{}: fixed runner rejected {op}: {}", self.node, tail_chars(err.trim(), 500)));
                }
                agent_runtime_support::pyjson::loads(&out).map_err(|_| Err::Unknown(format!("{}: fixed runner non-JSON result ({op})", self.node)))
            }
        }
    }
    fn ok(&self, op: &str, bound: u64, extra: Vec<(&str, PyValue)>, what: &str) -> R<()> {
        let r = self.call(op, bound, extra)?;
        if matches!(dget(&r, "ok"), Some(PyValue::Bool(true))) {
            Ok(())
        } else {
            unknown(format!("{}: {what}", self.node))
        }
    }
}
impl Runner for Ssh {
    fn node(&self) -> &str {
        &self.node
    }
    fn root(&self) -> &str {
        &self.root
    }
    fn init_root(&self, ensure_parent: bool) -> R<()> {
        self.ok("init", 30, vec![("ensure_parent", PyValue::Bool(ensure_parent))], "init non-pass")
    }
    fn put(&self, rel: &str, data: &[u8], mode: u32) -> R<()> {
        self.ok("put", 60, vec![("rel", vs(rel)), ("data", vs(&b64(data))), ("mode", vi(i64::from(mode)))], "put non-pass")
    }
    fn run(&self, argv: &[String], timeout: u64, check: bool) -> R<CmdResult> {
        check_argv(argv)?;
        let r = self.call("run", timeout, vec![("argv", PyValue::List(argv.iter().map(|a| vs(a)).collect())), ("timeout", vi(timeout as i64))])?;
        let rc = as_i64(dget(&r, "rc")).ok_or_else(|| Err::Unknown(format!("{}: fixed runner non-JSON result (run)", self.node)))? as i32;
        let res = CmdResult { rc, out: as_str(dget(&r, "out")).unwrap_or_default(), err: as_str(dget(&r, "err")).unwrap_or_default() };
        finish(&self.node, argv, res, check)
    }
    fn start(&self, argv: &[String], log_rel: &str, role: &str) -> R<Identity> {
        check_argv(argv)?;
        let r = self.call(
            "start",
            30,
            vec![("argv", PyValue::List(argv.iter().map(|a| vs(a)).collect())), ("log_rel", vs(log_rel)), ("role", vs(role)), ("node", vs(&self.node))],
        )?;
        Identity::from_row(&r)
    }
    fn stop(&self, i: &Identity, wait: u64, pid_only: bool) -> R<PyValue> {
        self.call("stop", wait + 15, vec![("identity", i.row()), ("wait", vi(wait as i64)), ("pid_only", PyValue::Bool(pid_only))])
    }
    fn text(&self, rel: &str) -> R<String> {
        let r = self.call("text", 30, vec![("rel", vs(rel))])?;
        as_str(dget(&r, "text")).ok_or_else(|| Err::Unknown(format!("{}: fixed runner non-JSON result (text)", self.node)))
    }
    fn tree(&self, pids: &[i64], pgids: &[i64]) -> R<Vec<ProcRow>> {
        let r = self.call("tree", 30, vec![("pids", PyValue::List(pids.iter().map(|p| vi(*p)).collect())), ("pgids", PyValue::List(pgids.iter().map(|p| vi(*p)).collect()))])?;
        Ok(list_items(dget(&r, "processes"))
            .into_iter()
            .filter_map(|row| {
                Some(ProcRow {
                    pid: as_i64(dget(row, "pid"))?,
                    ppid: as_i64(dget(row, "ppid"))?,
                    pgid: as_i64(dget(row, "pgid"))?,
                    starttime: py_str(dget(row, "starttime")),
                    argv: as_str(dget(row, "argv")).unwrap_or_default(),
                })
            })
            .collect())
    }
    fn gpu(&self) -> R<Vec<String>> {
        let r = self.call("gpu", 25, vec![])?;
        Ok(list_items(dget(&r, "processes")).into_iter().filter_map(|v| as_str(Some(v))).collect())
    }
    fn ports(&self) -> R<Vec<i64>> {
        let r = self.call("ports", 25, vec![])?;
        Ok(list_items(dget(&r, "ports")).into_iter().filter_map(|v| as_i64(Some(v))).collect())
    }
    fn exists(&self) -> R<bool> {
        let r = self.call("exists", 20, vec![])?;
        Ok(truthy(dget(&r, "exists")))
    }
    fn remove_owned_tree(&self, run_id: &str) -> R<()> {
        if Path::new(&self.root).file_name().and_then(|n| n.to_str()) != Some(run_id) {
            return serve("owned-tree basename/run-id mismatch");
        }
        self.ok("remove_owned_tree", 120, vec![], "owned-tree removal non-pass")
    }
}

// ── declaration and facts ──────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub name: String,
    pub host_ip: String,
    pub work_dir: String,
    pub root: String,
    pub short: String,
    pub session_py: String,
    pub renew_sh: String,
    pub watchdog_sh: String,
    pub node_dir: String,
    pub wheel_tool: String,
    pub wheel_accept: String,
}
impl NodeSpec {
    pub fn row(&self) -> PyValue {
        dict(vec![
            ("name", vs(&self.name)),
            ("node_id", vs(&self.name)),
            ("host_ip", vs(&self.host_ip)),
            ("work_dir", vs(&self.work_dir)),
            ("root", vs(&self.root)),
            ("short", vs(&self.short)),
            ("session_py", vs(&self.session_py)),
            ("renew_sh", vs(&self.renew_sh)),
            ("watchdog_sh", vs(&self.watchdog_sh)),
            ("node_dir", vs(&self.node_dir)),
            ("wheel_tool", vs(&self.wheel_tool)),
            ("wheel_accept", vs(&self.wheel_accept)),
        ])
    }
}

pub type Env = BTreeMap<String, String>;

pub struct Ctx {
    pub repo: PathBuf,
    pub cell: String,
    pub source_cell: String,
    pub run_id: String,
    pub campaign_id: String,
    pub base: String,
    pub max_bytes: u64,
    pub sub_host: String,
    pub ready_max_s: i64,
    pub overhead_mib: i64,
    pub serve_port: i64,
    pub ray_port: i64,
    pub model_name: String,
    pub object_store: String,
    pub image: String,
    pub cell_env: Env,
    pub cluster_env: Env,
    pub ic_env: Env,
    pub serve_env_keys: Vec<String>,
    pub triplet: BTreeMap<&'static str, String>,
    pub main: NodeSpec,
    pub sub: NodeSpec,
    pub proof_rel: String,
    pub simlog_topic: String,
    pub wheel_tool_rel: String,
    pub wheel_accept_rel: String,
}
impl Ctx {
    pub fn node(&self, name: &str) -> &NodeSpec {
        if name == "main" {
            &self.main
        } else {
            &self.sub
        }
    }
}

fn read_env(path: &Path) -> R<Env> {
    if !path.is_file() {
        return serve(format!("env file absent: {}", path.display()));
    }
    Ok(crate::native_triplet::read_env_first(path))
}

pub fn load_ctx(repo: &Path, cell: &str, run_id: &str, source_cell: Option<&str>, ready_max_s: Option<i64>, simlog_topic: &str) -> R<Ctx> {
    let repo = resolve_nonstrict(repo, 0);
    if !is_run_id(run_id) {
        return serve("unsafe run-id");
    }
    let active_p = repo.join("campaigns").join("ACTIVE");
    let active = if active_p.is_file() { String::from_utf8_lossy(&std::fs::read(&active_p).unwrap_or_default()).trim().to_string() } else { String::new() };
    if active.is_empty() || active == "_bootstrap" {
        return serve("활성 캠페인이 없다(campaigns/ACTIVE) — native 선언을 읽을 곳이 없다");
    }
    let camp = load_yaml(&repo.join("campaigns").join(&active).join("campaign.yaml"))?;
    let sec = dpath(&camp, &["topology_sections", TOPOLOGY]);
    let native_cells: Vec<String> = list_items(sec.and_then(|s| dget(s, "native_cells"))).into_iter().filter_map(|v| as_str(Some(v))).collect();
    if !native_cells.iter().any(|c| c == cell) {
        return serve(format!("{cell} 는 캠페인 {active} 의 native_cells 가 아니다"));
    }
    let g = |k: &str| sec.and_then(|s| dget(s, k));
    let base = as_str(g("native_run_root_base")).filter(|b| b.starts_with('/'));
    let max_gib = match g("native_run_root_max_gib_per_node") {
        Some(PyValue::Int(n)) => num_traits::ToPrimitive::to_f64(n),
        Some(PyValue::Float(f)) => Some(*f),
        _ => None,
    }
    .filter(|m| *m > 0.0);
    let sub_host = as_str(g("native_sub_host"));
    let (Some(base), Some(max_gib), Some(sub_host)) = (base, max_gib, sub_host) else {
        return serve("캠페인 topology_sections.multi 의 native_run_root_base/_max_gib_per_node/native_sub_host 선언이 불완전하다");
    };
    let budgets = dget(&camp, "budgets");
    let rmax = ready_max_s.unwrap_or_else(|| as_i64(budgets.and_then(|b| dget(b, "ready_max_seconds"))).unwrap_or(0));
    let overhead = as_i64(budgets.and_then(|b| dget(b, "smoke_budget_overhead_mib"))).unwrap_or(0);
    if rmax <= 0 || overhead <= 0 {
        return serve("budgets.ready_max_seconds·smoke_budget_overhead_mib 선언이 없다(기본값 ✗)");
    }
    let source = match source_cell {
        Some(s) => s.to_string(),
        None => cell.strip_suffix("-native").unwrap_or("").to_string(),
    };
    if source.is_empty() {
        return serve("원본 Docker 셀을 파생할 수 없다(--source-cell 또는 `<셀>-native` 이름)");
    }
    let rendered = crate::native_triplet::render(&repo, TOPOLOGY, &source, cell).map_err(|f| match f {
        crate::native_triplet::Failure::Render(m) => Err::Serve(format!("native 트리플렛 렌더 거부: {m}")),
        crate::native_triplet::Failure::Forward(m) => Err::Serve(m),
    })?;
    let drift = crate::native_triplet::check(&repo, &rendered);
    if !drift.is_empty() {
        return serve(format!("native 트리플렛이 렌더와 다르다(손편집·원본 변경) — render_native_triplet.py --apply: {}", drift.join("; ")));
    }
    let out = repo.join("output").join(TOPOLOGY);
    let triplet: BTreeMap<&'static str, String> = crate::native_triplet::triplet_rel(TOPOLOGY, cell).into_iter().collect();
    let cell_env = read_env(&repo.join(&triplet["env"]))?;
    let src_env = read_env(&out.join("envs").join(format!(".env.{source}")))?;
    let cluster_env = read_env(&out.join("envs").join(".env.cluster"))?;
    let ic_env = read_env(&out.join("envs").join(".env.interconnect"))?;
    let compose = out.join("docker-compose.yaml").to_string_lossy().into_owned();
    let cluster_p = out.join("envs").join(".env.cluster").to_string_lossy().into_owned();
    let groups = crate::slave_forward::candidates(&compose, Some(&cluster_p)).map_err(|e| Err::Serve(e.message))?;
    let serve_env_keys: Vec<String> = groups.into_iter().find(|(g, _)| *g == "serve_env").map(|(_, k)| k).unwrap_or_default();
    let image = src_env.get("IMAGE_TAG").map(|s| s.trim().to_string()).unwrap_or_default();
    if image.is_empty() {
        return serve("원본 셀 env 에 IMAGE_TAG 가 없다 — wheelhouse 의 원천 이미지를 알 수 없다");
    }
    let man = load_yaml(&out.join("manifest.yaml"))?;
    let mut nodes_m: BTreeMap<String, &PyValue> = BTreeMap::new();
    for n in list_items(dget(&man, "nodes")) {
        if matches!(n, PyValue::Dict(_)) {
            nodes_m.insert(py_str(dget(n, "role")), n);
        }
    }
    let (Some(msub), Some(mmain)) = (nodes_m.get("sub").copied(), nodes_m.get("main").copied()) else {
        return serve("manifest nodes[] 에 main·sub 가 없다");
    };
    if format!("{}@{}", py_str(dget(msub, "ssh_user")), py_str(dget(msub, "host"))) != sub_host {
        return serve("캠페인 native_sub_host 가 manifest nodes[sub] 와 다르다");
    }
    if !is_host(&sub_host) {
        return serve("unsafe sub host");
    }
    let sv = |n: &PyValue, k: &str| if truthy(dget(n, k)) { py_str(dget(n, k)) } else { String::new() };
    let (main_ip, sub_ip) = (sv(mmain, "host"), sv(msub, "host"));
    if cluster_env.get("MASTER_HOST_IP").is_some_and(|v| *v != main_ip) || cluster_env.get("SLAVE_HOST_IP").is_some_and(|v| *v != sub_ip) {
        return serve(".env.cluster 의 MASTER/SLAVE_HOST_IP 가 manifest nodes[] 와 다르다");
    }
    let sub_wd = sv(msub, "work_dir");
    if !sub_wd.starts_with('/') {
        return serve("manifest nodes[sub].work_dir 가 절대경로가 아니다");
    }
    let root = validate_remote_root(&format!("{}/{run_id}", base.trim_end_matches('/')), run_id)?;
    let short = validate_remote_root(&format!("{SHORT_BASE}/{run_id}"), run_id)?;
    if short.chars().count() + "/ray".len() + RAY_SESSION_SOCKET_TAIL.len() > AF_UNIX_BUDGET {
        return serve(format!("run-id 가 길어 Ray 소켓 경로가 AF_UNIX {AF_UNIX_BUDGET}B 를 넘는다 — 더 짧은 --run-id"));
    }
    let nb = ".claude/skills/terraforming_node/scripts";
    let rp = repo.to_string_lossy().into_owned();
    let main = NodeSpec {
        name: "main".into(),
        host_ip: main_ip,
        work_dir: rp.clone(),
        root: root.clone(),
        short: short.clone(),
        session_py: format!("{rp}/{nb}/node_blackbox/blackbox_session.sh"),
        renew_sh: format!("{rp}/{nb}/node_blackbox/budget_renew_loop.sh"),
        watchdog_sh: format!("{rp}/{nb}/host_safety/mem_watchdog.sh"),
        node_dir: format!("{rp}/docs/logs/main"),
        wheel_tool: format!("{rp}/{WHEEL_LAUNCHER_REL}"),
        wheel_accept: format!("{rp}/{WHEEL_ACCEPT_REL}"),
    };
    let sub = NodeSpec {
        name: "sub".into(),
        host_ip: sub_ip,
        work_dir: sub_wd.clone(),
        root: root.clone(),
        short,
        session_py: format!("{sub_wd}/.claude/runtime/node_blackbox/blackbox_session.sh"),
        renew_sh: format!("{sub_wd}/.claude/runtime/node_blackbox/budget_renew_loop.sh"),
        watchdog_sh: format!("{sub_wd}/.claude/runtime/host_safety/mem_watchdog.sh"),
        node_dir: format!("{sub_wd}/docs/logs/sub"),
        wheel_tool: format!("{root}/{SUB_WHEEL_TOOL}"),
        wheel_accept: format!("{root}/tools/native_wheelhouse_accept.json"),
    };
    let y = load_yaml(&repo.join(&triplet["yaml"]))?;
    let envn = |e: &Env, k: &str, d: i64| -> R<i64> {
        match e.get(k).filter(|v| !v.is_empty()) {
            None => Ok(d),
            Some(v) => v.trim().parse().map_err(|_| Err::Serve(format!("{k} 가 정수가 아니다: {v}"))),
        }
    };
    let port = envn(&cell_env, "SERVING_PORT", 0)?;
    let yport = if truthy(dget(&y, "port")) { as_i64(dget(&y, "port")).unwrap_or(0) } else { 0 };
    if port <= 0 || yport != port {
        return serve("셀 SERVING_PORT 와 yaml port 가 다르다");
    }
    let ray_port = envn(&cluster_env, "RAY_PORT", 6379)?;
    let nz = |e: &Env, k: &str, d: &str| e.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| d.into());
    Ok(Ctx {
        proof_rel: format!("output/{TOPOLOGY}/benchlog/serve_proof_{cell}.json"),
        model_name: cell_env.get("SERVING_MODEL_NAME").cloned().unwrap_or_default(),
        object_store: nz(&cluster_env, "RAY_OBJECT_STORE_MEMORY", "2000000000"),
        repo,
        cell: cell.into(),
        source_cell: source,
        run_id: run_id.into(),
        campaign_id: active,
        base,
        max_bytes: (max_gib * f64::from(1u32 << 30)) as u64,
        sub_host,
        ready_max_s: rmax,
        overhead_mib: overhead,
        serve_port: port,
        ray_port,
        image,
        cell_env,
        cluster_env,
        ic_env,
        serve_env_keys,
        triplet,
        main,
        sub,
        simlog_topic: simlog_topic.into(),
        wheel_tool_rel: WHEEL_LAUNCHER_REL.into(),
        wheel_accept_rel: WHEEL_ACCEPT_REL.into(),
    })
}

// ── pure builders ──────────────────────────────────────────────────────────────────────────────────

fn sv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}
pub fn wh_dir(root: &str) -> String {
    format!("{root}/wh")
}
pub fn venv_dir(root: &str) -> String {
    format!("{root}/venv")
}
pub fn wheelhouse_build_argv(tool: &str, image: &str, root: &str, generated_utc: &str, accept: &str) -> Vec<String> {
    let w = wh_dir(root);
    sv(&[tool, "build", "--image", image, "--out", &w, "--generated-utc", generated_utc, "--accept-file", accept])
}
/// `tool` = the main node's Rust launcher (`native_wheelhouse.sh`) or the ELF the main node put on the sub run root.
pub fn wheelhouse_verify_argv(tool: &str, root: &str, generated_utc: &str, accept: &str) -> Vec<String> {
    let (w, v) = (wh_dir(root), venv_dir(root));
    sv(&[tool, "verify", "--wheelhouse-dir", &w, "--venv", &v, "--generated-utc", generated_utc, "--accept-file", accept])
}
/// The ELF the main node's launcher starts (same root rule as the launcher: a set `EASY_VLLM_UPSTREAM_RUNTIME_ROOT` wins).
pub fn wheel_elf_path(repo: &std::path::Path) -> std::path::PathBuf {
    match std::env::var_os("EASY_VLLM_UPSTREAM_RUNTIME_ROOT").filter(|v| !v.is_empty()) {
        Some(root) => std::path::PathBuf::from(root).join("target/release/native_wheelhouse"),
        None => repo.join(WHEEL_ELF_REL),
    }
}
pub fn pip_freeze_argv(root: &str) -> Vec<String> {
    vec![format!("{}/bin/python", venv_dir(root)), "-m".into(), "pip".into(), "freeze".into(), "--all".into()]
}
pub fn watchdog_argv(spec: &NodeSpec, pgid: &str, starttime: &str) -> Vec<String> {
    sv(&["bash", &spec.watchdog_sh, "--pgid", pgid, "--starttime", starttime, "--thresh-mib", &WATCHDOG_THRESH_MIB.to_string(), "--interval-s", &WATCHDOG_INTERVAL_S.to_string()])
}
pub fn renew_argv(spec: &NodeSpec, pgid: &str, starttime: &str, ttl_s: i64) -> Vec<String> {
    sv(&["bash", &spec.renew_sh, "--node-dir", &spec.node_dir, "--pgid", pgid, "--starttime", starttime, "--ttl-s", &ttl_s.to_string()])
}
pub const CACHE_DIRS: [(&str, &str); 10] = [
    ("XDG_CACHE_HOME", "cache/xdg"),
    ("TRITON_CACHE_DIR", "cache/triton"),
    ("TORCHINDUCTOR_CACHE_DIR", "cache/inductor"),
    ("FLASHINFER_WORKSPACE_BASE", "cache/flashinfer"),
    ("VLLM_CACHE_ROOT", "cache/vllm"),
    ("HF_HOME", "cache/hf"),
    ("PIP_CACHE_DIR", "cache/pip"),
    ("CUDA_CACHE_PATH", "cache/nv"),
    ("TORCH_EXTENSIONS_DIR", "cache/torch_extensions"),
    ("HOME", "home"),
];
pub fn runtime_env(ctx: &Ctx, spec: &NodeSpec, verify_env: &Env) -> Env {
    let mut env = Env::new();
    env.extend(ctx.cluster_env.clone());
    env.extend(ctx.ic_env.clone());
    if spec.name == "main" {
        env.extend(ctx.cell_env.clone());
    } else {
        for k in &ctx.serve_env_keys {
            if let Some(v) = ctx.cell_env.get(k).filter(|v| !v.is_empty()) {
                env.insert(k.clone(), v.clone());
            }
        }
    }
    for (k, v) in verify_env {
        if k != "PATH" {
            env.insert(k.clone(), v.clone());
        }
    }
    let venv_bin = format!("{}/bin", venv_dir(&spec.root));
    let prefix = format!("{}/", spec.root);
    let owned: Vec<String> = verify_env.get("PATH").map(String::as_str).unwrap_or("").split(':').filter(|p| p.starts_with(&prefix) && *p != venv_bin).map(String::from).collect();
    for (k, v) in CACHE_DIRS {
        env.insert(k.into(), format!("{}/{v}", spec.root));
    }
    let mut path = owned;
    path.push(venv_bin);
    path.push("/usr/local/bin:/usr/bin:/bin".into());
    for (k, v) in [
        ("TMPDIR", format!("{}/tmp", spec.short)),
        ("PATH", path.join(":")),
        ("LANG", "C.UTF-8".into()),
        ("VLLM_HOST_IP", spec.host_ip.clone()),
        ("HF_HUB_OFFLINE", "1".into()),
        ("TRANSFORMERS_OFFLINE", "1".into()),
        ("VLLM_NO_USAGE_STATS", "1".into()),
        ("DO_NOT_TRACK", "1".into()),
        ("RAY_USAGE_STATS_ENABLED", "0".into()),
        ("NATIVE_VENV", venv_dir(&spec.root)),
    ] {
        env.insert(k.into(), v);
    }
    env
}
pub fn env_argv(env: &Env, argv: &[String]) -> Vec<String> {
    let mut out = sv(&["env", "-i"]);
    out.extend(env.iter().map(|(k, v)| format!("{k}={v}")));
    out.extend(argv.iter().cloned());
    out
}
pub fn client_wrapper_text(env: &Env, spec: &NodeSpec) -> String {
    let body = env.iter().map(|(k, v)| shlex_quote(&format!("{k}={v}"))).collect::<Vec<_>>().join(" ");
    format!(
        "#!/bin/sh\n# native 벤치 클라이언트 래퍼 — native_multinode_serve.py 가 설치 직후 생성(run root 와 함께 삭제)\nexec env -i {body} {} \"$@\"\n",
        shlex_quote(&format!("{}/bin/vllm", venv_dir(&spec.root)))
    )
}
pub fn mkdirs_argv(spec: &NodeSpec) -> Vec<String> {
    let dirs: BTreeSet<&str> = CACHE_DIRS.iter().map(|(_, v)| *v).collect();
    let mut out = sv(&["mkdir", "-p"]);
    out.extend(dirs.into_iter().map(|v| format!("{}/{v}", spec.root)));
    out.extend([format!("{}/logs", spec.root), format!("{}/tmp", spec.short), format!("{}/ray", spec.short)]);
    out
}
pub fn ray_head_argv(ctx: &Ctx, spec: &NodeSpec) -> Vec<String> {
    vec![
        format!("{}/bin/ray", venv_dir(&spec.root)),
        "start".into(),
        "--head".into(),
        "--node-ip-address".into(),
        spec.host_ip.clone(),
        "--port".into(),
        ctx.ray_port.to_string(),
        "--object-store-memory".into(),
        ctx.object_store.clone(),
        "--temp-dir".into(),
        format!("{}/ray", spec.short),
        "--disable-usage-stats".into(),
        "--block".into(),
    ]
}
pub fn ray_worker_argv(ctx: &Ctx, spec: &NodeSpec) -> Vec<String> {
    vec![
        format!("{}/bin/ray", venv_dir(&spec.root)),
        "start".into(),
        "--address".into(),
        format!("{}:{}", ctx.main.host_ip, ctx.ray_port),
        "--node-ip-address".into(),
        spec.host_ip.clone(),
        "--object-store-memory".into(),
        ctx.object_store.clone(),
        "--disable-usage-stats".into(),
        "--block".into(),
    ]
}
pub fn ray_status_argv(ctx: &Ctx, spec: &NodeSpec) -> Vec<String> {
    vec![format!("{}/bin/ray", venv_dir(&spec.root)), "status".into(), "--address".into(), format!("{}:{}", spec.host_ip, ctx.ray_port)]
}
pub fn serve_argv(ctx: &Ctx) -> Vec<String> {
    vec!["bash".into(), ctx.repo.join(&ctx.triplet["sh"]).to_string_lossy().into_owned()]
}

/// The dry-run plan of `up` (no side effects). `here` = the scripts directory (where the `.sh` launchers of the gate programs sit).
pub fn plan(c: &Ctx, here: &Path) -> Vec<String> {
    let ph = |role: &str| (format!("<pgid:{role}>"), format!("<starttime:{role}>"));
    let fake_ld: Env = [("LD_LIBRARY_PATH".to_string(), "<verify.env.LD_LIBRARY_PATH>".to_string())].into();
    let rp = c.repo.to_string_lossy().into_owned();
    let (m, s) = (&c.main, &c.sub);
    let mut lines = vec![
        format!("# native up 계획(dry-run · 부수효과 0) cell={} run={} source={} campaign={}", c.cell, c.run_id, c.source_cell, c.campaign_id),
        format!("#   run root={} (양 노드 · 상한 {}GiB/노드) · 짧은 루트={}", m.root, c.max_bytes >> 30, m.short),
        format!("#   ready_max_s={} overhead_mib={} serve_port={} ray_port={} image={}", c.ready_max_s, c.overhead_mib, c.serve_port, c.ray_port, c.image),
    ];
    let add = |lines: &mut Vec<String>, node: &str, argv: Vec<String>, note: &str| {
        lines.push(format!("[{node}] {}", shlex_join(&argv)) + &if note.is_empty() { String::new() } else { format!("   # {note}") });
    };
    for (n, spec) in [("main", m), ("sub", s)] {
        for p in [&spec.session_py, &spec.renew_sh, &spec.watchdog_sh] {
            add(&mut lines, n, sv(&["test", "-f", p]), "도구 실재");
        }
        for p in [&spec.renew_sh, &spec.watchdog_sh] {
            add(&mut lines, n, sv(&["grep", "-q", "--", "--pgid", p]), "pgid 모드 배달");
        }
    }
    let h = here.to_string_lossy();
    let yaml_p = c.repo.join(&c.triplet["yaml"]).to_string_lossy().into_owned();
    add(
        &mut lines,
        "main",
        sv(&[&format!("{h}/check_smoke_model.sh"), &c.source_cell, "--repo", &rp, "--topology", TOPOLOGY, "--emit-gate-params"]),
        "RAM 게이트(원본 Docker 셀 = 같은 모델·TP·KV)",
    );
    add(&mut lines, "sub", sv(&["cat", "/proc/meminfo"]), "동일 문턱 RAM 게이트(부족 시 vllm-drop-caches 1회)");
    add(
        &mut lines,
        "main",
        sv(&[&format!("{h}/budget_preflight.sh"), "--json", "--declared-gmu-yaml", &yaml_p, "--mem-total-mib", "<MemTotal>", "--weights-mib", "<(ckpt−ple)÷tp>", "--kv-mib", "<kv_mib>", "--overhead-mib", &c.overhead_mib.to_string()]),
        "선판정",
    );
    for (n, spec) in [("main", m), ("sub", s)] {
        add(
            &mut lines,
            n,
            sv(&[session_interpreter(&spec.session_py), &spec.session_py, "--node-dir", &spec.node_dir, "declare-budget", "--mem-total-mib", "<MemTotal>", "--weights-mib", "<w>", "--kv-mib", "<kv>", "--overhead-mib", &c.overhead_mib.to_string(), "--ttl-s", "<max(3×ready,floor)>", "--expected-load-s", &c.ready_max_s.to_string(), "--label", &format!("smoke-{}", c.cell), "--now", "<UTC>"]),
            "clear→declare→budget_honored 관측",
        );
    }
    for (n, spec) in [("main", m), ("sub", s)] {
        lines.push(format!("[{n}] <init marker-owned root> {} · {}", spec.root, spec.short));
        add(&mut lines, n, mkdirs_argv(spec), "");
    }
    lines.push(format!("[sub] <put> {} ← {} (메인 ELF · sha256 기록)", s.wheel_tool, WHEEL_ELF_REL));
    lines.push(format!("[sub] <put> {} ← {} (sha256 기록)", s.wheel_accept, c.wheel_accept_rel));
    for (n, spec) in [("main", m), ("sub", s)] {
        add(&mut lines, n, wheelhouse_build_argv(&spec.wheel_tool, &c.image, &spec.root, "<generated-utc>", &spec.wheel_accept), "자기 이미지에서 재포장 · 승인 선언 적용");
        add(&mut lines, n, sv(&["du", "-sxb", &spec.root]), "상한 검사");
        add(&mut lines, n, wheelhouse_verify_argv(&spec.wheel_tool, &spec.root, "<generated-utc>", &spec.wheel_accept), "venv·pip --no-index --no-deps·이미지 동등성·ldd·import+CUDA → env 레시피");
        add(&mut lines, n, sv(&["du", "-sxb", &spec.root]), "상한 검사");
        add(&mut lines, n, pip_freeze_argv(&spec.root), &format!("→ docs/simlog/<YYMMDDHH>_{}/pip-freeze-{n}.txt", c.simlog_topic));
    }
    let (hp, hs) = ph("ray-head");
    add(&mut lines, "main", env_argv(&runtime_env(c, m, &fake_ld), &ray_head_argv(c, m)), "start(새 세션) → identity");
    add(&mut lines, "main", watchdog_argv(m, &hp, &hs), "start");
    let (wp, ws) = ph("ray-worker");
    add(&mut lines, "sub", env_argv(&runtime_env(c, s, &fake_ld), &ray_worker_argv(c, s)), "start(새 세션) → identity");
    add(&mut lines, "sub", watchdog_argv(s, &wp, &ws), "start");
    add(&mut lines, "main", ray_status_argv(c, m), &format!("GPU 2 합류 대기 ≤{RAY_JOIN_MAX_S}s"));
    let mut env = runtime_env(c, m, &fake_ld);
    env.insert("RAY_ADDRESS".into(), format!("{}:{}", m.host_ip, c.ray_port));
    add(&mut lines, "main", env_argv(&env, &serve_argv(c)), "start → identity");
    let (sp, ss) = ph("vllm-serve");
    add(&mut lines, "main", watchdog_argv(m, &sp, &ss), "start");
    lines.push(format!("[main] GET http://127.0.0.1:{}/health 매 {HEALTH_INTERVAL_S}s ≤ {}s → POST /v1/chat/completions 1회", c.serve_port, c.ready_max_s));
    lines.push(format!("[main] → docs/simlog/<YYMMDDHH>_{}/{NATIVE_LAUNCH_FILE} (기동 기록 · 실행 argv·env 치환본 → proof native_launch_path)", c.simlog_topic));
    lines.push(format!("[main] → {} (native_multinode_serve_proof)", c.proof_rel));
    add(&mut lines, "main", renew_argv(m, &sp, &ss, 0), "start · ttl=<ttl>");
    add(&mut lines, "sub", renew_argv(s, &wp, &ws, 0), "start · ttl=<ttl>");
    lines.push(format!("[main] → {}/state.json · exit 0(서빙 유지) · 실패 시 자동 down", m.root));
    lines
}

// ── down ───────────────────────────────────────────────────────────────────────────────────────────

pub struct Runners {
    pub main: Box<dyn Runner>,
    pub sub: Box<dyn Runner>,
}
impl Runners {
    pub(crate) fn get(&self, n: &str) -> &dyn Runner {
        if n == "main" {
            self.main.as_ref()
        } else {
            self.sub.as_ref()
        }
    }
}
fn write_atomic(p: &Path, text: &str) -> std::io::Result<()> {
    let tmp = p.with_file_name(format!("{}.tmp", p.file_name().and_then(|n| n.to_str()).unwrap_or("x")));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, p)
}
fn str_list(v: &[String]) -> PyValue {
    PyValue::List(v.iter().map(|s| vs(s)).collect())
}

#[derive(Default)]
struct NodeOut {
    root_absent: bool,
    owned_processes: Vec<ProcRow>,
    owned_gpu: Vec<String>,
    unknown: Vec<String>,
}

pub fn down_from_state(state: &PyValue, repo: &Path, r: &Runners, rs: &Runners, clock: &dyn Clock, log: &dyn Fn(&str)) -> PyValue {
    let run_id = as_str(dget(state, "run_id")).unwrap_or_default();
    let cell = as_str(dget(state, "cell")).unwrap_or_default();
    let mut errors: Vec<String> = Vec::new();
    let mut nodes: BTreeMap<&str, NodeOut> = [("main", NodeOut::default()), ("sub", NodeOut::default())].into();
    let mut idents: Vec<(String, Identity)> = Vec::new();
    for row in list_items(dget(state, "identities")) {
        match Identity::from_row(row) {
            Ok(i) => idents.push((as_str(dget(row, "kind")).unwrap_or_default(), i)),
            Err(e) => errors.push(e.msg().into()),
        }
    }
    let mut cleanup: Vec<PyValue> = Vec::new();
    // ① disarm
    for (kind, i) in &idents {
        if kind == "watchdog" || kind == "renew" {
            match r.get(&i.node).stop(i, 10, false) {
                Ok(v) => cleanup.push(v),
                Err(e) => {
                    nodes.get_mut(i.node.as_str()).map(|n| n.unknown.push(format!("{}: {}", i.role, e.msg())));
                    errors.push(e.msg().into());
                }
            }
        }
    }
    let serve_ids: Vec<&Identity> = idents.iter().filter(|(k, _)| k == "serve").map(|(_, i)| i).collect();
    // ② snapshot
    let mut snap: BTreeMap<&str, BTreeMap<i64, String>> = [("main", BTreeMap::new()), ("sub", BTreeMap::new())].into();
    for n in ["main", "sub"] {
        let mine: Vec<&&Identity> = serve_ids.iter().filter(|i| i.node == n).collect();
        if mine.is_empty() {
            continue;
        }
        match r.get(n).tree(&mine.iter().map(|i| i.pid).collect::<Vec<_>>(), &mine.iter().map(|i| i.pgid).collect::<Vec<_>>()) {
            Ok(rows) => {
                for row in rows {
                    snap.get_mut(n).unwrap().insert(row.pid, row.starttime);
                }
            }
            Err(e) => {
                nodes.get_mut(n).unwrap().unknown.push(format!("스냅숏 조회 실패: {}", e.msg()));
                errors.push(e.msg().into());
            }
        }
    }
    // ③ leaders, reverse order
    for i in serve_ids.iter().rev() {
        match r.get(&i.node).stop(i, STOP_WAIT_S, false) {
            Ok(v) => cleanup.push(v),
            Err(e) => {
                if let Some(n) = nodes.get_mut(i.node.as_str()) {
                    n.unknown.push(format!("{}: {}", i.role, e.msg()));
                }
                errors.push(e.msg().into());
            }
        }
    }
    // ④ descendants outside the group
    for n in ["main", "sub"] {
        let leaders: BTreeSet<i64> = serve_ids.iter().filter(|i| i.node == n).map(|i| i.pid).collect();
        for (pid, st) in snap[n].clone() {
            if leaders.contains(&pid) {
                continue;
            }
            let res = (|| -> R<()> {
                let alive = r.get(n).tree(&[pid], &[])?;
                if alive.iter().any(|x| x.pid == pid && x.starttime == st) {
                    let d = Identity { node: n.into(), role: format!("descendant-{pid}"), pid, pgid: pid, starttime: st.clone(), log_rel: String::new() };
                    cleanup.push(r.get(n).stop(&d, 10, true)?);
                }
                Ok(())
            })();
            if let Err(e) = res {
                nodes.get_mut(n).unwrap().unknown.push(format!("자손 {pid}: {}", e.msg()));
                errors.push(e.msg().into());
            }
        }
    }
    // ⑤ preserve evidence before deleting
    let mut preserved = true;
    let preserve_rel = as_str(dget(state, "preserve_rel")).unwrap_or_default();
    let pdir = repo.join(&preserve_rel);
    let pres = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(&pdir)?;
        let (mut sub, mut k) = ("logs".to_string(), 2);
        while pdir.join(&sub).exists() {
            sub = format!("logs.attempt{k}");
            k += 1;
        }
        std::fs::create_dir(pdir.join(&sub))?;
        std::fs::write(pdir.join(&sub).join("state.json"), dumps_pretty(state))?;
        for (_, i) in &idents {
            match r.get(&i.node).text(&i.log_rel) {
                Ok(text) => {
                    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(pdir.join(&sub).join(format!("{}-{}.log", i.node, i.role)))?;
                    f.write_all(text.as_bytes())?;
                }
                Err(e) => {
                    preserved = false;
                    errors.push(format!("로그 보존 실패 {}/{}: {}", i.node, i.role, e.msg()));
                }
            }
        }
        let mut rels: Vec<String> = ["native_install_rel", "native_launch_rel"].iter().filter_map(|k| as_str(dget(state, k))).collect();
        if let Some(PyValue::Dict(d)) = dget(state, "pip_freeze_rel") {
            rels.extend(d.iter().filter_map(|(_, v)| as_str(Some(v))));
        }
        for rel in rels {
            if !rel.is_empty() && !repo.join(&rel).is_file() {
                preserved = false;
                errors.push(format!("보존 증거 부재: {rel}"));
            }
        }
        Ok(())
    })();
    if let Err(e) = pres {
        preserved = false;
        errors.push(format!("보존 디렉터리 실패: {e}"));
    }
    // ⑥ fresh residue
    for n in ["main", "sub"] {
        let mine: Vec<&&Identity> = serve_ids.iter().filter(|i| i.node == n).collect();
        let pgids: BTreeSet<i64> = mine.iter().map(|i| i.pgid).collect();
        let rows = if mine.is_empty() { Ok(vec![]) } else { r.get(n).tree(&mine.iter().map(|i| i.pid).collect::<Vec<_>>(), &mine.iter().map(|i| i.pgid).collect::<Vec<_>>()) };
        match rows {
            Ok(rows) => {
                let owned: Vec<ProcRow> = rows.into_iter().filter(|x| snap[n].get(&x.pid) == Some(&x.starttime) || pgids.contains(&x.pgid)).collect();
                if !owned.is_empty() {
                    errors.push(format!("{n}: owned process residue {:?}", owned.iter().map(|x| x.pid).collect::<Vec<_>>()));
                }
                nodes.get_mut(n).unwrap().owned_processes = owned;
            }
            Err(e) => {
                nodes.get_mut(n).unwrap().unknown.push(format!("잔재 조회 실패: {}", e.msg()));
                errors.push(e.msg().into());
            }
        }
        match r.get(n).gpu() {
            Ok(gpu) => {
                let pids: BTreeSet<i64> = snap[n].keys().copied().chain(mine.iter().map(|i| i.pid)).collect();
                let no = nodes.get_mut(n).unwrap();
                for line in gpu {
                    let head = line.split(',').next().unwrap_or("").trim().to_string();
                    if !head.is_empty() && head.bytes().all(|c| c.is_ascii_digit()) && head.parse().is_ok_and(|p| pids.contains(&p)) {
                        no.owned_gpu.push(line);
                    } else {
                        no.unknown.push(format!("귀속 불명 GPU compute app: {line}"));
                    }
                }
                if !no.owned_gpu.is_empty() {
                    errors.push(format!("{n}: owned GPU residue"));
                }
            }
            Err(e) => {
                nodes.get_mut(n).unwrap().unknown.push(format!("GPU 조회 실패: {}", e.msg()));
                errors.push(e.msg().into());
            }
        }
        if n == "main" {
            match r.get(n).ports() {
                Ok(listening) => {
                    for label in ["serve", "ray"] {
                        if let Some(p) = as_i64(dpath(state, &["ports", label])) {
                            if listening.contains(&p) {
                                nodes.get_mut(n).unwrap().unknown.push(format!("포트 {p}({label}) 가 아직 listen"));
                            }
                        }
                    }
                }
                Err(e) => {
                    nodes.get_mut(n).unwrap().unknown.push(format!("포트 조회 실패: {}", e.msg()));
                    errors.push(e.msg().into());
                }
            }
        }
    }
    // ⑦ remove owned roots of clean nodes only
    for n in ["main", "sub"] {
        let clean = {
            let no = &nodes[n];
            no.owned_processes.is_empty() && no.owned_gpu.is_empty() && no.unknown.is_empty()
        };
        if !clean || !preserved {
            errors.push(format!("{n}: 잔재·미확정·증거 미보존 — 소유 루트를 남긴다(재시도: down)"));
            continue;
        }
        let created: Vec<String> = list_items(dpath(state, &["roots_created", n])).into_iter().filter_map(|v| as_str(Some(v))).collect();
        let res = (|| -> R<()> {
            for (kind, runner) in [("short", rs.get(n)), ("run", r.get(n))] {
                if created.iter().any(|c| c == kind) && runner.exists()? {
                    runner.remove_owned_tree(&run_id)?;
                }
            }
            let absent = !rs.get(n).exists()? && !r.get(n).exists()?;
            nodes.get_mut(n).unwrap().root_absent = absent;
            if !absent {
                errors.push(format!("{n}: 삭제 뒤에도 루트가 남았다"));
            }
            Ok(())
        })();
        if let Err(e) = res {
            nodes.get_mut(n).unwrap().unknown.push(format!("루트 삭제 실패: {}", e.msg()));
            errors.push(e.msg().into());
        }
    }
    // ⑧ budget declaration and page cache
    for n in ["main", "sub"] {
        if truthy(dpath(state, &["budget_declared", n])) {
            let sp = as_str(dpath(state, &["nodes", n, "session_py"])).unwrap_or_default();
            let nd = as_str(dpath(state, &["nodes", n, "node_dir"])).unwrap_or_default();
            if let Err(e) = r.get(n).run(&[session_interpreter(&sp).into(), session_launcher(&sp), "--node-dir".into(), nd, "clear-budget".into(), "--now".into(), utc(clock.now())], 30, true) {
                errors.push(format!("{n}: clear-budget 실패: {}", e.msg()));
            }
        }
        let dc = "/usr/local/sbin/vllm-drop-caches".to_string();
        if r.get(n).run(&["test".into(), "-x".into(), dc.clone()], 15, false).is_ok_and(|x| x.rc == 0) {
            let _ = r.get(n).run(&["sudo".into(), "-n".into(), dc], 60, false);
        }
    }
    let ok = errors.is_empty() && preserved && nodes.values().all(|v| v.root_absent && v.owned_processes.is_empty() && v.owned_gpu.is_empty() && v.unknown.is_empty());
    let node_vals: Vec<(&str, PyValue)> = nodes
        .iter()
        .map(|(k, v)| {
            (
                *k,
                dict(vec![
                    ("root_absent", PyValue::Bool(v.root_absent)),
                    ("owned_processes", PyValue::List(v.owned_processes.iter().map(ProcRow::value).collect())),
                    ("owned_gpu_processes", str_list(&v.owned_gpu)),
                    ("unknown", str_list(&v.unknown)),
                ]),
            )
        })
        .collect();
    let status = if ok { "PASS" } else { "FAIL_CLOSED" };
    let att = dict(vec![
        ("schema_version", vi(1)),
        ("kind", vs("native_multinode_cleanup_attestation")),
        ("plane", vs("native")),
        ("run_id", vs(&run_id)),
        ("cell", vs(&cell)),
        ("status", vs(status)),
        ("nodes", dict(node_vals)),
        ("cleanup", PyValue::List(cleanup)),
        ("errors", str_list(&errors)),
        ("evidence_preserved", PyValue::Bool(preserved)),
        ("preserved", dict(vec![("evidence_root", vs(&preserve_rel))])),
        ("attested_utc", vs(&utc(clock.now()))),
        ("provenance", vs("measured(PID+starttime 정지 · 신선한 tree/GPU/port 조회 · 루트 부재 조회)")),
    ]);
    let att_rel = as_str(dget(state, "attestation_rel")).unwrap_or_default();
    let p = repo.join(&att_rel);
    if let Some(par) = p.parent() {
        let _ = std::fs::create_dir_all(par);
    }
    if let Err(e) = write_atomic(&p, &dumps_pretty(&att)) {
        errors.push(format!("attestation 쓰기 실패: {e}"));
    }
    log(&format!("[native] cleanup attestation → {att_rel} ({status}{})", if errors.is_empty() { String::new() } else { format!(" · errors={}", errors.len()) }));
    att
}

pub fn runners_for(state: &PyValue, sub_host: &str) -> R<(Runners, Runners)> {
    let root = |n: &str, k: &str| as_str(dpath(state, &["nodes", n, k])).ok_or_else(|| Err::Serve(format!("state.nodes.{n}.{k} 가 없다")));
    let (mr, ms, sr, ss) = (root("main", "root")?, root("main", "short")?, root("sub", "root")?, root("sub", "short")?);
    let sub_wd = root("sub", "work_dir")?;
    Ok((
        Runners { main: Box::new(Local::new(&mr, "main")), sub: Box::new(Ssh::new(sub_host, &sr, "sub", &sub_wd)?) },
        Runners { main: Box::new(Local::new(&ms, "main")), sub: Box::new(Ssh::new(sub_host, &ss, "sub", &sub_wd)?) },
    ))
}

fn say(m: &str) {
    gate_cli::write_out(&format!("{m}\n"));
}
fn say_err(m: &str) {
    gate_cli::write_err(&format!("{m}\n"));
}

pub fn down_cli(repo: &Path, cell: &str, run_id: &str, apply: bool, base: Option<&str>) -> i32 {
    if !is_run_id(run_id) {
        say_err("FAIL: unsafe run-id");
        return EXIT_USAGE;
    }
    let base = match base {
        Some(b) => b.to_string(),
        None => {
            let res = (|| -> R<Option<String>> {
                let active = String::from_utf8_lossy(&std::fs::read(repo.join("campaigns/ACTIVE")).map_err(|e| Err::Serve(e.to_string()))?).trim().to_string();
                let camp = load_yaml(&repo.join("campaigns").join(&active).join("campaign.yaml"))?;
                Ok(as_str(dpath(&camp, &["topology_sections", TOPOLOGY, "native_run_root_base"])))
            })();
            match res {
                Ok(Some(b)) => b,
                Ok(None) => "None".into(),
                Err(e) => {
                    say_err(&format!("FAIL: {}", e.msg()));
                    return EXIT_FAIL;
                }
            }
        }
    };
    let root = match validate_remote_root(&format!("{}/{run_id}", base.trim_end_matches('/')), run_id) {
        Ok(r) => r,
        Err(e) => {
            say_err(&format!("FAIL: {}", e.msg()));
            return EXIT_FAIL;
        }
    };
    let state_path = Path::new(&root).join("state.json");
    if !state_path.is_file() {
        let proof = repo.join(format!("output/{TOPOLOGY}/benchlog/serve_proof_{cell}.json"));
        let done = (|| -> Option<bool> {
            let doc = agent_runtime_support::pyjson::loads(&String::from_utf8_lossy(&std::fs::read(&proof).ok()?)).ok()?;
            let att_rel = as_str(dget(&doc, "cleanup_attestation_path"))?;
            let att = agent_runtime_support::pyjson::loads(&String::from_utf8_lossy(&std::fs::read(repo.join(att_rel)).ok()?)).ok()?;
            Some(as_str(dget(&doc, "run_id")).as_deref() == Some(run_id) && as_str(dget(&att, "run_id")).as_deref() == Some(run_id) && as_str(dget(&att, "status")).as_deref() == Some("PASS"))
        })();
        if done == Some(true) {
            say(&format!("[native] down: run {run_id} 은 이미 정리됐다(attestation PASS) — no-op"));
            return EXIT_OK;
        }
        say_err(&format!("FAIL_CLOSED_UNKNOWN: {} 없음 — 소유 정체를 모르면 아무것도 정지·삭제하지 않는다", state_path.display()));
        return EXIT_UNKNOWN;
    }
    let state = match std::fs::read(&state_path).ok().and_then(|b| agent_runtime_support::pyjson::loads(&String::from_utf8_lossy(&b)).ok()) {
        Some(s) => s,
        None => {
            say_err("FAIL: state.json 을 읽을 수 없다");
            return EXIT_FAIL;
        }
    };
    if as_str(dget(&state, "run_id")).as_deref() != Some(run_id) || as_str(dget(&state, "cell")).as_deref() != Some(cell) {
        say_err("FAIL: state.json 의 run_id/cell 이 인자와 다르다");
        return EXIT_FAIL;
    }
    if !apply {
        say(&format!("[native] down 계획(dry-run) run={run_id} cell={cell}"));
        for row in list_items(dget(&state, "identities")) {
            say(&format!(
                "[{}] stop pid={} pgid={} starttime={} role={} kind={}",
                py_str(dget(row, "node")),
                py_str(dget(row, "pid")),
                py_str(dget(row, "pgid")),
                py_str(dget(row, "starttime")),
                py_str(dget(row, "role")),
                py_str(dget(row, "kind"))
            ));
        }
        say(&format!(
            "[*] 보존 → {}/logs · 잔재 조회 → 루트 삭제 {} · {} (양 노드) · 예산 회수 · attestation → {}",
            py_str(dget(&state, "preserve_rel")),
            py_str(dpath(&state, &["nodes", "main", "root"])),
            py_str(dpath(&state, &["nodes", "main", "short"])),
            py_str(dget(&state, "attestation_rel"))
        ));
        return EXIT_OK;
    }
    let sub_host = as_str(dget(&state, "sub_host")).unwrap_or_default();
    let (r, rs) = match runners_for(&state, &sub_host) {
        Ok(x) => x,
        Err(e) => {
            say_err(&format!("FAIL: {}", e.msg()));
            return EXIT_FAIL;
        }
    };
    let att = down_from_state(&state, repo, &r, &rs, &RealClock, &|m| say(m));
    if as_str(dget(&att, "status")).as_deref() == Some("PASS") {
        EXIT_OK
    } else {
        EXIT_UNKNOWN
    }
}

// ── command line ───────────────────────────────────────────────────────────────────────────────────

fn opt(flags: &'static [&'static str], dest: &'static str, kind: Kind, required: bool) -> Opt {
    Opt { flags, dest, kind, help: None, required, group: None }
}
fn spec_for(op: &'static str) -> Spec {
    let prog: &'static str = match op {
        "up" => "native_multinode_serve.py up",
        "down" => "native_multinode_serve.py down",
        _ => "native_multinode_serve.py client",
    };
    let mut options = vec![
        opt(&["-h", "--help"], "help", Kind::Help, false),
        opt(&["--cell"], "cell", Kind::Text, true),
        opt(&["--run-id"], "run_id", Kind::Text, true),
        opt(&["--apply"], "apply", Kind::Flag, false),
        opt(&["--repo"], "repo", Kind::Text, false),
    ];
    match op {
        "up" => options.extend([
            opt(&["--source-cell"], "source_cell", Kind::Text, false),
            opt(&["--ready-max-seconds"], "ready_max_seconds", Kind::Int, false),
            opt(&["--simlog-topic"], "simlog_topic", Kind::Text, false),
        ]),
        "client" => options.push(opt(&["--source-cell"], "source_cell", Kind::Text, false)),
        _ => options.push(opt(&["--run-root-base"], "run_root_base", Kind::Text, false)),
    }
    Spec { prog, description: "native 2노드 Ray/vLLM 서빙 정문(up/down)", options, group_required: false, positional: None }
}

pub fn run(script: Option<&PyStr>, args: &[PyStr]) -> i32 {
    let first = args.first().and_then(PyStr::to_utf8).unwrap_or_default();
    if first == "--self-test" {
        return self_test::self_test(&gate_cli::scripts_dir(script, "native_multinode_serve"));
    }
    if first == "-h" || first == "--help" {
        // the original's top-level argparse page, captured from it (golden text)
        gate_cli::write_out(include_str!("help/native_multinode_serve.txt"));
        return 0;
    }
    let op: &'static str = match first.as_str() {
        "up" => "up",
        "down" => "down",
        "client" => "client",
        _ => {
            say_err("usage: native_multinode_serve.py [-h] [--self-test] {up,down,client} ...");
            return EXIT_USAGE;
        }
    };
    let values: Values = match gate_cli::parse_args(&spec_for(op), &args[1..]) {
        Ok(v) => v,
        Err(status) => return status,
    };
    let here = gate_cli::scripts_dir(script, "native_multinode_serve");
    let repo: PathBuf = match gate_cli::text(&values, "repo") {
        Some(r) => resolve_nonstrict(Path::new(&r), 0),
        None => here.ancestors().nth(4).map(Path::to_path_buf).unwrap_or_else(|| here.clone()),
    };
    let cell = gate_cli::text(&values, "cell").unwrap_or_default();
    let run_id = gate_cli::text(&values, "run_id").unwrap_or_default();
    let apply = gate_cli::flag(&values, "apply");
    let res: R<i32> = (|| {
        if op == "down" {
            return Ok(down_cli(&repo, &cell, &run_id, apply, gate_cli::text(&values, "run_root_base").as_deref()));
        }
        if op == "client" {
            let ctx = load_ctx(&repo, &cell, &run_id, gate_cli::text(&values, "source_cell").as_deref(), None, "native_N1")?;
            let st_text = std::fs::read(Path::new(&ctx.main.root).join("state.json")).map_err(|e| Err::Serve(e.to_string()))?;
            let st = agent_runtime_support::pyjson::loads(&String::from_utf8_lossy(&st_text)).map_err(|_| Err::Serve("state.json 을 읽을 수 없다".into()))?;
            let lds = dpath(&st, &["runtime_env_recipe", "main"]);
            let recipe: Env = match lds.map(PyValue::peel) {
                Some(PyValue::Dict(d)) => d.iter().map(|(k, v)| (k.to_utf8().unwrap_or_default(), py_str(Some(v)))).collect(),
                _ => Env::new(),
            };
            if !recipe.get("LD_LIBRARY_PATH").is_some_and(|v| !v.is_empty()) {
                return serve("state.json 에 main 런타임 레시피가 없다");
            }
            let dst = Path::new(&ctx.main.root).join(CLIENT_REL);
            if !apply {
                say(&dst.to_string_lossy());
                return Ok(EXIT_OK);
            }
            if let Some(par) = dst.parent() {
                std::fs::create_dir_all(par).map_err(|e| Err::Serve(e.to_string()))?;
            }
            std::fs::write(&dst, client_wrapper_text(&runtime_env(&ctx, &ctx.main, &recipe), &ctx.main)).map_err(|e| Err::Serve(e.to_string()))?;
            std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).map_err(|e| Err::Serve(e.to_string()))?;
            say(&dst.to_string_lossy());
            return Ok(EXIT_OK);
        }
        let ctx = load_ctx(&repo, &cell, &run_id, gate_cli::text(&values, "source_cell").as_deref(), gate_cli::int(&values, "ready_max_seconds"), &gate_cli::text(&values, "simlog_topic").unwrap_or_else(|| "native_N1".into()))?;
        if !apply {
            say(&plan(&ctx, &here).join("\n"));
            return Ok(EXIT_OK);
        }
        let (r, rs) = (
            Runners { main: Box::new(Local::new(&ctx.main.root, "main")), sub: Box::new(Ssh::new(&ctx.sub_host, &ctx.sub.root, "sub", &ctx.sub.work_dir)?) },
            Runners { main: Box::new(Local::new(&ctx.main.short, "main")), sub: Box::new(Ssh::new(&ctx.sub_host, &ctx.sub.short, "sub", &ctx.sub.work_dir)?) },
        );
        let log = |m: &str| say(m);
        let mut ns = up::NativeServe::new(&ctx, &r, &rs, &up::real_http, &RealClock, &log, here.clone());
        Ok(ns.up(&utc(RealClock.now())))
    })();
    match res {
        Ok(s) => s,
        Err(Err::Unknown(m)) => {
            say_err(&format!("FAIL_CLOSED_UNKNOWN: {m}"));
            EXIT_UNKNOWN
        }
        Err(Err::Serve(m)) => {
            say_err(&format!("FAIL: {m}"));
            EXIT_FAIL
        }
    }
}

/// The interpreter of a node's session tool: always `bash` — the tool is the Rust launcher `blackbox_session.sh` on both
/// nodes (main `scripts/node_blackbox/`, Sub `.claude/runtime/node_blackbox/` delivered by render_sub_env · plan_26111615
/// pass 3: no Python on the Sub).
pub fn session_interpreter(_session_tool: &str) -> &'static str {
    "bash"
}

/// A `session_py` recorded by an older run state names the original `.py`; its launcher is the sibling `.sh`.
pub fn session_launcher(session_tool: &str) -> String {
    match session_tool.strip_suffix(".py") {
        Some(stem) => format!("{stem}.sh"),
        None => session_tool.to_string(),
    }
}
