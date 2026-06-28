#!/bin/bash
# 10-deepgemm.sh — 빌드-바깥 의존 패치 (build_patches 모듈 · upstream-version-watch §4.7 · 헌법 3+1+1 따름정리)
#
# what : DeepGEMM (FP8/FP4 GEMM + DeepSeek 희소어텐션 커널) 설치.
# why  : DeepSeek-V4 DSA SparseAttnIndexer 가 요구 — 미설치 시 하드 RuntimeError
#        (vllm/model_executor/layers/sparse_attn_indexer.py:443 `not has_deep_gemm()`).
# model-trigger : DeepseekV4ForCausalLM (recipe-explorer crosscheck_model_card 발견 → upstream-version-watch 핸드오프).
# plan : plan_2026062812_1(3+1+1 패턴) · plan_2026062811_1(DeepSeek-V4-Flash 서빙).
# probe: deep_gemm 2.5.0+891d57b aarch64 wheel 빌드·설치·sm_121 인식 검증(devlog_2026062813_*).
# how  : vLLM 내장 공식 툴 tools/install_deepgemm.sh — deepseek-ai/DeepGEMM @ 핀 891d57b clone+build
#        (cmake/external_projects/deepgemm.cmake 와 동기). 범용 lib(모델-키잉 ✗ — DSA 안 쓰는 모델은 무시).
# 폐쇄망: github.com/deepseek-ai/DeepGEMM@891d57b4db1071624b5c8fa0d1e51cb317fa709f + 서브모듈 cutlass·fmt
#        사전미러 후 `install_deepgemm.sh --ref <local>` 또는 로컬 repo 경로 지정.
set -e

TOOL=/workspace/vllm-src/tools/install_deepgemm.sh
if [ ! -f "$TOOL" ]; then
    echo "[10-deepgemm] FAIL: $TOOL 부재 — vLLM 소스(/workspace/vllm-src) 빌드 뒤 실행 전제. core 빌드 순서 확인." >&2
    exit 1
fi

# 빌드타임 GPU 부재 → arch 명시(GPU 쿼리 회피). TORCH_CUDA_ARCH_LIST/MAX_JOBS 는 런타임 env_file override 무관(빌드 ARG/ENV).
export TORCH_CUDA_ARCH_LIST="${TORCH_CUDA_ARCH_LIST:-12.1a}"   # GB10 sm_121a
export MAX_JOBS="${MAX_JOBS:-4}"
echo "[10-deepgemm] install (TORCH_CUDA_ARCH_LIST=$TORCH_CUDA_ARCH_LIST MAX_JOBS=$MAX_JOBS) via $TOOL ..."
bash "$TOOL"

# 빌드타임 검증 = 패키지 존재만(find_spec). 실제 import deep_gemm._C / 커널 JIT 는 libcuda(드라이버) 필요 →
#   빌드타임 GPU 미주입이라 불가 → **런타임 serve 스모크가 최종 중재**(vLLM core 빌드검증과 동일 불변식).
python3 -c "import importlib.util,sys; sys.exit(0 if importlib.util.find_spec('deep_gemm') else 1)" \
    && echo "[10-deepgemm] OK — deep_gemm 패키지 설치 확인(런타임 _C/커널 검증 = serve 스모크)"
