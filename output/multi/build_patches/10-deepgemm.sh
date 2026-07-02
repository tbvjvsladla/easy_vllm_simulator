#!/bin/bash
# 10-deepgemm.sh — 빌드-바깥 의존 패치 (build_patches 모듈 · upstream-version-watch §4.7 · 헌법 3+1+1 따름정리)
# [후보 A · plan_2026070213_1 · exhaustive-matrix T-A] DeepGEMM nv_dev — SM120 네이티브 실행커널
#
# what : deepseek-ai/DeepGEMM @ nv_dev(a6b593d, 2026-06-29 = vLLM 0.24.0 릴리즈 동일자) 클론+빌드. arch-게이트 소스패치 **없음**.
# why  : 선행 음성정직(testlog_2026070207_1)은 "stock DeepGEMM 891d57b 에 SM120 FP8 실행커널 부재 → 서빙 불가"라 했으나,
#        그 판정은 nv_dev 를 시도하지 않았다. nv_dev 소스 직독(참조-그라운디드, plan_2026070213_1 §1.3):
#          (a) csrc/apis/layout.hpp: `(arch_major == 10 or arch_major == 12)` — SM120 SF-transform 네이티브(serve#3/#7 벽 해소)
#          (b) csrc/apis/gemm.hpp: SM120 전용 FP8 GEMM 플로우 `fp8_fp4_gemm_nt_sm120()`/`sm120_fp8_fp4_gemm_1d1d()`/
#              `sm120_to_k_major()` — 891d57b 가 없던 gemm.hpp:99 실행커널이 **nv_dev 엔 실재**(serve#10 벽 해소 기대)
#          (c) csrc/apis/hyperconnection.hpp: `sm120_tf32_hc_prenorm_gemm`(mHC HC 커널 네이티브 → patch32 TileLang 강제 불요)
#        ∴ Section-1 arch-게이트 소스패치 제거(nv_dev 네이티브) · patch32 비활성 · E8M0=1(arch12 네이티브 UE8M0 pack).
# model-trigger : DeepseekV4ForCausalLM FP8 block-scale dense/MoE (SM12x). 범용 arch-enablement(모델-키잉 ✗ — 이미지 네이밍 불변식).
# governance : 의존 오버라이드(포크 아님 — 공식 deepseek-ai repo 의 nv_dev 개발브랜치, SHA 핀). vLLM 0.24.0 deepgemm.cmake
#        핀(891d57b)을 런타임 pip install 로 오버라이드 = 아치-enablement 변종 트랙(래더: deps-패치→소스-게이트→**의존 오버라이드**).
#        리스크 = vLLM 0.24.0 ↔ nv_dev API drift(단 동일자 → 낮음). 최종중재 = serve 스모크(Unknown recipe/SF = drift 신호).
# probe: nv_dev SM120 심볼 실재 확인(sm120_fp8_fp4_gemm / sm120_tf32_hc_prenorm_gemm) — 부재 시 fail-loud(브랜치 변경 의심).
# 폐쇄망: github.com/deepseek-ai/DeepGEMM@a6b593d + 서브모듈(cutlass·fmt) 사전미러 후 로컬 경로 지정.
set -e

# nv_dev 핀 = SM120 실행커널 원천(#43477 동시대 · SHA 핀 force-push 면역).
DG_REPO="https://github.com/deepseek-ai/DeepGEMM.git"
DG_REF="a6b593d2826719dcf4892609af7b84ee23aaf32a"   # nv_dev @ 2026-06-29 (indexer_n_heads=16 merge)
DG_DIR=/workspace/deepgemm-src

export TORCH_CUDA_ARCH_LIST="${TORCH_CUDA_ARCH_LIST:-12.1a}"   # GB10 sm_121a
export MAX_JOBS="${MAX_JOBS:-4}"

echo "[10-deepgemm/nv_dev] cloning deepseek-ai/DeepGEMM @ $DG_REF (nv_dev SM120) ..."
rm -rf "$DG_DIR"
git clone "$DG_REPO" "$DG_DIR"
git -C "$DG_DIR" checkout --detach "$DG_REF"
git -C "$DG_DIR" submodule update --init --recursive
echo "[10-deepgemm/nv_dev] HEAD=$(git -C "$DG_DIR" rev-parse HEAD)"

# ── arch-게이트 소스패치 없음 — nv_dev 네이티브 SM120. 대신 SM120 실행커널 심볼 실재 확인(fail-loud probe) ──
python3 - "$DG_DIR" <<'PY'
import sys, pathlib
root = pathlib.Path(sys.argv[1])
# nv_dev 가 실제로 SM120 실행커널을 담고 있는지 소스 grep(참조-그라운디드 · false-positive 빌드 차단).
needles = {
    "csrc/apis/gemm.hpp": "sm120",              # fp8_fp4_gemm_nt_sm120 / sm120_to_k_major
    "csrc/apis/layout.hpp": "arch_major == 12", # SM120 SF-transform 네이티브
    "csrc/apis/hyperconnection.hpp": "sm120",   # sm120_tf32_hc_prenorm_gemm
}
missing = []
for rel, needle in needles.items():
    f = root / rel
    if not f.exists() or needle not in f.read_text():
        missing.append(f"{rel} :: '{needle}'")
if missing:
    sys.stderr.write("[10-deepgemm/nv_dev] FAIL: nv_dev SM120 심볼 미발견(브랜치/레이아웃 변경 의심):\n  " + "\n  ".join(missing) + "\n")
    sys.exit(1)
print("[10-deepgemm/nv_dev] probe OK — SM120 실행커널 심볼 실재(gemm sm120 · layout arch12 · HC sm120)")
PY

echo "[10-deepgemm/nv_dev] building wheel (TORCH_CUDA_ARCH_LIST=$TORCH_CUDA_ARCH_LIST MAX_JOBS=$MAX_JOBS) ..."
cd "$DG_DIR"
rm -rf -- build dist ./*.egg-info 2>/dev/null || true
python3 setup.py bdist_wheel
pip install --no-cache-dir dist/*.whl
cd /workspace

python3 -c "import importlib.util,sys; sys.exit(0 if importlib.util.find_spec('deep_gemm') else 1)" \
    && echo "[10-deepgemm/nv_dev] OK — DeepGEMM nv_dev(a6b593d) 설치(런타임 SM120 커널 검증 = serve 스모크)"
