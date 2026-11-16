//! Port of `bootstrap_canary.py` — the executor of the §2.5 model-less canary: assembles the request JSON from the manifest
//! (`--emit`), optionally runs it through agent_control (`--invoke`, only after HITL approval), verifies a preserved
//! request/result offline (`--verify-request/--verify-result`) and judges the caller-plane installable (`--phase caller`).
//! Same command line, exit codes, text and JSON (key order) as the original. `build_request` / `backend_choices` are the
//! in-process API relay uses.
//!
//! Intentional differences (the Python → Rust move itself): the permission probe the request asks the native tool to run is
//! the launcher `bash .claude/skills/terraforming_node/scripts/harness_permission_probe.sh ...` (was `python3 -B ...py`), the
//! caller-plane rule names `relay.sh` (was `python3 ...relay.py`), and agent_control is started through its launcher.

use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::sysexit::{Exit, Res};
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pyyaml;
use role_contract::turn_budget::{self as budget, Declaration, Failure, PathValue};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TASK_COMMON: &str = "부트스트랩 카나리다. 모델을 로드하지 말고, 빌드하지 말고, 아무 파일도 고치지 마라. \
다음을 스스로 확인해 **schema-valid JSON 리포트 1개**만 반환하라(raw 로그 나열 금지): \
① CLAUDE.md 를 읽고 네 정체성(sub_mode)과 작업공간 경로를 보고 \
② output/<topology>/manifest.yaml 의 self_role·terraforming Flag·gpus_per_node 와 \
메인이 렌더 때 node_role_contract 로 해소해 CLAUDE.md·Agent_Card 에 실은 sub_mode·rank 를 그대로 보고\
(계약기 자체는 서브에 배달되지 않는다 — 없다고 보고하지 마라 · SSH는 endpoint 인증만 담당), \
Agent_Card.json 이 있으면 capability metadata로만 목록을 보고 \
③ .claude/rules/comms.md 의 리포트 계약을 읽었음을 보고 \
④ 로드된 런타임 스킬 목록(없으면 빈 배열)을 보고 \
⑤ 리포트를 .claude/schemas/task-report.schema.json 에 대조해 self_verification.schema_valid 에 담아라. \
phase=inspect · status=completed 로 끝내라. ";

const MAIN_TASK: &str = "메인 discovery 카나리다. serving 모델·GPU·빌드·파일변경을 하지 마라. \
CLAUDE.md와 AGENTS.md의 정본·메인 역할·output/<topology>/manifest.yaml을 확인하고 \
.claude/skills/terraforming_node/sub_node/task-report.schema.json 정본에 맞는 \
phase=inspect schema-valid JSON 리포트 1개를 반환하라. 서브 comms나 sub_mode를 요구하지 않는다.";

pub const DEFAULT_CANARY_MODEL: &str = "sonnet";
/// The permission probe, by its launcher (relative to the target's work_dir).
pub const PROBE_REL: &str = ".claude/skills/terraforming_node/scripts/harness_permission_probe.sh";
pub const PLAN_PREFIX: &str = "HARNESS_CANARY_PLAN=";
const KNOWN_PROVIDERS: [&str; 2] = ["claude_code", "codex_cli"];

/// Caller plane (C): the narrow delegation rules the main harness must carry (the string authority; harness_environment.md
/// describes this list). (harness, installable, rules).
pub const CALLER_PLANE_RULES: &[(&str, &str, &[&str])] = &[(
    "claude_code",
    ".claude/settings.json",
    &[
        "Bash(bash .claude/skills/terraforming_node/scripts/relay.sh *)",
        "Bash(bash .claude/skills/upstream-version-watch/scripts/fetch_sub_docs.sh *)",
    ],
)];

fn scripts_dir() -> PathBuf {
    crate::owner::owner_path("bootstrap_canary.py").parent().map(Path::to_path_buf).unwrap_or_default()
}

/// `os.path.abspath(p)`: absolute and lexically normalised (symlinks are not resolved).
pub fn abspath(p: &str) -> String {
    let joined = if p.starts_with('/') {
        p.to_string()
    } else {
        format!("{}/{p}", std::env::current_dir().unwrap_or_default().display())
    };
    let mut parts: Vec<&str> = vec![];
    for c in joined.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x),
        }
    }
    let lead = if joined.starts_with("//") && !joined.starts_with("///") { "//" } else { "/" };
    format!("{lead}{}", parts.join("/"))
}

/// `REPO` (= turn_budget's `REPO`): `scripts/../../../..`, lexically.
pub fn repo() -> String {
    abspath(&format!("{}/../../../..", scripts_dir().display()))
}

fn request_schema() -> String {
    format!("{}/.claude/schemas/agent-control-request.schema.json", repo())
}

/// A traceback of an exception the original lets escape: printed here, the status is 1.
fn traceback(line: &str) -> Exit {
    let _ = std::io::stdout().flush();
    eprintln!("Traceback (most recent call last):\n  (Python source frames are not reproduced by the Rust port)\n{line}");
    Exit::Code(1)
}

fn failure(f: Failure) -> Exit {
    match f {
        Failure::Raised(r) if r.class == "SystemExit" => Exit::Msg(crate::py::text(&r.message)),
        Failure::Raised(r) => traceback(&format!("{}: {}", r.class, crate::py::text(&r.message))),
        Failure::Unsupported(m) => traceback(&format!("RuntimeError: {m}")),
    }
}

fn get<'a>(v: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    crate::py::get(v, k)
}
fn s(t: &str) -> PyValue {
    PyValue::str(t)
}
fn dict(pairs: Vec<(&str, PyValue)>) -> PyValue {
    crate::py::dict(pairs)
}
fn set(v: &mut PyValue, k: &str, x: PyValue) {
    if let PyValue::Dict(d) = v {
        d.set(k, x);
    }
}

/// `TASK_BY_MODE[sub_mode]` (the a2a-agent skill list derived from node_role_contract's `TOOL_PLANE_BY_SUB_MODE`).
fn task_by_mode(sub_mode: &str) -> Option<String> {
    let globals = role_contract::globals();
    let plane = globals.iter().find(|(k, _)| *k == "TOOL_PLANE_BY_SUB_MODE").map(|(_, v)| v.clone())?;
    let skills = |m: &str| -> Vec<String> {
        match get(&plane, m) {
            Some(PyValue::Tuple(t)) | Some(PyValue::List(t)) => t.iter().map(crate::py::display).collect(),
            _ => vec![],
        }
    };
    match sub_mode {
        "a2a-agent" => {
            let k = skills("a2a-agent");
            Some(format!(
                "너는 A2A 원격 에이전트다(Ray 워커가 아니다). rank 는 없어야 하고, 런타임 스킬 {}종({})이 \
실제로 존재하는지 경로로 확인해 보고하라. 추가로 이 정도 과업에 필요하다고 보는 턴 수(budget_recommendation)와 needs_hitl 을 notes 에 담아라.",
                k.len(),
                k.join(" · ")
            ))
        }
        "ray-worker" => {
            let k = skills("ray-worker");
            Some(format!(
                "너는 멀티노드 Ray 워커다. 허용된 스킬 {}종({})의 실제 경로를 확인하고 그 목록을 skills_loaded 에 보고하라. rank 값을 함께 보고하라.",
                k.len(),
                k.join(" · ")
            ))
        }
        _ => None,
    }
}

