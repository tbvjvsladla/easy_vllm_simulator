"""tests/harness/test_constitution_references.py -- TDD suite for the constitution/rules ⇄
policy-registry cross-reference contract (plan_26072506 Phase 4).

Runner: stdlib `unittest` (matches the rest of tests/harness/ -- see test_completion_gate.py's
docstring for the same pytest-unavailable rationale).

Invocation:
    python3 -m unittest tests.harness.test_constitution_references -v
    python3 -m unittest discover -s tests/harness -p 'test_*.py' -v

Scope (plan_26072506 Phase 4 rules):
  - constitution (CLAUDE.md) keeps only goal/invariant/safety-boundary/trigger content -- verified
    here as a CONCRETE denylist of implementation-command/provider-CLI syntax (pip install, docker
    compose ... --profile, vllm serve, git reset/tag, ssh ... claude -p, --permission-mode), not a
    brittle prose-similarity threshold (that would over- or under-fire on paraphrase).
  - duplicated rules collapse to a single `policy:<ID>` citation; the four tracked
    constitution/rules files never restate a policy's full statement verbatim, and every citation
    they make resolves to a real, non-retired registry entry.
  - every `active` registry entry is actually cited at least once (no orphan policies) --
    otherwise the "single source of truth" registry is dead weight nobody points at.

Hermetic design: the tracked-file set is a fixed, explicit 4-path list (CLAUDE.md +
.claude/rules/{workflow,docs,references}.md) -- never `git ls-files` (a gitless clean-index
checkout export has no `.git` to query) and never a raw `Path.glob("*.md")` (would silently pick
up the maintainer-local, untracked `.claude/rules/hermes-claude-control.md` on a real machine but
not on a fresh clone -- the exact `NON_HERMETIC_TDD_FIXTURES` class of bug test_completion_gate.py
already had to fix once in this project).
"""
from __future__ import annotations

import json
import re
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
REGISTRY_PATH = REPO_ROOT / ".claude" / "policies" / "registry.yaml"

sys.path.insert(0, str(SCRIPTS_DIR))
import policy_registry as pr  # noqa: E402 -- the one shared citation broad-capture+validate implementation

# Fixed, explicit tracked-file set -- see module docstring for why this is not git ls-files / glob.
CONSTITUTION_PATH = REPO_ROOT / "CLAUDE.md"
WORKFLOW_PATH = REPO_ROOT / ".claude" / "rules" / "workflow.md"
DOCS_RULE_PATH = REPO_ROOT / ".claude" / "rules" / "docs.md"
REFERENCES_PATH = REPO_ROOT / ".claude" / "rules" / "references.md"
TRACKED_FILES = {
    "CLAUDE.md": CONSTITUTION_PATH,
    ".claude/rules/workflow.md": WORKFLOW_PATH,
    ".claude/rules/docs.md": DOCS_RULE_PATH,
    ".claude/rules/references.md": REFERENCES_PATH,
}
RULES_TIER_FILES = {k: v for k, v in TRACKED_FILES.items() if k != "CLAUDE.md"}

# Narrow, valid-uppercase-only regex -- kept ONLY as the historical example of the brittle
# blind-to-malformed-citations shape this suite no longer relies on for production validation
# (see TestReviewCycle1MalformedCitationIsDetected). Every actual citation check below uses the
# broad-capture-then-validate implementation in scripts/policy_registry.py instead
# (pr.scan_policy_citations / pr.citation_violations).
POLICY_CITATION_RE = re.compile(r"policy:([A-Z][A-Z0-9_]*)")
POLICY_ID_RE = pr.POLICY_ID_RE

# Concrete, zero-tolerance denylist for CLAUDE.md (the strictest tier -- plan §5.1). Every pattern
# here is a literal implementation-command / provider-CLI invocation, not a generic word.
CLAUDE_MD_DENYLIST = {
    "pip install":       re.compile(r"\bpip install\b"),
    "docker compose":    re.compile(r"\bdocker compose\b"),
    "vllm serve":        re.compile(r"\bvllm serve\b"),
    "git reset":         re.compile(r"\bgit reset\b"),
    "git tag":           re.compile(r"\bgit tag\b"),
    "claude -p":         re.compile(r"\bclaude -p\b"),
    "--permission-mode": re.compile(r"--permission-mode\b"),
}

