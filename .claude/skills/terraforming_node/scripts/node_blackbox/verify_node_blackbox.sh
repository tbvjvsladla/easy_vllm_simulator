#!/bin/bash
# verify_node_blackbox.sh — proof-of-capture 게이트 (plan_26073109 §Phase 5).
#
#   이 스크립트의 존재 이유: 2026-07-30 두 번의 하드다운에서 kdump 는 양 노드 모두
#   `current state: ready to kdump` 였는데 **vmcore 는 0건**이었다. "ready" 는 주장이지
#   증거가 아니었다. superpowers `verification-before-completion` 의 게이트 함수를 그대로 적용한다:
#       주장 → 증명할 명령 특정 → 신선하게 완전 실행 → 전체 출력·exit code 확인 → 그제서야 주장.
#
#   ∴ 상태 권위를 `installed: true` 에서 **`capture_verified: <UTC>`** 로 바꾼다.
#
#   모드:
#     --check        비파괴 전수 검사(기본). 크래시가 필요한 항목은 pending 으로 남긴다.
#     --crash-test   ★파괴적★ 강제 커널 패닉 → 재부팅. 사후 포착을 증명하는 유일한 방법.
#     --post-crash   재부팅 후 efi_pstore 실착지 확인 → capture_verified 확정.
#
#   ★ 2026-07-31 크래시 시험 5회로 이 스크립트의 합격 기준이 역전됐다(testlog_26073113):
#     kdump 는 **제거 대상**이고(무장 시 pstore 를 원천 차단 + 2.25 GiB 예약), 사후 포착의
#     정본은 멀티=netconsole(4/4) · 싱글=efi_pstore(1/1) 다. ramoops 는 이 플랫폼에서 불가.
#
# 종료코드: 0=전항 통과 · 2=검사 실패 · 3=크래시 시험 거부(전제 미충족) · 1=인자/전제 오류.
set -uo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# ★ 리포 루트는 **고정 상대깊이로 세지 않는다**. 메인에서는 이 스크립트가
#   .claude/skills/terraforming_node/scripts/node_blackbox/ 에 있지만, 서브에는 런타임으로
#   .claude/runtime/node_blackbox/ 로 배달된다(host_safety 선례). 깊이가 다르므로 ../../../../..
#   는 서브에서 /home/cona 를 가리켜 로그 루트가 조용히 엉뚱한 곳이 된다. 마커로 찾는다.
_find_repo(){ local d="$1"; while [ "$d" != "/" ] && [ -n "$d" ]; do
    [ -d "$d/.claude" ] && [ -d "$d/docs" ] && { printf '%s' "$d"; return 0; }; d="$(dirname "$d")"; done; return 1; }
REPO="$(_find_repo "$SDIR" || (cd "$SDIR/../../../../.." 2>/dev/null && pwd))"
MODE="check"; NODE_ID="$(hostname)"; LOGS_ROOT=""; PEER_IP=""; CONFIRM=""
for a in "$@"; do
  case "$a" in
    --check) MODE="check" ;;
    --crash-test) MODE="crash" ;;
    --post-crash) MODE="post" ;;
    --node-id=*) NODE_ID="${a#--node-id=}" ;;
    --logs-root=*) LOGS_ROOT="${a#--logs-root=}" ;;
    --peer=*) PEER_IP="${a#--peer=}" ;;
    --confirm=*) CONFIRM="${a#--confirm=}" ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "[bb-verify] 알 수 없는 인자: $a" >&2; exit 1 ;;
  esac
done
[ -n "$LOGS_ROOT" ] || LOGS_ROOT="$REPO/docs/logs"
NODE_DIR="$LOGS_ROOT/$NODE_ID"
ETC=/etc/easy-vllm
PASS=0; FAILN=0; PEND=0
RESULTS=""

