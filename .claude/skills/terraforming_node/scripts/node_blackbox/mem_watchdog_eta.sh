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
#                        &&  ( rate >= BB_MAX_RATE_MIB_S ? mem <= BB_ABS_BAND_MIB   # 이중규칙
#                                                        : mem*1000 <= rate*BB_RUNWAY_MS ) )
#       실제 kill <=>  TRIP_NOW 가 **연속 BB_DEBOUNCE_N 폴** 지속        # 급락-후-정지(모델 로드) 제거
#
#   ★ 이중규칙(plan_26081415 C1-A · 2026-08-14). BB_MAX_RATE_MIB_S = mem_total/runway 파생값이며
#     이 하강률 위에서 ETA 부등식은 `mem <= mem_total` 과 같아져 **잔량과 무관하게 항상 참**이 된다
#     — 판정이 아니라 상수다. 1 Hz 실샘플 전수에서 그 구간은 모델 로드일에만 출현했고(유휴일 0폴),
#     R0 에서 정상 로드를 2회 사살했다(50,772@21,943 · 35,158@22,799). 그래서 그 구간만 절대 잔량
#     밴드로 판정을 바꾼다. BB_MAX_RATE_MIB_S=0 = mem_total 미상 = **이중규칙 OFF**(옛 규칙 유지 —
#     파생 실패가 규칙 완화로 이어지지 않게 하는 fail-safe 방향).
#
#     ⚠ **음성정직**: 상한 위에서는 진성도 사실상 포기한다. 유일한 진성(KV 벌룬)은 밴드를 밟지
#       않고 지나가며, 23 GiB/s 에서 10 GiB 구간 체류는 0.44 초라 1 Hz x 디바운스 3 으로 못 잡는다.
#       그 구간의 방어는 **선언 필수화(C1-B/C3)와 earlyoom(4% 최후선)** 에 위임된다.
#       상세 근거는 blackbox_eta.py 헤더 §이중규칙.
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
#       BB_DRY_RUN     (1 = **관측 전용 무장**. 판정은 그대로 하되 docker kill 을 하지 않고
#                       "실무장이었으면 kill 했을 판정" 을 로그·이벤트로만 남긴다.
#                       plan_26081415 C1-5: 워치독은 하드다운 최후 방어선이라 규칙 교체를
#                       즉시 실무장하지 않는다 — dry-run 로드 1회 관측이 실무장의 전제다.)
# 정지: kill <pid> 로만. **pkill -f 금지**(자기참조 부모셸 사망 선례 exit144, devlog_26062718).
# 종료코드: 0=정상종료 · 1=전제 실패 · 2=자체시험 실패.
set -u

SELFTEST=0
[ "${1:-}" = "--self-test" ] && SELFTEST=1

# dry-run 은 env 또는 첫 인자로 켠다. 인자를 받는 이유: systemd 유닛을 건드리지 않고
# **별도 인스턴스**를 손으로 띄워 관측하는 것이 실무장 전 정상 경로이기 때문이다(C1-5).
DRY_RUN="${BB_DRY_RUN:-0}"
if [ "${1:-}" = "--dry-run" ]; then DRY_RUN=1; shift; fi
case "$DRY_RUN" in 1|true|TRUE|yes) DRY_RUN=1 ;; *) DRY_RUN=0 ;; esac
MODE="armed"; [ "$DRY_RUN" = 1 ] && MODE="dry-run"

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
# 이중규칙 기본값. **정본은 eta_params.env**(blackbox_eta.DEFAULTS 가 생성) — 아래 source 가 덮는다.
#   BB_MAX_RATE_MIB_S=0 은 "미상 → 이중규칙 OFF" 다. 파라미터 파일이 없을 때 상한을 추측해
#   규칙을 완화하면 파일 부재가 곧 무방비가 된다 — 부재의 기본값은 항상 **옛 규칙(더 죽이는 쪽)**.
BB_MAX_RATE_MIB_S=0; BB_ABS_BAND_MIB=10240
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
BB_MAX_RATE_MIB_S=${BB_MAX_RATE_MIB_S%%.*}
BB_ABS_BAND_MIB=${BB_ABS_BAND_MIB%%.*}
# 선언된 바닥 상수도 정본은 eta_params.env 다(위 source 가 여기 기본값을 덮는다). 같은 절삭 적용.
BB_DECL_MARGIN_MIB=${BB_DECL_MARGIN_MIB%%.*}
BB_DECL_MIN_CEILING_MIB=${BB_DECL_MIN_CEILING_MIB%%.*}
# ── 만료 임박 경고 문턱 (plan_26081415 C4-2) ────────────────────────────────
#   현행은 만료 **순간**의 `budget_expired` 뿐이라 에이전트가 선제 대응할 수 없다 — 그 이벤트가
#   나온 시점에는 이미 옛 규칙(무조건-트립)으로 돌아가 있다. 경고는 그 전에 나와야 쓸모가 있다.
#   문턱은 손으로 적지 않고 **에이전트 알림 밴드**(BB_AGENT_NOTIFY_S · eta_params.env)에서 가져온다:
#   "에이전트가 반응하는 데 필요한 선행시간" 은 ETA 엔진이 이미 정의해 둔 값이며, 여기서 다른
#   숫자를 새로 만들면 같은 개념이 두 곳에 손으로 적힌다(4종 안티패턴 `매직넘버·결함`).
BB_DECL_EXPIRING_S="${BB_DECL_EXPIRING_S:-${BB_AGENT_NOTIFY_S:-900}}"
BB_DECL_EXPIRING_S=${BB_DECL_EXPIRING_S%%.*}

