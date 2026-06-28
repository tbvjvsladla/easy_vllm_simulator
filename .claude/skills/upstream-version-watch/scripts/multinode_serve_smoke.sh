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
EF="output/multi/envs/.env.${CONFIG}"   # 산출물 통로 분리(plan_2026062312_1): compose·env 는 output/multi/ 아래
[ -f "$EF" ] || { echo "[mn] FAIL: $EF 없음"; exit 3; }
EFC="output/multi/envs/.env.cluster"    # Ray 클러스터-배포 env(Band2, S6 env-split). compose 보간(${MASTER_HOST_IP}·${SLAVE_HOST_IP}·${RAY_PORT})에 필요.
[ -f "$EFC" ] || { echo "[mn] FAIL: $EFC 없음 — 'render_dockerfile.py --cluster-envfile --topology multi --manifest output/multi/manifest.yaml -o $EFC' 선행(S6)"; exit 3; }
# 통로 self-containment 전제(plan_2026062321_1 I1/I2): 러너 스크립트가 통로에 materialize 됐는지 fail-loud.
for s in serve_runner.sh debug-init.sh; do
  [ -f "output/multi/configs/$s" ] || { echo "[mn] FAIL: 통로 미완결 — output/multi/configs/$s 부재. 먼저 'render_dockerfile.py --materialize-configs --topology multi' 실행(후 sync_to_sub.sh --apply)"; exit 3; }
done
val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
MC=$(val MASTER_CONTAINER_NAME); PORT=$(val SERVING_PORT)
MODEL=$(val SERVING_MODEL_NAME); SLAVE_IP=$(val SLAVE_HOST_IP)

# ── 이미지 정체성 전달(멀티 = 클러스터-와이드: 슬레이브가 마스터와 동일 이미지여야) ──
#   콤보 EF 에서 IMAGE_TAG/VLLM_REPO/VLLM_REF '만' 읽어 슬레이브 compose 보간에 전달한다(빌드-평면 인프라).
#   마스터는 --env-file $EF 로 자동 획득. 슬레이브는 EFC(Band2)만 받으므로 비-기본 이미지 변종(예 포크 …-source-sm12x)을
#   못 봐 stock 으로 빌드/기동하는 불일치가 난다 → 이 3개만 명시 전달.
#   ⚠ 모델 serve config(CONFIG_FILE)는 전달 안 함 → 슬레이브 Band2-only 보존(슬레이브 컨테이너 env 는 compose env_file=
#     .env.interconnect+.env.cluster 만, CONFIG_FILE=default 유지). 값에 공백 없음(URL/태그/SHA) → 무인용 prefix 안전.
#   근거: plan_2026062818_1 §S2.5 R10 · 슬레이브 Band2-only(plan_2026062811_2).
IMG=$(val IMAGE_TAG); VREPO=$(val VLLM_REPO); VREF=$(val VLLM_REF)
SLAVE_IMGVARS="${IMG:+IMAGE_TAG=$IMG }${VREPO:+VLLM_REPO=$VREPO }${VREF:+VLLM_REF=$VREF}"

# ── 서브 식별자/경로 해소(단일계약): env-file > manifest nodes[sub] > 폴백. 옛 고정 서브경로 하드코딩 제거 ──
_mf_sub() {  # field → nodes[role=sub].field (role 정확매칭 — 'subordinate' 등 접두 오인 방지)
  local manifest="$REPO/output/multi/manifest.yaml"
  [ -f "$manifest" ] || return 1
  awk -v field="$1" '
    /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; next }
    /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
    in_sub && $0 ~ "^[[:space:]]*" field ":" { sub("^[[:space:]]*" field ":[[:space:]]*",""); sub(/[[:space:]]*#.*/,""); gsub(/[ "\r]/,""); print; exit }
  ' "$manifest"
}
SLAVE_IP="${SLAVE_IP:-$(_mf_sub host)}"                                              # env-file > manifest
SSH_USER="${SSH_USER:-$(val SSH_USER)}"; SSH_USER="${SSH_USER:-$(_mf_sub ssh_user)}"; SSH_USER="${SSH_USER:-$(id -un)}"  # env-file > manifest > 현재 사용자
SUB_HOST="${SUB_HOST:-${SSH_USER}@${SLAVE_IP}}"
SSH="ssh -o BatchMode=yes -o ConnectTimeout=8"
SUB_WORK_DIR="${SUB_WORK_DIR:-$(_mf_sub work_dir)}"; SUB_WORK_DIR="${SUB_WORK_DIR:-$REPO}"  # env > manifest > 메인 REPO(R2 기본값=동일)
case "$SUB_WORK_DIR" in *[[:space:]]*) echo "[mn] FAIL: SUB_WORK_DIR 공백 — 원격 cd 임베드 불가: '$SUB_WORK_DIR'"; exit 3;; esac
SUB_CD="cd $SUB_WORK_DIR &&"   # bash -lc '...' 단일인용 컨텍스트 임베드 — 무공백 보장(위 가드)
echo "[mn] config=$CONFIG master=$MC port=$PORT model=$MODEL sub=$SUB_HOST sub_work_dir=$SUB_WORK_DIR"

# ── NAS pre-flight (다운로드 금지) ──
python3 "$SDIR/check_smoke_model.py" "$CONFIG" --repo "$REPO" --topology multi || { echo "[mn] STOP: 스모크 모델 부재 — 다운로드 금지, 중단"; exit 3; }

# ── 빌드(옵션, 양 노드 병렬) ──
if [ "$BUILD" = "1" ]; then
  echo "[mn] 양 노드 빌드(병렬)..."
  docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master build >/tmp/mn_build_master.log 2>&1 & BPID=$!
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave build'" >/tmp/mn_build_slave.log 2>&1 & SPID=$!
  wait $BPID; MR=$?; wait $SPID; SR=$?
  if [ $MR -eq 0 ] && [ $SR -eq 0 ]; then echo "[mn] 빌드 OK(양 노드)";
  else echo "[mn] FAIL: 빌드(master=$MR slave=$SR). tail:"; tail -6 /tmp/mn_build_master.log /tmp/mn_build_slave.log; exit 2; fi
fi

# ── Ray 클러스터 기동 (master 먼저=head, slave 합류) ──
echo "[mn] master 기동(Ray head + serve)..."
docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master up -d >/dev/null 2>&1
echo "[mn] slave 기동(Ray worker, SSH)..."
$SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave up -d'" >/dev/null 2>&1

# ── 준비 폴링: 엔드포인트 health(거짓양성 회피) ──
# READY_MAX(폴링 횟수×5s) 환경변수로 조정 가능 — 대형모델(예 Qwen3-Next-80B bf16 151GB CIFS 로드 ~11분
#   + KV/compile setup)은 기본 15분(180회)으로 부족 → READY_MAX=360(30분) 등으로 연장(testlog_2026062501_1 결함).
echo "[mn] 엔드포인트 :$PORT health 폴링(2노드 분산 로드; READY_MAX=${READY_MAX:-180}회×5s ≈ $(( ${READY_MAX:-180} * 5 / 60 ))분)..."
READY=0
for i in $(seq 1 "${READY_MAX:-180}"); do
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
  docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master down >/dev/null 2>&1
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave down'" >/dev/null 2>&1
fi
echo "[mn] 종료코드 $RESULT"
exit $RESULT
