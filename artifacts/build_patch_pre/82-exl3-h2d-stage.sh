#!/bin/bash
# 82-exl3-h2d-stage.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · 가중치 H2D 복사를 pinned 스테이징 경유로
#
# what : Mia 오버레이 patch_h2d_stage.py 의 **v0.30.0 재앵커 등가물**. stock 로더가 쓰는 `X.copy_(Y)` 7개 지점을
#        `exl3port_h2d_copy(X, Y)` 로 바꾼다(CPU 소스 -> 재사용 pinned 버퍼에 memcpy -> DMA. 디바이스/CPU->CPU 복사는 불변).
#          model_loader/weight_utils.py (default_weight_loader 2곳 · 헬퍼 본체 footer)
#          layers/linear.py (8곳: 1+5+2) · layers/vocab_parallel_embedding.py (2곳)
#          models/deepseek_v41/nvidia/{model,dspark}.py (attn_sink / 드래프트 narrow 로더 각 1곳)
# why  : GB10 UMA + safetensors 0.8: 샤드를 MAP_PRIVATE|PROT_WRITE 로 매핑하므로 mmap 에서 곧장 디바이스 복사하면 드라이버가
#        쓰기 의도로 페이지를 pin -> copy-on-write 로 익명 호스트 메모리가 계속 쌓여(레이어당 ~1.2GiB 실측) 두 Spark 가 wedge
#        (Mia README 2026-09-11/12). 우리 이미지 safetensors 0.8.0 이라 위험이 실재한다(delta report §2). exl3.py 는 자체 h2d_copy 를
#        갖고 있어 EXL3 텐서는 이미 커버되며, 이 패치는 EXL3 가 쓰지 않는 stock 로더 경로(norm·router gate·engram q/k·embed·헤드 일부)를 덮는다.
# model-trigger : 모든 모델 로드(이식 변종 이미지 한정). 호스트 안전 방어(policy:HOST_SAFETY_LAYERED_DEFENSE)와 같은 방향의 방어.
# provenance : 편집 원본 = Mia 커밋 overlay/patch_h2d_stage.py(AGPL-3.0, 파일은 받지 않음 · 편집문 재작성).
#        이식 시 변경: 이름을 exl3port_* 로 바꿔 Mia 이미지의 dsv41_h2d_copy 와 충돌하지 않게 했고 경로를 deepseek_v41 로 정정했다.
# idempotent/fail-loud : 편집마다 정확한 발생 횟수를 요구한다(count 불일치 = 빌드 실패). 재실행은 no-op.
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
