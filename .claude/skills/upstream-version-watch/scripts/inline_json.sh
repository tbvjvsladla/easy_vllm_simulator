#!/bin/bash
# Direct normal ELF startup; no install/build/download or Python business fallback.
# Ported: the inline python3 -c / heredoc JSON readers of sync_branches.sh, sync_to_sub.sh, multinode_serve_smoke.sh and single_serve_down.sh (subcommands: allowed, gate-unreadable-*, divergence-summary, parity-summary, delivery-source, propagation-fields, chat-verdict, completions-len, campaign-budget, open-sessions).
set -euo pipefail
SCRIPT_PATH=${BASH_SOURCE[0]}
case "$SCRIPT_PATH" in */*) SCRIPT_DIR=${SCRIPT_PATH%/*}; [ -n "$SCRIPT_DIR" ] || SCRIPT_DIR=/ ;; *) SCRIPT_DIR=. ;; esac
SDIR="$(cd "$SCRIPT_DIR" && pwd -P)"
if [ "${EASY_VLLM_UPSTREAM_RUNTIME_ROOT+set}" = set ]; then
  runtime_root="$EASY_VLLM_UPSTREAM_RUNTIME_ROOT"
  if [ -z "$runtime_root" ]; then
    printf '[inline_json] ERROR explicit upstream runtime root is empty\n' >&2
    exit 2
  fi
else
  runtime_root="$SDIR/../rust/runtime"
fi
binary="$runtime_root/target/release/inline_json"
if [ ! -f "$binary" ] || [ ! -x "$binary" ]; then
  printf '[inline_json] ERROR normal Rust ELF missing/non-executable: %s\n' "$binary" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$binary" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[inline_json] ERROR normal Rust ELF invalid: %s\n' "$binary" >&2
  exit 2
fi
exec "$binary" "$@"
