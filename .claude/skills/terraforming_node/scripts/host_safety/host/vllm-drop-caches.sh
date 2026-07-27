#!/bin/bash
# vllm-drop-caches — 페이지캐시 드랍 단일-동작 헬퍼 (plan_26071019 §4.1).
#   GB10 통합메모리는 OS 페이지캐시가 CUDA 와 물리풀을 직접 공유·경쟁(외부 기술노트 그라운딩 —
#   simlog 26070821 run_summary) → 로드-전 게이트/teardown 루틴이 이 헬퍼를 자동 호출한다.
# 설치: install_host_safety.sh 가 /usr/local/sbin/vllm-drop-caches(root 소유 0755)로 복사하고
#   sudoers NOPASSWD **단일 엔트리**로 이 경로만 위임(인자 없음·스코프 고정 — 에이전트가 임의
#   sudo 를 얻는 게 아니라 감사 가능한 단일 동작만 위임).
set -eu
if [ "$(id -u)" -ne 0 ]; then
  echo "[vllm-drop-caches] root 필요: sudo -n /usr/local/sbin/vllm-drop-caches" >&2
  exit 1
fi
sync
echo 3 > /proc/sys/vm/drop_caches
echo "[vllm-drop-caches] done $(date -u +%FT%TZ)"
