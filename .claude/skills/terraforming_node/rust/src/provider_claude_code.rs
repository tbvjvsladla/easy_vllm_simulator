//! Port of `providers/claude_code.py` — the Claude Code provider adapter (v1). The ONLY place that emits Claude CLI
//! syntax (`claude -p`, `--model sonnet`, `--output-format json`); `agent_control --self-test` checks that boundary
//! against this file's source. The orchestrator calls `invoke` / `build_argv` / `reasoning_violations` /
//! `classify_runner_failure` and reads the tables (`SUPPORTS`, `RUNNER_ALIASES`, `BACKEND_TO_BINARY`,
//! `BACKEND_DEFAULT_MODEL`). The model is the caller's declaration; the model that ran is RECORDED (`model_used`).
//! Same argv, result envelopes (key order), stderr diagnostics and classification as the original.

use crate::agent_control::process::{subprocess_run, Completed, RunError, RunFn};
use crate::agent_control::pyhelp::{
    as_int, dict, dumps_sorted_unicode, dumps_unicode, get, has, head_chars, int, is_plain_int, py_in, repr,
    repr_str, s, shlex_quote, str_of, tail_chars,
};
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::time::Instant;

pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_EXECUTION_FAILED: &str = "execution_failed";
pub const STATUS_MALFORMED_OUTPUT: &str = "malformed_output";
pub const STATUS_TIMEOUT: &str = "timeout";
pub const EXIT_SUCCESS: i64 = 0;
pub const EXIT_EXECUTION_FAILED: i64 = 4;
pub const EXIT_MALFORMED_OUTPUT: i64 = 5;
pub const EXIT_TIMEOUT: i64 = 124;
pub const PROVIDER_NAME: &str = "claude_code";
pub const REASON_RUNNER_UNAVAILABLE: &str = "RUNNER_UNAVAILABLE";

/// `SUPPORTS`.
pub fn supports() -> PyValue {
    dict(vec![
        ("schema_versions", PyValue::List(vec![int(1)])),
        ("budget_units", PyValue::List(vec![s("native_turns")])),
        ("native_turn_limit", PyValue::Bool(true)),
        ("resume", s("explicit_native_id")),
        ("structured_output", s("json")),
        ("capabilities_mode", s("automatic_approval_list")),
    ])
}

/// `BACKEND_TO_BINARY` (closed list; the request schema's `backend` enum must equal it).
pub const BACKEND_TO_BINARY: &[(&str, &str)] = &[
    ("anthropic", "claude"),
    ("kimi", "kimi-claude"),
    ("minimax", "minimax-claude"),
    ("meta", "meta-claude"),
    ("openai", "openai-claude"),
    ("claude-sub", "claude-sub"),
];

/// `BACKEND_DEFAULT_MODEL`: the model token is backend-dependent.
pub const BACKEND_DEFAULT_MODEL: &[(&str, &str)] = &[
    ("anthropic", "sonnet"),
    ("kimi", "k3[1m]"),
    ("minimax", "MiniMax-M3"),
    ("meta", "muse-spark-1.3"),
    ("openai", "gpt-5.6-sol"),
    ("claude-sub", "sonnet"),
];

/// `RUNNER_ALIASES`: alias -> (backend, model). Closed tripwire list.
pub const RUNNER_ALIASES: &[(&str, (&str, &str))] = &[
    ("opus", ("anthropic", "opus")),
    ("sonnet", ("anthropic", "sonnet")),
    ("sonnet-5.5", ("anthropic", "claude-sonnet-5-5")),
    ("haiku", ("anthropic", "haiku")),
    ("kimi-claude", ("kimi", "k3[1m]")),
    ("minimax-claude", ("minimax", "MiniMax-M3")),
    ("meta-claude", ("meta", "muse-spark-1.3")),
    ("openai-claude", ("openai", "gpt-5.6-sol")),
    ("claude-sub-sonnet", ("claude-sub", "sonnet")),
    ("claude-sub-deepseek", ("claude-sub", "deepseek-flash")),
    ("claude-sub-minimax", ("claude-sub", "minimax-m3.1-flash")),
    ("claude-sub-deepseek-pro", ("claude-sub", "deepseek-pro")),
    ("claude-sub-kimi", ("claude-sub", "kimi-k3.1-pro")),
    ("claude-sub-minimax-m3", ("claude-sub", "minimax-m3")),
];

pub const EFFORT_LEVELS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

pub fn backend_binary(backend: &str) -> Option<&'static str> {
    BACKEND_TO_BINARY.iter().find(|(b, _)| *b == backend).map(|(_, x)| *x)
}

pub fn backend_default_model(backend: &str) -> Option<&'static str> {
    BACKEND_DEFAULT_MODEL.iter().find(|(b, _)| *b == backend).map(|(_, x)| *x)
}

