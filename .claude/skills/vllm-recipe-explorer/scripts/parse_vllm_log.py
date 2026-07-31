#!/usr/bin/env python3
"""parse_vllm_log.py — vLLM 서빙 로그 파서 (vllm-recipe-explorer 스킬, Phase 2).

CONTRACT(FROZEN) parse_vllm_log.py 절 준수. 실측 메모리 분해를 vLLM 로그에서 추출한다.
    "Model weights take 51.70GiB; non_torch_memory takes 0.34GiB; PyTorch activation
     peak memory takes 1.46GiB; the rest of the memory reserved for KV Cache is 28.00GiB."
    "total_gpu_memory (95.00GiB) x gpu_memory_utilization (0.90) = 85.50GiB"
    "Available KV cache memory: 28.00 GiB"
    "GPU KV cache size: 425,984 tokens"
    "Maximum concurrency for 32,768 tokens per request: 13.00x"
    "Using FlashInfer backend" / "Using Flash Attention backend"

importable 함수 `parse_log(text)->dict` + __main__ CLI 둘 다 제공.
모든 필드는 매칭 실패 시 null(예외 던지지 말 것). 정규식은 GiB/GB·쉼표·공백 변형에 견고하게.
stdlib(json,re,sys,argparse) 만 사용. 외부 네트워크 호출 없음.
"""

from __future__ import annotations

import argparse
import json
import re
import sys

# ── 정규식 빌딩블럭 ──────────────────────────────────────────────────────
# GiB/GB 단위 + 쉼표/공백 변형에 견고하게. 숫자는 쉼표 천단위 허용 후 제거.
_NUM = r"([\d,]+(?:\.\d+)?)"          # 정수/소수, 천단위 쉼표 허용
_GIB = r"\s*Gi?B"                      # "GiB" 또는 "GB", 앞 공백 0~N개


def _to_float(s: str) -> float:
    """쉼표 천단위를 제거하고 float 로 변환."""
    return float(s.replace(",", ""))


def _to_int(s: str) -> int:
    """쉼표 천단위를 제거하고 int 로 변환."""
    return int(s.replace(",", ""))


def _search_float(pattern: str, text: str) -> float | None:
    """pattern(그룹1=숫자)을 대소문자 무시로 검색해 float 반환. 없으면 None."""
    m = re.search(pattern, text, re.IGNORECASE)
    return _to_float(m.group(1)) if m else None


