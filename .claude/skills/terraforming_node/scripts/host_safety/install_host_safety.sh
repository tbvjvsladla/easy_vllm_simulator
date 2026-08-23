#!/bin/bash
# install_host_safety.sh — 호스트 안전체계 결정론 설치자 (plan_26071019 §2.2·§2.5·§3·§4.1).
#   설치물: ① mem_watchdog 상시 systemd 유닛(광역 @vllm) ② vllm-drop-caches 헬퍼 + sudoers 단일
#   NOPASSWD 엔트리 ③ earlyoom(최후선 — 워치독의 워치독).
#   ④ kdump 는 **제거됐다**(2026-07-31, testlog_26073114): 무장 시 panic() 이 kmsg_dump 보다 먼저
#      kexec 로 점프해 efi_pstore 를 원천 차단하고, 그 대가로 2.25 GiB 를 상시 예약한다.
#      사후 포착 정본 = node_blackbox --level=L3 (efi_pstore) + 멀티는 netconsole.
#   ⑤ **SBSA 하드웨어 워치독 무장**(plan_26082319 §5.1 · 2026-08-23 신설) — 하드 락업 자동 리셋.
#      ①~④ 는 전부 **userland** 다. 하드 락업(커널 완전 정지)에서는 감시자 자신이 함께 얼어붙으므로
#      원리적으로 무력하다. 하드웨어 타이머만이 그 상태에서 살아 있다.
#   실행 주체 = 사람(HITL sudo — terraforming "호스트 안전체계" 스텝): sudo bash .claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh --apply
#   기본 = dry-run(무엇을 설치할지 표시만). 멱등 — 재실행 안전. 서브노드에도 동일 실행(렌더 배달분).
# 종료코드: 0=성공(또는 dry-run) · 1=전제 실패 · 2=설치/검증 실패.
set -uo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=0; EARLYOOM_DEB=""
TARGET_USER="${SUDO_USER:-$(id -un)}"
# ★ 하드웨어 워치독 타임아웃(초). 기본 60 의 근거는 아래 ⑤ 블록 주석에 있다 — **한 곳에서만** 설명한다.
WATCHDOG_SEC=60
WITH_WATCHDOG=1
# 이보다 짧은 설정은 거부한다. 이 호스트는 100 GiB 급 모델 로드를 상시 수행하고, 그 구간에서
# PID1 이 수 초 스톨하는 것은 정상이다. 짧은 타임아웃은 **정상 부하를 재부팅으로 바꾼다**.
WATCHDOG_MIN_SEC=15
for a in "$@"; do
  case "$a" in
    --apply) APPLY=1 ;;
    --with-kdump)
      echo "[host-safety] 거부: --with-kdump 는 폐지됐습니다(2026-07-31)." >&2
      echo "  근거: kdump 무장 시 panic() 이 kmsg_dump 보다 먼저 kexec 로 점프해(crash_kexec_post_notifiers=N)" >&2
      echo "        efi_pstore 가 원리적으로 기록되지 못합니다. 게다가 vmcore 는 makedumpfile 이 커널 6.17 을" >&2
      echo "        미지원해 구조적으로 불가하고(0/4), 크래시커널 부팅은 1/4 간헐이며, 예약 2.25 GiB 는" >&2
      echo "        통합메모리 하드다운의 원인 자원 그 자체입니다. 증거: docs/testlog/testlog_26073114*." >&2
      echo "  대체: sudo bash .claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh --apply --level=L3" >&2
      exit 1 ;;
    --user=*) TARGET_USER="${a#--user=}" ;;
    --earlyoom-deb=*) EARLYOOM_DEB="${a#--earlyoom-deb=}" ;;
    --watchdog-sec=*) WATCHDOG_SEC="${a#--watchdog-sec=}" ;;
    --no-watchdog) WITH_WATCHDOG=0 ;;
    *) echo "[host-safety] 알 수 없는 인자: $a (사용: --apply [--earlyoom-deb=<path>] [--user=<name>] [--watchdog-sec=<n>] [--no-watchdog])"; exit 1 ;;
  esac