/// `reasoning_violations(request)`.
pub fn reasoning_violations(request: &PyValue) -> Vec<String> {
    match get(request, "reasoning_effort") {
        None | Some(PyValue::None) => vec![],
        Some(e) => {
            let mut ok: Vec<PyValue> = EFFORT_LEVELS.iter().map(|x| s(x)).collect();
            ok.push(s("default"));
            if py_in(e, &ok) {
                vec![]
            } else {
                vec!["$.reasoning_effort: Claude supports default, low, medium, high, xhigh, max".into()]
            }
        }
    }
}

pub const TERMINAL_REASON_API_ERROR: &str = "api_error";
const ROTATE_ON_STATUS: &[i64] = &[401, 402, 403, 408, 429, 500, 502, 503, 504, 529];
pub const RC_MISSING_BINARY: i32 = 127;
pub const RC_SSH_TRANSPORT: i32 = 255;

/// `classify_runner_failure(payload, returncode, transport)`: evidence dict for a runner-plane failure, else `None`.
pub fn classify_runner_failure(payload: Option<&PyValue>, returncode: i32, transport: &str) -> Option<PyValue> {
    if transport == "ssh" && returncode == RC_SSH_TRANSPORT {
        return None;
    }
    if returncode == RC_MISSING_BINARY {
        return Some(dict(vec![
            ("signal", s("missing_binary")),
            ("returncode", int(returncode.into())),
            ("provenance", s("measured")),
            ("rotate", PyValue::Bool(true)),
            ("detail", s("러너 바이너리를 실행할 수 없다(원격 PATH 부재)")),
        ]));
    }
    let payload = match payload.map(|p| p.peel()) {
        Some(p @ PyValue::Dict(_)) => p,
        _ => return None,
    };
    if !matches!(get(payload, "terminal_reason"), Some(PyValue::Str(t)) if t.equals(TERMINAL_REASON_API_ERROR)) {
        return None;
    }
    let status = get(payload, "api_error_status").cloned().unwrap_or(PyValue::None);
    let rc = int(returncode.into());
    if !has(payload, "api_error_status") || !(matches!(status, PyValue::None) || matches!(status, PyValue::Int(_))) {
        return Some(dict(vec![
            ("signal", s("unclassified_api_error")),
            ("api_error_status", status),
            ("rotate", PyValue::Bool(false)),
            ("provenance", s("measured")),
            ("returncode", rc),
            ("detail", s("API error status is missing or has an unsupported type")),
        ]));
    }
    let code = as_int(Some(&status));
    let rotating = match &status {
        PyValue::None => true,
        _ => code.is_some_and(|c| ROTATE_ON_STATUS.contains(&c)),
    };
    if !rotating {
        return Some(dict(vec![
            ("signal", s("api_error")),
            ("api_error_status", status),
            ("rotate", PyValue::Bool(false)),
            ("provenance", s("doc")),
            ("returncode", rc),
            ("detail", s("요청 거절 또는 미등록 상태 — 회전 근거가 없다")),
        ]));
    }
    let measured = matches!(status, PyValue::None) || code == Some(401);
    let detail = match &status {
        PyValue::None => "백엔드에 도달하지 못했다(HTTP 응답 없음)".to_string(),
        other => format!("백엔드가 HTTP {} 로 거절했다", str_of(other)),
    };
    let message = match get(payload, "result") {
        Some(r @ PyValue::Str(_)) => r.clone(),
        _ => PyValue::None,
    };
    Some(dict(vec![
        ("signal", s("api_error")),
        ("api_error_status", status),
        ("rotate", PyValue::Bool(true)),
        ("provenance", s(if measured { "measured" } else { "doc" })),
        ("returncode", rc),
        ("detail", s(&detail)),
        ("message", message),
    ]))
}

/// `CAPABILITY_TO_TOOL`.
fn capability_tools(name: &str) -> &'static [&'static str] {
    match name {
        "read" => &["Read"],
        "execute" => &["Bash"],
        "edit" => &["Edit"],
        "write" => &["Write"],
        "search" => &["WebSearch", "WebFetch"],
        _ => &[],
    }
}

fn text_of(v: Option<&PyValue>) -> String {
    v.map(str_of).unwrap_or_default()
}

fn capture(request: &PyValue) -> bool {
    matches!(get(request, "capture_native_events"), Some(PyValue::Bool(true)))
}

