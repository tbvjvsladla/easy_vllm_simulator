#!/bin/bash
# single_serve_up.sh — **단일노드 정규 기동 진입점** (plan_26090419 P1 · 2026-09-04)
#
# 왜 있나: single 에는 teardown(`single_serve_down.sh`)만 있고 **기동 경로가 없었다.** 현행은
#   README §B-3 의 `docker compose up` 을 사람이 직접 치는 것인데, 그러면 **예산 선언과 워치독
#   무장이 통째로 빠진다** — 즉 무보호 기동이다. 그 상태로 58 GiB 급 로드를 하면 ETA 워치독이
#   정상 로드를 사살한 계보(2026-08-01 3/3 · 2026-08-14 2회)로 곧장 돌아간다.
#   `workflow.md` §막힘 3분류로는 **침묵 누락**(가드가 막은 것이 아니라 배선이 없었다)이며,
#   처방은 우회가 아니라 **경로를 만드는 것**이다(D3).
#
# ★ 순서가 곧 판정 기준이다: 예산 선언 → **그 다음** 컨테이너 up.
#   사후 선언은 무의미하다 — 로드 골짜기는 이미 지나갔다(multinode_serve_smoke 와 같은 규약).
#
# ★ 입력은 전부 **파생**한다(손저작 ✗). weights/kv/tp 는 `check_smoke_model.py --emit-gate-params`
#   가 내는 BUDGET_PARAMS 한 곳에서 온다. 같은 값을 두 곳에서 읽으면 반드시 갈린다.
#
# ★ **침묵 skip 금지.** 각 단계는 DONE / SKIPPED(사유) / FAIL 중 하나를 반드시 출력한다.
#   그리고 **부분 상태를 남기지 않는다** — 어느 단계든 실패하면 teardown 으로 되돌린다.
#   반쯤 올라간 서빙은 다음 기동의 차단 사유가 되고, 남은 예산 선언은 다음 로드를 *남의 바닥*으로
#   무장시킨다(그것이 down 이 5단계를 갖는 이유와 같다).
#
# 7단계:
#   1) 통로·전제        — compose · envfile · 루트 .env · config yaml
#   2) 모델 + RAM 게이트 — check_smoke_model(로드-전 게이트) · BUDGET_PARAMS 파생
#   3) 예산 선언        — declare-budget (**로드 개시 전**)
#   4) 블랙박스 세션    — serve_start 이벤트
#   5) compose up       — 두 env-file(루트 .env 는 마운트 변수 · config env 는 서빙 변수)
#   6) 예산 갱신 루프   — TTL 만료로 선언이 사라지는 것을 막는다
#   7) health 대기      — READY_MAX 내 200
#
# 사용:
#   bash single_serve_up.sh <config_name> [--dry-run] [--node-id <slug>] [--session-id <id>]
#        [--ready-max <초>] [--ttl-s <초>] [--expected-load-s <초>] [--no-renew-loop]
#        [--overhead-mib <n>] [--label <text>]
# 종료코드: 0=서빙 READY · 2=단계 실패(정리됨) · 3=설정/통로 부재 · 4=RAM 게이트 거부 · 5=health 타임아웃(정리됨)
set -uo pipefail

TAG="[single-up]"
CONFIG="${1:-}"
[ -n "$CONFIG" ] || { echo "$TAG FAIL: 사용: single_serve_up.sh <config_name> [--dry-run] …" >&2; exit 3; }
shift

DRY=0; EXPLICIT_NODE_ID=""; SESSION_ID=""; RENEW=1; LABEL=""
# READY_MAX 기본 600 = 2026-08-15 실측(대형 MoE 로드가 그 안에 든다). 넘기면 타임아웃이 정직한
# 판정이며, 더 큰 모델은 호출부가 **명시**해서 늘린다(조용히 무한 대기하지 않는다).
READY_MAX=600
TTL_S=""            # 미지정이면 blackbox_session(단일 소유자)의 기본값을 쓴다(G-B2)
EXPECTED_LOAD_S=""
OVERHEAD_MIB=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run)          DRY=1; shift ;;
    --node-id)          EXPLICIT_NODE_ID="${2:-}"; shift 2 ;;
    --session-id)       SESSION_ID="${2:-}"; shift 2 ;;
    --ready-max)        READY_MAX="${2:-}"; shift 2 ;;
    --ttl-s)            TTL_S="${2:-}"; shift 2 ;;
    --expected-load-s)  EXPECTED_LOAD_S="${2:-}"; shift 2 ;;
    --overhead-mib)     OVERHEAD_MIB="${2:-}"; shift 2 ;;
    --no-renew-loop)    RENEW=0; shift ;;
    --label)            LABEL="${2:-}"; shift 2 ;;
    *) echo "$TAG FAIL: 알 수 없는 인자: $1" >&2; exit 3 ;;
  esac
