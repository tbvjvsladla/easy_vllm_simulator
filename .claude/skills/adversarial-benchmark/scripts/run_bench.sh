#!/usr/bin/env bash
# run_bench.sh — 실행 중인 vLLM serve 에 `vllm bench serve` 실행 → 결과 JSON + engine-log 캡처 (adversarial-benchmark §7)
#
# 이 스킬은 *돌고 있는* serve 를 검증한다(기동은 recipe-explorer/serve compose 담당 — §10 경계).
# 측정만: 컨테이너 내부 vllm bench serve(client-side 토크나이저 = 마운트된 모델 경로, airgap-safe) →
#   --num-warmups 로 콜드 JIT 폐기 → --save-result JSON → 호스트로 회수 + docker logs 교차캡처.
# 사용: run_bench.sh <config_name> [--topology single|multi] [--concurrency N] [--input-len N]
#                    [--output-len N] [--num-prompts N] [--warmups N] [--out-dir DIR]
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
# ★ 2026-09-01 신설 — backend 노브(기본값 openai-chat = 종전 동작, 후방호환).
#   왜: 정본 디코드 지표는 `1000/median_tpot_ms` 인데 **harmony 계열(gpt-oss)은 chat 엔드포인트에서
#   `--ignore-eos` 가 무력**하다 — harmony 의 assistant-action stop 토큰이 EOS 와 별개로 턴을 끝낸다.
#   그러면 요청당 생성이 목표보다 훨씬 짧아지고(실측: out-len 256 요청에 평균 41 토큰)
#   TPOT=(duration-TTFT)/(n-1) 의 분모가 작아져 **디코드가 3.3배 느린 것처럼** 측정된다
#   (client 10.33 t/s vs engine-log 34.1 t/s — parse_bench 의 client_engine_agreement 가 검출).
#   완결 엔드포인트(/v1/completions)에서는 같은 요청이 400/400 토큰을 낸다(finish_reason=length).
#   ∴ 측정 무효를 우회하지 않고 **경로를 고친다**(workflow.md §막힘 3분류 — 배선 부재는 배선을 만든다).
BACKEND="openai-chat"
# ★ 2026-09-04 신설(CP4 · plan_26090415 §3.1) — 측정 도구 선택.
#   기본은 `vllm`(현행 경로 · 후방호환). lite 는 이 기본에 남고 full 만 guidellm 으로 간다
#   (`full = lite ∪ GuideLLM`). 두 경로를 **이질적으로 유지**하는 것이 설계다 — 통합하면 같은
#   버그가 양쪽에 균일하게 먹어 "일치해 보이면서 둘 다 틀리는" 상태가 된다(실결함 2건이 이질성
#   덕에 잡혔다: 2026-09-01·09-03).
TOOL="vllm"
BENCH_BUDGET_MIB=""
TOPO=""; CONC=1; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; OUTDIR=""
while [ $# -gt 0 ]; do case "$1" in
  --tool) TOOL="$2"; shift 2;;
  --bench-budget-mib) BENCH_BUDGET_MIB="$2"; shift 2;;
  --topology) TOPO="$2"; shift 2;;
  --concurrency) CONC="$2"; shift 2;;
  --input-len) ILEN="$2"; shift 2;;
  --output-len) OLEN="$2"; shift 2;;
  --num-prompts) NPROMPTS="$2"; shift 2;;
  --warmups) WARMUPS="$2"; shift 2;;
  --out-dir) OUTDIR="$2"; shift 2;;
  --backend) BACKEND="$2"; shift 2;;
  *) echo "[run_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

# 도구 값역은 **어떤 일을 하기 전에** 친다. 게이트 뒤로 미루면 오타가 라이브 서빙 점검을 다 돌고
# 나서야 드러나고, 인자 평면만 시험하려는 가드가 그 오타를 검출할 수 없다.
case "$TOOL" in
  vllm) ;;
  guidellm)
    # 예산은 **선언에서만** 온다(§3.4·§4.8). 기본값을 두면 부하 생성기가 무제한으로 자라 serve 와
    # 같은 통합메모리를 두고 경쟁하는데 아무도 그것을 승인한 적이 없는 상태가 된다.
    # 이 검사를 **인자 평면**에 두는 이유: 게이트 뒤로 미루면 라이브 서빙 점검을 다 돌고 나서야
    # 드러나고, 인자만 시험하는 음성대조가 불가능해진다(그러면 가드가 다른 이유로 통과한다 —
    # judge_bench.sh 에서 실제로 그렇게 만들었다가 변이 시험에서 잡혔다).
    case "$BENCH_BUDGET_MIB" in
      ''|*[!0-9]*)
        echo "[run_bench] ERROR --tool guidellm 에는 --bench-budget-mib <양의 정수> 가 필수다(기본값 없음)." >&2
        echo "  벤치 컨테이너 몫은 serve 예산과 **나란히** 선언한다 — 합치면 서빙 자체의 점유를 읽을 수 없고" >&2
        echo "  부하 생성기의 순간 버스트가 정상상태 바닥 규칙에 섞여 위양성 트립을 만든다." >&2
        exit 2;;
    esac
    [ "$BENCH_BUDGET_MIB" -gt 0 ] || { echo "[run_bench] ERROR --bench-budget-mib 는 양수여야 한다" >&2; exit 2; }
    ;;
  *) echo "[run_bench] 알 수 없는 --tool: $TOOL (vllm|guidellm)" >&2; exit 2;;
