"""tests/harness/test_sync_to_sub_gate.py -- Phase 3 (plan_26072506) vertical slice 2C TDD suite:
wires `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh` to the shared
`completion_gate.py authorize --mode experimental|promotion --action sync_to_sub` contract already
covered in isolation by tests/harness/test_promotion_side_effects.py (vertical slice 1) and wired
into two sibling scripts by test_hint_tag_promotion_gate.py / test_sync_branches_gate.py (vertical
slices 2A/2B).

Hermetic design: every test builds its OWN throwaway git repository under a fresh tempdir that
reproduces sync_to_sub.sh's real on-disk nesting (`.claude/skills/upstream-version-watch/scripts/`,
4 levels below repo root -- the script derives its own repo root from this exact layout via
BASH_SOURCE), copies just the scripts/schemas this feature needs (read-only copies out of
REPO_ROOT -- this suite never writes into REPO_ROOT itself), and invokes the COPIED
sync_to_sub.sh as a real subprocess.

sync_to_sub.sh's real job is to `ssh`/`rsync` a genuine remote subnode -- this suite must NEVER
attempt that. Every test therefore prepends a `fakebin/` directory to PATH containing stand-in
`ssh`/`rsync` executables that only append an invocation record to a private log file and exit 0
(never touching the network). A denied/invalid authorization run must produce ZERO lines in that
log -- that is the suite's primary proof that the gate runs strictly before any transport is
attempted. An approved dry-run run is expected to produce at least one `ssh` line (the existing
`echo ok` pre-flight probe) and no real connection is ever made because `ssh` itself is fake.

Runner: stdlib `unittest` (same as the three sibling gate suites -- pytest is unavailable in this
environment and installing new dependencies is out of scope).
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SYNC_TO_SUB_REL = Path(".claude") / "skills" / "upstream-version-watch" / "scripts" / "sync_to_sub.sh"
SYNC_TO_SUB_SRC = REPO_ROOT / SYNC_TO_SUB_REL
RENDER_DOCKERFILE_SRC = REPO_ROOT / ".claude/skills/upstream-version-watch/scripts/render_dockerfile.py"
RUNNER_ASSET_DIR = REPO_ROOT / ".claude/skills/upstream-version-watch/assets/configs"
RUNNER_NAMES = ("serve_runner.sh", "debug-init.sh", "arm_patch.sh")
COMPLETION_GATE_SRC = REPO_ROOT / "scripts" / "completion_gate.py"
SCHEMA_SRC_DIR = REPO_ROOT / ".claude" / "schemas"
SCHEMA_NAMES = (
    "work-manifest.schema.json",
    "completion-manifest.schema.json",
    "side-effect-authorization.schema.json",
)

IDENTITY = {
    "model": "sync-to-sub-gate-test-model", "gpu": "GB10", "vllm": "0.97.0-test",
    "quant": "bf16", "topology": "multi", "tp": 2,
}

FAKE_SUB_HOST = "faketestuser@fake-sub-host.invalid"

FAKE_SSH_SCRIPT = """#!/bin/bash
# hermetic test double -- records the invocation and always succeeds; NEVER touches the network.
printf 'ssh %s\\n' "$*" >> "${FAKE_TRANSPORT_LOG:?FAKE_TRANSPORT_LOG not set}"
exit 0
"""

FAKE_RSYNC_SCRIPT = """#!/bin/bash
# hermetic test double -- records the invocation and always succeeds; NEVER touches the network.
printf 'rsync %s\\n' "$*" >> "${FAKE_TRANSPORT_LOG:?FAKE_TRANSPORT_LOG not set}"
exit 0
"""

RENDER_STUB_SCRIPT = """#!/usr/bin/env python3
# hermetic test stub for render_sub_env.py -- no-op regardless of args (--topology <t>).
import sys
sys.exit(0)
"""


# =============================================================================
# Isolated-repo + fake-transport scaffolding
# =============================================================================

def _git(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=str(repo), capture_output=True, text=True, timeout=30)


def _build_isolated_repo(root_tmp: Path) -> Path:
    """Reproduces sync_to_sub.sh's real relative nesting so its own BASH_SOURCE-derived repo-root
    resolution (`cd "$(dirname BASH_SOURCE)/../../../.." `) lands on THIS throwaway repo, never
    the real project."""
    repo = root_tmp / "repo"
    repo.mkdir()
    assert _git(repo, "init", "-q").returncode == 0, "git init unsupported by this git?"
    _git(repo, "config", "user.name", "Test Syncer")
    _git(repo, "config", "user.email", "test-syncer@example.invalid")
    _git(repo, "config", "commit.gpgsign", "false")

    script_dir = repo / SYNC_TO_SUB_REL.parent
    script_dir.mkdir(parents=True)
    shutil.copy(SYNC_TO_SUB_SRC, script_dir / "sync_to_sub.sh")
    shutil.copy(RENDER_DOCKERFILE_SRC, script_dir / "render_dockerfile.py")

    runner_dir = repo / ".claude" / "skills" / "upstream-version-watch" / "assets" / "configs"
    runner_dir.mkdir(parents=True)
    for name in RUNNER_NAMES:
        shutil.copy(RUNNER_ASSET_DIR / name, runner_dir / name)
        (runner_dir / name).chmod(0o755)

    (repo / "scripts").mkdir()
    shutil.copy(COMPLETION_GATE_SRC, repo / "scripts" / "completion_gate.py")

    (repo / ".claude" / "schemas").mkdir(parents=True, exist_ok=True)
    for name in SCHEMA_NAMES:
        shutil.copy(SCHEMA_SRC_DIR / name, repo / ".claude" / "schemas" / name)

    render_dir = repo / ".claude" / "skills" / "terraforming_node" / "scripts"
    render_dir.mkdir(parents=True)
    (render_dir / "render_sub_env.py").write_text(RENDER_STUB_SCRIPT, encoding="utf-8")

    (repo / "README.md").write_text("isolated test repo -- not the real project\n", encoding="utf-8")

    _git(repo, "add", "-A")
    commit = _git(repo, "commit", "-q", "-m", "init")
    assert commit.returncode == 0, commit.stderr
    return repo


def _build_fake_transport_bin(root_tmp: Path) -> Path:
    bindir = root_tmp / "fakebin"
    bindir.mkdir()
    ssh_path = bindir / "ssh"
    rsync_path = bindir / "rsync"
    ssh_path.write_text(FAKE_SSH_SCRIPT, encoding="utf-8")
    rsync_path.write_text(FAKE_RSYNC_SCRIPT, encoding="utf-8")
    ssh_path.chmod(0o755)
    rsync_path.chmod(0o755)
    return bindir


def _snapshot(repo: Path) -> tuple:
    """Side-effect fingerprint over everything a gated sync_to_sub.sh --apply could mutate
    LOCALLY (remote/subnode mutation is separately proven absent via the fake-transport log)."""
    status = _git(repo, "status", "--porcelain").stdout
    head = _git(repo, "rev-parse", "HEAD").stdout.strip()
    return (status, head)


def _transport_log_lines(transport_log: Path) -> list:
    if not transport_log.is_file():
        return []
    return [line for line in transport_log.read_text(encoding="utf-8").splitlines() if line.strip()]


class _IsolatedSyncToSubRepoTestCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="sync_to_sub_gate_test_")
        root_tmp = Path(self._tmp.name)
        self.repo = _build_isolated_repo(root_tmp)
        self.fakebin = _build_fake_transport_bin(root_tmp)
        self.transport_log = root_tmp / "transport.log"

    def tearDown(self):
        self._tmp.cleanup()

    def _run_sync(self, *args, cwd: Path | None = None, env: dict | None = None,
                  timeout: int = 60) -> tuple:
        """Invokes the copied sync_to_sub.sh with the fake ssh/rsync bin dir FIRST on PATH and
        FAKE_TRANSPORT_LOG wired -- every test gets this safety net by default, whether or not it
        expects the gate to ever reach the transport layer."""
        full_env = dict(os.environ)
        full_env["PATH"] = f"{self.fakebin}{os.pathsep}{full_env.get('PATH', '')}"
        full_env["FAKE_TRANSPORT_LOG"] = str(self.transport_log)
        if env:
            full_env.update(env)
        script_path = self.repo / SYNC_TO_SUB_REL
        proc = subprocess.run(
            ["bash", str(script_path), *args],
            cwd=str(cwd or self.repo), capture_output=True, text=True, timeout=timeout, env=full_env,
        )
        return proc.returncode, proc.stdout, proc.stderr

    def _assert_no_transport(self):
        self.assertEqual(_transport_log_lines(self.transport_log), [],
                          msg="denied/invalid authorization must never invoke ssh/rsync")


# =============================================================================
# Manifest builders (adapted from test_sync_branches_gate.py's fixtures, action=sync_to_sub)
# =============================================================================

def _write_text_evidence(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")


def _write_experimental_manifest(
    repo: Path, name: str, *, include_approval: bool = True, approved: bool = True,
    allowed_actions=("sync_to_sub",), manifest_dir: Path | None = None,
) -> str:
    """task_class=harness_change, minimal evidence, real on-disk plan.md (execution_approval.
    plan_path must resolve as a safe, existing, non-empty regular file). Returns the manifest path
    relative to `repo` (callers pass this to --manifest as-is or reconstruct a caller-CWD-relative
    form)."""
    manifest_dir = manifest_dir if manifest_dir is not None else (repo / "manifests")
    approved_at = "2026-07-25T06:00:00Z"
    plan_lines = ["# plan", "", "isolated sync-to-sub test plan.",
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
    manifest_dir = repo / "manifests"
    manifest_dir.mkdir(exist_ok=True)
    _write_text_evidence(manifest_dir / "evidence" / "plan.md", "# plan\n\nisolated sync-to-sub test plan body.\n")
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
    rel = _write_promotion_manifest_full_pass(repo, name="_pii_source.json")
    data = json.loads((repo / rel).read_text(encoding="utf-8"))
    data["pii_scan"]["passed"] = False
    out_path = repo / "manifests" / name
    out_path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    return str(out_path.relative_to(repo))


def _write_promotion_manifest_evidence_incomplete(repo: Path, name: str = "promo_evidence_incomplete.json") -> str:
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
# Section A -- missing --mode / --manifest / unknown arg -> stable failure, zero side effect
# =============================================================================

class TestMissingModeOrManifestStableFailure(_IsolatedSyncToSubRepoTestCase):
    def test_missing_both_flags(self):
        before = _snapshot(self.repo)
        code, out, err = self._run_sync()
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_missing_mode_only(self):
        manifest = _write_experimental_manifest(self.repo, "missing_mode.json")
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_missing_manifest_only(self):
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "experimental")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_unknown_arg_fails_stable(self):
        manifest = _write_experimental_manifest(self.repo, "unknown_arg.json")
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "experimental", "--manifest", manifest, "--frobnicate")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()


# =============================================================================
# Section B -- experimental mode: absent / false / unscoped approval -> denied, zero mutation,
# zero fake-transport invocation (SUB_HOST unset, no output/multi/manifest.yaml -- this doubles as
# the "denial wins over missing/bogus SUB_HOST/output manifest" proof: if host resolution ran
# first, these would fail with "서브노드 주소 미해소"/exit 4 plain text instead of gate JSON).
# =============================================================================

class TestExperimentalDeniedZeroMutation(_IsolatedSyncToSubRepoTestCase):
    def test_absent_execution_approval_block_denied(self):
        manifest = _write_experimental_manifest(self.repo, "absent_approval.json", include_approval=False)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "experimental", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_ABSENT", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_approved_false_denied(self):
        manifest = _write_experimental_manifest(self.repo, "approved_false.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "experimental", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_NOT_APPROVED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_unscoped_action_denied(self):
        manifest = _write_experimental_manifest(self.repo, "unscoped.json", allowed_actions=("sync_branches",))
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "experimental", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_ACTION_NOT_ALLOWED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_denied_even_with_apply_flag(self):
        manifest = _write_experimental_manifest(self.repo, "approved_false_apply.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "experimental", "--manifest", manifest, "--apply")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_denied_manifest_with_bogus_sub_host_reports_gate_denial(self):
        """Denial must win even when SUB_HOST is set to a bogus, unreachable value -- proving the
        gate runs BEFORE host resolution/preflight, not merely that it happens to run when
        SUB_HOST is unset."""
        manifest = _write_experimental_manifest(self.repo, "denied_bogus_host.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync(
            "--mode", "experimental", "--manifest", manifest,
            env={"SUB_HOST": "totally-bogus-user@totally-bogus-host.invalid.example"},
        )
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("EXECUTION_APPROVAL_NOT_APPROVED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertNotIn("서브노드 주소 미해소", out + err)
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()


# =============================================================================
# Section C -- experimental mode approved -> reaches existing preflight (fake ssh invoked),
# dry-run preview, never touching real network.
# =============================================================================

class TestExperimentalApprovedReachesPreflight(_IsolatedSyncToSubRepoTestCase):
    def test_approved_manifest_reaches_dryrun_preview_via_fake_transport(self):
        manifest = _write_experimental_manifest(self.repo, "approved.json")
        code, out, err = self._run_sync(
            "--mode", "experimental", "--manifest", manifest,
            env={"SUB_HOST": FAKE_SUB_HOST},
        )
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("DRY-RUN", out, msg=f"out={out}")
        lines = _transport_log_lines(self.transport_log)
        self.assertTrue(lines, msg="approved run must reach the existing ssh pre-flight (fake transport)")
        self.assertTrue(any(line.startswith("ssh ") for line in lines), msg=f"lines={lines}")
        # the fake ssh never actually connects anywhere real -- FAKE_SUB_HOST is not a resolvable
        # host, so the ONLY way this test passed is via the fake binary.
        self.assertIn(FAKE_SUB_HOST, "\n".join(lines))


# =============================================================================
# Section D -- promotion mode: runtime / evidence / PII denial -> zero mutation, zero transport
# =============================================================================

class TestPromotionDeniedZeroMutation(_IsolatedSyncToSubRepoTestCase):
    def test_runtime_ready_only_denied(self):
        manifest = _write_promotion_manifest_runtime_only(self.repo)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "promotion", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_evidence_incomplete_denied(self):
        manifest = _write_promotion_manifest_evidence_incomplete(self.repo)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "promotion", "--manifest", manifest)
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PROMOTION_GATE_NOT_ELIGIBLE", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertIn("EVIDENCE_MISSING:bench_report", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_pii_scan_failed_denied(self):
        manifest = _write_promotion_manifest_pii_failed(self.repo)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync("--mode", "promotion", "--manifest", manifest, "--apply")
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        self.assertIn("PII_SCAN_FAILED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()


# =============================================================================
# Section E -- promotion-ready manifest -> dry-run positive via fake transport
# =============================================================================

class TestPromotionReadyReachesPreflight(_IsolatedSyncToSubRepoTestCase):
    def test_full_pass_manifest_reaches_dryrun_preview_via_fake_transport(self):
        manifest = _write_promotion_manifest_full_pass(self.repo)
        code, out, err = self._run_sync(
            "--mode", "promotion", "--manifest", manifest,
            env={"SUB_HOST": FAKE_SUB_HOST},
        )
        self.assertEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertIn("DRY-RUN", out, msg=f"out={out}")
        lines = _transport_log_lines(self.transport_log)
        self.assertTrue(any(line.startswith("ssh ") for line in lines), msg=f"lines={lines}")


# =============================================================================
# Section F -- apply denial -> zero local changes AND zero fake-transport invocation
# =============================================================================

class TestApplyDenialZeroChanges(_IsolatedSyncToSubRepoTestCase):
    def test_experimental_denied_apply_zero_changes(self):
        manifest = _write_experimental_manifest(self.repo, "denied_apply.json", approved=False)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync(
            "--mode", "experimental", "--manifest", manifest, "--apply",
            env={"SUB_HOST": FAKE_SUB_HOST},
        )
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()

    def test_promotion_denied_apply_zero_changes(self):
        manifest = _write_promotion_manifest_runtime_only(self.repo)
        before = _snapshot(self.repo)
        code, out, err = self._run_sync(
            "--mode", "promotion", "--manifest", manifest, "--apply",
            env={"SUB_HOST": FAKE_SUB_HOST},
        )
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()


# =============================================================================
# Section G -- caller-CWD manifest resolution + quoting: a relative --manifest path must resolve
# against the directory the script was INVOKED from (which itself contains a space), never the
# script's own on-disk location or the repo root.
# =============================================================================

class TestCallerCwdManifestResolution(_IsolatedSyncToSubRepoTestCase):
    def test_relative_manifest_resolves_against_caller_cwd_with_spaces(self):
        caller_dir = self.repo / "caller dir"
        manifest_subdir = caller_dir / "manifests with space"
        manifest_subdir.mkdir(parents=True)
        rel_from_repo = _write_experimental_manifest(
            self.repo, "mymanifest.json", approved=False, manifest_dir=manifest_subdir,
        )
        # Sanity: the manifest must NOT be reachable via a repo-root-relative guess of the same
        # leaf name (proves the test genuinely distinguishes caller-CWD resolution from any other
        # resolution base).
        self.assertFalse((self.repo / "manifests" / "mymanifest.json").exists())
        self.assertTrue((self.repo / rel_from_repo).is_file())

        before = _snapshot(self.repo)
        code, out, err = self._run_sync(
            "--mode", "experimental", "--manifest", "manifests with space/mymanifest.json",
            cwd=caller_dir,
        )
        self.assertNotEqual(code, 0, msg=f"out={out}\nerr={err}")
        parsed = json.loads(out)
        self.assertFalse(parsed.get("allowed"), msg=f"parsed={parsed}")
        # A wrong resolution base would surface as MANIFEST_FILE_NOT_FOUND (file genuinely
        # missing there) instead of the manifest's real, intentional denial reason -- so getting
        # EXECUTION_APPROVAL_NOT_APPROVED here is the proof the file WAS found at the caller-CWD-
        # relative location.
        self.assertIn("EXECUTION_APPROVAL_NOT_APPROVED", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertNotIn("MANIFEST_FILE_NOT_FOUND", parsed.get("reason_codes", []), msg=f"parsed={parsed}")
        self.assertEqual(_snapshot(self.repo), before)
        self._assert_no_transport()


# =============================================================================
# Section H -- --help / -h exits 0 before ANY repo/manifest/SUB_HOST/SSH check
# =============================================================================

class TestHelpExitsZeroBeforeAnyCheck(unittest.TestCase):
    def _run_standalone_help(self, flag: str) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory(prefix="sync_to_sub_help_test_") as tmp:
            standalone_dir = Path(tmp) / "standalone"
            standalone_dir.mkdir()
            standalone_script = standalone_dir / "sync_to_sub.sh"
            shutil.copy(SYNC_TO_SUB_SRC, standalone_script)
            fakebin = Path(tmp) / "fakebin"
            fakebin.mkdir()
            # Deliberately NOT even executable stand-ins here -- if --help attempted to invoke
            # ssh/rsync/python3/git at all before exiting, this would surface as a hard failure
            # rather than a silent success.
            (fakebin / "ssh").write_text("#!/bin/bash\nexit 99\n", encoding="utf-8")
            (fakebin / "ssh").chmod(0o755)
            (fakebin / "rsync").write_text("#!/bin/bash\nexit 99\n", encoding="utf-8")
            (fakebin / "rsync").chmod(0o755)
            env = dict(os.environ)
            env["PATH"] = f"{fakebin}{os.pathsep}{env.get('PATH', '')}"
            # Deliberately NOT a git repository at all (no git init anywhere under `tmp`), and no
            # SUB_HOST/manifest/schemas/gate script present -- if --help touched any of those,
            # this fails loudly instead of silently.
            proc = subprocess.run(["bash", str(standalone_script), flag],
                                   cwd=str(standalone_dir), capture_output=True, text=True,
                                   timeout=30, env=env)
            return proc

    def test_help_long_flag(self):
        proc = self._run_standalone_help("--help")
        self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}\nstderr={proc.stderr}")
        self.assertNotIn("Traceback", proc.stderr)
        self.assertNotIn("서브노드 주소 미해소", proc.stdout + proc.stderr)

    def test_help_short_flag(self):
        proc = self._run_standalone_help("-h")
        self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}\nstderr={proc.stderr}")
        self.assertNotIn("Traceback", proc.stderr)


if __name__ == "__main__":
    unittest.main()
