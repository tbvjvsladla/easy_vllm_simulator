"""tests/harness/test_tracked_index.py -- TDD for `.claude/policies/tracked_index.json` and the
tracked-index evidence-trust tier (plan_26072506 Phase 4 cycle4 remediation of subagent-summary-1
finding 1).

The gap this closes: `resolve_evidence_path` previously treated membership in a caller-supplied
`evidence_manifest.json`-shaped dict as sufficient proof that a path is "tracked" -- a
self-authored manifest can list (and digest-match) ANY file, including one that was never staged
or committed at all. The exact reported reproduction: a temporary, non-git repository containing
an untracked `evidence.py`, a custom manifest whose digest matches it, and a registry entry citing
it, returned exit 0 with zero violations.

The fix: `.claude/policies/tracked_index.json` is a SEPARATE, independently-generated snapshot
(repo-relative path -> git blob sha1) that `resolve_evidence_path` cross-checks when given
(opt-in, see scripts/policy_registry.py module docstring "Tracked-index trust tiers"):
  - AMBIENT GIT (this repo, in this environment): the path's CURRENT STAGED blob
    (`git ls-files --stage`) must match the snapshot's recorded blob sha1 -- this is the actual
    provenance authority.
  - GITLESS (a clean-index/no-.git export): no live git call is possible, so trust rests on the
    snapshot's own recorded presence plus evidence_manifest.json's sha256 digest match -- this is
    an EXPORT-TRUST INPUT, not independent cryptographic proof (documented, not claimed as more).

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
POLICY_REGISTRY = SCRIPTS_DIR / "policy_registry.py"
SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "policy-registry.schema.json"
TRACKED_INDEX_PATH = REPO_ROOT / ".claude" / "policies" / "tracked_index.json"

sys.path.insert(0, str(SCRIPTS_DIR))
import policy_registry as pr  # noqa: E402


def canonical_tracked_index(entries):
    return {
        "schema_version": 1,
        "trust_model": {
            "ambient_git": "git index is provenance authority",
            "gitless_export": "snapshot plus content digests is export-trust input, not independent proof",
        },
        "base_tree_sha1": "0" * 40,
        "entries": entries,
    }


def git_blob_sha1(path: Path) -> str:
    payload = path.read_bytes()
    return hashlib.sha1(
        b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload).hexdigest()


class TestTrackedIndexFixtureShape(unittest.TestCase):
    def test_tracked_index_file_exists(self):
        self.assertTrue(TRACKED_INDEX_PATH.is_file(), msg=f"missing {TRACKED_INDEX_PATH}")

    def test_tracked_index_is_valid_json_object_with_canonical_schema_version(self):
        doc = pr.load_tracked_index()
        self.assertIsInstance(doc, dict)
        self.assertEqual(doc.get("schema_version"), 1)
        self.assertIsInstance(doc.get("entries"), dict)
        self.assertGreater(len(doc["entries"]), 0)

    def test_tracked_index_excludes_itself(self):
        doc = pr.load_tracked_index()
        self.assertNotIn(".claude/policies/tracked_index.json", doc["entries"])

    def test_tracked_index_includes_the_required_contract_files(self):
        doc = pr.load_tracked_index()
        required = {
            ".claude/policies/registry.yaml",
            ".claude/schemas/policy-registry.schema.json",
            ".claude/policies/evidence_manifest.json",
            ".claude/policies/claim_bindings.json",
            "scripts/policy_registry.py",
            "scripts/harness_verify.py",
            "tests/harness/test_policy_registry.py",
            "tests/harness/test_policy_registry_head_ledger.py",
            "tests/harness/fixtures/policy_registry/head_ledger.json",
            "CLAUDE.md",
            ".claude/rules/workflow.md",
            ".claude/rules/docs.md",
            ".claude/rules/references.md",
        }
        missing = required - set(doc["entries"])
        self.assertEqual(missing, set())

    def test_every_evidence_manifest_path_is_present_in_tracked_index(self):
        manifest = pr.load_evidence_manifest()
        doc = pr.load_tracked_index()
        missing = set(manifest) - set(doc["entries"])
        self.assertEqual(missing, set(), msg=f"evidence_manifest.json paths absent from tracked_index.json: "
                                              f"{missing}")

    def test_every_tracked_index_blob_sha_has_valid_hex_shape(self):
        doc = pr.load_tracked_index()
        for path, blob in doc["entries"].items():
            with self.subTest(path=path):
                self.assertRegex(blob, r"^[0-9a-f]{40}$")


class TestAmbientGitTrackedIndexVerification(unittest.TestCase):
    """Re-derives every tracked_index.json entry's blob sha1 directly via `git ls-files --stage`
    against the REAL staged index and confirms it matches -- proves the ambient-git tier is a
    genuine live cross-check, not merely re-reading the same frozen snapshot."""

    @classmethod
    def setUpClass(cls):
        result = subprocess.run(["git", "rev-parse", "--is-inside-work-tree"], cwd=REPO_ROOT,
                                 capture_output=True, text=True)
        if result.returncode != 0 or result.stdout.strip() != "true":
            raise unittest.SkipTest("ambient git unavailable in this environment")

    def test_every_entry_matches_the_currently_staged_git_blob(self):
        doc = pr.load_tracked_index()
        mismatches = []
        for path, expected_blob in doc["entries"].items():
            live = pr._git_staged_blob_sha(REPO_ROOT, path)
            if live is None or live != expected_blob:
                mismatches.append((path, expected_blob, live))
        self.assertEqual(mismatches, [], msg=f"tracked_index.json entries not matching the currently staged "
                                              f"git index: {mismatches}")


class TestTrackedIndexEvidenceResolution(unittest.TestCase):
    """Unit-level: resolve_evidence_path's tracked_index cross-check, exercised directly against
    temp repos (not the real project) so these are hermetic and fast."""

    def test_path_absent_from_tracked_index_is_rejected_even_with_a_matching_manifest_digest(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            f = repo_root / "evidence.py"
            f.write_text("def real_check():\n    pass\n", encoding="utf-8")
            manifest = {"evidence.py": pr._sha256_file(f)}
            tracked_index = canonical_tracked_index({})  # deliberately does NOT list evidence.py
            res = pr.resolve_evidence_path(repo_root, "evidence.py", manifest, tracked_index=tracked_index)
            self.assertEqual(res.status, "not_tracked_index")

    def test_path_present_in_both_manifest_and_tracked_index_with_no_ambient_git_resolves_ok(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)  # deliberately NOT a git repo -- gitless tier
            f = repo_root / "evidence.py"
            f.write_text("def real_check():\n    pass\n", encoding="utf-8")
            digest = pr._sha256_file(f)
            manifest = {"evidence.py": digest}
            tracked_index = canonical_tracked_index({"evidence.py": git_blob_sha1(f)})
            res = pr.resolve_evidence_path(repo_root, "evidence.py", manifest, tracked_index=tracked_index)
            self.assertEqual(res.status, "ok")

    def test_gitless_manifest_digest_rebinding_cannot_override_frozen_blob_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            f = repo_root / "evidence.py"
            f.write_text("approved original\n", encoding="utf-8")
            frozen_blob = git_blob_sha1(f)
            f.write_text("fabricated replacement\n", encoding="utf-8")
            manifest = {"evidence.py": pr._sha256_file(f)}
            tracked_index = canonical_tracked_index({"evidence.py": frozen_blob})
            res = pr.resolve_evidence_path(repo_root, "evidence.py", manifest, tracked_index=tracked_index)
            self.assertEqual(res.status, "tracked_index_drift")

    def test_tracked_index_none_default_skips_the_check_entirely(self):
        # Every existing fixture-level test in test_policy_registry.py relies on this default --
        # opt-in only, never a silent behavior change for a caller that never passes tracked_index.
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            f = repo_root / "evidence.py"
            f.write_text("def real_check():\n    pass\n", encoding="utf-8")
            manifest = {"evidence.py": pr._sha256_file(f)}
            res = pr.resolve_evidence_path(repo_root, "evidence.py", manifest)
            self.assertEqual(res.status, "ok")

    def test_drifted_blob_in_ambient_git_repo_is_rejected(self):
        # Real ambient-git tier: init a throwaway repo, stage a file, then hand tracked_index a
        # DIFFERENT (wrong) recorded blob sha -- the live `git ls-files --stage` comparison must
        # catch the mismatch.
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            subprocess.run(["git", "init", "-q"], cwd=repo_root, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repo_root, check=True)
            subprocess.run(["git", "config", "user.name", "test"], cwd=repo_root, check=True)
            f = repo_root / "evidence.py"
            f.write_text("def real_check():\n    pass\n", encoding="utf-8")
            subprocess.run(["git", "add", "evidence.py"], cwd=repo_root, check=True)
            manifest = {"evidence.py": pr._sha256_file(f)}
            tracked_index = canonical_tracked_index({"evidence.py": "0" * 40})  # wrong blob sha
            res = pr.resolve_evidence_path(repo_root, "evidence.py", manifest, tracked_index=tracked_index)
            self.assertEqual(res.status, "tracked_index_drift")

    def test_gitless_malformed_index_metadata_and_blob_types_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            f = repo_root / "evidence.py"
            f.write_text("def real_check():\n    pass\n", encoding="utf-8")
            manifest = {"evidence.py": pr._sha256_file(f)}
            malformed = canonical_tracked_index({"evidence.py": None})
            malformed["schema_version"] = "wrong"
            malformed["unknown"] = True
            res = pr.resolve_evidence_path(repo_root, "evidence.py", manifest, tracked_index=malformed)
            self.assertEqual(res.status, "tracked_index_invalid")

    def test_gitless_empty_trust_model_and_escaping_entries_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            f = repo_root / "evidence.py"
            f.write_text("def real_check():\n    pass\n", encoding="utf-8")
            manifest = {"evidence.py": pr._sha256_file(f)}
            for mutate in (
                lambda index: index.__setitem__("trust_model", {}),
                lambda index: index["entries"].__setitem__("../outside", "0" * 40),
                lambda index: index["entries"].__setitem__("/tmp/outside", "0" * 40),
                lambda index: index["entries"].__setitem__(r"C:\absolute\evidence.py", "0" * 40),
            ):
                index = canonical_tracked_index({"evidence.py": "0" * 40})
                mutate(index)
                self.assertEqual(pr.resolve_evidence_path(
                    repo_root, "evidence.py", manifest, tracked_index=index).status,
                    "tracked_index_invalid")

    def test_ambient_git_unstaged_file_cannot_fall_through_to_gitless_trust(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            subprocess.run(["git", "init", "-q"], cwd=repo_root, check=True)
            f = repo_root / "evidence.py"
            f.write_text("def real_check():\n    pass\n", encoding="utf-8")
            manifest = {"evidence.py": pr._sha256_file(f)}
            index = canonical_tracked_index({"evidence.py": "0" * 40})
            self.assertEqual(pr.resolve_evidence_path(
                repo_root, "evidence.py", manifest, tracked_index=index).status,
                "not_staged")

    def test_verify_without_git_executable_uses_gitless_tier_without_traceback(self):
        proc = subprocess.run(
            [sys.executable, str(POLICY_REGISTRY), "verify", "--as-of", "2026-07-25"],
            cwd=REPO_ROOT, env={"PATH": "/nonexistent"}, capture_output=True, text=True, timeout=60)
        self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout} stderr={proc.stderr}")
        self.assertNotIn("Traceback", proc.stderr)
        self.assertEqual(json.loads(proc.stdout)["violation_count"], 0)


class TestCustomNonGitRepoWithoutTrackedIndexFails(unittest.TestCase):
    """Exact cycle3 reproduction (subagent-summary-1 finding 1): a temporary, non-git repository
    containing an untracked evidence.py, a custom (self-authored) evidence_manifest.json whose
    digest matches it, and a registry entry citing it -- through the ACTUAL CLI (`verify`), which
    now always requires a `tracked_index.json` to exist under the repo_root being checked (default
    resolution is repo_root-relative, not this project's own file -- see
    scripts/policy_registry.py `_load_inputs_or_emit` docstring)."""

    def test_custom_repo_with_only_a_custom_evidence_manifest_and_no_tracked_index_fails_exit2(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            (repo_root / "scripts").mkdir()
            evidence_file = repo_root / "scripts" / "evidence.py"
            evidence_file.write_text("def real_check():\n    pass\n", encoding="utf-8")
            digest = pr._sha256_file(evidence_file)
            custom_manifest = repo_root / "evidence_manifest.json"
            custom_manifest.write_text(json.dumps({"scripts/evidence.py": digest}), encoding="utf-8")
            custom_registry = repo_root / "registry.yaml"
            custom_registry.write_text(json.dumps({
                "schema_version": 2,
                "policies": [{
                    "policy_id": "TEST_UNTRACKED_REPRO", "owner": "x", "statement": "s",
                    "scope": ["single-node"], "origin_failure": "plan_x",
                    "clauses": [{"clause_id": "TEST_UNTRACKED_REPRO.C1", "statement": "s"}],
                    "evidence": [{"path": "scripts/evidence.py", "supports": ["TEST_UNTRACKED_REPRO.C1"],
                                  "assertion_ids": ["real_check"]}],
                    "added_at": "2026-01-01", "last_reproduced": None,
                    "remove_when": "Removable once this repro fixture is deleted from the test suite.",
                    "status": "candidate",
                    "review": {"state": "current", "last_reviewed_at": "2026-07-25",
                               "next_review_due": "2026-10-23", "reviewer": "test"},
                }],
            }), encoding="utf-8")

            # No .git anywhere under repo_root, and no .claude/policies/tracked_index.json either --
            # this is the exact "custom non-git temp repo with only custom evidence manifest but
            # NO trusted tracked_index" shape the remediation requires to fail.
            result = subprocess.run(
                [sys.executable, str(POLICY_REGISTRY), "verify", "--as-of", "2026-07-25",
                 "--registry", str(custom_registry), "--schema", str(SCHEMA_PATH),
                 "--evidence-manifest", str(custom_manifest), "--repo-root", str(repo_root)],
                capture_output=True, text=True)

            self.assertEqual(result.returncode, 2, msg=result.stdout + result.stderr)
            payload = json.loads(result.stdout)
            self.assertTrue(any(v["reason_code"].startswith("TRACKED_INDEX") for v in payload["violations"]),
                             msg=payload)


if __name__ == "__main__":
    unittest.main()
