#!/bin/bash
# 89-kv-null-block-zero.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · CUDA graph 캡처 후 KV null 블록(block 0)을 0 으로
#
# what : vllm/v1/worker/gpu/model_runner.py 의 캡처 루틴 끝(`if not profile_only:` + lock_workspace 앞)에 한 블록을 넣는다:
#        캡처가 끝나면 static_forward_context 의 KV 텐서마다 block 0(null 블록)을 zero_() 한다.
#        EXL3PORT_KV_NULL_BLOCK_ZERO=1(기본) · =0 이면 끔. EXL3PORT_KV_NULL_BLOCK_CENSUS=1 이면 지우기 전 block 0 의
#        nonzero/NaN-패턴 바이트 수를 층별로 로그한다(진단).
# why  : graph 모드는 크기별 캡처 전에 eager 더미 forward 를 돈다(cudagraph_utils.py:436). 더미 입력의 block table 은 0(null 블록)이라
#        DSv4 압축기 링·SWA 가 block 0 에 쓰고, 우리 이식의 더미 행 출력이 쓰레기/NaN 이면 그대로 남는다. null 블록은 다시 0 이 되지 않고
#        sparse/paged 커널이 마스킹 슬롯을 거기서 읽어 0×NaN=NaN 이 모든 요청을 오염시켰다(eager 정상 · graph 전부 NaN).
#        실측 = testlog_26100408 (census: layers.0.attn.swa_cache block0 8,155 B · NaN-패턴 42 · layers.8.attn 4,086 B · 14 → zero 후 SMOKE PASS).
# 근거 진단 : track_i4/diagnosis.md(simlog 26100408) · 진단 이미지 v5probe(EXP_KV_ZERO=block0)를 상시 동작으로 정식화한 것.
# model-trigger : 캡처를 하는 모든 모델(그래프 모드). eager(--enforce-eager)는 캡처가 없으므로 no-op. block 0 은 정의상 쓰이지 않는 자리라
#        stock 경로에서도 지우는 것이 무해하다(값이 0 이 될 뿐).
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
