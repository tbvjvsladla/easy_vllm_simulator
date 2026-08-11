#!/usr/bin/env python3
"""estimate_vram.py — 결정론 VRAM 추정기 (vllm-recipe-explorer 스킬).

CONTRACT(FROZEN) estimate_vram.py 절 준수. plan §8 공식 리터럴:
    estimated_total = (weight_bytes/tp + KV_cache + overhead) / gpu_memory_utilization

importable 함수 `estimate(...)` + __main__ CLI 둘 다 제공.
quant_table 의 값(quant bpw · FRAMEWORK_OVERHEAD_BYTES)을 import 해 사용한다.
whichllm 패키지는 import 하지 않는다 — activation 공식은 골격을 벤더링(아래 출처 주석).
stdlib(json,os,sys,argparse) 만 사용. 외부 네트워크 호출 없음.
"""

from __future__ import annotations

import argparse
import json
import os
import sys

# scripts/ 디렉토리를 import 경로에 보장(직접 실행/타 스크립트에서 import 양쪽 대응).
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from quant_table import (  # noqa: E402
    FRAMEWORK_OVERHEAD_BYTES,
    is_offline_only_quant,
    vllm_quant_bpw,
    vllm_quant_bpw_or_none,
)

GIB = 1024 ** 3  # 단위 통일: GiB = 1024**3 (추정·예산 둘 다)


def _activation_bytes(eff_params: int, max_model_len: int) -> int:
    """Activation/scratch 버퍼 바이트 (벤더링).

    출처: seed/whichllm/src/whichllm/engine/vram.py `_activation_bytes` 골격 복사.
    base 400MB 플로어 + 파라미터당 ~0.08바이트 + 컨텍스트 4K당 +150MB.
    """
    base = 400_000_000  # 400 MB framework activation floor
    param_term = int(eff_params * 0.08)  # ~0.08 byte/param
    ctx_term = int((max_model_len / 4096) * 150_000_000)  # +150 MB per 4K
    return base + param_term + ctx_term


