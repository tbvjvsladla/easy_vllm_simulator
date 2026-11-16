#!/bin/bash
# `doc_naming.py [--self-test]` through the node-local Rust executable (crate .claude/rust/policy-gate). The naming
# owners stay where they are (wiki-desk doc_naming.ts via node · adversarial-benchmark bench naming via its Rust
# launcher); a missing executable is exit 2 with guidance, never a Python start.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${DOC_NAMING_BIN:-$SDIR/../../rust/policy-gate/target/release/doc_naming}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[doc_naming] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[doc_naming] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$(cd "$SDIR/../../.." && pwd -P)/.claude/skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[doc_naming] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec -a "$SDIR/doc_naming.py" "$BIN" "$@"
