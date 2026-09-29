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
# ★ 경량 리포트 발행(2026-09-14 · plan_26091407 §4.5 · 사용자 결정 Q4·Q10) — **`--publish-report` 를 줄 때만** 종결부에서
#   `render_report.py --lite-only` 로 `docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`(헤더 `mode: lite`
#   · 측정 구성 표 · lite 지표 표)를 발행한다. lite 만 잰 셀의 hint(`hint_map_only`)가 이 문서를 바인딩한다.
#   기본값이 발행하지 않는 이유(자동 경로에 부작용을 만들지 않는다):
#     ① 서빙 성공 직후 자동 lite 핸드오프는 헌법의 **관측·inform-only 한정** 예외다 — 문서 발행이라는 산출을 거기에 얹지 않는다.
#     ② full 스윕(`sweep_bench.sh`)이 이 스크립트를 lite 레그로 부른다. 레그가 리포트를 내면 같은 시간대·같은 조합의 full
#        리포트가 `_MM_SS` 접미로 밀려 인증서와 stem 이 갈라지고, publisher 가 PUBLISH_BENCHMARK_REPORT_CERTIFICATE_STEM_MISMATCH
#        로 거부한다(full 발행 체인이 lite 때문에 깨진다). full 리포트는 lite 표를 이미 품는다(full ⊇ lite).
#     ③ 발행 실패(명명 키 불성립·충돌)가 lite 측정 자체를 실패로 만들지 않게, 요청한 호출에서만 exit 5 로 알린다
#        (측정 산출물 raw/warm/cold/엔진 로그는 그대로 남는다).
#
# ★ lite 판정 = full 의 진입 게이트(2026-09-29 · plan_26092923 · 인터뷰 interview_20260929_132122) — lite ⊂ full.
#   측정 뒤 `lite_metrics.py --judge` 가 raw 에 `lite_verdict`(pass | measurement_path_failed | server_failed)를 적고,
#   이 스크립트의 종료코드는 **그 값에서 파생**한다(0 / 6 / 7). 단일 권위는 raw 이고 호출부(sweep_bench·인증서 발행기)는
#   raw 부재·어휘 밖 값·모르는 종료코드를 불통과로 읽는다. 종전에는 측정 0건도 rc 0 이었다(D1 · 2026-09-29 실측).
#   행동은 진입 경로가 정한다 — α 서빙 직후 자동 핸드오프는 판정을 **기록·보고만** 한다(인증서 ✗ · 재빌드 ✗ · 헌법 트리거 절).
#   β lite-only 캠페인 셀은 `--publish-report` 로 리포트를, 판정이 pass 일 때만 lite 등급 인증서를 낸다. γ 강등 셀은 sweep 경로다.
#
# 사용: lite_bench.sh <config_name> [--topology single|multi] [--serve-plane docker|native] [--host-endpoint URL]
#        [--engine-log PATH(native 전용 · 서버 로그)] [--burst-n N] [--out-dir DIR] [--no-sub-probe] [--publish-report]
# 종료: 0=lite 통과(요청 시 리포트·lite 인증서 발행 포함) · 2=인자/파일 부재 · 3=serve 미가동 · 4=정체성/Flag 게이트
#       5=리포트·인증서 발행 실패(측정·판정은 남음) · 6=lite 불통과 ① 측정 경로 불성립(하네스) · 7=lite 불통과 ② 서버 응답 실패
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
TOPO=""; SERVE_PLANE="docker"; HOST_ENDPOINT=""; CLIENT_VLLM=""; NATIVE_ELOG=""; BURST_N=3; OUTDIR=""; SUB_PROBE=1; BACKEND="openai-chat"; PUBLISH_REPORT=0
# ★ 2026-09-01 신설 — run_bench.sh·sweep_bench.sh 와 같은 backend 노브(기본값 동일, 후방호환).
#   왜: harmony 계열(gpt-oss)은 chat 엔드포인트에서 `--ignore-eos` 가 무력해 생성이 조기 종료되고
#   median_tpot 이 크게 부풀려진다. 실측: 같은 서빙에서 lite(chat) 16.43 t/s vs full(completions)
#   34.55 t/s — **같은 인증서 안에서 두 배 넘게 갈렸다**. 세 스크립트 중 여기만 노브가 없으면
#   그 왜곡이 인증서의 lite_* 필드로 그대로 발행된다.
while [ $# -gt 0 ]; do case "$1" in
  --topology) TOPO="$2"; shift 2;;
  --serve-plane) SERVE_PLANE="$2"; shift 2;;
  --host-endpoint) HOST_ENDPOINT="$2"; shift 2;;
  --client-vllm) CLIENT_VLLM="$2"; shift 2;;
  --engine-log) NATIVE_ELOG="$2"; shift 2;;
  --backend) BACKEND="$2"; shift 2;;
  --burst-n) BURST_N="$2"; shift 2;;
  --out-dir) OUTDIR="$2"; shift 2;;
  --no-sub-probe) SUB_PROBE=0; shift;;
  --publish-report) PUBLISH_REPORT=1; shift;;
  *) echo "[lite_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done
