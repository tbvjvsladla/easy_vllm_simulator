#!/usr/bin/env python3
"""policy_registry.py -- stdlib-only validator/auditor for .claude/policies/registry.yaml
(plan_26072506 Phase 4, review-cycle2 remediation -- subagent-summary-0/1-20260725_213652).

`registry.yaml` is plain JSON text (a valid YAML 1.2 subset) -- this module, and every consumer
in this repo, parses it with the stdlib `json` module only. PyYAML is not a dependency anywhere
in this project.

Design (cycle2):
  - Schema-shape validation reuses the sibling completion_gate.py generic Draft-07-subset
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
  - Evidence tracking is GIT-NATIVE -- git is the single provenance authority (2026-09-03
    G2-a, plan_26090222). This replaces the evidence_manifest.json + tracked_index.json ledger
    pair, which re-derived (sha256 / blob sha1) bytes git already holds: two self-authored
    files that an editor had to re-stamp by hand on every change, and that could be rewritten
    together. A path is admissible only if: it is a non-absolute, non-escaping (no `..`
    component) repo-relative path; no path component (including the leaf) is a symlink
    (checked component-by-component from repo_root, fail-closed on any stat error -- git does
    NOT check this for us, which is why the symlink gate survives the rewrite); it exists as a
    regular file; it is TRACKED in the repo_root's own git index; and its working-tree bytes
    hash to exactly the blob that index holds (a tampered-after-review file is caught, not
    silently trusted because the path still resolves). No usable git -- no executable, or a
    repo_root that is not a work tree -- is EVIDENCE_GIT_UNAVAILABLE: fail-closed, never a
    silent pass.
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
        [--claim-bindings <path>] [--repo-root <path>]
    python3 policy_registry.py check-removal --policy-id <ID> --as-of YYYY-MM-DD
        [--registry <path>] [--schema <path>] [--claim-bindings <path>] [--repo-root <path>]
    python3 policy_registry.py --self-test

--claim-bindings defaults to <repo-root>/.claude/policies/claim_bindings.json (repo-root-
relative -- NOT this script's own fixed location, unlike --registry/--schema) -- see
_load_inputs_or_emit's docstring.

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

REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "policy-registry.schema.json"
REGISTRY_PATH = REPO_ROOT / ".claude" / "policies" / "registry.yaml"
CLAIM_BINDINGS_PATH = REPO_ROOT / ".claude" / "policies" / "claim_bindings.json"

# repo-relative (no leading REPO_ROOT) -- used to build the CLI's repo_root-relative default
# resolution (cycle4 fix: a claim_bindings default must live INSIDE whatever --repo-root is being
# checked, not always resolve to THIS project's own file, or a custom/export repo that lacks its
# own copy would silently inherit this project's trust anchors).
CLAIM_BINDINGS_REL = ".claude/policies/claim_bindings.json"

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
# `policy:<ID>` citation scanning -- exercised by owner-local production `runtime_selftest.py` so
# there is one broad-capture-then-validate implementation.
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
# Safe, hermetic JSON loading -- one implementation every CLI entry point shares (cycle2 fix:
# cycle1 let OSError/UnicodeDecodeError/JSONDecodeError raise uncaught tracebacks with process
# exit 1, contradicting this module's own documented exit-2 invalid-input contract).
# =============================================================================

def safe_load_json_object(path: Path, label: str) -> tuple[dict | None, Violation | None]:
    """Reads `path`, decodes as UTF-8, parses as JSON, and requires the parsed root to be a JSON
    object (dict) -- returns (doc, None) on success or (None, Violation) on ANY failure mode
    (missing/unreadable file, invalid UTF-8, malformed JSON, non-object root incl. a JSON array).
    `label` becomes the reason_code prefix (e.g. "REGISTRY", "SCHEMA", "CLAIM_BINDINGS")."""
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


def load_claim_bindings(path: Path = CLAIM_BINDINGS_PATH) -> dict:
    doc, err = safe_load_json_object(path, "CLAIM_BINDINGS")
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
    keyword-value shapes this project's generic engine (the sibling completion_gate.py
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
    "git_unavailable": "EVIDENCE_GIT_UNAVAILABLE",
}


# =============================================================================
# Git is the single provenance authority (2026-09-03 G2-a, plan_26090222 -- supersedes the
# cycle4 "tracked-index trust tiers" comment that used to stand here). The problem cycle4 was
# solving is real and unchanged: a caller-supplied evidence_manifest.json was NOT independent
# proof that a path is actually tracked -- a self-authored manifest can list (and digest-match)
# any file, including one that was never staged at all. Cycle4's answer was a SECOND
# self-authored file (.claude/policies/tracked_index.json, path -> git blob sha1) cross-checked
# against `git ls-files --stage`, with a gitless fallback that re-derived git's own blob hash
# from worktree bytes.
#
# That answer carried the ledgers as a duplicate layer: every column in both files was bytes git
# already holds, re-stamped by hand on every edit, and the ledger's own trust_model graded the
# gitless tier as "export-trust input, not independent proof" -- i.e. the repo already declared
# tier 2 non-probative. The rewrite keeps the authority cycle4 identified (the git index) and
# drops the transcription of it:
#   - TRACKED  : `git ls-files --stage -- <rel>` must return a blob for the path in THIS
#                repo_root's index. Not staged (or not a repo) is not evidence.
#   - NO GIT   : EVIDENCE_GIT_UNAVAILABLE.
#
# 2026-09-05 (plan_26090516 3-7 / audit_26090515 G-C1): the UNDRIFTED tier is gone. It compared
# `git hash-object` (worktree) against the indexed blob -- a comparison git itself already
# answers, and whose consumer-facing form is `git status --porcelain`. Asking it a second time
# inside the policy gate had one measurable effect: any file being edited turned the whole
# harness RED until it was staged, so `git add` became a ritual performed to satisfy a checker
# rather than an act of staging evidence. Drift is a git question; ask git.
# (What survives is membership -- "does this repo's index carry these bytes at all" -- which is
# not a re-recorded digest but the tracked/untracked fact itself.) There is no fallback tier -- a repo_root with no git
#                simply cannot prove provenance, and saying so is fail-closed. (The gitless
#                CONSUMER that does exist -- hint_tag's read-only `match`/catalogue path -- never
#                enters this module; `require_git_repository()` gates the rest of hint_tag, and
#                the governance harness runs only in a git clone.)
# The symlink gate is NOT delegated to git: git tracks a symlink as its own blob, so a symlinked
# component would resolve as "tracked and undrifted" while pointing outside the reviewed tree.
# `evidence_manifest`/`tracked_index` parameters are gone from every signature in this module --
# there is nothing left to opt into, so fixtures and the CLI now exercise the identical gate.
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


def _git_inside_work_tree(repo_root: Path) -> bool:
    """True only when a git executable exists AND `repo_root` really is inside a git work tree.
    Every provenance answer in this module is gated on this: a False here is reported as
    EVIDENCE_GIT_UNAVAILABLE, never silently treated as 'nothing to check'."""
    if not _git_available(repo_root):
        return False
    try:
        probe = subprocess.run(["git", "rev-parse", "--is-inside-work-tree"], cwd=repo_root,
                                capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return False
    return probe.returncode == 0 and probe.stdout.strip() == "true"


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


def resolve_evidence_path(repo_root: Path, rel_path) -> _EvidenceResolution:
    """Admits `rel_path` only if it is a non-absolute, non-escaping repo-relative path that
    contains no symlink at any path component, exists as a regular file, and is TRACKED in
    `repo_root`'s own git index.

    2026-09-03 (G2-a, plan_26090222): git is the single authority -- the evidence_manifest sha256
    comparison and the tracked_index membership/blob comparison are gone, along with the gitless
    fallback tier (see the section comment above for why). 2026-09-05 (G-C1): the worktree-vs-index
    blob comparison is gone too -- drift is `git status --porcelain`'s answer, not a second gate. The order below is deliberate: the
    cheap, git-independent structural rejections run first so a malformed or symlinked path never
    reaches a subprocess, and the git verdict is the last word rather than an optional extra."""
    if not isinstance(rel_path, str) or not rel_path:
        return _EvidenceResolution("absent")
    if rel_path.startswith("/") or rel_path.startswith("~"):
        return _EvidenceResolution("untracked")
    parts = Path(rel_path).parts
    if not parts or ".." in parts or any(part in (".", "") for part in parts):
        return _EvidenceResolution("path_escape")
    if _has_symlink_component(repo_root, rel_path):
        return _EvidenceResolution("symlink")
    full = repo_root / rel_path
    if not full.is_file():
        return _EvidenceResolution("missing")
    if not _git_inside_work_tree(repo_root):
        return _EvidenceResolution("git_unavailable")
    if _git_staged_blob_sha(repo_root, rel_path) is None:
        # Regular file, but git does not carry it: an unstaged/ignored path is not evidence.
        return _EvidenceResolution("untracked")
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


def evidence_structural_violations(policies: list, repo_root: Path = REPO_ROOT) -> list:
    """Path resolves through the git+symlink gate (see resolve_evidence_path / module docstring:
    tracked in repo_root's git index, working-tree bytes undrifted from it, no symlink component),
    assertion_ids are nonempty and each one is an exact, real identifier in the cited file,
    supports[] entries resolve to real clause_ids on the SAME policy, and every declared clause_id
    has at least one evidence entry supporting it (no orphan clauses)."""
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

            res = resolve_evidence_path(repo_root, path)
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
# backed by this validator's `evaluate_lifecycle`, validated with zero violations).
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
                dedicated = (".claude/policies/predicates/claim_predicates.py",
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

# This validator's repo-relative path -- included in the contract-tree digest so
# an artifact binds to the exact VALIDATOR that checked it, not merely the data it validated.
_CONTRACT_SCRIPT_REL = ".claude/policies/runtime/policy_registry.py"


_CONTRACT_TREE_SCOPE = ".claude"


def _git_contract_tree_digest(repo_root: Path, exclude_paths: set) -> str:
    """sha256 over `git ls-tree -r -z HEAD -- .claude` reduced to "<blob sha1> <path>" lines, minus
    the selected removal artifact's own line.

    This is the git tree identity that replaces the two ledger components (2026-09-03 G2-a): the
    same fact -- "which exact bytes does every tracked contract file hold" -- read from git rather
    than from a hand-stamped transcription of git. `-z` is mandatory: without it git applies
    core.quotePath and a non-ASCII path would produce a DIFFERENT line for identical content
    depending on config, which is exactly the false-drift class this project has already been
    bitten by. A whole-tree id (`git rev-parse HEAD^{tree}`) is deliberately NOT used: the removal
    artifact itself lives in the tree, so an artifact would have to contain a digest of itself.
    Listing lines can be filtered; a tree id cannot.

    Returns the literal sentinel "unavailable" when git cannot answer. That sentinel is not a
    64-hex digest and can never equal a git-backed one, so check_removal_eligibility's exact
    comparison fails closed instead of silently accepting a stale artifact."""
    if not _git_inside_work_tree(repo_root):
        return "unavailable"
    try:
        result = subprocess.run(["git", "ls-tree", "-r", "-z", "HEAD", "--", _CONTRACT_TREE_SCOPE],
                                 cwd=repo_root, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return "unavailable"
    if result.returncode != 0:
        return "unavailable"
    lines = []
    for record in result.stdout.split("\0"):
        if not record:
            continue
        meta, _, path = record.partition("\t")
        fields = meta.split()
        if len(fields) < 3 or not path or path in exclude_paths:
            continue
        lines.append(f"{fields[2]} {path}")
    return hashlib.sha256("\n".join(sorted(lines)).encode("utf-8")).hexdigest()


def compute_contract_tree_sha256(doc: dict, schema: dict, claim_bindings: dict,
                                  repo_root: Path = REPO_ROOT,
                                  selected_removal_artifact_path: str | None = None) -> str:
    """Deterministic content-identity digest binding a removal artifact to the EXACT contract
    state it was checked against (cycle4 fix -- subagent-summary-1 finding 3: a removal artifact's
    `tree_sha256` previously only had to be well-formed-64-hex, never bound to any real tree, so a
    stale artifact from a completely different tree/registry state was silently accepted).

    Inputs: the registry document under test, the schema, the claim-binding map, the GIT TREE
    IDENTITY of the tracked contract surface, and this validator's own source bytes. Canonical
    encoding: every JSON-like input is serialized with `json.dumps(..., sort_keys=True,
    ensure_ascii=False)` (so key order in the source file never matters), each part is labeled and
    sha256'd independently, and the labeled digests are joined SORTED (labels are fixed and
    unique) before the final sha256, so argument order cannot change the result. A removal
    artifact cannot include its own path/digest in that result without self-reference, so only
    the selected artifact's own tree line is omitted; sibling artifacts remain ordinary contract
    material, and every `removal_evidence.path` remains in the canonical registry component.
    Policy_id, result, exact artifact digest and invocation as_of remain independently checked by
    check_removal_evidence_contract().

    2026-09-03 (G2-a): the `evidence_manifest`/`tracked_index` components are replaced by the
    single `git_tree` component. The digest VALUE therefore changes -- safe here because the repo
    holds zero removal artifacts (registry.yaml: `status: retired` 0, `removal_evidence` 0), so
    nothing was ever bound to the old value."""
    contract_doc = copy.deepcopy(doc)
    artifact_paths = ({selected_removal_artifact_path}
                      if isinstance(selected_removal_artifact_path, str) else set())

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
        f"claim_bindings:{_digest_json(claim_bindings)}",
        f"git_tree:{_git_contract_tree_digest(repo_root, artifact_paths)}",
        f"policy_registry.py:{hashlib.sha256(script_bytes).hexdigest()}",
    ]
    combined = "\n".join(sorted(parts))
    return hashlib.sha256(combined.encode("utf-8")).hexdigest()


def _load_removal_artifact(repo_root: Path, rel_path):
    """Resolves `rel_path` through the SAME git+symlink gate as any other evidence path, then
    parses it as a JSON object. Returns (artifact_dict_or_None, reason_code_or_None)."""
    res = resolve_evidence_path(repo_root, rel_path)
    if res.status != "ok":
        return None, _EVIDENCE_RESOLUTION_REASON[res.status]
    doc, err = safe_load_json_object(repo_root / rel_path, "REMOVAL_EVIDENCE_ARTIFACT")
    if err is not None:
        return None, "REMOVAL_EVIDENCE_ARTIFACT_UNREADABLE"
    return doc, None


def check_removal_eligibility(policy: dict, repo_root: Path = REPO_ROOT,
                               expected_tree_sha256: str | None = None,
                               expected_as_of: datetime.date | None = None) -> list:
    """Selected-policy-only eligibility check -- callers wanting the full contract (schema +
    registry-wide lifecycle validation FIRST, plus contract-tree/as_of identity binding) use
    cmd_check_removal / the check_removal_full helper below. This function alone is intentionally
    narrow, and can be called with a single policy dict in isolation.

    cycle4 fix: `expected_tree_sha256`/`expected_as_of` are opt-in (default None -- a narrow
    caller that never passes them is unaffected). When given (always the case from
    check_removal_full, which alone has the doc/schema/claim_bindings needed to compute
    expected_tree_sha256), the artifact's own tree_sha256/as_of fields -- once individually
    shape-valid -- must equal them EXACTLY, not merely look like a digest/date."""
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
    artifact, err_code = _load_removal_artifact(repo_root, path)
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
                        repo_root: Path = REPO_ROOT,
                        claim_bindings: dict | None = None) -> list:
    """The full removal contract (cycle2 fix): schema-shape validation, THEN full-registry
    lifecycle validation (both with the given as_of/repo_root), and ONLY if the WHOLE registry is
    clean does the selected policy's own removal_evidence get checked. A retired policy with a
    self-cycling superseded_by, a missing retirement reason, or ANY other registry-wide
    violation blocks removal -- cycle1's check-removal validated only the single selected
    record, so a self-cycle + no-reason combination returned zero violations.

    cycle4 fix (subagent-summary-1 finding 3): the selected policy's removal artifact is now bound
    to the EXACT contract tree and as_of this invocation is checking -- computed once here via
    compute_contract_tree_sha256 (using the real committed claim_bindings.json by default, loaded
    fresh when the caller does not supply it, plus the live git tree identity) and passed down as
    expected_tree_sha256/expected_as_of. The identity binding lives solely in the eligibility
    check, so it cannot introduce any new violation class into unrelated registry-wide lifecycle
    fixtures."""
    shape = schema_violations(doc, schema)
    if shape:
        return shape
    lifecycle = evaluate_lifecycle(doc, as_of, repo_root, claim_bindings=claim_bindings)
    if lifecycle:
        return lifecycle
    by_id = {p.get("policy_id"): p for p in doc.get("policies", []) if isinstance(p, dict)}
    policy = by_id.get(policy_id)
    if policy is None:
        return [Violation("UNKNOWN_POLICY_ID", f"{policy_id!r} not found in registry", path="$.policies")]
    resolved_claim_bindings = claim_bindings if claim_bindings is not None else load_claim_bindings()
    expected_tree = compute_contract_tree_sha256(doc, schema, resolved_claim_bindings, repo_root,
                                                  selected_removal_artifact_path=(
                                                      policy.get("removal_evidence") or {}).get("path"))
    return check_removal_eligibility(policy, repo_root,
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
    # schema v2 (2026-08-14): v1 은 변종을 `vllm_repo`+`vllm_ref` 로만 표현할 수 있어 **자체 이식**
    #   (arch-wall 사다리 3번째 칸 -- stock ref + 빌드패치)을 담을 칸이 없었다. v2 는 `build_patch_selectors`
    #   와 `port_manifest` 를 신설한다. 루트/항목의 밑줄 접두 키는 스칼라 메타데이터로만 허용한다 --
    #   dict 를 허용하면 밑줄 키 밑에 변종을 숨길 수 있다(C3 의 `_hidden_active_variant` 반증실험과 동종).
    # schema v3 (2026-08-15): v2 는 사다리 **4번째 칸**(소스-repo 오버라이드 = 포크 핀)을 표현하지 못했다.
    #   두 군데가 막았고, 둘 다 "규칙이 사다리를 막는" 형태였다(2026-08-15 실측 · R3 착수 중 발견):
    #     ① `image_grammar` 가 **아키텍처-전용**(`-source-sm<n>…`)이라, 같은 vLLM 버전에서 자체이식(R2)과
    #        포크핀(R3)이 **같은 태그**를 강요받는다. 배선이 다른 이미지가 같은 이름을 갖는 것은 불변식이
    #        아니라 결함이며(plan_26081410 §10.3.2.1), R2 는 그때 상주 서빙 중이라 덮어쓰기가 곧 사고였다.
    #     ② 후보 단일성(`ARCH_VARIANT_MULTIPLE_CANDIDATE_TRACKS`)이 사다리와 충돌한다 -- 사다리는 칸을
    #        순차로 밟으라 요구하는데 앞 칸(R2)이 아직 CANDIDATE 다.
    #   우회(architecture 문자열을 `sm12xfork` 로 위장)는 게이트의 취지를 정확히 무력화하므로 채택하지
    #   않았다 -- 경로를 고친다(D3 법칙). v3 는 `source-fork<PR>` 트랙·이미지 접미어를 신설하고,
    #   후보 다중을 **image_tag 상이**를 조건으로 허용한다(개수가 아니라 덮어쓰기가 진짜 해악이었다).
    root_metadata = {key for key in resolved if key.startswith("_")}
    if (set(resolved) - root_metadata) != {"schema_version", "source_build_variants"} or \
            resolved.get("schema_version") not in (1, 2, 3):
        fail("ARCH_VARIANT_LEDGER_SHAPE_INVALID",
             "ledger requires schema_version in {1,2,3}, source_build_variants, and only scalar underscore metadata besides",
             ".claude/policies/arch_variant_ledger.json")
    for key in sorted(root_metadata):
        if not isinstance(resolved[key], str) or not resolved[key].strip():
            fail("ARCH_VARIANT_LEDGER_SHAPE_INVALID",
                 f"root metadata key {key!r} must be a non-empty string",
                 f".claude/policies/arch_variant_ledger.json.{key}")
    variants = resolved.get("source_build_variants")
    if not isinstance(variants, dict):
        fail("ARCH_VARIANT_LEDGER_MISSING", ".claude/policies/arch_variant_ledger.json.source_build_variants must be an object",
             ".claude/policies/arch_variant_ledger.json.source_build_variants")
        return out

    # 2026-09-03 (G2-a): 이 자리에 있던 evidence_manifest/tracked_index 2종 로드와 실패 시
    # `ARCH_VARIANT_TRUST_INPUT_INVALID` 단일 위반 + `return out` 은 **침묵 범위붕괴**였다 --
    # 원장 하나만 읽히지 않아도 아래 아치 검사 전부(후보 증거·사다리·이미지 문법·빌드패치 선택자·
    # Dockerfile/SKILL 배선)가 통째로 건너뛰어지고, 그 사실은 위반 1건으로만 보였다. 이제 아티팩트
    # 결속은 git 권위(resolve_evidence_path)로 직접 판정하므로 읽을 신뢰입력 파일이 없고, 아치 검사는
    # 어떤 경우에도 끝까지 돈다.

    def validate_bound_artifact(name, item, field, kind, pfx):
        pointer = item.get(field)
        if not isinstance(pointer, dict) or set(pointer) != {"path"}:
            fail("ARCH_VARIANT_ARTIFACT_POINTER_INVALID",
                 f"{name!r} {field} must be an exact single-key path object", f"{pfx}.{field}")
            return
        rel = pointer.get("path")
        if not isinstance(rel, str):
            fail("ARCH_VARIANT_ARTIFACT_PATH_INVALID", f"{name!r} {field} requires a path string",
                 f"{pfx}.{field}.path")
            return
        # 무결성 권위는 Git 자신이다 — 손으로 적은 sha256 을 곁에 두지 않는다. 포인터가 가리키는
        # 파일이 인덱스에 추적돼 있고 워킹트리 바이트가 스테이징 blob 과 같은지를 Git 이 판정한다.
        resolution = resolve_evidence_path(repo_root, rel)
        if resolution.status != "ok":
            fail("ARCH_VARIANT_ARTIFACT_UNBOUND",
                 f"{name!r} {field} is not a regular, git-tracked, undrifted file "
                 f"(resolution={resolution.status})",
                 f"{pfx}.{field}.path")
            return
        artifact, artifact_err = safe_load_json_object(repo_root / rel, "ARCH_VARIANT_ARTIFACT")
        if artifact_err is not None or not isinstance(artifact, dict):
            message = artifact_err.message if artifact_err is not None else "artifact root must be an object"
            fail("ARCH_VARIANT_ARTIFACT_INVALID", message, f"{pfx}.{field}.path")
            return
        common_fields = {"schema_version", "kind", "result", "variant_id", "vllm_repo", "vllm_ref",
                         "track", "architecture", "image_tag"}
        kind_fields = ({"approved_by", "source_evidence", "source_path", "approved_scope"}
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
            expected_source_path = (".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_"
                                    "SM12x_DeepSeek-V4-Flash_2노드서빙.md")
            source_path_value = source_path if isinstance(source_path, str) else ""
            source_resolution = (resolve_evidence_path(repo_root, source_path_value)
                                 if source_path_value else _EvidenceResolution("absent"))
            try:
                source_text = (repo_root / source_path_value).read_text(encoding="utf-8") if source_resolution.status == "ok" else ""
            except (OSError, UnicodeDecodeError):
                source_text = ""
            source_artifact_ok = (
                source_path == expected_source_path
                and source_resolution.status == "ok"
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

    # 빌드-평면 배선 원본. 선택자 검증이 항목 루프 안에서 이 둘을 읽으므로 루프 앞에서 한 번만 읽는다.
    dockerfile_rel = ".claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template"
    smoke_rel = ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh"
    try:
        dockerfile = (repo_root / dockerfile_rel).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        fail("ARCH_VARIANT_DOCKERFILE_UNREADABLE", str(exc), dockerfile_rel)
        dockerfile = ""
    try:
        smoke = (repo_root / smoke_rel).read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        smoke = ""   # 선택자를 선언한 항목이 있을 때만 위반이 된다(아래 validate_build_patch_selectors).

    def validate_candidate_evidence(name, item, pfx):
        """CANDIDATE 는 승인 아티팩트가 아니라 **사다리 근거**를 증거로 갖는다.

        정본 approval 아티팩트(`validate_bound_artifact`)는 스모크 PASS 후 HITL 승인 시점에 나오므로
        후보 단계에서는 존재할 수 없다. 대신 후보는 (a) 자기를 낳은 plan 과 (b) **stock 이 구조적으로
        불가함을 보인 testlog** 를 최소 1건 인용해야 한다 -- 이것이 "사다리 칸을 건너뛰지 않았다"는
        후보 단계의 유일한 검증 가능한 주장이다.

        digest 바인딩을 하지 않는 이유: `docs/{plan,testlog}` 는 docs.md 보관 matrix 상 **비추적**이라
        git 이 들지 않는다(배포 산출물이 아니다). 따라서 여기서 강제할 수 있는 것은 명명 SSOT 준수와
        비어있지 않음뿐이며, 디스크 존재는 검사하지 않는다(fresh clone·export 에서 위양성이 된다).
        """
        pointer = item.get("evidence")
        if not isinstance(pointer, dict) or set(pointer) != {"plan", "stock_infeasible_testlogs"}:
            fail("ARCH_VARIANT_CANDIDATE_EVIDENCE_INVALID",
                 f"{name!r} candidate evidence must be an exact plan/stock_infeasible_testlogs object",
                 f"{pfx}.evidence")
            return
        plan = pointer.get("plan")
        if not isinstance(plan, str) or not re.fullmatch(r"docs/plan/plan_\d{8}(?:_\d{2}_\d{2})?_[^/]+\.md", plan):
            fail("ARCH_VARIANT_CANDIDATE_EVIDENCE_INVALID",
                 f"{name!r} candidate evidence requires a docs/plan naming-SSOT plan path", f"{pfx}.evidence.plan")
        logs = pointer.get("stock_infeasible_testlogs")
        if (not isinstance(logs, list) or not logs or
                any(not isinstance(entry, str) or
                    not re.fullmatch(r"docs/testlog/testlog_\d{8}(?:_\d{2}_\d{2})?_[^/]+\.md", entry)
                    for entry in logs)):
            fail("ARCH_VARIANT_CANDIDATE_EVIDENCE_INVALID",
                 f"{name!r} candidate requires at least one docs/testlog path evidencing stock structural infeasibility",
                 f"{pfx}.evidence.stock_infeasible_testlogs")

    def validate_build_patch_selectors(name, selectors, pfx):
        """선택자가 **이미지 정체성에만** 쓰이는지 교차검증한다.

        선택자는 build-arg 이름 -> 값이다. 정적 파일끼리는 한쪽이 다른 쪽을 생성할 수 없으므로
        (workflow.md §결정론 규율 "단일 소유가 불가능하면 교차검증이 차선") 원장 선언을 두 배선과 대조한다:
          (1) Dockerfile 템플릿에 `ARG <NAME>=0` -- **부재 = stock** 이 기본이어야 침묵 변종화가 막힌다.
          (2) 멀티 스모크가 같은 모델 env 에서 값을 뽑아(`$(val <NAME>)`) SLAVE_IMGVARS 로 전달 --
              멀티는 클러스터-와이드 이미지가 전제라 빌드 인자가 한 톨이라도 갈리면 마스터만 변종이 된다.
        (2)를 요구하는 것이 곧 "serve 평면이 아니라 build 평면의 값"이라는 증명이다.
        """
        if not isinstance(selectors, dict) or not selectors:
            fail("ARCH_VARIANT_PORT_SELECTORS_INVALID",
                 f"{name!r} build_patch_selectors must be a non-empty build-arg object", f"{pfx}.build_patch_selectors")
            return
        for arg in sorted(selectors):
            value = selectors[arg]
            sub = f"{pfx}.build_patch_selectors.{arg}"
            if not re.fullmatch(r"[A-Z][A-Z0-9_]*", arg) or not isinstance(value, str) or not value.strip():
                fail("ARCH_VARIANT_PORT_SELECTORS_INVALID",
                     f"{name!r} selector {arg!r} must be an uppercase build-arg name with a non-empty string value", sub)
                continue
            if value == "0":
                fail("ARCH_VARIANT_PORT_SELECTOR_NOT_ENABLING",
                     f"{name!r} selector {arg!r}=0 equals the stock default and creates no variant", sub)
            if f"ARG {arg}=0" not in dockerfile:
                fail("ARCH_VARIANT_PORT_SELECTOR_UNGATED",
                     f"{name!r} selector {arg!r} lacks a default-off `ARG {arg}=0` gate in the source template", sub)
            if not smoke:
                fail("ARCH_VARIANT_PORT_SELECTOR_WIRING_UNREADABLE",
                     f"{name!r} selector {arg!r} cannot be proven cluster-wide: {smoke_rel} is unreadable", sub)
                continue
            imgvars_line = next((ln for ln in smoke.splitlines()
                                 if ln.strip().startswith("SLAVE_IMGVARS=")), "")
            if f"$(val {arg})" not in smoke or f"{arg}=$" not in imgvars_line:
                fail("ARCH_VARIANT_PORT_SELECTOR_NOT_CLUSTER_WIDE",
                     f"{name!r} selector {arg!r} is not read from the model env file and propagated to the slave "
                     f"as image identity", sub)

    def validate_port_manifest(name, port_manifest, pfx):
        """이식 원장(provenance). 결정론 앵커는 `upstream_base_sha` 다 -- vllm_ref 가 stock 릴리스 태그일
        때 이 40-hex SHA 가 그 태그의 실체를 고정한다. 포크 좌표(pr/fork)는 참조 출처라 선택이다.
        path 는 pre 슬롯(`build_patches_src/`) 산출물이라 토폴로지 통로 안에 있어야 한다."""
        if not isinstance(port_manifest, dict):
            fail("ARCH_VARIANT_PORT_MANIFEST_INVALID", f"{name!r} port_manifest must be an object",
                 f"{pfx}.port_manifest")
            return
        required = {"path", "upstream_base_sha", "files"}
        optional = {"pr_base_sha", "fork_head_sha"}
        missing = required - set(port_manifest)
        unknown = set(port_manifest) - required - optional
        if missing or unknown:
            fail("ARCH_VARIANT_PORT_MANIFEST_INVALID",
                 f"{name!r} port_manifest requires {sorted(required)} and allows only {sorted(optional)}",
                 f"{pfx}.port_manifest")
        path = port_manifest.get("path")
        if not isinstance(path, str) or not re.fullmatch(
                r"output/(?:single|multi)/build_patches_src/PROVENANCE\.json", path):
            fail("ARCH_VARIANT_PORT_MANIFEST_INVALID",
                 f"{name!r} port_manifest.path must be the pre-slot PROVENANCE.json inside a topology output lane",
                 f"{pfx}.port_manifest.path")
        for field in sorted(required | optional):
            if field.endswith("_sha") and field in port_manifest:
                value = port_manifest.get(field)
                if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{40}", value):
                    fail("ARCH_VARIANT_PORT_MANIFEST_INVALID",
                         f"{name!r} port_manifest.{field} must be a 40-hex commit SHA",
                         f"{pfx}.port_manifest.{field}")
        files = port_manifest.get("files")
        if not isinstance(files, int) or isinstance(files, bool) or files <= 0:
            fail("ARCH_VARIANT_PORT_MANIFEST_INVALID",
                 f"{name!r} port_manifest.files must be a positive integer file count",
                 f"{pfx}.port_manifest.files")

    metadata_keys = {"_note", "_deprecation_0.24.0"}
    promoted = []
    candidates = []
    # v3: 태그 충돌 검사는 **상태와 무관하게** 전 항목을 본다 -- SUPERSEDED 항목도 last-good 롤백 앵커
    #   이미지를 실제로 붙들고 있으므로(policy:LAST_GOOD_ROLLBACK_ANCHOR), 그걸 덮어쓰는 것이 가장 나쁘다.
    all_tagged = []
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
                                "image_tag", "track", "evidence", "status", "regression_evidence",
                                "build_patch_selectors", "port_manifest"}
        entry_metadata = {key for key in item if key.startswith("_")}
        unknown_fields = set(item) - allowed_entry_fields - entry_metadata
        if unknown_fields:
            fail("ARCH_VARIANT_ENTRY_UNKNOWN_FIELD",
                 f"{name!r} contains unknown fields {sorted(unknown_fields)}", pfx)
        for key in sorted(entry_metadata):
            if not isinstance(item[key], str) or not item[key].strip():
                fail("ARCH_VARIANT_ENTRY_UNKNOWN_FIELD",
                     f"{name!r} metadata key {key!r} must be a non-empty string", f"{pfx}.{key}")
        for field in ("variant_id", "architecture", "vllm_repo", "vllm_ref", "image_tag", "track", "status"):
            if not isinstance(item.get(field), str) or not item[field].strip():
                fail("ARCH_VARIANT_FIELD_MISSING", f"{name!r} requires non-empty {field}", f"{pfx}.{field}")
        track = item.get("track", "")
        architecture = item.get("architecture", "")
        image_tag = item.get("image_tag", "")
        suffix = track.removeprefix("source-") if isinstance(track, str) else ""
        # `source-<arch>-port` = **자체 이식** 칸(사다리 3번째). 포크를 핀하지 않으므로 아키텍처 접미어는
        #   `-port` 를 벗긴 쪽이며, 이미지 태그도 아키텍처 접미어로 끝난다(변종성은 selector 가 만든다).
        port_track = suffix.endswith("-port")
        # `source-fork<PR번호>` = **포크 핀** 칸(사다리 4번째 · v3 신설). 이 칸에서는 접미어가 아키텍처가
        #   아니라 **상류 PR 식별자**다 -- 변종성을 만드는 것이 우리 빌드패치가 아니라 남의 소스트리이기
        #   때문이다. 그래서 `architecture`(sm12x)와 접미어(fork41834)는 같을 수 없고, 대신 태그가 접미어로
        #   끝나는지만 본다. `\d+` 로 묶은 이유: `fork<모델명>` 같은 모델-키잉을 **문법 차원에서** 봉쇄한다
        #   (아래 image_grammar 와 같은 근거 -- 자유 꼬리를 주면 그 자리가 곧 모델 이름의 자리가 된다).
        fork_track = re.fullmatch(r"fork\d+", suffix) is not None
        arch_suffix = suffix.removesuffix("-port")
        if fork_track:
            # 접미어가 아키텍처를 대신하므로, `architecture` 가 여전히 **진짜 아키텍처**인지는 따로 본다
            #   (안 그러면 이 칸이 architecture 필드의 검증 구멍이 된다).
            if not re.fullmatch(r"sm\d+[a-z0-9]*", architecture):
                fail("ARCH_VARIANT_FIELD_MISSING",
                     f"{name!r} fork-pin track still requires a real architecture (sm<n>…), got {architecture!r}",
                     f"{pfx}.architecture")
            if not image_tag.endswith(f"-source-{suffix}"):
                fail("ARCH_VARIANT_NOT_SUPERSET_TAG",
                     f"{name!r} fork-pin image_tag must end in -source-<fork track suffix>", f"{pfx}.image_tag")
        elif not arch_suffix or architecture != arch_suffix or not image_tag.endswith(f"-source-{arch_suffix}"):
            fail("ARCH_VARIANT_NOT_SUPERSET_TAG", f"{name!r} image_tag must end in -source-<track suffix>",
                 f"{pfx}.image_tag")
        selectors = item.get("build_patch_selectors")
        port_manifest = item.get("port_manifest")
        ref = item.get("vllm_ref", "")
        ref_is_sha = bool(re.fullmatch(r"[0-9a-f]{40}", ref))
        if port_track:
            # 자체 이식은 stock 업스트림에 붙는다 -- 포크 repo 를 핀하면 그건 다음 칸(포크 핀)이지 이 칸이 아니다.
            if item.get("vllm_repo") != "https://github.com/vllm-project/vllm.git":
                fail("ARCH_VARIANT_PORT_TRACK_NOT_STOCK",
                     f"{name!r} is a self-port track and must pin the canonical upstream repo, not a fork",
                     f"{pfx}.vllm_repo")
            # stock 릴리스 태그를 허용하되 결정론은 잃지 않는다: 태그일 때는 port_manifest.upstream_base_sha
            #   가 40-hex 앵커 역할을 대신해야 한다(둘 다 없으면 해소값이 부동한다).
            if not ref_is_sha and not re.fullmatch(r"v\d+\.\d+\.\d+(?:(?:rc|a|b|\.dev)\d+)?", ref):
                fail("ARCH_VARIANT_REF_NOT_SHA",
                     f"{name!r} vllm_ref must be a 40-hex commit SHA or an upstream release tag", f"{pfx}.vllm_ref")
            if selectors is None:
                fail("ARCH_VARIANT_PORT_SELECTORS_INVALID",
                     f"{name!r} is a self-port track and must declare the build_patch_selectors that create it",
                     f"{pfx}.build_patch_selectors")
            if port_manifest is None:
                fail("ARCH_VARIANT_PORT_MANIFEST_INVALID",
                     f"{name!r} is a self-port track and requires a port_manifest", f"{pfx}.port_manifest")
            elif not ref_is_sha and not isinstance(port_manifest.get("upstream_base_sha"), str):
                fail("ARCH_VARIANT_PORT_MANIFEST_INVALID",
                     f"{name!r} pins a mutable tag and therefore requires port_manifest.upstream_base_sha",
                     f"{pfx}.port_manifest.upstream_base_sha")
        elif fork_track:
            # 포크 핀은 **남의 repo** 를 핀한다 -- canonical upstream 을 핀했다면 그건 이 칸이 아니다
            #   (stock 이거나 자체 이식 칸이다). 사다리를 건너뛴 것이 아니라 칸을 잘못 적은 것이므로,
            #   port_track 의 `NOT_STOCK` 검사와 정확히 대칭으로 세운다.
            if item.get("vllm_repo") == "https://github.com/vllm-project/vllm.git":
                fail("ARCH_VARIANT_FORK_TRACK_NOT_FORK",
                     f"{name!r} is a fork-pin track and must pin a fork repo, not the canonical upstream",
                     f"{pfx}.vllm_repo")
            # 포크는 force-push 가 가능하므로 **SHA 핀만** 허용한다. 자체 이식 칸이 릴리스 태그를 허용한 것은
            #   `port_manifest.upstream_base_sha` 라는 대체 앵커가 있었기 때문인데, 포크엔 그 대체가 없다.
            if not ref_is_sha:
                fail("ARCH_VARIANT_REF_NOT_SHA",
                     f"{name!r} fork-pin vllm_ref must be a 40-hex commit SHA (forks can force-push)",
                     f"{pfx}.vllm_ref")
            # 포크 트리가 소스를 이미 담고 있으므로 이식 원장은 성립하지 않는다 -- 있으면 두 칸을 섞은 것이고,
            #   그 상태에서는 "무엇이 변종성을 만들었나"가 원장에서 갈리지 않는다.
            if port_manifest is not None:
                fail("ARCH_VARIANT_PORT_MANIFEST_INVALID",
                     f"{name!r} is a fork-pin track and must not carry a port_manifest "
                     f"(the fork already ships the source)", f"{pfx}.port_manifest")
        elif not ref_is_sha:
            fail("ARCH_VARIANT_REF_NOT_SHA", f"{name!r} vllm_ref must be a 40-hex commit SHA", f"{pfx}.vllm_ref")
        if selectors is not None:
            validate_build_patch_selectors(name, selectors, pfx)
        if port_manifest is not None:
            validate_port_manifest(name, port_manifest, pfx)
        # v3: 접미어를 2택으로 넓힌다. **모델-키잉 봉쇄라는 취지는 그대로다** -- 이 검사가 지키는 불변식은
        #   "아키텍처 전용"이 아니라 "이미지 하나가 모든 모델을 서빙한다"(CLAUDE.md)이고, `fork\d+` 는 상류
        #   PR 번호라 모델명이 들어갈 자리가 없다. `sm\d+` 쪽에만 자유 꼬리(`[a-z0-9]*`)가 남는데 그건 기존
        #   아키텍처 변형(sm121a 등)을 담던 자리이므로 v2 그대로 둔다.
        image_grammar = (r"easy-vllm:\d+\.\d+\.\d+-cu\d+-(?:aarch64|x86_64)-source-"
                         r"(?:sm\d+[a-z0-9]*|fork\d+)")
        if not re.fullmatch(image_grammar, image_tag):
            fail("ARCH_VARIANT_MODEL_KEYED_IMAGE",
                 f"{name!r} image_tag must use the architecture-only canonical grammar "
                 f"(or the v3 fork-pin `-source-fork<PR>` form)",
                 f"{pfx}.image_tag")
        all_tagged.append((name, image_tag))
        status = item.get("status", "")
        # v4(2026-09-03): 퇴역 판정을 **산문 재구성**에서 **토큰 문법**으로 바꾼다.
        #   이전 판은 `_deprecation_<ver>` 메타키의 버전 + image_tag 의 버전 + 정해진 문장 전문을 조립해
        #   `status` 와 **완전일치**를 요구했다(3중 버전매칭). 그래서 실제로 강제한 것은 안전 성질이 아니라
        #   **문장 한 글자**였고, 결합도 취약했다 — `_deprecation_*` 메타키를 하나 더 달면 len != 1 이
        #   되어 expected 가 None 이 되고, 정상 퇴역 항목이 통째로 promoted 로 떨어져 무관한 위반
        #   (ACTIVE_NOT_VALIDATED)이 터진다. 지키려는 불변식은 하나뿐이다:
        #   **"퇴역했다"는 선언은 어느 버전에서 퇴역했는지를 못박아야 한다**(맨 `SUPERSEDED` 로 활성
        #   후보를 숨길 수 없다). 그 한 가지만 검사하고, 뒤따르는 산문은 사람 몫으로 자유롭게 둔다.
        #   `_deprecation_*` 메타키는 사람용 근거 노트로 위 metadata_keys allowlist 에 그대로 남는다.
        superseded = (isinstance(status, str)
                      and re.match(r"SUPERSEDED@\d+\.\d+\.\d+(?![\d.])", status) is not None)
        if isinstance(status, str) and status.startswith("SUPERSEDED") and not superseded:
            fail("ARCH_VARIANT_STATUS_INVALID",
                 f"{name!r} superseded status must name the deprecating version as "
                 f"SUPERSEDED@<major.minor.patch> -- a bare marker must not hide an active candidate",
                 f"{pfx}.status")
        # 상태는 3종이다: SUPERSEDED(퇴역) · CANDIDATE(등재만, 미승격) · VALIDATED(승격=기본 트랙).
        #   CANDIDATE 는 C5 절 문언 "before it becomes the default" 의 **이전** 상태다 -- 좌표를 원장에
        #   두어야 빌드가 재현 가능하게 읽지만(workflow.md §변종 좌표의 거처), 회귀 재스모크 증거는
        #   아직 존재할 수 없다. 승인 아티팩트도 마찬가지라 후보 전용 증거형을 쓴다.
        candidate = (not superseded and isinstance(status, str)
                     and re.match(r"CANDIDATE\b", status) is not None)
        if not candidate:
            validate_bound_artifact(name, item, "evidence", "arch_variant_approval", pfx)
        if superseded:
            continue
        if candidate:
            candidates.append((name, item))
        else:
            promoted.append((name, item))

    if len(promoted) > 1:
        fail("ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS",
             f"only one variant track may be active, got {[name for name, _ in promoted]}",
             ".claude/policies/arch_variant_ledger.json.source_build_variants")
    # v3(2026-08-15): 후보 다중을 **허용한다**. v2 는 "다음 승격 대상이 모호해진다"를 근거로 1개로 묶었는데,
    #   그 규칙이 정작 사다리를 막았다 -- 사다리는 칸을 순차로 밟으라 요구하고, 앞 칸(R2 자체이식)이 아직
    #   CANDIDATE 인 채로 다음 칸(R3 포크핀)을 등재해야 두 칸이 like-with-like 대조가 된다(같은 상류 SHA).
    #   재검토해 보면 v2 가 막고 싶었던 진짜 해악은 **개수가 아니라 같은 이름의 이미지를 서로 덮어쓰는 것**
    #   이었다. 그래서 개수 상한을 **태그 상이** 요구로 갈아끼운다. 승격 단일성은 그대로다 --
    #   VALIDATED(promoted)는 여전히 1개뿐이고(위 ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS), "다음 기본 트랙"의
    #   모호함은 거기서 이미 닫힌다. 후보는 정의상 아직 기본이 아니다.
    tags_seen: dict = {}
    for name, image_tag in all_tagged:
        tags_seen.setdefault(image_tag, []).append(name)
    for image_tag, names in sorted(tags_seen.items()):
        if len(names) > 1:
            fail("ARCH_VARIANT_IMAGE_TAG_COLLISION",
                 f"variants {sorted(names)} share image_tag {image_tag!r} -- distinct wiring must build to "
                 f"distinct images or one silently overwrites the other",
                 ".claude/policies/arch_variant_ledger.json.source_build_variants")
    for name, item in candidates:
        pfx = f".claude/policies/arch_variant_ledger.json.source_build_variants.{name}"
        # 후보가 회귀 증거를 들고 있으면 승격 게이트를 우회한 것이다(스모크 전 last-good 앵커 승격 금지).
        if "regression_evidence" in item:
            fail("ARCH_VARIANT_CANDIDATE_PREPROMOTED",
                 f"candidate variant {name!r} must not carry regression evidence before promotion to VALIDATED",
                 f"{pfx}.regression_evidence")
        validate_candidate_evidence(name, item, pfx)
    for name, item in promoted:
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
    # 2026-08-02: 포크 핀 앞에 **자체 이식** 칸 추가(testlog_26080223 — 자체 이식 3회 실패로
    #   포크 핀 폴백이 실증됐고, 그 실패 자체가 "이식을 먼저 시도한다"는 순서를 정당화한다).
    #   불변식은 그대로다: 고정 순서 · 건너뜀 금지 · 단일 활성 트랙.
    ladder = ("deps-패치 → 소스-게이트 패치 → **자체 이식** → "
              "**소스-repo 오버라이드(포크 핀)** → 체크포인트-교체")
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
                        claim_bindings: dict | None = None) -> list:
    """Runs every Python-level (non-schema) check and returns the combined violation list. Does
    NOT run schema_violations -- callers that also want shape validation call that separately
    (cmd_verify does both, in that order).

    2026-09-03 (G2-a): `evidence_manifest`/`tracked_index`/`governed_prose_snapshot` parameters
    are gone. Evidence resolution now asks git directly (no trust-input files to thread through),
    and the governed-prose snapshot tripwire was removed outright. `claim_bindings` stays opt-in
    (default None) exactly as before -- see module docstring "Clause-specific claim bindings"."""
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
    out += evidence_structural_violations(policies, repo_root)
    out += clause_executable_evidence_violations(policies)
    if claim_bindings is not None:
        out += claim_binding_violations(policies, claim_bindings)
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
    claim-bindings -- each failure emits the stable JSON envelope (exit 2) and never raises.
    Returns (repo_root, as_of, doc, schema, claim_bindings) on success (never returns on failure
    -- _emit always raises SystemExit).

    2026-09-03 (G2-a, plan_26090222): the evidence-manifest / tracked-index / governed-prose
    loads are GONE, not made optional. The plan's premise that a missing ledger would be "treated
    as normal" was false -- each miss emitted exit 2, a hard block that harness_verify's first
    stage propagates as BLOCKED. Removing the load removes the block at its source. Provenance is
    now asked of git per path, inside whatever --repo-root is being checked, so a repo export
    without its own git still fails closed (EVIDENCE_GIT_UNAVAILABLE) rather than inheriting this
    project's trust anchors.

    cycle4 fix (retained): --claim-bindings defaults to living INSIDE the effective repo_root
    (`<repo_root>/.claude/policies/claim_bindings.json`) rather than always resolving to THIS
    project's own committed file regardless of --repo-root -- unlike --registry/--schema (which
    stay script-relative-default). That file attests to a fact about the repo_root BEING CHECKED
    (its own clause-evidence contract), so an export lacking its own copy must fail closed."""
    repo_root = Path(args.repo_root).resolve() if args.repo_root else REPO_ROOT
    registry_path = Path(args.registry).resolve() if args.registry else REGISTRY_PATH
    schema_path = Path(args.schema).resolve() if args.schema else SCHEMA_PATH
    claim_bindings_path = Path(args.claim_bindings).resolve() if getattr(args, "claim_bindings", None) \
        else (repo_root / CLAIM_BINDINGS_REL)

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

    claim_bindings, err = safe_load_json_object(claim_bindings_path, "CLAIM_BINDINGS")
    if err is not None:
        _emit([err], 2)

    return (repo_root, as_of, doc, schema, claim_bindings)


