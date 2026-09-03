#!/usr/bin/env bash
# lite_bench.sh — 경량(lite) inform-only 벤치 오케스트레이터 (adversarial-benchmark §5.5 · plan_26071115)
#
# lite = 기본 ON(서빙 성공 시 자동 수행) · inform-only(PASS/FAIL·자동 loop-back ✗ — verdict_rule 미투입).
# 돌고 있는 serve 를 경량 측정한다(기동 안 함 — run_bench.sh 와 동일 경계). full 벤치는 run_bench.sh.
#
# 수집: cold(무-warmup 단일요청 → cold TTFT) + warm burst(N=3·conc=1·warmup 1 → gen t/s) 2회 bench serve
#   + per-node nvidia-smi(통합메모리는 N/A → serve-log 폴백) + /proc/meminfo + master engine-log(KV).
#   multi = 서브 **SSH 읽기전용 probe**(nvidia-smi/proc·meminfo — health 폴링과 동형 관측평면, 재스캔 ✗).
# 산정·표 렌더는 결정론 lite_metrics.py.
#
# 사용: lite_bench.sh <config_name> [--topology single|multi] [--burst-n N] [--out-dir DIR] [--no-sub-probe]
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
TOPO=""; BURST_N=3; OUTDIR=""; SUB_PROBE=1; BACKEND="openai-chat"
# ★ 2026-09-01 신설 — run_bench.sh·sweep_bench.sh 와 같은 backend 노브(기본값 동일, 후방호환).
#   왜: harmony 계열(gpt-oss)은 chat 엔드포인트에서 `--ignore-eos` 가 무력해 생성이 조기 종료되고
#   median_tpot 이 크게 부풀려진다. 실측: 같은 서빙에서 lite(chat) 16.43 t/s vs full(completions)
#   34.55 t/s — **같은 인증서 안에서 두 배 넘게 갈렸다**. 세 스크립트 중 여기만 노브가 없으면
#   그 왜곡이 인증서의 lite_* 필드로 그대로 발행된다.
while [ $# -gt 0 ]; do case "$1" in
  --topology) TOPO="$2"; shift 2;;
  --backend) BACKEND="$2"; shift 2;;
  --burst-n) BURST_N="$2"; shift 2;;
  --out-dir) OUTDIR="$2"; shift 2;;
  --no-sub-probe) SUB_PROBE=0; shift;;
  *) echo "[lite_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done
case "$BACKEND" in
  openai-chat) LITE_ENDPOINT=/v1/chat/completions ;;
  openai)      LITE_ENDPOINT=/v1/completions ;;
  *) echo "[lite_bench] 알 수 없는 --backend: $BACKEND (openai-chat|openai)" >&2; exit 2 ;;
esac

REPO="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "$TOPO" ]; then
  BR="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo)"
  case "$BR" in multi-node) TOPO=multi;; single-node) TOPO=single;; *) TOPO=single;; esac  # unknown→single(run_bench.sh 정합)
fi

# 헌법 §테라포밍-완수/A2A-위임 Flag 게이트 — 결정론 백스톱(fail-closed, run_bench.sh 와 동형).
# lite 는 이미 Flag-게이트된 serve 위에서 돈다(§0.0 전이적 게이트) — 직접 진입도 동일 백스톱.
# ⚠ 서브에는 이 파일이 없다(설계) — `terraforming_node` 는 온보딩 스킬이라 배달되지 않는다.
#   서브는 아래 A2A 위임 *양성 키* 경로로 게이트를 통과하며, 키·MC·Flag 모두 부재면
#   fail-closed(exit 4)다. 즉 MC 경로 부재는 결함이 아니다 — **배달 목록에 넣지 마라**
#   (2026-09-03 명문화: 같은 가정을 하드코딩한 single_serve_down.sh 는 서브에서 실제로
#    죽었다. 그쪽은 블랙박스가 서브에 *있으므로* 경로만 갈렸던 것이고, 이쪽은 파일 자체가
#    없는 것이 계약이다 — 두 경우를 구분하라).
MC="$REPO/.claude/skills/terraforming_node/scripts/manifest_contract.py"
KEY="$REPO/.claude/a2a_delegation.json"
KEY_OK=0
[ -f "$KEY" ] && python3 -c "import json,sys;d=json.load(open('$KEY'));sys.exit(0 if d.get('delegation')=='main_cluster_flag' and d.get('issued_to')=='sub' else 1)" 2>/dev/null && KEY_OK=1
if [ "$KEY_OK" = 1 ] || [ "${EASY_VLLM_A2A_DELEGATED:-}" = "1" ]; then
  :  # 유효 A2A 위임 키 또는 테스트 override → 면제
