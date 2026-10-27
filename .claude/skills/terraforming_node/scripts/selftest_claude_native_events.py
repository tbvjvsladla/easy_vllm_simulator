#!/usr/bin/env python3
"""Focused offline provider/transport regressions. Fake native CLI; no model or SSH.

Event shapes follow Claude Code headless stream-json and complete SDK messages:
https://code.claude.com/docs/en/headless#stream-responses
They are synthetic protocol fixtures, never live permission-readiness evidence.
"""
from __future__ import annotations

import contextlib
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("native_receipt_agent_control", HERE / "agent_control.py")
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)

FINAL = {"type": "result", "subtype": "success", "is_error": False,
         "result": '{"phase":"inspect","status":"completed"}',
         "session_id": "fixture-session", "num_turns": 2,
         "duration_ms": 123, "duration_api_ms": 90,
         "permission_denials": [], "modelUsage": {"observed-model": {}}}
USE = {"type": "assistant", "session_id": "fixture-session", "message": {
    "role": "assistant", "content": [{"type": "tool_use", "id": "tool-1",
    "name": "Bash", "input": {"command": "/usr/bin/true"}}]}}
RECEIPT = {"type": "user", "session_id": "fixture-session", "message": {
    "role": "user", "content": [{"type": "tool_result", "tool_use_id": "tool-1",
    "is_error": False, "content": "HARNESS_PERMISSION_PROBE={\"ok\":true}"}]}}


