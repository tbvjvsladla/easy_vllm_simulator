#!/bin/bash
# mem_watchdog_eta.sh — ETA 기반 반사 방어 (plan_26073109 §2.2 · 레거시 mem_watchdog.sh 후계).
#
#   레거시와의 차이: 임계가 **절대 GiB 고정 → ETA(0 까지 남은 시간)** 로 바뀐다.
#   근거(testlog_26073109): 하강률 동적 범위가 p50 0.13 ~ max 6,241 MiB/s = 약 48,000배 →
#   단일 절대값은 급락에선 늦고 완만 구간에선 오발한다(7/22 10,186 MiB kill = 실여유 230초였음).
#
#   판정식 (blackbox_eta.py 가 emit 한 상수를 그대로 쓴다 — 파이썬 비의존 정수 산술):
#       TRIP_NOW  <=>  mem_mib <= BB_HARD_FLOOR_MIB                      # 최후 절대 바닥
#                  ||  ( rate >= BB_MIN_RATE  &&  mem <= BB_ARM_CEILING_MIB
#                        &&  mem*1000 <= rate*BB_RUNWAY_MS )
#       실제 kill <=>  TRIP_NOW 가 **연속 BB_DEBOUNCE_N 폴** 지속        # 급락-후-정지(모델 로드) 제거
#
#   ★ BB_ARM_CEILING_MIB = **선언된 바닥**(testlog_26073123 · 2026-08-01 사용자 승인).
#     근거: ETA 규칙은 `잔량 ÷ 하강률` 의 선형 외삽이다. 이 외삽은 하강이 계속된다는 가정 위에
#     서 있는데 **모델 로드의 하강은 체크포인트 크기로 유계**다. 유계인 것을 무계로 읽어
#     58 GiB 급 모델 로드를 3회 중 3회 사살했다(잔량 57 GiB — 레거시 절대임계의 5.6배).
#     더 나쁜 것은 발동 지점이 하강의 **꼬리**라는 점이다: 감속하면 분모가 줄지만 분자(잔량)도
#     이미 그만큼 줄어 비율이 회복되지 않는다 → 위험이 끝나는 순간 발동한다.
#
#     진성/위양성은 (잔량, 하강률) 평면에서 **분리되지 않는다**(둘 다 3초 버스트·25 GiB/s·완전회복,
#     오히려 위양성 쪽 여유가 2배 컸다). 임계 조정으로는 못 푼다 — 빠진 것은 임계값이 아니라
#     **"이 하강은 유계이며 바닥이 어디인가"** 라는 정보다. 그래서 임계를 낮추는 대신
#     서빙이 그 정보를 **선언**하게 하고, 규칙은 선언된 바닥 아래에서만 무장한다.
#
#     이 설계가 진성을 보존하는 이유: 유일한 진성(KV 벌룬 97 GiB)은 `kv_cache_memory_bytes: null`
#     이라 **애초에 선언할 수 없는** 사건이었다. 선언이 없으면 상한은 무한대 = 현행 규칙 그대로다.
#     선언보다 더 떨어지면 규칙이 재무장하므로 선언 후의 벌룬도 여전히 잡힌다.
#     즉 게이트를 눈멀게 하는 것이 아니라 빠진 정보를 보충하는 것이며,
#     헌법 `policy:KV_ABSOLUTE_CLAMP_PORTABILITY`(KV 절대클램프 의무)를 강화한다.
#
#   ★ 킬 대상 선별(@vllm 광역)은 레거시와 **동일하게 유지**한다 — plan R4 "한 번에 한 변수".
#     선별 정밀화는 다음 사이클(plan §6).
#
# 사용: mem_watchdog_eta.sh [name_filter|@vllm] [interval_sec]
# env : BB_PARAMS      (기본 /etc/easy-vllm/eta_params.env — blackbox_eta.py --emit-params 산출)
#       BB_EVENTS      (지정 시 트립/킬 이벤트를 JSONL append — 블랙박스 events 평면)
#       BB_DECL        (서빙 예산 선언 파일 — 기본 = BB_EVENTS 의 조부모/serve_budget.env.
#                       blackbox_session.py declare-budget 가 쓴다. **source 하지 않는다** —
#                       루트 데몬이 비루트 작성 파일을 source 하면 코드실행이다. sed 로만 읽는다.)
#       BB_DECL_MARGIN_MIB      (기본 8192 — 선언 바닥에서 뺄 여유. arm 상한 = 바닥 - 여유)
#       BB_DECL_MIN_CEILING_MIB (기본 16384 — arm 상한이 이보다 낮아지는 선언은 **거부**.
#                                선언으로 게이트를 실명시킬 수 없게 하는 하드가드)
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
BB_DECL_MARGIN_MIB="${BB_DECL_MARGIN_MIB:-8192}"
BB_DECL_MIN_CEILING_MIB="${BB_DECL_MIN_CEILING_MIB:-16384}"
# 선언 파일 기본 경로 = <node_dir>/serve_budget.env (BB_EVENTS 가 <node_dir>/events/*.jsonl 이므로 조부모)
if [ -z "${BB_DECL:-}" ]; then
  if [ -n "$BB_EVENTS" ]; then
    BB_DECL="$(dirname "$(dirname "$BB_EVENTS")")/serve_budget.env"
  else
    BB_DECL="/run/easy-vllm/serve_budget.env"
  fi
