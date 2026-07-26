#!/usr/bin/env python3
"""policy_registry.py -- stdlib-only validator/auditor for .claude/policies/registry.yaml
(plan_26072506 Phase 4, review-cycle2 remediation -- subagent-summary-0/1-20260725_213652).

`registry.yaml` is plain JSON text (a valid YAML 1.2 subset) -- this module, and every consumer
in this repo, parses it with the stdlib `json` module only. PyYAML is not a dependency anywhere
in this project.

Design (cycle2):
  - Schema-shape validation reuses scripts/completion_gate.py's existing generic Draft-07-subset
    engine (validate_against_schema) against .claude/schemas/policy-registry.schema.json -- no
    second, hand-duplicated shape checker.
  - Every cross-field / regex-shaped rule the schema engine cannot express lives here, as small
    composable check functions, each returning a list of Violation records (reason_code +
    message + optional policy_id/clause_id/path) -- never a bare bool.
  - No wall-clock access anywhere in this module -- `evaluate_lifecycle` / `cmd_verify` /
    `cmd_check_removal` take an explicit `as_of: datetime.date` (CLI: --as-of YYYY-MM-DD,
    required, no default). Deterministic "is this due/overdue/unreproduced/future today" is a
    pure function of that explicit input, never datetime.date.today().
  - TWO INDEPENDENT 90-day clocks (cycle2 fix -- these were conflated in cycle1):
      (a) reproduction-staleness clock: origin = last_reproduced if set, else added_at. Governs
          UNREPRODUCED_90_DAY_NOT_CANDIDATE (an `active` policy unreproduced/never-reproduced for
          >= 90 days from that origin must be `candidate`). A policy added_at == as_of with
          last_reproduced == null is NOT stale (0 elapsed days) -- cycle1's bug flagged it
          immediately regardless of added_at, because it used "last_reproduced is None" as an
          unconditional stale signal instead of measuring elapsed days from added_at.
      (b) review clock: last_reviewed_at -> next_review_due (== last_reviewed_at + 90d). Governs
          review.state (current/due/overdue) purely via as_of vs next_review_due -- decoupled
          from (a). A policy reviewed today (last_reviewed_at == as_of) is genuinely `current`
          for review purposes even if it is `candidate` (not yet reproduced) for status purposes
          -- these are orthogonal facts about the same record, not the same fact spelled twice.
  - Date chronology (cycle2 fix -- entirely absent in cycle1): added_at must itself be a valid
    ISO-8601 date; it, last_reproduced, and review.last_reviewed_at must each be <= the explicit
    as_of (no future dates); and added_at must be <= last_reproduced (when set) and <=
    review.last_reviewed_at (a policy cannot have been reproduced or reviewed before it was
    added).
  - Retirement/removal metadata closure (cycle2 fix): active/candidate entries may not carry
    retirement_reason, superseded_by, OR removal_evidence -- cycle1 only checked the first two,
    so an `active` policy self-declaring `removal_evidence.status: contract_e2e_green` was
    accepted with zero violations.
  - Evidence tracking is an EXACT, DIGEST-LOCKED MANIFEST (.claude/policies/evidence_manifest.json
    -- cycle2 replaces cycle1's prefix-allowlist, which let any path merely *starting with* a
    tracked prefix, e.g. `scripts/untracked_probe.py`, resolve as if it were a real, reviewed
    evidence file). A path is admissible only if: it is a non-absolute, non-escaping (no `..`
    component), EXACT key in the manifest; no path component (including the leaf) is a symlink
    (checked component-by-component from repo_root, fail-closed on any stat error); the file
    exists; and its current sha256 matches the manifest's recorded digest (a tampered-after-
    review file is caught, not silently trusted because the path still resolves).
  - Evidence RELEVANCE is no longer keyword corroboration (cycle1's `EVIDENCE_NOT_RELEVANT`,
    which a single appended keyword could satisfy, and which returned zero violations on Korean-
    only clause text with zero extracted keywords). Cycle2 requires each evidence entry to name
    exact, executable `assertion_ids` -- identifiers that must literally exist in the cited file:
    unittest `ClassName.method_name` dotted names, bare top-level function names, or bare
    top-level variable-assignment targets for `.py` (AST-derived, not substring-matched); for
    `.sh` (bash has no stdlib AST) the anchored-regex
    analogue -- function names (`name() {`), top-level UPPERCASE variable assignments
    (`KEY_OK=`), and `case`-branch long CLI options (`--topology)`); YAML mapping keys for
    `.yaml`/`.yml` (anchored `key:` regex); and heading
    text for `.md` (an exact `#`-stripped heading line -- prose is honest about being weaker
    evidence than an executable identifier, but is still checked exactly, not by substring).
    A clause whose evidence is ENTIRELY prose (`.md`) is a structural violation
    (CLAUSE_LACKS_EXECUTABLE_EVIDENCE) -- prose may corroborate a clause, but may never be its
    sole evidence.
  - Removal eligibility (cycle2 fix) runs full schema-shape AND full-registry lifecycle
    validation FIRST (with the same explicit --as-of) -- cycle1's `check-removal` validated only
    the single selected record's status/evidence-status, so a retired policy with a self-cycling
    `superseded_by` and no retirement reason was reported eligible (0 violations) despite the
    rest of the registry being invalid. Only once the WHOLE registry is clean does the selected
    policy's own removal_evidence get checked: its `path` must resolve through the same
    manifest+digest+symlink gate, and the JSON artifact it points to must be a structured
    contract/E2E-PASS record containing this policy's own policy_id, a PASS result, a valid
    as_of date, and a well-formed tree_sha256 -- never a bare status literal plus an arbitrary
    allowlisted source file (cycle1's exact reviewer-reported tautology).
  - CLI hardening (cycle2 fix): every JSON file this module reads (registry, schema, evidence
    manifest, removal artifact) goes through one safe loader that catches OSError, UnicodeError,
    and json.JSONDecodeError, and validates the parsed root is a JSON object (dict) before any
    `.get()` call touches it -- cycle1's `verify`/`check-removal` let invalid UTF-8, malformed
    JSON, missing files, and array roots raise uncaught Python tracebacks with process exit 1,
    contradicting this module's own documented exit-2 invalid-input contract.
  - `policy:<ID>` citation scanning (cycle2 fix): the broad-capture regex now (a) requires a
    non-word-character boundary before the literal `policy:` so an embedded token like
    `xpolicy:BAD-ID` is correctly NOT recognized as a citation attempt at all (cycle1 falsely
    matched it), (b) allows a ZERO-length captured token so a bare `policy:` with nothing after
    it is caught as its own EMPTY_POLICY_CITATION violation instead of being silently invisible
    to a `+`-quantified capture group, and (c) strips ONE trailing run of ordinary terminal
    sentence punctuation (`.,;:!?`) from the captured token before syntax-validating it, so a
    valid citation immediately followed by a full stop (`policy:KNOWN.`) is not spuriously
    reported as malformed -- while a genuinely malformed token's ORIGINAL (unstripped) text is
    still what gets reported, so a case like `policy:ALSO_BAD-ID!` is not silently "fixed" by
    punctuation-stripping into something that looks valid.

Usage:
    python3 policy_registry.py verify --as-of YYYY-MM-DD [--registry <path>] [--schema <path>]
        [--evidence-manifest <path>] [--tracked-index <path>] [--claim-bindings <path>]
        [--repo-root <path>]
    python3 policy_registry.py check-removal --policy-id <ID> --as-of YYYY-MM-DD
        [--registry <path>] [--schema <path>] [--evidence-manifest <path>]
        [--tracked-index <path>] [--claim-bindings <path>] [--repo-root <path>]
    python3 policy_registry.py --self-test

--tracked-index/--claim-bindings default to <repo-root>/.claude/policies/{tracked_index,
claim_bindings}.json (repo-root-relative -- NOT this script's own fixed location, unlike
--registry/--schema/--evidence-manifest) -- see _load_inputs_or_emit's docstring.

Exit codes (both subcommands):
    0  clean -- zero violations
    1  policy-block -- input was well-formed but carries one or more lifecycle/evidence/
       structural/removal-eligibility violations
    2  invalid-input -- malformed JSON / missing file / invalid UTF-8 / non-object root /
       schema-shape violation / bad --as-of / unknown --policy-id

Every invocation emits exactly one stable JSON envelope to stdout and calls sys.exit with the
JSON body's own `exit_code` -- no bare Python traceback is ever the process's final output.

No third-party dependencies (stdlib only).
"""
from __future__ import annotations

import argparse
import ast
import copy
import datetime
import hashlib
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "policy-registry.schema.json"
REGISTRY_PATH = REPO_ROOT / ".claude" / "policies" / "registry.yaml"
EVIDENCE_MANIFEST_PATH = REPO_ROOT / ".claude" / "policies" / "evidence_manifest.json"
CLAIM_BINDINGS_PATH = REPO_ROOT / ".claude" / "policies" / "claim_bindings.json"
TRACKED_INDEX_PATH = REPO_ROOT / ".claude" / "policies" / "tracked_index.json"
GOVERNED_PROSE_SNAPSHOT_PATH = REPO_ROOT / ".claude" / "policies" / "governed_prose_snapshot.json"

# repo-relative (no leading REPO_ROOT) -- used both to build the CLI's repo_root-relative default
# resolution (cycle4 fix: a tracked_index/claim_bindings default must live INSIDE whatever
# --repo-root is being checked, not always resolve to THIS project's own file, or a custom/export
# repo that lacks its own copy would silently inherit this project's trust anchors) and by the
# generator that produces these files' own committed content.
TRACKED_INDEX_REL = ".claude/policies/tracked_index.json"
CLAIM_BINDINGS_REL = ".claude/policies/claim_bindings.json"
GOVERNED_PROSE_SNAPSHOT_REL = ".claude/policies/governed_prose_snapshot.json"

sys.path.insert(0, str(Path(__file__).resolve().parent))
import completion_gate as _gate  # noqa: E402 -- reuse the one generic schema validator

SCHEMA_VERSION = 2

_WALLCLOCK_CALL_ATTRS = {"today", "now"}


def find_wallclock_calls(source: str) -> list[tuple[int, str]]:
    """AST-level (not substring) scan for an actual zero-argument call to a `.today()`/`.now()`
    attribute anywhere in `source`'s executable code -- e.g. `date.today()`,
    `datetime.date.today()`, `datetime.now()`, `datetime.datetime.now()`, under any import
    alias. Deliberately does NOT look at raw text: a docstring or comment that merely *mentions*
    "date.today()" while explaining why this module never calls it must not self-trip a
    substring check. Returns a list of (lineno, unparsed_call_source) for each real call found;
    empty means clean. A call with any positional/keyword argument (e.g. a tz-aware
    `datetime.now(tz)`) is intentionally still flagged -- this project has no legitimate
    wall-clock read anywhere, timezone-aware or not.
    """
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return [(0, "<source did not parse -- treated as clean; a real syntax error surfaces "
                    "elsewhere as an actual import/compile failure>")]
    hits: list[tuple[int, str]] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if not (isinstance(func, ast.Attribute) and func.attr in _WALLCLOCK_CALL_ATTRS):
            continue
        try:
            hits.append((node.lineno, ast.unparse(node)))
        except Exception:  # pragma: no cover -- ast.unparse is stdlib-stable on 3.9+, defensive only
            hits.append((node.lineno, f"<unparseable call to .{func.attr}()>"))
    return hits


def find_banned_imports(source: str, banned_top_level_modules) -> list[tuple[int, str]]:
    """AST-level (not substring) scan for an actual `import X` / `from X import ...` of one of
    `banned_top_level_modules` (matched on the top-level module name only). Returns
    (lineno, statement_source) pairs; empty means clean.
    """
    banned = set(banned_top_level_modules)
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return [(0, "<source did not parse -- treated as clean; a real syntax error surfaces "
                    "elsewhere as an actual import/compile failure>")]
    hits: list[tuple[int, str]] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name.split(".")[0] in banned:
                    hits.append((node.lineno, f"import {alias.name}"))
        elif isinstance(node, ast.ImportFrom):
            if node.module and node.module.split(".")[0] in banned:
                hits.append((node.lineno, f"from {node.module} import ..."))
    return hits


POLICY_ID_RE = re.compile(r"^[A-Z][A-Z0-9_]*$")
CLAUSE_ID_RE = re.compile(r"^[A-Z][A-Z0-9_]*\.C[0-9]+$")

REVIEW_PERIOD_DAYS = 90
OVERDUE_GRACE_DAYS = 30  # due -> overdue boundary past next_review_due; documented, not implicit

_LAZY_PLACEHOLDERS = {"", "tbd", "todo", "unknown", "n/a", "na", "none"}
_MIN_SUBSTANTIVE_LEN = 40
_MIN_SUBSTANTIVE_WORDS = 6
_MIN_SUBSTANTIVE_DISTINCT_CHARS = 8

_TRAILING_PUNCT = ".,;:!?…。"  # ASCII terminal punctuation + Unicode ellipsis (…) + 。


class Violation:
    __slots__ = ("reason_code", "message", "policy_id", "clause_id", "path")

    def __init__(self, reason_code, message, policy_id=None, clause_id=None, path=None):
        self.reason_code = reason_code
        self.message = message
        self.policy_id = policy_id
        self.clause_id = clause_id
        self.path = path

    def to_dict(self):
        return {
            "reason_code": self.reason_code,
            "message": self.message,
            "policy_id": self.policy_id,
            "clause_id": self.clause_id,
            "path": self.path,
        }

    def __eq__(self, other):  # pragma: no cover -- test convenience only
        if not isinstance(other, Violation):
            return NotImplemented
        return self.to_dict() == other.to_dict()

    def __hash__(self):  # pragma: no cover
        return hash(tuple(sorted(self.to_dict().items(), key=lambda kv: kv[0])))

    def __repr__(self):  # pragma: no cover -- debugging convenience only
        return f"Violation({self.reason_code!r}, {self.message!r}, policy_id={self.policy_id!r})"


