#!/bin/bash
# install_node_blackbox.sh — 노드블랙박스 통합 설치자 (plan_26073109 §Phase 3·§Phase 5).
#
#   승격 대상: 흩어져 있던 mem_watchdog·earlyoom·kdump·netconsole·임시 telemetry 를
#   **단일 패키지**로 묶고, 싱글/멀티 토폴로지 무관하게 동일 설치·동일 검증되게 만든다.
#
#   ★ 배치와 활성화의 분리(plan D2): 파일·유닛은 **레벨과 무관하게 항상 노드에 도착**하고,
#     활성화만 --level 로 고른다. 나중에 마음이 바뀌어도 재배달이 필요 없다.
#
#   레벨(재부팅 필요 여부가 자연 경계):
#     L1  무재부팅        수집기 · ETA 워치독 · **열·전력 워치독** · 이벤트 통합 · 수명 집행 ·
#                         drop-caches · earlyoom
#     L2  무재부팅·peer   netconsole 교차 스트리밍 (multi 전용 — single 은 N/A 로 정직 기록)
#     L3  재부팅 1회      사후 포착 = efi_pstore 확보 (crashkernel·ramoops **제거**)
#
#   ★ L3 설계는 2026-07-31 크래시 시험 5회로 뒤집혔다(testlog_26073113). 요지:
#       · kdump vmcore 0/4 — makedumpfile 이 커널 6.17 을 미지원(구조적).
#       · kdump 크래시커널 부팅 1/4 — 귀속 변수 없음(cmdline·적재시점·패닉CPU 전부 배제).
#       · **kdump 가 무장돼 있으면 pstore 가 원천 차단된다** — crash_kexec_post_notifiers=N 이라
#         panic() 이 kmsg_dump 보다 먼저 kexec 로 점프해 돌아오지 않는다. 즉 kdump 는 유일하게
#         작동하는 사후 포착 수단을 죽이면서, 그 대가로 2.25 GiB 를 상시 예약한다.
#       · ramoops 는 이 플랫폼에서 불가 — reserve_mem 주소가 부팅 간 동일한데도(0x1f68f8c000)
#         정상 재부팅만으로 헤더가 깨진다 = 펌웨어가 리셋 때 DRAM 을 초기화한다.
#       · efi_pstore 는 kdump 를 내린 상태에서 패닉 19 레코드를 포착했다(1/1).
#     ∴ L3 = crashkernel/ramoops 를 **걷어내고** efi_pstore 를 확보하는 계층이다.
#
#   실행 주체 = 사람(HITL sudo). 기본 = dry-run.
#     sudo bash install_node_blackbox.sh --apply --level L1
#     sudo bash install_node_blackbox.sh --apply --level=L3            # 인자 불요
#     sudo bash install_node_blackbox.sh --suggest-ramoops             # ramoops 판정(변경 없음)
#
# 종료코드: 0=성공(또는 dry-run) · 1=전제 실패 · 2=설치/검증 실패.
set -uo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# ★ 리포 루트는 **고정 상대깊이로 세지 않는다**. 메인에서는 이 스크립트가
#   .claude/skills/terraforming_node/scripts/node_blackbox/ 에 있지만, 서브에는 런타임으로
#   .claude/runtime/node_blackbox/ 로 배달된다(host_safety 선례). 깊이가 다르므로 ../../../../..
#   는 서브에서 홈 디렉터리(저장소 루트 바깥)를 가리켜 로그 루트가 조용히 엉뚱한 곳이 된다. 마커로 찾는다.
_find_repo(){ local d="$1"; while [ "$d" != "/" ] && [ -n "$d" ]; do
    [ -d "$d/.claude" ] && [ -d "$d/docs" ] && { printf '%s' "$d"; return 0; }; d="$(dirname "$d")"; done; return 1; }
REPO="$(_find_repo "$SDIR" || (cd "$SDIR/../../../../.." 2>/dev/null && pwd))"
# node_id 해소는 단일 소유다(plan_26081514 §4.2 · SKILL.md §2.7.6). 각자 파싱 금지.
# 부재를 조용히 넘기지 않는다 — 없으면 옛 hostname 파생으로 되돌아갈 길이 아니라 배달 결손이다.
[ -f "$SDIR/node_identity.sh" ] || {
  echo "[bb-install] FAIL: $SDIR/node_identity.sh 부재 — node_id 해소기가 배달되지 않았다." >&2
  echo "[bb-install]       메인: 리포 무결성 확인 · 서브: render_sub_env.py 재배달." >&2; exit 1; }
# shellcheck source=node_identity.sh
. "$SDIR/node_identity.sh"
APPLY=0; LEVEL="L1"; SUGGEST=0
RAMOOPS_ADDR=""; RAMOOPS_SIZE="4M"     # reserve_mem 문법(2M/4M…). 생주소 방식일 때만 0x… 로 준다
RAMOOPS_CONSOLE_SIZE="${RAMOOPS_CONSOLE_SIZE:-2097152}"   # 2 MiB — 커널 콘솔 상시 링버퍼(핵심)
RAMOOPS_RECORD_SIZE="${RAMOOPS_RECORD_SIZE:-262144}"      # 256 KiB — oops/panic dump 레코드
TARGET_USER="${SUDO_USER:-$(id -un)}"
# ★ NODE_ID 에 기본값이 **없다**(plan_26081514 스킴 R). 예전엔 `$(hostname)` 이었고, 그것이
#   틀려도 조용히 두 번째 로그 트리를 만들었다(침묵 폴백). 해소는 node_identity.sh 가 한다.
LOGS_ROOT=""; NODE_ID=""
# KDUMP_CRASHKERNEL/_LOW 는 삭제했다 — L3 가 crashkernel 을 **설정하지 않고 제거**하므로
# 예약 크기라는 개념 자체가 없어졌다(2026-07-31 판정, 파일 상단 주석).

# `--level L1`(공백형)과 `--level=L1`(등호형)을 **둘 다** 받는다.
# 예전엔 공백형을 거부했는데, 정작 이 파일 헤더(L27)가 공백형을 예시로 적고 있었다 —
# 문서와 파서가 어긋나면 사용자는 문서를 믿고 실패한다(2026-08-01 실제 왕복 발생).
# for 루프는 다음 인자를 소비할 수 없으므로 while+shift 로 바꾼다.
while [ $# -gt 0 ]; do
  a="$1"
  case "$a" in
    --apply) APPLY=1 ;;
    --level=*) LEVEL="${a#--level=}" ;;
    --level)
      [ $# -ge 2 ] || { echo "[bb-install] --level 뒤에 L1|L2|L3 이 필요하다" >&2; exit 1; }
      LEVEL="$2"; shift ;;
    L1|L2|L3) LEVEL="$a" ;;
    --suggest-ramoops) SUGGEST=1 ;;
    --ramoops-addr=*) RAMOOPS_ADDR="${a#--ramoops-addr=}" ;;
    --ramoops-size=*) RAMOOPS_SIZE="${a#--ramoops-size=}" ;;
    --logs-root=*) LOGS_ROOT="${a#--logs-root=}" ;;
    --user=*) TARGET_USER="${a#--user=}" ;;
    --node-id=*) NODE_ID="${a#--node-id=}" ;;
    -h|--help) sed -n '2,26p' "$0"; exit 0 ;;
    *) echo "[bb-install] 알 수 없는 인자: $a" >&2; exit 1 ;;
  esac
  shift
