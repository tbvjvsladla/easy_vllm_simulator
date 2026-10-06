#!/bin/bash
# 85-exl3-serial-streams.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · EXL3 커널 동시 실행 데드락 방지
#
# what : DeepSeek-V4.1 CUDA 모델/MTP 가 만드는 보조 CUDA 스트림 3개(`aux_stream_list`)를 EXL3 양자화일 때 만들지 않는다(None = 직렬 경로).
#          nvidia/model.py (DeepseekV41 본체) · deepseek_v4/nvidia/mtp.py (MTP 스택)
#        env DSV41_EXL3_SERIAL_STREAMS=0 이면 stock 동작(보조 스트림 사용)으로 되돌린다(기본 1).
# why  : exllamav3 의 trellis 커널(exl3_gemm split-K 타일 락, exl3_moe 배리어)은 **디바이스당 락 버퍼 1개**를 공유한다. 서로 다른 스트림에서 두 EXL3 커널이 동시에
#        돌면 락이 깨져 영원히 spin 한다. 전부 EXL3 인 체크포인트에서 보조 스트림 GEMM(작은 배치만: VLLM_MULTI_STREAM_GEMM_TOKEN_THRESHOLD)이 매 부팅 데드락을
#        일으켰다(Mia: 2026-09-12 cuda-gdb 로 두 exl3_gemm_kernel grid Active · thread 0 barrier_acquire spin). 공유 전문가 스트림은 런타임 env
#        VLLM_DISABLE_SHARED_EXPERTS_STREAM=1 로 끈다(0.30.0 에 env 존재 · 레시피 몫).
# model-trigger : DeepseekV41 + --quantization exl3. 조건부(quant_config.get_name()=="exl3") - 같은 이미지의 비-EXL3 모델은 stock 병렬 경로 유지.
# provenance : Mia 오버레이 patch_sm120_block64.py 의 aux-stream 2 job 에서 파생(편집문 재작성 · 조건을 EXL3 한정으로 좁힘). AGPL-3.0(provenance 참조).
# ⚠ UNTESTED(GPU): 다른 곳에 EXL3 커널을 별도 스트림에서 부르는 경로가 0.30.0 에 더 있는지(드래프트·MoE 보조 스트림 등)는 부팅으로만 확인된다.
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