case "$SERVE_PLANE" in
  docker|native) ;;
  *) echo "[lite_bench] 알 수 없는 --serve-plane: $SERVE_PLANE (docker|native)" >&2; exit 2;;
esac
if [ "$SERVE_PLANE" = "native" ] && { [ -z "$HOST_ENDPOINT" ] || [ -z "$CLIENT_VLLM" ]; }; then
  echo "[lite_bench] ERROR --serve-plane native에는 --host-endpoint URL 과 --client-vllm <전용 venv/bin/vllm>이 필수다." >&2; exit 2
fi
if [ "$SERVE_PLANE" = "native" ] && [[ ! "$HOST_ENDPOINT" =~ ^https?://[^/[:space:]]+(:[0-9]+)?$ ]]; then
  echo "[lite_bench] ERROR --host-endpoint는 경로 없는 http(s) origin 이어야 한다: $HOST_ENDPOINT" >&2; exit 2
fi
if [ "$SERVE_PLANE" = "docker" ] && { [ -n "$HOST_ENDPOINT" ] || [ -n "$CLIENT_VLLM" ] || [ -n "$NATIVE_ELOG" ]; }; then
  echo "[lite_bench] ERROR --host-endpoint/--client-vllm/--engine-log은 --serve-plane native에서만 준다." >&2; exit 2
fi
if [ -n "$NATIVE_ELOG" ] && [ ! -f "$NATIVE_ELOG" ]; then
  echo "[lite_bench] ERROR --engine-log 파일이 없다: $NATIVE_ELOG" >&2; exit 2
fi
if [ "$SERVE_PLANE" = "native" ] && { [ ! -x "$CLIENT_VLLM" ] || [ -L "$CLIENT_VLLM" ]; }; then
  echo "[lite_bench] ERROR --client-vllm이 실행 가능한 regular non-symlink 파일이 아니다: $CLIENT_VLLM" >&2; exit 2
fi
case "$BACKEND" in
  openai-chat) LITE_ENDPOINT=/v1/chat/completions ;;
  openai)      LITE_ENDPOINT=/v1/completions ;;
  *) echo "[lite_bench] 알 수 없는 --backend: $BACKEND (openai-chat|openai)" >&2; exit 2 ;;
esac

REPO="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "$TOPO" ]; then
  # ★ 2026-09-14(⑧ 분석 발견 T8 · 헌법 "토폴로지는 manifest 에서 읽고 브랜치로 추론하지 않는다"): 종전 관용구는
  #   브랜치 이름에서 토폴로지를 골랐고 알 수 없는 이름은 조용히 single 로 떨어졌다. 해소는 공용 해소기가 한다 —
  #   서명 카드 노드는 카드↔자기 manifest, 메인은 4자일치 술어(topology_parity), 둘 다 아니면 fail-loud.
  #   아래 Flag 게이트와 같은 평면이다: manifest 가 없으면(3) 미테라포밍 info-only, 정하지 못하면 fail-closed — 둘 다 exit 4.
  _TOPO_RC=0; TOPO="$(python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/resolve_topology.py" --repo "$REPO")" || _TOPO_RC=$?
  if [ "$_TOPO_RC" = 3 ]; then
    echo "[lite_bench] 토폴로지 사실(manifest) 부재 — 미테라포밍 info-only. terraforming_node 로 HW스캔·검증 먼저." >&2
    exit 4
  elif [ "$_TOPO_RC" != 0 ]; then
    echo "[lite_bench] 토폴로지를 정하지 못했다(위 사유) — info-only(fail-closed). --topology single|multi 로 명시할 수 있다." >&2
    exit 4
  fi
fi

