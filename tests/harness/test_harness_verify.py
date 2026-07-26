"""tests/harness/test_harness_verify.py -- TDD for scripts/harness_verify.py (plan_26072506
Phase4 cycle2): the harness audit-integration gap. No tracked runner previously invoked
`scripts/policy_registry.py verify --as-of` as part of a full harness E2E correction-cycle --
grep only ever found the CLI's own docstring, never a caller. This is a minimal, tracked,
stdlib-only, fail-closed sequential gate: (1) policy registry audit with an EXPLICIT --as-of
(never wall-clock `today`), (2) the harness unittest suite -- and stage 2 must never even start
if stage 1 reports any violation.

Runner: stdlib `unittest`.

Invocation:
    python3 -m unittest tests.harness.test_harness_verify -v
    python3 -m unittest discover -s tests/harness -p 'test_*.py' -v
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
HARNESS_VERIFY = REPO_ROOT / "scripts" / "harness_verify.py"
SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "policy-registry.schema.json"
EVIDENCE_MANIFEST_PATH = REPO_ROOT / ".claude" / "policies" / "evidence_manifest.json"
DUPLICATE_POLICY_ID_FIXTURE = REPO_ROOT / "tests" / "harness" / "fixtures" / "policy_registry" / \
    "neg_duplicate_policy_id.yaml"

# Fixed, explicit -- matches the real registry's own last_reviewed_at (see test_policy_registry's
# AS_OF). Never the wall-clock `today` (this whole gate exists to force an EXPLICIT --as-of).
REAL_AS_OF = "2026-07-25"

ALWAYS_PASS_TEST = (
    "import unittest\n"
    "from pathlib import Path\n\n"
    "class TestMarker(unittest.TestCase):\n"
    "    def test_writes_marker(self):\n"
    "        Path(__file__).resolve().parent.joinpath('MARKER_RAN').write_text('ran')\n"
    "        self.assertTrue(True)\n"
)

ALWAYS_FAIL_TEST = (
    "import unittest\n\n"
    "class TestFails(unittest.TestCase):\n"
    "    def test_fails(self):\n"
    "        self.fail('deliberate stage-2 failure')\n"
)


def _run(args: list) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(HARNESS_VERIFY)] + args,
                           cwd=REPO_ROOT, capture_output=True, text=True)


def _make_tests_dir(tmp_path: Path, name: str, body: str) -> Path:
    tests_dir = tmp_path / name
    tests_dir.mkdir()
    (tests_dir / "__init__.py").write_text("", encoding="utf-8")
    (tests_dir / "test_probe.py").write_text(body, encoding="utf-8")
    return tests_dir


class TestHarnessVerifyScriptExists(unittest.TestCase):
    def test_script_file_exists(self):
        self.assertTrue(HARNESS_VERIFY.is_file(), msg=f"missing {HARNESS_VERIFY}")

    def test_script_is_stdlib_only(self):
        # AST-level banned-import check reusing the shared production implementation, matching
        # TestReviewCycle1NoPyYAMLDependency's pattern in test_constitution_references.py.
        sys.path.insert(0, str(REPO_ROOT / "scripts"))
        import policy_registry as pr  # noqa: E402
        src = HARNESS_VERIFY.read_text(encoding="utf-8")
        hits = pr.find_banned_imports(src, {"yaml", "jsonschema", "requests"})
        self.assertEqual(hits, [])


class TestUsageErrors(unittest.TestCase):
    def test_missing_as_of_is_exit2(self):
        result = _run([])
        self.assertEqual(result.returncode, 2, msg=result.stdout + result.stderr)

    def test_malformed_as_of_is_exit2(self):
        result = _run(["--as-of", "not-a-date"])
        self.assertEqual(result.returncode, 2, msg=result.stdout + result.stderr)


class TestPositivePathRunsBothStages(unittest.TestCase):
    def test_real_registry_at_its_own_review_date_passes_both_stages_and_stage2_actually_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            tests_dir = _make_tests_dir(tmp_path, "passing_tests", ALWAYS_PASS_TEST)
            marker = tests_dir / "MARKER_RAN"

            result = _run(["--as-of", REAL_AS_OF, "--tests-dir", str(tests_dir)])

            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertTrue(marker.is_file(), msg="stage 2 must actually execute when stage 1 passes")


class TestNonzeroPolicyAuditBlocksCompletion(unittest.TestCase):
    """The core audit-integration contract this file exists to prove: a nonzero policy audit
    blocks completion outright -- stage 2 (the test suite) never runs at all. Proven by pointing
    stage 2 at a tests dir whose sole test writes a marker file on execution: if the marker is
    absent after a failing audit, stage 2 was structurally skipped, not merely ignored."""

    def test_registry_load_error_blocks_before_test_suite_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            tests_dir = _make_tests_dir(tmp_path, "passing_tests", ALWAYS_PASS_TEST)
            marker = tests_dir / "MARKER_RAN"
            nonexistent_registry = tmp_path / "does_not_exist.yaml"

            result = _run(["--as-of", REAL_AS_OF,
                            "--registry", str(nonexistent_registry),
                            "--schema", str(SCHEMA_PATH),
                            "--evidence-manifest", str(EVIDENCE_MANIFEST_PATH),
                            "--tests-dir", str(tests_dir)])

            self.assertEqual(result.returncode, 2, msg=result.stdout + result.stderr)
            self.assertFalse(marker.exists(), msg="stage 2 must never run after a failing audit")

    def test_lifecycle_violation_registry_blocks_before_test_suite_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            tests_dir = _make_tests_dir(tmp_path, "passing_tests", ALWAYS_PASS_TEST)
            marker = tests_dir / "MARKER_RAN"

            result = _run(["--as-of", REAL_AS_OF,
                            "--registry", str(DUPLICATE_POLICY_ID_FIXTURE),
                            "--schema", str(SCHEMA_PATH),
                            "--evidence-manifest", str(EVIDENCE_MANIFEST_PATH),
                            "--tests-dir", str(tests_dir)])

            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertFalse(marker.exists(), msg="stage 2 must never run after a failing audit")


class TestFailingStage2AfterCleanAuditIsADistinctExit(unittest.TestCase):
    def test_failing_stage2_after_passing_stage1_is_a_distinct_nonzero_exit(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            tests_dir = _make_tests_dir(tmp_path, "failing_tests", ALWAYS_FAIL_TEST)

            result = _run(["--as-of", REAL_AS_OF, "--tests-dir", str(tests_dir)])

            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertNotEqual(result.returncode, 2, msg="a stage-2 failure must not be reported "
                                                            "as a usage error")


class TestInterpreterFlagPropagation(unittest.TestCase):
    def test_no_site_flag_is_propagated_and_declared_dependencies_are_explicit(self):
        probe = subprocess.run(
            [sys.executable, "-S", "-c",
             "import sys; sys.path.insert(0,'scripts'); import harness_verify as h; "
             "print(h._child_python()); print(h._child_env().get('PYTHONPATH',''))"],
            cwd=REPO_ROOT, capture_output=True, text=True)
        self.assertEqual(probe.returncode, 0, msg=probe.stdout + probe.stderr)
        self.assertIn("'-S'", probe.stdout)
        self.assertIn("site-packages", probe.stdout)
        version = f"python{sys.version_info.major}.{sys.version_info.minor}"
        declared = Path(sys.executable).parent.parent / "lib" / version / "site-packages"
        env = os.environ.copy()
        env["PYTHONPATH"] = str(declared)
        imported = subprocess.run(
            [sys.executable, "-S", "-c", "import tests.harness.test_policy_claim_predicates"],
            cwd=REPO_ROOT, env=env, capture_output=True, text=True)
        self.assertEqual(imported.returncode, 0, msg=imported.stdout + imported.stderr)

    def test_optimized_mode_is_propagated_and_predicate_plane_refuses_it(self):
        probe = subprocess.run(
            [sys.executable, "-O", "-c",
             "import sys; sys.path.insert(0,'scripts'); import harness_verify as h; "
             "print(h._child_python()); import tests.harness.test_policy_claim_predicates"],
            cwd=REPO_ROOT, capture_output=True, text=True)
        self.assertNotEqual(probe.returncode, 0)
        self.assertIn("'-O'", probe.stdout)
        self.assertIn("refuse optimized Python", probe.stderr)


if __name__ == "__main__":
    unittest.main()
