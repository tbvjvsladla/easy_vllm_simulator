#!/bin/bash
# 10-deepgemm.sh — 빌드-바깥 의존 패치 (build_patches 모듈 · upstream-version-watch §4.7 · 헌법 3+1+1 따름정리)
#
# what : STOCK deepseek-ai/DeepGEMM @ 891d57b (vLLM 0.24.0 deepgemm.cmake 정확한 핀 — 포크·버전스왑 ✗) 를 클론하고,
#        csrc/*/layout.hpp 의 arch_major 게이트를 SM12x(GB10 sm_121a) 포함하도록 확장하는 **소스-게이트 패치** 후 빌드.
# why  : stock vLLM 0.24.0(#43477)의 DeepSeek-V4 FP8 dense SF-layout 경로가 DeepGEMM 을 호출하는데, DeepGEMM 891d57b 의
#        layout.hpp 게이트가 arch_major∈{9,10}(SM90/SM100) 만 처리 → GB10(arch_major==12)은 fallthrough:
#          (a) csrc/apis/layout.hpp transform_sf_into_required_layout: `arch_major == 10` 브랜치 밖 → "Unknown SF transformation"(serve#3·#7)
#          (b) csrc/utils/layout.hpp get_default_recipe: `else if (arch_major == 10)` 밖 → "Unknown recipe"(serve#6)
#        해소: **SM12x 를 SM10(Blackwell 동족)처럼 처리** — vLLM #41063 tracking "Section 1: Pure Dispatch Fixes(Safe Extensions)"
#        (커뮤니티 GB10 10-layer 디버그 확증). vLLM 측 게이트는 이미 SM120 허용(is_device_capability_family(120), oracle) → vLLM 패치 불요.
# model-trigger : DeepseekV4ForCausalLM FP8 block-scale dense linear (SM12x). 범용 arch-enablement(모델-키잉 ✗ — 이미지 네이밍 불변식).
# governance : 소스-게이트 패치(포크 아님·stock 핀 유지) — 3+1+1 build_patch. 근거: vLLM issue #41063 patch-matrix · HF discussions#28 ·
#        NVIDIA DGX Spark forum GUIDE(374742) · hazyumps/deepseek-v4-flash-gb10 · lmxxf/deepseek-v4-deployment-on-dgx-spark. 최종중재=serve 스모크.
# probe: serve#3~#7 crash 로그(layout.hpp:59/76 arch fallthrough) + upstream 정확 라인 독해(891d57b apis/layout.hpp:43-59·utils/layout.hpp:60-90).
# 폐쇄망: github.com/deepseek-ai/DeepGEMM@891d57b + 서브모듈(cutlass·fmt) 사전미러 후 로컬 경로 지정.
set -e

# STOCK DeepGEMM 핀 = vLLM 0.24.0 deepgemm.cmake·tools/install_deepgemm.sh 와 동일(포크 ✗·버전스왑 ✗). 소스만 arch-게이트 확장.
DG_REPO="https://github.com/deepseek-ai/DeepGEMM.git"
DG_REF="891d57b4db1071624b5c8fa0d1e51cb317fa709f"
DG_DIR=/workspace/deepgemm-src

export TORCH_CUDA_ARCH_LIST="${TORCH_CUDA_ARCH_LIST:-12.1a}"   # GB10 sm_121a
export MAX_JOBS="${MAX_JOBS:-4}"

echo "[10-deepgemm] cloning STOCK deepseek-ai/DeepGEMM @ $DG_REF (vLLM-pinned) ..."
rm -rf "$DG_DIR"
git clone --recursive "$DG_REPO" "$DG_DIR"
git -C "$DG_DIR" checkout --detach "$DG_REF"
git -C "$DG_DIR" submodule update --init --recursive
echo "[10-deepgemm] HEAD=$(git -C "$DG_DIR" rev-parse HEAD)"

# ── Section-1 소스-게이트 패치: arch_major==10(SM100) 게이트를 SM12x(arch_major==12·GB10) 도 허용하도록 확장 ──
#   fail-loud: 예상 패턴 미발견 시 빌드 실패(false-determinism 차단 — 상류 DeepGEMM 변경 의심). 멱등: 이미 12 있으면 skip.
python3 - "$DG_DIR" <<'PY'
import sys, pathlib, re
root = pathlib.Path(sys.argv[1])
# SM12x=SM100 동족 arch-게이트 확장 대상(Section-1 safe extensions):
#   layout.hpp = FP8 SF-transform/recipe(serve#3/#6/#7) · hyperconnection.hpp = TF32 HC prenorm GEMM(serve#4/#8).
#   ⚠ FP4 커널(gemm.hpp fp8_fp4·attention.hpp·einsum.hpp)은 tcgen05 ISA 라 Section-2(미확장 — humming MoE·flashinfer attn 이 우회).
targets = ["csrc/apis/layout.hpp", "csrc/utils/layout.hpp", "csrc/apis/hyperconnection.hpp"]
total = 0
for rel in targets:
    f = root / rel
    if not f.exists():
        sys.stderr.write(f"[10-deepgemm] FAIL: {rel} 부재 — DeepGEMM 소스레이아웃 변경 의심(891d57b 전제)\n"); sys.exit(1)
    s = f.read_text()
    if "arch_major == 12" in s:
        print(f"[10-deepgemm] {rel}: 이미 arch_major==12 포함 — skip(멱등)"); continue
    # SM10 게이트를 (SM10 or SM12) 로 확장. C++ alternative-token 'or' 사용(코드베이스 관용구).
    n = s.count("arch_major == 10")
    if n == 0:
        sys.stderr.write(f"[10-deepgemm] FAIL: {rel} 에 'arch_major == 10' 게이트 미발견 — 상류 변경 의심(false-determinism 차단)\n"); sys.exit(1)
    s2 = s.replace("arch_major == 10", "(arch_major == 10 or arch_major == 12)")
    f.write_text(s2)
    total += n
    print(f"[10-deepgemm] {rel}: arch_major==10 → (==10 or ==12) × {n} 확장(SM12x=SM100 Blackwell 동족 처리)")
if total == 0:
    print("[10-deepgemm] (모든 타겟 이미 패치됨 — 멱등)")
PY

echo "[10-deepgemm] building wheel (TORCH_CUDA_ARCH_LIST=$TORCH_CUDA_ARCH_LIST MAX_JOBS=$MAX_JOBS) ..."
cd "$DG_DIR"
rm -rf -- build dist ./*.egg-info 2>/dev/null || true
python3 setup.py bdist_wheel
pip install --no-cache-dir dist/*.whl
cd /workspace

# 빌드타임 검증 = 패키지 존재 + 패치 반영 확인. 실 SM12x SF-transform/recipe JIT 는 libcuda 필요 → **런타임 serve 스모크가 최종 중재**.
python3 -c "import importlib.util,sys; sys.exit(0 if importlib.util.find_spec('deep_gemm') else 1)" \
    && echo "[10-deepgemm] OK — stock DeepGEMM 891d57b + SM12x arch-게이트 패치 설치(런타임 커널 검증 = serve 스모크)"
