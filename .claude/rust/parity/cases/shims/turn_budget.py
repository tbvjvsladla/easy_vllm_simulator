"""Original-Python shim for turn_budget.py (parity golden capture).

The script's __main__ block now exec()s a Rust binary (a native Rust bridge replaced the Python main path), so running the file would
capture Rust output, not the original Python. This shim runs the file exactly as it was at the last pure-Python revision
(379cc822396de5eb47541b992b48db0e2ef3c77a, pure Python) with __name__ == "__main__". The shim has the
original's basename so argparse usage text is identical. Needs the repo's git history.
"""
import os
import subprocess
import sys

_REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", ".."))
_REL = ".claude/skills/terraforming_node/scripts/turn_budget.py"
_REV = "379cc822396de5eb47541b992b48db0e2ef3c77a"
_PATH = os.path.join(_REPO, _REL)
_SRC = subprocess.run(["git", "-C", _REPO, "show", _REV + ":" + _REL], stdout=subprocess.PIPE, check=True).stdout
sys.path.insert(0, os.path.dirname(_PATH))
exec(compile(_SRC, _PATH, "exec"), {"__name__": "__main__", "__file__": _PATH})
