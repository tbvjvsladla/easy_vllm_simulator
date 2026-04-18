#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
# Qwen3.5-397B-A17B-NVFP4 모델별 사전 처리 + vllm serve
# 397B total / 17B active, NVFP4 quantization
# ★ GB10 (Blackwell SM121)에서 NVFP4 Marlin kernel 강제 필요
# serve_runner.sh 에서 source 호출됨
# ═════════════════════════════════════════════════════════════════════

# ─── NVFP4 전용 환경변수 (★ master/slave 양쪽 모두 필요) ───
# 기본 NVFP4 커널이 SM120/121을 지원하지 않아 Marlin 커널로 강제 필요
export VLLM_TEST_FORCE_FP8_MARLIN=1

# 메모리 단편화 방지 (397B 대형 모델에서 필수적)
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True

# Ray 메모리 모니터 비활성화 (대형 모델 로드 시 false positive 방지)
export RAY_memory_monitor_refresh_ms=0

echo "[Qwen3.5-397B-NVFP4] Exported NVFP4 runtime flags:"
echo "  VLLM_TEST_FORCE_FP8_MARLIN=${VLLM_TEST_FORCE_FP8_MARLIN}"
echo "  PYTORCH_CUDA_ALLOC_CONF=${PYTORCH_CUDA_ALLOC_CONF}"
echo "  RAY_memory_monitor_refresh_ms=${RAY_memory_monitor_refresh_ms}"

# ─── SKIP_SERVE=1이면 환경변수만 세팅하고 종료 (slave 모드) ───
if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[Qwen3.5-397B-NVFP4] Env vars loaded, skipping serve (slave mode)"
    return 0
fi

# ─── vLLM serve 실행 (master only) ───
echo "[Qwen3.5-397B-NVFP4] Starting vLLM serve..."
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}"