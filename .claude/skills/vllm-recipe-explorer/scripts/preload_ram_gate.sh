#!/bin/bash
# preload_ram_gate (Rust). Direct normal ELF startup; no install/build/download or Python business fallback.
set -euo pipefail
SCRIPT_PATH=${BASH_SOURCE[0]}
case "$SCRIPT_PATH" in */*) SCRIPT_DIR=${SCRIPT_PATH%/*}; [ -n "$SCRIPT_DIR" ] || SCRIPT_DIR=/ ;; *) SCRIPT_DIR=. ;; esac
SDIR="$(cd "$SCRIPT_DIR" && pwd -P)"
if [ "${EASY_VLLM_RECIPE_RUNTIME_ROOT+set}" = set ]; then
  runtime_root="$EASY_VLLM_RECIPE_RUNTIME_ROOT"
  if [ -z "$runtime_root" ]; then
    printf '[preload_ram_gate] ERROR explicit upstream runtime root is empty\n' >&2
    exit 2
  fi
else
  runtime_root="$SDIR/../rust/runtime"
fi
binary="$runtime_root/target/release/preload_ram_gate"
if [ ! -f "$binary" ] || [ ! -x "$binary" ]; then
  printf '[preload_ram_gate] ERROR normal Rust ELF missing/non-executable: %s\n' "$binary" >&2
  printf '[preload_ram_gate] build first: bash .claude/skills/terraforming_node/scripts/build_rust_packages.sh\n' >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$binary" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[preload_ram_gate] ERROR normal Rust ELF invalid: %s\n' "$binary" >&2
  exit 2
fi
# argv[0] names the skill that owns this launcher (<skill>/rust/runtime/target/release/preload_ram_gate): a binary that derives its skill root
# (feedback dir, repo root) from it acts on THIS skill's tree even under a runtime-root override.
exec -a "${SDIR%/*}/rust/runtime/target/release/preload_ram_gate" "$binary" "$@"
