#!/bin/bash
# Golden capture (re-capture needs the ORIGINAL policy_registry.py): the exact assertion-id set the original extractor
# (`policy_registry.extract_assertion_ids`, Python `ast` for .py) returns for every .py file and every evidence /
# claim-binding path of commit $REV, read from that commit's blobs (not the working tree), into
# checks/data/assertion_ids.json. The Rust check (checks/ids_golden.rs) reads the same blobs and never runs Python.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(cd "$SDIR/../../../.." && pwd -P)"
REV=b1f3563e
OUT="$SDIR/data/assertion_ids.json"
TMP="$(mktemp -d)"
trap 'rm -rf -- "$TMP"' EXIT
git -C "$REPO" archive -o "$TMP/rev.tar" "$REV"
mkdir "$TMP/tree"
tar -x -C "$TMP/tree" -f "$TMP/rev.tar"
python3 - "$TMP/tree" "$REPO/.claude/policies/runtime" "$OUT" <<'PY'
import json, sys
from pathlib import Path
tree, runtime, out = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
sys.path.insert(0, runtime)
import policy_registry as pr
paths = {str(p.relative_to(tree)) for p in tree.rglob("*.py") if p.is_file() and not p.is_symlink()}
reg = json.loads((tree / ".claude/policies/registry.yaml").read_text())
cb = json.loads((tree / ".claude/policies/claim_bindings.json").read_text())
for p in reg["policies"]:
    for e in p.get("evidence") or []:
        paths.add(e["path"])
for v in cb["bindings"].values():
    for e in v:
        paths.add(e["path"])
res = {}
for rel in sorted(paths):
    ids = pr.extract_assertion_ids(tree / rel)
    res[rel] = None if ids is None else sorted(ids)
out.write_text(json.dumps({"rev": "b1f3563e", "ids": res}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
print(len(res))
PY
# Public-surface tripwire: tracked data must not spell non-public skill script paths literally (decoded by ids_golden.rs).
sed -i "s|terraforming""_node/scripts/|@TFN_SCRIPTS@/|g" "$OUT"
