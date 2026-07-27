#!/bin/bash
# exaone45-33b (LG/EXAONE-4.5-33B) — 멀티모달(VL) 모델. serve_runner.sh(master)가 source.
# 이미지 = easy-vllm:0.23.0-cu132-aarch64-source-mmshim (.env IMAGE_TAG): 0.23.0 + EXAONE-4.5 image/video
#   processor compat shim(.pth, lazy meta-path). transformers 5.12.1 의 exaone4_5→Qwen2VL alias 와 정합 —
#   모델파일이 선언한 Exaone4_5_Image/VideoProcessor 이름을 Qwen2VL 로 매핑(engine + ray workers 모두 자동).
# 외부검색 근거(참조-그라운디드): HF LGAI-EXAONE/EXAONE-4.5-33B + transformers/vLLM exaone4_5 fork(lkm2835/nuxlear).
# NO tiktoken(BPE) · NO trust-remote-code. plain completion(분산 serve 검증).

if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[exaone45-33b] Env vars loaded, skipping serve (slave mode)"
    return 0
fi

# --limit-mm-per-prompt 0: 텍스트-only serve. EXAONE-4.5 의 vLLM 비전타워(Exaone4_5_VisionBlock.forward)는
#   sequence_lengths kwarg 미수용(vLLM-side 비전 impl 버그) → 멀티모달 프로파일링서 TypeError. image/video=0 으로
#   멀티모달 경로(더미 프로파일+비전 forward) 전면 회피 → 텍스트 서빙만(가중치는 로드되나 비전 미forward).
echo "[exaone45-33b] Starting vLLM serve (TP=2, Ray, kv auto, enforce-eager; mmshim .pth; text-only mm=0)..."
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}" \
    --limit-mm-per-prompt '{"image":0,"video":0}'
