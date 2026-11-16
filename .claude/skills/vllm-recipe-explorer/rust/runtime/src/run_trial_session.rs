//! The real docker session of `scripts/run_trial.py` (`run_trial()` after the dry-run/mock branch): stale-container teardown, the
//! measuring-trial gmu cap, the `docker run -d` command and its emit audit, the trial memory watchdog side process, the health
//! poll with the container-liveness check, the functional smoke, the log capture into the simlog, the log parse and the
//! always-run teardown (container, watchdog, page cache).
//!
//! Two neighbours are reached through their Rust owners' launchers (the session never launches an interpreter): the
//! serving-budget declaration (`terraforming_node/scripts/node_blackbox/blackbox_session.sh` — declare / skip / clear, with the
//! node directory from `node_identity.sh --resolve`, as `_budget_*` of the original) and, for the real-trial command line only,
//! the grounding backstop of an active campaign (`terraforming_node/scripts/campaign_init.sh --grounding-check`, status 4 on
//! refusal). An undeclarable budget is a warned, recorded `budget-skip` (the original's policy); `--no-budget` is the explicit
//! unprotected entry (a stderr line: with no node directory there is nowhere to record it, as in the original).
//! Progress lines go to stderr live, as in the original.

use crate::functional_smoke::{smoke, Constants, Plain};
use crate::pyexpr::{get, is_none};
use crate::run_trial::{
    audit_emitted, build_docker_cmd, container_state, host_floor_gmu_cap, poll_health, DockerOptions,
    CONTAINER_MODELS, DEFAULT_IMAGE, DEFAULT_PORT, DEFAULT_TIMEOUT, HEALTH_POLL_INTERVAL, HOST_FLOOR_HEADROOM_MIB,
    HOST_FLOOR_MIB, NAS_MOUNT, PROVENANCE_MEASURED,
};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const BUDGET_OVERHEAD_ENV: &str = "TRIAL_BUDGET_OVERHEAD_MIB";

/// The options of the session (`opts` of the original; absent = `None`).
#[derive(Clone, Debug, Default)]
pub struct SessionOpts {
    pub image: Option<PyStr>,
    pub timeout: Option<f64>,
    pub port: Option<i64>,
    pub smoke_timeout: Option<f64>,
    pub served_model_name: Option<PyStr>,
    pub container_name: Option<PyStr>,
    pub nas_mount: Option<PyStr>,
    pub tiktoken_host_path: Option<PyStr>,
    pub nas_container_root: Option<PyStr>,
    pub jit_cache_root: Option<PyStr>,
    pub max_jobs: Option<i64>,
    pub manifest: Option<PyStr>,
    pub budget_node_dir: Option<PyStr>,
    pub model_host_path: Option<PyStr>,
    pub tp: Option<i64>,
    pub overhead_mib: Option<i64>,
    pub checkpoint_bytes: Option<i64>,
    pub unified_memory: Option<bool>,
    pub memwatch_thresh_mib: Option<i64>,
    pub no_budget: bool,
}

/// What stops a session before docker is touched (`status` is the process status).
#[derive(Debug)]
pub enum SessionError {
    /// A dependency without a Rust owner or a declaration refusal: status and the stderr text.
    Stop { status: i32, text: String },
    Failure(Failure),
}

impl From<Failure> for SessionError {
    fn from(f: Failure) -> SessionError {
        SessionError::Failure(f)
    }
}

fn err(line: &str) {
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// `_repo_root()`: derived lexically from the process name when the launcher (`scripts/run_trial.sh`) names the owning skill's
/// installed program (`<root>/.claude/skills/<skill>/rust/runtime/target/release/run_trial`, an absolute path), else from the
/// executable location; either way the program is eight path components below the root.
pub fn repo_root() -> PathBuf {
    let up = |p: &Path, n: usize| {
        let mut cur = p.to_path_buf();
        for _ in 0..n {
            if !cur.pop() {
                break;
            }
        }
        cur
    };
    if let Some(arg0) = std::env::args_os().next() {
        let p = PathBuf::from(arg0);
        if p.file_name().map_or(false, |n| n == "run_trial") && p.is_absolute() {
            // run_trial, release, target, runtime, rust, <skill>, skills, .claude: the root is above them.
            return up(&p, 8);
        }
    }
    match std::env::current_exe() {
        Ok(exe) => up(&exe, 8),
        Err(_) => PathBuf::from("."),
    }
}

/// `_node_tool_path(plane, name)`: owner canonical location first, then the rendered sub runtime delivery.
pub fn node_tool_path(root: &Path, plane: &str, name: &str) -> Option<PathBuf> {
    let owner = root.join(".claude/skills/terraforming_node/scripts").join(plane);
    let runtime = root.join(".claude/runtime").join(plane);
    [owner.join(name), runtime.join(name)].into_iter().find(|p| p.is_file())
}

fn read_meminfo_mib(key: &str) -> Option<i64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(&format!("{}:", key)) {
            return rest.split_whitespace().next()?.parse::<i64>().ok().map(|kib| kib / 1024);
        }
    }
    None
}

