#!/bin/bash
# solar31-nv4-1m-tp2 서빙 스크립트 (vllm-recipe-explorer 생성 구조 준용)
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi
if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME" \
    --no-async-scheduling   # yaml 의 `async-scheduling: false` 는 설정 파서가 불리언 false 를 버려 무효 — V2 logits processor 는 동기 스케줄링 전제
