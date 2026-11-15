//! The pure part of `scripts/run_trial.py` (single-trial runner): the `vllm serve` argument list, the `docker run -d` command, the
//! emit audit, the `docker inspect` state parser, the host-floor gmu cap, the mock/dry-run result and the health poll with its
//! container-liveness check. The session part (a real docker run: memory watchdog, budget declaration, functional smoke, log
//! capture) lives in `run_trial_session`.

use crate::pyexpr::{float_of, get, int_of, is_none, Res};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub const PROVENANCE_MEASURED: &str = "measured";
pub const PROVENANCE_MOCK: &str = "mock";
pub const PROVENANCE_DRY_RUN: &str = "dry-run";

pub const DEFAULT_IMAGE: &str = "vllm-src-022:clean";
pub const DEFAULT_TIMEOUT: f64 = 900.0;
pub const DEFAULT_PORT: i64 = 8903;
pub const NAS_MOUNT: &str = "/mnt/models";
pub const CONTAINER_MODELS: &str = "/app/models";
pub const CONTAINER_ENCODINGS: &str = "/encodings";
pub const HEALTH_POLL_INTERVAL: f64 = 3.0;
/// The host safety floor the trial watchdog trips under (single owner; lowering it is not a remedy).
pub const HOST_FLOOR_MIB: i64 = 10240;
pub const HOST_FLOOR_HEADROOM_MIB: i64 = 2048;
pub const MIN_MEASURE_KV_MIB: i64 = 4096;
/// Self-test fixture overhead only: production requires a declared overhead.
const FIXTURE_OVERHEAD_MIB: i64 = 12288; // antipattern-ok: G-B1-overhead-default

fn s(text: &str) -> PyStr {
    PyStr::new(text)
}

fn truthy(candidate: &PyValue, key: &str) -> Res<bool> {
    Ok(get(candidate, key)?.is_truthy())
}

fn capabilities(candidate: &PyValue) -> Res<PyValue> {
    let caps = get(candidate, "model_capabilities")?;
    Ok(if caps.is_truthy() {
        caps
    } else {
        PyValue::Dict(PyDict::new())
    })
}

fn int_text(value: &PyValue) -> Res<PyStr> {
    Ok(s(&int_of(value)?.to_string()))
}

fn push(args: &mut Vec<PyStr>, flag: &str, value: PyStr) {
    args.push(s(flag));
    args.push(value);
}

/// `_kv_dtype_flag(candidate)`: `str(kv_cache_quant)` or `None`.
pub fn kv_dtype_flag(candidate: &PyValue) -> Res<Option<PyStr>> {
    let q = get(candidate, "kv_cache_quant")?;
    Ok(if q.is_truthy() {
        Some(q.display_text())
    } else {
        None
    })
}

