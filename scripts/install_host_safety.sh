#!/bin/bash
# install_host_safety.sh — 호스트 안전체계 결정론 설치자 (plan_2026071019_1 §2.2·§2.5·§3·§4.1).
#   설치물: ① mem_watchdog 상시 systemd 유닛(광역 @vllm) ② vllm-drop-caches 헬퍼 + sudoers 단일
#   NOPASSWD 엔트리 ③ earlyoom(최후선 — 워치독의 워치독) ④ (--with-kdump) kdump-tools.
#   실행 주체 = 사람(HITL sudo — terraforming "호스트 안전체계" 스텝): sudo bash scripts/install_host_safety.sh --apply
#   기본 = dry-run(무엇을 설치할지 표시만). 멱등 — 재실행 안전. 서브노드에도 동일 실행(렌더 배달분).
# 종료코드: 0=성공(또는 dry-run) · 1=전제 실패 · 2=설치/검증 실패.
set -uo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=0; WITH_KDUMP=0; EARLYOOM_DEB=""
TARGET_USER="${SUDO_USER:-$(id -un)}"
for a in "$@"; do
  case "$a" in
    --apply) APPLY=1 ;;
    --with-kdump) WITH_KDUMP=1 ;;
    --user=*) TARGET_USER="${a#--user=}" ;;
    --earlyoom-deb=*) EARLYOOM_DEB="${a#--earlyoom-deb=}" ;;
    *) echo "[host-safety] 알 수 없는 인자: $a (사용: --apply [--with-kdump] [--earlyoom-deb=<path>] [--user=<name>])"; exit 1 ;;
  esac
done
# 오프라인 earlyoom: --earlyoom-deb 미지정 시 레포 루트의 earlyoom_*.deb 자동탐지(offline apt 대비).
if [ -z "$EARLYOOM_DEB" ]; then
  for d in "$SDIR/.."/earlyoom_*.deb; do [ -f "$d" ] && { EARLYOOM_DEB="$d"; break; }; done
