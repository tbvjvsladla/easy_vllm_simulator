import hashlib
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = ROOT / ".claude/skills/upstream-version-watch/scripts/validate_runtime_patch.py"
SYNC = ROOT / ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"
RESOLUTION = ROOT / ".claude/skills/upstream-version-watch/assets/current-production-resolution.json"
BINDINGS = ROOT / ".claude/policies/claim_bindings.json"


class RuntimePatchProvenanceTest(unittest.TestCase):
    def test_validator_stamps_and_rejects_every_stale_dimension(self):
        self.assertTrue(VALIDATOR.is_file())
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            configs = root / "configs"
            envs = root / "envs"
            configs.mkdir()
            envs.mkdir()
            patch = configs / "modelA_patch.py"
            patch.write_text("# current patch\n", encoding="utf-8")
            (configs / "modelA.yaml").write_text("model: A\nversion: 1\n", encoding="utf-8")
            (envs / ".env.modelA").write_text("MODEL=A\n", encoding="utf-8")
            common = ["--patch", str(patch), "--topology", "multi", "--config-dir", str(configs),
                      "--env-dir", str(envs), "--resolution", str(RESOLUTION)]
            stamp = subprocess.run(["python3", str(VALIDATOR), "stamp", *common], capture_output=True, text=True)
            self.assertEqual(stamp.returncode, 0, stamp.stderr)
            verify = subprocess.run(["python3", str(VALIDATOR), "verify", *common], capture_output=True, text=True)
            self.assertEqual(verify.returncode, 0, verify.stderr)
            sidecar = configs / "modelA_patch.provenance.json"
            data = json.loads(sidecar.read_text(encoding="utf-8"))
            self.assertEqual(data["patch_sha256"], hashlib.sha256(patch.read_bytes()).hexdigest())
            # Patch bytes drift after stamping.
            patch.write_text("# stale carried-forward patch\n", encoding="utf-8")
            bad = subprocess.run(["python3", str(VALIDATOR), "verify", *common], capture_output=True, text=True)
            self.assertNotEqual(bad.returncode, 0)
            patch.write_text("# current patch\n", encoding="utf-8")
            self.assertEqual(subprocess.run(["python3", str(VALIDATOR), "stamp", *common], capture_output=True).returncode, 0)
            # Model/version input drift after stamping.
            (configs / "modelA.yaml").write_text("model: A\nversion: 2\n", encoding="utf-8")
            bad = subprocess.run(["python3", str(VALIDATOR), "verify", *common], capture_output=True, text=True)
            self.assertNotEqual(bad.returncode, 0)
            (configs / "modelA.yaml").write_text("model: A\nversion: 1\n", encoding="utf-8")
            self.assertEqual(subprocess.run(["python3", str(VALIDATOR), "stamp", *common], capture_output=True).returncode, 0)
            # Sidecar topology laundering.
            data = json.loads(sidecar.read_text(encoding="utf-8")); data["topology"] = "single"
            sidecar.write_text(json.dumps(data), encoding="utf-8")
            bad = subprocess.run(["python3", str(VALIDATOR), "verify", *common], capture_output=True, text=True)
            self.assertNotEqual(bad.returncode, 0)

    def test_missing_sidecar_and_orphan_patch_fail_closed(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td); configs = root / "configs"; envs = root / "envs"
            configs.mkdir(); envs.mkdir(); patch = configs / "stale_model_patch.py"
            patch.write_text("# stale provenance-free carried-forward patch\n", encoding="utf-8")
            cmd = ["python3", str(VALIDATOR), "verify", "--patch", str(patch), "--topology", "multi",
                   "--config-dir", str(configs), "--env-dir", str(envs), "--resolution", str(RESOLUTION)]
            self.assertNotEqual(subprocess.run(cmd, capture_output=True).returncode, 0)

    def test_sync_delivery_calls_production_validator_and_transfers_sidecar(self):
        source = SYNC.read_text(encoding="utf-8")
        self.assertIn("validate_runtime_patches", source)
        self.assertIn('validate_runtime_patches "$1"', source)
        self.assertIn("*_patch.provenance.json", source)
        self.assertLess(source.index('validate_runtime_patches "$1"'), source.index("_band2_filters", source.index("deliver_build()")))

    def test_canonical_claim_bindings_are_not_test_only(self):
        bindings = json.loads(BINDINGS.read_text(encoding="utf-8"))["bindings"]
        paths = {entry["path"] for entry in bindings["TERRAFORM_FLAG_GATE.C4"]}
        self.assertIn(".claude/skills/wiki-desk/scripts/smoke_query.py", paths)
        self.assertTrue(any(not p.startswith("tests/") for p in paths))
        c2 = {entry["path"] for entry in bindings["RUNTIME_PATCH_NO_CARRY_FORWARD.C2"]}
        self.assertIn(".claude/skills/upstream-version-watch/scripts/validate_runtime_patch.py", c2)


if __name__ == "__main__":
    unittest.main()