/// `_load_yaml_min(path)`: `yaml.safe_load(f) or {}` (pyyaml is present on every node that runs the port).
fn load_yaml_min(path: &str) -> Res<PyValue> {
    let b = std::fs::read(path).map_err(|e| traceback(&crate::pathfmt::os_error(&e, path)))?;
    match pyyaml::safe_load(&b, &PyStr::new(path)) {
        Ok(v) if v.is_truthy() => Ok(v),
        Ok(_) => Ok(PyValue::Dict(PyDict::new())),
        Err(e) => Err(traceback(&crate::raise::Raised::yaml(&e).0)),
    }
}

/// `resolve_sub_mode(topology, manifest)`: node_role_contract (the authority) through its launcher.
fn resolve_sub_mode(topology: &str, manifest: &str) -> Res<(String, String)> {
    let launcher = scripts_dir().join("node_role_contract.sh");
    let argv: Vec<String> = vec![
        "bash".into(), launcher.display().to_string(), "evaluate".into(), "--topology".into(), topology.into(),
        "--manifest".into(), manifest.into(), "--field".into(), "sub_mode".into(), "--format".into(), "json".into(),
    ];
    let out = crate::proc::run(&argv, None, Some(30)).map_err(|r| traceback(&r.0))?;
    if out.returncode != 0 {
        let t = if out.stderr.is_empty() { &out.stdout } else { &out.stderr };
        let msg: String = crate::py::strip(t).chars().take(300).collect();
        return Err(Exit::Msg(format!("[canary] FAIL: sub_mode 판정 실패(rc={}) — {msg}", out.returncode)));
    }
    let doc = pyjson::loads(&out.stdout).map_err(|e| traceback(&format!("json.decoder.JSONDecodeError: {e}")))?;
    let inner = match get(&doc, "sub_mode") {
        Some(d @ PyValue::Dict(_)) => d.clone(),
        _ => doc,
    };
    let field = |k: &str| match get(&inner, k) {
        Some(v) if v.is_truthy() => crate::py::display(v),
        _ => String::new(),
    };
    Ok((field("value"), field("source")))
}

/// The provider adapter's `BACKEND_DEFAULT_MODEL` (first entry = default backend).
// NOTE: mirrors providers/claude_code.py and providers/codex_cli.py; to be read from the Rust providers' owner when
// agent_control exposes it (single authority).
fn backend_defaults(provider: &str) -> &'static [(&'static str, &'static str)] {
    match provider {
        "codex_cli" => &[("openai", "gpt-6.1-sol")],
        _ => &[("anthropic", "sonnet"), ("kimi", "k3[1m]"), ("minimax", "MiniMax-M3"), ("meta", "muse-spark-1.3"),
            ("openai", "gpt-5.6-sol"), ("claude-sub", "sonnet")],
    }
}

fn backend_default_model(backend: Option<&str>, provider: &str) -> Res<String> {
    if !KNOWN_PROVIDERS.contains(&provider) {
        return Err(Exit::Msg(format!("[canary] FAIL: unknown provider {}", PyStr::new(provider).repr())));
    }
    let d = backend_defaults(provider);
    let key = backend.filter(|b| !b.is_empty()).unwrap_or(d[0].0);
    d.iter().find(|(k, _)| *k == key).map(|(_, m)| m.to_string()).ok_or_else(|| {
        Exit::Msg(format!("[canary] FAIL: backend {} unsupported by {provider}", PyStr::new(key).repr()))
    })
}

/// `backend_choices()`: the request schema's `backend` enum.
pub fn backend_choices() -> Res<Vec<String>> {
    let path = request_schema();
    let fail = || Exit::Msg(format!("[canary] FAIL: 요청 스키마에서 backend enum 을 읽지 못했다 — {path}"));
    let b = std::fs::read(&path).map_err(|e| traceback(&crate::pathfmt::os_error(&e, &path)))?;
    let text = pyjson::decode_file_text(&b).map_err(|e| traceback(&format!("UnicodeDecodeError: {e}")))?;
    let doc = pyjson::loads_text(&text).map_err(|e| traceback(&format!("json.decoder.JSONDecodeError: {e}")))?;
    let props = get(&doc, "properties").filter(|v| v.is_truthy()).cloned().unwrap_or(PyValue::Dict(PyDict::new()));
    let backend = get(&props, "backend").filter(|v| v.is_truthy()).cloned().unwrap_or(PyValue::Dict(PyDict::new()));
    match get(&backend, "enum") {
        Some(PyValue::List(l)) if !l.is_empty() => Ok(l.iter().map(crate::py::display).collect()),
        _ => Err(fail()),
    }
}

// ── shlex (POSIX mode, as `shlex.split` / `shlex.join`) ──────────────────────────────────────

fn shlex_quote(a: &str) -> String {
    if a.is_empty() {
        return "''".into();
    }
    if a.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)) {
        return a.into();
    }
    format!("'{}'", a.replace('\'', "'\"'\"'"))
}

fn shlex_join(argv: &[String]) -> String {
    argv.iter().map(|a| shlex_quote(a)).collect::<Vec<_>>().join(" ")
}

/// `shlex.split(text)`; `None` = the ValueError of an unclosed quote or a trailing escape.
fn shlex_split(text: &str) -> Option<Vec<String>> {
    let ws = |c: char| c == ' ' || c == '\t' || c == '\r' || c == '\n';
    let mut out = vec![];
    let mut cur = String::new();
    let mut have = false;
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            c if ws(c) => {
                if have {
                    out.push(std::mem::take(&mut cur));
                    have = false;
                }
            }
            '\\' => {
                cur.push(it.next()?);
                have = true;
            }
            '\'' => {
                have = true;
                loop {
                    match it.next()? {
                        '\'' => break,
                        x => cur.push(x),
                    }
                }
            }
            '"' => {
                have = true;
                loop {
                    match it.next()? {
                        '"' => break,
                        '\\' => {
                            let n = it.next()?;
                            if !matches!(n, '\\' | '"' | '$' | '`' | '\n') {
                                cur.push('\\');
                            }
                            cur.push(n);
                        }
                        x => cur.push(x),
                    }
                }
            }
            x => {
                cur.push(x);
                have = true;
            }
        }
    }
    if have {
        out.push(cur);
    }
    Some(out)
}

fn probe_argv(work_dir: &str, topology: &str, role: &str, nonce: &str) -> Vec<String> {
    ["bash", PROBE_REL, "--workspace", work_dir, "--topology", topology, "--role", role, "--nonce", nonce]
        .iter().map(|x| x.to_string()).collect()
}

fn str_list(v: &[String]) -> PyValue {
    crate::py::list_str(v)
}

