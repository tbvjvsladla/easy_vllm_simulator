#!/usr/bin/env python3
"""Constitution-owned Claude Code provider adapter (Phase 6, plan_26072506).

The ONLY file in this repo allowed to emit Claude-specific CLI syntax (`claude -p`,
`--model sonnet`, `--output-format json`) outside `.claude/skills/terraforming_node/references/
agent-control-adapter.md`; `.claude/policies/runtime/runtime_selftest.py` enforces this boundary.
The sibling agent_control.py stays provider-neutral and only calls the two pure functions below.

Real `claude -p ... --output-format json` emits a top-level "modelUsage" object keyed by full
model-id strings (e.g. "claude-sonnet-4-5-20250929") -> per-model token-usage stats. That wrapper
metadata -- never the request's own "model" echo -- is what says which model actually ran, so it is
recorded as `model_used`.

2026-09-05 (plan_26090516 3-5 / audit_26090515 G-A1): this adapter used to REFUSE anything that was
not Sonnet -- a request-level gate (`REQUESTED_MODEL_NOT_SONNET`) plus a four-way classification of
the wrapper metadata (`OPUS_FALLBACK` / `MIXED_MODEL_USAGE` / `UNEXPECTED_MODEL_USAGE` /
`MISSING_MODEL_METADATA`). That is model overfitting in the harness: the model is the caller's
declaration, and pinning one vendor's model family in the control plane means every model swap is a
harness edit. The gate is gone; the model that ran is RECORDED (`model_used`, and the relay ledger
keeps it next to `model_requested`). Absent or odd metadata is now reported on stderr and leaves
`model_used` empty -- unknown is written as unknown, not as a block.
"""
from __future__ import annotations

import json
import shlex
import subprocess
import sys

STATUS_COMPLETED = "completed"
STATUS_EXECUTION_FAILED = "execution_failed"
STATUS_MALFORMED_OUTPUT = "malformed_output"
STATUS_TIMEOUT = "timeout"

EXIT_SUCCESS = 0
EXIT_EXECUTION_FAILED = 4
EXIT_MALFORMED_OUTPUT = 5
EXIT_TIMEOUT = 124

PROVIDER_NAME = "claude_code"

# 2026-09-07 (Qwen3-4B KV양자화 캠페인 · 사용자 지시): 같은 Claude Code 하네스를 내부 LLM만
#   바꿈(Kimi K3) — provider 가 아니라 **백엔드**가 다른 것이므로 스키마의 선택 필드 `backend` 가
#   바이너리를 고른다. 닫힌 열거(tripwire): 여기 없는 백엔드는 스키마 enum 에서 이미 걸린다.
#   `kimi-claude` 는 노드 로컬 shim(~/.local/bin, 비추적)이며 env 라우팅(엔드포인트·키·모델 슬롯)은
#   shim 이 소유한다 — 이 파일에 키·엔드포인트를 적지 않는다.
BACKEND_TO_BINARY = {"anthropic": "claude", "kimi": "kimi-claude"}
# capability → 하네스 도구. **한 capability 가 여러 도구로 갈 수 있다**(2026-09-04 · plan_26090412
# §6.1 B안). 종전에는 1:1 이라 어휘가 곧 도구였고, 그래서 `WebSearch`/`WebFetch` 로 가는 통로가
# **아예 존재하지 않았다** — 서브의 외부지식 획득 불가는 egress 나 위임 키 때문이 아니라 여기가
# 비어 있었기 때문이다(감사 실측). `search` 는 **하네스 도구 평면**이며 셸 egress(curl/wget)와
# 다른 평면이다 — 서브 권한 템플릿의 셸 네트워크 deny 는 그대로 둔다(방어심층 보존).
CAPABILITY_TO_TOOL = {
    "read": ("Read",),
    "execute": ("Bash",),
    "edit": ("Edit",),
    "write": ("Write",),
    "search": ("WebSearch", "WebFetch"),
}


