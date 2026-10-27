#!/bin/bash
# granite42-30b-v030 서빙 스크립트 (b-kvfp8.sh 구조 준용) — 파서 인자는 모델 카드 권장(플러그인은 read-only NAS 마운트 경로)
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi
if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME" \
    --reasoning-parser granite_thinking_parser \
    --reasoning-parser-plugin /app/models/IBM/Granite-4.2/granite-4.2-30b/granite_thinking_parser.py \
    --tool-call-parser qwen3_coder \
    --enable-auto-tool-choice
