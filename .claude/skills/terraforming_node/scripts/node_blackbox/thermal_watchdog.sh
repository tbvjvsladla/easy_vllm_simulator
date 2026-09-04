#!/bin/bash
# thermal_watchdog.sh — 열·전력 포락선 반사 방어 (plan_26082319 §5.2·§6.3).
#   `mem_watchdog_eta.sh`(RAM 축)의 **자매 파일**. 축이 다를 뿐 구조·규약은 같다:
#   blackbox_thermal.py 가 emit 한 상수를 source 해 **정수 산술만** 수행한다(파이썬 비의존).
#
#   판정식 (누설 버킷 — 지속성이 판정의 전부다):
#       poll 마다  값 >= 임계 ? bucket=min(cap,bucket+1) : bucket=max(0,bucket-decay)
#       TRIP  <=>  gpu_bucket >= BB_TP_GPU_SUSTAIN_S          # 지속 고전력
#              ||  soc_bucket >= BB_TP_SOC_SUSTAIN_S          # 지속 SoC 열
#              ||  soc_hard_streak >= BB_TP_SOC_HARD_POLLS    # SoC 즉시 계층
#       축은 **독립**이다 — 어느 하나만 차도 kill(하드다운은 어느 축으로든 온다).
#
#   ★ 왜 축이 하나 더 필요한가: 2026-08-23 R6 는 RAM OOM 이 아니라 **하드 락업**이었다.
#     MemAvailable 은 19.3 GiB 평탄이라 ETA 워치독은 정상적으로 미발동했고, 그것이 옳았다.
#     RAM 축은 이 사건에 대해 원리적으로 무력하다. 근거·임계 도출·음성정직은 전부
#     `blackbox_thermal.py` 헤더가 소유한다 — 여기에 두 번 적지 않는다(갈라지면 침묵 누락).
#
#   ★ 신호 출처가 축마다 다르다(의도적):
#       · GPU 전력  = **수집기 CSV 꼬리**. nvidia-smi 스폰은 50~200ms 라 1 Hz 핫루프가 직접
#         부를 수 없다. 수집기가 이미 `-l 1` 스트림 하나로 받아 fsync 로 적고 있으므로 그것을 읽는다.
#         대가: 수집기가 죽으면 이 축이 눈먼다 → **stale 로 외치고 버킷을 동결**한다(아래).
#       · SoC 열    = **sysfs 직접 판독**. 순수 파일 읽기라 스폰이 0 이고 수집기와 무관하게 산다.
#         한쪽 신호원이 죽어도 다른 축이 남는 것이 계층 방어의 요점이다.
#
#   ★ stale 은 '안전'이 아니다: 꼬리가 BB_TP_STALE_AFTER_S 보다 오래되면 GPU 버킷을 **동결**한다.
#     빼면 수집기 사망이 곧 조용한 무장해제가 되고, 더하면 위양성이 된다. 동결만이 정직하다.
#     그리고 stale 진입/이탈은 **에피소드당 1회** 로그·이벤트로 남긴다(침묵 금지).
#
# 사용: thermal_watchdog.sh [name_filter|@vllm] [interval_sec]
#   '@vllm' 광역 모드의 킬 대상 술어는 BB_TARGET_PREDICATE_V1 블록이 소유한다(세 워치독 공유).
# env : BB_TP_PARAMS  (기본 /etc/easy-vllm/thermal_params.env — blackbox_thermal.py --emit-params)
#       BB_TP_NODE_DIR(수집기 CSV 루트. 기본 = BB_TP_EVENTS 의 조부모)
#       BB_TP_EVENTS  (지정 시 트립/킬 이벤트를 JSONL append — 블랙박스 events 평면)
#       BB_TP_THERMAL_ROOT (기본 /sys/class/thermal)
#       BB_TP_DRY_RUN (1 = **관측 전용 무장**. 판정은 그대로 하되 docker kill 을 하지 않는다.
#                      새 축을 실무장하기 전 1 회 관측이 정석이다 — mem_watchdog_eta 의 C1-5 선례)
#       TW_HEARTBEAT_SEC (기본 15 · 0=끔) · TW_PIDFILE
# 정지: kill <pid> 로만. **pkill -f 금지**(자기참조 부모셸 사망 선례 exit144, devlog_26062718).
# 종료코드: 0=정상종료 · 1=전제 실패 · 2=자체시험 실패.
set -uo pipefail

SELFTEST=0
[ "${1:-}" = "--self-test" ] && { SELFTEST=1; shift; }
# dry-run 은 env 또는 첫 인자로 켠다(유닛을 건드리지 않고 별도 인스턴스를 손으로 띄우기 위해).
DRY_RUN="${BB_TP_DRY_RUN:-0}"
[ "${1:-}" = "--dry-run" ] && { DRY_RUN=1; shift; }
MODE="armed"; [ "$DRY_RUN" = 1 ] && MODE="dry-run"

FILTER="${1:-@vllm}"; [ "$SELFTEST" = 1 ] && FILTER="@vllm"
# 폴링 간격의 기본값은 **상수 파일**에서 온다(아래 source 뒤에 확정). 인자를 주면 인자가 이긴다 —
# 예전엔 여기서 `${2:-1}` 로 못박혀 emit 된 BB_TP_POLL_INTERVAL_S 가 **소비되지 않았다**
# (자체시험의 파라미터 패리티가 잡은 결함: 만들었는데 돌지 않는 배선).
INTERVAL_ARG="${2:-}"
HB_SEC="${TW_HEARTBEAT_SEC:-15}"
BB_TP_PARAMS="${BB_TP_PARAMS:-/etc/easy-vllm/thermal_params.env}"
BB_TP_EVENTS="${BB_TP_EVENTS:-}"
BB_TP_THERMAL_ROOT="${BB_TP_THERMAL_ROOT:-/sys/class/thermal}"
# 노드 디렉터리 기본값 = <node_dir>/events/*.jsonl 의 조부모 (mem_watchdog_eta 의 BB_DECL 선례)
if [ -n "${BB_TP_NODE_DIR:-}" ]; then :; elif [ -n "$BB_TP_EVENTS" ]; then
  BB_TP_NODE_DIR="$(dirname "$(dirname "$BB_TP_EVENTS")")"
