"""tests/harness/test_sync_branches_gate.py -- Phase 3 (plan_26072506) vertical slice 2B TDD suite:
wires scripts/sync_branches.sh to the shared `completion_gate.py authorize --mode
experimental|promotion --action sync_branches` contract already covered in isolation by
tests/harness/test_promotion_side_effects.py (vertical slice 1).

Hermetic design: every test builds its OWN throwaway git repository under a fresh tempdir with
TWO real branches (main = SRC_BRANCH default, multi-node = DST_BRANCH default), copies just the
scripts/schemas this feature needs from the real repo (read-only copies out of REPO_ROOT -- this
suite never writes into REPO_ROOT itself), and invokes the COPIED scripts/sync_branches.sh as a
real subprocess. sync_branches.sh resolves its own repo root from its ON-DISK location (not the
caller's cwd), so every git checkout/status/HEAD mutation this suite exercises -- including the
one positive apply control -- happens ONLY inside that throwaway repo, never against the real
project's branches.

Runner: stdlib `unittest` (same as tests/harness/test_promotion_side_effects.py and
test_hint_tag_promotion_gate.py -- pytest is unavailable in this environment and installing new
dependencies is out of scope).
"""
from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SYNC_BRANCHES_SRC = REPO_ROOT / "scripts" / "sync_branches.sh"
COMPLETION_GATE_SRC = REPO_ROOT / "scripts" / "completion_gate.py"
SCHEMA_SRC_DIR = REPO_ROOT / ".claude" / "schemas"
SCHEMA_NAMES = (
    "work-manifest.schema.json",
    "completion-manifest.schema.json",
    "side-effect-authorization.schema.json",
)

IDENTITY = {
    "model": "sync-branches-gate-test-model", "gpu": "GB10", "vllm": "0.98.0-test",
    "quant": "bf16", "topology": "single", "tp": 1,
}

MAIN_CLAUDE_MD = "# CLAUDE.md (main -- canonical)\nMAIN-VERSION-CONTENT\n"
STALE_MULTI_CLAUDE_MD = "# CLAUDE.md (multi-node -- stale)\nMULTI-VERSION-CONTENT-STALE\n"


# =============================================================================
# Isolated-repo scaffolding
# =============================================================================

def _git(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=str(repo), capture_output=True, text=True, timeout=30)


def _run_sync(repo: Path, *args: str, cwd: Path | None = None, env: dict | None = None,
              timeout: int = 60) -> tuple[int, str, str]:
    import os
    full_env = dict(os.environ)
    if env:
        full_env.update(env)
    proc = subprocess.run(
        ["bash", str(repo / "scripts" / "sync_branches.sh"), *args],
        cwd=str(cwd or repo), capture_output=True, text=True, timeout=timeout, env=full_env,
    )
    return proc.returncode, proc.stdout, proc.stderr


def _snapshot(repo: Path) -> tuple:
    """A side-effect fingerprint covering everything a gated sync_branches.sh --apply could
    mutate: tracked-file status (git status --porcelain, catches CLAUDE.md staged-copy), current
    HEAD sha (no commit should ever happen -- the script never commits), and CLAUDE.md's actual
    on-disk content (the one allowlisted file this suite deliberately diverges cross-branch)."""
    status = _git(repo, "status", "--porcelain").stdout
    head = _git(repo, "rev-parse", "HEAD").stdout.strip()
    claude_md = (repo / "CLAUDE.md").read_text(encoding="utf-8") if (repo / "CLAUDE.md").is_file() else None
    return (status, head, claude_md)