elif [ -f "$MC" ]; then
  python3 "$MC" --topology "$TOPO" --repo "$REPO" --require-flag >/dev/null 2>&1 || {
    echo "[lite_bench] 테라포밍-완수 Flag 미발급 — info-only. terraforming_node 로 검증 먼저(또는 EASY_VLLM_A2A_DELEGATED=1)." >&2; exit 4; }
else
  echo "[lite_bench] A2A 위임 키·테라포밍 Flag 모두 부재 — info-only(fail-closed)." >&2; exit 4
fi

EF="$REPO/output/$TOPO/envs/.env.$CONFIG"
[ -f "$EF" ] || { echo "[lite_bench] envfile 없음: $EF" >&2; exit 2; }
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
[ -f "$CFGYAML" ] || { echo "[lite_bench] config yaml 없음: $CFGYAML" >&2; exit 2; }
MODEL_PATH="$(awk -F': *' '/^model:/{print $2; exit}' "$CFGYAML" | tr -d '[:space:]')"
[ -n "$MODEL_PATH" ] || { echo "[lite_bench] config yaml 의 model: 경로 파싱 실패" >&2; exit 2; }

OUTDIR="${OUTDIR:-$REPO/output/$TOPO/benchlog}"; mkdir -p "$OUTDIR"
WARM="$OUTDIR/lite_warm_${CONFIG}.json"; COLD="$OUTDIR/lite_cold_${CONFIG}.json"
ELOG="$OUTDIR/lite_engine_${CONFIG}.log"; RAW="$OUTDIR/lite_raw_${CONFIG}.json"

# serve 가동 확인(이 스킬은 기동 안 함).
echo "[lite_bench] precheck http://localhost:$PORT/health"
[ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://localhost:$PORT/health" 2>/dev/null)" = "200" ] \
  || { echo "[lite_bench] serve 미가동(:$PORT/health≠200). recipe/compose 로 먼저 기동." >&2; exit 3; }
docker ps --filter "name=$CTR" --filter status=running -q | grep -q . \
  || { echo "[lite_bench] 컨테이너 $CTR 미실행" >&2; exit 3; }

_bench() {  # $1=out.json $2=num-prompts $3=warmups
  docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
    --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
    --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
    --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
    --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
    --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \
    && docker exec "$CTR" cat /tmp/lite_tmp.json > "$1"
}

echo "[lite_bench] cold(단일·warmup0) + warm burst(N=$BURST_N·conc1·warmup1) ..."
_bench "$COLD" 1 0 || { echo "[lite_bench] cold bench 실패" >&2; : > "$COLD"; }
_bench "$WARM" "$BURST_N" 1 || { echo "[lite_bench] warm bench 실패" >&2; : > "$WARM"; }
docker logs "$CTR" 2>&1 | tail -800 > "$ELOG" || true

# ── per-node readings 수집(읽기전용) ──────────────────────────────────────────
_smi() {  # nvidia-smi 메모리(통합메모리는 N/A 반환 — 값 그대로 캡처, lite_metrics 가 폴백)
  nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits 2>/dev/null \
    | awk -F', *' 'NR==1{print $1"|"$2; exit}'
}
_meminfo() { awk '/MemTotal:/{t=$2} /MemAvailable:/{a=$2} END{print t"|"a}' /proc/meminfo 2>/dev/null; }
# _jnum: 정수만 그대로, 그 외(빈값·"[N/A]"·통합메모리 nvidia-smi 비수치)는 JSON null 로 방어(malformed JSON 차단).
_jnum() { case "$1" in ''|*[!0-9]*) printf null;; *) printf '%s' "$1";; esac; }