else BB_TP_NODE_DIR=""; fi

# ── 파라미터 기본값 (부재가 무방비를 뜻하지 않게 — fail-safe 방향) ────────
#   정본은 thermal_params.env(blackbox_thermal.DEFAULTS 가 생성) — 아래 source 가 덮는다.
#   ⚠ 여기 값은 **정본의 사본**이다. 갈라지면 자체시험(파라미터 패리티)이 FAIL 한다.
BB_TP_GPU_PWR_DW=800; BB_TP_GPU_SUSTAIN_S=60; BB_TP_GPU_BUCKET_CAP=120
BB_TP_SOC_WARN_C=90;  BB_TP_SOC_SUSTAIN_S=30; BB_TP_SOC_BUCKET_CAP=60
BB_TP_SOC_HARD_C=95;  BB_TP_SOC_HARD_POLLS=3
BB_TP_BUCKET_DECAY=1; BB_TP_STALE_AFTER_S=10; BB_TP_POLL_INTERVAL_S=1
BB_TP_SOC_PLAUSIBLE_MIN_C=-40; BB_TP_SOC_PLAUSIBLE_MAX_C=150
BB_TP_UNCALIBRATED=""
PARAMS_SRC="defaults(내장)"
if [ -r "$BB_TP_PARAMS" ]; then
  # shellcheck disable=SC1090
  . "$BB_TP_PARAMS" && PARAMS_SRC="$BB_TP_PARAMS"
fi
# 소수점이 섞여 오면 정수부만 취한다(셸 산술은 정수 전용)
for _v in BB_TP_GPU_PWR_DW BB_TP_GPU_SUSTAIN_S BB_TP_GPU_BUCKET_CAP BB_TP_SOC_WARN_C \
          BB_TP_SOC_SUSTAIN_S BB_TP_SOC_BUCKET_CAP BB_TP_SOC_HARD_C BB_TP_SOC_HARD_POLLS \
          BB_TP_BUCKET_DECAY BB_TP_STALE_AFTER_S BB_TP_POLL_INTERVAL_S; do
  eval "$_v=\${$_v%%.*}"
done
INTERVAL="${INTERVAL_ARG:-$BB_TP_POLL_INTERVAL_S}"

ABSENT=-1                       # 부재/판독불가 센티넬(0 과 구별한다 — 0W 는 '아주 안전'이다)

ts(){ date -u +%FT%TZ; }
log(){ echo "[tp-watchdog] $*"; }

emit_event(){ # $1=kind $2=json-fragment
  [ -n "$BB_TP_EVENTS" ] || return 0
  local _new=0
  [ -e "$BB_TP_EVENTS" ] || _new=1
  # `mode` 를 모든 이벤트에 싣는다 — dry-run 관측 인스턴스와 실무장 데몬이 같은 파일에 함께
  # append 하므로, 이 필드가 없으면 "죽인 것인가 관측인가"를 데이터에서 가를 수 없다.
  printf '{"ts":"%s","kind":"%s","source":"thermal_watchdog","mode":"%s"%s}\n' \
    "$(ts)" "$1" "$MODE" "${2:+,$2}" >> "$BB_TP_EVENTS" 2>/dev/null || true
  # root 가 새로 만든 이벤트 파일은 디렉터리 소유자에게 넘긴다(비-root 사용자 도구와 공동 append —
  # mem_watchdog_eta / blackbox_events 의 2026-08-01 EACCES 선례와 같은 규율).
  if [ "$_new" = "1" ] && [ "$(id -u)" = "0" ] && [ -e "$BB_TP_EVENTS" ]; then
    chown --reference="$(dirname "$BB_TP_EVENTS")" "$BB_TP_EVENTS" 2>/dev/null || true
  fi
}

# ── 신호 판독 ─────────────────────────────────────────────────────────────
# SoC: sysfs 직접(스폰 0). 타당성 밴드 밖(미초기화 센서 -274000·2147483647)은 **읽지 않은 것**.
SOC_C=$ABSENT; SOC_ZONE=$ABSENT
tp_read_soc(){
  local f v c found=0 best=0 bz=0 zname
  SOC_C=$ABSENT; SOC_ZONE=$ABSENT
  for f in "$BB_TP_THERMAL_ROOT"/thermal_zone*/temp; do
    [ -r "$f" ] || continue
    read -r v < "$f" 2>/dev/null || continue
    case "$v" in ''|*[!0-9-]*) continue ;; esac
    c=$(( v / 1000 ))
    [ "$c" -lt "$BB_TP_SOC_PLAUSIBLE_MIN_C" ] && continue
    [ "$c" -gt "$BB_TP_SOC_PLAUSIBLE_MAX_C" ] && continue
    if [ "$found" = 0 ] || [ "$c" -gt "$best" ]; then
      zname="${f%/temp}"; zname="${zname##*/thermal_zone}"
      best=$c; bz=$zname; found=1
    fi
  done
  [ "$found" = 1 ] && { SOC_C=$best; SOC_ZONE=$bz; }
}

# GPU 전력: 수집기 CSV 꼬리. gpu_pwr 는 옛/새 스키마 모두 **5번째 열**이라 NF>=5 로 충분하다
# (soc_temp 만 새 스키마의 11번째 열인데, 그것은 위에서 sysfs 로 직접 읽으므로 무관하다).
# 꼬리만 읽어 파일 크기와 무관하게 상수시간으로 만든다(하루치 CSV 는 약 3.7 MB).
GPU_PWR_DW=$ABSENT; GPU_TS=0
tp_read_gpu(){ # $1=csv 경로
  local out
  GPU_PWR_DW=$ABSENT; GPU_TS=0
  [ -r "${1:-}" ] || return 0
  out=$(tail -c 8192 "$1" 2>/dev/null | awk -F, '
      $1 ~ /^[0-9]+$/ && NF >= 5 { t=$1; w=$5 }
      END { if (t == "") print "0 -1";
            else printf "%d %d\n", t, (w == "" ? -1 : int(w * 10 + 0.5)) }')
  GPU_TS="${out%% *}"; GPU_PWR_DW="${out##* }"
  case "$GPU_TS"      in ''|*[!0-9]*)  GPU_TS=0 ;; esac
  case "$GPU_PWR_DW"  in ''|*[!0-9-]*) GPU_PWR_DW=$ABSENT ;; esac
}