/// `permission_plan(request)`: the bounded plan carried by the request, or None.
pub fn permission_plan(request: &PyValue) -> Option<PyValue> {
    let task = get(request, "task").map(crate::py::display).unwrap_or_default();
    let lines: Vec<String> = crate::py::splitlines(&task).into_iter()
        .filter(|x| x.starts_with(PLAN_PREFIX)).map(|x| x[PLAN_PREFIX.len()..].to_string()).collect();
    if lines.len() != 1 {
        return None;
    }
    let p = pyjson::loads(&lines[0]).ok()?;
    if !matches!(p, PyValue::Dict(_)) || get(&p, "phase").map(crate::py::display).as_deref() != Some("permissions")
        || !matches!(get(&p, "phase"), Some(PyValue::Str(_)))
    {
        return None;
    }
    let nonce_ok = matches!(get(&p, "nonce"), Some(PyValue::Str(n)) if n.len() == 32
        && crate::py::text(n).chars().all(|c| "0123456789abcdef".contains(c)));
    let topo = match get(&p, "topology") {
        Some(PyValue::Str(t)) if t.equals("single") || t.equals("multi") => crate::py::text(t),
        _ => return None,
    };
    let sv_ok = matches!(get(&p, "schema_version"), Some(v) if role_contract::py_eq(v, &PyValue::int(1)));
    let tgt_eq = match (get(&p, "target"), get(request, "target")) {
        (Some(a), Some(b)) => role_contract::py_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !sv_ok || !nonce_ok || !tgt_eq {
        return None;
    }
    let target = get(request, "target")?;
    let expected = probe_argv(&crate::py::display_opt(get(target, "work_dir")), &topo,
        &crate::py::display_opt(get(target, "role")), &crate::py::display_opt(get(&p, "nonce")));
    match get(&p, "argv") {
        Some(a) if role_contract::py_eq(a, &str_list(&expected)) => Some(p),
        _ => None,
    }
}

fn permission_task(topology: &str, target: &PyValue, nonce: &str, provider: &str) -> String {
    let argv = probe_argv(&crate::py::display_opt(get(target, "work_dir")), topology,
        &crate::py::display_opt(get(target, "role")), nonce);
    let plan = dict(vec![("schema_version", PyValue::int(1)), ("phase", s("permissions")), ("nonce", s(nonce)),
        ("topology", s(topology)), ("target", target.clone()), ("argv", str_list(&argv))]);
    let enforcement = if provider == "codex_cli" {
        "Codex는 main/sub 공통 :danger-full-access를 사용한다. protected:*와 buildkit:*의 \
OS write-open은 allowed, preserved=true여야 한다. 이는 파일 변경 허가나 역할 소유의 증명이 아니다. \
정체성과 업무 소유는 manifest·SSH·노드 계약으로 별도 확인한다. "
    } else {
        "os_enforcement는 OS write-open 관측일 뿐 Claude 도구/managed 정책 집행의 증명이 아니다. \
이 표면만으로 업무 실패나 HITL을 만들지 말고 실제 관측과 미입증 한계를 notes에 남겨라. "
    };
    let role = crate::py::display_opt(get(target, "role"));
    format!(
        "업무 권한 카나리다. serving 모델·GPU·빌드·sudo·인증변경·원격 git·정책변경을 하지 마라. \
이 요청의 native 도구에서 아래 명령을 그대로 정확히 1회 실행하라. \
helper는 전용 scratch/독점 marker만 쓰고 회수하며 기존 canonical 파일에는 쓰지 않는다. \
도서관 query는 기존 파생 cache를 만들 수 있다. Docker는 version 조회만 한다. \
helper를 읽기만 하거나 결과를 만들어 쓰지 마라. 도구가 거부하면 우회·재실행하지 말고 보고하라.\n{}\n\
마지막 응답은 task-report.schema.json에 맞는 JSON 객체 1개다. \
task_id=context_id='harness-canary-{nonce}', turn=1, node_id='{role}', phase='inspect'. \
실제 수행만 self_verification.checks_run에 적어라. \
functional 검사는 모두 allowed여야 하며, model_source=ephemeral인 model_asset_read만 \
not_applicable이 정상이다. {enforcement}\
필요한 기능의 거부·미관측 또는 기대 결과 불일치는 input-required와 hitl로 보고한다. \
모든 해당 조건을 만족하면 status=completed, errors=[], hitl.needed=false, \
hitl.blocking=false로 보고하라. 이 제한된 카나리 밖의 build/run·원격 hint 발행·재개가 \
미검증인 것은 notes의 한계이며 그 자체로 추가 승인 요청을 만들지 않는다. \
helper stdout은 native 도구 결과에 남긴다. notes의 자기주장만으로 검증을 대신하지 않는다.\n{PLAN_PREFIX}{}",
        shlex_join(&argv),
        crate::harness_permission_probe::dumps_tight_unicode(&plan)
    )
}

fn command_matches(command: Option<&PyValue>, argv: &PyValue) -> bool {
    let Some(PyValue::Str(c)) = command else { return false };
    let Some(mut tokens) = shlex_split(&crate::py::text(c)) else { return false };
    if !tokens.is_empty() && ["/bin/bash", "bash", "/bin/sh", "sh"].contains(&tokens[0].as_str())
        && tokens.len() == 3 && (tokens[1] == "-lc" || tokens[1] == "-c")
    {
        match shlex_split(&tokens[2]) {
            Some(t) => tokens = t,
            None => return false,
        }
    }
    role_contract::py_eq(&str_list(&tokens), argv)
}

fn is_str(v: Option<&PyValue>, w: &str) -> bool {
    matches!(v, Some(PyValue::Str(x)) if x.equals(w))
}

/// `_tool_receipts(result, argv)`: only native completed tool results.
fn tool_receipts(result: &PyValue, argv: &PyValue) -> Vec<String> {
    let mut receipts = vec![];
    let mut uses: Vec<PyValue> = vec![];
    let mut seen: Vec<String> = vec![];
    let provider = get(result, "provider");
    let events = match get(result, "raw_events") {
        Some(PyValue::List(l)) => l.clone(),
        _ => vec![],
    };
    for e in &events {
        if !matches!(e.peel(), PyValue::Dict(_)) {
            continue;
        }
        let item = get(e, "item");
        if is_str(provider, "codex_cli") && is_str(get(e, "type"), "item.completed") {
            if let Some(it @ PyValue::Dict(_)) = item {
                if is_str(get(it, "type"), "command_execution") && is_str(get(it, "status"), "completed")
                    && get(it, "exit_code").is_some_and(|x| role_contract::py_eq(x, &PyValue::int(0)))
                    && command_matches(get(it, "command"), argv)
                {
                    if let Some(PyValue::Str(o)) = get(it, "aggregated_output") {
                        receipts.push(crate::py::text(o));
                    }
                }
            }
        }
        if !is_str(provider, "claude_code") {
            continue;
        }
        let Some(message @ PyValue::Dict(_)) = get(e, "message") else { continue };
        let content = match get(message, "content") {
            Some(PyValue::List(l)) => l.clone(),
            _ => vec![],
        };
        let etype_a = is_str(get(e, "type"), "assistant");
        let etype_u = is_str(get(e, "type"), "user");
        for block in &content {
            if !matches!(block.peel(), PyValue::Dict(_)) {
                continue;
            }
            if etype_a && is_str(get(block, "type"), "tool_use") {
                match get(block, "id") {
                    Some(PyValue::Str(id)) if !id.is_empty() && !seen.contains(&crate::py::text(id)) => {
                        seen.push(crate::py::text(id))
                    }
                    _ => return vec![],
                }
            }
            if etype_a && is_str(get(block, "type"), "tool_use") && is_str(get(block, "name"), "Bash") {
                if let Some(inp @ PyValue::Dict(_)) = get(block, "input") {
                    if command_matches(get(inp, "command"), argv) {
                        uses.push(get(block, "id").cloned().unwrap_or(PyValue::None));
                    }
                }
            }
            if etype_u && is_str(get(block, "type"), "tool_result")
                && get(block, "tool_use_id").is_some_and(|t| uses.iter().any(|u| role_contract::py_eq(u, t)))
                && !matches!(get(block, "is_error"), Some(PyValue::Bool(true)))
            {
                let body = match get(block, "content") {
                    Some(PyValue::List(l)) => Some(l.iter()
                        .filter(|b| matches!(b.peel(), PyValue::Dict(_)) && is_str(get(b, "type"), "text"))
                        .map(|b| get(b, "text").map(crate::py::display).unwrap_or_default())
                        .collect::<Vec<_>>().join("\n")),
                    Some(PyValue::Str(t)) => Some(crate::py::text(t)),
                    _ => None,
                };
                if let Some(b) = body {
                    receipts.push(b);
                }
            }
        }
    }
    receipts
}

/// `verify_result(request, result)`: native CLI / task report / domain evidence, no rescan, no fallback executor.
pub fn verify_result(request: &PyValue, result: &PyValue) -> Res<PyValue> {
    use crate::agent_control as ac;
    if !matches!(request, PyValue::Dict(_)) || !matches!(result, PyValue::Dict(_)) {
        return Ok(dict(vec![("status", s("input-required")), ("phase", s("inspect")), ("reason", s("REQUEST_OR_RESULT_NOT_OBJECT"))]));
    }
    let load = |p: &Path| ac::load_schema(p).map_err(|m| traceback(&m));
    let mut problems = ac::request_schema_violations(request);
    problems.extend(ac::schema_violations(result, &load(&ac::result_schema_path())?, "$"));
    if ac::request_schema_violations(request).is_empty() {
        problems.extend(ac::result_contract_violations(result, request));
    }
    let eq = |a: Option<&PyValue>, b: Option<&PyValue>| match (a, b) {
        (Some(x), Some(y)) => role_contract::py_eq(x, y),
        (None, None) => true,
        (Some(x), None) | (None, Some(x)) => matches!(x, PyValue::None),
    };
    let mut native_ok = problems.is_empty() && is_str(get(result, "status"), "completed")
        && get(result, "exit_code").is_some_and(|x| role_contract::py_eq(x, &PyValue::int(0)));
    native_ok = native_ok && eq(get(result, "provider"), get(request, "provider"));
    native_ok = native_ok && eq(get(result, "model_requested"), get(request, "model"));
    let output = match get(result, "output") {
        Some(v) if v.is_truthy() => Some(v.clone()),
        _ => None,
    };
    let report = match output {
        Some(PyValue::Str(t)) => pyjson::loads_text(&t).unwrap_or(PyValue::None),
        None => PyValue::None,
        Some(_) => PyValue::None,
    };
    let schema = load(&scripts_dir().parent().unwrap().join("sub_node/task-report.schema.json"))?;
    let schema_errors = ac::schema_violations(&report, &schema, "$");
    let mut report_ok = schema_errors.is_empty() && is_str(get(&report, "phase"), "inspect")
        && is_str(get(&report, "status"), "completed");
    let plan = permission_plan(request);
    let mut domain_status = "unproven";
    let mut reasons: Vec<String> = vec![];
    let mut observations: Option<PyValue> = None;
    let mut enf_status = "unproven";
    let mut limitations: Vec<String> = vec![];
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let role = get(&target, "role").cloned().unwrap_or(PyValue::None);
    let is_codex = is_str(get(request, "provider"), "codex_cli");
    match &plan {
        None => reasons.push("DISCOVERY_ONLY_NO_WORK_PERMISSION_OBSERVATIONS".into()),
        Some(plan) => {
            let expected_context = format!("harness-canary-{}", crate::py::display_opt(get(plan, "nonce")));
            report_ok = report_ok && ["task_id", "context_id"].iter().all(|k| is_str(get(&report, k), &expected_context));
            report_ok = report_ok && eq(get(&report, "node_id"), Some(&role));
            let mut packets: Vec<Option<PyValue>> = vec![];
            let argv = get(plan, "argv").cloned().unwrap_or(PyValue::None);
            for receipt in tool_receipts(result, &argv) {
                for line in crate::py::splitlines(&receipt) {
                    if let Some(rest) = line.strip_prefix(crate::harness_permission_probe::MARKER) {
                        packets.push(pyjson::loads(rest).ok());
                    }
                }
            }
            match packets.as_slice() {
                [Some(packet @ PyValue::Dict(_))] => {
                    let bound = get(packet, "schema_version").is_some_and(|v| role_contract::py_eq(v, &PyValue::int(crate::harness_permission_probe::SCHEMA_VERSION)))
                        && eq(get(packet, "nonce"), get(plan, "nonce"))
                        && eq(get(packet, "topology"), get(plan, "topology"))
                        && eq(get(packet, "role"), Some(&role))
                        && eq(get(packet, "workspace"), get(&target, "work_dir"))
                        && is_str(get(packet, "provenance"), "observed_local_process");
                    let checks = get(packet, "checks").cloned();
                    let list = match &checks {
                        Some(PyValue::List(l)) if l.iter().all(|c| matches!(c.peel(), PyValue::Dict(_)) && matches!(get(c, "name"), Some(PyValue::Str(_)))) => Some(l.clone()),
                        _ => None,
                    };
                    let mut by_name: Vec<(String, PyValue)> = vec![];
                    if let Some(l) = &list {
                        for c in l {
                            let n = crate::py::display_opt(get(c, "name"));
                            if let Some(slot) = by_name.iter_mut().find(|(k, _)| *k == n) {
                                slot.1 = c.clone();
                            } else {
                                by_name.push((n, c.clone()));
                            }
                        }
                    }
                    let valid = list.as_ref().is_some_and(|l| by_name.len() == l.len());
                    let find = |n: &str| by_name.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
                    let mut required: Vec<&str> = vec!["scratch_write_cleanup", "scratch_local_git", "scratch_removed",
                        "workspace_git_metadata", "docker_daemon_read", "docker_compose_read", "library_query",
                        "model_asset_read", "buildkit_declaration"];
                    let single_sub = is_str(get(plan, "topology"), "single") && is_str(Some(&role), "sub");
                    if single_sub {
                        required.extend(["hint_clone_write", "hint_clone_git_discovery", "hint_clone_write_git_1"]);
                    }
                    let mut functional = bound && valid && required.iter().all(|n| find(n).is_some());
                    functional = functional && list.clone().unwrap_or_default().iter().filter(|c| is_str(get(c, "axis"), "functional")).all(|c| {
                        is_str(get(c, "outcome"), "allowed") || (is_str(get(c, "name"), "model_asset_read")
                            && is_str(get(c, "outcome"), "not_applicable") && is_str(get(c, "model_source"), "ephemeral"))
                    });
                    functional = functional && required.iter().all(|n| find(n).is_some_and(|c| is_str(get(&c, "axis"), "functional")));
                    domain_status = if functional { "pass" } else { "unproven" };
                    observations = Some(packet.clone());
                    if !functional {
                        reasons.push("REQUIRED_ROLE_WORK_UNOBSERVED_OR_FAILED".into());
                    }
                    let bf: Option<Vec<String>> = match get(packet, "build_files") {
                        Some(PyValue::List(l)) => l.iter().map(|x| match x { PyValue::Str(t) => Some(crate::py::text(t)), _ => None }).collect(),
                        _ => None,
                    };
                    let probe = crate::harness_permission_probe::BUILD_FILES;
                    let dfs = crate::harness_permission_probe::DOCKERFILES;
                    let build_valid = bf.as_ref().is_some_and(|b| {
                        let mut u = b.clone();
                        u.sort();
                        u.dedup();
                        u.len() == b.len() && probe.iter().all(|p| b.iter().any(|x| x == p))
                            && dfs.iter().any(|d| b.iter().any(|x| x == d))
                            && b.iter().all(|x| probe.contains(&x.as_str()) || dfs.contains(&x.as_str()))
                    });
                    let multi_sub = is_str(Some(&role), "sub") && is_str(get(plan, "topology"), "multi");
                    let mut os_required: Vec<(String, &str)> = vec![];
                    let mut put = |k: String, v: &'static str| {
                        if let Some(slot) = os_required.iter_mut().find(|(n, _)| *n == k) {
                            slot.1 = v;
                        } else {
                            os_required.push((k, v));
                        }
                    };
                    if build_valid {
                        for n in bf.clone().unwrap() {
                            put(format!("buildkit:{n}"), if multi_sub { "denied" } else { "allowed" });
                        }
                    }
                    for (n, o) in crate::harness_permission_probe::protected_file_expectations(&crate::py::display(&role)) {
                        put(format!("protected:{n}"), o);
                    }
                    if is_codex {
                        for x in os_required.iter_mut() {
                            x.1 = "allowed";
                        }
                    }
                    let os_ok = bound && valid && build_valid && os_required.iter().all(|(n, o)| {
                        find(n).is_some_and(|c| is_str(get(&c, "outcome"), o)
                            && matches!(get(&c, "preserved"), Some(PyValue::Bool(true)))
                            && is_str(get(&c, "axis"), "os_enforcement"))
                    });
                    if is_codex {
                        enf_status = if os_ok { "pass" } else { "unproven" };
                        limitations.push("Full-access write-open observations do not prove role ownership, sandbox isolation or effective config selection".into());
                    } else {
                        limitations.push("Claude tool/managed deny must be observed at its native tool surface; Python write-open does not establish that boundary".into());
                    }
                }
                _ => reasons.push("EXACT_NATIVE_PROBE_RECEIPT_ABSENT_OR_AMBIGUOUS".into()),
            }
        }
    }
    let mut complete = native_ok && report_ok && domain_status == "pass";
    if is_codex {
        complete = complete && enf_status == "pass";
    }
    let mut domain = dict(vec![("status", s(domain_status)), ("reasons", str_list(&reasons))]);
    if let Some(o) = observations {
        set(&mut domain, "observations", o);
    }
    let model_observed = get(result, "model_used").cloned().unwrap_or(PyValue::List(vec![]));
    Ok(dict(vec![
        ("status", s(if complete { "completed" } else { "input-required" })),
        ("phase", s("inspect")),
        ("native_cli", dict(vec![("status", s(if native_ok { "pass" } else { "unproven" })), ("schema_errors", str_list(&problems)),
            ("model_requested", get(request, "model").cloned().unwrap_or(PyValue::None)), ("model_observed", model_observed)])),
        ("task_report", dict(vec![("status", s(if report_ok { "pass" } else { "unproven" })), ("schema_errors", str_list(&schema_errors))])),
        ("domain_evidence", domain),
        ("enforcement_evidence", dict(vec![("status", s(enf_status)), ("surface", s("os_write_open")), ("limitations", str_list(&limitations))])),
        ("limitations", str_list(&["No model serving, Docker build/run, remote push, or native resume proof".into(),
            "completed is this canary's bounded evidence, not performance/promotion completion".into()])),
    ]))
}

