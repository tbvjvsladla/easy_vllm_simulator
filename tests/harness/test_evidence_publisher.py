"""tests/harness/test_evidence_publisher.py -- TDD suite for scripts/doc_naming.py and
scripts/evidence_publisher.py (plan_26072506 Phase 2 -- Hybrid evidence publisher).

Runner: stdlib `unittest` (matches tests/harness/test_completion_gate.py convention -- pytest is
not installed in this environment; see that file's docstring for the same rationale).

Invocation:
    python3 -m unittest tests.harness.test_evidence_publisher -v
    python3 -m unittest discover -s tests/harness -p 'test_*.py' -v

Hermetic design: every test that needs a filesystem constructs its own `tempfile.TemporaryDirectory()`
as an isolated fake repo root (never writes inside the real repo tree, never depends on `.git`
auto-detection -- both scripts accept `--repo-root` explicitly for exactly this reason). This keeps
the real repo pristine across test runs and survives a `git checkout-index` clean-checkout export
unmodified (no fixture depends on real-repo state).

Section map:
  A -- doc_naming.py pure naming functions (white-box import)
  B -- evidence_publisher.required_evidence_for() parity with completion_gate (single SSOT, no
       duplicated divergent matrix)
  C -- `init` subcommand: scaffold + evidence/publication record creation (first run)
  D -- `init` idempotency (rerun preserves narrative/raw, no new files, stable JSON)
  E -- `append-raw` happy path (simlog copy + non-dir JSONL sidecar), atomic/append-only
  F -- `append-raw` rejections (absolute/escape/symlink/wrong-type/invalid-utf8/malformed) --
       one stable JSON, no traceback, exit 2
  G -- `set-narrative`: explicit-external-input-only, provenance, idempotent re-authoring, never
       fabricates
  H -- `publish-benchmark`: FAIL (bench_report scaffold, certificate omitted/rejected) / PASS
       (certificate ONLY from a supplied, parseable, verified artifact -- never synthesized)
  I -- `finalize`: delegates identity/link/PII/verdict checks to completion_gate.py (no
       reimplemented validators), relays its stable reason codes and exit code verbatim
  J -- `--self-test` CLI smoke
"""
from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
DOC_NAMING_SCRIPT = SCRIPTS_DIR / "doc_naming.py"
PUBLISHER_SCRIPT = SCRIPTS_DIR / "evidence_publisher.py"
GATE_SCRIPT = SCRIPTS_DIR / "completion_gate.py"

sys.path.insert(0, str(SCRIPTS_DIR))


# =============================================================================
# Section A -- doc_naming.py pure naming functions (white-box import)
# =============================================================================

class TestDocNamingKstTokens(unittest.TestCase):
    def test_known_utc_converts_to_kst_hour_token(self):
        import doc_naming
        self.assertEqual(doc_naming.kst_tokens("2026-07-24T16:22:29Z"), ("26072501", "22", "29"))

    def test_crosses_midnight_into_next_kst_day(self):
        import doc_naming
        self.assertEqual(doc_naming.kst_tokens("2026-07-15T21:21:33Z"), ("26071606", "21", "33"))

    def test_none_is_na_sentinel(self):
        import doc_naming
        self.assertEqual(doc_naming.kst_tokens(None), ("NA", "00", "00"))

    def test_unparseable_string_is_na_sentinel(self):
        import doc_naming
        self.assertEqual(doc_naming.kst_tokens("garbage"), ("NA", "00", "00"))


