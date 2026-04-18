#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
# Qwen3.5-122B-A10B-FP8 분산 서빙 스크립트 (master/slave 통합)
# NODE_ROLE 환경변수로 역할 분기:
#   NODE_ROLE=master → Ray head 시작 + slave 대기 + vllm serve
#   NODE_ROLE=slave  → master 대기 + Ray worker --block
# ═════════════════════════════════════════════════════════════════════
set -e

# 공통: 필수 환경변수 체크
: "${NODE_ROLE:?NODE_ROLE must be 'master' or 'slave'}"
: "${VLLM_HOST_IP:?VLLM_HOST_IP must be set}"
: "${HEAD_NODE_IP:?HEAD_NODE_IP must be set}"
: "${RAY_PORT:=6379}"

# ═════════════════════════════════════════════════════════════════════
# MASTER 노드 분기
# ═════════════════════════════════════════════════════════════════════
if [ "${NODE_ROLE}" = "master" ]; then

    # ─────────────────────────────────────────────────────────────
    # 1) Ray head 시작 (백그라운드 데몬화)
    # ─────────────────────────────────────────────────────────────
    echo "[master] Starting Ray head on ${VLLM_HOST_IP}:${RAY_PORT}..."
    ray start --head \
        --node-ip-address="${VLLM_HOST_IP}" \
        --port="${RAY_PORT}"

    # ─────────────────────────────────────────────────────────────
    # 2) Slave 노드 연결 대기 (최대 5분, GPU 2개 인식까지)
    # ─────────────────────────────────────────────────────────────
    echo "[master] Waiting for slave node to join the cluster (max 5 min)..."
    MAX_WAIT=60
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
    # ─────────────────────────────────────────────────────────────
    echo "[master] Starting vLLM serve..."
    exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
        --served-model-name "${SERVING_MODEL_NAME}" \
        --host "${SERVING_IP:-0.0.0.0}" \
        --port "${SERVING_PORT}"


# ═════════════════════════════════════════════════════════════════════
# SLAVE 노드 분기
# ═════════════════════════════════════════════════════════════════════
elif [ "${NODE_ROLE}" = "slave" ]; then

    # ─────────────────────────────────────────────────────────────
    # 1) Master의 Ray head 포트 대기 (최대 5분)
    # ─────────────────────────────────────────────────────────────
    echo "[slave] Waiting for master Ray head at ${HEAD_NODE_IP}:${RAY_PORT} (max 5 min)..."
    MAX_WAIT=60
    for i in $(seq 1 ${MAX_WAIT}); do
        if nc -z "${HEAD_NODE_IP}" "${RAY_PORT}" 2>/dev/null; then
            echo "[slave] Master Ray head is reachable."
            break
        fi
        if [ "${i}" -eq "${MAX_WAIT}" ]; then
            echo "[slave] ERROR: Master Ray head not reachable within 5 minutes. Exiting."
            exit 1
        fi
        echo "[slave] Waiting for master... (${i}/${MAX_WAIT})"
        sleep 5
    done

    # ─────────────────────────────────────────────────────────────
    # 2) Ray worker 시작 (foreground, --block)
    # ─────────────────────────────────────────────────────────────
    echo "[slave] Starting Ray worker..."
    exec ray start --address="${HEAD_NODE_IP}:${RAY_PORT}" \
        --node-ip-address="${VLLM_HOST_IP}" \
        --block

# ═════════════════════════════════════════════════════════════════════
# 알 수 없는 역할
# ═════════════════════════════════════════════════════════════════════
else
    echo "[error] Invalid NODE_ROLE='${NODE_ROLE}'. Must be 'master' or 'slave'."
    exit 1
fi