say(){ echo "[bb-verify] $*"; }
ok(){   PASS=$((PASS+1));  echo "  ✓ $1"; RESULTS="$RESULTS{\"check\":\"$2\",\"verdict\":\"pass\"},"; }
bad(){  FAILN=$((FAILN+1)); echo "  ✗ $1"; RESULTS="$RESULTS{\"check\":\"$2\",\"verdict\":\"fail\"},"; }
pend(){ PEND=$((PEND+1));  echo "  … $1"; RESULTS="$RESULTS{\"check\":\"$2\",\"verdict\":\"pending\"},"; }
na(){   echo "  – $1 (해당 없음)"; RESULTS="$RESULTS{\"check\":\"$2\",\"verdict\":\"n/a\"},"; }

# ── 크래시 시험 (파괴적 — 이중 게이트) ───────────────────────────────────
if [ "$MODE" = "crash" ]; then
  say "★★ 파괴적 시험: 강제 커널 패닉을 일으켜 사후 포착(efi_pstore)의 실동작을 증명한다."
  say "   이 노드는 즉시 죽고 재부팅된다. SSH 도 끊긴다."
  [ "$(id -u)" -eq 0 ] || { say "FAIL: root 필요(sudo)"; exit 1; }
  # 게이트 1: 서빙 중이면 거부 — 남의 작업을 죽이는 시험이 되어선 안 된다
  # ★ pipefail 하에서 `producer | grep -q` 는 금지다. grep -q 는 첫 매칭에서 즉시 끝나며,
  #   그때 상류가 아직 쓰고 있으면 SIGPIPE(141) 를 받고 pipefail 이 그 141 을 파이프라인
  #   상태로 채택한다 → **매칭했는데 if 가 거짓**이 된다. 심지어 타이밍 의존이라 간헐적이다.
  #   이 프로젝트는 2026-07-30 install_netconsole.sh 에서 이미 같은 함정을 겪었고,
  #   2026-07-31 이 검증자의 pstore 내용검사가 정확히 이 이유로 위음성을 냈다.
  #   여기는 특히 치명적이다 — 위음성이면 **서빙 중인 노드를 죽인다**. 파이프를 없앤다.
  _dps="$(docker ps --format '{{.Names}}' 2>/dev/null)"
  if grep -qi vllm <<< "$_dps"; then
    say "거부: vLLM 컨테이너가 실행 중이다. 먼저 serve 를 내려라."; exit 3
  fi
  # 게이트 2: 포착 수단이 하나도 없으면 시험 자체가 무의미.
  #   2026-07-31 교정: 예전 게이트는 kdump 적재를 필수로 요구했으나, 같은 날 4회 실측에서
  #   kdump 크래시커널 부팅이 1/4 로 간헐이고 vmcore 는 makedumpfile 이 커널 6.17 을
  #   미지원해 구조적으로 불가함이 확인됐다. 반대로 pstore 는 kdump 가 *적재돼 있으면*
  #   panic() 이 kmsg_dump 보다 먼저 kexec 로 점프해(crash_kexec_post_notifiers=N)
  #   원리적으로 기록될 수 없다 — 즉 pstore 검증은 kdump 를 내린 상태에서만 가능하다.
  #   따라서 "kdump 적재" 또는 "pstore 백엔드 등록" 중 하나만 있으면 시험은 유의미하다.
  CKS="$(cat /sys/kernel/kexec_crash_size 2>/dev/null || echo 0)"
  CKL="$(cat /sys/kernel/kexec_crash_loaded 2>/dev/null || echo 0)"
  PSB="$(cat /sys/module/pstore/parameters/backend 2>/dev/null || echo '')"
  case "$PSB" in ''|'(none)'|'null') PSB_OK=0 ;; *) PSB_OK=1 ;; esac
  if { [ "${CKS:-0}" -le 0 ] || [ "$CKL" != "1" ]; } && [ "$PSB_OK" -ne 1 ]; then
    say "거부: 포착 수단 부재(kexec_crash_size=$CKS loaded=$CKL pstore_backend='${PSB:-none}')."
    say "      kdump 적재 또는 pstore 백엔드 등록 중 최소 하나가 필요하다."; exit 3
  fi
  say "포착 수단: kdump_loaded=$CKL · pstore_backend='${PSB:-none}'"
  # 게이트 3: 명시 확인 문자열
  if [ "$CONFIRM" != "CRASH-$NODE_ID" ]; then
    say "거부: 확인 문자열 불일치. 정말 실행하려면 --confirm=CRASH-$NODE_ID 를 붙여라."
    say "      (kexec_crash_size=$CKS · loaded=$CKL · 서빙 컨테이너 없음 = 나머지 전제는 충족)"
    exit 3
  fi
  say "전제 충족. 마커를 남기고 3초 후 패닉한다."
  mkdir -p "$NODE_DIR"
  date -u +%Y-%m-%dT%H:%M:%SZ > "$NODE_DIR/.crash_test_marker"
  sync
  echo "easy-vllm-bb CRASH-TEST marker $(date -u +%FT%TZ)" > /dev/kmsg 2>/dev/null || true
  sleep 3
  echo 1 > /proc/sys/kernel/sysrq
  echo c > /proc/sysrq-trigger      # 여기서 노드가 죽는다
  exit 0
