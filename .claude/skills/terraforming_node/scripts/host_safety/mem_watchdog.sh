#!/bin/bash
# mem_watchdog.sh — GB10 통합메모리 OOM 호스트-하드다운 방지 안전망(ops backstop).
#   GB10 통합메모리는 GPU OOM 시 호스트 전체가 하드다운된다(GPU/호스트 메모리 풀 공유 →
#   ping-OK→No-route→down, 재부팅 필요). 이 워치독은 /proc/meminfo MemAvailable 을 폴링해
#   임계 밑으로 가면 대상 컨테이너를 docker kill 한다 → 호스트 대신 컨테이너를 희생.
#   (Ray master 는 그 워커 actor-unavailable 로 깨끗이 종료 → 호스트 보존.)
# 사용: mem_watchdog.sh [name_filter|@vllm] [threshold_mib] [interval_sec]
#   name_filter : docker ps --filter name=<filter> 부분일치(예: slave / master / vllm_trial)
#   생략 또는 '@vllm' = 광역 모드 — **이미지 저장소 이름** 또는 컨테이너 이름에 vllm 이 포함된
#     전 컨테이너(trial 포함). 레지스트리·조직 경로는 매칭 평면 밖이다(BB_TARGET_PREDICATE_V1).
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
#
# ── pgid 표적 모드 (2026-09-23 · plan_26092311 N-D5 / N3) ────────────────────────────────
# 사용: mem_watchdog.sh --pgid <N> --starttime <S> [--thresh-mib <T>] [--interval-s <I>]
#       mem_watchdog.sh --self-test
#   native(비-Docker) 서빙은 docker ps 에 안 보이므로 컨테이너 필터로는 **매칭 0** 이다(F9).
#   그래서 표적을 이름이 아니라 **프로세스 그룹 정체**(pgid + 그룹 리더의 /proc/<pgid>/stat
#   starttime)로 받는다. 이름 기반 매칭(pkill/pgrep -f)은 쓰지 않는다 — `VLLM::Worker_TP*` 가
#   argv 를 바꿔 이름 정리가 실패했고(F11 · 83.8 GiB 잔존) C6 이 금지한다.
#   · 트립 시 **그 그룹에만** SIGTERM → 유예(MEMWATCH_PGID_GRACE_S, 기본 2s) → SIGKILL.
#     두 신호 모두 직전에 리더 starttime 을 재확인한다(PID 재사용 가드). 불일치면 **죽이지 않는다**.
#     단 TERM 뒤 리더만 사라지고 멤버가 남은 경우엔 KILL 을 그대로 보낸다 — 커널은 그룹 멤버가
#     하나라도 사는 동안 그 번호를 새 프로세스에 재할당하지 않으므로 같은 그룹이다.
#   · 리더가 사라지면(또는 좀비인데 살아 있는 멤버 0) 표적 종료로 보고 exit 0.
#   · 무장 시점 검증 실패(형식 · 자기 그룹 · 부재 · 리더 아님 · starttime 불일치) = exit 2(fail-closed).
#   · 컨테이너 모드(위치 인자)와 결합할 수 없다 — 섞이면 exit 2. 컨테이너 모드 동작은 불변이다.
set -u
# pgid 모드 플래그 파싱. 첫 인자가 플래그일 때만 이 분기를 탄다 — 위치 인자 컨테이너 모드는 그대로다.
MW_TARGET_KIND="container"; MW_PGID=""; MW_STARTTIME=""; MW_SELFTEST=0
mw_usage_fail(){ echo "[mem-watchdog] FAIL: $* (사용: mem_watchdog.sh --pgid <N> --starttime <S> [--thresh-mib <T>] [--interval-s <I>] | [name_filter|@vllm] [threshold_mib] [interval_sec])" >&2; exit 2; }
case "${1:-}" in
  --pgid|--starttime|--thresh-mib|--interval-s)
    MW_TARGET_KIND="pgid"; _mw_thresh=""; _mw_interval=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --pgid)       [ $# -ge 2 ] || mw_usage_fail "--pgid 값 누락"; MW_PGID="$2"; shift 2 ;;
        --starttime)  [ $# -ge 2 ] || mw_usage_fail "--starttime 값 누락"; MW_STARTTIME="$2"; shift 2 ;;
        --thresh-mib) [ $# -ge 2 ] || mw_usage_fail "--thresh-mib 값 누락"; _mw_thresh="$2"; shift 2 ;;
        --interval-s) [ $# -ge 2 ] || mw_usage_fail "--interval-s 값 누락"; _mw_interval="$2"; shift 2 ;;
        *) mw_usage_fail "pgid 모드에 허용되지 않는 인자 [$1] — 컨테이너 필터(위치 인자)와 pgid 표적은 결합할 수 없다" ;;
      esac
    done
    # 값은 argv 로만 들어오고 산술·kill 인자로만 쓰인다 — 정수 형식을 강제해 주입 여지를 없앤다.
    case "$MW_PGID" in ''|0*|*[!0-9]*) mw_usage_fail "--pgid 는 선행 0 없는 양의 정수여야 한다(got=[$MW_PGID])" ;; esac
    [ "${#MW_PGID}" -le 9 ] || mw_usage_fail "--pgid 가 너무 길다(got=[$MW_PGID])"
    # pgid 1 은 init 그룹이다 — `kill -- -1` 은 **호출자가 보낼 수 있는 전 프로세스**가 된다.
    [ "$MW_PGID" -gt 1 ] || mw_usage_fail "--pgid 1 은 금지(kill -- -1 = 전 프로세스)"
    [ -n "$MW_STARTTIME" ] || mw_usage_fail "--starttime 필수(PID 재사용 가드 없이 그룹을 죽이지 않는다)"
    case "$MW_STARTTIME" in *[!0-9]*) mw_usage_fail "--starttime 은 정수여야 한다(got=[$MW_STARTTIME])" ;; esac
    case "$MW_STARTTIME" in 0?*) mw_usage_fail "--starttime 에 선행 0 금지(got=[$MW_STARTTIME])" ;; esac
    [ "${#MW_STARTTIME}" -le 20 ] || mw_usage_fail "--starttime 이 너무 길다"
    case "$_mw_thresh"   in *[!0-9]*) mw_usage_fail "--thresh-mib 는 정수여야 한다(got=[$_mw_thresh])" ;; esac
    case "$_mw_interval" in *[!0-9]*|0*) mw_usage_fail "--interval-s 는 양의 정수여야 한다(got=[$_mw_interval])" ;; esac
    # 아래 FILTER/THRESH_MIB/INTERVAL 정의줄을 **그대로** 재사용한다(빈 값 = 컨테이너 모드와 같은 기본값).
    set -- "pgid:$MW_PGID" "$_mw_thresh" "$_mw_interval"
    ;;
  --self-test)
    [ $# -eq 1 ] || mw_usage_fail "--self-test 는 단독으로만 쓴다"
    MW_SELFTEST=1 ;;
  *)
    # 컨테이너 모드: 뒤쪽 위치에 pgid 플래그가 섞이면 두 표적의 결합이다 → exit 2.
    for _mw_a in "$@"; do
      case "$_mw_a" in --pgid|--starttime|--thresh-mib|--interval-s|--self-test)
        mw_usage_fail "컨테이너 필터와 [$_mw_a] 를 결합할 수 없다" ;; esac
    done ;;
