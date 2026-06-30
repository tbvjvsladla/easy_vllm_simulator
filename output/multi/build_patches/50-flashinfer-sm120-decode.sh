#!/bin/bash
# 50-flashinfer-sm120-decode.sh — 빌드-바깥 의존 패치 (build_patches 모듈 · upstream-version-watch §4.7 · 헌법 3+1+1)
#
# what : flashinfer-python 0.6.12 → 0.6.13 (+ stale flashinfer-cubin/jit-cache 제거). DeepSeek-V4 SM120 decode MLA
#        커널 `flashinfer.mla._sparse_mla_sm120` 가 0.6.13/main 에만 존재(0.6.12 부재).
# why  : 포크 c766cbc6(=PR#41834 검증 head sm120-pr-41834-stable-preview-20260626)의 게이트
#        `VLLM_DEEPSEEK_V4_FLASHINFER_SM120_DECODE` 가 flashinfer≥0.6.13 을 요구 → 0.6.12 면 silent fallback
#        (느린 sparse-MLA decode 경로) → 2×GB10 디코드 ~16 t/s 천장(라이브 3-trial 측정: base+MTP+FULL cudagraph
#        다 켜도 ~16). PR#41834 자체 GB10 SM121 2-node 검증 = 40 t/s @ C=1(flashinfer 0.6.13 + 게이트 ON).
#        ∴ flashinfer 0.6.13 = decode 천장 해소 핵심 레버(포크 변경 불요 — 코드경로 이미 보유, 라이브러리만 부족).
#        활성 env(VLLM_DEEPSEEK_V4_FLASHINFER_SM120_DECODE=1 등)는 serve 평면(.env.cluster, 양노드).
# why-here : flashinfer 는 vLLM 의존(Dockerfile.source-build line85 `pip install -e .` 가 0.6.12 끌어옴) →
#        build_patches 가 그 *뒤*(line111) 적용되므로 여기서 bump 하면 override(§4.7 modular — 파일 drop).
# aarch64 : flashinfer-python = py3-none-any(순수 python wheel) → 소스빌드 불요. sm_121 prebuilt cubin 부재 →
#        첫 decode 시 1회 런타임 JIT(kernel_warmup 가 워밍). ⚠ stale flashinfer-cubin/jit-cache(0.6.12) 잔존 시
#        startup version-mismatch → 먼저 uninstall(PR#41834 본문 명시).
# model-trigger : DeepseekV4ForCausalLM(SM120 decode 게이트). 범용(flashinfer 처럼 — 게이트 OFF 모델엔 inert).
#        이미지-네이밍 불변식 보존(모델-키잉 ✗ — flashinfer 는 범용 lib).
# plan : plan_2026063012_1(디코드 성능 escalation). probe = PR#41834 body "Gated SM120 decode optimization".
#        실 커널 JIT/서빙/40 t/s = serve 스모크가 최종 중재(빌드검증 ≠ 서빙검증).
set -e

TARGET=0.6.13
CUR=$(python3 -c "import flashinfer,sys; sys.stdout.write(getattr(flashinfer,'__version__','none'))" 2>/dev/null || echo none)
if [ "$CUR" = "$TARGET" ]; then
    echo "[50-flashinfer] 이미 flashinfer-python==$TARGET — skip(멱등)"
else
    echo "[50-flashinfer] flashinfer-python $CUR → $TARGET (stale cubin/jit-cache 제거 후 bump)"
    # stale 0.6.12 binary helper 제거(버전-mismatch startup 에러 차단 — PR#41834).
    pip uninstall -y flashinfer-jit-cache flashinfer-cubin >/dev/null 2>&1 || true
    pip install --no-cache-dir "flashinfer-python==${TARGET}"
fi

# 검증(fail-loud): 버전 0.6.13 확정(하드) + SM120 decode 커널 심볼 존재(soft 보고).
python3 - <<'PY'
import sys, importlib
import flashinfer
v = getattr(flashinfer, "__version__", "")
assert v.startswith("0.6.13"), f"FAIL: flashinfer-python {v!r} != 0.6.13 (SM120 decode 커널 부재 위험)"
have = False
try:
    m = importlib.import_module("flashinfer.mla")
    have = hasattr(m, "_sparse_mla_sm120")
except Exception as e:
    sys.stderr.write(f"[50-flashinfer] (warn) flashinfer.mla import 예외: {e}\n")
print(f"[50-flashinfer] OK — flashinfer {v} · _sparse_mla_sm120 present={have} (런타임 JIT/서빙 = serve 스모크 최종중재)")
PY
