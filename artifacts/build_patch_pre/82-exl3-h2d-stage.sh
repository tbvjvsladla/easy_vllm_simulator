#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[82-exl3-h2d-stage]"

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
MARK = "# [exl3-port:h2d-stage]"

HELPER_MARK = f"{MARK} helper"
SHIM_MARK = f"{MARK} shim (lazy import: weight_utils imports the layers package)"

HELPER = f'''
{HELPER_MARK}
_EXL3PORT_PIN_POOL: dict = {{}}


def exl3port_h2d_copy(dest, src):
    """dest.copy_(src) that never DMAs from a (mmap-backed) CPU tensor."""
    if src.device.type != "cpu" or dest.device.type == "cpu" or src.numel() == 0:
        dest.copy_(src)
        return
    if src.dtype != dest.dtype:
        src = src.to(dtype=dest.dtype)
    numel = src.numel()
    buf = _EXL3PORT_PIN_POOL.get(src.dtype)
    if buf is None or buf.numel() < numel:
        want = max(numel, 1 << 20, 0 if buf is None else buf.numel() * 2)
        buf = torch.empty(want, dtype=src.dtype, device="cpu", pin_memory=True)
        _EXL3PORT_PIN_POOL[src.dtype] = buf
    stage = buf[:numel].view(src.shape)
    stage.copy_(src)
    dest.copy_(stage)
'''

SHIM = f'''
{SHIM_MARK}
def _exl3port_h2d_copy(dest, src):
    from vllm.model_executor.model_loader.weight_utils import exl3port_h2d_copy

    return exl3port_h2d_copy(dest, src)
'''

JOBS = [
    ("vllm/model_executor/model_loader/weight_utils.py",
     [Edit("dwl-view", "            param.data.copy_(loaded_weight.view(param.shape))\n",
           "            exl3port_h2d_copy(param.data, loaded_weight.view(param.shape))\n"),
      Edit("dwl-plain", "            param.data.copy_(loaded_weight)\n    except Exception:\n",
           "            exl3port_h2d_copy(param.data, loaded_weight)\n    except Exception:\n")],
     HELPER),
    ("vllm/model_executor/layers/linear.py",
     [Edit("lin-param.data", "\n        param.data.copy_(loaded_weight)\n", "\n        _exl3port_h2d_copy(param.data, loaded_weight)\n"),
      Edit("lin-param_data-8", "\n        param_data.copy_(loaded_weight)\n", "\n        _exl3port_h2d_copy(param_data, loaded_weight)\n", count=5),
      Edit("lin-param_data-16", "\n                param_data.copy_(loaded_weight)\n", "\n                _exl3port_h2d_copy(param_data, loaded_weight)\n", count=2)],
     SHIM),
    ("vllm/model_executor/layers/vocab_parallel_embedding.py",
     [Edit("vpe-param.data", "            param.data.copy_(loaded_weight)\n", "            _exl3port_h2d_copy(param.data, loaded_weight)\n"),
      Edit("vpe-slice", "        param[: loaded_weight.shape[0]].data.copy_(loaded_weight)\n",
           "        _exl3port_h2d_copy(param[: loaded_weight.shape[0]].data, loaded_weight)\n")],
     SHIM),
    ("vllm/models/deepseek_v41/nvidia/model.py",
     [Edit("dsv41-attn_sink", "                    params_dict[name][:n].copy_(narrow_weight)\n",
           "                    _exl3port_h2d_copy(params_dict[name][:n], narrow_weight)\n")],
     SHIM),
    ("vllm/models/deepseek_v41/nvidia/dspark.py",
     [Edit("dspark-narrow", "                    params_dict[name][: narrow.shape[0]].copy_(narrow)\n",
           "                    _exl3port_h2d_copy(params_dict[name][: narrow.shape[0]], narrow)\n")],
     SHIM),
]

results, touched = [], []
for rel, out in L.apply_edit_jobs(TAG, [(rel, edits, footer, (HELPER_MARK if footer is HELPER else SHIM_MARK)) for rel, edits, footer in JOBS]):
    print(f"{TAG} {rel}: {', '.join(out)}")
    results += out
    touched.append(L.VLLM_SRC / rel)

L.py_compile(TAG, touched)
# post-assert: 원본 `.copy_(loaded_weight)` 직접 호출이 대상 지점에서 사라졌고 헬퍼가 정의됐다.
wu = (L.VLLM_SRC / "vllm/model_executor/model_loader/weight_utils.py").read_text()
assert "def exl3port_h2d_copy(" in wu, "helper missing"
lin = (L.VLLM_SRC / "vllm/model_executor/layers/linear.py").read_text()
assert lin.count("_exl3port_h2d_copy(") >= 8 + 1, "linear.py shim call count"
assert "def _exl3port_h2d_copy(" in lin
n_new = sum(1 for r in results if r.endswith(":applied"))
L.status("applied", f"{n_new} new edits/footers" if n_new else "already present (no-op rerun)")
print(f"{TAG} OK - {len(results)} items across {len(JOBS)} files ({n_new} new)")
PY
