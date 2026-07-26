"""tests/harness/test_skill_contracts.py -- TDD suite for the 5 public skills' progressive-disclosure
contract (plan_26072506 Phase 5).

Runner: stdlib `unittest` (matches the rest of tests/harness/ -- see test_completion_gate.py's
docstring for the same pytest-unavailable rationale; `python3 -m pytest tests/harness` also collects
these because unittest.TestCase subclasses are pytest-collectable).

Invocation:
    python3 -m unittest tests.harness.test_skill_contracts -v
    python3 -m pytest tests/harness/test_skill_contracts.py -q

Scope (plan_26072506 Phase 5 "Required tests", one class per bullet):
  1. the 5 public skill names/count are preserved (no split, no rename, no 6th skill);
  2. every SKILL.md carries the SAME common contract section (plan Sec 5.2's 10 ordered labels);
  3. upstream / explorer / benchmark each keep a MANDATORY procedural spine -- and the
     low-capability-model procedural scaffold is *relocated, never deleted* (a fixed anchor list
     taken from the pre-refactor bodies must still resolve somewhere in the skill: SKILL.md or its
     conditional references/);
  4. every failure branch routes to an EXACT, existing reference/script path (no dangling target,
     no "see the docs" prose hand-wave), and no reference file is an orphan nobody links;
  5. wiki-desk is a sidecar (discovery/failure/publication) and explicitly NOT the owner of the
     benchmark report/certificate/verdict artifacts;
  6. the terraforming preflight/staleness trigger is DETERMINISTIC -- a real script with stable
     reason codes, no wall-clock read, byte-identical output for identical input;
  7. state ownership across the 5 skills has ZERO duplication (one owner per state key);
  8. Claude-provider-specific CLI syntax survives only inside the designated adapter boundary.

Hermetic design notes:
  - the skill set is a fixed explicit 5-name list, never a glob of `.claude/skills/*` (a glob would
    silently accept a 6th skill dir, which is exactly the regression bullet 1 exists to catch); the
    glob IS used, separately, to assert no EXTRA dir appeared.
  - staleness-gate behaviour is exercised against `tempfile.TemporaryDirectory()` fixtures and an
    injected `--now`, never against this repo's live manifests or the wall clock.
  - PRESERVED_SCAFFOLD_ANCHORS are literal substrings copied from the pre-Phase-5 skill bodies; they
    are the executable form of the plan's "절차적 scaffold는 삭제하지 않고 재배치한다" constraint.
"""
from __future__ import annotations

import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SKILLS_ROOT = REPO_ROOT / ".claude" / "skills"

# --------------------------------------------------------------------------------------------
# Fixed inventory (plan Sec 4.1) -- names and count are frozen by contract.
# --------------------------------------------------------------------------------------------
PUBLIC_SKILLS = (
    "terraforming_node",
    "upstream-version-watch",
    "vllm-recipe-explorer",
    "adversarial-benchmark",
    "wiki-desk",
)
# the model-serving execution chain (plan Sec 4.1/4.2): these three own the procedural scaffold
# that must be preserved as a mandatory spine + conditional references + deterministic scripts.
CHAIN_SKILLS = ("upstream-version-watch", "vllm-recipe-explorer", "adversarial-benchmark")

# plan Sec 5.2 -- the common contract every SKILL.md must expose, in this order.
CONTRACT_LABELS = (
    "Goal",
    "When to invoke",
    "Inputs",
    "Outputs",
    "Mandatory procedural spine",
    "State transitions",
    "HITL/safety boundaries",
    "Failure → reference routing",
    "Deterministic commands",
    "Handoff contract",
)
CONTRACT_HEADING = "## Contract"
SPINE_HEADING = "## Mandatory procedural spine"
ROUTING_HEADING = "## Failure → reference routing"
OWNS_LABEL = "Owns (state)"

# --------------------------------------------------------------------------------------------
# plan Sec 4.3 -- the 4 machine-judged states. SKILL.md may only describe the segment it advances.
# --------------------------------------------------------------------------------------------
STATE_NAMES = ("execution-approved", "runtime-ready", "evidence-complete", "promotion-ready")

# --------------------------------------------------------------------------------------------
# One owner per state key (plan: "skill 간 상태 ownership 중복 0").
# --------------------------------------------------------------------------------------------
REQUIRED_OWNERSHIP = {
    "terraforming_node": ("manifest.yaml", "terraforming-flag"),
    "upstream-version-watch": ("resolved.json", "image-identity"),
    "vllm-recipe-explorer": ("serving-triplet", "kv-clamp"),
    "adversarial-benchmark": ("bench-verdict", "bench-certificate"),
    "wiki-desk": ("wiki-index",),
}
# artifacts wiki-desk must NOT claim (bullet 5: sidecar, non-owner of benchmark).
BENCHMARK_OWNED_KEYS = ("bench-verdict", "bench-report", "bench-certificate")

# --------------------------------------------------------------------------------------------
# bullet 3 -- procedural scaffold that must survive the refactor SOMEWHERE in the skill
# (SKILL.md body or a conditional reference file). Literals lifted from the pre-Phase-5 bodies.
# --------------------------------------------------------------------------------------------
PRESERVED_SCAFFOLD_ANCHORS = {
    "terraforming_node": (
        "ib_write_bw",
        "show_gids",
        "emit_gate",
        "push-attestation",
        "install_host_safety.sh",
        "5-전제조건",
    ),
    "upstream-version-watch": (
        "use_existing_torch",
        "strip-hoist",
        "--no-build-isolation",
        "ray==2.48.0",
        "Application startup complete",
        "COPY build_patches/",
        "KNOWN_INCOMPAT",
        "VALIDATED_SOURCE_BUILD_KEYS",
        "docker buildx imagetools inspect",
        "--materialize-env",
    ),
    "vllm-recipe-explorer": (
        "per_token_kv_bytes",
        "--kv-cache-memory-bytes",
        "max_concurrency",
        "--moe-backend humming",
        "FLASHINFER_CUTLASS",
        "tool_chat_template_gemma4.jinja",
        "TIKTOKEN_ENCODINGS_BASE",
        "text_config",
        "vram_infeasible",
        "safetensors",
    ),
    "adversarial-benchmark": (
        "1000 / median_tpot_ms",
        "spec_decode_acceptance_length",
        "balance_dev",
        "--confirm-risk",
        "lite_burst_n",
        "NEEDS_RUBRIC",
        "realistic_fraction",
        "--e-search",
    ),
    "wiki-desk": (
        "scan_raw_copy.py",
        "realizes",
        "evidences",
        "authority",
    ),
}

# --------------------------------------------------------------------------------------------
# bullet 4 -- failure branches that must each resolve to one exact tracked target.
# --------------------------------------------------------------------------------------------
REQUIRED_ROUTING_TARGETS = {
    "terraforming_node": (
        ".claude/skills/terraforming_node/scripts/scan_node.py",
        ".claude/skills/terraforming_node/scripts/staleness_gate.py",
    ),
    "upstream-version-watch": (
        ".claude/skills/upstream-version-watch/scripts/classify_failure.py",
        ".claude/skills/upstream-version-watch/references/source-build.md",
        ".claude/skills/upstream-version-watch/references/multinode-build.md",
    ),
    "vllm-recipe-explorer": (
        ".claude/skills/vllm-recipe-explorer/scripts/sim_classify.py",
        ".claude/skills/vllm-recipe-explorer/references/kv-clamp.md",
        ".claude/skills/vllm-recipe-explorer/references/phase2-interview.md",
    ),
    "adversarial-benchmark": (
        ".claude/skills/adversarial-benchmark/scripts/verdict_rule.py",
        ".claude/skills/adversarial-benchmark/references/lite-and-publication.md",
        ".claude/skills/adversarial-benchmark/references/max-envelope.md",
    ),
    "wiki-desk": (
        ".claude/skills/wiki-desk/scripts/lint_wiki.py",
        ".claude/skills/wiki-desk/reference/MANUAL.md",
    ),
}
MIN_ROUTING_ROWS = {name: (4 if name in CHAIN_SKILLS else 2) for name in PUBLIC_SKILLS}
MIN_SPINE_STEPS = {name: (6 if name in CHAIN_SKILLS else 4) for name in PUBLIC_SKILLS}

