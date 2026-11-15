#!/usr/bin/env bash
# sweep_bench.sh — client-load 부하 스윕 (adversarial-benchmark full 모드 · report/인증서 입력 생성)
#
# 편지 패턴 B.4: 단일 running serve 에 **client-load 만** 변화(동시성 레벨 스윕) → 부하별 곡선.
#   **reload 0** — 벤치마커 "기동 안 함" 불변식 보존(config-space=batch×maxlen reload 는 Max 모드/explorer 소관).
#   판정점(동시성=1)을 스윕이 포함 → verdict 는 그 레벨을 재사용(재측정 0).
#   적응 상한 클램프 + **절삭 로그**(silent truncation 금지): 레벨 실패 시 그 레벨 절삭·기록하고 상위 중단.
# 각 레벨 = run_bench.sh 메커니즘 재사용(같은 serve·컨테이너·`--max-concurrency L`·request-rate inf).
#   run_bench 가 Flag/A2A 게이트·health precheck·envfile 해소를 수행 → 전이적 게이트 보존.
# 비용 규율(편지 B.5): 이 스윕은 재탐색 루프 내부가 아니라 **full 런 종결 시 1회**만 호출한다.
#
# 사용: sweep_bench.sh <config_name> [--topology single|multi] [--serve-plane docker|native] [--host-endpoint URL] [--engine-log PATH(native 필수)]
#        [--campaign-cell CELL(캠페인 셀 bench phase 를 직접 적는다 · 명시로만 · plan_26101722 K5)]
#        [--levels CSV] [--backend openai-chat|openai] [--bench-budget-mib N]
#        [--input-len N] [--output-len N] [--num-prompts N] [--warmups N] [--vllm-version X] [--dry-run]
#        [--preset NAME|PATH] [--knob-reason KNOB=CODE]... [--approved-by TEXT] [--pin KNOB=VALUE... --pinned-by TEXT]
#        [--repeats N [--repeats-source TEXT]] [--campaign-id ID] [--reassemble-only]
#        [--sweep-dir output/<topo>/benchlog/sweep_<unique-run>] (새 측정은 기존 경로 거부)
#        --check-sweep-dir --sweep-dir PATH --topology T (경로만 읽기 검증하고 정규화 경로 출력)
#        [--cross-node-tolerance-s N --cross-node-tolerance-source TEXT]   (multi 원격 노드 사살 대조 · 선언으로만)
#
# ★ 측정 형상은 프리셋이 정한다(2026-10-10 · plan_26101011 · interview_20261010_015151). full-bench(현재 구현체 GuideLLM)
#   의 knob 기본값·선택지·등급은 `config/<바인딩 폴더>/<preset>.yaml`(기본 default_preset) 하나에 있고, 이 스크립트에는
#   측정 형상의 리터럴 기본값이 없다. 해소·검증·기록의 소유자는 `bench_preset.py resolve` 이며 **어떤 부하도 걸기 전에**
#   부른다: 사람 핀(--pin K=V --pinned-by) > 사람 핀(campaign.yaml budgets.bench_pins · budgets.repeats) > Agent 선택
#   (스윕 플래그 + --knob-reason 근거 코드 · confirm 은 --approved-by 사전 승인) > 프리셋 default. 선택지 밖·escalate 를
#   Agent 가 고름·승인 없는 confirm·핀 충돌·서빙 상한(max-num-seqs·max-model-len) 위반은 exit 2(새 차단점 ✗ · 기동 거부).
#   결과는 sweep_index.json 최상위 `bench_knobs` 에 실린다. 적응 클램프는 이 체계와 분리된 실패 안전 장치다.
#
# ★ 반복 축(2026-09-14 · plan_26091407 §4.4 · 사용자 결정 Q3) — full 의 정의는 `lite ∪ GuideLLM × 반복 ≥3` 이다.
#   · 반복 대상은 **레벨 측정 레그**(`--tool` 이 고른 도구 · full 에서는 GuideLLM)다. lite 선행 레그는 반복하지
#     않는다 — cold TTFT 는 warmup 0 첫 요청이라 두 번째부터는 cold 가 아니고, lite 는 inform-only 스냅샷이다.
#     레벨 run 들의 spec 축 승계원은 그 **한 번의** lite warm JSON 을 함께 쓴다(`_spec_axis_args`).
#   · 반복 수 = `--repeats N` > 활성 캠페인(`--campaign-id` 가 있으면 그 캠페인) `campaign.yaml budgets.repeats` >
#     프리셋 기본값. 해소·값역·하한의 소유는 `repeat_axis.py` 이며(프리셋 등급 검사는 bench_preset) 출처는
#     `repetition.requested_source` 에 남는다. 귀속 없는 `--repeats N`(비기본)은 Agent 선택이라 escalate 로 거부된다 —
#     선언을 옮기는 호출자는 `--repeats-source` 로 귀속을 밝힌다(broad_search 가 그렇게 한다).
#     **N<3 명시는 exit 2** 다(full 정의 위반 — lite 만 원하면 lite_bench.sh). 선언을 읽지 못해도 exit 2 다.
#     `--reassemble-only` 에 반복 인자를 주면 exit 2 다(재조립은 측정 시점 index 의 요청 반복을 승계한다).
#   · 레벨마다 run 1..N 을 같은 serve 에 연속으로 잰다(`repeat_kind=warm-rerun` · cold-restart 는 선언 슬롯만).
#     run 1 은 `level_NN/` 에(종전 배치 그대로 — 판정점·인증서·judge_bench 승계가 읽는 대표 run), run k≥2 는
#     `level_NN/run_KK/` 에 쓴다. 레벨 종료 시 `repeat_axis.py aggregate-level` 이 대표 measured.json 에
#     `runs[]`·`repro_band_pct`·`repro_band_source=measured(n=N)`·`repeat_kind` 를 덧붙인다(대표 필드는 그대로).
#   · run 1 실패 = 종전 적응 상한 클램프(레벨 절삭 · 경계 사실은 `repetition.clamp_run`). run k≥2 실패
#     (measurement_ok=false · run_bench 비0) = **반복 중단 즉시 신호** — 남은 반복과 상위 레벨을 멈추고
#     truncation.log 에 적는다(끊긴 동시성은 반복해 버티지 못한 포화 경계라 상위도 무너진다 — 클램프와 같은
#     이유). 스윕이 멈춘 자리는 `repetition.stop` 하나다. 이 스크립트는 raw runs[] 와 즉시 신호만 쓰고,
#     종료부에서 판정 소유자 `classify_cell.py`(bench-mode 모드)를 **불러** 판정 기록 `bench_mode.json` 을
#     남긴다(판정 규칙은 여기 없다 — 정규 경로 sweep_bench → judge_bench → render_report 에서도 기록이 생긴다).
#     분산(밴드 폭)은 강등 사유가 아니다. 정상 경로(판정점 반복 완주 · 사살 없음 · 포화 경계 클램프 포함)는
#     강등 경로를 밟지 않는다.
#
# --reassemble-only: **측정하지 않고** 기존 SWEEPDIR 의 raw(level_NN/measured.json · lite raw ·
#   truncation.log)에서 sweep_index.json 만 다시 조립한다(복원 레벨은 **기존 index 의 levels[]** 로 한정 —
#   디스크에 남은 이전 스윕의 level_NN 을 이번 측정에 섞지 않는다). 라벨/파생키 계약이 바뀌었을 때
#   (예: 2026-08-15 model 강한키 파생 교정 · 2026-08-23 quantization/kv_cache_dtype 실측 승격)
#   **재측정 없이** 산출물을 정합화하는 유일한 정식 경로다 —
#   대안은 인증서 수기 편집(=증거 위조)이거나 재측정(=측정치가 아니라 라벨 문제인데 비용 지불)뿐이다.
#   측정시각(`generated_utc`)·실측 image_tag 는 **기존 index 에서 승계**한다(측정이 안 바뀌었으니
#   측정시각도 안 바뀐다). 기존 index 가 없으면 시각을 날조하는 대신 fail-closed 로 멈춘다.
# 산출: output/<topo>/benchlog/sweep_<config>/{level_NN/{bench_<config>.json|guidellm_<config>.json,engine_<config>.log,
#          measured.json(대표 run + runs[]·repro_band),repeat_run.json, run_KK/{…,measured.json,repeat_run.json}},
#        truncation.log, sweep_index.json, bench_mode.json(writer: classify_cell.py)}
#        ← render_report.py·publish_benchmark_record.py·judge_bench.sh·broad_search.sh 가 소비.
# 종료: 0=성공(레벨 ≥1 완료) · 2=인자/전제 오류 · 3=판정점(레벨1) 측정 불가(serve 미가동/게이트 등)
#       6/7=lite 게이트 불통과(6 측정 경로 불성립 · 7 서버 응답 실패 · GuideLLM 미진입 · 2026-09-29 plan_26092923).
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
TOPO=""; SERVE_PLANE="docker"; HOST_ENDPOINT=""; CLIENT_VLLM=""; NATIVE_ELOG=""; VLLM_VER=""; DRYRUN=0; REASSEMBLE=0
SWEEP_DIR_ARG=""; CHECK_SWEEP_DIR=0
# 측정 형상은 빈 값으로 출발한다 — 기본값의 자리는 프리셋 하나다(위 헤더 ★ · bench_preset.py 가 채운다).
LEVELS=""; ILEN=""; OLEN=""; NPROMPTS=""; WARMUPS=""
PRESET=""; PINNED_BY=""; APPROVED_BY=""; PIN_ARGS=(); KNOB_REASON_ARGS=()
# ★ 2026-09-01 신설 — run_bench.sh 의 --backend 를 레벨마다 그대로 전달한다.
#   전달하지 않으면 스윕 전 레벨이 openai-chat 로 돌아, harmony 계열(gpt-oss)에서 `--ignore-eos` 가
#   무력해져 **모든 레벨의 TPOT 이 동시에 왜곡**된다(run_bench.sh 의 BACKEND 주석 참조).
#   판정점(동시성=1)을 스윕이 포함하므로 그 왜곡은 곧 verdict 왜곡이다.
# ★ 2026-09-07: 여기서도 **기본값을 없앤다**(plan_26090715 §5 ⑤-①). 하류 run_bench 가 기본값을
#   버렸는데 이쪽이 들고 있으면 그 버림이 무효가 된다 — 기본값은 마지막 한 자리만 남아도 이긴다.
BACKEND=""
# ★ 2026-09-04(CP5 · plan_26090415 §3.1·§3.7) — 모드별 측정 도구 선택.
#   lite 레그는 이 노브와 무관하게 **언제나 `vllm bench serve`** 다(도구 구성 `full = lite ∪ GuideLLM` · 반복 축은 위 ★).
#   두 레그를 이질적으로 유지하는 것이 설계이며, 그 이질성이 실결함 2건을 잡았다(2026-09-01·09-03).
#   `--tool guidellm` 은 **레벨 측정만** 옮긴다.
TOOL="vllm"
BENCH_BUDGET_MIB=""
# 오류 허용치는 **선언에서만** 온다(기본 빈값 = 파서 기본 0 = 엄격).
MAX_ERROR_RATE=""
# 반복 축(위 헤더 ★). 빈 값 = repeat_axis 가 캠페인 선언·full 정의에서 해소한다(여기에 기본 숫자를 적지 않는다).
REPEATS_ARG=""; REPEATS_SOURCE_ARG=""; CAMPAIGN_ID=""
# 캠페인 셀 bench phase 를 이 실행이 직접 적는다(2026-10-17 · plan_26101722 K5 — lite_bench 의 같은 인자와 같은 규율).
#   명시로만 켠다: broad_search 는 자기 트랜잭션에서 적으므로 넘기지 않고, 여기서 부르는 lite 레그에도 넘기지 않는다(이중 기록 ✗).
CAMPAIGN_CELL=""
# 노드 간 사살 대조 허용오차(2026-09-14 · ⑧ 분석 발견 T1). **선언으로만** 온다 — 값과 출처를 함께 받아 판정 소유자
#   (classify_cell)에 넘긴다. 미선언이면 원격 노드 창을 넓히지 않고 미스를 결론내지 않는다(unavailable · 강등 트리거 ✗).
XNODE_TOL=""; XNODE_TOL_SRC=""
while [ $# -gt 0 ]; do case "$1" in
  --cross-node-tolerance-s) XNODE_TOL="$2"; shift 2;;
  --cross-node-tolerance-source) XNODE_TOL_SRC="$2"; shift 2;;
  --repeats) REPEATS_ARG="$2"; shift 2;;
  --preset) PRESET="$2"; shift 2;;
  --pin) PIN_ARGS+=(--pin "$2"); shift 2;;
  --pinned-by) PINNED_BY="$2"; shift 2;;
  --knob-reason) KNOB_REASON_ARGS+=(--knob-reason "$2"); shift 2;;
  --approved-by) APPROVED_BY="$2"; shift 2;;
  --repeats-source) REPEATS_SOURCE_ARG="$2"; shift 2;;
  --campaign-id) CAMPAIGN_ID="$2"; shift 2;;
  --campaign-cell) CAMPAIGN_CELL="$2"; shift 2;;
  --topology) TOPO="$2"; shift 2;;
  --sweep-dir)
    [ $# -ge 2 ] && [ -n "$2" ] || { echo "[sweep_bench] --sweep-dir requires a nonempty path" >&2; exit 2; }
    SWEEP_DIR_ARG="$2"; shift 2;;
  --check-sweep-dir) CHECK_SWEEP_DIR=1; shift;;
  --serve-plane) SERVE_PLANE="$2"; shift 2;;
  --host-endpoint) HOST_ENDPOINT="$2"; shift 2;;
  --client-vllm) CLIENT_VLLM="$2"; shift 2;;
  --engine-log) NATIVE_ELOG="$2"; shift 2;;
  --tool) TOOL="$2"; shift 2;;
  --bench-budget-mib) BENCH_BUDGET_MIB="$2"; shift 2;;
  --max-error-rate) MAX_ERROR_RATE="$2"; shift 2;;
  --backend) BACKEND="$2"; shift 2;;
  --reassemble-only) REASSEMBLE=1; shift;;
  --levels) LEVELS="$2"; shift 2;;
  --input-len) ILEN="$2"; shift 2;;
  --output-len) OLEN="$2"; shift 2;;
  --num-prompts) NPROMPTS="$2"; shift 2;;
  --warmups) WARMUPS="$2"; shift 2;;
  --vllm-version) VLLM_VER="$2"; shift 2;;
  --dry-run) DRYRUN=1; shift;;
  *) echo "[sweep_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$SDIR" rev-parse --show-toplevel 2>/dev/null || pwd)"

