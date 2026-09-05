#!/usr/bin/env python3
"""turn_budget.py — 턴 예산의 **선언 검증기**(값의 소유자가 아니다).

2026-09-05 개정(`plan_26090516` ③ 3-1 · `audit_26090515` G-A2). 이 파일은 grade 표
(S·L0~L4 의 max_turns/timeout 6쌍)를 갖고 있었고 그 표가 **모델 과적합의 정면 사례**로 지목됐다.
표는 스스로 *"cold-start prior 이며 원장 `max_turns_used` 의 p90 으로 교정한다"* 고 적었지만
**교정하는 코드도 부르는 사람도 없었다(소비자 0)**. 그 사이 값은 실측 없이 정본 행세를 했고,
호출부는 과업의 크기 대신 표의 라벨을 골랐다 — 판단이 값으로 고정된 자리다(3원칙 ③ 위반).

지금 규약: **예산은 부르는 쪽이 선언한다.** 기본값을 여기서 만들지 않는다.
전송 계약(요청 스키마)이 상한을 갖고, 원장이 이력을 갖고, 판단은 에이전트·사람이 한다.

보존한 규율 두 문장 — 이제 값이 아니라 **문장**으로만 남는다(강제하는 코드는 없다):
  ① **scope ⊥ budget** — `max_turns` 는 비용 노브이지 hang 노브가 아니다. 매달림은
     `timeout_seconds` + 진행로그 + post-verify 로 따로 잡는다. 그래서 둘을 **함께** 선언한다.
  ② **소진은 terminal 이고 다음은 더 큰 예산의 새 attempt 다** — 줄이면 하강나선이다.
     원장이 집행된 예산과 소진 사실을 그대로 보여주므로(`relay.py --continue` dry-run),
     선언하는 쪽이 그것을 보고 정한다. 규율을 지키는 주체가 표에서 사람·에이전트로 옮겨졌다.
"""
from __future__ import annotations

import argparse
import json
import os
import sys

# 요청 스키마가 상한의 **단일 권위**다 — 여기 숫자를 다시 적지 않는다(두 자리가 갈라진다).
REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))
REQUEST_SCHEMA = os.path.join(REPO, ".claude", "schemas", "agent-control-request.schema.json")


def schema_cap(field: str, path: str = REQUEST_SCHEMA) -> int:
    """agent-control 요청 스키마에서 그 필드의 상한을 읽는다. 못 읽으면 fail-loud.

    2026-09-03 실측(P4): 옛 등급표의 증액 계수가 104 를 냈고 요청 스키마가
    `$.max_turns: 104 above maximum 100` 으로 **정상 차단**했다. 가드는 제 일을 했지만
    증액하는 쪽이 상한을 몰라 **attempt 하나가 통째로 낭비됐다**. 상한을 상수로 복제하면
    스키마와 갈라지므로 **읽는다**.
    """
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    node = (doc.get("properties") or {}).get(field) or {}
    cap = node.get("maximum")
    if not isinstance(cap, int) or cap < 1:
        raise SystemExit(f"[turn-budget] FAIL: 요청 스키마에서 {field} 상한을 읽지 못했다 — {path}")
    return cap


def schema_max_turns(path: str = REQUEST_SCHEMA) -> int:
    return schema_cap("max_turns", path)


def declare(max_turns, timeout_seconds, *, source: str, caps: dict = None) -> dict:
    """**선언된** 예산을 검증해 그대로 돌려준다. 값을 만들지 않는다 — 없으면 fail-loud.

    `source` 는 선택이 아니다: 결정론 규율(§출처 표시)은 값 옆에 그 값이 어디서 왔는지를
    요구한다. 옛 표는 이 자리에 `grade-table:...` 을 적어 출처가 있는 것처럼 보였지만
    그 출처가 가리키는 곳에는 측정이 없었다. 이제 선언한 쪽이 자기 근거를 적는다.
    """
    caps = caps or {"max_turns": schema_cap("max_turns"),
                    "timeout_seconds": schema_cap("timeout_seconds")}
    vals = {"max_turns": max_turns, "timeout_seconds": timeout_seconds}
    for k, v in vals.items():
        if v is None:
            raise SystemExit(
                f"[turn-budget] FAIL: {k} 가 선언되지 않았다 — 기본값을 만들지 않는다(등급표 폐기).\n"
                "  → 예산은 과업의 크기를 아는 쪽이 정한다. `--max-turns`/`--timeout-seconds` 로 선언하라.\n"
                "  → 규율: scope ⊥ budget(매달림은 timeout 이 잡는다) · 소진 뒤 감액 ✗.")
        if not isinstance(v, int) or isinstance(v, bool) or v < 1:
            raise SystemExit(f"[turn-budget] FAIL: {k}={v!r} 은 1 이상의 정수가 아니다.")
        if v > caps[k]:
            raise SystemExit(
                f"[turn-budget] FAIL: {k}={v} 가 전송 상한 {caps[k]} 을 넘는다(요청 스키마).\n"
                "  → 상한에서 모자란다면 예산 문제가 아니라 **과업을 더 작게 쪼개라는 신호**다"
                "(scope ⊥ budget).")
    if not (isinstance(source, str) and source.strip()):
        raise SystemExit("[turn-budget] FAIL: 예산 출처(source)가 비어 있다 — 값 옆에 출처를 적는다.")
    return {"max_turns": max_turns, "timeout_seconds": timeout_seconds, "source": source.strip()}