fi

say "노드=$NODE_ID · 모드=$MODE · 로그=$NODE_DIR"
echo

# ── A. 결정론 엔진 자체시험 (하드웨어 불요) ──────────────────────────────
say "A. 결정론 엔진"
for t in "blackbox_eta.py --self-test:ETA 엔진" \
         "blackbox_collect.py --self-test:수집기" \
         "blackbox_events.py --self-test:이벤트 통합" \
         "logs_lifecycle.py --self-test:수명 집행" \
         "seed_from_journal.py --self-test:시드 임포터" \
         "blackbox_session.py --self-test:세션 사이드카"; do
  f="${t%%:*}"; label="${t##*:}"
  if python3 "$SDIR/${f%% *}" ${f#* } >/dev/null 2>&1; then ok "$label self-test" "selftest_${f%%.*}"
  else bad "$label self-test 실패 — python3 $SDIR/$f" "selftest_${f%%.*}"; fi
done
if bash "$SDIR/mem_watchdog_eta.sh" --self-test >/dev/null 2>&1; then
  ok "ETA 워치독 self-test" "selftest_watchdog"
else bad "ETA 워치독 self-test 실패" "selftest_watchdog"; fi

# ★ 배포본 신선도 — self-test 는 **소스**를 시험한다. 데몬이 실행하는 것은 $BIN 의 사본이고,
#   둘이 갈라져 있으면 "시험 통과 + 현장은 옛 코드"가 된다. 침묵 실패라 반드시 명시 검사한다.
#   (근거: install 의 `enable --now` 가 이미 돌던 유닛을 재시작하지 않던 결함 — testlog_26073123)
for pair in "mem_watchdog_eta.sh:easy-vllm-bb-watchdog" \
            "blackbox_collect.py:easy-vllm-bb-collect" \
            "blackbox_eta.py:easy-vllm-bb-eta"; do
  src="$SDIR/${pair%%:*}"; dst="/usr/local/sbin/${pair##*:}"
  if [ ! -f "$dst" ]; then bad "배포본 부재: $dst" "deployed_${pair##*:}"
  elif [ "$(sha256sum <"$src" | cut -d' ' -f1)" = "$(sha256sum <"$dst" | cut -d' ' -f1)" ]; then
    ok "배포본 최신 ${pair##*:}" "deployed_${pair##*:}"
  else
    bad "배포본 구버전 ${pair##*:} — 소스≠$dst. sudo bash $SDIR/install_node_blackbox.sh --apply --level L1" \
        "deployed_${pair##*:}"
  fi
done

# ── B. L1 런타임 ─────────────────────────────────────────────────────────
echo; say "B. L1 런타임 (무재부팅 계층)"
for u in easy-vllm-blackbox-collect easy-vllm-blackbox-watchdog; do
  if systemctl is-active --quiet "$u" 2>/dev/null; then ok "$u active" "unit_$u"
  else bad "$u 비활성 — journalctl -u $u" "unit_$u"; fi
done
for t in easy-vllm-blackbox-events.timer easy-vllm-blackbox-lifecycle.timer; do
  if systemctl is-active --quiet "$t" 2>/dev/null; then ok "$t active" "timer_$t"
  else bad "$t 비활성" "timer_$t"; fi
done
if systemctl is-active --quiet earlyoom 2>/dev/null; then ok "earlyoom active" "earlyoom"
else bad "earlyoom 비활성" "earlyoom"; fi

if [ -s "$ETC/eta_params.env" ]; then
  ok "ETA 상수 존재($ETC/eta_params.env)" "eta_params"
  # 상수가 하한 가드를 통과하는가 — 파일이 있다고 유효한 건 아니다
  # shellcheck disable=SC1090
  ( . "$ETC/eta_params.env"; python3 "$SDIR/blackbox_eta.py" \
      --set kill_latency_s="${BB_KILL_LATENCY_S:-4}" \
      --set detect_margin_s="${BB_DETECT_MARGIN_S:-2}" \
      --set debounce_polls="${BB_DEBOUNCE_POLLS:-3}" --explain 10000 100 >/dev/null 2>&1 ) \
    && ok "ETA 상수가 하한 가드 통과" "eta_guard" \
    || bad "ETA 상수가 하한 가드 위반 — 재생성 필요" "eta_guard"
else
  bad "ETA 상수 부재 — 워치독이 내장 기본값으로 동작 중" "eta_params"
fi

# ★ 샘플이 '실제로 자라는가' — 파일 존재가 아니라 증분을 본다
TODAY="$NODE_DIR/samples/$(date -u +%F).csv"
if [ -f "$TODAY" ]; then
  b1=$(wc -c < "$TODAY"); sleep 4; b2=$(wc -c < "$TODAY")
  if [ "$b2" -gt "$b1" ]; then ok "샘플 증가 확인(4초에 $((b2-b1))B — 배칭 없음)" "sample_growth"
  else bad "샘플이 자라지 않음(수집기 정지 의심)" "sample_growth"; fi
else bad "오늘 샘플 파일 없음: $TODAY" "sample_growth"; fi

# 좀비 협역 워치독 — 재발 감시
# pgrep -c 는 매칭 0 일 때 "0" 을 출력하면서 **exit 1** 을 낸다 → `|| echo 0` 을 붙이면
# "0\n0" 이 되어 산술·표시가 깨진다(실기 실행이 잡은 결함). 출력 첫 줄만 취한다.
Z=$(pgrep -fc '[m]em_watchdog\.sh' 2>/dev/null | head -1); Z=${Z:-0}
if [ "$Z" = "0" ]; then ok "레거시 협역 워치독 좀비 0" "no_zombie"
else bad "레거시 협역 워치독 $Z 개 잔존 — PID 기반으로 정리 필요" "no_zombie"; fi

# ── C. 오프박스 경로 (kmsg → netconsole/ramoops) ─────────────────────────
echo; say "C. 오프박스 경로"
if [ "$(id -u)" -eq 0 ]; then
  MARK="easy-vllm-bb VERIFY $(date -u +%FT%TZ) $$"
  if echo "$MARK" > /dev/kmsg 2>/dev/null; then
    sleep 1
    _dm="$(dmesg 2>/dev/null | tail -50)"      # 파이프에서 grep -q 를 떼어낸다(위 §게이트1 주석)
    if grep -qF "$MARK" <<< "$_dm"; then
      ok "kmsg 기록→커널 링버퍼 도달" "kmsg_write"
    else bad "kmsg 기록은 됐으나 링버퍼에서 확인 실패" "kmsg_write"; fi
  else bad "/dev/kmsg 기록 불가" "kmsg_write"; fi
else
  pend "kmsg 검사는 root 필요 — sudo 로 재실행" "kmsg_write"
fi

if grep -q '^netconsole ' /proc/modules 2>/dev/null; then
  ok "netconsole 모듈 로드됨" "netconsole_mod"
  # ★ 예전엔 여기서 "상대 노드에서 직접 확인하라"며 **영원히 pending** 으로 남겼다. 그건
  #   증명 불가가 아니라 구현을 안 한 것이었다 — proof-of-capture 게이트가 스스로 닫지 못하는
  #   항목을 갖고 있으면 "검증됨"의 의미가 흐려진다(2026-07-31 교정).
  #   ∴ 실제 왕복을 시험한다: 고유 토큰을 /dev/kmsg 에 쓰고 → netconsole 이 peer 로 실어 나르고
  #     → peer 의 rsyslog 수신 파일에서 그 토큰을 되찾는다. 이게 오프박스 경로의 E2E 증명이다.
  if [ -z "$PEER_IP" ]; then
    pend "peer 미지정 — --peer=<IP> 를 주면 kmsg 왕복을 실제로 시험한다" "netconsole_peer"
  elif [ "$(id -u)" -ne 0 ]; then
    pend "peer 왕복 시험은 root 필요(/dev/kmsg 쓰기) — sudo 로 재실행" "netconsole_peer"
  else
    # peer 로 나가는 경로의 자기 주소 = 수신측 파일명. 환경 하드코딩 금지.
    _self_ip="$(ip route get "$PEER_IP" 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1);exit}}')"
    # sudo 로 돌면 ssh 키는 root 가 아니라 **호출자**의 것이다. 호출자로 낮춰서 접속한다.
    _pu="${SUDO_USER:-$(id -un)}"
    if [ -z "$_self_ip" ]; then
      pend "peer 경로의 자기 IP 해소 실패 — 왕복 시험 생략" "netconsole_peer"
    else
      # 토큰에 공백을 넣지 않는다 — ssh 를 거치며 인용이 깨지는 사고를 원천 차단.
      _tok="easy-vllm-bb-peertest-$(date -u +%s)-$$"
      echo "$_tok" > /dev/kmsg 2>/dev/null
      sleep 3
      _rl="/var/log/remote-kmsg-${_self_ip}.log"
      _hit="$(sudo -u "$_pu" -n timeout 12 ssh -o BatchMode=yes -o ConnectTimeout=5 \
                "$_pu@$PEER_IP" "grep -c $_tok $_rl 2>/dev/null | head -1" 2>/dev/null | head -1)"
      case "${_hit:-0}" in
        ''|0) bad "kmsg 가 peer($PEER_IP:$_rl)에 도달하지 않음 — 오프박스 경로 단절" "netconsole_peer" ;;
        *)    ok "kmsg 왕복 확인: 토큰이 peer 수신파일에 도착($_hit 건) — 오프박스 경로 실동작" "netconsole_peer" ;;
      esac
    fi
  fi