fi
# arm 상한: 선언이 없으면 사실상 무한대 = 현행 규칙 그대로(fail-safe 방향)
BB_ARM_CEILING_MIB=999999999
BB_DECL_STATE=""

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

# ── 선언 파일 읽기 ────────────────────────────────────────────────────────
# **source 금지**. 값 문자셋을 [A-Za-z0-9._-]{1,64} 로 좁혀 주입 여지를 없앤다.
_decl_field(){ # $1=key → stdout=값(없으면 빈 문자열)
  [ -f "$BB_DECL" ] || return 0
  sed -n "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*\([A-Za-z0-9._-]\{1,64\}\)[[:space:]]*\$/\1/p" \
    "$BB_DECL" 2>/dev/null | head -1
}

# 선언을 재평가해 BB_ARM_CEILING_MIB 를 갱신한다. 상태 전이만 이벤트로 남긴다(폴마다 쓰면 홍수).
# 어느 경로로든 실패하면 상한을 무한대로 되돌린다 = 현행 규칙 복귀(fail-safe).
refresh_decl(){
  local floor exp now ceiling state reason extra
  state="none"; reason=""; extra=""
  BB_ARM_CEILING_MIB=999999999
  if [ -f "$BB_DECL" ]; then
    floor="$(_decl_field floor_mib)"; exp="$(_decl_field expires_epoch)"
    case "$floor" in ''|*[!0-9]*) floor="" ;; esac
    case "$exp"   in ''|*[!0-9]*) exp=""   ;; esac
    now=$(date +%s)
    if [ -z "$floor" ] || [ -z "$exp" ]; then
      state="rejected"; reason="floor_mib/expires_epoch 파싱 실패"
    elif [ "$exp" -le "$now" ]; then
      state="expired"; reason="만료(expires_epoch=$exp <= now=$now)"
    else
      ceiling=$(( floor - BB_DECL_MARGIN_MIB ))
      if [ "$ceiling" -lt "$BB_DECL_MIN_CEILING_MIB" ]; then
        state="rejected"
        reason="arm 상한 ${ceiling}MiB < 최소 ${BB_DECL_MIN_CEILING_MIB}MiB — 선언이 게이트를 실명시킨다"
      else
        BB_ARM_CEILING_MIB="$ceiling"; state="honored"
        extra=",\"floor_mib\":$floor,\"arm_ceiling_mib\":$ceiling,\"expires_epoch\":$exp"
        extra="$extra,\"label\":\"$(_decl_field label)\""
      fi
    fi
  fi
  if [ "$state" != "$BB_DECL_STATE" ]; then
    log "DECL $state ${reason:+($reason) }상한=${BB_ARM_CEILING_MIB}MiB file=$BB_DECL $(ts)"
    emit_event "budget_$state" "\"arm_ceiling_mib\":$BB_ARM_CEILING_MIB${reason:+,\"reason\":\"$reason\"}$extra"
    BB_DECL_STATE="$state"
  fi
}

