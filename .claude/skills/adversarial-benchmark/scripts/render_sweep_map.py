#!/usr/bin/env python3
# render_sweep_map.py — 광의의 탐색 지도 렌더러 (plan_26090415 §4.5·§4.7 · CP6)
#
# ★ 이 렌더러는 **순위를 만들지 않는다.** 목적함수도, 단일 스칼라도, 파레토 지배관계 선언도 없다.
#   축을 접는 순간 가중치가 들어가고 그것은 **용처 결정**이며 사람 몫이다(사용자 결정 2026-09-04).
#   근거는 둘이다: ⓐ 정규화·가중치 상수는 손저작일 수밖에 없어 매직넘버 **결함** 칸에 해당한다.
#   ⓑ 단일 지점으로 접으면 승자가 뒤집힌다는 실측이 있다(동시성 1 동률인데 2·4 에서 역전 —
#   `selfport-beats-fork-ladder-proof`). 그래서 셀은 **실행 순서**로만 나열한다. 실행 순서는
#   평가가 아니므로 어떤 축도 은근히 우대하지 않는다.
#
# ★ 그 금지는 산문이 아니라 **결정론으로 단언**한다(`assert_no_ranking`). 산문으로만 적으면
#   다음 편집이 조용히 `sorted(..., key=decode_tps)` 한 줄을 넣는다.
#
# ★ 미완 지도도 **발행한다**(§4.7 · `sweep_status: incomplete`). 부분 지도를 숨기면 "돌다 말았다"와
#   "돌지 않았다"가 구분되지 않는다 — truncation.log 의 silent-truncation 금지를 셀 층위로 넓힌 것이다.
#
# 사용: render_sweep_map.py --state PATH --stop-json PATH --out-md PATH [--out-json PATH]
#       render_sweep_map.py --self-test
# 종료: 0=발행 · 2=입력 오류 · 3=순위 금지 위반(자기 산출물 거부)
import argparse
import json
import sys

# 닫힌 금지 키 목록(tripwire). 값이 아니라 **키**를 본다 — 셀 이름에 'best' 가 들어갈 수는 있어도
# 산출물이 'best' 라는 *필드*를 갖는 순간 그것은 판단이다.
FORBIDDEN_KEYS = frozenset({
    "winner", "rank", "ranking", "best", "top", "score", "recommended", "recommendation",
    "champion", "leader", "dominates", "pareto_front",
})
# 마크다운 표에 순위 열이 생기는 것도 같은 위반이다.
FORBIDDEN_HEADERS = ("순위", "1위", "추천", "winner", "rank", "best")


class RenderError(ValueError):
    pass


def assert_no_ranking(payload, path="$"):
    """산출물 어디에도 순위 필드가 없음을 단언한다(깊이 무제한)."""
    if isinstance(payload, dict):
        for key, value in payload.items():
            if isinstance(key, str) and key.lower() in FORBIDDEN_KEYS:
                raise RenderError(
                    "순위 필드 금지 위반: %s.%s — 이 지도는 순위를 만들지 않는다(§4.5). "
                    "축을 접는 것은 용처 결정이며 사람 몫이다." % (path, key))
            assert_no_ranking(value, "%s.%s" % (path, key))
    elif isinstance(payload, list):
        for i, value in enumerate(payload):
            assert_no_ranking(value, "%s[%d]" % (path, i))


def build(state, stop):
    """상태 + 정지 판정 → 지도 문서(JSON). 셀은 **실행 순서**로만 나열한다."""
    if not isinstance(state, dict) or not isinstance(stop, dict):
        raise RenderError("state 와 stop-json 은 모두 JSON 객체여야 한다")
    cells = state.get("cells")
    if not isinstance(cells, list):
        raise RenderError("state.cells 는 리스트여야 한다")

    rows = []
    for i, cell in enumerate(cells):
        if not isinstance(cell, dict):
            raise RenderError("cells[%d] 는 객체여야 한다" % i)
        rows.append({
            "order": i + 1,                       # 실행 순서 — 평가가 아니다
            "cell_key": cell.get("cell_key"),
            "coordinates": cell.get("coordinates"),
            "cell_outcome": cell.get("cell_outcome"),
            # 동시성 축은 **벡터 전부**를 싣는다. 한 점으로 접으면 승자가 뒤집힌다.
            "concurrency_vector": cell.get("concurrency_vector"),
            # 용량 축은 속도와 **합치지 않고 나란히** 둔다.
            "capacity": cell.get("capacity"),
            "verdict_narrative": cell.get("verdict_narrative"),
            "measurement": cell.get("measurement"),
            "void_reason": cell.get("void_reason"),
            "void_reason_source": cell.get("void_reason_source"),
            "failure_note": cell.get("note"),
            # M3 — 자율 축 판단의 근거. 헌법 불변식 B: 인용 없는 결정은 거짓이 아니라 **누락**이며
            # 누락은 기계가 fail-closed 로 잡는다. 여기서는 누락을 **보이게** 만든다.
            "axis_citation": cell.get("axis_citation"),
            "axis_citation_present": bool(cell.get("axis_citation")),
        })

    uncited = [r["cell_key"] for r in rows if not r["axis_citation_present"]]
    return {
        "schema_version": 1,
        "kind": "broad_search_sweep_map",
        "sweep_id": state.get("sweep_id"),
        "sweep_status": stop.get("sweep_status"),
        "stopped_by": stop.get("stopped_by"),
        "cells_planned": stop.get("cells_planned"),
        "cells_attempted": stop.get("cells_attempted"),
        "cells_remaining": stop.get("cells_remaining"),
        "consecutive_failures": stop.get("consecutive_failures"),
        "elapsed_s": stop.get("elapsed_s"),
        "declared_budget": stop.get("declared_budget"),
        "budget_declared_by": stop.get("budget_declared_by"),
        "budget_basis": stop.get("budget_basis"),
        "rubric_authority": state.get("rubric_authority"),
        "control_variable": state.get("control_variable"),
        "cells": rows,
        "axis_citation_missing": uncited,
        "ordering": "execution_order",
        "ordering_note": "실행 순서다. 성능 순서가 아니며 이 문서는 순위를 만들지 않는다(§4.5).",
    }