esac
FILTER="${1:-@vllm}"
THRESH_MIB="${2:-10240}"   # MemAvailable < 10 GiB → trip
INTERVAL="${3:-1}"
HB_SEC="${MEMWATCH_HEARTBEAT_SEC:-15}"
[ -n "${MEMWATCH_PIDFILE:-}" ] && echo "$$" > "$MEMWATCH_PIDFILE"
ts(){ date -u +%FT%TZ; }
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

# ── pgid 표적 정체 판독 (이름 기반 ✗ · /proc/<pid>/stat 만 읽는다) ─────────────────────────
# /proc/<pid>/stat 의 2번 필드(comm)는 공백·괄호를 담을 수 있다 → **마지막 ')'** 뒤부터 센다.
#   그 뒤 필드: $1=state(3) $2=ppid(4) $3=pgrp(5) … $20=starttime(22).
MW_GRACE_S="${MEMWATCH_PGID_GRACE_S:-2}"
case "$MW_GRACE_S" in ''|*[!0-9]*) MW_GRACE_S=2 ;; esac
mw_proc_ident(){   # $1=pid → stdout "pgrp starttime state" · rc 1=부재/판독 불가
  local _s="" _rest
  read -r _s 2>/dev/null < "/proc/$1/stat" || [ -n "$_s" ] || return 1
  case "$_s" in *")"*) ;; *) return 1 ;; esac
  _rest="${_s##*) }"
  # shellcheck disable=SC2086
  set -- $_rest
  [ $# -ge 20 ] || return 1
  printf '%s %s %s\n' "$3" "${20}" "$1"
}
mw_group_live_pids(){   # $1=pgid → stdout: 그 그룹의 **살아 있는**(좀비 제외) pid 목록
  local _pg="$1" _f _s _rest _pid
  for _f in /proc/[0-9]*/stat; do
    _s=""; read -r _s 2>/dev/null < "$_f" || [ -n "$_s" ] || continue
    _pid="${_f#/proc/}"; _pid="${_pid%/stat}"
    _rest="${_s##*) }"
    # shellcheck disable=SC2086
    set -- $_rest
    [ "${3:-}" = "$_pg" ] && [ "${1:-}" != "Z" ] && printf '%s\n' "$_pid"
  done
  return 0
}
# 표적 상태: 0=살아 있고 정체 일치 · 1=종료(리더 부재, 또는 좀비 리더 + 산 멤버 0) · 3=정체 불일치
MW_TARGET_REASON=""
mw_target_state(){   # $1=pgid $2=starttime
  local _id _pgrp _st _state
  if ! _id="$(mw_proc_ident "$1")"; then MW_TARGET_REASON="리더 pid=$1 부재"; return 1; fi
  read -r _pgrp _st _state <<<"$_id"
  if [ "$_st" != "$2" ]; then MW_TARGET_REASON="리더 starttime 불일치(want=$2 got=$_st — PID 재사용)"; return 3; fi
  if [ "$_pgrp" != "$1" ]; then MW_TARGET_REASON="pid=$1 이 그룹 리더가 아니다(pgrp=$_pgrp)"; return 3; fi
  if [ "$_state" = "Z" ] && [ -z "$(mw_group_live_pids "$1")" ]; then
    MW_TARGET_REASON="리더 좀비 · 살아 있는 그룹 멤버 0"; return 1
  fi
  MW_TARGET_REASON="alive"; return 0
}
# 그룹 사살: 0=그룹 소멸 확인 · 3=거부(정체 불일치 — 죽이지 않았다) · 4=표적 이미 종료 · 1=KILL 뒤에도 잔존
mw_kill_pgid(){   # $1=pgid $2=starttime
  local _pg="$1" _st="$2" _i _n _live
  mw_target_state "$_pg" "$_st"
  case $? in
    1) echo "[mem-watchdog] KILL-SKIP target_kind=pgid pgid=$_pg — 표적 이미 종료($MW_TARGET_REASON) $(ts)"; return 4 ;;
    3) echo "[mem-watchdog] KILL-REFUSED target_kind=pgid pgid=$_pg — $MW_TARGET_REASON. 죽이지 않는다 $(ts)" >&2; return 3 ;;
  esac
  echo "[mem-watchdog] SIGTERM target_kind=pgid pgid=$_pg starttime=$_st members=$(mw_group_live_pids "$_pg" | tr '\n' ' ')$(ts)"
  kill -TERM -- "-$_pg" 2>/dev/null
  _n=$(( MW_GRACE_S * 4 )); _i=0
  while [ "$_i" -lt "$_n" ]; do
    [ -z "$(mw_group_live_pids "$_pg")" ] && { echo "[mem-watchdog] GROUP-GONE target_kind=pgid pgid=$_pg after=SIGTERM $(ts)"; return 0; }
    sleep 0.25; _i=$(( _i + 1 ))
  done
  _live="$(mw_group_live_pids "$_pg")"
  [ -z "$_live" ] && { echo "[mem-watchdog] GROUP-GONE target_kind=pgid pgid=$_pg after=SIGTERM $(ts)"; return 0; }
  # KILL 직전 재확인. 리더가 있으면 starttime 이 같아야 한다. 리더가 없고 멤버만 남았으면 커널이
  #   그 번호를 재할당하지 않은 **같은 그룹**이다(멤버가 pgid 번호를 붙들고 있다) → KILL 을 보낸다.
  if mw_proc_ident "$_pg" >/dev/null; then
    mw_target_state "$_pg" "$_st"
    if [ $? -eq 3 ]; then
      echo "[mem-watchdog] KILL-REFUSED target_kind=pgid pgid=$_pg — SIGKILL 직전 재확인: $MW_TARGET_REASON. 죽이지 않는다 $(ts)" >&2
      return 3
    fi
  fi
  echo "[mem-watchdog] SIGKILL target_kind=pgid pgid=$_pg (유예 ${MW_GRACE_S}s 뒤 잔존: $(printf '%s' "$_live" | tr '\n' ' '))$(ts)"
  kill -KILL -- "-$_pg" 2>/dev/null
  _i=0
  while [ "$_i" -lt 8 ]; do
    [ -z "$(mw_group_live_pids "$_pg")" ] && { echo "[mem-watchdog] GROUP-GONE target_kind=pgid pgid=$_pg after=SIGKILL $(ts)"; return 0; }
    sleep 0.25; _i=$(( _i + 1 ))
  done
  return 1
}