done
case "$LEVEL" in L1|L2|L3) ;; *) echo "[bb-install] --level 은 L1|L2|L3" >&2; exit 1 ;; esac
[ -n "$LOGS_ROOT" ] || LOGS_ROOT="$REPO/docs/logs"

say(){ echo "[bb-install] $*"; }
FAIL=0
# ★ rc 전파 (2026-09-03 · B4). 예전 run() 은 `"$@"` 의 rc 를 그대로 반환했지만 `set -e` 가 없고
#   18개 호출부 중 rc 를 보는 곳이 **0개**라, 실패가 조용히 삼켜지고 스크립트는 끝에서
#   "INSTALL PASS" 를 찍었다 — 반환값이 있어도 **듣는 사람이 없으면 fail-open 이다.**
#   호출부를 18곳 고치는 대신 래퍼가 직접 판정을 기록한다(단일 자리 = 갈라지지 않는다).
#   ⓐ 실패를 큰 소리로 찍고 ⓑ FAIL 을 세우고(최종 `exit "$FAIL"` 로 프로세스 rc 에 전파)
#   ⓒ rc 를 그대로 되돌려 호출부가 원하면 추가로 분기할 수 있게 한다.
#   ★ FAIL 은 **대입이지 증분이 아니다** — 헤더의 종료코드 taxonomy(0/1/2)를 넘겨 3,4… 로
#     새면 수행지시서의 성공판정이 깨진다.
run(){
  if [ "$APPLY" = 1 ]; then
    "$@"
    local _rc=$?
    if [ "$_rc" -ne 0 ]; then say "   ✗ 실패(rc=$_rc): $*"; FAIL=1; fi
    return "$_rc"
  else
    echo "            (dry-run) $*"
  fi
}
BIN=/usr/local/sbin
ETC=/etc/easy-vllm
# NODE_DIR 해소는 --suggest-ramoops 블록 **뒤**로 내렸다 — 그 판정은 노드 정체성과 무관한
# 플랫폼 질의(변경 0)라, 미테라포밍 노드에서도 답할 수 있어야 한다.

# ── ramoops 안전영역 제안 (변경 0 — 근거 제시만) ─────────────────────────
if [ "$SUGGEST" = 1 ]; then
  if [ "$(id -u)" -ne 0 ]; then
    say "FAIL: --suggest-ramoops 는 /proc/iomem 실주소가 필요해 root 여야 합니다(sudo)."
    say "      (비-root 는 kptr_restrict 로 전 항목이 0 으로 보여 오판을 부릅니다.)"
    exit 1
  fi
  say "ramoops 예약 방식 판정 (플랫폼 실측 — 주소를 고르지 않는다)"
  echo
  HAS_DT=0; [ -d /proc/device-tree ] && HAS_DT=1
  HAS_RESERVE_MEM=0
  grep -q "reserve_mem_find_by_name" /proc/kallsyms 2>/dev/null && HAS_RESERVE_MEM=1
  HAS_MEM_NAME=0
  _mi="$(modinfo ramoops 2>/dev/null)"      # grep -q 를 파이프에서 분리(pipefail 위음성 방지)
  grep -q "^parm:[[:space:]]*mem_name" <<< "$_mi" && HAS_MEM_NAME=1
  echo "  device-tree(reserved-memory 경로) : $([ $HAS_DT = 1 ] && echo 있음 || echo '없음 → ACPI 부팅')"
  echo "  커널 reserve_mem 지원             : $([ $HAS_RESERVE_MEM = 1 ] && echo 있음 || echo 없음)"
  echo "  ramoops mem_name 파라미터         : $([ $HAS_MEM_NAME = 1 ] && echo 있음 || echo 없음)"
  echo "  커널                              : $(uname -r) / $(uname -m)"
  echo
  # ★ 이 판정은 "예약이 문법적으로 가능한가"만 답한다. **그것이 곧 쓸모를 뜻하지 않는다.**
  #   2026-07-31 DGX Spark(GB10) 실측: reserve_mem 예약은 성공했고 물리주소도 부팅 간 동일했는데
  #   (0x400000@0x1f68f8c000 × 3 부팅), 크래시가 아닌 **정상 재부팅만으로** 헤더가 깨졌다
  #   ("ramoops: error in header" · /sys/fs/pstore 비어있음). 즉 예약은 되지만 내용이 안 남는다
  #   = 펌웨어가 리셋 때 DRAM 을 초기화한다. 문법 가능성과 잔존성은 별개 축이다.
  if [ "$HAS_RESERVE_MEM" = 1 ] && [ "$HAS_MEM_NAME" = 1 ]; then
    say "예약 문법 판정: reserve_mem 방식 사용 **가능**(reserve_mem=…:4096:oops + ramoops.mem_name=oops)."
    say "                주소를 우리가 고르지 않으므로 메모리 손상 위험은 없다."
  elif [ "$HAS_DT" = 1 ]; then
    say "예약 문법 판정: reserve_mem 미지원 · device-tree 있음 → DT reserved-memory 가 정본 경로."
    say "                cmdline 생주소 지정은 금지(예약되지 않은 RAM 을 덮어쓴다)."
  else
    say "예약 문법 판정: **불가** — reserve_mem 미지원 + device-tree 부재(ACPI)."
    say "                arm64 는 x86 의 memmap= 이 없어 생주소 지정은 커널 메모리를 덮어쓴다."
  fi
  echo
  say "★ 잔존성 판정(별개 축 — 이쪽이 실제 쓸모를 결정한다)"
  case "$(cat /sys/devices/virtual/dmi/id/product_name 2>/dev/null || echo unknown)" in
    *DGX_Spark*|*DGX\ Spark*)
      say "   이 플랫폼(DGX Spark/GB10) = **ramoops 사용 금지**. 2026-07-31 직접 반증:"
      say "     · 같은 물리주소(0x1f68f8c000)로 3회 부팅 · 마커 기록 후 **정상** 재부팅"
      say "     · 결과: 'ramoops: error in header' · pstore 비어있음 → DRAM 이 리셋에 소멸"
      say "   정본 = efi_pstore(NVRAM). 단 **kdump 를 내려야** 기록된다(kexec 가 kmsg_dump 선점)."
      say "   설치: --level=L3 (crashkernel·ramoops 를 제거하고 efi_pstore 를 확보한다)" ;;
    *)
      say "   미검증 플랫폼. ramoops 를 쓰려면 크래시 없이 먼저 확인하라(비용 0):"
      say "     ① pstore.backend=ramoops 로 부팅 → ② echo MARKER > /dev/kmsg → ③ **정상** reboot"
      say "     ④ 부팅 로그에 'error in header' 가 없고 /sys/fs/pstore 에 내용이 있으면 잔존 OK."
      say "   확인 못 했으면 efi_pstore 를 쓰라 — NVRAM 이라 리셋·전원차단을 견딘다." ;;
  esac
  exit 0
