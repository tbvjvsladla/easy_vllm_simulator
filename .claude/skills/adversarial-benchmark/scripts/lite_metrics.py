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

lite 는 **inform-only** — 이 스크립트는 PASS/FAIL 을 판정하지 않는다(verdict_rule.py 미투입 · NG-6).

single = 5행 표(메트릭|값). multi = 병합 표(용량 3종 열=Main|Sub per-node · 속도·cold TTFT=마스터 1행).
"""
import argparse
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
    r"kv[_ ]cache[_ ]memory[_ ]?bytes[=: ]+([\d,]+)",          # 절대 바이트
    r"[Aa]vailable KV cache memory[:= ]+([\d.]+)\s*GiB",
    r"reserved for KV [Cc]ache[:= ]+([\d.]+)\s*GiB",
    r"GPU KV cache (?:size|memory)[:= ]+([\d.]+)\s*GiB",
    r"KV cache[^\n]*?([\d.]+)\s*GiB",
]
_WEIGHTS_PAT = r"[Mm]odel (?:weights take|loading took)[:= ]*([\d.]+)\s*GiB"
_NONTORCH_PAT = r"non[_-]?torch[^\n]*?([\d.]+)\s*GiB"


def _grep_first(text, patterns):
    for pat in patterns:
        m = re.search(pat, text)
        if m:
            return m.group(1)
    return None


def parse_engine_log(path):
    """→ {kv_gib, weights_gib, nontorch_gib, reserved_total_gib}. 부재는 null(fail-soft)."""
    out = {"kv_gib": None, "weights_gib": None, "nontorch_gib": None, "reserved_total_gib": None}
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
    topo = raw.get("topology", "single")
    warm = _load_json(raw.get("bench_warm_json"))
    cold = _load_json(raw.get("bench_cold_json"))
    engine = parse_engine_log(raw.get("engine_log"))
    gen, gen_src = gen_tps_from_bench(warm)
    ttft = cold_ttft_ms(cold)
    nodes = raw.get("nodes", [])
    by_role = {n.get("role", "main"): n for n in nodes}

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


def main():
    ap = argparse.ArgumentParser(description="lite 5종 메트릭 결정론 파서/렌더러 (inform-only)")
    ap.add_argument("--raw-json", required=True, help="lite_bench.sh 산출 raw readings JSON")
    ap.add_argument("--json", action="store_true", help="구조화 JSON 출력(표 대신)")
    args = ap.parse_args()
    raw = _load_json(args.raw_json)
    if raw is None:
        print(f"[lite_metrics] raw JSON 로드 실패: {args.raw_json}", file=sys.stderr)
        return 2
    res = build(raw)
    if args.json:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        print(res["table"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
