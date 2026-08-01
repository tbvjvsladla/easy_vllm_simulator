#!/usr/bin/env python3
"""blackbox_eta.py -- ETA 산출 엔진 + 포락선 갱신 (plan_26073109 §2.2).

임계를 절대 GiB 가 아니라 **ETA(0 까지 남은 시간)** 로 정의한다. 근거(testlog_26073109):
하강률의 동적 범위가 p50 0.13 ~ max 6,241 MiB/s = **약 48,000배** 라, 단일 절대 임계로는
한쪽에서 늦고 한쪽에서 오발하는 것이 원리적으로 불가피하다.

산식 (사용자 승인 2026-07-31 -- kill 지연 중 추가 소비 항 포함):

    eta_zero_s       = mem_avail_mib / rate_mib_s          # 현 하강률로 0 도달까지
    eta_actionable_s = eta_zero_s - kill_latency_s         # **킬 완료 시점** 기준 실여유
    TRIP  <=>  eta_actionable_s <= detect_margin_s

  kill 지연 항이 필요한 이유: testlog_26073109 §4-3 실측에서 MemAvail 최저값(main 7,472 MiB)이
  임계(10,240)보다 **낮았다** -- 트립 이후 kill 이 완료되기까지 메모리는 계속 떨어진다.
  검출 시점 잔량이 아니라 킬 완료 시점 잔량을 기준으로 여유를 잡아야 한다.

역할 분리(헌법 결정론 기조):
  - 이 스크립트 = **오프라인·주기 실행**. 로그를 읽어 포락선을 갱신하고 상수를 emit 한다.
  - 1 초 핫루프(mem_watchdog.sh) = emit 된 상수를 source 해 **정수 산술만** 수행(파이썬 비의존).
  학습(추론)은 여기서, 실행(판정)은 저기서 -- 섞지 않는다.

종료코드: 0=성공 · 1=인자/입력 오류 · 2=자체시험 실패.
"""
from __future__ import annotations

import argparse
import json
import os
import sys

SCHEMA_VERSION = 1

# 하한 가드: 물리적으로 불가능한 설정을 사람도 에이전트도 넣지 못하게 막는 결정론 바닥.
# 근거: kill 지연보다 짧은 여유를 요구하는 임계는 "발동해도 늦는" 설정이다.
ETA_FLOOR_MULTIPLIER = 1.5

DEFAULTS = {
    "kill_latency_s": 4.0,      # testlog_26073109 관측 상한 하단(3~15s, HB 15s 격자로 과대) -- 실측 대체 대상
    "detect_margin_s": 2.0,     # 폴링 간격(1s) + 여유(1s)
    "debounce_polls": 3.0,      # 연속 N 폴 지속해야 실제 TRIP (사용자 승인 2026-07-31, 옵션 '가')
    "agent_act_s": 300.0,       # 에이전트 개입 구간 (ETA 5분)
    "agent_notify_s": 900.0,    # 에이전트 알림 구간 (ETA 15분)
    "min_rate_mib_s": 1.0,      # 이보다 느린 하강은 '정지'로 간주(0 나눗셈·잡음 방지)
    "poll_interval_s": 1.0,
    # ── 선언된 바닥(testlog_26073123 · 2026-08-01) ────────────────────────
    # ETA 규칙은 `잔량÷하강률` 선형 외삽이라 **유계**인 모델 로드 하강을 무계로 읽어
    # 58 GiB 급 모델을 3/3 사살했다. 서빙이 예상 바닥을 선언하면 규칙은 그 아래에서만 무장한다.
    # 두 상수는 워치독에도 같은 기본값이 있으나 **정본은 여기**다 — 나머지 상수와 같은
    # 파이프라인(emit_params)으로 조정 가능해야 캠페인 루프튜닝의 대상이 된다.
    "decl_margin_mib": 8192,      # arm 상한 = 선언바닥 - 이 값. 선언 오차·정상 변동 흡수분
    "decl_min_ceiling_mib": 16384,  # arm 상한이 이 밑이 되는 선언은 거부(게이트 실명 방지)
    # ★ 최후 절대 바닥. min_rate_mib_s 가 만든 구멍을 막는다 -- 0.5 MiB/s 로 천천히 새면
    #   ETA 는 영원히 'green'(rate_below_min)이라 절대 트립하지 않는다. 그 상태로 0 에 도달하면
    #   호스트가 죽는다. 따라서 "느리든 빠르든 이 밑이면 죽인다"는 무조건 바닥이 필요하다.
    #   5 GiB 로 잡은 근거: 실측 MemAvail 최저가 main 7,472 / sub 8,840 MiB 였고(그때 호스트는
    #   살았다), 7/22 오발 지점(10,186)보다는 충분히 낮아 그 오발을 되살리지 않는다.
    "hard_floor_mib": 5120.0,
}

