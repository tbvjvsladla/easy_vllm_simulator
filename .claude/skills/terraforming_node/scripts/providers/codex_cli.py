#!/usr/bin/env python3
"""Codex CLI v2 adapter: JSONL, explicit permissions, native-ID resume and wall deadline.

CLI grammar is isolated here. Model identifiers are caller declarations; JSONL does not
necessarily report the actual model, so absent observations stay empty. Operation
capabilities are intent, not a per-tool allowlist. The selected sandbox/built-in profile,
existing rules and managed policy enforce permissions. Permission profiles require the
node's deployment/trust/native-work canary; this adapter never bypasses policy or invents
effective-policy observations.

Seed reviewed: seed/hermes-subagent-control-plane (codex-cli-control). Reuses explicit
workspace, wall-time budget, primary provenance and empty/error rejection. Deliberately
does not reuse its text fallback: malformed JSONL must not become successful prose.
"""
from __future__ import annotations

import base64
import hashlib
import json
import os
import shlex
import shutil
import signal
import subprocess

PROVIDER_NAME = "codex_cli"
BACKEND_DEFAULT_MODEL = {"openai": "gpt-6.1-sol"}
RUNNER_ALIASES = {"codex-gpt-6.1-sol": ("openai", BACKEND_DEFAULT_MODEL["openai"]),
                  "codex-gpt-6.1-terra": ("openai", "gpt-6.1-terra")}
SUPPORTS = {
    "schema_versions": [2], "budget_units": ["wall_time"], "native_turn_limit": False,
    "resume": "explicit_native_id_with_binding", "structured_output": "jsonl",
    "capabilities_mode": "intent_only",
    "sandbox_modes": ["read-only", "workspace-write"],
    "permission_profiles": [":danger-full-access"],
    "permission_profile_verification": "deployment_and_native_work_canary_required",
    "cancellation": "local_process_group_and_remote_deadline",
    "workspace_exclusion": "nonblocking_flock_in_output",
}
SSH_HARDENING = (
    "-F", "/dev/null", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8",
    "-o", "StrictHostKeyChecking=yes", "-o", "IdentitiesOnly=yes",
    "-o", "ForwardAgent=no", "-o", "ClearAllForwardings=yes", "-o", "RequestTTY=no",
)
KILL_GRACE_SECONDS = 3
LOCK_CONFLICT_EXIT = 75


def _locked_argv(inner: list[str], work_dir: str) -> list[str]:
    # Keep the lock FD inherited by the worker. If the caller disappears, the
    # existing worker retains exclusion until it exits; there is no unlocked retry.
    marker = json.dumps({"type": "adapter.lock_acquired"}, separators=(",", ":"))
    script = "printf '%s\\n' " + shlex.quote(marker) + '; exec "$@"'
    return ["flock", "--nonblock", "--conflict-exit-code", str(LOCK_CONFLICT_EXIT),
            os.path.join(work_dir, "output", ".a2a-worker.lock"),
            "bash", "-c", script, "a2a-worker", *inner]


def _inner_argv(request: dict) -> list[str]:
    target = request["target"]
    argv = ["codex", "exec", "--cd", target["work_dir"]]
    resume = request.get("resume_session_id")
    if resume:
        argv.append("resume")
    argv += ["--json", "--model", request["model"], "-c", 'approval_policy="never"']
    if request.get("reasoning_effort") not in (None, "default"):
        argv += ["-c", "model_reasoning_effort=" + json.dumps(request["reasoning_effort"])]
    if "permission_profile" in request:
        argv += ["-c", "default_permissions=" + json.dumps(request["permission_profile"])]
    elif resume:
        # exec resume has no --sandbox option in CLI 0.164.0; its config override is explicit.
        argv += ["-c", "sandbox_mode=" + json.dumps(request["sandbox_mode"])]
    else:
        argv += ["--sandbox", request["sandbox_mode"]]
    argv += ["--"]
    if resume:
        argv.append(resume)
    argv.append(request["task"])
    return argv


def _request_digest(request: dict) -> str:
    return hashlib.sha256(json.dumps(request, sort_keys=True, ensure_ascii=False,
                                     separators=(",", ":")).encode("utf-8")).hexdigest()