/// `require_overhead_mib(opts)`: the declared overhead only (option, then env); absence is fail-loud.
pub fn require_overhead_mib(overhead_opt: Option<i64>, env: Option<&str>) -> Result<i64, String> {
    let sources: [(Option<String>, String); 2] = [
        (overhead_opt.map(|v| v.to_string()), "config candidate `overhead_mib`".to_string()),
        (env.map(str::to_string), format!("env {}", BUDGET_OVERHEAD_ENV)),
    ];
    for (value, wher) in sources {
        let Some(value) = value.filter(|v| !v.is_empty()) else { continue };
        let mib: i64 = value
            .trim()
            .parse()
            .map_err(|_| format!("[trial] FAIL: overhead 선언이 정수가 아니다({} = {:?})", wher, value))?;
        if mib <= 0 {
            return Err(format!("[trial] FAIL: overhead 선언이 양수가 아니다({} = {:?})", wher, value));
        }
        return Ok(mib);
    }
    Err(format!(
        "[trial] FAIL: 예산 overhead 가 선언되지 않았다 — 기본값을 쓰지 않는다(2026-09-05 · G-B1).\n  → 왜: overhead 를 낮게 잡으면 선언 바닥이 높아져 워치독 arm 상한이 정상 서빙 위로 올라간다.\n     옛 기본값 12288 MiB 는 gpt-oss-120b/GB10 실측(17,971)보다 5,683 작았고 정상 로드를 죽였다.\n  → 어떻게: 후보 config 에 `overhead_mib: <n>` 을 적거나 {}=<n> 으로 넘겨라.\n     모르면 재라: 로드 완료 후 (MemTotal − MemAvailable) − weights − kv.",
        BUDGET_OVERHEAD_ENV
    ))
}