# ── 자체시험 (--self-test · docker 불요 · 이 시험이 띄운 프로세스만 죽인다) ─────────────────
if [ "$MW_SELFTEST" = "1" ]; then
  st_fail=0
  st_chk(){ if [ "$2" = "$3" ]; then echo "  [PASS] $1"; else echo "  [FAIL] $1 (got=$2 want=$3)"; st_fail=1; fi; }
  ST_SELF="${BASH_SOURCE[0]}"
  st_tmp="$(mktemp -d)"
  ST_GROUPS=""    # "pgid:starttime" — 정리 대상은 이 목록뿐이다
  # 이중 fork + setsid: 리더가 새 그룹의 리더가 되고 부모가 init(또는 subreaper)로 넘어가 좀비가 남지 않는다.
  #   $1=라벨  $2=모양(plain|immune) → stdout "pgid starttime"
  st_spawn(){
    local _pf="$st_tmp/$1.pid" _i=0 _pid _id
    rm -f "$_pf"
    if [ "$2" = "immune" ]; then
      # TERM 무시 멤버 1개 포함 — SIGKILL 승격 경로를 밟게 한다(SIG_IGN 은 exec 를 넘어 상속된다).
      ( setsid bash -c 'echo $$ > "$1"; ( trap "" TERM; exec sleep 300 ) & sleep 300 & wait' _ "$_pf" \
          </dev/null >/dev/null 2>&1 & )
    else
      ( setsid bash -c 'echo $$ > "$1"; sleep 300 & wait' _ "$_pf" </dev/null >/dev/null 2>&1 & )
    fi
    while [ ! -s "$_pf" ] && [ "$_i" -lt 40 ]; do sleep 0.05; _i=$((_i+1)); done
    _pid="$(cat "$_pf" 2>/dev/null)"
    sleep 0.2   # 멤버 fork 대기
    # 서브셸(프로세스 치환)에서 불리므로 전역을 여기서 바꾸지 않는다 — 호출부가 ST_GROUPS 에 적고 빈 값을 FAIL 로 센다.
    _id="$(mw_proc_ident "$_pid")" || return 1
    set -- $_id
    printf '%s %s\n' "$_pid" "$2"
  }
  st_cleanup(){   # 이 시험이 기동한 그룹만, 정체 확인 뒤에만 KILL
    local _g _p _s
    for _g in $ST_GROUPS; do
      _p="${_g%%:*}"; _s="${_g##*:}"
      mw_target_state "$_p" "$_s" >/dev/null 2>&1 && kill -KILL -- "-$_p" 2>/dev/null
      [ -z "$(mw_group_live_pids "$_p")" ] || {
        # 리더가 이미 없고 멤버만 남았으면 같은 그룹이다(번호 미재할당) — 그룹째 정리한다.
        mw_proc_ident "$_p" >/dev/null || kill -KILL -- "-$_p" 2>/dev/null; }
    done
    rm -rf "$st_tmp"
  }
  trap 'st_cleanup' EXIT

  echo "인자 검증(음성대조):"
  bash "$ST_SELF" --pgid 12345 >/dev/null 2>&1;                         st_chk "--pgid 단독(starttime 누락) → exit 2" "$?" 2
  bash "$ST_SELF" --pgid abc --starttime 5 >/dev/null 2>&1;             st_chk "--pgid 비정수 → exit 2" "$?" 2
  bash "$ST_SELF" --pgid 1 --starttime 5 >/dev/null 2>&1;               st_chk "--pgid 1(kill -- -1) → exit 2" "$?" 2
  bash "$ST_SELF" --pgid 0 --starttime 5 >/dev/null 2>&1;               st_chk "--pgid 0 → exit 2" "$?" 2
  bash "$ST_SELF" --pgid 0123 --starttime 5 >/dev/null 2>&1;            st_chk "--pgid 선행 0 → exit 2" "$?" 2
  bash "$ST_SELF" --pgid 12345 --starttime '5;id' >/dev/null 2>&1;      st_chk "--starttime 주입 문자열 → exit 2" "$?" 2
  bash "$ST_SELF" --pgid 12345 --starttime 5 --interval-s 0 >/dev/null 2>&1; st_chk "--interval-s 0 → exit 2" "$?" 2
  bash "$ST_SELF" --pgid 12345 --starttime 5 myfilter >/dev/null 2>&1;  st_chk "pgid + 위치 필터 결합 → exit 2" "$?" 2
  bash "$ST_SELF" myfilter 10240 2 --pgid 5 >/dev/null 2>&1;            st_chk "컨테이너 필터 + --pgid 결합 → exit 2" "$?" 2
  bash "$ST_SELF" --self-test extra >/dev/null 2>&1;                    st_chk "--self-test 에 인자 결합 → exit 2" "$?" 2

  echo "무장 시점 정체 검증:"
  read -r G1 S1 < <(st_spawn g1 plain)
  [ -n "${G1:-}" ] && [ -n "${S1:-}" ] || { echo "  [FAIL] 시험 그룹 기동 실패(G1)"; exit 1; }
  ST_GROUPS="$ST_GROUPS $G1:$S1"
  # 부재 pid: 존재하지 않는 번호를 직접 찾는다(추측 상수 ✗)
  _absent=4194000; while [ -e "/proc/$_absent" ]; do _absent=$((_absent+1)); done
  timeout 10 bash "$ST_SELF" --pgid "$_absent" --starttime 1 --thresh-mib 0 >/dev/null 2>&1
  st_chk "표적 부재 → 무장 거부 exit 2" "$?" 2
  timeout 10 bash "$ST_SELF" --pgid "$G1" --starttime "$((S1+1))" --thresh-mib 0 >/dev/null 2>&1
  st_chk "starttime 불일치 → 무장 거부 exit 2" "$?" 2
  mw_target_state "$G1" "$S1"; st_chk "  무장 거부 뒤 표적 생존" "$?" 0
  # 자기 그룹 표적: 워치독 자식은 이 셸의 그룹을 물려받는다 → 그 그룹을 표적으로 주면 거부해야 한다.
  #   (--thresh-mib 0 = 트립 불가 — 거부가 깨졌어도 이 시험이 자기 그룹을 죽이지 않게 한다)
  _my_pg="$(mw_proc_ident "$$" | awk '{print $1}')"
  _my_st="$(mw_proc_ident "$_my_pg" | awk '{print $2}')"
  if [ -n "$_my_st" ]; then
    # ⚠ `timeout --foreground` 필수 — 기본 timeout 은 자식을 **새 프로세스그룹**에 넣어(setpgid) 이 시험의
    #   전제(자식이 이 셸의 그룹을 물려받는다)를 깨뜨린다(2026-09-23 첫 실행에서 124 로 드러났다).
    timeout --foreground 10 bash "$ST_SELF" --pgid "$_my_pg" --starttime "$_my_st" --thresh-mib 0 >/dev/null 2>&1
    st_chk "자기 프로세스그룹 표적 → 무장 거부 exit 2" "$?" 2
  else
    echo "  [SKIP] 자기 그룹 리더가 이 네임스페이스에서 보이지 않는다(pgid=$_my_pg)"
  fi

  echo "사살 함수 — 정체 불일치 거부:"
  mw_kill_pgid "$G1" "$((S1+1))" >/dev/null 2>&1; st_chk "starttime 불일치 → 거부 rc=3" "$?" 3
  mw_target_state "$G1" "$S1"; st_chk "  거부 뒤 표적 그룹 생존(죽이지 않았다)" "$?" 0

  echo "pgid 모드 실주행 — 트립 → 표적 그룹만 사살 → 리더 소멸 → exit 0:"
  read -r G2 S2 < <(st_spawn g2 immune)
  [ -n "${G2:-}" ] && [ -n "${S2:-}" ] || { echo "  [FAIL] 시험 그룹 기동 실패(G2)"; exit 1; }
  ST_GROUPS="$ST_GROUPS $G2:$S2"
  read -r G3 S3 < <(st_spawn g3 plain)     # 대조군: 절대 죽으면 안 된다
  [ -n "${G3:-}" ] && [ -n "${S3:-}" ] || { echo "  [FAIL] 시험 그룹 기동 실패(G3)"; exit 1; }
  ST_GROUPS="$ST_GROUPS $G3:$S3"
  _members_before="$(mw_group_live_pids "$G2" | wc -l)"
  # 임계를 MemAvailable 보다 높게 줘 첫 폴에서 트립시킨다(기존 임계 인자 재사용 · 새 시험 전용 손잡이 ✗).
  MEMWATCH_PGID_GRACE_S=1 MEMWATCH_HEARTBEAT_SEC=0 timeout 30 bash "$ST_SELF" \
      --pgid "$G2" --starttime "$S2" --thresh-mib 999999999 --interval-s 1 >"$st_tmp/run.log" 2>&1
  _rc=$?
  st_chk "리더 소멸 뒤 워치독 정상 종료 exit 0" "$_rc" 0
  [ "$_members_before" -ge 3 ]; st_chk "  시험 그룹이 리더+멤버(TERM 무시 포함) 로 기동됐다" "$?" 0
  [ -z "$(mw_group_live_pids "$G2")" ]; st_chk "  표적 그룹 전원 소멸" "$?" 0
  grep -q "TRIP MemAvailable=.* target_kind=pgid pgid=$G2" "$st_tmp/run.log"; st_chk "  TRIP 줄에 target_kind=pgid 표기" "$?" 0
  grep -q "SIGKILL target_kind=pgid pgid=$G2" "$st_tmp/run.log"; st_chk "  TERM 무시 멤버 → SIGKILL 승격" "$?" 0
  grep -q "docker kill" "$st_tmp/run.log"; st_chk "  pgid 모드는 docker kill 을 말하지 않는다" "$?" 1
  mw_target_state "$G3" "$S3"; st_chk "  ★대조군 그룹 생존(표적 밖은 건드리지 않는다)" "$?" 0

  echo "pgid 모드 — 트립 없이 리더 소멸 → exit 0:"
  read -r G4 S4 < <(st_spawn g4 plain)
  [ -n "${G4:-}" ] && [ -n "${S4:-}" ] || { echo "  [FAIL] 시험 그룹 기동 실패(G4)"; exit 1; }
  ST_GROUPS="$ST_GROUPS $G4:$S4"
  MEMWATCH_HEARTBEAT_SEC=0 timeout 20 bash "$ST_SELF" --pgid "$G4" --starttime "$S4" \
      --thresh-mib 0 --interval-s 1 >"$st_tmp/end.log" 2>&1 &
  _wd=$!
  sleep 1.5
  kill -0 "$_wd" 2>/dev/null; st_chk "  표적 생존 중 워치독 상주" "$?" 0
  mw_target_state "$G4" "$S4" && kill -KILL -- "-$G4" 2>/dev/null
  wait "$_wd"; st_chk "  리더 소멸 → 워치독 exit 0" "$?" 0
  grep -q "표적 종료 target_kind=pgid pgid=$G4" "$st_tmp/end.log"; st_chk "  종료 사유를 남긴다(침묵 종료 ✗)" "$?" 0

  echo "컨테이너 모드 회귀(가짜 docker · 동작 불변):"
  mkdir -p "$st_tmp/bin"
  printf '#!/bin/sh\necho "$*" >> "%s/docker_calls"\ncase "$1" in ps) echo cafe1234 ;; esac\nexit 0\n' "$st_tmp" > "$st_tmp/bin/docker"
  chmod +x "$st_tmp/bin/docker"
  PATH="$st_tmp/bin:$PATH" MEMWATCH_HEARTBEAT_SEC=0 timeout -s TERM 3 bash "$ST_SELF" x 999999999 1 >"$st_tmp/c.log" 2>&1
  grep -q "^\[mem-watchdog\] start filter='x' threshold=999999999MiB interval=1s heartbeat=0s pid=" "$st_tmp/c.log"
  st_chk "  start 줄 형식 불변" "$?" 0
  grep -q "TRIP MemAvailable=[0-9]*MiB < 999999999MiB → docker kill cafe1234 " "$st_tmp/c.log"
  st_chk "  TRIP 줄 형식 불변(docker kill)" "$?" 0
  grep -q "^kill cafe1234$" "$st_tmp/docker_calls"; st_chk "  docker kill 실호출(스텁)" "$?" 0
  grep -q "name=x" "$st_tmp/docker_calls"; st_chk "  이름 필터 전달 불변" "$?" 0
  grep -q "target_kind" "$st_tmp/c.log"; st_chk "  컨테이너 모드 로그에 pgid 문구가 섞이지 않는다" "$?" 1

  echo "self-test: $([ "$st_fail" = 0 ] && echo PASS || echo FAIL)"
  exit "$st_fail"