# ⚠ 이 값들은 **초기 임의값**이다(plan_26073109 §2.2). Plan B 7종 E2E 캠페인이 모델별 실측으로
#   교체한다 -- 여기 박힌 숫자를 확정 사실로 인용하지 말 것.


def runway_s(p):
    """TRIP 판정에 쓸 총 활주로.

    디바운스는 **kill 시작을 (N-1)x폴링 만큼 늦춘다** -- 그 지연을 활주로에 더하지 않으면
    디바운스를 켠 만큼 실효 여유가 줄어든다(늦은 킬). 그래서 활주로에 포함한다.
    """
    return (float(p["kill_latency_s"]) + float(p["detect_margin_s"])
            + (float(p["debounce_polls"]) - 1.0) * float(p["poll_interval_s"]))


def compute_eta(mem_avail_mib, rate_mib_s, params=None):
    """순수 함수. rate_mib_s 는 **양수 = 하강**(소비 속도). 반환 dict.

    rate 가 min_rate 미만이면 하강이 아니라고 보고 ETA=None(무한) 으로 음성정직 표기한다."""
    p = dict(DEFAULTS)
    if params:
        p.update({k: v for k, v in params.items() if k in DEFAULTS})
    if mem_avail_mib is None or mem_avail_mib < 0:
        raise ValueError("mem_avail_mib must be >= 0")
    # ★ 최후 바닥 -- 하강률과 **무관하게** 발동. 느린 누수(rate < min_rate)로 0 에 도달하는
    #   경로를 ETA 규칙만으로는 막을 수 없기 때문이다(구조적 구멍의 백스톱).
    if mem_avail_mib <= p["hard_floor_mib"]:
        return {"eta_zero_s": (round(mem_avail_mib / float(rate_mib_s), 3)
                               if rate_mib_s and rate_mib_s >= p["min_rate_mib_s"] else None),
                "eta_actionable_s": None, "band": "trip", "trip": True,
                "reason": "hard_floor"}
    if rate_mib_s is None or rate_mib_s < p["min_rate_mib_s"]:
        return {"eta_zero_s": None, "eta_actionable_s": None, "band": "green",
                "trip": False, "reason": "rate_below_min"}
    eta_zero = mem_avail_mib / float(rate_mib_s)
    # 실여유 = 0 도달까지 - (kill 지연 + 디바운스 지연). detect_margin 과 비교한다.
    eta_act = eta_zero - (runway_s(p) - p["detect_margin_s"])
    trip = eta_act <= p["detect_margin_s"]
    if trip:
        band = "trip"
    elif eta_act <= p["agent_act_s"]:
        band = "red"
    elif eta_act <= p["agent_notify_s"]:
        band = "amber"
    else:
        band = "green"
    return {"eta_zero_s": round(eta_zero, 3), "eta_actionable_s": round(eta_act, 3),
            "band": band, "trip": trip, "reason": "ok"}


def trip_threshold_mib(rate_mib_s, params=None):
    """주어진 하강률에서 TRIP 이 걸리는 MemAvailable 값(설명·검증용 역산)."""
    p = dict(DEFAULTS)
    if params:
        p.update({k: v for k, v in params.items() if k in DEFAULTS})
    if rate_mib_s is None or rate_mib_s < p["min_rate_mib_s"]:
        return None
    return round(rate_mib_s * runway_s(p), 1)


