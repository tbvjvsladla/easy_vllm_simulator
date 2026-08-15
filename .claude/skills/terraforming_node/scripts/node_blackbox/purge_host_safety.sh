#!/bin/bash
# purge_host_safety.sh — 레거시 호스트 안전체계 전면 제거자 (plan_26073109 §Phase 1).
#
#   목적: 노드블랙박스 승격에 앞서 기존 설치물을 **잔재 없이** 걷어낸다. 재설치는 별도
#   (install_node_blackbox.sh). 제거와 설치를 한 스크립트에 섞지 않는다 — 부분 적용 상태를
#   만들지 않기 위해서다.
#
#   ★ 선행 게이트: Phase 0(저널 수확)이 끝나 있어야 한다. mem_watchdog 저널은 포락선의 유일한
#     초기 데이터이고 유닛 제거 후 vacuum 되면 복구 불가다. --require-seed 로 강제한다.
#
#   제거 대상(설치자 install_host_safety.sh / install_netconsole.sh 가 만든 것 전부):
#     ① memwatch systemd 유닛 + /usr/local/sbin 바이너리
#     ② vllm-drop-caches 헬퍼 + sudoers.d 엔트리
#     ③ earlyoom 설정(+ --purge-packages 시 패키지)
#     ④ kdump: 우리 GRUB drop-in + sysctl trigger (+ --purge-packages 시 kdump-tools)
#     ⑤ netconsole: 모듈 언로드 + modprobe/modules-load/rsyslog/sysctl + ufw 룰
#     ⑥ 좀비 협역 워치독 프로세스 (PID 기반 — pkill -f 금지, 자기참조 사망 선례 devlog_26062718)
#     ⑦ 임시 telemetry 잔재
#
#   실행 주체 = 사람(HITL sudo). 기본 = dry-run.
#     sudo bash purge_host_safety.sh --apply [--purge-packages] [--seed-dir <path>]
#
#   ⚠ GRUB: 우리 drop-in 제거 후 update-grub 을 돌린다. **재부팅 전** 아래를 반드시 확인:
#        sudo grep -c 'crashkernel=\|reserve_mem=\|ramoops\.' /boot/grub/grub.cfg   # 0 이 정상
#      ★ sudo 없이 돌리지 말 것 — grub.cfg 는 root 전용이라 Permission denied 가 나는데,
#        그걸 "무출력 = 제거됨" 으로 오독한 전례가 있다(2026-07-31). kptr_restrict 위음성과
#        같은 계열의 함정이다: **읽지 못한 것과 없는 것은 다르다.**
#      백업은 /boot/grub/grub.cfg.easy-vllm-purge-backup 에 남긴다.
#
# 종료코드: 0=성공(또는 dry-run) · 1=전제 실패 · 2=제거/검증 실패.
set -uo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# 리포 루트는 고정 상대깊이로 세지 않는다(서브 배달 깊이가 다르다 — install/verify 선례와 동일).
_find_repo(){ local d="$1"; while [ "$d" != "/" ] && [ -n "$d" ]; do
    [ -d "$d/.claude" ] && [ -d "$d/docs" ] && { printf '%s' "$d"; return 0; }; d="$(dirname "$d")"; done; return 1; }
REPO="$(_find_repo "$SDIR" || (cd "$SDIR/../../../../.." 2>/dev/null && pwd))"
# node_id 해소는 단일 소유다(plan_26081514 §4.2 · SKILL.md §2.7.6). 각자 파싱 금지.
[ -f "$SDIR/node_identity.sh" ] || {
  echo "[purge] FAIL: $SDIR/node_identity.sh 부재 — node_id 해소기가 배달되지 않았다." >&2; exit 1; }
# shellcheck source=node_identity.sh
. "$SDIR/node_identity.sh"

APPLY=0; PURGE_PKGS=0; SEED_DIR=""; REQUIRE_SEED=1; NODE_ID=""
for a in "$@"; do
  case "$a" in
    --apply) APPLY=1 ;;
    --purge-packages) PURGE_PKGS=1 ;;
    --seed-dir=*) SEED_DIR="${a#--seed-dir=}" ;;
    --node-id=*) NODE_ID="${a#--node-id=}" ;;
    --no-require-seed) REQUIRE_SEED=0 ;;
    -h|--help)
      sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "[purge] 알 수 없는 인자: $a" >&2; exit 1 ;;
  esac