# The marker is written by the remote watchdog only after its owned process
# group has been signalled and the direct child reaped. Native argv stays here.
_REMOTE_WATCHDOG = r"""import base64,json,os,selectors,signal,subprocess,sys,time
seconds=float(sys.argv[1]); digest=sys.argv[2]
p=subprocess.Popen(sys.argv[3:], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, start_new_session=True)
def emit(obj): print(json.dumps(obj,separators=(',',':')),flush=True)
def child_line(data): emit({'type':'adapter.child_line','data':base64.b64encode(data).decode('ascii')})
emit({'type':'adapter.framing_started','request_digest':digest})
sel=selectors.DefaultSelector(); sel.register(p.stdout,selectors.EVENT_READ)
buf=b''; deadline=time.monotonic()+seconds; timed_out=False
while True:
    rc=p.poll()
    if rc is not None: break
    remaining=deadline-time.monotonic()
    if remaining <= 0:
        if p.poll() is not None: break
        try: os.killpg(p.pid, signal.SIGTERM); timed_out=True
        except ProcessLookupError: pass
        if timed_out:
            try: p.wait(timeout=3)
            except subprocess.TimeoutExpired:
                try: os.killpg(p.pid, signal.SIGKILL)
                except ProcessLookupError: pass
                p.wait()
            try: os.killpg(p.pid, signal.SIGKILL)
            except ProcessLookupError: pass
        break
    for key,_ in sel.select(min(remaining,0.1)):
        data=os.read(key.fd,65536)
        if not data:
            sel.unregister(p.stdout)
            continue
        buf+=data
        while b'\n' in buf:
            line,buf=buf.split(b'\n',1); child_line(line)
# Drain only currently available bytes: an inherited pipe FD must not hold the
# watchdog open or turn a child exit into a fabricated timeout.
remaining_drain=1048576
while remaining_drain and sel.get_map() and sel.select(0):
    data=os.read(p.stdout.fileno(),min(65536,remaining_drain))
    if not data: break
    remaining_drain-=len(data); buf+=data
    while b'\n' in buf:
        line,buf=buf.split(b'\n',1); child_line(line)
incomplete=not remaining_drain and bool(sel.get_map()) and bool(sel.select(0))
if buf: child_line(buf)
sel.close(); p.stdout.close()
if incomplete: emit({'type':'adapter.output_incomplete'})
if timed_out:
    emit({'type':'adapter.deadline_reaped','request_digest':digest})
    sys.exit(124)
rc=p.wait(); rc=128-rc if rc<0 else rc
emit({'type':'adapter.worker_exited','request_digest':digest,'exit_code':rc})
sys.exit(rc)
"""


def build_argv(request: dict) -> list[str]:
    inner = _inner_argv(request)
    target = request["target"]
    if target["transport"] == "local":
        # The adapter watchdog is the sole local deadline owner; a natural child
        # exit 124 remains ambiguous instead of racing a nested timeout wrapper.
        return _locked_argv(inner, target["work_dir"])
    # Host/user are one ssh operand, and all remote grammar is emitted from quoted tokens.
    # The marker distinguishes a missing runner from timeout/ssh/cd launch failures.
    marker = json.dumps({"type": "adapter.runner_missing"}, separators=(",", ":"))
    script = (
        "cd " + shlex.quote(target["work_dir"]) + " && mkdir -p -- output && "
        "{ command -v codex >/dev/null 2>&1 || "
        "{ printf '%s\\n' " + shlex.quote(marker) + "; exit 127; }; "
        "exec " + " ".join(shlex.quote(token) for token in
            ["python3", "-c", _REMOTE_WATCHDOG, str(request["timeout_seconds"]),
             _request_digest(request), *_locked_argv(inner, target["work_dir"])]) + "; }"
    )
    return ["ssh", *SSH_HARDENING, "--", target["ssh_user"] + "@" + target["host"],
            "bash -lc " + shlex.quote(script)]


def _result(request: dict, *, status="execution_failed", exit_code=4,
            reasons=None, output=None, session=None, execution_started=None,
            raw_exit_code=None, usage=None, evidence=None, events=None,
            budget_outcome=None, model_used=None) -> dict:
    return {
        "schema_version": 2, "provider": PROVIDER_NAME, "intent": request["intent"],
        "status": status, "exit_code": exit_code, "model_requested": request["model"],
        "model_used": model_used or [], "reason_codes": reasons or [], "output": output,
        "session_id": session, "num_turns": None, "budget_outcome": budget_outcome,
        "duration_ms": None, "duration_api_ms": None, "raw_exit_code": raw_exit_code,
        "execution_started": execution_started, "usage": usage,
        "failure_evidence": evidence, "raw_events": events or [],
    }


