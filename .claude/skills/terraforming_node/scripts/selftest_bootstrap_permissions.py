#!/usr/bin/env python3
"""Offline canary evidence regressions. No native CLI, network, Docker, GPU or sudo.

Native envelopes/tool receipts below are explicit fixtures, not runtime policy evidence.
One helper test really writes/commits only within a disposable local fixture checkout;
Docker/library responses are substituted there and never counted as live observations.
"""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import agent_control as ac
import bootstrap_canary as canary
import harness_permission_probe as probe


class CanaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / "manifest.yaml"
        self.manifest.write_text("topology: multi\nnodes:\n"
                                 "  - role: sub\n    host: 203.0.113.11\n"
                                 "    ssh_user: tester\n    work_dir: /srv/fixture\n")

    def request(self, topology="multi", role="sub", provider="codex_cli", **extra):
        args = dict(phase="permissions", target_role=role, provider=provider,
                    model="gpt-6.1-sol" if provider == "codex_cli" else "sonnet",
                    max_turns=None if provider == "codex_cli" else 10, timeout_seconds=120,
                    budget_source="declared: offline permission receipt fixture",
                    main_work_dir=str(self.root),
                    **({"max_dispatches": 1, "permission_profile": "vllm-" + role}
                       if provider == "codex_cli" else {}))
        args.update(extra)
        self.manifest.write_text(self.manifest.read_text().replace("topology: multi", "topology: " + topology))
        return canary.build_request(topology, str(self.manifest), **args)

    def packet(self, request):
        plan = canary.permission_plan(request)
        functional = ["scratch_write_cleanup", "scratch_local_git", "scratch_removed",
                      "workspace_git_metadata", "docker_daemon_read", "docker_compose_read", "library_query",
                      "model_asset_read", "buildkit_declaration"]
        if plan["topology"] == "single" and request["target"]["role"] == "sub":
            functional += ["hint_clone_write", "hint_clone_git_discovery", "hint_clone_write_git_1"]
        checks = [{"name": n, "axis": "functional", "outcome": "allowed"} for n in functional]
        checks += [{"name": "buildkit:" + n, "axis": "os_enforcement", "preserved": True,
                    "outcome": "denied" if plan["topology"] == "multi" and request["target"]["role"] == "sub" else "allowed"}
                   for n in (*probe.BUILD_FILES, "Dockerfile.source-build")]
        checks += [{"name": "protected:" + n, "axis": "os_enforcement", "outcome": outcome, "preserved": True}
                   for n, outcome in probe.protected_file_expectations(request["target"]["role"]).items()]
        return dict(schema_version=1, nonce=plan["nonce"], topology=plan["topology"], role=request["target"]["role"],
                    workspace=request["target"]["work_dir"], provenance="observed_local_process", checks=checks,
                    build_files=[*probe.BUILD_FILES, "Dockerfile.source-build"])

    def result(self, request, packet=None):
        plan = canary.permission_plan(request)
        text = probe.MARKER + json.dumps(packet or self.packet(request)) + "\n"
        command = __import__("shlex").join(plan["argv"])
        report = dict(task_id="harness-canary-" + plan["nonce"], context_id="harness-canary-" + plan["nonce"],
                      turn=1, node_id=request["target"]["role"], phase="inspect", status="completed",
                      self_verification={"checks_run": ["harness_permission_probe"], "schema_valid": True})
        events = [{"type": "item.completed", "item": {"type": "command_execution", "command": command,
                    "status": "completed", "exit_code": 0, "aggregated_output": text}}]
        if request["provider"] == "claude_code":
            events = [{"type": "assistant", "message": {"content": [
                {"type": "tool_use", "id": "use-1", "name": "Bash", "input": {"command": command}}]}},
                {"type": "user", "message": {"content": [
                {"type": "tool_result", "tool_use_id": "use-1", "content": text}]}}]
        result = dict(schema_version=request["schema_version"], provider=request["provider"], intent="bootstrap_canary",
                      status="completed", exit_code=0, model_requested=request["model"], model_used=[], reason_codes=[],
                      output=json.dumps(report), session_id="fixture-session", num_turns=None, budget_outcome="within_budget",
                      duration_ms=None, duration_api_ms=None, raw_events=events)
        if request["provider"] == "codex_cli":
            result.update(raw_exit_code=0, execution_started=True, usage=None, failure_evidence=None)
        return result

    def check(self, req, res):
        return canary.verify_result(req, res)

    def test_legacy_discovery_is_unchanged_and_cannot_prove_permissions(self):
        req = canary.build_request("multi", str(self.manifest), max_turns=10, timeout_seconds=120,
                                  budget_source="declared: discovery")
        self.assertNotIn("capture_native_events", req)
        self.assertIsNone(canary.permission_plan(req))
        self.assertIn("아무 파일도 고치지 마라", req["task"])

    def test_main_local_does_not_require_a_sub(self):
        self.manifest.write_text("topology: single\nnodes: []\n")
        req = self.request(topology="single", role="main")
        self.assertEqual(req["target"], {"role": "main", "transport": "local", "work_dir": str(self.root)})
        self.assertFalse(ac.request_schema_violations(req))

    def test_codex_permission_phase_requires_explicit_model_and_named_role(self):
        for changes in ({"model": None}, {"permission_profile": None},
                        {"sandbox_mode": "read-only"}, {"permission_profile": "vllm-main"}):
            with self.subTest(changes=changes), self.assertRaises(SystemExit):
                self.request(**changes)

    def test_codex_exact_receipt_and_role_work_complete(self):
        for topology, role in (("single", "sub"), ("multi", "sub"), ("single", "main"), ("multi", "main")):
            with self.subTest(topology=topology, role=role):
                req = self.request(topology, role)
                out = self.check(req, self.result(req))
                self.assertEqual(out["status"], "completed", out)
                self.assertEqual(out["native_cli"]["model_observed"], [])

    def test_read_only_self_report_without_tool_receipt_never_passes(self):
        req = self.request(); res = self.result(req); res["raw_events"] = []
        res["output"] = json.dumps({**json.loads(res["output"]), "notes": probe.MARKER + json.dumps(self.packet(req))})
        self.assertEqual(self.check(req, res)["status"], "input-required")

    def test_missing_work_and_os_failure_do_not_pass(self):
        req = self.request()
        for name in ("scratch_removed", "docker_daemon_read", "library_query", "workspace_git_metadata", "protected:AGENTS.md"):
            packet = self.packet(req); packet["checks"] = [c for c in packet["checks"] if c["name"] != name]
            self.assertEqual(self.check(req, self.result(req, packet))["status"], "input-required", name)

    def test_single_external_clone_required_but_multi_has_no_hint_grant(self):
        req = self.request("single"); packet = self.packet(req)
        packet["checks"] = [c for c in packet["checks"] if not c["name"].startswith("hint_clone")]
        self.assertEqual(self.check(req, self.result(req, packet))["status"], "input-required")
        req = self.request("multi")
        self.assertFalse(any(c["name"].startswith("hint_clone") for c in self.packet(req)["checks"]))

    def test_binding_rejects_wrong_nonce_workspace_and_report_identity(self):
        req = self.request()
        for field in ("nonce", "workspace", "topology", "role", "provenance"):
            packet = self.packet(req); packet[field] = "other"
            self.assertEqual(self.check(req, self.result(req, packet))["status"], "input-required", field)
        res = self.result(req); report = json.loads(res["output"]); report["context_id"] = "other"; res["output"] = json.dumps(report)
        self.assertEqual(self.check(req, res)["status"], "input-required")

    def test_ambiguous_and_failed_tool_receipts_rejected(self):
        req = self.request(); res = self.result(req)
        res["raw_events"] *= 2
        self.assertEqual(self.check(req, res)["status"], "input-required")
        res = self.result(req); res["raw_events"][0]["item"]["exit_code"] = 1
        self.assertEqual(self.check(req, res)["status"], "input-required")

    def test_receipt_command_cannot_be_echo_or_append_shell_code(self):
        req = self.request(); res = self.result(req)
        for command in ("echo fake", res["raw_events"][0]["item"]["command"] + "; echo fake"):
            res["raw_events"][0]["item"]["command"] = command
            self.assertEqual(self.check(req, res)["status"], "input-required")

    def test_tampered_plan_command_rejected(self):
        req = self.request(); plan = canary.permission_plan(req); plan["argv"] = ["echo", "fake"]
        req["task"] = canary.PLAN_PREFIX + json.dumps(plan)
        self.assertIsNone(canary.permission_plan(req))

    def test_claude_linked_native_receipt_and_os_surface_are_separate(self):
        req = self.request(provider="claude_code"); self.assertTrue(req["capture_native_events"])
        self.assertFalse(ac.request_schema_violations(req))
        packet = self.packet(req)
        for c in packet["checks"]:
            if c["axis"] == "os_enforcement": c["outcome"] = "allowed"
        out = self.check(req, self.result(req, packet))
        self.assertEqual(out["status"], "completed", out)
        self.assertEqual(out["enforcement_evidence"]["status"], "unproven")
        res = self.result(req); res["raw_events"][1]["message"]["content"][0]["tool_use_id"] = "unrelated"
        self.assertEqual(self.check(req, res)["status"], "input-required")

    def test_native_failure_and_invalid_report_separate_from_probe(self):
        req = self.request(); res = self.result(req); res["status"] = "timeout"; res["exit_code"] = 124
        self.assertEqual(self.check(req, res)["status"], "input-required")
        res = self.result(req); res["output"] = "not json"
        self.assertEqual(self.check(req, res)["task_report"]["status"], "unproven")

    def test_probe_real_local_fixture_only_and_no_canonical_mutation(self):
        ws = self.root / "workspace"; ws.mkdir(); clone = self.root / "external-hint"; clone.mkdir()
        for p in (ws, clone): subprocess.run(["git", "init", "--quiet", str(p)], check=True, capture_output=True)
        (ws / "envs").mkdir(); (ws / "envs/.env").write_text(f'HINT_CLONE_DIR={clone}\nGITHUB_TOKEN=DO_NOT_EXPOSE\n')
        for rel in (*probe.BUILD_FILES, "Dockerfile"):
            f = ws / "output/single" / rel; f.parent.mkdir(parents=True, exist_ok=True); f.write_bytes(b'fixture\x00bytes')
        (ws / "output/single/docker-compose.yaml").write_text('services:\n  vllm:\n    build:\n      dockerfile: Dockerfile\n')
        (ws / "output/single/manifest.yaml").write_text('topology: single\nmodel_source: managed\nnas_model_path: ' + str(clone) + '\n')
        saved = {p: p.read_bytes() for p in (ws / "output/single").iterdir()}
        original = probe._command
        def command(argv, cwd):
            if argv[0] == "docker": return 0, '"fixture-server"' if argv[1] == "version" else "fixture-compose"
            if argv[0] == "node": return 0, '{"query_tokens":["fixture"],"results":[]}'
            return original(argv, cwd)
        with mock.patch.object(probe, "_command", side_effect=command):
            packet = probe.observe(ws, "single", "main", "ab" * 16)
        self.assertTrue(all(c["outcome"] == "allowed" for c in packet["checks"]
                            if not c["name"].startswith("protected:")), packet)
        self.assertTrue(all(c["outcome"] == "unobserved" for c in packet["checks"]
                            if c["name"].startswith("protected:")), packet)
        self.assertNotIn("DO_NOT_EXPOSE", json.dumps(packet))
        self.assertFalse(list(ws.glob(".harness-permission-*")))
        self.assertFalse(list((ws / "docs/logs").glob("harness-permission-*")))
        self.assertFalse(list(clone.rglob(".harness-permission-*")))
        self.assertTrue(all(p.read_bytes() == b for p, b in saved.items()))
        self.assertEqual(subprocess.run(["git", "status", "--porcelain"], cwd=clone, capture_output=True, text=True).stdout, "")

    def test_preexisting_marker_preserved_and_missing_file_not_denial(self):
        marker = self.root / (".harness-permission-" + "ab" * 16); marker.write_text("existing")
        self.assertEqual(probe._write_marker(self.root, "ab" * 16), "unobserved")
        self.assertEqual(marker.read_text(), "existing")
        self.assertEqual(probe._write_open(self.root / "absent")["outcome"], "unobserved")

    def test_ephemeral_na_is_explicit_not_a_fake_model_read(self):
        req = self.request(); packet = self.packet(req)
        item = next(c for c in packet["checks"] if c["name"] == "model_asset_read")
        item.update(outcome="not_applicable", model_source="ephemeral")
        self.assertEqual(self.check(req, self.result(req, packet))["status"], "completed")
        item["model_source"] = "managed"
        self.assertEqual(self.check(req, self.result(req, packet))["status"], "input-required")

    def test_provider_event_surface_and_unique_tool_ids(self):
        codex = self.request(); claude = self.request(provider="claude_code")
        for req, other in ((codex, claude), (claude, codex)):
            res = self.result(req); res["raw_events"] = self.result(other)["raw_events"]
            self.assertEqual(self.check(req, res)["status"], "input-required")
        res = self.result(claude)
        for ident in (None, ""):
            res["raw_events"][0]["message"]["content"][0]["id"] = ident
            res["raw_events"][1]["message"]["content"][0]["tool_use_id"] = ident
            self.assertEqual(self.check(claude, res)["status"], "input-required")
        res = self.result(claude); res["raw_events"].insert(1, copy.deepcopy(res["raw_events"][0]))
        self.assertEqual(self.check(claude, res)["status"], "input-required")

    def test_result_intent_and_version_bound_to_request(self):
        req = self.request()
        for k, v in (("intent", "probe"), ("schema_version", 1)):
            res = self.result(req); res[k] = v
            self.assertEqual(self.check(req, res)["status"], "input-required")
        self.assertEqual(self.check(req, [])["status"], "input-required")

    def test_main_requires_owned_canonical_writes_and_preserved_bytes(self):
        req = self.request(role="main")
        self.assertEqual(probe.protected_file_expectations("main"),
                         {".codex/config.toml": "allowed", ".agents/skills/wiki-desk/SKILL.md": "allowed"})
        out = self.check(req, self.result(req))
        self.assertEqual(out["status"], "completed")
        self.assertIn("not denial outside", out["enforcement_evidence"]["limitations"][0])
        for name in ("protected:.codex/config.toml", "protected:.agents/skills/wiki-desk/SKILL.md"):
            for change in ({"outcome": "denied"}, {"outcome": "unobserved"}, {"preserved": False}):
                with self.subTest(name=name, change=change):
                    packet = self.packet(req)
                    item = next(c for c in packet["checks"] if c["name"] == name)
                    item.update(change)
                    result = self.check(req, self.result(req, packet))
                    self.assertEqual(result["domain_evidence"]["status"], "pass")
                    self.assertEqual(result["enforcement_evidence"]["status"], "unproven")
                    self.assertEqual(result["status"], "input-required")
            packet = self.packet(req); packet["checks"] = [c for c in packet["checks"] if c["name"] != name]
            self.assertEqual(self.check(req, self.result(req, packet))["status"], "input-required")

    def test_main_write_grant_does_not_authorize_sub_canonical_writes(self):
        for topology in ("single", "multi"):
            req = self.request(topology=topology)
            for rel in probe.PROTECTED_FILES:
                with self.subTest(topology=topology, path=rel):
                    packet = self.packet(req)
                    item = next(c for c in packet["checks"] if c["name"] == "protected:" + rel)
                    # A packet's proposed expectation cannot replace the role contract.
                    item.update(outcome="allowed", expected_outcome="allowed")
                    result = self.check(req, self.result(req, packet))
                    self.assertEqual(result["enforcement_evidence"]["status"], "unproven")
                    self.assertEqual(result["status"], "input-required")

    def test_permission_prompt_uses_the_same_role_expectations(self):
        for role, outcome in (("main", "allowed"), ("sub", "denied")):
            req = self.request(role=role)
            self.assertIn('".codex/config.toml":"' + outcome + '"', req["task"])
            self.assertIn('".agents/skills/wiki-desk/SKILL.md":"' + outcome + '"', req["task"])

    def test_main_write_open_observation_preserves_canonical_fixture_bytes(self):
        for rel in (".codex/config.toml", ".agents/skills/wiki-desk/SKILL.md"):
            path = self.root / rel; path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'canonical fixture\n')
            self.assertEqual(probe._write_open(path), {"outcome": "allowed", "preserved": True})
            self.assertEqual(path.read_bytes(), b'canonical fixture\n')

    def test_wheel_track_requires_active_file_not_absent_source_file(self):
        req = self.request(); packet = self.packet(req)
        packet["build_files"] = [*probe.BUILD_FILES, "Dockerfile"]
        for c in packet["checks"]:
            if c["name"] == "buildkit:Dockerfile.source-build": c["name"] = "buildkit:Dockerfile"
        self.assertEqual(self.check(req, self.result(req, packet))["status"], "completed")
        packet["checks"] = [c for c in packet["checks"] if c["name"] != "buildkit:Dockerfile"]
        self.assertEqual(self.check(req, self.result(req, packet))["status"], "input-required")

    def test_hint_parent_repository_rejected(self):
        ws = self.root / "child"; (ws / "envs").mkdir(parents=True)
        (ws / "envs/.env").write_text('HINT_CLONE_DIR=' + str(self.root) + '\n')
        with mock.patch.object(probe, "_command") as run:
            self.assertIsNone(probe._hint_root(ws)); run.assert_not_called()

    def test_ambient_git_location_and_template_cannot_touch_existing_repo(self):
        outside = self.root / "existing"; outside.mkdir()
        subprocess.run(["git", "init", "--quiet", "--template=", str(outside)], check=True, capture_output=True)
        subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@invalid", "-c",
                        "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-qm", "Existing"],
                       cwd=outside, check=True, capture_output=True)
        snapshot = {str(p.relative_to(outside)): p.read_bytes() for p in (outside / ".git").rglob("*") if p.is_file()}
        own = self.root / "own"; own.mkdir()
        template = self.root / "bad-template"; template.mkdir(); (template / "must-not-copy").write_text("fixture")
        poison = {"GIT_DIR": str(outside / ".git"), "GIT_WORK_TREE": str(outside),
                  "GIT_INDEX_FILE": str(outside / ".git/index"), "GIT_TEMPLATE_DIR": str(template),
                  "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "core.worktree", "GIT_CONFIG_VALUE_0": str(outside)}
        with mock.patch.dict(os.environ, poison):
            self.assertEqual(probe._command(["git", "init", "--quiet", "--template="], own)[0], 0)
            self.assertEqual(probe._git_dirs(own), [own / ".git"])
            self.assertEqual(probe._command(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@invalid", "-c",
                             "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-qm", "Scratch"], own)[0], 0)
            output = own / "output/multi"; output.mkdir(parents=True)
            (output / "docker-compose.yaml").write_text('services:\n  vllm:\n    build:\n      dockerfile: Dockerfile\n')
            (output / "Dockerfile").write_text('FROM fixture\n')
            (output / "requirements.txt").write_text('fixture\n')
            (output / "manifest.yaml").write_text('model_source: ephemeral\n')
            original = probe._command
            def command(argv, cwd):
                if argv[0] == "docker": return 0, '"fixture-server"' if argv[1] == "version" else "fixture-compose"
                if argv[0] == "node": return 0, '{"query_tokens":["fixture"]}'
                return original(argv, cwd)
            with mock.patch.object(probe, "_command", side_effect=command):
                observed = probe.observe(own, "multi", "main", "cd" * 16)
            self.assertEqual(next(c for c in observed["checks"] if c["name"] == "scratch_local_git")["outcome"], "allowed")
        self.assertFalse((own / ".git/must-not-copy").exists())
        self.assertEqual(snapshot, {str(p.relative_to(outside)): p.read_bytes() for p in (outside / ".git").rglob("*") if p.is_file()})


if __name__ == "__main__":
    unittest.main(verbosity=2)
