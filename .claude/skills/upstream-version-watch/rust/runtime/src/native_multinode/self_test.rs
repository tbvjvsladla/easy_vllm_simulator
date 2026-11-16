//! `native_multinode_serve.py --self-test` in Rust: fake runners over a temporary tree — no real serving, Ray, container or process kill.
//!
//! Ported: owned-tree primitives, the fixed remote program (run through `sh -c` instead of ssh), PID-reuse refusal, and the scenarios
//! (normal up/down, start failure → auto down, health window exhausted, RAM gate failure, PID reuse at down, GPU residue, dry-run purity).
//! NOT ported (remain in the Python original, need `hintlib`): the hint-publisher judgements (`_serve_proof_ok`, `_native_producer_evidence`,
//! `artifacts._native_launch_path`). The launch-record substitution and scan are the Rust `pii` module and are checked here.

use super::up::{real_http, NativeServe};
use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct FakeClock(Cell<f64>);
impl Clock for FakeClock {
    fn now(&self) -> f64 {
        self.0.get()
    }
    fn mono(&self) -> f64 {
        self.0.get()
    }
    fn sleep(&self, s: f64) {
        self.0.set(self.0.get() + s);
    }
}

#[derive(Clone)]
struct Alive {
    pid: i64,
    ppid: i64,
    pgid: i64,
    starttime: String,
    argv: String,
}
#[derive(Default)]
struct World {
    cfg_dir: String,
    pid: i64,
    alive: BTreeMap<(String, i64), Alive>,
    argv: BTreeMap<String, Vec<String>>,
    budget: Vec<String>, // node names
}
type Fault = Rc<RefCell<BTreeSet<String>>>;