/// The keyword arguments of `build_request(topology, manifest_path, *, ...)` with the original defaults
/// (`provider="claude_code"`, `phase="discovery"`, `target_role="sub"`, everything else `None`).
/// `max_turns` / `timeout_seconds` are Python values (they go to `turn_budget.declare` unchanged).
#[derive(Debug, Clone)]
pub struct RequestOpts {
    pub max_turns: PyValue,
    pub timeout_seconds: PyValue,
    pub budget_source: Option<String>,
    pub model: Option<String>,
    pub backend: Option<String>,
    pub provider: String,
    pub max_dispatches: Option<i64>,
    pub sandbox_mode: Option<String>,
    pub permission_profile: Option<String>,
    pub phase: String,
    pub target_role: String,
    pub main_work_dir: Option<String>,
    pub reasoning_effort: Option<String>,
}

impl Default for RequestOpts {
    fn default() -> Self {
        RequestOpts {
            max_turns: PyValue::None,
            timeout_seconds: PyValue::None,
            budget_source: None,
            model: None,
            backend: None,
            provider: "claude_code".into(),
            max_dispatches: None,
            sandbox_mode: None,
            permission_profile: None,
            phase: "discovery".into(),
            target_role: "sub".into(),
            main_work_dir: None,
            reasoning_effort: None,
        }
    }
}

