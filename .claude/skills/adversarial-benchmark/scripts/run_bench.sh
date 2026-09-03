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
TOPO=""; CONC=1; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; OUTDIR=""
while [ $# -gt 0 ]; do case "$1" in
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
BJSON="$OUTDIR/bench_${CONFIG}.json"; ELOG="$OUTDIR/engine_${CONFIG}.log"

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
echo "[run_bench] bench: ctr=$CTR inport=$INPORT model=$MODEL_NAME conc=$CONC in=$ILEN out=$OLEN n=$NPROMPTS warmup=$WARMUPS backend=$BACKEND endpoint=$ENDPOINT"
RFN="ab_bench_${CONFIG}.json"
docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
  --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $ENDPOINT \
  --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
  --dataset-name random --random-input-len $ILEN --random-output-len $OLEN --random-range-ratio 0 \
  --num-prompts $NPROMPTS --max-concurrency $CONC --request-rate inf --ignore-eos --num-warmups $WARMUPS --temperature 0 \
  --save-result --result-dir /tmp --result-filename '$RFN'" \
  || { echo "[run_bench] vllm bench serve 실패" >&2; docker exec "$CTR" bash -lc "tail -5 /tmp/$RFN 2>/dev/null" || true; exit 4; }

docker exec "$CTR" cat "/tmp/$RFN" > "$BJSON"
docker logs "$CTR" 2>&1 | tail -800 > "$ELOG" || true

echo "[run_bench] DONE"
echo "BENCH_JSON=$BJSON"
echo "ENGINE_LOG=$ELOG"