done

say(){ echo "[purge] $*"; }
run(){ if [ "$APPLY" = "1" ]; then "$@"; else echo "        (dry-run) $*"; fi; }
FAIL=0

# ── 0. 전제: Phase 0 시드 존재 ────────────────────────────────────────────
# ★ NODE_ID 기본값 없음(스킴 R) — 옛 `$(hostname)` 은 시드 탐색을 조용히 빗나가게 했다.
#   여기서 빗나가면 "시드 부재"로 오판해 **저널 수확 전 제거**를 막는 게이트가 헛돈다.
NODE_ID="$(ni_resolve_node_id "$REPO" "$NODE_ID")" || exit 1
if [ -z "$SEED_DIR" ]; then
  for cand in "/home/${SUDO_USER:-$(id -un)}/ws_docker/easy_vllm_simulator/docs/logs/${NODE_ID}/seed" \
              "$REPO/docs/logs/${NODE_ID}/seed"; do
    [ -d "$cand" ] && { SEED_DIR="$cand"; break; }
  done
fi
if [ "$REQUIRE_SEED" = "1" ]; then
  if [ -n "$SEED_DIR" ] && [ -s "$SEED_DIR/memwatch-journal.txt" ]; then
    say "✓ Phase 0 시드 확인: $SEED_DIR ($(wc -l < "$SEED_DIR/memwatch-journal.txt") 줄)"
  else
    say "FAIL: Phase 0 시드 부재 — 저널 수확 전 제거 금지(포락선 초기 데이터 영구 소실)."
    say "      수확: journalctl -u easy-vllm-memwatch --no-pager -o short-iso | seed_from_journal.py ..."
    say "      시드가 다른 노드에 보관됐다면 --seed-dir=<path> 또는 --no-require-seed 로 명시 우회."
    exit 1
  fi
fi

if [ "$APPLY" = "1" ] && [ "$(id -u)" -ne 0 ]; then
  say "FAIL: --apply 는 root 필요 — sudo bash $0 --apply"; exit 1
fi
say "노드: $NODE_ID · 모드: $([ "$APPLY" = 1 ] && echo APPLY || echo DRY-RUN) · 패키지제거: $([ "$PURGE_PKGS" = 1 ] && echo YES || echo '아니오(설정만)')"

# ── ⑥ 좀비 협역 워치독 (먼저 — 제거 중 오작동 방지) ──────────────────────
say "⑥ 협역 mem_watchdog 프로세스 정리 (PID 기반 · pkill -f 금지)"
mapfile -t WD_PIDS < <(ps -eo pid,args | awk '/[m]em_watchdog\.sh/ {print $1}')
if [ "${#WD_PIDS[@]}" -gt 0 ]; then
  for p in "${WD_PIDS[@]}"; do
    say "   kill $p — $(ps -o args= -p "$p" 2>/dev/null | head -c 120)"
    run kill "$p"
  done
else
  say "   (협역 인스턴스 없음)"
fi

# ── ① memwatch 상시 유닛 ─────────────────────────────────────────────────
say "① easy-vllm-memwatch systemd 유닛 + 바이너리"
if systemctl list-unit-files easy-vllm-memwatch.service >/dev/null 2>&1; then
  run systemctl disable --now easy-vllm-memwatch.service
fi
run rm -f /etc/systemd/system/easy-vllm-memwatch.service /usr/local/sbin/easy-vllm-memwatch
run systemctl daemon-reload
run systemctl reset-failed

# ── ② drop-caches 헬퍼 + sudoers ─────────────────────────────────────────
say "② vllm-drop-caches 헬퍼 + sudoers.d 엔트리"
run rm -f /usr/local/sbin/vllm-drop-caches /etc/sudoers.d/easy-vllm-host-safety

# ── ③ earlyoom ───────────────────────────────────────────────────────────
say "③ earlyoom"
if systemctl list-unit-files earlyoom.service >/dev/null 2>&1; then
  run systemctl disable --now earlyoom