def _build_isolated_repo(root_tmp: Path) -> Path:
    """Builds a throwaway repo with two real branches (main/multi-node) diverging on the ONE
    allowlisted file this suite cares about (CLAUDE.md), plus the copied gate machinery."""
    repo = root_tmp / "repo"
    repo.mkdir()
    assert _git(repo, "init", "-q", "-b", "main").returncode == 0, "git init -b main unsupported by this git?"
    _git(repo, "config", "user.name", "Test Syncer")
    _git(repo, "config", "user.email", "test-syncer@example.invalid")
    _git(repo, "config", "commit.gpgsign", "false")

    (repo / "scripts").mkdir(parents=True)
    (repo / ".claude" / "schemas").mkdir(parents=True)

    shutil.copy(SYNC_BRANCHES_SRC, repo / "scripts" / "sync_branches.sh")
    shutil.copy(COMPLETION_GATE_SRC, repo / "scripts" / "completion_gate.py")
    for name in SCHEMA_NAMES:
        shutil.copy(SCHEMA_SRC_DIR / name, repo / ".claude" / "schemas" / name)

    (repo / "CLAUDE.md").write_text(MAIN_CLAUDE_MD, encoding="utf-8")
    (repo / "README.md").write_text("isolated test repo -- not the real project\n", encoding="utf-8")

    _git(repo, "add", "-A")
    commit = _git(repo, "commit", "-q", "-m", "init on main")
    assert commit.returncode == 0, commit.stderr

    assert _git(repo, "checkout", "-q", "-b", "multi-node").returncode == 0
    (repo / "CLAUDE.md").write_text(STALE_MULTI_CLAUDE_MD, encoding="utf-8")
    _git(repo, "add", "-A")
    commit2 = _git(repo, "commit", "-q", "-m", "diverge on multi-node")
    assert commit2.returncode == 0, commit2.stderr
    # multi-node stays checked out -- sync_branches.sh's own default DST_BRANCH.
    return repo


class _IsolatedSyncRepoTestCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="sync_branches_gate_test_")
        self.repo = _build_isolated_repo(Path(self._tmp.name))

    def tearDown(self):
        self._tmp.cleanup()


# =============================================================================
# Manifest builders
# =============================================================================

def _write_text_evidence(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")


def _write_experimental_manifest(
    repo: Path, name: str, *, include_approval: bool = True, approved: bool = True,
    allowed_actions=("sync_branches",),
) -> str:
    """task_class=harness_change, minimal evidence -- the manifest shape covered by
    test_promotion_side_effects.py's experimental-mode fixtures, adapted to a real on-disk plan.md
    (execution_approval.plan_path must resolve as a safe, existing, non-empty regular file)."""
    manifest_dir = repo / "manifests"
    approved_at = "2026-07-25T05:00:00Z"
    plan_lines = ["# plan", "", "isolated sync-branches test plan.",
                  "## Execution approval", "approved_by: coag-ash",
                  f"approved_at_utc: {approved_at}",
                  *[f"allowed_action: {value}" for value in allowed_actions]]
    plan_path = manifest_dir / "evidence" / "plan.md"
    _write_text_evidence(plan_path, "\n".join(plan_lines) + "\n")
    plan_bytes = plan_path.read_bytes()
    manifest: dict = {
        "schema_version": 1, "task_class": "harness_change", "identity": dict(IDENTITY),
        "evidence": {
            "plan": {"path": "evidence/plan.md"}, "devlog": None, "testlog": None,
            "simlog": None, "bench_report": None, "certificate": None,
        },
        "pii_scan": {"passed": True, "scanned_paths": ["evidence/plan.md"]},
    }
    if include_approval:
        manifest["execution_approval"] = {
            "approved": approved, "approved_by": "coag-ash",
            "approved_at_utc": approved_at,
            "plan_path": "evidence/plan.md",
            "plan_sha256": hashlib.sha256(plan_bytes).hexdigest(),
            "plan_blob_sha1": hashlib.sha1(
                b"blob " + str(len(plan_bytes)).encode("ascii") + b"\0" + plan_bytes).hexdigest(),
            "approval_anchor": "## Execution approval",
            "approval_atoms": ["approved_by: coag-ash", f"approved_at_utc: {approved_at}",
                               *[f"allowed_action: {value}" for value in allowed_actions]],
            "allowed_actions": list(allowed_actions),
        }
    manifest_path = manifest_dir / name
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return str(manifest_path.relative_to(repo))


def _write_promotion_manifest_runtime_only(repo: Path, name: str = "promo_runtime_only.json") -> str:
    """Healthy runtime, zero evidence -- reaches runtime-ready at best; promotion authorize must
    deny with PROMOTION_GATE_NOT_ELIGIBLE, never touching any sync_branches.sh side effect."""
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


def _write_promotion_manifest_full_pass(repo: Path, name: str = "promo_full_pass.json") -> str:
    """task_class=full_benchmark, mode=full/verdict=PASS, every required evidence artifact
    genuinely present on disk (certificate identity fields match IDENTITY) -- reaches
    completion_gate.py verify state==promotion-ready."""
    manifest_dir = repo / "manifests"
    manifest_dir.mkdir(exist_ok=True)
    _write_text_evidence(manifest_dir / "evidence" / "plan.md", "# plan\n\nisolated sync-branches test plan body.\n")
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
    manifest_path = manifest_dir / name
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return str(manifest_path.relative_to(repo))


def _write_promotion_manifest_pii_failed(repo: Path, name: str = "promo_pii_failed.json") -> str:
    """Otherwise-complete evidence (same shape as the full-pass manifest) but pii_scan.passed is
    False -- the PII gate binds to evidence-complete itself, so this denies with PII_SCAN_FAILED
    before ever reaching promotion-ready."""
    rel = _write_promotion_manifest_full_pass(repo, name="_pii_source.json")
    data = json.loads((repo / rel).read_text(encoding="utf-8"))
    data["pii_scan"]["passed"] = False
    out_path = repo / "manifests" / name
    out_path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    return str(out_path.relative_to(repo))


def _write_promotion_manifest_evidence_incomplete(repo: Path, name: str = "promo_evidence_incomplete.json") -> str:
    """full_benchmark with SOME evidence present but bench_report missing -- evidence-tier denial
    (EVIDENCE_MISSING:bench_report), distinct from the runtime-only and PII-failure denials."""
    rel = _write_promotion_manifest_full_pass(repo, name="_evidence_source.json")
    data = json.loads((repo / rel).read_text(encoding="utf-8"))
    data["evidence"]["bench_report"] = None
    data["evidence"]["certificate"] = None
    data["benchmark"] = {"mode": "lite", "verdict": None}
    data["pii_scan"]["scanned_paths"] = [
        p for p in data["pii_scan"]["scanned_paths"]
        if "bench_report" not in p and "certificate" not in p
    ]
    out_path = repo / "manifests" / name
    out_path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    return str(out_path.relative_to(repo))


# =============================================================================
# Section A -- missing --mode / --manifest -> stable failure, zero side effect
# =============================================================================

class TestMissingModeOrManifestStableFailure(_IsolatedSyncRepoTestCase):
    def test_missing_both_flags(self):
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_missing_mode_only(self):
        manifest = _write_experimental_manifest(self.repo, "missing_mode.json")
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_missing_manifest_only(self):
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "experimental")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section B -- experimental mode: absent / false / unscoped approval -> denied, zero mutation
# =============================================================================