fn truthy(o: &Option<String>) -> bool {
    o.as_deref().is_some_and(|x| !x.is_empty())
}

fn token_hex16() -> String {
    let mut b = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut b);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `build_request(topology, manifest_path, **opts)` → the request dict (key order as the original builds it).
pub fn build_request(topology: &str, manifest_path: &str, o: &RequestOpts) -> Res<PyValue> {
    if !Path::new(manifest_path).exists() {
        return Err(Exit::Msg(format!("[canary] FAIL: manifest 부재 — {manifest_path}")));
    }
    let man = load_yaml_min(manifest_path)?;
    if !["discovery", "permissions"].contains(&o.phase.as_str()) || !["main", "sub"].contains(&o.target_role.as_str()) {
        return Err(Exit::msg("[canary] FAIL: unknown phase/target"));
    }
    let nodes = match get(&man, "nodes") {
        Some(PyValue::List(l)) => l.clone(),
        _ => vec![],
    };
    let sub = nodes.iter().find(|n| crate::py::display_opt(get(n, "role")) == "sub").cloned();
    let want_sub = o.target_role == "sub";
    if want_sub && sub.is_none() {
        return Err(Exit::msg("[canary] FAIL: manifest nodes[] 에 role: sub 가 없다 — 서브 미등록 상태에서는 \
카나리를 조립할 수 없다(§2.5 는 전달 후 게이트다)."));
    }
    if let (true, Some(sub)) = (want_sub, &sub) {
        let missing: Vec<String> = ["host", "ssh_user", "work_dir"].iter()
            .filter(|k| match get(sub, k) {
                Some(v) if v.is_truthy() => crate::py::display(v).starts_with("__REQUIRED__"),
                _ => true,
            })
            .map(|k| PyStr::new(k).repr()).collect();
        if !missing.is_empty() {
            return Err(Exit::Msg(format!("[canary] FAIL: nodes[sub] 필수 필드 미해소 [{}] — \
추측해서 접속하지 않는다(틀린 계정/경로는 권한 평면을 바꾼다).", missing.join(", "))));
        }
    }
    let codex = o.provider == "codex_cli";
    let unit = if codex { "wall_time" } else { "native_turns" };
    let mut decl = Declaration::new(o.max_turns.clone(), o.timeout_seconds.clone(), s(o.budget_source.as_deref().unwrap_or("")));
    decl.unit = s(unit);
    let bud = budget::declare(&decl, &PathValue::text_of(&PyStr::new(&request_schema()))).map_err(failure)?;
    let (sub_mode, mode_source) = if want_sub {
        resolve_sub_mode(topology, manifest_path)?
    } else {
        ("main".into(), "explicit main target".into())
    };
    let mode_task = if want_sub {
        match task_by_mode(&sub_mode) {
            Some(t) => Some(t),
            None => {
                return Err(Exit::Msg(format!("[canary] FAIL: 알 수 없는 sub_mode={}(출처 {mode_source}) — \
정체성이 정해지지 않으면 물어볼 내용도 정해지지 않는다.", PyStr::new(&sub_mode).repr())));
            }
        }
    } else {
        None
    };
    let model = if truthy(&o.model) { o.model.clone().unwrap() } else { backend_default_model(o.backend.as_deref(), &o.provider)? };
    let task = match &mode_task {
        Some(t) => format!("{TASK_COMMON}{t}"),
        None => MAIN_TASK.to_string(),
    };
    let target = match (&sub, want_sub) {
        (Some(sub), true) => dict(vec![("role", s("sub")), ("transport", s("ssh")),
            ("host", s(&crate::py::display_opt(get(sub, "host")))),
            ("ssh_user", s(&crate::py::display_opt(get(sub, "ssh_user")))),
            ("work_dir", s(&crate::py::display_opt(get(sub, "work_dir"))))]),
        _ => {
            let wd = if truthy(&o.main_work_dir) { o.main_work_dir.clone().unwrap() } else { repo() };
            dict(vec![("role", s("main")), ("transport", s("local")), ("work_dir", s(&abspath(&wd)))])
        }
    };
    let mut req = dict(vec![
        ("schema_version", PyValue::int(if codex { 2 } else { 1 })),
        ("provider", s(&o.provider)),
        ("intent", s("bootstrap_canary")),
        ("model", s(&model)),
        ("task", s(&task)),
        ("target", target.clone()),
        ("capabilities", str_list(&["read".into(), "execute".into()])),
    ]);
    if truthy(&o.backend) {
        set(&mut req, "backend", s(o.backend.as_deref().unwrap()));
    }
    set(&mut req, "timeout_seconds", get(&bud, "timeout_seconds").cloned().unwrap_or(PyValue::None));
    set(&mut req, "max_turns", get(&bud, "max_turns").cloned().unwrap_or(PyValue::None));
    if let Some(r) = &o.reasoning_effort {
        set(&mut req, "reasoning_effort", s(r));
    }
    if codex {
        set(&mut req, "budget_unit", s(unit));
        if truthy(&o.sandbox_mode) && truthy(&o.permission_profile) {
            return Err(Exit::msg("[canary] FAIL: choose sandbox_mode OR permission_profile"));
        }
        if truthy(&o.permission_profile) {
            set(&mut req, "permission_profile", s(o.permission_profile.as_deref().unwrap()));
        } else {
            set(&mut req, "sandbox_mode", s(if truthy(&o.sandbox_mode) { o.sandbox_mode.as_deref().unwrap() } else { "read-only" }));
        }
        let t = format!("AGENTS.md 를 먼저 읽고 그 문서가 지정한 정본 지침을 읽어라. {task}");
        set(&mut req, "task", s(&t));
    }
    if o.phase == "permissions" {
        if !truthy(&o.model) {
            return Err(Exit::msg("[canary] FAIL: permissions phase requires explicit --model"));
        }
        if codex && (truthy(&o.sandbox_mode) || o.permission_profile.as_deref() != Some(":danger-full-access")) {
            return Err(Exit::msg("[canary] FAIL: permissions phase requires :danger-full-access, no legacy sandbox"));
        }
        let t = permission_task(topology, &target, &token_hex16(), &o.provider);
        set(&mut req, "task", s(&t));
        set(&mut req, "capabilities", str_list(&["read".into(), "execute".into(), "edit".into(), "write".into()]));
        if o.provider == "claude_code" {
            set(&mut req, "capture_native_events", PyValue::Bool(true));
        }
    }
    Ok(req)
}

