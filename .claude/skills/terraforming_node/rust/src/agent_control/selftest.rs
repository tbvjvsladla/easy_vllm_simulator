//! `agent_control --self-test`: the boundary, turn-exhaustion, runner-ladder, public-gate and Codex v2 regressions of
//! the original, with the provider process injected (the original monkeypatched `provider.subprocess.run`), so no
//! provider CLI, model or network is ever started. The worker-exclusion and deadline checks run real local processes
//! (bash fixtures in a temporary directory) through the Codex adapter's own process handling.

use super::process::{Completed, OsErr, RunError};
use super::pyhelp::{dict, get, int, py_eq, s, set, shlex_quote, shlex_split, str_of, without};
use super::*;
use crate::provider_claude_code as claude;
use crate::provider_codex_cli as codex;
use agent_runtime_support::pyvalue::PyValue;
use std::time::{Duration, Instant};

const AGENT_SOURCE: &str = include_str!("../agent_control.rs");
const PROVIDER_SOURCE: &str = include_str!("../provider_claude_code.rs");
const SELFTEST_MARKER: &str = "// --self-test (plan_26093022";

type T = Result<(), String>;

fn req(c: bool, m: impl FnOnce() -> String) -> T {
    if c {
        Ok(())
    } else {
        Err(m())
    }
}

fn request(transport: &str) -> PyValue {
    let mut target = dict(vec![("role", s("main")), ("transport", s(transport)), ("work_dir", s("/tmp/runtime probe"))]);
    if transport == "ssh" {
        set(&mut target, "role", s("sub"));
        set(&mut target, "host", s("192.0.2.10"));
        set(&mut target, "ssh_user", s("probe"));
    }
    dict(vec![
        ("schema_version", int(1)),
        ("provider", s("claude_code")),
        ("intent", s("probe")),
        ("task", s("read-only runtime probe")),
        ("model", s("sonnet")),
        ("timeout_seconds", int(60)),
        ("max_turns", int(1)),
        ("capabilities", PyValue::List(vec![s("read")])),
        ("target", target),
    ])
}

fn with(v: &PyValue, pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut o = v.clone();
    for (k, x) in pairs {
        set(&mut o, k, x);
    }
    o
}

fn completed(rc: i32, payload: Option<&PyValue>, stderr: &str) -> impl Fn(&[String], Option<&str>, Option<f64>) -> Result<Completed, RunError> {
    let stdout = payload.map(|p| p.dumps_ascii().unwrap()).unwrap_or_default();
    let stderr = stderr.to_string();
    move |_: &[String], _: Option<&str>, _: Option<f64>| {
        Ok(Completed { returncode: rc, stdout: stdout.clone().into_bytes(), stderr: stderr.clone().into_bytes() })
    }
}

fn result_schema() -> PyValue {
    load_schema(&result_schema_path()).expect("result schema")
}

fn violations_of(v: &PyValue) -> Vec<String> {
    schema_violations(v, &result_schema(), "$")
}

fn jd(v: &PyValue) -> String {
    v.repr()
}

fn obj(text: &str) -> PyValue {
    agent_runtime_support::pyjson::loads(text).expect("fixture json")
}