elif [ -f "$REPO/output/multi/manifest.yaml" ]; then
  bad "multi 노드인데 netconsole 미로드" "netconsole_mod"
else
  na "netconsole (단일노드 — peer 부재, efi_pstore 가 사후 포착 담당)" "netconsole_mod"
fi

# ── D. L3 사후 포착 = efi_pstore (증명은 --crash-test 로만) ───────────────
#   ★ 이 절의 합격 조건은 2026-07-31 크래시 시험 5회로 **역전**됐다(testlog_26073113).
#     예전: "kdump 예약됨 = 좋음". 지금: "kdump 예약됨 = **결함**".
#     이유는 하나다 — crash_kexec_post_notifiers=N 인 리눅스 panic() 은 kmsg_dump 보다 먼저
#     kexec 로 점프해 돌아오지 않는다. 즉 kdump 가 무장돼 있으면 pstore 는 원리적으로 못 쓴다.
#     여기에 kdump 는 vmcore 0/4(makedumpfile 6.17 미지원) · 크래시커널 부팅 1/4 이면서
#     2.25 GiB 를 상시 예약한다 — 통합메모리 노드에서 그 자원이 곧 하드다운의 원인이다.
echo; say "D. L3 사후 포착 준비상태 (efi_pstore)"
CKS="$(cat /sys/kernel/kexec_crash_size 2>/dev/null || echo 0)"
CKL="$(cat /sys/kernel/kexec_crash_loaded 2>/dev/null || echo 0)"
if [ "${CKS:-0}" -eq 0 ] && [ "$CKL" != "1" ]; then
  ok "crashkernel 미예약 · kexec 미적재 — pstore 경로 확보(+약 2.25 GiB 가용)" "kdump_disarmed"