/// `caller_plane_check(repo, harness)`: static precondition only (the live proof is one delegation).
pub fn caller_plane_check(repo: &str, harness: &str) -> PyValue {
    let Some((_, rel, rules)) = CALLER_PLANE_RULES.iter().find(|(h, _, _)| *h == harness) else {
        return dict(vec![("phase", s("caller")), ("harness", s(harness)), ("status", s("completed")),
            ("reason", s("no caller-plane installable for this harness (worker config suffices)"))]);
    };
    let rules: Vec<String> = rules.iter().map(|r| r.to_string()).collect();
    let path = format!("{}/{rel}", repo.trim_end_matches('/'));
    let installer = "human (agent self-modification is refused by design)";
    let loaded: Result<PyValue, &str> = match std::fs::read(&path) {
        Err(e) => Err(match e.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            _ if e.raw_os_error() == Some(21) => "IsADirectoryError",
            _ => "OSError",
        }),
        Ok(b) => match pyjson::decode_file_text(&b) {
            Err(_) => Err("UnicodeDecodeError"),
            Ok(t) => pyjson::loads_text(&t).map_err(|_| "JSONDecodeError"),
        },
    };
    let doc = match loaded {
        Ok(d) => d,
        Err(cls) => {
            return dict(vec![("phase", s("caller")), ("harness", s(harness)), ("status", s("input-required")),
                ("installable", s(rel)), ("missing", str_list(&rules)), ("reason", s(&format!("unreadable: {cls}"))),
                ("installer", s(installer))]);
        }
    };
    let perms = get(&doc, "permissions").filter(|v| v.is_truthy()).cloned().unwrap_or(PyValue::Dict(PyDict::new()));
    let allow = match get(&perms, "allow") {
        Some(PyValue::List(l)) => l.clone(),
        _ => vec![],
    };
    let missing: Vec<String> = rules.iter().filter(|r| !allow.iter().any(|a| is_str(Some(a), r))).cloned().collect();
    let tracked = Command::new("git").args(["-C", repo, "ls-files", "--error-unmatch", rel])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).output().is_ok_and(|o| o.status.success());
    let ok = missing.is_empty() && tracked;
    dict(vec![("phase", s("caller")), ("harness", s(harness)), ("status", s(if ok { "completed" } else { "input-required" })),
        ("installable", s(rel)), ("tracked", PyValue::Bool(tracked)), ("missing", str_list(&missing)),
        ("installer", s(installer)),
        ("proof_limit", s("static precondition only; live proof = one agent-run relay delegation with zero denials"))])
}

