#!/bin/bash
# `policy_registry.py verify|check-removal|--self-test` through the node-built Rust executable (package
# .claude/rust/policy-audit). Direct normal ELF startup: no install, build, download or Python fallback; a missing
# executable is exit 2 with the build command.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BINARY="${POLICY_REGISTRY_BIN:-$SDIR/../../rust/policy-audit/target/release/policy_registry}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[policy_registry] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[policy_registry] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BINARY" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[policy_registry] ERROR normal Rust ELF invalid: %s\n' "$BINARY" >&2
  exit 2
fi
exec -a "$SDIR/policy_registry.sh" "$BINARY" "$@"