# 명시 출력은 과거 sweep을 덮지 않는 새 direct child다. broad_search의 선검사와
# 실제 측정 진입이 이 한 검증자를 쓴다. check는 읽기 전용, claim은 mkdir의 EEXIST로 경합을 거부한다.
_selected_sweep_dir() {
  bash "$SDIR/bench_shell.sh" sweep-path "$REPO" "$TOPO" "$SWEEP_DIR_ARG" "$REASSEMBLE" "$1"

}
if [ "$CHECK_SWEEP_DIR" = 1 ]; then
  # No preset, manifest, health, phase, filesystem write or measurement in this path.
  _selected_sweep_dir check
  exit $?
fi

case "$SERVE_PLANE" in
  docker|native) ;;
  *) echo "[sweep_bench] 알 수 없는 --serve-plane: $SERVE_PLANE (docker|native)" >&2; exit 2;;
esac
if [ "$SERVE_PLANE" = "native" ] && { [ -z "$HOST_ENDPOINT" ] || [ -z "$CLIENT_VLLM" ]; }; then
  echo "[sweep_bench] ERROR --serve-plane native에는 --host-endpoint URL 과 --client-vllm <전용 venv/bin/vllm>이 필수다." >&2; exit 2
fi
if [ "$SERVE_PLANE" = "native" ] && [[ ! "$HOST_ENDPOINT" =~ ^https?://[^/[:space:]]+(:[0-9]+)?$ ]]; then
  echo "[sweep_bench] ERROR --host-endpoint는 경로 없는 http(s) origin 이어야 한다: $HOST_ENDPOINT" >&2; exit 2
fi
# ★ 2026-09-29(plan_26092923_58_27): native 는 lite 레그에 서버 로그를 넘겨야 한다. 이 인자가 없던 동안 native lite 는 KV 가
#   N/A 라 늘 ① 이었고, lite 게이트(221627c) 이후 native full 스윕은 GuideLLM 에 한 번도 들어가지 못했다. 선언으로만 받는다.
if [ "$SERVE_PLANE" = "native" ] && [ "$REASSEMBLE" != "1" ] && [ -z "$NATIVE_ELOG" ]; then
  echo "[sweep_bench] ERROR --serve-plane native에는 --engine-log <native producer 의 서버 로그>가 필수다(lite 레그 판정 입력)." >&2; exit 2
fi
if [ -n "$NATIVE_ELOG" ] && [ ! -f "$NATIVE_ELOG" ]; then
  echo "[sweep_bench] ERROR --engine-log 파일이 없다: $NATIVE_ELOG" >&2; exit 2
fi
if [ "$SERVE_PLANE" = "docker" ] && { [ -n "$HOST_ENDPOINT" ] || [ -n "$CLIENT_VLLM" ] || [ -n "$NATIVE_ELOG" ]; }; then
  echo "[sweep_bench] ERROR --host-endpoint/--client-vllm/--engine-log은 --serve-plane native에서만 준다." >&2; exit 2
fi
if [ "$SERVE_PLANE" = "native" ] && { [ ! -x "$CLIENT_VLLM" ] || [ -L "$CLIENT_VLLM" ]; }; then
  echo "[sweep_bench] ERROR --client-vllm이 실행 가능한 regular non-symlink 파일이 아니다: $CLIENT_VLLM" >&2; exit 2
fi

# 재조립은 측정하지 않는다 — 반복 인자를 받으면 조용히 버리지 않고 거부한다(요청 반복은 측정 시점 index 에서 승계).
if [ "$REASSEMBLE" = "1" ] && { [ -n "$REPEATS_ARG" ] || [ -n "$REPEATS_SOURCE_ARG" ] || [ -n "$CAMPAIGN_ID" ]; }; then
  echo "[sweep_bench] ERROR --reassemble-only 에는 --repeats/--repeats-source/--campaign-id 를 줄 수 없다 —" >&2
  echo "  재조립은 측정하지 않으므로 요청 반복·출처를 **측정 시점** index 에서 승계한다(측정하지 않은 반복 수 ✗)." >&2
  exit 2
fi

# 노드 간 허용오차는 값·출처가 짝이다 — 부하 전에 친다(판정 기록 단계에서야 드러나면 스윕 한 판이 기록 없이 끝난다).
if { [ -n "$XNODE_TOL" ] && [ -z "$XNODE_TOL_SRC" ]; } || { [ -z "$XNODE_TOL" ] && [ -n "$XNODE_TOL_SRC" ]; }; then
  echo "[sweep_bench] ERROR --cross-node-tolerance-s 와 --cross-node-tolerance-source 는 함께 준다(출처 없는 숫자 ✗)" >&2; exit 2
fi
case "$XNODE_TOL" in
  *[!0-9]*) echo "[sweep_bench] ERROR --cross-node-tolerance-s 는 0 이상 정수(초)다: '$XNODE_TOL'" >&2; exit 2;;
esac

# 측정 엔드포인트 미선언을 **여기서** 친다 — 뒤로 미루면 lite 레그를 다 돌고 나서야 드러나고,
# 그때는 이미 잘못된 포맷으로 잰 판정점이 손에 있다(2026-09-07 · 유예 결함 ①).
#   ★ 2026-10-10(plan_26101011): 값이 비면 여기서 멈추지 않는다 — 사람 핀(--pin backend= · campaign.yaml bench_pins)
#     이 채울 수 있으므로 판정은 프리셋 해소 뒤(아래 `_backend_required`)다. 기본값이 생긴 것은 아니다(프리셋 default null).
_backend_required() {
  echo "[sweep_bench] ERROR --backend 는 필수다(기본값 없음 · 2026-09-07)." >&2
  echo "  요청 포맷은 TPOT 을 바꾸는 1급 측정 축이고, 스윕은 그 포맷으로 **전 레벨**을 잰다." >&2
  echo "  harmony 계열(gpt-oss)은 --backend openai (완결 엔드포인트) + --knob-reason backend=harmony_completion." >&2
  exit 2
}
case "$BACKEND" in
  openai|openai-chat|"") ;;
  *) echo "[sweep_bench] 알 수 없는 --backend: $BACKEND (openai|openai-chat)" >&2; exit 2 ;;
