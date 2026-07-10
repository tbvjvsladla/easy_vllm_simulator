#!/bin/bash
# install_host_safety.sh — 호스트 안전체계 결정론 설치자 (plan_2026071019_1 §2.2·§2.5·§3·§4.1).
#   설치물: ① mem_watchdog 상시 systemd 유닛(광역 @vllm) ② vllm-drop-caches 헬퍼 + sudoers 단일
#   NOPASSWD 엔트리 ③ earlyoom(최후선 — 워치독의 워치독) ④ (--with-kdump) kdump-tools.
#   실행 주체 = 사람(HITL sudo — terraforming "호스트 안전체계" 스텝): sudo bash scripts/install_host_safety.sh --apply
#   기본 = dry-run(무엇을 설치할지 표시만). 멱등 — 재실행 안전. 서브노드에도 동일 실행(렌더 배달분).
# 종료코드: 0=성공(또는 dry-run) · 1=전제 실패 · 2=설치/검증 실패.
set -uo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=0; WITH_KDUMP=0
TARGET_USER="${SUDO_USER:-$(id -un)}"
for a in "$@"; do
  case "$a" in
    --apply) APPLY=1 ;;
    --with-kdump) WITH_KDUMP=1 ;;
    --user=*) TARGET_USER="${a#--user=}" ;;
    *) echo "[host-safety] 알 수 없는 인자: $a (사용: --apply [--with-kdump] [--user=<name>])"; exit 1 ;;
  esac
done

say(){ echo "[host-safety] $*"; }
FAIL=0

if [ "$APPLY" = "1" ] && [ "$(id -u)" -ne 0 ]; then
  say "FAIL: --apply 는 root 필요 — sudo bash scripts/install_host_safety.sh --apply"; exit 1
fi
[ -f "$SDIR/mem_watchdog.sh" ] || { say "FAIL: $SDIR/mem_watchdog.sh 부재"; exit 1; }
[ -f "$SDIR/systemd/easy-vllm-memwatch.service" ] || { say "FAIL: systemd 유닛 템플릿 부재"; exit 1; }
[ -f "$SDIR/host/vllm-drop-caches.sh" ] || { say "FAIL: drop-caches 헬퍼 원본 부재"; exit 1; }

say "대상 사용자(sudoers 위임): $TARGET_USER · 모드: $([ "$APPLY" = "1" ] && echo APPLY || echo DRY-RUN)"

# ── ① mem_watchdog 상시 유닛 ─────────────────────────────────────────────
say "① mem_watchdog → /usr/local/sbin/easy-vllm-memwatch + systemd enable --now"
if [ "$APPLY" = "1" ]; then
  install -m 0755 "$SDIR/mem_watchdog.sh" /usr/local/sbin/easy-vllm-memwatch
  install -m 0644 "$SDIR/systemd/easy-vllm-memwatch.service" /etc/systemd/system/easy-vllm-memwatch.service
  systemctl daemon-reload
  systemctl enable --now easy-vllm-memwatch.service
  sleep 1
  if systemctl is-active --quiet easy-vllm-memwatch.service; then
    say "  ✓ easy-vllm-memwatch active"
  else
    say "  ✗ easy-vllm-memwatch 비활성 — journalctl -u easy-vllm-memwatch 확인"; FAIL=1
  fi
fi