fn test_turn_exhaustion() -> T {
    let payload = obj(r#"{"type": "result", "subtype": "error_max_turns", "is_error": true, "num_turns": 26,
        "session_id": "sess-abc", "duration_ms": 812345, "duration_api_ms": 640000,
        "errors": ["Reached maximum number of turns (25)"], "result": "", "modelUsage": {}}"#);
    let r = with(&request("local"), vec![("max_turns", int(25))]);
    let res = claude::invoke_with(&r, &completed(1, Some(&payload), ""));
    req(get(&res, "budget_outcome").is_some_and(|v| py_eq(v, &s("exhausted"))), || {
        format!("turn exhaustion must be classified as exhausted, got {} -- an unreachable branch leaves the ledger with no basis to size the next attempt",
                get(&res, "budget_outcome").map(jd).unwrap_or("None".into()))
    })?;
    req(py_eq(get(&res, "num_turns").unwrap(), &int(26)) && py_eq(get(&res, "session_id").unwrap(), &s("sess-abc")),
        || format!("num_turns/session_id must survive a non-zero exit: {}", jd(&res)))?;
    req(py_eq(get(&res, "status").unwrap(), &s("execution_failed")),
        || "exhaustion is still a failure of that attempt -- it must not read as completed".into())?;
    let v = violations_of(&res);
    req(v.is_empty(), || format!("provider result violates result schema: {v:?}"))?;
    req(py_eq(get(&res, "duration_ms").unwrap(), &int(812345)) && py_eq(get(&res, "duration_api_ms").unwrap(), &int(640000)),
        || format!("provider-reported durations must survive a non-zero exit: {}", jd(&res)))?;
    let res2 = claude::invoke_with(&request("ssh"), &completed(255, None, "ssh: connect failed"));
    req(py_eq(get(&res2, "reason_codes").unwrap(), &PyValue::List(vec![s("NONZERO_EXIT")]))
        && matches!(get(&res2, "budget_outcome"), Some(PyValue::None))
        && matches!(get(&res2, "num_turns"), Some(PyValue::None)),
        || format!("a transport failure has no budget story -- it must stay null, got {}", jd(&res2)))?;
    req(matches!(get(&res2, "duration_ms"), Some(PyValue::None)) && matches!(get(&res2, "duration_api_ms"), Some(PyValue::None)),
        || format!("unmeasured durations stay null (0 would read as 'finished instantly'): {}", jd(&res2)))?;
    for rc in [124, 143] {
        let res3 = claude::invoke_with(&request("ssh"), &completed(rc, None, "Terminated"));
        req(get(&res3, "budget_outcome").is_some_and(|v| py_eq(v, &s("external_interruption"))),
            || format!("rc={rc} is an external interruption, not a budget outcome: {}", jd(&res3)))?;
    }
    Ok(())
}

