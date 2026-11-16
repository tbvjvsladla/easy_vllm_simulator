//! `native_multinode_serve.py up --apply` (class `NativeServe`) ported to Rust. Child module of `native_multinode`, so it shares that module's private
//! helpers. Clock, HTTP and the runners are injected so the self-test can drive the same code with fakes.
//!
//! Python that remains by decision: `blackbox_session.py` (node session tool, the blackbox plane is out of this campaign's scope).
//! The launch record is sanitised by `pii` (the ported part of `hintlib.pii`); a failed substitution or scan still surfaces as `native_launch_error`.

use super::*;
use std::net::{TcpStream, ToSocketAddrs};

pub const BUDGET_EVENT_WAIT_S: f64 = 40.0;
pub const SMOKE_PROMPT: &str = "2+2= ? 숫자만 답하세요.";

pub type HttpFn<'a> = &'a dyn Fn(&str, &str, Option<&PyValue>, u64) -> (i64, String);

fn find_after(text: &str, key: &str) -> Option<String> {
    let i = text.find(key)?;
    Some(text[i + key.len()..].to_string())
}

/// `_http` for the local vLLM server (plain HTTP/1.0, `Connection: close`).
pub fn real_http(method: &str, url: &str, body: Option<&PyValue>, timeout: u64) -> (i64, String) {
    let rest = match url.strip_prefix("http://") {
        Some(r) => r,
        None => return (0, String::new()),
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let to = Duration::from_secs(timeout.max(1));
    let addr = match hostport.to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => return (0, String::new()),
    };
    let mut s = match TcpStream::connect_timeout(&addr, to) {
        Ok(s) => s,
        Err(_) => return (0, String::new()),
    };
    let _ = s.set_read_timeout(Some(to));
    let _ = s.set_write_timeout(Some(to));
    let data = body.map(dumps_tight);
    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: {hostport}\r\nConnection: close\r\n");
    if let Some(d) = &data {
        req.push_str(&format!("Content-Type: application/json\r\nContent-Length: {}\r\n", d.len()));
    }
    req.push_str("\r\n");
    if let Some(d) = &data {
        req.push_str(d);
    }
    if s.write_all(req.as_bytes()).is_err() {
        return (0, String::new());
    }
    let mut buf = Vec::new();
    if s.read_to_end(&mut buf).is_err() && buf.is_empty() {
        return (0, String::new());
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let code = text.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse::<i64>().ok()).unwrap_or(0);
    let body_text = find_after(&text, "\r\n\r\n").unwrap_or_default();
    (code, body_text)
}

/// `parse_verify_verdict`: (passed, env recipe, reason).
pub fn parse_verify_verdict(text: &str) -> (bool, Env, String) {
    let t = text.trim();
    let last = t.lines().last().unwrap_or("");
    let mut doc: Option<PyValue> = None;
    for cand in [t, last] {
        if let Ok(d) = agent_runtime_support::pyjson::loads(cand) {
            doc = Some(d);
            break;
        }
    }
    let doc = match doc {
        Some(d) if matches!(d.peel(), PyValue::Dict(_)) => d,
        _ => return (false, Env::new(), "verify 출력이 JSON object 가 아니다".into()),
    };
    let verdict = if truthy(dget(&doc, "verdict")) { py_str(dget(&doc, "verdict")).to_uppercase() } else { String::new() };
    let mut env: Env = Env::new();
    if let Some(PyValue::Dict(d)) = dget(&doc, "env") {
        for (k, v) in d.iter() {
            env.insert(k.to_utf8().unwrap_or_default(), py_str(Some(v)));
        }
    }
    let envld = env.get("LD_LIBRARY_PATH").cloned().filter(|s| !s.is_empty());
    let ld: Option<String> = match envld {
        Some(v) => Some(v),
        None => as_str(dget(&doc, "ld_library_path")).filter(|s| !s.is_empty()),
    };
    let selected = dget(&doc, "selected");
    if verdict != "PASS" {
        let sel = match selected.map(PyValue::peel) {
            None => "None".to_string(),
            Some(PyValue::Str(_)) => PyValue::repr(selected.unwrap()),
            Some(_) => py_str(selected),
        };
        return (false, Env::new(), format!("verify verdict={} (selected={sel})", if verdict.is_empty() { "없음" } else { &verdict }));
    }
    match ld {
        Some(l) if !l.trim().is_empty() => {
            env.insert("LD_LIBRARY_PATH".into(), l.trim().to_string());
            (true, env, format!("PASS({})", py_str(selected)))
        }
        _ => (false, Env::new(), "verify 가 LD_LIBRARY_PATH 레시피를 내지 않았다".into()),
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
fn fl(x: f64) -> String {
    format!("{x:?}")
}
/// `j()` for a list of strings: `json.dumps(v, ensure_ascii=False, separators=(",", ":"))`.
fn j_list(v: &[String]) -> String {
    let mut out = String::from("[");
    for (i, s) in v.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    out.push(']');
    out
}
fn sha_hex(b: &[u8]) -> String {
    crate::wheelhouse_util::sha256_hex(b)
}

#[derive(Clone, Default)]
pub struct Step {
    pub node: String,
    pub step: String,
    pub env: Option<String>,
    pub cmd: Vec<String>,
    pub argv_sha256: String,
    pub t_rel_s: f64,
    pub log_rel: Option<String>,
    pub note: Option<String>,
    pub polls: Option<i64>,
    pub matched: Option<Option<String>>,
    pub elapsed_s: Option<f64>,
}

#[derive(Default)]
pub struct Launch {
    pub steps: Vec<Step>,
    pub envs: BTreeMap<String, Env>,
    pub t0: Option<f64>,
    pub generated_utc: String,
    pub serve_exec: Option<ServeExec>,
    pub inference_elapsed_s: Option<f64>,
    pub total_elapsed_s: Option<f64>,
}
#[derive(Clone)]
pub enum ServeExec {
    Observed(Vec<String>),
    Not(String),
}

/// Observations of the health + smoke stage (the Python `obs` dict).
#[derive(Clone, Default)]
pub struct Obs {
    pub present: bool,
    pub health_ok: bool,
    pub health_http: i64,
    pub ready_after_s: Option<i64>,
    pub window_s: i64,
    pub inf_attempted: bool,
    pub inf_ok: bool,
    pub inf_evidence: Option<&'static str>,
    pub inf_len: i64,
    pub chat_len: i64,
    pub reasoning_len: i64,
    pub finish_reason: Option<String>,
    pub inf_path: Option<&'static str>,
    pub inf_http: i64,
}
impl Obs {
    fn health_v(&self) -> PyValue {
        dict(vec![
            ("ok", PyValue::Bool(self.health_ok)),
            ("http_status", vi(self.health_http)),
            ("ready_after_s", self.ready_after_s.map_or(PyValue::None, vi)),
            ("window_s", vi(self.window_s)),
        ])
    }
    fn inference_v(&self) -> PyValue {
        if !self.inf_attempted {
            return dict(vec![("ok", PyValue::Bool(false)), ("verdict", vs("not-attempted"))]);
        }
        dict(vec![
            ("chat_content_len", vi(self.chat_len)),
            ("reasoning_len", vi(self.reasoning_len)),
            ("finish_reason", self.finish_reason.as_deref().map_or(PyValue::None, vs)),
            ("ok", PyValue::Bool(self.inf_ok)),
            ("evidence", self.inf_evidence.map_or(PyValue::None, vs)),
            ("completion_text_len", vi(self.inf_len)),
        ])
    }
    fn endpoints_v(&self) -> PyValue {
        let mut items = vec![("health", dict(vec![("path", vs("/health")), ("http_status", vi(self.health_http))]))];
        if self.inf_attempted {
            items.push(("inference", dict(vec![("path", vs(self.inf_path.unwrap_or(""))), ("http_status", vi(self.inf_http))])));
        }
        dict(items)
    }
}

fn node_rel(name: &str) -> usize {
    usize::from(name != "main")
}

pub struct NativeServe<'a> {
    pub c: &'a Ctx,
    pub r: &'a Runners,
    pub rs: &'a Runners,
    pub http: HttpFn<'a>,
    pub clock: &'a dyn Clock,
    pub log: &'a dyn Fn(&str),
    pub state: PyValue,
    pub budget_declared: [bool; 2],
    pub launch: Launch,
    /// Test seam: directory holding the helper launchers (`check_smoke_model.sh`, `budget_preflight.sh`).
    pub here: PathBuf,
}

fn state_set(state: &mut PyValue, key: &str, v: PyValue) {
    if let PyValue::Dict(d) = state {
        d.set(key, v);
    }
}
fn state_get<'a>(state: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    dget(state, key)
}
fn state_remove(state: &mut PyValue, key: &str) {
    if let PyValue::Dict(d) = state {
        if d.contains_key(key) {
            let mut out = PyDict::new();
            for (k, v) in d.iter() {
                if k.to_utf8().as_deref() != Some(key) {
                    out.insert(k.clone(), v.clone());
                }
            }
            *d = out;
        }
    }
}
/// `state.setdefault(key, {})[sub] = v`
fn state_sub_set(state: &mut PyValue, key: &str, sub: &str, v: PyValue) {
    if let PyValue::Dict(d) = state {
        if !d.contains_key(key) {
            d.set(key, PyValue::Dict(PyDict::new()));
        }
        if let Some(PyValue::Dict(inner)) = d.get_mut(key) {
            inner.set(sub, v);
        }
    }
}