def _self_test() -> int:
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

    # ★ tripwire: 등급표가 되살아나면 여기서 잡는다(2026-09-05 삭제 · G-A2).
    revived = [n for n in ("GRADES", "GRADE_ORDER", "MIN_TURNS", "ESCALATION_FACTOR",  # antipattern-ok: G-A2-grade-table — tripwire 목록 자신(어휘를 적어야 검사가 성립한다)
                           "MAX_ATTEMPTS_BEFORE_HITL", "budget", "escalate")  # antipattern-ok: G-A2-grade-table — 동상
               if n in globals()]
    chk(revived == [], f"등급표·증액계수·정지한계가 되살아나지 않았다(발견 {revived})")

    caps = {"max_turns": schema_cap("max_turns"), "timeout_seconds": schema_cap("timeout_seconds")}
    chk(caps == {"max_turns": 100, "timeout_seconds": 3600},
        f"상한을 요청 스키마에서 읽는다(상수 복제 ✗) → {caps}")

    d = declare(24, 1800, source="선언: 빌드 1회 + 스모크 — 원장 attempt 3 의 실측 turns=19")
    chk(d["max_turns"] == 24 and d["timeout_seconds"] == 1800,
        "선언한 값이 그대로 나온다(재계산·보정 ✗)")
    chk("원장" in d["source"], "값 옆에 출처가 남는다")

    for bad, label in ((None, "미선언"), (0, "0"), (-1, "음수"), (True, "bool"), ("10", "문자열")):
        try:
            declare(bad, 600, source="s")
            chk(False, f"{label} max_turns → fail-loud")
        except SystemExit:
            chk(True, f"{label} max_turns → fail-loud")
    try:
        declare(10, None, source="s")
        chk(False, "timeout 미선언 → fail-loud")
    except SystemExit as e:
        chk("scope ⊥ budget" in str(e), "timeout 미선언 → fail-loud(두 규율을 함께 말한다)")
    try:
        declare(caps["max_turns"] + 1, 600, source="s")
        chk(False, "상한 초과 → fail-loud")
    except SystemExit as e:
        chk("쪼개라" in str(e), "상한 초과는 증액이 아니라 **과업 분할**을 지시한다")
    try:
        declare(10, 600, source="   ")
        chk(False, "출처 없는 선언 → fail-loud")
    except SystemExit:
        chk(True, "출처 없는 선언 → fail-loud(출처 표시 규율)")

    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


def main() -> int:
    ap = argparse.ArgumentParser(description="턴 예산 선언 검증기(값의 소유자 ✗ — 부르는 쪽이 선언한다)")
    ap.add_argument("--max-turns", type=int, default=None)
    ap.add_argument("--timeout-seconds", type=int, default=None)
    ap.add_argument("--source", default=None, help="이 예산을 그렇게 정한 근거(필수)")
    ap.add_argument("--caps", action="store_true", help="요청 스키마가 가진 상한을 출력한다")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if a.caps:
        json.dump({"max_turns": schema_cap("max_turns"),
                   "timeout_seconds": schema_cap("timeout_seconds"),
                   "source": f"request-schema:{os.path.relpath(REQUEST_SCHEMA, REPO)}"},
                  sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    json.dump(declare(a.max_turns, a.timeout_seconds, source=a.source or ""),
              sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
