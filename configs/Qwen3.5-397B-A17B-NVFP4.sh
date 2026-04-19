#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
# Qwen3.5-397B-A17B-NVFP4 모델별 사전 처리 + vllm serve
#
# 이 스크립트는 serve_runner.sh에서 두 번 source됨:
#   1) PRELOAD_ENV_ONLY=1: Ray 시작 전 환경변수 export (양 노드)
#   2) SKIP_SERVE=0 (master) / SKIP_SERVE=1 (slave): 실제 serve 시작 여부
# ═════════════════════════════════════════════════════════════════════

# ─── 환경변수 export (항상 실행됨) ───
# Ray 시작 전에 반드시 export되어야 효력 발생
export VLLM_TEST_FORCE_FP8_MARLIN=1
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True

# Ray OOM 킬러를 실질적으로 비활성화 (★ Ray 기동 전 export 필수)
export RAY_memory_usage_threshold=0.99
export RAY_memory_monitor_refresh_ms=0

# Ray object store 메모리 축소 (기본값 시스템 메모리의 30% → 2 GB)
export RAY_object_store_memory=2000000000

# HuggingFace 캐시 관련
export HF_HUB_DISABLE_IMPLICIT_TOKEN=1
export TRANSFORMERS_OFFLINE=1

# ─── PRELOAD_ENV_ONLY=1이면 환경변수만 export하고 즉시 종료 ───
if [ "${PRELOAD_ENV_ONLY:-0}" = "1" ]; then
    echo "[Qwen3.5-397B-NVFP4][preload] Env vars exported:"
    echo "  VLLM_TEST_FORCE_FP8_MARLIN=${VLLM_TEST_FORCE_FP8_MARLIN}"
    echo "  RAY_memory_usage_threshold=${RAY_memory_usage_threshold}"
    echo "  RAY_memory_monitor_refresh_ms=${RAY_memory_monitor_refresh_ms}"
    echo "  RAY_object_store_memory=${RAY_object_store_memory}"
    return 0
fi

# ─── SKIP_SERVE=1이면 serve 없이 종료 (slave 모드) ───
if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[Qwen3.5-397B-NVFP4] Skipping serve (slave mode)"
    return 0
fi

# ─── vllm serve 실행 (master only) ───
echo "[Qwen3.5-397B-NVFP4] Starting vLLM serve..."
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}"