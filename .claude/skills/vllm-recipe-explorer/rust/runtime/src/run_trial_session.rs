//! The real docker session of `scripts/run_trial.py` (`run_trial()` after the dry-run/mock branch): stale-container teardown, the
//! measuring-trial gmu cap, the `docker run -d` command and its emit audit, the trial memory watchdog side process, the health
//! poll with the container-liveness check, the functional smoke, the log capture into the simlog, the log parse and the
//! always-run teardown (container, watchdog, page cache).
//!
//! Two neighbours have no Rust owner yet and are therefore NOT started from Python here (the session never launches an
//! interpreter): the serving-budget declaration (`blackbox_session.py`, the blackbox plane) and the grounding backstop of an
//! active campaign (`campaign_init.py --grounding-check`). Both stop the session with status 2 *before any docker action*, with
//! the reason named; the explicit unprotected entry is `--no-budget` (the original records it as `budget_skipped`, which also
//! needs the blackbox writer, so here it is a stderr line only). Nothing is skipped silently.
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

/// The grounding backstop and the budget dependency: both stop before any docker action. `Ok(())` = proceed.
pub fn pre_session_gates(root: &Path, opts: &SessionOpts) -> Result<(), SessionError> {
    // campaigns/ACTIVE: no pointer or a pointer to a missing instance = `_bootstrap` (the backstop passes by itself).
    let base = root.join("campaigns");
    let active = std::fs::read_to_string(base.join("ACTIVE"))
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|n| !n.is_empty() && base.join(n).is_dir());
    match active {
        None => err("[campaign_init] 그라운딩 검사 생략 — 활성 캠페인 없음(_bootstrap)\n"),
        Some(name) => {
            return Err(SessionError::Stop {
                status: 2,
                text: format!(
                    "[run_trial] STOP: 활성 캠페인({})이 있어 그라운딩 백스톱(campaign_init --grounding-check · policy:LIBRARY_GROUNDING_FAIL_CLOSED C2)이 필요한데 그 검사의 Rust 소유자가 아직 없다 — Python 을 띄우지 않으므로 docker 에 진입하지 않는다(잔여: campaign_init grounding_reasons 이식).\n",
                    name
                ),
            })
        }
    }
    if !opts.no_budget {
        return Err(SessionError::Stop {
            status: 2,
            text: "[run_trial] STOP: 예산 선언(blackbox_session declare-budget)의 Rust 소유자가 아직 없다 — Python 을 띄우지 않으므로 docker 에 진입하지 않는다(잔여: blackbox 이식). 무보호 진입을 명시하려면 --no-budget.\n".to_string(),
        });
    }
    Ok(())
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

    err("[trial] ⚠ 예산 선언 생략(무보호 진입): no_budget_flag\n");
    let outcome = session_body(
        &candidate, &docker_cmd, &container, port, timeout, smoke_timeout, &served_model_name, &log_path,
    );

    // Always: container teardown, watchdog stop (PID based), page-cache drop.
    docker_teardown(&container);
    stop_memwatch(watchdog);
    drop_caches_best_effort();

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
