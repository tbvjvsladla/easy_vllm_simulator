#!/bin/bash
# The `selftest_claude_native_events` suite through its node-local Rust executable (`terraforming_node/rust`, binary `selftest_claude_native_events`; the
# port of `selftest_claude_native_events.py`, plan_26111615). Offline fixtures only. Installation and build are explicit operations
# (`build_rust_packages.sh`); a missing or non-executable binary is an explicit error (status 2 + build guidance). There is no
# Python route.
set -euo pipefail
# Self-contained: builtins only (no `dirname` is looked up in the caller's PATH).
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${TERRAFORMING_BIN_DIR:-$SDIR/../rust/target/release}/selftest_claude_native_events"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[selftest_claude_native_events] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[selftest_claude_native_events] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[selftest_claude_native_events] ERROR Rust 실행파일이 ELF 가 아니다: %s\n' "$BIN" >&2
  exit 2
fi
# The scripts directory under test (the retired program's location, never user intent; the file itself need not exist).
export TERRAFORMING_OWNER_PATH="$SDIR/selftest_claude_native_events.py"
exec "$BIN" "$@"