esac

case "$TOOL" in
  vllm) ;;
  guidellm)
    # 예산 미선언은 프리셋 해소 직후(부하 전) `_budget_required` 가 친다 — 사람 핀이 채울 수 있다(plan_26101011).
    case "$BENCH_BUDGET_MIB" in
      ''|[0-9]*) ;;
      *) echo "[sweep_bench] ERROR --bench-budget-mib 는 양의 정수다: '$BENCH_BUDGET_MIB'" >&2; exit 2;;
    esac
    ;;
  *) echo "[sweep_bench] 알 수 없는 --tool: $TOOL (vllm|guidellm)" >&2; exit 2;;
esac

REPEATS=""; REPEATS_SOURCE=""; REPEAT_KIND=""
if [ -z "$TOPO" ]; then
  # ★ 2026-09-14(⑧ 분석 발견 T8 · 헌법 "토폴로지는 manifest 에서 읽고 브랜치로 추론하지 않는다"): 종전 관용구는
  #   브랜치 이름에서 토폴로지를 골랐고 알 수 없는 이름은 조용히 single 로 떨어졌다. 해소는 공용 해소기가 한다 —
  #   카드 노드(unsigned capability Card)는 카드↔자기 manifest, 메인은 4자일치 술어(topology_parity), 둘 다 아니면 fail-loud.
  _TOPO_RC=0; TOPO="$(bash "$SDIR/resolve_topology.sh" --repo "$REPO")" || _TOPO_RC=$?
  if [ "$_TOPO_RC" != 0 ]; then
    echo "[sweep_bench] ERROR 토폴로지를 정하지 못했다(위 사유) — --topology single|multi 로 명시하라(브랜치 이름으로 고르지 않는다)" >&2
    exit 2
  fi
fi

CFGYAML="$REPO/output/$TOPO/configs/${CONFIG}.yaml"
EF="$REPO/output/$TOPO/envs/.env.${CONFIG}"
MANIFEST="$REPO/output/$TOPO/manifest.yaml"
SWEEPDIR="$REPO/output/$TOPO/benchlog/sweep_${CONFIG}"
if [ -n "$SWEEP_DIR_ARG" ]; then
  _SWEEP_ACTION=claim
  [ "$DRYRUN" = 1 ] && _SWEEP_ACTION=check
  SWEEPDIR="$(_selected_sweep_dir "$_SWEEP_ACTION")" || exit 2
fi

