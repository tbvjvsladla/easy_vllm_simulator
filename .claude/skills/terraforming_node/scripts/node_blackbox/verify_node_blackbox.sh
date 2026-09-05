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
#   는 서브에서 홈 디렉터리(저장소 루트 바깥)를 가리켜 로그 루트가 조용히 엉뚱한 곳이 된다. 마커로 찾는다.
_find_repo(){ local d="$1"; while [ "$d" != "/" ] && [ -n "$d" ]; do
    [ -d "$d/.claude" ] && [ -d "$d/docs" ] && { printf '%s' "$d"; return 0; }; d="$(dirname "$d")"; done; return 1; }
REPO="$(_find_repo "$SDIR" || (cd "$SDIR/../../../../.." 2>/dev/null && pwd))"
# node_id 해소는 단일 소유다(plan_26081514 §4.2 · SKILL.md §2.7.6). 각자 파싱 금지.
[ -f "$SDIR/node_identity.sh" ] || {
  echo "[bb-verify] FAIL: $SDIR/node_identity.sh 부재 — node_id 해소기가 배달되지 않았다." >&2; exit 1; }
# shellcheck source=node_identity.sh
. "$SDIR/node_identity.sh"
# ★ NODE_ID 기본값 없음(스킴 R) — 옛 `$(hostname)` 은 침묵 폴백이었다. 해소는 인자 파싱 뒤.
MODE="check"; NODE_ID=""; LOGS_ROOT=""; PEER_IP=""; CONFIRM=""
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
NODE_ID="$(ni_resolve_node_id "$REPO" "$NODE_ID")" || exit 1
NODE_DIR="$LOGS_ROOT/$NODE_ID"
# ── 파괴적 확인 토큰만 hostname 을 **의도적으로** 남긴다 (plan_26081514 §3.4 권고 · G2) ──
#   경로·기록은 role 이지만, 이 토큰은 사람이 대화형으로 타이핑하는 **휘발성 CLI 입력**이라
#   파일·문서에 적히지 않는다 → PII 스캔 평면 밖이다. 반대로 role 로 바꾸면 모든 노드에서
#   `CRASH-main`/`CRASH-sub` 로 동형이 되어 **엉뚱한 노드에 복붙 실행**하는 사고를 막지 못한다.
#   근거 없는 잔존은 다음 사람이 지우므로, 이 주석이 그 근거다. 균일성을 택하려면 G2 를 뒤집어라.
CRASH_TOKEN="CRASH-$(hostname)"
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
  # ★ 2026-09-01 (audit ⑤): 위 주석은 위음성의 위험을 정확히 서술하고 파이프 하나를
  #   없앴는데, **같은 줄의 `2>/dev/null` 이 동일한 위음성을 낸다.** docker 데몬이 죽어
  #   있거나 권한이 없으면 `_dps` 가 빈 문자열이 되고, grep 이 매칭하지 않아 게이트가
  #   **열린 채** 강제 커널 패닉으로 진행한다. 즉 "docker 를 못 물어봤다"가 "서빙 안 한다"로
  #   접힌다 — 부재와 판단 불가의 융합. 여기서 그 오판의 대가는 **남의 서빙이 도는 노드의
  #   즉사**다. rc 를 본다.
  # ★ B11(2026-09-03 · audit_26090121): 위 rc 검사는 "docker 를 못 물어봤다"를 닫았지만
  #   **매칭 술어 자체가 반쪽**이었다 — `{{.Names}}` 만 보면 이름에 'vllm' 이 없는 서빙
  #   컨테이너를 놓친다. 이 저장소의 실측 반례: `MASTER_CONTAINER_NAME=mn-hy3-master` ·
  #   `mn-exaone45-33b-master`(.env.hy3 · .env.exaone45-33b) — 둘 다 'vllm' 미포함이라
  #   옛 게이트를 **그대로 통과**한다. 그 대가는 남의 서빙이 도는 노드의 강제 커널 패닉이다.
  #   판정은 running 으로 한정하고 ID·**이미지**·이름 세 필드를 훑는다. 이미지에는
  #   vllm 이 들어가므로(easy-vllm*·vllm/vllm-openai 등) 이름 규약과 무관하게 잡힌다.
  # ★ 2026-09-04(CP0 · plan_26090415 §3.3): 이 게이트는 워치독의 `targets()` 를 더 이상
  #   재현하지 않는다 — **의도적 분기이며 침묵 분기가 아니다.** 워치독은 좁혔고 여기는 넓게
  #   둔다. 방향이 반대이기 때문이다:
  #     워치독  — 매칭하면 `docker kill` 한다. 과잉 매칭의 대가 = **측정 도구 동반 사살**.
  #     이 게이트 — 매칭하면 강제 커널 패닉을 **거부**한다. 과잉 매칭의 대가 = 시험 연기뿐.
  #   즉 여기서는 넓은 술어가 fail-closed 다. 벤치 컨테이너가 돌고 있을 때 노드를 패닉시키지
  #   않는 것도 옳다. 두 술어를 억지로 같게 두면 한쪽이 반드시 틀린 방향으로 실패한다.
  if ! _dps="$(docker ps --filter status=running --format '{{.ID}} {{.Image}} {{.Names}}' 2>&1)"; then
    say "거부: docker 상태를 조회할 수 없다(rc≠0) — 서빙 여부를 **판정할 수 없으므로** 진행하지 않는다."
    say "      docker 출력: ${_dps}"
    exit 3
  fi
  # 파이프 없이(here-string) 훑는다 — pipefail + 조기종료 SIGPIPE 위음성 회피(위 주석 계열).
  _hit="$(awk 'tolower($0) ~ /vllm/ {print; exit}' <<< "$_dps")"
  if [ -n "$_hit" ]; then
    say "거부: vLLM 컨테이너가 실행 중이다(이미지 또는 이름 매칭). 먼저 serve 를 내려라."
    say "      매칭: $_hit"
    exit 3
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
  if [ "$CONFIRM" != "$CRASH_TOKEN" ]; then
    say "거부: 확인 문자열 불일치. 정말 실행하려면 --confirm=$CRASH_TOKEN 를 붙여라."
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
         "regen_envelope.py --self-test:포락선 재생성·키 정합" \
         "blackbox_session.py --self-test:세션 사이드카" \
         "blackbox_thermal.py --self-test:열·전력 포락선 엔진"; do
  f="${t%%:*}"; label="${t##*:}"
  if python3 "$SDIR/${f%% *}" ${f#* } >/dev/null 2>&1; then ok "$label self-test" "selftest_${f%%.*}"
  else bad "$label self-test 실패 — python3 $SDIR/$f" "selftest_${f%%.*}"; fi
done
if bash "$SDIR/mem_watchdog_eta.sh" --self-test >/dev/null 2>&1; then
  ok "ETA 워치독 self-test" "selftest_watchdog"
else bad "ETA 워치독 self-test 실패" "selftest_watchdog"; fi
# ★ 정지 계약(2026-09-03 · ㉛ 회귀 · plan_26090317 P1). audit ㉛ 는 "정지 기록" 을 넣으려다
#   시그널 핸들러에 `exit` 를 빠뜨려 **워치독이 TERM 으로 죽지 않게** 만들었다 — systemd stop 은
#   90s 뒤 SIGKILL 로 끝나고, KILL 은 trap 을 안 돌아 기록도 안 남는다(라이브 고아 1건이 그 결과).
#   `--self-test` 는 이 축을 못 본다(프로세스를 띄우지 않으므로). 실제로 띄워서 TERM 을 보낸다.
_wd_term_contract() {  # $1=스크립트 절대경로 → 0=TERM 2.5s 내 종료
  local script="$1" tmp pid i
  tmp="$(mktemp -d)" || return 1
  printf '#!/bin/sh\nexit 0\n' > "$tmp/docker"; chmod +x "$tmp/docker"
  ( PATH="$tmp:$PATH" BB_DRY_RUN=1 timeout 20 bash "$script" >"$tmp/out" 2>&1 & echo $! > "$tmp/pid" )
  sleep 2
  pid="$(cat "$tmp/pid" 2>/dev/null)"; [ -n "$pid" ] || { rm -rf "$tmp"; return 1; }
  kill -TERM "$pid" 2>/dev/null
  for i in $(seq 1 25); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
  if kill -0 "$pid" 2>/dev/null; then kill -KILL "$pid" 2>/dev/null; rm -rf "$tmp"; return 1; fi
  rm -rf "$tmp"; return 0
}
for wd in "$SDIR/mem_watchdog_eta.sh:ETA 워치독" \
          "$SDIR/../host_safety/mem_watchdog.sh:협역 워치독"; do
  wdp="${wd%%:*}"; wdl="${wd##*:}"
  if [ ! -f "$wdp" ]; then bad "$wdl 파일 부재 — $wdp" "term_contract_missing"; continue; fi
  if _wd_term_contract "$wdp"; then ok "$wdl SIGTERM 정지 계약(2.5s 내)" "term_contract"
  else bad "$wdl 이 SIGTERM 으로 죽지 않는다 — 정지 불가(㉛ 회귀). 시그널 trap 에 exit 가 있는지 보라." "term_contract"; fi
done

if bash "$SDIR/thermal_watchdog.sh" --self-test >/dev/null 2>&1; then
  ok "열·전력 워치독 self-test" "selftest_thermal_watchdog"
else bad "열·전력 워치독 self-test 실패 — bash $SDIR/thermal_watchdog.sh --self-test" "selftest_thermal_watchdog"; fi

# ★ 배포본 신선도 — self-test 는 **소스**를 시험한다. 데몬이 실행하는 것은 $BIN 의 사본이고,
#   둘이 갈라져 있으면 "시험 통과 + 현장은 옛 코드"가 된다. 침묵 실패라 반드시 명시 검사한다.
#   (근거: install 의 `enable --now` 가 이미 돌던 유닛을 재시작하지 않던 결함 — testlog_26073123)
for pair in "mem_watchdog_eta.sh:easy-vllm-bb-watchdog" \
            "blackbox_collect.py:easy-vllm-bb-collect" \
            "blackbox_eta.py:easy-vllm-bb-eta" \
            "regen_envelope.py:easy-vllm-bb-regen-envelope" \
            "blackbox_events.py:easy-vllm-bb-events" \
            "logs_lifecycle.py:easy-vllm-bb-lifecycle" \
            "blackbox_thermal.py:easy-vllm-bb-thermal" \
            "thermal_watchdog.sh:easy-vllm-bb-tp-watchdog"; do
  src="$SDIR/${pair%%:*}"; dst="/usr/local/sbin/${pair##*:}"
  if [ ! -f "$dst" ]; then bad "배포본 부재: $dst" "deployed_${pair##*:}"
  elif cmp -s "$src" "$dst"; then
    ok "배포본 최신 ${pair##*:}" "deployed_${pair##*:}"
  else
    bad "배포본 구버전 ${pair##*:} — 소스≠$dst. sudo bash $SDIR/install_node_blackbox.sh --apply --level L1" \
        "deployed_${pair##*:}"
  fi
done

# ★ 신선도(내용 동일)는 **실행 가능성**을 보증하지 않는다 (2026-09-01 · audit_26090109 ⑦ 2단).
#   위 A 절의 self-test 는 $SDIR(소스)에서 돈다. 그런데 데몬이 실행하는 것은 $BIN 의 사본이고,
#   설치기가 `.py` 확장자를 떼므로 **sibling import 통로가 소스에만 존재**했다.
#   실측(2026-09-01) — 체크섬은 완전히 일치하는데 판정이 반대다:
#       소스   regen_envelope.py            --self-test → rc=0
#       설치본 easy-vllm-bb-regen-envelope  --self-test → rc=1 (ModuleNotFoundError)
#   그래서 lifecycle 유닛이 5일 연속 죽는 동안 이 검증자는 계속 초록불이었고,
#   envelope.json 은 **한 번도 생성된 적이 없다**. 내용이 아니라 **배치**가 갈린 것이므로
#   체크섬으로는 원리상 잡히지 않는다 — 설치된 자리에서, 중립 cwd 로 실제로 돌려 본다.
for _b in easy-vllm-bb-eta easy-vllm-bb-collect easy-vllm-bb-events \
          easy-vllm-bb-lifecycle easy-vllm-bb-regen-envelope; do
  _d="/usr/local/sbin/$_b"
  if [ ! -x "$_d" ]; then
    bad "배포본 실행권한/부재: $_d" "deployed_exec_$_b"
  elif ( cd / && python3 -B "$_d" --self-test >/dev/null 2>&1 ); then
    ok "배포본 실동작 $_b" "deployed_exec_$_b"
  else
    bad "배포본이 **설치된 자리에서** 실패: $_b (내용은 소스와 같아도 실행되지 않는다 — sibling import 통로 확인). sudo bash $SDIR/install_node_blackbox.sh --apply --level L1" \
        "deployed_exec_$_b"
  fi
done

# ── B. L1 런타임 ─────────────────────────────────────────────────────────
echo; say "B. L1 런타임 (무재부팅 계층)"
for u in easy-vllm-blackbox-collect easy-vllm-blackbox-watchdog easy-vllm-blackbox-thermal; do
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

# 열·전력 상수(plan_26082319 §6.3). 존재만으로 유효한 게 아니라 하한 가드까지 태운다.
if [ -s "$ETC/thermal_params.env" ]; then
  ok "열·전력 상수 존재($ETC/thermal_params.env)" "thermal_params"
  # shellcheck disable=SC1090
  ( . "$ETC/thermal_params.env"; python3 "$SDIR/blackbox_thermal.py" \
      --set gpu_pwr_w="$(( ${BB_TP_GPU_PWR_DW:-800} / 10 ))" \
      --set gpu_sustain_s="${BB_TP_GPU_SUSTAIN_S:-60}" \
      --set soc_warn_c="${BB_TP_SOC_WARN_C:-90}" \
      --set soc_hard_c="${BB_TP_SOC_HARD_C:-95}" --explain 90 47 >/dev/null 2>&1 ) \
    && ok "열·전력 상수가 하한 가드 통과" "thermal_guard" \
    || bad "열·전력 상수가 하한 가드 위반 — 재생성 필요" "thermal_guard"
  # 미교정 파라미터는 **PASS 로 덮지 않는다**. 통과했다는 말이 "실측으로 검증됐다"로 읽히면
  # 그 자체가 거짓이 된다(헌법 §결정론 규율 — 값 옆에 출처).
  # shellcheck disable=SC1090
  _unc="$( . "$ETC/thermal_params.env" 2>/dev/null; echo "${BB_TP_UNCALIBRATED:-}" )"
  [ -n "$_unc" ] && say "   ⚠ 미교정 파라미터(외부 보고 역산 — 이 노드 실측 분포 없음): $_unc"
else
  bad "열·전력 상수 부재 — 워치독이 내장 기본값으로 동작 중" "thermal_params"
fi

# ── B2. 하드웨어 워치독 = 하드 락업 **복구** 계층 (plan_26082319 §5.1 · §8 성공기준 1) ──
#   ★ 이 검사가 없으면 "설치했다"와 "무장됐다"가 구별되지 않는다. system.conf 는 PID1 자신의
#     설정이라 daemon-reload 로는 반영되지 않고(daemon-reexec 필요), 그 함정에 걸리면 파일은
#     놓였는데 state 는 inactive 인 채로 조용히 지나간다.
WD_SYS=/sys/class/watchdog/watchdog0
if [ ! -e "$WD_SYS" ]; then
  # 장치 부재는 실패가 아니라 부재다 — 그러나 "복구 계층 없음"은 반드시 말한다(침묵 금지).
  say "   ⚠ $WD_SYS 부재 — 이 플랫폼엔 하드웨어 워치독이 없다. **하드 락업 복구 계층 없음**"
  PEND=$((PEND+1)); RESULTS="$RESULTS{\"check\":\"hw_watchdog\",\"verdict\":\"n/a\"},"
else
  _wst="$(cat "$WD_SYS/state" 2>/dev/null || echo unknown)"
  _wto="$(cat "$WD_SYS/timeout" 2>/dev/null || echo ?)"
  _wus="$(systemctl show -p RuntimeWatchdogUSec --value 2>/dev/null || echo ?)"
  if [ "$_wst" = "active" ]; then
    ok "하드웨어 워치독 무장($(cat "$WD_SYS/identity" 2>/dev/null) · timeout=${_wto}s · RuntimeWatchdogUSec=$_wus)" "hw_watchdog"
  else
    bad "하드웨어 워치독 **미무장**(state=$_wst · RuntimeWatchdogUSec=$_wus) — 하드 락업 시 자동 리셋 없음. 무장: sudo bash $SDIR/../host_safety/install_host_safety.sh --apply" "hw_watchdog"
  fi
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
#
# ★ `pgrep -f` 를 쓰지 않는다(2026-08-02 위양성 실화). -f 는 **argv 전체를 부분문자열로** 훑기
#   때문에, 운영자나 다른 도구가 명령줄에서 그 이름을 *언급하기만 해도* 카운트가 올라간다.
#   실제로 정리 직후 실카운트 0 인데 이 검사가 2 를 보고했고, 그 2 는 내가 방금 친 진단 명령
#   두 개였다. `[m]em…` 브래킷 트릭은 pgrep **자신의** argv 만 피할 뿐 **제3자 명령줄**은 못 피한다.
#   → 실행 중인 프로그램의 **스크립트 인자 자체**가 그 파일인지로 판정한다(언급 ≠ 실행).
#   이 프로젝트에서 자기매칭 함정은 반복 계열이다(`pkill -f` 가 자기 셸을 죽인 전례 포함).
#
# ★★ 2026-08-18 교정(plan_26081809 §7 · testlog_26081810 §8): **개수 판정 → 대상 유무 판정**.
#   옛 판정은 "실행 중인 협역 워치독이 하나라도 있으면 FAIL" 이었다. 그런데 협역 워치독은
#   `policy:HOST_SAFETY_LAYERED_DEFENSE.C1` 이 요구하는 방어층이고, `multinode_serve_smoke.sh`
#   는 그것을 **canonical** 이라 부르며 부재 시 exit 2 로 죽는다. 즉 **살아 있는 것이 정상**인
#   국면이 규정돼 있다 — `--keep-up` 상주 서빙 중에는 워치독이 `PPID=1` 로 떠 있는 것이 설계다.
#   2026-08-17 실측: 상주 컨테이너를 내렸더니 코드 한 줄 안 고치고 28/1 → **29/0** 이 됐다.
#   그 판정은 결함을 발견한 게 아니라 **정상 상태를 결함이라 부른 것**이다(FALSE-POSITIVE).
#   위양성 가드는 다음 사람에게 "정상 방어층을 죽여라"라고 지시하므로, 없느니만 못하다.
#
#   ∴ 좀비의 정의를 다시 쓴다: **지킬 대상이 없는데 살아 있는 워치독**. 판정은 워치독 자신의
#   `targets()`(mem_watchdog.sh:34-43)를 그대로 재현한다 — argv $3 이 name_filter 이고
#   생략/`@vllm` 이면 광역(이미지·이름에 vllm 포함 전체), 아니면 `docker ps --filter name=<f>`.
#   대상이 하나라도 살아 있으면 그 워치독은 **제 일을 하는 중**이고, 0 이면 고아다.
#
#   ⚠ `pgrep -f` 는 여전히 쓰지 않는다(2026-08-02 위양성 실화 — 아래 원 주석 보존).
#     -f 는 argv 전체를 부분문자열로 훑어 **언급만 해도** 카운트가 오른다. 실제로 정리 직후
#     실카운트 0 인데 2 를 보고했고 그 2 는 방금 친 진단 명령이었다. `[m]em…` 브래킷 트릭은
#     pgrep 자신의 argv 만 피할 뿐 제3자 명령줄은 못 피한다. → 실행 중 프로그램의 **스크립트
#     인자 자체**가 그 파일인지로 판정한다(언급 ≠ 실행).
_wd_live_targets() {   # $1=name_filter → 살아 있는 대상 컨테이너 수
  if [ -z "$1" ] || [ "$1" = "@vllm" ]; then
    docker ps --filter status=running --format '{{.ID}} {{.Image}} {{.Names}}' 2>/dev/null \
      | awk 'tolower($0) ~ /vllm/ {n++} END {print n+0}'
  else
    # ⚠ `grep -c . || echo 0` 를 쓰지 않는다 — grep 은 0건일 때 "0" 을 찍고 **exit 1** 을 내므로
    #   `|| echo 0` 이 붙으면 "0\n0" 이 되어 뒤의 `[ -gt ]` 가 깨진다. 이 파일 위쪽이 `pgrep -c`
    #   로 같은 함정을 이미 문서화해 뒀는데, 이 함수를 처음 쓴 판본이 그대로 재현했다(2026-08-18
    #   음성대조에서 발화: `[: 0\n0: integer expression expected`). wc -l 은 0건에도 exit 0 이다.
    docker ps --filter "name=$1" --filter status=running -q 2>/dev/null | wc -l
  fi
}
WD_TOTAL=0; WD_ORPHAN=0; WD_GUARDING=0; WD_DETAIL=""
while IFS= read -r _flt; do
  [ -n "$_flt" ] || continue
  WD_TOTAL=$((WD_TOTAL+1))
  _n=$(_wd_live_targets "$_flt"); _n=${_n:-0}
  if [ "$_n" -gt 0 ]; then WD_GUARDING=$((WD_GUARDING+1)); WD_DETAIL="$WD_DETAIL '$_flt'→${_n}개(가동중)"
  else WD_ORPHAN=$((WD_ORPHAN+1));  WD_DETAIL="$WD_DETAIL '$_flt'→0개(고아)"; fi
done < <(ps -eo args= 2>/dev/null | awk '
    $1 ~ /(^|\/)bash$/ && $2 ~ /(^|\/)mem_watchdog\.sh$/ { print ($3 == "" ? "@vllm" : $3) }')
if [ "$WD_ORPHAN" = "0" ]; then
  if [ "$WD_TOTAL" = "0" ]; then ok "레거시 협역 워치독 고아 0 (실행 중 0개)" "no_zombie"
  else ok "레거시 협역 워치독 고아 0 (${WD_GUARDING}개가 대상 보호 중 —$WD_DETAIL)" "no_zombie"; fi
else
  bad "레거시 협역 워치독 고아 $WD_ORPHAN 개(전체 $WD_TOTAL) — 대상 컨테이너가 없는데 살아 있다. PID 기반으로 정리 필요:$WD_DETAIL" "no_zombie"
fi

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
  say "   sudo bash $0 --crash-test --confirm=$CRASH_TOKEN     # 노드가 죽고 재부팅됨"
  say "   (재부팅 후)  sudo bash $0 --post-crash"
fi
[ "$FAILN" = 0 ] && exit 0 || exit 2
