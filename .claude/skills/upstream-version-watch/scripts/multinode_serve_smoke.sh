#!/bin/bash
# 멀티노드 2노드 Ray 분산 서빙 + multi-smoke (검증된 오케스트레이션, 근거: docs/testlog/testlog_260607_7).
#
# master = 메인(로컬, Ray head + vllm serve), slave = 서브(SSH, Ray worker). 둘 다 동일 코드, IP로 역할 자동.
# 준비 판정은 **엔드포인트 health(http 200)** 폴링 — master 로그의 "Application startup complete" 는
# 조기 컴포넌트에서도 떠 거짓양성이 나므로 쓰지 않는다(testlog_260607_7 §3 학습).
#
# 사용: bash multinode_serve_smoke.sh <config_name> [--build] [--keep-up]
#   --build   : 서빙 전 양 노드 이미지 빌드(병렬, 최소병렬 원칙)
#   --keep-up : 스모크 후 컨테이너 유지(기본은 정리/down)
# 종료코드: 0=스모크 통과, 2=미준비/스모크 실패, 3=NAS/설정 실패.
set -uo pipefail

CONFIG="${1:?사용: multinode_serve_smoke.sh <config_name> [--build] [--keep-up]}"; shift || true
BUILD=0; KEEP=0
for a in "$@"; do [ "$a" = "--build" ] && BUILD=1; [ "$a" = "--keep-up" ] && KEEP=1; done

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
cd "$REPO"
EF="envs/.env.${CONFIG}"
[ -f "$EF" ] || { echo "[mn] FAIL: $EF 없음"; exit 3; }
val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
MC=$(val MASTER_CONTAINER_NAME); PORT=$(val SERVING_PORT)
MODEL=$(val SERVING_MODEL_NAME); SLAVE_IP=$(val SLAVE_HOST_IP)
SSH_USER="${SSH_USER:-$(val SSH_USER)}"; SSH_USER="${SSH_USER:-$(id -un)}"  # env-file > env > 현재 사용자(하드코딩 금지)
SUB_HOST="${SUB_HOST:-${SSH_USER}@${SLAVE_IP}}"
SSH="ssh -o BatchMode=yes -o ConnectTimeout=8"
SUB_CD="cd ~/ws_docker/vllm_serving_server &&"
echo "[mn] config=$CONFIG master=$MC port=$PORT model=$MODEL sub=$SUB_HOST"

# ── NAS pre-flight (다운로드 금지) ──
python3 "$SDIR/check_smoke_model.py" "$CONFIG" --repo "$REPO" || { echo "[mn] STOP: 스모크 모델 부재 — 다운로드 금지, 중단"; exit 3; }

# ── 빌드(옵션, 양 노드 병렬) ──
if [ "$BUILD" = "1" ]; then
  echo "[mn] 양 노드 빌드(병렬)..."
  docker compose --env-file "$EF" --profile master build >/tmp/mn_build_master.log 2>&1 & BPID=$!
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD docker compose --env-file $EF --profile slave build'" >/tmp/mn_build_slave.log 2>&1 & SPID=$!
  wait $BPID; MR=$?; wait $SPID; SR=$?
  if [ $MR -eq 0 ] && [ $SR -eq 0 ]; then echo "[mn] 빌드 OK(양 노드)";
  else echo "[mn] FAIL: 빌드(master=$MR slave=$SR). tail:"; tail -6 /tmp/mn_build_master.log /tmp/mn_build_slave.log; exit 2; fi
fi

# ── Ray 클러스터 기동 (master 먼저=head, slave 합류) ──
echo "[mn] master 기동(Ray head + serve)..."
docker compose --env-file "$EF" --profile master up -d >/dev/null 2>&1
echo "[mn] slave 기동(Ray worker, SSH)..."
$SSH "$SUB_HOST" "bash -lc '$SUB_CD docker compose --env-file $EF --profile slave up -d'" >/dev/null 2>&1

# ── 준비 폴링: 엔드포인트 health(거짓양성 회피) ──
echo "[mn] 엔드포인트 :$PORT health 폴링(2노드 분산 로드, 최대 15분)..."
READY=0
for i in $(seq 1 180); do
  [ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' http://localhost:$PORT/health 2>/dev/null)" = "200" ] && { echo "[mn] READY ~$((i*5))s"; READY=1; break; }
  docker ps --filter name="$MC" --filter status=running -q | grep -q . || { echo "[mn] master EXITED"; docker logs "$MC" 2>&1 | tail -12; break; }
  docker logs "$MC" 2>&1 | grep -qiE "CUDA out of memory|NCCL error|did not join|RuntimeError" && { echo "[mn] FAILURE(serve)"; docker logs "$MC" 2>&1 | grep -iE "out of memory|NCCL error|did not join|RuntimeError" | tail -3; break; }
  sleep 5
done

# ── multi-smoke (master 엔드포인트, reasoning 모델 대비 max_tokens 충분히) ──
RESULT=2
if [ "$READY" = "1" ]; then
  printf '{"model":"%s","messages":[{"role":"user","content":"2+2= ? \xec\x88\xab\xec\x9e\x90\xeb\xa7\x8c \xeb\x8b\xb5\xed\x95\x98\xec\x84\xb8\xec\x9a\x94."}],"max_tokens":256}' "$MODEL" > /tmp/mn_req.json
  curl -s -m 120 "http://localhost:$PORT/v1/chat/completions" -H "Content-Type: application/json" -d @/tmp/mn_req.json -o /tmp/mn_resp.json
  PASS=$(python3 -c "import json;d=json.load(open('/tmp/mn_resp.json'));m=d['choices'][0]['message'];print('1' if ((m.get('content') or '').strip() or (m.get('reasoning') or '').strip()) else '0')" 2>/dev/null)
  INFO=$(python3 -c "import json;d=json.load(open('/tmp/mn_resp.json'));m=d['choices'][0]['message'];print('content='+repr((m.get('content') or '')[:80]),'fr='+str(d['choices'][0].get('finish_reason')))" 2>/dev/null)
  if [ "$PASS" = "1" ]; then echo "[mn] SMOKE PASS — $INFO"; RESULT=0
  else echo "[mn] SMOKE FAIL — raw:"; head -c 300 /tmp/mn_resp.json; fi
fi

# ── 정리 ──
if [ "$KEEP" != "1" ]; then
  echo "[mn] 정리(양 노드 down)..."
  docker compose --env-file "$EF" --profile master down >/dev/null 2>&1
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD docker compose --env-file $EF --profile slave down'" >/dev/null 2>&1
fi
echo "[mn] 종료코드 $RESULT"
exit $RESULT
