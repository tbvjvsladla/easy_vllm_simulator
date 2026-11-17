#!/bin/bash
# `harness_verify.py [--as-of YYYY-MM-DD] [--registry ...]` (the three-stage completion gate) through the node-built Rust
# executable (package .claude/rust/policy-audit). Its stages run the Rust launchers policy_registry.sh, runtime_selftest.sh
# and predicates/*.sh. Direct normal ELF startup: no install, build, download or Python fallback; a missing executable is
# exit 2 with the build command.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BINARY="${HARNESS_VERIFY_BIN:-$SDIR/../../rust/policy-audit/target/release/harness_verify}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[harness_verify] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[harness_verify] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BINARY" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[harness_verify] ERROR normal Rust ELF invalid: %s\n' "$BINARY" >&2
  exit 2
fi
exec -a "$SDIR/harness_verify.sh" "$BINARY" "$@"