MAIN_SMI="$(_smi || true)"; MAIN_MEM="$(_meminfo || true)"
MU="${MAIN_SMI%%|*}"; MT="${MAIN_SMI#*|}"; [ "$MAIN_SMI" = "$MU" ] && { MU=""; MT=""; }
RMT="${MAIN_MEM%%|*}"; RMA="${MAIN_MEM#*|}"

NODES_JSON="{\"role\":\"main\",\"gpu_smi_used_mib\":$(_jnum "$MU"),\"gpu_smi_total_mib\":$(_jnum "$MT"),\"ram_total_kib\":$(_jnum "$RMT"),\"ram_avail_kib\":$(_jnum "$RMA")}"

if [ "$TOPO" = "multi" ] && [ "$SUB_PROBE" = 1 ]; then
  # 서브 식별 해소(단일계약): env-file > manifest nodes[sub] > 폴백 (multinode_serve_smoke.sh 동형)
  _mf_sub() { local m="$REPO/output/multi/manifest.yaml"; [ -f "$m" ] || return 1
    awk -v f="$1" '
      /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ {in_sub=1; next}
      /^[[:space:]]*-[[:space:]]*role:/ {in_sub=0}
      in_sub && $1==f":" {print $2; exit}' "$m"; }
  SLAVE_IP="${SLAVE_HOST_IP:-$(_mf_sub host)}"
  SSH_USER="${SSH_USER:-$(_mf_sub ssh_user)}"; SSH_USER="${SSH_USER:-$(id -un)}"
  SUB_HOST="${SUB_HOST:-${SSH_USER}@${SLAVE_IP}}"
  SSH="ssh -o BatchMode=yes -o ConnectTimeout=8"
  if [ -n "$SLAVE_IP" ] && $SSH "$SUB_HOST" 'echo ok' >/dev/null 2>&1; then
    S_SMI="$($SSH "$SUB_HOST" "nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits 2>/dev/null | awk -F', *' 'NR==1{print \$1\"|\"\$2; exit}'" 2>/dev/null || true)"
    S_MEM="$($SSH "$SUB_HOST" "awk '/MemTotal:/{t=\$2} /MemAvailable:/{a=\$2} END{print t\"|\"a}' /proc/meminfo" 2>/dev/null || true)"
    SU="${S_SMI%%|*}"; ST="${S_SMI#*|}"; [ "$S_SMI" = "$SU" ] && { SU=""; ST=""; }
    SRT="${S_MEM%%|*}"; SRA="${S_MEM#*|}"
    NODES_JSON="$NODES_JSON,{\"role\":\"sub\",\"probe_ok\":true,\"gpu_smi_used_mib\":$(_jnum "$SU"),\"gpu_smi_total_mib\":$(_jnum "$ST"),\"ram_total_kib\":$(_jnum "$SRT"),\"ram_avail_kib\":$(_jnum "$SRA")}"
  else
    echo "[lite_bench] ⚠ 서브 SSH probe 실패 — 마스터 단독 수집(음성정직 표기)" >&2
    NODES_JSON="$NODES_JSON,{\"role\":\"sub\",\"probe_ok\":false}"
  fi
fi

cat > "$RAW" <<JSON
{"topology":"$TOPO","burst_n":$BURST_N,
 "bench_warm_json":"$WARM","bench_cold_json":"$COLD","engine_log":"$ELOG",
 "nodes":[$NODES_JSON]}
JSON

echo "[lite_bench] 표 렌더(inform-only):"
python3 "$REPO/.claude/skills/adversarial-benchmark/scripts/lite_metrics.py" --raw-json "$RAW"
echo ""
echo "[lite_bench] DONE  RAW=$RAW  (inform-only — PASS/FAIL 없음)"