done

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
cd "$REPO" || exit 3

# ── 1/7 통로·전제 (single 고정 — 브랜치로 추론하지 않는다) ───────────────────────────
COMPOSE="output/single/docker-compose.yaml"
EF="output/single/envs/.env.${CONFIG}"
EF_ROOT="output/single/.env"
CFGYAML="output/single/configs/${CONFIG}.yaml"
for f in "$COMPOSE" "$EF" "$CFGYAML"; do
  [ -f "$f" ] || { echo "$TAG 1/7 통로·전제      : FAIL — $f 없음" >&2; exit 3; }
done
# 루트 .env 는 compose 의 마운트 변수(${NAS_MODEL_PATH} 등)를 채운다. 없으면 compose 기본값으로
# 조용히 폴백해 **다른 경로를 마운트한 채 뜬다** — 기동에서는 그것이 down 보다 위험하므로 거부한다.
[ -f "$EF_ROOT" ] || { echo "$TAG 1/7 통로·전제      : FAIL — $EF_ROOT 부재. 마운트 변수가 compose 기본값으로 조용히 폴백한다(render_dockerfile.py --materialize-env 선행)." >&2; exit 3; }

val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
CNAME="$(val CONTAINER_NAME)"; [ -n "$CNAME" ] || CNAME="vllm-serve-container"
PORT="$(val SERVING_PORT)"
[ -n "$PORT" ] || { echo "$TAG 1/7 통로·전제      : FAIL — $EF 에 SERVING_PORT 없음" >&2; exit 3; }
IMAGE_TAG="$(val IMAGE_TAG)"
ENVFILES=(--env-file "$EF_ROOT" --env-file "$EF")
# ── manifest ↔ materialize 산출물 정합 (2026-09-04 · 실패 1건이 만든 검사) ────────────
#   `output/<topo>/.env` 는 manifest 에서 **파생**되는데, manifest 가 바뀌어도 아무도 재생성을
#   요구하지 않는다. 2026-09-03 에 harmony 토크나이저가 NAS 정본으로 옮겨졌을 때 multi 의 .env 만
#   재생성되고 single 은 옛 로컬 경로를 든 채 남았다 → compose 가 없는 경로를 마운트 소스로 잡고
#   docker 가 **그 자리에 빈 디렉터리를 만들어** 컨테이너는 떴다가 6분 뒤 vocab 부재로 죽었다.
#   침묵 분기다: 권위(manifest)와 파생물이 갈라졌는데 묻는 사람이 없었다.
#   `staleness_gate.py` 는 HW·네트워크·attestation 만 본다 — 이 축은 그 바깥이다.
#   ★ 로드 **전에** 판정한다. 6분 뒤에 아는 것과 6초 만에 아는 것의 차이가 이 검사의 전부다.
mval(){ sed -n "s/^[[:space:]]*$1:[[:space:]]*\"\{0,1\}\([^\"#]*\)\"\{0,1\}.*/\1/p" \
          output/single/manifest.yaml 2>/dev/null | head -1 | sed 's/[[:space:]]*$//'; }
eval_root(){ grep -E "^$1=" "$EF_ROOT" | head -1 | cut -d= -f2-; }
_drift=0
for pair in "NAS_MODEL_PATH:nas_model_path" "QUANT_MODEL_PATH:quant_model_path" \
            "TIKTOKEN_HOST_PATH:tiktoken_host_path"; do
  _k="${pair%%:*}"; _mk="${pair##*:}"
  _have="$(eval_root "$_k")"; _want="$(mval "$_mk")"
  [ -n "$_want" ] || continue           # manifest 에 없는 키는 대조 대상이 아니다(부재 ≠ 불일치)
  if [ "$_have" != "$_want" ]; then
    echo "$TAG 1/7 통로·전제      : FAIL — $EF_ROOT 가 manifest 와 갈렸다($_k)" >&2
    echo "$TAG     .env=$_have" >&2
    echo "$TAG     manifest=$_want" >&2
    _drift=1
  fi