class TestDocNamingDatedDocBasename(unittest.TestCase):
    def test_plan_basename_no_collision(self):
        import doc_naming
        name = doc_naming.dated_doc_basename("plan", "2026-07-25T06:58:00Z", "하네스_루프_구조개선")
        self.assertEqual(name, "plan_26072515_하네스_루프_구조개선.md")

    def test_devlog_basename_no_collision(self):
        import doc_naming
        name = doc_naming.dated_doc_basename("devlog", "2026-06-08T05:00:00Z", "테스트")
        self.assertEqual(name, "devlog_26060814_테스트.md")

    def test_mm_ss_suffix_exact_value_on_collision(self):
        import doc_naming
        gen = "2026-07-25T06:58:29Z"  # -> hour token 26072515, mm=58, ss=29
        existing = {"testlog_26072515_구조개선.md"}
        name = doc_naming.dated_doc_basename("testlog", gen, "구조개선", existing_basenames=existing)
        self.assertEqual(name, "testlog_26072515_58_29_구조개선.md")

    def test_same_type_hour_different_topic_still_collides(self):
        """P2-A04 (reliability review subagent-summary-0-20260725_115851_232115): collision is
        keyed on (doc_type, hour) regardless of topic, per .claude/rules/docs.md 's 'YYMMDDHH 같은
        type 충돌' rule -- a same-type publication already occupying this hour must force the new
        one onto _MM_SS even though its own topic-based basename never literally matches any
        existing name. Supersedes the old exact-basename-only expectation this test used to
        encode."""
        import doc_naming
        gen = "2026-07-25T06:58:29Z"  # hour token 26072515, mm=58, ss=29
        existing = {"testlog_26072515_다른주제.md"}  # different topic, SAME type+hour
        name = doc_naming.dated_doc_basename("testlog", gen, "구조개선", existing_basenames=existing)
        self.assertEqual(name, "testlog_26072515_58_29_구조개선.md")

    def test_no_suffix_when_hour_bucket_empty(self):
        import doc_naming
        gen = "2026-07-25T06:58:29Z"
        existing = {"testlog_26060814_다른시각.md"}  # different hour entirely -> no collision
        name = doc_naming.dated_doc_basename("testlog", gen, "구조개선", existing_basenames=existing)
        self.assertEqual(name, "testlog_26072515_구조개선.md")

    def test_different_doc_type_same_hour_does_not_collide(self):
        import doc_naming
        gen = "2026-07-25T06:58:29Z"
        existing = {"plan_26072515_구조개선.md"}  # same hour, different doc_type -> no collision
        name = doc_naming.dated_doc_basename("testlog", gen, "구조개선", existing_basenames=existing)
        self.assertEqual(name, "testlog_26072515_구조개선.md")

    def test_rejects_unsupported_doc_type(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.dated_doc_basename("simlog", "2026-07-25T06:58:00Z", "topic")

    def test_rejects_empty_topic(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.dated_doc_basename("plan", "2026-07-25T06:58:00Z", "")

    def test_rejects_topic_with_path_separator(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.dated_doc_basename("plan", "2026-07-25T06:58:00Z", "../escape")

    def test_rejects_topic_with_space(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.dated_doc_basename("plan", "2026-07-25T06:58:00Z", "has space")


class TestDocNamingSimlogDirname(unittest.TestCase):
    def test_dirname_exact_value(self):
        import doc_naming
        name = doc_naming.simlog_dirname("2026-06-21T12:21:00Z", "vLLM0.22.1_KV클램프_시뮬")
        self.assertEqual(name, "26062121_vLLM0.22.1_KV클램프_시뮬")

    def test_collision_adds_mm_ss(self):
        import doc_naming
        gen = "2026-06-21T12:21:33Z"  # hour token 26062121
        existing = {"26062121_run"}
        name = doc_naming.simlog_dirname(gen, "run", existing_dirnames=existing)
        self.assertEqual(name, "26062121_21_33_run")

    def test_rejects_empty_topic(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.simlog_dirname("2026-06-21T12:21:00Z", "")

    def test_collision_adds_mm_ss_different_topic_same_hour(self):
        """P2-A04: simlog collision is keyed on the hour bucket alone -- a different-topic run
        directory already occupying this hour still forces the new one onto _MM_SS."""
        import doc_naming
        gen = "2026-06-21T12:21:33Z"  # hour token 26062121, mm=21, ss=33
        existing = {"26062121_other_run"}
        name = doc_naming.simlog_dirname(gen, "run", existing_dirnames=existing)
        self.assertEqual(name, "26062121_21_33_run")


class TestDocNamingBenchFilename(unittest.TestCase):
    META = {"model": "solar-open2-250b", "gpu_key": "GB10", "vllm_version": "0.22.0"}

    def test_bench_report_human_prefix(self):
        import doc_naming
        name = doc_naming.bench_filename("bench_report", self.META, "2026-07-24T16:22:29Z")
        self.assertEqual(name, "bench_report_26072501_solar-open2-250b_GB10_0.22.0.md")

    def test_certificate_prefix(self):
        import doc_naming
        name = doc_naming.bench_filename("benchmark", self.META, "2026-07-24T16:22:29Z")
        self.assertEqual(name, "benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml")

    def test_max_envelope_prefix(self):
        import doc_naming
        name = doc_naming.bench_filename("max_envelope", self.META, None)
        self.assertEqual(name, "max_envelope_NA_solar-open2-250b_GB10_0.22.0.md")

    def test_collision_adds_mm_ss(self):
        import doc_naming
        existing = {"bench_report_26072501_solar-open2-250b_GB10_0.22.0.md"}
        name = doc_naming.bench_filename("bench_report", self.META, "2026-07-24T16:22:29Z",
                                          existing_basenames=existing)
        self.assertEqual(name, "bench_report_26072501_22_29_solar-open2-250b_GB10_0.22.0.md")

    def test_rejects_unsupported_kind(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.bench_filename("plan", self.META, "2026-07-24T16:22:29Z")

    def test_collision_adds_mm_ss_different_combo_same_kind_hour(self):
        """P2-A04: bench collision is keyed on (kind, hour) -- a different model/gpu/vllm combo
        already published this hour still forces the new one onto _MM_SS ('동일 YYMMDDHH 다른
        측정=_MM_SS' per .claude/rules/docs.md)."""
        import doc_naming
        other_meta = {"model": "other-model", "gpu_key": "H200", "vllm_version": "0.25.0"}
        existing = {doc_naming.bench_filename("bench_report", other_meta, "2026-07-24T16:22:00Z")}
        name = doc_naming.bench_filename("bench_report", self.META, "2026-07-24T16:22:29Z",
                                          existing_basenames=existing)
        self.assertEqual(name, "bench_report_26072501_22_29_solar-open2-250b_GB10_0.22.0.md")

    def test_different_kind_same_hour_does_not_collide(self):
        import doc_naming
        existing = {"benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml"}
        name = doc_naming.bench_filename("bench_report", self.META, "2026-07-24T16:22:29Z",
                                          existing_basenames=existing)
        self.assertEqual(name, "bench_report_26072501_solar-open2-250b_GB10_0.22.0.md")


class TestDocNamingReportException(unittest.TestCase):
    def test_kebab_slug_no_date_token(self):
        import doc_naming
        name = doc_naming.report_basename("rtxpro6000-benchmark-explorer")
        self.assertEqual(name, "rtxpro6000-benchmark-explorer.html")

    def test_md_extension_supported(self):
        import doc_naming
        name = doc_naming.report_basename("claude-control-policy", ext="md")
        self.assertEqual(name, "claude-control-policy.md")

    def test_rejects_non_kebab_slug_with_underscore(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.report_basename("has_underscore")

    def test_rejects_uppercase_slug(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.report_basename("HasUpperCase")

    def test_rejects_empty_slug(self):
        import doc_naming
        with self.assertRaises(ValueError):
            doc_naming.report_basename("")


class TestDocNamingSelfTestCli(unittest.TestCase):
    def test_self_test_flag_exits_zero(self):
        proc = subprocess.run([sys.executable, str(DOC_NAMING_SCRIPT), "--self-test"],
                               capture_output=True, text=True, timeout=30)
        self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout!r} stderr={proc.stderr!r}")
        self.assertIn("PASS", proc.stdout + proc.stderr)


# =============================================================================
# Section B -- evidence_publisher.required_evidence_for() parity with completion_gate (single
# SSOT: literally the same function object, never a second hand-duplicated matrix)
# =============================================================================

class TestRequiredEvidenceParity(unittest.TestCase):
    def test_same_function_object_as_completion_gate(self):
        import completion_gate as gate
        import evidence_publisher as pub
        self.assertIs(
            pub.required_evidence_for, gate.required_evidence_for,
            msg="evidence_publisher must reuse completion_gate.required_evidence_for, not "
                "reimplement a second, divergence-prone copy",
        )

    def _cases(self):
        return [
            ("model_serving_strategy", {}, None, ["plan", "devlog", "simlog", "testlog"]),
            ("full_benchmark", {}, "FAIL", ["plan", "devlog", "simlog", "testlog", "bench_report"]),
            ("full_benchmark", {}, "PASS",
             ["plan", "devlog", "simlog", "testlog", "bench_report", "certificate"]),
            ("harness_change", {"actual_trial": True}, None, ["plan", "devlog", "testlog", "simlog"]),
            ("harness_change", {"actual_trial": False}, None, ["plan", "devlog", "testlog"]),
            ("harness_change", {}, None, ["plan", "devlog", "testlog"]),
            ("capacity_rejection", {}, None, ["plan"]),
            ("minor_patch", {}, None, ["verification", "commit"]),
            ("read_only_audit", {"report_requested": True}, None, ["report"]),
            ("read_only_audit", {"report_requested": False}, None, []),
            ("read_only_audit", {}, None, []),
        ]

    def test_matches_completion_gate_across_every_task_class_condition_combo(self):
        import completion_gate as gate
        import evidence_publisher as pub
        for task_class, conditions, verdict, _expected in self._cases():
            with self.subTest(task_class=task_class, conditions=conditions, verdict=verdict):
                self.assertEqual(
                    pub.required_evidence_for(task_class, conditions, verdict),
                    gate.required_evidence_for(task_class, conditions, verdict),
                )

    def test_exact_expected_lists(self):
        import evidence_publisher as pub
        for task_class, conditions, verdict, expected in self._cases():
            with self.subTest(task_class=task_class, conditions=conditions, verdict=verdict):
                self.assertEqual(pub.required_evidence_for(task_class, conditions, verdict), expected)


# =============================================================================
# Shared CLI-invocation helper for the publisher (black-box subprocess contract, matching
# test_completion_gate.py's `run_gate` pattern).
# =============================================================================

def run_publisher(args, repo_root):
    cmd = [sys.executable, str(PUBLISHER_SCRIPT), *args, "--repo-root", str(repo_root)]
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    try:
        parsed = json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        raise AssertionError(
            f"publisher stdout was not valid JSON (exit={proc.returncode}): {e}\n"
            f"--- stdout ---\n{proc.stdout}\n--- stderr ---\n{proc.stderr}"
        ) from e
    return proc.returncode, parsed, proc.stderr


def write_json(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f)


IDENTITY = {
    "model": "solar-open2-250b", "gpu": "GB10", "vllm": "0.22.0",
    "quant": "NVFP4", "topology": "multi", "tp": 2,
}


def make_repo(tmp_path):
    """Bootstraps a minimal fake repo root: just the docs/ directory skeleton the publisher
    writes under -- no .git required since every test passes --repo-root explicitly."""
    repo_root = tmp_path / "repo"
    for d in ("plan", "devlog", "testlog", "simlog", "benchmark", "report"):
        (repo_root / "docs" / d).mkdir(parents=True, exist_ok=True)
    return repo_root


def init_publication(repo_root, topic, task_class, generated_utc="2026-07-25T01:59:09Z",
                      conditions=None, benchmark_mode=None, benchmark_verdict=None,
                      identity=None, report_slug=None):
    identity_path = repo_root / "_scratch_identity.json"
    write_json(identity_path, identity if identity is not None else IDENTITY)
    args = [
        "init", "--topic", topic, "--task-class", task_class,
        "--generated-utc", generated_utc, "--identity-json", str(identity_path),
    ]
    if conditions is not None:
        cond_path = repo_root / "_scratch_conditions.json"
        write_json(cond_path, conditions)
        args += ["--conditions-json", str(cond_path)]
    if benchmark_mode is not None:
        args += ["--benchmark-mode", benchmark_mode]
    if benchmark_verdict is not None:
        args += ["--benchmark-verdict", benchmark_verdict]
    if report_slug is not None:
        args += ["--report-slug", report_slug]
    return run_publisher(args, repo_root)


# =============================================================================
# Section C -- `init` subcommand: scaffold + publication-record creation (first run)
# =============================================================================

class TestInitScaffoldModelServingStrategy(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()

    def test_exit_zero_and_ok_true(self):
        code, out, err = init_publication(self.repo_root, "구조개선_테스트", "model_serving_strategy")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertTrue(out.get("ok"), msg=out)

    def test_required_evidence_matches_matrix(self):
        code, out, err = init_publication(self.repo_root, "구조개선_테스트", "model_serving_strategy")
        self.assertEqual(sorted(out["required_evidence"]), sorted(["plan", "devlog", "simlog", "testlog"]))

    def test_scaffold_files_created_on_disk(self):
        code, out, err = init_publication(self.repo_root, "구조개선_테스트", "model_serving_strategy")
        for kind in ("plan", "devlog", "testlog"):
            rel = out["scaffolded"][kind]
            self.assertIsNotNone(rel, msg=f"{kind} missing from scaffolded: {out}")
            self.assertTrue((self.repo_root / rel).is_file(), msg=f"{kind} -> {rel} not created")
        simlog_rel = out["scaffolded"]["simlog"]
        self.assertTrue((self.repo_root / simlog_rel).is_dir(), msg=f"simlog -> {simlog_rel} not a directory")

    def test_scaffold_basenames_follow_doc_naming_convention(self):
        code, out, err = init_publication(self.repo_root, "구조개선_테스트", "model_serving_strategy",
                                           generated_utc="2026-07-25T01:59:09Z")
        import doc_naming
        expected_plan = doc_naming.dated_doc_basename("plan", "2026-07-25T01:59:09Z", "구조개선_테스트")
        self.assertEqual(Path(out["scaffolded"]["plan"]).name, expected_plan)

    def test_bench_report_and_certificate_not_required_hence_not_scaffolded(self):
        code, out, err = init_publication(self.repo_root, "구조개선_테스트", "model_serving_strategy")
        self.assertNotIn("bench_report", out["required_evidence"])
        self.assertIsNone(out["scaffolded"].get("bench_report"))
        self.assertIsNone(out["scaffolded"].get("certificate"))

    def test_record_path_reported_and_created(self):
        code, out, err = init_publication(self.repo_root, "구조개선_테스트", "model_serving_strategy")
        self.assertTrue((self.repo_root / out["record_path"]).is_file())

    def test_scaffold_file_contains_provenance_header(self):
        code, out, err = init_publication(self.repo_root, "구조개선_테스트", "model_serving_strategy")
        content = (self.repo_root / out["scaffolded"]["plan"]).read_text(encoding="utf-8")
        self.assertIn("model_serving_strategy", content)
        self.assertIn("구조개선_테스트", content)
        self.assertIn("PLACEHOLDER", content)


class TestInitScaffoldHarnessChange(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()

    def test_actual_trial_true_requires_and_scaffolds_simlog(self):
        code, out, err = init_publication(self.repo_root, "패치A", "harness_change",
                                           conditions={"actual_trial": True})
        self.assertIn("simlog", out["required_evidence"])
        self.assertTrue((self.repo_root / out["scaffolded"]["simlog"]).is_dir())

    def test_no_actual_trial_no_simlog(self):
        code, out, err = init_publication(self.repo_root, "패치B", "harness_change",
                                           conditions={"actual_trial": False})
        self.assertNotIn("simlog", out["required_evidence"])
        self.assertIsNone(out["scaffolded"].get("simlog"))


class TestInitScaffoldCapacityRejection(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()

    def test_plan_required_and_scaffolded(self):
        code, out, err = init_publication(self.repo_root, "397b용량거부", "capacity_rejection")
        self.assertEqual(out["required_evidence"], ["plan"])
        self.assertTrue((self.repo_root / out["scaffolded"]["plan"]).is_file())

    def test_or_group_convenience_scaffold_both_present(self):
        code, out, err = init_publication(self.repo_root, "397b용량거부", "capacity_rejection")
        self.assertEqual(sorted(out["or_group_scaffolded"]), ["devlog", "testlog"])
        self.assertTrue((self.repo_root / out["scaffolded"]["devlog"]).is_file())
        self.assertTrue((self.repo_root / out["scaffolded"]["testlog"]).is_file())

    def test_capacity_rejection_required_flag_true(self):
        code, out, err = init_publication(self.repo_root, "397b용량거부", "capacity_rejection")
        self.assertTrue(out["capacity_rejection_required"])


class TestInitScaffoldMinorPatch(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()

    def test_verification_and_commit_required_but_not_scaffolded(self):
        code, out, err = init_publication(self.repo_root, "사소한패치", "minor_patch")
        self.assertEqual(sorted(out["required_evidence"]), ["commit", "verification"])
        self.assertIsNone(out["scaffolded"].get("verification"))
        self.assertIsNone(out["scaffolded"].get("commit"))
        self.assertIn("verification", out["pending_evidence"])
        self.assertIn("commit", out["pending_evidence"])

    def test_no_plan_devlog_testlog_scaffolded(self):
        code, out, err = init_publication(self.repo_root, "사소한패치", "minor_patch")
        self.assertIsNone(out["scaffolded"].get("plan"))
        self.assertIsNone(out["scaffolded"].get("devlog"))
        self.assertIsNone(out["scaffolded"].get("testlog"))


class TestInitScaffoldReadOnlyAudit(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()

    def test_report_requested_true_requires_slug_and_scaffolds(self):
        code, out, err = init_publication(self.repo_root, "감사", "read_only_audit",
                                           conditions={"report_requested": True},
                                           report_slug="q3-read-only-audit")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertEqual(out["required_evidence"], ["report"])
        rel = out["scaffolded"]["report"]
        self.assertTrue((self.repo_root / rel).is_file())
        self.assertEqual(Path(rel).name, "q3-read-only-audit.html")

    def test_report_requested_true_without_slug_fails_closed(self):
        code, out, err = init_publication(self.repo_root, "감사2", "read_only_audit",
                                           conditions={"report_requested": True})
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("INIT_REPORT_SLUG_REQUIRED", out.get("reason_codes", []))

    def test_report_requested_false_empty_required(self):
        code, out, err = init_publication(self.repo_root, "감사3", "read_only_audit",
                                           conditions={"report_requested": False})
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertEqual(out["required_evidence"], [])


class TestInitScaffoldFullBenchmark(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()

    def test_pass_verdict_requires_certificate_but_not_scaffolded_at_init(self):
        code, out, err = init_publication(self.repo_root, "solar벤치", "full_benchmark",
                                           benchmark_mode="full", benchmark_verdict="PASS")
        self.assertIn("certificate", out["required_evidence"])
        self.assertIsNone(out["scaffolded"].get("certificate"))
        self.assertIn("certificate", out["pending_evidence"])
        self.assertIn("bench_report", out["pending_evidence"])

    def test_fail_verdict_does_not_require_certificate(self):
        code, out, err = init_publication(self.repo_root, "solar벤치2", "full_benchmark",
                                           benchmark_mode="full", benchmark_verdict="FAIL")
        self.assertNotIn("certificate", out["required_evidence"])


# =============================================================================
# Section D -- `init` idempotency (rerun preserves narrative/raw, no new files, stable JSON)
# =============================================================================

class TestInitIdempotency(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()

    def test_rerun_reuses_same_filenames(self):
        code1, out1, _ = init_publication(self.repo_root, "재실행테스트", "model_serving_strategy")
        code2, out2, _ = init_publication(self.repo_root, "재실행테스트", "model_serving_strategy")
        self.assertEqual(code1, 0)
        self.assertEqual(code2, 0)
        self.assertEqual(out1["scaffolded"], out2["scaffolded"])

    def test_rerun_creates_no_new_files_in_docs_plan(self):
        init_publication(self.repo_root, "재실행테스트2", "model_serving_strategy")
        before = sorted(p.name for p in (self.repo_root / "docs" / "plan").iterdir())
        init_publication(self.repo_root, "재실행테스트2", "model_serving_strategy")
        after = sorted(p.name for p in (self.repo_root / "docs" / "plan").iterdir())
        self.assertEqual(before, after)

    def test_rerun_does_not_touch_hand_authored_narrative(self):
        code, out, _ = init_publication(self.repo_root, "재실행테스트3", "model_serving_strategy")
        plan_path = self.repo_root / out["scaffolded"]["plan"]
        plan_path.write_text("# hand-authored narrative -- must survive rerun\n", encoding="utf-8")
        before_mtime = plan_path.stat().st_mtime_ns
        before_content = plan_path.read_text(encoding="utf-8")
        init_publication(self.repo_root, "재실행테스트3", "model_serving_strategy")
        self.assertEqual(plan_path.read_text(encoding="utf-8"), before_content)
        self.assertEqual(plan_path.stat().st_mtime_ns, before_mtime)

    def test_rerun_reports_already_existed(self):
        init_publication(self.repo_root, "재실행테스트4", "model_serving_strategy")
        code, out, _ = init_publication(self.repo_root, "재실행테스트4", "model_serving_strategy")
        self.assertEqual(code, 0)
        self.assertEqual(sorted(out["already_existed"]), sorted(["plan", "devlog", "simlog", "testlog"]))
        self.assertEqual(out["created_now"], [])

    def test_first_run_reports_created_now(self):
        code, out, _ = init_publication(self.repo_root, "최초실행", "model_serving_strategy")
        self.assertEqual(sorted(out["created_now"]), sorted(["plan", "devlog", "simlog", "testlog"]))
        self.assertEqual(out["already_existed"], [])

    def test_rerun_with_missing_scaffold_file_restores_only_that_file(self):
        code, out, _ = init_publication(self.repo_root, "복구테스트", "model_serving_strategy")
        devlog_path = self.repo_root / out["scaffolded"]["devlog"]
        plan_path = self.repo_root / out["scaffolded"]["plan"]
        plan_path.write_text("# survives\n", encoding="utf-8")
        os.remove(devlog_path)
        code2, out2, _ = init_publication(self.repo_root, "복구테스트", "model_serving_strategy")
        self.assertEqual(code2, 0)
        self.assertTrue(devlog_path.is_file(), msg="deleted scaffold must be restored")
        self.assertEqual(plan_path.read_text(encoding="utf-8"), "# survives\n")
        self.assertEqual(out2["created_now"], ["devlog"])

    def test_flagless_rerun_preserves_prior_benchmark_fail_verdict(self):
        code, out, err = init_publication(
            self.repo_root, "benchmark보존", "full_benchmark", benchmark_mode="full", benchmark_verdict="FAIL")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        self.assertEqual(json.loads(record_path.read_text())["benchmark"],
                         {"mode": "full", "verdict": "FAIL"})
        code2, out2, err2 = init_publication(self.repo_root, "benchmark보존", "full_benchmark")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertEqual(json.loads(record_path.read_text())["benchmark"],
                         {"mode": "full", "verdict": "FAIL"})

    def test_flagless_rerun_preserves_pass_certificate_requirement(self):
        code, out, err = init_publication(
            self.repo_root, "pass_certificate", "full_benchmark",
            benchmark_mode="full", benchmark_verdict="PASS")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        code2, out2, err2 = init_publication(self.repo_root, "pass_certificate", "full_benchmark")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        record = json.loads(record_path.read_text())
        self.assertEqual(record["benchmark"], {"mode": "full", "verdict": "PASS"})
        self.assertIn("certificate", record["required_evidence"])
        self.assertIn("certificate", out2["pending_evidence"])

    def test_malformed_identity_and_conditions_fail_before_scaffolds_or_record(self):
        before = sorted(str(p.relative_to(self.repo_root)) for p in self.repo_root.rglob("*") if p.is_file())
        code, out, _ = init_publication(self.repo_root, "bad_identity", "harness_change", identity={})
        self.assertEqual(code, 2)
        self.assertIn("INIT_INPUT_SCHEMA_INVALID", out["reason_codes"])
        code2, out2, _ = init_publication(
            self.repo_root, "bad_conditions", "harness_change",
            conditions={"actual_trial": "yes", "unknown": True})
        self.assertEqual(code2, 2)
        self.assertIn("INIT_INPUT_SCHEMA_INVALID", out2["reason_codes"])
        after = sorted(str(p.relative_to(self.repo_root)) for p in self.repo_root.rglob("*") if p.is_file()
                       and not p.name.startswith("_scratch_"))
        before = [p for p in before if not Path(p).name.startswith("_scratch_")]
        self.assertEqual(after, before)

    def test_persisted_publication_record_unknown_top_level_field_is_rejected(self):
        code, out, err = init_publication(self.repo_root, "unknown_record_field", "harness_change")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        record = json.loads(record_path.read_text())
        record["dangerous_unknown"] = {"path": "../../outside"}
        record_path.write_text(json.dumps(record), encoding="utf-8")
        code2, out2, err2 = init_publication(self.repo_root, "unknown_record_field", "harness_change")
        self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
        self.assertIn("PUBLICATION_RECORD_UNKNOWN_FIELDS", out2["reason_codes"])

    def test_minimal_persisted_record_is_rejected_before_append_raw_mutation(self):
        code, out, err = init_publication(self.repo_root, "malformed", "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        malformed = {"task_class": "minor_patch", "identity": {}}
        record_path.write_text(json.dumps(malformed), encoding="utf-8")
        before = record_path.read_bytes()
        (self.repo_root / "verify.txt").write_text("verification\n", encoding="utf-8")
        code2, out2, err2 = append_raw(
            self.repo_root, "malformed", "verification", "verify.txt")
        self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
        self.assertIn("PUBLICATION_RECORD_MISSING_FIELDS", out2["reason_codes"])
        self.assertEqual(record_path.read_bytes(), before)
        self.assertFalse((self.repo_root / "docs" / "_evidence" /
                          "malformed.verification.raw.jsonl").exists())

    def test_unknown_nested_scaffolded_kind_is_rejected_before_append_raw_mutation(self):
        code, out, err = init_publication(self.repo_root, "nested", "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["scaffolded"]["unexpected_kind"] = "attacker.txt"
        record_path.write_text(json.dumps(record), encoding="utf-8")
        before = record_path.read_bytes()
        (self.repo_root / "test.txt").write_text("raw\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, "nested", "testlog", "test.txt")
        self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
        self.assertIn("PUBLICATION_RECORD_SCAFFOLDED_KEYS_INVALID", out2["reason_codes"])
        self.assertEqual(record_path.read_bytes(), before)
        self.assertFalse((self.repo_root / "docs" / "_evidence" / "nested.testlog.raw.jsonl").exists())

    def test_top_level_null_record_is_rejected_without_init_overwrite(self):
        evidence_dir = self.repo_root / "docs" / "_evidence"
        evidence_dir.mkdir(parents=True, exist_ok=True)
        record_path = evidence_dir / "nullrecord.json"
        record_path.write_text("null\n", encoding="utf-8")
        before = record_path.read_bytes()
        code, out, err = init_publication(self.repo_root, "nullrecord", "minor_patch")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("PUBLICATION_RECORD_NOT_AN_OBJECT", out["reason_codes"])
        self.assertEqual(record_path.read_bytes(), before)

    def test_publication_id_must_match_selected_topic_before_append_mutation(self):
        code, out, err = init_publication(self.repo_root, "topic_b", "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["publication_id"] = "topic_a"
        record_path.write_text(json.dumps(record), encoding="utf-8")
        before = record_path.read_bytes()
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, "topic_b", "verification", "verify.txt")
        self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
        self.assertIn("PUBLICATION_RECORD_TOPIC_MISMATCH", out2["reason_codes"])
        self.assertEqual(record_path.read_bytes(), before)
        self.assertFalse((self.repo_root / "docs" / "_evidence" /
                          "topic_b.verification.raw.jsonl").exists())

    def test_hostile_raw_log_path_is_rejected_before_record_or_sidecar_mutation(self):
        topic = "raw_path_binding"
        code, out, err = init_publication(self.repo_root, topic, "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["raw_log_paths"]["plan"] = "../../outside.jsonl"
        record_path.write_text(json.dumps(record), encoding="utf-8")
        before = record_path.read_bytes()
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, topic, "verification", "verify.txt")
        self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
        self.assertIn("PUBLICATION_RECORD_RAW_LOG_PATH_MISMATCH:plan", out2["reason_codes"])
        self.assertEqual(record_path.read_bytes(), before)
        self.assertFalse((self.repo_root / "docs" / "_evidence" /
                          f"{topic}.verification.raw.jsonl").exists())

    def test_derived_record_fields_must_match_task_contract_before_rerun_mutation(self):
        cases = [
            ("required", "required_evidence", [], "PUBLICATION_RECORD_REQUIRED_EVIDENCE_MISMATCH"),
            ("orgroup", "or_group_scaffolded", ["devlog"], "PUBLICATION_RECORD_OR_GROUP_MISMATCH"),
            ("capacity", "capacity_rejection_required", True,
             "PUBLICATION_RECORD_CAPACITY_FLAG_MISMATCH"),
        ]
        for suffix, field, hostile_value, expected_code in cases:
            with self.subTest(field=field):
                topic = f"semantic_{suffix}"
                code, out, err = init_publication(
                    self.repo_root, topic, "full_benchmark",
                    benchmark_mode="full", benchmark_verdict="PASS")
                self.assertEqual(code, 0, msg=f"out={out} err={err}")
                record_path = self.repo_root / out["record_path"]
                record = json.loads(record_path.read_text(encoding="utf-8"))
                record[field] = hostile_value
                record_path.write_text(json.dumps(record), encoding="utf-8")
                before = record_path.read_bytes()
                code2, out2, err2 = init_publication(
                    self.repo_root, topic, "full_benchmark",
                    benchmark_mode="full", benchmark_verdict="PASS")
                self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
                self.assertIn(expected_code, out2["reason_codes"])
                self.assertEqual(record_path.read_bytes(), before)

    def test_scaffolded_null_and_missing_key_are_rejected_without_init_overwrite(self):
        for mutation in ("null", "missing"):
            topic = f"scaffold_{mutation}"
            code, out, err = init_publication(self.repo_root, topic, "minor_patch")
            self.assertEqual(code, 0, msg=f"out={out} err={err}")
            record_path = self.repo_root / out["record_path"]
            record = json.loads(record_path.read_text(encoding="utf-8"))
            if mutation == "null":
                record["scaffolded"] = None
            else:
                record["scaffolded"].pop("plan")
            record_path.write_text(json.dumps(record), encoding="utf-8")
            before = record_path.read_bytes()
            code2, out2, err2 = init_publication(self.repo_root, topic, "minor_patch")
            self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
            expected = ("PUBLICATION_RECORD_FIELD_NOT_AN_OBJECT:scaffolded" if mutation == "null"
                        else "PUBLICATION_RECORD_SCAFFOLDED_KEYS_INVALID")
            self.assertIn(expected, out2["reason_codes"])
            self.assertEqual(record_path.read_bytes(), before)

    def test_rerun_rejects_immutable_identity_time_and_benchmark_rebinding(self):
        code, out, err = init_publication(
            self.repo_root, "immutable_binding", "full_benchmark",
            generated_utc="2026-07-25T01:00:00Z", benchmark_mode="full",
            benchmark_verdict="FAIL", identity=IDENTITY)
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record_path = self.repo_root / out["record_path"]
        before = record_path.read_bytes()
        rebound_identity = dict(IDENTITY)
        rebound_identity["model"] = "different-model"
        code2, out2, _ = init_publication(
            self.repo_root, "immutable_binding", "full_benchmark",
            generated_utc="2026-07-25T02:00:00Z", benchmark_mode="full",
            benchmark_verdict="PASS", identity=rebound_identity)
        self.assertEqual(code2, 2)
        self.assertIn("INIT_IMMUTABLE_REBIND", out2["reason_codes"])
        self.assertEqual(record_path.read_bytes(), before)


# =============================================================================
# Section E/F -- `append-raw`: happy path (simlog copy + non-dir JSONL sidecar), atomicity, and
# rejections (absolute/escape/symlink/wrong-type/invalid-utf8/malformed) -- one stable JSON, no
# traceback, exit 2.
# =============================================================================

def append_raw(repo_root, topic, kind, src, recorded_utc="2026-07-25T02:00:00Z", dest_name=None):
    args = ["append-raw", "--topic", topic, "--kind", kind, "--src", src, "--recorded-utc", recorded_utc]
    if dest_name is not None:
        args += ["--dest-name", dest_name]
    return run_publisher(args, repo_root)


class _PublisherRepoTestCase(unittest.TestCase):
    """Shared setUp/tearDown for tests needing an isolated fake repo root."""

    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.repo_root = make_repo(Path(self._td.name))

    def tearDown(self):
        self._td.cleanup()


class TestAppendRawSimlogHappyPath(_PublisherRepoTestCase):
    def test_copies_file_into_simlog_run_dir(self):
        code, out, _ = init_publication(self.repo_root, "append원시테스트", "model_serving_strategy")
        simlog_rel = out["scaffolded"]["simlog"]
        src = self.repo_root / "trial01_vllm.log"
        src.write_text("hello raw evidence\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, "append원시테스트", "simlog", "trial01_vllm.log")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertTrue(out2["ok"])
        dest = self.repo_root / simlog_rel / "trial01_vllm.log"
        self.assertTrue(dest.is_file())
        self.assertEqual(dest.read_text(encoding="utf-8"), "hello raw evidence\n")

    def test_dest_name_override(self):
        init_publication(self.repo_root, "append원시테스트2", "model_serving_strategy")
        src = self.repo_root / "raw_source.log"
        src.write_text("content\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "append원시테스트2", "simlog", "raw_source.log",
                                     dest_name="trial01_renamed.log")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        simlog_rel = out["recorded_path"]
        self.assertTrue(self.repo_root.joinpath(simlog_rel).is_file())
        self.assertEqual(Path(simlog_rel).name, "trial01_renamed.log")

    def test_second_distinct_file_does_not_clobber_first(self):
        _, init_out, _ = init_publication(self.repo_root, "append원시테스트3", "model_serving_strategy")
        for name, content in (("a.log", "AAA\n"), ("b.log", "BBB\n")):
            (self.repo_root / name).write_text(content, encoding="utf-8")
            code, out, err = append_raw(self.repo_root, "append원시테스트3", "simlog", name)
            self.assertEqual(code, 0, msg=f"out={out} err={err}")
        simlog_dir = self.repo_root / init_out["scaffolded"]["simlog"]
        self.assertEqual((simlog_dir / "a.log").read_text(encoding="utf-8"), "AAA\n")
        self.assertEqual((simlog_dir / "b.log").read_text(encoding="utf-8"), "BBB\n")

    def test_dest_collision_rejected(self):
        init_publication(self.repo_root, "append원시테스트4", "model_serving_strategy")
        (self.repo_root / "dup.log").write_text("first\n", encoding="utf-8")
        append_raw(self.repo_root, "append원시테스트4", "simlog", "dup.log")
        (self.repo_root / "dup2.log").write_text("second\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "append원시테스트4", "simlog", "dup2.log", dest_name="dup.log")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_DEST_COLLISION", out["reason_codes"])
        self.assertNotIn("Traceback", err)


class TestAppendRawNonDirSidecarHappyPath(_PublisherRepoTestCase):
    def test_appends_jsonl_line_for_testlog_kind(self):
        init_publication(self.repo_root, "append사이드카", "model_serving_strategy")
        (self.repo_root / "measurement.txt").write_text("measured 42 tok/s\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "append사이드카", "testlog", "measurement.txt")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        rel = out["recorded_path"]
        lines = (self.repo_root / rel).read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 1)
        entry = json.loads(lines[0])
        self.assertEqual(entry["src"], "measurement.txt")
        self.assertEqual(entry["seq"], 1)
        self.assertIn("sha256", entry)

    def test_second_append_is_append_only_not_overwrite(self):
        init_publication(self.repo_root, "append사이드카2", "model_serving_strategy")
        (self.repo_root / "m1.txt").write_text("one\n", encoding="utf-8")
        (self.repo_root / "m2.txt").write_text("two\n", encoding="utf-8")
        append_raw(self.repo_root, "append사이드카2", "testlog", "m1.txt")
        code, out, err = append_raw(self.repo_root, "append사이드카2", "testlog", "m2.txt")
        rel = out["recorded_path"]
        lines = (self.repo_root / rel).read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 2)
        self.assertEqual(json.loads(lines[0])["src"], "m1.txt")
        self.assertEqual(json.loads(lines[1])["src"], "m2.txt")
        self.assertEqual(json.loads(lines[1])["seq"], 2)

    def test_verification_kind_sets_evidence_pointer(self):
        init_publication(self.repo_root, "검증패치", "minor_patch")
        (self.repo_root / "verify_out.txt").write_text("py_compile OK\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "검증패치", "verification", "verify_out.txt")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record = json.loads((self.repo_root / "docs" / "_evidence" / "검증패치.json").read_text(encoding="utf-8"))
        self.assertEqual(record["scaffolded"]["verification"], "verify_out.txt")

    def test_commit_kind_sets_evidence_pointer(self):
        init_publication(self.repo_root, "커밋패치", "minor_patch")
        (self.repo_root / "commit_sha.txt").write_text("abc123def\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "커밋패치", "commit", "commit_sha.txt")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        record = json.loads((self.repo_root / "docs" / "_evidence" / "커밋패치.json").read_text(encoding="utf-8"))
        self.assertEqual(record["scaffolded"]["commit"], "commit_sha.txt")

    def test_rejects_bench_report_kind_directs_to_publish_benchmark(self):
        init_publication(self.repo_root, "append벤치", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="FAIL")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "append벤치", "bench_report", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("APPEND_RAW_UNSUPPORTED_KIND", out["reason_codes"])

    def test_rejects_certificate_kind_directs_to_publish_benchmark(self):
        init_publication(self.repo_root, "append인증서", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "append인증서", "certificate", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("APPEND_RAW_UNSUPPORTED_KIND", out["reason_codes"])


class TestAppendRawRequiresInit(_PublisherRepoTestCase):
    def test_uninitialized_topic_rejected(self):
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "미초기화토픽", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("PUBLICATION_NOT_INITIALIZED", out["reason_codes"])
        self.assertNotIn("Traceback", err)


class TestAppendRawRejections(_PublisherRepoTestCase):
    def setUp(self):
        super().setUp()
        init_publication(self.repo_root, "거부테스트", "model_serving_strategy")

    def test_rejects_absolute_src(self):
        code, out, err = append_raw(self.repo_root, "거부테스트", "testlog", "/etc/passwd")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_ABSOLUTE_SRC", out["reason_codes"])
        self.assertNotIn("Traceback", err)

    def test_rejects_repo_escape(self):
        code, out, err = append_raw(self.repo_root, "거부테스트", "testlog", "../../../../../../etc/passwd")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_SRC_ESCAPES_REPO", out["reason_codes"])
        self.assertNotIn("Traceback", err)

    def test_rejects_symlink_in_path(self):
        real_target = self.repo_root.parent / "outside_target.txt"
        real_target.write_text("outside content\n", encoding="utf-8")
        link_path = self.repo_root / "sneaky_link.txt"
        os.symlink(real_target, link_path)
        code, out, err = append_raw(self.repo_root, "거부테스트", "testlog", "sneaky_link.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_SRC_SYMLINK", out["reason_codes"])
        self.assertNotIn("Traceback", err)

    def test_rejects_directory_as_src(self):
        (self.repo_root / "a_directory").mkdir()
        code, out, err = append_raw(self.repo_root, "거부테스트", "testlog", "a_directory")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_SRC_WRONG_TYPE", out["reason_codes"])
        self.assertNotIn("Traceback", err)

    def test_rejects_missing_src(self):
        code, out, err = append_raw(self.repo_root, "거부테스트", "testlog", "does_not_exist.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_SRC_NOT_FOUND", out["reason_codes"])
        self.assertNotIn("Traceback", err)

    def test_rejects_empty_file(self):
        (self.repo_root / "empty.txt").write_text("", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "거부테스트", "testlog", "empty.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_SRC_WRONG_TYPE", out["reason_codes"])
        self.assertNotIn("Traceback", err)

    def test_rejects_invalid_utf8(self):
        bad = self.repo_root / "bad_utf8.bin"
        with open(bad, "wb") as f:
            f.write(b"\xff\xfe\x00\x81not valid utf8")
        code, out, err = append_raw(self.repo_root, "거부테스트", "testlog", "bad_utf8.bin")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("RAW_EVIDENCE_SRC_INVALID_UTF8", out["reason_codes"])
        self.assertNotIn("Traceback", err)

    def test_malformed_cli_missing_required_arg(self):
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "append-raw", "--topic", "거부테스트",
               "--kind", "testlog", "--repo-root", str(self.repo_root)]  # missing --src
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertEqual(proc.returncode, 2)
        out = json.loads(proc.stdout)
        self.assertIn("CLI_USAGE_ERROR", out["reason_codes"])
        self.assertNotIn("Traceback", proc.stderr)


# =============================================================================
# Section G -- `set-narrative`: explicit external input only, provenance, idempotent
# re-authoring, never fabricates.
# =============================================================================

def set_narrative(repo_root, topic, kind, narrative_file, author="Sonnet",
                   generated_utc="2026-07-25T02:05:00Z"):
    args = ["set-narrative", "--topic", topic, "--kind", kind, "--narrative-file", narrative_file,
            "--author", author, "--generated-utc", generated_utc]
    return run_publisher(args, repo_root)


class TestSetNarrative(_PublisherRepoTestCase):
    def test_injects_supplied_narrative_with_provenance(self):
        code, out, _ = init_publication(self.repo_root, "서술테스트", "model_serving_strategy")
        plan_rel = out["scaffolded"]["plan"]
        narrative_src = self.repo_root / "sonnet_narrative.md"
        narrative_src.write_text("Root cause was X; fixed by Y.\n", encoding="utf-8")
        code2, out2, err2 = set_narrative(self.repo_root, "서술테스트", "plan", "sonnet_narrative.md")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        content = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        self.assertIn("Root cause was X; fixed by Y.", content)
        self.assertIn("author=Sonnet", content)
        self.assertNotIn("PLACEHOLDER", content)

    def test_provenance_header_survives_narrative_replace(self):
        code, out, _ = init_publication(self.repo_root, "서술테스트2", "model_serving_strategy")
        plan_rel = out["scaffolded"]["plan"]
        (self.repo_root / "n1.md").write_text("narrative content\n", encoding="utf-8")
        set_narrative(self.repo_root, "서술테스트2", "plan", "n1.md")
        content = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        self.assertIn("task_class: model_serving_strategy", content)
        self.assertIn("publication_id: 서술테스트2", content)

    def test_re_authoring_replaces_prior_narrative_only(self):
        code, out, _ = init_publication(self.repo_root, "서술재작성", "model_serving_strategy")
        plan_rel = out["scaffolded"]["plan"]
        (self.repo_root / "n1.md").write_text("first draft\n", encoding="utf-8")
        set_narrative(self.repo_root, "서술재작성", "plan", "n1.md")
        (self.repo_root / "n2.md").write_text("revised final draft\n", encoding="utf-8")
        set_narrative(self.repo_root, "서술재작성", "plan", "n2.md")
        content = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        self.assertIn("revised final draft", content)
        self.assertNotIn("first draft", content)
        self.assertIn("publication_id: 서술재작성", content)  # provenance header untouched

    def test_missing_narrative_file_argument_is_cli_usage_error(self):
        init_publication(self.repo_root, "서술누락", "model_serving_strategy")
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "set-narrative", "--topic", "서술누락",
               "--kind", "plan", "--author", "Sonnet", "--generated-utc", "2026-07-25T02:05:00Z",
               "--repo-root", str(self.repo_root)]  # missing --narrative-file
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertEqual(proc.returncode, 2)
        self.assertNotIn("Traceback", proc.stderr)

    def test_never_fabricates_when_narrative_file_argument_absent(self):
        code, out, _ = init_publication(self.repo_root, "서술위조방지", "model_serving_strategy")
        plan_rel = out["scaffolded"]["plan"]
        before = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "set-narrative", "--topic", "서술위조방지",
               "--kind", "plan", "--author", "Sonnet", "--generated-utc", "2026-07-25T02:05:00Z",
               "--repo-root", str(self.repo_root)]
        subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        after = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        self.assertEqual(before, after, msg="placeholder must be untouched when no narrative was supplied")
        self.assertIn("PLACEHOLDER", after)

    def test_rejects_narrative_file_absolute_path(self):
        init_publication(self.repo_root, "서술절대경로", "model_serving_strategy")
        code, out, err = set_narrative(self.repo_root, "서술절대경로", "plan", "/etc/passwd")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("NARRATIVE_ABSOLUTE_SRC", out["reason_codes"])

    def test_rejects_unscaffolded_kind(self):
        init_publication(self.repo_root, "서술미스캐폴드", "minor_patch")  # no plan scaffolded
        (self.repo_root / "n.md").write_text("x\n", encoding="utf-8")
        code, out, err = set_narrative(self.repo_root, "서술미스캐폴드", "plan", "n.md")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("NARRATIVE_TARGET_NOT_SCAFFOLDED", out["reason_codes"])


# =============================================================================
# Section E3 -- capacity_rejection raw gate evidence recording
# =============================================================================

class TestRecordCapacityRejection(_PublisherRepoTestCase):
    def test_records_gate_evidence_pointer(self):
        init_publication(self.repo_root, "용량거부기록", "capacity_rejection")
        (self.repo_root / "gate_exit7.txt").write_text("RAM gate: exit 7\n", encoding="utf-8")
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "record-capacity-rejection",
               "--topic", "용량거부기록", "--gate-evidence-src", "gate_exit7.txt",
               "--reason", "checkpoint total_size / tp + floor > MemAvailable",
               "--recorded-utc", "2026-07-25T02:10:00Z", "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        out = json.loads(proc.stdout)
        self.assertEqual(proc.returncode, 0, msg=f"out={out} err={proc.stderr}")
        record = json.loads((self.repo_root / "docs" / "_evidence" / "용량거부기록.json").read_text(encoding="utf-8"))
        self.assertEqual(record["capacity_rejection"]["gate_evidence"], "용량거부기록_capacity_gate.txt")
        self.assertTrue((self.repo_root / "docs" / "_evidence" /
                         record["capacity_rejection"]["gate_evidence"]).is_file())
        self.assertTrue(record["capacity_rejection"]["rejected"])

    def test_recorded_capacity_rejection_can_finalize(self):
        init_publication(self.repo_root, "용량거부완료", "capacity_rejection")
        (self.repo_root / "plan.md").write_text("# capacity rejection plan\n", encoding="utf-8")
        code, out, err = set_narrative(self.repo_root, "용량거부완료", "plan", "plan.md")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        (self.repo_root / "devlog.md").write_text("# deterministic capacity result\n", encoding="utf-8")
        code_d, out_d, err_d = set_narrative(
            self.repo_root, "용량거부완료", "devlog", "devlog.md")
        self.assertEqual(code_d, 0, msg=f"out={out_d} err={err_d}")
        (self.repo_root / "gate.txt").write_text("RAM gate: exit 7\n", encoding="utf-8")
        args = ["record-capacity-rejection", "--topic", "용량거부완료",
                "--gate-evidence-src", "gate.txt", "--reason", "capacity floor exceeded",
                "--recorded-utc", "2026-07-25T02:10:00Z"]
        code2, out2, err2 = run_publisher(args, self.repo_root)
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        code3, out3, err3 = finalize(
            self.repo_root, "용량거부완료",
            pii_scan={"passed": True,
                      "scanned_paths": manifest_relative_evidence_paths(self.repo_root, "용량거부완료")
                      + ["용량거부완료_capacity_gate.txt"]})
        self.assertEqual(code3, 1, msg=f"out={out3} err={err3}")
        self.assertEqual(out3["state"], "evidence-complete")
        self.assertIn("TASK_CLASS_CAPS_BELOW_PROMOTION:capacity_rejection", out3["reason_codes"])

    def test_wrong_task_class_rejected(self):
        init_publication(self.repo_root, "잘못된태스크", "model_serving_strategy")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "record-capacity-rejection",
               "--topic", "잘못된태스크", "--gate-evidence-src", "x.txt", "--reason", "n/a",
               "--recorded-utc", "2026-07-25T02:10:00Z", "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        out = json.loads(proc.stdout)
        self.assertEqual(proc.returncode, 2, msg=f"out={out}")
        self.assertIn("CAPACITY_REJECTION_WRONG_TASK_CLASS", out["reason_codes"])


# =============================================================================
# Section H -- `publish-benchmark`: FAIL (bench_report scaffold, certificate omitted/rejected) /
# PASS (certificate ONLY from a supplied, parseable, verified artifact -- never synthesized).
# =============================================================================

def publish_benchmark(repo_root, topic, verdict, bench_report_src, certificate_src=None,
                       generated_utc="2026-07-25T02:20:00Z"):
    args = ["publish-benchmark", "--topic", topic, "--verdict", verdict,
            "--generated-utc", generated_utc, "--bench-report-src", bench_report_src]
    if certificate_src is not None:
        args += ["--certificate-src", certificate_src]
    return run_publisher(args, repo_root)


VALID_FLAT_CERTIFICATE = """\
# flat-scalar certificate (publish_benchmark_record.py contract)
model: solar-open2-250b
gpu_model: GB10
vllm_version: 0.22.0
quantization: NVFP4
topology: multi
tensor_parallel_size: 2
verdict: PASS
benchmark_mode: full
"""


class TestPublishBenchmarkFail(_PublisherRepoTestCase):
    def test_bench_report_published_certificate_omitted(self):
        init_publication(self.repo_root, "벤치FAIL", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="FAIL")
        (self.repo_root / "report_src.md").write_text("# why it was slow\n", encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "벤치FAIL", "FAIL", "report_src.md")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertTrue((self.repo_root / out["bench_report_path"]).is_file())
        self.assertIsNone(out["certificate_path"])
        self.assertTrue(Path(out["bench_report_path"]).name.startswith("bench_report_"))

    def test_certificate_src_on_fail_is_rejected(self):
        init_publication(self.repo_root, "벤치FAIL2", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="FAIL")
        (self.repo_root / "report_src.md").write_text("# report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "벤치FAIL2", "FAIL", "report_src.md",
                                            certificate_src="cert.yaml")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("PUBLISH_BENCHMARK_CERTIFICATE_WITHOUT_PASS", out["reason_codes"])
        self.assertNotIn("Traceback", err)


class TestPublishBenchmarkPass(_PublisherRepoTestCase):
    def test_verdict_transition_recomputes_required_evidence_atomically(self):
        topic = "벤치전이"
        code0, out0, err0 = init_publication(
            self.repo_root, topic, "full_benchmark", benchmark_mode="full")
        self.assertEqual(code0, 0, msg=f"out={out0} err={err0}")
        self.assertNotIn("certificate", out0["required_evidence"])
        (self.repo_root / "report_src.md").write_text("# report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code1, out1, err1 = publish_benchmark(
            self.repo_root, topic, "PASS", "report_src.md", certificate_src="cert.yaml")
        self.assertEqual(code1, 0, msg=f"out={out1} err={err1}")
        record_path = self.repo_root / "docs" / "_evidence" / f"{topic}.json"
        record = json.loads(record_path.read_text(encoding="utf-8"))
        self.assertIn("certificate", record["required_evidence"])
        code2, out2, err2 = publish_benchmark(self.repo_root, topic, "FAIL", "report_src.md")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        record = json.loads(record_path.read_text(encoding="utf-8"))
        self.assertNotIn("certificate", record["required_evidence"])
        self.assertIsNone(record["scaffolded"]["certificate"])

    def test_pass_without_certificate_src_does_not_synthesize_one(self):
        init_publication(self.repo_root, "벤치PASS1", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "report_src.md").write_text("# it was fast enough\n", encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "벤치PASS1", "PASS", "report_src.md")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIsNone(out["certificate_path"])
        self.assertTrue(out["pending_certificate"])

    def test_pass_with_real_certificate_publishes_it(self):
        init_publication(self.repo_root, "벤치PASS2", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "report_src.md").write_text("# it was fast enough\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "벤치PASS2", "PASS", "report_src.md",
                                            certificate_src="cert.yaml")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIsNotNone(out["certificate_path"])
        cert_path = self.repo_root / out["certificate_path"]
        self.assertTrue(cert_path.is_file())
        self.assertEqual(cert_path.read_text(encoding="utf-8"), VALID_FLAT_CERTIFICATE)
        self.assertTrue(Path(out["certificate_path"]).name.startswith("benchmark_"))

    def test_pass_with_unparseable_certificate_rejected_not_published(self):
        init_publication(self.repo_root, "벤치PASS3", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "report_src.md").write_text("# report\n", encoding="utf-8")
        (self.repo_root / "garbage_cert.yaml").write_text("this: is:\n  not: a flat cert\nlist:\n- 1\n- 2\n",
                                                            encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "벤치PASS3", "PASS", "report_src.md",
                                            certificate_src="garbage_cert.yaml")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("PUBLISH_BENCHMARK_CERTIFICATE_UNPARSEABLE", out["reason_codes"])
        self.assertFalse(list((self.repo_root / "docs" / "benchmark").glob("benchmark_*")),
                          msg="unparseable certificate must never be copied into docs/benchmark/")

    def test_republish_same_topic_overwrites_same_filename(self):
        init_publication(self.repo_root, "벤치재발행", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "r1.md").write_text("draft 1\n", encoding="utf-8")
        code1, out1, _ = publish_benchmark(self.repo_root, "벤치재발행", "PASS", "r1.md")
        (self.repo_root / "r2.md").write_text("draft 2 final\n", encoding="utf-8")
        code2, out2, _ = publish_benchmark(self.repo_root, "벤치재발행", "PASS", "r2.md")
        self.assertEqual(out1["bench_report_path"], out2["bench_report_path"])
        self.assertEqual((self.repo_root / out2["bench_report_path"]).read_text(encoding="utf-8"), "draft 2 final\n")


class TestPublishBenchmarkWrongTaskClass(_PublisherRepoTestCase):
    def test_rejected_for_non_full_benchmark_topic(self):
        init_publication(self.repo_root, "잘못된벤치", "model_serving_strategy")
        (self.repo_root / "r.md").write_text("x\n", encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "잘못된벤치", "PASS", "r.md")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("PUBLISH_BENCHMARK_WRONG_TASK_CLASS", out["reason_codes"])


# =============================================================================
# Section I -- `finalize`: delegates identity/link/PII/verdict checks to completion_gate.py --
# never a reimplemented copy of those validators; relays its reason codes/exit code verbatim.
# =============================================================================

def manifest_relative_evidence_paths(repo_root, topic):
    """Mirrors cmd_finalize's own os.path.relpath computation (evidence.*.path is documented as
    relative to the work-manifest file's OWN directory, docs/_evidence/ -- NOT repo-root-relative)
    so tests build pii_scan.scanned_paths against the exact strings completion_gate will compare."""
    record = json.loads((repo_root / "docs" / "_evidence" / f"{topic}.json").read_text(encoding="utf-8"))
    manifest_dir = repo_root / "docs" / "_evidence"
    return [
        os.path.relpath(str(repo_root / rel), str(manifest_dir))
        for rel in record.get("scaffolded", {}).values() if rel
    ]


def finalize(repo_root, topic, pii_scan=None, runtime=None, capacity_rejection_extra=None):
    args = ["finalize", "--topic", topic]
    if pii_scan is not None:
        p = repo_root / "_scratch_pii.json"
        write_json(p, pii_scan)
        args += ["--pii-scan-json", str(p)]
    if runtime is not None:
        p = repo_root / "_scratch_runtime.json"
        write_json(p, runtime)
        args += ["--runtime-json", str(p)]
    return run_publisher(args, repo_root)


def finalize_with_raw_json(repo_root, topic, flag, raw_json):
    p = repo_root / f"_scratch_{flag}.json"
    p.write_text(raw_json, encoding="utf-8")
    cli_flag = f"--{flag.replace('_', '-')}-json"
    return run_publisher(["finalize", "--topic", topic, cli_flag, str(p)], repo_root)


HEALTHY_RUNTIME = {
    "health_ok": True, "functional_smoke_passed": True,
    "identity": IDENTITY,
    "containers": [{"name": "svc", "restart_count": 0, "oom_killed": False}],
}


class TestFinalizeDelegatesToCompletionGate(_PublisherRepoTestCase):
    def test_malformed_cli_json_shapes_are_rejected_before_work_manifest_write(self):
        cases = [
            ("finalize_pii_null", "minor_patch", "pii_scan", "null"),
            ("finalize_runtime_list", "minor_patch", "runtime", "[]"),
            ("finalize_capacity_null", "capacity_rejection", "capacity_rejection", "null"),
        ]
        for topic, task_class, flag, raw in cases:
            with self.subTest(flag=flag):
                code, out, err = init_publication(self.repo_root, topic, task_class)
                self.assertEqual(code, 0, msg=f"out={out} err={err}")
                manifest_path = self.repo_root / "docs" / "_evidence" / f"{topic}.work-manifest.json"
                self.assertFalse(manifest_path.exists())
                code2, out2, err2 = finalize_with_raw_json(self.repo_root, topic, flag, raw)
                self.assertEqual(code2, 2, msg=f"out={out2} err={err2}")
                self.assertIn("FINALIZE_WORK_MANIFEST_SCHEMA_INVALID", out2["reason_codes"])
                self.assertFalse(manifest_path.exists())

    def test_minor_patch_reaches_evidence_complete_but_capped(self):
        init_publication(self.repo_root, "파이널마이너", "minor_patch")
        (self.repo_root / "verify.txt").write_text("py_compile OK\n", encoding="utf-8")
        (self.repo_root / "commit.txt").write_text("abc123\n", encoding="utf-8")
        append_raw(self.repo_root, "파이널마이너", "verification", "verify.txt")
        append_raw(self.repo_root, "파이널마이너", "commit", "commit.txt")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, "파이널마이너")
        code, out, err = finalize(self.repo_root, "파이널마이너",
                                   pii_scan={"passed": True, "scanned_paths": pii_paths},
                                   runtime=HEALTHY_RUNTIME)
        self.assertEqual(code, 1, msg=f"out={out} err={err}")  # policy-block: capped below promotion
        self.assertEqual(out["state"], "evidence-complete")
        self.assertFalse(out["eligible_for_promotion"])

    def test_missing_pii_scan_fails_closed(self):
        init_publication(self.repo_root, "파이널피아이누락", "minor_patch")
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        (self.repo_root / "commit.txt").write_text("abc\n", encoding="utf-8")
        append_raw(self.repo_root, "파이널피아이누락", "verification", "verify.txt")
        append_raw(self.repo_root, "파이널피아이누락", "commit", "commit.txt")
        code, out, err = finalize(self.repo_root, "파이널피아이누락", runtime=HEALTHY_RUNTIME)  # no pii_scan
        self.assertIn("PII_SCAN_FAILED", out["reason_codes"])
        self.assertNotEqual(out.get("state"), "evidence-complete")

    def test_identity_mismatch_in_certificate_rejected_at_publish_time(self):
        """Post-P2-04 remediation: an identity-mismatched certificate is now rejected at
        `publish-benchmark` time (never written to disk at all) rather than being silently
        published and only caught later when `finalize` delegates to completion_gate. This is a
        strictly earlier and stronger fail-closed point than the original 'surfaces at finalize'
        behavior this test used to assert -- see TestP204ContradictoryCertificateRejected for the
        dedicated publish-time reproduction. This test additionally confirms the pipeline stays
        fail-closed all the way through finalize when the certificate was never published (missing
        required evidence, not a promotable state)."""
        init_publication(self.repo_root, "파이널정체성불일치", "full_benchmark",
                          benchmark_mode="full", benchmark_verdict="PASS",
                          identity={**IDENTITY, "gpu": "H200"})
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")  # gpu: GB10
        code0, out0, err0 = publish_benchmark(self.repo_root, "파이널정체성불일치", "PASS", "r.md",
                                               certificate_src="cert.yaml")
        self.assertNotEqual(code0, 0, msg=f"out={out0} err={err0}")
        self.assertIn("PUBLISH_BENCHMARK_CERTIFICATE_IDENTITY_MISMATCH", out0.get("reason_codes", []))
        self.assertEqual(list((self.repo_root / "docs" / "benchmark").glob("benchmark_*")), [])

        (self.repo_root / "devlog_src.md").write_text("x\n", encoding="utf-8")
        (self.repo_root / "testlog_src.md").write_text("x\n", encoding="utf-8")
        append_raw(self.repo_root, "파이널정체성불일치", "devlog", "devlog_src.md")
        append_raw(self.repo_root, "파이널정체성불일치", "testlog", "testlog_src.md")
        set_narrative(self.repo_root, "파이널정체성불일치", "plan", "devlog_src.md")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, "파이널정체성불일치")
        code, out, err = finalize(self.repo_root, "파이널정체성불일치",
                                   pii_scan={"passed": True, "scanned_paths": pii_paths},
                                   runtime={**HEALTHY_RUNTIME, "identity": {**IDENTITY, "gpu": "H200"}})
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=out)

    def test_full_benchmark_pass_end_to_end_reaches_promotion_ready(self):
        """Post-P2-A01 remediation: `finalize` requires every required narrative-bearing document
        (plan/devlog/testlog) to be EXPLICITLY authored via `set-narrative` -- a scaffold that
        still holds raw evidence (append-raw) but no authored narrative no longer counts as
        existing evidence (see TestP2A01PlaceholderPromotionBlocked). This positive control
        authors all three via `set-narrative` (never hand-edits a file) so promotion-ready must
        still be reachable when every other gate input is valid."""
        topic = "파이널완주"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        publish_benchmark(self.repo_root, topic, "PASS", "r.md", certificate_src="cert.yaml")
        for kind in ("devlog", "testlog"):
            src = f"{kind}_src.md"
            (self.repo_root / src).write_text(f"{kind} content\n", encoding="utf-8")
            append_raw(self.repo_root, topic, kind, src)
        for kind in ("plan", "devlog", "testlog"):
            narr_src = f"{kind}_narr.md"
            (self.repo_root / narr_src).write_text(f"{kind} narrative\n", encoding="utf-8")
            set_narrative(self.repo_root, topic, kind, narr_src)
        (self.repo_root / "sim_evidence.log").write_text("trial01 kv=16GiB\n", encoding="utf-8")
        append_raw(self.repo_root, topic, "simlog", "sim_evidence.log")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, topic)
        code, out, err = finalize(self.repo_root, topic,
                                   pii_scan={"passed": True, "scanned_paths": pii_paths},
                                   runtime=HEALTHY_RUNTIME)
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertEqual(out["state"], "promotion-ready")
        self.assertTrue(out["eligible_for_promotion"])

    def test_finalize_requires_init(self):
        code, out, err = finalize(self.repo_root, "미초기화파이널")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertIn("PUBLICATION_NOT_INITIALIZED", out["reason_codes"])


# =============================================================================
# Section J -- `--self-test` CLI smoke
# =============================================================================

class TestPublisherSelfTest(unittest.TestCase):
    def test_self_test_flag_exits_zero_and_prints_pass(self):
        proc = subprocess.run([sys.executable, str(PUBLISHER_SCRIPT), "--self-test"],
                               capture_output=True, text=True, timeout=30)
        self.assertEqual(proc.returncode, 0, msg=f"stdout={proc.stdout!r} stderr={proc.stderr!r}")
        self.assertIn("PASS", proc.stdout + proc.stderr)


# =============================================================================
# Section K -- Remediation for reliability review `deleg_1847701e` (staged tree
# 48ade456260e24db2f56e7bd7b9328fac6c16bdf, 7 blockers P2-01..P2-07). These are PERMANENT
# regression reproductions -- written and run for genuine RED against the pre-fix code before any
# production change, per strict-TDD remediation instructions.
# =============================================================================

# ---- P2-01: stable one-JSON exit2 for malformed input (no traceback) ----

class TestP201MalformedInputStableExit2(_PublisherRepoTestCase):
    def test_malformed_persisted_record_json_stable_exit2_no_traceback(self):
        init_publication(self.repo_root, "P201망가진레코드", "model_serving_strategy")
        record_path = self.repo_root / "docs" / "_evidence" / "P201망가진레코드.json"
        record_path.write_text("{ this is not valid json !!!", encoding="utf-8")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "P201망가진레코드", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertTrue(out.get("reason_codes"), msg=out)
        self.assertNotIn("Traceback", err)
        self.assertFalse(out.get("ok", True))

    def test_invalid_calendar_timestamp_stable_exit2_no_traceback(self):
        identity_path = self.repo_root / "_id.json"
        write_json(identity_path, IDENTITY)
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "init", "--topic", "P201잘못된시각",
               "--task-class", "model_serving_strategy", "--generated-utc", "2026-99-99T99:99:99Z",
               "--identity-json", str(identity_path), "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertEqual(proc.returncode, 2, msg=f"stdout={proc.stdout} stderr={proc.stderr}")
        self.assertNotIn("Traceback", proc.stderr)
        out = json.loads(proc.stdout)
        self.assertTrue(out.get("reason_codes"), msg=out)

    def test_invalid_report_slug_stable_exit2_no_traceback(self):
        identity_path = self.repo_root / "_id.json"
        write_json(identity_path, IDENTITY)
        cond_path = self.repo_root / "_cond.json"
        write_json(cond_path, {"report_requested": True})
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "init", "--topic", "P201나쁜슬러그",
               "--task-class", "read_only_audit", "--generated-utc", "2026-07-25T02:00:00Z",
               "--identity-json", str(identity_path), "--conditions-json", str(cond_path),
               "--report-slug", "BAD/slug", "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertEqual(proc.returncode, 2, msg=f"stdout={proc.stdout} stderr={proc.stderr}")
        self.assertNotIn("Traceback", proc.stderr)
        out = json.loads(proc.stdout)
        self.assertTrue(out.get("reason_codes"), msg=out)
        # must not have written anything under docs/report/
        self.assertEqual(list((self.repo_root / "docs" / "report").glob("*")), [])


# ---- P2-02: output containment/symlink safety ----

class TestP202ContainmentAndSymlinkSafety(_PublisherRepoTestCase):
    def test_topic_path_traversal_does_not_escape_evidence_dir(self):
        identity_path = self.repo_root / "_id.json"
        write_json(identity_path, IDENTITY)
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "init", "--topic", "../escaped",
               "--task-class", "model_serving_strategy", "--generated-utc", "2026-07-25T02:00:00Z",
               "--identity-json", str(identity_path), "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertNotIn("Traceback", proc.stderr)
        self.assertFalse((self.repo_root / "docs" / "escaped.json").exists(),
                          msg="topic path traversal must not write outside docs/_evidence/")
        self.assertNotEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}")

    def test_tampered_scaffolded_plan_path_rejected_not_written_to_repo_root(self):
        code, out, _ = init_publication(self.repo_root, "P202변조", "model_serving_strategy")
        record_path = self.repo_root / "docs" / "_evidence" / "P202변조.json"
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["scaffolded"]["plan"] = "outside.md"  # NOT under docs/plan/
        record_path.write_text(json.dumps(record), encoding="utf-8")
        code2, out2, err2 = init_publication(self.repo_root, "P202변조", "model_serving_strategy")
        self.assertNotIn("Traceback", err2)
        self.assertFalse((self.repo_root / "outside.md").exists(),
                          msg="tampered scaffolded.plan must never be trusted as a write target")
        self.assertNotEqual(code2, 0, msg=f"out={out2}")

    def test_tampered_scaffolded_plan_escape_rejected(self):
        code, out, _ = init_publication(self.repo_root, "P202이탈변조", "model_serving_strategy")
        record_path = self.repo_root / "docs" / "_evidence" / "P202이탈변조.json"
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["scaffolded"]["plan"] = "../../../../../../etc/nastyfile.md"
        record_path.write_text(json.dumps(record), encoding="utf-8")
        code2, out2, err2 = init_publication(self.repo_root, "P202이탈변조", "model_serving_strategy")
        self.assertNotIn("Traceback", err2)
        self.assertNotEqual(code2, 0, msg=f"out={out2}")

    def test_scaffold_directory_symlink_redirect_rejected(self):
        outside = Path(self._td.name) / "outside_target"
        outside.mkdir()
        (self.repo_root / "docs" / "plan").rmdir()
        os.symlink(outside, self.repo_root / "docs" / "plan")
        identity_path = self.repo_root / "_id.json"
        write_json(identity_path, IDENTITY)
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "init", "--topic", "P202심볼릭",
               "--task-class", "model_serving_strategy", "--generated-utc", "2026-07-25T02:00:00Z",
               "--identity-json", str(identity_path), "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertNotIn("Traceback", proc.stderr)
        self.assertEqual(list(outside.glob("*")), [],
                          msg="must never write through a symlinked docs/plan into the outside target")
        self.assertNotEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}")


# ---- P2-03: narrative marker injection ----

class TestP203NarrativeMarkerInjection(_PublisherRepoTestCase):
    def test_narrative_containing_end_marker_rejected(self):
        code, out, _ = init_publication(self.repo_root, "P203마커주입", "model_serving_strategy")
        plan_rel = out["scaffolded"]["plan"]
        before = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        malicious = self.repo_root / "malicious.md"
        malicious.write_text("normal text\n<!-- NARRATIVE:END -->\ninjected extra\n", encoding="utf-8")
        code2, out2, err2 = set_narrative(self.repo_root, "P203마커주입", "plan", "malicious.md")
        self.assertNotEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertNotIn("Traceback", err2)
        after = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        self.assertEqual(before, after, msg="rejected narrative must not modify the scaffold at all")

    def test_narrative_containing_begin_marker_rejected(self):
        init_publication(self.repo_root, "P203시작마커", "model_serving_strategy")
        malicious = self.repo_root / "malicious2.md"
        malicious.write_text("<!-- NARRATIVE:BEGIN -->\nfake\n", encoding="utf-8")
        code2, out2, err2 = set_narrative(self.repo_root, "P203시작마커", "plan", "malicious2.md")
        self.assertNotEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertNotIn("Traceback", err2)

    def test_second_authoring_after_crafted_first_leaves_no_corruption(self):
        """Permanent reproduction of the exact reviewer scenario: even if a first authoring
        attempt SOMEHOW got a reserved marker into the scaffold (e.g. direct external tampering,
        not through set-narrative -- since set-narrative itself now rejects such input), a second
        legitimate set-narrative call must fail closed rather than silently leaving old text and
        two marker pairs."""
        code, out, _ = init_publication(self.repo_root, "P203이중마커", "model_serving_strategy")
        plan_rel = out["scaffolded"]["plan"]
        target = self.repo_root / plan_rel
        # simulate external tampering that leaves a malformed (extra END) scaffold on disk
        tampered = target.read_text(encoding="utf-8").replace(
            "<!-- NARRATIVE:END -->",
            "old narrative text\n<!-- NARRATIVE:END -->\n<!-- NARRATIVE:END -->",
        )
        target.write_text(tampered, encoding="utf-8")
        (self.repo_root / "clean.md").write_text("clean new narrative\n", encoding="utf-8")
        code2, out2, err2 = set_narrative(self.repo_root, "P203이중마커", "plan", "clean.md")
        self.assertNotEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertNotIn("Traceback", err2)
        after = target.read_text(encoding="utf-8")
        self.assertEqual(after, tampered, msg="a malformed scaffold must be left untouched, not partially rewritten")


# ---- P2-04: contradictory certificate must not be published before finalize ----

CONTRADICTORY_CERTIFICATE = """\
model: some-other-model
gpu_model: H200
vllm_version: 9.9.9
quantization: bf16
topology: single
tensor_parallel_size: 1
verdict: FAIL
benchmark_mode: lite
"""


class TestP204ContradictoryCertificateRejected(_PublisherRepoTestCase):
    def test_contradictory_certificate_rejected_not_published(self):
        init_publication(self.repo_root, "P204모순인증서", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        (self.repo_root / "bad_cert.yaml").write_text(CONTRADICTORY_CERTIFICATE, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "P204모순인증서", "PASS", "r.md",
                                            certificate_src="bad_cert.yaml")
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(list((self.repo_root / "docs" / "benchmark").glob("benchmark_*")), [],
                          msg="a certificate whose own content contradicts the record must never be published")

    def test_wrong_verdict_inside_certificate_rejected_even_with_cli_verdict_pass(self):
        """Exact reviewer repro: cert with verdict=FAIL accepted with --verdict PASS."""
        init_publication(self.repo_root, "P204verdict불일치", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        cert = VALID_FLAT_CERTIFICATE.replace("verdict: PASS", "verdict: FAIL")
        (self.repo_root / "fail_cert.yaml").write_text(cert, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "P204verdict불일치", "PASS", "r.md",
                                            certificate_src="fail_cert.yaml")
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIn("PUBLISH_BENCHMARK_CERTIFICATE_VERDICT_MISMATCH", out.get("reason_codes", []))
        self.assertEqual(list((self.repo_root / "docs" / "benchmark").glob("benchmark_*")), [])

    def test_wrong_benchmark_mode_inside_certificate_rejected(self):
        init_publication(self.repo_root, "P204mode불일치", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        cert = VALID_FLAT_CERTIFICATE.replace("benchmark_mode: full", "benchmark_mode: lite")
        (self.repo_root / "lite_cert.yaml").write_text(cert, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "P204mode불일치", "PASS", "r.md",
                                            certificate_src="lite_cert.yaml")
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIn("PUBLISH_BENCHMARK_CERTIFICATE_MODE_MISMATCH", out.get("reason_codes", []))

    def test_wrong_identity_inside_certificate_rejected(self):
        init_publication(self.repo_root, "P204identity불일치", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        cert = VALID_FLAT_CERTIFICATE.replace("gpu_model: GB10", "gpu_model: H200")
        (self.repo_root / "wrong_gpu_cert.yaml").write_text(cert, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "P204identity불일치", "PASS", "r.md",
                                            certificate_src="wrong_gpu_cert.yaml")
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIn("PUBLISH_BENCHMARK_CERTIFICATE_IDENTITY_MISMATCH", out.get("reason_codes", []))

    def test_matching_certificate_still_publishes_no_regression(self):
        init_publication(self.repo_root, "P204정상", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        (self.repo_root / "good_cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "P204정상", "PASS", "r.md",
                                            certificate_src="good_cert.yaml")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIsNotNone(out["certificate_path"])


# ---- P2-05: naming helper must never return an occupied derived suffix ----

class TestP205NamingCollisionExhausted(unittest.TestCase):
    def test_dated_doc_basename_third_collision_fails_closed(self):
        import doc_naming
        gen = "2026-07-25T06:58:29Z"  # hour token 26072515, mm=58, ss=29
        base = "testlog_26072515_구조개선.md"
        suffixed = "testlog_26072515_58_29_구조개선.md"
        with self.assertRaises(Exception) as ctx:
            doc_naming.dated_doc_basename("testlog", gen, "구조개선", existing_basenames={base, suffixed})
        # must be a ValueError-family exception (stable, catchable at the CLI boundary) -- never
        # silently return an already-occupied name.
        self.assertIsInstance(ctx.exception, ValueError)

    def test_simlog_dirname_third_collision_fails_closed(self):
        import doc_naming
        gen = "2026-06-21T12:21:33Z"
        base = "26062121_run"
        suffixed = "26062121_21_33_run"
        with self.assertRaises(ValueError):
            doc_naming.simlog_dirname(gen, "run", existing_dirnames={base, suffixed})

    def test_bench_filename_third_collision_fails_closed(self):
        import doc_naming
        meta = {"model": "solar-open2-250b", "gpu_key": "GB10", "vllm_version": "0.22.0"}
        gen = "2026-07-24T16:22:29Z"
        base = "bench_report_26072501_solar-open2-250b_GB10_0.22.0.md"
        suffixed = "bench_report_26072501_22_29_solar-open2-250b_GB10_0.22.0.md"
        with self.assertRaises(ValueError):
            doc_naming.bench_filename("bench_report", meta, gen, existing_basenames={base, suffixed})

    def test_cli_level_third_collision_fails_closed_and_preserves_existing_files(self):
        """End-to-end: init() for a topic whose BOTH the unsuffixed and _MM_SS candidate names are
        already occupied (by other publications' real files) must fail closed rather than
        overwriting either occupant."""
        with tempfile.TemporaryDirectory() as td:
            repo_root = make_repo(Path(td))
            gen = "2026-07-25T06:58:29Z"
            topic = "충돌토픽"
            import doc_naming
            plan_dir = repo_root / "docs" / "plan"
            plan_dir.mkdir(parents=True, exist_ok=True)
            base_name = doc_naming.dated_doc_basename("plan", gen, topic, existing_basenames=set())
            (plan_dir / base_name).write_text("occupant A -- must survive\n", encoding="utf-8")
            mm_ss_name = doc_naming.dated_doc_basename("plan", gen, topic, existing_basenames={base_name})
            (plan_dir / mm_ss_name).write_text("occupant B -- must survive\n", encoding="utf-8")

            identity_path = repo_root / "_id.json"
            write_json(identity_path, IDENTITY)
            cmd = [sys.executable, str(PUBLISHER_SCRIPT), "init", "--topic", topic,
                   "--task-class", "model_serving_strategy", "--generated-utc", gen,
                   "--identity-json", str(identity_path), "--repo-root", str(repo_root)]
            proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
            self.assertNotIn("Traceback", proc.stderr)
            self.assertNotEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}")
            self.assertEqual((plan_dir / base_name).read_text(encoding="utf-8"), "occupant A -- must survive\n")
            self.assertEqual((plan_dir / mm_ss_name).read_text(encoding="utf-8"), "occupant B -- must survive\n")


# ---- P2-06: concurrent append-raw must not assign duplicate seq ----

class TestP206ConcurrentAppendRaw(_PublisherRepoTestCase):
    def test_16_concurrent_appends_produce_unique_contiguous_seq(self):
        init_publication(self.repo_root, "P206동시성", "harness_change", conditions={"actual_trial": False})
        n = 16
        src_files = []
        for i in range(n):
            p = self.repo_root / f"src_{i}.txt"
            p.write_text(f"payload {i}\n", encoding="utf-8")
            src_files.append(f"src_{i}.txt")

        procs = []
        for i, src in enumerate(src_files):
            cmd = [sys.executable, str(PUBLISHER_SCRIPT), "append-raw", "--topic", "P206동시성",
                   "--kind", "testlog", "--src", src, "--recorded-utc", "2026-07-25T02:30:00Z",
                   "--repo-root", str(self.repo_root)]
            procs.append(subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True))

        results = [p.communicate(timeout=60) for p in procs]
        returncodes = [p.returncode for p in procs]
        self.assertEqual(returncodes, [0] * n, msg=results)

        sidecar = self.repo_root / "docs" / "_evidence" / "P206동시성.testlog.raw.jsonl"
        lines = sidecar.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), n, msg="every concurrent append must land as its own line -- none lost")
        seqs = sorted(json.loads(line)["seq"] for line in lines)
        self.assertEqual(seqs, list(range(1, n + 1)),
                          msg=f"seq must be unique and contiguous 1..{n}, got {seqs}")


# =============================================================================
# Section L -- Remediation for reliability review `subagent-summary-0-20260725_115851_232115`
# (staged tree e25e11c166493efa5558c6441cd5907e6346b9a3, 5 blockers P2-A01..P2-A05). These are
# PERMANENT regression reproductions -- written and run for genuine RED against the pre-fix code
# before any production change, per strict-TDD remediation instructions.
# =============================================================================

# ---- P2-A01: finalize must fail closed on placeholder/un-authored narrative -- direct file
# tampering must not substitute for `set-narrative`. ----

class TestP2A01PlaceholderPromotionBlocked(_PublisherRepoTestCase):
    def test_exact_reviewer_repro_placeholder_narratives_block_promotion(self):
        """Reviewer repro: init full_benchmark PASS, publish a valid report/certificate, append
        only one simlog file, leave plan/devlog/testlog narratives untouched, then finalize with
        valid runtime/PII. Expected: state stays below promotion-ready because plan/devlog/testlog
        still hold their scaffold `_PLACEHOLDER` and narrative_status is empty for them."""
        topic = "A01재현"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code0, out0, err0 = publish_benchmark(self.repo_root, topic, "PASS", "r.md", certificate_src="cert.yaml")
        self.assertEqual(code0, 0, msg=f"out={out0} err={err0}")
        (self.repo_root / "sim_evidence.log").write_text("trial01 kv=16GiB\n", encoding="utf-8")
        append_raw(self.repo_root, topic, "simlog", "sim_evidence.log")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, topic)
        code, out, err = finalize(self.repo_root, topic,
                                   pii_scan={"passed": True, "scanned_paths": pii_paths},
                                   runtime=HEALTHY_RUNTIME)
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=f"out={out} err={err}")
        self.assertFalse(out.get("eligible_for_promotion"), msg=out)
        self.assertEqual(out.get("state"), "runtime-ready", msg=out)
        self.assertIn("EVIDENCE_MISSING:plan", out.get("reason_codes", []), msg=out)
        self.assertIn("EVIDENCE_MISSING:devlog", out.get("reason_codes", []), msg=out)
        self.assertIn("EVIDENCE_MISSING:testlog", out.get("reason_codes", []), msg=out)

    def test_direct_file_tampering_without_set_narrative_still_blocks(self):
        """Directly editing the scaffold file to remove `_PLACEHOLDER` (never calling
        set-narrative, so narrative_status stays un-authored) must NOT count as authored
        narrative -- file tampering is not a substitute for going through the CLI."""
        topic = "A01파일변조"
        init_publication(self.repo_root, topic, "harness_change", conditions={"actual_trial": False})
        import evidence_publisher as pub
        record = json.loads((self.repo_root / "docs" / "_evidence" / f"{topic}.json").read_text(encoding="utf-8"))
        for kind in ("plan", "devlog", "testlog"):
            path = self.repo_root / record["scaffolded"][kind]
            tampered = path.read_text(encoding="utf-8").replace(
                pub._PLACEHOLDER, "hand-edited prose, no set-narrative call")
            path.write_text(tampered, encoding="utf-8")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, topic)
        code, out, err = finalize(self.repo_root, topic,
                                   pii_scan={"passed": True, "scanned_paths": pii_paths},
                                   runtime=HEALTHY_RUNTIME)
        self.assertNotEqual(out.get("state"), "evidence-complete", msg=f"out={out} err={err}")
        self.assertIn("EVIDENCE_MISSING:plan", out.get("reason_codes", []), msg=out)
        self.assertIn("EVIDENCE_MISSING:devlog", out.get("reason_codes", []), msg=out)
        self.assertIn("EVIDENCE_MISSING:testlog", out.get("reason_codes", []), msg=out)

    def test_full_narrative_via_set_narrative_still_reaches_promotion_ready(self):
        """Positive control: when every required narrative document is explicitly authored via
        `set-narrative` (never hand-edited) and every other gate input is valid, promotion-ready
        must still be reachable -- the A01 fix must not be overbroad."""
        topic = "A01정상완주"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        publish_benchmark(self.repo_root, topic, "PASS", "r.md", certificate_src="cert.yaml")
        for kind in ("plan", "devlog", "testlog"):
            src = f"{kind}_narr.md"
            (self.repo_root / src).write_text(f"{kind} narrative content\n", encoding="utf-8")
            set_narrative(self.repo_root, topic, kind, src)
        (self.repo_root / "sim_evidence.log").write_text("trial01 kv=16GiB\n", encoding="utf-8")
        append_raw(self.repo_root, topic, "simlog", "sim_evidence.log")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, topic)
        code, out, err = finalize(self.repo_root, topic,
                                   pii_scan={"passed": True, "scanned_paths": pii_paths},
                                   runtime=HEALTHY_RUNTIME)
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertEqual(out["state"], "promotion-ready")
        self.assertTrue(out["eligible_for_promotion"])


# ---- P2-A02: a PASS->FAIL transition must clear the persisted certificate association; only a
# validated, contained, non-symlink regular file under docs/benchmark/ is ever unlinked. ----

class TestP2A02CertificateClearedOnFailTransition(_PublisherRepoTestCase):
    def test_pass_then_fail_clears_persisted_and_removes_file(self):
        topic = "A02PASS후FAIL"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r1.md").write_text("passing report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code0, out0, err0 = publish_benchmark(self.repo_root, topic, "PASS", "r1.md", certificate_src="cert.yaml")
        self.assertEqual(code0, 0, msg=f"out={out0} err={err0}")
        cert_path = self.repo_root / out0["certificate_path"]
        self.assertTrue(cert_path.is_file())

        (self.repo_root / "r2.md").write_text("it regressed\n", encoding="utf-8")
        code1, out1, err1 = publish_benchmark(self.repo_root, topic, "FAIL", "r2.md")
        self.assertEqual(code1, 0, msg=f"out={out1} err={err1}")
        self.assertIsNone(out1["certificate_path"], msg=out1)

        record = json.loads((self.repo_root / "docs" / "_evidence" / f"{topic}.json").read_text(encoding="utf-8"))
        self.assertIsNone(record["scaffolded"].get("certificate"),
                          msg="persisted record must not still associate the FAIL run with the old PASS certificate")
        self.assertEqual(record["benchmark"], {"mode": "full", "verdict": "FAIL"})
        self.assertFalse(cert_path.exists(), msg="the publisher-owned prior certificate file must be removed")

    def test_pass_then_fail_finalize_report_only_no_certificate_leak(self):
        """After a PASS->FAIL transition, `finalize` must never surface a certificate for this
        topic (report-only), matching the reviewer's 'result and record must be report-only'
        requirement end-to-end through completion_gate.py."""
        topic = "A02리포트전용"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r1.md").write_text("passing report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        publish_benchmark(self.repo_root, topic, "PASS", "r1.md", certificate_src="cert.yaml")
        (self.repo_root / "r2.md").write_text("it regressed\n", encoding="utf-8")
        publish_benchmark(self.repo_root, topic, "FAIL", "r2.md")
        for kind in ("plan", "devlog", "testlog"):
            src = f"{kind}_narr.md"
            (self.repo_root / src).write_text(f"{kind} content\n", encoding="utf-8")
            set_narrative(self.repo_root, topic, kind, src)
        (self.repo_root / "sim_evidence.log").write_text("trial01\n", encoding="utf-8")
        append_raw(self.repo_root, topic, "simlog", "sim_evidence.log")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, topic)
        code, out, err = finalize(self.repo_root, topic,
                                   pii_scan={"passed": True, "scanned_paths": pii_paths},
                                   runtime=HEALTHY_RUNTIME)
        self.assertIsNone(out.get("certificate"), msg=f"out={out} err={err}")
        self.assertNotEqual(out.get("state"), "promotion-ready", msg=out)  # FAIL verdict never promotes

    def test_tampered_prior_certificate_escape_path_never_unlinked(self):
        """Defense: if the persisted record's scaffolded.certificate were tampered to point
        outside docs/benchmark/, the FAIL-transition cleanup must refuse to unlink it -- never
        trust a persisted path as an unlink target without re-validating it the same hardened way
        every other publisher-controlled write path is validated."""
        topic = "A02변조인증서"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r1.md").write_text("passing report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        publish_benchmark(self.repo_root, topic, "PASS", "r1.md", certificate_src="cert.yaml")

        outside = self.repo_root.parent / "outside_cert.yaml"
        outside.write_text("not a real certificate location\n", encoding="utf-8")
        record_path = self.repo_root / "docs" / "_evidence" / f"{topic}.json"
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["scaffolded"]["certificate"] = "../outside_cert.yaml"
        record_path.write_text(json.dumps(record), encoding="utf-8")

        (self.repo_root / "r2.md").write_text("it regressed\n", encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, topic, "FAIL", "r2.md")
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertTrue(outside.exists(), msg="a tampered/out-of-bounds certificate path must never be unlinked")

    def test_tampered_certificate_symlink_never_followed_for_cleanup(self):
        """Defense: a certificate path record-tampered into a symlink must never be unlinked
        through (which could delete an arbitrary target chosen by whoever planted the symlink)."""
        topic = "A02심볼릭인증서"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r1.md").write_text("passing report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code0, out0, _ = publish_benchmark(self.repo_root, topic, "PASS", "r1.md", certificate_src="cert.yaml")

        outside = self.repo_root.parent / "outside_target.yaml"
        outside.write_text("outside content\n", encoding="utf-8")
        real_cert_path = self.repo_root / out0["certificate_path"]
        real_cert_path.unlink()
        os.symlink(outside, real_cert_path)

        (self.repo_root / "r2.md").write_text("it regressed\n", encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, topic, "FAIL", "r2.md")
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertTrue(outside.exists(), msg="must never unlink through a symlinked certificate path")
        self.assertTrue(real_cert_path.is_symlink())

    def test_fresh_fail_with_no_prior_certificate_unaffected(self):
        """Positive control (no regression): a topic that FAILs on its very first publish (never
        had a certificate) must behave exactly as before."""
        init_publication(self.repo_root, "A02최초FAIL", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="FAIL")
        (self.repo_root / "report_src.md").write_text("# why it was slow\n", encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "A02최초FAIL", "FAIL", "report_src.md")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIsNone(out["certificate_path"])

    def test_fresh_pass_unaffected(self):
        """Positive control (no regression): a topic that PASSes on its very first publish must
        still publish its certificate normally."""
        init_publication(self.repo_root, "A02최초PASS", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="PASS")
        (self.repo_root / "r.md").write_text("report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, "A02최초PASS", "PASS", "r.md", certificate_src="cert.yaml")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertIsNotNone(out["certificate_path"])
        self.assertTrue((self.repo_root / out["certificate_path"]).is_file())


# ---- P2-A03: a syntactically-valid-JSON but wrong-shape persisted record must never surface a
# raw Python traceback -- one stable JSON reason code, exit 2, every time. ----

class TestP2A03MalformedRecordShapeStableExit2(_PublisherRepoTestCase):
    def _write_record(self, topic, raw_text):
        record_path = self.repo_root / "docs" / "_evidence" / f"{topic}.json"
        record_path.parent.mkdir(parents=True, exist_ok=True)
        record_path.write_text(raw_text, encoding="utf-8")

    def test_top_level_json_array_rejected_via_finalize(self):
        """Exact reviewer repro: `[]` persisted as the record, then `finalize`."""
        topic = "A03배열"
        self._write_record(topic, "[]")
        code, out, err = finalize(self.repo_root, topic)
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertTrue(out.get("reason_codes"), msg=out)

    def test_top_level_json_null_rejected(self):
        topic = "A03널"
        self._write_record(topic, "null")
        code, out, err = finalize(self.repo_root, topic)
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_top_level_json_scalar_number_rejected(self):
        topic = "A03숫자"
        self._write_record(topic, "42")
        code, out, err = finalize(self.repo_root, topic)
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_top_level_json_scalar_string_rejected(self):
        topic = "A03문자열"
        self._write_record(topic, '"just a string"')
        code, out, err = finalize(self.repo_root, topic)
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_top_level_json_boolean_rejected(self):
        topic = "A03불리언"
        self._write_record(topic, "true")
        code, out, err = finalize(self.repo_root, topic)
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_malformed_nested_scaffolded_field_rejected(self):
        """A syntactically valid JSON object whose 'scaffolded' sub-field is itself the wrong
        shape (a list, not an object) must also fail closed rather than crashing a downstream
        `.get(kind)` call."""
        topic = "A03중첩배열"
        self._write_record(topic, json.dumps({
            "schema_version": 1, "publication_id": topic, "task_class": "minor_patch",
            "generated_utc": "2026-01-01T00:00:00Z", "identity": IDENTITY,
            "conditions": {}, "benchmark": {"mode": None, "verdict": None},
            "required_evidence": ["verification", "commit"],
            "or_group_scaffolded": [], "capacity_rejection_required": False,
            "scaffolded": ["not", "an", "object"],
            "narrative_status": {}, "raw_log_paths": {}, "capacity_rejection": None,
        }))
        code, out, err = finalize(self.repo_root, topic)
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_malformed_shape_rejected_via_append_raw_too(self):
        """Single-choke-point regression: the same malformed record must fail closed through
        append-raw as well, not only finalize (proves the fix lives at the shared record-load
        point, not a one-off patch inside cmd_finalize alone)."""
        topic = "A03append경로"
        self._write_record(topic, "[]")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, topic, "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_valid_object_record_positive_control_not_rejected(self):
        """Positive control: a properly-shaped record (real `init` output) must NOT trip the new
        shape guard."""
        topic = "A03정상객체"
        code, out, err = init_publication(self.repo_root, topic, "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, topic, "verification", "verify.txt")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")


# ---- P2-A04: naming collision is keyed on (doc_type, hour) / (kind, hour), regardless of topic
# -- CLI-level (end-to-end) regression, complementing the pure-function tests in Section A. ----

class TestP2A04NamingCollisionRegardlessOfTopicCli(_PublisherRepoTestCase):
    def test_cli_level_second_topic_same_hour_gets_mm_ss_suffix(self):
        """Two DIFFERENT topics published in the same YYMMDDHH hour must not collide-name -- the
        second publication's plan document must land on the _MM_SS form even though its own topic
        never literally matches the first publication's basename."""
        gen = "2026-07-25T06:58:29Z"  # hour token 26072515, mm=58, ss=29
        code1, out1, err1 = init_publication(self.repo_root, "토픽A", "model_serving_strategy", generated_utc=gen)
        self.assertEqual(code1, 0, msg=f"out={out1} err={err1}")
        code2, out2, err2 = init_publication(self.repo_root, "토픽B", "model_serving_strategy", generated_utc=gen)
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertEqual(Path(out1["scaffolded"]["plan"]).name, "plan_26072515_토픽A.md")
        self.assertEqual(Path(out2["scaffolded"]["plan"]).name, "plan_26072515_58_29_토픽B.md")
        self.assertTrue((self.repo_root / out1["scaffolded"]["plan"]).is_file())
        self.assertTrue((self.repo_root / out2["scaffolded"]["plan"]).is_file())

    def test_republishing_same_topic_reuses_persisted_filename_not_a_fresh_suffix(self):
        """Idempotency must survive the broadened collision rule: re-running `init` for the SAME
        topic must reuse the filename already persisted in its own record, never recompute (and
        potentially re-suffix) a fresh name just because another topic now also occupies this
        hour."""
        gen = "2026-07-25T06:58:29Z"
        init_publication(self.repo_root, "토픽A2", "model_serving_strategy", generated_utc=gen)
        init_publication(self.repo_root, "토픽B2", "model_serving_strategy", generated_utc=gen)
        code3, out3, err3 = init_publication(self.repo_root, "토픽A2", "model_serving_strategy", generated_utc=gen)
        self.assertEqual(code3, 0, msg=f"out={out3} err={err3}")
        self.assertEqual(Path(out3["scaffolded"]["plan"]).name, "plan_26072515_토픽A2.md")
        self.assertEqual(out3["already_existed"], ["devlog", "plan", "simlog", "testlog"])

    def test_simlog_dirname_collision_across_topics_same_hour(self):
        gen = "2026-07-25T06:58:29Z"
        code1, out1, err1 = init_publication(self.repo_root, "런A", "model_serving_strategy", generated_utc=gen)
        code2, out2, err2 = init_publication(self.repo_root, "런B", "model_serving_strategy", generated_utc=gen)
        self.assertEqual(code1, 0, msg=f"out={out1} err={err1}")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertEqual(Path(out1["scaffolded"]["simlog"]).name, "26072515_런A")
        self.assertEqual(Path(out2["scaffolded"]["simlog"]).name, "26072515_58_29_런B")

    def test_bench_filename_collision_across_topics_same_hour(self):
        """Even with DIFFERENT model identities (so the old exact-combo-match rule would never
        have collided them), two full_benchmark FAIL publications in the same hour must still
        collide on the shared bench_report kind/hour bucket."""
        gen = "2026-07-25T06:58:29Z"
        id_a = {**IDENTITY, "model": "model-a4"}
        id_b = {**IDENTITY, "model": "model-b4"}
        init_publication(self.repo_root, "벤치A4", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="FAIL", generated_utc=gen, identity=id_a)
        init_publication(self.repo_root, "벤치B4", "full_benchmark", benchmark_mode="full",
                          benchmark_verdict="FAIL", generated_utc=gen, identity=id_b)
        (self.repo_root / "r1.md").write_text("report A\n", encoding="utf-8")
        (self.repo_root / "r2.md").write_text("report B\n", encoding="utf-8")
        code1, out1, err1 = publish_benchmark(self.repo_root, "벤치A4", "FAIL", "r1.md", generated_utc=gen)
        code2, out2, err2 = publish_benchmark(self.repo_root, "벤치B4", "FAIL", "r2.md", generated_utc=gen)
        self.assertEqual(code1, 0, msg=f"out={out1} err={err1}")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertEqual(Path(out1["bench_report_path"]).name, "bench_report_26072515_model-a4_GB10_0.22.0.md")
        self.assertEqual(Path(out2["bench_report_path"]).name,
                          "bench_report_26072515_58_29_model-b4_GB10_0.22.0.md")


# =============================================================================
# Section M -- Remediation for reliability review `deleg_c118a2d9` (staged tree
# 3a8769b7648c96898ca94e91e114ebf3336a93a9, task index 1 -- 3 blockers P2-FINAL-01..03 approved
# for this remediation pass; P2-FINAL-04/05 are explicitly OUT of scope here). These are PERMANENT
# regression reproductions -- written and run for genuine RED against the pre-fix code before any
# production change, per strict-TDD remediation instructions.
# =============================================================================

# ---- P2-FINAL-01: docs/_evidence/ publication-record and JSONL-sidecar destinations must be
# repo-contained and symlink-safe -- reuses the SAME hardened dir_fd/O_NOFOLLOW machinery already
# used for scaffold writes, never merely resolve-then-write. ----

class TestPFinal01EvidenceDirSymlinkSafety(_PublisherRepoTestCase):
    def test_exact_reviewer_repro_docs_evidence_symlink_redirect_rejected_on_fresh_init(self):
        """Exact reviewer repro: replace docs/_evidence with a symlink to an external directory
        BEFORE any init ever ran for this repo, then run init."""
        outside = Path(self._td.name) / "outside_evidence_dir"
        outside.mkdir()
        os.symlink(outside, self.repo_root / "docs" / "_evidence")
        identity_path = self.repo_root / "_id.json"
        write_json(identity_path, IDENTITY)
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "init", "--topic", "F01심볼릭디렉토리",
               "--task-class", "model_serving_strategy", "--generated-utc", "2026-07-25T03:00:00Z",
               "--identity-json", str(identity_path), "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertNotIn("Traceback", proc.stderr)
        self.assertNotEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}")
        self.assertEqual(list(outside.glob("*")), [],
                          msg="a successful init must never write the publication record outside "
                              "the repo through a symlinked docs/_evidence/")
        self.assertTrue((self.repo_root / "docs" / "_evidence").is_symlink(),
                         msg="the planted symlink itself must be left untouched, never "
                             "replaced/unlinked")


class TestPFinal01RecordSaveSymlinkSafety(_PublisherRepoTestCase):
    def test_positive_control_normal_save_and_reload_roundtrip(self):
        code, out, err = init_publication(self.repo_root, "F01정상저장", "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, "F01정상저장", "verification", "verify.txt")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        record = json.loads((self.repo_root / "docs" / "_evidence" / "F01정상저장.json").read_text(encoding="utf-8"))
        self.assertEqual(record["scaffolded"]["verification"], "verify.txt")

    def test_record_basename_symlink_destination_rejected(self):
        init_publication(self.repo_root, "F01레코드심볼릭", "minor_patch")
        record_path = self.repo_root / "docs" / "_evidence" / "F01레코드심볼릭.json"
        record_path.unlink()
        outside = self.repo_root.parent / "outside_record.json"
        outside.write_text('{"planted": "by attacker"}', encoding="utf-8")
        os.symlink(outside, record_path)
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, "F01레코드심볼릭", "verification", "verify.txt")
        self.assertNotEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertNotIn("Traceback", err2)
        self.assertEqual(outside.read_text(encoding="utf-8"), '{"planted": "by attacker"}',
                          msg="a symlinked record destination must never be written through")
        self.assertTrue(record_path.is_symlink(), msg="the planted symlink must be left untouched")

    def test_docs_evidence_dir_symlinked_after_init_blocks_subsequent_save(self):
        """A symlink planted at docs/_evidence AFTER a legitimate init still must not be trusted by
        a later command that needs to update (save) the record."""
        code, out, err = init_publication(self.repo_root, "F01디렉토리사후심볼릭", "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        evidence_dir = self.repo_root / "docs" / "_evidence"
        relocated = self.repo_root.parent / "relocated_evidence"
        evidence_dir.rename(relocated)
        outside = self.repo_root.parent / "outside_evidence_dir2"
        outside.mkdir()
        os.symlink(outside, evidence_dir)
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, "F01디렉토리사후심볼릭", "verification", "verify.txt")
        self.assertNotEqual(code2, 0, msg=f"out={out2} err={err2}")
        self.assertNotIn("Traceback", err2)
        self.assertEqual(list(outside.glob("*")), [],
                          msg="must never write into the outside target through a post-init "
                              "symlinked docs/_evidence/")


class TestPFinal01AppendSidecarSymlinkSafety(_PublisherRepoTestCase):
    def test_positive_control_two_appends_persist_as_two_lines(self):
        init_publication(self.repo_root, "F01사이드카정상", "harness_change", conditions={"actual_trial": False})
        (self.repo_root / "m1.txt").write_text("one\n", encoding="utf-8")
        (self.repo_root / "m2.txt").write_text("two\n", encoding="utf-8")
        append_raw(self.repo_root, "F01사이드카정상", "testlog", "m1.txt")
        code, out, err = append_raw(self.repo_root, "F01사이드카정상", "testlog", "m2.txt")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        sidecar = self.repo_root / "docs" / "_evidence" / "F01사이드카정상.testlog.raw.jsonl"
        lines = sidecar.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 2)
        self.assertEqual(json.loads(lines[1])["seq"], 2)

    def test_sidecar_basename_symlink_destination_rejected(self):
        init_publication(self.repo_root, "F01사이드카심볼릭", "harness_change", conditions={"actual_trial": False})
        sidecar_path = self.repo_root / "docs" / "_evidence" / "F01사이드카심볼릭.testlog.raw.jsonl"
        outside = self.repo_root.parent / "outside_sidecar.jsonl"
        outside.write_text('{"planted": true}\n', encoding="utf-8")
        os.symlink(outside, sidecar_path)
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F01사이드카심볼릭", "testlog", "x.txt")
        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(outside.read_text(encoding="utf-8"), '{"planted": true}\n',
                          msg="a symlinked sidecar destination must never be appended through")
        self.assertTrue(sidecar_path.is_symlink(), msg="the planted symlink must be left untouched")


# ---- P2-FINAL-02: an explicit publication-record load-boundary validator -- required
# keys/types/allowed structures sufficient for every command; malformed top-level/nested records
# (including a bare `{}` or an incomplete-but-valid-JSON object) must return exactly one stable
# JSON exit-2, never a traceback. Existing valid records remain accepted. ----

class TestPFinal02EmptyAndIncompleteRecordStableExit2(_PublisherRepoTestCase):
    def _write_raw_record(self, topic, obj_or_text):
        record_path = self.repo_root / "docs" / "_evidence" / f"{topic}.json"
        record_path.parent.mkdir(parents=True, exist_ok=True)
        text = obj_or_text if isinstance(obj_or_text, str) else json.dumps(obj_or_text)
        record_path.write_text(text, encoding="utf-8")

    def test_exact_reviewer_repro_empty_object_via_finalize(self):
        """Exact reviewer repro: write `{}` to docs/_evidence/<topic>.json, then finalize."""
        self._write_raw_record("F02빈객체", "{}")
        code, out, err = finalize(self.repo_root, "F02빈객체")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertTrue(out.get("reason_codes"), msg=out)
        self.assertFalse(out.get("ok", True))

    def test_empty_object_rejected_via_append_raw_too(self):
        """Single-choke-point regression: the SAME empty record must fail closed through
        append-raw as well, not only finalize -- proves the fix lives at the shared record-load
        point, not a one-off patch inside cmd_finalize alone."""
        self._write_raw_record("F02빈객체append", {})
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F02빈객체append", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_missing_identity_key_rejected(self):
        self._write_raw_record("F02identity누락", {"task_class": "minor_patch"})
        code, out, err = finalize(self.repo_root, "F02identity누락")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_missing_task_class_key_rejected(self):
        self._write_raw_record("F02taskclass누락", {"identity": IDENTITY})
        code, out, err = finalize(self.repo_root, "F02taskclass누락")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_task_class_wrong_type_rejected(self):
        self._write_raw_record("F02taskclass타입", {"task_class": 123, "identity": IDENTITY})
        code, out, err = finalize(self.repo_root, "F02taskclass타입")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_identity_wrong_type_rejected(self):
        self._write_raw_record("F02identity타입", {"task_class": "minor_patch", "identity": "not-an-object"})
        code, out, err = finalize(self.repo_root, "F02identity타입")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_scaffolded_value_wrong_type_rejected(self):
        """'Do not accept unknown dangerous path shapes': scaffolded.* values are re-trusted as
        write/read path targets elsewhere -- a non-string, non-null value there must fail closed,
        not raise an AttributeError the first time some command re-validates it as a path."""
        self._write_raw_record("F02scaffolded타입", {
            "task_class": "minor_patch", "identity": IDENTITY,
            "scaffolded": {"plan": 42},
        })
        code, out, err = finalize(self.repo_root, "F02scaffolded타입")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_narrative_status_value_wrong_type_rejected(self):
        self._write_raw_record("F02내러티브타입", {
            "task_class": "minor_patch", "identity": IDENTITY,
            "narrative_status": {"plan": "authored"},  # must be an object, not a bare string
        })
        code, out, err = finalize(self.repo_root, "F02내러티브타입")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)

    def test_positive_control_existing_valid_record_unaffected(self):
        """Positive control: a properly-shaped record (real `init` output) must NOT trip any of
        the new required-field/path-shape guards."""
        topic = "F02정상객체"
        code, out, err = init_publication(self.repo_root, topic, "minor_patch")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        (self.repo_root / "verify.txt").write_text("ok\n", encoding="utf-8")
        (self.repo_root / "commit.txt").write_text("abc123\n", encoding="utf-8")
        code2, out2, err2 = append_raw(self.repo_root, topic, "verification", "verify.txt")
        self.assertEqual(code2, 0, msg=f"out={out2} err={err2}")
        code3, out3, err3 = append_raw(self.repo_root, topic, "commit", "commit.txt")
        self.assertEqual(code3, 0, msg=f"out={out3} err={err3}")
        pii_paths = manifest_relative_evidence_paths(self.repo_root, topic)
        code4, out4, err4 = finalize(self.repo_root, topic,
                                      pii_scan={"passed": True, "scanned_paths": pii_paths},
                                      runtime=HEALTHY_RUNTIME)
        self.assertEqual(out4["state"], "evidence-complete", msg=f"out={out4} err={err4}")

    def test_init_rejects_non_object_identity_json(self):
        """Write-time guard complementing the load-time validator: `init` itself must never
        persist a record with a non-object identity in the first place."""
        identity_path = self.repo_root / "_bad_identity.json"
        write_json(identity_path, ["not", "an", "object"])
        cmd = [sys.executable, str(PUBLISHER_SCRIPT), "init", "--topic", "F02init방어",
               "--task-class", "minor_patch", "--generated-utc", "2026-07-25T03:10:00Z",
               "--identity-json", str(identity_path), "--repo-root", str(self.repo_root)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(proc.returncode, 0, msg=f"stdout={proc.stdout}")
        self.assertNotIn("Traceback", proc.stderr)
        out = json.loads(proc.stdout)
        self.assertIn("INIT_IDENTITY_NOT_AN_OBJECT", out["reason_codes"])
        self.assertFalse((self.repo_root / "docs" / "_evidence" / "F02init방어.json").exists())


# ---- P2-FINAL-03: append-raw must not trust an existing JSONL sidecar's shape -- under the SAME
# interprocess lock, the full prior ledger is read and validated (every nonempty line a JSON
# object, integer 'seq' contiguous 1..N in file order, trailing newline for a nonempty file) BEFORE
# any append; an invalid ledger fails closed (stable JSON exit 2, no mutation); a valid ledger
# appends N+1 atomically/durably. ----

class TestPFinal03JsonlLedgerIntegrity(_PublisherRepoTestCase):
    def _sidecar_path(self, topic, kind="testlog"):
        return self.repo_root / "docs" / "_evidence" / f"{topic}.{kind}.raw.jsonl"

    def test_exact_reviewer_repro_no_newline_seed_rejected_no_mutation(self):
        """Exact reviewer repro: seed the sidecar with `{"seq":1}` and NO trailing newline, then
        append one valid raw source."""
        init_publication(self.repo_root, "F03개행없음", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03개행없음")
        sidecar.write_text('{"seq":1}', encoding="utf-8")  # no trailing newline
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03개행없음", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(sidecar.read_text(encoding="utf-8"), '{"seq":1}',
                          msg="an invalid ledger must never be mutated/appended to")

    def test_malformed_json_line_rejected_no_mutation(self):
        init_publication(self.repo_root, "F03깨진json", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03깨진json")
        seeded = '{ this is not valid json !!!\n'
        sidecar.write_text(seeded, encoding="utf-8")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03깨진json", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(sidecar.read_text(encoding="utf-8"), seeded)

    def test_entry_not_an_object_rejected(self):
        init_publication(self.repo_root, "F03엔트리비객체", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03엔트리비객체")
        seeded = "[1,2,3]\n"
        sidecar.write_text(seeded, encoding="utf-8")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03엔트리비객체", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(sidecar.read_text(encoding="utf-8"), seeded)

    def test_duplicate_seq_rejected_no_mutation(self):
        init_publication(self.repo_root, "F03중복시퀀스", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03중복시퀀스")
        seeded = (json.dumps({"src": "a.txt", "sha256": "a" * 64, "recorded_utc": "2026-07-25T03:00:00Z", "seq": 1}) + "\n"
                  + json.dumps({"src": "b.txt", "sha256": "b" * 64, "recorded_utc": "2026-07-25T03:00:01Z", "seq": 1}) + "\n")
        sidecar.write_text(seeded, encoding="utf-8")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03중복시퀀스", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(sidecar.read_text(encoding="utf-8"), seeded)

    def test_gap_in_seq_rejected_no_mutation(self):
        init_publication(self.repo_root, "F03시퀀스갭", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03시퀀스갭")
        seeded = (json.dumps({"src": "a.txt", "sha256": "a" * 64, "recorded_utc": "2026-07-25T03:00:00Z", "seq": 1}) + "\n"
                  + json.dumps({"src": "c.txt", "sha256": "c" * 64, "recorded_utc": "2026-07-25T03:00:02Z", "seq": 3}) + "\n")
        sidecar.write_text(seeded, encoding="utf-8")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03시퀀스갭", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(sidecar.read_text(encoding="utf-8"), seeded)

    def test_wrong_seq_type_rejected_no_mutation(self):
        init_publication(self.repo_root, "F03시퀀스타입", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03시퀀스타입")
        seeded = json.dumps({"src": "a.txt", "sha256": "a" * 64, "recorded_utc": "2026-07-25T03:00:00Z", "seq": "1"}) + "\n"
        sidecar.write_text(seeded, encoding="utf-8")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03시퀀스타입", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(sidecar.read_text(encoding="utf-8"), seeded)

    def test_entry_missing_seq_field_rejected(self):
        init_publication(self.repo_root, "F03시퀀스누락", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03시퀀스누락")
        seeded = json.dumps({"src": "a.txt", "sha256": "a" * 64, "recorded_utc": "2026-07-25T03:00:00Z"}) + "\n"
        sidecar.write_text(seeded, encoding="utf-8")
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03시퀀스누락", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertEqual(sidecar.read_text(encoding="utf-8"), seeded)

    def test_invalid_utf8_ledger_rejected_no_mutation(self):
        init_publication(self.repo_root, "F03깨진유니코드", "harness_change", conditions={"actual_trial": False})
        sidecar = self._sidecar_path("F03깨진유니코드")
        seeded = b'{"seq": 1, "src": "a.txt"}\n\xff\xfe not valid utf8\n'
        with open(sidecar, "wb") as f:
            f.write(seeded)
        (self.repo_root / "x.txt").write_text("x\n", encoding="utf-8")
        code, out, err = append_raw(self.repo_root, "F03깨진유니코드", "testlog", "x.txt")
        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        with open(sidecar, "rb") as f:
            self.assertEqual(f.read(), seeded)

    def test_positive_control_valid_existing_ledger_appends_n_plus_1(self):
        init_publication(self.repo_root, "F03정상원장", "harness_change", conditions={"actual_trial": False})
        (self.repo_root / "a.txt").write_text("a\n", encoding="utf-8")
        (self.repo_root / "b.txt").write_text("b\n", encoding="utf-8")
        (self.repo_root / "c.txt").write_text("c\n", encoding="utf-8")
        append_raw(self.repo_root, "F03정상원장", "testlog", "a.txt")
        append_raw(self.repo_root, "F03정상원장", "testlog", "b.txt")
        code, out, err = append_raw(self.repo_root, "F03정상원장", "testlog", "c.txt")
        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        self.assertEqual(out["seq"], 3)
        sidecar = self._sidecar_path("F03정상원장")
        lines = sidecar.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 3)
        self.assertEqual([json.loads(l)["seq"] for l in lines], [1, 2, 3])

    def test_positive_concurrency_still_produces_unique_contiguous_seq_under_new_validation(self):
        """The new full-ledger validation must not break legitimate concurrent appends -- every
        writer validates the CURRENT ledger state under the same flock hold it always did, so a
        valid, growing ledger must always keep validating as valid."""
        init_publication(self.repo_root, "F03동시성", "harness_change", conditions={"actual_trial": False})
        n = 12
        src_files = []
        for i in range(n):
            p = self.repo_root / f"f03_src_{i}.txt"
            p.write_text(f"payload {i}\n", encoding="utf-8")
            src_files.append(f"f03_src_{i}.txt")
        procs = []
        for src in src_files:
            cmd = [sys.executable, str(PUBLISHER_SCRIPT), "append-raw", "--topic", "F03동시성",
                   "--kind", "testlog", "--src", src, "--recorded-utc", "2026-07-25T03:20:00Z",
                   "--repo-root", str(self.repo_root)]
            procs.append(subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True))
        results = [p.communicate(timeout=60) for p in procs]
        returncodes = [p.returncode for p in procs]
        self.assertEqual(returncodes, [0] * n, msg=results)
        sidecar = self._sidecar_path("F03동시성")
        lines = sidecar.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), n, msg="every concurrent append must land as its own line -- none lost")
        seqs = sorted(json.loads(line)["seq"] for line in lines)
        self.assertEqual(seqs, list(range(1, n + 1)))
        in_order_seqs = [json.loads(line)["seq"] for line in lines]
        self.assertEqual(in_order_seqs, list(range(1, n + 1)),
                          msg="file order must also be contiguous, not merely a set match")


# ---- P2-FINAL-04: --author is interpolated into the provenance HTML comment
# (`<!-- provenance: author=... -->`) -- an author containing a newline + '-->' + '<!--' can
# terminate that comment early and forge arbitrary extra "content" right after it, while the
# command still reports success and marks narrative_status authored. Must fail closed (stable
# JSON, exit 2) with NEITHER the scaffold file NOR the persisted record touched. ----

class TestPFinal04AuthorProvenanceSafety(_PublisherRepoTestCase):
    def test_exact_reviewer_repro_author_html_comment_injection_rejected_no_mutation(self):
        """Exact reviewer repro: --author 'alice -->\\nFORGED: yes\\n<!--' must be rejected before
        anything is written -- the scaffold's narrative markers and the persisted record's
        narrative_status must be byte-for-byte identical to their pre-call state."""
        topic = "FINAL04주입"
        code0, out0, _ = init_publication(self.repo_root, topic, "model_serving_strategy")
        plan_path = self.repo_root / out0["scaffolded"]["plan"]
        record_path = self.repo_root / "docs" / "_evidence" / f"{topic}.json"
        before_plan = plan_path.read_bytes()
        before_record = record_path.read_bytes()

        (self.repo_root / "n.md").write_text("legit narrative\n", encoding="utf-8")
        code, out, err = set_narrative(self.repo_root, topic, "plan", "n.md",
                                        author="alice -->\nFORGED: yes\n<!--")

        self.assertEqual(code, 2, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertTrue(out.get("reason_codes"), msg=out)
        self.assertEqual(plan_path.read_bytes(), before_plan,
                          msg="scaffold must not be mutated when --author is rejected")
        self.assertEqual(record_path.read_bytes(), before_record,
                          msg="persisted record (narrative_status) must not be mutated when --author is rejected")

    def test_positive_normal_author_stays_authored(self):
        """Control: an ordinary, printable --author (including a space) must be completely
        unaffected by the new rejection -- it still authors successfully and is recorded verbatim."""
        topic = "FINAL04정상저자"
        code0, out0, _ = init_publication(self.repo_root, topic, "model_serving_strategy")
        plan_rel = out0["scaffolded"]["plan"]
        (self.repo_root / "n.md").write_text("legit narrative\n", encoding="utf-8")

        code, out, err = set_narrative(self.repo_root, topic, "plan", "n.md", author="Alice Kim")

        self.assertEqual(code, 0, msg=f"out={out} err={err}")
        content = (self.repo_root / plan_rel).read_text(encoding="utf-8")
        self.assertIn("author=Alice Kim", content)
        record = json.loads((self.repo_root / "docs" / "_evidence" / f"{topic}.json").read_text(encoding="utf-8"))
        self.assertEqual(record["narrative_status"]["plan"]["status"], "authored")
        self.assertEqual(record["narrative_status"]["plan"]["author"], "Alice Kim")


# ---- P2-FINAL-05: a rejected PASS->FAIL/REFUTE transition must be all-or-nothing -- the new
# bench_report is currently written to disk BEFORE the prior-certificate cleanup is validated, so
# a tampered/escaping certificate pointer rejects the command while leaving a NEW report on disk
# next to a persisted record that still says PASS. Must fail closed (stable JSON, exit 2) with the
# bench_report file and the persisted record BOTH byte-for-byte unchanged. ----

class TestPFinal05TransactionalCertificateCleanup(_PublisherRepoTestCase):
    def test_exact_reviewer_repro_tampered_certificate_escape_leaves_report_and_record_unchanged(self):
        """Exact reviewer repro: publish a valid PASS, tamper the persisted record's
        scaffolded.certificate to '../outside.yaml', then publish FAIL with a new report. The
        command must reject (exit != 0) and the OLD PASS bench_report bytes, the persisted record
        bytes, and the out-of-bounds file must ALL remain exactly as they were -- no orphan new
        report, no report/record contradiction, no unlink through the escaping path."""
        topic = "FINAL05변조탈출"
        init_publication(self.repo_root, topic, "full_benchmark", benchmark_mode="full", benchmark_verdict="PASS")
        (self.repo_root / "r1.md").write_text("passing report\n", encoding="utf-8")
        (self.repo_root / "cert.yaml").write_text(VALID_FLAT_CERTIFICATE, encoding="utf-8")
        code0, out0, err0 = publish_benchmark(self.repo_root, topic, "PASS", "r1.md", certificate_src="cert.yaml")
        self.assertEqual(code0, 0, msg=f"out={out0} err={err0}")
        report_path = self.repo_root / out0["bench_report_path"]
        record_path = self.repo_root / "docs" / "_evidence" / f"{topic}.json"

        outside = self.repo_root.parent / "outside.yaml"
        outside.write_text("not a real certificate location\n", encoding="utf-8")
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["scaffolded"]["certificate"] = "../outside.yaml"
        record_path.write_text(json.dumps(record), encoding="utf-8")

        before_report = report_path.read_bytes()
        before_record = record_path.read_bytes()

        (self.repo_root / "r2.md").write_text("it regressed\n", encoding="utf-8")
        code, out, err = publish_benchmark(self.repo_root, topic, "FAIL", "r2.md")

        self.assertNotEqual(code, 0, msg=f"out={out} err={err}")
        self.assertNotIn("Traceback", err)
        self.assertTrue(outside.exists(), msg="a tampered/out-of-bounds certificate path must never be unlinked")
        self.assertEqual(report_path.read_bytes(), before_report,
                          msg="the OLD PASS bench_report must not be overwritten before the rejected "
                              "certificate cleanup -- report and record must reject together, not separately")
        self.assertEqual(record_path.read_bytes(), before_record,
                          msg="the persisted record (still PASS) must not change when the transition is rejected")


class TestPFinalMalformedJsonFileArguments(_PublisherRepoTestCase):
    """Final acceptance regressions for JSON-file argument boundary handling."""

    def _run_init(self, *, identity_json, conditions_json=None):
        cmd = [
            sys.executable, str(PUBLISHER_SCRIPT), "init",
            "--topic", "FINAL입력경계",
            "--task-class", "harness_change",
            "--generated-utc", "2026-07-25T04:00:00Z",
            "--identity-json", identity_json,
            "--repo-root", str(self.repo_root),
        ]
        if conditions_json is not None:
            cmd.extend(["--conditions-json", conditions_json])
        return subprocess.run(cmd, cwd=self.repo_root, capture_output=True, text=True, timeout=30)

    def _assert_stable_exit2(self, proc):
        self.assertEqual(proc.returncode, 2, msg=f"stdout={proc.stdout} stderr={proc.stderr}")
        self.assertNotIn("Traceback", proc.stderr)
        out = json.loads(proc.stdout)
        self.assertFalse(out.get("ok", True))
        self.assertTrue(out.get("reason_codes"))

    def test_conditions_json_non_object_array_stable_exit2(self):
        write_json(self.repo_root / "identity.json", IDENTITY)
        write_json(self.repo_root / "conditions.json", [1])
        self._assert_stable_exit2(self._run_init(
            identity_json="identity.json", conditions_json="conditions.json"))

    def test_identity_json_directory_stable_exit2(self):
        self._assert_stable_exit2(self._run_init(identity_json="docs"))


if __name__ == "__main__":
    unittest.main()