# =============================================================================
# `policy:<ID>` citation scanning -- shared by tests/harness/test_constitution_references.py so
# there is one production broad-capture-then-validate implementation.
# =============================================================================

# (?<![A-Za-z0-9_가-힣]) -- token-boundary guard: `xpolicy:BAD-ID` must NOT be recognized as a
# citation attempt (cycle1 falsely matched the embedded "policy:" substring inside it). cycle4
# fix: the excluded-character class now also covers Hangul syllables (가-힣) so an embedded Korean
# token immediately before the literal, e.g. `가policy:BAD-ID`, is likewise never recognized as a
# citation attempt (cycle3 finding: this project's prose is Korean-dominant, so an ASCII-only
# word-boundary guard was incomplete).
# The captured group uses `*` (not `+`) so a bare `policy:` with nothing following is still
# CAPTURED (as an empty string) rather than silently invisible to the regex.
POLICY_CITATION_BROAD_RE = re.compile(r"(?<![A-Za-z0-9_가-힣])policy:([^\s`)\]},]*)")
_EMBEDDED_POLICY_RE = re.compile(r"(?<=[A-Za-z0-9_가-힣])policy:([A-Z][A-Z0-9_]*)(?![A-Za-z0-9_-])")


def scan_policy_citations(text: str) -> list[tuple[int, str]]:
    """Broad capture: every `policy:<token>` occurrence not itself embedded in a larger
    identifier, token running until whitespace or a closing/prose delimiter (backtick, paren,
    bracket, brace, comma) -- malformed and empty tokens are captured here too, not silently
    skipped. Returns (line_no, token) pairs in document order; callers validate each token's
    syntax and resolution separately via citation_violations."""
    return [(text.count("\n", 0, m.start()) + 1, m.group(1)) for m in POLICY_CITATION_BROAD_RE.finditer(text)]


def _split_trailing_punct(token: str) -> tuple[str, str]:
    """Strips ONE trailing run of ordinary terminal sentence punctuation from `token`. Returns
    (core, trailing) -- `core` is what gets syntax/resolution-checked; the ORIGINAL `token` (not
    `core`) is still what a MALFORMED_CITATION_SYNTAX violation reports, so stripping never
    disguises a genuinely malformed id (e.g. `ALSO_BAD-ID!` strips to `ALSO_BAD-ID`, which still
    fails POLICY_ID_RE and is still reported as the original `ALSO_BAD-ID!`)."""
    end = len(token)
    while end > 0 and token[end - 1] in _TRAILING_PUNCT:
        end -= 1
    return token[:end], token[end:]


def citation_violations(text: str, registry_doc: dict, source_label: str | None = None) -> list[Violation]:
    """Three-way validation over every broadly-captured citation in `text`: (1) EMPTY -- a bare
    `policy:` with no token at all; (2) syntax -- after stripping trailing terminal punctuation,
    the core token must match POLICY_ID_RE, else MALFORMED_CITATION_SYNTAX (reported against the
    ORIGINAL, unstripped token); (3) resolution -- an already-syntax-valid core token must
    resolve to a real, non-retired registry entry, else UNRESOLVED_CITATION. `source_label`
    (e.g. a tracked file's repo-relative path), when given, is stamped onto Violation.path."""
    by_id = {p.get("policy_id"): p for p in registry_doc.get("policies", []) if isinstance(p, dict)}
    out: list[Violation] = []
    for line_no, token in scan_policy_citations(text):
        if token == "":
            out.append(Violation("EMPTY_POLICY_CITATION",
                                  f"bare 'policy:' citation with no id at line {line_no}", path=source_label))
            continue
        core, _trailing = _split_trailing_punct(token)
        if not core or not POLICY_ID_RE.match(core):
            out.append(Violation("MALFORMED_CITATION_SYNTAX",
                                  f"policy: citation {token!r} at line {line_no} is not a syntactically "
                                  "valid policy_id", policy_id=token, path=source_label))
            continue
        entry = by_id.get(core)
        if entry is None or entry.get("status") == "retired":
            out.append(Violation("UNRESOLVED_CITATION",
                                  f"policy: citation {core!r} at line {line_no} does not resolve to a "
                                  "live (non-retired) registry entry", policy_id=core, path=source_label))
    # Embedded occurrences are not accepted as citations (the broad scanner's boundary contract),
    # but a citation-shaped, syntactically valid UNKNOWN id must not disappear merely because a
    # preceding word character was accidentally concatenated to it.  Known ids and malformed
    # `xpolicy:BAD-ID` examples remain ignored as ordinary larger-token substrings.
    for m in _EMBEDDED_POLICY_RE.finditer(text):
        token = m.group(1)
        entry = by_id.get(token)
        if entry is None or entry.get("status") == "retired":
            line_no = text.count("\n", 0, m.start()) + 1
            out.append(Violation("UNRESOLVED_CITATION",
                                 f"embedded citation-shaped policy id {token!r} at line {line_no} "
                                 "does not resolve to a live registry entry",
                                 policy_id=token, path=source_label))
    return out


# =============================================================================
# Governed-prose SSOT snapshot (cycle4 fix -- subagent-summary-0 finding 1 root cause /
# subagent-summary-1 finding 7): TestNoDuplicateProse's Jaccard token-overlap scan and
# TestNoVerbatimPolicyStatementInProse's whole-registry-statement substring check are BOTH
# deliberately diagnostic-only, not semantic proof -- a paraphrase that shares few tokens with the
# registry's own statement (a reviewer's OWN prose restating a rule in different words) sails past
# both checks even though it duplicates governed semantics.
# `.claude/policies/governed_prose_snapshot.json` records one exact SHA256 per governed file
# (CLAUDE.md, workflow.md, docs.md, references.md, in full -- byte content, no normalization
# ambiguity). governed_prose_snapshot_violations flags ANY drift from that snapshot -- including
# purely-prose edits that add no forbidden token and cite no policy at all. This is DELIBERATELY
# NOT itself semantic proof of anything -- it is a change-review TRIPWIRE: any edit to a governed
# file must be paired with a deliberate, reviewed snapshot update (see
# scripts/policy_registry.py --self-test docstring pattern and test_governed_prose_snapshot.py for
# the exact "append a paraphrase -> tripwire fires" regression this exists to lock in).
# =============================================================================

GOVERNED_PROSE_FILES = {
    "CLAUDE.md": "CLAUDE.md",
    ".claude/rules/workflow.md": ".claude/rules/workflow.md",
    ".claude/rules/docs.md": ".claude/rules/docs.md",
    ".claude/rules/references.md": ".claude/rules/references.md",
}


def governed_prose_snapshot_violations(snapshot: dict, repo_root: Path = REPO_ROOT) -> list:
    """Compares each of the 4 governed files' CURRENT exact byte-content SHA256 against
    `snapshot["files"][label]`. Any mismatch (including a change that adds no forbidden token and
    cites no policy) is GOVERNED_PROSE_SNAPSHOT_MISMATCH -- a change-review tripwire, not semantic
    proof (see module docstring section above)."""
    out: list = []
    canonical_trust = "change-review tripwire; ambient review establishes semantic equivalence"
    if (not isinstance(snapshot, dict) or set(snapshot) != {"schema_version", "_trust", "files"}
            or snapshot.get("schema_version") != 1 or snapshot.get("_trust") != canonical_trust):
        return [Violation(
            "GOVERNED_PROSE_SNAPSHOT_SHAPE_INVALID",
            "snapshot requires exactly schema_version=1, the canonical _trust declaration, and files",
            path=GOVERNED_PROSE_SNAPSHOT_REL)]
    files = snapshot.get("files", {}) if isinstance(snapshot, dict) else {}
    if (not isinstance(files, dict) or set(files) != set(GOVERNED_PROSE_FILES)
            or any(not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None
                   for value in files.values())):
        return [Violation(
            "GOVERNED_PROSE_SNAPSHOT_FILES_INVALID",
            "files must contain exactly the four governed paths with lowercase SHA-256 digests",
            path=GOVERNED_PROSE_SNAPSHOT_REL)]
    for label, rel_path in GOVERNED_PROSE_FILES.items():
        full = repo_root / rel_path
        try:
            digest = hashlib.sha256(full.read_bytes()).hexdigest()
        except OSError:
            out.append(Violation("GOVERNED_PROSE_FILE_UNREADABLE", f"{rel_path} could not be read",
                                  path=rel_path))
            continue
        expected = files.get(label)
        if expected != digest:
            out.append(Violation("GOVERNED_PROSE_SNAPSHOT_MISMATCH",
                                  f"{rel_path} content digest {digest} != snapshot {expected!r} -- governed "
                                  "prose changed without a reviewed snapshot update", path=rel_path))
    return out


# =============================================================================
# Safe, hermetic JSON loading -- one implementation every CLI entry point shares (cycle2 fix:
# cycle1 let OSError/UnicodeDecodeError/JSONDecodeError raise uncaught tracebacks with process
# exit 1, contradicting this module's own documented exit-2 invalid-input contract).
# =============================================================================

def safe_load_json_object(path: Path, label: str) -> tuple[dict | None, Violation | None]:
    """Reads `path`, decodes as UTF-8, parses as JSON, and requires the parsed root to be a JSON
    object (dict) -- returns (doc, None) on success or (None, Violation) on ANY failure mode
    (missing/unreadable file, invalid UTF-8, malformed JSON, non-object root incl. a JSON array).
    `label` becomes the reason_code prefix (e.g. "REGISTRY", "SCHEMA", "EVIDENCE_MANIFEST")."""
    try:
        raw = path.read_bytes()
    except OSError as e:
        return None, Violation(f"{label}_LOAD_ERROR", f"could not read {path}: {e}")
    try:
        text = raw.decode("utf-8")
    except UnicodeError as e:
        return None, Violation(f"{label}_INVALID_UTF8", f"{path} is not valid UTF-8: {e}")
    try:
        doc = json.loads(text)
    except (json.JSONDecodeError, RecursionError) as e:
        return None, Violation(f"{label}_INVALID_JSON", f"{path} is not valid JSON: {e}")
    if not isinstance(doc, dict):
        return None, Violation(f"{label}_ROOT_NOT_OBJECT",
                                f"{path} must parse to a JSON object at the root, got {type(doc).__name__}")
    return doc, None


def load_registry(path: Path = REGISTRY_PATH) -> dict:
    """Loads registry.yaml as plain JSON (stdlib only -- see module docstring). Raises on failure
    -- callers that need the safe/non-raising CLI contract use safe_load_json_object directly."""
    doc, err = safe_load_json_object(path, "REGISTRY")
    if err is not None:
        raise ValueError(err.message)
    return doc


def load_schema(path: Path = SCHEMA_PATH) -> dict:
    doc, err = safe_load_json_object(path, "SCHEMA")
    if err is not None:
        raise ValueError(err.message)
    return doc


def load_evidence_manifest(path: Path = EVIDENCE_MANIFEST_PATH) -> dict:
    doc, err = safe_load_json_object(path, "EVIDENCE_MANIFEST")
    if err is not None:
        raise ValueError(err.message)
    return doc


def load_claim_bindings(path: Path = CLAIM_BINDINGS_PATH) -> dict:
    doc, err = safe_load_json_object(path, "CLAIM_BINDINGS")
    if err is not None:
        raise ValueError(err.message)
    return doc


def load_tracked_index(path: Path = TRACKED_INDEX_PATH) -> dict:
    doc, err = safe_load_json_object(path, "TRACKED_INDEX")
    if err is not None:
        raise ValueError(err.message)
    return doc


def load_governed_prose_snapshot(path: Path = GOVERNED_PROSE_SNAPSHOT_PATH) -> dict:
    doc, err = safe_load_json_object(path, "GOVERNED_PROSE_SNAPSHOT")
    if err is not None:
        raise ValueError(err.message)
    return doc


# =============================================================================
# Schema TRUST (cycle4 fix -- subagent-summary-1 finding 4): `--schema {}` used to disable
# validation outright (an empty dict trivially satisfies every keyword-presence check in
# completion_gate.validate_against_schema, since every check is `if "keyword" in schema: ...`),
# and a malformed-but-truthy schema document (e.g. `{"type": 1}`, `{"required": 5}`) crashed the
# generic engine with an uncaught Python traceback (process exit 1), contradicting this module's
# own documented exit-2 invalid-input contract. schema_meta_violations validates the SCHEMA
# DOCUMENT ITSELF against this project's strict supported-shape contract (Draft-07 SUBSET only --
# no pattern/oneOf/anyOf/patternProperties/additionalItems/not) BEFORE the generic engine ever
# touches it; schema_violations also wraps the generic engine call itself in a catch-all as
# defense in depth, so NO input can ever produce a bare traceback -- only ever the one stable JSON
# envelope this module has always promised.
# =============================================================================

_SUPPORTED_JSON_SCHEMA_TYPES = {"object", "array", "string", "integer", "number", "boolean", "null"}
_UNSUPPORTED_SCHEMA_KEYWORDS = ("oneOf", "anyOf", "not", "patternProperties", "pattern", "additionalItems")


