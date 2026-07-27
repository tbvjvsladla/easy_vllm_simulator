"""tests/harness/test_context_budget.py -- TDD suite for the Phase 10 context-budget correction
cycle.

Baseline (pre-correction, exact HEAD 0fa8874512348842e4512a840e2197e3f32646f2): an independent
audit found the always-loaded CLAUDE.md + all tracked .claude/rules/*.md (4 files: CLAUDE.md,
workflow.md, docs.md, references.md) totalled 27,212 anthropic-tokenizer tokens across 610 lines
/ 38,828 UTF-8 characters, and failed 33/48 audit checks because .claude/rules/references.md
(rare/situational external-reference material) was loaded unconditionally alongside genuinely
always-true rules, and workflow.md/docs.md read as an implementation encyclopedia rather than a
compact contract.

This suite proves the correction structurally and mechanically:
  1. .claude/rules/references.md no longer exists (removed from the unconditional rules tier).
  2. its procedural/source-verification content survived, relocated to an on-demand owner
     reference (.claude/skills/wiki-desk/reference/references.md), not silently deleted.
  3. every current governing pointer (recipe.py's runtime path constant, and every tracked file's
     prose) resolves to the new location -- no dangling `.claude/rules/references.md` mentions.
  4. the always-loaded set (CLAUDE.md + .claude/rules/workflow.md + .claude/rules/docs.md) is
     under a reproducible line ceiling and a reproducible char/token-proxy ceiling, calibrated
     against the known baseline data point above (no anthropic tokenizer library is installed in
     this environment, so a literal token count is not obtainable here -- the proxy is a
     deterministic, reproducible stand-in, documented as such).
  5. the "standard path" (the same always-loaded set) is reduced by at least 40% against the
     27,212-token baseline.
  6. the five public skills are untouched (still exactly 5 SKILL.md files) and specific
     procedural anchors (HITL-gate-before-commit ordering, the redirect-template fragment, the
     arch-wall no-skip ladder, docs.md's naming grammar and the sub-recovery-is-docs-only phrase)
     remain byte-present, in order, after the rewrite.
  7. the previously nested "exception to the exception" report/ prose in docs.md is gone,
     replaced by a flat table -- without deleting the semantic fact that report/ is the one
     tracked docs/ exception.

Runner: stdlib `unittest`.
"""
from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
SKILLS_DIR = REPO_ROOT / ".claude" / "skills"

sys.path.insert(0, str(SCRIPTS_DIR))
import policy_registry as pr  # noqa: E402

CLAUDE_MD = REPO_ROOT / "CLAUDE.md"
WORKFLOW_MD = REPO_ROOT / ".claude" / "rules" / "workflow.md"
DOCS_MD = REPO_ROOT / ".claude" / "rules" / "docs.md"
OLD_REFERENCES_MD = REPO_ROOT / ".claude" / "rules" / "references.md"
NEW_REFERENCES_MD = SKILLS_DIR / "wiki-desk" / "reference" / "references.md"

ALWAYS_LOADED = [CLAUDE_MD, WORKFLOW_MD, DOCS_MD]

# Baseline data point, given verbatim by the pre-correction independent audit -- NOT recomputed
# here (the 4-file pre-correction tree this was measured against no longer exists post-correction,
# since references.md is relocated and workflow.md/docs.md are rewritten). Held as a fixed
# historical constant purely to calibrate/scale the reproducible char-based token proxy below.
BASELINE_TOTAL_TOKENS = 27212
BASELINE_TOTAL_CHARS = 38828  # CLAUDE.md(4599) + workflow.md(13926) + docs.md(13208) + references.md(7095)
BASELINE_TOTAL_LINES = 610

TOKENS_PER_CHAR = BASELINE_TOTAL_TOKENS / BASELINE_TOTAL_CHARS

TOKEN_PROXY_CEILING = 10000
LINE_CEILING = 500
MIN_REDUCTION_FRACTION = 0.40


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def estimate_tokens(text: str) -> float:
    """Deterministic, reproducible char-count-based token-count proxy. Not a real BPE tokenizer
    (none is installed in this environment) -- calibrated with one multiplier (TOKENS_PER_CHAR)
    derived from the single known ground-truth data point above (27,212 anthropic-tokenizer
    tokens for the exact pre-correction 4-file, 38,828-character baseline). Pure function of
    `text`'s length: same input always yields the same output, so this ceiling is reproducible
    across machines/runs without any external tokenizer dependency."""
    return len(text) * TOKENS_PER_CHAR