fn test_boundary() -> T {
    let request_schema = load_schema(&request_schema_path()).map_err(|e| e)?;
    let rs = result_schema();
    req(!schema_violations(&dict(vec![]), &request_schema, "$").is_empty(),
        || "empty agent request unexpectedly passed schema validation".into())?;
    let invalid = invalid_request_result(Some(&dict(vec![])));
    req(schema_violations(&invalid, &rs, "$").is_empty(), || "invalid-request fail-closed envelope violates result schema".into())?;
    req(AGENT_SOURCE.contains(SELFTEST_MARKER), || "self-test section marker missing -- boundary scan would be void".into())?;
    let agent = AGENT_SOURCE.split(SELFTEST_MARKER).next().unwrap();
    for token in ["claude -p", "--model", "--output-format json"] {
        req(!agent.contains(token), || format!("provider-specific token leaked into orchestrator: {token}"))?;
        req(PROVIDER_SOURCE.contains(token), || format!("provider adapter lost required CLI token: {token}"))?;
    }
    let local = claude::build_argv(&request("local")).unwrap();
    req(local[0] == "claude" && local.iter().any(|a| a == "--model"), || format!("local provider argv malformed: {local:?}"))?;
    let ssh = claude::build_argv(&request("ssh")).unwrap();
    req(ssh[0] == "ssh", || format!("SSH provider argv malformed: {ssh:?}"))?;
    let has = |x: &str| ssh.iter().any(|a| a == x);
    req(has("-o") && has("BatchMode=yes") && has("ConnectTimeout=8"),
        || format!("SSH delegation must never be able to prompt on a tty: {ssh:?}"))?;
    let dd = ssh.iter().position(|a| a == "--").unwrap_or(0);
    req(ssh[ssh.len() - 2] == "probe@192.0.2.10" && ssh[dd + 1] == "probe@192.0.2.10",
        || format!("SSH destination misplaced: {ssh:?}"))?;
    let remote = shlex_split(ssh.last().unwrap());
    req(remote.len() > 2 && remote[0] == "bash" && remote[1] == "-lc" && remote[2].starts_with("cd '/tmp/runtime probe' && "),
        || format!("SSH work_dir is not safely shell-quoted: {}", ssh.last().unwrap()))?;
    req(remote[2].contains("timeout ") && remote[2].contains(" claude "),
        || format!("remote command must be wrapped in `timeout` so the sub agent dies with the client: {}", remote[2]))?;
    let remote_cmd = remote[2].split_once("&&").map(|x| x.1.trim().to_string()).unwrap_or_default();
    req(remote_cmd.starts_with("CLAUDE_CODE_RETRY_WATCHDOG=1 timeout "),
        || format!("sub relay must set the retry watchdog before `timeout`: {}", super::pyhelp::head_chars(&remote_cmd, 120)))?;
    req(!local.join(" ").contains("CLAUDE_CODE_RETRY_WATCHDOG"),
        || "local transport is the main node's own plane -- the sub relay env must not leak into it".into())?;

    let opus = obj(r#"{"type": "result", "subtype": "success", "is_error": false, "num_turns": 2, "session_id": "sess-opus",
        "result": "done", "duration_ms": 4200, "duration_api_ms": 3900,
        "modelUsage": {"claude-opus-5": {"canonicalModel": "claude-opus-5"}}}"#);
    let declared = with(&request("local"), vec![("model", s("opus"))]);
    let result = claude::invoke_with(&declared, &completed(0, Some(&opus), ""));
    req(py_eq(get(&result, "status").unwrap(), &s("completed")) && py_eq(get(&result, "exit_code").unwrap(), &int(0)),
        || format!("the model is a declaration, not a gate -- opus must run: {}", jd(&result)))?;
    req(py_eq(get(&result, "model_used").unwrap(), &PyValue::List(vec![s("claude-opus-5")]))
        && py_eq(get(&result, "model_requested").unwrap(), &s("opus")),
        || format!("the model that actually ran must be recorded: {}", jd(&result)))?;
    req(violations_of(&result).is_empty(), || format!("non-Sonnet completed result must satisfy the result schema: {}", jd(&result)))?;
    let bare = without(&opus, "modelUsage");
    let bare_result = claude::invoke_with(&request("local"), &completed(0, Some(&bare), ""));
    req(py_eq(get(&bare_result, "status").unwrap(), &s("completed")) && py_eq(get(&bare_result, "model_used").unwrap(), &PyValue::List(vec![])),
        || format!("absent model metadata must be recorded as empty, not blocked: {}", jd(&bare_result)))?;
    req(violations_of(&bare_result).is_empty(),
        || format!("metadata-less completed result must satisfy the result schema: {}", jd(&bare_result)))?;
    let denied = obj(r#"{"type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
        "session_id": "sess-deny", "result": "…", "duration_ms": 10, "duration_api_ms": 5,
        "permission_denials": [{"tool_name": "Write", "tool_input": {"path": "x"}}],
        "modelUsage": {"claude-sonnet-4-5": {"canonicalModel": "claude-sonnet-4-5"}}}"#);
    let dr = claude::invoke_with(&request("local"), &completed(0, Some(&denied), ""));
    let out = get(&dr, "output").filter(|o| o.is_truthy()).map(str_of).unwrap_or_default();
    req(py_eq(get(&dr, "reason_codes").unwrap(), &PyValue::List(vec![s("PERMISSION_DENIED")])) && out.contains("permission_denials"),
        || format!("permission denial must carry its diagnosis: {}", jd(&dr)))?;
    let v = violations_of(&dr);
    req(v.is_empty(), || format!("permission-denied result must satisfy the result schema: {v:?}"))?;
    Ok(())
}

fn rotate_of(ev: &Option<PyValue>) -> Option<bool> {
    ev.as_ref().and_then(|e| match get(e, "rotate") {
        Some(PyValue::Bool(b)) => Some(*b),
        _ => None,
    })
}