def schema_meta_violations(schema, path: str = "$") -> list:
    """Recursively validates that `schema` (and every nested subschema reachable through
    properties/definitions/items/additionalProperties/allOf/if/then/else) uses only keywords and
    keyword-value shapes this project's generic engine (scripts/completion_gate.py
    validate_against_schema) actually implements. Does NOT resolve $ref targets (an unresolved
    forward reference inside `definitions` is legitimate and resolving here would risk infinite
    recursion on a self-referential schema) -- it only checks the $ref value's own shape."""
    out: list = []
    if not isinstance(schema, dict):
        out.append(Violation("SCHEMA_META_NOT_OBJECT",
                              f"{path}: schema node must be a JSON object, got {type(schema).__name__}", path=path))
        return out
    if path == "$" and not schema:
        out.append(Violation("SCHEMA_META_EMPTY_ROOT",
                              "root schema document is empty ({}) -- every keyword-presence check in the "
                              "generic engine is skipped, which would silently disable validation entirely",
                              path=path))
        return out
    for kw in _UNSUPPORTED_SCHEMA_KEYWORDS:
        if kw in schema:
            out.append(Violation("SCHEMA_META_UNSUPPORTED_KEYWORD",
                                  f"{path}: {kw!r} is not a supported keyword of this project's "
                                  "Draft-07-subset engine", path=f"{path}.{kw}"))
    if "type" in schema:
        t = schema["type"]
        types = t if isinstance(t, list) else [t]
        if not isinstance(t, (str, list)) or not all(isinstance(x, str) for x in types):
            out.append(Violation("SCHEMA_META_BAD_TYPE_KEYWORD",
                                  f"{path}.type must be a string or a list of strings, got {t!r}",
                                  path=f"{path}.type"))
        else:
            bad = [x for x in types if x not in _SUPPORTED_JSON_SCHEMA_TYPES]
            if bad:
                out.append(Violation("SCHEMA_META_UNSUPPORTED_TYPE_VALUE",
                                      f"{path}.type contains unsupported value(s) {bad!r}", path=f"{path}.type"))
    if "required" in schema:
        req = schema["required"]
        if not isinstance(req, list) or not all(isinstance(x, str) for x in req):
            out.append(Violation("SCHEMA_META_BAD_REQUIRED",
                                  f"{path}.required must be a list of strings, got {req!r}", path=f"{path}.required"))
    if "properties" in schema:
        props = schema["properties"]
        if not isinstance(props, dict):
            out.append(Violation("SCHEMA_META_BAD_PROPERTIES",
                                  f"{path}.properties must be an object, got {props!r}", path=f"{path}.properties"))
        else:
            for key, sub in props.items():
                out.extend(schema_meta_violations(sub, f"{path}.properties.{key}"))
    if "definitions" in schema:
        defs = schema["definitions"]
        if not isinstance(defs, dict):
            out.append(Violation("SCHEMA_META_BAD_DEFINITIONS",
                                  f"{path}.definitions must be an object, got {defs!r}", path=f"{path}.definitions"))
        else:
            for key, sub in defs.items():
                out.extend(schema_meta_violations(sub, f"{path}.definitions.{key}"))
    if "items" in schema:
        out.extend(schema_meta_violations(schema["items"], f"{path}.items"))
    if "additionalProperties" in schema:
        ap = schema["additionalProperties"]
        if isinstance(ap, dict):
            out.extend(schema_meta_violations(ap, f"{path}.additionalProperties"))
        elif not isinstance(ap, bool):
            out.append(Violation("SCHEMA_META_BAD_ADDITIONAL_PROPERTIES",
                                  f"{path}.additionalProperties must be a boolean or an object, got {ap!r}",
                                  path=f"{path}.additionalProperties"))
    if "allOf" in schema:
        allof = schema["allOf"]
        if not isinstance(allof, list):
            out.append(Violation("SCHEMA_META_BAD_ALLOF", f"{path}.allOf must be a list, got {allof!r}",
                                  path=f"{path}.allOf"))
        else:
            for i, sub in enumerate(allof):
                out.extend(schema_meta_violations(sub, f"{path}.allOf[{i}]"))
    for kw in ("if", "then", "else"):
        if kw in schema:
            out.extend(schema_meta_violations(schema[kw], f"{path}.{kw}"))
    if "$ref" in schema:
        ref = schema["$ref"]
        if not isinstance(ref, str) or not ref.startswith("#/"):
            out.append(Violation("SCHEMA_META_BAD_REF", f"{path}.$ref must be a local '#/...' string, got {ref!r}",
                                  path=f"{path}.$ref"))
    if "enum" in schema and not isinstance(schema["enum"], list):
        out.append(Violation("SCHEMA_META_BAD_ENUM", f"{path}.enum must be a list, got {schema['enum']!r}",
                              path=f"{path}.enum"))
    for kw in ("minLength", "minimum", "minItems"):
        if kw in schema:
            v = schema[kw]
            if isinstance(v, bool) or not isinstance(v, (int, float)):
                out.append(Violation("SCHEMA_META_BAD_NUMERIC_KEYWORD",
                                      f"{path}.{kw} must be numeric, got {v!r}", path=f"{path}.{kw}"))
    return out


def schema_violations(doc: dict, schema: dict) -> list:
    """Wraps completion_gate.py's generic (code, message) pairs -- code looks like
    'SCHEMA_MISSING_REQUIRED_KEY:policies[0].remove_when'. reason_code keeps the FULL original
    string (backward-compatible with every existing exact-reason_code assertion); `.path` is
    ADDITIONALLY populated with just the field-path suffix (cycle2 fix: subagent-summary-1
    finding 6 -- Violation.path stayed null even though the field path was embedded in the
    reason_code string, so a caller could not query violations by exact field path).

    cycle4 fix: `schema` itself is validated against schema_meta_violations FIRST (a malformed or
    empty schema document is rejected before the generic engine ever runs against it), and the
    generic engine call is wrapped in a catch-all exception handler as defense in depth -- no
    input can crash this function into a bare traceback."""
    meta = schema_meta_violations(schema)
    if meta:
        return meta
    try:
        raw = _gate.validate_against_schema(doc, schema)
    except Exception as e:  # pragma: no cover -- defense in depth; schema_meta_violations above
        # already rejects every schema shape known to crash the generic engine.
        return [Violation("SCHEMA_ENGINE_ERROR",
                           f"schema validation raised an unexpected exception: {type(e).__name__}: {e}")]
    out = []
    for code, msg in raw:
        base, _sep, field_path = code.partition(":")
        out.append(Violation(f"SCHEMA:{code}", msg, path=(field_path or None)))
    return out


def _is_iso_date(value) -> bool:
    if not isinstance(value, str):
        return False
    try:
        datetime.date.fromisoformat(value)
        return True
    except ValueError:
        return False


def _parse_iso_date(value) -> datetime.date | None:
    if not _is_iso_date(value):
        return None
    return datetime.date.fromisoformat(value)


def duplicate_id_violations(policies: list) -> list:
    seen, dupes = set(), set()
    for p in policies:
        pid = p.get("policy_id")
        if pid in seen:
            dupes.add(pid)
        seen.add(pid)
    return [Violation("DUPLICATE_POLICY_ID", f"policy_id {pid!r} appears more than once", policy_id=pid,
                       path=f"$.policies[{pid}].policy_id")
            for pid in sorted(d for d in dupes if d)]


def id_pattern_violations(policies: list) -> list:
    out = []
    for p in policies:
        pid = p.get("policy_id")
        if not POLICY_ID_RE.match(str(pid)):
            out.append(Violation("BAD_POLICY_ID_PATTERN", f"{pid!r} does not match {POLICY_ID_RE.pattern}",
                                  policy_id=pid, path=f"$.policies[{pid}].policy_id"))
        for c in p.get("clauses") or []:
            cid = c.get("clause_id") if isinstance(c, dict) else None
            if not CLAUSE_ID_RE.match(str(cid)):
                out.append(Violation("BAD_CLAUSE_ID_PATTERN",
                                      f"{cid!r} does not match {CLAUSE_ID_RE.pattern}",
                                      policy_id=pid, clause_id=cid, path=f"$.policies[{pid}].clauses[{cid}].clause_id"))
            elif cid and not cid.startswith(f"{pid}."):
                out.append(Violation("CLAUSE_ID_POLICY_MISMATCH",
                                      f"clause_id {cid!r} is not namespaced under policy_id {pid!r}",
                                      policy_id=pid, clause_id=cid, path=f"$.policies[{pid}].clauses[{cid}].clause_id"))
    return out


def provenance_violations(policies: list) -> list:
    out = []
    for p in policies:
        pid = p.get("policy_id")
        has_of = bool(p.get("origin_failure"))
        has_ui = bool(p.get("universal_invariant"))
        if has_of == has_ui:
            out.append(Violation("PROVENANCE_VIOLATION",
                                  "exactly one of origin_failure/universal_invariant is required",
                                  policy_id=pid, path=f"$.policies[{pid}]"))
    return out


def _is_substantive(text) -> bool:
    """Rejects both known lazy placeholders AND a superficially-long-enough but degenerate
    string (20 literal 'x' characters clears a naive len>=20 check). Substantive requires: not a
    known placeholder, real length, multiple distinct words, and enough distinct characters that
    a repeated-character filler cannot pass."""
    if not isinstance(text, str):
        return False
    stripped = text.strip()
    norm = stripped.strip(".").lower()
    if norm in _LAZY_PLACEHOLDERS:
        return False
    if len(stripped) < _MIN_SUBSTANTIVE_LEN:
        return False
    words = re.findall(r"[A-Za-z0-9가-힣]+", stripped)
    if len(words) < _MIN_SUBSTANTIVE_WORDS:
        return False
    distinct_chars = set(stripped.lower()) - {" ", "\t", "\n"}
    if len(distinct_chars) < _MIN_SUBSTANTIVE_DISTINCT_CHARS:
        return False
    return True


def remove_when_violations(policies: list) -> list:
    return [Violation("LAZY_REMOVE_WHEN", "remove_when is not substantive", policy_id=p.get("policy_id"),
                       path=f"$.policies[{p.get('policy_id')}].remove_when")
            for p in policies if not _is_substantive(p.get("remove_when"))]


def review_shape_violations(policies: list) -> list:
    out = []
    for p in policies:
        pid = p.get("policy_id")
        review = p.get("review") or {}
        if review.get("state") not in ("current", "due", "overdue"):
            out.append(Violation("REVIEW_SHAPE_VIOLATION", "review.state is not a valid enum value", policy_id=pid,
                                  path=f"$.policies[{pid}].review.state"))
        if not _is_iso_date(review.get("last_reviewed_at")):
            out.append(Violation("REVIEW_SHAPE_VIOLATION", "review.last_reviewed_at is not an ISO-8601 date",
                                  policy_id=pid, path=f"$.policies[{pid}].review.last_reviewed_at"))
        if not _is_iso_date(review.get("next_review_due")):
            out.append(Violation("REVIEW_SHAPE_VIOLATION", "review.next_review_due is not an ISO-8601 date",
                                  policy_id=pid, path=f"$.policies[{pid}].review.next_review_due"))
        reviewer = review.get("reviewer")
        if not isinstance(reviewer, str) or not reviewer.strip():
            out.append(Violation("REVIEW_SHAPE_VIOLATION", "review.reviewer is missing", policy_id=pid,
                                  path=f"$.policies[{pid}].review.reviewer"))
        if review.get("state") in ("due", "overdue") and not _is_substantive(review.get("reason")):
            out.append(Violation("REVIEW_REASON_MISSING",
                                  "review.state is due/overdue but review.reason is not a substantive explanation",
                                  policy_id=pid, path=f"$.policies[{pid}].review.reason"))
    return out


def review_due_arithmetic_violations(policies: list) -> list:
    """next_review_due must equal last_reviewed_at + REVIEW_PERIOD_DAYS exactly -- pure date
    arithmetic on the record's own two fields, not a wall-clock comparison."""
    out = []
    for p in policies:
        pid = p.get("policy_id")
        review = p.get("review") or {}
        last = _parse_iso_date(review.get("last_reviewed_at"))
        due = _parse_iso_date(review.get("next_review_due"))
        if last is None or due is None:
            continue  # already reported by review_shape_violations
        expected = last + datetime.timedelta(days=REVIEW_PERIOD_DAYS)
        if due != expected:
            out.append(Violation("REVIEW_DUE_ARITHMETIC_MISMATCH",
                                  f"next_review_due {due.isoformat()} != last_reviewed_at + {REVIEW_PERIOD_DAYS}d "
                                  f"({expected.isoformat()})", policy_id=pid,
                                  path=f"$.policies[{pid}].review.next_review_due"))
    return out


def last_reproduced_violations(policies: list) -> list:
    """last_reproduced must be null (never reproduced) or a real ISO-8601 date -- never an
    arbitrary string like the literal word "never"."""
    out = []
    for p in policies:
        pid = p.get("policy_id")
        val = p.get("last_reproduced")
        if val is None:
            continue
        if not _is_iso_date(val):
            out.append(Violation("BAD_LAST_REPRODUCED_DATE",
                                  f"last_reproduced {val!r} is neither null nor a valid ISO-8601 date",
                                  policy_id=pid, path=f"$.policies[{pid}].last_reproduced"))
    return out


# =============================================================================
# Date chronology (cycle2 -- entirely new). added_at is the anchor every other date is checked
# against; a policy cannot be reproduced/reviewed before it was added, and no date may be in the
# future relative to the explicit as_of.
# =============================================================================