class TestExperimentalDeniedZeroMutation(_IsolatedSyncRepoTestCase):
    def test_absent_execution_approval_block_denied(self):
        manifest = _write_experimental_manifest(self.repo, "absent_approval.json", include_approval=False)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "experimental", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_ABSENT", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_approved_false_denied(self):
        manifest = _write_experimental_manifest(self.repo, "approved_false.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "experimental", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_NOT_APPROVED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_unscoped_action_denied(self):
        manifest = _write_experimental_manifest(self.repo, "unscoped.json", allowed_actions=("sync_to_sub",))
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "experimental", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_ACTION_NOT_ALLOWED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_denied_even_with_apply_flag(self):
        manifest = _write_experimental_manifest(self.repo, "approved_false_apply.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "experimental", "--manifest", manifest, "--apply")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section C -- experimental mode approved -> reaches dry-run preview
# =============================================================================

class TestExperimentalApprovedReachesDryrun(_IsolatedSyncRepoTestCase):
    def test_approved_manifest_reaches_dryrun_preview(self):
        manifest = _write_experimental_manifest(self.repo, "approved.json")
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "experimental", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("DRY-RUN", out, msg=f"out={out}")
        # dry-run must never mutate anything, even though it was authorized.
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section D -- promotion mode: runtime / evidence / PII denial -> zero mutation
# =============================================================================

class TestPromotionDeniedZeroMutation(_IsolatedSyncRepoTestCase):
    def test_runtime_ready_only_denied(self):
        manifest = _write_promotion_manifest_runtime_only(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "promotion", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_evidence_incomplete_denied(self):
        manifest = _write_promotion_manifest_evidence_incomplete(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "promotion", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertIn("EVIDENCE_MISSING:bench_report", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)

    def test_pii_scan_failed_denied(self):
        manifest = _write_promotion_manifest_pii_failed(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "promotion", "--manifest", manifest, "--apply")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PII_SCAN_FAILED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section E -- promotion-ready manifest -> dry-run positive
# =============================================================================

class TestPromotionReadyDryrunPositive(_IsolatedSyncRepoTestCase):
    def test_full_pass_manifest_reaches_dryrun_preview(self):
        manifest = _write_promotion_manifest_full_pass(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "promotion", "--manifest", manifest)
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("DRY-RUN", out, msg=f"out={out}")
        self.assertEqual(_snapshot(self.repo), before)


# =============================================================================
# Section F -- apply denial -> zero worktree/index/ref changes
# =============================================================================

class TestApplyDenialZeroChanges(_IsolatedSyncRepoTestCase):
    def test_experimental_denied_apply_zero_changes(self):
        manifest = _write_experimental_manifest(self.repo, "denied_apply.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "experimental", "--manifest", manifest, "--apply")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        after = _snapshot(self.repo)
        self.assertEqual(after, before, msg="denied --apply must leave worktree/index/HEAD untouched")

    def test_promotion_denied_apply_zero_changes(self):
        manifest = _write_promotion_manifest_runtime_only(self.repo)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "promotion", "--manifest", manifest, "--apply")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        after = _snapshot(self.repo)
        self.assertEqual(after, before, msg="denied --apply must leave worktree/index/HEAD untouched")


# =============================================================================
# Section G -- approved apply -> positive control, real git checkout in isolated repo only
# =============================================================================

class TestApprovedApplyPositiveControl(_IsolatedSyncRepoTestCase):
    def test_approved_experimental_apply_copies_allowlisted_claude_md(self):
        manifest = _write_experimental_manifest(self.repo, "approved_apply.json")
        before = _snapshot(self.repo)
        before_head = _git(self.repo, "rev-parse", "HEAD").stdout
        self.assertEqual((self.repo / "CLAUDE.md").read_text(encoding="utf-8"), STALE_MULTI_CLAUDE_MD)

        code, out, err = _run_sync(self.repo, "--mode", "experimental", "--manifest", manifest, "--apply")
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")

        after_content = (self.repo / "CLAUDE.md").read_text(encoding="utf-8")
        self.assertEqual(after_content, MAIN_CLAUDE_MD, msg="CLAUDE.md must now match main's (src) content")
        self.assertNotEqual(_snapshot(self.repo), before)

        status_after = _git(self.repo, "status", "--porcelain").stdout
        self.assertIn("CLAUDE.md", status_after, msg=f"status={status_after!r}")
        # HEAD must never move -- sync_branches.sh only stages, it never commits.
        self.assertEqual(_git(self.repo, "rev-parse", "HEAD").stdout, before_head)

    def test_head_oracle_detects_auto_commit_mutant(self):
        manifest = _write_experimental_manifest(self.repo, "auto_commit_mutant.json")
        script = self.repo / "scripts" / "sync_branches.sh"
        text = script.read_text(encoding="utf-8")
        needle = 'echo "[sync-branches] 완료 — 공유 빌딩블럭을 working-dir 에 반영했습니다(스테이징됨)."'
        mutant = (
            'git -c user.name="Mutant" -c user.email="mutant@example.invalid" '
            'commit -q -m "forbidden auto commit"\n' + needle
        )
        self.assertIn(needle, text)
        script.write_text(text.replace(needle, mutant, 1), encoding="utf-8")
        before_head = _git(self.repo, "rev-parse", "HEAD").stdout

        code, out, err = _run_sync(
            self.repo, "--mode", "experimental", "--manifest", manifest, "--apply"
        )
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertNotEqual(_git(self.repo, "rev-parse", "HEAD").stdout, before_head,
                            msg="negative control must prove that the pre/post HEAD oracle detects an auto-commit")

    def test_promotion_ready_apply_also_works(self):
        manifest = _write_promotion_manifest_full_pass(self.repo)
        code, out, err = _run_sync(self.repo, "--mode", "promotion", "--manifest", manifest, "--apply")
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual((self.repo / "CLAUDE.md").read_text(encoding="utf-8"), MAIN_CLAUDE_MD)


class TestPhase7SharedAllowlistContract(unittest.TestCase):
    def test_phase1_through_phase6_shared_roots_are_explicitly_allowlisted(self):
        text = SYNC_BRANCHES_SRC.read_text(encoding="utf-8")
        block = text.split("ALLOWLIST=(", 1)[1].split("\n)", 1)[0]
        entries = {line.strip() for line in block.splitlines()
                   if line.strip() and not line.lstrip().startswith("#")}
        required = {
            ".claude/schemas", ".claude/policies", ".gitignore", "tests",
            "scripts/completion_gate.py", "scripts/doc_naming.py",
            "scripts/evidence_publisher.py", "scripts/harness_verify.py",
            "scripts/policy_registry.py", "scripts/agent_control.py", "scripts/providers",
            "docs/plan/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md",
        }
        self.assertEqual(set(), required - entries,
                         msg=f"Phase1-6 shared paths missing from allowlist: {sorted(required - entries)}")
        self.assertIn(
            "immutable historical policy trust source; evidence input only, never an active execution plan",
            text,
        )
        self.assertNotIn(
            "docs/plan/plan_26072506_하네스_루프_구조개선_Sonnet_E2E.md", entries,
            msg="the active execution plan must not be branch-synced",
        )


# =============================================================================
# Section H -- --help exits 0 before ANY repo/branch check (standalone script, no enclosing repo)
# =============================================================================

class TestHelpExitsZeroBeforeRepoChecks(unittest.TestCase):
    def test_help_works_with_no_enclosing_git_repo(self):
        with tempfile.TemporaryDirectory(prefix="sync_branches_help_test_") as tmp:
            standalone_dir = Path(tmp) / "standalone"
            standalone_dir.mkdir()
            standalone_script = standalone_dir / "sync_branches.sh"
            shutil.copy(SYNC_BRANCHES_SRC, standalone_script)
            # Deliberately NOT a git repository at all (no git init anywhere under `tmp`) --
            # if --help attempted repo-root resolution or any branch check first, this fails.
            proc = subprocess.run(["bash", str(standalone_script), "--help"],
                                   cwd=str(standalone_dir), capture_output=True, text=True, timeout=30)
            self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}\nstderr={proc.stderr}")
            self.assertNotIn("Traceback", proc.stderr)

    def test_help_short_flag_also_works(self):
        with tempfile.TemporaryDirectory(prefix="sync_branches_help_test_") as tmp:
            standalone_dir = Path(tmp) / "standalone"
            standalone_dir.mkdir()
            standalone_script = standalone_dir / "sync_branches.sh"
            shutil.copy(SYNC_BRANCHES_SRC, standalone_script)
            proc = subprocess.run(["bash", str(standalone_script), "-h"],
                                   cwd=str(standalone_dir), capture_output=True, text=True, timeout=30)
            self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}\nstderr={proc.stderr}")


# =============================================================================
# Section I -- gate invoked BEFORE source-branch validation (authorization denial wins)
# =============================================================================

class TestGateWinsOverInvalidSrcBranch(_IsolatedSyncRepoTestCase):
    def test_denied_manifest_with_nonexistent_src_branch_reports_gate_denial(self):
        manifest = _write_experimental_manifest(self.repo, "denied_bogus_src.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = _run_sync(
            self.repo, "--mode", "experimental", "--manifest", manifest,
            env={"SRC_BRANCH": "totally-bogus-branch-does-not-exist"},
        )
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        # If source-branch validation ran BEFORE the gate, this would be a plain-text bash
        # failure ("정본 브랜치") instead of parseable authorize-shaped JSON.
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_NOT_APPROVED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertNotIn("정본 브랜치", out)
        self.assertEqual(_snapshot(self.repo), before)

    def test_approved_manifest_with_nonexistent_src_branch_fails_after_gate(self):
        """Sanity companion: once authorization is GRANTED, a genuinely-missing source branch must
        still fail (source-branch validation is not skipped, only reordered to run after the
        gate) -- this is not gate JSON, it's sync_branches.sh's own pre-flight failure."""
        manifest = _write_experimental_manifest(self.repo, "approved_bogus_src.json")
        before = _snapshot(self.repo)
        code, out, err = _run_sync(
            self.repo, "--mode", "experimental", "--manifest", manifest,
            env={"SRC_BRANCH": "totally-bogus-branch-does-not-exist"},
        )
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual(_snapshot(self.repo), before)


if __name__ == "__main__":
    unittest.main()
