#!/bin/bash
# Golden capture for checks/data/cli/*: runs every case with the ORIGINAL Python program (re-capture needs the original
# policy_registry.py / harness_verify.py) and stores stdout, stderr and exit_code next to the case.
#   capture_goldens.sh [CASE_ID...]
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
cd "$SDIR/data/cli"
ids=("$@")
[ "${#ids[@]}" -gt 0 ] || ids=(*/)
for id in "${ids[@]}"; do
  id="${id%/}"
  bash "$SDIR/run_case.sh" "$SDIR/data/cli/$id" "$SDIR/data/cli/$id" orig
  printf '%s exit=%s\n' "$id" "$(cat "$SDIR/data/cli/$id/exit_code")"
done
