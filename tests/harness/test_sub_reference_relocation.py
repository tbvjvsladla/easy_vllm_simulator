"""Regression coverage for the references.md progressive-disclosure sub overlay."""
from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RENDER = ROOT / ".claude/skills/terraforming_node/scripts/render_sub_env.py"
SYNC = ROOT / ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"
REFERENCE = ROOT / ".claude/skills/wiki-desk/reference/references.md"


def _load_renderer():
    spec = importlib.util.spec_from_file_location("render_sub_env_reference_test", RENDER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TestSubReferenceRelocation(unittest.TestCase):
    def test_renderer_materializes_exact_recipe_reference_dependency(self):
        renderer = _load_renderer()
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "sub_provision"
            result = renderer.render_tree(
                {
                    "SUB_HOST": "203.0.113.11", "MASTER_HOST": "203.0.113.10",
                    "SSH_USER": "tester", "WORKSPACE_PATH": "/srv/ws", "NAS_MOUNT": "/srv/models",
                    "CPU_ARCH": "aarch64", "TOPOLOGY": "single", "INTERCONNECT": "RoCE v2",
                    "INTERCONNECT_IFACE": "eth0", "INTERCONNECT_MTU": "9000", "GID_INDEX": "3",
                    "HCA_DEVICES": "[]", "PLATFORM_PRESET": "test", "RAY_PORT": "6379",
                    "GPU_MODEL": "TEST", "SUB_HOSTNAME": "sub", "SUB_HW_VERIFIED": "true",
                },
                str(out), copy_runtime_block=False,
            )
            new_path = out / ".claude/skills/wiki-desk/reference/references.md"
            self.assertEqual(new_path.read_bytes(), REFERENCE.read_bytes())
            self.assertFalse((out / ".claude/rules/references.md").exists())
            self.assertIn(".claude/skills/wiki-desk/reference/references.md", result["produced"])

    def test_sync_delivers_replacement_before_narrow_tombstone_and_verifies_it(self):
        text = SYNC.read_text(encoding="utf-8")
        self.assertIn("OVERLAY_STALE_PATHS=(.claude/rules/references.md)", text)
        deliver = text.split("deliver_overlay()", 1)[1].split("verify_checksums()", 1)[0]
        self.assertLess(deliver.index("rsync -az"), deliver.index("rm -f --"))
        verify = text.split("verify_checksums()", 1)[1].split("return $fail", 1)[0]
        self.assertIn(".claude/skills/wiki-desk/reference/references.md", verify)


if __name__ == "__main__":
    unittest.main()