def parse_output(request: dict, stdout: str, stderr: str, returncode: int, *,
                 timed_out=False, cancelled=False, cancellation_signal=None) -> dict:
    """Parse observed events even after nonzero exit, preserving partial session/evidence.

    A completed CLI turn is control-plane completion, not project task completion. The
    relay still validates the final task report and existing artifact/completion gates.
    Unknown/API/permission failures never automatically replay work. A runner-missing
    marker is the sole remote rotation signal and must precede all native execution.
    """
    events, malformed = [], False
    framed = False
    terminal_count = 0
    terminal_seen = False
    child_count = 0
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except (ValueError, RecursionError):
            malformed = True
            continue
        if not isinstance(obj, dict) or not isinstance(obj.get("type"), str):
            malformed = True
            continue
        if obj.get("type") == "adapter.framing_started":
            if framed or events or obj != {"type": "adapter.framing_started", "request_digest": _request_digest(request)}:
                malformed = True
            else:
                framed = True
            continue
        if framed and obj.get("type") == "adapter.child_line":
            if terminal_seen:
                malformed = True
            try:
                if set(obj) != {"type", "data"} or not isinstance(obj["data"], str):
                    raise ValueError("invalid child frame")
                raw = base64.b64decode(obj["data"], validate=True)
                child = json.loads(raw.decode("utf-8"))
                if not isinstance(child, dict) or not isinstance(child.get("type"), str):
                    raise ValueError("invalid native event")
                # The first line is emitted by our flock wrapper before exec.
                # All other adapter.* records are untrusted child output.
                if child["type"].startswith("adapter.") and not (
                        child_count == 0 and child == {"type": "adapter.lock_acquired"}):
                    events.append({"type": "adapter.quarantined_child", "data": obj["data"]})
                    malformed = True
                else:
                    events.append(child)
                child_count += 1
            except (ValueError, UnicodeError, RecursionError):
                events.append({"type": "adapter.quarantined_child", "data": obj.get("data")})
                malformed = True
            continue
        if framed:
            kind = obj.get("type")
            if kind not in ("adapter.deadline_reaped", "adapter.worker_exited"):
                malformed = True
                continue
            terminal_count += 1
            terminal_seen = True
            if terminal_count != 1:
                malformed = True
            if kind == "adapter.worker_exited" and obj != {
                    "type": "adapter.worker_exited", "request_digest": _request_digest(request),
                    "exit_code": returncode}:
                malformed = True
        events.append(obj)
    if framed and terminal_count != 1:
        malformed = True
    session, usage, message = None, None, None
    started, completed, failed, denial, runner_missing = False, False, False, False, False
    lock_acquired = False
    protocol_bad, model_used = False, []
    failure_messages = []
    for event in events:
        kind = event["type"]
        if kind == "thread.started":
            value = event.get("thread_id")
            if not isinstance(value, str) or not value.strip() or (session and value != session):
                protocol_bad = True
            else:
                session = value
        if kind == "turn.started" or kind.startswith("item."):
            started = True
        if kind == "turn.completed":
            if completed or failed:
                protocol_bad = True
            completed = True
            value = event.get("usage")
            if value is not None:
                if isinstance(value, dict):
                    usage = value
                else:
                    protocol_bad = True
        if kind in ("turn.failed", "error"):
            failed = True
            failure_messages.append(event.get("message") if kind == "error" else
                                    (event.get("error") or {}).get("message")
                                    if isinstance(event.get("error"), dict) else None)
        if kind == "adapter.runner_missing":
            runner_missing = True
        if kind == "adapter.lock_acquired":
            lock_acquired = True
        item = event.get("item")
        if kind == "item.completed" and isinstance(item, dict) and item.get("type") == "agent_message":
            # Only the native agent-message item is the final response; command stdout is not.
            value = item.get("text")
            message = value if isinstance(value, str) else None
        # Do not infer permissions or actual model from free-form messages or the request.
        error = event.get("error")
        codes = [event.get("code")]
        if isinstance(error, dict):
            codes.append(error.get("code"))
        if any(code in ("permission_denied", "approval_required", "sandbox_denied") for code in codes):
            denial = True
        # The current event stream may omit model metadata entirely. Record only an
        # explicitly supplied native observed-model field, never task text/command output.
        if kind in ("thread.started", "turn.completed"):
            observed = event.get("model")
            if isinstance(observed, str) and observed.strip() and observed not in model_used:
                model_used.append(observed)
    worker_busy = returncode == LOCK_CONFLICT_EXIT and not events and not malformed
    observed_started = True if started else (False if runner_missing or worker_busy else None)
    common = dict(session=session, execution_started=observed_started, raw_exit_code=returncode,
                  usage=usage, events=events, model_used=model_used)
    evidence = {"signal": "unclassified", "rotate": False,
                "raw_stdout": stdout, "raw_stderr": stderr, "returncode": returncode,
                "lock_acquired": lock_acquired, "malformed_jsonl": malformed}
    # A negative subprocess rc observes a signal directly. Shell-style 128+signal
    # exits are conventions, so retain that weaker provenance rather than claiming
    # who sent it. In particular SIGTERM does not establish a user's intent.
    child_signal = (-returncode if returncode < 0 else
                    returncode - 128 if returncode in (130, 137, 143) else None)
    if cancelled or child_signal is not None:
        evidence["termination"] = {
            "adapter_received_signal": cancellation_signal,
            "child_signal": child_signal,
            "child_signal_source": ("negative_returncode" if returncode < 0 else
                                    "shell_exit_status" if child_signal else None),
        }
    failure_reasons = ["NONZERO_EXIT"]
    if denial:
        failure_reasons.append("PERMISSION_DENIED")
    denial_note = "[permission_denials] Codex denied the requested operation" if denial else None
    if denial_note and isinstance(message, str) and message.strip():
        denial_note += "\n\n" + message
    # Receiving an adapter signal is a separate observation, even when the child
    # also exits124. A TERM sent only by our deadline cleanup is not cancellation.
    if cancelled:
        evidence["signal"] = "process_signal" if child_signal or cancellation_signal else "execution_interrupted"
        return _result(request, reasons=failure_reasons, output=denial_note, evidence=evidence,
                       budget_outcome="external_interruption", **common)
    if worker_busy:
        evidence["signal"] = "worker_busy"
        return _result(request, reasons=["NONZERO_EXIT"], evidence=evidence, **common)
    if timed_out or returncode == 124:
        # Only the local adapter's own deadline and process-group cleanup prove
        # this was our controlled deadline; an SSH watchdog or exit 124 is ambiguous.
        remote_markers = [event for event in events if event.get("type") == "adapter.deadline_reaped"]
        remote_controlled = (framed and not timed_out and returncode == 124 and request["target"]["transport"] == "ssh"
                             and len(remote_markers) == 1 and events[-1] == remote_markers[0]
                             and remote_markers[0] == {"type": "adapter.deadline_reaped",
                                                       "request_digest": _request_digest(request)})
        common_controlled = (lock_acquired and not malformed and not protocol_bad and not failed
                             and not completed and not denial and bool(session) and started)
        local_controlled = timed_out and request["target"]["transport"] == "local"
        evidence["signal"] = (("controlled_remote_deadline" if remote_controlled else "controlled_local_deadline")
                              if common_controlled and (remote_controlled or local_controlled) else "wall_timeout")
        return _result(request, status="timeout", exit_code=124,
                       reasons=["TIMEOUT"] + (["PERMISSION_DENIED"] if denial else []),
                       output=denial_note,
                       evidence=evidence, budget_outcome="external_interruption", **common)
    if child_signal is not None:
        evidence["signal"] = "process_signal"
        return _result(request, reasons=failure_reasons, output=denial_note, evidence=evidence,
                       budget_outcome="external_interruption", **common)
    if runner_missing and not started and not malformed and len(events) == 1 and returncode == 127:
        evidence.update(signal="missing_binary", rotate=True, detail="Codex runner is not installed")
        return _result(request, reasons=["RUNNER_UNAVAILABLE"],
                       output="[runner_unavailable] " + json.dumps(evidence, ensure_ascii=False),
                       evidence=evidence, **common)
    if denial:
        evidence["signal"] = "permission_denied"
        return _result(request, reasons=["PERMISSION_DENIED"],
                       output=denial_note,
                       evidence=evidence, **common)
    if failed:
        capacity = (not malformed and not protocol_bad and not completed and returncode == 1
                    and failure_messages and all(message == "Selected model is at capacity. Please try a different model."
                                                 for message in failure_messages))
        evidence["signal"] = "provider_capacity" if capacity else "native_failure"
        return _result(request, reasons=["IS_ERROR"], evidence=evidence, **common)
    if malformed or protocol_bad:
        evidence["signal"] = "malformed_jsonl" if malformed else "invalid_event_sequence"
        return _result(request, status="malformed_output", exit_code=5,
                       reasons=["MALFORMED_JSON" if malformed else "PROVIDER_RESULT_INVALID"],
                       evidence=evidence, **common)
    if returncode != 0:
        evidence["signal"] = "ssh_transport" if request["target"]["transport"] == "ssh" and returncode == 255 else "process_failure"
        return _result(request, reasons=["NONZERO_EXIT"], evidence=evidence, **common)
    if not session or not started or not completed or not isinstance(message, str) or not message.strip():
        evidence["signal"] = "incomplete_event_stream_or_empty_message"
        return _result(request, status="malformed_output", exit_code=5,
                       reasons=["PROVIDER_RESULT_INVALID"], evidence=evidence, **common)
    if request.get("resume_session_id") and session != request["resume_session_id"]:
        evidence["signal"] = "resume_not_honored"
        return _result(request, status="malformed_output", exit_code=5,
                       reasons=["PROVIDER_RESULT_INVALID"], evidence=evidence, **common)
    return _result(request, status="completed", exit_code=0, output=message,
                   budget_outcome="within_budget", **common)