done
if [ "$_drift" = "1" ]; then
  echo "$TAG     재생성: python3 .claude/skills/upstream-version-watch/scripts/render_dockerfile.py \\" >&2
  echo "$TAG               --materialize-env --topology single --manifest output/single/manifest.yaml" >&2
  exit 3
fi
echo "$TAG 1/7 통로·전제      : DONE (container=$CNAME port=$PORT image=${IMAGE_TAG:-<compose 기본값>} · manifest 정합 ✓)"

# 블랙박스 도구 경로 — 메인/서브가 다르다(single_serve_down.sh 와 같은 해소기).
_bb_dir() {
  local c
  for c in "$REPO/.claude/runtime/node_blackbox" \
           "$REPO/.claude/skills/terraforming_node/scripts/node_blackbox"; do
    [ -d "$c" ] && { printf '%s' "$c"; return 0; }
  done
  return 1
}
BB_DIR="$(_bb_dir)" || { echo "$TAG FAIL: node_blackbox 도구 디렉터리 부재" >&2; exit 3; }
. "$BB_DIR/node_identity.sh"
NODE_ID="$(ni_resolve_node_id "$REPO" "$EXPLICIT_NODE_ID")" || {
  echo "$TAG FAIL: node_id 미해소 — 예산·세션 디렉터리를 정할 수 없다(fail-loud)." >&2; exit 2; }
NODE_DIR="$REPO/docs/logs/$NODE_ID"
SESSION_PY="$BB_DIR/blackbox_session.py"
# TTL 기본값은 **단일 소유자**(blackbox_session)에게 묻는다 — 여기에 7200 을 다시 적지 않는다(G-B2).
if [ -z "$TTL_S" ]; then
  TTL_S="$(python3 "$SESSION_PY" --node-dir "$NODE_DIR" budget-defaults --field ttl_s 2>/dev/null || echo)"
  case "$TTL_S" in ''|*[!0-9]*)
    echo "$TAG FAIL: 예산 TTL 기본값을 blackbox_session 에서 읽지 못했다 — 숫자를 여기 다시 적지 않는다" >&2
    exit 2;;
  esac
fi
RENEW_SH="$BB_DIR/budget_renew_loop.sh"
NOW_ISO(){ date -u +%FT%TZ; }

# 실패 시 되돌린다 — 반쯤 올라간 상태를 남기지 않는다.
CLEANUP_NEEDED=0
rollback(){
  [ "$CLEANUP_NEEDED" = "1" ] || return 0
  echo "$TAG ↩ 되돌리기: single_serve_down.sh 로 부분 상태를 정리한다" >&2
  bash "$SDIR/single_serve_down.sh" "$CONFIG" ${EXPLICIT_NODE_ID:+--node-id "$EXPLICIT_NODE_ID"} >&2 || true
}

# ── 2/7 모델 + 로드-전 RAM 게이트 + 예산 입력 파생 ──────────────────────────────────
GATE_OUT="$(python3 "$SDIR/check_smoke_model.py" "$CONFIG" --topology single --emit-gate-params 2>&1)"
GATE_RC=$?
printf '%s\n' "$GATE_OUT" | sed "s/^/$TAG     /"
if [ "$GATE_RC" != "0" ]; then
  echo "$TAG 2/7 모델·RAM 게이트 : FAIL — 로드-전 게이트 거부(rc=$GATE_RC). 잔존 컨테이너·페이지캐시 정리 후 재시도." >&2
  exit 4
fi
BP="$(printf '%s\n' "$GATE_OUT" | grep -o 'BUDGET_PARAMS .*' | head -1)"
CKPT_MIB="$(printf '%s' "$BP" | sed -n 's/.*ckpt_mib=\([0-9]*\).*/\1/p')"
TP="$(printf '%s' "$BP" | sed -n 's/.*tp=\([0-9]*\).*/\1/p')"
KV_MIB="$(printf '%s' "$BP" | sed -n 's/.*kv_mib=\([0-9a-z]*\).*/\1/p')"
if [ -z "$CKPT_MIB" ] || [ -z "$TP" ] || [ "$KV_MIB" = "none" ] || [ -z "$KV_MIB" ]; then
  # KV 절대클램프 미선언이면 **예산 선언 자체가 불가**하다(설계 의도 — policy:KV_ABSOLUTE_CLAMP_PORTABILITY).
  echo "$TAG 2/7 모델·RAM 게이트 : FAIL — 예산 입력을 파생하지 못했다(BUDGET_PARAMS='$BP'). config 의 kv-cache-memory-bytes 를 확인하라." >&2
  exit 3
