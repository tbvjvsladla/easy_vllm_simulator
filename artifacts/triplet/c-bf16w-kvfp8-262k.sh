#!/bin/bash
# c-bf16w-kvfp8-262k 서빙 스크립트 (vllm-recipe-explorer 생성, gpt-oss-20b-normal.sh 구조 동일)
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, TIKTOKEN_ENABLED

# TIKTOKEN 환경변수 설정
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

# 모델구동 런타임 패치 arming (configs/${CONFIG_FILE}_patch.py 존재 시; 메인 저작 arm_patch.sh)
if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi

# attention backend 고정
export VLLM_ATTENTION_BACKEND=FLASHINFER

# vllm serve 실행
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME" \
    --enable-auto-tool-choice \
    --tool-call-parser qwen3_coder \
    --reasoning-parser qwen3
