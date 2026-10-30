#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[81-exl3-model-wiring]"

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
M = "# [exl3-port:wiring]"

LM_HEAD_OLD = """            self.lm_head = ParallelLMHead(
                config.vocab_size,
                config.hidden_size,
                prefix=maybe_prefix(prefix, "lm_head"),
            )
"""
LM_HEAD_NEW = f"""            self.lm_head = ParallelLMHead(
                config.vocab_size,
                config.hidden_size,
                quant_config=vllm_config.quant_config,  {M}
                prefix=maybe_prefix(prefix, "lm_head"),
            )
"""

REGEX_OLD = """        orig_to_new_regex=scale_regex,
"""
REGEX_NEW = f"""        orig_to_new_regex={{
            **scale_regex,
            {M} MXFP8 lm_head keys. Regexes run BEFORE the suffix pass: route the weight back to
            # `head.weight` so the stock suffix rule renames it (a suffix result ending in `head.weight` would be re-fired).
            re.compile(r"^lm_head\\.weight$"): "head.weight",
            re.compile(r"^lm_head\\.weight_scale$"): "lm_head.weight_scale",
        }},
"""

VL_PREFIX_OLD = """        "mtp.": "language_model.model.mtp.",
    }
"""
VL_PREFIX_NEW = f"""        "mtp.": "language_model.model.mtp.",
        "lm_head.": "language_model.lm_head.",  {M} MXFP8 lm_head scale key -> VL root namespace
    }}
"""

JOBS = [
    ("vllm/models/deepseek_v41/nvidia/model.py",
     [Edit("lm_head-ctor", LM_HEAD_OLD, LM_HEAD_NEW), Edit("lm_head-mapper-regex", REGEX_OLD, REGEX_NEW)]),
    ("vllm/models/deepseek_v41/nvidia/vl_model.py", [Edit("lm_head-vl-prefix", VL_PREFIX_OLD, VL_PREFIX_NEW)]),
]

results, touched = [], []
for rel, out in L.apply_edit_jobs(TAG, [(rel, edits, None, None) for rel, edits in JOBS]):  # two-phase: all anchors verified before any write
    print(f"{TAG} {rel}: {', '.join(out)}")
    results += out
    touched.append(L.VLLM_SRC / rel)

L.py_compile(TAG, touched)
# post-assert: 모든 편집 마커가 정확히 자리에 있다
for rel, edits in JOBS:
    t = (L.VLLM_SRC / rel).read_text()
    for e in edits:
        if e.new not in t:
            L.die(TAG, f"{rel}: post-assert missing edit {e.label}")
n_new = sum(1 for r in results if r.endswith(":applied"))
L.status("applied", f"{len(results)} edits ({n_new} newly applied)" if n_new else f"{len(results)} edits already present (no-op rerun)")
print(f"{TAG} OK - {len(results)} edits across {len(JOBS)} files ({n_new} new)")
PY
