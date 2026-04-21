#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
# gpt-oss-120b-mxfp4 모델별 사전 처리 + vllm serve
# serve_runner.sh 에서 source 호출됨
# ═════════════════════════════════════════════════════════════════════

# ─── 모델별 환경변수 : TIKTOKEN 경로 설정 ───
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

# ─── SKIP_SERVE=1이면 환경변수만 세팅하고 종료 (slave 모드) ───
if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[gpt-oss-120b] Env vars loaded, skipping serve (slave mode)"
    return 0
fi

# ─── vLLM serve 실행 (master only) ───
echo "[gpt-oss-120b] Starting vLLM serve..."
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}"