fi

if [ "$APPLY" = 1 ] && [ "$(id -u)" -ne 0 ]; then
  say "FAIL: --apply 는 root 필요 — sudo bash $0 --apply --level=$LEVEL"; exit 1
fi

# ── node_id 해소 (fail-loud · 기본값 없음) ───────────────────────────────
#   여기서 실패하면 설치가 멈춘다. 예전처럼 hostname 으로 밀고 나가면 유닛에 엉뚱한 경로가
#   구워지고(문자열 bake) 워치독이 아무도 안 보는 트리에 기록한다 — 사고가 나야 발견된다.
NODE_ID="$(ni_resolve_node_id "$REPO" "$NODE_ID")" || exit 1
NODE_DIR="$LOGS_ROOT/$NODE_ID"

# ── 전제 확인 ────────────────────────────────────────────────────────────
for f in blackbox_collect.py blackbox_eta.py blackbox_events.py logs_lifecycle.py \
         regen_envelope.py mem_watchdog_eta.sh; do
  [ -f "$SDIR/$f" ] || { say "FAIL: $SDIR/$f 부재"; exit 1; }
done
DROP_HELPER="$SDIR/../host_safety/host/vllm-drop-caches.sh"
[ -f "$DROP_HELPER" ] || { say "FAIL: drop-caches 헬퍼 부재: $DROP_HELPER"; exit 1; }

say "노드=$NODE_ID · 레벨=$LEVEL · 모드=$([ "$APPLY" = 1 ] && echo APPLY || echo DRY-RUN)"
say "로그 루트=$NODE_DIR · 위임 사용자=$TARGET_USER"

# ── 0. 공통 배치 (레벨 무관 — 항상 도착시킨다) ───────────────────────────
say "0. 스크립트 배치 → $BIN · 설정 → $ETC · 로그 루트 준비"
run install -d -m 0755 "$ETC"
# ★ **프로젝트 경로 조상까지 위임 사용자 소유로** (2026-09-03 · plan_26090317 P3 실화).
#   `install -d` 는 없는 조상을 **root 소유로** 만든다. 사용자가 서브 프로젝트 경로를 완전삭제하면
#   (이 프로젝트의 CI/CD 대리 실험이 정확히 그 시나리오다) 다음 폴에서 root 데몬이 work_dir·docs·
#   docs/logs 를 **root:root 로 재생성**한다. 그 뒤로는 위임 사용자가 프로젝트 경로에 아무것도 쓸 수
#   없고, 메인의 정착(rsync/git init)이 통째로 막힌다 — 그런데 증상은 배달 중간의 rsync 실패로
#   나타나서 원인이 소유권이라는 것이 드러나지 않는다(2026-09-03 실측: 서브 work_dir = root:root 755,
#   롤백의 rm 까지 Permission denied 로 실패해 CRITICAL 로 끝났다).
#   데몬은 **로그만** 쓰면 되고 프로젝트는 위임 사용자의 것이다. 조상 체인을 멱등하게 정렬한다.
_bb_align_ancestors() {   # $1=work_dir 아래의 로그 루트 → work_dir 까지 거슬러 소유권 정렬
    local d="$1" root_dir
    root_dir="${d%/docs/logs/*}"
    [ -n "$root_dir" ] && [ "$root_dir" != "$d" ] || return 0
    local chain="$root_dir $root_dir/docs $root_dir/docs/logs"
    local x
    for x in $chain; do
        [ -e "$x" ] || run install -d -m 0775 -o "$TARGET_USER" -g "$TARGET_USER" "$x"
    done
    # 이미 root 소유로 굳어 있으면 되돌린다(재실행이 곧 교정 — 멱등).
    for x in $chain; do
        if [ -d "$x" ] && [ "$(stat -c '%U' "$x" 2>/dev/null)" != "$TARGET_USER" ]; then
            say "   소유권 정렬: $x ($(stat -c '%U:%G' "$x")) → $TARGET_USER"
            run chown "$TARGET_USER:$TARGET_USER" "$x"
        fi
    done
}
_bb_align_ancestors "$NODE_DIR"
run install -d -m 0775 -o "$TARGET_USER" -g "$TARGET_USER" "$NODE_DIR"
run install -d -m 0775 -o "$TARGET_USER" -g "$TARGET_USER" "$NODE_DIR/samples" \
        "$NODE_DIR/events" "$NODE_DIR/rollup"
# ★ **기존 파일 소유 복구** — 디렉터리만 위임 사용자 소유로 만들면 부족하다. `events/<월>.jsonl` 은
#   root 데몬과 비-root 사용자 도구가 함께 append 하는데, 파일은 install 이 만들지 않으므로
#   그 달 **먼저 쓴 쪽이 소유자**가 된다. root 가 이기면 `blackbox_session declare-budget` 이
#   EACCES 로 죽고 ETA 위양성 방어(선언된 바닥)가 통째로 못 선다(2026-08-01 서브 실화 —
#   메인은 우연히 사용자 도구가 먼저 써서 멀쩡했다. 우연한 성공을 설계로 착각하지 않는다).
#   작성자 쪽도 디렉터리 소유자를 상속하도록 고쳤지만, **이미 어긋난 노드**는 여기서만 복구된다.
#   재실행이 곧 교정이 되도록 멱등하게 매번 정렬한다.
run chown -R "$TARGET_USER:$TARGET_USER" "$NODE_DIR"
# 설치명은 전부 대시로 통일한다(파일명 언더스코어를 그대로 쓰면 easy-vllm-blackbox_collect 처럼
# 표기가 섞여 유닛/문서/검증자 사이에서 오탈자 원인이 된다).
run install -m 0755 "$SDIR/blackbox_collect.py"  "$BIN/easy-vllm-bb-collect"
run install -m 0755 "$SDIR/blackbox_eta.py"      "$BIN/easy-vllm-bb-eta"
# ★ sibling import 통로 (2026-09-01 · audit_26090109 ⑦ 1단) ─────────────────────────
#   regen_envelope · blackbox_session · agent_guard 는 `from blackbox_eta import DEFAULTS` 로
#   **정본 상수**를 파생한다(상수 사본은 갈라져 맹점을 만든다 — ① 참조). 그런데 위 줄이
#   `.py` 확장자를 떼고 콘솔 이름으로 설치하므로, 실행체의 `sys.path[0]`(=$BIN)에는
#   `blackbox_eta.py` 가 **없다**. 그래서 import 가 깨진다.
#   실측(2026-09-01): easy-vllm-blackbox-lifecycle.service 가 5일 연속 status=1/FAILURE 였고
#   저널에 `blackbox_eta.py 를 같은 디렉터리에서 찾지 못했다` 가 그대로 찍혀 있었으며,
#   **envelope.json 은 한 번도 생성된 적이 없다**. 가드는 정확히 울었고 듣는 사람이 없었다.
#   모듈 이름으로도 함께 둔다(0644 — 실행체가 아니라 라이브러리 사본임을 모드로 표시).
run install -m 0644 "$SDIR/blackbox_eta.py"      "$BIN/blackbox_eta.py"
run install -m 0755 "$SDIR/blackbox_events.py"   "$BIN/easy-vllm-bb-events"
run install -m 0755 "$SDIR/logs_lifecycle.py"    "$BIN/easy-vllm-bb-lifecycle"
run install -m 0755 "$SDIR/regen_envelope.py"   "$BIN/easy-vllm-bb-regen-envelope"
run install -m 0755 "$SDIR/mem_watchdog_eta.sh" "$BIN/easy-vllm-bb-watchdog"
# 열·전력 포락선 축(plan_26082319 §6.3 · 2026-08-23). RAM 축과 **별개 평면**이라 파일도 유닛도
# 상수도 따로 둔다 — 한쪽 재emit 이 다른 쪽 판정 규칙을 조용히 갈아끼우지 않게.
run install -m 0755 "$SDIR/blackbox_thermal.py"  "$BIN/easy-vllm-bb-thermal"
run install -m 0755 "$SDIR/thermal_watchdog.sh"  "$BIN/easy-vllm-bb-tp-watchdog"
run install -m 0755 "$DROP_HELPER" "$BIN/vllm-drop-caches"