# ── 측정 형상 해소(프리셋 · 2026-10-10 plan_26101011) — **어떤 부하도 걸기 전에** 친다 ─────────────────────────
#   재조립은 측정하지 않으므로 해소하지 않는다 — 측정 시점의 `bench_knobs`·요청 반복을 기존 index 에서 승계한다(아래 PY).
#   반복 수 해소는 bench_preset 이 repeat_axis 를 불러 하고(argv > 캠페인 > 프리셋 기본값), 등급·선택지 검사를 덧건다.
KNOB_RECORD=""
if [ "$REASSEMBLE" != "1" ]; then
  # dry-run 은 디스크에 산출 디렉터리를 만들지 않는다(종전 계약) — 기록은 임시 파일로만 낸다.
  if [ "$DRYRUN" = "1" ]; then KNOB_RECORD="$(mktemp)"; else mkdir -p "$SWEEPDIR"; KNOB_RECORD="$SWEEPDIR/.bench_knobs.pending.json"; fi
  _BP=(resolve --tool "$TOOL" --serving-yaml "$CFGYAML" --record-out "$KNOB_RECORD")
  [ "$DRYRUN" = "1" ] && _BP+=(--dry-run)
  [ -n "$PRESET" ] && _BP+=(--preset "$PRESET")
  [ -n "$LEVELS" ] && _BP+=(--levels "$LEVELS")
  [ -n "$ILEN" ] && _BP+=(--input-len "$ILEN")
  [ -n "$OLEN" ] && _BP+=(--output-len "$OLEN")
  [ -n "$NPROMPTS" ] && _BP+=(--num-prompts "$NPROMPTS")
  [ -n "$WARMUPS" ] && _BP+=(--warmups "$WARMUPS")
  [ -n "$BACKEND" ] && _BP+=(--backend "$BACKEND")
  [ -n "$BENCH_BUDGET_MIB" ] && _BP+=(--bench-budget-mib "$BENCH_BUDGET_MIB")
  [ -n "$REPEATS_ARG" ] && _BP+=(--repeats "$REPEATS_ARG")
  [ -n "$REPEATS_SOURCE_ARG" ] && _BP+=(--repeats-source "$REPEATS_SOURCE_ARG")
  [ -n "$CAMPAIGN_ID" ] && _BP+=(--campaign-id "$CAMPAIGN_ID")
  [ -n "$PINNED_BY" ] && _BP+=(--pinned-by "$PINNED_BY")
  [ -n "$APPROVED_BY" ] && _BP+=(--approved-by "$APPROVED_BY")
  [ "${#PIN_ARGS[@]}" -gt 0 ] && _BP+=("${PIN_ARGS[@]}")
  [ "${#KNOB_REASON_ARGS[@]}" -gt 0 ] && _BP+=("${KNOB_REASON_ARGS[@]}")
  _BPO="$(bash "$SDIR/bench_preset.sh" "${_BP[@]}")" || {
    echo "[sweep_bench] ERROR 측정 형상 프리셋 해소 실패(위 사유) — 측정하지 않는다(knob 계약 · plan_26101011)" >&2; exit 2; }
  PRESET_PATH=""
  while IFS=$'\x1f' read -r _k _v; do
    case "$_k" in
      LEVELS) LEVELS="$_v";; ILEN) ILEN="$_v";; OLEN) OLEN="$_v";; NPROMPTS) NPROMPTS="$_v";; WARMUPS) WARMUPS="$_v";;
      REPEATS) REPEATS="$_v";; REPEATS_SOURCE) REPEATS_SOURCE="$_v";; REPEAT_KIND) REPEAT_KIND="$_v";;
      BACKEND) BACKEND="$_v";; BENCH_BUDGET_MIB) BENCH_BUDGET_MIB="$_v";; PRESET_PATH) PRESET_PATH="$_v";;
    esac
  done <<< "$_BPO"
  [ -n "$BACKEND" ] || _backend_required
  if [ "$TOOL" = "guidellm" ]; then
    case "$BENCH_BUDGET_MIB" in
      ''|*[!0-9]*) echo "[sweep_bench] ERROR --tool guidellm 에는 --bench-budget-mib <양의 정수> 가 필수다(기본값 없음 · §4.8)" >&2; exit 2;;
    esac
    [ "$BENCH_BUDGET_MIB" -gt 0 ] || { echo "[sweep_bench] ERROR --bench-budget-mib 는 양수여야 한다" >&2; exit 2; }
  fi
  echo "[sweep_bench] 측정 형상 = 프리셋 ${PRESET:-default_preset} ($PRESET_PATH) · 기록 → sweep_index.bench_knobs"
  # 데이터셋 해소값(2026-10-16 · plan_26101614) — 기록 블록의 `dataset` 을 run_bench 에 그대로 넘긴다(두 번째 해소 ✗).
  #   전처리 캐시는 스윕 폴더 단위(모델 토크나이저가 같으면 재사용) · 런 번호는 레벨·반복을 가로질러 늘어난다.
  if [ "$DRYRUN" = "1" ]; then DATASET_SPEC="$(mktemp)"; else DATASET_SPEC="$SWEEPDIR/.dataset_spec.json"; fi
  DATASET_CACHE="$SWEEPDIR/.dataset_cache"
  bash "$SDIR/bench_shell.sh" extract-dataset \
    "$KNOB_RECORD" "$DATASET_SPEC" || { echo "[sweep_bench] ERROR 데이터셋 해소값을 꺼내지 못했다" >&2; exit 2; }
  DATASET_KIND="$(bash "$SDIR/bench_shell.sh" dataset-kind "$DATASET_SPEC")"
  echo "[sweep_bench] 데이터셋 = $DATASET_KIND (프리셋 해소값 · 판정 불변 · 출처는 런별 사이드카 → meta)"
fi
DATASET_RUN_INDEX=0
DATASET_SWEEP_SALT="$(date -u +%FT%TZ)"   # 스윕마다 다른 행 시작점(prefix cache 재적중 회피 · sidecar 에 기록 · plan_26101622 F2 후속)

# 레벨 파싱: 정렬·중복제거 + 판정점(1) 강제 포함(verdict 재사용 보장).
IFS=',' read -r -a _lv <<< "$LEVELS"
declare -A _seen; SORTED=()
for v in 1 "${_lv[@]}"; do
  v="$(echo "$v" | tr -d '[:space:]')"; [ -z "$v" ] && continue
  case "$v" in (*[!0-9]*) echo "[sweep_bench] 레벨은 정수만: '$v'" >&2; exit 2;; esac
  [ -n "${_seen[$v]:-}" ] && continue; _seen[$v]=1; SORTED+=("$v")
done
# 오름차순 정렬(적응 클램프 = 낮은 레벨부터, 실패 시 상위 중단)
IFS=$'\n' SORTED=($(printf '%s\n' "${SORTED[@]}" | sort -n)); unset IFS


echo "[sweep_bench] config=$CONFIG topo=$TOPO levels=[${SORTED[*]}] in=$ILEN out=$OLEN n=$NPROMPTS warmup=$WARMUPS tool=$TOOL${BENCH_BUDGET_MIB:+ bench_budget=${BENCH_BUDGET_MIB}MiB}"
echo "[sweep_bench] sweepdir=$SWEEPDIR (판정점=동시성1 재사용)"
[ "$REASSEMBLE" = "1" ] || echo "[sweep_bench] 반복 $REPEATS × 레벨 · $REPEAT_KIND · 출처 $REPEATS_SOURCE (lite 선행 레그는 1회)"
if [ "$DRYRUN" = "1" ]; then
  echo "[sweep_bench] DRY-RUN — 레벨별 실행 계획(serve-plane=$SERVE_PLANE${HOST_ENDPOINT:+ endpoint=$HOST_ENDPOINT}):"
  for L in "${SORTED[@]}"; do
    echo "  level $L × run 1..$REPEATS → run_bench.sh $CONFIG --topology $TOPO --serve-plane $SERVE_PLANE${HOST_ENDPOINT:+ --host-endpoint $HOST_ENDPOINT} --concurrency $L --tool $TOOL${BENCH_BUDGET_MIB:+ --bench-budget-mib $BENCH_BUDGET_MIB} --out-dir $SWEEPDIR/level_$(printf '%02d' "$L")[/run_KK]"
  done
  bash "$SDIR/bench_shell.sh" dry-knobs "$KNOB_RECORD" || true
  rm -f "$KNOB_RECORD" "${DATASET_SPEC:-}"
  echo "[sweep_bench] DRY-RUN 종료(실제 벤치·assemble 생략)"
  exit 0
fi