// ── self-test ────────────────────────────────────────────────────────────────────────────────

fn self_test() -> i32 {
    let mut ok = true;
    let mut chk = |c: bool, label: &str| {
        println!("  [{}] {label}", if c { "PASS" } else { "FAIL" });
        ok = ok && c;
    };
    // Plain concatenation: a `\` line continuation would strip the list indentation of the YAML fixture.
    let fixture = concat!(
        "topology: multi\ncpu_arch: \"aarch64\"\nnodes:\n",
        "  - role: main\n    host: \"203.0.113.10\"\n    ssh_user: tester\n    work_dir: /srv/ws\n",
        "  - role: sub\n    host: \"203.0.113.11\"\n    ssh_user: tester\n    work_dir: /srv/ws\n"
    );
    let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let d = base.join(format!("tmp{}{n:x}", std::process::id()));
    std::fs::create_dir_all(&d).expect("tempdir");
    let mp = d.join("manifest.yaml").display().to_string();
    std::fs::write(&mp, fixture).unwrap();
    let man = load_yaml_min(&mp).unwrap_or(PyValue::None);
    let sub = match get(&man, "nodes") {
        Some(PyValue::List(l)) => l.iter().find(|n| is_str(get(n, "role"), "sub")).cloned(),
        _ => None,
    };
    chk(sub.as_ref().is_some_and(|n| is_str(get(n, "host"), "203.0.113.11") && is_str(get(n, "ssh_user"), "tester")
        && is_str(get(n, "work_dir"), "/srv/ws")), "manifest nodes[sub] 5필드 해소");
    let opts = RequestOpts { max_turns: PyValue::int(10), timeout_seconds: PyValue::int(600),
        budget_source: Some("선언: 카나리 1왕복(인스펙트 전용 · 파일 수정 없음)".into()), ..Default::default() };
    let req = match build_request("multi", &mp, &opts) {
        Ok(r) => r,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&d);
            return e.finish();
        }
    };
    let viol = crate::agent_control::request_schema_violations(&req);
    let shown = format!("[{}]", viol.iter().map(|v| PyStr::new(v).repr()).collect::<Vec<_>>().join(", "));
    chk(viol.is_empty(), &format!("request 가 전송 스키마를 그대로 통과한다(위반 {shown})"));
    let target = get(&req, "target").cloned().unwrap_or(PyValue::None);
    chk(is_str(get(&req, "intent"), "bootstrap_canary") && is_str(get(&target, "role"), "sub")
        && is_str(get(&target, "transport"), "ssh"), "intent/target 계약");
    let task = get(&req, "task").map(crate::py::display).unwrap_or_default();
    chk(!task.to_lowercase().contains("ray") || task.contains("Ray 워커"), "multi 카나리 문구가 ray-worker 정체성을 묻는다");
    chk(get(&req, "max_turns").is_some_and(|v| role_contract::py_eq(v, &PyValue::int(10)))
        && get(&req, "timeout_seconds").is_some_and(|v| role_contract::py_eq(v, &PyValue::int(600)))
        && get(&req, "source").is_none(), "turn 예산이 **선언**으로 들어오고 출처는 요청 밖에 남는다(등급표 ✗)");
    for legacy in [None, Some(3)] {
        let co = RequestOpts { timeout_seconds: PyValue::int(90), budget_source: Some("per invocation".into()),
            provider: "codex_cli".into(), backend: Some("openai".into()), max_dispatches: legacy, ..Default::default() };
        let good = match build_request("multi", &mp, &co) {
            Ok(c) => get(&c, "max_dispatches").is_none()
                && get(&c, "timeout_seconds").is_some_and(|v| role_contract::py_eq(v, &PyValue::int(90)))
                && crate::agent_control::request_schema_violations(&c).is_empty(),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&d);
                return e.finish();
            }
        };
        chk(good, "Codex request omits historical cumulative count and preserves wall timeout");
    }
    match build_request("multi", &mp, &RequestOpts::default()) {
        Ok(_) => chk(false, "예산 미선언 → fail-loud"),
        Err(e) => {
            let m = match &e { Exit::Msg(m) => m.clone(), Exit::Code(c) => c.to_string() };
            chk(m.contains("선언되지 않았다"), "예산 미선언 → fail-loud(기본값을 만들지 않는다)");
        }
    }
    let mp2 = d.join("single.yaml").display().to_string();
    std::fs::write(&mp2, "topology: single\nnodes:\n  - role: main\n    host: \"h\"\n").unwrap();
    match build_request("single", &mp2, &RequestOpts::default()) {
        Ok(_) => chk(false, "서브 미등록 → fail-loud"),
        Err(_) => chk(true, "서브 미등록 → fail-loud"),
    }
    let mp3 = d.join("sentinel.yaml").display().to_string();
    std::fs::write(&mp3, fixture.replacen("  - role: sub\n    host: \"203.0.113.11\"\n    ssh_user: tester",
        "  - role: sub\n    host: \"203.0.113.11\"\n    ssh_user: __REQUIRED__", 1)).unwrap();
    match build_request("multi", &mp3, &RequestOpts::default()) {
        Ok(_) => chk(false, "__REQUIRED__ 센티넬 → fail-loud"),
        Err(_) => chk(true, "__REQUIRED__ 센티넬 → fail-loud"),
    }
    let _ = std::fs::remove_dir_all(&d);
    println!("self-test: {}", if ok { "PASS" } else { "FAIL" });
    if ok { 0 } else { 2 }
}

// ── CLI ──────────────────────────────────────────────────────────────────────────────────────

const PROVIDERS: &[&str] = &["claude_code", "codex_cli"];

