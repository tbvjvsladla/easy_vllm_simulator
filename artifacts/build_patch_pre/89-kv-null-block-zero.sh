#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[89-kv-null-block-zero]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="$HERE${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import sys
import exl3_port_lib as L
from exl3_port_lib import Edit

TAG = sys.argv[1]
M = "# [exl3-port:kv-null-block-zero]"
REL = "vllm/v1/worker/gpu/model_runner.py"

ANCHOR = """        if not profile_only:
            # Lock workspace to prevent resizing during execution. A resize after
            # capture frees the static cuda graph buffer.
            lock_workspace()
"""
E_CALL = Edit(
    "post-capture-null-block-zero",
    ANCHOR,
    f"""        if not profile_only:
            {M} capture dummy forwards write the null block (block 0); clear it before serving
            _exl3port_kv_null_block_zero(self)
            # Lock workspace to prevent resizing during execution. A resize after
            # capture frees the static cuda graph buffer.
            lock_workspace()
""",
)
FOOT_MARK = f"{M} helper"
FOOTER = f'''

{FOOT_MARK}
def _exl3port_kv_null_block_zero(runner) -> None:
    """Zero block 0 (the null block) of every KV-cache tensor after CUDA-graph capture (testlog_26100408)."""
    import os

    if os.environ.get("EXL3PORT_KV_NULL_BLOCK_ZERO", "1") != "1":
        return
    census = os.environ.get("EXL3PORT_KV_NULL_BLOCK_CENSUS", "0") == "1"
    torch.accelerator.synchronize()
    nb = int(runner.kv_cache_config.num_blocks)
    seen, dirty = set(), 0
    for name, layer in runner.compilation_config.static_forward_context.items():
        t = getattr(layer, "kv_cache", None)
        if isinstance(t, (list, tuple)) and t:
            t = t[0]
        if not isinstance(t, torch.Tensor) or t.numel() == 0 or t.shape[0] != nb:
            continue
        key = (t.data_ptr(), tuple(t.shape))
        if key in seen:
            continue
        seen.add(key)
        b0 = t[0]
        if census:
            u0 = b0.contiguous().view(torch.uint8)
            nz = int(u0.count_nonzero())
            dirty += nz > 0
            logger.info("exl3-port: kv null block census %s dtype=%s block0 nonzero_bytes=%d nan_like=%d",
                        name, t.dtype, nz, int(((u0 & 0x7F) == 0x7F).sum()))
        b0.zero_()
    torch.accelerator.synchronize()
    logger.info("exl3-port: kv null block zeroed after capture (tensors=%d%s)", len(seen),
                f", dirty_before={{dirty}}" if census else "")
'''

text = (L.VLLM_SRC / REL).read_text()
for needle in ("def capture_model", "static_forward_context", "kv_cache_config", "logger"):
    if needle not in text:
        L.die(TAG, f"{REL}: needle {needle!r} absent - 0.30.0 model_runner drifted from the audited shape")

report = L.apply_edit_jobs(TAG, [(REL, [E_CALL], FOOTER, FOOT_MARK)])
for rel, out in report:
    print(f"{TAG} {rel}: {', '.join(out)}")

p = L.VLLM_SRC / REL
L.py_compile(TAG, [p])
t = p.read_text()
if t.count("_exl3port_kv_null_block_zero(self)") != 1 or t.count(FOOT_MARK) != 1:
    L.die(TAG, "post-assert: call site or helper count != 1")
n_new = sum(1 for _, out in report for o in out if o.endswith(":applied"))
L.status("applied", f"{n_new} new edits" if n_new else "already present (no-op rerun)")
print(f"{TAG} OK ({n_new} new)")
PY