def parse_log(text: str) -> dict:
    """vLLM 서빙 로그 텍스트에서 실측 메모리 분해를 추출한다.

    반환 dict keys (없으면 null):
        weights_gib, kv_cache_gib, kv_cache_tokens(int), max_concurrency(float),
        non_torch_gib, torch_activation_peak_gib, total_pool_gib,
        non_kv_overhead_gib, attention_backend_used(str), raw_matched(dict 증거).

    non_kv_overhead_gib = total_pool_gib - weights_gib - kv_cache_gib
        (셋 다 있을 때만; 음수면 null).
    어떤 라인이 없으면 그 필드만 null — 예외를 던지지 않는다.
    """
    raw: dict = {}  # 매칭 증거(원문 일부)를 남겨 디버깅·재현에 사용

    # ── weights_gib ──────────────────────────────────────────────────────
    # 형식 1: "model weights take 51.70GiB"  (메모리 분해 라인)
    # 형식 2: "Model weights take 51.7000 GiB and 45.3 seconds"  (loading 라인)
    weights_gib = _search_float(
        r"(?:model\s+weights\s+take|weights\s+take)\s+" + _NUM + _GIB, text
    )
    # 형식 3 (NGC/0.22 빌드 — consolidated 라인 없음): "Model loading took 51.1 GiB memory and ..."
    # 형식 3b (vLLM 0.26.x): "Model loading took 9.84 GiB and 62.89 seconds" — **"memory" 단어가
    #   사라졌다**. 종전 패턴이 `GiB\s+memory` 를 강제해 0.26.x 에서 weights_gib=null 이 됐고,
    #   그 null 이 _resolve_clamp_kv 를 무력화해 trial-loop 이 조정 없이 바일아웃했다
    #   (2026-07-31 워치독 실화 · testlog_26073116 D6). "memory" 를 선택항으로 완화한다.
    if weights_gib is None:
        weights_gib = _search_float(
            r"Model\s+loading\s+took\s+" + _NUM + _GIB + r"(?:\s+memory)?\b", text
        )

    # ── non_torch_gib ────────────────────────────────────────────────────
    non_torch_gib = _search_float(
        r"non_torch_memory\s+takes\s+" + _NUM + _GIB, text
    )

    # ── torch_activation_peak_gib ────────────────────────────────────────
    torch_activation_peak_gib = _search_float(
        r"(?:PyTorch\s+)?activation\s+peak\s+memory\s+takes\s+" + _NUM + _GIB, text
    )

    # ── kv_cache_gib ─────────────────────────────────────────────────────
    # 형식 1: "the rest of the memory reserved for KV Cache is 28.00GiB"
    # 형식 2: "Available KV cache memory: 28.00 GiB"
    kv_cache_gib = _search_float(
        r"reserved\s+for\s+KV\s+Cache\s+is\s+" + _NUM + _GIB, text
    )
    if kv_cache_gib is None:
        kv_cache_gib = _search_float(
            r"Available\s+KV\s+cache\s+memory:\s*" + _NUM + _GIB, text
        )
    # 형식 3 (vLLM 0.26.x · **절대 클램프를 준 경우**): "Initial free memory 113.47 GiB,
    #   reserved 20.0 GiB memory for KV Cache as specified by kv_cache_memory_bytes config".
    #   수치가 "for KV Cache" **앞**에 온다 — 형식 1('...is N GiB')과 어순이 반대라 빗나갔다.
    #   클램프를 준 트라이얼에서 kv_cache_gib 가 통째로 null 이 되던 원인(2026-07-31).
    #   ※ lite_metrics.parse_engine_log 에도 동일 계열 결함이 있었다(D4) — 파서가 두 벌이므로
    #     한쪽만 고치면 다른 쪽이 남는다. 어형 변화 발견 시 **양쪽 다** 확인할 것.
    if kv_cache_gib is None:
        kv_cache_gib = _search_float(
            r"reserved\s+" + _NUM + _GIB + r"\s+memory\s+for\s+KV\s+Cache", text
        )

    # ── total_pool_gib ───────────────────────────────────────────────────
    # "total_gpu_memory (95.00GiB) x gpu_memory_utilization (0.90) = 85.50GiB"
    # → 우변(= 뒤)이 실제 사용 가능 풀 상한.
    total_pool_gib = _search_float(
        r"gpu_memory_utilization\s*\([\d.]+\)\s*=\s*" + _NUM + _GIB, text
    )

    # ── gmu_trial (이 트라이얼의 유효 gpu-memory-utilization) ──────────────
    # consolidated 라인이 없는 빌드는 total_pool 을 직접 안 주므로, gmu 와 device_total
    # 로 overhead 를 유도하기 위해 gmu 를 뽑는다.
    #   "The current --gpu-memory-utilization=0.9200 is equivalent ..."  (첫 매칭=현재값)
    #   "gpu_memory_utilization (0.90)"
    gmu_trial = _search_float(
        r"gpu[-_]memory[-_]utilization\s*[=(]\s*(\d+(?:\.\d+)?)", text
    )

    # ── cuda_graph_gib (실측 그래프 풀; 있으면) ───────────────────────────
    # "CUDA graph pool memory: 0.18 GiB (actual), ..."
    cuda_graph_gib = _search_float(
        r"CUDA\s+graph\s+pool\s+memory:\s*" + _NUM + _GIB, text
    )

    # ── kv_cache_tokens ──────────────────────────────────────────────────
    # "GPU KV cache size: 425,984 tokens"  (쉼표 제거 후 int)
    kv_cache_tokens = None
    m = re.search(r"GPU\s+KV\s+cache\s+size:\s*" + _NUM + r"\s*tokens", text, re.IGNORECASE)
    if m:
        kv_cache_tokens = _to_int(m.group(1))

    # ── max_concurrency ──────────────────────────────────────────────────
    # "Maximum concurrency for 32,768 tokens per request: 13.00x"
    max_concurrency = _search_float(
        r"Maximum\s+concurrency\s+for\s+[\d,]+\s+tokens\s+per\s+request:\s*" + _NUM + r"x",
        text,
    )

    # ── attention_backend_used ───────────────────────────────────────────
    # "Using FlashInfer backend" / "Using Flash Attention backend" / "Using FlashMLA backend"
    attention_backend_used = None
    m = re.search(r"Using\s+(.+?)\s+backend", text, re.IGNORECASE)
    if m:
        attention_backend_used = m.group(1).strip()

    # ── non_kv_overhead_gib (파생) ───────────────────────────────────────
    # total_pool - weights - kv (셋 다 있을 때만; 음수면 null).
    non_kv_overhead_gib = None
    if None not in (total_pool_gib, weights_gib, kv_cache_gib):
        overhead = total_pool_gib - weights_gib - kv_cache_gib
        non_kv_overhead_gib = overhead if overhead >= 0 else None

    # 매칭 증거: 추출된 비-null 값만 기록(재현·디버깅용).
    for k, v in (
        ("weights_gib", weights_gib),
        ("non_torch_gib", non_torch_gib),
        ("torch_activation_peak_gib", torch_activation_peak_gib),
        ("kv_cache_gib", kv_cache_gib),
        ("total_pool_gib", total_pool_gib),
        ("kv_cache_tokens", kv_cache_tokens),
        ("max_concurrency", max_concurrency),
        ("attention_backend_used", attention_backend_used),
        ("gmu_trial", gmu_trial),
        ("cuda_graph_gib", cuda_graph_gib),
    ):
        if v is not None:
            raw[k] = v

    return {
        "weights_gib": weights_gib,
        "kv_cache_gib": kv_cache_gib,
        "kv_cache_tokens": kv_cache_tokens,
        "max_concurrency": max_concurrency,
        "non_torch_gib": non_torch_gib,
        "torch_activation_peak_gib": torch_activation_peak_gib,
        "total_pool_gib": total_pool_gib,
        "non_kv_overhead_gib": non_kv_overhead_gib,
        "gmu_trial": gmu_trial,
        "cuda_graph_gib": cuda_graph_gib,
        "attention_backend_used": attention_backend_used,
        "raw_matched": raw,
    }


