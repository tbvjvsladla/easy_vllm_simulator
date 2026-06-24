#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
# 분산 vLLM 서빙 공통 런너
# 역할:
#   1) Ray 클러스터 세팅 (master: head 시작+대기, slave: head 접속)
#   2) master일 경우 모델별 sh 파일을 source하여 vllm serve 실행
#
# NODE_ROLE=master → Ray head → slave 대기 → /app/configs/${CONFIG_FILE}.sh source
# NODE_ROLE=slave  → master 대기 → Ray worker --block
# ═════════════════════════════════════════════════════════════════════
set -e

# ─── 공통: 필수 환경변수 체크 ───
: "${NODE_ROLE:?NODE_ROLE must be 'master' or 'slave'}"
: "${VLLM_HOST_IP:?VLLM_HOST_IP must be set}"
: "${HEAD_NODE_IP:?HEAD_NODE_IP must be set}"
: "${RAY_PORT:=6379}"
: "${CONFIG_FILE:?CONFIG_FILE must be set (e.g., gpt-oss-120b-source)}"

# ─── Ray object-store 상한 (통합메모리 풀 보호) ───
# ⚠ Ray 는 object-store 크기를 ENV 로 인식하지 않는다(RAY_object_store_memory = silently ignored,
#   Phase0 구조검증 PRIMARY DEFECT). `ray start --object-store-memory <bytes>` CLI 플래그로만 적용된다.
#   값은 env(.env.<config>)의 RAY_OBJECT_STORE_MEMORY 에서 받아 manifest/env-driven 유지.
: "${RAY_OBJECT_STORE_MEMORY:=2000000000}"

MODEL_SH="/app/configs/${CONFIG_FILE}.sh"
MODEL_YAML="/app/configs/${CONFIG_FILE}.yaml"

# ═════════════════════════════════════════════════════════════════════
# MASTER 노드 분기
# ═════════════════════════════════════════════════════════════════════
if [ "${NODE_ROLE}" = "master" ]; then

    # ─── 모델별 sh 존재 여부 확인 ───
    if [ ! -f "${MODEL_SH}" ]; then
        echo "[master][error] Model script not found: ${MODEL_SH}"
        exit 1
    fi
    if [ ! -f "${MODEL_YAML}" ]; then
        echo "[master][error] Model config yaml not found: ${MODEL_YAML}"
        exit 1
    fi

    # ─── 1) Ray head 시작 (object-store 상한 명시 — 통합메모리 풀 보호) ───
    echo "[master] Starting Ray head on ${VLLM_HOST_IP}:${RAY_PORT} (object-store ${RAY_OBJECT_STORE_MEMORY})..."
    ray start --head \
        --node-ip-address="${VLLM_HOST_IP}" \
        --port="${RAY_PORT}" \
        --object-store-memory "${RAY_OBJECT_STORE_MEMORY}"

    # ─── 2) Slave 노드 연결 대기 (최대 5분) ───
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

    # ─── 3) 모델별 sh 호출 (source로 현재 셸 컨텍스트에서 실행) ───
    # source를 쓰는 이유: 모델별 sh에서 export한 환경변수가 이후 vllm serve에서 유지되도록.
    echo "[master] Loading model-specific script: ${MODEL_SH}"
    source "${MODEL_SH}"

# ═════════════════════════════════════════════════════════════════════
# SLAVE 노드 분기
# ═════════════════════════════════════════════════════════════════════
elif [ "${NODE_ROLE}" = "slave" ]; then

    # ─── slave 는 Ray worker 전용 (vllm serve 안 함) ───
    # NCCL/RDMA env 는 compose env_file(envs/.env.interconnect — topology-keyed Band2)로 주입된다.
    # 모델 트리플렛(<model>.sh/.yaml = model-keyed Band3)은 slave 에 불요 — master 가 보유·서빙(인스턴스=클러스터).
    # (S0: 과거 slave 가 ${MODEL_SH} 를 source 했으나 NCCL env 는 이미 env_file 로 분리됨 → 교차-band 런타임 의존 제거.)
    # RAY OOM/alloc env(RAY_memory_usage_threshold·RAY_memory_monitor_refresh_ms·PYTORCH_CUDA_ALLOC_CONF)도
    #   env_file(.env.<config>)로 주입되어 ray start 前 도달(plan_2026062500_1 §4.2).

    # ─── 1) Master의 Ray head 포트 대기 (최대 5분) ───
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

    # ─── 2) Ray worker 시작 (foreground, --block; object-store 상한 명시) ───
    echo "[slave] Starting Ray worker (object-store ${RAY_OBJECT_STORE_MEMORY})..."
    exec ray start --address="${HEAD_NODE_IP}:${RAY_PORT}" \
        --node-ip-address="${VLLM_HOST_IP}" \
        --object-store-memory "${RAY_OBJECT_STORE_MEMORY}" \
        --block

# ═════════════════════════════════════════════════════════════════════
# 알 수 없는 역할
# ═════════════════════════════════════════════════════════════════════
else
    echo "[error] Invalid NODE_ROLE='${NODE_ROLE}'. Must be 'master' or 'slave'."
    exit 1
fi