fn test_runner_ladder() -> T {
    let cls = |p: Option<&PyValue>, rc: i32| claude::classify_runner_failure(p, rc, "ssh");
    let ok = obj(r#"{"type": "result", "subtype": "success", "is_error": false, "terminal_reason": "completed",
        "api_error_status": null, "session_id": "s-ok", "num_turns": 1}"#);
    req(cls(Some(&ok), 0).is_none(), || "정상 봉투가 러너 실패로 판정됐다".into())?;
    let e401 = obj(r#"{"type": "result", "subtype": "success", "is_error": true, "terminal_reason": "api_error",
        "api_error_status": 401, "session_id": "e939c9b7", "num_turns": 1, "duration_api_ms": 0,
        "result": "Failed to authenticate. API Error: 401 ..."}"#);
    let ev = cls(Some(&e401), 1);
    req(rotate_of(&ev) == Some(true) && ev.as_ref().is_some_and(|e| py_eq(get(e, "api_error_status").unwrap(), &int(401))),
        || format!("401 인증실패가 회전 대상으로 판정되지 않았다: {}", ev.as_ref().map(jd).unwrap_or("None".into())))?;
    let dns = obj(r#"{"type": "result", "subtype": "success", "is_error": true, "terminal_reason": "api_error",
        "api_error_status": null, "session_id": "e14d974b", "num_turns": 1, "duration_api_ms": 0,
        "result": "API Error: Can't reach the API server (ENOTFOUND)"}"#);
    let ev = cls(Some(&dns), 1);
    req(rotate_of(&ev) == Some(true) && ev.as_ref().is_some_and(|e| matches!(get(e, "api_error_status"), Some(PyValue::None))),
        || format!("백엔드 미도달이 회전 대상으로 판정되지 않았다: {}", ev.as_ref().map(jd).unwrap_or("None".into())))?;
    let ev = cls(None, 127);
    req(rotate_of(&ev) == Some(true) && ev.as_ref().is_some_and(|e| py_eq(get(e, "signal").unwrap(), &s("missing_binary"))),
        || "원격 바이너리 부재(rc 127)가 회전 대상이 아니다".into())?;
    req(cls(None, 255).is_none(), || "★ssh 전송 실패(rc 255)는 회전 대상이 아니다 — 백엔드를 바꿔도 낫지 않는다".into())?;
    let ev = cls(Some(&with(&e401, vec![("api_error_status", int(400))])), 1);
    req(rotate_of(&ev) == Some(false), || "★요청 자체가 거절된 것(400)은 회전해도 같은 거절을 받는다".into())?;
    let subfail = obj(r#"{"type": "result", "subtype": "success", "is_error": true, "terminal_reason": "completed",
        "session_id": "s-real", "num_turns": 18, "duration_api_ms": 40000, "result": "빌드가 실패했다"}"#);
    req(cls(Some(&subfail), 1).is_none(), || "★음성대조: 서브 과업 실패가 러너 실패로 접혔다(사다리를 태우는 형태)".into())?;
    let res = claude::invoke_with(&request("ssh"), &completed(1, Some(&e401), ""));
    req(py_eq(get(&res, "reason_codes").unwrap(), &PyValue::List(vec![s("RUNNER_UNAVAILABLE")])),
        || format!("401 이 RUNNER_UNAVAILABLE 로 오지 않았다: {}", jd(get(&res, "reason_codes").unwrap())))?;
    req(py_eq(get(&res, "session_id").unwrap(), &s("e939c9b7")) && py_eq(get(&res, "num_turns").unwrap(), &int(1)),
        || "★러너 실패 결과가 세션·턴을 버렸다(종전 IS_ERROR 경로의 회귀)".into())?;
    let out = get(&res, "output").filter(|o| o.is_truthy()).map(str_of).unwrap_or_default();
    req(out.contains("runner_unavailable") && out.contains("401"),
        || format!("증거가 output 에 실려 오지 않았다: {}", jd(get(&res, "output").unwrap())))?;
    req(matches!(get(&res, "budget_outcome"), Some(PyValue::None)),
        || "러너 실패에 예산 서사를 붙였다 — 예산을 키워도 죽은 백엔드는 살아나지 않는다".into())?;
    req(violations_of(&res).is_empty(), || "러너 실패 결과가 결과 스키마를 위반한다".into())?;
    let aliases: Vec<&str> = claude::RUNNER_ALIASES.iter().map(|(n, _)| *n).collect();
    let want = ["opus", "sonnet", "haiku", "kimi-claude", "minimax-claude", "meta-claude", "openai-claude"];
    req(want.iter().all(|w| aliases.contains(w)), || {
        let mut a = aliases.clone();
        a.sort();
        format!("러너 별칭 표가 좁다: {a:?}")
    })?;
    for (name, (b, _)) in claude::RUNNER_ALIASES {
        req(claude::backend_binary(b).is_some(), || format!("별칭 {name} 의 backend {b} 가 바이너리 표에 없다(닫힌 열거가 갈라졌다)"))?;
    }
    let schema = load_schema(&request_schema_path())?;
    let mut en: Vec<String> = match get(get(get(&schema, "properties").unwrap(), "backend").unwrap(), "enum") {
        Some(PyValue::List(l)) => l.iter().map(str_of).collect(),
        _ => vec![],
    };
    en.sort();
    let mut bin: Vec<String> = claude::BACKEND_TO_BINARY.iter().map(|(b, _)| b.to_string()).collect();
    bin.sort();
    let mut mdl: Vec<String> = claude::BACKEND_DEFAULT_MODEL.iter().map(|(b, _)| b.to_string()).collect();
    mdl.sort();
    req(en == bin && bin == mdl, || format!("백엔드 어휘가 갈라졌다: schema={en:?} binary={bin:?} model={mdl:?}"))?;
    for unknown in [int(418), int(499), int(999), s("429"), PyValue::Bool(true), dict(vec![]), PyValue::List(vec![])] {
        let ev = cls(Some(&with(&e401, vec![("api_error_status", unknown.clone())])), 1);
        req(rotate_of(&ev) == Some(false), || format!("unknown/wrong-typed HTTP status opened rotation: {}", jd(&unknown)))?;
    }
    let absent = without(&e401, "api_error_status");
    req(rotate_of(&cls(Some(&absent), 1)) == Some(false),
        || "missing HTTP status was confused with an observed connection-failure null".into())?;
    for (filename, transport, rotates) in [("/missing/workspace", "local", false), ("claude", "local", true), ("ssh", "ssh", false)] {
        let missing = move |_: &[String], _: Option<&str>, _: Option<f64>| -> Result<Completed, RunError> {
            Err(RunError::Os(OsErr { class: "FileNotFoundError", errno: 2, strerror: "fixture missing path".into(),
                                     filename: filename.into() }))
        };
        let value = claude::invoke_with(&request(transport), &missing);
        let codes = get(&value, "reason_codes").cloned().unwrap_or(PyValue::None);
        let has_ru = matches!(&codes, PyValue::List(l) if l.iter().any(|c| py_eq(c, &s("RUNNER_UNAVAILABLE"))));
        req(has_ru == rotates, || format!("launch path {} was assigned to the wrong failure plane: {}",
                                          super::pyhelp::repr_str(filename), jd(&value)))?;
    }
    Ok(())
}