else
  bad "kdump 무장 중(size=$CKS loaded=$CKL) — kexec 가 kmsg_dump 를 선점해 pstore 가 원천 차단된다" "kdump_disarmed"
  say "     → install_node_blackbox.sh --apply --level=L3 후 재부팅"
fi
say "   ⚠ /proc/iomem grep 으로 예약을 판정하지 말 것 — kptr_restrict=1 에서 전 항목이"
say "     00000000-00000000 으로 보인다(Crash kernel 뿐 아니라 GICD·NVDA 도). 위음성 함정이다."

# ★ ramoops 는 이 플랫폼에서 **금지**다. 예약 문법은 되지만 잔존이 안 된다:
#   같은 물리주소(0x1f68f8c000)로 3회 부팅 · 마커 기록 후 **정상** 재부팅 → 'error in header'.
#   파라미터가 남아 있으면 다음 사람이 "포착 수단이 있다"고 오독하므로 결함으로 잡는다.
if grep -q 'ramoops\.\|reserve_mem=' /proc/cmdline 2>/dev/null; then
  bad "ramoops/reserve_mem 파라미터 잔존 — 이 플랫폼은 리셋 때 DRAM 이 초기화된다(직접 반증됨)" "ramoops_absent"
else
  ok "ramoops/reserve_mem 파라미터 없음(의도된 제거)" "ramoops_absent"
