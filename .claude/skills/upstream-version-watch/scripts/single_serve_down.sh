#!/bin/bash
# single_serve_down.sh — **단일노드 정규 teardown 진입점** (W-6).
#
# 왜 있나(2026-08-22 실측 · testlog_26082215 §4.0 R-0): single 토폴로지에는 스크립트화된 회수 경로가
#   **없었다.** `multinode_serve_smoke.sh --down` 은 `EF="output/multi/envs/.env.<config>"` 로 통로를
#   multi 에 하드코딩하고 SSH 슬레이브를 전제하므로 single 에서는 `[mn] FAIL: … 없음` 으로 죽는다.
#   그래서 README §B-3 의 `docker compose … down` 을 직접 치게 되는데, 그러면 5단계 중 **뒤 셋을
#   조용히 빠뜨린다** — 워치독·페이지캐시·예산선언·세션이 남는다.
#   `workflow.md` §막힘 3분류의 **침묵 누락**이다(가드가 막은 것이 아니라 배선이 없었다).
#
# 왜 멀티 스크립트를 파라미터화하지 않았나: 그 스크립트는 Ray head/worker·SSH 슬레이브·클러스터
#   랑데부·양노드 예산을 **전제**로 짜여 있다. single 은 그 전제가 통째로 없으므로, 한 스크립트에
#   두 토폴로지를 넣으면 분기마다 "이건 멀티만" 주석이 붙는 코드가 된다 — 헌법 §불변식 A 가
#   경고하는 *한 스킴으로 두 존재를 덮는* 형태 그대로다. 진입점을 가른다.
#
# 6단계(순서가 곧 안전장치다):
#   1) 컨테이너 down        — 서빙을 먼저 내린다
#   2) 상주 사이드카 회수    — 협역 워치독 · 예산갱신 루프(argv **위치** 대조만)
#   3) 페이지캐시 드랍      — best-effort. **건너뛰면 사유를 반드시 말한다**
#   4) 예산 선언 회수       — 남기면 다음 로드가 *남의 바닥*으로 무장한다
#   5) 블랙박스 세션 stop   — 열린 세션이 없으면 그렇게 말하고, 둘 이상이면 fail-loud
#
# ★ **침묵 skip 금지가 이 스크립트의 존재 이유다.** 각 단계는 DONE/SKIPPED(사유)/FAIL 중 하나를
#   반드시 출력한다. R-0 에서 drop_caches 가 조용히 빠진 것이 정확히 이 결함이었다.
#
# 사용:
#   bash single_serve_down.sh <config_name> [--dry-run] [--session-id <id>] [--node-id <slug>]
#                                           [--no-drop-caches] [--keep-session]
# 종료코드: 0=전 단계 완료(정당한 skip 포함) · 2=단계 실패 · 3=설정/통로 부재
set -uo pipefail

TAG="[single-down]"
CONFIG="${1:-}"
[ -n "$CONFIG" ] || { echo "$TAG FAIL: 사용: single_serve_down.sh <config_name> [--dry-run] …" >&2; exit 3; }
shift

DRY=0; SESSION_ID=""; EXPLICIT_NODE_ID=""; DROP=1; KEEP_SESSION=0
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run)         DRY=1; shift ;;
    --session-id)      SESSION_ID="${2:-}"; shift 2 ;;
    --node-id)         EXPLICIT_NODE_ID="${2:-}"; shift 2 ;;
    --no-drop-caches)  DROP=0; shift ;;
    --keep-session)    KEEP_SESSION=1; shift ;;
    *) echo "$TAG FAIL: 알 수 없는 인자: $1" >&2; exit 3 ;;
  esac
done

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
cd "$REPO" || exit 3

