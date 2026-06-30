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
TOPO=""; CONC=1; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; OUTDIR=""
while [ $# -gt 0 ]; do case "$1" in
  --topology) TOPO="$2"; shift 2;;
  --concurrency) CONC="$2"; shift 2;;
  --input-len) ILEN="$2"; shift 2;;
  --output-len) OLEN="$2"; shift 2;;
  --num-prompts) NPROMPTS="$2"; shift 2;;
  --warmups) WARMUPS="$2"; shift 2;;
  --out-dir) OUTDIR="$2"; shift 2;;
  *) echo "[run_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

REPO="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "$TOPO" ]; then
  BR="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo)"
  case "$BR" in multi-node) TOPO=multi;; single-node) TOPO=single;; *) TOPO=single;; esac  # unknown→single(recipe.py _read_manifest 와 정합·보수적)
fi

# 헌법 §테라포밍-완수 Flag 게이트 (plan_2026063018_1) — 결정론 백스톱. bench·verdict 는 돌고 있는 serve
# 전제(전이적 게이트)이나 직접 진입도 Flag 확인 → 미발급이면 info-only(작업 거부). manifest_contract 부재(서브
# 에어갭 — terraforming_node 는 main-only) 또는 EASY_VLLM_SKIP_FLAG_GATE 설정 시 우회(서브는 A2A 권한 전제).
MC="$REPO/.claude/skills/terraforming_node/scripts/manifest_contract.py"
if [ -z "${EASY_VLLM_SKIP_FLAG_GATE:-}" ] && [ -f "$MC" ]; then
  if ! python3 "$MC" --topology "$TOPO" --repo "$REPO" --require-flag >/dev/null 2>&1; then
    echo "[run_bench] 테라포밍-완수 Flag 미발급 — info-only. terraforming_node 로 HW스캔·검증 먼저(또는 EASY_VLLM_SKIP_FLAG_GATE=1)." >&2
    exit 4
  fi
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

echo "[run_bench] bench: ctr=$CTR inport=$INPORT model=$MODEL_NAME conc=$CONC in=$ILEN out=$OLEN n=$NPROMPTS warmup=$WARMUPS"
RFN="ab_bench_${CONFIG}.json"
docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
  --backend openai-chat --base-url http://localhost:$INPORT --endpoint /v1/chat/completions \
  --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
  --dataset-name random --random-input-len $ILEN --random-output-len $OLEN --random-range-ratio 0 \
  --num-prompts $NPROMPTS --max-concurrency $CONC --request-rate inf --ignore-eos --num-warmups $WARMUPS \
  --save-result --result-dir /tmp --result-filename '$RFN'" \
  || { echo "[run_bench] vllm bench serve 실패" >&2; docker exec "$CTR" bash -lc "tail -5 /tmp/$RFN 2>/dev/null" || true; exit 4; }

docker exec "$CTR" cat "/tmp/$RFN" > "$BJSON"
docker logs "$CTR" 2>&1 | tail -800 > "$ELOG" || true

echo "[run_bench] DONE"
echo "BENCH_JSON=$BJSON"
echo "ENGINE_LOG=$ELOG"