struct Fake {
    node: String,
    root: String,
    clock: Rc<FakeClock>,
    w: Rc<RefCell<World>>,
    fault: Fault,
    calls: Rc<Cell<usize>>,
}
impl Fake {
    fn local(&self) -> Local {
        Local::new(&self.root, &self.node)
    }
    fn has(&self, f: &str) -> bool {
        self.fault.borrow().contains(f)
    }
}
impl Runner for Fake {
    fn node(&self) -> &str {
        &self.node
    }
    fn root(&self) -> &str {
        &self.root
    }
    fn init_root(&self, ensure_parent: bool) -> R<()> {
        self.local().init_root(ensure_parent)
    }
    fn put(&self, rel: &str, data: &[u8], mode: u32) -> R<()> {
        self.local().put(rel, data, mode)
    }
    fn run(&self, a: &[String], _timeout: u64, check: bool) -> R<CmdResult> {
        self.calls.set(self.calls.get() + 1);
        let s = a.join(" ");
        let (mut rc, mut out) = (0, String::new());
        let is = |i: usize, v: &str| a.get(i).map(String::as_str) == Some(v);
        if s.contains("check_smoke_model") {
            out = "[NAS-check] BUDGET_PARAMS ckpt_mib=60000 tp=2 kv_mib=20480 ple_mib=40000\n[NAS-check] GATE_PARAMS required_mib=40000\n".into();
        } else if is(0, "cat") && is(1, "/proc/meminfo") {
            if self.has("meminfo") {
                return serve(format!("{}: unreachable", self.node));
            }
            out = "MemTotal:       125000000 kB\nMemAvailable:   110000000 kB\n".into();
        } else if is(0, "cat") && a.get(1).is_some_and(|p| p.starts_with("/proc/") && p.ends_with("/cmdline")) {
            let cfg = self.w.borrow().cfg_dir.clone();
            let parts = [format!("{}/venv/bin/python3", self.root), format!("{}/venv/bin/vllm", self.root), "serve".into(), "--config".into(), format!("{cfg}/src-native.yaml"), "--served-model-name".into(), "fx-model".into()];
            out = parts.join("\0") + "\0";
        } else if a.iter().any(|x| x == "budget-defaults") {
            out = "7200\n".into();
        } else if s.contains("budget_preflight") {
            out = "{\"floor_mib\": 50000, \"arm_ceiling_mib\": 41808}".into();
        } else if a.iter().any(|x| x == "declare-budget" || x == "clear-budget") {
            self.w.borrow_mut().budget.push(self.node.clone());
        } else if is(0, "tail") {
            out = format!("{{\"ts\": \"{}\", \"event\": \"budget_honored\"}}\n", utc(self.clock.now()));
        } else if is(0, "test") && is(1, "-f") && a.get(2).is_some_and(|p| p.ends_with("serve_budget.env")) {
            rc = 1;
        } else if is(0, "test") && is(1, "-x") {
            rc = 1;
        } else if is(0, "du") {
            out = format!("1073741824\t{}\n", a.last().unwrap());
        } else if a.first().is_some_and(|t| t.ends_with("/scripts/native_wheelhouse.sh") || t.ends_with("/tools/native_wheelhouse")) && is(1, "verify") {
            let rl = format!("{}/wh/runtime-lib", self.root);
            let ld = format!("{rl}/cuda/lib:{rl}/lib");
            let env = dict(vec![("LD_LIBRARY_PATH", vs(&ld)), ("TRITON_PTXAS_PATH", vs(&format!("{rl}/cuda/bin/ptxas"))), ("PATH", vs(&format!("{rl}/cuda/bin:{}/venv/bin:/srv/x/bin:/usr/bin", self.root)))]);
            let doc = dict(vec![("kind", vs("native_wheelhouse_verify")), ("verdict", vs("PASS")), ("selected", vs("compat-first")), ("ld_library_path", vs(&ld)), ("env", env)]);
            out = dumps_pretty(&doc);
        } else if a.len() >= 2 && a[a.len() - 2] == "freeze" && a[a.len() - 1] == "--all" {
            out = "ray==2.49.0\ntorch==2.13.0a0\nvllm==0.29.0rc6\n".into();
        } else if a.iter().any(|x| x == "status") && a.iter().any(|x| x.ends_with("/bin/ray")) {
            out = "Resources\n 0.0/2.0 GPU\n".into();
        } else if is(0, "mkdir") {
        }
        if check && rc != 0 {
            return serve(format!("{}: rc={rc}: {:?}", self.node, &a[..a.len().min(3)]));
        }
        Ok(CmdResult { rc, out, err: String::new() })
    }
    fn start(&self, argv: &[String], log_rel: &str, role: &str) -> R<Identity> {
        if self.has(&format!("start:{role}")) {
            return serve(format!("fake {role} exited at start"));
        }
        let mut w = self.w.borrow_mut();
        w.pid = if w.pid == 0 { 1001 } else { w.pid + 1 };
        let pid = w.pid;
        self.local().put(log_rel, format!("{role} log line\n").as_bytes(), 0o600)?;
        let ident = Identity { node: self.node.clone(), role: role.into(), pid, pgid: pid, starttime: (pid * 7).to_string(), log_rel: log_rel.into() };
        w.alive.insert((self.node.clone(), pid), Alive { pid, ppid: 1, pgid: pid, starttime: (pid * 7).to_string(), argv: role.into() });
        if role == "ray-worker" || role == "vllm-serve" {
            w.pid += 1;
            let kid = w.pid;
            w.alive.insert((self.node.clone(), kid), Alive { pid: kid, ppid: pid, pgid: pid, starttime: (kid * 7).to_string(), argv: format!("{role}-worker") });
        }
        w.argv.insert(role.into(), argv.to_vec());
        Ok(ident)
    }
    fn stop(&self, ident: &Identity, _wait: u64, pid_only: bool) -> R<PyValue> {
        let mut w = self.w.borrow_mut();
        let row = w.alive.get(&(self.node.clone(), ident.pid)).cloned();
        if row.as_ref().is_some_and(|r| r.starttime != ident.starttime) || (self.has("reuse") && (ident.role == "ray-head" || ident.role == "ray-worker")) {
            return unknown(format!("{}: pid-reuse-before-term", self.node));
        }
        let keys: Vec<(String, i64)> = w.alive.iter().filter(|(k, v)| k.0 == self.node && (v.pid == ident.pid || (!pid_only && v.pgid == ident.pgid))).map(|(k, _)| k.clone()).collect();
        for k in keys {
            if self.has("survive") && w.alive[&k].argv.ends_with("-worker") {
                continue;
            }
            w.alive.remove(&k);
        }
        Ok(dict(vec![("node", vs(&self.node)), ("role", vs(&ident.role)), ("pid", vi(ident.pid)), ("term_status", vs("term-exited")), ("starttime_match", PyValue::Bool(true))]))
    }
    fn text(&self, rel: &str) -> R<String> {
        self.local().text(rel)
    }
    fn tree(&self, pids: &[i64], pgids: &[i64]) -> R<Vec<ProcRow>> {
        let w = self.w.borrow();
        let rows: Vec<&Alive> = w.alive.iter().filter(|(k, _)| k.0 == self.node).map(|(_, v)| v).collect();
        let mut keep: BTreeSet<i64> = BTreeSet::new();
        let mut todo: Vec<i64> = pids.iter().copied().filter(|p| rows.iter().any(|r| r.pid == *p)).collect();
        while let Some(x) = todo.pop() {
            if !keep.insert(x) {
                continue;
            }
            todo.extend(rows.iter().filter(|r| r.ppid == x).map(|r| r.pid));
        }
        keep.extend(rows.iter().filter(|r| pgids.contains(&r.pgid)).map(|r| r.pid));
        Ok(rows.iter().filter(|r| keep.contains(&r.pid)).map(|r| ProcRow { pid: r.pid, ppid: r.ppid, pgid: r.pgid, starttime: r.starttime.clone(), argv: r.argv.clone() }).collect())
    }
    fn gpu(&self) -> R<Vec<String>> {
        if self.has("gpu") {
            return unknown(format!("{}: fake GPU unknown", self.node));
        }
        let w = self.w.borrow();
        Ok(w.alive.iter().filter(|(k, r)| k.0 == self.node && r.argv.ends_with("-worker")).map(|(_, r)| format!("{}, python, 1000", r.pid)).collect())
    }
    fn ports(&self) -> R<Vec<i64>> {
        let w = self.w.borrow();
        Ok(if self.node == "main" && w.alive.keys().any(|k| k.0 == "main") { vec![8080] } else { vec![] })
    }
    fn exists(&self) -> R<bool> {
        Ok(lexists(Path::new(&self.root)))
    }
    fn remove_owned_tree(&self, run_id: &str) -> R<()> {
        safe_remove_owned_tree(Path::new(&self.root), run_id)
    }
}