# ★ 설치 사후검증 (2026-09-03 · B4). `install` 의 rc 는 이제 run() 이 보지만, **rc 0 이 곧
#   "쓸 수 있는 상태"는 아니다** — noexec 마운트·부분 쓰기·경합 재설치는 rc 0 을 내고도
#   실행 불가한 파일을 남긴다. 배치의 목적은 "명령이 돌았다"가 아니라 "그 자리에 실행 가능한
#   바이너리가 있다"이므로, 목적 그대로를 다시 읽어 판정한다.
#   0644 라이브러리 사본(blackbox_eta.py)은 **실행권한이 없어야 정상**이라 존재만 본다.
verify_installed(){   # $1=경로 $2=x(실행체)|r(라이브러리)
  [ "$APPLY" = 1 ] || return 0
  if [ ! -f "$1" ]; then say "   ✗ 미설치: $1"; FAIL=1; return 1; fi
  if [ "$2" = "x" ] && [ ! -x "$1" ]; then say "   ✗ 실행권한 없음: $1"; FAIL=1; return 1; fi
  if [ ! -s "$1" ]; then say "   ✗ 빈 파일: $1"; FAIL=1; return 1; fi
  return 0
}
if [ "$APPLY" = 1 ]; then
  _BB_MISS=0
  for _b in easy-vllm-bb-collect easy-vllm-bb-eta easy-vllm-bb-events easy-vllm-bb-lifecycle \
            easy-vllm-bb-regen-envelope easy-vllm-bb-watchdog easy-vllm-bb-thermal \
            easy-vllm-bb-tp-watchdog vllm-drop-caches; do
    verify_installed "$BIN/$_b" x || _BB_MISS=$((_BB_MISS+1))
  done
  verify_installed "$BIN/blackbox_eta.py" r || _BB_MISS=$((_BB_MISS+1))
  if [ "$_BB_MISS" = 0 ]; then say "   ✓ $BIN 배치 10종 실재·실행권한 확인"
  else say "   ✗ $BIN 배치 $_BB_MISS 종 결손 — 위 ✗ 확인(sys.path[0] sibling import 포함)"; fi
fi

# ★ **환류 배선**(plan_26081415 C2-4). 예전엔 `--node-dir` 없이 호출해 envelope 의 eta_params 가
#   상수 파일에 **한 번도 닿지 않았다** — 포락선을 아무리 갱신해도 워치독 판정은 DEFAULTS 그대로였고,
#   그래서 "동결을 풀면 고쳐진다"는 기대가 구조적으로 틀렸다(§1.2). 경로를 여기서 잇는다.
#   미지 키가 있으면 blackbox_eta 가 **거부**하므로(조용한 버림 폐지) 설치가 큰 소리로 멈춘다.
say "   ETA 상수 생성 → $ETC/eta_params.env (envelope 환류 + 하한 가드 통과 시에만 기록)"
if [ "$APPLY" = 1 ]; then
  if python3 "$SDIR/blackbox_eta.py" --node-dir "$NODE_DIR" --emit-params "$ETC/eta_params.env"; then
    say "   ✓ eta_params.env (envelope=$NODE_DIR/envelope.json 반영)"
  else
    say "   ✗ ETA 파라미터 거부(하한 가드 또는 envelope 키 불일치) — 설치 중단"
    say "     키 불일치면: $BIN/easy-vllm-bb-regen-envelope --node-dir $NODE_DIR regen --now <ISO>"
    exit 2
  fi
fi

# 열·전력 상수도 **같은 자리에서** 생성한다. 여기서 안 만들면 핫루프가 내장 기본값으로 돌고,
# 그 사실이 로그 한 줄(`params=defaults(내장)`)로만 남아 사실상 보이지 않는다.
say "   열·전력 상수 생성 → $ETC/thermal_params.env (하한 가드 통과 시에만 기록)"
if [ "$APPLY" = 1 ]; then
  if python3 "$SDIR/blackbox_thermal.py" --emit-params "$ETC/thermal_params.env"; then
    say "   ✓ thermal_params.env"
  else
    say "   ✗ 열·전력 파라미터 거부(하한 가드) — 설치 중단"
    exit 2
  fi
fi

