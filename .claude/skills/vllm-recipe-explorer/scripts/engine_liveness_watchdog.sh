#!/bin/bash
# engine_liveness_watchdog.sh — 엔진-행(SW-hang) 검출 → 컨테이너 graceful kill (계층 방어 심층층).
#   근거: 2026-07-11 768k prefill 크래시(testlog_26071111 §0). 마스터가 768k 요청 수락(POST 200) 직후
#   즉시 교착 — 메모리는 정상(11099MiB, 워치독 임계 위)인데 forward 가 멈춤(prompt/gen throughput 0,
#   /metrics 무응답) → ~22분 무음 후 호스트 wedge. RAM 워치독(mem_watchdog)은 MemAvailable 만 봐서
#   이 "메모리-정상-엔진-교착" 서브클래스를 **구조적으로 못 잡는다**. 이 워치독이 그 갭을 메운다:
#   컨테이너가 running 인데 (a) /metrics 무응답 또는 (b) num_requests_running>=1 인 채 prompt+gen 토큰
#   카운터가 STALL_SEC 정체 → 엔진 교착 판정 → docker kill(컨테이너 희생, 호스트 보존. Ray master 는
#   워커 actor-unavailable 로 깨끗이 종료). P1(입력상한 524288)이 1차 예방, 이건 잔여 SW-hang 심층방어.
#
# ⚠ 한계: 완전 즉발 SoC 프리즈(userspace 전체 동결)는 이 워치독 자신도 얼어 못 잡는다 → P1(예방)이 우선.
#   이 층은 "hang-with-limp"(엔진만 교착, 커널/워치독은 살아있음) 서브클래스 한정.
# ⚠ 오트립 방지: 정상 대량 prefill 중엔 gen 토큰=0 이지만 **prompt 토큰 카운터가 청크마다 전진**하므로
#   prompt+gen 합산 카운터로 판정(합이 정체해야 hang). STALL_SEC 기본 180s = 느린 청크에도 여유.
#   arming: /metrics 가 최소 1회 도달 확인(established) 후에만 무응답을 hang 으로 취급(기동 오검 차단).
# 사용: engine_liveness_watchdog.sh <container_name> <port> [stall_sec=180] [interval=10]
# env: LIVENESS_PIDFILE(자기 PID 기록 — 하네스 PID 정리) · LIVENESS_HB_SEC(하트비트, 기본 60)
# 정지: kill <pid> 로만(pkill -f 금지 — mem_watchdog 자기참조 선례 동일).
set -u
CTR="${1:?사용: engine_liveness_watchdog.sh <container_name> <port> [stall_sec] [interval]}"
PORT="${2:?port 필요}"
STALL_SEC="${3:-180}"
INTERVAL="${4:-10}"
HB_SEC="${LIVENESS_HB_SEC:-60}"
[ -n "${LIVENESS_PIDFILE:-}" ] && echo "$$" > "$LIVENESS_PIDFILE"
ts(){ date -u +%FT%TZ; }

# /metrics 에서 running gauge 와 (prompt+gen) 토큰 합산 카운터를 뽑는다. 무응답이면 running=-1(sentinel).
probe(){
  local m
  m=$(curl -s -m 5 "http://localhost:$PORT/metrics" 2>/dev/null)
  if [ -z "$m" ]; then echo "-1 -1"; return; fi
  # Prometheus: `vllm:num_requests_running{...} N` · `vllm:(prompt|generation)_tokens_total{...} N`
  awk '
    /^vllm:num_requests_running/ { r += $2 }
    /^vllm:prompt_tokens_total/ || /^vllm:generation_tokens_total/ { p += $2 }
    END { printf "%d %d\n", (r==""?0:r), (p==""?0:p) }
  ' <<< "$m"
}

echo "[liveness] start ctr=$CTR port=$PORT stall=${STALL_SEC}s interval=${INTERVAL}s pid=$$ $(ts)"
established=0
last_progress=-1
last_move=$(date +%s)
last_hb=0
while true; do
  now=$(date +%s)
  # 컨테이너가 안 돌면 대기(서빙 전/후) — 정지는 외부 kill.
  if ! docker ps --filter "name=$CTR" --filter status=running -q | grep -q .; then
    established=0; last_progress=-1; last_move=$now; sleep "$INTERVAL"; continue
  fi
  read -r running progress < <(probe)
  if [ "$running" = "-1" ]; then
    # /metrics 무응답. established 전이면 기동 중 — 관대. established 후면 hang 후보(progress 정체와 동일 취급).
    reachable=0
  else
    reachable=1; established=1
    if [ "$progress" -ne "$last_progress" ]; then last_move=$now; last_progress=$progress; fi
  fi
  stalled=$(( now - last_move ))
  if [ "$HB_SEC" -gt 0 ] && [ $(( now - last_hb )) -ge "$HB_SEC" ]; then
    echo "[liveness] HB running=${running} progress=${progress} reachable=${reachable} stalled=${stalled}s established=${established} $(ts)"
    last_hb=$now
  fi
  # 트립 조건: established 이후 · (무응답 OR running>=1) · 정체 >= STALL_SEC
  if [ "$established" = "1" ] && [ "$stalled" -ge "$STALL_SEC" ]; then
    if [ "$reachable" = "0" ] || { [ "$running" -ge 1 ]; }; then
      echo "[liveness] TRIP 엔진교착: running=${running} reachable=${reachable} progress 정체 ${stalled}s(>=${STALL_SEC}) → docker kill $CTR $(ts)"
      docker kill "$CTR" 2>&1 | sed 's/^/[liveness] /'
      # kill 후 established 리셋(재기동 대기). 재기동은 오케스트레이터/compose restart 정책 소관.
      established=0; last_progress=-1; last_move=$(date +%s); sleep 10
    fi
  fi
  sleep "$INTERVAL"
done
