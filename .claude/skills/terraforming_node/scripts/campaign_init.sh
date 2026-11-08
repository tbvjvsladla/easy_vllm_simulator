#!/bin/bash
# Read-only `campaign_init.py --active` / `--derive KIND ...` through the node-local Rust executable. Installation and build
# are explicit operations; a missing executable is an explicit error, never a silent Python start. Every other operation of
# campaign_init (the writers) keeps calling campaign_init.py directly.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${CAMPAIGN_READ_BIN:-$SDIR/../../adversarial-benchmark/rust/target/release/campaign_read}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[campaign_init] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[campaign_init] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../adversarial-benchmark/scripts/build_rust.sh" >&2
  exit 2
fi
# `CAMPAIGN_READ_VALIDATE_ONLY=1` is an availability check for callers that must stop before their first side effect: it
# returns 0 here, after the same ELF and executable-bit check, without any native or Python business. It proves the
# runtime is present, not that it is healthy; every other value (and every argument shape) routes exactly as before.
if [ "${CAMPAIGN_READ_VALIDATE_ONLY:-}" = 1 ]; then
  exit 0
fi
unset CAMPAIGN_READ_VALIDATE_ONLY
# The owner program is the campaign_init.py beside this launcher (its location, not user intent).
export CAMPAIGN_READ_COMPAT_OWNER="${CAMPAIGN_READ_COMPAT_OWNER:-$SDIR/campaign_init.py}"
# A caller-selected interpreter is explicit legacy intent; empty keeps the default meaning.
if [[ -n "${CAMPAIGN_READ_PYTHON:-}" ]]; then
  export CAMPAIGN_READ_PYTHON
else
  unset CAMPAIGN_READ_PYTHON
fi
exec "$BIN" "$@"
