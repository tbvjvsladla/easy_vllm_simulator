"""Phase 7 topology-neutral policy trust contract (strict TDD)."""
import ast
import importlib.util
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = ROOT / ".claude/policies/registry.yaml"
BINDINGS = ROOT / ".claude/policies/claim_bindings.json"
INDEX = ROOT / ".claude/policies/tracked_index.json"
PREDICATES = ROOT / "tests/harness/test_policy_claim_predicates.py"
RENDERER = ROOT / ".claude/skills/upstream-version-watch/scripts/render_dockerfile.py"
SHARED = ROOT / ".claude/skills/upstream-version-watch"

FORBIDDEN_PREFIXES = ("output/", "configs/", "README.md", "HINTS.md", "hints/")


def _registry():
    return json.loads(REGISTRY.read_text(encoding="utf-8"))


class TestTopologyNeutralPolicyTrust(unittest.TestCase):
    def test_exact_cardinality_is_13_56_56_56(self):
        registry = _registry()
        policies = registry["policies"]
        clauses = [f"{p['policy_id']}.{c['clause_id']}" for p in policies for c in p["clauses"]]
        bindings = json.loads(BINDINGS.read_text(encoding="utf-8"))["bindings"]
        tree = ast.parse(PREDICATES.read_text(encoding="utf-8"))
        predicates = {n.name for n in tree.body if isinstance(n, ast.FunctionDef)
                      and n.name.startswith("predicate_")}
        self.assertEqual((len(policies), len(clauses), len(bindings), len(predicates)), (13, 56, 56, 56))

    def test_registry_evidence_never_uses_branch_local_runtime_roots(self):
        paths = [e["path"] for p in _registry()["policies"] for e in p["evidence"]]
        bad = [p for p in paths if p.startswith(FORBIDDEN_PREFIXES)]
        self.assertEqual(bad, [])

    def test_predicates_never_read_branch_local_runtime_roots(self):
        source = PREDICATES.read_text(encoding="utf-8")
        bad = re.findall(r'_read\(["\']((?:output|configs)/[^"\']+)["\']\)', source)
        self.assertEqual(bad, [])

    def test_renderer_and_cleanup_use_shared_sources_not_branch_runtime_authority(self):
        source = RENDERER.read_text(encoding="utf-8")
        cleanup = (ROOT / "scripts/cleanup_docker.py").read_text(encoding="utf-8")
        self.assertIn("SHARED_ASSET_DIR", source)
        self.assertIn("SHARED_TEMPLATE_DIR", source)
        self.assertNotIn('src_dir = os.path.join(repo, "configs")', source)
        self.assertIn('"upstream-version-watch", "templates"', cleanup)
        self.assertIn("SHARED_RESOLUTION", source)
        self.assertIn("load_shared_resolution", source)
        self.assertIn('"current-production-resolution.json"', cleanup)
        self.assertIn("shared/Dockerfile.source-build.template", cleanup)
        for name in ("arm_patch.sh", "serve_runner.sh", "debug-init.sh"):
            self.assertTrue((SHARED / "assets/configs" / name).is_file(), name)

    def test_shared_templates_exist_for_explicit_synthetic_render(self):
        for name in ("Dockerfile.template", "Dockerfile.source-build.template",
                     "docker-compose.single.template.yaml", "docker-compose.multi.template.yaml"):
            self.assertTrue((SHARED / "templates" / name).is_file(), name)
        resolution = SHARED / "assets/current-production-resolution.json"
        self.assertTrue(resolution.is_file())
        data = json.loads(resolution.read_text(encoding="utf-8"))
        self.assertTrue(data.get("ngc_base", {}).get("tag"))
        for name in ("arm_patch.sh", "serve_runner.sh", "debug-init.sh"):
            path = SHARED / "assets/configs" / name
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o755, name)

    def test_production_canonical_cli_consumes_shared_resolution_fail_closed(self):
        with tempfile.TemporaryDirectory() as td:
            skill = Path(td) / "upstream-version-watch"
            shutil.copytree(SHARED, skill)
            manifest = Path(td) / "manifest.yaml"
            manifest.write_text("cpu_arch: aarch64\nnas_model_path: /models\n", encoding="utf-8")
            command = [
                sys.executable, str(skill / "scripts/render_dockerfile.py"),
                "--canonical-kind", "source-build", "--topology", "multi",
                "--manifest", str(manifest),
            ]
            good = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(good.returncode, 0, good.stderr.decode(errors="replace"))
            (skill / "assets/current-production-resolution.json").write_text("{}\n")
            bad = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertNotEqual(bad.returncode, 0, "normal production CLI must reject malformed canonical source")

    def test_cleanup_rejects_missing_or_malformed_canonical_resolution(self):
        spec = importlib.util.spec_from_file_location("cleanup_topology_contract", ROOT / "scripts/cleanup_docker.py")
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as td:
            setattr(module, "REPO", td)
            with self.assertRaises(RuntimeError):
                module.preserve_set()
            path = Path(td) / ".claude/skills/upstream-version-watch/assets/current-production-resolution.json"
            path.parent.mkdir(parents=True)
            path.write_text("{}\n", encoding="utf-8")
            with self.assertRaises(RuntimeError):
                module.preserve_set()

    def test_cleanup_rejects_missing_empty_or_invalid_utf8_canonical_templates(self):
        spec = importlib.util.spec_from_file_location("cleanup_template_contract", ROOT / "scripts/cleanup_docker.py")
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as td:
            repo = Path(td)
            skill = repo / ".claude/skills/upstream-version-watch"
            shutil.copytree(SHARED / "templates", skill / "templates")
            (skill / "assets").mkdir(parents=True)
            shutil.copy2(SHARED / "assets/current-production-resolution.json",
                         skill / "assets/current-production-resolution.json")
            setattr(module, "REPO", str(repo))
            module.preserve_set(canonical_only=True)
            target = skill / "templates/Dockerfile.source-build.template"
            valid = target.read_bytes()
            mutations = {
                "missing": lambda: target.unlink(),
                "empty": lambda: target.write_bytes(b""),
                "invalid-utf8": lambda: target.write_bytes(b"FROM bad\xffimage\n"),
                "no-from": lambda: target.write_text("# no canonical FROM instruction\\n", encoding="utf-8"),
            }
            for name, mutate in mutations.items():
                target.write_bytes(valid)
                mutate()
                with self.subTest(name=name), self.assertRaises(RuntimeError):
                    module.preserve_set(canonical_only=True)

    def test_executed_predicate_runtime_inputs_equal_tracked_closure(self):
        audit = r'''import json,os,runpy,sys
from pathlib import Path
root=Path.cwd().resolve(); seen=set()
def hook(event,args):
    if event != "open" or not args or not isinstance(args[0],(str,bytes,os.PathLike)):
        return
    try:
        p=Path(args[0]).resolve(); rel=p.relative_to(root).as_posix()
    except (OSError,ValueError):
        return
    if rel in {".git", ".claude/policies/tracked_index.json"}:
        return
    if not rel.startswith((".git/","__pycache__/")) and "/__pycache__/" not in rel:
        seen.add(rel)
sys.addaudithook(hook)
ns=runpy.run_path(str(root/"tests/harness/test_policy_claim_predicates.py"),run_name="policy_closure_audit")
for name,value in sorted(ns.items()):
    if name.startswith("predicate_") and callable(value):
        value()
opened=sorted(p for p in seen if (root/p).is_file())
print("__POLICY_OPENED__"+json.dumps(opened))'''
        env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
        proc = subprocess.run([sys.executable, "-B", "-c", audit], cwd=ROOT, env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr[-4000:])
        marker = next(line for line in proc.stdout.splitlines() if line.startswith("__POLICY_OPENED__"))
        opened = set(json.loads(marker.removeprefix("__POLICY_OPENED__")))
        entries = set(json.loads(INDEX.read_text(encoding="utf-8"))["entries"])
        forbidden = {p for p in opened if p.startswith(FORBIDDEN_PREFIXES)}
        self.assertEqual(forbidden, set(), "executed predicates must not open branch-local runtime roots")
        self.assertEqual(opened - entries, set(), "tracked index must cover every executed predicate input")

    def test_tracked_index_is_narrow_contract_closure(self):
        entries = json.loads(INDEX.read_text(encoding="utf-8"))["entries"]
        bad = [p for p in entries if p.startswith(FORBIDDEN_PREFIXES)]
        self.assertEqual(bad, [])
        required = set(json.loads((ROOT / ".claude/policies/evidence_manifest.json").read_text()))
        required.add(".claude/skills/upstream-version-watch/assets/current-production-resolution.json")
        required.update({
            ".claude/skills/terraforming_node/SKILL.md",
            ".claude/skills/terraforming_node/sub_node/CLAUDE.template.md",
            ".claude/skills/upstream-version-watch/SKILL.md",
            ".claude/skills/upstream-version-watch/scripts/fetch_sub_docs.sh",
            ".claude/skills/vllm-recipe-explorer/SKILL.md",
            ".claude/skills/vllm-recipe-explorer/scripts/gen_recipe_set.py",
            ".claude/skills/wiki-desk/scripts/smoke_query.py",
            ".gitignore",
            "scripts/templates/hint_recipe.template.md",
        })
        self.assertTrue(required.issubset(entries))
        self.assertLess(len(entries), 100, "whole-branch snapshots are forbidden")


if __name__ == "__main__":
    unittest.main()