fn spec(backends: Vec<String>) -> &'static Spec {
    let choices: &'static [&'static str] =
        Box::leak(backends.into_iter().map(|b| &*Box::leak(b.into_boxed_str())).collect::<Vec<&'static str>>().into_boxed_slice());
    let args: Vec<Arg> = vec![
        HELP,
        store(&["--topology"]).choices(&["single", "multi"]),
        store(&["--manifest"]),
        store(&["--phase"]).choices(&["discovery", "permissions", "caller"]),
        store(&["--caller-harness"]).choices(PROVIDERS),
        store(&["--target"]).choices(&["main", "sub"]),
        store(&["--main-work-dir"]),
        store(&["--verify-request"]),
        store(&["--verify-result"]),
        store(&["--emit"]),
        store(&["--max-turns"]).int(),
        store(&["--provider"]).choices(PROVIDERS),
        store(&["--max-dispatches"]).int(),
        store(&["--sandbox-mode"]).choices(&["read-only", "workspace-write"]).group(1),
        store(&["--permission-profile"]).group(1),
        store(&["--timeout-seconds"]).int(),
        store(&["--budget-source"]),
        store(&["--reasoning-effort"]).choices(&["default", "none", "minimal", "low", "medium", "high", "xhigh", "max"]),
        store(&["--model"]),
        store(&["--backend"]).choices(choices),
        flag(&["--invoke"]),
        flag(&["--self-test"]),
    ];
    Box::leak(Box::new(Spec {
        prog: "bootstrap_canary.py",
        usage: include_str!("../help/bootstrap_canary.usage"),
        help: include_str!("../help/bootstrap_canary.help"),
        args: Box::leak(args.into_boxed_slice()),
        subs: None,
    }))
}

fn load_json_file(path: &str) -> Res<PyValue> {
    let b = std::fs::read(path).map_err(|e| traceback(&crate::pathfmt::os_error(&e, path)))?;
    let t = pyjson::decode_file_text(&b).map_err(|e| traceback(&format!("UnicodeDecodeError: {e}")))?;
    pyjson::loads_text(&t).map_err(|e| traceback(&format!("json.decoder.JSONDecodeError: {e}")))
}

fn opt(t: Option<&str>) -> Option<String> {
    t.map(String::from)
}

fn run(args: &[String]) -> Res<i32> {
    let spec = spec(backend_choices()?);
    let a = match argp::parse(spec, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return Ok(c),
    };
    if a.flag(21) {
        return Ok(self_test());
    }
    let vreq = a.text(7).filter(|x| !x.is_empty());
    let vres = a.text(8).filter(|x| !x.is_empty());
    let emit = a.text(9).filter(|x| !x.is_empty());
    let invoke = a.flag(20);
    if vreq.is_some() || vres.is_some() {
        if vreq.is_none() || vres.is_none() || invoke || emit.is_some() {
            return Ok(argp::error(spec, "--verify-request/--verify-result require both, without --invoke/--emit"));
        }
        let request = load_json_file(vreq.unwrap())?;
        let result = load_json_file(vres.unwrap())?;
        let v = verify_result(&request, &result)?;
        println!("{}", crate::py::dumps_pretty(&v));
        return Ok(if is_str(get(&v, "status"), "completed") { 0 } else { 4 });
    }
    let phase = a.text(3).unwrap_or("discovery").to_string();
    if phase == "caller" {
        let v = caller_plane_check(&repo(), a.text(4).unwrap_or("claude_code"));
        println!("{}", crate::py::dumps_pretty(&v));
        return Ok(if is_str(get(&v, "status"), "completed") { 0 } else { 4 });
    }
    let Some(topology) = a.text(1).filter(|x| !x.is_empty()) else {
        return Err(Exit::msg("[canary] --topology 필수(토폴로지는 인터뷰가 정한다 — 브랜치로 추론하지 않는다)"));
    };
    let manifest = match a.text(2).filter(|x| !x.is_empty()) {
        Some(m) => m.to_string(),
        None => format!("{}/output/{topology}/manifest.yaml", repo()),
    };
    let int_or_none = |i: usize| a.int(i).map_or(PyValue::None, PyValue::int);
    let opts = RequestOpts {
        max_turns: int_or_none(10),
        timeout_seconds: int_or_none(15),
        budget_source: opt(a.text(16)),
        model: opt(a.text(18)),
        backend: opt(a.text(19)),
        provider: a.text(11).unwrap_or("claude_code").to_string(),
        max_dispatches: a.int(12),
        sandbox_mode: opt(a.text(13)),
        permission_profile: opt(a.text(14)),
        phase: phase.clone(),
        target_role: a.text(5).unwrap_or("sub").to_string(),
        main_work_dir: opt(a.text(6)),
        reasoning_effort: opt(a.text(17)),
    };
    let req = build_request(topology, &manifest, &opts)?;
    let blob = format!("{}\n", crate::py::dumps_pretty(&req));
    if let Some(e) = emit {
        std::fs::write(e, &blob).map_err(|err| traceback(&crate::pathfmt::os_error(&err, e)))?;
        println!("[canary] request → {e} (max_turns={} timeout={}s · {})",
            crate::py::display_opt(get(&req, "max_turns")), crate::py::display_opt(get(&req, "timeout_seconds")),
            a.text(16).unwrap_or("None"));
    } else if !(invoke && phase == "permissions") {
        print!("{blob}");
    }
    if !invoke {
        return Ok(0);
    }
    let _ = std::io::stdout().flush();
    let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let tmp = base.join(format!("tmp{}{n:x}.json", std::process::id()));
    std::fs::write(&tmp, &blob).map_err(|e| traceback(&crate::pathfmt::os_error(&e, &tmp.display().to_string())))?;
    let launcher = scripts_dir().join("agent_control.sh").display().to_string();
    let tmps = tmp.display().to_string();
    let outcome = (|| -> Res<i32> {
        if phase == "permissions" {
            let invoked = crate::proc::run(&["bash".into(), launcher.clone(), "invoke".into(), "--request".into(), tmps.clone()], None, None)
                .map_err(|r| traceback(&r.0))?;
            let result = match pyjson::loads(&invoked.stdout) {
                Ok(r) => r,
                Err(_) => {
                    let v = dict(vec![("status", s("input-required")), ("phase", s("inspect")),
                        ("native_cli", dict(vec![("status", s("unproven")), ("returncode", PyValue::int(invoked.returncode as i64))])),
                        ("reason", s("NATIVE_RESULT_NOT_JSON")), ("raw_stdout", s(&invoked.stdout)), ("raw_stderr", s(&invoked.stderr))]);
                    println!("{}", crate::py::dumps_pretty(&v));
                    return Ok(4);
                }
            };
            let verification = verify_result(&req, &result)?;
            let done = is_str(get(&verification, "status"), "completed");
            println!("{}", crate::py::dumps_pretty(&dict(vec![("request", req.clone()), ("native_result", result),
                ("verification", verification)])));
            return Ok(if done { 0 } else { 4 });
        }
        let st = Command::new("bash").args([&launcher, "invoke", "--request", &tmps]).status()
            .map_err(|e| traceback(&crate::pathfmt::os_error(&e, "bash")))?;
        use std::os::unix::process::ExitStatusExt;
        Ok(st.code().unwrap_or_else(|| -st.signal().unwrap_or(0)))
    })();
    let _ = std::fs::remove_file(&tmp);
    outcome
}

pub fn main(args: &[String]) -> i32 {
    crate::sysexit::run(run(args))
}
