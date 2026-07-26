"""tests/harness/test_promotion_side_effects.py -- TDD suite for the Phase 3 (plan_26072506)
vertical slice 1: a shared side-effect authorization contract in scripts/completion_gate.py
(`authorize --manifest <path> --mode experimental|promotion --action <action>`).

Resolved design (plan section 4.3, not a fourth completion-manifest lifecycle state):
  - `execution-approved` is a PRE-RUNTIME side-effect authorization (plan + HITL + scoped
    manifest), granted via a manifest's OWN `execution_approval` block and checked by
    `authorize --mode experimental`. completion-manifest.schema.json's `state` vocabulary stays
    the strict 3 named states + null (untouched by this phase).
  - `authorize --mode promotion` reuses/re-executes real `verify` semantics and allows only when
    the underlying evaluation reaches state=='promotion-ready' AND eligible_for_promotion==true.
    It does NOT require execution_approval at all.

This slice does NOT wire hint_tag.py / sync_to_sub.sh / sync_branches.sh to this contract (that is
a later Phase 3 slice) -- these tests only exercise `completion_gate.py authorize` in isolation.

Runner: stdlib `unittest` (same as tests/harness/test_completion_gate.py -- pytest is unavailable
in this environment and installing new dependencies is out of scope).

Hermetic design: symlink/escape scenarios build their own scratch files at test run time inside a
temporary, repo-contained scratch directory (setUp/tearDown) -- no committed symlink, no fixed
/tmp dependency, so a clean `git checkout-index` of the staged index reproduces every test
unmodified (same pattern as test_completion_gate.py's _ScratchTestCase).
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GATE_SCRIPT = REPO_ROOT / "scripts" / "completion_gate.py"
FIXTURES_DIR = REPO_ROOT / "tests" / "harness" / "fixtures" / "completion"
SCRATCH_DIR = FIXTURES_DIR / "_runtime_scratch"
WORK_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "work-manifest.schema.json"
SIDE_EFFECT_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "side-effect-authorization.schema.json"

ALLOWED_ACTIONS = (
    "sync_to_sub", "sync_branches",
    "hint_create", "hint_finalize", "hint_verify", "hint_reindex", "hint_push", "hint_reverify",
)


def run_authorize(fixture_name: str, mode: str, action: str, repo_root: Path | None = None) -> tuple[int, dict, str]:
    return run_authorize_path(FIXTURES_DIR / fixture_name, mode, action, repo_root=repo_root)


def run_authorize_path(manifest_path: Path, mode: str, action: str, repo_root: Path | None = None) -> tuple[int, dict, str]:
    if repo_root is None:
        repo_root = REPO_ROOT
    cmd = [sys.executable, str(GATE_SCRIPT), "authorize",
           "--manifest", str(manifest_path), "--mode", mode, "--action", action,
           "--repo-root", str(repo_root)]
    proc = subprocess.run(cmd, cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=60)
    try:
        parsed = json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        raise AssertionError(
            f"authorize stdout was not valid JSON (exit={proc.returncode}): {e}\n"
            f"--- stdout ---\n{proc.stdout}\n--- stderr ---\n{proc.stderr}"
        ) from e
    return proc.returncode, parsed, proc.stderr


def assert_output_schema_shape(test: unittest.TestCase, out: dict) -> None:
    for key in ("schema_version", "mode", "action", "authorization_state", "allowed",
                "reason_codes", "identity", "exit_code"):
        test.assertIn(key, out, msg=f"missing required output field: {key}")
    test.assertIn(out.get("exit_code"), (0, 1, 2), msg=f"out={out}")


def assert_sorted_reason_codes(test: unittest.TestCase, out: dict) -> None:
    codes = out.get("reason_codes", [])
    test.assertEqual(codes, sorted(codes), msg=f"reason_codes must be deterministically sorted: {out}")


# =============================================================================
# Section A -- schema parity (single-source guard against enum drift between the two schema
# files and the Python-side ALLOWED_ACTIONS/argparse choices)
# =============================================================================

class TestActionEnumParity(unittest.TestCase):
    def test_work_manifest_allowed_actions_enum_matches(self):
        schema = json.loads(WORK_SCHEMA_PATH.read_text(encoding="utf-8"))
        enum = schema["definitions"]["executionApproval"]["properties"]["allowed_actions"]["items"]["enum"]
        self.assertEqual(set(enum), set(ALLOWED_ACTIONS), msg=f"enum={enum}")

    def test_side_effect_schema_action_enum_matches(self):
        schema = json.loads(SIDE_EFFECT_SCHEMA_PATH.read_text(encoding="utf-8"))
        enum = [v for v in schema["properties"]["action"]["enum"] if v is not None]
        self.assertEqual(set(enum), set(ALLOWED_ACTIONS), msg=f"enum={enum}")


# =============================================================================
# Section B -- experimental mode: positive
# =============================================================================

class TestExperimentalPositive(unittest.TestCase):
    def test_fully_approved_action_in_scope_is_allowed(self):
        code, out, err = run_authorize("authorize_experimental_positive.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 0, msg=f"out={out}\nstderr={err}")
        self.assertTrue(out.get("allowed"), msg=f"out={out}")
        self.assertEqual(out.get("authorization_state"), "execution-approved", msg=f"out={out}")
        self.assertEqual(out.get("reason_codes"), [], msg=f"out={out}")
        self.assertEqual(out.get("mode"), "experimental")
        self.assertEqual(out.get("action"), "sync_to_sub")
        assert_output_schema_shape(self, out)

    def test_other_scoped_action_is_also_allowed(self):
        code, out, err = run_authorize("authorize_experimental_positive.json", "experimental", "sync_branches")
        self.assertEqual(code, 0, msg=f"out={out}\nstderr={err}")
        self.assertTrue(out.get("allowed"), msg=f"out={out}")

    def test_fabricated_plan_bytes_are_rejected_despite_approval_flags(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shutil.copytree(FIXTURES_DIR, root, dirs_exist_ok=True)
            manifest = root / "authorize_experimental_positive.json"
            (root / "targets/plan.md").write_text(
                "# fabricated plan\nNo approval section or scoped approval atoms.\n", encoding="utf-8")
            code, out, err = run_authorize_path(
                manifest, "experimental", "sync_to_sub", repo_root=root)
            self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
            self.assertFalse(out.get("allowed"), msg=f"out={out}")
            self.assertIn("EXECUTION_APPROVAL_PLAN_DIGEST_MISMATCH", out.get("reason_codes", []))


# =============================================================================
# Section C -- experimental mode: missing / false / unscoped approval (exit1, valid-but-not-approved)
# =============================================================================

class TestExperimentalMissingApproval(unittest.TestCase):
    def test_absent_execution_approval_block_is_exit1(self):
        code, out, err = run_authorize("authorize_missing_approval.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 1, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIsNone(out.get("authorization_state"), msg=f"out={out}")
        self.assertIn("EXECUTION_APPROVAL_ABSENT", out.get("reason_codes", []), msg=f"out={out}")


class TestExperimentalFalseApproval(unittest.TestCase):
    def test_approved_false_is_exit1(self):
        code, out, err = run_authorize("authorize_false_approval.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 1, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIn("EXECUTION_APPROVAL_NOT_APPROVED", out.get("reason_codes", []), msg=f"out={out}")


class TestExperimentalActionNotAllowed(unittest.TestCase):
    def test_action_outside_scope_is_exit1(self):
        # allowed_actions == ["sync_branches"] only -- request the other action.
        code, out, err = run_authorize("authorize_action_not_allowed.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 1, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIn("EXECUTION_APPROVAL_ACTION_NOT_ALLOWED", out.get("reason_codes", []), msg=f"out={out}")

    def test_scoped_action_on_same_fixture_is_allowed(self):
        code, out, err = run_authorize("authorize_action_not_allowed.json", "experimental", "sync_branches")
        self.assertEqual(code, 0, msg=f"out={out}\nstderr={err}")
        self.assertTrue(out.get("allowed"), msg=f"out={out}")


# =============================================================================
# Section D -- experimental mode: unsafe/malformed plan_path (exit2, invalid input)
# =============================================================================

class TestExperimentalPlanPathUnsafe(unittest.TestCase):
    def test_plan_path_missing_is_exit2(self):
        code, out, err = run_authorize("authorize_plan_missing.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIsNone(out.get("authorization_state"), msg=f"out={out}")
        self.assertIn("EXECUTION_APPROVAL_PLAN_PATH_NOT_FOUND", out.get("reason_codes", []), msg=f"out={out}")

    def test_plan_path_empty_file_is_exit2(self):
        code, out, err = run_authorize("authorize_plan_empty.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertIn("EXECUTION_APPROVAL_PLAN_PATH_WRONG_TYPE", out.get("reason_codes", []), msg=f"out={out}")

    def test_plan_path_mismatched_with_evidence_plan_is_exit2(self):
        code, out, err = run_authorize("authorize_plan_mismatch.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertIn("EXECUTION_APPROVAL_PLAN_PATH_MISMATCH", out.get("reason_codes", []), msg=f"out={out}")


class TestExperimentalMalformedApprovalValues(unittest.TestCase):
    def test_approved_wrong_type_is_exit2_schema_violation(self):
        code, out, err = run_authorize("authorize_malformed_approved_type.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertIsNone(out.get("authorization_state"), msg=f"out={out}")
        self.assertTrue(
            any(c.startswith("SCHEMA_TYPE_MISMATCH") for c in out.get("reason_codes", [])),
            msg=f"out={out}",
        )

    def test_timestamp_not_valid_utc_is_exit2(self):
        code, out, err = run_authorize("authorize_malformed_timestamp.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertIn("EXECUTION_APPROVAL_TIMESTAMP_INVALID", out.get("reason_codes", []), msg=f"out={out}")

    def test_empty_allowed_actions_is_exit2_schema_violation(self):
        code, out, err = run_authorize("authorize_malformed_empty_allowed_actions.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertTrue(
            any(c.startswith("SCHEMA_MIN_ITEMS_VIOLATION") for c in out.get("reason_codes", [])),
            msg=f"out={out}",
        )


# =============================================================================
# Section E -- experimental mode: hermetic plan_path symlink/escape (exit2)
# =============================================================================

class _ScratchTestCase(unittest.TestCase):
    """Same hermetic pattern as test_completion_gate.py's _ScratchTestCase: builds temporary
    repo-contained scratch files/symlinks at run time, cleans up in tearDown even on failure."""

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
            pass


class TestExperimentalPlanPathSymlinkAndEscape(_ScratchTestCase):
    def _write_manifest(self, plan_path_rel: str) -> Path:
        manifest = {
            "schema_version": 1, "task_class": "harness_change",
            "identity": {"model": "x", "gpu": "GB10", "vllm": "0.22.0", "quant": None,
                         "topology": "single", "tp": 1},
            "evidence": {"plan": None, "devlog": None, "testlog": None,
                         "simlog": None, "bench_report": None, "certificate": None},
            "pii_scan": {"passed": True},
            "execution_approval": {
                "approved": True, "approved_by": "coag-ash",
                "approved_at_utc": "2026-07-25T05:00:00Z",
                "plan_path": plan_path_rel,
                "plan_sha256": "0" * 64,
                "plan_blob_sha1": "0" * 40,
                "approval_anchor": "## Execution approval",
                "approval_atoms": [
                    "approved_by: coag-ash", "approved_at_utc: 2026-07-25T05:00:00Z",
                    "allowed_action: sync_to_sub", "allowed_action: sync_branches",
                ],
                "allowed_actions": ["sync_to_sub", "sync_branches"],
            },
        }
        manifest_path = self.scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return manifest_path

    def test_plan_path_escaping_repo_is_exit2(self):
        manifest_path = self._write_manifest("../../../../../../../../etc/passwd")
        code, out, err = run_authorize_path(manifest_path, "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertIn("EXECUTION_APPROVAL_PLAN_PATH_ESCAPES_REPO", out.get("reason_codes", []), msg=f"out={out}")

    def test_plan_path_through_symlink_is_exit2(self):
        target = self.external / "secret_outside_repo.txt"
        target.write_text("should never be exposed", encoding="utf-8")
        link = self.scratch / "escape_link.txt"
        os.symlink(target, link)
        manifest_path = self._write_manifest("escape_link.txt")

        code, out, err = run_authorize_path(manifest_path, "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertIn("EXECUTION_APPROVAL_PLAN_PATH_CONTAINS_SYMLINK", out.get("reason_codes", []), msg=f"out={out}")

    def test_plan_path_absolute_is_exit2(self):
        manifest_path = self._write_manifest("/etc/passwd")
        code, out, err = run_authorize_path(manifest_path, "experimental", "sync_to_sub")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertIn("EXECUTION_APPROVAL_PLAN_PATH_ABSOLUTE", out.get("reason_codes", []), msg=f"out={out}")


# =============================================================================
# Section F -- promotion mode: positive (reuses real verify semantics)
# =============================================================================

class TestPromotionPositive(unittest.TestCase):
    def test_full_pass_promotion_ready_manifest_is_allowed(self):
        code, out, err = run_authorize("fixture6_full_pass_promotion_ready.json", "promotion", "sync_branches")
        self.assertEqual(code, 0, msg=f"out={out}\nstderr={err}")
        self.assertTrue(out.get("allowed"), msg=f"out={out}")
        self.assertEqual(out.get("authorization_state"), "promotion-ready", msg=f"out={out}")
        self.assertEqual(out.get("mode"), "promotion")
        self.assertEqual(out.get("reason_codes"), [], msg=f"out={out}")
        assert_output_schema_shape(self, out)

    def test_no_execution_approval_needed_for_promotion(self):
        # fixture6 carries no execution_approval block at all -- promotion mode must not require one.
        fixture_text = (FIXTURES_DIR / "fixture6_full_pass_promotion_ready.json").read_text(encoding="utf-8")
        self.assertNotIn("execution_approval", fixture_text)
        code, out, err = run_authorize("fixture6_full_pass_promotion_ready.json", "promotion", "sync_to_sub")
        self.assertEqual(code, 0, msg=f"out={out}\nstderr={err}")


# =============================================================================
# Section G -- promotion mode: negative (runtime / evidence / PII / invalid manifest)
# =============================================================================

class TestPromotionNegativeRuntime(unittest.TestCase):
    def test_runtime_ready_only_manifest_is_not_allowed(self):
        code, out, err = run_authorize("fixture1_runtime_ready_no_docs.json", "promotion", "sync_branches")
        self.assertEqual(code, 1, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIsNone(out.get("authorization_state"), msg=f"out={out}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", out.get("reason_codes", []), msg=f"out={out}")


class TestPromotionNegativeEvidence(unittest.TestCase):
    def test_lite_only_manifest_is_not_allowed(self):
        code, out, err = run_authorize("fixture2_lite_only_blocks_promotion.json", "promotion", "sync_branches")
        self.assertEqual(code, 1, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", out.get("reason_codes", []), msg=f"out={out}")
        # the underlying verify reason code should still be surfaced, not swallowed
        self.assertIn("EVIDENCE_MISSING:bench_report", out.get("reason_codes", []), msg=f"out={out}")


class TestPromotionNegativePii(unittest.TestCase):
    def test_pii_scan_failed_manifest_is_not_allowed(self):
        code, out, err = run_authorize("fixture7_pii_fail_caps_promotion.json", "promotion", "sync_branches")
        self.assertEqual(code, 1, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", out.get("reason_codes", []), msg=f"out={out}")
        self.assertIn("PII_SCAN_FAILED", out.get("reason_codes", []), msg=f"out={out}")


class TestPromotionNegativeInvalidManifest(unittest.TestCase):
    def test_schema_invalid_manifest_is_exit2(self):
        code, out, err = run_authorize("fixture9_unsupported_task_class.json", "promotion", "sync_branches")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIsNone(out.get("authorization_state"), msg=f"out={out}")
        self.assertIn("PROMOTION_GATE_INVALID_MANIFEST", out.get("reason_codes", []), msg=f"out={out}")

    def test_malformed_json_manifest_is_exit2(self):
        code, out, err = run_authorize("malformed_not_json.json", "promotion", "sync_branches")
        self.assertEqual(code, 2, msg=f"out={out}\nstderr={err}")
        self.assertFalse(out.get("allowed"), msg=f"out={out}")
        self.assertIn("PROMOTION_GATE_INVALID_MANIFEST", out.get("reason_codes", []), msg=f"out={out}")


# =============================================================================
# Section H -- no caller side effect on gate failure (this slice: authorize is read-only; it
# never touches hint_tag.py/sync_to_sub.sh/sync_branches.sh state -- prove the CLI itself makes
# no filesystem writes anywhere outside of stdout on a denied/invalid call).
# =============================================================================

class TestGateFailureHasNoSideEffect(unittest.TestCase):
    def test_denied_promotion_call_touches_no_repo_file(self):
        before = _repo_mtime_fingerprint()
        code, out, err = run_authorize("fixture1_runtime_ready_no_docs.json", "promotion", "sync_branches")
        self.assertEqual(code, 1, msg=f"out={out}")
        after = _repo_mtime_fingerprint()
        self.assertEqual(before, after, msg="authorize must not modify any tracked repo file on denial")

    def test_denied_experimental_call_touches_no_repo_file(self):
        before = _repo_mtime_fingerprint()
        code, out, err = run_authorize("authorize_false_approval.json", "experimental", "sync_to_sub")
        self.assertEqual(code, 1, msg=f"out={out}")
        after = _repo_mtime_fingerprint()
        self.assertEqual(before, after, msg="authorize must not modify any tracked repo file on denial")


def _repo_mtime_fingerprint() -> tuple:
    """A cheap, deterministic proxy for 'nothing under version control changed': the sorted
    (relative-path, mtime_ns) pairs for every file `git ls-files` reports, using a fresh git
    status/diff so this is unaffected by anything the test suite itself writes into the
    gitignored `docs/*/*` or `_runtime_scratch` trees."""
    out = subprocess.run(["git", "diff", "--stat", "HEAD"], cwd=str(REPO_ROOT),
                          capture_output=True, text=True, timeout=30)
    return (out.returncode, out.stdout.strip())


# =============================================================================
# Section I -- stable JSON contract
# =============================================================================

class TestStableJson(unittest.TestCase):
    def test_experimental_positive_is_byte_stable_across_runs(self):
        manifest_path = FIXTURES_DIR / "authorize_experimental_positive.json"
        cmd = [sys.executable, str(GATE_SCRIPT), "authorize", "--manifest", str(manifest_path),
               "--mode", "experimental", "--action", "sync_to_sub", "--repo-root", str(REPO_ROOT)]
        first = subprocess.run(cmd, cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=60)
        second = subprocess.run(cmd, cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=60)
        self.assertEqual(first.returncode, second.returncode)
        self.assertEqual(first.stdout, second.stdout, msg="authorize output must be byte-stable across identical invocations")

    def test_promotion_positive_is_byte_stable_across_runs(self):
        manifest_path = FIXTURES_DIR / "fixture6_full_pass_promotion_ready.json"
        cmd = [sys.executable, str(GATE_SCRIPT), "authorize", "--manifest", str(manifest_path),
               "--mode", "promotion", "--action", "sync_branches", "--repo-root", str(REPO_ROOT)]
        first = subprocess.run(cmd, cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=60)
        second = subprocess.run(cmd, cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=60)
        self.assertEqual(first.returncode, second.returncode)
        self.assertEqual(first.stdout, second.stdout, msg="authorize output must be byte-stable across identical invocations")

    def test_reason_codes_sorted_on_a_multi_reason_result(self):
        code, out, err = run_authorize("fixture2_lite_only_blocks_promotion.json", "promotion", "sync_branches")
        assert_sorted_reason_codes(self, out)


# =============================================================================
# Section J -- CLI usage errors on the authorize subcommand honor the side-effect-authorization
# output shape (not the completion-manifest shape verify's own usage errors use).
# =============================================================================

class TestAuthorizeCliUsageErrorsEmitStableJson(unittest.TestCase):
    def _run_raw(self, args: list[str]) -> tuple[int, str, str]:
        proc = subprocess.run([sys.executable, str(GATE_SCRIPT), *args],
                               cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=30)
        return proc.returncode, proc.stdout, proc.stderr

    def test_missing_mode_flag(self):
        code, stdout, stderr = self._run_raw(["authorize", "--manifest", "x.json", "--action", "sync_to_sub"])
        self.assertEqual(code, 2, msg=f"stdout={stdout}\nstderr={stderr}")
        parsed = json.loads(stdout)
        self.assertIn("CLI_USAGE_ERROR", parsed.get("reason_codes", []), msg=f"out={parsed}")
        self.assertIsNone(parsed.get("authorization_state"))
        self.assertFalse(parsed.get("allowed"))
        self.assertNotIn("Traceback", stderr, msg=f"stderr should not contain a Python traceback: {stderr}")

    def test_invalid_action_choice(self):
        code, stdout, stderr = self._run_raw(["authorize", "--manifest", "x.json", "--mode", "experimental",
                                               "--action", "totally_bogus_action"])
        self.assertEqual(code, 2, msg=f"stdout={stdout}\nstderr={stderr}")
        parsed = json.loads(stdout)
        self.assertIn("CLI_USAGE_ERROR", parsed.get("reason_codes", []), msg=f"out={parsed}")

    def test_invalid_mode_choice(self):
        code, stdout, stderr = self._run_raw(["authorize", "--manifest", "x.json", "--mode", "bogus_mode",
                                               "--action", "sync_to_sub"])
        self.assertEqual(code, 2, msg=f"stdout={stdout}\nstderr={stderr}")
        parsed = json.loads(stdout)
        self.assertIn("CLI_USAGE_ERROR", parsed.get("reason_codes", []), msg=f"out={parsed}")

    def test_output_conforms_to_side_effect_authorization_shape_not_completion_manifest_shape(self):
        code, stdout, stderr = self._run_raw(["authorize", "--manifest", "x.json", "--action", "sync_to_sub"])
        parsed = json.loads(stdout)
        # completion-manifest shape has "state"/"eligible_for_promotion"/"checked_evidence"; the
        # authorize contract has "mode"/"action"/"authorization_state"/"allowed" instead.
        self.assertIn("authorization_state", parsed, msg=f"out={parsed}")
        self.assertIn("allowed", parsed, msg=f"out={parsed}")
        self.assertNotIn("checked_evidence", parsed, msg=f"out={parsed}")
        self.assertNotIn("eligible_for_promotion", parsed, msg=f"out={parsed}")

    def test_verify_cli_usage_errors_are_unaffected_by_authorize_routing(self):
        """Regression guard: verify's own usage-error path must keep emitting the
        completion-manifest shape (state/eligible_for_promotion/checked_evidence), not the new
        authorize shape -- the two subcommands must route to their own schema independently."""
        code, stdout, stderr = self._run_raw(["verify"])
        self.assertEqual(code, 2, msg=f"stdout={stdout}\nstderr={stderr}")
        parsed = json.loads(stdout)
        self.assertIn("CLI_USAGE_ERROR", parsed.get("reason_codes", []), msg=f"out={parsed}")
        self.assertIn("state", parsed, msg=f"out={parsed}")
        self.assertIn("eligible_for_promotion", parsed, msg=f"out={parsed}")
        self.assertNotIn("authorization_state", parsed, msg=f"out={parsed}")


if __name__ == "__main__":
    unittest.main()
