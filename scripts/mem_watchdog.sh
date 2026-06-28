#!/bin/bash
# mem_watchdog.sh — GB10 통합메모리 OOM 호스트-하드다운 방지 안전망(ops backstop).
#   GB10 통합메모리는 GPU OOM 시 호스트 전체가 하드다운된다(GPU/호스트 메모리 풀 공유 →
#   ping-OK→No-route→down, 재부팅 필요). 이 워치독은 /proc/meminfo MemAvailable 을 폴링해
#   임계 밑으로 가면 대상 컨테이너를 docker kill 한다 → 호스트 대신 컨테이너를 희생.
#   (Ray master 는 그 워커 actor-unavailable 로 깨끗이 종료 → 호스트 보존.)
# 사용: mem_watchdog.sh <name_filter> [threshold_mib] [interval_sec]
#   name_filter : docker ps --filter name=<filter> 부분일치(예: slave / master)
# 근거: DeepSeek-V4-Flash serve#1 서브 OOM 하드다운(2026-06-28) · Qwen3.5-397B 선례(워치독 6× 보호).
set -u
FILTER="${1:?usage: mem_watchdog.sh <name_filter> [threshold_mib] [interval_sec]}"
THRESH_MIB="${2:-10240}"   # MemAvailable < 10 GiB → trip (healthy 보수레시피 바닥 ~20-30GiB → false-trip 없음)
INTERVAL="${3:-1}"
ts(){ date -u +%FT%TZ; }
echo "[mem-watchdog] start filter='$FILTER' threshold=${THRESH_MIB}MiB interval=${INTERVAL}s pid=$$ $(ts)"
while true; do
  avail_mib=$(( $(awk '/MemAvailable:/{print $2}' /proc/meminfo) / 1024 ))
  if [ "$avail_mib" -lt "$THRESH_MIB" ]; then
    ids=$(docker ps --filter "name=$FILTER" --filter status=running -q)
    if [ -n "$ids" ]; then
      echo "[mem-watchdog] TRIP MemAvailable=${avail_mib}MiB < ${THRESH_MIB}MiB → docker kill $ids $(ts)"
      docker kill $ids 2>&1 | sed 's/^/[mem-watchdog] /'
    fi
  fi
  sleep "$INTERVAL"
done
