#!/usr/bin/env python3
"""Codex CLI v2 adapter: JSONL, explicit permissions, native-ID resume and wall deadline.

CLI grammar is isolated here. Model identifiers are caller declarations; JSONL does not
necessarily report the actual model, so absent observations stay empty. Operation
capabilities are intent, not a per-tool allowlist. The selected sandbox/named profile,
existing rules and managed policy enforce permissions. Named profiles require the
node's deployment/trust/denial canary; this adapter never bypasses policy or invents
effective-policy observations.

Seed reviewed: seed/hermes-subagent-control-plane (codex-cli-control). Reuses explicit
workspace, wall-time budget, primary provenance and empty/error rejection. Deliberately
does not reuse its text fallback: malformed JSONL must not become successful prose.
"""
from __future__ import annotations

import base64
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
    "permission_profiles": ["vllm-main", "vllm-sub"],
    "permission_profile_verification": "deployment_and_denial_canary_required",
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
    if "reasoning_effort" in request:
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


def build_argv(request: dict) -> list[str]:
    inner = _inner_argv(request)
    target = request["target"]
    if target["transport"] == "local":
        return ["timeout", "--signal=TERM", "--kill-after=" + str(KILL_GRACE_SECONDS) + "s",
                str(request["timeout_seconds"]) + "s", *_locked_argv(inner, target["work_dir"])]
    # Host/user are one ssh operand, and all remote grammar is emitted from quoted tokens.
    # The marker distinguishes a missing runner from timeout/ssh/cd launch failures.
    marker = json.dumps({"type": "adapter.runner_missing"}, separators=(",", ":"))
    script = (
        "cd " + shlex.quote(target["work_dir"]) + " && mkdir -p -- output && "
        "{ command -v codex >/dev/null 2>&1 || "
        "{ printf '%s\\n' " + shlex.quote(marker) + "; exit 127; }; "
        "exec timeout --signal=TERM --kill-after=" + str(KILL_GRACE_SECONDS) + "s "
        + str(request["timeout_seconds"]) + "s "
        + " ".join(shlex.quote(token) for token in _locked_argv(inner, target["work_dir"])) + "; }"
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
        events.append(obj)
    session, usage, message = None, None, None
    started, completed, failed, denial, runner_missing = False, False, False, False, False
    lock_acquired = False
    protocol_bad, model_used = False, []
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
        evidence["signal"] = "wall_timeout"
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
        evidence["signal"] = "native_failure"
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