class _Cancelled(Exception):
    def __init__(self, signum):
        self.signum = signum


def _signal_group(pid: int, signum: int) -> None:
    try:
        os.killpg(pid, signum)
    except ProcessLookupError:
        pass


def _stop_process(process) -> tuple[bytes, bytes]:
    _signal_group(process.pid, signal.SIGTERM)
    try:
        stdout, stderr = process.communicate(timeout=KILL_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        _signal_group(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
    finally:
        # The leader may have exited while an ignoring child remains in its group.
        _signal_group(process.pid, signal.SIGKILL)
    return stdout, stderr


def invoke(request: dict) -> dict:
    argv = build_argv(request)
    target = request["target"]
    cwd = target["work_dir"] if target["transport"] == "local" else None
    # SSH gets enough time to deliver the remote deadline's terminal status; the
    # remote deadline itself bounds the Codex process and its ordinary descendants.
    timeout = request["timeout_seconds"] + (8 + KILL_GRACE_SECONDS if target["transport"] == "ssh" else 0)
    try:
        if cwd is not None:
            # One owned generated directory only: never create a missing workspace.
            try:
                os.mkdir(os.path.join(cwd, "output"))
            except FileExistsError:
                pass
            executable = _inner_argv(request)[0]
            if shutil.which(executable) is None:
                raise FileNotFoundError(2, "runner executable not found", executable)
        process = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
    except OSError as exc:
        missing_runner = isinstance(exc, FileNotFoundError) and exc.filename == "codex" and target["transport"] == "local" and exc.filename != cwd
        evidence = {"signal": "missing_binary" if missing_runner else "launch_failure",
                    "rotate": missing_runner, "detail": str(exc), "filename": exc.filename}
        return _result(request, reasons=["RUNNER_UNAVAILABLE" if missing_runner else "NONZERO_EXIT"],
                       output=("[runner_unavailable] " + json.dumps(evidence)) if missing_runner else None,
                       execution_started=False, evidence=evidence)
    previous_handlers = {}
    def cancel(signum, _frame):
        raise _Cancelled(signum)
    try:
        for signum in (signal.SIGTERM, signal.SIGINT):
            try:
                previous_handlers[signum] = signal.signal(signum, cancel)
            except ValueError:  # Non-main-thread callers cannot install process handlers.
                pass
        timed_out, cancelled, cancellation_signal = False, False, None
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            stdout, stderr = _stop_process(process)
        except _Cancelled as exc:
            cancelled = True
            cancellation_signal = exc.signum
            # Further signals use their original disposition during bounded cleanup.
            for signum, previous in previous_handlers.items():
                signal.signal(signum, previous)
            stdout, stderr = _stop_process(process)
    finally:
        for signum, previous in previous_handlers.items():
            signal.signal(signum, previous)
    invalid_utf8 = False
    try:
        text = stdout.decode("utf-8")
        diagnostics = stderr.decode("utf-8")
    except UnicodeDecodeError:
        invalid_utf8 = True
        text, diagnostics = stdout.decode("utf-8", errors="replace"), stderr.decode("utf-8", errors="replace")
    result = parse_output(request, text, diagnostics, process.returncode,
                          timed_out=timed_out, cancelled=cancelled,
                          cancellation_signal=cancellation_signal)
    if invalid_utf8:
        evidence = result["failure_evidence"] or {"signal": "invalid_utf8", "rotate": False}
        evidence.update(raw_stdout_base64=base64.b64encode(stdout).decode("ascii"),
                        raw_stderr_base64=base64.b64encode(stderr).decode("ascii"))
        result["failure_evidence"] = evidence
        if not timed_out and not cancelled:
            result.update(status="malformed_output", exit_code=5,
                          reason_codes=["MALFORMED_JSON"], output=None, budget_outcome=None)
    return result
