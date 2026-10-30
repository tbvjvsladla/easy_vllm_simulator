#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[84-sm12x-block-shim]"

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
M = "# [exl3-port:sm12x-block]"

ATTN_MARK = f"{M} helpers"
ATTN_HELPER = f'''

{ATTN_MARK}
def _exl3port_swa_block_size() -> int:
    """SWA cache page: 64 tokens on SM12x (FlashInfer SM120 decode dispatch), else 32."""
    from vllm.platforms import current_platform

    return 64 if current_platform.is_device_capability_family(120) else 32


def _exl3port_cr_block_size(block_size: int, compress_ratio: int) -> int:
    """64 states per page on SM12x: FlashInfer's SM120 dual-cache prefill and DeepGEMM's
    paged indexer take 64-state pages (DeepGEMM also 32), so a compress-ratio-r cache
    uses r x the token block."""
    from vllm.platforms import current_platform

    if current_platform.is_device_capability_family(120) and int(compress_ratio) > 1:
        return int(block_size) * int(compress_ratio)
    return int(block_size)
'''

ATTN = [
    Edit("main-cache-spec", '''        return MLAAttentionSpec(
            block_size=vllm_config.cache_config.block_size,
            num_kv_heads=1,
            head_size=self.head_dim,
            dtype=torch.uint8 if uses_fp8_ds_mla_layout else self.kv_cache_torch_dtype,
''', f'''        return MLAAttentionSpec(
            block_size=_exl3port_cr_block_size(  {M}
                vllm_config.cache_config.block_size, self.compress_ratio
            ),
            num_kv_heads=1,
            head_size=self.head_dim,
            dtype=torch.uint8 if uses_fp8_ds_mla_layout else self.kv_cache_torch_dtype,
'''),
    Edit("indexer-cache-spec", '''        return MLAAttentionSpec(
            block_size=self.cache_config.block_size,
            num_kv_heads=1,
            head_size=self.head_dim,
            dtype=self.dtype,
''', f'''        return MLAAttentionSpec(
            block_size=_exl3port_cr_block_size(  {M}
                self.cache_config.block_size, self.compress_ratio
            ),
            num_kv_heads=1,
            head_size=self.head_dim,
            dtype=self.dtype,
'''),
    Edit("swa-block-64", '''            backend_cls=self.swa_backend_cls,
            block_size=32,
''', f'''            backend_cls=self.swa_backend_cls,
            block_size=_exl3port_swa_block_size(),  {M}
'''),
]

JOBS = [
    ("vllm/models/deepseek_v41/attention.py", ATTN, ATTN_HELPER, ATTN_MARK),
    ("vllm/v1/attention/backends/mla/indexer.py", [
        Edit("indexer-kernel-blocks",
             "        return [64 if current_platform.is_device_capability_family(90) else 128]\n",
             f"        {M} SM12x: kernel block == manager block (64 tokens for ratio-1 caches, 128 for ratio-2) so DeepGEMM's\n"
             "        # paged MQA logits always see 64 states per page.\n"
             "        if current_platform.is_device_capability_family(120):\n"
             "            return [64, 128]\n"
             "        return [64] if current_platform.is_device_capability_family(90) else [128]\n"),
    ], None, None),
    ("vllm/models/deepseek_v41/nvidia/flashinfer_sparse.py", [
        Edit("flashinfer-dsv41-kernel-blocks", '''    def get_supported_kernel_block_sizes(kv_cache_spec=None) -> list[int | MultipleOf]:
        return [128]
''', f'''    def get_supported_kernel_block_sizes(kv_cache_spec=None) -> list[int | MultipleOf]:
        {M} SM12x also runs on 64-token pages (--block-size 64)
        from vllm.platforms import current_platform

        if current_platform.is_device_capability_family(120):
            return [64, 128]
        return [128]
''')], None, None),
]

results, touched = [], []
for rel, out in L.apply_edit_jobs(TAG, [(rel, edits, footer, fmark) for rel, edits, footer, fmark in JOBS]):
    print(f"{TAG} {rel}: {', '.join(out)}")
    results += out
    touched.append(L.VLLM_SRC / rel)

L.py_compile(TAG, touched)
# post-assert: 모든 편집 존재 + 이식 대상이 아닌 persistent_topk 파일은 건드리지 않았다.
for rel, edits, footer, fmark in JOBS:
    t = (L.VLLM_SRC / rel).read_text()
    for e in edits:
        if e.new not in t:
            L.die(TAG, f"{rel}: post-assert missing edit {e.label}")
    if fmark and fmark not in t:
        L.die(TAG, f"{rel}: post-assert missing helper footer")
for rel in ("vllm/model_executor/layers/sparse_attn_indexer.py",):
    p = L.VLLM_SRC / rel
    if p.is_file() and "[exl3-port" in p.read_text():
        L.die(TAG, f"{rel}: must stay untouched (persistent_topk is a runtime flag now)")
n_new = sum(1 for r in results if r.endswith(":applied"))
L.status("applied", f"{n_new} new edits/footers" if n_new else "already present (no-op rerun)")
print(f"{TAG} OK - {len(results)} items ({n_new} new)")
PY