esac

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "$TOPO" ]; then
  BR="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo)"
  case "$BR" in multi-node) TOPO=multi;; single-node) TOPO=single;; *) TOPO=single;; esac  # unknown→single(recipe.py _read_manifest 와 정합·보수적)
fi

# 헌법 §테라포밍-완수/A2A-위임 Flag 게이트 (plan_26063018·plan_26063021_14_37) — 결정론 백스톱(**fail-closed**).
# 면제 2경로(recipe.py _require_terraform_flag 와 동형): (1차) 서브 A2A 위임 *양성 키* .claude/a2a_delegation.json 존재
#   (메인이 동질성 검증 후 발급, 메인 키와 UNIQUE) · (2차) EASY_VLLM_A2A_DELEGATED env(테스트 override).
# 그 외 메인이면 manifest_contract --require-flag. 키·MC·Flag 모두 부재 = fail-closed info-only(옛 [ -f $MC ]-부재 skip 은 fail-open 이었음).
# ⚠ 서브에는 이 파일이 없다(설계) — `terraforming_node` 는 온보딩 스킬이라 배달되지 않는다.
#   서브는 아래 A2A 위임 *양성 키* 경로로 게이트를 통과하며, 키·MC·Flag 모두 부재면
#   fail-closed(exit 4)다. 즉 MC 경로 부재는 결함이 아니다 — **배달 목록에 넣지 마라**
#   (2026-09-03 명문화: 같은 가정을 하드코딩한 single_serve_down.sh 는 서브에서 실제로
#    죽었다. 그쪽은 블랙박스가 서브에 *있으므로* 경로만 갈렸던 것이고, 이쪽은 파일 자체가
#    없는 것이 계약이다 — 두 경우를 구분하라).
MC="$REPO/.claude/skills/terraforming_node/scripts/manifest_contract.py"
KEY="$REPO/.claude/a2a_delegation.json"
KEY_OK=0   # 존재 + 내용·역할 검증(D8: 손상/외부 파일로 게이트 우회 차단 — WARN-1)
[ -f "$KEY" ] && python3 -c "import json,sys;d=json.load(open('$KEY'));sys.exit(0 if d.get('delegation')=='main_cluster_flag' and d.get('issued_to')=='sub' else 1)" 2>/dev/null && KEY_OK=1
if [ "$KEY_OK" = 1 ] || [ "${EASY_VLLM_A2A_DELEGATED:-}" = "1" ]; then
  : # 유효 A2A 위임 키(내용·역할 검증) 또는 명시 테스트 override(정확히 "1" — '0'/'false' 오인 차단) → 면제
elif [ -f "$MC" ]; then
  if ! python3 "$MC" --topology "$TOPO" --repo "$REPO" --require-flag >/dev/null 2>&1; then
    echo "[run_bench] 테라포밍-완수 Flag 미발급 — info-only. terraforming_node 로 HW스캔·검증 먼저(또는 EASY_VLLM_A2A_DELEGATED=1)." >&2
    exit 4
  fi
else
  echo "[run_bench] A2A 위임 키·테라포밍 Flag 모두 부재 — info-only(fail-closed). terraforming_node 로 검증 먼저(또는 EASY_VLLM_A2A_DELEGATED=1)." >&2
  exit 4
fi
EF="$REPO/output/$TOPO/envs/.env.$CONFIG"
[ -f "$EF" ] || { echo "[run_bench] envfile 없음: $EF" >&2; exit 2; }
# shellcheck disable=SC1090
set -a; . "$EF"; set +a

PORT="${SERVING_PORT:?SERVING_PORT 미정}"
MODEL_NAME="${SERVING_MODEL_NAME:?SERVING_MODEL_NAME 미정}"
CFGFILE="${CONFIG_FILE:-$CONFIG}"
if [ "$TOPO" = "multi" ]; then
  CTR="${MASTER_CONTAINER_NAME:?MASTER_CONTAINER_NAME 미정}"; INPORT="$PORT"
else
  CTR="${CONTAINER_NAME:-vllm-serve-container}"; INPORT=8000