fn rsv(v: &PyValue) -> Vec<String> {
    request_schema_violations(v)
}

fn stream(events: &[PyValue]) -> String {
    events.iter().map(|e| format!("{}\n", e.dumps_ascii().unwrap())).collect()
}

fn test_codex() -> T {
    let mut request = request("local");
    for (k, v) in [("schema_version", int(2)), ("provider", s("codex_cli")), ("model", s("gpt-6.1-sol")),
                   ("max_turns", PyValue::None), ("budget_unit", s("wall_time")), ("max_dispatches", int(2)),
                   ("sandbox_mode", s("read-only"))] {
        set(&mut request, k, v);
    }
    req(rsv(&request).is_empty(), || "legacy v2 wall-time request rejected".into())?;
    let uncapped = without(&request, "max_dispatches");
    req(rsv(&uncapped).is_empty(), || "per-invocation v2 request rejected without cumulative count".into())?;
    req(!rsv(&without(&uncapped, "timeout_seconds")).is_empty(), || "v2 request without timeout accepted".into())?;
    for (key, value) in [("max_turns", int(1)), ("max_dispatches", int(0)), ("budget_unit", s("native_turns"))] {
        let shown = value.repr();
        req(!rsv(&with(&request, vec![(key, value)])).is_empty(), || format!("unsupported v2 budget accepted: {key}={shown}"))?;
    }
    for (version, name) in [(1, "codex_cli"), (2, "claude_code")] {
        let bad = with(&request, vec![("schema_version", int(version)), ("provider", s(name))]);
        req(!rsv(&bad).is_empty(), || "provider/version mismatch accepted".into())?;
    }
    let bad = with(&request, vec![("provider", s("unknown"))]);
    req(violations_of(&invalid_request_result(Some(&bad))).is_empty(), || "unknown-provider failure envelope invalid".into())?;
    req(violations_of(&invalid_request_result(Some(&request))).is_empty(), || "v2 invalid-request failure envelope invalid".into())?;
    req(violations_of(&invalid_provider_result(&request)).is_empty(), || "v2 invalid-provider failure envelope invalid".into())?;
    let mut resume = with(&request, vec![("resume_session_id", s("thread-1"))]);
    req(!rsv(&resume).is_empty(), || "unbound native resume accepted".into())?;
    let target = get(&request, "target").cloned().unwrap();
    set(&mut resume, "resume_binding", dict(vec![("provider", s("codex_cli")), ("target", target.clone()),
                                                 ("sandbox_mode", s("read-only"))]));
    req(rsv(&resume).is_empty(), || "bound native resume rejected".into())?;
    for change in [vec![("provider", s("claude_code"))], vec![("sandbox_mode", s("workspace-write"))],
                   vec![("target", with(&target, vec![("work_dir", s("/other/workspace"))]))]] {
        let shown = dict(change.clone()).repr();
        let mut bad = resume.clone();
        let b = with(get(&bad, "resume_binding").unwrap(), change);
        set(&mut bad, "resume_binding", b);
        req(!rsv(&bad).is_empty(), || format!("mismatched native resume accepted: {shown}"))?;
    }
    let named = with(&without(&request, "sandbox_mode"), vec![("permission_profile", s(":danger-full-access"))]);
    req(rsv(&named).is_empty(), || "named-profile v2 request rejected".into())?;
    req(!rsv(&with(&named, vec![("sandbox_mode", s("workspace-write"))])).is_empty(),
        || "simultaneous named profile and legacy sandbox accepted".into())?;
    req(!rsv(&with(&named, vec![("permission_profile", s("vllm-sub"))])).is_empty(), || "retired role-specific profile accepted".into())?;
    let sub = with(&request, vec![("target", dict(vec![("role", s("sub")), ("transport", s("ssh")), ("host", s("192.0.2.10")),
                                                       ("ssh_user", s("probe")), ("work_dir", s("/tmp/sub"))]))]);
    req(!rsv(&with(&sub, vec![("sandbox_mode", s("workspace-write"))])).is_empty(),
        || "legacy workspace-write override bypassed sub role protection".into())?;
    req(rsv(&sub).is_empty(), || "read-only sub canary rejected".into())?;
    req(!rsv(&with(&request, vec![("target", with(&target, vec![("work_dir", s("relative"))]))])).is_empty(),
        || "relative workspace was accepted despite workspace lock identity ambiguity".into())?;
    req(!rsv(&with(&request, vec![("target", without(&target, "work_dir"))])).is_empty(),
        || "missing v2 workspace crashed or passed the gate".into())?;
    let named_argv = codex::build_argv(&named);
    req(!named_argv.iter().any(|a| a == "--sandbox") && named_argv.iter().any(|a| a == "default_permissions=\":danger-full-access\""),
        || format!("named profile was overridden by legacy sandbox: {named_argv:?}"))?;
    let resume_argv = codex::build_argv(&resume);
    req(resume_argv.iter().any(|a| a == "resume") && !resume_argv.iter().any(|a| a == "--last")
        && resume_argv.iter().any(|a| a == "sandbox_mode=\"read-only\""),
        || "resume must retain explicit sandbox and native ID".into())?;
    let task = "--dangerously-bypass-approvals-and-sandbox; $(touch /tmp/must-not-exist)";
    let work = "/tmp/work space'$(false)";
    let hostile = with(&request, vec![("task", s(task)),
        ("target", dict(vec![("role", s("sub")), ("transport", s("ssh")), ("ssh_user", s("probe")), ("host", s("192.0.2.10")),
                             ("work_dir", s(work))]))]);
    let argv = codex::build_argv(&hostile);
    let script = shlex_split(argv.last().unwrap()).get(2).cloned().unwrap_or_default();
    req(script.contains(&shlex_quote(task)) && script.contains(&shlex_quote(work)), || "SSH prompt/workspace shell quoting changed".into())?;
    // The remote deadline owner is a bash program (it was `python3 -c`): process-group TERM, setsid, the reaped marker.
    req(argv.iter().any(|a| a == "BatchMode=yes") && script.contains("adapter.deadline_reaped")
        && script.contains("kill -TERM") && script.contains("setsid") && !script.contains("python3"),
        || "SSH must have noninteractive identity and remote bounded termination".into())?;
    let inner = codex::inner_argv(&hostile);
    req(inner[inner.len() - 2..] == ["--".to_string(), task.to_string()], || "hostile prompt was not terminated as a positional operand".into())?;

    let started = vec![obj(r#"{"type": "thread.started", "thread_id": "thread-1"}"#), obj(r#"{"type": "turn.started"}"#)];
    let message = obj(r#"{"type": "item.completed", "item": {"type": "agent_message", "text": "final report"}}"#);
    let done = obj(r#"{"type": "turn.completed", "usage": {"input_tokens": 12, "output_tokens": 4}}"#);
    let cat = |extra: &[PyValue]| -> String {
        let mut v = started.clone();
        v.extend(extra.iter().cloned());
        stream(&v)
    };
    let good = cat(&[message.clone(), done.clone()]);
    let parsed = |raw: &str, rc: i32, timed_out: bool| -> Result<PyValue, String> {
        let v = codex::parse_output(&request, raw, "fixture diagnostic", rc, timed_out, false, None);
        let viol = violations_of(&v);
        req(viol.is_empty(), || format!("Codex result schema violation: {viol:?}"))?;
        Ok(v)
    };
    let success = parsed(&good, 0, false)?;
    req(!result_contract_violations(&with(&success, vec![("provider", s("claude_code"))]), &request).is_empty(),
        || "result from another provider was accepted as the dispatched provider".into())?;
    req(py_eq(get(&success, "status").unwrap(), &s("completed")) && py_eq(get(&success, "session_id").unwrap(), &s("thread-1"))
        && py_eq(get(&success, "usage").unwrap(), get(&done, "usage").unwrap())
        && matches!(get(&success, "num_turns"), Some(PyValue::None))
        && py_eq(get(&success, "model_used").unwrap(), &PyValue::List(vec![]))
        && matches!(get(&success, "duration_ms"), Some(PyValue::None)),
        || format!("successful native observations were synthesized or lost: {}", jd(&success)))?;
    for raw in [format!("{good}{{\"type\":"), cat(&[message.clone()]),
                cat(&[obj(r#"{"type": "item.completed", "item": {"type": "agent_message", "text": "  "}}"#), done.clone()]),
                cat(&[obj(r#"{"type": "item.completed", "item": {"type": "command_execution", "aggregated_output": "not a final response"}}"#), done.clone()])] {
        let r = parsed(&raw, 0, false)?;
        req(py_eq(get(&r, "status").unwrap(), &s("malformed_output")) && py_eq(get(&r, "session_id").unwrap(), &s("thread-1")),
            || "truncated/malformed/empty/non-agent output was accepted or lost its session".into())?;
    }
    let fe = |v: &PyValue, k: &str| get(get(v, "failure_evidence").unwrap_or(&PyValue::None), k).cloned().unwrap_or(PyValue::None);
    for rc in [0, 1] {
        let failed = parsed(&cat(&[obj(r#"{"type": "turn.failed", "error": {"message": "quota"}}"#)]), rc, false)?;
        req(py_eq(get(&failed, "status").unwrap(), &s("execution_failed"))
            && matches!(get(&failed, "execution_started"), Some(PyValue::Bool(true)))
            && matches!(fe(&failed, "rotate"), PyValue::Bool(false)),
            || "native failure or late effects permitted automatic replay".into())?;
    }
    let denied = parsed(&cat(&[obj(r#"{"type": "turn.failed", "error": {"code": "permission_denied"}}"#)]), 0, false)?;
    req(py_eq(get(&denied, "reason_codes").unwrap(), &PyValue::List(vec![s("PERMISSION_DENIED")]))
        && !fe(&denied, "rotate").is_truthy(), || "permission denial was a rotation signal".into())?;
    let reported = parsed(&cat(&[obj(r#"{"type": "error", "code": "permission_denied"}"#), message.clone(), done.clone()]), 0, false)?;
    req(get(&reported, "output").map(str_of).unwrap_or_default().contains("final report"),
        || "permission event discarded the final native task report needed by outer completion gates".into())?;
    let timed = parsed(&cat(&[]), -15, true)?;
    req(py_eq(get(&timed, "status").unwrap(), &s("timeout")) && py_eq(get(&timed, "session_id").unwrap(), &s("thread-1"))
        && py_eq(get(&timed, "budget_outcome").unwrap(), &s("external_interruption")), || "timeout lost partial observations".into())?;
    let unknown = parsed("", 1, false)?;
    req(matches!(get(&unknown, "execution_started"), Some(PyValue::None)) && !fe(&unknown, "rotate").is_truthy(),
        || "unobserved execution was treated as safely replayable".into())?;

    // End-to-end process lifetime check, no Codex/model/network: the child ignores TERM; the adapter must kill the
    // entire original process group after its grace period.
    let temp = std::env::temp_dir().join(format!("codex-provider-selftest-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let r = lifetime_checks(&request, &temp);
    let _ = std::fs::remove_dir_all(&temp);
    r
}

fn lifetime_checks(request: &PyValue, temp: &std::path::Path) -> T {
    let script = temp.join("fixture.sh");
    let pid_path = temp.join("child.pid");
    std::fs::write(&script, "( trap '' TERM; exec sleep 60 ) &\nprintf '%s' \"$!\" > \"$1\"\n\
        printf '%s\\n' '{\"type\": \"thread.started\", \"thread_id\": \"timeout-thread\"}' '{\"type\": \"turn.started\"}'\nsleep 60\n")
        .map_err(|e| e.to_string())?;
    let (sp, pp) = (script.display().to_string(), pid_path.display().to_string());
    let inner = move |_: &PyValue| vec!["bash".to_string(), sp.clone(), pp.clone()];
    let cfg = codex::Config { inner: Some(&inner), grace: 0.15 };
    let t = temp.display().to_string();
    let limited = with(request, vec![("timeout_seconds", int(1)),
                                     ("target", dict(vec![("role", s("main")), ("transport", s("local")), ("work_dir", s(&t))]))]);
    let before = Instant::now();
    let value = codex::invoke_with(&limited, &cfg);
    req(before.elapsed() < Duration::from_secs(5) && py_eq(get(&value, "status").unwrap(), &s("timeout"))
        && py_eq(get(&value, "session_id").unwrap(), &s("timeout-thread")), || "deadline did not cleanly terminate fixture".into())?;
    let child: i64 = std::fs::read_to_string(&pid_path).ok().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
    let state = std::fs::read_to_string(format!("/proc/{child}/stat")).ok()
        .and_then(|t| t.rsplit_once(')').map(|x| x.1.split_whitespace().next().unwrap_or("").to_string()));
    req(state.is_none() || state.as_deref() == Some("Z"), || "timeout left a live TERM-ignoring descendant".into())?;
    let entered = temp.join("entered");
    std::fs::write(&script, "printf 'entered\\n' >> \"$1\"\nsleep 60\n").map_err(|e| e.to_string())?;
    let (sp, ep) = (script.display().to_string(), entered.display().to_string());
    let inner2 = move |_: &PyValue| vec!["bash".to_string(), sp.clone(), ep.clone()];
    let cfg2 = codex::Config { inner: Some(&inner2), grace: 0.15 };
    let locking = with(&limited, vec![("timeout_seconds", int(5))]);
    let mut first = codex::Worker::start(&codex::build_argv_with(&locking, &inner2(&locking)), Some(&t))
        .map_err(|e| e.text())?;
    let res = (|| {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !entered.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        req(entered.exists(), || "first worker did not acquire the lock".into())?;
        let busy = codex::invoke_with(&locking, &cfg2);
        let fe = |k: &str| get(get(&busy, "failure_evidence").unwrap_or(&PyValue::None), k).cloned().unwrap_or(PyValue::None);
        req(py_eq(&fe("signal"), &s("worker_busy")) && matches!(get(&busy, "execution_started"), Some(PyValue::Bool(false)))
            && matches!(fe("rotate"), PyValue::Bool(false))
            && std::fs::read_to_string(&entered).unwrap_or_default() == "entered\n",
            || "workspace lock allowed a duplicate execution or rotated on contention".into())
    })();
    first.stop(0.15);
    res
}

/// The whole self-test; prints the original's PASS line, or `self-test FAIL: <reason>` on standard error (status 1).
pub fn run() -> i32 {
    let r: T = (|| {
        req(request_schema_path().is_file() && result_schema_path().is_file(),
            || format!("schema paths must resolve from the repo root (REPO_ROOT={})", repo_root().display()))?;
        req(rsv(&request("ssh")).is_empty(), || "the public transport gate must accept a well-formed ssh request".into())?;
        req(!rsv(&dict(vec![])).is_empty(), || "the public transport gate must reject an empty request".into())?;
        test_turn_exhaustion()?;
        test_boundary()?;
        test_runner_ladder()?;
        test_codex()?;
        Ok(())
    })();
    match r {
        Ok(()) => {
            println!("[agent_control] self-test PASS (boundary · turn exhaustion · runner ladder · public gate · Codex v2 · worker exclusion)");
            0
        }
        Err(m) => {
            eprintln!("[agent_control] self-test FAIL: {m}");
            1
        }
    }
}
