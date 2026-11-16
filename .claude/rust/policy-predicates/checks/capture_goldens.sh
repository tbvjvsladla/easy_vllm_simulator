#!/bin/bash
# Re-capture the goldens of the policy-predicates port FROM THE ORIGINAL Python predicates.
# Re-capture needs the original .claude/policies/predicates/*.py (and the Python owners they import); the cargo
# checks never run Python -- they only read what this script wrote:
#   checks/data/orig_verdict.json      `python3 claim_predicates.py` stdout on the real tree (+ its exit code)
#   checks/data/orig_unit.json         exit codes of `python3 -m unittest <module>` for the two companion suites
#   checks/data/negative_goldens.json  per mutation of checks/data/mutations.json, applied to a copy of the tracked
#                                      tree: the original's failure for the targeted clause (`<Class>: <message>`),
#                                      or for a unittest target its exit code and the FAIL/ERROR test names
# Usage: bash checks/capture_goldens.sh   (from anywhere; writes into checks/data/)
# Hand-maintained (not re-capturable from the Python originals): M05-signing-key and M11-push-tags mutate the Rust
# owner sources (render_sub_env.rs, hint tag.rs) the clauses now read (plan_26111615 pass 3); their expected outcomes in
# negative_goldens.json are the original's outcomes for the equivalent .py mutation (same message / same failed test).
set -euo pipefail
CDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$CDIR/../../../.." && pwd)"
DATA="$CDIR/data"
cd "$REPO"
set +e
python3 -B .claude/policies/predicates/claim_predicates.py > "$DATA/orig_verdict.stdout"
rc=$?
set -e
python3 -B - "$DATA" "$rc" <<'PY'
import json, sys
from pathlib import Path
data = Path(sys.argv[1])
out = (data / "orig_verdict.stdout").read_text(encoding="utf-8")
(data / "orig_verdict.stdout").unlink()
(data / "orig_verdict.json").write_text(json.dumps({"exit_code": int(sys.argv[2]), "stdout": out}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
PY
units='{}'
for m in companion_predicate runtime_patch_predicate; do
  set +e
  (cd .claude/policies/predicates && python3 -B -m unittest "$m" >/dev/null 2>&1)
  rc=$?
  set -e
  units="$(python3 -B -c 'import json,sys; d=json.loads(sys.argv[1]); d[sys.argv[2]]=int(sys.argv[3]); print(json.dumps(d, sort_keys=True))' "$units" "$m" "$rc")"
done
printf '%s\n' "$units" > "$DATA/orig_unit.json"

# Negative controls: one copy of the tracked tree per mutation, the original run inside it.
python3 -B - "$REPO" "$DATA" <<'PY'
import json, os, subprocess, sys, tempfile, shutil
from pathlib import Path
repo, data = Path(sys.argv[1]), Path(sys.argv[2])
muts = json.loads((data / "mutations.json").read_text(encoding="utf-8"))
listed = subprocess.run(["git", "-C", str(repo), "ls-files", "-z"], capture_output=True, check=True).stdout.decode().split("\0")
out = {}
for m in muts:
    with tempfile.TemporaryDirectory(prefix="pp-golden-") as td:
        root = Path(td)
        for rel in filter(None, listed):
            src = repo / rel
            if src.is_file() and not src.is_symlink():
                dst = root / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src, dst)
        target = root / m["file"]
        text = target.read_text(encoding="utf-8")
        assert m["old"] in text, f"{m['id']}: anchor missing"
        target.write_text(text.replace(m["old"], m["new"], 1), encoding="utf-8")
        pred = root / ".claude/policies/predicates"
        if "clause" in m:
            code = ("import sys; sys.path.insert(0, %r); import claim_predicates as cp\n"
                    "fn = cp.PREDICATES[%r]\n"
                    "try:\n    fn(); print('PASS')\n"
                    "except Exception as exc:\n    print(f'{type(exc).__name__}: {exc}')\n") % (str(pred), m["clause"])
            r = subprocess.run([sys.executable, "-B", "-c", code], cwd=root, capture_output=True, text=True)
            out[m["id"]] = {"clause": m["clause"], "error": r.stdout.strip().splitlines()[-1] if r.stdout.strip() else r.stderr[-300:]}
        else:
            r = subprocess.run([sys.executable, "-B", "-m", "unittest", m["unit"]], cwd=pred, capture_output=True, text=True)
            failed = sorted(line.split(" ", 2)[1] for line in r.stderr.splitlines() if line.startswith(("FAIL: ", "ERROR: ")))
            out[m["id"]] = {"unit": m["unit"], "exit_code": r.returncode, "failed": failed}
(data / "negative_goldens.json").write_text(json.dumps(out, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8")
PY
echo "[capture_goldens] wrote $DATA/{orig_verdict,orig_unit,negative_goldens}.json"
