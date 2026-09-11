#!/usr/bin/env python3
# escalation_predicate.py — escalation 발견 술어 ①("구동불가 증상")의 결정론 판정기
#   (2026-09-11 신설 · plan_26091108 R7 · SKILL.md §5.5)
#
# 왜 있는가. §5.5 는 escalation 을 **술어 둘의 동시 충족**으로 정의한다 —
#   ① 구동불가 증상 ∧ ② 외부 교차검증 확증. 그런데 ① 의 정의가 산문뿐이었고 **그것을 판정하는
#   실행자가 0** 이었다. 그 공백에서 camp-26090918 이 오진했다: 호스트 워치독이 22분 로딩 뒤
#   컨테이너를 사살한 것을 "FP8 컨테이너 일관 hang" 으로 적었고, 그 서술이 "모델 파일이 문제"
#   라는 결론과 근거 없는 체크포인트 재다운로드로 이어졌다.
#
# ★ 이 파일이 집행하는 한 문장:
#     **호스트 평면 사인은 ① 을 만들지 못한다.**
#   워치독 사살·earlyoom·열 사살은 "이 모델을 이 vLLM 이 못 받는다" 가 아니라 "이 호스트가
#   지금 이만큼의 상주를 감당하지 못한다" 는 진술이다. 전자는 버전핀·포크를 움직이고 후자는
#   예산·KV 클램프·PLE 모드를 움직인다 — 처방이 정반대다. 호스트 사인으로 ① 을 세우면
#   빌드 평면이 무고하게 흔들리고, 진짜 원인(예산)은 그대로 남아 다음 캠페인에서 재발한다.
#
# ★ 사살은 `serve_init_immediate_death` 를 **반증한다**. 구간 안에 집행된 사살이 있으면 그
#   죽음은 "즉사" 가 아니라 "사살" 이다 — 같은 관측(컨테이너가 사라졌다)에 두 해석이 있고
#   시각 대조가 그것을 가른다. 이 반증이 없으면 사살이 매번 ① 로 승격된다.
#
# 사살 이벤트 어휘의 **단일 소유는 `classify_cell.py`** 다(KILL_EVENT_KINDS·TRIP_EVENT_KINDS).
#   여기에 사본을 두지 않고 import 한다 — 거울을 두면 한쪽이 움직일 때 갈라진다.
#
# 사용: escalation_predicate.py --symptom <kind>... [--events PATH]...
#         [--started-utc T --ended-utc T] [--tolerance-s N] [--json]
#       escalation_predicate.py --self-test
# 종료: 0=① 충족 · 1=① 미충족 · 2=인자 오류
import argparse
import importlib.util
import json
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
_CLASSIFY = os.path.normpath(os.path.join(
    _HERE, "..", "..", "adversarial-benchmark", "scripts", "classify_cell.py"))


def _load_classifier():
    """사살 어휘의 정본을 import 한다. 부재는 **침묵하지 않는다** — 사본을 만들어 메우면
    그 순간 두 자리가 갈라지기 시작한다."""
    if not os.path.isfile(_CLASSIFY):
        raise SystemExit(
            "[escalation] FAIL: 사살 어휘의 정본을 찾지 못했다 — %s\n"
            "  여기에 사본을 두지 않는다(거울 금지). adversarial-benchmark 배달을 확인하라."
            % _CLASSIFY)
    spec = importlib.util.spec_from_file_location("_classify_cell", _CLASSIFY)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# ① 을 **만들 수 있는** 증상. 닫힌 목록(tripwire) — 새 증상을 늘리려면 이 줄을 고치게 하고,
# 그 편집이 리뷰를 강제한다. 셋 다 §5.5 산문에 이미 있던 것이고 새로 만든 것이 아니다.
MODEL_PLANE_SYMPTOMS = {
    "config_arch_unsupported":
        "config 의 architectures/quant_method 를 이 vLLM 이 모른다",
    "serve_init_immediate_death":
        "serve init 즉사 — 가중치 로드 이전에 죽는다",
    "transformers_only_fallback":
        "transformers-only 폴백 신호(네이티브 경로 부재)",
}

# ① 을 **만들지 못하는** 증상. 호스트 자원 평면의 판정이며 모델·버전과 독립이다.
# 이름은 노드 블랙박스 이벤트의 kind 와 같은 어휘를 쓴다(대조가 가능해야 한다).
HOST_PLANE_SIGNS = {
    "watchdog_kill_ack": "호스트 메모리 워치독이 집행한 사살",
    "watchdog_trip": "호스트 메모리 워치독의 트립 판정",
    "thermal_kill_ack": "SoC 열 임계 사살",
    "thermal_trip": "SoC 열 임계 트립 판정",
    "earlyoom_kill": "earlyoom 사살",
    "docker_kill": "호스트 감시자가 수행한 컨테이너 kill",
    "oom_killer": "커널 OOM killer",
}