/// `_build_serve_args(candidate)`.
pub fn build_serve_args(candidate: &PyValue) -> Res<Vec<PyStr>> {
    let mut model_path = get(candidate, "model_path_container")?;
    if !model_path.is_truthy() {
        model_path = get(candidate, "model_path")?;
    }
    if !model_path.is_truthy() {
        model_path = PyValue::str(CONTAINER_MODELS);
    }
    let mut args = vec![s("vllm"), s("serve"), model_path.display_text()];

    let served = get(candidate, "served_model_name")?;
    if served.is_truthy() {
        push(&mut args, "--served-model-name", served.display_text());
    }
    for (key, flag) in [
        ("max_model_len", "--max-model-len"),
        ("batch", "--max-num-seqs"),
        ("max_num_batched_tokens", "--max-num-batched-tokens"),
    ] {
        let value = get(candidate, key)?;
        if !is_none(&value) {
            push(&mut args, flag, int_text(&value)?);
        }
    }
    let gmu = get(candidate, "gpu_memory_utilization")?;
    if !is_none(&gmu) {
        push(
            &mut args,
            "--gpu-memory-utilization",
            PyValue::Float(float_of(&gmu)?).display_text(),
        );
    }
    let gdn = get(candidate, "gdn_prefill_backend")?;
    if gdn.is_truthy() {
        push(&mut args, "--gdn-prefill-backend", gdn.display_text());
    }
    if truthy(candidate, "enforce_eager")? {
        args.push(s("--enforce-eager"));
    }
    if truthy(candidate, "language_model_only")? {
        args.push(s("--language-model-only"));
    }
    let quant = get(candidate, "quantization")?;
    if quant.is_truthy() && !crate::pyexpr::lower(&quant.display_text()).equals("none") {
        push(&mut args, "--quantization", quant.display_text());
    }
    let moe = get(candidate, "moe_backend")?;
    if moe.is_truthy() && !crate::pyexpr::lower(&moe.display_text()).equals("auto") {
        push(&mut args, "--moe-backend", moe.display_text());
    }
    let kv_bytes = get(candidate, "kv_cache_memory_bytes")?;
    if !is_none(&kv_bytes) {
        push(&mut args, "--kv-cache-memory-bytes", int_text(&kv_bytes)?);
    }
    if let Some(dtype) = kv_dtype_flag(candidate)? {
        push(&mut args, "--kv-cache-dtype", dtype);
    }
    let caps = capabilities(candidate)?;
    let tcp = get(candidate, "tool_call_parser")?;
    if tcp.is_truthy() && get(&caps, "tool_call")?.is_truthy() {
        args.push(s("--enable-auto-tool-choice"));
        push(&mut args, "--tool-call-parser", tcp.display_text());
    }
    let rp = get(candidate, "reasoning_parser")?;
    if rp.is_truthy() && get(&caps, "reasoning")?.is_truthy() {
        push(&mut args, "--reasoning-parser", rp.display_text());
    }
    push(&mut args, "--host", s("0.0.0.0"));
    push(&mut args, "--port", s(&DEFAULT_PORT.to_string()));
    Ok(args)
}

/// `_needs_tiktoken(candidate)`.
pub fn needs_tiktoken(candidate: &PyValue) -> Res<bool> {
    let caps = capabilities(candidate)?;
    Ok(get(&caps, "tool_call")?.is_truthy() || get(&caps, "reasoning")?.is_truthy())
}

/// The arguments of `_build_docker_cmd` after the candidate.
pub struct DockerOptions<'a> {
    pub image: &'a PyStr,
    pub container_name: &'a PyStr,
    pub port: i64,
    pub nas_mount: &'a PyStr,
    pub tiktoken_host_path: Option<&'a PyStr>,
    pub jit_cache_root: Option<&'a PyStr>,
    pub max_jobs: Option<i64>,
    pub nas_container_root: Option<&'a PyStr>,
}

fn joined(parts: &[&PyStr]) -> PyStr {
    let mut out = PyStr::default();
    for part in parts {
        out.push_py(part);
    }
    out
}

