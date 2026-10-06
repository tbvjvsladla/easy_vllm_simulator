#!/bin/bash
# 84-sm12x-block-shim.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · SM12x(GB10) 64-토큰 커널 블록 shim
#
# what : Mia 오버레이 patch_sm120_block64.py 의 **v0.30.0 재앵커 등가물**(선별 · 아래 §버린 것). DeepSeek-V4.1 sparse-MLA 가 SM12x 에서
#        64-상태 페이지로 돌게 한다:
#          attention.py   : (a) 압축 KV 캐시 spec / (b) 인덱서 캐시 spec 의 block_size = cache block x compress_ratio(SM12x, ratio>1)
#                           (c) SWA 캐시 페이지 32 -> 64 (SM12x)   (d) 비전 확장 SWA 창 끄기(SM12x = 텍스트 전용 128 창)
#          sparse_swa.py  : (d) 의 메타데이터 빌더 쪽(prefill index 폭 = window only)
#          indexer.py     : DeepseekV41IndexerBackend 커널 블록 [64,128] (SM12x)
#          nvidia/flashinfer_sparse.py : FLASHINFER_MLA_SPARSE_DSV41 커널 블록 [64,128] (SM12x)
#          indexer.py(선택): 인덱서 prefill gather workspace 의 `max_model_len*40` 을 env DSV41_INDEXER_PREFILL_FACTOR 로 조정 가능하게
#                            (미설정 = stock 40 그대로 · Mia 기본(min(40,max_num_seqs))과 다르다 - 보수적 옵트인)
# why  : 이 이미지의 DeepGEMM _C.so 는 paged MQA logits 커널에서 `block_kv == 32 or block_kv == 64` 를 assert 한다(delta report §2, 문자열 실측).
#        0.30.0 의 DSV41 인덱서 백엔드는 [64 if sm90 else 128] · FlashInfer DSV41 sparse 백엔드는 [128] 만 선언하므로 SM12x 에서 ratio-1 인덱서 레이어가
#        128-상태 페이지가 되어 첫 디코드에서 assert 로 죽을 가능성이 높다(Mia/sfxnz 가 0909 에서 독립적으로 같은 벽을 만났다). `--block-size 64` 와 짝이다.
#        FlashInfer SM120 이중 캐시 prefill 은 64-상태 페이지만 받으므로(Mia 주석) ratio-r 캐시는 r x 토큰 블록을 쓴다.
# 경고: 0.30.0 은 이미 SM120 DSV4.1 FlashInfer 경로(DeepseekV4FlashInferSM120Attention · FLASHINFER_MLA_SPARSE_SM120)를 갖고 있다. 이 shim 이 stock 0.30.0 에서
#        **필요한지**(= 부분적으로 이미 upstream 이 해소했는지)는 코드 독해로 판정할 수 없다. 이 스크립트는 Mia 가 0909 에서 검증한 논리를 그대로 이식한 것이며
#        UNTESTED(GPU 부팅 필요)다. 부팅이 stock 으로도 되면 이 스크립트를 걷어내는 쪽이 더 단순하다.
# §버린 것: persistent_topk 스탠자 2곳(0.30.0 은 indexer_topk.py + 런타임 노브 `--kernel-config` sparse_indexer_topk_backend=per_row 로 대체) ·
#        mixed-warmup 스킵 · DSV41_ENGRAM_DISABLE 디버그 우회 · aux 스트림 직렬화(-> 85 로 분리).
# model-trigger : DeepseekV41 on SM12x(GB10). 비-SM12x 에서는 헬퍼가 stock 값을 돌려준다(런타임 분기).
# provenance : 편집 원본 = Mia 커밋 overlay/patch_sm120_block64.py(AGPL-3.0, 파일은 받지 않음 · 편집문 재작성). SWA block_size 앵커는 0.30.0 의
#        ctor 에 packed_* 인자가 생겨 재작성했다(delta report §2 (i)).
# idempotent/fail-loud : 편집마다 앵커 정확히 1회. 재실행 no-op.
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


def _exl3port_text_only_swa() -> bool:
    """FlashInfer SM120 has no prefill kernel for the vision-widened SWA window; on SM12x
    keep the text window (serve with --language-model-only or accept text-only SWA)."""
    from vllm.platforms import current_platform

    return bool(current_platform.is_device_capability_family(120))
'''

SWA_MARK = f"{M} helper"
SWA_HELPER = f'''

{SWA_MARK}
def _exl3port_text_only_swa() -> bool:
    from vllm.platforms import current_platform

    return bool(current_platform.is_device_capability_family(120))
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
    Edit("text-only-attn", '''        self.max_image_tokens = (
            getattr(config, "vision_max_n_token", 0)
            if getattr(config, "vision_n_layers", 0) > 0
            else 0
        )
''', f'''        self.max_image_tokens = (  {M} text-only SWA window on SM12x
            0
            if _exl3port_text_only_swa()
            else getattr(config, "vision_max_n_token", 0)
            if getattr(config, "vision_n_layers", 0) > 0
            else 0
        )
'''),
    Edit("swa-block-64", '''            backend_cls=self.swa_backend_cls,
            block_size=32,
''', f'''            backend_cls=self.swa_backend_cls,
            block_size=_exl3port_swa_block_size(),  {M}
'''),
]

JOBS = [
    ("vllm/models/deepseek_v41/attention.py", ATTN, ATTN_HELPER, ATTN_MARK),
    ("vllm/v1/attention/backends/mla/sparse_swa.py", [
        Edit("text-only-swa-meta", '''        self.max_image_tokens = (
            getattr(hf_config, "vision_max_n_token", 0)
            if getattr(hf_config, "vision_n_layers", 0) > 0
            else 0
        )
''', f'''        self.max_image_tokens = (  {M} text-only SWA window on SM12x
            0
            if _exl3port_text_only_swa()
            else getattr(hf_config, "vision_max_n_token", 0)
            if getattr(hf_config, "vision_n_layers", 0) > 0
            else 0
        )
''')], SWA_HELPER, SWA_MARK),
    ("vllm/v1/attention/backends/mla/indexer.py", [
        Edit("indexer-kernel-blocks",
             "        return [64 if current_platform.is_device_capability_family(90) else 128]\n",
             f"        {M} SM12x: kernel block == manager block (64 tokens for ratio-1 caches, 128 for ratio-2) so DeepGEMM's\n"
             "        # paged MQA logits always see 64 states per page.\n"
             "        if current_platform.is_device_capability_family(120):\n"
             "            return [64, 128]\n"
             "        return [64] if current_platform.is_device_capability_family(90) else [128]\n"),
        Edit("indexer-workspace-factor", '''    #   40 * 163840 * 132 = 865075200 bytes = 825 MB
    return max_model_len * 40
''', f'''    #   40 * 163840 * 132 = 865075200 bytes = 825 MB
    {M} opt-in: DSV41_INDEXER_PREFILL_FACTOR=<n> sizes the gather workspace max_model_len*n (unset = stock 40)
    _factor = __import__("os").environ.get("DSV41_INDEXER_PREFILL_FACTOR", "")
    if _factor.strip():
        return max_model_len * max(1, int(_factor))
    return max_model_len * 40
''')], None, None),
    ("vllm/models/deepseek_v41/nvidia/flashinfer_sparse.py", [
        Edit("flashinfer-dsv41-kernel-blocks", '''    def get_supported_kernel_block_sizes() -> list[int | MultipleOf]:
        return [128]
''', f'''    def get_supported_kernel_block_sizes() -> list[int | MultipleOf]:
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