# 사살이 관측되면 **반증되는** 모델 평면 증상. 같은 관측에 두 해석이 있고 시각 대조가 가른다.
REFUTED_BY_KILL = ("serve_init_immediate_death",)


def judge(symptoms, kill_hits, events_scanned="none"):
    """순수 함수. 반환 dict — `predicate_1` 은 bool 이고 나머지는 그 판정의 근거다."""
    symptoms = list(symptoms or [])
    kill_hits = list(kill_hits or [])
    classify = _load_classifier()
    executed = [h for h in kill_hits if h.get("kind") in classify.KILL_EVENT_KINDS]

    # ★ 호스트 평면이 **우선한다**(2026-09-11 음성대조가 잡은 결함). 종전에는 두 목록이 서로
    #   배타적이라는 가정 위에 서 있었고, 그래서 호스트 사인이 모델 목록에 한 줄 들어가는
    #   것만으로 이 가드가 조용히 죽었다 — 목록 위생에 의존하는 가드는 가드가 아니다.
    #   겹침 자체는 판정이 아니라 **배선 오류**이므로 소리를 내고 판정에서는 호스트가 이긴다.
    collisions = sorted(set(MODEL_PLANE_SYMPTOMS) & set(HOST_PLANE_SIGNS))
    unknown = [s for s in symptoms if s not in MODEL_PLANE_SYMPTOMS and s not in HOST_PLANE_SIGNS]
    host_used = [s for s in symptoms if s in HOST_PLANE_SIGNS]
    model_claimed = [s for s in symptoms
                     if s in MODEL_PLANE_SYMPTOMS and s not in HOST_PLANE_SIGNS]

    refuted = []
    if executed:
        refuted = [s for s in model_claimed if s in REFUTED_BY_KILL]
    model_standing = [s for s in model_claimed if s not in refuted]

    reasons = []
    for s in host_used:
        reasons.append(
            "호스트 평면 사인 '%s'(%s)은 술어 ① 을 만들지 못한다 — 그것은 '이 모델을 이 vLLM 이 "
            "못 받는다' 가 아니라 '이 호스트가 지금 이만큼의 상주를 감당하지 못한다' 는 진술이다. "
            "처방은 예산·KV 클램프·PLE 모드이지 버전핀·포크가 아니다." % (s, HOST_PLANE_SIGNS[s]))
    for s in refuted:
        first = executed[0]
        reasons.append(
            "'%s' 은 구간 안에 집행된 사살(%s @ %s)이 있어 **반증된다** — 그 죽음은 즉사가 "
            "아니라 사살이다." % (s, first.get("kind"), first.get("ts")))
    for s in unknown:
        reasons.append(
            "'%s' 은 닫힌 목록 어디에도 없다 — 증상 어휘를 늘리려면 MODEL_PLANE_SYMPTOMS 를 "
            "고쳐라(조용히 통과시키지 않는다)." % s)
    for s in model_standing:
        reasons.append("모델 평면 증상 '%s'(%s) 성립." % (s, MODEL_PLANE_SYMPTOMS[s]))

    for s in collisions:
        reasons.append(
            "배선 오류: '%s' 이 MODEL_PLANE_SYMPTOMS 와 HOST_PLANE_SIGNS 양쪽에 있다 — "
            "판정에서는 호스트 평면이 이긴다(가드는 목록 위생에 의존하지 않는다). "
            "두 목록 중 한쪽에서 지워라." % s)
    ok = bool(model_standing) and not unknown
    if not ok and not reasons:
        reasons.append("증상이 하나도 제시되지 않았다 — ① 은 부재로 미충족이다(추측 ✗).")
    return {
        "predicate_1": ok,
        "vocabulary_collisions": collisions,
        "model_plane_standing": model_standing,
        "model_plane_refuted": refuted,
        "host_plane_ignored": host_used,
        "unknown_symptoms": unknown,
        "kill_events_executed": executed,
        "evidence_source": events_scanned,
        "reasons": reasons,
        # ② 는 이 판정기의 소관이 아니다 — 외부 교차검증은 에이전트의 리서치이고 사람이 읽는다.
        "note": "술어 ① 만 판정한다. escalation 발동은 ① ∧ ② 이며 ② 는 외부 교차검증이다(§5.5).",
    }


