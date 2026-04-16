#!/bin/bash
# Qwen3.5-122B-A10B-FP8-Slave 노드 서빙 스크립트
# - Master의 Ray head 포트가 열릴 때까지 최대 5분 대기
# - Ray worker를 foreground(--block)로 실행

set -e

# ─────────────────────────────────────────────────────────────
# 1) Master 노드의 Ray head 대기 (최대 5분)
#    6379 포트가 열릴 때까지 nc로 TCP 연결 체크
# ─────────────────────────────────────────────────────────────
echo "[slave] Waiting for master Ray head at ${HEAD_NODE_IP}:6379 (max 5 min)..."
MAX_WAIT=60   # 5초 × 60회 = 300초 (5분)
for i in $(seq 1 ${MAX_WAIT}); do
    if nc -z "${HEAD_NODE_IP}" 6379 2>/dev/null; then
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
# 2) Ray worker 시작 (foreground, --block으로 컨테이너 유지)
# ─────────────────────────────────────────────────────────────
echo "[slave] Starting Ray worker..."
ray start --address="${HEAD_NODE_IP}:6379" \
    --node-ip-address="${VLLM_HOST_IP}" \
    --block