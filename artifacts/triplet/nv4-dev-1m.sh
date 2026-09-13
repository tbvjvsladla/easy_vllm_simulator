#!/bin/bash
# nv4-dev-1m 서빙 스크립트 (vllm-recipe-explorer 생성, gpt-oss-20b-normal.sh 구조 동일)
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, TIKTOKEN_ENABLED

# TIKTOKEN 자산 경로 미선언(manifest.tiktoken_host_path 비어 있음) — export 생략

# 모델구동 런타임 패치 arming (configs/${CONFIG_FILE}_patch.py 존재 시; 메인 저작 arm_patch.sh)
if [ -f <manifest.nodes[].work_dir>/output/single/configs/arm_patch.sh ]; then source <manifest.nodes[].work_dir>/output/single/configs/arm_patch.sh; fi

# vllm serve 실행
vllm serve --config "<manifest.nodes[].work_dir>/output/single/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
