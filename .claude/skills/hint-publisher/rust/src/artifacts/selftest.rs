//! `artifacts.selftest()` — port of `scripts/hintlib/artifacts.py` 5362–7644 (`_FakeTierRender` · `_fake_docker` ·
//! `_fake_image_docker` · `_set_mtime` · `selftest`). Isolated temporary repository · no live tags/branches/campaigns/docker.
//!
//! Translations (Rust has no meaning for the Python form; the closest Rust fact is checked instead):
//! - `render_module=fake_rd` (`_FakeTierRender`): the Rust `collect`/`sub_recipe` judge env tiers with the real renderer
//!   (`render_dockerfile.sh --env-tier`); `SHARED_ASSET_DIR` of the fake equals the real one in the fixture (the copied
//!   `assets/configs`), so the real module is used. Only `env_shape(render_module=fake_rd)` takes the fake (as its J form).
//! - the owner modules are copied with their native launchers (`slave_forward.sh` · `render_dockerfile.sh`) and the
//!   upstream runtime `target/` is linked from the own checkout (the Rust port runs the owner's launcher, never its .py).
//! - `crash_on_cp` (a runner raising RuntimeError): a Rust `DockerRunner` cannot raise — the cp call fails with
//!   `RunFail::Timeout` instead and the check asserts the container is still removed.
//! - `missing.append("…")` scan of `artifacts.py`: scans the Rust sources of this module for `missing.push("…")`.
//! - `_render_module(repo).RUNNER_SCRIPTS` / `LOCKSET_KNOB_SOURCES`: the literal is read from the owner's Rust source text (never a `.py`).
//! - an unexpected `HintError` (Python: an uncaught exception) ends the section and is reported as a failure line.
#![allow(non_snake_case, dead_code, unused_imports, clippy::too_many_lines)]
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::*;
use crate::core::{self, HintError, Proc, HR};
use crate::pyre::Re;
use crate::value::{Map, J};
use crate::jl;

// ── 작은 도구 ─────────────────────────────────────────────────────────────────────────────────────
struct St {
    bad: Vec<String>,
    n: usize,
}

impl St {
    fn ck(&mut self, name: &str, cond: bool) {
        self.n += 1;
        if !cond {
            self.bad.push(format!("artifacts: {name}"));
        }
    }
    fn raises<T>(&mut self, name: &str, code: &str, f: impl FnOnce() -> HR<T>) {
        match f() {
            Ok(_) => self.ck(&format!("{name}(차단 안 됨)"), false),
            Err(e) => self.ck(&format!("{name}({})", e.code), e.code == code),
        }
    }
}

fn p(s: &str) -> J {
    crate::value::loads(s).unwrap_or_else(|e| panic!("fixture json: {e}: {s}"))
}
fn js(j: &J) -> Option<&str> {
    j.as_str()
}
fn eqs(j: &J, s: &str) -> bool {
    j.as_str() == Some(s)
}
fn sw(j: &J, pre: &str) -> bool {
    j.as_str().is_some_and(|s| s.starts_with(pre))
}
fn ew(j: &J, suf: &str) -> bool {
    j.as_str().is_some_and(|s| s.ends_with(suf))
}
/// Python `needle in j` (str → substring · list → element · dict → key).
fn has(j: &J, needle: &str) -> bool {
    match j {
        J::Str(s) => s.contains(needle),
        J::List(l) => l.iter().any(|x| x.as_str() == Some(needle)),
        J::Dict(m) => m.contains_key(needle),
        _ => false,
    }
}
/// `str(x)` for strings, `""` otherwise (`.get(k, "")` reads).
fn sv(j: &J) -> String {
    j.as_str().map(str::to_string).unwrap_or_default()
}
fn is_true(j: &J) -> bool {
    j.as_bool() == Some(true)
}
fn is_false(j: &J) -> bool {
    j.as_bool() == Some(false)
}
/// Python `==` of JSON-like values (dict order ignored).
fn jeq(a: &J, b: &J) -> bool {
    match (a, b) {
        (J::Dict(x), J::Dict(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| jeq(v, w))),
        (J::List(x), J::List(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|(v, w)| jeq(v, w)),
        _ => {
            if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
                if !a.is_bool() && !b.is_bool() {
                    return x == y;
                }
            }
            a == b
        }
    }
}
fn strs(j: &J) -> Vec<String> {
    j.items().iter().map(J::display).collect()
}
fn sset(j: &J) -> BTreeSet<String> {
    j.items().iter().map(J::display).collect()
}
fn set_of(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn keys(j: &J) -> BTreeSet<String> {
    j.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default()
}
/// `{r[key]: r for r in rows}`.
fn by(rows: &J, key: &str) -> J {
    let mut m = Map::new();
    for r in rows.items() {
        m.insert(r[key].display(), r.clone());
    }
    J::Dict(m)
}
/// `dict(base, k=v, ...)`.
fn with(base: &J, kv: Vec<(&str, J)>) -> J {
    let mut out = base.clone();
    if !out.is_dict() {
        out = J::dict();
    }
    for (k, v) in kv {
        out.set(k, v);
    }
    out
}
fn without(base: &J, key: &str) -> J {
    let mut out = base.clone();
    if let Some(m) = out.as_dict_mut() {
        m.remove(key);
    }
    out
}
fn dumps(j: &J) -> String {
    crate::value::dumps_default(j)
}
fn ioe(e: std::io::Error, p: &Path) -> HintError {
    core::io_err(&e, p)
}
fn wt(path: &Path, text: &str) -> HR<()> {
    std::fs::write(path, text).map_err(|e| ioe(e, path))
}
fn wb(path: &Path, data: &[u8]) -> HR<()> {
    std::fs::write(path, data).map_err(|e| ioe(e, path))
}
fn rt(path: &Path) -> HR<String> {
    core::read_text(path).map_err(|e| HintError::py("OSError", e))
}
fn rb(path: &Path) -> HR<Vec<u8>> {
    std::fs::read(path).map_err(|e| ioe(e, path))
}
fn mkd(path: &Path) -> HR<()> {
    std::fs::create_dir_all(path).map_err(|e| ioe(e, path))
}
fn rm(path: &Path) -> HR<()> {
    std::fs::remove_file(path).map_err(|e| ioe(e, path))
}
fn rm_ok(path: &Path) {
    let _ = std::fs::remove_file(path);
}
fn mv(a: &Path, b: &Path) -> HR<()> {
    std::fs::rename(a, b).map_err(|e| ioe(e, a))
}
fn exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}
fn name_of(p: &str) -> String {
    p.rsplit('/').next().unwrap_or(p).to_string()
}
fn lines(s: &str) -> Vec<&str> {
    s.lines().collect()
}

/// `_set_mtime(path, utc)` — the fixture file's mtime = the injected instant (no wall clock).
fn _set_mtime(path: &Path, utc: &str) -> HR<()> {
    let ts = core::parse_utc(utc)?.epoch();
    set_mtime_secs(path, ts, 0)
}
fn set_mtime_secs(path: &Path, secs: i64, nanos: u32) -> HR<()> {
    let t = if secs >= 0 {
        std::time::UNIX_EPOCH + std::time::Duration::new(secs as u64, nanos)
    } else {
        std::time::UNIX_EPOCH - std::time::Duration::new((-secs) as u64, 0)
    };
    let f = std::fs::OpenOptions::new().write(true).open(path).or_else(|_| std::fs::File::open(path)).map_err(|e| ioe(e, path))?;
    f.set_times(std::fs::FileTimes::new().set_accessed(t).set_modified(t)).map_err(|e| ioe(e, path))
}
fn mtime_of(path: &Path) -> HR<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).map_err(|e| ioe(e, path))
}
fn restore_mtime(path: &Path, t: std::time::SystemTime) -> HR<()> {
    let f = std::fs::OpenOptions::new().write(true).open(path).map_err(|e| ioe(e, path))?;
    f.set_times(std::fs::FileTimes::new().set_accessed(t).set_modified(t)).map_err(|e| ioe(e, path))
}

fn proc_(rc: i32, out: &str, err: &str) -> Proc {
    Proc { returncode: rc, stdout: out.to_string(), stderr: err.to_string() }
}

type Runner = Rc<dyn DockerRunner>;
type Calls = Rc<RefCell<Vec<Vec<String>>>>;

/// `_fake_docker(table)` — prefix table over `argv[1:]` (first match in insertion order) · else rc 1 `no such object`.
fn _fake_docker(table: &[(&[&str], (i32, &str))]) -> Runner {
    let t: Vec<(Vec<String>, i32, String)> =
        table.iter().map(|(k, (rc, out))| (k.iter().map(|s| s.to_string()).collect(), *rc, out.to_string())).collect();
    Rc::new(move |argv: &[String], _t: u64| -> Result<Proc, RunFail> {
        let key = &argv[1..];
        for (pat, rc, out) in &t {
            if key.len() >= pat.len() && key[..pat.len()] == pat[..] {
                return Ok(proc_(*rc, out, ""));
            }
        }
        Ok(proc_(1, "", "no such object"))
    })
}

#[derive(Clone)]
enum FsEnt {
    Dir(Vec<(String, Vec<u8>)>),
    File(Vec<u8>),
}

/// `_fake_image_docker(...)` — one image: inspect(.Id · .Created · .Config.Env) · history(plain · `{{json .}}`) ·
/// create/cp/rm over an in-memory fs · run(ledger cat = absent). `probe` = declares `image_probe`.
struct FakeImage {
    layers: Vec<(String, String)>,
    created: String,
    fs: Vec<(String, FsEnt)>,
    env: Vec<String>,
    probe: bool,
    calls: Calls,
    removed: Rc<RefCell<BTreeSet<String>>>,
    crash_on_cp: bool,
}

impl DockerRunner for FakeImage {
    fn run(&self, argv: &[String], _t: u64) -> Result<Proc, RunFail> {
        let a: Vec<String> = argv[1..].to_vec();
        self.calls.borrow_mut().push(a.clone());
        let cid = "c".repeat(64);
        let fmt = || a.iter().position(|x| x == "--format").and_then(|i| a.get(i + 1)).cloned().unwrap_or_default();
        if a.len() >= 2 && a[0] == "image" && a[1] == "inspect" {
            let f = fmt();
            let out = match f.as_str() {
                "{{.Id}}" => Some("sha256:fx\n".to_string()),
                "{{.Created}}" => Some(format!("{}\n", self.created)),
                "{{json .Config.Env}}" => Some(dumps(&J::from(self.env.clone()))),
                _ => None,
            };
            let ok = out.as_deref().is_some_and(|s| !s.is_empty());
            return Ok(proc_(if ok { 0 } else { 1 }, out.as_deref().unwrap_or(""), ""));
        }
        if a.first().map(String::as_str) == Some("history") {
            let f = fmt();
            if f == "{{json .}}" {
                let mut s = String::new();
                for (by, at) in &self.layers {
                    let mut m = Map::new();
                    m.insert("CreatedBy", by.as_str());
                    m.insert("CreatedAt", at.as_str());
                    m.insert("Comment", "buildkit.dockerfile.v0");
                    m.insert("CreatedSince", "x");
                    s.push_str(&dumps(&J::Dict(m)));
                    s.push('\n');
                }
                return Ok(proc_(0, &s, ""));
            }
            let s: String = self.layers.iter().map(|(by, _)| format!("{by}\n")).collect();
            return Ok(proc_(0, &s, ""));
        }
        if a.first().map(String::as_str) == Some("create") {
            return Ok(proc_(0, &format!("{cid}\n"), ""));
        }
        if a.first().map(String::as_str) == Some("cp") {
            if self.crash_on_cp {
                return Err(RunFail::Timeout); // translation of `raise RuntimeError` (see module doc)
            }
            let src = a[a.len() - 2].split_once(':').map(|x| x.1.to_string()).unwrap_or_default();
            let dest = PathBuf::from(&a[a.len() - 1]);
            match self.fs.iter().find(|(k, _)| *k == src).map(|x| x.1.clone()) {
                Some(FsEnt::Dir(files)) => {
                    let _ = std::fs::create_dir_all(&dest);
                    for (n, b) in files {
                        let _ = std::fs::write(dest.join(n), b);
                    }
                    return Ok(proc_(0, "", ""));
                }
                Some(FsEnt::File(b)) => {
                    let _ = std::fs::write(&dest, b);
                    return Ok(proc_(0, "", ""));
                }
                None => {
                    return Ok(proc_(1, "", &format!("Error response from daemon: Could not find the file {src} in container {cid}")));
                }
            }
        }
        if a.first().map(String::as_str) == Some("rm") {
            let last = a.last().cloned().unwrap_or_default();
            self.removed.borrow_mut().insert(last.clone());
            return Ok(proc_(0, &format!("{last}\n"), ""));
        }
        if a.first().map(String::as_str) == Some("run") {
            return Ok(proc_(1, "", "cat: /opt/easy-vllm/build_ledger.json: No such file"));
        }
        Ok(proc_(1, "", "unexpected docker call"))
    }
    fn image_probe(&self) -> bool {
        self.probe
    }
}

struct ImgOpts {
    fs: Vec<(String, FsEnt)>,
    env: Vec<String>,
    probe: bool,
    calls: Option<Calls>,
    removed: Option<Rc<RefCell<BTreeSet<String>>>>,
    crash_on_cp: bool,
}
impl Default for ImgOpts {
    fn default() -> Self {
        ImgOpts { fs: Vec::new(), env: Vec::new(), probe: true, calls: None, removed: None, crash_on_cp: false }
    }
}
fn _fake_image_docker(layers: &[(String, String)], created: &str, o: ImgOpts) -> Runner {
    Rc::new(FakeImage {
        layers: layers.to_vec(),
        created: created.to_string(),
        fs: o.fs,
        env: o.env,
        probe: o.probe,
        calls: o.calls.unwrap_or_default(),
        removed: o.removed.unwrap_or_default(),
        crash_on_cp: o.crash_on_cp,
    })
}

/// A runner wrapper that declares `image_probe` (Python `fn.image_probe = True/False` on a function).
struct Declared {
    f: Box<dyn Fn(&[String], u64) -> Result<Proc, RunFail>>,
    probe: bool,
}
impl DockerRunner for Declared {
    fn run(&self, argv: &[String], t: u64) -> Result<Proc, RunFail> {
        (self.f)(argv, t)
    }
    fn image_probe(&self) -> bool {
        self.probe
    }
}

// ── 공개 함수의 주입 형(Python 키워드 runner= · render_module= 은 typed hook 으로) ─────────────────────────
fn ctx(repo: &Path, ev: &J, runner: Option<&Runner>) -> HR<_Ctx> {
    _ctx_with(repo, ev, runner.cloned(), None, None)
}
fn aset(repo: &Path, ev: &J, runner: &Runner) -> HR<J> {
    let c = ctx(repo, ev, Some(runner))?;
    Ok(_applied(&c, true)?.0)
}
fn coll(repo: &Path, ev: &J, payload: &Path, runner: Option<&Runner>) -> HR<J> {
    let c = ctx(repo, ev, runner)?;
    collect_ctx(&c, payload)
}
fn repro(repo: &Path, ev: &J, runner: &Runner) -> HR<Vec<J>> {
    let c = ctx(repo, ev, Some(runner))?;
    let plane = _plane(&c)?.0;
    let sel = if plane == "docker" { _applied(&c, false)?.1 } else { J::Null };
    _reproduce(&c, &sel, &plane, None)
}
fn steps_by(v: &[J]) -> J {
    by(&J::List(v.to_vec()), "step")
}
fn subr(repo: &Path, ev: &J) -> HR<J> {
    sub_recipe(repo, ev, &J::Null, &J::Null)
}
fn vsc(repo: &Path, ev: &J) -> HR<J> {
    Ok(J::List(value_status_candidates(repo, ev, &J::Null)?))
}

