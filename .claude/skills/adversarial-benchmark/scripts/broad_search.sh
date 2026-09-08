#!/usr/bin/env bash
# broad_search.sh — 광의의 탐색(Broad Search) 오퍼레이션 (plan_26090415 §4 · CP6)
#
# ★ Max 와 동형인 **별도 오퍼레이션**이다. 벤치마커 native 모드가 아니며, 측정 인프라만 공유한다.
#   자기 정체성 + 이중 게이트(`--confirm-risk` ∧ 에이전트의 챗 Y/N).
#
# ★ 이 스크립트는 **축을 고르지 않는다.** 축 선택·후보 생성·캠페인 안은 에이전트 판단이고
#   (결정론 스크립트가 축을 고르면 탐색 품질이 떨어진다 — 사용자 판단 2026-09-04),
#   여기가 소유하는 것은 게이트·안전·상태·산출물이라는 **결정론 부분**뿐이다.
#   그래서 `init` 은 셀 목록을 **인자로 받는다**. 만들지 않는다.
#
# ★ 순위를 만들지 않는다. 목적함수도 파레토 선언도 없다 — 평가축이 여럿이므로 상황에 맞는
#   레시피 선택은 사람 몫이고, 이 오퍼레이션은 레시피별 **진실된 결과**만 보인다.
#   그 금지는 산문이 아니라 `render_sweep_map.py` 의 결정론 단언이 집행한다.
#
# ★ 예산은 **선언에서만** 온다(§4.8 HITL 2단). 대화 층(깊이 승인)이 숫자를 낳고 스크립트 층은
#   인자로 받기만 한다. 미선언은 비0 종료이며, 정상 경로에서 이 거부는 발동하지 않는다 —
#   발동하면 대화 층을 건너뛴 비정상 호출이라는 뜻이다.
#
# 서브커맨드
#   init   상태 파일 생성(선언 예산·계획 셀·통제변인·루브릭 권한)
#   cell   **돌고 있는 serve** 한 칸을 측정·판정·기록한다(아래 경계 주의)
#   status 정지 조건 평가(sweep_stop.py)
#   map    지도 발행(render_sweep_map.py · 미완도 발행)
#
# ⚠ **경계 — 이 오퍼레이션은 serve 를 기동하지 않는다.**
#   셀 materialize(재서빙)는 `vllm-recipe-explorer` 가 소유한다(§4.1 역할표). `cell` 은 그 결과로
#   **이미 떠 있는** serve 를 측정한다. 기동을 여기에 넣지 않는 이유는 소유 경계이자 안전이다 —
#   단일노드에는 아직 **스크립트화된 정규 기동 경로가 없고**(teardown 만 `single_serve_down.sh` 로
#   존재한다), 그 자리를 이 스크립트가 `docker compose up` 으로 메우면 예산선언·워치독 무장이
#   빠진 무보호 기동이 오퍼레이션 안에 굳는다. 없는 경로는 **우회하지 않고 만든다**(D3).
#   그때까지 `cell` 은 serve 미가동을 exit 3 으로 알리고 멈춘다.
#
# --state 의 자리(2026-09-06 · plan_26090616 ②): 호출자가 임의로 고르지 않는다 — 활성 캠페인에서
#   파생한다.  STATE="$(python3 .claude/skills/terraforming_node/scripts/campaign_init.py \
#                        --derive sweep --sweep <sweep-id>)"
#   왜: 스윕 상태 파일이 캠페인 밖에 살면 캠페인이 끝나도 남아 다음 캠페인의 정지판정과 섞인다.
#   (기본값을 두지 않는 것은 유지한다 — 부재를 조용히 루트로 폴백시키지 않는 것이 이 설계의 핵심이다.)
# 사용: broad_search.sh init --sweep-id ID --state PATH --cells k1,k2 --control-variable TEXT
#                            --max-cells N --wall-clock-budget-s N --consecutive-failure-limit N
#                            --declared-by TEXT --basis TEXT --authority explore --now-utc T
#       broad_search.sh cell --state PATH --cell-key K --config NAME --axis-citation TEXT
#                            --next-intent TEXT [--ack-uncalibrated-thermal]
#                            --bench-budget-mib N --now-utc T --confirm-risk [--topology t]
#       broad_search.sh status --state PATH --now-utc T
#       broad_search.sh map    --state PATH --now-utc T --out-md PATH [--out-json PATH]
# 종료: 0=성공 · 2=인자/선언 오류 · 3=serve 미가동(materialize 는 explorer 소관) · 5=--confirm-risk 미명시
#       6=SoC 열 임계 미교정 미승인(--ack-uncalibrated-thermal)
set -euo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$SDIR" rev-parse --show-toplevel 2>/dev/null || pwd)"
CMD="${1:?서브커맨드 필요: init|cell|status|map}"; shift || true

