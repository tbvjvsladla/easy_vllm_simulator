"""tests/harness/test_hint_evidence_binding.py -- Phase 3 (plan_26072506) independent-review
remediation TDD suite: closes the 3 blocking findings from
subagent-summary-0-20260725_160544_088157.txt against the vertical-slice-2A gate wiring already
covered by tests/harness/test_hint_tag_promotion_gate.py:

  1. Promotion evidence must be BOUND to the specific hint being authorized -- a generic
     promotion-ready manifest (or one bound to an unrelated tag/topology/anchor/identity) must
     never authorize scripts/hint_tag.py's create/finalize/reverify for a DIFFERENT hint.
  2. `verify` must fail closed for hint tags lacking a durable evidence-binding footer (historical/
     unmigrated tags), even when the supplied manifest is otherwise promotion-ready and even after
     a `reindex` attempt -- `reindex`/`push` must refuse a partial/unsafe rewrite the same way.
  3. `create`'s emitted follow-up `finalize` command must remain literally executable under the
     new required `--manifest` contract (safely shell-quoted, spaces-in-path safe).

Hermetic design mirrors test_hint_tag_promotion_gate.py exactly: every test builds its own
throwaway git repository under a fresh tempdir, copies only the scripts/schemas/template this
feature needs from the real repo (read-only copies out of REPO_ROOT), and invokes the COPIED
scripts/hint_tag.py as a real subprocess with cwd pinned to that throwaway repo -- this suite never
mutates REPO_ROOT's own tags/index/HINTS.md. The current real repo's 23 legacy hint tags are
INTENTIONALLY not migrated in this phase (separate follow-up) -- every test here operates on a
temp repo it built itself, never the real one.
"""
from __future__ import annotations

import hashlib
import json
import re
import shlex
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


# =============================================================================
# Isolated-repo scaffolding (mirrors test_hint_tag_promotion_gate.py)
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
    status = _git(repo, "status", "--porcelain").stdout
    tags = _git(repo, "tag", "-l", "hint/*").stdout
    drafts_dir = repo / "hints" / ".drafts"
    drafts = sorted(p.name for p in drafts_dir.iterdir()) if drafts_dir.is_dir() else []
    return (status, tags, tuple(drafts))


def _head_sha(repo: Path) -> str:
    out = _git(repo, "rev-parse", "HEAD").stdout.strip()
    assert re.fullmatch(r"[0-9a-f]{40}", out), f"unexpected HEAD sha: {out!r}"
    return out


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
        self._tmp = tempfile.TemporaryDirectory(prefix="hint_evidence_binding_test_")
        self.repo = _build_isolated_repo(Path(self._tmp.name))

    def tearDown(self):
        self._tmp.cleanup()


# =============================================================================
# Manifest builders -- promotion-ready + configurable promotion_target/identity
# =============================================================================