def _inner_argv(request: dict) -> list[str]:
    allowed_tools = ",".join(tool for name in request["capabilities"]
                             for tool in CAPABILITY_TO_TOOL[name])
    binary = BACKEND_TO_BINARY[request.get("backend") or "anthropic"]  # 스키마 밖 값은 KeyError fail-closed
    argv = [
        binary,
        "-p", request["task"],
        "--model", request["model"],
        "--output-format", "json",
        "--max-turns", str(request["max_turns"]),
        "--allowedTools", allowed_tools,
    ]
    # 2026-09-03(P2 · plan_26090317): 턴제 릴레이. `input-required` 로 끊긴 세션에 답을 실어
    #   **같은 세션을 잇는다** — 새 세션이면 서브가 컨텍스트를 처음부터 재구축하고, 그 비용이
    #   곧 턴 소진의 주된 원인이었다(참고 프로젝트 e2e-lessons: max-turns 2/4 실패·6 성공).
    resume = request.get("resume_session_id")
    if isinstance(resume, str) and resume.strip():
        argv += ["--resume", resume.strip()]
    return argv


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

# 2026-09-05(F · plan_26090516 3-4): 공식 문서 기준 **자동 재개는 대화형 claude.ai 로그인에만** 있고
#   `-p`/게이트웨이 경로에는 없다 — 있는 것은 재시도 변수뿐이다. 서브 위임은 전부 `-p` 라
#   일시적 API 오류 한 번이 attempt 하나를 통째로 버린다. 원격 셸에 이 변수를 실어 재시도를 켠다.
#   (로컬 transport 는 메인 자신이라 대화형 세션과 섞이므로 켜지 않는다 — 평면이 다르다.)
REMOTE_ENV = ("CLAUDE_CODE_RETRY_WATCHDOG=1",)


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
        remote_inner = " ".join(REMOTE_ENV) + " " + remote_inner   # env 는 timeout 앞에 온다
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


def _durations(payload) -> tuple:
    """provider payload 가 보고한 (벽시계, API) 소요 ms. **메인이 재지 않는다** — 이 두 값의 차이가
    정지 시간이고, 그것을 알려면 같은 시계가 잰 두 수여야 한다(2026-09-05 · F 원장)."""
    if not isinstance(payload, dict):
        return None, None
    out = []
    for key in ("duration_ms", "duration_api_ms"):
        v = payload.get(key)
        out.append(v if isinstance(v, int) and not isinstance(v, bool) and v >= 0 else None)
    return out[0], out[1]


def _result(request: dict, *, status: str, exit_code: int, reason_codes: list[str],
            model_used: list[str] | None = None, output: str | None = None,
            session_id: str | None = None, num_turns: int | None = None,
            budget_outcome: str | None = None,
            duration_ms: int | None = None, duration_api_ms: int | None = None) -> dict:
    # 릴레이 원장 필드는 **모르면 null** 이다 — 그럴듯한 값으로 채우면 Layer2 보정이 거짓 위에 선다.
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
        "session_id": session_id,
        "num_turns": num_turns,
        "budget_outcome": budget_outcome,
        "duration_ms": duration_ms,
        "duration_api_ms": duration_api_ms,
    }


def _budget_outcome(payload: dict, request: dict) -> str:
    """성공 경로의 예산 결말.

    **소진은 실패의 한 형태이지 "예산을 다 썼다" 가 아니다.** 작업이 끝났으면 마지막 턴을 썼든
    아니든 `within_budget` 이다 — 소진(`exhausted`)은 provider 가 `subtype=error_max_turns` 로
    말할 때만 성립하고, 그 경로는 이 함수에 오지 않는다(위에서 이미 반환된다).
    천장에 얼마나 붙었는지는 원장의 `num_turns` ÷ `max_turns_allocated` 로 파생되므로 여기서
    별도 값으로 적지 않는다(파생 가능한 것을 손으로 적지 않는다).
    """
    turns = payload.get("num_turns")
    if isinstance(turns, int):
        return "within_budget"
    return "unknown"


