"""tests/harness/test_completion_gate.py -- TDD suite for scripts/completion_gate.py
(plan_26072506 Phase 1 + independent-review remediation).

Runner: stdlib `unittest` (pytest is unavailable in this environment and installing new
dependencies is out of scope for this phase -- see testlog design-rationale).

Invocation:
    python3 -m unittest tests.harness.test_completion_gate -v
    python3 -m unittest discover -s tests/harness -p 'test_*.py' -v

Each test invokes the gate as a real subprocess (`python3 scripts/completion_gate.py verify
--manifest <fixture>`) and asserts on actual stdout JSON + real process exit code -- black-box
contract tests that stay valid even if the script's internals are refactored later.

Hermetic design (review finding NON_HERMETIC_TDD_FIXTURES): no fixture depends on a fixed /tmp
path or on a file matched by .gitignore. Symlink-escape and FIFO-rejection scenarios build their
own external target + symlink/fifo inside a temporary, repo-contained scratch directory at test
run time (setUp/tearDown), so a clean `git checkout-index` of the staged index reproduces every
test unmodified.
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GATE_SCRIPT = REPO_ROOT / "scripts" / "completion_gate.py"
FIXTURES_DIR = REPO_ROOT / "tests" / "harness" / "fixtures" / "completion"
SCRATCH_DIR = FIXTURES_DIR / "_runtime_scratch"

# White-box import of the gate module itself -- used ONLY by TestOutputSchemaInvariantEnforced
# (Section H) to drive the schema-validator directly against hand-built (possibly contradictory)
# completion-manifest dicts. The gate's own business logic never legitimately emits a
# contradictory state/eligible_for_promotion pair, so proving the SCHEMA itself rejects one
# cannot be done through the CLI/subprocess black-box path used by every other test here.
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import completion_gate as _gate  # noqa: E402


def run_gate(fixture_name: str, repo_root: Path | None = None) -> tuple[int, dict, str]:
    """Invoke the gate CLI as a subprocess against a fixture file. Returns
    (exit_code, parsed_stdout_json, stderr_text)."""
    manifest_path = FIXTURES_DIR / fixture_name
    return run_gate_path(manifest_path, repo_root=repo_root)


def run_gate_path(manifest_path: Path, repo_root: Path | None = None) -> tuple[int, dict, str]:
    # Always pass --repo-root explicitly (defaulting to REPO_ROOT, computed from this test
    # file's own location) rather than relying on the gate's .git-based auto-detection -- a
    # `git checkout-index` clean-checkout export has no .git directory, and hermetic
    # verification (plan_26072506 required behavior E3) requires every test to pass unmodified
    # from such an export. Auto-detection itself remains real CLI behavior (verified manually,
    # see testlog) but is deliberately not exercised by this environment-coupled test suite.
    if repo_root is None:
        repo_root = REPO_ROOT
    cmd = [sys.executable, str(GATE_SCRIPT), "verify", "--manifest", str(manifest_path),
           "--repo-root", str(repo_root)]
    proc = subprocess.run(cmd, cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=30)
    try:
        parsed = json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        raise AssertionError(
            f"gate stdout was not valid JSON (exit={proc.returncode}): {e}\n"
            f"--- stdout ---\n{proc.stdout}\n--- stderr ---\n{proc.stderr}"
        ) from e
    return proc.returncode, parsed, proc.stderr


def assert_sorted_reason_codes(test: unittest.TestCase, out: dict) -> None:
    codes = out.get("reason_codes", [])
    test.assertEqual(codes, sorted(codes), msg=f"reason_codes must be deterministically sorted: {out}")


def assert_output_schema_shape(test: unittest.TestCase, out: dict) -> None:
    for key in ("schema_version", "state", "eligible_for_promotion", "reason_codes",
                "checked_evidence", "identity"):
        test.assertIn(key, out, msg=f"missing required output field: {key}")
    test.assertIn(out.get("exit_code"), (0, 1, 2), msg=f"out={out}")


# =============================================================================
# Section A -- full schema enforcement (generic validator, single source)
# =============================================================================

class TestSchemaVersion(unittest.TestCase):
    def test_schema_version_999_is_invalid_input(self):
        code, out, err = run_gate("fixture32_schema_version_mismatch.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_CONST_VIOLATION:schema_version", out.get("reason_codes", []), msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"))


class TestSchemaShapeUnsupportedTaskClass(unittest.TestCase):
    def test_exit_code_invalid_input(self):
        code, out, err = run_gate("fixture9_unsupported_task_class.json")
        self.assertEqual(code, 2, msg=f"out={out}")

    def test_state_is_null(self):
        code, out, err = run_gate("fixture9_unsupported_task_class.json")
        self.assertIsNone(out.get("state"), msg=f"out={out}")

    def test_reason_code_present(self):
        code, out, err = run_gate("fixture9_unsupported_task_class.json")
        self.assertIn("SCHEMA_ENUM_VIOLATION:task_class", out.get("reason_codes", []), msg=f"out={out}")

    def test_not_eligible(self):
        code, out, err = run_gate("fixture9_unsupported_task_class.json")
        self.assertFalse(out.get("eligible_for_promotion"))


class TestSchemaShapeMissingRequiredTopLevelKey(unittest.TestCase):
    CASES = [
        ("fixture10a_missing_identity.json", "identity"),
        ("fixture10b_missing_evidence.json", "evidence"),
        ("fixture10c_missing_pii_scan.json", "pii_scan"),
    ]

    def test_exit_code_invalid_input(self):
        for fixture, missing_key in self.CASES:
            with self.subTest(fixture=fixture):
                code, out, err = run_gate(fixture)
                self.assertEqual(code, 2, msg=f"out={out}")

    def test_reason_code_names_missing_key(self):
        for fixture, missing_key in self.CASES:
            with self.subTest(fixture=fixture):
                code, out, err = run_gate(fixture)
                self.assertIn(f"SCHEMA_MISSING_REQUIRED_KEY:{missing_key}",
                              out.get("reason_codes", []), msg=f"out={out}")

    def test_state_is_null(self):
        for fixture, missing_key in self.CASES:
            with self.subTest(fixture=fixture):
                code, out, err = run_gate(fixture)
                self.assertIsNone(out.get("state"), msg=f"out={out}")

    def test_nested_required_key_missing(self):
        code, out, err = run_gate("fixture10d_identity_missing_nested_key.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_MISSING_REQUIRED_KEY:identity.quant", out.get("reason_codes", []), msg=f"out={out}")


class TestSchemaShapeInvalidIdentity(unittest.TestCase):
    def test_wrong_type_field_exit_code(self):
        code, out, err = run_gate("fixture11a_identity_wrong_type.json")
        self.assertEqual(code, 2, msg=f"out={out}")

    def test_wrong_type_field_reason_code(self):
        code, out, err = run_gate("fixture11a_identity_wrong_type.json")
        self.assertIn("SCHEMA_TYPE_MISMATCH:identity.model", out.get("reason_codes", []), msg=f"out={out}")

    def test_tp_below_minimum_exit_code(self):
        code, out, err = run_gate("fixture11b_identity_tp_below_minimum.json")
        self.assertEqual(code, 2, msg=f"out={out}")

    def test_tp_below_minimum_reason_code(self):
        code, out, err = run_gate("fixture11b_identity_tp_below_minimum.json")
        self.assertIn("SCHEMA_MINIMUM_VIOLATION:identity.tp", out.get("reason_codes", []), msg=f"out={out}")

    def test_state_is_null(self):
        code, out, err = run_gate("fixture11b_identity_tp_below_minimum.json")
        self.assertIsNone(out.get("state"), msg=f"out={out}")


class TestSchemaShapeUnknownTopLevelProperty(unittest.TestCase):
    def test_exit_code_invalid_input(self):
        code, out, err = run_gate("fixture12_unknown_top_level_property.json")
        self.assertEqual(code, 2, msg=f"out={out}")

    def test_reason_code_names_property(self):
        code, out, err = run_gate("fixture12_unknown_top_level_property.json")
        self.assertIn("SCHEMA_UNKNOWN_PROPERTY:surprise_extra_field",
                      out.get("reason_codes", []), msg=f"out={out}")

    def test_state_is_null(self):
        code, out, err = run_gate("fixture12_unknown_top_level_property.json")
        self.assertIsNone(out.get("state"), msg=f"out={out}")


class TestSchemaShapeRootNotObject(unittest.TestCase):
    """Reviewer-verified gap: a JSON root that isn't an object at all must not crash or reach
    promotion-ready -- must be caught by the generic type check at the schema root."""

    def test_root_is_list(self):
        code, out, err = run_gate("fixture13_root_is_list.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:$", out.get("reason_codes", []), msg=f"out={out}")
        self.assertIsNone(out.get("state"))

    def test_root_is_null(self):
        code, out, err = run_gate("fixture14_root_is_null.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:$", out.get("reason_codes", []), msg=f"out={out}")


class TestSchemaShapeNestedAdditionalProperties(unittest.TestCase):
    def test_identity_extra_property_rejected(self):
        code, out, err = run_gate("fixture15_identity_extra_property.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_UNKNOWN_PROPERTY:identity.extra_field", out.get("reason_codes", []), msg=f"out={out}")


class TestSchemaShapeInvalidTopologyEnum(unittest.TestCase):
    def test_invalid_topology_rejected(self):
        code, out, err = run_gate("fixture16_invalid_topology_enum.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_ENUM_VIOLATION:identity.topology", out.get("reason_codes", []), msg=f"out={out}")


class TestSchemaShapeQuantWrongType(unittest.TestCase):
    def test_quant_number_rejected(self):
        code, out, err = run_gate("fixture17_quant_wrong_type.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:identity.quant", out.get("reason_codes", []), msg=f"out={out}")


class TestSchemaShapeMalformedNestedTypes(unittest.TestCase):
    """Direct reproductions of the reviewer's uncaught-AttributeError fuzz cases -- must now be
    stable JSON exit 2, never a bare traceback / non-JSON stdout."""

    def test_runtime_as_string(self):
        code, out, err = run_gate("fixture18_runtime_wrong_type.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:runtime", out.get("reason_codes", []), msg=f"out={out}")

    def test_container_item_malformed(self):
        code, out, err = run_gate("fixture19_container_item_malformed.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        codes = out.get("reason_codes", [])
        self.assertIn("SCHEMA_TYPE_MISMATCH:runtime.containers[0].name", codes, msg=f"out={out}")
        self.assertIn("SCHEMA_MISSING_REQUIRED_KEY:runtime.containers[0].restart_count", codes, msg=f"out={out}")
        self.assertIn("SCHEMA_MISSING_REQUIRED_KEY:runtime.containers[0].oom_killed", codes, msg=f"out={out}")

    def test_capacity_rejection_malformed(self):
        code, out, err = run_gate("fixture20_capacity_rejection_malformed.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        codes = out.get("reason_codes", [])
        self.assertIn("SCHEMA_MISSING_REQUIRED_KEY:capacity_rejection.gate_evidence", codes, msg=f"out={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:capacity_rejection.rejected", codes, msg=f"out={out}")

    def test_benchmark_invalid_mode(self):
        code, out, err = run_gate("fixture21_benchmark_invalid_mode.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("SCHEMA_ENUM_VIOLATION:benchmark.mode", out.get("reason_codes", []), msg=f"out={out}")

    def test_evidence_item_as_string(self):
        code, out, err = run_gate("fixture22_evidence_item_wrong_type.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:evidence.plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_evidence_item_missing_path(self):
        code, out, err = run_gate("fixture23_evidence_item_missing_path.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("SCHEMA_MISSING_REQUIRED_KEY:evidence.plan.path", out.get("reason_codes", []), msg=f"out={out}")

    def test_evidence_path_empty_string(self):
        code, out, err = run_gate("fixture24_evidence_path_empty_string.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("SCHEMA_MIN_LENGTH_VIOLATION:evidence.plan.path", out.get("reason_codes", []), msg=f"out={out}")

    def test_pii_scan_as_string(self):
        code, out, err = run_gate("fixture25_pii_scan_wrong_type.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:pii_scan", out.get("reason_codes", []), msg=f"out={out}")

    def test_pii_scanned_paths_item_wrong_type(self):
        code, out, err = run_gate("fixture26_pii_scanned_paths_item_wrong_type.json")
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("SCHEMA_TYPE_MISMATCH:pii_scan.scanned_paths[0]", out.get("reason_codes", []), msg=f"out={out}")

    def test_all_malformed_cases_emit_valid_json_never_traceback(self):
        for fixture in (
            "fixture18_runtime_wrong_type.json", "fixture19_container_item_malformed.json",
            "fixture20_capacity_rejection_malformed.json", "fixture21_benchmark_invalid_mode.json",
            "fixture22_evidence_item_wrong_type.json", "fixture23_evidence_item_missing_path.json",
            "fixture24_evidence_path_empty_string.json", "fixture25_pii_scan_wrong_type.json",
            "fixture26_pii_scanned_paths_item_wrong_type.json",
        ):
            with self.subTest(fixture=fixture):
                code, out, err = run_gate(fixture)  # raises AssertionError itself if stdout isn't JSON
                assert_output_schema_shape(self, out)
                assert_sorted_reason_codes(self, out)


class TestCertificateIdentityCannotBeSelfAsserted(unittest.TestCase):
    """B3: manifest-declared evidence.certificate.identity must be structurally impossible."""

    def test_certificate_identity_subobject_rejected(self):
        code, out, err = run_gate("fixture27_evidence_certificate_with_identity_rejected.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("SCHEMA_UNKNOWN_PROPERTY:evidence.certificate.identity",
                      out.get("reason_codes", []), msg=f"out={out}")


class TestInvalidInputHandling(unittest.TestCase):
    def test_missing_manifest_file_is_invalid_input(self):
        code, out, err = run_gate("does_not_exist_at_all.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("MANIFEST_FILE_NOT_FOUND", out.get("reason_codes", []))

    def test_malformed_json_is_invalid_input(self):
        code, out, err = run_gate("malformed_not_json.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("MANIFEST_INVALID_JSON", out.get("reason_codes", []))

    def test_invalid_input_state_is_null(self):
        code, out, err = run_gate("malformed_not_json.json")
        self.assertIsNone(out.get("state"))
        self.assertFalse(out.get("eligible_for_promotion"))

    def test_manifest_that_is_a_directory_is_invalid_input(self):
        code, out, err = run_gate_path(FIXTURES_DIR / "targets" / "simlog_run")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("MANIFEST_IS_A_DIRECTORY", out.get("reason_codes", []), msg=f"out={out}")

    def test_process_exit_code_equals_json_exit_code_field(self):
        for fixture in ("fixture9_unsupported_task_class.json", "fixture1_runtime_ready_no_docs.json",
                         "fixture6_full_pass_promotion_ready.json"):
            with self.subTest(fixture=fixture):
                code, out, err = run_gate(fixture)
                self.assertEqual(code, out.get("exit_code"), msg=f"out={out}")


# =============================================================================
# Section B -- strong (6-key) identity: runtime tier + certificate artifact trust
# =============================================================================

class TestFixture1RuntimeReadyNoDocs(unittest.TestCase):
    def test_state_is_runtime_ready(self):
        code, out, err = run_gate("fixture1_runtime_ready_no_docs.json")
        self.assertEqual(out.get("state"), "runtime-ready", msg=f"stderr={err}\nout={out}")

    def test_promotion_is_blocked(self):
        code, out, err = run_gate("fixture1_runtime_ready_no_docs.json")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")

    def test_exit_code_is_policy_block(self):
        code, out, err = run_gate("fixture1_runtime_ready_no_docs.json")
        self.assertEqual(code, 1, msg=f"stderr={err}\nout={out}")

    def test_missing_evidence_reasons_present_and_sorted(self):
        code, out, err = run_gate("fixture1_runtime_ready_no_docs.json")
        assert_sorted_reason_codes(self, out)
        codes = out.get("reason_codes", [])
        for key in ("plan", "devlog", "testlog", "simlog"):
            self.assertIn(f"EVIDENCE_MISSING:{key}", codes)

    def test_output_matches_completion_manifest_schema_shape(self):
        code, out, err = run_gate("fixture1_runtime_ready_no_docs.json")
        assert_output_schema_shape(self, out)


class TestRuntimeTierFunctionalSmokeAndIdentity(unittest.TestCase):
    """B2: runtime-backed classes require functional_smoke_passed=true and runtime.identity
    matching the top-level identity on all 6 strong fields."""

    def test_functional_smoke_not_passed_blocks_runtime_tier(self):
        code, out, err = run_gate("fixture28_runtime_functional_smoke_not_passed.json")
        self.assertEqual(code, 1, msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertIn("RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED", out.get("reason_codes", []), msg=f"out={out}")

    def test_runtime_identity_missing_blocks_runtime_tier(self):
        code, out, err = run_gate("fixture29_runtime_identity_missing.json")
        self.assertEqual(code, 1, msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertIn("RUNTIME_IDENTITY_MISSING", out.get("reason_codes", []), msg=f"out={out}")

    def test_runtime_identity_mismatch_blocks_runtime_tier(self):
        code, out, err = run_gate("fixture30_runtime_identity_mismatch.json")
        self.assertEqual(code, 1, msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertIn("RUNTIME_IDENTITY_MISMATCH:gpu", out.get("reason_codes", []), msg=f"out={out}")


class TestFixture3IdentityMismatch(unittest.TestCase):
    """Certificate identity is now trusted only from the parsed artifact -- fixture3's real
    certificate_mismatch_gpu.yaml disagrees with manifest identity.gpu."""

    def test_identity_mismatch_reason_present(self):
        code, out, err = run_gate("fixture3_identity_mismatch.json")
        self.assertIn("IDENTITY_MISMATCH:gpu", out.get("reason_codes", []), msg=f"out={out}")

    def test_state_capped_below_evidence_complete(self):
        code, out, err = run_gate("fixture3_identity_mismatch.json")
        self.assertEqual(out.get("state"), "runtime-ready", msg=f"out={out}")

    def test_promotion_blocked(self):
        code, out, err = run_gate("fixture3_identity_mismatch.json")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")

    def test_certificate_evidence_reported_with_identity_mismatch_flag(self):
        code, out, err = run_gate("fixture3_identity_mismatch.json")
        cert_check = out.get("checked_evidence", {}).get("certificate")
        self.assertIsNotNone(cert_check, msg=f"out={out}")
        self.assertTrue(cert_check["identity_checked"])
        self.assertFalse(cert_check["identity_match"])

    def test_certificate_output_reflects_actual_parsed_artifact(self):
        code, out, err = run_gate("fixture3_identity_mismatch.json")
        cert = out.get("certificate")
        self.assertIsNotNone(cert, msg=f"out={out}")
        self.assertTrue(cert["parsed"])
        self.assertEqual(cert["identity"]["gpu"], "RTX-PRO-6000", msg=f"out={out}")


class TestFixture4CertificateWithoutPass(unittest.TestCase):
    def test_exit_code_is_invalid_input(self):
        code, out, err = run_gate("fixture4_cert_without_pass.json")
        self.assertEqual(code, 2, msg=f"out={out}")

    def test_reason_code_present(self):
        code, out, err = run_gate("fixture4_cert_without_pass.json")
        self.assertIn("CERTIFICATE_PRESENT_WITHOUT_PASS_VERDICT", out.get("reason_codes", []), msg=f"out={out}")

    def test_state_is_null(self):
        code, out, err = run_gate("fixture4_cert_without_pass.json")
        self.assertIsNone(out.get("state"), msg=f"out={out}")

    def test_not_eligible(self):
        code, out, err = run_gate("fixture4_cert_without_pass.json")
        self.assertFalse(out.get("eligible_for_promotion"))


class TestCertificateNullVerdictAlsoStructural(unittest.TestCase):
    """PASS_ONLY_CERTIFICATE_GAP fix: null/absent verdict + certificate present must be exit 2 too,
    not just an explicit non-PASS verdict string."""

    def test_null_verdict_with_certificate_is_invalid_input(self):
        code, out, err = run_gate("fixture39_certificate_null_verdict_structural.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("CERTIFICATE_PRESENT_WITHOUT_PASS_VERDICT", out.get("reason_codes", []), msg=f"out={out}")
        self.assertIsNone(out.get("state"))


class TestFixture5CapacityRejection(unittest.TestCase):
    def test_state_is_evidence_complete(self):
        code, out, err = run_gate("fixture5_capacity_rejection.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")

    def test_promotion_capped_reason_present(self):
        code, out, err = run_gate("fixture5_capacity_rejection.json")
        self.assertIn("TASK_CLASS_CAPS_BELOW_PROMOTION:capacity_rejection",
                       out.get("reason_codes", []), msg=f"out={out}")

    def test_not_eligible(self):
        code, out, err = run_gate("fixture5_capacity_rejection.json")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")

    def test_no_pre_runtime_reason_despite_missing_runtime_block(self):
        code, out, err = run_gate("fixture5_capacity_rejection.json")
        self.assertNotIn("PRE_RUNTIME_NOT_HEALTHY", out.get("reason_codes", []), msg=f"out={out}")

    def test_gate_evidence_hashed_as_regular_file(self):
        code, out, err = run_gate("fixture5_capacity_rejection.json")
        codes = out.get("reason_codes", [])
        self.assertFalse(any("ESCAPES_REPO" in c or "CONTAINS_SYMLINK" in c for c in codes), msg=f"out={out}")

    def test_gate_evidence_appears_in_checked_evidence_with_hash(self):
        """CAPACITY_GATE_RAW_PII_COVERAGE_GAP fix, checked-evidence half: gate_evidence must be
        surfaced (path/exists/type/sha256), not silently absent from the output like a regular
        evidence.* entry that was never even looked at."""
        code, out, err = run_gate("fixture5_capacity_rejection.json")
        gate_check = out.get("checked_evidence", {}).get("capacity_rejection.gate_evidence")
        self.assertIsNotNone(gate_check, msg=f"out={out}")
        self.assertTrue(gate_check["required"], msg=f"out={out}")
        self.assertTrue(gate_check["exists"], msg=f"out={out}")
        self.assertEqual(gate_check["type"], "file", msg=f"out={out}")
        self.assertIsInstance(gate_check.get("sha256"), str, msg=f"out={out}")
        self.assertEqual(len(gate_check["sha256"]), 64, msg=f"out={out}")

    def test_pii_coverage_now_includes_gate_evidence(self):
        code, out, err = run_gate("fixture5_capacity_rejection.json")
        self.assertNotIn("PII_SCAN_COVERAGE_INCOMPLETE:capacity_rejection.gate_evidence",
                          out.get("reason_codes", []), msg=f"out={out}")


class TestCapacityGatePiiCoverageGap(unittest.TestCase):
    """CAPACITY_GATE_RAW_PII_COVERAGE_GAP: fixture5 with its own gate_evidence.txt path missing
    from pii_scan.scanned_paths must be blocked from evidence-complete (this is the exact
    reviewer-verified gap -- fixture60 is fixture5 with only that one field regressed)."""

    def test_missing_gate_evidence_coverage_blocks_evidence_complete(self):
        code, out, err = run_gate("fixture60_capacity_gate_pii_coverage_incomplete.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("PII_SCAN_COVERAGE_INCOMPLETE:capacity_rejection.gate_evidence",
                      out.get("reason_codes", []), msg=f"out={out}")


class TestFixture6FullPassPromotionReady(unittest.TestCase):
    def test_state_is_promotion_ready(self):
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        self.assertEqual(out.get("state"), "promotion-ready", msg=f"stderr={err}\nout={out}")

    def test_eligible_for_promotion_true(self):
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        self.assertTrue(out.get("eligible_for_promotion"), msg=f"out={out}")

    def test_exit_code_zero(self):
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        self.assertEqual(code, 0, msg=f"out={out}")

    def test_no_reason_codes(self):
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        self.assertEqual(out.get("reason_codes"), [], msg=f"out={out}")

    def test_state_eligible_invariant(self):
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        self.assertEqual(out.get("state") == "promotion-ready", out.get("eligible_for_promotion"))

    def test_certificate_hash_present(self):
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        cert_check = out.get("checked_evidence", {}).get("certificate")
        self.assertEqual(cert_check.get("type"), "file", msg=f"out={out}")
        self.assertIsInstance(cert_check.get("sha256"), str, msg=f"out={out}")
        self.assertEqual(len(cert_check["sha256"]), 64, msg=f"out={out}")

    def test_simlog_tree_fingerprint_present(self):
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        simlog_check = out.get("checked_evidence", {}).get("simlog")
        self.assertEqual(simlog_check.get("type"), "dir", msg=f"out={out}")
        self.assertIsInstance(simlog_check.get("tree_sha256"), str, msg=f"out={out}")
        self.assertEqual(len(simlog_check["tree_sha256"]), 64, msg=f"out={out}")


class TestFixture7PiiFailBlocksEvidenceComplete(unittest.TestCase):
    """C3 contract fix: PII fail must now block evidence-complete itself (plan's own definition),
    not just promotion -- the review's EVIDENCE_COMPLETE_PII_CONTRADICTION finding."""

    def test_state_capped_to_runtime_ready(self):
        code, out, err = run_gate("fixture7_pii_fail_caps_promotion.json")
        self.assertEqual(out.get("state"), "runtime-ready", msg=f"out={out}")

    def test_not_eligible(self):
        code, out, err = run_gate("fixture7_pii_fail_caps_promotion.json")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")

    def test_pii_reason_present(self):
        code, out, err = run_gate("fixture7_pii_fail_caps_promotion.json")
        self.assertIn("PII_SCAN_FAILED", out.get("reason_codes", []), msg=f"out={out}")

    def test_exit_code_policy_block(self):
        code, out, err = run_gate("fixture7_pii_fail_caps_promotion.json")
        self.assertEqual(code, 1, msg=f"out={out}")


class TestCertificateArtifactTrustBoundary(unittest.TestCase):
    """B3/B4: certificate identity/verdict/benchmark_mode trust comes only from parsing the real
    artifact; a PASS full_benchmark with a missing certificate must not be evidence-complete."""

    def test_missing_certificate_blocks_evidence_complete_even_with_pass_verdict(self):
        code, out, err = run_gate("fixture33_certificate_missing_but_pass.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertIn("EVIDENCE_MISSING:certificate", out.get("reason_codes", []), msg=f"out={out}")

    def test_partial_certificate_identity_blocks(self):
        code, out, err = run_gate("fixture34_certificate_identity_partial.json")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertTrue(any(c.startswith("IDENTITY_MISMATCH:") for c in codes), msg=f"out={out}")

    def test_quant_mismatch_blocks(self):
        code, out, err = run_gate("fixture35_certificate_quant_mismatch.json")
        self.assertIn("IDENTITY_MISMATCH:quant", out.get("reason_codes", []), msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")

    def test_manifest_claim_contradicting_artifact_verdict_blocks(self):
        code, out, err = run_gate("fixture36_certificate_claims_contradiction.json")
        self.assertIn("CERTIFICATE_VERDICT_MISMATCH", out.get("reason_codes", []), msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")

    def test_manifest_claim_contradicting_artifact_mode_blocks(self):
        code, out, err = run_gate("fixture37_certificate_wrong_mode.json")
        self.assertIn("CERTIFICATE_BENCHMARK_MODE_MISMATCH", out.get("reason_codes", []), msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")

    def test_unparseable_certificate_fails_closed(self):
        code, out, err = run_gate("fixture38_certificate_unparseable.json")
        self.assertIn("CERTIFICATE_UNPARSEABLE", out.get("reason_codes", []), msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        cert = out.get("certificate")
        self.assertIsNotNone(cert)
        self.assertFalse(cert["parsed"])


# =============================================================================
# Section C -- evidence / path / PII hardening
# =============================================================================

class TestPathEscapeBlocked(unittest.TestCase):
    def test_repo_escape_lexical_is_invalid_input(self):
        code, out, err = run_gate("fixture8a_repo_escape.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("EVIDENCE_PATH_ESCAPES_REPO:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_escaped_path_never_reported_as_existing(self):
        code, out, err = run_gate("fixture8a_repo_escape.json")
        plan_check = out.get("checked_evidence", {}).get("plan")
        if plan_check is not None:
            self.assertFalse(plan_check.get("exists"), msg=f"out={out}")

    def test_capacity_gate_repo_escape_is_invalid_input_not_pre_runtime(self):
        """CAPACITY_PATH_ESCAPE_MISCLASSIFIED fix: capacity_rejection.gate_evidence escape must be
        exit 2 with an explicit escape code, never silently downgraded to PRE_RUNTIME_NOT_HEALTHY
        exit 1."""
        code, out, err = run_gate("fixture31_capacity_gate_repo_escape.json")
        self.assertEqual(code, 2, msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertTrue(any("ESCAPES_REPO" in c and "gate_evidence" in c for c in codes), msg=f"out={out}")
        self.assertNotIn("PRE_RUNTIME_NOT_HEALTHY", codes, msg=f"out={out}")
        self.assertIsNone(out.get("state"))


class TestHardenedTypeChecks(unittest.TestCase):
    def test_directory_where_file_expected_is_rejected(self):
        code, out, err = run_gate("fixture47_evidence_is_directory_not_file.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        plan_check = out.get("checked_evidence", {}).get("plan")
        self.assertFalse(plan_check.get("exists"), msg=f"out={out}")
        self.assertEqual(plan_check.get("type"), "dir", msg=f"out={out}")
        self.assertIn("EVIDENCE_MISSING:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_file_where_directory_expected_is_rejected(self):
        code, out, err = run_gate("fixture48_simlog_is_file_not_directory.json")
        simlog_check = out.get("checked_evidence", {}).get("simlog")
        self.assertFalse(simlog_check.get("exists"), msg=f"out={out}")
        self.assertEqual(simlog_check.get("type"), "file", msg=f"out={out}")

    def test_empty_file_is_rejected(self):
        code, out, err = run_gate("fixture49_evidence_empty_file.json")
        plan_check = out.get("checked_evidence", {}).get("plan")
        self.assertFalse(plan_check.get("exists"), msg=f"out={out}")
        self.assertIn("EVIDENCE_MISSING:plan", out.get("reason_codes", []), msg=f"out={out}")


class TestPiiCoverageBinding(unittest.TestCase):
    """C3: pii_scan.passed=true alone is insufficient -- scanned_paths must cover every required
    evidence path for the achieved state."""

    def test_incomplete_coverage_blocks_evidence_complete(self):
        code, out, err = run_gate("fixture51_pii_coverage_incomplete.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertTrue(any(c.startswith("PII_SCAN_COVERAGE_INCOMPLETE:") for c in codes), msg=f"out={out}")


class _ScratchTestCase(unittest.TestCase):
    """Base for tests that build temporary repo-contained scratch files/symlinks/FIFOs at run
    time (hermetic design -- see module docstring). tearDown removes both the per-test scratch
    subdirectory AND the shared _runtime_scratch parent once it is empty, so a full suite run
    leaves zero filesystem residue -- not just the per-test leaf directory (reviewer suggestion:
    "remove the empty _runtime_scratch parent... so the suite leaves no ignored filesystem
    residue")."""

    def setUp(self):
        SCRATCH_DIR.mkdir(exist_ok=True)
        self.scratch = Path(tempfile.mkdtemp(dir=SCRATCH_DIR))
        self.external = Path(tempfile.mkdtemp())  # genuinely outside the repo

    def tearDown(self):
        shutil.rmtree(self.scratch, ignore_errors=True)
        shutil.rmtree(self.external, ignore_errors=True)
        try:
            SCRATCH_DIR.rmdir()
        except OSError:
            pass  # not empty (another scratch-using test is mid-run) or already gone -- fine either way


class TestHermeticSymlinkAndFifoRejection(_ScratchTestCase):
    """Hermetic fix (E2): builds its own external target + symlink/FIFO inside a temporary,
    repo-contained scratch dir at test run time -- no committed symlink, no fixed /tmp dependency.
    Cleans up reliably in tearDown even on assertion failure."""

    def _write_manifest(self, evidence_rel_path: str) -> Path:
        # FIFO_TEST_VACUITY fix: runtime-valid (functional_smoke_passed=true + runtime.identity
        # matching the top-level identity on all 6 strong fields) so the gate actually reaches the
        # evidence tier for every test in this class, rather than exiting early at the runtime
        # tier for an unrelated reason (RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED / RUNTIME_IDENTITY_
        # MISSING) that would make downstream assertions vacuously true. The symlink tests in this
        # class are unaffected (they already exit at the escape/symlink pre-runtime-tier check,
        # before the runtime tier is even evaluated).
        identity = {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None, "topology": "single", "tp": 1}
        manifest = {
            "schema_version": 1, "task_class": "model_serving_strategy",
            "identity": identity,
            "runtime": {"health_ok": True, "functional_smoke_passed": True, "identity": identity, "containers": []},
            "evidence": {"plan": {"path": evidence_rel_path}, "devlog": None, "testlog": None,
                         "simlog": None, "bench_report": None, "certificate": None},
            "pii_scan": {"passed": True},
        }
        manifest_path = self.scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return manifest_path

    def test_symlink_pointing_outside_repo_is_rejected(self):
        target = self.external / "secret_outside_repo.txt"
        target.write_text("should never be exposed", encoding="utf-8")
        link = self.scratch / "escape_link.txt"
        os.symlink(target, link)
        manifest_path = self._write_manifest("escape_link.txt")

        code, out, err = run_gate_path(manifest_path)
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("EVIDENCE_PATH_CONTAINS_SYMLINK:plan", out.get("reason_codes", []), msg=f"out={out}")
        plan_check = out.get("checked_evidence", {}).get("plan")
        if plan_check is not None:
            self.assertFalse(plan_check.get("exists"), msg=f"out={out}")

    def test_symlink_pointing_inside_repo_is_still_rejected(self):
        """C1: ANY symlink is rejected, not just ones that happen to escape -- eliminates the
        TOCTOU resolve-then-recheck window by construction (O_NOFOLLOW never follows at all)."""
        link = self.scratch / "internal_link.txt"
        os.symlink(FIXTURES_DIR / "targets" / "plan.md", link)
        manifest_path = self._write_manifest("internal_link.txt")

        code, out, err = run_gate_path(manifest_path)
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIn("EVIDENCE_PATH_CONTAINS_SYMLINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_capacity_gate_symlink_is_also_rejected(self):
        target = self.external / "secret.log"
        target.write_text("x", encoding="utf-8")
        link = self.scratch / "gate_link.txt"
        os.symlink(target, link)
        manifest = {
            "schema_version": 1, "task_class": "capacity_rejection",
            "identity": {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None, "topology": "single", "tp": 1},
            "capacity_rejection": {"rejected": True, "gate_evidence": "gate_link.txt"},
            "evidence": {"plan": {"path": "dummy.md"}, "devlog": {"path": "dummy.md"}, "testlog": None,
                         "simlog": None, "bench_report": None, "certificate": None},
            "pii_scan": {"passed": True},
        }
        (self.scratch / "dummy.md").write_text("x", encoding="utf-8")
        manifest_path = self.scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        code, out, err = run_gate_path(manifest_path)
        self.assertEqual(code, 2, msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertTrue(any("CONTAINS_SYMLINK" in c and "gate_evidence" in c for c in codes), msg=f"out={out}")

    @unittest.skipUnless(hasattr(os, "mkfifo"), "os.mkfifo unavailable on this platform")
    def test_fifo_evidence_is_rejected_without_hanging(self):
        """FIFO_TEST_VACUITY fix: the manifest is runtime-valid (see _write_manifest), so the gate
        genuinely reaches the evidence tier and classifies the FIFO there via the hardened
        resolver -- these assertions are unconditional (no `if x is not None:` escape hatch) and
        pin the EXACT stable status/reason, proving this is not an unrelated pre-runtime early
        exit."""
        fifo_path = self.scratch / "evidence.fifo"
        os.mkfifo(fifo_path)
        manifest_path = self._write_manifest("evidence.fifo")

        code, out, err = run_gate_path(manifest_path)  # must never hang (subprocess timeout would raise)
        codes = out.get("reason_codes", [])
        self.assertNotIn("RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED", codes, msg=f"out={out}")
        self.assertNotIn("RUNTIME_IDENTITY_MISSING", codes, msg=f"out={out}")
        self.assertFalse(any(c.startswith("RUNTIME_IDENTITY_MISMATCH") for c in codes), msg=f"out={out}")
        self.assertEqual(out.get("state"), "runtime-ready", msg=f"out={out} -- proves runtime tier passed")
        plan_check = out.get("checked_evidence", {}).get("plan")
        self.assertIsNotNone(plan_check, msg=f"out={out} -- evidence tier must have been reached (not vacuous)")
        self.assertFalse(plan_check.get("exists"), msg=f"out={out}")
        self.assertIsNone(plan_check.get("type"), msg=f"out={out} -- FIFO is neither 'file' nor 'dir'")
        self.assertIn("EVIDENCE_MISSING:plan", codes, msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")


# =============================================================================
# Section D -- exact dynamic evidence matrix
# =============================================================================

class TestFixture2LiteOnlyBlocksPromotion(unittest.TestCase):
    def test_promotion_is_blocked(self):
        code, out, err = run_gate("fixture2_lite_only_blocks_promotion.json")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")

    def test_bench_report_reported_missing(self):
        code, out, err = run_gate("fixture2_lite_only_blocks_promotion.json")
        self.assertIn("EVIDENCE_MISSING:bench_report", out.get("reason_codes", []), msg=f"out={out}")

    def test_base_docs_not_reported_missing(self):
        code, out, err = run_gate("fixture2_lite_only_blocks_promotion.json")
        codes = out.get("reason_codes", [])
        for key in ("plan", "devlog", "testlog", "simlog"):
            self.assertNotIn(f"EVIDENCE_MISSING:{key}", codes, msg=f"out={out}")

    def test_state_not_promotion_ready(self):
        code, out, err = run_gate("fixture2_lite_only_blocks_promotion.json")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")


class TestHarnessChangeActualTrialCondition(unittest.TestCase):
    def test_actual_trial_true_requires_simlog(self):
        code, out, err = run_gate("fixture40_harness_change_actual_trial_requires_simlog.json")
        self.assertIn("EVIDENCE_MISSING:simlog", out.get("reason_codes", []), msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")

    def test_no_trial_does_not_require_simlog(self):
        # harness_change is NOT in PROMOTION_CAPPED_TASK_CLASSES (only capacity_rejection/
        # minor_patch/read_only_audit are, per plan_26072506 D5/D6 explicit statement) -- it
        # naturally never reaches promotion here. Second review cycle (PROMOTION_CERTIFICATE_
        # BYPASS fix) additionally restricts promotion to task_class=="full_benchmark" outright,
        # checked before mode/verdict -- so harness_change is stopped by that class gate, not by
        # BENCHMARK_MODE_NOT_FULL (this fixture doesn't even set a benchmark block).
        code, out, err = run_gate("fixture41_harness_change_no_trial_ok.json")
        self.assertNotIn("EVIDENCE_MISSING:simlog", out.get("reason_codes", []), msg=f"out={out}")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertIn("PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS", out.get("reason_codes", []), msg=f"out={out}")


class TestMinorPatchRequiresVerificationAndCommit(unittest.TestCase):
    def test_missing_both_blocks_evidence_complete(self):
        code, out, err = run_gate("fixture42_minor_patch_requires_verification_and_commit.json")
        codes = out.get("reason_codes", [])
        self.assertIn("EVIDENCE_MISSING:verification", codes, msg=f"out={out}")
        self.assertIn("EVIDENCE_MISSING:commit", codes, msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")

    def test_complete_reaches_evidence_complete_capped(self):
        code, out, err = run_gate("fixture43_minor_patch_complete.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertIn("TASK_CLASS_CAPS_BELOW_PROMOTION:minor_patch", out.get("reason_codes", []), msg=f"out={out}")


class TestReadOnlyAuditReportRequestedCondition(unittest.TestCase):
    def test_report_requested_true_requires_report(self):
        code, out, err = run_gate("fixture44_read_only_audit_report_requested.json")
        self.assertIn("EVIDENCE_MISSING:report", out.get("reason_codes", []), msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")

    def test_no_report_requested_empty_evidence_still_terminates(self):
        code, out, err = run_gate("fixture45_read_only_audit_empty_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")


class TestFullBenchmarkFailPath(unittest.TestCase):
    def test_fail_verdict_certificate_absent_still_evidence_complete(self):
        code, out, err = run_gate("fixture46_full_benchmark_fail_certificate_absent_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotIn("EVIDENCE_MISSING:certificate", out.get("reason_codes", []), msg=f"out={out}")

    def test_fail_verdict_never_promotion_ready(self):
        code, out, err = run_gate("fixture46_full_benchmark_fail_certificate_absent_ok.json")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")


# =============================================================================
# Section G -- second independent-review cycle (subagent-summary-0-20260725_083608_365097)
# =============================================================================

class TestPromotionRestrictedToFullBenchmarkClass(unittest.TestCase):
    """PROMOTION_CERTIFICATE_BYPASS: promotion-ready is modeled EXCLUSIVELY by task_class ==
    'full_benchmark'. Other non-capped classes must never reach promotion-ready even when they
    self-declare benchmark.mode=full/verdict=PASS with no certificate/bench_report -- this is the
    exact dynamic_evidence the reviewer reproduced against the prior implementation."""

    def test_model_serving_strategy_full_pass_self_declaration_never_promotes(self):
        code, out, err = run_gate("fixture57_model_serving_strategy_promotion_bypass_blocked.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")
        self.assertIn("PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS", out.get("reason_codes", []), msg=f"out={out}")

    def test_harness_change_full_pass_self_declaration_never_promotes(self):
        code, out, err = run_gate("fixture58_harness_change_promotion_bypass_blocked.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")
        self.assertIn("PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS", out.get("reason_codes", []), msg=f"out={out}")

    def test_full_benchmark_class_is_unaffected(self):
        """Regression guard: the new gate must not accidentally block the legitimate class."""
        code, out, err = run_gate("fixture6_full_pass_promotion_ready.json")
        self.assertEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertTrue(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotIn("PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS", out.get("reason_codes", []), msg=f"out={out}")


class TestRequiredLinkValidation(unittest.TestCase):
    """REQUIRED_LINK_VALIDATION_ABSENT: deterministic local-relative-link validation for every
    required Markdown-like evidence file (plan/devlog/testlog/bench_report/report/verification/
    commit). http(s)/mailto/same-document-anchor targets are ignored; local targets must resolve
    (repo-contained, no symlink) and exist; absolute paths and malformed percent-encoding are
    rejected before any filesystem interaction."""

    def test_valid_links_do_not_block(self):
        """Positive control: external/anchor/mailto ignored, plain + percent-encoded + reference-
        style local links all resolve -- must reach evidence-complete cleanly."""
        code, out, err = run_gate("fixture52_link_validation_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              or c.startswith("EVIDENCE_LINK_SCAN_FAILED") for c in codes), msg=f"out={out}")

    def test_reviewer_repro_broken_inline_link_blocks(self):
        """Exact reviewer reproduction: `[broken](definitely-does-not-exist.md)`."""
        code, out, err = run_gate("fixture53_link_validation_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_absolute_path_link_rejected(self):
        code, out, err = run_gate("fixture54_link_validation_absolute.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_INVALID_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_broken_reference_definition_target_blocks(self):
        code, out, err = run_gate("fixture55_link_validation_refdef_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_malformed_percent_encoding_rejected(self):
        code, out, err = run_gate("fixture61_link_validation_malformed_percent.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_INVALID_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_broken_link_blocks_full_benchmark_promotion(self):
        """Exact reviewer reproduction at the promotion tier: a full-PASS bundle whose plan
        contains a broken link must not reach promotion-ready (was: exit 0, eligible=true)."""
        code, out, err = run_gate("fixture56_full_benchmark_broken_link_blocks_promotion.json")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")


class TestRequiredLinkValidationHermeticEscapes(_ScratchTestCase):
    """Link targets must be resolved through the SAME hardened, symlink-rejecting, repo-escape-
    rejecting mechanism as evidence paths -- not a second, weaker check. Built dynamically (no
    committed symlink/escape fixture), matching the hermetic design used elsewhere in this suite."""

    def _manifest_with_plan_only(self, plan_path_str: str) -> Path:
        ident = {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None, "topology": "single", "tp": 1}
        manifest = {
            "schema_version": 1, "task_class": "model_serving_strategy", "identity": ident,
            "runtime": {"health_ok": True, "functional_smoke_passed": True, "identity": ident, "containers": []},
            "evidence": {"plan": {"path": plan_path_str}, "devlog": None, "testlog": None, "simlog": None,
                         "bench_report": None, "certificate": None},
            "pii_scan": {"passed": True, "scanned_paths": [plan_path_str]},
        }
        manifest_path = self.scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return manifest_path

    def test_link_through_symlink_is_rejected(self):
        target = self.external / "secret.md"
        target.write_text("secret content", encoding="utf-8")
        link = self.scratch / "linked_via_symlink.md"
        os.symlink(target, link)
        plan_path = self.scratch / "plan.md"
        plan_path.write_text("[x](linked_via_symlink.md)\n", encoding="utf-8")

        manifest_path = self._manifest_with_plan_only("plan.md")
        code, out, err = run_gate_path(manifest_path)
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_link_escaping_repo_root_is_rejected(self):
        plan_path = self.scratch / "plan.md"
        plan_path.write_text("[x](../../../../../../../../etc/passwd)\n", encoding="utf-8")

        manifest_path = self._manifest_with_plan_only("plan.md")
        code, out, err = run_gate_path(manifest_path)
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")


class TestNonUtf8ManifestHandling(_ScratchTestCase):
    """NON_UTF8_MANIFEST_BREAKS_STABLE_JSON: an invalid-UTF-8 manifest must still produce exactly
    one stable completion-manifest JSON document on stdout (state null, exit 2), never a
    traceback. Built at test run time -- raw invalid bytes are not committed as a fixture file."""

    def test_invalid_utf8_manifest_is_stable_invalid_input(self):
        manifest_path = self.scratch / "invalid_utf8_manifest.json"
        # 0xff is not a valid UTF-8 lead byte in any position -- guaranteed UnicodeDecodeError.
        manifest_path.write_bytes(b'{"schema_version": 1, "bad": "\xff\xfe"}')

        code, out, err = run_gate_path(manifest_path)
        self.assertEqual(code, 2, msg=f"stderr={err}\nout={out}")
        self.assertIn("MANIFEST_INVALID_UTF8", out.get("reason_codes", []), msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"))
        self.assertNotIn("Traceback", err, msg=f"stderr should not contain a Python traceback: {err}")


class TestCliUsageErrorsEmitStableJson(unittest.TestCase):
    """Small contract fix: argparse usage errors (missing subcommand, missing --manifest, unknown
    argument) must honor the same stable-JSON-on-stdout / exit-2 contract as manifest/input
    errors, not prose-to-stderr with empty stdout."""

    def _run_raw(self, args: list[str]) -> tuple[int, str, str]:
        proc = subprocess.run([sys.executable, str(GATE_SCRIPT), *args],
                               cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=30)
        return proc.returncode, proc.stdout, proc.stderr

    def test_missing_subcommand(self):
        code, stdout, stderr = self._run_raw([])
        self.assertEqual(code, 2, msg=f"stdout={stdout}\nstderr={stderr}")
        parsed = json.loads(stdout)  # raises if not valid JSON -- itself part of the assertion
        self.assertIn("CLI_USAGE_ERROR", parsed.get("reason_codes", []), msg=f"out={parsed}")
        self.assertIsNone(parsed.get("state"))

    def test_missing_manifest_flag(self):
        code, stdout, stderr = self._run_raw(["verify"])
        self.assertEqual(code, 2, msg=f"stdout={stdout}\nstderr={stderr}")
        parsed = json.loads(stdout)
        self.assertIn("CLI_USAGE_ERROR", parsed.get("reason_codes", []), msg=f"out={parsed}")

    def test_unknown_argument(self):
        code, stdout, stderr = self._run_raw(["verify", "--manifest", "x.json", "--totally-bogus-flag"])
        self.assertEqual(code, 2, msg=f"stdout={stdout}\nstderr={stderr}")
        parsed = json.loads(stdout)
        self.assertIn("CLI_USAGE_ERROR", parsed.get("reason_codes", []), msg=f"out={parsed}")

    def test_process_exit_code_equals_json_exit_code_field(self):
        code, stdout, stderr = self._run_raw([])
        parsed = json.loads(stdout)
        self.assertEqual(code, parsed.get("exit_code"), msg=f"out={parsed}")


# =============================================================================
# Section F -- state contract: null is a sentinel, never a 4th named state
# =============================================================================

class TestStateVocabularyIsExactlyThreeNamedPlusNullSentinel(unittest.TestCase):
    NAMED_STATES = {"runtime-ready", "evidence-complete", "promotion-ready"}

    def test_every_fixture_state_is_named_or_null(self):
        for fixture_path in sorted(FIXTURES_DIR.glob("fixture*.json")):
            with self.subTest(fixture=fixture_path.name):
                code, out, err = run_gate(fixture_path.name)
                state = out.get("state")
                self.assertTrue(state is None or state in self.NAMED_STATES,
                                 msg=f"unexpected state token {state!r} in out={out}")


# =============================================================================
# Section H -- third independent-review cycle (subagent-summary-1-20260725_085654_777234)
# =============================================================================

class TestEmptySimlogPromotionBypass(_ScratchTestCase):
    """EMPTY_SIMLOG_PROMOTION_BYPASS: a required simlog directory must contain at least one
    non-empty regular raw-evidence file (checked recursively through the same hardened dir_fd
    walk used to compute its tree fingerprint) -- an empty directory, or a tree containing only
    zero-byte files, must fail evidence-completeness with a stable reason, never silently
    promote. Fully self-contained at test run time: git cannot track an empty directory, so the
    negative cases cannot be committed fixtures."""

    _IDENTITY = {"model": "gate-simlog-test", "gpu": "GB10", "vllm": "0.22.0",
                 "quant": "bf16", "topology": "single", "tp": 1}

    def _write_bundle(self, simlog_setup) -> Path:
        (self.scratch / "plan.md").write_text("# Plan\n\nNo links.\n", encoding="utf-8")
        (self.scratch / "devlog.md").write_text("# Devlog\n\nNo links.\n", encoding="utf-8")
        (self.scratch / "testlog.md").write_text("# Testlog\n\nNo links.\n", encoding="utf-8")
        (self.scratch / "bench_report.md").write_text("# Bench report\n\nNo links.\n", encoding="utf-8")
        (self.scratch / "certificate.yaml").write_text(
            "schema_version: 1\n"
            "record_type: benchmark_certificate\n"
            "verdict: PASS\n"
            f"model: {self._IDENTITY['model']}\n"
            f"gpu_model: {self._IDENTITY['gpu']}\n"
            f"vllm_version: {self._IDENTITY['vllm']}\n"
            f"quantization: {self._IDENTITY['quant']}\n"
            f"topology: {self._IDENTITY['topology']}\n"
            f"tensor_parallel_size: {self._IDENTITY['tp']}\n"
            "benchmark_mode: full\n",
            encoding="utf-8",
        )
        simlog_dir = self.scratch / "simlog"
        simlog_dir.mkdir()
        simlog_setup(simlog_dir)

        manifest = {
            "schema_version": 1, "task_class": "full_benchmark", "identity": self._IDENTITY,
            "runtime": {"health_ok": True, "functional_smoke_passed": True,
                        "identity": self._IDENTITY, "containers": []},
            "evidence": {
                "plan": {"path": "plan.md"}, "devlog": {"path": "devlog.md"},
                "testlog": {"path": "testlog.md"}, "simlog": {"path": "simlog"},
                "bench_report": {"path": "bench_report.md"}, "certificate": {"path": "certificate.yaml"},
            },
            "benchmark": {"mode": "full", "verdict": "PASS"},
            "pii_scan": {"passed": True, "scanned_paths": [
                "plan.md", "devlog.md", "testlog.md", "simlog", "bench_report.md", "certificate.yaml",
            ]},
        }
        manifest_path = self.scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return manifest_path

    def test_empty_simlog_directory_blocks_promotion(self):
        """Exact reviewer reproduction: a full-PASS bundle mutated to use a zero-entry simlog
        directory must not reach evidence-complete/promotion-ready."""
        manifest_path = self._write_bundle(lambda d: None)
        code, out, err = run_gate_path(manifest_path)
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")
        self.assertIn("EVIDENCE_MISSING:simlog", out.get("reason_codes", []), msg=f"out={out}")
        simlog_check = out.get("checked_evidence", {}).get("simlog")
        self.assertIsNotNone(simlog_check, msg=f"out={out}")
        self.assertFalse(simlog_check.get("exists"), msg=f"out={out}")
        self.assertEqual(simlog_check.get("type"), "dir", msg=f"out={out}")

    def test_simlog_tree_with_only_empty_files_blocks_promotion(self):
        def setup(d):
            (d / "empty1.txt").write_text("", encoding="utf-8")
            sub = d / "sub"
            sub.mkdir()
            (sub / "empty2.txt").write_text("", encoding="utf-8")
        manifest_path = self._write_bundle(setup)
        code, out, err = run_gate_path(manifest_path)
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertIn("EVIDENCE_MISSING:simlog", out.get("reason_codes", []), msg=f"out={out}")

    def test_simlog_tree_with_nested_nonempty_file_promotes(self):
        """Positive regression: recursion must still find a non-empty file nested inside a
        subdirectory (alongside an unrelated empty top-level file) and reach promotion-ready --
        proving the fix inspects the whole tree, not just top-level entries."""
        def setup(d):
            (d / "empty_top.txt").write_text("", encoding="utf-8")
            sub = d / "sub"
            sub.mkdir()
            (sub / "raw_evidence.log").write_text("real content\n", encoding="utf-8")
        manifest_path = self._write_bundle(setup)
        code, out, err = run_gate_path(manifest_path)
        self.assertEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertTrue(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertEqual(code, 0, msg=f"out={out}")
        simlog_check = out.get("checked_evidence", {}).get("simlog")
        self.assertTrue(simlog_check.get("exists"), msg=f"out={out}")
        self.assertIsInstance(simlog_check.get("tree_sha256"), str, msg=f"out={out}")


class TestMarkdownAngleDestinationLinkBypass(unittest.TestCase):
    """MARKDOWN_ANGLE_DESTINATION_LINK_BYPASS: the deterministic Markdown link scanner must
    correctly parse angle-bracket destinations containing spaces, balanced nested parentheses in
    ordinary (non-angle) destinations, backslash-escaped characters inside a destination, and both
    reference-style definitions and reference-style USES (`[text][label]`) -- not just the plain
    inline `[text](target)` form the prior regex covered."""

    def test_reviewer_exact_repro_angle_destination_with_space_blocks_evidence_complete(self):
        code, out, err = run_gate("fixture62_link_validation_angle_destination_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_reviewer_exact_repro_angle_destination_blocks_promotion(self):
        """The exact reviewer reproduction at the promotion tier: `[missing](<definitely
        missing.md>)` in a full-PASS bundle must not reach promotion-ready (was: exit 0,
        eligible=true, no link failure)."""
        code, out, err = run_gate("fixture63_full_benchmark_angle_destination_link_blocks_promotion.json")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_nested_parentheses_destination_resolves_when_target_exists(self):
        """`[nested](file(1).md)` -- a literal, unescaped, BALANCED pair of parentheses inside a
        non-angle destination must be treated as part of the destination, not as the link's own
        closing delimiter. targets/file(1).md genuinely exists."""
        code, out, err = run_gate("fixture64_link_validation_nested_parens_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_nested_parentheses_destination_still_detects_broken_target(self):
        code, out, err = run_gate("fixture65_link_validation_nested_parens_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_escaped_paren_in_destination_resolves_existing_target(self):
        """`[esc](weird\\)file.md)` -- the backslash-escaped `)` must be treated as a literal
        character of the destination, not the link's closing delimiter. targets/weird)file.md
        (a real filename containing a literal ')') genuinely exists."""
        code, out, err = run_gate("fixture66_link_validation_escaped_paren_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_escaped_paren_in_destination_still_detects_broken_target(self):
        code, out, err = run_gate("fixture67_link_validation_escaped_paren_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_reference_use_resolves_through_its_definition(self):
        """`[the doc][refdoc]` + `[refdoc]: devlog.md` elsewhere in the same document -- a
        reference-style USE (not just a reference-style DEFINITION) must resolve through its
        matching definition and validate the definition's target."""
        code, out, err = run_gate("fixture68_link_validation_reference_use_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_unresolved_reference_use_fails_closed_not_silently_skipped(self):
        """`[the doc][nosuchlabel]` with NO matching `[nosuchlabel]: ...` definition anywhere --
        unsupported/malformed link-like syntax must fail closed, never be silently ignored as if
        it were plain non-link text."""
        code, out, err = run_gate("fixture69_link_validation_reference_use_unresolved.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_INVALID_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")


class TestAbsoluteEvidencePathsRejected(_ScratchTestCase):
    """ABSOLUTE_EVIDENCE_PATHS_ACCEPTED: work-manifest.schema.json documents every evidence path,
    capacity_rejection.gate_evidence, and pii_scan.scanned_paths entry as relative to the
    manifest's own directory. An absolute path that happens to still resolve inside the repo must
    not silently work -- it must fail closed with a stable reason code, checked on the raw string
    BEFORE any join/normalization. Built at test run time: absolute paths are inherently
    environment-dependent (they embed this clone's own filesystem location), so this cannot be a
    static committed fixture -- mirrors the review's exact 'mutate every promotion fixture path to
    absolute' reproduction."""

    _IDENTITY = {"model": "solar-open2-250b", "gpu": "GB10", "vllm": "0.22.0",
                 "quant": "NVFP4", "topology": "multi", "tp": 2}

    def _full_pass_manifest_with_absolute_evidence(self):
        t = lambda name: str(FIXTURES_DIR / "targets" / name)
        return {
            "schema_version": 1, "task_class": "full_benchmark", "identity": self._IDENTITY,
            "runtime": {
                "health_ok": True, "functional_smoke_passed": True, "identity": self._IDENTITY,
                "containers": [{"name": "mn-solar-open2-250b-master", "restart_count": 0, "oom_killed": False},
                               {"name": "vllm-slave-serve-container", "restart_count": 0, "oom_killed": False}],
            },
            "evidence": {
                "plan": {"path": t("plan.md")}, "devlog": {"path": t("devlog.md")},
                "testlog": {"path": t("testlog.md")}, "simlog": {"path": t("simlog_run")},
                "bench_report": {"path": t("bench_report.md")},
                "certificate": {"path": t("certificate_pass_full.yaml")},
            },
            "benchmark": {"mode": "full", "verdict": "PASS"},
            "pii_scan": {"passed": True, "scanned_paths": [
                t("plan.md"), t("devlog.md"), t("testlog.md"), t("simlog_run"),
                t("bench_report.md"), t("certificate_pass_full.yaml"),
            ]},
        }

    def _write_and_run(self, manifest: dict):
        manifest_path = self.scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return run_gate_path(manifest_path)

    def test_absolute_evidence_path_rejected(self):
        """Exact reviewer reproduction: a full-PASS bundle mutated so every evidence.*.path is an
        absolute (but still in-repo) path must not promote -- must fail closed at exit 2."""
        manifest = self._full_pass_manifest_with_absolute_evidence()
        code, out, err = self._write_and_run(manifest)
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertTrue(any(c.startswith("ABSOLUTE_EVIDENCE_PATH:") for c in codes), msg=f"out={out}")

    def test_absolute_capacity_gate_evidence_path_rejected(self):
        manifest = {
            "schema_version": 1, "task_class": "capacity_rejection",
            "identity": {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None,
                         "topology": "single", "tp": 1},
            "capacity_rejection": {"rejected": True,
                                   "gate_evidence": str(FIXTURES_DIR / "targets" / "gate_evidence.txt")},
            "evidence": {"plan": {"path": str(FIXTURES_DIR / "targets" / "plan.md")}, "devlog": None,
                         "testlog": {"path": str(FIXTURES_DIR / "targets" / "testlog.md")},
                         "simlog": None, "bench_report": None, "certificate": None},
            "pii_scan": {"passed": True, "scanned_paths": [
                str(FIXTURES_DIR / "targets" / "plan.md"),
                str(FIXTURES_DIR / "targets" / "testlog.md"),
                str(FIXTURES_DIR / "targets" / "gate_evidence.txt"),
            ]},
        }
        code, out, err = self._write_and_run(manifest)
        self.assertEqual(code, 2, msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertIn("ABSOLUTE_CAPACITY_GATE_EVIDENCE_PATH", out.get("reason_codes", []), msg=f"out={out}")

    def test_absolute_pii_scanned_path_rejected(self):
        """Isolates the pii_scan.scanned_paths check: evidence.*.path stays relative (and
        resolves fine) so the ONLY contract violation is the scanned_paths list itself being
        absolute -- proving this is a distinct, independently-enforced check, not a side effect
        of the evidence-path check."""
        rel = lambda name: f"../../targets/{name}"
        abs_ = lambda name: str(FIXTURES_DIR / "targets" / name)
        manifest = {
            "schema_version": 1, "task_class": "full_benchmark", "identity": self._IDENTITY,
            "runtime": {
                "health_ok": True, "functional_smoke_passed": True, "identity": self._IDENTITY,
                "containers": [{"name": "mn-solar-open2-250b-master", "restart_count": 0, "oom_killed": False},
                               {"name": "vllm-slave-serve-container", "restart_count": 0, "oom_killed": False}],
            },
            "evidence": {
                "plan": {"path": rel("plan.md")}, "devlog": {"path": rel("devlog.md")},
                "testlog": {"path": rel("testlog.md")}, "simlog": {"path": rel("simlog_run")},
                "bench_report": {"path": rel("bench_report.md")},
                "certificate": {"path": rel("certificate_pass_full.yaml")},
            },
            "benchmark": {"mode": "full", "verdict": "PASS"},
            "pii_scan": {"passed": True, "scanned_paths": [
                abs_("plan.md"), abs_("devlog.md"), abs_("testlog.md"), abs_("simlog_run"),
                abs_("bench_report.md"), abs_("certificate_pass_full.yaml"),
            ]},
        }
        code, out, err = self._write_and_run(manifest)
        self.assertEqual(code, 2, msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertTrue(any(c.startswith("ABSOLUTE_PII_SCANNED_PATH:") for c in codes), msg=f"out={out}")
        self.assertFalse(any(c.startswith("ABSOLUTE_EVIDENCE_PATH:") for c in codes), msg=f"out={out}")
        self.assertFalse(any(c.startswith("EVIDENCE_PATH_ESCAPES_REPO") for c in codes), msg=f"out={out}")


class TestAmbiguousTreeFingerprintEncoding(_ScratchTestCase):
    """AMBIGUOUS_TREE_FINGERPRINT_ENCODING: reconstructs the reviewer's exact collision pair (two
    distinct directory trees whose OLD newline/space-joined text encoding produced the identical
    SHA-256 via plain delimiter injection -- no cryptographic hash break required) and asserts the
    fixed length-prefixed raw-byte framing tells them apart."""

    def _tree_hash_for(self, dir_path: Path) -> str:
        ident = {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None, "topology": "single", "tp": 1}
        manifest = {
            "schema_version": 1, "task_class": "model_serving_strategy", "identity": ident,
            "runtime": {"health_ok": True, "functional_smoke_passed": True, "identity": ident, "containers": []},
            "evidence": {"plan": None, "devlog": None, "testlog": None,
                         "simlog": {"path": dir_path.name}, "bench_report": None, "certificate": None},
            "pii_scan": {"passed": True, "scanned_paths": []},
        }
        manifest_path = dir_path.parent / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        code, out, err = run_gate_path(manifest_path)
        simlog_check = out.get("checked_evidence", {}).get("simlog")
        self.assertIsNotNone(simlog_check, msg=f"out={out}")
        tree_hash = simlog_check.get("tree_sha256")
        self.assertIsNotNone(tree_hash, msg=f"out={out}")
        return tree_hash

    def test_delimiter_injection_collision_pair_now_differs(self):
        tree1 = self.scratch / "tree1"
        tree1.mkdir()
        (tree1 / "a").write_text("content-a", encoding="utf-8")
        (tree1 / "b").write_text("content-b", encoding="utf-8")
        hash1 = self._tree_hash_for(tree1)

        digest_a = hashlib.sha256(b"content-a").hexdigest()
        # tree2: ONE file whose name is crafted to reproduce the OLD "F <name> <digest>" text,
        # joined by "\n", exactly -- via a name containing a literal embedded newline + "F b" +
        # digest_a's own hex text -- with content identical to tree1's file 'b' (so its digest
        # equals digest_b too). No cryptographic collision needed: the old encoding was ambiguous
        # by construction (unescaped separators over attacker-controlled filenames).
        crafted_name = f"a {digest_a}\nF b"
        tree2 = self.scratch / "tree2"
        tree2.mkdir()
        (tree2 / crafted_name).write_text("content-b", encoding="utf-8")
        hash2 = self._tree_hash_for(tree2)

        self.assertNotEqual(hash1, hash2,
                            msg="tree fingerprint encoding is still ambiguous -- delimiter injection collision reproduced")


class TestOutputSchemaInvariantEnforced(unittest.TestCase):
    """OUTPUT_SCHEMA_INVARIANT: completion-manifest.schema.json now machine-enforces `state ==
    'promotion-ready' iff eligible_for_promotion == true` via Draft-07 if/then/allOf, and the
    generic validator (validate_against_schema) now implements those keywords. Tested directly
    against the schema-driven validator (white-box import, not through the CLI) because the
    gate's own business logic never legitimately produces a contradictory output to drive one
    through the CLI -- proving the SCHEMA ITSELF rejects the contradiction, independent of
    whether the current implementation happens to maintain the invariant."""

    def _base_valid_output(self, **overrides):
        out = {
            "schema_version": 1, "task_class": "full_benchmark", "state": "promotion-ready",
            "eligible_for_promotion": True, "reason_codes": [], "checked_evidence": {},
            "identity": {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None,
                         "topology": "single", "tp": 1},
            "certificate": None, "exit_code": 0,
        }
        out.update(overrides)
        return out

    def test_state_promotion_ready_without_eligible_true_is_rejected(self):
        bad = self._base_valid_output(eligible_for_promotion=False, exit_code=1)
        violations = _gate.validate_against_schema(bad, _gate.COMPLETION_SCHEMA)
        self.assertTrue(violations,
                        msg="schema failed to reject state=promotion-ready with eligible_for_promotion=false")

    def test_eligible_true_without_state_promotion_ready_is_rejected(self):
        bad = self._base_valid_output(state="runtime-ready", eligible_for_promotion=True, exit_code=1)
        violations = _gate.validate_against_schema(bad, _gate.COMPLETION_SCHEMA)
        self.assertTrue(violations,
                        msg="schema failed to reject eligible_for_promotion=true with state != promotion-ready")

    def test_consistent_promotion_ready_output_still_validates_cleanly(self):
        good = self._base_valid_output()
        violations = _gate.validate_against_schema(good, _gate.COMPLETION_SCHEMA)
        self.assertEqual(violations, [], msg=f"violations={violations}")

    def test_consistent_non_promotion_output_still_validates_cleanly(self):
        good = self._base_valid_output(state="runtime-ready", eligible_for_promotion=False, exit_code=1)
        violations = _gate.validate_against_schema(good, _gate.COMPLETION_SCHEMA)
        self.assertEqual(violations, [], msg=f"violations={violations}")

    def test_null_state_with_false_eligible_still_validates_cleanly(self):
        """Regression guard: the pre-lifecycle sentinel (state=null, exit_code 1 or 2) must not be
        accidentally caught by the new if/then rules."""
        good = self._base_valid_output(state=None, eligible_for_promotion=False, exit_code=2)
        violations = _gate.validate_against_schema(good, _gate.COMPLETION_SCHEMA)
        self.assertEqual(violations, [], msg=f"violations={violations}")


# =============================================================================
# Section I -- fourth independent-review cycle (subagent-summary-1-20260725_093053_672255)
# =============================================================================

class TestMarkdownFenceCommonmarkDivergence(unittest.TestCase):
    """MARKDOWN_FENCE_COMMONMARK_DIVERGENCE: fenced-code recognition must match CommonMark: an
    opening fence is 0-3 leading spaces + a run of >=3 identical backticks/tildes, and (backtick
    fences only) the info string must not itself contain a backtick; a closing fence is 0-3
    leading spaces + a run of the SAME character >= the opener's length, with only whitespace
    trailing. A line that merely LOOKS like a closing run but has non-whitespace trailing content
    (e.g. ```not-a-close) is ordinary fence CONTENT, not a close -- verified against the
    markdown_it (commonmark preset) reference renderer as an oracle before being hand-encoded
    here (never imported by the production gate)."""

    def test_reviewer_repro_not_a_close_content_preserved_as_code_not_scanned(self):
        """```not-a-close inside an open fence must not end it -- the broken-link-looking text
        one line later stays inside the code block and must never be scanned."""
        code, out, err = run_gate("fixture70_fence_not_a_close_content_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_reviewer_repro_invalid_backtick_info_does_not_hide_real_broken_link(self):
        """A backtick-fence opener whose info string itself contains a backtick is not a valid
        CommonMark fence opener at all -- the line is ordinary text, and a genuine broken link
        appearing afterward must still be detected, not blanked away as if it were code."""
        code, out, err = run_gate("fixture71_fence_invalid_backtick_info_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_tilde_fence_control_blanks_broken_looking_content(self):
        code, out, err = run_gate("fixture72_fence_tilde_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_proper_backtick_fence_control_blanks_broken_looking_content(self):
        code, out, err = run_gate("fixture73_fence_proper_backtick_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_positive_promotion_probe_not_a_close_content_reaches_promotion_ready(self):
        code, out, err = run_gate("fixture74_full_benchmark_fence_not_a_close_promotion_ok.json")
        self.assertEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertTrue(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertEqual(code, 0, msg=f"out={out}")

    def test_negative_promotion_probe_invalid_backtick_info_blocks_promotion(self):
        code, out, err = run_gate("fixture75_full_benchmark_fence_invalid_info_blocks_promotion.json")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")


class TestMarkdownShortcutReferenceFalseNegative(unittest.TestCase):
    """MARKDOWN_SHORTCUT_REFERENCE_FALSE_NEGATIVE: a shortcut reference use `[label]` (or image
    `![label]`), with no following `(...)`/`[...]`, must be recognized and validated when a
    matching normalized reference definition exists elsewhere in the document -- verified against
    markdown_it (commonmark preset) as an oracle. Ordinary bracketed prose with NO matching
    definition must remain untouched (still out of scope, never flagged)."""

    def test_reviewer_exact_repro_missing_shortcut_reference_blocks_evidence_complete(self):
        """Exact reviewer reproduction: `[missing]` + `[missing]: definitely-missing.md`."""
        code, out, err = run_gate("fixture76_shortcut_reference_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_existing_target_shortcut_reference_resolves_cleanly(self):
        code, out, err = run_gate("fixture77_shortcut_reference_ok.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_ordinary_bracket_text_with_no_definition_is_preserved_not_flagged(self):
        code, out, err = run_gate("fixture78_shortcut_reference_no_definition_preserved.json")
        self.assertEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        codes = out.get("reason_codes", [])
        self.assertFalse(any(c.startswith("EVIDENCE_BROKEN_LINK") or c.startswith("EVIDENCE_INVALID_LINK")
                              for c in codes), msg=f"out={out}")

    def test_reviewer_exact_repro_at_promotion_tier_blocks_promotion(self):
        """The exact reviewer reproduction at the promotion tier: a full-PASS bundle containing
        `[missing]` + `[missing]: definitely-missing.md` must not reach promotion-ready (was:
        exit 0, promotion-ready, eligible=true, no reason codes)."""
        code, out, err = run_gate("fixture79_full_benchmark_shortcut_reference_blocks_promotion.json")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")

    def test_positive_promotion_probe_existing_target_shortcut_reaches_promotion_ready(self):
        code, out, err = run_gate("fixture80_full_benchmark_shortcut_reference_promotion_ok.json")
        self.assertEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertTrue(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertEqual(code, 0, msg=f"out={out}")

    def test_shortcut_image_reference_also_validated(self):
        """'and images where applicable' -- a bare image shortcut `![label]` with a matching
        reference definition must be validated exactly like a text shortcut reference."""
        code, out, err = run_gate("fixture81_shortcut_image_reference_broken.json")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")
        self.assertIn("EVIDENCE_BROKEN_LINK:plan", out.get("reason_codes", []), msg=f"out={out}")


class TestMarkdownMultiblankInlineFalsePositive(unittest.TestCase):
    """MARKDOWN_MULTIBLANK_INLINE_FALSE_POSITIVE: inline link destination/title whitespace must
    honor CommonMark's limit of at most one line ending between components (the 'spnl' rule) --
    crossing a blank line breaks block structure before any inline link could ever be parsed, per
    the markdown_it (commonmark preset) reference renderer used as an oracle. The exact
    `[x](\\n\\ndefinitely-missing.md\\n)` must never be validated as if 'definitely-missing.md'
    were a real, checked link destination (no EVIDENCE_BROKEN_LINK) -- it fails closed as
    malformed link-like syntax instead (EVIDENCE_INVALID_LINK), consistent with every other
    unparseable inline-link-tail case in this scanner (never silently reclassified as a valid,
    successfully-resolved link)."""

    def test_reviewer_exact_repro_is_not_scanned_as_a_broken_link(self):
        code, out, err = run_gate("fixture82_multiblank_inline_not_a_link.json")
        codes = out.get("reason_codes", [])
        self.assertNotIn("EVIDENCE_BROKEN_LINK:plan", codes, msg=f"out={out}")
        self.assertIn("EVIDENCE_INVALID_LINK:plan", codes, msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out}")

    def test_valid_single_newline_whitespace_and_title_forms_still_resolve(self):
        """Control: legitimate CommonMark multi-line link forms (a single line ending, optionally
        surrounded by spaces, between components) must still be recognized and validated -- the
        fix must not overcorrect and break real links that happen to wrap across one line."""
        code, out, err = run_gate("fixture83_multiblank_valid_whitespace_promotion_ok.json")
        self.assertEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertTrue(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertEqual(code, 0, msg=f"out={out}")

    def test_reviewer_exact_repro_at_promotion_tier_never_reports_false_broken_link(self):
        """The exact reviewer reproduction at the promotion tier: an otherwise valid full-PASS
        bundle containing `[x](\\n\\ndefinitely-missing.md\\n)` must not be downgraded via a
        FALSE EVIDENCE_BROKEN_LINK:plan (it still fails closed via EVIDENCE_INVALID_LINK:plan,
        matching every other malformed-inline-link-tail case -- never silently promoted either)."""
        code, out, err = run_gate("fixture84_full_benchmark_multiblank_not_a_link_no_false_broken.json")
        codes = out.get("reason_codes", [])
        self.assertNotIn("EVIDENCE_BROKEN_LINK:plan", codes, msg=f"out={out}")
        self.assertIn("EVIDENCE_INVALID_LINK:plan", codes, msg=f"out={out}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertNotEqual(code, 0, msg=f"out={out}")


class TestIncompleteAbsolutePathContract(_ScratchTestCase):
    """INCOMPLETE_ABSOLUTE_PATH_CONTRACT: _is_absolute_path_string recognized POSIX-leading-slash
    and Windows drive-absolute forms but not Windows UNC ('\\\\server\\share\\...') or Windows
    root-relative ('\\rooted\\...') forms -- both begin with a bare backslash, which the prior
    check never inspected at all. These paths were misclassified as ordinary missing evidence
    (EVIDENCE_MISSING, exit 1) instead of absolute-path invalid input (exit 2); the double-slash
    POSIX-style UNC form ('//server/share/...') is included as a continued-coverage control (it
    was already caught by the leading-'/' check). The absolute-path check runs on the raw string
    BEFORE any join/normalization or filesystem interaction, so none of these paths need to
    resolve to a real file."""

    _IDENTITY = {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None,
                 "topology": "single", "tp": 1}
    _NEW_ABSOLUTE_FORMS = [
        "\\\\server\\share\\plan.md",  # Windows UNC
        "\\rooted\\plan.md",           # Windows root-relative
        "//server/share/plan.md",      # POSIX-style double-slash UNC (continued-coverage control)
    ]

    def _write_and_run(self, manifest: dict):
        manifest_path = self.scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return run_gate_path(manifest_path)

    def test_evidence_path_unc_and_root_relative_forms_rejected(self):
        for bad_path in self._NEW_ABSOLUTE_FORMS:
            with self.subTest(bad_path=bad_path):
                manifest = {
                    "schema_version": 1, "task_class": "model_serving_strategy", "identity": self._IDENTITY,
                    "runtime": {"health_ok": True, "functional_smoke_passed": True,
                                "identity": self._IDENTITY, "containers": []},
                    "evidence": {"plan": {"path": bad_path}, "devlog": None, "testlog": None,
                                 "simlog": None, "bench_report": None, "certificate": None},
                    "pii_scan": {"passed": True, "scanned_paths": [bad_path]},
                }
                code, out, err = self._write_and_run(manifest)
                self.assertEqual(code, 2, msg=f"bad_path={bad_path!r} out={out}")
                self.assertIsNone(out.get("state"), msg=f"out={out}")
                self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
                codes = out.get("reason_codes", [])
                self.assertTrue(any(c.startswith("ABSOLUTE_EVIDENCE_PATH:") for c in codes), msg=f"out={out}")

    def test_capacity_gate_evidence_unc_and_root_relative_forms_rejected(self):
        for bad_path in self._NEW_ABSOLUTE_FORMS:
            with self.subTest(bad_path=bad_path):
                manifest = {
                    "schema_version": 1, "task_class": "capacity_rejection", "identity": self._IDENTITY,
                    "capacity_rejection": {"rejected": True, "gate_evidence": bad_path},
                    "evidence": {"plan": {"path": str(FIXTURES_DIR / "targets" / "plan.md")}, "devlog": None,
                                 "testlog": {"path": str(FIXTURES_DIR / "targets" / "testlog.md")},
                                 "simlog": None, "bench_report": None, "certificate": None},
                    "pii_scan": {"passed": True, "scanned_paths": [
                        str(FIXTURES_DIR / "targets" / "plan.md"),
                        str(FIXTURES_DIR / "targets" / "testlog.md"),
                        bad_path,
                    ]},
                }
                code, out, err = self._write_and_run(manifest)
                self.assertEqual(code, 2, msg=f"bad_path={bad_path!r} out={out}")
                self.assertIsNone(out.get("state"), msg=f"out={out}")
                self.assertIn("ABSOLUTE_CAPACITY_GATE_EVIDENCE_PATH", out.get("reason_codes", []), msg=f"out={out}")

    def test_pii_scanned_path_unc_and_root_relative_forms_rejected(self):
        rel = lambda name: f"../../targets/{name}"
        for bad_path in self._NEW_ABSOLUTE_FORMS:
            with self.subTest(bad_path=bad_path):
                manifest = {
                    "schema_version": 1, "task_class": "model_serving_strategy", "identity": self._IDENTITY,
                    "runtime": {"health_ok": True, "functional_smoke_passed": True,
                                "identity": self._IDENTITY, "containers": []},
                    "evidence": {"plan": {"path": rel("plan.md")}, "devlog": None, "testlog": None,
                                 "simlog": None, "bench_report": None, "certificate": None},
                    "pii_scan": {"passed": True, "scanned_paths": [rel("plan.md"), bad_path]},
                }
                code, out, err = self._write_and_run(manifest)
                self.assertEqual(code, 2, msg=f"bad_path={bad_path!r} out={out}")
                codes = out.get("reason_codes", [])
                self.assertTrue(any(c.startswith("ABSOLUTE_PII_SCANNED_PATH:") for c in codes), msg=f"out={out}")
                self.assertFalse(any(c.startswith("ABSOLUTE_EVIDENCE_PATH:") for c in codes), msg=f"out={out}")


class TestOutputSchemaExitStateGap(unittest.TestCase):
    """OUTPUT_SCHEMA_EXIT_STATE_GAP: the state<->eligible_for_promotion biconditional alone still
    left impossible promotion/exit combinations schema-VALID (e.g. state=promotion-ready +
    eligible_for_promotion=true + exit_code=1; state=evidence-complete + eligible_for_promotion=
    false + exit_code=0). completion-manifest.schema.json now also machine-enforces exit_code's
    place in the same invariant: exit_code==0 iff state=='promotion-ready' (and therefore
    eligible_for_promotion==true); state in {runtime-ready, evidence-complete} implies exit_code
    ==1 and eligible_for_promotion==false; state==null implies eligible_for_promotion==false and
    exit_code in {1,2}; exit_code==2 implies state==null. Tested directly against the schema-
    driven validator (white-box import), matching TestOutputSchemaInvariantEnforced's rationale --
    the gate's own business logic never legitimately produces a contradictory output to drive one
    through the CLI."""

    def _base_valid_output(self, **overrides):
        out = {
            "schema_version": 1, "task_class": "full_benchmark", "state": "promotion-ready",
            "eligible_for_promotion": True, "reason_codes": [], "checked_evidence": {},
            "identity": {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None,
                         "topology": "single", "tp": 1},
            "certificate": None, "exit_code": 0,
        }
        out.update(overrides)
        return out

    def test_reviewer_example_promotion_ready_eligible_true_exit_code_1_rejected(self):
        """Exact reviewer example 1: promotion-ready + eligible=true + exit_code=1 is impossible
        (promotion-ready must exit 0) but was schema-valid before this fix."""
        bad = self._base_valid_output(exit_code=1)
        violations = _gate.validate_against_schema(bad, _gate.COMPLETION_SCHEMA)
        self.assertTrue(violations, msg="schema failed to reject promotion-ready/eligible=true/exit_code=1")

    def test_reviewer_example_evidence_complete_eligible_false_exit_code_0_rejected(self):
        """Exact reviewer example 2: evidence-complete + eligible=false + exit_code=0 is
        impossible (exit_code=0 must mean promotion-ready) but was schema-valid before this fix."""
        bad = self._base_valid_output(state="evidence-complete", eligible_for_promotion=False, exit_code=0)
        violations = _gate.validate_against_schema(bad, _gate.COMPLETION_SCHEMA)
        self.assertTrue(violations, msg="schema failed to reject evidence-complete/eligible=false/exit_code=0")

    def test_runtime_ready_with_exit_code_0_rejected(self):
        bad = self._base_valid_output(state="runtime-ready", eligible_for_promotion=False, exit_code=0)
        violations = _gate.validate_against_schema(bad, _gate.COMPLETION_SCHEMA)
        self.assertTrue(violations, msg="schema failed to reject runtime-ready with exit_code=0")

    def test_null_state_with_exit_code_0_rejected(self):
        bad = self._base_valid_output(state=None, eligible_for_promotion=False, exit_code=0)
        violations = _gate.validate_against_schema(bad, _gate.COMPLETION_SCHEMA)
        self.assertTrue(violations, msg="schema failed to reject state=null with exit_code=0")

    def test_exit_code_2_with_named_state_rejected(self):
        bad = self._base_valid_output(state="runtime-ready", eligible_for_promotion=False, exit_code=2)
        violations = _gate.validate_against_schema(bad, _gate.COMPLETION_SCHEMA)
        self.assertTrue(violations, msg="schema failed to reject exit_code=2 with a non-null state")

    def test_null_state_exit_code_1_validates_cleanly(self):
        good = self._base_valid_output(state=None, eligible_for_promotion=False, exit_code=1)
        violations = _gate.validate_against_schema(good, _gate.COMPLETION_SCHEMA)
        self.assertEqual(violations, [], msg=f"violations={violations}")

    def test_evidence_complete_correct_exit_code_1_validates_cleanly(self):
        good = self._base_valid_output(state="evidence-complete", eligible_for_promotion=False, exit_code=1)
        violations = _gate.validate_against_schema(good, _gate.COMPLETION_SCHEMA)
        self.assertEqual(violations, [], msg=f"violations={violations}")


class TestMarkdownAlternateLineEndings(unittest.TestCase):
    """CommonMark treats LF, CRLF, and CR as equivalent line endings.

    The scanner must canonicalize them before fence and inline-link parsing so a CRLF fence cannot
    hide a live post-fence broken link and valid wrapped links are not rejected.
    """

    def test_crlf_fence_closes_and_live_post_fence_link_is_scanned(self):
        text = (
            "```text\r\n"
            "[inside](ignored-missing.md)\r\n"
            "```\r\n"
            "[outside](live-missing.md)\r\n"
        )
        targets, unresolved = _gate._scan_markdown_links(text)
        self.assertEqual(unresolved, [])
        self.assertNotIn("ignored-missing.md", targets)
        self.assertIn("live-missing.md", targets)

    def test_crlf_multiline_inline_link_matches_lf(self):
        lf = '[ok](\n devlog.md\n "a title"\n)'
        crlf = lf.replace("\n", "\r\n")
        self.assertEqual(_gate._scan_markdown_links(crlf), _gate._scan_markdown_links(lf))
        self.assertIn("devlog.md", _gate._scan_markdown_links(crlf)[0])

    def test_cr_multiline_inline_link_matches_lf(self):
        lf = '[ok](\n devlog.md\n "a title"\n)'
        cr = lf.replace("\n", "\r")
        self.assertEqual(_gate._scan_markdown_links(cr), _gate._scan_markdown_links(lf))
        self.assertIn("devlog.md", _gate._scan_markdown_links(cr)[0])


class TestMalformedSchemaEchoStableJson(_ScratchTestCase):
    """Schema-invalid echo values must never corrupt the stable exit-2 completion JSON."""

    def _run_mutation(self, key, value):
        manifest = json.loads((FIXTURES_DIR / "fixture1_runtime_ready_no_docs.json").read_text(encoding="utf-8"))
        manifest[key] = value
        path = self.scratch / "malformed_echo.json"
        path.write_text(json.dumps(manifest), encoding="utf-8")
        code, out, err = run_gate_path(path)
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertEqual(out.get("exit_code"), 2, msg=f"out={out}")
        self.assertIsNone(out.get("state"), msg=f"out={out}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=f"out={out}")
        self.assertIn(f"SCHEMA_TYPE_MISMATCH:{key}", out.get("reason_codes", []), msg=f"out={out}")
        return out

    def test_integer_task_class_emits_schema_valid_exit2_json(self):
        out = self._run_mutation("task_class", 7)
        self.assertIsNone(out.get("task_class"), msg=f"out={out}")

    def test_array_task_class_emits_schema_valid_exit2_json(self):
        out = self._run_mutation("task_class", ["full_benchmark"])
        self.assertIsNone(out.get("task_class"), msg=f"out={out}")

    def test_string_identity_emits_schema_valid_exit2_json(self):
        out = self._run_mutation("identity", "bad")
        self.assertIsNone(out.get("identity"), msg=f"out={out}")


if __name__ == "__main__":
    unittest.main()