fi

# pstore 백엔드 — efi_pstore 만이 리셋·전원차단을 견딘다(NVRAM).
BE="$(cat /sys/module/pstore/parameters/backend 2>/dev/null)"
case "$BE" in
  efi_pstore) ok "pstore 활성 백엔드 = efi_pstore(NVRAM — 리셋 생존)" "pstore_backend" ;;
  ramoops)    bad "백엔드가 ramoops — 이 플랫폼에서는 리셋 때 내용이 소멸한다" "pstore_backend" ;;
  ""|"(none)") bad "pstore 백엔드 미등록 — 사후 포착 수단이 없다" "pstore_backend" ;;
  *)          pend "pstore 백엔드 '$BE' — 미검증 백엔드" "pstore_backend" ;;
esac
if grep -q pstore /proc/mounts 2>/dev/null; then ok "pstore 마운트됨" "pstore_mount"
else bad "pstore 미마운트 — 기록돼도 회수 경로가 없다" "pstore_mount"; fi

# ★★ systemd-pstore 는 부팅마다 pstore 를 아카이브하고 **NVRAM 을 비운다**. 이게 없으면
#    EFI 변수가 누적돼 결국 기록에 실패한다. 그리고 이 이동 때문에 /sys/fs/pstore 는
#    평시 **항상 비어 있다** — 여기만 보고 "포착 실패"로 오독한 전례가 있다(2026-07-31).
if systemctl is-enabled systemd-pstore >/dev/null 2>&1; then
  ok "systemd-pstore 아카이브 활성(NVRAM 누적 방지)" "pstore_archive"
else
  bad "systemd-pstore 비활성 — EFI 변수가 누적돼 결국 기록 실패한다" "pstore_archive"
fi
PSA=/var/lib/systemd/pstore
NA=$(find "$PSA" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l)
if [ "${NA:-0}" -gt 0 ]; then
  ok "사후 포착 실적 $NA 건 — $PSA (평시 /sys/fs/pstore 가 비어있는 것이 정상)" "pstore_records"
else
  pend "포착 실적 0 — 아직 크래시가 없었거나 미검증(--crash-test 로 증명)" "pstore_records"
fi