STATE=""; NOW=""; SWEEP_ID=""; CELLS=""; CTRL=""; AUTHORITY=""; NODE=""
MAX_CELLS=""; WALL=""; FAILLIMIT=""; DECLARED_BY=""; BASIS=""
CELL_KEY=""; CONFIG=""; CITATION=""; BENCH_BUDGET=""; TOPO=""; CONFIRM=0
# 여정 한 줄(2026-09-07 · plan_26090715 §4.3 · 인터뷰 Q4). 새 절차를 만들지 않고 **이미 도는
# 자동쓰기**(이 셀 트랜잭션)에 인자 하나를 얹는다. 감수하지 말아야 할 유실은 여정 하나이며,
# 벤치 결과·3+1+1 산출물은 결손 기재로 복원된다.
NEXT_INTENT=""
# 열 임계 미교정 승인(2026-09-07 · plan_26090715 §5 ⑤-③ · 유예 결함 ③). SoC 열 파라미터 3종은
# 외부 보고에서 역산한 값이고 **교정되지 않았다**(UNCALIBRATED). 그 사실이 산출물에 표시는 됐지만
# **결정하는 소비자가 0** 이었다 — 실킬 3건이 그 임계로 났고 직전 인증서 1건이 오기록됐다.
# 이 플래그가 그 소비자다: 부하를 걸기 전에 "미교정 임계로 재는 것을 안다" 를 명시하게 한다.
ACK_UNCAL=0
# 측정 엔드포인트. harmony 계열(gpt-oss)은 **완결 엔드포인트**로 재야 한다 —
#   chat 에서는 `ignore_eos` 가 harmony 정지 토큰을 넘어 생성시키고, 그러면 서버의 harmony
#   파서가 `Unexpected token … while expecting start token …` 로 일부 요청을 깬다
#   (2026-09-04 실측: 16건 중 1건 errored → measurement_ok=false). 0.19.1 에서 chat+ignore_eos 가
#   길이 자체는 지켰다는 점이 2026-09-01 관측과 다른 부분인데, 길이를 지키는 대가가 파서 파손이라
#   결론은 같다: **완결 엔드포인트로 잰다.**
BACKEND=""
OUT_MD=""; OUT_JSON=""; REASSEMBLE=0; SERVE_FAILED_REASON=""; MAX_ERROR_RATE=""
# 부하 레벨 목록. 빈 값이면 sweep_bench 의 기본(1,2,4,8,16)을 그대로 쓴다 — 여기서 기본을
#   다시 적으면 같은 개념이 두 파일에 손으로 적히고 갈라진다(4종 안티패턴 · 매직넘버).
#   2026-09-07 신설: sweep_bench 에는 --levels 가 있었으나 이 호출자가 전달하지 않아
#   **열 예산 안에서 스윕을 짧게 도는 정식 경로가 없었다**(배선 부재 · GB10 120b multi 는
#   연속 포화부하 4분에 SoC 95C hard ceiling 에 닿아 워치독이 서빙을 죽인다).
LEVELS=""
# 2026-09-07 신설(같은 결함 계열 — --levels 와 동일): 프롬프트 형상(in/out/n)도 sweep_bench 에
#   있고 여기만 없었다. KV 압력 실험(DeepTailor 류 방법론)은 긴 프롬프트가 필요한데 기본
#   1024/256 은 풀 압력을 만들지 못한다. 빈 값이면 sweep_bench 기본을 그대로 쓴다(개념 이중기재 금지).
ILEN=""; OLEN=""; NPROMPTS=""
while [ $# -gt 0 ]; do case "$1" in
  --state) STATE="$2"; shift 2;;
  --now-utc) NOW="$2"; shift 2;;
  --sweep-id) SWEEP_ID="$2"; shift 2;;
  --cells) CELLS="$2"; shift 2;;
  # 배정에서 파생할 노드. `--cells` 를 함께 주면 손 목록이 이긴다(명시 > 파생) — 다만 그때는
  # 선언과 갈릴 수 있으므로 init 이 그 사실을 로그에 남긴다.
  --node) NODE="$2"; shift 2;;
  --control-variable) CTRL="$2"; shift 2;;
  --authority) AUTHORITY="$2"; shift 2;;
  --max-cells) MAX_CELLS="$2"; shift 2;;
  --wall-clock-budget-s) WALL="$2"; shift 2;;
  --consecutive-failure-limit) FAILLIMIT="$2"; shift 2;;
  --declared-by) DECLARED_BY="$2"; shift 2;;
  --basis) BASIS="$2"; shift 2;;
  --cell-key) CELL_KEY="$2"; shift 2;;
  --config) CONFIG="$2"; shift 2;;
  --axis-citation) CITATION="$2"; shift 2;;
  --next-intent) NEXT_INTENT="$2"; shift 2;;
  --ack-uncalibrated-thermal) ACK_UNCAL=1; shift;;
  --bench-budget-mib) BENCH_BUDGET="$2"; shift 2;;
  --max-error-rate) MAX_ERROR_RATE="$2"; shift 2;;
  --levels) LEVELS="$2"; shift 2;;
  --input-len) ILEN="$2"; shift 2;;
  --output-len) OLEN="$2"; shift 2;;
  --num-prompts) NPROMPTS="$2"; shift 2;;
  --topology) TOPO="$2"; shift 2;;
  --backend) BACKEND="$2"; shift 2;;
  --confirm-risk) CONFIRM=1; shift;;
  # 측정하지 않고 **기존 산출물에서 셀 기록만 다시 조립**한다. 라벨·파생키 계약이 바뀌었을 때
  #   (예: 2026-09-04 moe_backend 실측 승격) 재측정 없이 지도를 정합화하는 유일한 정식 경로다 —
  #   대안은 상태 파일 수기 편집(=증거 위조)이거나 재측정(=라벨 문제인데 비용 지불)뿐이다.
  #   `sweep_bench --reassemble-only` 와 같은 선례이며 같은 이유로 존재한다.
  --reassemble-only) REASSEMBLE=1; shift;;
  # 서빙에 도달하지 못한 셀을 기록한다(materialize 실패 등). §4.6 의 `serve_failed` 는 "서빙 자체
  #   미성립"을 담는 칸이고, 그런 셀은 트리플렛조차 없어 통상 경로(envfile 검사)를 탈 수 없다.
  #   기록하지 못하면 **지도에 구멍이 남고** 다음 캠페인이 같은 벽에 다시 부딪힌다 —
  #   실패는 숨길 것이 아니라 지도가 실어야 할 정보다.
  --serve-failed) SERVE_FAILED_REASON="$2"; shift 2;;
  --out-md) OUT_MD="$2"; shift 2;;
  --out-json) OUT_JSON="$2"; shift 2;;
  *) echo "[broad_search] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

