#!/usr/bin/env python3
"""turn_budget.py — 서브 위임 **턴 예산의 단일 소유자**(grade 표).

왜 표인가(2026-09-03 · P2 · plan_26090317 §5):
    이전 규약은 `max-turns = 3`(reconciliation_cap 미러)이라는 **한 개의 매직상수**였다. 그 값은
    카나리 같은 단발 태스크엔 넉넉하고 캠페인 태스크엔 턱없이 모자란데, 소진하면 `failed` 로
    끝나 사람에게 넘어갔다 — 정상 진행이 실패로 집계됐다.

    참고 프로젝트(`seed/hermes-subagent-control-plane/references/turn-budget-grades.md`)의 규율을
    차용한다. 핵심은 값이 아니라 **두 문장**이다:

      ① **scope ⊥ budget** — max-turns 는 비용 노브이지 hang 노브가 아니다. 작업이 안 끝나는
         것과 매달려 있는 것은 다른 문제이며, hang 은 timeout + 진행로그 + post-verify 로 따로 잡는다.
      ② **소진 시 예산을 줄이지 않는다** — 줄이면 하강나선이다. 소진은 terminal 이고, 다음은
         **더 큰 예산의 새 attempt**(직전 산출물을 prompt 로 실어 주는 G5 방식)다.

    아래 값은 **cold-start prior** 다 — 실측(원장 `max_turns_used` 의 p90)으로 교정한다.
    교정 전까지는 이 표가 정본이고, 호출부는 숫자를 손으로 적지 않는다.
"""
from __future__ import annotations

import argparse
import json
import os
import sys

# 요청 스키마가 max_turns 의 **상한 정본**이다 — 여기 숫자를 다시 적지 않는다(두 자리가 갈라진다).
REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))
REQUEST_SCHEMA = os.path.join(REPO, ".claude", "schemas", "agent-control-request.schema.json")


def schema_max_turns(path: str = REQUEST_SCHEMA) -> int:
    """agent-control 요청 스키마의 max_turns 상한을 읽는다. 못 읽으면 fail-loud.

    2026-09-03 실측(P4): L4(65) 소진 → escalate 가 104 를 냈고 → 요청 스키마가
    `$.max_turns: 104 above maximum 100` 으로 **정상 차단**했다. 가드는 제 일을 했지만,
    증액하는 쪽이 상한을 모르니 **L4 소진 뒤 릴레이가 구조적으로 막혔다**(attempt 하나가
    통째로 낭비됐다). 상한을 여기 상수로 복제하면 스키마와 갈라지므로 **읽는다**.
    """
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    node = (doc.get("properties") or {}).get("max_turns") or {}
    cap = node.get("maximum")
    if not isinstance(cap, int) or cap < 1:
        raise SystemExit(f"[turn-budget] FAIL: 요청 스키마에서 max_turns 상한을 읽지 못했다 — {path}")
    return cap

# grade → (max_turns, timeout_seconds, 제어 패턴). 하한은 6 — 그 아래는 왕복 자체가 성립하지 않는다.
GRADES = {
    "S":  {"max_turns": 10, "timeout_seconds": 600,  "pattern": "단발",
           "example": "inspect 카나리(정체성·grade 회신)"},
    "L0": {"max_turns": 8,  "timeout_seconds": 600,  "pattern": "bounded + post-verify",
           "example": "완성 런타임블럭 기동·단일 명령"},
    "L1": {"max_turns": 16, "timeout_seconds": 1200, "pattern": "bounded + post-verify",
           "example": "단일 파일 patch + 명령 1"},
    "L2": {"max_turns": 25, "timeout_seconds": 1800, "pattern": "bounded + 산출물 검증",
           "example": "트리플렛 저작·compose up 1단계"},
    "L3": {"max_turns": 40, "timeout_seconds": 3600, "pattern": "background + timeout/log watchdog",
           "example": "빌드·진단 + 원인분리"},
    "L4": {"max_turns": 65, "timeout_seconds": 3600, "pattern": "턴제 릴레이 + watchdog",
           "example": "캠페인 전체(빌드→서빙→벤치 + 도서관·HITL 루프)"},
}
GRADE_ORDER = ("S", "L0", "L1", "L2", "L3", "L4")
MIN_TURNS = 6
SOURCE = "grade-table:plan_26090317 §5 (cold-start prior · hermes turn-budget-grades 차용)"

# 소진 뒤 새 attempt 의 증액 계수. 값을 줄이는 방향은 **표현 불가**하다(하강나선 금지를 구조로 막는다).
ESCALATION_FACTOR = 1.6
MAX_ATTEMPTS_BEFORE_HITL = 3


def budget(grade: str) -> dict:
    if grade not in GRADES:
        raise SystemExit(f"[turn-budget] FAIL: 알 수 없는 grade={grade!r} — 어휘 {GRADE_ORDER}")
    row = dict(GRADES[grade])
    row["grade"] = grade
    row["source"] = SOURCE
    return row


