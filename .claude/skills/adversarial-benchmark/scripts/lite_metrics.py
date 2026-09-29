#!/usr/bin/env python3
"""lite_metrics.py — 경량(lite) 벤치 5종 메트릭 결정론 파서/렌더러 (adversarial-benchmark §5.5)

lite 모드(inform-only·기본 ON)의 **결정론 계층**: lite_bench.sh 가 수집한 raw 값
(warm/cold bench JSON · engine log · per-node nvidia-smi/proc·meminfo readings)을 받아
5종 메트릭을 산정하고 채팅용 표로 렌더한다. **확률론 산정 금지**(헌법 §금지 — 결정론 스크립트).

5종 메트릭(plan_26071115 D12):
  속도 2종: gen tokens/sec(warm, =1000/median_tpot_ms — adversarial §5 정본) · cold-start TTFT(별도 1줄)
  용량 3종: GPU VRAM 점유(GiB+%) · KV cache 점유(GiB+%) · 시스템 RAM 점유(GiB+%)

정직성(음성정직): 통합메모리(GB10 등)는 nvidia-smi 메모리가 N/A → serve 로그 VRAM 분해로 폴백,
그것도 없으면 값 없이 "N/A(source)" 로 표기(대체값 날조 ✗). engine log KV 라인은 vLLM 버전 따라
변할 수 있어 fail-soft(부재 시 null).

lite 는 성능 PASS/FAIL 을 판정하지 않는다(verdict_rule.py 미투입 · NG-6) — 대신 **성립 판정**(lite_verdict)을
소유한다: lite 는 full 의 첫 단계이자 진입 게이트다(judge_lite · 2026-09-29).

single = 5행 표(메트릭|값). multi = 병합 표(용량 3종 열=Main|Sub per-node · 속도·cold TTFT=마스터 1행).
"""
import argparse
import os
import json
import re
import sys

GIB = 1024.0 ** 3


# ── bench JSON(vllm bench serve --save-result) 파싱 ──────────────────────────
def _load_json(path):
    if not path:
        return None
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def gen_tps_from_bench(bench):
    """단일스트림 디코드 정본 = 1000/median_tpot_ms (adversarial §5). 폴백=output_throughput."""
    if not bench:
        return None, None
    tpot = bench.get("median_tpot_ms")
    if isinstance(tpot, (int, float)) and tpot > 0:
        return round(1000.0 / tpot, 2), "median_tpot"
    ot = bench.get("output_throughput")
    if isinstance(ot, (int, float)) and ot > 0:
        return round(float(ot), 2), "output_throughput"
    return None, None


def cold_ttft_ms(bench_cold):
    """cold(무-warmup 단일요청) TTFT. median_ttft_ms 우선, ttfts[0] 폴백."""
    if not bench_cold:
        return None
    v = bench_cold.get("median_ttft_ms") or bench_cold.get("mean_ttft_ms")
    if isinstance(v, (int, float)) and v > 0:
        return round(float(v), 1)
    arr = bench_cold.get("ttfts")
    if isinstance(arr, list) and arr:
        return round(float(arr[0]), 1)
    return None


# ── engine log 에서 KV cache + VRAM 분해(fail-soft regex) ────────────────────
_KV_PATTERNS = [
    # 절대 바이트. non-default args 는 파이썬 dict repr 이라 키가 따옴표에 싸여 나온다
    # ("'kv_cache_memory_bytes': 21474836480") — 종전 `bytes[=: ]+` 는 그 따옴표에서 끊겨
    # **절대 클램프를 명시했는데도** 매칭에 실패했다.
    r"kv[_ ]cache[_ ]memory[_ ]?bytes['\"]?\s*[=:]\s*([\d,]+)",
    r"[Aa]vailable KV cache memory[:= ]+([\d.]+)\s*GiB",
    r"reserved for KV [Cc]ache[:= ]+([\d.]+)\s*GiB",
    # vLLM 0.26.x 어형: "Initial free memory 113.47 GiB, reserved 20.0 GiB memory for KV Cache".
    # 수치가 "for KV Cache" **앞**에 오고 Cache 가 대문자라 위 두 패턴이 둘 다 빗나간다.
    r"reserved\s+([\d.]+)\s*GiB\s+memory\s+for\s+KV\s+[Cc]ache",
    r"GPU KV cache (?:size|memory)[:= ]+([\d.]+)\s*GiB",
    r"KV [Cc]ache[^\n]*?([\d.]+)\s*GiB",
]
# vLLM 0.26.x 는 KV 크기를 **토큰**으로 보고한다("GPU KV cache size: 154,192 tokens").
# GiB 가 아니므로 위 패턴들과 별개 축이며, 모델 간 비교에는 이쪽이 더 이식적이다.
_KV_TOKENS_PAT = r"GPU KV cache size[:= ]+([\d,]+)\s*tokens"
_WEIGHTS_PAT = r"[Mm]odel (?:weights take|loading took)[:= ]*([\d.]+)\s*GiB"
_NONTORCH_PAT = r"non[_-]?torch[^\n]*?([\d.]+)\s*GiB"