def validate_params(params):
    """하한 가드. 위반 시 (False, [사유...]) -- 조용한 보정 금지, 거부한다."""
    errs = []
    p = dict(DEFAULTS)
    p.update({k: v for k, v in (params or {}).items() if k in DEFAULTS})
    kl = float(p["kill_latency_s"])
    dm = float(p["detect_margin_s"])
    db = float(p["debounce_polls"])
    floor = kl * ETA_FLOOR_MULTIPLIER
    runway = runway_s(p)
    if kl <= 0:
        errs.append("kill_latency_s must be > 0")
    if dm <= 0:
        errs.append("detect_margin_s must be > 0")
    if db < 1 or db != int(db):
        errs.append("debounce_polls must be an integer >= 1 (got %r)" % db)
    if float(p["poll_interval_s"]) <= 0:
        errs.append("poll_interval_s must be > 0")
    if runway < floor:
        errs.append("runway(kill_latency+detect_margin)=%.2fs < floor(kill_latency*%.1f)=%.2fs"
                    % (runway, ETA_FLOOR_MULTIPLIER, floor))
    # 선언된 바닥 가드: 여유가 음수면 선언이 상한을 **올려** 규칙을 더 민감하게 만들고,
    # 최소상한이 절대바닥 이하면 "거부"가 아무것도 거부하지 못한다(가드가 가드가 아니게 된다).
    if float(p["decl_margin_mib"]) < 0:
        errs.append("decl_margin_mib must be >= 0")
    if float(p["decl_min_ceiling_mib"]) <= float(p["hard_floor_mib"]):
        errs.append("decl_min_ceiling_mib(%s) must exceed hard_floor_mib(%s) — 최소상한이 절대바닥 "
                    "이하면 어떤 선언도 거부되지 않아 가드가 무력해진다"
                    % (p["decl_min_ceiling_mib"], p["hard_floor_mib"]))
    for k in ("agent_act_s", "agent_notify_s"):
        if float(params.get(k, DEFAULTS[k])) <= runway:
            errs.append("%s must exceed runway %.2fs (에이전트 구간이 데몬 구간보다 안쪽일 수 없음)" % (k, runway))
    if float(params.get("agent_notify_s", DEFAULTS["agent_notify_s"])) <= \
       float(params.get("agent_act_s", DEFAULTS["agent_act_s"])):
        errs.append("agent_notify_s must exceed agent_act_s")
    return (not errs), errs


def emit_shell_params(params, path):
    """1초 핫루프가 source 할 셸 상수. 부동소수 나눗셈을 피하려고 **밀리초 정수**도 함께 낸다."""
    p = dict(DEFAULTS)
    p.update({k: v for k, v in (params or {}).items() if k in DEFAULTS})
    lines = [
        "# generated by blackbox_eta.py -- 편집 금지(재생성으로 갱신)",
        "# 산식: eta_actionable = mem_avail/rate - kill_latency ; TRIP <=> eta_actionable <= detect_margin",
        "# 핫루프는 등가 형태를 정수로 쓴다: TRIP <=> mem_avail_mib*1000 <= rate_mib_s*RUNWAY_MS",
    ]
    runway_ms = int(round(runway_s(p) * 1000))
    for k, v in sorted(p.items()):
        lines.append("BB_%s=%s" % (k.upper(), v))
    lines.append("BB_RUNWAY_MS=%d" % runway_ms)
    lines.append("BB_DEBOUNCE_N=%d" % int(p["debounce_polls"]))
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, path)
    return runway_ms