def _self_test():
    failures = []

    def ck(name, cond, detail=""):
        print(("  [PASS] " if cond else "  [FAIL] ") + name + ("" if cond else " " + detail))
        if not cond:
            failures.append(name)

    kill = [{"kind": "watchdog_kill_ack", "ts": "2026-09-10T05:20:54Z", "mode": "armed"}]
    trip_only = [{"kind": "watchdog_trip", "ts": "2026-09-10T05:20:53Z", "mode": "armed"}]

    r = judge(["watchdog_kill_ack"], kill)
    ck("E1 호스트 사인 단독은 ① 을 만들지 못한다",
       r["predicate_1"] is False and r["host_plane_ignored"] == ["watchdog_kill_ack"])

    r = judge(["serve_init_immediate_death"], kill)
    ck("E2 사살이 관측되면 '즉사' 는 반증된다",
       r["predicate_1"] is False and r["model_plane_refuted"] == ["serve_init_immediate_death"])

    r = judge(["serve_init_immediate_death"], [])
    ck("E3 사살이 없으면 '즉사' 는 ① 을 만든다(과잉차단 ✗)", r["predicate_1"] is True)

    r = judge(["serve_init_immediate_death"], trip_only)
    ck("E4 trip 만으로는 반증하지 않는다(집행된 사살만 반증한다)", r["predicate_1"] is True)

    r = judge(["config_arch_unsupported"], kill)
    ck("E5 사살이 있어도 arch 미지원은 독립으로 ① 을 만든다", r["predicate_1"] is True)

    r = judge(["fused_moe_fp8_config_absent"], [])
    ck("E6 닫힌 목록 밖 증상은 조용히 통과하지 않는다",
       r["predicate_1"] is False and r["unknown_symptoms"] == ["fused_moe_fp8_config_absent"])

    r = judge([], [])
    ck("E7 증상 부재는 미충족이다(부재 ≠ 충족)", r["predicate_1"] is False)

    r = judge(["config_arch_unsupported", "watchdog_kill_ack"], kill)
    ck("E8 호스트 사인이 섞여도 모델 평면 증상이 서면 ① 은 선다",
       r["predicate_1"] is True and r["host_plane_ignored"] == ["watchdog_kill_ack"])

    # 음성대조 — 호스트 사인을 모델 평면 목록에 넣으면 이 시험이 빨간불이어야 한다.
    _saved = MODEL_PLANE_SYMPTOMS.get("watchdog_kill_ack")
    MODEL_PLANE_SYMPTOMS["watchdog_kill_ack"] = "(음성대조 주입)"
    try:
        r = judge(["watchdog_kill_ack"], kill)
        ck("★E9 음성대조: 호스트 사인을 모델 평면에 넣어도 ① 은 서지 않는다(호스트 우선은 코드다)",
           r["predicate_1"] is False)
        ck("★E9b 그 겹침을 배선 오류로 지목한다(조용한 통과 ✗)",
           r["vocabulary_collisions"] == ["watchdog_kill_ack"]
           and any("배선 오류" in x for x in r["reasons"]))
    finally:
        if _saved is None:
            MODEL_PLANE_SYMPTOMS.pop("watchdog_kill_ack", None)
        else:
            MODEL_PLANE_SYMPTOMS["watchdog_kill_ack"] = _saved

    print("[escalation_predicate] %s" % ("PASS" if not failures else "FAIL (%d)" % len(failures)))
    return 0 if not failures else 1


def main():
    ap = argparse.ArgumentParser(description="escalation 술어 ① 판정 (§5.5 · plan_26091108 R7)")
    ap.add_argument("--symptom", action="append", default=[],
                    help="관측된 증상 kind (반복 가능)")
    ap.add_argument("--events", action="append", default=[], help="노드 블랙박스 events jsonl")
    ap.add_argument("--started-utc"); ap.add_argument("--ended-utc")
    ap.add_argument("--tolerance-s", type=int, default=0)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        sys.exit(_self_test())

    hits, scanned = [], "none"
    if a.events:
        if not (a.started_utc and a.ended_utc):
            print("[escalation] FAIL: --events 를 쓰려면 --started-utc/--ended-utc 가 필요하다 "
                  "(구간 없는 대조는 전 이력을 이 셀의 사인으로 읽는다)", file=sys.stderr)
            sys.exit(2)
        classify = _load_classifier()
        lines = []
        for path in a.events:
            try:
                with open(path, encoding="utf-8") as f:
                    lines.extend(f.readlines())
            except OSError as exc:
                print("[escalation] ⚠ events 읽기 실패 %s: %s" % (path, exc), file=sys.stderr)
        hits = classify.kill_events_in_window(
            lines, classify._utc(a.started_utc, "--started-utc"),
            classify._utc(a.ended_utc, "--ended-utc"), a.tolerance_s)
        scanned = ",".join(a.events)

    out = judge(a.symptom, hits, scanned)
    if a.json:
        print(json.dumps(out, ensure_ascii=False, indent=2))
    else:
        print("[escalation] 술어 ① = %s" % ("충족" if out["predicate_1"] else "미충족"))
        for r in out["reasons"]:
            print("  - %s" % r)
    sys.exit(0 if out["predicate_1"] else 1)


if __name__ == "__main__":
    main()
