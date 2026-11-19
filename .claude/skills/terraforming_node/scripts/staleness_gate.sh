#!/bin/bash
# `staleness_gate.py` through the node-local Rust executable (`terraforming_node/rust`, binary `staleness_gate`). The original's
# command line, exit codes, text and files without its attestation age axis (`--now`, `--max-age-days`: removed, h21). Installation and build are explicit operations (`build_rust_packages.sh`); a missing
# or non-executable binary is an explicit error (status 2 + build guidance) before anything else happens. There is no Python
# route and no silent start of another program.
set -euo pipefail
# Self-contained: builtins only (no `dirname` is looked up in the caller's business PATH, which is never modified).
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${TERRAFORMING_BIN_DIR:-$SDIR/../rust/target/release}/staleness_gate"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[staleness_gate] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[staleness_gate] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[staleness_gate] ERROR Rust 실행파일이 ELF 가 아니다: %s\n' "$BIN" >&2
  exit 2
fi
# `__file__` of the program this one stands for (its location, never user intent): the sibling `staleness_gate.py` path.
export TERRAFORMING_OWNER_PATH="$SDIR/staleness_gate.sh"
exec "$BIN" "$@"
