#!/usr/bin/env python3
"""rank_recipes.py — 하드게이트 + Judge 랭킹 + 리포트.

vllm-recipe-explorer 스킬의 결정론 랭킹 단계.
(quant x max-model-len x gpu-mem-util) 3축 후보를 estimate_vram.estimate 로 VRAM 추정 →
예산 x 안전마진 하드게이트로 통과/탈락 분리 → 통과분을 (headroom desc, max_model_len desc)
정렬해 리포트 표로 제시한다.

CONTRACT(FROZEN): docs/plan/plan_2026060819_1_whichllm_vLLM레시피탐색스킬_설계계획.md.
- 결정론 스크립트 책임(게이트/랭킹). LLM 후보 '생성'은 별도 런타임 단계.
- importable 함수 + __main__ CLI 둘 다 제공. estimate_vram.estimate 를 import 해 조립.
"""
import argparse
import json
import os
import sys

# scripts/ 를 import 경로에 넣어 형제 모듈(estimate_vram) 을 가져온다.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from estimate_vram import estimate  # noqa: E402  (estimate(parsed, candidate, tp, budget_gib, safety_margin, kv_cache_dtype_bytes=2) -> dict)


def auto_candidates(parsed, tp):
    """결정론 기본 그리드 후보 생성 (CONTRACT §9.1 규칙).

    - quant 집합: prequantized → [quant_method_native]; 아니면 ['none', 'fp8'].
    - max_model_len: 4096 부터 2의 거듭제곱으로 min(max_position_embeddings, 131072) 까지.
      max_position 없으면 [4096, 8192, 16384, 32768].
    - gpu_memory_utilization: [0.85, 0.90, 0.95].
    - 데카르트 곱, id = r1..rN.
    """
    if parsed.get("prequantized"):
        quant_set = [parsed.get("quant_method_native")]
    else:
        quant_set = ["none", "fp8"]

    max_position = parsed.get("max_position_embeddings")
    if max_position:
        cap = min(int(max_position), 131072)
        max_lens = []
        ml = 4096
        while ml <= cap:
            max_lens.append(ml)
            ml *= 2
        # max_position < 4096 인 극단 케이스 보호: 최소 한 개 후보는 둔다.
        if not max_lens:
            max_lens = [cap]
    else:
        max_lens = [4096, 8192, 16384, 32768]

    gmus = [0.85, 0.90, 0.95]

    candidates = []
    n = 0
    for quant in quant_set:
        for max_len in max_lens:
            for gmu in gmus:
                n += 1
                candidates.append({
                    "id": "r%d" % n,
                    "quantization": quant,
                    "max_model_len": max_len,
                    "gpu_memory_utilization": gmu,
                })
    return candidates


def rank(parsed, candidates, tp, budget_gib, margin, kv_bytes=2):
    """각 후보 estimate → gate_pass 분리 → 통과분 (headroom desc, max_model_len desc) 정렬.

    반환: {ranked:[...pass], failed:[...], all:[...]}. id 보존.
    """
    all_results = []
    for cand in candidates:
        result = estimate(parsed, cand, tp, budget_gib, margin, kv_bytes)
        # id 보존: 후보가 id 를 가졌는데 estimate 가 누락했으면 복원.
        if "id" not in result and "id" in cand:
            result["id"] = cand["id"]
        all_results.append(result)

    ranked = [r for r in all_results if r.get("gate_pass")]
    failed = [r for r in all_results if not r.get("gate_pass")]

    ranked.sort(
        key=lambda r: (r.get("headroom_gib", float("-inf")), r.get("max_model_len", 0)),
        reverse=True,
    )

    return {"ranked": ranked, "failed": failed, "all": all_results}


def _fmt_num(value, ndigits=2):
    if value is None:
        return "-"
    try:
        return "%.*f" % (ndigits, float(value))
    except (TypeError, ValueError):
        return str(value)


