#!/bin/bash
# Re-capture the goldens of this crate's checks from the ORIGINAL Python programs (needs python3 and the original
# `.claude/policies/runtime/{antipattern_scan,constitution_lint,runtime_selftest}.py`; the cargo checks themselves never
# start Python). Run from anywhere: bash .claude/rust/policy-selftest/checks/capture_goldens.sh [pyast|cli|all]
#
#  pyast: CPython 3.12 `ast.walk` digests (kind:lineno:end_lineno of Assign/Call/Compare/FunctionDef/AsyncFunctionDef/
#         Assert) of every checks/data/pyast/src/*.txt source -> checks/data/pyast/cases.json. The originals inspect
#         exactly those nodes (antipattern inventory, tripwire ⑩ literal binding, the bare-assert scan).
#  cli:   stdout/stderr/exit of the originals on the fixture repositories built by checks/fixtures.sh ->
#         checks/data/cli/<case>/{stdout,stderr,exit_code}.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
CHECKS="$(cd "${SELF%/*}" && pwd)"
REPO="$(cd "$CHECKS/../../../.." && pwd)"
what="${1:-all}"

if [ "$what" = pyast ] || [ "$what" = all ]; then
  # Sources: the policies bundle's own originals (the parser's real inputs), two repository files with tricky
  # literals, and the synthetic corpus. They are stored base64-encoded: a plain copy of source text in the tracked tree
  # would itself be governed by the repository-wide reference tripwires (⑩ anchors, the public-surface scan, PII).
  python3 -I - "$CHECKS/data/pyast" "$REPO" <<'PY'
import ast, base64, json, sys
from pathlib import Path
base, repo = Path(sys.argv[1]), Path(sys.argv[2])
kinds = (ast.Assign, ast.Call, ast.Compare, ast.FunctionDef, ast.AsyncFunctionDef, ast.Assert)
names = ["runtime/" + n for n in ("verify_distribution", "topology_parity", "runtime_selftest", "policy_registry",
         "harness_verify", "evidence_publisher", "doc_naming", "constitution_lint", "completion_gate", "antipattern_scan")]
names += ["predicates/" + n for n in ("claim_predicates", "companion_predicate", "runtime_patch_predicate")]
sources = [repo / ".claude/policies" / (n + ".py") for n in names]
sources += [repo / ".claude/skills/adversarial-benchmark/scripts/doc_naming.py",
            repo / ".claude/skills/upstream-version-watch/scripts/selftest_branch_sync.py"]
sources += sorted((base / "src").glob("*.txt"))
cases = []
for src in sources:
    text = src.read_text(encoding="utf-8")
    try:
        tree = ast.parse(text)
        walk = [f"{type(n).__name__}:{n.lineno}:{n.end_lineno}" for n in ast.walk(tree) if isinstance(n, kinds)]
    except SyntaxError:
        walk = None
    cases.append({"name": src.name, "source_b64": base64.b64encode(text.encode("utf-8")).decode("ascii"), "walk": walk})
(base / "cases.json").write_text(json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
print(f"pyast cases={len(cases)}")
PY
fi

if [ "$what" = cli ] || [ "$what" = all ]; then
  bash "$CHECKS/fixtures.sh" capture "$REPO"
fi
