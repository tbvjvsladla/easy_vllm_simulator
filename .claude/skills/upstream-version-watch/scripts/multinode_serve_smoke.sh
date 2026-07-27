#!/bin/bash
# 멀티노드 2노드 Ray 분산 서빙 + multi-smoke (검증된 오케스트레이션, 근거: docs/testlog/testlog_260607_7).
#
# master = 메인(로컬, Ray head + vllm serve), slave = 서브(SSH, Ray worker). 둘 다 동일 코드, IP로 역할 자동.
# 준비 판정은 **엔드포인트 health(http 200)** 폴링 — master 로그의 "Application startup complete" 는
# 조기 컴포넌트에서도 떠 거짓양성이 나므로 쓰지 않는다(testlog_260607_7 §3 학습).
#
# 사용: bash multinode_serve_smoke.sh <config_name> [--build] [--keep-up] [--no-watchdog]
#   --build   : 서빙 전 양 노드 이미지 빌드(병렬, 최소병렬 원칙)
#   --keep-up : 스모크 후 컨테이너 유지(기본은 정리/down — 워치독도 함께 유지)
#   --no-watchdog : 협역 워치독 사이드 기동 생략(plan_26071019 §2.3 — 진단 시)
# 종료코드: 0=스모크 통과, 2=미준비/스모크 실패, 3=NAS/설정 실패(7=RAM 게이트 거부 포함 시 3으로 수렴).
set -uo pipefail

CONFIG="${1:?사용: multinode_serve_smoke.sh <config_name> [--build] [--keep-up] [--no-watchdog]}"; shift || true
BUILD=0; KEEP=0; WATCHDOG=1
for a in "$@"; do [ "$a" = "--build" ] && BUILD=1; [ "$a" = "--keep-up" ] && KEEP=1; [ "$a" = "--no-watchdog" ] && WATCHDOG=0; done

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
cd "$REPO"
EF="output/multi/envs/.env.${CONFIG}"   # 산출물 통로 분리(plan_26062312): compose·env 는 output/multi/ 아래
[ -f "$EF" ] || { echo "[mn] FAIL: $EF 없음"; exit 3; }
EFC="output/multi/envs/.env.cluster"    # Ray 클러스터-배포 env(Band2, S6 env-split). compose 보간(${MASTER_HOST_IP}·${SLAVE_HOST_IP}·${RAY_PORT})에 필요.
[ -f "$EFC" ] || { echo "[mn] FAIL: $EFC 없음 — 'render_dockerfile.py --cluster-envfile --topology multi --manifest output/multi/manifest.yaml -o $EFC' 선행(S6)"; exit 3; }
# 통로 self-containment 전제(plan_26062321 I1/I2): 러너 스크립트가 통로에 materialize 됐는지 fail-loud.
for s in serve_runner.sh debug-init.sh; do
  [ -f "output/multi/configs/$s" ] || { echo "[mn] FAIL: 통로 미완결 — output/multi/configs/$s 부재. 먼저 'render_dockerfile.py --materialize-configs --topology multi' 실행(후 sync_to_sub.sh --apply)"; exit 3; }
done
val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
MC=$(val MASTER_CONTAINER_NAME); PORT=$(val SERVING_PORT)
MODEL=$(val SERVING_MODEL_NAME); SLAVE_IP=$(val SLAVE_HOST_IP)

