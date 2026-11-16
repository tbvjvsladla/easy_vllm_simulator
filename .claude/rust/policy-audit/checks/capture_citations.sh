#!/bin/bash
# Golden capture (re-capture needs the ORIGINAL policy_registry.py): `scan_policy_citations` and `citation_violations`
# over fixed texts against a small registry, into checks/data/citations.json (read by checks/citations_golden.rs).
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(cd "$SDIR/../../../.." && pwd -P)"
python3 - "$REPO/.claude/policies/runtime" "$SDIR/data/citations.json" <<'PY'
import json, sys
sys.path.insert(0, sys.argv[1])
import policy_registry as pr
reg = {"policies": [{"policy_id": "KNOWN", "status": "active"}, {"policy_id": "OLD", "status": "retired"}, "junk"]}
texts = [
    "see policy:KNOWN.", "policy:KNOWN, policy:OLD and policy:MISSING", "bare policy: here", "policy:",
    "xpolicy:BAD-ID", "가policy:BAD-ID", "xpolicy:UNKNOWN_ID tail", "xpolicy:KNOWN", "policy:ALSO_BAD-ID!",
    "line1\nline2 policy:lower\n`policy:KNOWN` (policy:KNOWN) [policy:X]{policy:Y},policy:Z", "policy:A…。", "policy:A-",
    "apolicy:ABC-def", "_policy:ABC", "policy:KNOWN　next", "policy:KNOWN\tpolicy:OLD!?", "policy:9X", "policy:KNOWN\n",
]
out = []
for t in texts:
    out.append({"text": t, "scan": [list(x) for x in pr.scan_policy_citations(t)],
                "violations": [v.to_dict() for v in pr.citation_violations(t, reg, "src.md")]})
open(sys.argv[2], "w", encoding="utf-8").write(json.dumps(out, ensure_ascii=False, indent=1) + "\n")
print(len(out))
PY
