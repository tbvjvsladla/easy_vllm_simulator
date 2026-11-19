#!/bin/bash
# `verdict_clock_scan check [...]` / `--self-test` through the node-built native executable (crate .claude/rust/py-boundary-guard).
# 하네스 판정은 벽시계를 읽지 않는다(CLAUDE.md 불변식 · h20): 추적 .rs·셸·.ts 의 시계 읽기 ⊆ verdict_clock_allowlist.tsv.
# Direct normal ELF startup; no install/build/download: a missing executable is exit 2 with guidance.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${VERDICT_CLOCK_SCAN_BIN:-$SDIR/../../rust/py-boundary-guard/target/release/verdict_clock_scan}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[verdict_clock_scan] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[verdict_clock_scan] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[verdict_clock_scan] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec "$BIN" "$@"