# ── 순수 판정(자체시험 대상) : $1=mem_mib $2=rate_mib_s(양수=하강) → 0=TRIP_NOW · 1=아님 ──
# BB_ARM_CEILING_MIB 는 전역이다(선언 상태에 따라 refresh_decl 이 갱신). 기본=무한대.
bb_trip_now(){
  local mem="$1" rate="$2"
  [ "$mem" -le "$BB_HARD_FLOOR_MIB" ] && return 0   # 절대 바닥은 선언과 무관하게 항상 무장
  [ "$rate" -lt "$BB_MIN_RATE_MIB_S" ] && return 1
  [ "$mem" -gt "$BB_ARM_CEILING_MIB" ] && return 1  # 선언된 바닥 위 = 예상된 유계 하강
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

  # ── 선언된 바닥(BB_ARM_CEILING_MIB) — testlog_26073123 실측 재생 ──────────
  echo "선언된 바닥:"
  _saved_ceiling=$BB_ARM_CEILING_MIB
  # (a) 선언 없음 = 무한대 → 현행과 동일해야 한다(회귀 방지: 진성 KV 벌룬 3폴 그대로 발동)
  BB_ARM_CEILING_MIB=999999999
  printf '%s\n' "74181 27893" "51848 22333" "28686 23162" \
    | seq_test "  진성 KV벌룬(선언 없음) → 발동" 0
  # (b) 위양성 GLM 로드 3회 실측 궤적. 선언 바닥 40960 → 상한 40960-8192=32768
  BB_ARM_CEILING_MIB=32768
  printf '%s\n' "112195 4127" "88000 24195" "63316 24684" "59894 3422" \
    | seq_test "  위양성 2차 로드(선언 有) → 미발동" 1
  printf '%s\n' "106510 9938" "82338 24172" "57986 24352" "59881 0" \
    | seq_test "  위양성 3차 로드(선언 有) → 미발동" 1
  printf '%s\n' "97363 18244" "72734 24629" "56839 15895" \
    | seq_test "  위양성 1차 로드(선언 有) → 미발동" 1
  # (c) ★ 선언이 있어도 그 아래로 뚫으면 재무장해야 한다 — 선언 후 벌룬 시나리오
  printf '%s\n' "31000 12000" "20000 11000" "9000 11000" \
    | seq_test "  선언 아래로 관통(벌룬) → 발동" 0
  # (d) 절대 바닥은 선언과 무관하게 무장
  BB_ARM_CEILING_MIB=999999999
  chk "  선언 무관 절대바닥 → TRIP"          0 4000 0
  BB_ARM_CEILING_MIB=$_saved_ceiling

  # ── 선언 파일 파싱·거부 (refresh_decl) ───────────────────────────────────
  echo "선언 파일 파싱:"
  _tmpd=$(mktemp -d); BB_DECL="$_tmpd/serve_budget.env"; BB_DECL_STATE=""
  _future=$(( $(date +%s) + 3600 )); _past=$(( $(date +%s) - 10 ))
  dchk(){ # $1=설명 $2=기대state $3=기대상한(""=무시)
    refresh_decl >/dev/null
    if [ "$BB_DECL_STATE" = "$2" ] && { [ -z "$3" ] || [ "$BB_ARM_CEILING_MIB" = "$3" ]; }; then
      echo "  [PASS] $1"
    else
      echo "  [FAIL] $1 (기대=$2/$3 실제=$BB_DECL_STATE/$BB_ARM_CEILING_MIB)"; fails=1
    fi
    BB_DECL_STATE=""
  }
  rm -f "$BB_DECL";                                        dchk "파일 없음 → none·무한대"       none      999999999
  printf 'floor_mib=40960\nexpires_epoch=%s\nlabel=glm-47-flash\n' "$_future" > "$BB_DECL"
                                                           dchk "정상 선언 → honored·상한 32768" honored   32768
  printf 'floor_mib=40960\nexpires_epoch=%s\n' "$_past"    > "$BB_DECL"
                                                           dchk "만료 → expired·무한대"          expired   999999999
  printf 'floor_mib=20000\nexpires_epoch=%s\n' "$_future"  > "$BB_DECL"
                                                           dchk "상한 11808<16384 → rejected"     rejected  999999999
  printf 'floor_mib=abc\nexpires_epoch=%s\n' "$_future"    > "$BB_DECL"
                                                           dchk "비숫자 floor → rejected"         rejected  999999999
  printf 'expires_epoch=%s\n' "$_future"                   > "$BB_DECL"
                                                           dchk "floor 누락 → rejected"           rejected  999999999
  # ★ 주입 방어: source 했다면 부수효과가 남는다. sed 파싱이면 값이 문자셋에 걸려 거부된다.
  _CANARY=clean
  printf 'floor_mib=40960\nexpires_epoch=%s\n_CANARY=pwned\nlabel=$(id -u)\n' "$_future" > "$BB_DECL"
  refresh_decl >/dev/null
  if [ "$_CANARY" = "clean" ]; then echo "  [PASS] 선언 파일을 source 하지 않는다(주입 무효)"
  else echo "  [FAIL] 선언 파일이 실행됐다 — _CANARY=$_CANARY"; fails=1; fi
  rm -rf "$_tmpd"; BB_DECL_STATE=""; BB_ARM_CEILING_MIB=999999999

  [ "$fails" = 0 ] && { echo "self-test: PASS"; exit 0; } || { echo "self-test: FAIL"; exit 2; }
fi

# ── 운전 루프 ────────────────────────────────────────────────────────────
[ -n "${MEMWATCH_PIDFILE:-}" ] && echo "$$" > "$MEMWATCH_PIDFILE"
log "start filter='$FILTER' interval=${INTERVAL}s params=$PARAMS_SRC floor=${BB_HARD_FLOOR_MIB}MiB runway=${BB_RUNWAY_MS}ms debounce=${BB_DEBOUNCE_N} decl=$BB_DECL pid=$$ $(ts)"
emit_event "watchdog_start" "\"filter\":\"$FILTER\",\"runway_ms\":$BB_RUNWAY_MS,\"debounce\":$BB_DEBOUNCE_N,\"hard_floor_mib\":$BB_HARD_FLOOR_MIB,\"decl_path\":\"$BB_DECL\",\"decl_margin_mib\":$BB_DECL_MARGIN_MIB"

prev_mem=""; streak=0; last_hb=0; min_since_hb=999999999
while true; do
  refresh_decl                      # 선언은 매 폴 재평가한다(선언 소멸이 즉시 규칙 복귀가 되도록)
  mem=$(( $(awk '/MemAvailable:/{print $2}' /proc/meminfo) / 1024 ))
  if [ -n "$prev_mem" ]; then rate=$(( (prev_mem - mem) / INTERVAL )); else rate=0; fi
  prev_mem="$mem"
  [ "$mem" -lt "$min_since_hb" ] && min_since_hb=$mem

  now=$(date +%s)
  if [ "$HB_SEC" -gt 0 ] && [ $(( now - last_hb )) -ge "$HB_SEC" ]; then
    log "HB MemAvailable=${mem}MiB min=${min_since_hb}MiB rate=${rate}MiB/s streak=${streak} decl=${BB_DECL_STATE}/${BB_ARM_CEILING_MIB}MiB $(ts)"
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