# ── 유닛 파일 (항상 배치, 활성화는 레벨이 결정) ──────────────────────────
write_units(){
  cat > /etc/systemd/system/easy-vllm-blackbox-collect.service <<EOF
[Unit]
Description=easy-vllm node blackbox: 1s sampler (crash-safe, no batching)
After=docker.service
[Service]
Type=simple
ExecStart=$BIN/easy-vllm-bb-collect --node-dir $NODE_DIR --interval 1
Restart=always
RestartSec=5
Nice=-5
OOMScoreAdjust=-500
[Install]
WantedBy=multi-user.target
EOF
  cat > /etc/systemd/system/easy-vllm-blackbox-watchdog.service <<EOF
[Unit]
Description=easy-vllm node blackbox: ETA reflex watchdog (unified-memory hard-down guard)
After=docker.service
Wants=docker.service
[Service]
Type=simple
Environment=BB_PARAMS=$ETC/eta_params.env
Environment=BB_EVENTS=$NODE_DIR/events/watchdog.jsonl
ExecStart=$BIN/easy-vllm-bb-watchdog @vllm 1
Restart=always
RestartSec=5
Nice=-10
OOMScoreAdjust=-500
[Install]
WantedBy=multi-user.target
EOF
  # 열·전력 포락선 워치독. **수집기 뒤에 세운다** — GPU 전력을 수집기 CSV 꼬리에서 읽기 때문이다
  # (SoC 열은 sysfs 직접이라 수집기와 무관하게 산다). 수집기가 죽으면 전력 축은 stale 로 외치고
  # 버킷을 동결한다 — 조용히 '안전'으로 넘어가지 않는다.
  cat > /etc/systemd/system/easy-vllm-blackbox-thermal.service <<EOF
[Unit]
Description=easy-vllm node blackbox: thermal/power envelope watchdog (hard-lockup prevention)
After=docker.service easy-vllm-blackbox-collect.service
Wants=docker.service easy-vllm-blackbox-collect.service
[Service]
Type=simple
Environment=BB_TP_PARAMS=$ETC/thermal_params.env
Environment=BB_TP_NODE_DIR=$NODE_DIR
Environment=BB_TP_EVENTS=$NODE_DIR/events/thermal.jsonl
ExecStart=$BIN/easy-vllm-bb-tp-watchdog @vllm 1
Restart=always
RestartSec=5
Nice=-10
OOMScoreAdjust=-500
[Install]
WantedBy=multi-user.target
EOF
  cat > /etc/systemd/system/easy-vllm-blackbox-events.service <<EOF
[Unit]
Description=easy-vllm node blackbox: unify kill paths into the event plane
[Service]
Type=oneshot
ExecStart=$BIN/easy-vllm-bb-events --node-dir $NODE_DIR
EOF
  cat > /etc/systemd/system/easy-vllm-blackbox-events.timer <<'EOF'
[Unit]
Description=easy-vllm node blackbox: event collection every 5 minutes
[Timer]
OnBootSec=2min
OnUnitActiveSec=5min
[Install]
WantedBy=timers.target
EOF
  # rollup → envelope 환류(plan_26081415 C2-1). rollup 이 **먼저** 끝난 뒤 재생성해야 그날 통계가
  # 포락선에 담긴다 — 두 ExecStart 는 systemd 가 순차 실행하며 앞이 실패하면 뒤는 돌지 않는다.
  #
  # ⚠ 여기서 `blackbox_eta --emit-params` 를 **부르지 않는다**(의도적). 포락선 갱신은 데이터이고
  #   상수 파일 갱신은 **판정 규칙 변경**이라 평면이 다르다. 자동 재emit 하면 실측 kill 지연이
  #   활주로를 늘려 무조건-트립 경계를 조용히 낮출 수 있다(2026-08-14 실측: 4.0s → 6.0s ⇒
  #   경계 15,576 → 12,461 MiB/s = 위양성 증가 방향). 규칙 변경은 C1 의 dry-run·HITL 을 거친다.
  cat > /etc/systemd/system/easy-vllm-blackbox-lifecycle.service <<EOF
[Unit]
Description=easy-vllm node blackbox: logs lifecycle (rollup -> envelope regen -> compress -> evict)
[Service]
Type=oneshot
ExecStart=/bin/sh -c '$BIN/easy-vllm-bb-lifecycle --node-dir $NODE_DIR --now "\$(date -u +%%Y-%%m-%%dT%%H:%%M:%%SZ)" --apply'
ExecStart=/bin/sh -c '$BIN/easy-vllm-bb-regen-envelope --node-dir $NODE_DIR regen --now "\$(date -u +%%Y-%%m-%%dT%%H:%%M:%%SZ)"'
EOF
  cat > /etc/systemd/system/easy-vllm-blackbox-lifecycle.timer <<'EOF'
[Unit]
Description=easy-vllm node blackbox: daily lifecycle enforcement
[Timer]
OnCalendar=daily
Persistent=true
RandomizedDelaySec=30min
[Install]
WantedBy=timers.target
EOF
}
say "   systemd 유닛 6종 배치(collect·watchdog·thermal·events.timer·lifecycle.timer)"
if [ "$APPLY" = 1 ]; then write_units; else echo "            (dry-run) write_units"; fi

# ── L1: 무재부팅 계층 활성화 ─────────────────────────────────────────────
say "L1. 수집기·ETA 워치독·열전력 워치독·이벤트·수명 활성화 + sudoers + earlyoom"
run systemctl daemon-reload
for u in easy-vllm-blackbox-collect.service easy-vllm-blackbox-watchdog.service \
         easy-vllm-blackbox-thermal.service \
         easy-vllm-blackbox-events.timer easy-vllm-blackbox-lifecycle.timer; do
  run systemctl enable --now "$u"
done
# ★ enable --now 는 **이미 돌고 있는 유닛을 재시작하지 않는다.** 재설치로 $BIN 의 스크립트를
#   갈아끼워도 옛 프로세스가 옛 코드로 계속 돌아 "배포했는데 반영이 안 되는" 침묵 실패가 된다.
#   try-restart 는 활성 유닛만 재시작하므로(비활성은 no-op) 위 enable 과 안전하게 겹친다.
#   (2026-08-01 선언된-바닥 배포 때 현실화 — testlog_26073123)
for u in easy-vllm-blackbox-collect.service easy-vllm-blackbox-watchdog.service \
         easy-vllm-blackbox-thermal.service; do
  run systemctl try-restart "$u"
done

say "   sudoers 단일 NOPASSWD 엔트리($TARGET_USER → vllm-drop-caches)"
if [ "$APPLY" = 1 ]; then
  T="$(mktemp)"
  printf '%s ALL=(root) NOPASSWD: %s/vllm-drop-caches\n' "$TARGET_USER" "$BIN" > "$T"
  # ★ 배치 사후검증(2026-09-03 · 2차 수리). 옛 코드는 후보의 visudo 검증만 게이트로 쓰고
  #   **배치 자체는 rc 를 버린 채**(run() 미경유) 다음 줄에서 "✓ sudoers" 를 조건 없이 찍었다 —
  #   /etc 가 읽기전용이거나 배치가 잘려도 PASS 로 보였고, $BIN 배치와 달리 새 사후검증에도
  #   들어 있지 않았다. 판정은 목적 그대로를 다시 읽어서 한다: **그 자리에 0440 으로 유효한
  #   sudoers 조각이 있는가**. 모드가 중요하다 — sudo 는 0440 이 아닌 조각을 무시하므로
  #   "파일은 있는데 권한 상승은 안 되는" 침묵 실패가 된다(verify_installed 는 모드를 보지
  #   않으므로 여기서는 쓰지 않고 명시 검사한다).
  SUDOERS_F=/etc/sudoers.d/easy-vllm-host-safety
  if visudo -cf "$T" >/dev/null 2>&1; then
    if run install -m 0440 "$T" "$SUDOERS_F" \
       && [ -f "$SUDOERS_F" ] \
       && [ "$(stat -c %a "$SUDOERS_F" 2>/dev/null)" = "440" ] \
       && visudo -cf "$SUDOERS_F" >/dev/null 2>&1; then
      say "   ✓ sudoers(visudo 검증 통과 · $SUDOERS_F 0440 배치 확인)"
    else
      say "   ✗ sudoers 배치 실패: $SUDOERS_F (실재·모드 0440·배치본 visudo 재검증 중 하나가 실패)"
      say "     확인: ls -l $SUDOERS_F · visudo -cf $SUDOERS_F"
      FAIL=1
    fi
  else
    say "   ✗ sudoers 후보 검증 실패 — 미설치"; FAIL=1
  fi
  rm -f "$T"