# Softer-tier denylist for workflow.md/docs.md/references.md: procedural detail is legitimate
# there (that's the file's role), but bare ecosystem-tool (git) incantations and Claude-specific
# CLI syntax are not -- a named project script (e.g. `sync_to_sub.sh --apply`) is this project's
# own interface and never matches these patterns in the first place, so no separate allowlist
# carve-out logic is needed.
RULES_TIER_DENYLIST = {
    "pip install":       re.compile(r"\bpip install\b"),
    "docker compose":    re.compile(r"\bdocker compose\b"),
    "vllm serve":        re.compile(r"\bvllm serve\b"),
    "git reset":         re.compile(r"\bgit reset\b"),
    "git tag":           re.compile(r"\bgit tag\b"),
    "git remote add":    re.compile(r"\bgit remote add\b"),
    "git status":        re.compile(r"\bgit status\b"),
    "git add -A":        re.compile(r"\bgit add -A\b"),
    "git init":          re.compile(r"\bgit init\b"),
    "claude -p":         re.compile(r"\bclaude -p\b"),
    "--permission-mode": re.compile(r"--permission-mode\b"),
}


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _find_denylist_hits(text: str, denylist: dict) -> list:
    hits = []
    for label, pattern in denylist.items():
        for m in pattern.finditer(text):
            line_no = text.count("\n", 0, m.start()) + 1
            hits.append((label, line_no))
    return hits


def _tokenize(s: str) -> set:
    return set(re.findall(r"[0-9a-zA-Z_.\-가-힣]{3,}", s.lower()))


def _jaccard(a: str, b: str) -> float:
    ta, tb = _tokenize(a), _tokenize(b)
    if not ta or not tb:
        return 0.0
    return len(ta & tb) / len(ta | tb)


# A short chunk that shares only a handful of common project-wide terms (e.g. two bullets that
# both merely mention "single-node"/"multi-node"/"브랜치") can clear a ratio threshold on raw
# token overlap alone without actually restating the same rule -- a known pitfall of Jaccard on
# short strings. Requiring a minimum ABSOLUTE shared-token count alongside the ratio (empirically
# calibrated against this repo's own prose: every genuine duplicate row found during Phase 4
# migration shared >= 5 distinctive tokens; the one false positive that remained after migration,
# a terse workflow.md procedure header sharing only "single-node"/"multi-node"/"브랜치" with
# CLAUDE.md's 브랜치 bullet, shared exactly 3) filters that noise without weakening real-duplicate
# detection.
MIN_SHARED_TOKENS = 4


def _extract_chunks(text: str) -> list:
    """Splits markdown into bullet/numbered-item chunks (one comparison unit each), matching this
    project's constitution/rules writing style (dense, single-line-per-bullet, occasional wrapped
    continuation lines). Resets at blank lines, headings (#), blockquotes (>), and fences (```)."""
    chunks = []
    current = []

    def flush():
        if current:
            chunks.append(" ".join(current).strip())
            current.clear()

    for raw_line in text.split("\n"):
        stripped = raw_line.strip()
        if not stripped:
            flush()
            continue
        if stripped.startswith(("#", ">", "```")):
            flush()
            continue
        if re.match(r"^(-\s|\d+\.\s)", stripped) and current:
            flush()
        current.append(stripped)
    flush()
    return [c for c in chunks if len(c) >= 40]


DUPLICATE_PROSE_THRESHOLD = 0.3


def _load_registry():
    if not REGISTRY_PATH.is_file():
        return {"policies": []}
    with open(REGISTRY_PATH, "r", encoding="utf-8") as f:
        doc = json.load(f)
    return doc if isinstance(doc, dict) else {"policies": []}


# =============================================================================
# Section A -- fixed tracked-file set (hermetic: no git subprocess, no glob)
# =============================================================================

class TestTrackedFileEnumeration(unittest.TestCase):
    def test_exactly_four_tracked_constitution_files(self):
        self.assertEqual(set(TRACKED_FILES), {
            "CLAUDE.md", ".claude/rules/workflow.md", ".claude/rules/docs.md", ".claude/rules/references.md",
        })

    def test_each_tracked_file_exists_and_is_nonempty(self):
        for label, path in TRACKED_FILES.items():
            with self.subTest(file=label):
                self.assertTrue(path.is_file(), msg=f"{label} missing at {path}")
                self.assertGreater(path.stat().st_size, 0, msg=f"{label} is empty")


# =============================================================================
# Section B -- constitution contains only goal/invariant/safety-boundary/trigger content
# (concrete denylist, not a prose-similarity threshold)
# =============================================================================

class TestNoImplementationCommandsInConstitution(unittest.TestCase):
    def test_no_denylisted_command_syntax(self):
        text = _read(CONSTITUTION_PATH)
        hits = _find_denylist_hits(text, CLAUDE_MD_DENYLIST)
        self.assertEqual(hits, [], msg=f"CLAUDE.md contains implementation-command/provider syntax: {hits}")


