#!/bin/bash
# Minimal NATIVE startup probes of the R48-R50 Rust-backed bridges, run by build_rust.sh after the explicit build. Each probe starts the REAL compiled
# executable through its launcher with a pure, side-effect-free primitive; there is no Original business, no Python fallback, no host scan, no network and
# no new command-line flag. Probes that RUN (printed one by one):
#   manifest_contract  private bridge `manifest_tp` (pure): {} -> a typed Rust `raised ValueError` (gpus_per_node absent = the contract's own rule) and
#                      {gpus_per_node: 1} on `single` -> a typed Rust `returned int 1`; the request is an owned mode-0600 file that is unlinked afterwards.
#                      The public `manifest_contract.sh --self-test` stays the LegacyShape contract (the reviewed original's Python self-test) and is NOT used here.
#   turn_budget        `turn_budget.sh --self-test` (native self-test over the tracked request schema and the lexical owner path) - main plane only: the
#                      launcher is not delivered to a2a-agent subs, which is reported as SKIPPED, never as passed.
#   scan_node          `scan_node.sh --p1-self-check` is run by build_rust.sh itself (not repeated here).
# Everything else of the 57 B executables is NOT started by this build (30 are started by the owner self-checks in build_rust.sh, +manifest_contract,
# +turn_budget on the main plane): this is not a claim that all native CLIs were started.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
CRATE="${1:-$SDIR/../rust}"
TS="$REPO/.claude/skills/terraforming_node/scripts"
fail() { printf '[native_probe] FAIL %s\n' "$*" >&2; exit 1; }
[ -x /bin/bash ] || { printf '[native_probe] ERROR /bin/bash is required (supported Linux boundary; the probes start every launcher through it)\n' >&2; exit 4; }
for tool in mktemp unlink; do
  command -v "$tool" >/dev/null 2>&1 || { printf '[native_probe] ERROR %s(coreutils) is required for the owned 0600 request file\n' "$tool" >&2; exit 4; }
done
# REQ is assigned in THIS (the parent) shell only - never inside a command substitution - so the EXIT trap below can see the path and clean it. INT/TERM/HUP
# turn into a normal exit (the EXIT trap then runs); bash runs a trap after the current foreground child returns. SIGKILL of this whole process cannot clean itself.
REQ=''
cleanup_req() { if [ -n "$REQ" ]; then command unlink -- "$REQ" 2>/dev/null || true; REQ=''; fi; }
trap cleanup_req EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
ran=()
skipped=()
# protocol: manifest-contract-bridge/1 (adapter `_ask`): {"v":1,"op":..,"args":[typed wires]}; wires: str = utf-8 hex + codepoints, int = sign + hex, dict = id + pairs
STR_SINGLE='{"t":"str","utf8_surrogatepass_hex":"73696e676c65","codepoints":6}'
STR_GPUS='{"t":"str","utf8_surrogatepass_hex":"677075735f7065725f6e6f6465","codepoints":13}'
INT_ONE='{"t":"int","sign":"+","hex":"1"}'
# bridge REQUEST_JSON -> sets BRIDGE_OUT (called directly, NOT as out="$(bridge ...)": a command substitution would own REQ in a subshell)
BRIDGE_OUT=''
bridge() {
  local rc=0
  BRIDGE_OUT=''
  REQ="$(mktemp "${TMPDIR:-/tmp}/manifest-contract-probe.XXXXXX")" || fail "cannot create the owned request file"
  printf '%s' "$1" > "$REQ"
  BRIDGE_OUT="$(MANIFEST_CONTRACT_BRIDGE=1 MANIFEST_CONTRACT_BRIDGE_REQUEST="$REQ" MANIFEST_CONTRACT_BIN="$CRATE/target/release/manifest_contract" \
         /bin/bash "$TS/manifest_contract.sh" </dev/null)" || rc=$?
  cleanup_req
  [ "$rc" -eq 0 ] || fail "manifest_contract bridge exited $rc (the transport must succeed; the API outcome is data)"
}
[ -f "$TS/manifest_contract.sh" ] || fail "manifest_contract.sh is not delivered beside this skill: $TS/manifest_contract.sh"
bridge '{"v":1,"op":"manifest_tp","args":[{"t":"dict","id":1,"pairs":[]},'"$STR_SINGLE"']}'
out="$BRIDGE_OUT"
[[ "$out" =~ \"protocol\"[[:space:]]*:[[:space:]]*\"manifest-contract-bridge/1\" ]] \
  && [[ "$out" =~ \"status\"[[:space:]]*:[[:space:]]*\"raised\" ]] && [[ "$out" =~ \"cls\"[[:space:]]*:[[:space:]]*\"ValueError\" ]] \
  || fail "manifest_tp({}) did not answer a typed raised ValueError: $out"
bridge '{"v":1,"op":"manifest_tp","args":[{"t":"dict","id":1,"pairs":[['"$STR_GPUS"','"$INT_ONE"']]},'"$STR_SINGLE"']}'
out="$BRIDGE_OUT"
[[ "$out" =~ \"status\"[[:space:]]*:[[:space:]]*\"returned\" ]] && [[ "$out" =~ \"value\"[[:space:]]*:[[:space:]]*\{\"t\":\ \"int\",\ \"sign\":\ \"\+\",\ \"hex\":\ \"1\"\} ]] \
  || fail "manifest_tp({gpus_per_node: 1}, single) did not answer the typed int 1: $out"
ran+=("manifest_contract(private bridge manifest_tp x2)")
if [ -f "$TS/turn_budget.sh" ]; then
  tb="$(TURN_BUDGET_BIN="$CRATE/target/release/turn_budget" /bin/bash "$TS/turn_budget.sh" --self-test </dev/null)" || fail "turn_budget --self-test failed"
  [[ "$tb" == *"[PASS]"* ]] && [[ "$tb" != *"[FAIL]"* ]] || fail "turn_budget --self-test did not report only PASS lines"
  ran+=("turn_budget(--self-test)")
else
  skipped+=("turn_budget(turn_budget.sh is not delivered to this plane: main-only)")
fi
printf '[native_probe] RAN: %s\n' "${ran[*]}"
if [ "${#skipped[@]}" -ne 0 ]; then printf '[native_probe] SKIPPED (not passed): %s\n' "${skipped[*]}"; fi
printf '[native_probe] NOT claimed: startup of the other native executables beyond the owner self-checks of build_rust.sh\n'