fi

say "   earlyoom (프로세스-레벨 최후선 — 빌드 평면까지 커버)"
if [ "$APPLY" = 1 ]; then
  if ! command -v earlyoom >/dev/null 2>&1; then
    DEB=""; for d in "$SDIR"/offline/earlyoom_*.deb; do [ -f "$d" ] && DEB="$d" && break; done
    if [ -n "$DEB" ]; then dpkg -i "$DEB" >/dev/null 2>&1 || apt-get install -y earlyoom >/dev/null 2>&1
    else apt-get install -y earlyoom >/dev/null 2>&1; fi
  fi
  if command -v earlyoom >/dev/null 2>&1; then
    cat > /etc/default/earlyoom <<'EOF'
# easy-vllm node blackbox — ETA 워치독보다 낮은 프로세스-레벨 최후선.
# 워치독은 컨테이너 평면만 본다(docker ps) → 빌드 평면(buildkit·cicc·JIT)은 여기서만 잡힌다.
EARLYOOM_ARGS="-m 4 -r 3600 --prefer '(VLLM|EngineCor|ray::|vllm)' --avoid '(systemd|sshd|dockerd|containerd|journald|earlyoom|easy-vllm)'"
EOF
    systemctl enable --now earlyoom >/dev/null 2>&1
    systemctl restart earlyoom >/dev/null 2>&1
    systemctl is-active --quiet earlyoom && say "   ✓ earlyoom active" || { say "   ✗ earlyoom 비활성"; FAIL=1; }
  else
    say "   ✗ earlyoom 설치 실패(오프라인이면 $SDIR/offline/ 에 .deb 스테이징)"; FAIL=1
  fi
fi

# ── L2: netconsole 교차 (multi 전용) ─────────────────────────────────────
if [ "$LEVEL" = "L2" ] || [ "$LEVEL" = "L3" ]; then
  say "L2. netconsole 교차 스트리밍 (multi 전용)"
  NC="$SDIR/../host_safety/install_netconsole.sh"
  if [ -f "$NC" ]; then
    if [ -f "$REPO/output/multi/manifest.yaml" ]; then
      run bash "$NC" $([ "$APPLY" = 1 ] && echo --apply)
    else
      say "   N/A: multi manifest 부재 = 단일노드 → netconsole 미적용(정직 기록)"
      say "        single 의 사후 포착은 L3 efi_pstore 가 담당한다(peer 불요 · 패닉 경로 한정)"
    fi
  else
    say "   ✗ $NC 부재"; FAIL=1
  fi
fi

# ── L3: 사후 포착 = efi_pstore 확보 (재부팅 1회) ─────────────────────────
#   이 계층이 하는 일은 무언가를 **더하는 게 아니라 걷어내는 것**이다. 근거는 파일 상단 주석의
#   2026-07-31 크래시 시험 5회. 요약하면: kdump 는 유일하게 작동하는 포착 수단(efi_pstore)을
#   원천 차단하면서 2.25 GiB 를 예약한다. 그래서 제거가 곧 설치다.
if [ "$LEVEL" = "L3" ]; then
  say "L3. 사후 포착(efi_pstore) 확보 — **재부팅 1회 필요**"
  say "    · crashkernel(2.25 GiB 예약) 제거 → kexec 가 kmsg_dump 를 선점하지 못하게 한다"
  say "    · ramoops/reserve_mem 제거 → 이 플랫폼은 리셋 때 DRAM 이 초기화된다(직접 반증됨)"
  say "    · efi_pstore + systemd-pstore 아카이브 경로 확보"

  if [ "$APPLY" = 1 ]; then
    # ① kdump 무장 해제. 패키지는 남긴다(제거는 되돌리기 어렵고, 우리가 원하는 건 '무장 해제'다).
    #    USE_KDUMP=0 으로 두면 kdump-tools.service 가 kexec 를 적재하지 않는다.
    if [ -f /etc/default/kdump-tools ]; then
      grep -qE '^USE_KDUMP=' /etc/default/kdump-tools \
        && sed -i 's/^USE_KDUMP=.*/USE_KDUMP=0/' /etc/default/kdump-tools \
        || echo 'USE_KDUMP=0' >> /etc/default/kdump-tools
      # 이전 설치가 남긴 KDUMP_CMDLINE 오버라이드는 의미를 잃었으므로 걷어낸다.
      sed -i '/^KDUMP_CMDLINE=/d' /etc/default/kdump-tools
    fi
    systemctl disable --now kdump-tools >/dev/null 2>&1 || true
    kdump-config unload >/dev/null 2>&1 || true
    # ★ B6(2026-09-03): 위 두 줄의 `|| true` 는 **남긴다** — 이 rc 는 "패키지 미설치"(정상)와
    #   "무장 해제 실패"(결함)를 구분하지 못해 판정 근거가 못 되기 때문이다. 그래서 폐기한 것은
    #   rc 가 아니라 판정 자체였다: 예전엔 결과를 한 번도 읽지 않고 ✓ 를 조건 없이 찍었다.
    #   판정은 **결과 상태**로 한다 — 커널이 크래시 이미지를 적재 중인지(kexec_crash_loaded)가
    #   pstore 차단 여부의 직접 사실이다(crash_kexec_post_notifiers=N 이라 적재돼 있으면
    #   panic() 이 kmsg_dump 보다 먼저 kexec 로 점프한다).
    _KL="$(cat /sys/kernel/kexec_crash_loaded 2>/dev/null || echo '')"
    if [ -z "$_KL" ]; then
      say "   ✗ /sys/kernel/kexec_crash_loaded 를 읽지 못했다 — kdump 무장 상태를 **판정할 수 없다**"; FAIL=1
    elif [ "$_KL" = "0" ]; then
      say "   ✓ kdump 무장 해제(USE_KDUMP=0 · 서비스 disable · kexec unload · kexec_crash_loaded=0)"
    else
      say "   ✗ kexec 크래시 이미지가 여전히 적재됨(kexec_crash_loaded=$_KL) — panic 이 pstore 를 못 탄다"
      say "     확인: systemctl status kdump-tools · kdump-config status · /etc/default/kdump-tools"
      FAIL=1
    fi

    # ② hang→panic 승격은 **유지한다**. 오히려 지금 더 중요하다: 무음 hang 은 아무 경로도 타지
    #    않아 기록이 0 이 되는데, panic 으로 승격되면 efi_pstore 가 그 순간을 잡는다.
    #    (이전엔 이 sysctl 이 kdump 를 먹였다. 이제는 pstore 를 먹인다 — 이름만 바꾼다.)
    cat > /etc/sysctl.d/99-easy-vllm-panic-promote.conf <<'EOF'
