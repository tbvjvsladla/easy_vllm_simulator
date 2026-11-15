#!/bin/bash
# run_trial.py: --self-test, --dry-run/--mock-profile and the real docker session (Rust; the session stops with status 2 before docker when an active campaign needs the grounding backstop or the budget declaration is not --no-budget — both owners are still Python). Direct normal ELF startup; no install/build/download or Python business fallback.
set -euo pipefail
SCRIPT_PATH=${BASH_SOURCE[0]}
case "$SCRIPT_PATH" in */*) SCRIPT_DIR=${SCRIPT_PATH%/*}; [ -n "$SCRIPT_DIR" ] || SCRIPT_DIR=/ ;; *) SCRIPT_DIR=. ;; esac
SDIR="$(cd "$SCRIPT_DIR" && pwd -P)"
if [ "${EASY_VLLM_RECIPE_RUNTIME_ROOT+set}" = set ]; then
  runtime_root="$EASY_VLLM_RECIPE_RUNTIME_ROOT"
  if [ -z "$runtime_root" ]; then
    printf '[run_trial] ERROR explicit upstream runtime root is empty\n' >&2
    exit 2
  fi
else
  runtime_root="$SDIR/../rust/runtime"
fi
binary="$runtime_root/target/release/run_trial"
if [ ! -f "$binary" ] || [ ! -x "$binary" ]; then
  printf '[run_trial] ERROR normal Rust ELF missing/non-executable: %s\n' "$binary" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$binary" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[run_trial] ERROR normal Rust ELF invalid: %s\n' "$binary" >&2
  exit 2
fi
exec -a "$SDIR/run_trial.py" "$binary" "$@"
