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
# ★ 2026-09-07(plan_26090715 §5 ⑤-① · 유예 결함 ①): 기본값을 **없앤다**. 종전 기본값
#   `openai-chat` 은 호출자가 아무 말도 안 하면 이기는 값이었고, 그래서 캠페인 ⑦ 은 선언이
#   "완결 엔드포인트" 인데 실제 측정은 chat 이었다(/v1/chat/completions 158 vs /v1/completions 14).
#   측정 조건은 요청 포맷이 TPOT 을 바꾸는 **1급 축**이다 — 기본값이 이기면 그 축이 침묵한다.
BACKEND=""
# ★ 2026-09-04 신설(CP4 · plan_26090415 §3.1) — 측정 도구 선택.
#   기본은 `vllm`(현행 경로 · 후방호환). lite 는 이 기본에 남고 full 만 guidellm 으로 간다
#   (`full = lite ∪ GuideLLM`). 두 경로를 **이질적으로 유지**하는 것이 설계다 — 통합하면 같은
#   버그가 양쪽에 균일하게 먹어 "일치해 보이면서 둘 다 틀리는" 상태가 된다(실결함 2건이 이질성
#   덕에 잡혔다: 2026-09-01·09-03).
TOOL="vllm"
TOOL_VERSION=""      # 미선언이면 기록의 default_version(= 마지막 스테이징분)
BENCH_BUDGET_MIB=""
TOPO=""; CONC=1; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; OUTDIR=""
while [ $# -gt 0 ]; do case "$1" in
  --tool) TOOL="$2"; shift 2;;
  --tool-version) TOOL_VERSION="$2"; shift 2;;
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

# 헌법 §테라포밍-완수 Flag 게이트 — 결정론 백스톱(**fail-closed**).
# 2026-09-05(③ 3-9 · G-E1): **위임 키 면제 경로 삭제**. 종전에는 메인이 발급한 실행 허가
#   (.claude/a2a_delegation.json)나 EASY_VLLM_A2A_DELEGATED=1 이 이 게이트를 면제했고, 그것이
#   "서브는 허가 없이 아무것도 못 한다"는 R3 구조였다. 서브는 이제 자기 manifest(메인 발급 Flag)와
#   **서명된 Agent Card** 를 가지므로 정규 경로로 통과한다 — 허가가 아니라 **정체성**을 본다.
#   · 프로비저닝된 노드(카드 있음): 카드 서명 검증 + 자기 manifest Flag (검증기는 배달된 런타임).
#   · 메인(카드 없음): 종전대로 manifest_contract --require-flag.
#   · 둘 다 아니면 fail-closed(exit 4).
CARD="$REPO/Agent_Card.json"
CARD_VERIFIER="$REPO/.claude/runtime/a2a/agent_card_contract.py"
[ -f "$CARD_VERIFIER" ] || CARD_VERIFIER="$REPO/.claude/skills/terraforming_node/scripts/agent_card_contract.py"
MC="$REPO/.claude/skills/terraforming_node/scripts/manifest_contract.py"
# 역할은 manifest 가 말한다(hostname·브랜치 추론 ✗). `self_role: sub` 면 카드 **부재도 거부**다 —
# 카드를 지우면 검사를 건너뛰는 형태가 되면 부재가 곧 면제가 된다(옛 결함의 거울상).
# ★ 2026-09-06 교정(plan_26090616 ⑥ · smoke_clone A8 이 RED 였던 자리): 종전 한 줄은
#   `sed ... 2>/dev/null | head -1 | ...` 였다. stderr 는 지웠지만 **rc 는 남는다** — 파일이 없으면
#   sed 가 2 를 내고 `set -euo pipefail` 이 스크립트를 그 자리에서 죽였다(rc=2). 그래서 fresh-clone
#   에서는 바로 아래 Flag 게이트(rc=4)가 **한 번도 도달되지 않았다** — 가드가 그 가드를 위해 만든
#   상황에서만 죽어 있었다. 부재는 정상 경로이므로 명시적으로 빈 역할로 흘려보낸다(침묵 폴백이
#   아니라 부재의 정직한 표현이며, 미테라포밍 판정의 권위는 아래 Flag 게이트가 갖는다).
SELF_ROLE=""
_ROLE_MANIFEST="$REPO/output/$TOPO/manifest.yaml"
if [ -f "$_ROLE_MANIFEST" ]; then
  SELF_ROLE="$(sed -n 's/^self_role:[[:space:]]*//p' "$_ROLE_MANIFEST" \
              | head -1 | tr -cd 'a-z' | head -c 16)"