/// `_build_docker_cmd(...)`. The stderr warning of the original (a tiktoken requirement without a host path) is appended to
/// `warnings`; creating the JIT cache directories is the one file-system effect, as in the original.
pub fn build_docker_cmd(
    candidate: &PyValue,
    options: &DockerOptions,
    warnings: &mut Vec<PyStr>,
) -> Res<Vec<PyStr>> {
    let mut cmd: Vec<PyStr> = ["docker", "run", "-d", "--name"].iter().map(|t| s(t)).collect();
    cmd.push(options.container_name.clone());
    for t in [
        "--runtime",
        "nvidia",
        "--ipc",
        "host",
        "--ulimit",
        "memlock=-1",
        "--oom-score-adj",
        "800",
        "-p",
    ] {
        cmd.push(s(t));
    }
    cmd.push(s(&format!("{}:{}", options.port, DEFAULT_PORT)));
    cmd.push(s("-v"));
    let root = options
        .nas_container_root
        .filter(|r| !r.is_empty())
        .cloned()
        .unwrap_or_else(|| s(CONTAINER_MODELS));
    cmd.push(joined(&[options.nas_mount, &s(":"), &root, &s(":ro")]));

    match options.tiktoken_host_path.filter(|p| !p.is_empty()) {
        Some(path) => {
            cmd.push(s("-v"));
            cmd.push(joined(&[path, &s(&format!(":{}:ro", CONTAINER_ENCODINGS))]));
            for env in [
                format!("TIKTOKEN_ENCODINGS_BASE={}", CONTAINER_ENCODINGS),
                format!("TIKTOKEN_RS_CACHE_DIR={}", CONTAINER_ENCODINGS),
                "TIKTOKEN_ENABLED=1".to_string(),
            ] {
                cmd.push(s("-e"));
                cmd.push(s(&env));
            }
        }
        None => {
            if needs_tiktoken(candidate)? {
                warnings.push(s("[run_trial] WARN: candidate가 tiktoken(harmony/o200k)을 요구하나 tiktoken_host_path 미설정 — 폐쇄망 스모크 실패 가능(config.tiktoken_host_path 설정 권장).\n"));
            }
        }
    }

    if let Some(root) = options.jit_cache_root.filter(|r| !r.is_empty()) {
        for sub in ["vllm", "flashinfer"] {
            let host_dir = joined(&[root, &s("/"), &s(sub)]);
            if let Ok(bytes) = host_dir.to_surrogateescape_bytes() {
                use std::os::unix::ffi::OsStringExt;
                let _ = std::fs::create_dir_all(std::ffi::OsString::from_vec(bytes));
            }
            cmd.push(s("-v"));
            cmd.push(joined(&[&host_dir, &s(&format!(":/root/.cache/{}", sub))]));
        }
    }
    if let Some(jobs) = options.max_jobs.filter(|j| *j != 0) {
        cmd.push(s("-e"));
        cmd.push(s(&format!("MAX_JOBS={}", jobs)));
    }

    let backend = get(candidate, "attention_backend")?;
    if backend.is_truthy() {
        cmd.push(s("-e"));
        cmd.push(joined(&[&s("VLLM_ATTENTION_BACKEND="), &backend.display_text()]));
    }
    let extra = get(candidate, "extra_env")?;
    if extra.is_truthy() {
        match extra.peel() {
            PyValue::Dict(map) => {
                for (_, key, value) in map.keyed_entries() {
                    cmd.push(s("-e"));
                    cmd.push(joined(&[key, &s("="), &value.display_text()]));
                }
            }
            other => {
                return Err(Failure::new(
                    "AttributeError",
                    &format!("'{}' object has no attribute 'items'", other.type_name()),
                ))
            }
        }
    }
    for (k, v) in crate::recipe_set::serve_env_pairs(&get(candidate, "serve_env")?)? {
        cmd.push(s("-e"));
        cmd.push(joined(&[&k, &s("="), &v]));
    }
    cmd.push(options.image.clone());
    cmd.extend(build_serve_args(candidate)?);
    Ok(cmd)
}