# ── 순수 판정(자체시험 대상) ──────────────────────────────────────────────
#   $1=gpu_pwr_dw($ABSENT 가능) $2=soc_temp_c($ABSENT 가능) $3=gpu_stale(0/1)
#   → 0=TRIP · 1=아님. 버킷 3종은 전역이며 TP_RULE 에 판정 규칙을 남긴다(출처 표시).
TP_GPU_BUCKET=0; TP_SOC_BUCKET=0; TP_SOC_HARD_STREAK=0; TP_RULE="none"
tp_step(){
  local w="$1" c="$2" stale="${3:-0}"
  if [ "$stale" = 1 ] || [ "$w" -eq "$ABSENT" ]; then
    :                                            # 동결 — 더하지도 빼지도 않는다
  elif [ "$w" -ge "$BB_TP_GPU_PWR_DW" ]; then
    TP_GPU_BUCKET=$(( TP_GPU_BUCKET + 1 ))
    [ "$TP_GPU_BUCKET" -gt "$BB_TP_GPU_BUCKET_CAP" ] && TP_GPU_BUCKET=$BB_TP_GPU_BUCKET_CAP
  else
    TP_GPU_BUCKET=$(( TP_GPU_BUCKET - BB_TP_BUCKET_DECAY ))
    [ "$TP_GPU_BUCKET" -lt 0 ] && TP_GPU_BUCKET=0
  fi
  if [ "$c" -ne "$ABSENT" ]; then
    if [ "$c" -ge "$BB_TP_SOC_WARN_C" ]; then
      TP_SOC_BUCKET=$(( TP_SOC_BUCKET + 1 ))
      [ "$TP_SOC_BUCKET" -gt "$BB_TP_SOC_BUCKET_CAP" ] && TP_SOC_BUCKET=$BB_TP_SOC_BUCKET_CAP
    else
      TP_SOC_BUCKET=$(( TP_SOC_BUCKET - BB_TP_BUCKET_DECAY ))
      [ "$TP_SOC_BUCKET" -lt 0 ] && TP_SOC_BUCKET=0
    fi
    if [ "$c" -ge "$BB_TP_SOC_HARD_C" ]; then
      TP_SOC_HARD_STREAK=$(( TP_SOC_HARD_STREAK + 1 ))
    else
      TP_SOC_HARD_STREAK=0
    fi
  fi
  # 우선순위 = 급한 것부터(파이썬 엔진 blackbox_thermal.step 과 동일 순서)
  if [ "$TP_SOC_HARD_STREAK" -ge "$BB_TP_SOC_HARD_POLLS" ]; then TP_RULE="soc_hard_ceiling"; return 0; fi
  if [ "$TP_SOC_BUCKET" -ge "$BB_TP_SOC_SUSTAIN_S" ];        then TP_RULE="soc_temp_sustained"; return 0; fi
  if [ "$TP_GPU_BUCKET" -ge "$BB_TP_GPU_SUSTAIN_S" ];        then TP_RULE="gpu_pwr_sustained"; return 0; fi
  TP_RULE="none"; return 1
}

tp_reset(){ TP_GPU_BUCKET=0; TP_SOC_BUCKET=0; TP_SOC_HARD_STREAK=0; }

# ── 발동 행위(자체시험 대상) : $1=rule $2=사람용요약 $3=json조각 $4=ids ────
#   반환 0=실제 kill · 1=dry-run(미실행) · 2=매칭 0.
#   ★ kill 경로를 함수로 가른 이유는 mem_watchdog_eta 와 같다 — "dry-run 이 kill 을 안 한다"가
#     사람의 코드 읽기가 아니라 **시험으로 관측**되어야 하기 때문이다.
#   ★ 사람용 요약과 JSON 조각을 **가른다**: 로그 줄은 journald 를 거쳐 `blackbox_events.py` 가
#     정규식으로 다시 읽는다(킬 경로 통합 평면). 로그에 JSON 조각을 그대로 흘리면 그 정규식이
#     따옴표·콤마와 싸우게 되고, 파서가 깨져도 조용히 이벤트만 사라진다.
tp_fire(){
  local rule="$1" human="$2" ev="$3" ids="$4"
  if [ "$DRY_RUN" = 1 ]; then
    log "TRIP-DRYRUN rule=$rule $human → 실무장이었으면 docker kill ${ids:-<매칭 0>} $(ts)"
    emit_event "thermal_trip_dryrun" "\"rule\":\"$rule\",$ev,\"would_kill_targets\":\"$ids\",\"action\":\"none\""
    return 1
  fi
  if [ -z "$ids" ]; then
    log "TRIP-nomatch rule=$rule $human (filter='$FILTER' 매칭 0) $(ts)"
    emit_event "thermal_trip_nomatch" "\"rule\":\"$rule\",$ev"
    return 2
  fi
  log "TRIP rule=$rule $human → docker kill $ids $(ts)"
  emit_event "thermal_trip" "\"rule\":\"$rule\",$ev,\"targets\":\"$ids\",\"action\":\"docker_kill\""
  # 2026-09-03(⑥ 잔여 · plan_26090317 P1): 파이프가 `docker kill` 의 rc 를 삼켜 **실패해도 무조건**
  #   `thermal_kill_ack` 을 발행했다 — 블랙박스가 "죽였다" 고 기록하는데 실제로는 안 죽은 상태다.
  #   사후 분석이 방어 실패를 방어 성공으로 읽는다(관측 장치의 위조). 자매 파일 mem_watchdog_eta
  #   `bb_fire` 는 2026-09-01 에 같은 교정을 받았는데 이 파일에는 오지 않았다.
  _kout="$(docker kill $ids 2>&1)"; _krc=$?
  printf '%s\n' "$_kout" | sed 's/^/[tp-watchdog] /'
  if [ "$_krc" -eq 0 ]; then
    emit_event "thermal_kill_ack" "\"targets\":\"$ids\""
  else
    log "KILL-FAILED rc=$_krc targets=$ids — 열 방어가 성립하지 않았다 $(ts)"
    emit_event "thermal_kill_failed" "\"targets\":\"$ids\",\"rc\":$_krc"
  fi
  return 0
}