# hang→panic 승격: 무음 hang 은 어떤 사후 경로도 타지 못한다. 승격돼야 efi_pstore 가 잡는다.
HP="$(cat /proc/sys/kernel/hung_task_panic 2>/dev/null || echo 0)"
SP="$(cat /proc/sys/kernel/softlockup_panic 2>/dev/null || echo 0)"
if [ "$HP" = "1" ] && [ "$SP" = "1" ]; then
  ok "hang/softlockup → panic 승격 활성(무음 hang 도 pstore 가 잡는다)" "panic_promote"
else
  bad "panic 승격 비활성(hung_task_panic=$HP softlockup_panic=$SP) — 무음 hang 은 무로그" "panic_promote"
fi

# ── E. post-crash: 실착지 확인 → capture_verified 확정 ───────────────────
if [ "$MODE" = "post" ]; then
  echo; say "E. 크래시 후 실착지 확인"
  if [ -f "$NODE_DIR/.crash_test_marker" ]; then
    ok "크래시 시험 마커 확인($(cat "$NODE_DIR/.crash_test_marker"))" "crash_marker"
  else
    pend "크래시 시험 마커 없음 — 자연 발생 크래시 후 검사인가?" "crash_marker"
  fi
  # ★★ /sys/fs/pstore 만 보고 판정하지 말 것 — **평시 항상 비어 있다**.
  #    systemd-pstore.service 가 부팅 직후 내용을 /var/lib/systemd/pstore/ 로 **옮기고**
  #    NVRAM 을 비우기 때문이다. 2026-07-31 에 내가 정확히 이 함정에 빠져 실제로 포착된
  #    기록을 4회 연속 "비어있음 = 실패"로 오독했다. 정본 경로는 아카이브다.
  PSA=/var/lib/systemd/pstore
  MARK="$NODE_DIR/.crash_test_marker"
  # 이번 크래시의 산출물만 센다 — 과거 아카이브를 성공으로 오계상하지 않기 위해 마커보다 새 것만.
  if [ -f "$MARK" ]; then
    FRESH=$(find "$PSA" -mindepth 1 -maxdepth 1 -type d -newer "$MARK" 2>/dev/null | sort)
  else
    FRESH=$(find "$PSA" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort | tail -2)
  fi
  NREC=$(printf '%s\n' "$FRESH" | grep -c . )
  if [ -n "$FRESH" ] && [ "$NREC" -gt 0 ]; then
    NF=$(printf '%s\n' "$FRESH" | while read -r d; do [ -n "$d" ] && find "$d" -type f; done 2>/dev/null | wc -l)
    ok "efi_pstore 포착: 아카이브 $NREC 뭉치 · 레코드 $NF 개 — $PSA" "pstore_landed"
    # 내용 확증 — 개수는 포착의 증거가 아니다. 레코드는 0600(root) 이므로 root 일 때만 확인 가능.
    if [ "$(id -u)" -eq 0 ]; then
      # 여기는 수십 KB 를 흘리므로 grep -q 조기종료 → 상류 SIGPIPE → pipefail 위음성이
      # 사실상 확정적이다(2026-07-31 실측: 포착 완벽한데 FAIL 판정). 임시파일로 분리한다.
      _pc="$(mktemp)"
      printf '%s\n' "$FRESH" | while read -r d; do [ -n "$d" ] && cat "$d"/*/* 2>/dev/null; done > "$_pc" 2>/dev/null
      if grep -q 'Kernel panic' "$_pc"; then
        ok "레코드 내용에 'Kernel panic' 확인(개수가 아니라 내용으로 확증)" "pstore_content"
      else
        bad "레코드는 있으나 'Kernel panic' 문자열 없음 — 다른 사건이거나 절단됐다" "pstore_content"
      fi
      rm -f "$_pc"
    else
      pend "레코드 내용 확인 생략(0600 — sudo 로 재실행하면 확증한다)" "pstore_content"
    fi
  else
    bad "efi_pstore 포착 0 — 패닉 로그가 NVRAM 에 남지 않았다" "pstore_landed"
    say "     확인: kdump 가 무장돼 있으면(kexec_crash_size>0) kmsg_dump 가 아예 실행되지 않는다"
  fi
  # kdump 산출물은 **기대하지 않는다**(설계상 제거). 남아 있으면 이전 구성의 잔재이므로 알린다.
  if [ -n "$(find /var/crash -maxdepth 1 -type d -name '20*' 2>/dev/null | head -1)" ]; then
    say "   참고: /var/crash 에 과거 kdump 산출물이 남아 있다(현 설계에서는 미사용)"
  fi
fi

# ── 판정 기록 ────────────────────────────────────────────────────────────
echo
say "═══ 판정: 통과 $PASS · 실패 $FAILN · 보류 $PEND ═══"
NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [ -d "$NODE_DIR" ]; then
  OUT="$NODE_DIR/capture_verified.json"
  # ★★ 포착 증명(`capture_proven`)은 **--post-crash 만** 쓸 수 있다.
  #   2026-07-31 실측 결함: 비파괴 `--check` 가 fail=0 이라는 이유로 verified_utc 를 새로 찍고
  #   기존 post-crash 증명(28/0/0)을 23/0/2 로 **덮어썼다**. --check 는 준비상태만 보므로
  #   포착을 증명할 수 없는데 증명 도장을 찍은 것이다 — 이 스크립트가 존재하는 이유였던
  #   `ready to kdump`(주장≠증거)를 그대로 재현한 셈이다.
  #   ∴ check/crash 실행은 증명을 **이월만** 하고 절대 생성·삭제하지 않는다.
  PREV_PROVEN="null"
  if [ -f "$OUT" ]; then
    PREV_PROVEN="$(python3 - "$OUT" <<'PY' 2>/dev/null || echo null
import json,sys
try:
    d=json.load(open(sys.argv[1],encoding="utf-8"))
except Exception:
    print("null"); raise SystemExit
p=d.get("capture_proven")
if not isinstance(p,dict):                      # schema_version 1 에서의 이월
    p=({"proven_utc":d.get("verified_utc"),"pass":d.get("pass"),"fail":d.get("fail"),
        "pending":d.get("pending")} if d.get("mode")=="post" and d.get("verified_utc") else None)
print(json.dumps(p,ensure_ascii=False) if p else "null")
PY
)"
  fi
  if [ "$MODE" = "post" ] && [ "$FAILN" = 0 ]; then
    PROVEN="$(printf '{"proven_utc": "%s", "pass": %d, "fail": %d, "pending": %d}' \
              "$NOW" "$PASS" "$FAILN" "$PEND")"
  else
    PROVEN="$PREV_PROVEN"
  fi
  {
    printf '{\n  "schema_version": 2,\n  "node_id": "%s",\n  "mode": "%s",\n' "$NODE_ID" "$MODE"
    printf '  "checked_utc": "%s",\n' "$NOW"
    printf '  "capture_proven": %s,\n' "$PROVEN"
    printf '  "pass": %d, "fail": %d, "pending": %d,\n' "$PASS" "$FAILN" "$PEND"
    printf '  "checks": [%s]\n}\n' "${RESULTS%,}"
  } > "$OUT"
  say "기록: $OUT"
  if [ "$PROVEN" = "null" ]; then
    say "★ capture_proven = null — 포착이 아직 증명되지 않았다(--crash-test → --post-crash 필요)."
  elif [ "$MODE" != "post" ]; then
    say "   capture_proven 은 이전 --post-crash 결과를 이월했다(이번 실행은 증명이 아니다)."
  fi
  if [ "$FAILN" != 0 ]; then
    say "★ 이번 실행에 실패가 있다 — 준비상태가 깨졌다는 뜻이다(증명 이력과 별개)."
  fi
fi

if [ "$PEND" -gt 0 ] && [ "$FAILN" = 0 ]; then
  echo
  say "보류 항목이 남았다. 사후 포착(efi_pstore)은 **강제 크래시 없이는 증명 불가**다:"
  say "   sudo bash $0 --crash-test --confirm=CRASH-$NODE_ID     # 노드가 죽고 재부팅됨"
  say "   (재부팅 후)  sudo bash $0 --post-crash"
fi
[ "$FAILN" = 0 ] && exit 0 || exit 2