# hang/softlockup -> panic 승격. 무음 hang 은 어떤 사후 포착 경로도 타지 못한다(2026-07-11 실증).
# panic 으로 승격되면 kmsg_dump 가 돌고 efi_pstore 가 마지막 커널 로그를 NVRAM 에 남긴다.
kernel.hung_task_panic=1
kernel.hung_task_timeout_secs=60
kernel.softlockup_panic=1
kernel.panic=10
EOF
    rm -f /etc/sysctl.d/99-easy-vllm-kdump-trigger.conf     # 구 이름 정리
    # ★ B6 계열(2026-09-03 · 2차 수리): 같은 블록 안에서 처방이 비대칭이었다 — kdump 는 결과
    #   상태(kexec_crash_loaded)로 판정하는데, 바로 아래 두 줄은 여전히 rc 를 버리고 ✓ 를
    #   **조건 없이** 찍었다. 여기서는 rc 를 살릴 수 있다: `sysctl -p` 의 비-0 은 "파일을 못
    #   읽었다/키가 없다"는 실패 신호이지 정상 상태가 아니다(kdump 의 `|| true` 와 다르다).
    #   그래도 최종 판정은 **결과 상태**로 한다 — sysctl 은 커널이 요청을 무시해도 0 을 낼 수
    #   있기 때문이다. 기대값은 방금 쓴 파일에서 파생한다(손으로 두 번 적으면 갈라진다).
    if sysctl -p /etc/sysctl.d/99-easy-vllm-panic-promote.conf >/dev/null 2>&1; then _SP_RC=0
    else _SP_RC=$?; fi
    _PP_BAD=""
    while IFS='=' read -r _k _want; do
      case "$_k" in ''|'#'*) continue ;; esac
      _got="$(sysctl -n "$_k" 2>/dev/null)"
      [ "$_got" = "$_want" ] || _PP_BAD="$_PP_BAD $_k=${_got:-<읽기실패>}(기대 $_want)"
    done < /etc/sysctl.d/99-easy-vllm-panic-promote.conf
    if [ -n "$_PP_BAD" ]; then
      say "   ✗ hang→panic 승격 미반영(sysctl -p rc=$_SP_RC):$_PP_BAD"
      say "     무음 hang 이 panic 으로 승격되지 않으면 efi_pstore 가 잡을 순간 자체가 없다"
      FAIL=1
    elif [ "$_SP_RC" -ne 0 ]; then
      say "   ✗ hang→panic 승격 값은 맞지만 sysctl -p 가 실패했다(rc=$_SP_RC) — 재부팅 후 재적용이 보장되지 않는다"
      FAIL=1
    else
      say "   ✓ hang→panic 승격 sysctl 적용 확인 (이제 efi_pstore 를 먹인다)"
    fi

    # ③ systemd-pstore: 부팅마다 pstore 를 /var/lib/systemd/pstore 로 옮기고 **NVRAM 을 비운다**.
    #    이게 없으면 EFI 변수가 누적돼 결국 기록 실패한다. 2026-07-31 실측에서 시험 전후 EFI
    #    변수 164 개로 동일 — 아카이브가 정상 동작하면 누적이 없다.
    #   ★ B6 계열(2026-09-03 · 2차 수리): `|| true` 뒤 무조건 ✓ 였다. 여기서 rc 는 kdump 와 달리
    #     "정상적 미설치"를 뜻할 수 없다 — systemd-pstore 유닛이 없으면 NVRAM 이 비워지지 않아
    #     결국 포착이 실패하므로, 부재는 정상이 아니라 **이 설치가 달성하려던 것의 결손**이다.
    #     그래도 판정은 결과 상태(is-enabled)로 한다: 이미 enabled 인 노드에서 enable 이 비-0 을
    #     내는 경우가 있고, 그때 실패라 부르면 위양성이 된다.
    if systemctl enable systemd-pstore >/dev/null 2>&1; then _PS_RC=0; else _PS_RC=$?; fi
    _PS_EN="$(systemctl is-enabled systemd-pstore 2>/dev/null)"
    case "$_PS_EN" in
      enabled|enabled-runtime|static|indirect|generated)
        say "   ✓ systemd-pstore 아카이브 활성(is-enabled=$_PS_EN · NVRAM 누적 방지)" ;;
      "")
        say "   ✗ systemd-pstore 유닛을 찾을 수 없다(enable rc=$_PS_RC) — NVRAM 이 비워지지 않아"
        say "     EFI 변수가 누적되면 결국 패닉 로그 기록 자체가 실패한다(2026-07-31 근거)"
        FAIL=1 ;;
      *)
        say "   ✗ systemd-pstore 미활성(is-enabled=$_PS_EN · enable rc=$_PS_RC)"
        say "     확인: systemctl status systemd-pstore"
        FAIL=1 ;;
    esac
  fi

  # ④ GRUB: 우리 drop-in 을 **비우고**, kdump-tools 가 넣는 crashkernel 도 걷어낸다.
  say "   GRUB drop-in: (비움) — crashkernel·reserve_mem·ramoops.* 전부 제거"
  if [ "$APPLY" = 1 ]; then
    # ★ B7(2026-09-03): 예전엔 `cp -a … 2>/dev/null || true` 로 rc 와 stderr 를 **둘 다** 버린 뒤
    #   블록 끝에서 "백업=…" 을 조건 없이 찍었다. 백업이 없는데 있다고 말하는 것은, 되돌릴 수
    #   없는 GRUB 갱신을 되돌릴 수 있는 것처럼 보이게 하는 가장 나쁜 형태의 fail-open 이다.
    GRUB_BAK=/boot/grub/grub.cfg.easy-vllm-install-backup
    GRUB_OK=1
    if [ ! -f /boot/grub/grub.cfg ]; then
      say "   ✗ /boot/grub/grub.cfg 부재 — GRUB 갱신 전제가 성립하지 않는다"; FAIL=1; GRUB_OK=0
    elif cp -a /boot/grub/grub.cfg "$GRUB_BAK" && [ -s "$GRUB_BAK" ]; then
      say "   ✓ GRUB 백업 확보: $GRUB_BAK ($(stat -c %s "$GRUB_BAK" 2>/dev/null) bytes)"
    else
      say "   ✗ GRUB 백업 실패: $GRUB_BAK — 되돌릴 수단 없이 grub.cfg 를 갱신하지 않는다"
      FAIL=1; GRUB_OK=0
    fi
    KT=/etc/default/grub.d/kdump-tools.cfg
    [ -f "$KT" ] && sed -i 's/ *crashkernel=[^ "]*//g' "$KT"
    cat > /etc/default/grub.d/zz-easy-vllm-blackbox.cfg <<'EOF'
