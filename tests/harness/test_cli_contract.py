"""tests/harness/test_cli_contract.py -- TDD for scripts/policy_registry.py CLI exit-code
contract fixes (plan_26072506 Phase 4 cycle4 remediation of subagent-summary-1 finding 6).

The gap this closes: the module's OWN documented exit-code table already listed "unknown
--policy-id" under exit 2 (invalid-input), but `check-removal --policy-id DOES_NOT_EXIST` actually
emitted exit 1 (policy-block) -- falling through the generic "any violation -> exit 1" branch
instead of being classified as invalid input.

Runner: stdlib `unittest`.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
import unittest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
POLICY_REGISTRY = SCRIPTS_DIR / "policy_registry.py"

REAL_AS_OF = "2026-07-25"


def _run(args: list) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(POLICY_REGISTRY)] + args, cwd=REPO_ROOT,
                           capture_output=True, text=True)


class TestUnknownPolicyIdIsExit2(unittest.TestCase):
    def test_unknown_policy_id_is_exit2_not_exit1(self):
        result = _run(["check-removal", "--policy-id", "DOES_NOT_EXIST", "--as-of", REAL_AS_OF])
        self.assertEqual(result.returncode, 2, msg=result.stdout + result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["exit_code"], 2)
        self.assertTrue(any(v["reason_code"] == "UNKNOWN_POLICY_ID" for v in payload["violations"]), msg=payload)

    def test_known_retired_ineligible_policy_id_is_still_exit1(self):
        # sanity: a REAL, well-formed policy_id that simply isn't removal-eligible must still be
        # exit 1 (policy-block), not reclassified as exit 2 -- only genuinely UNKNOWN ids move.
        result = _run(["check-removal", "--policy-id", "HOST_SAFETY_LAYERED_DEFENSE", "--as-of", REAL_AS_OF])
        self.assertEqual(result.returncode, 1, msg=result.stdout + result.stderr)
        payload = json.loads(result.stdout)
        self.assertNotIn("UNKNOWN_POLICY_ID", [v["reason_code"] for v in payload["violations"]])


if __name__ == "__main__":
    unittest.main()
