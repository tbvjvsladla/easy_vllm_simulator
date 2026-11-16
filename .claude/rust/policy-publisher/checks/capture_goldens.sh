#!/bin/bash
# Re-captures checks/data/golden from the ORIGINAL evidence_publisher.py (needs the .py and its owners present:
# completion_gate.py, doc_naming.py + node, the classifier/bench-naming native launchers, render_bench_section.py).
# The cargo check (checks/golden_scenarios.rs) never runs Python; it compares the Rust binary with these files.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(cd "$SDIR/../../../.." && pwd -P)"
ORIG="$REPO/.claude/policies/runtime/evidence_publisher.py"
[ -f "$ORIG" ] || { echo "original $ORIG is gone -- goldens can no longer be re-captured" >&2; exit 2; }
TMP="$(mktemp -d)"
trap 'rm -rf -- "$TMP"' EXIT
cd "$REPO"
bash "$SDIR/scenarios.sh" "$TMP/out" "$TMP/root" python3 -B "$ORIG"
rm -rf -- "$SDIR/data/golden"
mkdir -p -- "$SDIR/data"
cp -r -- "$TMP/out" "$SDIR/data/golden"
echo "captured $(ls "$SDIR/data/golden" | wc -l) files"
