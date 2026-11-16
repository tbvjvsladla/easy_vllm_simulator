#!/bin/bash
# Explicit node-local application build; normal evaluation never installs or builds.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
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
# B reads the node-role contract through the local path dependencies `role-contract` and the common package under it: both must
# sit beside this skill before any build or self-check starts.
# The lite/sweep self-checks ask the hint reader as the hint-publisher Rust library (plan_26111615 pass 3): that crate is a path
# dependency too.
for dependency in rust/runtime-support rust/role-contract skills/hint-publisher/rust; do
  if [ ! -f "$REPO/.claude/$dependency/Cargo.toml" ]; then
    printf '[build_rust] ERROR 경로 의존성 부재: %s — adversarial-benchmark 는 role-contract·runtime-support·hint-publisher crate 를 함께 필요로 한다\n' "$REPO/.claude/$dependency" >&2
    exit 2
  fi
done
# production build: an inherited EASY_VLLM_SOURCE_ROOT (a prototype override that build.rs reads the campaign schema from) must not pull a foreign tree into
# this node's build; the Q/T/P/U helper clears it the same way. Prototype builds select it by running cargo explicitly.
env -u EASY_VLLM_SOURCE_ROOT bash "$BOOTSTRAP" exec "${runtime_args[@]}" -- cargo build --release --locked \
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
  bash "$SDIR/repeat_axis.sh" --self-test
BENCH_PRESET_BIN="$CRATE/target/release/bench_preset" \
  BENCH_PRESET_API_BIN="$CRATE/target/release/bench_preset_api" \
  BENCH_PRESET_REPO_ROOT="$REPO" \
  BENCH_PRESET_SKILL_DIR="$SDIR/.." \
  BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_bench_preset.sh"
LITE_METRICS_API_BIN="$CRATE/target/release/lite_metrics_api" \
  LITE_METRICS_FIXTURE="$SDIR/../fixtures/lite_raw_d1_connection_refused.json" \
  bash "$SDIR/lite_metrics_api.sh" self-test
CLASSIFY_CELL_BIN="$CRATE/target/release/classify_cell" \
  CLASSIFY_CELL_API_BIN="$CRATE/target/release/classify_cell_api" \
  CLASSIFY_CELL_SCRIPTS_DIR="$SDIR" \
  bash "$SDIR/classify_cell.sh" --self-test
RENDER_SWEEP_MAP_BIN="$CRATE/target/release/render_sweep_map" \
  bash "$SDIR/render_sweep_map.sh" --self-test
BENCH_DOC_NAMING_BIN="$CRATE/target/release/bench_doc_naming" \
  bash "$SDIR/bench_doc_naming.sh"
DATASET_PREP_BIN="$CRATE/target/release/dataset_prep" \
  bash "$SDIR/dataset_prep.sh" --self-test
RESOLVE_BENCH_TOOL_BIN="$CRATE/target/release/resolve_bench_tool" \
  bash "$SDIR/resolve_bench_tool.sh" --self-test
RESOLVE_GUIDELLM_BIN="$CRATE/target/release/resolve_guidellm" \
  GUIDELLM_RECORD_PATH="$SDIR/../bench_tool_pin.json" \
  bash "$SDIR/resolve_guidellm.sh" --self-test
RESOLVE_TOPOLOGY_BIN="$CRATE/target/release/resolve_topology"   RESOLVE_TOPOLOGY_API_BIN="$CRATE/target/release/resolve_topology_api"   RESOLVE_TOPOLOGY_REPO_ROOT="$REPO"   RESOLVE_TOPOLOGY_SCRIPTS_DIR="$SDIR"   RESOLVE_TOPOLOGY_RUNTIME_DIR="$CRATE/target/release"   bash "$SDIR/resolve_topology.sh" --self-test

CONFORMANCE_BIN="$CRATE/target/release/conformance_full_superset_lite" \
  CONFORMANCE_SCRIPTS_DIR="$SDIR" \
  bash "$SDIR/conformance_full_superset_lite.sh"

SELFTEST_BROAD_CELL_BIN="$CRATE/target/release/selftest_broad_search_cell_args" BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_broad_search_cell_args.sh"

SELFTEST_BROAD_PRECHECK_BIN="$CRATE/target/release/selftest_broad_search_precheck" BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_broad_search_precheck.sh"

SELFTEST_JUDGE_BENCH_BIN="$CRATE/target/release/selftest_judge_bench" BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_judge_bench.sh"

SELFTEST_LITE_REPORT_BIN="$CRATE/target/release/selftest_lite_report" BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_lite_report.sh"

SELFTEST_LITE_GATE_BIN="$CRATE/target/release/selftest_lite_gate" BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_lite_gate.sh"

SELFTEST_SWEEP_META_BIN="$CRATE/target/release/selftest_sweep_meta" BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_sweep_meta.sh"

SELFTEST_SWEEP_REPEATS_BIN="$CRATE/target/release/selftest_sweep_repeats" BENCH_VERIFY_REPO_ROOT="$REPO" bash "$SDIR/selftest_sweep_repeats.sh"

ASSEMBLE_SWEEP_META_BIN="$CRATE/target/release/assemble_sweep_meta" bash "$SDIR/assemble_sweep_meta.sh" --self-test

BENCH_SHELL_BIN="$CRATE/target/release/bench_shell" bash "$SDIR/bench_shell.sh" --self-test

# scan_node P1 (the private bridge of the Python adapter): the native self-check of the 16 P1 names against the embedded saved-raw rows.
# It checks the Rust P1 only; the legacy `scan_node_original.py --self-test` runs the original Python and is no evidence for it.
SCAN_NODE_BIN="$CRATE/target/release/scan_node" bash "$REPO/.claude/skills/terraforming_node/scripts/scan_node.sh" --p1-self-check

# R48 O2: minimal NATIVE startup probes of the manifest_contract / turn_budget bridges (scan_node's own check is the line above). Which probes ran is printed;
# this build does NOT start all 57 native bins (see native_health_probes.sh header for the exact set).
/bin/bash "$SDIR/native_health_probes.sh" "$CRATE"
