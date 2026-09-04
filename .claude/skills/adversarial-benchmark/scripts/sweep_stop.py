#!/usr/bin/env python3
# sweep_stop.py — 광의의 탐색(Broad Search) 정지 조건 평가기 (plan_26090415 §4.6·§4.7·§4.8 · CP1)
#
# 계약:  stop ⟸ (남은 셀 없음) ∨ (셀 수 예산 소진) ∨ (벽시계 예산 소진) ∨ (연속 실패 ≥ 한도)
#
# ★ 이 스크립트에는 **예산 상수가 없다.** 세 한도는 전부 상태 파일의 `declared_budget` 에서 오며,
#   하나라도 없으면 판정하지 않고 exit 2 다. 근거는 §4.8 의 HITL 2단이다 — 대화 층(깊이 승인)이
#   숫자를 낳고, 스크립트 층은 그 숫자를 받기만 한다. 그래서 여기서 기본값을 발명하면 두 층 중
#   하나가 조용히 사라진다: 기본값이 있으면 아무도 깊이를 승인하지 않아도 스윕이 돈다.
#   정상 경로에서 이 거부는 발동하지 않으며, 발동하면 대화 층을 건너뛴 비정상 호출이라는 뜻이다.
#
# ★ 연속 실패 한도의 관례값은 3 이며 explorer 의 `DEFAULT_TRIAL_CAP`(= `reconciliation_cap`)과 같은
#   숫자다. 그러나 **같은 개념이 아니다** — trial cap 은 한 레시피 안에서 수렴을 몇 번 시도하는가이고,
#   여기 한도는 서로 다른 셀이 연달아 몇 번 무너지면 스윕 자체를 의심하는가다. 값이 우연히 같다고
#   상수를 공유하면 한쪽을 조정할 때 다른 쪽이 따라 움직인다(workflow.md §4종 안티패턴: 개념 중복은
#   값 스캔으로 찾지 못한다). 여기서는 아예 상수를 두지 않으므로 그 함정에 빠질 자리가 없다.
#   `reconciliation_cap` 을 **셀 수 예산**으로 전용하지 않는 것도 같은 이유다(재탐색 상한 ≠ 셀 예산).
#
# ★ 셀 종결 3분류(§4.6)는 이 평가기의 입력이다. `measurement_void` 는 서빙은 됐는데 측정이 파괴된
#   경우(워치독 사살 등)이며 **연속 실패에 포함된다** — 성능이 나쁜 게 아니라 잴 수 없었다는 뜻이라
#   진행해도 얻는 것이 없기 때문이다. 반대로 verdict 가 REFUTE 인 것은 **실패가 아니다**(잴 수
#   있었고 결과가 나왔다). 그것을 실패로 세면 "느린 레시피가 연달아 나왔다"가 스윕 중단 사유가 되어
#   지도에 구멍을 낸다.
#
# 시각: `--now-utc` 주입만 사용한다(벽시계 금지 — docs.md §기계판독 데이터 평면·staleness_gate 선례).
# 입출력: stdin/파일 JSON 상태 → stdout JSON 판정. stdlib only.
# 종료:  0=계속 · 3=정지(정상 판정) · 2=인자/선언 오류(판정 불가)
import argparse
import datetime as _dt
import json
import sys

SCHEMA_VERSION = 1

# 셀 종결 3분류(§4.6). 닫힌 목록이며 tripwire 다 — 새 분류를 도입하면 여기서 먼저 빨간불이 켜지고,
# 그때 "이 분류는 연속 실패에 드는가"를 사람이 판단한다. 조용히 자라지 않게 한다.
OUTCOME_MEASURED = "measured"
OUTCOME_SERVE_FAILED = "serve_failed"
OUTCOME_MEASUREMENT_VOID = "measurement_void"
CELL_OUTCOMES = (OUTCOME_MEASURED, OUTCOME_SERVE_FAILED, OUTCOME_MEASUREMENT_VOID)
FAILURE_OUTCOMES = (OUTCOME_SERVE_FAILED, OUTCOME_MEASUREMENT_VOID)

BUDGET_KEYS = ("max_cells", "wall_clock_budget_s", "consecutive_failure_limit")
BUDGET_PROVENANCE_KEYS = ("declared_by", "basis")

STOP_CELLS_EXHAUSTED = "cells_exhausted"
STOP_CELL_BUDGET = "cell_budget_exhausted"
STOP_WALL_CLOCK = "wall_clock_exhausted"
STOP_CONSECUTIVE_FAILURES = "consecutive_failures"


class StopEvalError(ValueError):
    """상태·선언이 판정 가능한 형태가 아니다. 추정으로 메우지 않고 호출자에게 돌려준다."""