done
# 오프라인 earlyoom: --earlyoom-deb 미지정 시 레포 루트의 earlyoom_*.deb 자동탐지(offline apt 대비).
if [ -z "$EARLYOOM_DEB" ]; then
  for d in "$SDIR/.."/earlyoom_*.deb; do [ -f "$d" ] && { EARLYOOM_DEB="$d"; break; }; done
fi

say(){ echo "[host-safety] $*"; }
FAIL=0

if [ "$APPLY" = "1" ] && [ "$(id -u)" -ne 0 ]; then
  say "FAIL: --apply 는 root 필요 — sudo bash .claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh --apply"; exit 1
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
# easy-vllm host-safety (plan_26071019 §2.5) — 워치독(10GiB)보다 낮은 최후선(≈4%).
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
#   aarch64(GB10)는 low-mem 예약이 "not ready" 를 자주 유발 → `,high`(고메모리 우선) 권장. ★커널 6.17
#   업데이트 실증(2026-07-15 Phase0 · plan_26071512): `,high` **단독이 예약 실패**(addr 0x·/proc/iomem
#   0-0, 양노드 — GRUB/cmdline 엔 2G,high 있으나 커널 미예약) → 참조-그라운디드(docs.kernel.org/arch/arm64/kdump
#   · 6.17 CMA 변경, Phoronix): **high + 명시 `,low` 병기 필수**(자동 low 128M 이 6.17서 실패). ∴ 옛 "단일 값"
#   교정을 high+low 병기로 대체(SUPERSEDES 단일-값 접근 for 커널 6.17+).
#   [P4 · testlog_26071111 §0]: 512M→2G,high — 124GiB 호스트서 512M 는 crash-kernel makedumpfile OOM
#     으로 vmcore 저장 실패(2026-07-11 실증 vmcore 0). kdump-config 자체 권고 1660M · NVIDIA Tegra r36=2G.
# ④ kdump — **폐지**. 이 설치자는 더 이상 crashkernel 을 예약하지 않는다.
#   기존 설치분의 제거와 efi_pstore 확보는 node_blackbox 설치자의 L3 가 소유한다.
say "④ kdump: 폐지됨(2026-07-31) — 사후 포착은 node_blackbox --level=L3 의 efi_pstore 가 담당"
say "   기존 kdump 가 남아 있다면 그것이 efi_pstore 를 막고 있다: install_node_blackbox.sh --apply --level=L3"

