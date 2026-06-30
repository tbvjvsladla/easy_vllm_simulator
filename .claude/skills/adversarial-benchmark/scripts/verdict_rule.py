#!/usr/bin/env python3
# verdict_rule.py — 결정론 PASS/REFUTE 게이트 (adversarial-benchmark §6·§9)
#
# ★ 게이트는 *규칙*이 결정한다 — LLM 다수결 아님(§6). LLM Devil's Advocate 는 외부검색(E)·
#   진단·재탐색힌트(증거/판정)만 생산해 이 규칙에 투입. 여기서 PASS/REFUTE 가 결정론으로 닫힌다.
#
# 루브릭 우선순위(3중 방어막): primary = E(외부 현실-달성치) > target(c, 사용자) > expected_achievable(루프라인×MBU).
#   PASS  : M ≥ primary × (1 − tol)
#   REFUTE: M <  primary × (1 − tol)
#   establish 실패(E·target·expected 모두 부재/불가) → failure_axis=establish → (c) 사용자 백스톱.
# like-with-like: spec on 서브는 R_token, off 서브는 R_fp 기준(루프라인이 이미 accept_len 반영해 산출).
# CONTRACT: 출력 verdict JSON. stdlib only.
import argparse, json, sys


def _load(p, label):
    try:
        with open(p, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        sys.stderr.write("[verdict] ERROR %s: %s\n" % (label, e))
        sys.exit(2)


def main():
    ap = argparse.ArgumentParser(description="결정론 PASS/REFUTE 게이트")
    ap.add_argument("--measured", required=True, help="parse_bench.py JSON")
    ap.add_argument("--roofline", required=True, help="roofline.py JSON")
    ap.add_argument("--reference-tps", type=float, help="E: 외부 현실-달성치(검증기 (b) 외부검색 산물)")
    ap.add_argument("--target-tps", type=float, help="c: 사용자 선언 목표(백스톱)")
    ap.add_argument("--tolerance", type=float, default=0.15, help="PASS 허용오차(기본 15%)")
    ap.add_argument("--spec-supported", action="store_true", help="모델이 speculative(MTP) 지원 — off 면 강제함수")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    m = _load(args.measured, "measured")
    r = _load(args.roofline, "roofline")
    tol = args.tolerance

    M = m.get("decode_tps")
    spec_on = bool(m.get("spec_on"))
    accept_M = m.get("accept_len")
    R_fp = r.get("R_fp")
    R_token = r.get("R_token")
    expected = r.get("expected_achievable")

    reasons, refuted_claims, notes = [], [], []

    if not m.get("measurement_ok") or M is None:
        print(json.dumps({
            "verdict": "INVALID", "failure_axis": None, "structural_or_strategy": None,
            "reason": "측정 실패(completed=%s failed=%s decode_tps=%s) — 재측정 필요" % (
                m.get("completed"), m.get("failed"), M),
            "measured_decode_tps": M, "rubric": None,
        }, ensure_ascii=False, indent=2))
        return

    # --- 루브릭 primary 선택 (3중 방어막 우선순위) ---
    if args.reference_tps is not None:
        primary, primary_src = args.reference_tps, "E(external_reference)"
    elif args.target_tps is not None:
        primary, primary_src = args.target_tps, "c(user_target)"
    elif expected is not None:
        primary, primary_src = expected, "expected_achievable(roofline×MBU)"
    else:
        primary, primary_src = None, None

    # --- establish 실패 → 사용자 백스톱 ---
    if primary is None:
        print(json.dumps({
            "verdict": "NEEDS_RUBRIC", "failure_axis": "establish",
            "structural_or_strategy": None,
            "reason": "루브릭 못 세움: E(외부검색) 부재 ∧ c(사용자) 부재 ∧ expected 산출 불가 → (c) 사용자 백스톱 필요",
            "measured_decode_tps": M, "rubric": None,
            "ask_user": "동일 HW(%s, tp=%s)에서 이 모델의 정상 디코드 t/s 레퍼런스를 제공해 주세요." % (
                r.get("gpu_model"), r.get("tp")),
        }, ensure_ascii=False, indent=2))
        return

    floor = primary * (1.0 - tol)
    passed = M >= floor
    ratio = round(M / primary, 3) if primary else None

    # --- spec-off 강제함수: 모델이 MTP 지원하는데 off 면, R_token 기대 대비 미달을 명시 ---
    spec_hint = None
    if args.spec_supported and not spec_on and R_token and R_fp and R_token > R_fp * 1.1:
        spec_hint = "speculative(MTP) OFF 감지(accept_len=%s). 지원 모델인데 미사용 → R_token(%s) 대비 단일패스 R_fp(%s)에 묶임. enable 권장." % (
            accept_M, R_token, R_fp)
        notes.append(spec_hint)

    # --- structural vs strategy 힌트(최종은 LLM+HITL — 여기선 결정론 힌트) ---
    if passed:
        verdict, axis, sos = "PASS", "meet", None
        reasons.append("M(%s) ≥ %s×(1−%.2f)=%.2f [%s] → 충족" % (M, primary, tol, floor, primary_src))
    else:
        verdict, axis = "REFUTE", "meet"
        reasons.append("M(%s) < %s×(1−%.2f)=%.2f [%s] (ratio %s) → 기대 이하" % (M, primary, tol, floor, primary_src, ratio))
        refuted_claims.append({
            "claim": "서빙이 충분히 빠르다(production/agent-ready)",
            "reason": "단일스트림 디코드 %s t/s 가 루브릭 %s t/s(%s)의 %s배 — floor %.2f 미달" % (M, primary, primary_src, ratio, floor),
            "evidence": {"measured": m.get("decode_tps"), "engine_cross": m.get("engine_gen_throughput_max"),
                         "R_fp": R_fp, "R_token": R_token, "expected": expected},
        })
        if spec_hint:
            sos = "strategy"
            notes.append("처방 후보: --speculative-config(MTP) 활성(recipe 재탐색).")
        elif expected and M < expected * (1.0 - tol) and spec_on:
            sos = "structural?"
            notes.append("spec on 인데도 expected(%s) 한참 아래 → 커널/이미지/백엔드 구조 의심(escalation 후보 — 외부 확증 필요)." % expected)
        else:
            sos = "strategy"
            notes.append("처방 후보: recipe serve-config 재탐색(백엔드·플래그·KV).")

    out = {
        "verdict": verdict,
        "failure_axis": axis,
        "structural_or_strategy": sos if verdict == "REFUTE" else None,
        "measured_decode_tps": M,
        "measured_spec_on": spec_on,
        "measured_accept_len": accept_M,
        "rubric": {"primary": primary, "source": primary_src, "floor": round(floor, 2),
                   "tolerance": tol, "ratio_M_over_primary": ratio,
                   "R_fp": R_fp, "R_token": R_token, "expected_achievable": expected,
                   "reference_E": args.reference_tps, "target_c": args.target_tps},
        "reasons": reasons,
        "refuted_claims": refuted_claims,
        "diagnosis_hint": notes,
        "note": "게이트=결정론(이 규칙). LLM Devil's Advocate 는 E 외부검색·정성 진단·재탐색힌트만 보탠다(최종 structural/strategy = LLM+HITL).",
    }
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
