#!/bin/bash
# `campaign_init` (the retired `campaign_init.py` command line) through the node-local Rust executable: every argument shape —
# the read shapes (`--active`, `--derive KIND ...`), every writer and lifecycle operation, `--selftest`, `-h` and argparse's usage
# errors — is answered natively. No shape starts Python. Installation and build are explicit operations; a missing executable is
# an explicit error.
set -euo pipefail
# Self-contained: builtins only (no `dirname` is looked up in the caller's business PATH, which is never modified).
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${CAMPAIGN_READ_BIN:-$SDIR/../../adversarial-benchmark/rust/target/release/campaign_init}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[campaign_init] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[campaign_init] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../adversarial-benchmark/scripts/build_rust.sh" >&2
  exit 2
fi
# `CAMPAIGN_READ_VALIDATE_ONLY=1` is an availability check for callers that must stop before their first side effect: it
# returns 0 here, after the same file and executable-bit check, without any business. It proves the runtime is present, not
# that it is healthy; every other value (and every argument shape) routes exactly as before.
if [ "${CAMPAIGN_READ_VALIDATE_ONLY:-}" = 1 ]; then
  exit 0
fi
unset CAMPAIGN_READ_VALIDATE_ONLY
# The program path the checkout root is derived from (`parents[4]` of the retired campaign_init.py beside this launcher — its
# location, not user intent; the file itself need not exist).
export CAMPAIGN_READ_COMPAT_OWNER="${CAMPAIGN_READ_COMPAT_OWNER:-$SDIR/campaign_init.sh}"
# No interpreter variable is honoured any more (there is no Python route).
unset CAMPAIGN_READ_PYTHON CAMPAIGN_READ_COMPAT_PYTHON
exec "$BIN" "$@"