# ── ② drop-caches 헬퍼 + sudoers 단일 엔트리 ────────────────────────────
say "② vllm-drop-caches → /usr/local/sbin + sudoers.d/easy-vllm-host-safety (${TARGET_USER} 단일 경로 NOPASSWD)"
if [ "$APPLY" = "1" ]; then
  install -m 0755 "$SDIR/host/vllm-drop-caches.sh" /usr/local/sbin/vllm-drop-caches
  SUDOERS_TMP="$(mktemp)"
  printf '%s ALL=(root) NOPASSWD: /usr/local/sbin/vllm-drop-caches\n' "$TARGET_USER" > "$SUDOERS_TMP"
  if visudo -cf "$SUDOERS_TMP" >/dev/null 2>&1; then
    install -m 0440 "$SUDOERS_TMP" /etc/sudoers.d/easy-vllm-host-safety
    say "  ✓ sudoers 엔트리 설치(visudo 검증 통과)"
  else
    say "  ✗ sudoers 후보 visudo 검증 실패 — 미설치"; FAIL=1
  fi
  rm -f "$SUDOERS_TMP"
  if sudo -n -u "$TARGET_USER" sudo -n /usr/local/sbin/vllm-drop-caches >/dev/null 2>&1; then
    say "  ✓ ${TARGET_USER} 무암호 실행 검증"
  else
    # root 컨텍스트에 따라 이 자기검증이 위음성일 수 있음 — 사용자 셸에서 최종 확인 안내
    say "  ⚠ 무암호 자기검증 미통과 — 사용자 셸에서 'sudo -n /usr/local/sbin/vllm-drop-caches' 로 확인 필요"
  fi
fi

# ── ③ earlyoom (최후선) ─────────────────────────────────────────────────
say "③ earlyoom: apt 설치 + '-m 4(≈4.9GiB<워치독10GiB — 워치독 선발화) --prefer vLLM 계열 --avoid 핵심데몬'"
if [ "$APPLY" = "1" ]; then
  if ! command -v earlyoom >/dev/null 2>&1; then
    apt-get install -y earlyoom >/dev/null 2>&1 || { say "  ✗ earlyoom apt 설치 실패"; FAIL=1; }
  fi
  if command -v earlyoom >/dev/null 2>&1; then
    cat > /etc/default/earlyoom <<'EOF'
# easy-vllm host-safety (plan_2026071019_1 §2.5) — 워치독(10GiB)보다 낮은 최후선(≈4%).
# prefer = vLLM 계열 우선 희생 · avoid = 시스템 핵심 보호.
EARLYOOM_ARGS="-m 4 -r 3600 --prefer '(VLLM|EngineCor|ray::|vllm)' --avoid '(systemd|sshd|dockerd|containerd|journald|earlyoom|easy-vllm-memw)'"
EOF
    systemctl enable --now earlyoom >/dev/null 2>&1
    systemctl restart earlyoom >/dev/null 2>&1
    if systemctl is-active --quiet earlyoom; then say "  ✓ earlyoom active"; else say "  ✗ earlyoom 비활성"; FAIL=1; fi
  fi
fi

# ── ④ kdump (선택 — 재부팅 필요 고지) ───────────────────────────────────
if [ "$WITH_KDUMP" = "1" ]; then
  say "④ kdump-tools 설치 (crashkernel 예약 — **재부팅 1회 필요**, RAM 수백 MB 상시 점유)"
  if [ "$APPLY" = "1" ]; then
    DEBIAN_FRONTEND=noninteractive apt-get install -y kdump-tools >/dev/null 2>&1 || { say "  ✗ kdump-tools 설치 실패"; FAIL=1; }
    kdump-config show 2>&1 | sed 's/^/[host-safety]   /'
    say "  ⚠ 'Not ready' 면 재부팅 후 'kdump-config show' 재확인(ready 필요 — plan §8 Phase2 합격기준)"
  fi
else
  say "④ kdump: 건너뜀(--with-kdump 로 활성 — 스트레스 게이트 전 필수, plan §3)"
fi

if [ "$APPLY" = "1" ]; then
  say "설치 요약: memwatch=$(systemctl is-active easy-vllm-memwatch.service 2>/dev/null) · earlyoom=$(systemctl is-active earlyoom 2>/dev/null) · sudoers=$([ -f /etc/sudoers.d/easy-vllm-host-safety ] && echo ok || echo missing)"
  exit "$([ "$FAIL" = "0" ] && echo 0 || echo 2)"
else
  say "DRY-RUN 종료 — 실제 설치: sudo bash scripts/install_host_safety.sh --apply [--with-kdump]"
fi
