#!/bin/bash
# Qwen3.5-35B-A3B 기본 서빙 스크립트
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, TIKTOKEN_ENABLED

# ── vLLM qwen3_5_moe.py 패치 (ignore_keys_at_rope_validation: list → set) ──
# vLLM 0.18.x에서 transformers의 validate_rope가 set을 기대하나 list로 정의된 버그 수정
# grep으로 미적용 상태인지 확인 후 패치 (중복 적용 방지)
PATCH_TARGET="/usr/local/lib/python3.12/dist-packages/vllm/transformers_utils/configs/qwen3_5_moe.py"
if [ -f "$PATCH_TARGET" ] && grep -q 'validation"\] = \[' "$PATCH_TARGET"; then
    python -c "
p='$PATCH_TARGET'
t=open(p).read()
t=t.replace('validation\"] = [','validation\"] = {',1)
t=t.replace('\"mrope_interleaved\",\n        ]','\"mrope_interleaved\",\n        }',1)
open(p,'w').write(t)
"
    echo "[patch] qwen3_5_moe.py: ignore_keys_at_rope_validation list -> set"
else
    echo "[patch] qwen3_5_moe.py: already patched or not found, skipping"
fi

# ── TIKTOKEN 환경변수 설정 ──
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

# ── vllm serve 실행 ──
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
