#!/usr/bin/env python3
"""relay.py — 메인↔서브 **턴제 릴레이**의 실행자(A2A 평면).

왜 턴제인가(2026-09-03 · P2 · plan_26090317 Q5):
    서브→메인 방향에는 별도 채널을 **만들지 않는다**. 폴링 inbox 는 헌법의 자동 폴링 금지와
    충돌하고, 역방향 ssh 는 서브에게 메인 접속 권한을 주는 것이며, 서브의 쓰기 허용 표면
    (`configs/`·`envs/`·`campaigns/**`·`output/**`) 밖이다. 서브가 메인에게 할 말은 **이미 있는 통로**
    — task-report — 로 온다(§2.7.8 도 그렇게 설계돼 있다). 그러므로 릴레이는:

        delegate → 리포트 수신 → (input-required 면) 답을 실어 **같은 세션 재개** → …

    이 파일이 그 왕복의 상태를 `campaigns/<camp-id>/relay/<context_id>.json`(파일 = 세션)에 적고,
    사람이 답해야 하는 것만 같은 자리의 `pending_hitl.json` 에 표면화한다(2026-09-06 이관 · 옛 루트
    `tasks/` 는 폐지 · root_registry tombstone).

규율(참고 프로젝트 차용 — memory: hermes-control-plane-reference-turn-budget):
    · scope ⊥ budget · 호출 소진 뒤 checkpoint에서 새 예산을 선언해 이어간다
    · SILENT_FALLBACK 금지 — 메인이 대신 한 것을 서브 성공으로 집계하지 않는다
    · 원장은 예산 서사(선언값·출처·사용량·결과)를 매 턴 적는다

2026-09-05 개정(③ 3-1 · G-A2): 예산은 **등급표가 아니라 선언**이다. `--max-turns`·
`--timeout-seconds`·`--budget-source` 를 부르는 쪽이 정하고, 이 파일은 원장의 사실(집행된 바닥·
직전 소진 여부·전진 없는 연속 attempt 수)을 dry-run 에 **보여줄 뿐 대신 정하지 않는다**.
"""
from __future__ import annotations

import argparse
import datetime
import contextlib
import fcntl
import hashlib
import json
import math
import os
import re
import shlex
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
AGENT_CONTROL = os.path.join(HERE, "agent_control.py")
sys.path.insert(0, HERE)
import turn_budget  # noqa: E402
import agent_control as _agent_control  # noqa: E402
import bootstrap_canary as _canary  # noqa: E402  (manifest → target 해소를 재사용)

PENDING_HITL = "pending_hitl.json"

# ★ 2026-09-06(plan_26090616 ②): 원장 루트가 **루트 `tasks/` 에서 활성 캠페인 아래로** 옮겨간다.
#   왜: 루트 원장은 어느 캠페인의 왕복인지 이름으로만 구분됐고, 캠페인이 끝나도 남아 다음 캠페인
#   입력과 섞였다. 활성 캠페인이 없을 때는 예약 id `_bootstrap` 으로 간다 — 부재를 루트 폴백으로
#   처리하지 않는 것이 핵심이다(그 폴백이 산출물 누출의 직접 원인이었다).
#   경로 규칙의 단일 소유자는 `campaign_init.derive_path` 이며 여기서 복제하지 않는다.
import campaign_init as _campaign  # noqa: E402


# 수명 제어(--campaign-control)의 원장 캠페인. 그 지시는 **보류된** 캠페인을 겨냥하므로 ACTIVE(=_bootstrap)에서 파생하면
# 원장이 대기실로 가고, 대기실은 다음 init 이 비운다 — 2026-10-17 라이브(plan_26101719 E2)에서 그렇게 갈 뻔했다.
_RELAY_CAMPAIGN = None


def relay_root(repo_root: str) -> str:
    """이 저장소의 릴레이 원장 루트. 활성 캠페인 선언에서 파생한다(루트 기본값 없음)."""
    return str(_campaign.derive_path("relay-root", repo_root=repo_root, camp_id=_RELAY_CAMPAIGN))


def ledger_path(repo_root: str, context_id: str) -> str:
    return os.path.join(relay_root(repo_root), f"{context_id}.json")


def load_ledger(path: str) -> dict:
    if not os.path.exists(path):
        return {"schema_version": 1, "context_id": None, "attempts": []}
    with open(path, encoding="utf-8") as f:
        return json.load(f)


@contextlib.contextmanager
def state_lock(path: str, *, blocking: bool = True):
    """Serialize a complete state transaction; never use an atomic rename as a lock."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path + ".lock", "a", encoding="utf-8") as stream:
        try:
            fcntl.flock(stream, fcntl.LOCK_EX | (0 if blocking else fcntl.LOCK_NB))
        except BlockingIOError:
            raise SystemExit("[relay] STOP: another writer owns this workspace; inspect its ledger")
        try:
            yield
        finally:
            fcntl.flock(stream, fcntl.LOCK_UN)


def _atomic_json(path: str, doc: dict) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = None
    try:
        with tempfile.NamedTemporaryFile("w", dir=os.path.dirname(path), delete=False,
                                         encoding="utf-8", prefix=".relay-") as f:
            tmp = f.name
            json.dump(doc, f, ensure_ascii=False, indent=2, sort_keys=True)
            f.write("\n")
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp, path)
        directory = os.open(os.path.dirname(path), os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if tmp and os.path.exists(tmp):
            os.unlink(tmp)


def save_ledger(path: str, doc: dict) -> None:
    """Reject stale writers instead of losing an attempt during handoff."""
    with state_lock(path):
        current = load_ledger(path)
        if current.get("revision", 0) != doc.get("revision", 0):
            raise SystemExit("[relay] STOP: stale ledger revision; reload before changing state")
        doc["revision"] = doc.get("revision", 0) + 1
        _atomic_json(path, doc)


RUNNER_UNAVAILABLE = "RUNNER_UNAVAILABLE"


def _reached_sub(att: dict) -> bool:
    """이 attempt 가 서브에 **닿았는가**. 전송·스키마 실패는 예산 서사도 세션도 남기지 않는다.

    ★ 2026-09-08: 러너 평면 실패는 **닿은 것이 아니다**. 함정은 그 봉투가 `session_id` 를 **싣고
      온다**는 데 있다(실측: 인증 실패도 세션 id 를 준다) — 그 세션은 백엔드가 첫 요청 전에 연
      빈 껍데기이고, 여기서 True 를 돌려주면 ⓐ 예산 바닥이 한 번도 집행된 적 없는 값이 되고
      ⓑ 정체 카운트가 러너 회전으로 부풀고 ⓒ `last_known_session` 이 그 껍데기를 마지막 세션으로
      돌려줘 다음 재개가 **빈 세션을 잇는다**. 한 술어를 고치면 네 소비자가 함께 옳아진다.
    """
    if att.get("execution_started") is True:
        return True
    if RUNNER_UNAVAILABLE in (att.get("reason_codes") or []):
        return False
    return bool(att.get("session_id") or att.get("status") or att.get("budget_outcome"))


def last_known_session(doc: dict) -> dict:
    """**마지막으로 알려진 세션**과 그 맥락. 판정하지 않고 사실만 돌려준다.

    2026-09-05 개정(F · plan_26090516 3-3): 종전 이름은 `latest_session_id` 였고, 이 함수가
    *재개할지 말지*를 코드 규칙으로 정했다(소진이면 새 세션 · completed 면 새 세션 · input-required
    면 이어붙임). 그 규칙은 두 번 어긋났다 — ① 완결 뒤에도 옛 세션을 돌려줘 라이브에서 잘못
    이어붙였고(감사 D3) ② 소진 세션을 무조건 버리는 것이 항상 옳지도 않았다(직전 산출물이 그
    세션에만 있는 경우가 있다). 재개는 **맥락을 아는 쪽의 판단**이므로 이제 `--resume` 로
    선언한다. 이 함수는 그 선언을 위한 재료를 보여줄 뿐이다.

    닿지 못한 attempt(전송 실패)는 서브 상태를 바꾸지 못했으므로 건너뛴다.
    """
    for att in reversed(doc.get("attempts") or []):
        if not _reached_sub(att):
            continue
        return {"session_id": att.get("session_id"), "attempt": att.get("attempt"),
                "status": att.get("status"), "control_status": att.get("control_status"),
                "budget_outcome": att.get("budget_outcome"),
                "binding": att.get("session_binding"),
                "provider": (att.get("runner") or {}).get("provider", "claude_code")}
    return {"session_id": None, "attempt": None, "status": None,
            "control_status": None, "budget_outcome": None}


END_REASONS = ("completed", "completed_with_denials", "sub_input_required", "sub_failed",
               "budget_exhausted", "external_interruption", "permission_denied", "malformed_output",
               "invalid_request", "model_blocked", "transport_or_launch_failure",
               "runner_unavailable", "process_interrupted", "control_failure", "unclassified")


def codex_recovery_requirement(att: dict) -> dict | None:
    """An unresolved native failure needs a human answer and an explicit manual resume.

    Derive this from persisted control observations too, so old ledgers or a crash
    before pending-HITL publication cannot silently reopen automatic execution.
    A successful CLI turn with an unparsable task report retains the existing D9
    progress-based resume policy; an observed wall deadline retains it as well.
    """
    provider = att.get("provider") or (att.get("runner") or {}).get("provider")
    if provider != "codex_cli":
        return None
    codes = att.get("reason_codes") or []
    evidence = att.get("failure_evidence") or {}
    if (evidence.get("signal") in ("process_signal", "execution_interrupted")
          or (att.get("control_status") != "timeout" and
              isinstance(att.get("raw_exit_code"), int) and
              (att["raw_exit_code"] < 0 or att["raw_exit_code"] in (130, 137, 143)))):
        kind = "process_interrupted"
    elif "PERMISSION_DENIED" in codes:
        if att.get("status") == "completed":  # existing completed-with-denials contract
            return None
        kind = "permission_denied"
    elif (att.get("control_status") == "timeout" and "TIMEOUT" in codes
          and evidence.get("signal") in ("controlled_local_deadline", "controlled_remote_deadline")):
        return None
    elif att.get("control_status") == "completed":
        return None
    elif (RUNNER_UNAVAILABLE in codes and evidence.get("rotate") is True
          and att.get("execution_started") is not True):
        return None  # positively observed pre-execution missing runner
    else:
        kind = "control_failure"
    return {"kind": kind, "provider": "codex_cli", "attempt": att.get("attempt"),
            "dispatch_id": att.get("dispatch_id"),
            "signal": evidence.get("signal") or "unclassified_control_failure",
            "explicit_resume_required": True}


def end_reason(att: dict) -> str:
    """이 attempt 가 **왜 끝났는가** — 원장에 남는 단일 라벨(축 F).

    파생이지 선언이 아니다: 입력은 전부 이미 기록된 필드(control_status·reason_codes·
    budget_outcome·서브 status)다. 그런데도 값을 적어 두는 이유는, 나중에 "정지 시간을 무엇으로
    계상했나" 를 묻는 쪽이 판정 규칙이 아니라 **그때의 판정 결과**를 봐야 하기 때문이다.
    분류 불가는 `unclassified` 로 남긴다 — 모르는 것을 아는 척하지 않는다.
    """
    codes = att.get("reason_codes") or []
    control, sub = att.get("control_status"), att.get("status")
    recovery = codex_recovery_requirement(att)
    if recovery and recovery["kind"] == "process_interrupted":
        return "process_interrupted"
    if "PERMISSION_DENIED" in codes:
        # ★ 2026-10-01(plan_26100113 D2 · F7): 거부는 **사건**이고 완수 여부는 **리포트**가 말한다. 종전에는 거부 1건이
        #   과업 전체를 실패로 접었다 — 라이브에서 서브가 `/tmp` 쓰기 1건을 거부당한 뒤 서빙·lite·testlog·teardown 을
        #   모두 마치고 completed 리포트를 냈는데 원장은 permission_denied(팝업)였다. 리포트가 파싱되고 completed 일 때만
        #   가른다 — 리포트 부재·다른 status 는 종전대로 permission_denied 다. 거부 목록은 원장에 남는다(숨기지 않는다).
        return "completed_with_denials" if sub == "completed" else "permission_denied"
    # 2026-09-08: 러너(백엔드×모델) 평면이 실패했다. **예산도 전송도 아니다** — 처방은 회전이고,
    #   그래서 라벨을 따로 둔다(같은 이름이면 감독이 같은 처방을 낸다).
    if RUNNER_UNAVAILABLE in codes:
        return "runner_unavailable"
    if recovery:
        return recovery["kind"]
    if att.get("budget_outcome") == "exhausted":
        return "budget_exhausted"
    if att.get("budget_outcome") == "external_interruption" or "TIMEOUT" in codes:
        return "external_interruption"
    if control == "invalid_request":
        return "invalid_request"
    if control == "model_safety_blocked":
        return "model_blocked"
    if control == "malformed_output":
        return "malformed_output"
    if sub == "completed" and control == "completed":
        return "completed"
    if sub == "input-required":
        return "sub_input_required"
    if sub in ("failed", "execution_failed"):
        return "sub_failed"
    if control == "execution_failed" and not att.get("session_id"):
        return "transport_or_launch_failure"
    return "unclassified"


def require_resume(declared, last: dict) -> str:
    """재개 선언을 검증한다. **없으면 fail-loud** — 코드가 대신 정하지 않는다(축 F · 3-3).

    선언 어휘는 둘뿐이다: 이어받을 `session_id`, 또는 새 세션이면 문자열 `new`. 종전에는
    `latest_session_id()` 가 원장을 보고 **혼자 정했고**, 그 규칙이 라이브에서 두 번 어긋났다.
    모르면 묻는 것이 맞다 — 그래서 여기서 죽고, 죽는 자리에 마지막 알려진 세션을 함께 적는다.
    """
    if isinstance(declared, str) and declared.strip():
        return declared.strip()
    raise SystemExit(
        "[relay] STOP: --resume 이 선언되지 않았다 — 재개 여부는 맥락을 아는 쪽이 정한다.\n"
        f"  → 마지막 알려진 세션: {last.get('session_id') or '(없음)'} "
        f"(attempt {last.get('attempt')} · sub_status={last.get('status')} · "
        f"budget={last.get('budget_outcome')})\n"
        "  → 이어받으려면 `--resume <session_id>`, 새 맥락이면 `--resume new` 를 붙여라.")


def budget_floor(doc: dict) -> int:
    """이 context 에서 **실제로 집행된** 최대 예산. 이제 강제가 아니라 **보여주는 사실**이다.

    2026-09-05(G-A2): 종전에는 이 값이 다음 예산의 하한으로 자동 집행됐다. 규율("소진 뒤 감액 ✗")은
    그대로지만 그것을 지키는 주체가 표에서 **선언하는 쪽**으로 옮겨졌다 — 그래서 여기 값은
    dry-run 이 출력하고, 무엇을 선언할지는 과업을 아는 쪽이 정한다.

    ⚠ 닿지 못한 attempt 는 세지 않는다(2026-09-04 라이브 실측): P4 attempt 4 는 104 를 요청했다가
    **요청 스키마가 정상 차단**해 서브에 닿지 않았다. 그 값을 바닥으로 삼으면 한 번도 집행된 적
    없는 예산이 바닥이 되고, 다음 증액이 전송 상한을 넘어 `escalate` 가 fail-loud 한다 —
    거절된 요청이 릴레이를 잠그는 형태다. 바닥은 **집행된 사실**이지 요청한 희망이 아니다.
    """
    return max([int(a["max_turns_allocated"]) for a in (doc.get("attempts") or [])
                if a.get("max_turns_allocated") and _reached_sub(a)] or [0])


def continuation_budget_preview(doc: dict, ladder: list, last: dict | None,
                                next_timeout: int | None) -> tuple[str, str]:
    """Format observed budget facts for the runner actually selected next."""
    runner = ladder[next_runner_index(doc, ladder)]
    provider = runner.get("provider", "claude_code")
    if provider == "codex_cli":
        previous = last or {}
        allocation = previous.get("timeout_seconds_allocated")
        allocation = f"{allocation}초" if isinstance(allocation, int) and not isinstance(allocation, bool) and allocation > 0 else "미관측"
        duration = previous.get("duration_ms")
        if isinstance(duration, (int, float)) and not isinstance(duration, bool) and duration > 0:
            elapsed = f"{duration / 1000:g}초(provider duration_ms)"
        else:
            start, end = _utc_epoch(previous.get("started_utc")), _utc_epoch(previous.get("ended_utc"))
            elapsed = f"{end - start:g}초(main 시작/종료 시각)" if start is not None and end is not None and end > start else "미관측"
        proposed = f"{next_timeout}초(명시)" if next_timeout is not None else "미선언"
        flags = "--timeout-seconds/--budget-source 와 --resume"
        if any(r.get("provider", "claude_code") != "codex_cli" for r in ladder):
            flags += " (혼합 사다리의 Claude fallback용 --max-turns 도 선언 · Codex native turn 아님)"
        return (f"다음 러너 {runner['name']} · provider={provider} · 직전 wall 배정 {allocation}"
                f" · 관측 경과 {elapsed} · 다음 wall 배정 {proposed}", flags)
    return (f"다음 러너 {runner['name']} · provider={provider} · 집행된 native turn 바닥 {budget_floor(doc)}"
            f" · max_turns 스키마 필드 상한 {turn_budget.schema_cap('max_turns')}",
            "--max-turns/--timeout-seconds/--budget-source 와 --resume")


def stalled_attempts(doc: dict) -> int:
    """**전진 없이** 이어진 말미 attempt 수. dry-run 이 이 수를 보여준다.

    2026-09-05(G-A2): 종전에는 이 값이 3(`MAX_ATTEMPTS_BEFORE_HITL`)에 닿으면 릴레이가 스스로
    멈췄다. 그 숫자는 어떤 실측에서도 오지 않았고, 정지 여부는 이미 `--apply`(사람 승인 정문)가
    쥐고 있었다 — 같은 결정을 두 자리에서 하면 한 자리는 판단을 대체하는 상수가 된다.

    전진의 정의는 둘뿐이다 — ⓐ `completed` ⓑ phase 가 바뀌었다. 예산만 키우며 같은 벽에 부딪히는
    것은 전진이 아니다. 이 술어가 없으면 `--continue` 는 무한 재개가 되고, 그것은 "루프를 만들면서
    정지 조건을 나중으로 미룬" 형태다(plan §5 risk).
    """
    atts = [a for a in (doc.get("attempts") or []) if _reached_sub(a)]
    n, phase = 0, None
    for att in reversed(atts):
        if att.get("status") == "completed":
            break
        if phase is None:
            phase = att.get("phase")
        elif att.get("phase") != phase:
            break                            # phase 가 바뀌었다 = 전진했다
        n += 1
    return n


DEFAULT_CAPABILITIES = ("read", "execute", "edit", "write", "search")


def campaign_control_block(campaign_id: str | None, control: dict | None) -> str:
    """위임 헤더의 **캠페인 정체성 + layer-1 통제변인** 칸(2026-09-07 · plan_26090715 §4.8).

    왜: 종전 요청에는 캠페인 id 가 없어서 서브가 자기 인스턴스 이름을 **스스로 지었다**
    (메인 `camp-26090617-gb10-native` vs 서브 `camp7-sub-native`) — 교차 상관이 불가능했다.
    그리고 2026-09-05 요청의 통제변인 표에는 **모델 행이 없었고**, 서브는 저장소 기본 config 의
    다른 모델을 집었다. 나를 것을 헤더가 나른다.
    """
    if not campaign_id and not control:
        return ""
    lines = ["## [relay] 캠페인 정체성 — 서브는 이 id 로 자기 인스턴스를 만든다(이름을 짓지 않는다)\n"]
    if campaign_id:
        lines.append(f"- campaign_id: {campaign_id}\n")
        lines.append(f"- 서브 인스턴스 경로: campaigns/{campaign_id}/  (디렉터리명 = campaign_id)\n")
    if control:
        lines.append("- layer-1 통제변인(메인 단일 창구 · 서브는 **그대로 echo** 한다):\n")
        for k in sorted(control):
            lines.append(f"    - {k}: {control[k]}\n")
    lines.append("- 규약: 리포트 `campaign_id` 에 위 값을 그대로 적고, `control_variables_echo` 에\n"
                 "  위 표를 그대로 되받아 적어라. **부재도 불일치도 메인이 STOP 한다**(fail-closed).\n"
                 "  통제변인을 바꿔야 한다고 판단하면 스스로 바꾸지 말고 `input-required` 로 끊고\n"
                 "  `next_steps` 에 사유를 적어라 — 개정 창구는 메인 하나다.\n\n")
    return "".join(lines)


def relay_header(context_id: str, attempt: int, allocated: int, budget_source: str,
                 timeout_seconds: int | None = None) -> str:
    """위임 본문 머리에 붙는 결정론 헤더.

    2026-09-04 신설(감사 D6/comms 배선 부재). `comms.md` 는 *"메인이 Task 와 함께
    `max_turns_allocated` 를 준다"* 고 적었지만 **주는 코드가 없었다** — 서브는 자기 예산을 모른 채
    일했고, 메인 원장의 `context_id` 는 파일명으로만 존재해 서브가 회신한 것과 **상관 검증이
    불가능**했다(라이브: 메인 `p4-sub-20b-0180` vs 서브 `gpt-oss-20b-vllm0180-single`).
    """
    return (
        "## [relay] 위임 헤더 — 메인이 결정론으로 붙였다. 서브는 이 값을 그대로 회신한다.\n"
        f"- context_id: {context_id}\n"
        f"- attempt: {attempt}\n"
        f"- max_turns_allocated: {json.dumps(allocated)}\n"
        # 2026-09-08(plan_26090813 §4.3): 시간 상한도 **서브가 알아야 한다**. 종전 헤더는 턴만
        # 말했고, 그래서 서브는 자기가 몇 초 뒤에 잘리는지 모른 채 장기 작업을 시작했다 —
        # 2026-09-07 서브 attempt 2회가 그렇게 캡에서 잘렸다(scope ⊥ budget 은 양쪽 다 알아야 성립).
        + (f"- timeout_seconds_allocated: {timeout_seconds}\n" if timeout_seconds else "")
        + f"- 예산 근거(메인 선언): {budget_source}\n"
        "- 규약: 리포트 `context_id` 에 위 값을 그대로 적는다. 예산이 모자라면 **소진하지 말고**\n"
        "  `input-required` 로 끊고 남은 일을 `next_steps` 에 적어라(scope ⊥ budget).\n"
        "- 외부지식을 검색했다면 `external_search[]` 에 질의·출처·요지를 남겨라 — 그 기록이\n"
        "  메인의 자산이 되고, 인용 없는 결정은 거짓이 아니라 **누락**이다(헌법 불변식 B).\n\n"
    )


# ═══════════════════════════════════════════════════════════════════════════════════════════
# 러너 사다리 (2026-09-08 · 사용자 지시)
# ───────────────────────────────────────────────────────────────────────────────────────────
# 어휘: **러너 = (backend, model) 한 쌍**이다. 두 축이 아니다 — shim 이 ANTHROPIC_DEFAULT_*_MODEL
#   을 자기 슬롯으로 덮으므로 kimi 아래의 `sonnet` 은 sonnet 이 아니고, 모델 토큰은 백엔드 사이에서
#   이식되지 않는다. 별칭 표는 **어댑터가 소유**하고 여기서는 읽기만 한다(사본 ✗).
# 사다리 = 러너의 순서 있는 목록. 회전 = 다음 칸으로 **같은 과업·같은 예산·같은 세션 선언**을 다시
#   발급하는 것(재시도도 예산 사건도 아니다 — 바뀌는 것은 *누가 실행하는가* 하나다).
# 소진 = 서브에 닿은 마지막 attempt 이후 사다리를 **한 바퀴** 다 돌았는데 전부 러너 평면에서 실패.
#   여기서만 HITL 이며, 사다리가 1칸이면 첫 실패가 곧 소진이다(사용자 규격).
# 2026-10-26 (사용자 정의 · plan_26102607 후속): 기본값은 `claude -p` · 모델 sonnet **하나뿐**이다. 그 뒤 슬롯의 개수·순서와
#   서드파티 LLM 호출 방식(claude-sub 같은 런처)은 provider 정책·크레딧에 따라 세션마다 바뀌는 **가변 요소**라 기본값에 굽지
#   않는다 — 그때그때 `--runners a,b,c` 로 선언한다(10-25 의 claude-sub 우선 기본값은 그 가변값이 공통층에 굳은 것이었다).
DEFAULT_LADDER = ("sonnet",)


def runner_table(repo_root: str) -> dict:
    """별칭 → {name, backend, model}. **어댑터가 단일 권위**이며 중립 통로로 읽는다."""
    out = subprocess.run([sys.executable, AGENT_CONTROL, "runners", "--provider", "all"],
                         capture_output=True, text=True, cwd=repo_root)
    if out.returncode != 0:
        raise SystemExit(f"[relay] STOP: 러너 표를 읽지 못했다(rc={out.returncode}) — "
                         f"{(out.stderr or out.stdout).strip()[:300]}")
    return {r["name"]: r for r in json.loads(out.stdout)["runners"]}


def resolve_ladder(repo_root: str, spec, *, backend=None, model=None, provider=None) -> list:
    """선언을 사다리(러너 dict 목록)로 편다. 미등재 별칭은 **STOP**(닫힌 목록).

    `--runners` 가 없으면 `--backend`/`--model` 단수 선언을 1칸 사다리로 승격한다 — 옛 호출자가
    그대로 돌고, 기본값(`sonnet` 1칸)은 이 변경 **전과 동작이 같다**.
    """
    table = runner_table(repo_root)
    if spec:
        names = [x.strip() for x in (spec.split(",") if isinstance(spec, str) else spec) if x.strip()]
        if not names:
            raise SystemExit("[relay] STOP: --runners 가 비었다 — 빈 사다리는 실행자가 0이다.")
        unknown = [n for n in names if n not in table]
        if unknown:
            raise SystemExit(f"[relay] STOP: 등재되지 않은 러너 {unknown} — 아는 이름은 "
                             f"{sorted(table)} 다. 새 러너는 어댑터 RUNNER_ALIASES 에 등재한다"
                             f"(닫힌 목록이라 오타가 조용히 통과하지 않는다).")
        return [dict(table[n]) for n in names]
    if backend or model or provider:
        # 단수 선언. 별칭 표에 같은 쌍이 있으면 그 이름을 쓰고, 없으면 익명 러너로 둔다.
        p = provider or "claude_code"
        eligible = [r for r in table.values() if r.get("provider", "claude_code") == p]
        b = backend or (eligible[0]["backend"] if eligible else None)
        m = model or next((r["model"] for r in eligible if r["backend"] == b), None)
        if not m:
            raise SystemExit(f"[relay] STOP: backend={b!r} 의 기본 모델을 알 수 없다 — --model 을 선언하라.")
        named = next((r for r in eligible if r["backend"] == b and r["model"] == m), None)
        return [dict(named)] if named else [{"name": f"{p}:{b}:{m}", "provider": p,
                                             "backend": b, "model": m}]
    return [dict(table[n]) for n in DEFAULT_LADDER]


def ladder_names(ladder: list) -> str:
    return " → ".join(r["name"] for r in ladder)


def next_runner_index(doc: dict, ladder: list) -> int:
    """다음에 쓸 칸. **커서를 저장하지 않고 원장에서 파생한다** — 같은 개념이 두 자리에 앉으면
    갈라지고, 갈라진 쪽이 조용히 늦는다(workflow.md §4종 안티패턴)."""
    names = [r["name"] for r in ladder]
    for att in reversed(doc.get("attempts") or []):
        used = (att.get("runner") or {}).get("name")
        if used in names:
            return (names.index(used) + 1) % len(names) \
                if att.get("end_reason") == "runner_unavailable" else names.index(used)
    return 0


def consecutive_runner_unavailable(doc: dict) -> int:
    """서브에 닿은 마지막 attempt 이후로 **연속** 러너 실패가 몇 번인가(소진 술어의 입력)."""
    n = 0
    for att in reversed(doc.get("attempts") or []):
        if att.get("end_reason") == "runner_unavailable":
            n += 1
            continue
        break
    return n


def campaign_slice_block(topology: str, campaign_id: str | None, delegated_node: str | None) -> str:
    """Carry an independent single-node sub declaration in the A2A task body only."""
    if topology != "single" or not campaign_id or not delegated_node:
        return ""
    ci = _campaign_init()
    base = ci.Path(REPO) / "campaigns" / campaign_id
    decl = ci.read_declaration(base)
    node_doc = next((node for node in (decl.get("nodes") or [])
                     if isinstance(node, dict) and node.get("node_id") == delegated_node), None)
    if not isinstance(node_doc, dict) or node_doc.get("role") != "sub":
        return ""
    # emit_slice owns assignment validation; do not duplicate its slicing rules here.
    slice_doc = ci.emit_slice(base, delegated_node, utc="delegated-task-body")
    blob = json.dumps(slice_doc, ensure_ascii=False, indent=2)
    return ("## [relay] main-derived campaign declaration — stdin으로만 소비(중간 파일 생성 금지)\n"
            f"서브에 campaigns/{campaign_id}/campaign.yaml 이 없으면 아래 JSON을 정확히 stdin으로 넘겨 "
            f"`campaign_init.py --init {campaign_id} --plan-ref {slice_doc.get('plan_ref')} --from-slice - --apply` 를 실행하라.\n"
            "이미 존재하면 같은 JSON을 stdin으로 넘겨 `campaign_init.py --slice-update - --utc <현재 UTC> --apply` 를 실행하고 개정 번호와 값을 확인하라.\n"
            "```json\n" + blob + "\n```\n\n")


CAMPAIGN_CONTROLS = {"suspend": "suspended", "resume": "active", "close": "closed", "abandon": "closed",
                     "reslice": "active"}
# 종결 제어의 echo 는 상태만으로 갈리지 않는다(둘 다 closed) — 사유까지 대조한다(plan_26101722 K1).
CONTROL_CLOSE_REASON = {"close": "completed", "abandon": "abandoned"}


def campaign_control_task(campaign_id: str, control: str, *, preempt: bool = False,
                          reason: "str | None" = None, slice_doc: "dict | None" = None) -> str:
    """수명 전이의 서브 전달(2026-10-17 · plan_26101719 D9). 메인이 캠페인을 보류·재개하면 같은 전환 안에서 이 지시가
    나간다. 서브는 **같은 campaign_init.py** 로 자기 인스턴스를 전이하고 echo 를 돌려준다 — 메인은 서브 인스턴스를
    직접 읽지 않는다(불변식 1). echo 가 오지 않으면 메인은 '서브 미수렴' 으로 두고 진행한다(차단 ✗)."""
    if control not in CAMPAIGN_CONTROLS:
        raise SystemExit(f"[relay] STOP: --campaign-control 은 {tuple(CAMPAIGN_CONTROLS)} 중 하나다: {control!r}")
    ci = "python3 .claude/skills/terraforming_node/scripts/campaign_init.py"
    if control == "suspend":
        cmd = (f"{ci} --suspend {campaign_id} --release <이 캠페인을 위해 네가 띄운 서빙 config | none>"
               + (" --preempt" if preempt else "")
               + " --approved-by \"main-a2a: 메인 전환 승인 전사\" --utc <지금 UTC> --apply")
        what = ("보류(suspend)했다. 네가 이 캠페인을 위해 띄운 서빙은 위 명령이 정식 teardown 으로 내린다"
                "(compose down 직접 ✗ · 띄운 것이 없으면 `--release none`)."
                + (" 메인이 강제 전환(--preempt)을 승인했다 — 진행 중 셀은 interrupted 로 남는다." if preempt else
                   " 진행 중 셀이 있으면 강제하지 말고 failed 로 보고하라(협조 전환이 기본)."))
    elif control == "resume":
        cmd = f"{ci} --resume-campaign {campaign_id} --approved-by \"main-a2a: 메인 재개 승인 전사\" --utc <지금 UTC> --apply"
        what = ("재개(resume)했다. 재개 게이트(핀·digest·토폴로지 대조)가 거부하면 우회하지 말고 그 안내문 원문을 "
                "보고하라 — drift 수용은 사람 결정이다.")
    elif control == "reslice":
        # 재배정(K8). 선언은 stdin 으로만 소비한다(중간 파일 ✗ — 개설 지시서와 같은 규율).
        if not isinstance(slice_doc, dict):
            raise SystemExit("[relay] STOP: reslice 는 메인 파생 선언(slice)이 필요하다")
        rev = (slice_doc.get("_assignment_revision") or {}).get("rev")
        cmd = f"{ci} --slice-update - --utc <지금 UTC> --apply   # stdin = 아래 JSON 그대로"
        what = (f"재배정(reslice · 메인 rev {rev})했다. 네 인스턴스의 배정을 아래 선언으로 바꾼다 — pending 이 아닌 셀을 "
                "빼거나 바꾸려 해서 writer 가 거부하면 우회하지 말고 그 사유 원문을 보고하라(메인이 본 상태와 네 실물이 "
                "갈라졌다는 뜻이다).\n```json\n" + json.dumps(slice_doc, ensure_ascii=False, indent=2) + "\n```")
    elif control == "close":
        cmd = f"{ci} --close {campaign_id} --utc <지금 UTC> --apply"
        what = ("완주 종결(close)했다. 네 인스턴스도 닫는다 — 종결 선행조건(증거 포인터 실재·릴레이 요약)이 거부하면 "
                "우회하지 말고 그 사유 원문을 보고하라(사람이 포기로 바꿀지 정한다).")
    else:
        cmd = (f"{ci} --abandon {campaign_id} --reason \"{reason or '메인 포기 종결'}\" "
               "--summary-testlog <네가 발행한 포기 요약 testlog — doc_naming 규약 이름 · 포기 사유·미완 셀 목록> "
               "--release <이 캠페인을 위해 네가 띄운 서빙 config | none> "
               "--approved-by \"main-a2a: 메인 포기 승인 전사\" --utc <지금 UTC> --apply")
        what = ("포기 종결(abandon)했다. 네 인스턴스도 같은 사유로 닫는다 — 먼저 포기 요약 testlog 1건을 규약 이름"
                "(`docs/testlog/testlog_<KST YYMMDDHH>_<주제>.md`)으로 발행하고 그것을 인용한다. 축소 게이트가 거부하면 "
                "우회하지 말고 사유 원문을 보고하라.")
    return (f"## [relay] 캠페인 수명 제어 — {control}\n"
            f"메인이 캠페인 `{campaign_id}` 를 {what}\n\n"
            f"1. 실행: `{cmd}`\n"
            f"2. `{ci} --resume-brief` 의 **첫 줄**을 읽는다.\n"
            f"3. 리포트 JSON 에 `campaign_control_echo` 를 적는다: "
            f"{{\"campaign_id\": \"{campaign_id}\", \"control\": \"{control}\", "
            f"\"state\": \"<campaign_init.py --lifecycle {campaign_id} 의 state>\", "
            + ("\"close_reason\": \"<같은 출력의 close_reason>\", " if control in CONTROL_CLOSE_REASON else "")
            + ("\"rev\": <위 선언의 _assignment_revision.rev>, \"assigned_cells\": [<campaign.yaml 의 네 배정 셀 순서대로>], "
               if control == "reslice" else "")
            + f"\"first_line\": \"<첫 줄>\"}}. "
            f"실패하면 state 에 실패 사유를 적고 status 를 failed 로 둔다(성공인 척 ✗).\n"
            f"4. 셀 진행·빌드·서빙 기동은 **하지 않는다** — 이 지시는 수명 전이 하나다.\n\n")


def orphan_sweep_task(campaign_ids: list, reason: str) -> str:
    """메인 인스턴스가 없는 서브 orphan 들의 묶음 포기(K4). 대상 목록은 사람이 팝업에서 고른 것이다 — 서브가 늘리거나 줄이지
    않는다. 요약 testlog 는 1건을 묶어 쓰고 각 `--abandon` 이 그것을 인용한다."""
    ci = "python3 .claude/skills/terraforming_node/scripts/campaign_init.py"
    lines = "\n".join(f"   - `{ci} --abandon {c} --reason \"{reason}\" --summary-testlog <위 요약> --release none "
                      f"--approved-by \"main-a2a: 사람이 고른 orphan 정리 전사\" --utc <지금 UTC> --apply`"
                      for c in campaign_ids)
    return ("## [relay] 캠페인 수명 제어 — orphan sweep(abandon)\n"
            f"메인에는 인스턴스가 없고 네 쪽에만 남은 캠페인 {len(campaign_ids)}건을 사람이 포기로 정리하기로 했다(사유: {reason}).\n\n"
            "1. 포기 요약 testlog 1건을 규약 이름(`docs/testlog/testlog_<KST YYMMDDHH>_<주제>.md`)으로 발행한다 — 대상 id 전수 · "
            "각자의 fold state · 미완 셀 · 사유.\n"
            f"2. 아래를 차례로 실행한다(목록 밖 캠페인은 건드리지 않는다):\n{lines}\n"
            "   축소 게이트가 거부한 것은 우회하지 말고 그 사유 원문을 결과에 적고 다음으로 넘어간다.\n"
            f"3. `{ci} --resume-brief` 의 **첫 줄**을 읽는다.\n"
            "4. 리포트 JSON 에 `campaign_control_echo` 를 적는다: {\"control\": \"orphan_sweep\", \"summary_testlog\": \"<경로>\", "
            "\"results\": [{\"campaign_id\": \"<id>\", \"state\": \"<--lifecycle 의 state>\", \"close_reason\": \"<같은 출력>\", "
            "\"refusal\": \"<거부 원문 | null>\"}, …], \"first_line\": \"<첫 줄>\"}.\n"
            "5. 셀 진행·빌드·서빙 기동은 **하지 않는다**.\n\n")


def orphan_sweep_echo_ok(report, campaign_ids: list) -> "tuple[bool, str]":
    echo = (report or {}).get("campaign_control_echo") if isinstance(report, dict) else None
    if not isinstance(echo, dict) or echo.get("control") != "orphan_sweep":
        return False, "리포트에 orphan_sweep echo 가 없다"
    got = {r.get("campaign_id"): r for r in (echo.get("results") or []) if isinstance(r, dict)}
    left = [c for c in campaign_ids if (got.get(c) or {}).get("state") != "closed"
            or (got.get(c) or {}).get("close_reason") != "abandoned"]
    extra = sorted(set(got) - set(campaign_ids))
    if extra:
        return False, f"목록 밖 캠페인을 건드렸다: {extra}"
    if left:
        return False, f"미정리 {len(left)}건: " + "; ".join(f"{c}={(got.get(c) or {}).get('state')}"
                                                        f"({(got.get(c) or {}).get('refusal')})" for c in left)
    return True, f"orphan {len(campaign_ids)}건 closed(abandoned) · 요약 {echo.get('summary_testlog')} · 첫 줄: {echo.get('first_line')}"


def campaign_control_echo_ok(report, campaign_id: str, control: str,
                             slice_doc: "dict | None" = None) -> "tuple[bool, str]":
    echo = (report or {}).get("campaign_control_echo") if isinstance(report, dict) else None
    if not isinstance(echo, dict):
        return False, "리포트에 campaign_control_echo 가 없다"
    want = CAMPAIGN_CONTROLS[control]
    if echo.get("campaign_id") != campaign_id or echo.get("control") != control:
        return False, f"echo 정체성 불일치: {echo.get('campaign_id')!r}/{echo.get('control')!r}"
    if echo.get("state") != want:
        return False, f"서브 상태가 {echo.get('state')!r} 다(기대 {want!r})"
    if control == "reslice":
        want_cells = [i.get("cell") for items in ((slice_doc or {}).get("assignments") or {}).values()
                      for i in (items or []) if isinstance(i, dict)]
        want_rev = ((slice_doc or {}).get("_assignment_revision") or {}).get("rev")
        if echo.get("rev") != want_rev or list(echo.get("assigned_cells") or []) != want_cells:
            return False, (f"서브 배정이 메인 선언과 다르다: rev {echo.get('rev')!r}/{want_rev!r} · "
                           f"셀 {echo.get('assigned_cells')!r}/{want_cells!r}")
    if control in CONTROL_CLOSE_REASON and echo.get("close_reason") != CONTROL_CLOSE_REASON[control]:
        return False, (f"서브 종결 사유가 {echo.get('close_reason')!r} 다(기대 {CONTROL_CLOSE_REASON[control]!r}) — "
                       "같은 closed 라도 사유가 다르면 수렴이 아니다")
    return True, f"서브 {control} echo 수렴 · 첫 줄: {echo.get('first_line')}"


def build_request(topology: str, manifest: str, task: str, bud: dict,
                  resume_session_id=None, capabilities=None,
                  context_id: str = None, attempt: int = 0, model: str = None,
                  campaign_id: str = None, control_variables: dict = None,
                  backend: str = None, delegated_node: str | None = None,
                  provider: str = "claude_code", sandbox_mode: str | None = None,
                  permission_profile: str | None = None, resume_binding: dict | None = None,
                  reasoning_effort: str | None = None) -> dict:
    """위임 request 조립. `bud` 는 **선언된** 예산이다(`turn_budget.declare` 산출)."""
    base = _canary.build_request(topology, manifest, max_turns=bud["max_turns"],
                                 timeout_seconds=bud["timeout_seconds"],
                                 budget_source=bud["source"],
                                 model=model, backend=backend, provider=provider,
                                 sandbox_mode=sandbox_mode,
                                 permission_profile=permission_profile, reasoning_effort=reasoning_effort)
    allocated = bud["max_turns"]
    base["intent"] = "delegate"
    camp_block = campaign_control_block(campaign_id, control_variables)
    slice_block = campaign_slice_block(topology, campaign_id, delegated_node)
    base["task"] = ((relay_header(context_id, attempt, allocated, bud["source"],
                                  bud.get("timeout_seconds")) + camp_block + slice_block + task)
                    if context_id else (camp_block + slice_block + task))
    if campaign_id:
        base["campaign_id"] = campaign_id
    if control_variables:
        base["control_variables"] = dict(control_variables)
    base["capabilities"] = list(capabilities or DEFAULT_CAPABILITIES)
    if provider == "codex_cli":
        base["task"] = (
            f"예산 단위는 wall_time 이며 native turn 제한은 없다. 시간 상한 {bud['timeout_seconds']}초. "
            "max_turns_allocated=null 을 0턴이나 무제한 실행 승인으로 해석하지 마라. "
            "AGENTS.md와 그 문서가 지정한 정본을 읽고, 제공된 아티팩트에서 이어가라.\n" + base["task"])
    if resume_session_id:
        base["resume_session_id"] = resume_session_id
        if provider == "codex_cli":
            base["resume_binding"] = resume_binding
    if backend:
        base["backend"] = backend
    return base


def resolve_campaign_axis(a) -> "tuple[str | None, dict | None]":
    """요청에 실을 캠페인 정체성 + layer-1 통제변인을 **활성 캠페인 선언에서 파생**한다.

    손으로 적지 않는다 — 2026-09-05 요청의 통제변인 표에서 모델 행이 빠진 것이 손저작의 결과였다.
    `_bootstrap`(캠페인 밖)이면 싣지 않는다: 캠페인 밖 온보딩·카나리가 캠페인 정체성을 주장하면
    그 왕복이 캠페인 흉내를 내게 된다.
    """
    if getattr(a, "no_campaign", False):
        return None, None
    camp = getattr(a, "campaign_id", None)
    if not camp:
        try:
            camp = _campaign_init().active_campaign_id(a.repo_root)
        except Exception:                                    # noqa: BLE001
            return None, None
    if not camp or camp == "_bootstrap":
        return None, None
    decl_path = os.path.join(a.repo_root, "campaigns", camp, "campaign.yaml")
    if not os.path.isfile(decl_path):
        return camp, None
    try:
        with open(decl_path, encoding="utf-8") as fh:
            decl = json.load(fh)
    except (OSError, ValueError):
        return camp, None
    revs = decl.get("revisions")
    cur = (revs[-1].get("values") if isinstance(revs, list) and revs else None) \
        or decl.get("control_variables") or {}
    # `_` 로 시작하는 키는 사람 주석이다 — echo 대조의 시민이 아니다.
    control = {k: v for k, v in cur.items()
               if isinstance(k, str) and not k.startswith("_") and not isinstance(v, (dict, list))}
    return camp, (control or None)


def require_official_axis_revision(repo_root: str, campaign_id: str, old_axis, new_axis) -> None:
    """Only an explicit appended campaign revision can change a live context's axis."""
    try:
        path = Path(repo_root) / "campaigns" / campaign_id / "campaign.yaml"
        declaration = json.loads(path.read_text(encoding="utf-8"))
        revisions = declaration.get("revisions")
        if (declaration.get("id") != campaign_id or not isinstance(old_axis, dict) or not old_axis
                or not isinstance(revisions, list) or len(revisions) < 2
                or not isinstance(declaration.get("control_variables"), dict)
                or revisions[0].get("values") != declaration["control_variables"]):
            raise ValueError("missing original or revision")
        def axis(values):
            if not isinstance(values, dict):
                raise ValueError("invalid revision values")
            return {k: v for k, v in values.items() if isinstance(k, str) and not k.startswith("_")
                    and not isinstance(v, (dict, list))}
        for i, revision in enumerate(revisions):
            if not isinstance(revision, dict) or type(revision.get("rev")) is not int or revision["rev"] != i:
                raise ValueError("invalid revision sequence")
            if i and any(not isinstance(revision.get(k), str) or not revision[k].strip()
                         for k in ("declared_utc", "reason", "evidence", "approved_by")):
                raise ValueError("incomplete revision approval")
        if axis(revisions[-1].get("values")) != new_axis or not any(
                axis(revision.get("values")) == old_axis for revision in revisions[:-1]):
            raise ValueError("historical or current control axis not in revision chain")
    except (OSError, ValueError, TypeError, AttributeError, KeyError):
        raise SystemExit("[relay] STOP: control axis change lacks an approved campaign revision")


