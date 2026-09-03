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

APPLY=0; PURGE_PKGS=0; SEED_DIR=""; REQUIRE_SEED=1; NODE_ID=""; PURGE_WATCHDOG=0
for a in "$@"; do
  case "$a" in
    --apply) APPLY=1 ;;
    --purge-packages) PURGE_PKGS=1 ;;
    --purge-watchdog) PURGE_WATCHDOG=1 ;;
    --seed-dir=*) SEED_DIR="${a#--seed-dir=}" ;;
    --node-id=*) NODE_ID="${a#--node-id=}" ;;
    --no-require-seed) REQUIRE_SEED=0 ;;
    -h|--help)
      sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "[purge] 알 수 없는 인자: $a" >&2; exit 1 ;;
  esac
done

say(){ echo "[purge] $*"; }
FAIL=0
# ★ rc 전파 (2026-09-03 · B5). install_node_blackbox.sh 와 동일 결함·동일 처방: run() 은 rc 를
#   반환했지만 `set -e` 가 없고 25개 호출부 중 rc 를 보는 곳이 0개였다. 여기서는 대가가 더 크다 —
#   **제거가 실패해도 "PURGE PASS — 잔재 0" 이 찍히고**, 그 거짓 위에서 재설치가 돌기 때문이다.
#   래퍼 한 자리에서 판정을 기록한다(FAIL 은 대입 · 종료코드 taxonomy 0/1/2 유지).
run(){
  if [ "$APPLY" = "1" ]; then
    "$@"
    local _rc=$?
    if [ "$_rc" -ne 0 ]; then say "   ✗ 실패(rc=$_rc): $*"; FAIL=1; fi
    return "$_rc"
  else
    echo "        (dry-run) $*"
  fi
}

# ── 파괴 단계 사후 검증 헬퍼 (주장이 아니라 결과 상태로 판정한다) ──────────
#   rc 0 은 "명령이 돌았다"이지 "그렇게 됐다"가 아니다. 특히 apt-get/sysctl 은 부분 성공·
#   무시된 요청에도 0 을 낸다. 그래서 파괴 단계마다 결과를 다시 읽는다.
verify_pkg_absent(){   # $1=패키지명
  [ "$APPLY" = "1" ] || return 0
  local _st; _st="$(dpkg-query -W -f='${Status}' "$1" 2>/dev/null)"
  case "$_st" in
    *"install ok installed"*) say "   ✗ 패키지 잔존: $1 (status=$_st)"; FAIL=1 ;;
    *)                        say "   ✓ 패키지 제거 확인: $1" ;;
  esac
}
verify_sysctl(){       # $1=키 $2=기대값
  [ "$APPLY" = "1" ] || return 0
  local _v; _v="$(sysctl -n "$1" 2>/dev/null)"
  if [ -z "$_v" ]; then say "   ✗ $1 을 읽지 못했다 — 되돌림 여부를 **판정할 수 없다**"; FAIL=1
  elif [ "$_v" = "$2" ]; then say "   ✓ $1=$_v"
  else say "   ✗ $1=$_v (기대 $2) — sysctl -w 가 반영되지 않았다"; FAIL=1; fi
}
# 실행 중인 협역 워치독 PID — **언급이 아니라 실행**으로 판정한다(B12).
#   ⚠ `pgrep -f`/`pkill -f` 금지(2026-08-02 위양성 실화: 실카운트 0인데 2 를 보고했고 그 2 는
#     방금 친 진단 명령이었다). 브래킷 트릭 `[m]em…` 은 pgrep 자신의 argv 만 피할 뿐 제3자
#     명령줄은 못 피하고, 전체 줄 부분일치도 마찬가지다. 정본은 verify_node_blackbox.sh 의
#     **필드 앵커**다 — argv[0] 이 bash 이고 argv[1] 이 그 스크립트 자신일 때만 실행이다.
#     (상시 유닛은 /usr/local/sbin/easy-vllm-memwatch 로 설치되므로 여기 걸리지 않는 것이 맞다 —
#      이 자리가 노리는 것은 손으로 띄운 협역 인스턴스뿐이다.)
wd_pids(){ ps -eo pid,args 2>/dev/null \
  | awk '$2 ~ /(^|\/)bash$/ && $3 ~ /(^|\/)mem_watchdog\.sh$/ {print $1}'; }

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
mapfile -t WD_PIDS < <(wd_pids)
if [ "${#WD_PIDS[@]}" -gt 0 ]; then
  for p in "${WD_PIDS[@]}"; do
    say "   kill $p — $(ps -o args= -p "$p" 2>/dev/null | head -c 120)"
    if [ "$APPLY" = "1" ]; then
      # ★ kill 의 rc 는 판정 근거가 못 된다 — 대상이 **이미 죽었으면** 비-0 이 나오는데 그것은
      #   우리가 원하던 결과다. 그래서 여기만 run() 을 쓰지 않고, 판정은 **사후 소멸 확인**이
      #   한다(최대 5초). `kill -0` 은 좀비에도 성공하므로 상태 Z 는 소멸로 친다.
      kill "$p" 2>/dev/null
      # ★ off-by-one 수리(2026-09-03 · 2차): 옛 루프는 매 회 `확인 → sleep 1` 순서라 **마지막
      #   sleep 뒤 재확인이 없었다** — 실제 판정 지평은 4초인데 5초를 기다렸고, 5초째에 죽은
      #   프로세스를 "살아 있다"(FAIL=1)로 오보했다. 확인을 t=0,1,2,3,4,5 여섯 시점에 두어
      #   대기시간 5초를 모두 판정에 쓴다(좀비 Z* 처리는 그대로 보존).
      _gone=0
      for _i in 0 1 2 3 4 5; do
        [ "$_i" = "0" ] || sleep 1
        kill -0 "$p" 2>/dev/null || { _gone=1; break; }
        case "$(ps -o stat= -p "$p" 2>/dev/null)" in Z*) _gone=1; break ;; esac
      done
      if [ "$_gone" = "1" ]; then say "   ✓ PID $p 종료 확인"
      else say "   ✗ PID $p 가 kill 후에도 살아 있다 — 수동 확인 필요(ps -o pid,args -p $p)"; FAIL=1; fi
    else
      echo "        (dry-run) kill $p"
    fi
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
  verify_pkg_absent earlyoom