def estimate(
    parsed: dict,
    candidate: dict,
    tp: int,
    budget_gib: float,
    safety_margin: float,
    kv_cache_dtype_bytes: int = 2,
) -> dict:
    """단일 (모델 × 후보) 레시피의 VRAM 추정 + 하드게이트 판정.

    candidate = {id?, quantization, max_model_len, gpu_memory_utilization}.

    공식(plan §8 수용기준 리터럴 준수):
        estimated_total = (weight_bytes/tp + KV_cache + overhead) / gpu_memory_utilization

    반환: candidate 필드 + weight_gib, kv_gib, overhead_gib, estimated_total_gib,
          headroom_gib, gate_pass, (있으면) error.
    """
    cand_id = candidate.get("id")
    quantization = candidate.get("quantization")
    max_model_len = int(candidate["max_model_len"])
    gpu_memory_utilization = float(candidate["gpu_memory_utilization"])

    # 결과 골격(candidate 필드 보존).
    result: dict = {
        "id": cand_id,
        "quantization": quantization,
        "max_model_len": max_model_len,
        "gpu_memory_utilization": gpu_memory_utilization,
        "weight_gib": None,
        "kv_gib": None,
        "overhead_gib": None,
        "estimated_total_gib": None,
        "headroom_gib": None,
        "gate_pass": False,
    }

    serve_bpw = parsed.get("serve_bpw")
    num_params = parsed.get("num_params")
    prequantized = bool(parsed.get("prequantized"))

    # ── weight_bytes 산정 ──────────────────────────────────────────────
    # prequantized: native 가중치 바이트가 디스크에 그대로 있으므로 quant 축은 native 고정.
    # 비prequantized: num_params × 후보 quant 의 bpw 로 추정.
    if prequantized:
        weight_bytes = parsed.get("native_weight_bytes")
        if weight_bytes is None:
            result["error"] = "native_weight_bytes_unknown"
            result["gate_pass"] = False
            return result
        weight_bytes = float(weight_bytes)
    else:
        if num_params is None:
            # parse 단계에서 가중치 원천을 못 찾음 → 추정 불가, 게이트 실패.
            result["error"] = "num_params_unknown"
            result["gate_pass"] = False
            return result
        # awq/gptq(및 Marlin 변종)는 '사전양자화 체크포인트'를 요구한다(런타임 온라인
        # 양자화 불가 — 비prequantized 체크포인트라 사전양자화 가중치 자체가 부재; 네트워크 유무 무관). bf16 native 체크포인트에 이 quant 를 지정한
        # 레시피는 weight=num_params*0.5 로 예산을 통과해도 실제 `vllm serve` 에서
        # 사전양자화 가중치가 없어 서빙이 실패한다(오도된 feasible). 결정론 계층에서
        # 경고를 달아 LLM 산문 지시에만 의존하지 않게 한다(SKILL.md §2 ②를 결정론으로).
        # fp8·bitsandbytes 는 온라인 양자화 가능하므로 차단 대상이 아니다.
        if is_offline_only_quant(quantization):
            result["warning"] = "offline_quant_on_non_prequantized_checkpoint"
        # 미지 quant 키(테이블 미등록)면 ValueError traceback 으로 죽지 않고(=크래시-음성 금지)
        # graceful 변종으로 error dict 반환 — parse_model_config 의 동일 처리와 정합.
        bpw, _qwarn = vllm_quant_bpw_or_none(quantization, serve_bpw)
        if bpw is None:
            result["error"] = "unknown_quant_method"
            result["note"] = (
                "quant 테이블 미등록 ≠ vLLM 미지원 — vLLM 소스/HF 카드로 외부 확인 필요(HITL surface)"
            )
            result["gate_pass"] = False
            return result
        weight_bytes = float(num_params) * bpw

    # ── KV_cache ───────────────────────────────────────────────────────
    # KV_cache = 2 * layers * num_key_value_heads * head_dim * kv_dtype_bytes * max_model_len
    # (factor 2 = K + V. GQA 는 num_key_value_heads 사용 — MHA 면 = num_attention_heads.)
    num_hidden_layers = parsed.get("num_hidden_layers")
    num_key_value_heads = parsed.get("num_key_value_heads")
    head_dim = parsed.get("head_dim")
    if None in (num_hidden_layers, num_key_value_heads, head_dim):
        result["error"] = "kv_dims_unknown"
        result["gate_pass"] = False
        return result
    kv_cache = (
        2
        * int(num_hidden_layers)
        * int(num_key_value_heads)
        * int(head_dim)
        * int(kv_cache_dtype_bytes)
        * max_model_len
    )

    # ── overhead = activation + framework ──────────────────────────────
    # eff_params = num_params (MoE active 미상 → num_params 사용; 약간 과대=보수적).
    eff_params = int(num_params) if num_params is not None else 0
    activation = _activation_bytes(eff_params, max_model_len)
    overhead = activation + FRAMEWORK_OVERHEAD_BYTES

    # ── plan §8 공식 리터럴 ────────────────────────────────────────────
    # weight 만 /tp(텐서병렬로 가중치 분할). KV·overhead 는 /tp 하지 않는다 =
    # plan 공식 리터럴. 멀티노드에서 KV/overhead 를 노드수로 나누지 않아 보수적
    # 과대추정이 되며, 이는 OOM 하드게이트에 안전(과소추정으로 인한 OOM 회피).
    estimated_total = (weight_bytes / tp + kv_cache + overhead) / gpu_memory_utilization

    estimated_total_gib = estimated_total / GIB
    headroom_gib = budget_gib - estimated_total_gib
    gate_pass = estimated_total_gib <= budget_gib * safety_margin

    result["weight_gib"] = weight_bytes / GIB
    result["kv_gib"] = kv_cache / GIB
    result["overhead_gib"] = overhead / GIB
    result["estimated_total_gib"] = estimated_total_gib
    result["headroom_gib"] = headroom_gib
    result["gate_pass"] = bool(gate_pass)
    return result


# ===========================================================================
# Phase 2 — 절대 KV 클램프 모델 (CONTRACT v1).
#   vLLM 인자 --kv-cache-memory-bytes 로 GPU당 KV 바이트를 절대값으로 고정한다.
#   설정 시 gpu-memory-utilization 은 무시되며(=safety_margin 풀 상한으로만 emit),
#   총 VRAM = weights + non_kv_overhead + kv_cache_memory_bytes  (gmu로 나누지 않음 —
#   Phase1 estimate() 공식과 다름). 검증 = total <= budget_gib*safety_margin*GIB.
# 아래 함수는 모두 importable. 기존 estimate()/_activation_bytes 는 손대지 않는다.
# ===========================================================================


def per_token_kv_bytes(parsed: dict, kv_dtype_bytes: int) -> int:
    """토큰 1개당 KV 캐시 바이트.

    per_token_kv_bytes = 2 * num_hidden_layers * num_key_value_heads * head_dim
                         * kv_dtype_bytes   (factor 2 = K + V).
    GQA 면 num_key_value_heads(<num_attention_heads) 사용.
    """
    return (
        2
        * int(parsed["num_hidden_layers"])
        * int(parsed["num_key_value_heads"])
        * int(parsed["head_dim"])
        * int(kv_dtype_bytes)
    )


