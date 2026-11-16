//! Port of `selftest_claude_native_events.py` — focused offline provider/transport regressions with a fake native CLI.
//! No model, no SSH.
//!
//! Event shapes follow Claude Code headless stream-json and complete SDK messages
//! (https://code.claude.com/docs/en/headless#stream-responses). They are synthetic protocol fixtures, never live
//! permission-readiness evidence.
//!
//! The original patched `provider.subprocess.run`; here the provider's own injection seam
//! (`provider_claude_code::invoke_with`) receives a recording stand-in with the same return value / exception. The
//! stand-in sees `(argv, cwd, timeout)`; `stdin=DEVNULL` is fixed by the `RunFn` contract of `agent_control::process`
//! (the real `subprocess_run` always spawns with a null standard input), so that keyword assertion of the original has
//! no separate observable here. The last case runs the real `agent_control.sh invoke` with a fake `claude` on `PATH`.

use super::{dumps, j, path_with, scripts_dir, stub, Checks, TempDir};
use crate::agent_control::process::{Completed, RunError};
use crate::agent_control::{
    load_schema, request_schema_violations, result_contract_violations, result_schema_path, schema_violations,
};
use crate::relay::pv::{self, g, py_str, s, set, V};
use std::cell::RefCell;

const FINAL: &str = r#"{"type": "result", "subtype": "success", "is_error": false,
    "result": "{\"phase\":\"inspect\",\"status\":\"completed\"}",
    "session_id": "fixture-session", "num_turns": 2, "duration_ms": 123, "duration_api_ms": 90,
    "permission_denials": [], "modelUsage": {"observed-model": {}}}"#;
const USE: &str = r#"{"type": "assistant", "session_id": "fixture-session", "message": {
    "role": "assistant", "content": [{"type": "tool_use", "id": "tool-1",
    "name": "Bash", "input": {"command": "/usr/bin/true"}}]}}"#;
const RECEIPT: &str = r#"{"type": "user", "session_id": "fixture-session", "message": {
    "role": "user", "content": [{"type": "tool_result", "tool_use_id": "tool-1",
    "is_error": false, "content": "HARNESS_PERMISSION_PROBE={\"ok\":true}"}]}}"#;

struct Call {
    argv: Vec<String>,
    timeout: Option<f64>,
}

enum Capture {
    Absent,
    Value(bool),
}

struct Fx {
    request: V,
    _tmp: TempDir,
    work: std::path::PathBuf,
}

fn lines(events: &[V]) -> String {
    events.iter().map(dumps).collect::<Vec<_>>().join("\n")
}

impl Fx {
    fn new() -> Fx {
        let tmp = TempDir::new("claude-native-events-");
        let work = tmp.path().to_path_buf();
        let request = j(&format!(
            r#"{{"schema_version": 1, "provider": "claude_code", "intent": "bootstrap_canary", "backend": "anthropic",
            "model": "declared-model", "task": "Synthetic offline canary", "max_turns": 8, "timeout_seconds": 10,
            "capabilities": ["read", "execute"],
            "target": {{"role": "main", "transport": "local", "work_dir": {}}}}}"#,
            dumps(&s(&work.display().to_string()))
        ));
        Fx { request, _tmp: tmp, work }
    }

    /// `invoke(events, rc=, raw=, capture=, error=)` of the original: one provider call with a stand-in process.
    fn invoke(&self, c: &mut Checks, label: &str, events: &[V], rc: i32, raw: Option<&str>, capture: Capture,
              error: Option<RunError>) -> (V, Call) {
        let mut req = self.request.clone();
        let captured = match capture {
            Capture::Absent => false,
            Capture::Value(b) => {
                set(&mut req, "capture_native_events", V::Bool(b));
                b
            }
        };
        let stdout = match raw {
            Some(r) => r.to_string(),
            None if captured => lines(events),
            None => dumps(&j(FINAL)),
        };
        let calls: RefCell<Vec<Call>> = RefCell::new(vec![]);
        let run = |argv: &[String], _cwd: Option<&str>, timeout: Option<f64>| -> Result<Completed, RunError> {
            calls.borrow_mut().push(Call { argv: argv.to_vec(), timeout });
            match &error {
                Some(e) => Err(e.clone()),
                None => Ok(Completed { returncode: rc, stdout: stdout.clone().into_bytes(), stderr: vec![] }),
            }
        };
        let result = crate::provider_claude_code::invoke_with(&req, &run);
        let schema = load_schema(&result_schema_path()).expect("result schema");
        let sv = schema_violations(&result, &schema, "$");
        let cv = result_contract_violations(&result, &req);
        c.ck(&format!("{label}: result schema and dispatch contract hold"), sv.is_empty() && cv.is_empty(),
             || format!("{sv:?} {cv:?}"));
        let call = calls.into_inner().pop().unwrap_or(Call { argv: vec![], timeout: None });
        (result, call)
    }
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn reasons(result: &V) -> Vec<String> {
    pv::items(g(result, "reason_codes")).iter().map(py_str).collect()
}