class TestReferencesRelocated(unittest.TestCase):
    """references.md is removed from .claude/rules (no longer unconditional) and its content
    survives at an on-demand owner reference -- not silently deleted."""

    def test_old_rules_references_path_no_longer_exists(self):
        self.assertFalse(OLD_REFERENCES_MD.exists(),
                          msg=".claude/rules/references.md must be removed from the always-loaded "
                              "rules tier")

    def test_new_on_demand_reference_exists_and_is_nonempty(self):
        self.assertTrue(NEW_REFERENCES_MD.is_file(),
                         msg=f"relocated on-demand reference missing at {NEW_REFERENCES_MD}")
        self.assertGreater(NEW_REFERENCES_MD.stat().st_size, 0)

    def test_new_reference_preserves_the_source_verification_sections(self):
        text = _read(NEW_REFERENCES_MD)
        # Semantic-preservation spot-check: the ID->URL normalization templates, the HW-scope GPU
        # table (recipe.py's runtime dependency, see TestGpuLookupPointerResolves below), and the
        # negative-honesty minimum-scope recipe must all still be present -- not silently dropped.
        for marker in (
            "ID→URL 정규화",
            "### NVIDIA GB10",
            "### NVIDIA RTX PRO 6000",
            "per-card VRAM (GiB): 96",
            "부정판정 최소범위 레시피",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, text, msg=f"relocated reference lost section marker {marker!r}")

    def test_new_reference_is_owned_by_a_skill_not_a_new_sixth_skill(self):
        # The relocation must not create a new tracked skill directory -- it must land inside an
        # EXISTING skill's own reference/ tree (wiki-desk), preserving the five-public-skill
        # cardinality (see TestFivePublicSkillsPreserved).
        self.assertEqual(NEW_REFERENCES_MD.parent, SKILLS_DIR / "wiki-desk" / "reference")


class TestGovernedPointersResolve(unittest.TestCase):
    """Every current governing pointer that used to name .claude/rules/references.md must resolve
    to the new location -- no dangling full-path references left anywhere in tracked content."""

    def test_recipe_py_runtime_constant_points_at_the_new_location_and_resolves(self):
        skills_sys_path = str(SKILLS_DIR / "vllm-recipe-explorer")
        sys.path.insert(0, skills_sys_path)
        try:
            import recipe  # noqa: PLC0415 -- deliberately lazy, isolated import
            self.assertEqual(Path(recipe.REFERENCES_MD_PATH), NEW_REFERENCES_MD)
            self.assertTrue(Path(recipe.REFERENCES_MD_PATH).is_file())
        finally:
            sys.path.remove(skills_sys_path)
            sys.modules.pop("recipe", None)

    def test_no_current_governing_consumer_contains_a_dangling_full_path_mention(self):
        # Historical tests/ledgers may name the removed path while describing the migration. Scan
        # the live consumer surfaces instead: always-loaded prose, public skill contracts, and
        # production Python. This catches an actionable dangling pointer without self-matching this
        # test's own negative-control literal after it enters the tracked index.
        offenders = []
        candidates = set(ALWAYS_LOADED)
        candidates.update(SKILLS_DIR.rglob("*.md"))
        candidates.update(SKILLS_DIR.rglob("*.py"))
        candidates.update(SCRIPTS_DIR.rglob("*.py"))
        for full in sorted(candidates):
            if not full.is_file():
                continue
            try:
                text = full.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            if ".claude/rules/references.md" in text:
                offenders.append(full.relative_to(REPO_ROOT).as_posix())
        self.assertEqual(offenders, [], msg=f"dangling '.claude/rules/references.md' path mention(s) "
                                             f"in tracked files: {offenders}")

    def test_claude_md_conditional_reference_section_names_the_new_path(self):
        text = _read(CLAUDE_MD)
        self.assertIn(".claude/skills/wiki-desk/reference/references.md", text)


class TestAlwaysLoadedBudget(unittest.TestCase):
    """The always-loaded set (CLAUDE.md + workflow.md + docs.md) must be materially smaller than
    the pre-correction 4-file baseline, under both a line ceiling and a reproducible token-count
    proxy ceiling."""

    def test_combined_line_count_is_under_ceiling(self):
        total_lines = sum(len(_read(p).splitlines()) for p in ALWAYS_LOADED)
        self.assertLess(total_lines, LINE_CEILING,
                         msg=f"always-loaded combined line count {total_lines} >= {LINE_CEILING}")

    def test_combined_token_proxy_is_under_ceiling(self):
        combined = "".join(_read(p) for p in ALWAYS_LOADED)
        proxy = estimate_tokens(combined)
        self.assertLessEqual(proxy, TOKEN_PROXY_CEILING,
                              msg=f"always-loaded token proxy {proxy:.0f} > {TOKEN_PROXY_CEILING}")

    def test_standard_path_is_reduced_by_at_least_40_percent_vs_baseline(self):
        combined = "".join(_read(p) for p in ALWAYS_LOADED)
        proxy = estimate_tokens(combined)
        reduction = 1 - (proxy / BASELINE_TOTAL_TOKENS)
        self.assertGreaterEqual(reduction, MIN_REDUCTION_FRACTION,
                                 msg=f"standard-path reduction {reduction:.1%} < {MIN_REDUCTION_FRACTION:.0%} "
                                     f"(proxy={proxy:.0f}, baseline={BASELINE_TOTAL_TOKENS})")

    def test_claude_md_alone_still_respects_the_existing_12000_byte_cap(self):
        # Documentary cross-check only -- the authoritative assertion lives in
        # test_constitution_references.py's TestConstitutionStructuralContract; this just proves
        # the two budgets are not in tension.
        size = len(_read(CLAUDE_MD).encode("utf-8"))
        self.assertLessEqual(size, 12000)