# ── 이미지 정체성 전달(멀티 = 클러스터-와이드: 슬레이브가 마스터와 동일 이미지여야) ──
#   콤보 EF 에서 IMAGE_TAG/BUILD_DOCKERFILE/VLLM_REPO/VLLM_REF '만' 읽어 슬레이브 compose 보간에 전달한다(빌드-평면 인프라).
#   마스터는 --env-file $EF 로 자동 획득. 슬레이브는 EFC(Band2)만 받으므로 비-기본 이미지 변종(예 포크 …-source-sm12x)을
#   못 봐 stock 으로 빌드/기동하는 불일치가 난다 → 이 4개만 명시 전달.
#   ⚠ BUILD_DOCKERFILE 누락 결함(Solar-Open2 가 최초 노출, 2026-07-24): 슬레이브 build 는 --env-file $EFC(Band2)
#     만 받으므로 BUILD_DOCKERFILE 이 compose 기본값(Dockerfile.source-build)으로 폴백 → 변종 트랙(예
#     Dockerfile.source-build-upstage)서 **슬레이브만 다른 Dockerfile 로 빌드**. 종전 콤보는 전부
#     BUILD_DOCKERFILE=Dockerfile.source-build(=기본값)이라 잠복했다. 이미지 정체성의 일부이므로 동반 전달.
#   ⚠ 모델 serve config(CONFIG_FILE)는 전달 안 함 → 슬레이브 Band2-only 보존(슬레이브 컨테이너 env 는 compose env_file=
#     .env.interconnect+.env.cluster 만, CONFIG_FILE=default 유지). 값에 공백 없음(URL/태그/SHA) → 무인용 prefix 안전.
#   근거: plan_26062818 §S2.5 R10 · 슬레이브 Band2-only(plan_26062811_30_33).
IMG=$(val IMAGE_TAG); VREPO=$(val VLLM_REPO); VREF=$(val VLLM_REF); BDF=$(val BUILD_DOCKERFILE)
SLAVE_IMGVARS="${IMG:+IMAGE_TAG=$IMG }${BDF:+BUILD_DOCKERFILE=$BDF }${VREPO:+VLLM_REPO=$VREPO }${VREF:+VLLM_REF=$VREF}"

# ── 마운트 vars 전달(결함#2b · plan_26070119): materialize-env 산출(output/multi/.env)은 compose 가
#   --env-file 사용 시 auto-load 하지 않는다(--env-file 이 기본 .env 자동로드를 대체) → NAS/quant/tiktoken 마운트가
#   docker-compose.yaml 의 ${NAS_MODEL_PATH:-/mnt/models} 기본으로 폴백 → 컨테이너가 모델을 못 찾음(serve 즉사).
#   해소: 마운트 경로를 shell-env(compose 보간 최고 우선순위)로 명시 주입 — 마스터(env prefix)·슬레이브(ssh prefix) 동일.
#   경로값에 공백 없음(SLAVE_IMGVARS 와 동형) → 무인용 prefix 안전. 헌법 serve-time env 통로 불변식.
PENV_FILE="output/multi/.env"
MOUNTVARS=""
[ -f "$PENV_FILE" ] && MOUNTVARS="$(grep -E '^(NAS_MODEL_PATH|QUANT_MODEL_PATH|TIKTOKEN_HOST_PATH)=' "$PENV_FILE" | tr '\n' ' ')"

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

# ── NAS pre-flight + 로드-전 RAM 게이트(메인) — 다운로드 금지 · plan_26071019 §2.6 ──
#   rc 구분(2=모델부재 / 7=RAM게이트 거부 / 3=설정) — exit 7 을 '모델 부재·다운로드 금지'로 오귀속 금지.
NAS_OUT=$(python3 "$SDIR/check_smoke_model.py" "$CONFIG" --repo "$REPO" --topology multi --emit-gate-params 2>&1); NAS_RC=$?
printf '%s\n' "$NAS_OUT"
case "$NAS_RC" in
  0) : ;;
  7) echo "[mn] STOP: 로드-전 RAM 게이트 거부(메인) — 모델은 실재하나 가용 RAM 부족. 잔존 컨테이너/페이지캐시 정리 후 재시도(§모델 확보 결정트리 아님 — 다운로드 불요)"; exit 3 ;;
  2) echo "[mn] STOP: 스모크 모델 부재 — 다운로드 금지, 중단"; exit 3 ;;
  *) echo "[mn] STOP: NAS/설정 확인 실패(rc=$NAS_RC)"; exit 3 ;;
esac