/// `_inner_argv(request)`.
pub fn inner_argv(request: &PyValue) -> Vec<String> {
    let caps: Vec<String> = match get(request, "capabilities") {
        Some(PyValue::List(l)) => l.iter().map(str_of).collect(),
        _ => vec![],
    };
    let allowed: Vec<&str> = caps.iter().flat_map(|c| capability_tools(c).iter().copied()).collect();
    let backend = match get(request, "backend") {
        Some(b) if b.is_truthy() => str_of(b),
        _ => "anthropic".into(),
    };
    let runner = backend_binary(&backend).unwrap_or("claude").to_string();
    let mut args = vec![
        "-p".to_string(),
        text_of(get(request, "task")),
        "--model".into(),
        text_of(get(request, "model")),
        "--output-format".into(),
        if capture(request) { "stream-json".into() } else { "json".into() },
        "--max-turns".into(),
        text_of(get(request, "max_turns")),
        "--allowedTools".into(),
        allowed.join(","),
    ];
    if capture(request) {
        args.push("--verbose".into());
    }
    match get(request, "reasoning_effort") {
        None | Some(PyValue::None) => {}
        Some(PyValue::Str(e)) if e.equals("default") => {}
        Some(e) => {
            args.push("--effort".into());
            args.push(str_of(e));
        }
    }
    if let Some(PyValue::Str(r)) = get(request, "resume_session_id") {
        let r = crate::py::text(&r.strip());
        if !r.is_empty() {
            args.push("--resume".into());
            args.push(r);
        }
    }
    if backend == "anthropic" {
        let mut v = vec![runner];
        v.extend(args);
        return v;
    }
    let mut v = vec!["bash".to_string(), "-ic".into(), format!("{runner} \"$@\""), runner];
    v.extend(args);
    v
}

fn ssh_destination(target: &PyValue) -> String {
    let host = text_of(get(target, "host"));
    match get(target, "ssh_user") {
        Some(u) if u.is_truthy() => format!("{}@{host}", str_of(u)),
        _ => host,
    }
}

pub const SSH_HARDENING: &[&str] = &[
    "-F", "/dev/null", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "-o", "StrictHostKeyChecking=yes", "-o",
    "IdentitiesOnly=yes", "-o", "ForwardAgent=no", "-o", "ClearAllForwardings=yes", "-o", "RequestTTY=no",
];
pub const REMOTE_ENV: &[&str] = &["CLAUDE_CODE_RETRY_WATCHDOG=1"];

/// `build_argv(request)`; `Err` = the original's `ValueError` for an unsupported transport.
pub fn build_argv(request: &PyValue) -> Result<Vec<String>, String> {
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let inner = inner_argv(request);
    let transport = text_of(get(&target, "transport"));
    if transport == "local" {
        return Ok(inner);
    }
    if transport == "ssh" {
        let mut remote_inner = inner.iter().map(|t| shlex_quote(t)).collect::<Vec<_>>().join(" ");
        let positive = match get(request, "timeout_seconds") {
            Some(PyValue::Int(_)) | Some(PyValue::Float(_)) => {
                let t = get(request, "timeout_seconds").unwrap();
                agent_runtime_support::pyops::compare(
                    agent_runtime_support::pyops::Compare::Greater,
                    t,
                    &int(0),
                )
                .unwrap_or(false)
            }
            _ => false,
        };
        if positive {
            let t = match get(request, "timeout_seconds").unwrap() {
                PyValue::Float(f) => format!("{}", f.trunc() as i64),
                other => str_of(other),
            };
            remote_inner = format!("timeout {t} {remote_inner}");
        }
        remote_inner = format!("{} {remote_inner}", REMOTE_ENV.join(" "));
        let script = match get(&target, "work_dir") {
            Some(w) if w.is_truthy() => format!("cd {} && {remote_inner}", shlex_quote(&str_of(w))),
            _ => remote_inner,
        };
        let mut v: Vec<String> = vec!["ssh".into()];
        v.extend(SSH_HARDENING.iter().map(|x| x.to_string()));
        v.push("--".into());
        v.push(ssh_destination(&target));
        v.push(format!("bash -lc {}", shlex_quote(&script)));
        return Ok(v);
    }
    Err(format!("unsupported transport: {}", repr_str(&transport)))
}

/// `_diag`: one diagnosis line (+ provider tails) on standard error; never fails.
pub fn diag(request: &PyValue, code: &str, message: &str, stderr: Option<&str>, stdout: Option<&str>) {
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let role = get(&target, "role").map(str_of).unwrap_or_else(|| "None".into());
    let transport = get(&target, "transport").map(str_of).unwrap_or_else(|| "None".into());
    eprintln!("[agent-control] {code}: {message} (target={role}/{transport})");
    for (label, blob) in [("stderr", stderr), ("stdout", stdout)] {
        let t = crate::py::strip(blob.unwrap_or(""));
        if !t.is_empty() {
            eprintln!("[agent-control]   provider {label} tail: {}", tail_chars(t, 2000));
        }
    }
}

fn nonneg_int(v: Option<&PyValue>) -> PyValue {
    match v.map(|x| x.peel()) {
        Some(n @ PyValue::Int(_)) if !agent_runtime_support::pyops::compare(
            agent_runtime_support::pyops::Compare::Less, n, &int(0)).unwrap_or(true) => n.clone(),
        _ => PyValue::None,
    }
}