# ── BB_TARGET_PREDICATE_V1 ────────────────────────────────────────────────
# 킬 대상 술어. **세 워치독(node_blackbox/mem_watchdog_eta · host_safety/mem_watchdog ·
# node_blackbox/thermal_watchdog)이 이 블록을 글자 그대로 공유**한다. 갈라지면
# runtime_selftest 의 parity tripwire 가 잡는다(정적 파일끼리는 한쪽이 다른 쪽을 생성할 수
# 없으므로 단일 소유가 불가능하다 — 차선은 교차검증이다: workflow.md §결정론 규율).
# sourcing 하지 않는 이유: 설치기가 이 파일들을 **확장자 없는 단독 바이너리**로 복사하므로
# sibling 경로가 현장에서 사라진다(2026-09-01 블랙박스 sibling import 파손 선례).
#
# ★ 2026-09-04(CP0 · plan_26090415 §3.3). 종전 술어는 `{{.ID}} {{.Image}} {{.Names}}` **한 줄
#   전체**를 `/vllm/` 로 훑었다. 그래서 이미지 경로의 **레지스트리·조직 세그먼트**까지 매칭
#   대상이 됐고, 전용 벤치툴 공식 이미지 `ghcr.io/vllm-project/guidellm` 이 조직명
#   `vllm-project` 때문에 걸렸다. 트립하면 `docker kill $ids` 가 매칭 전부를 한 번에 죽이므로
#   **서빙 컨테이너와 측정 컨테이너가 동반 사살**된다 — 안전장치가 측정을 공격하는 형태이며,
#   이름 우연 일치이지 설계된 동작이 아니다.
#   교정은 이름 목록이 아니라 **원인**을 친다: 이미지에서 레지스트리·조직 경로를 벗기고
#   **저장소 이름**만 본다(조직명이 매칭 평면에서 사라진다). 커버리지는 보존된다 —
#     easy-vllm:<tag>                      → easy-vllm:<tag>     매칭 ○ (로컬 빌드 서빙 이미지)
#     vllm/vllm-openai:latest              → vllm-openai:latest  매칭 ○ (업스트림 서버)
#     ghcr.io/vllm-project/guidellm:latest → guidellm:latest     매칭 ✗ (측정 도구)
#   이름 필드 매칭은 그대로 둔다(vllm_trial01 등). **이미지 매칭이 살아 있어야** 이름에 vllm 이
#   없는 서빙 컨테이너(mn-hy3-master·mn-exaone45-33b-master 실측 반례)를 계속 잡는다
#   — verify_node_blackbox.sh B11 이 그 반례로 이름-단독 술어를 이미 기각했다.
bb_target_match(){   # $1=ID $2=IMAGE $3=NAMES → rc 0=킬 대상 · 1=제외
  local _repo _name
  _repo="$(printf '%s' "${2##*/}" | tr 'A-Z' 'a-z')"
  _name="$(printf '%s' "${3:-}"   | tr 'A-Z' 'a-z')"
  case "$_repo" in *vllm*) return 0 ;; esac
  case "$_name" in *vllm*) return 0 ;; esac
  return 1
}
targets(){
  if [ "$FILTER" = "@vllm" ]; then
    local _id _img _names
    while IFS='|' read -r _id _img _names; do
      [ -n "$_id" ] || continue
      bb_target_match "$_id" "$_img" "$_names" && printf '%s\n' "$_id"
    done < <(docker ps --filter status=running \
               --format '{{.ID}}|{{.Image}}|{{.Names}}' 2>/dev/null)
  else
    docker ps --filter "name=$FILTER" --filter status=running -q
  fi
}
# ── /BB_TARGET_PREDICATE_V1 ───────────────────────────────────────────────