# ── 슬레이브 노드 동일-문턱 RAM 게이트(예방 대칭 — 하드다운 #2=DS4 serve#1 '서브'였음 · §2.6) ──
#   메인이 emit 한 required_mib(같은 NAS·같은 ckpt÷TP)를 슬레이브 /proc/meminfo 에 비교. 슬레이브는
#   config/게이트 모듈 의존 없이 순수 MemAvailable 만 필요(부족 시 drop-caches 1회 재측정 후 판정).
REQ_MIB=$(printf '%s\n' "$NAS_OUT" | grep -oE 'required_mib=[0-9]+' | head -1 | cut -d= -f2)
if [ -n "$REQ_MIB" ]; then
  _slave_avail(){ $SSH "$SUB_HOST" "awk '/MemAvailable:/{print int(\$2/1024)}' /proc/meminfo" 2>/dev/null; }
  SLAVE_AVAIL=$(_slave_avail)
  if [ -n "$SLAVE_AVAIL" ] && [ "$SLAVE_AVAIL" -lt "$REQ_MIB" ]; then
    $SSH "$SUB_HOST" "[ -x /usr/local/sbin/vllm-drop-caches ] && sudo -n /usr/local/sbin/vllm-drop-caches" >/dev/null 2>&1
    SLAVE_AVAIL=$(_slave_avail)   # drop 후 재측정
  fi
  if [ -z "$SLAVE_AVAIL" ]; then
    echo "[mn] ⚠ 슬레이브 MemAvailable 조회 실패 — 슬레이브 게이트 생략(상시 워치독 층만 커버)"
  elif [ "$SLAVE_AVAIL" -lt "$REQ_MIB" ]; then
    echo "[mn] STOP: 슬레이브 로드-전 RAM 게이트 거부 — MemAvailable=${SLAVE_AVAIL}MiB < required=${REQ_MIB}MiB. 슬레이브 잔존 컨테이너/페이지캐시 정리 후 재시도(하드다운 #2 서브노드 예방)"; exit 3
  else
    echo "[mn] 슬레이브 RAM-gate PASS: MemAvailable=${SLAVE_AVAIL}MiB ≥ required=${REQ_MIB}MiB"
  fi
fi

# ── 빌드(옵션, 양 노드 병렬) ──
if [ "$BUILD" = "1" ]; then
  echo "[mn] 양 노드 빌드(병렬)..."
  docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master build >/tmp/mn_build_master.log 2>&1 & BPID=$!
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave build'" >/tmp/mn_build_slave.log 2>&1 & SPID=$!
  wait $BPID; MR=$?; wait $SPID; SR=$?
  if [ $MR -eq 0 ] && [ $SR -eq 0 ]; then echo "[mn] 빌드 OK(양 노드)";
  else echo "[mn] FAIL: 빌드(master=$MR slave=$SR). tail:"; tail -6 /tmp/mn_build_master.log /tmp/mn_build_slave.log; exit 2; fi
fi

