#!/bin/bash
# what: Install the exact optional B12X extra pinned by official vLLM v0.32.0/setup.py:1709.
# why: B12X1.5.1 wrapper API and CUTLASS4.8 match official0.32; operator B12X1.2.3/DSL4.6 is not reused.
# model-trigger: Generic SM120/SM121 B12X optional backend; no model weights.
# plan-ref: docs/plan/plan_26102919_멀티4종_v032_완주.md
# source: https://pypi.org/pypi/b12x/1.5.1/json; wheel SHA pinned below.
set -euo pipefail
python3 -m pip install --no-build-isolation 'https://files.pythonhosted.org/packages/0c/7f/31fbc8311188835a509d97c38f6400071ba5b01a5ca54cb33d7b22dec3fa/b12x-1.5.1-py3-none-any.whl#sha256=8731e23571d8d45db26d78e7699ec0b23e7ba20b52487adf8c1063c8d8b53a3d'
python3 - <<'CHECK'
from importlib.metadata import version
assert version('b12x') == '1.5.1'
print('[06-b12x] exact installed distribution', version('b12x'))
CHECK
python3 -m pip check
if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'applied\tOfficial vLLM0.32 optional B12X1.5.1 exact wheel; pip check passed; GPU kernels unverified\n' > "$EASY_VLLM_PATCH_STATUS"; fi