struct Scenario {
    repo: PathBuf,
    ctx: Ctx,
    world: Rc<RefCell<World>>,
    faults: Vec<Fault>,
    r: Runners,
    rs: Runners,
    calls: Rc<Cell<usize>>,
    clock: Rc<FakeClock>,
    http_ok: bool,
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn fixture_repo(base: &Path, main_ip: &str, sub_ip: &str) -> (PathBuf, String) {
    let extra = format!("nodes:\n  - role: main\n    host: \"{main_ip}\"\n    ssh_user: \"u\"\n    work_dir: \"/fx/repo\"\n  - role: sub\n    host: \"{sub_ip}\"\n    ssh_user: \"u\"\n    work_dir: \"/fx/sub\"\n");
    let repo = crate::native_triplet::fixture_extra(&base.join("repo"), &extra);
    let rendered = crate::native_triplet::render(&repo, "multi", "src", "src-native").ok().expect("render fixture");
    crate::native_triplet::apply(&repo, &rendered).ok().expect("apply fixture");
    write(&repo.join("output/multi/envs/.env.interconnect"), "NCCL_SOCKET_IFNAME=eth9\nNCCL_IB_HCA==dev0\n");
    write(&repo.join("output/multi/envs/.env.cluster"), &format!("MASTER_HOST_IP={main_ip}\nSLAVE_HOST_IP={sub_ip}\nRAY_PORT=6379\nRAY_OBJECT_STORE_MEMORY=2000000000\n"));
    write(&repo.join(WHEEL_LAUNCHER_REL), "# fixture wheelhouse launcher\n");
    write(&repo.join(WHEEL_ELF_REL), "\x7fELF fixture wheelhouse ELF\n");
    write(&repo.join(WHEEL_ACCEPT_REL), "{\"fixture\": \"accept\"}\n");
    let camp = format!(
        "{{\"id\": \"camp-fx\", \"budgets\": {{\"ready_max_seconds\": 60, \"smoke_budget_overhead_mib\": 24800}}, \"topology_sections\": {{\"multi\": {{\"native_cells\": [\"src-native\"], \"native_run_root_base\": \"{}\", \"native_run_root_max_gib_per_node\": 80, \"native_sub_host\": \"u@{sub_ip}\"}}}}}}",
        base.join("runs").display()
    );
    write(&repo.join("campaigns/camp-fx/campaign.yaml"), &camp);
    write(&repo.join("campaigns/ACTIVE"), "camp-fx\n");
    (repo, "src-native".into())
}

fn scenario(base: &Path, tag: &str, faults: &[&str], http_ok: bool) -> Scenario {
    let (repo, cell) = fixture_repo(&base.join(tag), "198.51.100.1", "198.51.100.2");
    let mut ctx = load_ctx(&repo, &cell, &format!("r-{tag}"), None, None, "native_N1").ok().expect("load_ctx fixture");
    ctx.main.root = base.join(tag).join("main-run").join(&ctx.run_id).to_string_lossy().into_owned();
    ctx.main.short = base.join(tag).join("main-short").join(&ctx.run_id).to_string_lossy().into_owned();
    ctx.sub.root = base.join(tag).join("sub-run").join(&ctx.run_id).to_string_lossy().into_owned();
    ctx.sub.short = base.join(tag).join("sub-short").join(&ctx.run_id).to_string_lossy().into_owned();
    let clock = Rc::new(FakeClock(Cell::new(1_790_000_000.0)));
    let world = Rc::new(RefCell::new(World { cfg_dir: repo.join("output/multi/configs").to_string_lossy().into_owned(), ..World::default() }));
    let fault: Fault = Rc::new(RefCell::new(faults.iter().map(|s| s.to_string()).collect()));
    let calls = Rc::new(Cell::new(0));
    let mk = |node: &str, root: &str| -> Box<dyn Runner> { Box::new(Fake { node: node.into(), root: root.into(), clock: clock.clone(), w: world.clone(), fault: fault.clone(), calls: calls.clone() }) };
    let r = Runners { main: mk("main", &ctx.main.root), sub: mk("sub", &ctx.sub.root) };
    let rs = Runners { main: mk("main", &ctx.main.short), sub: mk("sub", &ctx.sub.short) };
    Scenario { repo, ctx, world, faults: vec![fault], r, rs, calls, clock, http_ok }
}

impl Scenario {
    fn up(&self, here: &Path, utc_s: &str) -> (i32, String) {
        let polls = Cell::new(0);
        let ok = self.http_ok;
        let http = move |_m: &str, url: &str, _b: Option<&PyValue>, _t: u64| -> (i64, String) {
            if url.ends_with("/health") {
                polls.set(polls.get() + 1);
                return if ok && polls.get() >= 3 { (200, String::new()) } else { (503, String::new()) };
            }
            (200, "{\"choices\": [{\"message\": {\"content\": \"4\"}, \"finish_reason\": \"stop\"}]}".into())
        };
        let logs = RefCell::new(String::new());
        let log = |m: &str| logs.borrow_mut().push_str(&format!("{m}\n"));
        let mut ns = NativeServe::new(&self.ctx, &self.r, &self.rs, &http, self.clock.as_ref(), &log, here.to_path_buf());
        let rc = ns.up(utc_s);
        (rc, logs.into_inner())
    }
    fn proof(&self) -> PyValue {
        read_json(&self.repo.join(&self.ctx.proof_rel))
    }
    fn state(&self) -> PyValue {
        read_json(&Path::new(&self.ctx.main.root).join("state.json"))
    }
    fn down(&self, st: &PyValue) -> PyValue {
        down_from_state(st, &self.repo, &self.r, &self.rs, &FakeClock(Cell::new(1_790_000_000.0)), &|_| {})
    }
    fn alive(&self) -> usize {
        self.world.borrow().alive.len()
    }
    fn budget_count(&self, node: &str) -> usize {
        self.world.borrow().budget.iter().filter(|n| n.as_str() == node).count()
    }
    fn has_main_alive(&self) -> bool {
        self.world.borrow().alive.keys().any(|k| k.0 == "main")
    }
    fn argv(&self, role: &str) -> Vec<String> {
        self.world.borrow().argv.get(role).cloned().unwrap_or_default()
    }
}

fn read_json(p: &Path) -> PyValue {
    std::fs::read(p).ok().and_then(|b| agent_runtime_support::pyjson::loads(&String::from_utf8_lossy(&b)).ok()).unwrap_or(PyValue::None)
}
fn jstr(v: &PyValue, ks: &[&str]) -> String {
    py_str(dpath(v, ks))
}

pub fn self_test(here: &Path) -> i32 {
    let mut bad: Vec<String> = Vec::new();
    let mut ck = |name: &str, cond: bool| {
        say(&format!("{}{name}", if cond { "[PASS] " } else { "[FAIL] " }));
        if !cond {
            bad.push(name.to_string());
        }
    };
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let base = std::env::temp_dir().join(format!("native-mn-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let marker = |p: &Path, id: &str| std::fs::write(p.join(MARKER), marker_bytes(id)).unwrap();

    // ── owned-tree primitives ──
    let root = base.join("run");
    std::fs::create_dir_all(root.join("a")).unwrap();
    marker(&root, "run");
    std::fs::write(root.join("a/f"), "x").unwrap();
    let _ = safe_remove_owned_tree(&root, "run");
    ck("owned tree depth-first deletion", !root.exists());
    let outside = base.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep"), "k").unwrap();
    let lr = base.join("linkrun");
    std::fs::create_dir_all(lr.join("lib")).unwrap();
    marker(&lr, "linkrun");
    std::os::unix::fs::symlink(outside.join("keep"), lr.join("lib/libx.so")).unwrap();
    std::os::unix::fs::symlink(&outside, lr.join("lib/dirlink")).unwrap();
    std::os::unix::fs::symlink(base.join("nope"), lr.join("lib/dangling")).unwrap();
    drop(std::os::unix::net::UnixListener::bind(lr.join("lib/ray.sock")));
    let _ = Command::new("mkfifo").arg(lr.join("lib/p.fifo")).status();
    let res = safe_remove_owned_tree(&lr, "linkrun");
    ck("★트리 안 유닉스 소켓·FIFO 는 unlink(Ray/ZMQ ipc 잔재)", res.is_ok() && !lexists(&lr));
    ck("★트리 안 링크 = 링크만 unlink · 대상 불변(파일·디렉터리·끊긴 링크)", !lexists(&lr) && std::fs::read_to_string(outside.join("keep")).unwrap_or_default() == "k");
    // symlink root / bad marker
    let real = base.join("realroot");
    std::fs::create_dir_all(&real).unwrap();
    marker(&real, "symlink-root");
    std::os::unix::fs::symlink(&real, base.join("symlink-root")).unwrap();
    ck("reject symlink-root", safe_remove_owned_tree(&base.join("symlink-root"), "symlink-root").is_err());
    let bm = base.join("bad-marker");
    std::fs::create_dir_all(&bm).unwrap();
    std::fs::write(bm.join(MARKER), "bad").unwrap();
    ck("reject bad-marker", safe_remove_owned_tree(&bm, "bad-marker").is_err());

    // ── fixed sub-node runner (`native_remote`, this executable's sibling), run locally through `sh -c` (same argv as over ssh) ──
    let rroot = base.join("rparent/rid1");
    let remote = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("native_remote"))).unwrap_or_default();
    ck("native_remote ELF 가 이 실행파일 옆에 있다(서브 고정 러너 · Python ✗)", remote.is_file());
    let rr = Ssh { root: rroot.to_string_lossy().into_owned(), node: "sub".into(), transport: vec!["sh".into(), "-c".into()], remote: remote.to_string_lossy().into_owned() };
    let rinit = rr.init_root(true);
    ck("remote init(ensure_parent) + 정확한 마커", rinit.is_ok() && std::fs::read(rroot.join(MARKER)).ok().as_deref() == Some(marker_bytes("rid1").as_slice()));
    ck("remote init 은 기존 루트를 거부", matches!(rr.init_root(false), Err(Err::Unknown(_))));
    let _ = rr.put("tools/t.py", b"print(1)\n", 0o600);
    ck("remote put + text", rr.text("tools/t.py").ok().as_deref() == Some("print(1)\n"));
    ck("remote put 은 덮어쓰기 거부", matches!(rr.put("tools/t.py", b"x", 0o600), Err(Err::Unknown(_))));
    ck("remote 경로 탈출 거부", matches!(rr.text("../escape"), Err(Err::Unknown(_))));
    ck("remote run(정확한 argv · 셸 없음)", rr.run(&sv(&["printf", "%s", "a b;c"]), 10, true).map(|r| r.out).ok().as_deref() == Some("a b;c"));
    let me = i64::from(std::process::id());
    ck("remote tree(ppid 사슬)가 자기 부모를 본다", rr.tree(&[me], &[]).map(|rows| rows.iter().any(|x| x.pid == me)).unwrap_or(false));
    let probe = Identity { node: "sub".into(), role: "reuse-probe".into(), pid: me, pgid: me, starttime: "0".into(), log_rel: String::new() };
    ck("★remote stop: starttime 불일치 = 신호 없이 거부(PID 재사용)", matches!(rr.stop(&probe, 1, false), Err(Err::Unknown(m)) if m.contains("pid-reuse-before-term")));
    let lprobe = Identity { node: "main".into(), ..probe.clone() };
    ck("★local stop: starttime 불일치 = 신호 없이 거부(PID 재사용)", matches!(Local::new(&base.to_string_lossy(), "main").stop(&lprobe, 1, false), Err(Err::Unknown(m)) if m.contains("pid-reuse-before-term")));
    std::os::unix::fs::symlink(&base, rroot.join("evil")).unwrap();
    std::fs::create_dir_all(rroot.join("d")).unwrap();
    std::os::unix::fs::symlink(outside.join("keep"), rroot.join("d/l.so")).unwrap();
    let rm = rr.remove_owned_tree("rid1");
    ck("★remote remove: 트리 안 링크는 링크만 unlink · 대상 불변 · 깊이 우선 삭제", rm.is_ok() && !rr.exists().unwrap_or(true) && base.is_dir() && std::fs::read_to_string(outside.join("keep")).unwrap_or_default() == "k");

    // ── normal path ──
    let sc = scenario(&base, "ok", &[], true);
    let (rc, ok_log) = sc.up(here, "2026-09-23T03:30:00Z");
    if rc != 0 {
        say(&format!("[diag] up(ok) rc={rc}\n{}", ok_log.trim_end()));
    }
    let proof = sc.proof();
    ck("up 정상 = exit 0 · proof PASS · 서빙 유지(리더 생존)", rc == 0 && jstr(&proof, &["status"]) == "PASS" && sc.has_main_alive());
    let st = sc.state();
    let kinds: Vec<String> = list_items(dget(&st, "identities")).iter().map(|x| format!("{}:{}", py_str(dget(x, "kind")), py_str(dget(x, "role")))).collect();
    let cnt = |p: &str| kinds.iter().filter(|k| k.starts_with(p)).count();
    ck("state.json = 리더 3 + 워치독 3(그룹마다) + 갱신 루프 2", kinds.iter().filter(|k| k.as_str() == "serve:ray-head").count() == 1 && cnt("watchdog") == 3 && cnt("renew") == 2);
    let wd = sc.argv("watchdog-vllm-serve");
    ck("워치독 = host_safety/mem_watchdog.sh --pgid --starttime(이름 ✗)", wd.get(1).is_some_and(|x| x.ends_with("host_safety/mem_watchdog.sh")) && wd.contains(&"--pgid".to_string()) && wd.contains(&"--starttime".to_string()));
    let rn = sc.argv("budget-renew-main");
    ck("갱신 루프 = --node-dir --pgid --starttime --ttl-s", ["--node-dir", "--pgid", "--starttime", "--ttl-s"].iter().all(|x| rn.iter().any(|t| t == x)));
    let head = sc.argv("ray-head");
    let mroot = sc.ctx.main.root.clone();
    ck(
        "env -i · 캐시·TMPDIR·HOME 가 소유 루트 아래(HOME 독립)",
        head.len() > 2
            && head[..2] == ["env".to_string(), "-i".to_string()]
            && ["XDG_CACHE_HOME", "TRITON_CACHE_DIR", "TORCHINDUCTOR_CACHE_DIR", "FLASHINFER_WORKSPACE_BASE", "VLLM_CACHE_ROOT", "HF_HOME", "PIP_CACHE_DIR", "HOME"].iter().all(|k| head.iter().any(|t| t.starts_with(&format!("{k}={mroot}/"))))
            && head.contains(&format!("TMPDIR={}/tmp", sc.ctx.main.short)),
    );
    ck(
        "NCCL interconnect env + verify 레시피(LD_LIBRARY_PATH·번들 도구) · PATH 는 소유 루트 항목만",
        head.contains(&"NCCL_IB_HCA==dev0".to_string()) && head.iter().any(|t| t.starts_with("LD_LIBRARY_PATH=") && t.contains("runtime-lib")) && head.iter().any(|t| t.starts_with("TRITON_PTXAS_PATH=")) && !head.iter().any(|t| t.starts_with("PATH=") && t.contains("/srv/x")),
    );
    let wk = sc.argv("ray-worker");
    ck("서브 워커엔 셀 트리플렛(CONFIG_FILE) 미전파 · serve_env(PLE)는 전달", !wk.iter().any(|t| t.starts_with("CONFIG_FILE=")) && wk.contains(&"VLLM_PLE_MMAP=1".to_string()));
    let sv_argv = sc.argv("vllm-serve");
    ck("vllm serve = 렌더된 native .sh", sv_argv.len() >= 2 && sv_argv[sv_argv.len() - 2..] == ["bash".to_string(), sc.repo.join("output/multi/configs/src-native.sh").to_string_lossy().into_owned()]);
    ck("예산 clear→declare 양 노드(로드 전)", sc.budget_count("main") >= 2 && sc.budget_count("sub") >= 2);
    let inst = std::fs::read_to_string(sc.repo.join(jstr(&proof, &["native_install_path"]))).unwrap_or_default();
    ck("native-install.sh = 실행 빌더와 같은 명령(build→verify) · 운영자 경로 ✗", inst.contains("--no-index") && inst.contains("\"${RUN_ROOT}/wh\"") && inst.contains(" verify ") && inst.contains(" build ") && !inst.contains(&*base.to_string_lossy()) && !inst.contains("/home/"));
    let att = sc.down(&st);
    ck(
        "down 정상 = attestation PASS · 루트 부재 · 잔재 0",
        jstr(&att, &["status"]) == "PASS" && !lexists(Path::new(&sc.ctx.main.root)) && !lexists(Path::new(&sc.ctx.sub.short)) && sc.alive() == 0,
    );
    let prel = jstr(&st, &["preserve_rel"]);
    ck("down 이 로그·state 를 삭제 전에 보존", sc.repo.join(&prel).join("logs/main-vllm-serve.log").is_file() && sc.repo.join(&prel).join("logs/state.json").is_file());
    ck(
        "attestation 모양(unknown 리스트 · evidence_preserved · errors=[])",
        list_items(dpath(&att, &["nodes", "main", "unknown"])).is_empty() && matches!(dget(&att, "evidence_preserved"), Some(PyValue::Bool(true))) && list_items(dget(&att, "errors")).is_empty(),
    );
    let lrel = jstr(&proof, &["native_launch_path"]);
    let ltxt = std::fs::read_to_string(sc.repo.join(&lrel)).unwrap_or_default();
    ck(
        "★기동 기록: proof native_launch_path = native-launch.md · native_launch_error 없음 · 서빙 판정 PASS",
        lrel.ends_with("native-launch.md") && !ltxt.is_empty() && dget(&proof, "native_launch_error").is_none() && jstr(&proof, &["status"]) == "PASS",
    );
    ck(
        "★치환본: 노드 IP·repo·run root 원값 0 · 표지 존재 · deploy 스캔 0",
        !ltxt.contains("198.51.100.") && !ltxt.contains(&sc.repo.to_string_lossy().to_string()) && !ltxt.contains(&sc.ctx.main.root)
            && ltxt.contains("<node:main>") && ltxt.contains(super::pii::PH_RUN_ROOT_BASE)
            && super::pii::scan_deploy(&ltxt, None).map(|h| h.is_empty()).unwrap_or(false),
    );
    {
        use super::pii::{scan_deploy, substitute, substitution_table};
        let tbl = substitution_table(&sc.repo, Some(&load_yaml(&sc.repo.join("output/multi/manifest.yaml")).ok().expect("manifest"))).ok().expect("table");
        let n = "10.1.2.3 and 10.1.2 §10.1.2 ok";
        let m = format!("a@b.example /home/zz/q spark-abc1f 198.51.100.1:80 198.51.100.12 {n}");
        let out = substitute(&m, &tbl).ok().expect("substitute");
        ck(
            "치환: 잔여 4종 가림 · 절번호 보존 · IPv4 경계(…100.12 는 노드로 오귀속 ✗) · 멱등",
            out.starts_with("<redacted> <abs-path> <host> <node:main>:80 198.51.100.12 <priv-ip> and <priv-ip> §10.1.2 ok")
                && substitute(&out, &tbl).ok().as_deref() == Some(out.as_str())
                && scan_deploy("host 10.9.8.7 and /mnt/x", Some(&["tok".to_string()])).map(|h| h.len() == 2).unwrap_or(false),
        );
    }

    // ── start failure → auto down ──
    let s2 = scenario(&base, "startfail", &["start:vllm-serve"], true);
    let (rc2, _) = s2.up(here, "2026-09-23T04:30:00Z");
    let proof2 = s2.proof();
    let att2 = read_json(&s2.repo.join(jstr(&proof2, &["cleanup_attestation_path"])));
    ck("★기동 실패 → exit≠0 · proof FAIL(stage=start)", rc2 == EXIT_FAIL && jstr(&proof2, &["status"]) == "FAIL" && jstr(&proof2, &["failed_stage"]) == "start");
    ck("★기동 실패 → 자동 down 잔재 0(루트 부재 · 프로세스 0 · attestation PASS)", jstr(&att2, &["status"]) == "PASS" && s2.alive() == 0 && !lexists(Path::new(&s2.ctx.main.root)) && !lexists(Path::new(&s2.ctx.sub.root)));
    ck("★기동 실패 → 예산 선언 회수(clear-budget 양 노드)", s2.budget_count("sub") >= 3);

    // ── health window exhausted → auto down ──
    let s3 = scenario(&base, "nohealth", &[], false);
    let (rc3, _) = s3.up(here, "2026-09-23T05:30:00Z");
    ck("★health 미도달 → 자동 down 잔재 0", rc3 == EXIT_FAIL && s3.alive() == 0 && !lexists(Path::new(&s3.ctx.main.root)));

    // ── RAM gate failure before run roots ──
    let s4 = scenario(&base, "gate", &[], true);
    s4.faults[0].borrow_mut().insert("meminfo".into());
    let (rc4, _) = s4.up(here, "2026-09-23T06:30:00Z");
    ck("★RAM 게이트 실패 → 루트 미생성 · 기동 0", rc4 == EXIT_FAIL && !lexists(Path::new(&s4.ctx.main.root)) && s4.alive() == 0);
    ck("RAM 게이트 실패 → 기동 기록 없음(실행된 기동 단계 0 · native_launch_path=null)", matches!(dget(&s4.proof(), "native_launch_path"), Some(PyValue::None)));

    // ── PID reuse at down → FAIL_CLOSED ──
    let s5 = scenario(&base, "reuse", &[], true);
    s5.up(here, "2026-09-23T07:30:00Z");
    let st5 = s5.state();
    s5.faults[0].borrow_mut().insert("reuse".into());
    let att5 = s5.down(&st5);
    ck("★PID 재사용 → attestation FAIL_CLOSED · unknown 비어있지 않음 · 루트 보존", jstr(&att5, &["status"]) == "FAIL_CLOSED" && !list_items(dpath(&att5, &["nodes", "main", "unknown"])).is_empty() && lexists(Path::new(&s5.ctx.main.root)));

    // ── GPU residue ──
    let s6 = scenario(&base, "gpu", &[], true);
    s6.up(here, "2026-09-23T08:30:00Z");
    let st6 = s6.state();
    s6.faults[0].borrow_mut().insert("survive".into());
    let att6 = s6.down(&st6);
    ck("★살아남은 소유 워커·GPU 잔재 → FAIL_CLOSED(루트 보존)", jstr(&att6, &["status"]) == "FAIL_CLOSED" && !list_items(dpath(&att6, &["nodes", "main", "owned_gpu_processes"])).is_empty() && lexists(Path::new(&s6.ctx.main.root)));

    // ── dry-run: plan only, no side effects ──
    let s7 = scenario(&base, "dry", &[], true);
    let snapshot = |d: &Path| -> Vec<String> {
        fn walk(p: &Path, out: &mut Vec<String>) {
            if let Ok(rd) = std::fs::read_dir(p) {
                for e in rd.flatten() {
                    out.push(e.path().to_string_lossy().into_owned());
                    if e.path().is_dir() && !is_symlink(&e.path()) {
                        walk(&e.path(), out);
                    }
                }
            }
        }
        let mut v = Vec::new();
        walk(d, &mut v);
        v.sort();
        v
    };
    let before = snapshot(&base);
    let lines = plan(&s7.ctx, &s7.repo.join(".claude/skills/upstream-version-watch/scripts"));
    let after = snapshot(&base);
    ck("★dry-run: 파일 변화 0 · 러너 호출 0", before == after && s7.calls.get() == 0 && s7.world.borrow().alive.is_empty());
    ck(
        "dry-run: 노드별 명령(메인·서브 · wheelhouse·ray·serve·watchdog·renew)",
        lines.iter().any(|x| x.starts_with("[sub] ") && x.contains("tools/native_wheelhouse build") && x.contains("--accept-file") && x.contains("tools/native_wheelhouse_accept.json"))
            && lines.iter().any(|x| x.starts_with("[main] ") && x.contains("scripts/native_wheelhouse.sh build"))
            && lines.iter().filter(|x| x.contains("native_wheelhouse build") || x.contains("native_wheelhouse verify") || x.contains("native_wheelhouse.sh build") || x.contains("native_wheelhouse.sh verify")).count() == 4
            && lines.iter().filter(|x| x.contains("native_wheelhouse build") || x.contains("native_wheelhouse verify") || x.contains("native_wheelhouse.sh build") || x.contains("native_wheelhouse.sh verify")).all(|x| x.contains("--accept-file"))
            && !lines.iter().any(|x| x.contains("python3") && x.contains("native_wheelhouse"))
            && lines.iter().any(|x| x.contains("ray start --head"))
            && lines.iter().any(|x| x.starts_with("[sub] bash") && x.contains("mem_watchdog.sh --pgid"))
            && lines.iter().any(|x| x.contains("budget_renew_loop.sh")),
    );

    // ── source contracts ──
    let body = [include_str!("../native_multinode.rs"), include_str!("up.rs")].concat();
    ck("no name-based cleanup primitives", !body.contains(concat!("p", "kill")) && !body.contains(concat!("p", "grep")) && !body.contains(concat!("kill", "all")));
    ck("same-starttime KILL contract", body.contains("pid-reuse-before-kill"));
    ck("offline/no-index contract · 이미지 내 고정 wheelhouse 경로 기본값 없음", body.contains("--no-index") && !body.contains(concat!("/opt/", "wheelhouse")));
    let _ = real_http; // the real HTTP client is exercised by the live path only
    let _ = std::fs::remove_dir_all(&base);
    say(&format!("[native-multinode-serve] {}", if bad.is_empty() { "PASS" } else { "FAIL" }));
    if bad.is_empty() {
        0
    } else {
        1
    }
}