# ── ⑤ SBSA 하드웨어 워치독 무장 (하드 락업 자동 리셋) ───────────────────
#   근거 plan_26082319 §5.1 · 사건 2026-08-23 R6.
#   ①~④ 는 전부 userland 다. 2026-08-23 R6 에서 호스트가 **하드 락업**(커널 완전 정지)에 빠졌을 때
#   softlockup_panic·hung_task_panic 은 무장돼 있었는데도 **둘 다 미발동**했다 — 인터럽트까지
#   멈췄다는 뜻이다. 하드 락업을 잡는 유일한 커널 감지기(NMI 워치독)는 이 플랫폼에서
#   `watchdog: Hard watchdog permanently disabled`(부트로그 실측)로 **영구 비활성**이고,
#   대안인 "buddy" 감지기는 **전 CPU 동시 정지를 못 잡는다**(감시 CPU 도 얼음) — 이번 사건이
#   정확히 그 경우다. 결과: 18:28 정지 → 18:41 사용자 수동 재부팅까지 **13 분 무방비**.
#
#   SBSA Generic Watchdog 은 **하드웨어 타이머**라 커널 정지의 영향을 받지 않는다. 커널이 얼면
#   pet(피드)이 멈추고, 타이머가 만료되면 시스템을 리셋한다. 이 호스트는 GTDT 에 이미 장치가
#   있고(`ACPI GTDT: found 1 SBSA generic Watchdog(s)`) `/dev/watchdog` 으로 노출돼 있다 —
#   **무장만 하면 된다**(설치 전 실측: state=inactive · RuntimeWatchdogUSec=0).
#
#   ⚠ 이것은 **복구**이지 예방이 아니다. 예방은 node_blackbox 의 열·전력 포락선 워치독
#     (`thermal_watchdog.sh`)이 담당한다. 리셋이 반복된다면 그것은 워치독의 성공이 아니라
#     **예방 실패 신호**다 — 그때는 열·전력 임계를 재교정하라.
#
#   ⚠ **새 실패 모드를 하나 들여온다(음성정직)**: PID1 이 타임아웃 동안 ping 하지 못하면
#     *멀쩡한* 호스트도 리셋된다. 그래서 기본을 60 초로 잡았다(systemd 는 그 절반인 30 초마다
#     ping 한다). PID1 이 30 초를 놓치려면 시스템이 이미 사용 불가 상태여야 하며, 이 호스트의
#     과거 사고는 전부 그 상태에서 **어차피 하드다운으로 끝났다** — 그 구간의 리셋은 퇴행이
#     아니라 개선이다. 관측 후 조이려면 --watchdog-sec=<n> (하한 ${WATCHDOG_MIN_SEC}초).
#   ⚠ efi_pstore(C7)와의 관계: panic 시 systemd 가 ping 을 멈추므로 워치독이 결국 리셋한다.
#     efi_pstore 기록은 1 초 미만이라 60 초 타임아웃이 사후 포착을 자르지 않는다.
say "⑤ SBSA 하드웨어 워치독: $([ "$WITH_WATCHDOG" = 1 ] && echo "RuntimeWatchdogSec=${WATCHDOG_SEC}s 무장" || echo "건너뜀(--no-watchdog)")"
WD_SYS=/sys/class/watchdog/watchdog0
WD_DROPIN=/etc/systemd/system.conf.d/10-easy-vllm-watchdog.conf
if [ "$WITH_WATCHDOG" = "1" ]; then
  case "$WATCHDOG_SEC" in
    ''|*[!0-9]*) say "  ✗ --watchdog-sec 는 정수여야 한다: '$WATCHDOG_SEC'"; FAIL=1; WITH_WATCHDOG=0 ;;
    *) if [ "$WATCHDOG_SEC" -lt "$WATCHDOG_MIN_SEC" ]; then
         # 조용히 올리지 않는다 — 거부한다(안전값을 몰래 바꾸면 사람이 무엇이 걸렸는지 모른다).
         say "  ✗ --watchdog-sec=$WATCHDOG_SEC 는 하한 ${WATCHDOG_MIN_SEC}초 미만이라 거부한다."
         say "    근거: 100 GiB 급 모델 로드 중 PID1 수 초 스톨은 정상이며, 짧은 타임아웃은"
         say "          정상 부하를 재부팅으로 바꾼다. 필요하면 하한 자체를 근거와 함께 바꿔라."
         FAIL=1; WITH_WATCHDOG=0
       fi ;;
  esac
fi
if [ "$WITH_WATCHDOG" = "1" ] && [ ! -e "$WD_SYS" ]; then
  # 장치 부재는 **실패가 아니라 부재**다(플랫폼에 따라 없을 수 있다). 정직하게 알리고 넘어간다 —
  # 다만 "설치했다"고 말하지는 않는다. 그 노드는 하드 락업 복구 계층이 없는 것이다.
  say "  ⚠ $WD_SYS 부재 — 이 플랫폼엔 하드웨어 워치독이 없다. **하드 락업 복구 계층 없음**."
  say "    (예방 계층 thermal_watchdog 은 여전히 유효하다 — install_node_blackbox.sh --level=L1)"
  WITH_WATCHDOG=0