fi

# ── pgid 모드 무장 시점 검증 (fail-closed) ──────────────────────────────────────────────
if [ "$MW_TARGET_KIND" = "pgid" ]; then
  # 자기 그룹을 표적으로 받으면 트립이 워치독 자신(과 그 부모 셸)을 죽인다.
  _mw_self_id="$(mw_proc_ident "$$")" || mw_usage_fail "자기 /proc/$$/stat 판독 실패"
  set -- $_mw_self_id
  [ "$1" != "$MW_PGID" ] || mw_usage_fail "표적 pgid=$MW_PGID 가 워치독 자신의 프로세스그룹이다 — 서빙은 setsid 로 별도 그룹에서 띄워라"
  mw_target_state "$MW_PGID" "$MW_STARTTIME"
  case $? in
    0) ;;
    *) mw_usage_fail "무장 거부 — $MW_TARGET_REASON" ;;
  esac
fi
# ★ 정지 기록 (2026-09-01 · audit ㉛). start 만 있고 stop 이 없으면 저널에서
#   "돌고 있다"와 "사라졌다"가 구분되지 않는다.
trap '_rc=$?; echo "[mem-watchdog] stop rc=$_rc signal=${_mw_sig:-EXIT} $(ts)"; exit $_rc' EXIT
# 2026-09-03(㉛ 회귀 · plan_26090317 P1): 핸들러가 변수만 놓고 `exit` 하지 않아 bash 가 루프를
#   **계속 돌았다** — TERM 으로는 죽지 않고 SIGKILL 까지 가며, KILL 은 trap 을 안 돌므로
#   정지 기록도 남지 않는다. 즉 ㉛ 이 만들려던 기록은 TERM 경로에서 달성되지 않고, 그 대신
#   **정상 정지 경로가 사라졌다**(라이브 고아 워치독 1건이 그 결과다). 128+signum 으로 나간다.
trap '_mw_sig=TERM; exit 143' TERM
trap '_mw_sig=INT;  exit 130' INT
trap '_mw_sig=HUP;  exit 129' HUP
if [ "$MW_TARGET_KIND" = "pgid" ]; then
  # 시작 줄의 `filter='pgid:N'` 은 옛 파서(watchdog_start)가 그대로 읽고, target_kind 가 모드를 못박는다.
  echo "[mem-watchdog] start filter='$FILTER' target_kind=pgid pgid=$MW_PGID starttime=$MW_STARTTIME threshold=${THRESH_MIB}MiB interval=${INTERVAL}s heartbeat=${HB_SEC}s grace=${MW_GRACE_S}s pid=$$ $(ts)"