/// `_audit_emitted(candidate, docker_cmd)`: `Err` is the `RuntimeError` of a serve field missing from the command.
pub fn audit_emitted(candidate: &PyValue, docker_cmd: &[PyStr]) -> Res<()> {
    let mut cmd_str = PyStr::default();
    for (i, part) in docker_cmd.iter().enumerate() {
        if i > 0 {
            cmd_str.push_str(" ");
        }
        cmd_str.push_py(part);
    }
    let caps = capabilities(candidate)?;
    let mut flags: Vec<(&'static str, PyStr)> = [
        ("gpu_memory_utilization", "--gpu-memory-utilization"),
        ("max_model_len", "--max-model-len"),
        ("batch", "--max-num-seqs"),
        ("max_num_batched_tokens", "--max-num-batched-tokens"),
        ("kv_cache_memory_bytes", "--kv-cache-memory-bytes"),
        ("kv_cache_quant", "--kv-cache-dtype"),
        ("attention_backend", "VLLM_ATTENTION_BACKEND="),
        ("moe_backend", "--moe-backend"),
    ]
    .iter()
    .map(|(k, f)| (*k, s(f)))
    .collect();
    for (k, _) in crate::recipe_set::serve_env_pairs(&get(candidate, "serve_env")?)? {
        flags.push(("serve_env", joined(&[&k, &s("=")])));
    }
    if truthy(candidate, "enforce_eager")? {
        flags.push(("enforce_eager", s("--enforce-eager")));
    }
    if truthy(candidate, "language_model_only")? {
        flags.push(("language_model_only", s("--language-model-only")));
    }
    if truthy(candidate, "gdn_prefill_backend")? {
        flags.push(("gdn_prefill_backend", s("--gdn-prefill-backend")));
    }
    let quant = get(candidate, "quantization")?;
    if quant.is_truthy() && !crate::pyexpr::lower(&quant.display_text()).equals("none") {
        flags.push(("quantization", s("--quantization")));
    }
    if truthy(candidate, "tool_call_parser")? && get(&caps, "tool_call")?.is_truthy() {
        flags.push(("tool_call_parser", s("--tool-call-parser")));
    }
    if truthy(candidate, "reasoning_parser")? && get(&caps, "reasoning")?.is_truthy() {
        flags.push(("reasoning_parser", s("--reasoning-parser")));
    }
    let mut missing: Vec<String> = Vec::new();
    for (key, flag) in &flags {
        if !is_none(&get(candidate, key)?) && !contains(&cmd_str, flag) {
            missing.push(format!("{}(기대 {})", key, flag.to_utf8().unwrap_or_default()));
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(Failure::new(
        "RuntimeError",
        &format!(
            "emit-audit 실패 — candidate 의 비-null serve 필드가 cmd 에 누락됨: {} | 이 가드는 gmu 미emit 같은 회귀류 클래스를 차단한다(§9.3).",
            missing.join(", ")
        ),
    ))
}

fn contains(haystack: &PyStr, needle: &PyStr) -> bool {
    let (h, n) = (haystack.code_points(), needle.code_points());
    n.is_empty() || h.windows(n.len()).any(|w| w == n)
}

/// The liveness reading of one `docker inspect`: `running` | `dead` | `absent` | `unknown`.
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerState {
    pub state: &'static str,
    pub detail: String,
    pub exit_code: Option<String>,
    pub oom_killed: Option<bool>,
}

fn clip(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// `parse_inspect_state(returncode, stdout, stderr)`.
pub fn parse_inspect_state(returncode: i32, stdout: &str, stderr: &str) -> ContainerState {
    let out = stdout.trim();
    let err = stderr.trim();
    let plain = |state: &'static str, detail: String| ContainerState {
        state,
        detail,
        exit_code: None,
        oom_killed: None,
    };
    if returncode != 0 {
        let low = err.to_lowercase();
        if low.contains("no such object") || low.contains("no such container") {
            return plain("absent", clip(err, 200));
        }
        let detail = if err.is_empty() {
            format!("rc={}", returncode)
        } else {
            clip(err, 200)
        };
        return plain("unknown", detail);
    }
    let parts: Vec<&str> = out.split_whitespace().collect();
    if parts.is_empty() {
        return plain("unknown", "빈 inspect 출력".to_string());
    }
    let state = match parts[0].to_lowercase().as_str() {
        "true" => "running",
        "false" => "dead",
        _ => "unknown",
    };
    ContainerState {
        state,
        detail: out.to_string(),
        exit_code: parts.get(1).map(|p| p.to_string()),
        oom_killed: parts.get(2).map(|p| p.to_lowercase() == "true"),
    }
}

/// `_container_state(name)`: one `docker inspect` (20 s limit); a missing CLI or a timeout is `unknown`.
pub fn container_state(name: &str) -> ContainerState {
    let child = std::process::Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}} {{.State.ExitCode}} {{.State.OOMKilled}}", name])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            return ContainerState {
                state: "unknown",
                detail: format!("OSError: {}", error),
                exit_code: None,
                oom_killed: None,
            }
        }
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (mut out, mut err) = (String::new(), String::new());
                if let Some(mut pipe) = child.stdout.take() {
                    let mut bytes = Vec::new();
                    let _ = pipe.read_to_end(&mut bytes);
                    out = String::from_utf8_lossy(&bytes).into_owned();
                }
                if let Some(mut pipe) = child.stderr.take() {
                    let mut bytes = Vec::new();
                    let _ = pipe.read_to_end(&mut bytes);
                    err = String::from_utf8_lossy(&bytes).into_owned();
                }
                return parse_inspect_state(status.code().unwrap_or(-1), &out, &err);
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return ContainerState {
                    state: "unknown",
                    detail: "TimeoutExpired: docker inspect".to_string(),
                    exit_code: None,
                    oom_killed: None,
                };
            }
        }
    }
}

