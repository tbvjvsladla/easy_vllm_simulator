#!/bin/bash
# 32-mhc-hc-tilelang-sm121.sh — 빌드-바깥 패치(vLLM 소스-게이트) · upstream-version-watch §4.7 · 헌법 3+1+1
#
# what : DeepSeek-V4 HyperConnection prenorm GEMM(mhc)을 sm_121a(GB10)서 DeepGEMM 대신 **순수 TileLang 폴백**으로 강제.
#        파일 vllm/model_executor/kernels/mhc/tilelang.py 의 mhc_pre_tilelang:
#          `use_deep_gemm = is_deep_gemm_supported()` → `use_deep_gemm = False`(TileLang HC 경로).
# why  : stock 0.24.0(#43477)의 HyperConnection 이 use_deep_gemm=True 시 DeepGEMM tf32_hc_prenorm_gemm 호출 →
#        DeepGEMM hyperconnection.hpp 가 sm100 커널(sm100_tf32_hc_prenorm_gemm)로 디스패치하는데, 그 커널이
#        **tcgen05.fence**(데이터센터 Blackwell sm_100 전용 ISA)를 써 sm_121a(GB10 consumer Blackwell)서 NVCC ptxas
#        "Instruction 'tcgen05.fence' not supported on .target 'sm_121a'" → 하드 크래시(serve#8 arch-gate·serve#9 tcgen05 실증).
#        = vLLM #41063 tracking "Section 2"(tcgen05 커널은 arch-gate 확장 불가 — 실제 SM120 커널/폴백 필요).
#        해소: vLLM 자체 `_tilelang_hc_prenorm_gemm`(순수 TileLang, sm_121a JIT 동작 — serve#5·#6 컴파일 확증) 강제.
#        커뮤니티(hazyumps/lmxxf/NVIDIA forum GUIDE)도 sm_121 서 HC/indexer 를 Triton/TileLang 폴백으로 처리.
# scope: HC prenorm 한정(FP8 dense linear 은 layout.hpp arch-패치로 DeepGEMM 유지 — 별개 커널). SM12x arch-enablement(모델-키잉 ✗).
# probe: serve#9 ptxas tcgen05.fence 크래시 로그 · mhc/tilelang.py:167 use_deep_gemm 게이트 독해.
# REF  : vLLM v0.24.0 vllm/model_executor/kernels/mhc/tilelang.py:167,194 · issue #41063 Section-2 · plan_2026070119_1.
set -e
F=/workspace/vllm-src/vllm/model_executor/kernels/mhc/tilelang.py
[ -f "$F" ] || { echo "[32-mhc-hc-tilelang] FAIL: $F 부재 — vLLM 소스 빌드 전제" >&2; exit 1; }

python3 - "$F" <<'PY'
import sys
f = sys.argv[1]; s = open(f).read()
OLD = "    use_deep_gemm = is_deep_gemm_supported()"
NEW = ("    # [sm_121a/GB10 · plan_2026070119_1] DeepGEMM sm100 HC 커널이 tcgen05.fence(sm_100 전용) 사용 → sm_121a NVCC 실패.\n"
       "    #   vLLM 자체 순수-TileLang HC 폴백 강제(FP8 linear 은 layout.hpp arch-패치로 DeepGEMM 유지 — 별개 경로).\n"
       "    use_deep_gemm = False  # was: is_deep_gemm_supported()")
if "was: is_deep_gemm_supported()" in s:
    print("[32-mhc-hc-tilelang] 이미 패치됨 — skip(멱등)"); sys.exit(0)
if OLD not in s:
    sys.stderr.write("[32-mhc-hc-tilelang] FAIL: 예상 라인 'use_deep_gemm = is_deep_gemm_supported()'(4-space indent) 미발견 — 상류 변경 의심(false-determinism 차단)\n"); sys.exit(1)
n = s.count(OLD)
open(f, "w").write(s.replace(OLD, NEW))
print(f"[32-mhc-hc-tilelang] OK — mhc HC use_deep_gemm=False 강제 × {n}(TileLang HC 폴백, sm_121a tcgen05 회피)")
PY

# stale .pyc 무효화(즉시 반영).
find "$(dirname "$F")" -name '*.pyc' -path '*mhc*' -delete 2>/dev/null || true

# 검증(fail-loud): 패치 반영 확인. 실 TileLang HC JIT/서빙 = serve 스모크 최종중재.
grep -qF "was: is_deep_gemm_supported()" "$F" && echo "[32-mhc-hc-tilelang] 검증 OK — mhc TileLang HC 폴백 강제 반영"