def date_chronology_violations(policies: list, as_of: datetime.date) -> list:
    out = []
    for p in policies:
        pid = p.get("policy_id")
        added_raw = p.get("added_at")
        if not _is_iso_date(added_raw):
            out.append(Violation("BAD_ADDED_AT", f"added_at {added_raw!r} is not a valid ISO-8601 date",
                                  policy_id=pid, path=f"$.policies[{pid}].added_at"))
            continue  # every other ordering check below is anchored on a valid added_at
        added = _parse_iso_date(added_raw)
        if added > as_of:
            out.append(Violation("FUTURE_ADDED_AT",
                                  f"added_at {added.isoformat()} is after as_of={as_of.isoformat()}",
                                  policy_id=pid, path=f"$.policies[{pid}].added_at"))
        last_repro_raw = p.get("last_reproduced")
        if last_repro_raw is not None and _is_iso_date(last_repro_raw):
            last_repro = _parse_iso_date(last_repro_raw)
            if last_repro > as_of:
                out.append(Violation("FUTURE_REPRODUCTION",
                                      f"last_reproduced {last_repro.isoformat()} is after as_of={as_of.isoformat()}",
                                      policy_id=pid, path=f"$.policies[{pid}].last_reproduced"))
            if added > last_repro:
                out.append(Violation("ADDED_AFTER_REPRODUCTION",
                                      f"added_at {added.isoformat()} is after last_reproduced "
                                      f"{last_repro.isoformat()}", policy_id=pid,
                                      path=f"$.policies[{pid}].last_reproduced"))
        review = p.get("review") or {}
        last_reviewed_raw = review.get("last_reviewed_at")
        if _is_iso_date(last_reviewed_raw):
            last_reviewed = _parse_iso_date(last_reviewed_raw)
            if last_reviewed > as_of:
                out.append(Violation("FUTURE_REVIEW",
                                      f"review.last_reviewed_at {last_reviewed.isoformat()} is after "
                                      f"as_of={as_of.isoformat()}", policy_id=pid,
                                      path=f"$.policies[{pid}].review.last_reviewed_at"))
            if added > last_reviewed:
                out.append(Violation("ADDED_AFTER_REVIEW",
                                      f"added_at {added.isoformat()} is after review.last_reviewed_at "
                                      f"{last_reviewed.isoformat()}", policy_id=pid,
                                      path=f"$.policies[{pid}].review.last_reviewed_at"))
    return out


def _reproduction_stale_origin(p: dict) -> datetime.date | None:
    """The date the reproduction-staleness clock counts from: last_reproduced when set (and
    valid), else added_at (cycle2 fix -- cycle1 treated a null last_reproduced as unconditionally
    stale regardless of how recently the policy was added)."""
    last_repro = _parse_iso_date(p.get("last_reproduced"))
    if last_repro is not None:
        return last_repro
    return _parse_iso_date(p.get("added_at"))


def unreproduced_90_day_violations(policies: list, as_of: datetime.date) -> list:
    """An `active` policy unreproduced (or never reproduced) for >= REVIEW_PERIOD_DAYS, measured
    from _reproduction_stale_origin, must be status == candidate, not active."""
    out = []
    for p in policies:
        pid = p.get("policy_id")
        status = p.get("status")
        origin = _reproduction_stale_origin(p)
        if origin is None:
            continue  # BAD_ADDED_AT already reported; can't measure elapsed days
        stale = (as_of - origin).days >= REVIEW_PERIOD_DAYS
        if stale and status == "active":
            out.append(Violation("UNREPRODUCED_90_DAY_NOT_CANDIDATE",
                                  "unreproduced (or never-reproduced) for >= 90 days (measured from "
                                  "last_reproduced, or added_at when never reproduced) but status is "
                                  "still active, not candidate", policy_id=pid, path=f"$.policies[{pid}].status"))
    return out


def as_of_state_violations(policies: list, as_of: datetime.date) -> list:
    """Deterministic review.state consistency for the given as_of -- purely a function of
    next_review_due vs as_of (cycle2 fix: DECOUPLED from reproduction staleness, which cycle1
    conflated with review cadence; a policy reviewed today is genuinely `current` for review
    purposes even when it is `candidate`, not yet reproduced, for status purposes -- see module
    docstring "TWO INDEPENDENT 90-day clocks"). as_of is always an explicit function argument --
    never wall-clock.

    cycle4 boundary fix (subagent-summary-1 finding 5): review.state is now an EXACT, single
    deterministic function of as_of vs next_review_due -- before due => current; due <= as_of <=
    due+30d => due (both endpoints inclusive to `due`); as_of > due+30d => overdue. Previously an
    `overdue` state at as_of == next_review_due (i.e. still within the due window) was ALSO
    silently accepted alongside `due` (the code only rejected `current` in that window) -- this
    collapses the window to exactly one compliant state, so `overdue` at as_of == due is now
    correctly flagged."""
    out = []
    for p in policies:
        pid = p.get("policy_id")
        review = p.get("review") or {}
        state = review.get("state")
        due = _parse_iso_date(review.get("next_review_due"))
        if due is None:
            continue  # already reported by review_shape_violations
        if as_of < due:
            expected_state = "current"
        elif as_of <= due + datetime.timedelta(days=OVERDUE_GRACE_DAYS):
            expected_state = "due"
        else:
            expected_state = "overdue"
        if state != expected_state:
            out.append(Violation("AS_OF_STATE_MISMATCH",
                                  f"as_of={as_of.isoformat()} vs next_review_due={due.isoformat()} requires "
                                  f"review.state={expected_state!r} exactly, got {state!r}",
                                  policy_id=pid, path=f"$.policies[{pid}].review.state"))
    return out


def status_enum_violations(policies: list) -> list:
    return [Violation("STATUS_VIOLATION", "status is not one of active/candidate/retired",
                       policy_id=p.get("policy_id"), path=f"$.policies[{p.get('policy_id')}].status")
            for p in policies if p.get("status") not in ("active", "candidate", "retired")]


def retirement_metadata_closure_violations(policies: list) -> list:
    """active/candidate entries may not carry retirement_reason, superseded_by, OR
    removal_evidence (cycle2 fix -- cycle1 only checked the first two, so an `active` policy
    self-declaring removal_evidence.status=contract_e2e_green passed with zero violations).
    retired entries must have a substantive retirement_reason OR a superseded_by that resolves
    and forms no cycle.

    cycle4 fix (subagent-summary-1 finding 5): the active/candidate closure check now rejects
    these three keys by KEY PRESENCE, not truthiness -- cycle3's `if reason:` /
    `if successor:` / `if removal_evidence:` treated an explicitly-declared `null` value (the key
    IS present, schema-legal per the field's `["string","null"]` / object-or-absent type) as if
    the field were simply absent, silently accepting a live policy that carries
    `"retirement_reason": null` verbatim. A live policy must not carry these keys AT ALL, whatever
    their value."""
    out = []
    ids = {p.get("policy_id") for p in policies}
    edges = {}
    for p in policies:
        pid = p.get("policy_id")
        status = p.get("status")
        reason = p.get("retirement_reason")
        successor = p.get("superseded_by")
        removal_evidence = p.get("removal_evidence")
        if status in ("active", "candidate"):
            if "retirement_reason" in p:
                out.append(Violation("RETIREMENT_METADATA_ON_LIVE_POLICY",
                                      "active/candidate entry carries a retirement_reason key (even if null)",
                                      policy_id=pid, path=f"$.policies[{pid}].retirement_reason"))
            if "superseded_by" in p:
                out.append(Violation("RETIREMENT_METADATA_ON_LIVE_POLICY",
                                      "active/candidate entry carries a superseded_by key (even if null)",
                                      policy_id=pid, path=f"$.policies[{pid}].superseded_by"))
            if "removal_evidence" in p:
                out.append(Violation("RETIREMENT_METADATA_ON_LIVE_POLICY",
                                      "active/candidate entry carries a removal_evidence key (even if null)",
                                      policy_id=pid, path=f"$.policies[{pid}].removal_evidence"))
        if status == "retired":
            has_reason = _is_substantive(reason)
            if successor:
                edges[pid] = successor
                if successor not in ids:
                    out.append(Violation("DANGLING_SUPERSEDE",
                                          f"superseded_by {successor!r} does not resolve to a real policy_id",
                                          policy_id=pid, path=f"$.policies[{pid}].superseded_by"))
            elif not has_reason:
                out.append(Violation("RETIRED_WITHOUT_REASON",
                                      "retired with neither a substantive retirement_reason nor a superseded_by",
                                      policy_id=pid, path=f"$.policies[{pid}].retirement_reason"))
    for start in edges:
        seen = set()
        cur = start
        while cur in edges:
            if cur in seen:
                out.append(Violation("SUPERSEDE_CYCLE", f"superseded_by cycle detected starting at {start!r}",
                                      policy_id=start, path=f"$.policies[{start}].superseded_by"))
                break
            seen.add(cur)
            cur = edges[cur]
    return out


# =============================================================================
# Evidence resolution -- exact, digest-locked manifest (cycle2 replaces cycle1's prefix
# allowlist). See module docstring for the exploit this closes.
# =============================================================================

class _EvidenceResolution:
    __slots__ = ("status",)

    def __init__(self, status):
        self.status = status


_EVIDENCE_RESOLUTION_REASON = {
    "absent": "EVIDENCE_PATH_ABSENT",
    "untracked": "EVIDENCE_UNTRACKED",
    "path_escape": "EVIDENCE_PATH_ESCAPES_REPO",
    "symlink": "EVIDENCE_PATH_CONTAINS_SYMLINK",
    "missing": "EVIDENCE_MISSING_FILE",
    "digest_mismatch": "EVIDENCE_DIGEST_MISMATCH",
    "not_tracked_index": "EVIDENCE_NOT_IN_TRACKED_INDEX",
    "tracked_index_invalid": "EVIDENCE_TRACKED_INDEX_INVALID",
    "tracked_index_drift": "EVIDENCE_TRACKED_INDEX_DRIFT",
    "not_staged": "EVIDENCE_NOT_STAGED",
}


# =============================================================================
# Tracked-index trust tiers (cycle4 fix -- subagent-summary-1 finding 1): membership in a
# caller-supplied evidence_manifest.json is NOT independent proof that a path is actually
# git-tracked -- a self-authored manifest can list (and digest-match) any file, including one
# that was never staged/committed at all. `.claude/policies/tracked_index.json` is a SEPARATE,
# independently-generated snapshot (repo-relative path -> git blob sha1) that resolve_evidence_path
# additionally cross-checks:
#   - AMBIENT GIT tier (this repo, in this environment): the path's CURRENT STAGED blob
#     (`git ls-files --stage`) must match the snapshot's recorded blob sha1 exactly -- this is the
#     actual provenance authority; a path present in tracked_index.json but drifted (or never
#     staged) fails here.
#   - GITLESS tier (a clean-index/no-.git export): no live git call is possible, so trust rests on
#     tracked_index.json's own recorded snapshot (presence) plus evidence_manifest.json's sha256
#     digest match (already checked above) -- this is an EXPORT-TRUST INPUT, not an independent
#     cryptographic proof (see module docstring / test_tracked_index.py for why this distinction
#     matters: two self-consistent files can still both be wrong about the same fabricated path).
# `tracked_index` is None by default everywhere in this module (opt-in) so every existing
# fixture-level caller that builds its own ad-hoc manifest/repo_root is completely unaffected;
# only the CLI (`cmd_verify`/`cmd_check_removal`) and the "real registry" test path load and pass
# the real committed tracked_index.json.
# =============================================================================

def _git_available(repo_root: Path) -> bool:
    if shutil.which("git") is None:
        return False
    return (repo_root / ".git").exists() or (repo_root / ".git").is_file()