fi
if [ "$SELF_ROLE" = "sub" ] && [ ! -f "$CARD" ]; then
  echo "[run_bench] manifest 가 self_role: sub 인데 Agent_Card.json 이 없다 — 정체성 증명 부재는 면제가 아니다(fail-closed)." >&2
  exit 4
fi
if [ -f "$CARD" ]; then
  if [ ! -f "$CARD_VERIFIER" ]; then
    echo "[run_bench] Agent_Card 는 있는데 검증기가 없다 — 서명을 확인할 수 없어 진행하지 않는다(fail-closed)." >&2
    echo "[run_bench]   메인의 재배달이 필요하다(.claude/runtime/a2a/agent_card_contract.py)." >&2
    exit 4
  fi
  if ! GATE_OUT="$(python3 "$CARD_VERIFIER" prove-identity --repo-root "$REPO" --require-flag 2>&1)"; then
    echo "[run_bench] 정체성 증명 실패 — info-only(fail-closed):" >&2
    printf '%s\n' "$GATE_OUT" | sed "s|^|[run_bench]   |" >&2
    exit 4
  fi
elif [ -f "$MC" ]; then
  if ! python3 "$MC" --topology "$TOPO" --repo "$REPO" --require-flag >/dev/null 2>&1; then
    echo "[run_bench] 테라포밍-완수 Flag 미발급 — info-only. terraforming_node 로 HW스캔·검증 먼저." >&2
    exit 4
  fi
else
  echo "[run_bench] 정체성 증명·테라포밍 Flag 모두 부재 — info-only(fail-closed)." >&2
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
  "") echo "[run_bench] ERROR --backend 는 필수다(기본값 없음 · 2026-09-07)." >&2
      echo "  요청 포맷은 TPOT 을 바꾸는 1급 측정 축이다 — 기본값이 이기면 그 축이 침묵한다." >&2
      echo "  harmony 계열(gpt-oss)은 **완결 엔드포인트** --backend openai 로 재라." >&2
      echo "  chat 을 의도했다면 --backend openai-chat 을 **명시**하라." >&2
      exit 2 ;;
  *) echo "[run_bench] 알 수 없는 --backend: $BACKEND (openai-chat|openai)" >&2; exit 2 ;;
esac

# ── 측정 엔드포인트의 **출처 표시**(헌법 §결정론 규율). 값 옆에 어떻게 정해졌는지를 둔다.
BACKEND_SOURCE="declared(--backend)"

# ── harmony 계열 × chat 엔드포인트 = 측정 무효 (2026-09-05 · 주석을 집행으로 승격) ──
#   위 BACKEND 주석(2026-09-01)이 "harmony 는 chat 에서 ignore_eos 가 무력하다"를 이미 적어
#   두었는데 **집행하는 코드가 없었다**. 그 결과 오늘 캠페인 1 A0 가 기본값 openai-chat 으로
#   측정돼 서버 harmony 파서가 16건 중 4건을 깼고(`HarmonyError: Unexpected token 200002 while
#   expecting start token 200006`) error_rate 0.25 로 measurement_void 가 됐다. 주석은 사람이
#   읽어야 작동하고, 사람은 기본값을 그대로 쓴다.
#   신호는 모델 이름이 아니라 **트리플렛 러너가 선언한 reasoning parser** 다 — 이름 매칭은
#   새 harmony 모델을 놓치고, 러너 선언은 그 모델을 실제로 어떻게 서빙 중인지 말한다.
#   부재는 통과다(러너가 없거나 harmony 가 아니면 이 가드는 무동작).
#   ★ 2026-09-07: 신호를 **둘**로 늘린다. 러너 선언만 보면 러너가 그 플래그를 방출하지 않는
#   구성(캠페인 ⑦ 전 셀이 그랬다)에서 가드가 조용히 무동작한다 — "부재는 통과" 가 침묵 폴백이
#   되는 자리다. 두 번째 신호는 **엔진 로그 실측**이다: 실제로 뜬 서버가 harmony 파서를 물고
#   있는지는 그 로그가 말한다(선언이 아니라 관측).
_RUNNER="$REPO/output/$TOPO/configs/$CFGFILE.sh"
_HARMONY=0; _HARMONY_SRC=""
if [ -f "$_RUNNER" ] && grep -qE -- '--reasoning-parser[= ]+openai_gptoss' "$_RUNNER"; then
  _HARMONY=1; _HARMONY_SRC="declared(runner --reasoning-parser openai_gptoss)"
