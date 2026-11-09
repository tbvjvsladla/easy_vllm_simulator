#!/bin/bash
# `scan_node.py` P1 bridge through the node-local Rust executable (B build, `scan_node`). Installation and build are explicit
# operations; a missing or non-executable file is an explicit error (status 2 + build guidance) before anything else happens. There
# is no Python route for P1 and no silent start of another program. The scanner command line, probes and network collection are
# other phases: they keep their own explicit compat route in scan_node.py, never through this launcher.
set -euo pipefail
# Logical directory (no symbolic link is resolved): the owner identity is the lexical path of this script's Python sibling.
# Self-contained: builtins only (no `dirname` is looked up in the caller's business PATH, which is never modified).
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${SCAN_NODE_BIN:-$SDIR/../../adversarial-benchmark/rust/target/release/scan_node}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[scan_node] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[scan_node] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../adversarial-benchmark/scripts/build_rust.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[scan_node] ERROR Rust 실행파일이 ELF 가 아니다: %s\n' "$BIN" >&2
  exit 2
fi
# `SCAN_NODE_VALIDATE_ONLY=1` is an availability check only (same ELF and executable-bit check, status 0, no business) - for the public
# command line. A bridge start (`SCAN_NODE_BRIDGE` set by the adapter) is never an availability probe: a caller's exported
# VALIDATE_ONLY must not turn a bridge call into an empty answer, so the bridge always reaches the Rust program.
if [ "${SCAN_NODE_VALIDATE_ONLY:-}" = 1 ] && [ -z "${SCAN_NODE_BRIDGE:-}" ]; then
  exit 0
fi
unset SCAN_NODE_VALIDATE_ONLY
# `__file__` of the program this one stands for: exported by the Python adapter when it started this launcher, otherwise the sibling
# `scan_node.py` of this script (its location, never user intent).
export SCAN_NODE_OWNER_PATH="${SCAN_NODE_OWNER_PATH:-$SDIR/scan_node.py}"
exec "$BIN" "$@"