pub fn run() -> i32 {
    let mut c = Checks::new("selftest_claude_native_events");
    let (use_, receipt, final_) = (j(USE), j(RECEIPT), j(FINAL));

    c.group("default and false keep the json backend, resume and envelope");
    {
        let mut fx = Fx::new();
        set(&mut fx.request, "resume_session_id", s("existing-native"));
        for (name, capture) in [("absent", Capture::Absent), ("false", Capture::Value(false))] {
            let (result, call) = fx.invoke(&mut c, &format!("capture {name}"), &[], 0, None, capture, None);
            c.ck(&format!("capture {name}: json argv with --resume"), call.argv == strs(&[
                "claude", "-p", "Synthetic offline canary", "--model", "declared-model", "--output-format", "json",
                "--max-turns", "8", "--allowedTools", "Read,Bash", "--resume", "existing-native"]), || format!("{:?}", call.argv));
            c.ck(&format!("capture {name}: no raw_events · model_used observed · envelope"),
                 !pv::has(&result, "raw_events")
                     && pv::eq(g(&result, "model_used"), &j(r#"["observed-model"]"#))
                     && py_str(g(&result, "session_id")) == "fixture-session"
                     && pv::int(g(&result, "num_turns")) == Some(2)
                     && py_str(g(&result, "budget_outcome")) == "within_budget",
                 || dumps(&result));
        }
        set(&mut fx.request, "backend", s("claude-sub"));
        let (result, call) = fx.invoke(&mut c, "claude-sub", &[], 0, None, Capture::Value(false), None);
        c.ck("claude-sub: bash -ic wrapper · --resume last · no raw_events",
             call.argv.len() >= 4 && call.argv[..4] == strs(&["bash", "-ic", "claude-sub \"$@\"", "claude-sub"])
                 && call.argv[call.argv.len() - 2..] == strs(&["--resume", "existing-native"])
                 && !pv::has(&result, "raw_events"),
             || format!("{:?}", call.argv));
    }

    c.group("opt-in keeps the exact tool receipt without changing the terminal report");
    {
        let fx = Fx::new();
        let events = vec![use_.clone(), receipt.clone(), final_.clone()];
        let (result, call) = fx.invoke(&mut c, "opt-in", &events, 0, None, Capture::Value(true), None);
        let fmt = call.argv.iter().position(|a| a == "--output-format").map(|i| call.argv[i + 1].clone());
        c.ck("opt-in: raw_events exact · output = terminal result · completed",
             pv::eq(g(&result, "raw_events"), &V::List(events.clone()))
                 && py_str(g(&result, "output")) == py_str(g(&final_, "result"))
                 && py_str(g(&result, "status")) == "completed",
             || dumps(&result));
        c.ck("opt-in: stream-json · --verbose · timeout 10 (stdin is the RunFn's DEVNULL)",
             fmt.as_deref() == Some("stream-json") && call.argv.iter().any(|a| a == "--verbose") && call.timeout == Some(10.0),
             || format!("{:?} {:?}", call.argv, call.timeout));
    }

    c.group("denial, budget and runner failures keep classification and events");
    {
        let fx = Fx::new();
        let variants: [(&str, i32, &str, Option<&str>); 4] = [
            (r#"{"permission_denials": [{"tool_name": "Bash", "tool_input": {"command": "denied"}}]}"#, 0, "PERMISSION_DENIED", None),
            (r#"{"subtype": "error_max_turns", "is_error": true}"#, 1, "NONZERO_EXIT", Some("exhausted")),
            (r#"{"subtype": "error_during_execution", "is_error": true}"#, 1, "NONZERO_EXIT", Some("external_interruption")),
            (r#"{"terminal_reason": "api_error", "api_error_status": 401, "is_error": true}"#, 1, "RUNNER_UNAVAILABLE", None),
        ];
        for (changes, rc, reason, budget) in variants {
            let mut terminal = final_.clone();
            pv::update(&mut terminal, &j(changes));
            let (result, _) = fx.invoke(&mut c, changes, &[use_.clone(), receipt.clone(), terminal.clone()], rc, None,
                                        Capture::Value(true), None);
            let last = pv::items(g(&result, "raw_events")).last().cloned().unwrap_or(V::None);
            c.ck(&format!("{changes}: execution_failed · {reason} · budget {budget:?} · session kept · last event"),
                 py_str(g(&result, "status")) == "execution_failed"
                     && reasons(&result).iter().any(|r| r == reason)
                     && pv::eq(g(&result, "budget_outcome"), &budget.map_or(V::None, s))
                     && py_str(g(&result, "session_id")) == "fixture-session"
                     && pv::eq(&last, &terminal),
                 || dumps(&result));
        }
    }

    c.group("broken or ambiguous stream never claims completion");
    {
        let fx = Fx::new();
        let good = lines(&[use_.clone(), receipt.clone(), final_.clone()]);
        let variants = [
            dumps(&use_),
            format!("{good}\n{}", dumps(&final_)),
            format!("{good}\n{}", dumps(&use_)),
            format!("broken\n{good}"),
            format!("[]\n{good}"),
            format!("{{}}\n{good}"),
            String::new(),
            dumps(&V::List(vec![use_.clone(), receipt.clone(), final_.clone()])),
        ];
        for raw in &variants {
            let label: String = raw.chars().take(60).collect::<String>().replace('\n', "\\n");
            let (result, _) = fx.invoke(&mut c, &label, &[], 0, Some(raw), Capture::Value(true), None);
            c.ck(&format!("malformed_output [MALFORMED_JSON]: {label}"),
                 py_str(g(&result, "status")) == "malformed_output" && reasons(&result) == ["MALFORMED_JSON"],
                 || dumps(&result));
        }
    }

    c.group("timeout preserves partial native events without a synthetic terminal");
    {
        let fx = Fx::new();
        let partial = lines(&[use_.clone(), receipt.clone()]).into_bytes();
        let (result, _) = fx.invoke(&mut c, "timeout", &[], 0, Some(""), Capture::Value(true),
                                    Some(RunError::Timeout { stdout: partial }));
        c.ck("timeout: status · external_interruption · partial events · output None",
             py_str(g(&result, "status")) == "timeout"
                 && py_str(g(&result, "budget_outcome")) == "external_interruption"
                 && pv::eq(g(&result, "raw_events"), &V::List(vec![use_.clone(), receipt.clone()]))
                 && pv::is_none(g(&result, "output")),
             || dumps(&result));
    }

    c.group("opt-in schema and dispatch binding fail closed");
    {
        let fx = Fx::new();
        let with = |changes: &str| {
            let mut r = fx.request.clone();
            pv::update(&mut r, &j(changes));
            r
        };
        c.ck("capture true on bootstrap_canary is a valid request",
             request_schema_violations(&with(r#"{"capture_native_events": true}"#)).is_empty(), String::new);
        for changes in [r#"{"capture_native_events": "true"}"#, r#"{"capture_native_events": 1}"#,
                        r#"{"capture_native_events": true, "intent": "delegate"}"#] {
            c.ck(&format!("rejected request {changes}"), !request_schema_violations(&with(changes)).is_empty(), String::new);
        }
        let codex = with(r#"{"schema_version": 2, "provider": "codex_cli", "backend": "openai", "max_turns": null,
            "budget_unit": "wall_time", "max_dispatches": 1, "permission_profile": ":danger-full-access"}"#);
        c.ck("codex v2 request is valid", request_schema_violations(&codex).is_empty(), || format!("{:?}", request_schema_violations(&codex)));
        for value in [true, false] {
            let mut r = codex.clone();
            set(&mut r, "capture_native_events", V::Bool(value));
            c.ck(&format!("codex rejects capture_native_events={value}"), !request_schema_violations(&r).is_empty(), String::new);
        }
        let (plain, _) = fx.invoke(&mut c, "plain", &[], 0, None, Capture::Absent, None);
        c.ck("plain result violates a capture request",
             !result_contract_violations(&plain, &with(r#"{"capture_native_events": true}"#)).is_empty(), String::new);
        let (captured, _) = fx.invoke(&mut c, "captured", &[final_.clone()], 0, None, Capture::Value(true), None);
        c.ck("captured result violates a plain request", !result_contract_violations(&captured, &fx.request).is_empty(), String::new);
    }

    c.group("real local transport persists receipts in the private outcome");
    {
        // A harmless fake CLI through the real agent_control launcher, router, provider process, both schemas and the
        // durable outcome writer.
        let fx = Fx::new();
        let events = vec![use_.clone(), receipt.clone(), final_.clone()];
        let mut body = String::from(
            "verbose=0; fmt=\nprev=\nfor a in \"$@\"; do [ \"$a\" = --verbose ] && verbose=1; [ \"$prev\" = --output-format ] && fmt=\"$a\"; prev=\"$a\"; done\n\
             [ \"$verbose\" = 1 ] || { echo 'no --verbose' >&2; exit 9; }\n[ \"$fmt\" = stream-json ] || { echo \"format $fmt\" >&2; exit 9; }\n",
        );
        for e in &events {
            body.push_str(&format!("printf '%s\\n' {}\n", crate::agent_control::pyhelp::shlex_quote(&dumps(e))));
        }
        stub(&fx.work.join("claude"), &body);
        let mut req = fx.request.clone();
        set(&mut req, "capture_native_events", V::Bool(true));
        let request_path = fx.work.join("request.json");
        std::fs::write(&request_path, dumps(&req)).expect("request");
        let outcome = fx.work.join("private-outcome.json");
        let done = std::process::Command::new("bash")
            .arg(scripts_dir().join("agent_control.sh"))
            .args(["invoke", "--request"]).arg(&request_path)
            .arg("--result-file").arg(&outcome)
            .args(["--dispatch-id", "fixture-dispatch"])
            .env("PATH", path_with(&fx.work))
            .stdin(std::process::Stdio::null())
            .output()
            .expect("agent_control.sh");
        let stdout = String::from_utf8_lossy(&done.stdout).into_owned();
        let result = pv::loads(&stdout).unwrap_or(V::None);
        let durable = pv::load_file(&outcome).unwrap_or(V::None);
        c.ck("agent_control.sh invoke rc 0 · raw_events exact · durable outcome = result · dispatch id",
             done.status.code() == Some(0)
                 && pv::eq(g(&result, "raw_events"), &V::List(events))
                 && py_str(g(&durable, "dispatch_id")) == "fixture-dispatch"
                 && pv::eq(g(&durable, "result"), &result),
             || format!("rc={:?} stderr={}", done.status.code(), String::from_utf8_lossy(&done.stderr)));
    }
    c.finish()
}