fi

# ── ④ kdump ──────────────────────────────────────────────────────────────
say "④ kdump: 우리 GRUB drop-in + sysctl trigger"
GRUB_BAK=/boot/grub/grub.cfg.easy-vllm-purge-backup
GRUB_OK=1
if [ -f /boot/grub/grub.cfg ]; then
  # ★ B7 계열(2026-09-03 · 2차 수리): 1차는 `run cp` 와 `[ -s "$GRUB_BAK" ]` 를 **나란히 두기만**
  #   해서 둘이 하나의 게이트로 결합되지 않았다. 그 결과 이전 실행이 남긴 stale 백업이 있으면
  #   cp 가 실패해도 `-s` 가 참이라 "✓ 백업: …" 을 찍고 GRUB_OK=1 이 유지돼 update-grub 이
  #   그대로 grub.cfg 를 재생성했다 — "되돌릴 수단 없이 GRUB 을 재생성하지 않는다"는 보장이
  #   깨져 있었다(백업 파일은 있지만 **현재 grub.cfg 의 사본이 아니다**).
  #   처방: ⓐ cp 의 rc **와** 결과 파일 검증을 하나의 조건으로 묶고 ⓑ 그 사본이 **이번 실행에서**
  #   떠진 것임을 내용 동일성(cmp)으로 증명한다 = stale 재사용 금지.
  #   stale 파일 자체는 지우지 않는다 — 옛 grub.cfg 라도 사람의 수동 복원에는 쓸 수 있으므로,
  #   게이트에서 인정하지 않을 뿐 증거를 파괴하지는 않는다.
  if [ "$APPLY" = "1" ]; then
    _BAK_OK=0
    if run cp -a /boot/grub/grub.cfg "$GRUB_BAK" \
       && [ -s "$GRUB_BAK" ] && cmp -s /boot/grub/grub.cfg "$GRUB_BAK"; then _BAK_OK=1; fi
    if [ "$_BAK_OK" = "1" ]; then
      say "   ✓ 백업: $GRUB_BAK ($(stat -c %s "$GRUB_BAK" 2>/dev/null) bytes · 이번 실행 사본 확인)"
    else
      say "   ✗ 백업 미확보: $GRUB_BAK — 되돌릴 수단 없이 GRUB 을 재생성하지 않는다"
      say "      (파일이 남아 있어도 **이번 실행의 사본**이 아니면 백업으로 인정하지 않는다)"
      FAIL=1; GRUB_OK=0
    fi
  else
    run cp -a /boot/grub/grub.cfg "$GRUB_BAK"
  fi
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
  verify_pkg_absent kdump-tools
fi
# hang→panic sysctl 은 파일 제거만으로는 현재 커널에 남아 있다 — 런타임도 되돌린다.
#   ★ `sysctl -w` 는 커널이 요청을 무시해도 0 을 낼 수 있으므로 **되읽어서** 판정한다.
for k in kernel.hung_task_panic kernel.softlockup_panic; do
  run sysctl -w "$k=0"
  verify_sysctl "$k" 0
done
run sysctl -w kernel.hung_task_timeout_secs=120
verify_sysctl kernel.hung_task_timeout_secs 120

