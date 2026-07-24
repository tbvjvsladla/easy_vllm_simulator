#!/bin/bash
# mem_watchdog.sh — GB10 통합메모리 OOM 호스트-하드다운 방지 안전망(ops backstop).
#   GB10 통합메모리는 GPU OOM 시 호스트 전체가 하드다운된다(GPU/호스트 메모리 풀 공유 →
#   ping-OK→No-route→down, 재부팅 필요). 이 워치독은 /proc/meminfo MemAvailable 을 폴링해
#   임계 밑으로 가면 대상 컨테이너를 docker kill 한다 → 호스트 대신 컨테이너를 희생.
#   (Ray master 는 그 워커 actor-unavailable 로 깨끗이 종료 → 호스트 보존.)
# 사용: mem_watchdog.sh [name_filter|@vllm] [threshold_mib] [interval_sec]
#   name_filter : docker ps --filter name=<filter> 부분일치(예: slave / master / vllm_trial)
#   생략 또는 '@vllm' = 광역 모드 — 이미지/이름에 vllm 이 포함된 전 컨테이너(trial 포함).
#     (systemd 상시 인스턴스용 — δ2-1 사고에서 name_filter 가 vllm_trial01 을 미커버한 갭의 교정.)
# env: MEMWATCH_HEARTBEAT_SEC(기본 15, 0=끔 — MemAvailable 시계열 1줄/주기 + 직전주기 MIN 병기, 트립
#      전조 사후분석용. 60→15 하향 근거: 2026-07-11 768k 크래시서 60s HB 가 임계-하 최종접근을 가림
#      (마지막 샘플 11099MiB 후 무음) → testlog_26071111 §0 P2, 재현 반증가능성 확보)
#      MEMWATCH_PIDFILE(지정 시 자기 PID 기록 — 하네스가 PID 기반으로 정리)
# 정지: kill <pid> 로만. **pkill -f mem_watchdog 금지** — 호출자 자신의 명령줄을 매칭해
#      부모 셸이 죽는 자기참조 버그 선례(exit 144, devlog_26062718).
# ⚠ 커버리지 경계: 이 워치독은 **컨테이너-레벨**(docker ps 열거→docker kill)이라 **빌드 평면
#      (buildkit·cicc·MoE-JIT 컴파일)은 못 잡는다**(빌드 프로세스는 docker ps 에 비표시). 빌드-OOM
#      시 트립하면 무고한 serve 컨테이너만 희생되고 빌드는 계속될 수 있다 → **빌드 평면 백스톱 =
#      프로세스-레벨 earlyoom(4% 최후선, install_host_safety.sh)**. 운영 권고: 대형 소스빌드는
#      기존 serve 를 down 시킨 뒤 수행(동시 가동 시 serve 오킬 위험).
# 임계 참고: gmu0.80 보수레시피의 healthy 바닥은 ~13GiB 까지 내려올 수 있음(plan_26062818 —
#      구 주석 "20-30GiB 바닥" 은 거짓으로 정정됨). 기본 10240MiB 는 그 아래 최후선.
# 근거: DeepSeek-V4-Flash serve#1 서브 OOM 하드다운(2026-06-28) · Qwen3.5-397B 선례(워치독 6× 보호)
#      · δ2-1/δ2-2 13+ 트립 100% 호스트 보존 · plan_26071019 §2.1-2.2 계층 방어.
set -u
FILTER="${1:-@vllm}"
THRESH_MIB="${2:-10240}"   # MemAvailable < 10 GiB → trip
INTERVAL="${3:-1}"
HB_SEC="${MEMWATCH_HEARTBEAT_SEC:-15}"
[ -n "${MEMWATCH_PIDFILE:-}" ] && echo "$$" > "$MEMWATCH_PIDFILE"
ts(){ date -u +%FT%TZ; }
targets(){
  if [ "$FILTER" = "@vllm" ]; then
    # 광역: 이미지명 또는 컨테이너명에 vllm 포함(easy-vllm*·multi-vllm·vllm_trial·mn-* serve 등)
    docker ps --filter status=running --format '{{.ID}} {{.Image}} {{.Names}}' 2>/dev/null \
      | awk 'tolower($0) ~ /vllm/ {print $1}'
  else
    docker ps --filter "name=$FILTER" --filter status=running -q
  fi
}
echo "[mem-watchdog] start filter='$FILTER' threshold=${THRESH_MIB}MiB interval=${INTERVAL}s heartbeat=${HB_SEC}s pid=$$ $(ts)"
last_hb=0
min_since_hb=999999999   # 직전 HB 이후 1s-폴 최저치(P2 — 임계-하 순간 dip 을 60s HB 가 놓치지 않게, testlog_26071111 §0)
while true; do
  avail_mib=$(( $(awk '/MemAvailable:/{print $2}' /proc/meminfo) / 1024 ))
  [ "$avail_mib" -lt "$min_since_hb" ] && min_since_hb=$avail_mib
  now=$(date +%s)
  if [ "$HB_SEC" -gt 0 ] && [ $(( now - last_hb )) -ge "$HB_SEC" ]; then
    echo "[mem-watchdog] HB MemAvailable=${avail_mib}MiB min=${min_since_hb}MiB $(ts)"
    last_hb=$now
    min_since_hb=$avail_mib
  fi
  if [ "$avail_mib" -lt "$THRESH_MIB" ]; then
    ids=$(targets)
    if [ -n "$ids" ]; then
      echo "[mem-watchdog] TRIP MemAvailable=${avail_mib}MiB < ${THRESH_MIB}MiB → docker kill $ids $(ts)"
      docker kill $ids 2>&1 | sed 's/^/[mem-watchdog] /'
    else
      # 매칭 0 인데 임계 미달 = vllm 밖 원인(관측만) — 로그 폭주 방지 감속
      echo "[mem-watchdog] TRIP-nomatch MemAvailable=${avail_mib}MiB < ${THRESH_MIB}MiB (filter='$FILTER' 매칭 0) $(ts)"
      sleep 5
    fi
  fi
  sleep "$INTERVAL"
done
