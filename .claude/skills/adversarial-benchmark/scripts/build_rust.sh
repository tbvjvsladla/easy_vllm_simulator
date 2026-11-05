#!/bin/bash
# Explicit node-local application build; normal evaluation never installs or builds.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
CRATE="$SDIR/../rust"
BOOTSTRAP="$REPO/.claude/skills/terraforming_node/scripts/rust_bootstrap.sh"
runtime_args=()
cargo_args=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --offline) cargo_args+=(--offline); shift ;;
    --runtime-home)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        echo '[build_rust] ERROR --runtime-home 에 경로가 필요하다' >&2; exit 2
      fi
      runtime_args+=(--runtime-home "$2"); shift 2 ;;
    --help|-h)
      echo '사용법: bash build_rust.sh [--offline] [--runtime-home DIR]'
      echo 'Terraform install/check 뒤 Cargo.lock 으로 현재 노드용 release 실행파일을 빌드한다.'
      exit 0 ;;
    *) printf '[build_rust] ERROR 알 수 없는 인자: %s\n' "$1" >&2; exit 2 ;;
  esac
done
bash "$BOOTSTRAP" exec "${runtime_args[@]}" -- cargo build --release --locked \
  --manifest-path "$CRATE/Cargo.toml" --target-dir "$CRATE/target" "${cargo_args[@]}"
SWEEP_STOP_BIN="$CRATE/target/release/sweep_stop" bash "$SDIR/sweep_stop.sh" --self-test
PARSE_BENCH_BIN="$CRATE/target/release/parse_bench" bash "$SDIR/parse_bench.sh" self-test
ROOFLINE_BIN="$CRATE/target/release/roofline" bash "$SDIR/roofline.sh" self-test
VERDICT_RULE_BIN="$CRATE/target/release/verdict_rule" bash "$SDIR/verdict_rule.sh" --self-test
PARSE_GUIDELLM_BIN="$CRATE/target/release/parse_guidellm" \
  PARSE_GUIDELLM_FIXTURE="$SDIR/../fixtures/guidellm_benchmarks_sample.json" \
  bash "$SDIR/parse_guidellm.sh" --self-test
REPEAT_AXIS_BIN="$CRATE/target/release/repeat_axis" \
  REPEAT_AXIS_API_BIN="$CRATE/target/release/repeat_axis_api" \
  REPEAT_AXIS_REPO_ROOT="$REPO" \
  REPEAT_AXIS_CAMPAIGN_ADAPTER="$SDIR/repeat_axis_owner_query.py" \
  bash "$SDIR/repeat_axis.sh" --self-test
BENCH_PRESET_BIN="$CRATE/target/release/bench_preset" \
  BENCH_PRESET_API_BIN="$CRATE/target/release/bench_preset_api" \
  BENCH_PRESET_REPO_ROOT="$REPO" \
  BENCH_PRESET_SKILL_DIR="$SDIR/.." \
  REPEAT_AXIS_CAMPAIGN_ADAPTER="$SDIR/repeat_axis_owner_query.py" \
  bash "$SDIR/bench_preset.sh" --self-test
LITE_METRICS_API_BIN="$CRATE/target/release/lite_metrics_api" \
  LITE_METRICS_FIXTURE="$SDIR/../fixtures/lite_raw_d1_connection_refused.json" \
  bash "$SDIR/lite_metrics_api.sh" self-test
CLASSIFY_CELL_BIN="$CRATE/target/release/classify_cell" \
  CLASSIFY_CELL_API_BIN="$CRATE/target/release/classify_cell_api" \
  CLASSIFY_CELL_SCRIPTS_DIR="$SDIR" \
  CLASSIFY_CELL_ROLE_ADAPTER="$SDIR/classify_cell_role_query.py" \
  bash "$SDIR/classify_cell.sh" --self-test
RENDER_SWEEP_MAP_BIN="$CRATE/target/release/render_sweep_map" \
  bash "$SDIR/render_sweep_map.sh" --self-test
BENCH_DOC_NAMING_BIN="$CRATE/target/release/bench_doc_naming" \
  bash "$SDIR/bench_doc_naming.sh"
DATASET_PREP_BIN="$CRATE/target/release/dataset_prep" \
  bash "$SDIR/dataset_prep.sh" --self-test
RESOLVE_BENCH_TOOL_BIN="$CRATE/target/release/resolve_bench_tool" \
  bash "$SDIR/resolve_bench_tool.sh" --self-test