def _git_staged_blob_sha(repo_root: Path, rel_path: str) -> str | None:
    """Returns the git blob sha1 currently STAGED (`git ls-files --stage`, the index -- not
    working-tree content, and not HEAD) for `rel_path` at `repo_root`, or None if git is
    unavailable or the path is not staged there."""
    if not _git_available(repo_root):
        return None
    try:
        result = subprocess.run(["git", "ls-files", "--stage", "--", rel_path],
                                 cwd=repo_root, capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    line = result.stdout.strip()
    if not line:
        return None
    fields = line.split()
    if len(fields) < 2:
        return None
    return fields[1]


def _has_symlink_component(repo_root: Path, rel_path: str) -> bool:
    """Checked component-by-component from repo_root (including the leaf) -- a symlink anywhere
    in the path, not merely at the final component, is rejected. Fails closed (treats an OSError
    while stat-ing a component as if it were a symlink) rather than silently treating an
    unreadable component as safe."""
    cur = repo_root
    for part in Path(rel_path).parts:
        cur = cur / part
        try:
            if cur.is_symlink():
                return True
        except OSError:
            return True
    return False


def _sha256_file(path: Path) -> str | None:
    try:
        h = hashlib.sha256()
        with open(path, "rb") as f:
            for chunk in iter(lambda: f.read(65536), b""):
                h.update(chunk)
        return h.hexdigest()
    except OSError:
        return None


def resolve_evidence_path(repo_root: Path, rel_path, evidence_manifest: dict,
                           tracked_index: dict | None = None) -> _EvidenceResolution:
    """Admits `rel_path` only if it is a non-absolute, non-escaping, EXACT key of
    `evidence_manifest` (never a prefix match), contains no symlink at any path component, exists
    as a regular file, and its current sha256 matches the manifest's recorded digest.

    cycle4 fix: when `tracked_index` is given (not None -- opt-in, see module docstring "Tracked-
    index trust tiers"), evidence_manifest membership is no longer sufficient ON ITS OWN: `rel_path`
    must ALSO be a key of tracked_index's own snapshot, and -- when ambient git is available -- its
    CURRENTLY STAGED blob must match the snapshot's recorded blob sha1 exactly."""
    if not isinstance(rel_path, str) or not rel_path:
        return _EvidenceResolution("absent")
    if rel_path.startswith("/") or rel_path.startswith("~"):
        return _EvidenceResolution("untracked")
    parts = Path(rel_path).parts
    if not parts or ".." in parts or any(part in (".", "") for part in parts):
        return _EvidenceResolution("path_escape")
    if rel_path not in evidence_manifest:
        return _EvidenceResolution("untracked")
    if _has_symlink_component(repo_root, rel_path):
        return _EvidenceResolution("symlink")
    full = repo_root / rel_path
    if not full.is_file():
        return _EvidenceResolution("missing")
    digest = _sha256_file(full)
    expected = evidence_manifest.get(rel_path)
    if digest is None or not isinstance(expected, str) or digest != expected:
        return _EvidenceResolution("digest_mismatch")
    if tracked_index is not None:
        expected_index_keys = {"schema_version", "trust_model", "base_tree_sha1", "entries"}
        expected_trust_model = {
            "ambient_git": "git index is provenance authority",
            "gitless_export": "snapshot plus content digests is export-trust input, not independent proof",
        }
        if (not isinstance(tracked_index, dict) or set(tracked_index) != expected_index_keys
                or tracked_index.get("schema_version") != 1
                or tracked_index.get("trust_model") != expected_trust_model
                or not re.fullmatch(r"[0-9a-f]{40}", str(tracked_index.get("base_tree_sha1", "")))
                or not isinstance(tracked_index.get("entries"), dict)):
            return _EvidenceResolution("tracked_index_invalid")
        entries = tracked_index["entries"]
        if any(not isinstance(path, str) or not path or "\\" in path or Path(path).is_absolute()
               or ".." in Path(path).parts or any(part in ("", ".") for part in Path(path).parts) or
               not isinstance(blob, str) or re.fullmatch(r"[0-9a-f]{40}", blob) is None
               for path, blob in entries.items()):
            return _EvidenceResolution("tracked_index_invalid")
        if rel_path not in entries:
            return _EvidenceResolution("not_tracked_index")
        recorded_blob = entries.get(rel_path)
        live_blob = _git_staged_blob_sha(repo_root, rel_path)
        if live_blob is not None and live_blob != recorded_blob:
            return _EvidenceResolution("tracked_index_drift")
        git_inside = False
        if _git_available(repo_root):
            try:
                git_probe = subprocess.run(["git", "rev-parse", "--is-inside-work-tree"], cwd=repo_root,
                                           capture_output=True, text=True)
                git_inside = git_probe.returncode == 0 and git_probe.stdout.strip() == "true"
            except OSError:
                git_inside = False
        if live_blob is None and git_inside:
            return _EvidenceResolution("not_staged")
        if live_blob is None:
            # Gitless/no-executable tier still proves that the regular file's actual bytes are the
            # frozen tracked blob. A manifest SHA and tracked-index map may otherwise be rewritten
            # together by an attacker. Reproduce Git's canonical blob object hash directly.
            try:
                payload = full.read_bytes()
            except OSError:
                return _EvidenceResolution("missing")
            computed_blob = hashlib.sha1(
                b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload).hexdigest()
            if computed_blob != recorded_blob:
                return _EvidenceResolution("tracked_index_drift")
    return _EvidenceResolution("ok")


# =============================================================================
# Executable assertion IDs -- exact AST unittest method/function names (.py), exact anchored
# shell function names (.sh), exact anchored YAML mapping keys (.yaml/.yml), or exact markdown
# heading text (.md, weakest tier -- and per CLAUSE_LACKS_EXECUTABLE_EVIDENCE below, never
# admissible as a clause's SOLE evidence). Never substring matching.
# =============================================================================

def _python_assertion_ids(path: Path) -> set | None:
    try:
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source)
    except (OSError, UnicodeError, SyntaxError):
        return None
    ids: set = set()
    for node in tree.body:
        if isinstance(node, ast.FunctionDef):
            ids.add(node.name)
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for t in targets:
                if isinstance(t, ast.Name):
                    ids.add(t.id)
    for node in ast.walk(tree):
        if isinstance(node, ast.ClassDef):
            base_names = {b.id for b in node.bases if isinstance(b, ast.Name)}
            base_names |= {b.attr for b in node.bases if isinstance(b, ast.Attribute)}
            if "TestCase" in base_names:
                for item in node.body:
                    if isinstance(item, ast.FunctionDef) and item.name.startswith("test"):
                        ids.add(f"{node.name}.{item.name}")
    return ids


_SH_FUNC_RE = re.compile(r"^[ \t]*([A-Za-z_][A-Za-z0-9_]*)[ \t]*\([ \t]*\)[ \t]*\{", re.MULTILINE)
# Bash has no stdlib AST -- these two anchored regexes are the shell-file analogue of the .py
# AST walk: top-level variable ASSIGNMENTS (the script's own named intermediate results/config
# knobs, e.g. `KEY_OK=`, `KDUMP_CRASHKERNEL=`, `_patch=`) and long-option CLI flags declared
# as `case` branch patterns (e.g. `--topology)`), both exact-anchored, never substring.
_SH_VAR_RE = re.compile(
    r"(?:^|[;() \t])([A-Za-z_][A-Za-z0-9_]*)=", re.MULTILINE
)
_SH_LONGOPT_RE = re.compile(r"--([a-z][a-z0-9-]*)\)")


def _shell_assertion_ids(path: Path) -> set | None:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return None
    # Bash has no stdlib parser.  Strip unquoted comment tails before anchored extraction so a
    # retired assignment/function left only in `#` prose cannot remain executable evidence.
    executable_lines = []
    for line in text.splitlines():
        out_line, quote, escaped = [], None, False
        for ch in line:
            if escaped:
                out_line.append(ch); escaped = False; continue
            if ch == "\\" and quote != "'":
                out_line.append(ch); escaped = True; continue
            if ch in ("'", '"'):
                if quote is None: quote = ch
                elif quote == ch: quote = None
                out_line.append(ch); continue
            if ch == "#" and quote is None:
                break
            out_line.append(ch)
        executable_lines.append("".join(out_line))
    executable = "\n".join(executable_lines)
    ids = set(_SH_FUNC_RE.findall(executable))
    ids |= set(_SH_VAR_RE.findall(executable))
    ids |= set(_SH_LONGOPT_RE.findall(executable))
    return ids


_YAML_KEY_RE = re.compile(r"^[ \t]*([A-Za-z_][A-Za-z0-9_.\-]*)[ \t]*:", re.MULTILINE)


def _yaml_assertion_ids(path: Path) -> set | None:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return None
    # Minimal YAML lexical pass: ignore comments and all more-indented lines belonging to `|`/`>`
    # block scalars.  Mapping keys outside scalar prose remain exact anchored identifiers.
    ids, block_indent = set(), None
    for raw in text.splitlines():
        stripped = raw.lstrip(" ")
        indent = len(raw) - len(stripped)
        if block_indent is not None:
            if not stripped or indent > block_indent:
                continue
            block_indent = None
        if not stripped or stripped.startswith("#"):
            continue
        line = raw.split(" #", 1)[0]
        m = re.match(r"^[ \t]*([A-Za-z_][A-Za-z0-9_.\-]*)[ \t]*:[ \t]*(.*)$", line)
        if not m:
            continue
        ids.add(m.group(1))
        value = m.group(2).strip()
        if value.startswith("|") or value.startswith(">"):
            block_indent = indent
    return ids


def _markdown_assertion_ids(path: Path) -> set | None:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError):
        return None
    ids: set = set()
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("#"):
            heading = stripped.lstrip("#").strip()
            if heading:
                ids.add(heading)
    return ids


def _json_assertion_ids(path: Path) -> set | None:
    obj, err = safe_load_json_object(path, "ASSERTION_JSON")
    if err is not None or not isinstance(obj, dict):
        return None
    return set(obj)


_ASSERTION_EXTRACTORS = {
    ".py": _python_assertion_ids,
    ".sh": _shell_assertion_ids,
    ".yaml": _yaml_assertion_ids,
    ".yml": _yaml_assertion_ids,
    ".md": _markdown_assertion_ids,
    ".json": _json_assertion_ids,
}


def extract_assertion_ids(path: Path) -> set | None:
    """Returns the set of exact assertion ids `path` can support, or None if this file kind has
    no supported extraction (an evidence entry citing such a path can never satisfy
    assertion_ids validation)."""
    extractor = _ASSERTION_EXTRACTORS.get(path.suffix.lower())
    if extractor is None:
        return None
    return extractor(path)


def evidence_structural_violations(policies: list, repo_root: Path = REPO_ROOT,
                                    evidence_manifest: dict | None = None,
                                    tracked_index: dict | None = None) -> list:
    """Path resolves through the manifest+digest+symlink gate (plus the tracked_index trust tier
    when `tracked_index` is given -- see resolve_evidence_path / module docstring), assertion_ids
    are nonempty and each one is an exact, real identifier in the cited file, supports[] entries
    resolve to real clause_ids on the SAME policy, and every declared clause_id has at least one
    evidence entry supporting it (no orphan clauses)."""
    if evidence_manifest is None:
        evidence_manifest = load_evidence_manifest()
    out = []
    for p in policies:
        pid = p.get("policy_id")
        clause_ids = {c.get("clause_id") for c in (p.get("clauses") or []) if isinstance(c, dict)}
        evidence = p.get("evidence") or []
        if not evidence:
            out.append(Violation("EVIDENCE_EMPTY", "no evidence entries at all", policy_id=pid,
                                  path=f"$.policies[{pid}].evidence"))
        supported = set()
        for e in evidence:
            if not isinstance(e, dict):
                continue  # schema-shape catches this class separately
            path = e.get("path")
            supports = e.get("supports") or []
            assertion_ids = e.get("assertion_ids") or []

            res = resolve_evidence_path(repo_root, path, evidence_manifest, tracked_index=tracked_index)
            if res.status != "ok":
                out.append(Violation(_EVIDENCE_RESOLUTION_REASON[res.status],
                                      f"{path!r}: {res.status}", policy_id=pid, path=path))

            if not assertion_ids:
                out.append(Violation("EVIDENCE_ASSERTION_IDS_EMPTY",
                                      "evidence entry has no assertion_ids", policy_id=pid, path=path))
            elif res.status == "ok":
                available = extract_assertion_ids(repo_root / path)
                if available is None:
                    out.append(Violation("EVIDENCE_ASSERTION_KIND_UNSUPPORTED",
                                          f"{path!r} has no supported assertion-id extraction for its file kind",
                                          policy_id=pid, path=path))
                else:
                    for aid in assertion_ids:
                        if aid not in available:
                            out.append(Violation("EVIDENCE_ASSERTION_ID_NOT_FOUND",
                                                  f"assertion id {aid!r} is not an exact identifier found in "
                                                  f"{path!r}", policy_id=pid, path=path))

            for cid in supports:
                if cid not in clause_ids:
                    out.append(Violation("EVIDENCE_DANGLING_CLAUSE_REF",
                                          f"supports references clause_id {cid!r} which this policy does not declare",
                                          policy_id=pid, clause_id=cid, path=path))
                else:
                    supported.add(cid)
        for cid in sorted(clause_ids - supported):
            out.append(Violation("CLAUSE_WITHOUT_EVIDENCE", f"clause {cid!r} has no supporting evidence entry",
                                  policy_id=pid, clause_id=cid, path=f"$.policies[{pid}].clauses[{cid}]"))
    return out


def clause_executable_evidence_violations(policies: list) -> list:
    """A clause that HAS evidence but every supporting entry's path ends in `.md` (prose) is a
    structural violation -- prose may corroborate a clause but may never be its sole evidence.
    A clause with ZERO evidence is reported separately (CLAUSE_WITHOUT_EVIDENCE, above)."""
    out = []
    for p in policies:
        pid = p.get("policy_id")
        clause_ids = {c.get("clause_id") for c in (p.get("clauses") or []) if isinstance(c, dict)}
        by_clause: dict[str, list] = {cid: [] for cid in clause_ids}
        for e in p.get("evidence") or []:
            if not isinstance(e, dict):
                continue
            for cid in (e.get("supports") or []):
                if cid in by_clause:
                    by_clause[cid].append(e)
        for cid, entries in by_clause.items():
            if entries and all(str(entry.get("path") or "").lower().endswith(".md") for entry in entries):
                out.append(Violation("CLAUSE_LACKS_EXECUTABLE_EVIDENCE",
                                      f"clause {cid!r} is supported only by prose (.md) evidence -- prose "
                                      "cannot be the sole evidence for a clause", policy_id=pid, clause_id=cid,
                                      path=f"$.policies[{pid}].clauses[{cid}]"))
    return out


# =============================================================================
# Clause-specific claim bindings (cycle4 fix -- subagent-summary-1 finding 2): a real, digest-
# matching, exact identifier's mere EXISTENCE in some cited file was previously enough to support
# ANY clause referencing it -- `test_korean_only_clause_with_real_assertion_id_still_validates`
# explicitly locked this bypass in as accepted behavior (a semantically unrelated Korean clause,
# backed by `scripts/policy_registry.py:evaluate_lifecycle`, validated with zero violations).
# `.claude/policies/claim_bindings.json` fixes this by pre-registering the EXACT allowed
# (path, assertion_id) predicate set for each of the registry's own clause_ids; the registry's
# actual evidence tuples for a bound clause must equal that allowed set exactly -- not merely
# overlap with it. A clause_id absent from claim_bindings is OUT OF SCOPE (this is what lets every
# TEST_*/fixture-only clause in this test suite pass untouched -- see module docstring "TWO
# INDEPENDENT 90-day clocks" for the same opt-in-parameter design pattern applied here).
# =============================================================================