fi
WEIGHTS_MIB=$(( CKPT_MIB / TP ))
MEM_TOTAL_MIB="$(awk '/MemTotal/{print int($2/1024)}' /proc/meminfo)"
echo "$TAG 2/7 모델·RAM 게이트 : DONE (ckpt=${CKPT_MIB}MiB ÷ tp=${TP} → weights=${WEIGHTS_MIB}MiB · kv=${KV_MIB}MiB · mem_total=${MEM_TOTAL_MIB}MiB)"

# ── 3/7 서빙 예산 선언 (로드 개시 **전**) ───────────────────────────────────────────
DECL_ARGS=(--node-dir "$NODE_DIR" declare-budget
           --mem-total-mib "$MEM_TOTAL_MIB" --weights-mib "$WEIGHTS_MIB" --kv-mib "$KV_MIB"
           --ttl-s "$TTL_S" --label "${LABEL:-serve-$CONFIG}" --now "$(NOW_ISO)")
# 2026-09-05(G-B1): overhead 는 **선언 필수**다. 종전에는 안 넘기면 blackbox_session 의 기본값
#   12288 이 조용히 쓰였고, 그 값이 실측(gpt-oss-120b/GB10 17,971)보다 작아 선언 바닥을 높이고
#   정상 서빙을 워치독 무장 밴드에 넣었다. 여기서 죽는 편이 로드 중 사살보다 싸다.
if [ -z "$OVERHEAD_MIB" ]; then
  echo "$TAG 3/7 예산 선언       : FAIL — --overhead-mib 가 선언되지 않았다(기본값 없음)." >&2
  echo "$TAG     왜: 낮은 overhead 는 선언 바닥을 높여 정상 서빙을 사살 대상으로 만든다." >&2
  echo "$TAG     어떻게: --overhead-mib <n>. 모르면 로드 완료 후" >&2
  echo "$TAG     (MemTotal − MemAvailable) − weights − kv 를 재서 그 값을 쓴다." >&2
  exit 2
fi
DECL_ARGS+=(--overhead-mib "$OVERHEAD_MIB")
[ -n "$EXPECTED_LOAD_S" ] && DECL_ARGS+=(--expected-load-s "$EXPECTED_LOAD_S")
if [ "$DRY" = "1" ]; then
  echo "$TAG 3/7 예산 선언       : (dry-run) python3 $SESSION_PY ${DECL_ARGS[*]}"
elif out="$(python3 "$SESSION_PY" "${DECL_ARGS[@]}" 2>&1)"; then
  printf '%s\n' "$out" | sed "s/^/$TAG     /"
  echo "$TAG 3/7 예산 선언       : DONE"
  CLEANUP_NEEDED=1
else
  printf '%s\n' "$out" | sed "s/^/$TAG     /" >&2
  echo "$TAG 3/7 예산 선언       : FAIL — 선언 없이 로드하지 않는다(무방비 기동 금지)." >&2
  exit 2
fi

# ── 4/7 블랙박스 세션 ───────────────────────────────────────────────────────────────
MODEL_PATH="$(awk -F': *' '/^model:/{print $2; exit}' "$CFGYAML" | tr -d '[:space:]')"
# `--session-id` 는 blackbox_session 이 **요구**한다(선택 아님 — 2026-09-04 첫 실행에서 드러났다).
# 미지정 시 여기서 만든다: config + UTC 초. 결정론이며 사람이 읽을 수 있고, 같은 config 를 다시
# 올리면 다른 id 가 되어 세션이 겹치지 않는다(시각은 이미 주입점이 셸이다).
[ -n "$SESSION_ID" ] || SESSION_ID="serve-${CONFIG}-$(date -u +%Y%m%dT%H%M%SZ)"
SESS_ARGS=(--node-dir "$NODE_DIR" start --session-id "$SESSION_ID"
           --topology single --tp "$TP"
           --config-file "$CONFIG" --model "$(val SERVING_MODEL_NAME)" --model-path "$MODEL_PATH"
           --now "$(NOW_ISO)")
