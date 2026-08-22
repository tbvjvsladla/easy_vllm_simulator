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

# KV 블록정렬 라운딩 버퍼 — **단일 소유**(2026-08-13 추출).
#   vLLM 은 KV 를 고정 블록 단위(block_size, 기본 16토큰)로 할당하는데 우리 선형식
#   (per_token × max_len × batch)은 블록 경계 반올림을 반영하지 못해, vLLM 자신의
#   `_check_enough_kv_cache_memory` 문턱에 **근소 미달**하는 타이가 실측됐다
#   (2026-08-11: gemma-4-e2b-it "needed 0.24GiB > available 0.24GiB", qwen3-4b "4.5GiB > 4.5GiB"
#    — 표시상 동률이나 내부적으로 조금 더 필요했다).
#   ⚠ 소비자가 둘이다: 여기 required_kv_bytes()(공식 폴백 경로)와 recipe._resolve_clamp_kv()의
#   measured-per-token 경로(실제로 항상 우선 실행되는 진짜 적용점). 값을 두 벌로 두면 갈라지므로
#   **여기서만 선언**하고 recipe 는 import 해서 쓴다. 정책 술어
#   KV_ABSOLUTE_CLAMP_PORTABILITY.C3 도 이 상수를 참조해 기대값을 세운다.
KV_BLOCK_ALIGN_BUFFER = 1.02


def kv_bearing_layers(parsed: dict) -> "int | None":
    """KV 캐시를 **실제로 보유하는** 층 수. KV 공식의 `layers` 항 단일 소유자.

    하이브리드(GDN/linear-attention) 모델은 전 층이 full-KV 가 아니다 — Qwen3.8-27B 는
    64층 중 16층만 `full_attention` 이고 48층은 `linear_attention`(conv+recurrent state,
    토큰 비례 KV 아님)이다. 전층으로 세면 KV 가 **3.95× 과대추정**되어 Phase-1 하드게이트가
    실제로 서빙되는 티어를 FAIL 시킨다(2026-08-22 실측 · plan_26082223 결함 A ·
    `docs/simlog/26082221_qwen38_27b_R0기준선/raw/P0_P2_findings.md`).

    선택 규칙(순수 산술 — 여기가 단일 소유다):
      `num_full_attention_layers` 가 `0 < n <= num_hidden_layers` 면 그 값, 아니면 전층.
    → **폴백이 현행 동작**이므로 비하이브리드/구버전 parsed dict 는 회귀 0 이다.

    ★ `parsed["is_hybrid"]` 는 **보고용 라벨**이고 이 함수는 그것을 읽지 않는다. 같은 판정을
      두 곳에서 각자 하면 갈라진다(§결정론 규율 — 개념 중복). 라벨은 parse_model_config 가,
      산술은 이 함수가 소유한다.
    """
    total = parsed.get("num_hidden_layers")
    n_full = parsed.get("num_full_attention_layers")
    if n_full is None:
        return int(total) if total is not None else None
    try:
        n_full = int(n_full)
    except (TypeError, ValueError):
        return int(total) if total is not None else None
    if total is None:
        # 전층을 모르면 상한 검증을 못 한다 — 값 자체는 결정론이므로 그대로 쓴다.
        return n_full if n_full > 0 else None
    total = int(total)
    if not (0 < n_full <= total):
        # 부정합(0 이하·전층 초과) → 보수적 전층 폴백. parse 단계가 이미 경고를 남긴다.
        return total
    return n_full


