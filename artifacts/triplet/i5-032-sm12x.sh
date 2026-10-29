#!/bin/bash
set -e
if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" --served-model-name "$SERVING_MODEL_NAME" --no-enable-prefix-caching --no-async-scheduling
