#!/bin/bash
# Re-capture the policy-gate goldens from the ORIGINAL Python programs (needs the original
# .claude/policies/runtime/completion_gate.py; the cargo checks never run Python, they compare against these files).
#
#   bash .claude/rust/policy-gate/checks/capture_goldens.sh
#
# For every row of checks/data/cg/cases.tsv (`id<TAB>args`): copy checks/data/cg to a fresh temp dir, create the
# symlinks of symlinks.tsv, run `python3 -B completion_gate.py <args>` with cwd = that copy, and store stdout/stderr/
# exit code under checks/data/cg/golden/<id>.{out,err,rc} with the temp dir spelled <TMP> and the repo root <REPO>.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(cd "$HERE/../../../.." && pwd -P)"
DATA="$HERE/data/cg"
ORIG="$REPO/.claude/policies/runtime/completion_gate.py"
[ -f "$ORIG" ] || { echo "original missing: $ORIG" >&2; exit 2; }
mkdir -p "$DATA/golden"
while IFS=$'\t' read -r id args; do
  [ -n "$id" ] || continue
  tmp="$(mktemp -d)"
  tmp="$(cd "$tmp" && pwd -P)"
  cp -R "$DATA/fx" "$tmp/fx"
  while IFS=$'\t' read -r link target; do ln -s "$target" "$tmp/$link"; done < "$DATA/symlinks.tsv"
  read -r -a argv <<< "$args"
  set +e
  (cd "$tmp" && python3 -B "$ORIG" "${argv[@]}") > "$tmp.out" 2> "$tmp.err"
  rc=$?
  set -e
  for ch in out err; do
    text="$(cat "$tmp.$ch"; printf x)"; text="${text%x}"
    text="${text//$tmp/<TMP>}"; text="${text//$REPO/<REPO>}"
    printf '%s' "$text" > "$DATA/golden/$id.$ch"
  done
  printf '%s\n' "$rc" > "$DATA/golden/$id.rc"
  rm -rf -- "$tmp" "$tmp.out" "$tmp.err"
done < "$DATA/cases.tsv"
# doc_naming.py command line (owners reached for real: node + the bench naming Rust launcher must be present).
DN="$REPO/.claude/policies/runtime/doc_naming.py"
DNDATA="$HERE/data/dn"
mkdir -p "$DNDATA"
while IFS=$'\t' read -r id args; do
  [ -n "$id" ] || continue
  read -r -a argv <<< "$args"
  set +e
  python3 -B "$DN" "${argv[@]}" > "$DNDATA/$id.out" 2> "$DNDATA/$id.err"
  printf '%s\n' "$?" > "$DNDATA/$id.rc"
  set -e
done < "$DNDATA/cases.tsv"
echo "captured $(ls "$DATA/golden" | grep -c '\.rc$') completion_gate cases, $(ls "$DNDATA" | grep -c '\.rc$') doc_naming cases"