# ── 미완 표식(2026-10-26 · plan_26102607 · testlog_26102504 §2 #4) ─────────────────────────────────────
#   측정 진입부터 정상 종료(DONE)까지 SWEEPDIR 에 `sweep_incomplete.json` 을 둔다. 레벨 1 실패(exit 3)는 index 조립 전에
#   죽어서 **옛 sweep_index.json + 옛 level_01/measured.json 짝**이 그대로 남는다 — 짝이 서로 맞으니 파일만 봐서는
#   낡았는지 알 수 없고, judge_bench 가 그것으로 PASS 를 냈다(Solar 실측). 표식이 남아 있으면 judge 가 거부한다.
#   재조립(--reassemble-only)은 측정하지 않으므로 표식을 만들지도 지우지도 않는다(옛 실패를 재조립이 덮지 않는다).
SWEEP_INCOMPLETE="$SWEEPDIR/sweep_incomplete.json"
if [ "$REASSEMBLE" != "1" ]; then
  mkdir -p "$SWEEPDIR"
  printf '{"started_utc": "%s", "pid": %s, "config": "%s"}\n' "$(date -u +%FT%TZ)" "$$" "$CONFIG" > "$SWEEP_INCOMPLETE"
fi

if [ "$REASSEMBLE" = "1" ]; then
  [ -d "$SWEEPDIR" ] || { echo "[sweep_bench] --reassemble-only: SWEEPDIR 부재 — $SWEEPDIR" >&2; exit 2; }
  [ -s "$SWEEPDIR/sweep_index.json" ] || {
    echo "[sweep_bench] --reassemble-only: 기존 sweep_index.json 부재 — 측정시각을 날조하지 않는다(중단)" >&2; exit 2; }
else
  mkdir -p "$SWEEPDIR"
fi
TRUNCLOG="$SWEEPDIR/truncation.log"
[ "$REASSEMBLE" = "1" ] || : > "$TRUNCLOG"   # 재조립은 절삭 기록을 승계한다(지우면 그 사실이 사라진다)

if [ "$REASSEMBLE" = "1" ]; then
  # 완료 레벨을 **디스크에서** 복원한다. 판정 기준은 최초 루프와 동일(measured.json 의
  # measurement_ok=true) — 기준을 새로 쓰면 두 벌이 되어 어긋난다.
  # ★ 2026-09-14(리뷰 정정): 복원 대상은 **기존 index 의 levels[]** 로 한정한다. 스윕 디렉터리에는 이전 스윕이
  #   남긴 level_NN 이 있을 수 있고(이번 측정이 클램프·반복 중단으로 그 레벨에 닿지 않았다), 디스크 전체를 훑으면
  #   그 낡은 레벨(자기 runs[] 포함)이 측정시각을 승계한 이번 index 에 부활한다. index 가 측정의 권위다.
  _PRIOR_LEVELS="$(bash "$SDIR/bench_shell.sh" prior-levels "$SWEEPDIR/sweep_index.json")" || { echo "[sweep_bench] --reassemble-only: 기존 index 판독 실패 — 중단" >&2; exit 2; }
  COMPLETED=()
  for _lv in $_PRIOR_LEVELS; do
    _d="$SWEEPDIR/level_$(printf '%02d' "$_lv")"
    _m="$_d/measured.json"; [ -s "$_m" ] || continue
    bash "$SDIR/bench_shell.sh" measurement-ok "$_m" \
      || continue
    COMPLETED+=("$_lv")
  done
  if [ "${#COMPLETED[@]}" -eq 0 ]; then
    echo "[sweep_bench] --reassemble-only: 기존 index levels[] 중 복원 가능한 레벨 0개(measurement_ok) — 중단" >&2; exit 3
  fi
  IFS=$'\n' COMPLETED=($(printf '%s\n' "${COMPLETED[@]}" | sort -n)); unset IFS
  case " ${COMPLETED[*]} " in *" 1 "*) :;; *) echo "[sweep_bench] --reassemble-only: 판정점(레벨1) 부재 — 중단" >&2; exit 3;; esac
  LITE_RAW="$SWEEPDIR/lite_raw_${CONFIG}.json"
  [ -s "$LITE_RAW" ] || { echo "[sweep_bench] ⚠ --reassemble-only: lite raw 부재 — lite 블록 결손 승계" | tee -a "$TRUNCLOG"; LITE_RAW=""; }
  IMAGE_TAG_ACTUAL="NA"   # 재조립은 docker 를 건드리지 않는다 — 실측값은 기존 index 에서 승계(아래 PY)
  IMAGE_DIGEST_ACTUAL="NA"  # 동상(태그와 같은 승계 규율)
  echo "[sweep_bench] --reassemble-only: 측정 0회 · 복원 레벨 [${COMPLETED[*]}] · 측정시각 승계"
else

# ── full ⊇ lite 불변식 (2026-07-31 사용자 지시) ────────────────────────────────
# full 은 lite 의 **상위집합**이어야 한다. 종전엔 교집합이었다 — lite 만 cold TTFT·시스템 RAM·
# per-node nvidia-smi 를 재고, full 만 스윕·루프라인·verdict 를 냈다. 그러면 모드가 다른 두 모델의
# 지표 열(column) 집합이 달라져 **조건별 비교가 성립하지 않는다**(사용자 지적).
#
# 병렬 목록을 손으로 맞추는 방식은 쓰지 않는다 — 같은 날 파서 두 벌(D4↔D6)이 정확히 그 방식으로
# 어긋났다. 대신 **full 이 lite 를 실제로 실행해서 포함**한다. 포함관계가 구조로 보장되고,
# lite 에 지표가 추가되면 full 이 자동으로 따라온다(동기화 대상이 애초에 없다).
#
# 순서: lite 를 **먼저**. lite 의 cold TTFT 는 warmup 0 측정이라 스윕이 엔진을 데운 뒤에 재면
# 더 이상 cold 가 아니다.
# 캠페인 셀 bench phase(K5) — 측정 진입 = running · 종료 = done(rc 0)/failed(그 밖 · EXIT trap). 노드는 배정에서 파생된다.
_CI="$REPO/.claude/skills/terraforming_node/scripts/campaign_init.py"
_CAMPAIGN_STARTED=""
_campaign_phase() {  # $1=state(running|done|failed) $2=rc
  local _now; _now="$(date -u +%FT%TZ)"
  local _a=(--phase-set bench --cell "$CAMPAIGN_CELL" --state "$1")
  if [ "$1" = running ]; then _CAMPAIGN_STARTED="$_now"; _a+=(--started-utc "$_now"); else
    _a+=(--started-utc "$_CAMPAIGN_STARTED" --ended-utc "$_now" --proof-predicate "sweep_bench exit=$2 (0=스윕 완주)")
    if [ "$1" = done ]; then _a+=(--proof-ok --proof-source "sweep_bench: $SWEEPDIR/sweep_index.json"); fi
  fi
  bash "${_CI%.py}.sh" "${_a[@]}" >&2 \
    || echo "[sweep_bench] ⚠ 캠페인 셀 '$CAMPAIGN_CELL' bench phase=$1 기록 실패(위 사유) — 진행표가 낡는다" >&2
}
_campaign_exit() {
  local rc=$?
  if [ "$rc" = 0 ]; then _campaign_phase done 0; else _campaign_phase failed "$rc"; fi
  return "$rc"
}
if [ -n "$CAMPAIGN_CELL" ]; then
  _campaign_phase running 0
  trap _campaign_exit EXIT
