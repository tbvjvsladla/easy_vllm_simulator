#!/usr/bin/env python3
"""Constitution-owned Claude Code provider adapter (Phase 6, plan_26072506).

The ONLY file in this repo allowed to emit Claude-specific CLI syntax (`claude -p`,
`--model sonnet`, `--output-format json`) outside `.claude/skills/terraforming_node/references/
agent-control-adapter.md`; `.claude/policies/runtime/runtime_selftest.py` enforces this boundary.
The sibling agent_control.py stays provider-neutral and only calls the two pure functions below.

Real `claude -p ... --output-format json` emits a top-level "modelUsage" object keyed by full
model-id strings (e.g. "claude-sonnet-4-5-20250929") -> per-model token-usage stats. That wrapper
metadata -- never the request's own "model" echo -- is what proves Sonnet actually ran. A model id
is classified Sonnet/Opus by case-insensitive substring match on "sonnet"/"opus".
"""
from __future__ import annotations

import json
import shlex
import subprocess

STATUS_COMPLETED = "completed"
STATUS_MODEL_SAFETY_BLOCKED = "model_safety_blocked"
STATUS_EXECUTION_FAILED = "execution_failed"
STATUS_MALFORMED_OUTPUT = "malformed_output"
STATUS_TIMEOUT = "timeout"

EXIT_SUCCESS = 0
EXIT_MODEL_SAFETY_BLOCKED = 3
EXIT_EXECUTION_FAILED = 4
EXIT_MALFORMED_OUTPUT = 5
EXIT_TIMEOUT = 124

PROVIDER_NAME = "claude_code"
CAPABILITY_TO_TOOL = {
    "read": "Read",
    "execute": "Bash",
    "edit": "Edit",
    "write": "Write",
}


def _inner_argv(request: dict) -> list[str]:
    allowed_tools = ",".join(CAPABILITY_TO_TOOL[name] for name in request["capabilities"])
    return [
        "claude",
        "-p", request["task"],
        "--model", request["model"],
        "--output-format", "json",
        "--max-turns", str(request["max_turns"]),
        "--allowedTools", allowed_tools,
    ]


def _ssh_destination(target: dict) -> str:
    host = target["host"]
    ssh_user = target.get("ssh_user")
    return f"{ssh_user}@{host}" if ssh_user else host


def build_argv(request: dict) -> list[str]:
    """Provider CLI invocation argv for `request`. `local` transport returns a flat argv list;
    `ssh` transport wraps it in a single ssh invocation, safely shell-quoting host/user/work_dir
    (never `bypassPermissions` / `--dangerously-skip-permissions`)."""
    target = request["target"]
    inner = _inner_argv(request)
    transport = target["transport"]
    if transport == "local":
        return inner
    if transport == "ssh":
        work_dir = target.get("work_dir")
        remote_inner = " ".join(shlex.quote(tok) for tok in inner)
        remote_script = f"cd {shlex.quote(work_dir)} && {remote_inner}" if work_dir else remote_inner
        remote_command = "bash -lc " + shlex.quote(remote_script)
        return ["ssh", "--", _ssh_destination(target), remote_command]
    raise ValueError(f"unsupported transport: {transport!r}")


def _result(request: dict, *, status: str, exit_code: int, reason_codes: list[str],
            model_used: list[str] | None = None, output: str | None = None) -> dict:
    return {
        "schema_version": request.get("schema_version", 1),
        "provider": PROVIDER_NAME,
        "intent": request["intent"],
        "status": status,
        "exit_code": exit_code,
        "model_requested": request["model"],
        "model_used": model_used or [],
        "reason_codes": reason_codes,
        "output": output,
    }