else
echo "[mem-watchdog] start filter='$FILTER' threshold=${THRESH_MIB}MiB interval=${INTERVAL}s heartbeat=${HB_SEC}s pid=$$ $(ts)"
fi
last_hb=0
min_since_hb=999999999   # 직전 HB 이후 1s-폴 최저치(P2 — 임계-하 순간 dip 을 60s HB 가 놓치지 않게, testlog_26071111 §0)
while true; do
  # pgid 모드: 매 폴 표적 정체를 본다. 표적이 끝났으면 지킬 것이 없다 → 정상 종료(exit 0).
  if [ "$MW_TARGET_KIND" = "pgid" ]; then
    mw_target_state "$MW_PGID" "$MW_STARTTIME"
    if [ $? -ne 0 ]; then
      _mw_left="$(mw_group_live_pids "$MW_PGID" | tr '\n' ' ')"
      echo "[mem-watchdog] 표적 종료 target_kind=pgid pgid=$MW_PGID — $MW_TARGET_REASON${_mw_left:+ · ⚠ 그룹 잔존 멤버: $_mw_left(정리는 down 의 몫)} $(ts)"
      exit 0
    fi
  fi
  # ★ 판독 실패는 판정 불가다 (2026-09-01 · audit ④). 종전 형태는 /proc/meminfo 를 못
  #   읽으면 산술 확장 오류로 **셸이 즉사**했고(비대화형 bash), 이 정본 워치독은
  #   policy:HOST_SAFETY_LAYERED_DEFENSE.C1 이 요구하는 실물 방어층이라 그 침묵 사망이
  #   곧 무방비다. 이 폴만 건너뛰고 큰 소리로 남긴다.
  _mem_kb="$(awk '/MemAvailable:/{print $2; exit}' /proc/meminfo 2>/dev/null || true)"
  case "$_mem_kb" in
    ''|*[!0-9]*)
      echo "[mem-watchdog] READ-FAIL /proc/meminfo MemAvailable 판독 실패(raw=[$_mem_kb]) — 이 폴은 판정하지 않는다 $(ts)"
      sleep "$INTERVAL"; continue ;;
  esac
  avail_mib=$(( _mem_kb / 1024 ))
  [ "$avail_mib" -lt "$min_since_hb" ] && min_since_hb=$avail_mib
  now=$(date +%s)
  if [ "$HB_SEC" -gt 0 ] && [ $(( now - last_hb )) -ge "$HB_SEC" ]; then
    echo "[mem-watchdog] HB MemAvailable=${avail_mib}MiB min=${min_since_hb}MiB $(ts)"
    last_hb=$now
    min_since_hb=$avail_mib
  fi
  if [ "$MW_TARGET_KIND" = "pgid" ] && [ "$avail_mib" -lt "$THRESH_MIB" ]; then
    echo "[mem-watchdog] TRIP MemAvailable=${avail_mib}MiB < ${THRESH_MIB}MiB → target_kind=pgid pgid=$MW_PGID starttime=$MW_STARTTIME SIGTERM→${MW_GRACE_S}s→SIGKILL $(ts)"
    mw_kill_pgid "$MW_PGID" "$MW_STARTTIME"
    case $? in
      0|4) ;;   # 소멸 확인 · 이미 종료 — 다음 폴의 정체 검사가 exit 0 으로 닫는다
      3) echo "[mem-watchdog] 표적 종료 target_kind=pgid pgid=$MW_PGID — 정체 불일치로 사살 거부(원 표적은 이미 없다) $(ts)"; exit 0 ;;
      *) echo "[mem-watchdog] KILL-FAILED target_kind=pgid pgid=$MW_PGID — SIGKILL 뒤에도 그룹 잔존: $(mw_group_live_pids "$MW_PGID" | tr '\n' ' ')— 방어가 성립하지 않았다 $(ts)" >&2 ;;
    esac
    sleep "$INTERVAL"; continue
  fi
  if [ "$avail_mib" -lt "$THRESH_MIB" ]; then
    ids=$(targets)
    if [ -n "$ids" ]; then
      echo "[mem-watchdog] TRIP MemAvailable=${avail_mib}MiB < ${THRESH_MIB}MiB → docker kill $ids $(ts)"
      # 2026-09-03(⑥ 잔여 · plan_26090317 P1): 파이프가 rc 를 삼켜 kill 실패가 **로그에서 성공과
      #   구분되지 않았다**. 이 워치독은 policy:HOST_SAFETY_LAYERED_DEFENSE.C1 의 실물 방어층이라
      #   "죽였다고 적혔는데 안 죽었다" 는 곧 무방비를 방어로 오독하게 만든다.
      _kout="$(docker kill $ids 2>&1)"; _krc=$?
      printf '%s\n' "$_kout" | sed 's/^/[mem-watchdog] /'
      if [ "$_krc" -ne 0 ]; then
        echo "[mem-watchdog] KILL-FAILED rc=$_krc targets=$ids — 방어가 성립하지 않았다 $(ts)" >&2
      fi
    else
      # 매칭 0 인데 임계 미달 = vllm 밖 원인(관측만) — 로그 폭주 방지 감속
      echo "[mem-watchdog] TRIP-nomatch MemAvailable=${avail_mib}MiB < ${THRESH_MIB}MiB (filter='$FILTER' 매칭 0) $(ts)"
      sleep 5
    fi
  fi
  sleep "$INTERVAL"
done