impl<'a> NativeServe<'a> {
    pub fn new(c: &'a Ctx, r: &'a Runners, rs: &'a Runners, http: HttpFn<'a>, clock: &'a dyn Clock, log: &'a dyn Fn(&str), here: PathBuf) -> Self {
        NativeServe { c, r, rs, http, clock, log, state: PyValue::None, budget_declared: [false, false], launch: Launch::default(), here }
    }
    fn has_state(&self) -> bool {
        matches!(self.state.peel(), PyValue::Dict(_))
    }

    fn state_write(&self) -> R<()> {
        if !self.has_state() {
            return Ok(());
        }
        let path = Path::new(&self.c.main.root).join("state.json");
        if !path.parent().is_some_and(Path::is_dir) {
            return Ok(());
        }
        write_atomic(&path, &dumps_pretty(&self.state)).map_err(|e| Err::Serve(e.to_string()))
    }
    fn add_ident(&mut self, ident: &Identity, kind: &str, target: Option<&str>) -> R<()> {
        let mut row = ident.row();
        if let PyValue::Dict(d) = &mut row {
            d.set("kind", vs(kind));
            if let Some(t) = target {
                d.set("target_role", vs(t));
            }
        }
        if let PyValue::Dict(d) = &mut self.state {
            if !d.contains_key("identities") {
                d.set("identities", PyValue::List(Vec::new()));
            }
            if let Some(PyValue::List(l)) = d.get_mut("identities") {
                l.push(row);
            }
        }
        self.state_write()
    }
    fn preserve(&self, rel: &str, data: &[u8], overwrite: bool) -> R<String> {
        let prel = as_str(state_get(&self.state, "preserve_rel")).ok_or_else(|| Err::Serve("preserve_rel".into()))?;
        let pdir = self.c.repo.join(&prel);
        let dst = beneath(&pdir, &pdir.join(rel))?;
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p).map_err(|e| Err::Serve(e.to_string()))?;
        }
        if is_symlink(&dst) {
            return serve(format!("preserve target is symlink: {rel}"));
        }
        if overwrite {
            let tmp = dst.with_file_name(format!("{}.tmp", dst.file_name().and_then(|n| n.to_str()).unwrap_or("x")));
            std::fs::write(&tmp, data).map_err(|e| Err::Serve(e.to_string()))?;
            std::fs::rename(&tmp, &dst).map_err(|e| Err::Serve(e.to_string()))?;
        } else {
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&dst).map_err(|e| Err::Serve(e.to_string()))?;
            f.write_all(data).map_err(|e| Err::Serve(e.to_string()))?;
        }
        Ok(dst.strip_prefix(&self.c.repo).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| dst.to_string_lossy().into_owned()))
    }
    fn write_repo_json(&self, rel: &str, doc: &PyValue) -> R<()> {
        let p = self.c.repo.join(rel);
        if let Some(par) = p.parent() {
            std::fs::create_dir_all(par).map_err(|e| Err::Serve(e.to_string()))?;
        }
        write_atomic(&p, &dumps_pretty(doc)).map_err(|e| Err::Serve(e.to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    fn launch_step(&mut self, node: &str, step: &str, env_name: Option<&str>, env: Option<&Env>, cmd: &[String], full_argv: &[String], t_start: f64, mut extra: Step) -> R<()> {
        if let (Some(n), Some(e)) = (env_name, env) {
            if let Some(prev) = self.launch.envs.get(n) {
                if prev != e {
                    return serve(format!("기동 기록: 같은 env 블록 이름 {n} 에 다른 환경"));
                }
            }
            self.launch.envs.insert(n.to_string(), e.clone());
            if full_argv != env_argv(e, cmd).as_slice() {
                return serve(format!("기동 기록: {step} 의 argv 가 env 블록+cmd 와 다르다"));
            }
        } else if full_argv != cmd {
            return serve(format!("기동 기록: {step} 의 argv 가 cmd 와 다르다"));
        }
        extra.node = node.into();
        extra.step = step.into();
        extra.env = env_name.map(String::from);
        extra.cmd = cmd.to_vec();
        extra.argv_sha256 = sha_hex(j_list(full_argv).as_bytes());
        extra.t_rel_s = round1(t_start - self.launch.t0.unwrap_or(t_start));
        self.launch.steps.push(extra);
        Ok(())
    }

    fn write_launch(&mut self, status: &str, obs: &Obs, stage: &str, reason: &str) {
        if as_str(state_get(&self.state, "preserve_rel")).is_none() || self.launch.steps.is_empty() {
            return;
        }
        let now = self.clock.mono();
        self.launch.total_elapsed_s = Some(round1(now - self.launch.t0.unwrap_or(now)));
        let res: R<String> = (|| {
            let raw = native_launch_markdown(self.c, &self.launch, obs, status, stage, reason, &utc(self.clock.now()));
            let table = pii::launch_substitution_table(self.c)?;
            let text = pii::substitute(&raw, &table)?;
            let terms = pii::load_pii_terms(&self.c.repo)?;
            let hits = pii::scan_deploy(&text, terms.as_deref())?;
            if !hits.is_empty() {
                return serve(format!("치환 뒤에도 deploy 스캔 검출 {}건 — 기록을 쓰지 않는다", hits.len()));
            }
            self.preserve(NATIVE_LAUNCH_FILE, text.as_bytes(), true)
        })();
        match res {
            Ok(rel) => {
                state_set(&mut self.state, "native_launch_rel", vs(&rel));
                state_remove(&mut self.state, "native_launch_error");
            }
            Err(e) => {
                state_remove(&mut self.state, "native_launch_rel");
                let msg = format!("ServeError: {}", e.msg());
                let msg = tail_chars(&msg, 400);
                (self.log)(&format!("[native] ⚠ 기동 기록 미작성: {msg}"));
                state_set(&mut self.state, "native_launch_error", vs(&msg));
            }
        }
        let _ = self.state_write();
    }

    // ---- budget ----
    fn wait_event(&self, name: &str, kind: &str, t0: f64) -> R<Option<String>> {
        let ev = format!("{}/events/watchdog.jsonl", self.c.node(name).node_dir);
        let end = self.clock.mono() + BUDGET_EVENT_WAIT_S;
        let runner = self.r.get(name);
        loop {
            let r = runner.run(&sv(&["tail", "-n", "30", &ev]), 20, false)?;
            for line in r.out.lines().rev() {
                if line.contains(&format!("\"{kind}\"")) {
                    let ts = line.find("\"ts\"").and_then(|i| {
                        let rest = &line[i + 4..];
                        let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
                        rest.find('"').map(|e| rest[..e].to_string())
                    });
                    if let Some(t) = ts.and_then(|s| parse_utc(&s)) {
                        if t >= t0 {
                            return Ok(Some(line.to_string()));
                        }
                    }
                    break;
                }
            }
            if self.clock.mono() >= end {
                return Ok(None);
            }
            self.clock.sleep(1.0);
        }
    }
    fn session(&self, name: &str, args: &[&str]) -> Vec<String> {
        let spec = self.c.node(name);
        let mut v = sv(&[super::session_interpreter(&spec.session_py), &spec.session_py, "--node-dir", &spec.node_dir]);
        v.extend(args.iter().map(|s| s.to_string()));
        v.extend(["--now".to_string(), utc(self.clock.now())]);
        v
    }
    fn declare(&mut self, name: &str, weights: i64, kv: i64, ttl: i64) -> R<()> {
        let spec = self.c.node(name);
        let r = self.r.get(name);
        let mem = self.meminfo(name)?;
        let had = r.run(&sv(&["test", "-f", &format!("{}/serve_budget.env", spec.node_dir)]), 15, false)?.rc == 0;
        let t_clear = self.clock.now().floor();
        r.run(&self.session(name, &["clear-budget"]), 30, true)?;
        if had && self.wait_event(name, "budget_none", t_clear)?.is_none() {
            return serve(format!("{name}: clear-budget 상태 전이 미관측"));
        }
        let t0 = self.clock.now().floor();
        let a = self.session(
            name,
            &["declare-budget", "--mem-total-mib", &mem.0.to_string(), "--weights-mib", &weights.to_string(), "--kv-mib", &kv.to_string(), "--overhead-mib", &self.c.overhead_mib.to_string(), "--ttl-s", &ttl.to_string(), "--expected-load-s", &self.c.ready_max_s.to_string(), "--label", &format!("smoke-{}", self.c.cell)],
        );
        r.run(&a, 30, true)?;
        self.budget_declared[node_rel(name)] = true;
        if self.wait_event(name, "budget_honored", t0)?.is_none() {
            return serve(format!("{name}: budget_honored 미검출({}s) — 워치독이 선언을 수락하지 않았다(진입 차단)", BUDGET_EVENT_WAIT_S as i64));
        }
        (self.log)(&format!("[native] {name}: budget_honored ✓"));
        Ok(())
    }
    fn clear_budget(&self, name: &str) -> Option<String> {
        match self.r.get(name).run(&self.session(name, &["clear-budget"]), 30, true) {
            Ok(_) => None,
            Err(e) => Some(format!("{name}: clear-budget 실패: {}", e.msg())),
        }
    }
    /// (MemTotal MiB, MemAvailable MiB)
    fn meminfo(&self, name: &str) -> R<(i64, i64)> {
        let out = self.r.get(name).run(&sv(&["cat", "/proc/meminfo"]), 15, true)?.out;
        let get = |k: &str| -> Option<i64> {
            out.lines().find_map(|l| {
                let rest = l.strip_prefix(k)?.strip_prefix(':')?;
                let mut it = rest.split_whitespace();
                let n: i64 = it.next()?.parse().ok()?;
                (it.next()? == "kB").then_some(n / 1024)
            })
        };
        match (get("MemTotal"), get("MemAvailable")) {
            (Some(t), Some(a)) => Ok((t, a)),
            _ => serve(format!("{name}: /proc/meminfo 해석 실패")),
        }
    }

    // ---- up stages ----
    fn gates_and_budget(&mut self) -> R<(i64, i64, i64, String)> {
        let c = self.c;
        let h = self.here.to_string_lossy().into_owned();
        let rp = c.repo.to_string_lossy().into_owned();
        let r = self.r.main.run(&sv(&[&format!("{h}/check_smoke_model.sh"), &c.source_cell, "--repo", &rp, "--topology", TOPOLOGY, "--emit-gate-params"]), 600, false)?;
        let text = format!("{}{}", r.out, r.err);
        if r.rc == 7 {
            return serve("로드-전 RAM 게이트 거부(메인) — 모델 실재·가용 RAM 부족(다운로드 불요)");
        }
        if r.rc == 2 {
            return serve("스모크 모델 부재 — 다운로드 금지, 중단");
        }
        if r.rc != 0 {
            return serve(format!("NAS/설정 확인 실패(rc={}): {}", r.rc, tail_chars(&text, 400)));
        }
        let bp = find_after(&text, "BUDGET_PARAMS ckpt_mib=");
        let req = find_after(&text, "GATE_PARAMS required_mib=");
        let (bp, req) = match (bp, req) {
            (Some(b), Some(q)) => (b, q),
            _ => return serve("BUDGET_PARAMS/GATE_PARAMS 미검출 — 예산 선언 입력을 파생할 수 없다(진입 차단)"),
        };
        let digits = |s: &str| s.chars().take_while(char::is_ascii_digit).collect::<String>();
        let ckpt_s = digits(&bp);
        let after = &bp[ckpt_s.len()..];
        let tp_s = after.strip_prefix(" tp=").map(digits);
        let after_tp = tp_s.as_ref().map(|t| &after[4 + t.len()..]);
        let kv_s = after_tp.and_then(|a| a.strip_prefix(" kv_mib=")).map(|a| a.split_whitespace().next().unwrap_or("").to_string());
        let ple_s = after_tp.and_then(|a| a.split(" ple_mib=").nth(1)).map(|a| a.split_whitespace().next().unwrap_or("").to_string());
        let (ckpt, tp, kv, ple, required) = match (ckpt_s.parse::<i64>(), tp_s.and_then(|t| t.parse::<i64>().ok()), kv_s, ple_s, digits(&req).parse::<i64>()) {
            (Ok(a), Some(b), Some(k), Some(p), Ok(q)) if !digits(&bp).is_empty() => (a, b, k, p, q),
            _ => return serve("BUDGET_PARAMS/GATE_PARAMS 미검출 — 예산 선언 입력을 파생할 수 없다(진입 차단)"),
        };
        if kv.is_empty() || !kv.chars().all(|ch| ch.is_ascii_digit()) || tp <= 0 {
            return serve("kv_mib/tp 파생 실패 — 트리플렛 kv-cache-memory-bytes 확인");
        }
        let mut avail = self.meminfo("sub")?.1;
        if avail < required {
            let dc = "/usr/local/sbin/vllm-drop-caches";
            if self.r.sub.run(&sv(&["test", "-x", dc]), 15, false)?.rc == 0 {
                self.r.sub.run(&sv(&["sudo", "-n", dc]), 60, false)?;
            }
            avail = self.meminfo("sub")?.1;
        }
        if avail < required {
            return serve(format!("슬레이브 로드-전 RAM 게이트 거부 — MemAvailable={avail}MiB < required={required}MiB"));
        }
        (self.log)(&format!("[native] RAM 게이트 PASS: main(check_smoke_model) · sub MemAvailable={avail}MiB ≥ {required}MiB"));
        let (mut resident, mut note) = (ckpt, "ple=resident".to_string());
        if c.cell_env.get("VLLM_PLE_MMAP").map(|v| v.trim()) == Some("1") {
            if !ple.is_empty() && ple.chars().all(|ch| ch.is_ascii_digit()) && ple.parse::<i64>().unwrap_or(0) > 0 {
                let p: i64 = ple.parse().unwrap_or(0);
                resident = ckpt - p;
                note = format!("ple=mmap(−{ple}MiB)");
            } else {
                note = "ple=mmap선언·크기미상(보정 0)".into();
            }
        }
        let weights = resident.div_euclid(tp);
        let floor = self.r.main.run(&sv(&[super::session_interpreter(&c.main.session_py), &c.main.session_py, "--node-dir", ".", "budget-defaults", "--field", "ttl_s"]), 30, true)?.out.trim().to_string();
        if floor.is_empty() || !floor.chars().all(|ch| ch.is_ascii_digit()) {
            return serve("예산 TTL 기본값을 blackbox_session 에서 읽지 못했다");
        }
        let ttl = std::cmp::min(std::cmp::max(c.ready_max_s * 3, floor.parse().unwrap_or(0)), 86400);
        let mem_main = self.meminfo("main")?.0;
        let pf = self.r.main.run(
            &sv(&[&format!("{h}/budget_preflight.sh"), "--json", "--declared-gmu-yaml", &c.repo.join(&c.triplet["yaml"]).to_string_lossy(), "--mem-total-mib", &mem_main.to_string(), "--weights-mib", &weights.to_string(), "--kv-mib", &kv, "--overhead-mib", &c.overhead_mib.to_string()]),
            60,
            false,
        )?;
        if pf.rc == 4 {
            return serve(format!("예산 선판정: 워치독이 이 선언을 거부한다(arm 상한 < 최소) — {}", tail_chars(&pf.out, 300)));
        }
        if pf.rc != 0 {
            return serve(format!("예산 선판정 실패(rc={})", pf.rc));
        }
        let kv_i: i64 = kv.parse().unwrap_or(0);
        (self.log)(&format!("[native] 예산 파생: weights={weights}MiB({note}) kv={kv}MiB overhead={}MiB ttl={ttl}s", c.overhead_mib));
        for name in ["main", "sub"] {
            self.declare(name, weights, kv_i, ttl)?;
        }
        Ok((weights, kv_i, ttl, note))
    }

    fn preflight_tools(&self) -> R<()> {
        for name in ["main", "sub"] {
            let spec = self.c.node(name);
            let r = self.r.get(name);
            for path in [&spec.session_py, &spec.renew_sh, &spec.watchdog_sh] {
                if r.run(&sv(&["test", "-f", path]), 15, false)?.rc != 0 {
                    return serve(format!("{name}: 필수 도구 부재: {path} (서브는 sync_to_sub 배달 필요)"));
                }
            }
            for path in [&spec.renew_sh, &spec.watchdog_sh] {
                if r.run(&sv(&["grep", "-q", "--", "--pgid", path]), 15, false)?.rc != 0 {
                    return serve(format!("{name}: {path} 에 pgid 표적 모드가 없다(커밋 65bdaec 미배달)"));
                }
            }
        }
        if !self.c.repo.join(&self.c.wheel_tool_rel).is_file() {
            return serve(format!("wheelhouse 도구 부재: {}", self.c.wheel_tool_rel));
        }
        let elf = wheel_elf_path(&self.c.repo);
        if !std::fs::read(&elf).is_ok_and(|b| b.starts_with(b"\x7fELF")) {
            return serve(format!("wheelhouse Rust ELF 부재/무효: {} — 메인에서 upstream runtime 을 release 빌드한다(Python 폴백 ✗)", elf.display()));
        }
        if !self.c.repo.join(&self.c.wheel_accept_rel).is_file() {
            return serve(format!("wheelhouse 승인 선언 부재: {}", self.c.wheel_accept_rel));
        }
        Ok(())
    }

    /// `doc_naming.simlog_dirname` through the TypeScript owner (`node doc_naming.ts simlog_dirname '<json args>'`) — no Python.
    fn simlog_dirname(&self, generated_utc: &str, topic: &str, existing: &[String]) -> R<String> {
        let src_root = self.here.ancestors().nth(4).map(Path::to_path_buf).unwrap_or_else(|| self.c.repo.clone());
        let ts = src_root.join(".claude/skills/wiki-desk/src/doc_naming.ts");
        let mut ex: Vec<&String> = existing.iter().collect();
        ex.sort();
        let args = PyValue::List(vec![vs(generated_utc), vs(topic), PyValue::List(ex.iter().map(|s| vs(s)).collect())]);
        let json = args.dumps_unicode().ok().and_then(|s| s.to_utf8()).unwrap_or_default();
        let out = Command::new("node").arg(&ts).arg("simlog_dirname").arg(json).output().map_err(|e| Err::Serve(format!("doc_naming: node 실행 실패: {e}")))?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let doc = agent_runtime_support::pyjson::loads(&text).map_err(|_| Err::Serve(format!("doc_naming.ts failed (exit {:?}): {}", out.status.code(), String::from_utf8_lossy(&out.stderr).trim())))?;
        if matches!(dget(&doc, "ok"), Some(PyValue::Bool(true))) {
            return as_str(dget(&doc, "value")).ok_or_else(|| Err::Serve("doc_naming: value 없음".into()));
        }
        serve(py_str(dget(&doc, "message")))
    }

    fn make_roots(&mut self, generated_utc: &str) -> R<()> {
        let c = self.c;
        let simlog = c.repo.join("docs/simlog");
        let existing: Vec<String> = match std::fs::read_dir(&simlog) {
            Ok(rd) => rd.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect(),
            Err(_) => Vec::new(),
        };
        let preserve_rel = format!("docs/simlog/{}", self.simlog_dirname(generated_utc, &c.simlog_topic, &existing)?);
        self.state = dict(vec![
            ("schema_version", vi(1)),
            ("kind", vs("native_multinode_state")),
            ("run_id", vs(&c.run_id)),
            ("cell", vs(&c.cell)),
            ("campaign_id", vs(&c.campaign_id)),
            ("created_utc", vs(generated_utc)),
            ("sub_host", vs(&c.sub_host)),
            ("nodes", dict(vec![("main", c.main.row()), ("sub", c.sub.row())])),
            ("ports", dict(vec![("serve", vi(c.serve_port)), ("ray", vi(c.ray_port))])),
            ("preserve_rel", vs(&preserve_rel)),
            ("proof_rel", vs(&c.proof_rel)),
            ("attestation_rel", vs(&format!("{preserve_rel}/cleanup_attestation.json"))),
            ("budget_declared", self.budget_v()),
            ("roots_created", PyValue::Dict(PyDict::new())),
            ("identities", PyValue::List(Vec::new())),
        ]);
        for name in ["main", "sub"] {
            for (kind, runner) in [("run", self.r.get(name)), ("short", self.rs.get(name))] {
                runner.init_root(true)?;
                if let PyValue::Dict(d) = &mut self.state {
                    if let Some(PyValue::Dict(rc)) = d.get_mut("roots_created") {
                        if !rc.contains_key(name) {
                            rc.set(name, PyValue::List(Vec::new()));
                        }
                        if let Some(PyValue::List(l)) = rc.get_mut(name) {
                            l.push(vs(kind));
                        }
                    }
                }
                if name == "main" && kind == "run" {
                    self.state_write()?;
                }
            }
        }
        self.state_write()?;
        let pdir = c.repo.join(&preserve_rel);
        if let Some(par) = pdir.parent() {
            std::fs::create_dir_all(par).map_err(|e| Err::Serve(e.to_string()))?;
        }
        std::fs::create_dir(&pdir).map_err(|e| Err::Serve(e.to_string()))?;
        for name in ["main", "sub"] {
            self.r.get(name).run(&mkdirs_argv(c.node(name)), 30, true)?;
        }
        Ok(())
    }
    fn budget_v(&self) -> PyValue {
        dict(vec![("main", PyValue::Bool(self.budget_declared[0])), ("sub", PyValue::Bool(self.budget_declared[1]))])
    }

    fn du_guard(&mut self, name: &str, stage: &str) -> R<()> {
        let out = self.r.get(name).run(&sv(&["du", "-sxb", &self.c.node(name).root]), 600, true)?.out;
        let first = out.split_whitespace().next().unwrap_or("");
        let used: i64 = if !first.is_empty() && first.chars().all(|c| c.is_ascii_digit()) { first.parse().unwrap_or(-1) } else { -1 };
        if used < 0 {
            return serve(format!("{name}: run root 크기 측정 실패({stage})"));
        }
        if used as u64 > self.c.max_bytes {
            return serve(format!("{name}: run root {:.1}GiB > 상한 {:.0}GiB ({stage})", used as f64 / (1u64 << 30) as f64, self.c.max_bytes as f64 / (1u64 << 30) as f64));
        }
        if let PyValue::Dict(d) = &mut self.state {
            if !d.contains_key("root_bytes") {
                d.set("root_bytes", PyValue::Dict(PyDict::new()));
            }
            if let Some(PyValue::Dict(rb)) = d.get_mut("root_bytes") {
                if !rb.contains_key(name) {
                    rb.set(name, PyValue::Dict(PyDict::new()));
                }
                if let Some(PyValue::Dict(n)) = rb.get_mut(name) {
                    n.set(stage, vi(used));
                }
            }
        }
        Ok(())
    }

    fn install(&mut self, generated_utc: &str) -> R<BTreeMap<String, Env>> {
        let c = self.c;
        let tool_bytes = std::fs::read(wheel_elf_path(&c.repo)).map_err(|e| Err::Serve(e.to_string()))?;
        let tool_sha = sha_hex(&tool_bytes);
        self.r.sub.put(SUB_WHEEL_TOOL, &tool_bytes, 0o700)?;
        state_set(&mut self.state, "wheel_tool_sha256", vs(&tool_sha));
        let accept_bytes = std::fs::read(c.repo.join(&c.wheel_accept_rel)).map_err(|e| Err::Serve(e.to_string()))?;
        let accept_sha = sha_hex(&accept_bytes);
        self.r.sub.put("tools/native_wheelhouse_accept.json", &accept_bytes, 0o600)?;
        state_set(&mut self.state, "wheel_accept_sha256", vs(&accept_sha));
        let mut lds: BTreeMap<String, Env> = BTreeMap::new();
        for name in ["main", "sub"] {
            let spec = c.node(name);
            let r = self.r.get(name);
            (self.log)(&format!("[native] {name}: wheelhouse 재포장(자기 이미지 {})…", c.image));
            let b = r.run(&wheelhouse_build_argv(&spec.wheel_tool, &c.image, &spec.root, generated_utc, &spec.wheel_accept), 7200, false)?;
            self.preserve(&format!("wheelhouse-build-{name}.json"), if b.out.is_empty() { b.err.as_bytes() } else { b.out.as_bytes() }, false)?;
            if b.rc != 0 {
                return serve(format!("{name}: wheelhouse 재포장 불통과(rc={}) — 다운로드 폴백 ✗(O-N2): {}", b.rc, tail_chars(&b.err, 400)));
            }
            self.du_guard(name, "wheelhouse")?;
            let v = r.run(&wheelhouse_verify_argv(&spec.wheel_tool, &spec.root, generated_utc, &spec.wheel_accept), 3600, false)?;
            self.preserve(&format!("wheelhouse-verify-{name}.json"), if v.out.is_empty() { v.err.as_bytes() } else { v.out.as_bytes() }, false)?;
            self.du_guard(name, "venv")?;
            let (ok, venv_env, why) = parse_verify_verdict(&v.out);
            if v.rc != 0 || !ok {
                return serve(format!("{name}: 설치 게이트 불통과(rc={}): {why}", v.rc));
            }
            lds.insert(name.to_string(), venv_env);
            let freeze = r.run(&pip_freeze_argv(&spec.root), 120, true)?.out;
            let rel = self.preserve(&format!("pip-freeze-{name}.txt"), freeze.as_bytes(), false)?;
            state_sub_set(&mut self.state, "pip_freeze_rel", name, vs(&rel));
        }
        let script = native_install_script(c, &tool_sha, generated_utc, &accept_sha);
        let rel = self.preserve("native-install.sh", script.as_bytes(), false)?;
        state_set(&mut self.state, "native_install_rel", vs(&rel));
        let mut recipe = PyDict::new();
        for (n, e) in &lds {
            let mut d = PyDict::new();
            for (k, v) in e {
                d.set(k, vs(v));
            }
            recipe.set(n, PyValue::Dict(d));
        }
        state_set(&mut self.state, "runtime_env_recipe", PyValue::Dict(recipe));
        self.r.main.put(CLIENT_REL, client_wrapper_text(&runtime_env(c, &c.main, &lds["main"]), &c.main).as_bytes(), 0o755)?;
        state_set(&mut self.state, "client_vllm", vs(&format!("{}/{CLIENT_REL}", c.main.root)));
        (self.log)(&format!("[native] 벤치 클라이언트 래퍼 → {}/{CLIENT_REL} (--client-vllm 로 넘긴다)", c.main.root));
        self.state_write()?;
        Ok(lds)
    }

    fn arm_watchdog(&mut self, name: &str, target: &Identity) -> R<()> {
        let spec = self.c.node(name);
        let argv = watchdog_argv(spec, &target.pgid.to_string(), &target.starttime);
        let t = self.clock.mono();
        let wd = self.r.get(name).start(&argv, &format!("logs/watchdog-{}.log", target.role), &format!("watchdog-{}", target.role))?;
        self.add_ident(&wd, "watchdog", Some(&target.role))?;
        self.launch_step(
            name,
            &format!("watchdog({})", target.role),
            None,
            None,
            &argv,
            &argv,
            t,
            Step { log_rel: Some(wd.log_rel.clone()), note: Some("안전 부속 — host_safety mem_watchdog pgid 모드(pid·pgid·starttime 은 이 run 의 값)".into()), ..Step::default() },
        )
    }

    fn start_cluster(&mut self, lds: &BTreeMap<String, Env>) -> R<Identity> {
        let c = self.c;
        let (m, s) = (&c.main, &c.sub);
        let (env_m, env_s) = (runtime_env(c, m, &lds["main"]), runtime_env(c, s, &lds["sub"]));
        let cmd = ray_head_argv(c, m);
        let mut t = self.clock.mono();
        let argv = env_argv(&env_m, &cmd);
        let head = self.r.main.start(&argv, "logs/ray-head.log", "ray-head")?;
        self.add_ident(&head, "serve", None)?;
        self.launch_step("main", "ray-head", Some("main"), Some(&env_m), &cmd, &argv, t, Step { log_rel: Some(head.log_rel.clone()), ..Step::default() })?;
        self.arm_watchdog("main", &head)?;
        let cmd = ray_worker_argv(c, s);
        t = self.clock.mono();
        let argv = env_argv(&env_s, &cmd);
        let worker = self.r.sub.start(&argv, "logs/ray-worker.log", "ray-worker")?;
        self.add_ident(&worker, "serve", None)?;
        self.launch_step("sub", "ray-worker(join)", Some("sub"), Some(&env_s), &cmd, &argv, t, Step { log_rel: Some(worker.log_rel.clone()), ..Step::default() })?;
        self.arm_watchdog("sub", &worker)?;
        let cmd = ray_status_argv(c, m);
        t = self.clock.mono();
        let argv = env_argv(&env_m, &cmd);
        let end = t + RAY_JOIN_MAX_S as f64;
        let mut polls = 0i64;
        let hit: String;
        loop {
            let st = self.r.main.run(&argv, 60, false)?;
            polls += 1;
            if let Some(l) = st.out.lines().find(|l| l.contains("/2.0 GPU")) {
                hit = l.to_string();
                break;
            }
            if self.clock.mono() >= end {
                self.launch_step(
                    "main",
                    "gpu-join(ray status)",
                    Some("main"),
                    Some(&env_m),
                    &cmd,
                    &argv,
                    t,
                    Step { polls: Some(polls), matched: Some(None), elapsed_s: Some(round1(self.clock.mono() - t)), note: Some(format!("GPU 2 미관측 · 창 {RAY_JOIN_MAX_S}s 소진")), ..Step::default() },
                )?;
                return serve(format!("Ray worker 가 {RAY_JOIN_MAX_S}s 안에 합류하지 않았다(GPU 2 미관측)"));
            }
            self.clock.sleep(HEALTH_INTERVAL_S as f64);
        }
        self.launch_step(
            "main",
            "gpu-join(ray status)",
            Some("main"),
            Some(&env_m),
            &cmd,
            &argv,
            t,
            Step { polls: Some(polls), matched: Some(Some(hit.trim().to_string())), elapsed_s: Some(round1(self.clock.mono() - t)), note: Some(format!("`/2.0 GPU` 관측까지 {HEALTH_INTERVAL_S}s 간격 폴링(상한 {RAY_JOIN_MAX_S}s)")), ..Step::default() },
        )?;
        (self.log)("[native] Ray 클러스터 GPU 2 관측 — vllm serve 기동");
        let mut env = env_m.clone();
        env.insert("RAY_ADDRESS".into(), format!("{}:{}", m.host_ip, c.ray_port));
        let cmd = serve_argv(c);
        t = self.clock.mono();
        let argv = env_argv(&env, &cmd);
        let serve_id = self.r.main.start(&argv, "logs/vllm-serve.log", "vllm-serve")?;
        self.add_ident(&serve_id, "serve", None)?;
        self.launch_step(
            "main",
            "vllm-serve",
            Some("main-serve"),
            Some(&env),
            &cmd,
            &argv,
            t,
            Step { log_rel: Some(serve_id.log_rel.clone()), note: Some("러너 .sh 가 `exec <venv>/bin/vllm serve --config … --served-model-name …` 한다(아래 §vllm serve 실효 인자)".into()), ..Step::default() },
        )?;
        self.arm_watchdog("main", &serve_id)?;
        Ok(serve_id)
    }

    fn starttime_of(&self, runner: &dyn Runner, ident: &Identity) -> bool {
        match runner.tree(&[ident.pid], &[]) {
            Err(_) => true,
            Ok(rows) => rows.iter().any(|r| r.pid == ident.pid && r.starttime == ident.starttime),
        }
    }

    fn observe_serve_exec(&self, serve_id: &Identity) -> ServeExec {
        if !self.starttime_of(self.r.main.as_ref(), serve_id) {
            return ServeExec::Not("리더 starttime 불일치/부재".into());
        }
        match self.r.main.run(&sv(&["cat", &format!("/proc/{}/cmdline", serve_id.pid)]), 15, false) {
            Err(e) => ServeExec::Not(format!("cmdline 읽기 실패 rc={}", e.msg())),
            Ok(r) => {
                let argv: Vec<String> = r.out.split('\0').filter(|x| !x.is_empty()).map(String::from).collect();
                if r.rc != 0 || argv.is_empty() {
                    ServeExec::Not(format!("cmdline 읽기 실패 rc={}", r.rc))
                } else {
                    ServeExec::Observed(argv)
                }
            }
        }
    }

    fn health_and_smoke(&mut self, serve_id: &Identity) -> R<Obs> {
        let c = self.c;
        let base = format!("http://127.0.0.1:{}", c.serve_port);
        let t0 = self.clock.mono();
        let (mut last, mut ready) = (0i64, false);
        while self.clock.mono() - t0 <= c.ready_max_s as f64 {
            last = (self.http)("GET", &format!("{base}/health"), None, 5).0;
            if last == 200 {
                ready = true;
                break;
            }
            if !self.starttime_of(self.r.main.as_ref(), serve_id) {
                return serve("vllm serve 가 준비 전에 종료됐다(logs/vllm-serve.log)");
            }
            self.clock.sleep(HEALTH_INTERVAL_S as f64);
        }
        let waited = (self.clock.mono() - t0).floor() as i64;
        let mut obs = Obs { present: true, health_ok: ready, health_http: last, ready_after_s: ready.then_some(waited), window_s: c.ready_max_s, ..Obs::default() };
        if !ready {
            return Ok(obs);
        }
        let msg = dict(vec![("role", vs("user")), ("content", vs(SMOKE_PROMPT))]);
        let body = dict(vec![("model", vs(&c.model_name)), ("messages", PyValue::List(vec![msg])), ("max_tokens", vi(256))]);
        let t_inf = self.clock.mono();
        let (code, text) = (self.http)("POST", &format!("{base}/v1/chat/completions"), Some(&body), 120);
        let (mut content, mut reasoning, mut fr) = (String::new(), String::new(), None::<String>);
        if let Ok(doc) = agent_runtime_support::pyjson::loads(&text) {
            if let Some(ch) = list_items(dget(&doc, "choices")).first() {
                if let Some(msg) = dget(ch, "message") {
                    content = as_str(dget(msg, "content")).unwrap_or_default().trim().to_string();
                    let r1 = as_str(dget(msg, "reasoning")).filter(|s| !s.is_empty());
                    reasoning = r1.or_else(|| as_str(dget(msg, "reasoning_content"))).unwrap_or_default().trim().to_string();
                }
                fr = as_str(dget(ch, "finish_reason"));
            }
        }
        obs.inf_attempted = true;
        obs.chat_len = content.chars().count() as i64;
        obs.reasoning_len = reasoning.chars().count() as i64;
        obs.finish_reason = fr;
        obs.inf_path = Some("/v1/chat/completions");
        obs.inf_http = code;
        if code == 200 && !content.is_empty() {
            obs.inf_ok = true;
            obs.inf_evidence = Some("chat.content");
            obs.inf_len = obs.chat_len;
        } else if code == 200 && !reasoning.is_empty() {
            let b2 = dict(vec![("model", vs(&c.model_name)), ("prompt", vs("2+2=")), ("max_tokens", vi(32))]);
            let (c2, t2) = (self.http)("POST", &format!("{base}/v1/completions"), Some(&b2), 120);
            let mut n = 0i64;
            if let Ok(doc) = agent_runtime_support::pyjson::loads(&t2) {
                if let Some(ch) = list_items(dget(&doc, "choices")).first() {
                    n = as_str(dget(ch, "text")).unwrap_or_default().trim().chars().count() as i64;
                }
            }
            obs.inf_path = Some("/v1/completions");
            obs.inf_http = c2;
            obs.inf_ok = c2 == 200 && n > 0;
            obs.inf_evidence = Some("v1.completions");
            obs.inf_len = n;
        }
        self.launch.inference_elapsed_s = Some(round1(self.clock.mono() - t_inf));
        Ok(obs)
    }

    fn write_proof(&mut self, status: &str, obs: &Obs, stage: &str, reason: &str) -> R<()> {
        let st = &self.state;
        let opt_s = |k: &str| as_str(state_get(st, k)).map_or(PyValue::None, |s| vs(&s));
        let mut doc = dict(vec![
            ("schema_version", vi(1)),
            ("kind", vs("native_multinode_serve_proof")),
            ("plane", vs("native")),
            ("topology", vs(TOPOLOGY)),
            ("status", vs(status)),
            ("cell", vs(&self.c.cell)),
            ("config", vs(&self.c.cell)),
            ("run_id", vs(&self.c.run_id)),
            ("source_cell", vs(&self.c.source_cell)),
            ("campaign_id", vs(&self.c.campaign_id)),
            ("health", obs.health_v()),
            ("inference", obs.inference_v()),
            ("endpoints", obs.endpoints_v()),
            ("native_install_path", opt_s("native_install_rel")),
            ("pip_freeze_paths", state_get(st, "pip_freeze_rel").cloned().unwrap_or(PyValue::None)),
            ("cleanup_attestation_path", opt_s("attestation_rel")),
            ("evidence_dir", opt_s("preserve_rel")),
            ("native_launch_path", opt_s("native_launch_rel")),
            (
                "wheelhouse",
                dict(vec![
                    ("source_image_tag", vs(&self.c.image)),
                    ("tool_sha256", opt_s("wheel_tool_sha256")),
                    ("accept_sha256", opt_s("wheel_accept_sha256")),
                    ("note", vs("각 노드가 자기 로컬 이미지에서 재포장(이미지 전송 ✗)")),
                ]),
            ),
            ("provenance", vs("measured(native_multinode_serve.py up · health GET + 추론 POST 관측)")),
        ]);
        if let PyValue::Dict(d) = &mut doc {
            if !obs.present {
                d.set("health", dict(vec![("ok", PyValue::Bool(false))]));
                d.set("inference", dict(vec![("ok", PyValue::Bool(false))]));
                d.set("endpoints", PyValue::Dict(PyDict::new()));
            }
            if truthy(state_get(st, "native_launch_error")) {
                d.set("native_launch_error", state_get(st, "native_launch_error").cloned().unwrap_or(PyValue::None));
            }
            if status != "PASS" {
                d.set("failed_stage", vs(stage));
                d.set("reason", vs(&tail_chars(reason, 600)));
            }
        }
        let proof = self.c.proof_rel.clone();
        self.write_repo_json(&proof, &doc)
    }

    fn arm_renew(&mut self, serve_main: &Identity, ttl: i64) -> R<()> {
        let rows: Vec<PyValue> = list_items(state_get(&self.state, "identities")).into_iter().filter(|r| as_str(dget(r, "kind")).as_deref() == Some("serve")).cloned().collect();
        let mut worker: Option<Identity> = None;
        for row in &rows {
            if as_str(dget(row, "role")).as_deref() == Some("ray-worker") {
                worker = Some(Identity::from_row(row)?);
            }
        }
        let worker = worker.ok_or_else(|| Err::Serve("ray-worker identity".into()))?;
        for (name, target) in [("main", serve_main), ("sub", &worker)] {
            let argv = renew_argv(self.c.node(name), &target.pgid.to_string(), &target.starttime, ttl);
            let rn = self.r.get(name).start(&argv, &format!("logs/budget-renew-{name}.log"), &format!("budget-renew-{name}"))?;
            self.add_ident(&rn, "renew", Some(&target.role))?;
        }
        Ok(())
    }

    pub fn up(&mut self, generated_utc: &str) -> i32 {
        let c = self.c;
        (self.log)(&format!("[native] up cell={} run={} source={} image={}", c.cell, c.run_id, c.source_cell, c.image));
        self.launch.t0 = Some(self.clock.mono());
        self.launch.generated_utc = generated_utc.into();
        let mut stage = "preflight";
        let mut obs = Obs::default();
        let res: R<()> = (|| {
            self.preflight_tools()?;
            stage = "gates+budget";
            let (_, _, ttl, _) = self.gates_and_budget()?;
            stage = "run-roots";
            self.make_roots(generated_utc)?;
            let bd = self.budget_v();
            state_set(&mut self.state, "budget_declared", bd);
            state_set(&mut self.state, "budget_ttl_s", vi(ttl));
            self.state_write()?;
            stage = "install";
            let lds = self.install(generated_utc)?;
            stage = "start";
            let serve_id = self.start_cluster(&lds)?;
            stage = "health";
            obs = self.health_and_smoke(&serve_id)?;
            if !obs.health_ok {
                return serve(format!("health 200 미도달({}s 창 · 마지막 http={})", c.ready_max_s, obs.health_http));
            }
            if !obs.inf_ok {
                return serve(format!("추론 1회 관측 실패: {}", py_str(Some(&obs.inference_v()))));
            }
            self.launch.serve_exec = Some(self.observe_serve_exec(&serve_id));
            let o = obs.clone();
            self.write_launch("PASS", &o, "", "");
            self.write_proof("PASS", &o, "", "")?;
            stage = "renew";
            self.arm_renew(&serve_id, ttl)?;
            state_set(&mut self.state, "up_completed_utc", vs(&utc(self.clock.now())));
            self.state_write()?;
            (self.log)(&format!("[native] serve proof → {} (PASS) · 서빙 유지 — 회수: native_multinode_serve.py down --cell {} --run-id {} --apply", c.proof_rel, c.cell, c.run_id));
            Ok(())
        })();
        match res {
            Ok(()) => EXIT_OK,
            Err(e) => {
                (self.log)(&format!("[native] FAIL stage={stage}: {}: {}", if matches!(e, Err::Unknown(_)) { "UnknownState" } else { "ServeError" }, e.msg()));
                let reason = e.msg().to_string();
                let o = obs.clone();
                self.write_launch("FAIL", &o, stage, &reason);
                let _ = self.write_proof("FAIL", &o, stage, &reason);
                if truthy(state_get(&self.state, "roots_created")) && matches!(dget(&self.state, "roots_created").map(PyValue::peel), Some(PyValue::Dict(d)) if !d.is_empty()) {
                    (self.log)("[native] run root 가 있다 — 자동 down(AC-N5)");
                    let st = self.state.clone();
                    let att = down_from_state(&st, &c.repo, self.r, self.rs, self.clock, self.log);
                    return if as_str(dget(&att, "status")).as_deref() == Some("PASS") { EXIT_FAIL } else { EXIT_UNKNOWN };
                }
                let errs: Vec<String> = ["main", "sub"].iter().filter(|n| self.budget_declared[node_rel(n)]).filter_map(|n| self.clear_budget(n)).collect();
                for e in &errs {
                    (self.log)(&format!("[native] ⚠ {e}"));
                }
                if errs.is_empty() {
                    EXIT_FAIL
                } else {
                    EXIT_UNKNOWN
                }
            }
        }
    }
}

fn parse_utc(s: &str) -> Option<f64> {
    // %Y-%m-%dT%H:%M:%SZ
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some((days * 86400 + h * 3600 + mi * 60 + se) as f64)
}

/// `native_launch_markdown` — the record before substitution (input: the launch log).
pub fn native_launch_markdown(c: &Ctx, launch: &Launch, obs: &Obs, status: &str, stage: &str, reason: &str, recorded_utc: &str) -> String {
    let fence = |body: &str, lang: &str| format!("```{lang}\n{}\n```", body.trim_end());
    let ph_base = "<campaign.native_run_root_base>";
    let mut l: Vec<String> = vec![
        format!("# native 기동 기록 — {}", c.cell),
        String::new(),
        format!("- run-id: `{}` · 캠페인: `{}` · 원본 Docker 셀: `{}` · 원천 이미지: `{}`", c.run_id, c.campaign_id, c.source_cell, c.image),
        format!("- 상태: **{status}**{}", if status != "PASS" { format!(" (failed_stage={stage} · {})", tail_chars(reason, 300)) } else { String::new() }),
        format!("- generated_utc(up 시작): {} · recorded_utc(이 기록): {recorded_utc}", launch.generated_utc),
        "- provenance: measured — `native_multinode_serve.py up` 이 각 프로세스를 띄우기 **직전에** 넘긴 argv·env 를 그대로 적었다(로그 재구성 ✗). 설치 절차는 serve proof `native_install_path`(native-install.sh)가 따로 든다.".into(),
        format!("- 치환: 운영자 절대경로·노드 IP·호스트·NIC·사람 식별자는 발행기 치환표(`hintlib.pii.substitution_table` · manifest 사실)로 `<manifest.<field>>`·`<node:main|sub>`·`<repo>`·`<nic:…>` 자리표시가 됐다. 캠페인 run root base 는 `{ph_base}`. 표가 놓친 4종 패턴 잔여는 `<abs-path>`·`<priv-ip>` 등으로 가려졌다."),
        "- 재현 규칙: 각 단계의 실제 argv = `[\"env\", \"-i\", *sorted(\"K=V\" for ENV[<블록>]), *cmd]` (env 블록이 `-` 인 단계는 up 프로세스 환경 상속). 단계의 `argv_sha256` 은 **치환 전** 실제 argv(JSON 직렬화)의 해시다.".into(),
        String::new(),
        "## 포트".into(),
        String::new(),
        "| 용도 | 노드 | 주소 |".into(),
        "|---|---|---|".into(),
        format!("| vLLM OpenAI 서버 | main | port `{}` (셀 env SERVING_PORT = yaml port · 바인드 host 는 아래 yaml · 판정은 `http://127.0.0.1:{}`) |", c.serve_port, c.serve_port),
        format!("| Ray GCS | main | `{}:{}` |", c.main.host_ip, c.ray_port),
        String::new(),
        "## 실행 순서".into(),
        String::new(),
        "| # | 노드 | 단계 | env | 시작(up+s) | 소요(s) | 로그 |".into(),
        "|---|---|---|---|---|---|---|".into(),
    ];
    for (i, s) in launch.steps.iter().enumerate() {
        l.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            i + 1,
            s.node,
            s.step,
            s.env.as_deref().unwrap_or("-"),
            fl(s.t_rel_s),
            s.elapsed_s.map_or("-".into(), fl),
            s.log_rel.as_deref().unwrap_or("-")
        ));
    }
    l.push(String::new());
    l.push("## 단계별 argv".into());
    l.push(String::new());
    for (i, s) in launch.steps.iter().enumerate() {
        l.push(format!("### {}. [{}] {}", i + 1, s.node, s.step));
        if let Some(n) = &s.note {
            l.push(format!("- {n}"));
        }
        let envtxt = match &s.env {
            Some(e) => format!("ENV[{e}]"),
            None => "상속(env -i 아님)".into(),
        };
        l.push(format!("- env: `{envtxt}` · argv_sha256: `{}`", s.argv_sha256));
        if let Some(p) = s.polls {
            l.push(format!("- polls: `{p}`"));
        }
        if let Some(m) = &s.matched {
            l.push(format!("- matched: `{}`", m.clone().unwrap_or_else(|| "None".into())));
        }
        l.push(fence(&shlex_join(&s.cmd), "bash"));
        l.push(String::new());
    }
    l.push("## 노드별 env 형상 (`env -i` 로 넘긴 환경 전부 · 키 정렬)".into());
    l.push(String::new());
    for (name, env) in &launch.envs {
        l.push(format!("### ENV[{name}] — {} 키", env.len()));
        l.push(String::new());
        l.push(fence(&env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n"), ""));
        l.push(String::new());
    }
    l.push("## vllm serve 실효 인자".into());
    l.push(String::new());
    match &launch.serve_exec {
        Some(ServeExec::Observed(argv)) => {
            l.push("- 관측: /proc/<pid:vllm-serve>/cmdline(starttime 일치 확인 뒤 · health 200 이후)".into());
            l.push(fence(&shlex_join(argv), "bash"));
        }
        Some(ServeExec::Not(why)) => l.push(format!("- 관측 안 됨: {why}")),
        None => l.push("- 관측 안 됨: 기동 전 종료/미도달".into()),
    }
    for key in ["yaml", "sh"] {
        let rel = &c.triplet[key];
        match std::fs::read(c.repo.join(rel)) {
            Err(_) => l.push(format!("- `{rel}` 읽기 실패")),
            Ok(b) => {
                l.push(format!("- `{rel}` sha256=`{}`", sha_hex(&b)));
                l.push(fence(&String::from_utf8_lossy(&b), if key == "yaml" { "yaml" } else { "bash" }));
            }
        }
    }
    let inf_ep_path = if obs.inf_attempted { obs.inf_path.unwrap_or("None").to_string() } else { "None".into() };
    let inf_http = if obs.inf_attempted { obs.inf_http.to_string() } else { "None".into() };
    let none_or = |o: Option<String>| o.unwrap_or_else(|| "None".into());
    l.push(String::new());
    l.push("## 성공 판정".into());
    l.push(String::new());
    l.push("| 판정 | 결과 | 근거 |".into());
    l.push("|---|---|---|".into());
    l.push(format!(
        "| health | {} | GET /health http={} · ready_after_s={} · (vllm serve 기동 뒤 · 창 {}s · 폴링 {HEALTH_INTERVAL_S}s) |",
        if obs.health_ok { "PASS" } else { "FAIL" },
        obs.health_http,
        none_or(obs.ready_after_s.map(|x| x.to_string())),
        obs.window_s
    ));
    l.push(format!(
        "| 추론 1회 | {} | {inf_ep_path} http={inf_http} · evidence={} · completion_text_len={} · finish_reason={} · elapsed_s={} |",
        if obs.inf_ok { "PASS" } else { "FAIL" },
        none_or(obs.inf_evidence.map(String::from)),
        if obs.inf_attempted { obs.inf_len.to_string() } else { "None".into() },
        none_or(obs.finish_reason.clone()),
        none_or(launch.inference_elapsed_s.map(fl))
    ));
    l.push(String::new());
    l.push(format!("- 총 소요(up 시작 → 판정): {}s", none_or(launch.total_elapsed_s.map(fl))));
    l.push(String::new());
    l.join("\n")
}

/// `native_install_script` — the reproduction command text (the same builders called with `${RUN_ROOT}`/`${IMAGE}` placeholders).
pub fn native_install_script(c: &Ctx, tool_sha: &str, generated_utc: &str, accept_sha: &str) -> String {
    let q = |tok: &str| if tok.contains("${") { format!("\"{tok}\"") } else { shlex_quote(tok) };
    let (root, image, tool, accept) = ("${RUN_ROOT}", "${IMAGE}", "${WH_TOOL}", "${WH_ACCEPT}");
    let join = |v: Vec<String>| v.iter().map(|t| q(t)).collect::<Vec<_>>().join(" ");
    let lines: Vec<String> = vec![
        "#!/bin/bash".into(),
        format!("# native-install.sh — {} native 설치 재현 명령(native_multinode_serve.py up 이 run {} 에서 **각 노드마다** 실행한 순서).", c.cell, c.run_id),
        "#   각 노드가 자기 로컬 이미지에서 wheelhouse 를 재포장하고(이미지 전송 ✗) 오프라인으로만 설치한다(--no-index · 다운로드 ✗).".into(),
        format!("#   provenance: 원천 이미지 태그 {} · wheelhouse 도구 sha256={tool_sha} · 승인 선언 sha256={accept_sha} · generated_utc={generated_utc}", c.image),
        "#   사용: bash native-install.sh <run-root> [image] — 저장소 루트에서 실행(도구는 저장소 상대경로).".into(),
        "set -euo pipefail".into(),
        "RUN_ROOT=\"${1:?사용: native-install.sh <run-root> [image]}\"".into(),
        format!("IMAGE=\"${{2:-{}}}\"", c.image),
        format!("WH_TOOL={}", shlex_quote(&c.wheel_tool_rel)),
        format!("WH_ACCEPT={}", shlex_quote(&c.wheel_accept_rel)),
        "# ① 재포장(docker create → 설치 트리 cp → RECORD 대조 재포장 · 시작하지 않는 컨테이너)".into(),
        join(wheelhouse_build_argv(tool, image, root, generated_utc, accept)),
        "#    (실행기는 메인 노드에서 Rust 런처 scripts/native_wheelhouse.sh 로, 서브에서는 메인이 run root 에 put 한 같은 ELF(tools/native_wheelhouse)로 부른다 — 같은 판정 JSON)".into(),
        "# ② 설치 게이트(이미지 동등성) — 도구가 python3.12 -m venv → pip install --no-index --no-deps -r wh/requirements-closure.txt".into(),
        "#    → 설치 집합 == closure 핀 → pip check ⊆ 선언된 이미지 고유 충돌 → ldd(vllm *.so) → import vllm,torch + cuda → env 레시피".into(),
        join(wheelhouse_verify_argv(tool, root, generated_utc, accept)),
    ];
    lines.join("\n") + "\n"
}