class TestNoBareProviderSyntaxInRules(unittest.TestCase):
    def test_no_denylisted_command_syntax(self):
        for label, path in RULES_TIER_FILES.items():
            with self.subTest(file=label):
                text = _read(path)
                hits = _find_denylist_hits(text, RULES_TIER_DENYLIST)
                self.assertEqual(hits, [], msg=f"{label} contains bare git/provider CLI syntax: {hits}")


# =============================================================================
# Section C -- duplicated rules collapse to a single canonical statement (registry), the four
# tracked files only cite `policy:<ID>` instead of restating it
# =============================================================================

class TestNoDuplicateProse(unittest.TestCase):
    """Generic near-duplicate-paragraph scanner (token-set Jaccard overlap, not a hand-maintained
    list of specific rows) run pairwise across the 4 tracked files -- catches both today's known
    duplicate rows and any future duplication regression. Deliberately NOT difflib character-ratio:
    empirically, this project's CLAUDE.md/workflow.md pairs restate the same *rule* in structurally
    different sentences (different word order, one file terse and the other verbose), so a
    character-sequence ratio misses most real duplicates while a shared-distinctive-token overlap
    catches them; threshold calibrated empirically against this repo's own prose (see testlog)."""

    def test_no_near_duplicate_bullets_across_tracked_files(self):
        chunks_by_file = {label: _extract_chunks(_read(path)) for label, path in TRACKED_FILES.items()}
        labels = list(chunks_by_file)
        violations = []
        for i in range(len(labels)):
            for j in range(i + 1, len(labels)):
                a_label, b_label = labels[i], labels[j]
                for a in chunks_by_file[a_label]:
                    for b in chunks_by_file[b_label]:
                        score = _jaccard(a, b)
                        shared = len(_tokenize(a) & _tokenize(b))
                        if score >= DUPLICATE_PROSE_THRESHOLD and shared >= MIN_SHARED_TOKENS:
                            violations.append((round(score, 3), a_label, a[:80], b_label, b[:80]))
        violations.sort(reverse=True)
        self.assertEqual(violations, [], msg=f"near-duplicate bullets found (>= {DUPLICATE_PROSE_THRESHOLD}): "
                                              f"{violations[:10]}")


class TestNoVerbatimPolicyStatementInProse(unittest.TestCase):
    def test_registry_statement_text_not_copied_into_tracked_files(self):
        registry = _load_registry()
        combined = "\n".join(_read(p) for p in TRACKED_FILES.values())
        offenders = []
        for policy in registry.get("policies", []):
            if not isinstance(policy, dict):
                continue
            statement = (policy.get("statement") or "").strip()
            normalized = " ".join(statement.split())
            if len(normalized) < 40:
                continue
            if normalized in " ".join(combined.split()):
                offenders.append(policy.get("policy_id"))
        self.assertEqual(offenders, [], msg=f"registry statement copied verbatim into prose: {offenders}")


# =============================================================================
# Section D -- policy:<ID> citations resolve to real, non-retired entries; every active policy
# is cited at least once (no orphans)
# =============================================================================

class TestPolicyIdReferencesResolve(unittest.TestCase):
    """Uses the production broad-capture-then-validate implementation
    (pr.citation_violations), not a local valid-uppercase-only regex -- a malformed citation
    (wrong case, stray punctuation) is caught here as MALFORMED_CITATION_SYNTAX, not silently
    invisible to the scan the way the old narrow-regex-only check was
    (subagent-summary-1's brittle-test finding)."""

    def test_every_citation_is_syntactically_valid_and_resolves_to_a_live_entry(self):
        registry = _load_registry()
        all_violations = []
        for label, path in TRACKED_FILES.items():
            text = _read(path)
            all_violations.extend(pr.citation_violations(text, registry, source_label=label))
        offenders = [(v.reason_code, v.path, v.policy_id) for v in all_violations]
        self.assertEqual(offenders, [], msg=f"citations that do not validate: {offenders}")


class TestOrphanActivePolicy(unittest.TestCase):
    def test_every_active_policy_is_cited_at_least_once(self):
        registry = _load_registry()
        combined_text = "\n".join(_read(p) for p in TRACKED_FILES.values())
        # Only syntactically-valid tokens can possibly resolve to (and thus cite) a real active
        # policy -- a malformed token is reported separately by TestPolicyIdReferencesResolve.
        cited_ids = {tok for _line, tok in pr.scan_policy_citations(combined_text) if POLICY_ID_RE.match(tok)}
        orphans = [p.get("policy_id") for p in registry.get("policies", [])
                   if isinstance(p, dict) and p.get("status") == "active"
                   and p.get("policy_id") not in cited_ids]
        self.assertEqual(orphans, [], msg=f"active policies with zero citations in tracked files: {orphans}")