elif docker logs "$CTR" 2>&1 | tail -2000 \
     | grep -qEi 'reasoning[_-]parser.*(openai_gptoss|gpt.oss)|harmony'; then
  _HARMONY=1; _HARMONY_SRC="measured(engine log)"
fi
[ "$_HARMONY" = 1 ] && BACKEND_SOURCE="$BACKEND_SOURCE · harmony=$_HARMONY_SRC"
if [ "$BACKEND" = "openai-chat" ] && [ "$_HARMONY" = 1 ]; then
  echo "[run_bench] harmony 신호 출처: $_HARMONY_SRC" >&2
  echo "[run_bench] 거부: harmony 계열을 chat 엔드포인트로 재려 한다." >&2
  echo "  chat 에서는 --ignore-eos 가 무력하고(harmony stop 토큰이 EOS 와 별개), 서버 harmony 파서가" >&2
  echo "  스트림 중 깨져 요청이 errored 로 빠진다 → TPOT 왜곡 또는 measurement_void." >&2
  echo "  → --backend openai (완결 엔드포인트 /v1/completions) 로 재라." >&2
  exit 2
fi

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
  # 버전은 **선언**이면 그것을, 아니면 기록의 default_version(= 마지막 스테이징분)을 쓴다.
  # 최신 릴리즈 해소는 스테이징 평면(resolve_guidellm.py)의 일이다 — 벤치 도중 상류를 조회하지 않는다.
  local VERARG=()
  # `[ ... ] && x=(...)` 로 쓰면 조건이 거짓일 때 AND-리스트가 1 을 돌려주고 `set -e` 가 스크립트를
  # 죽인다 — 버전 미선언(정상 경로)에서 벤치가 통째로 사라지는 형태다. if 로 쓴다.
  if [ -n "${TOOL_VERSION:-}" ]; then VERARG=(--tool-version "$TOOL_VERSION"); fi
  PINJSON="$(python3 "$SDIR/resolve_bench_tool.py" --tool guidellm "${VERARG[@]}" --json 2>/dev/null)"; PRC=$?
  if [ "$PRC" != "0" ]; then
    python3 "$SDIR/resolve_bench_tool.py" --tool guidellm "${VERARG[@]}" >/dev/null  # 사유는 stderr 로
    echo "[run_bench] 측정 도구 해소 실패(rc=$PRC) — 실행하지 않는다" >&2
    return "$PRC"
  fi
  IMAGE="$(printf '%s' "$PINJSON" | python3 -c "import json,sys;print(json.load(sys.stdin)['image_ref'])")"
  # ★ 런별 기록(2026-09-05 · 축 A): 무엇으로 쟀는지를 산출물에 남긴다. digest 게이트를 걷어낸
  #   대신 **실제로 돈 이미지의 digest** 가 vault 에 남아 리포트·인증서가 그것을 인용한다.
  BENCH_TOOL_JSON="$OUTDIR/bench_tool_${CONFIG}.json"
  printf '%s' "$PINJSON" | GL_ENDPOINT="$ENDPOINT" GL_BACKEND_SOURCE="$BACKEND_SOURCE" \
    GL_OUT="$BENCH_TOOL_JSON" python3 -c '
import json, os, sys
doc = json.load(sys.stdin)
doc["endpoint"] = os.environ["GL_ENDPOINT"]        # 측정 조건 지문(요청 포맷이 TPOT 을 바꾼다)
doc["backend_source"] = os.environ.get("GL_BACKEND_SOURCE") or "unknown"  # 그 포맷이 어떻게 정해졌나
with open(os.environ["GL_OUT"], "w", encoding="utf-8") as f:
    json.dump(doc, f, ensure_ascii=False, indent=2); f.write("\n")
