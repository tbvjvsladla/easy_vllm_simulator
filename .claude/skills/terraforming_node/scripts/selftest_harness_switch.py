#!/usr/bin/env python3
"""Offline integration checks for provider handoff and durable relay ownership.

Uses real request builders, transport validation, ledger writers and subprocess locks.
Only the external agent invocation is replaced; no model, SSH or GPU is contacted.
All generated state lives in isolated temporary directories.
"""
from __future__ import annotations

import contextlib
import copy
import hashlib
import io
import json
import multiprocessing
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest import mock

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import agent_control
import relay
import turn_budget


MANIFEST = '''topology: multi
cpu_arch: aarch64
gpus_per_node: 1
nodes:
  - role: main
    host: 192.0.2.10
    ssh_user: tester
    work_dir: /srv/main
  - role: sub
    host: 192.0.2.11
    ssh_user: tester
    work_dir: /srv/sub
'''


def _pending_worker(root, context, start):
    """Widen the actual read/modify/write window, inside the production lock."""
    original = relay._atomic_json

    def delayed_write(path, document):
        time.sleep(0.03)
        original(path, document)

    relay._atomic_json = delayed_write
    if not start.wait(10):
        raise RuntimeError("parent did not release concurrent writers")
    for attempt in range(1, 5):
        relay.surface_requests(root, context, {"hitl": {
            "needed": True, "request_id": context + "-question",
            "prompt": "preserve this context", "blocking": True}}, attempt=attempt)


class HarnessSwitchChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="harness-switch-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / "manifest.yaml"
        self.manifest.write_text(MANIFEST, encoding="utf-8")
        self.native = turn_budget.declare(12, 90, source="selftest explicit declaration")
        self.wall = turn_budget.declare(None, 90, source="selftest explicit declaration",
                                        unit="wall_time", max_dispatches=3)
        self.claude = {"name": "sonnet", "provider": "claude_code",
                       "backend": "anthropic", "model": "sonnet"}
        self.codex = {"name": "codex-gpt-6.1-sol", "provider": "codex_cli",
                      "backend": "openai", "model": "gpt-6.1-sol"}

    def request(self, provider="claude_code", **changes):
        runner = self.codex if provider == "codex_cli" else self.claude
        return relay.build_request("multi", str(self.manifest), "inspect provided evidence",
                                   self.wall if provider == "codex_cli" else self.native,
                                   context_id="switch", attempt=1, provider=provider,
                                   model=runner["model"], backend=runner["backend"], **changes)

    def assertRejected(self, function, *args, message=None, **kwargs):
        with self.assertRaises(SystemExit) as caught:
            function(*args, **kwargs)
        if message:
            self.assertIn(message, str(caught.exception))

    def test_codex_reasoning_effort_contract_and_argv(self):
        adapter = agent_control._load_provider("codex_cli")
        request = self.request("codex_cli", reasoning_effort="low")
        self.assertEqual(agent_control.request_schema_violations(request), [])
        argv = adapter._inner_argv(request)
        self.assertEqual(argv[argv.index("--model") + 1], "gpt-6.1-sol")
        self.assertIn('model_reasoning_effort="low"', argv)
        resumed = dict(request, resume_session_id="native-id")
        self.assertEqual(adapter._inner_argv(resumed),
                         argv[:4] + ["resume"] + argv[4:argv.index("--sandbox")] +
                         ["-c", 'sandbox_mode="read-only"', "--", "native-id", request["task"]])
        old = self.request("codex_cli")
        self.assertNotIn("reasoning_effort", old)
        self.assertFalse(any("model_reasoning_effort" in item for item in adapter._inner_argv(old)))
        self.assertTrue(agent_control.request_schema_violations(dict(request, reasoning_effort="invalid")))
        self.assertTrue(agent_control.request_schema_violations(dict(self.request(), reasoning_effort="low")))

    def test_canary_cli_forwards_effort(self):
        command = [sys.executable, str(HERE / "bootstrap_canary.py"), "--topology", "multi",
                   "--manifest", str(self.manifest), "--provider", "codex_cli", "--backend", "openai",
                   "--model", "gpt-6.1-terra", "--reasoning-effort", "low", "--timeout-seconds", "90",
                   "--max-dispatches", "3", "--budget-source", "offline fixture"]
        result = subprocess.run(command, capture_output=True, text=True, check=True)
        request = json.loads(result.stdout)
        self.assertEqual((request["model"], request["reasoning_effort"]), ("gpt-6.1-terra", "low"))
        self.assertEqual(agent_control.request_schema_violations(request), [])

    def test_relay_rejects_ambiguous_task_arguments(self):
        for args in (("--task", "one", "--task-file", "two"), ("--continue", "--task", "one")):
            with mock.patch.object(sys, "argv", ["relay.py", *args]), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    relay.main()

    def test_continue_preview_reports_selected_provider_budget_without_writes(self):
        path = relay.ledger_path(str(self.root), "switch")
        doc = relay.load_ledger(path)
        result = {"status": "completed", "provider": "codex_cli", "session_id": "native",
                  "execution_started": True, "reason_codes": [], "budget_outcome": "within_budget"}
        relay.record_attempt(doc, context_id="switch", bud=turn_budget.declare(None, 3600, source="fixture", unit="wall_time"),
                             result=result, runner=self.codex,
                             report={"status": "input-required", "context_id": "switch", "running_services": []},
                             started_utc="2026-10-28T21:23:51Z", ended_utc="2026-10-28T22:14:00Z")
        doc.update(topology="multi", manifest=str(self.manifest), runner_ladder=self.codex["name"])
        relay.save_ledger(path, doc)
        task = self.root / "preview-task.txt"
        task.write_text("complete task\n", encoding="utf-8")
        before = Path(path).read_bytes()
        argv = [str(HERE / "relay.py"), "--repo-root", str(self.root), "--context-id", "switch", "--no-campaign",
                "--continue", "--task-file", str(task), "--timeout-seconds", "1800"]
        out = io.StringIO()
        with mock.patch.object(sys, "argv", argv), contextlib.redirect_stdout(out):
            self.assertEqual(relay.main(), 0)
        preview = out.getvalue()
        self.assertIn("provider=codex_cli", preview)
        self.assertIn("직전 wall 배정 3600초", preview)
        self.assertIn("관측 경과 3009초(main 시작/종료 시각)", preview)
        self.assertIn("다음 wall 배정 1800초(명시)", preview)
        self.assertNotIn("max_turns", preview)
        self.assertNotIn("--max-turns", preview)
        self.assertNotIn("전송 상한 100", preview)
        self.assertEqual(Path(path).read_bytes(), before)
        self.assertFalse((Path(relay.relay_root(str(self.root))) / "switch.requests").exists())
        request = relay.build_request("multi", str(self.manifest), "task",
            turn_budget.declare(None, 1800, source="fixture", unit="wall_time"),
            provider="codex_cli", backend="openai", model=self.codex["model"], sandbox_mode="read-only")
        self.assertEqual((request["budget_unit"], request["max_turns"], request["timeout_seconds"]),
                         ("wall_time", None, 1800))
        unknown, _ = relay.continuation_budget_preview(doc, [self.codex], {}, None)
        self.assertIn("관측 경과 미관측", unknown)
        self.assertIn("다음 wall 배정 미선언", unknown)
        measured, _ = relay.continuation_budget_preview(doc, [self.codex], {"duration_ms": 1250}, None)
        self.assertIn("1.25초(provider duration_ms)", measured)
        claude_doc = {"attempts": [{"max_turns_allocated": 24, "execution_started": True}]}
        claude, flags = relay.continuation_budget_preview(claude_doc, [self.claude], None, None)
        self.assertIn("native turn", claude)
        self.assertIn("바닥 24", claude)
        self.assertIn("스키마 필드 상한 100", claude)
        self.assertIn("--max-turns", flags)
        with mock.patch.object(relay, "next_runner_index", return_value=1):
            mixed, flags = relay.continuation_budget_preview(doc, [self.claude, self.codex], doc["attempts"][-1], None)
        self.assertIn("provider=codex_cli", mixed)
        self.assertIn("--max-turns", flags)
        self.assertIn("Claude fallback", flags)
        mixed_argv = argv + ["--runners", "codex-gpt-6.1-sol,sonnet", "--max-turns", "24"]
        mixed_out = io.StringIO()
        with mock.patch.object(sys, "argv", mixed_argv), contextlib.redirect_stdout(mixed_out):
            self.assertEqual(relay.main(), 0)
        self.assertIn("provider=codex_cli", mixed_out.getvalue())
        self.assertIn("Claude fallback용 --max-turns", mixed_out.getvalue())
        emit = mixed_argv + ["--apply", "--emit-only", "--resume", "new", "--budget-source", "fixture",
                             "--sandbox-mode", "read-only"]
        emitted = io.StringIO()
        with mock.patch.object(sys, "argv", emit), contextlib.redirect_stdout(emitted):
            self.assertEqual(relay.main(), 0)
        blob = emitted.getvalue()
        mixed_request = json.loads(blob[blob.index("{\n"):])
        self.assertEqual((mixed_request["provider"], mixed_request["max_turns"], mixed_request["timeout_seconds"]),
                         ("codex_cli", None, 1800))
        with mock.patch.object(sys, "argv", [x for x in emit if x not in ("--max-turns", "24")]), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertRejected(relay.main)
        self.assertEqual(Path(path).read_bytes(), before)

    def test_v1_builder_remains_valid_and_uses_native_budget(self):
        request = self.request()
        self.assertEqual(agent_control.request_schema_violations(request), [])
        self.assertEqual(request["schema_version"], 1)
        self.assertEqual(request["max_turns"], 12)
        self.assertEqual(request["timeout_seconds"], 90)
        self.assertNotIn("budget_unit", request)
        self.assertNotIn("sandbox_mode", request)
        argv = agent_control._load_provider("claude_code").build_argv(request)
        self.assertEqual(argv[0], "ssh")
        self.assertIn("--max-turns", argv[-1])

    def test_v2_builder_and_real_transport_validator(self):
        request = self.request("codex_cli", permission_profile="vllm-sub")
        self.assertEqual(agent_control.request_schema_violations(request), [])
        self.assertEqual(request["schema_version"], 2)
        self.assertIsNone(request["max_turns"])
        self.assertEqual((request["budget_unit"], request["timeout_seconds"]), ("wall_time", 90))
        self.assertNotIn("max_dispatches", request)
        self.assertIn("AGENTS.md", request["task"])
        for changes in ({"schema_version": 1}, {"provider": "claude_code"},
                        {"max_turns": 12}, {"max_dispatches": 0},
                        {"max_dispatches": True}, {"timeout_seconds": None},
                        {"permission_profile": "vllm-main"},
                        {"sandbox_mode": "workspace-write"}):
            with self.subTest(changes=changes):
                self.assertTrue(agent_control.request_schema_violations(dict(request, **changes)))
        missing = dict(request)
        missing["max_dispatches"] = 3  # Historical request remains readable.
        self.assertEqual(agent_control.request_schema_violations(missing), [])
        self.assertRejected(turn_budget.declare, 1, 90, source="declared", unit="wall_time")
        self.assertNotIn("max_dispatches", turn_budget.declare(None, 90, source="declared", unit="wall_time"))

    def test_real_runner_registry_preserves_default_and_provider_identity(self):
        default = relay.resolve_ladder(str(HERE.parents[3]), None)
        self.assertEqual([(r["name"], r["provider"]) for r in default],
                         [("sonnet", "claude_code")])
        explicit = relay.resolve_ladder(str(HERE.parents[3]), "sonnet,codex-gpt-6.1-sol")
        self.assertEqual([r["provider"] for r in explicit], ["claude_code", "codex_cli"])
        chosen = relay.resolve_ladder(str(HERE.parents[3]), None, provider="codex_cli",
                                      backend="openai", model="gpt-6.1-sol")
        self.assertEqual(chosen, [self.codex])

    def test_native_resume_never_crosses_provider_or_permission_boundary(self):
        claude_request = self.request()
        codex_request = self.request("codex_cli", sandbox_mode="read-only")
        doc = {"attempts": [{"session_id": "native-claude", "runner": self.claude,
                             "session_binding": relay.permission_binding(claude_request)}]}
        self.assertEqual(relay.bound_resume(doc, "native-claude", codex_request), ("new", None))
        binding = relay.permission_binding(codex_request)
        doc["attempts"].append({"session_id": "native-codex", "runner": self.codex,
                                "session_binding": binding})
        self.assertEqual(relay.bound_resume(doc, "native-codex", codex_request),
                         ("native-codex", binding))
        resumed = dict(codex_request, resume_session_id="native-codex", resume_binding=binding)
        self.assertEqual(agent_control.request_schema_violations(resumed), [])
        self.assertEqual(relay.bound_resume(doc, "native-codex", claude_request), ("new", None))
        for key, value in (("sandbox_mode", "workspace-write"),
                           ("target", dict(codex_request["target"], work_dir="/srv/other"))):
            changed = dict(codex_request, **{key: value})
            self.assertRejected(relay.bound_resume, doc, "native-codex", changed,
                                message="binding changed")
            self.assertTrue(agent_control.request_schema_violations(dict(
                changed, resume_session_id="native-codex", resume_binding=binding)))
        self.assertRejected(relay.bound_resume, doc, "unrecorded", codex_request)
        self.assertEqual(relay.bound_resume({"attempts": []}, "legacy", claude_request),
                         ("legacy", None))

    def test_rejected_rotation_is_not_reopened_by_supervisor(self):
        doc = {"attempts": []}
        calls = []

        def unavailable(a, state, lp, task, budget, resume, runner):
            calls.append(runner["name"])
            relay.record_attempt(state, context_id="switch", bud=budget, runner=runner,
                                 result={"status": "execution_failed", "reason_codes": [
                                     relay.RUNNER_UNAVAILABLE], "execution_started": False,
                                     "failure_evidence": {"rotate": False, "signal": "unknown"}})
            return 5

        args = SimpleNamespace(ladder=[self.claude, self.codex], emit_only=False)
        with mock.patch.object(relay, "run_attempt_once", unavailable), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(relay.run_attempt(args, doc, "unused", "task", self.native, "new"), 5)
        self.assertEqual(calls, ["sonnet"])
        self.assertEqual(relay.supervise_decide(doc, ladder=args.ladder)["action"], "popup")
        doc["attempts"][-1]["runner_evidence"]["rotate"] = True
        self.assertEqual(relay.supervise_decide(doc, ladder=args.ladder)["action"], "rotate_runner")

    def test_late_execution_consumes_cost_and_cannot_rotate(self):
        doc = {"attempts": []}
        attempt = relay.record_attempt(doc, context_id="switch", bud=self.wall, runner=self.codex,
                                       result={"status": "execution_failed", "reason_codes": [
                                           relay.RUNNER_UNAVAILABLE], "execution_started": True,
                                           "raw_exit_code": 1,
                                           "failure_evidence": {"rotate": True}})
        self.assertTrue(relay._reached_sub(attempt))
        decision = relay.supervise_decide(doc, cost_cap_attempts=1,
                                          ladder=[self.codex, self.claude])
        self.assertEqual(decision["action"], "popup")
        self.assertNotIn("1/1", decision["reason"])
        self.assertEqual(relay.supervise_decide(doc, ladder=[self.codex, self.claude])["action"], "popup")

    def test_stale_ledger_cannot_erase_new_attempt(self):
        path = str(self.root / "state.json")
        initial = relay.load_ledger(path)
        relay.save_ledger(path, initial)
        stale = relay.load_ledger(path)
        winner = relay.load_ledger(path)
        winner["attempts"].append({"attempt": 1, "execution_started": True})
        relay.save_ledger(path, winner)
        stale["attempts"].append({"attempt": 99})
        self.assertRejected(relay.save_ledger, path, stale, message="stale ledger")
        self.assertEqual(relay.load_ledger(path), winner)

    def dispatch_args(self):
        return SimpleNamespace(repo_root=str(self.root), context_id="switch", topology="multi",
                               manifest_path=str(self.manifest), emit_only=False, campaign_id=None,
                               control_variables=None, campaign_control=None, node=None,
                               permission_profile="vllm-sub", sandbox_mode=None,
                               continue_=False, apply=False, resume=None)

    def test_codex_failure_recovery_is_durable_and_never_replays_on_progress(self):
        provider = agent_control._load_provider("codex_cli")
        started = [{"type": "thread.started", "thread_id": "interrupted-native"},
                   {"type": "turn.started"}]
        completed_report = started + [
            {"type": "error", "code": "permission_denied"},
            {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps({
                "status": "completed", "context_id": "switch", "phase": "inspect",
                "running_services": [], "artifacts": []})}},
            {"type": "turn.completed", "usage": {}}]
        cases = [
            ("adapter_signal", started, -15, {"cancelled": True, "cancellation_signal": signal.SIGTERM}),
            ("adapter_signal_with124", started, 124, {"cancelled": True, "cancellation_signal": signal.SIGINT}),
            ("completed_report_then_signal", completed_report, 124,
             {"cancelled": True, "cancellation_signal": signal.SIGINT}),
            ("completed_report_then_child_signal", completed_report, -15, {}),
            ("child_signal", started, -2, {}),
            ("shell130", started, 130, {}),
            ("native_failure", started + [{"type": "turn.failed", "error": {"message": "opaque"}}], 1, {}),
            ("process_failure", started, 1, {}),
            ("unknown_start", [], 1, {}),
            ("approval_required", started + [{"type": "turn.failed", "error": {"code": "approval_required"}}], 1, {}),
            ("permission_denied", started + [{"type": "turn.failed", "error": {"code": "permission_denied"}}], 1, {}),
        ]
        real_run = subprocess.run
        for name, events, rc, options in cases:
            with self.subTest(name=name):
                root = self.root / name
                root.mkdir()
                args = self.dispatch_args()
                args.repo_root = str(root)
                args.ladder = [self.codex, self.claude]
                path = relay.ledger_path(str(root), "switch")
                state = relay.load_ledger(path)
                calls = []

                def invoke(command, **kwargs):
                    if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                        return real_run(command, **kwargs)
                    request = json.loads(Path(command[command.index("--request") + 1]).read_text())
                    calls.append(request)
                    if len(calls) == 1:
                        raw = "".join(json.dumps(e) + "\n" for e in events)
                        result = provider.parse_output(request, raw, "isolated failure diagnostic", rc, **options)
                    else:
                        report = {"status": "completed", "context_id": "switch", "phase": "inspect",
                                  "running_services": [], "artifacts": []}
                        raw = "".join(json.dumps(e) + "\n" for e in started + [
                            {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(report)}},
                            {"type": "turn.completed", "usage": {}}])
                        result = provider.parse_output(request, raw, "", 0)
                    agent_control.persist_outcome(command[command.index("--result-file") + 1],
                        dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
                    return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=result["exit_code"])

                with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
                     mock.patch.object(relay.subprocess, "run", side_effect=invoke), \
                     contextlib.redirect_stdout(io.StringIO()):
                    relay.run_attempt_once(args, state, path, "preserved task", self.wall, "new", self.codex)
                    saved = relay.load_ledger(path)
                    attempt = saved["attempts"][0]
                    self.assertIn("recovery_required", attempt)
                    self.assertEqual(attempt["raw_exit_code"], rc)
                    self.assertIs(attempt["failure_evidence"]["rotate"], False)
                    self.assertEqual(relay.supervise_decide(saved, doc_progress="observed context document")["action"], "popup")
                    pending = relay.pending_for(str(root), "switch")
                    self.assertEqual(len(relay.resume_blockers(pending)), 1)
                    self.assertEqual(pending[0]["recovery_required"]["dispatch_id"], attempt["dispatch_id"])
                    outcome = json.loads((root / attempt["outcome_path"]).read_text())
                    self.assertEqual(outcome["result"]["failure_evidence"]["raw_stderr"], "isolated failure diagnostic")
                    for key, value in attempt["failure_evidence"].items():
                        self.assertEqual(outcome["result"]["failure_evidence"][key], value)
                    control = json.loads((root / attempt["report_path"]).read_text())["control"]
                    self.assertEqual(control["failure_evidence"], outcome["result"]["failure_evidence"])
                    if name == "adapter_signal":
                        self.assertEqual(attempt["failure_evidence"]["termination"]["adapter_received_signal"], signal.SIGTERM)
                        self.assertNotIn("user_cancelled", json.dumps(attempt))
                    if name == "shell130":
                        self.assertEqual(attempt["failure_evidence"]["termination"], {
                            "adapter_received_signal": None, "child_signal": 2, "child_signal_source": "shell_exit_status"})
                    if name in ("adapter_signal_with124", "completed_report_then_signal", "completed_report_then_child_signal"):
                        self.assertEqual(attempt["end_reason"], "process_interrupted")
                        self.assertEqual(attempt["failure_evidence"]["signal"], "process_signal")
                    if name.startswith("completed_report_then_"):
                        self.assertEqual(attempt["status"], "completed")
                        self.assertIn("PERMISSION_DENIED", attempt["reason_codes"])
                    # All dispatch entries stop before building a request or touching a runner.
                    with mock.patch.object(relay, "build_request", side_effect=AssertionError("duplicate execution")):
                        self.assertRejected(relay.run_attempt, args, saved, path, "task", self.wall, "new",
                                            message="unanswered blocking")
                        self.assertEqual(relay._supervise_resume(args, path, saved, 90, "progress"), 3)
                    self.assertEqual(len(calls), 1)
                    # The ledger repairs a missing pending file after a producer crash.
                    pending_path = Path(relay.relay_root(str(root))) / relay.PENDING_HITL
                    pending_path.unlink()
                    relay.ensure_recovery_pending(str(root), relay.load_ledger(path))
                    self.assertEqual(len(relay.resume_blockers(relay.pending_for(str(root), "switch"))), 1)
                    task_file = root / "recovery-task.txt"
                    task_body = "Updated complete instruction: Terra low.\nRemaining work only.\n"
                    task_file.write_text(task_body, encoding="utf-8")
                    blocked_command = [str(HERE / "relay.py"), "--repo-root", str(root), "--context-id", "switch",
                                       "--continue", "--apply", "--task-file", str(task_file), "--resume", "new", "--no-campaign"]
                    with mock.patch.object(sys, "argv", blocked_command):
                        self.assertRejected(relay.main, message="차단성")
                    document = json.loads(pending_path.read_text())
                    entry = document["pending"][0]
                    entry["answer"] = "Human reviewed observed effects; explicitly resume this task."
                    # A stale dispatch answer cannot authorize the current dispatch.
                    entry["recovery_required"]["dispatch_id"] = "older-dispatch"
                    relay._atomic_json(str(pending_path), document)
                    relay.ensure_recovery_pending(str(root), saved)
                    self.assertIsNone(relay.pending_for(str(root), "switch")[0]["answer"])
                    document = json.loads(pending_path.read_text())
                    document["pending"][0]["answer"] = "Human reviewed effects; explicitly resume."
                    relay._atomic_json(str(pending_path), document)
                    with mock.patch.object(relay, "build_request", side_effect=AssertionError("automatic replay after answer")):
                        self.assertRejected(relay.run_attempt_once, args, saved, path, "task", self.wall,
                                            "new", self.codex, message="explicit --continue")
                    self.assertEqual(relay.supervise_decide(saved, doc_progress="still progressing")["action"], "popup")
                    # The official manual path consumes one new dispatch, with all normal guards.
                    command = [str(HERE / "relay.py"), "--repo-root", str(root), "--context-id", "switch",
                               "--continue", "--apply", "--task-file", str(task_file), "--resume", "new", "--no-campaign",
                               "--timeout-seconds", "90", "--budget-source", "explicit human recovery fixture"]
                    with mock.patch.object(sys, "argv", command):
                        self.assertEqual(relay.main(), 0)
                    final = relay.load_ledger(path)
                    self.assertEqual(len(calls), 2)
                    self.assertEqual(len(final["attempts"]), 2)
                    self.assertEqual(final["attempts"][0], attempt)
                    self.assertNotEqual(final["attempts"][0]["dispatch_id"], final["attempts"][1]["dispatch_id"])
                    instruction = final["attempts"][1]["task_instruction"]
                    self.assertEqual(instruction["kind"], "explicit")
                    self.assertEqual(instruction["body"], task_body)
                    self.assertEqual(instruction["body_digest"], hashlib.sha256(task_body.encode()).hexdigest())
                    self.assertNotIn("max_dispatches", final["attempts"][1])
                    self.assertEqual(relay.pending_for(str(root), "switch"), [])

    def test_capacity_is_strict_structured_failure_and_retains_recovery_gate(self):
        adapter = agent_control._load_provider("codex_cli")
        request = self.request("codex_cli")
        msg = "Selected model is at capacity. Please try a different model."
        base = [{"type": "thread.started", "thread_id": "session"}, {"type": "turn.started"}]
        failures = [{"type": "error", "message": msg}, {"type": "turn.failed", "error": {"message": msg}}]
        def parse(events, rc=1, **kw):
            return adapter.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", rc, **kw)
        result = parse(base + failures)
        self.assertEqual(result["failure_evidence"]["signal"], "provider_capacity")
        self.assertNotEqual(result["status"], "completed")
        att = relay.record_attempt({"attempts": []}, context_id="switch", bud=self.wall, result=result, runner=self.codex)
        self.assertIsNotNone(relay.codex_recovery_requirement(att))
        for events, rc, kwargs in ((base + [{"type": "item.completed", "item": {"type": "agent_message", "text": msg}}], 1, {}),
                                   (base + failures + [{"type": "error", "message": "other"}], 1, {}),
                                   (base + failures, 143, {}), (base + failures, 124, {}),
                                   (base + failures, 1, {"cancelled": True}),
                                   (base + failures + [{"type": "error", "code": "permission_denied"}], 1, {})):
            self.assertNotEqual(parse(events, rc, **kwargs)["failure_evidence"]["signal"], "provider_capacity")
        malformed = adapter.parse_output(request, json.dumps(failures[0]) + "\n{bad", "", 1)
        self.assertNotEqual(malformed["failure_evidence"]["signal"], "provider_capacity")

    def test_capacity_batch_counts_and_durable_recovery(self):
        args = self.dispatch_args()
        args.ladder = [self.codex]
        args.reasoning_effort = "low"
        args.capacity_retries = 2
        args.capacity_backoff_seconds = 1
        args.capacity_deadline_utc = "2099-01-01T00:00:00Z"
        approval = self.root / "approval.json"
        approval.write_text(json.dumps({"authorization": "actual user authorization", "source": "fixture message"}))
        args.capacity_approval_file = str(approval)
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        provider = agent_control._load_provider("codex_cli")
        real_run = subprocess.run
        calls = []
        def invoke(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            request = json.loads(Path(command[command.index("--request") + 1]).read_text())
            calls.append(request)
            events = [{"type": "thread.started", "thread_id": "capacity-session"}, {"type": "turn.started"},
                      {"type": "turn.failed", "error": {"message": "Selected model is at capacity. Please try a different model."}}]
            result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 1)
            agent_control.persist_outcome(command[command.index("--result-file") + 1],
                dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
            return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=1)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=invoke), \
             mock.patch.object(relay.time, "sleep"), contextlib.redirect_stdout(io.StringIO()):
            relay.run_attempt(args, state, path, "full task", self.wall, "new")
        saved = relay.load_ledger(path)
        self.assertEqual(len(calls), 3)
        self.assertEqual(len({x["dispatch_id"] for x in saved["attempts"]}), 3)
        self.assertEqual(saved["capacity_bursts"][0]["allocated_slots"], 3)
        self.assertEqual([r.get("resume_session_id") for r in calls], [None, "capacity-session", "capacity-session"])
        self.assertTrue(all(r["model"] == "gpt-6.1-sol" and r["reasoning_effort"] == "low" for r in calls))
        self.assertEqual(saved["attempts"][1]["task_instruction"]["kind"], "continuation")
        self.assertIn("벤치", calls[1]["task"])
        self.assertEqual(len(relay.resume_blockers(relay.pending_for(str(self.root), "switch"))), 0)
        self.assertEqual(saved["capacity_deferred"]["dispatch_id"], saved["attempts"][-1]["dispatch_id"])
        self.assertEqual(relay.supervise_decide(saved)["action"], "resume")
        wake = SimpleNamespace(repo_root=str(self.root), topology=None, manifest=None, context_id="switch",
                               ladder=[self.codex], max_turns=None, permission_profile=None, sandbox_mode=None,
                               reasoning_effort=None, apply=True, emit_only=False)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=invoke), \
             mock.patch.object(relay.time, "sleep"), contextlib.redirect_stdout(io.StringIO()):
            relay._supervise_resume(wake, path, saved, 90, "deferred wake")
        again = relay.load_ledger(path)
        self.assertEqual(len(calls), 6)
        self.assertEqual(len(again["capacity_bursts"]), 2)
        self.assertEqual(len({x["dispatch_id"] for x in again["attempts"]}), 6)
        self.assertEqual(again["capacity_bursts"][0], saved["capacity_bursts"][0])
        self.assertTrue(all(r.get("resume_session_id") == "capacity-session" for r in calls[1:]))
        self.assertEqual(relay.supervise_decide(again)["action"], "resume")

    def test_capacity_batch_two_failures_then_completed(self):
        args = self.dispatch_args()
        args.ladder = [self.codex]
        args.reasoning_effort = "low"
        args.capacity_retries = 2
        args.capacity_backoff_seconds = 1
        args.capacity_deadline_utc = "2099-01-01T00:00:00Z"
        approval = self.root / "approval.json"
        approval.write_text(json.dumps({"authorization": "actual user authorization", "source": "fixture message"}))
        args.capacity_approval_file = str(approval)
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        provider = agent_control._load_provider("codex_cli")
        real_run = subprocess.run
        calls = []
        def invoke(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            request = json.loads(Path(command[command.index("--request") + 1]).read_text())
            calls.append(request)
            events = [{"type": "thread.started", "thread_id": "capacity-session"}, {"type": "turn.started"},
                      {"type": "turn.failed", "error": {"message": "Selected model is at capacity. Please try a different model."}}]
            if len(calls) == 3:
                events = [{"type": "thread.started", "thread_id": "capacity-session"}, {"type": "turn.started"},
                          {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(
                              {"status": "completed", "context_id": "switch", "phase": "bench", "running_services": [], "artifacts": []})}},
                          {"type": "turn.completed", "usage": {}}]
            result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 0 if len(calls) == 3 else 1)
            agent_control.persist_outcome(command[command.index("--result-file") + 1],
                dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
            return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=1)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=invoke), \
             mock.patch.object(relay.time, "sleep"), contextlib.redirect_stdout(io.StringIO()):
            relay.run_attempt(args, state, path, "full task", self.wall, "new")
        saved = relay.load_ledger(path)
        self.assertEqual(len(calls), 3)
        self.assertEqual(len({x["dispatch_id"] for x in saved["attempts"]}), 3)
        self.assertEqual(saved["capacity_bursts"][0]["allocated_slots"], 3)
        self.assertEqual([r.get("resume_session_id") for r in calls], [None, "capacity-session", "capacity-session"])
        self.assertTrue(all(r["model"] == "gpt-6.1-sol" and r["reasoning_effort"] == "low" for r in calls))
        self.assertEqual(saved["attempts"][1]["task_instruction"]["kind"], "continuation")
        self.assertIn("벤치", calls[1]["task"])
        self.assertEqual(saved["attempts"][2]["status"], "completed")
        self.assertEqual([x["failure_evidence"]["signal"] for x in saved["attempts"][:2]], ["provider_capacity"] * 2)
        self.assertEqual(len(relay.resume_blockers(relay.pending_for(str(self.root), "switch"))), 0)
        self.assertTrue(all(r["target"] == calls[0]["target"] and r["permission_profile"] == "vllm-sub" for r in calls))
        self.assertEqual(len(calls), 3)

    def test_capacity_handoff_reuses_only_remaining_slots(self):
        args = self.dispatch_args()
        args.ladder = [self.codex]
        args.reasoning_effort = "low"
        args.capacity_retries = 2
        args.capacity_backoff_seconds = 1
        args.capacity_deadline_utc = "2099-01-01T00:00:00Z"
        approval = self.root / "approval.json"
        approval.write_text(json.dumps({"authorization": "actual user authorization", "source": "fixture message"}))
        args.capacity_approval_file = str(approval)
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        provider = agent_control._load_provider("codex_cli")
        real_run = subprocess.run
        calls = []
        def invoke(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            request = json.loads(Path(command[command.index("--request") + 1]).read_text())
            calls.append(request)
            events = [{"type": "thread.started", "thread_id": "capacity-session"}, {"type": "turn.started"},
                      {"type": "turn.failed", "error": {"message": "Selected model is at capacity. Please try a different model."}}]
            if len(calls) == 1:
                events = [{"type": "thread.started", "thread_id": "capacity-session"}, {"type": "turn.started"},
                          {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(
                              {"status": "input-required", "context_id": "switch", "phase": "bench", "running_services": [], "artifacts": [],
                               "task_id": "fixture", "node_id": "fixture-sub", "turn": 1, "self_verification": {"checks_run": []}})}},
                          {"type": "turn.completed", "usage": {}}]
            if len(calls) == 3:
                events = [{"type": "thread.started", "thread_id": "capacity-session"}, {"type": "turn.started"},
                          {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(
                              {"status": "completed", "context_id": "switch", "phase": "bench", "running_services": [], "artifacts": []})}},
                          {"type": "turn.completed", "usage": {}}]
            result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 0 if len(calls) in (1, 3) else 1)
            agent_control.persist_outcome(command[command.index("--result-file") + 1],
                dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
            return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=1)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=invoke), \
             mock.patch.object(relay.time, "sleep"), contextlib.redirect_stdout(io.StringIO()):
            relay.run_attempt(args, state, path, "full task", self.wall, "new")
            self.assertEqual(len(calls), 1)
            original_policy = dict(state["capacity_bursts"][0])
            args.continue_, args.apply, args.resume = True, True, "capacity-session"
            args.task_file = "explicit-task-file"
            last = state["attempts"][-1]
            outcome_path = self.root / last["outcome_path"]
            report_path = self.root / last["report_path"]
            outcome_bytes, report_bytes = outcome_path.read_bytes(), report_path.read_bytes()
            for which, broken in ((outcome_path, b"{"), (report_path, b"{"),
                                  (outcome_path, b""), (report_path, b""),
                                  (report_path, b"[]"), (report_path, b"null")):
                which.write_bytes(broken)
                self.assertFalse(relay.capacity_handoff_reentry(args, state, self.codex, original_policy, "capacity-session"))
                which.write_bytes(outcome_bytes if which == outcome_path else report_bytes)
            outcome_path.unlink()
            self.assertFalse(relay.capacity_handoff_reentry(args, state, self.codex, original_policy, "capacity-session"))
            outcome_path.write_bytes(outcome_bytes)
            report_path.unlink()
            self.assertFalse(relay.capacity_handoff_reentry(args, state, self.codex, original_policy, "capacity-session"))
            report_path.write_bytes(report_bytes)
            different = json.loads(report_bytes)
            different["report"]["task_id"] = "different-valid-report"
            report_path.write_text(json.dumps(different))
            self.assertFalse(relay.capacity_handoff_reentry(args, state, self.codex, original_policy, "capacity-session"))
            report_path.write_bytes(report_bytes)
            different = json.loads(outcome_bytes)
            different["result"]["session_id"] = "wrong-session"
            outcome_path.write_text(json.dumps(different))
            self.assertFalse(relay.capacity_handoff_reentry(args, state, self.codex, original_policy, "capacity-session"))
            outcome_path.write_bytes(outcome_bytes)
            relay.run_attempt(args, state, path, "updated complete task", self.wall, "capacity-session")
            self.assertEqual(state["capacity_bursts"][0], original_policy)
        saved = relay.load_ledger(path)
        self.assertEqual(len(calls), 3)
        self.assertEqual(len({x["dispatch_id"] for x in saved["attempts"]}), 3)
        self.assertEqual(saved["capacity_bursts"][0]["allocated_slots"], 3)
        self.assertEqual([r.get("resume_session_id") for r in calls], [None, "capacity-session", "capacity-session"])
        self.assertTrue(all(r["model"] == "gpt-6.1-sol" and r["reasoning_effort"] == "low" for r in calls))
        self.assertEqual(saved["attempts"][1]["task_instruction"]["kind"], "explicit")
        self.assertEqual(saved["attempts"][2]["task_instruction"]["kind"], "continuation")
        self.assertIn("벤치", calls[2]["task"])
        self.assertEqual(saved["attempts"][2]["status"], "completed")
        self.assertEqual(saved["attempts"][1]["failure_evidence"]["signal"], "provider_capacity")
        self.assertEqual(len(relay.resume_blockers(relay.pending_for(str(self.root), "switch"))), 0)
        self.assertTrue(all(r["target"] == calls[0]["target"] and r["permission_profile"] == "vllm-sub" for r in calls))
        self.assertEqual(len(calls), 3)

    def test_capacity_handoff_rejects_unsafe_reentry(self):
        args = self.dispatch_args()
        args.continue_, args.apply, args.resume, args.task_file = True, True, "session", "full-task"
        args.reasoning_effort = "low"
        policy = {"start_attempt": 3, "allocated_slots": 3}
        last = {"attempt": 4, "control_status": "completed", "status": "input-required",
                "end_reason": "sub_input_required", "sub_reported": True, "session_id": "session",
                "runner": self.codex, "session_binding": {"permission_profile": "vllm-sub"}}
        doc = {"context_id": "switch", "attempts": [{}, {}, {}, last]}
        req = {"provider": "codex_cli", "model": "gpt-6.1-sol", "reasoning_effort": "low"}
        with mock.patch.object(relay, "_instruction_request", return_value=req), \
             mock.patch.object(relay, "continuation_stop_reasons", return_value=[]), \
             mock.patch.object(relay, "pending_for", return_value=[]):
            for change in ({"control_status": "execution_failed"}, {"status": "completed"},
                           {"sub_reported": False}, {"session_id": None}, {"resume_honored": False}):
                altered = dict(doc, attempts=doc["attempts"][:-1] + [dict(last, **change)])
                self.assertFalse(relay.capacity_handoff_reentry(args, altered, self.codex, policy, "session"))
            self.assertFalse(relay.capacity_handoff_reentry(args, dict(doc, inflight={"id": 1}), self.codex, policy, "session"))
            self.assertFalse(relay.capacity_handoff_reentry(args, doc, self.codex, policy, "different"))
            args.task_file = None
            self.assertFalse(relay.capacity_handoff_reentry(args, doc, self.codex, policy, "session"))

    def test_capacity_policy_preview_and_rejections_preserve_ledger(self):
        args = self.dispatch_args()
        args.ladder = [self.codex]
        args.capacity_retries = 2
        args.capacity_backoff_seconds = 1
        args.capacity_deadline_utc = "2099-01-01T00:00:00Z"
        approval = self.root / "approval.json"
        approval.write_text(json.dumps({"authorization": "user text", "source": "fixture"}))
        args.capacity_approval_file = str(approval)
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        relay.save_ledger(path, state)
        before = Path(path).read_bytes()
        args.emit_only = True
        with contextlib.redirect_stdout(io.StringIO()):
            relay.run_attempt(args, state, path, "preview", self.wall, "new")
        self.assertEqual(Path(path).read_bytes(), before)
        args.emit_only = False
        args.ladder = [self.claude]
        self.assertRejected(relay.run_attempt, args, state, path, "task", self.wall, "new")
        self.assertEqual(Path(path).read_bytes(), before)
        args.ladder = [self.codex]
        with mock.patch.object(relay, "require_dispatch_recovery", side_effect=SystemExit("blocked")):
            self.assertRejected(relay.run_attempt, args, state, path, "task", self.wall, "new")
        self.assertEqual(Path(path).read_bytes(), before)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay, "capacity_deadline_fits", return_value=False):
            self.assertRejected(relay.run_attempt, args, state, path, "task", self.wall, "new", message="deadline")
        self.assertEqual(Path(path).read_bytes(), before)

    def test_inherited_capacity_burst_never_extends_not_after(self):
        import datetime
        now = datetime.datetime.now(datetime.timezone.utc)
        until = (now + datetime.timedelta(seconds=120)).strftime("%Y-%m-%dT%H:%M:%SZ")
        inherited = {"retries": 2, "backoff_seconds": 1, "deadline_utc": until,
                     "not_after_utc": until, "authorization": "actual authorization", "source": "user"}
        args = SimpleNamespace(_capacity_resume_policy=inherited)
        policy = relay.capacity_policy(args, {"attempts": []}, "unused", self.wall)
        self.assertEqual(policy["deadline_utc"], until)
        self.assertEqual(policy["not_after_utc"], until)
        approval = self.root / "deadline-approval.json"
        approval.write_text(json.dumps({"authorization": "actual authorization", "source": "user"}))
        explicit = SimpleNamespace(capacity_retries=2, capacity_backoff_seconds=1,
                                   capacity_deadline_utc="2099-01-01T00:00:00Z",
                                   capacity_approval_file=str(approval))
        legacy = {"attempts": [], "capacity_policy": {"deadline_utc": until}}
        self.assertEqual(relay.capacity_policy(explicit, legacy, "unused", self.wall)["deadline_utc"], "2099-01-01T00:00:00Z")
        expired = dict(inherited, not_after_utc="2000-01-01T00:00:00Z")
        args._capacity_resume_policy = expired
        self.assertRejected(relay.capacity_policy, args, {"attempts": []}, "unused", self.wall,
                            message="deadline")

    def test_capacity_batch_blocks_unfinished_command_and_missing_session(self):
        provider = agent_control._load_provider("codex_cli")
        for case in ("unfinished", "missing_session", "denial"):
            with self.subTest(case=case):
                root = self.root / case
                root.mkdir()
                args = self.dispatch_args()
                args.repo_root = str(root)
                args.ladder = [self.codex]
                args.capacity_retries = 2
                args.capacity_backoff_seconds = 1
                args.capacity_deadline_utc = "2099-01-01T00:00:00Z"
                approval = root / "approval.json"
                approval.write_text(json.dumps({"authorization": "user text", "source": "fixture"}))
                args.capacity_approval_file = str(approval)
                path = relay.ledger_path(str(root), "switch")
                state = relay.load_ledger(path)
                calls = []
                real_run = subprocess.run
                def invoke(command, **kwargs):
                    if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                        return real_run(command, **kwargs)
                    request = json.loads(Path(command[command.index("--request") + 1]).read_text())
                    calls.append(request)
                    events = ([{"type": "thread.started", "thread_id": "session"}] if case != "missing_session" else [])
                    events += [{"type": "turn.started"}]
                    if case == "unfinished":
                        events += [{"type": "item.started", "item": {"type": "command_execution", "id": "cmd"}}]
                    if case == "denial":
                        events += [{"type": "error", "code": "permission_denied"}]
                    events += [{"type": "turn.failed", "error": {"message": "Selected model is at capacity. Please try a different model."}}]
                    result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 1)
                    agent_control.persist_outcome(command[command.index("--result-file") + 1],
                        dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
                    return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=1)
                with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
                     mock.patch.object(relay.subprocess, "run", side_effect=invoke), \
                     mock.patch.object(relay.time, "sleep"), contextlib.redirect_stdout(io.StringIO()):
                    relay.run_attempt(args, state, path, "full task", self.wall, "new")
                self.assertEqual(len(calls), 1)
                self.assertEqual(len(relay.resume_blockers(relay.pending_for(str(root), "switch"))), 1)

    def test_codex_wall_timeout_and_successful_cli_without_report_keep_d9(self):
        provider = agent_control._load_provider("codex_cli")
        request = self.request("codex_cli", sandbox_mode="read-only")
        start = [{"type": "thread.started", "thread_id": "native"}, {"type": "turn.started"}]
        good = start + [{"type": "item.completed", "item": {"type": "agent_message", "text": "plain final prose"}},
                        {"type": "turn.completed", "usage": {}}]
        for name, events, rc, opts in [("deadline", start, 124, {}),
                                      ("adapter_deadline", start, -15, {"timed_out": True}),
                                      ("completed_cli_without_report", good, 0, {})]:
            with self.subTest(name=name):
                result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", rc, **opts)
                doc = {"context_id": "switch", "attempts": []}
                att = relay.record_attempt(doc, context_id="switch", bud=self.wall, result=result, runner=self.codex)
                if name != "completed_cli_without_report":
                    self.assertIsNotNone(relay.codex_recovery_requirement(att))
                    self.assertEqual(relay.supervise_decide(doc, doc_progress="observed context document")["action"], "popup")
                    continue
                self.assertIsNone(relay.codex_recovery_requirement(att))
                self.assertEqual(relay.continuation_stop_reasons(doc), [])
                self.assertEqual(relay.supervise_decide(doc)["action"], "popup")
                self.assertEqual(relay.supervise_decide(doc, doc_progress="observed context document")["action"], "resume")
        # A typed denial is not erased by a simultaneous deadline.
        denied = start + [{"type": "error", "code": "approval_required"}]
        result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in denied), "", 124)
        doc = {"attempts": []}
        att = relay.record_attempt(doc, context_id="switch", bud=self.wall, result=result, runner=self.codex)
        self.assertEqual(att["end_reason"], "permission_denied")
        self.assertEqual(relay.supervise_decide(doc, doc_progress="observed progress")["action"], "popup")
        # An unsignalled completed task with incidental denial retains the old contract.
        completed = start + [{"type": "error", "code": "permission_denied"},
                             {"type": "item.completed", "item": {"type": "agent_message", "text":
                              '{"status":"completed","context_id":"switch"}'}},
                             {"type": "turn.completed", "usage": {}}]
        result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in completed), "", 0)
        doc = {"attempts": []}
        att = relay.record_attempt(doc, context_id="switch", bud=self.wall, result=result, runner=self.codex,
                                   report=relay.parse_report(result["output"]))
        self.assertEqual(att["end_reason"], "completed_with_denials")
        self.assertIsNone(relay.codex_recovery_requirement(att))
        self.assertEqual(relay.supervise_decide(doc)["action"], "next_cell")

    def test_codex_dispatch_reserves_durably_before_execution_and_commits_observations(self):
        args = self.dispatch_args()
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        observed = []
        provider = agent_control._load_provider("codex_cli")
        real_run = subprocess.run

        def invoke(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            self.assertEqual(command[1:3], [relay.AGENT_CONTROL, "invoke"])
            request = json.loads(Path(command[command.index("--request") + 1]).read_text(encoding="utf-8"))
            self.assertEqual(agent_control.request_schema_violations(request), [])
            reservation = relay.load_ledger(path)
            self.assertEqual(reservation["inflight"]["provider"], "codex_cli")
            self.assertEqual(reservation["inflight"]["attempt"], 1)
            self.assertEqual(reservation["attempts"], [])
            self.assertTrue((self.root / reservation["inflight"]["request_path"]).is_file())
            observed.append(reservation["inflight"]["dispatch_id"])
            report = {"status": "completed", "context_id": "switch", "phase": "bench",
                      "running_services": [], "artifacts": []}
            events = [{"type": "thread.started", "thread_id": "codex-fixture-thread"},
                      {"type": "turn.started"},
                      {"type": "item.completed", "item": {"type": "agent_message",
                                                            "text": json.dumps(report)}},
                      {"type": "turn.completed", "usage": {"input_tokens": 12,
                                                             "output_tokens": 4}}]
            result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 0)
            self.assertEqual(agent_control._schema_violations(result, agent_control._load_schema(
                agent_control.RESULT_SCHEMA_PATH)), [])
            agent_control.persist_outcome(command[command.index("--result-file") + 1],
                    dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
            return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=0)

        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
                mock.patch.object(relay.subprocess, "run", side_effect=invoke), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(relay.run_attempt_once(args, state, path, "task", self.wall,
                                                    "new", self.codex), 0)
        saved = relay.load_ledger(path)
        self.assertNotIn("inflight", saved)
        self.assertEqual(len(saved["attempts"]), 1)
        attempt = saved["attempts"][0]
        self.assertEqual(attempt["dispatch_id"], observed[0])
        self.assertEqual(attempt["session_id"], "codex-fixture-thread")
        self.assertIsNone(attempt["max_turns_allocated"])
        self.assertIsNone(attempt["max_turns_used"])
        self.assertEqual(attempt["usage"], {"input_tokens": 12, "output_tokens": 4})
        self.assertEqual(attempt["model_used"], [], "declared model is not an observation")
        self.assertEqual(attempt["end_reason"], "completed")
        self.assertEqual(attempt["session_binding"]["permission_profile"], "vllm-sub")
        self.assertTrue((self.root / attempt["report_path"]).is_file())

    def test_lost_dispatch_output_preserves_reservation_and_prevents_replay(self):
        args = self.dispatch_args()
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        real_run = subprocess.run

        def lose_result(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            return SimpleNamespace(stdout="interrupted before JSON result",
                                   stderr="fixture lost pipe", returncode=1)

        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
                mock.patch.object(relay.subprocess, "run", side_effect=lose_result), \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertRejected(relay.run_attempt_once, args, state, path, "task", self.wall,
                                "new", self.codex, message="agent_control")
        unresolved = relay.load_ledger(path)
        self.assertTrue(unresolved.get("inflight"))
        self.assertEqual(unresolved["attempts"], [])
        with mock.patch.object(relay, "build_request", side_effect=AssertionError("replayed lost result")):
            self.assertRejected(relay.run_attempt_once, args, unresolved, path, "task", self.wall,
                                "new", self.codex, message="unresolved dispatch")
        self.assertEqual(relay.supervise_decide(unresolved)["action"], "popup")

    def test_concurrent_pending_writers_preserve_both_contexts(self):
        ctx = multiprocessing.get_context("fork")
        start = ctx.Event()
        workers = [ctx.Process(target=_pending_worker, args=(str(self.root), name, start))
                   for name in ("context-a", "context-b")]
        for worker in workers:
            worker.start()
        try:
            start.set()
            for worker in workers:
                worker.join(15)
                self.assertEqual(worker.exitcode, 0, "pending-HITL writer failed or deadlocked")
        finally:
            for worker in workers:
                if worker.is_alive():
                    worker.terminate()
                    worker.join(3)
        for name in ("context-a", "context-b"):
            pending = relay.pending_for(str(self.root), name)
            self.assertEqual(len(pending), 1)
            self.assertEqual((pending[0]["request_id"], pending[0]["attempt"]),
                             (name + "-question", 4))

    def test_real_dispatch_entrypoint_rejects_competing_owner(self):
        command = [sys.executable, "-B", str(HERE / "relay.py"),
                   "--repo-root", str(self.root), "--context-id", "competing"]
        path = str(self.root / "output" / ".relay-dispatch")
        with relay.state_lock(path):
            outcome = subprocess.run(command, capture_output=True, text=True, timeout=10)
        self.assertNotEqual(outcome.returncode, 0)
        self.assertIn("another writer owns", outcome.stderr)
        self.assertFalse((self.root / "campaigns").exists(), "rejected dispatcher mutated campaign state")

    def test_inflight_stops_replay_before_build_or_external_invocation(self):
        doc = {"attempts": [], "inflight": {"dispatch_id": "unresolved"}}
        before = copy.deepcopy(doc)
        with mock.patch.object(relay, "build_request", side_effect=AssertionError("replay built a request")):
            self.assertRejected(relay.run_attempt_once, SimpleNamespace(), doc, "unused", "task",
                                self.wall, "new", self.codex, message="unresolved dispatch")
        self.assertEqual(doc, before)
        self.assertEqual(relay.supervise_decide(doc)["action"], "popup")
        doc["attempts"] = [{"end_reason": "completed", "status": "completed"}]
        self.assertEqual(relay.supervise_decide(doc)["action"], "popup",
                         "completed older attempt must not hide unresolved dispatch")

    def test_remote_deadline_marker_requires_bound_complete_control_evidence(self):
        import base64
        provider = agent_control._load_provider("codex_cli")
        request = self.request("codex_cli", sandbox_mode="read-only")
        digest = provider._request_digest(request)
        marker = {"type": "adapter.deadline_reaped", "request_digest": digest}
        start = [{"type": "adapter.lock_acquired"}, {"type": "thread.started", "thread_id": "native"},
                 {"type": "turn.started"}]
        header = {"type": "adapter.framing_started", "request_digest": digest}
        def frames(events):
            return [header] + [{"type": "adapter.child_line", "data": base64.b64encode(json.dumps(e).encode()).decode()} for e in events]
        def parse(events, **opts):
            return provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 124, **opts)
        self.assertEqual(parse(frames(start) + [marker])["failure_evidence"]["signal"], "controlled_remote_deadline")
        for events, opts in ((frames(start), {}), (frames(start) + [dict(marker, request_digest="wrong")], {}),
                             (frames(start) + [marker, {"type": "turn.started"}], {}),
                             (frames(start[1:]) + [marker], {}),
                             (frames(start + [{"type": "error", "code": "approval_required"}]) + [marker], {}),
                             (frames(start) + [marker], {"timed_out": True}),
                             (frames(start) + [marker], {"cancelled": True})):
            self.assertNotEqual(parse(events, **opts)["failure_evidence"]["signal"], "controlled_remote_deadline")
        run = subprocess.run([sys.executable, "-c", provider._REMOTE_WATCHDOG, "0.05", digest,
                              sys.executable, "-c", "import time; time.sleep(5)"],
                             capture_output=True, text=True, timeout=5)
        self.assertEqual(run.returncode, 124)
        self.assertEqual(json.loads(run.stdout.splitlines()[-1]), marker)
        # The child can know the request digest. Its own JSONL must never be
        # mistaken for the watchdog's terminal proof, even with an exact match.
        forged_events = start + [marker]
        child = "import sys; sys.stdout.write(" + repr("".join(json.dumps(e) + "\n" for e in forged_events)) + "); sys.exit(124)"
        forged = subprocess.run([sys.executable, "-c", provider._REMOTE_WATCHDOG, "5", digest,
                                 sys.executable, "-c", child], capture_output=True, text=True, timeout=5)
        self.assertEqual(forged.returncode, 124)
        self.assertNotEqual(provider.parse_output(request, forged.stdout, forged.stderr, forged.returncode)
                            ["failure_evidence"]["signal"], "controlled_remote_deadline")
        proc_attack = ("import os,sys,time; time.sleep(.05); "
                       "os.write(os.open('/proc/%d/fd/1' % os.getppid(),os.O_WRONLY), "
                       + repr((json.dumps(marker) + "\n").encode()) + "); sys.exit(124)")
        direct = subprocess.run([sys.executable, "-c", provider._REMOTE_WATCHDOG, "5", digest,
                                 sys.executable, "-c", proc_attack], capture_output=True, text=True, timeout=5)
        self.assertEqual(direct.returncode, 124)
        self.assertNotEqual(provider.parse_output(request, direct.stdout, direct.stderr, direct.returncode)
                            ["failure_evidence"]["signal"], "controlled_remote_deadline")
        inherited = "import subprocess,sys; subprocess.Popen([sys.executable,'-c','import time; time.sleep(1)']); sys.exit(124)"
        late = subprocess.run([sys.executable, "-c", provider._REMOTE_WATCHDOG, "0.5", digest,
                               sys.executable, "-c", inherited], capture_output=True, text=True, timeout=3)
        self.assertEqual(late.returncode, 124)
        self.assertNotIn("adapter.deadline_reaped", late.stdout)
        writer = "import os,time; end=time.monotonic()+.4\nwhile time.monotonic()<end:\n try: os.write(1,b'x'*4096)\n except BrokenPipeError: break"
        parent = "import subprocess,sys; subprocess.Popen([sys.executable,'-c'," + repr(writer) + "],stderr=subprocess.DEVNULL); sys.exit(124)"
        busy = subprocess.run([sys.executable, "-c", provider._REMOTE_WATCHDOG, "2", digest,
                               sys.executable, "-c", parent], capture_output=True, text=True, timeout=4)
        self.assertEqual(busy.returncode, 124)
        self.assertNotEqual(provider.parse_output(request, busy.stdout, busy.stderr, busy.returncode)
                            ["failure_evidence"]["signal"], "controlled_remote_deadline")

    def test_local_deadline_uses_one_watchdog_and_bare_124_is_ambiguous(self):
        provider = agent_control._load_provider("codex_cli")
        request = self.request("codex_cli", sandbox_mode="read-only")
        request["target"] = {"role": "main", "transport": "local", "work_dir": str(self.root)}
        argv = provider.build_argv(request)
        self.assertNotEqual(argv[0], "timeout")
        self.assertNotIn("timeout", argv)
        events = [{"type": "adapter.lock_acquired"}, {"type": "thread.started", "thread_id": "native"},
                  {"type": "turn.started"}]
        raw = "".join(json.dumps(e) + "\n" for e in events)
        self.assertEqual(provider.parse_output(request, raw, "", 124)["failure_evidence"]["signal"], "wall_timeout")
        self.assertEqual(provider.parse_output(request, raw, "", -15, timed_out=True)["failure_evidence"]["signal"],
                         "controlled_local_deadline")

    def test_supervised_budget_uses_current_phase_evidence_and_typed_recommendations(self):
        args = SimpleNamespace(ladder=[self.codex], timeout_seconds=None, max_turns=None)
        old = {"attempt": 1, "phase": "serve", "control_status": "completed", "status": "completed",
               "duration_ms": 900000, "execution_started": True, "sub_reported": True,
               "budget_outcome": "within_budget", "provider": "codex_cli", "runner": self.codex}
        last = {"attempt": 2, "phase": "publish", "control_status": "completed", "status": "input-required",
                "execution_started": True, "sub_reported": True, "next_steps": ["remaining"], "provider": "codex_cli", "runner": self.codex,
                "timeout_seconds_allocated": 600, "budget_recommended": {"timeout_seconds": 120}}
        doc = {"attempts": [old, last]}
        budget = relay.select_supervised_budget(args, doc, 99999, "unrelated aggregate")
        self.assertEqual(budget["timeout_seconds"], 120)
        self.assertIsNone(budget["max_turns"])
        self.assertIn("recommendation", budget["source"])
        last["budget_recommended"] = {"timeout_seconds": "120"}
        budget = relay.select_supervised_budget(args, doc, 99999, "unrelated aggregate")
        self.assertEqual(budget["timeout_seconds"], 600)
        same = dict(old, attempt=3, phase="publish", duration_ms=80000, next_steps=["prior checkpoint"])
        doc["attempts"] = [old, same, last]
        self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 120)
        last["failure_evidence"] = {"signal": "controlled_remote_deadline"}
        self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 900)
        args.timeout_seconds = 77
        self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 77)
        args.timeout_seconds = None
        last["timeout_seconds_allocated"] = turn_budget.schema_cap("timeout_seconds")
        capped = relay.select_supervised_budget(args, doc)
        self.assertEqual(capped["timeout_seconds"], turn_budget.schema_cap("timeout_seconds"))
        self.assertIn("checkpoint continues", capped["source"])
        args.ladder = [self.claude]
        last.pop("failure_evidence")
        last.update(max_turns_allocated=100, budget_recommended={"timeout_seconds": 120, "max_turns": 20})
        reduced = relay.select_supervised_budget(args, doc)
        self.assertEqual((reduced["timeout_seconds"], reduced["max_turns"]), (120, 20))
        last["budget_outcome"] = "exhausted"
        self.assertEqual(relay.select_supervised_budget(args, doc)["max_turns"], turn_budget.schema_cap("max_turns"))
        args.ladder = [self.codex]
        args.max_turns = 10
        self.assertRejected(relay.select_supervised_budget, args, doc)

    def test_supervised_budget_shrink_needs_new_progress_and_same_provider(self):
        args = SimpleNamespace(ladder=[self.codex], timeout_seconds=None, max_turns=None)
        previous = {"attempt": 1, "phase": "bench", "provider": "codex_cli", "execution_started": True,
                    "status": "input-required", "control_status": "completed", "sub_reported": True, "runner": self.codex,
                    "artifact_paths": ["same"], "next_steps": ["same"]}
        last = dict(previous, attempt=2, timeout_seconds_allocated=600,
                    budget_recommended={"timeout_seconds": 120})
        doc = {"attempts": [previous, last]}
        self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 600)
        for change in ({"phase": "publish"}, {"artifact_paths": ["same", "new"]}, {"next_steps": ["changed"]}):
            doc["attempts"][-1] = dict(last, **change)
            self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 120)
        for other_runner in (self.claude, dict(self.codex, model="other-model")):
            other = dict(previous, attempt=3, provider=other_runner["provider"], runner=other_runner,
                         status="completed", duration_ms=10000)
            doc["attempts"] = [other, last]
            self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 600)
        contradicted = dict(previous, attempt=3, status="completed", duration_ms=10000,
                            model_used=["other-observed-model"])
        doc["attempts"] = [contradicted, last]
        self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 600)

    def test_successful_worker_checkpoint_calibrates_wall_without_completing_task(self):
        args = SimpleNamespace(ladder=[self.codex], timeout_seconds=None, max_turns=None)
        checkpoint = {"attempt": 1, "phase": "publish", "provider": "codex_cli", "runner": self.codex,
                      "control_status": "completed", "status": "input-required", "sub_reported": True,
                      "hitl_needed": False, "budget_outcome": "within_budget", "execution_started": True,
                      "started_utc": "2026-10-28T10:46:50Z", "ended_utc": "2026-10-28T10:50:04Z",
                      "duration_ms": None, "next_steps": ["old"]}
        last = dict(checkpoint, attempt=2, started_utc=None, ended_utc=None, next_steps=["new"],
                    timeout_seconds_allocated=600)
        doc = {"attempts": [checkpoint, last]}
        budget = relay.select_supervised_budget(args, doc)
        self.assertEqual(budget["timeout_seconds"], 291)
        self.assertIn("main_wall_ms", budget["source"])
        self.assertEqual(checkpoint["status"], "input-required")
        for bad in ({"control_status": "execution_failed"}, {"hitl_needed": True},
                    {"budget_outcome": "exhausted"}, {"permission_denials": ["denied"]}):
            doc["attempts"][0] = dict(checkpoint, **bad)
            self.assertEqual(relay.select_supervised_budget(args, doc)["timeout_seconds"], 600)

    def test_controlled_deadline_requires_durable_bound_outcome(self):
        import base64
        args = self.dispatch_args()
        args.ladder = [self.codex]
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        provider = agent_control._load_provider("codex_cli")
        real_run = subprocess.run
        def invoke(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            req = json.loads(Path(command[command.index("--request") + 1]).read_text())
            digest = provider._request_digest(req)
            child = [{"type": "adapter.lock_acquired"}, {"type": "thread.started", "thread_id": "deadline-session"},
                     {"type": "turn.started"}]
            events = [{"type": "adapter.framing_started", "request_digest": digest}] + [
                {"type": "adapter.child_line", "data": base64.b64encode(json.dumps(e).encode()).decode()} for e in child] + [
                {"type": "adapter.deadline_reaped", "request_digest": digest}]
            result = provider.parse_output(req, "".join(json.dumps(e) + "\n" for e in events), "", 124)
            agent_control.persist_outcome(command[command.index("--result-file") + 1],
                dispatch_id=command[command.index("--dispatch-id") + 1], request=req, result=result)
            return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=1)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=invoke), contextlib.redirect_stdout(io.StringIO()):
            relay.run_attempt(args, state, path, "task", self.wall, "new")
        saved = relay.load_ledger(path)
        last = saved["attempts"][-1]
        self.assertTrue(relay.controlled_deadline_ready(str(self.root), saved))
        outcome = self.root / last["outcome_path"]
        raw = outcome.read_bytes()
        outcome.unlink()
        self.assertFalse(relay.controlled_deadline_ready(str(self.root), saved))
        outcome.write_bytes(raw)
        for mutate in (lambda x: x["result"].update(session_id="wrong"),
                       lambda x: x["request"].update(model="wrong"),
                       lambda x: x["result"]["raw_events"][-1].update(request_digest="wrong")):
            altered = json.loads(raw)
            mutate(altered)
            outcome.write_text(json.dumps(altered), encoding="utf-8")
            self.assertFalse(relay.controlled_deadline_ready(str(self.root), saved))
        outcome.write_bytes(raw)
        self.assertTrue(relay.controlled_deadline_ready(str(self.root), saved))
        altered = json.loads(raw)
        altered["result"]["raw_events"].insert(-1, {"type": "item.started", "item": {"type": "command_execution", "id": "open"}})
        outcome.write_text(json.dumps(altered), encoding="utf-8")
        self.assertTrue(relay.controlled_deadline_ready(str(self.root), saved))

    def test_historical_count_caps_do_not_stop_supervision_or_enter_new_budgets(self):
        args = SimpleNamespace(ladder=[self.claude], max_turns=10, max_dispatches=None)
        state = {"max_dispatches": 1, "attempts": [{"attempt": 1}]}
        budget = relay.declared_budget(args, 30, "explicit inherited context cap", state)
        self.assertNotIn("max_dispatches", budget)
        for provider in (self.claude, self.codex):
            for count in (6, 101):
                attempts = [{"attempt": n, "end_reason": "sub_input_required", "status": "input-required",
                             "hitl_needed": False, "next_steps": [str(n)], "phase": "bench",
                             "execution_started": True, "control_status": "completed", "runner": provider} for n in range(1, count + 1)]
                doc = {"max_dispatches": 6, "attempts": attempts}
                with mock.patch.object(relay, "ledger_progress", return_value="observed progress"):
                    decision = relay.supervise_decide(doc, cost_cap_attempts=1, doc_progress="progress")
                self.assertEqual(decision["action"], "resume")
                self.assertEqual(len(doc["attempts"]), count)

    def test_native_exhaustion_requires_bound_proof_and_reconciliation_prelude(self):
        req = self.request()
        binding = relay.permission_binding(req)
        last = {"provider": "claude_code", "end_reason": "budget_exhausted", "session_id": "native",
                "session_binding": binding, "outcome_path": "o", "request_path": "r", "request_digest": "d",
                "dispatch_id": "id", "attempt": 1, "execution_started": True, "runner": self.claude,
                "timeout_seconds_allocated": 90, "max_turns_allocated": 12}
        doc = {"context_id": "switch", "topology": "multi", "manifest": str(self.manifest), "attempts": [last]}
        result = {"session_id": "native", "budget_outcome": "exhausted", "reason_codes": []}
        args = SimpleNamespace(ladder=[self.claude], max_turns=None, max_dispatches=None, topology=None,
                               manifest=None, repo_root=str(self.root), campaign_id=None, node=None,
                               permission_profile=None, sandbox_mode=None)
        with mock.patch.object(relay, "_instruction_request", return_value=req), \
             mock.patch.object(relay, "load_dispatch_outcome", return_value=(req, result)), \
             mock.patch.object(relay, "continuation_stop_reasons", return_value=[]), \
             mock.patch.object(relay, "pending_for", return_value=[]), \
             mock.patch.object(relay, "continuation_instruction", return_value={"attempt": 1}), \
             mock.patch.object(relay, "assemble_continuation", return_value="checkpoint"), \
             mock.patch.object(relay, "run_attempt", return_value=0) as dispatch:
            self.assertTrue(relay.native_budget_exhaustion_ready(str(self.root), doc))
            self.assertEqual(relay._supervise_resume(args, "unused", doc, 30, "evidence"), 0)
            task = dispatch.call_args.args[3]
            self.assertIn("백그라운드 명령", task)
            self.assertIn("sweep 상태", task)
            self.assertIn("조정 전에는 벤치를 재시작", task)
            for bad in (dict(result, budget_outcome="unknown"), dict(result, session_id="other"),
                        dict(result, reason_codes=["PERMISSION_DENIED"])):
                with mock.patch.object(relay, "load_dispatch_outcome", return_value=(req, bad)):
                    self.assertFalse(relay.native_budget_exhaustion_ready(str(self.root), doc))

    def test_preflight_quotes_apostrophe_and_has_local_and_remote_deadlines(self):
        request = self.request("codex_cli", sandbox_mode="read-only")
        request["target"]["work_dir"] = "/tmp/a'; printf injected; #"
        with mock.patch.object(relay.subprocess, "run", return_value=SimpleNamespace(
                returncode=0, stdout="multi\n", stderr="")) as called:
            self.assertEqual(relay.assert_sub_branch(request, "multi"), "multi")
        arguments, options = called.call_args
        self.assertEqual(options["timeout"], request["timeout_seconds"] + 3)
        command = arguments[0]
        self.assertIn("StrictHostKeyChecking=yes", command)
        self.assertIn("--kill-after=3s", command[-1])
        import shlex
        self.assertIn(shlex.quote(request["target"]["work_dir"]), command[-1])
        self.assertNotIn("git -C '/tmp/a'; printf", command[-1])
        with mock.patch.object(relay.subprocess, "run", side_effect=subprocess.TimeoutExpired("ssh", 3)):
            self.assertRejected(relay.assert_sub_branch, request, "multi", message="rc=124")

    def test_supervisor_restores_context_without_leaking_and_honors_resource_and_identity_gates(self):
        args = SimpleNamespace(ladder=[self.codex], max_turns=None, max_dispatches=None,
                               topology=None, manifest=None, repo_root=str(self.root),
                               campaign_id=None, node=None, permission_profile=None, sandbox_mode=None)
        state = {"context_id": "switch", "campaign_id": "approved-campaign", "campaign_node": "sub-one",
                 "control_variables": {"model": "fixed-target"}, "topology": "multi", "manifest": str(self.manifest),
                 "max_dispatches": 3, "reasoning_effort": "low", "permission_selection": {"permission_profile": "vllm-sub"},
                 "attempts": [{"attempt": 1, "status": "working", "execution_started": True,
                               "session_id": "native", "runner": self.codex,
                               "timeout_seconds_allocated": 90,
                               "control_status": "completed", "reason_codes": [],
                               "failure_evidence": {"signal": "completed", "rotate": False}}]}
        observed = []
        def record(a, *_args):
            observed.append(a)
            return 0
        with mock.patch.object(relay, "pending_for", return_value=[]), \
             mock.patch.object(relay, "continuation_instruction", return_value={"attempt": 1}), \
             mock.patch.object(relay, "assemble_continuation", return_value="artifact facts"), \
             mock.patch.object(relay, "run_attempt", side_effect=record):
            relay._supervise_resume(args, "unused", state, 30, "explicit context evidence")
            other = dict(state, context_id="other", campaign_node="sub-two",
                         permission_selection={"sandbox_mode": "read-only"}, reasoning_effort=None)
            relay._supervise_resume(args, "unused", other, 30, "explicit second context")
            self.assertEqual(observed[0].campaign_id, "approved-campaign")
            self.assertEqual(observed[0].control_variables, {"model": "fixed-target"})
            self.assertEqual(observed[0].reasoning_effort, "low")
            self.assertIsNone(observed[1].reasoning_effort)
            self.assertEqual(observed[0].node, "sub-one")
            self.assertEqual(observed[1].node, "sub-two")
            self.assertEqual(observed[1].sandbox_mode, "read-only")
            self.assertIsNone(observed[1].permission_profile)
            self.assertIsNone(args.permission_profile)
            state["attempts"][0]["running_services"] = [{"name": "existing-service"}]
            self.assertEqual(relay._supervise_resume(args, "unused", state, 30, "unsafe"), 3)
            state["attempts"][0].update(running_services=[], sub_reported=True,
                                        context_id_reported="wrong-context")
            self.assertEqual(relay._supervise_resume(args, "unused", state, 30, "wrong identity"), 3)
            self.assertEqual(len(observed), 2)

    def test_lost_stdout_recovers_real_durable_subprocess_outcome_once(self):
        args = self.dispatch_args()
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        helper = self.root / "fixture_agent.py"
        helper.write_text(
            "import json,sys\nfrom types import SimpleNamespace\n"
            "sys.dont_write_bytecode=True\n"
            f"sys.path.insert(0,{str(HERE)!r})\n"
            "import agent_control as ac\n"
            "provider=ac._load_provider('codex_cli')\n"
            "def invoke(request):\n"
            " report={'task_id':'fixture','context_id':'switch','turn':1,'phase':'inspect','status':'completed',"
            "'node_id':'fixture-sub','self_verification':{'checks_run':[]},'artifacts':[],'running_services':[]}\n"
            " events=[{'type':'thread.started','thread_id':'recovered-native'},{'type':'turn.started'},"
            "{'type':'item.completed','item':{'type':'agent_message','text':json.dumps(report)}},"
            "{'type':'turn.completed','usage':{'input_tokens':1,'output_tokens':1}}]\n"
            " return provider.parse_output(request,''.join(json.dumps(e)+'\\n' for e in events),'',0)\n"
            "ac._load_provider=lambda name:SimpleNamespace(invoke=invoke)\n"
            "ac.main()\n", encoding="utf-8")
        real_run = subprocess.run
        def lost_stdout(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            process = subprocess.Popen([sys.executable, "-B", str(helper), *command[2:]],
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            process.stdout.close()  # The producer's pipe vanishes; child must persist before emitting.
            process.wait(timeout=10)
            stderr = process.stderr.read().decode("utf-8")
            process.stderr.close()
            return SimpleNamespace(stdout="", stderr=stderr, returncode=process.returncode)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=lost_stdout), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertRejected(relay.run_attempt_once, args, state, path, "preserved original task", self.wall,
                                "new", self.codex, message="agent_control")
        pending = relay.load_ledger(path)
        self.assertEqual((pending["context_id"], pending["topology"], pending["manifest"], pending["task"]),
                         ("switch", "multi", str(self.manifest), "preserved original task"))
        original = copy.deepcopy(pending)
        request, result = relay.load_dispatch_outcome(str(self.root), pending)
        self.assertEqual(result["session_id"], "recovered-native")
        self.assertEqual(pending["attempts"], [])
        outcome_path = self.root / pending["inflight"]["outcome_path"]
        wrapper = json.loads(outcome_path.read_text())
        alias = self.root / "outcome-alias"
        alias.symlink_to(outcome_path.parent, target_is_directory=True)
        self.assertRejected(relay._owned_artifact, str(self.root), "outcome-alias/" + outcome_path.name,
                            message="symlinks")
        with self.assertRaises(FileExistsError):
            agent_control.persist_outcome(str(outcome_path), dispatch_id="different-generation",
                                          request=request, result=result)
        self.assertEqual(json.loads(outcome_path.read_text()), wrapper)
        stale = dict(wrapper, dispatch_id="older-generation")
        outcome_path.write_text(json.dumps(stale))
        self.assertRejected(relay.load_dispatch_outcome, str(self.root), pending, message="stale")
        outcome_path.write_text(json.dumps(wrapper))
        request_path = self.root / pending["inflight"]["request_path"]
        original_request = request_path.read_text()
        altered = dict(request, model="different-model")
        request_path.write_text(json.dumps(altered))
        outcome_path.write_text(json.dumps(dict(wrapper, request=altered)))
        self.assertRejected(relay.load_dispatch_outcome, str(self.root), pending, message="request bytes changed")
        request_path.write_text(original_request)
        outcome_path.write_text(json.dumps(wrapper))
        recovered = real_run([sys.executable, "-B", str(HERE / "relay.py"), "--repo-root", str(self.root),
                              "--context-id", "switch", "--reconcile-inflight", "--apply"],
                             capture_output=True, text=True, timeout=15)
        self.assertEqual(recovered.returncode, 0, recovered.stderr)
        final = relay.load_ledger(path)
        self.assertNotIn("inflight", final)
        self.assertEqual(len(final["attempts"]), 1)
        self.assertEqual(final["attempts"][0]["session_id"], "recovered-native")
        self.assertEqual(final["attempts"][0]["end_reason"], "completed")
        self.assertFalse(final["attempts"][0]["reconciliation"]["replayed"])
        self.assertNotIn("max_dispatches", final["attempts"][0])
        self.assertRejected(relay.reconcile_inflight, args, final, path, message="no unresolved")
        self.assertEqual(len(final["attempts"]), 1)

    def test_reconciliation_does_not_upgrade_a_malformed_task_self_report(self):
        args = self.dispatch_args()
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        provider = agent_control._load_provider("codex_cli")
        real_run = subprocess.run
        def lost_result(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            request = json.loads(Path(command[command.index("--request") + 1]).read_text())
            events = [{"type": "thread.started", "thread_id": "malformed-report-session"},
                      {"type": "turn.started"}, {"type": "item.completed", "item": {
                          "type": "agent_message", "text": '{"status":"completed"}'}},
                      {"type": "turn.completed", "usage": {}}]
            result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 0)
            agent_control.persist_outcome(command[command.index("--result-file") + 1],
                dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
            return SimpleNamespace(stdout="", stderr="fixture lost stdout", returncode=1)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=lost_result), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertRejected(relay.run_attempt_once, args, state, path, "task", self.wall,
                                "new", self.codex, message="agent_control")
        args.apply = True
        with contextlib.redirect_stdout(io.StringIO()):
            relay.reconcile_inflight(args, relay.load_ledger(path), path)
        final = relay.load_ledger(path)
        self.assertEqual(len(final["attempts"]), 1)
        self.assertIsNone(final["attempts"][0]["status"])
        self.assertEqual(final["attempts"][0]["end_reason"], "unclassified")
        self.assertFalse(final["attempts"][0]["sub_reported"])
        report = json.loads((self.root / final["attempts"][0]["report_path"]).read_text())
        self.assertEqual(report["raw_output"], '{"status":"completed"}')

    def instruction_fixture(self, *, legacy=False):
        """An actual builder request in fixture storage; no native/remote execution."""
        state = {"context_id": "switch", "topology": "multi", "task": "initial approved full task", "attempts": []}
        request = relay.build_request("multi", str(self.manifest), state["task"], self.wall,
            context_id="switch", attempt=1, provider="codex_cli", model="gpt-6.1-sol",
            backend="openai", permission_profile="vllm-sub")
        path = self.root / "campaigns/_bootstrap/relay/switch.requests/attempt-01.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(request), encoding="utf-8")
        att = {"attempt": 1, "provider": "codex_cli", "session_binding": relay.permission_binding(request),
               "request_path": str(path.relative_to(self.root)), "dispatch_id": "fixture-dispatch-1",
               "status": "input-required", "phase": "inspect", "session_id": "fixture-session",
               "running_services": [], "budget_outcome": "within_budget", "max_turns_allocated": None}
        if legacy:
            outcome = self.root / "campaigns/_bootstrap/relay/switch.outcomes/fixture-dispatch-1.json"
            outcome.parent.mkdir(parents=True, exist_ok=True)
            outcome.write_text(json.dumps({"schema_version": 1, "dispatch_id": att["dispatch_id"],
                                            "request": request, "result": {}}), encoding="utf-8")
            att["outcome_path"] = str(outcome.relative_to(self.root))
        else:
            att["request_digest"] = hashlib.sha256(path.read_bytes()).hexdigest()
            att["task_instruction"] = {"schema_version": 1, "kind": "explicit", "source_attempt": 1,
                "scope": relay._instruction_scope(state), "body": state["task"],
                "body_digest": hashlib.sha256(state["task"].encode()).hexdigest()}
        state["attempts"].append(att)
        return state

    def test_slice_header_distinguishes_existing_campaign(self):
        ci = mock.Mock()
        ci.Path = Path
        ci.read_declaration.return_value = {"nodes": [{"node_id": "sub", "role": "sub"}]}
        ci.emit_slice.return_value = {"plan_ref": "plan.md"}
        with mock.patch.object(relay, "_campaign_init", return_value=ci):
            header = relay.campaign_slice_block("single", "camp-x", "sub")
        self.assertIn("없으면", header)
        self.assertIn("--init camp-x", header)
        self.assertIn("이미 존재하면", header)
        self.assertIn("--slice-update - --utc <현재 UTC> --apply", header)
        self.assertIn("개정 번호와 값을 확인", header)

    def test_revised_control_axis_preserves_old_echo_and_requires_new_instruction(self):
        old_axis = {"model": "target", "vllm_version": "0.19.1"}
        new_axis = {"model": "target", "vllm_version": "0.32.0"}
        campaign = "approved-campaign"
        declaration_path = self.root / "campaigns" / campaign / "campaign.yaml"
        declaration_path.parent.mkdir(parents=True, exist_ok=True)
        declaration_path.write_text(json.dumps({"id": campaign, "control_variables": old_axis, "revisions": [
            {"rev": 0, "values": old_axis},
            {"rev": 1, "values": new_axis, "declared_utc": "2026-10-29T00:00:00Z",
             "reason": "latest requested", "evidence": "fixture", "approved_by": "user: latest"}]}), encoding="utf-8")
        saved_campaign = relay._RELAY_CAMPAIGN
        relay._RELAY_CAMPAIGN = campaign
        self.addCleanup(setattr, relay, "_RELAY_CAMPAIGN", saved_campaign)
        state = {"context_id": "switch", "campaign_id": campaign, "topology": "multi",
                 "manifest": str(self.manifest), "control_variables": old_axis,
                 "task": "original full task", "attempts": [], "permission_selection": {"permission_profile": "vllm-sub"}}
        request = relay.build_request("multi", str(self.manifest), state["task"], self.wall,
            context_id="switch", attempt=1, provider="codex_cli", model="gpt-6.1-sol", backend="openai",
            permission_profile="vllm-sub", campaign_id=campaign, control_variables=old_axis)
        path = self.root / "campaigns" / campaign / "relay/switch.requests/attempt-01.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(request), encoding="utf-8")
        att = {"attempt": 1, "provider": "codex_cli", "runner": self.codex, "session_binding": relay.permission_binding(request),
               "request_path": str(path.relative_to(self.root)), "request_digest": hashlib.sha256(path.read_bytes()).hexdigest(),
               "status": "input-required", "control_status": "completed", "sub_reported": True,
               "context_id_reported": "switch", "campaign_id_reported": campaign,
               "control_variables_echo": old_axis, "session_id": "fixture-session", "running_services": [],
               "budget_outcome": "within_budget", "max_turns_allocated": None}
        state["attempts"].append(att)
        ledger = relay.ledger_path(str(self.root), "switch")
        relay.save_ledger(ledger, state)
        task_file = self.root / "new-task.txt"
        task_file.write_text("Updated complete approved task", encoding="utf-8")
        command = ["relay.py", "--repo-root", str(self.root), "--context-id", "switch", "--continue", "--apply",
                   "--resume", "fixture-session", "--campaign-id", campaign, "--task-file", str(task_file),
                   "--timeout-seconds", "90", "--budget-source", "fixture"]
        captured = []
        def dispatch(a, doc, lp, task, bud, resume):
            relay.require_dispatch_recovery(a, doc, resume)
            new_req = relay.build_request("multi", str(self.manifest), task, bud, provider="codex_cli",
                model="gpt-6.1-sol", backend="openai", permission_profile="vllm-sub",
                campaign_id=campaign, control_variables=a.control_variables)
            captured.append(new_req)
            return 0
        with mock.patch.object(relay, "resolve_campaign_axis", return_value=(campaign, new_axis)), \
             mock.patch.object(relay, "run_attempt", side_effect=dispatch), \
             mock.patch.object(sys, "argv", command), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(relay.main(), 0)
        self.assertEqual(captured[0]["control_variables"], new_axis)
        declaration = json.loads(declaration_path.read_text(encoding="utf-8"))
        for bad_revisions in ([declaration["revisions"][0]],
                              [declaration["revisions"][0], dict(declaration["revisions"][1], approved_by="")]):
            bad = dict(declaration, revisions=bad_revisions)
            declaration_path.write_text(json.dumps(bad), encoding="utf-8")
            self.assertRejected(relay.require_official_axis_revision, str(self.root), campaign, old_axis, new_axis)
        declaration_path.write_text(json.dumps(declaration), encoding="utf-8")
        state["attempts"][0]["control_variables_echo"] = {"model": "target", "vllm_version": "wrong"}
        relay.save_ledger(ledger, state)
        with mock.patch.object(relay, "resolve_campaign_axis", return_value=(campaign, new_axis)), \
             mock.patch.object(relay, "run_attempt", side_effect=AssertionError("mismatched echo dispatched")), \
             mock.patch.object(sys, "argv", command), contextlib.redirect_stdout(io.StringIO()):
            self.assertRejected(relay.main, message="echo")
        state["attempts"][0]["control_variables_echo"] = old_axis
        relay.save_ledger(ledger, state)
        no_file = command[:command.index("--task-file")] + command[command.index("--task-file") + 2:]
        with mock.patch.object(relay, "resolve_campaign_axis", return_value=(campaign, new_axis)), \
             mock.patch.object(sys, "argv", no_file), contextlib.redirect_stdout(io.StringIO()):
            self.assertRejected(relay.main, message="explicit --task-file")

    def test_instruction_corruption_never_falls_back_to_initial_task(self):
        state = self.instruction_fixture()
        self.assertEqual(relay.continuation_instruction(str(self.root), state)["body"], state["task"])
        for field, value in (("source_attempt", 99), ("kind", "continuation"), ("body", "changed"),
                             ("body_digest", "bad"), ("scope", {})):
            bad = copy.deepcopy(state); bad["attempts"][0]["task_instruction"][field] = value
            self.assertRejected(relay.continuation_instruction, str(self.root), bad,
                                message="TASK_INSTRUCTION_SOURCE_UNRESOLVED")
        for field, value in (("request_digest", None), ("request_path", "campaigns/other/relay/switch.requests/attempt-01.json"),
                             ("provider", "claude_code"), ("session_binding", {}), ("task_instruction", "broken")):
            bad = copy.deepcopy(state); bad["attempts"][0][field] = value
            self.assertRejected(relay.continuation_instruction, str(self.root), bad,
                                message="TASK_INSTRUCTION_SOURCE_UNRESOLVED")
        for field, value in (("campaign_id", "another-campaign"), ("context_id", "another-context"),
                             ("campaign_cell", "other-cell"), ("control_variables", {"model": "other"})):
            bad = copy.deepcopy(state); bad[field] = value
            self.assertRejected(relay.continuation_instruction, str(self.root), bad,
                                message="TASK_INSTRUCTION_SOURCE_UNRESOLVED")
        path = self.root / state["attempts"][0]["request_path"]
        original = path.read_bytes()
        for raw in (b'not-json', original + b'\n'):
            path.write_bytes(raw)
            self.assertRejected(relay.continuation_instruction, str(self.root), state,
                                message="TASK_INSTRUCTION_SOURCE_UNRESOLVED")
        path.unlink()
        self.assertRejected(relay.continuation_instruction, str(self.root), state,
                            message="TASK_INSTRUCTION_SOURCE_UNRESOLVED")

    def test_legacy_single_requires_issued_evidence_and_multi_needs_main_handoff(self):
        state = self.instruction_fixture(legacy=True)
        resolved = relay.continuation_instruction(str(self.root), state)
        self.assertTrue(resolved["legacy"])
        self.assertEqual(resolved["body"], state["task"])
        bad = copy.deepcopy(state); bad["task"] = "unissued replacement"
        self.assertRejected(relay.continuation_instruction, str(self.root), bad, message="does not match")
        bad = copy.deepcopy(state); bad["attempts"].append(dict(bad["attempts"][0], attempt=2))
        self.assertRejected(relay.continuation_instruction, str(self.root), bad, message="legacy multiple attempts")
        self.assertEqual(relay.pending_for(str(self.root), "switch"), [], "technical handoff is not a human recovery question")
        self.assertNotIn("recovery_required", state["attempts"][0])
        (self.root / state["attempts"][0]["outcome_path"]).unlink()
        self.assertRejected(relay.continuation_instruction, str(self.root), state, message="legacy instruction outcome")

    def test_explicit_feedback_survives_supervision_without_recursive_task_growth(self):
        args = self.dispatch_args()
        args.ladder = [self.codex]; args.max_turns = None; args.max_dispatches = 6
        args.manifest = str(self.manifest)
        wall = turn_budget.declare(None, 90, source="fixture explicit finite sequence", unit="wall_time", max_dispatches=6)
        path = relay.ledger_path(str(self.root), "switch")
        state = relay.load_ledger(path)
        initial = "OBSOLETE: all HTTP must use urllib."
        corrected = "CURRENT: approved canonical internal HTTP is allowed; finish the approved cell and stop services."
        observed = []
        provider = agent_control._load_provider("codex_cli")
        real_run = subprocess.run
        def invoke(command, **kwargs):
            if command[1:3] != [relay.AGENT_CONTROL, "invoke"]:
                return real_run(command, **kwargs)
            request = json.loads(Path(command[command.index("--request") + 1]).read_text())
            observed.append(request)
            report = {"status": "input-required", "context_id": "switch", "phase": "bench",
                      "running_services": [], "hitl": {"needed": False, "blocking": False},
                      "next_steps": ["finish measured-result narrative"],
                      "artifacts": [{"kind": "log", "path": "docs/testlog/progress-%s.md" % len(observed)}]}
            events = [{"type": "thread.started", "thread_id": "fixture-session"}, {"type": "turn.started"},
                      {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(report)}},
                      {"type": "turn.completed", "usage": {}}]
            result = provider.parse_output(request, "".join(json.dumps(e) + "\n" for e in events), "", 0)
            agent_control.persist_outcome(command[command.index("--result-file") + 1],
                dispatch_id=command[command.index("--dispatch-id") + 1], request=request, result=result)
            return SimpleNamespace(stdout=json.dumps(result), stderr="", returncode=0)
        with mock.patch.object(relay, "assert_sub_branch", return_value="multi"), \
             mock.patch.object(relay.subprocess, "run", side_effect=invoke), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(relay.run_attempt_once(args, state, path, initial, wall, "new", self.codex), 2)
            first = copy.deepcopy(state["attempts"][0])
            self.assertEqual(relay.run_attempt_once(args, state, path, corrected, wall, "fixture-session", self.codex), 2)
            for _ in range(2):
                state = relay.load_ledger(path)
                self.assertEqual(relay.supervise_decide(state)["action"], "resume")
                self.assertEqual(relay._supervise_resume(args, path, state, 90, "fixture observed progress"), 2)
        state = relay.load_ledger(path)
        self.assertEqual(state["task"], initial)
        self.assertEqual(state["attempts"][0], first)
        for att in state["attempts"][2:]:
            self.assertEqual(att["task_instruction"]["kind"], "continuation")
            self.assertEqual(att["task_instruction"]["source_attempt"], 2)
            self.assertNotIn("body", att["task_instruction"])
        for request in observed[2:]:
            self.assertNotIn(initial, request["task"])
            self.assertEqual(request["task"].count(corrected), 1)
            self.assertEqual(request["task"].count("## 이어받기"), 1)
        self.assertEqual(relay.pending_for(str(self.root), "switch"), [])
        self.assertNotIn("max_dispatches", state)
        self.assertEqual(relay.continuation_instruction(str(self.root), state)["attempt"], 2)
        stale = copy.deepcopy(state); stale["attempts"][-1]["task_instruction"]["source_attempt"] = 1
        self.assertRejected(relay.continuation_instruction, str(self.root), stale, message="later task source")


if __name__ == "__main__":
    unittest.main(verbosity=2)