# ── 통로 해소 (single 고정 — 브랜치로 추론하지 않는다. 통로는 인자와 경로가 정한다) ──
COMPOSE="output/single/docker-compose.yaml"
EF="output/single/envs/.env.${CONFIG}"
EF_ROOT="output/single/.env"          # 마운트 경로 치환(NAS/quant/tiktoken) — materialize-env 산출
[ -f "$COMPOSE" ] || { echo "$TAG FAIL: $COMPOSE 없음 — single 통로가 렌더되지 않았다." >&2; exit 3; }
[ -f "$EF" ]      || { echo "$TAG FAIL: $EF 없음 — config 이름을 확인하라." >&2; exit 3; }

# ⚠ 루트 .env 는 **선택이 아니다**: compose 의 ${NAS_MODEL_PATH} 등이 여기서 채워진다. 없으면
#   down 은 되지만 경고를 남긴다(기동 때 같은 값으로 떴는지 알 수 없기 때문).
ENVFILES=(--env-file "$EF")
if [ -f "$EF_ROOT" ]; then ENVFILES=(--env-file "$EF_ROOT" --env-file "$EF")
else echo "$TAG ⚠ $EF_ROOT 부재 — 마운트 변수는 compose 기본값으로 해소된다(render_dockerfile.py --materialize-env 선행 권장)."; fi

val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
CNAME="$(val CONTAINER_NAME)"
[ -n "$CNAME" ] || CNAME="vllm-serve-container"     # compose 기본값과 동일(정보 손실 없음)

# ── 블랙박스 도구 디렉터리 해소 — **메인과 서브가 경로가 다르다** ──
#
# 2026-09-03 실측: 이 스크립트는 서브에 배달되는데(런타임블럭) node_blackbox 경로를
#   `.claude/skills/terraforming_node/scripts/node_blackbox/` 로 하드코딩했다. 서브에는
#   `terraforming_node`(온보딩 스킬)가 **가지 않는다** — 렌더러가 블랙박스만 떼어
#   `.claude/runtime/node_blackbox/` 로 배치한다. 그래서 서브에서 teardown 이
#   `node_identity.sh: No such file` → `ni_resolve_node_id: command not found` →
#   `FAIL: node_id 미해소` 로 **구조적으로 못 돌았다**. 서브가 이 스크립트를 처음 쓰려 한
#   순간에 드러났다(그 전까지는 아무도 부르지 않아 보이지 않았다).
#   침묵 누락이 아니라 **경로 가정과 배달 표면의 불일치**다 — 배달되는 스크립트는 배달된
#   자리에서 도는지로 판정해야 한다.
_bb_dir() {
  local c
  for c in "$REPO/.claude/runtime/node_blackbox" \
           "$REPO/.claude/skills/terraforming_node/scripts/node_blackbox"; do
    [ -d "$c" ] && { printf '%s' "$c"; return 0; }
  done
  return 1
}
BB_DIR="$(_bb_dir)" || {
  echo "$TAG FAIL: node_blackbox 도구 디렉터리를 찾지 못했다 — 메인 .claude/skills/terraforming_node/scripts/node_blackbox 도, 서브 .claude/runtime/node_blackbox 도 없다." >&2; exit 3; }

# ── node_id 는 단일 해소기가 답한다(각자 파싱 ✗ · hostname 폴백 ✗) ──
. "$BB_DIR/node_identity.sh"
NODE_ID="$(ni_resolve_node_id "$REPO" "$EXPLICIT_NODE_ID")" || {
  echo "$TAG FAIL: node_id 미해소 — 예산·세션 회수 대상 디렉터리를 정할 수 없다(fail-loud)." >&2; exit 2; }
NODE_DIR="$REPO/docs/logs/$NODE_ID"
SESSION_PY="$BB_DIR/blackbox_session.py"
NOW_ISO(){ date -u +%FT%TZ; }

echo "$TAG 대상: container=$CNAME · compose=$COMPOSE · config=$CONFIG · node_id=$NODE_ID$([ $DRY = 1 ] && echo ' · DRY-RUN(변경 없음)')"

FAILED=0
run(){   # $1=설명 · 나머지=명령. DRY 면 찍기만 한다.
  if [ "$DRY" = "1" ]; then echo "$TAG   (dry-run) $*"; return 0; fi
  "$@"
}