def invoke(request: dict) -> dict:
    """Actually runs the `claude` provider binary (resolved via PATH) for `request` and returns a
    result dict shaped per agent-control-result.schema.json. Fail-closed (never raises)."""
    if request["model"] != "sonnet":
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED,
                       exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                       reason_codes=["REQUESTED_MODEL_NOT_SONNET"])

    argv = build_argv(request)
    target = request["target"]
    cwd = target.get("work_dir") if target["transport"] == "local" else None
    timeout_seconds = request.get("timeout_seconds")

    try:
        completed = subprocess.run(
            argv, cwd=cwd, capture_output=True, text=True, timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired:
        return _result(request, status=STATUS_TIMEOUT, exit_code=EXIT_TIMEOUT, reason_codes=["TIMEOUT"])
    except UnicodeDecodeError:
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["MALFORMED_JSON"])
    except OSError:
        return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                        reason_codes=["NONZERO_EXIT"])

    if completed.returncode != 0:
        return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                        reason_codes=["NONZERO_EXIT"])

    try:
        payload = json.loads(completed.stdout)
    except (ValueError, RecursionError):
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["MALFORMED_JSON"])
    if not isinstance(payload, dict):
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                        reason_codes=["MALFORMED_JSON"])

    if payload.get("type") != "result":
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["PROVIDER_RESULT_INVALID"])

    denials = payload.get("permission_denials")
    if "permission_denials" in payload:
        if not isinstance(denials, list):
            return _result(request, status=STATUS_MALFORMED_OUTPUT,
                           exit_code=EXIT_MALFORMED_OUTPUT,
                           reason_codes=["PROVIDER_RESULT_INVALID"])
        if denials:
            return _result(request, status=STATUS_EXECUTION_FAILED,
                           exit_code=EXIT_EXECUTION_FAILED,
                           reason_codes=["PERMISSION_DENIED"])

    if payload.get("is_error") is True:
        return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                       reason_codes=["IS_ERROR"])

    if payload.get("is_error") is not False or payload.get("subtype") != "success" \
            or not isinstance(payload.get("result"), str) or not payload["result"]:
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["PROVIDER_RESULT_INVALID"])

    model_usage = payload.get("modelUsage")
    if not isinstance(model_usage, dict) or not model_usage:
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                        reason_codes=["MISSING_MODEL_METADATA"])

    model_ids = list(model_usage.keys())
    if not all(isinstance(model_id, str) for model_id in model_ids):
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED,
                       exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                       reason_codes=["UNEXPECTED_MODEL_USAGE"])
    has_sonnet = any("sonnet" in model_id.lower() for model_id in model_ids)
    has_opus = any("opus" in model_id.lower() for model_id in model_ids)
    has_unknown = any("sonnet" not in model_id.lower() and "opus" not in model_id.lower()
                      for model_id in model_ids)

    if has_sonnet and has_opus:
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                        reason_codes=["MIXED_MODEL_USAGE"], model_used=model_ids)
    if has_opus:
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                        reason_codes=["OPUS_FALLBACK"], model_used=model_ids)
    if has_unknown:
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED,
                       exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                       reason_codes=["UNEXPECTED_MODEL_USAGE"], model_used=model_ids)
    if not has_sonnet:
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                        reason_codes=["MISSING_MODEL_METADATA"], model_used=model_ids)

    canonical_ids = []
    for model_id, metadata in model_usage.items():
        if not isinstance(metadata, dict) or not isinstance(metadata.get("canonicalModel"), str):
            return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED,
                           exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                           reason_codes=["UNEXPECTED_MODEL_USAGE"], model_used=model_ids)
        canonical_ids.append(metadata["canonicalModel"])
        if metadata["canonicalModel"] != model_id:
            return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED,
                           exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                           reason_codes=["UNEXPECTED_MODEL_USAGE"], model_used=model_ids)
    if any("sonnet" not in model_id.lower() or "opus" in model_id.lower()
           for model_id in canonical_ids):
        return _result(request, status=STATUS_MODEL_SAFETY_BLOCKED,
                       exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                       reason_codes=["UNEXPECTED_MODEL_USAGE"], model_used=model_ids)

    return _result(request, status=STATUS_COMPLETED, exit_code=EXIT_SUCCESS, reason_codes=[],
                    model_used=model_ids, output=payload["result"])