fi
run rm -f /etc/default/earlyoom
if [ "$PURGE_PKGS" = "1" ]; then
  run env DEBIAN_FRONTEND=noninteractive apt-get purge -y earlyoom
fi

# ── ④ kdump ──────────────────────────────────────────────────────────────
say "④ kdump: 우리 GRUB drop-in + sysctl trigger"
if [ -f /boot/grub/grub.cfg ]; then
  run cp -a /boot/grub/grub.cfg /boot/grub/grub.cfg.easy-vllm-purge-backup
  say "   백업: /boot/grub/grub.cfg.easy-vllm-purge-backup"
fi
# zz-easy-vllm-blackbox.cfg 는 **신** 설치자(install_node_blackbox.sh)가 쓰는 파일이다.
#   빠뜨리면 purge→재설치 사이클에서 구 crashkernel/ramoops 파라미터가 그대로 살아남아
#   "깨끗이 지웠다"가 거짓이 된다. 레거시 두 이름과 함께 반드시 제거한다.
run rm -f /etc/default/grub.d/zz-easy-vllm-kdump.cfg \
          /etc/default/grub.d/99-easy-vllm-kdump.cfg \
          /etc/default/grub.d/zz-easy-vllm-blackbox.cfg \
          /etc/sysctl.d/99-easy-vllm-kdump-trigger.conf \
          /etc/sysctl.d/99-easy-vllm-panic-promote.conf \
          /etc/modules-load.d/easy-vllm-ramoops.conf
if systemctl list-unit-files kdump-tools.service >/dev/null 2>&1; then
  run systemctl disable --now kdump-tools
fi
if [ "$PURGE_PKGS" = "1" ]; then
  run env DEBIAN_FRONTEND=noninteractive apt-get purge -y kdump-tools
fi
# hang→panic sysctl 은 파일 제거만으로는 현재 커널에 남아 있다 — 런타임도 되돌린다.
for k in kernel.hung_task_panic kernel.softlockup_panic; do
  run sysctl -w "$k=0"
done
run sysctl -w kernel.hung_task_timeout_secs=120

# ── ⑤ netconsole ─────────────────────────────────────────────────────────
say "⑤ netconsole 모듈 + rsyslog 수신 + printk + ufw"
if grep -q '^netconsole ' /proc/modules 2>/dev/null; then
  run modprobe -r netconsole
fi
run rm -f /etc/modules-load.d/netconsole.conf \
          /etc/modprobe.d/netconsole.conf \
          /etc/rsyslog.d/49-netconsole-remote.conf \
          /etc/sysctl.d/90-netconsole-printk.conf
if systemctl is-active --quiet rsyslog 2>/dev/null; then
  run systemctl restart rsyslog
fi
# "Status: active" 는 ufw 출력의 **첫 줄**이고 뒤로 규칙 목록이 계속 나온다 = grep -q 조기종료로
# 상류가 SIGPIPE 를 받는 전형적 형태. pipefail 하에서 위음성이 나므로 파이프를 없앤다.
_ufw="$(ufw status 2>/dev/null)"
if command -v ufw >/dev/null 2>&1 && grep -q "Status: active" <<< "$_ufw"; then
  # 주석 'netconsole peer' 로 단 룰만 선별 삭제(번호는 삭제할 때마다 밀리므로 역순)
  mapfile -t UFW_NUMS < <(ufw status numbered 2>/dev/null | awk -F'[][]' '/netconsole peer/{print $2}' | sort -rn)
  for n in "${UFW_NUMS[@]:-}"; do [ -n "$n" ] && run bash -c "yes | ufw delete $n"; done
fi

# ── ⑦ 임시 telemetry 잔재 ────────────────────────────────────────────────
say "⑦ 임시 telemetry 잔재 (hunt_telemetry 산출물)"
TU="${SUDO_USER:-$(id -un)}"; TH="$(getent passwd "$TU" | cut -d: -f6)"
if [ -n "$TH" ]; then
  for f in "$TH"/hunt_*.jsonl "$TH"/hunt_*.jsonl.mirror; do
    [ -e "$f" ] && { say "   rm $f"; run rm -f "$f"; }
  done
fi