def _write_text_evidence(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")


def _write_certificate(path: Path, identity: dict) -> None:
    _write_text_evidence(path, "\n".join([
        "schema_version: 1",
        "record_type: benchmark_certificate",
        "verdict: PASS",
        f"model: {identity['model']}",
        f"gpu_model: {identity['gpu']}",
        f"vllm_version: {identity['vllm']}",
        f"quantization: {identity['quant']}",
        f"topology: {identity['topology']}",
        f"tensor_parallel_size: {identity['tp']}",
        "benchmark_mode: full",
        "",
    ]))


def _write_bound_manifest(repo: Path, *, name: str, identity: dict, promotion_target: dict | None) -> str:
    """A genuinely promotion-ready (task_class=full_benchmark, mode=full/verdict=PASS) manifest
    with every required evidence artifact actually present on disk, optionally carrying a
    promotion_target block. Returns the manifest path relative to `repo`. Evidence lives under a
    name-namespaced subdirectory so multiple manifests can coexist in one repo without collision."""
    stem = re.sub(r"[^A-Za-z0-9_.-]", "_", name.rsplit(".", 1)[0])
    manifest_dir = repo / "manifests"
    manifest_dir.mkdir(exist_ok=True)
    ev_dir = f"evidence_{stem}"
    _write_text_evidence(manifest_dir / ev_dir / "plan.md", "# plan\n\nisolated test plan body.\n")
    _write_text_evidence(manifest_dir / ev_dir / "devlog.md", "# devlog\n\nisolated test devlog body.\n")
    _write_text_evidence(manifest_dir / ev_dir / "testlog.md", "# testlog\n\nisolated test testlog body.\n")
    _write_text_evidence(manifest_dir / ev_dir / "simlog_run" / "note.txt", "raw evidence note\n")
    _write_text_evidence(manifest_dir / ev_dir / "bench_report.md", "# bench report\n\nisolated test bench report.\n")
    _write_certificate(manifest_dir / ev_dir / "certificate.yaml", identity)

    manifest = {
        "schema_version": 1, "task_class": "full_benchmark", "identity": dict(identity),
        "runtime": {
            "health_ok": True, "functional_smoke_passed": True, "identity": dict(identity),
            "containers": [],
        },
        "evidence": {
            "plan": {"path": f"{ev_dir}/plan.md"},
            "devlog": {"path": f"{ev_dir}/devlog.md"},
            "testlog": {"path": f"{ev_dir}/testlog.md"},
            "simlog": {"path": f"{ev_dir}/simlog_run"},
            "bench_report": {"path": f"{ev_dir}/bench_report.md"},
            "certificate": {"path": f"{ev_dir}/certificate.yaml"},
        },
        "benchmark": {"mode": "full", "verdict": "PASS"},
        "pii_scan": {
            "passed": True,
            "scanned_paths": [
                f"{ev_dir}/plan.md", f"{ev_dir}/devlog.md", f"{ev_dir}/testlog.md",
                f"{ev_dir}/simlog_run", f"{ev_dir}/bench_report.md", f"{ev_dir}/certificate.yaml",
            ],
        },
    }
    if promotion_target is not None:
        manifest["promotion_target"] = promotion_target
    manifest_path = manifest_dir / name
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return str(manifest_path.relative_to(repo))


def _fill_judgment_slots(text: str) -> str:
    return re.sub(r"<!--.*?TODO\(judgment.*?-->", "(filled for isolated positive-control test)", text, flags=re.S)


def _finalize_bound_tag(repo: Path, *, tag: str, topology: str, identity: dict, manifest_name: str) -> str:
    """Drives a full create->finalize for ONE correctly-bound hint tag. Returns the manifest path
    (relative to repo) used, so callers can mutate/re-use it afterward."""
    anchor = _head_sha(repo)
    pt = {"kind": "hint", "tag": tag, "topology": topology, "anchor": anchor}
    manifest_rel = _write_bound_manifest(repo, name=manifest_name, identity=identity, promotion_target=pt)
    code, out, err = _run_hint(repo, "create", "--tag", tag, "--topology", topology,
                                "--allow-new-slug", "--manifest", manifest_rel)
    assert code == 0, f"create failed: out={out}\nerr={err}"
    draft = repo / "hints" / ".drafts" / (tag.replace("/", "_") + ".md")
    assert draft.is_file(), f"draft not scaffolded: out={out}"
    draft.write_text(_fill_judgment_slots(draft.read_text(encoding="utf-8")), encoding="utf-8")
    code, out, err = _run_hint(repo, "finalize", "--tag", tag, "--recipe", str(draft.relative_to(repo)),
                                "--topology", topology, "--allow-new-slug",
                                "--tagger-name", TEST_TAGGER_NAME, "--tagger-email", TEST_TAGGER_EMAIL,
                                "--manifest", manifest_rel)
    assert code == 0, f"finalize failed: out={out}\nerr={err}"
    return manifest_rel


def _create_legacy_unbound_tag(repo: Path, tag: str,
                                body: str = "legacy hint body without any evidence pointer\n") -> None:
    """Simulates a pre-Phase-3 hint tag that predates the evidence-binding footer entirely (the
    exact shape of the reviewer's blocker-2 reproduction)."""
    r = _git(repo, "tag", "-a", tag, "-m", body)
    assert r.returncode == 0, r.stderr


# =============================================================================
# Section A -- blocker 1, reviewer-exact reproduction: a generic promotion-ready manifest with NO
# promotion_target must not authorize create for an unrelated hint.
# =============================================================================

class TestCreateUnrelatedManifestRejectedReviewerRepro(_IsolatedHintRepoTestCase):
    def test_create_without_promotion_target_rejected(self):
        identity = {
            "model": "hint-gate-test-model", "gpu": "GB10", "vllm": "0.99.0-test",
            "quant": "bf16", "topology": "single", "tp": 1,
        }
        manifest = _write_bound_manifest(self.repo, name="unrelated.json", identity=identity, promotion_target=None)
        before = _snapshot(self.repo)

        code, out, err = _run_hint(self.repo, "create", "--tag", "hint/9.9.9/unrelated-model/unrelated-arch",
                                    "--topology", "multi TP8", "--allow-new-slug", "--manifest", manifest)

        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("HINT_PROMOTION_TARGET_MISSING", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before, msg="unrelated create must have zero side effect")
        draft = self.repo / "hints" / ".drafts" / "hint_9.9.9_unrelated-model_unrelated-arch.md"
        self.assertFalse(draft.exists())


# =============================================================================
# Section B -- promotion_target present but individually wrong tag/topology/anchor -> rejected.
# =============================================================================

class TestCreatePromotionTargetFieldMismatch(_IsolatedHintRepoTestCase):
    TAG = "hint/1.2.3-test/hint-bind-model/testarch"
    TOPOLOGY = "single 1노드"
    IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "1.2.3-test",
                "quant": "bf16", "topology": "single", "tp": 1}

    def _run_create_with_target(self, target_overrides: dict, expected_code: str):
        anchor = _head_sha(self.repo)
        pt = {"kind": "hint", "tag": self.TAG, "topology": self.TOPOLOGY, "anchor": anchor}
        pt.update(target_overrides)
        manifest = _write_bound_manifest(self.repo, name="m.json", identity=self.IDENTITY, promotion_target=pt)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "create", "--tag", self.TAG, "--topology", self.TOPOLOGY,
                                    "--allow-new-slug", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn(expected_code, parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_tag_mismatch_rejected(self):
        self._run_create_with_target({"tag": "hint/1.2.3-test/hint-bind-model/other-arch"},
                                      "HINT_PROMOTION_TARGET_TAG_MISMATCH")

    def test_topology_mismatch_rejected(self):
        self._run_create_with_target({"topology": "multi 2노드 TP8"},
                                      "HINT_PROMOTION_TARGET_TOPOLOGY_MISMATCH")

    def test_anchor_mismatch_rejected(self):
        self._run_create_with_target({"anchor": "0" * 40},
                                      "HINT_PROMOTION_TARGET_ANCHOR_MISMATCH")

    def test_wrong_kind_rejected_by_schema_shape(self):
        # promotion_target.kind is schema-const'd to "hint" (work-manifest.schema.json) -- a wrong
        # kind is caught by the SHARED completion_gate.py schema-shape pass (before hint_tag.py's
        # own binding logic even runs), not by a redundant hint_tag.py-local kind check.
        self._run_create_with_target({"kind": "sync_to_sub"}, "PROMOTION_GATE_INVALID_MANIFEST")


# =============================================================================
# Section C -- promotion_target matches the CLI exactly, but identity does not relate to the tag.
# =============================================================================

class TestCreateIdentityRelationMismatch(_IsolatedHintRepoTestCase):
    TAG = "hint/1.2.3-test/hint-bind-model/testarch"
    TOPOLOGY = "single 1노드"

    def _create_with_identity(self, identity_overrides: dict, *, topology: str | None = None):
        identity = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "1.2.3-test",
                     "quant": "bf16", "topology": "single", "tp": 1}
        identity.update(identity_overrides)
        topology = topology or self.TOPOLOGY
        anchor = _head_sha(self.repo)
        pt = {"kind": "hint", "tag": self.TAG, "topology": topology, "anchor": anchor}
        manifest = _write_bound_manifest(self.repo, name="m.json", identity=identity, promotion_target=pt)
        before = _snapshot(self.repo)
        code, out, err = _run_hint(self.repo, "create", "--tag", self.TAG, "--topology", topology,
                                    "--allow-new-slug", "--manifest", manifest)
        return code, out, err, before

    def test_model_mismatch_rejected(self):
        code, out, err, before = self._create_with_identity({"model": "totally-different-model"})
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertIn("HINT_IDENTITY_MODEL_MISMATCH", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_vllm_mismatch_rejected(self):
        code, out, err, before = self._create_with_identity({"vllm": "9.9.9-unrelated"})
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertIn("HINT_IDENTITY_VLLM_MISMATCH", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_vllm_fork_suffix_accepted(self):
        # identity.vllm beginning with '<tag-vllm>-' (fork suffix) is the documented deterministic
        # exception -- this must be ACCEPTED, not rejected.
        code, out, err, before = self._create_with_identity({"vllm": "1.2.3-test-forkxyz"})
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        draft = self.repo / "hints" / ".drafts" / (self.TAG.replace("/", "_") + ".md")
        self.assertTrue(draft.is_file())

    def test_topology_class_mismatch_rejected(self):
        code, out, err, before = self._create_with_identity({"topology": "multi"})
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertIn("HINT_IDENTITY_TOPOLOGY_CLASS_MISMATCH", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_tp_label_mismatch_rejected(self):
        code, out, err, before = self._create_with_identity(
            {"topology": "multi", "tp": 1}, topology="multi 2노드 TP2")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertIn("HINT_IDENTITY_TP_MISMATCH", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_tp_label_match_accepted(self):
        code, out, err, before = self._create_with_identity(
            {"topology": "multi", "tp": 2}, topology="multi 2노드 TP2")
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")


# =============================================================================
# Section D -- finalize cross-target rejection: --tag A but promotion_target targets tag B.
# =============================================================================

class TestFinalizeCrossTargetRejection(_IsolatedHintRepoTestCase):
    TAG_A = "hint/1.2.3-test/hint-bind-model/arch-a"
    TAG_B = "hint/1.2.3-test/hint-bind-model/arch-b"
    TOPOLOGY = "single 1노드"
    IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "1.2.3-test",
                "quant": "bf16", "topology": "single", "tp": 1}

    def test_finalize_rejects_manifest_targeting_different_tag(self):
        anchor = _head_sha(self.repo)
        pt = {"kind": "hint", "tag": self.TAG_B, "topology": self.TOPOLOGY, "anchor": anchor}
        manifest = _write_bound_manifest(self.repo, name="m.json", identity=self.IDENTITY, promotion_target=pt)
        before = _snapshot(self.repo)

        # --recipe deliberately doesn't exist -- proves the binding check runs before hint_tag.py's
        # own recipe-existence check (same ordering-proof pattern as test_hint_tag_promotion_gate.py).
        code, out, err = _run_hint(self.repo, "finalize", "--tag", self.TAG_A,
                                    "--recipe", "manifests/nonexistent_recipe.md",
                                    "--topology", self.TOPOLOGY, "--allow-new-slug", "--manifest", manifest)

        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertIn("HINT_PROMOTION_TARGET_TAG_MISMATCH", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertNotIn("레시피 파일 없음", out)
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section E -- blocker 2: historical unbound tag + unrelated manifest -> reindex/verify/push
# rejection with zero index mutation, and STILL failing after a reindex attempt.
# =============================================================================

class TestHistoricalUnboundTagFailsClosed(_IsolatedHintRepoTestCase):
    LEGACY_TAG = "hint/8.8.8/legacy-model/legacy-arch"

    def _seed_legacy_index_entry(self) -> None:
        idx_path = self.repo / "hints" / "index.json"
        idx = json.loads(idx_path.read_text(encoding="utf-8"))
        idx["hints"].append({
            "tag": self.LEGACY_TAG, "vllm": "8.8.8", "model": "legacy-model", "arch": "legacy-arch",
            "topology": "single", "brief": "legacy", "anchor": _head_sha(self.repo),
            "related": "", "status": "active", "last_verified": "2020-01-01",
        })
        idx_path.write_text(json.dumps(idx, indent=2) + "\n", encoding="utf-8")
        _git(self.repo, "add", "-A")
        r = _git(self.repo, "commit", "-q", "-m", "seed legacy index entry")
        assert r.returncode == 0, r.stderr

    def _unrelated_manifest(self) -> str:
        identity = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "1.2.3-test",
                     "quant": "bf16", "topology": "single", "tp": 1}
        other_tag = "hint/1.2.3-test/hint-bind-model/other-arch"
        pt = {"kind": "hint", "tag": other_tag, "topology": "single 1노드", "anchor": _head_sha(self.repo)}
        return _write_bound_manifest(self.repo, name="unrelated.json", identity=identity, promotion_target=pt)

    def test_reindex_rejects_and_leaves_index_untouched(self):
        _create_legacy_unbound_tag(self.repo, self.LEGACY_TAG)
        self._seed_legacy_index_entry()
        manifest = self._unrelated_manifest()

        idx_path = self.repo / "hints" / "index.json"
        hints_path = self.repo / "HINTS.md"
        idx_before = idx_path.read_text(encoding="utf-8")
        hints_before = hints_path.read_text(encoding="utf-8")

        code, out, err = _run_hint(self.repo, "reindex", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("HINT_EVIDENCE_BINDING_INCOMPLETE", out)
        self.assertIn("HINT_EVIDENCE_BINDING_MISSING", out)
        self.assertIn(self.LEGACY_TAG, out)
        self.assertEqual(idx_path.read_text(encoding="utf-8"), idx_before,
                          msg="reindex must not partially rewrite index.json")
        self.assertEqual(hints_path.read_text(encoding="utf-8"), hints_before,
                          msg="reindex must not partially rewrite HINTS.md")

    def test_verify_rejects_unbound_historical_tag(self):
        _create_legacy_unbound_tag(self.repo, self.LEGACY_TAG)
        self._seed_legacy_index_entry()
        manifest = self._unrelated_manifest()

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("HINT_EVIDENCE_BINDING_MISSING", out)
        self.assertIn(self.LEGACY_TAG, out)

    def test_verify_still_rejects_after_failed_reindex_attempt(self):
        _create_legacy_unbound_tag(self.repo, self.LEGACY_TAG)
        self._seed_legacy_index_entry()
        manifest = self._unrelated_manifest()

        code1, out1, err1 = _run_hint(self.repo, "reindex", "--manifest", manifest)
        self.assertNotEqual(code1, 0, msg=f"out={out1}\nerr={err1}")

        code2, out2, err2 = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertNotEqual(code2, 0, msg=f"out={out2}\nerr={err2}")
        self.assertIn("HINT_EVIDENCE_BINDING_MISSING", out2)
        self.assertIn(self.LEGACY_TAG, out2)

    def test_push_rejects_unbound_historical_tag_no_dry_run_output(self):
        _create_legacy_unbound_tag(self.repo, self.LEGACY_TAG)
        self._seed_legacy_index_entry()
        manifest = self._unrelated_manifest()
        before = _snapshot(self.repo)

        code, out, err = _run_hint(self.repo, "push", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertNotIn("DRY-RUN", out)
        self.assertIn("HINT_EVIDENCE_BINDING_INCOMPLETE", out)
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section F -- malformed / duplicate footer -> verify and reindex reject, zero index mutation.
# =============================================================================

class TestMalformedDuplicateFooterRejected(_IsolatedHintRepoTestCase):
    TAG = "hint/3.0.0-test/hint-bind-model/testarch"

    def _tag_with_body(self, body: str) -> None:
        r = _git(self.repo, "tag", "-a", self.TAG, "-m", body)
        assert r.returncode == 0, r.stderr

    def _unrelated_manifest(self) -> str:
        identity = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "3.0.0-test",
                     "quant": "bf16", "topology": "single", "tp": 1}
        return _write_bound_manifest(self.repo, name="unrelated.json", identity=identity, promotion_target=None)

    def test_verify_rejects_duplicate_footer_key(self):
        body = (
            "brief\n\n"
            "<!-- hint-evidence-binding:v1\n"
            "version: 1\n"
            f"tag: {self.TAG}\n"
            "topology: single\n"
            f"anchor: {'a' * 40}\n"
            f"manifest_sha256: {'b' * 64}\n"
            f"manifest_sha256: {'c' * 64}\n"
            f"identity_sha256: {'d' * 64}\n"
            "manifest_ref: manifests/x.json\n"
            f"certificate_sha256: {'e' * 64}\n"
            "certificate_ref: evidence/cert.yaml\n"
            "-->\n"
        )
        self._tag_with_body(body)
        manifest = self._unrelated_manifest()
        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("HINT_EVIDENCE_BINDING_MALFORMED", out)

    def test_verify_rejects_missing_required_footer_field(self):
        body = (
            "brief\n\n"
            "<!-- hint-evidence-binding:v1\n"
            "version: 1\n"
            f"tag: {self.TAG}\n"
            "topology: single\n"
            f"anchor: {'a' * 40}\n"
            f"manifest_sha256: {'b' * 64}\n"
            f"identity_sha256: {'d' * 64}\n"
            "manifest_ref: manifests/x.json\n"
            f"certificate_sha256: {'e' * 64}\n"
            "-->\n"
        )
        self._tag_with_body(body)
        manifest = self._unrelated_manifest()
        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("HINT_EVIDENCE_BINDING_MALFORMED", out)

    def test_verify_rejects_malformed_anchor_format(self):
        body = (
            "brief\n\n"
            "<!-- hint-evidence-binding:v1\n"
            "version: 1\n"
            f"tag: {self.TAG}\n"
            "topology: single\n"
            "anchor: not-a-valid-sha\n"
            f"manifest_sha256: {'b' * 64}\n"
            f"identity_sha256: {'d' * 64}\n"
            "manifest_ref: manifests/x.json\n"
            f"certificate_sha256: {'e' * 64}\n"
            "certificate_ref: evidence/cert.yaml\n"
            "-->\n"
        )
        self._tag_with_body(body)
        manifest = self._unrelated_manifest()
        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("HINT_EVIDENCE_BINDING_MALFORMED", out)

    def test_reindex_rejects_malformed_footer_atomically(self):
        body = (
            "brief\n\n"
            "<!-- hint-evidence-binding:v1\n"
            "version: 1\n"
            f"tag: {self.TAG}\n"
            "topology: single\n"
            f"anchor: {'a' * 40}\n"
            f"manifest_sha256: {'b' * 64}\n"
            f"manifest_sha256: {'c' * 64}\n"
            f"identity_sha256: {'d' * 64}\n"
            "manifest_ref: manifests/x.json\n"
            f"certificate_sha256: {'e' * 64}\n"
            "certificate_ref: evidence/cert.yaml\n"
            "-->\n"
        )
        self._tag_with_body(body)
        manifest = self._unrelated_manifest()
        idx_before = (self.repo / "hints" / "index.json").read_text(encoding="utf-8")

        code, out, err = _run_hint(self.repo, "reindex", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("HINT_EVIDENCE_BINDING_MALFORMED", out)
        self.assertEqual((self.repo / "hints" / "index.json").read_text(encoding="utf-8"), idx_before)


# =============================================================================
# Section G -- positive control: a correctly bound finalize, then
# verify/reindex/reverify/push(dry-run) all PASS, with binding-fidelity assertions.
# =============================================================================

class TestValidBoundFullFlowPositive(_IsolatedHintRepoTestCase):
    TAG = "hint/2.0.0-test/hint-bind-model/testarch"
    TOPOLOGY = "single 1노드"
    IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "2.0.0-test",
                "quant": "bf16", "topology": "single", "tp": 1}

    def test_full_bound_flow(self):
        anchor = _head_sha(self.repo)
        manifest = _finalize_bound_tag(self.repo, tag=self.TAG, topology=self.TOPOLOGY,
                                        identity=self.IDENTITY, manifest_name="bound.json")

        body = _git(self.repo, "cat-file", "tag", self.TAG).stdout
        self.assertIn("hint-evidence-binding:v1", body)
        self.assertIn(f"tag: {self.TAG}", body)
        self.assertIn(f"anchor: {anchor}", body)

        draft = self.repo / "hints" / ".drafts" / (self.TAG.replace("/", "_") + ".md")
        self.assertNotIn("hint-evidence-binding", draft.read_text(encoding="utf-8"),
                          msg="finalize must never mutate the source recipe file")

        idx = json.loads((self.repo / "hints" / "index.json").read_text(encoding="utf-8"))
        entry = next(e for e in idx["hints"] if e["tag"] == self.TAG)
        self.assertEqual(entry["anchor"], anchor)
        self.assertEqual(entry["topology"], self.TOPOLOGY)

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

        code, out, err = _run_hint(self.repo, "reindex", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        idx2 = json.loads((self.repo / "hints" / "index.json").read_text(encoding="utf-8"))
        entry2 = next(e for e in idx2["hints"] if e["tag"] == self.TAG)
        self.assertEqual(entry2["anchor"], anchor)
        self.assertEqual(entry2["topology"], self.TOPOLOGY)

        code, out, err = _run_hint(self.repo, "reverify", "--tag", self.TAG, "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

        code, out, err = _run_hint(self.repo, "push", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("DRY-RUN", out)


# =============================================================================
# Section H -- manifest changed after tag -> verify rejection.
# =============================================================================

class TestManifestChangedAfterTagRejected(_IsolatedHintRepoTestCase):
    TAG = "hint/2.1.0-test/hint-bind-model/testarch"
    TOPOLOGY = "single 1노드"
    IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "2.1.0-test",
                "quant": "bf16", "topology": "single", "tp": 1}

    def test_verify_rejects_after_manifest_bytes_change(self):
        manifest_rel = _finalize_bound_tag(self.repo, tag=self.TAG, topology=self.TOPOLOGY,
                                            identity=self.IDENTITY, manifest_name="bound.json")

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest_rel)
        self.assertEqual(code, 0, msg=f"sanity check failed right after finalize: out={out}\nerr={err}")

        mpath = self.repo / manifest_rel
        mpath.write_text(mpath.read_text(encoding="utf-8") + "\n", encoding="utf-8")

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest_rel)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("manifest", out.lower())


# =============================================================================
# Section I -- certificate changed/missing -> verify rejection (despite common gate).
# =============================================================================

class TestCertificateChangedOrMissingRejected(_IsolatedHintRepoTestCase):
    TAG = "hint/2.2.0-test/hint-bind-model/testarch"
    TOPOLOGY = "single 1노드"
    IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "2.2.0-test",
                "quant": "bf16", "topology": "single", "tp": 1}

    def test_verify_rejects_after_certificate_content_tampered(self):
        manifest_rel = _finalize_bound_tag(self.repo, tag=self.TAG, topology=self.TOPOLOGY,
                                            identity=self.IDENTITY, manifest_name="bound1.json")
        cert_path = self.repo / "manifests" / "evidence_bound1" / "certificate.yaml"
        # A comment-only edit: still parses fine (parse_flat_certificate skips '#' lines) and still
        # identity-matches the manifest -- the common gate alone would NOT catch this. Only the
        # hint-specific raw-bytes sha256 recompute (against the footer) can.
        cert_path.write_text(cert_path.read_text(encoding="utf-8") + "# tampered\n", encoding="utf-8")

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest_rel)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn(self.TAG, out)

    def test_verify_rejects_after_certificate_deleted(self):
        manifest_rel = _finalize_bound_tag(self.repo, tag=self.TAG, topology=self.TOPOLOGY,
                                            identity=self.IDENTITY, manifest_name="bound2.json")
        before = _snapshot(self.repo)
        cert_path = self.repo / "manifests" / "evidence_bound2" / "certificate.yaml"
        cert_path.unlink()

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest_rel)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section J -- blocker 3: create's emitted finalize command is safely quoted and truly executable,
# including a --manifest path containing spaces.
# =============================================================================

class TestCreateEmittedFinalizeCommandWithSpaces(_IsolatedHintRepoTestCase):
    TAG = "hint/4.0.0-test/hint-bind-model/testarch"
    TOPOLOGY = "single 1노드"
    IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "4.0.0-test",
                "quant": "bf16", "topology": "single", "tp": 1}

    def test_emitted_finalize_command_quotes_manifest_path_with_spaces_and_executes(self):
        anchor = _head_sha(self.repo)
        pt = {"kind": "hint", "tag": self.TAG, "topology": self.TOPOLOGY, "anchor": anchor}
        manifest_rel = _write_bound_manifest(self.repo, name="promo ready with spaces.json",
                                              identity=self.IDENTITY, promotion_target=pt)
        self.assertIn(" ", manifest_rel)

        code, out, err = _run_hint(self.repo, "create", "--tag", self.TAG, "--topology", self.TOPOLOGY,
                                    "--allow-new-slug", "--manifest", manifest_rel)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

        cmd_line = next(l for l in out.splitlines() if "finalize" in l and "--tag" in l).strip()
        self.assertIn(shlex.quote(manifest_rel), cmd_line,
                      msg=f"emitted line does not safely quote the manifest path: {cmd_line!r}")

        tokens = shlex.split(cmd_line)
        self.assertEqual(tokens[0], "python3")
        tokens[0] = sys.executable  # hermetic: use this interpreter, not a bare 'python3' on PATH
        self.assertIn("--manifest", tokens)
        self.assertEqual(tokens[tokens.index("--manifest") + 1], manifest_rel)

        draft = self.repo / "hints" / ".drafts" / (self.TAG.replace("/", "_") + ".md")
        self.assertTrue(draft.is_file())
        draft.write_text(_fill_judgment_slots(draft.read_text(encoding="utf-8")), encoding="utf-8")

        proc = subprocess.run(tokens, cwd=str(self.repo), capture_output=True, text=True, timeout=60)
        self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}\nstderr={proc.stderr}")
        tags = _git(self.repo, "tag", "-l", "hint/*").stdout.split()
        self.assertIn(self.TAG, tags, msg=f"tags={tags}")