' 

  # 토크나이저는 **호스트 경로**가 필요하다. serve 컨테이너 안의 /app/models/... 를 manifest 의
  # nas_model_path 로 되돌린다. 되돌리지 못하면 추측하지 않고 멈춘다.
  local NAS HOSTTOK
  NAS="$(sed -n 's/^[[:space:]]*nas_model_path:[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
          "$REPO/output/$TOPO/manifest.yaml" 2>/dev/null | head -1 | sed 's/[[:space:]]*$//')"
  # /app/quant_models/* 도 되돌린다(2026-09-10 · camp-26090918): NVFP4/FP4 계열은 별도 NAS
  #   루트(manifest.quant_model_path)라 /app/models 매핑만 있으면 호스트 경로가 어긋나
  #   "토크나이저 부재" 로 죽는다 — 침묵 누락 배선. 미선언 manifest 에서 quant 경로가 오면
  #   추측하지 않고 멈춘다(nas 폐백 금지 — 경로가 틀리면 벤치가 아니라 배선이 틀린 것).
  local QUANT
  QUANT="$(sed -n 's/^[[:space:]]*quant_model_path:[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p'           "$REPO/output/$TOPO/manifest.yaml" 2>/dev/null | head -1 | sed 's/[[:space:]]*$//')"
  case "$MODEL_PATH" in
    /app/models/*) HOSTTOK="${NAS%/}/${MODEL_PATH#/app/models/}" ;;
    /app/quant_models/*)
      [ -n "$QUANT" ] || { echo "[run_bench] ERROR manifest 의 quant_model_path 부재 — /app/quant_models 매핑 불가" >&2; return 2; }
      HOSTTOK="${QUANT%/}/${MODEL_PATH#/app/quant_models/}" ;;
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

  # ── 벤치 진입 예산 게이트(2026-09-11 신설 · plan_26091108 R3) ─────────────────────────
  #   ★ 종전에는 **끊겨 있었다**: 이 컨테이너는 `--memory ${BENCH_BUDGET_MIB}m` 하드 제한을
  #     갖는데, 서빙 예산 선언은 `weights+kv+overhead` 만 파생하고 벤치 몫을 잇는 코드가 0 건이었다.
  #     그 결과 camp-26090918 의 `fp8-bf-262k-mmp` 은 **서빙에 성공하고 벤치에서 죽었다**
  #     (바닥 15,280 − 8,192 = 7,088 < 절대밴드 10,240).
  #   ★ 위상: serve overhead 에 **더하지 않는다.** 이 컨테이너는 로드 시점에 상주하지 않으므로
  #     로드 게이트에 상주분으로 넣으면 그만큼 과보수적으로 틀려 뜰 수 있는 셀을 죽인다.
  #     묻는 자리는 **여기**(벤치 진입)이고, 묻는 것은 "지금 선언된 바닥에서 이 몫을 빼도
  #     절대밴드가 남는가" 다.
  #   ★ 노드: GuideLLM 은 `--network host` 로 `localhost:$PORT` 에 붙으므로 **API 서버가 뜬
  #     노드에만** 존재한다(multi 에서는 메인). 그래서 이 노드의 선언만 본다 — 양 노드에
  #     같은 몫을 반영하면 서브 예산이 없는 비용을 계상한다.
  local _BB_DIR="$REPO/.claude/skills/terraforming_node/scripts/node_blackbox"
  local _SESSION_PY="$_BB_DIR/blackbox_session.py"
  local _PREFLIGHT="$REPO/.claude/skills/upstream-version-watch/scripts/budget_preflight.py"
  if [ -f "$_BB_DIR/node_identity.sh" ] && [ -f "$_SESSION_PY" ] && [ -f "$_PREFLIGHT" ]; then
    # shellcheck source=/dev/null
    . "$_BB_DIR/node_identity.sh"
    local _NID _NDIR _BSTAT _BRC _FLOOR
    if _NID="$(ni_resolve_node_id "$REPO" "" 2>/dev/null)" && [ -n "$_NID" ]; then
      _NDIR="$REPO/docs/logs/$_NID"
      _BSTAT="$(python3 "$_SESSION_PY" --node-dir "$_NDIR" budget-status --now "$(date -u +%FT%TZ)" 2>/dev/null)"; _BRC=$?
      if [ "$_BRC" = "0" ]; then
        _FLOOR="$(printf '%s' "$_BSTAT" | python3 -c "import json,sys;print(json.load(sys.stdin)['floor_mib'])" 2>/dev/null)"
        if [ -n "$_FLOOR" ]; then
          if ! python3 "$_PREFLIGHT" --floor-mib "$_FLOOR" --bench-budget-mib "$BENCH_BUDGET_MIB"; then
            echo "[run_bench] STOP(벤치 예산 게이트): 이 몫으로 벤치를 띄우면 서빙이 사정거리에 든다." >&2
            echo "[run_bench]   벤치를 **시작하지 않았다**. --bench-budget-mib 를 위 상한 이하로 낮추거나" >&2
            echo "[run_bench]   KV 절대클램프를 낮춰 바닥을 올려라(그래야 둘 다 산다)." >&2
            return 5
          fi
          # 선언이 벤치 도중 만료되면 워치독은 옛 규칙(무제한 arm)으로 돌아간다. 시계만 민다
          #   — 바닥은 건드리지 않는다(원 산출 provenance 보존).
          python3 "$_SESSION_PY" --node-dir "$_NDIR" renew-budget --now "$(date -u +%FT%TZ)" \
            >/dev/null 2>&1 || echo "[run_bench] ⚠ 예산 TTL 갱신 실패 — 벤치 도중 만료 위험" >&2
        fi
      else
        # 부재는 침묵이 아니다 — 선언 없이 뜬 서빙 위에서 재고 있다는 사실을 남긴다.
        echo "[run_bench] ⚠ 이 노드($_NID)에 서빙 예산 선언이 없다 — 벤치 진입 게이트를 적용할 수 없다." >&2
        echo "[run_bench]   서빙이 무보호로 떴다는 뜻이고, 벤치 컨테이너가 그 위에 얹힌다." >&2
      fi
    fi
  fi

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

# ── 벤치 종료 시 서버 생존 관측 (2026-09-07 · plan_26090715 §5 ⑤-② · 유예 결함 ②) ──────────
#   `RemoteProtocolError` 는 두 원인이 같은 모양으로 나온다: ⓐ 도구가 스트림을 먼저 끊었다
#   (도구 경계 · 면제 대상) ⓑ 엔진이 죽어 스트림이 잘렸다(서버 오류 · 면제 불가).
#   둘을 가르는 절단선은 **벤치가 끝난 시점에 서버가 살아 있었는가** 인데 그 관측이 어디에도
#   기록되지 않았다 — 그래서 엔진 사망 중 잘린 SSE 가 tool_boundary 로 면제되는 역방향
#   fail-open 이 열려 있었다. 관측은 여기서만 할 수 있다(벤치 직후 · 아직 teardown 전).
POST_HEALTH="$OUTDIR/post_health_${CONFIG}.json"
_PH_CODE="$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://localhost:$PORT/health" 2>/dev/null || echo 000)"
_PH_RUNNING="$(docker inspect -f '{{.State.Running}}' "$CTR" 2>/dev/null || echo unknown)"
_PH_OOM="$(docker inspect -f '{{.State.OOMKilled}}' "$CTR" 2>/dev/null || echo unknown)"
_PH_EXIT="$(docker inspect -f '{{.State.ExitCode}}' "$CTR" 2>/dev/null || echo unknown)"
_PH_ALIVE=false
[ "$_PH_CODE" = "200" ] && [ "$_PH_RUNNING" = "true" ] && _PH_ALIVE=true
cat > "$POST_HEALTH" <<JSON
{
  "schema_version": 1,
  "provenance": "measured",
  "config": "$CONFIG",
  "checked_after": "bench",
  "server_alive_at_bench_end": $_PH_ALIVE,
  "health_http_code": "$_PH_CODE",
  "container_running": "$_PH_RUNNING",
  "container_oom_killed": "$_PH_OOM",
  "container_exit_code": "$_PH_EXIT",
  "_note": "이 값이 false 면 errored 를 tool_boundary 로 면제할 수 없다 — 엔진이 죽어 잘린 스트림과 도구가 끊은 스트림은 같은 예외로 나온다."
}
JSON
echo "[run_bench] post-bench health: alive=$_PH_ALIVE (http=$_PH_CODE running=$_PH_RUNNING oom=$_PH_OOM exit=$_PH_EXIT)"

echo "[run_bench] DONE"
echo "BENCH_TOOL=$TOOL"
echo "BENCH_ENDPOINT=$ENDPOINT"
echo "BENCH_BACKEND_SOURCE=$BACKEND_SOURCE"
echo "BENCH_JSON=$BJSON"
echo "ENGINE_LOG=$ELOG"
echo "POST_HEALTH=$POST_HEALTH"
