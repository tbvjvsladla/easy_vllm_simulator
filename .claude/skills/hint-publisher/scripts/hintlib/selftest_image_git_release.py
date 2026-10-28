#!/usr/bin/env python3
"""Offline image Git provenance: no Docker, network, or image execution."""
import os
import contextlib
import json
import io
import hint
from pathlib import Path
import shutil
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

from hintlib import artifacts, evidence, naming


class ImageGitRelease(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo = self.root / 'upstream'
        subprocess.run(['git', 'init', '-q', str(self.repo)], check=True)
        subprocess.run(['git', '-C', str(self.repo), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        'commit', '--allow-empty', '-qm', 'fixture'], check=True)
        subprocess.run(['git', '-C', str(self.repo), 'tag', 'v1.2.3'], check=True)
        self.sha = subprocess.check_output(['git', '-C', str(self.repo), 'rev-parse', 'HEAD'], text=True).strip()
        self.ev = SimpleNamespace(build_identity={'vllm_ref': self.sha, 'vllm_sha': self.sha,
            'vllm_repo': 'https://github.com/vllm-project/vllm.git', 'image_digest': 'sha256:' + 'a' * 64,
            'track': 'source-build', 'source': {}}, attestation=None, node='sub',
            topology='single', manifest={'topology': 'single', 'self_role': 'sub'})
        self.calls = []
        self.runner = SimpleNamespace(image_probe=True)
        self.ctx = SimpleNamespace(runner=self.runner, default_runner=False, docker=self.docker)

    def docker(self, *args, **kw):
        self.calls.append(args)
        if args[0] == 'create':
            return 'ok', SimpleNamespace(stdout='a' * 64)
        if args[0] == 'cp':
            shutil.copytree(self.repo / '.git', Path(args[-1]), symlinks=True)
            return 'ok', SimpleNamespace(stderr='')
        if args[0] == 'rm':
            return 'ok', SimpleNamespace(stdout='')
        raise AssertionError(args)

    def probe(self):
        with mock.patch.object(artifacts, '_ctx', return_value=self.ctx), mock.patch.object(evidence, '_self_node', return_value=('sub', 'fixture')):
            artifacts.bind_image_git_release(self.root, self.ev)

    def test_real_partial_clone_local_objects_and_missing_objects(self):
        # file:// + upload-pack filtering creates real .promisor packs without network.
        subprocess.run(['git', '-C', str(self.repo), 'config', 'uploadpack.allowFilter', 'true'], check=True)
        subprocess.run(['git', '-C', str(self.repo), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        'tag', '-a', 'v1.2.4', '-m', 'annotated'], check=True)
        subprocess.run(['git', '-C', str(self.repo), 'tag', '-d', 'v1.2.3'], check=True, capture_output=True)
        partial = self.root / 'partial'
        subprocess.run(['git', 'clone', '-q', '--filter=blob:none', '--no-checkout', self.repo.as_uri(), str(partial)], check=True)
        self.assertTrue(list((partial / '.git/objects/pack').glob('*.promisor')))
        self.repo = partial
        with mock.patch.object(artifacts.subprocess, 'run', wraps=subprocess.run) as runs:
            self.probe()
        self.assertEqual(self.ev.build_identity.get('image_git_exact_tag'), 'v1.2.4')
        git_calls = [c.args[0] for c in runs.call_args_list if c.args and c.args[0][0] == 'git']
        self.assertTrue(git_calls)
        self.assertTrue(all(x[1:3] == ['-c', 'protocol.allow=never'] for x in git_calls))
        for call in runs.call_args_list:
            if call.args and call.args[0][0] == 'git':
                self.assertEqual(call.kwargs['env']['GIT_ALLOW_PROTOCOL'], '')
                self.assertEqual(call.kwargs['env']['GIT_CONFIG_NOSYSTEM'], '1')
                self.assertEqual(call.kwargs['env']['GIT_CONFIG_GLOBAL'], os.devnull)
        self.ev.build_identity.pop('image_git_exact_tag')
        # Forge a missing tag object; copied hostile helpers/config must never execute.
        helper_marker = self.root / 'helper-ran'
        with (partial / '.git/config').open('a') as config:
            config.write('\n[credential]\n helper = !touch ' + str(helper_marker) + '\n')
            config.write('[remote "origin"]\n uploadpack = touch ' + str(helper_marker) + '\n')
        packed = partial / '.git/packed-refs'
        original_refs = packed.read_text()
        lines = original_refs.splitlines()
        forged = []
        drop_peeled = False
        for line in lines:
            if line.endswith(' refs/tags/v1.2.4'):
                forged.append('f' * 40 + ' refs/tags/v1.2.4')
                drop_peeled = True
            elif drop_peeled and line.startswith('^'):
                drop_peeled = False
            else:
                forged.append(line)
                drop_peeled = False
        packed.write_text('\n'.join(forged) + '\n')
        self.probe()
        self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
        self.assertFalse(helper_marker.exists())
        packed.write_text(original_refs)
        # Remove all locally packed objects: neither missing annotated tag nor commit can fetch.
        for path in (partial / '.git/objects/pack').iterdir():
            if path.suffix in ('.pack', '.idx'):
                path.unlink()
        self.probe()
        self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
        self.assertFalse(helper_marker.exists())
        self.assertEqual([x[0] for x in self.calls], ['create', 'cp', 'rm'] * 3)

    def test_local_sub_ledger_is_bound_as_build_time_not_later_layer(self):
        self.ev.topology = "single"
        self.ev.manifest = {"topology": "single", "self_role": "sub"}
        self.ev.declaration = {"nodes": [{"node_id": "sub", "role": "sub"}]}
        self.ev.image_ledger = None
        self.ev.resolve = None
        self.ev.sources = {}
        self.ev.plane = "docker"
        self.ev.pip_freeze_paths = {}
        self.ev.pip_freeze_path = None
        ledger = {"provenance": "measured(build-time, in-image)", "vllm": {"git_sha": self.sha,
                  "describe": "v1.2.3", "dist_version": "1.2.4.dev0+gfixture"}, "patches": []}
        calls = []
        def docker(*args, **kwargs):
            calls.append(args)
            if args[0] == "image":
                return "ok", SimpleNamespace(stdout=self.ev.build_identity["image_digest"])
            if args[0] == "create":
                return "ok", SimpleNamespace(stdout="a" * 64)
            if args[0] == "cp":
                Path(args[-1]).write_text(json.dumps(ledger))
                return "ok", SimpleNamespace(stderr="")
            if args[0] == "rm":
                return "ok", SimpleNamespace(stdout="")
            raise AssertionError(args)
        ctx = SimpleNamespace(repo=self.root, ev=self.ev, node="sub", runner=self.runner,
                              default_runner=False, docker=docker)
        with mock.patch.object(artifacts, "_attestation_ref", return_value="fixture"), \
             mock.patch.object(evidence, "_self_node", return_value=("sub", "fixture")), \
             mock.patch.object(artifacts, "_image_ref", return_value=(self.ev.build_identity["image_digest"], "measured")):
            got, source, _, probes = artifacts._observe_ledger(ctx)
        self.assertEqual(got, ledger)
        self.assertIn("후속 이미지 층은 별도 관측", source)
        self.assertEqual(probes[-1]["result"], "observed")
        self.assertEqual([c[0] for c in calls], ["image", "create", "cp", "rm"])
        evidence.bind_image_ledger(self.ev, {"vllm": ledger["vllm"], "source": source, "provenance": ledger["provenance"]})
        version, where = evidence._wheel_meta(self.root, self.ev)
        self.assertEqual(version, ledger["vllm"]["dist_version"])
        self.assertIn("빌드 시점", where)
        self.assertNotIn("release", version)

    def test_sub_ledger_remote_missing_digest_and_multi_sub_do_not_probe(self):
        self.ev.topology = "single"
        ctx = SimpleNamespace(repo=self.root, ev=self.ev, node="sub", runner=self.runner,
                              default_runner=False, docker=mock.Mock())
        with mock.patch.object(artifacts, "_attestation_ref", return_value="fixture"):
            for local, topology, digest in (("main", "single", "sha256:" + "a" * 64),
                                             ("sub", "single", None),
                                             ("sub", "multi", "sha256:" + "a" * 64)):
                self.ev.topology = topology
                self.ev.build_identity["image_digest"] = digest
                with mock.patch.object(evidence, "_self_node", return_value=(local, "fixture")):
                    ledger, _, _, probes = artifacts._observe_ledger(ctx)
                self.assertIsNone(ledger)
                self.assertTrue(probes[-1]["result"].startswith("skipped"))
        self.ev.node = "main"
        self.ev.topology = "multi"
        ctx.node = "main"
        with mock.patch.object(artifacts, "_attestation_ref", return_value="fixture"), \
             mock.patch.object(evidence, "_self_node", return_value=("sub", "fixture")):
            ledger, _, _, probes = artifacts._observe_ledger(ctx)
        self.assertIsNone(ledger)
        self.assertTrue(probes[-1]["result"].startswith("skipped"))
        ctx.docker.assert_not_called()

    def test_multi_sub_git_and_readonly_sub_ledger_never_run(self):
        self.ev.topology = "multi"
        self.ev.manifest = {"topology": "multi", "self_role": "sub"}
        self.probe()
        self.assertEqual(self.calls, [])
        self.ev.topology = "single"
        self.ev.manifest = {"topology": "single", "self_role": "sub"}
        self.runner.image_probe = False
        calls = []
        def docker(*args, **kwargs):
            calls.append(args)
            if args[0] == "image":
                return "ok", SimpleNamespace(stdout="measured")
            raise AssertionError("read-only sub must not execute image")
        ctx = SimpleNamespace(repo=self.root, ev=self.ev, node="sub", runner=self.runner,
                              default_runner=False, docker=docker)
        with mock.patch.object(artifacts, "_attestation_ref", return_value="fixture"), \
             mock.patch.object(evidence, "_self_node", return_value=("sub", "fixture")), \
             mock.patch.object(artifacts, "_image_ref", return_value=(self.ev.build_identity["image_digest"], "measured")):
            ledger, _, _, probes = artifacts._observe_ledger(ctx)
        self.assertIsNone(ledger)
        self.assertEqual([c[0] for c in calls], ["image"])
        self.assertIn("no image execution", probes[-1]["result"])

    def test_public_name_path_does_not_probe(self):
        args = SimpleNamespace(repo=str(self.root), campaign="camp", cell="cell", publication=None, node="sub", json=True)
        derived = SimpleNamespace(tag="hint/fixture", to_payload=lambda: {})
        with mock.patch.object(hint.core, "resolve_repo", return_value=self.root), \
             mock.patch.object(hint.evidence, "from_campaign", return_value=self.ev), \
             mock.patch.object(hint.evidence, "naming_facts", return_value={}), \
             mock.patch.object(hint.naming, "load_vocab", return_value={}), \
             mock.patch.object(hint.naming, "derive_name", return_value=derived), \
             mock.patch.object(hint.naming, "tail_candidates", return_value=[]), \
             mock.patch.object(hint.artifacts, "bind_image_git_release", side_effect=AssertionError("name probed")), \
             contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(hint.cmd_name(args), 0)

    def test_exact_bound_tag_and_no_image_start(self):
        self.probe()
        self.assertEqual(evidence._prev_release(self.ev), 'v1.2.3')
        self.assertIn(self.ev.build_identity['image_digest'], self.ev.build_identity['source']['image_git_exact_tag'])
        self.assertEqual([x[0] for x in self.calls], ['create', 'cp', 'rm'])
        facts = {**self.ev.build_identity, 'prev_release': evidence._prev_release(self.ev)}
        self.assertEqual(naming.vllm_segment(facts).value, '1.2.3-g' + self.sha[:12])

    def test_wrong_sha_unknown_repository_or_digest_fails_closed(self):
        for field, val in [('vllm_sha', 'b' * 40), ('vllm_repo', 'https://github.com/fork/vllm'),
                           ('vllm_repo', None), ('image_digest', None)]:
            with self.subTest(field=field, val=val):
                before = self.ev.build_identity[field]
                self.ev.build_identity[field] = val
                self.probe()
                self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
                self.ev.build_identity[field] = before
        self.assertEqual(self.calls, [])

    def test_mismatched_head_multiple_tags_and_missing_tag_fail_closed(self):
        subprocess.run(['git', '-C', str(self.repo), 'tag', 'v1.2.4'], check=True)
        self.probe(); self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
        subprocess.run(['git', '-C', str(self.repo), 'tag', '-d', 'v1.2.3', 'v1.2.4'], check=True, capture_output=True)
        self.probe(); self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
        subprocess.run(['git', '-C', str(self.repo), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        'commit', '--allow-empty', '-qm', 'later'], check=True)
        self.probe(); self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
        self.assertEqual([x[0] for x in self.calls].count('rm'), 3)

    def test_other_or_unknown_local_node_never_probes(self):
        for local in ("main", None):
            with mock.patch.object(evidence, "_self_node", return_value=(local, "fixture")):
                artifacts.bind_image_git_release(self.root, self.ev, runner=self.runner)
            self.assertEqual(self.calls, [])

    def test_cluster_probe_is_owned_by_the_unique_multi_main(self):
        self.ev.node = "cluster"
        self.ev.topology = "multi"
        self.ev.manifest = {"topology": "multi", "self_role": "main"}
        self.ev.declaration = {"nodes": [{"node_id": "primary-7", "role": "main"}, {"node_id": "sub", "role": "sub"}]}
        with mock.patch.object(artifacts, "_ctx", return_value=self.ctx), mock.patch.object(evidence, "_self_node", return_value=("primary-7", "fixture")):
            artifacts.bind_image_git_release(self.root, self.ev)
        self.assertEqual(self.ev.build_identity.get("image_git_exact_tag"), "v1.2.3")
        self.ev.build_identity.pop("image_git_exact_tag")
        self.calls.clear()
        for local in ("sub", "cluster", None):
            with mock.patch.object(artifacts, "_ctx", return_value=self.ctx), mock.patch.object(evidence, "_self_node", return_value=(local, "fixture")):
                artifacts.bind_image_git_release(self.root, self.ev)
            self.assertEqual(self.calls, [])
        self.ev.topology = "single"
        with mock.patch.object(artifacts, "_ctx", return_value=self.ctx), mock.patch.object(evidence, "_self_node", return_value=("primary-7", "fixture")):
            artifacts.bind_image_git_release(self.root, self.ev)
        self.assertEqual(self.calls, [])
        self.ev.topology = "multi"
        self.ev.declaration = {"nodes": [{"node_id": "primary-7", "role": "sub"}]}
        with mock.patch.object(artifacts, "_ctx", return_value=self.ctx), mock.patch.object(evidence, "_self_node", return_value=("primary-7", "fixture")):
            artifacts.bind_image_git_release(self.root, self.ev)
        self.assertEqual(self.calls, [])
        self.ev.declaration = {"nodes": [{"node_id": "primary-7", "role": "main"}, {"node_id": "sub", "role": "sub"}]}
        self.ev.declaration = {"nodes": [{"node_id": "sub", "role": "main"}]}
        for manifest in ({"topology": "multi", "self_role": "sub"},
                         {"topology": "single", "self_role": "main"},
                         {"topology": "multi"}, {}, None):
            self.ev.manifest = manifest
            with mock.patch.object(artifacts, "_ctx", return_value=self.ctx), mock.patch.object(evidence, "_self_node", return_value=("sub", "fixture")):
                artifacts.bind_image_git_release(self.root, self.ev)
            self.assertEqual(self.calls, [])
        self.ev.manifest = {"topology": "multi", "self_role": "main"}
        self.ev.declaration = {"nodes": [{"node_id": "primary-7", "role": "main"}, {"node_id": "sub", "role": "sub"}]}
        self.ev.node = "sub"
        with mock.patch.object(artifacts, "_ctx", return_value=self.ctx), mock.patch.object(evidence, "_self_node", return_value=("primary-7", "fixture")):
            artifacts.bind_image_git_release(self.root, self.ev)
        self.assertEqual(self.calls, [])
        self.ev.node = "cluster"
        self.ev.declaration["nodes"].append({"node_id": "other", "role": "main"})
        with mock.patch.object(artifacts, "_ctx", return_value=self.ctx), mock.patch.object(evidence, "_self_node", return_value=("primary-7", "fixture")):
            artifacts.bind_image_git_release(self.root, self.ev)
        self.assertEqual(self.calls, [])

    def test_copied_root_symlink_is_rejected(self):
        original = self.ctx.docker
        def symlink_copy(*args, **kwargs):
            if args[0] == "cp":
                Path(args[-1]).symlink_to(self.repo / ".git", target_is_directory=True)
                return "ok", None
            return original(*args, **kwargs)
        self.ctx.docker = symlink_copy
        self.probe()
        self.assertNotIn("image_git_exact_tag", self.ev.build_identity)
        self.assertEqual(self.calls[-1][0], "rm")

    def test_cleanup_failure_is_not_silently_accepted(self):
        original = self.ctx.docker
        def failing_rm(*args, **kwargs):
            if args[0] == "rm":
                return "rc=1", SimpleNamespace(stderr="fixture removal refused", stdout="")
            return original(*args, **kwargs)
        self.ctx.docker = failing_rm
        with self.assertRaises(Exception) as caught:
            self.probe()
        self.assertEqual(getattr(caught.exception, "code", None), "HINT_IMAGE_PROBE_CLEANUP_FAILED")
        self.assertIn("fixture removal refused", str(caught.exception))
        self.assertNotIn("image_git_exact_tag", self.ev.build_identity)

    def test_conflicting_ledger_describe_fails_closed(self):
        self.probe()
        self.ev.attestation = {"build_ledger": {"vllm": {"describe": "v9.9.9"}}}
        with self.assertRaises(Exception) as caught:
            evidence._prev_release(self.ev)
        self.assertEqual(getattr(caught.exception, "code", None), "HINT_VLLM_INPUT_CONFLICT")

    def test_alternates_symlink_and_readonly_runner_fail_closed(self):
        alt = self.repo / '.git/objects/info/alternates'; alt.write_text('/tmp/foreign\n')
        self.probe(); self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
        alt.unlink(); (self.repo / '.git/objects/info/bad').symlink_to('/etc/passwd')
        self.probe(); self.assertNotIn('image_git_exact_tag', self.ev.build_identity)
        self.runner.image_probe = False
        n = len(self.calls); self.probe(); self.assertEqual(len(self.calls), n)


if __name__ == '__main__':
    unittest.main(verbosity=2)