def claim_binding_violations(policies: list, claim_bindings: dict) -> list:
    """For every clause_id that IS a key of claim_bindings["bindings"] (or of claim_bindings
    itself, if it has no "bindings" wrapper), the registry's own ACTUAL (path, assertion_id)
    evidence tuples declared for that clause must equal the allowed predicate set exactly (no
    extra, no missing). A clause_id not present in claim_bindings is out of scope -- this function
    never invents a requirement for a clause nobody has registered a binding for."""
    canonical_ids = {
        str(clause.get("clause_id"))
        for policy in policies if isinstance(policy, dict)
        for clause in (policy.get("clauses") or []) if isinstance(clause, dict)
        if isinstance(clause.get("clause_id"), str) and not clause["clause_id"].startswith("TEST_")
    }
    out = []
    if canonical_ids:
        if (not isinstance(claim_bindings, dict) or set(claim_bindings) != {"schema_version", "bindings"}
                or claim_bindings.get("schema_version") != 1
                or not isinstance(claim_bindings.get("bindings"), dict)):
            return [Violation("CLAIM_BINDINGS_INVALID",
                              "canonical claim_bindings must be an exact schema_version=1 bindings object",
                              path=".claude/policies/claim_bindings.json")]
        candidate_bindings = claim_bindings["bindings"]
        if set(candidate_bindings) != canonical_ids:
            out.append(Violation(
                "CLAIM_BINDINGS_INCOMPLETE",
                f"canonical binding keys must equal all clause IDs; missing={sorted(canonical_ids-set(candidate_bindings))} "
                f"extra={sorted(set(candidate_bindings)-canonical_ids)}",
                path=".claude/policies/claim_bindings.json.bindings"))
        for cid, entries in candidate_bindings.items():
            valid = isinstance(entries, list) and bool(entries)
            if valid:
                for entry in entries:
                    if (not isinstance(entry, dict) or set(entry) != {"path", "assertion_ids"}
                            or not isinstance(entry.get("path"), str) or not entry["path"]
                            or not isinstance(entry.get("assertion_ids"), list) or not entry["assertion_ids"]
                            or any(not isinstance(aid, str) or not aid for aid in entry["assertion_ids"])):
                        valid = False
                        break
            if not valid:
                out.append(Violation("CLAIM_BINDINGS_INVALID",
                                     f"clause {cid!r} has malformed binding entries",
                                     clause_id=cid, path=f".claude/policies/claim_bindings.json.bindings.{cid}"))
    bindings = claim_bindings.get("bindings", claim_bindings) if isinstance(claim_bindings, dict) else {}
    if not isinstance(bindings, dict):
        bindings = {}
    for p in policies:
        pid = p.get("policy_id")
        clause_ids = {c.get("clause_id") for c in (p.get("clauses") or []) if isinstance(c, dict)}
        actual_by_clause: dict = {cid: set() for cid in clause_ids}
        for e in p.get("evidence") or []:
            if not isinstance(e, dict):
                continue
            path = e.get("path")
            for cid in (e.get("supports") or []):
                if cid in actual_by_clause:
                    for aid in (e.get("assertion_ids") or []):
                        actual_by_clause[cid].add((path, aid))
        for cid in sorted(clause_ids):
            if cid not in bindings:
                continue  # out of scope -- no registered binding for this clause
            allowed_entries = bindings[cid] or []
            allowed = set()
            if isinstance(allowed_entries, list):
                for entry in allowed_entries:
                    if not isinstance(entry, dict):
                        continue
                    epath = entry.get("path")
                    for aid in (entry.get("assertion_ids") or []):
                        allowed.add((epath, aid))
            actual = actual_by_clause.get(cid, set())
            if actual != allowed:
                extra = sorted(actual - allowed)
                missing = sorted(allowed - actual)
                out.append(Violation("CLAIM_BINDING_MISMATCH",
                                      f"clause {cid!r} evidence tuples do not exactly match claim_bindings.json "
                                      f"-- extra={extra} missing={missing}",
                                      policy_id=pid, clause_id=cid, path=f"$.policies[{pid}].clauses[{cid}]"))
                continue
            # Canonical policies require a distinct, executable semantic predicate whose identity
            # is derived from the stable clause ID.  This prevents a registry and its self-authored
            # binding map from colluding on an unrelated-but-real identifier. TEST_* fixture IDs
            # remain outside this production naming contract.
            if isinstance(cid, str) and not cid.startswith("TEST_"):
                dedicated = ("tests/harness/test_policy_claim_predicates.py",
                             "predicate_" + cid.replace(".", "_"))
                if dedicated not in actual:
                    out.append(Violation(
                        "CLAIM_BINDING_MISSING_DEDICATED_PREDICATE",
                        f"canonical clause {cid!r} lacks required dedicated predicate {dedicated!r}",
                        policy_id=pid, clause_id=cid,
                        path=f"$.policies[{pid}].clauses[{cid}]"))
    return out


# =============================================================================
# Removal eligibility
# =============================================================================

_TREE_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

# scripts/policy_registry.py's own repo-relative path -- included in the contract-tree digest so
# an artifact binds to the exact VALIDATOR that checked it, not merely the data it validated.
_CONTRACT_SCRIPT_REL = "scripts/policy_registry.py"