[ -n "$STATE" ] || { echo "[broad_search] ERROR --state 는 필수다" >&2; exit 2; }
[ -n "$NOW" ]   || { echo "[broad_search] ERROR --now-utc 는 필수다(벽시계 금지 — 시각은 주입만)" >&2; exit 2; }
if [ -z "$TOPO" ]; then
  BR="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo)"
  case "$BR" in multi-node) TOPO=multi;; single-node) TOPO=single;; *) TOPO=single;; esac
fi

STOPJSON="${STATE%.json}.stop.json"

_stop(){   # 상태 → 정지 판정 파일. rc 0=계속 · 3=정지 를 그대로 돌려준다.
  # ★ 여기서 `set -e` 를 다시 켜지 않는다. 셸 옵션은 함수 지역이 아니라 **전역**이라,
  #   켜 두면 호출부가 `set +e` 로 감싸 놨어도 `return 3`(정지) 이 errexit 를 물어
  #   스크립트가 **게이트 메시지를 찍기 전에** 죽는다. 2026-09-05 실측: 연속실패 정지가
  #   성립했는데 stdout·stderr 둘 다 비고 rc=3 만 남았다 — 가드가 듣는 사람 없는 자리에서
  #   울린 것이다. errexit 복원은 호출부 책임이다(모든 호출부가 이미 `set -e` 로 닫는다).
  set +e
  python3 "$SDIR/sweep_stop.py" --state "$STATE" --now-utc "$NOW" > "$STOPJSON"
  local rc=$?
  [ "$rc" = "2" ] && { cat "$STOPJSON" >&2; echo "[broad_search] 정지 조건을 판정할 수 없다" >&2; exit 2; }
  return "$rc"
}

case "$CMD" in

init)
  # ── 셀 목록은 **선언에서 파생**한다(2026-09-08 · plan_26090813 §4.4). 손으로 적은 목록은
  #    campaign.yaml 의 배정과 갈라지고, 갈라지면 스윕 지도가 자기가 안 돈 셀을 주장하거나
  #    배정된 셀을 빠뜨린다. `--node` 를 주면 `assignments[<node>]` 가 그대로 온다.
  if [ -z "$CELLS" ] && [ -n "${NODE:-}" ]; then
    _CI="$REPO/.claude/skills/terraforming_node/scripts/campaign_init.py"
    if [ ! -f "$_CI" ]; then
      echo "[broad_search] ERROR --node 파생을 요청했는데 campaign_init 이 없다: $_CI" >&2; exit 2
    fi
    CELLS="$(python3 "$_CI" --assigned-cells "$NODE")" || {
      echo "[broad_search] ERROR assignments[$NODE] 에서 셀을 파생하지 못했다(위 사유 참조)" >&2; exit 2; }
    echo "[broad_search] cells ← assignments[$NODE] = $CELLS"
  elif [ -n "$CELLS" ] && [ -n "${NODE:-}" ]; then
    echo "[broad_search] (info) --cells 를 명시했으므로 assignments[$NODE] 파생을 쓰지 않는다 — 선언과 갈릴 수 있다"
  fi
  for pair in "--sweep-id:$SWEEP_ID" "--cells:$CELLS" "--control-variable:$CTRL" \
              "--authority:$AUTHORITY" "--max-cells:$MAX_CELLS" \
              "--wall-clock-budget-s:$WALL" "--consecutive-failure-limit:$FAILLIMIT" \
              "--declared-by:$DECLARED_BY" "--basis:$BASIS"; do
    [ -n "${pair#*:}" ] || { echo "[broad_search] ERROR ${pair%%:*} 는 필수다(선언 없이 스윕을 열지 않는다 · §4.8)" >&2; exit 2; }
  done
  [ -e "$STATE" ] && { echo "[broad_search] ERROR 상태 파일이 이미 있다: $STATE (덮어쓰지 않는다)" >&2; exit 2; }
  mkdir -p "$(dirname "$STATE")"
  SWEEP_ID="$SWEEP_ID" CELLS="$CELLS" CTRL="$CTRL" AUTHORITY="$AUTHORITY" \
  MAX_CELLS="$MAX_CELLS" WALL="$WALL" FAILLIMIT="$FAILLIMIT" \
  DECLARED_BY="$DECLARED_BY" BASIS="$BASIS" NOW="$NOW" TOPO="$TOPO" \
  python3 - "$STATE" <<'PY'