def _main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        description="vLLM 서빙 로그 파서 (vllm-recipe-explorer Phase 2)."
    )
    p.add_argument(
        "--log",
        default=None,
        help="vLLM 로그 파일 경로(미지정 시 stdin 에서 읽음)",
    )
    args = p.parse_args(argv)

    if args.log:
        with open(args.log, "r", encoding="utf-8") as f:
            text = f.read()
    else:
        text = sys.stdin.read()

    result = parse_log(text)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())


# ── 기대 파싱값(CONTRACT SAMPLE_VLLM_LOG 기준 자체 검증) ──────────────────
# weights_gib              ≈ 51.70   (메모리 분해 라인 "model weights take 51.70GiB";
#                                      loading 라인 "Model weights take 51.7000 GiB" 도 동일값)
# kv_cache_gib             ≈ 28.00   ("reserved for KV Cache is 28.00GiB")
# kv_cache_tokens          == 425984 ("GPU KV cache size: 425,984 tokens")
# max_concurrency          ≈ 13.00   ("Maximum concurrency ...: 13.00x")
# non_torch_gib            ≈ 0.34
# torch_activation_peak_gib≈ 1.46
# total_pool_gib           ≈ 85.50   ("... gpu_memory_utilization (0.90) = 85.50GiB")
# non_kv_overhead_gib      ≈ 5.80    (= 85.50 - 51.70 - 28.00)
# attention_backend_used   == "FlashInfer"  ("Using FlashInfer backend")