def kv_layers_source(parsed: dict) -> str:
    """`kv_bearing_layers` 가 어느 축을 썼는지의 출처 표시(§결정론 규율 — `*_source`)."""
    total = parsed.get("num_hidden_layers")
    total = int(total) if total is not None else None
    return (
        "num_hidden_layers"
        if kv_bearing_layers(parsed) == total
        else "num_full_attention_layers"
    )


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
    # layers 항은 `kv_bearing_layers` 가 단일 소유한다 — 하이브리드면 full-attention 층수,
    # 아니면 전층(현행 = 폴백). plan_26082223 결함 A.
    kv_layers = kv_bearing_layers(parsed)
    num_key_value_heads = parsed.get("num_key_value_heads")
    head_dim = parsed.get("head_dim")
    if None in (kv_layers, num_key_value_heads, head_dim):
        result["error"] = "kv_dims_unknown"
        result["gate_pass"] = False
        return result
    kv_cache = (
        2
        * int(kv_layers)
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
    # 출처 표시(§결정론 규율): KV 를 몇 층으로 셌는지와 그 근거 축을 값 옆에 남긴다.
    result["kv_layers"] = int(kv_layers)
    result["kv_layers_source"] = kv_layers_source(parsed)
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

    per_token_kv_bytes = 2 * kv_bearing_layers * num_key_value_heads * head_dim
                         * kv_dtype_bytes   (factor 2 = K + V).
    GQA 면 num_key_value_heads(<num_attention_heads) 사용.
    layers 항은 `kv_bearing_layers()` 가 정한다 — 하이브리드는 full-attention 층수, 그 외 전층
    (plan_26082223 결함 A). 이 경로는 `recipe._resolve_clamp_kv` 의 **측정 per-token 부재 시
    공식 폴백**이므로, 측정이 있으면 여전히 측정이 우선한다(측정 > 공식).
    """
    layers = kv_bearing_layers(parsed)
    if layers is None:
        raise KeyError("num_hidden_layers")
    return (
        2
        * int(layers)
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
    return int(base * KV_BLOCK_ALIGN_BUFFER)


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


# ===========================================================================
# 자체검사 (--self-test) — 하이브리드 KV 층수 인지(plan_26082223 결함 A).
#   픽스처는 **Qwen3.8-27B 실측 형상**(64층 · full 16 · kv_heads 4 · head_dim 256)이며
#   기대값 16 GiB 는 손계산 리터럴이 아니라 공식의 산출값이다. 모델·하드웨어 불요.
# ===========================================================================

# 2026-08-22 실측 per-token KV — `Available KV cache memory 52.28 GiB` ÷
# `GPU KV cache size 846,926 tokens` (docs/simlog/26082221_qwen38_27b_R0기준선/raw/
#  P0_P2_findings.md). 공식이 이 실측과 어긋나면 자체검사가 빨간불을 켠다.
_MEASURED_PER_TOKEN_QWEN38_27B = 66290


def _self_test() -> int:
    failures: list[str] = []

    def check(name, got, want):
        if got != want:
            failures.append("%s: got=%r want=%r" % (name, got, want))

    hybrid = {"num_hidden_layers": 64, "num_full_attention_layers": 16, "is_hybrid": True,
              "num_key_value_heads": 4, "head_dim": 256, "max_position_embeddings": 262144,
              "num_params": 27_000_000_000, "prequantized": False, "serve_bpw": 2.0}
    # 비하이브리드 = **필드 자체가 없는** 구형 parsed dict(회귀 0 을 이 형태로 증명한다).
    dense = {k: v for k, v in hybrid.items() if k != "num_full_attention_layers"}
    dense["is_hybrid"] = False
    cand = {"id": "selftest", "quantization": None, "max_model_len": 262144,
            "gpu_memory_utilization": 0.90}

    # E1 — 하이브리드: 262k KV = 16 GiB (전층 64 GiB 의 정확히 1/4).
    r_h = estimate(hybrid, cand, tp=1, budget_gib=115.0, safety_margin=0.90)
    check("E1.kv_gib", r_h["kv_gib"], 16.0)
    check("E1.kv_layers", r_h["kv_layers"], 16)
    check("E1.kv_layers_source", r_h["kv_layers_source"], "num_full_attention_layers")

    # E2 — 비하이브리드 회귀 0: 현행값(전층 64 GiB) 유지.
    r_d = estimate(dense, cand, tp=1, budget_gib=115.0, safety_margin=0.90)
    check("E2.kv_gib", r_d["kv_gib"], 64.0)
    check("E2.kv_layers", r_d["kv_layers"], 64)
    check("E2.kv_layers_source", r_d["kv_layers_source"], "num_hidden_layers")
    check("E2.ratio_is_exactly_quarter", r_h["kv_gib"] * 4, r_d["kv_gib"])
    # 회귀 0 의 진짜 판정: KV 밖 항이 하나도 안 움직였는지.
    for k in ("weight_gib", "overhead_gib"):
        check("E2.%s_unchanged" % k, r_h[k], r_d[k])

    # E3 — 전층이 full_attention 인 모델은 하이브리드가 아니다(값은 있어도 전층).
    allfull = dict(dense, num_full_attention_layers=64)
    check("E3.layers", kv_bearing_layers(allfull), 64)
    check("E3.source", kv_layers_source(allfull), "num_hidden_layers")

    # E4 — 부정합은 **보수적 전층 폴백**이다(과소추정 금지).
    check("E4.zero", kv_bearing_layers(dict(dense, num_full_attention_layers=0)), 64)
    check("E4.over", kv_bearing_layers(dict(dense, num_full_attention_layers=99)), 64)
    check("E4.garbage", kv_bearing_layers(dict(dense, num_full_attention_layers="x")), 64)

    # E5 — per_token / required 도 같은 축을 쓴다(KV 항의 단일 소유).
    check("E5.per_token_hybrid", per_token_kv_bytes(hybrid, 2), 65536)
    check("E5.per_token_dense", per_token_kv_bytes(dense, 2), 262144)
    check("E5.required_hybrid", required_kv_bytes(hybrid, 262144, 1, 2),
          int(65536 * 262144 * KV_BLOCK_ALIGN_BUFFER))

    # E6 — **실측 대조**(그라운딩): 하이브리드 공식은 실측의 ±5% 안, 전층 공식은 밖.
    err_h = abs(per_token_kv_bytes(hybrid, 2) - _MEASURED_PER_TOKEN_QWEN38_27B) / \
        _MEASURED_PER_TOKEN_QWEN38_27B
    err_d = abs(per_token_kv_bytes(dense, 2) - _MEASURED_PER_TOKEN_QWEN38_27B) / \
        _MEASURED_PER_TOKEN_QWEN38_27B
    if not err_h <= 0.05:
        failures.append("E6.hybrid_vs_measured: 오차 %.3f > 0.05 (공식이 실측과 어긋난다)" % err_h)
    if not err_d > 1.0:
        failures.append("E6.dense_vs_measured: 전층 공식 오차 %.3f — 결함 A 전제가 무너졌다" % err_d)

    # E7 — 하드게이트 결과가 실제로 뒤집힌다(이 수정의 존재 이유).
    #      262k 티어는 as-shipped 에서 FAIL 이었고 교정 후 PASS 여야 한다.
    check("E7.hybrid_gate", r_h["gate_pass"], True)
    check("E7.dense_gate", r_d["gate_pass"], False)

    # E8 — 파생 함수(max_feasible_*)도 같은 축을 타는지. 하이브리드가 4배 긴 ctx 를 준다.
    kw = dict(budget_gib=115.0, safety_margin=0.90, quantization=None, kv_dtype_bytes=2,
              weights_bytes=51.1 * GIB, overhead_bytes=2.0 * GIB)
    len_h = max_feasible_max_len(hybrid, batch=1, **kw)
    len_d = max_feasible_max_len(dense, batch=1, **kw)
    if not (len_h >= len_d * 4 or len_h == int(hybrid["max_position_embeddings"])):
        failures.append("E8.max_len: hybrid=%d dense=%d — 파생 함수가 층수 인지를 안 탄다"
                        % (len_h, len_d))
    b_h = max_feasible_batch(hybrid, max_model_len=32768, **kw)
    b_d = max_feasible_batch(dense, max_model_len=32768, **kw)
    # per-token 이 정확히 1/4 이므로 batch 는 4배 대역에 든다. floor 나눗셈이라 정확히 4배가
    # 아닐 수 있어(잔여가 hybrid 쪽에서 한 칸 더 나온다) 상·하한으로 단언한다.
    if not (b_d * 4 <= b_h <= (b_d + 1) * 4 - 1):
        failures.append("E8.batch: hybrid=%d dense=%d — 4배 대역 밖(층수 인지 미적용 의심)"
                        % (b_h, b_d))

    if failures:
        sys.stderr.write("[estimate_vram --self-test] FAIL %d 건:\n" % len(failures))
        for f in failures:
            sys.stderr.write("  - %s\n" % f)
        return 1
    sys.stdout.write("[estimate_vram --self-test] OK — E1~E8 통과"
                     " (하이브리드 KV 층수 인지 · plan_26082223 결함 A)\n")
    return 0


def _main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        description="결정론 VRAM 추정기 (vllm-recipe-explorer)."
    )
    p.add_argument("--parsed", help="parse_model_config 출력 JSON 경로")
    p.add_argument("--self-test", action="store_true",
                   help="하이브리드 KV 층수 인지 회귀(모델/하드웨어 불요)")
    p.add_argument("--quant", default=None, help="후보 quantization 플래그(none/fp8/awq/...)")
    p.add_argument("--max-model-len", type=int)
    p.add_argument("--gmu", type=float, help="gpu_memory_utilization (0~1)")
    p.add_argument("--tp", type=int, help="tensor_parallel_size")
    p.add_argument("--budget", type=float, help="vram_budget_gib")
    p.add_argument("--margin", type=float, help="safety_margin (하드게이트 임계)")
    p.add_argument("--kv-bytes", type=int, default=2, help="KV dtype 바이트(기본 2=fp16)")
    p.add_argument("--id", default=None, help="후보 id (선택)")
    args = p.parse_args(argv)

    if args.self_test:
        return _self_test()
    _missing = [n for n, v in (("--parsed", args.parsed), ("--max-model-len", args.max_model_len),
                               ("--gmu", args.gmu), ("--tp", args.tp),
                               ("--budget", args.budget), ("--margin", args.margin))
                if v is None]
    if _missing:
        p.error("%s 는 필수다(--self-test 는 예외)" % ", ".join(_missing))

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