fn docker_logs(name: &str) -> String {
    match Command::new("docker")
        .args(["logs", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(out) => {
            // stdout+stderr merged; the original interleaves through one pipe, here the two are appended (vLLM logs to stderr).
            let mut bytes = out.stdout;
            bytes.extend_from_slice(&out.stderr);
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Err(_) => String::new(),
    }
}

fn docker_teardown(name: &str) {
    let _ = Command::new("docker")
        .args(["rm", "-f", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn start_memwatch(
    root: &Path,
    container: &str,
    simlog_dir: &Path,
    trial: i64,
    thresh_mib: i64,
) -> Option<Child> {
    let Some(script) = node_tool_path(root, "host_safety", "mem_watchdog.sh") else {
        err("[run_trial] ⚠ host-safety mem_watchdog asset 부재 — 협역 워치독 생략(상시 systemd 층만)\n");
        return None;
    };
    let log_path = simlog_dir.join(format!("trial{:02}_memwatch.log", trial));
    let file = std::fs::File::create(&log_path).ok()?;
    let file2 = file.try_clone().ok()?;
    match Command::new("bash")
        .arg(&script)
        .arg(container)
        .arg(thresh_mib.to_string())
        .arg("2")
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(file2))
        .spawn()
    {
        Ok(child) => Some(child),
        Err(e) => {
            err(&format!("[run_trial] ⚠ 협역 워치독 기동 실패({}) — 상시 systemd 층만\n", e));
            None
        }
    }
}

/// PID-based stop (never `pkill -f`).
fn stop_memwatch(child: Option<Child>) {
    let Some(mut child) = child else { return };
    let _ = Command::new("kill").arg("-TERM").arg(child.id().to_string()).status();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
}

fn drop_caches_best_effort() {
    let helper = "/usr/local/sbin/vllm-drop-caches";
    if !Path::new(helper).exists() {
        return;
    }
    let _ = Command::new("sudo")
        .args(["-n", helper])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn error_excerpt(text: &str) -> PyValue {
    // The last 2000 code points; an empty text is the empty string (the original returns "").
    let chars: Vec<char> = text.chars().collect();
    let start = chars.len().saturating_sub(2000);
    PyValue::str(&chars[start..].iter().collect::<String>())
}

/// The campaign writer's launcher (its Rust `campaign_init` owns `--grounding-check`).
pub const CAMPAIGN_WRITER_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.sh";

/// The grounding entry backstop of `run_trial.py main()` (policy:LIBRARY_GROUNDING_FAIL_CLOSED C2): the campaign writer's
/// `--grounding-check` runs with this process's streams; a non-zero status stops the trial with status 4 before any docker
/// action. A missing writer is reported (silent omission ✗) and does not stop, as in the original. Only the real-trial command
/// line calls this (the original's backstop sat in `main()`, not in `run_trial()` that `recipe.py simulate` calls).
pub fn pre_session_gates(root: &Path, _opts: &SessionOpts) -> Result<(), SessionError> {
    let writer = root.join(CAMPAIGN_WRITER_REL);
    if !writer.is_file() {
        err(&format!(
            "[run_trial] ⚠ campaigns writer 부재({}) — 그라운딩 백스톱이 돌지 않았다(침묵 누락 ✗).\n",
            writer.display()
        ));
        return Ok(());
    }
    let status = Command::new("bash").arg(&writer).arg("--grounding-check").status();
    if !matches!(status, Ok(s) if s.success()) {
        return Err(SessionError::Stop {
            status: 4,
            text: format!(
                "[run_trial] STOP: 그라운딩 기록 없이 트라이얼에 진입하지 않는다(policy:LIBRARY_GROUNDING_FAIL_CLOSED C2).\n  → bash {} --ground --utc <UTC> 로 사서에게 먼저 물어라.\n  → 사서가 못 찾으면 그 사실이 기록에 남고 그대로 통과한다(공백은 차단이 아니다).\n",
                writer.display()
            ),
        });
    }
    Ok(())
}

// ── serving budget declaration (blackbox plane · `_budget_*` of the original) ─────────────────────────────────────────────

/// The blackbox session sidecar's launcher (Rust `blackbox_session`, same command line as `blackbox_session.py`).
pub const BLACKBOX_SESSION: &str = "blackbox_session.sh";
const BUDGET_CALL_SECONDS: u64 = 60;

/// `time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())` of `seconds` since the epoch (civil-from-days, proleptic Gregorian).
pub fn utc_iso(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, rem / 3600, rem % 3600 / 60, rem % 60)
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    utc_iso(secs)
}

/// A bounded child: `(status, stdout, stderr)`, or the reason it did not complete (`Type: message`, as the original's `%s: %s`).
fn bounded(cmd: &mut Command, seconds: u64) -> Result<(Option<i32>, Vec<u8>, Vec<u8>), String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("OSError: {}", e))?;
    let (mut out, mut er) = (child.stdout.take(), child.stderr.take());
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_end(&mut buf);
        }
        buf
    });
    let ereader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        if let Some(e) = er.as_mut() {
            let _ = e.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("TimeoutExpired: Command timed out after {} seconds", seconds));
            }
            Err(e) => return Err(format!("OSError: {}", e)),
        }
    };
    Ok((status.code(), reader.join().unwrap_or_default(), ereader.join().unwrap_or_default()))
}

fn blackbox_session_path(root: &Path) -> PathBuf {
    node_tool_path(root, "node_blackbox", BLACKBOX_SESSION)
        .unwrap_or_else(|| root.join(".claude/skills/terraforming_node/scripts/node_blackbox").join(BLACKBOX_SESSION))
}

/// `_budget_session(node_dir, *args)`: one sidecar subcommand → `(succeeded, output stripped)`. The original merged stderr into
/// stdout through one pipe; here stdout and then stderr are joined (the text is only quoted in a progress line).
fn budget_session(root: &Path, node_dir: &str, args: &[String]) -> (bool, String) {
    let mut cmd = Command::new("bash");
    cmd.arg(blackbox_session_path(root)).arg("--node-dir").arg(node_dir).args(args);
    match bounded(&mut cmd, BUDGET_CALL_SECONDS) {
        Ok((code, mut out, er)) => {
            out.extend_from_slice(&er);
            (code == Some(0), String::from_utf8_lossy(&out).trim().to_string())
        }
        Err(reason) => (false, reason),
    }
}