import json, os, sys
cells = [c.strip() for c in os.environ["CELLS"].split(",") if c.strip()]
if not cells:
    sys.stderr.write("[broad_search] ERROR --cells 가 비었다\n"); raise SystemExit(2)
def _pos(name):
    raw = os.environ[name]
    if not raw.isdigit() or int(raw) <= 0:
        sys.stderr.write("[broad_search] ERROR %s 는 양의 정수여야 한다(받은 값: %r)\n" % (name, raw))
        raise SystemExit(2)
    return int(raw)
state = {
    "schema_version": 1,
    "sweep_id": os.environ["SWEEP_ID"],
    "topology": os.environ["TOPO"],
    "control_variable": os.environ["CTRL"],
    "rubric_authority": os.environ["AUTHORITY"],
    "started_utc": os.environ["NOW"],
    "declared_budget": {
        "max_cells": _pos("MAX_CELLS"),
        "wall_clock_budget_s": _pos("WALL"),
        "consecutive_failure_limit": _pos("FAILLIMIT"),
        "declared_by": os.environ["DECLARED_BY"],
        "basis": os.environ["BASIS"],
    },
    "cells_planned": len(cells),
    "cells_remaining": cells,
    "cells": [],
}
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(state, f, ensure_ascii=False, indent=2)
print("[broad_search] init — sweep=%s cells=%d 예산=%s"
      % (state["sweep_id"], len(cells), json.dumps(state["declared_budget"], ensure_ascii=False)))
PY
  ;;

status)
  set +e; _stop; rc=$?; set -e
  cat "$STOPJSON"
  exit "$rc"
  ;;

map)
  [ -n "$OUT_MD" ] || { echo "[broad_search] ERROR map 에는 --out-md 가 필수다" >&2; exit 2; }
  set +e; _stop; set -e
  MAP_ARGS=(--state "$STATE" --stop-json "$STOPJSON" --out-md "$OUT_MD")
  [ -n "$OUT_JSON" ] && MAP_ARGS+=(--out-json "$OUT_JSON")
  python3 "$SDIR/render_sweep_map.py" "${MAP_ARGS[@]}"
  ;;

cell)
  # ── 무부하 파생변수(2026-09-07 · plan_26090715 §5 ⑤-④ · 유예 결함 ④) ───────────────────────
  #   `--serve-failed`(이미 끝난 일의 기록)와 `--reassemble-only`(재측정 없는 지도 정합화)는 둘 다
  #   **컨테이너도 벤치도 띄우지 않는다**. 그런데 종전에는 정지조건 게이트만 그 사실을 알았고
  #   위험 게이트는 몰라서 `--confirm-risk` 를 요구했다. 그 결과 캠페인 ⑦ 의 b1·b4·b5·b6 이
  #   **위험 플래그를 붙인 채 무위험 기록**을 남겼다 — 플래그가 "부하를 걸겠다" 는 뜻을 잃으면
  #   다음 사람은 그것을 형식으로 읽고 진짜 위험 구간에서도 반사적으로 붙인다(게이트 의미 희석).
  #   두 게이트가 **같은 질문**(이 호출이 로드를 하는가)을 보게 파생변수 하나로 묶는다.
  NO_LOAD=0
  { [ -n "$SERVE_FAILED_REASON" ] || [ "$REASSEMBLE" = 1 ]; } && NO_LOAD=1

  # 이중 게이트 (1) — 스크립트 플래그. (2) 는 에이전트가 챗에서 받는다(선-기록 후-위험).
  #   무부하 호출은 위험 게이트를 타지 않는다(부하도 재기동도 없다).
  if [ "$CONFIRM" != 1 ] && [ "$NO_LOAD" != 1 ]; then
    cat >&2 <<'MSG'
[broad_search] ⚠ 셀 실행 거부(--confirm-risk 미명시).
  이 오퍼레이션은 통합메모리 위에서 부하를 건다 — 호스트 하드다운 계보가 있는 축이다.
  실행하려면 --confirm-risk 를 붙여라(에이전트는 챗 경고톤 Y/N 승인 뒤에만 붙일 것).
MSG
    exit 5
  fi
  # ── 열 임계 미교정 게이트(유예 결함 ③). 무부하 호출은 열을 만들지 않으므로 대상이 아니다.
  if [ "$NO_LOAD" != 1 ]; then
    _UNCAL="$(REPO="$REPO" python3 - <<'PY' 2>/dev/null || true
import importlib.util, os, sys
_p = os.path.join(os.environ["REPO"], ".claude", "skills", "terraforming_node", "scripts",
                  "node_blackbox", "blackbox_thermal.py")
if os.path.isfile(_p):
    _s = importlib.util.spec_from_file_location("_bt", _p)
    _m = importlib.util.module_from_spec(_s); _s.loader.exec_module(_m)
    print(",".join(getattr(_m, "UNCALIBRATED", ()) or ()))
