#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
# Qwen3.5-122B-A10B-NVFP4 모델별 사전 처리 + vllm serve
# serve_runner.sh 에서 source 호출됨
# ═════════════════════════════════════════════════════════════════════

# ─── 모델별 환경변수 (현재 특별한 것 없음) ───
# export VLLM_SOMETHING=...

# ─── SKIP_SERVE=1이면 환경변수만 세팅하고 종료 (slave 모드) ───
if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[Qwen3.5-122B-NVFP4] Env vars loaded, skipping serve (slave mode)"
    return 0
fi

# ─── vLLM serve 실행 (master only) ───
echo "[Qwen3.5-122B-NVFP4] Starting vLLM serve..."
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}"