/// `budget_ttl_floor_s()`: the TTL floor asked from its single owner; an unreadable answer stops (no number is re-typed here).
fn budget_ttl_floor_s(root: &Path) -> Result<i64, SessionError> {
    let mut cmd = Command::new("bash");
    cmd.arg(blackbox_session_path(root)).args(["--node-dir", ".", "budget-defaults", "--field", "ttl_s"]);
    let (code, out, er) = bounded(&mut cmd, BUDGET_CALL_SECONDS).unwrap_or((None, Vec::new(), Vec::new()));
    let text = String::from_utf8_lossy(&out).trim().to_string();
    if code != Some(0) || text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        let stderr: String = String::from_utf8_lossy(&er).trim().chars().take(200).collect();
        return Err(SessionError::Stop {
            status: 1,
            text: format!(
                "[trial] FAIL: 예산 TTL 기본값을 blackbox_session 에서 읽지 못했다 — 여기에 숫자를 다시 적지 않는다(rc={} {})\n",
                code.map_or("None".to_string(), |c| c.to_string()),
                stderr
            ),
        });
    }
    text.parse::<i64>().map_err(|_| SessionError::Stop { status: 1, text: "[trial] FAIL: 예산 TTL 기본값이 범위를 넘는다\n".to_string() })
}

/// `_budget_node_dir(opts)`: explicit > the node-identity resolver (`node_identity.sh --resolve`) > `None` (logged).
fn budget_node_dir(root: &Path, opts: &SessionOpts) -> Option<String> {
    if let Some(explicit) = opts.budget_node_dir.as_ref().and_then(PyStr::to_utf8).filter(|e| !e.is_empty()) {
        return Some(explicit);
    }
    let Some(resolver) = node_tool_path(root, "node_blackbox", "node_identity.sh") else {
        err(&format!(
            "[budget] node_id 해소기 부재 — {}(정본) · .claude/runtime/node_blackbox(서브 배달분) 어디에도 없다. 선언을 생략한다(추측하지 않는다).\n",
            root.join(".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh").display()
        ));
        return None;
    };
    let mut cmd = Command::new("bash");
    cmd.arg(&resolver).arg("--resolve").arg("--repo").arg(root);
    let (code, out, er) = match bounded(&mut cmd, BUDGET_CALL_SECONDS) {
        Ok(r) => r,
        Err(reason) => {
            err(&format!("[budget] node_id 해소기 실행 실패({}) — 선언을 생략한다.\n", reason));
            return None;
        }
    };
    if code != Some(0) {
        // the resolver's fail-loud reason goes up as it is (swallowing it would be a silent fallback)
        let _ = std::io::stderr().write_all(&er);
        err(&format!("[budget] node_id 미해소(rc={}) — 선언을 생략한다.\n", code.map_or(-1, |c| c)));
        return None;
    }
    let node_id = String::from_utf8_lossy(&out).trim().to_string();
    // the resolver enforces the scheme already; it becomes a path component, so the consumer checks again
    let mut chars = node_id.chars();
    let scheme_ok = node_id.len() <= 32
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !scheme_ok {
        err(&format!("[budget] 해소기가 낸 node_id 가 스킴 위반이다({:?}) — 선언을 생략한다.\n", node_id));
        return None;
    }
    Some(root.join("docs/logs").join(node_id).to_string_lossy().into_owned())
}

/// The declaration inputs (`_budget_inputs`), all MiB.
#[derive(Clone, Debug, PartialEq)]
pub struct BudgetInputs {
    pub mem_total_mib: i64,
    pub weights_mib: i64,
    pub kv_mib: i64,
    pub overhead_mib: i64,
}

const MIB: i64 = 1024 * 1024;

fn ceil_div(a: i64, b: i64) -> i64 {
    -((-a).div_euclid(b))
}

/// `_budget_inputs(candidate, opts)` over the facts it reads (`mem_total_mib` = `/proc/meminfo` MemTotal ÷ 1024): `Ok(Ok(inputs))`,
/// `Ok(Err(skip reason))`, or the overhead declaration's fail-loud stop.
pub fn budget_inputs(
    kv_bytes: Option<i64>,
    mem_total_mib: Option<i64>,
    checkpoint_bytes: Option<i64>,
    tp: Option<i64>,
    overhead: Result<i64, String>,
) -> Result<Result<BudgetInputs, &'static str>, SessionError> {
    let Some(kv) = kv_bytes.filter(|k| *k != 0) else { return Ok(Err("kv_clamp_absent")) };
    let Some(mem_total_mib) = mem_total_mib else { return Ok(Err("mem_total_unavailable")) };
    let Some(ckpt) = checkpoint_bytes.filter(|c| *c > 0) else { return Ok(Err("checkpoint_size_unavailable")) };
    let tp = tp.filter(|t| *t != 0).unwrap_or(1);
    let overhead_mib = overhead.map_err(|m| SessionError::Stop { status: 1, text: format!("{}\n", m) })?;
    Ok(Ok(BudgetInputs {
        mem_total_mib,
        // `-(-int(ckpt) // tp // M)`: ceil(ceil(ckpt ÷ tp) ÷ MiB) (ceil-div, the RAM gate's axis)
        weights_mib: ceil_div(ceil_div(ckpt, tp), MIB),
        kv_mib: ceil_div(kv, MIB),
        overhead_mib,
    }))
}