def _grep_first(text, patterns):
    for pat in patterns:
        m = re.search(pat, text)
        if m:
            return m.group(1)
    return None


def parse_engine_log(path):
    """→ {kv_gib, kv_tokens, weights_gib, nontorch_gib, reserved_total_gib}. 부재는 null(fail-soft)."""
    out = {"kv_gib": None, "kv_tokens": None, "weights_gib": None,
           "nontorch_gib": None, "reserved_total_gib": None}
    if not path:
        return out
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            text = f.read()
    except OSError:
        return out
    raw = _grep_first(text, _KV_PATTERNS)
    if raw is not None:
        raw = raw.replace(",", "")
        try:
            val = float(raw)
            # 바이트(정수·큰 값)면 GiB 로 환산, 이미 GiB 면 그대로.
            out["kv_gib"] = round(val / GIB, 2) if val > 1e6 else round(val, 2)
        except ValueError:
            pass
    m = re.search(_KV_TOKENS_PAT, text)
    if m:
        try:
            out["kv_tokens"] = int(m.group(1).replace(",", ""))
        except ValueError:
            pass
    for key, pat in (("weights_gib", _WEIGHTS_PAT), ("nontorch_gib", _NONTORCH_PAT)):
        m = re.search(pat, text)
        if m:
            try:
                out[key] = round(float(m.group(1)), 2)
            except ValueError:
                pass
    parts = [out[k] for k in ("weights_gib", "nontorch_gib", "kv_gib") if out[k] is not None]
    if parts:
        out["reserved_total_gib"] = round(sum(parts), 2)
    return out


# ── per-node 용량 산정(nvidia-smi > serve-log 폴백 > N/A 음성정직) ────────────
def _num(v):
    try:
        f = float(v)
        return f
    except (TypeError, ValueError):
        return None


def node_capacity(node, engine, is_master):
    """node readings(dict) + engine 분해 → {gpu, ram, kv} 표시 문자열(음성정직)."""
    # GPU VRAM: nvidia-smi(discrete) > serve-log reserved(통합메모리 폴백) > N/A
    used = _num(node.get("gpu_smi_used_mib"))
    total = _num(node.get("gpu_smi_total_mib"))
    if used is not None and total and total > 0:
        gpu = f"{used/1024:.1f} GiB ({100*used/total:.0f}%)"
    elif is_master and engine.get("reserved_total_gib") is not None:
        gpu = f"~{engine['reserved_total_gib']:.1f} GiB (serve-log 분해; nvidia-smi N/A)"
    else:
        gpu = "N/A (통합메모리 nvidia-smi 미보고 · serve-log 분해 없음)"
    # 시스템 RAM: /proc/meminfo(항상 가용)
    rtot = _num(node.get("ram_total_kib"))
    ravail = _num(node.get("ram_avail_kib"))
    if rtot and rtot > 0 and ravail is not None:
        rused = rtot - ravail
        ram = f"{rused/1024/1024:.1f} GiB ({100*rused/rtot:.0f}%)"
    else:
        ram = "N/A"
    return {"gpu": gpu, "ram": ram}


# ── 렌더 ─────────────────────────────────────────────────────────────────────
def render_single(gen, gen_src, ttft, cap, kv_gib, gpu_total_gib):
    kv = f"{kv_gib:.1f} GiB" if kv_gib is not None else "N/A (serve 로그 KV 라인 부재 — fail-soft)"
    if kv_gib is not None and gpu_total_gib:
        kv += f" ({100*kv_gib/gpu_total_gib:.0f}%)"
    rows = [
        ("gen tokens/sec (warm)", f"{gen:.2f} t/s" if gen is not None else "N/A"),
        ("cold-start TTFT", f"{ttft:.0f} ms" if ttft is not None else "N/A"),
        ("GPU VRAM 점유", cap["gpu"]),
        ("KV cache 점유", kv),
        ("시스템 RAM 점유", cap["ram"]),
    ]
    out = ["| 메트릭 | 값 |", "|---|---|"]
    out += [f"| {k} | {v} |" for k, v in rows]
    return "\n".join(out)