def required_kv_bytes(
    parsed: dict, max_model_len: int, batch: int, kv_dtype_bytes: int
) -> int:
    """주어진 max_model_len·batch 동시요청에 필요한 KV 캐시 총 바이트.

    required_kv_bytes = per_token_kv_bytes * max_model_len * batch, +2% 라운딩버퍼.

    vLLM 은 KV 를 고정 크기 블록 단위로 할당한다(block_size, 기본 16토큰) — 우리 선형식은
    블록 경계 반올림을 반영하지 않아 vLLM 자신의 `_check_enough_kv_cache_memory` 문턱에
    근소 미달하는 사례가 실측됐다(2026-08-11: gemma-4-e2b-it batch=1 "needed 0.24GiB > available
    0.24GiB", qwen3-4b batch=1 "needed 4.5GiB > available 4.5GiB" — 둘 다 표시상 동률이지만
    내부적으로 아주 조금 더 필요했다). 2% 버퍼는 GiB 스케일에서 무시할 만한 초과지만 이 타이를
    안전하게 넘긴다.
    """
    base = per_token_kv_bytes(parsed, kv_dtype_bytes) * int(max_model_len) * int(batch)
    return int(base * 1.02)


def max_safe_kv_bytes(
    budget_gib: float,
    safety_margin: float,
    weights_bytes: float,
    overhead_bytes: float,
) -> int:
    """천장(budget*margin) 안에서 KV 에 줄 수 있는 최대 바이트.

    max_safe_kv_bytes = int(budget_gib*safety_margin*GIB) - weights_bytes - overhead_bytes.
    음수일 수 있음(=weights+overhead 만으로 천장 초과 → vram_infeasible 신호).
    """
    return int(budget_gib * safety_margin * GIB) - int(weights_bytes) - int(overhead_bytes)


def weights_bytes_for(parsed: dict, quantization: "str | None") -> float:
    """모델 가중치 바이트 산정 (기존 estimate() weight 로직 추출/재사용).

    - prequantized: 디스크 native 가중치 바이트를 그대로 사용(native_weight_bytes).
    - 비prequantized: num_params × 후보 quant 의 bpw(vllm_quant_bpw).
    실패 시(원천 미상) ValueError 를 던진다(estimate 는 error dict 로 처리하지만,
    이 추출 함수는 호출측이 분기하도록 예외로 명시).
    """
    prequantized = bool(parsed.get("prequantized"))
    if prequantized:
        weight_bytes = parsed.get("native_weight_bytes")
        if weight_bytes is None:
            raise ValueError("native_weight_bytes_unknown")
        return float(weight_bytes)
    num_params = parsed.get("num_params")
    if num_params is None:
        raise ValueError("num_params_unknown")
    serve_bpw = parsed.get("serve_bpw")
    bpw = vllm_quant_bpw(quantization, serve_bpw)
    return float(num_params) * bpw


def estimate_absolute(
    parsed: dict,
    candidate: dict,
    weights_bytes: "float | None" = None,
    overhead_bytes: "float | None" = None,
    kv_dtype_bytes: int = 2,
) -> dict:
    """절대 KV 클램프 VRAM 추정 (Phase 2).

    total = weights + non_kv_overhead + kv_bytes  (gmu로 나누지 않음).
    weights/overhead 가 인자로 주어지면(vllm_profile 실측) 그 값 사용, 없으면 추정
    (기존 _activation_bytes + FRAMEWORK_OVERHEAD_BYTES).

    candidate 사용 키: quantization, max_model_len, batch, kv_cache_memory_bytes(있으면),
    budget_gib·safety_margin(있으면 headroom 계산용).

    반환: weight_gib, overhead_gib, kv_gib, total_gib, headroom_gib.
    """
    quantization = candidate.get("quantization")
    max_model_len = int(candidate["max_model_len"])
    batch = int(candidate.get("batch", 1))

    # ── weights ────────────────────────────────────────────────────────
    if weights_bytes is None:
        weights_bytes = weights_bytes_for(parsed, quantization)
    weights_bytes = float(weights_bytes)

    # ── non_kv_overhead ────────────────────────────────────────────────
    if overhead_bytes is None:
        num_params = parsed.get("num_params")
        eff_params = int(num_params) if num_params is not None else 0
        overhead_bytes = _activation_bytes(eff_params, max_model_len) + FRAMEWORK_OVERHEAD_BYTES
    overhead_bytes = float(overhead_bytes)

    # ── kv_bytes ───────────────────────────────────────────────────────
    # 후보가 kv_cache_memory_bytes(절대 클램프)를 명시하면 그 값을 사용,
    # 아니면 required_kv_bytes(max_model_len, batch)로 산정.
    kv_bytes = candidate.get("kv_cache_memory_bytes")
    if kv_bytes is None:
        kv_bytes = required_kv_bytes(parsed, max_model_len, batch, kv_dtype_bytes)
    kv_bytes = float(kv_bytes)

    total_bytes = weights_bytes + overhead_bytes + kv_bytes

    result: dict = {
        "weight_gib": weights_bytes / GIB,
        "overhead_gib": overhead_bytes / GIB,
        "kv_gib": kv_bytes / GIB,
        "total_gib": total_bytes / GIB,
        "headroom_gib": None,
    }
    # budget 가 주어지면 headroom(=budget - total) 계산.
    budget_gib = candidate.get("budget_gib")
    if budget_gib is not None:
        result["headroom_gib"] = float(budget_gib) - (total_bytes / GIB)
    return result


