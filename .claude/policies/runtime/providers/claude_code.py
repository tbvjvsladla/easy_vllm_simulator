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
import sys

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


# 2026-09-03(F5 · plan_26090317 P1): 이 저장소의 다른 모든 ssh 는 BatchMode/ConnectTimeout 을 쓰는데
#   위임 전송만 맨 ssh 였다 — 미등록 host key·패스프레이즈·비밀번호 인증에서 ssh 가 /dev/tty 를 읽으며
#   timeout_seconds(최대 3600s)까지 멈추고, 그 TIMEOUT 뒤에도 **원격 claude 는 계속 돌며 서브
#   워크스페이스를 편집한다**(tty 가 없어 SIGHUP 이 없다). 메인은 실패로 기록했는데 서브는 살아 있는
#   상태가 A2A 원장의 최악 형태다. 처방: 프롬프트를 원천 차단하고, 원격 쪽에도 같은 시한을 건다.
SSH_HARDENING = ("-o", "BatchMode=yes", "-o", "ConnectTimeout=8")


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
        timeout_seconds = request.get("timeout_seconds")
        if isinstance(timeout_seconds, (int, float)) and timeout_seconds > 0:
            # 원격 동반사망: 클라이언트만 죽으면 고아 에이전트가 남는다.
            remote_inner = f"timeout {int(timeout_seconds)} {remote_inner}"
        remote_script = f"cd {shlex.quote(work_dir)} && {remote_inner}" if work_dir else remote_inner
        remote_command = "bash -lc " + shlex.quote(remote_script)
        return ["ssh", *SSH_HARDENING, "--", _ssh_destination(target), remote_command]
    raise ValueError(f"unsupported transport: {transport!r}")


def _diag(request: dict, code: str, message: str, *, stderr: str | None = None,
          stdout: str | None = None) -> None:
    """진단을 stderr 로 낸다(stdout 은 안정 JSON 계약이라 절대 건드리지 않는다).

    2026-09-03(F4 · plan_26090317 P1): 결과 스키마가 additionalProperties=false 라 사유를 결과에
    실을 수 없다 — 그래서 이전 판본은 `completed.stderr` 를 통째로 버렸고, 호스트 미도달·키 거부·
    바이너리 부재·서브 에이전트 실패가 전부 `NONZERO_EXIT` 한 단어가 됐다. 스키마를 넓히는 대신
    **사이드채널(stderr)** 로 원인을 남긴다. 이 함수는 절대 예외를 올리지 않는다.
    """
    try:
        target = request.get("target") or {}
        where = f"{target.get('role')}/{target.get('transport')}"
        print(f"[agent-control] {code}: {message} (target={where})", file=sys.stderr)
        for label, blob in (("stderr", stderr), ("stdout", stdout)):
            text = (blob or "").strip()
            if text:
                print(f"[agent-control]   provider {label} tail: {text[-2000:]}", file=sys.stderr)
    except Exception:  # 진단이 본 경로를 죽이지 않는다
        pass


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
            # stdin=DEVNULL: `claude -p` 는 파이프된 stdin 을 3초 기다렸다가 경고를 찍는다(2026-09-03
            #   서브 실측). 위임에는 넘길 stdin 이 없으므로 명시적으로 닫는다.
            argv, cwd=cwd, capture_output=True, text=True, timeout=timeout_seconds,
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        _diag(request, "TIMEOUT", f"provider exceeded timeout_seconds={timeout_seconds!r}")
        return _result(request, status=STATUS_TIMEOUT, exit_code=EXIT_TIMEOUT, reason_codes=["TIMEOUT"])
    except UnicodeDecodeError:
        _diag(request, "MALFORMED_JSON", "provider stdout was not valid UTF-8")
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["MALFORMED_JSON"])
    except OSError as exc:
        # 2026-09-03(F4): `claude` 가 PATH 에 없거나 work_dir 이 없는 것과 "서브 에이전트가 돌다 실패"
        #   가 같은 NONZERO_EXIT 한 단어로 접혔다. 스키마 enum 은 못 늘리므로 사유는 stderr 로 낸다.
        _diag(request, "NONZERO_EXIT",
              f"provider could not be executed: {type(exc).__name__}: {exc} · argv[0]={argv[0]!r}")
        return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                        reason_codes=["NONZERO_EXIT"])

    if completed.returncode != 0:
        _diag(request, "NONZERO_EXIT",
              f"provider exited {completed.returncode}"
              + (" (ssh transport: 255 = 전송 실패, host key/키인증/네트워크를 먼저 본다)"
                 if request["target"]["transport"] == "ssh" and completed.returncode == 255 else ""),
              stderr=completed.stderr, stdout=completed.stdout)
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
