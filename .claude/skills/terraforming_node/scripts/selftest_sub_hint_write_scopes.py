#!/usr/bin/env python3
"""Offline scope declaration/rendering checks; no claim of native permission enforcement."""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest import mock

import render_sub_env as render
import check_sub_hint_equip as equip


def placeholders(topology="single"):
    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "manifest.yaml"
        p.write_text(render.FIXTURE_MANIFEST)
        data = render.parse_manifest(str(p))
    data["topology"] = topology
    ph, missing = render.build_placeholders(data)
    if missing:
        raise RuntimeError("fixture incomplete: " + repr(missing))
    return ph


def declaration(ph):
    return {"schema_version": 1, "topology": "single",
            "target": {"host": ph["SUB_HOST"], "ssh_user": ph["SSH_USER"], "work_dir": ph["WORKSPACE_PATH"]},
            "observed_home_dir": "/Users/tester", "hint_clone_dir": "/srv/hint checkout",
            "git_dir": "/srv/hint checkout/.git", "git_common_dir": "/srv/hint checkout/.git",
            "observation_source": "operator terminal report fixture", "observed_utc": "2026-10-27T04:00:00Z",
            "approved_by": "existing user scope decision fixture", "approved_utc": "2026-10-27T03:00:00Z",
            "evidence": "existing approved task fixture"}


def profile(ph, scopes=None):
    txt = Path(render.SUBNODE_DIR, "codex.config.template.toml").read_text()
    txt = render.apply_mode_blocks(txt, ph["SUB_MODE"])
    values = {**ph, "HINT_EXTERNAL_WRITE_TOML": render.hint_write_scopes_toml(
        render.validate_hint_write_scopes(ph, scopes))}
    return tomllib.loads(render.render_text(txt, values))["permissions"]["vllm-sub"]


