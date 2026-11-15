#!/bin/bash
# `campaign_template_validator.py` through the node-local Rust executable: `--acceptance INSTANCE` (the acceptance predicates
# P4-P6) is answered natively; every other mode (`--campaign`, `--instance`, `--template`, `--selftest`) reaches the owner
# `campaign_template_validator.py` through the explicit compatibility route. Installation and build are explicit operations;
# a missing executable is an explicit error, never a silent Python start.
set -euo pipefail
# Self-contained: builtins only (no `dirname` is looked up in the caller's business PATH, which is never modified).
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${CAMPAIGN_VALIDATOR_BIN:-$SDIR/../../adversarial-benchmark/rust/target/release/campaign_template_validator}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[campaign_template_validator] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[campaign_template_validator] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../adversarial-benchmark/scripts/build_rust.sh" >&2
  exit 2
fi
# `CAMPAIGN_VALIDATOR_VALIDATE_ONLY=1` is an availability check for callers that must stop before their first side effect: it
# returns 0 after the same ELF and executable-bit check, without any native or Python business.
if [ "${CAMPAIGN_VALIDATOR_VALIDATE_ONLY:-}" = 1 ]; then
  exit 0
fi
unset CAMPAIGN_VALIDATOR_VALIDATE_ONLY
# The owner program is the campaign_template_validator.py beside this launcher (its location, not user intent).
export CAMPAIGN_VALIDATOR_COMPAT_OWNER="${CAMPAIGN_VALIDATOR_COMPAT_OWNER:-$SDIR/campaign_template_validator.py}"
# A caller-selected interpreter is explicit legacy intent; empty keeps the default meaning.
if [[ -n "${CAMPAIGN_VALIDATOR_PYTHON:-}" ]]; then
  export CAMPAIGN_VALIDATOR_PYTHON
else
  unset CAMPAIGN_VALIDATOR_PYTHON
fi
exec "$BIN" "$@"