def render_markdown(doc):
    out = []
    A = out.append
    A("# 광의의 탐색 지도 — %s" % (doc.get("sweep_id") or "N/A"))
    A("")
    A("> **이 문서는 순위를 만들지 않는다.** 셀은 **실행 순서**로 나열되며 성능 순서가 아니다.")
    A("> 평가축이 여러 개이므로 상황에 맞는 레시피 선택과 의사결정은 사람이 한다 —")
    A("> 여기 실린 것은 레시피별 **진실된 측정 결과**뿐이다.")
    A("")
    A("| 항목 | 값 |")
    A("|---|---|")
    A("| 상태 | `%s` |" % doc.get("sweep_status"))
    A("| 정지 사유 | %s |" % (", ".join(doc.get("stopped_by") or []) or "—"))
    A("| 셀 계획/시도/남음 | %s / %s / %s |"
      % (doc.get("cells_planned"), doc.get("cells_attempted"),
         len(doc.get("cells_remaining") or [])))
    A("| 연속 실패 | %s |" % doc.get("consecutive_failures"))
    A("| 경과 | %ss |" % doc.get("elapsed_s"))
    A("| 통제변인 | %s |" % (doc.get("control_variable") or "N/A"))
    A("| 루브릭 권한 | `%s` |" % (doc.get("rubric_authority") or "N/A"))
    A("| 선언 예산 | `%s` |" % json.dumps(doc.get("declared_budget"), ensure_ascii=False))
    A("| 예산 승인 | %s |" % (doc.get("budget_declared_by") or "N/A"))
    A("| 예산 근거 | %s |" % (doc.get("budget_basis") or "N/A"))
    A("")
    if doc.get("cells_remaining"):
        A("**미완이다.** 남은 셀: %s" % ", ".join("`%s`" % c for c in doc["cells_remaining"]))
        A("")
    if doc.get("axis_citation_missing"):
        A("⚠ **도서관 인용 누락 셀**: %s"
          % ", ".join("`%s`" % c for c in doc["axis_citation_missing"]))
        A("> 인용 없는 축 판단은 거짓이 아니라 **누락**이다(헌법 불변식 B). 지도에 남겨 보이게 한다.")
        A("")
    A("## 셀 (실행 순서)")
    A("")
    for row in doc.get("cells") or []:
        A("### %d. `%s` — `%s`" % (row["order"], row["cell_key"], row["cell_outcome"]))
        A("")
        A("- 좌표: `%s`" % json.dumps(row.get("coordinates"), ensure_ascii=False))
        A("- 동시성 축(전 벡터): `%s`" % json.dumps(row.get("concurrency_vector"), ensure_ascii=False))
        A("- 용량 축(속도와 합치지 않는다): `%s`" % json.dumps(row.get("capacity"), ensure_ascii=False))
        A("- 측정 조건·도구: `%s`" % json.dumps(row.get("measurement"), ensure_ascii=False))
        if row.get("verdict_narrative"):
            A("- 판정 서술: %s" % row["verdict_narrative"])
        if row.get("void_reason"):
            A("- 측정 무효 사인: `%s` (출처 `%s`)" % (row["void_reason"], row.get("void_reason_source")))
        if row.get("failure_note"):
            A("- 비고: %s" % row["failure_note"])
        A("- 축 근거(도서관 인용): %s" % (row.get("axis_citation") or "**누락**"))
        A("")
    return "\n".join(out) + "\n"