/// The checkpoint size the declaration uses: the caller's value, else the host model path's checkpoint files (`None` = the skip
/// reason `checkpoint_size_unavailable` / `model_host_path_absent`).
fn checkpoint_bytes(opts: &SessionOpts) -> Result<Option<i64>, &'static str> {
    if let Some(c) = opts.checkpoint_bytes.filter(|c| *c != 0) {
        return Ok(Some(c));
    }
    let Some(host) = opts.model_host_path.clone().filter(|h| h.to_utf8().is_some_and(|p| Path::new(&p).is_dir())) else {
        return Err("model_host_path_absent");
    };
    match crate::memory::checkpoint_bytes_for(&PyValue::Str(host), None) {
        Ok(v) => Ok(crate::pyexpr::int_of(&v).ok().and_then(|n| i64::try_from(n).ok())),
        Err(_) => Err("checkpoint_size_unavailable"),
    }
}

/// `_budget_skip(node_dir, reason, label)`: an unprotected entry is recorded, never silent.
fn budget_skip(root: &Path, node_dir: Option<&str>, reason: &str, label: &str) {
    err(&format!("[trial] ⚠ 예산 선언 생략(무보호 진입): {}\n", reason));
    if let Some(dir) = node_dir {
        let _ = budget_session(root, dir, &["budget-skip".into(), "--reason".into(), reason.into(), "--label".into(), label.into(), "--now".into(), now_iso()]);
    }
}

/// The declaration step of `run_trial()` (after the narrow watchdog starts, before the load): `Ok(Some(node_dir))` when a
/// declaration stands and must be cleared after teardown.
fn budget_declare_step(root: &Path, opts: &SessionOpts, candidate: &PyValue, label: &str, timeout: f64) -> Result<Option<String>, SessionError> {
    if opts.no_budget {
        budget_skip(root, None, "no_budget_flag", label);
        return Ok(None);
    }
    let Some(node_dir) = budget_node_dir(root, opts) else {
        // without a node directory there is nowhere to record the event — stderr only, honestly
        budget_skip(root, None, "node_dir_unresolved", label);
        return Ok(None);
    };
    let kv = get(candidate, "kv_cache_memory_bytes")?;
    let kv_bytes = if kv.is_truthy() { crate::pyexpr::float_of(&kv).ok().map(|f| f as i64) } else { None };
    let inputs = if kv_bytes.is_none() {
        Ok(Err("kv_clamp_absent"))
    } else {
        match read_meminfo_mib("MemTotal") {
            None => Ok(Err("mem_total_unavailable")),
            Some(total) => match checkpoint_bytes(opts) {
                Err(reason) => Ok(Err(reason)),
                Ok(ckpt) => budget_inputs(
                    kv_bytes,
                    Some(total),
                    ckpt,
                    opts.tp,
                    require_overhead_mib(opts.overhead_mib, std::env::var(BUDGET_OVERHEAD_ENV).ok().as_deref()),
                ),
            },
        }
    }?;
    let inputs = match inputs {
        Ok(i) => i,
        Err(reason) => {
            budget_skip(root, Some(&node_dir), reason, label);
            return Ok(None);
        }
    };
    let ttl_s = budget_ttl_floor_s(root)?.max(timeout as i64 * 3);
    let (ok, out) = budget_session(
        root,
        &node_dir,
        &[
            "declare-budget".into(),
            "--mem-total-mib".into(),
            inputs.mem_total_mib.to_string(),
            "--weights-mib".into(),
            inputs.weights_mib.to_string(),
            "--kv-mib".into(),
            inputs.kv_mib.to_string(),
            "--overhead-mib".into(),
            inputs.overhead_mib.to_string(),
            "--ttl-s".into(),
            ttl_s.to_string(),
            "--expected-load-s".into(),
            (timeout as i64).to_string(),
            "--label".into(),
            label.into(),
            "--now".into(),
            now_iso(),
        ],
    );
    err(&format!("[trial] 예산 선언: {}\n", if out.is_empty() { if ok { "ok" } else { "실패" } } else { &out }));
    if !ok {
        budget_skip(root, Some(&node_dir), "declare_failed", label);
        return Ok(None);
    }
    Ok(Some(node_dir))
}

/// `_budget_clear(node_dir)`: a declaration outlives the container, then goes (a stale one would arm the next load).
fn budget_clear(root: &Path, node_dir: &str) {
    let (ok, out) = budget_session(root, node_dir, &["clear-budget".into(), "--now".into(), now_iso()]);
    err(&format!("[trial] 예산 회수: {}\n", if out.is_empty() { if ok { "ok" } else { "실패" } } else { &out }));
}