# ── GRUB 재생성 ──────────────────────────────────────────────────────────
say "GRUB 재생성 (crashkernel 제거 반영 — **재부팅 전 확인 필수**)"
if command -v update-grub >/dev/null 2>&1; then run update-grub
elif command -v update-grub2 >/dev/null 2>&1; then run update-grub2
else run grub-mkconfig -o /boot/grub/grub.cfg; fi

# ── 검증 (superpowers 게이트 함수: 주장이 아니라 신선한 출력으로) ─────────
echo
say "═══ 제거 검증 ═══"
verify_absent(){ # $1=설명 $2=경로
  if [ -e "$2" ]; then say "  ✗ 잔존: $2 ($1)"; FAIL=1; else say "  ✓ 부재: $2"; fi
}
if [ "$APPLY" = "1" ]; then
  verify_absent "memwatch 유닛"   /etc/systemd/system/easy-vllm-memwatch.service
  verify_absent "memwatch 바이너리" /usr/local/sbin/easy-vllm-memwatch
  verify_absent "drop-caches 헬퍼" /usr/local/sbin/vllm-drop-caches
  verify_absent "sudoers 엔트리"   /etc/sudoers.d/easy-vllm-host-safety
  verify_absent "earlyoom 설정"    /etc/default/earlyoom
  verify_absent "kdump GRUB drop-in" /etc/default/grub.d/zz-easy-vllm-kdump.cfg
  verify_absent "kdump sysctl"     /etc/sysctl.d/99-easy-vllm-kdump-trigger.conf
  verify_absent "netconsole modprobe" /etc/modprobe.d/netconsole.conf
  verify_absent "netconsole modules-load" /etc/modules-load.d/netconsole.conf
  verify_absent "netconsole rsyslog"  /etc/rsyslog.d/49-netconsole-remote.conf
  verify_absent "netconsole printk"   /etc/sysctl.d/90-netconsole-printk.conf

  if grep -q '^netconsole ' /proc/modules 2>/dev/null; then
    say "  ✗ netconsole 모듈 여전히 로드됨"; FAIL=1
  else say "  ✓ netconsole 모듈 언로드"; fi

  if systemctl is-active --quiet easy-vllm-memwatch 2>/dev/null; then
    say "  ✗ easy-vllm-memwatch 여전히 active"; FAIL=1
  else say "  ✓ easy-vllm-memwatch 비활성"; fi

  if pgrep -f '[m]em_watchdog\.sh' >/dev/null 2>&1; then
    say "  ✗ 협역 워치독 프로세스 잔존: $(pgrep -f '[m]em_watchdog\.sh' | tr '\n' ' ')"; FAIL=1
  else say "  ✓ 협역 워치독 프로세스 0"; fi

  CK_LEFT="$(grep -o 'crashkernel=[^ ]*' /boot/grub/grub.cfg 2>/dev/null | sort -u | tr '\n' ' ')"
  if [ -n "$CK_LEFT" ]; then
    say "  ⚠ grub.cfg 에 crashkernel 잔존: $CK_LEFT"
    say "    (kdump-tools 패키지를 남긴 경우 그 drop-in 이 원인일 수 있음 — 재설치가 덮어쓰므로 무해)"
  else
    say "  ✓ grub.cfg crashkernel 제거됨"
  fi

  echo
  say "═══ 요약 ═══"
  # ★ sudo 필수 — 비-root 로는 Permission denied 가 나고, 그 빈 출력을 '제거됨' 으로 오독하기 쉽다.
  say "재부팅 **전** 필수 확인:  sudo grep -o 'crashkernel=[^ ]*' /boot/grub/grub.cfg   # sudo 없이 금지"
  say "백업:                    /boot/grub/grub.cfg.easy-vllm-purge-backup"
  say "현재 커널의 예약(재부팅 전까지는 살아있음): $(cat /sys/kernel/kexec_crash_size 2>/dev/null) bytes"
  if [ "$FAIL" = "0" ]; then say "PURGE PASS — 잔재 0"; else say "PURGE FAIL — 위 ✗ 항목 확인"; fi
  exit "$FAIL"
else
  echo
  say "DRY-RUN 종료. 실제 제거: sudo bash $0 --apply [--purge-packages]"
  exit 0
fi