/// `_durations(payload)`.
fn durations(payload: Option<&PyValue>) -> (PyValue, PyValue) {
    match payload.map(|p| p.peel()) {
        Some(p @ PyValue::Dict(_)) => (nonneg_int(get(p, "duration_ms")), nonneg_int(get(p, "duration_api_ms"))),
        _ => (PyValue::None, PyValue::None),
    }
}

pub struct Res {
    pub model_used: Vec<String>,
    pub output: Option<String>,
    pub session_id: PyValue,
    pub num_turns: PyValue,
    pub budget_outcome: Option<&'static str>,
    pub duration_ms: PyValue,
    pub duration_api_ms: PyValue,
}

/// `_result(request, status=..., exit_code=..., reason_codes=..., ...)`.
pub fn result(request: &PyValue, status: &str, exit_code: i64, reasons: &[&str], r: Res) -> PyValue {
    dict(vec![
        ("schema_version", get(request, "schema_version").cloned().unwrap_or(int(1))),
        ("provider", s(PROVIDER_NAME)),
        ("intent", get(request, "intent").cloned().unwrap_or(PyValue::None)),
        ("status", s(status)),
        ("exit_code", int(exit_code)),
        ("model_requested", get(request, "model").cloned().unwrap_or(PyValue::None)),
        ("model_used", PyValue::List(r.model_used.iter().map(|m| s(m)).collect())),
        ("reason_codes", PyValue::List(reasons.iter().map(|x| s(x)).collect())),
        ("output", r.output.map_or(PyValue::None, |o| s(&o))),
        ("session_id", r.session_id),
        ("num_turns", r.num_turns),
        ("budget_outcome", r.budget_outcome.map_or(PyValue::None, s)),
        ("duration_ms", r.duration_ms),
        ("duration_api_ms", r.duration_api_ms),
    ])
}

fn res0() -> Res {
    Res {
        model_used: vec![],
        output: None,
        session_id: PyValue::None,
        num_turns: PyValue::None,
        budget_outcome: None,
        duration_ms: PyValue::None,
        duration_api_ms: PyValue::None,
    }
}

fn sess_turns(p: &PyValue) -> (PyValue, PyValue) {
    let sess = match get(p, "session_id") {
        Some(v @ PyValue::Str(_)) => v.clone(),
        _ => PyValue::None,
    };
    // `isinstance(x, int)` includes bool here (the original does not exclude it).
    let turns = match get(p, "num_turns") {
        Some(v @ (PyValue::Int(_) | PyValue::Bool(_))) => v.clone(),
        _ => PyValue::None,
    };
    (sess, turns)
}

fn runner_unavailable(request: &PyValue, evidence: &PyValue, payload: Option<&PyValue>, stderr: Option<&str>) -> PyValue {
    let p = match payload.map(|x| x.peel()) {
        Some(x @ PyValue::Dict(_)) => x.clone(),
        _ => dict(vec![]),
    };
    let (sess, turns) = sess_turns(&p);
    let (d, da) = durations(Some(&p));
    let backend = match get(request, "backend") {
        Some(b) if b.is_truthy() => str_of(b),
        _ => "anthropic".into(),
    };
    let model = get(request, "model").map(str_of).unwrap_or_else(|| "None".into());
    let detail = get(evidence, "detail").map(str_of).unwrap_or_else(|| "None".into());
    let rotate = get(evidence, "rotate").map(str_of).unwrap_or_else(|| "None".into());
    diag(
        request,
        "RUNNER_UNAVAILABLE",
        &format!("러너(backend={backend} model={model})가 실행하지 못했다 — {detail}. 회전 가능={rotate}"),
        stderr,
        None,
    );
    let note = format!("[runner_unavailable] {}", dumps_sorted_unicode(evidence));
    result(
        request,
        STATUS_EXECUTION_FAILED,
        EXIT_EXECUTION_FAILED,
        &[REASON_RUNNER_UNAVAILABLE],
        Res { output: Some(note), session_id: sess, num_turns: turns, duration_ms: d, duration_api_ms: da, ..res0() },
    )
}

/// `_native_stream(stdout)` -> (terminal result, events, malformed).
fn native_stream(stdout: &str) -> (Option<PyValue>, Vec<PyValue>, bool) {
    let mut events = vec![];
    let mut malformed = false;
    for line in crate::py::splitlines(stdout) {
        if crate::py::strip(&line).is_empty() {
            continue;
        }
        match pyjson::loads(&line) {
            Ok(ev @ PyValue::Dict(_)) if matches!(get(&ev, "type"), Some(PyValue::Str(_))) => events.push(ev),
            _ => malformed = true,
        }
    }
    let is_result = |e: &PyValue| matches!(get(e, "type"), Some(PyValue::Str(t)) if t.equals("result"));
    let results: Vec<&PyValue> = events.iter().filter(|e| is_result(e)).collect();
    if results.len() != 1 || events.is_empty() || !is_result(events.last().unwrap()) {
        malformed = true;
    }
    let terminal = if results.len() == 1 { Some(results[0].clone()) } else { None };
    (terminal, events, malformed)
}