# ── 자체시험 (하드웨어·도커 불요) ────────────────────────────────────────
if [ "$SELFTEST" = 1 ]; then
  fails=0
  pass(){ echo "  [PASS] $1"; }
  fail(){ echo "  [FAIL] $1"; fails=1; }
  ck(){ if [ "$2" = "$3" ]; then pass "$1"; else fail "$1 (기대=$2 실제=$3)"; fi; }

  # 자체시험은 **결정론적 입력**으로 돈다 — 설치 환경의 thermal_params.env 유무와 무관하게
  # 같은 판정을 내야 한다(mem_watchdog_eta 의 "내 노드에선 PASS" 회귀 차단과 같은 규율).
  BB_TP_GPU_PWR_DW=800; BB_TP_GPU_SUSTAIN_S=60; BB_TP_GPU_BUCKET_CAP=120
  BB_TP_SOC_WARN_C=90;  BB_TP_SOC_SUSTAIN_S=30; BB_TP_SOC_BUCKET_CAP=60
  BB_TP_SOC_HARD_C=95;  BB_TP_SOC_HARD_POLLS=3
  BB_TP_BUCKET_DECAY=1; BB_TP_STALE_AFTER_S=10; BB_TP_POLL_INTERVAL_S=1
  BB_TP_SOC_PLAUSIBLE_MIN_C=-40; BB_TP_SOC_PLAUSIBLE_MAX_C=150
  echo "파라미터: gpu>=${BB_TP_GPU_PWR_DW}dW ${BB_TP_GPU_SUSTAIN_S}s · soc>=${BB_TP_SOC_WARN_C}C ${BB_TP_SOC_SUSTAIN_S}s · soc-hard>=${BB_TP_SOC_HARD_C}C ${BB_TP_SOC_HARD_POLLS}폴"

  # 시퀀스 시험 — ⚠ 폴 목록은 **인자로** 받는다. 파이프(stdin)로 넘기면 서브셸에서 돌아
  #   `fails=1` 이 부모로 올라오지 않고 모든 실패가 조용히 삼켜진다(mem_watchdog_eta 가
  #   2026-08-14 에 실제로 겪은 결함 — 검증기가 거짓 PASS 를 낸다).
  # 폴은 `pwr_dw:soc_c[:stale]` 한 덩어리로 적는다. 공백으로 적으면 `$(rep ...)` 의 비인용
  # 확장이 쌍을 쪼개 버린다(첫 구현이 실제로 그랬다 — `$2: unbound variable`).
  seq_test(){ # $1=설명 $2=기대발동(0/1) $3=기대rule("" = 무시) $4.. = "pwr_dw:soc_c[:stale]"
    local desc="$1" want="$2" wrule="$3"; shift 3
    local fired=1 poll w c st
    tp_reset
    for poll in "$@"; do
      IFS=':' read -r w c st <<< "$poll"
      if tp_step "$w" "$c" "${st:-0}"; then fired=0; fi
    done
    if [ "$fired" != "$want" ]; then fail "$desc (기대발동=$want 실제=$fired)"; return; fi
    if [ -n "$wrule" ] && [ "$fired" = 0 ] && [ "$TP_RULE" != "$wrule" ]; then
      fail "$desc (규칙 기대=$wrule 실제=$TP_RULE)"; return
    fi
    pass "$desc"
  }
  rep(){ # $1=횟수 $2=폴 → 폴을 n번 복제(긴 지속 시퀀스 생성용)
    local i=0; while [ "$i" -lt "$1" ]; do printf '%s\n' "$2"; i=$((i+1)); done
  }

  echo "GPU 전력 축:"
  # 59 폴은 미발동, 60 폴에서 발동 (경계)
  # shellcheck disable=SC2046
  seq_test "  90.0W 59폴 → 미발동(경계 아래)" 1 "" $(rep 59 "900:-1")
  # shellcheck disable=SC2046
  seq_test "  90.0W 60폴 → 발동(gpu_pwr_sustained)" 0 "gpu_pwr_sustained" $(rep 60 "900:-1")
  # ★음성대조 — 사건일 15:24 실측 순간버스트(100.79W 10초)
  # shellcheck disable=SC2046
  seq_test "  ★음성대조 100.8W 10초 버스트 → 미발동" 1 "" $(rep 10 "1008:-1")
  # 경계값 자체는 고부하로 센다(>=)
  tp_reset; tp_step 800 $ABSENT 0; ck "  80.0W 경계는 고부하(>=)" 1 "$TP_GPU_BUCKET"
  tp_reset; tp_step 799 $ABSENT 0; ck "  79.9W 는 고부하 아님"     0 "$TP_GPU_BUCKET"
  # 저전력은 아무리 길어도 미발동
  # shellcheck disable=SC2046
  seq_test "  45W 600폴 → 미발동" 1 "" $(rep 600 "450:-1")
  # dropout 내성: 8초마다 1초 저전력이 섞여도 지속으로 인식(연속 스트릭이면 미발동)
  _osc=""; _i=0
  while [ "$_i" -lt 80 ]; do
    if [ $(( _i % 8 )) -eq 7 ]; then _osc="$_osc 300:-1"; else _osc="$_osc 900:-1"; fi
    _i=$((_i+1))
  done
  # shellcheck disable=SC2086
  seq_test "  dropout 진동(8초마다 1초 저전력) → 발동" 0 "gpu_pwr_sustained" $_osc
  # 버킷 상한: 장시간 부하 뒤에도 배출이 유계여야 한다
  tp_reset; _i=0; while [ "$_i" -lt 500 ]; do tp_step 900 $ABSENT 0; _i=$((_i+1)); done
  ck "  버킷은 cap 을 넘지 않는다(회복 유계)" "$BB_TP_GPU_BUCKET_CAP" "$TP_GPU_BUCKET"

  echo "SoC 열 축:"
  # shellcheck disable=SC2046
  seq_test "  91C 29폴 → 미발동" 1 "" $(rep 29 "-1:91")
  # shellcheck disable=SC2046
  seq_test "  91C 30폴 → 발동(soc_temp_sustained)" 0 "soc_temp_sustained" $(rep 30 "-1:91")
  # shellcheck disable=SC2046
  seq_test "  97C 3폴 → 즉시계층 발동(soc_hard_ceiling)" 0 "soc_hard_ceiling" $(rep 3 "-1:97")
  seq_test "  즉시계층은 연속이어야 한다(끊기면 리셋)" 1 "" "-1:97" "-1:60" "-1:97"
  # shellcheck disable=SC2046
  seq_test "  평시 47C 600폴 → 미발동" 1 "" $(rep 600 "-1:47")
  # 축 독립 — GPU 는 한산한데 SoC 만 뜨겁다
  # shellcheck disable=SC2046
  seq_test "  한 축만 차도 발동(축 독립)" 0 "soc_temp_sustained" $(rep 30 "100:91")

  echo "부재·stale 정직성:"
  # shellcheck disable=SC2046
  seq_test "  양축 부재 600폴 → 미발동(0 취급 금지)" 1 "" $(rep 600 "-1:-1")
  # stale 동결: 축적분이 늘지도 줄지도 않는다
  tp_reset; _i=0; while [ "$_i" -lt 30 ]; do tp_step 900 $ABSENT 0; _i=$((_i+1)); done
  _before=$TP_GPU_BUCKET
  _i=0; while [ "$_i" -lt 50 ]; do tp_step 900 $ABSENT 1; _i=$((_i+1)); done
  ck "  stale 은 GPU 버킷을 동결한다(무장해제 아님)" "$_before" "$TP_GPU_BUCKET"
  # 부재로 바뀌어도 축적분이 사라지지 않는다
  tp_reset; _i=0; while [ "$_i" -lt 30 ]; do tp_step 900 $ABSENT 0; _i=$((_i+1)); done
  _i=0; while [ "$_i" -lt 40 ]; do tp_step $ABSENT $ABSENT 0; _i=$((_i+1)); done
  ck "  부재 전환이 축적분을 지우지 않는다" 30 "$TP_GPU_BUCKET"

  # ★ 메타시험: seq_test 의 실패가 실제로 fails 에 반영되는가(서브셸 결함의 회귀 고정).
  _meta_before=$fails
  seq_test "  (메타시험 — 아래 FAIL 은 의도된 것이다)" 0 "" "100:47" >/dev/null
  if [ "$fails" = 1 ]; then pass "  시퀀스 시험의 실패가 최종 판정에 반영된다(서브셸 아님)"
  else fail "  시퀀스 시험 실패가 삼켜진다 — 검증기가 거짓 PASS 를 낸다"; fi
  fails=$_meta_before

  echo "신호 판독:"
  _td=$(mktemp -d)
  # SoC sysfs 판독
  for _z in 0 1 5 6; do mkdir -p "$_td/thermal_zone$_z"; done
  echo 46800 > "$_td/thermal_zone0/temp"; echo 45000 > "$_td/thermal_zone1/temp"
  echo 91200 > "$_td/thermal_zone5/temp"; echo 45800 > "$_td/thermal_zone6/temp"
  BB_TP_THERMAL_ROOT="$_td" tp_read_soc
  ck "  SoC 최고 zone 온도" 91 "$SOC_C"; ck "  SoC 최고 zone 인덱스" 5 "$SOC_ZONE"
  mkdir -p "$_td/thermal_zone7"; echo 2147483647 > "$_td/thermal_zone7/temp"
  mkdir -p "$_td/thermal_zone8"; echo -274000     > "$_td/thermal_zone8/temp"
  BB_TP_THERMAL_ROOT="$_td" tp_read_soc
  ck "  타당성 밴드 밖 센서는 최대값을 먹지 않는다" 91 "$SOC_C"
  BB_TP_THERMAL_ROOT="$_td/none" tp_read_soc
  ck "  zone 부재 → ABSENT(부재는 0 이 아니다)" "$ABSENT" "$SOC_C"
  # GPU CSV 꼬리 판독 — 새 스키마
  _csv="$_td/s.csv"
  printf 'ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,soc_temp,soc_zone\n' > "$_csv"
  printf '1787477017,19251,0,63,90.33,2457,96,,1.0,1,47,0\n' >> "$_csv"
  tp_read_gpu "$_csv"
  ck "  CSV 꼬리 → 데시와트 정수(90.33W)" 903 "$GPU_PWR_DW"
  ck "  CSV 꼬리 → ts 보존" 1787477017 "$GPU_TS"
  # 옛 스키마(10열)도 gpu_pwr 는 5번째 열이다
  _csv2="$_td/legacy.csv"
  printf 'ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n' > "$_csv2"
  printf '1787400000,60000,0,70,45.0,2400,50,,1.0,1\n' >> "$_csv2"
  tp_read_gpu "$_csv2"
  ck "  옛 스키마 CSV 도 전력 판독(하위호환)" 450 "$GPU_PWR_DW"
  # gpu_pwr 가 빈 칸(부재)이면 ABSENT
  printf '1787400001,60000,0,70,,2400,50,,1.0,1\n' >> "$_csv2"
  tp_read_gpu "$_csv2"
  ck "  gpu_pwr 빈 칸 → ABSENT" "$ABSENT" "$GPU_PWR_DW"
  # 헤더만 있는 파일 / 부재 파일
  printf 'ts,mem_avail\n' > "$_td/empty.csv"; tp_read_gpu "$_td/empty.csv"
  ck "  데이터행 없음 → ABSENT" "$ABSENT" "$GPU_PWR_DW"
  tp_read_gpu "$_td/nope.csv"
  ck "  파일 부재 → ABSENT(예외 아님)" "$ABSENT" "$GPU_PWR_DW"

  echo "dry-run:"
  # "kill 하지 않는다"는 주장이 아니라 관측이어야 한다 — docker 를 스텁으로 가로챈다.
  _calls="$_td/docker_calls"; : > "$_calls"
  BB_TP_EVENTS="$_td/events.jsonl"
  docker(){ echo "$*" >> "$_calls"; }
  _sd="$DRY_RUN"; _sm="$MODE"
  DRY_RUN=1; MODE="dry-run"
  tp_fire gpu_pwr_sustained "gpu=90W buckets=60/0 streak=0" '"gpu_bucket":60' "cafe1234" >/dev/null; _rc=$?
  if [ "$_rc" = 1 ] && [ ! -s "$_calls" ]; then pass "  dry-run 은 docker 를 부르지 않는다(호출 0건)"
  else fail "  dry-run 인데 docker 호출됨/반환 이상 (rc=$_rc)"; fi
  if grep -q '"kind":"thermal_trip_dryrun"' "$BB_TP_EVENTS" && grep -q '"mode":"dry-run"' "$BB_TP_EVENTS"; then
    pass "  dry-run 판정이 이벤트로 남는다(kind·mode 표기)"
  else fail "  dry-run 이벤트 누락/표기 불량"; fi
  if grep -q '"kind":"thermal_trip"' "$BB_TP_EVENTS"; then
    fail "  dry-run 이 실무장 이벤트를 냈다 — 하류가 kill 로 오독한다"
  else pass "  dry-run 은 실무장 이벤트를 내지 않는다(하류 오독 방지)"; fi
  DRY_RUN=0; MODE="armed"; : > "$BB_TP_EVENTS"
  tp_fire gpu_pwr_sustained "gpu=90W buckets=60/0 streak=0" '"gpu_bucket":60' "cafe1234" >/dev/null; _rc=$?
  if [ "$_rc" = 0 ] && grep -q "kill cafe1234" "$_calls"; then
    pass "  실무장은 실제로 docker kill 한다(대조 — 스텁이 살아있음의 증명)"
  else fail "  실무장 kill 경로 이상 (rc=$_rc)"; fi
  if grep -q '"mode":"armed"' "$BB_TP_EVENTS" && grep -q '"kind":"thermal_kill_ack"' "$BB_TP_EVENTS"; then
    pass "  실무장 이벤트에 mode=armed 와 kill_ack 이 남는다"
  else fail "  실무장 이벤트 표기 불량"; fi
  # 2026-09-03(⑥ 잔여 고정 · plan_26090317 P1): 지금까지 이 절의 docker 스텁은 **항상 rc=0** 이라
  #   kill 실패 분기가 시험 밖이었다. 그래서 "파이프가 rc 를 삼켜 무조건 kill_ack" 이라는 결함이
  #   자체검사 전부 초록인 채로 살아 있었다. 실패하는 스텁을 넣어 **양방향**을 본다.
  : > "$_calls"; : > "$BB_TP_EVENTS"
  docker(){ echo "$*" >> "$_calls"; [ "${1:-}" = "kill" ] && { echo "cannot kill" >&2; return 1; }; return 0; }
  tp_fire gpu_pwr_sustained "gpu=90W buckets=60/0 streak=0" '"gpu_bucket":60' "cafe1234" >/dev/null; _rc=$?
  if grep -q '"kind":"thermal_kill_failed"' "$BB_TP_EVENTS" \
     && ! grep -q '"kind":"thermal_kill_ack"' "$BB_TP_EVENTS"; then
    pass "  docker kill 실패 → kill_failed 발행·kill_ack 없음(관측 위조 방지)"
  else fail "  kill 실패인데 kill_ack 이 남았다 — 사후 분석이 방어 실패를 성공으로 읽는다"; fi
  docker(){ echo "$*" >> "$_calls"; }
  : > "$_calls"
  tp_fire gpu_pwr_sustained "gpu=90W buckets=60/0 streak=0" '"gpu_bucket":60' "" >/dev/null; _rc=$?
  if [ "$_rc" = 2 ] && [ ! -s "$_calls" ]; then pass "  매칭 0 은 kill 없이 nomatch 반환"
  else fail "  매칭 0 경로 이상 (rc=$_rc)"; fi
  unset -f docker; DRY_RUN="$_sd"; MODE="$_sm"; BB_TP_EVENTS=""
  rm -rf "$_td"

  # ── BB_TARGET_PREDICATE_SELFTEST_V1 ─────────────────────────────────────
  # 킬 대상 술어의 **음성대조**(plan_26090415 §3.3 · CP0). 양성 사례만 두면 "좁혔다"가
  # 증명되지 않는다 — 이 블록의 존재 이유는 측정 도구 컨테이너가 대상 목록에서 **실제로
  # 빠지는지**이고, 동시에 이름에 vllm 이 없는 서빙 컨테이너가 **여전히 잡히는지**다.
  # GuideLLM 이미지 리터럴은 tripwire 로서의 하드코딩이다(workflow.md §4종 안티패턴 판정표:
  # "변경 시 리뷰를 강제하는 닫힌 목록" = 정당). 벤치 이미지를 바꾸면 이 시험이 먼저
  # 빨간불을 켜고, 그때 술어를 다시 본다.
  # 이 블록도 세 워치독 사이에서 parity tripwire 의 대상이다.
  tmchk(){ # $1=설명 $2=기대rc(0=대상·1=제외) $3=IMAGE $4=NAMES
    bb_target_match "deadbeef" "$3" "$4"; local got=$?
    if [ "$got" = "$2" ]; then echo "  [PASS] $1"
    else echo "  [FAIL] $1 (기대rc=$2 실제=$got)"; fails=1; fi
  }
  tmchk "대상: 서빙 이미지(easy-vllm)"                 0 "easy-vllm:0.19.1-cu130-aarch64-wheel" "vllm-serve-container"
  tmchk "대상: 이름에 vllm 없는 서빙도 이미지로 포착"  0 "easy-vllm:0.27.1-cu133-aarch64-source" "mn-hy3-master"
  tmchk "대상: 업스트림 공식 서버 이미지"              0 "vllm/vllm-openai:latest" "openai-server"
  tmchk "대상: 이름에만 vllm(trial)"                   0 "ubuntu:24.04" "vllm_trial01"
  tmchk "대상: 대소문자 무관"                          0 "EASY-VLLM:0.19.1" "SERVE"
  tmchk "★음성대조: GuideLLM 벤치 컨테이너는 제외"     1 "ghcr.io/vllm-project/guidellm:latest" "guidellm-bench"
  tmchk "★음성대조: GuideLLM digest 핀도 제외"         1 "ghcr.io/vllm-project/guidellm@sha256:00ff" "guidellm-bench"
  tmchk "제외: 무관 컨테이너(NGC 베이스)"              1 "nvcr.io/nvidia/pytorch:25.08-py3" "build-helper"
  # ── /BB_TARGET_PREDICATE_SELFTEST_V1 ────────────────────────────────────

  [ "$fails" = 0 ] && { echo "self-test: PASS"; exit 0; } || { echo "self-test: FAIL"; exit 2; }
