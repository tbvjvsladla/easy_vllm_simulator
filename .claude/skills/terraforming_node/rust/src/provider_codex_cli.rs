//! Port of `providers/codex_cli.py` — the Codex CLI v2 adapter: JSONL, explicit permissions, native-ID resume and a
//! wall deadline. CLI grammar is isolated here. Same argv, result envelopes, classification and process handling
//! (own process group, deadline, TERM then KILL of the group, workspace flock) as the original.
//!
//! One intentional difference: the remote watchdog the SSH transport runs on the sub was a `python3 -c` program; it is
//! now the bash program [`REMOTE_WATCHDOG`] passed as `bash -c <program> a2a-watchdog <seconds> <digest> <worker argv>`
//! (same marker protocol: `adapter.framing_started`, base64 `adapter.child_line` frames, `adapter.deadline_reaped` /
//! `adapter.worker_exited`, exit 124 / the worker's status). The sub therefore needs bash, coreutils `base64` and
//! util-linux `setsid`/`flock`, never Python.

use crate::agent_control::process::{returncode, spawn, OsErr};
use crate::agent_control::pyhelp::{
    b64decode_strict, b64encode, dict, dumps_ascii, dumps_ascii_tight, dumps_sorted_tight_unicode, dumps_unicode, get,
    get_text, int, py_eq, s, set, sha256_hex, shlex_quote, str_of,
};
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Read;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PROVIDER_NAME: &str = "codex_cli";
pub const BACKEND_DEFAULT_MODEL: &[(&str, &str)] = &[("openai", "gpt-6.1-sol")];
pub const RUNNER_ALIASES: &[(&str, (&str, &str))] = &[
    ("codex-gpt-6.1-sol", ("openai", "gpt-6.1-sol")),
    ("codex-gpt-6.1-terra", ("openai", "gpt-6.1-terra")),
];

/// `SUPPORTS`.
pub fn supports() -> PyValue {
    let l = |x: &[&str]| PyValue::List(x.iter().map(|v| s(v)).collect());
    dict(vec![
        ("schema_versions", PyValue::List(vec![int(2)])),
        ("budget_units", l(&["wall_time"])),
        ("native_turn_limit", PyValue::Bool(false)),
        ("resume", s("explicit_native_id_with_binding")),
        ("structured_output", s("jsonl")),
        ("capabilities_mode", s("intent_only")),
        ("sandbox_modes", l(&["read-only", "workspace-write"])),
        ("permission_profiles", l(&[":danger-full-access"])),
        ("permission_profile_verification", s("deployment_and_native_work_canary_required")),
        ("cancellation", s("local_process_group_and_remote_deadline")),
        ("workspace_exclusion", s("nonblocking_flock_in_output")),
    ])
}

pub const SSH_HARDENING: &[&str] = crate::provider_claude_code::SSH_HARDENING;
pub const KILL_GRACE_SECONDS: f64 = 3.0;
pub const LOCK_CONFLICT_EXIT: i32 = 75;

fn text_of(v: Option<&PyValue>) -> String {
    v.map(str_of).unwrap_or_default()
}

/// `_locked_argv(inner, work_dir)`.
pub fn locked_argv(inner: &[String], work_dir: &str) -> Vec<String> {
    let marker = r#"{"type":"adapter.lock_acquired"}"#;
    let script = format!("printf '%s\\n' {}; exec \"$@\"", shlex_quote(marker));
    let lock = if work_dir.ends_with('/') {
        format!("{work_dir}output/.a2a-worker.lock")
    } else {
        format!("{work_dir}/output/.a2a-worker.lock")
    };
    let mut v: Vec<String> = ["flock", "--nonblock", "--conflict-exit-code"].iter().map(|x| x.to_string()).collect();
    v.push(LOCK_CONFLICT_EXIT.to_string());
    v.push(lock);
    v.extend(["bash".to_string(), "-c".into(), script, "a2a-worker".into()]);
    v.extend(inner.iter().cloned());
    v
}