/// `subprocess.run(..., text=True)` decoding: strict UTF-8 with universal newlines.
fn text_mode(b: &[u8]) -> Option<String> {
    let t = std::str::from_utf8(b).ok()?;
    Some(t.replace("\r\n", "\n").replace('\r', "\n"))
}

/// `invoke(request)` with the real process.
pub fn invoke(request: &PyValue) -> PyValue {
    invoke_with(request, &subprocess_run)
}

/// `invoke(request)` with an injectable `subprocess.run`.
pub fn invoke_with(request: &PyValue, run: RunFn) -> PyValue {
    let mut request = request.clone();
    let mut selection = None;
    let with_events = |mut result: PyValue, events: &[PyValue], req: &PyValue| {
        if capture(req) {
            crate::agent_control::pyhelp::set(&mut result, "raw_events", PyValue::List(events.to_vec()));
        }
        result
    };
    if matches!(get(&request, "reasoning_effort"), Some(PyValue::Str(e)) if e.equals("default")) {
        let started = Instant::now();
        let mut probe = request.clone();
        crate::agent_control::pyhelp::set(&mut probe, "task", s("/effort status"));
        crate::agent_control::pyhelp::set(&mut probe, "capture_native_events", PyValue::Bool(true));
        let probe = crate::agent_control::pyhelp::without(&probe, "reasoning_effort");
        let mut events = vec![];
        let policy = invoke_inner(&probe, Some(&mut events), run);
        let terminal = events
            .iter()
            .rev()
            .find(|e| matches!(get(e, "type"), Some(PyValue::Str(t)) if t.equals("result")))
            .cloned()
            .unwrap_or_else(|| dict(vec![]));
        let status = match get(&policy, "output") {
            Some(o) if o.is_truthy() => str_of(o),
            _ => String::new(),
        };
        let m = effort_match(crate::py::strip(&status));
        let resume = get(&request, "resume_session_id").cloned();
        let completed = matches!(get(&policy, "status"), Some(PyValue::Str(t)) if t.equals(STATUS_COMPLETED));
        let local_effort = matches!(get(&terminal, "local_command"), Some(PyValue::Str(t)) if t.equals("effort"));
        let resume_bad = match &resume {
            Some(r) if r.is_truthy() => !crate::agent_control::pyhelp::py_eq(
                get(&policy, "session_id").unwrap_or(&PyValue::None),
                r,
            ),
            _ => false,
        };
        if !completed || !local_effort || m.is_none() || resume_bad {
            if !completed {
                return with_events(policy, &events, &request);
            }
            diag(&request, "PROVIDER_RESULT_INVALID", "Native effort policy could not be resolved from /effort status", None, None);
            return with_events(
                result(&request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["PROVIDER_RESULT_INVALID"], res0()),
                &events,
                &request,
            );
        }
        let (mode, current) = m.unwrap();
        let displayed = match current {
            Some(c) => PyValue::str(&c),
            None if mode == "auto" => PyValue::None,
            None => PyValue::str(&mode),
        };
        selection = Some(dict(vec![
            ("requested", s("default")),
            ("resolved", displayed),
            ("source", s("target-native:/effort status")),
            ("native_status", s(&status)),
            ("mode", s("native-default")),
            ("upstream_observed", PyValue::None),
        ]));
        let timeout = get(&request, "timeout_seconds").and_then(|t| match t {
            PyValue::Int(_) => as_int(Some(t)).map(|x| x as f64),
            PyValue::Float(f) => Some(*f),
            _ => None,
        });
        let remaining = timeout.unwrap_or(0.0) - started.elapsed().as_secs_f64();
        if remaining < 1.0 {
            return with_events(
                result(&request, STATUS_TIMEOUT, EXIT_TIMEOUT, &["TIMEOUT"],
                       Res { budget_outcome: Some("external_interruption"), ..res0() }),
                &events,
                &request,
            );
        }
        crate::agent_control::pyhelp::set(&mut request, "timeout_seconds", int((remaining as i64).max(1)));
    }
    let mut events = vec![];
    let cap = capture(&request);
    let mut res = invoke_inner(&request, if cap { Some(&mut events) } else { None }, run);
    if let Some(sel) = selection {
        crate::agent_control::pyhelp::set(&mut res, "reasoning_selection", sel);
    }
    if cap {
        crate::agent_control::pyhelp::set(&mut res, "raw_events", PyValue::List(events));
    }
    res
}