class HintWriteScopes(unittest.TestCase):
    def setUp(self):
        self.ph = placeholders()
        self.decl = declaration(self.ph)

    def reject(self, value, ph=None):
        with self.assertRaisesRegex(SystemExit, "HINT_WRITE_SCOPES_INVALID"):
            render.validate_hint_write_scopes(ph or self.ph, value)

    def test_existing_render_has_no_external_write_grant(self):
        self.assertEqual(render.validate_hint_write_scopes(self.ph, None),
                         {"status": "not-declared", "write_paths": []})
        fs = profile(self.ph)["filesystem"]
        self.assertEqual(set(fs), {":workspace_roots"})

    def test_exact_single_paths_and_existing_protections(self):
        fs = profile(self.ph, self.decl)["filesystem"]
        self.assertEqual({k: v for k, v in fs.items() if k != ":workspace_roots"},
                         {"/srv/hint checkout": "write", "/srv/hint checkout/.git": "write"})
        self.assertEqual(fs[":workspace_roots"][".claude"], "read")
        self.assertEqual(fs[":workspace_roots"][".git"], "write")
        self.assertNotIn("workspace_roots", profile(self.ph, self.decl))

    def test_linked_worktree_metadata_is_explicit_not_inferred(self):
        self.decl.update(git_dir="/srv/hint-meta.git/worktrees/publish", git_common_dir="/srv/hint-meta.git")
        paths = render.validate_hint_write_scopes(self.ph, self.decl)["write_paths"]
        self.assertEqual(paths, [self.decl[k] for k in ("hint_clone_dir", "git_dir", "git_common_dir")])
        self.decl["git_dir"] = "/srv/unrelated.git"
        self.reject(self.decl)

    def test_multi_rejects_grants_but_keeps_read_only_buildkit(self):
        multi = placeholders("multi")
        self.reject(self.decl, multi)
        p = profile(multi)
        self.assertEqual(set(p["filesystem"]), {":workspace_roots"})
        self.assertEqual(p["filesystem"][":workspace_roots"]["output/multi/Dockerfile"], "read")

    def test_target_identity_is_exact(self):
        for key in ("host", "ssh_user", "work_dir"):
            with self.subTest(key=key):
                d = copy.deepcopy(self.decl)
                d["target"][key] += "-other"
                self.reject(d)

    def test_observation_and_approval_are_both_required(self):
        for key in ("observation_source", "observed_utc", "approved_by", "approved_utc", "evidence"):
            with self.subTest(key=key):
                d = dict(self.decl)
                d[key] = "pending"
                self.reject(d)
        self.reject({**self.decl, "unexpected_permission": True})
        self.reject({**self.decl, "schema_version": True})

    def test_roots_home_workspace_nas_and_private_config_are_rejected(self):
        for path in ("/", "/tmp", "/root", "/unlisted-root", "/home", "/Users/tester", "/Users/other", "/srv", "/etc/policy", "/usr/local/tool",
                     self.ph["WORKSPACE_PATH"], self.ph["WORKSPACE_PATH"] + "/nested",
                     str(Path(self.ph["WORKSPACE_PATH"]).parent), self.ph["NAS_MOUNT"] + "/hint",
                     "/Users/tester/.ssh/clone", "/Users/tester/.codex/clone", "/Users/tester/.config/clone"):
            with self.subTest(path=path):
                self.reject({**self.decl, "hint_clone_dir": path})

    def test_ambiguous_path_syntax_fails_closed(self):
        for path in ("relative/repo", "~/repo", "//srv/repo", "/srv/../hint", "/srv/./hint", "/srv/hint/",
                     "/srv/*", "/srv/a?", "/srv/[ab]", "/srv/{ab}", "/srv/a\\b", "/srv/a\nb"):
            with self.subTest(path=path):
                self.reject({**self.decl, "hint_clone_dir": path})

    def test_toml_quotes_unicode_and_injection_are_literal_keys(self):
        clone = '/srv/hint "출판" # key = "deny"'
        d = {**self.decl, "hint_clone_dir": clone, "git_dir": clone + "/.git", "git_common_dir": clone + "/.git"}
        fs = profile(self.ph, d)["filesystem"]
        self.assertEqual(fs[clone], "write")
        self.assertEqual(fs[clone + "/.git"], "write")
        self.assertEqual(set(fs), {clone, clone + "/.git", ":workspace_roots"})

    def test_bad_declaration_cannot_delete_existing_staging(self):
        with tempfile.TemporaryDirectory() as td:
            marker = Path(td) / "keep"
            marker.write_text("existing staging")
            with self.assertRaises(SystemExit):
                render.render_tree(self.ph, td, copy_runtime_block=False, hint_write_scopes={})
            self.assertEqual(marker.read_text(), "existing staging")

    def test_real_renderer_wires_scopes_and_both_role_probe_delivery(self):
        with tempfile.TemporaryDirectory() as td:
            single = render.render_tree(self.ph, str(Path(td) / "single"), copy_runtime_block=False,
                                        hint_write_scopes=self.decl)
            config = tomllib.loads(Path(single["out_dir"], ".codex/config.toml").read_text())
            self.assertEqual(config["permissions"]["vllm-sub"]["filesystem"][self.decl["hint_clone_dir"]], "write")
            self.assertEqual(single["hint_write_scopes"]["approved_by"], self.decl["approved_by"])
            multi = render.render_tree(placeholders("multi"), str(Path(td) / "multi"), copy_runtime_block=False)
            helper = ".claude/skills/terraforming_node/scripts/harness_permission_probe.py"
            for result in (single, multi):
                self.assertIn(helper, result["produced"])
                self.assertEqual(Path(result["out_dir"], helper).read_bytes(), Path(render.HERE, "harness_permission_probe.py").read_bytes())
                self.assertNotIn("selftest_bootstrap_permissions.py", "\n".join(result["produced"]))

    def test_json_duplicates_and_nonobjects_are_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "scopes.json"
            for text in ('null', '[]', '{"target":{},"target":{}}'):
                with self.subTest(text=text):
                    path.write_text(text)
                    with self.assertRaisesRegex(SystemExit, "HINT_WRITE_SCOPES_INVALID"):
                        render.load_hint_write_scopes(str(path))
            path.write_text(json.dumps(self.decl))
            self.assertEqual(render.load_hint_write_scopes(str(path)), self.decl)

    def test_real_local_git_observation_does_not_invent_approval(self):
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            ws, clone, worktree = base / "ws", base / "hint", base / "linked"
            (ws / "envs").mkdir(parents=True)
            env = {**os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
                   "GIT_AUTHOR_NAME": "fixture", "GIT_AUTHOR_EMAIL": "fixture@invalid",
                   "GIT_COMMITTER_NAME": "fixture", "GIT_COMMITTER_EMAIL": "fixture@invalid"}
            def git(*args):
                return subprocess.run(["git", *args], env=env, check=True, capture_output=True, text=True)
            git("init", "-q", "-b", "hint", str(clone))
            git("-C", str(clone), "commit", "--allow-empty", "-qm", "fixture")
            git("-C", str(clone), "remote", "add", "origin", "https://example.invalid/project")
            git("-C", str(clone), "worktree", "add", "-qb", "fixture", str(worktree))
            (ws / "envs/.env").write_text("HINT_CLONE_DIR=" + str(worktree) + "\nGITHUB_TOKEN=fixture_not_a_secret\n")
            obs = equip.check_clone(ws)
            self.assertEqual(obs["status"], "ok")
            self.assertEqual(obs["hint_clone_dir"], str(worktree))
            self.assertEqual(obs["git_common_dir"], str(clone / ".git"))
            self.assertEqual(obs["git_dir"], str(clone / ".git/worktrees/linked"))
            self.assertNotIn("approved_by", obs)
            self.assertNotIn("fixture_not_a_secret", json.dumps(obs))

    def test_inherited_git_location_and_config_cannot_redirect_observation(self):
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            ws, clone, other = base / "ws", base / "hint", base / "unrelated"
            (ws / "envs").mkdir(parents=True)
            env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
            env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                       GIT_AUTHOR_NAME="fixture", GIT_AUTHOR_EMAIL="fixture@invalid",
                       GIT_COMMITTER_NAME="fixture", GIT_COMMITTER_EMAIL="fixture@invalid")
            def git(*args, use_env=None):
                return subprocess.run(["git", *args], env=use_env or env, check=True,
                                      capture_output=True, text=True)
            for path in (clone, other):
                git("init", "-q", "-b", "hint", str(path))
                git("-C", str(path), "commit", "--allow-empty", "-qm", "fixture")
            git("-C", str(clone), "remote", "add", "origin", "https://example.invalid/approved")
            git("-C", str(other), "remote", "add", "origin", "http://example.invalid/unrelated")
            (ws / "envs/.env").write_text("HINT_CLONE_DIR=" + str(clone) + "\n")
            poison = {"GIT_DIR": str(other / ".git"), "GIT_WORK_TREE": str(other),
                      "GIT_COMMON_DIR": str(other / ".git"), "GIT_INDEX_FILE": str(other / ".git/index"),
                      "GIT_OBJECT_DIRECTORY": str(other / ".git/objects"),
                      "GIT_ALTERNATE_OBJECT_DIRECTORIES": str(other / ".git/objects"),
                      "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "remote.origin.url",
                      "GIT_CONFIG_VALUE_0": "http://example.invalid/injected",
                      "GIT_CONFIG_PARAMETERS": "'remote.origin.url'='http://example.invalid/injected'",
                      "GIT_CONFIG_GLOBAL": str(other / ".git/config"),
                      "GIT_CONFIG_SYSTEM": str(other / ".git/config"),
                      "GIT_SSH_COMMAND": "fixture-preserve-ssh -F /fixture/config",
                      "GIT_SSH_VARIANT": "ssh", "SSH_AUTH_SOCK": "/fixture/agent.sock"}
            # The negative control actually observes the unrelated repository.
            redirected = git("-C", str(clone), "rev-parse", "--absolute-git-dir", use_env={**env, **poison})
            self.assertEqual(redirected.stdout.strip(), str(other / ".git"))
            def snapshot():
                return {str(p.relative_to(base)): p.read_bytes() for root in (clone, other, ws)
                        for p in root.rglob("*") if p.is_file()}
            before = snapshot()
            with mock.patch.dict(os.environ, poison), mock.patch.object(equip.subprocess, "run", wraps=subprocess.run) as observed:
                result = equip.check_clone(ws)
            self.assertEqual(result["status"], "ok")
            self.assertEqual(result["origin_scheme"], "https")
            self.assertEqual(result["hint_clone_dir"], str(clone))
            self.assertEqual(result["git_dir"], str(clone / ".git"))
            self.assertEqual(result["git_common_dir"], str(clone / ".git"))
            for call in observed.call_args_list:
                used = call.kwargs["env"]
                self.assertFalse(any(k.startswith("GIT_CONFIG") for k in used))
                self.assertFalse(set(poison).intersection(used) - {"GIT_SSH_COMMAND", "GIT_SSH_VARIANT", "SSH_AUTH_SOCK"})
                self.assertEqual(used["GIT_SSH_COMMAND"], poison["GIT_SSH_COMMAND"])
                self.assertEqual(used["SSH_AUTH_SOCK"], poison["SSH_AUTH_SOCK"])
                self.assertEqual(used["GIT_OPTIONAL_LOCKS"], "0")
            self.assertEqual(snapshot(), before)


if __name__ == "__main__":
    unittest.main(verbosity=2)