# =============================================================================
# Section E -- review-cycle1 remediation (subagent-summary-0/1-20260725_203232): reproduces every
# reported blocker as a permanent regression test BEFORE the fix (see
# docs/simlog/26072520_Phase4_policy_registry_TDD/cycle1_review_RED.log for the RED run these
# were first captured against). They stay in the suite after the fix as the regression lock.
# =============================================================================

MALFORMED_CITATION_FIXTURE = REPO_ROOT / "tests" / "harness" / "fixtures" / "policy_registry" / \
    "malformed_citation_snippet.txt"

# Broad capture (pr.POLICY_CITATION_BROAD_RE): any `policy:<token>` where token runs until
# whitespace or a closing/prose delimiter -- this is what actually sees a malformed citation
# like the one in subagent-summary-1's reproduction; the narrow POLICY_CITATION_RE above only
# ever matched already-valid uppercase IDs, so a malformed one was silently invisible to it
# rather than being caught and rejected. Production code lives once in
# scripts/policy_registry.py; this test module only consumes it.


class TestReviewCycle1MalformedCitationIsDetected(unittest.TestCase):
    """subagent-summary-1's brittle-test finding: a `policy:not-a-valid-id`-shaped citation must
    be actively caught, not merely fail to match a narrow valid-only regex. These are acceptance
    assertions against the actual production validator (scripts/policy_registry.py's
    scan_policy_citations / citation_violations) -- not a documentary test tied to a now-fixed
    narrow implementation that would need to keep proving its own blindness forever."""

    def test_broad_regex_finds_and_flags_both_malformed_tokens(self):
        text = MALFORMED_CITATION_FIXTURE.read_text(encoding="utf-8")
        broad_hits = pr.POLICY_CITATION_BROAD_RE.findall(text)
        malformed = [tok for tok in broad_hits if not POLICY_ID_RE.match(tok)]
        self.assertEqual(sorted(malformed), ["ALSO_BAD-ID!", "not-a-valid-id"],
                          msg=f"broad capture should have found both malformed tokens, got: {broad_hits!r}")

    def test_production_validator_flags_both_malformed_tokens_with_exact_reason_and_path(self):
        text = MALFORMED_CITATION_FIXTURE.read_text(encoding="utf-8")
        registry = _load_registry()
        violations = pr.citation_violations(text, registry, source_label="fixture:malformed_citation_snippet.txt")
        by_token = {v.policy_id: v for v in violations if v.reason_code == "MALFORMED_CITATION_SYNTAX"}
        self.assertEqual(set(by_token), {"not-a-valid-id", "ALSO_BAD-ID!"},
                          msg=f"expected both malformed tokens flagged, got: {[v.to_dict() for v in violations]}")
        for token, v in by_token.items():
            with self.subTest(token=token):
                self.assertEqual(v.reason_code, "MALFORMED_CITATION_SYNTAX")
                self.assertEqual(v.path, "fixture:malformed_citation_snippet.txt")

    def test_well_formed_but_unknown_id_is_a_distinct_unresolved_reason(self):
        # Syntactically valid (uppercase, matches the id shape) but not a real registry entry --
        # this must be a DIFFERENT reason_code than a malformed one, since the fix is different
        # (register the policy vs. fix a typo).
        registry = _load_registry()
        violations = pr.citation_violations("see (policy:TOTALLY_UNKNOWN_ID) for detail.", registry,
                                             source_label="inline")
        self.assertEqual([(v.reason_code, v.policy_id, v.path) for v in violations],
                          [("UNRESOLVED_CITATION", "TOTALLY_UNKNOWN_ID", "inline")])


class TestReviewCycle1LayerBoundaryStillViolated(unittest.TestCase):
    """subagent-summary-0 finding 3: CLAUDE.md still asserts a nonexistent Phase 6 provider-
    neutral agent-control/adapter contract, and still carries full document-naming grammar and a
    tool-inventory axis explanation -- not goal/invariant/safety-boundary/trigger content."""

    def test_no_nonexistent_agent_control_adapter_boundary_assertion(self):
        text = _read(CONSTITUTION_PATH)
        self.assertNotIn("agent-control", text.lower(),
                          msg="CLAUDE.md must not assign behavior to a provider-neutral "
                              "agent-control/adapter contract Phase 6 has not created")

    def test_no_full_document_naming_grammar(self):
        text = _read(CONSTITUTION_PATH)
        self.assertNotIn("YYMMDDHH", text,
                          msg="the full docs/<type>/<type>_<YYMMDDHH>... naming grammar belongs "
                              "in .claude/rules/docs.md only, cited by pointer from CLAUDE.md")

    def test_no_tool_inventory_axis_explanation(self):
        text = _read(CONSTITUTION_PATH)
        self.assertNotIn("두 직교 축", text,
                          msg="the skill/tool-boundary two-axis explanation is implementation "
                              "detail, not a goal/invariant/safety-boundary/trigger")