# easy-vllm 노드블랙박스 — 커널 파라미터 없음(의도적).
#   2026-07-31 크래시 시험 5회 결과 crashkernel/ramoops 는 제거가 정답이다. 이 파일은 "우리가
#   판단해서 비워 뒀다"는 표시로 남긴다 — 파일이 없으면 다음 사람이 "아직 설정 안 했나?" 로
#   오독하고 되돌릴 수 있다. 근거: testlog_26073113 · install_node_blackbox.sh 상단 주석.
EOF
    if [ "$GRUB_OK" != 1 ]; then
      say "   ⊘ GRUB 재생성 건너뜀(백업 미확보) — 백업을 확보한 뒤 재실행하라"
    else
      if command -v update-grub >/dev/null 2>&1; then update-grub >/dev/null 2>&1; _GR=$?
      else grub-mkconfig -o /boot/grub/grub.cfg >/dev/null 2>&1; _GR=$?; fi
      if [ "$_GR" -ne 0 ]; then
        say "   ✗ GRUB 재생성 실패(rc=$_GR) — 복원: cp -a $GRUB_BAK /boot/grub/grub.cfg"; FAIL=1
      elif [ ! -s /boot/grub/grub.cfg ]; then
        say "   ✗ 재생성 후 /boot/grub/grub.cfg 가 비었다 — 복원: cp -a $GRUB_BAK /boot/grub/grub.cfg"; FAIL=1
      else
        say "   ✓ GRUB 갱신 · 백업=$GRUB_BAK"
      fi
    fi
    rm -f /etc/modules-load.d/easy-vllm-ramoops.conf        # ramoops 자동로드 철회
  fi
fi

# ── 즉시 검증 (주장이 아니라 신선한 출력으로) ────────────────────────────
echo
say "═══ 설치 직후 검증 ═══"
if [ "$APPLY" = 1 ]; then
  for u in easy-vllm-blackbox-collect easy-vllm-blackbox-watchdog; do
    if systemctl is-active --quiet "$u"; then say "  ✓ $u active"; else say "  ✗ $u 비활성 — journalctl -u $u"; FAIL=1; fi
  done
  for t in easy-vllm-blackbox-events.timer easy-vllm-blackbox-lifecycle.timer; do
    if systemctl is-active --quiet "$t"; then say "  ✓ $t active"; else say "  ✗ $t 비활성"; FAIL=1; fi
  done
  [ -s "$ETC/eta_params.env" ] && say "  ✓ $ETC/eta_params.env" || { say "  ✗ eta_params.env 없음"; FAIL=1; }
  sleep 3
  TODAY_CSV="$NODE_DIR/samples/$(date -u +%F).csv"
  if [ -s "$TODAY_CSV" ]; then
    say "  ✓ 샘플 기록 확인: $(wc -l < "$TODAY_CSV") 줄 — $TODAY_CSV"
  else
    say "  ✗ 샘플 미기록 — journalctl -u easy-vllm-blackbox-collect"; FAIL=1
  fi
  # ★ B6(2026-09-03): 예전엔 예약 크기를 **출력만** 했다. 사람이 그 숫자를 읽고 판단하기를
  #   기대한 형태인데, 바로 아래에서 스크립트가 "INSTALL PASS" 를 찍어 버리므로 실제로는
  #   아무도 판단하지 않았다 — 숫자를 보여 주는 것은 판정이 아니다. `== 0` 으로 판정한다.
  #   예약이 남아 있으면 panic 이 kexec 로 점프해 efi_pstore 가 원천 차단되므로, 이 설치의
  #   목적(L3 = 사후 포착 확보)이 아직 달성되지 않은 상태다 = PASS 가 아니다.
  CKS_NOW="$(cat /sys/kernel/kexec_crash_size 2>/dev/null || echo '')"
  if [ -z "$CKS_NOW" ]; then
    say "  ✗ /sys/kernel/kexec_crash_size 를 읽지 못했다 — 예약 여부를 **판정할 수 없다**"; FAIL=1
  elif [ "$CKS_NOW" -eq 0 ] 2>/dev/null; then
    say "  ✓ kdump 예약 0 bytes (kexec_crash_size=0 — pstore 경로가 열려 있다)"
  else
    say "  ✗ kdump 예약 잔존: $CKS_NOW bytes (kexec_crash_size≠0 — panic 이 kexec 로 점프해 pstore 차단)"
    say "    L3 는 crashkernel 을 GRUB 에서 걷어냈으므로 **재부팅 1회** 뒤 0 이 되어야 한다."
    say "    재부팅 뒤 재판정: bash $SDIR/verify_node_blackbox.sh --check"
    FAIL=1
  fi
  say "     ⚠ /proc/iomem grep 로 예약을 판정하지 말 것 — kptr_restrict 로 전 항목이 0 으로 보인다(위음성 함정)"
  echo
  if [ "$LEVEL" = "L3" ]; then
    say "★ 다음 단계(사람): 재부팅 전 아래 확인 → 재부팅 → verify_node_blackbox.sh"
    # ★ sudo 필수 — /boot/grub/grub.cfg 는 root 전용이다. 비-root 로 돌리면 Permission denied 가
    #   나는데, 2>/dev/null 과 조합되면 **빈 출력을 '부재'로 오독**한다(2026-07-31 실전 교훈:
    #   읽지 못한 것을 없는 것으로 보고했다). kptr_restrict 위음성과 같은 계열의 함정이다.
    say "   sudo grep -c 'crashkernel=\\|reserve_mem=\\|ramoops\\.' /boot/grub/grub.cfg    # **0 이어야 정상**"
    say "   (sudo 없이 돌리지 말 것 — Permission denied 를 '제거됨'으로 오독한다)"
    say "   재부팅 후 기대: kexec_crash_size=0 · pstore backend=efi_pstore · MemTotal 약 +2.25 GiB"
  else
    say "★ 다음 단계: bash $SDIR/verify_node_blackbox.sh"
  fi
  [ "$FAIL" = 0 ] && say "INSTALL PASS ($LEVEL)" || say "INSTALL FAIL — 위 ✗ 확인"
  exit "$FAIL"
else
  echo
  say "DRY-RUN 종료. 실제 설치: sudo bash $0 --apply --level=$LEVEL"
  exit 0
fi