def invoke(request: dict) -> dict:
    """Actually runs the `claude` provider binary (resolved via PATH) for `request` and returns a
    result dict shaped per agent-control-result.schema.json. Fail-closed (never raises)."""
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
        # 2026-09-05(F): 시한 초과는 **예산이 모자란 것이 아니다**. 예산을 키워도 같은 자리서 끊긴다 —
        #   원장이 둘을 섞으면 다음 attempt 의 처방을 정반대로 고르게 된다.
        return _result(request, status=STATUS_TIMEOUT, exit_code=EXIT_TIMEOUT, reason_codes=["TIMEOUT"],
                       budget_outcome="external_interruption")
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
        # 2026-09-04(P4 라이브 실측 · plan_26090317): 이 조기 반환이 아래의 **예산 소진 분기를
        #   도달 불가로 만들고 있었다.** `claude -p` 는 max-turns 소진 시 exit 1 로 나가면서
        #   stdout 에는 `subtype:"error_max_turns"` · `num_turns` · `session_id` 가 든 **정상 result
        #   JSON** 을 낸다. returncode 만 보고 돌아서면 그 셋을 통째로 버리게 되고, 원장에는
        #   `budget=None`·`turns=None` 만 남아 다음 attempt 를 얼마나 늘려야 할지 알 수 없다.
        #   (내가 P2 에서 넣은 분기가 선행 게이트와 상호배타라 한 번도 실행되지 않았다 —
        #    단위 자체검사는 못 잡고 **첫 라이브 실행이 알려줬다**.)
        #   그러므로 비-0 종료에서도 **먼저 payload 를 읽어 본다**. 읽히지 않으면 그때 NONZERO_EXIT.
        _payload = None
        try:
            _cand = json.loads(completed.stdout)
            if isinstance(_cand, dict) and _cand.get("type") == "result":
                _payload = _cand
        except (ValueError, RecursionError):
            _payload = None
        if _payload is not None:
            _sess0 = _payload.get("session_id") if isinstance(_payload.get("session_id"), str) else None
            _turns0 = _payload.get("num_turns") if isinstance(_payload.get("num_turns"), int) else None
            _d0, _da0 = _durations(_payload)
            if _payload.get("subtype") == "error_during_execution":
                # provider 가 "실행 도중 밖에서 끊겼다" 고 말하는 형태. 예산 서사가 아니다.
                _diag(request, "NONZERO_EXIT",
                      f"provider 가 실행 도중 중단됐다(subtype=error_during_execution · "
                      f"rc={completed.returncode}).", stderr=completed.stderr)
                return _result(request, status=STATUS_EXECUTION_FAILED,
                               exit_code=EXIT_EXECUTION_FAILED, reason_codes=["NONZERO_EXIT"],
                               session_id=_sess0, num_turns=_turns0,
                               budget_outcome="external_interruption",
                               duration_ms=_d0, duration_api_ms=_da0)
            if _payload.get("subtype") == "error_max_turns":
                _diag(request, "TURN_BUDGET_EXHAUSTED",
                      f"provider 가 max_turns={request.get('max_turns')} 를 소진했다"
                      f"(num_turns={_turns0}). 같은 예산의 자동 재시도 ✗ — 더 큰 예산의 새 attempt 를 "
                      f"열고 session_id 로 이어라(scope ⊥ budget).")
                return _result(request, status=STATUS_EXECUTION_FAILED,
                               exit_code=EXIT_EXECUTION_FAILED, reason_codes=["NONZERO_EXIT"],
                               session_id=_sess0, num_turns=_turns0, budget_outcome="exhausted",
                               duration_ms=_d0, duration_api_ms=_da0)
            _diag(request, "NONZERO_EXIT",
                  f"provider exited {completed.returncode} · subtype={_payload.get('subtype')!r} "
                  f"errors={_payload.get('errors')}", stderr=completed.stderr)
            return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                           reason_codes=["NONZERO_EXIT"], session_id=_sess0, num_turns=_turns0,
                           budget_outcome="aborted", duration_ms=_d0, duration_api_ms=_da0)
        _diag(request, "NONZERO_EXIT",
              f"provider exited {completed.returncode}"
              + (" (ssh transport: 255 = 전송 실패, host key/키인증/네트워크를 먼저 본다)"
                 if request["target"]["transport"] == "ssh" and completed.returncode == 255 else ""),
              stderr=completed.stderr, stdout=completed.stdout)
        # 2026-09-05(F): 원격 `timeout` 이 죽인 것(124)과 밖에서 SIGTERM 을 받은 것(143)은
        #   예산 사건이 아니라 **외생 중단**이다. 종전에는 둘 다 null 로 남아 정지 시간이
        #   "모름" 으로 접혔다.
        return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                       reason_codes=["NONZERO_EXIT"],
                       budget_outcome=("external_interruption"
                                       if completed.returncode in (124, 143) else None))

    # 2026-09-05: 아래 세 분기는 **진단 없이** terminal 이었다. NONZERO_EXIT 경로는 stderr 로
    #   원인을 남기는데 여기만 침묵이라, 원장에는 `PROVIDER_RESULT_INVALID` 한 단어와 null 세 개만
    #   남는다 — "무엇이 왔길래 result 가 아닌가" 를 아무도 알 수 없다. 사이드채널로 원문을 남긴다
    #   (stdout 계약은 건드리지 않는다).
    try:
        payload = json.loads(completed.stdout)
    except (ValueError, RecursionError) as exc:
        _diag(request, "MALFORMED_JSON",
              f"provider stdout 을 JSON 으로 읽지 못했다: {type(exc).__name__}: {exc}",
              stderr=completed.stderr, stdout=completed.stdout)
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["MALFORMED_JSON"])
    if not isinstance(payload, dict):
        _diag(request, "MALFORMED_JSON",
              f"provider stdout 이 JSON 객체가 아니다(type={type(payload).__name__})",
              stderr=completed.stderr, stdout=completed.stdout)
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                        reason_codes=["MALFORMED_JSON"])

    if payload.get("type") != "result":
        _diag(request, "PROVIDER_RESULT_INVALID",
              f"봉투가 result 가 아니다 — type={payload.get('type')!r} subtype={payload.get('subtype')!r} "
              f"keys={sorted(payload)[:12]}",
              stderr=completed.stderr, stdout=completed.stdout)
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["PROVIDER_RESULT_INVALID"])

    _sess = payload.get("session_id") if isinstance(payload.get("session_id"), str) else None
    _turns = payload.get("num_turns") if isinstance(payload.get("num_turns"), int) else None
    _dur, _dur_api = _durations(payload)

    denials = payload.get("permission_denials")
    if "permission_denials" in payload:
        if not isinstance(denials, list):
            return _result(request, status=STATUS_MALFORMED_OUTPUT,
                           exit_code=EXIT_MALFORMED_OUTPUT,
                           reason_codes=["PROVIDER_RESULT_INVALID"])
        if denials:
            # 2026-09-05 실측: 거부 1건이 실행 전체를 버렸고 **무엇이 거부됐는지도, 세션 id 도**
            #   함께 사라졌다. 그래서 호출자는 (a) 권한을 어떻게 고쳐야 하는지 알 수 없고
            #   (b) `--resume` 으로 이어받을 수도 없어, 서브가 한 일이 통째로 고아가 된다.
            #   거부는 terminal 이 맞다 — 하지만 **말없이 terminal 인 것은 교착이다**.
            #   다른 terminal 분기(error_max_turns·PROVIDER_RESULT_INVALID)는 이미 세션을 싣고 있다.
            _det = []
            for _d in denials[:10]:
                if isinstance(_d, dict):
                    _det.append("%s %s" % (
                        _d.get("tool_name") or "?",
                        json.dumps(_d.get("tool_input") or {}, ensure_ascii=False)[:240]))
                else:
                    _det.append(str(_d)[:240])
            _note = "[permission_denials] %d건 — 거부된 도구 호출:\n  - %s" % (
                len(denials), "\n  - ".join(_det))
            _said = payload.get("result")
            if isinstance(_said, str) and _said:
                _note += "\n\n[서브가 남긴 말]\n" + _said
            return _result(request, status=STATUS_EXECUTION_FAILED,
                           exit_code=EXIT_EXECUTION_FAILED,
                           reason_codes=["PERMISSION_DENIED"],
                           output=_note, session_id=_sess, num_turns=_turns,
                           duration_ms=_dur, duration_api_ms=_dur_api)

    if payload.get("is_error") is True:
        return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                       reason_codes=["IS_ERROR"])

    # 2026-09-03(P2 · plan_26090317): `subtype == "error_max_turns"` 는 provider 가 **예산 소진**을
    #   말하는 방식인데, 이전에는 그것이 `PROVIDER_RESULT_INVALID`(형식 오류)로 접혔다 — 원장이
    #   "예산이 모자랐다" 와 "출력이 깨졌다" 를 구분하지 못했고, 그래서 다음 attempt 에 예산을 얼마나
    #   늘려야 하는지 알 수 없었다. 소진은 terminal 이되 **분류가 다르다**.
    if payload.get("subtype") == "error_max_turns":
        _diag(request, "TURN_BUDGET_EXHAUSTED",
              f"provider 가 max_turns={request.get('max_turns')} 를 소진했다(num_turns={_turns}). "
              f"같은 예산의 자동 재시도 ✗ — 더 큰 예산의 새 attempt 를 열어라(scope ⊥ budget).")
        return _result(request, status=STATUS_EXECUTION_FAILED, exit_code=EXIT_EXECUTION_FAILED,
                       reason_codes=["NONZERO_EXIT"], session_id=_sess, num_turns=_turns,
                       budget_outcome="exhausted", duration_ms=_dur, duration_api_ms=_dur_api)

    if payload.get("is_error") is not False or payload.get("subtype") != "success" \
            or not isinstance(payload.get("result"), str) or not payload["result"]:
        _diag(request, "PROVIDER_RESULT_INVALID",
              f"result 봉투가 성공 형태가 아니다 — is_error={payload.get('is_error')!r} "
              f"subtype={payload.get('subtype')!r} result_type={type(payload.get('result')).__name__} "
              f"session={_sess} turns={_turns}",
              stderr=completed.stderr, stdout=completed.stdout)
        return _result(request, status=STATUS_MALFORMED_OUTPUT, exit_code=EXIT_MALFORMED_OUTPUT,
                       reason_codes=["PROVIDER_RESULT_INVALID"], session_id=_sess, num_turns=_turns,
                       budget_outcome="unknown", duration_ms=_dur, duration_api_ms=_dur_api)

    # 2026-09-05(G-A1): 실행 모델은 **기록**한다 — 막지 않는다. 형태가 이상하면 그 사실을
    #   사이드채널로 남기고 `model_used` 를 비운다(모르는 것을 아는 척하지 않는다).
    model_usage = payload.get("modelUsage")
    model_ids = []
    if isinstance(model_usage, dict) and model_usage:
        model_ids = [mid for mid in model_usage if isinstance(mid, str)]
        if len(model_ids) != len(model_usage):
            _diag(request, "MODEL_METADATA_ODD",
                  f"modelUsage 키에 문자열이 아닌 것이 섞였다 — 기록 가능한 것만 남긴다: {model_ids}")
    else:
        _diag(request, "MODEL_METADATA_ABSENT",
              "provider 가 modelUsage 를 주지 않았다 — 어느 모델이 돌았는지 기록할 수 없다"
              "(차단하지 않는다 · model_used=[]).")

    return _result(request, status=STATUS_COMPLETED, exit_code=EXIT_SUCCESS, reason_codes=[],
                    model_used=model_ids, output=payload["result"],
                    session_id=_sess, num_turns=_turns,
                    budget_outcome=_budget_outcome(payload, request),
                    duration_ms=_dur, duration_api_ms=_dur_api)