PY
)"
    if [ -n "$_UNCAL" ] && [ "$ACK_UNCAL" != 1 ]; then
      echo "[broad_search] ⚠ 셀 실행 거부 — SoC 열 임계가 **미교정**이다(UNCALIBRATED: $_UNCAL)." >&2
      echo "  이 값들은 외부 보고에서 역산한 것이고 이 하드웨어에서 교정된 적이 없다." >&2
      echo "  그런데도 워치독은 이 임계로 서빙을 죽인다(실킬 3건 · 인증서 1건 오기록)." >&2
      echo "  임계를 올리지 마라 — 그것은 정상 차단을 지우는 것이다. 대신 **알고 있음을 선언**하라:" >&2
      echo "    --ack-uncalibrated-thermal  (셀 기록에 그 사실이 남는다)" >&2
      echo "  교정 자체는 벤더 근거가 필요한 사람 과업이다(docs/request/ 위임 대상)." >&2
      exit 6
    fi
  fi

  if [ "$NO_LOAD" = 1 ] && [ "$CONFIRM" = 1 ]; then
    echo "[broad_search] ⓘ 이 호출은 로드를 하지 않는다(serve_failed 기록 또는 재조립) — " \
         "--confirm-risk 는 불필요하다. 위험 플래그가 형식이 되면 진짜 위험 구간에서 무뎌진다." >&2
  fi
  for pair in "--cell-key:$CELL_KEY" "--config:$CONFIG" "--bench-budget-mib:$BENCH_BUDGET" \
              "--axis-citation:$CITATION" "--next-intent:$NEXT_INTENT"; do
    [ -n "${pair#*:}" ] || { echo "[broad_search] ERROR ${pair%%:*} 는 필수다" >&2; exit 2; }
  done
  [ -f "$STATE" ] || { echo "[broad_search] ERROR 상태 파일 부재: $STATE (먼저 init)" >&2; exit 2; }

  # 진입 전 정지 조건. 이미 멈춰야 하는 스윕에 셀을 하나 더 밀어 넣지 않는다.
  #   단 `--serve-failed` 는 **이미 끝난 일의 기록**이다 — 컨테이너도 벤치도 띄우지 않고
  #   상태 파일에 사실 한 줄을 적을 뿐이다. 게이트의 목적은 *새 작업 착수*를 막는 것이지
  #   *이미 관측한 사실*을 지우는 것이 아니다. 여기서 막으면 "C7 은 불가"가 "C7 은 미상"이
  #   되어 지도에 구멍이 남는다(2026-09-05 실측 — 연속실패 정지가 마지막 셀 기록을 삼켰다).
  #   기록 뒤 정지 조건은 어차피 하단에서 다시 평가돼 그대로 성립한다.
  # ★ 2026-09-05: `--reassemble-only` 도 이 게이트에서 면제한다. 근거는 `--serve-failed` 와 같다 —
  #   측정을 돌리지 않고 셀 예산을 쓰지 않으며 **기존 산출물에서 기록만 다시 조립**한다.
  #   면제가 없으면 그 옵션의 선언된 목적("재측정 없이 지도를 정합화하는 유일한 정식 경로")이
  #   **스윕이 끝난 시점에 정확히 도달 불가**가 된다 — 라벨·파생키 계약이 바뀌었음을 알게 되는
  #   때가 바로 그때다. 남는 길은 상태 파일 수기 편집(증거 위조)뿐이라 게이트가 우회를 만든다.
  #   (캠페인 1 실측: cells_exhausted 뒤 MoE mismatch 필드를 실으려는데 이 게이트가 막았다.)
  if [ "$NO_LOAD" != 1 ]; then
    set +e; _stop; STOPRC=$?; set -e
    if [ "$STOPRC" = "3" ]; then
      echo "[broad_search] 정지 조건 성립 — 셀을 실행하지 않는다:" >&2
      python3 -c "import json;d=json.load(open('$STOPJSON'));print('  stopped_by:', d['stopped_by'])" >&2
      exit 0
    fi
  fi

  if [ -n "$SERVE_FAILED_REASON" ]; then
    # materialize 단계에서 죽은 셀 — 트리플렛이 없으므로 envfile·포트·측정 경로를 타지 않는다.
    STARTED="$NOW"; ENDED="$(date -u +%FT%TZ)"; SERVE_RC=3; MEASURE_RC=absent
    SWEEPDIR="$REPO/output/$TOPO/benchlog/sweep_${CONFIG}"
    echo "[broad_search] serve_failed 기록 — $SERVE_FAILED_REASON"
  else
  EF="$REPO/output/$TOPO/envs/.env.$CONFIG"
  [ -f "$EF" ] || { echo "[broad_search] ERROR envfile 없음: $EF — 셀 materialize 는 explorer 소관이다" >&2; exit 2; }
  PORT="$(sed -n 's/^SERVING_PORT=//p' "$EF" | head -1)"
  [ -n "$PORT" ] || { echo "[broad_search] ERROR SERVING_PORT 미정($EF)" >&2; exit 2; }

  STARTED="$NOW"
  SERVE_RC=0
  if [ "$REASSEMBLE" = "1" ]; then
    # 판정은 **기존 산출물**이 한다: 판정점 measured.json 의 measurement_ok 가 유일한 근거다.
    SWEEPDIR_PRE="$REPO/output/$TOPO/benchlog/sweep_${CONFIG}"
    if python3 -c "import json,sys;d=json.load(open('$SWEEPDIR_PRE/level_01/measured.json'));sys.exit(0 if d.get('measurement_ok') else 1)" 2>/dev/null; then
      MEASURE_RC=0
    else
      MEASURE_RC=1
    fi
    echo "[broad_search] 재조립 — 측정하지 않는다(기존 산출물에서 셀 기록만 다시 만든다)"
  elif [ "$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://localhost:$PORT/health" 2>/dev/null)" != "200" ]; then
    # 이 오퍼레이션은 serve 를 기동하지 않는다(위 §경계). 미가동은 **정직하게** serve_failed 로
    # 기록하고 멈춘다 — 여기서 몰래 띄우면 무보호 기동이 굳는다.
    SERVE_RC=3
    echo "[broad_search] serve 미가동(:$PORT/health≠200) — materialize 는 vllm-recipe-explorer 소관이다" >&2
  fi

  # 2026-09-05(G-B13): 기본 0 삭제 — "측정 안 함"과 "측정 성공"이 같은 값이 되면 재조립 경로가
  #   아무것도 재지 않고 `measured` 로 종결된다. 부재는 `absent` 로 **명시**해 넘긴다.
  MEASURE_RC="${MEASURE_RC:-absent}"
  SWEEPDIR="$REPO/output/$TOPO/benchlog/sweep_${CONFIG}"
  if [ "$SERVE_RC" = "0" ] && [ "$REASSEMBLE" != "1" ]; then
    set +e
    SB_ARGS=("$CONFIG" --topology "$TOPO" --tool guidellm --bench-budget-mib "$BENCH_BUDGET")
    [ -n "$BACKEND" ] && SB_ARGS+=(--backend "$BACKEND")
    [ -n "$MAX_ERROR_RATE" ] && SB_ARGS+=(--max-error-rate "$MAX_ERROR_RATE")
    [ -n "$LEVELS" ] && SB_ARGS+=(--levels "$LEVELS")
    [ -n "$ILEN" ] && SB_ARGS+=(--input-len "$ILEN")
    [ -n "$OLEN" ] && SB_ARGS+=(--output-len "$OLEN")
    [ -n "$NPROMPTS" ] && SB_ARGS+=(--num-prompts "$NPROMPTS")
    bash "$SDIR/sweep_bench.sh" "${SB_ARGS[@]}"
    MEASURE_RC=$?
    if [ "$MEASURE_RC" = "0" ]; then
      bash "$SDIR/judge_bench.sh" "$CONFIG" --topology "$TOPO" --authority "${AUTHORITY:-explore}" \
           --sweep-dir "$SWEEPDIR"
      MEASURE_RC=$?
    fi
    set -e
  fi
  ENDED="$(date -u +%FT%TZ)"
  fi

  EVARGS=()
  for evf in "$REPO"/docs/logs/*/events/*.jsonl; do
    [ -f "$evf" ] && EVARGS+=(--events "$evf")
  done
  CLS="$(python3 "$SDIR/classify_cell.py" --serve-rc "$SERVE_RC" --measure-rc "$MEASURE_RC" \
          --started-utc "$STARTED" --ended-utc "$ENDED" "${EVARGS[@]+"${EVARGS[@]}"}")"

  CELL_KEY="$CELL_KEY" CONFIG="$CONFIG" CITATION="$CITATION" SWEEPDIR="$SWEEPDIR" \
  CLS="$CLS" ENDED="$ENDED" STARTED="$STARTED" SERVE_FAILED_REASON="$SERVE_FAILED_REASON" \
  NEXT_INTENT="$NEXT_INTENT" THERMAL_UNCAL="${_UNCAL:-}" ACK_UNCAL="$ACK_UNCAL" \
  python3 - "$STATE" <<'PY'
import json, os, sys
state_path = sys.argv[1]
with open(state_path, encoding="utf-8") as f:
    state = json.load(f)
cls = json.loads(os.environ["CLS"])
sweepdir = os.environ["SWEEPDIR"]

def _load(name):
    try:
        with open(os.path.join(sweepdir, name), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None

index = _load("sweep_index.json") or {}
verdict = _load("verdict.json") or {}
# ★ 2026-09-06: 오류 분할(도구 경계 대 서버)이 판정점 measured.json 에는 있는데 **셀 기록으로
#   올라오지 않았다**. 그러면 지도에는 깨끗한 `measured` 셀만 보이고, 요청의 일부가 클라이언트
#   파서에 버려졌다는 사실이 사라진다. 실측(a0): 24건 중 3건이 harmony 토큰 경계에서 errored.
#   parse_guidellm 이 "가르되 삼키지 않는다"로 고쳐졌는데 **마지막 홉에서 다시 삼켜졌다** —
#   바로 위 moe_backend_mismatch 주석이 적은 것과 같은 계열의 결함이다(계산은 해 놓고 버린다).
judged = _load(os.path.join("level_01", "measured.json")) or {}
meta = index.get("meta") or {}
# 동시성 축은 **벡터 전부**를 싣는다(한 점으로 접으면 승자가 뒤집힌다는 실측이 있다).
vector = {str(l.get("level")): ((l.get("measured") or {}).get("decode_tps"))
          for l in (index.get("levels") or [])}
cell = {
    "cell_key": os.environ["CELL_KEY"],
    "config": os.environ["CONFIG"],
    "started_utc": os.environ["STARTED"],
    "ended_utc": os.environ["ENDED"],
    "cell_outcome": cls["cell_outcome"],
    "void_reason": cls.get("void_reason"),
    "void_reason_source": cls.get("void_reason_source"),
    "note": cls.get("note"),
    "kill_events": cls.get("kill_events"),
    # ★ sweep meta 의 키 이름을 그대로 쓴다. 종전에 `tp`·`vllm` 을 읽어 **지도의 두 칸이 항상
    #   null** 이었다(2026-09-04 첫 지도에서 발견). 좌표가 비면 그 셀은 재현 불가이고, 조용히
    #   비어 있으므로 로그로는 보이지 않는다 — 같은 계열 결함이 judge_bench 에도 있었다.
    "coordinates": {k: meta.get(k) for k in
                    ("quantization", "max_model_len", "kv_cache_dtype", "tensor_parallel_size",
                     "image_tag", "image_digest", "moe_backend", "attention_backend",
                     "gpu_memory_utilization", "max_num_seqs")},
    "concurrency_vector": vector or None,
    # 용량 축은 속도와 **합치지 않고 나란히** 둔다.
    "capacity": {"kv_cache_memory_bytes": meta.get("kv_cache_memory_bytes"),
                 "max_model_len": meta.get("max_model_len")},
    "measurement": {"bench_tool": meta.get("bench_tool"),
                    "bench_tool_version": meta.get("bench_tool_version"),
                    "bench_tool_version_source": meta.get("bench_tool_version_source"),
                    "vllm_version": meta.get("vllm_version"),
                    "gpu_model": meta.get("gpu_model"),
                    # 커널 축은 요청과 실효가 갈릴 수 있으므로 **출처·불일치·후보**까지 싣는다.
                    "attention_backend_source": meta.get("attention_backend_source"),
                    "attention_backend_declared": meta.get("attention_backend_declared"),
                    "attention_backend_mismatch": meta.get("attention_backend_mismatch"),
                    "attention_backend_candidates": meta.get("attention_backend_candidates"),
                    # ★ 2026-09-05: MoE 도 같은 세 필드를 싣는다. 바로 위 주석이 "출처·불일치까지
                    #   싣는다"고 적어 두었는데 **어텐션에만 적용돼 있었다** — sweep_bench 는
                    #   `moe_backend_mismatch` 를 정확히 계산해 놓고(YES(measured=marlin
                    #   declared=triton)) broad_search 가 그것을 버렸다. 캠페인 1 실측:
                    #   moe-backend 를 triton·flashinfer_trtllm·cutlass 로 선언한 셀 3개가 전부
                    #   marlin 으로 돌았는데 지도에는 그 사실이 없었다. 좌표는 실측값이라 거짓은
                    #   아니었지만, "축을 옮겼는데 안 옮겨졌다"는 **가장 중요한 결과**가 사라졌다.
                    "moe_backend_source": meta.get("moe_backend_source"),
                    "moe_backend_declared": meta.get("moe_backend_declared"),
                    "moe_backend_mismatch": meta.get("moe_backend_mismatch"),
                    # 오류 분할(판정점 기준). 전체 오류율과 서버 오류율을 **나란히** 둔다 —
                    # 판정은 서버 쪽으로 하되 버려진 요청 수를 읽는 사람에게서 감추지 않는다.
                    "error_rate": judged.get("error_rate"),
                    "server_error_rate": judged.get("server_error_rate"),
                    "tool_boundary_errors": judged.get("tool_boundary_errors"),
                    "server_errors": judged.get("server_errors"),
                    "error_split_source": judged.get("error_split_source"),
                    "completed_requests": judged.get("completed"),
                    "num_prompts": judged.get("num_prompts")},
    "verdict_narrative": (("%s · authority=%s · source=%s · floor=%s"
                           % (verdict.get("verdict"),
                              (verdict.get("rubric") or {}).get("authority"),
                              (verdict.get("rubric") or {}).get("source"),
                              (verdict.get("rubric") or {}).get("floor")))
                          if verdict else None),
    "axis_citation": os.environ["CITATION"],
    # 여정 한 줄 — "다음에 무엇을 할 참인가". 지도(선언)가 영토(실측)와 갈라진 지점을 남기는
    # 유일한 자리이며, 이 체인에서 **복원 불가능한 유일한 정보**다.
    "next_intent": os.environ.get("NEXT_INTENT") or None,
    # 열 임계 미교정 사실을 **셀마다** 남긴다(2026-09-07 · 유예 결함 ③). 표시만 하고 아무도
    # 읽지 않으면 그 표시는 없는 것과 같다 — 이 필드가 그 표시의 소비자이자 기록이다.
    "thermal_uncalibrated": ([x for x in (os.environ.get("THERMAL_UNCAL") or "").split(",") if x]
                             or None),
    "thermal_uncalibrated_ack": os.environ.get("ACK_UNCAL") == "1",
}
_sf = os.environ.get("SERVE_FAILED_REASON") or ""
if _sf:
    # 사유는 **인용**이다 — 로그의 실제 문장을 옮긴다. 요약·추측 ✗.
    cell["serve_failed_reason"] = _sf
    cell["note"] = "%s / %s" % (cell.get("note") or "", _sf)
# 같은 셀 키가 이미 있으면 **교체**한다(재조립). 덧붙이면 지도에 같은 좌표가 두 번 나온다.
state["cells"] = [c for c in (state.get("cells") or []) if c.get("cell_key") != cell["cell_key"]]
state["cells"].append(cell)
state["cells_remaining"] = [c for c in (state.get("cells_remaining") or [])
                            if c != cell["cell_key"]]
with open(state_path, "w", encoding="utf-8") as f:
    json.dump(state, f, ensure_ascii=False, indent=2)
print("[broad_search] cell %s → %s" % (cell["cell_key"], cell["cell_outcome"]))
PY
  # ── 채우는 손(2026-09-07 · plan_26090715 §4.1). 이 트랜잭션이 sweep 레코드를 쓴 **바로 그
  #    자리**에서 cell.status 와 여정 줄도 쓴다. 두 자리를 다른 시점에 쓰면 갈라지고, 갈라진 것이
  #    캠페인 ⑦ 의 b1~b6 이다(sweep=serve_failed 인데 cell.status=pending · P1 이 그것을 잡는다).
  #    포맷 소유는 campaign_init 하나이고 여기는 호출부다. ACTIVE=_bootstrap 이면 no-op 이다.
  _CI="$REPO/.claude/skills/terraforming_node/scripts/campaign_init.py"
  if [ -f "$_CI" ]; then
    _OUTCOME="$(python3 -c "import json,sys;d=json.load(open(sys.argv[1]));c=[x for x in d.get('cells') or [] if x.get('cell_key')==sys.argv[2]];print((c[-1].get('cell_outcome') if c else '') or '')" "$STATE" "$CELL_KEY")"
    if [ -n "$_OUTCOME" ]; then
      # ★ 2026-09-08(plan_26090813 D19): 측정값을 **호출부가 나르지 않는다**. sweep 기록을 방금
      #   쓴 그 파일을 writer 가 직접 읽는다 — 종전에는 이 호출부가 --measurement-decode-tps 를
      #   아예 안 넘겨서 sweep 엔 19.12 t/s 가 있는데 cell.status 의 측정이 null 이었다.
      #   노드도 넘기지 않는다: 배정(assignments)에서 파생된다.
      _WARGS=(--cell-set "$CELL_KEY" --outcome "$_OUTCOME" --axis-citation "$CITATION"
              --next-intent "$NEXT_INTENT" --utc "$ENDED" --sweep-state "$STATE")
      [ -n "$SERVE_FAILED_REASON" ] && _WARGS+=(--void-reason "$SERVE_FAILED_REASON"
                                                --void-reason-source "broad_search cell(엔진 로그 인용)")
      # writer 실패는 삼키지 않는다 — 상태가 안 적혔다는 사실 자체가 다음 재개의 함정이다.
      python3 "$_CI" "${_WARGS[@]}" \
        || echo "[broad_search] ⚠ campaigns writer 실패 — cell.status/여정이 기록되지 않았다(위 사유 참조)" >&2
      # bench phase 진행표 + 관측면 갱신(2026-09-08 · plan_26090813 §4.2). 노드는 셀 상태에서
      # 되읽는다 — writer 가 배정에서 파생해 방금 적은 값이고, 여기서 두 번째 파생을 만들지 않는다.
      _CS="$(python3 "$_CI" --derive cell-status --cell "$CELL_KEY" 2>/dev/null || true)"
      _NODE=""
      if [ -n "$_CS" ] && [ -f "$_CS" ]; then
        _NODE="$(python3 -c "import json,sys;print(json.load(open(sys.argv[1])).get('node_id') or '')" "$_CS" 2>/dev/null || true)"
      fi
      if [ -n "$_NODE" ]; then
        # `set -e` 아래에서 `[ … ] && x` 는 마지막 문장일 때 스크립트를 죽인다 — if 로 적는다.
        _BOK=(); _BSTATE="failed"
        if [ "$_OUTCOME" = "measured" ]; then _BOK=(--proof-ok); _BSTATE="done"; fi
        python3 "$_CI" --phase-set bench --node "$_NODE" --cell "$CELL_KEY" \
          --state "$_BSTATE" \
          --proof-predicate "sweep 레코드 실재 + cell_outcome=$_OUTCOME" "${_BOK[@]}" \
          --proof-source "broad_search cell: $STATE#cells[cell_key=$CELL_KEY]" \
          --started-utc "$STARTED" --ended-utc "$ENDED" --authored-by "$_NODE" \
          || echo "[broad_search] ⚠ bench 진행표 기록 실패" >&2
        python3 "$_CI" --write-brief --node "$_NODE" --utc "$ENDED" \
          || echo "[broad_search] ⚠ campaign_brief 갱신 실패 — 감독자가 읽을 관측면이 낡았다" >&2
      fi
    fi
  else
    # 부재는 침묵이 아니라 배선 결함이다(2026-09-08 · F5). 서브 오버레이에 writer 가 없던 동안
    # 이 자리가 조용히 통과해서, 서브의 셀 상태를 캠페인 종료 뒤 메인이 대신 적었다.
    echo "[broad_search] ⚠ campaigns writer 부재($_CI) — cell.status/여정을 아무도 적지 않았다." >&2
    echo "[broad_search]   서브라면 오버레이 배달이 캠페인 도구를 빠뜨린 것이다(침묵 누락 ✗)." >&2
  fi
  set +e; _stop; rc=$?; set -e
  python3 -c "import json;d=json.load(open('$STOPJSON'));print('[broad_search] stop=%s by=%s 남은셀=%d'%(d['stop'],d['stopped_by'],len(d['cells_remaining'])))"
  exit 0
  ;;

*) echo "[broad_search] 알 수 없는 서브커맨드: $CMD (init|cell|status|map)" >&2; exit 2;;
esac
