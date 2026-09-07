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
    · scope ⊥ budget · 소진은 terminal → **더 큰 예산의 새 attempt**(예산 축소 금지)
    · SILENT_FALLBACK 금지 — 메인이 대신 한 것을 서브 성공으로 집계하지 않는다
    · 원장은 예산 서사(선언값·출처·사용량·결과)를 매 턴 적는다

2026-09-05 개정(③ 3-1 · G-A2): 예산은 **등급표가 아니라 선언**이다. `--max-turns`·
`--timeout-seconds`·`--budget-source` 를 부르는 쪽이 정하고, 이 파일은 원장의 사실(집행된 바닥·
직전 소진 여부·전진 없는 연속 attempt 수)을 dry-run 에 **보여줄 뿐 대신 정하지 않는다**.
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
AGENT_CONTROL = os.path.join(REPO, ".claude", "policies", "runtime", "agent_control.py")
sys.path.insert(0, HERE)
import turn_budget  # noqa: E402
import bootstrap_canary as _canary  # noqa: E402  (manifest → target 해소를 재사용)

PENDING_HITL = "pending_hitl.json"

# ★ 2026-09-06(plan_26090616 ②): 원장 루트가 **루트 `tasks/` 에서 활성 캠페인 아래로** 옮겨간다.
#   왜: 루트 원장은 어느 캠페인의 왕복인지 이름으로만 구분됐고, 캠페인이 끝나도 남아 다음 캠페인
#   입력과 섞였다. 활성 캠페인이 없을 때는 예약 id `_bootstrap` 으로 간다 — 부재를 루트 폴백으로
#   처리하지 않는 것이 핵심이다(그 폴백이 산출물 누출의 직접 원인이었다).
#   경로 규칙의 단일 소유자는 `campaign_init.derive_path` 이며 여기서 복제하지 않는다.
import campaign_init as _campaign  # noqa: E402


def relay_root(repo_root: str) -> str:
    """이 저장소의 릴레이 원장 루트. 활성 캠페인 선언에서 파생한다(루트 기본값 없음)."""
    return str(_campaign.derive_path("relay-root", repo_root=repo_root))


def ledger_path(repo_root: str, context_id: str) -> str:
    return os.path.join(relay_root(repo_root), f"{context_id}.json")


def load_ledger(path: str) -> dict:
    if not os.path.exists(path):
        return {"schema_version": 1, "context_id": None, "attempts": []}
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def save_ledger(path: str, doc: dict) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")
    os.replace(tmp, path)


def _reached_sub(att: dict) -> bool:
    """이 attempt 가 서브에 **닿았는가**. 전송·스키마 실패는 예산 서사도 세션도 남기지 않는다."""
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
                "budget_outcome": att.get("budget_outcome")}
    return {"session_id": None, "attempt": None, "status": None,
            "control_status": None, "budget_outcome": None}


END_REASONS = ("completed", "sub_input_required", "sub_failed", "budget_exhausted",
               "external_interruption", "permission_denied", "malformed_output",
               "invalid_request", "model_blocked", "transport_or_launch_failure", "unclassified")


def end_reason(att: dict) -> str:
    """이 attempt 가 **왜 끝났는가** — 원장에 남는 단일 라벨(축 F).

    파생이지 선언이 아니다: 입력은 전부 이미 기록된 필드(control_status·reason_codes·
    budget_outcome·서브 status)다. 그런데도 값을 적어 두는 이유는, 나중에 "정지 시간을 무엇으로
    계상했나" 를 묻는 쪽이 판정 규칙이 아니라 **그때의 판정 결과**를 봐야 하기 때문이다.
    분류 불가는 `unclassified` 로 남긴다 — 모르는 것을 아는 척하지 않는다.
    """
    codes = att.get("reason_codes") or []
    control, sub = att.get("control_status"), att.get("status")
    if "PERMISSION_DENIED" in codes:
        return "permission_denied"
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