ts(){ date -u +%FT%TZ; }
log(){ echo "[bb-watchdog] $*"; }

emit_event(){ # $1=kind $2=json-fragment
  [ -n "$BB_EVENTS" ] || return 0
  local _new=0
  [ -e "$BB_EVENTS" ] || _new=1
  # ★ `mode` 를 모든 이벤트에 싣는다. dry-run 관측 인스턴스와 실무장 데몬이 **같은 events 파일에
  #   함께 append** 하므로(관측은 유닛을 건드리지 않고 별도 프로세스로 한다), 이 필드가 없으면
  #   나중에 "이 트립은 죽인 것인가 관측인가"를 데이터에서 가를 수 없다(§결정론 규율 — 출처 표시).
  printf '{"ts":"%s","kind":"%s","source":"mem_watchdog_eta","mode":"%s"%s}\n' \
    "$(ts)" "$1" "$MODE" "${2:+,$2}" >> "$BB_EVENTS" 2>/dev/null || true
  # ★ root 가 새로 만든 이벤트 파일은 **디렉터리 소유자에게 넘긴다**. 이 파일은 root 데몬과
  #   비-root 사용자 도구(blackbox_session declare-budget)가 함께 append 하는데, install 은
  #   디렉터리만 위임 사용자 소유로 만들어 **그 달 먼저 쓴 쪽이 소유자**가 된다. root 가 이기면
  #   사용자 도구가 EACCES 로 죽고 ETA 위양성 방어(선언된 바닥)가 통째로 못 선다
  #   (2026-08-01 서브에서 실제 발생 — 메인은 우연히 사용자 도구가 먼저 써서 멀쩡했다).
  if [ "$_new" = "1" ] && [ "$(id -u)" = "0" ] && [ -e "$BB_EVENTS" ]; then
    chown --reference="$(dirname "$BB_EVENTS")" "$BB_EVENTS" 2>/dev/null || true
  fi
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
  local floor exp now ceiling state reason extra _rem
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
        # ★ 만료 임박은 **여전히 무장 상태**다 — 상한을 그대로 두고 상태만 가른다(C4-2).
        #   상태를 가르는 것만으로 기존 전이-전용 emit 이 경고를 정확히 1회 내보낸다(폴마다 쓰면 홍수).
        #   honored → expiring → expired 순으로 흐르며, 갱신(renew-budget)이 들어오면 expiring →
        #   honored 로 되돌아가 다음 임박 때 다시 1회 경고한다.
        _rem=$(( exp - now ))
        [ "$_rem" -le "$BB_DECL_EXPIRING_S" ] && state="expiring"
        # ⚠ `arm_ceiling_mib` 는 emit_event 의 기본 필드에 이미 들어간다 — 여기서 또 넣으면
        #   JSON 키가 중복돼 파서마다 다른 값을 집는다(json.loads 는 뒤엣것을 취해 조용히 통과).
        #   2026-08-14 C3 검증 중 실제 산출물에서 발견. 중복을 만들지 않는다.
        extra=",\"floor_mib\":$floor,\"expires_epoch\":$exp,\"remaining_s\":$_rem"
        extra="$extra,\"label\":\"$(_decl_field label)\""
        [ "$state" = "expiring" ] \
          && reason="만료 임박(잔여 ${_rem}s <= ${BB_DECL_EXPIRING_S}s) — 갱신: blackbox_session.py renew-budget"
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
# BB_TRIP_RULE 에 **어느 규칙이 판정했는지**를 남긴다(이벤트·로그의 출처 표시용).
BB_TRIP_RULE="none"
bb_trip_now(){
  local mem="$1" rate="$2"
  BB_TRIP_RULE="hard_floor"
  [ "$mem" -le "$BB_HARD_FLOOR_MIB" ] && return 0   # 절대 바닥은 선언·하강률과 무관하게 항상 무장
  BB_TRIP_RULE="min_rate"
  [ "$rate" -lt "$BB_MIN_RATE_MIB_S" ] && return 1
  BB_TRIP_RULE="arm_ceiling"
  [ "$mem" -gt "$BB_ARM_CEILING_MIB" ] && return 1  # 선언된 바닥 위 = 예상된 유계 하강
  # ★ 이중규칙: 상한 위에서는 ETA 가 상수가 되므로 절대 잔량 밴드로 판정을 바꾼다.
  if [ "$BB_MAX_RATE_MIB_S" -gt 0 ] && [ "$rate" -ge "$BB_MAX_RATE_MIB_S" ]; then
    BB_TRIP_RULE="abs_band"
    [ "$mem" -le "$BB_ABS_BAND_MIB" ] && return 0
    return 1
  fi
  BB_TRIP_RULE="eta"
  [ $(( mem * 1000 )) -le $(( rate * BB_RUNWAY_MS )) ] && return 0
  return 1
}

# ── 발동 행위(자체시험 대상) : $1=mem $2=rate $3=rule $4=streak $5=ids ──────────────
#   반환 0=실제 kill · 1=dry-run(미실행) · 2=매칭 0.
#   ★ kill 경로를 함수로 가른 이유: dry-run 이 "kill 을 안 한다"는 것을 **시험할 수 있어야** 하기
#     때문이다. 루프 안에 인라인으로 두면 그 분기를 자체시험이 밟을 방법이 없고, 관측 전용 모드가
#     실제로 안전한지는 사람의 코드 읽기에만 의존하게 된다.
bb_fire(){
  local mem="$1" rate="$2" rule="$3" streak="$4" ids="$5"
  if [ "$DRY_RUN" = 1 ]; then
    # 관측 전용: "실무장이었으면 kill 했을 판정" 만 남긴다. 컨테이너는 건드리지 않는다.
    log "TRIP-DRYRUN mem=${mem}MiB rate=${rate}MiB/s rule=$rule streak=${streak} → 실무장이었으면 docker kill ${ids:-<매칭 0>} $(ts)"
    emit_event "watchdog_trip_dryrun" "\"mem_avail_mib\":$mem,\"rate_mib_s\":$rate,\"streak\":$streak,\"rule\":\"$rule\",\"would_kill_targets\":\"$ids\",\"action\":\"none\""
    return 1
  fi
  if [ -z "$ids" ]; then
    log "TRIP-nomatch mem=${mem}MiB rate=${rate}MiB/s rule=$rule (filter='$FILTER' 매칭 0) $(ts)"
    emit_event "watchdog_trip_nomatch" "\"mem_avail_mib\":$mem,\"rate_mib_s\":$rate,\"rule\":\"$rule\""
    return 2
  fi
  log "TRIP mem=${mem}MiB rate=${rate}MiB/s rule=$rule streak=${streak} → docker kill $ids $(ts)"
  emit_event "watchdog_trip" "\"mem_avail_mib\":$mem,\"rate_mib_s\":$rate,\"streak\":$streak,\"rule\":\"$rule\",\"targets\":\"$ids\",\"action\":\"docker_kill\""
  # ★ 부작용 명령의 rc 를 보지 않고 성공 문구를 발행하지 않는다 (2026-09-01 · audit ⑥).
  #   종전에는 파이프(`| sed`) 때문에 rc 가 사라진 채 `kill_ack` 가 **무조건** 발행됐다.
  #   그러면 블랙박스가 "죽였다"고 기록하는데 실제로는 안 죽은 상태가 되어, 사후 분석이
  #   방어 실패를 방어 성공으로 읽는다 — **관측 장치의 위조**다.
  _kout="$(docker kill $ids 2>&1)"; _krc=$?
  printf '%s\n' "$_kout" | sed 's/^/[bb-watchdog] /'
  if [ "$_krc" -eq 0 ]; then
    emit_event "watchdog_kill_ack" "\"targets\":\"$ids\""
  else
    log "KILL-FAILED rc=$_krc targets=$ids — 방어가 성립하지 않았다 $(ts)"
    emit_event "watchdog_kill_failed" "\"targets\":\"$ids\",\"rc\":$_krc"
  fi
  return 0
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
  echo "파라미터: floor=${BB_HARD_FLOOR_MIB}MiB runway=${BB_RUNWAY_MS}ms debounce=${BB_DEBOUNCE_N} min_rate=${BB_MIN_RATE_MIB_S} max_rate=${BB_MAX_RATE_MIB_S} band=${BB_ABS_BAND_MIB}MiB"
  # 자체시험은 **결정론적 입력**으로 돈다 — 설치 환경의 eta_params.env 가 있든 없든 같은 판정을
  # 내야 한다. 그러지 않으면 "내 노드에선 PASS" 가 된다(파라미터 종속 위양성).
  _st_saved="$BB_HARD_FLOOR_MIB $BB_RUNWAY_MS $BB_DEBOUNCE_N $BB_MIN_RATE_MIB_S $BB_MAX_RATE_MIB_S $BB_ABS_BAND_MIB $BB_DECL_MARGIN_MIB $BB_DECL_MIN_CEILING_MIB"
  BB_HARD_FLOOR_MIB=5120; BB_RUNWAY_MS=8000; BB_DEBOUNCE_N=3; BB_MIN_RATE_MIB_S=1
  BB_MAX_RATE_MIB_S=0; BB_ABS_BAND_MIB=10240      # 먼저 이중규칙 OFF 로 기존 회귀를 돌린다
  # ★ 선언 상수도 **고정**한다(2026-08-23 교정). 예전엔 이 둘만 빠져 있어서 자체시험이 설치
  #   환경의 eta_params.env 를 그대로 물고 돌았다 — 2026-08-18 에 정본이 8192/16384 →
  #   3072/8192 로 바뀌자(testlog_26081811 §5.3.2) 기대값 32768/16384 와 어긋나 **설치된
  #   노드에서만 3 건 FAIL** 이 났다. 이 파일이 스스로 경고한 "내 노드에선 PASS" 의 뒤집힌 형태다.
  #   아래 dchk 기대값(32768·16384 거부선)은 이 두 상수에서 파생되므로 여기서 고정해야 한다.
  BB_DECL_MARGIN_MIB=8192; BB_DECL_MIN_CEILING_MIB=16384
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
  #
  # ⚠ 폴 목록은 **인자로** 받는다(예전엔 `printf ... | seq_test` 로 stdin 이었다). 파이프는
  #   함수를 서브셸에서 돌리므로 `fails=1` 이 부모로 올라오지 않았다 — 즉 이 함수의 모든 실패가
  #   **조용히 삼켜져** self-test 가 `PASS` 를 찍었다(2026-08-14 C1 작업 중 실측 발견:
  #   기대값이 틀린 신규 케이스가 [FAIL] 을 출력하고도 최종 판정은 PASS 였다).
  #   검증기가 거짓 판정을 내는 것은 검증기가 없는 것보다 나쁘다.
  seq_test(){ # $1=설명 $2=기대발동(0/1) $3.. = "mem rate" 폴들
    local desc="$1" want="$2"; shift 2
    local streak=0 fired=1 poll m r
    for poll in "$@"; do
      m="${poll%% *}"; r="${poll##* }"
      if bb_trip_now "$m" "$r"; then
        streak=$((streak+1)); [ "$streak" -ge "$BB_DEBOUNCE_N" ] && fired=0
      else streak=0; fi
    done
    if [ "$fired" = "$want" ]; then echo "  [PASS] $desc"
    else echo "  [FAIL] $desc (기대=$want 실제=$fired)"; fails=1; fi
  }
  # 1폴만 트립조건 성립 후 정지 → 디바운스가 걸러야 함
  seq_test "급락 1폴 후 정지 → 미발동" 1 "37000 6241" "20000 100" "19800 10"
  # 3폴 연속 지속 → 발동
  seq_test "급락 3폴 지속 → 발동" 0 "37000 6241" "30800 6200" "24600 6200"
  # 2폴만 지속 → 미발동 (경계)
  seq_test "급락 2폴 후 회복 → 미발동" 1 "37000 6241" "30800 6200" "31000 0"
  # ★ 메타시험: 이 함수의 실패가 실제로 fails 에 반영되는가(위 서브셸 결함의 회귀 고정).
  #   일부러 틀린 기대를 넣어 [FAIL] 을 유발하고, fails 가 올라왔는지 확인한 뒤 되돌린다.
  _meta_before=$fails
  seq_test "  (메타시험 — 아래 FAIL 은 의도된 것이다)" 0 "120000 0" >/dev/null
  if [ "$fails" = 1 ]; then echo "  [PASS] 시퀀스 시험의 실패가 최종 판정에 반영된다(서브셸 아님)"
  else echo "  [FAIL] 시퀀스 시험 실패가 삼켜진다 — 검증기가 거짓 PASS 를 낸다"; fails=1; fi
  fails=$_meta_before

  # ── 선언된 바닥(BB_ARM_CEILING_MIB) — testlog_26073123 실측 재생 ──────────
  echo "선언된 바닥:"
  _saved_ceiling=$BB_ARM_CEILING_MIB
  # (a) 선언 없음 = 무한대 → 현행과 동일해야 한다(회귀 방지: 진성 KV 벌룬 3폴 그대로 발동)
  BB_ARM_CEILING_MIB=999999999
  seq_test "  진성 KV벌룬(선언 없음) → 발동" 0 "74181 27893" "51848 22333" "28686 23162"
  # (b) 위양성 GLM 로드 3회 실측 궤적. 선언 바닥 40960 → 상한 40960-8192=32768
  BB_ARM_CEILING_MIB=32768
  seq_test "  위양성 2차 로드(선언 有) → 미발동" 1 \
    "112195 4127" "88000 24195" "63316 24684" "59894 3422"
  seq_test "  위양성 3차 로드(선언 有) → 미발동" 1 \
    "106510 9938" "82338 24172" "57986 24352" "59881 0"
  seq_test "  위양성 1차 로드(선언 有) → 미발동" 1 \
    "97363 18244" "72734 24629" "56839 15895"
  # (c) ★ 선언이 있어도 그 아래로 뚫으면 재무장해야 한다 — 선언 후 벌룬 시나리오
  seq_test "  선언 아래로 관통(벌룬) → 발동" 0 "31000 12000" "20000 11000" "9000 11000"
  # (d) 절대 바닥은 선언과 무관하게 무장
  BB_ARM_CEILING_MIB=999999999
  chk "  선언 무관 절대바닥 → TRIP"          0 4000 0
  BB_ARM_CEILING_MIB=$_saved_ceiling

  # ── 이중규칙(plan_26081415 C1-A) ─────────────────────────────────────────
  # 상한 = mem_total(124610) / runway(8s) = 15,576 MiB/s. **여기서 계산하지 않는다** —
  # 파생은 blackbox_eta 가 하고 셸은 emit 된 값을 쓴다. 아래는 그 값이 왔다고 가정한 판정 시험이다.
  echo "이중규칙:"
  BB_MAX_RATE_MIB_S=15576
  chk "  상한 미만은 기존 ETA 그대로(37000@6241) → TRIP"     0 37000  6241
  chk "  상한 미만·완만(10186@44) → 무트립"                  1 10186    44
  chk "  ★R0 시도① 사살점(50772@21943) → 무트립"            1 50772 21943
  chk "  ★R0 시도② 사살점(35158@22799) → 무트립"            1 35158 22799
  chk "  ★R0 시도③ 반사실점(46641@23313) → 무트립"          1 46641 23313
  chk "  상한 경계값 자체(60000@15576) → 밴드 판정 → 무트립" 1 60000 15576
  chk "  상한 바로 아래(60000@15575) → ETA 판정 → TRIP"      0 60000 15575
  chk "  상한 위·밴드 경계(10240@20000) → TRIP"              0 10240 20000
  chk "  상한 위·밴드 위 1MiB(10241@20000) → 무트립"         1 10241 20000
  chk "  상한 위·절대바닥 밑(5000@30000) → TRIP(바닥 무장)"  0  5000 30000
  # 규칙 라벨이 이벤트/로그의 출처로 나가므로 값 자체를 고정한다
  bb_trip_now 50772 21943; _r=$BB_TRIP_RULE
  if [ "$_r" = "abs_band" ]; then echo "  [PASS]   판정 규칙 라벨=abs_band"
  else echo "  [FAIL]   판정 규칙 라벨 기대=abs_band 실제=$_r"; fails=1; fi
  bb_trip_now 37000 6241; _r=$BB_TRIP_RULE
  if [ "$_r" = "eta" ]; then echo "  [PASS]   상한 아래 규칙 라벨=eta"
  else echo "  [FAIL]   상한 아래 규칙 라벨 기대=eta 실제=$_r"; fails=1; fi
  bb_trip_now 4000 30000; _r=$BB_TRIP_RULE
  if [ "$_r" = "hard_floor" ]; then echo "  [PASS]   바닥 규칙 라벨=hard_floor"
  else echo "  [FAIL]   바닥 규칙 라벨 기대=hard_floor 실제=$_r"; fails=1; fi
  # R0 실궤적 3종(선언 없음) — 디바운스까지 태워 **kill 0회** 인지 본다
  BB_ARM_CEILING_MIB=999999999
  seq_test "  ★R0 시도① 실궤적(선언 없음) → 미발동" 1 \
    "112195 4127" "88000 24195" "63316 24684" "59894 3422"
  seq_test "  ★R0 시도② 실궤적(선언 없음) → 미발동" 1 \
    "106510 9938" "82338 24172" "57986 24352" "59881 0"
  seq_test "  ★R0 시도③ 실궤적(선언 없음) → 미발동" 1 \
    "97363 18244" "72734 24629" "56839 15895"
  # ⚠ 음성정직 — 진성 KV 벌룬은 이 규칙에서 **미발동**한다(설계상 포기 · plan §2 트레이드오프).
  #    방어는 선언 필수화(C1-B/C3)와 earlyoom 으로 옮겨간다. 오해 방지를 위해 시험으로 못을 박는다.
  seq_test "  ⚠진성 KV벌룬 → **미발동**(포기 · 방어는 선언/earlyoom)" 1 \
    "74181 27893" "51848 22333" "28686 23162"
  # 되찾는 조건은 **밴드 아래 3폴 연속**이다. 20 GiB/s 에서 그건 이미 늦다는 것이 바로 위
  # 트레이드오프의 실체다 — 기계적으로는 살아 있음을 고정하되, 실효성은 헤더 §음성정직 참조.
  seq_test "  진성이 밴드 아래 3폴 머무르면 되찾는다 → 발동" 0 \
    "10000 20000" "9000 20000" "8000 20000"
  seq_test "  밴드 아래 2폴만(20000→12000→9000)은 디바운스가 거른다 → 미발동" 1 \
    "20000 20000" "12000 20000" "9000 20000"
  # OFF 스위치: 상한 0 = mem_total 미상 = 옛 규칙(더 죽이는 쪽)
  BB_MAX_RATE_MIB_S=0
  chk "  상한 0(미상) → 이중규칙 OFF → 옛 규칙대로 TRIP"     0 50772 21943
  BB_MAX_RATE_MIB_S=15576

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
  # ★ 만료 임박(C4-2): 잔여가 알림 밴드 안이면 **무장은 유지한 채** expiring 으로 전이해 경고 1회.
  #   상한이 honored 와 같아야 한다 — 경고가 방어를 약화시키면 경고가 아니라 사고다.
  _soon=$(( $(date +%s) + BB_DECL_EXPIRING_S - 60 ))
  printf 'floor_mib=40960\nexpires_epoch=%s\nlabel=glm-47-flash\n' "$_soon" > "$BB_DECL"
                                                           dchk "만료 임박 → expiring·상한 유지"  expiring  32768
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

  # ── dry-run 모드(plan_26081415 C1-5) ─────────────────────────────────────
  # "kill 하지 않는다"는 **주장이 아니라 관측**이어야 한다. docker 를 셸 함수로 가로채
  # 실제 호출 여부를 기록하고, 두 모드에서 그 파일이 어떻게 달라지는지 본다.
  echo "dry-run:"
  _tmpd2=$(mktemp -d); _calls="$_tmpd2/docker_calls"; : > "$_calls"
  BB_EVENTS="$_tmpd2/events.jsonl"
  docker(){ echo "$*" >> "$_calls"; }        # 스텁 — 실제 docker 를 부르지 않는다
  _saved_dry="$DRY_RUN"; _saved_mode="$MODE"

  DRY_RUN=1; MODE="dry-run"
  bb_fire 9000 20000 abs_band 3 "cafe1234" >/dev/null; _rc=$?
  if [ "$_rc" = 1 ] && [ ! -s "$_calls" ]; then
    echo "  [PASS] dry-run 은 docker 를 부르지 않는다(호출 0건)"
  else echo "  [FAIL] dry-run 인데 docker 호출됨/반환 이상 (rc=$_rc calls=$(cat "$_calls"))"; fails=1; fi
  if grep -q '"kind":"watchdog_trip_dryrun"' "$BB_EVENTS" 2>/dev/null \
     && grep -q '"mode":"dry-run"' "$BB_EVENTS" 2>/dev/null \
     && grep -q '"action":"none"' "$BB_EVENTS" 2>/dev/null; then
    echo "  [PASS] dry-run 판정이 이벤트로 남는다(kind·mode·action 표기)"
  else echo "  [FAIL] dry-run 이벤트 누락/표기 불량"; fails=1; fi
  if grep -q '"kind":"watchdog_trip"' "$BB_EVENTS" 2>/dev/null; then
    echo "  [FAIL] dry-run 이 실무장 이벤트(watchdog_trip)를 냈다 — 하류가 kill 로 오독한다"; fails=1
  else echo "  [PASS] dry-run 은 실무장 이벤트를 내지 않는다(하류 오독 방지)"; fi

  DRY_RUN=0; MODE="armed"; : > "$BB_EVENTS"
  bb_fire 9000 20000 abs_band 3 "cafe1234" >/dev/null; _rc=$?
  if [ "$_rc" = 0 ] && grep -q "kill cafe1234" "$_calls"; then
    echo "  [PASS] 실무장은 실제로 docker kill 한다(대조 — 스텁이 살아있음의 증명)"
  else echo "  [FAIL] 실무장 kill 경로 이상 (rc=$_rc calls=$(cat "$_calls"))"; fails=1; fi
  if grep -q '"mode":"armed"' "$BB_EVENTS" 2>/dev/null \
     && grep -q '"kind":"watchdog_kill_ack"' "$BB_EVENTS" 2>/dev/null; then
    echo "  [PASS] 실무장 이벤트에 mode=armed 와 kill_ack 이 남는다"
  else echo "  [FAIL] 실무장 이벤트 표기 불량"; fails=1; fi
  : > "$_calls"
  bb_fire 9000 20000 abs_band 3 "" >/dev/null; _rc=$?
  if [ "$_rc" = 2 ] && [ ! -s "$_calls" ]; then
    echo "  [PASS] 매칭 0 은 kill 없이 nomatch 반환"
  else echo "  [FAIL] 매칭 0 경로 이상 (rc=$_rc)"; fails=1; fi

  unset -f docker; DRY_RUN="$_saved_dry"; MODE="$_saved_mode"; BB_EVENTS=""
  rm -rf "$_tmpd2"
  # 파라미터 복원 — 자체시험이 전역을 남기지 않게(뒤에 코드가 붙어도 안전하도록)
  set -- $_st_saved
  BB_HARD_FLOOR_MIB=$1; BB_RUNWAY_MS=$2; BB_DEBOUNCE_N=$3; BB_MIN_RATE_MIB_S=$4
  BB_MAX_RATE_MIB_S=$5; BB_ABS_BAND_MIB=$6; BB_DECL_MARGIN_MIB=$7; BB_DECL_MIN_CEILING_MIB=$8

  [ "$fails" = 0 ] && { echo "self-test: PASS"; exit 0; } || { echo "self-test: FAIL"; exit 2; }
fi

# ── 운전 루프 ────────────────────────────────────────────────────────────
[ -n "${MEMWATCH_PIDFILE:-}" ] && echo "$$" > "$MEMWATCH_PIDFILE"
if [ "$BB_MAX_RATE_MIB_S" -gt 0 ]; then
  DUAL_DESC="이중규칙 ON(상한=${BB_MAX_RATE_MIB_S}MiB/s 위는 밴드 ${BB_ABS_BAND_MIB}MiB 판정)"
else
  DUAL_DESC="이중규칙 OFF(mem_total 미상 — 상한 위도 옛 규칙=무조건 트립)"
fi
[ "$DRY_RUN" = 1 ] && log "★★ DRY-RUN — 판정만 하고 docker kill 을 하지 않는다(관측 전용). 이 인스턴스는 호스트를 지키지 않는다."
log "start mode=$MODE filter='$FILTER' interval=${INTERVAL}s params=$PARAMS_SRC floor=${BB_HARD_FLOOR_MIB}MiB runway=${BB_RUNWAY_MS}ms debounce=${BB_DEBOUNCE_N} $DUAL_DESC decl=$BB_DECL pid=$$ $(ts)"
# ★ 정지 경로 (2026-09-01 · audit ㉛). start 는 있고 stop 이 없으면 "지금도 도는 중"과
#   "조용히 사라졌다"가 기록상 같아진다. 침묵 금지 계약은 시작만이 아니라 **끝**에도 걸린다.
trap '_rc=$?; emit_event "watchdog_stop" "\"rc\":$_rc,\"signal\":\"${_bb_sig:-EXIT}\""; exit $_rc' EXIT
# 2026-09-03(㉛ 회귀 · plan_26090317 P1): 핸들러가 변수만 놓고 `exit` 하지 않아 bash 가 루프를
#   **계속 돌았다** — TERM 으로는 죽지 않고 SIGKILL 까지 가며, KILL 은 trap 을 안 돌므로
#   정지 기록도 남지 않는다. 즉 ㉛ 이 만들려던 기록은 TERM 경로에서 달성되지 않고, 그 대신
#   **정상 정지 경로가 사라졌다**(라이브 고아 워치독 1건이 그 결과다). 128+signum 으로 나간다.
trap '_bb_sig=TERM; exit 143' TERM
trap '_bb_sig=INT;  exit 130' INT
trap '_bb_sig=HUP;  exit 129' HUP

emit_event "watchdog_start" "\"filter\":\"$FILTER\",\"runway_ms\":$BB_RUNWAY_MS,\"debounce\":$BB_DEBOUNCE_N,\"hard_floor_mib\":$BB_HARD_FLOOR_MIB,\"max_rate_mib_s\":$BB_MAX_RATE_MIB_S,\"abs_band_mib\":$BB_ABS_BAND_MIB,\"decl_path\":\"$BB_DECL\",\"decl_margin_mib\":$BB_DECL_MARGIN_MIB"

# 상한 위 '보류'(이중규칙이 옛 규칙과 갈리는 유일한 지점)는 **에피소드당 1회** 남긴다.
# 폴마다 쓰면 홍수고, 아예 안 쓰면 규칙이 조용히 완화된 것과 구별되지 않는다(침묵 금지).
highrate_ep=0
prev_mem=""; streak=0; last_hb=0; min_since_hb=999999999
while true; do
  refresh_decl                      # 선언은 매 폴 재평가한다(선언 소멸이 즉시 규칙 복귀가 되도록)
  # ★ 판독 실패는 **판정 불가**이지 "메모리 0" 도 "안전" 도 아니다 (2026-09-01 · audit ④).
  #   종전: `mem=$(( $(awk …) / 1024 ))`. /proc/meminfo 를 못 읽으면 산술 확장이 문법
  #   오류가 되고, 비대화형 bash 는 **확장 오류에서 즉시 종료**한다(실측 rc=1). 그 순간
  #   워치독은 사라지는데 events 에는 한 줄도 남지 않고, `watchdog_stop` 이벤트도 없어
  #   "돌았는데 못 잡았다"와 "3시간 전에 사라졌다"를 **데이터로 가를 수 없었다**(㉛과 결합).
  mem_kb="$(awk '/MemAvailable:/{print $2; exit}' /proc/meminfo 2>/dev/null || true)"
  case "$mem_kb" in
    ''|*[!0-9]*)
      log "READ-FAIL /proc/meminfo MemAvailable 판독 실패(raw=[$mem_kb]) — 이 폴은 판정하지 않는다 $(ts)"
      emit_event "watchdog_read_fail" "\"source\":\"/proc/meminfo\",\"field\":\"MemAvailable\""
      sleep "$INTERVAL"
      continue ;;
  esac
  mem=$(( mem_kb / 1024 ))
  if [ -n "$prev_mem" ]; then rate=$(( (prev_mem - mem) / INTERVAL )); else rate=0; fi
  prev_mem="$mem"
  [ "$mem" -lt "$min_since_hb" ] && min_since_hb=$mem

  now=$(date +%s)
  if [ "$HB_SEC" -gt 0 ] && [ $(( now - last_hb )) -ge "$HB_SEC" ]; then
    log "HB[$MODE] MemAvailable=${mem}MiB min=${min_since_hb}MiB rate=${rate}MiB/s streak=${streak} decl=${BB_DECL_STATE}/${BB_ARM_CEILING_MIB}MiB $(ts)"
    last_hb=$now; min_since_hb=$mem
  fi

  # ── 상한 위 보류 관측: 옛 규칙이었으면 여기서 죽었다 ─────────────────────
  #   판정 자체와 무관한 순수 관측이므로 bb_trip_now 앞에 둔다(BB_TRIP_RULE 을 덮지 않게).
  if [ "$BB_MAX_RATE_MIB_S" -gt 0 ] && [ "$rate" -ge "$BB_MAX_RATE_MIB_S" ] \
     && [ "$mem" -gt "$BB_ABS_BAND_MIB" ] && [ "$mem" -gt "$BB_HARD_FLOOR_MIB" ]; then
    if [ "$highrate_ep" = 0 ]; then
      highrate_ep=1
      log "HIGHRATE-HOLD mem=${mem}MiB rate=${rate}MiB/s ≥ 상한 ${BB_MAX_RATE_MIB_S} — 밴드(${BB_ABS_BAND_MIB}MiB) 위라 보류. 옛 규칙이었으면 트립했다. $(ts)"
      emit_event "watchdog_highrate_hold" "\"mem_avail_mib\":$mem,\"rate_mib_s\":$rate,\"max_rate_mib_s\":$BB_MAX_RATE_MIB_S,\"abs_band_mib\":$BB_ABS_BAND_MIB,\"legacy_rule_would_trip\":true"
    fi
  elif [ "$highrate_ep" = 1 ] && [ "$rate" -lt "$BB_MAX_RATE_MIB_S" ]; then
    highrate_ep=0            # 에피소드 종료 — 다음 급락에서 다시 1회 남긴다
  fi

  if bb_trip_now "$mem" "$rate"; then
    rule="$BB_TRIP_RULE"
    streak=$(( streak + 1 ))
    [ "$streak" -eq 1 ] && log "TRIP-ARM[$MODE] mem=${mem}MiB rate=${rate}MiB/s rule=$rule (디바운스 1/${BB_DEBOUNCE_N}) $(ts)"
    if [ "$streak" -ge "$BB_DEBOUNCE_N" ]; then
      ids=$(targets)
      bb_fire "$mem" "$rate" "$rule" "$streak" "$ids"
      case $? in
        0) streak=0; prev_mem="" ;;   # 실제 kill — 회수 반등을 하강률로 오독하지 않도록 리셋
        1) streak=0 ;;                # dry-run — 컨테이너가 살아 있으니 rate 는 계속 유효하다
        *) sleep 5; prev_mem="" ;;    # 매칭 0
      esac
    fi
  else
    [ "$streak" -gt 0 ] && log "TRIP-DISARM[$MODE] mem=${mem}MiB rate=${rate}MiB/s (지속 실패 — 급락 후 정지) $(ts)"
    streak=0
  fi
  sleep "$INTERVAL"
done
