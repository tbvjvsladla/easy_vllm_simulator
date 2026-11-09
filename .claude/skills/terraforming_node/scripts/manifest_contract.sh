#!/bin/bash
# `manifest_contract.py --topology T --repo R [--require-flag] [--json]` (the Flag-gate reader) through the node-local Rust
# executable. Installation and build are explicit operations; a missing executable is an explicit exit 2 with guidance, never a
# silent Python start and never the exit 4 / 3 of the Flag contract. The in-process Python importers of manifest_contract.py
# (render_sub_env, scan_node, claim_predicates, ...) keep importing the Python file; only command-line callers use this launcher.
# The public `manifest_contract.py` command line execs the executable DIRECTLY (same PID, same environment construction as below) and continues here only when the
# executable is missing/not executable, for the availability probe (VALIDATE_ONLY=1) or when that exec fails: this file stays the single place of the guidance and of
# the standalone Bash -> Rust route (run_bench.sh / lite_bench.sh). Behaviour is unchanged.
set -euo pipefail
# Self-contained: no external program is looked up through the caller's PATH (`dirname`, `head`, `mktemp`, ...): the importers' bridge must start even when the
# caller's business PATH holds no tools at all (that PATH is handed unchanged to the `git` child, whose absence the native probe handles). Builtins only.
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
BIN="${MANIFEST_CONTRACT_BIN:-$SDIR/../../adversarial-benchmark/rust/target/release/manifest_contract}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[manifest_contract] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[manifest_contract] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../adversarial-benchmark/scripts/build_rust.sh" >&2
  exit 2
fi
# The private bridge of the in-process importers (`MANIFEST_CONTRACT_BRIDGE` set by manifest_contract.py): the adapter has already
# recorded the caller's plumbing facts, and this launcher must not add, rewrite or drop anything for it. Above all the availability
# check below is NOT the bridge's business: an inherited `MANIFEST_CONTRACT_VALIDATE_ONLY=1` is a caller fact for the `git` child, and
# stopping here would answer the importer with an empty stream.
if [ -n "${MANIFEST_CONTRACT_BRIDGE+x}" ]; then
  exec "$BIN" "$@"
fi
# Before this launcher changes anything it records the CALLER's value of each plumbing variable (and only those): the native program
# hands them back to the children it starts (the `git` probe, the legacy interpreter), so those children see the caller's environment,
# not this launcher's. A missing carrier = the caller had the variable unset.
export MANIFEST_CONTRACT_CALLER_ENV=1
for suffix in BRIDGE_REQUEST COMPAT_OWNER PUBLIC_ARGV0 PYTHON VALIDATE_ONLY; do
  plumbing="MANIFEST_CONTRACT_$suffix"
  carrier="MANIFEST_CONTRACT_CALLER_$suffix"
  if [ -n "${!plumbing+x}" ]; then
    export "$carrier=${!plumbing}"
  else
    unset "$carrier"
  fi
done
unset MANIFEST_CONTRACT_CALLER_BRIDGE
# `MANIFEST_CONTRACT_VALIDATE_ONLY=1` is an availability check for callers that must stop before their first side effect: it
# returns 0 here, after the same ELF and executable-bit check, without any native or Python business. It proves the runtime is
# present, not that it is healthy; every other value (and every argument shape) routes exactly as before.
if [ "${MANIFEST_CONTRACT_VALIDATE_ONLY:-}" = 1 ]; then
  exit 0
fi
unset MANIFEST_CONTRACT_VALIDATE_ONLY
# The owner program is the reviewed ORIGINAL kept beside this launcher as manifest_contract_compat_owner.py (its location, not user intent);
# manifest_contract.py is the thin import adapter. Legacy shapes run the owner under the PUBLIC name so argparse prints the public usage.
export MANIFEST_CONTRACT_COMPAT_OWNER="${MANIFEST_CONTRACT_COMPAT_OWNER:-$SDIR/manifest_contract_compat_owner.py}"
export MANIFEST_CONTRACT_PUBLIC_ARGV0="${MANIFEST_CONTRACT_PUBLIC_ARGV0:-$SDIR/manifest_contract.py}"
# A caller-selected interpreter is explicit legacy intent; empty keeps the default meaning.
if [[ -n "${MANIFEST_CONTRACT_PYTHON:-}" ]]; then
  export MANIFEST_CONTRACT_PYTHON
else
  unset MANIFEST_CONTRACT_PYTHON
fi
exec "$BIN" "$@"
