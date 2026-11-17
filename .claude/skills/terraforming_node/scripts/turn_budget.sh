#!/bin/bash
# `turn_budget.py` command line through the node-local Rust executable (B build, `turn_budget`). Installation and build are
# explicit operations; a missing or non-executable file is an explicit error (status 2 + build guidance) before anything else
# happens - there is no Python route and no silent start of another program.
set -euo pipefail
# Logical directory (no symbolic link is resolved): the owner identity is the lexical path of this script's Python sibling.
# Self-contained: builtins only (no `dirname` is looked up in the caller's business PATH, which is never modified).
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${TURN_BUDGET_BIN:-$SDIR/../../adversarial-benchmark/rust/target/release/turn_budget}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[turn_budget] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[turn_budget] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../adversarial-benchmark/scripts/build_rust.sh" >&2
  exit 2
fi
# `TURN_BUDGET_VALIDATE_ONLY=1` is an availability check only (same ELF and executable-bit check, status 0, no business).
if [ "${TURN_BUDGET_VALIDATE_ONLY:-}" = 1 ]; then
  exit 0
fi
unset TURN_BUDGET_VALIDATE_ONLY
# `__file__` of the program this one stands for: exported by the Python adapter when it started this launcher, otherwise the
# sibling `turn_budget.py` of this script (its location, never user intent). The default schema is derived from it lexically.
export TURN_BUDGET_OWNER_PATH="${TURN_BUDGET_OWNER_PATH:-$SDIR/turn_budget.sh}"
exec "$BIN" "$@"