/// `run_trial(...)` for the real docker path. `candidate` is the loaded candidate; the result is the measured record.
pub fn run_session(
    candidate: &PyValue,
    simlog_dir: &PyStr,
    trial_number: i64,
    opts: &SessionOpts,
) -> Result<PyValue, SessionError> {
    let root = repo_root();
    let default_image = PyStr::new(DEFAULT_IMAGE);
    let image = opts.image.clone().unwrap_or(default_image);
    let timeout = opts.timeout.unwrap_or(DEFAULT_TIMEOUT);
    let port = opts.port.unwrap_or(DEFAULT_PORT);
    let smoke_timeout = opts.smoke_timeout.unwrap_or(60.0);
    let container_name = opts
        .container_name
        .clone()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| PyStr::new(&format!("vllm_trial{:02}", trial_number)));
    let container = container_name.to_utf8().unwrap_or_default();
    let sim_dir_text = simlog_dir.to_utf8().unwrap_or_default();
    let log_path = if sim_dir_text.ends_with('/') {
        format!("{}trial{:02}_vllm.log", sim_dir_text, trial_number)
    } else {
        format!("{}/trial{:02}_vllm.log", sim_dir_text, trial_number)
    };

    // served_model_name: opts → candidate.served_model_name → model_id → model_path_container → model_path → /app/models.
    let mut served = opts.served_model_name.clone().filter(|s| !s.is_empty()).map(PyValue::Str);
    for key in ["served_model_name", "model_id", "model_path_container", "model_path"] {
        if served.is_some() {
            break;
        }
        let v = get(candidate, key)?;
        if v.is_truthy() {
            served = Some(v);
        }
    }
    let served_model_name = served.unwrap_or_else(|| PyValue::str(CONTAINER_MODELS));

    docker_teardown(&container);

    let nas_mount = opts.nas_mount.clone().unwrap_or_else(|| PyStr::new(NAS_MOUNT));
    let memwatch_thresh = opts.memwatch_thresh_mib.unwrap_or(HOST_FLOOR_MIB);

    let mut candidate = candidate.clone();
    let mut gmu_cap_info = PyValue::None;
    let mut effective_gmu = get(&candidate, "gpu_memory_utilization")?;
    if is_none(&get(&candidate, "kv_cache_memory_bytes")?) && opts.unified_memory == Some(true) {
        let tp = opts.tp.filter(|t| *t != 0).unwrap_or(1);
        let weights = opts.checkpoint_bytes.map(|ck| -(-ck / tp / (1024 * 1024)));
        let requested = if is_none(&effective_gmu) {
            None
        } else {
            Some(crate::pyexpr::float_of(&effective_gmu)?)
        };
        let overhead = require_overhead_mib(opts.overhead_mib, std::env::var(BUDGET_OVERHEAD_ENV).ok().as_deref())
            .map_err(|m| SessionError::Stop { status: 1, text: format!("{}\n", m) })?;
        let info = host_floor_gmu_cap(
            requested,
            read_meminfo_mib("MemTotal"),
            read_meminfo_mib("MemAvailable"),
            memwatch_thresh,
            weights,
            Some(overhead),
        );
        let show = |v: Option<i64>| v.map_or("None".to_string(), |x| x.to_string());
        if info.applied {
            err(&format!(
                "[trial] gmu 캡 적용: {} → {} (호스트 바닥 {}MiB + 여유 {}MiB 준수, MemAvailable={}MiB/MemTotal={}MiB). 측정 트라이얼의 구조적 사살 회피 — 바닥을 낮춘 것이 아니라 요구를 깎았다.\n",
                PyValue::Float(info.requested.unwrap_or(0.0)).display_text().to_utf8().unwrap_or_default(),
                PyValue::Float(info.gmu.unwrap_or(0.0)).display_text().to_utf8().unwrap_or_default(),
                memwatch_thresh,
                HOST_FLOOR_HEADROOM_MIB,
                show(info.mem_available_mib),
                show(info.mem_total_mib),
            ));
            if let PyValue::Dict(d) = &candidate {
                let mut d: PyDict = d.clone();
                d.set("gpu_memory_utilization", PyValue::Float(info.gmu.unwrap_or(0.0)));
                candidate = PyValue::Dict(d);
            }
            effective_gmu = PyValue::Float(info.gmu.unwrap_or(0.0));
        } else {
            err(&format!(
                "[trial] gmu 캡 미적용({}) — 요청값 {} 그대로 진입. 침묵 금지: 이 사유가 trial 산출물 gmu_cap.reason 에 남는다.\n",
                info.reason,
                info.requested.map_or("None".to_string(), |r| PyValue::Float(r).display_text().to_utf8().unwrap_or_default()),
            ));
        }
        gmu_cap_info = info.to_value();
    }

    let mut warnings: Vec<PyStr> = Vec::new();
    let docker_cmd = build_docker_cmd(
        &candidate,
        &DockerOptions {
            image: &image,
            container_name: &container_name,
            port,
            nas_mount: &nas_mount,
            tiktoken_host_path: opts.tiktoken_host_path.as_ref(),
            jit_cache_root: opts.jit_cache_root.as_ref(),
            max_jobs: Some(opts.max_jobs.unwrap_or(4)),
            nas_container_root: opts.nas_container_root.as_ref(),
        },
        &mut warnings,
    )?;
    for w in &warnings {
        err(&w.to_utf8().unwrap_or_default());
    }
    audit_emitted(&candidate, &docker_cmd)?;

    let simlog_path = PathBuf::from(&sim_dir_text);
    let watchdog = start_memwatch(&root, &container, &simlog_path, trial_number, memwatch_thresh);

    // Serving budget declaration — right after the narrow watchdog, before the load (declare → honored → up, as the multi smoke).
    let served_text = served_model_name.display_text().to_utf8().unwrap_or_default();
    let budget_label = format!("trial{:02}-{}", trial_number, served_text.rsplit('/').next().unwrap_or(""));
    let budget = match budget_declare_step(&root, opts, &candidate, &budget_label, timeout) {
        Ok(b) => b,
        Err(stop) => {
            // the declaration's fail-loud stop happens before the load; the watchdog started above does not outlive it
            stop_memwatch(watchdog);
            return Err(stop);
        }
    };
    let outcome = session_body(
        &candidate, &docker_cmd, &container, port, timeout, smoke_timeout, &served_model_name, &log_path,
    );

    // Always: container teardown, watchdog stop (PID based), page-cache drop, then the budget clear (after teardown: the
    // declaration covers the load, so it outlives the container).
    docker_teardown(&container);
    stop_memwatch(watchdog);
    drop_caches_best_effort();
    if let Some(node_dir) = &budget {
        budget_clear(&root, node_dir);
    }

    let body = outcome?;
    let mut d = PyDict::new();
    d.set("trial_number", PyValue::int(trial_number));
    d.set("candidate", candidate.clone());
    d.set("load_ok", PyValue::Bool(body.load_ok));
    d.set("vllm_profile", body.vllm_profile);
    d.set("functional", body.functional);
    d.set("log_path", PyValue::str(&log_path));
    d.set("error_excerpt", body.error_excerpt);
    d.set("provenance", PyValue::str(PROVENANCE_MEASURED));
    d.set("health_wait", body.health_wait);
    d.set("effective_gmu", effective_gmu);
    d.set("gmu_cap", gmu_cap_info);
    Ok(PyValue::Dict(d))
}