def compute_contract_tree_sha256(doc: dict, schema: dict, evidence_manifest: dict, claim_bindings: dict,
                                  tracked_index: dict, repo_root: Path = REPO_ROOT,
                                  selected_removal_artifact_path: str | None = None) -> str:
    """Deterministic content-identity digest binding a removal artifact to the EXACT contract
    state it was checked against (cycle4 fix -- subagent-summary-1 finding 3: a removal artifact's
    `tree_sha256` previously only had to be well-formed-64-hex, never bound to any real tree, so a
    stale artifact from a completely different tree/registry state was silently accepted).

    Inputs: the registry document under test, the schema, the evidence manifest, the claim-binding
    map, the tracked-index snapshot, and this validator's own source bytes -- exactly the six
    named in the remediation spec. Canonical encoding: every JSON-like input is serialized with
    `json.dumps(..., sort_keys=True, ensure_ascii=False)` (so key order in the source file never
    matters), each part is labeled and sha256'd independently, and the six labeled digests are
    joined SORTED (labels are fixed and unique) before the final sha256, so argument order cannot
    change the result.  A removal artifact cannot include its own path/digest in that result without
    self-reference. Every `removal_evidence.path` remains in the canonical registry component.
    Only the selected artifact's own manifest/tracked-index entries are omitted; sibling artifacts
    remain ordinary contract material. Policy_id, result, exact artifact digest and invocation as_of
    remain independently checked by check_removal_evidence_contract()."""
    contract_doc = copy.deepcopy(doc)
    artifact_paths = ({selected_removal_artifact_path}
                      if isinstance(selected_removal_artifact_path, str) else set())
    contract_manifest = ({path: digest for path, digest in evidence_manifest.items()
                          if path not in artifact_paths}
                         if isinstance(evidence_manifest, dict) else evidence_manifest)
    contract_tracked_index = copy.deepcopy(tracked_index)
    if isinstance(contract_tracked_index, dict) and isinstance(contract_tracked_index.get("entries"), dict):
        for path in artifact_paths:
            contract_tracked_index["entries"].pop(path, None)

    def _digest_json(o) -> str:
        return hashlib.sha256(json.dumps(o, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()

    script_path = repo_root / _CONTRACT_SCRIPT_REL
    try:
        script_bytes = script_path.read_bytes()
    except OSError:
        script_bytes = b""
    parts = [
        f"registry:{_digest_json(contract_doc)}",
        f"schema:{_digest_json(schema)}",
        f"evidence_manifest:{_digest_json(contract_manifest)}",
        f"claim_bindings:{_digest_json(claim_bindings)}",
        f"tracked_index:{_digest_json(contract_tracked_index)}",
        f"policy_registry.py:{hashlib.sha256(script_bytes).hexdigest()}",
    ]
    combined = "\n".join(sorted(parts))
    return hashlib.sha256(combined.encode("utf-8")).hexdigest()


def _load_removal_artifact(repo_root: Path, rel_path, evidence_manifest: dict,
                           tracked_index: dict | None = None):
    """Resolves `rel_path` through the SAME manifest+digest+symlink gate as any other evidence
    path, then parses it as a JSON object. Returns (artifact_dict_or_None, reason_code_or_None)."""
    res = resolve_evidence_path(repo_root, rel_path, evidence_manifest, tracked_index)
    if res.status != "ok":
        return None, _EVIDENCE_RESOLUTION_REASON[res.status]
    doc, err = safe_load_json_object(repo_root / rel_path, "REMOVAL_EVIDENCE_ARTIFACT")
    if err is not None:
        return None, "REMOVAL_EVIDENCE_ARTIFACT_UNREADABLE"
    return doc, None


def check_removal_eligibility(policy: dict, repo_root: Path = REPO_ROOT,
                               evidence_manifest: dict | None = None,
                               tracked_index: dict | None = None,
                               expected_tree_sha256: str | None = None,
                               expected_as_of: datetime.date | None = None) -> list:
    """Selected-policy-only eligibility check -- callers wanting the full contract (schema +
    registry-wide lifecycle validation FIRST, plus contract-tree/as_of identity binding) use
    cmd_check_removal / the check_removal_full helper below. This function alone is intentionally
    narrow (it is also exercised directly, fixture-style, by tests that construct a single policy
    dict in isolation).

    cycle4 fix: `expected_tree_sha256`/`expected_as_of` are opt-in (default None -- every existing
    narrow/direct caller that never passes them is completely unaffected). When given (always the
    case from check_removal_full, which alone has the doc/schema/claim_bindings/tracked_index
    needed to compute expected_tree_sha256), the artifact's own tree_sha256/as_of fields -- once
    individually shape-valid -- must equal them EXACTLY, not merely look like a digest/date."""
    if evidence_manifest is None:
        evidence_manifest = load_evidence_manifest()
    pid = policy.get("policy_id")
    out = []
    if policy.get("status") != "retired":
        out.append(Violation("REMOVAL_NOT_ELIGIBLE", "policy is not retired", policy_id=pid,
                              path=f"$.policies[{pid}].status"))
        return out
    removal_evidence = policy.get("removal_evidence") or {}
    if removal_evidence.get("status") != "contract_e2e_green":
        out.append(Violation("REMOVAL_NOT_ELIGIBLE",
                              "removal_evidence.status is not contract_e2e_green", policy_id=pid,
                              path=f"$.policies[{pid}].removal_evidence.status"))
        return out
    path = removal_evidence.get("path")
    artifact, err_code = _load_removal_artifact(repo_root, path, evidence_manifest, tracked_index)
    if artifact is None:
        out.append(Violation(err_code or "REMOVAL_NOT_ELIGIBLE",
                              "removal_evidence.path does not resolve to a readable, tracked, digest-matching "
                              "JSON artifact", policy_id=pid, path=path))
        return out
    if artifact.get("policy_id") != pid:
        out.append(Violation("REMOVAL_EVIDENCE_POLICY_ID_MISMATCH",
                              f"removal artifact policy_id {artifact.get('policy_id')!r} != {pid!r}",
                              policy_id=pid, path=path))
    if artifact.get("result") != "PASS":
        out.append(Violation("REMOVAL_EVIDENCE_RESULT_NOT_PASS",
                              f"removal artifact result {artifact.get('result')!r} is not 'PASS'",
                              policy_id=pid, path=path))
    artifact_as_of_raw = artifact.get("as_of")
    if not _is_iso_date(artifact_as_of_raw):
        out.append(Violation("REMOVAL_EVIDENCE_AS_OF_INVALID",
                              f"removal artifact as_of {artifact_as_of_raw!r} is not a valid ISO-8601 date",
                              policy_id=pid, path=path))
    elif expected_as_of is not None and _parse_iso_date(artifact_as_of_raw) != expected_as_of:
        out.append(Violation("REMOVAL_EVIDENCE_AS_OF_MISMATCH",
                              f"removal artifact as_of {artifact_as_of_raw!r} != the invocation's own "
                              f"--as-of {expected_as_of.isoformat()!r}", policy_id=pid, path=path))
    tree_sha = artifact.get("tree_sha256")
    if not isinstance(tree_sha, str) or not _TREE_SHA256_RE.match(tree_sha):
        out.append(Violation("REMOVAL_EVIDENCE_DIGEST_FIELD_INVALID",
                              f"removal artifact tree_sha256 {tree_sha!r} is not a well-formed 64-hex digest",
                              policy_id=pid, path=path))
    elif expected_tree_sha256 is not None and tree_sha != expected_tree_sha256:
        out.append(Violation("REMOVAL_EVIDENCE_TREE_MISMATCH",
                              f"removal artifact tree_sha256 does not match the exact contract tree that was "
                              f"checked (expected {expected_tree_sha256}, got {tree_sha})",
                              policy_id=pid, path=path))
    return out


def check_removal_full(doc: dict, policy_id: str, as_of: datetime.date, schema: dict,
                        repo_root: Path = REPO_ROOT, evidence_manifest: dict | None = None,
                        claim_bindings: dict | None = None, tracked_index: dict | None = None,
                        governed_prose_snapshot: dict | None = None) -> list:
    """The full removal contract (cycle2 fix): schema-shape validation, THEN full-registry
    lifecycle validation (both with the given as_of/repo_root), and ONLY if the WHOLE registry is
    clean does the selected policy's own removal_evidence get checked. A retired policy with a
    self-cycling superseded_by, a missing retirement reason, or ANY other registry-wide
    violation blocks removal -- cycle1's check-removal validated only the single selected
    record, so a self-cycle + no-reason combination returned zero violations.

    cycle4 fix (subagent-summary-1 finding 3): the selected policy's removal artifact is now bound
    to the EXACT contract tree and as_of this invocation is checking -- computed once here via
    compute_contract_tree_sha256 (using the real committed claim_bindings.json/tracked_index.json
    by default, loaded fresh when the caller does not supply them) and passed down as
    expected_tree_sha256/expected_as_of. This does NOT change the `evaluate_lifecycle` call below
    (still exactly the pre-cycle4 signature/behavior) -- the identity binding lives solely in the
    eligibility check, so it cannot introduce any new violation class into unrelated
    registry-wide lifecycle fixtures."""
    if evidence_manifest is None:
        evidence_manifest = load_evidence_manifest()
    shape = schema_violations(doc, schema)
    if shape:
        return shape
    lifecycle = evaluate_lifecycle(
        doc, as_of, repo_root, evidence_manifest,
        tracked_index=tracked_index, claim_bindings=claim_bindings,
        governed_prose_snapshot=governed_prose_snapshot)
    if lifecycle:
        return lifecycle
    by_id = {p.get("policy_id"): p for p in doc.get("policies", []) if isinstance(p, dict)}
    policy = by_id.get(policy_id)
    if policy is None:
        return [Violation("UNKNOWN_POLICY_ID", f"{policy_id!r} not found in registry", path="$.policies")]
    resolved_claim_bindings = claim_bindings if claim_bindings is not None else load_claim_bindings()
    resolved_tracked_index = tracked_index if tracked_index is not None else load_tracked_index()
    expected_tree = compute_contract_tree_sha256(doc, schema, evidence_manifest, resolved_claim_bindings,
                                                  resolved_tracked_index, repo_root,
                                                  selected_removal_artifact_path=(
                                                      policy.get("removal_evidence") or {}).get("path"))
    return check_removal_eligibility(policy, repo_root, evidence_manifest, resolved_tracked_index,
                                      expected_tree_sha256=expected_tree, expected_as_of=as_of)


def arch_variant_contract_violations(repo_root: Path = REPO_ROOT) -> list:
    """Fail-closed contract for the arch-wall variant ledger and promotion procedure.

    This deliberately validates the executable data/control plane rather than accepting the
    registry statement as its own evidence: ``resolved.json`` owns pinned variants;
    ``Dockerfile.source-build`` consumes the repo/ref build args; and workflow/SKILL ordering owns
    the HITL/evidence/re-smoke promotion path.  Underscore-prefixed ledger keys are metadata.
    """
    out = []

    def fail(code, message, path):
        out.append(Violation(code, message, policy_id="ARCH_WALL_VARIANT_LADDER", path=path))

    resolved_path = repo_root / ".claude/policies/arch_variant_ledger.json"
    resolved, err = safe_load_json_object(resolved_path, "ARCH_VARIANT_LEDGER")
    if err is not None or not isinstance(resolved, dict):
        message = err.message if err is not None else "resolved.json root must be an object"
        fail("ARCH_VARIANT_LEDGER_INVALID", message, "resolved.json")
        return out
    if set(resolved) != {"schema_version", "source_build_variants"} or resolved.get("schema_version") != 1:
        fail("ARCH_VARIANT_LEDGER_SHAPE_INVALID",
             "ledger requires exactly schema_version=1 and source_build_variants",
             ".claude/policies/arch_variant_ledger.json")
    variants = resolved.get("source_build_variants")
    if not isinstance(variants, dict):
        fail("ARCH_VARIANT_LEDGER_MISSING", ".claude/policies/arch_variant_ledger.json.source_build_variants must be an object",
             ".claude/policies/arch_variant_ledger.json.source_build_variants")
        return out

    evidence_manifest, manifest_err = safe_load_json_object(
        repo_root / ".claude/policies/evidence_manifest.json", "ARCH_EVIDENCE_MANIFEST")
    tracked_index, index_err = safe_load_json_object(
        repo_root / ".claude/policies/tracked_index.json", "ARCH_TRACKED_INDEX")
    if (manifest_err is not None or index_err is not None or
            not isinstance(evidence_manifest, dict) or not isinstance(tracked_index, dict)):
        fail("ARCH_VARIANT_TRUST_INPUT_INVALID", "evidence manifest and tracked index must be readable objects",
             ".claude/policies")
        return out

    def validate_bound_artifact(name, item, field, kind, pfx):
        pointer = item.get(field)
        if not isinstance(pointer, dict) or set(pointer) != {"path", "sha256"}:
            fail("ARCH_VARIANT_ARTIFACT_POINTER_INVALID",
                 f"{name!r} {field} must be an exact path/sha256 object", f"{pfx}.{field}")
            return
        rel = pointer.get("path")
        digest = pointer.get("sha256")
        if not isinstance(rel, str):
            fail("ARCH_VARIANT_ARTIFACT_PATH_INVALID", f"{name!r} {field} requires a path string",
                 f"{pfx}.{field}.path")
            return
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            fail("ARCH_VARIANT_ARTIFACT_DIGEST_INVALID", f"{name!r} {field} requires sha256",
                 f"{pfx}.{field}.sha256")
            return
        resolution = resolve_evidence_path(repo_root, rel, evidence_manifest, tracked_index)
        if resolution.status != "ok" or _sha256_file(repo_root / rel) != digest:
            fail("ARCH_VARIANT_ARTIFACT_UNBOUND",
                 f"{name!r} {field} is not regular, manifest/index tracked, and digest-bound",
                 f"{pfx}.{field}.path")
            return
        artifact, artifact_err = safe_load_json_object(repo_root / rel, "ARCH_VARIANT_ARTIFACT")
        if artifact_err is not None or not isinstance(artifact, dict):
            message = artifact_err.message if artifact_err is not None else "artifact root must be an object"
            fail("ARCH_VARIANT_ARTIFACT_INVALID", message, f"{pfx}.{field}.path")
            return
        common_fields = {"schema_version", "kind", "result", "variant_id", "vllm_repo", "vllm_ref",
                         "track", "architecture", "image_tag"}
        kind_fields = ({"approved_by", "source_evidence", "source_path", "source_sha256", "approved_scope"}
                       if kind == "arch_variant_approval" else {"existing_models"})
        if artifact.get("schema_version") != 1 or set(artifact) != common_fields | kind_fields:
            fail("ARCH_VARIANT_ARTIFACT_SHAPE_INVALID",
                 f"{name!r} {field} must match the closed schema_version=1 {kind} shape",
                 f"{pfx}.{field}.path")
        expected = {
            "kind": kind, "result": "PASS", "variant_id": item.get("variant_id"),
            "vllm_repo": item.get("vllm_repo"), "vllm_ref": item.get("vllm_ref"),
            "track": item.get("track"), "architecture": item.get("architecture"),
            "image_tag": item.get("image_tag"),
        }
        if any(artifact.get(key) != value for key, value in expected.items()):
            fail("ARCH_VARIANT_ARTIFACT_BINDING_MISMATCH",
                 f"{name!r} {field} must PASS and bind the exact variant/ref/track/architecture/image",
                 f"{pfx}.{field}.path")
        if kind == "arch_variant_approval":
            approved_by = artifact.get("approved_by")
            approval_match = (re.fullmatch(r"HITL:(plan_\d{8})", approved_by)
                              if isinstance(approved_by, str) else None)
            source_evidence = artifact.get("source_evidence")
            source_path = artifact.get("source_path")
            source_sha256 = artifact.get("source_sha256")
            approved_scope = artifact.get("approved_scope")
            if approval_match is None:
                fail("ARCH_VARIANT_APPROVAL_IDENTITY_MISSING", f"{name!r} approval lacks explicit HITL identity",
                     f"{pfx}.{field}.path")
            plan_id = approval_match.group(1) if approval_match else ""
            expected_source = (f"{plan_id} §3 R1 primary-source fork inspection and "
                               "same-hardware community evidence")
            expected_scope = ("historical 0.23.0 rollback variant; serving smoke remains "
                              "final arbitration")
            source_ok = source_evidence == expected_source
            expected_source_path = ("docs/plan/plan_26062818_RouteB_jasl-fork_SM12x_"
                                    "DeepSeek-V4-Flash_2노드서빙.md")
            source_path_value = source_path if isinstance(source_path, str) else ""
            source_resolution = (resolve_evidence_path(repo_root, source_path_value, evidence_manifest, tracked_index)
                                 if source_path_value else _EvidenceResolution("absent"))
            try:
                source_text = (repo_root / source_path_value).read_text(encoding="utf-8") if source_resolution.status == "ok" else ""
            except (OSError, UnicodeDecodeError):
                source_text = ""
            source_artifact_ok = (
                source_path == expected_source_path
                and isinstance(source_sha256, str)
                and re.fullmatch(r"[0-9a-f]{64}", source_sha256) is not None
                and source_resolution.status == "ok"
                and _sha256_file(repo_root / source_path_value) == source_sha256
                and "## 3. R1 해소 — 비-repack MoE 경로" in source_text
                and "포크 소스를 직독해 **확정**" in source_text
                and "**HUMMING** (fused grouped MoE)" in source_text
            )
            scope_ok = approved_scope == expected_scope
            if not source_ok or not source_artifact_ok or not scope_ok:
                fail("ARCH_VARIANT_APPROVAL_GROUNDING_MISSING",
                     f"{name!r} approval must bind the HITL plan section/rationale and final-arbitration scope",
                     f"{pfx}.{field}.path")
        if kind == "arch_variant_regression":
            models = artifact.get("existing_models")
            if (not isinstance(models, list) or not models or
                    any(not isinstance(model, str) or not model.strip() for model in models)):
                fail("ARCH_VARIANT_REGRESSION_MODELS_MISSING",
                     f"{name!r} regression requires a non-empty list of model result identities",
                     f"{pfx}.{field}.path")

    metadata_keys = {"_note", "_deprecation_0.24.0"}
    active = []
    for name, item in variants.items():
        if name.startswith("_"):
            if name not in metadata_keys or not isinstance(item, str) or not item.strip():
                fail("ARCH_VARIANT_METADATA_KEY_INVALID",
                     f"{name!r} is not an allowlisted scalar metadata key",
                     f".claude/policies/arch_variant_ledger.json.source_build_variants.{name}")
            continue
        pfx = f".claude/policies/arch_variant_ledger.json.source_build_variants.{name}"
        if not isinstance(item, dict):
            fail("ARCH_VARIANT_ENTRY_INVALID", f"{name!r} must be an object", pfx)
            continue
        allowed_entry_fields = {"variant_id", "architecture", "vllm_repo", "vllm_ref", "tag",
                                "image_tag", "track", "evidence", "status", "regression_evidence"}
        unknown_fields = set(item) - allowed_entry_fields
        if unknown_fields:
            fail("ARCH_VARIANT_ENTRY_UNKNOWN_FIELD",
                 f"{name!r} contains unknown fields {sorted(unknown_fields)}", pfx)
        for field in ("variant_id", "architecture", "vllm_repo", "vllm_ref", "image_tag", "track", "status"):
            if not isinstance(item.get(field), str) or not item[field].strip():
                fail("ARCH_VARIANT_FIELD_MISSING", f"{name!r} requires non-empty {field}", f"{pfx}.{field}")
        ref = item.get("vllm_ref", "")
        if not re.fullmatch(r"[0-9a-f]{40}", ref):
            fail("ARCH_VARIANT_REF_NOT_SHA", f"{name!r} vllm_ref must be a 40-hex commit SHA", f"{pfx}.vllm_ref")
        track = item.get("track", "")
        architecture = item.get("architecture", "")
        image_tag = item.get("image_tag", "")
        suffix = track.removeprefix("source-") if isinstance(track, str) else ""
        if not suffix or architecture != suffix or not image_tag.endswith(f"-source-{suffix}"):
            fail("ARCH_VARIANT_NOT_SUPERSET_TAG", f"{name!r} image_tag must end in -source-<track suffix>",
                 f"{pfx}.image_tag")
        image_grammar = r"easy-vllm:\d+\.\d+\.\d+-cu\d+-(?:aarch64|x86_64)-source-sm\d+[a-z0-9]*"
        if not re.fullmatch(image_grammar, image_tag):
            fail("ARCH_VARIANT_MODEL_KEYED_IMAGE",
                 f"{name!r} image_tag must use the architecture-only canonical grammar",
                 f"{pfx}.image_tag")
        validate_bound_artifact(name, item, "evidence", "arch_variant_approval", pfx)
        status = item.get("status", "")
        deprecation_versions = sorted(key.removeprefix("_deprecation_") for key in metadata_keys
                                      if key.startswith("_deprecation_"))
        image_version_match = re.match(r"easy-vllm:(\d+\.\d+\.\d+)-", image_tag)
        expected_superseded = (f"SUPERSEDED@{deprecation_versions[0]}; retained only as the "
                               f"{image_version_match.group(1)} last-good rollback anchor"
                               if len(deprecation_versions) == 1 and image_version_match else None)
        superseded = isinstance(status, str) and status == expected_superseded
        if isinstance(status, str) and status.startswith("SUPERSEDED") and not superseded:
            fail("ARCH_VARIANT_STATUS_INVALID",
                 f"{name!r} malformed superseded status must not hide an active candidate",
                 f"{pfx}.status")
        if not superseded:
            active.append((name, item))

    if len(active) > 1:
        fail("ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS",
             f"only one variant track may be active, got {[name for name, _ in active]}",
             ".claude/policies/arch_variant_ledger.json.source_build_variants")
    for name, item in active:
        pfx = f".claude/policies/arch_variant_ledger.json.source_build_variants.{name}"
        if item.get("status") != "VALIDATED":
            fail("ARCH_VARIANT_ACTIVE_NOT_VALIDATED", f"active variant {name!r} lacks VALIDATED status",
                 f"{pfx}.status")
        if not isinstance(item.get("regression_evidence"), dict):
            fail("ARCH_VARIANT_REGRESSION_EVIDENCE_MISSING",
                 f"active variant {name!r} requires existing-model regression re-smoke evidence",
                 f"{pfx}.regression_evidence")
        else:
            validate_bound_artifact(name, item, "regression_evidence", "arch_variant_regression", pfx)

    dockerfile_rel = ".claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template"
    dockerfile_path = repo_root / dockerfile_rel
    try:
        dockerfile = dockerfile_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        fail("ARCH_VARIANT_DOCKERFILE_UNREADABLE", str(exc), dockerfile_rel)
        dockerfile = ""
    required_docker = ("ARG VLLM_REPO=", "ARG VLLM_REF=", "--filter=blob:none",
                       "checkout --detach ${VLLM_REF}")
    if dockerfile and not all(token in dockerfile for token in required_docker):
        fail("ARCH_VARIANT_SOURCE_OVERRIDE_UNWIRED", "source template lacks pinned repo/ref checkout wiring",
             dockerfile_rel)

    try:
        workflow = (repo_root / ".claude/rules/workflow.md").read_text(encoding="utf-8")
        skill = (repo_root / ".claude/skills/upstream-version-watch/SKILL.md").read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        fail("ARCH_VARIANT_PROCEDURE_UNREADABLE", str(exc), ".claude/rules/workflow.md")
        return out
    ladder = "deps-패치 → 소스-게이트 패치 → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체"
    if ladder not in workflow:
        fail("ARCH_VARIANT_LADDER_ORDER_MISSING", "fixed no-skip ladder is missing", ".claude/rules/workflow.md")
    ordered = ("참조-그라운디드 확증", "testlog 기록 + 사람 승인", "source_build_variants",
               "Dockerfile build-arg 파라미터화", "clean 빌드(양노드) → 스모크")
    try:
        positions = [workflow.index(token, workflow.index("stock-구조적-불가")) for token in ordered]
        if positions != sorted(positions):
            raise ValueError("out of order")
    except ValueError:
        fail("ARCH_VARIANT_HITL_EVIDENCE_ORDER_INVALID",
             "reference evidence -> testlog/HITL -> ledger -> build args -> clean smoke order is required",
             ".claude/rules/workflow.md")
    if "기존모델 회귀 재스모크" not in skill or "단일 변종-트랙" not in skill or "무증거 오버라이드 금지" not in skill:
        fail("ARCH_VARIANT_PROMOTION_GUARDS_MISSING",
             "procedure must require existing-model regression re-smoke, one active track, and evidence",
             ".claude/skills/upstream-version-watch/SKILL.md")
    return out


def evaluate_lifecycle(doc: dict, as_of: datetime.date, repo_root: Path = REPO_ROOT,
                        evidence_manifest: dict | None = None, tracked_index: dict | None = None,
                        claim_bindings: dict | None = None,
                        governed_prose_snapshot: dict | None = None) -> list:
    """Runs every Python-level (non-schema) check and returns the combined violation list. Does
    NOT run schema_violations -- callers that also want shape validation call that separately
    (cmd_verify does both, in that order).

    cycle4 fix: `tracked_index`/`claim_bindings` are opt-in (default None), threaded straight into
    evidence_structural_violations / claim_binding_violations respectively -- every existing
    fixture-level test that never passes them exercises EXACTLY the pre-cycle4 check set (see
    module docstring "Tracked-index trust tiers" / "Clause-specific claim bindings" for why this
    is opt-in rather than always-on)."""
    if evidence_manifest is None:
        evidence_manifest = load_evidence_manifest()
    policies = [p for p in doc.get("policies", []) if isinstance(p, dict)]
    out = []
    out += duplicate_id_violations(policies)
    out += id_pattern_violations(policies)
    out += provenance_violations(policies)
    out += remove_when_violations(policies)
    out += review_shape_violations(policies)
    out += review_due_arithmetic_violations(policies)
    out += last_reproduced_violations(policies)
    out += date_chronology_violations(policies, as_of)
    out += unreproduced_90_day_violations(policies, as_of)
    out += as_of_state_violations(policies, as_of)
    out += status_enum_violations(policies)
    out += retirement_metadata_closure_violations(policies)
    out += evidence_structural_violations(policies, repo_root, evidence_manifest, tracked_index=tracked_index)
    out += clause_executable_evidence_violations(policies)
    if claim_bindings is not None:
        out += claim_binding_violations(policies, claim_bindings)
    if governed_prose_snapshot is not None:
        out += governed_prose_snapshot_violations(governed_prose_snapshot, repo_root)
    if any(p.get("policy_id") == "ARCH_WALL_VARIANT_LADDER" for p in policies):
        out += arch_variant_contract_violations(repo_root)
    return out


# =============================================================================
# CLI
# =============================================================================

class _PolicyRegistryArgumentParser(argparse.ArgumentParser):
    """Every usage error (missing subcommand, missing required flag, unrecognized argument)
    honors the same stable-JSON-on-stdout / exit-2 contract as any other invalid-input case,
    rather than argparse's default prose-to-stderr + bare exit(2)."""

    def error(self, message: str) -> None:
        _emit([Violation("CLI_USAGE_ERROR", f"invalid command-line invocation: {message}")], 2)


def _emit(violations: list, exit_code: int) -> None:
    obj = {
        "schema_version": SCHEMA_VERSION,
        "violation_count": len(violations),
        "violations": [v.to_dict() for v in violations],
        "exit_code": exit_code,
    }
    print(json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2))
    raise SystemExit(exit_code)


def _load_inputs_or_emit(args: argparse.Namespace):
    """Shared safe-loading sequence for both verify and check-removal: --as-of, registry, schema,
    evidence-manifest, tracked-index, claim-bindings -- each failure emits the stable JSON
    envelope (exit 2) and never raises. Returns (repo_root, as_of, doc, schema, evidence_manifest,
    tracked_index, claim_bindings) on success (never returns on failure -- _emit always raises
    SystemExit).

    cycle4 fix: --tracked-index/--claim-bindings default to living INSIDE the effective repo_root
    (`<repo_root>/.claude/policies/{tracked_index,claim_bindings}.json`) rather than always
    resolving to THIS project's own committed files regardless of --repo-root -- unlike
    --registry/--schema/--evidence-manifest (which stay script-relative-default, unchanged, for
    full backward compatibility with every existing caller). This is a deliberate asymmetry: these
    two files each attest to facts about the repo_root BEING CHECKED (its own git-staged state /
    its own clause-evidence contract), so a repo export that lacks its own copy must fail closed
    here -- it must not silently inherit this project's trust anchors (subagent-summary-1
    finding 1's exact reproduction: a custom, non-git temp repo with only a self-authored evidence
    manifest and no trusted tracked_index of its own)."""
    repo_root = Path(args.repo_root).resolve() if args.repo_root else REPO_ROOT
    registry_path = Path(args.registry).resolve() if args.registry else REGISTRY_PATH
    schema_path = Path(args.schema).resolve() if args.schema else SCHEMA_PATH
    manifest_path = Path(args.evidence_manifest).resolve() if getattr(args, "evidence_manifest", None) \
        else EVIDENCE_MANIFEST_PATH
    tracked_index_path = Path(args.tracked_index).resolve() if getattr(args, "tracked_index", None) \
        else (repo_root / TRACKED_INDEX_REL)
    claim_bindings_path = Path(args.claim_bindings).resolve() if getattr(args, "claim_bindings", None) \
        else (repo_root / CLAIM_BINDINGS_REL)
    governed_snapshot_path = Path(args.governed_prose_snapshot).resolve() \
        if getattr(args, "governed_prose_snapshot", None) \
        else (repo_root / GOVERNED_PROSE_SNAPSHOT_REL)

    try:
        as_of = datetime.date.fromisoformat(args.as_of)
    except (ValueError, TypeError):
        _emit([Violation("BAD_AS_OF", f"--as-of {args.as_of!r} is not a valid ISO-8601 date")], 2)

    doc, err = safe_load_json_object(registry_path, "REGISTRY")
    if err is not None:
        _emit([err], 2)

    schema, err = safe_load_json_object(schema_path, "SCHEMA")
    if err is not None:
        _emit([err], 2)

    evidence_manifest, err = safe_load_json_object(manifest_path, "EVIDENCE_MANIFEST")
    if err is not None:
        _emit([err], 2)

    tracked_index, err = safe_load_json_object(tracked_index_path, "TRACKED_INDEX")
    if err is not None:
        _emit([err], 2)

    claim_bindings, err = safe_load_json_object(claim_bindings_path, "CLAIM_BINDINGS")
    if err is not None:
        _emit([err], 2)

    governed_prose_snapshot, err = safe_load_json_object(
        governed_snapshot_path, "GOVERNED_PROSE_SNAPSHOT")
    if err is not None:
        _emit([err], 2)

    return (repo_root, as_of, doc, schema, evidence_manifest, tracked_index, claim_bindings,
            governed_prose_snapshot)


def cmd_verify(args: argparse.Namespace) -> None:
    (repo_root, as_of, doc, schema, evidence_manifest, tracked_index, claim_bindings,
     governed_prose_snapshot) = _load_inputs_or_emit(args)
    shape_violations = schema_violations(doc, schema)
    if shape_violations:
        _emit(shape_violations, 2)
        return
    lifecycle_violations = evaluate_lifecycle(
        doc, as_of, repo_root, evidence_manifest, tracked_index=tracked_index,
        claim_bindings=claim_bindings, governed_prose_snapshot=governed_prose_snapshot)
    _emit(lifecycle_violations, 1 if lifecycle_violations else 0)


def cmd_check_removal(args: argparse.Namespace) -> None:
    (repo_root, as_of, doc, schema, evidence_manifest, tracked_index, claim_bindings,
     governed_prose_snapshot) = _load_inputs_or_emit(args)
    violations = check_removal_full(doc, args.policy_id, as_of, schema, repo_root, evidence_manifest,
                                     claim_bindings=claim_bindings, tracked_index=tracked_index,
                                     governed_prose_snapshot=governed_prose_snapshot)
    # cycle4 fix (subagent-summary-1 finding 6): this module's own documented exit-code contract
    # already lists "unknown --policy-id" under exit 2 (invalid-input) -- the implementation
    # previously fell through to the generic "any violation -> exit 1" branch instead.
    if any(v.reason_code == "UNKNOWN_POLICY_ID" for v in violations):
        _emit(violations, 2)
        return
    _emit(violations, 1 if violations else 0)


def _self_test() -> int:
    """Deterministic in-memory round-trip -- no filesystem/registry dependency. Exercises the
    date-chronology, decoupled 90-day/as-of, and retirement-closure logic directly."""
    ok_policy = {
        "policy_id": "SELFTEST_OK", "owner": "x", "statement": "s", "scope": ["single-node"],
        "origin_failure": "plan_x",
        "clauses": [{"clause_id": "SELFTEST_OK.C1", "statement": "mentions selftest keyword uniquely"}],
        "evidence": [{"path": "scripts/policy_registry.py", "supports": ["SELFTEST_OK.C1"],
                      "assertion_ids": ["evaluate_lifecycle"]}],
        "added_at": "2026-01-01", "last_reproduced": None,
        "remove_when": "Removable only once this self-test itself is deleted from the module.",
        "status": "candidate",
        "review": {"state": "current", "last_reviewed_at": "2026-01-15", "next_review_due": "2026-04-15",
                   "reviewer": "selftest"},
    }
    manifest = {"scripts/policy_registry.py": _sha256_file(Path(__file__).resolve())}
    as_of = datetime.date(2026, 1, 15)
    violations = evaluate_lifecycle({"policies": [ok_policy]}, as_of, REPO_ROOT, manifest)
    assert violations == [], f"self-test OK fixture unexpectedly flagged: {[v.to_dict() for v in violations]}"

    bad_policy = dict(ok_policy, policy_id="SELFTEST_BAD",
                       clauses=[{"clause_id": "SELFTEST_BAD.C1", "statement": "x"}],
                       evidence=[{"path": "scripts/policy_registry.py", "supports": ["SELFTEST_BAD.C1"],
                                  "assertion_ids": ["evaluate_lifecycle"]}],
                       added_at="2025-01-01",
                       last_reproduced="never", remove_when="xxxxxxxxxxxxxxxxxxxx",
                       status="active",
                       review={"state": "current", "last_reviewed_at": "2025-01-01",
                               "next_review_due": "2025-04-01", "reviewer": "selftest"})
    violations = evaluate_lifecycle({"policies": [bad_policy]}, as_of, REPO_ROOT, manifest)
    codes = {v.reason_code for v in violations}
    expected = {"LAZY_REMOVE_WHEN", "BAD_LAST_REPRODUCED_DATE", "UNREPRODUCED_90_DAY_NOT_CANDIDATE",
                "AS_OF_STATE_MISMATCH"}
    assert expected <= codes, f"self-test BAD fixture missing expected codes: {expected - codes}, got {codes}"
    print("[policy_registry] --self-test PASS")
    return 0


def main() -> None:
    ap = _PolicyRegistryArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="command")

    p_verify = sub.add_parser("verify")
    p_verify.add_argument("--as-of", required=True, help="YYYY-MM-DD, required, no wall-clock default")
    p_verify.add_argument("--registry", default=None)
    p_verify.add_argument("--schema", default=None)
    p_verify.add_argument("--evidence-manifest", default=None)
    p_verify.add_argument("--tracked-index", default=None,
                           help="default: <repo-root>/.claude/policies/tracked_index.json")
    p_verify.add_argument("--claim-bindings", default=None,
                           help="default: <repo-root>/.claude/policies/claim_bindings.json")
    p_verify.add_argument("--governed-prose-snapshot", default=None,
                          help="default: <repo-root>/.claude/policies/governed_prose_snapshot.json")
    p_verify.add_argument("--repo-root", default=None)
    p_verify.set_defaults(func=cmd_verify)

    p_removal = sub.add_parser("check-removal")
    p_removal.add_argument("--policy-id", required=True)
    p_removal.add_argument("--as-of", required=True, help="YYYY-MM-DD, required, no wall-clock default")
    p_removal.add_argument("--registry", default=None)
    p_removal.add_argument("--schema", default=None)
    p_removal.add_argument("--evidence-manifest", default=None)
    p_removal.add_argument("--tracked-index", default=None,
                            help="default: <repo-root>/.claude/policies/tracked_index.json")
    p_removal.add_argument("--claim-bindings", default=None,
                            help="default: <repo-root>/.claude/policies/claim_bindings.json")
    p_removal.add_argument("--governed-prose-snapshot", default=None,
                           help="default: <repo-root>/.claude/policies/governed_prose_snapshot.json")
    p_removal.add_argument("--repo-root", default=None)
    p_removal.set_defaults(func=cmd_check_removal)

    args = ap.parse_args()
    if args.self_test:
        raise SystemExit(_self_test())
    if args.command == "verify":
        cmd_verify(args)
    elif args.command == "check-removal":
        cmd_check_removal(args)
    else:
        _emit([Violation("CLI_USAGE_ERROR", "no subcommand given -- expected 'verify' or 'check-removal'")], 2)


if __name__ == "__main__":
    main()
