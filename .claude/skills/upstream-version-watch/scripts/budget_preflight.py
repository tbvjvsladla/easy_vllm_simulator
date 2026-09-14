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
# ★ `--declared-gmu`(2026-09-14 · plan_26091407 §4.3 · 사용자 결정 Q2·Q9)은 **기재 전용**이다. 서빙 yaml 의
#   gpu-memory-utilization 을 받아 `expected_vllm_share = gmu × MemTotal` 과 `residual = share − weights − kv` 를
#   provenance=declared 로 **병기**할 뿐, arm 산식·declare_ok·bench_ok·종료코드에는 들어가지 않는다(게이트 ✗).
#   왜 게이트가 아닌가: vLLM 소스상 kv-cache-memory-bytes 를 주면 gmu 가 관여하는 자리는 기동 전 `free ≥ ceil(total×gmu)`
#   검사이고 총량 캡을 거는 코드는 없다(`gpu_worker.determine_available_memory`·`utils.request_memory`). 그런데 0.85→0.80 이
#   5,562MiB 를 연 관측(총량 cap 처럼 작용)은 사실이라 기전이 미확정이다 — **기재가 먼저, 게이트는 실측이 쌓인 뒤**다
#   (plan_26091407 F5·R4).
#
# 사용: budget_preflight.py --mem-total-mib N --ckpt-mib N --kv-mib N --overhead-mib N --tp N
#         [--ple-mib N] [--ple-mmap] [--bench-budget-mib N] [--declared-gmu G] [--json]
#       budget_preflight.py --self-test
# 종료: 0=통과 · 4=선언 불가(로드 차단) · 5=벤치 진입 불가 · 2=인자 오류
import argparse
import json
import os
import re
import sys

# ★ 노드 도구는 **소유자 정본 → 서브 런타임 배달분** 순으로 찾는다(2026-09-11 서브 라이브 교정).
#   종전에는 `.claude/skills/terraforming_node/scripts/node_blackbox/` 하나만 들었는데, 그 스킬은
#   서브에 **배달되지 않는다**(`tool_plane` 이 정한다) — 서브는 `.claude/runtime/node_blackbox/` 로
#   받는다. 메인에는 정본이 실재하므로 결함이 보이지 않고 **서브에서만** 부재가 된다. 이 저장소가
#   이미 겪은 형태다(결함 ⑧: node_id 해소기가 서브에서 부재였고, 하필 그것이 서브 무보호 로드를
#   닫으려던 장치였다). 두-후보 패턴의 단일 소유는 `run_trial._node_tool_path` 이고 여기는 같은 규약을 따른다.
_REPO = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))
_ETA_CANDIDATES = (
    os.path.join(_REPO, ".claude", "skills", "terraforming_node", "scripts", "node_blackbox"),
    os.path.join(_REPO, ".claude", "runtime", "node_blackbox"),
)


def constants():
    """선언 상수의 정본. 여기에 사본을 두지 않는다."""
    for _d in _ETA_CANDIDATES:
        if os.path.isfile(os.path.join(_d, "blackbox_eta.py")):
            if _d not in sys.path:
                sys.path.insert(0, _d)
            break
    try:
        from blackbox_eta import DEFAULTS as D  # noqa: E402
    except ImportError as exc:
        raise SystemExit(
            "[budget-preflight] FAIL: 선언 상수를 blackbox_eta.DEFAULTS 에서 읽지 못했다(%s).\n"
            "  찾은 자리: %s\n"
            "  여기에 사본을 두지 않는다 — 거울은 정본이 움직일 때 조용히 옛값으로 남는다."
            % (exc, " · ".join(_ETA_CANDIDATES)))
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