fi

# ── 운전 루프 ────────────────────────────────────────────────────────────
[ -n "${TW_PIDFILE:-}" ] && echo "$$" > "$TW_PIDFILE"
if [ -z "$BB_TP_NODE_DIR" ]; then
  log "FAIL: 노드 디렉터리를 알 수 없다 — BB_TP_NODE_DIR 또는 BB_TP_EVENTS 를 지정하라."
  log "      (GPU 전력은 수집기 CSV <node_dir>/samples/<UTC일자>.csv 에서 읽는다)"
  exit 1
fi
[ "$DRY_RUN" = 1 ] && log "★★ DRY-RUN — 판정만 하고 docker kill 을 하지 않는다(관측 전용). 이 인스턴스는 호스트를 지키지 않는다."
tp_read_soc
if [ "$SOC_C" = "$ABSENT" ]; then
  SOC_DESC="SoC 축 OFF(zone 부재: $BB_TP_THERMAL_ROOT) — 전력 축만 무장"
else
  SOC_DESC="SoC 축 ON(현재 ${SOC_C}C @zone${SOC_ZONE})"
fi
log "start mode=$MODE filter='$FILTER' interval=${INTERVAL}s params=$PARAMS_SRC node=$BB_TP_NODE_DIR"
log "  규칙: gpu>=$(( BB_TP_GPU_PWR_DW / 10 ))W ${BB_TP_GPU_SUSTAIN_S}s 지속 · soc>=${BB_TP_SOC_WARN_C}C ${BB_TP_SOC_SUSTAIN_S}s 지속 · soc>=${BB_TP_SOC_HARD_C}C ${BB_TP_SOC_HARD_POLLS}폴 · $SOC_DESC"
# 미교정 파라미터를 기동 로그에 **매번** 밝힌다 — 교정 전 판정을 실측인 척하지 않는다.
[ -n "$BB_TP_UNCALIBRATED" ] && log "  ⚠ 미교정(외부 보고 역산 — 이 노드 실측 분포 없음): $BB_TP_UNCALIBRATED"
emit_event "thermal_watchdog_start" "\"filter\":\"$FILTER\",\"gpu_pwr_dw\":$BB_TP_GPU_PWR_DW,\"gpu_sustain_s\":$BB_TP_GPU_SUSTAIN_S,\"soc_warn_c\":$BB_TP_SOC_WARN_C,\"soc_sustain_s\":$BB_TP_SOC_SUSTAIN_S,\"soc_hard_c\":$BB_TP_SOC_HARD_C,\"soc_axis\":\"$([ "$SOC_C" = "$ABSENT" ] && echo off || echo on)\",\"uncalibrated\":\"$BB_TP_UNCALIBRATED\""