fi

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
say "③ earlyoom: $([ -n "$EARLYOOM_DEB" ] && echo "로컬 .deb($EARLYOOM_DEB)" || echo "apt") 설치 + '-m 4(≈4.9GiB<워치독10GiB — 워치독 선발화) --prefer vLLM 계열 --avoid 핵심데몬'"
if [ "$APPLY" = "1" ]; then
  if ! command -v earlyoom >/dev/null 2>&1; then
    if [ -n "$EARLYOOM_DEB" ] && [ -f "$EARLYOOM_DEB" ]; then
      # 오프라인 우선: 사전 다운로드 .deb(예 GB10 airgap). dpkg 실패 시 apt 폴백.
      dpkg -i "$EARLYOOM_DEB" >/dev/null 2>&1 || apt-get install -y earlyoom >/dev/null 2>&1 \
        || { say "  ✗ earlyoom 설치 실패(.deb+apt 모두)"; FAIL=1; }
    else
      apt-get install -y earlyoom >/dev/null 2>&1 || { say "  ✗ earlyoom apt 설치 실패(오프라인이면 --earlyoom-deb=<path> 지정)"; FAIL=1; }
    fi
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
#   Ubuntu 24.04 kdump-tools 는 noninteractive 설치 시 USE_KDUMP=0 + crashkernel=1G-:0M
#   (플레이스홀더 = 0M 예약)로 남아 kdump 가 절대 ready 안 된다 → 명시 활성화·크기지정 필수.
#   aarch64(GB10)는 low-mem 예약이 "not ready" 를 자주 유발 → `,high`(고메모리 우선 예약)가 권장
#   (참조: docs.kernel.org/arch/arm64/kdump.html — high 성공 시 low 128M 자동). 커널은 cmdline 의
#   **마지막** crashkernel= 를 채택하므로 dual-param(99-정렬 함정)을 피해 **단일 값**으로 교정한다.
#   [P4 · testlog_2026071111_1 §0]: 512M→2G,high — 124GiB 호스트서 512M 는 crash-kernel makedumpfile OOM
#     으로 vmcore 저장 실패(2026-07-11 실증 vmcore 0). kdump-config 자체 권고 1660M · NVIDIA Tegra r36=2G.
KDUMP_CRASHKERNEL="${KDUMP_CRASHKERNEL:-2G,high}"   # env 로 조정 가능. 2G = 128GiB 호스트 vmcore 저장 여유
if [ "$WITH_KDUMP" = "1" ]; then
  say "④ kdump-tools 설치 + 활성화(USE_KDUMP=1) + crashkernel=${KDUMP_CRASHKERNEL} 예약 (**재부팅 1회 필요**)"
  if [ "$APPLY" = "1" ]; then
    DEBIAN_FRONTEND=noninteractive apt-get install -y kdump-tools >/dev/null 2>&1 || { say "  ✗ kdump-tools 설치 실패"; FAIL=1; }
    if command -v kdump-config >/dev/null 2>&1; then
      # (a) USE_KDUMP=1 (noninteractive 기본 0 교정)
      if grep -qE '^USE_KDUMP=' /etc/default/kdump-tools 2>/dev/null; then
        sed -i 's/^USE_KDUMP=.*/USE_KDUMP=1/' /etc/default/kdump-tools
      else
        echo 'USE_KDUMP=1' >> /etc/default/kdump-tools
      fi
      # (a2) KDUMP_SKIP_VMCORE=0 (P4 · testlog_2026071111_1 §0 — BSP 가 =1 이면 vmcore 저장 스킵 = 2026-07-11
      #      vmcore 0 근본원인. 존재하는 =1 만 뒤집음; 부재 시 append 금지 — 비표준 노브 신설 위험).
      if grep -qE '^KDUMP_SKIP_VMCORE=' /etc/default/kdump-tools 2>/dev/null; then
        sed -i 's/^KDUMP_SKIP_VMCORE=.*/KDUMP_SKIP_VMCORE=0/' /etc/default/kdump-tools
      fi
      # (b) crashkernel 을 **단일 파라미터**로 교정. 옛 잘못정렬 오버라이드(99-/zz-) 제거 후,
      #     kdump-tools.cfg 의 플레이스홀더 값 자체를 sed 로 실값으로 치환(dual-param 마지막-승 함정 회피).
      rm -f /etc/default/grub.d/99-easy-vllm-kdump.cfg /etc/default/grub.d/zz-easy-vllm-kdump.cfg
      KT=/etc/default/grub.d/kdump-tools.cfg
      if [ -f "$KT" ] && grep -q 'crashkernel=' "$KT"; then
        sed -i "s|crashkernel=[^ \"]*|crashkernel=${KDUMP_CRASHKERNEL}|g" "$KT"
      else
        # kdump-tools.cfg 에 crashkernel 부재 → 마지막 정렬(zz-, 'z'>'k')로 예약(승리 보장)
        cat > /etc/default/grub.d/zz-easy-vllm-kdump.cfg <<EOF
GRUB_CMDLINE_LINUX_DEFAULT="\$GRUB_CMDLINE_LINUX_DEFAULT crashkernel=${KDUMP_CRASHKERNEL}"
EOF
      fi
      if command -v update-grub >/dev/null 2>&1; then update-grub >/dev/null 2>&1
      elif command -v update-grub2 >/dev/null 2>&1; then update-grub2 >/dev/null 2>&1
      else grub-mkconfig -o /boot/grub/grub.cfg >/dev/null 2>&1; fi
      systemctl enable kdump-tools >/dev/null 2>&1
      # (c) hang→panic 승격 (P4 — 22분 무음 non-panic hang 은 crash_kexec 미발동 → vmcore 0. hung_task·
      #     softlockup 을 panic 으로 올려 kexec 경로 진입. panic=10 = 패닉 후 10s 자동재부팅).
      cat > /etc/sysctl.d/99-easy-vllm-kdump-trigger.conf <<'EOF'
kernel.hung_task_panic=1
kernel.hung_task_timeout_secs=60
kernel.softlockup_panic=1
kernel.panic=10
EOF
      sysctl -p /etc/sysctl.d/99-easy-vllm-kdump-trigger.conf >/dev/null 2>&1
      say "  ✓ USE_KDUMP=1 · KDUMP_SKIP_VMCORE=0 · crashkernel=${KDUMP_CRASHKERNEL} · hang→panic sysctl · update-grub — **재부팅 후** ready"
      say "  ⚠ 재부팅 후 검증: kdump-config show|grep 'current state'→'ready' + grep -i crash /proc/iomem(~2G 예약) + (scratch)echo c>/proc/sysrq-trigger 로 vmcore 실착지"
    fi
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