# ── 협역 워치독(계층 2층 — plan_26071019 §2.3): 컨테이너 기동 *전* 폴링 개시(로드 구간 커버) ──
#   필터 = 컨테이너명 공통 접두(mn-<config> — master/slave 양쪽 부분일치). 정지는 PID 기반만
#   (**pkill -f 금지** — 자기참조 부모셸 사망 exit144 선례, devlog_26062718).
WD_MAIN_PID=""; WD_SUB_PID=""
if [ "$WATCHDOG" = "1" ]; then
  WFILTER="${MC%-master}"
  MAIN_WATCHDOG="$REPO/.claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh"
  SUB_WATCHDOG_REL=".claude/runtime/host_safety/mem_watchdog.sh"
  [ -f "$MAIN_WATCHDOG" ] || { echo "[mn] FAIL: canonical host-safety watchdog absent: $MAIN_WATCHDOG"; exit 2; }
  bash "$MAIN_WATCHDOG" "$WFILTER" "${WATCHDOG_THRESH_MIB:-10240}" 2 >/tmp/mn_watchdog_master.log 2>&1 & WD_MAIN_PID=$!
  echo "[mn] 워치독(master) pid=$WD_MAIN_PID filter=$WFILTER thresh=${WATCHDOG_THRESH_MIB:-10240}MiB (/tmp/mn_watchdog_master.log)"
  if $SSH "$SUB_HOST" "bash -lc '[ -f $SUB_WORK_DIR/$SUB_WATCHDOG_REL ]'" 2>/dev/null; then
    # ⚠ 원격 백그라운드 detach — 3-FD 리다이렉트(</dev/null + ssh -n)만으론 여전히 hang(2026-07-11 hy3 serve#1 실증:
    #   슬레이브 워치독은 정상 기동했으나 command-substitution ssh 가 ~8분 안 끝나 서빙 전체 블록). 원인 = 원격
    #   백그라운드 프로세스가 ssh 세션 프로세스그룹에 남아 sshd 가 채널 EOF 를 안 보냄(stdin 분리만으론 부족).
    #   해소 = setsid(새 세션 완전 분리 → sshd 즉시 채널 close) + exit 0(원격 셸 즉시 종료) + timeout 20(백스톱:
    #   그래도 hang 시 20s 후 ssh 만 종료 — 워치독은 이미 기동·PID 는 이미 echo 됨). PID 캡처 동작 보존.
    WD_SUB_PID=$(timeout 20 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD setsid nohup bash $SUB_WATCHDOG_REL $WFILTER ${WATCHDOG_THRESH_MIB:-10240} 2 </dev/null >/tmp/mn_watchdog_slave.log 2>&1 & echo \$!; exit 0'" 2>/dev/null || true)
    echo "[mn] 워치독(slave) pid=${WD_SUB_PID:-?} (원격 /tmp/mn_watchdog_slave.log)"
  else
    echo "[mn] ⚠ 서브에 $SUB_WATCHDOG_REL 부재 — 슬레이브 워치독 생략(상시 systemd 층만. render_sub_env/sync_to_sub 재배달 필요)"
  fi
fi

# ── Ray 클러스터 기동 (master 먼저=head, slave 합류) ──
echo "[mn] master 기동(Ray head + serve)..."
env $MOUNTVARS docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master up -d >/dev/null 2>&1
echo "[mn] slave 기동(Ray worker, SSH)..."
$SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS $MOUNTVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave up -d'" >/dev/null 2>&1

# ── 준비 폴링: 엔드포인트 health(거짓양성 회피) ──
# READY_MAX(폴링 횟수×5s) 환경변수로 조정 가능 — 대형모델(예 Qwen3-Next-80B bf16 151GB CIFS 로드 ~11분
#   + KV/compile setup)은 기본 15분(180회)으로 부족 → READY_MAX=360(30분) 등으로 연장(testlog_26062501 결함).
echo "[mn] 엔드포인트 :$PORT health 폴링(2노드 분산 로드; READY_MAX=${READY_MAX:-180}회×5s ≈ $(( ${READY_MAX:-180} * 5 / 60 ))분)..."
READY=0
for i in $(seq 1 "${READY_MAX:-180}"); do
  [ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' http://localhost:$PORT/health 2>/dev/null)" = "200" ] && { echo "[mn] READY ~$((i*5))s"; READY=1; break; }
  docker ps --filter name="$MC" --filter status=running -q | grep -q . || { echo "[mn] master EXITED"; docker logs "$MC" 2>&1 | tail -12; break; }
  docker logs "$MC" 2>&1 | grep -qiE "CUDA out of memory|NCCL error|did not join|RuntimeError" && { echo "[mn] FAILURE(serve)"; docker logs "$MC" 2>&1 | grep -iE "out of memory|NCCL error|did not join|RuntimeError" | tail -3; break; }
  sleep 5
done

# 미준비 진단 보강: 워치독 트립 = 마진 결함 증거(plan_26071019 §5 판정축)를 표면화.
if [ "$READY" != "1" ] && [ "$WATCHDOG" = "1" ]; then
  grep -h "TRIP" /tmp/mn_watchdog_master.log 2>/dev/null | tail -3 | sed 's/^/[mn] watchdog(master): /'
  $SSH "$SUB_HOST" "grep -h TRIP /tmp/mn_watchdog_slave.log 2>/dev/null | tail -3" 2>/dev/null | sed 's/^/[mn] watchdog(slave): /'
fi

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
  # 워치독 정지 = PID 기반만(pkill -f 금지) → 잔여 페이지캐시 드랍(§4.1 ② — 헬퍼 설치 시 best-effort).
  [ -n "$WD_MAIN_PID" ] && kill "$WD_MAIN_PID" 2>/dev/null
  [ -n "$WD_SUB_PID" ] && $SSH "$SUB_HOST" "kill $WD_SUB_PID" 2>/dev/null
  [ -x /usr/local/sbin/vllm-drop-caches ] && sudo -n /usr/local/sbin/vllm-drop-caches >/dev/null 2>&1
  $SSH "$SUB_HOST" "[ -x /usr/local/sbin/vllm-drop-caches ] && sudo -n /usr/local/sbin/vllm-drop-caches" >/dev/null 2>&1
elif [ -n "$WD_MAIN_PID" ]; then
  echo "[mn] --keep-up: 워치독 유지(master pid=$WD_MAIN_PID · slave pid=${WD_SUB_PID:-없음}) — 정지는 kill <pid> 로만"
fi
echo "[mn] 종료코드 $RESULT"
exit $RESULT
