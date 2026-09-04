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
set -u
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
echo "[mem-watchdog] start filter='$FILTER' threshold=${THRESH_MIB}MiB interval=${INTERVAL}s heartbeat=${HB_SEC}s pid=$$ $(ts)"
last_hb=0
min_since_hb=999999999   # 직전 HB 이후 1s-폴 최저치(P2 — 임계-하 순간 dip 을 60s HB 가 놓치지 않게, testlog_26071111 §0)
while true; do
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