def render_multi(gen, ttft, caps, kv_gib):
    # caps: {"main": {...}, "sub": {...}}. 속도/cold=마스터 1행, 용량 3종=per-node 열.
    main, sub = caps.get("main", {}), caps.get("sub")
    subcol = (lambda k: sub.get(k) if sub else "— (probe 실패)")
    kvmain = f"{kv_gib:.1f} GiB (클러스터)" if kv_gib is not None else "N/A (fail-soft)"
    rows = [
        ("gen tokens/sec (warm) [master]", f"{gen:.2f} t/s" if gen is not None else "N/A", "—"),
        ("cold-start TTFT [master]", f"{ttft:.0f} ms" if ttft is not None else "N/A", "—"),
        ("GPU VRAM 점유", main.get("gpu", "N/A"), subcol("gpu")),
        ("KV cache 점유", kvmain, "≈ ÷TP (클러스터 공유)"),
        ("시스템 RAM 점유", main.get("ram", "N/A"), subcol("ram")),
    ]
    out = ["| 메트릭 | Main | Sub |", "|---|---|---|"]
    out += [f"| {k} | {m} | {s} |" for k, m, s in rows]
    return "\n".join(out)


def build(raw):
    # 2026-09-05(G-B8): 기본값 "single" 삭제. **토폴로지는 인터뷰/manifest 만이 정한다** —
    #   추론도 기본값도 헌법이 금지한다(브랜치로도 추론하지 않는다). 기본값이 있으면 멀티 산출물이
    #   조용히 single 로 기록되고, 그 라벨은 인증서의 강한 키로 흘러간다.
    topo = raw.get("topology")
    if topo not in ("single", "multi"):
        raise SystemExit("[lite_metrics] FAIL: topology 가 선언되지 않았다(%r) — 기본값을 쓰지 않는다. "
                         "호출부가 manifest/인터뷰에서 읽어 넘겨라." % (topo,))
    warm = _load_json(raw.get("bench_warm_json"))
    cold = _load_json(raw.get("bench_cold_json"))
    engine = parse_engine_log(raw.get("engine_log"))
    gen, gen_src = gen_tps_from_bench(warm)
    ttft = cold_ttft_ms(cold)
    nodes = raw.get("nodes", [])
    # 역할 기본값 "main" 도 같은 이유로 삭제 — 역할 미상 노드를 main 으로 적으면 서브 측정이
    #   메인 열에 들어가 두 노드의 수치가 뒤바뀐다(조용한 오배치).
    for _n in nodes:
        if _n.get("role") not in ("main", "sub"):
            raise SystemExit("[lite_metrics] FAIL: nodes[] 에 role 이 없다(%r) — 추측하지 않는다."
                             % (_n.get("role"),))
    by_role = {n["role"]: n for n in nodes}

    def total_gib(node):
        t = _num(node.get("gpu_smi_total_mib"))
        return (t / 1024.0) if t else None

    result = {"topology": topo, "gen_tps": gen, "gen_src": gen_src, "cold_ttft_ms": ttft,
              "kv_gib": engine.get("kv_gib"), "engine": engine}
    if topo == "multi":
        caps = {}
        for role in ("main", "sub"):
            if role in by_role:
                node = by_role[role]
                # 서브 SSH probe 실패는 통합메모리 N/A 와 **다른 원인** — 명시 마커로 표기(음성정직 · D22).
                if role == "sub" and node.get("probe_ok") is False:
                    caps[role] = {"gpu": "— (SSH probe 실패)", "ram": "— (SSH probe 실패)"}
                else:
                    caps[role] = node_capacity(node, engine, is_master=(role == "main"))
        result["capacity"] = caps
        result["table"] = render_multi(gen, ttft, caps, engine.get("kv_gib"))
    else:
        node = by_role.get("main", nodes[0] if nodes else {})
        cap = node_capacity(node, engine, is_master=True)
        result["capacity"] = {"main": cap}
        result["table"] = render_single(gen, gen_src, ttft, cap, engine.get("kv_gib"), total_gib(node))
    return result


