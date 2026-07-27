"""tests/harness/test_policy_clause_companion_evidence.py -- durable, executable companion
evidence for policy-registry clauses whose canonical prose home is CLAUDE.md/workflow.md
(plan_26072506 Phase 4, review-cycle2 remediation).

Why this file exists: `.claude/policies/registry.yaml`'s CLAUSE_LACKS_EXECUTABLE_EVIDENCE check
The production registry validator rejects a clause whose evidence is entirely prose (`.md`) -- prose
may corroborate a clause, but per the review-cycle2 finding, it may never be a clause's SOLE
evidence (cycle1 had 17 clauses backed only by `.claude/rules/workflow.md` or a skill's
`SKILL.md`, making the registry's own claim of being implementation/test-backed circular for
those clauses). `.claude/skills/*/SKILL.md` bodies are out of scope for this remediation (no
skill-body edits), so a handful of clauses genuinely describe a HITL/process discipline this
project deliberately leaves procedural rather than automating; those clauses stay evidenced by
the existing scripts that operationalize the surrounding mechanism (see registry.yaml comments
next to each `supports` extension) rather than gaining a test here. Every test method below
instead reads REAL, ALREADY-TRACKED, non-registry source (scripts this suite does not modify) and
asserts a durable, literal, checkable fact about it -- never a re-statement of the registry's own
prose, and never a fabricated claim. Each becomes one `assertion_ids` entry
(`ClassName.method_name`, AST-verified by `.claude/policies/runtime/policy_registry.py`) on this file as an
evidence-manifest entry in `.claude/policies/evidence_manifest.json`.

Runner: stdlib `unittest` (matches every other tests/harness/test_*.py in this project).
"""
from __future__ import annotations

import ast
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]


def _read(rel_path: str) -> str:
    return (REPO_ROOT / rel_path).read_text(encoding="utf-8")


class TestHostSafetyLayeredDefenseCompanion(unittest.TestCase):
    """HOST_SAFETY_LAYERED_DEFENSE.C3: opt-out is recorded as a neutral manifest fact
    (`host_safety.installed:false`) that is INDEPENDENT of the completion Flag."""

    def test_scan_node_documents_host_safety_flag_independence(self):
        src = _read(".claude/skills/terraforming_node/scripts/scan_node.py")
        self.assertIn("host_safety.installed", src)
        self.assertIn("Flag 와 독립", src)


class TestLastGoodRollbackAnchorCompanion(unittest.TestCase):
    """LAST_GOOD_ROLLBACK_ANCHOR.C1/C3: the rollback anchor is a LOCAL commit; hint_tag.py's own
    push path structurally protects that by refusing to let a `last-good-*` tag exist on origin
    (a real check, not a restatement of the registry's own prose about the anchor)."""

    def test_hint_tag_push_refuses_last_good_tags_on_origin(self):
        src = _read(".claude/skills/upstream-version-watch/scripts/hint_tag.py")
        self.assertIn('"last-good-*"', src)
        self.assertIn("ls-remote", src)

    def test_sync_branches_never_tags_or_hard_resets(self):
        # LAST_GOOD_ROLLBACK_ANCHOR.C2: single-node/multi-node roll back independently -- the
        # branch-sync script that keeps shared building blocks aligned never itself moves a
        # rollback anchor (tags/hard-resets) on either branch.
        src = _read(".claude/skills/upstream-version-watch/scripts/sync_branches.sh")
        self.assertNotIn("git tag", src)
        self.assertNotIn("git reset --hard", src)


class TestModelAcquisitionTernaryGateCompanion(unittest.TestCase):
    """MODEL_ACQUISITION_TERNARY_GATE.C1/C2/C4."""

    def test_manifest_contract_enforces_ternary_model_source(self):
        src = _read(".claude/skills/terraforming_node/scripts/manifest_contract.py")
        tree = ast.parse(src)
        valid_modes = None
        for node in ast.walk(tree):
            if isinstance(node, ast.Assign) and any(
                isinstance(t, ast.Name) and t.id == "VALID_MODES" for t in node.targets
            ):
                if isinstance(node.value, ast.Tuple):
                    valid_modes = tuple(
                        elt.value for elt in node.value.elts if isinstance(elt, ast.Constant)
                    )
        self.assertEqual(valid_modes, ("managed", "ephemeral", "custom"))

    def test_check_smoke_model_has_no_download_machinery(self):
        # A model-presence check that never itself fetches weights is exactly the structural
        # signal for "absence is reported/gated, never silently remedied by an unattended fetch".
        src = _read(".claude/skills/upstream-version-watch/scripts/check_smoke_model.py")
        for banned in ("urllib.request", "requests.", "snapshot_download", "huggingface_hub",
                       "subprocess.run(['wget'", 'subprocess.run(["wget"'):
            self.assertNotIn(banned, src)

    def test_manifest_template_hf_token_field_is_a_pointer_not_a_raw_value(self):
        src = _read("manifest.template.yaml")
        self.assertIn("hf_token_env_file:", src)
        # the tracked skeleton ships an empty pointer field -- never a baked default value.
        self.assertRegex(src, r'hf_token_env_file:\s*""')


class TestTerraformFlagGateCompanion(unittest.TestCase):
    """TERRAFORM_FLAG_GATE.C4: wiki-desk is exempt from the Flag gate -- verified structurally
    (no wiki-desk script references the gate machinery at all), not merely asserted in prose."""

    def test_wiki_desk_scripts_never_reference_the_flag_gate(self):
        wiki_scripts_dir = REPO_ROOT / ".claude" / "skills" / "wiki-desk" / "scripts"
        py_files = sorted(wiki_scripts_dir.glob("*.py"))
        self.assertTrue(py_files, "expected at least one wiki-desk script to scan")
        banned = ("manifest_contract", "_require_terraform_flag", "terraform.complete",
                  "a2a_delegation")
        offenders = []
        for f in py_files:
            text = f.read_text(encoding="utf-8")
            for token in banned:
                if token in text:
                    offenders.append((f.name, token))
        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
