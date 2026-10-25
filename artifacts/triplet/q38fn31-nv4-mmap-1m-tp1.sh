#!/bin/bash
# q38fn31-nv4-mmap-1m-tp1 서빙 스크립트 (vllm-recipe-explorer 생성 구조 준용)
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi
if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
