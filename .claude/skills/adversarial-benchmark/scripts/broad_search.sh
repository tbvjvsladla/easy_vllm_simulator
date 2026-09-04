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
# 사용: broad_search.sh init --sweep-id ID --state PATH --cells k1,k2 --control-variable TEXT
#                            --max-cells N --wall-clock-budget-s N --consecutive-failure-limit N
#                            --declared-by TEXT --basis TEXT --authority explore --now-utc T
#       broad_search.sh cell --state PATH --cell-key K --config NAME --axis-citation TEXT
#                            --bench-budget-mib N --now-utc T --confirm-risk [--topology t]
#       broad_search.sh status --state PATH --now-utc T
#       broad_search.sh map    --state PATH --now-utc T --out-md PATH [--out-json PATH]
# 종료: 0=성공 · 2=인자/선언 오류 · 3=serve 미가동(materialize 는 explorer 소관) · 5=--confirm-risk 미명시
set -euo pipefail

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$SDIR" rev-parse --show-toplevel 2>/dev/null || pwd)"
CMD="${1:?서브커맨드 필요: init|cell|status|map}"; shift || true

STATE=""; NOW=""; SWEEP_ID=""; CELLS=""; CTRL=""; AUTHORITY=""
MAX_CELLS=""; WALL=""; FAILLIMIT=""; DECLARED_BY=""; BASIS=""
CELL_KEY=""; CONFIG=""; CITATION=""; BENCH_BUDGET=""; TOPO=""; CONFIRM=0
# 측정 엔드포인트. harmony 계열(gpt-oss)은 **완결 엔드포인트**로 재야 한다 —
#   chat 에서는 `ignore_eos` 가 harmony 정지 토큰을 넘어 생성시키고, 그러면 서버의 harmony
#   파서가 `Unexpected token … while expecting start token …` 로 일부 요청을 깬다
#   (2026-09-04 실측: 16건 중 1건 errored → measurement_ok=false). 0.19.1 에서 chat+ignore_eos 가
#   길이 자체는 지켰다는 점이 2026-09-01 관측과 다른 부분인데, 길이를 지키는 대가가 파서 파손이라
#   결론은 같다: **완결 엔드포인트로 잰다.**
BACKEND=""
OUT_MD=""; OUT_JSON=""; REASSEMBLE=0; SERVE_FAILED_REASON=""; MAX_ERROR_RATE=""
while [ $# -gt 0 ]; do case "$1" in
  --state) STATE="$2"; shift 2;;
  --now-utc) NOW="$2"; shift 2;;
  --sweep-id) SWEEP_ID="$2"; shift 2;;
  --cells) CELLS="$2"; shift 2;;
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
  --bench-budget-mib) BENCH_BUDGET="$2"; shift 2;;
  --max-error-rate) MAX_ERROR_RATE="$2"; shift 2;;
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
  set +e
  python3 "$SDIR/sweep_stop.py" --state "$STATE" --now-utc "$NOW" > "$STOPJSON"
  local rc=$?
  set -e
  [ "$rc" = "2" ] && { cat "$STOPJSON" >&2; echo "[broad_search] 정지 조건을 판정할 수 없다" >&2; exit 2; }
  return "$rc"
}

case "$CMD" in

init)
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
  # 이중 게이트 (1) — 스크립트 플래그. (2) 는 에이전트가 챗에서 받는다(선-기록 후-위험).
  #   재조립은 **측정하지 않으므로** 위험 게이트를 타지 않는다(부하도 재기동도 없다).
  if [ "$CONFIRM" != 1 ] && [ "$REASSEMBLE" != 1 ]; then
    cat >&2 <<'MSG'
[broad_search] ⚠ 셀 실행 거부(--confirm-risk 미명시).
  이 오퍼레이션은 통합메모리 위에서 부하를 건다 — 호스트 하드다운 계보가 있는 축이다.
  실행하려면 --confirm-risk 를 붙여라(에이전트는 챗 경고톤 Y/N 승인 뒤에만 붙일 것).
MSG
    exit 5
  fi
  for pair in "--cell-key:$CELL_KEY" "--config:$CONFIG" "--bench-budget-mib:$BENCH_BUDGET" \
              "--axis-citation:$CITATION"; do
    [ -n "${pair#*:}" ] || { echo "[broad_search] ERROR ${pair%%:*} 는 필수다" >&2; exit 2; }
  done
  [ -f "$STATE" ] || { echo "[broad_search] ERROR 상태 파일 부재: $STATE (먼저 init)" >&2; exit 2; }

  # 진입 전 정지 조건. 이미 멈춰야 하는 스윕에 셀을 하나 더 밀어 넣지 않는다.
  set +e; _stop; STOPRC=$?; set -e
  if [ "$STOPRC" = "3" ]; then
    echo "[broad_search] 정지 조건 성립 — 셀을 실행하지 않는다:" >&2
    python3 -c "import json;d=json.load(open('$STOPJSON'));print('  stopped_by:', d['stopped_by'])" >&2
    exit 0
  fi

  if [ -n "$SERVE_FAILED_REASON" ]; then
    # materialize 단계에서 죽은 셀 — 트리플렛이 없으므로 envfile·포트·측정 경로를 타지 않는다.
    STARTED="$NOW"; ENDED="$(date -u +%FT%TZ)"; SERVE_RC=3; MEASURE_RC=0
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

  MEASURE_RC="${MEASURE_RC:-0}"
  SWEEPDIR="$REPO/output/$TOPO/benchlog/sweep_${CONFIG}"
  if [ "$SERVE_RC" = "0" ] && [ "$REASSEMBLE" != "1" ]; then
    set +e
    SB_ARGS=("$CONFIG" --topology "$TOPO" --tool guidellm --bench-budget-mib "$BENCH_BUDGET")
    [ -n "$BACKEND" ] && SB_ARGS+=(--backend "$BACKEND")
    [ -n "$MAX_ERROR_RATE" ] && SB_ARGS+=(--max-error-rate "$MAX_ERROR_RATE")
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
                    "attention_backend_mismatch": meta.get("attention_backend_mismatch"),
                    "attention_backend_candidates": meta.get("attention_backend_candidates"),
                    "moe_backend_source": meta.get("moe_backend_source")},
    "verdict_narrative": (("%s · authority=%s · source=%s · floor=%s"
                           % (verdict.get("verdict"),
                              (verdict.get("rubric") or {}).get("authority"),
                              (verdict.get("rubric") or {}).get("source"),
                              (verdict.get("rubric") or {}).get("floor")))
                          if verdict else None),
    "axis_citation": os.environ["CITATION"],
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
  set +e; _stop; rc=$?; set -e
  python3 -c "import json;d=json.load(open('$STOPJSON'));print('[broad_search] stop=%s by=%s 남은셀=%d'%(d['stop'],d['stopped_by'],len(d['cells_remaining'])))"
  exit 0
  ;;

*) echo "[broad_search] 알 수 없는 서브커맨드: $CMD (init|cell|status|map)" >&2; exit 2;;
esac