/// `_inner_argv(request)`.
pub fn inner_argv(request: &PyValue) -> Vec<String> {
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let mut argv: Vec<String> = vec!["codex".into(), "exec".into(), "--cd".into(), text_of(get(&target, "work_dir"))];
    let resume = get(request, "resume_session_id").filter(|r| r.is_truthy()).map(str_of);
    if resume.is_some() {
        argv.push("resume".into());
    }
    argv.extend(["--json".into(), "--model".into(), text_of(get(request, "model")), "-c".into(),
                 "approval_policy=\"never\"".into()]);
    match get(request, "reasoning_effort") {
        None | Some(PyValue::None) => {}
        Some(PyValue::Str(e)) if e.equals("default") => {}
        Some(e) => {
            argv.push("-c".into());
            argv.push(format!("model_reasoning_effort={}", dumps_ascii(e)));
        }
    }
    if let Some(p) = crate::agent_control::pyhelp::has(request, "permission_profile").then(|| get(request, "permission_profile")).flatten() {
        argv.push("-c".into());
        argv.push(format!("default_permissions={}", dumps_ascii(p)));
    } else if resume.is_some() {
        argv.push("-c".into());
        argv.push(format!("sandbox_mode={}", dumps_ascii(get(request, "sandbox_mode").unwrap_or(&PyValue::None))));
    } else {
        argv.push("--sandbox".into());
        argv.push(text_of(get(request, "sandbox_mode")));
    }
    argv.push("--".into());
    if let Some(r) = resume {
        argv.push(r);
    }
    argv.push(text_of(get(request, "task")));
    argv
}

/// `_request_digest(request)`: sha256 of the sorted, tight, non-ASCII-preserving JSON.
pub fn request_digest(request: &PyValue) -> String {
    sha256_hex(dumps_sorted_tight_unicode(request).as_bytes())
}