def load_envelope(node_dir):
    path = os.path.join(node_dir, "envelope.json")
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def _self_test():
    checks = []

    # 1) 정지 상태 -- 하강 아님
    r = compute_eta(50000, 0.0)
    checks.append(("정지 -> ETA None·green·무트립",
                   r["eta_zero_s"] is None and r["band"] == "green" and not r["trip"]))

    # 2) 실측 최대 하강률(6241 MiB/s)에서의 트립 지점 역산
    #    runway = kill 4s + detect 2s + 디바운스 (3-1)x1s = 8s -> 6241.07*8 = 49,928.6 MiB
    th = trip_threshold_mib(6241.07)
    checks.append(("6.1GiB/s 트립 지점 ~49.9GiB(디바운스 포함)", abs(th - 49928.6) < 1.0))
    # 2b) 디바운스를 끄면 활주로가 줄어 트립 지점도 낮아진다(등가성 확인)
    th1 = trip_threshold_mib(6241.07, {"debounce_polls": 1.0})
    checks.append(("디바운스 1폴 -> 활주로 6s -> ~37.4GiB", abs(th1 - 37446.4) < 1.0))
    checks.append(("디바운스↑ -> 트립 지점↑(지연분 보상)", th > th1))

    # 3) 완만 구간(44 MiB/s) -- 7/22 오발 재현 검사.
    #    당시 10,186 MiB 에서 kill 했으나 ETA 기준으로는 트립 대상이 아니어야 한다.
    r = compute_eta(10186, 44.0)
    checks.append(("44MiB/s·10.2GiB -> 무트립(오발 제거)", not r["trip"]))
    checks.append(("같은 지점 eta_zero ~231s", abs(r["eta_zero_s"] - 10186 / 44.0) < 0.01))

    # 4) kill 지연 항이 실제로 보수적으로 작동하는가 (동일 조건에서 지연을 늘리면 더 일찍 트립)
    slow = trip_threshold_mib(1000.0, {"kill_latency_s": 4.0})
    fast = trip_threshold_mib(1000.0, {"kill_latency_s": 10.0})
    checks.append(("kill 지연↑ -> 트립 지점↑(보수)", fast > slow))

    # 5) 급락에서 트립
    r = compute_eta(5000, 2000.0)
    checks.append(("2GiB/s·5GiB -> TRIP", r["trip"] and r["band"] == "trip"))

    # 6) 밴드 경계
    r = compute_eta(100000, 200.0)      # eta_zero=500s, actionable=496s -> red(<=300? no) -> amber
    checks.append(("500s -> amber", r["band"] == "amber"))
    r = compute_eta(100000, 500.0)      # eta_zero=200s, actionable=196s -> red
    checks.append(("196s -> red", r["band"] == "red"))
    r = compute_eta(100000, 50.0)       # eta_zero=2000s -> green
    checks.append(("2000s -> green", r["band"] == "green"))

    # 7) 하한 가드 -- 물리 불가 설정 거부
    ok, errs = validate_params({"kill_latency_s": 4.0, "detect_margin_s": 2.0})
    checks.append(("정상 파라미터 통과", ok))
    ok2, errs2 = validate_params({"kill_latency_s": 10.0, "detect_margin_s": 0.5})
    checks.append(("runway < floor 거부", (not ok2) and any("floor" in e for e in errs2)))
    ok3, _ = validate_params({"kill_latency_s": 4.0, "detect_margin_s": 2.0, "agent_act_s": 3.0})
    checks.append(("에이전트 구간이 데몬 안쪽이면 거부", not ok3))
    ok4, _ = validate_params({"agent_act_s": 900.0, "agent_notify_s": 300.0})
    checks.append(("notify <= act 거부", not ok4))
    ok5, errs5 = validate_params({"debounce_polls": 0.0})
    checks.append(("디바운스 0폴 거부", (not ok5) and any("debounce" in e for e in errs5)))
    ok6, errs6 = validate_params({"debounce_polls": 2.5})
    checks.append(("디바운스 비정수 거부", (not ok6) and any("debounce" in e for e in errs6)))

    # 9) 최후 절대 바닥 -- min_rate 구멍(느린 누수)의 백스톱
    r = compute_eta(4000, 0.2)      # 0.2 MiB/s = min_rate 미만 = ETA 규칙상 'green'
    checks.append(("느린 누수라도 바닥 밑이면 TRIP", r["trip"] and r["reason"] == "hard_floor"))
    r = compute_eta(6000, 0.2)      # 바닥 위 + 느린 하강 -> 트립 안 함(기존 동작 보존)
    checks.append(("바닥 위 느린 하강은 무트립", not r["trip"] and r["reason"] == "rate_below_min"))
    r = compute_eta(10186, 44.0)    # 7/22 오발 지점이 바닥에 걸리지 않아야 한다
    checks.append(("7/22 오발 지점(10,186)은 바닥 밖", not r["trip"]))
    r = compute_eta(5120, 0.0)      # 경계 포함(<=)
    checks.append(("바닥 경계값 포함", r["trip"] and r["reason"] == "hard_floor"))

    # 8) 셸 상수 emit -- 핫루프 등가식이 파이썬 판정과 일치하는가
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        p = os.path.join(td, "eta_params.env")
        runway_ms = emit_shell_params({}, p)
        txt = open(p, encoding="utf-8").read()
        # 핫루프(mem_watchdog.sh)가 쓰는 **완전한** 등가식: 최후 바닥 OR ETA 활주로.
        # 바닥 항을 빼면 느린 누수 케이스에서 셸과 파이썬이 갈린다 -- 그 갈림을 여기서 고정한다.
        floor = DEFAULTS["hard_floor_mib"]
        agree, disagreements = True, []
        for mem, rate in ((5000, 2000.0), (10186, 44.0), (37000, 6241.07), (120000, 1.0),
                          (4000, 0.2), (6000, 0.2), (5120, 0.0), (49000, 6241.07)):
            py = compute_eta(mem, rate)["trip"]
            sh = (mem <= floor) or ((mem * 1000) <= (rate * runway_ms))
            if py != sh:
                agree = False
                disagreements.append((mem, rate, py, sh))
        checks.append(("셸 등가식 == 파이썬 판정 (8 케이스)%s"
                       % ("" if agree else " 불일치=%r" % disagreements), agree))
        checks.append(("emit 파일에 RUNWAY_MS=8000", "BB_RUNWAY_MS=8000" in txt))
        checks.append(("emit 파일에 DEBOUNCE_N=3", "BB_DEBOUNCE_N=3" in txt))
        # 선언된 바닥 상수가 셸로 흘러가는가 — 워치독이 읽는 **정확한 변수명**이어야 한다.
        # 이름이 어긋나면 워치독은 조용히 자기 하드코딩 기본값으로 돌고, 여기서 조정한 값은 증발한다.
        checks.append(("emit 에 DECL_MARGIN_MIB=8192", "BB_DECL_MARGIN_MIB=8192\n" in txt))
        checks.append(("emit 에 DECL_MIN_CEILING_MIB=16384", "BB_DECL_MIN_CEILING_MIB=16384\n" in txt))
        # 정수로 나가야 한다 — 셸 산술은 정수 전용이고 '8192.0' 은 워치독 비교에서 터진다.
        checks.append(("선언 상수가 정수 표기",
                       "BB_DECL_MARGIN_MIB=8192.0" not in txt
                       and "BB_DECL_MIN_CEILING_MIB=16384.0" not in txt))
        # 가드가 가드로 작동하는가
        bad, _ = validate_params({"decl_min_ceiling_mib": 5120})   # == hard_floor
        checks.append(("최소상한 <= 절대바닥 거부", not bad))
        bad2, _ = validate_params({"decl_margin_mib": -1})
        checks.append(("음수 여유 거부", not bad2))

    ok_all = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok_all = ok_all and passed
    print("self-test: %s (%d 케이스)" % ("PASS" if ok_all else "FAIL", len(checks)))
    return 0 if ok_all else 2