# Terraform readiness is manifest/Flag/topology-based. SSH authenticates a provisioned
# endpoint; unsigned Agent Card metadata is not an execution credential.
MC="$REPO/.claude/skills/terraforming_node/scripts/manifest_contract.py"
if [ ! -f "$MC" ] || ! python3 "$MC" --topology "$TOPO" --repo "$REPO" --require-flag >/dev/null 2>&1; then
  echo "[lite_bench] 테라포밍-완수 Flag 또는 topology 준비성 미확인 — info-only(fail-closed)." >&2
  exit 4
fi

EF="$REPO/output/$TOPO/envs/.env.$CONFIG"
[ -f "$EF" ] || { echo "[lite_bench] envfile 없음: $EF" >&2; exit 2; }
# shellcheck disable=SC1090
set -a; . "$EF"; set +a

PORT="${SERVING_PORT:?SERVING_PORT 미정}"
MODEL_NAME="${SERVING_MODEL_NAME:?SERVING_MODEL_NAME 미정}"
CFGFILE="${CONFIG_FILE:-$CONFIG}"
if [ "$SERVE_PLANE" = "docker" ]; then
  if [ "$TOPO" = "multi" ]; then
    CTR="${MASTER_CONTAINER_NAME:?MASTER_CONTAINER_NAME 미정}"; INPORT="$PORT"
  else
    CTR="${CONTAINER_NAME:-vllm-serve-container}"; INPORT=8000
  fi
else
  CTR=""; INPORT=""
fi
CFGYAML="$REPO/output/$TOPO/configs/$CFGFILE.yaml"
[ -f "$CFGYAML" ] || { echo "[lite_bench] config yaml 없음: $CFGYAML" >&2; exit 2; }
MODEL_PATH="$(awk -F': *' '/^model:/{print $2; exit}' "$CFGYAML" | tr -d '[:space:]')"
[ -n "$MODEL_PATH" ] || { echo "[lite_bench] config yaml 의 model: 경로 파싱 실패" >&2; exit 2; }

OUTDIR="${OUTDIR:-$REPO/output/$TOPO/benchlog}"; mkdir -p "$OUTDIR"
WARM="$OUTDIR/lite_warm_${CONFIG}.json"; COLD="$OUTDIR/lite_cold_${CONFIG}.json"
ELOG="$OUTDIR/lite_engine_${CONFIG}.log"; RAW="$OUTDIR/lite_raw_${CONFIG}.json"
BLOG="$OUTDIR/lite_client_${CONFIG}.log"   # vllm bench serve 출력(판정 입력: 결과 파일 없이 끝났을 때의 사유)
# 이전 측정의 판정 입력이 이번 판정에 섞이지 않게 비운다(부재 = 이번 측정이 쓰지 않았다).
rm -f "$RAW" "$COLD" "$WARM"; : > "$BLOG"

