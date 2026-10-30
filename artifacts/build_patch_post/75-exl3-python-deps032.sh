#!/bin/bash
# Official0.32 EXL3 variant dependency closure, campaign26102919.
# Trigger: actual final pipcheck found exllamav3 requires flash-linear-attention/marisa-trie.
# PyPI wheel metadata and deps2 dryrun: exactly FLA0.6.0/fla-core0.6.0/marisa1.4.1;
# no core vLLM/torch/CUDA/transformers replacement. Exact wheel SHA pins below.
set -euo pipefail
if [ "${SM12X_PORT:-0}" != 1 ]; then
  [ -z "${EASY_VLLM_PATCH_STATUS:-}" ] || printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"
  exit 0
fi
python3 -m pip install \
  'https://files.pythonhosted.org/packages/3c/c1/ad271e49528b06c7032c35e6dca85592f96ef8390e6ce1b803d6b23eae0a/flash_linear_attention-0.6.0-py3-none-any.whl#sha256=6dec9735f8454523c13553622e98aff30152dde6586b4ac7d88010fbb7a06b5f' \
  'https://files.pythonhosted.org/packages/23/08/a1a16725117cddded90bb11afed4b31f23d97f7f554775e66af998818ef9/fla_core-0.6.0-py3-none-any.whl#sha256=b0708b70e0b119cd58a0a5146ea56cb175cef6ed969d8ac60267e3e077ce3ebd' \
  'https://files.pythonhosted.org/packages/15/0c/376e21c62bd0e658a5e9f6b8912f3116591778c639857ad374c7639ceebe/marisa_trie-1.4.1-cp312-cp312-manylinux_2_24_aarch64.manylinux_2_28_aarch64.whl#sha256=7bed50d39ff1391a67b9383a7f3c458a1a0cb40fe8dd16952f813fbf8939eeff'
python3 - <<'PY_CHECK'
from importlib.metadata import version
for package, expected in [('flash-linear-attention','0.6.0'),('fla-core','0.6.0'),('marisa-trie','1.4.1')]:
    assert version(package)==expected, (package,version(package))
import marisa_trie
assert 'exl3' in marisa_trie.Trie(['exl3'])
print('[75-exl3-deps] exact metadata + marisa tiny operation PASS')
PY_CHECK
python3 -m pip check
[ -z "${EASY_VLLM_PATCH_STATUS:-}" ] || printf 'applied\tFLA0.6.0/fla-core0.6.0/marisa1.4.1 closure; pinned wheel SHA\n' > "$EASY_VLLM_PATCH_STATUS"
