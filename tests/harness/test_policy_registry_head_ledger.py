"""tests/harness/test_policy_registry_head_ledger.py -- HEAD-anchored ledger acceptance oracle
for `.claude/policies/registry.yaml` (plan_26072506 Phase 4 cycle2).

Companion to tests/harness/test_policy_registry.py's `TestBaselineHeadLedger` pointer comment.
Replaces the tautological `baseline_contract.yaml` fixture (a self-authored token list derived
from the registry's OWN statements -- circular, and never actually consumed by any test) with an
independently source-anchored ledger: every migrated rule/gate/exception is traced to the EXACT
pre-migration commit (`f1dcb799d71f42823c492fda663ed2e0672626e2` -- the last commit before the
constitution/workflow prose this registry replaced was thinned) by source path, git blob SHA,
exact line range/excerpt, and a SHA256 of that excerpt. If a clause's registry statement was ever
silently invented (not actually migrated from real prior prose), this ledger has no honest entry
to point to it.

Runner: stdlib `unittest` (matches the rest of tests/harness/).

Invocation:
    python3 -m unittest tests.harness.test_policy_registry_head_ledger -v
    python3 -m unittest discover -s tests/harness -p 'test_*.py' -v

Two independent verification tiers, matching this project's own gitless-deployability
requirement (a fresh clone / clean-index export has no `.git` to query -- see
test_constitution_references.py's module docstring for the same NON_HERMETIC_TDD_FIXTURES class
of bug):

  - TestGitlessLedgerSelfConsistency: no subprocess, no git. The ledger fixture is FROZEN --
    it carries its own excerpt text alongside each excerpt's SHA256, so this tier re-hashes the
    frozen text and compares, checks coverage against the live registry, and checks shape/
    classification -- all without ever touching git. This is the acceptance oracle in a gitless
    environment.
  - TestAmbientGitLedgerVerification: when ambient git IS available (this repo, in this
    environment), re-derives every excerpt directly via `git show <commit>:<path>` sliced to the
    ledger's own line range, and the blob SHA via `git rev-parse <commit>:<path>`, and confirms
    both match the frozen ledger byte-for-byte. Skips (not fails) when git is unavailable --
    that is a legitimate deployment shape this project explicitly supports, not a test failure.

Design note: this ledger deliberately reuses the SAME source excerpt across every clause drawn
from one dense pre-migration bullet (this project's constitution writes one bullet per rule,
often covering several now-split-out clauses) -- the ledger's job is proving each clause traces
to real pre-migration prose/script content, not achieving artificial per-clause excerpt
uniqueness.
"""
from __future__ import annotations

import hashlib
import json
import re
import shutil
import subprocess
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTRY_PATH = REPO_ROOT / ".claude" / "policies" / "registry.yaml"
LEDGER_PATH = REPO_ROOT / "tests" / "harness" / "fixtures" / "policy_registry" / "head_ledger.json"

HEAD_COMMIT = "f1dcb799d71f42823c492fda663ed2e0672626e2"

_BLOB_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_VALID_CLASSIFICATIONS = {"rule", "gate", "exception"}


def _load_ledger() -> dict:
    with open(LEDGER_PATH, "r", encoding="utf-8") as f:
        doc = json.load(f)
    if not isinstance(doc, dict):
        raise AssertionError("head_ledger.json root must be a JSON object")
    return doc


def _load_registry() -> dict:
    with open(REGISTRY_PATH, "r", encoding="utf-8") as f:
        doc = json.load(f)
    return doc if isinstance(doc, dict) else {"policies": []}


def _git_available() -> bool:
    if shutil.which("git") is None:
        return False
    return (REPO_ROOT / ".git").exists() or (REPO_ROOT / ".git").is_file()


# =============================================================================
# Tier 1 -- gitless: works on a clean-index / no-.git export, self-consistency + coverage only.
# =============================================================================