class NativeEvents(unittest.TestCase):
    def setUp(self):
        self.provider = control._load_provider("claude_code")
        self.tmp = tempfile.TemporaryDirectory(prefix="claude-native-events-")
        self.addCleanup(self.tmp.cleanup)
        self.work = Path(self.tmp.name)
        self.request = {"schema_version": 1, "provider": "claude_code",
            "intent": "bootstrap_canary", "backend": "anthropic", "model": "declared-model",
            "task": "Synthetic offline canary", "max_turns": 8, "timeout_seconds": 10,
            "capabilities": ["read", "execute"],
            "target": {"role": "main", "transport": "local", "work_dir": str(self.work)}}

    def invoke(self, events=None, *, rc=0, raw=None, capture=True, error=None):
        req = copy.deepcopy(self.request)
        if capture is not None:
            req["capture_native_events"] = capture
        stdout = raw if raw is not None else ("\n".join(json.dumps(x) for x in events)
                                               if capture else json.dumps(FINAL))
        completed = subprocess.CompletedProcess([], rc, stdout, "")
        with mock.patch.object(self.provider.subprocess, "run", side_effect=error,
                               return_value=completed) as run, contextlib.redirect_stderr(io.StringIO()):
            result = self.provider.invoke(req)
        self.assertFalse(control._schema_violations(result, control._load_schema(control.RESULT_SCHEMA_PATH)))
        self.assertFalse(control._result_contract_violations(result, req))
        return result, run.call_args

    def test_default_and_false_keep_json_backend_resume_and_envelope(self):
        self.request["resume_session_id"] = "existing-native"
        for capture in (None, False):
            result, call = self.invoke(capture=capture)
            args = call.args[0]
            self.assertEqual(args, ["claude", "-p", self.request["task"], "--model", "declared-model",
                "--output-format", "json", "--max-turns", "8", "--allowedTools", "Read,Bash",
                "--resume", "existing-native"])
            self.assertNotIn("raw_events", result)
            self.assertEqual(result["model_used"], ["observed-model"])
            self.assertEqual((result["session_id"], result["num_turns"], result["budget_outcome"]),
                             ("fixture-session", 2, "within_budget"))
        self.request["backend"] = "claude-sub"
        result, call = self.invoke(capture=False)
        self.assertEqual(call.args[0][:4], ["bash", "-ic", 'claude-sub "$@"', "claude-sub"])
        self.assertEqual(call.args[0][-2:], ["--resume", "existing-native"])
        self.assertNotIn("raw_events", result)

    def test_opt_in_keeps_exact_tool_receipt_without_changing_terminal_report(self):
        events = [USE, RECEIPT, FINAL]
        result, call = self.invoke(events)
        self.assertEqual(result["raw_events"], events)
        self.assertEqual(result["output"], FINAL["result"])
        self.assertEqual(result["status"], "completed")
        args = call.args[0]
        self.assertEqual(args[args.index("--output-format") + 1], "stream-json")
        self.assertIn("--verbose", args)
        self.assertEqual(call.kwargs["timeout"], 10)
        self.assertEqual(call.kwargs["stdin"], subprocess.DEVNULL)

    def test_denial_budget_and_runner_failures_keep_classification_and_events(self):
        variants = [
            ({"permission_denials": [{"tool_name": "Bash", "tool_input": {"command": "denied"}}]},
             0, "PERMISSION_DENIED", None),
            ({"subtype": "error_max_turns", "is_error": True}, 1, "NONZERO_EXIT", "exhausted"),
            ({"subtype": "error_during_execution", "is_error": True},
             1, "NONZERO_EXIT", "external_interruption"),
            ({"terminal_reason": "api_error", "api_error_status": 401, "is_error": True},
             1, "RUNNER_UNAVAILABLE", None)]
        for changes, rc, reason, budget in variants:
            with self.subTest(changes=changes):
                terminal = {**FINAL, **changes}
                result, _ = self.invoke([USE, RECEIPT, terminal], rc=rc)
                self.assertEqual(result["status"], "execution_failed")
                self.assertIn(reason, result["reason_codes"])
                self.assertEqual(result["budget_outcome"], budget)
                self.assertEqual(result["session_id"], "fixture-session")
                self.assertEqual(result["raw_events"][-1], terminal)

    def test_broken_or_ambiguous_stream_never_claims_completion(self):
        good = "\n".join(map(json.dumps, [USE, RECEIPT, FINAL]))
        variants = [json.dumps(USE), good + "\n" + json.dumps(FINAL),
                    good + "\n" + json.dumps(USE), "broken\n" + good,
                    "[]\n" + good, "{}\n" + good, "", json.dumps([USE, RECEIPT, FINAL])]
        for raw in variants:
            with self.subTest(raw=raw[:60]):
                result, _ = self.invoke(raw=raw)
                self.assertEqual(result["status"], "malformed_output")
                self.assertEqual(result["reason_codes"], ["MALFORMED_JSON"])

    def test_timeout_preserves_partial_native_events_without_synthetic_terminal(self):
        partial = "\n".join(map(json.dumps, [USE, RECEIPT])).encode()
        result, _ = self.invoke(raw="", error=subprocess.TimeoutExpired("fixture", 10, output=partial))
        self.assertEqual(result["status"], "timeout")
        self.assertEqual(result["budget_outcome"], "external_interruption")
        self.assertEqual(result["raw_events"], [USE, RECEIPT])
        self.assertIsNone(result["output"])

    def test_opt_in_schema_and_dispatch_binding_fail_closed(self):
        self.assertFalse(control.request_schema_violations({**self.request, "capture_native_events": True}))
        for changes in ({"capture_native_events": "true"}, {"capture_native_events": 1},
                        {"capture_native_events": True, "intent": "delegate"}):
            self.assertTrue(control.request_schema_violations({**self.request, **changes}))
        codex = {**self.request, "schema_version": 2, "provider": "codex_cli", "backend": "openai",
                 "max_turns": None, "budget_unit": "wall_time", "max_dispatches": 1,
                 "permission_profile": "vllm-main"}
        self.assertFalse(control.request_schema_violations(codex))
        for value in (True, False):
            self.assertTrue(control.request_schema_violations({**codex, "capture_native_events": value}))
        plain, _ = self.invoke(capture=None)
        self.assertTrue(control._result_contract_violations(plain, {**self.request, "capture_native_events": True}))
        captured, _ = self.invoke([FINAL])
        self.assertTrue(control._result_contract_violations(captured, self.request))

    def test_real_local_transport_persists_receipts_in_private_outcome(self):
        # Execute a harmless fake CLI through the real agent_control command, router,
        # provider subprocess, both schemas and durable outcome writer.
        fixture = self.work / "claude"
        fixture.write_text("#!" + sys.executable + "\nimport json,sys\n"
            "assert '--verbose' in sys.argv\n"
            "assert sys.argv[sys.argv.index('--output-format')+1]=='stream-json'\n"
            "for event in " + repr([USE, RECEIPT, FINAL]) + ": print(json.dumps(event),flush=True)\n")
        fixture.chmod(0o700)
        request_path = self.work / "request.json"
        request_path.write_text(json.dumps({**self.request, "capture_native_events": True}))
        outcome = self.work / "private-outcome.json"
        env = {**os.environ, "PATH": str(self.work) + os.pathsep + os.environ.get("PATH", "")}
        done = subprocess.run([sys.executable, "-B", str(HERE / "agent_control.py"), "invoke",
            "--request", str(request_path), "--result-file", str(outcome), "--dispatch-id", "fixture-dispatch"],
            capture_output=True, text=True, timeout=20, env=env, stdin=subprocess.DEVNULL)
        self.assertEqual(done.returncode, 0, done.stderr)
        result = json.loads(done.stdout)
        self.assertEqual(result["raw_events"], [USE, RECEIPT, FINAL])
        durable = json.loads(outcome.read_text())
        self.assertEqual(durable["dispatch_id"], "fixture-dispatch")
        self.assertEqual(durable["result"], result)


if __name__ == "__main__":
    unittest.main(verbosity=2)
