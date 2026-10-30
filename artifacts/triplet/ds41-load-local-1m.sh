#!/bin/bash
set -euo pipefail
if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi
printf '[load-measure] vllm_exec_start_unix_ns=%s\n' "$(date +%s%N)"
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" --served-model-name "$SERVING_MODEL_NAME" --no-async-scheduling