struct Body {
    load_ok: bool,
    vllm_profile: PyValue,
    functional: PyValue,
    error_excerpt: PyValue,
    health_wait: PyValue,
}

fn write_log(path: &str, text: &str) -> Result<(), Failure> {
    std::fs::write(path, text).map_err(|e| Failure::new("OSError", &e.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn session_body(
    candidate: &PyValue,
    docker_cmd: &[PyStr],
    container: &str,
    port: i64,
    timeout: f64,
    smoke_timeout: f64,
    served_model_name: &PyValue,
    log_path: &str,
) -> Result<Body, Failure> {
    let argv: Vec<std::ffi::OsString> = docker_cmd
        .iter()
        .map(|a| {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(a.to_surrogateescape_bytes().unwrap_or_default())
        })
        .collect();
    let started = Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let run_out = match started {
        Ok(o) => o,
        Err(e) => {
            // The original lets the OSError propagate after the finally block (teardown still runs in the caller).
            return Err(Failure::new("FileNotFoundError", &format!("{}", e)));
        }
    };
    if !run_out.status.success() {
        let stderr_text = String::from_utf8_lossy(&run_out.stderr).into_owned();
        write_log(log_path, &stderr_text)?;
        return Ok(Body {
            load_ok: false,
            vllm_profile: PyValue::None,
            functional: PyValue::None,
            error_excerpt: error_excerpt(&stderr_text),
            health_wait: PyValue::None,
        });
    }
    let mut sink = |line: &str| err(line);
    let wait = poll_health(port, timeout, Some(container), &mut container_state, HEALTH_POLL_INTERVAL, &mut sink);
    let healthy = wait.healthy;
    let mut functional = PyValue::None;
    if healthy {
        let base_url = PyValue::str(&format!("http://127.0.0.1:{}", port));
        functional = smoke(
            &mut Plain,
            &Constants::canonical(),
            &base_url,
            served_model_name,
            candidate,
            &PyValue::Float(smoke_timeout),
            &PyValue::None,
        )
        .map_err(|r| Failure::new("RuntimeError", &r.message.to_utf8().unwrap_or_default()))?;
    }
    let log_text = docker_logs(container);
    write_log(log_path, &log_text)?;
    let profile = crate::vllm_log::parse_log(&PyValue::str(&log_text))?;
    let excerpt = if healthy { PyValue::None } else { error_excerpt(&log_text) };
    Ok(Body {
        load_ok: healthy,
        vllm_profile: PyValue::Dict(profile),
        functional,
        error_excerpt: excerpt,
        health_wait: wait.to_value(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_text_is_the_gmtime_format() {
        assert_eq!(utc_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_iso(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_iso(1_709_251_199), "2024-02-29T23:59:59Z");
    }

    #[test]
    fn budget_inputs_follow_the_original_order_and_rounding() {
        let gib = 1024 * MIB;
        let ok = budget_inputs(Some(40 * gib + 1), Some(122_000), Some(65 * gib + 3), Some(2), Ok(17_971)).ok().unwrap();
        // weights = ceil(ceil(ckpt ÷ tp) ÷ MiB), the original's `-(-c // tp // M)`
        assert_eq!(
            ok,
            Ok(BudgetInputs { mem_total_mib: 122_000, weights_mib: 33_281, kv_mib: 40 * 1024 + 1, overhead_mib: 17_971 })
        );
        assert_eq!(budget_inputs(None, Some(1), Some(1), None, Ok(1)).ok().unwrap(), Err("kv_clamp_absent"));
        assert_eq!(budget_inputs(Some(1), None, Some(1), None, Ok(1)).ok().unwrap(), Err("mem_total_unavailable"));
        assert_eq!(budget_inputs(Some(1), Some(1), Some(0), None, Ok(1)).ok().unwrap(), Err("checkpoint_size_unavailable"));
        // an undeclared overhead is the fail-loud stop, not a skip
        assert!(matches!(budget_inputs(Some(1), Some(1), Some(1), None, Err("no".into())), Err(SessionError::Stop { status: 1, .. })));
        let one = budget_inputs(Some(MIB), Some(1), Some(3 * MIB), Some(0), Ok(1)).ok().unwrap().unwrap();
        assert_eq!(one.weights_mib, 3);
        // ceil before ceil: (2 MiB + 1) ÷ 2 → 1 MiB + 1 byte → 2 MiB (a floor first would say 1)
        let two = budget_inputs(Some(MIB), Some(1), Some(2 * MIB + 1), Some(2), Ok(1)).ok().unwrap().unwrap();
        assert_eq!(two.weights_mib, 2);
    }

    #[test]
    fn the_budget_sidecar_is_the_rust_launcher_and_a_failing_call_is_reported() {
        let dir = std::env::temp_dir().join(format!("rts-budget-{}", std::process::id()));
        let plane = dir.join(".claude/skills/terraforming_node/scripts/node_blackbox");
        std::fs::create_dir_all(&plane).unwrap();
        assert_eq!(blackbox_session_path(&dir), plane.join(BLACKBOX_SESSION));
        // a stand-in launcher (test plane): echoes its argv, refuses declare-budget
        std::fs::write(
            plane.join(BLACKBOX_SESSION),
            "echo \"$*\"; case \" $* \" in *' declare-budget '*) echo refused >&2; exit 3;; esac\n",
        )
        .unwrap();
        let (ok, out) = budget_session(&dir, "/n", &["clear-budget".into(), "--now".into(), "T".into()]);
        assert!(ok);
        assert_eq!(out, "--node-dir /n clear-budget --now T");
        let (ok, out) = budget_session(&dir, "/n", &["declare-budget".into()]);
        assert!(!ok);
        assert_eq!(out, "--node-dir /n declare-budget\nrefused");
        // budget-defaults must answer digits, else the stop names the owner
        assert!(matches!(budget_ttl_floor_s(&dir), Err(SessionError::Stop { status: 1, .. })));
        std::fs::write(plane.join(BLACKBOX_SESSION), "echo 7200\n").unwrap();
        assert_eq!(budget_ttl_floor_s(&dir).ok(), Some(7200));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