def main(argv=None):
    ap = argparse.ArgumentParser(description="노드블랙박스 ETA 엔진")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    ap.add_argument("--emit-params", help="셸 상수 출력 경로 (eta_params.env)")
    ap.add_argument("--set", action="append", default=[], metavar="K=V",
                    help="파라미터 override (예: --set kill_latency_s=5)")
    ap.add_argument("--explain", nargs=2, type=float, metavar=("MEM_MIB", "RATE_MIB_S"),
                    help="주어진 잔량·하강률의 ETA/밴드/트립 판정을 설명")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()

    params = {}
    if args.node_dir:
        env = load_envelope(args.node_dir)
        params.update({k: v for k, v in (env.get("eta_params") or {}).items() if k in DEFAULTS})
    for kv in args.set:
        if "=" not in kv:
            ap.error("--set 는 K=V 형식: %r" % kv)
        k, v = kv.split("=", 1)
        if k not in DEFAULTS:
            ap.error("알 수 없는 파라미터 %r (가능: %s)" % (k, ", ".join(sorted(DEFAULTS))))
        params[k] = float(v)

    ok, errs = validate_params(params)
    if not ok:
        print("[eta] 하한 가드 위반 — 거부:", file=sys.stderr)
        for e in errs:
            print("  - " + e, file=sys.stderr)
        return 1

    if args.explain:
        mem, rate = args.explain
        r = compute_eta(mem, rate, params)
        eff = dict(DEFAULTS); eff.update(params)
        print(json.dumps({"input": {"mem_avail_mib": mem, "rate_mib_s": rate},
                          "params": eff, "result": r,
                          "trip_threshold_mib_at_this_rate": trip_threshold_mib(rate, params)},
                         ensure_ascii=False, indent=2))
        return 0

    if args.emit_params:
        runway_ms = emit_shell_params(params, args.emit_params)
        print("[eta] emit %s (runway=%dms)" % (args.emit_params, runway_ms))
        return 0

    ap.error("--explain / --emit-params / --self-test 중 하나가 필요합니다")


if __name__ == "__main__":
    sys.exit(main())
