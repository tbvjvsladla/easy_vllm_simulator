#!/usr/bin/env python3
"""Offline integration checks for provider handoff and durable relay ownership.

Uses real request builders, transport validation, ledger writers and subprocess locks.
Only the external agent invocation is replaced; no model, SSH or GPU is contacted.
All generated state lives in isolated temporary directories.
"""
from __future__ import annotations

import contextlib
import copy
import io
import json
import multiprocessing
from pathlib import Path
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
        self.assertEqual((request["budget_unit"], request["timeout_seconds"],
                          request["max_dispatches"]), ("wall_time", 90, 3))
        self.assertIn("AGENTS.md", request["task"])
        for changes in ({"schema_version": 1}, {"provider": "claude_code"},
                        {"max_turns": 12}, {"max_dispatches": 0},
                        {"max_dispatches": True}, {"timeout_seconds": None},
                        {"permission_profile": "vllm-main"},
                        {"sandbox_mode": "workspace-write"}):
            with self.subTest(changes=changes):
                self.assertTrue(agent_control.request_schema_violations(dict(request, **changes)))
        missing = dict(request)
        missing.pop("max_dispatches")
        self.assertTrue(agent_control.request_schema_violations(missing))
        for native, dispatch in ((1, 3), (None, None), (None, 0), (None, True)):
            with self.subTest(native=native, dispatch=dispatch):
                self.assertRejected(turn_budget.declare, native, 90, source="declared",
                                    unit="wall_time", max_dispatches=dispatch)

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
        self.assertIn("1/1", decision["reason"])
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
                               permission_profile="vllm-sub", sandbox_mode=None)

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

    def test_outer_cap_survives_switch_to_claude_and_completion_is_not_a_cap_error(self):
        args = SimpleNamespace(ladder=[self.claude], max_turns=10, max_dispatches=None)
        state = {"max_dispatches": 1, "attempts": [{"attempt": 1}]}
        budget = relay.declared_budget(args, 30, "explicit inherited context cap", state)
        self.assertEqual(budget["max_dispatches"], 1)
        with mock.patch.object(relay, "build_request", side_effect=AssertionError("cap bypass")):
            self.assertRejected(relay.run_attempt_once, SimpleNamespace(), state, "unused", "task",
                                budget, "new", self.claude, message="dispatch cap")
        args.max_dispatches = 0
        self.assertRejected(relay.declared_budget, args, 30, "zero must not inherit", state)
        state["attempts"] = [{"end_reason": "completed", "status": "completed"}]
        self.assertEqual(relay.supervise_decide(state)["action"], "next_cell")

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
                 "max_dispatches": 3, "permission_selection": {"permission_profile": "vllm-sub"},
                 "attempts": [{"attempt": 1, "status": "working", "execution_started": True,
                               "session_id": "native", "runner": self.codex}]}
        observed = []
        def record(a, *_args):
            observed.append(a)
            return 0
        with mock.patch.object(relay, "pending_for", return_value=[]), \
             mock.patch.object(relay, "assemble_continuation", return_value="artifact facts"), \
             mock.patch.object(relay, "run_attempt", side_effect=record):
            relay._supervise_resume(args, "unused", state, 30, "explicit context evidence")
            other = dict(state, context_id="other", campaign_node="sub-two",
                         permission_selection={"sandbox_mode": "read-only"})
            relay._supervise_resume(args, "unused", other, 30, "explicit second context")
            self.assertEqual(observed[0].campaign_id, "approved-campaign")
            self.assertEqual(observed[0].control_variables, {"model": "fixed-target"})
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
        self.assertEqual(final["attempts"][0]["max_dispatches"], 3)
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
