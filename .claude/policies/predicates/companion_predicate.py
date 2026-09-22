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
(`ClassName.method_name`, AST-verified by `.claude/policies/runtime/policy_registry.py`) on this file as a
registry `evidence` entry, whose path is resolved against Git itself (tracked in the index and
byte-identical to the staged blob) rather than against a hand-maintained digest ledger.

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
    """LAST_GOOD_ROLLBACK_ANCHOR.C1/C3: the rollback anchor is a LOCAL commit; the hint push path
    structurally protects that by pushing exactly one refspec `refs/tags/<tag>:refs/tags/<tag>` for a
    single `hint/` name -- never `--tags`, never a glob, never a branch -- so no local ref can ride
    along to a public origin.

    2026-09-03 (plan_26090222 F-6c): this used to pin cmd_verify's origin-side `ls-remote --tags
    origin last-good-*` scan instead. That scan was deleted -- it asserted a condition about a tag
    this repo never creates, while the guard that actually prevents the leak lives in the push path.

    2026-09-21 (plan_26092119 O2 · O5): the push path moved from hint_tag.cmd_push (a `--tag`
    fnmatch pattern rendered into the glob refspec `refs/tags/hint/*`) to hintlib/tag.py push_tag
    (one exact, validated tag). The glob refspec literal must not come back: fnmatch selection and
    git's glob disagree on `?`/`[...]`, so the verified set and the pushed set could differ."""

    _TAG_PY = ".claude/skills/hint-publisher/scripts/hintlib/tag.py"

    def _functions(self):
        src = _read(self._TAG_PY)
        tree = ast.parse(src)
        return src, {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}

    def test_hint_push_sends_exactly_one_tag_refspec(self):
        src, fns = self._functions()
        for name in ("push_tag", "_tag_ref", "check_ref_format", "git_push_authenticated"):
            self.assertIn(name, fns, f"hintlib.tag.{name} must exist")
        push_src = ast.get_source_segment(src, fns["push_tag"]) or ""
        self.assertIn("check_ref_format(repo, tag)", push_src)
        self.assertIn('refspec = f"{ref}:{ref}"', push_src)
        self.assertIn('return f"refs/tags/{tag}"', ast.get_source_segment(src, fns["_tag_ref"]) or "")
        guard_src = ast.get_source_segment(src, fns["check_ref_format"]) or ""
        self.assertIn("not tag.startswith(core.HINT_TAG_PREFIX)", guard_src)
        self.assertIn("_GLOB_CHARS", guard_src)
        self.assertIn('HINT_TAG_PREFIX = "hint/"', _read(".claude/skills/hint-publisher/scripts/hintlib/core.py"))
        self.assertNotIn('"refs/tags/hint/*"', push_src)
        for name in ("push_tag", "git_push_authenticated"):
            literals = {n.value for n in ast.walk(fns[name])
                        if isinstance(n, ast.Constant) and isinstance(n.value, str)}
            self.assertNotIn("--tags", literals, f"{name} must never pass --tags")

    def test_retired_glob_push_cli_is_gone(self):
        # O5: no shim -- the old CLI whose push rendered the glob refspec must not linger beside the new path,
        # under its old name or any other. Judged as a CLOSED list of the scripts dir's top-level programs (a
        # tripwire: adding one forces this review) rather than by naming the retired file -- a live path to the
        # retired CLI here would itself be an "old CLI reference" for the AC11 grep (plan_26092119 §7).
        scripts = REPO_ROOT / ".claude/skills/hint-publisher/scripts"
        top = sorted(p.name for p in scripts.iterdir() if p.is_file() and p.suffix in (".py", ".sh"))
        self.assertEqual(top, ["hint.py", "render_bench_section.py"],
                         "hint-publisher/scripts must hold only the single CLI and the bench-section renderer "
                         "(+ hintlib/) -- a retired CLI must be removed, not kept as a shim")
        self.assertTrue((scripts / "hintlib" / "tag.py").is_file(), "the push path lives in hintlib/tag.py")

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