fi
CFGYAML="$REPO/output/$TOPO/configs/$CFGFILE.yaml"
[ -f "$CFGYAML" ] || { echo "[run_bench] config yaml 없음: $CFGYAML" >&2; exit 2; }
MODEL_PATH="$(awk -F': *' '/^model:/{print $2; exit}' "$CFGYAML" | tr -d '[:space:]')"
[ -n "$MODEL_PATH" ] || { echo "[run_bench] config yaml 의 model: 경로 파싱 실패" >&2; exit 2; }

OUTDIR="${OUTDIR:-$REPO/output/$TOPO/benchlog}"
mkdir -p "$OUTDIR"
ELOG="$OUTDIR/engine_${CONFIG}.log"   # BJSON 은 도구 분기가 정한다(스키마가 다르므로 파일명도 다르다)

# --- serve 가동 확인(이 스킬은 기동 안 함) ---
echo "[run_bench] precheck http://localhost:$PORT/health"
[ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://localhost:$PORT/health" 2>/dev/null)" = "200" ] \
  || { echo "[run_bench] serve 미가동(:$PORT/health≠200). recipe/compose 로 먼저 기동하세요." >&2; exit 3; }
docker ps --filter "name=$CTR" --filter status=running -q | grep -q . \
  || { echo "[run_bench] 컨테이너 $CTR 미실행" >&2; exit 3; }

case "$BACKEND" in
  openai-chat) ENDPOINT=/v1/chat/completions ;;
  openai)      ENDPOINT=/v1/completions ;;
  *) echo "[run_bench] 알 수 없는 --backend: $BACKEND (openai-chat|openai)" >&2; exit 2 ;;
esac

# ── 도구별 실행 ────────────────────────────────────────────────────────────────
#   두 분기는 **같은 게이트를 이미 통과한 뒤** 갈라진다(Flag/A2A · envfile · health · 컨테이너 생존).
#   게이트를 분기 안으로 복제하지 않는 것이 요점이다 — 정책 술어가 위 블록을 **원문 그대로 뽑아
#   실행**하며 검증하므로, 복제본은 검증되지 않는 두 번째 게이트가 된다.
bench_with_vllm(){
  echo "[run_bench] bench(vllm): ctr=$CTR inport=$INPORT model=$MODEL_NAME conc=$CONC in=$ILEN out=$OLEN n=$NPROMPTS warmup=$WARMUPS backend=$BACKEND endpoint=$ENDPOINT"
  local RFN="ab_bench_${CONFIG}.json"
  docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
  --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $ENDPOINT \
  --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
  --dataset-name random --random-input-len $ILEN --random-output-len $OLEN --random-range-ratio 0 \
  --num-prompts $NPROMPTS --max-concurrency $CONC --request-rate inf --ignore-eos --num-warmups $WARMUPS --temperature 0 \
  --save-result --result-dir /tmp --result-filename '$RFN'" \
    || { echo "[run_bench] vllm bench serve 실패" >&2; docker exec "$CTR" bash -lc "tail -5 /tmp/$RFN 2>/dev/null" || true; return 4; }
  docker exec "$CTR" cat "/tmp/$RFN" > "$BJSON"
}