def relay_header(context_id: str, attempt: int, allocated: int, budget_source: str) -> str:
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
        f"- max_turns_allocated: {allocated}\n"
        f"- 예산 근거(메인 선언): {budget_source}\n"
        "- 규약: 리포트 `context_id` 에 위 값을 그대로 적는다. 예산이 모자라면 **소진하지 말고**\n"
        "  `input-required` 로 끊고 남은 일을 `next_steps` 에 적어라(scope ⊥ budget).\n"
        "- 외부지식을 검색했다면 `external_search[]` 에 질의·출처·요지를 남겨라 — 그 기록이\n"
        "  메인의 자산이 되고, 인용 없는 결정은 거짓이 아니라 **누락**이다(헌법 불변식 B).\n\n"
    )


def build_request(topology: str, manifest: str, task: str, bud: dict,
                  resume_session_id=None, capabilities=None,
                  context_id: str = None, attempt: int = 0, model: str = None,
                  campaign_id: str = None, control_variables: dict = None,
                  backend: str = None) -> dict:
    """위임 request 조립. `bud` 는 **선언된** 예산이다(`turn_budget.declare` 산출)."""
    base = _canary.build_request(topology, manifest, max_turns=bud["max_turns"],
                                 timeout_seconds=bud["timeout_seconds"],
                                 budget_source=bud["source"],
                                 model=model, backend=backend)  # target 해소·센티넬 거부를 재사용
    allocated = bud["max_turns"]
    base["intent"] = "delegate"
    camp_block = campaign_control_block(campaign_id, control_variables)
    base["task"] = ((relay_header(context_id, attempt, allocated, bud["source"]) + camp_block + task)
                    if context_id else (camp_block + task))
    if campaign_id:
        base["campaign_id"] = campaign_id
    if control_variables:
        base["control_variables"] = dict(control_variables)
    base["capabilities"] = list(capabilities or DEFAULT_CAPABILITIES)
    if resume_session_id:
        base["resume_session_id"] = resume_session_id
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