/// The remote deadline owner (bash; formerly a `python3 -c` program). `$1` seconds, `$2` request digest, the rest is
/// the worker argv. The worker runs in its own session (`setsid` in place: a process-substitution child is not a group
/// leader) with stdin from /dev/null; every stdout line is re-emitted as a base64 `adapter.child_line` frame. At the
/// deadline the whole group gets SIGTERM, then SIGKILL after 3 s, and `adapter.deadline_reaped` is written only after
/// that (exit 124); otherwise `adapter.worker_exited` carries the worker's status. After the worker exits only the
/// bytes already available are drained, so an inherited pipe cannot hold the watchdog open.
pub const REMOTE_WATCHDOG: &str = r#"seconds=$1; digest=$2; shift 2
emit() { printf '%s\n' "$1"; }
child_line() { emit "{\"type\":\"adapter.child_line\",\"data\":\"$(printf '%s' "$1" | base64 -w0)\"}"; }
now_us() { local t=$EPOCHREALTIME; t=${t//[.,]/}; echo $((10#$t)); }
case $seconds in *.*) whole=${seconds%%.*}; frac=${seconds#*.}000000; frac=${frac:0:6} ;; *) whole=$seconds; frac=0 ;; esac
emit "{\"type\":\"adapter.framing_started\",\"request_digest\":\"$digest\"}"
exec 3< <(exec setsid "$@" </dev/null)
pid=$!
exited() { local st; [ -r "/proc/$pid/stat" ] || return 0; st=$(< "/proc/$pid/stat") || return 0; st=${st##*) }; [ "${st%% *}" = Z ]; }
deadline=$(( $(now_us) + ${whole:-0} * 1000000 + 10#$frac ))
buf=; timed_out=0; open=1
while :; do
  exited && break
  if (( $(now_us) >= deadline )); then
    exited && break
    kill -TERM -- "-$pid" 2>/dev/null && timed_out=1
    if (( timed_out )); then
      w=$(( $(now_us) + 3000000 ))
      while ! exited && (( $(now_us) < w )); do sleep 0.05; done
      exited || kill -KILL -- "-$pid" 2>/dev/null
      kill -KILL -- "-$pid" 2>/dev/null
    fi
    break
  fi
  if (( open )); then
    line=; IFS= read -r -t 0.1 -u 3 line; rc=$?
    if (( rc == 0 )); then child_line "$buf$line"; buf=
    elif (( rc > 128 )); then buf+=$line
    else buf+=$line; open=0; fi
  else
    sleep 0.1
  fi
done
if (( open )); then
  while read -t 0 -u 3; do
    line=; IFS= read -r -t 0.05 -u 3 line; rc=$?
    if (( rc == 0 )); then child_line "$buf$line"; buf=
    elif (( rc > 128 )); then buf+=$line; break
    else buf+=$line; break; fi
  done
fi
[ -n "$buf" ] && child_line "$buf"
exec 3<&-
if (( timed_out )); then
  wait "$pid" 2>/dev/null
  emit "{\"type\":\"adapter.deadline_reaped\",\"request_digest\":\"$digest\"}"
  exit 124
fi
wait "$pid"; rc=$?
emit "{\"type\":\"adapter.worker_exited\",\"request_digest\":\"$digest\",\"exit_code\":$rc}"
exit "$rc"
"#;

/// `build_argv(request)` with the native argv from `inner`.
pub fn build_argv_with(request: &PyValue, inner: &[String]) -> Vec<String> {
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let work_dir = text_of(get(&target, "work_dir"));
    if text_of(get(&target, "transport")) == "local" {
        return locked_argv(inner, &work_dir);
    }
    let marker = r#"{"type":"adapter.runner_missing"}"#;
    let mut tokens: Vec<String> = vec![
        "bash".into(),
        "-c".into(),
        REMOTE_WATCHDOG.into(),
        "a2a-watchdog".into(),
        text_of(get(request, "timeout_seconds")),
        request_digest(request),
    ];
    tokens.extend(locked_argv(inner, &work_dir));
    let script = format!(
        "cd {} && mkdir -p -- output && {{ command -v codex >/dev/null 2>&1 || {{ printf '%s\\n' {}; exit 127; }}; exec {}; }}",
        shlex_quote(&work_dir),
        shlex_quote(marker),
        tokens.iter().map(|t| shlex_quote(t)).collect::<Vec<_>>().join(" ")
    );
    let mut v: Vec<String> = vec!["ssh".into()];
    v.extend(SSH_HARDENING.iter().map(|x| x.to_string()));
    v.push("--".into());
    v.push(format!("{}@{}", text_of(get(&target, "ssh_user")), text_of(get(&target, "host"))));
    v.push(format!("bash -lc {}", shlex_quote(&script)));
    v
}

/// `build_argv(request)`.
pub fn build_argv(request: &PyValue) -> Vec<String> {
    build_argv_with(request, &inner_argv(request))
}

struct R {
    status: Option<&'static str>,
    exit_code: Option<i64>,
    reasons: Vec<&'static str>,
    output: Option<String>,
    session: Option<String>,
    execution_started: PyValue,
    raw_exit_code: PyValue,
    usage: PyValue,
    evidence: PyValue,
    events: Vec<PyValue>,
    budget_outcome: Option<&'static str>,
    model_used: Vec<String>,
}

fn base() -> R {
    R { status: None, exit_code: None, reasons: vec![], output: None, session: None, execution_started: PyValue::None,
        raw_exit_code: PyValue::None, usage: PyValue::None, evidence: PyValue::None, events: vec![], budget_outcome: None,
        model_used: vec![] }
}

fn result(request: &PyValue, r: R) -> PyValue {
    dict(vec![
        ("schema_version", int(2)),
        ("provider", s(PROVIDER_NAME)),
        ("intent", get(request, "intent").cloned().unwrap_or(PyValue::None)),
        ("status", s(r.status.unwrap_or("execution_failed"))),
        ("exit_code", int(r.exit_code.unwrap_or(4))),
        ("model_requested", get(request, "model").cloned().unwrap_or(PyValue::None)),
        ("model_used", PyValue::List(r.model_used.iter().map(|m| s(m)).collect())),
        ("reason_codes", PyValue::List(r.reasons.iter().map(|x| s(x)).collect())),
        ("output", r.output.map_or(PyValue::None, |o| s(&o))),
        ("session_id", r.session.map_or(PyValue::None, |x| s(&x))),
        ("num_turns", PyValue::None),
        ("budget_outcome", r.budget_outcome.map_or(PyValue::None, s)),
        ("duration_ms", PyValue::None),
        ("duration_api_ms", PyValue::None),
        ("raw_exit_code", r.raw_exit_code),
        ("execution_started", r.execution_started),
        ("usage", r.usage),
        ("failure_evidence", r.evidence),
        ("raw_events", PyValue::List(r.events)),
    ])
}

fn is_type(v: &PyValue, t: &str) -> bool {
    matches!(get(v, "type"), Some(PyValue::Str(x)) if x.equals(t))
}

fn type_text(v: &PyValue) -> String {
    get_text(v, "type").unwrap_or_default()
}

/// `parse_output(request, stdout, stderr, returncode, timed_out=, cancelled=, cancellation_signal=)`.
pub fn parse_output(request: &PyValue, stdout: &str, stderr: &str, rc: i32, timed_out: bool, cancelled: bool,
                    cancellation_signal: Option<i32>) -> PyValue {
    let digest = request_digest(request);
    let mut events: Vec<PyValue> = vec![];
    let mut malformed = false;
    let mut framed = false;
    let mut terminal_count = 0;
    let mut terminal_seen = false;
    let mut child_count = 0;
    let framing = dict(vec![("type", s("adapter.framing_started")), ("request_digest", s(&digest))]);
    for line in crate::py::splitlines(stdout) {
        if crate::py::strip(&line).is_empty() {
            continue;
        }
        let obj = match pyjson::loads(&line) {
            Ok(o) => o,
            Err(_) => {
                malformed = true;
                continue;
            }
        };
        if !matches!(obj.peel(), PyValue::Dict(_)) || !matches!(get(&obj, "type"), Some(PyValue::Str(_))) {
            malformed = true;
            continue;
        }
        if is_type(&obj, "adapter.framing_started") {
            if framed || !events.is_empty() || !py_eq(&obj, &framing) {
                malformed = true;
            } else {
                framed = true;
            }
            continue;
        }
        if framed && is_type(&obj, "adapter.child_line") {
            if terminal_seen {
                malformed = true;
            }
            let decoded: Option<PyValue> = (|| {
                let PyValue::Dict(d) = obj.peel() else { return None };
                if d.len() != 2 || !d.contains_key("type") || !d.contains_key("data") {
                    return None;
                }
                let PyValue::Str(data) = get(&obj, "data")? else { return None };
                let raw = b64decode_strict(data)?;
                let text = std::str::from_utf8(&raw).ok()?;
                let child = pyjson::loads(text).ok()?;
                if !matches!(child.peel(), PyValue::Dict(_)) || !matches!(get(&child, "type"), Some(PyValue::Str(_))) {
                    return None;
                }
                Some(child)
            })();
            match decoded {
                Some(child) => {
                    let lock = dict(vec![("type", s("adapter.lock_acquired"))]);
                    if type_text(&child).starts_with("adapter.") && !(child_count == 0 && py_eq(&child, &lock)) {
                        events.push(dict(vec![("type", s("adapter.quarantined_child")),
                                              ("data", get(&obj, "data").cloned().unwrap_or(PyValue::None))]));
                        malformed = true;
                    } else {
                        events.push(child);
                    }
                    child_count += 1;
                }
                None => {
                    events.push(dict(vec![("type", s("adapter.quarantined_child")),
                                          ("data", get(&obj, "data").cloned().unwrap_or(PyValue::None))]));
                    malformed = true;
                }
            }
            continue;
        }
        if framed {
            let kind = type_text(&obj);
            if kind != "adapter.deadline_reaped" && kind != "adapter.worker_exited" {
                malformed = true;
                continue;
            }
            terminal_count += 1;
            terminal_seen = true;
            if terminal_count != 1 {
                malformed = true;
            }
            if kind == "adapter.worker_exited" {
                let want = dict(vec![("type", s("adapter.worker_exited")), ("request_digest", s(&digest)),
                                     ("exit_code", int(rc.into()))]);
                if !py_eq(&obj, &want) {
                    malformed = true;
                }
            }
        }
        events.push(obj);
    }
    if framed && terminal_count != 1 {
        malformed = true;
    }
    let (mut session, mut usage, mut message): (Option<String>, PyValue, Option<String>) = (None, PyValue::None, None);
    let (mut started, mut completed, mut failed, mut denial, mut runner_missing) = (false, false, false, false, false);
    let mut lock_acquired = false;
    let (mut protocol_bad, mut model_used) = (false, Vec::<String>::new());
    let mut failure_messages: Vec<PyValue> = vec![];
    for event in &events {
        let kind = type_text(event);
        if kind == "thread.started" {
            match get(event, "thread_id") {
                Some(PyValue::Str(v)) if !v.strip().is_empty()
                    && session.as_ref().map_or(true, |x| *x == crate::py::text(v)) => {
                    session = Some(crate::py::text(v));
                }
                _ => protocol_bad = true,
            }
        }
        if kind == "turn.started" || kind.starts_with("item.") {
            started = true;
        }
        if kind == "turn.completed" {
            if completed || failed {
                protocol_bad = true;
            }
            completed = true;
            match get(event, "usage") {
                None | Some(PyValue::None) => {}
                Some(v @ PyValue::Dict(_)) => usage = v.clone(),
                Some(_) => protocol_bad = true,
            }
        }
        if kind == "turn.failed" || kind == "error" {
            failed = true;
            let m = if kind == "error" {
                get(event, "message").cloned().unwrap_or(PyValue::None)
            } else {
                match get(event, "error") {
                    Some(e @ PyValue::Dict(_)) => get(e, "message").cloned().unwrap_or(PyValue::None),
                    _ => PyValue::None,
                }
            };
            failure_messages.push(m);
        }
        if kind == "adapter.runner_missing" {
            runner_missing = true;
        }
        if kind == "adapter.lock_acquired" {
            lock_acquired = true;
        }
        if kind == "item.completed" {
            if let Some(item @ PyValue::Dict(_)) = get(event, "item") {
                if is_type(item, "agent_message") {
                    message = get_text(item, "text");
                }
            }
        }
        let mut codes = vec![get(event, "code").cloned().unwrap_or(PyValue::None)];
        if let Some(e @ PyValue::Dict(_)) = get(event, "error") {
            codes.push(get(e, "code").cloned().unwrap_or(PyValue::None));
        }
        let deny = [s("permission_denied"), s("approval_required"), s("sandbox_denied")];
        if codes.iter().any(|c| deny.iter().any(|d| py_eq(c, d))) {
            denial = true;
        }
        if kind == "thread.started" || kind == "turn.completed" {
            if let Some(PyValue::Str(o)) = get(event, "model") {
                let o = crate::py::text(o);
                if !crate::py::strip(&o).is_empty() && !model_used.contains(&o) {
                    model_used.push(o);
                }
            }
        }
    }
    let worker_busy = rc == LOCK_CONFLICT_EXIT && events.is_empty() && !malformed;
    let observed_started = if started {
        PyValue::Bool(true)
    } else if runner_missing || worker_busy {
        PyValue::Bool(false)
    } else {
        PyValue::None
    };
    let common = |r: R| R {
        session: session.clone(),
        execution_started: observed_started.clone(),
        raw_exit_code: int(rc.into()),
        usage: usage.clone(),
        events: events.clone(),
        model_used: model_used.clone(),
        ..r
    };
    let mut evidence = dict(vec![
        ("signal", s("unclassified")),
        ("rotate", PyValue::Bool(false)),
        ("raw_stdout", s(stdout)),
        ("raw_stderr", s(stderr)),
        ("returncode", int(rc.into())),
        ("lock_acquired", PyValue::Bool(lock_acquired)),
        ("malformed_jsonl", PyValue::Bool(malformed)),
    ]);
    let child_signal: Option<i32> = if rc < 0 {
        Some(-rc)
    } else if [130, 137, 143].contains(&rc) {
        Some(rc - 128)
    } else {
        None
    };
    if cancelled || child_signal.is_some() {
        let source = if rc < 0 {
            s("negative_returncode")
        } else if child_signal.is_some_and(|x| x != 0) {
            s("shell_exit_status")
        } else {
            PyValue::None
        };
        set(&mut evidence, "termination", dict(vec![
            ("adapter_received_signal", cancellation_signal.map_or(PyValue::None, |x| int(x.into()))),
            ("child_signal", child_signal.map_or(PyValue::None, |x| int(x.into()))),
            ("child_signal_source", source),
        ]));
    }
    let mut failure_reasons = vec!["NONZERO_EXIT"];
    if denial {
        failure_reasons.push("PERMISSION_DENIED");
    }
    let mut denial_note = if denial { Some("[permission_denials] Codex denied the requested operation".to_string()) } else { None };
    if let (Some(n), Some(m)) = (&mut denial_note, &message) {
        if !crate::py::strip(m).is_empty() {
            n.push_str("\n\n");
            n.push_str(m);
        }
    }
    let sig = |e: &mut PyValue, x: &str| set(e, "signal", s(x));
    if cancelled {
        let named = if child_signal.is_some_and(|x| x != 0) || cancellation_signal.is_some_and(|x| x != 0) {
            "process_signal"
        } else {
            "execution_interrupted"
        };
        sig(&mut evidence, named);
        return result(request, common(R { reasons: failure_reasons, output: denial_note, evidence,
                                          budget_outcome: Some("external_interruption"), ..base() }));
    }
    if worker_busy {
        sig(&mut evidence, "worker_busy");
        return result(request, common(R { reasons: vec!["NONZERO_EXIT"], evidence, ..base() }));
    }
    let transport = text_of(get(get(request, "target").unwrap_or(&PyValue::None), "transport"));
    if timed_out || rc == 124 {
        let reaped = dict(vec![("type", s("adapter.deadline_reaped")), ("request_digest", s(&digest))]);
        let markers: Vec<&PyValue> = events.iter().filter(|e| is_type(e, "adapter.deadline_reaped")).collect();
        let remote_controlled = framed && !timed_out && rc == 124 && transport == "ssh" && markers.len() == 1
            && py_eq(events.last().unwrap(), markers[0]) && py_eq(markers[0], &reaped);
        let common_controlled = lock_acquired && !malformed && !protocol_bad && !failed && !completed && !denial
            && session.as_ref().is_some_and(|x| !x.is_empty()) && started;
        let local_controlled = timed_out && transport == "local";
        let named = if common_controlled && (remote_controlled || local_controlled) {
            if remote_controlled { "controlled_remote_deadline" } else { "controlled_local_deadline" }
        } else {
            "wall_timeout"
        };
        sig(&mut evidence, named);
        let mut reasons = vec!["TIMEOUT"];
        if denial {
            reasons.push("PERMISSION_DENIED");
        }
        return result(request, common(R { status: Some("timeout"), exit_code: Some(124), reasons, output: denial_note,
                                          evidence, budget_outcome: Some("external_interruption"), ..base() }));
    }
    if child_signal.is_some() {
        sig(&mut evidence, "process_signal");
        return result(request, common(R { reasons: failure_reasons, output: denial_note, evidence,
                                          budget_outcome: Some("external_interruption"), ..base() }));
    }
    if runner_missing && !started && !malformed && events.len() == 1 && rc == 127 {
        sig(&mut evidence, "missing_binary");
        set(&mut evidence, "rotate", PyValue::Bool(true));
        set(&mut evidence, "detail", s("Codex runner is not installed"));
        let out = format!("[runner_unavailable] {}", dumps_unicode(&evidence));
        return result(request, common(R { reasons: vec!["RUNNER_UNAVAILABLE"], output: Some(out), evidence, ..base() }));
    }
    if denial {
        sig(&mut evidence, "permission_denied");
        return result(request, common(R { reasons: vec!["PERMISSION_DENIED"], output: denial_note, evidence, ..base() }));
    }
    if failed {
        let cap = "Selected model is at capacity. Please try a different model.";
        let capacity = !malformed && !protocol_bad && !completed && rc == 1 && !failure_messages.is_empty()
            && failure_messages.iter().all(|m| matches!(m, PyValue::Str(t) if t.equals(cap)));
        sig(&mut evidence, if capacity { "provider_capacity" } else { "native_failure" });
        return result(request, common(R { reasons: vec!["IS_ERROR"], evidence, ..base() }));
    }
    if malformed || protocol_bad {
        sig(&mut evidence, if malformed { "malformed_jsonl" } else { "invalid_event_sequence" });
        return result(request, common(R { status: Some("malformed_output"), exit_code: Some(5),
                                          reasons: vec![if malformed { "MALFORMED_JSON" } else { "PROVIDER_RESULT_INVALID" }],
                                          evidence, ..base() }));
    }
    if rc != 0 {
        sig(&mut evidence, if transport == "ssh" && rc == 255 { "ssh_transport" } else { "process_failure" });
        return result(request, common(R { reasons: vec!["NONZERO_EXIT"], evidence, ..base() }));
    }
    let empty_msg = message.as_ref().map_or(true, |m| crate::py::strip(m).is_empty());
    if session.is_none() || !started || !completed || empty_msg {
        sig(&mut evidence, "incomplete_event_stream_or_empty_message");
        return result(request, common(R { status: Some("malformed_output"), exit_code: Some(5),
                                          reasons: vec!["PROVIDER_RESULT_INVALID"], evidence, ..base() }));
    }
    if let Some(r) = get(request, "resume_session_id").filter(|r| r.is_truthy()) {
        if session.as_deref() != Some(str_of(r).as_str()) {
            sig(&mut evidence, "resume_not_honored");
            return result(request, common(R { status: Some("malformed_output"), exit_code: Some(5),
                                              reasons: vec!["PROVIDER_RESULT_INVALID"], evidence, ..base() }));
        }
    }
    result(request, common(R { status: Some("completed"), exit_code: Some(0), output: message,
                               budget_outcome: Some("within_budget"), ..base() }))
}

extern "C" {
    fn killpg(pgrp: i32, sig: i32) -> i32;
    fn signal(signum: i32, handler: usize) -> usize;
}
const SIGINT: i32 = 2;
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

static CANCEL: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_signal(signum: i32) {
    CANCEL.store(signum, Ordering::SeqCst);
}

fn signal_group(pid: u32, sig: i32) {
    unsafe {
        killpg(pid as i32, sig);
    }
}

/// A started worker: the child plus readers that keep its two outputs.
pub struct Worker {
    pub child: std::process::Child,
    out: Arc<Mutex<Vec<u8>>>,
    err: Arc<Mutex<Vec<u8>>>,
    readers: Vec<std::thread::JoinHandle<()>>,
}

fn pump<R: Read + Send + 'static>(mut r: R, sink: Arc<Mutex<Vec<u8>>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0u8; 65536];
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => sink.lock().unwrap().extend_from_slice(&buf[..n]),
            }
        }
    })
}

impl Worker {
    /// `subprocess.Popen(argv, cwd=cwd, stdin=DEVNULL, stdout=PIPE, stderr=PIPE, start_new_session=True)`.
    pub fn start(argv: &[String], cwd: Option<&str>) -> Result<Worker, OsErr> {
        let mut child = spawn(argv, cwd, true)?;
        let out = Arc::new(Mutex::new(vec![]));
        let err = Arc::new(Mutex::new(vec![]));
        let readers = vec![pump(child.stdout.take().unwrap(), out.clone()), pump(child.stderr.take().unwrap(), err.clone())];
        Ok(Worker { child, out, err, readers })
    }

    fn finished(&mut self) -> Option<i32> {
        if self.readers.iter().all(|h| h.is_finished()) {
            if let Ok(Some(st)) = self.child.try_wait() {
                return Some(returncode(st));
            }
        }
        None
    }

    /// `communicate(timeout)`: `Ok(rc)` once both outputs are closed and the child exited; `Err(None)` on timeout,
    /// `Err(Some(sig))` when a cancellation signal arrived.
    fn communicate(&mut self, timeout: Option<f64>, watch_cancel: bool) -> Result<i32, Option<i32>> {
        let start = Instant::now();
        loop {
            if let Some(rc) = self.finished() {
                return Ok(rc);
            }
            if watch_cancel {
                let c = CANCEL.swap(0, Ordering::SeqCst);
                if c != 0 {
                    return Err(Some(c));
                }
            }
            if timeout.is_some_and(|t| start.elapsed().as_secs_f64() >= t) {
                return Err(None);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// `_stop_process(process)`: TERM the group, wait `grace`, KILL the group, KILL again in any case.
    pub fn stop(&mut self, grace: f64) -> i32 {
        let pid = self.child.id();
        signal_group(pid, SIGTERM);
        let rc = match self.communicate(Some(grace), false) {
            Ok(rc) => rc,
            Err(_) => {
                signal_group(pid, SIGKILL);
                self.communicate(None, false).unwrap_or(-SIGKILL)
            }
        };
        signal_group(pid, SIGKILL);
        rc
    }

    fn take(self) -> (Vec<u8>, Vec<u8>) {
        for h in self.readers {
            let _ = h.join();
        }
        let o = std::mem::take(&mut *self.out.lock().unwrap());
        let e = std::mem::take(&mut *self.err.lock().unwrap());
        (o, e)
    }
}

/// Knobs the self-test replaces (`_inner_argv`, `KILL_GRACE_SECONDS` in the original).
pub struct Config<'a> {
    pub inner: Option<&'a dyn Fn(&PyValue) -> Vec<String>>,
    pub grace: f64,
}

impl Default for Config<'_> {
    fn default() -> Self {
        Config { inner: None, grace: KILL_GRACE_SECONDS }
    }
}

/// `invoke(request)`.
pub fn invoke(request: &PyValue) -> PyValue {
    invoke_with(request, &Config::default())
}

fn launch_failure(request: &PyValue, e: &OsErr, local: bool, cwd: Option<&str>) -> PyValue {
    let missing = e.class == "FileNotFoundError" && e.filename == "codex" && local && Some(e.filename.as_str()) != cwd;
    let evidence = dict(vec![
        ("signal", s(if missing { "missing_binary" } else { "launch_failure" })),
        ("rotate", PyValue::Bool(missing)),
        ("detail", s(&e.text())),
        ("filename", s(&e.filename)),
    ]);
    result(request, R {
        reasons: vec![if missing { "RUNNER_UNAVAILABLE" } else { "NONZERO_EXIT" }],
        output: missing.then(|| format!("[runner_unavailable] {}", dumps_ascii(&evidence))),
        execution_started: PyValue::Bool(false),
        evidence,
        ..base()
    })
}

/// `invoke(request)` with replaceable knobs.
pub fn invoke_with(request: &PyValue, cfg: &Config) -> PyValue {
    let inner = match cfg.inner {
        Some(f) => f(request),
        None => inner_argv(request),
    };
    let argv = build_argv_with(request, &inner);
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let local = text_of(get(&target, "transport")) == "local";
    let cwd = if local { Some(text_of(get(&target, "work_dir"))) } else { None };
    let secs = crate::agent_control::pyhelp::as_int(get(request, "timeout_seconds")).unwrap_or(0) as f64;
    let timeout = secs + if local { 0.0 } else { 8.0 + cfg.grace };
    if let Some(c) = &cwd {
        let out_dir = if c.ends_with('/') { format!("{c}output") } else { format!("{c}/output") };
        if let Err(e) = std::fs::create_dir(&out_dir) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return launch_failure(request, &OsErr::new(e.raw_os_error().unwrap_or(2), &out_dir), local, cwd.as_deref());
            }
        }
        let exe = &inner[0];
        if crate::agent_control::process::resolve(exe, None).is_err() {
            let e = OsErr { class: "FileNotFoundError", errno: 2, strerror: "runner executable not found".into(),
                            filename: exe.clone() };
            return launch_failure(request, &e, local, cwd.as_deref());
        }
    }
    let mut worker = match Worker::start(&argv, cwd.as_deref()) {
        Ok(w) => w,
        Err(e) => return launch_failure(request, &e, local, cwd.as_deref()),
    };
    CANCEL.store(0, Ordering::SeqCst);
    let previous = unsafe {
        [(SIGTERM, signal(SIGTERM, on_signal as *const () as usize)), (SIGINT, signal(SIGINT, on_signal as *const () as usize))]
    };
    let restore = || unsafe {
        for (n, p) in previous {
            signal(n, p);
        }
    };
    let (mut timed_out, mut cancelled, mut cancellation_signal) = (false, false, None);
    let rc = match worker.communicate(Some(timeout), true) {
        Ok(rc) => rc,
        Err(None) => {
            timed_out = true;
            worker.stop(cfg.grace)
        }
        Err(Some(sig)) => {
            cancelled = true;
            cancellation_signal = Some(sig);
            restore();
            worker.stop(cfg.grace)
        }
    };
    restore();
    let (stdout, stderr) = worker.take();
    let (text, diagnostics, invalid) = match (std::str::from_utf8(&stdout), std::str::from_utf8(&stderr)) {
        (Ok(a), Ok(b)) => (a.to_string(), b.to_string(), false),
        _ => (String::from_utf8_lossy(&stdout).into_owned(), String::from_utf8_lossy(&stderr).into_owned(), true),
    };
    let mut res = parse_output(request, &text, &diagnostics, rc, timed_out, cancelled, cancellation_signal);
    if invalid {
        let mut evidence = match get(&res, "failure_evidence") {
            Some(e @ PyValue::Dict(_)) => e.clone(),
            _ => dict(vec![("signal", s("invalid_utf8")), ("rotate", PyValue::Bool(false))]),
        };
        set(&mut evidence, "raw_stdout_base64", s(&b64encode(&stdout)));
        set(&mut evidence, "raw_stderr_base64", s(&b64encode(&stderr)));
        set(&mut res, "failure_evidence", evidence);
        if !timed_out && !cancelled {
            set(&mut res, "status", s("malformed_output"));
            set(&mut res, "exit_code", int(5));
            set(&mut res, "reason_codes", PyValue::List(vec![s("MALFORMED_JSON")]));
            set(&mut res, "output", PyValue::None);
            set(&mut res, "budget_outcome", PyValue::None);
        }
    }
    let _ = (dumps_ascii_tight, PyStr::default);
    res
}