# ── 1/5 컨테이너 down ───────────────────────────────────────────────────────────────
if [ "$DRY" = "1" ]; then
  echo "$TAG 1/5 컨테이너 down : (dry-run) docker compose -f $COMPOSE ${ENVFILES[*]} --profile serve down"
elif docker compose -f "$COMPOSE" "${ENVFILES[@]}" --profile serve down >/dev/null 2>&1; then
  echo "$TAG 1/5 컨테이너 down : DONE"
else
  echo "$TAG 1/5 컨테이너 down : FAIL — compose down 비정상 종료(수동 확인 필요)" >&2; FAILED=1
fi

# ── 2/5 상주 사이드카 회수 (협역 워치독 · 예산갱신 루프) ──────────────────────────────
#   판정은 argv **위치**로만 한다: argv[1]=bash · argv[2]=…/<script>(끝 앵커). 부분문자열 매칭
#   (`pkill -f`)은 이름을 언급만 한 명령까지 잡아 자기 부모셸을 죽인 선례가 있다(exit144).
reap_by_argv(){   # $1=스크립트 basename(정규식 앵커용) $2=라벨
  local script="$1" label="$2" n=0 p
  while read -r p; do
    [ -n "$p" ] || continue
    if [ "$DRY" = "1" ]; then echo "$TAG   (dry-run) kill $p ($label)"; n=$((n+1)); continue; fi
    kill "$p" 2>/dev/null && { echo "$TAG   회수 $label pid=$p"; n=$((n+1)); }
  done < <(ps -eo pid= -o args= | awk -v self="$$" -v s="$script" \
             '$1 != self && $2 ~ /(^|\/)bash$/ && $3 ~ ("(^|/)" s "$") {print $1}')
  echo "$n"
}
# ⚠ awk **문자열 리터럴**로 넘어가므로 백슬래시를 한 겹 더 쓴다("\\." → awk 문자열 `\.` → 정규식 `\.`).
#   한 겹만 쓰면 awk 가 "escape sequence `\.' treated as plain `.'" 경고를 내고 점이 임의문자가 된다.
WD_N="$(reap_by_argv 'mem_watchdog\\.sh' '협역 워치독' | tail -1)"
RN_N="$(reap_by_argv 'budget_renew_loop\\.sh' '예산갱신 루프' | tail -1)"
echo "$TAG 2/5 사이드카 회수  : DONE (워치독 ${WD_N}건 · 갱신루프 ${RN_N}건)"

# ── 3/5 페이지캐시 드랍 — **건너뛰면 사유를 말한다**(R-0 의 침묵 skip 이 이 단계였다) ────
DROP_HELPER="/usr/local/sbin/vllm-drop-caches"
if [ "$DROP" = "0" ]; then
  echo "$TAG 3/5 페이지캐시     : SKIPPED — --no-drop-caches 로 명시 요청됨"
elif [ ! -x "$DROP_HELPER" ]; then
  echo "$TAG 3/5 페이지캐시     : SKIPPED — 헬퍼 부재($DROP_HELPER). 설치: install_host_safety.sh"
elif ! sudo -n true 2>/dev/null; then
  # ★ R-0 실측: 비밀번호 없는 sudo 가 없어 조용히 빠졌다. 조용히 빠지면 "했다"와 구분되지 않는다.
  echo "$TAG 3/5 페이지캐시     : SKIPPED — 비밀번호 없는 sudo 부재(sudo -n 실패). 잔여 페이지캐시가 남는다."
elif run sudo -n "$DROP_HELPER" >/dev/null 2>&1; then
  echo "$TAG 3/5 페이지캐시     : DONE"
else
  echo "$TAG 3/5 페이지캐시     : FAIL — 헬퍼 실행 실패" >&2; FAILED=1
fi