fi
echo "[sweep_bench] ── lite 포함 실행(full ⊇ lite 불변식) ──"
LITE_RAW="$SWEEPDIR/lite_raw_${CONFIG}.json"
# ★ 2026-09-03: `--backend "$BACKEND"` 가 **이 호출에만 빠져 있었다**(레벨 호출에는 있다).
#   2026-09-01 에 harmony 왜곡 대응으로 세 파일(run_bench·sweep_bench·lite_bench)에 노브를 신설했는데
#   sweep→lite 호출부 하나가 전달을 빠뜨렸다. 그래서 full 런의 lite 열은 **언제나 openai-chat** 으로
#   재졌고, gpt-oss 에서 gen_tps 11.83 vs 같은 런의 full decode 34.42 (2.9배)가 나왔다 —
#   2026-09-01 에 인증서까지 갔던 그 왜곡과 **같은 뿌리**다. 노브를 만든 것과 그것이 도는 것은 다르다.
LITE_PLANE_ARGS=(--serve-plane "$SERVE_PLANE")
[ -n "$HOST_ENDPOINT" ] && LITE_PLANE_ARGS+=(--host-endpoint "$HOST_ENDPOINT")
[ -n "$CLIENT_VLLM" ] && LITE_PLANE_ARGS+=(--client-vllm "$CLIENT_VLLM")
[ -n "$NATIVE_ELOG" ] && LITE_PLANE_ARGS+=(--engine-log "$NATIVE_ELOG")
# ★ 2026-09-29(plan_26092923 · 인터뷰 interview_20260929_132122): lite 는 full 의 **진입 게이트**다(lite ⊂ full).
#   종전은 fail-soft 였다 — lite 가 실패해도 `lite truncated` 로 적고 GuideLLM 으로 넘어가, "full 은 됐는데 lite 는
#   성립하지 않음" 이라는 교집합 상태가 존재할 수 있었다(D1 · single lite 측정 0건 rc 0 · 2026-09-29 실측).
#   이제 lite 판정(raw 의 `lite_verdict` 단일 권위 · lite_metrics.read_lite_verdict)이 pass 가 아니면 **GuideLLM 에
#   들어가지 않고** lite 종료코드(6 = 측정 경로 불성립 · 7 = 서버 응답 실패)로 끝난다. raw 부재·어휘 밖 값·모르는
#   종료코드는 6 으로 읽는다(부재를 통과로 접지 않는다). index·리포트·인증서는 만들지 않는다 — 판정점 측정이 없다.
LITE_LOG="$SWEEPDIR/lite_leg.log"
LITE_RC=0
bash "$SDIR/lite_bench.sh" "$CONFIG" --topology "$TOPO" "${LITE_PLANE_ARGS[@]}" --backend "$BACKEND" --out-dir "$SWEEPDIR" >"$LITE_LOG" 2>&1 \
  || LITE_RC=$?
LITE_VERDICT="$(bash "$SDIR/bench_shell.sh" read-lite-verdict "$LITE_RAW" 2>/dev/null || true)"
if [ "$LITE_RC" = 0 ] && [ "$LITE_VERDICT" = "pass" ]; then
  echo "[sweep_bench] lite 게이트 통과(lite_verdict=pass) → $LITE_RAW"
else
  case "$LITE_RC" in 6|7) GATE_RC="$LITE_RC" ;; *) GATE_RC=6 ;; esac
  [ "$LITE_VERDICT" = "server_failed" ] && GATE_RC=7
  echo "[sweep_bench] ✗ lite 게이트 불통과(lite_verdict=${LITE_VERDICT:-부재} · lite_bench rc $LITE_RC) — GuideLLM 에 진입하지 않는다(lite ⊂ full)" | tee -a "$TRUNCLOG" >&2
  echo "lite gate failed: lite_verdict=${LITE_VERDICT:-absent} lite_rc=$LITE_RC → exit $GATE_RC (GuideLLM 미진입 · 인증서 ✗)" >> "$TRUNCLOG"
  tail -12 "$LITE_LOG" >&2 || true
  exit "$GATE_RC"
fi

# >>> spec-axis-args (selftest_sweep_meta.py 가 이 구간을 **바이트 그대로** 뽑아 실행한다 — 복제 ✗)
# GuideLLM 레벨 파서의 spec 축 인자를 정해 전역 배열 SPEC_AXIS_ARGS 에 싣는다.
#   spec 축은 GuideLLM 이 보고하지 않으므로 **같은 스윕의 lite 레그**에서 승계한다 — 이것이
#   도구 구성 `full = lite ∪ GuideLLM` 이 값으로 갚아 주는 자리다(반복 축을 포함한 정의는 위 헤더 ★ ·
#   `lite ∪ GuideLLM × 반복 ≥3`). 승계원이 없으면 부재를 명시 선언한다.
#   ★ 2026-09-14(plan_26091407 §4.1 · 항목 1): 승계원은 lite **raw 포인터 문서가 아니라** 그것이 가리키는
#     `vllm bench serve` warm JSON 이다. 종전에는 `lite_raw_<cfg>.json`(키: bench_warm_json·bench_cold_json·
#     engine_log·nodes)을 그대로 넘겨, 파서가 그 문서 최상위에서 수용길이를 찾다 **None 을 승계**했다 —
#     spec 을 선언한 GuideLLM 스윕의 measured.json 이 전부 accept_len=null 이 된 뿌리다(실측:
#     sweep_nv4-bf-262k-res 의 warm JSON 에는 spec_decode_acceptance_length=2.2069 가 있었다). 포인터 해석은
#     lite_metrics 가 이미 쓰는 `bench_warm_json` 키 하나로 한다.
#   ★ 그 warm JSON 이 승계원 모양인지는 **파서가 받기 전에** 파서의 술어(native `spec-source` ·
#     복제 ✗)로 판정한다(리뷰 정정). 받지 못할 문서(잘린 JSON·bench 결과 아닌 모양)를 그대로 넘기면 파서가
#     exit 2 로 끝나고, 레벨 1 에서는 스윕 전체가 exit 3 으로 죽는다 — inform-only lite 레그의 결손이
#     판정점 측정 불가로 격상되는 것이다. 무효면 사유를 truncation.log 에 적고 부재 명시로 강등한다:
#     spec 이 선언된 셀은 판정기에서 SPEC_ACCEPT_LEN_MISSING 으로 크게 드러나고, 스윕은 살아남는다.
_spec_axis_args() {   # $1=lite raw 경로(빈 값 = lite 절삭) · $2=truncation.log · $3=스크립트 디렉터리
  local raw="$1" trunclog="$2" sdir="$3" out="" rc=0
  SPEC_AXIS_ARGS=(--spec-axis-absent)
  # lite 절삭·raw 부재는 이미 truncation.log 에 적혀 있다(여기서 다시 적지 않는다).
  if [ -z "$raw" ] || [ ! -s "$raw" ]; then return 0; fi
  out="$(bash "$sdir/parse_guidellm.sh" spec-source --lite-raw "$raw" 2>&1)" || rc=$?
  if [ "$rc" = 0 ] && [ -n "$out" ]; then
    SPEC_AXIS_ARGS=(--accept-len-src "$out")
  else
    echo "[sweep_bench] ⚠ spec 축 승계원 무효 — 부재 명시로 강등(rc=$rc): ${out:-사유 없음}" | tee -a "$trunclog"
  fi
}
# <<< spec-axis-args

# spec 축 승계원은 lite 레그가 정한다 — lite 는 스윕당 1회이므로 판정도 **1회**다(run 마다 다시 물으면 같은 무효
#   사유가 레벨 × 반복만큼 절삭 로그에 쌓인다). 레벨 run 전부가 이 SPEC_AXIS_ARGS 를 함께 쓴다.
if [ "$TOOL" = "guidellm" ]; then
  _spec_axis_args "$LITE_RAW" "$TRUNCLOG" "$SDIR"
fi