def declared_gmu_row(declared_gmu, mem_total_mib=None, weights_mib=None, kv_mib=None):
    """`--declared-gmu` 기재 행. 순수 함수 — **판정에 쓰지 않는다**(호출부는 이 행을 출력에 덧붙이기만 한다).

    status: recorded(산출) · invalid(0 < gmu ≤ 1 밖 — 멈추지 않고 사실로 적는다) · not_applicable(벤치 진입
    경로 — weights·kv 입력이 없어 몫·잔차를 물을 수 없다). 멈추지 않는 이유: 이 행이 종료코드를 바꾸면
    기재가 게이트로 격상되고, 호출부(서빙 스모크)에 실패 경로가 새로 생긴다.
    """
    row = {"provenance": "declared", "gmu": float(declared_gmu), "status": "recorded",
           "mem_total_mib": None, "expected_vllm_share_mib": None, "weights_mib": None, "kv_mib": None,
           "residual_mib": None,
           "note": ("기재 전용(게이트 ✗) — kv 클램프 시 vLLM 은 gmu 를 기동 전 free ≥ ceil(total×gmu) 검사에만 쓴다"
                    "(총량 캡 코드는 소스에 없고 캡처럼 작용한 관측만 있다 · 기전 미확정 · plan_26091407 F5). "
                    "residual = 예상 몫 − weights − kv")}
    if not (0.0 < float(declared_gmu) <= 1.0):
        row.update(status="invalid", note="gmu 는 (0, 1] 이어야 한다 — 산출하지 않고 사실만 적는다(기재 · 차단 ✗)")
        return row
    if mem_total_mib is None or weights_mib is None or kv_mib is None:
        row.update(status="not_applicable",
                   note="벤치 진입 경로(선언된 바닥 사용)는 weights·kv 입력이 없다 — 몫·잔차를 묻지 않는다")
        return row
    share = int(float(declared_gmu) * int(mem_total_mib))
    row.update(mem_total_mib=int(mem_total_mib), expected_vllm_share_mib=share, weights_mib=int(weights_mib),
               kv_mib=int(kv_mib), residual_mib=share - int(weights_mib) - int(kv_mib))
    return row


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

    # ★ 서브 이식성 앵커(2026-09-11 서브 라이브가 잡았다): 노드 도구는 메인 정본과 **서브 런타임
    #   배달분** 두 자리에 있을 수 있다. 한 자리만 들면 메인에서는 초록이고 서브에서만 죽는다 —
    #   그리고 그 죽음은 하필 서브에서 예산 게이트를 닫으려던 장치에서 난다(결함 ⑧과 같은 형태).
    ck("★서브 이식성: 노드 도구를 정본·런타임 두 자리에서 찾는다(한 자리만 들면 서브에서 죽는다)",
       len(_ETA_CANDIDATES) == 2
       and _ETA_CANDIDATES[0].endswith(os.path.join("terraforming_node", "scripts", "node_blackbox"))
       and _ETA_CANDIDATES[1].endswith(os.path.join(".claude", "runtime", "node_blackbox")))

    # ── --declared-gmu 기재 행 (2026-09-14 · plan_26091407 §4.3 · §7 O3) ─────────────────────────
    #   두 가지를 친다: ① 기재 행이 실제 CLI 출력에 선다 ② **인자 유무로 기존 출력이 바이트 단위로 같다**
    #   (arm 산식·기존 필드·종료코드). ②가 깨지면 기재가 판정을 흔든 것이다. CLI 를 그대로 돌린다 —
    #   함수만 부르면 main() 의 출력 조립(키 순서·텍스트 줄)이 증명되지 않는다.
    import subprocess as _sp

    def _cli(*argv):
        p = _sp.run([sys.executable, os.path.abspath(__file__), *argv], capture_output=True, text=True,
                    timeout=60)
        return p.returncode, p.stdout

    w_res, _ = resident_weights_mib(CKPT["nv4"], TP, PLE, False)
    for label, w_case in (("통과 구성 nv4+res", w_res), ("차단 구성 fp8+res", CKPT["fp8"] // TP)):
        base = ("--mem-total-mib", str(MEM), "--weights-mib", str(w_case), "--kv-mib", str(KV),
                "--overhead-mib", str(OH))
        rc0, j0 = _cli("--json", *base)
        rc1, j1 = _cli("--json", *base, "--declared-gmu", "0.85")
        d0, d1 = json.loads(j0), json.loads(j1)
        row = d1.get("declared_gmu_row") or {}
        ck("★기재 %s: 기재 행이 선다(provenance=declared · 예상 몫 = gmu×MemTotal · 잔차 = 몫−weights−kv)" % label,
           row.get("provenance") == "declared" and row.get("status") == "recorded"
           and row.get("expected_vllm_share_mib") == int(0.85 * MEM)
           and row.get("residual_mib") == int(0.85 * MEM) - w_case - KV, "→ %s" % row)
        ck("★기재 %s: 인자 유무로 종료코드·기존 JSON 필드가 바이트 동일(arm 산식 불변 · 게이트 ✗)" % label,
           rc0 == rc1 and "declared_gmu_row" not in d0
           and json.dumps({k: v for k, v in d1.items() if k != "declared_gmu_row"}, ensure_ascii=False)
           == j0.strip(), "→ rc %s/%s" % (rc0, rc1))
        rc2, t0 = _cli(*base)
        rc3, t1 = _cli(*base, "--declared-gmu", "0.85")
        ck("★기재 %s: 텍스트 출력은 기존 바이트가 그대로 앞에 서고 기재 줄 1행만 뒤에 붙는다" % label,
           rc2 == rc3 and t1.startswith(t0) and t1[len(t0):].count("\n") == 1
           and "declared-gmu" in t1[len(t0):], "→ tail=%r" % t1[len(t0):])
    rc4, j4 = _cli("--json", "--mem-total-mib", str(MEM), "--weights-mib", str(w_res), "--kv-mib", str(KV),
                   "--overhead-mib", str(OH), "--declared-gmu", "1.5")
    ck("★기재 음성대조: 범위 밖 gmu 는 멈추지 않고 invalid 로 적는다(종료코드 불변 — 실패 경로 신설 ✗)",
       rc4 == 0 and (json.loads(j4).get("declared_gmu_row") or {}).get("status") == "invalid", "→ rc=%s" % rc4)
    rc5, j5 = _cli("--json", "--floor-mib", "30000", "--bench-budget-mib", "8192", "--declared-gmu", "0.85")
    ck("★기재: 벤치 진입 경로(바닥 선언)에서는 not_applicable 로 적는다(몫·잔차를 지어내지 않는다)",
       rc5 == 0 and (json.loads(j5).get("declared_gmu_row") or {}).get("status") == "not_applicable",
       "→ rc=%s %s" % (rc5, j5))
    rc6, t6 = _cli("--floor-mib", "30000", "--bench-budget-mib", "8192", "--declared-gmu", "0.85")
    ck("★기재 텍스트: 산출하지 않은 행(not_applicable)은 None 산술을 찍지 않고 사유만 싣는다",
       rc6 == 0 and "status=not_applicable" in t6 and "None" not in t6, "→ %r" % t6)
    if os.path.isfile(_mn):
        body = open(_mn, encoding="utf-8").read()
        # 배선: 호출부가 **함수 산출**을 인자로 넘기고 기재 줄을 찍는다(코드 토큰).
        ck("★기재 배선 앵커: 멀티 선판정 호출부가 서빙 yaml gmu 를 --declared-gmu 로 넘긴다",
           '_DECL_GMU="$(_yaml_declared_gmu "output/multi/configs/${CONFIG}.yaml")"' in body
           and '_DECL_GMU_ARGS=(--declared-gmu "$_DECL_GMU")' in body and "declared_gmu_row" in body)
        # 추출: 문자열 앵커는 정규식·경로가 깨져도 초록이다 — 함수 **본문을 그대로** bash 로 실행해 픽스처로 친다.
        _fn = re.search(r"^_yaml_declared_gmu\(\)\{.*?^\}$", body, re.MULTILINE | re.DOTALL)
        ck("★기재 추출 함수가 스모크에 실재한다(_yaml_declared_gmu)", _fn is not None)
        if _fn is not None:
            import tempfile as _tf
            cases = (
                ("수치", "model: x\ngpu-memory-utilization: 0.85\nmax-model-len: 8192\n", "0.85"),
                ("따옴표+주석", "gpu-memory-utilization: \"0.8\"   # 손 편집\n", "0.8"),
                ("밑줄 키", "gpu_memory_utilization: 0.9\n", "0.9"),
                ("주석 줄만(생성기 설명 주석)", "# gpu-memory-utilization = startup free-memory 게이트\n", ""),
                ("키 부재", "model: x\nmax-model-len: 8192\n", ""),
                ("비수치", "gpu-memory-utilization: auto\n", ""),
                ("점 두 개", "gpu-memory-utilization: 0.8.5\n", ""),
                ("첫 줄만(중복 키)", "gpu-memory-utilization: 0.7\ngpu-memory-utilization: 0.9\n", "0.7"),
            )
            with _tf.TemporaryDirectory(prefix="pf_yaml_gmu_") as td:
                for label, text, want in cases:
                    fx = os.path.join(td, "c.yaml")
                    with open(fx, "w", encoding="utf-8") as fh:
                        fh.write(text)
                    p = _sp.run(["bash", "-c", 'set -uo pipefail\n%s\n_yaml_declared_gmu "$1"' % _fn.group(0),
                                 "_", fx], capture_output=True, text=True, timeout=30)
                    ck("★기재 추출(%s) → %r" % (label, want), p.returncode == 0 and p.stdout == want,
                       "→ rc=%s out=%r err=%r" % (p.returncode, p.stdout, p.stderr[-200:]))
                p = _sp.run(["bash", "-c", 'set -uo pipefail\n%s\n_yaml_declared_gmu "$1"' % _fn.group(0),
                             "_", os.path.join(td, "absent.yaml")], capture_output=True, text=True, timeout=30)
                ck("★기재 추출(파일 부재) → 빈 문자열 · 실패 경로 없음", p.returncode == 0 and p.stdout == "",
                   "→ rc=%s out=%r" % (p.returncode, p.stdout))

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
    row = out.get("declared_gmu_row")
    if row is not None:   # 기재 행은 **맨 뒤 한 줄** — 인자가 없을 때의 출력 바이트를 그대로 둔다
        if row["status"] == "recorded":
            print("  ⓘ declared-gmu 기재(게이트 ✗ · provenance=%s · status=%s): gmu=%s × MemTotal %sMiB = 예상 vLLM 몫 "
                  "%sMiB · 잔차(몫 − weights − kv) %sMiB"
                  % (row["provenance"], row["status"], row["gmu"], row["mem_total_mib"],
                     row["expected_vllm_share_mib"], row["residual_mib"]))
        else:   # 산출하지 않은 행에 None 산술을 찍지 않는다 — 사유 문장만 싣는다(리뷰 교정 2026-09-14)
            print("  ⓘ declared-gmu 기재(게이트 ✗ · provenance=%s · status=%s): gmu=%s — %s"
                  % (row["provenance"], row["status"], row["gmu"], row["note"]))


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
    ap.add_argument("--declared-gmu", type=float, default=None,
                    help="서빙 yaml 의 gpu-memory-utilization — 예상 vLLM 몫·잔차를 **기재만** 한다(게이트 ✗)")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        sys.exit(_self_test())
    if a.floor_mib is not None:
        # 벤치 진입 경로 — 바닥은 선언에서 온다. 나머지 입력은 요구하지 않는다.
        out = preflight(0, 0, 0, 0, a.bench_budget_mib, floor_mib=a.floor_mib)
        out["weights_mib"] = None
        out["ple_applied_mib"] = None
        if a.declared_gmu is not None:
            out["declared_gmu_row"] = declared_gmu_row(a.declared_gmu)
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
    if a.declared_gmu is not None:
        # 판정(out 의 기존 필드·종료코드)이 확정된 **뒤에** 덧붙인다 — 기재가 판정에 닿을 순서가 없다.
        out["declared_gmu_row"] = declared_gmu_row(a.declared_gmu, a.mem_total_mib, weights, a.kv_mib)
    _emit(a, out)
    if not out["declare_ok"]:
        sys.exit(4)
    if out["bench_ok"] is False:
        sys.exit(5)
    sys.exit(0)


if __name__ == "__main__":
    main()
