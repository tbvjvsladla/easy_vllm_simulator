#!/bin/bash
# mem_watchdog_eta.sh — ETA 기반 반사 방어 (plan_26073109 §2.2 · 레거시 mem_watchdog.sh 후계).
#
#   레거시와의 차이: 임계가 **절대 GiB 고정 → ETA(0 까지 남은 시간)** 로 바뀐다.
#   근거(testlog_26073109): 하강률 동적 범위가 p50 0.13 ~ max 6,241 MiB/s = 약 48,000배 →
#   단일 절대값은 급락에선 늦고 완만 구간에선 오발한다(7/22 10,186 MiB kill = 실여유 230초였음).
#
#   판정식 (blackbox_eta.py 가 emit 한 상수를 그대로 쓴다 — 파이썬 비의존 정수 산술):
#       TRIP_NOW  <=>  mem_mib <= BB_HARD_FLOOR_MIB                      # 최후 절대 바닥
#                  ||  ( rate >= BB_MIN_RATE  &&  mem*1000 <= rate*BB_RUNWAY_MS )
#       실제 kill <=>  TRIP_NOW 가 **연속 BB_DEBOUNCE_N 폴** 지속        # 급락-후-정지(모델 로드) 제거
#
#   ★ 킬 대상 선별(@vllm 광역)은 레거시와 **동일하게 유지**한다 — plan R4 "한 번에 한 변수".
#     선별 정밀화는 다음 사이클(plan §6).
#
# 사용: mem_watchdog_eta.sh [name_filter|@vllm] [interval_sec]
# env : BB_PARAMS      (기본 /etc/easy-vllm/eta_params.env — blackbox_eta.py --emit-params 산출)
#       BB_EVENTS      (지정 시 트립/킬 이벤트를 JSONL append — 블랙박스 events 평면)
#       MEMWATCH_HEARTBEAT_SEC (기본 15 · 0=끔)
#       MEMWATCH_PIDFILE
# 정지: kill <pid> 로만. **pkill -f 금지**(자기참조 부모셸 사망 선례 exit144, devlog_26062718).
# 종료코드: 0=정상종료 · 1=전제 실패 · 2=자체시험 실패.
set -u

SELFTEST=0
[ "${1:-}" = "--self-test" ] && SELFTEST=1

FILTER="${1:-@vllm}"; [ "$SELFTEST" = 1 ] && FILTER="@vllm"
INTERVAL="${2:-1}"
HB_SEC="${MEMWATCH_HEARTBEAT_SEC:-15}"
BB_PARAMS="${BB_PARAMS:-/etc/easy-vllm/eta_params.env}"
BB_EVENTS="${BB_EVENTS:-}"

# ── 파라미터 로드(없으면 보수 기본값 — fail-safe: 파라미터 부재가 무방비를 뜻하지 않게) ──
BB_HARD_FLOOR_MIB=5120; BB_RUNWAY_MS=8000; BB_DEBOUNCE_N=3; BB_MIN_RATE_MIB_S=1
if [ -f "$BB_PARAMS" ]; then
  # shellcheck disable=SC1090
  . "$BB_PARAMS"
  PARAMS_SRC="$BB_PARAMS"
else
  PARAMS_SRC="(내장 기본값 — $BB_PARAMS 부재)"
fi
# 소수점이 섞여 오면 정수부만 취한다(셸 산술은 정수 전용)
BB_HARD_FLOOR_MIB=${BB_HARD_FLOOR_MIB%%.*}
BB_MIN_RATE_MIB_S=${BB_MIN_RATE_MIB_S%%.*}
BB_DEBOUNCE_N=${BB_DEBOUNCE_N%%.*}
BB_RUNWAY_MS=${BB_RUNWAY_MS%%.*}

ts(){ date -u +%FT%TZ; }
log(){ echo "[bb-watchdog] $*"; }

emit_event(){ # $1=kind $2=json-fragment
  [ -n "$BB_EVENTS" ] || return 0
  printf '{"ts":"%s","kind":"%s","source":"mem_watchdog_eta"%s}\n' \
    "$(ts)" "$1" "${2:+,$2}" >> "$BB_EVENTS" 2>/dev/null || true
}

# ── 순수 판정(자체시험 대상) : $1=mem_mib $2=rate_mib_s(양수=하강) → 0=TRIP_NOW · 1=아님 ──
bb_trip_now(){
  local mem="$1" rate="$2"
  [ "$mem" -le "$BB_HARD_FLOOR_MIB" ] && return 0
  [ "$rate" -lt "$BB_MIN_RATE_MIB_S" ] && return 1
  [ $(( mem * 1000 )) -le $(( rate * BB_RUNWAY_MS )) ] && return 0
  return 1
}

targets(){
  if [ "$FILTER" = "@vllm" ]; then
    docker ps --filter status=running --format '{{.ID}} {{.Image}} {{.Names}}' 2>/dev/null \
      | awk 'tolower($0) ~ /vllm/ {print $1}'
  else
    docker ps --filter "name=$FILTER" --filter status=running -q
  fi
}

