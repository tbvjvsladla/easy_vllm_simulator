#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
# GLM-4.5-Air-FP8 모델별 사전 처리 + vllm serve
# 106B total / 12B active, hybrid reasoning (thinking/non-thinking)
# serve_runner.sh 에서 source 호출됨
# ═════════════════════════════════════════════════════════════════════

# ─── GLM-4.5 전용 환경변수 ───
# flash infer 이슈 발생 시 XFORMERS로 대체 (참조: 모델 카드)
# export VLLM_ATTENTION_BACKEND=XFORMERS
# GB10 (Blackwell SM121)에서 flash infer 사용하려면 TORCH_CUDA_ARCH_LIST 설정 필요
# export TORCH_CUDA_ARCH_LIST='12.1+PTX'

# ─── SKIP_SERVE=1이면 환경변수만 세팅하고 종료 (slave 모드) ───
if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[GLM-4.5-Air-FP8] Env vars loaded, skipping serve (slave mode)"
    return 0
fi

# ─── vLLM serve 실행 (master only) ───
echo "[GLM-4.5-Air-FP8] Starting vLLM serve..."
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}"