/// `re.fullmatch(r"Effort level: (auto|low|medium|high|xhigh|max)(?: \(currently (low|...|max)\))?", text)`.
fn effort_match(text: &str) -> Option<(String, Option<String>)> {
    let rest = text.strip_prefix("Effort level: ")?;
    let modes = ["auto", "low", "medium", "high", "xhigh", "max"];
    for m in modes {
        if let Some(after) = rest.strip_prefix(m) {
            if after.is_empty() {
                return Some((m.into(), None));
            }
            if let Some(inner) = after.strip_prefix(" (currently ").and_then(|x| x.strip_suffix(')')) {
                if modes[1..].contains(&inner) {
                    return Some((m.into(), Some(inner.into())));
                }
            }
        }
    }
    None
}

fn is_type(v: &PyValue, t: &str) -> bool {
    matches!(get(v, "type"), Some(PyValue::Str(x)) if x.equals(t))
}

fn subtype_is(v: &PyValue, t: &str) -> bool {
    matches!(get(v, "subtype"), Some(PyValue::Str(x)) if x.equals(t))
}

fn opt_repr(v: Option<&PyValue>) -> String {
    v.map(repr).unwrap_or_else(|| "None".into())
}

/// `_invoke(request, observation)`: runs the provider binary; fail-closed (never raises).
fn invoke_inner(request: &PyValue, observation: Option<&mut Vec<PyValue>>, run: RunFn) -> PyValue {
    let argv = match build_argv(request) {
        Ok(a) => a,
        Err(_) => vec![],
    };
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let transport = text_of(get(&target, "transport"));
    let cwd = if transport == "local" {
        get(&target, "work_dir").filter(|w| !matches!(w, PyValue::None)).map(str_of)
    } else {
        None
    };
    let timeout = get(request, "timeout_seconds").and_then(|t| match t {
        PyValue::Int(_) => as_int(Some(t)).map(|x| x as f64),
        PyValue::Float(f) => Some(*f),
        _ => None,
    });
    let completed: Completed = match run(&argv, cwd.as_deref(), timeout) {
        Ok(c) => c,
        Err(RunError::Timeout { stdout }) => {
            if let Some(obs) = observation {
                let text = std::str::from_utf8(&stdout).ok().map(|t| t.replace("\r\n", "\n").replace('\r', "\n"));
                *obs = match text {
                    Some(t) => native_stream(&t).1,
                    None => vec![],
                };
            }
            diag(request, "TIMEOUT", &format!("provider exceeded timeout_seconds={}", opt_repr(get(request, "timeout_seconds"))), None, None);
            return result(request, STATUS_TIMEOUT, EXIT_TIMEOUT, &["TIMEOUT"],
                          Res { budget_outcome: Some("external_interruption"), ..res0() });
        }
        Err(RunError::Os(e)) => {
            let argv0 = argv.first().cloned().unwrap_or_default();
            if e.class == "FileNotFoundError" && e.filename == argv0 && Some(&e.filename) != cwd.as_ref() {
                if transport == "ssh" {
                    diag(request, "NONZERO_EXIT", "SSH executable is unavailable", None, None);
                    return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["NONZERO_EXIT"], res0());
                }
                let ev = dict(vec![
                    ("signal", s("missing_binary")),
                    ("rotate", PyValue::Bool(true)),
                    ("provenance", s("measured")),
                    ("binary", s(&argv0)),
                    ("detail", s(&format!("{} 를 실행할 수 없다(PATH 부재)", repr_str(&argv0)))),
                ]);
                return runner_unavailable(request, &ev, None, None);
            }
            diag(request, "NONZERO_EXIT",
                 &format!("provider could not be executed: {}: {} · argv[0]={}", e.class, e.text(), repr_str(&argv0)),
                 None, None);
            return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["NONZERO_EXIT"], res0());
        }
    };
    let (mut stdout, stderr) = match (text_mode(&completed.stdout), text_mode(&completed.stderr)) {
        (Some(o), Some(e)) => (o, e),
        _ => {
            diag(request, "MALFORMED_JSON", "provider stdout was not valid UTF-8", None, None);
            return result(request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["MALFORMED_JSON"], res0());
        }
    };
    let rc = completed.returncode;
    if let Some(obs) = observation {
        let (terminal, events, malformed) = native_stream(&stdout);
        *obs = events;
        if malformed {
            diag(request, "MALFORMED_JSON", "native event stream is incomplete or malformed", None, None);
            return result(request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["MALFORMED_JSON"], res0());
        }
        stdout = dumps_unicode(&terminal.unwrap_or(PyValue::None));
    }
    if rc != 0 {
        let payload = match pyjson::loads(&stdout) {
            Ok(c @ PyValue::Dict(_)) if is_type(&c, "result") => Some(c),
            _ => None,
        };
        if let Some(ev) = classify_runner_failure(payload.as_ref(), rc, &transport) {
            return runner_unavailable(request, &ev, payload.as_ref(), Some(&stderr));
        }
        if let Some(p) = &payload {
            let (sess, turns) = sess_turns(p);
            let (d, da) = durations(Some(p));
            if subtype_is(p, "error_during_execution") {
                diag(request, "NONZERO_EXIT",
                     &format!("provider 가 실행 도중 중단됐다(subtype=error_during_execution · rc={rc})."),
                     Some(&stderr), None);
                return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["NONZERO_EXIT"],
                              Res { session_id: sess, num_turns: turns, budget_outcome: Some("external_interruption"),
                                    duration_ms: d, duration_api_ms: da, ..res0() });
            }
            if subtype_is(p, "error_max_turns") {
                diag(request, "TURN_BUDGET_EXHAUSTED",
                     &format!("provider 가 max_turns={} 를 소진했다(num_turns={}). checkpoint와 남은 과업을 확인하고 session_id로 이어라; \
                               관측과 남은 작업에 맞게 다음 호출 예산을 선언한다(scope ⊥ budget).",
                              text_or_none(get(request, "max_turns")), str_of(&turns)),
                     None, None);
                return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["NONZERO_EXIT"],
                              Res { session_id: sess, num_turns: turns, budget_outcome: Some("exhausted"),
                                    duration_ms: d, duration_api_ms: da, ..res0() });
            }
            diag(request, "NONZERO_EXIT",
                 &format!("provider exited {rc} · subtype={} errors={}", opt_repr(get(p, "subtype")),
                          text_or_none(get(p, "errors"))),
                 Some(&stderr), None);
            return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["NONZERO_EXIT"],
                          Res { session_id: sess, num_turns: turns, budget_outcome: Some("aborted"),
                                duration_ms: d, duration_api_ms: da, ..res0() });
        }
        let note = if transport == "ssh" && rc == 255 {
            " (ssh transport: 255 = 전송 실패, host key/키인증/네트워크를 먼저 본다)"
        } else {
            ""
        };
        diag(request, "NONZERO_EXIT", &format!("provider exited {rc}{note}"), Some(&stderr), Some(&stdout));
        return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["NONZERO_EXIT"],
                      Res { budget_outcome: if rc == 124 || rc == 143 { Some("external_interruption") } else { None },
                            ..res0() });
    }
    let payload = match pyjson::loads(&stdout) {
        Ok(p) => p,
        Err(e) => {
            let class = match e {
                pyjson::LoadError::Decode { .. } => "JSONDecodeError",
                pyjson::LoadError::TooDeep { .. } => "RecursionError",
                _ => "ValueError",
            };
            let msg = match &e {
                pyjson::LoadError::TooDeep { .. } => e.to_string().trim_start_matches("RecursionError: ").to_string(),
                _ => e.to_string(),
            };
            diag(request, "MALFORMED_JSON", &format!("provider stdout 을 JSON 으로 읽지 못했다: {class}: {msg}"),
                 Some(&stderr), Some(&stdout));
            return result(request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["MALFORMED_JSON"], res0());
        }
    };
    if !matches!(payload.peel(), PyValue::Dict(_)) {
        diag(request, "MALFORMED_JSON", &format!("provider stdout 이 JSON 객체가 아니다(type={})", payload.type_name()),
             Some(&stderr), Some(&stdout));
        return result(request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["MALFORMED_JSON"], res0());
    }
    if !is_type(&payload, "result") {
        let mut keys: Vec<agent_runtime_support::pystr::PyStr> = match payload.peel() {
            PyValue::Dict(d) => d.iter().map(|(k, _)| k.clone()).collect(),
            _ => vec![],
        };
        keys.sort_by(|a, b| a.code_points().cmp(b.code_points()));
        let keys: Vec<String> = keys.iter().take(12).map(|k| k.repr()).collect();
        diag(request, "PROVIDER_RESULT_INVALID",
             &format!("봉투가 result 가 아니다 — type={} subtype={} keys=[{}]", opt_repr(get(&payload, "type")),
                      opt_repr(get(&payload, "subtype")), keys.join(", ")),
             Some(&stderr), Some(&stdout));
        return result(request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["PROVIDER_RESULT_INVALID"], res0());
    }
    let (sess, turns) = sess_turns(&payload);
    let (dur, dur_api) = durations(Some(&payload));
    if has(&payload, "permission_denials") {
        let denials = get(&payload, "permission_denials").cloned().unwrap_or(PyValue::None);
        let PyValue::List(list) = denials.peel() else {
            return result(request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["PROVIDER_RESULT_INVALID"], res0());
        };
        if !list.is_empty() {
            let mut det = vec![];
            for d in list.iter().take(10) {
                if matches!(d.peel(), PyValue::Dict(_)) {
                    let name = match get(d, "tool_name") {
                        Some(n) if n.is_truthy() => str_of(n),
                        _ => "?".into(),
                    };
                    let input = match get(d, "tool_input") {
                        Some(i) if i.is_truthy() => i.clone(),
                        _ => dict(vec![]),
                    };
                    det.push(format!("{name} {}", head_chars(&dumps_unicode(&input), 240)));
                } else {
                    det.push(head_chars(&str_of(d), 240));
                }
            }
            let mut note = format!("[permission_denials] {}건 — 거부된 도구 호출:\n  - {}", list.len(), det.join("\n  - "));
            if let Some(PyValue::Str(said)) = get(&payload, "result") {
                if !said.is_empty() {
                    note.push_str("\n\n[서브가 남긴 말]\n");
                    note.push_str(&crate::py::text(said));
                }
            }
            return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["PERMISSION_DENIED"],
                          Res { output: Some(note), session_id: sess, num_turns: turns, duration_ms: dur,
                                duration_api_ms: dur_api, ..res0() });
        }
    }
    if matches!(get(&payload, "is_error"), Some(PyValue::Bool(true))) {
        if let Some(ev) = classify_runner_failure(Some(&payload), rc, &transport) {
            return runner_unavailable(request, &ev, Some(&payload), Some(&stderr));
        }
        let res_text = head_chars(&text_or_none(get(&payload, "result")), 300);
        diag(request, "IS_ERROR",
             &format!("provider 가 is_error=true 로 끝냈다 — terminal_reason={} result={}",
                      opt_repr(get(&payload, "terminal_reason")), repr_str(&res_text)),
             Some(&stderr), None);
        return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["IS_ERROR"],
                      Res { session_id: sess, num_turns: turns, duration_ms: dur, duration_api_ms: dur_api, ..res0() });
    }
    if subtype_is(&payload, "error_max_turns") {
        diag(request, "TURN_BUDGET_EXHAUSTED",
             &format!("provider 가 max_turns={} 를 소진했다(num_turns={}). \
                       checkpoint와 남은 과업을 확인하고 관측에 맞게 다음 호출 예산을 선언해 이어라(scope ⊥ budget).",
                      text_or_none(get(request, "max_turns")), str_of(&turns)),
             None, None);
        return result(request, STATUS_EXECUTION_FAILED, EXIT_EXECUTION_FAILED, &["NONZERO_EXIT"],
                      Res { session_id: sess, num_turns: turns, budget_outcome: Some("exhausted"), duration_ms: dur,
                            duration_api_ms: dur_api, ..res0() });
    }
    let result_text = match get(&payload, "result") {
        Some(PyValue::Str(t)) if !t.is_empty() => Some(crate::py::text(t)),
        _ => None,
    };
    if !matches!(get(&payload, "is_error"), Some(PyValue::Bool(false))) || !subtype_is(&payload, "success")
        || result_text.is_none()
    {
        let rtype = get(&payload, "result").map(|r| r.type_name()).unwrap_or("NoneType");
        diag(request, "PROVIDER_RESULT_INVALID",
             &format!("result 봉투가 성공 형태가 아니다 — is_error={} subtype={} result_type={rtype} session={} turns={}",
                      opt_repr(get(&payload, "is_error")), opt_repr(get(&payload, "subtype")), str_of(&sess),
                      str_of(&turns)),
             Some(&stderr), Some(&stdout));
        return result(request, STATUS_MALFORMED_OUTPUT, EXIT_MALFORMED_OUTPUT, &["PROVIDER_RESULT_INVALID"],
                      Res { session_id: sess, num_turns: turns, budget_outcome: Some("unknown"), duration_ms: dur,
                            duration_api_ms: dur_api, ..res0() });
    }
    let mut model_ids = vec![];
    match get(&payload, "modelUsage") {
        Some(PyValue::Dict(mu)) if !mu.is_empty() => {
            // keys of a JSON object are always strings
            model_ids = mu.iter().map(|(k, _)| crate::py::text(k)).collect();
        }
        _ => diag(request, "MODEL_METADATA_ABSENT",
                  "provider 가 modelUsage 를 주지 않았다 — 어느 모델이 돌았는지 기록할 수 없다(차단하지 않는다 · model_used=[]).",
                  None, None),
    }
    let budget = if is_plain_int(get(&payload, "num_turns")) || matches!(get(&payload, "num_turns"), Some(PyValue::Bool(_))) {
        "within_budget"
    } else {
        "unknown"
    };
    result(request, STATUS_COMPLETED, EXIT_SUCCESS, &[],
           Res { model_used: model_ids, output: result_text, session_id: sess, num_turns: turns,
                 budget_outcome: Some(budget), duration_ms: dur, duration_api_ms: dur_api })
}

fn text_or_none(v: Option<&PyValue>) -> String {
    v.map(str_of).unwrap_or_else(|| "None".into())
}