[ -n "$IMAGE_TAG" ] && SESS_ARGS+=(--image "$IMAGE_TAG")
if [ "$DRY" = "1" ]; then
  echo "$TAG 4/7 세션 start      : (dry-run) python3 $SESSION_PY ${SESS_ARGS[*]}"
elif out="$(python3 "$SESSION_PY" "${SESS_ARGS[@]}" 2>&1)"; then
  printf '%s\n' "$out" | sed "s/^/$TAG     /"
  echo "$TAG 4/7 세션 start      : DONE"
else
  printf '%s\n' "$out" | sed "s/^/$TAG     /" >&2
  echo "$TAG 4/7 세션 start      : FAIL — 관측 없이 로드하지 않는다." >&2
  rollback; exit 2
fi

# ── 5/7 compose up ──────────────────────────────────────────────────────────────────
if [ "$DRY" = "1" ]; then
  echo "$TAG 5/7 compose up      : (dry-run) docker compose -f $COMPOSE ${ENVFILES[*]} --profile serve up -d"
elif out="$(docker compose -f "$COMPOSE" "${ENVFILES[@]}" --profile serve up -d 2>&1)"; then
  echo "$TAG 5/7 compose up      : DONE"
else
  printf '%s\n' "$out" | sed "s/^/$TAG     /" >&2
  echo "$TAG 5/7 compose up      : FAIL" >&2
  rollback; exit 2
fi

# ── 6/7 예산 갱신 루프 ──────────────────────────────────────────────────────────────
if [ "$RENEW" != "1" ]; then
  echo "$TAG 6/7 갱신 루프       : SKIPPED — --no-renew-loop 로 명시 요청됨(TTL ${TTL_S}s 뒤 선언이 만료된다)"
elif [ "$DRY" = "1" ]; then
  echo "$TAG 6/7 갱신 루프       : (dry-run) bash $RENEW_SH --node-dir $NODE_DIR --container $CNAME --ttl-s $TTL_S"
else
  setsid nohup bash "$RENEW_SH" --node-dir "$NODE_DIR" --container "$CNAME" --ttl-s "$TTL_S" \
    >> "$NODE_DIR/budget_renew.log" 2>&1 < /dev/null &
  echo "$TAG 6/7 갱신 루프       : DONE (pid=$! · log=$NODE_DIR/budget_renew.log)"
fi

# ── 7/7 health 대기 ─────────────────────────────────────────────────────────────────
if [ "$DRY" = "1" ]; then
  echo "$TAG 7/7 health 대기     : (dry-run) http://localhost:$PORT/health · 최대 ${READY_MAX}s"
  echo "$TAG DRY-RUN 종료(변경 없음)"
  exit 0
fi
echo "$TAG 7/7 health 대기     : http://localhost:$PORT/health (최대 ${READY_MAX}s)"
_waited=0
while [ "$_waited" -lt "$READY_MAX" ]; do
  if [ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://localhost:$PORT/health" 2>/dev/null)" = "200" ]; then
    echo "$TAG 7/7 health 대기     : DONE (${_waited}s)"
    echo "$TAG READY container=$CNAME port=$PORT node_id=$NODE_ID session=$SESSION_ID"
    exit 0
  fi
  # 컨테이너가 죽었으면 더 기다리는 것은 거짓 인내다 — 즉시 실패로 간다.
  if ! docker ps --filter "name=^${CNAME}$" --filter status=running -q | grep -q .; then
    echo "$TAG 7/7 health 대기     : FAIL — 컨테이너가 종료됐다(로그 마지막 20줄):" >&2
    docker logs "$CNAME" 2>&1 | tail -20 | sed "s/^/$TAG     /" >&2 || true
    rollback; exit 2
  fi
  sleep 5; _waited=$(( _waited + 5 ))
done
echo "$TAG 7/7 health 대기     : FAIL — ${READY_MAX}s 내 READY 아님(타임아웃). 더 큰 모델이면 --ready-max 로 **명시** 연장하라." >&2
rollback; exit 5
