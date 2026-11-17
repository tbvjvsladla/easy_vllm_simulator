#!/bin/bash
# Run one CLI golden case with a given program and write its stdout/stderr/exit_code (normalised) to OUTDIR.
#   run_case.sh CASE_DIR OUTDIR rust
# CASE_DIR/case.sh is sourced in a fresh temporary ROOT (cwd = ROOT). It sets PROG_KIND (policy_registry|harness_verify),
# ARGS=(...), optionally ENVS=(K=V ...), and may define setup() to build fixtures (git repos etc.). REPO = this checkout.
# Program: `rust` = bash .claude/policies/runtime/<kind>.sh (the Rust launcher; no Python). The goldens are frozen
# captures of the deleted Python originals — the `orig` capture mode is retired (plan_26111717).
# Normalisation: the ROOT path -> @ROOT@, the REPO path -> @REPO@ (both channels).
set -uo pipefail
CASE_DIR="$1"; OUTDIR="$2"; MODE="$3"
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(cd "$SDIR/../../../.." && pwd -P)"
ROOT="$(mktemp -d "${TMPDIR:-/tmp}/policy-audit-case.XXXXXX")"
ROOT="$(cd "$ROOT" && pwd -P)"
trap 'rm -rf -- "$ROOT"' EXIT
export GIT_AUTHOR_NAME=fixture GIT_AUTHOR_EMAIL=fixture@invalid GIT_COMMITTER_NAME=fixture GIT_COMMITTER_EMAIL=fixture@invalid
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
unset COLUMNS   # help/usage width comes from the case's ENVS only (the goldens were captured without a terminal)
ARGS=(); ENVS=()
cd "$ROOT" || exit 90
# shellcheck disable=SC1091
. "$CASE_DIR/case.sh"
if declare -F setup >/dev/null; then
  setup >/dev/null 2>&1 || { echo "setup failed for $CASE_DIR" >&2; exit 91; }
fi
cd "$ROOT" || exit 92
case "$MODE" in
  rust) PROGRAM=(bash "$REPO/.claude/policies/runtime/$PROG_KIND.sh") ;;
  *) echo "bad mode $MODE" >&2; exit 93 ;;
esac
env ${ENVS[@]+"${ENVS[@]}"} "${PROGRAM[@]}" ${ARGS[@]+"${ARGS[@]}"} >"$OUTDIR/stdout.raw" 2>"$OUTDIR/stderr.raw" </dev/null
echo $? >"$OUTDIR/exit_code"
for ch in stdout stderr; do
  sed -e "s#$ROOT#@ROOT@#g" -e "s#$REPO#@REPO@#g" "$OUTDIR/$ch.raw" >"$OUTDIR/$ch"
  rm -f -- "$OUTDIR/$ch.raw"
done