/// `core.git(repo, *args, env_extra=genv)`.
fn git(repo: &Path, args: &[&str], env: &[(String, String)]) -> HR<Proc> {
    core::git_full(repo, args, true, None, env)
}
fn git_commit(repo: &Path, msg: &str, env: &[(String, String)]) -> HR<Proc> {
    git(repo, &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", msg], env)
}
/// `subprocess.run([...], input=data, cwd=repo, env={**os.environ, **genv}).stdout.strip()`.
fn git_stdin(repo: &Path, args: &[&str], data: &[u8], env: &[(String, String)]) -> HR<String> {
    use std::io::Write;
    let mut cmd = std::process::Command::new("git");
    cmd.args(args).current_dir(repo).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut ch = cmd.spawn().map_err(|e| HintError::py("OSError", e.to_string()))?;
    ch.stdin.take().unwrap().write_all(data).map_err(|e| HintError::py("OSError", e.to_string()))?;
    let o = ch.wait_with_output().map_err(|e| HintError::py("OSError", e.to_string()))?;
    if !o.status.success() {
        return Err(HintError::py("CalledProcessError", format!("git {}", args.join(" "))));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn genv_at(utc: &str) -> HR<Vec<(String, String)>> {
    let d = core::git_date(utc)?;
    let email = core::synthetic_email();
    Ok(vec![
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_AUTHOR_NAME".into(), core::SYNTHETIC_NAME.into()),
        ("GIT_AUTHOR_EMAIL".into(), email.clone()),
        ("GIT_COMMITTER_NAME".into(), core::SYNTHETIC_NAME.into()),
        ("GIT_COMMITTER_EMAIL".into(), email),
        ("GIT_AUTHOR_DATE".into(), d.clone()),
        ("GIT_COMMITTER_DATE".into(), d),
    ])
}

// ── 픽스처 ─────────────────────────────────────────────────────────────────────────────────────
struct Fx {
    own: PathBuf,
    repo: PathBuf,
    out: PathBuf,
    assets: PathBuf,
    cell: String,
    ev: J,
    ip: String,
    mnt: String,
    operator_root: String,
    runner_txt: String,
    history: String,
    docker_old: Runner,
    ledger: J,
    payload: PathBuf,
    res: J,
    decl: J,
    ev_w: J,
    native_ev: J,
    ev_bare: J,
}

pub fn selftest() -> Vec<String> {
    let mut st = St { bad: Vec::new(), n: 0 };
    let operator_root = ["", "home", "fixture", "models"].join("/");
    let source_env = format!(
        "# model settings\nVLLM_PLE_MMAP_DIR=/app/ple/x\nBUILD_JOBS=8\nexport QUANT_MODEL_PATH='{operator_root}'\nOTHER_ROOT={operator_root}\n"
    );
    match _docker_triplet_env_shape(&source_env) {
        Ok((shaped_env, shaped_root)) => st.ck(
            "Docker triplet host model root is a recipient coordinate; tuning/container paths stay intact",
            shaped_root
                && shaped_env.contains("QUANT_MODEL_PATH=<manifest.quant_model_path>")
                && shaped_env.contains("VLLM_PLE_MMAP_DIR=/app/ple/x\nBUILD_JOBS=8\n")
                && shaped_env.contains(&format!("OTHER_ROOT={operator_root}")),
        ),
        Err(e) => st.ck(&format!("Docker triplet host model root ({})", e.code), false),
    }
    st.ck(
        "Docker triplet absent/empty selector is unchanged",
        _docker_triplet_env_shape("BUILD_JOBS=8\nQUANT_MODEL_PATH=\n").ok()
            == Some(("BUILD_JOBS=8\nQUANT_MODEL_PATH=\n".to_string(), false)),
    );
    let own = core::skill_dir().parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
    let tmp = core::TempDir::new("hint-artifacts-");
    let repo = core::resolve(tmp.path());
    let key = core::posix(&repo);
    let prev = crate::evidence::_self_node_cache_put(&key, Some((Some("main".to_string()), "isolated fixture".to_string())));
    let r = run_fixture(&mut st, &own, &repo, operator_root);
    if let Err(e) = r {
        st.bad.push(format!("artifacts: 자체검사 예외 {}: {}", e.code, core::head_chars(&e.message, 300)));
    }
    crate::evidence::_self_node_cache_put(&key, prev);
    drop(tmp);
    st.bad
}

fn stage_owner(own: &Path, repo: &Path, rel_path: &str) -> HR<()> {
    let dst = repo.join(rel_path);
    mkd(dst.parent().unwrap())?;
    wb(&dst, &rb(&own.join(rel_path))?)
}

fn run_fixture(st: &mut St, own: &Path, repo: &Path, operator_root: String) -> HR<()> {
    // 소유 모듈 실물(slave_forward · render_dockerfile)을 격리 저장소로 복사 — 대역이 실물보다 좁지 않게.
    // Rust (plan_26111615 D4): the owners are their launchers + the renderer's Rust source (revision identity) — never a `.py`.
    for rel_path in [core::REL_SLAVE_FORWARD, core::REL_RENDER_DOCKERFILE, core::REL_RENDER_DOCKERFILE_SRC] {
        stage_owner(own, repo, rel_path)?;
    }
    let rt_rel = ".claude/skills/upstream-version-watch/rust/runtime/target";
    if own.join(rt_rel).exists() {
        mkd(repo.join(rt_rel).parent().unwrap())?;
        std::os::unix::fs::symlink(own.join(rt_rel), repo.join(rt_rel)).map_err(|e| ioe(e, &repo.join(rt_rel)))?;
    }
    let out = repo.join("output").join("multi");
    for d in ["configs", "envs", "build_patches_src/files", "build_patches"] {
        mkd(&out.join(d))?;
    }
    mkd(repo.join(core::REL_PII_TERMS).parent().unwrap())?;
    wt(&repo.join(core::REL_PII_TERMS), "fixture-secret-literal\n")?;
    let assets = repo.join(".claude/skills/upstream-version-watch/assets/configs");
    mkd(&assets)?;
    let runner_txt = "#!/bin/bash\nif [ -f /app/configs/arm_patch.sh ]; then\n    source /app/configs/arm_patch.sh\nfi\n\
GPU_COUNT=$(ray status | head -1)\nif [ \"${GPU_COUNT}\" = \"2\" ] || ray status | grep -q \"0.0/2.0 GPU\"; then :; fi\n"
        .to_string();
    for (name, text) in [
        ("serve_runner.sh", runner_txt.as_str()),
        ("arm_patch.sh", "#!/bin/bash\n_cfg=\"${CONFIG_FILE:-default}\"\n"),
        ("debug-init.sh", "#!/bin/bash\n"),
    ] {
        wt(&assets.join(name), text)?;
        wt(&out.join("configs").join(name), text)?;
    }
    let cell = "c1-mmp".to_string();
    wt(
        &out.join("configs").join(format!("{cell}.yaml")),
        "# 헤더 주석\nmodel: /app/models/X\nhost: localhost\nport: 8000\nserved_model_name: fx\n\
max-model-len: 262144\nkv-cache-dtype: auto\n\
kv-cache-memory-bytes: 21474836480   # 20GiB 음성대조\nmax-num-seqs: 8\n\
enforce-eager: true   # 승계 통제변인\nurl-ish: 'a#b'   # 따옴표 안 # 는 값\n\
async-scheduling: false   # MTP+async 금지(명시 필수)\ngpu-memory-utilization: 0.85   # 요구 조건\n\
speculative-config:\n  method: mtp\n  num_speculative_tokens: 3\n",
    )?;
    wt(
        &out.join("configs").join(format!("{cell}.sh")),
        "#!/bin/bash\nif [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi\nvllm serve\n",
    )?;
    wt(&out.join("configs").join(format!("{cell}_patch.py")), "# runtime patch\n")?;
    wt(
        &out.join("envs").join(format!(".env.{cell}")),
        "CONFIG_FILE=c1-mmp\nSLAVE_CONTAINER_NAME=c1-slave\nIMAGE_TAG=easy-vllm:fx-source\n\
BUILD_DOCKERFILE=Dockerfile.source-build\nVLLM_REF=v0.1.0\nBUILD_JOBS=8\nVLLM_PLE_MMAP=1\n\
VLLM_PLE_MMAP_DIR=/app/ple/x\n",
    )?;
    wt(
        &out.join("Dockerfile.source-build"),
        "FROM base\nARG VLLM_REF=v0.1.0\nARG SM12X_PORT=0\nARG SRC_DEPS_AUTHORITY=0\nARG BUILD_JOBS=16\n\
COPY requirements.txt /tmp/requirements.txt\nCOPY build_patches_src/ /tmp/build_patches_src/\n\
COPY build_patches/ /tmp/build_patches/\n",
    )?;
    wt(&out.join("Dockerfile"), "FROM wheel\nARG VLLM_VERSION=0.0.1\nCOPY requirements.txt /tmp/r.txt\n")?;
    wt(&out.join("requirements.txt"), "transformers<5.15.0\n")?;
    let ip = ["192", "168", "7", "11"].join(".");
    let mnt = format!("/{}", "mnt");
    let compose = format!(
        concat!(
            "x-gpu-common: &gpu-common\n  image: ${{IMAGE_TAG:-easy-vllm:fx-source}}\n  build:\n    context: .\n",
            "    dockerfile: ${{BUILD_DOCKERFILE:-Dockerfile.source-build}}\n    args:\n",
            "      VLLM_REPO: ${{VLLM_REPO:-repo}}\n      VLLM_REF: ${{VLLM_REF:-v0.1.0}}\n      SM12X_PORT: ${{SM12X_PORT:-0}}\n",
            "      SRC_DEPS_AUTHORITY: ${{SRC_DEPS_AUTHORITY:-0}}\n      BUILD_JOBS: ${{BUILD_JOBS:-16}}\n",
            "  volumes:\n    - ${{NAS_MODEL_PATH:-{mnt}/models}}:/app/models:ro\n    - ./configs:/app/configs:ro\n",
            "services:\n  vllm-debug:\n    <<: *gpu-common\n    profiles: [debug]\n    env_file:\n",
            "      - envs/.env.interconnect\n    command: [\"bash\", \"--rcfile\", \"/app/configs/debug-init.sh\"]\n",
            "  vllm-master-serve:\n    <<: *gpu-common\n    profiles: [master]\n    environment:\n",
            "      NODE_ROLE: master\n      CONFIG_FILE: ${{CONFIG_FILE:-default}}\n",
            "      RUNTIME_PATCH_STEM: ${{RUNTIME_PATCH_STEM:-}}\n    env_file:\n",
            "      - envs/.env.interconnect\n      - envs/.env.cluster\n      - path: envs/.env.${{CONFIG_FILE:-default}}\n",
            "    command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n",
            "  vllm-slave-serve:\n    <<: *gpu-common\n    profiles: [slave]\n",
            "    container_name: ${{SLAVE_CONTAINER_NAME:-slave}}\n    environment:\n      NODE_ROLE: slave\n",
            "      CONFIG_FILE: ${{CONFIG_FILE:-default}}\n      RUNTIME_PATCH_STEM: ${{RUNTIME_PATCH_STEM:-}}\n",
            "      RAY_PORT: ${{RAY_PORT:-6379}}\n",
            "      VLLM_PLE_MMAP: ${{VLLM_PLE_MMAP:-0}}\n      VLLM_PLE_MMAP_DIR: ${{VLLM_PLE_MMAP_DIR:-/app/ple}}\n",
            "    env_file:\n      - envs/.env.interconnect\n      - envs/.env.cluster\n",
            "    command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n",
        ),
        mnt = mnt
    );
    wt(&out.join("docker-compose.yaml"), &compose)?;
    wt(
        &out.join("envs/.env.cluster"),
        &format!("# 헤더\nMASTER_HOST_IP={ip}\nSLAVE_HOST_IP=node-sub-fixture\nSSH_USER=opfixture\nMAX_JOBS=4\nRAY_PORT=6379\n"),
    )?;
    wt(&out.join("envs/.env.interconnect"), "NCCL_DEBUG=INFO\nNCCL_SOCKET_IFNAME=enpfixture0\nNCCL_IB_DISABLE=1\n")?;
    wt(&out.join(".env"), &format!("NAS_MODEL_PATH={mnt}/nas-fixture/models\nPLE_MMAP_HOST_PATH={mnt}/nvme-fixture\n"))?;
    for p_ in [
        out.join("envs/.env.cluster"),
        out.join("envs/.env.interconnect"),
        out.join(".env"),
        out.join("configs").join(format!("{cell}.yaml")),
        out.join("configs").join(format!("{cell}.sh")),
        out.join("envs").join(format!(".env.{cell}")),
    ] {
        _set_mtime(&p_, "2026-01-01T00:00:00Z")?;
    }
    wt(
        &out.join("build_patches_src/50-port.sh"),
        "#!/bin/bash\n# model-trigger : OtherModel\n: \"${SM12X_PORT:=0}\"\nif [ \"${SM12X_PORT}\" != \"1\" ]; then\n    echo \"skip\"\n    exit 0\nfi\n",
    )?;
    wt(
        &out.join("build_patches_src/55-deps.sh"),
        "#!/bin/bash\n: \"${SM12X_PORT:=0}\"\n: \"${SRC_DEPS_AUTHORITY:=0}\"\n\
if [ \"${SRC_DEPS_AUTHORITY}\" != \"1\" ] && [ \"${SM12X_PORT}\" = \"1\" ]; then\n    SRC_DEPS_AUTHORITY=1\nfi\n\
if [ \"${SRC_DEPS_AUTHORITY}\" != \"1\" ]; then\n    echo \"skip\"\n    exit 0\nfi\n",
    )?;
    wt(&out.join("build_patches_src/60-ungated.sh"), "#!/bin/bash\n# gate : 없음(ungated)\n")?;
    wt(
        &out.join("build_patches_src/58-effect-first.sh"),
        "#!/bin/bash\npip install something\n: \"${SM12X_PORT:=0}\"\nif [ \"${SM12X_PORT}\" != \"1\" ]; then\n    exit 0\nfi\n",
    )?;
    wt(&out.join("build_patches_src/PROVENANCE.json"), "{}")?;
    wt(&out.join("build_patches_src/.gitkeep"), "")?;
    wt(&out.join("build_patches_src/stray.txt"), "x")?;
    wt(&out.join("build_patches/10-shared.sh"), "#!/bin/bash\n# model-trigger : DeepseekV4 범용\n")?;
    let history = "WORKDIR /workspace\nRUN |4 VLLM_REF=v0.1.0 SM12X_PORT=0 SRC_DEPS_AUTHORITY=0 BUILD_JOBS=8 /bin/bash -c \
export SM12X_PORT=\"${SM12X_PORT}\"\nRUN |3 NVIDIA_PYTORCH_VERSION=26.07 SM12X_PORT=1 X=1 /bin/sh -c base\n"
        .to_string();
    let docker_old = _fake_docker(&[
        (&["image", "inspect", "--format", "{{.Id}}"], (0, "sha256:fx\n")),
        (&["image", "inspect", "--format", "{{.Created}}"], (0, "2026-01-02T03:04:05.1+09:00\n")),
        (&["history"], (0, &history)),
        (&["run"], (1, "")),
    ]);
    mkd(&repo.join("output/multi/benchlog/sweep_c1-mmp/level_01"))?;
    wt(&repo.join("output/multi/benchlog/sweep_c1-mmp/lite_cold.json"), &dumps(&p(r#"{"date": "20260101-190000"}"#)))?;
    wt(
        &repo.join("output/multi/benchlog/sweep_c1-mmp/level_01/bench.json"),
        &dumps(&p(r#"{"date": "20260101-190500", "backend": "openai", "model_id": "fx", "tokenizer_id": "/app/models/X",
                     "num_prompts": 4, "max_concurrency": 1, "request_rate": "inf", "burstiness": 1.0, "completed": 4,
                     "total_input_tokens": 4096, "total_output_tokens": 1024}"#)),
    )?;
    mkd(&repo.join("docs/logs/main/events"))?;
    wt(
        &repo.join("docs/logs/main/events/2026-01.jsonl"),
        &format!(
            "{}\n{}\n{}\n",
            dumps(&jd(&[("kind", "budget_declare".into()), ("label", format!("smoke-{cell}").into()), ("ts", "2026-01-01T18:30:00Z".into())])),
            dumps(&jd(&[("kind", "budget_declare".into()), ("label", format!("smoke-{cell}").into()), ("ts", "2026-01-01T21:00:00Z".into())])),
            dumps(&jd(&[("kind", "budget_declare".into()), ("label", "smoke-other".into()), ("ts", "2026-01-01T18:20:00Z".into())])),
        ),
    )?;
    let ev = p(&format!(
        r#"{{"topology": "multi", "cell": "{cell}", "node": "cluster",
            "manifest": {{"topology": "multi", "self_role": "main"}},
            "declaration": {{"nodes": [{{"node_id": "main", "role": "main"}}, {{"node_id": "sub", "role": "sub"}}]}},
            "lockset": {{"kv_source": "measured-clamp", "batch_source": "hand-lever"}},
            "cell_status": {{}}, "attestation": null, "build_identity": {{"image_tag": "easy-vllm:fx-source"}},
            "sweep": {{"dir": "output/multi/benchlog/sweep_c1-mmp", "generated_utc": "2026-01-01T19:30:00Z",
                      "index": {{"config": "{cell}"}}}}}}"#
    ));
    let mut fx = Fx {
        own: own.to_path_buf(),
        repo: repo.to_path_buf(),
        out,
        assets,
        cell,
        ev,
        ip,
        mnt,
        operator_root,
        runner_txt,
        history,
        docker_old,
        ledger: J::Null,
        payload: repo.join("payload"),
        res: J::Null,
        decl: J::Null,
        ev_w: J::Null,
        native_ev: J::Null,
        ev_bare: J::Null,
    };
    sec_applied(st, &mut fx)?;
    sec_ledger(st, &mut fx)?;
    sec_collect(st, &mut fx)?;
    sec_repro(st, &mut fx)?;
    sec_phases_drift(st, &mut fx)?;
    if let Err(e) = sec_git(st, &mut fx) {
        st.bad.push(format!("artifacts: 임시 git 픽스처 실패 {}: {}", e.code, core::head_chars(&e.message, 300)));
    }
    sec_inputs(st, &mut fx)?;
    sec_regen(st, &mut fx)?;
    Ok(())
}

fn trip_with_env(cell: &str, env: &str) -> J {
    jd(&[
        ("env", env.into()),
        ("yaml", format!("output/multi/configs/{cell}.yaml").into()),
        ("sh", format!("output/multi/configs/{cell}.sh").into()),
    ])
}

/// `set(re.findall(r'missing\.append\("([A-Z0-9_]+)"\)', artifacts.py))` → the Rust sources of this module (selftest excluded).
fn emitted_missing_codes() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let root = core::skill_dir().join("rust/src/artifacts");
    let re = Re::new(r#"missing\.push\("([A-Z0-9_]+)""#, 0);
    let mut stack = vec![root];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p_ = e.path();
            if p_.is_dir() {
                stack.push(p_);
            } else if p_.extension().is_some_and(|x| x == "rs") && p_.file_name().is_some_and(|n| n != "selftest.rs") {
                if let Ok(t) = std::fs::read_to_string(&p_) {
                    out.extend(re.findall(&t));
                }
            }
        }
    }
    out
}

// ── 트리플렛 부재 · 빌드 컨텍스트 원천 · fork_pin · 평면 · native · 결손 코드 · 형상 부재 ──
fn sec_inputs(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    st.raises("★트리플렛 부재", "HINT_TRIPLET_ABSENT", || {
        coll(&repo, &with(&ev, vec![("triplet", trip_with_env(&cell, "output/multi/envs/.env.none"))]), &repo.join("p5"), Some(&docker_old))
    });
    st.ck("★실패한 수집은 artifacts/ 를 남기지 않는다", !exists(&repo.join("p2/artifacts")) && !exists(&repo.join("p3/artifacts")));
    wt(
        &out.join("Dockerfile.ctx"),
        concat!(
            "FROM base\nARG SM12X_PORT=0\nCOPY requirements.txt /tmp/requirements.txt\nADD extra.txt /x\n",
            "COPY sub/extra2.txt /y\nCOPY *.cfg /z\nADD https://example.invalid/x.tar /w\n",
            "COPY build_patches_src/ /tmp/build_patches_src/\n",
        ),
    )?;
    wt(&out.join("extra.txt"), "x\n")?;
    mkd(&out.join("sub"))?;
    wt(&out.join("sub/extra2.txt"), "y\n")?;
    wt(&out.join("envs/.env.ctx"), "IMAGE_TAG=easy-vllm:fx-source\nBUILD_DOCKERFILE=Dockerfile.ctx\n")?;
    let rcx = coll(&repo, &with(&ev, vec![("triplet", trip_with_env(&cell, "output/multi/envs/.env.ctx"))]), &repo.join("p_ctx"), Some(&docker_old))?;
    let bre = rcx["slots"]["build_recipe"]["evidence"].clone();
    let files = sset(&rcx["files"]);
    st.ck(
        "ADD·하위 디렉터리 COPY 원천 실림 · ★글롭은 기재만 · 원격 ADD 는 컨텍스트 아님",
        ["artifacts/build_recipe/extra.txt", "artifacts/build_recipe/extra2.txt", "artifacts/build_recipe/Dockerfile.ctx"].iter().all(|x| files.contains(*x))
            && bre["context_not_shipped"].items().iter().map(|x| x["source"].display()).collect::<Vec<_>>() == ["*.cfg"]
            && bre["recipe_vs_image"].items().iter().any(|r| eqs(&r["path"], "output/multi/sub/extra2.txt")),
    );
    // fork_pin
    let led_p = repo.join(REL_ARCH_VARIANT_LEDGER);
    mkd(led_p.parent().unwrap())?;
    wt(
        &led_p,
        &dumps(&p(r#"{"source_build_variants": {"_note": "x", "fx-key": {"variant_id": "source-fx-0.1.0", "vllm_repo": "https://example.invalid/v.git",
                     "vllm_ref": "v0.1.0", "image_tag": "easy-vllm:fx-source"}}}"#)),
    )?;
    let envc = out.join("envs").join(format!(".env.{cell}"));
    let base_env = rt(&envc)?;
    for (i, (label, line, code)) in [
        ("키", "VARIANT=fx-key", None),
        ("유일 variant_id", "VARIANT=source-fx-0.1.0", None),
        ("★원장 밖 변종", "VARIANT=nope", Some("HINT_FORK_PIN_UNBOUND")),
        ("★예약 키", "VARIANT=_note", Some("HINT_FORK_PIN_UNBOUND")),
    ]
    .iter()
    .enumerate()
    {
        wt(&envc, &format!("{base_env}{line}\n"))?;
        let dest = repo.join(format!("p_fp_{i}"));
        if let Some(code) = code {
            st.raises(&format!("fork_pin {label}"), code, || coll(&repo, &ev, &dest, Some(&docker_old)));
        } else {
            match coll(&repo, &ev, &dest, Some(&docker_old)) {
                Ok(rfp) => st.ck(
                    &format!("fork_pin {label} → variant.json"),
                    has(&rfp["files"], "artifacts/fork_pin/variant.json") && is_true(&rfp["slots"]["fork_pin"]["applicable"]),
                ),
                Err(e) => st.ck(&format!("fork_pin {label} → variant.json(예외 {})", e.code), false),
            }
        }
    }
    wt(&envc, &format!("{}VARIANT=fx-key\n", base_env.replace("VLLM_REF=v0.1.0", "VLLM_REF=v0.1.9")))?;
    st.raises("★변종 vllm_ref ≠ 셀 env VLLM_REF", "HINT_FORK_PIN_MISMATCH", || coll(&repo, &ev, &repo.join("p_fp_mm"), Some(&docker_old)));
    wt(&envc, &base_env)?;
    _set_mtime(&envc, "2026-01-01T00:00:00Z")?;
    // ★평면 파생 불가 = 차단
    wt(&out.join("envs/.env.bare"), "CONFIG_FILE=x\n")?;
    let ev_bare = with(&ev, vec![("build_identity", J::dict()), ("triplet", trip_with_env(&cell, "output/multi/envs/.env.bare"))]);
    st.raises("★평면 파생 불가", "HINT_PLANE_UNDERIVABLE", || plane_of(&repo, &ev_bare, &J::Null));
    let ev_bn = with(&ev_bare, vec![("plane", "native".into())]);
    st.ck("선언 native + 신호 없음 = native", plane_of(&repo, &ev_bn, &J::Null).ok().as_deref() == Some("native"));
    let outside = repo.join("outside-artifacts");
    mkd(&outside)?;
    let sy_payload = repo.join("p_symlink");
    mkd(&sy_payload)?;
    std::os::unix::fs::symlink(&outside, sy_payload.join("artifacts")).map_err(|e| ioe(e, &sy_payload))?;
    st.raises("★artifacts 출력 symlink = draft 밖 write 전 차단", "HINT_ARTIFACTS_OUTPUT_SYMLINK", || coll(&repo, &ev_bn, &sy_payload, Some(&docker_old)));
    st.ck("★출력 symlink 차단 뒤 외부 잔재 0", std::fs::read_dir(&outside).map(|mut r| r.next().is_none()).unwrap_or(false));
    wt(&repo.join("output/multi/native-install.sh"), "python -m pip install vllm\n")?;
    wt(&repo.join("output/multi/native-freeze-main.txt"), "vllm==0.1.0\n")?;
    wt(&repo.join("output/multi/native-freeze-sub.txt"), "vllm==0.1.0\n")?;
    let native_ev = with(
        &ev_bare,
        vec![
            ("plane", "native".into()),
            ("native_install_path", "output/multi/native-install.sh".into()),
            ("pip_freeze_path", "output/multi/native-freeze-main.txt".into()),
            ("pip_freeze_paths", p(r#"{"main": "output/multi/native-freeze-main.txt", "sub": "output/multi/native-freeze-sub.txt"}"#)),
        ],
    );
    let rn = coll(&repo, &native_ev, &repo.join("p_nat"), Some(&docker_old))?;
    let nn = sset(&rn["files"]);
    let areq: BTreeSet<(String, String)> = rn["agent_requests"].items().iter().map(|r| (r["slot"].display(), r["name"].display())).collect();
    let miss = sset(&rn["missing"]);
    st.ck(
        "native 원천 이미지 미관측: producer install+양 node lock 동봉 · 빌드 입력 추측 ✗ · 결손 코드(APPLIED_SET·NATIVE_LAUNCH·SUB_RECIPE) · Agent 요청(Dockerfile·compose·serve_runner·기동 기록) · 러너 중복 ✗",
        ["artifacts/build_recipe/native-install.sh", "artifacts/build_recipe/pip-freeze-main.txt", "artifacts/build_recipe/pip-freeze-sub.txt"].iter().all(|x| nn.contains(*x))
            && !nn.iter().any(|x| x.starts_with("artifacts/compose/") || x.starts_with("artifacts/build_patch") || x.contains("Dockerfile"))
            && rn["applied_set"].is_null()
            && ["HINT_MISSING_APPLIED_SET", "HINT_MISSING_NATIVE_LAUNCH", "HINT_MISSING_SUB_RECIPE"].iter().all(|x| miss.contains(*x))
            && [("build_recipe", "Dockerfile"), ("compose", "docker-compose.yaml"), ("compose", "serve_runner.sh"), ("triplet", NATIVE_LAUNCH_NAME)]
                .iter()
                .all(|(a, b)| areq.contains(&(a.to_string(), b.to_string())))
            && rn["agent_requests"].items().iter().all(|r| eqs(&r["generated_by"], "agent") && eqs(&r["verification"], "generated-unverified") && r["header_required"].truthy())
            && !rn["agent_requests"].items().iter().any(|r| exists(&repo.join("p_nat").join(r["path"].display())))
            && is_false(&rn["native_source"]["ok"]),
    );
    st.raises("★native producer install 신호 부재 차단", "HINT_NATIVE_EVIDENCE_MISSING", || {
        coll(&repo, &with(&native_ev, vec![("native_install_path", J::Null)]), &repo.join("p_nat_missing"), Some(&docker_old))
    });
    st.raises("★미등재 결손 코드 = tripwire(직접)", "HINT_MISSING_CODE_UNREGISTERED", || {
        _require_registered(&repo, &["HINT_MISSING_BUILD_LEDGER".to_string(), "HINT_MISSING_FIXTURE_NOT_A_CODE".to_string()])
    });
    let emitted = emitted_missing_codes();
    let reg = keys(crate::evidence::MISSING_CODES());
    let unreg: Vec<&String> = emitted.iter().filter(|x| !reg.contains(*x)).collect();
    st.ck(
        &format!("artifacts 가 내는 결손 코드 전부 evidence.MISSING_CODES 등재: {}", J::from(unreg.iter().map(|s| s.to_string()).collect::<Vec<_>>()).repr()),
        !emitted.is_empty() && unreg.is_empty(),
    );
    st.raises("★native pip lock 신호 부재 차단", "HINT_NATIVE_EVIDENCE_MISSING", || {
        coll(&repo, &with(&native_ev, vec![("pip_freeze_paths", J::dict())]), &repo.join("p_nat2"), Some(&docker_old))
    });
    st.ck("★native evidence 차단 뒤 artifacts 잔재 없음", !exists(&repo.join("p_nat2/artifacts")));
    mv(&out.join("envs/.env.cluster"), &out.join("envs/.env.cluster.off"))?;
    mv(&out.join(".env"), &out.join(".env.off"))?;
    let rc_ = coll(&repo, &ev, &repo.join("p_nocl"), Some(&docker_old));
    mv(&out.join("envs/.env.cluster.off"), &out.join("envs/.env.cluster"))?;
    mv(&out.join(".env.off"), &out.join(".env"))?;
    let rc_ = rc_?;
    let mut absent_rows: Vec<String> = rc_["env_shapes"].items().iter().filter(|e| eqs(&e["status"], "absent")).map(|e| e["file"].display()).collect();
    absent_rows.sort();
    st.ck(
        "★.env.cluster·프로젝트 .env 부재 = 결손 기재 + 형상 표에 부재 행",
        has(&rc_["missing"], "HINT_MISSING_ENV_SHAPE") && absent_rows == [".env", "envs/.env.cluster"],
    );
    st.raises("★선언·관측 평면 모순", "HINT_PLANE_MISMATCH", || plane_of(&repo, &with(&ev, vec![("plane", "native".into())]), &J::Null));
    fx.native_ev = native_ev;
    fx.ev_bare = ev_bare;
    Ok(())
}

// ── F2 · R-b · F3 · Solar F2/F3 · F5 벤치 ──
fn sec_regen(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let ycell = out.join("configs").join(format!("{cell}.yaml"));
    _set_mtime(&out.join("envs/.env.interconnect"), "2026-01-02T00:00:00Z")?;
    _set_mtime(&out.join(".env"), "2026-01-02T00:00:00Z")?;
    _set_mtime(&ycell, "2026-01-02T00:00:00Z")?;
    let rg2 = coll(&repo, &ev, &repo.join("p_regen"), Some(&docker_old))?;
    let clr2 = rt(&repo.join("p_regen/artifacts/compose/.env.cluster.template"))?;
    let exr = by(&rg2["slots"]["compose"]["excluded"], "file");
    let f2 = sset(&rg2["files"]);
    st.ck(
        "★F2 측정 뒤 재생성 형상(interconnect · 프로젝트 .env) = 미실림 · 파일 목록·트리 모두에서 없다",
        !f2.contains("artifacts/compose/.env.interconnect.template")
            && !f2.contains("artifacts/compose/.env.template")
            && !exists(&repo.join("p_regen/artifacts/compose/.env.interconnect.template"))
            && !exists(&repo.join("p_regen/artifacts/compose/.env.template")),
    );
    let xi = exr["envs/.env.interconnect"].clone();
    st.ck(
        "F2 공유 계약: slots.compose.excluded reason = post-measurement-regenerated(why 첫 토큰 같음 · 측정·mtime 기재)",
        eqs(&xi["reason"], POST_MEASUREMENT_REGENERATED)
            && eqs(&exr[".env"]["reason"], POST_MEASUREMENT_REGENERATED)
            && sv(&xi["why"]).split('(').next() == Some(POST_MEASUREMENT_REGENERATED)
            && eqs(&xi["mtime_utc"], "2026-01-02T00:00:00Z")
            && eqs(&xi["measured_utc"], "2026-01-01T19:30:00Z")
            && has(&xi["why"], "measurement_env_observed"),
    );
    let sr2 = repo.join("p_regen/artifacts/compose/sub_recipe.json");
    let sr2_txt = if sr2.exists() { rt(&sr2)? } else { String::new() };
    let shipped_txt = format!("{}{}", core::dumps(&rg2["slots"]["compose"]), sr2_txt);
    st.ck(
        "★F2 측정 당시 env 좌표 = zip 안의 자리(01 §1.4 · PAYLOAD.json) · 내부 키 `facts.` 를 배포 문구에 싣지 않는다",
        has(&xi["why"], ENV_OBSERVED_POINTER)
            && !re_search(r"(?<![A-Za-z_])facts\.", &shipped_txt)
            && (!sr2.exists() || sr2_txt.contains(ENV_OBSERVED_POINTER)),
    );
    st.ck(
        "★F2 측정 전 파일(cluster) = 실림 · 값 유지 + mtime 대조 사실 기재 · excluded 에 없다",
        f2.contains("artifacts/compose/.env.cluster.template") && clr2.contains("MAX_JOBS=4") && clr2.contains("mtime ≤ 측정") && exr.get("envs/.env.cluster").is_none(),
    );
    let mut shp_m = Map::new();
    for e in rg2["env_shapes"].items() {
        shp_m.insert(e["file"].display(), jl![e.get("post_measurement_regenerated").cloned().unwrap_or(J::Null), e.get("status").cloned().unwrap_or(J::Null)]);
    }
    st.ck(
        "F2 공유 계약: slots.compose.evidence.post_measurement_regenerated(저장소 상대 경로) · env_shapes 행 = excluded",
        strs(&rg2["slots"]["compose"]["evidence"]["post_measurement_regenerated"]) == ["output/multi/.env", "output/multi/envs/.env.interconnect"]
            && jeq(
                &J::Dict(shp_m),
                &p(r#"{"envs/.env.interconnect": [true, "excluded"], "envs/.env.cluster": [false, null], ".env": [true, "excluded"]}"#),
            )
            && has(&rg2["slots"]["compose"]["evidence"]["note"], "형상 미실림"),
    );
    st.ck(
        "F2 트리플렛은 가리지 않고 재생성 사실만 적는다",
        strs(&rg2["slots"]["triplet"]["evidence"]["post_measurement_regenerated"]) == [format!("output/multi/configs/{cell}.yaml")]
            && rt(&repo.join(format!("p_regen/artifacts/triplet/{cell}.yaml")))? == rt(&ycell)?,
    );
    let rsr = if rg2["sub_recipe"].truthy() { rg2["sub_recipe"].clone() } else { J::dict() };
    st.ck(
        "★F2 재생성 형상을 싣지 않으면 sub_recipe 가 그 템플릿을 가리키지 않는다 · 빠진 이유를 스스로 말한다 · 실린 cluster 표지는 남는다",
        rsr["per_node_values"].get("interconnect").is_none()
            && rsr["env_shapes_not_shipped"].items().iter().map(|x| x["file"].display()).collect::<Vec<_>>() == ["envs/.env.interconnect"]
            && rsr["per_node_values"]["MASTER_HOST_IP"].display().starts_with("<manifest"),
    );
    st.ck("F2 공개 sub_recipe() 도 같은 규칙(재생성 interconnect 미참조)", subr(&repo, &ev)?["per_node_values"].get("interconnect").is_none());
    // ── R-b ──
    let ic_row2 = find_in(&rg2["env_shapes"], "file", "envs/.env.interconnect");
    st.ck(
        "★R-b 재렌더 불가(manifest 부재) = 결손 코드 + 제외 행에 사유(침묵 ✗)",
        has(&rg2["missing"], "HINT_MISSING_ENV_SHAPE_RERENDER")
            && sv(&ic_row2["rerender_error"]).contains("manifest.yaml 부재")
            && sv(&xi["rerender_error"]).contains("manifest.yaml 부재"),
    );
    let rev_m = _git_rev_before(&ctx(&repo, &ev, None)?, core::REL_RENDER_DOCKERFILE_SRC, Some("2026-01-01T19:30:00Z"))?;
    st.ck("R-b 픽스처: 측정 전 렌더러 커밋이 있다", rev_m.is_some());
    let man_p = out.join("manifest.yaml");
    // evidence.selftest_stage_manifest_contract(own, repo): the adapter, its runtime files and the built executable.
    for rel_path in std::iter::once(crate::evidence::REL_MANIFEST_CONTRACT).chain(crate::evidence::REL_MANIFEST_CONTRACT_RUNTIME.iter().copied()) {
        let (source, dest) = (fx.own.join(rel_path), repo.join(rel_path));
        mkd(dest.parent().unwrap())?;
        wb(&dest, &rb(&source)?)?;
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&source).map_err(|e| ioe(e, &source))?.permissions().mode() & 0o777;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode)).map_err(|e| ioe(e, &dest))?;
    }
    wt(
        &man_p,
        "topology: multi\ninterconnect:\n  hca_devices: rocep1s0f1\n  gid_index: 3\n  socket_iface: enpfixture0\n  platform_preset: dgx-spark-gb10\n  nccl_transport: rdma\n",
    )?;
    _set_mtime(&man_p, "2026-01-01T00:00:00Z")?;
    let r = (|st: &mut St| -> HR<()> {
        let rgr = coll(&repo, &ev, &repo.join("p_rerender"), Some(&docker_old))?;
        let icr_path = repo.join("p_rerender/artifacts/compose/.env.interconnect.template");
        let icr = if icr_path.is_file() { rt(&icr_path)? } else { String::new() };
        let icr_l: Vec<&str> = icr.lines().collect();
        let row_r = find_in(&rgr["env_shapes"], "file", "envs/.env.interconnect");
        let mut rec_r = J::Null;
        if let Some(m) = rgr["slots"].as_dict() {
            for v in m.values() {
                for r_ in v["file_records"].items() {
                    if eqs(&r_["path"], "artifacts/compose/.env.interconnect.template") {
                        rec_r = r_.clone();
                    }
                }
            }
        }
        let revm = rev_m.clone().unwrap_or_default();
        let gb = format!("renderer@{}", core::head_chars(&revm, 12));
        let first = icr_l.first().copied().unwrap_or("");
        st.ck(
            "★R-b 재렌더 성공 = 형상 실림 · 첫 줄 경고(renderer@rev12) · 헤더에 판본·실행 바이트 아님",
            has(&rgr["files"], "artifacts/compose/.env.interconnect.template")
                && first.starts_with("# ⚠ generated-unverified — ")
                && first.ends_with(&format!("생성: {gb}"))
                && icr.contains("측정 당시 판본 재렌더")
                && icr.contains("실행 바이트 아님")
                && icr.contains(&format!("git show {}", &gb[9..])),
        );
        st.ck(
            "R-b PAYLOAD 파일 기록 = generated-unverified · renderer@rev12 · 기록 header = 파일 첫 줄",
            eqs(&rec_r["verification"], "generated-unverified") && eqs(&rec_r["generated_by"], &gb) && eqs(&rec_r["header"]["text"], first) && eqs(&rec_r["relevance"], "unknown"),
        );
        st.ck(
            "R-b env_shapes 행 = 재렌더 상태 · 판본 · 입력 근거 · 재생성 사실 유지 · excluded 아님 · 결손 코드 없음",
            eqs(&row_r["status"], RERENDER_STATUS)
                && rev_m.as_deref().is_some_and(|r_| eqs(&row_r["renderer_rev"], r_))
                && is_true(&row_r["post_measurement_regenerated"])
                && sv(&row_r["manifest_basis"]).contains("mtime ≤ 측정")
                && !rgr["slots"]["compose"]["excluded"].items().iter().any(|x| eqs(&x["file"], "envs/.env.interconnect"))
                && !has(&rgr["missing"], "HINT_MISSING_ENV_SHAPE_RERENDER"),
        );
        st.ck(
            "R-b 측정 당시 판본의 값(프리셋·불변 리터럴) · 환경값은 자리표시(운영자 값 ✗)",
            icr.contains("NCCL_IB_GID_INDEX=<manifest.interconnect.gid_index>")
                && !icr.contains("enpfixture0")
                && !icr.contains("rocep1s0f1")
                && Re::new(r"(?m)^NCCL_IB_QPS_PER_CONNECTION=4\b", 0).search(&icr).is_some(),
        );
        let rgs = if rgr["sub_recipe"].truthy() { rgr["sub_recipe"].clone() } else { J::dict() };
        st.ck(
            "R-b sub_recipe 가 재렌더 형상을 가리킨다(collect · 공개 sub_recipe() 같은 규칙)",
            eqs(&rgs["per_node_values"]["interconnect"], "artifacts/compose/.env.interconnect.template")
                && subr(&repo, &ev)?["per_node_values"].get("interconnect").is_some(),
        );
        st.ck(
            "★R-b 프로젝트 .env(render 3층 아님) = 재렌더 대상 아님 · 여전히 제외",
            rgr["slots"]["compose"]["excluded"].items().iter().any(|x| eqs(&x["file"], ".env")),
        );
        wt(&man_p, "topology: multi\ninterconnect:\n  hca_devices: rocep1s0f1\n  gid_index: 3\n  socket_iface: enpfixture0\n  platform_preset: no-such-preset\n")?;
        _set_mtime(&man_p, "2026-01-01T00:00:00Z")?;
        let rgx = coll(&repo, &ev, &repo.join("p_rerender_x"), Some(&docker_old))?;
        let row_x = find_in(&rgx["env_shapes"], "file", "envs/.env.interconnect");
        st.ck(
            "★R-b 렌더러 거부(미지 프리셋) = 결손 코드 + 거부 사유 · 형상 미실림",
            has(&rgx["missing"], "HINT_MISSING_ENV_SHAPE_RERENDER")
                && eqs(&row_x["status"], "excluded")
                && sv(&row_x["rerender_error"]).contains("거부")
                && sv(&row_x["rerender_error"]).contains("no-such-preset")
                && !has(&rgx["files"], "artifacts/compose/.env.interconnect.template"),
        );
        Ok(())
    })(st);
    rm(&man_p)?;
    r?;
    // ★R-b Rust 이식 경계(plan_26111615 D4): 측정 당시 렌더러가 이식 전 판본(.py 만 있던 시점)이면 Python 을 돌리지 않고 fail closed.
    //   이식 전 커밋은 과거 날짜로 넣고 바로 지운다(작업트리에 .py 를 남기지 않는다 · 이력에만 있다).
    {
        let pre = repo.join(core::REL_RENDER_DOCKERFILE_PRE_PORT);
        mkd(pre.parent().unwrap())?;
        wt(&pre, "# pre-port renderer (fixture · never run)\n")?;
        git(&repo, &["add", core::REL_RENDER_DOCKERFILE_PRE_PORT], &genv_at("2025-12-01T00:00:00Z")?)?;
        git_commit(&repo, "pre-port renderer", &genv_at("2025-12-01T00:00:00Z")?)?;
        git(&repo, &["rm", "-q", core::REL_RENDER_DOCKERFILE_PRE_PORT], &genv_at("2025-12-02T00:00:00Z")?)?;
        git_commit(&repo, "pre-port renderer retired", &genv_at("2025-12-02T00:00:00Z")?)?;
        let ev_pre = with(&ev, vec![("certificate", p(r#"{"measured_utc": "2025-12-15T00:00:00Z"}"#))]);
        let rr_pre = _rerender_at_measurement(&ctx(&repo, &ev_pre, None)?, "interconnect")?;
        let why = sv(&rr_pre["why"]);
        st.ck(
            "★R-b 측정 당시 렌더러 = Rust 이식 전 판본 → RustPortBoundary fail closed(Python 실행 ✗ · 판본 이름을 댄다)",
            !rr_pre["ok"].truthy() && why.contains("RustPortBoundary") && why.contains("Rust 이식 전") && why.contains(core::REL_RENDER_DOCKERFILE_PRE_PORT),
        );
        st.ck("R-b 이식 경계 픽스처: 작업트리에 .py 렌더러가 없다", !pre.exists());
    }
    st.ck(
        "★R-b generated_by 어휘: renderer@<12hex> 만 확장 · 모양 밖은 거부",
        generated_by_ok(&"renderer@0123456789ab".into())?
            && !generated_by_ok(&"renderer@xyz".into())?
            && !generated_by_ok(&"renderer@0123456789abc".into())?
            && !generated_by_ok(&"someone".into())?,
    );
    let rg3 = coll(&repo, &with(&ev, vec![("certificate", p(r#"{"measured_utc": "2026-01-03T00:00:00Z"}"#))]), &repo.join("p_regen_cert"), Some(&docker_old))?;
    st.ck(
        "★F2 측정 시각 = 인증서 measured_utc 우선(그보다 이른 mtime 은 재생성 아님 → 실린다)",
        rg3["slots"]["compose"]["evidence"]["post_measurement_regenerated"].as_list().is_some_and(|l| l.is_empty())
            && has(&rg3["files"], "artifacts/compose/.env.interconnect.template")
            && !rg3["slots"]["compose"]["excluded"].items().iter().any(|x| eqs(&x["reason"], POST_MEASUREMENT_REGENERATED)),
    );
    let ev_nomeas = with(&ev, vec![("sweep", without(&ev["sweep"], "generated_utc"))]);
    let rg4 = coll(&repo, &ev_nomeas, &repo.join("p_regen_none"), Some(&docker_old))?;
    let ic4 = rt(&repo.join("p_regen_none/artifacts/compose/.env.interconnect.template"))?;
    st.ck(
        "★F2 측정 시각 미관측 = 재생성 판정 ✗(실린다) · 대조하지 못했다고 적는다",
        rg4["slots"]["compose"]["evidence"]["post_measurement_regenerated"].as_list().is_some_and(|l| l.is_empty())
            && ic4.contains("측정 시각 미관측")
            && ic4.contains("NCCL_DEBUG=INFO"),
    );
    // _FakeTierRender as its J form (only the keys this file has).
    let fake_rd = p(r#"{"env_tier": {"NCCL_DEBUG": ["invariant", null], "NCCL_SOCKET_IFNAME": ["env", "manifest.fixture.nccl_socket_ifname"]},
                       "RUNNER_SCRIPTS": ["serve_runner.sh", "debug-init.sh", "arm_patch.sh"]}"#);
    let ish = env_shape(&repo, &out.join("envs/.env.interconnect"), "interconnect", &fake_rd, &J::Null, Some("2026-01-01T19:30:00Z"))?;
    st.ck(
        "env_shape(measured_utc) = 측정 후 재생성 표시 · 리터럴 값 미기재(형상화 함수 · 발행 여부와 별개)",
        ish.contains(&format!("NCCL_DEBUG={REGENERATED_VALUE}")) && !ish.contains("재현 사실"),
    );
    _set_mtime(&out.join("envs/.env.interconnect"), "2026-01-01T19:30:00Z")?;
    st.ck(
        "★F2 경계: mtime == 측정 시각 = 재생성 아님 · 1초 뒤(.env 01-02) = 재생성",
        _regenerated(&ctx(&repo, &ev, None)?, &out.join("envs/.env.interconnect"))?.is_none() && _regenerated(&ctx(&repo, &ev, None)?, &out.join(".env"))?.is_some(),
    );
    for p_ in [out.join("envs/.env.interconnect"), out.join(".env"), ycell.clone()] {
        _set_mtime(&p_, "2026-01-01T00:00:00Z")?;
    }
    // ── F3 arm_patch.sh 는 이 셀이 런타임 패치를 무장할 때만 ──
    let pp = out.join("configs").join(format!("{cell}_patch.py"));
    mv(&pp, &out.join("configs").join(format!("{cell}_patch.py.off")))?;
    let rna = coll(&repo, &ev, &repo.join("p_noarm"), Some(&docker_old));
    let rsa = rna.as_ref().ok().map(|_| ()).map(|_| -> HR<J> {
        wt(&out.join("configs/default_patch.py"), "# slave runtime patch\n")?;
        let r_ = coll(&repo, &ev, &repo.join("p_slavearm"), Some(&docker_old));
        rm(&out.join("configs/default_patch.py"))?;
        r_
    });
    mv(&out.join("configs").join(format!("{cell}_patch.py.off")), &pp)?;
    let rna = rna?;
    st.ck(
        "★F3 무장할 런타임 패치 없음 = arm_patch.sh 미실림 · compose excluded not-armed · serve_runner 는 실림",
        !has(&rna["files"], "artifacts/compose/arm_patch.sh")
            && has(&rna["files"], "artifacts/compose/serve_runner.sh")
            && rna["slots"]["compose"]["excluded"].items().iter().any(|x| eqs(&x["file"], "arm_patch.sh") && sw(&x["why"], "not-armed")),
    );
    let rsa = rsa.unwrap_or_else(|| Ok(J::Null))?;
    st.ck("F3 slave(CONFIG_FILE=default) 가 무장하면 싣는다", has(&rsa["files"], "artifacts/compose/arm_patch.sh"));
    // `getattr(_render_module(repo), "RUNNER_SCRIPTS", ())` → the owner's literal array in its Rust source (render_dockerfile.rs · no `.py`).
    let rd_txt = rt(&repo.join(core::REL_RENDER_DOCKERFILE_SRC))?;
    let rs_lit = Re::new(r"(?m)^pub const RUNNER_SCRIPTS\s*:[^=]*=\s*\[([^\]]*)\]", 0)
        .search(&rd_txt)
        .and_then(|m| m.get(1).map(|x| x.as_str().to_string()))
        .unwrap_or_default();
    st.ck(
        "F3 무장 러너 이름 = 소유자 RUNNER_SCRIPTS 원소(tripwire 교차대조)",
        Re::new(r#"["']([^"']+)["']"#, 0).findall(&rs_lit).iter().any(|x| x == RUNTIME_PATCH_ARMER),
    );
    let res = fx.res.clone();
    st.ck(
        "F2 픽스처 armer(echo 없음) = arming 관측 불가로 표시(관측했다고 말하지 않는다)",
        res["slots"]["runtime_patch"]["file_records"].items().first().is_some_and(|r| has(&r["verification_basis"], "arming 관측 불가")),
    );
    sec_real_armer(st, fx)?;
    sec_bench_cmds(st, fx)?;
    sec_probe(st, fx)?;
    let mut nfix: Vec<PathBuf> = Vec::new();
    let r = sec_native(st, fx, &mut nfix);
    for p_ in &nfix {
        rm_ok(p_);
    }
    r?;
    sec_tail(st, fx)?;
    Ok(())
}

/// `{k for k, v in r_["slots"].items() if v["files"]}`.
fn slots_with_files(r_: &J) -> BTreeSet<String> {
    r_["slots"].as_dict().map(|m| m.iter().filter(|(_, v)| v["files"].truthy()).map(|(k, _)| k.to_string()).collect()).unwrap_or_default()
}
/// `{PurePath(f).name for f in r_["slots"][s_]["files"]}`.
fn slot_names(r_: &J, s_: &str) -> BTreeSet<String> {
    r_["slots"][s_]["files"].items().iter().map(|f| name_of(&f.display())).collect()
}
fn bare_ctx(repo: &Path, ev: &J, cell: &str) -> _Ctx {
    _Ctx::new(repo, ev, None, None, None, None, "", cell, "")
}

// ── native 구조 통일 · 파일 단위 표시 · 관련성 · 빌드 컨텍스트 매핑 ──
fn sec_native(st: &mut St, fx: &mut Fx, nfix: &mut Vec<PathBuf>) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let native_ev = fx.native_ev.clone();
    let ncell = format!("{cell}-native");
    let patches_fx: Vec<(&str, &str)> = vec![
        (
            "70-fx-fire.sh",
            concat!(
                "#!/bin/bash\n# 70 fixture — fx_model 경로\nset -euo pipefail\n",
                "cat >> /workspace/vllm-src/vllm/models/fx_model/layer.py <<'PY'\n",
                "logger.info(\"fixture patch 70 fired for %s\", name)\nPY\n",
            ),
        ),
        (
            "71-fx-other.sh",
            concat!(
                "#!/bin/bash\n# 71 fixture — 다른 모델 경로\nset -euo pipefail\n",
                "cat >> /workspace/vllm-src/vllm/models/other_model/layer.py <<'PY'\n",
                "logger.info(\n    \"fixture patch 71 other model path %s\", name)\nPY\n",
            ),
        ),
        (
            "72-fx-silent.sh",
            concat!(
                "#!/bin/bash\n# 72 fixture — 발화 서명 없음\nset -euo pipefail\n",
                "sed -i 's/a/b/' /workspace/vllm-src/vllm/models/fx_model/other.py\n",
            ),
        ),
    ];
    for (n_, t_) in &patches_fx {
        wt(&out.join("build_patches_src").join(n_), t_)?;
        nfix.push(out.join("build_patches_src").join(n_));
    }
    let mut lpatches: Vec<J> = Vec::new();
    for (n_, _) in &patches_fx {
        lpatches.push(jd(&[
            ("phase", "pre".into()),
            ("file", (*n_).into()),
            ("result", "applied".into()),
            ("result_source", "exit-code".into()),
            ("script_sha256", core::sha256_file(&out.join("build_patches_src").join(n_))?.into()),
        ]));
    }
    lpatches.push(jd(&[
        ("phase", "post".into()),
        ("file", "10-shared.sh".into()),
        ("result", "applied".into()),
        ("result_source", "status-file".into()),
        ("declared_model_trigger", "DeepseekV4 범용".into()),
        ("script_sha256", core::sha256_file(&out.join("build_patches/10-shared.sh"))?.into()),
    ]));
    let led_n = jd(&[
        ("schema_version", 1.into()),
        ("dockerfile", "Dockerfile.source-build".into()),
        ("track", "source-build".into()),
        ("requirements_sha256", core::sha256_file(&out.join("requirements.txt"))?.into()),
        ("patches", J::List(lpatches)),
    ]);
    let att_n = jd(&[
        ("schema_version", 2.into()),
        ("_path", "output/multi/benchlog/attestation_c1-mmp.json".into()),
        ("nodes", jd(&[("main", jd(&[("build_ledger", led_n.clone())])), ("sub", jd(&[("build_ledger", led_n.clone())]))])),
    ]);
    wt(&out.join("configs").join(format!("{ncell}.yaml")), "# native 렌더\nmodel: X\nmax-model-len: 262144\n")?;
    wt(&out.join("configs").join(format!("{ncell}.sh")), "#!/bin/bash\nexec \"${NATIVE_VENV}/bin/vllm\" serve\n")?;
    let src_sha = core::head_chars(&core::sha256_file(&out.join("envs").join(format!(".env.{cell}")))?, 16);
    wt(
        &out.join("envs").join(format!(".env.{ncell}")),
        &format!("# ⚙ 렌더 산출물 — render_native_triplet.sh 가 Docker 셀 env output/multi/envs/.env.{cell} (sha256={src_sha}) 에서 파생.\nCONFIG_FILE={ncell}\n"),
    )?;
    mkd(&repo.join("output/multi/benchlog"))?;
    wt(&repo.join("output/multi/benchlog/fx_engine.log"), "INFO boot\nINFO fixture patch 70 fired for layer.0\n")?;
    for p_ in [
        out.join("configs").join(format!("{ncell}.yaml")),
        out.join("configs").join(format!("{ncell}.sh")),
        out.join("envs").join(format!(".env.{ncell}")),
        repo.join("output/multi/benchlog/fx_engine.log"),
    ] {
        _set_mtime(&p_, "2026-01-01T00:00:00Z")?;
        nfix.push(p_);
    }
    let nat_trip = jd(&[
        ("yaml", format!("output/multi/configs/{ncell}.yaml").into()),
        ("sh", format!("output/multi/configs/{ncell}.sh").into()),
        ("env", format!("output/multi/envs/.env.{ncell}").into()),
    ]);
    let sp_n = jd(&[("plane", "native".into()), ("source_cell", cell.as_str().into()), ("wheelhouse", p(r#"{"source_image_tag": "easy-vllm:fx-source"}"#))]);
    let ev_n = with(
        &native_ev,
        vec![
            ("cell", ncell.as_str().into()),
            ("triplet", nat_trip),
            ("serve_proof", sp_n.clone()),
            ("attestation", att_n.clone()),
            ("build_identity", p(r#"{"image_tag": "easy-vllm:fx-source", "native_wheelhouse_source": true}"#)),
            ("model_architectures", p(r#"["FxModelForCausalLM"]"#)),
            ("model_type", "fx_model".into()),
            ("engine_logs", p(r#"["output/multi/benchlog/fx_engine.log"]"#)),
        ],
    );
    let ev_d = with(
        &ev,
        vec![
            ("attestation", att_n.clone()),
            ("model_architectures", p(r#"["FxModelForCausalLM"]"#)),
            ("model_type", "fx_model".into()),
            ("engine_logs", p(r#"["output/multi/benchlog/fx_engine.log"]"#)),
        ],
    );
    let rn2 = coll(&repo, &ev_n, &repo.join("p_nat_u"), Some(&docker_old))?;
    let rd2 = coll(&repo, &ev_d, &repo.join("p_doc_u"), Some(&docker_old))?;
    let slots_set: BTreeSet<String> = SLOTS.iter().map(|s| s.to_string()).collect();
    let pf: BTreeSet<String> = patches_fx.iter().map(|(n, _)| n.to_string()).collect();
    let mut hn = slots_with_files(&rn2);
    hn.remove("runtime_patch");
    let mut hd_ = slots_with_files(&rd2);
    hd_.remove("runtime_patch");
    st.ck(
        "★native 슬롯 집합 = Docker 셀(런타임 패치 제외) · 슬롯 키 동일 · 빌드 패치 파일 동일",
        keys(&rn2["slots"]) == keys(&rd2["slots"])
            && keys(&rd2["slots"]) == slots_set
            && hn == hd_
            && hd_ == set_of(&["triplet", "build_patch_pre", "build_patch_post", "build_recipe", "compose"])
            && slot_names(&rn2, "build_patch_pre") == slot_names(&rd2, "build_patch_pre")
            && slot_names(&rd2, "build_patch_pre") == pf
            && slot_names(&rn2, "build_patch_post") == slot_names(&rd2, "build_patch_post")
            && slot_names(&rd2, "build_patch_post") == set_of(&["10-shared.sh"]),
    );
    st.ck(
        "★native 빌드 입력 근거 = '원천 이미지 기준(원천 셀 attestation …)' 명시 · evidence 표지 교차 기재",
        ["build_patch_pre", "build_patch_post"]
            .iter()
            .all(|s_| rn2["slots"][*s_]["file_records"].items().iter().all(|r| has(&r["verification_basis"], "원천 이미지 기준(원천 셀 attestation attestation:")))
            && is_true(&rn2["native_source"]["evidence_marker"]),
    );
    st.ck(
        "native build_recipe = 원천 이미지 Dockerfile·requirements + native 고유(install · freeze main/sub)",
        slot_names(&rn2, "build_recipe") == set_of(&["Dockerfile.source-build", "requirements.txt", "native-install.sh", "pip-freeze-main.txt", "pip-freeze-sub.txt"])
            && eqs(&rn2["plane"], "native")
            && eqs(&rn2["track"], "native")
            && is_true(&rn2["native_source"]["ok"])
            && eqs(&rn2["native_source"]["cell"], &cell)
            && is_true(&rn2["native_source"]["render_header_sha_match"])
            && rn2["native_source"].get("triplet_paths").is_none(),
    );
    let mut recs = Map::new();
    if let Some(m) = rn2["slots"].as_dict() {
        for v in m.values() {
            for r in v["file_records"].items() {
                recs.insert(r["path"].display(), r.clone());
            }
        }
    }
    let recs = J::Dict(recs);
    let rec_list: Vec<(String, J)> = recs.as_dict().unwrap().iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    st.ck(
        "★모든 실린 파일에 file_records(verification·generated_by·근거·relevance·근거) — files 와 1:1",
        keys(&recs) == sset(&rn2["files"])
            && rec_list.iter().all(|(_, r)| {
                VERIFICATION_VALUES.contains(&sv(&r["verification"]).as_str())
                    && generated_by_ok(&r["generated_by"]).unwrap_or(false)
                    && RELEVANCE_VALUES.contains(&sv(&r["relevance"]).as_str())
                    && r["verification_basis"].truthy()
                    && r["relevance_basis"].truthy()
                    && sv(&r["sha256"]).chars().count() == 64
            }),
    );
    let comp: Vec<J> = rec_list.iter().filter(|(p_, _)| p_.starts_with("artifacts/compose/")).map(|(_, r)| r.clone()).collect();
    let comp_paths: BTreeSet<String> = comp.iter().map(|r| r["path"].display()).collect();
    st.ck(
        "N1 형: compose·serve_runner·env 형상·sub_recipe·원천 트리플렛 = generated-unverified(renderer) · 그 밖 = verified(cell-run)",
        !comp.is_empty()
            && comp.iter().all(|r| eqs(&r["verification"], "generated-unverified") && eqs(&r["generated_by"], "renderer"))
            && [
                "artifacts/compose/docker-compose.yaml".to_string(),
                "artifacts/compose/serve_runner.sh".to_string(),
                "artifacts/compose/sub_recipe.json".to_string(),
                format!("artifacts/compose/.env.{cell}"),
                format!("artifacts/compose/{cell}.yaml"),
                format!("artifacts/compose/{cell}.sh"),
            ]
            .iter()
            .all(|x| comp_paths.contains(x))
            && rec_list.iter().filter(|(p_, _)| !p_.starts_with("artifacts/compose/")).all(|(_, r)| eqs(&r["verification"], "verified") && eqs(&r["generated_by"], "cell-run")),
    );
    let mut hdr_ok = true;
    for r in &comp {
        let body = rt(&repo.join("p_nat_u").join(r["path"].display()))?;
        let h = if r["header"].truthy() { r["header"].clone() } else { J::dict() };
        if eqs(&h["style"], "json-key") {
            let doc = crate::value::loads(&body).unwrap_or(J::Null);
            hdr_ok &= jeq(&doc[UNVERIFIED_HEADER_KEY], &h["text"])
                && body.lines().nth(1).is_some_and(|l| l.trim().starts_with(&format!("\"{UNVERIFIED_HEADER_KEY}\"")));
        } else {
            let line = h["line"].as_i64().unwrap_or(0);
            let bl: Vec<&str> = body.lines().collect();
            let at = if line >= 1 { bl.get((line - 1) as usize).copied() } else { bl.last().copied() };
            hdr_ok &= at == h["text"].as_str() && has(&h["text"], "⚠ generated-unverified");
            hdr_ok &= (line == 2) == body.starts_with("#!");
        }
    }
    let rsr = if rn2["sub_recipe"].truthy() { rn2["sub_recipe"].clone() } else { J::dict() };
    st.ck(
        "★generated-unverified 파일 전부 첫 줄(셸은 shebang 뒤 2행 · JSON 은 _verification 키) 경고 = 기록된 헤더 · 생성 주체 문구",
        hdr_ok && comp.iter().all(|r| sv(&r["header"]["text"]).contains("생성: renderer")) && sw(&rsr[UNVERIFIED_HEADER_KEY], "⚠ generated-unverified"),
    );
    let rd2sr = if rd2["sub_recipe"].truthy() { rd2["sub_recipe"].clone() } else { J::dict() };
    st.ck(
        "Docker compose 후보의 원본 바이트는 미검증이며 경고를 싣는다",
        rd2["slots"]["compose"]["file_records"]
            .items()
            .iter()
            .filter(|r| matches!(name_of(&r["path"].display()).as_str(), "docker-compose.yaml" | "serve_runner.sh"))
            .all(|r| eqs(&r["verification"], "generated-unverified") && r["header"].truthy())
            && rd2sr.get(UNVERIFIED_HEADER_KEY).is_none(),
    );
    let mut rel_m = Map::new();
    for (p_, r) in &rec_list {
        if p_.contains("/build_patch_") {
            rel_m.insert(name_of(p_), r.clone());
        }
    }
    let rel_ = J::Dict(rel_m);
    st.ck(
        "★관련성: 선언 일치 × 발화 = required · 선언 불일치 × 무발화 = inactive-inferred · 발화 서명 없음·범용 = unknown",
        eqs(&rel_["70-fx-fire.sh"]["relevance"], "required")
            && eqs(&rel_["71-fx-other.sh"]["relevance"], "inactive-inferred")
            && has(&rel_["71-fx-other.sh"]["relevance_basis"], "추론")
            && eqs(&rel_["72-fx-silent.sh"]["relevance"], "unknown")
            && has(&rel_["72-fx-silent.sh"]["relevance_basis"], "서명 없음")
            && eqs(&rel_["10-shared.sh"]["relevance"], "unknown")
            && is_true(&rel_["70-fx-fire.sh"]["relevance_evidence"]["fired"])
            && strs(&rel_["71-fx-other.sh"]["relevance_evidence"]["declared"]["modules"]) == ["other_model"],
    );
    st.ck(
        "★script sha 칸: 원장 script_sha256 = 실린 바이트 → bytes_observed True · 원장 requirements_sha256 일치 → 쓰인 바이트 관측 = 예",
        rel_.as_dict().unwrap().values().all(|r| is_true(&r["bytes_observed"]) && r["script_sha256_ledger"] == r["sha256"])
            && is_true(&recs["artifacts/build_recipe/requirements.txt"]["bytes_observed"])
            && rn2["slots"]["build_recipe"]["evidence"]["selected_revisions"].items().iter().any(|s_| s_["method"].display().contains("빌드 원장 sha256 일치")),
    );
    let ap_d = by(&rd2["applied_set"]["patches"], "file");
    let mut f1 = true;
    for (n_, _) in &patches_fx {
        f1 &= ap_d[*n_]["script_identity"].display().contains("원장 script_sha256 = 작업트리 바이트")
            && eqs(&ap_d[*n_]["ledger_script_sha256"], &core::sha256_file(&out.join("build_patches_src").join(n_))?);
    }
    st.ck("F1 원장 경로 판정 근거: 스크립트 바이트 정체 = 원장 script_sha256 대조 · 원장 sha 를 옮긴다", f1);
    let mut ns = bare_ctx(&repo, &J::dict(), "");
    ns.out = out.clone();
    let z64 = "0".repeat(64);
    let lsi = |row: J| _ledger_script_identity(&ns, &row);
    st.ck(
        "★F1 음성대조: 원장 sha ≠ 작업트리 → '≠' 로 적는다(일치로 접지 않는다) · 파일 없음 → 대조 불가",
        has(&lsi(jd(&[("phase", "pre".into()), ("file", "70-fx-fire.sh".into()), ("script_sha256", z64.as_str().into())]))?["script_identity"], "≠ 작업트리")
            && has(&lsi(jd(&[("phase", "pre".into()), ("file", "99-none.sh".into()), ("script_sha256", z64.as_str().into())]))?["script_identity"], "파일 없음")
            && lsi(jd(&[("phase", "inline".into()), ("file", "x".into()), ("script_sha256", z64.as_str().into())]))?.get("script_identity").is_none(),
    );
    st.ck(
        "F5 기계 파생 사유: fork_pin 부재 = VARIANT 줄 없음 · 빌드 패치 = 적용 판정 요약 · native compose = Docker 형 참고",
        rd2["slots"]["fork_pin"]["machine_reason"].display().contains("VARIANT 줄 없음")
            && rd2["slots"]["build_patch_pre"]["machine_reason"].display().contains("applied")
            && rn2["slots"]["compose"]["machine_reason"].display().contains("Docker 형 참고")
            && rn2["slots"]["compose"]["machine_reason"].display().contains("이 셀 run 아님"),
    );
    st.ck(
        "★F5 음성대조: 기계 사유는 rationale(선언 신호)에 넣지 않는다 — confidence 불변",
        rd2["slots"].as_dict().unwrap().values().all(|v| v["rationale"].is_null() && slot_confidence(v).ok().as_deref() == v["confidence"].as_str()),
    );
    st.ck(
        "★F13 generated-unverified 는 실행 여부를 증명하지 않는다 · 관련성 unknown",
        comp.iter().all(|r| eqs(&r["relevance"], "unknown") && has(&r["relevance_basis"], "경로 실행 여부나 필요 여부를 판정하지 않는다")),
    );
    st.ck(
        "F13 음성대조: Docker 셀 compose(실행됨 · verified) = required",
        rd2["slots"]["compose"]["file_records"]
            .items()
            .iter()
            .filter(|r| eqs(&r["path"], "artifacts/compose/docker-compose.yaml") || eqs(&r["path"], "artifacts/compose/serve_runner.sh"))
            .all(|r| eqs(&r["relevance"], "unknown")),
    );
    let nrev = rn2["slots"]["compose"]["evidence"]["selected_revisions"].items().to_vec();
    st.ck(
        "G5 native(실행 안 된 compose 경로) 개정 행 = executed False · verified None(쓰인 바이트 관측 ✗)",
        !nrev.is_empty() && nrev.iter().all(|x| is_false(&x["executed"]) && x["verified"].is_null()),
    );
    st.ck(
        "★G5 음성대조: Docker 셀(실행됨)은 executed 표지 없음 · verified 불리언 유지",
        rd2["slots"]["compose"]["evidence"]["selected_revisions"].items().iter().all(|x| x.get("executed").is_none() && x["verified"].is_bool()),
    );
    let fr_ = rel_["70-fx-fire.sh"].clone();
    let fsig = fr_["relevance_evidence"]["fired_signatures"].clone();
    st.ck(
        "G4 관련성 근거 = 실제 발화 서명 · 횟수(relevance_evidence.fired_signatures 와 같은 서명)",
        fsig.truthy()
            && fsig.as_dict().is_some_and(|m| {
                m.iter().all(|(k, v)| fr_["relevance_basis"].display().contains(&format!("{} {}회", crate::value::py_repr(&core::head_chars(k, 48)), v.display())))
            }),
    );
    let d14 = _patch_declared(
        "mo=/workspace/vllm-src/vllm/model_executor/layers/quantization/modelopt.py\nm=$DST/vllm/models/fx_model/mtp.py\nr=vllm/model_executor/models/registry.py\n",
        None,
    )?;
    st.ck(
        "F14 공유 파일 = 모델 디렉터리 밖 vLLM 파일(generic 모듈 포함) · 모델 디렉터리 파일 제외",
        strs(&d14["shared_files"]) == ["vllm/model_executor/layers/quantization/modelopt.py", "vllm/model_executor/models/registry.py"]
            && strs(&d14["modules"]) == ["fx_model"],
    );
    st.ck(
        "★F14 음성대조: 모델 디렉터리만 편집하면 공유 파일 없음 · 관련성 근거에 공유 목록 없음",
        _patch_declared(patches_fx[0].1, None)?["shared_files"].as_list().is_some_and(|l| l.is_empty()) && !has(&rel_["70-fx-fire.sh"]["relevance_basis"], "공유"),
    );
    // ── F8 · F11 ──
    mkd(&repo.join("docs/benchmark"))?;
    let lrep = repo.join("docs/benchmark/bench_report_fx_lite.md");
    wt(&lrep, "# 경량 성능 보고서(lite)\n\nmode: lite\n\n> 생성일 2026-01-01T02:00:00Z.\n")?;
    nfix.push(lrep.clone());
    let lev = p(r#"{"report_kind": "lite", "bench_report_path": "docs/benchmark/bench_report_fx_lite.md"}"#);
    st.ck(
        "F8 lite-only 셀 측정 시각 = lite 리포트 생성일(출처 표지)",
        _lite_measured_utc(&repo, &lev)?
            == (
                Some("2026-01-01T02:00:00Z".to_string()),
                "lite bench_report 생성일(bench_report_fx_lite.md 머리 · lite 측정 시작 — 부하 직전 lite_bench.sh MEASURED_UTC)".to_string(),
            ),
    );
    st.ck("★G7 음성대조: lite 리포트 생성일을 '측정 끝' 이라 부르지 않는다(lite_bench.sh 는 부하 직전에 찍는다)", !_lite_measured_utc(&repo, &lev)?.1.contains("측정 끝"));
    let blz = repo.join("output/multi/benchlog");
    mkd(&blz)?;
    core::write_json(&blz.join("lite_raw_cz.json"), &p(r#"{"config_name": "cz", "measured_utc": "2026-01-01T02:00:00Z", "topology": "multi", "backend": "openai-chat"}"#))?;
    let legz = p(r#"{"backend": "openai-chat", "model_id": "fx", "num_prompts": 3, "completed": 3, "total_input_tokens": 1695,
                    "total_output_tokens": 384, "max_concurrency": 1, "request_rate": "inf", "date": "20260101-020100",
                    "spec_decode_acceptance_length": 1.81}"#);
    core::write_json(&blz.join("lite_warm_cz.json"), &legz)?;
    let mut evz = crate::evidence::new_cell_evidence("", None, "cz", "", "multi");
    evz.set("mode", J::Null);
    evz.set("node", J::Null);
    evz.set("repo", core::posix(&repo));
    evz.set("report_kind", "lite");
    evz.set("bench_report_path", "docs/benchmark/bench_report_fx_lite.md");
    evz.set("sources", J::dict());
    let cz = ctx(&repo, &evz, None)?;
    cz.memo_set(
        &jl!["lite_tool_args"],
        p(r#"{"flags": [["--dataset-name", "random"], ["--random-input-len", "512"], ["--random-range-ratio", "0"], ["--ignore-eos", null],
                        ["--save-result", null], ["--result-dir", "/tmp"]], "source": "lite_bench.sh@fixture L1-L4"}"#),
    );
    let (cmdz, srcz) = _bench_commands(&cz)?;
    st.ck(
        "G3 · G9 lite-only: 레그 JSON 명령 · 도구 원문 --random-input-len 512(평균 565 ✗) · 실측 토큰 주석 · 호출 재구성 · 수용 길이",
        cmdz.contains("--random-input-len 512")
            && !cmdz.contains("--random-input-len 565")
            && cmdz.contains("입력 1695/3 = 565")
            && cmdz.contains("--random-range-ratio 0")
            && cmdz.contains("--ignore-eos")
            && cmdz.contains("lite_bench.sh cz --topology multi --backend openai-chat --publish-report")
            && cmdz.contains("spec_decode_acceptance_length 1.81")
            && cmdz.contains("lite_warm_cz.json")
            && srcz.ends_with("lite 인자)"),
    );
    st.ck(
        "★G3 음성대조: 저장 · 출력 경로 인자(--save-result · --result-dir)는 재현 부하가 아니다 — 싣지 않는다",
        !cmdz.contains("--save-result") && !cmdz.contains("--result-dir"),
    );
    cz.memo_set(&jl!["lite_tool_args"], J::Null);
    let (cmdz2, srcz2) = _bench_commands(&cz)?;
    st.ck(
        "★G3 음성대조: 도구 원문을 못 읽으면 평균을 인자로 쓰되 인자 옆에 '평균 · 원문 미관측' 을 밝힌다",
        cmdz2.contains("--random-input-len 565") && cmdz2.contains("total ÷ completed 평균") && srcz2 == "reconstructed(bench json)",
    );
    rm(&blz.join("lite_warm_cz.json"))?;
    st.ck(
        "★G9 음성대조: 레그 JSON 이 없으면 lite-only 사유로 미관측(스윕 사유 ✗)",
        _bench_commands(&ctx(&repo, &evz, None)?)?.1.starts_with("unobserved(lite-only 셀"),
    );
    st.ck("★F3 음성대조: 측정 조인 로그가 없으면(씨앗도 없으면) 엔진 로그 0개", _engine_log_paths(&ctx(&repo, &evz, None)?)?.is_empty());
    wt(&blz.join("lite_engine_cz.log"), "(EngineCore pid=1) INFO 01-01 01:59:00 x\n")?;
    let elp: Vec<String> = _engine_log_paths(&ctx(&repo, &evz, None)?)?.iter().map(|q| q.file_name().unwrap().to_string_lossy().to_string()).collect();
    st.ck(
        "F3 관련성 ② 엔진 로그 = lite_raw 조인(measured_utc = 리포트 생성일)의 lite_engine — 증거 기록 씨앗에 없어도",
        elp == ["lite_engine_cz.log"],
    );
    rm(&blz.join("lite_engine_cz.log"))?;
    rm(&blz.join("lite_raw_cz.json"))?;
    let ft = _fired_text(&["PLE mmap: madvise(".to_string(), "PLE mmap stats (last".to_string()], &p(r#"{"PLE mmap stats (last": 66}"#), Some(true))?;
    st.ck("G4 발화 근거 = 매치된 서명 · 횟수(목록 첫째 ✗)", ft.contains("'PLE mmap stats (last' 66회") && !ft.contains("madvise"));
    st.ck(
        "★G4 음성대조: 무발화면 서명 이름을 발화 예시로 찍지 않는다",
        _fired_text(&["PLE mmap: madvise(".to_string()], &J::dict(), Some(false))? == "무발화(서명 1개 전부 0회)",
    );
    mkd(&repo.join("docs/simlog/fx_wd/logs"))?;
    for nm_ in ["main-watchdog-vllm-serve.log", "main-vllm-serve.log"] {
        wt(&repo.join("docs/simlog/fx_wd/logs").join(nm_), "x\n")?;
    }
    let cw = ctx(
        &repo,
        &p(r#"{"topology": "multi", "cell": "cw", "lineage_seeds": {"evidence_candidates": [
              {"kind": "engine_log", "path": "docs/simlog/fx_wd/logs/main-watchdog-vllm-serve.log"},
              {"kind": "engine_log", "path": "docs/simlog/fx_wd/logs/main-vllm-serve.log"}]}}"#),
        None,
    )?;
    let cwp: Vec<String> = _engine_log_paths(&cw)?.iter().map(|q| q.file_name().unwrap().to_string_lossy().to_string()).collect();
    st.ck("★G4 음성대조: 계보 씨앗으로 들어온 워치독 로그도 엔진 로그로 세지 않는다", cwp == ["main-vllm-serve.log"]);
    st.ck(
        "★F8 음성대조: lite 가 아니면(full 리포트) 이 원천을 쓰지 않는다",
        _lite_measured_utc(&repo, &p(r#"{"report_kind": null, "bench_report_path": "docs/benchmark/bench_report_fx_lite.md"}"#))?.0.is_none(),
    );
    let ndir = repo.join("docs/simlog/fx_n1");
    mkd(&ndir.join("logs"))?;
    core::write_json(&ndir.join("logs/state.json"), &p(r#"{"created_utc": "2026-01-01T00:00:00Z", "up_completed_utc": "2026-01-01T00:30:00Z"}"#))?;
    core::write_json(&ndir.join("wheelhouse-build-main.json"), &p(r#"{"elapsed_s_monotonic": 400.5}"#))?;
    let spx = jd(&[
        ("evidence_dir", "docs/simlog/fx_n1".into()),
        ("health", p(r#"{"ready_after_s": 600}"#)),
        ("run_id", "r1".into()),
        ("cell", ncell.as_str().into()),
        ("source_cell", cell.as_str().into()),
    ]);
    let cx = bare_ctx(&repo, &jd(&[("serve_proof", spx.clone())]), &ncell);
    let ntx = _native_run_timing(&cx)?;
    st.ck(
        "F11 native run 시각: install = created → serve 시작(상한) · serve = up_completed − ready_after_s → up_completed",
        eqs(&ntx["created"], "2026-01-01T00:00:00Z")
            && eqs(&ntx["serve_start"], "2026-01-01T00:20:00Z")
            && eqs(&ntx["up_completed"], "2026-01-01T00:30:00Z")
            && jeq(&ntx["wheelhouse"], &p(r#"{"main": 400.5}"#))
            && sv(&ntx["serve_src"]).contains("ready_after_s=600"),
    );
    let (scx, ssx) = _native_serve_command(&cx)?;
    st.ck(
        "F11 native serve 명령 = native 정문 up(serve proof cell · run_id · source_cell) · down 정리 · 재구성 표지",
        scx.contains(&format!("bash {REL_NATIVE_SERVE} up --cell {ncell} --run-id r1 --source-cell {cell} --apply"))
            && scx.contains("down --cell")
            && ssx.starts_with("reconstructed(serve proof")
            && !scx.contains("--plane native"),
    );
    let empty_sp = jd(&[("serve_proof", J::dict())]);
    st.ck(
        "★F11 음성대조: evidence_dir 절대경로 · 부재 → 시각 없음(합성 ✗) · run_id 없음 → 자리표시 + derived",
        jeq(&_native_run_timing(&bare_ctx(&repo, &jd(&[("serve_proof", with(&spx, vec![("evidence_dir", core::posix(&ndir).into())]))]), &ncell))?, &J::dict())
            && jeq(&_native_run_timing(&bare_ctx(&repo, &empty_sp, &ncell))?, &J::dict())
            && _native_serve_command(&bare_ctx(&repo, &empty_sp, &ncell))?.1.starts_with("derived("),
    );
    let _ = std::fs::remove_dir_all(&ndir);
    let rn_u = coll(&repo, &without(&without(&ev_n, "model_architectures"), "model_type"), &repo.join("p_nat_u2"), Some(&docker_old))?;
    st.ck(
        "★모델 config 미관측 → 모든 패치 관련성 unknown",
        ["build_patch_pre", "build_patch_post"].iter().all(|s_| rn_u["slots"][*s_]["file_records"].items().iter().all(|r| eqs(&r["relevance"], "unknown"))),
    );
    let rn_nolog = coll(&repo, &with(&ev_n, vec![("engine_logs", J::list())]), &repo.join("p_nat_u3"), Some(&docker_old))?;
    st.ck(
        "★엔진 로그 미관측 → 선언 일치여도 required ✗(unknown)",
        rn_nolog["slots"]["build_patch_pre"]["file_records"]
            .items()
            .iter()
            .find(|r| name_of(&r["path"].display()) == "70-fx-fire.sh")
            .is_some_and(|r| eqs(&r["relevance"], "unknown")),
    );
    let m2 = sset(&rn2["missing"]);
    st.ck(
        "결손: native 기동 기록 부재 → HINT_MISSING_NATIVE_LAUNCH · HINT_MISSING_SUB_RECIPE · Agent 요청(생성 ✗) · 적용 집합 관측",
        m2.contains("HINT_MISSING_NATIVE_LAUNCH")
            && m2.contains("HINT_MISSING_SUB_RECIPE")
            && !m2.contains("HINT_MISSING_APPLIED_SET")
            && rn2["agent_requests"].items().iter().map(|r| (r["slot"].display(), r["name"].display())).collect::<Vec<_>>() == [("triplet".to_string(), NATIVE_LAUNCH_NAME.to_string())]
            && !exists(&repo.join("p_nat_u/artifacts/triplet").join(NATIVE_LAUNCH_NAME))
            && !has(&rd2["missing"], "HINT_MISSING_NATIVE_LAUNCH")
            && rd2["agent_requests"].as_list().is_some_and(|l| l.is_empty()),
    );
    wt(&out.join("native-launch.md"), "# 기동 기록 fixture\nray start --head\n")?;
    nfix.push(out.join("native-launch.md"));
    let rn_l = coll(&repo, &with(&ev_n, vec![("serve_proof", with(&sp_n, vec![("native_launch_path", "output/multi/native-launch.md".into())]))]), &repo.join("p_nat_l"), Some(&docker_old))?;
    let lrec = by(&rn_l["slots"]["triplet"]["file_records"], "path").get(&format!("artifacts/triplet/{NATIVE_LAUNCH_NAME}")).cloned();
    let ml = sset(&rn_l["missing"]);
    st.ck(
        "native 기동 기록이 보존되면 triplet 에 verified 로 싣고 NATIVE_LAUNCH·SUB_RECIPE 결손 ✗",
        lrec.as_ref().is_some_and(|r| eqs(&r["verification"], "verified"))
            && !ml.contains("HINT_MISSING_NATIVE_LAUNCH")
            && !ml.contains("HINT_MISSING_SUB_RECIPE")
            && rn_l["agent_requests"].as_list().is_some_and(|l| l.is_empty()),
    );
    let rn_mm = coll(&repo, &with(&ev_n, vec![("serve_proof", with(&sp_n, vec![("wheelhouse", p(r#"{"source_image_tag": "easy-vllm:other"}"#))]))]), &repo.join("p_nat_mm"), Some(&docker_old))?;
    st.ck(
        "★원천 셀 env IMAGE_TAG ≠ wheelhouse 원천 이미지 → 묶지 않는다(빌드 입력 ✗ · APPLIED_SET 결손 · 사유 기재)",
        is_false(&rn_mm["native_source"]["ok"])
            && has(&rn_mm["native_source"]["why"], "IMAGE_TAG")
            && has(&rn_mm["missing"], "HINT_MISSING_APPLIED_SET")
            && !rn_mm["slots"]["build_patch_pre"]["files"].truthy()
            && !rn_mm["files"].items().iter().any(|f| f.display().contains("Dockerfile")),
    );
    let bmap = by(&rn2["build_context_map"], "slot_path");
    let cpth = |k: &str| bmap[k]["context_path"].clone();
    st.ck(
        "★build_context_map: 패치 → build_patches_src/·build_patches/ · env 형상 → envs/ · 러너 → configs/(volume) · 셀 env → envs/.env.<cell> · native 트리플렛 → 원래 자리 · sub_recipe = 컨텍스트 밖",
        eqs(&cpth("artifacts/build_patch_pre/70-fx-fire.sh"), "build_patches_src/70-fx-fire.sh")
            && eqs(&cpth("artifacts/build_patch_post/10-shared.sh"), "build_patches/10-shared.sh")
            && eqs(&cpth("artifacts/build_recipe/requirements.txt"), "requirements.txt")
            && eqs(&cpth("artifacts/build_recipe/Dockerfile.source-build"), "Dockerfile.source-build")
            && eqs(&cpth("artifacts/compose/serve_runner.sh"), "configs/serve_runner.sh")
            && eqs(&cpth("artifacts/compose/.env.cluster.template"), "envs/.env.cluster")
            && eqs(&cpth(&format!("artifacts/compose/.env.{cell}")), &format!("envs/.env.{cell}"))
            && eqs(&cpth(&format!("artifacts/triplet/{ncell}.yaml")), &format!("configs/{ncell}.yaml"))
            && bmap["artifacts/compose/sub_recipe.json"].get("context_path").is_some_and(J::is_null)
            && bmap["artifacts/build_recipe/native-install.sh"].get("context_path").is_some_and(J::is_null)
            && keys(&bmap) == sset(&rn2["files"]),
    );
    st.ck(
        "native 재현 절차: 원천 이미지 build 단계가 install 앞",
        rn2["reproduce_steps"].items().iter().take(2).map(|s_| s_["step"].display()).collect::<Vec<_>>() == ["build", "install"],
    );
    let att_bad = with(&att_n, vec![("nodes", jd(&[("main", jd(&[("build_ledger", with(&led_n, vec![("requirements_sha256", z64.as_str().into())]))]))]))]);
    let rq_bad = coll(&repo, &with(&ev_d, vec![("attestation", att_bad)]), &repo.join("p_doc_rq"), Some(&docker_old))?;
    let mut sq_bad = Map::new();
    for x in rq_bad["slots"]["build_recipe"]["evidence"]["selected_revisions"].items() {
        sq_bad.insert(name_of(&x["path"].display()), x.clone());
    }
    let sq_bad = J::Dict(sq_bad);
    let rq_rec = by(&rq_bad["slots"]["build_recipe"]["file_records"], "path")["artifacts/build_recipe/requirements.txt"].clone();
    st.ck(
        "★원장 requirements_sha256 ≠ 실린 바이트 → verified ✗ · bytes_observed False",
        is_false(&sq_bad["requirements.txt"]["verified"]) && is_false(&rq_rec["bytes_observed"]),
    );
    let rq_txt = rt(&repo.join("p_doc_rq/artifacts/build_recipe/requirements.txt"))?;
    st.ck(
        "★F7 원장 sha256 불일치 requirements = generated-unverified(renderer) · 파일 머리 경고 · 근거에 두 sha",
        eqs(&rq_rec["verification"], "generated-unverified")
            && eqs(&rq_rec["generated_by"], "renderer")
            && rq_rec["header"].truthy()
            && rq_txt.starts_with("# ")
            && has(&rq_rec["verification_basis"], "빌드 원장 sha256 0000000000000000 과 다르다")
            && eqs(&rq_rec["relevance"], "unknown"),
    );
    st.ck(
        "F7 음성대조: 원장 sha256 일치 requirements 는 verified · cell-run 유지(머리 경고 없음)",
        eqs(&recs["artifacts/build_recipe/requirements.txt"]["verification"], "verified") && recs["artifacts/build_recipe/requirements.txt"].get("header").is_none(),
    );
    let mut slots_x = p(r#"{"compose": {"files": ["artifacts/compose/docker-compose.yaml"], "evidence": {}}}"#);
    let marks = p(r#"{"artifacts/compose/docker-compose.yaml": {"verification": "generated-unverified", "generated_by": "renderer"}}"#);
    let rctx = p(r#"{"_per": {}, "_logs": [], "model": {}}"#);
    let cd = ctx(&repo, &ev_d, None)?;
    st.raises("★generated-unverified 인데 헤더 없음 = 표시 불일치 차단", "HINT_ARTIFACT_MARK_INVALID", || {
        _file_records(&cd, &repo.join("p_doc_u"), &mut slots_x, &marks, &J::Null, &rctx)
    });
    Ok(())
}

// ── 헤더 삽입 단위 · env 형상 키별 자리표시 · slave_forward 거부 · lockset 소유자 교차대조 ──
fn sec_tail(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let hd = repo.join("p_hdr");
    mkd(&hd.join("artifacts/compose"))?;
    wt(&hd.join("artifacts/compose/a.sh"), "#!/bin/bash\necho x\n")?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(hd.join("artifacts/compose/a.sh"), std::fs::Permissions::from_mode(0o755)).map_err(|e| ioe(e, &hd))?;
    }
    wt(&hd.join("artifacts/compose/b.yaml"), "k: v\n")?;
    wt(&hd.join("artifacts/compose/c.json"), "{\"z\": 1}\n")?;
    wt(&hd.join("artifacts/compose/d.json"), "[1]\n")?;
    let h1 = _insert_header(&hd, "artifacts/compose/a.sh", "사유", "renderer")?;
    let h2 = _insert_header(&hd, "artifacts/compose/b.yaml", "사유", "agent")?;
    let h3 = _insert_header(&hd, "artifacts/compose/c.json", "사유", "renderer")?;
    let a_txt = rt(&hd.join("artifacts/compose/a.sh"))?;
    let a_mode = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(hd.join("artifacts/compose/a.sh")).map_err(|e| ioe(e, &hd))?.permissions().mode()
    };
    let cdoc = crate::value::loads(&rt(&hd.join("artifacts/compose/c.json"))?).unwrap_or(J::Null);
    let uh = unverified_header("사유", "renderer")?;
    st.ck(
        "헤더 삽입: shebang 뒤 2행 · 실행 비트 보존 · yaml 1행 · JSON _verification 키 · 문구 = unverified_header",
        a_txt.lines().take(2).collect::<Vec<_>>() == vec!["#!/bin/bash".to_string(), format!("# {uh}")]
            && h1["line"].as_i64() == Some(2)
            && a_mode & 0o111 != 0
            && rt(&hd.join("artifacts/compose/b.yaml"))?.starts_with("# ⚠ generated-unverified — 사유 · 생성: agent\n")
            && h2["line"].as_i64() == Some(1)
            && jeq(&cdoc, &jd(&[("z", 1.into()), (UNVERIFIED_HEADER_KEY, uh.as_str().into())]))
            && eqs(&h3["style"], "json-key"),
    );
    st.raises("★JSON 최상위 목록 = 경고 키 삽입 불가 차단", "HINT_ARTIFACT_HEADER_UNSUPPORTED", || {
        _insert_header(&hd, "artifacts/compose/d.json", "사유", "renderer")
    });
    // ★env 형상 키마다 자기 자리표시(실물 렌더러)
    let icp = repo.join("p_ic/.env.interconnect");
    mkd(icp.parent().unwrap())?;
    wt(&icp, "NCCL_IB_DISABLE=0\nNCCL_NET=IB\nNCCL_SOCKET_IFNAME=enpfixture0\nGLOO_SOCKET_IFNAME=enpfixture0\nNCCL_IB_GID_INDEX=3\nNCCL_DEBUG=INFO\n")?;
    let mut dv: Vec<J> = Vec::new();
    let (ic_txt, _) = _env_shape_d(&repo, &icp, "interconnect", &J::Null, &J::Null, &J::Null, None, Some(&mut dv), &J::Null)?;
    let mut kv = Map::new();
    for ln in ic_txt.lines() {
        if ln.contains('=') && !ln.starts_with('#') {
            let head = ln.split("    #").next().unwrap_or("");
            if let Some((k, v)) = head.split_once('=') {
                kv.insert(k, v);
            }
        }
    }
    let kvj = J::Dict(kv);
    let dvf = by(&J::List(dv), "field");
    let vals: BTreeSet<String> = kvj.as_dict().unwrap().values().map(J::display).collect();
    st.ck(
        "★키마다 자기 자리표시: nccl_transport 2키·socket_iface 2키가 서로 다른 표지 · 단일 파생 키는 <manifest.…> 유지 · 실효값 표",
        eqs(&kvj["NCCL_IB_DISABLE"], "<derived:interconnect.nccl_transport→NCCL_IB_DISABLE>")
            && eqs(&kvj["NCCL_NET"], "<derived:interconnect.nccl_transport→NCCL_NET>")
            && eqs(&kvj["NCCL_SOCKET_IFNAME"], "<derived:interconnect.socket_iface→NCCL_SOCKET_IFNAME>")
            && eqs(&kvj["NCCL_IB_GID_INDEX"], "<manifest.interconnect.gid_index>")
            && eqs(&kvj["NCCL_DEBUG"], "INFO")
            && vals.len() == kvj.len()
            && jeq(
                &dvf["interconnect.nccl_transport"]["effective_values"],
                &p(r#"{"rdma": {"NCCL_IB_DISABLE": "0", "NCCL_NET": "IB"}, "socket": {"NCCL_IB_DISABLE": "1", "NCCL_NET": "Socket"}}"#),
            )
            && jeq(
                &dvf["interconnect.socket_iface"]["value_rule"],
                &p(r#"{"NCCL_SOCKET_IFNAME": "{interconnect.socket_iface}", "GLOO_SOCKET_IFNAME": "{interconnect.socket_iface}"}"#),
            )
            && ic_txt.contains("rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB"),
    );
    // ★slave_forward 거부(SystemExit)는 HintError 로 번역된다
    let envc = out.join("envs").join(format!(".env.{cell}"));
    wt(&envc, &format!("{}VLLM_REPO=bad value\n", rt(&envc)?))?;
    st.raises("★slave_forward 거부 번역", "HINT_SUB_RECIPE_DERIVE_FAILED", || subr(&repo, &ev));
    // lockset 출처 키 ↔ 소유자(campaign_validator.rs KNOB_SOURCES) 교차대조 — 소유 Rust 원본의 리터럴만 읽는다(.py ✗).
    match owner_lockset_sources(&fx.own.join(".claude/skills/adversarial-benchmark/rust/src/campaign_validator.rs")) {
        Err(e) => st.bad.push(format!("artifacts: lockset 소유자 교차대조 불가 — {e}")),
        Ok(None) => st.bad.push("artifacts: LOCKSET_KNOB_SOURCES 리터럴을 찾지 못했다(소유자 이동 — 교차대조 갱신)".to_string()),
        Ok(Some(owner)) => {
            let ok: BTreeSet<String> = owner.iter().map(|(k, _)| k.clone()).collect();
            st.ck("LOCKSET_SOURCE_KNOB 키 = 소유자 키(campaign_template_validator.LOCKSET_KNOB_SOURCES)", ok == keys(LOCKSET_SOURCE_KNOB()));
            st.ck("출처 어휘 전부에 지위 후보", owner.iter().all(|(_, vals)| vals.iter().all(|v| _SOURCE_STATUS().get(v).is_some())));
        }
    }
    Ok(())
}

/// `Err` = unreadable (OSError/SyntaxError/ValueError → its type name).
/// The lockset knob → source vocabulary of the owner's **Rust** source (adversarial-benchmark `campaign_validator.rs`
/// `pub const KNOB_SOURCES: [(&str, &[&str]); N] = [ ("knob", &["a", "b"]), … ];` — one knob per line · plan_26111615 D4:
/// the retired `campaign_template_validator.py` literal is never read). `Ok(None)` = the literal moved.
fn owner_lockset_sources(path: &Path) -> Result<Option<Vec<(String, Vec<String>)>>, String> {
    let text = std::fs::read_to_string(path).map_err(|_| "OSError".to_string())?;
    let Some(start) = text.lines().position(|l| Re::new(r"^pub const KNOB_SOURCES\s*:[^=]*=\s*\[\s*$", 0).search(l).is_some()) else {
        return Ok(None);
    };
    let entry = Re::new(r#"^\s*\(\s*"([^"]+)"\s*,\s*&\[([^\]]*)\]\s*\)\s*,?\s*$"#, 0);
    let item = Re::new(r#""([^"]+)""#, 0);
    let mut out = Vec::new();
    for l in text.lines().skip(start + 1) {
        if l.trim_start().starts_with("];") {
            break;
        }
        let m = entry.search(l).ok_or("SyntaxError")?;
        let k = m.get(1).map(|x| x.as_str().to_string()).unwrap_or_default();
        let v = m.get(2).map(|x| x.as_str().to_string()).unwrap_or_default();
        out.push((k, item.findall(&v)));
    }
    if out.is_empty() {
        return Err("ValueError".to_string());
    }
    Ok(Some(out))
}

fn fs_with(base: &[(String, FsEnt)], path: &str, ent: FsEnt) -> Vec<(String, FsEnt)> {
    let mut v: Vec<(String, FsEnt)> = base.iter().filter(|(k, _)| k != path).cloned().collect();
    v.push((path.to_string(), ent));
    v
}
fn a1(a: &[String]) -> Option<&str> {
    a.first().map(String::as_str)
}

// ── F12: 원장 없는 옛 이미지 재구성 v2 — 정적 분석 × 이미지 탐침(대역 docker) ──
fn sec_probe(st: &mut St, fx: &mut Fx) -> HR<()> {
    let repo = fx.repo.clone();
    let so = repo.join("output").join("single");
    for d in ["configs", "envs", "build_patches_src", "build_patches"] {
        mkd(&so.join(d))?;
    }
    let pc = "p1";
    wt(&so.join("configs").join(format!("{pc}.yaml")), "model: /app/models/P\nmax-model-len: 4096\n")?;
    wt(
        &so.join("configs").join(format!("{pc}.sh")),
        "#!/bin/bash\nif [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi\nvllm serve\n",
    )?;
    wt(&so.join("envs").join(format!(".env.{pc}")), "IMAGE_TAG=easy-vllm:fx-probe\nBUILD_DOCKERFILE=Dockerfile.probe\n")?;
    wt(
        &so.join("docker-compose.yaml"),
        &format!(
            concat!(
                "x-gpu-common: &gpu-common\n  image: ${{IMAGE_TAG:-easy-vllm:fx-probe}}\n  build:\n    context: .\n",
                "    dockerfile: ${{BUILD_DOCKERFILE:-Dockerfile.probe}}\n    args:\n      SM12X_PORT: ${{SM12X_PORT:-0}}\n",
                "services:\n  vllm:\n    <<: *gpu-common\n    env_file:\n      - path: envs/.env.${{CONFIG_FILE:-default}}\n",
                "    command: [\"bash\", \"/app/configs/{pc}.sh\"]\n",
            ),
            pc = pc
        ),
    )?;
    wt(
        &so.join("Dockerfile.probe"),
        concat!(
            "FROM base\nARG SM12X_PORT=0\nARG FX_OPEN=0\nARG FX_SHUT=0\nCOPY requirements.txt /tmp/requirements.txt\n",
            "COPY build_patches_src/ /tmp/build_patches_src/\n",
            "RUN for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done\n",
            "COPY build_patches/ /tmp/build_patches/\n",
            "RUN for p in $(ls /tmp/build_patches/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done\n",
        ),
    )?;
    let req: &[u8] = b"transformers<5.15.0\n";
    wb(&so.join("requirements.txt"), req)?;
    let loud = |n: i64| format!("#!/bin/bash\nset -euo pipefail\nDST=\"/workspace/vllm-src\"\necho \"[{n}] apply\"\n");
    let sp: Vec<(&str, String)> = vec![
        (
            "50-gate.sh",
            concat!(
                "#!/bin/bash\nset -euo pipefail\n: \"${SM12X_PORT:=0}\"\nif [ \"${SM12X_PORT}\" != \"1\" ]; then\n",
                "    echo \"[50] skip — SM12X_PORT=0\"\n    exit 0\nfi\necho applying\n",
            )
            .to_string(),
        ),
        (
            "55-open.sh",
            concat!(
                "#!/bin/bash\nset -euo pipefail\n: \"${FX_OPEN:=0}\"\nif [ \"${FX_OPEN}\" != \"1\" ]; then\n",
                "    echo \"[55] skip — FX_OPEN=0\"\n    exit 0\nfi\necho \"[55] applying\"\n",
            )
            .to_string(),
        ),
        (
            "56-two.sh",
            concat!(
                "#!/bin/bash\nset -euo pipefail\n: \"${FX_OPEN:=0}\"\n: \"${FX_SHUT:=0}\"\n",
                "if [ \"${FX_OPEN}\" != \"1\" ]; then\n    exit 0\nfi\nif [ \"${FX_SHUT}\" != \"1\" ]; then\n    exit 0\nfi\n",
                "echo \"[56] applying\"\n",
            )
            .to_string(),
        ),
        (
            "60-loud.sh",
            concat!(
                "#!/bin/bash\nset -euo pipefail\nTAG=\"[60]\"\nDST=\"/workspace/vllm-src\"\n",
                "[ -d \"$DST/vllm\" ] || { echo \"$TAG FAIL\" >&2; exit 1; }\npython3 - \"$DST\" <<'PY'\nimport sys\n",
                "p = sys.argv[1] + \"/vllm/a.py\"\ns = open(p).read()\nif \"anchor\" not in s:\n    raise SystemExit(1)\n",
                "open(p, \"w\").write(s.replace(\"anchor\", \"anchor  # [port:60-fx]\"))\nPY\n",
                "grep -q 'port:60-fx' \"$DST/vllm/a.py\" || { echo \"$TAG FAIL: 마커 부재\" >&2; exit 1; }\n",
                "if grep -q 'already-upstream' \"$DST/vllm/a.py\"; then echo \"$TAG FAIL\" >&2; exit 1; fi\n",
            )
            .to_string(),
        ),
        ("61-drift.sh", "#!/bin/bash\nset -e\nif [ -f /x ]; then exit 0; fi\necho \"[61] apply\"\n".to_string()),
        ("63-notbaked.sh", loud(63)),
    ];
    let pp: Vec<(&str, String)> = vec![
        (
            "10-loud.sh",
            concat!(
                "#!/bin/bash\nset -e\nrm -rf -- /tmp/fx-build 2>/dev/null || true\n",
                "python3 -c \"import importlib.util,sys; sys.exit(0 if importlib.util.find_spec('fx') else 1)\" \\\n",
                "    && echo \"[10] OK\"\n",
            )
            .to_string(),
        ),
        ("15-noe.sh", "#!/bin/bash\necho \"[15] apply\"\n".to_string()),
        ("16-var.sh", "#!/bin/bash\nset -e\nrc=0\nexit $rc\n".to_string()),
        (
            "20-cond.sh",
            concat!(
                "#!/bin/bash\nset -e\nLOC=$(python3 -c \"print('')\" 2>/dev/null || true)\ncase \"$LOC\" in\n",
                "    \"\") echo \"[20] 부재 — skip(이미 vendored)\" ;;\n    *) pip uninstall -y fx >/dev/null 2>&1 || true ;;\n",
                "esac\npython3 -c \"\nassert has_it(), 'FAIL'\n\"\n",
            )
            .to_string(),
        ),
        (
            "30-cond.sh",
            concat!(
                "#!/bin/bash\nset -e\nF=/workspace/vllm-src/gate.py\n[ -f \"$F\" ] || { echo FAIL >&2; exit 1; }\n",
                "python3 - \"$F\" <<'PY'\nimport sys\nf = sys.argv[1]; s = open(f).read()\nif \"< (13, 0)\" in s:\n",
                "    print(\"[30] 이미 완화됨 — skip\")\nelif \"< (11, 0)\" in s:\n",
                "    open(f, \"w\").write(s.replace(\"< (11, 0)\", \"< (13, 0)\"))\nelse:\n    sys.exit(1)\nPY\n",
                "grep -qF \"(9, 0) <= cap < (13, 0)\" \"$F\" && echo \"[30] OK\"\n",
            )
            .to_string(),
        ),
        (
            "40-mod.sh",
            concat!(
                "#!/bin/bash\nset -e\nF=$(python3 -c \"import fxmod.dev as d; print(d.__file__)\" 2>/dev/null || true)\n",
                "[ -n \"$F\" ] && [ -f \"$F\" ] || { echo FAIL >&2; exit 1; }\npython3 - \"$F\" <<'PY'\nimport sys\n",
                "f = sys.argv[1]; s = open(f).read()\nif \"[GB10] guard-a\" in s:\n",
                "    print(\"[40] 이미 가드됨 — skip\"); sys.exit(0)\nopen(f, \"w\").write(s + \"\\n# [GB10] guard-a\\n\")\nPY\n",
                "python3 -c \"\nimport fxmod.dev as d\nsrc = open(d.__file__).read()\n",
                "assert '[GB10] guard-a' in src, 'guard 미적용'\n\"\n",
            )
            .to_string(),
        ),
    ];
    for (name, text) in &sp {
        wt(&so.join("build_patches_src").join(name), text)?;
    }
    for (name, text) in &pp {
        wt(&so.join("build_patches").join(name), text)?;
    }
    let baked61 = loud(61).into_bytes();
    let baked62 = loud(62).into_bytes();
    let mut src_dir: Vec<(String, Vec<u8>)> =
        sp.iter().filter(|(n, _)| *n != "61-drift.sh" && *n != "63-notbaked.sh").map(|(n, t)| (n.to_string(), t.clone().into_bytes())).collect();
    src_dir.push(("61-drift.sh".into(), baked61.clone()));
    src_dir.push(("62-bakedonly.sh".into(), baked62.clone()));
    src_dir.push(("PROVENANCE.json".into(), b"{}".to_vec()));
    let img_fs: Vec<(String, FsEnt)> = vec![
        ("/tmp/build_patches_src".into(), FsEnt::Dir(src_dir)),
        ("/tmp/build_patches".into(), FsEnt::Dir(pp.iter().map(|(n, t)| (n.to_string(), t.clone().into_bytes())).collect())),
        ("/tmp/requirements.txt".into(), FsEnt::File(req.to_vec())),
        ("/workspace/vllm-src/vllm/a.py".into(), FsEnt::File(b"anchor  # [port:60-fx]\n".to_vec())),
        ("/workspace/vllm-src/gate.py".into(), FsEnt::File(b"if (9, 0) <= cap < (13, 0):\n".to_vec())),
        ("/usr/lib/pyfx/dist-packages/fxmod/dev.py".into(), FsEnt::File(b"x = 1\n# [GB10] guard-a\n".to_vec())),
    ];
    let args3 = "RUN |3 SM12X_PORT=0 FX_OPEN=1 FX_SHUT=0 /bin/sh -c ";
    let lay_p: Vec<(String, String)> = vec![
        (format!("{args3}for p in $(ls /tmp/build_patches/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done # buildkit"), "2026-01-02T03:04:00+09:00".into()),
        ("COPY build_patches/ /tmp/build_patches/ # buildkit".into(), "2026-01-02T03:03:00+09:00".into()),
        (format!("{args3}for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done # buildkit"), "2026-01-02T03:02:00+09:00".into()),
        ("COPY build_patches_src/ /tmp/build_patches_src/ # buildkit".into(), "2026-01-02T03:01:00+09:00".into()),
        ("COPY requirements.txt /tmp/requirements.txt # buildkit".into(), "2026-01-02T03:00:00+09:00".into()),
        ("ARG FX_SHUT=0".into(), "2026-01-02T03:00:00+09:00".into()),
        ("ARG FX_OPEN=0".into(), "2026-01-02T03:00:00+09:00".into()),
        ("RUN |2 NVIDIA_X=1 SM12X_PORT=1 /bin/sh -c base # buildkit".into(), "2025-12-01T00:00:00+09:00".into()),
    ];
    let img_env = vec!["PATH=/usr/bin".to_string(), "LD_LIBRARY_PATH=/usr/lib/pyfx/dist-packages/torch/lib:/usr/local/nvidia/lib".to_string()];
    let dg2 = format!("sha256:{}", "e".repeat(64));
    let ev_p = p(&format!(
        r#"{{"topology": "single", "cell": "{pc}", "node": "main", "lockset": {{}}, "attestation": null,
            "build_identity": {{"image_digest": "{dg2}", "image_tag": "easy-vllm:fx-probe"}}}}"#
    ));
    let created = "2026-01-02T03:04:05+09:00";
    let calls_p: Calls = Rc::default();
    let removed_p: Rc<RefCell<BTreeSet<String>>> = Rc::default();
    let dk_p = _fake_image_docker(
        &lay_p,
        created,
        ImgOpts { fs: img_fs.clone(), env: img_env.clone(), calls: Some(calls_p.clone()), removed: Some(removed_p.clone()), ..Default::default() },
    );
    let rp_ = coll(&repo, &ev_p, &repo.join("p_probe"), Some(&dk_p))?;
    let bp = by(&rp_["applied_set"]["patches"], "file");
    let want: Vec<(&str, (&str, &str))> = vec![
        ("50-gate.sh", ("skipped", "reconstructed(docker-history+gate)")),
        ("55-open.sh", ("applied", "reconstructed(fail-loud+built)")),
        ("56-two.sh", ("unobserved", "none")),
        ("60-loud.sh", ("applied", "reconstructed(fail-loud+built)")),
        ("61-drift.sh", ("applied", "reconstructed(fail-loud+built)")),
        ("62-bakedonly.sh", ("applied", "reconstructed(fail-loud+built)")),
        ("10-loud.sh", ("applied", "reconstructed(fail-loud+built)")),
        ("15-noe.sh", ("unobserved", "none")),
        ("16-var.sh", ("unobserved", "none")),
        ("20-cond.sh", ("unobserved", "none")),
        ("30-cond.sh", ("applied", "image-probe(script-sha+marker)")),
        ("40-mod.sh", ("applied", "image-probe(script-sha+marker)")),
    ];
    let got = |f: &str| -> Option<(String, String)> {
        bp.get(f).map(|r| (r["result"].display(), r["result_source"].display()))
    };
    let mut diff: Vec<(String, Option<(String, String)>)> = want
        .iter()
        .filter(|(f, (a, b))| got(f) != Some((a.to_string(), b.to_string())))
        .map(|(f, _)| (f.to_string(), got(f)))
        .collect();
    diff.sort();
    let diff_s = format!(
        "[{}]",
        diff.iter()
            .map(|(f, g)| match g {
                Some((a, b)) => format!("({}, ({}, {}))", crate::value::py_repr(f), crate::value::py_repr(a), crate::value::py_repr(b)),
                None => format!("({}, None)", crate::value::py_repr(f)),
            })
            .collect::<Vec<_>>()
            .join(", ")
    );
    let want_keys: BTreeSet<String> = want.iter().map(|(f, _)| f.to_string()).collect();
    st.ck(&format!("F12 패치별 판정·출처(태그2 모양) {diff_s}"), diff.is_empty() && keys(&bp) == want_keys);
    st.ck("★F12 내용 조건부 + 마커 없음(20) = 이미지 탐침으로도 판정 ✗", has(&bp["20-cond.sh"]["reason"], "마커 없음"));
    st.ck(
        "★F12 set -e 없음(15) · exit $변수(16) · 일부만 열린 게이트(56) = 미관측",
        has(&bp["15-noe.sh"]["reason"], "set -e") && has(&bp["16-var.sh"]["reason"], "exit $rc") && has(&bp["56-two.sh"]["reason"], "전부 열리지는"),
    );
    st.ck("★F12 `sys.exit(0 if …)` · `rm … || true` 는 skip 경로가 아니다(10)", eqs(&bp["10-loud.sh"]["result"], "applied"));
    let sp60 = &sp.iter().find(|(n, _)| *n == "60-loud.sh").unwrap().1;
    st.ck(
        "★F12 조건문 안 grep(트립와이어)은 적용 마커가 아니다",
        _script_shape(sp60)?["markers"].items().len() == 1 && bp["60-loud.sh"]["evidence"]["markers"]["found"].as_i64() == Some(1),
    );
    st.ck(
        "★F12 판정은 이미지에 구워진 바이트로(61: 작업트리엔 exit 0 · 이미지 바이트는 fail-loud) · 이미지 바이트를 싣는다",
        sw(&bp["61-drift.sh"]["script_identity"], "image-probe(작업트리와 다름")
            && rb(&repo.join("p_probe/artifacts/build_patch_pre/61-drift.sh")).ok() == Some(baked61.clone())
            && rb(&repo.join("p_probe/artifacts/build_patch_pre/62-bakedonly.sh")).ok() == Some(baked62.clone()),
    );
    st.ck(
        "★F12 이미지에 없는 작업트리 패치(63) = not-in-image · 싣지 않음",
        bp.get("63-notbaked.sh").is_none()
            && rp_["applied_set"]["excluded"].items().iter().any(|x| eqs(&x["file"], "63-notbaked.sh") && sw(&x["why"], "not-in-image"))
            && !has(&rp_["files"], "artifacts/build_patch_pre/63-notbaked.sh"),
    );
    let prec = find_in(&rp_["applied_set"]["probes"], "tier", "image-probe");
    let cp_ = calls_p.borrow().clone();
    st.ck(
        "F12 탐침 = create(--pull never · --network none · start ✗) → cp → rm(항상)",
        eqs(&prec["result"], "done")
            && is_true(&prec["container_removed"])
            && removed_p.borrow().contains(&"c".repeat(64))
            && cp_.iter().any(|a| a1(a) == Some("create") && a.iter().any(|x| x == "--pull") && a.iter().any(|x| x == "never") && a.iter().any(|x| x == "--network"))
            && !cp_.iter().any(|a| matches!(a1(a), Some("start") | Some("exec") | Some("run")))
            && cp_.iter().all(|a| a1(a) != Some("create") || a.iter().any(|x| x == "/bin/true"))
            && cp_.iter().any(|a| a1(a) == Some("rm"))
            && cp_.iter().all(|a| a1(a) != Some("rm") || a.get(1).map(String::as_str) == Some("-v")),
    );
    st.ck(
        "★F12 원장 탐침(②)도 시작하지 않는 컨테이너(cp) — 원장 없음 = no-ledger(docker cp)",
        sw(&rp_["applied_set"]["probes"][1]["result"], "no-ledger(이미지에") && sw(&rp_["applied_set"]["probes"][1]["method"], "docker create+cp"),
    );
    st.ck("F12 모듈 대상 해소 = 이미지 ENV 의 dist-packages 루트", strs(&prec["python_roots"]) == ["/usr/lib/pyfx/dist-packages"]);
    let vocab = ["reconstructed(docker-history+gate)", "reconstructed(fail-loud+built)", "image-probe(script-sha+marker)", "build-ledger", "none"];
    st.ck(
        "F12 결과 출처 어휘(공유 계약)",
        rp_["applied_set"]["patches"].items().iter().all(|r| vocab.contains(&sv(&r["result_source"]).as_str())),
    );
    st.ck("F12 applied_set 은 직렬화 가능(이미지 바이트는 PAYLOAD 로 새지 않는다)", !core::dumps(&rp_["applied_set"]).contains("port:60-fx"));
    let sp_ = by(&rp_["slots"]["build_recipe"]["evidence"]["selected_revisions"], "path");
    st.ck(
        "F1 COPY 대상 파일(requirements) = 이미지 안 바이트와 대조 · Dockerfile = history 전수 일치",
        is_true(&sp_["output/single/requirements.txt"]["verified"]) && is_false(&sp_["output/single/Dockerfile.probe"]["verified"]),
    );
    st.ck("F3 single: 무장 패치 없음 = arm_patch.sh 미실림", !has(&rp_["files"], "artifacts/compose/arm_patch.sh"));
    let bps = sv(&steps_by(rp_["reproduce_steps"].items())["build"]["command"]);
    st.ck(
        "F5 single 빌드 명령(history 인자 · compose 컨텍스트)",
        bps.contains("docker build -f output/single/Dockerfile.probe")
            && bps.contains("--build-arg FX_OPEN=1")
            && bps.contains("${REPRO_IMAGE_TAG:?set a separate reproduction image tag}")
            && bps.contains("docker compose -f output/single/docker-compose.yaml"),
    );
    let cmp_ = so.join("docker-compose.yaml");
    let cmp_b = rt(&cmp_)?;
    let r = (|| -> HR<(J, J)> {
        wt(&cmp_, &cmp_b.replace("${BUILD_DOCKERFILE:-Dockerfile.probe}", "Dockerfile.source-build   # lit"))?;
        let rl2 = coll(&repo, &ev_p, &repo.join("p_probe_lit"), Some(&dk_p))?;
        wt(&cmp_, &cmp_b.replace("${BUILD_DOCKERFILE:-Dockerfile.probe}", "Dockerfile.probe"))?;
        let rl3 = coll(&repo, &ev_p, &repo.join("p_probe_lit3"), Some(&dk_p))?;
        Ok((rl2, rl3))
    })();
    wt(&cmp_, &cmp_b)?;
    let (rl2, rl3) = r?;
    let bl2 = sv(&steps_by(rl2["reproduce_steps"].items())["build"]["command"]);
    let bl3 = sv(&steps_by(rl3["reproduce_steps"].items())["build"]["command"]);
    let mp2 = by(&rl2["build_context_map"], "slot_path")["artifacts/build_recipe/Dockerfile.probe"].clone();
    let mp3 = by(&rl3["build_context_map"], "slot_path")["artifacts/build_recipe/Dockerfile.probe"].clone();
    let b2 = if mp2.get("basis").is_some() { mp2["basis"].display() } else { "None".to_string() };
    let b3 = if mp3.get("basis").is_some() { mp3["basis"].display() } else { "None".to_string() };
    st.ck(
        "★F2 compose 리터럴(Dockerfile.source-build) ≠ 이 이미지 Dockerfile → '동등 경로(compose)' ✗ · '동등 경로가 아니다' · 매핑 근거 = 리터럴",
        !bl2.contains("동등 경로(compose)")
            && bl2.contains("파일만 단독 사용하고 override가 없으면")
            && bl2.contains("Dockerfile.source-build이다")
            && bl2.contains("실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인")
            && !bl2.contains("위 docker build 명령만 쓴다")
            && !b2.contains("${BUILD_DOCKERFILE}")
            && b2.contains("파일만 단독 사용하고 override가 없으면")
            && b2.contains("실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인"),
    );
    st.ck(
        "F2 음성대조: 리터럴 = 이 이미지 Dockerfile 이면 동등 경로 유지 · 근거 '(리터럴)'",
        bl3.contains("compose 빌드 사용법(실제 실행·재현 등치는 미검증)") && b3.contains("compose build.dockerfile Dockerfile.probe(리터럴)"),
    );
    st.ck(
        "F12 슬롯 증거: 전부 applied 인 슬롯만 reconstruction",
        eqs(&rp_["slots"]["build_patch_pre"]["evidence"]["kind"], "none") && has(&rp_["slots"]["build_patch_pre"]["evidence"]["note"], "적용 미관측"),
    );
    // ★읽기 전용 실행기(image_probe 미선언)
    let calls_ro: Calls = Rc::default();
    let dk_ro = _fake_image_docker(&lay_p, created, ImgOpts { fs: img_fs.clone(), env: img_env.clone(), probe: false, calls: Some(calls_ro.clone()), ..Default::default() });
    let rro = coll(&repo, &ev_p, &repo.join("p_probe_ro"), Some(&dk_ro))?;
    let bro = by(&rro["applied_set"]["patches"], "file");
    st.ck(
        "★F12 읽기 전용 = create/cp/rm 호출 0 · 30·40 미관측 · 61 은 작업트리(exit 0) = 미관측",
        !calls_ro.borrow().iter().any(|a| matches!(a1(a), Some("create") | Some("cp") | Some("rm")))
            && eqs(&bro["30-cond.sh"]["result"], "unobserved")
            && eqs(&bro["40-mod.sh"]["result"], "unobserved")
            && eqs(&bro["61-drift.sh"]["result"], "unobserved")
            && sw(&bro["63-notbaked.sh"]["script_identity"], "unobserved("),
    );
    let bro_vals: Vec<J> = bro.as_dict().map(|m| m.values().cloned().collect()).unwrap_or_default();
    st.ck(
        "★F12 탐침 없음 = (ii) applied 0(63-notbaked 는 이미지에 없다) · 사유가 동일성 미확인 · skip 재구성은 유지",
        !bro_vals.iter().any(|r| eqs(&r["result"], "applied"))
            && eqs(&bro["63-notbaked.sh"]["result"], "unobserved")
            && eqs(&bro["60-loud.sh"]["result"], "unobserved")
            && has(&bro["63-notbaked.sh"]["reason"], "--docker-read-only")
            && has(&bro["10-loud.sh"]["reason"], "--docker-read-only")
            && eqs(&bro["50-gate.sh"]["result"], "skipped")
            && eqs(&rro["slots"]["build_patch_pre"]["evidence"]["kind"], "none"),
    );
    st.ck(
        "★F12 미관측 사유에 no-op 옛 플래그(--docker-probe)를 처방으로 적지 않는다",
        !bro_vals.iter().any(|r| (if r["reason"].truthy() { r["reason"].display() } else { String::new() }).contains("--docker-probe")),
    );
    // ── 탐침 기본값 ──
    let c_def = ctx(&repo, &ev_p, None)?;
    let (ok_def, why_def) = _probe_allowed(&c_def)?;
    st.ck(
        "runner=None = 탐침 ON(기본 실행기가 image_probe 선언) · 탐침 기록 문구 = 기본 실행기",
        ok_def && c_def.default_runner.get() && c_def.runner_probes() && why_def.contains("기본 실행기"),
    );
    let undeclared: Runner = Rc::new(|_a: &[String], _t: u64| -> Result<Proc, RunFail> { Ok(proc_(1, "", "")) });
    let declared_false: Runner = Rc::new(Declared { f: Box::new(|_a: &[String], _t: u64| Ok(proc_(1, "", ""))), probe: false });
    st.ck(
        "★선언 없는 주입 실행기 · image_probe=False = 탐침 OFF(읽기 전용으로 본다 · 추측해서 켜지 않는다)",
        !_probe_allowed(&ctx(&repo, &ev_p, Some(&undeclared))?)?.0
            && !_probe_allowed(&ctx(&repo, &ev_p, Some(&declared_false))?)?.0
            && _probe_allowed(&ctx(&repo, &ev_p, Some(&dk_p))?)?.1 == "주입 실행기(image_probe 선언)",
    );
    let seen_d: Calls = Rc::default();
    let sd_ = seen_d.clone();
    let fake_sub: RealRun = Rc::new(move |argv: &[String], _t: u64| -> Result<Proc, RunFail> {
        sd_.borrow_mut().push(argv.to_vec());
        if argv.get(1).map(String::as_str) == Some("create") {
            return Ok(proc_(0, &format!("{}\n", "f".repeat(64)), ""));
        }
        Ok(proc_(0, "", ""))
    });
    let dr = _probe_only_runner(Some(fake_sub))?;
    let call = |v: &[&str]| -> i32 {
        let argv: Vec<String> = v.iter().map(|s| s.to_string()).collect();
        dr.run(&argv, DOCKER_TIMEOUT_S as u64).map(|p_| p_.returncode).unwrap_or(-999)
    };
    let refused: BTreeSet<i32> = ["run", "start", "exec", "build", "pull", "compose", "kill"].iter().map(|v| call(&["docker", v, "x"])).collect();
    st.ck(
        "★기본 실행기: run·start·exec·build·pull·compose·kill = rc 125 · 실물 호출 0",
        refused == [125].into_iter().collect::<BTreeSet<i32>>() && seen_d.borrow().is_empty(),
    );
    st.ck(
        "★기본 실행기: `--pull never --network none` 없는 create = 거부(실물 호출 0)",
        call(&["docker", "create", "img"]) == 125
            && call(&["docker", "create", "--pull", "never", "img"]) == 125
            && call(&["docker", "create", "--pull", "always", "--network", "none", "img"]) == 125
            && seen_d.borrow().is_empty(),
    );
    let a64 = format!("{}:/x", "a".repeat(64));
    st.ck(
        "★기본 실행기: 남이 만든 컨테이너 cp·rm = 거부(실물 호출 0)",
        call(&["docker", "cp", "-L", &a64, "y"]) == 125 && call(&["docker", "rm", &"a".repeat(64)]) == 125 && seen_d.borrow().is_empty(),
    );
    let r_cr = call(&["docker", "create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", "img"]);
    let f64_ = "f".repeat(64);
    let f64x = format!("{f64_}:/x");
    let c1 = call(&["docker", "cp", "-L", &f64x, "y"]);
    let c2 = call(&["docker", "rm", &f64_]);
    let c3 = call(&["docker", "rm", &f64_]);
    let c4 = call(&["docker", "image", "inspect", "img"]);
    let c5 = call(&["docker", "history", "img"]);
    st.ck(
        "기본 실행기: create(--pull never --network none) → 자기 컨테이너 cp → rm · ★지운 뒤 rm 재시도 = 거부 · inspect·history 통과",
        r_cr == 0
            && c1 == 0
            && c2 == 0
            && c3 == 125
            && c4 == 0
            && c5 == 0
            && seen_d.borrow().iter().map(|a| a[1].clone()).collect::<Vec<_>>() == ["create", "cp", "rm", "image", "history"]
            && dr.image_probe(),
    );
    // ★(ii) + 선언 마커 부재
    let fs_nomark60 = fs_with(&img_fs, "/workspace/vllm-src/vllm/a.py", FsEnt::File(b"anchor\n".to_vec()));
    let rn60 = coll(&repo, &ev_p, &repo.join("p_probe_nm60"), Some(&_fake_image_docker(&lay_p, created, ImgOpts { fs: fs_nomark60, env: img_env.clone(), ..Default::default() })))?;
    let b60 = by(&rn60["applied_set"]["patches"], "file");
    st.ck(
        "★F12 (ii) + 선언 마커 부재 = 미관측(모순 기재) · 마커를 선언하지 않은 (ii)(10)는 applied 유지",
        eqs(&b60["60-loud.sh"]["result"], "unobserved") && has(&b60["60-loud.sh"]["reason"], "선언 마커 부재") && eqs(&b60["10-loud.sh"]["result"], "applied"),
    );
    let env_ok = p(r#"{"loop": [true, "fx 루프"], "image_exists": true, "identity_ok": true,
                      "probe": {"ran": true, "files": {}, "modules": {}, "scripts": {}}}"#);
    let r_sw = _reconstruct("70-sw.sh", "#!/bin/bash\nset -e\npython3 - \"$F\" <<'PY' || true\nopen('/x', 'w').write('y')\nPY\necho done\n", "pre", &J::dict(), &env_ok)?;
    let r_tr = _reconstruct("71-tr.sh", "#!/bin/bash\nset -e\ntrap 'exit 0' ERR\npython3 /x.py\n", "pre", &J::dict(), &env_ok)?;
    let r_ok = _reconstruct(
        "72-ok.sh",
        "#!/bin/bash\nset -e\nrm -rf -- /tmp/b 2>/dev/null || true\nV=\"$(grep -E x /f || true)\"\nfind /w -name '*.pyc' -delete 2>/dev/null || true\npython3 /x.py\n",
        "pre",
        &J::dict(),
        &env_ok,
    )?;
    st.ck(
        "★(ii) 패치 단계 `|| true`·trap = applied ✗ · 정리 명령·치환 안 `|| true` 는 skip 경로가 아니다(applied)",
        eqs(&r_sw["result"], "unobserved")
            && has(&r_sw["reason"], "실패 삼킴")
            && eqs(&r_tr["result"], "unobserved")
            && has(&r_tr["reason"], "trap")
            && eqs(&r_ok["result"], "applied")
            && eqs(&r_ok["result_source"], "reconstructed(fail-loud+built)"),
    );
    st.ck(
        "★따옴표 안 성공 종료(`eval 'exit 0'`)도 성공 종료 가능 exit 로 센다 · `exit 1` 은 아니다",
        _script_shape("#!/bin/bash\nset -e\neval 'exit 0'\n[ -f /x ] || exit 1\n")?["exits"].items().iter().map(|e| e["code"].display()).collect::<Vec<_>>() == ["0"],
    );
    let fs_nomark = fs_with(&img_fs, "/workspace/vllm-src/gate.py", FsEnt::File(b"if (9, 0) <= cap < (11, 0):\n".to_vec()));
    let rnm = coll(&repo, &ev_p, &repo.join("p_probe_nm"), Some(&_fake_image_docker(&lay_p, created, ImgOpts { fs: fs_nomark, env: img_env.clone(), ..Default::default() })))?;
    let bnm = by(&rnm["applied_set"]["patches"], "file");
    st.ck(
        "★F12 선언 마커 부재 = 미관측(모순 기재)",
        eqs(&bnm["30-cond.sh"]["result"], "unobserved") && has(&bnm["30-cond.sh"]["reason"], "선언 마커 부재"),
    );
    let led_p = p(r#"{"schema_version": 1, "dockerfile": "Dockerfile.probe", "track": "source-build",
                     "build_args": {"SM12X_PORT": "0", "FX_OPEN": "1", "FX_SHUT": "0"},
                     "patches": [{"phase": "pre", "file": "60-loud.sh", "result": "applied", "result_source": "exit-code"}]}"#);
    let calls_l: Calls = Rc::default();
    let removed_l: Rc<RefCell<BTreeSet<String>>> = Rc::default();
    let rl = coll(
        &repo,
        &ev_p,
        &repo.join("p_probe_led"),
        Some(&_fake_image_docker(
            &lay_p,
            created,
            ImgOpts {
                fs: fs_with(&img_fs, LEDGER_IN_IMAGE, FsEnt::File(dumps(&led_p).into_bytes())),
                env: img_env.clone(),
                calls: Some(calls_l.clone()),
                removed: Some(removed_l.clone()),
                ..Default::default()
            },
        )),
    )?;
    let rl_steps = steps_by(rl["reproduce_steps"].items());
    st.ck(
        "F12 이미지 원장 = docker create+cp 로 관측(run 0 · rm 됨) · 원장 행 출처 build-ledger",
        eqs(&rl["applied_set"]["status"], "observed")
            && !calls_l.borrow().iter().any(|a| a1(a) == Some("run"))
            && removed_l.borrow().contains(&"c".repeat(64))
            && eqs(&rl["applied_set"]["patches"][0]["result_source"], "build-ledger")
            && eqs(&rl["applied_set"]["patches"][0]["ledger_result_source"], "exit-code")
            && has(&rl_steps["build"]["command"], "--build-arg FX_OPEN=1")
            && eqs(&rl_steps["build"]["command_source"], "reconstructed(build-ledger build_args + compose build 선언/설정(실행 관측 아님))"),
    );
    let base_rm = _fake_image_docker(&lay_p, created, ImgOpts { fs: img_fs.clone(), env: img_env.clone(), ..Default::default() });
    let rm_fails: Runner = Rc::new(Declared {
        f: Box::new(move |argv: &[String], t: u64| {
            if argv.get(1).map(String::as_str) == Some("rm") {
                return Ok(proc_(1, "", "Error: cannot remove"));
            }
            base_rm.run(argv, t)
        }),
        probe: true,
    });
    let arm = aset(&repo, &ev_p, &rm_fails)?;
    st.ck(
        "★F12 원장 cp 뒤 docker rm 실패 = no-ledger 줄에 rm-failed 기재 · 이미지 탐침 기록도 제거 실패",
        sw(&arm["probes"][1]["result"], "no-ledger(")
            && has(&arm["probes"][1]["result"], "rm-failed(")
            && is_false(&find_in(&arm["probes"], "tier", "image-probe")["container_removed"]),
    );
    // ★cp 가 실패해도 컨테이너는 지운다(finally) — translation: a Rust runner cannot raise; cp fails with RunFail::Timeout.
    let removed_c: Rc<RefCell<BTreeSet<String>>> = Rc::default();
    let rcrash = coll(
        &repo,
        &ev_p,
        &repo.join("p_probe_crash"),
        Some(&_fake_image_docker(
            &lay_p,
            created,
            ImgOpts { fs: img_fs.clone(), env: img_env.clone(), removed: Some(removed_c.clone()), crash_on_cp: true, ..Default::default() },
        )),
    );
    st.ck(
        "★F12 cp 예외에도 docker rm(finally) · artifacts/ 잔재 없음",
        removed_c.borrow().contains(&"c".repeat(64)) && (rcrash.is_ok() || !exists(&repo.join("p_probe_crash/artifacts"))),
    );
    let rrq = coll(
        &repo,
        &ev_p,
        &repo.join("p_probe_req"),
        Some(&_fake_image_docker(
            &lay_p,
            created,
            ImgOpts { fs: fs_with(&img_fs, "/tmp/requirements.txt", FsEnt::File(b"other\n".to_vec())), env: img_env.clone(), ..Default::default() },
        )),
    )?;
    let rvq = by(&rrq["slots"]["build_recipe"]["evidence"]["recipe_vs_image"], "path");
    let sq = by(&rrq["slots"]["build_recipe"]["evidence"]["selected_revisions"], "path");
    st.ck(
        "★F1 COPY 대상 바이트 불일치 = verified ✗ (추적 파일이면 경고 행)",
        is_false(&sq["output/single/requirements.txt"]["verified"])
            && has(&sq["output/single/requirements.txt"]["method"], "이미지 안 바이트와 다르고")
            && (rvq.get("output/single/requirements.txt").is_none() || rvq["output/single/requirements.txt"].get("warning").is_some()),
    );
    Ok(())
}

// ── 2026-10-25 Solar 재생 사실검증 F2·F3 — 실물 armer · 실물 로그 모양 ──
fn sec_real_armer(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let assets = fx.assets.clone();
    let real_arm = fx.own.join(".claude/skills/upstream-version-watch/assets/configs").join(RUNTIME_PATCH_ARMER);
    let envc = out.join("envs").join(format!(".env.{cell}"));
    let (arm0, envc0) = (rb(&assets.join(RUNTIME_PATCH_ARMER))?, rt(&envc)?);
    let armed_line = format!("[arm_patch] runtime patch armed → /app/configs/{cell}_patch.py (purelib /usr/local/lib/python3.12/dist-packages)\n");
    let sweep_d = repo.join("output/multi/benchlog/sweep_c1-mmp");
    let fail_d = repo.join("output/multi/benchlog").join(format!("serve_fail_{cell}"));
    let r = (|| -> HR<(J, J)> {
        let ra = rb(&real_arm)?;
        for dst in [assets.join(RUNTIME_PATCH_ARMER), out.join("configs").join(RUNTIME_PATCH_ARMER)] {
            wb(&dst, &ra)?;
        }
        wt(&envc, &format!("{envc0}RUNTIME_PATCH_STEM={cell}\n"))?;
        _set_mtime(&envc, "2026-01-01T00:00:00Z")?;
        let r_nolog = coll(&repo, &ev, &repo.join("p_arm_nolog"), Some(&docker_old))?;
        wt(&sweep_d.join(format!("lite_engine_head_{cell}.log")), &format!("INFO 01-01 19:00:00 x\n\u{1b}[0m{armed_line}y\n{armed_line}"))?;
        mkd(&fail_d)?;
        wt(&fail_d.join("slave_failure.log"), &format!("z\n{armed_line}"))?;
        let r_log = coll(&repo, &ev, &repo.join("p_arm_log"), Some(&docker_old))?;
        Ok((r_nolog, r_log))
    })();
    for dst in [assets.join(RUNTIME_PATCH_ARMER), out.join("configs").join(RUNTIME_PATCH_ARMER)] {
        wb(&dst, &arm0)?;
    }
    wt(&envc, &envc0)?;
    _set_mtime(&envc, "2026-01-01T00:00:00Z")?;
    rm_ok(&sweep_d.join(format!("lite_engine_head_{cell}.log")));
    let _ = std::fs::remove_dir_all(&fail_d);
    let (r_nolog, r_log) = r?;
    let sr_s = crate::value::loads(&rt(&repo.join("p_arm_log/artifacts/compose/sub_recipe.json"))?).map_err(|e| HintError::py("JSONDecodeError", e))?;
    let rd_s = sr_s["role_diff"].clone();
    st.ck(
        "★F3 실물 armer 키 체인(RUNTIME_PATCH_STEM > CONFIG_FILE > 'default') × slave 전달 RUNTIME_PATCH_STEM = slave armed",
        sw(&rd_s["slave_runtime_patch"], "armed")
            && has(&rd_s["slave_runtime_patch"], "RUNTIME_PATCH_STEM")
            && !has(&rd_s["slave_runtime_patch"], "K11")
            && strs(&rd_s["runtime_patch_key_chain"]).iter().take(2).cloned().collect::<Vec<_>>() == ["RUNTIME_PATCH_STEM", "CONFIG_FILE"]
            && sr_s["env_forward"].items().iter().any(|r| eqs(&r["key"], "RUNTIME_PATCH_STEM")),
    );
    let rec_nolog = r_nolog["slots"]["runtime_patch"]["file_records"].at(0).clone();
    let rec_log = r_log["slots"]["runtime_patch"]["file_records"].at(0).clone();
    st.ck(
        "F2 음성대조: 실물 armer · 로그에 무장 줄 없음 = 서명 미관측(적용 관측 ✗) · 1-signal",
        has(&rec_nolog["verification_basis"], "arming 서명 미관측") && eqs(&r_nolog["slots"]["runtime_patch"]["confidence"], "1-signal"),
    );
    st.ck(
        "★F2 측정 머리 로그(ANSI 섞임)·실패 기동 slave 로그의 무장 줄 = 관측(파일:줄 · 측정/실패 기동 구분) · 2-signal",
        has(&rec_log["verification_basis"], &format!("sweep_c1-mmp/lite_engine_head_{cell}.log:2,4"))
            && has(&rec_log["verification_basis"], &format!("slave output/multi/benchlog/serve_fail_{cell}/slave_failure.log:2"))
            && has(&rec_log["verification_basis"], "실패 기동 로그(측정 기동 아님)")
            && !has(&rec_log["verification_basis"], "미배선")
            && eqs(&r_log["slots"]["runtime_patch"]["confidence"], "2-signal"),
    );
    Ok(())
}

// ── F5 벤치 명령 음성대조 ──
fn sec_bench_cmds(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, ev) = (fx.repo.clone(), fx.ev.clone());
    mkd(&repo.join("output/multi/benchlog/sweep_nonuni/level_01"))?;
    wt(
        &repo.join("output/multi/benchlog/sweep_nonuni/level_01/bench.json"),
        &dumps(&p(r#"{"date": "20260101-190500", "backend": "openai", "num_prompts": 4, "max_concurrency": 2, "completed": 4,
                     "total_input_tokens": 4097, "total_output_tokens": 1024}"#)),
    )?;
    mkd(&repo.join("output/multi/benchlog/sweep_empty"))?;
    let cnu = ctx(&repo, &with(&ev, vec![("sweep", p(r#"{"dir": "output/multi/benchlog/sweep_nonuni"}"#))]), None)?;
    let (cmd_nu, _src_nu) = _bench_commands(&cnu)?;
    st.ck(
        "★F5 요청 길이 불균일 = 데이터셋 인자를 지어내지 않는다(명령 줄 · 머리 주석은 규칙 설명)",
        !cmd_nu.lines().any(|ln| !ln.starts_with('#') && ln.contains("--dataset-name"))
            && cmd_nu.lines().any(|ln| ln.starts_with("vllm bench serve"))
            && cmd_nu.contains("--max-concurrency 2")
            && cmd_nu.contains("불균일"),
    );
    let (cmd_e, src_e) = _bench_commands(&ctx(&repo, &with(&ev, vec![("sweep", p(r#"{"dir": "output/multi/benchlog/sweep_empty"}"#))]), None)?)?;
    st.ck("★F5 bench JSON 없음 = 명령 비움 · 출처 unobserved(자리표시 ✗)", cmd_e.is_empty() && src_e.starts_with("unobserved("));
    let (cmd_b, src_b) = _bench_commands(&ctx(&repo, &with(&ev, vec![("sweep", with(&ev["sweep"], vec![("binding", "simlog-copy".into())]))]), None)?)?;
    let (cmd_o, src_o) = _bench_commands(&ctx(&repo, &with(&ev, vec![("sweep", with(&ev["sweep"], vec![("binding", "output".into())]))]), None)?)?;
    st.ck(
        "★F5 스윕 binding≠output = 재구성 ✗(unobserved) · output = 재구성",
        cmd_b.is_empty() && src_b.contains("binding='simlog-copy'") && cmd_o.contains("vllm bench serve") && src_o == "reconstructed(bench json)",
    );
    Ok(())
}

fn layers(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}
/// `text.split("\n", 1)[1]`.
fn after_first_line(s: &str) -> &str {
    s.split_once('\n').map(|x| x.1).unwrap_or("")
}
/// `data.replace(data.splitlines(keepends=True)[1], b"", 1)`.
fn drop_second_line(data: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(data).to_string();
    let parts: Vec<&str> = s.split_inclusive('\n').collect();
    match parts.get(1) {
        Some(l) => s.replacen(l, "", 1).into_bytes(),
        None => s.into_bytes(),
    }
}
fn re_search(pat: &str, text: &str) -> bool {
    Re::new(pat, 0).search(text).is_some()
}

// ── 레시피 ↔ 이미지 · 개정 선택(임시 git) ──
fn sec_git(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let assets = fx.assets.clone();
    let decl = fx.decl.clone();
    let genv = genv_at("2026-01-01T00:00:00Z")?;
    git(&repo, &["init", "-q"], &genv)?;
    git(&repo, &["add", "output/multi/Dockerfile.source-build", "output/multi/requirements.txt"], &genv)?;
    git_commit(&repo, "fx", &genv)?;
    let dsb = rt(&out.join("Dockerfile.source-build"))?;
    wt(&out.join("Dockerfile.source-build"), &format!("{dsb}# 빌드 뒤 수정\n"))?;
    let rg = coll(&repo, &ev, &repo.join("p_git"), Some(&docker_old))?;
    let rv = by(&rg["slots"]["build_recipe"]["evidence"]["recipe_vs_image"], "path");
    st.ck(
        "★빌드 뒤 수정된 추적 Dockerfile = 경고",
        is_false(&rv["output/multi/Dockerfile.source-build"]["worktree_equals_head"]) && rv["output/multi/Dockerfile.source-build"].get("warning").is_some(),
    );
    st.ck(
        "커밋이 이미지보다 이르고 수정 없음 = 경고 없음",
        rv["output/multi/requirements.txt"].get("warning").is_none() && eqs(&rv["output/multi/requirements.txt"]["last_commit_utc"], "2026-01-01T00:00:00Z"),
    );
    let df_rev = concat!(
        "FROM base AS builder\nRUN echo builder-only\nFROM base\nARG SM12X_PORT=0\nARG BUILD_JOBS=16\n",
        "COPY requirements.txt /tmp/requirements.txt\n",
        "RUN --mount=type=cache,target=/root/.ccache pip install \\\n    # 이어짐 사이 주석\n    fx-pkg && \\\n",
        "    echo done\nRUN python3 - <<'PY'\nimport sys\nprint(\"hoist\")\nPY\n",
        "COPY build_patches_src/ /tmp/build_patches_src/\n",
        "RUN for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done\n",
    )
    .to_string();
    wt(&out.join("Dockerfile.rev"), &df_rev)?;
    git(&repo, &["add", "output/multi/Dockerfile.rev"], &genv)?;
    git_commit(&repo, "rev", &genv)?;
    let rev1 = core::git_out(&repo, &["rev-parse", "HEAD"])?;
    let df_wt = format!(
        "{}COPY build_patches/ /tmp/build_patches/\nRUN python3 - <<'PY'\nprint(\"ledger\")\nPY\n",
        df_rev.replace("do bash \"$p\" || exit 1; done", "do bash \"$p\"; echo \"$?\" > /x; done")
    );
    wt(&out.join("Dockerfile.rev"), &df_wt)?;
    let genv3 = genv_at("2026-01-03T00:00:00Z")?;
    git(&repo, &["add", "output/multi/Dockerfile.rev"], &genv3)?;
    git_commit(&repo, "late", &genv3)?;
    wt(&out.join("envs/.env.rev"), "IMAGE_TAG=easy-vllm:fx-rev\nBUILD_DOCKERFILE=Dockerfile.rev\n")?;
    let ev_rev = with(
        &ev,
        vec![(
            "triplet",
            jd(&[
                ("env", "output/multi/envs/.env.rev".into()),
                ("yaml", format!("output/multi/configs/{cell}.yaml").into()),
                ("sh", format!("output/multi/configs/{cell}.sh").into()),
            ]),
        )],
    );
    let layers_rev = layers(&[
        (
            "RUN |2 SM12X_PORT=0 BUILD_JOBS=8 /bin/sh -c for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done # buildkit",
            "2026-01-02T03:04:00+09:00",
        ),
        ("COPY build_patches_src/ /tmp/build_patches_src/ # buildkit", "2026-01-02T03:03:00+09:00"),
        ("RUN |2 SM12X_PORT=0 BUILD_JOBS=8 /bin/sh -c python3 - <<'PY'\nimport sys\nprint(\"hoist\")\nPY # buildkit", "2026-01-02T03:02:00+09:00"),
        ("RUN |2 SM12X_PORT=0 BUILD_JOBS=8 /bin/sh -c pip install     fx-pkg &&     echo done # buildkit", "2026-01-01T09:00:00+09:00"),
        ("COPY requirements.txt /tmp/requirements.txt # buildkit", "2026-01-01T09:00:00+09:00"),
        ("ARG BUILD_JOBS=16", "2026-01-01T09:00:00+09:00"),
        ("ARG SM12X_PORT=0", "2026-01-01T09:00:00+09:00"),
        ("RUN |2 NVIDIA_X=1 SM12X_PORT=1 /bin/sh -c base-layer # buildkit", "2025-12-01T00:00:00+09:00"),
        ("ADD file:abc in / ", "2025-12-01T00:00:00+09:00"),
    ]);
    let noprobe = || ImgOpts { probe: false, ..Default::default() };
    let dk_rev = _fake_image_docker(&layers_rev, "2026-01-02T03:04:05+09:00", noprobe());
    let rrv = coll(&repo, &ev_rev, &repo.join("p_rev"), Some(&dk_rev))?;
    let srv = rrv["slots"]["build_recipe"]["evidence"]["selected_revision"].clone();
    st.ck(
        "★F1 작업트리가 이미지 history 와 다르면 Created 이전 개정을 싣는다(Created 뒤 커밋 ✗)",
        eqs(&srv["commit"], &rev1)
            && jeq(&srv["history_match"], &p(r#"{"matched": 5, "total": 5}"#))
            && after_first_line(&rt(&repo.join("p_rev/artifacts/build_recipe/Dockerfile.rev"))?) == df_rev
            && srv["candidates"].items().iter().any(|c| jeq(c, &p(r#"{"revision": "worktree", "matched": 4, "total": 8}"#))),
    );
    let rvr = by(&rrv["slots"]["build_recipe"]["evidence"]["recipe_vs_image"], "path");
    st.ck(
        "F1 대조로 확인된 개정을 실으면 작업트리 경고 대신 무엇이 실렸는지 적는다",
        eqs(&rvr["output/multi/Dockerfile.rev"]["shipped"], &format!("git:{rev1}")),
    );
    st.ck(
        "★F1 COPY 판독 = 이미지를 지은 개정 텍스트(작업트리가 더한 post COPY 는 이 빌드의 입력이 아니다)",
        rrv["applied_set"]["excluded"].items().iter().any(|x| eqs(&x["file"], "10-shared.sh") && sw(&x["why"], "excluded_by_recipe")),
    );
    let brv = steps_by(rrv["reproduce_steps"].items())["build"].clone();
    let brv_cmd = sv(&brv["command"]);
    st.ck(
        "F1·F5 빌드 명령 = 개정 텍스트의 ARG 로 거른 history 인자 · 층 시각 범위(캐시 층 포함 · 소요 아님)",
        brv_cmd.contains("-f output/multi/Dockerfile.rev")
            && brv_cmd.contains("--build-arg BUILD_JOBS=8")
            && brv_cmd.contains(&format!("git:{rev1}"))
            && eqs(&brv["observed"]["layer_span"]["oldest_utc"], "2026-01-01T00:00:00Z")
            && brv["observed"]["layer_span"]["layers"].as_i64() == Some(5),
    );
    st.ck(
        "★빌드 관측 구간 = 층 CreatedAt 첫 층 → 끝 층(phase 기록 없음) · 한계 layer-window · 소요 문구가 창임을 말한다",
        eqs(&brv["observed"]["start_utc"], "2026-01-01T00:00:00Z")
            && eqs(&brv["observed"]["end_utc"], "2026-01-01T18:04:00Z")
            && eqs(&brv["observed"]["bound"], LAYER_WINDOW)
            && brv["observed"]["seconds"].as_i64() == Some(65040)
            && sw(&brv["duration"], "층 창 18시간 4분 0초")
            && has(&brv["duration"], "빌드 소요 아님")
            && has(&brv["source"], "첫 층 → 끝 층"),
    );
    let rph = steps_by(&repro(&repo, &with(&ev_rev, vec![("campaign_id", "campx".into()), ("declaration", decl.clone())]), &dk_rev)?);
    st.ck(
        "★캠페인 phase 기록(exact)이 층 창보다 먼저 · 층 범위는 부가 사실로 남는다",
        eqs(&rph["build"]["observed"]["bound"], "exact")
            && rph["build"]["observed"]["seconds"].as_i64() == Some(5400)
            && rph["build"]["observed"]["layer_span"]["layers"].as_i64() == Some(5),
    );
    // ★F4 다른 ref · 다른 통로(output/single)
    let df_x = df_rev.replace("fx-pkg", "fx-single-pkg");
    let req_x: &[u8] = b"transformers<5.14.1\n# single\n";
    let layers_x: Vec<(String, String)> = layers_rev.iter().map(|(b_, at_)| (b_.replace("fx-pkg", "fx-single-pkg"), at_.clone())).collect();
    let blob = |data: &[u8]| git_stdin(&repo, &["hash-object", "-w", "--stdin"], data, &genv);
    let tree = |entries: &[(&str, &str, String, &str)]| {
        let body: String = entries.iter().map(|(m, t, h, n)| format!("{m} {t} {h}\t{n}\n")).collect();
        git_stdin(&repo, &["mktree"], body.as_bytes(), &genv)
    };
    let t_bps = tree(&[("100644", "blob", blob(b"")?, ".gitkeep")])?;
    let t_single = tree(&[
        ("100644", "blob", blob(df_x.as_bytes())?, "Dockerfile.rev"),
        ("100644", "blob", blob(req_x)?, "requirements.txt"),
        ("040000", "tree", t_bps, "build_patches_src"),
    ])?;
    let t_root = tree(&[("040000", "tree", tree(&[("040000", "tree", t_single, "single")])?, "output")])?;
    let genv_late = genv_at("2026-01-05T00:00:00Z")?;
    let c_single = git_stdin(&repo, &["commit-tree", &t_root, "-m", "single-fx"], b"", &genv_late)?;
    git(&repo, &["update-ref", "refs/heads/single-fx", &c_single], &genv)?;
    let r = (|| -> HR<(J, J, J, J)> {
        let dk_x = _fake_image_docker(&layers_x, "2026-01-02T03:04:05+09:00", noprobe());
        let rx = coll(&repo, &ev_rev, &repo.join("p_rev_x"), Some(&dk_x))?;
        let cx = ctx(&repo, &ev_rev, Some(&dk_x))?;
        let qx = _select_copied(&cx, "requirements.txt", &out.join("requirements.txt"), &J::Null, "탐침 안 함", Some(&core::sha256_bytes(req_x)))?;
        let qn = _select_copied(&ctx(&repo, &ev_rev, Some(&dk_x))?, "requirements.txt", &out.join("requirements.txt"), &J::Null, "탐침 안 함", Some(&"0".repeat(64)))?;
        Ok((rx, J::Null, qx, qn))
    })();
    git(&repo, &["update-ref", "-d", "refs/heads/single-fx"], &genv)?;
    let (rx, _, qx, qn) = r?;
    let sx = rx["slots"]["build_recipe"]["evidence"]["selected_revision"].clone();
    let bx = sv(&steps_by(rx["reproduce_steps"].items())["build"]["command"]);
    st.ck(
        "★F4 다른 ref · 다른 통로(output/single)의 전수 일치 Dockerfile = 그 커밋·경로로 싣는다(Created 뒤 커밋이어도 · 바이트가 증거)",
        eqs(&sx["commit"], &c_single)
            && sx["history_match"]["matched"] == sx["history_match"]["total"]
            && after_first_line(&rt(&repo.join("p_rev_x/artifacts/build_recipe/Dockerfile.rev"))?) == df_x
            && has(&sx["method"], "output/single/Dockerfile.rev")
            && has(&sx["method"], "통로(output/multi/Dockerfile.rev) 밖"),
    );
    let bx_lines: Vec<&str> = bx.trim_end().lines().collect();
    st.ck(
        "★F4 빌드 예시 = 현재 토폴로지 통로 · 과거 실제 경로 및 동등성 미관측 명시",
        bx.contains("-f output/multi/Dockerfile.rev")
            && bx_lines.len() >= 2
            && bx_lines[bx_lines.len() - 2].trim() == "output/multi"
            && bx.contains("동등 여부는 별도 확인")
            && bx.contains("실린 compose 파일만 단독")
            && bx.contains("override가 없을")
            && bx.contains("실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인")
            && !bx.contains("--build-only"),
    );
    let rvx = by(&rx["slots"]["build_recipe"]["evidence"]["recipe_vs_image"], "path");
    st.ck(
        "★F15 레시피 대조 행 = 실린 파일의 경로·커밋(이 체크아웃 통로 작업트리 경로 · 그 마지막 커밋 ✗)",
        rvx.get("output/single/Dockerfile.rev").is_some()
            && rvx.get("output/multi/Dockerfile.rev").is_none()
            && eqs(&rvx["output/single/Dockerfile.rev"]["last_commit_utc"], "2026-01-05T00:00:00Z")
            && eqs(&rvx["output/single/Dockerfile.rev"]["checkout_path"], "output/multi/Dockerfile.rev")
            && rvx["output/single/Dockerfile.rev"].get("warning").is_none(),
    );
    let exx: Vec<J> = rx["applied_set"]["excluded"].items().iter().filter(|x| eqs(&x["phase"], "pre")).cloned().collect();
    st.ck(
        "★F24 패치 디렉터리 = 이미지를 지은 레시피 통로의 커밋 트리(.gitkeep 만) — 작업트리 output/multi/build_patches_src 의 50·55·60 은 열거되지 않는다 · 실린 pre 패치 0",
        exx.iter().map(|x| x["file"].display()).collect::<Vec<_>>() == [".gitkeep"] && !rx["slots"]["build_patch_pre"]["files"].truthy(),
    );
    let bmx_rows: Vec<J> = rx["build_context_map"].items().iter().filter(|r| r["slot_path"].truthy()).cloned().collect();
    let bmx = by(&J::List(bmx_rows), "slot_path");
    let b_df = bmx["artifacts/build_recipe/Dockerfile.rev"].clone();
    st.ck(
        "★F15 이력 통로가 달라도 build_recipe 예시는 현재 토폴로지 뿌리 · 이력 출처는 별도",
        eqs(&b_df["context_root"], "output/multi")
            && has(&b_df["basis"], "docker build -f output/multi/Dockerfile.rev")
            && has(&b_df["basis"], "실린 compose 파일만 단독")
            && has(&b_df["basis"], "override가 없는")
            && has(&b_df["basis"], "실제 원본·후속층 실행 경로는 별도 실행 기록에서 확인")
            && !has(&b_df["basis"], "compose build.dockerfile")
            && !bmx.as_dict().unwrap().iter().any(|(k, r)| r["context_root"].truthy() && (k.contains("/triplet/") || k.contains("/compose/"))),
    );
    let mkx: Vec<J> = rx["build_context_map"].items().iter().filter(|r| eqs(&r["create"], "empty-dir")).cloned().collect();
    st.ck(
        "★F13 실린 Dockerfile 이 COPY 하는 패치 디렉터리에 실린 파일이 없으면 빈 디렉터리 생성 행 · 빌드 명령에 mkdir -p(컨텍스트 통로)",
        mkx.iter().map(|r| r["context_path"].display()).collect::<Vec<_>>() == ["build_patches_src/"]
            && has(&mkx[0]["basis"], "미관측")
            && bx.contains("mkdir -p output/multi/build_patches_src"),
    );
    st.ck(
        "ledger hash mismatch does not claim direct image-file observation",
        is_false(&qn["ledger_sha256_match"]) && !qn["image_bytes_differ"].truthy(),
    );
    st.ck(
        "★F4 requirements = 원장 sha256 과 바이트가 같은 이력 blob(다른 ref · 다른 통로) · verified · 출처 경로 · 음성대조: 없으면 종전(쓰인 바이트 아님)",
        is_true(&qx["verified"])
            && eqs(&qx["commit"], &c_single)
            && eqs(&qx["origin_path"], "output/single/requirements.txt")
            && _j_bytes(&qx["bytes"]).as_deref() == Some(req_x)
            && is_false(&qn["verified"])
            && has(&qn["method"], "전 이력"),
    );
    let rbad = coll(
        &repo,
        &ev_rev,
        &repo.join("p_rev_bad"),
        Some(&_fake_image_docker(
            &layers(&[("RUN |1 SM12X_PORT=0 /bin/sh -c something-else # buildkit", "2026-01-02T03:00:00+09:00")]),
            "2026-01-02T03:04:05+09:00",
            noprobe(),
        )),
    )?;
    let sbad = rbad["slots"]["build_recipe"]["evidence"]["selected_revision"].clone();
    let rvb = by(&rbad["slots"]["build_recipe"]["evidence"]["recipe_vs_image"], "path");
    let fr3 = by(&rbad["slots"]["build_recipe"]["file_records"], "path")["artifacts/build_recipe/Dockerfile.rev"].clone();
    let body3 = rt(&repo.join("p_rev_bad/artifacts/build_recipe/Dockerfile.rev"))?;
    st.ck(
        "★F1 어느 개정도 더 맞지 않으면 작업트리 + 경고(대조 결과 기재)",
        sbad["commit"].is_null()
            && has(&sbad["method"], "어느 개정도")
            && rvb["output/multi/Dockerfile.rev"].get("warning").is_some()
            && after_first_line(&body3) == df_wt,
    );
    let hm3x = sbad["history_match"].clone();
    let hline = fr3["header"]["line"].as_i64().unwrap_or(0);
    let body3_lines: Vec<&str> = body3.split('\n').collect();
    st.ck(
        "★F3 history 대조 불일치 Dockerfile = generated-unverified(renderer) · 표시 3자리(머리 경고 = file_records header · 대조 수)",
        eqs(&fr3["verification"], "generated-unverified")
            && eqs(&fr3["generated_by"], "renderer")
            && hline >= 1
            && body3_lines.get((hline - 1) as usize).copied() == fr3["header"]["text"].as_str()
            && has(&fr3["header"]["text"], &format!("{}/{} 일치", hm3x["matched"].display(), hm3x["total"].display()))
            && !is_true(&fr3["bytes_observed"]),
    );
    let bb3 = sv(&steps_by(rbad["reproduce_steps"].items())["build"]["command"]);
    st.ck(
        "★F3 빌드 머리 = '실린 개정 … 확인 못함'('이미지를 지은 개정' ✗) · 전수 일치 개정은 종전 문구",
        bb3.contains("실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함")
            && !bb3.contains("이미지를 지은 개정:")
            && brv_cmd.contains("실린 개정: git:"),
    );
    let dfx = repo.join("p_dirx.Dockerfile");
    wt(&dfx, "# syntax=docker/dockerfile:1\nFROM x\n")?;
    let hx = _insert_header(&repo, "p_dirx.Dockerfile", "r", "renderer")?;
    let dfx_t = rt(&dfx)?;
    let dfx_l: Vec<&str> = dfx_t.split('\n').collect();
    st.ck(
        "★F3 Dockerfile 파서 지시자(# syntax=) 뒤에 경고(지시자 보존)",
        hx["line"].as_i64() == Some(2) && dfx_l.len() >= 2 && dfx_l[0] == "# syntax=docker/dockerfile:1" && Some(dfx_l[1]) == hx["text"].as_str(),
    );
    rm(&dfx)?;
    let rnc = coll(&repo, &ev_rev, &repo.join("p_rev_nocreated"), Some(&_fake_image_docker(&layers_rev, "not-a-time", noprobe())))?;
    let snc = rnc["slots"]["build_recipe"]["evidence"]["selected_revision"].clone();
    st.ck(
        "★F1 이미지 Created 미관측 = 시각 기반 개정 후보 없음 · F4 내용 대조(전 이력 전수 일치)만 개정을 고른다",
        snc["candidates"].items().len() == 1
            && eqs(&snc["commit"], &rev1)
            && has(&snc["method"], "전수 일치")
            && snc["history_match"]["matched"] == snc["history_match"]["total"],
    );
    let l0 = format!("{}\n", layers_rev[0].0);
    let rplain = coll(
        &repo,
        &ev_rev,
        &repo.join("p_rev_plain"),
        Some(&_fake_docker(&[
            (&["image", "inspect", "--format", "{{.Created}}"], (0, "2026-01-02T03:04:05+09:00\n")),
            (&["image", "inspect"], (0, "sha256:fx\n")),
            (&["history"], (0, &l0)),
        ])),
    )?;
    st.ck(
        "★F1 history JSON 판독 불가 = 대조하지 않는다(작업트리 · '미관측' 기재 · 다른 형식으로 대체 ✗)",
        has(&rplain["slots"]["build_recipe"]["evidence"]["selected_revision"]["method"], "미관측")
            && rplain["slots"]["build_recipe"]["evidence"]["selected_revision"]["commit"].is_null(),
    );
    wt(&out.join("Dockerfile.rev"), &df_rev)?;
    let rsame = coll(&repo, &ev_rev, &repo.join("p_rev_same"), Some(&dk_rev))?;
    let ssame = rsame["slots"]["build_recipe"]["evidence"]["selected_revision"].clone();
    let rvs = by(&rsame["slots"]["build_recipe"]["evidence"]["recipe_vs_image"], "path");
    let same_record = rsame["slots"]["build_recipe"]["file_records"]
        .items()
        .iter()
        .find(|r| name_of(&sv(&r["path"])) == "Dockerfile.rev")
        .cloned()
        .unwrap_or(J::Null);
    st.ck(
        "history command match does not prove full bytes or imply execution absent",
        ssame["history_match"]["matched"] == ssame["history_match"]["total"]
            && eqs(&same_record["verification"], "generated-unverified")
            && is_false(&same_record["bytes_observed"])
            && eqs(&same_record["relevance"], "unknown")
            && has(&same_record["relevance_basis"], "경로 실행 여부나 필요 여부를 판정하지 않는다")
            && !has(&same_record["relevance_basis"], "실행되지 않았다"),
    );
    st.ck(
        "F1 작업트리가 history 와 전수 일치 = 작업트리(HEAD 대비 수정본 경고는 실린 파일의 결함이 아니다)",
        ssame["commit"].is_null()
            && sw(&ssame["method"], "worktree(측정 이미지 docker history")
            && is_false(&rvs["output/multi/Dockerfile.rev"]["worktree_equals_head"])
            && has(&ssame["method"], "history"),
    );
    let want_steps = vec![
        "COPY requirements.txt /tmp/requirements.txt".to_string(),
        "RUN pip install fx-pkg && echo done".to_string(),
        "RUN python3 - <<'PY' import sys print(\"hoist\") PY".to_string(),
        "COPY build_patches_src/ /tmp/build_patches_src/".to_string(),
        "RUN for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done".to_string(),
    ];
    let steps_ = _dockerfile_steps(&df_rev)?;
    let hist5: Vec<Option<String>> = layers_rev[..5].iter().map(|(b, _)| _history_step(b).ok().flatten()).collect();
    let rev_steps: Vec<Option<String>> = steps_.iter().rev().cloned().map(Some).collect();
    st.ck("정규화: 이어짐 주석·--mount·heredoc·빌더 스테이지", steps_ == want_steps && hist5 == rev_steps);
    sec_git_compose(st, fx, &genv, &genv3)?;
    let _ = assets;
    Ok(())
}

// ── compose 슬롯 = 측정 당시 개정 ──
fn sec_git_compose(st: &mut St, fx: &mut Fx, genv: &[(String, String)], genv3: &[(String, String)]) -> HR<()> {
    let (repo, out, ev) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone());
    let docker_old = fx.docker_old.clone();
    let assets = fx.assets.clone();
    let asset_rel = ".claude/skills/upstream-version-watch/assets/configs/serve_runner.sh";
    let compose0 = rb(&out.join("docker-compose.yaml"))?;
    let runner0 = rb(&assets.join("serve_runner.sh"))?;
    let ev_nomeas = with(&ev, vec![("sweep", without(&ev["sweep"], "generated_utc"))]);
    let r = (|st: &mut St| -> HR<()> {
        git(&repo, &["add", "output/multi/docker-compose.yaml", asset_rel], genv)?;
        git_commit(&repo, "compose", genv)?;
        let rev_c = core::git_out(&repo, &["rev-parse", "HEAD"])?;
        let c0 = String::from_utf8_lossy(&compose0).to_string();
        let compose_late = c0
            .replacen("services:\n", "# 측정 뒤 주석: NCCL 19 keys + envs/.env.late\nservices:\n", 1)
            .replace("${CONFIG_FILE:-default}", "${CONFIG_FILE:-latecfg}")
            .replacen("profiles: [master]", "profiles: [master]\n    command: [\"bash\", \"/app/configs/late_runner.sh\"]", 1);
        wt(&out.join("docker-compose.yaml"), &compose_late)?;
        git(&repo, &["add", "output/multi/docker-compose.yaml"], genv3)?;
        git_commit(&repo, "late compose", genv3)?;
        let late_c = core::git_out(&repo, &["rev-parse", "HEAD"])?;
        let mut r_late = runner0.clone();
        r_late.extend_from_slice("# 측정 뒤 수정\n".as_bytes());
        wb(&assets.join("serve_runner.sh"), &r_late)?;
        wt(&out.join("envs/.env.late"), "LATE_KEY=1\n")?;
        for (p_, t_) in [
            (out.join("docker-compose.yaml"), "2026-01-02T00:00:00Z"),
            (assets.join("serve_runner.sh"), "2026-01-02T00:00:00Z"),
            (out.join("configs/serve_runner.sh"), "2026-01-01T10:00:00Z"),
            (out.join("envs/.env.late"), "2026-01-01T00:00:00Z"),
        ] {
            _set_mtime(&p_, t_)?;
        }
        let rcr = coll(&repo, &ev, &repo.join("p_crev"), Some(&docker_old))?;
        let cev = rcr["slots"]["compose"]["evidence"].clone();
        let csr = cev["selected_revision"].clone();
        st.ck(
            "★compose 작업트리가 측정 뒤 바뀌었으면 측정 전 마지막 커밋 바이트(측정 뒤 커밋 ✗) · 방법·교차대조 기재",
            rb(&repo.join("p_crev/artifacts/compose/docker-compose.yaml"))?.ends_with(&compose0)
                && eqs(&csr["commit"], &rev_c)
                && eqs(&csr["shipped"], &format!("git:{rev_c}"))
                && eqs(&csr["path"], "output/multi/docker-compose.yaml")
                && eqs(&csr["basis"], "git-rev-before-measurement")
                && is_false(&csr["verified"])
                && strs(&csr["commits_after_measurement"]) == vec![late_c[..12].to_string()]
                && strs(&csr["cross_check"]).join(" ").contains("측정 뒤 커밋 1")
                && has(&csr["method"], "git log -1 --before=2026-01-01T19:30:00Z -- output/multi/docker-compose.yaml")
                && csr.get("history_match").is_some_and(J::is_null)
                && csr.get("bytes").is_none(),
        );
        let rsr = if rcr["sub_recipe"].truthy() { rcr["sub_recipe"].clone() } else { J::dict() };
        st.ck(
            "★측정 뒤 더해진 env_file 참조(.env.late)·러너(late_runner.sh)·slave CONFIG_FILE 기본값은 이 셀의 입력이 아니다(측정 당시 개정 텍스트로 파생 · 서브 전달 파생도 그 개정 파일)",
            !has(&rcr["files"], "artifacts/compose/.env.late.template")
                && !rcr["env_shapes"].items().iter().any(|e| eqs(&e["file"], "envs/.env.late"))
                && !has(&rcr["files"], "artifacts/compose/late_runner.sh")
                && eqs(&rsr["role_diff"]["slave_config_file"], "default")
                && eqs(&subr(&repo, &ev)?["role_diff"]["slave_config_file"], "default"),
        );
        let srun = by(&cev["selected_revisions"], "path");
        let shipped_runner = rb(&repo.join("p_crev/artifacts/compose/serve_runner.sh"))?;
        st.ck(
            "★러너 정본이 측정 뒤 바뀌어도 측정 전 커밋 = 서빙한 마운트본(mtime ≤ 측정)과 일치 → 차단 ✗ · 실린 러너 = 측정 당시 바이트",
            drop_second_line(&shipped_runner) == runner0
                && eqs(&srun[asset_rel]["commit"], &rev_c)
                && srun[asset_rel]["cross_check"].items().iter().any(|x| has(x, "마운트본의 현재 mtime 관측"))
                && keys(&srun).is_superset(&set_of(&["output/multi/docker-compose.yaml", asset_rel])),
        );
        st.ck(
            "공유 계약: compose 선택 행 = build_recipe 와 같은 자리(selected_revision · selected_revisions) · 문구에 compose 개정",
            has(&cev["note"], "compose = 측정 전 이력 기반 후보 git:") && cev["selected_revisions"].is_list(),
        );
        let mut rd_ = runner0.clone();
        rd_.extend_from_slice(b"# served-different\n");
        wb(&out.join("configs/serve_runner.sh"), &rd_)?;
        _set_mtime(&out.join("configs/serve_runner.sh"), "2026-01-01T10:00:00Z")?;
        st.raises("★서빙한 마운트본(mtime ≤ 측정) ≠ 실을 러너", "HINT_RUNNER_RENDER_DRIFT", || coll(&repo, &ev, &repo.join("p_crev_drift"), Some(&docker_old)));
        wb(&out.join("configs/serve_runner.sh"), &runner0)?;
        _set_mtime(&out.join("configs/serve_runner.sh"), "2026-01-01T10:00:00Z")?;
        wb(&out.join("docker-compose.yaml"), &compose0)?;
        st.raises("★측정 시각 미관측 + 정본 ≠ 마운트본", "HINT_RUNNER_RENDER_DRIFT", || coll(&repo, &ev_nomeas, &repo.join("p_crev_nomeas"), Some(&docker_old)));
        wb(&assets.join("serve_runner.sh"), &runner0)?;
        _set_mtime(&assets.join("serve_runner.sh"), "2026-01-01T00:00:00Z")?;
        let mut compose_wt = compose0.clone();
        compose_wt.extend_from_slice("# 측정 전 미커밋 주석\n".as_bytes());
        wb(&out.join("docker-compose.yaml"), &compose_wt)?;
        _set_mtime(&out.join("docker-compose.yaml"), "2026-01-01T10:00:00Z")?;
        let rcw = coll(&repo, &ev, &repo.join("p_crev_wt"), Some(&docker_old))?;
        let cw = rcw["slots"]["compose"]["evidence"]["selected_revision"].clone();
        st.ck(
            "★compose mtime ≤ 측정 = 작업트리(측정 당시 바이트 · verified) · 측정 전 커밋과 다르면 미커밋 사실 기재",
            rb(&repo.join("p_crev_wt/artifacts/compose/docker-compose.yaml"))?.ends_with(&compose_wt)
                && eqs(&cw["shipped"], "worktree")
                && eqs(&cw["basis"], "mtime≤measured")
                && is_false(&cw["verified"])
                && cw["commit"].is_null()
                && has(&cw["method"], "측정 당시 바이트는 미검증"),
        );
        _set_mtime(&out.join("docker-compose.yaml"), "2026-01-02T00:00:00Z")?;
        let rcn = coll(&repo, &ev_nomeas, &repo.join("p_crev_none"), Some(&docker_old))?;
        let cn = rcn["slots"]["compose"]["evidence"]["selected_revision"].clone();
        st.ck(
            "★측정 시각 미관측 = compose 작업트리 · basis unobserved · '측정 시각 미관측' 기재",
            eqs(&cn["shipped"], "worktree")
                && eqs(&cn["basis"], "unobserved")
                && cn["commit"].is_null()
                && has(&cn["method"], "측정 시각 미관측")
                && rb(&repo.join("p_crev_none/artifacts/compose/docker-compose.yaml"))?.ends_with(&compose_wt),
        );
        wb(&out.join("docker-compose.yaml"), &compose0)?;
        _set_mtime(&out.join("docker-compose.yaml"), "2026-01-02T00:00:00Z")?;
        let rcs = coll(&repo, &ev, &repo.join("p_crev_same"), Some(&docker_old))?;
        let cs = rcs["slots"]["compose"]["evidence"]["selected_revision"].clone();
        st.ck(
            "compose mtime 만 뒤(바이트 = 측정 전 커밋) = 작업트리 · commit 기재 · git 개정 선택 방법",
            eqs(&cs["shipped"], "worktree") && eqs(&cs["commit"], &rev_c) && eqs(&cs["basis"], "git-rev-before-measurement") && has(&cs["method"], "바이트가 그 개정과 같다"),
        );
        let c_un = ctx(&repo, &ev, None)?;
        let un = _select_tracked(&c_un, &out.join("envs/.env.late"))?;
        _set_mtime(&out.join("envs/.env.late"), "2026-01-02T00:00:00Z")?;
        let un2 = _select_tracked(&ctx(&repo, &ev, None)?, &out.join("envs/.env.late"))?;
        st.ck(
            "★측정 전 개정 없는 파일: mtime ≤ 측정 = verified · 측정 뒤 = 작업트리 + 경고",
            is_false(&un["verified"]) && eqs(&un["basis"], "mtime≤measured") && eqs(&un2["shipped"], "worktree") && un2.get("warning").is_some() && un2["commit"].is_null(),
        );
        // (a) 마운트본 측정 뒤 재작성
        wb(&out.join("docker-compose.yaml"), &compose0)?;
        _set_mtime(&out.join("docker-compose.yaml"), "2026-01-01T00:00:00Z")?;
        let r0 = String::from_utf8_lossy(&runner0).to_string();
        wt(&assets.join("serve_runner.sh"), &r0.replace("= \"2\"", "= \"4\"").replace("0.0/2.0", "0.0/4.0"))?;
        _set_mtime(&assets.join("serve_runner.sh"), "2026-01-02T00:00:00Z")?;
        let mut rm2 = runner0.clone();
        rm2.extend_from_slice("# 측정 뒤 재materialize\n".as_bytes());
        wb(&out.join("configs/serve_runner.sh"), &rm2)?;
        _set_mtime(&out.join("configs/serve_runner.sh"), "2026-01-02T00:00:00Z")?;
        let rms = coll(&repo, &ev, &repo.join("p_crev_mnt"), Some(&docker_old))?;
        let msr = by(&rms["slots"]["compose"]["evidence"]["selected_revisions"], "path")[asset_rel].clone();
        st.ck(
            "★마운트본 측정 뒤 재작성 + 정본 측정 뒤 수정 = 차단 ✗(서빙 증거 아님 기재) · 실린 러너 = 측정 전 커밋 바이트",
            drop_second_line(&rb(&repo.join("p_crev_mnt/artifacts/compose/serve_runner.sh"))?) == runner0
                && eqs(&msr["commit"], &rev_c)
                && msr["cross_check"].items().iter().any(|x| has(x, "서빙 증거 아님")),
        );
        let rmsr = if rms["sub_recipe"].truthy() { rms["sub_recipe"].clone() } else { J::dict() };
        st.ck(
            "★sub_recipe GPU 합류 리터럴 = 측정 당시 개정의 러너(정본이 측정 뒤 \"4\" 로 바뀌어도 \"2\")",
            eqs(&rmsr["role_diff"]["runner_gpu_join_literal"]["value"], "2") && eqs(&subr(&repo, &ev)?["role_diff"]["runner_gpu_join_literal"]["value"], "2"),
        );
        wb(&assets.join("serve_runner.sh"), &runner0)?;
        _set_mtime(&assets.join("serve_runner.sh"), "2026-01-01T00:00:00Z")?;
        st.raises(
            "★마운트본 측정 뒤 재작성 + 정본 = 측정 당시 바이트(작업트리) 인데 다르다 → 렌더 드리프트 차단 유지",
            "HINT_RUNNER_RENDER_DRIFT",
            || coll(&repo, &ev, &repo.join("p_crev_mnt2"), Some(&docker_old)),
        );
        wb(&out.join("configs/serve_runner.sh"), &runner0)?;
        _set_mtime(&out.join("configs/serve_runner.sh"), "2026-01-01T00:00:00Z")?;
        // (b) render 단계: 렌더러 측정 뒤 변경 경고
        let rd_path = repo.join(core::REL_RENDER_DOCKERFILE_SRC);
        let rd0 = rb(&rd_path)?;
        let rr = (|st: &mut St| -> HR<()> {
            _set_mtime(&rd_path, "2026-01-02T00:00:00Z")?;
            let st0 = steps_by(&repro(&repo, &ev, &docker_old)?)["render"].clone();
            let rr0 = if st0["renderer_revision"].truthy() { st0["renderer_revision"].clone() } else { J::dict() };
            st.ck(
                "★렌더러 측정 전 개정 없음(비추적 · mtime 만 뒤) = 드리프트 경고 ✗(지어내지 않는다) · 행에 경고 사실",
                sw(&st0["command"], "bash ") && !has(&st0["command"], "⚠") && rr0.get("commit").is_some_and(J::is_null) && rr0.get("warning").is_some(),
            );
            git(&repo, &["add", core::REL_RENDER_DOCKERFILE_SRC], genv)?;
            git_commit(&repo, "renderer", genv)?;
            let rev_r = core::git_out(&repo, &["rev-parse", "HEAD"])?;
            _set_mtime(&rd_path, "2026-01-01T00:00:00Z")?;
            let st1 = steps_by(&repro(&repo, &ev, &docker_old)?)["render"].clone();
            let rr1 = if st1["renderer_revision"].truthy() { st1["renderer_revision"].clone() } else { J::dict() };
            st.ck(
                "★렌더러 mtime ≤ 측정 = 측정 당시 렌더러 · 경고 ✗",
                !has(&st1["command"], "⚠") && eqs(&rr1["basis"], "mtime≤measured") && eqs(&rr1["commit"], &rev_r),
            );
            let mut rd1 = rd0.clone();
            rd1.extend_from_slice("# 측정 뒤 렌더러 수정(소켓 기준선 흉내)\n".as_bytes());
            wb(&rd_path, &rd1)?;
            git(&repo, &["add", core::REL_RENDER_DOCKERFILE_SRC], genv3)?;
            git_commit(&repo, "late renderer", genv3)?;
            let late_r = core::git_out(&repo, &["rev-parse", "HEAD"])?;
            _set_mtime(&rd_path, "2026-01-02T00:00:00Z")?;
            let st2 = steps_by(&repro(&repo, &ev, &docker_old)?)["render"].clone();
            let cmd2 = sv(&st2["command"]);
            let l2: Vec<&str> = cmd2.split('\n').collect();
            let rr2 = if st2["renderer_revision"].truthy() { st2["renderer_revision"].clone() } else { J::dict() };
            st.ck(
                "★렌더러가 측정 뒤 바뀌었으면(git 관측) render 명령 앞에 경고 주석 · 측정 전 커밋 · 측정 뒤 커밋 수 · git show",
                l2.len() >= 3
                    && l2[0].starts_with("# ⚠ 측정 당시 렌더러 ≠ 작업트리 렌더러")
                    && l2[0].contains(&rev_r[..12])
                    && l2[0].contains("측정 뒤 커밋 1건")
                    && l2[0].contains(&late_r[..12])
                    && l2[1].contains(&format!("git show {}:{}", &rev_r[..12], core::REL_RENDER_DOCKERFILE_SRC))
                    && l2[1].contains(ENV_OBSERVED_POINTER)
                    && !re_search(r"(?<![A-Za-z_])facts\.", &l2.join("\n"))
                    && l2[2..].iter().any(|x| x.starts_with("bash ") && x.contains("--nccl-envfile"))
                    && eqs(&rr2["commit"], &rev_r)
                    && strs(&rr2["commits_after_measurement"]) == vec![late_r[..12].to_string()]
                    && rr2.get("bytes").is_none(),
            );
            Ok(())
        })(st);
        wb(&rd_path, &rd0)?;
        _set_mtime(&rd_path, "2026-01-01T00:00:00Z")?;
        rr
    })(st);
    wb(&assets.join("serve_runner.sh"), &runner0)?;
    wb(&out.join("configs/serve_runner.sh"), &runner0)?;
    wb(&out.join("docker-compose.yaml"), &compose0)?;
    rm_ok(&out.join("envs/.env.late"));
    for p_ in [out.join("docker-compose.yaml"), assets.join("serve_runner.sh"), out.join("configs/serve_runner.sh")] {
        _set_mtime(&p_, "2026-01-01T00:00:00Z")?;
    }
    r
}

// ── 캠페인 phase · 잔재 · 실물 렌더러 · 드리프트 ──
fn sec_phases_drift(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let ip = fx.ip.clone();
    for (nid, s_, e_, cid_cell) in [
        ("gx-a", "2026-01-01T10:00:00Z", "2026-01-01T11:00:00Z", cell.as_str()),
        ("gx-b", "2026-01-01T10:05:00Z", "2026-01-01T11:30:00Z", cell.as_str()),
        ("gx-c", "2026-01-01T01:00:00Z", "2026-01-01T23:00:00Z", "other-cell"),
        ("main", "2026-01-01T00:00:00Z", "2026-01-01T23:59:00Z", cell.as_str()),
    ] {
        mkd(&repo.join(format!("campaigns/campx/phases/{nid}")))?;
        wt(
            &repo.join(format!("campaigns/campx/phases/{nid}/build.status.json")),
            &dumps(&jd(&[("cell_id", cid_cell.into()), ("started_utc", s_.into()), ("ended_utc", e_.into())])),
        )?;
    }
    let decl = p(r#"{"nodes": [{"node_id": "gx-a"}, {"node_id": "gx-b"}, {"node_id": "gx-c"}]}"#);
    let rs = steps_by(&repro(&repo, &with(&ev, vec![("campaign_id", "campx".into()), ("declaration", decl.clone())]), &docker_old)?);
    st.ck(
        "빌드 = 캠페인 phase(선언 노드 id span · ★다른 셀·선언 밖 디렉터리 제외)",
        rs["build"]["observed"]["seconds"].as_i64() == Some(5400) && eqs(&rs["build"]["observed"]["bound"], "exact"),
    );
    let ev_main = with(
        &ev,
        vec![
            ("node", "main".into()),
            ("campaign_id", "campx".into()),
            ("declaration", decl.clone()),
            (
                "phases",
                p(r#"{"build": {"started_utc": "2026-01-01T10:00:00Z", "ended_utc": "2026-01-01T10:30:00Z",
                               "_path": "campaigns/campx/phases/gx-a/build.status.json"}}"#),
            ),
        ],
    );
    let rm_ = steps_by(&repro(&repo, &ev_main, &docker_old)?);
    st.ck("main 축 = evidence 가 해소한 배정 노드 기록(ev.phases)", rm_["build"]["observed"]["seconds"].as_i64() == Some(1800));
    let rm2 = steps_by(&repro(&repo, &with(&ev_main, vec![("phases", J::dict())]), &docker_old)?);
    st.ck("★축 이름 디렉터리(phases/main)를 노드 id 로 추측하지 않는다", rm2["build"]["observed"].get("start_utc").is_some_and(J::is_null));
    st.raises("★artifacts 잔재 차단", "HINT_ARTIFACTS_DIR_NOT_EMPTY", || coll(&repo, &ev, &fx.payload, Some(&docker_old)));
    // ★env_tier 부재 → 같은 모듈 3층 상수에서 파생(소리 나는 폴백 · 실물 render_dockerfile)
    let res_r = coll(&repo, &ev, &repo.join("payload_real"), Some(&docker_old))?;
    let shp = by(&res_r["env_shapes"], "file");
    // Rust: the real render module always has env_tier (its native launcher `--env-tier`).
    let want_rule = "render_dockerfile.env_tier";
    st.ck("실물 render 모듈로 cluster 형상(env_tier 또는 3층 상수 파생)", sw(&shp["envs/.env.cluster"]["rule"], want_rule));
    let clr = rt(&repo.join("payload_real/artifacts/compose/.env.cluster.template"))?;
    st.ck("★실물 규칙에서도 호스트 실값 제거", !clr.contains(&ip) && !clr.contains("opfixture") && clr.contains("MAX_JOBS=4"));
    wt(&out.join("configs/serve_runner.sh"), "drift\n")?;
    st.raises("★러너 드리프트", "HINT_RUNNER_RENDER_DRIFT", || coll(&repo, &ev, &repo.join("p2"), Some(&docker_old)));
    wt(&out.join("configs/serve_runner.sh"), &fx.runner_txt)?;
    let mut drift_ledger = fx.ledger.clone();
    if let Some(l) = drift_ledger.get_mut("patches").and_then(J::as_list_mut) {
        l[1].set("script_sha256", "0".repeat(64));
    }
    let dl_txt = dumps(&drift_ledger);
    st.raises("★적용 패치 드리프트", "HINT_APPLIED_PATCH_DRIFT", || {
        coll(&repo, &ev, &repo.join("p3"), Some(&_fake_docker(&[(&["image", "inspect"], (0, "x")), (&["run"], (0, &dl_txt))])))
    });
    wt(&out.join("envs/.env.interconnect"), "NCCL_DEBUG=fixture-secret-literal\n")?;
    _set_mtime(&out.join("envs/.env.interconnect"), "2026-01-01T00:00:00Z")?;
    st.raises("★형상 백스톱(리터럴)", "HINT_ENV_SHAPE_PII", || coll(&repo, &ev, &repo.join("p4"), Some(&docker_old)));
    wt(&out.join("envs/.env.interconnect"), "NCCL_DEBUG=INFO\nNCCL_SOCKET_IFNAME=enpfixture0\n")?;
    _set_mtime(&out.join("envs/.env.interconnect"), "2026-01-01T00:00:00Z")?;
    fx.decl = decl;
    Ok(())
}

fn jline(j: &J) -> String {
    format!("{}\n", dumps(j))
}

// ── 재현 절차 · 관측 소요 ──
fn sec_repro(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, ev, cell) = (fx.repo.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let res = fx.res.clone();
    let mnt = fx.mnt.clone();
    let steps = steps_by(res["reproduce_steps"].items());
    st.ck(
        "재현 절차 순서",
        res["reproduce_steps"].items().iter().map(|s| s["step"].display()).collect::<Vec<_>>() == ["render", "build", "serve", "bench"],
    );
    st.ck(
        "빌드 종료 = 이미지 Created(UTC) · ★층 CreatedAt 미관측(history JSON 판독 불가)이면 시작을 지어내지 않는다",
        eqs(&steps["build"]["observed"]["end_utc"], "2026-01-01T18:04:05Z")
            && steps["build"]["observed"].get("start_utc").is_some_and(J::is_null)
            && eqs(&steps["build"]["observed"]["bound"], "none"),
    );
    st.ck(
        "기동 상한 = 측정 전 마지막 예산 선언 → 첫 측정(다른 label·뒤 선언 제외)",
        jeq(
            &steps["serve"]["observed"],
            &p(r#"{"start_utc": "2026-01-01T18:30:00Z", "end_utc": "2026-01-01T19:00:00Z", "seconds": 1800, "bound": "upper"}"#),
        ),
    );
    st.ck("벤치 = 첫 측정 → generated_utc", steps["bench"]["observed"]["seconds"].as_i64() == Some(1800));
    let bcmd = sv(&steps["build"]["command"]);
    let ev_phase = with(
        &ev,
        vec![
            ("campaign_id", "fixture-phase".into()),
            ("phases", p(r#"{"build": {"started_utc": "2026-01-01T17:00:00Z", "ended_utc": "2026-01-01T17:37:55Z"}}"#)),
        ],
    );
    let phase_build = find_in(&J::List(repro(&repo, &ev_phase, &docker_old)?), "step", "build");
    let tagv = "${REPRO_IMAGE_TAG:?set a separate reproduction image tag}";
    st.ck(
        "reproduction image tag is separate and phase duration is not attributed to that command",
        bcmd.contains(tagv)
            && phase_build["observed"]["seconds"].as_i64() == Some(2275)
            && has(&phase_build["source"], "측정 이미지의 전체 빌드 소요로 결속되지 않음"),
    );
    st.ck(
        "F5 빌드 = docker build -f <Dockerfile> --build-arg(history) … -t <tag> <context>",
        bcmd.contains("docker build -f output/multi/Dockerfile.source-build \\")
            && bcmd.contains("--build-arg SM12X_PORT=0")
            && bcmd.contains("--build-arg BUILD_JOBS=8")
            && bcmd.contains(tagv)
            && bcmd.contains("\n  output/multi\n")
            && eqs(&steps["build"]["command_source"], "reconstructed(docker-history build-args + compose build 선언/설정(실행 관측 아님))"),
    );
    let (no_args_cmd, no_args_src) = _build_command(&ctx(&repo, &ev, None)?, &p(r#"{"dockerfile": "Dockerfile.source-build"}"#))?;
    st.ck(
        "build-arg 미관측 재구성도 compose 설정과 실행 관측을 구분한다",
        no_args_src.starts_with("reconstructed(compose build 선언/설정(실행 관측 아님) · build-arg 미관측)")
            && no_args_cmd.contains("docker build -f output/multi/Dockerfile.source-build"),
    );
    st.ck(
        "F5 벤치 = bench JSON 필드 복원(vllm bench serve)",
        eqs(&steps["bench"]["command_source"], "reconstructed(bench json)")
            && has(
                &steps["bench"]["command"],
                "vllm bench serve --backend openai --model fx --tokenizer /app/models/X --dataset-name random --random-input-len 1024 --random-output-len 256 --num-prompts 4 --max-concurrency 1 --request-rate inf",
            ),
    );
    st.ck(
        "★F5 재현 명령에 자리표시(§) 없음",
        !res["reproduce_steps"].items().iter().any(|s| sv(&s["command"]).contains('§')),
    );
    st.ck(
        "★PII 면제 기록에 매치 원문 없음(compose `${VAR:-…}` 기본값)",
        res["pii_exempt"].truthy() && !res["pii_exempt"].items().iter().any(|x| sv(x).contains(&mnt)),
    );
    let rs_clock = steps_by(&repro(&repo, &with(&ev, vec![("sweep", with(&ev["sweep"], vec![("generated_utc", "2026-01-01T18:00:00Z".into())]))]), &docker_old)?);
    st.ck(
        "★date 시계 가정 불성립 → serve·bench 미관측",
        rs_clock["serve"]["observed"].get("seconds").is_some_and(J::is_null) && rs_clock["bench"]["observed"].get("seconds").is_some_and(J::is_null),
    );
    st.ck(
        "★P14 lite_raw 없으면 시작 = 첫 JSON date(컨테이너 시계) · 끝 generated_utc(호스트) → bound clock-mixed(exact ✗) · ≈ 근사 표기",
        eqs(&steps["bench"]["observed"]["bound"], CLOCK_MIXED) && sw(&steps["bench"]["duration"], "≈ "),
    );
    let sd = repo.join("output/multi/benchlog/sweep_c1-mmp");
    let p14 = sd.join(format!("lite_raw_{cell}.json"));
    wt(&p14, &dumps(&jd(&[("config_name", cell.as_str().into()), ("measured_utc", "2026-01-01T18:59:41Z".into())])))?;
    let r = (|| -> HR<(J, J)> {
        let rs14 = steps_by(&repro(&repo, &ev, &docker_old)?);
        wt(&p14, &dumps(&p(r#"{"config_name": "other-cell", "measured_utc": "2026-01-01T18:59:41Z"}"#)))?;
        let rs14o = steps_by(&repro(&repo, &ev, &docker_old)?);
        Ok((rs14, rs14o))
    })();
    rm_ok(&p14);
    let (rs14, rs14o) = r?;
    st.ck(
        "★P14 lite_raw measured_utc 가 있으면 bench 시작 = 그것(lite_cold date 19:00:00 ✗) · 두 끝 호스트 시계 → exact · 출처 표시",
        eqs(&rs14["bench"]["observed"]["start_utc"], "2026-01-01T18:59:41Z")
            && rs14["bench"]["observed"]["seconds"].as_i64() == Some(1819)
            && eqs(&rs14["bench"]["observed"]["bound"], "exact")
            && has(&rs14["bench"]["source"], &format!("lite_raw_{cell}.json measured_utc"))
            && has(&rs14["bench"]["source"], HOST_START_MARK)
            && !has(&rs14["bench"]["source"], "컨테이너 시계"),
    );
    let evp = repo.join("docs/logs/main/events/2026-01.jsonl");
    let ev_bytes = rb(&evp)?;
    wt(
        &evp,
        &format!(
            "{}{}{}",
            jline(&jd(&[("kind", "budget_declare".into()), ("label", format!("serve-{cell}").into()), ("ts", "2026-01-01T18:40:00Z".into())])),
            jline(&jd(&[("kind", "budget_declare".into()), ("label", format!("serve-{cell}x").into()), ("ts", "2026-01-01T18:20:00Z".into())])),
            jline(&p(r#"{"kind": "budget_clear", "ts": "2026-01-01T21:00:00Z"}"#)),
        ),
    )?;
    let rs15 = repro(&repo, &ev, &docker_old);
    wb(&evp, &ev_bytes)?;
    let rs15 = steps_by(&rs15?);
    st.ck(
        "★F15 serve-<셀> 라벨 선언(single_serve_up) → serve 상한 시작 · 출처에 그 라벨 · 접두만 같은 다른 셀 라벨 ✗",
        eqs(&rs15["serve"]["observed"]["start_utc"], "2026-01-01T18:40:00Z")
            && eqs(&rs15["serve"]["observed"]["bound"], "upper")
            && has(&rs15["serve"]["source"], &format!("label serve-{cell})")),
    );
    st.ck(
        "★P14 serve 상한의 끝 = lite_raw measured_utc(첫 측정 시각)",
        eqs(&rs14["serve"]["observed"]["end_utc"], "2026-01-01T18:59:41Z") && has(&rs14["serve"]["source"], "첫 측정 시각("),
    );
    st.ck(
        "P14 음성대조: 다른 셀의 lite_raw 는 쓰지 않는다(시작 = JSON date · clock-mixed)",
        eqs(&rs14o["bench"]["observed"]["start_utc"], "2026-01-01T19:00:00Z") && eqs(&rs14o["bench"]["observed"]["bound"], CLOCK_MIXED),
    );
    // F5 · FC1 — 최종 스윕 첫 측정 전 요청 · serve 세션 경계
    let sd5 = sd.clone();
    let (hd5, tl5) = (sd5.join(format!("lite_engine_head_{cell}.log")), sd5.join(format!("lite_engine_{cell}.log")));
    let journal = repo.join("docs/logs/main/sessions").join(format!("serve-{cell}-20260101T183000Z.json"));
    mkd(journal.parent().unwrap())?;
    let jstem = |p_: &Path| p_.file_stem().unwrap().to_string_lossy().to_string();
    let session_doc = jd(&[
        ("session_id", jstem(&journal).into()),
        ("config_file", cell.as_str().into()),
        ("topology", "multi".into()),
        ("started_utc", "2026-01-01T18:30:00Z".into()),
        ("stopped_utc", "2026-01-01T20:00:00Z".into()),
    ]);
    let post5 = |port: i64| format!("(APIServer pid=1) INFO:     127.0.0.1:{port} - \"POST /v1/chat/completions HTTP/1.1\" 200 OK");
    let stamp5 = |hms: &str| format!("(APIServer pid=1) INFO 01-01 {hms} [loggers.py:259] Engine 000: Avg prompt throughput: 1.0 tokens/s");
    let raw5 = sd5.join(format!("lite_raw_{cell}.json"));
    let r = (|| -> HR<(J, J)> {
        wt(&journal, &dumps(&session_doc))?;
        wt(&raw5, &dumps(&jd(&[("config_name", cell.as_str().into()), ("measured_utc", "2026-01-01T18:59:41Z".into())])))?;
        wt(
            &hd5,
            &format!(
                "{}\n",
                [
                    "INFO 01-01 18:30:00 [api_server.py] Starting vLLM server on http://0.0.0.0:8000".to_string(),
                    post5(40001),
                    stamp5("18:31:00"),
                    post5(40002),
                    post5(40003),
                    post5(40004),
                    stamp5("18:40:10"),
                ]
                .join("\n")
            ),
        )?;
        wt(&tl5, &format!("{}\n", [post5(40004), stamp5("18:40:10"), post5(40009), stamp5("18:59:50")].join("\n")))?;
        let rs5 = steps_by(&repro(&repo, &ev, &docker_old)?);
        wt(&hd5, &format!("{}\n", [post5(40001), stamp5("18:31:00")].join("\n")))?;
        wt(&tl5, &format!("{}\n", [post5(40009), stamp5("18:59:50")].join("\n")))?;
        let rs5n = steps_by(&repro(&repo, &ev, &docker_old)?);
        Ok((rs5, rs5n))
    })();
    for f_ in [&hd5, &tl5, &raw5, &journal] {
        rm_ok(f_);
    }
    let (rs5, rs5n) = r?;
    let pr5 = if rs5["bench"]["observed"]["pre_final_sweep_requests"].truthy() { rs5["bench"]["observed"]["pre_final_sweep_requests"].clone() } else { J::dict() };
    st.ck(
        "★F5 최종 스윕 첫 측정 전 요청 4건(스모크 1 + 앞선 3 · 머리·꼬리 겹친 줄 1번 · 첫 측정 뒤 요청 ✗) → bench·serve 소요 표지 · 값 불변",
        pr5["count"].as_i64() == Some(4)
            && eqs(&pr5["first_ub"], "2026-01-01T18:31:00Z")
            && eqs(&pr5["last_ub"], "2026-01-01T18:40:10Z")
            && rs5["bench"]["observed"]["seconds"].as_i64() == Some(1819)
            && has(&rs5["bench"]["duration"], "(최종 스윕 기준 · 그 전 요청 4건 — 소요 출처 칸)")
            && has(&rs5["bench"]["source"], "최종 스윕 기준 — 관측된 serve 세션 경계 안 첫 측정 전 completions 요청 4건 관측")
            && has(&rs5["serve"]["duration"], "(끝 = 최종 스윕 첫 측정 · 그 전 요청 4건 — 소요 출처 칸)"),
    );
    st.ck(
        "★F5 음성대조: 첫 측정 전 요청이 스모크 추론 1회뿐이면 표지 없음",
        rs5n["bench"]["observed"].get("pre_final_sweep_requests").is_none() && !has(&rs5n["bench"]["source"], "최종(재실행)"),
    );
    // FC1
    let prior = journal.with_file_name(format!("serve-{cell}-20251222T183000Z.json"));
    let restart = journal.with_file_name("operator_attempt-restart.json");
    let custom = journal.with_file_name("operator_attempt-01.json");
    let old_events = rb(&evp)?;
    let r = (|st: &mut St| -> HR<()> {
        wt(
            &prior,
            &dumps(&with(&session_doc, vec![("session_id", jstem(&prior).into()), ("started_utc", "2025-12-22T18:30:00Z".into()), ("stopped_utc", J::Null)])),
        )?;
        wt(&journal, &dumps(&with(&session_doc, vec![("started_utc", "2026-01-01T18:50:00Z".into())])))?;
        wt(&raw5, &dumps(&jd(&[("config_name", cell.as_str().into()), ("measured_utc", "2026-01-01T18:59:41Z".into())])))?;
        wt(
            &hd5,
            &format!(
                "{}\n",
                [
                    "INFO 12-22 18:30:00 Starting vLLM server on fixture".to_string(),
                    post5(40001),
                    post5(40002),
                    "INFO 12-22 18:31:00 old metrics".to_string(),
                    post5(40003),
                    "INFO 01-01 18:50:20 Starting vLLM server on fixture".to_string(),
                    post5(40011),
                    post5(40012),
                    stamp5("18:50:40"),
                ]
                .join("\n")
            ),
        )?;
        wt(&tl5, &format!("{}\n", [post5(40003), stamp5("18:50:20"), post5(40011), post5(40012), stamp5("18:50:40")].join("\n")))?;
        let before: Vec<(PathBuf, Vec<u8>)> = [&hd5, &tl5, &journal, &prior].iter().map(|p_| (p_.to_path_buf(), rb(p_).unwrap_or_default())).collect();
        let scoped = steps_by(&repro(&repo, &ev, &docker_old)?);
        st.ck(
            "★FC1 current session bounds old head/tail epochs and untimed leading requests; final sweep unchanged",
            eqs(&scoped["serve"]["observed"]["start_utc"], "2026-01-01T18:50:00Z")
                && scoped["bench"]["observed"]["seconds"].as_i64() == Some(1819)
                && scoped["bench"]["observed"]["pre_final_sweep_requests"]["count"].as_i64() == Some(2)
                && before.iter().all(|(p_, raw)| rb(p_).ok().as_ref() == Some(raw)),
        );
        st.ck(
            "★FC6 full certificate requires explicit authority and PASS in the reproduction contract",
            has(&scoped["bench"]["success"], "explicit ∧ PASS") && !has(&scoped["bench"]["success"], "+PASS 면 인증서"),
        );
        let custom_doc = with(&session_doc, vec![("session_id", jstem(&custom).into()), ("started_utc", "2026-01-01T18:50:00Z".into())]);
        rm(&journal)?;
        wt(&custom, &dumps(&custom_doc))?;
        let custom_before = rb(&custom)?;
        let custom_steps = steps_by(&repro(&repo, &ev, &docker_old)?);
        let custom_window = current_serve_window(&repo, &ev, None, None)?;
        st.ck(
            "custom official session ID binds by config/topology, preserving measured duration and request scope",
            custom_window.as_ref().is_some_and(|w| w.truthy() && eqs(&w["session_id"], &jstem(&custom)))
                && ["start_utc", "end_utc", "seconds", "bound"].iter().all(|k| jeq(&custom_steps["serve"]["observed"][*k], &scoped["serve"]["observed"][*k]))
                && custom_steps["bench"]["observed"]["seconds"].as_i64() == Some(1819)
                && custom_steps["bench"]["observed"]["pre_final_sweep_requests"]["count"].as_i64() == Some(2)
                && eqs(&custom_steps["bench"]["observed"]["pre_final_sweep_requests"]["session_id"], &jstem(&custom))
                && rb(&custom)? == custom_before,
        );
        wt(&custom, &dumps(&with(&custom_doc, vec![("session_id", "unrelated_filename".into())])))?;
        st.ck("custom session ID must match its official journal filename", current_serve_window(&repo, &ev, None, None)?.is_none());
        rm(&custom)?;
        wt(&journal, &dumps(&with(&session_doc, vec![("started_utc", "2026-01-01T18:50:00Z".into())])))?;
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&hd5).map_err(|e| ioe(e, &hd5))?;
            let add = format!(
                "{}\n",
                ["INFO 01-01 18:52:00 Starting vLLM server on fixture".to_string(), post5(40021), post5(40022), stamp5("18:53:00")].join("\n")
            );
            f.write_all(add.as_bytes()).map_err(|e| ioe(e, &hd5))?;
        }
        let epoch = steps_by(&repro(&repo, &ev, &docker_old)?);
        let ep = if epoch["bench"]["observed"]["pre_final_sweep_requests"].truthy() { epoch["bench"]["observed"]["pre_final_sweep_requests"].clone() } else { J::dict() };
        st.ck(
            "★FC1 later engine epoch inside a session excludes earlier-epoch requests across both captures",
            ep["count"].as_i64() == Some(2) && eqs(&ep["epoch_start_utc"], "2026-01-01T18:52:00Z") && eqs(&ep["first_ub"], "2026-01-01T18:53:00Z"),
        );
        wt(
            &restart,
            &dumps(&with(&session_doc, vec![("session_id", jstem(&restart).into()), ("started_utc", "2026-01-01T19:10:00Z".into())])),
        )?;
        st.ck("★FC1 restart during the measured sweep refuses current-session binding", current_serve_window(&repo, &ev, None, None)?.is_none());
        rm(&restart)?;
        for (label, changes) in [
            ("open-only", vec![("stopped_utc", J::Null)]),
            ("stopped before sweep end", vec![("stopped_utc", J::from("2026-01-01T19:20:00Z"))]),
            ("other config", vec![("config_file", J::from(format!("{cell}-other")))]),
            ("other topology", vec![("topology", J::from("single"))]),
        ] {
            let mut kv = vec![("started_utc", J::from("2026-01-01T18:50:00Z"))];
            kv.extend(changes);
            wt(&journal, &dumps(&with(&session_doc, kv)))?;
            st.ck(&format!("★FC1 {label} cannot supply the current serve window"), current_serve_window(&repo, &ev, None, None)?.is_none());
        }
        wt(&journal, &dumps(&session_doc))?;
        let alias = repo.join("docs/logs/alias/sessions");
        std::fs::create_dir(alias.parent().unwrap()).map_err(|e| ioe(e, &alias))?;
        std::os::unix::fs::symlink(journal.parent().unwrap(), &alias).map_err(|e| ioe(e, &alias))?;
        let alias_ev = with(&ev, vec![("declaration", p(r#"{"nodes": [{"node_id": "alias", "role": "main"}]}"#))]);
        let w = current_serve_window(&repo, &alias_ev, None, None);
        let _ = std::fs::remove_file(&alias);
        let _ = std::fs::remove_dir(alias.parent().unwrap());
        st.ck("★FC1 session journal ancestor symlink cannot redirect node identity", w?.is_none());
        rm(&journal)?;
        wt(
            &evp,
            &format!(
                "{}{}",
                jline(&jd(&[("kind", "budget_declare".into()), ("label", format!("serve-{cell}").into()), ("ts", "2025-12-22T18:30:00Z".into())])),
                jline(&p(r#"{"kind": "budget_clear", "ts": "2025-12-22T19:00:00Z"}"#)),
            ),
        )?;
        let unknown = steps_by(&repro(&repo, &ev, &docker_old)?);
        st.ck(
            "★FC1 absent current boundary is scoped unobserved, not historical start/count or changed bench time",
            unknown["serve"]["observed"].get("start_utc").is_some_and(J::is_null)
                && unknown["bench"]["observed"]["seconds"].as_i64() == Some(1819)
                && unknown["bench"]["observed"].get("pre_final_sweep_requests").is_none(),
        );
        let s0 = jd(&[("kind", "budget_declare".into()), ("label", format!("serve-{cell}").into()), ("ts", "2025-12-22T18:30:00Z".into())]);
        let stale_open = vec![s0.clone(), p(r#"{"kind": "sample", "ts": "2026-01-01T20:00:00Z"}"#)];
        let late = vec![with(&s0, vec![("ttl_s", 1800.into())]), p(r#"{"kind": "budget_clear", "ts": "2026-01-01T20:00:00Z"}"#)];
        for (label, rows) in [("stale open budget", stale_open), ("late clear after known TTL", late)] {
            wt(&evp, &rows.iter().map(jline).collect::<String>())?;
            let stale = steps_by(&repro(&repo, &ev, &docker_old)?);
            st.ck(
                &format!("★FC1 {label} cannot reintroduce an old serve start through the budget fallback"),
                stale["serve"]["observed"].get("start_utc").is_some_and(J::is_null) && stale["bench"]["observed"].get("pre_final_sweep_requests").is_none(),
            );
        }
        Ok(())
    })(st);
    wb(&evp, &old_events)?;
    for p_ in [&hd5, &tl5, &journal, &prior, &restart, &custom, &raw5] {
        rm_ok(p_);
    }
    r?;
    Ok(())
}

// ── 수집(원장 부재 이미지) ──
fn sec_collect(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let (ip, mnt, operator_root) = (fx.ip.clone(), fx.mnt.clone(), fx.operator_root.clone());
    let payload = fx.payload.clone();
    let res = coll(&repo, &ev, &payload, Some(&docker_old))?;
    let env_path = out.join("envs").join(format!(".env.{cell}"));
    let env_original = rb(&env_path)?;
    let env_mtime = mtime_of(&env_path)?;
    let r = (|st: &mut St| -> HR<()> {
        let mut env_local = env_original.clone();
        env_local.extend_from_slice(format!("QUANT_MODEL_PATH={operator_root}\n").as_bytes());
        wb(&env_path, &env_local)?;
        let local_payload = repo.join("payload_local_model_root");
        let local_result = coll(&repo, &ev, &local_payload, Some(&docker_old))?;
        let exported = rt(&local_payload.join("artifacts/triplet").join(format!(".env.{cell}")))?;
        st.ck(
            "Docker local model-root export passes PII without changing measured source",
            !exported.contains(&operator_root)
                && exported.contains("QUANT_MODEL_PATH=<manifest.quant_model_path>")
                && rb(&env_path)? == env_local
                && has(&local_result["slots"]["triplet"]["evidence"]["substituted"], "original env unchanged"),
        );
        let mut same = true;
        for cpath in [out.join("configs").join(format!("{cell}.yaml")), out.join("configs").join(format!("{cell}.sh"))] {
            let nm = cpath.file_name().unwrap().to_string_lossy().to_string();
            same &= rb(&local_payload.join("artifacts/triplet").join(nm)).ok() == Some(rb(&cpath)?);
        }
        st.ck("Docker model-root shaping does not rewrite container model YAML or launch script", same);
        let mut e2 = env_local.clone();
        e2.extend_from_slice(format!("OTHER_ROOT={operator_root}\n").as_bytes());
        wb(&env_path, &e2)?;
        st.raises("Unknown Docker triplet operator data still fails closed", "HINT_ARTIFACT_PII", || {
            coll(&repo, &ev, &repo.join("payload_unknown_operator_root"), Some(&docker_old))
        });
        Ok(())
    })(st);
    wb(&env_path, &env_original)?;
    restore_mtime(&env_path, env_mtime)?;
    r?;
    let names = sset(&res["files"]);
    let n_has = |x: &str| names.contains(x);
    st.ck("쓰인 source Dockerfile 포함", n_has("artifacts/build_recipe/Dockerfile.source-build"));
    st.ck("★쓰이지 않은 wheel Dockerfile 제외", !n_has("artifacts/build_recipe/Dockerfile"));
    st.ck(
        "source-build 가 COPY 하는 requirements(제약 천장) 포함",
        n_has("artifacts/build_recipe/requirements.txt") && sv(&res["slots"]["build_recipe"]["evidence"]["note"]).contains("constraint"),
    );
    st.ck("★skip 패치 제외", !names.iter().any(|p| p.contains("50-port") || p.contains("55-deps")));
    st.ck("미관측 패치는 포함", n_has("artifacts/build_patch_pre/60-ungated.sh") && n_has("artifacts/build_patch_post/10-shared.sh"));
    st.ck(
        "★known non-patch 미포함",
        !names.iter().any(|p| p.ends_with("PROVENANCE.json") || p.ends_with(".gitkeep") || p.ends_with("stray.txt")),
    );
    st.ck("serve_runner·arm_patch(서빙 경로 파생) 포함", n_has("artifacts/compose/serve_runner.sh") && n_has("artifacts/compose/arm_patch.sh"));
    st.ck("★debug-init(디버그 프로필 전용) 제외", !n_has("artifacts/compose/debug-init.sh"));
    let ct = _command_texts("    command:\n      - bash\n      - /app/configs/serve_runner.sh\n    env_file:\n      - envs/.env.cluster\n")?;
    st.ck(
        "★블록 꼴 command 도 러너 참조를 읽는다(한 줄 꼴만 읽으면 러너가 조용히 빠진다)",
        _CONFIGS_REF().findall(&ct.join(" ")) == vec!["serve_runner.sh".to_string()],
    );
    st.ck(
        "env 형상 3종 + sub_recipe",
        ["artifacts/compose/.env.cluster.template", "artifacts/compose/.env.interconnect.template", "artifacts/compose/.env.template", "artifacts/compose/sub_recipe.json"]
            .iter()
            .all(|x| n_has(x)),
    );
    let cl = rt(&payload.join("artifacts/compose/.env.cluster.template"))?;
    let ic = rt(&payload.join("artifacts/compose/.env.interconnect.template"))?;
    let pj = rt(&payload.join("artifacts/compose/.env.template"))?;
    st.ck(
        "★cluster 형상: 호스트 실값 제거 · 키 전부 유지",
        !cl.contains(&ip)
            && !cl.contains("node-sub-fixture")
            && !cl.contains("opfixture")
            && ["MASTER_HOST_IP", "SLAVE_HOST_IP", "SSH_USER", "MAX_JOBS", "RAY_PORT"].iter().all(|k| cl.contains(&format!("{k}="))),
    );
    st.ck("interconnect 형상: ①env 치환 · ③invariant 유지", !ic.contains("enpfixture0") && ic.contains("NCCL_DEBUG=INFO"));
    st.ck("★project .env: 운영자 경로 제거(키-축)", !pj.contains(&format!("{mnt}/")) && pj.contains("<manifest.nas_model_path>"));
    let sr = crate::value::loads(&rt(&payload.join("artifacts/compose/sub_recipe.json"))?).map_err(|e| HintError::py("JSONDecodeError", e))?;
    st.ck(
        "★sub_recipe 에 mount 실값 없음",
        !dumps(&sr).contains(&format!("{mnt}/")) && eqs(&sr["per_node_values"]["NAS_MODEL_PATH"], "<manifest.nas_model_path>"),
    );
    let fwd: BTreeSet<String> = sr["env_forward"].items().iter().map(|r| r["key"].display()).collect();
    st.ck(
        "sub_recipe 전달 목록 = slave_forward(스모크와 같은 함수)",
        fwd == set_of(&["IMAGE_TAG", "BUILD_DOCKERFILE", "VLLM_REF", "BUILD_JOBS", "NAS_MODEL_PATH", "SLAVE_CONTAINER_NAME", "VLLM_PLE_MMAP", "VLLM_PLE_MMAP_DIR"]),
    );
    st.ck(
        "K11 관측: slave CONFIG_FILE=default → 런타임 패치 미무장",
        eqs(&sr["role_diff"]["slave_config_file"], "default")
            && sw(&sr["role_diff"]["slave_runtime_patch"], "not-armed")
            && sw(&sr["role_diff"]["master_runtime_patch"], "armed"),
    );
    st.ck("serve_runner GPU 리터럴 \"2\"", eqs(&sr["role_diff"]["runner_gpu_join_literal"]["value"], "2"));
    st.ck(
        "per_node_values: ①env 표지 · interconnect 템플릿",
        sv(&sr["per_node_values"]["MASTER_HOST_IP"]).starts_with('<')
            && eqs(&sr["per_node_values"]["interconnect"], "artifacts/compose/.env.interconnect.template"),
    );
    st.ck("parity = attestation 부재 → unobservable", eqs(&sr["parity_attestation"]["observed"], "unobservable"));
    st.ck("결손: 원장 부재 기재", has(&res["missing"], "HINT_MISSING_BUILD_LEDGER"));
    st.ck(
        "슬롯 rationale 은 기계가 채우지 않는다(선언 = 저작자 몫)",
        res["slots"].as_dict().is_some_and(|m| m.values().all(|r| r["rationale"].is_null())),
    );
    st.ck(
        "★원장 부재 패치 슬롯 = 1-signal(적용 미관측은 증거가 아니다)",
        eqs(&res["slots"]["build_patch_pre"]["confidence"], "1-signal")
            && eqs(&res["slots"]["build_patch_pre"]["evidence"]["kind"], "none")
            && eqs(&res["slots"]["triplet"]["confidence"], "2-signal"),
    );
    st.ck(
        "★skip 사유가 슬롯 excluded 에 남는다",
        res["slots"]["build_patch_pre"]["excluded"].items().iter().any(|x| eqs(&x["file"], "50-port.sh") && sw(&x["why"], "skipped")),
    );
    let vs = by(&res["value_status_candidates"], "knob");
    st.ck("값의 지위: lockset kv_source=measured-clamp → tuned", eqs(&vs["kv-cache-memory-bytes"]["candidate_status"], "tuned"));
    st.ck("값의 지위: batch_source=hand-lever → inherited 후보", eqs(&vs["max-num-seqs"]["candidate_status"], "inherited"));
    st.ck(
        "값의 지위: 주석 '승계' · auto=engine-default",
        eqs(&vs["enforce-eager"]["candidate_status"], "inherited") && eqs(&vs["kv-cache-dtype"]["candidate_status"], "engine-default"),
    );
    st.ck("★근거 없으면 후보를 지어내지 않는다", vs["max-model-len"].get("candidate_status").is_some_and(J::is_null));
    st.ck(
        "★주석 단어 '필수' 만 = declared-requirement ✗ → inherited + 문구(note · reason)",
        eqs(&vs["async-scheduling"]["candidate_status"], "inherited")
            && eqs(&vs["async-scheduling"]["note"], COMMENT_ONLY_REQUIREMENT_NOTE)
            && has(&vs["async-scheduling"]["reason"], COMMENT_ONLY_REQUIREMENT_NOTE)
            && eqs(&vs["gpu-memory-utilization"]["candidate_status"], "inherited"),
    );
    let vs_l = by(&vsc(&repo, &with(&ev, vec![("lockset", with(&ev["lockset"], vec![("gmu_source", "target_gmu".into())]))]))?, "knob");
    st.ck(
        "★lockset 출처(target_gmu · 기록된 요구)는 declared-requirement 유지 — 주석 규칙이 덮지 않는다 · 문구 없음",
        eqs(&vs_l["gpu-memory-utilization"]["candidate_status"], "declared-requirement") && vs_l["gpu-memory-utilization"].get("note").is_none(),
    );
    st.ck(
        "★주석 규칙은 다른 단어 후보를 건드리지 않는다(승계 = inherited · 문구 없음 · 음성대조 = negative-control)",
        vs["enforce-eager"].get("note").is_none() && eqs(&vs["enforce-eager"]["candidate_status"], "inherited"),
    );
    st.ck(
        "COMMENT_ONLY_REQUIREMENT_NOTE 문구(공유 계약 · 글자 그대로)",
        COMMENT_ONLY_REQUIREMENT_NOTE == "yaml 주석만 있음 — 관측된 실패가 있으면 declared-requirement",
    );
    st.ck(
        "★중첩 yaml 은 부모 노브 하나 · 따옴표 안 # 는 값",
        vs.get("method").is_none() && eqs(&vs["url-ish"]["value"], "a#b") && has(&vs["speculative-config"]["value"], "num_speculative_tokens"),
    );
    let vk = keys(&vs);
    st.ck(
        "D-b 전송·신원 노브(model)는 value-status 후보에서 빠진다 · 튜닝 노브는 전부 남는다",
        ["model", "host", "port", "served_model_name", "served-model-name"].iter().all(|k| !vk.contains(*k))
            && ["max-model-len", "kv-cache-memory-bytes", "enforce-eager", "speculative-config"].iter().all(|k| vk.contains(*k)),
    );
    st.ck(
        "D-b 제외 목록은 닫힌 4종(tripwire)",
        VALUE_STATUS_EXCLUDED_KNOBS.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>() == set_of(&["model", "host", "port", "served-model-name"]),
    );
    let ypath = out.join("configs").join(format!("{cell}.yaml"));
    let y0 = rt(&ypath)?;
    wt(&ypath, &format!("{y0}max-num-batched-tokens: 8192   # 측정으로 고른 값\n"))?;
    let vs15 = vsc(&repo, &ev);
    wt(&ypath, &y0)?;
    let vs15 = by(&vs15?, "knob");
    st.ck(
        "★F15 주석 단어 '측정' 만 = tuned ✗ → inherited + 스윕 기록 없음 문구",
        eqs(&vs15["max-num-batched-tokens"]["candidate_status"], "inherited") && eqs(&vs15["max-num-batched-tokens"]["note"], COMMENT_ONLY_TUNED_NOTE),
    );
    st.ck("F15 음성대조: lockset kv_source=measured-clamp(스윕 기록)은 tuned 유지", eqs(&vs15["kv-cache-memory-bytes"]["candidate_status"], "tuned"));
    fx.res = res;
    Ok(())
}

/// `next(x[field] for x in rows if x[key] == val)` (Null when absent — Python would raise StopIteration).
fn find_in(rows: &J, key: &str, val: &str) -> J {
    rows.items().iter().find(|x| eqs(&x[key], val)).cloned().unwrap_or(J::Null)
}
/// `{(r["phase"], r["file"]): r}` as `"phase\u{1f}file"` keys.
fn by_pf(rows: &J) -> J {
    let mut m = Map::new();
    for r in rows.items() {
        m.insert(format!("{}\u{1f}{}", r["phase"].display(), r["file"].display()), r.clone());
    }
    J::Dict(m)
}

// ── 원장 관측(② 메인 로컬 이미지) ──
fn sec_ledger(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone());
    let sha60 = core::sha256_file(&out.join("build_patches_src/60-ungated.sh"))?;
    let ledger = p(&format!(
        r#"{{"schema_version": 1, "dockerfile": "Dockerfile.source-build", "track": "source-build",
            "patches": [{{"phase": "pre", "file": "50-port.sh", "result": "skipped", "reason": "SM12X_PORT=0",
                          "result_source": "log-token"}},
                        {{"phase": "pre", "file": "60-ungated.sh", "result": "applied", "result_source": "exit-code",
                          "script_sha256": "{sha60}"}},
                        {{"phase": "post", "file": "10-shared.sh", "result": "applied", "result_source": "status-file"}}],
            "inline_patches": [{{"name": "strip-hoist", "result": "skipped", "reason": "accepts hoist"}}]}}"#
    ));
    let led_txt = dumps(&ledger);
    let id_pat: &[&str] = &["image", "inspect", "--format", "{{.Id}}"];
    let run_pat: &[&str] = &["run", "--rm", "--network", "none", "--pull", "never"];
    let docker_new = _fake_docker(&[(id_pat, (0, "sha256:fx\n")), (run_pat, (0, &led_txt))]);
    let apn = aset(&repo, &ev, &docker_new)?;
    let byn = by_pf(&apn["patches"]);
    st.ck(
        "원장 관측 = observed · 메인 로컬 출처",
        eqs(&apn["status"], "observed") && sw(&apn["source"], "docker-image(main-local)"),
    );
    st.ck("inline 패치 행", eqs(&byn["inline\u{1f}strip-hoist"]["result"], "skipped"));
    let led_v = with(
        &ledger,
        vec![
            ("provenance", "measured(build-time, in-image)".into()),
            (
                "vllm",
                p(&format!(r#"{{"dist_version": "0.1.1.dev0+gfx", "git_sha": "{}", "describe": "v0.1.0", "direct_url": {{"x": 1}}}}"#, "e".repeat(40))),
            ),
        ],
    );
    let lv_txt = dumps(&led_v);
    let cv_ = ctx(&repo, &ev, Some(&_fake_docker(&[(id_pat, (0, "sha256:fx\n")), (run_pat, (0, &lv_txt))])))?;
    _applied(&cv_, true)?;
    let lv_ = _ledger_vllm_view(&cv_)?;
    let cn_ = ctx(&repo, &ev, Some(&docker_new))?;
    _applied(&cn_, true)?;
    st.ck(
        "F7 대표 원장 vllm 블록 뷰 = 적용 집합과 같은 원장(dist_version · git_sha · describe 만) · 출처 로컬 이미지 · 원장에 vllm 없으면 None",
        lv_.as_ref().is_some_and(|lv| {
            jeq(&lv["vllm"], &p(&format!(r#"{{"dist_version": "0.1.1.dev0+gfx", "git_sha": "{}", "describe": "v0.1.0"}}"#, "e".repeat(40))))
                && sw(&lv["source"], "docker-image(main-local)")
                && sw(&lv["provenance"], "measured")
        }) && _ledger_vllm_view(&cn_)?.is_none(),
    );
    st.ck(
        "★원장에 없는 파일(55) = not-in-ledger 로 싣지 않음",
        apn["excluded"].items().iter().any(|x| eqs(&x["file"], "55-deps.sh") && sw(&x["why"], "not-in-ledger")),
    );
    let led_ci = with(&ledger, vec![("context_inventory", p(r#"{"build_patch_pre": [".gitkeep"], "build_patch_post": [".gitkeep"]}"#))]);
    let p55 = out.join("build_patches_src/55-deps.sh");
    let st55 = mtime_of(&p55)?;
    _set_mtime(&p55, "2026-01-01T00:00:00Z")?;
    let r = (|| -> HR<(J, J)> {
        let ci_txt = dumps(&led_ci);
        let dk_ci = _fake_docker(&[
            (id_pat, (0, "sha256:fx\n")),
            (&["image", "inspect", "--format", "{{.Created}}"], (0, "2026-10-14T01:45:19Z\n")),
            (run_pat, (0, &ci_txt)),
        ]);
        let w_ci = find_in(&aset(&repo, &ev, &dk_ci)?["excluded"], "file", "55-deps.sh")["why"].clone();
        let dk_old = _fake_docker(&[
            (id_pat, (0, "sha256:fx\n")),
            (&["image", "inspect", "--format", "{{.Created}}"], (0, "2025-12-01T00:00:00Z\n")),
            (run_pat, (0, &led_txt)),
        ]);
        let w_old = find_in(&aset(&repo, &ev, &dk_old)?["excluded"], "file", "55-deps.sh")["why"].clone();
        Ok((w_ci, w_old))
    })();
    restore_mtime(&p55, st55)?;
    let (w_ci, w_old) = r?;
    st.ck(
        "★F5 원장 context_inventory 에 없음 = 빌드 컨텍스트에 들지 않았다 · mtime ≤ Created = 빌드 뒤 추가 아님(단정 ✗)",
        sw(&w_ci, "not-in-ledger(")
            && has(&w_ci, "context_inventory.build_patch_pre = ['.gitkeep']")
            && has(&w_ci, "빌드 컨텍스트에 들지 않았다")
            && has(&w_ci, "빌드 뒤 추가 아님")
            && !has(&w_ci, "로 본다"),
    );
    st.ck(
        "F5 옛 원장(context_inventory 없음) · mtime > Created = 관측대로 빌드 뒤 추가·수정 · Created 미관측이면 미관측이라 적는다",
        has(&w_old, "옛 원장")
            && has(&w_old, "빌드 뒤 추가·수정")
            && has(&find_in(&apn["excluded"], "file", "55-deps.sh")["why"], "빌드 뒤 추가 여부 미관측"),
    );
    st.ck(
        "★서브 셀은 메인 로컬 이미지를 보지 않는다",
        sw(&aset(&repo, &with(&ev, vec![("node", "sub".into())]), &docker_new)?["probes"][1]["result"], "skipped(node=sub"),
    );
    let bad_ledger = with(&ledger, vec![("dockerfile", "Dockerfile".into())]);
    let bl_txt = dumps(&bad_ledger);
    st.raises("★원장·셀 env 선택자 모순", "HINT_BUILD_SELECTOR_MISMATCH", || {
        aset(&repo, &ev, &_fake_docker(&[(&["image", "inspect"], (0, "x")), (&["run"], (0, &bl_txt))]))
    });
    // ① attestation 원장이 로컬보다 먼저 · 노드 인식(main)
    let ev_att = with(
        &ev,
        vec![("attestation", jd(&[("schema_version", 2.into()), ("nodes", jd(&[("sub", p(r#"{"build_ledger": {"patches": []}}"#)), ("main", jd(&[("build_ledger", ledger.clone())]))]))]))],
    );
    let apa = aset(&repo, &ev_att, &_fake_docker(&[]))?;
    let host_like = format!("spark-{}", "a1b2c3");
    let mut att2 = ev_att["attestation"].clone();
    att2.set("parity_blocking", false);
    att2.set(
        "nodes",
        jd(&[
            ("main", jd(&[("node_id", host_like.as_str().into()), ("build_ledger", ledger.clone())])),
            ("sub", jd(&[("node_id", host_like.as_str().into()), ("build_ledger", ledger.clone())])),
        ]),
    );
    att2.set(
        "parity",
        p(&format!(r#"{{"vllm_sha": {{"verdict": "equal", "main": "{a}", "sub": "{a}"}}, "build_ledger": {{"verdict": "equal"}}}}"#, a = "a".repeat(40))),
    );
    let ev_att2 = with(&ev_att, vec![("attestation", att2)]);
    let sr2 = subr(&repo, &ev_att2)?;
    st.ck(
        "v2 parity 축별 판정 · ★노드 id 비유출",
        eqs(&sr2["parity_attestation"]["observed"]["vllm_sha"]["verdict"], "equal") && !dumps(&sr2).contains(&host_like),
    );
    st.ck(
        "① attestation 원장 우선 · main 노드 선택",
        eqs(&apa["status"], "observed") && ew(&apa["source"], "#main") && apa.get("per_node_patchsets").is_some(),
    );
    let asa = if apa["node_asymmetry"].truthy() { apa["node_asymmetry"].clone() } else { J::dict() };
    st.ck(
        "★F2 cluster 원장 비대칭 = node_asymmetry(대표 main · 노드별 행 그대로 · note) · 합집합 ✗",
        eqs(&asa["representative"], "main")
            && asa["nodes"]["main"]["patches"].items().len() == 4
            && asa["nodes"]["sub"]["patches"].as_list().is_some_and(|l| l.is_empty())
            && ew(&asa["nodes"]["sub"]["source"], "#sub")
            && eqs(&asa["note"], NODE_ASYMMETRY_NOTE)
            && apa["patches"].items().len() == 4,
    );
    let main_l = p(r#"{"schema_version": 1, "dockerfile": "Dockerfile.source-build", "track": "source-build", "patches": [],
                      "inline_patches": [{"name": "strip-hoist", "result": "skipped", "reason": "accepts hoist"}]}"#);
    let ev_r = with(
        &ev,
        vec![("attestation", jd(&[("schema_version", 2.into()), ("nodes", jd(&[("main", jd(&[("build_ledger", main_l)])), ("sub", jd(&[("build_ledger", ledger.clone())]))]))]))],
    );
    let apr = aset(&repo, &ev_r, &_fake_docker(&[]))?;
    let asr = if apr["node_asymmetry"].truthy() { apr["node_asymmetry"].clone() } else { J::dict() };
    let ex10 = sv(&find_in(&apr["excluded"], "file", "10-shared.sh")["why"]);
    let subrows: BTreeSet<(String, String, String)> = asr["nodes"]["sub"]["patches"]
        .items()
        .iter()
        .map(|r| (r["phase"].display(), r["file"].display(), r["result"].display()))
        .collect();
    st.ck(
        "★F2 회귀 모양(메인 빈 슬롯 · 서브 post applied/pre skipped): 서브 원장 행이 사라지지 않는다 · 대표 밖 파일 사유 = 노드 비대칭(빌드 뒤 추가 ✗)",
        ew(&apr["source"], "#main")
            && subrows.contains(&("post".into(), "10-shared.sh".into(), "applied".into()))
            && subrows.contains(&("pre".into(), "50-port.sh".into(), "skipped".into()))
            && ex10.contains("대표 main 원장에 없음 · sub 원장 applied")
            && !ex10.contains("빌드 뒤 추가"),
    );
    let ptxt = node_asymmetry_phase_text(&asr, "post")?.unwrap_or_default();
    st.ck(
        "★F2 단계별 노드 비대칭 문장 = 노드마다 각각(대표 표지 · 행 없음 명시)",
        ptxt.contains("main(대표) 원장: 이 단계 행 없음")
            && ptxt.contains("sub 원장: applied 10-shared.sh")
            && node_asymmetry_phase_text(&asr, "pre")?.unwrap_or_default().contains("skipped 50-port.sh"),
    );
    let mut sl_r = p(r#"{"build_patch_post": {"files": [], "evidence": {"kind": "build-ledger", "note": "원장 관측"}}}"#);
    let cr = ctx(&repo, &ev_r, Some(&_fake_docker(&[])))?;
    _slot_machine_reasons(&cr, &mut sl_r, &apr, &J::Null, "docker", &cr)?;
    st.ck(
        "★F2 슬롯 사유 = 대표 원장 사유 + 노드별 원장(실림·판정 기준 = 대표)",
        sw(&sl_r["build_patch_post"]["machine_reason"], "적용 판정 없음 — 원장 관측 · 노드 비대칭(cluster)")
            && has(&sl_r["build_patch_post"]["machine_reason"], "sub 원장: applied 10-shared.sh"),
    );
    let ev_eq = with(
        &ev,
        vec![("attestation", jd(&[("schema_version", 2.into()), ("nodes", jd(&[("main", jd(&[("build_ledger", ledger.clone())])), ("sub", jd(&[("build_ledger", ledger.clone())]))]))]))],
    );
    let ev_rm = with(&ev_r, vec![("node", "main".into())]);
    st.ck(
        "★F2 음성대조: 원장이 같으면 · cluster 가 아니면 비대칭 표지 없음",
        aset(&repo, &ev_eq, &_fake_docker(&[]))?.get("node_asymmetry").is_none()
            && aset(&repo, &ev_rm, &_fake_docker(&[]))?.get("node_asymmetry").is_none()
            && sv(&find_in(&aset(&repo, &ev_rm, &_fake_docker(&[]))?["excluded"], "file", "10-shared.sh")["why"]).contains("빌드 뒤 추가"),
    );
    let ev_att3 = with(
        &ev,
        vec![(
            "attestation",
            jd(&[
                ("schema_version", 2.into()),
                ("_source", "output/multi/benchlog/attestation_fx-build.json (이미지 compose 라벨 vllm_fx-build_project)".into()),
                ("nodes", jd(&[(host_like.as_str(), jd(&[("build_ledger", ledger.clone())])), ("sub", jd(&[("build_ledger", ledger.clone())]))])),
            ]),
        )],
    );
    let apa3 = aset(&repo, &ev_att3, &_fake_docker(&[]))?;
    st.ck(
        "★비역할 키 무시 · sub 원장으로 main 대리 ✗ · 키 문자열 비유출",
        eqs(&apa3["status"], "unobservable")
            && apa3["probes"][0]["ignored_node_keys"].as_i64() == Some(1)
            && sw(&apa3["probes"][0]["result"], "partial")
            && !dumps(&apa3).contains(&host_like),
    );
    st.ck(
        "★attestation 출처 = evidence 가 묶은 파일(셀 이름 파일 ✗)",
        eqs(&apa3["probes"][0]["ref"], "output/multi/benchlog/attestation_fx-build.json")
            && eqs(&subr(&repo, &ev_att3)?["parity_attestation"]["source"], "output/multi/benchlog/attestation_fx-build.json"),
    );
    fx.ledger = ledger;
    Ok(())
}

/// `{k: v}` from pairs (insertion order).
fn jd(kv: &[(&str, J)]) -> J {
    let mut m = Map::new();
    for (k, v) in kv {
        m.insert(*k, v.clone());
    }
    J::Dict(m)
}

// ── 적용 집합(원장 부재 · X9 재구성) ──
fn sec_applied(st: &mut St, fx: &mut Fx) -> HR<()> {
    let (repo, out, ev, cell) = (fx.repo.clone(), fx.out.clone(), fx.ev.clone(), fx.cell.clone());
    let docker_old = fx.docker_old.clone();
    let history = fx.history.clone();
    let ap = aset(&repo, &ev, &docker_old)?;
    let byf = by(&ap["patches"], "file");
    st.ck("원장 부재 = unobservable", eqs(&ap["status"], "unobservable"));
    st.ck(
        "★자기게이트 skip 재구성(실물 꼴 따옴표 기본값)",
        eqs(&byf["50-port.sh"]["result"], "skipped") && eqs(&byf["50-port.sh"]["result_source"], "reconstructed(docker-history+gate)"),
    );
    st.ck("★뒤집기 게이트(55) 재구성 = 모든 게이트 변수 ≠ 1 일 때만 skip", eqs(&byf["55-deps.sh"]["result"], "skipped"));
    st.ck("ungated = 적용 미관측", eqs(&byf["60-ungated.sh"]["result"], "unobserved"));
    st.ck("★게이트 앞 부수효과 → skip 재구성 ✗(미관측)", eqs(&byf["58-effect-first.sh"]["result"], "unobserved"));
    st.ck(
        "★베이스 층 인자(NVIDIA_*·다른 값) 배제",
        jeq(&ap["reconstruction"]["build_args"], &p(r#"{"BUILD_JOBS": "8", "SM12X_PORT": "0", "SRC_DEPS_AUTHORITY": "0", "VLLM_REF": "v0.1.0"}"#)),
    );
    st.ck(
        "X10 post 라벨",
        is_true(&byf["10-shared.sh"]["shared_image_input"]) && has(&byf["10-shared.sh"]["label"], "model-trigger"),
    );
    let exc: BTreeSet<(String, String)> = ap["excluded"]
        .items()
        .iter()
        .map(|x| (x["file"].display(), x["why"].display().split('(').next().unwrap_or("").to_string()))
        .collect();
    let want: BTreeSet<(String, String)> = [(".gitkeep", "known-non-patch"), ("PROVENANCE.json", "known-non-patch"), ("files/", "known-non-patch")]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    st.ck("★.gitkeep·PROVENANCE.json·files/ 명시 건너뜀", want.is_subset(&exc));
    st.ck("★규약 밖 이름은 소리내어 기재", exc.contains(&("stray.txt".to_string(), "nonconforming".to_string())));
    st.ck(
        "탐침 기록(attestation 부재 → 로컬 이미지 원장 없음 → 재구성 → 이미지 탐침)",
        ap["probes"].items().iter().map(|p| p["tier"].display()).collect::<Vec<_>>() == ["attestation", "local-image", "reconstruction", "image-probe"]
            && sw(&ap["probes"][1]["result"], "no-ledger"),
    );
    st.ck(
        "★image_probe 를 선언하지 않은 주입 실행기 = 탐침 건너뜀(create/cp/rm 0 · 읽기 전용)",
        sw(&ap["probes"][3]["result"], "skipped(주입 실행기가 image_probe"),
    );
    let ap_rf = aset(&repo, &ev, &_fake_docker(&[(&["image", "inspect"], (0, "x")), (&["run"], (125, "")), (&["history"], (0, &history))]))?;
    st.ck("★docker 자신의 실패(125)는 '원장 없음' 이 아니다", eqs(&ap_rf["probes"][1]["result"], "docker-run-failed(rc=125)"));
    let ap2 = aset(&repo, &ev, &_fake_docker(&[]))?;
    st.ck("★빌드 인자 미관측 → skip 단정 ✗", ap2["patches"].items().iter().all(|r| eqs(&r["result"], "unobserved")));
    let hist1 = history.replace("SM12X_PORT=0 SRC", "SM12X_PORT=1 SRC");
    let ap3 = aset(&repo, &ev, &_fake_docker(&[(&["image", "inspect"], (0, "x")), (&["history"], (0, &hist1))]))?;
    let b3 = by(&ap3["patches"], "file");
    st.ck(
        "★게이트 변수 1 → 미관측(55 는 뒤집기로 켜질 수 있다)",
        eqs(&b3["50-port.sh"]["result"], "unobserved") && eqs(&b3["55-deps.sh"]["result"], "unobserved"),
    );
    wt(&out.join("build_patches_src/57-default-on.sh"), "#!/bin/bash\n: \"${FX_ON:=1}\"\nif [ \"${FX_ON}\" != \"1\" ]; then\n    exit 0\nfi\n")?;
    let dsb = rt(&out.join("Dockerfile.source-build"))?;
    wt(&out.join("Dockerfile.source-build"), &dsb.replace("ARG BUILD_JOBS=16\n", "ARG BUILD_JOBS=16\nARG FX_ON=1\n"))?;
    let hist_on = history.replace("|4 VLLM_REF=v0.1.0", "|5 FX_ON= VLLM_REF=v0.1.0");
    let ap_on = aset(&repo, &ev, &_fake_docker(&[(&["image", "inspect"], (0, "x")), (&["history"], (0, &hist_on))]))?;
    let bon = by(&ap_on["patches"], "file");
    st.ck(
        "★빈 build-arg + `:=1` 기본값 → skip 단정 ✗(실효값 1)",
        eqs(&bon["57-default-on.sh"]["result"], "unobserved") && eqs(&bon["50-port.sh"]["result"], "skipped"),
    );
    rm(&out.join("build_patches_src/57-default-on.sh"))?;
    let dsb = rt(&out.join("Dockerfile.source-build"))?;
    wt(&out.join("Dockerfile.source-build"), &dsb.replace("ARG FX_ON=1\n", ""))?;
    let dg = format!("sha256:{}", "d".repeat(64));
    let (dg2, h2) = (dg.clone(), history.clone());
    let docker_tag_only: Runner = Rc::new(move |argv: &[String], _t: u64| -> Result<Proc, RunFail> {
        if argv.last().map(String::as_str) == Some(dg2.as_str()) || argv.contains(&dg2) {
            return Ok(proc_(1, "", "No such image"));
        }
        if argv.get(1).map(String::as_str) == Some("history") {
            return Ok(proc_(0, &h2, ""));
        }
        if argv.len() >= 3 && argv[1] == "image" && argv[2] == "inspect" {
            return Ok(proc_(0, "sha256:other\n", ""));
        }
        Ok(proc_(1, "", ""))
    });
    let ev_dg = with(&ev, vec![("build_identity", jd(&[("image_tag", "easy-vllm:fx-source".into()), ("image_digest", dg.as_str().into())]))]);
    let ap_dg = aset(&repo, &ev_dg, &docker_tag_only)?;
    st.ck(
        "★태그가 다른 이미지를 가리켜도 측정 digest 로만 관측(skip 재구성 ✗)",
        ap_dg["patches"].items().iter().all(|r| eqs(&r["result"], "unobserved"))
            && sw(&ap_dg["probes"][1]["result"], "image-absent-local")
            && eqs(&ap_dg["probes"][1]["image"], &dg)
            && eqs(&ap_dg["reconstruction"]["image"], &dg),
    );
    let ap_dg2 = aset(&repo, &ev_dg, &docker_old)?;
    st.ck(
        "측정 digest 이미지의 history 로 재구성",
        eqs(&by(&ap_dg2["patches"], "file")["50-port.sh"]["result"], "skipped") && has(&ap_dg2["reconstruction"]["build_args_source"], &dg),
    );
    let trip_w = jd(&[
        ("env", "output/multi/envs/.env.wheel".into()),
        ("yaml", format!("output/multi/configs/{cell}.yaml").into()),
        ("sh", format!("output/multi/configs/{cell}.sh").into()),
    ]);
    let ev_w = with(&ev, vec![("triplet", trip_w.clone())]);
    wt(&out.join("envs/.env.wheel"), "IMAGE_TAG=easy-vllm:fx-wheel\nBUILD_DOCKERFILE=Dockerfile\n")?;
    let apw = aset(&repo, &ev_w, &docker_old)?;
    st.ck(
        "★wheel 트랙 패치 미실행(excluded_by_recipe)",
        apw["patches"].items().is_empty() && apw["excluded"].items().iter().filter(|x| sw(&x["why"], "excluded_by_recipe")).count() == 5,
    );
    let wheel_text = rt(&out.join("Dockerfile"))?;
    wt(&out.join("build_patches/20-other.sh"), "#!/bin/bash\n")?;
    wt(&out.join("Dockerfile"), &format!("{wheel_text}\nCOPY build_patches/10-shared.sh /tmp/build_patches/\n"))?;
    let file_copy = aset(&repo, &ev_w, &docker_old)?;
    let copy_excluded: BTreeSet<(String, String)> = file_copy["excluded"]
        .items()
        .iter()
        .filter(|r| sw(&r["why"], "excluded_by_recipe"))
        .map(|r| (r["phase"].display(), r["file"].display()))
        .collect();
    st.ck("exact-file COPY is not falsely excluded", !copy_excluded.contains(&("post".into(), "10-shared.sh".into())));
    let want: BTreeSet<(String, String)> = [("post", "20-other.sh"), ("pre", "50-port.sh"), ("pre", "55-deps.sh"), ("pre", "58-effect-first.sh"), ("pre", "60-ungated.sh")]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    st.ck("exact-file COPY does not admit uncopied POST/PRE files", copy_excluded == want);
    wt(&out.join("Dockerfile"), &wheel_text)?;
    rm(&out.join("build_patches/20-other.sh"))?;
    // ★결함 B(2026-10-10 · plan_26101017 · M2)
    wt(&out.join("envs/.env.nosel"), "IMAGE_TAG=easy-vllm:fx-wheel\n")?;
    let ev_ns = with(&ev, vec![("triplet", with(&trip_w, vec![("env", "output/multi/envs/.env.nosel".into())]))]);
    let hsrc = format!("{}(VLLM_VERSION=0.8.0 · VLLM_REF 없음 → wheel Dockerfile)", crate::evidence::HISTORY_TRACK_MARK);
    let ev_h = with(
        &ev_ns,
        vec![(
            "build_identity",
            with(&ev["build_identity"], vec![("dockerfile", "Dockerfile".into()), ("source", jd(&[("dockerfile", hsrc.as_str().into())]))]),
        )],
    );
    let sel_h = _build_selection(&ctx(&repo, &ev_h, Some(&docker_old))?, &J::Null)?;
    let sel_c = _build_selection(&ctx(&repo, &ev_ns, Some(&docker_old))?, &J::Null)?;
    st.ck(
        "★결함 B 선택자 순위: 원장 · 셀 env 없음 → 이미지 history 관측(wheel)이 compose 기본값(source-build)을 이긴다 · 출처 표시",
        eqs(&sel_h["dockerfile"], "Dockerfile") && eqs(&sel_h["track"], "wheel") && eqs(&sel_h["dockerfile_source"], &format!("image-history:{hsrc}")),
    );
    st.ck(
        "결함 B 음성대조: history 관측 표지가 없으면 종전대로 compose 기본값",
        eqs(&sel_c["dockerfile"], "Dockerfile.source-build") && eqs(&sel_c["dockerfile_source"], "compose-default:BUILD_DOCKERFILE"),
    );
    wt(&out.join("envs/.env.nodf"), "IMAGE_TAG=easy-vllm:fx-x\nBUILD_DOCKERFILE=Dockerfile.absent\n")?;
    let ev_nodf = with(&ev_w, vec![("triplet", with(&trip_w, vec![("env", "output/multi/envs/.env.nodf".into())]))]);
    st.raises("★쓰인 Dockerfile 부재", "HINT_DOCKERFILE_ABSENT", || aset(&repo, &ev_nodf, &docker_old));
    fx.ev_w = ev_w;
    Ok(())
}
