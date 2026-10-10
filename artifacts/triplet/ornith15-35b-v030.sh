#!/bin/bash
# ornith15-35b-v030 서빙 스크립트 (서브 자율 저작)
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME

if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi

# 파서·KV·ctx 는 yaml 이 소유(--config). 여기서는 served-model-name 만 주입.
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
