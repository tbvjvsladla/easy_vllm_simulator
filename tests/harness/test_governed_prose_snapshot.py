"""tests/harness/test_governed_prose_snapshot.py -- TDD for `.claude/policies/
governed_prose_snapshot.json` and its validator (plan_26072506 Phase 4 cycle4 remediation of
subagent-summary-1 finding 7 / subagent-summary-0 finding 1's root cause).

The gap this closes: TestNoDuplicateProse's Jaccard token-overlap scan and
TestNoVerbatimPolicyStatementInProse's whole-registry-statement substring check
(test_constitution_references.py) are deliberately diagnostic-only, not semantic proof -- a
reviewer's own paraphrase of a governed rule, sharing few tokens with the registry's own
statement, sails past both. `.claude/policies/governed_prose_snapshot.json` records one exact
SHA256 per governed file (CLAUDE.md, workflow.md, docs.md, references.md, full byte content) --
ANY drift from that snapshot is flagged, including a purely-prose paraphrase that adds no
forbidden token and cites no policy at all. This is explicitly documented as a CHANGE-REVIEW
TRIPWIRE, not independent proof that the new prose is semantically equivalent or non-duplicative --
see scripts/policy_registry.py module docstring.

Runner: stdlib `unittest`.
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
SNAPSHOT_PATH = REPO_ROOT / ".claude" / "policies" / "governed_prose_snapshot.json"

sys.path.insert(0, str(SCRIPTS_DIR))
import policy_registry as pr  # noqa: E402

GOVERNED_LABELS = ["CLAUDE.md", ".claude/rules/workflow.md", ".claude/rules/docs.md",
                   ".claude/rules/references.md"]


class TestGovernedProseSnapshotShape(unittest.TestCase):
    def test_snapshot_file_exists(self):
        self.assertTrue(SNAPSHOT_PATH.is_file())

    def test_snapshot_is_valid_and_covers_all_four_governed_files(self):
        doc = pr.load_governed_prose_snapshot()
        self.assertIsInstance(doc, dict)
        self.assertEqual(set(doc.get("files", {})), set(GOVERNED_LABELS))

    def test_snapshot_digests_are_valid_sha256_hex(self):
        doc = pr.load_governed_prose_snapshot()
        for label, digest in doc["files"].items():
            with self.subTest(label=label):
                self.assertRegex(digest, r"^[0-9a-f]{64}$")

    def test_snapshot_rejects_unknown_version_trust_and_top_level_fields(self):
        base = pr.load_governed_prose_snapshot()
        cases = [
            {**base, "schema_version": "unsupported"},
            {**base, "_trust": None},
            {**base, "override": True},
        ]
        for hostile in cases:
            with self.subTest(hostile=hostile):
                violations = pr.governed_prose_snapshot_violations(hostile, REPO_ROOT)
                self.assertEqual([v.reason_code for v in violations],
                                 ["GOVERNED_PROSE_SNAPSHOT_SHAPE_INVALID"])

    def test_real_governed_files_match_the_committed_snapshot_exactly(self):
        doc = pr.load_governed_prose_snapshot()
        violations = pr.governed_prose_snapshot_violations(doc, REPO_ROOT)
        self.assertEqual(violations, [], msg=[v.to_dict() for v in violations])


class TestGovernedProseSnapshotIsATripwire(unittest.TestCase):
    """Exact cycle3 reproduction: appending a reviewer's prose-only paraphrase of a governed rule
    (no forbidden token, no citation) must still fail -- the tripwire is content identity, not
    keyword/citation presence."""

    def _copy_governed_files(self, dest: Path):
        for label in GOVERNED_LABELS:
            src = REPO_ROOT / label
            target = dest / label
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(src.read_bytes())

    def test_appending_a_prose_only_paraphrase_with_no_forbidden_token_still_trips_the_snapshot(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            self._copy_governed_files(repo_root)
            doc = pr.load_governed_prose_snapshot()
            # sanity: unmodified copy matches
            self.assertEqual(pr.governed_prose_snapshot_violations(doc, repo_root), [])

            docs_md = repo_root / ".claude" / "rules" / "docs.md"
            paraphrase = ("\n## 리뷰어 참고\n\n서브 작업공간에 아직 정리되지 않은 변경이 감지되면 메인 전달을 "
                          "중단하고, 내용을 보존한 채 사람이 먼저 정리하도록 요구한다.\n")
            docs_md.write_text(docs_md.read_text(encoding="utf-8") + paraphrase, encoding="utf-8")

            violations = pr.governed_prose_snapshot_violations(doc, repo_root)
            self.assertEqual([v.reason_code for v in violations], ["GOVERNED_PROSE_SNAPSHOT_MISMATCH"])
            self.assertEqual(violations[0].path, ".claude/rules/docs.md")

    def test_unreadable_governed_file_is_reported_not_silently_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            self._copy_governed_files(repo_root)
            (repo_root / "CLAUDE.md").unlink()
            doc = pr.load_governed_prose_snapshot()
            violations = pr.governed_prose_snapshot_violations(doc, repo_root)
            self.assertIn("GOVERNED_PROSE_FILE_UNREADABLE", [v.reason_code for v in violations])

    def test_standalone_verify_cli_enforces_snapshot_tripwire(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad_snapshot = Path(tmp) / "snapshot.json"
            snapshot = pr.load_governed_prose_snapshot()
            snapshot["files"]["CLAUDE.md"] = "0" * 64
            bad_snapshot.write_text(json.dumps(snapshot), encoding="utf-8")
            proc = subprocess.run(
                [sys.executable, str(SCRIPTS_DIR / "policy_registry.py"), "verify",
                 "--as-of", "2026-07-25", "--repo-root", str(REPO_ROOT),
                 "--governed-prose-snapshot", str(bad_snapshot)],
                capture_output=True, text=True, timeout=60)
            self.assertEqual(proc.returncode, 1, msg=f"out={proc.stdout} err={proc.stderr}")
            out = json.loads(proc.stdout)
            self.assertIn("GOVERNED_PROSE_SNAPSHOT_MISMATCH",
                          [v["reason_code"] for v in out["violations"]])

    def test_check_removal_cli_also_enforces_snapshot_tripwire(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad_snapshot = Path(tmp) / "snapshot.json"
            snapshot = pr.load_governed_prose_snapshot()
            snapshot["files"][".claude/rules/workflow.md"] = "0" * 64
            bad_snapshot.write_text(json.dumps(snapshot), encoding="utf-8")
            proc = subprocess.run(
                [sys.executable, str(SCRIPTS_DIR / "policy_registry.py"), "check-removal",
                 "--policy-id", "ARCH_WALL_VARIANT_LADDER", "--as-of", "2026-07-25",
                 "--repo-root", str(REPO_ROOT),
                 "--governed-prose-snapshot", str(bad_snapshot)],
                capture_output=True, text=True, timeout=60)
            self.assertEqual(proc.returncode, 1, msg=f"out={proc.stdout} err={proc.stderr}")
            out = json.loads(proc.stdout)
            self.assertIn("GOVERNED_PROSE_SNAPSHOT_MISMATCH",
                          [v["reason_code"] for v in out["violations"]])


if __name__ == "__main__":
    unittest.main()