# ── ④b SBSA 하드웨어 워치독 — **기본 보존**(plan_26082319 §5.1 · 2026-08-23) ─────
#   install_host_safety.sh 가 만든 파일이므로 형식적으로는 이 스크립트의 제거 대상이다.
#   그런데 **재설치 경로가 다르다**: 이 purge 뒤에 도는 install_node_blackbox.sh 는 하드웨어
#   워치독을 다시 놓지 않는다(그건 install_host_safety.sh 소관). 여기서 지우면 노드는
#   **하드 락업 복구 계층을 잃은 채 아무도 되돌려놓지 않는** 상태가 된다 —
#   레거시 RAM 평면을 걷어내려다 다른 평면을 조용히 무장해제하는 **오배달**이다.
#   그래서 기본은 보존이고, 지우려면 명시해야 한다(--purge-watchdog).
WD_DROPIN=/etc/systemd/system.conf.d/10-easy-vllm-watchdog.conf
if [ "${PURGE_WATCHDOG:-0}" = "1" ]; then
  say "④b 하드웨어 워치독 drop-in 제거(--purge-watchdog 명시) → $WD_DROPIN"
  say "   ⚠ 제거 후 이 노드는 하드 락업 시 **자동 리셋되지 않는다**(수동 재부팅만)."
  run rm -f "$WD_DROPIN"
  run systemctl daemon-reexec        # system.conf 는 daemon-reload 로 반영되지 않는다
elif [ -f "$WD_DROPIN" ]; then
  say "④b 하드웨어 워치독 drop-in **보존**: $WD_DROPIN (state=$(cat /sys/class/watchdog/watchdog0/state 2>/dev/null || echo n/a))"
  say "   근거: 이 purge 뒤의 재설치 경로가 그것을 되돌려놓지 않는다 — 지우면 복구 계층이 사라진 채 남는다."
  say "   정말 지우려면: --purge-watchdog"
fi

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
if [ "$GRUB_OK" != "1" ]; then
  say "   ⊘ GRUB 재생성 건너뜀(백업 미확보) — 백업을 확보한 뒤 재실행하라"
else
  if command -v update-grub >/dev/null 2>&1; then run update-grub
  elif command -v update-grub2 >/dev/null 2>&1; then run update-grub2
  else run grub-mkconfig -o /boot/grub/grub.cfg; fi
  # 재생성기가 rc 0 을 내고도 빈 파일을 남기는 경우가 있다(디스크 가득·중단). 결과를 다시 읽는다.
  if [ "$APPLY" = "1" ] && [ ! -s /boot/grub/grub.cfg ]; then
    say "   ✗ /boot/grub/grub.cfg 가 비었거나 사라졌다 — 복원: cp -a $GRUB_BAK /boot/grub/grub.cfg"
    FAIL=1
  fi
fi

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

  # B12(2026-09-03): 판정 자리의 `pgrep -f` 를 정본 필드 앵커로 교체했다. 옛 술어는 argv 전체
  #   부분일치라 **이 스크립트를 논한 명령줄·에디터·grep 자신**까지 세어 FAIL=1 을 만들었다.
  _WD_LEFT="$(wd_pids | tr '\n' ' ')"
  if [ -n "${_WD_LEFT// /}" ]; then
    say "  ✗ 협역 워치독 프로세스 잔존: $_WD_LEFT"; FAIL=1
  else say "  ✓ 협역 워치독 프로세스 0"; fi

  # ★ B10(2026-09-03): 잔재를 찾고도 ⚠ 만 찍고 성공 반환했다. 이 스크립트의 계약은
  #   "잔재 없이 걷어낸다"이고 요약이 "PURGE PASS — 잔재 0" 을 찍으므로, 잔재를 본 채로
  #   PASS 하는 것은 **거짓 판정**이다. 무해 여부는 사람이 정하고, 기계는 사실만 말한다.
  #   ★ 읽지 못한 것과 없는 것은 다르다 — 비-root 의 Permission denied 를 "제거됨" 으로
  #     오독한 전례(2026-07-31)를 여기서 구조적으로 막는다.
  if [ ! -r /boot/grub/grub.cfg ]; then
    say "  ✗ /boot/grub/grub.cfg 를 읽을 수 없다 — crashkernel 잔재를 **판정할 수 없다**(sudo 로 재실행)"
    FAIL=1
  else
    CK_LEFT="$(grep -o 'crashkernel=[^ ]*' /boot/grub/grub.cfg 2>/dev/null | sort -u | tr '\n' ' ')"
    if [ -n "$CK_LEFT" ]; then
      say "  ✗ grub.cfg 에 crashkernel 잔존: $CK_LEFT"
      say "    (kdump-tools 패키지를 남긴 경우 그 drop-in 이 원인 — --purge-packages 로 재실행하거나"
      say "     /etc/default/grub.d/kdump-tools.cfg 의 crashkernel= 를 지우고 update-grub)"
      FAIL=1
    else
      say "  ✓ grub.cfg crashkernel 제거됨"
    fi
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