def escalate(grade: str, previous_allocated: int, cap: int = None) -> dict:
    """소진 뒤 새 attempt 의 예산. **항상 이전보다 크다** — 같거나 작으면 fail-loud.

    단 전송 계약(요청 스키마)의 상한을 넘지 않는다. 상한에 닿으면 **그 사실을 출처에 적는다** —
    조용히 깎으면 "왜 안 늘었나" 를 다음 사람이 알 수 없다(침묵 폴백 금지).
    상한에서 또 소진되면 그것은 예산 문제가 아니라 **과업을 쪼개라는 신호**이며, 출처가 그렇게 말한다.
    """
    base = budget(grade)
    cap = schema_max_turns() if cap is None else cap
    # 바닥이 둘이다: **이력**(직전 배정의 ×1.6)과 **선언된 등급**(표의 값). 둘 중 큰 쪽을 쓴다.
    #   이력만 보면, 등급을 올려 재개할 때 표의 값보다 **낮게** 배정된다 — 2026-09-05 실측:
    #   L2(25) 소진 뒤 `--grade L4` 로 이어받았는데 라벨만 L4 이고 예산은 40 이었다(L4=65).
    #   그것은 선언 대비 하강이며, 이 모듈이 구조로 막겠다고 한 바로 그 방향이다.
    want = max(int(previous_allocated * ESCALATION_FACTOR),
               previous_allocated + MIN_TURNS,
               base["max_turns"])
    nxt = min(want, cap)
    if nxt <= previous_allocated:
        raise SystemExit(
            "[turn-budget] FAIL: 증액이 성립하지 않는다 — 이전 %d, 전송 상한 %d. 예산 축소는 하강나선이다. "
            "상한에서 소진됐다면 예산을 늘릴 것이 아니라 **과업을 더 작은 턴으로 쪼개라**(scope ⊥ budget)."
            % (previous_allocated, cap))
    base["max_turns"] = nxt
    base["source"] = SOURCE + f" · escalated from {previous_allocated} (×{ESCALATION_FACTOR})"
    if nxt < want:
        base["source"] += f" · 전송 상한 {cap} 으로 클램프(요청 {want})"
    return base


def _self_test() -> int:
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

    chk(all(GRADES[g]["max_turns"] >= MIN_TURNS for g in GRADE_ORDER),
        f"모든 grade 가 하한 {MIN_TURNS} 이상(왕복이 성립하는 최소치)")
    ascending = [GRADES[g]["max_turns"] for g in ("L0", "L1", "L2", "L3", "L4")]
    chk(ascending == sorted(ascending), f"L0→L4 예산 단조 증가 {ascending}")
    chk(budget("S")["source"].startswith("grade-table:"), "값 옆에 출처가 있다(매직상수 ✗)")
    e = escalate("L2", 25)
    chk(e["max_turns"] > 25, f"소진 뒤 증액 25 → {e['max_turns']}")
    e2 = escalate("S", 10)
    chk(e2["max_turns"] >= 10 + MIN_TURNS, f"작은 예산도 의미 있게 증액 10 → {e2['max_turns']}")
    # 음성대조: 축소를 표현할 수 있으면 규율이 구조가 아니라 관습이 된다.
    chk(all(escalate(g, GRADES[g]["max_turns"])["max_turns"] > GRADES[g]["max_turns"]
            for g in GRADE_ORDER), "어떤 grade 에서도 증액만 나온다(축소 표현 불가)")
    try:
        budget("XL")
        chk(False, "어휘 밖 grade → fail-loud")
    except SystemExit:
        chk(True, "어휘 밖 grade → fail-loud")
    # 전송 상한 인지(2026-09-03 P4 실측: 104 를 스키마가 정상 차단해 attempt 하나가 낭비됐다)
    cap = schema_max_turns()
    chk(cap == 100, f"요청 스키마에서 상한을 읽는다(cap={cap}) — 상수 복제 ✗")
    e = escalate("L4", 65)
    chk(e["max_turns"] == cap and "클램프" in e["source"],
        f"상한 초과 요청은 클램프되고 그 사실이 출처에 남는다 → {e['max_turns']} · {e['source'][-40:]}")
    e2 = escalate("L2", 25)
    chk(e2["max_turns"] == 40 and "클램프" not in e2["source"],
        f"상한 미만은 종전대로 증액(클램프 표시 없음) → {e2['max_turns']}")
    # ★ 등급 상향 재개: 라벨만 오르고 예산이 표보다 낮게 배정되면 선언 대비 하강이다.
    e3 = escalate("L4", 25)
    chk(e3["max_turns"] >= GRADES["L4"]["max_turns"],
        f"등급을 올려 이어받으면 표의 값 이상을 배정한다 L2(25)→L4 = {e3['max_turns']} (표 {GRADES['L4']['max_turns']})")
    # 음성대조: 등급을 낮춰 이어받아도 **이력 바닥**이 지켜져 축소가 되지 않는다.
    e4 = escalate("L0", 40)
    chk(e4["max_turns"] > 40,
        f"등급을 낮춰 이어받아도 이력 바닥이 이긴다 40→L0 = {e4['max_turns']}")
    try:
        escalate("L4", cap)
        chk(False, "상한에서의 증액 시도는 fail-loud 여야 한다")
    except SystemExit as _e:
        chk("쪼개라" in str(_e), "상한 소진 → 예산이 아니라 과업 분할을 지시한다")

    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


def main() -> int:
    ap = argparse.ArgumentParser(description="서브 위임 턴 예산 grade 표")
    ap.add_argument("--grade", choices=list(GRADE_ORDER))
    ap.add_argument("--escalate-from", type=int, default=None,
                    help="소진한 이전 예산 — 증액된 새 attempt 예산을 낸다.")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if not a.grade:
        json.dump({g: GRADES[g] for g in GRADE_ORDER}, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    row = escalate(a.grade, a.escalate_from) if a.escalate_from else budget(a.grade)
    json.dump(row, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
