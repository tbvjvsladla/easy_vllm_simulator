#!/bin/bash
# debug 모드 진입 시 자동으로 CX7 인터페이스 IP 감지
# 노드마다 다른 IP를 수동 설정할 필요 없음

# CX7 Domain 0 인터페이스(enp1s0f1np1)의 IPv4 주소를 자동 추출
VLLM_HOST_IP=$(ip -4 addr show enp1s0f1np1 2>/dev/null | grep -oP '(?<=inet\s)\d+(\.\d+){3}')

if [ -z "$VLLM_HOST_IP" ]; then
    echo "[WARN] Could not detect CX7 Domain 0 IP from enp1s0f1np1"
    VLLM_HOST_IP="unknown"
fi

export VLLM_HOST_IP
export SERVING_PORT="${SERVING_PORT:-8081}"

# 진입 배너
cat <<EOF
─────────────────────────────────────────────
[Debug Mode] 현재 노드 정보
  VLLM_HOST_IP   : $VLLM_HOST_IP
  SERVING_PORT   : $SERVING_PORT

Ray 세팅 예시:
  [메인] ray start --head --node-ip-address=\$VLLM_HOST_IP --port=6379
  [서브] ray start --address=<master_ip>:6379 --node-ip-address=\$VLLM_HOST_IP
─────────────────────────────────────────────
EOF