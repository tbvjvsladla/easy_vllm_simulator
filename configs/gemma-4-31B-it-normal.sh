#!/bin/bash
# gemma-4-31B-it 기본 서빙 스크립트
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME

# vllm serve 실행
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"