# ── lite 판정(2026-09-29 · plan_26092923 · 인터뷰 interview_20260929_132122) ──────────────────────
# lite 는 full 의 **첫 단계이자 진입 게이트**다(lite ⊂ full). 판정은 성립만 묻는다 — 임계값이 없다.
#   pass                    = cold 1건 + warm N건 전부 성공(failed 0) · 토큰 실제 생성 · 메인 5지표에 N/A 없음
#   measurement_path_failed = 요청이 서버에 닿지 못했거나(클라이언트 평면 미도달) 측정기가 값을 못 냈다 — 하네스 결함
#                             (재빌드 ✗ · cap 차감 ✗ · 하네스 수리로)
#   server_failed           = 서버가 응답했지만 실패(5xx·빈 출력·타임아웃)했거나 측정 중 죽었다 — 실사용 불가
#                             (upstream·explorer 재발동 · cap −1 — 이번 범위는 신호까지)
# **단일 권위는 raw 의 `lite_verdict`** 이고 종료코드는 여기서 파생한다(LITE_EXIT). 판정 함수는 이것 하나이며
#   lite_bench.sh 가 유일한 호출부다(세 진입 경로 α·β·γ 공통 — 행동만 맥락별로 갈린다).
LITE_VERDICTS = ("pass", "measurement_path_failed", "server_failed")
LITE_EXIT = {"pass": 0, "measurement_path_failed": 6, "server_failed": 7}
# vllm bench serve 가 결과 파일 없이 끝났을 때 stderr 에서 가르는 두 모양(실물 문구 — 픽스처가 같은 문구를 쓴다).
#   초기 시험 요청이 **서버 응답으로** 실패하면 "Initial test run failed" 이고, 연결 자체가 안 되면 그 뒤에
#   aiohttp 연결 오류가 붙는다. 연결 오류는 클라이언트 평면 도달 실패이므로 서버 실패로 접지 않는다.
_INITIAL_TEST_FAILED = "Initial test run failed"
_CONNECT_ERROR_RE = re.compile(r"Cannot connect to host|Connect call failed|Connection refused|ClientConnectorError",
                               re.I)


def _requests_ok(bench, expected):
    """bench JSON 한 건이 '전부 성공 · 토큰 생성' 인가. → (ok, 사유 또는 None)."""
    if not isinstance(bench, dict):
        return False, "결과 파일 없음"
    completed, failed = bench.get("completed"), bench.get("failed")
    out_tok = bench.get("total_output_tokens")
    if not isinstance(completed, int) or completed != expected:
        return False, "completed=%r ≠ 요청 %d" % (completed, expected)
    if failed not in (0, None):
        return False, "failed=%r" % (failed,)
    if not (isinstance(out_tok, (int, float)) and out_tok > 0):
        return False, "total_output_tokens=%r(생성 없음)" % (out_tok,)
    return True, None


