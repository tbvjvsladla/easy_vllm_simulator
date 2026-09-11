#!/usr/bin/env python3
# budget_preflight.py — 서빙 예산 선판정의 **단일 소유자** (2026-09-11 · plan_26091108 R1·R2·R3)
#
# 왜 있는가. 같은 산술이 세 자리에서 필요했다:
#   ① `multinode_serve_smoke.sh` 의 선판정(arm 상한 < 가드 최소 → exit 4)
#   ② `single_serve_up.sh` — **선판정이 아예 없었다**. 선언을 쓰고 워치독이 15초 뒤 거절하면
#      그때서야 알았다(왜 막혔는지가 불투명하다 · 노드 비대칭).
#   ③ 벤치 진입 — GuideLLM 컨테이너 몫이 예산과 **끊겨 있었다**(R3).
#   세 곳에 각자 적으면 반드시 갈린다. 여기가 유일한 자리이고 셋은 호출부다.
#
# 상수는 **거울을 두지 않는다** — `blackbox_eta.DEFAULTS` 를 import 한다(2026-09-05 G-B3 의 교훈:
#   거울로 둔 대가는 이미 치렀다. 정본이 움직였을 때 셸의 사본은 따라가지 못해 옛값으로 남았다).
#
# 모델:
#   resident_weights = (ckpt − ple×[ple_mmap]) ÷ tp     ← R2. 파일 크기가 아니라 **상주할 바이트**
#   floor            = mem_total − weights − kv − overhead
#   arm_ceiling      = floor − decl_margin
#   선언 가능        ⟺ arm_ceiling ≥ decl_min_ceiling
#   벤치 진입 가능   ⟺ floor − bench_budget ≥ abs_band      ← R3. **위상이 다르다**(아래)
#
# ★ 벤치 몫은 serve overhead 에 **더하지 않는다**. 벤치 컨테이너는 로드 시점에 상주하지 않으므로
#   로드 게이트에 상주분으로 넣으면 그만큼 과보수적으로 틀려 뜰 수 있는 셀을 죽인다. 3+1+1 슬롯
#   판정과 같은 물음이다 — **"무엇을 고치나" 가 아니라 "언제 성립해야 하나"**.
#
# 사용: budget_preflight.py --mem-total-mib N --ckpt-mib N --kv-mib N --overhead-mib N --tp N
#         [--ple-mib N] [--ple-mmap] [--bench-budget-mib N] [--json]
#       budget_preflight.py --self-test
# 종료: 0=통과 · 4=선언 불가(로드 차단) · 5=벤치 진입 불가 · 2=인자 오류
import argparse
import json
import os
import sys