# =============================================================================
# Cycle-2 remediation (subagent-summary-0-20260725_164222_333592.txt): a historical hint tag can
# carry a SYNTACTICALLY valid footer whose manifest_ref/certificate_ref are nonexistent, unsafe, or
# whose referenced content has drifted/never matched -- verify/reindex/push must fail closed for
# EVERY such tag, using ONLY that tag's own footer + ROOT, never the --manifest the command happens
# to be invoked with.
# =============================================================================

_STRONG_IDENTITY_FIELDS = ("model", "gpu", "vllm", "quant", "topology", "tp")
_FOOTER_FIELD_ORDER = (
    "version", "tag", "topology", "anchor", "manifest_sha256", "identity_sha256",
    "manifest_ref", "certificate_sha256", "certificate_ref",
)


def _identity_sha256(identity: dict) -> str:
    payload = {f: identity.get(f) for f in _STRONG_IDENTITY_FIELDS}
    blob = json.dumps(payload, sort_keys=True, ensure_ascii=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(blob).hexdigest()


def _write_footer_tag(repo: Path, tag: str, *, fields: dict, tag_commit: str | None = None,
                       brief: str = "crafted brief") -> None:
    """Hand-crafts an annotated hint tag directly via `git tag -a` (bypassing create/finalize
    entirely) whose footer is exactly `fields` -- lets a test construct any combination of
    legitimate/forged footer values."""
    commit = tag_commit or _head_sha(repo)
    footer_lines = ["<!-- hint-evidence-binding:v1"]
    footer_lines += [f"{k}: {fields[k]}" for k in _FOOTER_FIELD_ORDER]
    footer_lines.append("-->")
    body = brief + "\n\n" + "\n".join(footer_lines) + "\n"
    r = _git(repo, "tag", "-a", tag, commit, "-m", body)
    assert r.returncode == 0, r.stderr


def _craft_bound_tag_with_real_evidence(repo: Path, tag: str, *, manifest_identity: dict,
                                         manifest_promotion_target: dict | None,
                                         manifest_name: str, footer_overrides: dict | None = None,
                                         tag_commit: str | None = None) -> str:
    """Writes a REAL manifest+certificate on disk (via _write_bound_manifest) with REAL computed
    digests, then hand-crafts the tag directly -- the footer is self-consistent (hash-wise) by
    default, but `footer_overrides` can inject exactly ONE deliberate defect (a mismatched anchor,
    an unrelated promotion_target inside the referenced manifest, ...) while everything else about
    the tag stays legitimate. Returns the manifest path (relative to repo)."""
    commit = tag_commit or _head_sha(repo)
    manifest_rel = _write_bound_manifest(repo, name=manifest_name, identity=manifest_identity,
                                          promotion_target=manifest_promotion_target)
    manifest_path = repo / manifest_rel
    manifest_sha256 = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    cert_rel = json.loads(manifest_path.read_text(encoding="utf-8"))["evidence"]["certificate"]["path"]
    cert_path = manifest_path.parent / cert_rel
    certificate_sha256 = hashlib.sha256(cert_path.read_bytes()).hexdigest()
    fields = {
        "version": "1", "tag": tag, "topology": manifest_identity.get("topology", "single"),
        "anchor": commit, "manifest_sha256": manifest_sha256,
        "identity_sha256": _identity_sha256(manifest_identity),
        "manifest_ref": manifest_rel, "certificate_sha256": certificate_sha256, "certificate_ref": cert_rel,
    }
    fields.update(footer_overrides or {})
    _write_footer_tag(repo, tag, fields=fields, tag_commit=commit)
    return manifest_rel


def _craft_tag_with_fabricated_refs(repo: Path, tag: str, *, manifest_ref: str, certificate_ref: str,
                                     topology: str = "single", footer_overrides: dict | None = None,
                                     tag_commit: str | None = None) -> None:
    """Reproduces the exact blocker-2-cycle-2 shape: a syntactically valid footer whose
    manifest_ref/certificate_ref are attacker-controlled (nonexistent/unsafe) and whose digests are
    well-formed but fabricated (never computed from any real file)."""
    commit = tag_commit or _head_sha(repo)
    fields = {
        "version": "1", "tag": tag, "topology": topology, "anchor": commit,
        "manifest_sha256": "a" * 64, "identity_sha256": "b" * 64,
        "manifest_ref": manifest_ref, "certificate_sha256": "c" * 64, "certificate_ref": certificate_ref,
    }
    fields.update(footer_overrides or {})
    _write_footer_tag(repo, tag, fields=fields, tag_commit=commit)


def _craft_tag_with_raw_manifest_bytes(repo: Path, tag: str, *, manifest_rel: str, manifest_bytes: bytes,
                                        footer_overrides: dict | None = None,
                                        tag_commit: str | None = None) -> None:
    """A footer whose manifest_ref genuinely resolves (real file, real matching sha256) but whose
    CONTENT is deliberately malformed/non-promotion-ready -- isolates 'exists + hash matches, but
    fails referenced-manifest evaluation' from 'doesn't exist at all'."""
    commit = tag_commit or _head_sha(repo)
    path = repo / manifest_rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(manifest_bytes)
    fields = {
        "version": "1", "tag": tag, "topology": "single", "anchor": commit,
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(), "identity_sha256": "b" * 64,
        "manifest_ref": manifest_rel, "certificate_sha256": "c" * 64,
        "certificate_ref": "evidence/DOES-NOT-EXIST.yaml",
    }
    fields.update(footer_overrides or {})
    _write_footer_tag(repo, tag, fields=fields, tag_commit=commit)


class TestFabricatedHistoricalFooterRejected(_IsolatedHintRepoTestCase):
    """Exact cycle-2 reproduction: one legitimate bound tag + one historical tag whose footer is
    syntactically valid but declares nonexistent manifest_ref/certificate_ref with fabricated,
    well-formed digests."""

    LEGIT_TAG = "hint/5.0.0-test/hint-bind-model/legit-arch"
    LEGIT_TOPOLOGY = "single 1노드"
    LEGIT_IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "5.0.0-test",
                       "quant": "bf16", "topology": "single", "tp": 1}
    FORGED_TAG = "hint/5.0.0-test/hint-bind-model/forged-arch"

    def _seed_legit_and_forged(self) -> str:
        legit_manifest = _finalize_bound_tag(self.repo, tag=self.LEGIT_TAG, topology=self.LEGIT_TOPOLOGY,
                                              identity=self.LEGIT_IDENTITY, manifest_name="legit.json")
        _craft_tag_with_fabricated_refs(self.repo, self.FORGED_TAG,
                                         manifest_ref="manifests/DOES-NOT-EXIST.json",
                                         certificate_ref="evidence/DOES-NOT-EXIST.yaml")
        return legit_manifest

    def test_reproduction_reindex_and_verify_reject_forged_footer(self):
        legit_manifest = self._seed_legit_and_forged()
        idx_before = (self.repo / "hints" / "index.json").read_text(encoding="utf-8")
        hints_before = (self.repo / "HINTS.md").read_text(encoding="utf-8")

        code, out, err = _run_hint(self.repo, "reindex", "--manifest", legit_manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn(self.FORGED_TAG, out)
        self.assertEqual((self.repo / "hints" / "index.json").read_text(encoding="utf-8"), idx_before,
                          msg="reindex must not partially rewrite index.json around the forged tag")
        self.assertEqual((self.repo / "HINTS.md").read_text(encoding="utf-8"), hints_before)

        code, out, err = _run_hint(self.repo, "verify", "--manifest", legit_manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn(self.FORGED_TAG, out)

    def test_push_rejects_forged_footer_before_dry_run(self):
        legit_manifest = self._seed_legit_and_forged()
        before = _snapshot(self.repo)

        code, out, err = _run_hint(self.repo, "push", "--manifest", legit_manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertNotIn("DRY-RUN", out)
        self.assertEqual(_snapshot(self.repo), before)


class TestForgedFooterVariants(_IsolatedHintRepoTestCase):
    """One legitimate tag (used as the --manifest every command is invoked with) plus a SECOND tag
    whose evidence is forged/drifted in exactly one way per test -- verify/reindex must both fail
    closed, and reindex must never partially rewrite the index around the bad tag."""

    LEGIT_TAG = "hint/5.1.0-test/hint-bind-model/legit-arch"
    LEGIT_TOPOLOGY = "single 1노드"
    LEGIT_IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "5.1.0-test",
                       "quant": "bf16", "topology": "single", "tp": 1}
    SECOND_TAG = "hint/5.1.0-test/hint-bind-model/second-arch"

    def setUp(self):
        super().setUp()
        self.legit_manifest = _finalize_bound_tag(self.repo, tag=self.LEGIT_TAG, topology=self.LEGIT_TOPOLOGY,
                                                    identity=self.LEGIT_IDENTITY, manifest_name="legit.json")

    def _assert_verify_and_reindex_reject(self, expect_substr: str | None = None) -> None:
        idx_before = (self.repo / "hints" / "index.json").read_text(encoding="utf-8")

        code, out, err = _run_hint(self.repo, "verify", "--manifest", self.legit_manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        if expect_substr:
            self.assertIn(expect_substr, out)

        code, out, err = _run_hint(self.repo, "reindex", "--manifest", self.legit_manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual((self.repo / "hints" / "index.json").read_text(encoding="utf-8"), idx_before,
                          msg="reindex must not partially rewrite index.json around the bad tag")

    def test_referenced_manifest_missing(self):
        _craft_tag_with_fabricated_refs(self.repo, self.SECOND_TAG,
                                         manifest_ref="manifests/totally_missing.json",
                                         certificate_ref="evidence/totally_missing.yaml")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_manifest_outside_repo_escape(self):
        _craft_tag_with_fabricated_refs(self.repo, self.SECOND_TAG,
                                         manifest_ref="../outside.json",
                                         certificate_ref="evidence/whatever.yaml")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_manifest_absolute_path(self):
        _craft_tag_with_fabricated_refs(self.repo, self.SECOND_TAG,
                                         manifest_ref="/etc/passwd",
                                         certificate_ref="evidence/whatever.yaml")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_manifest_symlink(self):
        outside_dir = Path(self._tmp.name) / "outside"
        outside_dir.mkdir()
        outside_target = outside_dir / "real.json"
        outside_target.write_text("{}", encoding="utf-8")
        link_path = self.repo / "manifests" / "sneaky_link.json"
        link_path.parent.mkdir(parents=True, exist_ok=True)
        link_path.symlink_to(outside_target)
        _craft_tag_with_fabricated_refs(self.repo, self.SECOND_TAG,
                                         manifest_ref="manifests/sneaky_link.json",
                                         certificate_ref="evidence/whatever.yaml")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_manifest_malformed_json(self):
        _craft_tag_with_raw_manifest_bytes(self.repo, self.SECOND_TAG,
                                            manifest_rel="manifests/malformed.json",
                                            manifest_bytes=b"{ this is not json")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_manifest_not_promotion_ready(self):
        capped = json.dumps({
            "schema_version": 1, "task_class": "read_only_audit",
            "identity": dict(self.LEGIT_IDENTITY),
            "evidence": {}, "benchmark": {"mode": None, "verdict": None},
            "pii_scan": {"passed": True, "scanned_paths": []},
        }, indent=2).encode("utf-8")
        _craft_tag_with_raw_manifest_bytes(self.repo, self.SECOND_TAG,
                                            manifest_rel="manifests/capped.json",
                                            manifest_bytes=capped)
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_manifest_bytes_changed_after_binding(self):
        second_identity = dict(self.LEGIT_IDENTITY, vllm="5.1.0-test")
        pt = {"kind": "hint", "tag": self.SECOND_TAG, "topology": self.LEGIT_TOPOLOGY, "anchor": _head_sha(self.repo)}
        manifest_rel = _craft_bound_tag_with_real_evidence(
            self.repo, self.SECOND_TAG, manifest_identity=second_identity,
            manifest_promotion_target=pt, manifest_name="second.json")
        mpath = self.repo / manifest_rel
        mpath.write_text(mpath.read_text(encoding="utf-8") + "\n", encoding="utf-8")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_certificate_missing_after_binding(self):
        second_identity = dict(self.LEGIT_IDENTITY, vllm="5.1.0-test")
        pt = {"kind": "hint", "tag": self.SECOND_TAG, "topology": self.LEGIT_TOPOLOGY, "anchor": _head_sha(self.repo)}
        manifest_rel = _craft_bound_tag_with_real_evidence(
            self.repo, self.SECOND_TAG, manifest_identity=second_identity,
            manifest_promotion_target=pt, manifest_name="second.json")
        cert_rel = json.loads((self.repo / manifest_rel).read_text(encoding="utf-8"))["evidence"]["certificate"]["path"]
        (self.repo / manifest_rel).parent.joinpath(cert_rel).unlink()
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_certificate_changed_after_binding(self):
        second_identity = dict(self.LEGIT_IDENTITY, vllm="5.1.0-test")
        pt = {"kind": "hint", "tag": self.SECOND_TAG, "topology": self.LEGIT_TOPOLOGY, "anchor": _head_sha(self.repo)}
        manifest_rel = _craft_bound_tag_with_real_evidence(
            self.repo, self.SECOND_TAG, manifest_identity=second_identity,
            manifest_promotion_target=pt, manifest_name="second.json")
        cert_rel = json.loads((self.repo / manifest_rel).read_text(encoding="utf-8"))["evidence"]["certificate"]["path"]
        cert_path = (self.repo / manifest_rel).parent.joinpath(cert_rel)
        cert_path.write_text(cert_path.read_text(encoding="utf-8") + "# tampered\n", encoding="utf-8")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_referenced_manifest_promotion_target_points_elsewhere(self):
        second_identity = dict(self.LEGIT_IDENTITY, vllm="5.1.0-test")
        unrelated_pt = {"kind": "hint", "tag": "hint/9.9.9-test/hint-bind-model/unrelated-arch",
                         "topology": self.LEGIT_TOPOLOGY, "anchor": _head_sha(self.repo)}
        _craft_bound_tag_with_real_evidence(
            self.repo, self.SECOND_TAG, manifest_identity=second_identity,
            manifest_promotion_target=unrelated_pt, manifest_name="second.json")
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)

    def test_footer_anchor_does_not_match_actual_tag_commit(self):
        actual_commit = _head_sha(self.repo)
        wrong_anchor = ("0" if actual_commit[0] != "0" else "1") + actual_commit[1:]
        second_identity = dict(self.LEGIT_IDENTITY, vllm="5.1.0-test")
        pt = {"kind": "hint", "tag": self.SECOND_TAG, "topology": self.LEGIT_TOPOLOGY, "anchor": wrong_anchor}
        _craft_bound_tag_with_real_evidence(
            self.repo, self.SECOND_TAG, manifest_identity=second_identity,
            manifest_promotion_target=pt, manifest_name="second.json",
            footer_overrides={"anchor": wrong_anchor}, tag_commit=actual_commit)
        self._assert_verify_and_reindex_reject(self.SECOND_TAG)


class TestTwoTagsSelfVerifyIndependently(_IsolatedHintRepoTestCase):
    """Positive control: two legitimately bound tags, each with its OWN distinct referenced
    manifest -- verify/reindex/push, invoked with only ONE of the two manifests as the commanding
    --manifest, must still succeed for BOTH tags, because each self-verifies from its own footer."""

    TAG_A = "hint/6.0.0-test/hint-bind-model/arch-a"
    TAG_B = "hint/6.0.0-test/hint-bind-model/arch-b"
    TOPOLOGY = "single 1노드"
    IDENTITY = {"model": "hint-bind-model", "gpu": "GB10", "vllm": "6.0.0-test",
                "quant": "bf16", "topology": "single", "tp": 1}

    def test_verify_reindex_push_succeed_via_each_tags_own_footer(self):
        manifest_a = _finalize_bound_tag(self.repo, tag=self.TAG_A, topology=self.TOPOLOGY,
                                          identity=self.IDENTITY, manifest_name="a.json")
        manifest_b = _finalize_bound_tag(self.repo, tag=self.TAG_B, topology=self.TOPOLOGY,
                                          identity=self.IDENTITY, manifest_name="b.json")
        self.assertNotEqual(manifest_a, manifest_b)

        code, out, err = _run_hint(self.repo, "verify", "--manifest", manifest_a)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

        code, out, err = _run_hint(self.repo, "reindex", "--manifest", manifest_a)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        idx = json.loads((self.repo / "hints" / "index.json").read_text(encoding="utf-8"))
        self.assertEqual({e["tag"] for e in idx["hints"]}, {self.TAG_A, self.TAG_B})

        code, out, err = _run_hint(self.repo, "push", "--manifest", manifest_a)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("DRY-RUN", out)


if __name__ == "__main__":
    unittest.main()
