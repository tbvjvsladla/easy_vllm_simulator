"""Root-rebinding shim for campaign_init.py (parity golden capture).

campaign_init.py derives its repository root from its own file location (parents[4]), so running it would read and
write the real repository's campaigns/. This shim loads the unmodified original (the file at HEAD, __name__ != "__main__")
and rebinds REPO_ROOT / CAMPAIGNS / TEMPLATE / ACTIVE_POINTER to the current directory (a throwaway synthetic repo),
then calls main(). The shim's basename equals the original so argparse usage text is identical.
"""
import importlib.util
import os
import sys
from pathlib import Path

_REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", ".."))
_PATH = os.path.join(_REPO, ".claude/skills/terraforming_node/scripts/campaign_init.py")
sys.path.insert(0, os.path.dirname(_PATH))
_spec = importlib.util.spec_from_file_location("campaign_init", _PATH)
_mod = importlib.util.module_from_spec(_spec)
sys.modules["campaign_init"] = _mod
_spec.loader.exec_module(_mod)
_root = Path(os.getcwd()).resolve()
_mod.REPO_ROOT = _root
_mod.CAMPAIGNS = _root / "campaigns"
_mod.TEMPLATE = _mod.CAMPAIGNS / "_template"
_mod.ACTIVE_POINTER = _mod.CAMPAIGNS / "ACTIVE"
sys.exit(_mod.main())