# --------------------------------------------------------------------------------------------
# bullet 8 -- provider-specific CLI syntax. Phase 6 owns scripts/agent_control.py +
# scripts/providers/claude_code.py; Phase 5's job is to make sure the *skill execution plane*
# no longer hardcodes it. The allowlist is the designated adapter boundary:
#   - the adapter reference doc (single place a human/agent reads the Claude invocation form),
#   - terraforming_node/sub_node/** (the rendered provider payload + its transport contract; these
#     are delivery templates, not skill procedure, and Phase 6 relocates their syntax under
#     scripts/providers/),
#   - scripts/agent_control.py + scripts/providers/** once Phase 6 lands.
# --------------------------------------------------------------------------------------------
ADAPTER_ALLOWLIST_PREFIXES = (
    ".claude/skills/terraforming_node/references/agent-control-adapter.md",
    ".claude/skills/terraforming_node/sub_node/",
    "scripts/agent_control.py",
    "scripts/providers/",
)
CLAUDE_ONLY_PATTERNS = {
    "claude -p": re.compile(r"\bclaude\s+-p\b"),
    "--output-format json": re.compile(r"--output-format\s+json\b"),
    "--permission-mode": re.compile(r"--permission-mode\b"),
    "--model sonnet": re.compile(r"--model\s+sonnet\b"),
    "--dangerously-skip-permissions": re.compile(r"--dangerously-skip-permissions\b"),
    "bypassPermissions": re.compile(r"\bbypassPermissions\b"),
    "acceptEdits": re.compile(r"\bacceptEdits\b"),
}

STALENESS_GATE = SKILLS_ROOT / "terraforming_node" / "scripts" / "staleness_gate.py"
SCAN_NODE = SKILLS_ROOT / "terraforming_node" / "scripts" / "scan_node.py"
WALL_CLOCK_TOKENS = ("datetime.now", "date.today", "time.time(", "utcnow", "datetime.today")


# --------------------------------------------------------------------------------------------
# helpers
# --------------------------------------------------------------------------------------------
def skill_dir(name: str) -> Path:
    return SKILLS_ROOT / name


def skill_md_path(name: str) -> Path:
    return skill_dir(name) / "SKILL.md"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def skill_md(name: str) -> str:
    return read(skill_md_path(name))


def reference_files(name: str) -> list[Path]:
    """Conditional-reference markdown for a skill. Both spellings are accepted: wiki-desk shipped
    `reference/` long before this refactor and renaming it is out of Phase 5 scope."""
    out: list[Path] = []
    for sub in ("references", "reference"):
        d = skill_dir(name) / sub
        if d.is_dir():
            out.extend(sorted(p for p in d.rglob("*.md") if p.is_file()))
    return out


def combined_skill_text(name: str) -> str:
    """SKILL.md + every conditional reference -- the full disclosure surface of one skill."""
    return "\n".join([skill_md(name)] + [read(p) for p in reference_files(name)])


def section(body: str, heading: str) -> str:
    """Text of a `## <heading>` section, up to the next `## ` heading (exclusive). '' if absent."""
    start = body.find(heading + "\n")
    if start < 0:
        start = body.find(heading + " ")
        if start < 0:
            return ""
    rest = body[start + len(heading):]
    nxt = rest.find("\n## ")
    return rest if nxt < 0 else rest[:nxt]


def scanned_files(name: str) -> list[Path]:
    """Every file of a skill that Phase 5 governs: SKILL.md, references, scripts, root-level md."""
    d = skill_dir(name)
    out = [skill_md_path(name)]
    out.extend(reference_files(name))
    scripts = d / "scripts"
    if scripts.is_dir():
        out.extend(sorted(p for p in scripts.rglob("*") if p.is_file() and p.suffix in (".py", ".sh")))
    return out


