#!/bin/bash
# fp8-f8-262k-mmp 서빙 스크립트

if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi

vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