class TestReviewCycle1PolicyBulletsAreCitationOnly(unittest.TestCase):
    """subagent-summary-1 finding 2: CLAUDE.md bullets still restate a policy's rule right next
    to its `policy:<ID>` citation instead of citing it alone plus procedural routing -- e.g. the
    ARCH_WALL_VARIANT_LADDER bullet still spells out the ladder order and single-active-track
    rule verbatim next to the citation."""

    def test_arch_wall_bullet_does_not_restate_the_ladder_order(self):
        text = _read(CONSTITUTION_PATH)
        line = next((l for l in text.splitlines() if "policy:ARCH_WALL_VARIANT_LADDER" in l), None)
        self.assertIsNotNone(line, msg="policy:ARCH_WALL_VARIANT_LADDER citation not found in CLAUDE.md")
        self.assertNotIn("단일 변종-트랙", line or "",
                          msg="restates the registry statement's own ladder/single-track rule "
                              "instead of citing it alone")

    def test_host_safety_bullet_does_not_restate_the_layered_defense_list(self):
        text = _read(CONSTITUTION_PATH)
        line = next((l for l in text.splitlines() if "policy:HOST_SAFETY_LAYERED_DEFENSE" in l), None)
        self.assertIsNotNone(line, msg="policy:HOST_SAFETY_LAYERED_DEFENSE citation not found in CLAUDE.md")
        self.assertNotIn("워치독", line or "",
                          msg="restates the registry statement's own layered-defense component "
                              "list instead of citing it alone")