fi
if [ "$WITH_WATCHDOG" = "1" ]; then
  WD_ID="$(cat "$WD_SYS/identity" 2>/dev/null || echo unknown)"
  WD_STATE="$(cat "$WD_SYS/state" 2>/dev/null || echo unknown)"
  WD_TMO="$(cat "$WD_SYS/timeout" 2>/dev/null || echo unknown)"
  WD_OPT="$(cat "$WD_SYS/options" 2>/dev/null || echo 0)"
  say "  장치: $WD_ID · 현재 state=$WD_STATE · timeout=${WD_TMO}s · options=$WD_OPT"
  # WDIOF_PRETIMEOUT(0x0200) 능력 판정. **있다고 가정하고 쓰면 PID1 설정이 통째로 거부될 수 있다.**
  #   이 호스트 실측 options=0x81a0 → pretimeout 미지원이므로 RuntimeWatchdogPreSec 은 쓰지 않는다.
  WD_PRE=0
  if [ "$WD_OPT" != "0" ] && [ $(( WD_OPT & 0x0200 )) -ne 0 ]; then WD_PRE=1; fi
  if [ "$WD_PRE" = "1" ]; then
    say "  pretimeout 지원됨 → 2 단계(WS0 panic → WS1 reset) 사용: RuntimeWatchdogPreSec=$(( WATCHDOG_SEC / 2 ))s"
  else
    say "  pretimeout 미지원(options 에 WDIOF_PRETIMEOUT 없음) → 1 단계 리셋만. RuntimeWatchdogPreSec 생략."
  fi
  say "  drop-in → $WD_DROPIN (system.conf 원본은 건드리지 않는다 — 패키지 갱신과 충돌하지 않게)"
  if [ "$APPLY" = "1" ]; then
    install -d -m 0755 /etc/systemd/system.conf.d
    {
      printf '# easy-vllm host-safety (plan_26082319 §5.1) — SBSA 하드웨어 워치독.
'
      printf '# 하드 락업(커널 완전 정지)에서 자동 리셋하는 **유일한** 기구다. userland 워치독은
'
      printf '# 정의상 이 클래스를 못 잡는다(감시자도 함께 얼어붙는다).
'
      printf '# 갱신: install_host_safety.sh --apply --watchdog-sec=<n> · 해제: --no-watchdog 후 이 파일 삭제
'
      printf '[Manager]
'
      printf 'RuntimeWatchdogSec=%ss
' "$WATCHDOG_SEC"
      [ "$WD_PRE" = "1" ] && printf 'RuntimeWatchdogPreSec=%ss
' "$(( WATCHDOG_SEC / 2 ))"
    } > "$WD_DROPIN"
    chmod 0644 "$WD_DROPIN"
    # ★ **daemon-reload 로는 반영되지 않는다.** system.conf 는 PID1 자신의 설정이라
    #   `daemon-reexec` 로 PID1 을 재실행해야 읽힌다. 이걸 빠뜨리면 파일은 놓였는데 워치독은
    #   여전히 꺼져 있고, 그 상태가 "설치 완료"로 보고된다(만든 것과 도는 것이 다른 전형).
    systemctl daemon-reexec
    sleep 1
    WD_NOW="$(cat "$WD_SYS/state" 2>/dev/null || echo unknown)"
    WD_USEC="$(systemctl show -p RuntimeWatchdogUSec --value 2>/dev/null || echo unknown)"
    if [ "$WD_NOW" = "active" ]; then
      say "  ✓ 하드웨어 워치독 active (timeout=$(cat "$WD_SYS/timeout" 2>/dev/null)s · systemd RuntimeWatchdogUSec=$WD_USEC)"
    else
      say "  ✗ 무장 실패 — state=$WD_NOW (systemd RuntimeWatchdogUSec=$WD_USEC)"
      say "    확인: cat $WD_DROPIN · systemctl show -p RuntimeWatchdogUSec · journalctl -b -u init.scope"
      FAIL=1
    fi
  fi
fi

if [ "$APPLY" = "1" ]; then
  say "설치 요약: memwatch=$(systemctl is-active easy-vllm-memwatch.service 2>/dev/null) · earlyoom=$(systemctl is-active earlyoom 2>/dev/null) · sudoers=$([ -f /etc/sudoers.d/easy-vllm-host-safety ] && echo ok || echo missing) · hw-watchdog=$(cat /sys/class/watchdog/watchdog0/state 2>/dev/null || echo none)"
  exit "$([ "$FAIL" = "0" ] && echo 0 || echo 2)"
else
  say "DRY-RUN 종료 — 실제 설치: sudo bash .claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh --apply"
fi