_ETA_DIR = os.path.normpath(os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..", "..", "terraforming_node", "scripts", "node_blackbox"))


def constants():
    """선언 상수의 정본. 여기에 사본을 두지 않는다."""
    if _ETA_DIR not in sys.path:
        sys.path.insert(0, _ETA_DIR)
    try:
        from blackbox_eta import DEFAULTS as D  # noqa: E402
    except ImportError as exc:
        raise SystemExit(
            "[budget-preflight] FAIL: 선언 상수를 blackbox_eta.DEFAULTS 에서 읽지 못했다(%s).\n"
            "  여기에 사본을 두지 않는다 — 거울은 정본이 움직일 때 조용히 옛값으로 남는다." % exc)
    return (int(D["decl_margin_mib"]), int(D["decl_min_ceiling_mib"]), int(D["abs_band_mib"]))


def resident_weights_mib(ckpt_mib, tp, ple_mib=None, ple_mmap=False):
    """이 노드에 **상주할** weight MiB.

    `ple_mmap` 이 참이고 `ple_mib` 가 실측됐을 때에만 덜어낸다 — 선언만 있고 크기를 모르면
    보정 0 이다(추측으로 메우면 게이트가 없는 여유를 있다고 말한다).
    """
    tp = max(1, int(tp))
    resident = int(ckpt_mib)
    applied = 0
    if ple_mmap and ple_mib is not None and int(ple_mib) > 0:
        applied = int(ple_mib)
        resident -= applied
    return resident // tp, applied


def preflight(mem_total_mib, weights_mib, kv_mib, overhead_mib, bench_budget_mib=0,
              floor_mib=None):
    """선언 가능성과 벤치 진입 가능성을 한 번에 판정한다. 순수 함수.

    `floor_mib` 를 주면 바닥을 **다시 계산하지 않고** 그 값을 쓴다 — 벤치 진입은 이미 선언된
    바닥 위에서 묻는 물음이고(로드는 끝났다), 그 시점에 weights/kv/overhead 를 다시 구할
    경로가 없다. 다시 구하게 하면 사람이 손으로 적게 되고 선언과 갈린다(renew 가 바닥을
    재산출하지 않는 것과 같은 이유).
    """
    margin, min_ceiling, abs_band = constants()
    if floor_mib is not None:
        floor = int(floor_mib)
    else:
        floor = int(mem_total_mib) - int(weights_mib) - int(kv_mib) - int(overhead_mib)
    ceiling = floor - margin
    overhead_max = int(mem_total_mib) - int(weights_mib) - int(kv_mib) - margin - min_ceiling
    declare_ok = ceiling >= min_ceiling
    bench_floor = floor - int(bench_budget_mib or 0)
    # 벤치 몫이 0 이면 이 물음은 성립하지 않는다(묻지 않은 것과 통과한 것을 가른다).
    bench_ok = None if not bench_budget_mib else (bench_floor >= abs_band)
    bench_budget_max = floor - abs_band
    reasons = []
    if not declare_ok:
        reasons.append(
            "선언 불가: arm 상한 %dMiB < 가드 최소 %dMiB (%dMiB 부족). 예상 바닥 %dMiB. "
            "선택지 — overhead 를 실측으로 줄인다(이 노드 상한 %dMiB) · KV 절대클램프를 낮춘다"
            "(현재 %dMiB) · --no-budget 으로 무보호를 감수한다(그 사실이 이벤트로 남는다)."
            % (ceiling, min_ceiling, min_ceiling - ceiling, floor, overhead_max, int(kv_mib)))
    if bench_ok is False:
        reasons.append(
            "벤치 진입 불가: 바닥 %dMiB − 벤치 %dMiB = %dMiB < 절대밴드 %dMiB. "
            "벤치 예산 상한은 이 구성에서 %dMiB 다 — 그보다 크면 벤치 컨테이너가 뜨는 순간 "
            "워치독 사정거리에 들어간다(camp-26090918 `fp8-bf-262k-mmp` 실측: 서빙은 성공하고 "
            "벤치가 죽었다)."
            % (floor, int(bench_budget_mib), bench_floor, abs_band, bench_budget_max))
    return {
        "floor_mib": floor,
        "arm_ceiling_mib": ceiling,
        "overhead_max_mib": overhead_max,
        "declare_ok": declare_ok,
        "bench_floor_mib": bench_floor if bench_budget_mib else None,
        "bench_ok": bench_ok,
        "bench_budget_max_mib": bench_budget_max,
        "decl_margin_mib": margin,
        "decl_min_ceiling_mib": min_ceiling,
        "abs_band_mib": abs_band,
        "reasons": reasons,
    }


def _self_test():
    failures = []

    def ck(name, cond, detail=""):
        print(("  [PASS] " if cond else "  [FAIL] ") + name + ("" if cond else " " + detail))
        if not cond:
            failures.append(name)

    # ── 역채점: camp-26090918 의 실측 4조합 (plan_26091108 §2.3·§7-3) ─────────────────────
    #   입력은 전부 관측값이다 — MemTotal 은 이 호스트, ckpt·ple 는 체크포인트 index 파생,
    #   kv 는 셀 트리플렛의 절대클램프, overhead 는 캠페인 선언값.
    MEM, KV, OH, TP = 124610, 20480, 24800, 2
    CKPT = {"nv4": 126533, "fp8": 176928}
    PLE = 48828
    observed = {  # 직전 캠페인이 실제로 관측한 것
        ("nv4", False): True,    # nv4+res — 서빙 성공(32.53 t/s · 인증서 발행)
        ("nv4", True): True,     # nv4+mmp — 서빙 성공(35.39 t/s · 인증서 발행)
        ("fp8", True): True,     # fp8+mmp — **서빙 성공**(MTP accept 58.3%) · 벤치만 죽었다
        ("fp8", False): False,   # fp8+res — 22분 로딩 후 워치독 사살
    }
    for (variant, mmap_on), should_pass in observed.items():
        w, applied = resident_weights_mib(CKPT[variant], TP, PLE, mmap_on)
        r = preflight(MEM, w, KV, OH)
        ck("★R2 역채점 %s+%s → %s (weights=%d floor=%d)"
           % (variant, "mmp" if mmap_on else "res",
              "통과" if should_pass else "차단", w, r["floor_mib"]),
           r["declare_ok"] is should_pass,
           "→ 판정=%s 실측=%s" % (r["declare_ok"], should_pass))

    # 원장 대조 — nv4+res 의 floor 는 당시 원장에 `floor_mib: 16064` 로 적혔다.
    w, _ = resident_weights_mib(CKPT["nv4"], TP, PLE, False)
    ck("★R2 원장 대조: nv4+res 의 floor 가 기록된 16064 와 1MiB 안에서 일치",
       abs(preflight(MEM, w, KV, OH)["floor_mib"] - 16064) <= 1,
       "→ %d" % preflight(MEM, w, KV, OH)["floor_mib"])

    # ── 음성대조: mmap 보정을 끄면 fp8+mmp 가 차단된다(= 교정 전 동작) ────────────────────
    w_off, applied_off = resident_weights_mib(CKPT["fp8"], TP, PLE, False)
    ck("★R2 음성대조: mmap 보정을 끄면 fp8+mmp 가 차단된다(교정 전 동작 재현)",
       preflight(MEM, w_off, KV, OH)["declare_ok"] is False and applied_off == 0)
    w_on, applied_on = resident_weights_mib(CKPT["fp8"], TP, PLE, True)
    ck("★R2 음성대조 짝: 보정을 켜면 같은 셀이 통과한다",
       preflight(MEM, w_on, KV, OH)["declare_ok"] is True and applied_on == PLE)
    ck("R2 선언만 있고 크기를 모르면 보정하지 않는다(추측 ✗)",
       resident_weights_mib(CKPT["fp8"], TP, None, True) == (CKPT["fp8"] // TP, 0))

    # ── R3 역채점: fp8+mmp 에 GuideLLM 8GiB 를 얹으면 벤치 진입이 막힌다 ──────────────────
    r = preflight(MEM, w_on, KV, OH, bench_budget_mib=8192)
    ck("★R3 역채점: fp8+mmp 바닥에서 벤치 8192MiB 는 절대밴드를 침식한다(벤치사망 실측과 부합)",
       r["bench_ok"] is False and r["bench_floor_mib"] < r["abs_band_mib"],
       "→ bench_floor=%s" % r["bench_floor_mib"])
    ck("★R3 그 구성의 벤치 예산 상한을 숫자로 말한다(막기만 하지 않는다)",
       r["bench_budget_max_mib"] == r["floor_mib"] - r["abs_band_mib"])
    ck("★R3 상한 이하의 벤치 몫은 통과한다(과잉차단 ✗)",
       preflight(MEM, w_on, KV, OH,
                 bench_budget_mib=r["bench_budget_max_mib"])["bench_ok"] is True)
    ck("★R3 벤치 몫 0 은 '묻지 않았다' 이지 '통과' 가 아니다",
       preflight(MEM, w_on, KV, OH)["bench_ok"] is None)
    # 여유가 넉넉한 nv4+mmp 는 같은 벤치 몫으로도 통과해야 한다(가드가 전부를 막으면 가드가 아니다).
    w_nv4m, _ = resident_weights_mib(CKPT["nv4"], TP, PLE, True)
    ck("★R3 음성대조: 여유가 큰 nv4+mmp 는 같은 8192MiB 로 통과한다",
       preflight(MEM, w_nv4m, KV, OH, bench_budget_mib=8192)["bench_ok"] is True)

    # ── R1 배선 앵커: 이 게이트를 **누가 부르는가**. 산술이 옳아도 호출부가 없으면 아무 일도
    #    일어나지 않는다(이 저장소가 반복해 겪은 결함 계열: "만든 것과 도는 것은 다르다").
    #    앵커는 **코드 토큰**만 본다 — 주석에 걸면 리팩터가 주석만 남기고 코드를 옮겨도 초록이다.
    _sdir = os.path.dirname(os.path.abspath(__file__))
    _mn = os.path.join(_sdir, "multinode_serve_smoke.sh")
    _sg = os.path.join(_sdir, "single_serve_up.sh")
    if os.path.isfile(_mn):
        body = open(_mn, encoding="utf-8").read()
        ck("★R1 멀티: 선판정이 워치독 arming 과 분리돼 있다(결합하면 --no-watchdog 이 가드를 끈다)",
           'if [ "$BUDGET" = "1" ]; then' in body
           and 'if [ "$BUDGET" = "1" ] && [ "$WATCHDOG" = "1" ]; then' not in body)
        ck("★R1 멀티: 선판정 호출부가 실재한다",
           'budget_preflight.py" --json' in body)
        ck("★R1 멀티: 선언 생략 경로가 원장에 남는다(탈출구가 조용하면 구멍이다)",
           "--reason=--no-watchdog" in body)
    else:
        print("  [SKIP] R1 멀티 배선 앵커 — multinode_serve_smoke.sh 부재")
    if os.path.isfile(_sg):
        body = open(_sg, encoding="utf-8").read()
        ck("★R1 싱글: 선판정 호출부가 실재한다(종전에는 선판정이 아예 없었다 — 노드 비대칭)",
           "budget_preflight.py" in body)
    else:
        print("  [SKIP] R1 싱글 배선 앵커 — single_serve_up.sh 부재")

    _rb = os.path.normpath(os.path.join(_sdir, "..", "..", "adversarial-benchmark",
                                        "scripts", "run_bench.sh"))
    if os.path.isfile(_rb):
        body = open(_rb, encoding="utf-8").read()
        ck("★R3 벤치 진입 게이트의 호출부가 실재한다(종전 배선 0건)",
           "--floor-mib" in body and "budget_preflight.py" in body)
        ck("★R3 벤치 도중 선언 만료를 막는다(만료되면 워치독이 옛 규칙으로 돌아간다)",
           "renew-budget" in body)
        ck("★R3 벤치 몫을 serve overhead 에 더하지 않는다(위상 오배정 금지)",
           "--overhead-mib" not in body)
    else:
        print("  [SKIP] R3 배선 앵커 — run_bench.sh 부재")

    m, mc, ab = constants()
    ck("상수를 정본에서 읽는다(거울 ✗)", (m, mc, ab) == (3072, 8192, 10240),
       "→ %s" % ((m, mc, ab),))

    print("[budget_preflight] %s" % ("PASS" if not failures else "FAIL (%d)" % len(failures)))
    return 0 if not failures else 1


def _emit(a, out):
    if a.json:
        print(json.dumps(out, ensure_ascii=False))
        return
    print("[budget-preflight] weights=%s floor=%dMiB arm_ceiling=%dMiB declare=%s bench=%s"
          % ("%dMiB(ple 보정 −%dMiB)" % (out["weights_mib"], out["ple_applied_mib"])
             if out.get("weights_mib") is not None else "(선언된 바닥 사용)",
             out["floor_mib"], out["arm_ceiling_mib"],
             "OK" if out["declare_ok"] else "BLOCK",
             "n/a" if out["bench_ok"] is None else ("OK" if out["bench_ok"] else "BLOCK")))
    for r in out["reasons"]:
        print("  - %s" % r)


def main():
    ap = argparse.ArgumentParser(description="서빙 예산 선판정 (plan_26091108 R1·R2·R3)")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--mem-total-mib", type=int)
    ap.add_argument("--ckpt-mib", type=int)
    ap.add_argument("--weights-mib", type=int, help="이미 파생된 상주 weights(ckpt 대신)")
    ap.add_argument("--ple-mib", type=int)
    ap.add_argument("--ple-mmap", action="store_true")
    ap.add_argument("--tp", type=int, default=1)
    ap.add_argument("--kv-mib", type=int)
    ap.add_argument("--overhead-mib", type=int)
    ap.add_argument("--bench-budget-mib", type=int, default=0)
    ap.add_argument("--floor-mib", type=int,
                    help="이미 선언된 바닥(벤치 진입 게이트용 — 바닥을 재산출하지 않는다)")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        sys.exit(_self_test())
    if a.floor_mib is not None:
        # 벤치 진입 경로 — 바닥은 선언에서 온다. 나머지 입력은 요구하지 않는다.
        out = preflight(0, 0, 0, 0, a.bench_budget_mib, floor_mib=a.floor_mib)
        out["weights_mib"] = None
        out["ple_applied_mib"] = None
        _emit(a, out)
        sys.exit(0 if out["bench_ok"] is not False else 5)
    for name in ("mem_total_mib", "kv_mib", "overhead_mib"):
        if getattr(a, name) is None:
            print("[budget-preflight] FAIL: --%s 는 필수다(기본값을 두지 않는다)"
                  % name.replace("_", "-"), file=sys.stderr)
            sys.exit(2)
    if a.weights_mib is not None:
        weights, applied = a.weights_mib, 0
    elif a.ckpt_mib is not None:
        weights, applied = resident_weights_mib(a.ckpt_mib, a.tp, a.ple_mib, a.ple_mmap)
    else:
        print("[budget-preflight] FAIL: --ckpt-mib 또는 --weights-mib 가 필요하다", file=sys.stderr)
        sys.exit(2)
    out = preflight(a.mem_total_mib, weights, a.kv_mib, a.overhead_mib, a.bench_budget_mib)
    out["weights_mib"] = weights
    out["ple_applied_mib"] = applied
    _emit(a, out)
    if not out["declare_ok"]:
        sys.exit(4)
    if out["bench_ok"] is False:
        sys.exit(5)
    sys.exit(0)


if __name__ == "__main__":
    main()