class TestCitationTokenizerEdgeCases(unittest.TestCase):
    """Regression lock for the citation tokenizer edge cases plan_26072506 Phase4 cycle2 called
    out as gaps: the production implementation (pr._split_trailing_punct / pr.citation_violations
    / pr.POLICY_CITATION_BROAD_RE) already handles every case below -- this class is the missing
    test coverage, not new production logic."""

    def _registry(self):
        return _load_registry()

    def test_bare_policy_colon_with_nothing_following_is_a_malformed_class_violation(self):
        violations = pr.citation_violations("see (policy:) for detail.", self._registry(), source_label="inline")
        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].reason_code, "EMPTY_POLICY_CITATION")
        self.assertEqual(violations[0].path, "inline")

    def test_embedded_xpolicy_is_never_recognized_as_a_citation_attempt(self):
        text = "xpolicy:BAD-ID should never be scanned as a citation (word-boundary guard)."
        self.assertEqual(pr.scan_policy_citations(text), [])
        self.assertEqual(pr.citation_violations(text, self._registry(), source_label="inline"), [])

    def test_trailing_sentence_punctuation_is_stripped_before_syntax_check(self):
        # policy:<KNOWN>. at the end of a sentence must resolve to KNOWN plus a punctuation
        # boundary -- NOT be reported as a malformed token just because '.' rode along in the
        # naive broad capture.
        registry = self._registry()
        known_id = registry["policies"][0]["policy_id"]
        text = f"이 규칙은 (policy:{known_id}.) 을 따른다."
        violations = pr.citation_violations(text, registry, source_label="inline")
        self.assertEqual(violations, [], msg=f"trailing '.' must not cause a malformed/unresolved "
                                              f"violation: {[v.to_dict() for v in violations]}")
        # and the broad scanner itself must still have captured the punctuation-attached token
        # (proving the fix is punctuation-STRIPPING, not a different capture regex)
        hits = pr.scan_policy_citations(text)
        self.assertEqual([tok for _line, tok in hits], [f"{known_id}."])

    def test_trailing_punctuation_does_not_rescue_a_genuinely_malformed_id(self):
        # Stripping trailing punctuation must not disguise a REAL malformation (hyphen / wrong
        # case) -- `not-a-known-id.` still fails POLICY_ID_RE after stripping the period.
        violations = pr.citation_violations("policy:not-a-known-id. 참조.", self._registry(), source_label="inline")
        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].reason_code, "MALFORMED_CITATION_SYNTAX")
        self.assertEqual(violations[0].policy_id, "not-a-known-id.")

    def test_wrong_case_and_hyphenated_ids_are_malformed_with_the_exact_source_path(self):
        registry = self._registry()
        cases = ["policy:lowercase_id", "policy:With-Hyphen", "policy:ALL-CAPS-HYPHEN"]
        for case in cases:
            with self.subTest(case=case):
                violations = pr.citation_violations(case, registry, source_label="tests/fixture/example.md")
                self.assertEqual(len(violations), 1)
                self.assertEqual(violations[0].reason_code, "MALFORMED_CITATION_SYNTAX")
                self.assertEqual(violations[0].path, "tests/fixture/example.md")

    def test_embedded_korean_before_policy_colon_is_never_recognized_as_a_citation_attempt(self):
        # cycle3 finding: "가policy:" (an embedded Korean syllable immediately before the literal,
        # not a real citation attempt) was still being scanned -- the word-boundary guard was
        # ASCII-only ([A-Za-z0-9_]), which does not exclude Hangul.
        text = "가policy:BAD-ID 는 인용이 아니다(단어경계 가드)."
        self.assertEqual(pr.scan_policy_citations(text), [])
        self.assertEqual(pr.citation_violations(text, self._registry(), source_label="inline"), [])

    def test_terminal_unicode_ellipsis_after_a_valid_id_is_stripped_as_punctuation(self):
        registry = self._registry()
        known_id = registry["policies"][0]["policy_id"]
        text = f"이 규칙은 policy:{known_id}… 을 따른다."
        violations = pr.citation_violations(text, registry, source_label="inline")
        self.assertEqual(violations, [], msg=f"trailing unicode ellipsis must not cause a malformed/unresolved "
                                              f"violation: {[v.to_dict() for v in violations]}")
        hits = pr.scan_policy_citations(text)
        self.assertEqual([tok for _line, tok in hits], [f"{known_id}…"])

    def test_terminal_ideographic_full_stop_after_a_valid_id_is_stripped_as_punctuation(self):
        registry = self._registry()
        known_id = registry["policies"][0]["policy_id"]
        text = f"policy:{known_id}。"
        violations = pr.citation_violations(text, registry, source_label="inline")
        self.assertEqual(violations, [])

    def test_unicode_trailing_punctuation_does_not_rescue_a_genuinely_malformed_id(self):
        # Stripping "…"/"。" must not disguise a REAL malformation either.
        violations = pr.citation_violations("policy:not-a-known-id…", self._registry(), source_label="inline")
        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].reason_code, "MALFORMED_CITATION_SYNTAX")
        self.assertEqual(violations[0].policy_id, "not-a-known-id…")


MIGRATED_SEMANTICS_DENYLIST_PATH = REPO_ROOT / "tests" / "harness" / "fixtures" / "policy_registry" / \
    "migrated_semantics_denylist.json"


class TestReviewCycle3MigratedSectionDenylist(unittest.TestCase):
    """subagent-summary-0 cycle3 finding 1: workflow.md restated HOST_SAFETY_LAYERED_DEFENSE's
    RAM-gate formula/drop-attempt-count/helper-absence-handling/exit-7-refusal (lines ~61/68) and
    LAST_GOOD_ROLLBACK_ANCHOR's anchor-selection/disposable/clean/topology-independence semantics
    (lines ~168-170) -- canonical policy semantics duplicated outside the registry, which the
    Jaccard-based TestNoDuplicateProse and the verbatim-statement TestNoVerbatimPolicyStatementInProse
    checks above did not catch (they compare prose chunks pairwise / whole registry statements,
    not curated policy-owned semantic fragments against every tracked file). This is a fixed,
    literal-substring denylist -- deterministic, not a similarity score -- so a future regression
    reintroducing any of these exact fragments is caught immediately."""

    def _denylist(self) -> dict:
        with open(MIGRATED_SEMANTICS_DENYLIST_PATH, "r", encoding="utf-8") as f:
            return json.load(f)

    def test_denylist_fixture_exists(self):
        self.assertTrue(MIGRATED_SEMANTICS_DENYLIST_PATH.is_file())

    def test_denylist_covers_every_canonical_policy_exactly(self):
        denylist = self._denylist()
        covered = {key for key in denylist if not key.startswith("_")}
        canonical = {policy["policy_id"] for policy in _load_registry()["policies"]}
        self.assertEqual(covered, canonical)
        for policy_id in sorted(canonical):
            with self.subTest(policy_id=policy_id):
                self.assertTrue(denylist[policy_id])

    def test_no_tracked_file_contains_a_denylisted_policy_owned_fragment(self):
        denylist = self._denylist()
        combined = {label: _read(path) for label, path in TRACKED_FILES.items()}
        offenders = []
        for policy_id, fragments in denylist.items():
            if policy_id.startswith("_"):
                continue
            for fragment in fragments:
                for label, text in combined.items():
                    if fragment in text:
                        offenders.append((policy_id, fragment, label))
        self.assertEqual(offenders, [], msg=f"policy-owned semantic fragment(s) duplicated outside the "
                                             f"registry: {offenders}")

    def test_workflow_md_still_cites_migrated_policy_pointers(self):
        # The collapse must be citation-only, not a silent deletion of the reference itself.
        text = _read(WORKFLOW_PATH)
        self.assertIn("policy:HOST_SAFETY_LAYERED_DEFENSE", text)
        self.assertIn("policy:LAST_GOOD_ROLLBACK_ANCHOR", text)
        self.assertIn("policy:MODEL_ACQUISITION_TERNARY_GATE", text)