# GuideLLM = **별도 컨테이너**. 이 스킬의 "기동하지 않는다" 불변식의 목적어는 **추론 서버**이며,
#   측정 도구 컨테이너는 teardown 계약 하에 이 스킬이 소유한다(SKILL.md §8).
#   소유 기준은 기동이 아니라 정리다 — 실질 실패모드는 "누가 띄웠나"가 아니라 "크래시가 컨테이너를
#   흘렸나"이므로 explorer run_trial 의 `finally: docker rm -f` 선례를 그대로 이식한다.
bench_with_guidellm(){
  # 예산 유효성은 **인자 평면**이 이미 쳤다(위 case). 여기서 다시 적으면 두 벌이 갈라진다.
  local PINJSON IMAGE PRC
  PINJSON="$(python3 "$SDIR/resolve_bench_tool.py" --tool guidellm --json 2>/dev/null)"; PRC=$?
  if [ "$PRC" != "0" ]; then
    python3 "$SDIR/resolve_bench_tool.py" --tool guidellm >/dev/null   # 사람이 읽을 사유를 stderr 로
    echo "[run_bench] 측정 도구 핀 해소 실패(rc=$PRC) — 실행하지 않는다" >&2
    return "$PRC"
  fi
  IMAGE="$(printf '%s' "$PINJSON" | python3 -c "import json,sys;print(json.load(sys.stdin)['image_ref'])")"

  # 토크나이저는 **호스트 경로**가 필요하다. serve 컨테이너 안의 /app/models/... 를 manifest 의
  # nas_model_path 로 되돌린다. 되돌리지 못하면 추측하지 않고 멈춘다.
  local NAS HOSTTOK
  NAS="$(sed -n 's/^[[:space:]]*nas_model_path:[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
          "$REPO/output/$TOPO/manifest.yaml" 2>/dev/null | head -1 | sed 's/[[:space:]]*$//')"
  case "$MODEL_PATH" in
    /app/models/*) HOSTTOK="${NAS%/}/${MODEL_PATH#/app/models/}" ;;
    *)             HOSTTOK="$MODEL_PATH" ;;
  esac
  [ -n "$NAS" ] || { echo "[run_bench] ERROR manifest 의 nas_model_path 를 읽지 못했다 — 토크나이저 호스트 경로를 추측하지 않는다" >&2; return 2; }
  [ -d "$HOSTTOK" ] || { echo "[run_bench] ERROR 토크나이저 호스트 경로 부재: $HOSTTOK" >&2; return 2; }

  # warmup 단위 변환. vLLM 은 **요청 수**로, GuideLLM 은 **비율/시간**으로 warmup 을 센다.
  # 같은 숫자를 그대로 넘기면 "2 요청"이 "2 초"가 되어 조용히 다른 것을 잰다 — 비율로 옮기고
  # 총 요청을 warmup 만큼 늘려 **집계 대상 수를 보존**한다(근사이며, 실제 집계 수는 산출물이 밝힌다).
  local TOTAL_REQ WARM_FRAC
  TOTAL_REQ=$(( NPROMPTS + WARMUPS ))
  WARM_FRAC="$(python3 -c "print(round($WARMUPS/max($TOTAL_REQ,1), 4))")"

  local GLNAME="guidellm-bench-${CONFIG}-c${CONC}"
  # teardown 계약: 정상·비정상·시그널 어느 경로로 나가도 컨테이너를 남기지 않는다.
  trap 'docker rm -f "$GLNAME" >/dev/null 2>&1 || true' RETURN
  docker rm -f "$GLNAME" >/dev/null 2>&1 || true

  echo "[run_bench] bench(guidellm): image=$IMAGE budget=${BENCH_BUDGET_MIB}MiB conc=$CONC in=$ILEN out=$OLEN n=$NPROMPTS(+warm $WARMUPS) endpoint=$ENDPOINT"
  # ★ 호스트 사용자로 돌린다. 이미지 기본 UID(1001)로 두면 산출물 디렉터리에 **쓰지 못해**
  #   벤치를 다 돌고 마지막 저장에서 죽는다(2026-09-04 실측: PermissionError /out/…json —
  #   측정은 성립했는데 기록이 사라지는, 가장 비싼 형태의 실패다).
  #   `chmod 777` 로 여는 대신 UID 를 맞춘다 — 산출물 소유가 호스트 사용자로 남아야
  #   teardown 의 "캐시 소유권 정렬" 단계와도 어긋나지 않는다.
  docker run --rm --name "$GLNAME" --network host \
    --user "$(id -u):$(id -g)" \
    --memory "${BENCH_BUDGET_MIB}m" --memory-swap "${BENCH_BUDGET_MIB}m" \
    -v "$HOSTTOK:/tok:ro" -v "$OUTDIR:/out" \
    -e HOME=/tmp -e XDG_CACHE_HOME=/tmp/.cache \
    -e HF_HUB_OFFLINE=1 -e TRANSFORMERS_OFFLINE=1 \
    --entrypoint guidellm "$IMAGE" run \
    --backend "kind=openai_http,target=http://localhost:$PORT,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
    --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
    --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
    --tokenizer "kind=huggingface_auto,model=/tok" \
    --constraint "kind=max_requests,count=$TOTAL_REQ" \
    --output "kind=json,path=/out/$(basename "$BJSON")" \
    --disable-progress \
    || { echo "[run_bench] guidellm run 실패" >&2; return 4; }
}

BENCH_TOOL_RC=0
case "$TOOL" in
  vllm)     BJSON="$OUTDIR/bench_${CONFIG}.json";    bench_with_vllm     || BENCH_TOOL_RC=$? ;;
  guidellm) BJSON="$OUTDIR/guidellm_${CONFIG}.json"; bench_with_guidellm || BENCH_TOOL_RC=$? ;;
  *) echo "[run_bench] 내부오류: --tool 값역 검사를 통과한 미지의 값 '$TOOL'" >&2; exit 2 ;;
esac
[ "$BENCH_TOOL_RC" = "0" ] || exit "$BENCH_TOOL_RC"

docker logs "$CTR" 2>&1 | tail -800 > "$ELOG" || true

echo "[run_bench] DONE"
echo "BENCH_TOOL=$TOOL"
echo "BENCH_JSON=$BJSON"
echo "ENGINE_LOG=$ELOG"