/// What `_poll_health` returns (a dict in the original).
#[derive(Clone, Debug)]
pub struct HealthWait {
    pub healthy: bool,
    pub outcome: &'static str,
    pub waited_s: f64,
    pub polls: i64,
    pub container_state: Option<&'static str>,
    pub container_detail: Option<String>,
    pub unknown_polls: i64,
}

impl HealthWait {
    pub fn to_value(&self) -> PyValue {
        let mut d = PyDict::new();
        d.set("healthy", PyValue::Bool(self.healthy));
        d.set("outcome", PyValue::str(self.outcome));
        d.set("waited_s", PyValue::Float((self.waited_s * 1000.0).round() / 1000.0));
        d.set("polls", PyValue::int(self.polls));
        d.set(
            "container_state",
            self.container_state.map_or(PyValue::None, PyValue::str),
        );
        d.set(
            "container_detail",
            self.container_detail.as_deref().map_or(PyValue::None, PyValue::str),
        );
        d.set("unknown_polls", PyValue::int(self.unknown_polls));
        PyValue::Dict(d)
    }
}

/// One `GET /health` on the loopback port; true only for HTTP 200 (connection refused and timeouts are "not ready yet").
fn health_ok(port: i64) -> bool {
    let Ok(mut addrs) = format!("127.0.0.1:{}", port).to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(5)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let request = format!(
        "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n",
        port
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut head = [0u8; 32];
    let mut got = 0;
    while got < 12 {
        match stream.read(&mut head[got..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n,
        }
    }
    got >= 12 && (&head[..9] == b"HTTP/1.1 " || &head[..9] == b"HTTP/1.0 ") && head[9..12] == *b"200"
}

/// `_poll_health(port, timeout, container_name)`: poll `/health` until HTTP 200, leaving at once when the container is dead or
/// absent. `state_of` is the liveness probe (`container_state` in production) and `interval` the poll gap; `log` receives the
/// stderr lines of the original.
pub fn poll_health(
    port: i64,
    timeout: f64,
    container_name: Option<&str>,
    state_of: &mut dyn FnMut(&str) -> ContainerState,
    interval: f64,
    log: &mut dyn FnMut(&str),
) -> HealthWait {
    let started = Instant::now();
    let deadline = started + Duration::from_secs_f64(timeout.max(0.0));
    let mut polls = 0i64;
    let mut unknown_polls = 0i64;
    let mut last_state: Option<&'static str> = None;
    let mut last_detail: Option<String> = None;
    let done = |healthy: bool, outcome: &'static str, polls, last_state, last_detail, unknown_polls| HealthWait {
        healthy,
        outcome,
        waited_s: started.elapsed().as_secs_f64(),
        polls,
        container_state: last_state,
        container_detail: last_detail,
        unknown_polls,
    };
    while Instant::now() < deadline {
        polls += 1;
        if health_ok(port) {
            return done(true, "healthy", polls, last_state, last_detail, unknown_polls);
        }
        if let Some(name) = container_name.filter(|n| !n.is_empty()) {
            let info = state_of(name);
            last_state = Some(info.state);
            last_detail = Some(info.detail.clone());
            if info.state == "dead" || info.state == "absent" {
                log(&format!(
                    "[run_trial] 컨테이너 사망 감지({}) — /health 폴링 즉시 중단 (경과 {:.1}s / 타임아웃 {:.0}s). detail={}\n",
                    info.state,
                    started.elapsed().as_secs_f64(),
                    timeout,
                    info.detail
                ));
                return done(false, "container_died", polls, last_state, last_detail, unknown_polls);
            }
            if info.state == "unknown" {
                unknown_polls += 1;
                if unknown_polls == 1 || unknown_polls % 20 == 0 {
                    log(&format!(
                        "[run_trial] ⚠ 컨테이너 생존조회 실패({}회째) — 폴링 계속(위양성 방지). {}\n",
                        unknown_polls, info.detail
                    ));
                }
            }
        }
        std::thread::sleep(Duration::from_secs_f64(interval));
    }
    done(false, "timeout", polls, last_state, last_detail, unknown_polls)
}

/// The record `host_floor_gmu_cap` returns (a dict in the original).
#[derive(Clone, Debug)]
pub struct GmuCap {
    pub gmu: Option<f64>,
    pub applied: bool,
    pub reason: String,
    pub cap: Option<f64>,
    pub requested: Option<f64>,
    pub mem_total_mib: Option<i64>,
    pub mem_available_mib: Option<i64>,
    pub floor_mib: i64,
    pub headroom_mib: i64,
    pub weights_mib: Option<i64>,
    pub overhead_mib: Option<i64>,
}

impl GmuCap {
    pub fn to_value(&self) -> PyValue {
        let f = |v: Option<f64>| v.map_or(PyValue::None, PyValue::Float);
        let i = |v: Option<i64>| v.map_or(PyValue::None, PyValue::int);
        let mut d = PyDict::new();
        d.set("gmu", f(self.gmu));
        d.set("applied", PyValue::Bool(self.applied));
        d.set("reason", PyValue::str(&self.reason));
        d.set("cap", f(self.cap));
        d.set("requested", f(self.requested));
        d.set("mem_total_mib", i(self.mem_total_mib));
        d.set("mem_available_mib", i(self.mem_available_mib));
        d.set("floor_mib", PyValue::int(self.floor_mib));
        d.set("headroom_mib", PyValue::int(self.headroom_mib));
        d.set("weights_mib", i(self.weights_mib));
        d.set("overhead_mib", i(self.overhead_mib));
        PyValue::Dict(d)
    }
}

/// `host_floor_gmu_cap(...)`: cap a measuring trial's gmu to the host safety floor (never lowering the floor).
pub fn host_floor_gmu_cap(
    requested_gmu: Option<f64>,
    mem_total_mib: Option<i64>,
    mem_available_mib: Option<i64>,
    floor_mib: i64,
    weights_mib: Option<i64>,
    overhead_mib: Option<i64>,
) -> GmuCap {
    let mut info = GmuCap {
        gmu: requested_gmu,
        applied: false,
        reason: String::new(),
        cap: None,
        requested: requested_gmu,
        mem_total_mib,
        mem_available_mib,
        floor_mib,
        headroom_mib: HOST_FLOOR_HEADROOM_MIB,
        weights_mib,
        overhead_mib,
    };
    let Some(requested) = requested_gmu else {
        info.reason = "gmu_unset".into();
        return info;
    };
    let (total, avail) = match (mem_total_mib, mem_available_mib) {
        (Some(t), Some(a)) if t != 0 && a != 0 && t > 0 => (t, a),
        _ => {
            info.reason = "meminfo_unavailable".into();
            return info;
        }
    };
    let cap = (avail - floor_mib - HOST_FLOOR_HEADROOM_MIB) as f64 / total as f64;
    let cap = ((cap * 10000.0).trunc() as i64) as f64 / 10000.0;
    info.cap = Some(cap);
    if cap >= requested {
        info.reason = "not_needed".into();
        return info;
    }
    if cap <= 0.0 {
        info.reason = "cap_nonpositive".into();
        return info;
    }
    let Some(weights) = weights_mib else {
        info.reason = "weights_unknown".into();
        return info;
    };
    let allowed = cap * total as f64;
    let need = weights + overhead_mib.unwrap_or(0) + MIN_MEASURE_KV_MIB;
    if allowed < need as f64 {
        info.reason = format!(
            "cap_infeasible(allowed={}MiB < need={}MiB)",
            allowed.trunc() as i64,
            need
        );
        return info;
    }
    info.gmu = Some(cap);
    info.applied = true;
    info.reason = "capped_to_host_floor".into();
    info
}

/// `_mock_result(...)`: the dry-run/mock record; it never claims measured provenance.
pub fn mock_result(
    candidate: &PyValue,
    trial_number: i64,
    vllm_profile: PyValue,
    functional: PyValue,
    log_path: &PyStr,
    provenance: &str,
) -> Res<PyValue> {
    if provenance == PROVENANCE_MEASURED {
        return Err(Failure::new(
            "ValueError",
            "mock result must never claim measured provenance",
        ));
    }
    let mut d = PyDict::new();
    d.set("trial_number", PyValue::int(trial_number));
    d.set("candidate", candidate.clone());
    d.set("load_ok", PyValue::Bool(!is_none(&vllm_profile)));
    d.set("vllm_profile", vllm_profile);
    d.set("functional", functional);
    d.set("log_path", PyValue::Str(log_path.clone()));
    d.set("error_excerpt", PyValue::None);
    d.set("provenance", PyValue::str(provenance));
    d.set("health_wait", PyValue::None);
    Ok(PyValue::Dict(d))
}

/// `_self_test()`: the report line on success, the failure lines otherwise.
pub fn self_test(log_lines: &mut String) -> Result<String, Vec<String>> {
    let mut failures: Vec<String> = Vec::new();
    fn check(failures: &mut Vec<String>, name: &str, ok: bool, detail: String) {
        if !ok {
            failures.push(format!("{}: {}", name, detail));
        }
    }
    // A port nothing listens on.
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map(|l| l.local_addr().map(|a| a.port()));
        match listener {
            Ok(Ok(p)) => p as i64,
            _ => 1,
        }
    };
    let mut log_buf = String::new();
    let mut quiet = |line: &str| log_buf.push_str(line);
    let mut never = |_: &str| -> ContainerState { unreachable_state() };
    let r = poll_health(port, 0.15, None, &mut never, 0.01, &mut quiet);
    check(&mut failures, "C1.healthy", !r.healthy, format!("got={}", r.healthy));
    check(&mut failures, "C1.outcome", r.outcome == "timeout", format!("got={}", r.outcome));

    let mut dead = |_: &str| ContainerState { state: "dead", detail: "false 137 true".into(), exit_code: None, oom_killed: None };
    let r = poll_health(port, 30.0, Some("probe"), &mut dead, 0.01, &mut quiet);
    check(&mut failures, "C2.outcome", r.outcome == "container_died", format!("got={}", r.outcome));
    check(&mut failures, "C2.healthy", !r.healthy, format!("got={}", r.healthy));
    check(&mut failures, "C2.container_state", r.container_state == Some("dead"), format!("got={:?}", r.container_state));
    if r.waited_s >= 5.0 {
        failures.push(format!("C2.early_exit: {:.2}s 소요 — 즉시 이탈이 아니다", r.waited_s));
    }
    let mut absent = |_: &str| ContainerState { state: "absent", detail: "No such object".into(), exit_code: None, oom_killed: None };
    let r = poll_health(port, 30.0, Some("probe"), &mut absent, 0.01, &mut quiet);
    check(&mut failures, "C3.outcome", r.outcome == "container_died", format!("got={}", r.outcome));
    let mut unknown = |_: &str| ContainerState { state: "unknown", detail: "daemon busy".into(), exit_code: None, oom_killed: None };
    let r = poll_health(port, 0.15, Some("probe"), &mut unknown, 0.01, &mut quiet);
    check(&mut failures, "C4.outcome", r.outcome == "timeout", format!("got={}", r.outcome));
    if r.unknown_polls < 1 {
        failures.push("C4.unknown_polls: 조회 실패가 기록되지 않았다(침묵 금지 위반)".into());
    }
    let state = |rc, out: &str, err: &str| parse_inspect_state(rc, out, err);
    fn c6(failures: &mut Vec<String>, name: &str, got: &str, want: &str) {
        if got != want {
            failures.push(format!("{}: got={:?} want={:?}", name, got, want));
        }
    }
    c6(&mut failures, "C6.running", state(0, "true 0 false", "").state, "running");
    c6(&mut failures, "C6.dead", state(0, "false 137 false", "").state, "dead");
    if state(0, "false 137 true", "").oom_killed != Some(true) {
        failures.push("C6.oom: got!=True".into());
    }
    c6(&mut failures, "C6.absent", state(1, "", "Error: No such object: x").state, "absent");
    c6(&mut failures, "C6.unknown_rc", state(1, "", "daemon down").state, "unknown");
    c6(&mut failures, "C6.unknown_empty", state(0, "", "").state, "unknown");

    let (total, weights, floor) = (124610i64, 52989i64, HOST_FLOOR_MIB);
    let avail = 119215i64;
    let b = host_floor_gmu_cap(Some(0.90), Some(total), Some(avail), floor, Some(weights), Some(FIXTURE_OVERHEAD_MIB));
    if !b.applied {
        failures.push("B1.applied: got=false want=true".into());
    }
    let bg = b.gmu.unwrap_or(f64::NAN);
    if !(bg < 0.90) {
        failures.push(format!("B1.cap: 캡이 요청값을 낮추지 않았다({})", bg));
    }
    let landed = avail as f64 - bg * total as f64;
    if !(landed >= floor as f64) {
        failures.push(format!("B1.floor: 캡 후 착지 {:.0}MiB < 바닥 {}MiB — 캡이 제 목적을 못 한다", landed, floor));
    }
    if b.floor_mib != floor {
        failures.push("B2.floor_untouched".into());
    }
    let b3 = host_floor_gmu_cap(Some(0.50), Some(total), Some(avail), floor, Some(weights), Some(FIXTURE_OVERHEAD_MIB));
    if b3.applied || b3.reason != "not_needed" || b3.gmu != Some(0.50) {
        failures.push(format!("B3: applied={} reason={} gmu={:?}", b3.applied, b3.reason, b3.gmu));
    }
    let b4 = host_floor_gmu_cap(Some(0.90), Some(total), Some(70000), floor, Some(weights), Some(FIXTURE_OVERHEAD_MIB));
    if b4.applied || !b4.reason.starts_with("cap_infeasible") {
        failures.push(format!("B4: applied={} reason={}", b4.applied, b4.reason));
    }
    for (name, g) in [
        ("weights_unknown", host_floor_gmu_cap(Some(0.90), Some(total), Some(avail), floor, None, None)),
        ("meminfo_unavailable", host_floor_gmu_cap(Some(0.90), None, None, floor, None, None)),
        ("gmu_unset", host_floor_gmu_cap(None, Some(total), Some(avail), floor, None, None)),
        ("cap_nonpositive", host_floor_gmu_cap(Some(0.90), Some(total), Some(floor), floor, Some(1), None)),
    ] {
        if g.reason != name || g.applied {
            failures.push(format!("B5.{}: reason={} applied={}", name, g.reason, g.applied));
        }
        if g.gmu != g.requested {
            failures.push(format!("B5.{}: applied=False 인데 gmu 가 바뀌었다", name));
        }
    }
    for (name, g) in [("B3", &b3), ("B4", &b4)] {
        if !g.applied && g.gmu != g.requested {
            failures.push(format!("{}: applied=False 인데 gmu 가 바뀌었다", name));
        }
    }
    let plan_literal = (total - floor - weights - FIXTURE_OVERHEAD_MIB) as f64 / total as f64;
    if !(plan_literal * (total as f64) < weights as f64) {
        failures.push("B6: plan 리터럴 식이 weights 를 담는다".into());
    }
    if !(bg > plan_literal) {
        failures.push(format!("B6: 교정된 캡({:.4})이 리터럴 식({:.4})보다 크지 않다", bg, plan_literal));
    }
    drop(quiet);
    log_lines.push_str(&log_buf);
    if failures.is_empty() {
        Ok("[run_trial --self-test] OK — C1~C6(생존검사) · B1~B6(gmu 캡) 통과 (plan_26082223 결함 C·B)\n".to_string())
    } else {
        Err(failures)
    }
}

fn unreachable_state() -> ContainerState {
    ContainerState { state: "unknown", detail: "probe called without container name".into(), exit_code: None, oom_killed: None }
}