def max_feasible_max_len(
    parsed: dict,
    budget_gib: float,
    safety_margin: float,
    batch: int,
    quantization: "str | None",
    kv_dtype_bytes: int,
    weights_bytes: "float | None" = None,
    overhead_bytes: "float | None" = None,
) -> int:
    """천장 내에서 가능한 최대 max_model_len.

    max_safe_kv 를 batch·per_token_kv_bytes 로 나눠 가능한 토큰 길이를 구하고,
    2의 거듭제곱으로 내림(power-of-two floor), max_position_embeddings 로 상한한다.
    overhead 를 추정해야 하면 max_position_embeddings 기준으로 산정(보수적).
    feasible 길이가 없으면 0.
    """
    if weights_bytes is None:
        weights_bytes = weights_bytes_for(parsed, quantization)
    cap = parsed.get("max_position_embeddings")
    if overhead_bytes is None:
        num_params = parsed.get("num_params")
        eff_params = int(num_params) if num_params is not None else 0
        ref_len = int(cap) if cap is not None else 0
        overhead_bytes = _activation_bytes(eff_params, ref_len) + FRAMEWORK_OVERHEAD_BYTES

    safe_kv = max_safe_kv_bytes(budget_gib, safety_margin, weights_bytes, overhead_bytes)
    if safe_kv <= 0:
        return 0
    per_tok = per_token_kv_bytes(parsed, kv_dtype_bytes)
    if per_tok <= 0 or int(batch) <= 0:
        return 0
    raw_len = safe_kv // (per_tok * int(batch))
    if raw_len <= 0:
        return 0
    # 2의 거듭제곱으로 내림.
    pow2 = 1 << (int(raw_len).bit_length() - 1)
    if cap is not None:
        pow2 = min(pow2, int(cap))
    return int(pow2)


def max_feasible_batch(
    parsed: dict,
    budget_gib: float,
    safety_margin: float,
    max_model_len: int,
    quantization: "str | None",
    kv_dtype_bytes: int,
    weights_bytes: "float | None" = None,
    overhead_bytes: "float | None" = None,
) -> int:
    """천장 내에서 주어진 max_model_len 으로 가능한 최대 동시요청 batch.

    max_safe_kv 를 max_model_len·per_token_kv_bytes 로 나눠 floor. feasible 없으면 0.
    """
    if weights_bytes is None:
        weights_bytes = weights_bytes_for(parsed, quantization)
    if overhead_bytes is None:
        num_params = parsed.get("num_params")
        eff_params = int(num_params) if num_params is not None else 0
        overhead_bytes = _activation_bytes(eff_params, int(max_model_len)) + FRAMEWORK_OVERHEAD_BYTES

    safe_kv = max_safe_kv_bytes(budget_gib, safety_margin, weights_bytes, overhead_bytes)
    if safe_kv <= 0:
        return 0
    per_tok = per_token_kv_bytes(parsed, kv_dtype_bytes)
    denom = per_tok * int(max_model_len)
    if denom <= 0:
        return 0
    return int(safe_kv // denom)


def _main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        description="결정론 VRAM 추정기 (vllm-recipe-explorer)."
    )
    p.add_argument("--parsed", required=True, help="parse_model_config 출력 JSON 경로")
    p.add_argument("--quant", default=None, help="후보 quantization 플래그(none/fp8/awq/...)")
    p.add_argument("--max-model-len", type=int, required=True)
    p.add_argument("--gmu", type=float, required=True, help="gpu_memory_utilization (0~1)")
    p.add_argument("--tp", type=int, required=True, help="tensor_parallel_size")
    p.add_argument("--budget", type=float, required=True, help="vram_budget_gib")
    p.add_argument("--margin", type=float, required=True, help="safety_margin (하드게이트 임계)")
    p.add_argument("--kv-bytes", type=int, default=2, help="KV dtype 바이트(기본 2=fp16)")
    p.add_argument("--id", default=None, help="후보 id (선택)")
    args = p.parse_args(argv)

    with open(args.parsed, "r", encoding="utf-8") as f:
        parsed = json.load(f)

    candidate = {
        "id": args.id,
        "quantization": args.quant,
        "max_model_len": args.max_model_len,
        "gpu_memory_utilization": args.gmu,
    }
    result = estimate(
        parsed,
        candidate,
        tp=args.tp,
        budget_gib=args.budget,
        safety_margin=args.margin,
        kv_cache_dtype_bytes=args.kv_bytes,
    )
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