def _campaign_init():
    import importlib.util
    mod_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "campaign_init.py")
    spec = importlib.util.spec_from_file_location("_campaign_init_for_relay", mod_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


CONTEXT_KINDS = ("build", "cell")


def bind_campaign_context(doc: dict, *, node: str | None, kind: str | None,
                          cell: str | None) -> None:
    """원장에 **캠페인 축**을 못박는다(2026-09-08 · plan_26090813 §4.3).

    왜 이름 파싱이 아니라 선언인가: context_id 는 사람이 짓는 문자열이고, 거기서 노드·종류를
    추론하면 이름을 바꾸는 순간 술어가 조용히 눈이 먼다(이 저장소가 여러 번 만난 형태). P4 는
    "서브 빌드 지시서가 메인 첫 착수보다 먼저 나갔는가" 를 이 두 필드로 읽는다.

    ★ 셀 context 는 **셀 하나**다(사용자 결정 D20). 서빙→벤치가 한 attempt 의 범위이고 빌드는
      별도 문맥이다 — 2026-09-07 에는 셀 둘을 한 context 에 묶어 2회 모두 캡에서 잘렸고,
      서브 자신이 "셀 단위 attempt" 를 권고했다.
    """
    if kind is not None and kind not in CONTEXT_KINDS:
        raise SystemExit(f"[relay] STOP: --context-kind 는 {CONTEXT_KINDS} 중 하나다: {kind!r}")
    for key, val in (("campaign_node", node), ("campaign_context_kind", kind)):
        if val is None:
            continue
        have = doc.get(key)
        if have and have != val:
            raise SystemExit(
                f"[relay] STOP: 이 원장의 {key} 는 이미 {have!r} 다 — {val!r} 로 바꾸려면 "
                f"새 context 를 열어라(한 원장이 두 정체성을 주장하면 상관검증이 무너진다).")
        doc[key] = val
    if cell:
        have = doc.get("campaign_cell")
        if have and have != cell:
            raise SystemExit(
                f"[relay] STOP: 이 context 는 이미 셀 {have!r} 의 것이다 — {cell!r} 은 "
                f"**새 context** 로 연다(셀 하나 = context 하나 · D20).\n"
                f"  셀 둘을 한 문맥에 묶으면 그 attempt 는 캡에서 잘린다(2026-09-07 실측 2/2).")
        doc["campaign_cell"] = cell


def derive_timeout(repo_root: str, campaign_id, node, *, margin: float = 1.5):
    """같은 노드의 **지난 phase 실측**에서 시간 예산을 파생한다(2026-09-08 · D21).

    실측이 없으면 파생하지 않고 그 사실을 말한다 — 상한 값을 여기 리터럴로 다시 적으면 그 상수가
    두 자리에 손으로 적힌 값이 된다(workflow.md §4종 안티패턴 '매직넘버' 결함 칸). 그래서 이
    설명문에도 그 숫자를 쓰지 않는다: 규칙을 적은 줄이 자기 스캔에 걸리면 게이트가 무의미해진다
    (docs.md §Docker 브리지 예외의 선례와 같은 처방).
    돌려주는 것은 (초 또는 None, 근거 문장) 이며 근거는 그대로 `budget_source` 에 실린다.
    """
    cap = turn_budget.schema_cap("timeout_seconds")
    if not campaign_id or campaign_id == "_bootstrap" or not node:
        return None, f"실측 파생 불가(캠페인·노드 미선언) — 스키마 상한 {cap} 사용"
    pdir = os.path.join(repo_root, "campaigns", campaign_id, "phases", node)
    spans = []
    for name in sorted(os.listdir(pdir)) if os.path.isdir(pdir) else []:
        if not name.endswith(".status.json"):
            continue
        try:
            with open(os.path.join(pdir, name), encoding="utf-8") as fh:
                st = json.load(fh)
        except (OSError, ValueError):
            continue
        a, b = st.get("first_started_utc") or st.get("started_utc"), st.get("ended_utc")
        if not (isinstance(a, str) and isinstance(b, str)):
            continue
        try:
            t0 = datetime.datetime.strptime(a, "%Y-%m-%dT%H:%M:%SZ")
            t1 = datetime.datetime.strptime(b, "%Y-%m-%dT%H:%M:%SZ")
        except ValueError:
            continue
        secs = int((t1 - t0).total_seconds())
        if secs > 0:
            spans.append((secs, name))
    if not spans:
        return None, f"{node} 의 phase 실측이 없다(진행표에 시각 없음) — 스키마 상한 {cap} 사용"
    worst, where = max(spans)
    want = int(worst * margin)
    used = min(want, cap)
    note = "" if used == want else f" · 스키마 상한 {cap} 으로 clamp"
    return used, (f"{node}/{where} 실측 {worst}s x {margin} = {want}s{note} "
                  f"(campaigns/{campaign_id}/phases 파생)")


def echo_stop_reasons(att: dict, *, context_id: str, campaign_id: str | None,
                      control_variables: dict | None) -> list[str]:
    """서브가 되받아 적은 정체성을 판정한다 — **부재도 불일치도 STOP**(2026-09-07 · 인터뷰 Q7).

    ★ 종전 검사는 `if att["context_id_reported"] and …` 였다. 값이 있을 때만 물었으므로
      **부재는 침묵 통과**했다. 캠페인 ⑦ 왕복 4회 중 3회가 echo 부재였고 원장은 그대로 진행했다
      (testlog_26090716 §3). 부재를 통과시키면 "서브가 규약을 안 지켰다"와 "서브가 다른 작업을
      하고 있다"가 구분되지 않는다 — 둘 다 이어가면 안 되는 상태다.

    서브 리포트가 아예 없는 attempt(크래시)는 이 판정의 대상이 아니다. 그것은 echo 위반이 아니라
    도달 실패이고, 이미 `end_reason` 이 말한다(같은 사실을 두 사유로 세지 않는다).
    """
    if not att.get("sub_reported"):
        return []
    reasons: list[str] = []
    got_ctx = att.get("context_id_reported")
    if not got_ctx:
        reasons.append(f"context_id echo 부재 — 메인은 '{context_id}' 를 헤더로 보냈다. "
                       f"서브가 그것을 되받지 않으면 두 원장이 같은 작업인지 확인할 수단이 없다")
    elif got_ctx != context_id:
        reasons.append(f"context_id 불일치 — 메인 '{context_id}' vs 서브 '{got_ctx}'")
    if campaign_id:
        got_camp = att.get("campaign_id_reported")
        if not got_camp:
            reasons.append(f"campaign_id echo 부재 — 메인은 '{campaign_id}' 를 보냈다. "
                           f"부재를 통과시키면 서브가 자기 인스턴스 이름을 스스로 짓는다"
                           f"(2026-09-07 실측: 서브가 'camp7-sub-native' 를 저작했다)")
        elif got_camp != campaign_id:
            reasons.append(f"campaign_id 불일치 — 메인 '{campaign_id}' vs 서브 '{got_camp}'")
    if control_variables:
        echo = att.get("control_variables_echo")
        if not isinstance(echo, dict) or not echo:
            reasons.append("control_variables echo 부재 — layer-1 선언은 메인 단일 창구이고 "
                           "서브는 그대로 되받아야 한다(2026-09-05: 모델 행이 빠져 서브가 다른 "
                           "모델을 집었다)")
        else:
            diff = [k for k, v in control_variables.items()
                    if str(echo.get(k)) != str(v)]
            if diff:
                reasons.append("control_variables 불일치 — " + ", ".join(
                    f"{k}: 메인 {control_variables[k]!r} vs 서브 {echo.get(k)!r}" for k in diff)
                    + ". 서브가 선언을 바꿨다면 그것은 개정이 아니라 드리프트다")
    return reasons


def parse_runner_evidence(result: dict) -> dict | None:
    """러너-불가 결과의 `output` 에 실린 증거 JSON 을 되읽는다(어댑터가 실어 보낸 그대로).

    결과 스키마를 넓히는 대신 `output` 에 싣는 것이 이 저장소의 선례이고(PERMISSION_DENIED),
    되읽는 자리를 만들지 않으면 그 증거는 **또 도달하지 못한다**.
    """
    if RUNNER_UNAVAILABLE not in (result.get("reason_codes") or []):
        return None
    if isinstance(result.get("failure_evidence"), dict):
        return dict(result["failure_evidence"])
    text = result.get("output") or ""
    i = text.find("{")
    if i < 0:
        return {"signal": "unparsed", "rotate": False}   # 모르면 회전하지 않는다
    try:
        ev = json.loads(text[i:])
    except ValueError:
        return {"signal": "unparsed", "rotate": False}
    return ev if isinstance(ev, dict) else {"signal": "unparsed", "rotate": False}


def _denial_block(result: dict):
    """provider 가 거부 시 output 머리에 싣는 `[permission_denials] …` 블록을 그대로 돌려준다(없으면 None)."""
    if "PERMISSION_DENIED" not in (result.get("reason_codes") or []):
        return None
    out = result.get("output")
    if not isinstance(out, str) or not out.startswith("[permission_denials]"):
        return None
    return out.split("\n\n[서브가 남긴 말]", 1)[0]


def record_attempt(doc: dict, *, context_id: str, bud: dict, result: dict, report=None,
                   resume_declared=None, request_path=None, runner=None,
                   started_utc=None, ended_utc=None, session_binding=None, dispatch_id=None) -> dict:
    """원장 한 줄을 **덧붙인다**(append-only — 앞선 attempt 는 건드리지 않는다).

    2026-09-05(축 F): 요청경로·응답경로·종료사유·시작/종료 UTC·소요 2필드를 남긴다. 종전 원장은
    "무엇을 보냈는지" 를 남기지 않아, 재개 본문이 실제로 무엇이었는지 사후에 알 수 없었다
    (조립기는 매번 다시 조립하므로 재현도 되지 않는다).
    시각의 출처가 다르다: `started/ended_utc` 는 **메인이 잰 벽시계**, `duration_*` 은
    **provider 가 보고한 값**이다 — 섞으면 정지 시간(wall − api)이 두 시계의 차가 된다.
    """
    report = report or {}
    resume_requested = None if resume_declared in (None, "new") else resume_declared
    session = result.get("session_id")
    evidence = result.get("failure_evidence")
    # Keep decisions self-contained without copying an entire native transcript
    # into the frequently rewritten ledger. Full raw evidence stays in the unique
    # durable outcome and report control, referenced by their persisted paths.
    compact_evidence = ({key: evidence[key] for key in
                         ("signal", "rotate", "returncode", "termination", "lock_acquired", "malformed_jsonl")
                         if key in evidence} if isinstance(evidence, dict) else None)
    att = {
        "attempt": len(doc.get("attempts") or []) + 1,
        # 2026-09-05(G-A2): `task_grade` 대신 **선언된 예산과 그 근거**를 적는다. 등급 라벨은
        #   어휘가 사라졌고, 남길 가치가 있는 것은 "얼마를 왜 줬는가" 다.
        "max_turns_allocated": bud["max_turns"],
        "timeout_seconds_allocated": bud["timeout_seconds"],
        "budget_source": bud["source"],
        "budget_unit": bud.get("budget_unit", "native_turns"),
        "execution_started": result.get("execution_started"),
        "raw_exit_code": result.get("raw_exit_code"),
        "usage": result.get("usage"),
        "provider": result.get("provider") or (runner or {}).get("provider"),
        "failure_evidence": compact_evidence,
        "session_binding": session_binding,
        "dispatch_id": dispatch_id,
        "budget_recommended": report.get("budget_recommendation"),
        # ── 축 F 원장(2026-09-05): 요청·응답 경로와 시간 서사
        "request_path": request_path,
        "started_utc": started_utc,
        "ended_utc": ended_utc,
        "duration_ms": result.get("duration_ms"),
        "duration_api_ms": result.get("duration_api_ms"),
        # 재개는 이제 **선언**이다. 무엇을 선언했는지(new 포함)를 그대로 남긴다.
        "resume_declared": resume_declared,
        # 모르면 null — 그럴듯한 값으로 채우면 Layer2 보정이 거짓 위에 선다.
        "max_turns_used": result.get("num_turns"),
        "budget_outcome": result.get("budget_outcome"),
        "session_id": session,
        # 2026-09-04(감사 D7): 재개를 **요청했다는 사실**을 남긴다. 종전에는 요청한 세션 id 를 적는
        #   자리조차 없어, provider 가 재개에 실패하고 새 세션을 열어도 원장에는 정상으로 보였다
        #   — 그 경우 서브는 컨텍스트를 처음부터 재구축하고, 그것이 곧 소진의 주된 원인이다.
        "resume_requested": resume_requested,
        "resume_honored": (None if not resume_requested else session == resume_requested),
        # 2026-09-05(G-A1): 모델 게이트가 사라진 자리에 **기록**이 온다 — 무엇을 선언했고
        #   무엇이 실제로 돌았는지. 이 두 줄이 없으면 "하네스 변경 0 으로 모델을 바꿨다" 를
        #   나중에 증명할 수 없다.
        "model_declared": result.get("model_requested"),
        "model_used": result.get("model_used") or [],
        # 2026-09-08: **어느 러너가 이 결정을 냈는가**. 회전이 조용하면 그 결정의 출처가 사라진다 —
        #   약한 칸으로 내려간 것이 판단 품질을 바꿀 수 있고, 그러면 추적 가능해야 한다.
        "runner": dict(runner) if isinstance(runner, dict) else None,
        "runner_evidence": parse_runner_evidence(result),
        "control_status": result.get("status"),
        "reason_codes": result.get("reason_codes") or [],
        # 2026-10-01(plan_26100113 D2): 거부 상세(provider 가 output 머리에 실은 블록). 완수 리포트가 파싱되면
        #   reports 파일의 raw_output 은 비므로, 거부 사실을 남기는 자리는 여기뿐이다 — 숨기지 않는다.
        "permission_denials": _denial_block(result),
        "status": report.get("status"),
        "phase": report.get("phase"),
        # 2026-09-04(감사 D6): 서브가 회신한 정체성을 **그대로** 남기고 대조는 소비자가 한다.
        "context_id_reported": report.get("context_id"),
        # 2026-09-07(plan_26090715 §4.8): 캠페인 정체성 echo. 대조는 소비자(--continue)가 하며
        #   **부재도 불일치도 STOP** 이다 — 종전 `if 값 and …` 는 부재를 침묵 통과시켰다.
        "campaign_id_reported": report.get("campaign_id"),
        # 2026-09-07(유예 결함 ⑦): 이 턴이 끝난 시점에 아직 도는 서비스. `None` = 서브가 말하지
        #   않았다(모름) · `[]` = 없다는 **선언**. 둘을 같은 값으로 접으면 고아 컨테이너가
        #   "없음" 으로 보인다.
        "running_services": report.get("running_services"),
        "control_variables_echo": report.get("control_variables_echo"),
        # B안(2026-09-04 사용자 결정): 서브가 직접 수행한 외부검색 이력. 이것이 남아야 B안은
        #   권한 확대가 아니라 **자산화 경로**가 된다(plan §6.1).
        "external_search": report.get("external_search") or [],
        # SILENT_FALLBACK 금지: 메인이 대신 한 것은 여기에 적히지 않는다. 서브 산출만 집계한다.
        "sub_reported": bool(report),
        # 2026-09-29(plan_26092917_59_03 S4 선행): 감독이 **전진**을 원장만으로 읽을 수 있게 리포트의
        #   진행 사실을 옮겨 둔다. 종전 전진 신호는 phase 변화와 회수 브리핑 둘뿐이었는데, 멀티 Ray
        #   워커는 캠페인 인스턴스가 없어 브리핑을 쓸 수 없고 비서빙 과업은 phase 가 움직이지 않는다
        #   — 그래서 멀티 서브의 중단은 **구조적으로** 전부 '정체' 팝업이었다.
        #   `None` = 서브가 말하지 않았다(모름) · `[]` = 없다는 선언. 옛 원장은 필드가 없어 종전대로 읽힌다.
        "next_steps": report.get("next_steps") if report else None,
        "artifact_paths": ([a.get("path") for a in report.get("artifacts") or []
                            if isinstance(a, dict) and a.get("path")] if report else None),
        "hitl_needed": bool((report.get("hitl") or {}).get("needed")) if report else None,
        # 2026-09-29(plan_26092919 P2): 서브의 예산 **제안**. 배정은 메인이 하지만(comms 계약), 제안을
        #   원장에 옮기지 않으면 감독 재발급이 그것을 볼 자리가 없다 — 라이브에서 서브가 "600s 이상
        #   필요" 를 남겼는데 감독은 서빙 phase 실측 228s 로 같은 벽을 다시 열었다.
        "budget_recommendation": (report.get("budget_recommendation")
                                  if report and isinstance(report.get("budget_recommendation"), dict)
                                  else None),
    }
    att["end_reason"] = end_reason(att)
    recovery = codex_recovery_requirement(att)
    if recovery:
        att["recovery_required"] = recovery
    doc["context_id"] = context_id
    doc.setdefault("attempts", []).append(att)
    return att


def entry_blocking(entry: dict) -> bool:
    """이 대기 항목이 **차단성**인가 — 저장된 필드가 아니라 요청 본문에서 파생한다.

    2026-09-04: 처음에는 표면화 시점에 `blocking` 을 계산해 항목에 적었다. 그런데 그러면 같은
    개념이 두 자리(요청 본문 · 항목 필드)에 앉고, 실제로 **옛 형식으로 쓰인 라이브 항목을 읽지
    못했다**(요청 본문에는 차단성이 있는데 항목에는 필드가 없어 비차단으로 읽혔다).
    `workflow.md` §4종 안티패턴의 "같은 개념이 두 곳 이상에 손으로 적힌 값" = 결함 칸이다.
    2026-10-19 · plan_26101912: 서고 교환은 은퇴 — 서브는 접근자로 직접 묻는다. 옛 항목에 남은 서고 요청 본문은 읽어도 차단성을 만들지 않는다 — 답할 주체가 없다.
    """
    return bool(entry.get("blocking"))


def sort_pending(pending: list) -> list:
    """대기 요청의 **처리 순서**. `blocking` 이 먼저, 그 안에서는 도착 순(attempt 오름차순).

    2026-09-04 신설(감사 B3 · 사용자 지목). 종전에는 우선순위 개념이 **어디에도 없었다** —
    도서관 교환의 처리 순서는 `sorted(os.listdir(...))` = exchange_id 사전순이었고, 요청 스키마에
    priority/urgency 필드가 없었으며, 가장 가까운 `blocking` 은 **읽는 코드가 0** 이었다.
    즉 "서브가 막혔다" 는 선언이 아무것도 바꾸지 못했다.

    순서 키를 벽시계가 아니라 **attempt 번호**로 잡는다 — 이 저장소는 벽시계를 금지하고 주입만
    허용하는데(`docs.md` §기계판독 평면), attempt 는 이미 단조 증가하는 결정론 서수다.
    """
    return sorted(pending, key=lambda e: (not entry_blocking(e), e.get("attempt") or 0,
                                          e.get("request_id") or ""))


def surface_requests(repo_root: str, context_id: str, report: dict, *, attempt: int = 0,
                    crash: dict = None) -> str | None:
    path = os.path.join(relay_root(repo_root), PENDING_HITL)
    with state_lock(path):
        return _surface_requests_unlocked(repo_root, context_id, report, attempt=attempt, crash=crash)


def _surface_requests_unlocked(repo_root: str, context_id: str, report: dict, *, attempt: int = 0,
                    crash: dict = None) -> str | None:
    """서브가 스스로 표면화한 것을 릴레이한다(디스크 재스캔 ✗ — §2.4 push-attestation).

    2026-09-04 교정:
      · **A4(해소 미반영)**: 종전에는 `needed=false` 인 턴이 정리 필터에 **도달조차 못 해**, 이미
        해소된 질문이 계속 남았다(라이브 잔존 실측). 이제 어떤 경우에도 이 context 의 낡은 항목을
        먼저 걷어낸다.
      · **B3(우선순위)**: `blocking` 을 읽어 정렬 키로 쓴다.
    2026-10-19 · plan_26101912: 서고 교환은 은퇴 — 서브는 접근자로 직접 묻는다. 표면화 표지는 `hitl.needed` 하나다 — 리포트에 옛 서고 요청 배열이 실려 와도 읽지 않는다.
    사람이 적어 넣은 `answer` 는 같은 `request_id` 로 **이월**한다 — 재표면화가 답을 지우면
    사람이 두 번 답해야 하고, 그것은 릴레이가 사람의 일을 늘리는 것이다.
    """
    report = report or {}
    hitl = report.get("hitl") or {}
    path = os.path.join(relay_root(repo_root), PENDING_HITL)
    doc = {"schema_version": 1, "pending": []}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
    previous = {e.get("request_id"): e for e in doc.get("pending", [])
                if e.get("context_id") == context_id}
    prior = {rid: e.get("answer") for rid, e in previous.items()}
    doc["pending"] = [e for e in doc.get("pending", []) if e.get("context_id") != context_id]

    entry = None
    recovery = (crash or {}).get("recovery_required")
    if recovery:
        rid = "relay-recovery-a%02d" % attempt
        old = previous.get(rid) or {}
        # Never transfer an answer to a different dispatch, even if a damaged
        # ledger reuses its attempt number or the sub repeats a question ID.
        answer = old.get("answer") if old.get("recovery_required") == recovery else None
        entry = {"context_id": context_id, "attempt": attempt, "request_id": rid,
                 "source": "relay-control-recovery", "blocking": True,
                 "recovery_required": recovery, "answer": answer,
                 "prompt": (f"Codex execution remains unresolved ({recovery['kind']}; "
                            f"signal={recovery['signal']}). Inspect the preserved outcome and effects. "
                            "Record the human recovery decision in answer, then explicitly use "
                            "--continue --apply --resume <session_id|new>. A signal does not identify "
                            "who stopped the process, and this answer grants no additional tool permissions."
                            + ("\nSub request: " + str(hitl.get("prompt")) if hitl.get("needed") else ""))}
        doc["pending"].append(entry)
    elif crash and not hitl.get("needed"):
        # 2026-09-05(3-3): 리포트가 없거나 제어가 실패한 턴. 서브가 요청을 남기지 못한 상태이므로
        #   **메인이 사실만** 표면화한다 — 재개 여부는 사람·에이전트가 `--resume` 로 선언한다.
        entry = {
            "context_id": context_id,
            "attempt": attempt,
            "request_id": "relay-crash-a%02d" % attempt,
            "source": "relay-crash",
            # 2026-09-08: 사유가 다르면 문구도 달라야 한다 — 러너 사다리 소진을 "리포트 없이
            #   끝났다" 로 적으면 사람이 서브의 과업을 들여다보게 되고, 실제 원인(실행자 평면)은
            #   화면 밖에 남는다. 호출자가 문구를 주면 그것을 쓴다.
            "prompt": crash.get("prompt") or (
                      "attempt %s 가 리포트 없이 끝났다(end_reason=%s · control=%s · %s). "
                      "마지막 알려진 세션 = %s. 이어받으려면 `--resume <session_id>`, "
                      "새로 열려면 `--resume new` 를 선언하라."
                      % (crash.get("attempt"), crash.get("end_reason"), crash.get("control_status"),
                         ",".join(crash.get("reason_codes") or []) or "코드 없음",
                         crash.get("last_known_session") or "(없음)")),
            # 차단성은 **호출자가 선언**한다. 러너 소진은 답 없이 진행하면 같은 벽에 다시 닿는다.
            **({"blocking": True} if crash.get("blocking") else {}),
            "answer": prior.get("relay-crash-a%02d" % attempt),
        }
        doc["pending"].append(entry)
    elif hitl.get("needed"):
        rid = hitl.get("request_id")
        entry = {
            "context_id": context_id,
            "attempt": attempt,
            "request_id": rid,
            "source": hitl.get("source") or "sub-relay",
            "prompt": hitl.get("prompt"),
            # 차단성은 hitl 절이 스스로 말한 경우만 항목에 남긴다(entry_blocking 이 읽는 유일한 자리).
            **({"blocking": True} if hitl.get("blocking") else {}),
            "answer": prior.get(rid),
        }
        doc["pending"].append(entry)
    doc["pending"] = sort_pending(doc["pending"])
    _atomic_json(path, doc)
    return path if entry else None


def pending_for(repo_root: str, context_id: str) -> list:
    """이 context 의 대기 요청(우선순위 순). `--continue` 가 읽는다 — **쓰기만 하던 파일에 소비자가 생긴다**."""
    path = os.path.join(relay_root(repo_root), PENDING_HITL)
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    return sort_pending([e for e in doc.get("pending", []) if e.get("context_id") == context_id])


def dispatch_crash(att: dict, report: dict | None) -> dict | None:
    recovery = codex_recovery_requirement(att)
    if not recovery and report and (att.get("control_status") == "completed"
                                   or att.get("end_reason") == "completed_with_denials"):
        return None
    return {"attempt": att["attempt"], "end_reason": att["end_reason"],
            "control_status": att.get("control_status"), "reason_codes": att.get("reason_codes"),
            "last_known_session": att.get("session_id"),
            **({"recovery_required": recovery, "blocking": True} if recovery else {})}


def ensure_recovery_pending(repo_root: str, doc: dict) -> list:
    """Repair the ledger->pending publication gap without launching any work.

    The ledger remains authoritative if the producer died after committing the
    outcome but before publishing its blocking question. Deleting pending cannot
    turn a persisted unresolved failure into permission to retry.
    """
    pending = pending_for(repo_root, doc.get("context_id"))
    last = (doc.get("attempts") or [None])[-1]
    recovery = codex_recovery_requirement(last) if last else None
    if recovery and not any(e.get("recovery_required") == recovery and entry_blocking(e)
                            for e in pending):
        report = load_report(repo_root, last)
        surface_requests(repo_root, doc["context_id"], report or {}, attempt=last["attempt"],
                         crash=dispatch_crash(last, report))
        pending = pending_for(repo_root, doc["context_id"])
    return pending


def require_dispatch_recovery(a, doc: dict, resume_declared) -> None:
    """Both direct and supervised dispatch pass the same persisted recovery latch."""
    pending = ensure_recovery_pending(a.repo_root, doc)
    if resume_blockers(pending):
        raise SystemExit("[relay] STOP: unanswered blocking HITL request; record the human answer before dispatch")
    last = (doc.get("attempts") or [None])[-1]
    recovery = codex_recovery_requirement(last) if last else None
    manual = bool(getattr(a, "continue_", False) and getattr(a, "apply", False)
                  and isinstance(getattr(a, "resume", None), str) and a.resume.strip()
                  and resume_declared == a.resume.strip())
    if recovery:
        answered = any(e.get("recovery_required") == recovery and entry_blocking(e)
                       and isinstance(e.get("answer"), str) and e["answer"].strip() for e in pending)
        if not answered or not manual:
            raise SystemExit("[relay] STOP: unresolved Codex failure requires its human recovery answer "
                             "and explicit --continue --apply --resume <session_id|new>; automatic replay is forbidden")
    reasons = continuation_stop_reasons(doc, allow_control_recovery=bool(recovery and manual),
                                        historical_control_variables=getattr(a, "_historical_control_variables", None))
    if reasons:
        raise SystemExit("[relay] STOP: " + "; ".join(reasons))


def parse_report(output: str):
    """서브 리포트는 JSON 1개다. 산문에 섞여 와도 마지막 JSON 객체를 집는다 — 못 찾으면 None.

    ★ 2026-10-01(plan_26100113 F7): 종전 판본은 괄호 깊이를 셌다. 그러면 앞에 **잘린 JSON** 이 하나 있을 때
      (권한 거부 상세는 도구 입력을 240자에서 자른다) 깊이가 0 으로 돌아오지 않아, 뒤따르는 **완전한 리포트**까지
      못 찾는다 — 라이브에서 completed 리포트가 "JSON 객체 없음" 으로 집계됐다. 이제 `{` 마다 디코딩을 시도하고,
      성공하면 그 객체 끝으로 건너뛴다(안쪽의 중첩 객체를 따로 집지 않는다). 추측 파싱은 여전히 없다 — 온전한 JSON
      객체만 집는다.
    """
    if not isinstance(output, str):
        return None
    dec = json.JSONDecoder()
    best, i = None, output.find("{")
    while i >= 0:
        try:
            obj, end = dec.raw_decode(output, i)
        except ValueError:
            i = output.find("{", i + 1)
            continue
        if isinstance(obj, dict):
            best = obj
        i = output.find("{", end)
    return best


def load_report(repo_root: str, att: dict):
    """원장 attempt 가 가리키는 서브 리포트 원본을 읽는다(없으면 None — 합성 ✗)."""
    rel = (att or {}).get("report_path")
    if not rel:
        return None
    path = os.path.join(repo_root, rel)
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as f:
        return (json.load(f) or {}).get("report")


def reuse_completed_handoff(a, doc: dict, lp: str) -> bool:
    """Accept a completed result without dispatching a verification worker.

    An observed handoff issue takes the existing explicit feedback path. This
    guard owns neither native execution outcomes nor a second completion ledger.
    """
    if getattr(a, "campaign_control", None) or not (doc.get("attempts") or []):
        return False
    last = doc["attempts"][-1]
    if (last.get("status") != "completed" or last.get("control_status") != "completed"
            or not last.get("sub_reported") or codex_recovery_requirement(last)):
        return False
    if getattr(a, "handoff_issue", None):
        return False
    # A declared new campaign input is actual new work, not a repeat check.
    prior_axis = last.get("control_variables_echo")
    if prior_axis is not None and doc.get("control_variables") != prior_axis:
        return False
    topology = getattr(a, "topology", None)
    if topology and topology != doc.get("topology"):
        raise SystemExit("[relay] HANDOFF_RESULT_SCOPE_MISMATCH: topology differs")
    manifest = getattr(a, "manifest", None)
    if manifest and os.path.realpath(manifest) != os.path.realpath(doc.get("manifest") or ""):
        raise SystemExit("[relay] HANDOFF_RESULT_SCOPE_MISMATCH: manifest differs")
    blockers = continuation_stop_reasons(doc)
    if resume_blockers(pending_for(a.repo_root, a.context_id)):
        blockers.append("existing blocking request remains")
    if blockers:
        raise SystemExit("[relay] HANDOFF_RESULT_UNAVAILABLE: " + "; ".join(blockers))
    try:
        expected = (Path(lp).resolve().with_suffix(".reports") /
                    ("attempt-%02d.json" % last["attempt"])).relative_to(Path(a.repo_root).resolve())
        if last.get("report_path") != expected.as_posix():
            raise ValueError("report does not belong to this context/attempt")
        stored = json.loads(_owned_artifact(a.repo_root, last["report_path"]).read_text(encoding="utf-8"))
        report = stored["report"]
        schema = _agent_control._load_schema(
            _agent_control.RUNTIME_DIR.parent / "sub_node" / "task-report.schema.json")
        if (_agent_control._schema_violations(report, schema) or report.get("status") != "completed"
                or report.get("context_id") != a.context_id
                or (doc.get("campaign_node") and report.get("node_id") != doc["campaign_node"])):
            raise ValueError("completed report scope/schema differs")
    except (OSError, ValueError, KeyError, TypeError, UnicodeError) as exc:
        raise SystemExit("[relay] HANDOFF_RESULT_UNAVAILABLE: " + str(exc)) from exc
    decision = {"action": "reuse_completed_result", "reason": "no_new_handoff_issue",
                "context_id": a.context_id, "source_attempt": last["attempt"],
                "report_path": last["report_path"], "execution_started": False,
                "continue_workflow": True}
    persist = not getattr(a, "emit_only", False) and (
        not getattr(a, "continue_", False) or getattr(a, "apply", False))
    if persist and doc.get("handoff_reuse") != decision:
        doc["handoff_reuse"] = decision
        save_ledger(lp, doc)
    print(json.dumps({**decision, "report": report}, ensure_ascii=False))
    return True


def _instruction_scope(doc: dict) -> dict:
    return {key: doc.get(key) for key in ("context_id", "campaign_id", "topology",
                                         "campaign_node", "campaign_context_kind", "campaign_cell",
                                         "control_variables")}


def _instruction_refusal(reason: str):
    # This is a main-owned evidence/technical handoff problem, not a fabricated
    # user answer or a Codex permission/control recovery event.
    raise SystemExit("[relay] TASK_INSTRUCTION_SOURCE_UNRESOLVED: " + reason
                     + "; main must review the issued requests and supply a complete explicit --task-file "
                       "under the existing dispatch guards; do not reset the context or budget")


def _instruction_request(repo_root: str, doc: dict, att: dict) -> dict:
    """Read an issued request bound to this ledger, never a caller-guessed task file."""
    number, context = att.get("attempt"), doc.get("context_id")
    if not isinstance(number, int) or isinstance(number, bool) or number < 1 or not isinstance(context, str) or not context:
        _instruction_refusal("instruction attempt/context is absent or malformed")
    expected = str(Path("campaigns") / (doc.get("campaign_id") or "_bootstrap") / "relay"
                   / (context + ".requests") / ("attempt-%02d.json" % number))
    if att.get("request_path") != expected:
        _instruction_refusal("instruction request path does not belong to this context/attempt")
    try:
        path = _owned_artifact(repo_root, expected)
        raw = path.read_bytes()
        request = json.loads(raw)
    except (OSError, ValueError, UnicodeDecodeError, SystemExit) as exc:
        _instruction_refusal("issued instruction request is unavailable: " + type(exc).__name__)
    digest = att.get("request_digest")
    if digest is not None:
        if hashlib.sha256(raw).hexdigest() != digest:
            _instruction_refusal("issued instruction request bytes changed")
    elif att.get("task_instruction") is not None:
        _instruction_refusal("issued instruction request digest is absent")
    else:
        # Pre-revision ledgers retain the same request in the durable dispatch
        # outcome. Require that independent persisted copy for legacy readback.
        try:
            outcome_path = _owned_artifact(repo_root, att.get("outcome_path") or "")
            wrapper = json.loads(outcome_path.read_bytes())
        except (OSError, ValueError, UnicodeDecodeError, SystemExit) as exc:
            _instruction_refusal("legacy instruction outcome is unavailable: " + type(exc).__name__)
        if (not isinstance(wrapper, dict) or wrapper.get("schema_version") != 1
                or wrapper.get("dispatch_id") != att.get("dispatch_id") or not att.get("dispatch_id")
                or wrapper.get("request") != request):
            _instruction_refusal("legacy instruction outcome does not match the issued dispatch")
    if (not isinstance(request, dict) or _agent_control.request_schema_violations(request)
            or request.get("intent") != "delegate"
            or permission_binding(request) != att.get("session_binding")
            or request.get("provider") != att.get("provider")
            or request.get("campaign_id") != doc.get("campaign_id")
            or request.get("control_variables") != doc.get("control_variables")
            or not isinstance(request.get("task"), str)):
        _instruction_refusal("issued instruction target/provider/campaign binding differs")
    return request


def continuation_instruction(repo_root: str, doc: dict) -> dict:
    """Latest explicit full task; assembled continuations never become task revisions."""
    attempts = doc.get("attempts") or []
    if not attempts:
        _instruction_refusal("there is no issued task")
    if any(not isinstance(a, dict) or a.get("attempt") != i for i, a in enumerate(attempts, 1)):
        _instruction_refusal("instruction attempt sequence is malformed")
    latest = attempts[-1]
    instruction = latest.get("task_instruction")
    if instruction is None:
        # Old records did not distinguish explicit feedback from assembled text.
        # A multi-attempt history cannot safely be guessed from prompt wording.
        if len(attempts) != 1:
            _instruction_refusal("legacy multiple attempts have no explicit task-source declaration")
        request = _instruction_request(repo_root, doc, latest)
        body = doc.get("task")
        if not isinstance(body, str) or not body or not request["task"].endswith(body):
            _instruction_refusal("legacy initial task does not match its issued request")
        return {"attempt": 1, "body": body, "request_path": latest["request_path"], "legacy": True}
    if not isinstance(instruction, dict):
        _instruction_refusal("instruction metadata is malformed")
    source = instruction.get("source_attempt")
    if not isinstance(source, int) or isinstance(source, bool) or not 1 <= source <= len(attempts):
        _instruction_refusal("instruction source attempt is invalid")
    origin = attempts[source - 1]
    declared = origin.get("task_instruction")
    if declared is None:
        if source != 1:
            _instruction_refusal("a legacy feedback attempt cannot be selected implicitly")
        resolved = continuation_instruction(repo_root, {**doc, "attempts": [origin]})
    else:
        if (not isinstance(declared, dict) or declared.get("schema_version") != 1
                or declared.get("kind") != "explicit" or declared.get("source_attempt") != source
                or declared.get("scope") != _instruction_scope(doc)):
            _instruction_refusal("explicit task source scope/kind is invalid")
        body = declared.get("body")
        request = _instruction_request(repo_root, doc, origin)
        if (not isinstance(body, str) or not body or hashlib.sha256(body.encode()).hexdigest() != declared.get("body_digest")
                or not request["task"].endswith(body)):
            _instruction_refusal("explicit task body differs from its issued request")
        resolved = {"attempt": source, "body": body, "request_path": origin["request_path"], "legacy": False}
    for att in attempts[source:]:
        ref = att.get("task_instruction")
        if (not isinstance(ref, dict) or set(ref) != {"schema_version", "kind", "source_attempt", "scope"}
                or ref.get("schema_version") != 1 or ref.get("kind") != "continuation"
                or ref.get("source_attempt") != source or ref.get("scope") != _instruction_scope(doc)):
            _instruction_refusal("later task source is missing, stale, or ambiguous")
        request = _instruction_request(repo_root, doc, att)
        if request["target"] != origin["session_binding"]["target"]:
            _instruction_refusal("continuation target differs from its instruction source")
    return resolved


def assemble_continuation(repo_root: str, doc: dict, pending: list) -> str:
    """이어붙일 task 본문을 **기계가** 조립한다 — 이것이 없어서 자율 재개가 성립하지 않았다.

    2026-09-04 신설(감사 D1 · 심각도 1). 종전에는 소진 시 `relay.py` 가 *"다음 attempt 는
    max_turns=N 으로 열고 직전 산출물을 prompt 로 실어라"* 라는 **산문 안내를 출력하고 죽었고**,
    그 안내를 받아 다시 부르는 코드가 저장소에 0건이었다. 즉 "이어붙일 수 있는 상태" 는 남지만
    "스스로 잇는 루프" 가 없었다.

    조립 원칙 — **합성하지 않는다**. 여기 들어가는 것은 전부 이미 기록된 사실뿐이다:
      · 직전 attempt 의 제어 상태(원장)
      · 서브가 보낸 리포트의 `artifacts[]`·`next_steps`·`notes`(서브가 쓴 그대로)
      · 사람이 `pending_hitl.json` 에 적어 넣은 `answer`(사람이 쓴 그대로)
      · 실제 발급 증거에 결속된 최신 명시 지시(최초 원문은 이력으로 보존)
    메인이 추측한 진행상황을 여기에 적으면 그것이 SILENT_FALLBACK 이다.
    """
    instruction = continuation_instruction(repo_root, doc)
    atts = [a for a in (doc.get("attempts") or []) if _reached_sub(a)]
    last = atts[-1] if atts else None
    report = load_report(repo_root, last)
    out = ["## 이어받기 — 아래는 메인이 원장에서 **그대로 옮긴** 사실이다(추측 없음).", ""]
    if last:
        out.append(f"- 직전 attempt {last['attempt']}: status={last.get('status')} · "
                   f"phase={last.get('phase')} · turns={last.get('max_turns_used')}/"
                   f"{last.get('max_turns_allocated')} · budget={last.get('budget_outcome')}")
        if last.get("budget_outcome") == "exhausted":
            out.append("- ⚠ 직전 attempt 는 **예산 소진**으로 끊겼다. 같은 일을 처음부터 하지 말고 "
                       "아래 산출물을 신뢰해 그 다음부터 이어라.")
    if report is None:
        out.append("- ⚠ 직전 리포트(JSON)가 없다 — 서브가 산문만 보냈거나 전송이 실패했다. "
                   "무엇이 끝났는지 **실물로 확인한 뒤** 진행하라.")
    for art in (report or {}).get("artifacts") or []:
        out.append(f"  - 산출물[{art.get('kind')}]: {art.get('path')}")
    for step in (report or {}).get("next_steps") or []:
        out.append(f"  - 직전 턴이 남긴 다음 단계: {step}")
    if (report or {}).get("notes"):
        out.append(f"  - 직전 턴 메모: {report['notes']}")

    answered = [e for e in pending if e.get("answer")]
    if answered:
        out += ["", "## 네 질문에 대한 메인의 답"]
        for e in answered:
            if e.get("prompt"):
                out.append(f"- (질문) {e['prompt']}")
            out.append(f"- **(답)** {e['answer']}")
    open_nonblocking = [e for e in pending if not e.get("answer") and not entry_blocking(e)]
    if open_nonblocking:
        out += ["", "## 아직 답이 없는 비차단 질문(진행을 막지 않는다 — 없이 갈 수 있으면 가라)"]
        for e in open_nonblocking:
            out.append(f"- {e.get('prompt') or '(질문 문구 없음 · request_id=%s)' % e.get('request_id')}")
    out += ["", "- 이번 attempt 의 시간·턴이 남은 일에 모자라 보이면, 필요한 값을 산문이 아니라 리포트 "
            "`budget_recommendation{max_turns,timeout_seconds,why}` 에 **수치로** 적어라 — 감독 재발급은 그 필드만 읽는다."]
    out += ["", "## 현재 적용할 명시 지시", "",
            f"- 원문 출처: attempt {instruction['attempt']} · {instruction['request_path']}",
            "- 최초 지시는 원장 이력에 보존돼 있다. 이번 재개에는 아래 최신 명시 원문을 적용한다.",
            "", instruction["body"]]
    return "\n".join(out) + "\n"


# ─────────────────────────────────────────────────────────────────────────────
# 감독 스텝 (2026-09-08 신설 · plan_26090813 §4.3 · 사용자 결정 D7·D9·D16·D21)
#
# 왜 데몬이 아닌가: 사용자는 "메인노드의 세션 리볼빙 방식을 응용" 하라고 정했다. 감독자는 상주
# 프로세스가 아니라 **아티팩트에서 복원되는 한 걸음**이다 — 원장과 회수된 브리핑을 읽고, 한 번
# 판정하고, 그 판정을 원장에 적고 끝난다. 깨우는 손은 하네스의 예약 wakeup 이거나 다음 세션의
# 재개다. 상주하지 않으므로 세션이 끊겨도 잃는 것이 없다.
#
# ★ 이 자리가 비어 있어서 2026-09-07 에 메인이 서브를 ssh 로 32회 직접 관측했다(헌법 노드제어 ①
#   위반). 감독자의 주기 읽기는 **승인된 attempt 에 종속된 관측**이지 새 트리거가 아니다.

SUPERVISOR_ACTIONS = ("await_dispatch", "next_cell", "resume", "popup", "idle")


def _last_reached(doc: dict) -> dict | None:
    return next((x for x in reversed(doc.get("attempts") or []) if _reached_sub(x)), None)


def continuation_stop_reasons(doc: dict, *, allow_control_recovery: bool = False,
                              historical_control_variables=None) -> list[str]:
    """Manual and supervised resumes respect the same identity/resource boundaries."""
    if doc.get("inflight"):
        return ["unresolved in-flight dispatch; reconcile before replay"]
    latest = (doc.get("attempts") or [None])[-1]
    reasons = []
    if latest and codex_recovery_requirement(latest) and not allow_control_recovery:
        reasons.append("unresolved Codex control outcome requires a human answer and explicit manual resume")
    last = _last_reached(doc)
    if last is None:
        return reasons
    reasons += echo_stop_reasons(last, context_id=doc.get("context_id"),
                                campaign_id=doc.get("campaign_id"),
                                control_variables=(historical_control_variables if historical_control_variables is not None
                                                   else doc.get("control_variables")))
    if isinstance(last.get("running_services"), list) and last["running_services"]:
        reasons.append("previous attempt left running services; use explicit A2A cleanup or a declared new context")
    return reasons


def budget_yield(att: dict) -> bool:
    """`input-required` 가 **질문**이 아니라 **예산 양보**인가 — 원장에 옮긴 리포트 사실에서 파생.

    서브 계약은 "예산이 모자라 보이면 소진하지 말고 input-required 로 끊고 남은 일을 next_steps 에
    적어라" 이다. 그런데 감독은 input-required 를 전부 '답이 필요하다' 팝업으로 읽어, 계약대로
    양보한 서브가 매번 사람을 불렀다(2026-09-29 발견 · 멀티 AC5 구조적 불가). 질문은 표지가 있다 —
    `hitl.needed`. 그것이 없고 남은 일이 적혀 있으면 양보다.
    필드가 없는 옛 원장(None)은 양보로 읽지 않는다 — 모르면 묻는다.
    2026-10-19 · plan_26101912: 서고 교환은 은퇴 — 서브는 접근자로 직접 묻는다. 서고 질문으로 끊는 턴은 더 이상 없다(서브가 자기 턴 안에서 답을 얻는다) — 옛 원장의
    서고 차단 필드는 읽지 않는다.
    """
    return att.get("hitl_needed") is False and bool(att.get("next_steps"))


def ledger_progress(last: dict, prev: dict | None) -> str | None:
    """원장만으로 본 전진. 사유 문자열 또는 None. 값을 만들지 않고 두 attempt 의 기록만 비교한다.

    · 직전이 있으면: 새 산출물 경로가 생겼거나 남은 일(next_steps)이 바뀌었다.
    · 첫 중단이면: 이 attempt 가 턴을 쓰고 산출물 또는 남은 일을 남겼다(일한 흔적).
    같은 남은 일·같은 산출물이 반복되면 전진이 아니다. 판정 불가 중단은 이 전진 판정과
    잔존 서비스 가드를 따른다.
    """
    arts, steps = last.get("artifact_paths"), last.get("next_steps")
    if arts is None and steps is None:
        return None
    if prev is None:
        if (last.get("max_turns_used") or 0) > 0 and (arts or steps):
            return "첫 중단 · 산출물/남은 일 기록"
        return None
    new_arts = set(arts or []) - set(prev.get("artifact_paths") or [])
    if new_arts:
        return f"새 산출물 {len(new_arts)}건"
    if steps is not None and prev.get("next_steps") is not None and steps != prev.get("next_steps"):
        return "남은 일 변화"
    return None


# 노드 간 시계 오차 여유(초) — 이 파일에서만 쓰는 국소 상수. 서브 mtime 은 서브 시계이고
# attempt 창은 메인 시계다.
DOC_PLANE_SKEW_S = 120


def _utc_epoch(v) -> float | None:
    try:
        return datetime.datetime.strptime(str(v), "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=datetime.timezone.utc).timestamp()
    except (TypeError, ValueError):
        return None


def doc_plane_progress(repo_root: str, doc: dict) -> str | None:
    """회수 미러의 **문서**로 본 전진 — 리포트 없이 잘린 attempt 를 위해.

    2026-09-29(plan_26092919 P1 · R3-ⓑ): CLI 턴 상한에서 잘린 attempt 는 리포트가 없어 원장 전진이
    0 이었다. 그런데 서브 문서 평면에는 완성 문서가 있었다. 두 조건이 **모두** 참인 문서만 센다 —
    ① 본문 앞부분에 이 context_id 가 있다(다른 context 문서 ✗) ② mtime 이 마지막 attempt 창 안이다
    (지난 attempt 의 문서 ✗). 미러(`fetch_sub_docs.sh` · rsync -a 로 mtime 보존)만 읽는다 — 서브를
    직접 읽으면 무단 스캔이다. 미러가 없거나 창을 모르면 None(모르면 묻는다).
    """
    last, ctx = _last_reached(doc), doc.get("context_id")
    if not last or not ctx:
        return None
    t0, t1 = _utc_epoch(last.get("started_utc")), _utc_epoch(last.get("ended_utc"))
    if t0 is None or t1 is None:
        return None
    root = os.path.join(repo_root, "sync_staging", "sub_docs")
    pat = re.compile(r"(?<![\w-])" + re.escape(str(ctx)) + r"(?![\w-])")
    hits = []
    for dp, dns, fns in os.walk(root):
        if os.path.relpath(dp, root).split(os.sep)[0] == "logs":   # 기계판독 평면은 문서가 아니다
            dns[:] = []
            continue
        for fn in fns:
            if not fn.endswith(".md"):
                continue
            fp = os.path.join(dp, fn)
            try:
                mt = os.path.getmtime(fp)
                if not (t0 - DOC_PLANE_SKEW_S <= mt <= t1 + DOC_PLANE_SKEW_S):
                    continue
                with open(fp, encoding="utf-8", errors="replace") as fh:
                    head = fh.read(8192)
            except OSError:
                continue
            if pat.search(head):
                hits.append(os.path.relpath(fp, root))
    return f"문서 평면 {len(hits)}건({sorted(hits)[0]} 등)" if hits else None


def supervise_decide(doc: dict, *, brief: dict | None = None,
                     cost_cap_attempts: int | None = None, ladder: list | None = None,
                     doc_progress: str | None = None) -> dict:
    """원장 하나의 **다음 한 걸음**을 판정한다. 값을 만들지 않고 기록된 사실만 읽는다.

    전진의 정의는 둘이다 — phase 가 바뀌었거나(원장), 회수된 브리핑의 `last_utc` 가 직전 감독
    스텝이 본 값보다 앞섰거나. 후자를 넣는 이유는 R2 다: 전진 신호를 phase 로만 잡으면 긴 벤치
    한 판이 통째로 '정체' 로 보인다.
    """
    if doc.get("inflight"):
        return {"action": "popup", "reason": "unresolved in-flight dispatch; reconcile before replay"}
    atts = doc.get("attempts") or []
    if not atts:
        return {"action": "await_dispatch",
                "reason": "이 context 에 아직 아무것도 나가지 않았다 — 지시서를 열어라"}
    recovery = codex_recovery_requirement(atts[-1])
    if recovery:
        deferred = doc.get("capacity_deferred") or {}
        if recovery.get("signal") == "provider_capacity" and deferred.get("dispatch_id") == atts[-1].get("dispatch_id"):
            return {"action": "resume", "reason": "capacity burst deferred; continue with a new bounded burst"}
        return {"action": "popup",
                "reason": f"unresolved Codex outcome ({recovery['kind']}; {recovery['signal']}); "
                          "human answer and explicit manual resume required; progress does not authorize replay"}
    # ★ 러너 판정은 `_last_reached` **앞**에 온다. 러너 실패는 정의상 서브에 닿지 않았으므로
    #   (`_reached_sub` 가 false) 뒤에 두면 "닿은 것이 없다" 분기가 먼저 삼켜 **회전이 도달
    #   불가**가 된다 — 자체검사가 이 순서를 첫 실행에서 잡았다(도달 불가 분기를 가드처럼 두지 마라).
    if (atts[-1] or {}).get("end_reason") == "runner_unavailable":
        if (atts[-1].get("runner_evidence") or {}).get("rotate") is not True:
            return {"action": "popup", "reason": "runner failure does not authorize rotation"}
        if atts[-1].get("execution_started") is True:
            return {"action": "popup", "reason": "runner failed after execution; reconcile effects first"}
        rungs = len(ladder or []) or 1
        burned = consecutive_runner_unavailable(doc)
        if burned < rungs:
            return {"action": "rotate_runner",
                    "reason": f"러너 평면 실패({burned}/{rungs}칸 소모) — 다음 칸으로 회전한다. "
                              f"예산도 범위도 바꾸지 않는다(바뀌는 것은 실행자 하나)"}
        return {"action": "popup",
                "reason": f"러너 사다리 소진 — 선언된 {rungs}칸이 연속 {burned}회 전부 실행되지 "
                          f"않았다. 서브가 기동되지 않는 것이고 예산·과업 문제가 아니다"}
    last = _last_reached(doc)
    if last is None:
        return {"action": "popup",
                "reason": "attempt 가 있으나 서브에 닿은 것이 없다(전송·기동 실패) — 통신 평면 확인"}
    reason = last.get("end_reason")
    if reason == "completed":
        return {"action": "next_cell", "reason": "직전 attempt 가 completed 다 — 이 셀은 끝났다",
                "cell": doc.get("campaign_cell")}
    if reason == "completed_with_denials":
        # 거부는 원장(`permission_denials`)에 남아 있다 — 완수를 막지 않되 다음 사람이 볼 수 있게 사유에 적는다.
        return {"action": "next_cell",
                "reason": "직전 attempt 가 completed 다(권한 거부 사건 동반 — 원장 permission_denials 참조) — 이 셀은 끝났다",
                "cell": doc.get("campaign_cell")}
    if reason == "permission_denied":
        # ★ 2026-09-08 라이브 교정: 종전 문구는 "모델·통신 평면이 깨졌다" 였는데, 실제로 일어난 것은
        #   **도구 호출 하나가 권한 평면에서 거부된 것**이었다(서브가 배달받은 스크립트를 grep 하려다).
        #   판정(팝업)은 맞았지만 사유가 과장되면 사람이 엉뚱한 곳을 본다 — 관측한 것만 적는다.
        return {"action": "popup",
                "reason": "권한 평면에서 거부된 호출이 있다 — 무엇이 막혔는지 사람이 봐야 한다"
                          "(리포트 raw_output 의 permission_denials 를 읽어라). 그냥 재개하면 "
                          "같은 벽에 다시 닿는다"}
    if reason in ("transport_or_launch_failure", "model_blocked",
                  "invalid_request", "malformed_output"):
        return {"action": "popup",
                "reason": f"모델·통신 평면이 깨졌다(end_reason={reason}) — 재개로 낫는 종류가 아니다"}
    if reason == "sub_input_required" and not budget_yield(last):
        return {"action": "popup",
                "reason": "서브가 input-required 로 끊었다 — 답이 필요하다(차단성이면 답이 승인이다)"}
    # ★ 2026-09-08 라이브: 계획은 자동 재개를 `external_interruption|budget_exhausted` 로만 적었다.
    #   그런데 실제로 3회 중 2회는 **제어가 completed 인데 서브 리포트가 기계판독 불가**여서
    #   `unclassified` 로 끝났다(서브가 산문으로 끝맺었다). 그 상태를 매번 사람에게 올리면 사용자
    #   불명 중단은 보고서 형식만으로 성공을 추정하지 않는다. `unclassified` 는
    #   **전진이 관측되면** 잇고,
    #   전진이 없으면 그때 묻는다. 판정 불가는 추측의 근거가 아니라 관측을 볼 이유다.
    if reason == "budget_exhausted":
        return {"action": "resume", "reason": "native invocation budget exhausted; reconcile and resume"}
    if (reason == "external_interruption" and
            (last.get("failure_evidence") or {}).get("signal") in ("controlled_local_deadline", "controlled_remote_deadline")):
        return {"action": "resume", "reason": "controlled local invocation deadline; reconcile and resume"}
    if reason in ("external_interruption", "budget_exhausted", "unclassified", "sub_input_required"):
        # 여기 도달한 sub_input_required 는 위에서 걸러진 **예산 양보**뿐이다(질문이 아니다).
        prev = [a for a in (doc.get("attempts") or []) if _reached_sub(a)][:-1]
        moved_phase = bool(prev) and prev[-1].get("phase") != last.get("phase")
        seen = doc.get("supervisor_last_seen_utc")
        moved_brief = bool(brief and brief.get("last_utc")
                           and (not seen or str(brief["last_utc"]) > str(seen)))
        moved_ledger = ledger_progress(last, prev[-1] if prev else None)
        if moved_phase or moved_brief or moved_ledger or doc_progress:
            why = ("phase 전진" if moved_phase
                   else f"브리핑 last_utc 전진({brief.get('last_utc')})" if moved_brief
                   else f"원장 전진({moved_ledger})" if moved_ledger
                   else doc_progress)
            if reason == "sub_input_required":
                reason = "sub_input_required(예산 양보)"
            note = (" · 종료 사유는 판정 불가지만 일한 흔적이 있다"
                    if reason == "unclassified" else "")
            return {"action": "resume",
                    "reason": f"{reason} 이지만 전진이 보인다({why}){note} — 자동 재발급(D9: 항상 자동)"}
        return {"action": "popup",
                "reason": f"{reason} 이고 전진이 없다 — 같은 벽에 부딪히는 중이다. "
                          f"예산을 키우기 전에 사람에게 묻는다"}
    return {"action": "popup", "reason": f"처음 보는 종료(end_reason={reason}) — 모르면 묻는다"}


def record_supervisor_step(doc: dict, decision: dict, *, utc: str,
                           brief: dict | None = None) -> dict:
    """감독 판정을 원장에 **덧붙인다**(append-only). 판정이 남지 않으면 다음 스텝이 같은 것을
    다시 판정하고, 사람은 감독자가 무엇을 보고 그랬는지 알 수 없다."""
    entry = dict(decision)
    entry["utc"] = utc
    entry["brief_last_utc"] = (brief or {}).get("last_utc")
    doc.setdefault("supervisor_steps", []).append(entry)
    if (brief or {}).get("last_utc"):
        doc["supervisor_last_seen_utc"] = brief["last_utc"]
    return entry


def read_brief(repo_root: str, node: str | None) -> dict | None:
    """회수된 서브 브리핑. **미러 밖은 보지 않는다** — 서브를 직접 읽으면 그것이 무단 스캔이다."""
    if not node:
        return None
    path = os.path.join(repo_root, "sync_staging", "sub_docs", "logs", node,
                        "campaign_brief.json")
    if not os.path.isfile(path):
        return None
    try:
        with open(path, encoding="utf-8") as fh:
            doc = json.load(fh)
    except (OSError, ValueError):
        return None
    return doc if isinstance(doc, dict) else None


def campaign_ledgers(repo_root: str, camp: str) -> list:
    """이 캠페인의 릴레이 원장(캠페인 축이 못박힌 것만). 이름으로 추론하지 않는다."""
    root = os.path.join(repo_root, "campaigns", camp, "relay")
    out = []
    for name in sorted(os.listdir(root)) if os.path.isdir(root) else []:
        if not name.endswith(".json") or name == PENDING_HITL:
            continue
        path = os.path.join(root, name)
        try:
            with open(path, encoding="utf-8") as fh:
                doc = json.load(fh)
        except (OSError, ValueError):
            continue
        if isinstance(doc, dict) and doc.get("campaign_node"):
            out.append((path, doc))
    return out


def _lifecycle_state(repo_root: str, camp: str) -> str:
    """수명 fold 의 단일 소유는 campaign_init 이다 — 여기서 이벤트를 다시 해석하지 않는다."""
    try:
        ci = _campaign_init()
        return ci.fold_lifecycle(ci.Path(repo_root) / "campaigns" / camp,
                                 active=ci.active_campaign_id(repo_root))["state"]
    except Exception as exc:                                            # noqa: BLE001
        return f"unknown(fold 실패: {exc})"


def supervise_step(a) -> int:
    """감독 한 걸음. 상주하지 않고, 판정하고, 적고, 끝난다."""
    camp = a.supervise_step
    ledgers = campaign_ledgers(a.repo_root, camp)
    if not ledgers:
        print(f"[relay] 감독: campaigns/{camp}/relay 에 캠페인 축이 못박힌 원장이 없다 — "
              f"`--node`/`--context-kind` 를 선언하고 지시서를 열어라(이름 추론 ✗)")
        return 0
    utc = a.utc or _utcnow()
    rc = 0
    explicit_runner = bool(a.runners or a.backend or a.model or getattr(a, "provider", None))
    base_args = argparse.Namespace(**vars(a))
    for path, doc in ledgers:
        a = argparse.Namespace(**vars(base_args))
        if not explicit_runner and ledger_runner_spec(doc):
            a.ladder = resolve_ladder(a.repo_root, ledger_runner_spec(doc))
        node = doc.get("campaign_node")
        ensure_recovery_pending(a.repo_root, doc)
        brief = read_brief(a.repo_root, node)
        decision = supervise_decide(doc, brief=brief, cost_cap_attempts=a.cost_cap_attempts,
                                    ladder=getattr(a, "ladder", None),
                                    doc_progress=doc_plane_progress(a.repo_root, doc))
        if decision["action"] in ("resume", "rotate_runner"):
            deferred = doc.get("capacity_deferred") or {}
            last = (doc.get("attempts") or [None])[-1] or {}
            allow_capacity = (deferred.get("dispatch_id") == last.get("dispatch_id") and
                              (last.get("failure_evidence") or {}).get("signal") == "provider_capacity")
            reasons = continuation_stop_reasons(doc, allow_control_recovery=allow_capacity)
            if reasons:
                decision = {"action": "popup", "reason": "; ".join(reasons)}
        # 수명 가드(2026-10-17 · plan_26101719 D4·D13): 감독은 id 를 직접 받으므로 ACTIVE 를 거치지 않는다. 보류·미결·잔재
        # 캠페인에서 전진 판정을 내면 보류한 캠페인이 감독 손에 다시 굴러가고, **popup 을 내면 리볼빙마다 같은 것을 묻는다**
        # (라이브 E4: 대체된 첫 위임의 sub_failed 원장이 감독 스텝마다 popup 을 띄웠다). 보류 캠페인은 계기(a)(d) 전까지
        # 묻지 않는다 — 판정 전부를 hold 로 바꾸고, 원래 판정은 사유에 남긴다(다시 활성이 되면 그때 본다).
        _life = _lifecycle_state(a.repo_root, camp)
        if _life != "active":
            decision = {"action": "hold",
                        "reason": f"캠페인 {camp} 은 {_life} 다 — 감독 판정({decision['action']}: "
                                  f"{decision['reason'][:80]})을 보류한다. 묻지도 잇지도 않는다(계기 a·d 전까지 · D4). "
                                  f"재개는 사람 결정(`campaign_init.py --resume-campaign {camp}`) · 정상 차단"}
        record_supervisor_step(doc, decision, utc=utc, brief=brief)
        save_ledger(path, doc)
        head = (f"[relay] 감독 · {doc.get('context_id')} (node={node} "
                f"kind={doc.get('campaign_context_kind')} cell={doc.get('campaign_cell')})")
        print(f"{head}\n  → {decision['action']}: {decision['reason']}")
        if decision["action"] == "popup":
            rc = max(rc, 3)
            print("  → 이것은 **질문이지 차단이 아니다**. 사람이 답하면 그대로 잇는다"
                  f"(pending: {os.path.join(relay_root(a.repo_root), PENDING_HITL)}).")
        elif decision["action"] == "next_cell":
            print("  → 다음 셀의 지시서는 **새 context** 로 연다(셀 하나 = context 하나 · D20). "
                  "모드가 HITL 이면 열기 전에 사람에게 묻는다.")
        elif decision["action"] == "rotate_runner":
            if not a.apply:
                print("  → 회전은 `--supervise-step <camp> --apply` 로 실행한다(단일 스텝).")
            else:
                rc = max(rc, _supervise_resume(a, path, doc, None,
                                               "러너 회전; context의 호출 증거로 예산 선택"))
        elif decision["action"] == "resume":
            secs, why = None, "context의 호출 증거로 예산 선택"
            if not a.apply:
                print("  → 재발급은 `--supervise-step <camp> --apply` 로 실행한다(단일 스텝).")
            else:
                rc = max(rc, _supervise_resume(a, path, doc, secs, why))
    return rc


def ledger_runner_spec(doc: dict) -> str | None:
    """원장이 **실제로 쓴** 러너 사다리(첫 등장 순서 · 이름). 재발급이 러너를 조용히 바꾸지 않게.

    2026-09-29(plan_26092919 P2) 라이브: 첫 위임은 `--runners sonnet-5.5`(모델 ID 고정)로 나갔는데
    `--supervise-step` 은 `--runners` 를 받지 않아 기본 사다리(`sonnet` 별칭)로 재발급했다 — 러너가
    선언 없이 바뀌었고, 별칭 해소가 달라지면 평가가 오염된다(조용한 대체). 명시가 없으면 원장을 잇는다.
    """
    if doc.get("runner_ladder"):
        return doc["runner_ladder"]
    # 옛 원장(선언 사다리 미기록): 그 context 를 **연** attempt 의 러너 한 칸만 잇는다. 이후 칸을
    #   합치면 이미 잘못 바뀐 러너가 '마지막 사용' 으로 사다리 커서를 끌고 간다(next_runner_index).
    first = next((a for a in doc.get("attempts") or [] if (a.get("runner") or {}).get("name")), None)
    return (first or {}).get("runner", {}).get("name") if first else None


def recommended_timeout(doc: dict) -> int | None:
    """마지막으로 닿은 attempt 의 서브 제안 timeout(초). 없거나 수가 아니면 None — 값을 만들지 않는다."""
    last = _last_reached(doc) or {}
    v = (last.get("budget_recommendation") or {}).get("timeout_seconds")
    return int(v) if isinstance(v, (int, float)) and not isinstance(v, bool) and v > 0 else None


def resume_blockers(pending: list) -> list:
    """자동 재발급을 막는 대기 항목 — 사람 답이 없는 차단성 요청.

    2026-10-19 · plan_26101912: 서고 교환은 은퇴 — 서브는 접근자로 직접 묻는다. 옛 항목에 남은 사서 반출 필드는 답이 아니다 — 차단을 풀지도 만들지도 않는다.
    """
    return [e for e in pending if entry_blocking(e) and not e.get("answer")]


def select_supervised_budget(a, doc: dict, fallback_seconds=None, fallback_reason="") -> dict:
    """Choose a fresh per-invocation budget from this context's recorded evidence."""
    attempts = doc.get("attempts") or []
    last = _last_reached(doc) or {}
    phase = last.get("phase")
    previous = next((x for x in reversed(attempts[:-1]) if _reached_sub(x)), None)
    progress = bool(last.get("sub_reported") and (
        (previous and phase and previous.get("phase") and previous.get("phase") != phase)
        or ledger_progress(last, previous)))
    rec = last.get("budget_recommended") if last.get("sub_reported") else None
    rec = rec if isinstance(rec, dict) else {}
    providers = {r.get("provider", "claude_code") for r in a.ladder}
    selected_runner = a.ladder[next_runner_index(doc, a.ladder)]
    selected_provider = selected_runner.get("provider", "claude_code")
    selected_model = selected_runner.get("model")
    reasons = []

    def positive(value):
        return value if isinstance(value, int) and not isinstance(value, bool) and value > 0 else None

    def choose(field, explicit, rec_key, allocation_key, sample_key, exhausted):
        cap = turn_budget.schema_cap(field)
        if explicit is not None:
            reasons.append(f"{field}: caller explicit={explicit}")
            return explicit
        prior = positive(last.get(allocation_key))
        recommendation = positive(rec.get(rec_key))
        if exhausted and prior:
            raw = max(math.ceil(prior * 1.5), recommendation or 0)
            chosen = min(raw, cap)
            reasons.append(f"{field}: observed exhaustion allocation={prior} × policy margin=1.5"
                           f" recommendation={recommendation} selected={chosen}"
                           + (f" schema ceiling={cap}; checkpoint continues" if raw > cap else ""))
            return chosen
        if recommendation and (not prior or recommendation >= prior or progress):
            chosen = min(recommendation, cap)
            reasons.append(f"{field}: sub recommendation={recommendation} progress={bool(progress)} selected={chosen}"
                           + (f" schema ceiling={cap}" if recommendation > cap else ""))
            return chosen
        def observed_value(item):
            value = positive(item.get(sample_key))
            if value:
                return value, sample_key
            if sample_key == "duration_ms":
                started, ended = _utc_epoch(item.get("started_utc")), _utc_epoch(item.get("ended_utc"))
                if started is not None and ended is not None and ended > started:
                    return int((ended - started) * 1000), "main_wall_ms"
            return None, None
        def observed_model_matches(item):
            observed = item.get("model_used")
            return (not observed or (isinstance(observed, list)
                    and all(isinstance(model, str) and model == selected_model for model in observed)))
        def successful_checkpoint(item):
            if (item.get("control_status") != "completed" or not item.get("sub_reported")
                    or item.get("budget_outcome") == "exhausted" or item.get("permission_denials")
                    or any(code in ("PERMISSION_DENIED", "TIMEOUT", "NONZERO_EXIT")
                           for code in item.get("reason_codes") or [])):
                return False
            return (item.get("status") == "completed" or
                    (item.get("status") == "input-required" and item.get("hitl_needed") is False))
        sample = next((x for x in reversed(attempts) if phase and x.get("phase") == phase
                       and successful_checkpoint(x)
                       and (x.get("provider") or (x.get("runner") or {}).get("provider")) == selected_provider
                       and (x.get("model_declared") or (x.get("runner") or {}).get("model")) == selected_model
                       and selected_model and observed_model_matches(x) and observed_value(x)[0]), None)
        if sample:
            observed, observed_source = observed_value(sample)
            raw = math.ceil(observed * 1.5 / (1000 if sample_key == "duration_ms" else 1))
            chosen = min(max(raw, 1), cap)
            if not prior or chosen >= prior or progress:
                reasons.append(f"{field}: same-context same-phase provider={selected_provider} "
                               f"declared_model={selected_model} attempt={sample.get('attempt')} "
                               f"observed {observed_source}={observed} × policy margin=1.5 selected={chosen}"
                               + (f" schema ceiling={cap}" if raw > cap else ""))
                return chosen
        if prior:
            reasons.append(f"{field}: last declared per-call allocation={prior}; no comparable observation")
            return prior
        raise SystemExit(f"[relay] STOP: no comparable per-call {field} evidence; declare it explicitly")

    timeout_signal = (last.get("failure_evidence") or {}).get("signal")
    secs = choose("timeout_seconds", getattr(a, "timeout_seconds", None), "timeout_seconds",
                  "timeout_seconds_allocated", "duration_ms",
                  timeout_signal in ("controlled_local_deadline", "controlled_remote_deadline"))
    turns = None
    if providers != {"codex_cli"}:
        turns = choose("max_turns", getattr(a, "max_turns", None), "max_turns",
                       "max_turns_allocated", "max_turns_used", last.get("budget_outcome") == "exhausted")
    source = "supervisor per-invocation: " + "; ".join(reasons)
    if providers == {"codex_cli"}:
        return turn_budget.declare(getattr(a, "max_turns", None), secs, source=source, unit="wall_time")
    return turn_budget.declare(turns, secs, source=source)


def _supervise_resume(a, lp: str, doc: dict, secs, why: str) -> int:
    """Resume the ledger context; a provider change starts a fresh native session."""
    a = argparse.Namespace(**vars(a))  # One context's permissions/budget must not leak to the next.
    last = (doc.get("attempts") or [None])[-1] or {}
    deferred = doc.get("capacity_deferred") or {}
    allow_capacity = (deferred.get("dispatch_id") == last.get("dispatch_id") and
                      (last.get("failure_evidence") or {}).get("signal") == "provider_capacity")
    reasons = continuation_stop_reasons(doc, allow_control_recovery=allow_capacity)
    if reasons:
        print("[relay] STOP: " + "; ".join(reasons))
        return 3
    pend = pending_for(a.repo_root, doc.get("context_id"))
    blocked = resume_blockers(pend)
    if blocked:
        print("  → STOP: 답이 필요한 차단성 요청이 남아 있다 — 자동 재발급하지 않는다.")
        return 3
    a.topology = a.topology or doc.get("topology")
    a.manifest_path = a.manifest or doc.get("manifest") or os.path.join(
        REPO, "output", a.topology or "single", "manifest.yaml")
    a.context_id = doc.get("context_id")
    a.campaign_id = doc.get("campaign_id")
    a.control_variables = doc.get("control_variables")
    a.node = doc.get("campaign_node")
    a.context_kind = doc.get("campaign_context_kind")
    a.cell = doc.get("campaign_cell")
    if not (getattr(a, "permission_profile", None) or getattr(a, "sandbox_mode", None)):
        for key, value in (doc.get("permission_selection") or {}).items():
            setattr(a, key, value)
    if getattr(a, "reasoning_effort", None) is None:
        a.reasoning_effort = doc.get("reasoning_effort")
    lks = last_known_session(doc)
    deadline_resume = (last.get("failure_evidence") or {}).get("signal") in ("controlled_local_deadline", "controlled_remote_deadline")
    native_exhausted = last.get("end_reason") == "budget_exhausted"
    if native_exhausted and not native_budget_exhaustion_ready(a.repo_root, doc):
        print("[relay] STOP: native budget exhaustion lacks durable same-session evidence")
        return 3
    if deadline_resume and not controlled_deadline_ready(a.repo_root, doc):
        print("[relay] STOP: controlled deadline lacks durable same-session/command evidence")
        return 3
    if allow_capacity:
        previous = next((b for b in reversed(doc.get("capacity_bursts") or [])
                         if b.get("burst_id") == deferred.get("burst_id")), None)
        answered = next((e for e in pend if e.get("recovery_required") == codex_recovery_requirement(last)
                         and e.get("answer") == (previous or {}).get("authorization")
                         and e.get("answer_source") == (previous or {}).get("source")), None)
        if not last.get("session_id") or not previous or not answered:
            print("[relay] STOP: deferred capacity lacks its recorded burst authorization")
            return 3
        a._capacity_resume_policy = previous
        a.continue_, a.apply, a.resume = True, True, last["session_id"]
    bud = select_supervised_budget(a, doc, secs, why)
    a._continuation_source_attempt = continuation_instruction(a.repo_root, doc)["attempt"]
    task = assemble_continuation(a.repo_root, doc, pend)
    if deadline_resume or native_exhausted:
        task = ("다른 행동 전에 기존 백그라운드 명령, 실행 중 서비스, sweep 상태와 산출물을 확인해 "
                "상태를 조정하고 부분 파일을 보존하라. 조정 전에는 벤치를 재시작하거나 "
                "기존 파일을 덮어쓰지 마라.\n" + task)
    return run_attempt(a, doc, lp, task, bud, lks.get("session_id") or "new")


def _self_test_handoff_reuse(chk):
    """Exercise the real CLI routing with a local returned report; no worker runs."""
    import copy
    import io
    from types import SimpleNamespace
    from unittest.mock import patch

    global _RELAY_CAMPAIGN
    previous_campaign = _RELAY_CAMPAIGN
    _RELAY_CAMPAIGN = None
    try:
        with tempfile.TemporaryDirectory() as td:
            ctx = "handoff-test"
            lp = ledger_path(td, ctx)
            rel = str((Path(lp).with_suffix(".reports") / "attempt-01.json").relative_to(td))
            report = {"task_id": "task-1", "context_id": ctx, "turn": 1, "phase": "publish",
                      "status": "completed", "node_id": "sub", "running_services": [],
                      "self_verification": {"checks_run": ["identity", "schema_valid"], "identity_ok": True},
                      "artifacts": [{"kind": "report", "path": "docs/testlog/result.md"}]}
            _atomic_json(str(Path(td) / rel), {"report": report})
            att = {"attempt": 1, "status": "completed", "control_status": "completed",
                   "sub_reported": True, "context_id_reported": ctx, "report_path": rel,
                   "running_services": [], "provider": "claude_code", "session_id": "original-native",
                   "execution_started": True, "reason_codes": [], "budget_outcome": "within_budget"}
            base = {"context_id": ctx, "topology": "single", "campaign_node": "sub",
                    "manifest": str(Path(td) / "manifest.yaml"), "attempts": [att]}
            a = SimpleNamespace(repo_root=td, context_id=ctx, topology="single", manifest=None,
                                campaign_control=None, handoff_issue=None, continue_=True,
                                apply=False, emit_only=False)
            with contextlib.redirect_stdout(io.StringIO()) as output:
                reused = reuse_completed_handoff(a, copy.deepcopy(base), lp)
            got = json.loads(output.getvalue())
            chk(reused and got["report"] == report and got["execution_started"] is False
                and got["continue_workflow"], "handoff: return the original report without native execution")

            def rejected(doc, args=a):
                try:
                    with contextlib.redirect_stdout(io.StringIO()):
                        reuse_completed_handoff(args, doc, lp)
                except SystemExit:
                    return True
                return False

            chk(rejected({**base, "inflight": {"dispatch_id": "running"}}),
                "handoff: unresolved execution is not hidden by reuse")
            wrong = copy.copy(a)
            wrong.topology = "multi"
            chk(rejected(copy.deepcopy(base), wrong), "handoff: other topology cannot reuse this report")
            _atomic_json(str(Path(td) / rel), {"report": {**report, "context_id": "other"}})
            chk(rejected(copy.deepcopy(base)), "handoff: wrong report context is not accepted")
            _atomic_json(str(Path(td) / rel), {"report": report})
            changed = copy.deepcopy(base)
            changed["attempts"][0]["control_variables_echo"] = {"model": "old"}
            changed["control_variables"] = {"model": "new"}
            chk(not reuse_completed_handoff(a, changed, lp), "handoff: changed declared inputs require actual work")
            failed = copy.deepcopy(base)
            failed["attempts"][0]["status"] = "failed"
            chk(not reuse_completed_handoff(a, failed, lp), "handoff: failed work is not reused as success")

            module = sys.modules[__name__]
            cli = ["relay.py", "--repo-root", td, "--context-id", ctx]
            runner = [{"name": "fixture", "provider": "claude_code", "backend": "anthropic", "model": "sonnet"}]
            with patch.object(module, "resolve_ladder", return_value=runner), \
                    patch.object(module, "resolve_campaign_axis", return_value=(None, None)):
                for flags in (["--continue", "--apply"], ["--topology", "single", "--task", "다시 전부 검증"]):
                    Path(lp).unlink(missing_ok=True)
                    save_ledger(lp, copy.deepcopy(base))
                    with patch.object(sys, "argv", cli + flags), \
                            patch.object(module, "run_attempt", side_effect=AssertionError("worker must not start")), \
                            contextlib.redirect_stdout(io.StringIO()):
                        rc = _main()
                    saved = load_ledger(lp)
                    chk(rc == 0 and saved["attempts"] == base["attempts"]
                        and saved["handoff_reuse"]["execution_started"] is False,
                        "handoff CLI: suppress completed-context reissue without attempt or runner failure")
                Path(lp).unlink(missing_ok=True)
                save_ledger(lp, copy.deepcopy(base))
                with patch.object(sys, "argv", cli + ["--topology", "single", "--task", "해당 항목을 정정하라",
                                  "--handoff-issue", "docs/testlog/result.md#F03 실제 모순",
                                  "--max-turns", "2", "--timeout-seconds", "30", "--budget-source", "fixture",
                                  "--resume", "new"]), \
                        patch.object(module, "run_attempt", return_value=73) as dispatch:
                    rc = _main()
                chk(rc == 73 and dispatch.call_count == 1 and "F03" in dispatch.call_args.args[3],
                    "handoff CLI: observed issue follows the existing explicit feedback path")
    finally:
        _RELAY_CAMPAIGN = previous_campaign


def _self_test() -> int:
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

    _self_test_handoff_reuse(chk)

    BUD25 = turn_budget.declare(25, 1800, source="선언: 자체검사 픽스처")
    with tempfile.TemporaryDirectory() as d:
        lp = ledger_path(d, "ctx-1")
        doc = load_ledger(lp)
        chk(doc["attempts"] == [], "빈 원장 초기화")

        # ① 정상 완료 → within_budget · 세션 이어붙이지 않음(완결된 세션은 잇지 않는다)
        record_attempt(doc, context_id="ctx-1", bud=BUD25,
                       result={"num_turns": 12, "budget_outcome": "within_budget",
                               "session_id": "s1", "status": "completed", "reason_codes": []},
                       report={"status": "completed", "phase": "config"})
        save_ledger(lp, doc)
        _a0 = json.load(open(lp))["attempts"][0]
        chk(_a0["max_turns_used"] == 12 and _a0["max_turns_allocated"] == 25
            and _a0["timeout_seconds_allocated"] == 1800 and "선언" in _a0["budget_source"]
            and "task_grade" not in _a0,
            "원장이 예산 서사(선언값·timeout·출처)를 적고 등급 라벨은 남기지 않는다")
        chk(last_known_session(doc)["session_id"] == "s1"
            and last_known_session(doc)["status"] == "completed",
            "마지막 알려진 세션을 **사실로** 돌려준다(재개 여부는 판정하지 않는다)")

        # ② input-required → 같은 세션을 잇는다
        record_attempt(doc, context_id="ctx-1", bud=BUD25,
                       result={"num_turns": 5, "budget_outcome": "within_budget",
                               "session_id": "s2", "status": "completed", "reason_codes": []},
                       report={"status": "input-required", "phase": "config"})
        chk(last_known_session(doc)["session_id"] == "s2", "직전 input-required 세션이 사실로 보인다")

        # ③ 소진 → 세션을 잇지 않고, 다음 예산은 **더 크다**
        record_attempt(doc, context_id="ctx-1", bud=BUD25,
                       result={"num_turns": 25, "budget_outcome": "exhausted",
                               "session_id": "s3", "status": "execution_failed",
                               "reason_codes": ["NONZERO_EXIT"]},
                       report=None)
        _lk = last_known_session(doc)
        chk(_lk["session_id"] == "s3" and _lk["budget_outcome"] == "exhausted",
            "소진된 세션도 **사실로는** 보인다 — 이을지는 --resume 선언이 정한다")
        chk(budget_floor(doc) == 25, f"집행된 예산 바닥이 사실로 남는다 → {budget_floor(doc)}")

        # ④ SILENT_FALLBACK 금지: 리포트 없는 attempt 는 서브 산출로 집계되지 않는다
        chk(doc["attempts"][-1]["sub_reported"] is False,
            "서브 리포트 없는 attempt 는 sub_reported=false(메인 대행을 성공으로 집계 ✗)")

        # ⑤ HITL 표면화 — 서브가 스스로 needed 를 말한 것만
        p = surface_requests(d, "ctx-1", {"hitl": {"needed": True, "request_id": "h1",
                                                   "prompt": "어느 핀?"}},
                             attempt=1)
        chk(p and json.load(open(p))["pending"][0]["request_id"] == "h1", "HITL 표면화")
        p2 = surface_requests(d, "ctx-2", {"hitl": {"needed": False}}, attempt=1)
        chk(p2 is None, "요청이 없으면 새 항목을 만들지 않는다(메인이 대신 만들지 않는다)")

        # ⑥ 리포트 파싱 — 산문에 섞여 와도 JSON 을 집는다 / 없으면 None
        chk(parse_report("어쩌고 {\"status\": \"completed\"} 끝")["status"] == "completed",
            "산문 속 JSON 리포트 추출")
        chk(parse_report("리포트 없음") is None, "JSON 이 없으면 None(추측 파싱 ✗)")
        # ★ 2026-10-01(plan_26100113 F7 · 라이브 attempt-01 형상): 잘린 거부 상세 JSON 이 앞에 있어도 뒤따르는
        #   완전한 리포트를 집는다 — 종전 괄호-깊이 판본은 여기서 None 이었다(완수가 실패로 집계).
        _trunc = ('[permission_denials] 1건 — 거부된 도구 호출:\n  - Bash {"command": "cat > /tmp/slice.json '
                  '<<\'EOF\'\\n{\\"schema_version\\":1,\\"id\\":\\"camp\n\n[서브가 남긴 말]\n```json\n'
                  '{"task_id": "t", "status": "completed", "image": {"tag": "x"}, "artifacts": [{"path": "a"}]}\n```\n끝')
        _pr = parse_report(_trunc)
        chk(isinstance(_pr, dict) and _pr.get("status") == "completed" and _pr.get("task_id") == "t",
            f"★잘린 JSON 뒤의 완전한 리포트를 집는다(중첩 객체를 따로 집지 않는다) → {_pr!r}")
        chk(parse_report('{"a": 1} 산문 {"status": "completed", "n": {"x": 1}}')["status"] == "completed",
            "마지막 최상위 객체를 집는다(안쪽 중첩 객체 ✗)")
        chk(parse_report('{"status": "completed"') is None, "★음성대조: 닫히지 않은 JSON 은 리포트가 아니다(추측 ✗)")

        # ⑦ 위임 전 서브 브랜치 == 토폴로지 (양방향 + 판독불가는 fail-closed)
        _req = {"target": {"host": "h", "ssh_user": "u", "work_dir": "/w"}}
        chk(assert_sub_branch(_req, "single", runner=lambda t, c: (0, "single", "")) == "single",
            "서브 브랜치가 토폴로지와 같으면 통과")
        try:
            assert_sub_branch(_req, "single", runner=lambda t, c: (0, "multi", ""))
            chk(False, "불일치는 STOP")
        except SystemExit as e:
            chk("브랜치 불일치" in str(e), "불일치는 STOP(브랜치 불일치)")
        try:
            assert_sub_branch(_req, "single", runner=lambda t, c: (255, "", "Connection refused"))
            chk(False, "판독 불가는 STOP")
        except SystemExit as e:
            chk("판독하지 못했다" in str(e), "판독 불가는 일치로 치지 않는다(fail-closed)")

        # ⑧ 2026-09-04 회귀 — 픽스처를 **실물 폭**으로 넓힌다.
        #    ①의 "완료된 세션은 재개 대상 아님" 은 attempt 1건 픽스처라 루프가 우연히 소진되어
        #    통과했다. 실물 원장(7건)에서는 실패했다(감사 D3 라이브 재현).
        d2 = {"attempts": [
            {"attempt": 1, "status": "input-required", "session_id": "sA",
             "budget_outcome": "within_budget", "max_turns_allocated": 25},
            {"attempt": 2, "status": "completed", "session_id": "sB",
             "budget_outcome": "within_budget", "max_turns_allocated": 25},
        ]}
        chk(last_known_session(d2)["session_id"] == "sB",
            "★음성대조: 앞에 input-required 가 있어도 **마지막 것**을 돌려준다(뒤로 스캔 ✗)")
        d3 = {"attempts": [d2["attempts"][0],
                           {"attempt": 2}]}          # 전송 실패(닿지 못함) — 건너뛴다
        chk(last_known_session(d3)["session_id"] == "sA",
            "닿지 못한 attempt 는 서브 상태를 바꾸지 못한다(건너뛴다)")

        # ⑨ 예산 바닥은 **집행된 사실**이다 — 닿지 못한 요청은 바닥이 되지 못한다(2026-09-04 실측)
        d4 = {"attempts": [
            {"attempt": 1, "budget_outcome": "exhausted", "max_turns_allocated": 25,
             "status": None, "session_id": "s1"},
            {"attempt": 2, "max_turns_allocated": 104},        # 스키마가 차단 = 닿지 못했다
        ]}
        chk(budget_floor(d4) == 25,
            f"★음성대조: 거절된 요청(104)은 바닥이 되지 못한다 → {budget_floor(d4)}")
        # ★ tripwire: 예산을 대신 정하던 함수가 되살아나면 여기서 잡는다(G-A2 · 2026-09-05 삭제).
        chk("next_budget" not in globals() and not hasattr(turn_budget, "GRADES"),
            "예산을 자동으로 정하는 경로가 되살아나지 않았다(선언만 남는다)")

        # ⑩ 정지 조건 — 전진 없는 연속 attempt
        d6 = {"attempts": [{"attempt": i, "status": "input-required", "phase": "build",
                            "session_id": "s%d" % i} for i in (1, 2, 3)]}
        chk(stalled_attempts(d6) == 3, f"전진 없는 3연속을 센다 → {stalled_attempts(d6)}")
        d7 = {"attempts": [{"attempt": 1, "status": "input-required", "phase": "config",
                            "session_id": "s1"},
                           {"attempt": 2, "status": "input-required", "phase": "build",
                            "session_id": "s2"}]}
        chk(stalled_attempts(d7) == 1, "phase 가 바뀌면 전진으로 센다(예산만 태운 것과 구분)")

        # ⑪ B2/B3/A4 — 표면화·우선순위·해소
        rep_lib = {"hitl": {"needed": True, "prompt": "q1", "blocking": True, "request_id": "r1"}}
        pth = surface_requests(d, "ctx-lib", rep_lib, attempt=1)
        chk(pth is not None, "hitl.needed 는 표면화된다")
        chk(entry_blocking(pending_for(d, "ctx-lib")[0]) is True,
            "blocking 을 메인이 **읽는다**(hitl 절의 차단성 선언)")
        chk(surface_requests(d, "ctx-old", {"library" + "_request": [
                {"question": "q0", "blocking": True, "request_id": "r0"}]}, attempt=1) is None
            and entry_blocking({"library" + "_request": [{"blocking": True}]}) is False,
            "★서고 교환 은퇴: 옛 요청 배열은 읽혀도 표면화·차단성을 만들지 않는다(plan_26101912)")
        surface_requests(d, "ctx-nb", {"hitl": {"needed": True, "prompt": "q2", "blocking": False,
                                                "request_id": "r2"}}, attempt=1)
        order = [e["request_id"] for e in sort_pending(
            pending_for(d, "ctx-nb") + pending_for(d, "ctx-lib"))]
        chk(order[0] == "r1", f"★우선순위: blocking 이 먼저다(사전순이 아니다) → {order}")
        # 사람이 답을 적으면 재표면화가 그것을 지우지 않는다
        _pp = os.path.join(relay_root(d), PENDING_HITL)
        _doc = json.load(open(_pp, encoding="utf-8"))
        for e in _doc["pending"]:
            if e["request_id"] == "r1":
                e["answer"] = "NAS 경로 X 를 쓴다"
        json.dump(_doc, open(_pp, "w", encoding="utf-8"), ensure_ascii=False)
        surface_requests(d, "ctx-lib", rep_lib, attempt=2)
        chk(pending_for(d, "ctx-lib")[0]["answer"] == "NAS 경로 X 를 쓴다",
            "재표면화가 사람의 답을 지우지 않는다(두 번 답하게 하지 않는다)")
        # 해소되면 사라진다 — 종전에는 needed=false 가 정리 필터에 도달조차 못 했다
        surface_requests(d, "ctx-lib", {"hitl": {"needed": False}}, attempt=3)
        chk(pending_for(d, "ctx-lib") == [],
            "★음성대조: 요청이 사라진 턴에는 낡은 항목이 제거된다(라이브 잔존 실측 교정)")

        # ⑫ A1 — 조립기는 **기록된 것만** 옮긴다
        rd = os.path.join(relay_root(d), "ctx-c.reports")
        os.makedirs(rd, exist_ok=True)
        json.dump({"report": {"artifacts": [{"kind": "log", "path": "output/x.json"}],
                              "next_steps": ["serve 재기동"], "notes": "KV 미정"}},
                  open(os.path.join(rd, "attempt-01.json"), "w", encoding="utf-8"))
        d8 = {"context_id": "ctx-c", "topology": "multi", "task": "원 지시 본문", "attempts": [
            {"attempt": 1, "status": "input-required", "phase": "build", "session_id": "s1",
             "budget_outcome": "exhausted", "max_turns_allocated": 25,
             "report_path": "campaigns/_bootstrap/relay/ctx-c.reports/attempt-01.json"}]}
        _im = Path(d) / "instruction-manifest.yaml"
        _im.write_text("topology: multi\nnodes:\n  - role: sub\n    host: 192.0.2.11\n"
                       "    ssh_user: fixture\n    work_dir: /srv/fixture\n", encoding="utf-8")
        _iq = build_request("multi", str(_im), d8["task"], BUD25, context_id="ctx-c", attempt=1,
                            model="sonnet", backend="anthropic")
        _ip = Path(d) / "campaigns/_bootstrap/relay/ctx-c.requests/attempt-01.json"
        _ip.parent.mkdir(parents=True)
        _ip.write_text(json.dumps(_iq), encoding="utf-8")
        d8["attempts"][0].update(provider="claude_code", session_binding=permission_binding(_iq),
            request_path=str(_ip.relative_to(d)), request_digest=hashlib.sha256(_ip.read_bytes()).hexdigest(),
            task_instruction={"schema_version": 1, "kind": "explicit", "source_attempt": 1,
                              "scope": _instruction_scope(d8), "body": d8["task"],
                              "body_digest": hashlib.sha256(d8["task"].encode()).hexdigest()})
        body = assemble_continuation(d, d8, [{"request_id": "r9", "blocking": True,
                                              "answer": "답 A", "prompt": "질문 Q"}])
        for token in ("output/x.json", "serve 재기동", "KV 미정", "답 A", "원 지시 본문", "예산 소진"):
            chk(token in body, f"조립 본문이 기록된 사실을 담는다: {token!r}")
        chk("추측" not in body.replace("추측 없음", ""), "조립기는 추측을 적지 않는다")

        # ⑬ A5/A6 — 헤더 주입과 재개 검증
        hdr = relay_header("ctx-h", 3, 40, "선언: 빌드 1회 — 원장 attempt 2 실측 turns=31")
        chk("context_id: ctx-h" in hdr and "max_turns_allocated: 40" in hdr
            and "실측 turns=31" in hdr,
            "위임 헤더가 context_id·예산·**그 근거**를 서브에게 알린다(comms.md 규약의 실배선)")
        d9 = {"attempts": []}
        record_attempt(d9, context_id="ctx-h", bud=turn_budget.declare(40, 3600, source="선언: 픽스처"),
                       result={"session_id": "other", "status": "completed"},
                       report={"status": "completed", "context_id": "ctx-h"},
                       resume_declared="wanted")
        chk(d9["attempts"][0]["resume_honored"] is False,
            "★재개 불발이 원장에 남는다(침묵 새 세션 ✗)")
        chk(d9["attempts"][0]["external_search"] == [], "외부검색 기록 자리가 있다(B안 자산화 입력)")

        # ⑭ 축 F — 원장 필드·재개 선언·종료사유(2026-09-05 · plan_26090516 3-2/3-3)
        dF = {"attempts": []}
        aF = record_attempt(dF, context_id="ctx-f", bud=turn_budget.declare(12, 900, source="선언: F"),
                            result={"session_id": "sF", "status": "completed", "num_turns": 7,
                                    "budget_outcome": "within_budget", "reason_codes": [],
                                    "duration_ms": 90_000, "duration_api_ms": 61_000},
                            report={"status": "completed", "phase": "bench"},
                            resume_declared="new", request_path="campaigns/_bootstrap/relay/ctx-f.requests/attempt-01.json",
                            started_utc="2026-09-05T10:00:00Z", ended_utc="2026-09-05T10:01:30Z")
        for k in ("request_path", "started_utc", "ended_utc", "duration_ms", "duration_api_ms",
                  "end_reason", "resume_declared"):
            chk(aF.get(k) is not None, f"원장이 축 F 필드를 적는다: {k}={aF.get(k)!r}")
        chk(aF["duration_ms"] - aF["duration_api_ms"] == 29_000,
            "정지 = wall − api 가 원장에서 파생된다(29,000ms)")
        chk(aF["resume_declared"] == "new" and aF["resume_requested"] is None,
            "`new` 선언은 재개 요청이 아니다(둘을 구분해 적는다)")
        chk(aF["end_reason"] == "completed", f"종료사유 파생 → {aF['end_reason']}")

        # append-only: 다음 attempt 를 적어도 앞선 줄은 **바이트 그대로** 남는다
        _snap = json.dumps(dF["attempts"][0], sort_keys=True, ensure_ascii=False)
        record_attempt(dF, context_id="ctx-f", bud=turn_budget.declare(20, 900, source="선언: F2"),
                       result={"session_id": "sF2", "status": "execution_failed", "num_turns": 20,
                               "budget_outcome": "exhausted", "reason_codes": ["NONZERO_EXIT"]},
                       report=None, resume_declared="sF")
        chk(json.dumps(dF["attempts"][0], sort_keys=True, ensure_ascii=False) == _snap,
            "★append-only: 새 attempt 가 앞선 줄을 소급 수정하지 않는다")
        chk(dF["attempts"][1]["end_reason"] == "budget_exhausted"
            and dF["attempts"][1]["resume_requested"] == "sF",
            "소진·재개요청이 각각의 자리에 남는다")
        chk(end_reason({"budget_outcome": "external_interruption"}) == "external_interruption"
            and end_reason({"reason_codes": ["TIMEOUT"], "control_status": "timeout"})
                == "external_interruption",
            "외생 중단(timeout/SIGTERM)은 예산 소진과 **다른 사유**로 적힌다")
        # ★ 2026-10-01(plan_26100113 D2): 거부 1건 ≠ 과업 실패 — 리포트가 completed 일 때만 가른다.
        chk(end_reason({"reason_codes": ["PERMISSION_DENIED"], "control_status": "execution_failed",
                        "status": "completed"}) == "completed_with_denials",
            "★거부 사건 + completed 리포트 → completed_with_denials")
        chk(end_reason({"reason_codes": ["PERMISSION_DENIED"], "control_status": "execution_failed",
                        "status": None}) == "permission_denied"
            and end_reason({"reason_codes": ["PERMISSION_DENIED"], "control_status": "execution_failed",
                            "status": "failed"}) == "permission_denied",
            "★음성대조: 리포트 부재·completed 아님은 종전대로 permission_denied")
        _dG = {}
        _aG = record_attempt(_dG, context_id="ctx-g", bud=turn_budget.declare(40, 3600, source="선언: G"),
                             result={"session_id": "sG", "status": "execution_failed", "num_turns": 31,
                                     "reason_codes": ["PERMISSION_DENIED"],
                                     "output": "[permission_denials] 1건 — 거부된 도구 호출:\n  - Bash {}"
                                               "\n\n[서브가 남긴 말]\n{\"status\": \"completed\"}"},
                             report={"status": "completed"}, resume_declared="new")
        chk(_aG["end_reason"] == "completed_with_denials"
            and (_aG.get("permission_denials") or "").startswith("[permission_denials] 1건")
            and "[서브가 남긴 말]" not in _aG["permission_denials"],
            f"★원장이 거부 블록을 따로 남긴다(숨기지 않는다) → {_aG.get('permission_denials')!r}")
        chk(end_reason({"control_status": "execution_failed", "session_id": None})
            == "transport_or_launch_failure"
            and end_reason({}) == "unclassified",
            "전송 실패와 분류 불가를 구분한다(모르는 것을 아는 척하지 않는다)")

        # ⑮ 재개 선언 — 없으면 fail-loud, 마지막 알려진 세션을 함께 말한다
        try:
            require_resume(None, {"session_id": "sZ", "attempt": 3, "status": "input-required",
                                  "budget_outcome": "within_budget"})
            chk(False, "재개 미선언 → fail-loud")
        except SystemExit as _e:
            chk("sZ" in str(_e) and "--resume new" in str(_e),
                "★재개 미선언은 죽되 **마지막 알려진 세션**을 함께 제시한다")
        chk(require_resume("new", {}) == "new" and require_resume(" sX ", {}) == "sX",
            "선언 어휘 둘(session_id · new)을 그대로 받는다")

        # ⑯ 크래시 표면화 — 리포트 없는 턴에도 재개 재료가 대기 목록에 남는다
        pc = surface_requests(d, "ctx-crash", {}, attempt=2,
                              crash={"attempt": 2, "end_reason": "malformed_output",
                                     "control_status": "malformed_output",
                                     "reason_codes": ["PROVIDER_RESULT_INVALID"],
                                     "last_known_session": "sC"})
        _e0 = pending_for(d, "ctx-crash")
        chk(pc is not None and _e0 and "sC" in (_e0[0].get("prompt") or ""),
            "★리포트 없이 끝난 턴은 대기 목록에 마지막 알려진 세션을 남긴다(침묵 종결 ✗)")
    # ── 캠페인 정체성 배달 + echo fail-closed (2026-09-07 · plan_26090715 §4.8) ─────────────
    CV = {"model": "gpt-oss-20b", "vllm_version": "0.18.0", "topology": "single",
          "target_gpu": "NVIDIA GB10"}
    blk = campaign_control_block("camp-x", CV)
    chk("campaign_id: camp-x" in blk, "위임 헤더가 campaign_id 를 싣는다")
    chk("campaigns/camp-x/" in blk, "서브 인스턴스 경로를 디렉터리명 = campaign_id 로 지시한다")
    chk(all(f"- {k}: {CV[k]}" in blk for k in CV),
        "★layer-1 통제변인 4키가 전부 실린다(2026-09-05 에 모델 행이 빠졌다)")
    chk("control_variables_echo" in blk and "부재도 불일치도" in blk,
        "echo 규약과 fail-closed 를 서브에게 명시한다")
    chk(campaign_control_block(None, None) == "",
        "캠페인 밖(_bootstrap)에서는 캠페인 축을 싣지 않는다")
    # Ray worker는 main-owned distributed cell에 참여할 뿐 독립 sub campaign을 열지 않는다.
    chk(campaign_slice_block("multi", "camp-x", "sub") == "",
        "★multi Ray-worker request에는 autonomous slice를 싣지 않는다")

    def _att(**kw):
        base = {"sub_reported": True, "context_id_reported": "ctx-1",
                "campaign_id_reported": "camp-x",
                "control_variables_echo": dict(CV)}
        base.update(kw)
        return base

    _S = lambda att: echo_stop_reasons(att, context_id="ctx-1", campaign_id="camp-x",
                                       control_variables=CV)
    chk(not _S(_att()), "전부 일치하면 통과")
    chk(any("context_id echo 부재" in r for r in _S(_att(context_id_reported=None))),
        "★음성대조 context_id echo **부재**도 STOP(종전 `if 값 and` 는 침묵 통과했다)")
    chk(any("context_id 불일치" in r for r in _S(_att(context_id_reported="other"))),
        "★음성대조 context_id 불일치 STOP")
    chk(any("campaign_id echo 부재" in r for r in _S(_att(campaign_id_reported=None))),
        "★음성대조 campaign_id echo 부재 STOP(서브가 이름을 스스로 짓는 것을 막는다)")
    chk(any("campaign_id 불일치" in r for r in _S(_att(campaign_id_reported="camp7-sub-native"))),
        "★음성대조 서브 자작 인스턴스명 STOP(2026-09-07 실측 형태)")
    chk(any("control_variables echo 부재" in r for r in _S(_att(control_variables_echo=None))),
        "★음성대조 통제변인 echo 부재 STOP")
    _drift = dict(CV, model="gpt-oss-120b")
    chk(any("control_variables 불일치" in r and "gpt-oss-120b" in r
            for r in _S(_att(control_variables_echo=_drift))),
        "★음성대조 서브가 모델을 바꾸면 드리프트로 STOP(2026-09-05 서브 120b 사건 형태)")
    chk(_S(_att(sub_reported=False, context_id_reported=None)) == [],
        "리포트 없는 크래시는 echo 위반이 아니다(같은 사실을 두 사유로 세지 않는다)")
    chk(echo_stop_reasons(_att(campaign_id_reported=None, control_variables_echo=None),
                          context_id="ctx-1", campaign_id=None, control_variables=None) == [],
        "캠페인 축을 안 실었으면 그 echo 도 묻지 않는다")

    # record_attempt 가 echo 두 필드를 실제로 원장에 옮기는가(배선 실효)
    with tempfile.TemporaryDirectory() as d2:
        lp2 = ledger_path(d2, "ctx-camp")
        doc2 = load_ledger(lp2)
        record_attempt(doc2, context_id="ctx-camp", bud=BUD25,
                       result={"num_turns": 5, "session_id": "s", "status": "completed",
                               "reason_codes": []},
                       report={"status": "completed", "phase": "publish",
                               "context_id": "ctx-camp", "campaign_id": "camp-x",
                               "control_variables_echo": dict(CV)})
        _a = doc2["attempts"][-1]
        chk(_a["campaign_id_reported"] == "camp-x" and _a["control_variables_echo"] == CV,
            "원장이 campaign echo 두 필드를 남긴다(소비자가 읽을 자리)")

    # ── 고아 서비스 원장 기록(2026-09-07 · 유예 결함 ⑦) ────────────────────────────────
    with tempfile.TemporaryDirectory() as d3:
        lp3 = ledger_path(d3, "ctx-svc")
        doc3 = load_ledger(lp3)
        record_attempt(doc3, context_id="ctx-svc", bud=BUD25,
                       result={"num_turns": 25, "budget_outcome": "exhausted", "session_id": "s",
                               "status": "completed", "reason_codes": []},
                       report={"status": "input-required", "context_id": "ctx-svc",
                               "running_services": [{"kind": "container", "name": "cell-x-serving",
                                                     "port": 8000, "note": "예산 소진 시점 상주"}]})
        chk(doc3["attempts"][-1]["running_services"][0]["name"] == "cell-x-serving",
            "원장이 고아 서비스를 남긴다(소비자가 읽을 자리)")
        doc4 = load_ledger(ledger_path(d3, "ctx-svc2"))
        record_attempt(doc4, context_id="ctx-svc2", bud=BUD25,
                       result={"num_turns": 5, "session_id": "s", "status": "completed",
                               "reason_codes": []},
                       report={"status": "completed", "context_id": "ctx-svc2"})
        chk(doc4["attempts"][-1]["running_services"] is None,
            "★말하지 않은 것은 None(모름)이지 빈 목록(없다는 선언)이 아니다")

    # ── 단계 ③: 캠페인 축 · 예산 파생 · 감독 스텝 (plan_26090813) ──────────
    import tempfile as _tf

    chk("timeout_seconds_allocated" in relay_header("c", 1, 40, "근거", 3600),
        "★위임 헤더가 시간 상한도 말한다(서브가 몇 초 뒤 잘리는지 알아야 한다)")
    chk("timeout_seconds_allocated" not in relay_header("c", 1, 40, "근거"),
        "시간 상한이 없으면 없다고 적지 않는다(모르는 값을 지어내지 않는다)")

    _d = {}
    bind_campaign_context(_d, node="sub", kind="cell", cell="c1")
    chk(_d["campaign_node"] == "sub" and _d["campaign_context_kind"] == "cell"
        and _d["campaign_cell"] == "c1", "원장이 캠페인 축을 못박는다(P4 가 읽을 자리)")
    _raised = None
    try:
        bind_campaign_context(_d, node=None, kind=None, cell="c2")
    except SystemExit as e:
        _raised = str(e)
    chk(_raised and "셀 하나 = context 하나" in _raised,
        "★음성대조 한 context 에 셀 둘을 묶으면 STOP(D20 · 2026-09-07 캡 절단 2/2)")
    _raised = None
    try:
        bind_campaign_context({"campaign_node": "main"}, node="sub", kind=None, cell=None)
    except SystemExit as e:
        _raised = str(e)
    chk(bool(_raised), "★음성대조 한 원장이 두 노드를 주장하면 STOP")
    _raised = None
    try:
        bind_campaign_context({}, node=None, kind="nope", cell=None)
    except SystemExit as e:
        _raised = str(e)
    chk(bool(_raised), "★음성대조 알 수 없는 context 종류 STOP")

    with _tf.TemporaryDirectory() as _t:
        _pd = os.path.join(_t, "campaigns", "camp-x", "phases", "sub")
        os.makedirs(_pd)
        with open(os.path.join(_pd, "serve.status.json"), "w", encoding="utf-8") as _f:
            json.dump({"first_started_utc": "2026-09-08T10:00:00Z",
                       "ended_utc": "2026-09-08T10:20:00Z"}, _f)
        _secs, _why = derive_timeout(_t, "camp-x", "sub")
        chk(_secs == 1800 and "실측 1200s" in _why,
            "★시간 예산을 지난 phase 실측에서 파생한다(1200s x 1.5 = 1800s)")
        _n, _why2 = derive_timeout(_t, "camp-x", "ghost")
        chk(_n is None and "스키마 상한" in _why2 and str(turn_budget.schema_cap("timeout_seconds")) in _why2,
            "★실측이 없으면 파생하지 않고 **스키마 상한을 읽어** 그 사실을 적는다(3600 재기재 ✗)")

    # 감독 판정 — 전진/정체/비용/단절 4분기 + 상주 0(함수 하나가 끝난다)
    _mk = lambda **kw: dict({"attempt": 1, "session_id": "s1", "sub_reported": True,
                             "control_status": "completed"}, **kw)
    chk(supervise_decide({"attempts": []})["action"] == "await_dispatch",
        "감독: 아무것도 안 나갔으면 지시서를 열라고 말한다")
    chk(supervise_decide({"attempts": [_mk(end_reason="completed", status="completed")]}
                         )["action"] == "next_cell", "감독: completed → 다음 셀")
    _cwd = supervise_decide({"attempts": [_mk(end_reason="completed_with_denials", status="completed",
                                              control_status="execution_failed")]})
    chk(_cwd["action"] == "next_cell" and "권한 거부" in _cwd["reason"],
        "★감독: completed_with_denials → 다음 셀(차단 ✗) · 사유에 거부 사건을 적는다")
    _prog = {"attempts": [_mk(end_reason="budget_exhausted", phase="build"),
                          _mk(attempt=2, end_reason="budget_exhausted", phase="serve")]}
    chk(supervise_decide(_prog)["action"] == "resume",
        "★감독: 소진이지만 phase 가 전진했으면 **자동 재발급**(D9 항상 자동)")
    _stall = {"attempts": [_mk(end_reason="unclassified", phase="serve"),
                           _mk(attempt=2, end_reason="unclassified", phase="serve")]}
    chk(supervise_decide(_stall)["action"] == "popup",
        "★감독: 전진이 없으면 예산을 키우기 전에 사람에게 묻는다")
    chk(supervise_decide(_stall, brief={"last_utc": "2026-09-08T20:00:00Z"})["action"] == "resume",
        "★감독: phase 가 그대로여도 브리핑이 전진했으면 재발급(R2 — 긴 벤치를 정체로 오판 ✗)")
    chk(supervise_decide(dict(_stall, supervisor_last_seen_utc="2026-09-08T21:00:00Z"),
                         brief={"last_utc": "2026-09-08T20:00:00Z"})["action"] == "popup",
        "★감독: 브리핑이 직전 스텝이 본 값보다 앞서지 않으면 전진이 아니다")
    chk(supervise_decide(_prog, cost_cap_attempts=2)["action"] == "resume",
        "★감독: 과거 누적 비용 상한은 재개를 막지 않는다")
    chk(supervise_decide({"attempts": [_mk(end_reason="transport_or_launch_failure")]}
                         )["action"] == "popup",
        "★감독: 통신·모델 평면이 깨지면 재개로 낫지 않는다 → 팝업")
    _pd = supervise_decide({"attempts": [_mk(end_reason="permission_denied")]})
    chk(_pd["action"] == "popup" and "권한 평면" in _pd["reason"]
        and "모델·통신" not in _pd["reason"],
        "★감독: 권한 거부는 **권한 거부라고** 말한다(사유가 과장되면 사람이 엉뚱한 곳을 본다)")
    chk(supervise_decide({"attempts": [_mk(end_reason="sub_input_required")]}
                         )["action"] == "popup", "감독: input-required 는 답이 승인이다")
    # 2026-09-29: 예산 양보(질문 표지 없음 + 남은 일) 는 원장 전진으로 잇는다 · 질문은 여전히 묻는다
    _y = dict(end_reason="sub_input_required", hitl_needed=False,
              max_turns_used=8, next_steps=["T1 나머지"], artifact_paths=["docs/testlog/a.md"])
    chk(supervise_decide({"attempts": [_mk(**_y)]})["action"] == "resume",
        "★감독: 예산 양보 input-required + 일한 흔적 → 자동 재발급(브리핑 없는 멀티 워커)")
    chk(supervise_decide({"attempts": [_mk(**dict(_y, hitl_needed=True))]})["action"] == "popup",
        "★감독 음성대조: hitl.needed 가 있으면 질문이다 → 팝업")
    chk(supervise_decide({"attempts": [_mk(**dict(_y, hitl_needed=None))]})["action"] == "popup",
        "★감독 음성대조: hitl 표지를 모르는 옛 원장(None)은 양보로 읽지 않는다 — 모르면 묻는다")
    chk(supervise_decide({"attempts": [_mk(**_y)]}, cost_cap_attempts=1)["action"] == "resume",
        "★감독 음성대조: 예산 양보 재개도 비용 상한을 넘지 않는다")
    chk(ledger_runner_spec({"runner_ladder": "kimi-claude,sonnet-5.5",
                            "attempts": [{"runner": {"name": "sonnet"}}]}) == "kimi-claude,sonnet-5.5"
        and ledger_runner_spec({"attempts": [{"runner": {"name": "sonnet-5.5"}},
                                             {"runner": {"name": "sonnet"}}]}) == "sonnet-5.5"
        and ledger_runner_spec({"attempts": [{}]}) is None,
        "★재발급 러너: 명시가 없으면 원장이 쓴 사다리를 잇는다(조용한 별칭 대체 ✗)")
    chk(recommended_timeout({"attempts": [_mk(budget_recommendation={"timeout_seconds": 900})]}) == 900
        and recommended_timeout({"attempts": [_mk(budget_recommendation={"timeout_seconds": "많이"})]}) is None
        and recommended_timeout({"attempts": [_mk()]}) is None,
        "★재발급 timeout: 서브 제안 수치만 읽는다(산문·비수치는 None — 값을 만들지 않는다)")
    chk(len(resume_blockers([{"blocking": True}])) == 1
        and resume_blockers([{"blocking": True, "answer": "a"}]) == []
        and resume_blockers([{"prompt": "비차단"}]) == [],
        "★재발급 차단: 답 없는 차단성 요청만 멈춘다 · 답했거나 비차단이면 통과")
    # 2026-09-29(P1 · R3-ⓑ): 리포트 없이 잘린 attempt — 문서 평면 전진
    _cut = {"context_id": "c-t1", "attempts": [_mk(end_reason="budget_exhausted", sub_reported=False,
                                                  started_utc="2026-09-29T10:06:05Z",
                                                  ended_utc="2026-09-29T10:07:05Z")]}
    with tempfile.TemporaryDirectory() as _d:
        _tl = os.path.join(_d, "sync_staging", "sub_docs", "testlog")
        os.makedirs(_tl)
        _t = _utc_epoch("2026-09-29T10:06:50Z")

        def _doc(name, body, when=_t):
            fp = os.path.join(_tl, name)
            with open(fp, "w", encoding="utf-8") as fh:
                fh.write(body)
            os.utime(fp, (when, when))
        chk(doc_plane_progress(_d, _cut) is None, "★문서 평면 음성대조: 미러가 비면 전진 없음")
        _doc("b.md", "context_id c-t10 · 다른 과업\n")
        chk(doc_plane_progress(_d, _cut) is None,
            "★문서 평면 음성대조: 다른 context(c-t10)의 문서는 세지 않는다(접두 일치 ✗)")
        _doc("c.md", "context_id c-t1 · 지난 attempt\n", when=_t - 3600)
        chk(doc_plane_progress(_d, _cut) is None,
            "★문서 평면 음성대조: attempt 창 밖 mtime 은 세지 않는다")
        _doc("a.md", "# T1\ncontext_id c-t1 · campaign x\n")
        _dp = doc_plane_progress(_d, _cut)
        chk(_dp is not None and "a.md" in _dp, f"★문서 평면: 창 안 · 같은 context 문서 → 전진 ({_dp})")
        chk(supervise_decide(_cut)["action"] == "resume"
            and supervise_decide(_cut, doc_progress=_dp)["action"] == "resume",
            "★감독: 호출 예산 소진은 증거 검증 후 이어진다")
    chk(supervise_decide({"attempts": [_mk(end_reason="sub_input_required", next_steps=["x"])]}
                         )["action"] == "popup",
        "★감독 음성대조: 표지 필드가 없는 옛 원장은 양보로 읽지 않는다(모르면 묻는다)")
    _stall = {"attempts": [_mk(**_y), _mk(**dict(_y, attempt=2))]}
    chk(supervise_decide(_stall)["action"] == "popup",
        "★감독 음성대조: 같은 남은 일·같은 산출물 반복 = 정체 → 팝업(무한 재발급 ✗)")
    _mv = {"attempts": [_mk(**_y), _mk(**dict(_y, attempt=2, next_steps=["T2"],
                                              artifact_paths=["docs/testlog/a.md"]))]}
    chk(supervise_decide(_mv)["action"] == "resume", "★감독: 남은 일이 바뀌면 전진이다")
    _un = {"attempts": [_mk(end_reason="unclassified", phase="serve"),
                        _mk(attempt=2, end_reason="unclassified", phase="serve")]}
    chk(supervise_decide(_un, brief={"last_utc": "2026-09-08T07:44:28Z"})["action"] == "resume",
        "★감독: 판정 불가라도 **전진이 관측되면** 잇는다(리포트 형식 미준수가 D9 를 무력화 ✗)")
    chk(supervise_decide(_un)["action"] == "popup",
        "★감독 음성대조: 판정 불가 + 전진 없음이면 묻는다(모르면서 잇지 않는다)")
    _led = {"attempts": [_mk(end_reason="budget_exhausted", phase="serve")]}
    record_supervisor_step(_led, {"action": "popup", "reason": "r"}, utc="t0",
                           brief={"last_utc": "u1"})
    chk(len(_led["supervisor_steps"]) == 1 and _led["supervisor_last_seen_utc"] == "u1",
        "★감독 판정이 원장에 남는다(다음 스텝이 같은 것을 다시 판정하지 않는다)")

    # 시간 상한 리터럴이 **프로덕션 코드**에 다시 적히지 않았다(같은 상수가 두 자리에 있으면
    # 갈라지고, 갈라진 쪽이 조용히 늦는다 · workflow.md §매직넘버 결함 칸). 시험 픽스처는
    # 판정 대상이 아니다 — 여기서 3600 을 쓰는 것은 헤더가 값을 그대로 나르는지 보는 용도다.
    _all = open(os.path.abspath(__file__), encoding="utf-8").read()
    _cap = str(turn_budget.schema_cap("timeout_seconds"))
    _prod = _all.split("def _self_test", 1)[0] + _all.split("\nSSH_PROBE = ", 1)[-1]
    chk(_cap not in _prod,
        f"★시간 상한({_cap}) 리터럴이 프로덕션 코드에 없다 — 상한은 스키마에서 읽는다")
    chk(_cap in _all, "음성대조: 슬라이스가 파일 전체를 지우지 않았다(도달 불가 시험 금지)")

    # ── 러너 사다리 (2026-09-08) ────────────────────────────────────────────────────────
    _tbl = runner_table(REPO)
    chk(set(_tbl) >= {"sonnet", "haiku", "opus", "kimi-claude", "minimax-claude", "meta-claude"},
        "별칭 표를 **어댑터에서** 읽는다(사본 ✗) → %s" % sorted(_tbl))
    _lad = resolve_ladder(REPO, "kimi-claude,minimax-claude,sonnet,haiku")
    chk([r["name"] for r in _lad] == ["kimi-claude", "minimax-claude", "sonnet", "haiku"]
        and _lad[0]["backend"] == "kimi" and _lad[2]["model"] == "sonnet",
        "사다리는 **선언 순서 그대로**이고 각 칸이 (backend, model) 쌍으로 펴진다")
    chk(len(resolve_ladder(REPO, None)) == 1
        and resolve_ladder(REPO, None)[0]["name"] == "sonnet"
        and resolve_ladder(REPO, None)[0]["backend"] == "anthropic",
        "기본 사다리는 1칸(claude -p · sonnet) — 그 밖의 슬롯·호출 방식은 가변(--runners · 2026-10-26 사용자 정의)")
    try:
        resolve_ladder(REPO, "sonnet,gpt-5")
        chk(False, "미등재 별칭은 STOP")
    except SystemExit as e:
        chk("등재되지 않은 러너" in str(e), "미등재 별칭은 STOP(오타가 조용히 통과하지 않는다)")

    def _ru_att(n, name, rotate=True, session="stub"):
        return {"attempt": n, "reason_codes": [RUNNER_UNAVAILABLE], "session_id": session,
                "runner": {"name": name}, "end_reason": "runner_unavailable",
                "runner_evidence": {"rotate": rotate}}

    _d = {"attempts": [_ru_att(1, "kimi-claude"), _ru_att(2, "minimax-claude")]}
    chk(next_runner_index(_d, _lad) == 2, "다음 칸은 원장에서 **파생**한다(커서 저장 ✗)")
    _d4 = {"attempts": [_ru_att(i + 1, n) for i, n in enumerate(
        ["kimi-claude", "minimax-claude", "sonnet", "haiku"])]}
    chk(next_runner_index(_d4, _lad) == 0, "★사다리는 **순환**한다(4 → 1)")
    _dok = {"attempts": [_ru_att(1, "kimi-claude"),
                         {"attempt": 2, "runner": {"name": "minimax-claude"},
                          "status": "completed", "session_id": "s9", "end_reason": "completed"}]}
    chk(next_runner_index(_dok, _lad) == 1,
        "성공한 칸은 **그 자리를 지킨다**(성공 뒤 회전은 이유 없는 강등이다)")
    chk(consecutive_runner_unavailable(_d4) == 4 and consecutive_runner_unavailable(_dok) == 0,
        "소진 술어는 **연속** 러너 실패만 센다(서브에 닿으면 0으로 리셋)")

    chk(_reached_sub(_ru_att(1, "kimi-claude")) is False,
        "★러너 실패는 세션 스텁을 싣고 와도 **닿은 것이 아니다**(예산 바닥·정체·마지막세션 오염 ✗)")
    chk(budget_floor({"attempts": [dict(_ru_att(1, "kimi-claude"), max_turns_allocated=40)]}) == 0,
        "★음성대조: 회전 attempt 는 **예산 바닥이 되지 못한다**(집행된 적이 없다)")
    chk(last_known_session({"attempts": [
            {"attempt": 1, "session_id": "real", "status": "input-required"},
            _ru_att(2, "kimi-claude", session="stub")]})["session_id"] == "real",
        "★음성대조: 회전이 만든 빈 세션이 **마지막 알려진 세션을 덮지 않는다**")

    chk(end_reason({"reason_codes": [RUNNER_UNAVAILABLE], "control_status": "execution_failed"})
        == "runner_unavailable", "러너 실패는 전송·예산과 **다른 라벨**을 받는다")
    chk(end_reason({"reason_codes": ["NONZERO_EXIT"], "control_status": "execution_failed",
                    "session_id": "s1", "status": "failed"}) == "sub_failed",
        "★음성대조: 서브가 일하다 실패한 것은 러너 실패로 접히지 않는다")

    _dec = supervise_decide({"attempts": [_ru_att(1, "kimi-claude")]}, ladder=_lad)
    chk(_dec["action"] == "rotate_runner", "감독: 칸이 남으면 **회전**(예산·범위 불변)")
    _dec4 = supervise_decide(_d4, ladder=_lad)
    chk(_dec4["action"] == "popup" and "소진" in _dec4["reason"],
        "감독: 한 바퀴를 다 돌면 **팝업**(사용자 규격 — 다 안 되면 HITL)")
    _dec1 = supervise_decide({"attempts": [_ru_att(1, "sonnet")]},
                             ladder=resolve_ladder(REPO, "sonnet"))
    chk(_dec1["action"] == "popup",
        "★사다리 1칸이면 첫 실패가 곧 소진 → 즉시 HITL(사용자 규격)")
    _capd = supervise_decide({"attempts": [_ru_att(i + 1, n) for i, n in enumerate(
        ["kimi-claude", "minimax-claude"])]}, ladder=_lad, cost_cap_attempts=2)
    chk(_capd["action"] == "rotate_runner",
        "★음성대조: 회전은 **비용이 아니다** — 상한 2에 회전 2회가 닿아도 팝업이 아니다")

    _ru_out = '[runner_unavailable] {"rotate": true, "signal": "api_error", "detail": "x"}'
    chk(parse_report(_ru_out) is not None,
        "음성대조: `parse_report` 는 이 문자열에서 JSON 을 **집는다**(그래서 위험했다)")
    _ru_res = {"reason_codes": [RUNNER_UNAVAILABLE], "output": _ru_out, "status": "execution_failed"}
    _guarded = (None if RUNNER_UNAVAILABLE in (_ru_res.get("reason_codes") or [])
                else parse_report(_ru_res["output"]))
    chk(_guarded is None,
        "★라이브 교정: 러너-불가 진단이 **서브 리포트로 집계되지 않는다** "
        "(종전엔 sub_reported=true 가 적혔다 — 메인 산출을 서브 산출로 세는 형태)")
    chk(echo_stop_reasons(_ru_att(1, "kimi-claude"), context_id="ctx-1",
                          campaign_id=None, control_variables=None) == [],
        "닿지 않은 attempt 에는 echo 판정을 걸지 않는다(sub_reported 가드)")
    _pe = parse_runner_evidence({"reason_codes": [RUNNER_UNAVAILABLE],
                                 "output": '[runner_unavailable] {"rotate": true, "signal": "api_error"}'})
    chk(_pe and _pe["rotate"] is True, "증거는 output 에서 **되읽힌다**(실어만 보내면 또 못 닿는다)")
    chk(parse_runner_evidence({"reason_codes": [RUNNER_UNAVAILABLE], "output": "깨진 텍스트"})
        ["rotate"] is False, "★판독 불가 증거는 회전하지 않는다(모르면 태우지 않는다)")

    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


SSH_PROBE = ("ssh", "-F", "/dev/null", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8",
             "-o", "StrictHostKeyChecking=yes", "-o", "IdentitiesOnly=yes",
             "-o", "ForwardAgent=no", "-o", "ClearAllForwardings=yes", "-o", "RequestTTY=no",
             "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=2")


def _ssh_run(target: dict, cmd: str, *, timeout_seconds: int):
    argv = [*SSH_PROBE, "--", f"{target['ssh_user']}@{target['host']}", cmd]
    try:
        out = subprocess.run(argv, capture_output=True, text=True, stdin=subprocess.DEVNULL,
                             timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        return 124, "", "branch preflight exceeded the declared wall deadline"
    return out.returncode, out.stdout.strip(), out.stderr.strip()


def assert_sub_branch(req: dict, topology: str, runner=None) -> str:
    """위임 직전 서브의 **현재 브랜치가 요청 토폴로지와 같은지** 확인한다(fail-closed).

    왜(2026-09-03 체크포인트 · plan_26090317 P4): 서브의 페르소나·tool_plane 은 **브랜치별로
    추적된 CLAUDE.md** 가 정한다(single=a2a-agent·런타임 스킬 3, multi=ray-worker·0). 릴레이는
    지금까지 manifest 의 토폴로지로 주소만 풀었고 서브가 실제로 어느 브랜치에 있는지는 보지
    않았다 — `sync_to_sub` 의 REST_BRANCH(마지막 배달 토폴로지) 덕에 **관행상** 일치했을 뿐
    게이트가 아니었다. 불일치 상태로 위임하면 다른 정체성에게 말하는 것이며, 그 실패는 서브의
    산문 거절로만 드러나 조용하다.
    서브 git 은 메인의 관측 장치다(헌법 노드 제어 불변식 2) — 이 읽기는 스캔이 아니다.
    """
    t = req["target"]
    cmd = "git -C " + shlex.quote(t["work_dir"]) + " rev-parse --abbrev-ref HEAD"
    if runner:
        rc, out, err = runner(t, cmd)
    else:
        # Bound both sides: killing only ssh can leave a hung remote git process.
        seconds = req["timeout_seconds"]
        remote = "timeout --signal=TERM --kill-after=3s " + str(seconds) + "s " + cmd
        rc, out, err = _ssh_run(t, remote, timeout_seconds=seconds + 3)
    if rc != 0:
        raise SystemExit(f"[relay] STOP: 서브 브랜치를 판독하지 못했다(rc={rc}) — {err or out or '(출력 없음)'}\n"
                         "  → 판독 불가는 일치가 아니다(fail-closed). 서브 도달성·work_dir 를 먼저 확인하라.")
    if out != topology:
        raise SystemExit(f"[relay] STOP(브랜치 불일치): 서브는 '{out}' 에 있고 요청 토폴로지는 '{topology}' 다.\n"
                         "  → 브랜치별 CLAUDE.md 가 정체성을 정하므로 이 상태의 위임은 다른 정체성에게 말하는 것이다.\n"
                         f"  → `sync_to_sub.sh --branch {topology}` 가 서브를 그 브랜치에 안착시킨다(REST_BRANCH).")
    return out


def _utcnow() -> str:
    """지금 시각(UTC). **실측이다** — 끝난 시각은 끝나 봐야 알므로 주입할 수 없다.

    이 저장소의 벽시계 금지는 문서 명명·유효성 판정 평면의 규칙이다(`docs.md`). 여기 두 값은
    원장의 시간 서사이며 판정에 쓰이지 않는다 — 그리고 이 값과 provider 가 보고한 `duration_*` 은
    **다른 시계**이므로 원장에서도 따로 적는다.
    """
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def permission_binding(request: dict) -> dict:
    return {"provider": request["provider"], "target": dict(request["target"]),
            **{k: request[k] for k in ("sandbox_mode", "permission_profile") if k in request}}


def bound_resume(doc: dict, declared, request: dict) -> tuple[str, dict | None]:
    """A native session belongs to its provider, workspace and permission selection."""
    if declared in (None, "new"):
        return "new", None
    prior = next((x for x in reversed(doc.get("attempts") or [])
                  if x.get("session_id") == declared), None)
    provider = request["provider"]
    if prior is None:
        if provider == "claude_code":
            return declared, None  # v1 explicit external-session contract
        raise SystemExit("[relay] STOP: Codex resume has no recorded session binding")
    previous_provider = (prior.get("runner") or {}).get("provider", "claude_code")
    if previous_provider != provider:
        return "new", None
    binding = prior.get("session_binding")
    if binding is None and provider == "claude_code":
        return declared, None  # pre-v2 ledger; preserve legacy explicit behavior
    if binding != permission_binding(request):
        raise SystemExit("[relay] STOP: resume target or permission binding changed")
    return declared, binding


def declared_budget(a, timeout_seconds, source: str, doc: dict | None = None) -> dict:
    providers = {r.get("provider", "claude_code") for r in a.ladder}
    if providers == {"codex_cli"}:
        return turn_budget.declare(a.max_turns, timeout_seconds, source=source, unit="wall_time")
    return turn_budget.declare(a.max_turns, timeout_seconds, source=source)


def _owned_artifact(repo_root: str, relative: str):
    root = Path(repo_root).resolve()
    rel = Path(relative)
    if rel.is_absolute() or ".." in rel.parts:
        raise SystemExit("[relay] STOP: recovery artifact must be workspace-relative")
    path = root
    for part in rel.parts:
        path = path / part
        if path.is_symlink():
            raise SystemExit("[relay] STOP: recovery artifact path cannot contain symlinks")
    if not path.resolve().is_relative_to(root) or not path.is_file():
        raise SystemExit("[relay] STOP: recovery artifact must be an existing regular file inside this workspace")
    return path


def load_dispatch_outcome(repo_root: str, doc: dict, selected_path: str | None = None) -> tuple[dict, dict]:
    """Accept only the outcome reserved for this exact request and dispatch generation."""
    reservation = doc.get("inflight") or {}
    relative = reservation.get("outcome_path")
    if not relative:
        raise SystemExit("[relay] STOP: this reservation has no durable provider outcome; it cannot be discarded or replayed")
    path = _owned_artifact(repo_root, relative)
    if selected_path:
        selected = Path(selected_path)
        if not selected.is_absolute():
            selected = Path(repo_root) / selected
        if selected.resolve() != path.resolve():
            raise SystemExit("[relay] STOP: selected outcome is not the reserved dispatch artifact")
    request_path = _owned_artifact(repo_root, reservation["request_path"])
    if hashlib.sha256(request_path.read_bytes()).hexdigest() != reservation.get("request_digest"):
        raise SystemExit("[relay] STOP: reserved request bytes changed; model/permissions/budget/resume cannot be rewritten")
    try:
        wrapper = json.loads(path.read_text(encoding="utf-8"))
        request = json.loads(request_path.read_text(encoding="utf-8"))
    except (OSError, ValueError, UnicodeDecodeError) as exc:
        raise SystemExit(f"[relay] STOP: unreadable recovery evidence: {type(exc).__name__}")
    if (not isinstance(wrapper, dict) or set(wrapper) != {"schema_version", "dispatch_id", "request", "result"}
            or wrapper.get("schema_version") != 1
            or wrapper.get("dispatch_id") != reservation.get("dispatch_id")
            or wrapper.get("request") != request):
        raise SystemExit("[relay] STOP: stale or mismatched dispatch outcome")
    violations = _agent_control.request_schema_violations(request)
    result = wrapper["result"]
    violations += _agent_control._schema_violations(result, _agent_control._load_schema(_agent_control.RESULT_SCHEMA_PATH))
    if not violations:
        violations += _agent_control._result_contract_violations(result, request)
    if (violations or request["target"] != reservation.get("target")
            or request["provider"] != reservation.get("provider")):
        raise SystemExit("[relay] STOP: recovered request/result violates the reserved execution contract")
    return request, result


def commit_dispatch_outcome(a, doc: dict, lp: str, request: dict, result: dict, *,
                            ended: str, control_stderr: str = "", reconciled: bool = False):
    """Commit an observed outcome once; recovery never launches a replacement worker."""
    reservation = doc["inflight"]
    dispatch_id = reservation["dispatch_id"]
    latest = load_ledger(lp)
    if (latest.get("inflight") or {}).get("dispatch_id") != dispatch_id:
        raise SystemExit("[relay] STOP: stale dispatch result; owner generation changed")
    if reservation["attempt"] != len(doc.get("attempts") or []) + 1:
        raise SystemExit("[relay] STOP: recovery attempt sequence changed")
    report = (None if RUNNER_UNAVAILABLE in (result.get("reason_codes") or [])
              else parse_report(result.get("output") or ""))
    if reconciled and report is not None:
        schema = _agent_control._load_schema(_agent_control.RUNTIME_DIR.parent / "sub_node" / "task-report.schema.json")
        if _agent_control._schema_violations(report, schema):
            # Preserve the native output without upgrading an invalid self-report to success.
            report = None
    att = record_attempt(doc, context_id=doc["context_id"], bud=reservation["budget"], result=result,
                         report=report, resume_declared=reservation["resume_declared"],
                         runner=reservation["runner"], session_binding=permission_binding(request),
                         dispatch_id=dispatch_id, request_path=reservation["request_path"],
                         started_utc=reservation["started_utc"], ended_utc=ended)
    directory = os.path.join(relay_root(a.repo_root), f"{doc['context_id']}.reports")
    os.makedirs(directory, exist_ok=True)
    path = os.path.join(directory, "attempt-%02d.json" % att["attempt"])
    _atomic_json(path, {"attempt": att["attempt"], "report": report,
                       "raw_output": None if report else (result.get("output") or ""),
                       "control_stderr_tail": control_stderr[-4000:] or None,
                       "control": {key: result.get(key) for key in
                           ("status", "exit_code", "reason_codes", "session_id", "num_turns", "budget_outcome",
                            "duration_ms", "duration_api_ms", "raw_exit_code", "execution_started", "usage",
                            "failure_evidence", "raw_events")}})
    att["report_path"] = os.path.relpath(path, a.repo_root)
    att["outcome_path"] = reservation["outcome_path"]
    att["request_digest"] = reservation["request_digest"]
    if "task_instruction" in reservation:
        # Keep the issued raw task and its authority on the immutable attempt.
        # Reconciliation consumes the same reservation without inventing a revision.
        att["task_instruction"] = reservation["task_instruction"]
    if reconciled:
        att["reconciliation"] = {"utc": ended, "source": reservation["outcome_path"],
                                 "method": "dispatch-bound-provider-outcome", "replayed": False}
    doc.pop("inflight")
    save_ledger(lp, doc)
    return att, report


def reconcile_inflight(a, doc: dict, lp: str) -> int:
    if not doc.get("inflight"):
        raise SystemExit("[relay] STOP: no unresolved in-flight dispatch exists")
    request, result = load_dispatch_outcome(a.repo_root, doc, getattr(a, "reconcile_result", None))
    reservation = doc["inflight"]
    print(f"[relay] reconciliation evidence: {reservation['dispatch_id']} -> {result['status']}; no replacement dispatch")
    if not a.apply:
        print("[relay] dry-run: pass --apply to append this observed outcome and consume its dispatch")
        return 0
    att, report = commit_dispatch_outcome(a, doc, lp, request, result, ended=_utcnow(), reconciled=True)
    crash = dispatch_crash(att, report)
    surface_requests(a.repo_root, doc["context_id"], report or {}, attempt=att["attempt"], crash=crash)
    print(f"[relay] reconciled attempt {att['attempt']}: {att['end_reason']}; dispatch counted, native observations retained")
    return 0


def run_attempt_once(a, doc: dict, lp: str, task: str, bud: dict, resume_declared,
                     runner: dict) -> int:
    """한 번의 위임 왕복 — **사다리 한 칸**. `--task` 진입과 `--continue` 진입이 같은 몸통을 쓴다."""
    attempt_no = len(doc.get("attempts") or []) + 1
    if doc.get("inflight"):
        raise SystemExit("[relay] STOP: unresolved dispatch remains; reconcile remote outcome before replay")
    provider = runner.get("provider", "claude_code")
    bud = dict(bud)
    if provider == "codex_cli":
        bud.update(max_turns=None, budget_unit="wall_time")
    else:
        bud.pop("budget_unit", None)
    require_dispatch_recovery(a, doc, resume_declared)
    profile = getattr(a, "permission_profile", None)
    sandbox = getattr(a, "sandbox_mode", None)
    if provider == "codex_cli" and not (profile or sandbox):
        raise SystemExit("[relay] STOP: Codex permission profile or sandbox mode must be declared")
    req = build_request(a.topology, a.manifest_path, task, bud,
                        context_id=a.context_id, attempt=attempt_no,
                        model=runner["model"], backend=runner["backend"],
                        provider=provider, permission_profile=profile, sandbox_mode=sandbox,
                        reasoning_effort=getattr(a, "reasoning_effort", None),
                        campaign_id=getattr(a, "campaign_id", None),
                        control_variables=getattr(a, "control_variables", None),
                        # 수명 제어 지시에는 개설 선언(slice)을 싣지 않는다 — 서브가 그것을 보고 --init 을 치면 전이가 아니라 개설이 된다.
                        delegated_node=(None if getattr(a, "campaign_control", None) else getattr(a, "node", None)))
    effective_resume, binding = bound_resume(doc, resume_declared, req)
    resume = None if effective_resume == "new" else effective_resume
    if effective_resume != resume_declared:
        req["task"] = ("[하네스 인계] native session은 provider 경계에서 새로 시작한다. "
                       "이전 실행의 증거·통제변인·원장에 따라 이어가라.\n" + req["task"])
        print("[relay] provider changed: fresh native session with artifact handoff", file=sys.stderr)
    if resume:
        req["resume_session_id"] = resume
        if provider == "codex_cli":
            req["resume_binding"] = binding
    violations = _agent_control.request_schema_violations(req)
    if violations:
        raise SystemExit("[relay] STOP: request rejected before remote preflight: " + "; ".join(violations))

    source_attempt = getattr(a, "_continuation_source_attempt", None)
    task_instruction = {"schema_version": 1, "scope": _instruction_scope(doc)}
    # Context/topology are filled below on the first dispatch; bind the input now.
    task_instruction["scope"].update(context_id=a.context_id, topology=a.topology)
    if source_attempt is None:
        task_instruction.update(kind="explicit", source_attempt=attempt_no, body=task,
                                body_digest=hashlib.sha256(task.encode()).hexdigest())
    else:
        resolved = continuation_instruction(a.repo_root, doc)
        if resolved["attempt"] != source_attempt:
            _instruction_refusal("continuation source changed before dispatch")
        task_instruction.update(kind="continuation", source_attempt=source_attempt)

    if a.emit_only:
        json.dump(req, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0

    branch = assert_sub_branch(req, a.topology)
    policy = getattr(a, "_prepared_capacity_policy", None)
    if policy and not capacity_deadline_fits(policy, bud):
        raise SystemExit("[relay] STOP: capacity deadline cannot fit the full attempt after preflight")
    print(f"[relay] attempt={attempt_no} runner={runner['name']} "
          f"(backend={runner['backend']} model={runner['model']}) "
          f"max_turns={bud['max_turns']} timeout={bud['timeout_seconds']}s ({bud['source']}) "
          f"resume={resume_declared} sub_branch={branch}")
    # 2026-09-05(축 F): 보낸 요청을 **보존한다**. 종전에는 임시파일로 보내고 지웠기 때문에 "그때
    #   무엇을 보냈는가" 가 남지 않았고, 조립기는 매번 다시 조립하므로 재현도 되지 않았다.
    _qdir = os.path.join(relay_root(a.repo_root), f"{a.context_id}.requests")
    os.makedirs(_qdir, exist_ok=True)
    _qp = os.path.join(_qdir, "attempt-%02d.json" % attempt_no)
    with open(_qp, "w", encoding="utf-8") as f:
        json.dump(req, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")
    started = _utcnow()
    dispatch_id = uuid.uuid4().hex
    outcome_dir = os.path.join(relay_root(a.repo_root), f"{a.context_id}.outcomes")
    os.makedirs(outcome_dir, exist_ok=True)
    outcome_path = os.path.join(outcome_dir, dispatch_id + ".json")
    doc.update(context_id=a.context_id, topology=a.topology, manifest=a.manifest_path)
    doc["task"] = doc.get("task") or task
    if policy:
        bursts = doc.setdefault("capacity_bursts", [])
        if not any(b.get("burst_id") == policy["burst_id"] for b in bursts):
            bursts.append(dict(policy))
    doc["inflight"] = {"dispatch_id": dispatch_id, "attempt": attempt_no,
                       "target": req["target"], "provider": provider,
                       "request_path": os.path.relpath(_qp, a.repo_root),
                       "request_digest": hashlib.sha256(Path(_qp).read_bytes()).hexdigest(),
                       "task_instruction": task_instruction,
                       "outcome_path": os.path.relpath(outcome_path, a.repo_root),
                       "budget": dict(bud), "runner": dict(runner),
                       "resume_declared": effective_resume,
                       "started_utc": started, "owner_pid": os.getpid()}
    if "reasoning_effort" in req:
        doc["reasoning_effort"] = req["reasoning_effort"]
    elif "reasoning_effort" in doc:
        del doc["reasoning_effort"]
    if provider == "codex_cli":
        doc["permission_selection"] = {k: req[k] for k in ("permission_profile", "sandbox_mode") if k in req}
    save_ledger(lp, doc)  # reserve durably before the external process can produce side effects
    if policy and not capacity_deadline_fits(policy, bud):
        doc.pop("inflight")
        save_ledger(lp, doc)
        raise SystemExit("[relay] STOP: capacity deadline cannot fit the full attempt before launch")
    out = subprocess.run([sys.executable, AGENT_CONTROL, "invoke", "--request", _qp,
                          "--result-file", outcome_path, "--dispatch-id", dispatch_id],
                         capture_output=True, text=True)
    ended = _utcnow()
    sys.stderr.write(out.stderr)
    control_stderr = (out.stderr or "").strip()
    try:
        result = json.loads(out.stdout)
    except ValueError:
        raise SystemExit(f"[relay] FAIL: agent_control 출력을 읽지 못했다 — {out.stdout[:300]}")

    _, durable_result = load_dispatch_outcome(a.repo_root, doc)
    if result != durable_result:
        raise SystemExit("[relay] STOP: stdout result differs from durable dispatch evidence")
    att, report = commit_dispatch_outcome(a, doc, lp, req, result,
                                          ended=ended, control_stderr=control_stderr)
    _crash = dispatch_crash(att, report)
    hp = surface_requests(a.repo_root, a.context_id, report or {}, attempt=att["attempt"],
                          crash=_crash)

    print(f"[relay] 원장 → {lp}")
    _stall = None
    if isinstance(att.get("duration_ms"), int) and isinstance(att.get("duration_api_ms"), int):
        _stall = att["duration_ms"] - att["duration_api_ms"]
    print(f"[relay] control={att['control_status']} sub_status={att['status']} "
          f"turns={att['max_turns_used']}/{att['max_turns_allocated']} budget={att['budget_outcome']} "
          f"end_reason={att['end_reason']}")
    print(f"[relay] 시간: {started} → {ended} · provider wall={att.get('duration_ms')}ms "
          f"api={att.get('duration_api_ms')}ms 정지={_stall if _stall is not None else '(모름)'}ms")
    if att.get("permission_denials"):
        # 완수와 별개로 거부 사건을 화면에 남긴다(원장에도 같은 블록이 있다).
        print("[relay] ⚠ 권한 거부 사건 — 과업 판정은 리포트가 한다(end_reason=%s):" % att["end_reason"])
        for _ln in att["permission_denials"].splitlines():
            print("[relay]   " + _ln)
    if att["resume_honored"] is False:
        print(f"[relay] ⚠ 재개 불발: 요청한 세션 {resume} 과 다른 세션 {att['session_id']} 이 열렸다 — "
              "서브가 컨텍스트를 처음부터 재구축했을 수 있다(소진의 주된 원인). 다음 턴의 예산을 그렇게 읽어라.")
    # ★ 2026-09-08 라이브 교정: echo 대조는 **서브에 닿은 attempt** 에만 뜻이 있다. 러너 평면
    #   실패는 서브가 한 글자도 받지 못한 것이므로 "echo 부재" 는 위반이 아니라 당연한 사실이고,
    #   그것을 경고로 찍으면 사람이 서브의 규약 위반을 의심하며 엉뚱한 곳을 본다(라이브 실측:
    #   회전 1회에 이 경고가 그대로 떴다). 관측한 것만 적는다.
    if _reached_sub(att):
        _echo_stop = echo_stop_reasons(att, context_id=a.context_id,
                                       campaign_id=(a.campaign_id or doc.get("campaign_id")),
                                       control_variables=doc.get("control_variables"))
        for _r in _echo_stop:
            print(f"[relay] ⚠ 정체성 echo: {_r} — `--continue` 는 이 상태에서 진행하지 않는다.")
    if getattr(a, "campaign_control", None) and _reached_sub(att):
        _camp = a.campaign_id or doc.get("campaign_id")
        _ok, _why = (orphan_sweep_echo_ok(report, a._sweep_ids) if getattr(a, "orphan_sweep", False) else
                     campaign_control_echo_ok(report, _camp, a.campaign_control,
                                              slice_doc=getattr(a, "_control_slice", None)))
        if _ok and getattr(a, "orphan_sweep", False):
            print(f"[relay] 수명 제어 · orphan sweep {_camp}@{a.node} · {_why} — 메인 인스턴스가 없어 journey 기록 ✗ "
                  f"(echo 원문은 원장 {lp} · docs 평면으로 옮겨라)")
        elif _ok:
            try:
                _echo = (report or {}).get("campaign_control_echo") or {}
                for _m in _campaign_init().record_sub_echo(_camp, node=a.node, control=a.campaign_control,
                                                           utc=ended, apply=True,
                                                           rev=(_echo.get("rev") if a.campaign_control == "reslice"
                                                                else None)):
                    print(f"[relay] 수명 제어 · {_m} · {_why}")
            except Exception as exc:                                    # noqa: BLE001 — 사유를 삼키지 않는다
                print(f"[relay] ⚠ 수명 제어 echo 기록 실패 — {exc} (서브 미수렴으로 남는다)")
        else:
            print(f"[relay] ⚠ 수명 제어 · 서브 미수렴 — {_why}. 메인은 진행한다(차단 ✗ · D9) — "
                  f"`--resume-brief` 첫 줄에 '서브 미수렴' 으로 남는다")
    if att["external_search"]:
        print(f"[relay] 서브가 외부검색 {len(att['external_search'])}건을 기록했다 — 자산화 후보. "
              f"근거는 원장 attempt {att['attempt']}.external_search 에 있다.")
    if hp:
        # 2026-09-08: 크래시 경로의 항목은 **메인이** 만든 것이다 — "서브가 표면화했다" 고 적으면
        #   서브가 규약대로 물어본 것과 구분되지 않는다(관측한 것만 적는다).
        _who = "메인이 사실을" if _crash else "서브가 요청을"
        print(f"[relay] ⚠ {_who} 표면화했다 → {hp} "
              "(`answer` 를 적고 `--continue` 로 재개. blocking 은 답 없이는 진행하지 않는다)")
    if att["end_reason"] == "runner_unavailable":
        _ev = att.get("runner_evidence") or {}
        print(f"[relay] ⚠ 러너 평면 실패 — {runner['name']}: {_ev.get('detail')} "
              f"(signal={_ev.get('signal')} status={_ev.get('api_error_status')} "
              f"provenance={_ev.get('provenance')} 회전가능={_ev.get('rotate')})")
        if _ev.get("message"):
            print(f"[relay]   백엔드가 남긴 말: {str(_ev['message'])[:300]}")
        return 5
    if att["budget_outcome"] == "exhausted":
        print(f"[relay] 호출 예산 소진. 다음 실행 예산을 선언하고 checkpoint부터 이어라; "
              f"관측과 남은 과업에 따라 필요하면 조정한다. context={a.context_id}")
        return 3
    if report is None:
        # 2026-09-05(N6): 종전 문구는 **검증하지 않은 원인**을 단정했다("산문만 왔다"). 라이브에서
        #   실제로는 stdout 이 비어 있었고(provider exit 5), 그 단정 때문에 나는 서브가 형식을
        #   어겼다고 읽었다. 관측된 것만 적는다 — 무엇이 왔는지, 제어가 뭐라고 했는지.
        _raw = result.get("output")
        if _raw is None or not str(_raw).strip():
            _what = "출력이 비었다(서브가 아무것도 돌려주지 않았거나 전송이 실패했다)"
        else:
            _what = "출력 %d자가 왔지만 그 안에서 JSON 객체를 찾지 못했다" % len(str(_raw))
        print("[relay] ⚠ 서브 리포트(JSON) 없음 — %s. control=%s codes=%s. 성공으로 집계하지 않는다."
              % (_what, att["control_status"], ",".join(att["reason_codes"]) or "-"))
        if control_stderr:
            print("[relay]   provider 진단(끝 400자): %s" % control_stderr[-400:])
        print("[relay]   원문 보존: %s" % att["report_path"])
        return 4
    return 0 if att["status"] == "completed" else 2


def capacity_policy(a, doc, lp, bud):
    values = [getattr(a, k, None) for k in ("capacity_retries", "capacity_backoff_seconds",
              "capacity_deadline_utc", "capacity_approval_file")]
    inherited = getattr(a, "_capacity_resume_policy", None)
    if not any(v is not None for v in values) and not inherited:
        return None
    if inherited and not any(v is not None for v in values):
        retries, delay = inherited.get("retries"), inherited.get("backoff_seconds")
        approval = {"authorization": inherited.get("authorization"), "source": inherited.get("source")}
        if (not isinstance(retries, int) or isinstance(retries, bool) or not 1 <= retries <= 10
                or not isinstance(delay, int) or isinstance(delay, bool)
                or not 1 <= delay <= turn_budget.schema_cap("timeout_seconds")):
            raise SystemExit("[relay] STOP: invalid inherited capacity burst bounds")
        expiry = datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(
            seconds=(retries + 1) * bud["timeout_seconds"] + retries * delay)
        not_after = inherited.get("not_after_utc", inherited.get("deadline_utc"))
        try:
            independent_deadline = datetime.datetime.strptime(not_after, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)
        except (TypeError, ValueError):
            raise SystemExit("[relay] STOP: invalid recorded capacity not-after deadline")
        expiry = min(expiry, independent_deadline)
        deadline = expiry.strftime("%Y-%m-%dT%H:%M:%SZ")
    else:
        if any(v is None for v in values):
            raise SystemExit("[relay] STOP: capacity policy requires all four options")
        retries, delay, deadline, file = values
        not_after = deadline
        if not 1 <= retries <= 10 or not 1 <= delay <= turn_budget.schema_cap("timeout_seconds"):
            raise SystemExit("[relay] STOP: invalid capacity retry bounds")
        try:
            expiry = datetime.datetime.strptime(deadline, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)
            approval = json.loads(Path(file).read_text(encoding="utf-8"))
        except (OSError, ValueError, UnicodeError):
            raise SystemExit("[relay] STOP: invalid capacity deadline or approval file")
    if (not isinstance(approval, dict) or set(approval) != {"authorization", "source"}
            or any(not isinstance(approval[k], str) or not approval[k].strip() for k in approval)):
        raise SystemExit("[relay] STOP: capacity approval needs verbatim authorization and source")
    if doc.get("inflight"):
        raise SystemExit("[relay] STOP: cannot start capacity burst with inflight dispatch")
    if (expiry - datetime.datetime.now(datetime.timezone.utc)).total_seconds() < bud["timeout_seconds"]:
        raise SystemExit("[relay] STOP: insufficient capacity burst deadline")
    return {"burst_id": uuid.uuid4().hex, "retries": retries, "backoff_seconds": delay,
            "deadline_utc": deadline, "not_after_utc": not_after,
            "authorization": approval["authorization"], "source": approval["source"], "start_attempt": len(doc.get("attempts") or []),
            "allocated_slots": 1 + retries}


def capacity_deadline_fits(policy, bud):
    deadline = datetime.datetime.strptime(policy["deadline_utc"], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)
    return (deadline - datetime.datetime.now(datetime.timezone.utc)).total_seconds() >= bud["timeout_seconds"]


def capacity_handoff_reentry(a, doc, runner, policy, resume_declared):
    """Spend an existing slot only on an explicit completed technical handoff."""
    attempts = doc.get("attempts") or []
    if (doc.get("inflight")
            or not getattr(a, "continue_", False) or not getattr(a, "apply", False)
            or not getattr(a, "task_file", None) or not attempts):
        return False
    last = attempts[-1]
    session = last.get("session_id")
    if (last.get("control_status") != "completed" or last.get("status") != "input-required"
            or last.get("end_reason") != "sub_input_required" or not last.get("sub_reported")
            or codex_recovery_requirement(last) or not session
            or resume_declared != session or getattr(a, "resume", None) != session
            or last.get("resume_honored") is False or last.get("runner") != runner
            or continuation_stop_reasons(doc, historical_control_variables=getattr(a, "_historical_control_variables", None))
            or resume_blockers(pending_for(a.repo_root, doc.get("context_id")))):
        return False
    try:
        historical_doc = dict(doc)
        if getattr(a, "_historical_control_variables", None) is not None:
            historical_doc["control_variables"] = a._historical_control_variables
        req = _instruction_request(a.repo_root, historical_doc, last)
        binding = last.get("session_binding") or {}
        reservation = {"outcome_path": last["outcome_path"], "request_path": last["request_path"],
                       "request_digest": last["request_digest"], "dispatch_id": last["dispatch_id"],
                       "target": req["target"], "provider": req["provider"]}
        issued, result = load_dispatch_outcome(a.repo_root, {"inflight": reservation})
        expected_report = str(Path("campaigns") / (doc.get("campaign_id") or "_bootstrap") / "relay"
                              / (doc["context_id"] + ".reports") / ("attempt-%02d.json" % last["attempt"]))
        if last.get("report_path") != expected_report or issued != req:
            return False
        wrapped = json.loads(_owned_artifact(a.repo_root, expected_report).read_text(encoding="utf-8"))
        if not isinstance(wrapped, dict):
            return False
        report = wrapped.get("report")
        report_schema = _agent_control._load_schema(_agent_control.RUNTIME_DIR.parent / "sub_node" / "task-report.schema.json")
        if (wrapped.get("attempt") != last["attempt"] or not isinstance(report, dict)
                or _agent_control._schema_violations(report, report_schema)
                or parse_report(result.get("output") or "") != report
                or result.get("status") != "completed" or result.get("session_id") != session
                or result.get("raw_exit_code") != last.get("raw_exit_code")
                or result.get("reason_codes") != last.get("reason_codes")
                or wrapped.get("control") != {k: result.get(k) for k in
                    ("status", "exit_code", "reason_codes", "session_id", "num_turns", "budget_outcome",
                     "duration_ms", "duration_api_ms", "raw_exit_code", "execution_started", "usage",
                     "failure_evidence", "raw_events")}
                or any(last.get(ledger_key) != report.get(report_key) for ledger_key, report_key in
                       (("status", "status"), ("phase", "phase"), ("context_id_reported", "context_id"),
                        ("campaign_id_reported", "campaign_id"), ("control_variables_echo", "control_variables_echo"),
                        ("running_services", "running_services")))):
            return False
        return (req.get("provider") == runner.get("provider") == "codex_cli"
                and req.get("model") == runner.get("model")
                and req.get("reasoning_effort") == getattr(a, "reasoning_effort", None)
                and binding.get("permission_profile") == getattr(a, "permission_profile", None)
                and binding.get("sandbox_mode") == getattr(a, "sandbox_mode", None))
    except (SystemExit, OSError, ValueError, TypeError, KeyError):
        return False


def native_budget_exhaustion_ready(repo_root, doc):
    """Accept only a durable provider-reported native budget exhaustion."""
    last = (doc.get("attempts") or [None])[-1] or {}
    if (last.get("provider") != "claude_code" or last.get("end_reason") != "budget_exhausted"
            or not last.get("session_id") or last.get("resume_honored") is False
            or last.get("permission_denials") or continuation_stop_reasons(doc)):
        return False
    try:
        req = _instruction_request(repo_root, doc, last)
        reservation = {"outcome_path": last["outcome_path"], "request_path": last["request_path"],
                       "request_digest": last["request_digest"], "dispatch_id": last["dispatch_id"],
                       "target": req["target"], "provider": req["provider"]}
        issued, result = load_dispatch_outcome(repo_root, {"inflight": reservation})
        return (issued == req and req.get("provider") == "claude_code"
                and permission_binding(req) == last.get("session_binding")
                and result.get("session_id") == last.get("session_id")
                and result.get("budget_outcome") == "exhausted"
                and not any(code in ("PERMISSION_DENIED", "TIMEOUT")
                            for code in result.get("reason_codes") or []))
    except (OSError, ValueError, KeyError, TypeError, SystemExit):
        return False


def controlled_deadline_ready(repo_root, doc):
    """Verify the owned worker was reaped; unfinished tools require reconciliation."""
    last = (doc.get("attempts") or [None])[-1] or {}
    binding = last.get("session_binding") or {}
    if (not last.get("session_id") or last.get("resume_honored") is False or
            (last.get("failure_evidence") or {}).get("signal") not in ("controlled_local_deadline", "controlled_remote_deadline")):
        return False
    try:
        req = _instruction_request(repo_root, doc, last)
        reservation = {"outcome_path": last["outcome_path"], "request_path": last["request_path"],
                       "request_digest": last["request_digest"], "dispatch_id": last["dispatch_id"],
                       "target": req["target"], "provider": req["provider"]}
        issued, result = load_dispatch_outcome(repo_root, {"inflight": reservation})
        signal_kind = (result.get("failure_evidence") or {}).get("signal")
        if (issued != req or permission_binding(req) != binding or result.get("status") != "timeout" or
                result.get("session_id") != last["session_id"] or
                signal_kind != (last.get("failure_evidence") or {}).get("signal") or
                (signal_kind == "controlled_local_deadline" and req["target"].get("transport") != "local") or
                (signal_kind == "controlled_remote_deadline" and req["target"].get("transport") != "ssh")):
            return False
        if signal_kind == "controlled_remote_deadline":
            provider = _agent_control._load_provider("codex_cli")
            events = result.get("raw_events") or []
            if not events or events[-1] != {"type": "adapter.deadline_reaped", "request_digest": provider._request_digest(req)}:
                return False
        if signal_kind not in ("controlled_local_deadline", "controlled_remote_deadline"):
            return False
        open_commands = set()
        for event in result.get("raw_events") or []:
            item = event.get("item") if isinstance(event, dict) else None
            if not isinstance(item, dict) or item.get("type") != "command_execution":
                continue
            ident = item.get("id")
            if not isinstance(ident, str) or not ident:
                return False
            if event.get("type") == "item.completed" and item.get("status") not in ("in_progress", "running"):
                open_commands.discard(ident)
            elif event.get("type") in ("item.started", "item.updated"):
                open_commands.add(ident)
            else:
                return False
        # An unfinished tool is evidence to reconcile, not evidence of completion.
        # The next task carries the mandatory reconciliation-first instruction.
        return True
    except (OSError, ValueError, KeyError, TypeError, SystemExit):
        return False


def capacity_retry_ready(a, doc, bud, runner, policy, *, check_deadline=True):
    if not policy or doc.get("inflight"):
        return False
    attempts = doc.get("attempts") or []
    if len(attempts) - policy["start_attempt"] >= policy["allocated_slots"]:
        return False
    last = attempts[-1] if attempts else {}
    if ((last.get("failure_evidence") or {}).get("signal") != "provider_capacity"
            or not last.get("session_id") or last.get("resume_honored") is False
            or last.get("runner") != runner or runner.get("provider") != "codex_cli"):
        return False
    if continuation_stop_reasons(doc, allow_control_recovery=True):
        return False
    try:
        path = _owned_artifact(a.repo_root, last["outcome_path"])
        wrapper = json.loads(path.read_text(encoding="utf-8"))
        req, result = wrapper["request"], wrapper["result"]
        request_path = _owned_artifact(a.repo_root, last["request_path"])
        if (hashlib.sha256(request_path.read_bytes()).hexdigest() != last.get("request_digest")
                or json.loads(request_path.read_text(encoding="utf-8")) != req
                or result.get("session_id") != last.get("session_id")
                or (result.get("failure_evidence") or {}).get("signal") != "provider_capacity"):
            return False
        if (wrapper.get("schema_version") != 1 or set(wrapper) != {"schema_version", "dispatch_id", "request", "result"}
            or _agent_control.request_schema_violations(req)
            or _agent_control._schema_violations(result, _agent_control._load_schema(_agent_control.RESULT_SCHEMA_PATH))
            or _agent_control._result_contract_violations(result, req)
            or result.get("provider") != req.get("provider")
            or result.get("model_requested") != req.get("model")
            or result.get("status") != "execution_failed"
            or result.get("status") != last.get("control_status")
            or result.get("raw_exit_code") != last.get("raw_exit_code")
            or result.get("reason_codes") != last.get("reason_codes")
            or wrapper.get("dispatch_id") != last.get("dispatch_id") or
            permission_binding(req) != last.get("session_binding") or
            req.get("model") != runner["model"] or
            req.get("reasoning_effort") != getattr(a, "reasoning_effort", None)):
            return False
        open_commands = set()
        for event in result.get("raw_events") or []:
            item = event.get("item") if isinstance(event, dict) else None
            if not isinstance(item, dict) or item.get("type") != "command_execution":
                continue
            ident = item.get("id")
            if not isinstance(ident, str) or not ident:
                return False
            if event.get("type") == "item.completed":
                if item.get("status") in ("in_progress", "running"):
                    return False
                open_commands.discard(ident)
            elif event.get("type") in ("item.started", "item.updated"):
                open_commands.add(ident)
            else:
                return False
        if open_commands:
            return False
        deadline = datetime.datetime.strptime(policy["deadline_utc"], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)
        if check_deadline and (deadline - datetime.datetime.now(datetime.timezone.utc)).total_seconds() < policy["backoff_seconds"] + bud["timeout_seconds"]:
            return False
        pending = pending_for(a.repo_root, doc.get("context_id"))
        recovery = codex_recovery_requirement(last)
        if not recovery or any(entry_blocking(e) and e.get("recovery_required") != recovery and not e.get("answer") for e in pending):
            return False
        return True
    except (OSError, ValueError, KeyError, TypeError, SystemExit):
        return False


def capacity_answer(repo_root, context_id, last, policy):
    path = os.path.join(relay_root(repo_root), PENDING_HITL)
    with state_lock(path):
        doc = json.loads(Path(path).read_text(encoding="utf-8"))
        matches = [e for e in doc.get("pending", []) if e.get("context_id") == context_id
                   and e.get("recovery_required") == codex_recovery_requirement(last)]
        if len(matches) != 1 or matches[0].get("answer"):
            raise SystemExit("[relay] STOP: capacity recovery pending does not match")
        matches[0]["answer"] = policy["authorization"]
        matches[0]["answer_source"] = policy["source"]
        _atomic_json(path, doc)


def run_attempt(a, doc: dict, lp: str, task: str, bud: dict, resume_declared) -> int:
    """사다리를 **회전하며** 한 attempt 를 성립시킨다.

    회전의 실행자는 여기다(헌법 노드제어 ③ "처방을 누가 실행하는가를 먼저 적는다"). 사람 승인은
    사다리 **선언** 시점에 이미 받았으므로, 회전은 무인 자동 착수가 아니라 *승인된 attempt 의
    실행자 교체*다 — 감독 스텝의 자동 재발급과 같은 논리다.

    The original resume declaration is retained as evidence. bound_resume preserves it only
    for a matching provider/target/permission binding; a provider change starts a fresh
    native session using the artifact continuation, never a failed runner's empty session.
    """
    ladder = a.ladder
    requested_policy = bool(getattr(a, "_capacity_resume_policy", None)) or any(
        getattr(a, k, None) is not None for k in
        ("capacity_retries", "capacity_backoff_seconds", "capacity_deadline_utc", "capacity_approval_file"))
    if requested_policy and (len(ladder) != 1 or ladder[0].get("provider") != "codex_cli"):
        raise SystemExit("[relay] STOP: capacity retry requires one Codex runner")
    policy = capacity_policy(a, doc, lp, bud)
    prior = (doc.get("attempts") or [None])[-1]
    if (policy and prior and prior.get("status") == "input-required" and getattr(a, "task_file", None)
            and not capacity_handoff_reentry(a, doc, ladder[0], policy, resume_declared)):
        raise SystemExit("[relay] STOP: explicit handoff lacks durable same-session evidence")
    a._prepared_capacity_policy = policy
    start = next_runner_index(doc, ladder)
    if len(ladder) > 1:
        print(f"[relay] 사다리 {len(ladder)}칸: {ladder_names(ladder)} · 시작 칸 "
              f"{ladder[start]['name']}")
    tried = []
    for step in range(len(ladder)):
        runner = ladder[(start + step) % len(ladder)]
        last = (doc.get("attempts") or [None])[-1]
        if (policy and not a.emit_only and last and (last.get("failure_evidence") or {}).get("signal") == "provider_capacity"):
            if not capacity_retry_ready(a, doc, bud, runner, policy):
                raise SystemExit("[relay] STOP: capacity continuation lacks session, evidence, or remaining wall time")
            if resume_declared != last.get("session_id"):
                raise SystemExit("[relay] STOP: capacity continuation must resume the same session")
            time.sleep(policy["backoff_seconds"])
            if not capacity_retry_ready(a, doc, bud, runner, dict(policy, backoff_seconds=0)):
                raise SystemExit("[relay] STOP: capacity continuation deadline or evidence changed")
            pending = pending_for(a.repo_root, a.context_id)
            if any(e.get("recovery_required") == codex_recovery_requirement(last) and not e.get("answer") for e in pending):
                capacity_answer(a.repo_root, a.context_id, last, policy)
            a.continue_, a.apply, a.resume = True, True, resume_declared
            require_dispatch_recovery(a, doc, resume_declared)
            a._continuation_source_attempt = continuation_instruction(a.repo_root, doc)["attempt"]
            task = ("먼저 기존 실행 중 서비스와 명령 및 산출물을 확인하고 부분 파일을 보존하라. "
                    "이미 실행된 벤치를 다시 시작하거나 부분 파일을 덮어쓰지 마라.\n" +
                    assemble_continuation(a.repo_root, doc, pending_for(a.repo_root, a.context_id)))
        rc = run_attempt_once(a, doc, lp, task, bud, resume_declared, runner)
        if a.emit_only:
            return rc
        a._historical_control_variables = None
        last = (doc.get("attempts") or [])[-1]
        while capacity_retry_ready(a, doc, bud, runner, policy):
            time.sleep(policy["backoff_seconds"])
            if not capacity_retry_ready(a, doc, bud, runner, dict(policy, backoff_seconds=0)):
                break
            capacity_answer(a.repo_root, a.context_id, last, policy)
            a.continue_, a.apply, a.resume = True, True, last["session_id"]
            require_dispatch_recovery(a, doc, a.resume)
            a._continuation_source_attempt = continuation_instruction(a.repo_root, doc)["attempt"]
            task = ("먼저 기존 실행 중 서비스와 명령 및 산출물을 확인하고 부분 파일을 보존하라. "
                    "이미 실행된 벤치를 다시 시작하거나 부분 파일을 덮어쓰지 마라.\n" +
                    assemble_continuation(a.repo_root, doc, pending_for(a.repo_root, a.context_id)))
            rc = run_attempt_once(a, doc, lp, task, bud, a.resume, runner)
            last = (doc.get("attempts") or [])[-1]
        if (policy and (last.get("failure_evidence") or {}).get("signal") == "provider_capacity"):
            probe = dict(policy, allocated_slots=len(doc.get("attempts") or []) - policy["start_attempt"] + 1)
            if capacity_retry_ready(a, doc, bud, runner, probe, check_deadline=False):
                pending = pending_for(a.repo_root, a.context_id)
                if any(e.get("recovery_required") == codex_recovery_requirement(last) and not e.get("answer") for e in pending):
                    capacity_answer(a.repo_root, a.context_id, last, policy)
                doc["capacity_deferred"] = {"dispatch_id": last["dispatch_id"], "burst_id": policy["burst_id"]}
                save_ledger(lp, doc)
                print("[relay] capacity burst deferred; a later bounded burst may continue the same session")
                return 2
        if last.get("end_reason") != "runner_unavailable":
            return rc
        ev = last.get("runner_evidence") or {}
        tried.append("%s(%s)" % (runner["name"], ev.get("detail") or ev.get("signal")))
        if ev.get("rotate") is not True or last.get("execution_started") is True:
            # 회전해도 낫지 않는 실패(요청 거절·판정 불가)는 사다리를 태우지 않는다.
            print(f"[relay] ⚠ 회전하지 않는다 — {runner['name']} 의 실패는 러너를 바꿔도 같다. "
                  f"사람이 봐야 한다.")
            return rc
        if step + 1 < len(ladder):
            print(f"[relay] ↻ 회전 {step + 1}/{len(ladder) - 1}: {runner['name']} → "
                  f"{ladder[(start + step + 1) % len(ladder)]['name']} "
                  f"(같은 과업·같은 예산·같은 재개 선언 — 회전은 예산 사건이 아니다)")
    # ── 소진: 한 바퀴를 다 돌았는데 전부 러너 평면에서 실패했다 → HITL(사용자 규격)
    hp = surface_requests(a.repo_root, a.context_id, {}, attempt=len(doc.get("attempts") or []),
                          crash={"attempt": len(doc.get("attempts") or []),
                                 "end_reason": "runner_ladder_exhausted",
                                 "control_status": "execution_failed",
                                 "reason_codes": [RUNNER_UNAVAILABLE],
                                 "blocking": True,
                                 "runner_ladder": [r["name"] for r in ladder],
                                 "runner_attempts": tried,
                                 "prompt": (
                                     "러너 사다리가 소진됐다 — 선언된 %d칸(%s)이 **전부 실행되지 "
                                     "않았다**. 서브가 기동되지 않는 것이고, 예산도 과업도 아니다.\n"
                                     "칸별 관측: %s\n"
                                     "마지막 알려진 세션 = %s.\n"
                                     "→ 한도 리셋을 기다릴지, 사다리에 칸을 더할지, 다른 노드로 "
                                     "옮길지는 사람이 정한다. 답을 `answer` 에 적으면 잇는다."
                                     % (len(ladder), ladder_names(ladder), " · ".join(tried),
                                        last_known_session(doc)["session_id"] or "(없음)")),
                                 "last_known_session": last_known_session(doc)["session_id"]})
    print(f"[relay] ⛔ 러너 사다리 소진 — {len(ladder)}칸 전부 실행하지 못했다:")
    for t in tried:
        print(f"[relay]     · {t}")
    print("[relay]   서브가 기동되지 않는다. 이것은 예산도 과업도 아닌 **실행자 평면**의 문제다.")
    if hp:
        print(f"[relay]   차단성 HITL 표면화 → {hp} (답을 적기 전까지 --continue 는 진행하지 않는다)")
    return 3


def _main() -> int:
    global _RELAY_CAMPAIGN
    ap = argparse.ArgumentParser(description="메인↔서브 턴제 릴레이")
    ap.add_argument("--topology", choices=["single", "multi"])
    ap.add_argument("--manifest", default=None)
    ap.add_argument("--context-id")
    ap.add_argument("--max-turns", type=int, default=None,
                    help="이 attempt 에 배정할 턴 예산(선언 필수 — 등급표 폐기 · 기본값 없음)")
    ap.add_argument("--provider", choices=("claude_code", "codex_cli"), default=None)
    ap.add_argument("--max-dispatches", type=int, default=None,
                    help="구 호출 호환 인자; 누적 호출 상한으로 적용하지 않음")
    permissions = ap.add_mutually_exclusive_group()
    permissions.add_argument("--permission-profile")
    permissions.add_argument("--sandbox-mode", choices=("read-only", "workspace-write"))
    ap.add_argument("--timeout-seconds", type=int, default=None,
                    help="매달림 상한(scope ⊥ budget — 예산과 별개 노브)")
    ap.add_argument("--budget-source", default=None,
                    help="그 예산을 그렇게 정한 근거(필수 · 원장에 남는다)")
    ap.add_argument("--backend", choices=_canary.backend_choices(), default=None,
                    help="provider 실행 백엔드(생략 = anthropic). 선택지는 전송 스키마 enum 에서 읽는다. "
                         "별칭으로 부르려면 --runners(예: meta-claude)")
    ap.add_argument("--capacity-retries", type=int)
    ap.add_argument("--capacity-backoff-seconds", type=int)
    ap.add_argument("--capacity-deadline-utc")
    ap.add_argument("--capacity-approval-file")
    ap.add_argument("--reasoning-effort", choices=("none", "minimal", "low", "medium", "high", "xhigh", "max"), default=None)
    ap.add_argument("--model", default=None,
                    help="위임 모델 선언(기본은 카나리 기본값). 어댑터는 모델로 차단하지 않는다 — "
                         "실제로 돈 모델은 원장 `model_used` 가 말한다(2026-09-05 · G-A1).")
    ap.add_argument("--runners", default=None, metavar="a,b,c",
                    help="러너 **사다리**(순서 있는 목록 · 쉼표 구분). 예: "
                         "`kimi-claude,minimax-claude,sonnet,haiku`. 러너 = (backend, model) 한 쌍이며 "
                         "아는 이름은 `agent_control.py runners` 가 낸다. 러너 평면 실패(백엔드 한도·"
                         "인증·미도달·바이너리 부재)에서 다음 칸으로 **회전**하고, 한 바퀴를 다 돌면 "
                         "차단성 HITL 로 끊는다. 생략 시 1칸(sonnet) = 이 기능 도입 전과 동작 동일. "
                         "--backend/--model 과 상호배타.")
    ap.add_argument("--resume", default=None, metavar="SESSION_ID|new",
                    help="이어받을 provider 세션 id, 또는 새 세션이면 `new`. **선언 필수** — "
                         "코드가 대신 정하지 않는다(2026-09-05 · 축 F).")
    ap.add_argument("--task", help="서브에 보낼 지시(또는 --task-file)")
    ap.add_argument("--task-file")
    ap.add_argument("--handoff-issue", help="반환 결과에서 발견한 실제 인계 문제와 근거 위치; 명시적인 정정 Task와 함께 사용")
    ap.add_argument("--continue", dest="continue_", action="store_true",
                    help="원장의 중단점에서 이어간다 — 본문을 기계가 조립한다. 기본 dry-run.")
    ap.add_argument("--apply", action="store_true",
                    help="--continue 와 함께: 조립한 본문으로 실제 위임한다(사람 승인 정문).")
    ap.add_argument("--reconcile-inflight", action="store_true",
                    help="consume only the reserved durable provider outcome; never launch a replacement worker")
    ap.add_argument("--reconcile-result", help="optional exact reserved outcome path; defaults to inflight.outcome_path")
    ap.add_argument("--emit-only", action="store_true", help="request 만 조립해 출력(실행 ✗)")
    ap.add_argument("--repo-root", default=REPO)
    # 2026-09-07(plan_26090715 §4.8): 캠페인 정체성은 **요청에 실려 간다**. 생략하면 활성
    #   캠페인에서 파생하고, `_bootstrap`(캠페인 밖)이면 싣지 않는다.
    ap.add_argument("--campaign-id", default=None,
                    help="위임에 실을 캠페인 id(생략 시 활성 캠페인 · _bootstrap 이면 미탑재)")
    ap.add_argument("--no-campaign", action="store_true",
                    help="캠페인 축을 싣지 않는다(캠페인 밖 온보딩·카나리)")
    # ── 캠페인 축(2026-09-08 · plan_26090813 §4.3). 이름에서 추론하지 않고 **선언**한다.
    ap.add_argument("--node", default=None,
                    help="이 context 를 수행하는 노드 id(원장에 못박힌다 · P4 가 읽는다)")
    ap.add_argument("--context-kind", choices=list(CONTEXT_KINDS), default=None,
                    help="build = 빌드 문맥 · cell = 셀 하나(서빙→벤치). 셀 하나 = context 하나(D20)")
    ap.add_argument("--cell", default=None, help="--context-kind cell 의 셀 id(원장에 못박힌다)")
    ap.add_argument("--budget-from-phase", action="store_true",
                    help="--timeout-seconds 를 같은 노드의 지난 phase 실측에서 파생한다 "
                         "(실측이 없으면 스키마 상한을 쓰고 그 사실을 근거에 적는다)")
    ap.add_argument("--supervise-step", metavar="CAMP_ID", default=None,
                    help="감독 한 걸음 — 원장+회수 브리핑을 읽어 판정하고 원장에 적고 끝난다. "
                         "상주 ✗(세션 리볼빙 · D7). --apply 면 전진이 보이는 중단을 자동 재발급한다")
    ap.add_argument("--cost-cap-attempts", type=int, default=None,
                    help="구 호출 호환 인자; 누적 호출 상한으로 적용하지 않음")
    ap.add_argument("--utc", default=None, help="감독 스텝 기록 시각(주입 · 생략 시 메인 벽시계)")
    ap.add_argument("--campaign-control", choices=tuple(CAMPAIGN_CONTROLS), default=None,
                    help="수명 전이(보류·재개·종결·포기)를 서브에 A2A 로 전한다 · --campaign-id --node 필수 · 지시 본문은 "
                         "기계가 조립한다(--task 생략) · echo 가 수렴하면 메인 journey 에 sub_control_echo 를 적는다")
    ap.add_argument("--orphan-sweep", action="store_true",
                    help="--campaign-control abandon 과 함께: 메인 인스턴스가 없는 서브 orphan 묶음 정리(--campaign-id a,b,… · "
                         "부재 선언 · --reason 필수)")
    ap.add_argument("--reason", default=None, help="--campaign-control abandon 의 포기 사유(생략 = 메인 종결 줄의 note)")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if a.task and a.task_file:
        ap.error("--task and --task-file are mutually exclusive")
    if a.continue_ and a.task:
        ap.error("--continue accepts --task-file, not --task")
    if a.handoff_issue is not None:
        if not a.handoff_issue.strip() or not (a.task or a.task_file):
            ap.error("--handoff-issue requires an observed issue and an explicit correction task")
    if a.supervise_step and (a.task or a.task_file):
        ap.error("--supervise-step does not accept task input")
    if a.reconcile_result and not a.reconcile_inflight:
        raise SystemExit("[relay] --reconcile-result requires --reconcile-inflight")
    if a.reconcile_inflight and (a.continue_ or a.task or a.task_file or a.supervise_step or a.campaign_control):
        raise SystemExit("[relay] reconciliation cannot be combined with a dispatch or supervision action")
    # ── 사다리 해소. 두 선언 창구가 동시에 열려 있으면 **어느 쪽이 이겼는지 조용해진다**.
    if a.runners and (a.backend or a.model or a.provider):
        raise SystemExit("[relay] STOP: --runners 와 --backend/--model 은 같은 것을 정하는 "
                         "두 창구다 — 하나만 선언하라(사다리를 쓸 거면 --runners).")
    a.ladder = resolve_ladder(a.repo_root, a.runners, backend=a.backend, model=a.model, provider=a.provider)
    if a.supervise_step:
        return supervise_step(a)
    if not a.context_id:
        raise SystemExit("[relay] --context-id 필수")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", a.context_id):
        raise SystemExit("[relay] STOP: context-id must be a single safe filename component")

    if a.reconcile_inflight and a.campaign_id:
        _RELAY_CAMPAIGN = a.campaign_id
    lp = ledger_path(a.repo_root, a.context_id)
    doc = load_ledger(lp)
    if a.reconcile_inflight:
        return reconcile_inflight(a, doc, lp)
    bind_campaign_context(doc, node=a.node, kind=a.context_kind, cell=a.cell)
    if a.runners:
        # 선언된 사다리를 원장에 적는다 — 감독 재발급이 명시 없이도 같은 사다리를 잇는다(ledger_runner_spec).
        doc["runner_ladder"] = ",".join(r["name"] for r in a.ladder)
    a.campaign_id, a.control_variables = resolve_campaign_axis(a)
    old_axis = doc.get("control_variables")
    if (a.continue_ and doc.get("attempts") and a.control_variables and old_axis != a.control_variables):
        if not a.task_file:
            raise SystemExit("[relay] STOP: revised campaign controls require a complete explicit --task-file")
        require_official_axis_revision(a.repo_root, a.campaign_id, old_axis, a.control_variables)
        previous = _last_reached(doc)
        if not previous:
            raise SystemExit("[relay] STOP: revised controls have no prior reached checkpoint")
        issued = _instruction_request(a.repo_root, doc, previous)
        if issued.get("control_variables") != old_axis or issued.get("campaign_id") != a.campaign_id:
            raise SystemExit("[relay] STOP: prior checkpoint request does not bind its historical campaign axis")
        a._historical_control_variables = dict(old_axis or {})
    if a.campaign_id:
        doc["campaign_id"] = a.campaign_id
    if a.control_variables:
        doc["control_variables"] = a.control_variables

    if (a.continue_ or a.task or a.task_file) and reuse_completed_handoff(a, doc, lp):
        return 0

    if a.continue_:
        # 이어가기에 필요한 것은 전부 **원장**에서 온다 — 사람이 다시 적지 않는다.
        if not (doc.get("attempts") or []):
            raise SystemExit(f"[relay] STOP: 이어갈 원장이 없다 — {lp}. 첫 위임은 `--task` 로 연다.")
        ensure_recovery_pending(a.repo_root, doc)
        a.topology = a.topology or doc.get("topology")
        if not (a.runners or a.backend or a.model or a.provider) and ledger_runner_spec(doc):
            a.ladder = resolve_ladder(a.repo_root, ledger_runner_spec(doc))   # 러너를 조용히 바꾸지 않는다
        if a.reasoning_effort is None:
            a.reasoning_effort = doc.get("reasoning_effort")
        if not (a.permission_profile or a.sandbox_mode):
            for key, value in (doc.get("permission_selection") or {}).items():
                setattr(a, key, value)
        if not a.topology:
            raise SystemExit("[relay] STOP: 원장에 topology 가 없다(구버전 원장) — 명시하라.")
        a.manifest_path = a.manifest or doc.get("manifest") or os.path.join(
            REPO, "output", a.topology, "manifest.yaml")

        # ── 정지 조건. 예산·전진 수는 **보여주고**, 멈출지는 --apply(승인 정문)가 정한다.
        stalled = stalled_attempts(doc)
        last = next((x for x in reversed(doc["attempts"]) if _reached_sub(x)), None)
        if (last and last.get("status") == "completed" and not codex_recovery_requirement(last)
                and not a.handoff_issue and not getattr(a, "_historical_control_variables", None)):
            raise SystemExit("[relay] STOP: 직전 attempt 가 completed 다 — 이을 중단점이 없다.\n"
                             "  → 새 과업이면 새 --context-id 로 `--task` 를 연다.")
        if last:
            # ── 고아 서비스 게이트(2026-09-07 · 유예 결함 ⑦) ──────────────────────────────
            #   예산 소진으로 끊긴 턴이 서브에 컨테이너를 남겼는데 그것을 모른 채 다음 턴을 열면
            #   두 서빙이 같은 호스트 메모리를 두고 다툰다(하드다운 계보가 있는 축이다).
            #   `running_services` 가 **비어 있지 않으면** 회신 없이 잇지 않는다.
            _svcs = last.get("running_services")
            if isinstance(_svcs, list) and _svcs:
                _names = ", ".join(str(x.get("name")) for x in _svcs if isinstance(x, dict))
                raise SystemExit(
                    f"[relay] STOP: 직전 턴이 서비스를 남긴 채 끝났다 — {_names}\n"
                    f"  그 상태로 다음 턴을 열면 두 서빙이 같은 호스트 메모리를 두고 다툰다.\n"
                    f"  → 서브에 정리를 지시하거나(A2A 제어명령), 의도적 상주면 그 사실을\n"
                    f"    `--task` 본문에 적고 **새 context** 로 열어라(우회로 잇지 않는다).")
            if (last.get("budget_outcome") == "exhausted"
                    and last.get("running_services") is None):
                print("[relay] ⚠ 직전 턴이 예산을 소진했는데 running_services 를 말하지 않았다 — "
                      "고아 서비스 여부가 **모름**이다. 서브 규약(task-report.running_services)을 "
                      "따르게 하라.", file=sys.stderr)
            _stop = echo_stop_reasons(last, context_id=a.context_id,
                                      campaign_id=(a.campaign_id or doc.get("campaign_id")),
                                      control_variables=getattr(a, "_historical_control_variables", doc.get("control_variables")))
            if _stop:
                raise SystemExit(
                    "[relay] STOP: 서브 정체성 echo 가 성립하지 않는다 — 부재도 불일치도 STOP 이다.\n"
                    + "".join(f"  - {r}\n" for r in _stop)
                    + "  → 서브가 위임 헤더의 값을 그대로 회신하도록 한 뒤 재개하라(우회 ✗).")
        # 2026-10-19 · plan_26101912: 서고 교환은 은퇴 — 서브는 접근자로 직접 묻는다(릴레이가 사서를 나르지 않는다).
        pend = pending_for(a.repo_root, a.context_id)
        blocked = resume_blockers(pend)
        if blocked:
            ids = ", ".join(str(e.get("request_id")) for e in blocked)
            raise SystemExit(
                f"[relay] STOP: 답이 필요한 **차단성** 요청이 있다 — {ids}\n"
                f"  → {relay_root(a.repo_root)}/{PENDING_HITL} 의 해당 항목 `answer` 에 실제 사용자 답을 기록하라.\n"
                "  → 답의 존재는 승인이 아니다. 사용자의 재개 의사를 확인한 경우에만 --continue 하라.\n"
                "  → Codex 미해결 실패의 명시적 수동 복구 조건은 그대로 적용된다.")

        lks = last_known_session(doc)
        if a.task_file:
            with open(a.task_file, encoding="utf-8", newline="") as f:
                task = f.read()
            if not task.strip():
                raise SystemExit("[relay] STOP: --task-file is empty")
        else:
            a._continuation_source_attempt = continuation_instruction(a.repo_root, doc)["attempt"]
            task = assemble_continuation(a.repo_root, doc, pend)
        if a.handoff_issue:
            task = "[실제 인계 문제의 정정·설명 요청] " + a.handoff_issue + "\n" + task
        _last_out = (last or {}).get("budget_outcome")
        if not a.apply:
            # dry-run 은 **사실만** 보여준다 — 예산도 재개도 이 화면을 본 쪽이 선언한다.
            print(f"[relay] --continue DRY-RUN · context={a.context_id}")
            print(f"[relay] 마지막 알려진 세션: {lks['session_id'] or '(없음)'} "
                  f"(attempt {lks['attempt']} · sub_status={lks['status']} · "
                  f"budget={lks['budget_outcome']}) → `--resume <id|new>` 로 선언하라")
            budget_text, budget_flags = continuation_budget_preview(doc, a.ladder, last, a.timeout_seconds)
            print(f"[relay] 예산 사실: {budget_text} · 직전 결과 {_last_out} · 대기 요청 {len(pend)}건 · "
                  f"전진 없는 연속 attempt {stalled}회")
            if _last_out == "exhausted":
                print("[relay]   직전 호출은 소진됐다 — 새 실행 예산을 선언하고 checkpoint부터 이어라. "
                      "관측과 남은 과업에 따라 필요하면 조정한다.")
            print("─" * 72)
            sys.stdout.write(task)
            print("─" * 72)
            print(f"[relay] 위 본문으로 위임하려면 {budget_flags} 을 선언해 --apply 하라.")
            return 0
        bud = declared_budget(a, a.timeout_seconds, a.budget_source or "", doc)
        return run_attempt(a, doc, lp, task, bud, require_resume(a.resume, lks))

    if not a.topology:
        raise SystemExit("[relay] --topology 필수")
    task = a.task
    if a.campaign_control:
        if not (a.campaign_id and a.node):
            raise SystemExit("[relay] STOP: --campaign-control 은 --campaign-id 와 --node 가 필요하다(이름 추론 ✗)")
        if task or a.task_file:
            raise SystemExit("[relay] STOP: --campaign-control 의 본문은 기계가 조립한다 — --task 와 함께 쓰지 않는다")
        _ci = _campaign_init()
        _mbase = _ci.Path(a.repo_root) / "campaigns" / a.campaign_id
        _reason = None
        if a.orphan_sweep:
            # 메인에 인스턴스가 없는 서브 orphan 정리(K4). 메인 전이가 없으므로 메인 상태 대조를 건너뛰는 대신 **부재를
            # 선언**으로 받는다 — 메인 인스턴스가 있으면 정식 경로(메인 --abandon 뒤 전파)를 타야 한다(이름 추론 ✗).
            if a.campaign_control != "abandon":
                raise SystemExit("[relay] STOP: --orphan-sweep 은 --campaign-control abandon 하고만 쓴다")
            a._sweep_ids = [c.strip() for c in a.campaign_id.split(",") if c.strip()]
            _present = [c for c in a._sweep_ids if (_ci.Path(a.repo_root) / "campaigns" / c).exists()]
            if _present:
                raise SystemExit(f"[relay] STOP: 메인에 campaigns/{_present} 이 있다 — orphan sweep 이 아니라 메인 "
                                 "`--abandon` 뒤 전파로 간다(정식 경로)")
            if not a.reason:
                raise SystemExit("[relay] STOP: --orphan-sweep 은 --reason 이 필요하다(사람이 고른 포기 사유 전사)")
            _reason = a.reason
            _life = {"interrupted_cells": []}
        else:
            _life = _ci.fold_lifecycle(_mbase, active=_ci.active_campaign_id(a.repo_root))
            if a.campaign_control == "reslice" and a.node not in _life["reslice_pending"]:
                raise SystemExit(f"[relay] STOP: {a.node} 에 미수렴 재배정이 없다({_life['reslice_pending']}) — 메인 "
                                 "`campaign_init --assign-set` 이 먼저다")
            if a.campaign_control != "reslice" and a.node not in _life["sub_nodes"]:
                raise SystemExit(f"[relay] STOP: {a.node} 는 {a.campaign_id} 의 마지막 전이가 지목한 서브가 아니다"
                                 f"({_life['sub_nodes']}) — 메인 전이를 먼저 하라")
            if a.campaign_control == "reslice":
                if _life["state"] != "active":
                    raise SystemExit(f"[relay] STOP: 메인 {a.campaign_id} 은 {_life['state']} 다 — 재배정은 active 캠페인에만 "
                                     "전한다(보류 중이면 서브 writer 도 거부한다)")
            elif a.campaign_control in CONTROL_CLOSE_REASON:
                if _life["state"] != "closed" or _life["close_reason"] != CONTROL_CLOSE_REASON[a.campaign_control]:
                    raise SystemExit(f"[relay] STOP: 메인 {a.campaign_id} 은 {_life['state']}"
                                     f"({_life.get('close_reason')}) 다 — {a.campaign_control} 를 서브에 전하려면 메인이 "
                                     f"먼저 closed({CONTROL_CLOSE_REASON[a.campaign_control]}) 여야 한다")
                _closed = next((e for e in reversed(_ci.read_journey(_mbase))
                                if isinstance(e, dict) and e.get("kind") == "campaign_closed"), {})
                _reason = a.reason or _closed.get("note")
            else:
                _want = "suspended" if a.campaign_control == "suspend" else "active"
                if _life["state"] not in (_want, "resume_pending"):
                    raise SystemExit(f"[relay] STOP: 메인 {a.campaign_id} 은 {_life['state']} 다 — {a.campaign_control} 를 "
                                     f"서브에 전할 상태가 아니다(메인 전이가 먼저)")
        _slice = (_ci.emit_slice(_mbase, a.node, utc="delegated-task-body")
                  if a.campaign_control == "reslice" else None)
        a._control_slice = _slice
        task = (orphan_sweep_task(a._sweep_ids, _reason) if a.orphan_sweep else
                campaign_control_task(a.campaign_id, a.campaign_control,
                                      preempt=bool(_life.get("interrupted_cells")), reason=_reason, slice_doc=_slice))
        # orphan sweep 은 메인 인스턴스가 없다 — 원장은 대기실(_bootstrap)로 가고 휘발이므로, 호출자가 echo 원문을
        # docs 평면(simlog)에 옮긴다(K4 · D2 휘발 대비).
        _RELAY_CAMPAIGN = None if a.orphan_sweep else a.campaign_id
        lp = ledger_path(a.repo_root, a.context_id)      # 원장을 대상 캠페인 아래로 다시 잡는다
        doc = load_ledger(lp)
        bind_campaign_context(doc, node=a.node, kind=a.context_kind, cell=a.cell)
        doc["campaign_id"] = a.campaign_id
        if a.control_variables:
            doc["control_variables"] = a.control_variables
        if a.runners:
            doc["runner_ladder"] = ",".join(r["name"] for r in a.ladder)
    if a.task_file:
        with open(a.task_file, encoding="utf-8") as f:
            task = f.read()
    if not task:
        raise SystemExit("[relay] --task 또는 --task-file 필수")
    if a.handoff_issue:
        task = "[실제 인계 문제의 정정·설명 요청] " + a.handoff_issue + "\n" + task
    a.manifest_path = a.manifest or os.path.join(REPO, "output", a.topology, "manifest.yaml")
    _timeout, _src = a.timeout_seconds, a.budget_source or ""
    if a.budget_from_phase and _timeout is None:
        _timeout, _why = derive_timeout(a.repo_root, a.campaign_id, a.node)
        _timeout = _timeout or turn_budget.schema_cap("timeout_seconds")
        _src = (f"{_src} · " if _src else "") + f"시간 예산 파생: {_why}"
    bud = declared_budget(a, _timeout, _src, doc)
    return run_attempt(a, doc, lp, task, bud, require_resume(a.resume, last_known_session(doc)))


def main() -> int:
    if "--help" in sys.argv or "-h" in sys.argv or "--self-test" in sys.argv:
        return _main()
    # One local producer owns dispatch for this repository, including across campaigns/harnesses.
    repo = REPO
    for i, arg in enumerate(sys.argv[1:], 1):
        if arg == "--repo-root" and i + 1 < len(sys.argv):
            repo = sys.argv[i + 1]
        elif arg.startswith("--repo-root="):
            repo = arg.split("=", 1)[1]
    with state_lock(os.path.join(os.path.abspath(repo), "output", ".relay-dispatch"), blocking=False):
        return _main()


if __name__ == "__main__":
    sys.exit(main())
