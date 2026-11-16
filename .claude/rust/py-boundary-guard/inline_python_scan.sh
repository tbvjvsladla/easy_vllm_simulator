#!/bin/bash
# `inline_python_scan check [...]` through the node-built native executable (crate .claude/rust/py-boundary-guard).
# plan_26111615 pass 3: 추적 비-.py 실행 표면이 호스트·서브에서 Python 을 띄우면 실패한다(허용 = 컨테이너 내부 · 골든 재캡처, 사유와 함께
# inline_python_allowlist.tsv). Direct normal ELF startup; no install/build/download: a missing executable is exit 2 with guidance.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${INLINE_PYTHON_SCAN_BIN:-$SDIR/target/release/inline_python_scan}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[inline_python_scan] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[inline_python_scan] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[inline_python_scan] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec "$BIN" "$@"