stale_ep=0; last_hb=0; peak_gpu=0; peak_soc=0
while true; do
  read -r now day <<< "$(date -u +'%s %F')"
  tp_read_soc
  tp_read_gpu "$BB_TP_NODE_DIR/samples/$day.csv"
  # stale 판정: 수집기 꼬리가 뒤처졌는가. 진입/이탈만 남긴다(폴마다 쓰면 홍수).
  stale=0
  if [ "$GPU_PWR_DW" -ne "$ABSENT" ] && [ $(( now - GPU_TS )) -gt "$BB_TP_STALE_AFTER_S" ]; then
    stale=1
  fi
  if [ "$stale" != "$stale_ep" ]; then
    if [ "$stale" = 1 ]; then
      log "GPU-STALE 수집기 꼬리가 $(( now - GPU_TS ))s 뒤짐(>${BB_TP_STALE_AFTER_S}s) — 전력 축 동결. 수집기 확인: systemctl status easy-vllm-blackbox-collect $(ts)"
      emit_event "thermal_gpu_stale" "\"lag_s\":$(( now - GPU_TS )),\"gpu_bucket\":$TP_GPU_BUCKET,\"axis\":\"frozen\""
    else
      log "GPU-STALE 해제 — 전력 축 재개 $(ts)"
      emit_event "thermal_gpu_stale_clear" "\"gpu_bucket\":$TP_GPU_BUCKET"
    fi
    stale_ep=$stale
  fi

  tp_step "$GPU_PWR_DW" "$SOC_C" "$stale"; tripped=$?
  [ "$TP_GPU_BUCKET" -gt "$peak_gpu" ] && peak_gpu=$TP_GPU_BUCKET
  [ "$TP_SOC_BUCKET" -gt "$peak_soc" ] && peak_soc=$TP_SOC_BUCKET

  if [ "$HB_SEC" -gt 0 ] && [ $(( now - last_hb )) -ge "$HB_SEC" ]; then
    log "HB[$MODE] gpu=$(( GPU_PWR_DW == ABSENT ? 0 : GPU_PWR_DW / 10 ))W$([ "$GPU_PWR_DW" = "$ABSENT" ] && echo '(na)') bucket=${TP_GPU_BUCKET}/${BB_TP_GPU_SUSTAIN_S} peak=${peak_gpu} | soc=${SOC_C}C bucket=${TP_SOC_BUCKET}/${BB_TP_SOC_SUSTAIN_S} peak=${peak_soc} | stale=$stale $(ts)"
    last_hb=$now; peak_gpu=$TP_GPU_BUCKET; peak_soc=$TP_SOC_BUCKET
  fi

  if [ "$tripped" = 0 ]; then
    _w=$([ "$GPU_PWR_DW" = "$ABSENT" ] && echo na || echo $(( GPU_PWR_DW / 10 )))
    _c=$([ "$SOC_C" = "$ABSENT" ] && echo na || echo "$SOC_C")
    human="gpu=${_w}W soc=${_c}C buckets=${TP_GPU_BUCKET}/${TP_SOC_BUCKET} streak=${TP_SOC_HARD_STREAK}"
    ev="\"gpu_pwr_w\":$([ "$GPU_PWR_DW" = "$ABSENT" ] && echo null || echo $(( GPU_PWR_DW / 10 )))"
    ev="$ev,\"soc_temp_c\":$([ "$SOC_C" = "$ABSENT" ] && echo null || echo "$SOC_C")"
    ev="$ev,\"gpu_bucket\":$TP_GPU_BUCKET,\"soc_bucket\":$TP_SOC_BUCKET,\"soc_hard_streak\":$TP_SOC_HARD_STREAK"
    tp_fire "$TP_RULE" "$human" "$ev" "$(targets)"
    case $? in
      0) tp_reset ;;                 # 실제 kill — 부하가 끊겼으니 축적분도 리셋
      1) tp_reset ;;                 # dry-run — 리셋하지 않으면 매 폴 트립해 로그가 폭주한다
      *) sleep 5; tp_reset ;;        # 매칭 0
    esac
  fi
  sleep "$INTERVAL"
done