COMPLETED=()
REPEAT_STOP=0
# 클램프(레벨 첫 run 실패)가 난 레벨 — 그 레벨은 index levels 에 실리지 않으므로 경계 사실(repeat_run.json)을
#   조립부에 따로 넘긴다(블랙박스 사살 대조 창 · classify_cell). 판정점(레벨1) 첫 run 실패는 exit 3 이라 해당 없음.
CLAMP_LEVEL=""
for L in "${SORTED[@]}"; do
  LDIR="$SWEEPDIR/level_$(printf '%02d' "$L")"; mkdir -p "$LDIR"
  # 이전 스윕의 반복 산출물을 걷어낸다 — 남아 있으면 이번 레벨의 runs[] 에 **다른 측정의 run** 이 섞인다.
  rm -rf "$LDIR"/run_[0-9][0-9]* "$LDIR/repeat_run.json"
  # 도구별 원시(bench_=vllm · guidellm_=GuideLLM)도 걷어낸다 — 도구를 바꿔 재스윕하면 옛 도구 원시가 레벨 자리에 남아
  #   소비자가 이번 측정으로 읽는다(2026-09-23 D1: 09-12 vllm bench JSON 이 GuideLLM 스윕의 첫 측정 시각으로 읽혔다).
  rm -f "$LDIR/bench_${CONFIG}.json" "$LDIR/guidellm_${CONFIG}.json"
  echo "[sweep_bench] ── level 동시성=$L (반복 $REPEATS · $REPEAT_KIND) ──"
  for ((K = 1; K <= REPEATS; K++)); do
    if [ "$K" = 1 ]; then RDIR="$LDIR"; else RDIR="$LDIR/run_$(printf '%02d' "$K")"; mkdir -p "$RDIR"; fi
    RUN_STARTED="$(date -u +%FT%TZ)"
    RB_ARGS=("$CONFIG" --topology "$TOPO" --serve-plane "$SERVE_PLANE" --concurrency "$L"
             --input-len "$ILEN" --output-len "$OLEN" --num-prompts "$NPROMPTS"
             --warmups "$WARMUPS" --backend "$BACKEND" --out-dir "$RDIR" --tool "$TOOL")
    [ -n "$HOST_ENDPOINT" ] && RB_ARGS+=(--host-endpoint "$HOST_ENDPOINT")
    [ -n "$CLIENT_VLLM" ] && RB_ARGS+=(--client-vllm "$CLIENT_VLLM")
    [ -n "$BENCH_BUDGET_MIB" ] && RB_ARGS+=(--bench-budget-mib "$BENCH_BUDGET_MIB")
    RB_ARGS+=(--dataset-spec "$DATASET_SPEC" --dataset-cache "$DATASET_CACHE" --dataset-run-index "$DATASET_RUN_INDEX" --dataset-sweep-salt "$DATASET_SWEEP_SALT")
    DATASET_RUN_INDEX=$((DATASET_RUN_INDEX + 1))
    RB_RC=0; PARSE_RC=0; RUN_OK=0; _PERR=""
    bash "$SDIR/run_bench.sh" "${RB_ARGS[@]}" || RB_RC=$?
    if [ "$RB_RC" = "0" ]; then
      ELOG="$RDIR/engine_${CONFIG}.log"
      # 파서는 도구가 정한다 — 스키마가 다르므로 파일명도 다르고, 잘못된 파서가 조용히 빈 값을
      # 내는 일이 없게 한다.
      if [ "$TOOL" = "guidellm" ]; then
        BJSON="$RDIR/guidellm_${CONFIG}.json"
        # spec 축은 GuideLLM 이 보고하지 않는다. **같은 스윕의 lite 레그**에서 승계한다(위 `_spec_axis_args` ·
        # 루프 앞에서 1회 판정 · 자체검사가 함수 구간을 바이트 그대로 실행).
        PARSE_CMD=(bash "$SDIR/parse_guidellm.sh" --benchmarks-json "$BJSON"
                   --engine-log "$ELOG" "${SPEC_AXIS_ARGS[@]}")
        [ -n "$MAX_ERROR_RATE" ] && PARSE_CMD+=(--max-error-rate "$MAX_ERROR_RATE")
        # 벤치 종료 시 서버 생존 관측(2026-09-07 · 유예 결함 ②). run_bench 가 벤치 직후 · teardown
        # 전에만 남길 수 있는 사실이며, 이것이 없으면 엔진 사망 중 잘린 SSE 가 도구 경계로 면제된다.
        _PH="$RDIR/post_health_${CONFIG}.json"
        [ -s "$_PH" ] && PARSE_CMD+=(--post-health-json "$_PH")
        _PC="$RDIR/prefix_cache_${CONFIG}.json"
        [ -s "$_PC" ] && PARSE_CMD+=(--prefix-cache-json "$_PC")
      else
        BJSON="$RDIR/bench_${CONFIG}.json"
        PARSE_CMD=(bash "$SDIR/parse_bench.sh" --bench-json "$BJSON" --engine-log "$ELOG")
      fi
      # 파서의 거부 사유를 버리지 않는다(2026-09-14 리뷰 정정) — 종전 `2>/dev/null` 은 파서가 exit 2 로
      #   이유를 말해도 절삭 로그에 "파싱 실패" 만 남겨, 레벨 1 중단(exit 3)의 원인을 아무도 읽을 수 없었다.
      _PERR="$(mktemp)"
      "${PARSE_CMD[@]}" > "$RDIR/measured.json" 2>"$_PERR" || PARSE_RC=$?
      if [ "$PARSE_RC" = "0" ] \
         && bash "$SDIR/bench_shell.sh" measurement-ok "$RDIR/measured.json"; then
        RUN_OK=1
      fi
    fi
    RUN_ENDED="$(date -u +%FT%TZ)"
    # run 경계 사실(시각·rc) — 반복 중단의 시각 대조 창이 여기서 온다(사인 확정은 classify_cell).
    bash "$SDIR/repeat_axis.sh" record-run --run-dir "$RDIR" --level "$L" --run "$K" \
      --started-utc "$RUN_STARTED" --ended-utc "$RUN_ENDED" --run-bench-rc "$RB_RC" --parse-rc "$PARSE_RC" >/dev/null \
      || echo "[sweep_bench] ⚠ level $L run $K 경계 사실 기록 실패 — 이 레벨의 runs[] 집계가 판정 불가로 남는다" | tee -a "$TRUNCLOG"
    if [ "$RUN_OK" = "1" ]; then
      [ -n "$_PERR" ] && rm -f "$_PERR"
      [ "$K" = 1 ] && COMPLETED+=("$L")
      echo "[sweep_bench] level $L run $K/$REPEATS ✓"
      continue
    fi
    if [ "$K" = 1 ]; then
      # 레벨 첫 run 실패 = 종전 적응 상한 클램프(레벨 절삭). 반복 축 판정 대상이 아니다.
      if [ "$RB_RC" != "0" ]; then
        echo "level $L truncated: run_bench exit $RB_RC (serve health-drop/게이트/bench 실패 — 상위 레벨 중단)" | tee -a "$TRUNCLOG"
        if [ "$L" = "1" ]; then
          echo "[sweep_bench] 판정점(레벨1) 측정 불가(run_bench exit $RB_RC) — serve 미가동/게이트 확인" >&2; exit 3
        fi
      else
        echo "[sweep_bench] level $L 측정 파싱 실패 → 절삭" | tee -a "$TRUNCLOG"
        if [ -s "$_PERR" ]; then tail -n 5 "$_PERR" | sed 's/^/  parse stderr: /' | tee -a "$TRUNCLOG"; fi
        echo "level $L truncated: parse/measurement_ok=false" >> "$TRUNCLOG"
        [ -n "$_PERR" ] && rm -f "$_PERR"
        [ "$L" = "1" ] && { echo "[sweep_bench] 판정점(레벨1) 측정 불가 — 중단" >&2; exit 3; }
      fi
      [ -n "$_PERR" ] && rm -f "$_PERR"
      CLAMP_LEVEL="$L"
      REPEAT_STOP=1   # 적응 클램프: 낮은 레벨 실패면 상위도 실패 → 중단
      break
    fi
    # run k≥2 실패 = 반복 중단 **즉시 신호**. 대표 run 은 이미 섰으므로 레벨은 측정 레벨로 남고,
    #   남은 반복·상위 레벨은 멈춘다(무너진 서빙 위에서 더 재면 사인이 원인에서 멀어진다).
    echo "[sweep_bench] ⚠ level $L run $K/$REPEATS 실패 → 반복 중단(즉시 신호 · bench_mode 확정은 classify_cell)" | tee -a "$TRUNCLOG"
    if [ -n "$_PERR" ] && [ -s "$_PERR" ]; then tail -n 5 "$_PERR" | sed 's/^/  parse stderr: /' | tee -a "$TRUNCLOG"; fi
    [ -n "$_PERR" ] && rm -f "$_PERR"
    echo "level $L truncated-above: run $K repeat-break(run_bench exit $RB_RC · parse rc $PARSE_RC · measurement_ok=false) — 남은 반복·상위 레벨 중단" >> "$TRUNCLOG"
    REPEAT_STOP=1
    break
  done
  case " ${COMPLETED[*]:-} " in
    *" $L "*)
      bash "$SDIR/repeat_axis.sh" aggregate-level --level-dir "$LDIR" --requested "$REPEATS" \
        --requested-source "$REPEATS_SOURCE" --kind "$REPEAT_KIND" \
        || echo "[sweep_bench] ⚠ level $L 반복 집계 실패(위 사유) — runs[] 없는 레벨로 남아 bench_mode 가 판정 불가가 된다" | tee -a "$TRUNCLOG"
      ;;
  esac
  [ "$REPEAT_STOP" = "1" ] && break
