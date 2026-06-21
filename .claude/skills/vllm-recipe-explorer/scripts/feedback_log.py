#!/usr/bin/env python3
"""feedback_log.py — vllm-recipe-explorer 되먹임 로그 라이터.

CONTRACT(FROZEN) 준수:
- `append(record, path)` 가 JSONL 한 줄을 append. 디렉토리는 mkdir.
- record 는 '추정 채움' 필드(generate 시점에 결정론적으로 채운 값) + '실측 nullable 예약'
  필드(서빙/실측 단계 = Phase 1 범위 밖 → 전부 null)로 구성된다.
- stdlib 단독(json,os,sys,argparse,datetime). 외부 네트워크/패키지 import 금지.

Phase 1 범위: 추정·추천·생성까지만. 품질·속도 측정/실서빙·실측은 범위 밖이므로
actual_vram_gb / serve_success / error_type / tokens_per_sec 는 null 로 예약해 둔다(추후 채움).
"""

import argparse
import json
import os
import sys
from datetime import datetime

# 기본 로그 경로(SKILL 루트 = 이 파일의 두 단계 상위) 아래 feedback/recipe_feedback.jsonl.
_SKILL_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_LOG_PATH = os.path.join(_SKILL_ROOT, "feedback", "recipe_feedback.jsonl")

# 추정 단계에서 채우는 필수 필드(키 고정).
ESTIMATED_FIELDS = (
    "timestamp",
    "model_id",
    "quantization",
    "max_model_len",
    "gpu_memory_utilization",
    "vram_budget_gb",
    "estimated_vram_gb",
    "tensor_parallel_size",
    "safety_margin_threshold",
    "selection_timestamp",
)

# 실측(서빙) 단계 nullable 예약 필드 — Phase 1 에서는 전부 null.
# Phase 2 시뮬레이터 예약 필드(있으면 기록, 없으면 null) 추가.
NULLABLE_FIELDS = (
    "actual_vram_gb",
    "serve_success",
    "error_type",
    "tokens_per_sec",
    "batch",
    "kv_cache_memory_bytes",
    "attention_backend",
    "tool_call_parser",
    "reasoning_parser",
    "converged",
    "trial_count",
    "correction_history",
)


def build_record(
    model_id,
    quantization,
    max_model_len,
    gpu_memory_utilization,
    vram_budget_gb,
    estimated_vram_gb,
    tensor_parallel_size,
    safety_margin_threshold,
    selection_timestamp=None,
):
    """추정 필드를 채우고 실측 nullable 필드를 예약한 완전한 record dict 를 만든다.

    timestamp / selection_timestamp 미지정 시 datetime.now().isoformat() 로 채운다.
    """
    now_iso = datetime.now().isoformat()
    record = {
        "timestamp": now_iso,
        "model_id": model_id,
        "quantization": quantization,
        "max_model_len": max_model_len,
        "gpu_memory_utilization": gpu_memory_utilization,
        "vram_budget_gb": vram_budget_gb,
        "estimated_vram_gb": estimated_vram_gb,
        "tensor_parallel_size": tensor_parallel_size,
        "safety_margin_threshold": safety_margin_threshold,
        "selection_timestamp": selection_timestamp if selection_timestamp is not None else now_iso,
    }
    # 실측 nullable 예약(전부 null).
    for key in NULLABLE_FIELDS:
        record[key] = None
    return record


def _normalize(record):
    """append 직전 record 정규화: timestamp/selection_timestamp 자동 채움 +
    nullable 예약 필드를 누락 없이 보강(존재하면 보존)."""
    out = dict(record)
    now_iso = datetime.now().isoformat()
    if not out.get("timestamp"):
        out["timestamp"] = now_iso
    if not out.get("selection_timestamp"):
        out["selection_timestamp"] = out["timestamp"]
    for key in NULLABLE_FIELDS:
        out.setdefault(key, None)
    return out


def append(record, path=DEFAULT_LOG_PATH):
    """record(dict) 를 JSONL 한 줄로 path 에 append. 디렉토리는 mkdir.

    반환: 기록된 path(str).
    """
    norm = _normalize(record)
    log_dir = os.path.dirname(os.path.abspath(path))
    if log_dir:
        os.makedirs(log_dir, exist_ok=True)
    line = json.dumps(norm, ensure_ascii=False)
    with open(path, "a", encoding="utf-8") as fh:
        fh.write(line + "\n")
    return path


def _build_arg_parser():
    p = argparse.ArgumentParser(
        description="vllm-recipe-explorer 되먹임 로그(JSONL) append."
    )
    p.add_argument("--path", default=DEFAULT_LOG_PATH, help="JSONL 로그 경로(기본=feedback/recipe_feedback.jsonl)")
    # 추정 필드(필수).
    p.add_argument("--model-id", required=True)
    p.add_argument("--quantization", required=True)
    p.add_argument("--max-model-len", type=int, required=True)
    p.add_argument("--gpu-memory-utilization", type=float, required=True)
    p.add_argument("--vram-budget-gb", type=float, required=True)
    p.add_argument("--estimated-vram-gb", type=float, required=True)
    p.add_argument("--tensor-parallel-size", type=int, required=True)
    p.add_argument("--safety-margin-threshold", type=float, required=True)
    p.add_argument("--selection-timestamp", default=None, help="미지정 시 now()")
    # 대안: 완성된 record JSON 을 직접 주입(--record 사용 시 위 추정 인자는 무시).
    p.add_argument("--record", default=None, help="완성 record JSON 문자열(주면 개별 인자 대신 이 record 를 append)")
    return p


def main(argv=None):
    args = _build_arg_parser().parse_args(argv)
    if args.record is not None:
        record = json.loads(args.record)
    else:
        record = build_record(
            model_id=args.model_id,
            quantization=args.quantization,
            max_model_len=args.max_model_len,
            gpu_memory_utilization=args.gpu_memory_utilization,
            vram_budget_gb=args.vram_budget_gb,
            estimated_vram_gb=args.estimated_vram_gb,
            tensor_parallel_size=args.tensor_parallel_size,
            safety_margin_threshold=args.safety_margin_threshold,
            selection_timestamp=args.selection_timestamp,
        )
    written = append(record, args.path)
    print(f"appended 1 record -> {written}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
