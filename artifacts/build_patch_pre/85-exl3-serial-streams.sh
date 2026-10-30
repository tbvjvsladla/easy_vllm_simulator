#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[85-exl3-serial-streams]"

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
M = "# [exl3-port:serial-streams]"
OLD = "        aux_stream_list = [torch.cuda.Stream() for _ in range(3)]\n"
NEW = f'''        aux_stream_list = (  {M} EXL3 kernels share one per-device lock buffer
            None
            if __import__("os").environ.get("DSV41_EXL3_SERIAL_STREAMS", "1") == "1"
            and getattr(vllm_config.quant_config, "get_name", lambda: "")() == "exl3"
            else [torch.cuda.Stream() for _ in range(3)]
        )
'''
JOBS = [
    ("vllm/models/deepseek_v41/nvidia/model.py", [Edit("aux-streams-model", OLD, NEW)]),
    ("vllm/models/deepseek_v4/nvidia/mtp.py", [Edit("aux-streams-mtp", OLD, NEW)]),
]
results, touched = [], []
for rel, out in L.apply_edit_jobs(TAG, [(rel, edits, None, None) for rel, edits in JOBS]):
    print(f"{TAG} {rel}: {', '.join(out)}")
    results += out
    touched.append(L.VLLM_SRC / rel)
L.py_compile(TAG, touched)
for rel, edits in JOBS:
    t = (L.VLLM_SRC / rel).read_text()
    if M not in t or "aux_stream_list = [torch.cuda.Stream() for _ in range(3)]" in t:
        L.die(TAG, f"{rel}: post-assert failed (marker missing or stock aux-stream line still present)")
n_new = sum(1 for r in results if r.endswith(":applied"))
L.status("applied", f"{n_new} new edits" if n_new else "already present (no-op rerun)")
print(f"{TAG} OK ({n_new} new)")
PY