class TestReviewCycle1NoPyYAMLDependency(unittest.TestCase):
    def test_this_module_does_not_import_yaml(self):
        # AST-level, not substring -- see scripts/policy_registry.find_banned_imports docstring
        # for why (a substring check here is self-defeating: its own msg/comment would have to
        # contain the exact banned import phrase as text).
        src = Path(__file__).read_text(encoding="utf-8")
        hits = pr.find_banned_imports(src, {"yaml"})
        self.assertEqual(hits, [], msg=f"registry.yaml must become stdlib-json-parseable so this "
                                        f"suite never needs PyYAML, but found: {hits}")


# =============================================================================
# Section F -- structural contract for CLAUDE.md itself (plan_26072506 Phase4 cycle2): size cap,
# allowed heading set, no fenced code / CLI long flags / script filenames / formula operators /
# procedural step markers / tool-inventory enumeration, and the 13 policy-boundary anchors are
# byte-exact citation-only lines. Every rule below is a fixed regex or set-membership check --
# deterministic denylist/allowlist classification, never a similarity score. TestNoDuplicateProse
# (Section C above) stays purely diagnostic and is never the authority for these assertions.
# =============================================================================

STRUCTURE_FIXTURE_PATH = REPO_ROOT / "tests" / "harness" / "fixtures" / "policy_registry" / \
    "claude_md_structure_fixture.json"

_HEADING_LINE_RE = re.compile(r"^#{1,6}\s")
_CLI_LONG_FLAG_RE = re.compile(r"(?<![A-Za-z0-9_-])--[A-Za-z][A-Za-z0-9-]*")
_SCRIPT_FILE_RE = re.compile(r"\b[\w.-]+\.(?:py|sh)\b")
_PROCEDURAL_STEP_MARKER_RE = re.compile(r"[①②③④⑤⑥⑦⑧⑨⑩]")
_NUMBERED_STEP_LINE_RE = re.compile(r"^\s*(?:\d+[.)]|Step\s+\d+)\s")
_FORMULA_OPERATOR_RE = re.compile(r"[×÷≤≥]")
_TOOL_INVENTORY_RE = re.compile(r"(?:`[a-z][a-z0-9_-]*`\s*[·,]\s*){2,}`[a-z][a-z0-9_-]*`")


