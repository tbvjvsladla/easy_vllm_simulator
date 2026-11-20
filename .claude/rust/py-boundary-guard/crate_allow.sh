#!/bin/bash
# `crate_allow check [...]` through the node-built native executable (crate .claude/rust/py-boundary-guard).
# plan 26111805 D2·D9: every external package of every tracked Cargo.lock is approved in crate_allowlist.tsv (same version),
# under a permitted license that equals the downloaded manifest's, and named in notices/NOTICE-crates.md. Last line on a pass:
# DEPS-ALLOW-PASS. Direct normal ELF startup; no install/build/download: a missing executable is exit 2 with guidance.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${CRATE_ALLOW_BIN:-$SDIR/target/release/crate_allow}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[crate_allow] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[crate_allow] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[crate_allow] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec "$BIN" "$@"
