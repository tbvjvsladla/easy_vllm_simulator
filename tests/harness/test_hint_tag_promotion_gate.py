"""tests/harness/test_hint_tag_promotion_gate.py -- Phase 3 (plan_26072506) vertical slice 2A TDD
suite: wires scripts/hint_tag.py's gated subcommands (create/finalize/verify/reindex/push/reverify)
to the shared `completion_gate.py authorize --mode promotion` contract already covered in
isolation by tests/harness/test_promotion_side_effects.py (vertical slice 1).

Hermetic design: every test builds its OWN throwaway git repository under a fresh tempdir,
copies just the scripts/schemas/template this feature needs from the real repo (read-only copies
out of REPO_ROOT -- this suite never writes into REPO_ROOT itself), and invokes the COPIED
scripts/hint_tag.py as a real subprocess with cwd pinned to that throwaway repo. hint_tag.py's own
module-level `ROOT = repo_root()` (git rev-parse --show-toplevel) then resolves to the throwaway
repo, so every git tag / hints/index.json / HINTS.md / hints/.drafts mutation this suite exercises
-- including the one positive full create->finalize->verify->reindex->reverify->push(dry-run)
control -- happens ONLY inside that throwaway repo, never against the real project's tags/index.

`match` is intentionally the ONE hint_tag.py subcommand this suite proves stays read-only /
ungated (no --manifest, no shellout to completion_gate.py at all) -- every other subcommand is
proven to require --manifest and to call the gate before any mutation.

Runner: stdlib `unittest` (same as tests/harness/test_promotion_side_effects.py -- pytest is
unavailable in this environment and installing new dependencies is out of scope).
"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
HINT_TAG_SRC = REPO_ROOT / "scripts" / "hint_tag.py"
COMPLETION_GATE_SRC = REPO_ROOT / "scripts" / "completion_gate.py"
TEMPLATE_SRC = REPO_ROOT / "scripts" / "templates" / "hint_recipe.template.md"
SCHEMA_SRC_DIR = REPO_ROOT / ".claude" / "schemas"
SCHEMA_NAMES = (
    "work-manifest.schema.json",
    "completion-manifest.schema.json",
    "side-effect-authorization.schema.json",
)

HINTS_MARKER = "<!-- hint-index:rows -->"
TEST_TAGGER_NAME = "Test Tagger"
TEST_TAGGER_EMAIL = "test-tagger@example.invalid"

IDENTITY = {
    "model": "hint-gate-test-model", "gpu": "GB10", "vllm": "0.99.0-test",
    "quant": "bf16", "topology": "single", "tp": 1,
}


# =============================================================================
# Isolated-repo scaffolding
# =============================================================================

def _git(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=str(repo), capture_output=True, text=True, timeout=30)


def _run_hint(repo: Path, *args: str, timeout: int = 60) -> tuple[int, str, str]:
    proc = subprocess.run(
        [sys.executable, str(repo / "scripts" / "hint_tag.py"), *args],
        cwd=str(repo), capture_output=True, text=True, timeout=timeout,
    )
    return proc.returncode, proc.stdout, proc.stderr


def _snapshot(repo: Path) -> tuple:
    """A side-effect fingerprint covering everything a gated hint_tag.py subcommand could mutate:
    tracked-file changes (git status -- catches hints/index.json and HINTS.md, both committed at
    repo-build time), the set of hint/* git tags (refs aren't tracked-file content, checked
    separately), and the untracked hints/.drafts scaffold directory (also not tracked-file
    content)."""
    status = _git(repo, "status", "--porcelain").stdout
    tags = _git(repo, "tag", "-l", "hint/*").stdout
    drafts_dir = repo / "hints" / ".drafts"
    drafts = sorted(p.name for p in drafts_dir.iterdir()) if drafts_dir.is_dir() else []
    return (status, tags, tuple(drafts))


def _build_isolated_repo(root_tmp: Path) -> Path:
    repo = root_tmp / "repo"
    repo.mkdir()
    assert _git(repo, "init", "-q").returncode == 0
    _git(repo, "config", "user.name", TEST_TAGGER_NAME)
    _git(repo, "config", "user.email", TEST_TAGGER_EMAIL)
    _git(repo, "config", "commit.gpgsign", "false")

    (repo / "scripts" / "templates").mkdir(parents=True)
    (repo / ".claude" / "schemas").mkdir(parents=True)
    (repo / "hints").mkdir()

    shutil.copy(HINT_TAG_SRC, repo / "scripts" / "hint_tag.py")
    shutil.copy(COMPLETION_GATE_SRC, repo / "scripts" / "completion_gate.py")
    shutil.copy(TEMPLATE_SRC, repo / "scripts" / "templates" / "hint_recipe.template.md")
    for name in SCHEMA_NAMES:
        shutil.copy(SCHEMA_SRC_DIR / name, repo / ".claude" / "schemas" / name)

    # Synthetic, test-only PII term list -- deliberately NOT a copy of the real (gitignored,
    # private) .claude/pii_terms.txt, so this suite never depends on -- or leaks -- real PII terms.
    (repo / ".claude" / "pii_terms.txt").write_text(
        "# synthetic test-only PII terms (not the real repo's private list)\n"
        "FORBIDDEN_TEST_TOKEN\n",
        encoding="utf-8",
    )
    (repo / "hints" / "index.json").write_text(json.dumps({"hints": []}, indent=2) + "\n", encoding="utf-8")
    (repo / "HINTS.md").write_text(
        "# HINTS (isolated test catalog)\n\n"
        "| tag | vllm | model | arch | topology | status | related | last_verified | brief |\n"
        "|---|---|---|---|---|---|---|---|---|\n"
        f"{HINTS_MARKER}\n",
        encoding="utf-8",
    )
    (repo / "README.md").write_text("isolated test repo -- not the real project\n", encoding="utf-8")

    _git(repo, "add", "-A")
    commit = _git(repo, "commit", "-q", "-m", "init")
    assert commit.returncode == 0, commit.stderr
    return repo


class _IsolatedHintRepoTestCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="hint_gate_test_")
        self.repo = _build_isolated_repo(Path(self._tmp.name))

    def tearDown(self):
        self._tmp.cleanup()


# =============================================================================
# Manifest builders
# =============================================================================

def _write_text_evidence(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")


def _write_promotion_ready_manifest(repo: Path, name: str = "promotion_ready.json",
                                     tag: str | None = None, topology: str | None = None) -> str:
    """task_class=full_benchmark, mode=full/verdict=PASS, every required evidence artifact
    genuinely present on disk (certificate identity fields match IDENTITY via
    CERTIFICATE_FIELD_MAP) -- reaches completion_gate.py verify state==promotion-ready. Returns
    the manifest path as a string relative to `repo` (suitable for --manifest).

    `tag`/`topology` are OPTIONAL (Phase 3 review-remediation, tests/harness/
    test_hint_evidence_binding.py): when given, a promotion_target block is embedded, exactly
    binding this manifest to that hint tag/topology/HEAD anchor -- scripts/hint_tag.py's gated
    create/finalize/reverify subcommands require this block (a generic promotion-ready manifest is
    no longer sufficient on its own). Callers that stay denied at the common completion_gate.py
    layer (runtime-ready-only / PII-failed / capped-task-class manifests, all derived from OTHER
    builder functions in this module) never reach hint_tag.py's own binding check at all, so they
    are unaffected and omit tag/topology as before."""
    manifest_dir = repo / "manifests"
    manifest_dir.mkdir(exist_ok=True)
    _write_text_evidence(manifest_dir / "evidence" / "plan.md", "# plan\n\nisolated test plan body.\n")
    _write_text_evidence(manifest_dir / "evidence" / "devlog.md", "# devlog\n\nisolated test devlog body.\n")
    _write_text_evidence(manifest_dir / "evidence" / "testlog.md", "# testlog\n\nisolated test testlog body.\n")
    _write_text_evidence(manifest_dir / "evidence" / "simlog_run" / "note.txt", "raw evidence note\n")
    _write_text_evidence(manifest_dir / "evidence" / "bench_report.md", "# bench report\n\nisolated test bench report.\n")
    _write_text_evidence(manifest_dir / "evidence" / "certificate.yaml", "\n".join([
        "schema_version: 1",
        "record_type: benchmark_certificate",
        "verdict: PASS",
        f"model: {IDENTITY['model']}",
        f"gpu_model: {IDENTITY['gpu']}",
        f"vllm_version: {IDENTITY['vllm']}",
        f"quantization: {IDENTITY['quant']}",
        f"topology: {IDENTITY['topology']}",
        f"tensor_parallel_size: {IDENTITY['tp']}",
        "benchmark_mode: full",
        "",
    ]))
    manifest = {
        "schema_version": 1, "task_class": "full_benchmark", "identity": dict(IDENTITY),
        "runtime": {
            "health_ok": True, "functional_smoke_passed": True, "identity": dict(IDENTITY),
            "containers": [],
        },
        "evidence": {
            "plan": {"path": "evidence/plan.md"},
            "devlog": {"path": "evidence/devlog.md"},
            "testlog": {"path": "evidence/testlog.md"},
            "simlog": {"path": "evidence/simlog_run"},
            "bench_report": {"path": "evidence/bench_report.md"},
            "certificate": {"path": "evidence/certificate.yaml"},
        },
        "benchmark": {"mode": "full", "verdict": "PASS"},
        "pii_scan": {
            "passed": True,
            "scanned_paths": [
                "evidence/plan.md", "evidence/devlog.md", "evidence/testlog.md",
                "evidence/simlog_run", "evidence/bench_report.md", "evidence/certificate.yaml",
            ],
        },
    }
    if tag is not None:
        anchor = _git(repo, "rev-parse", "HEAD").stdout.strip()
        manifest["promotion_target"] = {
            "kind": "hint", "tag": tag, "topology": topology or "", "anchor": anchor,
        }
    manifest_path = manifest_dir / name
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return str(manifest_path.relative_to(repo))


def _write_runtime_ready_manifest(repo: Path, name: str = "runtime_ready.json") -> str:
    """Healthy runtime, zero evidence -- reaches runtime-ready at best; promotion authorize must
    deny with PROMOTION_GATE_NOT_ELIGIBLE (exit 1), never touching any hint_tag.py side effect."""
    manifest_dir = repo / "manifests"
    manifest_dir.mkdir(exist_ok=True)
    manifest = {
        "schema_version": 1, "task_class": "full_benchmark", "identity": dict(IDENTITY),
        "runtime": {
            "health_ok": True, "functional_smoke_passed": True, "identity": dict(IDENTITY),
            "containers": [],
        },
        "evidence": {},
        "benchmark": {"mode": None, "verdict": None},
        "pii_scan": {"passed": True, "scanned_paths": []},
    }
    manifest_path = manifest_dir / name
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return str(manifest_path.relative_to(repo))


def _write_pii_failed_manifest(repo: Path, name: str = "pii_failed.json") -> str:
    """Otherwise-complete evidence (same shape as the promotion-ready manifest) but
    pii_scan.passed=False -- the PII gate binds to evidence-complete itself (not only promotion),
    so this denies with PII_SCAN_FAILED before ever reaching promotion-ready."""
    rel = _write_promotion_ready_manifest(repo, name="_pii_source.json")
    data = json.loads((repo / rel).read_text(encoding="utf-8"))
    data["pii_scan"]["passed"] = False
    out_path = repo / "manifests" / name
    out_path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    return str(out_path.relative_to(repo))


def _write_capped_task_class_manifest(repo: Path, name: str = "capped.json") -> str:
    """read_only_audit is a PROMOTION_CAPPED_TASK_CLASSES member -- structurally can never reach
    promotion-ready even when otherwise evidence-complete."""
    manifest_dir = repo / "manifests"
    manifest_dir.mkdir(exist_ok=True)
    manifest = {
        "schema_version": 1, "task_class": "read_only_audit", "identity": dict(IDENTITY),
        "runtime": {
            "health_ok": True, "functional_smoke_passed": True, "identity": dict(IDENTITY),
            "containers": [],
        },
        "evidence": {},
        "benchmark": {"mode": None, "verdict": None},
        "pii_scan": {"passed": True, "scanned_paths": []},
    }
    manifest_path = manifest_dir / name
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return str(manifest_path.relative_to(repo))


def _fill_judgment_slots(text: str) -> str:
    """Strips every <!-- TODO(judgment ...) ... --> comment from a scaffolded recipe draft (the
    agent-authored-judgment slots hint_tag.py's own cmd_finalize refuses to tag over) so the
    positive control can drive create->finalize end to end without a real LLM judgment pass."""
    return re.sub(r"<!--.*?TODO\(judgment.*?-->", "(filled for isolated positive-control test)", text, flags=re.S)


# =============================================================================
# Section A -- --manifest is a required flag on every GATED subcommand
# =============================================================================

GATED_SUBCOMMAND_MIN_ARGS = {
    "create": ["--tag", "hint/0.1.0-test/hint-gate-test-model/testarch", "--topology", "single", "--allow-new-slug"],
    "finalize": ["--tag", "hint/0.1.0-test/hint-gate-test-model/testarch", "--recipe", "does_not_matter.md",
                 "--topology", "single", "--allow-new-slug"],
    "verify": [],
    "push": [],
    "reverify": ["--tag", "hint/0.1.0-test/hint-gate-test-model/testarch"],
    "reindex": [],
}


class TestManifestFlagRequiredOnGatedSubcommands(_IsolatedHintRepoTestCase):
    def test_each_gated_subcommand_requires_manifest_flag(self):
        for cmd, extra_args in GATED_SUBCOMMAND_MIN_ARGS.items():
            with self.subTest(cmd=cmd):
                before = _snapshot(self.repo)
                code, out, err = _run_hint(self.repo, cmd, *extra_args)
                self.assertNotEqual(code, 0, msg=f"cmd={cmd} unexpectedly succeeded without --manifest: out={out}\nerr={err}")
                self.assertEqual(_snapshot(self.repo), before, msg=f"cmd={cmd} had a side effect despite missing --manifest")


# =============================================================================
# Section B -- `match` stays read-only / ungated
# =============================================================================

class TestMatchStaysUngated(_IsolatedHintRepoTestCase):
    def test_match_works_without_manifest_flag(self):
        code, out, err = _run_hint(self.repo, "match", "--vllm", "x", "--model", "y", "--arch", "z")
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

    def test_match_never_shells_out_to_the_gate(self):
        # Remove the copied completion_gate.py entirely from this isolated repo -- if `match`
        # secretly tried to invoke it, this would surface as a crash/non-zero exit instead.
        (self.repo / "scripts" / "completion_gate.py").unlink()
        code, out, err = _run_hint(self.repo, "match", "--vllm", "x", "--model", "y", "--arch", "z")
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")


# =============================================================================
# Section C -- denied (runtime-ready-only) manifest -> zero side effect, every gated subcommand
# =============================================================================

class TestDeniedManifestZeroSideEffect(_IsolatedHintRepoTestCase):
    TAG = "hint/0.1.0-test/hint-gate-test-model/testarch"

    def _assert_denied_gate_shape(self, out: str, expected_action: str) -> dict:
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(parsed.get("mode"), "promotion", msg=f"parsed={parsed}")
        self.assertEqual(parsed.get("action"), expected_action, msg=f"parsed={parsed}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        return parsed

    def test_create_denied_by_runtime_ready_manifest(self):
        manifest = _write_runtime_ready_manifest(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "create", "--tag", self.TAG, "--topology", "single",
                                    "--allow-new-slug", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self._assert_denied_gate_shape(out, "hint_create")
        self.assertEqual(_snapshot(self.repo), before)

    def test_finalize_denied_by_runtime_ready_manifest(self):
        manifest = _write_runtime_ready_manifest(self.repo)
        before = _snapshot(self.repo)
        # --recipe deliberately points at a path that doesn't exist -- if the gate really runs
        # BEFORE hint_tag.py's own recipe-existence check, denial happens before that check is
        # ever reached (the stable JSON below never mentions "레시피 파일 없음").
        code, out, err = _run_hint(self.repo, "finalize", "--tag", self.TAG,
                                    "--recipe", "manifests/nonexistent_recipe.md",
                                    "--topology", "single", "--allow-new-slug", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self._assert_denied_gate_shape(out, "hint_finalize")
        self.assertEqual(_snapshot(self.repo), before)

    def test_verify_denied_by_runtime_ready_manifest(self):
        manifest = _write_runtime_ready_manifest(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self._assert_denied_gate_shape(out, "hint_verify")
        self.assertEqual(_snapshot(self.repo), before)

    def test_push_denied_by_runtime_ready_manifest(self):
        manifest = _write_runtime_ready_manifest(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "push", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self._assert_denied_gate_shape(out, "hint_push")
        self.assertNotIn("DRY-RUN", out)
        self.assertEqual(_snapshot(self.repo), before)

    def test_reverify_denied_by_runtime_ready_manifest(self):
        manifest = _write_runtime_ready_manifest(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "reverify", "--tag", self.TAG, "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self._assert_denied_gate_shape(out, "hint_reverify")
        self.assertEqual(_snapshot(self.repo), before)

    def test_reindex_denied_by_runtime_ready_manifest(self):
        manifest = _write_runtime_ready_manifest(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "reindex", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self._assert_denied_gate_shape(out, "hint_reindex")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section D -- missing / malformed / non-promotion manifest -> stable failure, zero side effect
# =============================================================================

class TestMalformedManifestStableFailure(_IsolatedHintRepoTestCase):
    def test_missing_manifest_file_denies_verify(self):
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "verify", "--manifest", "manifests/does_not_exist.json")
        self.assertEqual(code, 2, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PROMOTION_GATE_INVALID_MANIFEST", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertIn("MANIFEST_FILE_NOT_FOUND", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_malformed_json_manifest_denies_create(self):
        bad = self.repo / "manifests" / "bad.json"
        bad.parent.mkdir(parents=True, exist_ok=True)
        bad.write_text("{ this is not json", encoding="utf-8")
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "create", "--tag", "hint/0.1.0-test/hint-gate-test-model/testarch",
                                    "--topology", "single", "--allow-new-slug",
                                    "--manifest", "manifests/bad.json")
        self.assertEqual(code, 2, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PROMOTION_GATE_INVALID_MANIFEST", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertIn("MANIFEST_INVALID_JSON", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_non_promotion_capped_task_class_denies_reindex(self):
        manifest = _write_capped_task_class_manifest(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "reindex", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section E -- PII scan failure -> zero side effect (existing manifest, otherwise complete)
# =============================================================================

class TestPiiFailureZeroSideEffect(_IsolatedHintRepoTestCase):
    def test_finalize_denied_by_pii_scan_failed(self):
        manifest = _write_pii_failed_manifest(self.repo)
        recipe = self.repo / "manifests" / "unused_recipe.md"
        recipe.write_text("dummy\n", encoding="utf-8")
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "finalize", "--tag", "hint/0.1.0-test/hint-gate-test-model/testarch",
                                    "--recipe", "manifests/unused_recipe.md", "--topology", "single",
                                    "--allow-new-slug", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PII_SCAN_FAILED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_push_denied_by_pii_scan_failed(self):
        manifest = _write_pii_failed_manifest(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "push", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertNotIn("DRY-RUN", out)
        parsed = json.loads(out)
        self.assertIn("PII_SCAN_FAILED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section F -- promotion-ready manifest is the ONLY positive control: full
# create -> finalize -> verify -> reindex -> reverify -> push(dry-run) flow, isolated repo only.
# =============================================================================

class TestPromotionPositiveControlFullFlow(_IsolatedHintRepoTestCase):
    TAG = "hint/0.99.0-test/hint-gate-test-model/testarch"

    def test_full_pass_manifest_drives_create_finalize_verify_reindex_reverify_push(self):
        manifest = _write_promotion_ready_manifest(self.repo, tag=self.TAG, topology="single 1노드")

        code, out, err = _run_hint(self.repo, "create", "--tag", self.TAG, "--topology", "single 1노드",
                                    "--allow-new-slug", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        draft = self.repo / "hints" / ".drafts" / (self.TAG.replace("/", "_") + ".md")
        self.assertTrue(draft.is_file(), msg=f"draft not scaffolded: out={out}")
        draft.write_text(_fill_judgment_slots(draft.read_text(encoding="utf-8")), encoding="utf-8")

        code, out, err = _run_hint(self.repo, "finalize", "--tag", self.TAG,
                                    "--recipe", str(draft.relative_to(self.repo)),
                                    "--topology", "single 1노드", "--allow-new-slug",
                                    "--tagger-name", TEST_TAGGER_NAME, "--tagger-email", TEST_TAGGER_EMAIL,
                                    "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        tags = _git(self.repo, "tag", "-l", "hint/*").stdout.split()
        self.assertIn(self.TAG, tags, msg=f"tags={tags}")
        idx = json.loads((self.repo / "hints" / "index.json").read_text(encoding="utf-8"))
        self.assertTrue(any(e["tag"] == self.TAG for e in idx["hints"]), msg=f"idx={idx}")
        hints_body = (self.repo / "HINTS.md").read_text(encoding="utf-8")
        self.assertIn(self.TAG, hints_body)

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

        code, out, err = _run_hint(self.repo, "reindex", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

        code, out, err = _run_hint(self.repo, "reverify", "--tag", self.TAG, "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        idx2 = json.loads((self.repo / "hints" / "index.json").read_text(encoding="utf-8"))
        entry = next(e for e in idx2["hints"] if e["tag"] == self.TAG)
        self.assertTrue(entry.get("anchor_reachable"), msg=f"entry={entry}")

        code, out, err = _run_hint(self.repo, "push", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("DRY-RUN", out)


if __name__ == "__main__":
    unittest.main()