class TestFivePublicSkillsPreserved(unittest.TestCase):
    EXPECTED_SKILLS = {
        "adversarial-benchmark", "terraforming_node", "upstream-version-watch",
        "vllm-recipe-explorer", "wiki-desk",
    }

    def test_exactly_five_skill_md_files_exist(self):
        found = {p.parent.name for p in SKILLS_DIR.glob("*/SKILL.md")}
        self.assertEqual(found, self.EXPECTED_SKILLS)


class TestProceduralAnchorsPreserved(unittest.TestCase):
    """Spot-checks for the specific procedural anchors this rewrite must not lose (a superset of
    these are separately, exhaustively enforced by test_policy_claim_predicates.py's real
    predicate functions and test_constitution_references.py's structural classifier -- this class
    exists so the budget-correction intent and its concrete evidence live in one place)."""

    def test_workflow_smoke_before_commit_gate_precedes_s4_and_both_are_present(self):
        text = _read(WORKFLOW_MD)
        gate = "HITL 게이트 ③ : 스모크 결과(+분류·risk-memo)를 사람이 확인 (smoke-before-commit)"
        s4 = "S4 commit   → 스모크 통과분만 로컬 last-good 커밋"
        self.assertIn(gate, text)
        self.assertIn(s4, text)
        self.assertLess(text.index(gate), text.index(s4))

    def test_workflow_redirect_template_fragment_present(self):
        text = _read(WORKFLOW_MD)
        fragment = ("① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? 컨테이너 임시 다운로드(컨테이너 down 시 삭제)? "
                    "특정 경로 저장·마운트?)을 먼저 정합시다.")
        self.assertIn(fragment, text)

    def test_workflow_arch_wall_ladder_and_ordered_tokens_present(self):
        text = _read(WORKFLOW_MD)
        ladder = "deps-패치 → 소스-게이트 패치 → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체"
        self.assertIn(ladder, text)
        anchor = text.index("stock-구조적-불가")
        ordered = ("참조-그라운디드 확증", "testlog 기록 + 사람 승인", "source_build_variants",
                   "Dockerfile build-arg 파라미터화", "clean 빌드(양노드) → 스모크")
        positions = [text.index(token, anchor) for token in ordered]
        self.assertEqual(positions, sorted(positions))

    def test_workflow_still_cites_the_three_migrated_policy_pointers(self):
        text = _read(WORKFLOW_MD)
        for token in ("policy:HOST_SAFETY_LAYERED_DEFENSE", "policy:LAST_GOOD_ROLLBACK_ANCHOR",
                      "policy:MODEL_ACQUISITION_TERNARY_GATE"):
            with self.subTest(token=token):
                self.assertIn(token, text)

    def test_docs_sub_recovery_is_docs_only_phrase_present(self):
        text = _read(DOCS_MD)
        self.assertIn("**상향 회수(서브→메인) = 문서기반 only**", text)

    def test_docs_naming_grammar_semantics_unchanged(self):
        text = _read(DOCS_MD)
        for marker in ("YYMMDDHH", "_MM_SS", "bench_report_", "benchmark_"):
            with self.subTest(marker=marker):
                self.assertIn(marker, text)


class TestNoNestedExceptionProseRegression(unittest.TestCase):
    """docs.md's report/ carve-out used to be explained as a nested "exception to the exception"
    (a rule against per-folder gitignore carve-outs, immediately followed by prose justifying why
    report/ is exempt from THAT rule). The correction flattens this into one allowlist table --
    this locks the flattening in without losing the underlying fact (report/ is still the one
    tracked docs/ exception)."""

    OLD_NESTED_EXCEPTION_FRAGMENTS = (
        "바로 위 \"전용 규칙 추가 금지\"의 명시",
        "그 금지는 산출물을 *무시*하려는 폴더",
    )

    def test_old_nested_exception_justification_prose_is_gone(self):
        text = _read(DOCS_MD)
        for fragment in self.OLD_NESTED_EXCEPTION_FRAGMENTS:
            with self.subTest(fragment=fragment):
                self.assertNotIn(fragment, text)

    def test_report_tracked_exception_fact_still_present(self):
        text = _read(DOCS_MD)
        self.assertIn("report", text.lower())
        self.assertTrue(
            re.search(r"report.{0,40}(추적|tracked)", text) or re.search(r"(추적|tracked).{0,40}report", text),
            msg="docs.md must still state report/ is the tracked docs/ exception somewhere")


if __name__ == "__main__":
    unittest.main()