# ── 자체시험 (하드웨어·도커 불요) ────────────────────────────────────────
if [ "$SELFTEST" = 1 ]; then
  fails=0
  chk(){ # $1=설명 $2=기대(0/1) $3=mem $4=rate
    bb_trip_now "$3" "$4"; local got=$?
    if [ "$got" = "$2" ]; then echo "  [PASS] $1"; else echo "  [FAIL] $1 (기대=$2 실제=$got)"; fails=1; fi
  }
  echo "파라미터: floor=${BB_HARD_FLOOR_MIB}MiB runway=${BB_RUNWAY_MS}ms debounce=${BB_DEBOUNCE_N} min_rate=${BB_MIN_RATE_MIB_S}"
  chk "최후 바닥 밑 → TRIP"                       0 4000   0
  chk "바닥 경계값 포함 → TRIP"                   0 5120   0
  chk "바닥 위·정지 → 무트립"                     1 6000   0
  chk "바닥 위·느린 하강(min_rate 미만) → 무트립" 1 60000  0
  chk "7/22 오발 지점(10186 @44MiB/s) → 무트립"   1 10186 44
  chk "급락 2GiB/s·잔량 5GiB → TRIP(바닥)"        0 5000  2000
  chk "급락 6.2GiB/s·잔량 37GiB → TRIP"           0 37000 6241
  chk "급락 6.2GiB/s·잔량 60GiB → 무트립"         1 60000 6241
  chk "정상 서빙(120GiB·1MiB/s) → 무트립"         1 120000 1

  # 디바운스 상태기계: 급락-후-정지 시퀀스가 걸러지는가
  seq_test(){ # $1=설명 $2=기대발동(0/1) ; stdin = "mem rate" 줄들
    local streak=0 fired=1
    while read -r m r; do
      if bb_trip_now "$m" "$r"; then
        streak=$((streak+1)); [ "$streak" -ge "$BB_DEBOUNCE_N" ] && fired=0
      else streak=0; fi
    done
    if [ "$fired" = "$2" ]; then echo "  [PASS] $1"; else echo "  [FAIL] $1 (기대=$2 실제=$fired)"; fails=1; fi
  }
  # 1폴만 트립조건 성립 후 정지 → 디바운스가 걸러야 함
  printf '%s\n' "37000 6241" "20000 100" "19800 10" | seq_test "급락 1폴 후 정지 → 미발동" 1
  # 3폴 연속 지속 → 발동
  printf '%s\n' "37000 6241" "30800 6200" "24600 6200" | seq_test "급락 3폴 지속 → 발동" 0
  # 2폴만 지속 → 미발동 (경계)
  printf '%s\n' "37000 6241" "30800 6200" "31000 0" | seq_test "급락 2폴 후 회복 → 미발동" 1

  [ "$fails" = 0 ] && { echo "self-test: PASS"; exit 0; } || { echo "self-test: FAIL"; exit 2; }
fi

# ── 운전 루프 ────────────────────────────────────────────────────────────
[ -n "${MEMWATCH_PIDFILE:-}" ] && echo "$$" > "$MEMWATCH_PIDFILE"
log "start filter='$FILTER' interval=${INTERVAL}s params=$PARAMS_SRC floor=${BB_HARD_FLOOR_MIB}MiB runway=${BB_RUNWAY_MS}ms debounce=${BB_DEBOUNCE_N} pid=$$ $(ts)"
emit_event "watchdog_start" "\"filter\":\"$FILTER\",\"runway_ms\":$BB_RUNWAY_MS,\"debounce\":$BB_DEBOUNCE_N,\"hard_floor_mib\":$BB_HARD_FLOOR_MIB"

prev_mem=""; streak=0; last_hb=0; min_since_hb=999999999
while true; do
  mem=$(( $(awk '/MemAvailable:/{print $2}' /proc/meminfo) / 1024 ))
  if [ -n "$prev_mem" ]; then rate=$(( (prev_mem - mem) / INTERVAL )); else rate=0; fi
  prev_mem="$mem"
  [ "$mem" -lt "$min_since_hb" ] && min_since_hb=$mem

  now=$(date +%s)
  if [ "$HB_SEC" -gt 0 ] && [ $(( now - last_hb )) -ge "$HB_SEC" ]; then
    log "HB MemAvailable=${mem}MiB min=${min_since_hb}MiB rate=${rate}MiB/s streak=${streak} $(ts)"
    last_hb=$now; min_since_hb=$mem
  fi

  if bb_trip_now "$mem" "$rate"; then
    streak=$(( streak + 1 ))
    [ "$streak" -eq 1 ] && log "TRIP-ARM mem=${mem}MiB rate=${rate}MiB/s (디바운스 1/${BB_DEBOUNCE_N}) $(ts)"
    if [ "$streak" -ge "$BB_DEBOUNCE_N" ]; then
      ids=$(targets)
      if [ -n "$ids" ]; then
        log "TRIP mem=${mem}MiB rate=${rate}MiB/s streak=${streak} → docker kill $ids $(ts)"
        emit_event "watchdog_trip" "\"mem_avail_mib\":$mem,\"rate_mib_s\":$rate,\"streak\":$streak,\"targets\":\"$ids\",\"action\":\"docker_kill\""
        docker kill $ids 2>&1 | sed 's/^/[bb-watchdog] /'
        emit_event "watchdog_kill_ack" "\"targets\":\"$ids\""
        streak=0; prev_mem=""     # 킬 후 회수까지의 급반등을 하강률로 오독하지 않도록 리셋
      else
        log "TRIP-nomatch mem=${mem}MiB rate=${rate}MiB/s (filter='$FILTER' 매칭 0) $(ts)"
        emit_event "watchdog_trip_nomatch" "\"mem_avail_mib\":$mem,\"rate_mib_s\":$rate"
        sleep 5; prev_mem=""
      fi
    fi
  else
    [ "$streak" -gt 0 ] && log "TRIP-DISARM mem=${mem}MiB rate=${rate}MiB/s (지속 실패 — 급락 후 정지) $(ts)"
    streak=0
  fi
  sleep "$INTERVAL"
done
