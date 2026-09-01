# syntax=docker/dockerfile:1
# 스켈레톤 — render_dockerfile.py(G2)가 manifest+버전해소값으로 채움

# Phase 1 안정 앵커: vLLM 0.19.1 / NGC 26.01-py3 (torch 핀 prefix 매칭) — prebuilt 동작 검증 구간.
FROM nvcr.io/nvidia/pytorch:26.01-py3

# 아래 3개 ARG는 resolve_wheel.py 가 GitHub Release 자산에서 자동 도출한 값으로 채운다.
# 버전마다 살짝 바뀌는 휘발성 디테일(cuXXX·manylinux)을 ARG로 격리 — 사람은 vLLM 버전만 지정.
ARG VLLM_VERSION=0.19.1
ARG CUDA_VERSION=130
ARG VLLM_MANYLINUX=manylinux_2_35

RUN cp /etc/pip/constraint.txt /etc/pip/constraint.txt.bak \
    && : > /etc/pip/constraint.txt

# CPU_ARCH 는 render-time 리터럴 baking 금지 → build-time $(uname -m) (헌법: manifest 스캔값과 로컬빌드 일치)
RUN CPU_ARCH=$(uname -m) \
    && pip install --no-deps \
      "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-${VLLM_MANYLINUX}_${CPU_ARCH}.whl"

# 조건부 패치: NGC torch(2.11 alpha)의 register_opaque_type 가 hoist 인자를 안 받을 때만
# LayerName opaque-type 최적화를 끈다(Phase 2 구간 호환). torch 2.10대(이 앵커)에선 opaque_object 자체가
# 없거나 HAS_OPAQUE_TYPE=False 라 안전하게 skip 된다(서빙 기능엔 무영향).
RUN python3 - <<'PY'
import inspect, pathlib
needs = False
try:
    from torch._library.opaque_object import register_opaque_type
    needs = "hoist" not in inspect.signature(register_opaque_type).parameters
except Exception:
    needs = False  # torch 2.10 등 opaque_object 미존재 → 패치 불요
p = pathlib.Path("/usr/local/lib/python3.12/dist-packages/vllm/utils/torch_utils.py")
if needs and p.exists():
    s = p.read_text().replace(
        'HAS_OPAQUE_TYPE = is_torch_equal_or_newer("2.11.0.dev")',
        'HAS_OPAQUE_TYPE = False  # patched: NGC torch register_opaque_type lacks hoist arg',
    )
    p.write_text(s)
    print("[patch] HAS_OPAQUE_TYPE disabled (NGC torch hoist mismatch)")
else:
    print("[patch] skipped (torch supports hoist / no opaque_object / file missing)")
PY

COPY requirements.txt /tmp/requirements.txt
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r /tmp/requirements.txt

WORKDIR /workspace

EXPOSE 8000
