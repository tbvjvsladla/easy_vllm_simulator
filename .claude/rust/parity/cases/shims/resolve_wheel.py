"""Original-Python shim for resolve_wheel.py (parity golden capture).

The script's __main__ block now exec()s a Rust binary (a native Rust bridge replaced the Python main path), so running the file would
capture Rust output, not the original Python. This shim runs the file exactly as it was at the last pure-Python revision
(da6a7299b82208b9b8b8d6773753c2f3833ac5e4, pure Python) with __name__ == "__main__". The shim has the
original's basename so argparse usage text is identical. Needs the repo's git history.
"""
import os
import subprocess
import sys

_REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", ".."))
_REL = ".claude/skills/upstream-version-watch/scripts/resolve_wheel.py"
_REV = "da6a7299b82208b9b8b8d6773753c2f3833ac5e4"
_PATH = os.path.join(_REPO, _REL)
_SRC = subprocess.run(["git", "-C", _REPO, "show", _REV + ":" + _REL], stdout=subprocess.PIPE, check=True).stdout
sys.path.insert(0, os.path.dirname(_PATH))
exec(compile(_SRC, _PATH, "exec"), {"__name__": "__main__", "__file__": _PATH})