# serve 가동 확인(이 스킬은 기동 안 함).
if [ "$SERVE_PLANE" = "docker" ]; then
  BASE_URL="http://localhost:$PORT"
  echo "[lite_bench] precheck $BASE_URL/health"
  [ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' "$BASE_URL/health" 2>/dev/null)" = "200" ] \
    || { echo "[lite_bench] serve 미가동(:$PORT/health≠200). recipe/compose 로 먼저 기동." >&2; exit 3; }
  docker ps --filter "name=$CTR" --filter status=running -q | grep -q . \
    || { echo "[lite_bench] 컨테이너 $CTR 미실행" >&2; exit 3; }
else
  BASE_URL="${HOST_ENDPOINT%/}"
  echo "[lite_bench] native precheck $BASE_URL/health"
  [ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' "$BASE_URL/health" 2>/dev/null)" = "200" ] \
    || { echo "[lite_bench] native serve 미가동($BASE_URL/health≠200). native producer로 먼저 기동." >&2; exit 3; }
fi

_bench() {  # $1=out.json $2=num-prompts $3=warmups
  if [ "$SERVE_PLANE" = "docker" ]; then
    # 클라이언트는 **서빙 컨테이너 안**에서 돈다 — 주소는 컨테이너 안의 포트(INPORT)다. 호스트 포트($PORT)는
    #   single 의 bridge 포트매핑에서 컨테이너 안에 없다(2026-09-29 plan_26092919 P2 실측: 연결 거부로 측정 0건 ·
    #   232f988 native 평면 도입 때 `localhost:$INPORT` 가 호스트 BASE_URL 로 바뀐 회귀). multi 는 host 네트워크라
    #   INPORT=PORT 로 같은 값이다.
    docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
      --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
      --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
      --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
      --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
      --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \
      && docker exec "$CTR" cat /tmp/lite_tmp.json > "$1"
  else
    "$CLIENT_VLLM" bench serve --backend "$BACKEND" --base-url "$BASE_URL" --endpoint "$LITE_ENDPOINT" \
      --model "$MODEL_NAME" --tokenizer "$MODEL_PATH" --trust-remote-code \
      --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
      --num-prompts "$2" --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups "$3" \
      --save-result --result-dir "$(dirname "$1")" --result-filename "$(basename "$1")"
  fi
}

# 측정시각 — 리포트 이름의 시간 토큰이자 "같은 측정인가" 판정 근거다(doc_naming). 부하를 걸기 직전의 호스트 UTC 를
#   **측정 사실**로 raw 에 남긴다(sweep_bench 의 run 경계 시각과 같은 자격 · 리포트 렌더러는 이 값을 날조하지 않고 요구한다).
MEASURED_UTC="$(date -u +%FT%TZ)"
echo "[lite_bench] cold(단일·warmup0) + warm burst(N=$BURST_N·conc1·warmup1) ..."
COLD_RC=0; _bench "$COLD" 1 0 >>"$BLOG" 2>&1 || COLD_RC=$?
[ "$COLD_RC" = 0 ] || { echo "[lite_bench] cold bench 실패(rc $COLD_RC) — 출력 끝:" >&2; tail -5 "$BLOG" >&2; rm -f "$COLD"; }
WARM_RC=0; _bench "$WARM" "$BURST_N" 1 >>"$BLOG" 2>&1 || WARM_RC=$?
[ "$WARM_RC" = 0 ] || { echo "[lite_bench] warm bench 실패(rc $WARM_RC) — 출력 끝:" >&2; tail -5 "$BLOG" >&2; rm -f "$WARM"; }
if [ "$SERVE_PLANE" = "docker" ]; then
  docker logs "$CTR" 2>&1 | tail -800 > "$ELOG" || true
elif [ -n "$NATIVE_ELOG" ]; then
  tail -800 "$NATIVE_ELOG" > "$ELOG" || true   # native producer 의 서버 로그(선언으로 받는다 · 경로 추측 ✗)
else
  : > "$ELOG"  # native producer owns server-log proof; 선언이 없으면 KV·VRAM 이 N/A 라 판정은 ① 이 된다(정직한 부재).
fi

# ── 판정 입력: 측정 뒤 두 평면의 health ────────────────────────────────────────────
#   호스트 평면 = precheck 와 같은 주소 · 클라이언트 평면 = 측정 클라이언트가 **도는 자리에서** 본 주소.
#   둘이 갈리면(호스트 200 · 클라이언트 미도달) 요청이 서버에 닿지 못한 것이다 — D1(2026-09-29)이 정확히 그 모양이었다.
HOST_HEALTH_AFTER="$(curl -s -m 5 -o /dev/null -w '%{http_code}' "$BASE_URL/health" 2>/dev/null || true)"
if [ "$SERVE_PLANE" = "docker" ]; then
  CLIENT_PLANE_HEALTH="$(docker exec "$CTR" python3 -c "import sys,urllib.request
try:
    print(urllib.request.urlopen('http://localhost:$INPORT/health', timeout=5).status)
except Exception as e:
    print('unreachable')" 2>/dev/null || echo unreachable)"
else
  CLIENT_PLANE_HEALTH="$HOST_HEALTH_AFTER"   # native 클라이언트는 호스트에서 돈다 — 같은 평면
fi
CLIENT_PLANE_HEALTH="$(printf '%s' "$CLIENT_PLANE_HEALTH" | tail -1 | tr -cd '[:alnum:]_')"

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

if [ "$SERVE_PLANE" = "docker" ] && [ "$TOPO" = "multi" ] && [ "$SUB_PROBE" = 1 ]; then
  # 서브 식별 해소(단일계약): env-file > manifest nodes[sub] > 폴백 (multinode_serve_smoke.sh 동형)
  _mf_sub() { local m="$REPO/output/multi/manifest.yaml"; [ -f "$m" ] || return 1
    awk -v f="$1" '
      /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ {in_sub=1; next}
      /^[[:space:]]*-[[:space:]]*role:/ {in_sub=0}
      in_sub && $1==f":" {v=$2; sub(/#.*/,"",v); gsub(/["\047\r]/,"",v); print v; exit}' "$m"; }
  # ↑ 2026-09-23: YAML 따옴표 값(`host: "<sub-ip>"`)을 벗기지 않아 `"cona"@"…"` 로 SSH 해 probe 가 늘 실패했다
  #   (camp-26092301 D1·D2 sub probe_ok=false). multinode_serve_smoke.sh `_mf_node` 와 같은 정규화다.
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

# 명명 입력(config_name·config_yaml·env_file·manifest)은 경량 리포트가 모델·GPU·버전 축을 **파생**하는 자리다
#   (sweep_bench 조립부와 같은 규칙 · render_report.lite_identity). 값을 여기서 미리 해석하지 않고 경로만 남긴다.
BERR_TAIL="$(tail -c 4000 "$BLOG" 2>/dev/null | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"
cat > "$RAW" <<JSON
{"topology":"$TOPO","serve_plane":"$SERVE_PLANE","host_endpoint":"$BASE_URL","burst_n":$BURST_N,
 "config_name":"$CONFIG","measured_utc":"$MEASURED_UTC","backend":"$BACKEND","endpoint":"$LITE_ENDPOINT",
 "config_yaml":"$CFGYAML","env_file":"$EF","manifest":"$REPO/output/$TOPO/manifest.yaml",
 "bench_warm_json":"$WARM","bench_cold_json":"$COLD","engine_log":"$ELOG",
 "bench_cold_rc":$COLD_RC,"bench_warm_rc":$WARM_RC,"bench_client_log":"$BLOG","bench_stderr_tail":$BERR_TAIL,
 "host_health_after":"$HOST_HEALTH_AFTER","client_plane_health":"$CLIENT_PLANE_HEALTH",
 "nodes":[$NODES_JSON]}
JSON

echo "[lite_bench] 표 렌더 + lite 판정:"
LM="$REPO/.claude/skills/adversarial-benchmark/scripts/lite_metrics.py"
python3 "$LM" --raw-json "$RAW" --judge || { echo "[lite_bench] 판정기 실패 — raw 에 판정이 없다(호출부는 불통과로 읽는다)" >&2; exit 6; }
echo ""
# 종료코드는 raw 의 판정에서 **파생**한다(두 자리가 갈라질 수 없다 — 같은 값을 읽는다).
LITE_VERDICT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("lite_verdict",""))' "$RAW" 2>/dev/null || true)"
case "$LITE_VERDICT" in
  pass) LITE_RC=0 ;;
  server_failed) LITE_RC=7 ;;
  *) LITE_RC=6 ;;   # measurement_path_failed · 판독 불가 — 부재를 통과로 접지 않는다
esac
if [ "$PUBLISH_REPORT" = "1" ]; then
  # 리포트는 판정과 무관하게 발행한다("왜 실패했나"도 사람이 본다 — full report 와 같은 규율). 인증서는 pass 만.
  if REPORT_PATH="$(python3 "$REPO/.claude/skills/adversarial-benchmark/scripts/render_report.py" --lite-only --lite-raw-json "$RAW")"; then
    echo "[lite_bench] 경량 리포트 발행 → $REPORT_PATH (mode: lite · lite_verdict=$LITE_VERDICT)"
  else
    echo "[lite_bench] ⚠ 경량 리포트 발행 실패(위 사유) — 측정 산출물은 남았다: RAW=$RAW" >&2
    echo "[lite_bench]   원인을 고친 뒤 재측정 없이 다시 렌더할 수 있다: render_report.py --lite-only --lite-raw-json $RAW" >&2
    exit 5
  fi
  if [ "$LITE_RC" = 0 ]; then
    if CERT_PATH="$(python3 "$REPO/.claude/skills/adversarial-benchmark/scripts/publish_benchmark_record.py" --lite-raw-json "$RAW")"; then
      echo "[lite_bench] lite 등급 인증서 발행 → $CERT_PATH (bench_mode: lite · verdict: not_applicable · hint 발행 자격)"
    else
      echo "[lite_bench] ⚠ lite 인증서 발행 실패(위 사유) — 재측정 없이: publish_benchmark_record.py --lite-raw-json $RAW" >&2
      exit 5
    fi
  else
    echo "[lite_bench] lite 불통과 → 인증서 미발행(lite ⊂ full · 인증서는 lite 통과에서만)"
  fi
else
  echo "[lite_bench] 리포트·인증서 미발행(기본 · 자동 핸드오프 경로 = 기록·보고만) — lite-only 셀은 --publish-report"
fi
echo "[lite_bench] DONE  RAW=$RAW  lite_verdict=$LITE_VERDICT (exit $LITE_RC)"
exit "$LITE_RC"