def render_report(result, parsed, budget_gib, margin, tp):
    """랭킹 결과를 표 + 헤더 텍스트로 렌더링.

    표 컬럼: id, quant, max_len, gmu, est_vram_gib, headroom_gib, PASS/FAIL.
    헤더: 모델 id, num_params, budget, margin, tp, 경고.
    """
    lines = []
    lines.append("=" * 72)
    lines.append("vLLM Recipe Ranking — %s" % parsed.get("model_id", "?"))
    lines.append("=" * 72)

    num_params = parsed.get("num_params")
    if num_params is None:
        params_str = "unknown"
    else:
        params_str = "{:,}".format(int(num_params))
    lines.append("model_id     : %s" % parsed.get("model_id", "?"))
    lines.append("num_params   : %s" % params_str)
    lines.append("budget (GiB) : %s   safety_margin: %s   hard-gate: %s GiB"
                 % (_fmt_num(budget_gib), _fmt_num(margin),
                    _fmt_num(budget_gib * margin)))
    lines.append("tensor_parallel_size: %d" % tp)

    warnings = parsed.get("warnings") or []
    if warnings:
        lines.append("warnings:")
        for w in warnings:
            lines.append("  - %s" % w)

    lines.append("-" * 72)

    # 표 (통과분 정렬순 먼저, 이어서 탈락분 — 전체 가시화).
    header = ("id", "quant", "max_len", "gmu", "est_vram_gib", "headroom_gib", "verdict")
    widths = (5, 18, 8, 6, 13, 13, 7)
    row_fmt = "  ".join("{:<%d}" % w for w in widths)
    lines.append(row_fmt.format(*header))
    lines.append(row_fmt.format(*("-" * w for w in widths)))

    ranked = result.get("ranked", [])
    failed = result.get("failed", [])

    def _row(r):
        verdict = "PASS" if r.get("gate_pass") else "FAIL"
        # error 가 있으면 verdict 칸에 표기(예: num_params_unknown → FAIL 사유).
        err = r.get("error")
        # 경고(예: offline_quant_on_non_prequantized_checkpoint)는 verdict 에 (!) 표식.
        if r.get("warning"):
            verdict = verdict + "(!)"
        est = _fmt_num(r.get("estimated_total_gib"))
        head = _fmt_num(r.get("headroom_gib"))
        if err and est == "-":
            est = "ERR"
        return row_fmt.format(
            str(r.get("id", "-")),
            str(r.get("quantization", "-")),
            str(r.get("max_model_len", "-")),
            _fmt_num(r.get("gpu_memory_utilization")),
            est,
            head,
            verdict,
        )

    for r in ranked:
        lines.append(_row(r))
    if failed:
        if ranked:
            lines.append("  " + "-" * 68)
        for r in failed:
            lines.append(_row(r))

    # 후보별 경고(결정론 계층) 명시 — 표의 (!) 표식 풀어쓰기.
    warned = [r for r in result.get("all", []) if r.get("warning")]
    if warned:
        lines.append("-" * 72)
        lines.append("recipe warnings (결정론 계층):")
        for r in warned:
            lines.append(
                "  ! %s quant=%s: %s"
                % (r.get("id", "-"), r.get("quantization", "-"), r.get("warning"))
            )
        if any(
            r.get("warning") == "offline_quant_on_non_prequantized_checkpoint"
            for r in warned
        ):
            lines.append(
                "    (awq/gptq 류는 사전양자화 체크포인트 필수 — 이 비prequantized "
                "모델에 지정 시 `vllm serve` 실패 가능. fp8·bitsandbytes 는 온라인 가능.)"
            )

    lines.append("-" * 72)
    lines.append("PASS: %d / %d candidates" % (len(ranked), len(result.get("all", []))))
    if ranked:
        top = ranked[0]
        lines.append("top recipe: %s (quant=%s, max_len=%s, gmu=%s, headroom=%s GiB)"
                     % (top.get("id"), top.get("quantization"),
                        top.get("max_model_len"),
                        _fmt_num(top.get("gpu_memory_utilization")),
                        _fmt_num(top.get("headroom_gib"))))
    else:
        lines.append("top recipe: (none passed the hard-gate — relax budget/margin or quantize)")
    lines.append("=" * 72)
    return "\n".join(lines)


def _main(argv=None):
    parser = argparse.ArgumentParser(
        description="Rank vLLM serving recipes against a VRAM budget hard-gate.")
    parser.add_argument("--parsed", required=True,
                        help="parse_model_config.py 의 JSON 출력 파일 경로")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--candidates",
                       help="후보 리스트 JSON 파일 (LLM 생성 candidates.json)")
    group.add_argument("--auto", action="store_true",
                       help="결정론 기본 그리드(auto_candidates) 사용")
    parser.add_argument("--tp", type=int, default=1, help="tensor_parallel_size")
    parser.add_argument("--budget", type=float, required=True,
                        help="VRAM 예산 (GiB)")
    parser.add_argument("--margin", type=float, default=0.90,
                        help="안전마진 하드게이트 임계 (기본 0.90)")
    parser.add_argument("--kv-bytes", type=int, default=2,
                        help="KV cache dtype 바이트 (기본 2=fp16)")
    parser.add_argument("--json", action="store_true",
                        help="리포트 표 대신 랭킹 결과 JSON 출력")
    args = parser.parse_args(argv)

    with open(args.parsed, "r", encoding="utf-8") as f:
        parsed = json.load(f)

    if args.auto:
        candidates = auto_candidates(parsed, args.tp)
    else:
        with open(args.candidates, "r", encoding="utf-8") as f:
            candidates = json.load(f)
        # id 없는 후보엔 r1..rN 부여(estimate/rank 가 id 보존 전제).
        for idx, cand in enumerate(candidates, start=1):
            cand.setdefault("id", "r%d" % idx)

    result = rank(parsed, candidates, args.tp, args.budget, args.margin, args.kv_bytes)

    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(render_report(result, parsed, args.budget, args.margin, args.tp))

    return 0


if __name__ == "__main__":
    sys.exit(_main())