# ── 4/5 예산 선언 회수 ──────────────────────────────────────────────────────────────
#   무조건 회수한다(멱등). 이 실행이 선언한 바가 없어도 — 내리려는 그 서빙의 선언은 다른 실행이
#   남긴 것이고, 그것을 남기는 것이 정확히 이 진입점이 고치는 결함이다.
if [ "$DRY" = "1" ]; then
  echo "$TAG 4/5 예산선언 회수  : (dry-run) python3 blackbox_session.py --node-dir $NODE_DIR clear-budget --now <ISO>"
elif out="$(python3 "$SESSION_PY" --node-dir "$NODE_DIR" clear-budget --now "$(NOW_ISO)" 2>&1)"; then
  echo "$TAG 4/5 예산선언 회수  : DONE — $(printf '%s' "$out" | tr '\n' ' ')"
else
  echo "$TAG 4/5 예산선언 회수  : FAIL — $out" >&2; FAILED=1
fi

# ── 5/5 블랙박스 세션 stop ──────────────────────────────────────────────────────────
#   열린 세션 = sessions/*.json 중 stopped_utc 가 null 인 것. 둘 이상이면 **조용히 첫 항목을
#   고르지 않는다**(resolve_rank 의 RANK_AMBIGUOUS_ROLE 과 같은 규율) — 어느 서빙을 닫는지
#   모른 채 닫으면 그 기록은 거짓이 된다.
if [ "$KEEP_SESSION" = "1" ]; then
  echo "$TAG 5/5 세션 stop      : SKIPPED — --keep-session 으로 명시 요청됨"
else
  if [ -z "$SESSION_ID" ]; then
    OPEN_IDS="$(python3 - "$NODE_DIR" <<'PY'
import glob, json, os, sys
node_dir = sys.argv[1]
for path in sorted(glob.glob(os.path.join(node_dir, "sessions", "*.json"))):
    try:
        with open(path, encoding="utf-8") as fh:
            doc = json.load(fh)
    except (OSError, ValueError):
        continue
    if doc.get("stopped_utc") is None:
        print(doc.get("session_id") or os.path.basename(path)[:-5])
PY
)"
    N_OPEN="$(printf '%s' "$OPEN_IDS" | grep -c . || true)"
    if [ "${N_OPEN:-0}" = "0" ]; then
      echo "$TAG 5/5 세션 stop      : SKIPPED — 열린 세션 없음($NODE_DIR/sessions)"
    elif [ "$N_OPEN" = "1" ]; then
      SESSION_ID="$(printf '%s' "$OPEN_IDS" | head -1)"
    else
      echo "$TAG 5/5 세션 stop      : FAIL — 열린 세션이 ${N_OPEN}개다. 조용히 하나를 고르지 않는다." >&2
      printf '%s\n' "$OPEN_IDS" | sed "s|^|$TAG     후보: |" >&2
      echo "$TAG     → --session-id <id> 로 지목하라." >&2
      FAILED=1
    fi
  fi
  if [ -n "$SESSION_ID" ]; then
    if [ "$DRY" = "1" ]; then
      echo "$TAG 5/5 세션 stop      : (dry-run) stop --session-id $SESSION_ID"
    elif out="$(python3 "$SESSION_PY" --node-dir "$NODE_DIR" stop --session-id "$SESSION_ID" --now "$(NOW_ISO)" 2>&1)"; then
      echo "$TAG 5/5 세션 stop      : DONE — $SESSION_ID"
    else
      echo "$TAG 5/5 세션 stop      : FAIL — $out" >&2; FAILED=1
    fi
  fi
fi

