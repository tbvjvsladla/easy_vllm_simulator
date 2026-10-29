#!/bin/bash
# Solar official0.32 plugin candidate; explicitly synchronous and uncached prefix.
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi
if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME" \
    --no-async-scheduling \
    --no-enable-prefix-caching
