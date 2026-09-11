#!/bin/bash
# 20-triton-kernels.sh — 빌드-바깥 의존 패치 (build_patches 모듈 · upstream-version-watch §4.7 · 헌법 3+1+1 따름정리)
#
# what : NGC 번들 standalone `triton_kernels`(불완전) 제거 → vLLM 자체 vendored 완전본
#        (`vllm.third_party.triton_kernels`)이 활성화되게 한다.
# why  : DeepSeek-V4 MXFP4 MoE 백엔드 오라클 cascade = TRTLLM→TRITON→FLASHINFER_CUTLASS→MARLIN.
#        GB10(sm_121a)에선 flashinfer CUTLASS=SM90전용·TRTLLM=SM100데이터센터 → 둘 다 arch-월(미지원).
#        TRITON 만 가능한데, NGC `triton_kernels 1.0.0+gitb4e20bb.nv26.5`엔 vLLM 이 import 하는
#        `matmul_ogs`·`routing` 서브모듈이 부재(구버전 API — `matmul.py`만 존재) → import 실패 → MARLIN 강제.
#        MARLIN 은 expert 가중치를 repack(~37GiB 트랜지언트)하는데, 74.3GiB/node 위에서 통합메모리 OOM →
#        **호스트 하드다운**(serve#1) / 워치독 트립(serve#2·#3, MemAvailable 0.5~1.4GiB까지 급락). 측정 진단 = devlog 후속.
#        vLLM `import_triton_kernels()` 는 site-packages `triton_kernels` 를 우선하고 **부재 시에만** vendored 로
#        `sys.modules` alias 한다. ∴ 불완전 NGC 패키지를 제거하면 vLLM 이 자기 vendored 완전본(matmul_ogs+routing 보유)을
#        alias → TRITON 백엔드 활성 → MARLIN repack 회피 → 74.3GiB/node 가 메모리에 적합.
# model-trigger : DeepseekV4ForCausalLM (MXFP4 MoE 74GiB/node — MARLIN repack 미적합). 범용(MXFP4 MoE 의 TRITON 경로 필요 모델 공통).
# plan : plan_2026062811_1(DeepSeek-V4-Flash 서빙) · plan_2026062812_1(3+1+1). serve#1-3 OOM 측정 진단.
# probe: 에페메럴 컨테이너서 uninstall 후 has_triton_kernels()=True · triton_kernels→vendored · matmul_ogs/routing import OK 확인(2026-06-28).
# how  : pip uninstall(순수 패키지 제거 — native 컴파일 없음). vendored 는 triton @jit 런타임 컴파일(sm_121a 검증=serve 스모크).
set -e

# vendored 가 아닌 standalone(site-packages NGC) triton_kernels 만 제거 — 멱등.
LOC=$(python3 -c "import importlib.util as u; s=u.find_spec('triton_kernels'); print(s.origin or '' if s else '')" 2>/dev/null || true)
case "$LOC" in
    "")                       echo "[20-triton-kernels] standalone triton_kernels 부재 — skip(이미 vendored 경로)" ;;
    */vllm/third_party/*)     echo "[20-triton-kernels] 이미 vendored($LOC) — skip" ;;
    *)                        echo "[20-triton-kernels] standalone 제거: $LOC"; pip uninstall -y triton_kernels >/dev/null 2>&1 || true ;;
esac

# 검증(fail-loud): 제거 후 vLLM 이 vendored 완전본을 alias 하고 matmul_ogs+routing 이 import 되는지.
#   (런타임 triton JIT / 실서빙 / repack 회피 = serve 스모크가 최종 중재 — core 빌드검증 불변식과 동일.)
python3 -c "
from vllm.utils.import_utils import has_triton_kernels
assert has_triton_kernels(), 'FAIL: has_triton_kernels() False — vendored fallback 미작동'
import triton_kernels
assert 'third_party' in (triton_kernels.__file__ or ''), f'FAIL: triton_kernels 가 vendored 아님: {triton_kernels.__file__}'
from triton_kernels.matmul_ogs import PrecisionConfig, FlexCtx
from triton_kernels.routing import routing, RoutingData
print('[20-triton-kernels] OK — vendored 활성:', triton_kernels.__file__)
"