def _self_test():
    failures = []

    def check(name, condition, detail=""):
        if condition:
            print("  [PASS] %s" % name)
        else:
            print("  [FAIL] %s %s" % (name, detail)); failures.append(name)

    state = {
        "sweep_id": "bs_2026090416",
        "rubric_authority": "explore",
        "control_variable": "target HW: GB10 unified 120GiB",
        "cells": [
            {"cell_key": "len131072-kvfp8", "coordinates": {"max_model_len": 131072},
             "cell_outcome": "measured", "concurrency_vector": {"1": 34.4, "2": 60.1},
             "capacity": {"kv_gib": 12.0}, "verdict_narrative": "explore: E 대비 서술",
             "measurement": {"bench_tool": "guidellm", "bench_tool_version": "0.7.3"},
             "axis_citation": "wiki: kv-clamp-portability"},
            {"cell_key": "len262144-kvfp8", "coordinates": {"max_model_len": 262144},
             "cell_outcome": "serve_failed", "note": "OOM", "concurrency_vector": None,
             "capacity": None, "measurement": None},
        ],
    }
    stop = {"sweep_status": "incomplete", "stopped_by": ["consecutive_failures"],
            "cells_planned": 4, "cells_attempted": 2, "cells_remaining": ["c3", "c4"],
            "consecutive_failures": 1, "elapsed_s": 1800,
            "declared_budget": {"max_cells": 4, "wall_clock_budget_s": 7200,
                                "consecutive_failure_limit": 3},
            "budget_declared_by": "hitl:2026-09-04", "budget_basis": "직전 로드 실측"}

    doc = build(state, stop)
    check("R1 셀은 실행 순서로만 나열된다",
          [c["order"] for c in doc["cells"]] == [1, 2] and doc["ordering"] == "execution_order")
    check("R2 순위 필드가 없다(자기 단언 통과)", assert_no_ranking(doc) is None)
    check("R3 미완이 산출물에 남는다",
          doc["sweep_status"] == "incomplete" and doc["cells_remaining"] == ["c3", "c4"])
    check("R4 인용 누락 셀이 드러난다", doc["axis_citation_missing"] == ["len262144-kvfp8"])
    check("R5 동시성은 벡터 전부·용량은 나란히",
          doc["cells"][0]["concurrency_vector"] == {"1": 34.4, "2": 60.1}
          and doc["cells"][0]["capacity"] == {"kv_gib": 12.0})

    md = render_markdown(doc)
    lower = md.lower()
    check("R6 마크다운에 순위 열이 없다",
          not any(("| %s" % h.lower()) in lower for h in FORBIDDEN_HEADERS))
    check("R7 마크다운이 비순위 계약을 명시한다", "순위를 만들지 않는다" in md)

    # 음성 회귀: 순위 필드를 심으면 반드시 거부한다.
    poisoned = json.loads(json.dumps(doc))
    poisoned["cells"][0]["rank"] = 1
    try:
        assert_no_ranking(poisoned)
    except RenderError as exc:
        check("R8 ★음성대조: 순위 필드를 심으면 거부", "순위 필드 금지 위반" in str(exc))
    else:
        check("R8 ★음성대조: 순위 필드를 심으면 거부", False, "(예외가 나지 않았다)")

    poisoned2 = json.loads(json.dumps(doc))
    poisoned2["summary"] = {"nested": {"best": "c1"}}
    try:
        assert_no_ranking(poisoned2)
    except RenderError as exc:
        check("R9 ★음성대조: 깊이 무제한으로 잡는다", "summary.nested.best" in str(exc))
    else:
        check("R9 ★음성대조: 깊이 무제한으로 잡는다", False, "(예외가 나지 않았다)")

    ok_value = json.loads(json.dumps(doc))
    ok_value["cells"][0]["cell_key"] = "best-effort-len131072"
    check("R10 값에 들어간 단어는 위반이 아니다(키만 본다)", assert_no_ranking(ok_value) is None)

    if failures:
        sys.stderr.write("[render_sweep_map --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[render_sweep_map --self-test] OK — R1~R10 전부 통과")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="광의의 탐색 지도 렌더러(순위 ✗)")
    ap.add_argument("--state")
    ap.add_argument("--stop-json")
    ap.add_argument("--out-md")
    ap.add_argument("--out-json")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not (args.state and args.stop_json and args.out_md):
        sys.stderr.write("[render_sweep_map] ERROR --state · --stop-json · --out-md 는 필수다\n")
        return 2
    try:
        with open(args.state, encoding="utf-8") as handle:
            state = json.load(handle)
        with open(args.stop_json, encoding="utf-8") as handle:
            stop = json.load(handle)
        doc = build(state, stop)
    except (OSError, ValueError) as exc:
        sys.stderr.write("[render_sweep_map] ERROR %s\n" % exc)
        return 2
    try:
        assert_no_ranking(doc)
    except RenderError as exc:
        # 자기 산출물을 스스로 거부한다 — 발행 후에 발견하면 이미 지도가 나간 뒤다.
        sys.stderr.write("[render_sweep_map] ERROR %s\n" % exc)
        return 3
    with open(args.out_md, "w", encoding="utf-8") as handle:
        handle.write(render_markdown(doc))
    if args.out_json:
        with open(args.out_json, "w", encoding="utf-8") as handle:
            json.dump(doc, handle, ensure_ascii=False, indent=2)
    print("[render_sweep_map] MAP=%s%s" % (args.out_md,
                                           (" JSON=%s" % args.out_json) if args.out_json else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