def _campaign_init():
    import importlib.util
    mod_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "campaign_init.py")
    spec = importlib.util.spec_from_file_location("_campaign_init_for_relay", mod_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


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


def record_attempt(doc: dict, *, context_id: str, bud: dict, result: dict, report=None,
                   resume_declared=None, request_path=None,
                   started_utc=None, ended_utc=None) -> dict:
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
    att = {
        "attempt": len(doc.get("attempts") or []) + 1,
        # 2026-09-05(G-A2): `task_grade` 대신 **선언된 예산과 그 근거**를 적는다. 등급 라벨은
        #   어휘가 사라졌고, 남길 가치가 있는 것은 "얼마를 왜 줬는가" 다.
        "max_turns_allocated": bud["max_turns"],
        "timeout_seconds_allocated": bud["timeout_seconds"],
        "budget_source": bud["source"],
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
        "control_status": result.get("status"),
        "reason_codes": result.get("reason_codes") or [],
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
    }
    att["end_reason"] = end_reason(att)
    doc["context_id"] = context_id
    doc.setdefault("attempts", []).append(att)
    return att


def entry_blocking(entry: dict) -> bool:
    """이 대기 항목이 **차단성**인가 — 저장된 필드가 아니라 요청 본문에서 파생한다.

    2026-09-04: 처음에는 표면화 시점에 `blocking` 을 계산해 항목에 적었다. 그런데 그러면 같은
    개념이 두 자리(요청 본문 · 항목 필드)에 앉고, 실제로 **옛 형식으로 쓰인 라이브 항목을 읽지
    못했다**(`library_request[0].blocking=true` 인데 항목에는 필드가 없어 비차단으로 읽혔다).
    `workflow.md` §4종 안티패턴의 "같은 개념이 두 곳 이상에 손으로 적힌 값" = 결함 칸이다.
    파생으로 바꾸면 옛 항목도 그대로 읽히고 갈라질 자리가 사라진다.
    """
    if any(bool(r.get("blocking")) for r in entry.get("library_request") or []):
        return True
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
    """서브가 스스로 표면화한 것을 릴레이한다(디스크 재스캔 ✗ — §2.4 push-attestation).

    2026-09-04 교정 3건:
      · **B2(침묵 누락)**: 종전에는 `hitl.needed` 가 거짓이면 즉시 return 했다. 그런데 서브 규약
        (`comms.md` 도서관 절)은 *"리포트에 `library_request[]` 를 싣고 `input-required` 로 끝내라"*
        만 지시하고 `hitl.needed` 를 요구하지 않는다 — **절차대로 한 요청이 사라졌다.** 이제
        요청 배열 하나만으로도 표면화한다.
      · **A4(해소 미반영)**: 같은 이유로 `needed=false` 인 턴은 `:126` 의 정리 필터에 **도달조차
        못 해**, 이미 해소된 질문이 계속 남았다(라이브 잔존 실측). 이제 어떤 경우에도 이 context
        의 낡은 항목을 먼저 걷어낸다.
      · **B3(우선순위)**: `blocking` 을 읽어 정렬 키로 쓴다.
    사람이 적어 넣은 `answer` 는 같은 `request_id` 로 **이월**한다 — 재표면화가 답을 지우면
    사람이 두 번 답해야 하고, 그것은 릴레이가 사람의 일을 늘리는 것이다.
    """
    report = report or {}
    hitl = report.get("hitl") or {}
    requests = report.get("library_request") or []
    path = os.path.join(relay_root(repo_root), PENDING_HITL)
    doc = {"schema_version": 1, "pending": []}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
    prior = {e.get("request_id"): e.get("answer") for e in doc.get("pending", [])
             if e.get("context_id") == context_id}
    doc["pending"] = [e for e in doc.get("pending", []) if e.get("context_id") != context_id]

    entry = None
    if crash and not (hitl.get("needed") or requests):
        # 2026-09-05(3-3): 리포트가 없거나 제어가 실패한 턴. 서브가 요청을 남기지 못한 상태이므로
        #   **메인이 사실만** 표면화한다 — 재개 여부는 사람·에이전트가 `--resume` 로 선언한다.
        entry = {
            "context_id": context_id,
            "attempt": attempt,
            "request_id": "relay-crash-a%02d" % attempt,
            "source": "relay-crash",
            "prompt": ("attempt %s 가 리포트 없이 끝났다(end_reason=%s · control=%s · %s). "
                       "마지막 알려진 세션 = %s. 이어받으려면 `--resume <session_id>`, "
                       "새로 열려면 `--resume new` 를 선언하라."
                       % (crash.get("attempt"), crash.get("end_reason"), crash.get("control_status"),
                          ",".join(crash.get("reason_codes") or []) or "코드 없음",
                          crash.get("last_known_session") or "(없음)")),
            "library_request": [],
            "answer": prior.get("relay-crash-a%02d" % attempt),
        }
        doc["pending"].append(entry)
    elif hitl.get("needed") or requests:
        rid = hitl.get("request_id") or (requests[0].get("request_id") if requests else None)
        entry = {
            "context_id": context_id,
            "attempt": attempt,
            "request_id": rid,
            "source": hitl.get("source") or "sub-relay",
            "prompt": hitl.get("prompt"),
            "library_request": requests,
            # 서브의 자기억제 선언은 요청 본문에 있다 — 여기에 **다시 적지 않는다**(entry_blocking).
            #   hitl 절이 스스로 차단성을 말한 경우만 항목에 남긴다(요청 본문에 없는 정보라서).
            **({"blocking": True} if hitl.get("blocking") else {}),
            "answer": prior.get(rid),
        }
        doc["pending"].append(entry)
    doc["pending"] = sort_pending(doc["pending"])
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")
    return path if entry else None


def pending_for(repo_root: str, context_id: str) -> list:
    """이 context 의 대기 요청(우선순위 순). `--continue` 가 읽는다 — **쓰기만 하던 파일에 소비자가 생긴다**."""
    path = os.path.join(relay_root(repo_root), PENDING_HITL)
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    return sort_pending([e for e in doc.get("pending", []) if e.get("context_id") == context_id])


def parse_report(output: str):
    """서브 리포트는 JSON 1개다. 산문에 섞여 와도 마지막 JSON 객체를 집는다 — 못 찾으면 None."""
    if not isinstance(output, str):
        return None
    depth, start = 0, None
    best = None
    for i, ch in enumerate(output):
        if ch == "{":
            if depth == 0:
                start = i
            depth += 1
        elif ch == "}" and depth:
            depth -= 1
            if depth == 0 and start is not None:
                try:
                    best = json.loads(output[start:i + 1])
                except ValueError:
                    pass
    return best if isinstance(best, dict) else None


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
      · 원래 지시(원장에 보존한 원문)
    메인이 추측한 진행상황을 여기에 적으면 그것이 SILENT_FALLBACK 이다.
    """
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
            for req in e.get("library_request") or []:
                out.append(f"- (질문) {req.get('question')}")
            if e.get("prompt"):
                out.append(f"- (질문) {e['prompt']}")
            out.append(f"- **(답)** {e['answer']}")
    open_nonblocking = [e for e in pending if not e.get("answer") and not entry_blocking(e)]
    if open_nonblocking:
        out += ["", "## 아직 답이 없는 비차단 질문(진행을 막지 않는다 — 없이 갈 수 있으면 가라)"]
        for e in open_nonblocking:
            out.append(f"- {e.get('prompt') or (e.get('library_request') or [{}])[0].get('question')}")
    out += ["", "## 원래 지시(변경 없음)", "", doc.get("task") or "(원장에 원 지시가 없다)"]
    return "\n".join(out) + "\n"


def _self_test() -> int:
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

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
                                                   "prompt": "어느 핀?"},
                                          "library_request": [{"question": "q", "kind": "version-pin"}]},
                             attempt=1)
        chk(p and json.load(open(p))["pending"][0]["request_id"] == "h1", "HITL 표면화")
        p2 = surface_requests(d, "ctx-2", {"hitl": {"needed": False}}, attempt=1)
        chk(p2 is None, "요청이 없으면 새 항목을 만들지 않는다(메인이 대신 만들지 않는다)")

        # ⑥ 리포트 파싱 — 산문에 섞여 와도 JSON 을 집는다 / 없으면 None
        chk(parse_report("어쩌고 {\"status\": \"completed\"} 끝")["status"] == "completed",
            "산문 속 JSON 리포트 추출")
        chk(parse_report("리포트 없음") is None, "JSON 이 없으면 None(추측 파싱 ✗)")

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
        rep_lib = {"library_request": [{"question": "q1", "kind": "serving-recipe",
                                        "blocking": True, "request_id": "r1"}]}
        pth = surface_requests(d, "ctx-lib", rep_lib, attempt=1)
        chk(pth is not None, "★hitl.needed 없이 library_request 만으로도 표면화된다(침묵 누락 ✗)")
        chk(entry_blocking(pending_for(d, "ctx-lib")[0]) is True,
            "blocking 을 메인이 **읽는다**(요청 본문에서 파생 — 옛 형식 항목도 읽힌다)")
        surface_requests(d, "ctx-nb", {"library_request": [{"question": "q2", "blocking": False,
                                                            "request_id": "r2"}]}, attempt=1)
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
        d8 = {"task": "원 지시 본문", "attempts": [
            {"attempt": 1, "status": "input-required", "phase": "build", "session_id": "s1",
             "budget_outcome": "exhausted", "max_turns_allocated": 25,
             "report_path": "campaigns/_bootstrap/relay/ctx-c.reports/attempt-01.json"}]}
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

    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


SSH_PROBE = ("ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8",
             "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=2")


def _ssh_run(target: dict, cmd: str):
    argv = [*SSH_PROBE, f"{target['ssh_user']}@{target['host']}", cmd]
    out = subprocess.run(argv, capture_output=True, text=True, stdin=subprocess.DEVNULL)
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
    rc, out, err = (runner or _ssh_run)(t, f"git -C '{t['work_dir']}' rev-parse --abbrev-ref HEAD")
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


def run_attempt(a, doc: dict, lp: str, task: str, bud: dict, resume_declared) -> int:
    """한 번의 위임 왕복. `--task` 진입과 `--continue` 진입이 **같은 몸통**을 쓴다."""
    attempt_no = len(doc.get("attempts") or []) + 1
    resume = None if resume_declared in (None, "new") else resume_declared
    req = build_request(a.topology, a.manifest_path, task, bud, resume_session_id=resume,
                        context_id=a.context_id, attempt=attempt_no, model=a.model, backend=a.backend,
                        campaign_id=getattr(a, "campaign_id", None),
                        control_variables=getattr(a, "control_variables", None))

    if a.emit_only:
        json.dump(req, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0

    branch = assert_sub_branch(req, a.topology)
    print(f"[relay] attempt={attempt_no} max_turns={bud['max_turns']} "
          f"timeout={bud['timeout_seconds']}s ({bud['source']}) "
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
    out = subprocess.run([sys.executable, AGENT_CONTROL, "invoke", "--request", _qp],
                         capture_output=True, text=True)
    ended = _utcnow()
    sys.stderr.write(out.stderr)
    control_stderr = (out.stderr or "").strip()
    try:
        result = json.loads(out.stdout)
    except ValueError:
        raise SystemExit(f"[relay] FAIL: agent_control 출력을 읽지 못했다 — {out.stdout[:300]}")

    report = parse_report(result.get("output") or "")
    att = record_attempt(doc, context_id=a.context_id, bud=bud, result=result, report=report,
                         resume_declared=resume_declared,
                         request_path=os.path.relpath(_qp, a.repo_root),
                         started_utc=started, ended_utc=ended)
    # 2026-09-04(P4 라이브): 원장이 status 만 적고 **리포트 본문을 버렸다** — 나중에 "서브가 무엇을
    #   근거로 completed 라 했는가" 를 메인이 감사할 수 없었다(내가 서브를 의심했다가 dotfile 을
    #   놓친 내 실수임을 확인하는 데도 서브 워크스페이스를 다시 뒤져야 했다 — 재스캔은 계약 밖이다).
    #   서브가 보낸 것은 서브가 보낸 그대로 남긴다. 없으면 산문 원문을 남긴다(추측 파싱 ✗).
    _adir = os.path.join(relay_root(a.repo_root), f"{a.context_id}.reports")
    os.makedirs(_adir, exist_ok=True)
    _ap = os.path.join(_adir, "attempt-%02d.json" % att["attempt"])
    with open(_ap, "w", encoding="utf-8") as f:
        json.dump({"attempt": att["attempt"], "report": report,
                   "raw_output": None if report else (result.get("output") or ""),
                   # 2026-09-05(N6): provider 진단(`_diag` 의 stderr)을 **원장 옆에 보존**한다.
                   #   라이브에서 malformed 로 끝났을 때 "무엇이 왔길래" 를 알 수 있는 유일한 통로가
                   #   이 텍스트였는데, 화면을 스크롤해 지나가면 그대로 사라졌다.
                   "control_stderr_tail": control_stderr[-4000:] or None,
                   "control": {k: result.get(k) for k in
                               ("status", "exit_code", "reason_codes", "session_id",
                                "num_turns", "budget_outcome", "duration_ms",
                                "duration_api_ms")}},
                  f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")
    att["report_path"] = os.path.relpath(_ap, a.repo_root)
    doc["task"] = doc.get("task") or task
    doc["topology"] = a.topology
    doc["manifest"] = a.manifest_path
    save_ledger(lp, doc)
    _crash = None
    if report is None or att["control_status"] != "completed":
        # 3-3: 파싱 실패·프로세스 크래시는 **fail-loud + 마지막 알려진 세션 제시**다. 재개 여부는
        #   사람·에이전트가 정하므로, 그 판단에 필요한 세션 id 를 대기 목록에 남긴다.
        _crash = {"attempt": att["attempt"], "end_reason": att["end_reason"],
                  "control_status": att["control_status"],
                  "reason_codes": att["reason_codes"],
                  "last_known_session": att.get("session_id") or last_known_session(doc)["session_id"]}
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
    if att["resume_honored"] is False:
        print(f"[relay] ⚠ 재개 불발: 요청한 세션 {resume} 과 다른 세션 {att['session_id']} 이 열렸다 — "
              "서브가 컨텍스트를 처음부터 재구축했을 수 있다(소진의 주된 원인). 다음 턴의 예산을 그렇게 읽어라.")
    _echo_stop = echo_stop_reasons(att, context_id=a.context_id,
                                   campaign_id=(a.campaign_id or doc.get("campaign_id")),
                                   control_variables=doc.get("control_variables"))
    for _r in _echo_stop:
        print(f"[relay] ⚠ 정체성 echo: {_r} — `--continue` 는 이 상태에서 진행하지 않는다.")
    if att["external_search"]:
        print(f"[relay] 서브가 외부검색 {len(att['external_search'])}건을 기록했다 — 자산화 후보. "
              f"근거는 원장 attempt {att['attempt']}.external_search 에 있다.")
    if hp:
        print(f"[relay] ⚠ 서브가 요청을 표면화했다 → {hp} "
              "(`answer` 를 적고 `--continue` 로 재개. blocking 은 답 없이는 진행하지 않는다)")
    if att["budget_outcome"] == "exhausted":
        print(f"[relay] ⚠ 예산 소진(terminal). 자동 재시도하지 않는다 — "
              f"`relay.py --continue --context-id {a.context_id}` 로 이어라(본문은 기계가 조립한다).\n"
              f"[relay]   다음 예산은 **선언**이다: 집행된 바닥 {budget_floor(doc)} 아래로 내리면 하강나선이다.")
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


def main() -> int:
    ap = argparse.ArgumentParser(description="메인↔서브 턴제 릴레이")
    ap.add_argument("--topology", choices=["single", "multi"])
    ap.add_argument("--manifest", default=None)
    ap.add_argument("--context-id")
    ap.add_argument("--max-turns", type=int, default=None,
                    help="이 attempt 에 배정할 턴 예산(선언 필수 — 등급표 폐기 · 기본값 없음)")
    ap.add_argument("--timeout-seconds", type=int, default=None,
                    help="매달림 상한(scope ⊥ budget — 예산과 별개 노브)")
    ap.add_argument("--budget-source", default=None,
                    help="그 예산을 그렇게 정한 근거(필수 · 원장에 남는다)")
    ap.add_argument("--backend", choices=["anthropic", "kimi"], default=None,
                    help="provider 실행 백엔드(기본 anthropic=claude · kimi=kimi-claude shim, 내부 LLM Kimi K3)")
    ap.add_argument("--model", default=None,
                    help="위임 모델 선언(기본은 카나리 기본값). 어댑터는 모델로 차단하지 않는다 — "
                         "실제로 돈 모델은 원장 `model_used` 가 말한다(2026-09-05 · G-A1).")
    ap.add_argument("--resume", default=None, metavar="SESSION_ID|new",
                    help="이어받을 provider 세션 id, 또는 새 세션이면 `new`. **선언 필수** — "
                         "코드가 대신 정하지 않는다(2026-09-05 · 축 F).")
    ap.add_argument("--task", help="서브에 보낼 지시(또는 --task-file)")
    ap.add_argument("--task-file")
    ap.add_argument("--continue", dest="continue_", action="store_true",
                    help="원장의 중단점에서 이어간다 — 본문을 기계가 조립한다. 기본 dry-run.")
    ap.add_argument("--apply", action="store_true",
                    help="--continue 와 함께: 조립한 본문으로 실제 위임한다(사람 승인 정문).")
    ap.add_argument("--emit-only", action="store_true", help="request 만 조립해 출력(실행 ✗)")
    ap.add_argument("--repo-root", default=REPO)
    # 2026-09-07(plan_26090715 §4.8): 캠페인 정체성은 **요청에 실려 간다**. 생략하면 활성
    #   캠페인에서 파생하고, `_bootstrap`(캠페인 밖)이면 싣지 않는다.
    ap.add_argument("--campaign-id", default=None,
                    help="위임에 실을 캠페인 id(생략 시 활성 캠페인 · _bootstrap 이면 미탑재)")
    ap.add_argument("--no-campaign", action="store_true",
                    help="캠페인 축을 싣지 않는다(캠페인 밖 온보딩·카나리)")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if not a.context_id:
        raise SystemExit("[relay] --context-id 필수")

    lp = ledger_path(a.repo_root, a.context_id)
    doc = load_ledger(lp)
    a.campaign_id, a.control_variables = resolve_campaign_axis(a)
    if a.campaign_id:
        doc["campaign_id"] = a.campaign_id
    if a.control_variables:
        doc["control_variables"] = a.control_variables

    if a.continue_:
        # 이어가기에 필요한 것은 전부 **원장**에서 온다 — 사람이 다시 적지 않는다.
        if not (doc.get("attempts") or []):
            raise SystemExit(f"[relay] STOP: 이어갈 원장이 없다 — {lp}. 첫 위임은 `--task` 로 연다.")
        a.topology = a.topology or doc.get("topology")
        if not a.topology:
            raise SystemExit("[relay] STOP: 원장에 topology 가 없다(구버전 원장) — 명시하라.")
        a.manifest_path = a.manifest or doc.get("manifest") or os.path.join(
            REPO, "output", a.topology, "manifest.yaml")

        # ── 정지 조건. 예산·전진 수는 **보여주고**, 멈출지는 --apply(승인 정문)가 정한다.
        stalled = stalled_attempts(doc)
        last = next((x for x in reversed(doc["attempts"]) if _reached_sub(x)), None)
        if last and last.get("status") == "completed":
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
                                      control_variables=doc.get("control_variables"))
            if _stop:
                raise SystemExit(
                    "[relay] STOP: 서브 정체성 echo 가 성립하지 않는다 — 부재도 불일치도 STOP 이다.\n"
                    + "".join(f"  - {r}\n" for r in _stop)
                    + "  → 서브가 위임 헤더의 값을 그대로 회신하도록 한 뒤 재개하라(우회 ✗).")
        pend = pending_for(a.repo_root, a.context_id)
        blocked = [e for e in pend if entry_blocking(e) and not e.get("answer")]
        if blocked:
            ids = ", ".join(str(e.get("request_id")) for e in blocked)
            raise SystemExit(
                f"[relay] STOP: 답이 필요한 **차단성** 요청이 있다 — {ids}\n"
                f"  → {relay_root(repo_root)}/{PENDING_HITL} 의 해당 항목 `answer` 에 답을 적고 다시 --continue 하라.\n"
                "  → 이것이 사람의 승인 정문이다(답 = 승인). 답 없이 진행하면 서브가 근거 없이 결정한다.")

        lks = last_known_session(doc)
        task = assemble_continuation(a.repo_root, doc, pend)
        floor = budget_floor(doc)
        _last_out = (last or {}).get("budget_outcome")
        if not a.apply:
            # dry-run 은 **사실만** 보여준다 — 예산도 재개도 이 화면을 본 쪽이 선언한다.
            print(f"[relay] --continue DRY-RUN · context={a.context_id}")
            print(f"[relay] 마지막 알려진 세션: {lks['session_id'] or '(없음)'} "
                  f"(attempt {lks['attempt']} · sub_status={lks['status']} · "
                  f"budget={lks['budget_outcome']}) → `--resume <id|new>` 로 선언하라")
            print(f"[relay] 예산 사실: 집행된 바닥 {floor} · 직전 결과 {_last_out} · "
                  f"전송 상한 {turn_budget.schema_cap('max_turns')} · 대기 요청 {len(pend)}건 · "
                  f"전진 없는 연속 attempt {stalled}회")
            if _last_out == "exhausted":
                print("[relay]   직전은 **소진**이다 — 같은 값으로 다시 열면 그 자리서 또 소진된다"
                      "(감액은 하강나선 · scope ⊥ budget).")
            print("─" * 72)
            sys.stdout.write(task)
            print("─" * 72)
            print("[relay] 위 본문으로 위임하려면 --max-turns/--timeout-seconds/--budget-source 와 "
                  "--resume 을 선언해 --apply 하라(사람 승인 정문).")
            return 0
        bud = turn_budget.declare(a.max_turns, a.timeout_seconds, source=a.budget_source or "")
        return run_attempt(a, doc, lp, task, bud, require_resume(a.resume, lks))

    if not a.topology:
        raise SystemExit("[relay] --topology 필수")
    task = a.task
    if a.task_file:
        with open(a.task_file, encoding="utf-8") as f:
            task = f.read()
    if not task:
        raise SystemExit("[relay] --task 또는 --task-file 필수")
    a.manifest_path = a.manifest or os.path.join(REPO, "output", a.topology, "manifest.yaml")
    bud = turn_budget.declare(a.max_turns, a.timeout_seconds, source=a.budget_source or "")
    return run_attempt(a, doc, lp, task, bud, require_resume(a.resume, last_known_session(doc)))


if __name__ == "__main__":
    sys.exit(main())