# ── 6/6 컨테이너 생성물 소유권 정렬 ──────────────────────────────────────────
#
# 왜(2026-09-03 wipe 2차 실측): compose 는 JIT/컴파일 캐시를 `./cache/{vllm,flashinfer}` →
#   컨테이너 `/root/.cache/*` 로 마운트한다(통로 self-containment 가 설계 의도 · compose 주석).
#   컨테이너 프로세스는 root 라 그 캐시 파일이 **호스트에서 root 소유**로 남는다. 그러면
#   위임 사용자가 자기 워크스페이스를 정리할 수 없다:
#     `rm: cannot remove '.../output/single/cache/vllm/torch_compile_cache/...': Permission denied`
#   포크 사용자가 서빙 한 번 한 뒤 워크스페이스를 지우려면 sudo 가 필요해진다 — 배포 결함이다.
#   1차 wipe 때는 서브가 서빙한 적이 없어 캐시가 없었고, 그래서 **보이지 않았다**.
#
# 왜 캐시를 프로젝트 밖으로 빼지 않는가: compose 주석이 근거를 적는다 — 통로 self-containment
#   (`./cache = output/<topology>/cache`, gitignored)와 레퍼런스 레시피(호스트 venv `~/.cache`
#   유지 전제)와의 parity. 그 의도를 깨지 않고 **소유권만** 되돌린다.
#
# 왜 컨테이너 경유인가: chown 은 root 권한을 요구하고 이 스크립트는 비밀번호 없는 sudo 를
#   가정하지 않는다(3단계가 이미 그래서 skip 된다). 도커 소켓 접근권은 이미 전제이므로
#   같은 권한의 **정식 통로**를 쓴다(우회가 아니라 경로다). 대상 uid/gid 는 프로젝트 경로
#   소유자에서 **파생**한다 — 하드코딩하지 않는다. chown 은 멱등이라 재실행이 안전하다.
CACHE_DIR="$REPO/output/single/cache"   # 통로는 54-56행과 동일 고정(이 진입점은 single 전용)
if [ ! -d "$CACHE_DIR" ]; then
  echo "$TAG 6/6 캐시 소유권     : SKIPPED — $CACHE_DIR 부재(서빙 이력 없음)"
else
  _own="$(stat -c '%u:%g' "$REPO")"
  _root_n="$(find "$CACHE_DIR" ! -user "$(stat -c '%U' "$REPO")" 2>/dev/null | wc -l)"
  if [ "${_root_n:-0}" = "0" ]; then
    echo "$TAG 6/6 캐시 소유권     : DONE — 이미 정렬됨(타 소유 0건 · no-op)"
  elif [ "$DRY" = "1" ]; then
    echo "$TAG 6/6 캐시 소유권     : (dry-run) docker run --rm -v $CACHE_DIR:/w <IMAGE> chown -R $_own /w  (타 소유 ${_root_n}건)"
  else
    _img="$(grep -E '^IMAGE_TAG=' "$EF" 2>/dev/null | head -1 | cut -d= -f2-)"
    [ -n "$_img" ] || _img="$(docker images --format '{{.Repository}}:{{.Tag}}' 2>/dev/null | grep -E '^easy-vllm:' | head -1)"
    if [ -z "$_img" ]; then
      echo "$TAG 6/6 캐시 소유권     : FAIL — chown 을 실행할 로컬 이미지를 찾지 못했다(타 소유 ${_root_n}건 잔존)." >&2
      echo "$TAG   → 수동: sudo chown -R $(stat -c '%U:%G' "$REPO") $CACHE_DIR" >&2
      FAILED=1
    elif out="$(docker run --rm -v "$CACHE_DIR":/w --entrypoint chown "$_img" -R "$_own" /w 2>&1)"; then
      echo "$TAG 6/6 캐시 소유권     : DONE — ${_root_n}건 → $(stat -c '%U:%G' "$REPO") (이미지 $_img)"
    else
      echo "$TAG 6/6 캐시 소유권     : FAIL — $out" >&2
      echo "$TAG   → 수동: sudo chown -R $(stat -c '%U:%G' "$REPO") $CACHE_DIR" >&2
      FAILED=1
    fi
  fi
fi

if [ "$FAILED" = "0" ]; then
  echo "$TAG 완료(6단계). 재기동은 README §B-3 또는 에이전트 경로(run_trial)로."
  exit 0
fi
echo "$TAG 일부 단계 실패 — 위 FAIL 을 확인하라(부분 회수 상태다)." >&2
exit 2