done

# ── 실제 측정 대상 이미지 캡처(2026-08-13 신설) ────────────────────────────────
# image_tag 는 여태 envfile 선언값만 적었다. 그런데 IMAGE_TAG 는 `docker compose` 호출 시
# 환경변수로 override 되는 것이 정상 경로이므로(변종·재빌드 트랙), **선언값과 실측이 갈린다**.
#   2026-08-13 X1 실측: envfile 은 `...-source`, 실제 컨테이너는 `...-source-canonical`.
#   그대로 두면 인증서가 **측정하지 않은 이미지**의 이름을 달고 발행되어 재현이 불가능해진다.
# 이 파일은 vLLM 버전에 대해 이미 같은 규율을 갖고 있다(선언값 대신 **실측을 강한키로 싣고**
# 선언값은 `*_declared` 로 나란히 남긴다). image_tag 에도 동형으로 채운다.
_CTR_RE='^(MASTER_)?CONTAINER_NAME='
[ "$TOPO" = "multi" ] && _CTR_RE='^MASTER_CONTAINER_NAME='
# tr 의 인자는 8진 이스케이프로 준다(\042=" \047=') — 셸 따옴표 중첩 회피.
_CTR_NAME="$( { grep -E "$_CTR_RE" "$EF" 2>/dev/null || true; } | tail -1 | cut -d= -f2- | tr -d '\042\047' )"
IMAGE_TAG_ACTUAL="NA"
# ★ 태그는 **가변 포인터**다 — 같은 문자열이 다른 이미지를 가리킬 수 있다(2026-09-04 실측:
#   메인 `easy-vllm:0.18.0-cu130-aarch64-wheel` = ca36fd12…, 서브 같은 태그 = c642de38…,
#   그 뒤 멀티 빌드가 메인 것을 또 덮었다). 인증서가 태그만 적으면 "어느 이미지로 쟀는가" 에
#   답하지 못한다. 컨테이너 이미지는 git 이 바이트를 들지 않고 추적 입력만으로 결정론 재생성도
#   보장되지 않으므로 **맹점층**이고(헌법 2문항 판정), 그 자리의 digest 는 중복이 아니라 유일한 증거다.
IMAGE_DIGEST_ACTUAL="NA"
if [ -n "$_CTR_NAME" ]; then
  IMAGE_TAG_ACTUAL="$(docker inspect "$_CTR_NAME" --format '{{.Config.Image}}' 2>/dev/null || echo NA)"
  [ -n "$IMAGE_TAG_ACTUAL" ] || IMAGE_TAG_ACTUAL="NA"
  # `.Image` = 컨테이너가 실제로 기동한 이미지의 content id(태그 재사용과 무관).
  IMAGE_DIGEST_ACTUAL="$(docker inspect "$_CTR_NAME" --format '{{.Image}}' 2>/dev/null || echo NA)"
  [ -n "$IMAGE_DIGEST_ACTUAL" ] || IMAGE_DIGEST_ACTUAL="NA"
fi

fi   # ── /REASSEMBLE 분기 끝(위 측정·캡처 전량은 재조립 모드에서 건너뛴다) ──

# ── sweep_index.json 조립 + meta 추출(Rust native · 측정 출처 보존) ──────────
CONFIG="$CONFIG" TOPO="$TOPO" CFGYAML="$CFGYAML" EF="$EF" MANIFEST="$MANIFEST" \
SWEEPDIR="$SWEEPDIR" VLLM_VER="$VLLM_VER" COMPLETED="${COMPLETED[*]:-}" ILEN="$ILEN" KNOB_RECORD="$KNOB_RECORD" \
 REPEATS="$REPEATS" REPEATS_SOURCE="$REPEATS_SOURCE" REPEAT_KIND="$REPEAT_KIND" CLAMP_LEVEL="${CLAMP_LEVEL:-}" \
 IMAGE_TAG_ACTUAL="$IMAGE_TAG_ACTUAL" IMAGE_DIGEST_ACTUAL="$IMAGE_DIGEST_ACTUAL" REASSEMBLE="$REASSEMBLE" \
 LITE_RAW="$LITE_RAW" SDIR="$SDIR" SERVE_PLANE="$SERVE_PLANE" bash "$SDIR/assemble_sweep_meta.sh"

[ -n "$KNOB_RECORD" ] && rm -f "$KNOB_RECORD"   # 기록은 sweep_index.bench_knobs 로 옮겨졌다(두 번째 자리를 남기지 않는다)
[ -n "${DATASET_SPEC:-}" ] && rm -f "$DATASET_SPEC"   # 해소값은 bench_knobs.dataset 에 이미 있다(같은 이유)

# ── bench_mode 판정 기록(2026-09-14 · plan_26091407 §4.4 · 리뷰 정정) ─────────────────────────────────
#   판정 소유자는 classify_cell.py 다 — 여기서는 **부르기만** 한다(규칙 복제 ✗). 정규 경로(sweep_bench →
#   judge_bench → render_report · publish)에 호출자가 없으면 기록이 생기지 않아 리포트는 영원히 "미확정" 이고
#   인증서·단계 ⑤ 가 읽을 자리가 없다. 재조립도 부른다(측정시각 승계라 기록이 같은 측정에 묶인다).
#   실패는 스윕을 죽이지 않되 침묵하지 않는다 — 기록이 없으면 리포트는 '미확정', full 인증서는 미발행이다.
#   ★ 2026-09-14(⑧ 분석 발견 T1): 대조 대상 노드는 분류기가 이 스윕의 meta(topology · measured_node)와 manifest 로 정한다
#     (single = 측정 노드 · multi = 서빙 참여 노드 전부 · 원격 노드는 fetch_sub_docs 회수 미러). multi 에서 서브 기록이
#     아직 회수되지 않았으면 대조는 not_scanned 로 **기재**된다 — 회수 뒤 `--reassemble-only` 로 이 기록을 다시 쓴다.
_BM_ARGS=(--sweep-index "$SWEEPDIR/sweep_index.json" --events-from-repo "$REPO" --manifest "$MANIFEST" --write-bench-mode)
[ -n "$XNODE_TOL" ] && _BM_ARGS+=(--cross-node-tolerance-s "$XNODE_TOL" --cross-node-tolerance-source "$XNODE_TOL_SRC")
if _BM_OUT="$(bash "$SDIR/classify_cell.sh" "${_BM_ARGS[@]}")"; then
  printf '%s' "$_BM_OUT" | bash "$SDIR/bench_shell.sh" bench-mode-log || true
else
  echo "[sweep_bench] ⚠ bench_mode 판정 기록 실패(위 사유) — 리포트는 '미확정', full 인증서는 발행되지 않는다" >&2
fi

[ "$REASSEMBLE" = "1" ] || rm -f "$SWEEP_INCOMPLETE"   # 정상 종료만 표식을 지운다(위 ★ 미완 표식)
echo "[sweep_bench] DONE — SWEEP_INDEX=$SWEEPDIR/sweep_index.json"
