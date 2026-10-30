#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[87-engram-embed-skip]"

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
M = "# [exl3-port:engram-embed-skip]"

backend = L.VLLM_SRC / "vllm/models/deepseek_v41/common/engram_file_backend.py"
if not backend.is_file() or "def _configured_table_dir()" not in backend.read_text():
    L.die(TAG, "83-engram-file-backend must run first (nvidia/engram_file_backend.py with _configured_table_dir is missing)")

OLD = '''    belong to the local rank and should be skipped during loading."""
    if local_expert_ids is None:
        return False
'''
NEW = f'''    belong to the local rank and should be skipped during loading."""
    if _exl3port_skip_engram_embed(weight_name):  {M}
        return True
    if local_expert_ids is None:
        return False
'''
FOOT_MARK = f"{M} footer"
FOOTER = f'''
{FOOT_MARK}
_EXL3PORT_ENGRAM_EMBED_RE = re.compile(r"(?:^|\\.)engram\\.embed\\.(?:weight|scale)$")


def _exl3port_skip_engram_embed(weight_name: str) -> bool:
    """True for the (huge) Engram embedding tables when the file-backed Engram serves them from the checkpoint shards.

    Stock behaviour (False -> the tensor is read like any other) when no Engram table dir is configured. A broken import of the
    backend propagates: reading ~94 GiB per shard by accident is worse than a loud failure.
    """
    if _EXL3PORT_ENGRAM_EMBED_RE.search(weight_name) is None:
        return False
    from vllm.models.deepseek_v41.common.engram_file_backend import _configured_table_dir

    table_dir = _configured_table_dir()
    if not table_dir:
        return False
    from vllm.logger import init_logger

    init_logger(__name__).info_once(
        "exl3-port: file-backed Engram active (%s) - skipping *.engram.embed.* tensors in the weight iterator", table_dir
    )
    return True
'''
rel = "vllm/model_executor/model_loader/ep_weight_filter.py"
out = L.apply_edits(TAG, rel, [Edit("should_skip_weight-guard", OLD, NEW)], footer=FOOTER, footer_marker=FOOT_MARK)
print(f"{TAG} {rel}: {out}")
L.py_compile(TAG, [L.VLLM_SRC / rel])
t = (L.VLLM_SRC / rel).read_text()
if t.count(FOOT_MARK) != 1 or t.count("_exl3port_skip_engram_embed(weight_name)") != 1:
    L.die(TAG, "post-assert: guard/footer count mismatch")
applied = any(o.endswith(":applied") for o in out)
L.status("applied", "engram embed skip installed" if applied else "already present (no-op rerun)")
print(f"{TAG} OK")
PY

python3 -c "
import sys; sys.path.insert(0, '$HERE')
import exl3_port_lib as L; L.cleanup_cache(); print('$TAG fetch cache cleaned')"
