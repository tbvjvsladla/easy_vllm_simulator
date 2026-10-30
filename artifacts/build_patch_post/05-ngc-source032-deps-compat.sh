#!/bin/bash
# what: Reuse measured NGC26.07+official v0.32.0 dependency intersection from prior successful sub repair.
# why: NGC datasets requires huggingface-hub<2 while vLLM requires >=1.31; NGC triton-kernels requires pytest.
# model-trigger: generic shared runtime dependency consistency; no model weights.
# plan-ref: docs/plan/plan_26102919_멀티4종_v032_완주.md
# evidence: docs/simlog/single8_gptoss120b_20261028T135226Z/build_dependency_repair/{pip-dryrun.json,probe.txt}
set -euo pipefail
python3 -m pip install 'huggingface-hub==1.33.0' 'pytest==9.2.0'
python3 -m pip check
if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'applied\tNGC26.07 source032 measured hub/pytest intersection reused; pip check passed\n' > "$EASY_VLLM_PATCH_STATUS"; fi
