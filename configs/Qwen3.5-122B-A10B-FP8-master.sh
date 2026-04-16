#!/bin/bash
# Qwen3.5-122B-A10B-FP8-Master 노드 서빙 스크립트
# - Ray head 시작 (백그라운드 데몬)
# - vLLM serve 실행 (foreground, 최대 5분 내 slave 연결 대기)

set -e

# ─────────────────────────────────────────────────────────────
# 1) Ray head 시작 (백그라운드 데몬화, 즉시 리턴)
# ─────────────────────────────────────────────────────────────
echo "[master] Starting Ray head on ${VLLM_HOST_IP}:6379..."
ray start --head \
    --node-ip-address="${VLLM_HOST_IP}" \
    --port=6379

# ─────────────────────────────────────────────────────────────
# 2) Slave 노드 연결 대기 (최대 5분)
#    Ray 클러스터에 GPU가 2개 보일 때까지 확인
# ─────────────────────────────────────────────────────────────
echo "[master] Waiting for slave node to join the cluster (max 5 min)..."
MAX_WAIT=60   # 5초 × 60회 = 300초 (5분)
for i in $(seq 1 ${MAX_WAIT}); do
    GPU_COUNT=$(ray status 2>/dev/null | grep -oP '\d+(?=\.\d+/\d+\.\d+ GPU)' | head -1)
    if [ "${GPU_COUNT}" = "2" ] 2>/dev/null || ray status 2>/dev/null | grep -q "0.0/2.0 GPU"; then
        echo "[master] Slave node connected. Cluster ready with 2 GPUs."
        break
    fi
    if [ "${i}" -eq "${MAX_WAIT}" ]; then
        echo "[master] ERROR: Slave node did not join within 5 minutes. Exiting."
        ray stop
        exit 1
    fi
    echo "[master] Waiting for slave... (${i}/${MAX_WAIT})"
    sleep 5
done

# ─────────────────────────────────────────────────────────────
# 3) vLLM serve 실행 (foreground)
#    CONFIG_FILE에서 -master/-slave 접미사 제거 후 yaml 참조
# ─────────────────────────────────────────────────────────────
echo "[master] Starting vLLM serve..."
vllm serve --config "/app/configs/${CONFIG_FILE%-*}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --port "${SERVING_PORT}"