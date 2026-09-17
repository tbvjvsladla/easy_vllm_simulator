#!/usr/bin/env bash
# Requirement test: canonical active inputs only; no sibling/output identity fallback.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SYNC="$ROOT/.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"
RENDER="$ROOT/.claude/skills/upstream-version-watch/scripts/render_dockerfile.py"

fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

grep -q 'required render input missing' "$SYNC" || fail 'required miss is not fail-loud'
grep -q 'optional render input absent' "$SYNC" || fail 'optional miss is not recorded'
if grep -q 'worktree list --porcelain' "$SYNC"; then fail 'sibling worktree discovery remains'; fi
if grep -q 'a2a_signing/main_ed25519.pem' "$SYNC"; then fail 'legacy private key remains a render input'; fi
python3 - "$RENDER" <<'PY'
import importlib.util, pathlib, sys, tempfile
p=pathlib.Path(sys.argv[1]); s=importlib.util.spec_from_file_location('render_dockerfile',p)
m=importlib.util.module_from_spec(s); s.loader.exec_module(m)
missing=pathlib.Path(tempfile.mkdtemp())/'missing.yaml'
try: m.load_manifest(str(missing))
except FileNotFoundError: pass
else: raise SystemExit('load_manifest swallowed FileNotFoundError')
PY
printf 'PASS: canonical required/optional semantics and missing-manifest behavior\n'