def cmd_verify(args: argparse.Namespace) -> None:
    (repo_root, as_of, doc, schema, claim_bindings) = _load_inputs_or_emit(args)
    shape_violations = schema_violations(doc, schema)
    if shape_violations:
        _emit(shape_violations, 2)
        return
    lifecycle_violations = evaluate_lifecycle(doc, as_of, repo_root, claim_bindings=claim_bindings)
    _emit(lifecycle_violations, 1 if lifecycle_violations else 0)


def cmd_check_removal(args: argparse.Namespace) -> None:
    (repo_root, as_of, doc, schema, claim_bindings) = _load_inputs_or_emit(args)
    violations = check_removal_full(doc, args.policy_id, as_of, schema, repo_root,
                                     claim_bindings=claim_bindings)
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
        "evidence": [{"path": ".claude/policies/runtime/policy_registry.py", "supports": ["SELFTEST_OK.C1"],
                      "assertion_ids": ["evaluate_lifecycle"]}],
        "added_at": "2026-01-01", "last_reproduced": None,
        "remove_when": "Removable only once this self-test itself is deleted from the module.",
        "status": "candidate",
        "review": {"state": "current", "last_reviewed_at": "2026-01-15", "next_review_due": "2026-04-15",
                   "reviewer": "selftest"},
    }
    as_of = datetime.date(2026, 1, 15)
    violations = evaluate_lifecycle({"policies": [ok_policy]}, as_of, REPO_ROOT)
    if violations:
        raise RuntimeError(f"self-test OK fixture unexpectedly flagged: {[v.to_dict() for v in violations]}")

    bad_policy = dict(ok_policy, policy_id="SELFTEST_BAD",
                       clauses=[{"clause_id": "SELFTEST_BAD.C1", "statement": "x"}],
                       evidence=[{"path": ".claude/policies/runtime/policy_registry.py", "supports": ["SELFTEST_BAD.C1"],
                                  "assertion_ids": ["evaluate_lifecycle"]}],
                       added_at="2025-01-01",
                       last_reproduced="never", remove_when="xxxxxxxxxxxxxxxxxxxx",
                       status="active",
                       review={"state": "current", "last_reviewed_at": "2025-01-01",
                               "next_review_due": "2025-04-01", "reviewer": "selftest"})
    violations = evaluate_lifecycle({"policies": [bad_policy]}, as_of, REPO_ROOT)
    codes = {v.reason_code for v in violations}
    expected = {"LAZY_REMOVE_WHEN", "BAD_LAST_REPRODUCED_DATE", "UNREPRODUCED_90_DAY_NOT_CANDIDATE",
                "AS_OF_STATE_MISMATCH"}
    if not expected <= codes:
        raise RuntimeError(f"self-test BAD fixture missing expected codes: {expected - codes}, got {codes}")
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
    p_verify.add_argument("--claim-bindings", default=None,
                           help="default: <repo-root>/.claude/policies/claim_bindings.json")
    p_verify.add_argument("--repo-root", default=None)
    p_verify.set_defaults(func=cmd_verify)

    p_removal = sub.add_parser("check-removal")
    p_removal.add_argument("--policy-id", required=True)
    p_removal.add_argument("--as-of", required=True, help="YYYY-MM-DD, required, no wall-clock default")
    p_removal.add_argument("--registry", default=None)
    p_removal.add_argument("--schema", default=None)
    p_removal.add_argument("--claim-bindings", default=None,
                            help="default: <repo-root>/.claude/policies/claim_bindings.json")
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