def _load_structure_fixture() -> dict:
    with open(STRUCTURE_FIXTURE_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def classify_constitution_structure(text: str) -> list:
    """Deterministic structural classifier for CLAUDE.md's own goal/invariant/safety-boundary/
    trigger/policy-pointer/conditional-reference-pointer contract. Returns a list of
    (reason_code, line_no) violations -- empty means structurally compliant."""
    fixture = _load_structure_fixture()
    violations: list = []

    size = len(text.encode("utf-8"))
    if size > fixture["max_bytes"]:
        violations.append(("OVER_SIZE_LIMIT", 0))

    allowed_headings = set(fixture["allowed_headings"])
    lines = text.split("\n")
    numbered_run = 0
    for i, line in enumerate(lines, start=1):
        if "```" in line:
            violations.append(("FENCED_CODE_BLOCK", i))
        if _HEADING_LINE_RE.match(line) and line.rstrip() not in allowed_headings:
            violations.append(("DISALLOWED_HEADING", i))
        if _CLI_LONG_FLAG_RE.search(line):
            violations.append(("CLI_LONG_FLAG", i))
        if _SCRIPT_FILE_RE.search(line):
            violations.append(("SCRIPT_FILE_REFERENCE", i))
        if _PROCEDURAL_STEP_MARKER_RE.search(line):
            violations.append(("PROCEDURAL_STEP_MARKER", i))
        if _FORMULA_OPERATOR_RE.search(line):
            violations.append(("FORMULA_OPERATOR", i))
        if _TOOL_INVENTORY_RE.search(line):
            violations.append(("TOOL_INVENTORY_ENUMERATION", i))
        if _NUMBERED_STEP_LINE_RE.match(line):
            numbered_run += 1
            if numbered_run == 2:
                violations.append(("NUMBERED_STEP_BLOCK", i))
        else:
            numbered_run = 0

    return violations


class TestConstitutionStructuralContract(unittest.TestCase):
    def test_structure_fixture_exists(self):
        self.assertTrue(STRUCTURE_FIXTURE_PATH.is_file())

    def test_claude_md_is_at_most_12000_utf8_bytes(self):
        text = _read(CONSTITUTION_PATH)
        size = len(text.encode("utf-8"))
        self.assertLessEqual(size, 12000, msg=f"CLAUDE.md is {size} bytes, over the 12000 cap")

    def test_claude_md_has_zero_structural_violations(self):
        text = _read(CONSTITUTION_PATH)
        violations = classify_constitution_structure(text)
        self.assertEqual(violations, [], msg=f"CLAUDE.md fails structural classification: {violations}")

    def test_every_heading_in_claude_md_is_in_the_allowed_set(self):
        fixture = _load_structure_fixture()
        allowed = set(fixture["allowed_headings"])
        text = _read(CONSTITUTION_PATH)
        offenders = [line for line in text.split("\n")
                     if _HEADING_LINE_RE.match(line) and line.rstrip() not in allowed]
        self.assertEqual(offenders, [])

    def test_claude_md_contains_no_fenced_code_blocks(self):
        text = _read(CONSTITUTION_PATH)
        self.assertNotIn("```", text)

    def test_claude_md_contains_no_cli_long_flags(self):
        text = _read(CONSTITUTION_PATH)
        self.assertEqual(_CLI_LONG_FLAG_RE.findall(text), [])

    def test_claude_md_contains_no_script_filename_references(self):
        text = _read(CONSTITUTION_PATH)
        self.assertEqual(_SCRIPT_FILE_RE.findall(text), [])

    def test_claude_md_contains_no_procedural_step_markers_or_numbered_step_blocks(self):
        text = _read(CONSTITUTION_PATH)
        self.assertEqual(_PROCEDURAL_STEP_MARKER_RE.findall(text), [])
        run = 0
        blocks = 0
        for line in text.split("\n"):
            if _NUMBERED_STEP_LINE_RE.match(line):
                run += 1
                if run == 2:
                    blocks += 1
            else:
                run = 0
        self.assertEqual(blocks, 0, msg="CLAUDE.md must not contain a 2+ line numbered step block")

    def test_claude_md_contains_no_formula_operators_or_tool_inventory_enumeration(self):
        text = _read(CONSTITUTION_PATH)
        self.assertEqual(_FORMULA_OPERATOR_RE.findall(text), [])
        self.assertEqual(_TOOL_INVENTORY_RE.findall(text), [])

    def test_all_13_policy_boundary_anchors_appear_as_exact_citation_only_lines(self):
        fixture = _load_structure_fixture()
        text = _read(CONSTITUTION_PATH)
        lines = {line.rstrip() for line in text.split("\n")}
        missing = [line for line in fixture["policy_boundary_lines"] if line not in lines]
        self.assertEqual(missing, [], msg=f"missing exact citation-only anchor lines: {missing}")
        self.assertEqual(len(fixture["policy_boundary_lines"]), 13)

    def test_appending_a_reviewer_paraphrase_violates_the_structural_classifier(self):
        # A realistic reviewer paraphrase of a registered policy inevitably restates
        # implementation flavor (a CLI flag, a formula operator, a script filename, an
        # unregistered heading) -- exactly what this classifier exists to catch. This is the
        # regression lock against the classifier being trivially satisfiable by leaving a
        # "just prose" loophole.
        text = _read(CONSTITUTION_PATH)
        paraphrase = ("\n## 리뷰어 참고\n\n- 호스트 안전체계는 cleanup_docker.py 가 dry-run 표를 보여주고 "
                      "사람이 --apply 로 승인해야 실제 삭제가 일어난다는 뜻이다.\n")
        mutated = text + paraphrase
        violations = classify_constitution_structure(mutated)
        self.assertNotEqual(violations, [], msg="appending a reviewer paraphrase must trip the classifier")


if __name__ == "__main__":
    unittest.main()