def _parse_utc(value, field):
    if not isinstance(value, str) or not value.strip():
        raise StopEvalError("%s 는 ISO-8601 UTC 문자열이어야 한다(받은 값: %r)" % (field, value))
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = _dt.datetime.fromisoformat(text)
    except ValueError as exc:
        raise StopEvalError("%s 를 시각으로 읽을 수 없다(%r): %s" % (field, value, exc)) from exc
    if parsed.tzinfo is None:
        raise StopEvalError("%s 에 타임존이 없다(%r) — 벽시계 추정 금지" % (field, value))
    return parsed.astimezone(_dt.timezone.utc)


def _positive_int(container, key, where):
    if key not in container:
        raise StopEvalError(
            "%s.%s 미선언 — 예산은 대화 층(깊이 HITL)이 낳는다(§4.8). "
            "스크립트가 기본값을 발명하면 승인 없이 스윕이 돈다." % (where, key))
    value = container[key]
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise StopEvalError("%s.%s 는 양의 정수여야 한다(받은 값: %r)" % (where, key, value))
    return value


def evaluate(state, now_utc):
    """상태 → 정지 판정. 순수 함수이며 파일·시계·환경을 읽지 않는다."""
    if not isinstance(state, dict):
        raise StopEvalError("상태는 JSON 객체여야 한다")

    budget = state.get("declared_budget")
    if not isinstance(budget, dict):
        raise StopEvalError(
            "declared_budget 부재 — 셀 수·벽시계·연속 실패 한도를 선언 없이 판정하지 않는다(§4.8)")
    limits = {key: _positive_int(budget, key, "declared_budget") for key in BUDGET_KEYS}
    for key in BUDGET_PROVENANCE_KEYS:
        text = budget.get(key)
        if not isinstance(text, str) or not text.strip():
            raise StopEvalError(
                "declared_budget.%s 가 비었다 — 누가 언제 왜 이 숫자를 승인했는지가 남지 않으면 "
                "'사람이 숫자를 발명하지 않았다'가 검증 불가가 된다(§4.8)" % key)

    planned = state.get("cells_planned")
    if isinstance(planned, bool) or not isinstance(planned, int) or planned < 0:
        raise StopEvalError("cells_planned 는 0 이상 정수여야 한다(받은 값: %r)" % (planned,))

    cells = state.get("cells")
    if not isinstance(cells, list):
        raise StopEvalError("cells 는 리스트여야 한다(받은 값: %r)" % type(cells).__name__)

    attempted_keys = []
    consecutive = 0
    for pos, cell in enumerate(cells):
        if not isinstance(cell, dict):
            raise StopEvalError("cells[%d] 는 객체여야 한다" % pos)
        key = cell.get("cell_key")
        if not isinstance(key, str) or not key.strip():
            raise StopEvalError("cells[%d].cell_key 가 비었다 — 셀 좌표 없이는 지도가 성립하지 않는다" % pos)
        outcome = cell.get("cell_outcome")
        if outcome not in CELL_OUTCOMES:
            raise StopEvalError(
                "cells[%d].cell_outcome 이 3분류 밖이다(받은 값: %r · 허용: %s). "
                "모르는 분류를 실패로 접지 않는다 — 부재와 결측은 다르다."
                % (pos, outcome, "|".join(CELL_OUTCOMES)))
        attempted_keys.append(key)
        consecutive = 0 if outcome == OUTCOME_MEASURED else consecutive + 1

    remaining = [key for key in (state.get("cells_remaining") or []) if isinstance(key, str)]
    if not isinstance(state.get("cells_remaining", []), list):
        raise StopEvalError("cells_remaining 은 리스트여야 한다")

    started = _parse_utc(state.get("started_utc"), "started_utc")
    elapsed_s = int((now_utc - started).total_seconds())
    if elapsed_s < 0:
        raise StopEvalError(
            "started_utc 가 --now-utc 보다 미래다(경과 %ds) — 시각 주입이 뒤엉켰다" % elapsed_s)

    reasons = []
    if not remaining:
        reasons.append(STOP_CELLS_EXHAUSTED)
    if len(attempted_keys) >= limits["max_cells"]:
        reasons.append(STOP_CELL_BUDGET)
    if elapsed_s >= limits["wall_clock_budget_s"]:
        reasons.append(STOP_WALL_CLOCK)
    if consecutive >= limits["consecutive_failure_limit"]:
        reasons.append(STOP_CONSECUTIVE_FAILURES)

    return {
        "schema_version": SCHEMA_VERSION,
        "stop": bool(reasons),
        # 여러 조건이 동시에 성립할 수 있고 그 전부를 싣는다 — 첫 이유만 남기면 "예산도 소진됐다"가
        # 사라져 재개 판단이 틀어진다.
        "stopped_by": reasons,
        # 완결 판정은 이유가 아니라 **남은 셀**로 한다. 연속 실패로 멈춰도 남은 셀이 없으면 그 지도는
        # 계획한 만큼은 다 돈 것이고, 반대로 이유가 없어도 남았으면 미완이다.
        "sweep_status": "complete" if not remaining else "incomplete",
        "cells_planned": planned,
        "cells_attempted": len(attempted_keys),
        "cells_remaining": remaining,
        "consecutive_failures": consecutive,
        "elapsed_s": elapsed_s,
        "declared_budget": dict(limits),
        "budget_declared_by": budget["declared_by"],
        "budget_basis": budget["basis"],
        "evaluated_utc": now_utc.strftime("%Y-%m-%dT%H:%M:%SZ"),
    }


