#!/bin/bash
# 30-mxfp4-triton-sm121.sh — 빌드-바깥 패치(소스 게이트 완화) · upstream-version-watch §4.7 · 헌법 3+1+1
#
# what : OAI-Triton MXFP4 MoE expert 디바이스 게이트의 capability 윈도우를 SM120/121(GB10) 포함하도록 확장.
#        파일 .../fused_moe/experts/gpt_oss_triton_kernels_moe.py 의
#        `_triton_kernel_moe_supports_current_device()` 에서 (9,0)<=cap<(11,0) → < (13,0).
# why  : DeepSeek-V4 MXFP4 expert(74.3GiB/node)는 GB10/SM121 서 TRITON 만 유효(flashinfer TRTLLM=SM100·
#        CUTLASS=SM90 arch-월, devlog 측정). 이 게이트가 SM120 을 *보수적* 제외(소스 주석: "this PR is
#        ROCm-scoped and the broader CUDA range was not validated") → MARLIN 강제 → MARLIN 이 expert 를
#        repack(~37GiB 트랜지언트) → 통합메모리 OOM·호스트다운(serve#1-5 측정). 게이트 완화 → TRITON(non-repacking) → 적합.
# why-build-not-runtime : 동일 완화를 런타임 .pth(arm_patch) 로 시도했으나 **vllm/ray serve 워커 프로세스가
#        site-init 을 안 타 .pth 불발**(serve#5 측정: "gate relaxed" 0회 발화, MARLIN 선택). 소스패치는 전
#        프로세스에 균일·timing-free → robust. (런타임 .pth 메커니즘 한계는 별도 조사 대상 — exaone45 패치도 영향.)
# scope: 범용(MXFP4 MoE). 오라클이 TRITON 을 MARLIN 보다 선호 → SM121 의 모든 MXFP4 MoE 모델이 TRITON 채택.
#        이미지-네이밍 불변식 보존(모델-키잉 ✗ — arch 가용성 확장일 뿐). 전제: 20-triton-kernels.sh(vendored matmul_ogs).
# probe: serve#5 .pth 불발 측정 + ephemeral gate→True. 실 triton JIT(sm_121a)/서빙/repack 회피 = serve 스모크가 최종 중재.
# REF  : gpt_oss_triton_kernels_moe.py:37-50 · oracle/mxfp4.py · plan_2026062811_1.
set -e
F=/workspace/vllm-src/vllm/model_executor/layers/fused_moe/experts/gpt_oss_triton_kernels_moe.py
[ -f "$F" ] || { echo "[30-mxfp4-triton-sm121] FAIL: $F 부재 — vLLM 소스 빌드 전제" >&2; exit 1; }

python3 - "$F" <<'PY'
import sys
f = sys.argv[1]; s = open(f).read()
OLD = "(9, 0) <= (cap.major, cap.minor) < (11, 0)"
NEW = "(9, 0) <= (cap.major, cap.minor) < (13, 0)"
if NEW in s:
    print("[30-mxfp4-triton-sm121] 이미 완화됨 — skip")
elif OLD in s:
    open(f, "w").write(s.replace(OLD, NEW, 1))
    print("[30-mxfp4-triton-sm121] OK — 게이트 완화:", OLD, "→ < (13, 0)")
else:
    sys.stderr.write("[30-mxfp4-triton-sm121] FAIL: 예상 게이트 문자열 미발견 — 상류 변경 의심(false-determinism 차단)\n")
    sys.exit(1)
PY

# 편집 즉시 반영(editable install) — stale .pyc 무효화.
find /workspace/vllm-src -path '*fused_moe/experts/__pycache__*gpt_oss_triton*' -name '*.pyc' -delete 2>/dev/null || true

# 검증(fail-loud): 소스가 SM121(12.x) 을 허용하는지. 실 게이트 평가/서빙 = serve 스모크.
grep -qF "(9, 0) <= (cap.major, cap.minor) < (13, 0)" "$F" && echo "[30-mxfp4-triton-sm121] 검증 OK — 소스 게이트 SM120/121 포함"
