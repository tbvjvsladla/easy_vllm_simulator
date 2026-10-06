#!/bin/bash
# 87-engram-embed-skip.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · 파일 기반 Engram 일 때 `*.engram.embed.{weight,scale}` 텐서를 가중치 이터레이터에서 건너뛴다 (round 2 신규 · 마지막 PRE 스크립트)
#
# what : vllm/model_executor/model_loader/ep_weight_filter.py 의 `should_skip_weight()` 맨 앞에 가드를 넣고, 판정 헬퍼를 파일 끝에 덧붙인다.
#          `layers.N.engram.embed.weight|scale`(팩 A: 레이어당 ~94 GiB x 2 = 189 GiB) 이고 **파일 기반 Engram 이 활성**(engram_file_backend._configured_table_dir() 비어있지 않음 =
#          `--hf-overrides '{"engram_table_dir": ...}'` 또는 env DSV41_SOURCE/ENGRAM_MOUNT)이면 True(건너뜀). 비활성이면 stock 동작(False 로 통과 -> 기존 로드 경로).
# why  : safetensors_weights_iterator(weight_utils.py:962/979/997)는 should_skip_weight 가 False 인 모든 키에 대해 `f.get_tensor(name)` 을 호출한다. 파일 기반 Engram 의 모듈 파라미터는
#        0-행 더미라 이 텐서가 들어갈 곳이 없고, 읽는 것 자체가 샤드당 ~94 GiB 의 mmap 읽기 + page cache 팽창(GB10 UMA: 호스트 RAM = GPU 메모리)이라 치명적이다.
#        should_skip_weight 는 EP 필터가 꺼져 있어도(local_expert_ids=None) 호출되므로 거기가 유일한 삽입점이다(전 이터레이터 모드 중 eager/torchao/lazy 3곳이 호출).
# 한계(문서화): `enable_multithread_load`(multi_thread_safetensors_weights_iterator)·`--load-format fastsafetensors|instanttensor`·`--safetensors-load-strategy eager|prefetch` 는 이 필터를 타지 않거나
#        파일 전체를 읽는다 -> 이 팩에서는 쓰지 않는다(레시피 몫). 기본(lazy)만 지원.
# model-trigger : DeepseekV41 + 파일 기반 Engram(83). 다른 모델은 `.engram.embed.` 이름이 없어 불변.
# fail-loud : 83 의 백엔드 모듈이 없거나 `_configured_table_dir` 가 없으면 빌드 실패(헬퍼가 런타임에 그것을 import 한다 - ImportError 를 삼키지 않는다).
# provenance : 우리 저장소의 자체 편집(외부 코드 사본 아님).
# 마지막 PRE 스크립트: fetch 캐시 정리(이미지 레이어에 tarball 이 남지 않게). 재실행은 다음 실행이 다시 받으므로 안전.
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

backend = L.VLLM_SRC / "vllm/models/deepseek_v41/nvidia/engram_file_backend.py"
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
    from vllm.models.deepseek_v41.nvidia.engram_file_backend import _configured_table_dir

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
