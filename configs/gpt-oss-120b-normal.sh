#!/bin/bash
# gpt-oss-120b 기본 서빙 스크립트
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, TIKTOKEN_ENABLED

# TIKTOKEN 환경변수 설정
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

# vllm serve 실행
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