class TestGitlessLedgerSelfConsistency(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ledger = _load_ledger()
        cls.registry = _load_registry()

    def test_ledger_file_exists_and_is_nonempty(self):
        self.assertTrue(LEDGER_PATH.is_file(), msg=f"missing {LEDGER_PATH}")
        self.assertGreater(LEDGER_PATH.stat().st_size, 0)

    def test_head_commit_is_the_exact_pre_migration_sha(self):
        self.assertEqual(self.ledger.get("head_commit"), HEAD_COMMIT)

    def test_entries_is_a_nonempty_list(self):
        entries = self.ledger.get("entries")
        self.assertIsInstance(entries, list)
        self.assertGreater(len(entries), 0)

    def test_every_entry_excerpt_sha256_matches_its_own_frozen_text(self):
        mismatches = []
        for entry in self.ledger["entries"]:
            for ref in entry["source_refs"]:
                digest = hashlib.sha256(ref["excerpt_text"].encode("utf-8")).hexdigest()
                if digest != ref["excerpt_sha256"]:
                    mismatches.append((entry["clause_id"], ref["name"]))
        self.assertEqual(mismatches, [], msg=f"excerpt_sha256 does not match frozen excerpt_text: {mismatches}")

    def test_every_blob_sha_and_excerpt_sha256_has_valid_hex_shape(self):
        for entry in self.ledger["entries"]:
            for ref in entry["source_refs"]:
                with self.subTest(clause=entry["clause_id"], ref=ref["name"]):
                    self.assertRegex(ref["blob_sha"], _BLOB_SHA_RE)
                    self.assertRegex(ref["excerpt_sha256"], _SHA256_RE)

    def test_every_classification_is_one_of_rule_gate_exception(self):
        bad = [e["clause_id"] for e in self.ledger["entries"]
               if e.get("classification") not in _VALID_CLASSIFICATIONS]
        self.assertEqual(bad, [])

    def test_every_entry_has_at_least_one_source_ref(self):
        empty = [e["clause_id"] for e in self.ledger["entries"] if not e.get("source_refs")]
        self.assertEqual(empty, [])

    def test_every_source_ref_line_range_is_well_formed(self):
        for entry in self.ledger["entries"]:
            for ref in entry["source_refs"]:
                with self.subTest(clause=entry["clause_id"], ref=ref["name"]):
                    self.assertGreaterEqual(ref["line_start"], 1)
                    self.assertGreaterEqual(ref["line_end"], ref["line_start"])

    def test_source_paths_are_repo_relative_and_do_not_escape(self):
        for entry in self.ledger["entries"]:
            for ref in entry["source_refs"]:
                p = ref["source_path"]
                with self.subTest(clause=entry["clause_id"], path=p):
                    self.assertFalse(p.startswith("/"), msg=f"{p} must be repo-relative")
                    self.assertNotIn("..", Path(p).parts, msg=f"{p} must not escape the repo")

    def test_no_duplicate_policy_clause_pairs(self):
        pairs = [(e["policy_id"], e["clause_id"]) for e in self.ledger["entries"]]
        self.assertEqual(len(pairs), len(set(pairs)), msg="duplicate ledger entries for the same clause")

    def test_every_clause_id_is_namespaced_under_its_own_policy_id(self):
        for entry in self.ledger["entries"]:
            with self.subTest(clause=entry["clause_id"]):
                self.assertTrue(entry["clause_id"].startswith(entry["policy_id"] + "."),
                                 msg=f"{entry['clause_id']} not namespaced under {entry['policy_id']}")

    def test_ledger_covers_all_13_policies_from_the_current_registry(self):
        registry_ids = {p["policy_id"] for p in self.registry.get("policies", []) if isinstance(p, dict)}
        ledger_ids = {e["policy_id"] for e in self.ledger["entries"]}
        self.assertEqual(registry_ids, ledger_ids)
        self.assertEqual(len(registry_ids), 13)

    def test_ledger_covers_every_clause_id_in_the_current_registry_exactly(self):
        registry_clause_ids = set()
        for p in self.registry.get("policies", []):
            if not isinstance(p, dict):
                continue
            for c in p.get("clauses", []):
                registry_clause_ids.add(c["clause_id"])
        ledger_clause_ids = {e["clause_id"] for e in self.ledger["entries"]}
        missing_from_ledger = registry_clause_ids - ledger_clause_ids
        extra_in_ledger = ledger_clause_ids - registry_clause_ids
        self.assertEqual(missing_from_ledger, set(), msg="registry clauses with no ledger entry")
        self.assertEqual(extra_in_ledger, set(), msg="ledger entries with no matching registry clause")

    def test_host_safety_kdump_clause_is_present_and_source_anchored_to_the_install_script(self):
        # The specific gap this ledger exists to close: kdump/crashkernel/reboot HITL was
        # previously omitted from any source-anchored ledger entirely.
        by_id = {e["clause_id"]: e for e in self.ledger["entries"]}
        c7 = by_id["HOST_SAFETY_LAYERED_DEFENSE.C7"]
        paths = {r["source_path"] for r in c7["source_refs"]}
        self.assertIn("scripts/install_host_safety.sh", paths)
        text = "\n".join(r["excerpt_text"] for r in c7["source_refs"])
        self.assertIn("kdump", text.lower())
        self.assertIn("crashkernel", text.lower())

    def test_host_safety_kdump_clause_covers_every_material_semantic_atom_separately(self):
        # cycle3 finding (subagent-summary-0): the OLD combined-excerpt 'kdump'+'crashkernel'
        # substring check is broad-excerpt laundering -- it passed even though neither ledger ref
        # (workflow.md:162, install script lines 99-112) actually contained the '--with-kdump'
        # flag literal, the vmcore/post-mortem preservation mechanism, or the post-reboot
        # verification procedure. This asserts each material semantic atom of the clause
        # SEPARATELY, against the literal source term that actually demonstrates it (the Korean
        # source excerpts never literally contain the registry's own English paraphrase, e.g.
        # "post-mortem" -- what matters is the underlying mechanism is genuinely present, checked
        # via its real literal term, e.g. "vmcore").
        by_id = {e["clause_id"]: e for e in self.ledger["entries"]}
        c7 = by_id["HOST_SAFETY_LAYERED_DEFENSE.C7"]
        paths = {r["source_path"] for r in c7["source_refs"]}
        self.assertIn("scripts/install_host_safety.sh", paths)
        text = "\n".join(r["excerpt_text"] for r in c7["source_refs"])
        atoms = {
            "explicit --with-kdump flag literal": "--with-kdump",
            "flag defaults off (opt-in, not the installer's unconditional default)": "WITH_KDUMP=0",
            "crashkernel reservation (high)": "KDUMP_CRASHKERNEL",
            "explicit low reservation (aarch64 kernel 6.17)": "KDUMP_CRASHKERNEL_LOW",
            "vmcore preservation / post-mortem evidence": "vmcore",
            "one reboot required to take effect": "재부팅",
            "post-reboot verification procedure": "재부팅 후 검증",
        }
        missing = [label for label, atom in atoms.items() if atom not in text]
        self.assertEqual(missing, [], msg=f"C7 ledger excerpts do not cover: {missing}")

    def test_host_safety_kdump_clause_does_not_claim_exclusivity_without_a_source_atom(self):
        # cycle3 finding: the registry clause invented "it is the only host-safety component that
        # requires one reboot to take effect" -- no ledger excerpt (nor any other tracked source)
        # establishes that exclusivity, so the registry statement must not assert it either.
        registry = self.registry
        by_id = {p["policy_id"]: p for p in registry["policies"]}
        c7_statement = next(c["statement"] for c in by_id["HOST_SAFETY_LAYERED_DEFENSE"]["clauses"]
                             if c["clause_id"] == "HOST_SAFETY_LAYERED_DEFENSE.C7")
        self.assertNotIn("only host-safety component", c7_statement)

    def test_host_safety_cleanup_clause_is_present_and_source_anchored_to_the_cleanup_script(self):
        by_id = {e["clause_id"]: e for e in self.ledger["entries"]}
        c8 = by_id["HOST_SAFETY_LAYERED_DEFENSE.C8"]
        paths = {r["source_path"] for r in c8["source_refs"]}
        self.assertIn("scripts/cleanup_docker.py", paths)
        text = "\n".join(r["excerpt_text"] for r in c8["source_refs"])
        self.assertIn("--apply", text)
        self.assertIn("트리거", text)

    def test_baseline_contract_fixture_no_longer_exists(self):
        stale = REPO_ROOT / "tests" / "harness" / "fixtures" / "policy_registry" / "baseline_contract.yaml"
        self.assertFalse(stale.exists(),
                          msg="tautological self-authored baseline_contract.yaml must be replaced by this "
                              "source-anchored ledger, not left alongside it")


# =============================================================================
# Tier 2 -- ambient git: re-derive every excerpt live and diff against the frozen ledger.
# Skips (not fails) when git is unavailable in this environment.
# =============================================================================

class TestAmbientGitLedgerVerification(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not _git_available():
            raise unittest.SkipTest("ambient git unavailable in this environment -- "
                                     "TestGitlessLedgerSelfConsistency is the acceptance oracle here")
        cls.ledger = _load_ledger()

    def _git_show_lines(self, path: str) -> list[str]:
        out = subprocess.run(["git", "show", f"{self.ledger['head_commit']}:{path}"],
                              cwd=REPO_ROOT, capture_output=True, text=True, check=True)
        return out.stdout.split("\n")

    def test_every_blob_sha_matches_git_rev_parse_at_head_commit(self):
        checked: dict[str, str] = {}
        for entry in self.ledger["entries"]:
            for ref in entry["source_refs"]:
                checked[ref["source_path"]] = ref["blob_sha"]
        mismatches = []
        for path, expected_sha in checked.items():
            out = subprocess.run(["git", "rev-parse", f"{self.ledger['head_commit']}:{path}"],
                                  cwd=REPO_ROOT, capture_output=True, text=True, check=True)
            if out.stdout.strip() != expected_sha:
                mismatches.append((path, expected_sha, out.stdout.strip()))
        self.assertEqual(mismatches, [])

    def test_every_excerpt_matches_the_exact_line_range_at_head_commit(self):
        mismatches = []
        file_cache: dict[str, list[str]] = {}
        for entry in self.ledger["entries"]:
            for ref in entry["source_refs"]:
                path = ref["source_path"]
                if path not in file_cache:
                    file_cache[path] = self._git_show_lines(path)
                lines = file_cache[path]
                chunk = "\n".join(lines[ref["line_start"] - 1:ref["line_end"]])
                if chunk != ref["excerpt_text"]:
                    mismatches.append((entry["clause_id"], ref["name"]))
        self.assertEqual(mismatches, [], msg=f"live git excerpt diverges from frozen ledger: {mismatches}")

    def test_head_commit_is_reachable_and_is_a_real_commit(self):
        out = subprocess.run(["git", "cat-file", "-t", self.ledger["head_commit"]],
                              cwd=REPO_ROOT, capture_output=True, text=True, check=True)
        self.assertEqual(out.stdout.strip(), "commit")


if __name__ == "__main__":
    unittest.main()