def _self_test():
    """결정론 자체검사 — 파일 입력 불요. 0=전부 통과 · 1=실패 나열."""
    failures = []

    def check(name, condition, detail=""):
        if condition:
            print("  [PASS] %s" % name)
        else:
            print("  [FAIL] %s %s" % (name, detail))
            failures.append(name)

    def expect_error(name, state, fragment, now="2026-09-04T10:00:00Z"):
        try:
            evaluate(state, _parse_utc(now, "now"))
        except StopEvalError as exc:
            check(name, fragment in str(exc), "(기대 조각 %r · 실제 %r)" % (fragment, str(exc)))
        else:
            check(name, False, "(예외가 나지 않았다)")

    budget = {"max_cells": 6, "wall_clock_budget_s": 3600, "consecutive_failure_limit": 3,
              "declared_by": "hitl:2026-09-04 깊이 승인", "basis": "직전 로드 실측 12분 × 6셀"}

    def state(cells, remaining, planned=6, started="2026-09-04T09:00:00Z", budget_over=None):
        b = dict(budget)
        if budget_over:
            b.update(budget_over)
        return {"declared_budget": b, "cells_planned": planned, "started_utc": started,
                "cells": cells, "cells_remaining": remaining}

    def cell(key, outcome):
        return {"cell_key": key, "cell_outcome": outcome}

    now = _parse_utc("2026-09-04T09:30:00Z", "now")

    # S1: 아무것도 소진되지 않았으면 계속한다.
    out = evaluate(state([cell("c1", OUTCOME_MEASURED)], ["c2", "c3"]), now)
    check("S1 진행 중이면 stop=False", out["stop"] is False and out["stopped_by"] == []
          and out["sweep_status"] == "incomplete" and out["cells_attempted"] == 1)

    # S2: 남은 셀이 없으면 정지·완결.
    out = evaluate(state([cell("c1", OUTCOME_MEASURED)], []), now)
    check("S2 남은 셀 0 → stop·complete",
          out["stop"] is True and out["stopped_by"] == [STOP_CELLS_EXHAUSTED]
          and out["sweep_status"] == "complete")

    # S3: 연속 실패 한도. 두 실패 분류가 **섞여도** 연속으로 센다.
    out = evaluate(state([cell("c1", OUTCOME_SERVE_FAILED), cell("c2", OUTCOME_MEASUREMENT_VOID),
                          cell("c3", OUTCOME_SERVE_FAILED)], ["c4"]), now)
    check("S3 연속 실패 3(분류 혼합) → stop",
          out["stop"] is True and STOP_CONSECUTIVE_FAILURES in out["stopped_by"]
          and out["consecutive_failures"] == 3 and out["sweep_status"] == "incomplete")

    # S4: 성공이 끼면 카운터가 끊긴다 — 이것이 "연속"의 전부다.
    out = evaluate(state([cell("c1", OUTCOME_SERVE_FAILED), cell("c2", OUTCOME_MEASURED),
                          cell("c3", OUTCOME_SERVE_FAILED)], ["c4"]), now)
    check("S4 성공이 끼면 연속 카운터 리셋",
          out["stop"] is False and out["consecutive_failures"] == 1)

    # S5: 셀 수 예산.
    out = evaluate(state([cell("c%d" % i, OUTCOME_MEASURED) for i in range(6)], ["c9"]), now)
    check("S5 셀 수 예산 소진 → stop",
          out["stop"] is True and STOP_CELL_BUDGET in out["stopped_by"])

    # S6: 벽시계 예산(경계 포함).
    out = evaluate(state([cell("c1", OUTCOME_MEASURED)], ["c2"]),
                   _parse_utc("2026-09-04T10:00:00Z", "now"))
    check("S6 벽시계 경계값 포함 → stop",
          out["stop"] is True and STOP_WALL_CLOCK in out["stopped_by"] and out["elapsed_s"] == 3600)

    # S7: 동시 성립은 전부 싣는다.
    out = evaluate(state([cell("c%d" % i, OUTCOME_SERVE_FAILED) for i in range(6)], []),
                   _parse_utc("2026-09-04T11:00:00Z", "now"))
    check("S7 동시 성립 4가지를 모두 싣는다",
          sorted(out["stopped_by"]) == sorted([STOP_CELLS_EXHAUSTED, STOP_CELL_BUDGET,
                                               STOP_WALL_CLOCK, STOP_CONSECUTIVE_FAILURES]))

    # S8~S12: 선언 fail-closed. 여기가 이 스크립트의 존재 이유이므로 음성 사례를 두껍게 둔다.
    expect_error("S8 declared_budget 부재 → 거부",
                 {"cells_planned": 1, "started_utc": "2026-09-04T09:00:00Z",
                  "cells": [], "cells_remaining": ["c1"]}, "declared_budget 부재")
    for key in BUDGET_KEYS:
        broken = dict(budget)
        del broken[key]
        expect_error("S9 %s 미선언 → 거부" % key,
                     state([], ["c1"], budget_over=None) | {"declared_budget": broken},
                     "%s 미선언" % key)
    expect_error("S10 한도 0 → 거부", state([], ["c1"], budget_over={"max_cells": 0}),
                 "양의 정수")
    expect_error("S11 declared_by 공백 → 거부", state([], ["c1"], budget_over={"declared_by": "  "}),
                 "declared_budget.declared_by")
    expect_error("S12 basis 부재 → 거부", state([], ["c1"], budget_over={"basis": ""}),
                 "declared_budget.basis")

    # S13: 모르는 분류를 실패로 접지 않는다.
    expect_error("S13 미지의 cell_outcome → 거부(실패로 접지 않는다)",
                 state([{"cell_key": "c1", "cell_outcome": "refuted"}], ["c2"]),
                 "3분류 밖")

    # S14: verdict REFUTE 는 measured 이므로 실패가 아니다(§4.6 — 잴 수 있었다).
    out = evaluate(state([cell("c1", OUTCOME_MEASURED), cell("c2", OUTCOME_MEASURED),
                          cell("c3", OUTCOME_MEASURED)], ["c4"]), now)
    check("S14 measured 는 verdict 와 무관하게 실패가 아니다",
          out["stop"] is False and out["consecutive_failures"] == 0)

    # S15: 시각 주입 뒤엉킴.
    expect_error("S15 시작이 미래 → 거부",
                 state([], ["c1"], started="2026-09-04T23:00:00Z"), "미래다")

    # S16: 예산·근거는 산출물이 스스로 밝힌다(출처 표시 규율).
    out = evaluate(state([cell("c1", OUTCOME_MEASURED)], ["c2"]), now)
    check("S16 산출물이 예산과 승인 출처를 싣는다",
          out["declared_budget"] == {k: budget[k] for k in BUDGET_KEYS}
          and out["budget_declared_by"] == budget["declared_by"]
          and out["budget_basis"] == budget["basis"])

    if failures:
        sys.stderr.write("[sweep_stop --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[sweep_stop --self-test] OK — S1~S16 전부 통과")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="Broad Search 정지 조건 평가기(결정론)")
    ap.add_argument("--state", help="스윕 상태 JSON 경로('-' = stdin)")
    ap.add_argument("--now-utc", help="현재 시각(ISO-8601 UTC). 주입만 사용한다 — 벽시계 금지")
    ap.add_argument("--self-test", action="store_true", help="결정론 자체검사(S1~S16)")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not args.state or not args.now_utc:
        sys.stderr.write("[sweep_stop] ERROR --state 와 --now-utc 는 필수다(--self-test 제외)\n")
        return 2

    try:
        if args.state == "-":
            state = json.load(sys.stdin)
        else:
            with open(args.state, encoding="utf-8") as handle:
                state = json.load(handle)
    except (OSError, ValueError) as exc:
        sys.stderr.write("[sweep_stop] ERROR 상태를 읽을 수 없다: %s\n" % exc)
        return 2

    try:
        result = evaluate(state, _parse_utc(args.now_utc, "--now-utc"))
    except StopEvalError as exc:
        sys.stderr.write("[sweep_stop] ERROR %s\n" % exc)
        return 2

    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 3 if result["stop"] else 0


if __name__ == "__main__":
    sys.exit(main())