def rel(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def import_staleness_gate():
    spec = importlib.util.spec_from_file_location("staleness_gate_under_test", STALENESS_GATE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def import_scan_node():
    spec = importlib.util.spec_from_file_location("scan_node_under_test", SCAN_NODE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def child_python() -> list[str]:
    """Preserve security-relevant interpreter flags in gate subprocesses."""
    flags = []
    if sys.flags.isolated:
        flags.append("-I")
    if sys.flags.no_site:
        flags.append("-S")
    if sys.flags.optimize:
        flags.append("-" + "O" * sys.flags.optimize)
    if sys.flags.dont_write_bytecode:
        flags.append("-B")
    return [sys.executable, *flags]


def run_gate(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [*child_python(), str(STALENESS_GATE), *args],
        capture_output=True, text=True, cwd=str(REPO_ROOT))


def write_manifest(repo: Path, topology: str, payload: str) -> Path:
    out = repo / "output" / topology
    out.mkdir(parents=True, exist_ok=True)
    p = out / "manifest.yaml"
    p.write_text(payload, encoding="utf-8")
    return p


FRESH_MANIFEST = """topology: single
cpu_arch: "aarch64"
cuda_version: "132"
gpus_per_node: 1
gpu_model: "NVIDIA GB10"
terraforming:
  complete: true
  branch_verified: true
  scanned_at: "2026072000"
model_source: managed
nas_model_path: /mnt/models
interconnect:
  type: generic-ethernet
  hca_devices: []
  gid_index: null
  socket_iface: eth0
  bandwidth_gbps: 0
  platform_preset: null
nodes: []
"""

OBSERVED_MATCHING = {
    "topology": "single",
    "cpu_arch": "aarch64",
    "cuda_version": "132",
    "gpus_per_node": 1,
    "gpu_model": "NVIDIA GB10",
    "interconnect": {
        "type": "generic-ethernet", "hca_devices": [], "gid_index": None,
        "socket_iface": "eth0", "bandwidth_gbps": 0, "platform_preset": None,
    },
    "nodes": [],
}


# --------------------------------------------------------------------------------------------
# bullet 1 -- 5 public skill names preserved
# --------------------------------------------------------------------------------------------
class TestSkillInventoryPreserved(unittest.TestCase):
    def test_exactly_the_five_named_skills_exist(self):
        found = sorted(p.name for p in SKILLS_ROOT.iterdir() if p.is_dir())
        self.assertEqual(sorted(PUBLIC_SKILLS), found,
                         msg="skill count/name drift: Phase 5 must neither split, rename nor add a skill")

    def test_each_skill_has_a_skill_md(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                self.assertTrue(skill_md_path(name).is_file(), msg=f"missing {skill_md_path(name)}")

    def test_frontmatter_name_matches_directory_name(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                body = skill_md(name)
                self.assertTrue(body.startswith("---\n"), msg=f"{name}: SKILL.md must open with frontmatter")
                fm = body.split("---", 2)[1]
                m = re.search(r"^name:\s*(\S+)\s*$", fm, re.M)
                self.assertIsNotNone(m, msg=f"{name}: frontmatter has no `name:`")
                self.assertEqual(name, m.group(1))

    def test_frontmatter_keeps_a_description_trigger_surface(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                fm = skill_md(name).split("---", 2)[1]
                self.assertRegex(fm, re.compile(r"^description:", re.M),
                                 msg=f"{name}: description (invocation trigger) removed")


# --------------------------------------------------------------------------------------------
# bullet 2 -- common contract section
# --------------------------------------------------------------------------------------------
class TestCommonContractSection(unittest.TestCase):
    def test_every_skill_has_a_contract_section(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                self.assertNotEqual("", section(skill_md(name), CONTRACT_HEADING),
                                    msg=f"{name}: missing `{CONTRACT_HEADING}` section")

    def test_contract_labels_present_and_in_canonical_order(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                sec = section(skill_md(name), CONTRACT_HEADING)
                positions = []
                for label in CONTRACT_LABELS:
                    token = f"**{label}**"
                    idx = sec.find(token)
                    self.assertGreaterEqual(idx, 0, msg=f"{name}: contract label {token} missing")
                    positions.append(idx)
                self.assertEqual(sorted(positions), positions,
                                 msg=f"{name}: contract labels out of canonical order {CONTRACT_LABELS}")

    def test_state_transitions_line_uses_the_canonical_state_vocabulary(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                sec = section(skill_md(name), CONTRACT_HEADING)
                start = sec.index("**State transitions**")
                line = sec[start:sec.find("\n", start)]
                self.assertTrue(any(s in line for s in STATE_NAMES),
                                msg=f"{name}: State transitions must name at least one of {STATE_NAMES}")

    def test_contract_section_is_compact(self):
        """The contract is a routing header, not a second copy of the procedure."""
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                sec = section(skill_md(name), CONTRACT_HEADING)
                self.assertLessEqual(len(sec.splitlines()), 40,
                                     msg=f"{name}: contract section bloated back into prose")


# --------------------------------------------------------------------------------------------
# bullet 3 -- mandatory spine + scaffold relocation (never deletion)
# --------------------------------------------------------------------------------------------
class TestMandatoryProceduralSpine(unittest.TestCase):
    def test_every_skill_declares_a_mandatory_spine_section(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                self.assertNotEqual("", section(skill_md(name), SPINE_HEADING),
                                    msg=f"{name}: missing `{SPINE_HEADING}` section")

    def test_spine_is_an_ordered_step_list(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                sec = section(skill_md(name), SPINE_HEADING)
                steps = re.findall(r"^\s*(\d+)\.\s+\S", sec, re.M)
                self.assertGreaterEqual(len(steps), MIN_SPINE_STEPS[name],
                                        msg=f"{name}: spine has {len(steps)} ordered steps, "
                                            f"needs >= {MIN_SPINE_STEPS[name]}")
                self.assertEqual([str(i + 1) for i in range(len(steps))], steps,
                                 msg=f"{name}: spine steps must be numbered 1..N (order is load-bearing)")

    def test_chain_skills_mark_their_spine_as_non_skippable(self):
        for name in CHAIN_SKILLS:
            with self.subTest(skill=name):
                sec = section(skill_md(name), SPINE_HEADING)
                self.assertRegex(sec, r"(생략 금지|건너뛰지|필수 순서)",
                                 msg=f"{name}: the spine must state that steps are not skippable")

    def test_low_capability_procedural_scaffold_is_relocated_not_deleted(self):
        for name, anchors in PRESERVED_SCAFFOLD_ANCHORS.items():
            text = combined_skill_text(name)
            for anchor in anchors:
                with self.subTest(skill=name, anchor=anchor):
                    self.assertIn(anchor, text,
                                  msg=f"{name}: procedural anchor {anchor!r} vanished -- Phase 5 relocates "
                                      f"scaffold into references/, it never deletes it")

    def test_chain_skills_actually_moved_detail_into_conditional_references(self):
        for name in CHAIN_SKILLS:
            with self.subTest(skill=name):
                refs = [p for p in reference_files(name)]
                self.assertGreaterEqual(len(refs), 2,
                                        msg=f"{name}: expected >= 2 conditional reference files")

    def test_chain_skill_md_bodies_are_thinner_than_their_reference_corpus(self):
        """Progressive disclosure: the always-loaded SKILL.md must not remain the fat one."""
        for name in CHAIN_SKILLS:
            with self.subTest(skill=name):
                body_lines = len(skill_md(name).splitlines())
                ref_lines = sum(len(read(p).splitlines()) for p in reference_files(name))
                self.assertLess(body_lines, ref_lines,
                                msg=f"{name}: SKILL.md {body_lines} lines vs references {ref_lines} -- "
                                    f"detail was not actually disclosed conditionally")


# --------------------------------------------------------------------------------------------
# bullet 4 -- failure branch -> exact reference/script routing
# --------------------------------------------------------------------------------------------
def routing_rows(name: str) -> list[tuple[str, str]]:
    sec = section(skill_md(name), ROUTING_HEADING)
    rows = []
    for line in sec.splitlines():
        line = line.strip()
        if not line.startswith("|") or set(line) <= set("|- :"):
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) < 2 or cells[0].startswith("실패") or cells[0].lower().startswith("failure"):
            continue
        rows.append((cells[0], cells[-1]))
    return rows


PATHLIKE = re.compile(r"^(?:\.claude|scripts|tests)/[\w./+-]+$")


class TestFailureReferenceRouting(unittest.TestCase):
    def test_every_skill_has_a_routing_table_with_enough_branches(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                rows = routing_rows(name)
                self.assertGreaterEqual(len(rows), MIN_ROUTING_ROWS[name],
                                        msg=f"{name}: routing table has {len(rows)} branches, "
                                            f"needs >= {MIN_ROUTING_ROWS[name]}")

    def test_every_routing_target_is_an_existing_repo_path(self):
        for name in PUBLIC_SKILLS:
            rows = routing_rows(name)
            for signal, target in rows:
                with self.subTest(skill=name, signal=signal):
                    toks = [t for t in re.findall(r"`([^`]+)`", target) if PATHLIKE.match(t.split("#")[0])]
                    self.assertTrue(toks,
                                    msg=f"{name}: branch {signal!r} routes to prose, not an exact path")
                    for tok in toks:
                        p = REPO_ROOT / tok.split("#")[0]
                        self.assertTrue(p.exists(), msg=f"{name}: dangling routing target {tok!r}")

    def test_required_failure_branches_are_wired(self):
        for name, targets in REQUIRED_ROUTING_TARGETS.items():
            table = "\n".join(f"{a}|{b}" for a, b in routing_rows(name))
            for target in targets:
                with self.subTest(skill=name, target=target):
                    self.assertIn(target, table, msg=f"{name}: no failure branch routes to {target}")

    def test_no_orphan_reference_file(self):
        for name in PUBLIC_SKILLS:
            body = skill_md(name)
            for p in reference_files(name):
                with self.subTest(skill=name, ref=rel(p)):
                    self.assertTrue(rel(p) in body or p.name in body,
                                    msg=f"{name}: {rel(p)} is never linked from SKILL.md (orphan)")


# --------------------------------------------------------------------------------------------
# bullet 5 -- wiki-desk is a sidecar, not the benchmark owner
# --------------------------------------------------------------------------------------------
class TestWikiSidecarNonOwner(unittest.TestCase):
    def test_wiki_declares_itself_a_sidecar(self):
        body = skill_md("wiki-desk")
        self.assertIn("sidecar", body.lower(), msg="wiki-desk must declare its sidecar role")

    def test_wiki_declares_the_three_sidecar_attachment_points(self):
        body = skill_md("wiki-desk").lower()
        for point in ("discovery", "failure", "publication"):
            with self.subTest(point=point):
                self.assertIn(point, body,
                              msg=f"wiki-desk must attach at {point} (plan Sec 4.1)")

    def test_wiki_is_not_in_the_execution_chain(self):
        body = skill_md("wiki-desk")
        sec = section(body, CONTRACT_HEADING)
        self.assertRegex(sec, r"(sidecar|비-소유|non-owner)",
                         msg="wiki-desk's contract must mark it out-of-chain")

    def test_wiki_owns_no_benchmark_artifact(self):
        owned = owns_keys("wiki-desk")
        for key in BENCHMARK_OWNED_KEYS:
            with self.subTest(key=key):
                self.assertNotIn(key, owned, msg=f"wiki-desk must not own {key}")

    def test_wiki_names_adversarial_benchmark_as_the_benchmark_owner(self):
        body = skill_md("wiki-desk")
        self.assertIn("adversarial-benchmark", body,
                      msg="wiki-desk must point at the real benchmark owner")
        self.assertRegex(
            body,
            r"(?is)(benchmark report, certificate and verdict are owned by\s+`?adversarial-benchmark`?|"
            r"adversarial-benchmark\s+(소유|owns)\s+.{0,120}(report|certificate|verdict))",
            msg="wiki-desk must state explicitly that benchmark artifacts are owned elsewhere")

    def test_benchmark_artifacts_are_owned_by_adversarial_benchmark(self):
        owned = owns_keys("adversarial-benchmark")
        for key in BENCHMARK_OWNED_KEYS:
            with self.subTest(key=key):
                self.assertIn(key, owned, msg=f"adversarial-benchmark must own {key}")


# --------------------------------------------------------------------------------------------
# bullet 6 -- deterministic terraforming staleness trigger
# --------------------------------------------------------------------------------------------
class TestTerraformingStalenessTriggerIsDeterministic(unittest.TestCase):
    def test_gate_script_exists(self):
        self.assertTrue(STALENESS_GATE.is_file(), msg=f"missing {STALENESS_GATE}")

    def test_gate_exposes_stable_reason_codes(self):
        mod = import_staleness_gate()
        for code in ("MANIFEST_ABSENT", "FLAG_ABSENT", "HW_DRIFT", "ATTESTATION_STALE", "FRESH"):
            with self.subTest(code=code):
                self.assertIn(code, mod.REASON_CODES)

    def test_scan_node_emits_the_canonical_hw_fields_consumed_by_staleness_gate(self):
        scan = import_scan_node()
        scan.scan_cpu_arch = lambda: "aarch64"
        scan.scan_cuda_version = lambda: "132"
        scan.scan_gpus_per_node = lambda: 1
        scan.scan_gpu_model = lambda: "NVIDIA GB10"
        scan.cross_validate = lambda _ic, _compose: {"ok": True}
        result = scan.build_local_scan_result(
            {"type": "generic-ethernet", "hca_devices": [], "gid_index": None,
             "socket_iface": "eth0", "bandwidth_gbps": 0, "platform_preset": None,
             "_probe": "detail"}, compose="compose.yaml", topology="single")
        for field in ("topology_declared", "cpu_arch", "cuda_version",
                      "gpus_per_node", "gpu_model", "interconnect"):
            with self.subTest(field=field):
                self.assertIn(field, result)
        self.assertEqual("NVIDIA GB10", result["gpu_model"])
        self.assertEqual("single", result["topology_declared"])
        self.assertNotIn("_probe", result["interconnect"])
        self.assertEqual("detail", result["scan_detail"]["_probe"])

        gate = import_staleness_gate()
        manifest = dict(OBSERVED_MATCHING)
        manifest["terraforming"] = {
            "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
        evaluated = gate.evaluate(
            manifest, result, now=gate.parse_stamp("2026-07-25"), expected_topology="single")
        self.assertEqual(["FRESH"], evaluated["reasons"])
        self.assertFalse(evaluated["preflight_required"])

    def test_emitted_manifest_preserves_gpu_model_and_round_trips_fresh(self):
        scan = import_scan_node()
        scan.scan_cpu_arch = lambda: "aarch64"
        scan.scan_cuda_version = lambda: "132"
        scan.scan_gpus_per_node = lambda: 1
        scan.scan_gpu_model = lambda: "NVIDIA GB10"
        scan.scan_infiniband_hcas = lambda: []
        scan.scan_roce_v2_gids = lambda: []
        scan.cross_validate = lambda _ic, _compose: {"ok": True}
        observed = scan.build_local_scan_result(
            scan.detect_interconnect(), compose="compose.yaml", topology="single")
        block = scan.emit_manifest_block(observed)
        self.assertIn('gpu_model: "NVIDIA GB10"', block)

        import yaml
        manifest = yaml.safe_load(block)
        gate = import_staleness_gate()
        evaluated = gate.evaluate(
            manifest, observed, now=gate.parse_stamp(manifest["terraforming"]["scanned_at"]),
            expected_topology="single")
        self.assertEqual(["FRESH"], evaluated["reasons"])

    def test_multi_hca_gid_index_list_round_trips_through_emitter_and_gate(self):
        scan = import_scan_node()
        scan.scan_cpu_arch = lambda: "aarch64"
        scan.scan_cuda_version = lambda: "132"
        scan.scan_gpus_per_node = lambda: 1
        scan.scan_gpu_model = lambda: "NVIDIA GB10"
        scan.scan_infiniband_hcas = lambda: ["roce0", "roce1"]
        scan.scan_roce_v2_gids = lambda: [
            {"hca": "roce0", "gid_index": 1, "ipv4": "192.0.2.1", "iface": "eth0"},
            {"hca": "roce1", "gid_index": 3, "ipv4": "192.0.2.2", "iface": "eth1"},
        ]
        scan.cross_validate = lambda _ic, _compose: {"ok": True}
        observed = scan.build_local_scan_result(
            scan.detect_interconnect(), compose="compose.yaml", topology="multi")
        self.assertEqual([1, 3], observed["interconnect"]["gid_index"])
        import yaml
        manifest = yaml.safe_load(scan.emit_manifest_block(observed))
        manifest["nodes"] = [
            {"role": "main", "host": "node-a"},
            {"role": "sub", "host": "node-b"},
        ]
        gate = import_staleness_gate()
        result = gate.evaluate(
            manifest, observed, now=gate.parse_stamp(manifest["terraforming"]["scanned_at"]),
            expected_topology="multi")
        self.assertEqual(["FRESH"], result["reasons"])

        for malformed in ([1], [3, 1], [1, 1], [1, True]):
            with self.subTest(malformed=malformed):
                bad_manifest = dict(manifest)
                bad_observed = dict(observed)
                bad_manifest["interconnect"] = dict(
                    manifest["interconnect"], gid_index=malformed)
                bad_observed["interconnect"] = dict(
                    observed["interconnect"], gid_index=malformed)
                rejected = gate.evaluate(
                    bad_manifest, bad_observed,
                    now=gate.parse_stamp(manifest["terraforming"]["scanned_at"]),
                    expected_topology="multi")
                self.assertNotEqual(["FRESH"], rejected["reasons"])
                self.assertTrue(rejected["preflight_required"])

    def test_unknown_or_malformed_mirrored_hardware_facts_never_report_fresh(self):
        gate = import_staleness_gate()
        base_manifest = dict(OBSERVED_MATCHING)
        base_manifest["terraforming"] = {
            "complete": True, "branch_verified": True, "scanned_at": "2026072000"}

        cases = []
        for value in (None, "", True):
            manifest = dict(base_manifest, gpu_model=value)
            observed = dict(OBSERVED_MATCHING, gpu_model=value)
            cases.append(("gpu_model=%r" % value, manifest, observed))
        missing_manifest = dict(base_manifest)
        missing_observed = dict(OBSERVED_MATCHING)
        missing_manifest.pop("gpu_model")
        missing_observed.pop("gpu_model")
        cases.append(("gpu_model missing", missing_manifest, missing_observed))

        for field, value in (("type", {}), ("hca_devices", [{}])):
            manifest = dict(base_manifest)
            observed = dict(OBSERVED_MATCHING)
            manifest["interconnect"] = dict(base_manifest["interconnect"], **{field: value})
            observed["interconnect"] = dict(OBSERVED_MATCHING["interconnect"], **{field: value})
            cases.append(("interconnect.%s" % field, manifest, observed))

        manifest = dict(base_manifest, nodes=[{"role": [], "host": "main"}])
        observed = dict(OBSERVED_MATCHING, nodes=[{"role": [], "host": "main"}])
        cases.append(("nodes[0].role", manifest, observed))

        for label, manifest, observed in cases:
            with self.subTest(case=label):
                result = gate.evaluate(
                    manifest, observed, now=gate.parse_stamp("2026-07-25"),
                    expected_topology="single")
                self.assertNotEqual(["FRESH"], result["reasons"])
                self.assertTrue(result["preflight_required"])
                self.assertTrue(set(result["reasons"]) & {"MANIFEST_INVALID", "OBSERVED_INVALID"})

    def test_all_canonical_interconnect_fields_participate_in_drift(self):
        gate = import_staleness_gate()
        manifest = dict(OBSERVED_MATCHING)
        manifest["terraforming"] = {
            "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
        mutations = {
            "type": "RoCE v2",
            "hca_devices": ["rocep1s0f1"],
            "gid_index": 3,
            "socket_iface": "eth1",
            "bandwidth_gbps": 100,
            "platform_preset": "dgx-spark-gb10",
        }
        for field, changed in mutations.items():
            with self.subTest(field=field):
                observed = dict(OBSERVED_MATCHING)
                if field == "type":
                    observed["interconnect"] = dict(
                        OBSERVED_MATCHING["interconnect"], type="RoCE v2",
                        hca_devices=["rocep1s0f1"], gid_index=3, socket_iface="eth0")
                else:
                    observed["interconnect"] = dict(
                        OBSERVED_MATCHING["interconnect"], **{field: changed})
                result = gate.evaluate(
                    manifest, observed, now=gate.parse_stamp("2026-07-25"),
                    expected_topology="single")
                self.assertEqual(["HW_DRIFT"], result["reasons"])
                self.assertIn("interconnect.%s" % field, result["drift_fields"])

    def test_incomplete_scan_facts_cannot_authorize_complete_manifest_emission(self):
        scan = import_scan_node()
        incomplete = {
            "topology_declared": "single", "cpu_arch": "aarch64",
            "cuda_version": None, "gpus_per_node": None, "gpu_model": None,
            "interconnect": {
                "type": "generic-ethernet", "hca_devices": [], "gid_index": None,
                "socket_iface": None, "bandwidth_gbps": None, "platform_preset": None,
            },
        }
        self.assertTrue(scan.emission_blockers(incomplete))
        complete = dict(incomplete, cuda_version="132", gpus_per_node=1,
                        gpu_model="NVIDIA GB10")
        self.assertEqual([], scan.emission_blockers(complete))
        for bad_bandwidth in (float("nan"), float("inf"), -1):
            with self.subTest(bandwidth=bad_bandwidth):
                candidate = dict(complete)
                candidate["interconnect"] = dict(
                    complete["interconnect"], bandwidth_gbps=bad_bandwidth)
                self.assertIn("interconnect.bandwidth_gbps",
                              scan.emission_blockers(candidate))
        malformed_interconnect = {
            "type": "bogus",
            "hca_devices": ["bad/hca"],
            "gid_index": "oops",
            "socket_iface": "bad/iface",
            "bandwidth_gbps": 1.0,
            "platform_preset": "bad preset",
        }
        blockers = scan.emission_blockers(dict(complete, interconnect=malformed_interconnect))
        for field in ("type", "hca_devices", "gid_index", "socket_iface", "platform_preset"):
            self.assertIn("interconnect." + field, blockers)
        valid_multi = dict(
            complete, topology_declared="multi",
            interconnect=dict(complete["interconnect"], type="RoCE v2",
                              hca_devices=["roce0"], gid_index=3,
                              socket_iface="eth0"),
            nodes=[{"role": "main", "host": "node-a"},
                   {"role": "sub", "host": "node-b"}])
        self.assertEqual([], scan.emission_blockers(valid_multi))
        bad_host = dict(valid_multi, nodes=[
            {"role": "main", "host": "node-a"},
            {"role": "sub", "host": "bad host"}])
        self.assertIn("nodes[1].host", scan.emission_blockers(bad_host))
        for malformed_ip in ("1.2.3.4%zone", "fe80::1%"):
            with self.subTest(malformed_ip=malformed_ip):
                bad_ip = dict(valid_multi, nodes=[
                    {"role": "main", "host": "node-a"},
                    {"role": "sub", "host": malformed_ip}])
                self.assertIn("nodes[1].host", scan.emission_blockers(bad_ip))
        scoped_ipv6 = dict(valid_multi, nodes=[
            {"role": "main", "host": "node-a"},
            {"role": "sub", "host": "fe80::1%eth0"}])
        self.assertNotIn("nodes[1].host", scan.emission_blockers(scoped_ipv6))
        bad_role = dict(valid_multi, nodes=valid_multi["nodes"] + [
            {"role": "leader", "host": "node-c"}])
        self.assertIn("nodes[2].role", scan.emission_blockers(bad_role))
        for field, value in (("cpu_arch", "bad arch"), ("cuda_version", "latest")):
            with self.subTest(field=field):
                self.assertIn(field, scan.emission_blockers(dict(complete, **{field: value})))
        malformed_roce = dict(
            valid_multi,
            interconnect=dict(valid_multi["interconnect"], hca_devices=[],
                              gid_index=None, socket_iface=None))
        roce_blockers = scan.emission_blockers(malformed_roce)
        self.assertIn("interconnect.hca_devices", roce_blockers)
        self.assertIn("interconnect.gid_index", roce_blockers)
        self.assertIn("interconnect.socket_iface", roce_blockers)

    def test_roce_identity_requires_hca_gid_and_socket_on_both_documents(self):
        gate = import_staleness_gate()
        malformed_ic = dict(
            OBSERVED_MATCHING["interconnect"], type="RoCE v2",
            hca_devices=[], gid_index=None, socket_iface=None)
        manifest = dict(OBSERVED_MATCHING, topology="multi", interconnect=malformed_ic)
        manifest["terraforming"] = {
            "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
        manifest["nodes"] = [
            {"role": "main", "host": "node-a"},
            {"role": "sub", "host": "node-b"},
        ]
        observed = dict(OBSERVED_MATCHING, topology="multi", interconnect=malformed_ic)
        observed.pop("nodes", None)
        result = gate.evaluate(manifest, observed, expected_topology="multi")
        self.assertNotEqual(["FRESH"], result["reasons"])
        self.assertTrue(result["preflight_required"])

    def test_mixed_malformed_gid_scan_is_structured_and_never_raises(self):
        scan = import_scan_node()
        scan.scan_infiniband_hcas = lambda: ["roce0", "roce1"]
        scan.scan_roce_v2_gids = lambda: [
            {"hca": "roce0", "gid_index": 3, "ipv4": "192.0.2.1", "iface": "eth0"},
            {"hca": "roce1", "gid_index": "oops", "ipv4": "192.0.2.2", "iface": "eth1"},
        ]
        interconnect = scan.detect_interconnect()
        self.assertEqual("invalid", interconnect["gid_index"])
        result = {
            "topology_declared": "multi", "cpu_arch": "aarch64",
            "cuda_version": "132", "gpus_per_node": 1, "gpu_model": "NVIDIA GB10",
            "interconnect": interconnect,
            "nodes": [{"role": "main", "host": "node-a"},
                      {"role": "sub", "host": "node-b"}],
        }
        self.assertIn("interconnect.gid_index", scan.emission_blockers(result))

    def test_mirrored_scalar_grammar_and_nonfinite_numbers_never_report_fresh(self):
        gate = import_staleness_gate()
        cases = []
        for field, value in (("cpu_arch", "bad arch"), ("cuda_version", "latest")):
            manifest = dict(OBSERVED_MATCHING, **{field: value})
            observed = dict(OBSERVED_MATCHING, **{field: value})
            cases.append((field, manifest, observed))
        for field, value in (
                ("type", "bogus"), ("hca_devices", ["bad/hca"]),
                ("socket_iface", "bad iface"), ("bandwidth_gbps", float("nan")),
                ("bandwidth_gbps", float("inf")), ("platform_preset", "bad preset!")):
            manifest = dict(OBSERVED_MATCHING)
            observed = dict(OBSERVED_MATCHING)
            manifest["interconnect"] = dict(OBSERVED_MATCHING["interconnect"], **{field: value})
            observed["interconnect"] = dict(OBSERVED_MATCHING["interconnect"], **{field: value})
            cases.append(("interconnect.%s" % field, manifest, observed))
        for label, manifest, observed in cases:
            with self.subTest(case=label):
                manifest["terraforming"] = {
                    "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
                result = gate.evaluate(manifest, observed, expected_topology="single")
                self.assertNotEqual(["FRESH"], result["reasons"])
                self.assertTrue(result["preflight_required"])

    def test_multi_ready_requires_an_explicit_reachable_peer(self):
        scan = import_scan_node()
        _assertion, gate, exit_code = scan.evaluate_gate(
            declared="multi", ic_present=True, peer_given=False, peer_reachable=False,
            bandwidth=200, bw_floor=180, branch="multi-node", branch_topo="multi",
            mani_topo="multi", model_env_ok=True)
        self.assertNotEqual(0, exit_code)
        self.assertEqual("blocked", gate["status"])
        self.assertNotEqual("multi-ready", gate["branch"])

    def test_nonfinite_bandwidth_or_floor_never_authorizes_multi_ready(self):
        scan = import_scan_node()
        for bandwidth, floor in ((1.0, float("nan")), (float("nan"), 1.0),
                                 (1.0, float("inf")), (float("inf"), 1.0)):
            with self.subTest(bandwidth=bandwidth, floor=floor):
                _assertion, gate, code = scan.evaluate_gate(
                    declared="multi", ic_present=True, peer_given=True,
                    peer_reachable=True, bandwidth=bandwidth, bw_floor=floor)
                self.assertEqual(2, code)
                self.assertEqual("blocked", gate["status"])

    def test_multi_manifest_requires_authoritative_main_and_sub_roster(self):
        gate = import_staleness_gate()
        for nodes in (None, [], [{"role": "main", "host": "node-a"}],
                      [{"role": "sub", "host": "node-b"}]):
            with self.subTest(nodes=nodes):
                manifest = dict(OBSERVED_MATCHING, topology="multi")
                manifest["terraforming"] = {
                    "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
                if nodes is None:
                    manifest.pop("nodes", None)
                else:
                    manifest["nodes"] = nodes
                observed = dict(OBSERVED_MATCHING, topology="multi")
                observed.pop("nodes", None)
                result = gate.evaluate(
                    manifest, observed, expected_topology="multi")
                self.assertEqual(["MANIFEST_INVALID"], result["reasons"])
                self.assertTrue(result["preflight_required"])

    def test_attestation_timestamp_is_required_and_grammar_closed(self):
        gate = import_staleness_gate()
        for value in (None, "", "not-a-stamp", ["2026072000"]):
            with self.subTest(value=value):
                manifest = dict(OBSERVED_MATCHING)
                manifest["terraforming"] = {
                    "complete": True, "branch_verified": True, "scanned_at": value}
                result = gate.evaluate(
                    manifest, OBSERVED_MATCHING, now=gate.parse_stamp("2026-07-25"),
                    expected_topology="single")
                self.assertEqual(["MANIFEST_INVALID"], result["reasons"])
                self.assertIn("terraforming.scanned_at", result["invalid_fields"])
                self.assertTrue(result["preflight_required"])
        for padded in (" 2026072000", "2026072000 ", "\t2026072000"):
            with self.subTest(padded=padded):
                manifest = dict(OBSERVED_MATCHING)
                manifest["terraforming"] = {
                    "complete": True, "branch_verified": True, "scanned_at": padded}
                result = gate.evaluate(manifest, dict(OBSERVED_MATCHING))
                self.assertEqual(["MANIFEST_INVALID"], result["reasons"])

    def test_single_manifest_roster_is_dormant_and_host_tokens_are_closed(self):
        gate = import_staleness_gate()
        single = dict(OBSERVED_MATCHING)
        single["terraforming"] = {
            "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
        single["nodes"] = [{"role": "main", "host": "node-a"}]
        result = gate.evaluate(single, dict(OBSERVED_MATCHING), expected_topology="single")
        self.assertEqual(["MANIFEST_INVALID"], result["reasons"])
        for bad_host in ("bad host", "bad/host", "\t", ":", "---", "bad..host",
                         "1.2.3.4%zone", "fe80::1%"):
            with self.subTest(bad_host=bad_host):
                multi = dict(OBSERVED_MATCHING, topology="multi")
                multi["terraforming"] = {
                    "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
                multi["nodes"] = [
                    {"role": "main", "host": "node-a"},
                    {"role": "sub", "host": bad_host},
                ]
                observed = dict(OBSERVED_MATCHING, topology="multi")
                observed.pop("nodes", None)
                rejected = gate.evaluate(multi, observed, expected_topology="multi")
                self.assertEqual(["MANIFEST_INVALID"], rejected["reasons"])
        valid_scoped = dict(OBSERVED_MATCHING, topology="multi")
        valid_scoped["terraforming"] = {
            "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
        valid_scoped["nodes"] = [
            {"role": "main", "host": "node-a"},
            {"role": "sub", "host": "fe80::1%eth0"},
        ]
        observed = dict(OBSERVED_MATCHING, topology="multi")
        observed.pop("nodes", None)
        accepted = gate.evaluate(valid_scoped, observed, expected_topology="multi")
        self.assertEqual(["FRESH"], accepted["reasons"])

    def test_timestamp_parser_rejects_trailing_garbage_and_future_order(self):
        gate = import_staleness_gate()
        for value in ("2026-07-25garbage", "20260725garbage", "2026-07-25T99:99:99"):
            with self.subTest(value=value):
                self.assertIsNone(gate.parse_stamp(value))
        manifest = dict(OBSERVED_MATCHING)
        manifest["terraforming"] = {
            "complete": True, "branch_verified": True, "scanned_at": "9999-01-01"}
        result = gate.evaluate(
            manifest, OBSERVED_MATCHING, now=gate.parse_stamp("2026-07-25"),
            expected_topology="single")
        self.assertEqual(["MANIFEST_INVALID"], result["reasons"])
        self.assertTrue(result["preflight_required"])

    def test_malformed_observed_json_is_structured_when_json_requested(self):
        mod = import_staleness_gate()
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            observed = repo / "observed.json"
            observed.write_text("{not-json", encoding="utf-8")
            cp = run_gate("--topology", "single", "--repo", str(repo),
                          "--observed", str(observed), "--json")
            self.assertEqual(mod.EXIT_USAGE, cp.returncode)
            doc = json.loads(cp.stdout)
            self.assertEqual(["OBSERVED_INVALID"], doc["reasons"])
            self.assertTrue(doc["preflight_required"])
            self.assertNotIn("Traceback", cp.stderr)

    def test_observed_missing_path_and_directory_are_structured_in_json_mode(self):
        mod = import_staleness_gate()
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            for target in (repo / "missing.json", repo):
                with self.subTest(target=str(target)):
                    cp = run_gate("--topology", "single", "--repo", str(repo),
                                  "--observed", str(target), "--json")
                    self.assertEqual(mod.EXIT_USAGE, cp.returncode)
                    doc = json.loads(cp.stdout)
                    self.assertEqual(["OBSERVED_INVALID"], doc["reasons"])
                    self.assertTrue(doc["preflight_required"])

    def test_standalone_no_site_mode_restores_only_the_declared_yaml_dependency(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            env = os.environ.copy()
            env.pop("PYTHONPATH", None)
            cp = subprocess.run(
                [sys.executable, "-S", str(STALENESS_GATE), "--topology", "single",
                 "--repo", str(repo), "--now", "2026-07-25", "--json"],
                capture_output=True, text=True, cwd=str(REPO_ROOT), env=env)
            self.assertEqual(0, cp.returncode, msg=cp.stdout + cp.stderr)
            self.assertEqual(["FRESH"], json.loads(cp.stdout)["reasons"])

    def test_hca_names_and_node_roles_are_grammar_closed(self):
        gate = import_staleness_gate()
        cases = []
        for hcas in ([""], ["roce ok"]):
            manifest = dict(OBSERVED_MATCHING)
            observed = dict(OBSERVED_MATCHING)
            manifest["interconnect"] = dict(OBSERVED_MATCHING["interconnect"], hca_devices=hcas)
            observed["interconnect"] = dict(OBSERVED_MATCHING["interconnect"], hca_devices=hcas)
            cases.append(("hca", manifest, observed))
        for role in ("leader", "", ["main"]):
            node = {"role": role, "host": "node-a"}
            manifest = dict(OBSERVED_MATCHING, nodes=[node])
            observed = dict(OBSERVED_MATCHING, nodes=[node])
            cases.append(("role", manifest, observed))
        for label, manifest, observed in cases:
            with self.subTest(case=label):
                manifest["terraforming"] = {
                    "complete": True, "branch_verified": True, "scanned_at": "2026072000"}
                result = gate.evaluate(manifest, observed, expected_topology="single")
                self.assertNotEqual(["FRESH"], result["reasons"])
                self.assertTrue(result["preflight_required"])

    def test_multi_scan_without_manifest_roster_does_not_create_permanent_nodes_drift(self):
        scan = import_scan_node()
        scan.scan_cpu_arch = lambda: "aarch64"
        scan.scan_cuda_version = lambda: "132"
        scan.scan_gpus_per_node = lambda: 1
        scan.scan_gpu_model = lambda: "NVIDIA GB10"
        scan.cross_validate = lambda _ic, _compose: {"ok": True}
        observed = scan.build_local_scan_result(
            {"type": "RoCE v2", "hca_devices": ["rocep1s0f1"], "gid_index": 3,
             "socket_iface": "eth0", "bandwidth_gbps": 100,
             "platform_preset": "dgx-spark-gb10"},
            compose="compose.yaml", topology="multi")
        manifest = {
            "topology": "multi", "cpu_arch": "aarch64", "cuda_version": "132",
            "gpus_per_node": 1, "gpu_model": "NVIDIA GB10",
            "interconnect": {"type": "RoCE v2", "hca_devices": ["rocep1s0f1"],
                             "gid_index": 3, "socket_iface": "eth0",
                             "bandwidth_gbps": 100,
                             "platform_preset": "dgx-spark-gb10"},
            "nodes": [{"role": "main", "host": "192.0.2.10"},
                      {"role": "sub", "host": "192.0.2.11"}],
            "terraforming": {"complete": True, "branch_verified": True,
                              "scanned_at": "2026072000"},
        }
        gate = import_staleness_gate()
        evaluated = gate.evaluate(
            manifest, observed, now=gate.parse_stamp("2026-07-25"), expected_topology="multi")
        self.assertEqual(["FRESH"], evaluated["reasons"])
        self.assertNotIn("nodes.count", evaluated["drift_fields"])

        observed_with_roster = dict(observed)
        observed_with_roster["nodes"] = [
            {"role": "main", "host": "192.0.2.10"},
            {"role": "sub", "host": "192.0.2.99"},
        ]
        drifted = gate.evaluate(
            manifest, observed_with_roster, now=gate.parse_stamp("2026-07-25"),
            expected_topology="multi")
        self.assertEqual(["HW_DRIFT"], drifted["reasons"])
        self.assertIn("nodes[1].host", drifted["drift_fields"])

    def test_gate_never_reads_the_wall_clock(self):
        src = read(STALENESS_GATE)
        for token in WALL_CLOCK_TOKENS:
            with self.subTest(token=token):
                self.assertNotIn(token, src,
                                 msg=f"staleness gate must take --now, never read {token}")

    def test_gate_self_test_passes(self):
        cp = run_gate("--self-test")
        self.assertEqual(0, cp.returncode, msg=cp.stdout + cp.stderr)

    def test_identical_input_yields_byte_identical_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            obs = repo / "observed.json"
            obs.write_text(json.dumps(OBSERVED_MATCHING), encoding="utf-8")
            runs = [run_gate("--topology", "single", "--repo", str(repo), "--observed", str(obs),
                             "--now", "2026-07-25", "--json") for _ in range(2)]
            self.assertEqual(runs[0].stdout, runs[1].stdout)
            self.assertEqual(runs[0].returncode, runs[1].returncode)

    def test_fresh_manifest_requires_no_preflight(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            obs = repo / "observed.json"
            obs.write_text(json.dumps(OBSERVED_MATCHING), encoding="utf-8")
            cp = run_gate("--topology", "single", "--repo", str(repo), "--observed", str(obs),
                          "--now", "2026-07-25", "--json")
            self.assertEqual(0, cp.returncode, msg=cp.stdout + cp.stderr)
            doc = json.loads(cp.stdout)
            self.assertFalse(doc["preflight_required"])
            self.assertEqual(["FRESH"], doc["reasons"])

    def test_missing_manifest_requires_preflight(self):
        mod = import_staleness_gate()
        with tempfile.TemporaryDirectory() as tmp:
            cp = run_gate("--topology", "single", "--repo", tmp, "--now", "2026-07-25", "--json")
            self.assertEqual(mod.EXIT_PREFLIGHT_REQUIRED, cp.returncode, msg=cp.stdout + cp.stderr)
            doc = json.loads(cp.stdout)
            self.assertTrue(doc["preflight_required"])
            self.assertIn("MANIFEST_ABSENT", doc["reasons"])

    def test_flag_absent_requires_preflight(self):
        mod = import_staleness_gate()
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST.replace("complete: true", "complete: false"))
            cp = run_gate("--topology", "single", "--repo", str(repo), "--now", "2026-07-25", "--json")
            self.assertEqual(mod.EXIT_PREFLIGHT_REQUIRED, cp.returncode)
            self.assertIn("FLAG_ABSENT", json.loads(cp.stdout)["reasons"])

    def test_hardware_drift_requires_preflight(self):
        mod = import_staleness_gate()
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            obs = repo / "observed.json"
            drifted = dict(OBSERVED_MATCHING, gpus_per_node=2)
            obs.write_text(json.dumps(drifted), encoding="utf-8")
            cp = run_gate("--topology", "single", "--repo", str(repo), "--observed", str(obs),
                          "--now", "2026-07-25", "--json")
            self.assertEqual(mod.EXIT_PREFLIGHT_REQUIRED, cp.returncode)
            doc = json.loads(cp.stdout)
            self.assertIn("HW_DRIFT", doc["reasons"])
            self.assertIn("gpus_per_node", json.dumps(doc, ensure_ascii=False))

    def test_attestation_age_beyond_threshold_requires_preflight(self):
        mod = import_staleness_gate()
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            obs = repo / "observed.json"
            obs.write_text(json.dumps(OBSERVED_MATCHING), encoding="utf-8")
            cp = run_gate("--topology", "single", "--repo", str(repo), "--observed", str(obs),
                          "--now", "2027-07-25", "--json")
            self.assertEqual(mod.EXIT_PREFLIGHT_REQUIRED, cp.returncode)
            self.assertIn("ATTESTATION_STALE", json.loads(cp.stdout)["reasons"])

    def test_skipped_axes_are_reported_honestly(self):
        """No --now / no --observed must be *declared as skipped*, never silently treated fresh."""
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST)
            cp = run_gate("--topology", "single", "--repo", str(repo), "--json")
            doc = json.loads(cp.stdout)
            self.assertEqual("skipped:no-now", doc["age_check"])
            self.assertEqual("skipped:no-observed", doc["hw_check"])

    def test_malformed_manifest_roots_fail_closed_without_traceback(self):
        mod = import_staleness_gate()
        for payload in ("[]\n", "broken\n", "7\n", "null\n"):
            with self.subTest(payload=payload.strip()), tempfile.TemporaryDirectory() as tmp:
                repo = Path(tmp)
                write_manifest(repo, "single", payload)
                cp = run_gate("--topology", "single", "--repo", str(repo), "--json")
                self.assertEqual(mod.EXIT_PREFLIGHT_REQUIRED, cp.returncode, msg=cp.stdout + cp.stderr)
                self.assertNotIn("Traceback", cp.stderr)
                doc = json.loads(cp.stdout)
                self.assertTrue(doc["preflight_required"])
                self.assertIn("MANIFEST_INVALID", doc["reasons"])

    def test_malformed_observed_roots_are_structured_usage_errors(self):
        mod = import_staleness_gate()
        for payload in ([], "broken", 7, None):
            with self.subTest(payload=payload), tempfile.TemporaryDirectory() as tmp:
                repo = Path(tmp)
                write_manifest(repo, "single", FRESH_MANIFEST)
                observed = repo / "observed.json"
                observed.write_text(json.dumps(payload), encoding="utf-8")
                cp = run_gate("--topology", "single", "--repo", str(repo),
                              "--observed", str(observed), "--json")
                self.assertEqual(mod.EXIT_USAGE, cp.returncode, msg=cp.stdout + cp.stderr)
                self.assertNotIn("Traceback", cp.stderr)
                doc = json.loads(cp.stdout)
                self.assertIn("OBSERVED_INVALID", doc["reasons"])

    def test_malformed_nested_shapes_never_report_fresh(self):
        mod = import_staleness_gate()
        replacements = (
            ("interconnect:\n  type: generic-ethernet\n  hca_devices: []\n  gid_index: null\n  socket_iface: eth0\n  bandwidth_gbps: 0\n  platform_preset: null", "interconnect: broken"),
            ("nodes: []", "nodes: [broken]"),
            ("terraforming:\n  complete: true\n  branch_verified: true\n  scanned_at: \"2026072000\"", "terraforming: broken"),
        )
        for old, new in replacements:
            with self.subTest(replacement=new), tempfile.TemporaryDirectory() as tmp:
                repo = Path(tmp)
                write_manifest(repo, "single", FRESH_MANIFEST.replace(old, new))
                cp = run_gate("--topology", "single", "--repo", str(repo), "--json")
                self.assertEqual(mod.EXIT_PREFLIGHT_REQUIRED, cp.returncode, msg=cp.stdout + cp.stderr)
                self.assertNotIn("Traceback", cp.stderr)
                doc = json.loads(cp.stdout)
                self.assertIn("MANIFEST_INVALID", doc["reasons"])
                self.assertNotEqual(["FRESH"], doc["reasons"])

    def test_manifest_topology_must_match_selected_topology(self):
        mod = import_staleness_gate()
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            write_manifest(repo, "single", FRESH_MANIFEST.replace("topology: single", "topology: multi"))
            cp = run_gate("--topology", "single", "--repo", str(repo), "--json")
            self.assertEqual(mod.EXIT_PREFLIGHT_REQUIRED, cp.returncode, msg=cp.stdout + cp.stderr)
            doc = json.loads(cp.stdout)
            self.assertIn("MANIFEST_INVALID", doc["reasons"])
            self.assertIn("topology", doc["invalid_fields"])

    def test_skill_md_documents_the_gate_as_the_deterministic_trigger(self):
        body = skill_md("terraforming_node")
        self.assertIn("staleness_gate.py", body,
                      msg="terraforming_node/SKILL.md must name the deterministic staleness gate")
        sec = section(body, CONTRACT_HEADING)
        start = sec.index("**Deterministic commands**")
        self.assertIn("staleness_gate.py", sec[start:start + 400],
                      msg="the gate must appear under the contract's Deterministic commands")

    def test_skill_md_states_the_conditional_preflight_rule(self):
        body = skill_md("terraforming_node")
        self.assertRegex(body, r"(조건부 preflight|staleness)",
                         msg="terraforming must be documented as a conditional preflight, not an always-run step")


# --------------------------------------------------------------------------------------------
# bullet 7 -- state ownership, zero duplication
# --------------------------------------------------------------------------------------------
def owns_keys(name: str) -> list[str]:
    sec = section(skill_md(name), CONTRACT_HEADING)
    idx = sec.find(f"**{OWNS_LABEL}**")
    if idx < 0:
        return []
    line = sec[idx:sec.find("\n", idx)]
    return re.findall(r"`([^`]+)`", line)


class TestStateOwnershipHasNoDuplication(unittest.TestCase):
    def test_every_skill_declares_owned_state(self):
        for name in PUBLIC_SKILLS:
            with self.subTest(skill=name):
                self.assertTrue(owns_keys(name),
                                msg=f"{name}: contract must carry an `**{OWNS_LABEL}**` line")

    def test_no_state_key_has_two_owners(self):
        seen: dict[str, str] = {}
        dupes = []
        for name in PUBLIC_SKILLS:
            for key in owns_keys(name):
                if key in seen:
                    dupes.append(f"{key}: {seen[key]} and {name}")
                seen[key] = name
        self.assertEqual([], dupes, msg=f"duplicated state ownership: {dupes}")

    def test_required_ownership_assignments(self):
        for name, keys in REQUIRED_OWNERSHIP.items():
            owned = owns_keys(name)
            for key in keys:
                with self.subTest(skill=name, key=key):
                    self.assertIn(key, owned, msg=f"{name} must own {key}")

    def test_non_owners_do_not_reclaim_a_foreign_key(self):
        owners = {k: n for n in PUBLIC_SKILLS for k in owns_keys(n)}
        for name in PUBLIC_SKILLS:
            for key, owner in owners.items():
                if owner == name:
                    continue
                with self.subTest(skill=name, key=key):
                    self.assertNotIn(key, owns_keys(name))


# --------------------------------------------------------------------------------------------
# bullet 8 -- Claude-specific commands live only inside the adapter boundary
# --------------------------------------------------------------------------------------------
class TestClaudeCommandsIsolatedToAdapter(unittest.TestCase):
    def _violations(self):
        out = []
        for name in PUBLIC_SKILLS:
            for path in scanned_files(name):
                relpath = rel(path)
                if relpath.startswith(ADAPTER_ALLOWLIST_PREFIXES):
                    continue
                text = read(path)
                for label, rx in CLAUDE_ONLY_PATTERNS.items():
                    if rx.search(text):
                        out.append(f"{relpath}: {label}")
        return out

    def test_no_provider_specific_command_outside_the_adapter(self):
        self.assertEqual([], self._violations(),
                         msg="Claude-only CLI syntax must live only in the adapter boundary "
                             f"{ADAPTER_ALLOWLIST_PREFIXES}")

    def test_adapter_boundary_document_exists(self):
        p = REPO_ROOT / ADAPTER_ALLOWLIST_PREFIXES[0]
        self.assertTrue(p.is_file(), msg=f"missing adapter boundary doc {p}")

    def test_adapter_document_actually_holds_the_provider_syntax(self):
        """The syntax must be relocated, not erased -- the sub-agent handshake still has to work."""
        text = read(REPO_ROOT / ADAPTER_ALLOWLIST_PREFIXES[0])
        self.assertRegex(text, CLAUDE_ONLY_PATTERNS["claude -p"],
                         msg="adapter doc must carry the concrete provider invocation")

    def test_skills_that_delegate_link_the_adapter_instead_of_inlining_it(self):
        for name in ("terraforming_node", "upstream-version-watch"):
            with self.subTest(skill=name):
                self.assertIn("agent-control-adapter.md", combined_skill_text(name),
                              msg=f"{name}: sub-agent delegation must point at the adapter boundary")

    def test_adapter_document_is_marked_provider_scoped(self):
        text = read(REPO_ROOT / ADAPTER_ALLOWLIST_PREFIXES[0])
        self.assertRegex(text, r"(provider|어댑터|adapter)",
                         msg="adapter doc must declare itself the provider-scoped boundary")


if __name__ == "__main__":
    unittest.main(verbosity=2)