def judge_lite(raw, built):
    """raw(lite_bench.sh 수집) + build() 결과 → {lite_verdict, lite_exit, lite_verdict_reasons[], lite_verdict_source}.

    raw 가 드는 판정 입력(lite_bench.sh 가 적는다):
      burst_n · bench_cold_json · bench_warm_json · bench_cold_rc · bench_warm_rc · bench_stderr_tail
      host_health_after(측정 뒤 호스트 평면 /health HTTP 코드) · client_plane_health(클라이언트가 도는 평면에서 본 코드)
    입력이 빠지면 판정하지 못한 것이다 — 통과로 접지 않고 measurement_path_failed(측정기가 판정 입력을 못 냈다)다."""
    reasons = []
    burst_n = raw.get("burst_n")
    if not isinstance(burst_n, int) or burst_n < 1:
        reasons.append("판정 입력 burst_n 부재·무효(%r)" % (burst_n,))
        return _verdict("measurement_path_failed", reasons)
    cold = _load_json(raw.get("bench_cold_json"))
    warm = _load_json(raw.get("bench_warm_json"))
    ok_c, why_c = _requests_ok(cold, 1)
    ok_w, why_w = _requests_ok(warm, burst_n)
    if ok_c and ok_w:
        cap = ((built.get("capacity") or {}).get("main") or {})
        na = [k for k, v in (("gen_tps", built.get("gen_tps")), ("cold_ttft_ms", built.get("cold_ttft_ms")),
                             ("kv_gib", built.get("kv_gib"))) if v is None]
        na += [k for k in ("gpu", "ram") if str(cap.get(k) or "N/A").startswith(("N/A", "—"))]
        if na:
            reasons.append("요청은 전부 성공했지만 메인 지표 N/A: %s — 측정기가 값을 못 냈다" % ",".join(na))
            return _verdict("measurement_path_failed", reasons)
        return _verdict("pass", ["cold 1/1 · warm %d/%d 성공 · 토큰 생성 · 메인 5지표 실측" % (burst_n, burst_n)])
    for label, why in (("cold", why_c), ("warm", why_w)):
        if why:
            reasons.append("%s: %s" % (label, why))
    host = str(raw.get("host_health_after") or "")
    client = str(raw.get("client_plane_health") or "")
    reasons.append("측정 뒤 health — 호스트 평면 %s · 클라이언트 평면 %s" % (host or "미관측", client or "미관측"))
    if host != "200":
        reasons.append("서버가 측정 뒤 health 에 응답하지 않는다 — 측정 중 서버 실패")
        return _verdict("server_failed", reasons)
    if client != "200":
        reasons.append("서버는 살아 있는데 클라이언트가 도는 평면에서 닿지 않는다 — 측정 경로 불성립(하네스)")
        return _verdict("measurement_path_failed", reasons)
    # 서버에 닿는다. 결과 파일이 있는데 실패 요청·빈 출력이면 서버가 실패로 응답한 것이다.
    tail = str(raw.get("bench_stderr_tail") or "")
    have_result = isinstance(cold, dict) or isinstance(warm, dict)
    if have_result:
        reasons.append("서버에 닿았고 결과 파일이 실패 요청·빈 출력을 말한다 — 서버 응답 실패")
        return _verdict("server_failed", reasons)
    if _INITIAL_TEST_FAILED in tail and not _CONNECT_ERROR_RE.search(tail):
        reasons.append("결과 파일 없음 · 초기 시험 요청이 서버 응답으로 실패(%r) — 서버 응답 실패" % _INITIAL_TEST_FAILED)
        return _verdict("server_failed", reasons)
    reasons.append("결과 파일 없음 · 서버 응답 실패의 증거도 없다 — 클라이언트(측정기) 실패로 읽는다")
    return _verdict("measurement_path_failed", reasons)


def _verdict(v, reasons):
    return {"lite_verdict": v, "lite_exit": LITE_EXIT[v], "lite_verdict_reasons": reasons,
            "lite_verdict_source": "lite_metrics.judge_lite(raw)"}


def read_lite_verdict(raw):
    """호출부(sweep_bench·인증서 발행기·게이트)가 판정을 읽는 **유일한** 규칙. raw 가 dict 가 아니거나 lite_verdict 가
    어휘 밖이면 None — 호출부는 None 을 불통과로 읽는다(부재를 통과로 접지 않는다)."""
    if not isinstance(raw, dict):
        return None
    v = raw.get("lite_verdict")
    return v if v in LITE_VERDICTS else None


def main():
    ap = argparse.ArgumentParser(description="lite 5종 메트릭 결정론 파서/렌더러 + lite 판정")
    ap.add_argument("--raw-json", required=True, help="lite_bench.sh 산출 raw readings JSON")
    ap.add_argument("--json", action="store_true", help="구조화 JSON 출력(표 대신)")
    ap.add_argument("--judge", action="store_true",
                    help="판정해 raw 에 lite_verdict·lite_exit·사유를 **제자리 기록**하고 판정 줄을 출력한다(호출부 lite_bench.sh)")
    args = ap.parse_args()
    raw = _load_json(args.raw_json)
    if raw is None:
        print(f"[lite_metrics] raw JSON 로드 실패: {args.raw_json}", file=sys.stderr)
        return 2
    res = build(raw)
    if args.judge:
        v = judge_lite(raw, res)
        raw.update(v)
        tmp = args.raw_json + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(raw, f, ensure_ascii=False, indent=1)
        os.replace(tmp, args.raw_json)
        print(res["table"])
        print("")
        print("lite_verdict: %s (exit %d)" % (v["lite_verdict"], v["lite_exit"]))
        for r in v["lite_verdict_reasons"]:
            print("  - %s" % r)
        return 0
    if args.json:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        print(res["table"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
