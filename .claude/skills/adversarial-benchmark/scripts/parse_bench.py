#!/usr/bin/env python3
# parse_bench.py — `vllm bench serve` 결과 JSON + (선택) engine-log throughput 교차검증 파싱 (결정론)
#
# 측정 M 산출(adversarial-benchmark §7). 핵심: 단일스트림(--max-concurrency 1) 디코드 t/s =
#   1000 / median_tpot_ms  (TPOT=Time Per Output Token, 1st 토큰 제외 → warm). output_throughput 은
#   콜드 JIT 첫요청 TTFT 에 끌려 과소 → median_tpot 가 정본 디코드 지표. engine-log 는 교차검증.
# CONTRACT: 출력 키는 verdict_rule.py 가 소비(고정). stdlib only.
import argparse, json, re, sys

# recipe parse_vllm_log 와 동일 관례: engine 측 steady throughput.
_GEN_TPS_RE = re.compile(r"generation throughput:\s*([\d,]+(?:\.\d+)?)\s*tokens/s", re.IGNORECASE)


def _f(d, k):
    v = d.get(k)
    try:
        return float(v) if v is not None else None
    except (TypeError, ValueError):
        return None


def main():
    ap = argparse.ArgumentParser(description="vllm bench serve 결과 + engine-log 파싱 → 측정 M")
    ap.add_argument("--bench-json", required=True, help="vllm bench serve --save-result JSON 경로")
    ap.add_argument("--engine-log", help="docker logs 캡처 파일(교차검증, 선택)")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    try:
        with open(args.bench_json, "r", encoding="utf-8") as f:
            d = json.load(f)
    except Exception as e:
        sys.stderr.write("[parse_bench] ERROR bench-json: %s\n" % e)
        sys.exit(2)

    median_tpot = _f(d, "median_tpot_ms")
    mean_tpot = _f(d, "mean_tpot_ms")
    decode_tps = round(1000.0 / median_tpot, 2) if median_tpot and median_tpot > 0 else None
    decode_tps_mean = round(1000.0 / mean_tpot, 2) if mean_tpot and mean_tpot > 0 else None
    accept_len = _f(d, "spec_decode_acceptance_length")
    completed = int(d.get("completed") or 0)
    failed = int(d.get("failed") or 0)
    max_conc = d.get("max_concurrency")

    # engine-log 교차검증(steady = 관측 최대값).
    engine_max = None
    if args.engine_log:
        try:
            with open(args.engine_log, "r", encoding="utf-8", errors="replace") as f:
                vals = [float(m.replace(",", "")) for m in _GEN_TPS_RE.findall(f.read())]
            if vals:
                engine_max = round(max(vals), 2)
        except Exception as e:
            sys.stderr.write("[parse_bench] WARN engine-log: %s\n" % e)

    agreement = None
    if engine_max and decode_tps:
        # 단일스트림이면 engine throughput ≈ decode_tps(±). 큰 괴리 = warmup/콜드 의심.
        ratio = engine_max / decode_tps if decode_tps else None
        agreement = {"engine_max": engine_max, "client_decode_tps": decode_tps,
                     "ratio": round(ratio, 2) if ratio else None}

    out = {
        "decode_tps": decode_tps,                 # ★ 단일스트림 warm 디코드(=1000/median_tpot). verdict 의 M.
        "decode_tps_mean": decode_tps_mean,
        "output_throughput": _f(d, "output_throughput"),       # 집계(콜드에 끌릴 수 있음 — warmup 폐기 권장)
        "total_token_throughput": _f(d, "total_token_throughput"),
        "ttft_ms_median": _f(d, "median_ttft_ms"),
        "ttft_ms_mean": _f(d, "mean_ttft_ms"),
        "itl_ms_median": _f(d, "median_itl_ms"),
        "tpot_ms_median": median_tpot,
        "accept_len": accept_len,                 # spec on 판정(>1.05) + R_token 비교용
        "spec_on": bool(accept_len and accept_len > 1.05),
        "completed": completed,
        "failed": failed,
        "max_concurrency": max_conc,
        "num_prompts": d.get("num_prompts"),
        "engine_gen_throughput_max": engine_max,
        "client_engine_agreement": agreement,
        "measurement_ok": completed > 0 and failed == 0 and decode_tps is not None,
    }
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
