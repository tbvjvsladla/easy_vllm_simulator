#!/bin/bash
# `audit_session_ssh.py` through the node-local Rust executable (`terraforming_node/rust`, binary `audit_session_ssh`). Same command line, exit
# codes, text and files as the original. Installation and build are explicit operations (`build_rust_packages.sh`); a missing
# or non-executable binary is an explicit error (status 2 + build guidance) before anything else happens. There is no Python
# route and no silent start of another program.
set -euo pipefail
# Self-contained: builtins only (no `dirname` is looked up in the caller's business PATH, which is never modified).
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${TERRAFORMING_BIN_DIR:-$SDIR/../rust/target/release}/audit_session_ssh"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[audit_session_ssh] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[audit_session_ssh] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[audit_session_ssh] ERROR Rust 실행파일이 ELF 가 아니다: %s\n' "$BIN" >&2
  exit 2
fi
# `__file__` of the program this one stands for (its location, never user intent): the sibling `audit_session_ssh.py` path.
export TERRAFORMING_OWNER_PATH="$SDIR/audit_session_ssh.sh"
exec "$BIN" "$@"
